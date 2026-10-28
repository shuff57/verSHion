//! The four centres of a triangle: the circumcentre, the incentre, the centroid and the orthocentre, each drawn with
//! one of its construction lines and, where it has one, its circle.
//!
//! The triangle is fixed; the choice picks the centre. Drawn with `svg::Doc` only, on a light grid with ink axes.
use super::circle::num;
use super::svg::{self, Doc};
use super::*;

type Pt = (f64, f64);

pub const ID: &str = "triangle-centres";
const INK: &str = "#111111";
const SHAPE: &str = "#1f5fbf";
const SHAPE_PRINT: &str = "#111111";
const CONSTRUCTION: &str = "#a0704c";
const CONSTRUCTION_PRINT: &str = "#777777";
const MARK: &str = "#7a4fc4";
const MARK_PRINT: &str = "#333333";
const GRID_COLOUR: &str = "#e6ebf1";
const GRID_PRINT: &str = "#dedede";
const W: f64 = 470.0;
const PLOT: f64 = 330.0;
const L: f64 = 56.0;
const T: f64 = 16.0;
const ROWS_Y0: f64 = T + PLOT + 26.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;

/// The triangle, in data coordinates.
pub const A: Pt = (1.0, 1.0);
pub const B: Pt = (9.0, 2.0);
pub const C: Pt = (3.0, 8.0);
const NAMES: [&str; 3] = ["A", "B", "C"];

const CENTRES: [&str; 4] = ["Circumcentre", "Incentre", "Centroid", "Orthocentre"];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Triangle centres",
        blurb: "The circumcentre, incentre, centroid or orthocentre of a triangle, with the construction that finds it and, where it has one, its circle.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("centre", "Show the", &CENTRES, CENTRES[0]),
        ParamDef::bool("show_lines", "Show the construction", true),
        ParamDef::int("dec", "Decimal places", 0, 2, 1),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn sub(a: Pt, b: Pt) -> Pt { (a.0 - b.0, a.1 - b.1) }
fn add(a: Pt, b: Pt) -> Pt { (a.0 + b.0, a.1 + b.1) }
fn mul(a: Pt, k: f64) -> Pt { (a.0 * k, a.1 * k) }
fn len(a: Pt) -> f64 { a.0.hypot(a.1) }
fn mid(a: Pt, b: Pt) -> Pt { ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0) }
fn cross(a: Pt, b: Pt) -> f64 { a.0 * b.1 - a.1 * b.0 }

/// The four centres of the fixed triangle, in `CENTRES` order.
pub fn centres() -> [Pt; 4] {
    let (a, b, c) = (len(sub(B, C)), len(sub(C, A)), len(sub(A, B)));
    let centroid = mul(add(add(A, B), C), 1.0 / 3.0);
    // the circumcentre is where the perpendicular bisectors of AB and AC meet: X . d1 = m1 . d1 and X . d2 = m2 . d2
    let (m1, m2) = (mid(A, B), mid(A, C));
    let (d1, d2) = (sub(B, A), sub(C, A));
    let (b1, b2) = (m1.0 * d1.0 + m1.1 * d1.1, m2.0 * d2.0 + m2.1 * d2.1);
    let det = cross(d1, d2);
    let circum = if det.abs() < 1e-12 { m1 } else { ((b1 * d2.1 - d1.1 * b2) / det, (d1.0 * b2 - b1 * d2.0) / det) };
    // the incentre weights each vertex by the length of the opposite side
    let incentre = mul(add(add(mul(A, a), mul(B, b)), mul(C, c)), 1.0 / (a + b + c));
    // the orthocentre follows from the Euler relation H = A + B + C - 2O
    let ortho = sub(add(add(A, B), C), mul(circum, 2.0));
    [circum, incentre, centroid, ortho]
}

/// The radius of the circumcircle and of the incircle.
pub fn radii() -> (f64, f64) {
    let cs = centres();
    let circum_r = len(sub(A, cs[0]));
    let (a, b, c) = (len(sub(B, C)), len(sub(C, A)), len(sub(A, B)));
    let s = (a + b + c) / 2.0;
    let area = cross(sub(B, A), sub(C, A)).abs() / 2.0;
    (circum_r, area / s)
}

pub fn warnings(_p: &Params) -> Vec<String> { Vec::new() }

/// A segment clipped to a rectangle (Liang-Barsky); `None` when it misses the rectangle entirely.
fn clip_seg(x0: f64, y0: f64, x1: f64, y1: f64, rx0: f64, ry0: f64, rx1: f64, ry1: f64) -> Option<(Pt, Pt)> {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, x0 - rx0), (dx, rx1 - x0), (-dy, y0 - ry0), (dy, ry1 - y0)] {
        if p.abs() < 1e-12 {
            if q < 0.0 { return None; }
        } else {
            let r = q / p;
            if p < 0.0 { if r > t1 { return None; } if r > t0 { t0 = r; } } else { if r < t0 { return None; } if r < t1 { t1 = r; } }
        }
    }
    if t0 > t1 { return None; }
    Some(((x0 + t0 * dx, y0 + t0 * dy), (x0 + t1 * dx, y0 + t1 * dy)))
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}" stroke="#fff" stroke-width="2" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        svg::esc(s)
    )
}

pub struct Report {
    pub centres: [Pt; 4],
    pub radii: (f64, f64),
}

pub fn build(p: &Params) -> (String, Report) {
    let dec = p.i("dec").clamp(0, 2);
    let print = p.s("style") == "Print";
    let f = svg::label_factor(p.s("label_size"));
    let which = CENTRES.iter().position(|s| *s == p.s("centre")).unwrap_or(0);
    let cs = centres();
    let radii = radii();
    let centre = cs[which];

    // the grid takes in the triangle and whichever circle is drawn
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for q in [A, B, C] { x0 = x0.min(q.0); x1 = x1.max(q.0); y0 = y0.min(q.1); y1 = y1.max(q.1); }
    if which <= 1 { x0 = x0.min(centre.0 - radii.0.min(6.0)); x1 = x1.max(centre.0 + radii.0.min(6.0)); y0 = y0.min(centre.1 - radii.0.min(6.0)); y1 = y1.max(centre.1 + radii.0.min(6.0)); }
    x0 = x0.min(0.0) - 1.0; x1 = x1.max(0.0) + 1.0;
    y0 = y0.min(0.0) - 1.0; y1 = y1.max(0.0) + 1.0;
    let u = (PLOT / (x1 - x0)).min(PLOT / (y1 - y0));
    let (gw, gh) = ((x1 - x0) * u, (y1 - y0) * u);
    let ox = L + (PLOT - gw) / 2.0;
    let oy = T + (PLOT - gh) / 2.0;
    let px = |x: f64| ox + (x - x0) * u;
    let py = |y: f64| oy + (y1 - y) * u;

    let (ink, shape, con, mark, grid) = (
        if print { "#555555" } else { INK },
        if print { SHAPE_PRINT } else { SHAPE },
        if print { CONSTRUCTION_PRINT } else { CONSTRUCTION },
        if print { MARK_PRINT } else { MARK },
        if print { GRID_PRINT } else { GRID_COLOUR },
    );
    let mut d = Doc::new(W, ROWS_Y0 + 2.0 * ROW_H * f + 8.0);
    d.describe("Triangle centres", "A triangle with one of its four centres and the construction that finds it");

    // grid and axes
    let mut k = x0.ceil();
    while k <= x1 + 1e-9 { d.line(px(k), oy, px(k), oy + gh, grid, 1.0); k += 1.0; }
    let mut k = y0.ceil();
    while k <= y1 + 1e-9 { d.line(ox, py(k), ox + gw, py(k), grid, 1.0); k += 1.0; }
    d.line(ox, py(0.0), ox + gw, py(0.0), ink, 1.6);
    d.line(px(0.0), oy, px(0.0), oy + gh, ink, 1.6);

    // the construction, behind the triangle
    if p.b("show_lines") {
        match which {
            0 => {
                // perpendicular bisectors of AB and AC, and the circle through all three vertices
                let bisec = |d: &mut Doc, a: Pt, b: Pt| {
                    let m = mid(a, b);
                    let n = sub(b, a);
                    let l = len(n).max(1e-9);
                    let dir = (-n.1 / l, n.0 / l);
                    let (ax, ay) = (px(m.0) - dir.0 * 400.0, py(m.1) + dir.1 * 400.0);
                    let (bx, by) = (px(m.0) + dir.0 * 400.0, py(m.1) - dir.1 * 400.0);
                    // never past the plot: a construction line runs from edge to edge of the grid
                    if let Some((s, e)) = clip_seg(ax, ay, bx, by, ox, oy, ox + gw, oy + gh) { d.line(s.0, s.1, e.0, e.1, con, 1.4); }
                };
                bisec(&mut d, A, B);
                bisec(&mut d, A, C);
                d.circle(px(centre.0), py(centre.1), radii.0 * u, "none", con, 1.4);
            }
            1 => {
                // the bisectors at A and B, and the circle touching all three sides
                d.line(px(A.0), py(A.1), px(centre.0), py(centre.1), con, 1.4);
                d.line(px(B.0), py(B.1), px(centre.0), py(centre.1), con, 1.4);
                d.circle(px(centre.0), py(centre.1), radii.1 * u, "none", con, 1.4);
            }
            2 => {
                // the medians from B and C
                d.line(px(B.0), py(B.1), px(mid(A, C).0), py(mid(A, C).1), con, 1.4);
                d.line(px(C.0), py(C.1), px(mid(A, B).0), py(mid(A, B).1), con, 1.4);
            }
            _ => {
                // the altitudes from B and C
                d.line(px(B.0), py(B.1), px(centre.0), py(centre.1), con, 1.4);
                d.line(px(C.0), py(C.1), px(centre.0), py(centre.1), con, 1.4);
            }
        }
    }

    // the triangle
    d.polyline(&[(px(A.0), py(A.1)), (px(B.0), py(B.1)), (px(C.0), py(C.1)), (px(A.0), py(A.1))], shape, if print { 2.4 } else { 2.8 });

    // the centre and its name
    d.circle(px(centre.0), py(centre.1), 4.5, mark, mark, 0.0);
    let size = 14.0 * f;
    let name = ["circumcentre", "incentre", "centroid", "orthocentre"][which];
    d.raw(&text_el(px(centre.0) + 9.0, py(centre.1) - 7.0, size, "start", mark, name));
    for (i, q) in [A, B, C].iter().enumerate() {
        d.raw(&text_el(px(q.0) - 9.0, py(q.1) + 4.0, size, "end", shape, NAMES[i]));
    }

    // the readout
    let row1 = format!("{name}  ({}, {})", num(centre.0, dec), num(centre.1, dec));
    let row2 = match which {
        0 => format!("circumradius  {}", num(radii.0, dec)),
        1 => format!("inradius  {}", num(radii.1, dec)),
        2 => "the medians meet two thirds of the way from a vertex".to_string(),
        _ => "the altitudes meet at a right angle to the opposite side".to_string(),
    };
    for (i, line) in [row1, row2].iter().enumerate() {
        let mut s = line.clone();
        if let Some(c) = s.get_mut(0..1) { c.make_ascii_uppercase(); }
        d.raw(&format!(
            r##"<text x="{:.2}" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            24.0, ROWS_Y0 + i as f64 * ROW_H * f, ROW_FS * f, ink, svg::esc(&s)
        ));
    }

    (d.finish(), Report { centres: cs, radii })
}


pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "The circumcentre and the circle through the vertices", params: vec![s("centre", CENTRES[0])] },
        Example { name: "The incentre and the inscribed circle", params: vec![s("centre", CENTRES[1])] },
        Example { name: "The centroid: where the medians meet", params: vec![s("centre", CENTRES[2])] },
        Example { name: "The orthocentre: where the altitudes meet", params: vec![s("centre", CENTRES[3])] },
        Example { name: "The centroid with the construction hidden", params: vec![s("centre", CENTRES[2]), ("show_lines", Value::B(false))] },
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
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
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
    fn the_four_centres_are_what_geometry_says() {
        let cs = centres();
        // the centroid is the mean of the vertices
        assert!((cs[2].0 - (A.0 + B.0 + C.0) / 3.0).abs() < 1e-9 && (cs[2].1 - (A.1 + B.1 + C.1) / 3.0).abs() < 1e-9);
        // the circumcentre is the same distance from all three vertices
        for q in [A, B, C] { assert!((len(sub(cs[0], q)) - radii().0).abs() < 1e-9, "{cs:?}"); }
        // the incentre is the same distance from all three sides
        let r = radii().1;
        for (p, q) in [(A, B), (B, C), (C, A)] {
            let d = cross(sub(q, p), sub(cs[1], p)).abs() / len(sub(q, p));
            assert!((d - r).abs() < 1e-9, "incentre to {p:?}{q:?} is {d}, not {r}");
        }
        // the Euler line: G = (2O + H)/3, so OG : GH is 1 : 2
        let (o, g, h) = (cs[0], cs[2], cs[3]);
        assert!((g.0 - (2.0 * o.0 + h.0) / 3.0).abs() < 1e-9 && (g.1 - (2.0 * o.1 + h.1) / 3.0).abs() < 1e-9, "G = (2O + H)/3");
        // the orthocentre lies on every altitude: the line from a vertex to it is perpendicular to the opposite side
        for (v, (p, q)) in [(A, (B, C)), (B, (C, A)), (C, (A, B))] {
            let dot = (cs[3].0 - v.0) * (q.0 - p.0) + (cs[3].1 - v.1) * (q.1 - p.1);
            assert!(dot.abs() < 1e-9, "the altitude from {v:?} is not perpendicular to its opposite side: {dot}");
        }
    }

    #[test]
    fn each_choice_names_its_own_centre() {
        for (i, name) in ["circumcentre", "incentre", "centroid", "orthocentre"].iter().enumerate() {
            let t = texts(&render_of(&[s("centre", CENTRES[i])])).to_lowercase();
            assert!(t.contains(name), "{i}: {t}");
        }
        let t = texts(&render_of(&[s("centre", CENTRES[0])])).to_lowercase();
        assert!(t.contains("circumradius"), "{t}");
        let t = texts(&render_of(&[s("centre", CENTRES[1])])).to_lowercase();
        assert!(t.contains("inradius"), "{t}");
    }

    #[test]
    fn the_construction_can_be_hidden() {
        let with_lines = render_of(&[s("centre", CENTRES[0])]);
        let without = render_of(&[s("centre", CENTRES[0]), ("show_lines", Value::B(false))]);
        assert!(with_lines.matches("stroke-dasharray").count() >= without.matches("stroke-dasharray").count());
        assert!(with_lines.contains("<circle") && without.contains("<circle"), "the centre marker stays");
    }

    #[test]
    fn the_radii_are_positive_and_sane() {
        let (r, ri) = radii();
        assert!(r > 0.0 && ri > 0.0);
        assert!(ri < r, "the incircle is inside the circumcircle: {ri} vs {r}");
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
    fn triangle_centres_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/centres-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/centres-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
