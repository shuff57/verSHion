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
    // (a file of lines "name<TAB>params as JSON")
    if let Ok(f) = std::env::var("LIM_CASES") { cases = std::fs::read_to_string(f).unwrap().lines().filter(|l| l.contains('\t')).map(|l| { let (n, j) = l.split_once('\t').unwrap(); (n.to_string(), with(serde_json::from_str(j).unwrap())) }).collect(); }
    for (name, p) in cases {
        let t0 = std::time::Instant::now();
        let s = svg(&p);
        println!("{name}: {} ms", t0.elapsed().as_millis());
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
/// A test value read exactly (0.5 is 1/2, -2 is -2).
fn kk(v: f64) -> super::limits::cas::K { let mut k = super::limits::Num::read(&format!("{v}")).unwrap().k; k.dec = false; k }
fn m(kv: J) -> Model { super::limits::model_of(&with(kv)) }
fn val(l: &Lim) -> String { match l { Lim::Val(_, t) => t.clone(), other => format!("{other:?}") } }
fn kinds(md: &Model) -> Vec<String> { md.discontinuities().0.iter().map(|d| format!("{} {}", d.text, d.kind)).collect() }
fn close(l: &Lim, v: f64) -> bool { matches!(l, Lim::Val(u, _) if (u - v).abs() < 1e-9) }

// ───────────────────────── hand-worked values ─────────────────────────

/// The opening state: y = 4 - (x - 1)^2/3 for x < 1 (a hole at -2), y = 2 + 3/(x - 4) for x >= 1, the point (-2, 3).
#[test]
fn the_opening_state_worked_by_hand() {
    let md = super::limits::model_of(&start_params(&def()));
    // at -2: 4 - (-3)^2/3 = 4 - 3 = 1 from both sides; the hole leaves the formula's value out, the point gives f(-2) = 3
    assert_eq!(val(&md.side(&kk(-2.0), -1.0)), "1");
    assert_eq!(val(&md.side(&kk(-2.0), 1.0)), "1");
    assert_eq!(val(&md.two(&kk(-2.0))), "1");
    assert_eq!(val(&md.value(&kk(-2.0))), "3");
    // at 1: from the left 4 - 0 = 4 (open end); from the right 2 + 3/(1 - 4) = 2 - 1 = 1 (closed end, so f(1) = 1)
    assert_eq!(val(&md.side(&kk(1.0), -1.0)), "4");
    assert_eq!(val(&md.side(&kk(1.0), 1.0)), "1");
    assert_eq!(md.two(&kk(1.0)), Lim::Dne);
    assert_eq!(val(&md.value(&kk(1.0))), "1");
    // at 4: 3/(x - 4) is a large negative number just left of 4 and a large positive one just right: -∞ and ∞; 4 is not in
    // the domain
    assert_eq!(md.side(&kk(4.0), -1.0), Lim::NInf);
    assert_eq!(md.side(&kk(4.0), 1.0), Lim::PInf);
    assert_eq!(md.two(&kk(4.0)), Lim::Dne);
    assert_eq!(md.value(&kk(4.0)), Lim::Undef);
    // at ∞: 3/(x - 4) → 0, so 2; at -∞: -(x - 1)^2/3 → -∞
    assert_eq!(val(&md.at_inf(1.0).unwrap()), "2");
    assert_eq!(md.at_inf(-1.0).unwrap(), Lim::NInf);
    // the discontinuities in the window -6..10: a hole at -2, a jump at 1, an infinite one at 4; nothing else
    let ds: Vec<(String, &str)> = md.discontinuities().0.iter().map(|d| (d.text.clone(), d.kind)).collect();
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
    assert_eq!(val(&md.two(&kk(-3.0))), "-6");
    assert_eq!(md.value(&kk(-3.0)), Lim::Undef);
    assert_eq!(kinds(&md), ["-3 removable"]);
    // (x - 1)/(x^2 - 1) = 1/(x + 1): 1/2 at 1 (a hole), and a pole at -1
    let md = m(json!({"pieces": "y = (x - 1)/(x^2 - 1)", "dots": "", "xmin": -4.0, "xmax": 4.0, "ymin": -4.0, "ymax": 4.0}));
    assert_eq!(val(&md.two(&kk(1.0))), "1/2");
    assert_eq!(kinds(&md), ["-1 infinite", "1 removable"]);
    // (4x^2 - 1)/(2x^2 + 3x - 2) = (2x - 1)(2x + 1)/((2x - 1)(x + 2)) = (2x + 1)/(x + 2): at 1/2, 2/(5/2) = 4/5; a pole at -2;
    // the limit at ±∞ is 4/2 = 2
    let md = m(json!({"pieces": "y = (4x^2 - 1)/(2x^2 + 3x - 2)", "dots": "", "ask_at": "1/2"}));
    assert_eq!(val(&md.two(&kk(0.5))), "4/5");
    assert!(md.verticals().iter().any(|v| v.1 == "-2"));
    assert_eq!(val(&md.at_inf(1.0).unwrap()), "2");
    assert_eq!(val(&md.at_inf(-1.0).unwrap()), "2");
    // (x^3 - 8)/(x - 2) = x^2 + 2x + 4: 12 at 2
    let md = m(json!({"pieces": "y = (x^3 - 8)/(x - 2)", "dots": "", "ymax": 14.0}));
    assert_eq!(val(&md.two(&kk(2.0))), "12");
    // sin x / x at 0 is 1 (a hole); √x at 2 is √2; arctan x at ∞ is π/2 and at -∞ -π/2
    assert_eq!(val(&m(json!({"pieces": "y = sin(x)/x", "dots": ""})).two(&kk(0.0))), "1");
    assert_eq!(val(&m(json!({"pieces": "y = sqrt(x)", "dots": ""})).two(&kk(2.0))), "\u{221a}2");
    let md = m(json!({"pieces": "y = arctan(x)", "dots": ""}));
    assert_eq!(val(&md.at_inf(1.0).unwrap()), "\u{3c0}/2");
    assert_eq!(val(&md.at_inf(-1.0).unwrap()), "-\u{3c0}/2");
    // (1 - cos x)/x at 0: 1 - cos x ≈ x^2/2, so x/2 → 0
    assert_eq!(val(&m(json!({"pieces": "y = (1 - cos(x))/x", "dots": ""})).two(&kk(0.0))), "0");
}

#[test]
fn infinite_limits_and_poles() {
    // 1/x^2: an even pole, ∞ from both sides, so the limit is ∞ (it is written ∞, or does not exist when chosen)
    let md = m(json!({"pieces": "y = 1/x^2", "dots": ""}));
    assert_eq!((md.side(&kk(0.0), -1.0), md.side(&kk(0.0), 1.0), md.two(&kk(0.0))), (Lim::PInf, Lim::PInf, Lim::PInf));
    assert_eq!(md.verticals().iter().map(|v| v.1.clone()).collect::<Vec<_>>(), ["0"]);
    // an even pole between the samples, off the grid's quarter steps: 1/(x - 0.3)^2
    let md = m(json!({"pieces": "y = 1/(x - 0.3)^2", "dots": ""}));
    assert!(md.verticals().iter().any(|v| (v.0 - 0.3).abs() < 1e-9), "{:?}", md.verticals());
    // 1/(x - 2): -∞ from the left, ∞ from the right
    let md = m(json!({"pieces": "y = 1/(x - 2)", "dots": ""}));
    assert_eq!((md.side(&kk(2.0), -1.0), md.side(&kk(2.0), 1.0), md.two(&kk(2.0))), (Lim::NInf, Lim::PInf, Lim::Dne));
    // √x and x^(3/2) at 0 from the right: 0 (the first closes in as slowly as √h); -x for x < 0 meets √x at 0: continuous
    assert_eq!(val(&m(json!({"pieces": "y = sqrt(x)", "dots": ""})).side(&kk(0.0), 1.0)), "0");
    assert_eq!(val(&m(json!({"pieces": "y = x/sqrt(x)", "dots": ""})).side(&kk(0.0), 1.0)), "0");
    assert!(m(json!({"pieces": "y = -x, x < 0\ny = sqrt(x), x >= 0", "dots": ""})).discontinuities().0.is_empty());
    // √x from the left of 0: no values there, so that limit does not exist (and so neither does the limit); f(0) = 0
    let w = words(&svg(&with(json!({"pieces": "y = sqrt(x)", "dots": "", "ask_at": "0"}))));
    assert!(w.contains("lim x→0- f(x) = does not exist") && w.contains("lim x→0+ f(x) = 0") && w.contains("lim x→0 f(x) = does not exist") && w.contains("f(0) = 0"), "{w}");
    // ln x: -∞ as x → 0 from the right, nothing to the left (a domain edge, not listed as a discontinuity), and the vertical
    // asymptote x = 0
    let md = m(json!({"pieces": "y = ln(x)", "dots": ""}));
    assert_eq!((md.side(&kk(0.0), -1.0), md.side(&kk(0.0), 1.0)), (Lim::Undef, Lim::NInf));
    assert!(md.discontinuities().0.is_empty());
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
    assert_eq!(val(&at("(1 + 1/x)^x").0.unwrap()), "e");
    // a piece that stops (x <= 5) is not asked about at ∞; the panel says why
    let p = with(json!({"pieces": "y = x, x <= 5", "dots": "", "ask_inf": "Both"}));
    assert!(warnings(&p).iter().any(|n| n.starts_with("No formula piece runs on to x → ∞")), "{:?}", warnings(&p));
    assert!(!words(&svg(&p)).contains("x→∞"));
    assert!(words(&svg(&p)).contains("x→-∞"));
}

#[test]
fn jumps_points_and_values() {
    // |x|/x: -1 from the left, 1 from the right, no value at 0: a jump
    let md = m(json!({"pieces": "y = |x|/x", "dots": ""}));
    assert_eq!((val(&md.side(&kk(0.0), -1.0)), val(&md.side(&kk(0.0), 1.0)), md.value(&kk(0.0))), ("-1".into(), "1".into(), Lim::Undef));
    assert_eq!(md.discontinuities().0.iter().map(|d| d.kind).collect::<Vec<_>>(), ["jump"]);
    // sin(1/x) near 0 neither settles nor grows: the limits do not exist, an oscillating discontinuity
    let md = m(json!({"pieces": "y = sin(1/x)", "dots": ""}));
    assert_eq!((md.side(&kk(0.0), -1.0), md.side(&kk(0.0), 1.0)), (Lim::Dne, Lim::Dne));
    // x^2 for x < 2 (4), 6 - x for 2 <= x < 4 (4, then 2 at 4 open), 1 for x >= 4: continuous at 2, a jump at 4
    let md = m(json!({"pieces": "y = x^2, x < 2\ny = 6 - x, 2 <= x < 4\ny = 1, x >= 4", "dots": ""}));
    assert_eq!((val(&md.side(&kk(2.0), -1.0)), val(&md.side(&kk(2.0), 1.0)), val(&md.value(&kk(2.0)))), ("4".into(), "4".into(), "4".into()));
    assert_eq!((val(&md.side(&kk(4.0), -1.0)), val(&md.side(&kk(4.0), 1.0)), val(&md.value(&kk(4.0)))), ("2".into(), "1".into(), "1".into()));
    assert_eq!(kinds(&md), ["4 jump"]);
    // typed in decimals, written in decimals: 0.5x + 2 at 1 is 2.5
    assert_eq!(val(&m(json!({"pieces": "y = 0.5x + 2", "dots": ""})).two(&kk(1.0))), "2.5");
    // points: the values at the points are exactly as typed; the curve between stays between them (no bulge)
    let md = m(json!({"pieces": "(-5, 1) (-3, 7/2) (-1, 2) (1, 4) open\nclosed (1, 1) (3, 0) (5, 2)", "dots": ""}));
    assert_eq!((val(&md.side(&kk(1.0), -1.0)), val(&md.side(&kk(1.0), 1.0)), val(&md.value(&kk(1.0)))), ("4".into(), "1".into(), "1".into()));
    assert_eq!(val(&md.two(&kk(-3.0))), "7/2");
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
    // an odd root of a negative number is real here (the figure works it out itself): drawn on both sides, no note
    let p = with(json!({"pieces": "y = x^(1/3)", "dots": ""}));
    assert!(warnings(&p).is_empty(), "{:?}", warnings(&p));
    assert!(svg(&p).matches("<polyline").count() >= 1);
    // a point, a circle, a value of a or a discontinuity outside the window
    assert!(notes(json!({"dots": "(4, 20)"})).iter().any(|n| n.contains("outside the window")));
    assert!(notes(json!({"ask_at": "40"})).iter().any(|n| n.starts_with("x = 40 is outside the window")));
    assert!(notes(json!({"pieces": "y = 1/(x - 12)", "dots": ""})).iter().any(|n| n.contains("outside the window: x = 12 (infinite)")));
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
/// (and which of the strokes are dashed: asymptotes, guides, band edges)
fn scene(svg: &str) -> (Vec<(Pt, Pt)>, Vec<(Pt, f64)>, Vec<(Pt, Pt)>, Vec<(Pt, Pt)>) {
    let mut segs = vec![];
    let mut dashed = vec![];
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
            if name == "labels" { leaders.push(s); } else { segs.push(s); if part.contains("stroke-dasharray") { dashed.push(s); } }
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
    (segs, dots, leaders, dashed)
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
    let (segs, dots, leaders, dashed) = scene(s);
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
            // (it may cross one dashed line at an angle to reach a clear spot beyond it, never a solid one)
            let mut dash_crossed = 0;
            for (a, c) in segs.iter() { if seg_cross(*s0, *e0, *a, *c) { if dashed.contains(&(*a, *c)) { dash_crossed += 1; } else { r.problems.push(format!("leader of {} crosses a stroke", l.1)); } } }
            if dash_crossed > 1 { r.problems.push(format!("leader of {} crosses {dash_crossed} dashed lines", l.1)); }
            // and starts on (or at the edge of the circle of) its own thing
            let start = l.3.iter().map(|(a, c)| dist_pt_seg(*s0, *a, *c)).fold(f64::MAX, f64::min);
            if start > 8.0 { r.problems.push(format!("leader of {} starts {start:.1} px from its own thing", l.1)); }
        }, Spot::Off => {} }
        let own = l.3.iter().map(|(a, c)| seg_dist(*a, *c, *b)).fold(f64::MAX, f64::min);
        let own_pts: Vec<Pt> = l.3.iter().flat_map(|(a, c)| [*a, *c]).collect();
        if matches!(l.2, Spot::Beside(_)) {
            // (the circles and points a label could be taken to name; the x = a markers carry no label)
            for (q, _) in dots.iter().filter(|(_, r)| *r < 5.5) {
                if own_pts.iter().any(|o| (o.0 - q.0).abs() < 0.6 && (o.1 - q.1).abs() < 0.6) { continue; }
                if pt_box(*q, *b) < own { r.problems.push(format!("{} is nearer a point {q:?} than its own thing ({own:.1})", l.1)); }
            }
        }
    }
    for (s0, e0) in &leaders { for (s1, e1) in &leaders { if (s0, e0) != (s1, e1) && seg_cross(*s0, *e0, *s1, *e1) { r.problems.push("two leaders cross".into()); } } }
    r.under = dr.labels.iter().filter(|l| l.2 == Spot::Off).count();
    r
}
fn dist_pt_seg(q: Pt, a: Pt, c: Pt) -> f64 { let (dx, dy) = (c.0 - a.0, c.1 - a.1); let l = dx * dx + dy * dy; let t = if l < 1e-9 { 0.0 } else { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l).clamp(0.0, 1.0) }; (q.0 - a.0 - t * dx).hypot(q.1 - a.1 - t * dy) }
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
        // the second check's cases
        ("R03 hole and pole", f("y = (x^2 - 1)/((x - 1)(x - 3))", "1, 3", std)),
        ("R14 (x-1)sin(1/(x-1))", f("y = (x - 1)sin(1/(x - 1))", "1", [-6.0, 10.0, -6.0, 7.0])),
        ("R57 two poles close", f("y = (5 - 2x^2)/(3x^2 + x)", "0, -1/3", [-6.0, 10.0, -6.0, 7.0])),
        ("R86 hole and pole", f("y = (x^2 - 2x + 1)/(x^2 - 1)", "1, -1", [-6.0, 10.0, -6.0, 7.0])),
        ("floor(x) + 1", f("y = floor(x) + 1", "0", [-6.0, 10.0, -6.0, 7.0])),
        ("floor(x) on ±5", f("y = floor(x)", "2", [-5.0, 5.0, -5.0, 5.0])),
        ("floor(sin x)", f("y = floor(sin(x))", "0, pi/2, pi", [-6.0, 10.0, -6.0, 7.0])),
        ("floor(cos x)", f("y = floor(cos(x))", "0, pi", [-6.0, 10.0, -6.0, 7.0])),
        ("floor(1/x)", f("y = floor(1/x)", "1", [-4.0, 4.0, -4.0, 4.0])),
        ("x floor(x)", f("y = x floor(x)", "1", [-3.0, 3.0, -3.0, 6.0])),
        ("tan 2x", f("y = tan(2x)", "", [-6.0, 10.0, -6.0, 7.0])),
        ("sin(pi floor(1/x))", f("y = sin(pi floor(1/x))", "0", [-4.0, 4.0, -2.0, 2.0])),
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
/// Cases whose labels may be written under the graph with any option and at any size, each with why there is no spot that
/// touches no stroke and no leader that crosses nothing.
const NAMED_UNDER: &[(&str, &str)] = &[
    ("R03 hole and pole", "the hole at (1, -1) has the y-axis 30 px to its left, the curve below-right and the asymptote x = 3 beyond; every leader to open space crosses the y-axis or the asymptote"),
    ("R57 two poles close", "the asymptotes x = -1/3 and x = 0 are 10 px apart"),
    ("R86 hole and pole", "at Large the hole at (1, 0) sits on the x-axis between its numbers and the curve"),
    ("R14 (x-1)sin(1/(x-1))", "at Large the hole at (1, 0) sits on the x-axis inside the oscillation"),
    ("tan 2x", "ten asymptotes 47 px apart with a branch filling each gap"),
    ("floor(x) + 1", "at Large the steps leave no gap the width of a label beside the jumps nearest the corners"),
    ("floor(sin x)", "the steps lie along the x-axis and its numbers"),
    ("floor(cos x)", "the steps lie along the x-axis and its numbers"),
];

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
        ("kinds and equations", json!({"kind_labels": true, "asym_eqs": true})),
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
                if r.under > 0 && !NAMED_UNDER.iter().any(|(n, _)| n == name) && !(*oname == "all" && ALL_UNDER.contains(name)) && (size != "Large" || !LARGE_UNDER.contains(name)) { problems.push(format!("{name} / {oname} / {size}: {} label(s) written under the graph: {:?}", r.under, last_drawn(&p).labels.iter().filter(|l| l.2 == Spot::Off).map(|l| l.1.clone()).collect::<Vec<_>>())); }
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
        // (the second check's: many asymptotes, steps running off the plot, a floor with breaks it cannot all find)
        json!({"pieces": "y = tan(x)", "dots": "", "ask_at": "", "xmin": -100.0, "xmax": 100.0, "xstep": 10.0, "ymin": -10.0, "ymax": 10.0, "kind_labels": true}),
        json!({"pieces": "y = floor(x) + 1", "dots": "", "ask_at": "0", "kind_labels": true}),
        json!({"pieces": "y = floor(1/x)", "dots": "", "ask_at": "1", "xmin": -4.0, "xmax": 4.0, "ymin": -4.0, "ymax": 4.0, "show_answers": false}),
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
    for kv in [json!({"xmin": -8.0}), json!({"xmax": 12.0}), json!({"xstep": 2.0}), json!({"ymin": -8.0}), json!({"ymax": 8.0}), json!({"ystep": 2.0}), json!({"minor": 2}), json!({"numbers": false}), json!({"square": false}), json!({"xlabel": "t"}), json!({"ylabel": "d"}), json!({"title": "Limits"}), json!({"ask_at": "-2, 1, 4"}), json!({"dots": "(-2, 3), (4, 5)"})] {
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
    // (the point (-2, 3) then disagrees with the curve's value 1 at -2: it is not drawn, and the panel says why)
    assert!(warnings(&q).iter().any(|n| n.starts_with("The point (-2, 3) is not drawn: f already has the value 1 at x = -2")), "{:?}", warnings(&q));
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
        assert!(ms < 500, "{c}: {ms} ms");
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
    // the student copy that does not ask the limit at a keeps the ε band and L, and leaves δ to find; one that asks it
    // leaves the band off (it would give the limit away)
    let mut q = p.clone();
    q.set_b("show_answers", false);
    let asked = svg(&q);
    assert!(asked.matches(r#"data-lim="eps""#).count() == 0 && !words(&asked).contains("L = 1"), "{}", words(&asked));
    for k in ["ask_left", "ask_right", "ask_two"] { q.set_b(k, false); }
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

/// A label sits nearer its own asymptote (or discontinuity) than any other: two vertical asymptotes a little apart.
#[test]
fn like_labels_sit_on_their_own_thing() {
    for size in ["Small", "Medium", "Large"] {
        for e in ["y = 1/(x^2 - 1)", "y = 1/((x - 1)(x - 2))", "y = tan(x)", "y = x/((x + 1)(x - 2)(x - 3))"] {
            let p = with(json!({"pieces": e, "dots": "", "ask_at": "", "asym_eqs": true, "kind_labels": true, "label_size": size, "xmin": -5.0, "xmax": 5.0, "ymin": -5.0, "ymax": 5.0}));
            let _ = svg(&p);
            let dr = last_drawn(&p);
            for (i, l) in dr.labels.iter().enumerate() {
                let Spot::Beside(c) = l.2 else { continue };
                let b = [c.0 - 1.0, c.1 - 1.0, c.0 + 1.0, c.1 + 1.0];
                let own = l.3.iter().map(|(a, d)| seg_dist(*a, *d, b)).fold(f64::MAX, f64::min);
                for (j, o) in dr.labels.iter().enumerate() {
                    if i == j || o.0 != l.0 { continue; }
                    let other = o.3.iter().map(|(a, d)| seg_dist(*a, *d, b)).fold(f64::MAX, f64::min);
                    assert!(other > own, "{e} {size}: {} is {own:.1} from its own thing but {other:.1} from {}'s", l.1, o.1);
                }
            }
        }
    }
    // an odd root of a negative number is real: ∛(-1) = -1 from both sides and at -1
    let p = with(json!({"pieces": "y = x^(1/3)", "dots": "", "ask_at": "-1"}));
    let w = words(&svg(&p));
    assert!(w.contains("lim x→-1- f(x) = -1") && w.contains("f(-1) = -1") && !w.contains("undefined"), "{w}");
    // a value this small keeps its first figures: 0.00001x at 1 is 0.00001, never 0
    assert_eq!(super::limits::cas::short(1e-5 / 3.0), "0.00000333");
    // a drag never carries a point or a marker off the window
    let p = start_params(&def());
    let hs = handles(&p);
    let dot = hs.iter().find(|h| h.tip.starts_with("Drag up or down")).unwrap();
    let mut q = p.clone();
    (dot.drag.as_ref().unwrap())(&mut q, (dot.pos.0, -5000.0));
    assert_eq!(q.s("dots"), "(-2, 7)");
    let mk = hs.iter().find(|h| h.tip.starts_with("Drag along")).unwrap();
    let mut q = p.clone();
    (mk.drag.as_ref().unwrap())(&mut q, (-5000.0, mk.pos.1));
    assert_eq!(q.s("ask_at"), "-6");
}

#[test]
#[ignore]
fn debug_model() {
    let mut p = start_params(&def());
    if let Ok(j) = std::env::var("LIM_JSON") { p = crate::figures::params_from_json(&def(), &serde_json::from_str(&j).unwrap()); }
    let md = super::limits::model_of(&p);
    let (ds, done) = md.discontinuities();
    println!("complete {done}: {}", ds.iter().map(|d| format!("{} {}", d.text, d.kind)).collect::<Vec<_>>().join(", "));
    for pc in &md.pieces { println!("piece {} lo {:?} hi {:?} holes {:?}", pc.n, pc.lo.as_ref().map(|e| e.x.v), pc.hi.as_ref().map(|e| e.x.v), pc.holes.iter().map(|h| h.0.v).collect::<Vec<_>>()); }
    println!("{:?}", md.notes);
}

/// Every classic setup and example is what its name says, by the figure's own analysis.
#[test]
fn every_example_is_what_its_title_says() {
    let d = def();
    for e in crate::figures::examples_for(ID) {
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        let md = super::limits::model_of(&p);
        let (ds, complete) = md.discontinuities();
        assert!(complete, "{}", e.name);
        let kinds: Vec<&str> = ds.iter().map(|d| d.kind).collect();
        let has = |k: &str| kinds.contains(&k);
        match e.name {
            "Hole with the point moved" => assert!(kinds == ["removable"] && md.dots.len() == 1, "{}: {kinds:?}", e.name),
            "Jump with one-sided limits" => {
                // x + 2 → 3 from the left of 1, 4 - 2x → 2 from the right: a real jump
                assert_eq!(kinds, ["jump"], "{}", e.name);
                let a = Num::read("1").unwrap().k;
                assert_eq!((md.side(&a, -1.0), md.side(&a, 1.0)), (Lim::Val(3.0, "3".into()), Lim::Val(2.0, "2".into())));
            }
            "Vertical asymptote with limits of \u{b1}\u{221e}" => { assert_eq!(kinds, ["infinite"], "{}", e.name); assert_eq!(md.verticals().len(), 1); }
            "Horizontal asymptote at infinity" => { assert!(kinds.is_empty()); assert_eq!(md.horizontals().iter().map(|h| h.1.clone()).collect::<Vec<_>>(), ["2"]); }
            "AP-style graph with every kind of discontinuity" => assert!(has("removable") && has("jump") && has("infinite"), "{}: {kinds:?}", e.name),
            "A graph from points with a hole" => assert!(has("removable"), "{}: {kinds:?}", e.name),
            "sin x / x near 0" => assert_eq!(kinds, ["removable"], "{}", e.name),
            "Piecewise function with a closed and an open end" => assert_eq!(kinds, ["jump"], "{}: x² meets 6 - x at 2 (4 = 4), 2 then 1 at 4", e.name),
            other => panic!("an example without a check: {other}"),
        }
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
    }
}

/// No student copy gives an answer away: over every function of a set and every combination of the options a student copy
/// can have, no answer the key writes appears in it (as "= answer"), no kind label or asymptote equation the teacher has
/// not kept, and no L beside a limit it asks for.
#[test]
fn no_student_copy_gives_an_answer_away() {
    let fns = [
        json!({}),
        json!({"pieces": "y = (x^2 - 4)/(x - 2)", "dots": "(2, 1)", "ask_at": "2"}),
        json!({"pieces": "y = x + 2, x < 1\ny = 4 - 2x, x >= 1", "dots": "", "ask_at": "1"}),
        json!({"pieces": "y = (2x^2 + 1)/(x^2 + 1)", "dots": "", "ask_at": "0"}),
        json!({"pieces": "y = sin(x)/x", "dots": "", "ask_at": "0", "eps": "1/4"}),
        json!({"pieces": "y = (exp(x) - exp(3))/(x - 3)", "dots": "", "ask_at": "3", "ymax": 30.0}),
    ];
    let mut n = 0;
    for f in &fns {
        for mask in 0..64u32 {
            let opt = |i: u32| mask & (1 << i) != 0;
            let o = json!({"ask_disc": opt(0), "ask_inf": if opt(1) { "Both" } else { "None" }, "kind_labels": opt(2), "asym_eqs": opt(3), "eps_band": opt(4), "ask_two": !opt(5)});
            let mut kv = f.as_object().unwrap().clone();
            for (k, v) in o.as_object().unwrap() { kv.insert(k.clone(), v.clone()); }
            let key = with(J::Object(kv.clone()));
            let mut stu = key.clone();
            stu.set_b("show_answers", false);
            let (ks, ss) = (svg(&key), svg(&stu));
            let block = |s: &str| { let i = s.find(r#"data-lim="block""#).unwrap(); words(&s[i + s[i..].find('>').unwrap() + 1..]) };
            let (kb, sb) = (block(&ks), block(&ss));
            // every "= answer" of the key, missing from the student copy (whose prompts end in "=")
            for part in kb.split(" = ").skip(1) {
                let ans: String = part.split(" lim ").next().unwrap().split(" f(").next().unwrap().trim().to_string();
                if ans.is_empty() || ans.starts_with("lim") { continue; }
                assert!(!sb.contains(&format!("= {ans}")), "{f} {o}: the student copy writes = {ans}: {sb}");
            }
            assert!(!sb.contains("nowhere") && !sb.contains("(removable)") && !sb.contains("(jump)") && !sb.contains("(infinite)"), "{f} {o}: {sb}");
            let labels = { let i = ss.find(r#"data-lim="labels""#).unwrap(); let i = i + ss[i..].find('>').unwrap() + 1; words(&ss[i..ss[i..].find("</g>").unwrap() + i]) };
            assert!(labels.trim().is_empty(), "{f} {o}: labels on the student copy: {labels}");
            // the ε band and its L only where the limit at a is not asked
            if opt(4) && (!opt(5) || stu.b("ask_left") || stu.b("ask_right")) { assert!(!sb.contains("L = "), "{f} {o}: {sb}"); }
            // and nothing in the SVG's own title or description
            let annotated = svg(&stu);
            assert!(!annotated[..annotated.find("</desc>").unwrap()].contains(" = "), "{f}");
            n += 1;
        }
    }
    assert_eq!(n, 6 * 64);
}

#[test]
fn an_empty_function_asks_nothing_and_says_why() {
    let p = with(json!({"pieces": "", "dots": "", "ask_at": "1", "ask_inf": "Both", "ask_disc": true}));
    let w = words(&svg(&p));
    assert!(!w.contains("does not exist") && !w.contains("undefined") && !w.contains("lim "), "{w}");
    assert!(warnings(&p).iter().any(|n| n.starts_with("Type the function in Function")), "{:?}", warnings(&p));
}

/// δ for (x² - 4)/(x - 2) = x + 2 at 2 with ε = 1/2: |x + 2 - 4| < 1/2 exactly when |x - 2| < 1/2, so δ = 0.5; for x² at 1 with
/// ε = 1: |x² - 1| < 1 for 0 < x < √2, so δ = √2 - 1 = 0.414…, 0.41 to the hundredth below.
#[test]
fn the_widest_delta_worked_by_hand() {
    let w = words(&svg(&with(json!({"pieces": "y = (x^2 - 4)/(x - 2)", "dots": "", "ask_at": "2", "eps_band": true, "eps": "1/2", "xmin": -4.0, "xmax": 6.0}))));
    assert!(w.contains("δ = 0.5"), "{w}");
    let w = words(&svg(&with(json!({"pieces": "y = x^2", "dots": "", "ask_at": "1", "eps_band": true, "eps": "1"}))));
    assert!(w.contains("δ = 0.41"), "{w}");
}

#[test]
#[ignore]
fn font_probe() {
    let db = crate::figures::fontdb();
    for f in db.faces() {
        let fam = f.families.first().map(|x| x.0.clone()).unwrap_or_default();
        if !(fam.contains("Liberation Serif") || fam.contains("DejaVu Serif") || fam.contains("Source Serif")) { continue; }
        let has: Vec<String> = ['ε', 'δ', '≈', '≠', '→', '∞', 'π', '√', '−', '∛'].iter().map(|c| {
            let ok = db.with_face_data(f.id, |data, i| ttf_parser::Face::parse(data, i).ok().and_then(|ff| ff.glyph_index(*c)).is_some()).unwrap_or(false);
            format!("{c}{}", if ok { "+" } else { "-" })
        }).collect();
        println!("{fam} {:?}: {}", f.style, has.join(" "));
    }
}

#[test]
#[ignore]
fn width_probe() {
    let mut m = Meter::new();
    for t in ["lim", "x\u{2192}1^(-)", " f(x) = ", "does not exist", "undefined", "Where is f not continuous? Give each x and its kind:", "x \u{2248} -4.712 (cannot be determined)", "\u{3b5} = 1/2, L = 1, a = -2: f stays within \u{3b5} of L for every x \u{2260} a within \u{3b4} of a", "removable", "jump", "infinite", "x = 4", "y = -\u{3c0}/2", "4 + e^3", "On the graph: x = 8 (jump); x = 9 (jump)"] {
        let el = super::limits::text_probe(t, 15.0);
        let ink = m.ink(&el).map(|b| b[2] - b[0]).unwrap_or(0.0);
        println!("{t:60} est {:6.1} ink {:6.1} ratio {:.3}", super::limits::tw(t, 15.0), ink, ink / super::limits::tw(t, 15.0));
    }
}

/// A floor of a curve is drawn, at every point inside a stretch, at the value it has there (worked by hand: floor(sin x) is 0
/// on (-2π, -π) since sin x > 0 there, floor(cos x) is 0 on (-π/2, π/2) away from 0, …).
#[test]
fn a_floor_of_a_curve_is_drawn_at_its_value_inside_every_stretch() {
    let cases: [(&str, fn(f64) -> f64, [f64; 4]); 4] = [
        ("y = floor(sin(x))", |x| x.sin().floor(), [-6.0, 10.0, -6.0, 7.0]),
        ("y = floor(cos(x))", |x| x.cos().floor(), [-6.0, 10.0, -6.0, 7.0]),
        ("y = floor(2sin(x))", |x| (2.0 * x.sin()).floor(), [-4.0, 4.0, -3.0, 3.0]),
        ("y = ceil(cos(x))", |x| x.cos().ceil(), [-4.0, 4.0, -2.0, 2.0]),
    ];
    for (e, f, win) in cases {
        let p = with(json!({"pieces": e, "dots": "", "ask_at": "", "xmin": win[0], "xmax": win[1], "ymin": win[2], "ymax": win[3]}));
        let s = svg(&p);
        let (x0, _, y0, _, ml, mt, ux, uy) = last_drawn(&p).frame;
        let ph = (win[3] - win[2]) * uy;
        let pts: Vec<Pt> = { let g = group(&s, "curves"); g.split("points=\"").skip(1).flat_map(|t| t.split('"').next().unwrap().split(' ').filter_map(|q| { let (a, b) = q.split_once(',')?; Some((a.parse::<f64>().ok()?, b.parse::<f64>().ok()?)) }).collect::<Vec<_>>()).collect() };
        let mut checked = 0;
        for i in 1..400 {
            let x = win[0] + (win[1] - win[0]) * i as f64 / 400.0;
            // (away from where the value changes)
            if [-0.04, 0.04].iter().any(|d| f(x + d) != f(x)) { continue; }
            let px = ml + (x - x0) * ux;
            let want = mt + ph - (f(x) - y0) * uy;
            let near: Vec<&Pt> = pts.iter().filter(|q| (q.0 - px).abs() < 1.0).collect();
            if near.is_empty() { continue; }
            assert!(near.iter().all(|q| (q.1 - want).abs() < 1.5), "{e}: at x = {x:.3} drawn at {:?}, true value {}", near.iter().map(|q| y0 + (mt + ph - q.1) / uy).collect::<Vec<_>>(), f(x));
            checked += 1;
        }
        assert!(checked > 100, "{e}: {checked}");
        // and the single values: floor(sin x) = 1 at π/2 gets its filled dot over an open circle at 0
        if e == "y = floor(sin(x))" {
            let (cx, cy0, cy1) = (ml + (std::f64::consts::FRAC_PI_2 - x0) * ux, mt + ph - (0.0 - y0) * uy, mt + ph - (1.0 - y0) * uy);
            let marks = group(&s, "marks");
            assert!(marks.contains(&format!(r#"cx="{cx:.2}" cy="{cy1:.2}""#)) && marks.contains(&format!(r#"cx="{cx:.2}" cy="{cy0:.2}""#)), "{marks}");
        }
        // no false note that the piece is outside the window
        assert!(!warnings(&p).iter().any(|n| n.contains("is outside the window")), "{e}: {:?}", warnings(&p));
    }
    for (e, win) in [("y = floor(x) + 1", [-6.0, 10.0, -6.0, 7.0]), ("y = floor(x)", [-6.0, 10.0, -6.0, 7.0]), ("y = x floor(x)", [-3.0, 3.0, -3.0, 3.0]), ("y = floor(x^2)", [-3.0, 3.0, -3.0, 3.0])] {
        let p = with(json!({"pieces": e, "dots": "", "xmin": win[0], "xmax": win[1], "ymin": win[2], "ymax": win[3]}));
        assert!(!warnings(&p).iter().any(|n| n.contains("is outside the window")), "{e}: {:?}", warnings(&p));
    }
}

/// δ worked by hand: y = 10x at 1 with ε = 0.05 needs |10x - 10| < 0.05, |x - 1| < 0.005 (0.01 would fail at x = 1.009);
/// y = 3 with a hole at 1, and (x - 3)/(x - 3), stay within any ε of their limit everywhere: there is no widest δ.
#[test]
fn delta_is_written_only_as_it_is() {
    let w = words(&svg(&with(json!({"pieces": "y = 10x", "dots": "", "ask_at": "1", "eps_band": true, "eps": "0.05"}))));
    assert!(w.contains("δ = 0.005") && !w.contains("δ = 0.01"), "{w}");
    for kv in [json!({"pieces": "y = 3, x != 1", "dots": "", "ask_at": "1", "eps_band": true, "eps": "1/2"}), json!({"pieces": "y = (x - 3)/(x - 3)", "dots": "", "ask_at": "3", "eps_band": true, "eps": "1/2", "xmin": -2.0, "xmax": 8.0})] {
        let w = words(&svg(&with(kv.clone())));
        assert!(w.contains("so any δ &gt; 0 works") && !w.contains("δ = 16") && !w.contains("δ = 10"), "{kv}: {w}");
    }
    // an empty function with the band on: one note only
    let n = warnings(&with(json!({"pieces": "", "dots": "", "ask_at": "1", "eps_band": true})));
    assert_eq!(n.len(), 1, "{n:?}");
}

/// Exact values are written as a textbook writes them: quotients as fractions, sin/cos as tan, and true minus signs
/// (the third check: "(cos 1)⁻¹ sin 1", "(sin 1)⁻¹", "π⁻¹", "2π⁻¹", "x = -ln 20").
#[test]
fn exact_values_are_written_in_textbook_form() {
    let show = |t: &str| super::limits::Num::read(t).unwrap().text;
    for (typed, want) in [("1/pi", "1/\u{3c0}"), ("2/pi", "2/\u{3c0}"), ("1/ln(3)", "1/ln 3"), ("1/sin(1)", "1/sin 1"), ("tan(1)", "tan 1"), ("1/tan(1)", "cot 1"), ("2tan(1)", "2 tan 1"), ("-pi/2", "-\u{3c0}/2"), ("exp(-1)", "1/e"), ("3/e^2", "3/e^2"), ("-2 + 1/pi", "-2 + 1/\u{3c0}"), ("ln(2)", "ln 2"), ("6 + cos(1)", "6 + cos 1")] {
        assert_eq!(show(typed), want, "{typed}");
    }
    let md = |pieces: &str, at: &str| { let m = m(serde_json::json!({"pieces": pieces, "dots": "", "ask_at": at})); let k = super::limits::Num::read(at).unwrap().k; m.two(&k) };
    assert!(matches!(md("y = tan(x)", "1"), Lim::Val(_, t) if t == "tan 1"));
    assert!(matches!(md("y = 1/tan(x)", "1"), Lim::Val(_, t) if t == "cot 1"));
    assert!(matches!(md("y = 1/sin(x)", "1"), Lim::Val(_, t) if t == "1/sin 1"));
    assert!(matches!(md("y = 1/ln(x)", "3"), Lim::Val(_, t) if t == "1/ln 3"));
    // the key and the graph write them so, with no ASCII hyphen before a word
    for (pieces, x0, x1, want) in [("y = floor(exp(-x))", -3.5, 1.0, "\u{2212}ln 20"), ("y = 1/(x + e)", -4.0, 4.0, "x = \u{2212}e")] {
        let svg = svg(&with(serde_json::json!({"pieces": pieces, "dots": "", "ask_at": "", "ask_disc": true, "asym_eqs": true, "xmin": x0, "xmax": x1})));
        let text: String = { let mut out = String::new(); let mut inside = false; for c in svg.chars() { if c == '<' { inside = true; } else if c == '>' { inside = false; } else if !inside { out.push(c); } } out };
        assert!(text.contains(want), "{pieces}: {want} not in {text}");
        assert!(!text.contains(" -ln") && !text.contains("= -e"), "{pieces}: a hyphen for a minus sign: {text}");
    }
    let a = svg(&with(serde_json::json!({"pieces": "y = 1/(x - 1/pi)", "dots": "", "ask_at": "1/pi", "asym_eqs": true, "xmin": -2.0, "xmax": 2.0})));
    assert!(!a.contains("\u{207b}") && !a.contains("^(-1)"), "a negative power is written: {a}");
}

/// The third check's slow graphs (thousands of poles, or poles piling up at 0) draw in well under half a second: past a
/// few hundred places the key says there are too many, and poles piling up are said to pile up.
#[test]
fn graphs_with_thousands_of_poles_draw_quickly_and_say_so() {
    for (pieces, x0, x1, says) in [
        ("y = tan(x^2)", -100.0, 100.0, "too many to work out one by one"),
        ("y = tan(x^3)", -10.0, 10.0, "too many to work out one by one"),
        ("y = 1/sin(x^2)", -100.0, 100.0, "too many to work out one by one"),
        ("y = tan(x^2)", -20.0, 20.0, "too many to work out one by one"),
        ("y = 1/sin(1/x)", -1.0, 1.0, "pile up without end near x = 0"),
        // (the fourth check's: thousands of cuts, counted cheaply before any is solved for)
        ("y = floor(10sin(x^2))", -6.0, 10.0, "too many to work out one by one"),
        ("y = floor(10sin(x^2))", -10.0, 10.0, "too many to work out one by one"),
        ("y = floor(10sin(x^2))", -100.0, 100.0, "too many to work out one by one"),
        ("y = tan(exp(x))", -100.0, 100.0, "too many to work out one by one"),
        ("y = 1/tan(x^2)", -10.0, 10.0, "line to fill in"),
        ("y = 1/tan(x^2)", -6.0, 10.0, "line to fill in"),
        ("y = floor(sin(1/x))", -1.0, 1.0, "pile up without end near x = 0"),
        // (a pile at every pole: the wider searches for the outside note are skipped)
        ("y = floor(tan(x))", -6.0, 10.0, "pile up without end near x = \u{2212}3\u{3c0}/2"),
    ] {
        let p = with(json!({"pieces": pieces, "dots": "", "xmin": x0, "xmax": x1, "ask_disc": true, "kind_labels": true}));
        // (the fastest of three fresh builds, each with its own title so none comes from the cache: the whole suite runs
        // alongside and slows any one measurement)
        let ms = (0..3).map(|i| { let q = with(json!({"pieces": pieces, "dots": "", "xmin": x0, "xmax": x1, "ask_disc": true, "kind_labels": true, "title": format!("timing {i}")})); let t0 = thread_cpu(); let _ = svg(&q); ((thread_cpu() - t0) * 1000.0) as u128 }).min().unwrap();
        let _ = svg(&p);
        assert!(ms < 500, "{pieces} on {x0}..{x1} took {ms} ms");
        let w = warnings(&p);
        assert!(w.iter().any(|n| n.contains(says)), "{pieces}: {w:?}");
    }
}

/// 0^0 has no value: x^x, |x|^x, (x²)^x have no value at 0 and the limit from the side with values is 1, drawn as an
/// open circle at (0, 1) (never a filled one); 0^x is 0 for x > 0 only; x² at 0 is 0.
#[test]
fn zero_to_the_zero_has_no_value_and_is_drawn_open() {
    for pieces in ["y = x^x", "y = abs(x)^x", "y = (x^2)^x", "y = sin(x)^x"] {
        let p = with(json!({"pieces": pieces, "dots": "", "ask_at": "0"}));
        let md = model_of(&p);
        let z = Num::read("0").unwrap().k;
        assert_eq!(md.value(&z), Lim::Undef, "{pieces}");
        assert!(matches!(md.side(&z, 1.0), Lim::Val(v, _) if (v - 1.0).abs() < 1e-12), "{pieces}");
        let s = svg(&p);
        let circles: Vec<&str> = s.split("<circle").skip(1).filter(|c| c.contains("data-lim")).collect();
        // (sin(x)^x at π is 0^π = 0, a value: its only open circle is at (0, 1))
        let opens: Vec<&&str> = circles.iter().filter(|c| c.contains("data-lim=\"open\"")).collect();
        assert_eq!(opens.len(), 1, "{pieces}: open circles: {circles:?}");
        assert!(opens[0].contains("cx=\"218.80\" cy=\"236.20\""), "{pieces}: the open circle is at (0, 1): {circles:?}");
        assert!(!circles.iter().any(|c| c.contains("data-lim=\"closed\"")), "{pieces}: no filled circle: {circles:?}");
    }
    let md = model_of(&with(json!({"pieces": "y = 0^x", "dots": ""})));
    assert_eq!(md.value(&Num::read("0").unwrap().k), Lim::Undef);
    assert!(matches!(md.value(&Num::read("2").unwrap().k), Lim::Val(v, _) if v == 0.0));
    assert_eq!(md.value(&Num::read("-1").unwrap().k), Lim::Undef);
    let md = model_of(&with(json!({"pieces": "y = x^2", "dots": ""})));
    assert!(matches!(md.value(&Num::read("0").unwrap().k), Lim::Val(v, _) if v == 0.0));
}

#[test]
#[ignore]
fn time_probe() {
    let p = with(serde_json::from_str(&std::env::var("LIM_JSON").unwrap()).unwrap());
    let t = std::time::Instant::now(); let md = model_of(&p); println!("model {:?}", t.elapsed());
    if let super::limits::Src::Formula { x } = &md.pieces[0].src {
        let t = std::time::Instant::now(); let b = super::limits::cas::busy(x, md.x0, md.x1); println!("busy {:?} {b}", t.elapsed());
        let t = std::time::Instant::now(); let (pp, all) = super::limits::cas::piles_in(x, md.x0, md.x1); println!("piles_in {:?} {} all {all} work {}", t.elapsed(), pp.len(), super::limits::cas::work());
        let t = std::time::Instant::now(); let n = super::limits::cas::floors_inert(x, md.x0, md.x1).is_some(); println!("inert {:?} {n}", t.elapsed());
    }
    let t = std::time::Instant::now(); let pl0 = md.piles(); println!("piles first {:?} ({:?}) work {}", t.elapsed(), pl0.iter().map(|k| k.show()).collect::<Vec<_>>(), super::limits::cas::work());
    let t = std::time::Instant::now(); let d = md.discontinuities(); println!("discs {:?} ({})", t.elapsed(), d.0.len());
    let t = std::time::Instant::now(); let o = md.outside(); println!("outside {:?} ({})", t.elapsed(), o.0.len());
    let t = std::time::Instant::now(); let v = md.verticals(); println!("verticals {:?} ({})", t.elapsed(), v.len());
    let t = std::time::Instant::now(); let pl = md.piles(); println!("piles {:?} ({:?})", t.elapsed(), pl.iter().map(|k| k.show()).collect::<Vec<_>>());
    for pc in &md.pieces { if let super::limits::Src::Formula { x } = &pc.src { for pl in super::limits::cas::piles_in(x, md.x0, md.x1).0 {
        println!("pile at {} arg {:?} level {:?}", pl.at.show(), pl.arg, pl.level);
        let w = md.x1 - md.x0;
        for j in 2..=4 { let h = w * 10f64.powi(-j); let (u, v) = (pl.at.value() + h, pl.at.value() + 3.0 * h);
            let vals: Vec<f64> = (0..=200).filter_map(|i| super::limits::cas::ev(&pl.arg, u + (v - u) * i as f64 / 200.0)).collect();
            let (mn, mx) = vals.iter().fold((f64::MAX, f64::MIN), |(p, q), y| (p.min(*y), q.max(*y)));
            let lev = pl.level_near((mn + mx) / 2.0);
            let (z, d) = super::limits::cas::zeros(&super::limits::cas::X::Add(vec![pl.arg.clone(), super::limits::cas::X::C(lev.neg())]), u, v);
            println!("band {u}..{v} range {mn}..{mx} level {} zeros {:?} {d}", lev.show(), z.iter().map(|k| (k.show(), k.is_exact(), md.side(k, -1.0), md.side(k, 1.0))).collect::<Vec<_>>()); }
    } } }
    let (ds, done) = md.discontinuities(); println!("complete {done}: {}", ds.iter().take(12).map(|d| format!("{} {}", d.text, d.kind)).collect::<Vec<_>>().join(", "));
    println!("work so far {}", super::limits::cas::work());
    let t = std::time::Instant::now(); let _ = svg(&p); println!("svg {:?} work {}", t.elapsed(), super::limits::cas::work());
}

/// The third check's labels that went under the graph with room on it: the hole of (x² - 1)/((x - 1)(x - 3)) at (1, -1)
/// is labelled on a leader across the dashed x = 3 (the 55 px between the hole and x = 3 is narrower than "removable",
/// and the axis numbers fill the band above it); the staircase's jump at x = 1 sits over its filled end (1, 3), 12 px
/// from the dashed x = 2 (a label centred across its own gap needs only a plain margin from the next thing); 1/tan x
/// labels its holes at π/2 and 3π/2 on the graph (those at -π/2 and -3π/2 have no room: the y-axis numbers and the
/// curve leave 66 px for a 76 px label).
#[test]
fn labels_with_room_on_the_graph_are_drawn_there() {
    let on_graph = |p: &Params, x: &str| -> bool {
        let _ = svg(p);
        let d = last_drawn(p);
        let xs = super::limits::Num::read(x).unwrap().v;
        let m = model_of(p);
        let px = |v: f64| { let (ds, _) = m.discontinuities(); ds.iter().any(|dd| (dd.x - v).abs() < 1e-9) };
        assert!(px(xs), "{x} is a discontinuity");
        d.labels.iter().any(|(k, _, spot, own)| *k == 'k' && !matches!(spot, Spot::Off) && own.iter().any(|(a, b)| (a.0 - b.0).abs() < 0.5 && { let (x0, _, _, _, ml, _, ux, _) = d.frame; (x0 + (a.0 - ml) / ux - xs).abs() < 0.05 }))
    };
    let r03 = with(json!({"pieces": "y = (x^2 - 1)/((x - 1)(x - 3))", "dots": "", "ask_at": "1, 3", "ask_inf": "Both", "ask_disc": true, "asym_eqs": true, "kind_labels": true}));
    assert!(on_graph(&r03, "1"), "R03: the removable label at x = 1 is on the graph: {:?}", warnings(&r03));
    assert!(!warnings(&r03).iter().any(|w| w.contains("under the graph")), "{:?}", warnings(&r03));
    let stair = with(json!({"pieces": "y = 1, x < 0\ny = 2, 0 <= x < 1\ny = 3, 1 <= x < 2\ny = 1/(x - 2), x > 2", "label_size": "Medium", "kind_labels": true}));
    assert!(on_graph(&stair, "1"), "the staircase's jump at x = 1 is on the graph: {:?}", warnings(&stair));
    assert!(!warnings(&stair).iter().any(|w| w.contains("under the graph")), "{:?}", warnings(&stair));
    let cot = with(json!({"pieces": "y = 1/tan(x)", "dots": "", "kind_labels": true, "ask_disc": true}));
    for x in ["pi/2", "3pi/2"] { assert!(on_graph(&cot, x), "1/tan x: the hole at {x} is labelled on the graph: {:?}", warnings(&cot)); }
}

/// One rule for every power, the same in the key and on the graph: a base of 0 has a value only to a positive power
/// (0^p = 0 for p > 0; 0^0 and 0 to a negative power have none, whether the power is a number or has x in it), and a
/// negative base to a power with x in it has none. So x^0 is 1 except at 0, where it has no value (a removable
/// discontinuity, drawn as a hole); (x - 2)^x at 2 is 0^2 = 0; x^x at 0 is 0^0, no value; √x and x^(3/2) at 0 are 0.
#[test]
fn powers_of_zero_follow_one_rule() {
    // (function, a, f(a), left limit, right limit, the discontinuity at a or "")
    let table: &[(&str, &str, &str, &str, &str, &str)] = &[
        ("y = x^0", "0", "undef", "1", "1", "removable"),
        ("y = (x - 2)^0", "2", "undef", "1", "1", "removable"),
        ("y = (x^2)^0", "0", "undef", "1", "1", "removable"),
        ("y = x^0", "3", "1", "1", "1", ""),
        ("y = x^x", "0", "undef", "DNE", "1", ""),
        ("y = abs(x)^x", "0", "undef", "1", "1", "removable"),
        ("y = 0^x", "0", "undef", "DNE", "0", ""),
        ("y = 0^x", "1", "0", "0", "0", ""),
        ("y = x^(-1)", "0", "undef", "-∞", "∞", "infinite"),
        ("y = (x - 2)^x", "2", "0", "DNE", "0", ""),
        ("y = x^(1/(1 - x))", "0", "0", "DNE", "0", ""),
        ("y = (1 - 1/x)^x", "1", "0", "DNE", "0", ""),
        ("y = cos(x)^(1/x)", "pi/2", "0", "0", "DNE", ""),
        ("y = sqrt(x)", "0", "0", "DNE", "0", ""),
        ("y = x^(3/2)", "0", "0", "DNE", "0", ""),
        ("y = x^2", "0", "0", "0", "0", ""),
        ("y = x^3", "0", "0", "0", "0", ""),
        ("y = sin(x)^x", "pi", "0", "0", "DNE", ""),
    ];
    let key = |l: &Lim| match l { Lim::Val(_, t) => t.clone(), Lim::PInf => "∞".into(), Lim::NInf => "-∞".into(), Lim::Undef => "undef".into(), Lim::Dne => "DNE".into(), Lim::Unknown => "?".into(), Lim::Two => "two".into() };
    let side = |l: &Lim| if *l == Lim::Undef { "DNE".to_string() } else { key(l) };
    let mut fails = vec![];
    for (f, a, fa, l, r, kind) in table {
        let p = with(json!({"pieces": f, "dots": "", "ask_at": a, "ask_disc": true}));
        let md = model_of(&p);
        let k = Num::read(a).unwrap().k;
        let got = (key(&md.value(&k)), side(&md.side(&k, -1.0)), side(&md.side(&k, 1.0)));
        if got != (fa.to_string(), l.to_string(), r.to_string()) { fails.push(format!("{f} at {a}: got {got:?}, want ({fa}, {l}, {r})")); }
        let (ds, _) = md.discontinuities();
        let at = ds.iter().find(|d| (d.x - k.value()).abs() < 1e-9).map_or("", |d| d.kind);
        if at != *kind { fails.push(format!("{f} at {a}: discontinuity {at:?}, want {kind:?}")); }
        // the key and the graph agree: a hole is drawn exactly where f(a) has no value but a two-sided limit
        let s = svg(&p);
        let holes = s.matches("data-lim=\"open\"").count();
        if *kind == "removable" && holes == 0 { fails.push(format!("{f}: no hole drawn at {a}")); }
        if let Lim::Val(y, _) = md.value(&k) {
            let d = last_drawn(&p);
            let (x0, _, _, y1, ml, mt, ux, uy) = d.frame;
            let (cx, cy) = (ml + (k.value() - x0) * ux, mt + (y1 - y) * uy);
            if d.circles.iter().any(|c| !c.closed && (c.at.0 - cx).abs() < 1.0 && (c.at.1 - cy).abs() < 1.0) { fails.push(format!("{f}: an open circle at ({a}, {fa}), where f has that value")); }
        }
    }
    // a typed domain that includes 0 for x^0: the formula has no value there, so the end is open and a note says so
    let p = with(json!({"pieces": "y = x^0, x >= 0", "dots": "", "ask_at": "0"}));
    assert_eq!(model_of(&p).value(&Num::read("0").unwrap().k), Lim::Undef);
    assert!(warnings(&p).iter().any(|w| w.contains("no value there")), "{:?}", warnings(&p));
    // 0^0 has no value anywhere: nothing is drawn, and a note says why
    let p = with(json!({"pieces": "y = 0^0", "dots": "", "ask_at": "1"}));
    assert_eq!(model_of(&p).value(&Num::read("1").unwrap().k), Lim::Undef);
    assert!(warnings(&p).iter().any(|w| w.contains("no value anywhere")), "{:?}", warnings(&p));
    assert!(!svg(&p).contains("polyline"), "0^0 is drawn");
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

fn plain_text(svg: &str) -> String { let mut out = String::new(); let mut inside = false; for c in svg.chars() { if c == '<' { inside = true; } else if c == '>' { inside = false; } else if !inside { out.push(c); } } out }

/// "Infinitely many" is a claim and is made only when established. A floor whose value does not matter (a constant in
/// disguise built on an accumulating floor) has cuts that are no discontinuities, so its key never claims infinitely
/// many: it gives the complete list when that is established, and otherwise leaves the line blank; its limits are right.
#[test]
fn constants_in_disguise_claim_no_pile_of_discontinuities() {
    // (function, f(1/2) and its limit at 1/2)
    let cases: &[(&str, &str)] = &[
        ("y = sin(pi floor(1/x))", "0"), ("y = cos(2pi floor(1/x))", "1"), ("y = abs(cos(pi floor(1/x)))", "1"),
        ("y = floor(1/x) - floor(1/x)", "0"), ("y = sin(pi ceil(1/x))", "0"), ("y = x + sin(pi floor(1/x))", "1/2"),
        ("y = floor(1/x) + ceil(-1/x)", "0"), ("y = floor(1/x)^2 - floor(1/x)^2", "0"), ("y = sin(pi floor(ln(x)))", "0"),
        ("y = sin(pi floor(tan(x)))", "0"), ("y = floor(1/x)/floor(1/x)", "1"), ("y = sin(2pi floor(1/x))", "0"),
        ("y = cos(2pi ceil(1/x))", "1"), ("y = sin(pi floor(1/x))^2", "0"), ("y = abs(sin(pi floor(-ln(x))))", "0"),
        ("y = sin(pi floor(x^(-2)))", "0"), ("y = cos(2pi floor(1/sin(x)))", "1"), ("y = sin(pi floor(exp(1/x)))", "0"),
        ("y = x^2 + sin(pi floor(1/x))", "1/4"), ("y = floor(2/x) - floor(2/x)", "0"), ("y = sin(3pi floor(1/x))", "0"),
        ("y = 1 + sin(pi floor(1/(x - 1)))", "1"), ("y = cos(2pi floor(ln(abs(x))))", "1"), ("y = abs(cos(pi floor(tan(x))))", "1"),
        ("y = sin(pi floor(1/x)) + cos(2pi floor(1/x))", "1"), ("y = exp(sin(pi floor(1/x)))", "1"),
    ];
    let mut fails = vec![];
    for (f, want) in cases {
        let p = with(json!({"pieces": f, "dots": "", "ask_at": "1/2", "ask_disc": true, "kind_labels": true}));
        let t = plain_text(&svg(&p));
        if t.contains("infinitely many") { fails.push(format!("{f}: claims infinitely many: {t}")); }
        let md = model_of(&p);
        let k = Num::read("1/2").unwrap().k;
        for (what, l) in [("limit", md.two(&k)), ("value", md.value(&k))] { if !matches!(&l, Lim::Val(_, t) if t == want) { fails.push(format!("{f}: {what} at 1/2 is {l:?}, want {want}")); } }
        // what is listed is never a cut of the floor: only removable points where f has no value, or poles
        let (ds, _) = md.discontinuities();
        if ds.iter().any(|d| d.kind == "jump") { fails.push(format!("{f}: a jump listed: {:?}", ds.iter().map(|d| (&d.text, d.kind)).collect::<Vec<_>>())); }
    }
    // the first one, worked through: only the removable point at 0, and the list is complete
    let md = model_of(&with(json!({"pieces": "y = sin(pi floor(1/x))", "dots": ""})));
    let (ds, done) = md.discontinuities();
    assert!(done && ds.len() == 1 && ds[0].text == "0" && ds[0].kind == "removable", "{:?} {done}", ds.iter().map(|d| (&d.text, d.kind)).collect::<Vec<_>>());
    let md = model_of(&with(json!({"pieces": "y = sin(pi floor(tan(x)))", "dots": ""})));
    let (ds, done) = md.discontinuities();
    assert!(done && ds.iter().all(|d| d.kind == "removable") && ds.len() == 5, "{:?} {done}", ds.iter().map(|d| (&d.text, d.kind)).collect::<Vec<_>>());
    assert!(fails.is_empty(), "{} wrong:\n{}", fails.len(), fails.join("\n"));
}

/// And where f does jump at cuts that pile up without end, the key still says so (established in three bands).
#[test]
fn true_piles_of_discontinuities_are_still_said() {
    let cases: &[(&str, f64, f64, &str)] = &[
        ("y = floor(1/x)", -6.0, 10.0, "0"), ("y = floor(ln(x))", 0.0, 10.0, "0"), ("y = 1/sin(1/x)", -1.0, 1.0, "0"),
        ("y = x floor(1/x)", -2.0, 2.0, "0"), ("y = ceil(1/x)", -3.0, 3.0, "0"), ("y = floor(-ln(x))", 0.0, 10.0, "0"),
        ("y = floor(2/x) + x", -3.0, 3.0, "0"), ("y = tan(1/x)", -1.0, 1.0, "0"), ("y = floor(exp(1/x))", -2.0, 2.0, "0"),
        ("y = 1/cos(1/x)", -1.0, 1.0, "0"), ("y = floor(ln(x - 2))", -6.0, 10.0, "2"),
    ];
    let mut fails = vec![];
    for (f, x0, x1, at) in cases {
        let p = with(json!({"pieces": f, "dots": "", "ask_at": "", "ask_disc": true, "xmin": x0, "xmax": x1}));
        let t = plain_text(&svg(&p));
        // (the key may wrap between the two parts, and the comma goes with the wrap)
        if !(t.contains("infinitely many") && t.contains(&format!("piling up near x = {at}"))) { fails.push(format!("{f}: {}", &t[t.find("Where is").unwrap_or(0)..])); }
    }
    assert!(fails.is_empty(), "{} wrong:\n{}", fails.len(), fails.join("\n"));
}

/// The note on discontinuities outside the window names the nearest ones and never counts what it did not enumerate
/// (floor(x) and tan x have infinitely many; floor(ln x) on 2..10 has e^4 next after e^3, and infinitely many near 0).
#[test]
fn outside_notes_name_the_nearest_and_never_count() {
    let note = |pieces: &str, x0: f64, x1: f64| -> String { warnings(&with(json!({"pieces": pieces, "dots": "", "ask_at": "", "xmin": x0, "xmax": x1}))).into_iter().find(|w| w.contains("outside the window: x")).unwrap_or_default() };
    let counted = regex_free_count;
    for (f, x0, x1, names) in [
        ("y = floor(x)", -6.0, 10.0, vec!["x = \u{2212}8 (jump), x = \u{2212}7 (jump), x = 11 (jump), x = 12 (jump) and more"]),
        ("y = floor(ln(x))", 2.0, 10.0, vec!["x = 1/e (jump), x = 1 (jump), x = e^3 (jump), x = e^4 (jump) and more"]),
        ("y = floor(1/x)", 0.4, 10.0, vec!["x = 1/4 (jump), x = 1/3 (jump)", "and more"]),
        ("y = tan(x)", -6.0, 10.0, vec!["x = \u{2212}5\u{3c0}/2 (infinite)", "x = 7\u{3c0}/2 (infinite)", "and more"]),
    ] {
        let n = note(f, x0, x1);
        for want in names { assert!(n.contains(want), "{f}: {want:?} not in {n:?}"); }
        assert!(!counted(&n), "{f}: a count: {n}");
    }
}
/// " and 26 more": a number before "more".
fn regex_free_count(s: &str) -> bool { s.split(" more").next().is_some_and(|h| h.rsplit(' ').next().is_some_and(|w| w.chars().all(|c| c.is_ascii_digit()) && !w.is_empty())) }

/// The fourth check's small things: tan written as tan, a true minus sign at Small, no "nowhere" beside a point with
/// two values, the cuts of floor(5 sin x) each established, and a note that names a few places, not eighty.
#[test]
fn small_things_of_the_fourth_check() {
    let show = |t: &str| Num::read(t).unwrap().text;
    assert_eq!(show("tan(1/2)"), "tan(1/2)");
    assert_eq!(show("sin(1/8)/cos(1/8)"), "tan(1/8)");
    assert_eq!(show("1/(2pi)"), "1/(2\u{3c0})");
    let md = model_of(&with(json!({"pieces": "y = tan(x)", "dots": "", "ask_at": "1/2"})));
    assert!(matches!(md.two(&Num::read("1/2").unwrap().k), Lim::Val(_, t) if t == "tan(1/2)"));
    let t = plain_text(&svg(&with(json!({"pieces": "y = (x + 2)/(x^2 + 3x + 2)", "dots": "", "label_size": "Small", "ask_disc": true}))));
    assert!(t.contains("x = \u{2212}1 (infinite)") && !t.contains("= -"), "{t}");
    let p = with(json!({"pieces": "y = x, x <= 1\ny = 2, x >= 1", "dots": "", "ask_at": "1", "ask_disc": true}));
    let t = plain_text(&svg(&p));
    assert!(!t.contains("nowhere"), "{t}");
    assert!(warnings(&p).iter().any(|w| w.contains("two values at x = 1, so the key leaves")), "{:?}", warnings(&p));
    for f in ["y = floor(5sin(x))", "y = floor(3sin(x))"] {
        let p = with(json!({"pieces": f, "dots": "", "ask_disc": true}));
        let (ds, done) = model_of(&p).discontinuities();
        assert!(done && ds.len() > 20 && ds.iter().all(|d| d.kind == "jump" || d.kind == "removable"), "{f}: {done} {:?}", ds.iter().map(|d| (&d.text, d.kind)).collect::<Vec<_>>());
        assert!(!warnings(&p).iter().any(|w| w.contains("cannot")), "{:?}", warnings(&p));
    }
}

/// Every flat stroke of a step function is drawn at the function's own value inside it, on narrow and wide windows alike
/// (the fifth check: floor(4 - x²) was drawn at 4 on (-1, 1), where it is 3 except at the single point 0, because the
/// level of a stretch was read at its middle).
#[test]
fn every_stretch_of_a_step_function_is_drawn_at_its_own_value() {
    let fns = ["floor(4 - x^2)", "floor(1 - x^2)", "floor(-x^2)", "floor(2 - x^2/2)", "floor(6cos(x))", "floor(10sin(x))", "floor(sin(x))",
        "floor(cos(x))", "floor(2sin(x))", "ceil(sin(x))", "floor(sin(x)) + 2", "floor(3cos(x/2))", "floor(5 - abs(x))", "ceil(x^2/4)",
        "floor(x/3)", "floor(sqrt(abs(x)))", "floor(5sin(x))", "ceil(cos(x)) - 1", "floor(x)", "floor(2cos(x)) + floor(sin(x))"];
    let mut fails = vec![];
    let mut checked = 0;
    for f in fns {
        let e = super::limits::cas::parse(f).unwrap();
        for (x0, x1) in [(-6.0, 10.0), (-15.0, 15.0), (-100.0, 100.0)] {
            let p = with(json!({"pieces": format!("y = {f}"), "dots": "", "ask_at": "", "xmin": x0, "xmax": x1, "square": false}));
            let s = svg(&p);
            let (fx0, _, fy0, fy1, ml, mt, ux, uy) = last_drawn(&p).frame;
            let to_x = |px: f64| fx0 + (px - ml) / ux;
            let to_y = |py: f64| fy1 - (py - mt) / uy;
            let g = group(&s, "curves");
            for line in g.split("points=\"").skip(1) {
                let pts: Vec<(f64, f64)> = line.split('"').next().unwrap().split(' ').filter_map(|q| { let (a, b) = q.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
                // (runs of points at one height, 2 px long or more)
                let mut runs: Vec<((f64, f64), (f64, f64))> = vec![];
                let mut i = 0;
                while i < pts.len() {
                    let mut j = i;
                    while j + 1 < pts.len() && (pts[j + 1].1 - pts[i].1).abs() < 0.01 { j += 1; }
                    if pts[j].0 - pts[i].0 >= 2.0 { runs.push((pts[i], pts[j])); }
                    i = j + 1;
                }
                for (a, b) in runs {
                    let w = [a, b];
                    let (xa, xb) = (to_x(w[0].0), to_x(w[1].0));
                    let x = xa + (xb - xa) * 0.4137;
                    let Some(v) = super::limits::cas::ev(&e, x) else { continue };
                    let h = (x1 - x0) / 20000.0;
                    if [-h, h].iter().any(|d| super::limits::cas::ev(&e, x + d) != Some(v)) { continue; }
                    let drawn = to_y(w[0].1);
                    if drawn < fy0 || drawn > fy1 { continue; }
                    checked += 1;
                    if (drawn - v).abs() > 0.05 * (fy1 - fy0).max(1.0) / 10.0 + 0.02 { fails.push(format!("{f} on {x0}..{x1}: at x = {x:.4} drawn at {drawn:.3}, value {v}")); break; }
                }
            }
        }
    }
    assert!(checked > 500, "{checked} stretches checked");
    assert!(fails.is_empty(), "{} wrong:\n{}", fails.len(), fails.join("\n"));
}

/// b(x)^0 has no value wherever b(x) = 0 (0^0 has none), as b/b has none: every zero of the base is a removable point,
/// found by the same search, whether or not it is asked about, with a hole drawn; and a power the rule gives no value is
/// not drawn there (sin(x)^(x - x) where sin x < 0; 0^(x - 2) at and left of 2: no spike to (2, 1)).
#[test]
fn every_zero_of_a_base_to_the_power_zero_is_a_hole() {
    let pi = "\u{3c0}";
    let cases: Vec<(&str, &str, String)> = vec![
        ("y = (x - 1)^0 (x - 3)^0", "2", "1 removable, 3 removable".into()),
        ("y = (x - 2)^0", "", "2 removable".into()),
        ("y = sin(x)^0", "1", format!("-{pi} removable, 0 removable, {pi} removable, 2{pi} removable, 3{pi} removable")),
        ("y = cos(x)^0", "0", format!("-3{pi}/2 removable, -{pi}/2 removable, {pi}/2 removable, 3{pi}/2 removable, 5{pi}/2 removable")),
        ("y = tan(x)^0", "1", format!("-3{pi}/2 removable, -{pi} removable, -{pi}/2 removable, 0 removable, {pi}/2 removable, {pi} removable, 3{pi}/2 removable, 2{pi} removable, 5{pi}/2 removable, 3{pi} removable")),
        ("y = (x^2 - 2)^0 + sin(x)^0", "", format!("-{pi} removable, -\u{221a}2 removable, 0 removable, \u{221a}2 removable, {pi} removable, 2{pi} removable, 3{pi} removable")),
        ("y = sin(x)^0 + 1/x", "", format!("-{pi} removable, 0 infinite, {pi} removable, 2{pi} removable, 3{pi} removable")),
        ("y = sin(x)^0, x >= 0", "", format!("{pi} removable, 2{pi} removable, 3{pi} removable")),
        ("y = sin(pi x)^0", "1/2", (-6..=10).map(|k| format!("{k} removable")).collect::<Vec<_>>().join(", ")),
    ];
    let mut fails = vec![];
    for (f, at, want) in &cases {
        let p = with(json!({"pieces": f, "dots": "", "ask_at": at, "ask_disc": true}));
        let (ds, done) = model_of(&p).discontinuities();
        let got = ds.iter().map(|d| format!("{} {}", d.text, d.kind)).collect::<Vec<_>>().join(", ");
        if &got != want || !done { fails.push(format!("{f}: {got:?} (complete {done}), want {want:?}")); continue; }
        // a hole drawn at each removable point inside the window (the window's edges excepted)
        let _ = svg(&p);
        let opens = last_drawn(&p).circles.iter().filter(|c| !c.closed).count();
        let inside = ds.iter().filter(|d| d.kind == "removable" && d.x > -6.0 + 1e-9 && d.x < 10.0 - 1e-9).count();
        if opens < inside { fails.push(format!("{f}: {opens} open circles for {inside} removable points")); }
    }
    // sin(x)^(x - x): no values where sin x < 0, so nothing is drawn on (π, 2π); its key says nowhere (ends of the domain)
    let p = with(json!({"pieces": "y = sin(x)^(x - x)", "dots": "", "ask_at": "", "ask_disc": true}));
    let s = svg(&p);
    let (fx0, _, _, _, ml, _, ux, _) = last_drawn(&p).frame;
    let xs: Vec<f64> = group(&s, "curves").split("points=\"").skip(1).flat_map(|t| t.split('"').next().unwrap().split(' ').filter_map(|q| q.split_once(',').and_then(|(a, _)| a.parse::<f64>().ok())).collect::<Vec<_>>()).map(|px| fx0 + (px - ml) / ux).collect();
    if xs.iter().any(|x| *x > 3.2 && *x < 6.2) { fails.push("sin(x)^(x - x) is drawn where sin x < 0".into()); }
    // 0^(x - 2): no value at 2 or left of it: nothing drawn there, so no stroke up to (2, 1)
    let p = with(json!({"pieces": "y = 0^(x - 2)", "dots": "", "ask_at": "2, 3", "ask_inf": "Both", "ask_disc": true}));
    let s = svg(&p);
    let (fx0, _, _, fy1, ml, mt, ux, uy) = last_drawn(&p).frame;
    let pts: Vec<(f64, f64)> = group(&s, "curves").split("points=\"").skip(1).flat_map(|t| t.split('"').next().unwrap().split(' ').filter_map(|q| { let (a, b) = q.split_once(',')?; Some((fx0 + (a.parse::<f64>().ok()? - ml) / ux, fy1 - (b.parse::<f64>().ok()? - mt) / uy)) }).collect::<Vec<_>>()).collect();
    if pts.iter().any(|(x, y)| *x <= 2.0 + 1e-6 || *y > 0.01) { fails.push(format!("0^(x - 2) drawn at {:?}", pts.iter().filter(|(x, y)| *x <= 2.0 + 1e-6 || *y > 0.01).take(3).collect::<Vec<_>>())); }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

/// Sixty adversarial compositions of floor, ceil, tan and exp on three windows: each render finishes (a 5-second kill
/// timer) and takes under half a second (measured again once if the machine was busy); the analysis stops at its work
/// budget, counted in steps of exact arithmetic, and the key then leaves its lines to fill in.
#[test]
fn adversarial_compositions_finish_quickly() {
    let fns = [
        "floor(tan(x^2))", "floor(tan(exp(x)))", "1/(sin(x)sin(2x)sin(3x))", "floor(exp(x^2))", "floor(tan(x)) + floor(1/x)", "floor(1/sin(x))",
        "floor(1/cos(x))", "floor(tan(1/x))", "ceil(tan(x^2))", "floor(exp(tan(x)))", "tan(floor(x^2))", "floor(sin(exp(x)))",
        "floor(x^2 sin(1/x))", "floor(1/(x^2 - 1))", "floor(tan(x)^2)", "ceil(exp(1/x))", "floor(10tan(x))", "floor(exp(x))/floor(x)",
        "tan(exp(x))", "tan(x^3)", "1/tan(x^2)", "floor(sin(1/x)/x)", "floor(x tan(x))", "floor(ln(abs(tan(x))))",
        "floor(sqrt(abs(tan(x))))", "floor(1/x) floor(tan(x))", "ceil(cos(1/x))", "floor(exp(sin(x^2)))", "floor(tan(x/2) + tan(x))", "floor(1/sin(x^2))",
        "floor(x/tan(x))", "floor(exp(-1/x^2)*10)", "floor(tan(sqrt(abs(x))))", "1/(sin(x^2)sin(x))", "floor(sin(x)^2*10)", "floor(cos(x^2)*10)",
        "floor(1/(sin(x)+cos(x)))", "floor(tan(x)) - floor(tan(x))", "sin(pi floor(tan(x^2)))", "floor(x^3 - x)", "floor(exp(x)) + floor(exp(-x))", "floor(2^x)",
        "ceil(log(abs(x)))", "floor(1/x^3)", "floor(tan(2x)) + ceil(tan(x))", "tan(1/x)/x", "floor(x)^floor(x)", "floor(abs(tan(x)))",
        "floor(exp(x) sin(x))", "floor(tan(pi x))", "floor(1/tan(pi x))", "ceil(x tan(1/x))", "floor(exp(1/sin(x)))", "floor(sin(tan(x)))",
        "floor(tan(sin(x)) * 5)", "floor(cos(tan(x)) * 3)", "1/(tan(x) - x)", "floor(1/(tan(x) - x))", "floor(exp(cos(1/x)))", "floor(tan(exp(-x)))",
    ];
    assert_eq!(fns.len(), 60);
    let mut fails = vec![];
    for f in fns {
        for (x0, x1) in [(-6.0, 10.0), (-100.0, 100.0), (-10.0, 10.0)] {
            let p = with(json!({"pieces": format!("y = {f}"), "dots": "", "ask_disc": true, "kind_labels": true, "xmin": x0, "xmax": x1}));
            let time = |p: Params| -> Option<f64> {
                // (a fresh build each time: the cache keeps the last few)
                let mut p = p;
                static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                p.0.insert("title".into(), crate::figures::Value::S(format!("t{}", N.fetch_add(1, std::sync::atomic::Ordering::Relaxed))));
                let (tx, rx) = std::sync::mpsc::channel();
                // (the render thread's own CPU time: the rest of the suite runs alongside)
                std::thread::spawn(move || { let t = thread_cpu(); let _ = svg(&p); let _ = tx.send(thread_cpu() - t); });
                rx.recv_timeout(std::time::Duration::from_secs(5)).ok()
            };
            match time(p.clone()) {
                None => fails.push(format!("{f} on {x0}..{x1}: still running after 5 s")),
                // (the fastest of three: the whole suite runs alongside)
                Some(t) if t >= 0.5 => if (0..2).all(|_| time(p.clone()).is_none_or(|t2| t2 >= 0.5)) { fails.push(format!("{f} on {x0}..{x1}: {t:.2} s")); },
                _ => {}
            }
        }
    }
    assert!(fails.is_empty(), "{} slow:\n{}", fails.len(), fails.join("\n"));
}

/// "Piling up near" is written only for all of the places in the window where cuts pile up, each established, and only
/// at points where they do pile up: floor(tan x) on -100..100 has 64 poles (more than are worked out), floor(e^(x²)) on
/// -30..30 has finitely many cuts and nothing piles up at ±30. Otherwise the line is a blank or "too many".
#[test]
fn piles_are_written_only_when_all_are_established() {
    let pi = "\u{3c0}";
    let key = |f: &str, x0: f64, x1: f64| -> String { let t = plain_text(&svg(&with(json!({"pieces": f, "dots": "", "ask_at": "1/2", "ask_disc": true, "kind_labels": true, "xmin": x0, "xmax": x1, "square": false})))); t[t.find("Where is").unwrap_or(0)..].to_string() };
    let mut fails = vec![];
    for (f, x0, x1) in [("y = floor(tan(x))", -100.0, 100.0), ("y = floor(tan(x)) + floor(1/x)", -100.0, 100.0), ("y = floor(1/sin(x))", -100.0, 100.0),
        ("y = floor(1/cos(x))", -50.0, 50.0), ("y = floor(exp(x^2))", -30.0, 30.0), ("y = floor(exp(x^2))", -100.0, 100.0)] {
        let k = key(f, x0, x1);
        if k.contains("piling up") { fails.push(format!("{f} on {x0}..{x1}: {k}")); }
    }
    // on -30..30 floor(tan x) has 19 poles: all of them, or none claimed
    let k = key("y = floor(tan(x))", -30.0, 30.0);
    if k.contains("piling up") {
        for m in [-19i32, -17, -15, -13, -11, -1, 1, 13, 15, 17, 19] {
            let t = if m == 1 { format!("{pi}/2") } else if m == -1 { format!("\u{2212}{pi}/2") } else if m < 0 { format!("\u{2212}{}{pi}/2", -m) } else { format!("{m}{pi}/2") };
            if !k.contains(&format!("x = {t}")) { fails.push(format!("floor(tan x) on -30..30 leaves out {t}: {k}")); }
        }
    }
    // and on the default window it is still said (five poles)
    let k = key("y = floor(tan(x))", -6.0, 10.0);
    if !k.contains("piling up") { fails.push(format!("floor(tan x): {k}")); }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

/// The fifth check's small things and the answers it saw lost: floor(5 - |x|) cut exactly at whole numbers; roots
/// written √(14π)/2, not √14·π^(1/2)/2; a squeezed column of y numbers thinned, not drawn over itself; open circles at every
/// end of 0^sin(x)'s domain; floor(1 - x²)'s list of jumps, x e^(1/x) → ∞ at 0⁺, the piles of floor(1/x) - 1/x and
/// x² floor(1/x²) restored; sin 5x/sin 2x and |sin x|/sin x have no limit at ±∞.
#[test]
fn small_things_of_the_fifth_check() {
    let mut fails = vec![];
    // floor(5 - |x|): every cut exact, the list complete
    let p = with(json!({"pieces": "y = floor(5 - abs(x))", "dots": "", "ask_disc": true, "kind_labels": true}));
    let (ds, done) = model_of(&p).discontinuities();
    if !done || ds.len() != 17 || ds.iter().any(|d| d.text.contains('\u{2248}')) || !ds.iter().any(|d| d.text == "0" && d.kind == "removable") { fails.push(format!("floor(5 - |x|): {done} {:?}", ds.iter().map(|d| (&d.text, d.kind)).collect::<Vec<_>>())); }
    if warnings(&p).iter().any(|w| w.contains('\u{2248}')) { fails.push(format!("floor(5 - |x|): {:?}", warnings(&p))); }
    // roots of a multiple of π under one root
    let n = warnings(&with(json!({"pieces": "y = tan(x^2)", "dots": "", "ask_disc": true, "xmin": -2.0, "xmax": 2.0}))).join(" ");
    if n.contains("^(1/2)") || !n.contains("\u{221a}(6\u{3c0})/2") { fails.push(format!("root forms: {n}")); }
    // a squeezed column of y numbers: no two drawn on each other
    let s = svg(&with(json!({"pieces": "y = floor(sin(x))", "dots": "", "xmin": -100.0, "xmax": 100.0, "square": true})));
    let mut ys: Vec<f64> = s.split("<text").filter(|t| t.contains(r#"text-anchor="end""#)).filter_map(|t| attr(t, "y")).collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // (each number is drawn twice, a halo under it)
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.1);
    if ys.windows(2).any(|w| w[1] - w[0] < 8.0) { fails.push(format!("y numbers on each other: {ys:?}")); }
    // 0^sin(x): an open circle at each end of its domain in the window (-π, 0, π, 2π, 3π)
    let p = with(json!({"pieces": "y = 0^sin(x)", "dots": "", "ask_at": "pi"}));
    let _ = svg(&p);
    let opens = last_drawn(&p).circles.iter().filter(|c| !c.closed).count();
    if opens != 5 { fails.push(format!("0^sin(x): {opens} open circles")); }
    // restored answers
    let (ds, done) = model_of(&with(json!({"pieces": "y = floor(1 - x^2)", "dots": ""}))).discontinuities();
    if !done || ds.len() < 60 || ds.iter().any(|d| d.kind == UNSURE) { fails.push(format!("floor(1 - x²): {done}, {} places", ds.len())); }
    let md = model_of(&with(json!({"pieces": "y = x exp(1/x)", "dots": ""})));
    if md.side(&Num::read("0").unwrap().k, 1.0) != Lim::PInf { fails.push(format!("x e^(1/x) at 0+: {:?}", md.side(&Num::read("0").unwrap().k, 1.0))); }
    for f in ["y = floor(1/x) - 1/x", "y = x^2 floor(1/x^2)"] {
        let t = plain_text(&svg(&with(json!({"pieces": f, "dots": "", "ask_at": "", "ask_disc": true, "xmin": -3.0, "xmax": 3.0}))));
        if !(t.contains("infinitely many") && t.contains("piling up near x = 0")) { fails.push(format!("{f}: {}", &t[t.find("Where is").unwrap_or(0)..])); }
    }
    for f in ["y = sin(5x)/sin(2x)", "y = abs(sin(x))/sin(x)"] {
        let md = model_of(&with(json!({"pieces": f, "dots": ""})));
        for dir in [1.0, -1.0] { if md.at_inf(dir) != Some(Lim::Dne) { fails.push(format!("{f} at {dir}∞: {:?}", md.at_inf(dir))); } }
    }
    // and a slowly dying oscillation still has its limit, not "does not exist"
    let md = model_of(&with(json!({"pieces": "y = sin(x)/ln(x)", "dots": ""})));
    if md.at_inf(1.0) == Some(Lim::Dne) { fails.push("sin(x)/ln(x) at ∞: does not exist".into()); }
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

/// Seconds of CPU this thread has used (from /proc on Linux, in 10 ms ticks): what a render costs, whatever else the
/// machine is running; elsewhere, wall time.
fn thread_cpu() -> f64 {
    #[cfg(target_os = "linux")]
    if let Ok(st) = std::fs::read_to_string("/proc/thread-self/stat") {
        // (fields after the command's closing bracket: utime and stime are the 12th and 13th)
        if let Some(rest) = st.rsplit_once(')').map(|r| r.1) {
            let f: Vec<&str> = rest.split_whitespace().collect();
            if let (Some(u), Some(k)) = (f.get(11).and_then(|v| v.parse::<f64>().ok()), f.get(12).and_then(|v| v.parse::<f64>().ok())) { return (u + k) / 100.0; }
        }
    }
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

#[test]
fn ordinary_classroom_spellings_read_and_evaluate() {
    for (f, at, expected) in [("ln x", "1", 0.0), ("sin x / x", "pi/2", 2.0/std::f64::consts::PI),
        ("sinx/x", "pi/2", 2.0/std::f64::consts::PI), ("√(x + 1)", "3", 2.0),
        ("x²", "3", 9.0), ("log10(x)", "100", 2.0), ("log(x, 2)", "8", 3.0)] {
        let p = with(json!({"pieces": format!("y = {f}"), "dots": "", "ask_at": at}));
        assert!(!warnings(&p).iter().any(|n| n.contains("cannot be read")), "{f}: {:?}", warnings(&p));
        let e = cas::parse(f).unwrap();
        let a = cas::ev(&cas::parse(at).unwrap(), 0.0).unwrap();
        assert!((cas::ev(&e, a).unwrap() - expected).abs() < 1e-12, "{f}");
        assert!(svg(&p).contains("data-lim"), "{f}");
    }
}


#[test]
fn bounded_wave_zero_limit_is_a_horizontal_asymptote() {
    let p = with(json!({"pieces": "y = sin(x)^2/x", "dots": "", "ask_inf": "Both", "show_asym": true, "asym_eqs": true}));
    let s = svg(&p);
    let d = last_drawn(&p);
    assert!(d.labels.iter().any(|(_, text, _, _)| text == "y = 0"), "{}", words(&s));
}
