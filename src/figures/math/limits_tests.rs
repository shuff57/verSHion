//! Tests of the limits-and-discontinuities figure (src/figures/math/limits.rs). Every limit, value and kind checked here was
//! worked by hand (the working is in the comments), not read back from the code.
use super::limits::*;
use crate::figures::{registry, start_params, FigureDef, Params};
use serde_json::{json, Value as J};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
fn with(kv: J) -> Params { crate::figures::params_from_json(&def(), &kv) }
fn svg(p: &Params) -> String { def().svg(p) }

#[test]
#[ignore]
fn dump_pngs() {
    let dir = std::env::var("LIM_OUT").unwrap_or_else(|_| "/tmp/lim".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut cases: Vec<(String, Params)> = vec![("open".into(), start_params(&def()))];
    for (k, e) in crate::figures::examples_for(ID).iter().enumerate() {
        let mut p = start_params(&def());
        for (kk, v) in &e.params { p.0.insert(kk.to_string(), v.clone()); }
        cases.push((format!("ex{k}"), p));
    }
    if let Ok(extra) = std::env::var("LIM_CASE") { cases = vec![("case".into(), with(serde_json::from_str(&extra).unwrap()))]; }
    for (name, p) in cases {
        let s = svg(&p);
        std::fs::write(format!("{dir}/{name}.svg"), &s).unwrap();
        std::fs::write(format!("{dir}/{name}.png"), crate::figures::to_png(&s, 1.0).unwrap()).unwrap();
        println!("{name}: {:?}", warnings(&p));
    }
}

/// The text a reader sees, tags dropped and every tspan joined, the true minus read as a hyphen.
fn words(svg: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    let mut tag = String::new();
    for c in svg.chars() {
        match c {
            '<' => { inside = true; tag.clear(); }
            '>' => { inside = false; if tag.starts_with("/text") || tag.starts_with("text") { out.push(' '); } }
            _ if inside => tag.push(c),
            _ => out.push(if c == '\u{2212}' { '-' } else { c }),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn m(kv: J) -> Model { super::limits::model_of(&with(kv)) }
fn val(l: &Lim) -> String { match l { Lim::Val(_, t) => t.clone(), other => format!("{other:?}") } }
fn kinds(md: &Model) -> Vec<String> { md.discontinuities().iter().map(|d| format!("{} {}", d.text, d.kind)).collect() }
fn close(l: &Lim, v: f64) -> bool { matches!(l, Lim::Val(u, _) if (u - v).abs() < 1e-9) }

// ───────────────────────── hand-worked values ─────────────────────────

/// The opening state: y = 4 - (x - 1)^2/3 for x < 1 (a hole at -2), y = 2 + 3/(x - 4) for x >= 1, the point (-2, 3).
#[test]
fn the_opening_state_worked_by_hand() {
    let md = super::limits::model_of(&start_params(&def()));
    // at -2: 4 - (-3)^2/3 = 4 - 3 = 1 from both sides; the hole leaves the formula's value out, the point gives f(-2) = 3
    assert_eq!(val(&md.side(-2.0, -1.0)), "1");
    assert_eq!(val(&md.side(-2.0, 1.0)), "1");
    assert_eq!(val(&md.two(-2.0)), "1");
    assert_eq!(val(&md.value(-2.0)), "3");
    // at 1: from the left 4 - 0 = 4 (open end); from the right 2 + 3/(1 - 4) = 2 - 1 = 1 (closed end, so f(1) = 1)
    assert_eq!(val(&md.side(1.0, -1.0)), "4");
    assert_eq!(val(&md.side(1.0, 1.0)), "1");
    assert_eq!(md.two(1.0), Lim::Dne);
    assert_eq!(val(&md.value(1.0)), "1");
    // at 4: 3/(x - 4) is a large negative number just left of 4 and a large positive one just right: -∞ and ∞; 4 is not in
    // the domain
    assert_eq!(md.side(4.0, -1.0), Lim::NInf);
    assert_eq!(md.side(4.0, 1.0), Lim::PInf);
    assert_eq!(md.two(4.0), Lim::Dne);
    assert_eq!(md.value(4.0), Lim::Undef);
    // at ∞: 3/(x - 4) → 0, so 2; at -∞: -(x - 1)^2/3 → -∞
    assert_eq!(val(&md.at_inf(1.0).unwrap()), "2");
    assert_eq!(md.at_inf(-1.0).unwrap(), Lim::NInf);
    // the discontinuities in the window -6..10: a hole at -2, a jump at 1, an infinite one at 4; nothing else
    let ds: Vec<(String, &str)> = md.discontinuities().iter().map(|d| (d.text.clone(), d.kind)).collect();
    assert_eq!(ds, [("-2".to_string(), "removable"), ("1".to_string(), "jump"), ("4".to_string(), "infinite")]);
    assert_eq!(md.verticals().iter().map(|v| v.1.clone()).collect::<Vec<_>>(), ["4"]);
    assert_eq!(md.horizontals().iter().map(|v| v.1.clone()).collect::<Vec<_>>(), ["2"]);
    // and the key under the figure says so: lim x→1- f(x) = 4, lim x→1+ f(x) = 1, the limit does not exist, f(1) = 1
    let w = words(&svg(&start_params(&def())));
    assert!(w.contains("lim x→1- f(x) = 4") && w.contains("lim x→1+ f(x) = 1") && w.contains("lim x→1 f(x) = does not exist") && w.contains("f(1) = 1"), "{w}");
}

/// Holes from 0/0, written exactly.
#[test]
fn holes_are_found_and_written_exactly() {
    // (x^2 - 9)/(x + 3) = x - 3 away from -3: the limit at -3 is -6, f(-3) is undefined
    let md = m(json!({"pieces": "y = (x^2 - 9)/(x + 3)", "dots": "", "ask_at": "-3"}));
    assert_eq!(val(&md.two(-3.0)), "-6");
    assert_eq!(md.value(-3.0), Lim::Undef);
    assert_eq!(kinds(&md), ["-3 removable"]);
    // (x - 1)/(x^2 - 1) = 1/(x + 1): 1/2 at 1 (a hole), and a pole at -1
    let md = m(json!({"pieces": "y = (x - 1)/(x^2 - 1)", "dots": "", "xmin": -4.0, "xmax": 4.0, "ymin": -4.0, "ymax": 4.0}));
    assert_eq!(val(&md.two(1.0)), "1/2");
    assert_eq!(kinds(&md), ["-1 infinite", "1 removable"]);
    // (4x^2 - 1)/(2x^2 + 3x - 2) = (2x - 1)(2x + 1)/((2x - 1)(x + 2)) = (2x + 1)/(x + 2): at 1/2, 2/(5/2) = 4/5; a pole at -2;
    // the limit at ±∞ is 4/2 = 2
    let md = m(json!({"pieces": "y = (4x^2 - 1)/(2x^2 + 3x - 2)", "dots": "", "ask_at": "1/2"}));
    assert_eq!(val(&md.two(0.5)), "4/5");
    assert!(md.verticals().iter().any(|v| v.1 == "-2"));
    assert_eq!(val(&md.at_inf(1.0).unwrap()), "2");
    assert_eq!(val(&md.at_inf(-1.0).unwrap()), "2");
    // (x^3 - 8)/(x - 2) = x^2 + 2x + 4: 12 at 2
    let md = m(json!({"pieces": "y = (x^3 - 8)/(x - 2)", "dots": "", "ymax": 14.0}));
    assert_eq!(val(&md.two(2.0)), "12");
    // sin x / x at 0 is 1 (a hole); √x at 2 is √2; arctan x at ∞ is π/2 and at -∞ -π/2
    assert_eq!(val(&m(json!({"pieces": "y = sin(x)/x", "dots": ""})).two(0.0)), "1");
    assert_eq!(val(&m(json!({"pieces": "y = sqrt(x)", "dots": ""})).two(2.0)), "\u{221a}2");
    let md = m(json!({"pieces": "y = arctan(x)", "dots": ""}));
    assert_eq!(val(&md.at_inf(1.0).unwrap()), "\u{3c0}/2");
    assert_eq!(val(&md.at_inf(-1.0).unwrap()), "-\u{3c0}/2");
    // (1 - cos x)/x at 0: 1 - cos x ≈ x^2/2, so x/2 → 0
    assert_eq!(val(&m(json!({"pieces": "y = (1 - cos(x))/x", "dots": ""})).two(0.0)), "0");
}

#[test]
fn infinite_limits_and_poles() {
    // 1/x^2: an even pole, ∞ from both sides, so the limit is ∞ (it is written ∞, or does not exist when chosen)
    let md = m(json!({"pieces": "y = 1/x^2", "dots": ""}));
    assert_eq!((md.side(0.0, -1.0), md.side(0.0, 1.0), md.two(0.0)), (Lim::PInf, Lim::PInf, Lim::PInf));
    assert_eq!(md.verticals().iter().map(|v| v.1.clone()).collect::<Vec<_>>(), ["0"]);
    // an even pole between the samples, off the grid's quarter steps: 1/(x - 0.3)^2
    let md = m(json!({"pieces": "y = 1/(x - 0.3)^2", "dots": ""}));
    assert!(md.verticals().iter().any(|v| (v.0 - 0.3).abs() < 1e-9), "{:?}", md.verticals());
    // 1/(x - 2): -∞ from the left, ∞ from the right
    let md = m(json!({"pieces": "y = 1/(x - 2)", "dots": ""}));
    assert_eq!((md.side(2.0, -1.0), md.side(2.0, 1.0), md.two(2.0)), (Lim::NInf, Lim::PInf, Lim::Dne));
    // ln x: -∞ as x → 0 from the right, nothing to the left (a domain edge, not listed as a discontinuity), and the vertical
    // asymptote x = 0
    let md = m(json!({"pieces": "y = ln(x)", "dots": ""}));
    assert_eq!((md.side(0.0, -1.0), md.side(0.0, 1.0)), (Lim::Undef, Lim::NInf));
    assert!(md.discontinuities().is_empty());
    assert_eq!(md.verticals().iter().map(|v| v.1.clone()).collect::<Vec<_>>(), ["0"]);
    assert_eq!(md.at_inf(1.0), Some(Lim::PInf));
    assert_eq!(md.at_inf(-1.0), None);
    // tan x in -5..5: poles at ±π/2 and ±3π/2, written exactly
    let md = m(json!({"pieces": "y = tan(x)", "dots": "", "xmin": -5.0, "xmax": 5.0}));
    let v: Vec<String> = md.verticals().iter().map(|v| v.1.clone()).collect();
    assert_eq!(v, ["-3\u{3c0}/2", "-\u{3c0}/2", "\u{3c0}/2", "3\u{3c0}/2"]);
    // the key writes ∞ and -∞, or does not exist when chosen
    let p = with(json!({"ask_at": "4"}));
    let w = words(&svg(&p));
    assert!(w.contains("lim x→4- f(x) = -∞") && w.contains("lim x→4+ f(x) = ∞") && w.contains("f(4) = undefined"), "{w}");
    let mut q = p.clone();
    q.set_s("inf_answer", "does not exist");
    let w = words(&svg(&q));
    assert!(w.contains("lim x→4- f(x) = does not exist") && w.contains("lim x→4+ f(x) = does not exist") && !w.contains("= ∞") && !w.contains("= -∞"), "{w}");
    q.set_s("dne_word", "DNE");
    assert!(words(&svg(&q)).contains("lim x→4 f(x) = DNE"));
}

#[test]
fn limits_at_infinity_worked_by_hand() {
    let at = |e: &str| { let md = m(json!({"pieces": format!("y = {e}"), "dots": ""})); (md.at_inf(1.0), md.at_inf(-1.0)) };
    // (2x - 1)/(x + 1) → 2/1 both ways
    let (a, b) = at("(2x - 1)/(x + 1)");
    assert_eq!((val(&a.unwrap()), val(&b.unwrap())), ("2".into(), "2".into()));
    // 5x/(x^2 + 1): the denominator wins, 0 both ways
    let (a, b) = at("5x/(x^2 + 1)");
    assert_eq!((val(&a.unwrap()), val(&b.unwrap())), ("0".into(), "0".into()));
    // (x^2 + 1)/(x - 1) ≈ x + 1: ∞ and -∞
    assert_eq!(at("(x^2 + 1)/(x - 1)"), (Some(Lim::PInf), Some(Lim::NInf)));
    // e^(-x): 0 at ∞, ∞ at -∞
    let (a, b) = at("e^(-x)");
    assert!(close(&a.unwrap(), 0.0));
    assert_eq!(b, Some(Lim::PInf));
    // sin x keeps waving: does not exist; sin x / x shrinks to 0
    assert_eq!(at("sin(x)"), (Some(Lim::Dne), Some(Lim::Dne)));
    assert_eq!(val(&at("sin(x)/x").0.unwrap()), "0");
    // 3 - 2/x^2 → 3; (1 + 1/x)^x → e (≈ 2.718)
    assert_eq!(val(&at("3 - 2/x^2").0.unwrap()), "3");
    assert_eq!(val(&at("(1 + 1/x)^x").0.unwrap()), "\u{2248} 2.718");
    // a piece that stops (x <= 5) is not asked about at ∞; the panel says why
    let p = with(json!({"pieces": "y = x, x <= 5", "dots": "", "ask_inf": "Both"}));
    assert!(warnings(&p).iter().any(|n| n.starts_with("No piece runs on to x → ∞")), "{:?}", warnings(&p));
    assert!(!words(&svg(&p)).contains("x→∞"));
    assert!(words(&svg(&p)).contains("x→-∞"));
}

#[test]
fn jumps_points_and_values() {
    // |x|/x: -1 from the left, 1 from the right, no value at 0: a jump
    let md = m(json!({"pieces": "y = |x|/x", "dots": ""}));
    assert_eq!((val(&md.side(0.0, -1.0)), val(&md.side(0.0, 1.0)), md.value(0.0)), ("-1".into(), "1".into(), Lim::Undef));
    assert_eq!(md.discontinuities().iter().map(|d| d.kind).collect::<Vec<_>>(), ["jump"]);
    // sin(1/x) near 0 neither settles nor grows: the limits do not exist, an oscillating discontinuity
    let md = m(json!({"pieces": "y = sin(1/x)", "dots": ""}));
    assert_eq!((md.side(0.0, -1.0), md.side(0.0, 1.0)), (Lim::Dne, Lim::Dne));
    // x^2 for x < 2 (4), 6 - x for 2 <= x < 4 (4, then 2 at 4 open), 1 for x >= 4: continuous at 2, a jump at 4
    let md = m(json!({"pieces": "y = x^2, x < 2\ny = 6 - x, 2 <= x < 4\ny = 1, x >= 4", "dots": ""}));
    assert_eq!((val(&md.side(2.0, -1.0)), val(&md.side(2.0, 1.0)), val(&md.value(2.0))), ("4".into(), "4".into(), "4".into()));
    assert_eq!((val(&md.side(4.0, -1.0)), val(&md.side(4.0, 1.0)), val(&md.value(4.0))), ("2".into(), "1".into(), "1".into()));
    assert_eq!(kinds(&md), ["4 jump"]);
    // typed in decimals, written in decimals: 0.5x + 2 at 1 is 2.5
    assert_eq!(val(&m(json!({"pieces": "y = 0.5x + 2", "dots": ""})).two(1.0)), "2.5");
    // points: the values at the points are exactly as typed; the curve between stays between them (no bulge)
    let md = m(json!({"pieces": "(-5, 1) (-3, 7/2) (-1, 2) (1, 4) open\nclosed (1, 1) (3, 0) (5, 2)", "dots": ""}));
    assert_eq!((val(&md.side(1.0, -1.0)), val(&md.side(1.0, 1.0)), val(&md.value(1.0))), ("4".into(), "1".into(), "1".into()));
    assert_eq!(val(&md.two(-3.0)), "7/2");
    for k in 0..=100 {
        let x = -3.0 + 2.0 * k as f64 / 100.0;
        let y = md.pieces[0].g(x).unwrap();
        assert!((2.0 - 1e-9..=3.5 + 1e-9).contains(&y), "the curve bulges past its points at {x}: {y}");
    }
    // typed a as a fraction: written x→1/2
    let w = words(&svg(&with(json!({"ask_at": "1/2"}))));
    assert!(w.contains("lim x→1/2- f(x)") && w.contains("f(1/2) ="), "{w}");
}

#[test]
fn what_cannot_be_drawn_is_said() {
    let notes = |kv: J| warnings(&with(kv));
    // not a function of x, an unknown function, an unreadable domain
    assert!(notes(json!({"pieces": "x^2 + y^2 = 4"})).iter().any(|n| n.starts_with("Piece 1 is not a function of x")));
    assert!(notes(json!({"pieces": "y = foo(x)"})).iter().any(|n| n.starts_with("Piece 1: the formula cannot be read")));
    assert!(notes(json!({"pieces": "y = x, x < abc"})).iter().any(|n| n.contains("is not a number")));
    assert!(notes(json!({"pieces": "y = x, 3 < x < 1"})).iter().any(|n| n.contains("has no x in its domain")));
    // two pieces over the same x, and two values at one x
    assert!(notes(json!({"pieces": "y = x, x < 2\ny = 3, x > 1"})).iter().any(|n| n.starts_with("Pieces 1 and 2 both cover x from 1 to 2")));
    let p = with(json!({"pieces": "y = x, x <= 1\ny = 3, x >= 1", "dots": "", "ask_at": "1"}));
    assert!(warnings(&p).iter().any(|n| n.starts_with("f has two values at x = 1")), "{:?}", warnings(&p));
    assert!(words(&svg(&p)).contains("f(1) = —"));
    // a closed end where the formula has no value: (x^2 - 4)/(x - 2) at 2
    assert!(notes(json!({"pieces": "y = (x^2 - 4)/(x - 2), x <= 2"})).iter().any(|n| n.contains("includes x = 2 but its formula has no value there")));
    // a fractional power of a negative number is not read by the app: said, not drawn as something false
    assert!(notes(json!({"pieces": "y = x^(1/3)"})).iter().any(|n| n.contains("fractional power of a negative number")));
    // a point, a circle, a value of a or a discontinuity outside the window
    assert!(notes(json!({"dots": "(20, 3)"})).iter().any(|n| n.contains("outside the window")));
    assert!(notes(json!({"ask_at": "40"})).iter().any(|n| n.starts_with("x = 40 is outside the window")));
    assert!(notes(json!({"pieces": "y = 1/(x - 12)", "dots": ""})).iter().any(|n| n.contains("infinite discontinuity at x = 12, outside the window")));
    assert!(notes(json!({"ask_at": "1, abc"})).iter().any(|n| n.contains("\u{201c}abc\u{201d}")));
    assert!(notes(json!({"dots": "(1, 2), (3"})).iter().any(|n| n.contains("not closed")));
}

#[test]
#[ignore]
fn debug_labels() {
    let p = with(serde_json::from_str(&std::env::var("LIM_CASE").unwrap()).unwrap());
    let _ = svg(&p);
    for l in last_drawn(&p).labels { println!("{l:?}"); }
    println!("{:?}", last_drawn(&p).frame);
}

// ───────────────────────── labels ─────────────────────────

type Bx = [f64; 4];
type Pt = (f64, f64);
struct Meter { opt: resvg::usvg::Options<'static>, memo: std::collections::HashMap<String, Option<Bx>> }
impl Meter {
    fn new() -> Meter { let mut opt = resvg::usvg::Options::default(); opt.fontdb = crate::figures::fontdb(); Meter { opt, memo: Default::default() } }
    /// The ink box of one `<text>` element as drawn.
    fn ink(&mut self, el: &str) -> Option<Bx> {
        if let Some(v) = self.memo.get(el) { return *v; }
        fn find(g: &resvg::usvg::Group) -> Option<resvg::usvg::Rect> {
            for n in g.children() {
                match n { resvg::usvg::Node::Text(t) => return Some(t.flattened().abs_bounding_box()), resvg::usvg::Node::Group(g2) => if let Some(r) = find(g2) { return Some(r); }, _ => {} }
            }
            None
        }
        let s = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="4000" height="4000">{el}</svg>"#);
        let out = resvg::usvg::Tree::from_str(&s, &self.opt).ok().and_then(|t| find(t.root())).filter(|r| r.width() > 0.0).map(|r| [r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64]);
        self.memo.insert(el.to_string(), out);
        out
    }
}
fn attr(tag: &str, n: &str) -> Option<f64> { let pat = format!(" {n}=\""); let j = tag.find(&pat)? + pat.len(); tag[j..].split('"').next()?.parse().ok() }
fn group<'a>(svg: &'a str, name: &str) -> &'a str {
    let pat = format!(r#"data-lim="{name}">"#);
    let Some(i) = svg.find(&pat) else { return "" };
    let rest = &svg[i..];
    // up to the matching </g>
    let mut depth = 0i32;
    let mut k = 0;
    while k < rest.len() {
        if rest[k..].starts_with("<g") { depth += 1; } else if rest[k..].starts_with("</g>") { if depth == 0 { return &rest[..k]; } depth -= 1; }
        k += rest[k..].chars().next().unwrap().len_utf8();
    }
    rest
}
/// (group, element) for every text drawn in ink (the white halo copies are left out).
fn texts(svg: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    for name in ["texts", "labels", "block"] {
        let g = group(svg, name);
        let mut rest = g;
        while let Some(i) = rest.find("<text") {
            let end = rest[i..].find("</text>").unwrap() + i + 7;
            let el = &rest[i..end];
            if !el.contains(r##"fill="#fff""##) { out.push((name.to_string(), el.to_string())); }
            rest = &rest[end..];
        }
    }
    out
}
/// The plot's strokes (curves, asymptotes, guides, band edges, leaders, the axes), arrowheads and point marks.
fn scene(svg: &str) -> (Vec<(Pt, Pt)>, Vec<(Pt, f64)>, Vec<(Pt, Pt)>) {
    let mut segs = vec![];
    let mut dots = vec![];
    let mut leaders = vec![];
    let mut curves = group(svg, "curves").to_string();
    while let Some(a) = curves.find("<mask") { let b = curves[a..].find("</mask>").unwrap(); curves.replace_range(a..a + b + 7, ""); }
    for part in curves.split('<') {
        if part.starts_with("polyline ") || part.starts_with("polygon ") {
            let pts: Vec<Pt> = part.split("points=\"").nth(1).unwrap().split('"').next().unwrap().split(' ').filter_map(|q| { let (a, b) = q.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
            for w in pts.windows(2) { segs.push((w[0], w[1])); }
            if part.starts_with("polygon ") && pts.len() > 2 { segs.push((pts[pts.len() - 1], pts[0])); }
        }
    }
    for name in ["lines", "band", "labels"] {
        for part in group(svg, name).split('<').filter(|p| p.starts_with("line ")) {
            let s = ((attr(part, "x1").unwrap(), attr(part, "y1").unwrap()), (attr(part, "x2").unwrap(), attr(part, "y2").unwrap()));
            if name == "labels" { leaders.push(s); } else { segs.push(s); }
        }
    }
    for part in group(svg, "marks").split('<') {
        if part.starts_with("circle ") { dots.push(((attr(part, "cx").unwrap(), attr(part, "cy").unwrap()), attr(part, "r").unwrap())); }
        if part.starts_with("polygon ") {
            let pts: Vec<f64> = part.split("points=\"").nth(1).unwrap().split('"').next().unwrap().split([' ', ',']).filter_map(|t| t.parse().ok()).collect();
            dots.push(((pts[0], pts[3]), 6.0));
        }
    }
    // the axes: the frame's lines 2 px or heavier, before the first group of this figure
    let frame = &svg[..svg.find(r#"data-lim="band""#).unwrap_or(0)];
    for part in frame.split('<').filter(|p| p.starts_with("line ") && attr(p, "stroke-width").is_some_and(|w| w >= 2.0)) {
        segs.push(((attr(part, "x1").unwrap(), attr(part, "y1").unwrap()), (attr(part, "x2").unwrap(), attr(part, "y2").unwrap())));
    }
    (segs, dots, leaders)
}
fn seg_box(a: Pt, b: Pt, r: Bx) -> bool {
    let n = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / 0.5).ceil().clamp(1.0, 4000.0) as usize;
    (0..=n).any(|i| { let t = i as f64 / n as f64; let (x, y) = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)); x > r[0] && x < r[2] && y > r[1] && y < r[3] })
}
fn pt_box(q: Pt, r: Bx) -> f64 { let dx = (r[0] - q.0).max(q.0 - r[2]).max(0.0); let dy = (r[1] - q.1).max(q.1 - r[3]).max(0.0); dx.hypot(dy) }
fn seg_dist(a: Pt, b: Pt, r: Bx) -> f64 {
    if seg_box(a, b, r) { return 0.0; }
    let n = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / 1.0).ceil().clamp(1.0, 4000.0) as usize;
    (0..=n).map(|i| { let t = i as f64 / n as f64; pt_box((a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)), r) }).fold(f64::MAX, f64::min)
}

/// What one figure's check counted.
#[derive(Default, Debug, Clone)]
struct Res { beside: usize, leader: usize, under: usize, hidden_numbers: usize, dropped_numbers: usize, labels_on_plot: usize, problems: Vec<String> }

/// Checks one figure as drawn: every text on the page and on no other text; every label on the plot clear of every stroke
/// and point mark, nearer its own thing than any other mark, and on a leader only when it has one.
fn check(m: &mut Meter, s: &str, p: &Params) -> Res {
    let dr = last_drawn(p);
    let (w, h) = crate::figures::svg_size(s);
    let (segs, dots, leaders) = scene(s);
    let all: Vec<(String, String, Bx)> = texts(s).into_iter().filter_map(|(g, el)| m.ink(&el).map(|b| (g, el, b))).collect();
    let mut r = Res { hidden_numbers: dr.hidden_numbers, dropped_numbers: dr.dropped_numbers, ..Default::default() };
    for (i, (g, el, b)) in all.iter().enumerate() {
        if !(b[0] >= 0.0 && b[1] >= 0.0 && b[2] <= w && b[3] <= h) { r.problems.push(format!("off the page: {g} {b:?}")); }
        for (j, (g2, _, b2)) in all.iter().enumerate() {
            if j <= i { continue; }
            if b[0] + 0.5 < b2[2] && b2[0] + 0.5 < b[2] && b[1] + 0.5 < b2[3] && b2[1] + 0.5 < b[3] { r.problems.push(format!("text on text: {g} {b:?} and {g2} {b2:?}: {}", &el[el.len().saturating_sub(60)..])); }
        }
        if g != "labels" { continue; }
        r.labels_on_plot += 1;
        for (a, c) in &segs { if seg_box(*a, *c, *b) { r.problems.push(format!("label on a line: {b:?} {a:?}-{c:?}")); break; } }
        for (q, rr) in &dots { if pt_box(*q, *b) < *rr + 1.0 { r.problems.push(format!("label on a point: {b:?} {q:?}")); } }
    }
    // each label by its own thing: its ink box nearer its own thing than any other point mark
    let lab_boxes: Vec<Bx> = all.iter().filter(|t| t.0 == "labels").map(|t| t.2).collect();
    let placed: Vec<&(char, String, Spot, Vec<(Pt, Pt)>)> = dr.labels.iter().filter(|l| l.2 != Spot::Off).collect();
    assert_eq!(placed.len(), lab_boxes.len(), "labels drawn and labels recorded differ");
    for (l, b) in placed.iter().zip(&lab_boxes) {
        match &l.2 { Spot::Beside(_) => r.beside += 1, Spot::Leader(_, s0, e0) => {
            r.leader += 1;
            // a leader runs from its own thing to its own text and crosses no stroke
            if pt_box(*e0, grow(*b, 5.0)) > 0.0 { r.problems.push(format!("leader does not reach its label {}", l.1)); }
            for (a, c) in &segs { if seg_cross(*s0, *e0, *a, *c) { r.problems.push(format!("leader of {} crosses a stroke", l.1)); } }
        }, Spot::Off => {} }
        let own = l.3.iter().map(|(a, c)| seg_dist(*a, *c, *b)).fold(f64::MAX, f64::min);
        let own_pts: Vec<Pt> = l.3.iter().flat_map(|(a, c)| [*a, *c]).collect();
        if matches!(l.2, Spot::Beside(_)) {
            for (q, _) in &dots {
                if own_pts.iter().any(|o| (o.0 - q.0).abs() < 0.6 && (o.1 - q.1).abs() < 0.6) { continue; }
                if pt_box(*q, *b) < own { r.problems.push(format!("{} is nearer a point {q:?} than its own thing ({own:.1})", l.1)); }
            }
        }
    }
    for (s0, e0) in &leaders { for (s1, e1) in &leaders { if (s0, e0) != (s1, e1) && seg_cross(*s0, *e0, *s1, *e1) { r.problems.push("two leaders cross".into()); } } }
    r.under = dr.labels.iter().filter(|l| l.2 == Spot::Off).count();
    r
}
fn grow(r: Bx, t: f64) -> Bx { [r[0] - t, r[1] - t, r[2] + t, r[3] + t] }
fn seg_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

/// The written classroom list: the functions, windows and values of a a limits unit uses (rational functions with holes and
/// poles, piecewise functions with jumps and steps, graphs drawn from points, limits at infinity).
fn classroom() -> Vec<(&'static str, J)> {
    let f = |pieces: &str, ask: &str, w: [f64; 4]| json!({"pieces": pieces, "dots": "", "ask_at": ask, "xmin": w[0], "xmax": w[1], "ymin": w[2], "ymax": w[3]});
    let fd = |pieces: &str, dots: &str, ask: &str, w: [f64; 4]| json!({"pieces": pieces, "dots": dots, "ask_at": ask, "xmin": w[0], "xmax": w[1], "ymin": w[2], "ymax": w[3]});
    let std = [-6.0, 6.0, -6.0, 6.0];
    vec![
        ("hole (x^2-4)/(x-2)", f("y = (x^2 - 4)/(x - 2)", "2", [-4.0, 6.0, -2.0, 7.0])),
        ("hole (x^2-1)/(x-1)", f("y = (x^2 - 1)/(x - 1)", "1", std)),
        ("hole (x^2-9)/(x+3)", f("y = (x^2 - 9)/(x + 3)", "-3", [-8.0, 4.0, -9.0, 3.0])),
        ("hole (x^2+x-6)/(x-2)", f("y = (x^2 + x - 6)/(x - 2)", "2", [-6.0, 6.0, -4.0, 8.0])),
        ("hole (x^3-8)/(x-2)", f("y = (x^3 - 8)/(x - 2)", "2", [-4.0, 5.0, -2.0, 14.0])),
        ("1/x", f("y = 1/x", "0", std)),
        ("1/x^2", f("y = 1/x^2", "0", [-5.0, 5.0, -2.0, 8.0])),
        ("1/(x-2)", f("y = 1/(x - 2)", "2", [-4.0, 8.0, -6.0, 6.0])),
        ("3/(x+1)^2", f("y = 3/(x + 1)^2", "-1", [-6.0, 4.0, -2.0, 8.0])),
        ("(x+1)/(x-3)", f("y = (x + 1)/(x - 3)", "3", [-6.0, 10.0, -6.0, 8.0])),
        ("2x/(x-1)", f("y = 2x/(x - 1)", "1", [-6.0, 8.0, -6.0, 8.0])),
        ("(2x^2+1)/(x^2+1)", f("y = (2x^2 + 1)/(x^2 + 1)", "0", [-8.0, 8.0, -3.0, 4.0])),
        ("x^2/(x^2-4)", f("y = x^2/(x^2 - 4)", "2", [-7.0, 7.0, -5.0, 6.0])),
        ("1/(x^2-1)", f("y = 1/(x^2 - 1)", "-1, 1", [-5.0, 5.0, -5.0, 5.0])),
        ("(x-1)/(x^2-1)", f("y = (x - 1)/(x^2 - 1)", "-1, 1", [-5.0, 5.0, -5.0, 5.0])),
        ("|x|/x", f("y = |x|/x", "0", [-5.0, 5.0, -3.0, 3.0])),
        ("|x-2|/(x-2)", f("y = |x - 2|/(x - 2)", "2", [-3.0, 7.0, -3.0, 3.0])),
        ("sin x / x", json!({"pieces": "y = sin(x)/x", "dots": "", "ask_at": "0", "xmin": -10.0, "xmax": 10.0, "ymin": -1.0, "ymax": 2.0, "ystep": 0.5, "xstep": 2.0, "square": false})),
        ("(1-cos x)/x", json!({"pieces": "y = (1 - cos(x))/x", "dots": "", "ask_at": "0", "xmin": -10.0, "xmax": 10.0, "ymin": -2.0, "ymax": 2.0, "ystep": 0.5, "xstep": 2.0, "square": false})),
        ("e^(-x)", f("y = e^(-x)", "0", [-3.0, 8.0, -2.0, 8.0])),
        ("ln x", f("y = ln(x)", "1", [-2.0, 10.0, -5.0, 4.0])),
        ("sqrt x", f("y = sqrt(x)", "0, 4", [-2.0, 10.0, -2.0, 5.0])),
        ("tan x", f("y = tan(x)", "", [-5.0, 5.0, -6.0, 6.0])),
        ("1/(x-2)+3", f("y = 1/(x - 2) + 3", "2", [-4.0, 8.0, -2.0, 9.0])),
        ("(3x-6)/(x+2)", f("y = (3x - 6)/(x + 2)", "-2", [-10.0, 6.0, -8.0, 12.0])),
        ("jump x+2 | 4-x", f("y = x + 2, x < 1\ny = 4 - x, x >= 1", "1", [-5.0, 5.0, -3.0, 6.0])),
        ("three pieces x^2 | 6-x | 1", f("y = x^2, x < 2\ny = 6 - x, 2 <= x < 4\ny = 1, x >= 4", "2, 4", [-5.0, 6.0, -3.0, 7.0])),
        ("continuous join", f("y = 2x + 1, x <= 0\ny = x^2 + 1, x > 0", "0", [-4.0, 4.0, -4.0, 8.0])),
        ("-x | sqrt x", f("y = -x, x < 0\ny = sqrt(x), x >= 0", "0", [-5.0, 9.0, -2.0, 6.0])),
        ("3 | x^2 | 4-x", f("y = 3, x < -1\ny = x^2, -1 <= x <= 2\ny = 4 - x, x > 2", "-1, 2", [-5.0, 6.0, -3.0, 6.0])),
        ("x^2-4 | 2x and a point", fd("y = x^2 - 4, x < 2\ny = 2x, x > 2", "(2, 1)", "2", [-4.0, 5.0, -5.0, 9.0])),
        ("hole by x≠2 with a point", fd("y = x + 1, x \u{2260} 2", "(2, 5)", "2", [-4.0, 6.0, -3.0, 7.0])),
        ("hole by x≠1", f("y = x^2 - 1, x != 1", "1", [-4.0, 4.0, -2.0, 6.0])),
        ("step 2 | -1", f("y = 2, x < 0\ny = -1, x >= 0", "0", [-5.0, 5.0, -4.0, 4.0])),
        ("staircase", f("y = 0, 0 <= x < 1\ny = 1, 1 <= x < 2\ny = 2, 2 <= x < 3\ny = 3, 3 <= x < 4", "1, 2, 3", [-1.0, 5.0, -1.0, 5.0])),
        ("1/(x-1) | x-1", f("y = 1/(x - 1), x < 1\ny = x - 1, x >= 1", "1", [-5.0, 6.0, -6.0, 5.0])),
        ("(x^2-1)/(x+1) | 5-x", f("y = (x^2 - 1)/(x + 1), x < 2\ny = 5 - x, x >= 2", "-1, 2", [-5.0, 7.0, -7.0, 6.0])),
        ("sin x | x", f("y = sin(x), x < 0\ny = x, x >= 0", "0", [-7.0, 4.0, -3.0, 4.0])),
        ("e^x | 2-x", f("y = e^x, x < 0\ny = 2 - x, x >= 0", "0", [-5.0, 5.0, -4.0, 4.0])),
        ("closed both ends 4-x^2", f("y = 4 - x^2, -2 <= x <= 2", "-2, 0, 2", [-4.0, 4.0, -2.0, 6.0])),
        ("semicircle", f("y = sqrt(9 - x^2)", "3", [-5.0, 5.0, -2.0, 5.0])),
        ("2+1/(x-3) | x-1", f("y = x - 1, x <= 3\ny = 2 + 1/(x - 3), x > 3", "3", [-3.0, 10.0, -4.0, 8.0])),
        ("x^2 | -2/(x+1)", f("y = x^2, x < -1\ny = -2/(x + 1), x > -1", "-1", [-5.0, 5.0, -7.0, 7.0])),
        ("AP style", json!({"pieces": "(-6, -1) (-4, 1) (-2, 2) open\nopen (-2, -1) (0, 0) (1, 2) closed\nopen (1, 4) (2, 3) (3, 3.5) open\ny = 1 + 1/(x - 5), x > 3", "dots": "(3, 1)", "ask_at": "-2, 1, 3, 5", "xmin": -6.0, "xmax": 8.0, "ymin": -5.0, "ymax": 6.0})),
        ("points: jump", f("(-5, 1) (-3, 3) (-1, 2) (1, 4) open\nclosed (1, 1) (3, 0) (5, 2)", "1", std)),
        ("points: hill", f("(-4, -2) (-2, 0) (0, 1) (2, 0) (4, -2)", "0", std)),
        ("points: hole with a point", fd("(-5, 2) (-3, 2) (-1, 4) open\nopen (-1, 4) (2, 3) (4, 3)", "(-1, 1)", "-1", std)),
        ("points: jump down", f("(-6, 0) (-4, 3) (-2, 1) open\nopen (-2, 4) (0, 2) (3, 2)", "-2", std)),
        ("points: zigzag", f("(0, 0) (1, 1) (2, 0) (3, 1) (4, 0)", "2", [-1.0, 5.0, -1.0, 3.0])),
        ("points: closed ends", f("closed (-3, 1) (0, 3) (3, 1) closed", "-3, 3", std)),
        ("points and a pole", f("(-6, 1) (-4, 2) (-2, 0) open\ny = 1/(x + 1), x > -1", "-2, -1", std)),
        ("points: open and closed", f("open (-4, 3) (-2, 1) (0, 2) closed\nopen (0, -1) (2, -2) (4, 0)", "0", std)),
        ("points: wave", f("(-3, -1) (-1, 2) (1, -1) (3, 2)", "1", std)),
        ("points: three parts", fd("(-5, 4) (-3, 2) open\n(-3, 0) (-1, -2) (1, 0) open\n(1, 3) (3, 3)", "(-3, 2)", "-3, 1", std)),
        ("(2x-1)/(x+1)", f("y = (2x - 1)/(x + 1)", "-1", [-8.0, 6.0, -6.0, 8.0])),
        ("(x^2-4)/(x^2-x-2)", f("y = (x^2 - 4)/(x^2 - x - 2)", "-1, 2", [-6.0, 6.0, -5.0, 6.0])),
        ("5x/(x^2+1)", f("y = 5x/(x^2 + 1)", "0", [-8.0, 8.0, -4.0, 4.0])),
        ("(x^2+1)/(x-1)", f("y = (x^2 + 1)/(x - 1)", "1", [-6.0, 7.0, -8.0, 10.0])),
        ("3-2/x^2", f("y = 3 - 2/x^2", "0", [-6.0, 6.0, -5.0, 6.0])),
        ("arctan", json!({"pieces": "y = arctan(x)", "dots": "", "ask_at": "", "xmin": -8.0, "xmax": 8.0, "ymin": -3.0, "ymax": 3.0})),
        ("(4x^2-1)/(2x^2+3x-2)", f("y = (4x^2 - 1)/(2x^2 + 3x - 2)", "1/2, -2", [-6.0, 4.0, -6.0, 8.0])),
        ("opening state", json!({})),
    ]
}
fn merge(a: &J, b: &J) -> J { let mut o = a.as_object().unwrap().clone(); for (k, v) in b.as_object().unwrap() { o.insert(k.clone(), v.clone()); } J::Object(o) }

/// Labels left off the graph (written under it) at Large on the classroom list, each named with why there is no room.
const LARGE_UNDER: &[&str] = &[
    // the hole sits near the top of a window 9 wide, on a curve rising almost straight up between the y-axis and the edge
    "hole (x^3-8)/(x-2)",
    // the hole at (1/2, 4/5) sits in the corner between the axes and the curve, under the asymptote y = 2
    "(4x^2-1)/(2x^2+3x-2)",
];
/// With every option on at once (the guide, the epsilon band, both kinds of label, arrows), the crowded cases whose label is
/// written under the graph, each named.
const ALL_UNDER: &[&str] = &["hole (x^3-8)/(x-2)", "sin x / x", "ln x", "hole by x≠1", "points and a pole", "5x/(x^2+1)", "(4x^2-1)/(2x^2+3x-2)"];

#[test]
fn every_label_on_the_classroom_list_is_clear_and_by_its_own_thing() {
    let mut m = Meter::new();
    let list = classroom();
    assert!(list.len() >= 60, "{}", list.len());
    let options = [
        ("none", json!({})),
        ("asymptote equations", json!({"asym_eqs": true})),
        ("kind labels", json!({"kind_labels": true})),
        ("guide", json!({"guide": true})),
        ("arrows", json!({"arrows": true})),
        ("epsilon band", json!({"eps_band": true})),
        ("questions", json!({"ask_inf": "Both", "ask_disc": true})),
        ("all", json!({"asym_eqs": true, "kind_labels": true, "guide": true, "arrows": true, "eps_band": true, "ask_inf": "Both", "ask_disc": true})),
    ];
    let mut tally: std::collections::BTreeMap<String, Res> = Default::default();
    let mut problems: Vec<String> = vec![];
    let mut figs = 0;
    for (name, base) in &list {
        for (oname, o) in &options {
            for size in ["Small", "Medium", "Large"] {
                let kv = merge(&merge(base, o), &json!({"label_size": size}));
                let p = with(kv.clone());
                let s = svg(&p);
                let r = check(&mut m, &s, &p);
                figs += 1;
                for pr in &r.problems { problems.push(format!("{name} / {oname} / {size}: {pr}")); }
                if r.under > 0 && !(*oname == "all" && ALL_UNDER.contains(name)) && (size != "Large" || !LARGE_UNDER.contains(name)) { problems.push(format!("{name} / {oname} / {size}: {} label(s) written under the graph: {:?}", r.under, last_drawn(&p).labels.iter().filter(|l| l.2 == Spot::Off).map(|l| l.1.clone()).collect::<Vec<_>>())); }
                for kind in ['a', 'k'] {
                    let key = format!("{size} {}", if kind == 'a' { "asymptote equation" } else { "kind label" });
                    let t = tally.entry(key).or_default();
                    for l in last_drawn(&p).labels.iter().filter(|l| l.0 == kind) { match l.2 { Spot::Beside(_) => t.beside += 1, Spot::Leader(..) => t.leader += 1, Spot::Off => t.under += 1 } }
                }
                { let t = tally.entry(format!("{size} axis numbers")).or_default(); t.hidden_numbers += r.hidden_numbers; t.dropped_numbers += r.dropped_numbers; }
            }
        }
    }
    for (name, base) in &list { for size in ["Medium"] { let p = with(merge(base, &json!({"label_size": size}))); let _ = svg(&p); let d = last_drawn(&p).dropped_numbers; if d > 0 { println!("numbers left out: {name} {size}: {d}"); } } }
    for (k, t) in &tally { if k.ends_with("numbers") { println!("{k}: under a point {}, left out where two numbers met {}", t.hidden_numbers, t.dropped_numbers); } else { println!("{k}: beside {}, on a leader {}, written under the graph {}", t.beside, t.leader, t.under); } }
    println!("{figs} figures checked");
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.iter().take(60).cloned().collect::<Vec<_>>().join("\n"));
}

// ───────────────────────── controls ─────────────────────────

fn bases() -> Vec<J> {
    vec![
        json!({}),
        json!({"show_answers": false}),
        json!({"show_answers": false, "kind_labels": true, "asym_eqs": true}),
        json!({"kind_labels": true, "asym_eqs": true, "guide": true, "arrows": true, "eps_band": true, "ask_at": "-2", "ask_inf": "Both", "ask_disc": true}),
        json!({"pieces": "y = x + 2, x < 1\ny = 4 - x, x >= 1", "dots": "", "xmin": -5.0, "xmax": 5.0, "ymin": -5.0, "ymax": 5.0}),
        json!({"pieces": "y = (x^2 - 4)/(x - 2)", "dots": "(2, 1)", "ask_at": "2", "inf_answer": "does not exist"}),
        json!({"pieces": "(-5, 1) (-3, 3) (-1, 2) (1, 4) open\nclosed (1, 1) (3, 0) (5, 2)", "dots": "", "ask_at": ""}),
        json!({"pieces": "y = 1/x^2", "dots": "", "ask_at": "0", "dne_word": "DNE", "show_asym": false}),
        json!({"ask_at": "", "show_answers": false}),
        json!({"pieces": "y = sin(x)/x", "dots": "", "ask_at": "0", "eps_band": true, "show_answers": false, "title_mode": "Blank line"}),
        json!({"ask_at": "4", "show_answers": false, "eps_band": true}),
    ]
}

/// Each switch and choice is shown exactly where changing it changes the drawing; the hidden ones change nothing.
#[test]
fn every_control_is_shown_exactly_where_it_draws() {
    let d = def();
    let defs = (d.params)();
    let mut n = 0;
    for base in bases() {
        let p = with(base.clone());
        let s0 = svg(&p);
        for pd in &defs {
            let changed = match &pd.kind {
                crate::figures::Kind::Bool => { let mut q = p.clone(); q.set_b(pd.key, !p.b(pd.key)); svg(&q) != s0 }
                crate::figures::Kind::Choice(opts) => opts.iter().any(|o| { let mut q = p.clone(); q.set_s(pd.key, o); svg(&q) != s0 }),
                _ => continue,
            };
            assert_eq!(crate::figures::param_relevant(ID, pd.key, &p), changed, "{base}: {} shown {} but changes the drawing {changed}", pd.key, !changed);
            n += 1;
        }
        // the typed boxes that act only sometimes
        for (key, v) in [("title", "Limits"), ("eps", "1/10")] {
            let mut q = p.clone();
            q.set_s(key, v);
            assert_eq!(crate::figures::param_relevant(ID, key, &p), svg(&q) != s0, "{base}: {key}");
        }
    }
    assert!(n > 200, "{n}");
}

#[test]
fn each_option_draws_differently() {
    for (key, opts, base) in [
        ("ask_inf", vec!["None", "x \u{2192} \u{221e}", "x \u{2192} \u{2212}\u{221e}", "Both"], json!({})),
        ("inf_answer", vec!["\u{221e} or \u{2212}\u{221e}", "does not exist"], json!({"ask_at": "4"})),
        ("dne_word", vec!["does not exist", "DNE"], json!({})),
        ("title_mode", vec!["Text", "Blank line", "None"], json!({"title": "Limits from a graph"})),
        ("grid", vec!["Light", "Standard", "Bold", "None"], json!({})),
        ("label_size", vec!["Large", "Medium", "Small"], json!({})),
        ("style", vec!["Colour", "Print"], json!({})),
    ] {
        let renders: Vec<String> = opts.iter().map(|o| { let mut p = with(base.clone()); p.set_s(key, o); svg(&p) }).collect();
        for i in 0..renders.len() { for j in i + 1..renders.len() { assert_ne!(renders[i], renders[j], "{key}: {} and {} draw the same", opts[i], opts[j]); } }
    }
    // the axes' From, To and Count by, minor lines, numbers, square cells and the axis labels each change the drawing
    let s0 = svg(&with(json!({})));
    for kv in [json!({"xmin": -8.0}), json!({"xmax": 12.0}), json!({"xstep": 2.0}), json!({"ymin": -8.0}), json!({"ymax": 8.0}), json!({"ystep": 2.0}), json!({"minor": 2}), json!({"numbers": false}), json!({"square": false}), json!({"xlabel": "t"}), json!({"ylabel": "d"}), json!({"title": "Limits"}), json!({"ask_at": "-2, 1, 4"}), json!({"dots": "(-2, 3), (7, -3)"})] {
        assert_ne!(svg(&with(kv.clone())), s0, "{kv}");
    }
    // the same params always give the same drawing (a build in between does not change it)
    let p = with(json!({"kind_labels": true, "asym_eqs": true}));
    let a = svg(&p);
    let _ = svg(&with(json!({"ask_at": "4"})));
    for k in 0..6 { let _ = svg(&with(json!({"ask_at": format!("{k}")}))); }
    assert_eq!(svg(&p), a);
}

// ───────────────────────── copies, Print, links ─────────────────────────

#[test]
fn student_copy_and_key() {
    let d = def();
    let p = with(json!({"ask_inf": "Both", "ask_disc": true, "kind_labels": true, "asym_eqs": true}));
    let (stu, key) = crate::figures::answer_pair(&d, &p).unwrap();
    let (ws, wk) = (words(&stu), words(&key));
    // the key, worked by hand above: 4, 1, does not exist, 1 at x = 1; 2 at ∞ and -∞ at -∞; the three discontinuities
    for answer in ["lim x→1- f(x) = 4", "lim x→1+ f(x) = 1", "lim x→1 f(x) = does not exist", "f(1) = 1", "lim x→∞ f(x) = 2", "lim x→-∞ f(x) = -∞", "x = -2 (removable)", "x = 1 (jump)", "x = 4 (infinite)", "removable", "jump", "infinite", "x = 4", "y = 2"] {
        assert!(wk.contains(answer), "key lacks {answer}: {wk}");
    }
    // the student copy asks the same questions with a line for each answer, and gives nothing away: no answer, no kind label,
    // no asymptote equation (the dashed asymptotes and every circle stay)
    for q in ["lim x→1- f(x) =", "lim x→1+ f(x) =", "lim x→1 f(x) =", "f(1) =", "lim x→∞ f(x) =", "lim x→-∞ f(x) =", "Where is f not continuous?"] { assert!(ws.contains(q), "student copy lacks {q}: {ws}"); }
    for answer in ["= 4", "= 1 ", "does not exist", "= 2", "= -∞", "removable", "jump", "infinite", "x = 4", "y = 2", "(removable)"] { assert!(!ws.contains(answer), "student copy shows {answer}: {ws}"); }
    assert_eq!(stu.matches("data-blank").count(), 7, "a line for each of the 4 + 2 limits asked and one for the list");
    assert_eq!(key.matches("data-blank").count(), 0);
    assert_eq!(stu.matches("<circle").count(), key.matches("<circle").count(), "the circles and points stay");
    assert_eq!(stu.matches(r#"data-lim="asym""#).count(), 2, "the dashed asymptotes stay");
    // the SVG's own title and description name the figure only
    let annotated = svg(&{ let mut q = p.clone(); q.set_b("show_answers", false); q });
    let desc = &annotated[..annotated.find("</desc>").unwrap()];
    assert!(!desc.contains("does not exist") && !desc.contains("x = 4"));
    // a teacher may keep the kind labels and the equations on the student copy
    let mut q = p.clone();
    q.set_b("show_answers", false);
    q.set_b("stu_hide_kinds", false);
    q.set_b("stu_hide_asym", false);
    let w = words(&svg(&q));
    assert!(w.contains("removable") && w.contains("x = 4") && !w.contains("does not exist"), "{w}");
    // a title can be a line to write on
    let w = svg(&with(json!({"title_mode": "Blank line"})));
    assert!(w.contains(r#"y1="28.00""#) && w.contains(r#"y2="28.00""#));
}

#[test]
fn print_is_grey_only() {
    for base in bases() {
        let p = with(merge(&base, &json!({"style": "Print", "kind_labels": true, "asym_eqs": true, "guide": true, "arrows": true, "eps_band": true})));
        let s = svg(&p);
        for cap in s.split('#').skip(1) {
            let h: String = cap.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if h.len() != 6 { continue; }
            let ch: Vec<i32> = (0..3).map(|i| i32::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap()).collect();
            assert!(ch.iter().max().unwrap() - ch.iter().min().unwrap() <= 24, "{base}: colour #{h} in Print");
        }
        crate::figures::to_png(&s, 0.5).unwrap();
    }
}

#[test]
fn shared_links_and_saved_specs_round_trip() {
    use crate::figures::{share, Value};
    let d = def();
    let start = start_params(&d);
    let mut cases = vec![start.clone()];
    for e in crate::figures::examples_for(ID) {
        let mut p = start.clone();
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        cases.push(p);
    }
    cases.push(with(json!({"pieces": "y = x \u{2260} 2; y = 3/(x - 1), x >= 2", "dots": "(2, -1)", "ask_at": "1/2, \u{2212}3", "style": "Print", "label_size": "Large", "show_answers": false, "kind_labels": true})));
    for p in cases {
        let link = share::encode(ID, &p, &start);
        let (lid, changed) = share::decode(&link).unwrap();
        assert_eq!(lid, ID);
        let q = share::link_params(&d, changed, share::version(&link));
        assert_eq!(d.svg(&q), d.svg(&p), "share link");
        let spec = crate::model::FigureSpec { id: ID.to_string(), params: p.0.iter().map(|(k, v)| (k.clone(), match v { Value::F(f) => json!(f), Value::I(i) => json!(i), Value::B(b) => json!(b), Value::S(s) => json!(s) })).collect() };
        let back: crate::model::FigureSpec = serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
        let r = crate::figures::spec::params_from_spec(&d, &back);
        assert_eq!(d.svg(&r), d.svg(&p), "saved spec");
    }
}

// ───────────────────────── handles ─────────────────────────

fn handles(p: &Params) -> Vec<crate::figures::interact::Handle> {
    let s = svg(p);
    crate::figures::interact::handles_for(ID, p, crate::figures::svg_size(&s))
}
/// The handle the workspace gives a press at `at` (the real hit test).
fn under(p: &Params, at: (f64, f64)) -> crate::figures::interact::Handle {
    let hs = handles(p);
    let i = crate::figures::interact::pick(&hs, at, 1.0).expect("a handle under the pointer");
    hs.into_iter().nth(i).unwrap()
}

#[test]
fn the_opening_state_has_its_handles_and_they_act() {
    let p = start_params(&def());
    let hs = handles(&p);
    let tips: Vec<&str> = hs.iter().map(|h| h.tip.as_str()).collect();
    for t in ["Click to switch this endpoint to closed", "Click to switch this endpoint to open", "Click to fill this hole (takes out its x \u{2260} part)", "Drag up or down to change f(\u{2212}2)", "Drag along the x-axis to move x = a, where the limits are asked (a = 1)"] {
        assert!(tips.contains(&t), "{t}: {tips:?}");
    }
    let (x0, _, y0, _, ml, mt, ux, uy) = last_drawn(&p).frame;
    let ph = { let g = last_drawn(&p).frame; let _ = g; 13.0 * uy };
    let px = |x: f64, y: f64| (ml + (x - x0) * ux, mt + ph - (y - y0) * uy);
    // the open end of piece 1 at (1, 4): a click makes it closed (x < 1 becomes x <= 1)
    let h = under(&p, px(1.0, 4.0));
    assert_eq!(h.tip, "Click to switch this endpoint to closed");
    let mut q = p.clone();
    (h.click.as_ref().unwrap())(&mut q);
    assert!(q.s("pieces").starts_with("y = 4 - (x - 1)^2/3, x <= 1"), "{}", q.s("pieces"));
    assert!(svg(&q).matches(r#"data-lim="closed""#).count() == svg(&p).matches(r#"data-lim="closed""#).count() + 1);
    // the closed end of piece 2 at (1, 1): a click makes it open (x >= 1 becomes x > 1), and f(1) is then undefined
    let h = under(&p, px(1.0, 1.0));
    let mut q = p.clone();
    (h.click.as_ref().unwrap())(&mut q);
    assert!(q.s("pieces").ends_with("y = 2 + 3/(x - 4), x > 1"), "{}", q.s("pieces"));
    assert!(words(&svg(&q)).contains("f(1) = undefined"));
    // the hole at (-2, 1): a click fills it (the x ≠ -2 is taken out), and f then has two values at -2 (the curve and the point)
    let h = under(&p, px(-2.0, 1.0));
    let mut q = p.clone();
    (h.click.as_ref().unwrap())(&mut q);
    assert!(q.s("pieces").starts_with("y = 4 - (x - 1)^2/3, x < 1\n"), "{}", q.s("pieces"));
    assert!(warnings(&q).iter().any(|n| n.starts_with("f has two values at x = \u{2212}2")), "{:?}", warnings(&q));
    // the point above the hole: dragged up to 5, f(-2) = 5
    let h = under(&p, px(-2.0, 3.0));
    let mut q = p.clone();
    (h.drag.as_ref().unwrap())(&mut q, px(-2.3, 5.1));
    assert_eq!(q.s("dots"), "(-2, 5)");
    q.set_s("ask_at", "-2");
    assert!(words(&svg(&q)).contains("f(-2) = 5"));
    // the marker at x = 1 on the axis: dragged to 3, the questions are asked at 3
    let h = under(&p, px(1.0, 0.0));
    assert!(h.tip.starts_with("Drag along the x-axis to move x = a"));
    let mut q = p.clone();
    (h.drag.as_ref().unwrap())(&mut q, px(2.9, 0.4));
    assert_eq!(q.s("ask_at"), "3");
    assert!(words(&svg(&q)).contains("lim x→3- f(x) = -1"), "2 + 3/(3 - 4) = -1: {}", words(&svg(&q)));
    // with several values of a, a drag moves only its own
    let p2 = with(json!({"ask_at": "-2, 1/2, 4"}));
    let h = under(&p2, px(4.0, 0.0));
    let mut q = p2.clone();
    (h.drag.as_ref().unwrap())(&mut q, px(6.0, 0.0));
    assert_eq!(q.s("ask_at"), "-2, 1/2, 6");
}

#[test]
fn points_ends_switch_by_their_words() {
    let p = with(json!({"pieces": "(-5, 1) (-3, 3) (-1, 2) (1, 4) open\nclosed (1, 1) (3, 0) (5, 2)", "dots": "", "ask_at": "1"}));
    let (x0, _, y0, y1, ml, mt, ux, uy) = last_drawn(&p).frame;
    let px = |x: f64, y: f64| (ml + (x - x0) * ux, mt + (y1 - y0) * uy - (y - y0) * uy);
    let h = under(&p, px(1.0, 4.0));
    let mut q = p.clone();
    (h.click.as_ref().unwrap())(&mut q);
    assert_eq!(q.s("pieces"), "(-5, 1) (-3, 3) (-1, 2) (1, 4) closed\nclosed (1, 1) (3, 0) (5, 2)");
}

// ───────────────────────── examples, extremes, time ─────────────────────────

#[test]
fn examples_and_classic_setups() {
    let ex = crate::figures::examples_for(ID);
    assert!(ex.len() >= 6);
    let names: Vec<&str> = crate::figures::classic_setups(ID).iter().map(|e| e.name).collect();
    assert_eq!(names, ["Hole with the point moved", "Jump with one-sided limits", "Vertical asymptote with limits of \u{b1}\u{221e}", "Horizontal asymptote at infinity", "AP-style graph with every kind of discontinuity"]);
    let d = def();
    for e in &ex {
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        let s = d.svg(&p);
        assert!(s.contains("<polyline"), "{}: no curve", e.name);
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
    }
    // the AP-style graph, read by hand: a jump at -2 (2 and -1, no value), a jump at 1 (2 closed, 4 open), a jump at 3 (3.5 and
    // 1 + 1/(3 - 5) = 1/2, the point gives f(3) = 1), an infinite one at 5; at ∞, 1 + 1/(x - 5) → 1
    let ap = &ex[4];
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in &ap.params { p.0.insert(k.to_string(), v.clone()); }
    let w = words(&d.svg(&p));
    for a in ["lim x→-2- f(x) = 2", "lim x→-2+ f(x) = -1", "f(-2) = undefined", "lim x→1- f(x) = 2", "lim x→1+ f(x) = 4", "f(1) = 2", "lim x→3- f(x) = 3.5", "lim x→3+ f(x) = 1/2", "f(3) = 1", "lim x→5- f(x) = -∞", "lim x→5+ f(x) = ∞", "x = -2 (jump)", "x = 1 (jump)", "x = 3 (jump)", "x = 5 (infinite)"] {
        assert!(w.contains(a), "{a}: {w}");
    }
}

#[test]
fn the_ends_of_every_range_draw_cleanly_and_quickly() {
    let long = (0..12).map(|k| format!("y = {k} + 1/(x - {k}.5) + sin({k}x)/(x^2 + 1), {} <= x < {}", k - 6, k - 5)).collect::<Vec<_>>().join("\n");
    let knots = (0..30).map(|k| format!("({}, {})", k - 15, (k * 7) % 5)).collect::<Vec<_>>().join(" ");
    let cases = vec![
        json!({"xmin": -100.0, "xmax": 100.0, "xstep": 0.1, "ymin": -100.0, "ymax": 100.0, "ystep": 0.1, "minor": 10, "kind_labels": true, "asym_eqs": true, "guide": true, "arrows": true, "eps_band": true, "ask_inf": "Both", "ask_disc": true}),
        json!({"xmin": -0.5, "xmax": 0.5, "xstep": 0.1, "ymin": -0.5, "ymax": 0.5, "ystep": 0.1}),
        json!({"pieces": long, "ask_at": "-6, -3, 0, 2, 4, 6, 8", "kind_labels": true, "asym_eqs": true, "ask_disc": true, "ask_inf": "Both", "label_size": "Large"}),
        json!({"pieces": knots, "dots": "(1, 1) (2, 2) (3, 3) (4, 4) (5, 5) (6, 6) (7, 7) (8, 8) (9, 9) (10, 10) (11, 11) (12, 12) (13, 13)"}),
        json!({"pieces": "y = tan(x)", "xmin": -100.0, "xmax": 100.0, "xstep": 10.0, "ymin": -10.0, "ymax": 10.0, "kind_labels": true, "asym_eqs": true, "ask_disc": true}),
        json!({"pieces": "y = sin(1/x)", "ask_at": "0", "kind_labels": true, "ask_disc": true}),
        json!({"pieces": "y = 1e300 * x", "ask_inf": "Both"}),
        json!({"pieces": "", "dots": "", "ask_at": ""}),
        json!({"pieces": ")))(((", "dots": "(((", "ask_at": "1/0, 2"}),
        json!({"pieces": "y = x, 1000 < x", "dots": "(1000, 1000)", "ask_at": "999"}),
        json!({"xmin": 5.0, "xmax": -5.0, "ymin": 3.0, "ymax": 3.0}),
    ];
    for c in cases {
        let p = with(c.clone());
        let t = std::time::Instant::now();
        let s = svg(&p);
        let ms = t.elapsed().as_millis();
        let _ = warnings(&p);
        let _ = handles(&p);
        assert!(ms < 800, "{c}: {ms} ms");
        assert!(s.len() < 3_000_000, "{c}: {} bytes", s.len());
        assert!(!s.contains("NaN") && !s.contains("inf\"") && !s.contains("e+") && !s.contains("e-1"), "{c}");
        assert!(!words(&s).contains("0000000") && !words(&s).contains("999999"), "{c}: float noise in {}", words(&s));
        crate::figures::validate_svg(&s).unwrap_or_else(|e| panic!("{c}: {e}"));
        crate::figures::to_png(&s, 0.3).unwrap();
    }
    // the opening state and every example well under a second
    for e in std::iter::once(start_params(&def())).chain(crate::figures::examples_for(ID).iter().map(|e| { let mut p = start_params(&def()); for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); } p })) {
        let t = std::time::Instant::now();
        let _ = (def().render)(&{ let mut q = e.clone(); q.set_s("title", &format!("{:?}", t)); q });
        assert!(t.elapsed().as_millis() < 300);
    }
}

/// The epsilon band: sin x / x at 0 has L = 1; with ε = 1/4 the band is 3/4 < y < 5/4, and sin x / x first falls to 3/4 at
/// x ≈ 1.2757 (sin 1.2757 = 0.95679, / 1.2757 = 0.7500), so δ = 1.27 to the hundredth below.
#[test]
fn the_epsilon_band_worked_by_hand() {
    let p = with(json!({"pieces": "y = sin(x)/x", "dots": "", "ask_at": "0", "eps_band": true, "eps": "1/4", "xmin": -10.0, "xmax": 10.0, "ymin": -1.0, "ymax": 2.0, "square": false}));
    let key = svg(&p);
    let w = words(&key);
    assert!(w.contains("ε = 1/4, L = 1, a = 0") && w.contains("δ = 1.27"), "{w}");
    assert_eq!(key.matches(r#"data-lim="eps""#).count(), 4, "L ± ε and a ± δ");
    // the student copy keeps the ε band and L, and leaves δ to find
    let mut q = p.clone();
    q.set_b("show_answers", false);
    let stu = svg(&q);
    assert!(!words(&stu).contains("1.27") && stu.matches(r#"data-lim="eps""#).count() == 2, "{}", words(&stu));
    // the opening state at a = 1 has no limit, so the band is refused with a note, and the switch is hidden
    let p = with(json!({"eps_band": true}));
    assert!(warnings(&p).iter().any(|n| n.starts_with("The epsilon band needs a limit that is a number at x = 1")));
    assert!(!crate::figures::param_relevant(ID, "eps_band", &p));
    // with ε = 1/2 at the hole at -2 of the opening state (L = 1): the parabola 4 - (x - 1)^2/3 leaves 1/2 < y < 3/2 where
    // (x - 1)^2 = 7.5, x = 1 - √7.5 ≈ -1.7386 on the right (δ ≈ 0.2614) and (x - 1)^2 = 10.5, x ≈ -2.2404 on the left
    // (δ ≈ 0.2404): δ = 0.24. The point (-2, 3) is at a itself, which the definition leaves out.
    let w = words(&svg(&with(json!({"eps_band": true, "ask_at": "-2"}))));
    assert!(w.contains("δ = 0.24"), "{w}");
}

/// Words in a sentence upright, its letters and f(1) in italic.
#[test]
fn sentences_set_their_letters_in_italic() {
    let s = svg(&with(json!({"ask_disc": true})));
    let i = s.find("Where is").unwrap();
    let el = &s[s[..i].rfind("<text").unwrap()..i + s[i..].find("</text>").unwrap()];
    assert!(el.contains(r#"<tspan font-style="italic">f</tspan> not"#) && el.contains(r#"<tspan font-style="italic">x</tspan> and"#), "{el}");
}
