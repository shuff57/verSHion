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
    assert!(w.contains("lim x→4- f(x) = does not exist") && !w.contains('∞'.to_string().as_str()) || w.contains("x→∞"), "{w}");
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
