//! Drawing a net: fitted to a 480 × 360 box, cut edges solid and fold lines dashed (or solid), the measures written on
//! the edges they belong to, the lines a measure needs drawn inside a face (a triangle's height, an apothem, a slant
//! height, a radius), right-angle squares, and the surface area and volume under it.
use super::geom::{self, Face, Outline, Pt};
use super::*;
use super::mathtext::{self, Bx, Draw};
use crate::figures::segment::{seg_hits, text_svg, tw, TextBox};
use std::f64::consts::PI;

const INK: &str = "#111827";
const INK_PRINT: &str = "#111111";
const FACE_FILL: &str = "#ffffff";
const FIT_W: f64 = 480.0;
const FIT_H: f64 = 360.0;
const MARGIN: f64 = 12.0;
const CUT_W: f64 = 2.4;
const FOLD_W: f64 = 1.6;
const BLANK: &str = "_____";

#[derive(Clone, Debug)]
pub enum Prim {
    /// A face's fill, under everything.
    Fill(Outline),
    Line { a: Pt, b: Pt, w: f64, dash: Option<&'static str> },
    /// A circle's or arc's outline (a = b = 0 for a whole circle).
    Arc { c: Pt, r: f64, a0: f64, a1: f64, w: f64 },
    Dot(Pt, f64),
    Square(Vec<Pt>),
}

pub struct Rendered {
    pub svg: String,
    /// Every text on the page, in page coordinates.
    pub texts: Vec<TextBox>,
    /// Every stroke, as segments in page coordinates (arcs as short chords), with whether it is a fold.
    pub strokes: Vec<(Pt, Pt)>,
    /// The faces on the page.
    pub faces: Vec<Face>,
    /// Leaders tying a label to its line or edge: (the label, where it starts on the line, where it ends by the label).
    pub leaders: Vec<(String, Pt, Pt)>,
    pub size: (f64, f64),
}

fn sub(a: Pt, b: Pt) -> Pt { (a.0 - b.0, a.1 - b.1) }
fn len(a: Pt) -> f64 { (a.0 * a.0 + a.1 * a.1).sqrt() }
fn unit_v(a: Pt) -> Pt {
    let l = len(a).max(1e-12);
    (a.0 / l, a.1 / l)
}
fn mid(a: Pt, b: Pt) -> Pt { ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0) }
fn along(a: Pt, b: Pt, t: f64) -> Pt { (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t) }

/// How far an upright w × h box reaches from its centre in direction `d`.
fn reach(d: Pt, w: f64, h: f64) -> f64 { d.0.abs() * w / 2.0 + d.1.abs() * h / 2.0 }

/// Widths measured with ASCII stand-ins for the signs the width table counts as a full em.
fn text_w(s: &str, size: f64) -> f64 {
    let t: String = s.chars().map(|c| match c {
        '\u{3c0}' => 'n',
        '\u{221a}' => 'v',
        '\u{b0}' | '\u{b2}' | '\u{b3}' => '.',
        '\u{d7}' => 'x',
        c => c,
    }).collect();
    tw(&t, size) * 0.94
}

/// A single letter in italic (x, a); words (cm, Base), digits, π and √ upright.
pub fn runs(s: &str) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    let cs: Vec<char> = s.chars().collect();
    let phrase = s.split(|c: char| !c.is_ascii_alphabetic()).any(|w| w.chars().count() > 1);
    let mut i = 0;
    while i < cs.len() {
        let st = i;
        let alpha = cs[i].is_ascii_alphabetic();
        while i < cs.len() && cs[i].is_ascii_alphabetic() == alpha {
            i += 1;
        }
        let run: String = cs[st..i].iter().collect();
        // a single letter stands for a measure (x, h); a word (cm, Base, length) is upright, and in a phrase so is every letter
        let italic = alpha && run.chars().count() == 1 && !phrase;
        match out.last_mut() {
            Some(l) if l.1 == italic => l.0.push_str(&run),
            _ => out.push((run, italic)),
        }
    }
    out
}

const SERIF: &str = "Liberation Serif, Noto Serif, DejaVu Serif, Times New Roman, serif";

/// A measure's label: its text, how it's written (italic runs, or set as a book sets a fraction or a root) and its size.
struct Lab {
    text: String,
    runs: Vec<(String, bool)>,
    math: Option<(Vec<Draw>, Bx)>,
    w: f64,
    h: f64,
}

fn lab_of(p: &Params, m: &Model, md: &str, fs: f64) -> Option<Lab> {
    let (body, unit, typed) = label_parts(p, m, md)?;
    let as_text = mode(p, md) == "Text";
    // a Text label's trailing words (6pi cm, x/2 cm, 2x units) are a unit: upright after the math before them
    let (body, unit) = if as_text { split_words(&body) } else { (body, unit) };
    let text = format!("{body}{unit}");
    // a typed measure, or a Text label such as x/2, sqrt(x) or 6pi, with a fraction, a root or π is set as a book sets it
    let set = if as_text { mathtext::text_is_math(&body) } else { typed && mathtext::needs_setting(&body) };
    if set && let Some(n) = mathtext::parse(&body) {
        let n = mathtext::Node::Row(vec![n, mathtext::Node::Text(unit.clone())]);
        let (bx, draws) = mathtext::layout(&n, fs, &|s, size| text_w(s, size));
        return Some(Lab { runs: vec![(text.clone(), false)], text, math: Some((draws, bx)), w: bx.w, h: bx.asc + bx.desc });
    }
    // a typed measure's digits upright; a Text label's lone letters italic; the unit always upright
    let mut runs = if typed { vec![(body.clone(), false)] } else { runs(&body) };
    if !unit.is_empty() {
        runs.push((unit, false));
    }
    Some(Lab { w: text_w(&text, fs), h: fs * 1.05, text, runs, math: None })
}

/// A Text label split into its math and the words after it ("6pi cm" is 6pi and " cm"), when there's math before the
/// words; a phrase ("Base length") stays whole.
fn split_words(s: &str) -> (String, String) {
    let words: Vec<&str> = s.split(' ').collect();
    let is_word = |w: &str| w.chars().count() >= 2 && w.chars().all(|c| c.is_ascii_alphabetic()) && !matches!(w.to_lowercase().as_str(), "pi" | "sqrt");
    let mut k = words.len();
    // a lone m at the end (metres) is a unit too, after math and a space
    if k > 1 && words[k - 1] == "m" {
        k -= 1;
    }
    while k > 0 && is_word(words[k - 1]) {
        k -= 1;
    }
    let head = words[..k].join(" ");
    let mathy = !head.trim().is_empty() && head.split(|c: char| !c.is_ascii_alphabetic()).all(|w| w.replace("sqrt", "").replace("pi", "").chars().count() <= 1);
    if k < words.len() && mathy {
        (head, format!(" {}", words[k..].join(" ")))
    } else {
        (s.to_string(), String::new())
    }
}

fn text_box(c: Pt, s: &str, size: f64, ink: &'static str, what: String) -> TextBox {
    TextBox { c, w: text_w(s, size), h: size * 1.05, size, runs: runs(s), ink, bold: false, serif: true, what }
}

fn push_lab(texts: &mut Vec<TextBox>, maths: &mut Vec<(usize, Vec<Draw>, Bx)>, c: Pt, lab: Lab, fs: f64, ink: &'static str, what: String) {
    if let Some((d, bx)) = lab.math {
        maths.push((texts.len(), d, bx));
    }
    let _ = lab.text;
    texts.push(TextBox { c, w: lab.w, h: lab.h, size: fs, runs: lab.runs, ink, bold: false, serif: true, what });
}

fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64), pad: f64) -> bool { a.0 < b.2 + pad && b.0 < a.2 + pad && a.1 < b.3 + pad && b.1 < a.3 + pad }

/// The faces on the page: scaled by `s` with (x0, y0) at the origin.
fn to_page(f: &Face, s: f64, o: Pt) -> Face {
    let tp = |q: Pt| ((q.0 - o.0) * s, (q.1 - o.1) * s);
    let outline = match &f.outline {
        Outline::Poly(v) => Outline::Poly(v.iter().map(|q| tp(*q)).collect()),
        Outline::Circle(c, r) => Outline::Circle(tp(*c), r * s),
        Outline::Sector { apex, r, a0, a1 } => Outline::Sector { apex: tp(*apex), r: r * s, a0: *a0, a1: *a1 },
    };
    let edges = f.edges.iter().map(|e| geom::Edge { a: tp(e.a), b: tp(e.b), key: e.key, role: e.role }).collect();
    Face { outline, edges }
}

/// Chords along an arc, for overlap tests.
fn arc_chords(c: Pt, r: f64, a0: f64, a1: f64) -> Vec<(Pt, Pt)> {
    let n = 48;
    let p = |k: usize| {
        let a = a0 + (a1 - a0) * k as f64 / n as f64;
        (c.0 + r * a.cos(), c.1 + r * a.sin())
    };
    (0..n).map(|k| (p(k), p(k + 1))).collect()
}

/// The measures labeled on an edge (the rest are drawn on a line inside a face), in the order they are placed.
const EDGE_MEASURES: [&str; 9] = ["length", "width", "tri_base", "tri_height", "hyp", "side", "height", "slant", "circ"];

/// Whether measure `md` is drawn on a line inside a face for this shape rather than on an edge.
fn inner(m: &Model, md: &str) -> bool {
    match md {
        "tri_height" => m.shape == Shape::Prism && m.base == Base::Isos,
        "slant" | "slant_w" => m.shape == Shape::Pyramid,
        "apothem" | "radius" | "angle" => true,
        _ => false,
    }
}

/// A square pyramid's two slant heights are one: the width's isn't drawn.
fn square_pyramid(m: &Model) -> bool { m.shape == Shape::Pyramid && m.base == Base::Rect && (m.v("length") - m.v("width")).abs() <= 1e-9 * m.v("length").max(1.0) }

pub fn render_full(p: &Params) -> Rendered {
    let m = model(p);
    let print = p.s("style") == "Print";
    let ink: &'static str = if print { INK_PRINT } else { INK };
    let fs = 18.0 * svg::label_factor(p.s("label_size"));
    let fold_dash: Option<&'static str> = if p.s("folds") == "Solid" { None } else { Some("7 5") };

    // the net, fitted to its box
    let raw = geom::build(&m);
    let (x0, y0, x1, y1) = geom::bounds(&raw);
    let s = (FIT_W / (x1 - x0).max(1e-9)).min(FIT_H / (y1 - y0).max(1e-9));
    let faces: Vec<Face> = raw.iter().map(|f| to_page(f, s, (x0, y0))).collect();
    let fold = geom::folds(&faces);

    let mut prims: Vec<Prim> = Vec::new();
    for f in &faces {
        prims.push(Prim::Fill(f.outline.clone()));
    }
    // folds once each (the face with the lower index draws it), then the cut edges over them
    for (i, f) in faces.iter().enumerate() {
        for (j, e) in f.edges.iter().enumerate() {
            if let Some(k) = fold[i][j] && k > i {
                prims.push(Prim::Line { a: e.a, b: e.b, w: FOLD_W, dash: fold_dash });
            }
        }
    }
    for (i, f) in faces.iter().enumerate() {
        for (j, e) in f.edges.iter().enumerate() {
            if fold[i][j].is_none() {
                prims.push(Prim::Line { a: e.a, b: e.b, w: CUT_W, dash: None });
            }
        }
        match &f.outline {
            Outline::Circle(c, r) => prims.push(Prim::Arc { c: *c, r: *r, a0: 0.0, a1: 2.0 * PI, w: CUT_W }),
            Outline::Sector { apex, r, a0, a1 } => prims.push(Prim::Arc { c: *apex, r: *r, a0: *a0, a1: *a1, w: CUT_W }),
            _ => {}
        }
    }

    let squares_on = p.b("squares") && (m.shape == Shape::Pyramid || (m.shape == Shape::Prism && m.base != Base::Rect));
    let mut squares: Vec<Vec<Pt>> = Vec::new();
    // a right-angle square at corner v with arms along u and w (unit), each at most 11 px and 35% of its side
    let square = |v: Pt, u: Pt, w: Pt, lu: f64, lw: f64| -> Vec<Pt> {
        let sz = 11.0f64.min(0.35 * lu).min(0.35 * lw);
        let p1 = (v.0 + u.0 * sz, v.1 + u.1 * sz);
        let p3 = (v.0 + w.0 * sz, v.1 + w.1 * sz);
        vec![p1, (p1.0 + w.0 * sz, p1.1 + w.1 * sz), p3]
    };
    if squares_on && m.shape == Shape::Prism && m.base == Base::RightTri {
        let c = faces.len() - 2;
        for f in &faces[c..] {
            if let Outline::Poly(v) = &f.outline {
                let (u, w) = (sub(v[1], v[0]), sub(v[2], v[0]));
                squares.push(square(v[0], unit_v(u), unit_v(w), len(u), len(w)));
            }
        }
    }

    // ── lines inside faces, with where their labels go ──
    // (measure, line start, line end, dashed, dot at the start, foot for a square, label at this fraction from the start)
    struct InnerLine {
        md: &'static str,
        a: Pt,
        b: Pt,
        dashed: bool,
        dot: bool,
        foot: Option<(Pt, Pt)>,
        at: f64,
        /// The face the line lies in, when its label should stay inside it (an apothem in its base).
        home: Option<usize>,
    }
    let mut lines: Vec<InnerLine> = Vec::new();
    let mut pending_apothem: Option<(usize, Vec<usize>)> = None;
    let shown = |md: &str| measures_of(m.shape, m.base).contains(&md) && on_net(m.shape, m.base, md) && mode(p, md) != "None";
    match (m.shape, m.base) {
        (Shape::Prism, Base::Isos) if shown("tri_height") => {
            if let Outline::Poly(v) = &faces[3].outline {
                let foot = mid(v[0], v[1]);
                lines.push(InnerLine { md: "tri_height", a: foot, b: v[2], dashed: true, dot: false, foot: Some((foot, v[1])), at: 0.5, home: None });
            }
        }
        // the apothem is drawn once the edges' labels are down, on an edge that leaves room for them and for its own
        (Shape::Prism, Base::Regular(n)) if shown("apothem") => {
            pending_apothem = Some((n, Vec::new()));
        }
        (Shape::Pyramid, _) => {
            // a regular pyramid's apothem, on its base: from the middle to the top edge's middle
            if matches!(m.base, Base::Regular(_)) && shown("apothem") {
                // not to edge 0, where the slant height and the side's label are
                pending_apothem = Some((0, vec![0]));
            }
            for (e, md) in [(0usize, "slant"), (1, "slant_w")] {
                if !shown(md) || (md == "slant_w" && (m.base != Base::Rect || square_pyramid(&m))) {
                    continue;
                }
                let base_e = &faces[0].edges[e];
                let foot = mid(base_e.a, base_e.b);
                let apex = geom::pyramid_apex(&faces, e);
                lines.push(InnerLine { md, a: foot, b: apex, dashed: true, dot: false, foot: Some((foot, base_e.b)), at: 0.32, home: None });
            }
        }
        (Shape::Cylinder | Shape::Cone, _) if shown("radius") => {
            if let Outline::Circle(c, r) = faces[1].outline {
                let (a, dot) = if m.diameter { ((c.0 - r, c.1), false) } else { (c, true) };
                lines.push(InnerLine { md: "radius", a, b: (c.0 + r, c.1), dashed: false, dot, foot: None, at: 0.5, home: None });
            }
        }
        _ => {}
    }
    for l in &lines {
        prims.push(Prim::Line { a: l.a, b: l.b, w: FOLD_W, dash: if l.dashed { Some("5 4") } else { None } });
        if l.dot {
            prims.push(Prim::Dot(l.a, 3.0));
        }
        if squares_on && let Some((f, other)) = l.foot {
            let far = if len(sub(l.b, f)) > len(sub(l.a, f)) { l.b } else { l.a };
            let (u, w) = (sub(far, f), sub(other, f));
            squares.push(square(f, unit_v(u), unit_v(w), len(u), len(w)));
        }
    }
    for sq in &squares {
        prims.push(Prim::Square(sq.clone()));
    }
    // the cone's sector angle: a small arc at the apex
    let mut angle_arc: Option<(Pt, f64)> = None;
    if m.shape == Shape::Cone && mode(p, "angle") != "None" && let Outline::Sector { apex, r, a0, a1 } = faces[0].outline {
        let ar = 28.0f64.min(0.25 * r);
        prims.push(Prim::Arc { c: apex, r: ar, a0, a1, w: FOLD_W });
        angle_arc = Some((apex, ar));
    }

    // every stroke so far, for keeping labels off them
    let mut strokes: Vec<(Pt, Pt)> = Vec::new();
    for q in &prims {
        match q {
            Prim::Line { a, b, .. } => strokes.push((*a, *b)),
            Prim::Arc { c, r, a0, a1, .. } => strokes.extend(arc_chords(*c, *r, *a0, *a1)),
            Prim::Square(v) => strokes.extend(v.windows(2).map(|w| (w[0], w[1]))),
            _ => {}
        }
    }

    // while the apothem waits for the edges' labels, they keep off the base's middle, where it starts
    let reserved: Vec<(Pt, Pt)> = match &pending_apothem {
        Some((fi, _)) => {
            let c = faces[*fi].centre();
            let k = 7.0;
            let (a, b, cc, d) = ((c.0 - k, c.1 - k), (c.0 + k, c.1 - k), (c.0 + k, c.1 + k), (c.0 - k, c.1 + k));
            vec![(a, b), (b, cc), (cc, d), (d, a)]
        }
        None => Vec::new(),
    };
    strokes.extend(reserved.iter().copied());
    // ── labels ──
    let mut texts: Vec<TextBox> = Vec::new();
    let mut leaders: Vec<(String, Pt, Pt)> = Vec::new();
    // the labels set as a book sets them (by their index in `texts`)
    let mut maths: Vec<(usize, Vec<Draw>, Bx)> = Vec::new();
    let clear = |r: (f64, f64, f64, f64), texts: &[TextBox], strokes: &[(Pt, Pt)]| !texts.iter().any(|t| overlaps(r, t.rect(), 2.0)) && !strokes.iter().any(|(a, b)| seg_hits(*a, *b, r, 1.5));
    // the box at `c`, slid along `dir` in 8 px steps up to `room` either way until it's clear
    let slide = |c: Pt, w: f64, h: f64, dir: Pt, room: f64, texts: &[TextBox], strokes: &[(Pt, Pt)]| -> Pt {
        let rect = |c: Pt| (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
        if clear(rect(c), texts, strokes) {
            return c;
        }
        let mut k = 1.0;
        while k * 8.0 <= room {
            for sgn in [1.0, -1.0] {
                let q = (c.0 + dir.0 * k * 8.0 * sgn, c.1 + dir.1 * k * 8.0 * sgn);
                if clear(rect(q), texts, strokes) {
                    return q;
                }
            }
            k += 1.0;
        }
        c
    };
    let mut taken: Vec<(u32, u32)> = Vec::new();
    let same_edge = |a: (Pt, Pt), b: (Pt, Pt)| {
        let near = |p: Pt, q: Pt| (p.0 - q.0).abs() < 1e-6 && (p.1 - q.1).abs() < 1e-6;
        (near(a.0, b.0) && near(a.1, b.1)) || (near(a.0, b.1) && near(a.1, b.0))
    };
    let mut deferred: Vec<(&'static str, Lab, Pt, Pt, (Pt, Pt))> = Vec::new();
    for md in EDGE_MEASURES {
        if inner(&m, md) || !measures_of(m.shape, m.base).contains(&md) || !on_net(m.shape, m.base, md) {
            continue;
        }
        let Some(lab) = lab_of(p, &m, md, fs) else { continue };
        // the best edge: a cut edge facing down (the lowest), then left (the leftmost), then up (the topmost), then any
        // other cut edge (the rightmost); fold edges last (the lowest); the first listed wins a tie. An edge's glue
        // partner (the same key) is taken with it.
        let mut best: Option<(f64, usize, usize)> = None;
        for (i, f) in faces.iter().enumerate() {
            for (j, e) in f.edges.iter().enumerate() {
                if e.role != Some(md) || taken.contains(&e.key) {
                    continue;
                }
                let mp = mid(e.a, e.b);
                let n = f.out_normal(j);
                let score = if fold[i][j].is_some() {
                    -100.0 + mp.1 * 0.001
                } else if n.1 > 0.7 {
                    40.0 + mp.1 * 0.001
                } else if n.0 < -0.7 {
                    30.0 - mp.0 * 0.001
                } else if n.1 < -0.7 {
                    20.0 - mp.1 * 0.001
                } else {
                    10.0 + mp.0 * 0.001
                };
                if best.as_ref().is_none_or(|b| score > b.0 + 1e-9) {
                    best = Some((score, i, j));
                }
            }
        }
        let Some((_, i, j)) = best else { continue };
        let e = &faces[i].edges[j];
        taken.push(e.key);
        let (w, h) = (lab.w, lab.h);
        // a fold's label goes just inside the larger of its two faces (or the other, when the larger has no room); a cut
        // edge's just outside
        let dirs: Vec<Pt> = match fold[i][j] {
            Some(k) => {
                let jj = faces[k].edges.iter().position(|o| same_edge((o.a, o.b), (e.a, e.b))).unwrap_or(0);
                let into_k = { let n = faces[k].out_normal(jj); (-n.0, -n.1) };
                let into_i = { let n = faces[i].out_normal(j); (-n.0, -n.1) };
                if faces[k].area() > faces[i].area() + 1e-9 { vec![into_k, into_i] } else { vec![into_i, into_k] }
            }
            None => vec![faces[i].out_normal(j)],
        };
        // the face each direction leads into (a fold's label stays inside it), or none (a cut edge's stays outside the net)
        let homes: Vec<Option<usize>> = match fold[i][j] {
            Some(k) if faces[k].area() > faces[i].area() + 1e-9 => vec![Some(k), Some(i)],
            Some(k) => vec![Some(i), Some(k)],
            None => vec![None],
        };
        // the circumference sits a fifth of the way along, clear of the circle on the middle of the edge
        let (l, r) = if e.a.0 <= e.b.0 { (e.a, e.b) } else { (e.b, e.a) };
        let at = if md == "circ" { along(l, r, 0.2) } else { mid(e.a, e.b) };
        let el = len(sub(e.b, e.a));
        let along_e = unit_v(sub(e.b, e.a));
        let rect = |c: Pt| (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
        let start = |dir: Pt| {
            let off = 6.0 + reach(dir, w, h);
            (at.0 + dir.0 * off, at.1 + dir.1 * off)
        };
        // over the edge's own span: the label's middle projects onto the edge
        let within = |q: Pt| {
            let t = ((q.0 - e.a.0) * (e.b.0 - e.a.0) + (q.1 - e.a.1) * (e.b.1 - e.a.1)) / (el * el).max(1e-12);
            (0.0..=1.0).contains(&t)
        };
        let mut c = start(dirs[0]);
        let mut found = false;
        // just beside the edge, slid along it, inside the larger face (a fold) or outside (a cut edge); then the other face
        let at_home = |q: Pt, home: Option<usize>| match home {
            Some(f) => inside_any(&faces[f..=f], q),
            None => !inside_any(&faces, q),
        };
        'near: for (dir, home) in dirs.iter().zip(homes.iter().copied()) {
            let q = slide(start(*dir), w, h, along_e, 0.4 * el, &texts, &strokes);
            if clear(rect(q), &texts, &strokes) && within(q) && at_home(q, home) {
                c = q;
                found = true;
                break 'near;
            }
            // a little further from the edge, still over it
            let span = (0.5 * el - 4.0).max(0.0);
            for out in 1..=2 {
                let o = 6.0 + reach(*dir, w, h) + out as f64 * 6.0;
                let mut k = 0.0;
                while k * 8.0 <= span {
                    for sg in [1.0, -1.0] {
                        let q = (at.0 + dir.0 * o + along_e.0 * k * 8.0 * sg, at.1 + dir.1 * o + along_e.1 * k * 8.0 * sg);
                        if clear(rect(q), &texts, &strokes) && within(q) && at_home(q, home) {
                            c = q;
                            found = true;
                            break 'near;
                        }
                    }
                    k += 1.0;
                }
            }
        }
        // no room by the edge: placed once every label that fits by its own edge is down, so it keeps clear of them
        if !found {
            deferred.push((md, lab, at, dirs[0], (e.a, e.b)));
            continue;
        }
        push_lab(&mut texts, &mut maths, c, lab, fs, ink, format!("lab {md}"));
    }
    strokes.retain(|q| !reserved.contains(q));
    // the apothem, now the edges' labels are down: to an edge whose line and label keep clear of them
    if let Some((fi, skip)) = pending_apothem.take() {
        let (w, h) = lab_of(p, &m, "apothem", fs).map(|l| (l.w, l.h)).unwrap_or((0.0, 0.0));
        let obstacles: Vec<(f64, f64, f64, f64)> = texts.iter().map(|t| t.rect()).collect();
        let (c, f, other) = apothem_to(&faces[fi], w, h, &skip, &obstacles);
        prims.push(Prim::Line { a: c, b: f, w: FOLD_W, dash: None });
        prims.push(Prim::Dot(c, 3.0));
        strokes.push((c, f));
        if squares_on {
            let (u, ww) = (sub(c, f), sub(other, f));
            let sq = square(f, unit_v(u), unit_v(ww), len(u), len(ww));
            strokes.extend(sq.windows(2).map(|q| (q[0], q[1])));
            squares.push(sq.clone());
            prims.push(Prim::Square(sq));
        }
        lines.push(InnerLine { md: "apothem", a: c, b: f, dashed: false, dot: true, foot: Some((f, other)), at: 0.5, home: Some(fi) });
    }
    // the inner lines' labels: beside a steep line (to its right), above a flat one; when that crosses a line, the other
    // side, then a little further along the line either way
    for l in &lines {
        let Some(lab) = lab_of(p, &m, l.md, fs) else { continue };
        let (w, h) = (lab.w, lab.h);
        let d = sub(l.b, l.a);
        let side: Pt = if d.1.abs() > d.0.abs() { (1.0, 0.0) } else { (0.0, -1.0) };
        let rect = |c: Pt| (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
        let place = |t: f64, sd: Pt| {
            let at = along(l.a, l.b, t);
            let off = 6.0 + reach(sd, w, h);
            (at.0 + sd.0 * off, at.1 + sd.1 * off)
        };
        let first = place(l.at, side);
        let mut pick = None;
        'find: for k in 0..=24 {
            for sg in [1.0, -1.0] {
                if k == 0 && sg < 0.0 {
                    continue;
                }
                let t = l.at + sg * k as f64 * 0.02;
                if !(0.08..=0.92).contains(&t) {
                    continue;
                }
                for sd in [side, (-side.0, -side.1)] {
                    let c = place(t, sd);
                    let r = rect(c);
                    let covers_dot = l.dot && l.a.0 > r.0 - 4.0 && l.a.0 < r.2 + 4.0 && l.a.1 > r.1 - 4.0 && l.a.1 < r.3 + 4.0;
                    if !covers_dot && clear(r, &texts, &strokes) {
                        pick = Some(c);
                        break 'find;
                    }
                }
            }
        }
        // a line inside a face (an apothem): anywhere clear inside that face, nearest the line's middle, tied to it by a
        // short leader when it's away from the line
        if pick.is_none() && let Some(fi) = l.home {
            let lm = along(l.a, l.b, 0.5);
            let face = std::slice::from_ref(&faces[fi]);
            'ring: for ring in 0..=40 {
                let rr = ring as f64 * 4.0;
                let n_ang = if ring == 0 { 1 } else { 24 };
                for k in 0..n_ang {
                    let a = k as f64 * 2.0 * PI / n_ang as f64;
                    let q = (lm.0 + rr * a.cos(), lm.1 + rr * a.sin());
                    let r = rect(q);
                    let r3 = (r.0 - 3.0, r.1 - 3.0, r.2 + 3.0, r.3 + 3.0);
                    let inside = [(r3.0, r3.1), (r3.2, r3.1), (r3.2, r3.3), (r3.0, r3.3)].iter().all(|p| inside_any(face, *p));
                    let dot_clear = !(l.a.0 > r3.0 && l.a.0 < r3.2 && l.a.1 > r3.1 && l.a.1 < r3.3);
                    if inside && dot_clear && clear(r, &texts, &strokes) {
                        // a leader only when the label is away from the line: from the line's nearest point
                        let d = sub(l.b, l.a);
                        let tq = (((q.0 - l.a.0) * d.0 + (q.1 - l.a.1) * d.1) / (d.0 * d.0 + d.1 * d.1).max(1e-12)).clamp(0.2, 0.8);
                        let near = along(l.a, l.b, tq);
                        let end = (near.0.clamp(r.0 - 2.0, r.2 + 2.0), near.1.clamp(r.1 - 2.0, r.3 + 2.0));
                        if len(sub(end, near)) > 10.0 && !seg_hits(l.a, l.b, (r.0 - 8.0, r.1 - 8.0, r.2 + 8.0, r.3 + 8.0), 0.0) {
                            prims.push(Prim::Dot(near, 2.2));
                            prims.push(Prim::Line { a: near, b: end, w: 1.0, dash: None });
                            strokes.push((near, end));
                            leaders.push((format!("lab {}", l.md), near, end));
                        }
                        pick = Some(q);
                        break 'ring;
                    }
                }
            }
        }
        // no room beside it: on the line itself, the line broken round the label
        if pick.is_none() {
            let own = |q: &(Pt, Pt)| (q.0 == l.a && q.1 == l.b) || (q.0 == l.b && q.1 == l.a);
            let others: Vec<(Pt, Pt)> = strokes.iter().filter(|q| !own(q)).copied().collect();
            'on: for k in 0..=180 {
                for sg in [1.0, -1.0] {
                    let t = 0.5 + sg * k as f64 * 0.0025;
                    let c = along(l.a, l.b, t);
                    let r = rect(c);
                    // the line has to show past the label at both ends (and its dot stay clear of it)
                    let Some((t0, t1)) = clip_t(l.a, l.b, (r.0 - 3.0, r.1 - 3.0, r.2 + 3.0, r.3 + 3.0)) else { continue };
                    // (a line in a small face, an apothem, may be covered more: its dot and square show its ends)
                    let keep = if l.home.is_some() { 0.15 } else { 0.25 };
                    let covers_dot = l.dot && l.a.0 > r.0 - 4.0 && l.a.0 < r.2 + 4.0 && l.a.1 > r.1 - 4.0 && l.a.1 < r.3 + 4.0;
                    if t0 < keep || t1 > 1.0 - keep || covers_dot {
                        continue;
                    }
                    if clear(r, &texts, &others) {
                        {
                            let pieces = [(l.a, along(l.a, l.b, t0)), (along(l.a, l.b, t1), l.b)];
                            if let Some(ix) = prims.iter().position(|q| matches!(q, Prim::Line { a, b, .. } if *a == l.a && *b == l.b)) {
                                let Prim::Line { w, dash, .. } = prims[ix].clone() else { unreachable!() };
                                prims.remove(ix);
                                for (a, b) in pieces {
                                    if len(sub(b, a)) > 0.5 {
                                        prims.insert(ix, Prim::Line { a, b, w, dash });
                                    }
                                }
                            }
                            strokes.retain(|q| !own(q));
                            strokes.extend(pieces.iter().filter(|(a, b)| len(sub(*b, *a)) > 0.5).copied());
                        }
                        pick = Some(c);
                        break 'on;
                    }
                }
            }
        }
        // still no room: in clear space outside the net, nearest first, tied to the line by a thin leader
        if pick.is_none() {
            // (a diameter's leader starts out toward its end, so it isn't read as a radius from the centre)
            let at = along(l.a, l.b, if l.md == "radius" && !l.dot { 0.8 } else { l.at });
            let sq_segs: Vec<(Pt, Pt)> = squares.iter().flat_map(|v| v.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>()).collect();
            let spot = far_out_avoiding(at, side, w, h, &faces, &texts, &strokes, &sq_segs, Some(unit_v(sub(l.b, l.a)))).or_else(|| far_out(at, side, w, h, &faces, &texts, &strokes));
            if let Some((q, end)) = spot {
                prims.push(Prim::Dot(at, 2.2));
                prims.push(Prim::Line { a: at, b: end, w: 1.0, dash: None });
                strokes.push((at, end));
                leaders.push((format!("lab {}", l.md), at, end));
                pick = Some(q);
            }
        }
        // the last resort before an overlap: any clear spot, nearest first, on a plain leader
        if pick.is_none() {
            let at = along(l.a, l.b, l.at);
            if let Some((q, end)) = clear_spot(at, w, h, &texts, &strokes, size_of_page(&faces, &texts)) {
                prims.push(Prim::Dot(at, 2.2));
                prims.push(Prim::Line { a: at, b: end, w: 1.0, dash: None });
                strokes.push((at, end));
                leaders.push((format!("lab {}", l.md), at, end));
                pick = Some(q);
            }
        }
        push_lab(&mut texts, &mut maths, pick.unwrap_or(first), lab, fs, ink, format!("lab {}", l.md));
    }
    if let Some((apex, ar)) = angle_arc && let Some(lab) = lab_of(p, &m, "angle", fs) {
        // under the arc, moved down the sector's middle until it's clear of the sector's edges (a narrow sector)
        let (w, h) = (lab.w, lab.h);
        let mut c = (apex.0, apex.1 + ar + 4.0 + h / 2.0);
        let r_sec = match faces[0].outline { Outline::Sector { r, .. } => r, _ => 0.0 };
        let rect = |c: Pt| (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
        let mut k = 0.0;
        while !clear(rect(c), &texts, &strokes) && c.1 + h / 2.0 < apex.1 + r_sec - 4.0 {
            k += 1.0;
            c = (apex.0, apex.1 + ar + 4.0 + h / 2.0 + 4.0 * k);
        }
        push_lab(&mut texts, &mut maths, c, lab, fs, ink, "lab angle".into());
    }

    // the edge labels with no room by their edge: in clear space outside the net, nearest first, tied to the edge's
    // middle by a leader
    for (md, lab, at, dir0, (ea, eb)) in deferred {
        let (w, h) = (lab.w, lab.h);
        // a leader starting where a slant height or a triangle's height meets the edge would read as that line's:
        // start it a little along the edge instead
        let at = if lines.iter().any(|l| len(sub(l.a, at)) < 6.0 || len(sub(l.b, at)) < 6.0) { along(ea, eb, 0.3) } else { at };
        let rect = |c: Pt| (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
        let el = len(sub(eb, ea));
        let along_e = unit_v(sub(eb, ea));
        let e = geom::Edge { a: ea, b: eb, key: (0, 0), role: None };
        let dirs = [dir0];
        let mut c = (at.0 + dir0.0 * (6.0 + reach(dir0, w, h)), at.1 + dir0.1 * (6.0 + reach(dir0, w, h)));
        let mut found = false;
            // no room by the edge: in clear space outside the net, nearest first, tied to the edge's middle by a leader
            if !found && let Some((q, end)) = far_out(at, dirs[0], w, h, &faces, &texts, &strokes) {
                prims.push(Prim::Dot(at, 2.2));
                prims.push(Prim::Line { a: at, b: end, w: 1.0, dash: None });
                strokes.push((at, end));
                leaders.push((format!("lab {md}"), at, end));
                c = q;
                found = true;
            }
            // a crowded edge: further out, and further along it, tied back to the edge by a leader once away from it
            if !found {
                let dir = dirs[0];
                let off = 6.0 + reach(dir, w, h);
                let span = (0.5 * el - 4.0).max(0.0);
                let reach_along = 0.5 * el + w / 2.0 + 24.0;
                'more: for out in 0..=14 {
                        let o = off + out as f64 * 6.0;
                        let mut k = 0.0;
                        while k * 8.0 <= reach_along {
                            for sg in [1.0, -1.0] {
                                    let q = (at.0 + dir.0 * o + along_e.0 * k * 8.0 * sg, at.1 + dir.1 * o + along_e.1 * k * 8.0 * sg);
                                    let foot = along(e.a, e.b, (0.5 + sg * k * 8.0 / el.max(1e-9)).clamp(0.2, 0.8));
                                    let needs = out >= 3 || k * 8.0 > span;
                                    let r = rect(q);
                                    let end = (foot.0.clamp(r.0 - 2.0, r.2 + 2.0), foot.1.clamp(r.1 - 2.0, r.3 + 2.0));
                                    let leader_ok = !needs || (len(sub(end, foot)) > 6.0 && !texts.iter().any(|tb| seg_hits(foot, end, tb.rect(), 1.0)));
                                    if leader_ok && clear(r, &texts, &strokes) {
                                        if needs {
                                                prims.push(Prim::Dot(foot, 2.2));
                                                prims.push(Prim::Line { a: foot, b: end, w: 1.0, dash: None });
                                                strokes.push((foot, end));
                                                leaders.push((format!("lab {md}"), foot, end));
                                        }
                                        c = q;
                                        break 'more;
                                    }
                            }
                            k += 1.0;
                        }
                }
            }
        push_lab(&mut texts, &mut maths, c, lab, fs, ink, format!("lab {md}"));
    }

    // ── the surface area and the volume ──
    let (fx0, fy0, fx1, fy1) = extent(&faces, &texts);
    if p.b("readout") {
        let (sa, vol) = measures(&m);
        let dec = decimals(p);
        let ans = p.b(SHOW_ANSWERS);
        let rs = fs * 0.85;
        let rows = [
            format!("Surface area  {}", if ans { format!("{}{}", num(sa, dec), unit_pow(p, false)) } else { BLANK.into() }),
            format!("Volume  {}", if ans { format!("{}{}", num(vol, dec), unit_pow(p, true)) } else { BLANK.into() }),
        ];
        let cx = (fx0 + fx1) / 2.0;
        for (k, row) in rows.iter().enumerate() {
            let y = fy1 + 16.0 + rs * 0.6 + k as f64 * rs * 1.45;
            let mut tb = text_box((cx, y), row, rs, ink, format!("readout {k}"));
            // the words upright: only a letter standing for a measure is italic
            tb.runs = vec![(row.clone(), false)];
            texts.push(tb);
        }
    }
    let _ = (fx0, fy0);

    // ── the page ──
    let (bx0, by0, bx1, by1) = extent(&faces, &texts);
    let off = (MARGIN - bx0, MARGIN - by0);
    let (w, h) = (bx1 - bx0 + 2.0 * MARGIN, by1 - by0 + 2.0 * MARGIN);
    let mut d = svg::Doc::new(w.ceil(), h.ceil());
    let (title, desc) = describe(&m);
    d.describe(&title, &desc);
    let t = |q: Pt| (q.0 + off.0, q.1 + off.1);
    let fill = FACE_FILL;
    for q in &prims {
        match q {
            Prim::Fill(o) => match o {
                Outline::Poly(v) => {
                    d.poly(&v.iter().map(|q| t(*q)).collect::<Vec<_>>(), fill, "none", 0.0);
                }
                Outline::Circle(c, r) => {
                    let c = t(*c);
                    d.circle(c.0, c.1, *r, fill, "none", 0.0);
                }
                Outline::Sector { apex, r, a0, a1 } => {
                    d.path(&sector_path(t(*apex), *r, *a0, *a1), fill, "none", 0.0);
                }
            },
            Prim::Line { a, b, w, dash } => {
                let (a, b) = (t(*a), t(*b));
                let dash = dash.map(|s| format!(r#" stroke-dasharray="{s}""#)).unwrap_or_default();
                d.raw(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{ink}" stroke-width="{w}" stroke-linecap="round"{dash}/>"#, a.0, a.1, b.0, b.1));
            }
            Prim::Arc { c, r, a0, a1, w } => {
                let c = t(*c);
                if (a1 - a0 - 2.0 * PI).abs() < 1e-9 {
                    d.circle(c.0, c.1, *r, "none", ink, *w);
                } else {
                    let p0 = (c.0 + r * a0.cos(), c.1 + r * a0.sin());
                    let p1 = (c.0 + r * a1.cos(), c.1 + r * a1.sin());
                    let large = (a1 - a0 > PI) as u8;
                    d.path(&format!("M {:.2} {:.2} A {r:.2} {r:.2} 0 {large} 1 {:.2} {:.2}", p0.0, p0.1, p1.0, p1.1), "none", ink, *w);
                }
            }
            Prim::Dot(c, r) => {
                let c = t(*c);
                d.circle(c.0, c.1, *r, ink, "none", 0.0);
            }
            Prim::Square(v) => {
                d.polyline(&v.iter().map(|q| t(*q)).collect::<Vec<_>>(), ink, FOLD_W);
            }
        }
    }
    for (i, tb) in texts.iter().enumerate() {
        match maths.iter().find(|q| q.0 == i) {
            Some((_, draws, bx)) => {
                d.raw(&mathtext::svg(draws, tb.c.0 - tb.w / 2.0 + off.0, tb.c.1 - tb.h / 2.0 + bx.asc + off.1, ink, SERIF));
            }
            None => {
                d.raw(&text_svg(tb, off));
            }
        }
    }
    let shift = |q: Pt| (q.0 + off.0, q.1 + off.1);
    let texts: Vec<TextBox> = texts.into_iter().map(|mut tb| {
        tb.c = shift(tb.c);
        tb
    }).collect();
    let strokes = strokes.into_iter().map(|(a, b)| (shift(a), shift(b))).collect();
    let faces = faces.iter().map(|f| to_page(f, 1.0, (-off.0, -off.1))).collect();
    let leaders = leaders.into_iter().map(|(w, a, b)| (w, shift(a), shift(b))).collect();
    Rendered { svg: d.finish(), texts, strokes, faces, leaders, size: (w.ceil(), h.ceil()) }
}

/// Whether point q lies inside any face.
pub(super) fn inside_any(faces: &[Face], q: Pt) -> bool {
    faces.iter().any(|f| match &f.outline {
        Outline::Poly(v) => {
            let mut inside = false;
            let n = v.len();
            for i in 0..n {
                let (a, b) = (v[i], v[(i + 1) % n]);
                if (a.1 > q.1) != (b.1 > q.1) && q.0 < a.0 + (q.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
                    inside = !inside;
                }
            }
            inside
        }
        Outline::Circle(c, r) => len(sub(q, *c)) < *r,
        Outline::Sector { apex, r, a0, a1 } => {
            let d = sub(q, *apex);
            let mut a = d.1.atan2(d.0);
            while a < *a0 {
                a += 2.0 * PI;
            }
            len(d) < *r && a <= *a1
        }
    })
}

/// A spot for a w × h label in clear space outside the net, as near to `at` as there is one (rings 14 px apart, the
/// directions nearest `pref` first), with where its leader from `at` ends by the label.
fn far_out(at: Pt, pref: Pt, w: f64, h: f64, faces: &[Face], texts: &[TextBox], strokes: &[(Pt, Pt)]) -> Option<(Pt, Pt)> { far_out_avoiding(at, pref, w, h, faces, texts, strokes, &[], None) }

/// `far_out`, with a leader that keeps 3 px off `avoid` (the right-angle squares) and, for a line's label, leaves the
/// line at a clear angle (more than 25°) rather than running along it.
#[allow(clippy::too_many_arguments)]
fn far_out_avoiding(at: Pt, pref: Pt, w: f64, h: f64, faces: &[Face], texts: &[TextBox], strokes: &[(Pt, Pt)], avoid: &[(Pt, Pt)], line: Option<Pt>) -> Option<(Pt, Pt)> {
    let mut dirs: Vec<Pt> = (0..16).map(|k| {
        let a = k as f64 * PI / 8.0;
        (a.cos(), a.sin())
    }).collect();
    dirs.sort_by(|a, b| (b.0 * pref.0 + b.1 * pref.1).total_cmp(&(a.0 * pref.0 + a.1 * pref.1)));
    let corners: Vec<Pt> = faces.iter().flat_map(|f| match &f.outline { Outline::Poly(v) => v.clone(), _ => Vec::new() }).collect();
    for ring in 1..=16 {
        let r0 = 14.0 * ring as f64;
        for d in &dirs {
            let c = (at.0 + d.0 * (r0 + reach(*d, w, h)), at.1 + d.1 * (r0 + reach(*d, w, h)));
            let r = (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
            let pts = [(r.0, r.1), (r.2, r.1), (r.2, r.3), (r.0, r.3), c];
            if pts.iter().any(|q| inside_any(faces, *q)) {
                continue;
            }
            // well apart from other labels, so it isn't read as one of them
            // and off every line, so it doesn't read as an edge's
            if texts.iter().any(|t| overlaps(r, t.rect(), 10.0)) || strokes.iter().any(|(a, b)| seg_hits(*a, *b, r, 6.0)) {
                continue;
            }
            let end = (at.0.clamp(r.0 - 2.0, r.2 + 2.0), at.1.clamp(r.1 - 2.0, r.3 + 2.0));
            let near_avoid = avoid.iter().any(|(p, q)| segs_cross(at, end, *p, *q) || seg_near(at, end, *p, 3.0) || seg_near(at, end, *q, 3.0));
            // nor through a corner of the net, where it would read as a line from that corner
            let near_corner = corners.iter().any(|v| len(sub(*v, at)) > 6.0 && seg_near(at, end, *v, 4.0));
            let along_line = line.is_some_and(|ld| {
                let u = unit_v(sub(end, at));
                (u.0 * ld.1 - u.1 * ld.0).abs() < 0.42
            });
            if len(sub(end, at)) < 12.0 || texts.iter().any(|t| seg_hits(at, end, t.rect(), 1.0)) || near_avoid || near_corner || along_line {
                continue;
            }
            return Some((c, end));
        }
    }
    None
}

/// The apothem of a regular polygon face: from its middle to the middle of an edge whose line leaves room inside the
/// face for a w × h label beside it. The top edge first (as the reference draws it), then the flattest lines (a label
/// sits above a flat line); edges in `skip` are left out. Returns (middle, foot, the edge's far end for the square).
fn apothem_to(face: &Face, w: f64, h: f64, skip: &[usize], obstacles: &[(f64, f64, f64, f64)]) -> (Pt, Pt, Pt) {
    let c = face.centre();
    // an edge whose line would run through a label already down isn't a choice
    let line_clear = |i: usize| {
        let f = mid(face.edges[i].a, face.edges[i].b);
        !obstacles.iter().any(|r| seg_hits(c, f, *r, 3.0))
    };
    let free = |q: Pt| {
        let r = (q.0 - w / 2.0, q.1 - h / 2.0, q.0 + w / 2.0, q.1 + h / 2.0);
        !obstacles.iter().any(|o| overlaps(r, *o, 3.0))
    };
    let mut order: Vec<usize> = (0..face.edges.len()).filter(|i| !skip.contains(i) && line_clear(*i)).collect();
    if order.is_empty() {
        order = (0..face.edges.len()).filter(|i| !skip.contains(i)).collect();
    }
    let foot = |i: usize| mid(face.edges[i].a, face.edges[i].b);
    let top = *order.iter().min_by(|a, b| foot(**a).1.total_cmp(&foot(**b).1)).unwrap_or(&0);
    order.sort_by(|a, b| {
        let flat = |i: usize| { let d = unit_v(sub(foot(i), c)); d.1.abs() };
        (*a != top).cmp(&(*b != top)).then(flat(*a).total_cmp(&flat(*b)))
    });
    let fits = |i: usize| {
        let f = foot(i);
        let d = sub(f, c);
        let side: Pt = if d.1.abs() > d.0.abs() { (1.0, 0.0) } else { (0.0, -1.0) };
        [0.5, 0.4, 0.6, 0.3, 0.7].iter().any(|t| {
            [side, (-side.0, -side.1)].iter().any(|sd| {
                let at = along(c, f, *t);
                let off = 6.0 + reach(*sd, w, h);
                let q = (at.0 + sd.0 * off, at.1 + sd.1 * off);
                let r = (q.0 - w / 2.0 - 3.0, q.1 - h / 2.0 - 3.0, q.0 + w / 2.0 + 3.0, q.1 + h / 2.0 + 3.0);
                let inside = [(r.0, r.1), (r.2, r.1), (r.2, r.3), (r.0, r.3)].iter().all(|p| inside_any(std::slice::from_ref(face), *p));
                inside && free(q) && !seg_hits(c, f, r, 0.0) && !(c.0 > r.0 && c.0 < r.2 && c.1 > r.1 && c.1 < r.3)
            })
        })
    };
    let face1 = std::slice::from_ref(face);
    let inside_box = |q: Pt, pad: f64| {
        let r = (q.0 - w / 2.0 - pad, q.1 - h / 2.0 - pad, q.0 + w / 2.0 + pad, q.1 + h / 2.0 + pad);
        [(r.0, r.1), (r.2, r.1), (r.2, r.3), (r.0, r.3)].iter().all(|p| inside_any(face1, *p))
    };
    let dot_in = |q: Pt| (c.0 - q.0).abs() < w / 2.0 + 4.0 && (c.1 - q.1).abs() < h / 2.0 + 4.0;
    // the right-angle square's corner region at the foot
    let near_foot = |q: Pt, f: Pt| (f.0 - q.0).abs() < w / 2.0 + 14.0 && (f.1 - q.1).abs() < h / 2.0 + 14.0;
    // anywhere clear inside the face, off the line, its dot and its square
    let anywhere = |i: usize| {
        let f = foot(i);
        (0..=30).any(|ring| {
            let rr = ring as f64 * 4.0;
            (0..24).any(|k| {
                let a = k as f64 * PI / 12.0;
                let q = (c.0 + rr * a.cos(), c.1 + rr * a.sin());
                let r = (q.0 - w / 2.0 - 2.0, q.1 - h / 2.0 - 2.0, q.0 + w / 2.0 + 2.0, q.1 + h / 2.0 + 2.0);
                inside_box(q, 3.0) && free(q) && !seg_hits(c, f, r, 0.0) && !dot_in(q) && !near_foot(q, f)
            })
        })
    };
    // on the line, a sixth of it showing at each end, clear of the dot and the square
    let on_line = |i: usize| {
        let f = foot(i);
        (15..=85).any(|k| {
            let q = along(c, f, k as f64 / 100.0);
            let r = (q.0 - w / 2.0 - 3.0, q.1 - h / 2.0 - 3.0, q.0 + w / 2.0 + 3.0, q.1 + h / 2.0 + 3.0);
            clip_t(c, f, r).is_some_and(|(t0, t1)| t0 >= 0.15 && t1 <= 0.85) && inside_box(q, 2.0) && free(q) && !dot_in(q) && !near_foot(q, f)
        })
    };
    let pick = order.iter().copied().find(|&i| fits(i)).or_else(|| order.iter().copied().find(|&i| anywhere(i))).or_else(|| order.iter().copied().find(|&i| on_line(i))).unwrap_or(top);
    (c, foot(pick), face.edges[pick].b)
}

/// The page's extent so far, grown by a margin a label may use.
fn size_of_page(faces: &[Face], texts: &[TextBox]) -> (f64, f64, f64, f64) {
    let b = extent(faces, texts);
    (b.0 - 60.0, b.1 - 60.0, b.2 + 60.0, b.3 + 60.0)
}

/// Any spot for a w × h label clear of every text and line within `area`, nearest `at` first, with its leader's end.
fn clear_spot(at: Pt, w: f64, h: f64, texts: &[TextBox], strokes: &[(Pt, Pt)], area: (f64, f64, f64, f64)) -> Option<(Pt, Pt)> {
    for ring in 1..=60 {
        let r0 = 6.0 * ring as f64;
        for k in 0..32 {
            let a = k as f64 * PI / 16.0;
            let q = (at.0 + r0 * a.cos(), at.1 + r0 * a.sin());
            let r = (q.0 - w / 2.0, q.1 - h / 2.0, q.0 + w / 2.0, q.1 + h / 2.0);
            if r.0 < area.0 || r.1 < area.1 || r.2 > area.2 || r.3 > area.3 {
                continue;
            }
            if texts.iter().any(|t| overlaps(r, t.rect(), 2.0)) || strokes.iter().any(|(a, b)| seg_hits(*a, *b, r, 1.5)) {
                continue;
            }
            let end = (at.0.clamp(r.0 - 2.0, r.2 + 2.0), at.1.clamp(r.1 - 2.0, r.3 + 2.0));
            if texts.iter().any(|t| seg_hits(at, end, t.rect(), 1.0)) {
                continue;
            }
            return Some((q, end));
        }
    }
    None
}

/// Whether point p lies within `d` of segment a-b.
fn seg_near(a: Pt, b: Pt, p: Pt, d: f64) -> bool {
    let ab = sub(b, a);
    let t = (((p.0 - a.0) * ab.0 + (p.1 - a.1) * ab.1) / (ab.0 * ab.0 + ab.1 * ab.1).max(1e-12)).clamp(0.0, 1.0);
    len(sub(p, along(a, b, t))) < d
}

/// Whether segments a-b and c-d cross.
fn segs_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    (d1 * d2 < 0.0) && (d3 * d4 < 0.0)
}

/// Where segment a-b enters and leaves box r, as fractions along it (Liang–Barsky).
fn clip_t(a: Pt, b: Pt, r: (f64, f64, f64, f64)) -> Option<(f64, f64)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (pp, q) in [(-dx, a.0 - r.0), (dx, r.2 - a.0), (-dy, a.1 - r.1), (dy, r.3 - a.1)] {
        if pp.abs() < 1e-12 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / pp;
            if pp < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((t0, t1))
}

fn sector_path(apex: Pt, r: f64, a0: f64, a1: f64) -> String {
    let p0 = (apex.0 + r * a0.cos(), apex.1 + r * a0.sin());
    let p1 = (apex.0 + r * a1.cos(), apex.1 + r * a1.sin());
    let large = (a1 - a0 > PI) as u8;
    format!("M {:.2} {:.2} L {:.2} {:.2} A {r:.2} {r:.2} 0 {large} 1 {:.2} {:.2} Z", apex.0, apex.1, p0.0, p0.1, p1.0, p1.1)
}

/// Everything drawn: the faces and the texts.
fn extent(faces: &[Face], texts: &[TextBox]) -> (f64, f64, f64, f64) {
    let mut b = geom::bounds(faces);
    for t in texts {
        let r = t.rect();
        b = (b.0.min(r.0), b.1.min(r.1), b.2.max(r.2), b.3.max(r.3));
    }
    b
}

/// The accessible title and description.
fn describe(m: &Model) -> (String, String) {
    let what = match (m.shape, m.base) {
        (Shape::Cube, _) => format!("a cube (net {})", m.net),
        (Shape::Prism, Base::Rect) => "a rectangular prism".into(),
        (Shape::Prism, Base::RightTri) => "a prism with a right-triangle base".into(),
        (Shape::Prism, Base::Isos) => "a prism with an isosceles-triangle base".into(),
        (Shape::Prism, Base::Regular(n)) => format!("a prism with a regular {n}-sided base"),
        (Shape::Pyramid, Base::Regular(n)) => format!("a pyramid with a regular {n}-sided base"),
        (Shape::Pyramid, _) => "a rectangular pyramid".into(),
        (Shape::Cylinder, _) => "a cylinder".into(),
        (Shape::Cone, _) => "a cone".into(),
    };
    ("Nets".into(), format!("The net of {what}, with its fold lines"))
}
