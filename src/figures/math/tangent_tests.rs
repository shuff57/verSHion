//! Tests of the tangent and secant lines figure (`tangent-secant-lines`). Every number checked here was worked by hand
//! (the working is in the comments), never read back from the figure's own code.
use super::tangent::{self, build, Slope, ID};
use super::tangent_expr as ex;
use crate::figures::{registry, start_params, FigureDef, Params, Value};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("registered") }
fn with(over: &[(&str, Value)]) -> Params {
    let d = def();
    let mut p = start_params(&d);
    for (k, v) in over { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn s(v: &str) -> Value { Value::S(v.to_string()) }
fn f(v: f64) -> Value { Value::F(v) }
fn b(v: bool) -> Value { Value::B(v) }

/// The plain text of every element tagged `tag` (tspans dropped, entities read).
fn texts(svg: &str, tag: &str) -> Vec<String> {
    let pat = format!(r#"data-tsl="{}">"#, tag.trim_end_matches('^'));
    svg.match_indices(&pat).map(|(i, _)| {
        let rest = &svg[i + pat.len()..];
        let body = &rest[..rest.find("</text>").unwrap_or(0)];
        let body = if tag.ends_with('^') { body.replace(r#"<tspan baseline-shift="super" font-size="70%">"#, "^") } else { body.to_string() };
        let mut out = String::new();
        let mut inside = false;
        for c in body.chars() { match c { '<' => inside = true, '>' => inside = false, c if !inside => out.push(c), _ => {} } }
        out.replace("&amp;", "&")
    }).collect()
}
fn panel(p: &Params) -> Vec<String> { texts(&build(p).svg, "panel") }
fn has(lines: &[String], want: &str) -> bool { lines.iter().any(|l| l.contains(want)) }
const M: &str = "\u{2212}";

// ───────────────────────── numbers, worked by hand ─────────────────────────

/// The exact text of a typed constant, or of a function's exact value at an exact point.
fn tx(t: &str, dec_ok: bool) -> Option<String> { ex::text(&ex::typed_exact(t)?, dec_ok) }
fn fx_at(f: &str, a: &str) -> Option<String> { ex::text(&ex::exact(&ex::parse(f)?, Some(&ex::typed_exact(a)?))?, false) }

#[test]
fn exact_forms_are_derived_and_textbook() {
    assert_eq!(tx("3", false).as_deref(), Some("3"));
    assert_eq!(tx("-1/2", false).as_deref(), Some("\u{2212}1/2"));
    assert_eq!(tx("-0.5", true).as_deref(), Some("\u{2212}0.5"));
    assert_eq!(tx("-0.5", false).as_deref(), Some("\u{2212}1/2"));
    assert_eq!(tx("1/3", true).as_deref(), Some("1/3"));
    assert_eq!(tx("1/(2sqrt(2))", false).as_deref(), Some("\u{221a}2/4"));
    assert_eq!(tx("-3sqrt(2)", false).as_deref(), Some("\u{2212}3\u{221a}2"));
    assert_eq!(tx("pi/4", false).as_deref(), Some("\u{3c0}/4"));
    assert_eq!(tx("e", false).as_deref(), Some("e"));
    assert_eq!(tx("e^2/2", false).as_deref(), Some("e^2/2"));
    assert_eq!(tx("1/e", false).as_deref(), Some("1/e"));
    assert_eq!(tx("2/pi", false).as_deref(), Some("2/\u{3c0}"));
    assert_eq!(tx("e - 1", false).as_deref(), Some(&*format!("e {M} 1")));
    assert_eq!(tx("1 + sqrt(2)", false).as_deref(), Some("\u{221a}2 + 1"));
    // known exact values of the functions: sin(π/3) = √3/2, cos(π/12) = (√6 + √2)/4, tan(π/6) = √3/3, ln(e^2) = 2, e^0 = 1,
    // arctan(1) = π/4
    assert_eq!(fx_at("sin(x)", "pi/3").as_deref(), Some("\u{221a}3/2"));
    assert_eq!(fx_at("cos(x)", "pi/12").as_deref(), Some("\u{221a}2/4 + \u{221a}6/4"));
    assert_eq!(fx_at("tan(x)", "pi/6").as_deref(), Some("\u{221a}3/3"));
    assert_eq!(fx_at("ln(x)", "e^2").as_deref(), Some("2"));
    assert_eq!(fx_at("e^x", "0").as_deref(), Some("1"));
    assert_eq!(fx_at("arctan(x)", "1").as_deref(), Some("\u{3c0}/4"));
    // nothing is recognised from a float: ln 2, e^0.3, cos(1.2) and √1.0001 have no exact form here
    assert_eq!(fx_at("ln(x)", "2"), None);
    assert_eq!(fx_at("e^x", "0.3"), None);
    assert_eq!(fx_at("cos(x)", "1.2"), None);
    // (√1.0001 = √10001/100 exactly, but a root over 200 is written as a decimal)
    assert_eq!(fx_at("sqrt(x)", "1.0001"), None);
    // (1.0001)^2 = 1.00020001 exactly
    assert_eq!(ex::text(&ex::exact(&ex::parse("x^2").unwrap(), Some(&ex::typed_exact("1.0001").unwrap())).unwrap(), true).as_deref(), Some("1.00020001"));
}

#[test]
fn slopes_of_the_curve_by_hand() {
    let d = |e: &str, a: f64| match tangent::slope_of(e, a) { Slope::Finite(v) => v, o => panic!("{e} at {a}: {o:?}") };
    // d/dx x^2 = 2x; x^3 = 3x^2; sin = cos; e^x = e^x; ln = 1/x; sqrt = 1/(2 sqrt x); 1/x = -1/x^2
    assert!((d("x^2", 1.0) - 2.0).abs() < 1e-14);
    assert!((d("x^3", 2.0) - 12.0).abs() < 1e-13);
    assert!((d("sin(x)", std::f64::consts::PI / 3.0) - 0.5).abs() < 1e-14);
    assert!((d("e^x", 0.0) - 1.0).abs() < 1e-14);
    assert!((d("ln(x)", 2.0) - 0.5).abs() < 1e-14);
    assert!((d("sqrt(x)", 4.0) - 0.25).abs() < 1e-14);
    assert!((d("1/x", 2.0) + 0.25).abs() < 1e-14);
    assert!((d("x^3 - 3x", 1.0)).abs() < 1e-14);
    assert!(matches!(tangent::slope_of("abs(x)", 0.0), Slope::Corner(l, r) if (l + 1.0).abs() < 1e-6 && (r - 1.0).abs() < 1e-6));
    assert_eq!(tangent::slope_of("sqrt(abs(x))", 0.0), Slope::Cusp);
    assert_eq!(tangent::slope_of("x^(2/3)", 0.0), Slope::Cusp);
    assert_eq!(tangent::slope_of("sqrt(x)", 0.0), Slope::OneSided);
    assert_eq!(tangent::slope_of("ln(x)", 0.0), Slope::Undefined);
    assert_eq!(tangent::slope_of("x^(1/3)", 0.0), Slope::Vertical);
    // |x|·x at 0: the derivative's formula has 0/0 there, both sides give 0
    assert!(matches!(tangent::slope_of("abs(x)*x", 0.0), Slope::Finite(v) if v.abs() < 1e-6));
}

#[test]
fn opening_state_is_the_checklists() {
    // y = x^2, P(1, 1), Q(2, 4): tangent slope 2x = 2, secant (4 - 1)/1 = 3
    let p = start_params(&def());
    let l = panel(&p);
    assert!(has(&l, "f(x) = x2"), "{l:?}");
    assert!(has(&l, "slope f\u{2032}(1) = 2"), "{l:?}");
    assert!(has(&l, "slope m = 3"), "{l:?}");
    let o = build(&p);
    assert!(o.notes.is_empty(), "{:?}", o.notes);
    assert!(o.p_px.is_some() && o.q_px.is_some());
    let hs = crate::figures::interact::handles_for(ID, &p, crate::figures::svg_size(&(def().render)(&p)));
    for tip in ["Drag P along the curve", "Drag Q along the curve to change h", "Drag to move this label"] { assert!(hs.iter().any(|h| h.tip.contains(tip)), "{tip}"); }
}

#[test]
fn hand_worked_slopes_and_equations() {
    // x^2 at 1: tangent y - 1 = 2(x - 1), y = 2x - 1; secant through (1, 1), (2, 4): y = 3x - 2; normal slope -1/2: y = -(1/2)x + 3/2
    let p = with(&[("tangent_eq", s("Slope-intercept")), ("secant_eq", s("Slope-intercept")), ("normal", s("With its equation"))]);
    let l = panel(&p);
    for w in [format!("y = 2x {M} 1"), format!("y = 3x {M} 2"), format!("y = {M}(1/2)x + 3/2")] { assert!(has(&l, &w), "{w}: {l:?}"); }
    let l = panel(&with(&[("tangent_eq", s("Point-slope")), ("secant_eq", s("Point-slope"))]));
    for w in [format!("y {M} 1 = 2(x {M} 1)"), format!("y {M} 1 = 3(x {M} 1)")] { assert!(has(&l, &w), "{w}: {l:?}"); }
    // worked: [f(2) - f(1)]/1 = (4 - 1)/1 = 3
    let l = panel(&with(&[("worked", b(true))]));
    assert!(has(&l, &format!("m = [f(a + h) {M} f(a)]/h")) && has(&l, &format!("= [f(2) {M} f(1)]/1")) && has(&l, &format!("= (4 {M} 1)/1 = 3")), "{l:?}");
    // h = -1: Q(0, 0), (0 - 1)/(-1) = 1
    let l = panel(&with(&[("h", s("-1")), ("worked", b(true))]));
    assert!(has(&l, &format!("= (0 {M} 1)/({M}1) = 1")), "{l:?}");
    // shrinking h: 2 -> 4, 1 -> 3, 0.5 -> 2.5, 0.1 -> 2.1 (slope 2 + h), limit 2
    let l = panel(&with(&[("h", s("2")), ("more_h", s("1, 0.5, 0.1")), ("table", b(true))]));
    for w in ["4", "3", "2.5", "2.1", "f\u{2032}(1) = 2"] { assert!(has(&l, w), "{w}: {l:?}"); }
    // a = 1/2, h = 1/2 (typed fractions): f(1/2) = 1/4, f(1) = 1, slope (1 - 1/4)/(1/2) = 3/2; tangent y = x - 1/4
    let l = panel(&with(&[("a", s("1/2")), ("h", s("1/2")), ("worked", b(true)), ("tangent_eq", s("Slope-intercept"))]));
    assert!(has(&l, &format!("= (1 {M} 1/4)/(1/2) = 3/2")) && has(&l, &format!("y = x {M} 1/4")), "{l:?}");
    // sqrt at 4, h = 5: (3 - 2)/5 = 1/5; f'(4) = 1/4; tangent y = (1/4)x + 1
    let l = panel(&with(&[("func", s("sqrt(x)")), ("a", s("4")), ("h", s("5")), ("worked", b(true)), ("tangent_eq", s("Slope-intercept")), ("xmin", f(0.0)), ("xmax", f(10.0)), ("ymin", f(-1.0)), ("ymax", f(4.0))]));
    assert!(has(&l, &format!("= (3 {M} 2)/5 = 1/5")) && has(&l, "f\u{2032}(4) = 1/4") && has(&l, "y = (1/4)x + 1"), "{l:?}");
    // sin at pi/3, h = pi/2: f(pi/3) = √3/2, f(5pi/6) = 1/2, f'(pi/3) = cos(pi/3) = 1/2; secant (1/2 - √3/2)/(pi/2) = (1 - √3)/pi ≈ -0.233
    let l = panel(&with(&[("func", s("sin(x)")), ("a", s("pi/3")), ("h", s("pi/2")), ("worked", b(true)), ("tangent_eq", s("Slope-intercept")), ("xunits", s("Multiples of \u{3c0}")), ("xmin", f(-1.0)), ("xmax", f(1.0)), ("xstep", f(0.25)), ("ymin", f(-2.0)), ("ymax", f(2.0))]));
    assert!(has(&l, "f\u{2032}(\u{3c0}/3) = 1/2") && has(&l, &format!("= [f(5\u{3c0}/6) {M} f(\u{3c0}/3)]/(\u{3c0}/2)")) && has(&l, &format!("= (1/2 {M} \u{221a}3/2)/(\u{3c0}/2) = ")), "{l:?}");
    // the slope (1 − √3)/π = −0.2330 is derived exactly, and read back is that value
    let w = l.iter().find(|x| x.starts_with("= (1/2")).unwrap();
    assert!((read_back(w.rsplit(" = ").next().unwrap()).unwrap() - (1.0 - 3f64.sqrt()) / std::f64::consts::PI).abs() < 1e-12, "{w}");
    // its intercept √3/2 − π/6 is derived exactly too (f(a) − f′(a)·a)
    assert!(has(&l, &format!("y = (1/2)x + \u{221a}3/2 {M} \u{3c0}/6")), "{l:?}");
    // the cubic x^3 - 3x at 1: f'(1) = 3 - 3 = 0, tangent y = -2; secant to (2, 2): (2 - (-2))/1 = 4
    let l = panel(&with(&[("func", s("x^3 - 3x")), ("tangent_eq", s("Slope-intercept")), ("worked", b(true)), ("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-4.0)), ("ymax", f(4.0))]));
    assert!(has(&l, "f\u{2032}(1) = 0") && has(&l, &format!("y = {M}2")) && has(&l, &format!("= (2 {M} ({M}2))/1 = 4")), "{l:?}");
    // velocity: s(t) = -16t^2 + 64t, s(1) = 48, s(2) = 64, average 16, s'(t) = -32t + 64 = 32, tangent s = 32t + 16
    let l = panel(&with(&[("func", s("-16t^2 + 64t")), ("fname", s("s")), ("var", s("t")), ("tangent_eq", s("Slope-intercept")), ("worked", b(true)), ("xmin", f(0.0)), ("xmax", f(4.0)), ("ymin", f(0.0)), ("ymax", f(80.0)), ("ystep", f(10.0))]));
    assert!(has(&l, "s\u{2032}(1) = 32") && has(&l, "s = 32t + 16") && has(&l, &format!("= [s(2) {M} s(1)]/1")) && has(&l, &format!("= (64 {M} 48)/1 = 16")), "{l:?}");
    // e^x at 0: tangent y = x + 1, normal slope -1: y = -x + 1
    let l = panel(&with(&[("func", s("e^x")), ("a", s("0")), ("tangent_eq", s("Slope-intercept")), ("normal", s("With its equation")), ("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-2.0)), ("ymax", f(4.0))]));
    assert!(has(&l, "y = x + 1") && has(&l, &format!("y = {M}x + 1")), "{l:?}");
    // ln at 1, h = 1: slope ln 2 = 0.693.. (no exact form: ≈ 0.69); tangent y = x - 1
    let l = panel(&with(&[("func", s("ln(x)")), ("tangent_eq", s("Slope-intercept")), ("xmin", f(-1.0)), ("xmax", f(5.0)), ("ymin", f(-3.0)), ("ymax", f(3.0))]));
    assert!(has(&l, "m \u{2248} 0.69") && has(&l, &format!("y = x {M} 1")), "{l:?}");
    // Decimals with 3 places: normal slope -1/2 -> -0.5; sqrt at 2 slope 1/(2√2) = √2/4 exact, 0.354 in decimals
    let l = panel(&with(&[("func", s("sqrt(x)")), ("a", s("2")), ("xmin", f(0.0)), ("xmax", f(5.0)), ("ymin", f(-1.0)), ("ymax", f(3.0))]));
    assert!(has(&l, "f\u{2032}(2) = \u{221a}2/4"), "{l:?}");
    let l = panel(&with(&[("func", s("sqrt(x)")), ("a", s("2")), ("slope_form", s("Decimals")), ("dec", Value::I(3)), ("xmin", f(0.0)), ("xmax", f(5.0)), ("ymin", f(-1.0)), ("ymax", f(3.0))]));
    assert!(has(&l, "f\u{2032}(2) \u{2248} 0.354"), "{l:?}");
    // more tangents of x^2 - 4x + 3 at 0 and 4: slopes 2x - 4 = -4 and 4; at P (2): 0
    let l = panel(&with(&[("func", s("x^2 - 4x + 3")), ("a", s("2")), ("more_a", s("0, 4")), ("show_secant", b(false)), ("xmin", f(-1.0)), ("xmax", f(5.0)), ("ymin", f(-3.0)), ("ymax", f(5.0))]));
    assert!(has(&l, &format!("x = 0: slope {M}4")) && has(&l, "x = 4: slope 4") && has(&l, "f\u{2032}(2) = 0"), "{l:?}");
}

#[test]
fn triangles_and_labels_say_what_was_worked() {
    // secant triangle of x^2 from P(1, 1): h = 1, Δy = 4 - 1 = 3; tangent triangle run 1 rise 2
    let svg = build(&with(&[("secant_triangle", b(true)), ("tangent_triangle", b(true)), ("h_mark", b(true)), ("pt_labels", s("Letters and coordinates"))])).svg;
    let labs: Vec<String> = crate::figures::math::flat_b9::text_boxes(&svg).into_iter().map(|t| t.0).collect();
    for w in ["h = 1", "\u{394}y = 3", "P(1, 1)", "Q(2, 4)", "h"] { assert!(labs.iter().any(|l| l == w), "{w}: {labs:?}"); }
    assert!(labs.iter().any(|l| l == "1") && labs.iter().any(|l| l == "2"), "{labs:?}");
    // the key line: f'(1) = 2, tangent y = 2x - 1, secant slope 3
    let k = texts(&build(&with(&[("key_line", b(true))])).svg, "key").join(" ");
    assert!(k.contains("f\u{2032}(1) = 2") && k.contains(&format!("tangent y = 2x {M} 1")) && k.contains("secant slope 3 (h = 1)"), "{k}");
}

// ───────────────────────── refusals ─────────────────────────

#[test]
fn bad_inputs_are_said_in_the_panel() {
    let n = |o: &[(&str, Value)]| tangent::warnings(&with(o)).join(" | ");
    assert!(n(&[("func", s("sin x"))]).contains("brackets"));
    assert!(n(&[("func", s("x^2, x > 0"))]).contains("one function"));
    assert!(n(&[("func", s(""))]).contains("Type a function"));
    assert!(n(&[("var", s("t"))]).contains("uses x"));
    assert!(n(&[("var", s("e"))]).contains("one letter other than e"));
    assert!(n(&[("a", s("abc"))]).contains("Type a as a number"));
    assert!(n(&[("h", s("0"))]).contains("h cannot be 0"));
    assert!(n(&[("func", s("abs(x)")), ("a", s("0"))]).contains("corner"));
    assert!(n(&[("func", s("sqrt(x)")), ("a", s("0"))]).contains("one side"));
    assert!(n(&[("func", s("ln(x)")), ("a", s("0"))]).contains("undefined"));
    assert!(n(&[("func", s("sqrt(x)")), ("a", s("1")), ("h", s("-2"))]).contains("no secant"));
    assert!(n(&[("a", s("9"))]).contains("P is outside the window"));
    assert!(n(&[("more_h", s("1, 2, 3, 4, 5, 6, 7, 8"))]).contains("only the first 6"));
    assert!(n(&[("normal", s("With its slope"))]).contains("Same scale"));
    assert!(!n(&[("normal", s("With its slope")), ("square", b(true))]).contains("Same scale"));
    // y = and f(x) = in front are read; anything else before = is refused
    assert!(n(&[("func", s("f(x) = x^2"))]).is_empty());
    assert!(n(&[("func", s("y = x^2"))]).is_empty());
    assert!(n(&[("func", s("g(x) = x^2"))]).contains("right-hand side"));
}

// ───────────────────────── the label sweep ─────────────────────────

type Bx = (f64, f64, f64, f64);
type Seg = ((f64, f64), (f64, f64));
fn attr<'a>(h: &'a str, n: &str) -> Option<&'a str> { h.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()) }
fn numa(h: &str, n: &str) -> f64 { attr(h, n).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }

/// Every stroke a label must keep off, with its tag (curves, axes, the lines, the slope triangles' legs, the h mark and
/// its drop lines), clipped to what is seen.
fn strokes(svg: &str) -> Vec<(String, Seg)> {
    let clip = svg.split(r#"<clipPath id="plot"><rect "#).nth(1).map(|r| { let h = r.split('>').next().unwrap(); (numa(h, "x"), numa(h, "y"), numa(h, "x") + numa(h, "width"), numa(h, "y") + numa(h, "height")) }).unwrap();
    let mut v = vec![];
    for (i, _) in svg.match_indices("<line ") {
        let h = &svg[i..i + svg[i..].find('>').unwrap()];
        let tag = attr(h, "data-tsl").unwrap_or("");
        if matches!(tag, "swatch" | "blank") { continue; }
        if numa(h, "stroke-width") < 1.9 && !matches!(tag, "hmark" | "drop") { continue; }
        let name = if tag == "tri" { format!("tri:{}", attr(h, "data-leg").unwrap_or("")) } else if tag.is_empty() { "axis".to_string() } else { tag.to_string() };
        v.push((name, ((numa(h, "x1"), numa(h, "y1")), (numa(h, "x2"), numa(h, "y2")))));
    }
    for (i, _) in svg.match_indices("<polyline ") {
        let h = &svg[i..i + svg[i..].find('>').unwrap()];
        if numa(h, "stroke-width") < 1.9 { continue; }
        let pts: Vec<(f64, f64)> = attr(h, "points").unwrap_or("").split(' ').filter_map(|q| { let (a, b) = q.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
        for w in pts.windows(2) { if let Some(s) = clip_seg(w[0], w[1], clip) { v.push(("curve".into(), s)); } }
    }
    v
}
fn clip_seg(a: (f64, f64), b: (f64, f64), r: Bx) -> Option<Seg> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - r.0), (dx, r.2 - a.0), (-dy, a.1 - r.1), (dy, r.3 - a.1)] {
        if p.abs() < 1e-12 { if q < 0.0 { return None; } continue; }
        let t = q / p;
        if p < 0.0 { t0 = t0.max(t) } else { t1 = t1.min(t) }
        if t0 > t1 { return None; }
    }
    Some(((a.0 + t0 * dx, a.1 + t0 * dy), (a.0 + t1 * dx, a.1 + t1 * dy)))
}
fn on(b: Bx, s: Seg) -> bool { clip_seg(s.0, s.1, (b.0 + 0.5, b.1 + 0.5, b.2 - 0.5, b.3 - 0.5)).is_some() }
fn hit(a: Bx, b: Bx) -> bool { a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3 }

/// The labels of a render: (id, text, box, has a leader).
fn my_labels(svg: &str) -> Vec<(String, String, Bx, bool)> {
    let leaders: Vec<(f64, f64)> = svg.match_indices(r#"data-tsl="leader""#).map(|(i, _)| { let st = svg[..i].rfind("<line ").unwrap(); let h = &svg[st..i]; (numa(h, "x2"), numa(h, "y2")) }).collect();
    let mut out = vec![];
    for (i, _) in svg.match_indices("<g data-lbl=\"") {
        let end = svg[i..].find("</g>").map(|e| i + e + 4).unwrap();
        let id = attr(&svg[i..end], "data-lbl").unwrap_or("").to_string();
        for (t, b) in crate::figures::math::flat_b9::text_boxes(&svg[i..end]) {
            let led = leaders.iter().any(|e| e.0 >= b.0 - 2.0 && e.0 <= b.2 + 2.0 && e.1 >= b.1 - 2.0 && e.1 <= b.3 + 2.0);
            out.push((id.clone(), t, b, led));
        }
    }
    out
}

/// The faults of one render: (label, what is wrong). Read from the drawn SVG, with the checker's rules: a label is never off
/// the page, on a stroke, on a dot or on other text; a point's label is nearer its own dot than any other (by 2 px) and
/// within 30 px of it, unless on a leader; a leg's label is nearer its own leg than any other stroke (by 3 px) and within
/// 30 px of it, unless on a leader; a leader crosses no stroke, ends at its label and runs through no text.
fn faults(svg: &str) -> Vec<(String, String)> {
    let (w, h) = crate::figures::svg_size(svg);
    let all = crate::figures::math::flat_b9::text_boxes(svg);
    let mine = my_labels(svg);
    let lines = strokes(svg);
    let dots: Vec<(f64, f64, f64, String)> = svg.match_indices("<circle ").map(|(i, _)| { let hd = &svg[i..i + svg[i..].find('>').unwrap()]; (numa(hd, "cx"), numa(hd, "cy"), numa(hd, "r"), attr(hd, "data-tsl").unwrap_or("").to_string()) }).filter(|d| d.2 >= 3.0).collect();
    let mut out = vec![];
    for (id, t, bx, led) in &mine {
        if bx.0 < 0.0 || bx.1 < 0.0 || bx.2 > w || bx.3 > h { out.push((t.clone(), "off the page".into())); }
        if lines.iter().any(|(k, s)| k != "leader" && on(*bx, *s)) { out.push((t.clone(), "on a line".into())); }
        if dots.iter().any(|d| d.0 + d.2 > bx.0 && d.0 - d.2 < bx.2 && d.1 + d.2 > bx.1 && d.1 - d.2 < bx.3) { out.push((t.clone(), "on a dot".into())); }
        if all.iter().any(|(_, o)| (o.0 - bx.0).abs() + (o.1 - bx.1).abs() > 0.02 && hit(*bx, *o)) { out.push((t.clone(), "on other text".into())); }
        if *led { continue; }
        if id == "P" || id == "Q" {
            let tag = format!("pt{id}");
            if let Some(own) = dots.iter().find(|d| d.3 == tag) {
                let d0 = tangent::box_pt(*bx, (own.0, own.1));
                if dots.iter().any(|d| d.3 != tag && tangent::box_pt(*bx, (d.0, d.1)) + 2.0 < d0) { out.push((t.clone(), "nearer another dot".into())); }
                if d0 > 30.0 { out.push((t.clone(), "far from its point".into())); }
            }
        }
        if matches!(id.as_str(), "run" | "rise" | "trun" | "trise" | "hmark") {
            let own_tag = if id == "hmark" { "hmark".to_string() } else { format!("tri:{id}") };
            let own = lines.iter().filter(|(k, _)| *k == own_tag).map(|(_, s)| tangent::box_seg(*bx, s.0, s.1)).fold(f64::INFINITY, f64::min);
            let other = lines.iter().filter(|(k, _)| *k != own_tag && k != "leader").map(|(_, s)| tangent::box_seg(*bx, s.0, s.1)).fold(f64::INFINITY, f64::min);
            if own > other + 3.0 { out.push((t.clone(), "nearer another line".into())); }
            if own > 30.0 { out.push((t.clone(), "far from its leg".into())); }
        }
    }
    // a leader runs from its own point to its own label through no line and no other text
    for (i, _) in svg.match_indices(r#"data-tsl="leader""#) {
        let st = svg[..i].rfind("<line ").unwrap();
        let hd = &svg[st..i];
        let (a, e) = ((numa(hd, "x1"), numa(hd, "y1")), (numa(hd, "x2"), numa(hd, "y2")));
        let n = ((e.0 - a.0).powi(2) + (e.1 - a.1).powi(2)).sqrt().max(1e-9);
        let a6 = (a.0 + (e.0 - a.0) / n * 6.0, a.1 + (e.1 - a.1) / n * 6.0);
        if lines.iter().any(|(k, s)| k != "leader" && tangent::seg_cross(a6, e, s.0, s.1)) { out.push(("leader".into(), "crosses a line".into())); }
        if !mine.iter().any(|(_, _, b, _)| e.0 >= b.0 - 0.6 && e.0 <= b.2 + 0.6 && e.1 >= b.1 - 0.6 && e.1 <= b.3 + 0.6) { out.push(("leader".into(), "not at its label".into())); }
        if all.iter().filter(|(_, b)| !(e.0 >= b.0 - 0.6 && e.0 <= b.2 + 0.6 && e.1 >= b.1 - 0.6 && e.1 <= b.3 + 0.6)).any(|(_, b)| clip_seg(a6, (e.0 - (e.0 - a.0) / n, e.1 - (e.1 - a.1) / n), (b.0 + 0.5, b.1 + 0.5, b.2 - 0.5, b.3 - 0.5)).is_some()) { out.push(("leader".into(), "through text".into())); }
    }
    // the panel and the key inside the page
    for (t, bx) in all.iter().filter(|(t, _)| !mine.iter().any(|m| &m.1 == t)) {
        if bx.2 > w + 0.5 || bx.3 > h + 0.5 { out.push((t.clone(), "panel text off the page".into())); }
    }
    out
}

/// Classroom examples of tangents and secants (the functions, points and windows a textbook uses): 67 of mine and the 66
/// the first independent check used. (function, name, variable, a, h, [xmin, xmax, ymin, ymax], π units)
type Case = (&'static str, &'static str, &'static str, &'static str, &'static str, [f64; 4], bool);
fn classroom() -> Vec<Case> {
    let n = |fx, a, h, w| (fx, "f", "x", a, h, w, false);
    let pi = |fx, a, h, w| (fx, "f", "x", a, h, w, true);
    vec![
        n("x^2", "1", "1", [-2.0, 4.0, -2.0, 10.0]), n("x^2", "2", "1", [-1.0, 4.0, -1.0, 10.0]), n("x^2", "-1", "1", [-3.0, 3.0, -2.0, 9.0]),
        n("x^2", "0", "1", [-3.0, 3.0, -2.0, 9.0]), n("x^2", "1", "0.5", [-2.0, 3.0, -2.0, 6.0]), n("x^2", "1", "-1", [-2.0, 3.0, -2.0, 6.0]),
        n("x^2", "3", "-2", [-1.0, 5.0, -2.0, 12.0]), n("2x^2", "1", "1", [-2.0, 3.0, -2.0, 10.0]), n("x^2 - 4", "1", "2", [-4.0, 4.0, -6.0, 8.0]),
        n("x^2 - 4x + 3", "2", "1", [-1.0, 5.0, -3.0, 5.0]), n("x^2 - 4x + 3", "0", "1", [-1.0, 5.0, -3.0, 8.0]), n("-x^2 + 4", "1", "1", [-3.0, 3.0, -6.0, 6.0]),
        n("-x^2 + 2x + 3", "1", "1", [-2.0, 4.0, -4.0, 6.0]), n("x^3", "1", "1", [-2.0, 3.0, -4.0, 10.0]), n("x^3", "0", "1", [-2.0, 2.0, -4.0, 4.0]),
        n("x^3 - 3x", "1", "1", [-3.0, 3.0, -4.0, 4.0]), n("x^3 - 3x", "-1", "1", [-3.0, 3.0, -4.0, 4.0]), n("x^3 - 3x", "2", "-1", [-3.0, 3.0, -4.0, 4.0]),
        n("x^3 - 2x^2 + 1", "1", "1", [-2.0, 3.0, -3.0, 5.0]), n("0.5x^3", "2", "1", [-2.0, 4.0, -4.0, 16.0]), n("x^4 - 2x^2", "1", "0.5", [-2.0, 2.0, -2.0, 4.0]),
        n("x^4", "1", "0.5", [-2.0, 2.0, -1.0, 6.0]), n("3x + 1", "1", "2", [-2.0, 4.0, -4.0, 14.0]), n("x^2/2", "2", "2", [-2.0, 5.0, -1.0, 10.0]),
        n("(x - 1)^2", "1", "1", [-2.0, 4.0, -1.0, 5.0]), n("x^2 + 2x", "-1", "2", [-4.0, 2.0, -2.0, 4.0]), n("1/x", "1", "1", [0.0, 4.0, 0.0, 4.0]),
        n("1/x", "2", "-1", [0.0, 4.0, 0.0, 4.0]), n("1/x", "-1", "-1", [-4.0, 0.0, -4.0, 0.0]), n("sqrt(x)", "4", "5", [0.0, 10.0, -1.0, 4.0]),
        n("sqrt(x)", "1", "3", [0.0, 5.0, -1.0, 3.0]), n("sqrt(x)", "9", "-5", [0.0, 10.0, -1.0, 4.0]), n("sqrt(x + 2)", "2", "2", [-2.0, 6.0, -1.0, 4.0]),
        n("1/(x^2 + 1)", "1", "1", [-3.0, 3.0, -1.0, 2.0]), n("x/(x + 1)", "1", "1", [-0.5, 4.0, -1.0, 2.0]), n("2/x", "1", "1", [0.0, 4.0, 0.0, 4.0]),
        pi("sin(x)", "0", "pi/2", [-1.0, 1.0, -2.0, 2.0]), pi("sin(x)", "pi/3", "pi/2", [-1.0, 1.0, -2.0, 2.0]), pi("sin(x)", "pi/2", "pi/4", [-1.0, 1.0, -2.0, 2.0]),
        pi("cos(x)", "0", "pi/2", [-1.0, 1.0, -2.0, 2.0]), pi("cos(x)", "pi/2", "pi/4", [-1.0, 1.0, -2.0, 2.0]), pi("sin(x)", "pi/6", "pi/6", [-0.5, 1.5, -2.0, 2.0]),
        pi("tan(x)", "0", "pi/4", [-0.5, 0.5, -3.0, 3.0]), pi("tan(x)", "pi/4", "-pi/4", [-0.5, 0.5, -3.0, 3.0]), pi("2sin(x)", "pi", "pi/2", [0.0, 2.0, -3.0, 3.0]),
        n("sin(x)", "1", "1", [-1.0, 4.0, -2.0, 2.0]), pi("cos(2x)", "pi/4", "pi/4", [-1.0, 1.0, -2.0, 2.0]), n("e^x", "0", "1", [-3.0, 3.0, -1.0, 8.0]),
        n("e^x", "1", "-1", [-2.0, 3.0, -1.0, 10.0]), n("e^x", "0", "0.5", [-2.0, 2.0, -1.0, 5.0]), n("ln(x)", "1", "1", [0.0, 5.0, -3.0, 3.0]),
        n("ln(x)", "2", "1", [0.0, 5.0, -3.0, 3.0]), n("ln(x)", "3", "-1", [0.0, 5.0, -3.0, 3.0]), n("2^x", "0", "1", [-3.0, 3.0, -1.0, 8.0]),
        n("e^(-x)", "0", "1", [-2.0, 3.0, -1.0, 5.0]), n("xe^x", "0", "1", [-3.0, 2.0, -1.0, 6.0]), n("ln(x + 1)", "0", "1", [-1.0, 4.0, -2.0, 3.0]),
        n("e^x - 1", "1", "-1", [-2.0, 3.0, -2.0, 8.0]),
        ("-16t^2 + 64t", "s", "t", "1", "1", [0.0, 4.0, 0.0, 80.0], false), ("-4.9t^2 + 20t", "s", "t", "1", "1", [0.0, 4.5, 0.0, 25.0], false),
        ("t^3 - 6t^2 + 9t", "s", "t", "2", "1", [0.0, 4.0, -2.0, 6.0], false), ("5t^2", "d", "t", "2", "1", [0.0, 4.0, 0.0, 60.0], false),
        ("4/3 pi r^3", "V", "r", "1", "1", [0.0, 3.0, 0.0, 40.0], false),
        n("x^2", "1/2", "1/2", [-1.0, 2.0, -1.0, 3.0]), n("x^2", "-3/2", "1", [-3.0, 1.0, -1.0, 5.0]), n("x^3", "1/2", "1/4", [-1.0, 2.0, -1.0, 2.0]),
        n("x^2", "1", "0.1", [0.0, 2.0, 0.0, 4.0]),
        // the first check's list
        n("x^2", "-2", "1", [-4.0, 3.0, -2.0, 10.0]), n("x^2", "3", "-1", [-1.0, 5.0, -2.0, 12.0]), n("x^2", "1/2", "1/4", [-1.0, 2.0, -1.0, 3.0]),
        n("x^2", "1", "0.5", [-1.0, 3.0, -1.0, 6.0]), n("x^2 + 1", "2", "-1", [-3.0, 4.0, -1.0, 10.0]), n("x^2 - 2x", "3", "1", [-2.0, 5.0, -2.0, 10.0]),
        n("-x^2 + 6x - 5", "2", "2", [-1.0, 6.0, -4.0, 5.0]), n("2x^2 - 3", "-1", "2", [-3.0, 3.0, -4.0, 8.0]), ("3x^2", "g", "x", "1", "-1/2", [-2.0, 2.0, -1.0, 8.0], false),
        n("x^2/4", "2", "2", [-4.0, 6.0, -1.0, 8.0]), n("(x + 2)^2", "-1", "1", [-5.0, 2.0, -1.0, 9.0]), n("x^3", "2", "-1", [-1.0, 3.0, -2.0, 12.0]),
        n("x^3", "-1", "1", [-2.0, 2.0, -4.0, 4.0]), n("x^3 - x", "0", "1", [-2.0, 2.0, -3.0, 3.0]), n("x^3 - 3x^2", "2", "1", [-1.0, 4.0, -5.0, 3.0]),
        n("x^3 - 6x^2 + 9x", "1", "2", [-1.0, 5.0, -2.0, 6.0]), n("-x^3 + 3x", "-1", "2", [-3.0, 3.0, -4.0, 4.0]), n("x^4 - 4x^2", "1", "1", [-3.0, 3.0, -5.0, 5.0]),
        n("x^4", "-1", "1/2", [-2.0, 2.0, -1.0, 4.0]), n("x^5 - 5x", "1", "-1", [-2.0, 2.0, -6.0, 6.0]), n("1/x", "2", "1", [-1.0, 5.0, -1.0, 3.0]),
        n("1/x", "1/2", "1/2", [0.0, 4.0, 0.0, 4.0]), n("1/x", "-2", "-1", [-5.0, 1.0, -3.0, 1.0]), n("1/x^2", "1", "1", [-3.0, 3.0, -1.0, 4.0]),
        n("1/(x - 1)", "2", "1", [-1.0, 5.0, -4.0, 4.0]), n("x/(x^2 + 1)", "0", "1", [-4.0, 4.0, -1.0, 1.0]), n("(x + 1)/(x - 1)", "3", "1", [1.0, 6.0, -1.0, 4.0]),
        n("4/(x^2 + 1)", "1", "1", [-3.0, 3.0, -1.0, 5.0]), n("sqrt(x)", "1", "3", [-1.0, 5.0, -1.0, 3.0]), n("sqrt(x)", "9", "-5", [0.0, 12.0, -1.0, 4.0]),
        n("sqrt(x)", "1/4", "3/4", [0.0, 2.0, 0.0, 2.0]), n("sqrt(x - 1)", "2", "3", [0.0, 6.0, -1.0, 3.0]), n("sqrt(25 - x^2)", "3", "1", [-6.0, 6.0, -1.0, 6.0]),
        n("2sqrt(x)", "4", "-3", [0.0, 6.0, -1.0, 6.0]), n("x^(1/3)", "1", "7", [-1.0, 9.0, -1.0, 3.0]), pi("sin(x)", "0", "pi/2", [-1.0, 1.0, -2.0, 2.0]),
        pi("sin(x)", "pi/6", "pi/3", [-1.0, 1.0, -2.0, 2.0]), pi("sin(x)", "pi/4", "-pi/4", [-1.0, 1.0, -2.0, 2.0]), pi("cos(x)", "pi/3", "pi/6", [-1.0, 1.0, -2.0, 2.0]),
        pi("cos(x)", "pi", "-pi/2", [0.0, 2.0, -2.0, 2.0]), pi("tan(x)", "pi/4", "pi/12", [-0.5, 0.5, -4.0, 4.0]), pi("2cos(x)", "pi/2", "pi/2", [-1.0, 1.0, -3.0, 3.0]),
        pi("sin(2x)", "pi/4", "pi/4", [-1.0, 1.0, -2.0, 2.0]), n("sin(x)", "1", "0.5", [-2.0, 4.0, -2.0, 2.0]), n("cos(x)", "2", "-1", [-1.0, 5.0, -2.0, 2.0]),
        n("e^x", "0", "1", [-3.0, 3.0, -1.0, 8.0]), n("e^x", "1", "1/2", [-2.0, 3.0, -1.0, 10.0]), n("e^x", "-1", "-1", [-4.0, 2.0, -1.0, 4.0]),
        n("e^(2x)", "0", "1/2", [-2.0, 2.0, -1.0, 8.0]), n("2^x", "1", "1", [-3.0, 4.0, -1.0, 10.0]), n("ln(x)", "1", "1", [-1.0, 5.0, -3.0, 3.0]),
        n("ln(x)", "e", "-1", [0.0, 5.0, -2.0, 3.0]), n("ln(x)", "2", "1/2", [0.0, 4.0, -2.0, 2.0]), n("x ln(x)", "1", "1", [-1.0, 4.0, -2.0, 6.0]),
        n("x e^(-x)", "1", "1", [-1.0, 4.0, -1.0, 1.0]), n("log(x)", "10", "-5", [0.0, 12.0, -1.0, 2.0]), ("-16t^2 + 64t", "s", "t", "1", "1", [0.0, 4.0, 0.0, 80.0], false),
        ("-16t^2 + 64t", "s", "t", "2", "-1/2", [0.0, 4.0, 0.0, 80.0], false), ("-4.9t^2 + 19.6t", "h", "t", "1", "1", [0.0, 4.0, 0.0, 25.0], false), ("t^3 - 6t^2 + 9t", "s", "t", "1", "1", [0.0, 4.0, -2.0, 6.0], false),
        ("5t^2", "d", "t", "3", "-1", [0.0, 4.0, 0.0, 90.0], false), ("100e^(0.5t)", "P", "t", "2", "1", [0.0, 4.0, 0.0, 800.0], false), n("2x + 1", "1", "2", [-3.0, 4.0, -6.0, 10.0]),
        n("abs(x - 1)", "2", "1", [-2.0, 4.0, -1.0, 4.0]), n("x^2 - 4", "2", "0.1", [0.0, 3.0, -5.0, 5.0]),
        // the second check's crowded cases
        pi("tan(x)", "-pi/6", "pi/12", [-0.5, 0.5, -4.0, 4.0]), pi("tan(x)", "pi/4", "pi/12", [-0.5, 0.5, -4.0, 4.0]),
        // the third check's single-option fallbacks
        n("x^4", "1", "-1/2", [-2.0, 2.0, -1.0, 5.0]), n("x^4 - 2x^2", "-1", "1/2", [-2.0, 2.0, -2.0, 3.0]), n("x^3/3 - x", "2", "-1", [-3.0, 3.0, -2.0, 3.0]),
        pi("sin(x)", "pi/2", "-pi/4", [-1.0, 1.0, -2.0, 2.0]), pi("tan(x)", "-pi/6", "pi/12", [-0.5, 0.5, -3.0, 3.0]), pi("3sin(x)", "pi/6", "pi/6", [-1.0, 1.0, -4.0, 4.0]),
        n("e^x", "1", "-1/2", [-2.0, 2.0, -1.0, 8.0]),
    ]
}

fn ystep(lo: f64, hi: f64) -> f64 { let r = (hi - lo) / 10.0; [0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0].into_iter().find(|s| *s >= r).unwrap_or(100.0) }

fn case(c: &Case, size: &str, opts: &[(&str, Value)]) -> Params {
    let (fx, fname, var, a, h, w, pi) = c;
    let mut o: Vec<(&str, Value)> = vec![("func", s(fx)), ("fname", s(fname)), ("var", s(var)), ("a", s(a)), ("h", s(h)), ("xmin", f(w[0])), ("xmax", f(w[1])), ("ymin", f(w[2])), ("ymax", f(w[3])), ("ystep", f(ystep(w[2], w[3]))), ("label_size", s(size))];
    if *pi { o.extend([("xunits", s("Multiples of \u{3c0}")), ("xstep", f(0.25))]); } else if w[1] - w[0] > 8.0 { o.push(("xstep", f(2.0))); }
    o.extend(opts.iter().cloned());
    with(&o)
}

/// Each optional part alone, all together, with the key, the student copies and Print (the first check's sets).
fn option_sets() -> Vec<(&'static str, Vec<(&'static str, Value)>)> {
    let all: Vec<(&'static str, Value)> = vec![("secant_triangle", b(true)), ("tangent_triangle", b(true)), ("h_mark", b(true)), ("pt_labels", s("Letters and coordinates")), ("normal", s("With its equation")), ("more_h", s("0.5, -0.5")), ("table", b(true)), ("worked", b(true)), ("tangent_eq", s("Point-slope")), ("secant_eq", s("Slope-intercept")), ("more_a", s("0"))];
    let mut v: Vec<(&'static str, Vec<(&'static str, Value)>)> = vec![("plain", vec![])];
    for (k, val) in &all { v.push((k, vec![(*k, val.clone())])); }
    v.push(("short", vec![("extent", s("Short segment through the point"))]));
    v.push(("all", all.clone()));
    let mut ak = all.clone(); ak.push(("key_line", b(true))); v.push(("allkey", ak));
    let mut st = all.clone(); st.push(("show_answers", b(false))); v.push(("stud", st));
    let mut co = all.clone(); co.extend([("show_answers", b(false)), ("student_copy", s("Curve and P only, to draw the tangent"))]); v.push(("curveonly", co));
    let mut pr = all.clone(); pr.push(("style", s("Print"))); v.push(("print", pr));
    v
}

/// The sweep: every classroom example, at every size, with each optional part alone and all together. Read from the
/// drawn SVG with the checker's rules (see `faults`); also counted: labels set by name only (their whole in the panel).
#[test]
fn classroom_sweep_labels_are_clear() {
    let list = classroom();
    assert!(list.len() >= 130);
    let mut total = 0;
    let mut by: std::collections::BTreeMap<(String, String), usize> = Default::default();
    let mut fails: Vec<String> = vec![];
    for size in ["Small", "Medium", "Large"] {
        for c in &list {
            for (name, opts) in option_sets() {
                let p = case(c, size, &opts);
                let o = build(&p);
                total += 1;
                let mut fs = faults(&o.svg);
                for (k, t) in &o.crowded { fs.push((format!("{k}: {t}"), "no clear place".into())); }
                // a point's coordinates on their own (one option) stay on the graph
                // a label set by name with its value in the panel, with ONE option on, is rare and named (SINGLE_FALLBACKS)
                let single = !matches!(name, "plain" | "everything" | "all" | "allkey" | "stud" | "curveonly" | "print" | "short");
                let moved = texts(&o.svg, "panel").iter().any(|l| l.starts_with("On the graph"));
                if single && moved { println!("single-option fallback: {size} {} a={} h={} [{name}]", c.0, c.3, c.4); }
                if single && moved && size != "Large" && !SINGLE_FALLBACKS.contains(&(size, c.0, c.3, name)) { fs.push(("coordinates".into(), "moved to the panel with one option on".into())); }
                if texts(&o.svg, "panel").iter().any(|l| l.starts_with("On the graph")) { *by.entry((size.to_string(), "(count) a label by name only".into())).or_default() += 1; }
                for (t, why) in fs {
                    *by.entry((size.to_string(), why.clone())).or_default() += 1;
                    if !EXCEPTIONS.contains(&(size, c.0, c.3, name)) { fails.push(format!("{size} {} a={} h={} [{name}]: {t} {why}", c.0, c.3, c.4)); }
                }
            }
        }
    }
    println!("tangent sweep: {total} figures; by size and kind: {by:?}");
    assert!(fails.is_empty(), "{} faults:\n{}", fails.len(), fails.iter().take(80).cloned().collect::<Vec<_>>().join("\n"));
}

/// Single-option figures where a label is set on the graph by its name and its value written in the panel (lesson 18: rare and
/// named), at Small and Medium: the leg or point has no room for the whole label between the curve, the lines, an axis and its
/// numbers. (Large is counted, not named.)
const SINGLE_FALLBACKS: &[(&str, &str, &str, &str)] = &[
    ("Medium", "3sin(x)", "pi/6", "secant_triangle"),
    ("Medium", "sin(x)", "1", "secant_triangle"),
    ("Medium", "sin(x)", "pi/2", "secant_triangle"),
    ("Medium", "sin(x)", "pi/3", "secant_triangle"),
    ("Medium", "tan(x)", "-pi/6", "pt_labels"),
    ("Medium", "tan(x)", "-pi/6", "secant_triangle"),
    ("Medium", "x^3/3 - x", "2", "pt_labels"),
    ("Medium", "x^4 - 2x^2", "-1", "pt_labels"),
    ("Medium", "x^4 - 2x^2", "-1", "secant_triangle"),
    ("Medium", "x^4", "-1", "pt_labels"),
    ("Medium", "x^4", "1", "pt_labels"),
    ("Small", "sin(x)", "pi/2", "secant_triangle"),
    ("Small", "x^4 - 2x^2", "-1", "pt_labels"),
];

/// Named cases where one label may be crowded (drawn where it is least in the way, with a note telling the teacher to drag
/// it): at Small and Medium only the second check's tan at −π/6 and two student copies with every option on; at Large
/// these. (size, function, a, option set)
const EXCEPTIONS: &[(&str, &str, &str, &str)] = &[
    ("Large", "1/x", "-2", "all"),
    ("Large", "1/x", "2", "all"),
    ("Large", "1/x", "-2", "allkey"),
    ("Large", "1/x", "2", "allkey"),
    ("Large", "1/x", "-2", "print"),
    ("Large", "1/x", "2", "print"),
    ("Large", "1/x", "-2", "stud"),
    ("Large", "1/x", "2", "stud"),
    ("Large", "cos(x)", "pi/3", "all"),
    ("Large", "cos(x)", "pi/3", "allkey"),
    ("Large", "cos(x)", "pi/3", "print"),
    ("Large", "cos(x)", "pi/3", "stud"),
    ("Large", "e^(2x)", "0", "pt_labels"),
    ("Large", "e^x", "-1", "all"),
    ("Large", "e^x", "-1", "allkey"),
    ("Large", "e^x", "-1", "print"),
    ("Large", "e^x", "-1", "stud"),
    ("Large", "sin(x)", "1", "pt_labels"),
    ("Large", "sin(x)", "1", "secant_triangle"),
    ("Large", "sin(x)", "pi/6", "pt_labels"),
    ("Large", "x^2", "1", "pt_labels"),
    ("Large", "x^2", "-3/2", "pt_labels"),
    ("Large", "x^3 - 3x", "2", "pt_labels"),
    ("Medium", "sin(x)", "pi/6", "stud"),
    ("Large", "ln(x)", "1", "stud"),
    // the second check's crowded case (tan at −π/6, h = π/12): the secant's triangle is about 22 px tall, between the x-axis,
    // its numbers and the curve, so Δy has no clear place at any size (drawn where least in the way, with a note)
    ("Large", "tan(x)", "-pi/6", "all"),
    ("Large", "tan(x)", "-pi/6", "allkey"),
    ("Large", "tan(x)", "-pi/6", "print"),
    ("Large", "tan(x)", "-pi/6", "pt_labels"),
    ("Large", "tan(x)", "-pi/6", "stud"),
    ("Large", "x^4 - 4x^2", "1", "pt_labels"),
    ("Medium", "e^x", "-1", "stud"),
    ("Medium", "tan(x)", "-pi/6", "all"),
    ("Medium", "tan(x)", "-pi/6", "allkey"),
    ("Medium", "tan(x)", "-pi/6", "print"),
    ("Medium", "tan(x)", "-pi/6", "pt_labels"),
    ("Medium", "tan(x)", "-pi/6", "secant_triangle"),
    ("Medium", "tan(x)", "-pi/6", "stud"),
    ("Small", "tan(x)", "-pi/6", "all"),
    ("Small", "tan(x)", "-pi/6", "allkey"),
    ("Small", "tan(x)", "-pi/6", "print"),
    ("Small", "tan(x)", "-pi/6", "stud"),
];

/// Every example renders with no note and every label in a clear place.
#[test]
fn examples_draw_cleanly() {
    let d = def();
    let exs = crate::figures::examples_for(ID);
    assert!(exs.len() >= 6);
    for e in exs {
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        let o = build(&p);
        assert!(o.notes.is_empty() && o.crowded.is_empty(), "{}: {:?} {:?}", e.name, o.notes, o.crowded);
        assert!(faults(&o.svg).is_empty(), "{}: {:?}", e.name, faults(&o.svg));
    }
}

// ───────────────────────── the panel, versions, purity ─────────────────────────

/// A control shows in the panel exactly when changing it changes what is drawn or said (the render, which holds the key,
/// and the notes), in ordinary states and degenerate ones: every hidden control does nothing.
#[test]
fn controls_show_exactly_when_they_act() {
    let d = def();
    let states: Vec<Vec<(&str, Value)>> = vec![
        // the third check's: P off the axes beside a pole (Points P and Q acts through Q's label); a stacked power; a step Δt
        vec![("func", s("tan(x)")), ("a", s("pi/2+0.000001")), ("h", s("0.1")), ("xmin", f(0.0)), ("xmax", f(3.0)), ("ymin", f(-10.0)), ("ymax", f(10.0))],
        vec![("func", s("x^(1/3)^2")), ("a", s("1")), ("h", s("1")), ("tangent_eq", s("Slope-intercept")), ("worked", b(true)), ("key_line", b(true)), ("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-1.0)), ("ymax", f(3.0))],
        vec![("func", s("-4.9t^2+19.6t")), ("var", s("t")), ("fname", s("h")), ("worked", b(true)), ("xmin", f(0.0)), ("xmax", f(4.0)), ("ymin", f(0.0)), ("ymax", f(25.0))],
        vec![("func", s("(-2)^x")), ("a", s("2")), ("xmin", f(-1.0)), ("xmax", f(4.0))],
        vec![],
        vec![("show_answers", b(false))],
        vec![("show_answers", b(false)), ("student_copy", s("Curve and P only, to draw the tangent"))],
        vec![("show_secant", b(false))],
        vec![("show_tangent", b(false)), ("normal", s("With its slope"))],
        vec![("show_tangent", b(false)), ("key_line", b(true))],
        vec![("show_tangent", b(false)), ("normal", s("With its equation")), ("tangent_eq", s("Point-slope"))],
        vec![("show_tangent", b(false)), ("normal", s("With its equation")), ("more_a", s("1, 0.5"))],
        vec![("show_tangent", b(false)), ("show_secant", b(false))],
        vec![("slope_form", s("Decimals")), ("func", s("ln(x)")), ("xmin", f(-1.0))],
        vec![("title_mode", s("None")), ("show_slopes", b(false))],
        vec![("func", s("abs(x)")), ("a", s("0"))],
        vec![("func", s("abs(x)")), ("a", s("0")), ("show_answers", b(false)), ("student_copy", s("Curve and P only, to draw the tangent"))],
        vec![("func", s("sqrt(x)")), ("a", s("0")), ("xmin", f(-1.0))],
        vec![("func", s("sin x"))],
        vec![("a", s("9"))],
        vec![("h", s("9"))],
        vec![("func", s("x^(1/3)")), ("a", s("0")), ("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-2.0)), ("ymax", f(2.0))],
        vec![("grid", s("None")), ("numbers", b(false))],
        vec![("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-1.0)), ("ymax", f(5.0)), ("func", s("x^2 - 4x + 3")), ("a", s("2"))],
        vec![("func", s("tan(x)")), ("a", s("pi/4")), ("h", s("pi/4")), ("xunits", s("Multiples of \u{3c0}")), ("xmin", f(-0.5)), ("xmax", f(0.75)), ("xstep", f(0.25)), ("ymin", f(-4.0)), ("ymax", f(4.0)), ("key_line", b(true)), ("table", b(true))],
    ];
    let mut wrong = vec![];
    for st in &states {
        let p = with(st);
        let (base, base_notes) = ((d.render)(&p), tangent::warnings(&p));
        for pd in (d.params)() {
            if pd.key == "label_offsets" { continue; }
            let alts: Vec<Value> = match &pd.kind {
                crate::figures::Kind::Bool => vec![b(!p.b(pd.key))],
                crate::figures::Kind::Choice(o) => o.iter().filter(|o| **o != p.s(pd.key)).map(|o| s(o)).collect(),
                crate::figures::Kind::Int { .. } => vec![Value::I(p.i(pd.key) + 2), Value::I((p.i(pd.key) - 1).max(0))],
                crate::figures::Kind::Float { .. } => vec![f(p.f(pd.key) + if pd.key.contains("min") { -1.0 } else { 1.0 }), f(p.f(pd.key) * 0.5)],
                _ => ["", "2", "1, 0.5", "x^3", "Hello", "-1", "g", "t", "u", "time", "0.5"].iter().filter(|v| **v != p.s(pd.key)).map(|v| s(v)).collect(),
            };
            let changed = alts.iter().any(|v| { let mut q = p.clone(); q.0.insert(pd.key.to_string(), v.clone()); q.normalize(&(d.params)(), false); (d.render)(&q) != base || tangent::warnings(&q) != base_notes });
            if crate::figures::param_relevant(ID, pd.key, &p) != changed { wrong.push(format!("{} shown={} acts={} in {:?}", pd.key, !changed, changed, st)); }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn render_is_pure_and_student_copies_hide_the_answers() {
    let d = def();
    let p = with(&[("worked", b(true)), ("tangent_eq", s("Slope-intercept")), ("secant_eq", s("Slope-intercept")), ("table", b(true)), ("more_h", s("0.5")), ("secant_triangle", b(true)), ("tangent_triangle", b(true)), ("normal", s("With its equation")), ("key_line", b(true))]);
    assert_eq!((d.render)(&p), (d.render)(&p));
    let (stu, key) = crate::figures::answer_pair(&d, &p).unwrap();
    let st = texts(&stu, "panel").join(" | ") + &crate::figures::math::flat_b9::text_boxes(&stu).into_iter().map(|t| t.0).collect::<Vec<_>>().join(" | ");
    // the answers: f'(1) = 2, slope 3 and 2.5, y = 2x - 1, y = 3x - 2, normal y = -(1/2)x + 3/2, Δy = 3, the key line, rise 2
    for a in ["f\u{2032}(1) = 2", "= 3", "2.5", "y = 2x", "y = 3x", "3/2", "\u{394}y = 3", "Key"] { assert!(!st.contains(a), "student copy shows {a}: {st}"); }
    assert!(texts(&stu, "key").is_empty() && stu.matches(r#"data-tsl="blank""#).count() >= 6);
    let kt = texts(&key, "panel").join(" | ");
    for a in ["f\u{2032}(1) = 2", "= 3", "2.5", "y = 2x", "y = 3x"] { assert!(kt.contains(a), "key lacks {a}: {kt}"); }
    // the description exported with the SVG carries no answer
    let full = def().svg(&with(&[("show_answers", b(false))]));
    let desc = full.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
    assert!(!desc.contains('2') && !desc.contains('3'), "{desc}");
    // curve and P only: no line, no Q, no slope written
    let c = build(&with(&[("show_answers", b(false)), ("student_copy", s("Curve and P only, to draw the tangent"))]));
    assert!(!c.svg.contains(r#"data-tsl="tangent""#) && !c.svg.contains(r#"data-tsl="secant0""#) && c.q_px.is_none());
    assert!(texts(&c.svg, "panel").join(" ").contains("Draw the tangent line at P(1, 1)."));
}

#[test]
fn print_is_grey_only() {
    let p = with(&[("style", s("Print")), ("more_h", s("0.5, 0.1")), ("normal", s("With its slope")), ("more_a", s("-1")), ("secant_triangle", b(true)), ("tangent_triangle", b(true)), ("h_mark", b(true)), ("table", b(true))]);
    let svg = build(&p).svg;
    for (i, _) in svg.match_indices('#') {
        let h = &svg[i + 1..];
        if h.len() >= 6 && h[..6].chars().all(|c| c.is_ascii_hexdigit()) && !h[6..].starts_with(|c: char| c.is_ascii_hexdigit()) {
            let c: Vec<u8> = (0..3).map(|k| u8::from_str_radix(&h[2 * k..2 * k + 2], 16).unwrap()).collect();
            assert!(c[0] == c[1] && c[1] == c[2], "colour #{} in Print", &h[..6]);
        }
    }
}

#[test]
fn extremes_stay_bounded() {
    for o in [
        vec![("xmin", f(-1000.0)), ("xmax", f(1000.0)), ("ymin", f(-1000.0)), ("ymax", f(1000.0)), ("xstep", f(0.05)), ("ystep", f(0.05)), ("minor", Value::I(10))],
        vec![("a", s("1000000")), ("h", s("-1000000"))],
        vec![("h", s("0.000001")), ("more_h", s("1, 2, 3, 0.5, 0.25, 0.1, 0.01"))],
        vec![("func", s(&"x+".repeat(150))), ("label_size", s("Large"))],
        vec![("more_a", s("-2, -1, 0, 2, 3, 4"))],
    ] {
        let o2 = build(&with(&o));
        assert!(o2.svg.len() < 3_000_000, "{:?}", o.iter().map(|x| x.0).collect::<Vec<_>>());
        assert!(!o2.svg.contains("NaN") && !o2.svg.contains("inf"), "{o:?}");
        let (w, h) = crate::figures::svg_size(&o2.svg);
        assert!(w < 2500.0 && h < 2500.0, "{w} {h}");
    }
}

// ───────────────────────── drags and links ─────────────────────────

/// P and Q are dragged through the workspace's own hit test.
#[test]
fn drag_p_and_q_through_the_hit_test() {
    use crate::figures::interact::{handles_for, pick};
    let d = def();
    let p = start_params(&d);
    let size = crate::figures::svg_size(&(d.render)(&p));
    let o = build(&p);
    let g = o.layout.as_ref().unwrap();
    let px = |x: f64| g.ml + (x - g.x0) * g.ux;
    let hs = handles_for(ID, &p, size);
    let pp = o.p_px.unwrap();
    for (dx, dy) in [(0.0, 0.0), (3.0, -3.0), (-4.0, 2.0)] {
        let i = pick(&hs, (pp.0 + dx, pp.1 + dy), 1.0).expect("P is under the pointer");
        assert!(hs[i].tip.contains("Drag P"), "{}", hs[i].tip);
    }
    let i = pick(&hs, pp, 1.0).unwrap();
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, (px(2.0), pp.1));
    assert_eq!(q.s("a"), "2");
    // P at 2, Q at 3: the secant slope (9 - 4)/1 = 5 and f'(2) = 4
    let l = panel(&q);
    assert!(has(&l, "f\u{2032}(2) = 4") && has(&l, "m = 5"), "{l:?}");
    let qq = o.q_px.unwrap();
    let i = pick(&hs, qq, 1.0).unwrap();
    assert!(hs[i].tip.contains("Drag Q"));
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, (px(1.5), qq.1));
    assert_eq!(q.s("h"), "0.5");
    // dragged onto P, Q keeps its side at one step (h is never 0)
    (hs[i].drag.as_ref().unwrap())(&mut q, (px(1.0), qq.1));
    assert_eq!(q.s("h"), "0.1");
    // a typed fraction is kept when the drag ends where it began
    let p2 = with(&[("a", s("1/3"))]);
    let hs2 = handles_for(ID, &p2, size);
    let mut q2 = p2.clone();
    let i = pick(&hs2, build(&p2).p_px.unwrap(), 1.0).unwrap();
    (hs2[i].drag.as_ref().unwrap())(&mut q2, hs2[i].pos);
    assert_eq!(q2.s("a"), "1/3");
    // on a π axis P snaps to π/12 steps and is written in π
    let p3 = with(&[("func", s("sin(x)")), ("a", s("0")), ("h", s("pi/2")), ("xunits", s("Multiples of \u{3c0}")), ("xmin", f(-1.0)), ("xmax", f(1.0)), ("xstep", f(0.25)), ("ymin", f(-2.0)), ("ymax", f(2.0))]);
    let o3 = build(&p3);
    let g3 = o3.layout.as_ref().unwrap();
    let hs3 = handles_for(ID, &p3, crate::figures::svg_size(&o3.svg));
    let i = pick(&hs3, o3.p_px.unwrap(), 1.0).unwrap();
    let mut q3 = p3.clone();
    (hs3[i].drag.as_ref().unwrap())(&mut q3, (g3.ml + (std::f64::consts::PI / 4.0 - g3.x0) * g3.ux, 0.0));
    assert_eq!(q3.s("a"), "pi/4");
    // a moved label is drawn where it was dropped
    let lab = hs.iter().find(|h| h.label_drag).unwrap();
    let mut q4 = p.clone();
    (lab.drag.as_ref().unwrap())(&mut q4, (lab.pos.0 + 20.0, lab.pos.1 + 10.0));
    assert!(!q4.s("label_offsets").is_empty() && (d.render)(&q4).contains("translate(20.00 10.00)"));
}

#[test]
fn saved_specs_and_share_links_round_trip() {
    use crate::figures::share;
    let d = def();
    let base = start_params(&d);
    let mut cases: Vec<Params> = vec![base.clone(), with(&[("func", s("-16t^2 + 64t")), ("fname", s("s")), ("var", s("t")), ("a", s("1/2")), ("h", s("-pi/4")), ("more_h", s("0.5, 0.1")), ("label_offsets", s("P:5,-3")), ("style", s("Print")), ("show_answers", b(false))])];
    for e in crate::figures::examples_for(ID) { let mut p = Params::defaults(&(d.params)()); for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); } p.normalize(&(d.params)(), false); cases.push(p); }
    for p in cases {
        let link = share::encode(ID, &p, &base);
        let (id, m) = share::decode(&link).unwrap();
        assert_eq!(id, ID);
        let back = share::link_params(&d, m, share::version(&link));
        assert_eq!((d.render)(&back), (d.render)(&p), "{link}");
        // a saved spec (every param) reads back the same
        let spec = crate::model::FigureSpec { id: ID.to_string(), params: p.0.iter().map(|(k, v)| (k.clone(), crate::figures::spec::value_to_json(v))).collect() };
        let saved = crate::figures::spec::params_from_spec(&d, &spec);
        assert_eq!((d.render)(&saved), (d.render)(&p));
    }
}

// ───────────────────────── round 1: the first check's defects ─────────────────────────

/// A value at an exact pole is undefined, never an enormous number: P, Q, every extra h and every extra a, for tan, sec,
/// csc, cot, 1/x forms and rational poles. (Each pole worked by hand: tan, sec at odd multiples of π/2; csc, cot at
/// multiples of π; 1/(x − c) at c.)
#[test]
fn poles_are_undefined() {
    let pi = |o: Vec<(&'static str, Value)>| { let mut v = vec![("xunits", s("Multiples of \u{3c0}")), ("xmin", f(-1.0)), ("xmax", f(1.0)), ("xstep", f(0.25)), ("ymin", f(-4.0)), ("ymax", f(4.0))]; v.extend(o); v };
    // (case, params, a note that must be said)
    let cases: Vec<(&str, Vec<(&'static str, Value)>, &str)> = vec![
        ("tan Q at π/2", pi(vec![("func", s("tan(x)")), ("a", s("pi/4")), ("h", s("pi/4"))]), "f(\u{3c0}/2) is undefined, so there is no secant"),
        ("tan P at π/2", pi(vec![("func", s("tan(x)")), ("a", s("pi/2"))]), "f(\u{3c0}/2) is undefined"),
        ("tan P at −π/2", pi(vec![("func", s("tan(x)")), ("a", s("-pi/2"))]), "undefined"),
        ("sec Q at π/2", pi(vec![("func", s("sec(x)")), ("a", s("0")), ("h", s("pi/2"))]), "no secant"),
        ("1/cos P at 3π/2", pi(vec![("func", s("1/cos(x)")), ("a", s("3pi/2")), ("xmax", f(2.0))]), "undefined"),
        ("csc P at π", pi(vec![("func", s("csc(x)")), ("a", s("pi"))]), "undefined"),
        ("cot P at 0", pi(vec![("func", s("cot(x)")), ("a", s("0"))]), "undefined"),
        ("cot Q at π", pi(vec![("func", s("cot(x)")), ("a", s("pi/2")), ("h", s("pi/2"))]), "no secant"),
        ("1/sin Q at π", pi(vec![("func", s("1/sin(x)")), ("a", s("pi/2")), ("h", s("pi/2"))]), "no secant"),
        ("tan(2x) P at π/4", pi(vec![("func", s("tan(2x)")), ("a", s("pi/4"))]), "undefined"),
        ("1/x P at 0", vec![("func", s("1/x")), ("a", s("0"))], "undefined"),
        ("1/(x − 1) Q at 1", vec![("func", s("1/(x - 1)")), ("a", s("2")), ("h", s("-1"))], "no secant"),
        ("1/(x − 1/3) P at 1/3", vec![("func", s("1/(x - 1/3)")), ("a", s("1/3"))], "undefined"),
        ("1/(3x − 1) P at 1/3", vec![("func", s("1/(3x - 1)")), ("a", s("1/3"))], "undefined"),
        ("1/(x^2 − 4) P at 2", vec![("func", s("1/(x^2 - 4)")), ("a", s("2"))], "undefined"),
        ("(x + 1)/(x − 1/2) Q at 1/2", vec![("func", s("(x + 1)/(x - 1/2)")), ("a", s("1")), ("h", s("-1/2"))], "no secant"),
        ("tan extra h at π/2", pi(vec![("func", s("tan(x)")), ("a", s("0")), ("h", s("pi/4")), ("more_h", s("pi/2, pi/6"))]), "no secant for h = \u{3c0}/2"),
        ("tan extra a at π/2", pi(vec![("func", s("tan(x)")), ("a", s("0")), ("more_a", s("pi/2"))]), "More tangents: f(\u{3c0}/2) is undefined"),
        ("1/x extra a at 0", vec![("func", s("1/x")), ("a", s("1")), ("more_a", s("0"))], "More tangents: f(0) is undefined"),
    ];
    assert!(cases.len() >= 15);
    for (name, o, note) in cases {
        let mut o = o;
        o.extend([("table", b(true)), ("key_line", b(true))]);
        let p = with(&o);
        let out = build(&p);
        let n = out.notes.join(" | ");
        assert!(n.contains(note), "{name}: {n}");
        // nothing enormous is written anywhere: no number of seven or more digits
        let written = texts(&out.svg, "panel").join(" ") + &texts(&out.svg, "key").join(" ");
        assert!(!written.chars().collect::<Vec<_>>().windows(7).any(|w| w.iter().all(|c| c.is_ascii_digit())), "{name}: {written}");
        assert!(!out.svg.contains("NaN") && !out.svg.contains("inf"), "{name}");
    }
    // the tan case of the check: no secant drawn, none in the table or the key
    let p = with(&[("func", s("tan(x)")), ("a", s("pi/4")), ("h", s("pi/4")), ("xunits", s("Multiples of \u{3c0}")), ("xmin", f(-0.5)), ("xmax", f(0.75)), ("xstep", f(0.25)), ("ymin", f(-4.0)), ("ymax", f(4.0)), ("key_line", b(true)), ("table", b(true))]);
    let o = build(&p);
    assert!(!o.svg.contains(r#"data-tsl="secant0""#) && o.q_px.is_none());
    assert!(!texts(&o.svg, "key").join(" ").contains("secant"), "{:?}", texts(&o.svg, "key"));
    // a secant across a pole is drawn (two points make a line) and said to join separate pieces
    let n = tangent::warnings(&with(&[("func", s("tan(x)")), ("a", s("pi/4")), ("h", s("pi/2")), ("xunits", s("Multiples of \u{3c0}")), ("xmin", f(-0.5)), ("xmax", f(1.0)), ("xstep", f(0.25)), ("ymin", f(-4.0)), ("ymax", f(4.0))])).join(" | ");
    assert!(n.contains("separate pieces"), "{n}");
    // and an ordinary steep value is not a pole: e^x at 20 (4.85e8), tan near π/2 but not at it
    assert!(tangent::fval("e^x", 20.0).is_some_and(|v| (v - 20f64.exp()).abs() < 1.0));
    assert!(tangent::fval("tan(x)", 1.57).is_some());
}

/// A value typed as a decimal stays the decimal typed, its slope a decimal; one typed as a fraction stays a fraction; a
/// nonzero h is never written as 0. Worked by hand: x^2 at 1, slope 2 + h: 5/2, 2.1, 2.01, 2.001, 2.0001.
#[test]
fn typed_values_keep_their_form() {
    let p = with(&[("h", s("1/2")), ("more_h", s("0.1, 0.01, 0.001, 0.0001")), ("table", b(true)), ("key_line", b(true))]);
    let svg = build(&p).svg;
    let l = texts(&svg, "panel");
    for w in ["1/2", "5/2", "0.1", "2.1", "0.01", "2.01", "0.001", "2.001", "0.0001", "2.0001"] { assert!(l.iter().any(|x| x == w), "{w}: {l:?}"); }
    assert!(!l.iter().any(|x| x == "0" || x.contains("1/10") || x.contains("21/10")), "{l:?}");
    let k = texts(&svg, "key").join(" ");
    assert!(k.contains("(h = 0.0001)") && k.contains("2.0001") && !k.contains("(h = 0)"), "{k}");
    // pi/4 and 0.1: Q at π/4 + 0.1 (a decimal beside π keeps its decimal); a typed e is read
    let l = panel(&with(&[("func", s("sin(x)")), ("a", s("pi/4")), ("h", s("0.1")), ("worked", b(true)), ("xunits", s("Multiples of \u{3c0}")), ("xmin", f(-1.0)), ("xmax", f(1.0)), ("xstep", f(0.25)), ("ymin", f(-2.0)), ("ymax", f(2.0))]));
    assert!(has(&l, &format!("= [f(\u{3c0}/4 + 0.1) {M} f(\u{3c0}/4)]/0.1")), "{l:?}");
    // the working with a rounded value is all in decimals, with places enough for the difference (0.067 needs 4):
    // sin(π/4 + 0.1) = 0.7742, sin(π/4) = 0.7071, 0.0671/0.1 = 0.67
    assert!(has(&l, &format!("\u{2248} (0.7742 {M} 0.7071)/0.1 \u{2248} 0.67")), "{l:?}");
    let l = panel(&with(&[("func", s("ln(x)")), ("a", s("e")), ("tangent_eq", s("Slope-intercept")), ("xmin", f(0.0)), ("xmax", f(5.0)), ("ymin", f(-2.0)), ("ymax", f(3.0))]));
    // ln at e: slope 1/e, ln e = 1, tangent y = (1/e)x (intercept 1 − 1 = 0)
    assert!(has(&l, "f\u{2032}(e) = 1/e") && has(&l, "y = (1/e)x"), "{l:?}");
    // x², √x and π typed as symbols are read
    for (typed, want) in [("x²", "f(x) = x^2"), ("√x", "f(x) = \u{221a}x"), ("πx", "f(x) = \u{3c0}x")] {
        let n = tangent::warnings(&with(&[("func", s(typed)), ("xmin", f(0.0))]));
        assert!(n.is_empty(), "{typed}: {n:?}");
        let pl = panel(&with(&[("func", s(typed)), ("xmin", f(0.0))]));
        assert!(pl[0].replace(['\u{b2}'], "2").contains(&want.replace("^", "")), "{typed}: {pl:?}");
    }
}

/// Point-slope is exact when the slope and the point are, even where the intercept is not. By hand: sin at π/3, slope
/// cos(π/3) = 1/2, point (π/3, √3/2); normal slope −2. tan at π/4: slope sec²(π/4) = 2, point (π/4, 1). cos at π/3: slope
/// −sin(π/3) = −√3/2, point (π/3, 1/2). sin at π/6: slope √3/2, point (π/6, 1/2). e^x at 1: slope e, point (1, e), normal
/// slope −1/e.
#[test]
fn point_slope_is_exact_where_it_can_be() {
    let pi = |func: &str, a: &str| with(&[("func", s(func)), ("a", s(a)), ("h", s("pi/6")), ("tangent_eq", s("Point-slope")), ("normal", s("With its equation")), ("xunits", s("Multiples of \u{3c0}")), ("xmin", f(-1.0)), ("xmax", f(1.0)), ("xstep", f(0.25)), ("ymin", f(-2.0)), ("ymax", f(2.0)), ("key_line", b(true))]);
    let p = pi("sin(x)", "pi/3");
    let svg = build(&p).svg;
    let (l, k) = (texts(&svg, "panel"), texts(&svg, "key").join(" "));
    assert!(has(&l, &format!("y {M} \u{221a}3/2 = (1/2)(x {M} \u{3c0}/3)")) && has(&l, &format!("y {M} \u{221a}3/2 = {M}2(x {M} \u{3c0}/3)")), "{l:?}");
    assert!(k.contains(&format!("y {M} \u{221a}3/2 = (1/2)(x {M} \u{3c0}/3)")), "{k}");
    let l = panel(&pi("tan(x)", "pi/4"));
    assert!(has(&l, &format!("y {M} 1 = 2(x {M} \u{3c0}/4)")), "{l:?}");
    let l = panel(&pi("cos(x)", "pi/3"));
    assert!(has(&l, &format!("y {M} 1/2 = {M}(\u{221a}3/2)(x {M} \u{3c0}/3)")), "{l:?}");
    let l = panel(&pi("sin(x)", "pi/6"));
    assert!(has(&l, &format!("y {M} 1/2 = (\u{221a}3/2)(x {M} \u{3c0}/6)")), "{l:?}");
    let l = panel(&with(&[("func", s("e^x")), ("a", s("1")), ("normal", s("With its equation")), ("tangent_eq", s("Point-slope")), ("xmin", f(-2.0)), ("xmax", f(3.0)), ("ymin", f(-1.0)), ("ymax", f(8.0))]));
    assert!(has(&l, &format!("y {M} e = e(x {M} 1)")) && has(&l, &format!("y {M} e = {M}(1/e)(x {M} 1)")), "{l:?}");
    // with no form chosen, the key takes slope-intercept where it is exact (here the intercept √3/2 − π/6 is derived exactly)
    let p = with(&[("func", s("sin(x)")), ("a", s("pi/3")), ("h", s("pi/6")), ("normal", s("With its equation")), ("xunits", s("Multiples of \u{3c0}")), ("xmin", f(-1.0)), ("xmax", f(1.0)), ("xstep", f(0.25)), ("ymin", f(-2.0)), ("ymax", f(2.0)), ("key_line", b(true))]);
    let svg = build(&p).svg;
    assert!(texts(&svg, "key").join(" ").contains(&format!("tangent y = (1/2)x + \u{221a}3/2 {M} \u{3c0}/6")), "{:?}", texts(&svg, "key"));
    // written forms: y = x − 2 (not y = (x − 2)); y = −4 for a horizontal line in point-slope; |x| for abs(x)
    let l = panel(&with(&[("func", s("x^2 - 4x")), ("a", s("2")), ("h", s("1")), ("secant_eq", s("Point-slope")), ("tangent_eq", s("Point-slope")), ("xmin", f(-1.0)), ("xmax", f(5.0)), ("ymin", f(-5.0)), ("ymax", f(5.0))]));
    // tangent at the vertex (2, −4): y = −4; secant to (3, −3): slope 1, y + 4 = x − 2
    assert!(has(&l, &format!("y = {M}4")) && has(&l, &format!("y + 4 = x {M} 2")), "{l:?}");
    let l = panel(&with(&[("func", s("x^2")), ("a", s("2")), ("h", s("-2")), ("secant_eq", s("Point-slope")), ("xmin", f(-1.0)), ("xmax", f(4.0)), ("ymin", f(-1.0)), ("ymax", f(8.0))]));
    // secant through (2, 4) and (0, 0): slope 2, y − 4 = 2(x − 2)
    assert!(has(&l, &format!("y {M} 4 = 2(x {M} 2)")), "{l:?}");
    let l = panel(&with(&[("func", s("abs(x - 1)")), ("a", s("2"))]));
    assert!(l[0].contains("|x \u{2212} 1|") || l[0].contains("|x - 1|"), "{l:?}");
    // a sum is written exactly: a = √2, h = 1: Q(√2 + 1, 2√2 + 3), worked (2√2 + 3 − 2)/1 = 2√2 + 1
    let o = build(&with(&[("func", s("x^2")), ("a", s("sqrt(2)")), ("h", s("1")), ("worked", b(true)), ("pt_labels", s("Letters and coordinates")), ("xmin", f(-1.0)), ("xmax", f(4.0)), ("ymin", f(-1.0)), ("ymax", f(12.0))]));
    let labs: Vec<String> = my_labels(&o.svg).into_iter().map(|x| x.1).collect();
    assert!(labs.iter().any(|x| x == "Q(\u{221a}2 + 1, 2\u{221a}2 + 3)") && labs.iter().any(|x| x == "P(\u{221a}2, 2)"), "{labs:?}");
    assert!(has(&texts(&o.svg, "panel"), "= 2\u{221a}2 + 1"));
    // 2/π, 3/π and e − 1 are exact forms
    assert_eq!(tx("2/pi", false).as_deref(), Some("2/\u{3c0}"));
    // a rounded coordinate is said with ≈: e^x at 0.1, f = 1.105…
    let o = build(&with(&[("func", s("e^x")), ("a", s("0.1")), ("pt_labels", s("Letters and coordinates")), ("xmin", f(-2.0)), ("xmax", f(3.0)), ("ymin", f(-1.0)), ("ymax", f(8.0))]));
    assert!(my_labels(&o.svg).iter().any(|x| x.1 == "P \u{2248} (0.1, 1.11)"), "{:?}", my_labels(&o.svg));
}

/// x^(1/3) and x^(2/3) for negative x: read as the real root, so the curve, the values and the special points are right.
/// By hand: (−1)^(1/3) = −1, slope (1/3)x^(−2/3) = 1/3 at −1; at 0 the slope grows without bound (vertical tangent);
/// x^(2/3) has a cusp at 0; (−8)^(1/3) = −2.
#[test]
fn odd_roots_of_negative_numbers() {
    let w = [("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-2.0)), ("ymax", f(2.0)), ("ystep", f(0.5))];
    let with_w = |o: &[(&'static str, Value)]| { let mut v = w.to_vec(); v.extend(o.iter().cloned()); with(&v) };
    let n = tangent::warnings(&with_w(&[("func", s("x^(1/3)")), ("a", s("0"))])).join(" | ");
    assert!(n.contains("vertical") && !n.contains("one side"), "{n}");
    let l = panel(&with_w(&[("func", s("x^(1/3)")), ("a", s("-1")), ("h", s("2"))]));
    assert!(has(&l, &format!("f\u{2032}({M}1) = 1/3")) && has(&l, "slope m = 1"), "{l:?}");
    let n = tangent::warnings(&with_w(&[("func", s("x^(2/3)")), ("a", s("0"))])).join(" | ");
    assert!(n.contains("cusp"), "{n}");
    let e = tangent::expr_in_x("x^(1/3)", "f", 'x').unwrap();
    assert!((tangent::fval(&e, -8.0).unwrap() + 2.0).abs() < 1e-12 && tangent::fval(&e, 0.0) == Some(0.0));
    let e = tangent::expr_in_x("(x - 1)^(2/3) + x^(-1/5)", "f", 'x').unwrap();
    // (−1 − 1)^(2/3) = 4^(1/3) = 1.5874…, (−1)^(−1/5) = −1
    assert!((tangent::fval(&e, -1.0).unwrap() - (4f64.cbrt() - 1.0)).abs() < 1e-12);
    // the curve is drawn for x < 0: the grid's polyline reaches the left edge of the window
    let svg = build(&with_w(&[("func", s("x^(1/3)")), ("a", s("1"))])).svg;
    let o = build(&with_w(&[("func", s("x^(1/3)")), ("a", s("1"))]));
    let g = o.layout.as_ref().unwrap();
    let left = g.ml + 2.0;
    assert!(strokes(&svg).iter().any(|(k, sg)| k == "curve" && sg.0 .0 < left), "the curve stops at 0");
}

/// The curve-and-P copy never asks for a tangent that does not exist: at a corner, a cusp or an end of the domain it asks
/// whether there is one.
#[test]
fn no_impossible_student_prompt() {
    for (func, a) in [("abs(x)", "0"), ("sqrt(x)", "0"), ("x^(2/3)", "0")] {
        let l = panel(&with(&[("func", s(func)), ("a", s(a)), ("xmin", f(-3.0)), ("show_answers", b(false)), ("student_copy", s("Curve and P only, to draw the tangent"))])).join(" | ");
        assert!(!l.contains("Draw the tangent") && !l.contains("f\u{2032}") && l.contains("Is there a tangent line at P(0, 0)?"), "{func}: {l}");
    }
    let l = panel(&with(&[("show_answers", b(false)), ("student_copy", s("Curve and P only, to draw the tangent"))])).join(" | ");
    assert!(l.contains("Draw the tangent line at P(1, 1)."));
}

/// A drag keeps P and Q in the window: dragged to where the curve leaves it, the point stops where the curve is still in.
#[test]
fn drags_stay_in_the_window() {
    use crate::figures::interact::handles_for;
    let d = def();
    let p = start_params(&d);
    let o = build(&p);
    let g = o.layout.as_ref().unwrap();
    let px = |x: f64| g.ml + (x - g.x0) * g.ux;
    let hs = handles_for(ID, &p, crate::figures::svg_size(&o.svg));
    let ph = hs.iter().find(|h| h.tip.contains("Drag P")).unwrap();
    for target in [3.5, 4.0, 100.0] {
        let mut q = p.clone();
        (ph.drag.as_ref().unwrap())(&mut q, (px(target), 0.0));
        // y = x^2 stays under 10 for x ≤ √10 = 3.16: P stops at 3.1
        let a: f64 = q.s("a").parse().unwrap();
        assert!(a * a <= 10.0 + 1e-9 && a >= 3.0, "{target}: a = {a}");
        assert!(build(&q).p_px.is_some() && handles_for(ID, &q, crate::figures::svg_size(&build(&q).svg)).iter().any(|h| h.tip.contains("Drag P")));
    }
    let qh = hs.iter().find(|h| h.tip.contains("Drag Q")).unwrap();
    let mut q = p.clone();
    (qh.drag.as_ref().unwrap())(&mut q, (px(3.9), 0.0));
    let h: f64 = q.s("h").parse().unwrap();
    assert!((1.0 + h).powi(2) <= 10.0 + 1e-9, "h = {h}");
    assert!(build(&q).q_px.is_some());
}

/// A horizontal secant has no rise: its slope triangle is not drawn and the panel says why. By hand: −x² + 6x − 5 at 2 and 4
/// is 3 at both, so Δy = 0.
#[test]
fn a_flat_secant_has_no_triangle() {
    let o = build(&with(&[("func", s("-x^2 + 6x - 5")), ("a", s("2")), ("h", s("2")), ("secant_triangle", b(true)), ("xmin", f(-1.0)), ("xmax", f(6.0)), ("ymin", f(-4.0)), ("ymax", f(5.0))]));
    assert!(!o.svg.contains(r#"data-leg="rise""#) && o.notes.join(" ").contains("horizontal"), "{:?}", o.notes);
    assert!(has(&texts(&o.svg, "panel"), "slope m = 0"));
}

// ───────────────────────── round 2: the second check's numerical faults ─────────────────────────

/// A written number read back (√n, π, e, fractions, −): its value.
fn read_back(t: &str) -> Option<f64> {
    let mut s = String::new();
    let cs: Vec<char> = t.trim().chars().collect();
    let mut i = 0;
    while i < cs.len() {
        match cs[i] {
            '\u{221a}' => { s.push_str("sqrt("); i += 1; while i < cs.len() && cs[i].is_ascii_digit() { s.push(cs[i]); i += 1; } s.push(')'); continue; }
            '\u{3c0}' => s.push_str("pi"),
            '\u{2212}' => s.push('-'),
            c => s.push(c),
        }
        i += 1;
    }
    ex::eval(&ex::parse(&s)?, 0.0).is_finite().then(|| ex::eval(&ex::parse(&s).unwrap(), 0.0))
}

/// f′(a) is the derivative to every digit shown, however near a pole of f: 60 points 0.2, 0.05, 0.01 and 0.001 either side
/// of poles of tan, sec, csc, cot, 1/(x − c) and 1/(x − c)², and 0.2 … 0.001 from the end of ln's and √'s domains, each
/// against its closed-form derivative (sec² x; sec x tan x; −csc x cot x; −csc² x; −1/(x − c)²; −2/(x − c)³; 1/x; 1/(2√x)),
/// written to 6 places.
#[test]
fn derivative_is_right_near_poles() {
    use std::f64::consts::PI;
    type D = fn(f64) -> f64;
    let fams: Vec<(&str, f64, D)> = vec![
        ("tan(x)", PI / 2.0, |x| 1.0 / x.cos().powi(2)),
        ("tan(x)", -PI / 2.0, |x| 1.0 / x.cos().powi(2)),
        ("sec(x)", PI / 2.0, |x| x.tan() / x.cos()),
        ("csc(x)", PI, |x| -1.0 / (x.sin() * x.tan())),
        ("cot(x)", PI, |x| -1.0 / x.sin().powi(2)),
        ("1/(x - 0.7)", 0.7, |x| -1.0 / (x - 0.7).powi(2)),
        ("1/(x - 0.7)^2", 0.7, |x| -2.0 / (x - 0.7).powi(3)),
    ];
    let mut n = 0;
    let mut bad = vec![];
    let mut check = |func: &str, a: f64, d: fn(f64) -> f64| {
        let at = format!("{a:.12}");
        let a: f64 = at.parse().unwrap();
        let p = with(&[("func", s(func)), ("a", s(&at)), ("show_secant", b(false)), ("slope_form", s("Decimals")), ("dec", Value::I(6)), ("xmin", f((a - 3.0).floor())), ("xmax", f((a + 3.0).ceil())), ("ymin", f(-20.0)), ("ymax", f(20.0)), ("ystep", f(5.0))]);
        let l = panel(&p);
        let Some(line) = l.iter().find(|x| x.contains('\u{2032}')) else { bad.push(format!("{func} at {at}: no slope written: {l:?}")); return };
        let val = line.rsplit([' ']).next().and_then(read_back);
        let want = d(a);
        match val {
            // (to half a unit of the last place written: beside a pole only the digits left standing by rounding a to 16 figures
            // are written)
            Some(v) if (v - want).abs() <= 0.5 * 10f64.powi(-(line.rsplit(' ').next().unwrap().split_once('.').map(|(_, f)| f.len()).unwrap_or(0) as i32)).max(0.5e-6) + 1e-12 * want.abs() => {}
            _ => bad.push(format!("{func} at {at}: wrote {line}, true {want}")),
        }
        n += 1;
    };
    for (func, pole, d) in &fams {
        for dist in [0.2, 0.05, 0.01, 0.001] { for side in [-1.0, 1.0] { check(func, pole + side * dist, *d); } }
    }
    for dist in [0.2, 0.05, 0.01, 0.001] { check("ln(x)", dist, |x| 1.0 / x); check("sqrt(x)", dist, |x| 0.5 / x.sqrt()); }
    // the second check's cases: tan at 1.5 (199.85…), 1.55, 1.57; sec at 1.5; 1/(x − 0.05) at 0.1 (−400); 1/(x − 0.7) at 0.65
    check("tan(x)", 1.5, |x| 1.0 / x.cos().powi(2));
    check("tan(x)", 1.55, |x| 1.0 / x.cos().powi(2));
    check("tan(x)", 1.57, |x| 1.0 / x.cos().powi(2));
    check("sec(x)", 1.5, |x| x.tan() / x.cos());
    check("1/(x - 0.05)", 0.1, |x| -1.0 / (x - 0.05).powi(2));
    check("1/(x - 0.7)", 0.65, |x| -1.0 / (x - 0.7).powi(2));
    println!("derivative points checked: {n}");
    assert!(n >= 60, "{n}");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    // and in the exact mode, 1/(x − 0.05) at 0.1 is exactly −400 (exact arithmetic on what was typed)
    let l = panel(&with(&[("func", s("1/(x-0.05)")), ("a", s("0.1")), ("h", s("0.5")), ("xmin", f(-1.0)), ("xmax", f(1.0)), ("ymin", f(-10.0)), ("ymax", f(30.0)), ("ystep", f(5.0)), ("worked", b(true)), ("tangent_eq", s("Point-slope"))]));
    assert!(has(&l, &format!("f\u{2032}(0.1) = {M}400")), "{l:?}");
}

/// An exact form is written only when derived exactly; anything else is a decimal with ≈. Over 200 decimal and mixed
/// inputs: no written value mixes a root (or π) with a decimal of more than 3 places, and every value written with "=" is
/// the true value (checked against the closed-form f and f′).
#[test]
fn exact_forms_are_only_derived() {
    type F = fn(f64) -> f64;
    let fams: Vec<(&str, F, F)> = vec![
        ("x^2", |x| x * x, |x| 2.0 * x),
        ("x^3", |x| x.powi(3), |x| 3.0 * x * x),
        ("sin(x)", f64::sin, f64::cos),
        ("cos(x)", f64::cos, |x| -x.sin()),
        ("tan(x)", f64::tan, |x| 1.0 / x.cos().powi(2)),
        ("e^x", f64::exp, f64::exp),
        ("ln(x)", f64::ln, |x| 1.0 / x),
        ("sqrt(x)", f64::sqrt, |x| 0.5 / x.sqrt()),
        ("1/x", |x| 1.0 / x, |x| -1.0 / (x * x)),
        ("1000(1.05)^x", |x| 1000.0 * 1.05f64.powf(x), |x| 1000.0 * 1.05f64.powf(x) * 1.05f64.ln()),
    ];
    let a_s = ["0.3", "1.1", "1.2", "2.5", "e", "pi/4", "sqrt(2)", "1.0001"];
    let h_s = ["0.5", "0.0001", "-0.4", "0.2"];
    let mut n = 0;
    let mut bad = vec![];
    for (func, fv, dv) in &fams {
        for at in a_s {
            for ht in h_s {
                let a = ex::eval(&ex::parse(&at.replace("pi", "pi")).unwrap(), 0.0);
                let h: f64 = ht.parse().unwrap();
                let p = with(&[("func", s(func)), ("a", s(at)), ("h", s(ht)), ("worked", b(true)), ("table", b(true)), ("key_line", b(true)), ("normal", s("With its equation")), ("tangent_eq", s("Slope-intercept")), ("secant_eq", s("Point-slope")), ("pt_labels", s("Letters and coordinates")), ("xmin", f(-1.0)), ("xmax", f(4.0)), ("ymin", f(-5.0)), ("ymax", f(30.0)), ("ystep", f(5.0))]);
                let o = build(&p);
                if o.notes.iter().any(|x| x.contains("undefined")) { continue; }
                n += 1;
                // (superscripts read as ^: 3e^2, not 3e2)
                let mut written: Vec<String> = texts(&o.svg, "panel^");
                written.extend(texts(&o.svg, "key^"));
                written.extend(my_labels(&o.svg).into_iter().map(|t| t.1));
                for line in &written {
                    // a root or π beside a decimal of 4 or more places
                    let cs: Vec<char> = line.chars().collect();
                    let mut k = 0;
                    while k < cs.len() {
                        if cs[k].is_ascii_digit() {
                            let st = k;
                            while k < cs.len() && (cs[k].is_ascii_digit() || cs[k] == '.') { k += 1; }
                            let tok: String = cs[st..k].iter().collect();
                            let places = tok.split_once('.').map(|(_, f)| f.len()).unwrap_or(0);
                            let near: String = cs[st.saturating_sub(4)..(k + 4).min(cs.len())].iter().collect();
                            if places > 3 && (near.contains('\u{221a}') || near.contains('\u{3c0}')) { bad.push(format!("{func} a={at} h={ht}: mixed form in {line}")); }
                            continue;
                        }
                        k += 1;
                    }
                }
                // every "=" slope is the true value: f′(a), and the secant's (f(a + h) − f(a))/h
                let want_d = dv(a);
                let want_m = (fv(a + h) - fv(a)) / h;
                for line in &written {
                    let checks: Vec<(&str, f64)> = vec![("\u{2032}(", want_d), ("m = ", want_m)];
                    for (marker, want) in checks {
                        if let Some(i) = line.find(marker) {
                            let rest = &line[i..];
                            if let Some(j) = rest.find(" = ") {
                                let val: String = rest[j + 3..].split(';').next().unwrap_or("").split(" (").next().unwrap_or("").trim().trim_end_matches('.').to_string();
                                if let Some(v) = read_back(&val) {
                                    if (v - want).abs() > 1e-9 * (1.0 + want.abs()) { bad.push(format!("{func} a={at} h={ht}: {line} is not {want}")); }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("exactness inputs checked: {n}");
    assert!(n >= 200, "{n}");
    assert!(bad.is_empty(), "{} faults:\n{}", bad.len(), bad.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
}

/// The figure's tree reads every expression as the grid's reader does (the same value at every x where the reader gives
/// one; nothing where it gives nothing), apart from the odd roots of negative numbers, which the tree takes as real.
#[test]
fn the_tree_reads_as_the_grid_does() {
    let exprs = ["x^2", "2x^2 - 3x + 1", "-x^2", "x^-1", "2^-1x", "2^x", "e^x", "2ex", "xe^x", "x e^(-x)", "sin(x)", "sin(x)^2", "sin^-1(x)", "arcsin(x)", "tan(x)", "sec(x)", "csc(x)", "cot(x)", "arctan(x)", "acos(x)", "arcsec(x)", "arccsc(x)", "arccot(x)", "ln(x)", "log(x)", "log_2(x)", "exp(x)", "sqrt(x)", "sqrt(x + 2)", "|x - 1|", "abs(x)*x", "1/x", "1/(x - 0.05)", "(x + 1)/(x - 1)", "x/(x^2 + 1)", "x^0.5", "x^(1/2)", "x^2/4", "3x(x - 1)", "(x - 1)(x + 2)", "4/3pi x^3", "pi x^2", "\u{3c0}x", "2.5x", ".5x", "1e2x", "x^2e3", "100e^(0.5x)", "1000(1.05)^x", "x^x", "sin(2x)cos(x)", "-16x^2 + 64x", "x^3 - 6x^2 + 9x", "5x^4 - 2", "1/(x^2 + 1)", "e^(-x^2)", "ln(x + 1)", "x ln(x)", "sin(x)/x", "(x^2 - 1)/(x - 1)"];
    let xs = [-2.5, -1.0, -0.3, 0.0, 0.25, 0.5, 1.0, 1.7, 2.0, 3.3];
    let mut bad = vec![];
    for e in exprs {
        let tree = ex::parse(e);
        for x in xs {
            let grid = super::eval(e, x).filter(|v| v.is_finite());
            let mine = tree.as_ref().map(|t| ex::eval(t, x)).filter(|v| v.is_finite());
            let same = match (grid, mine) { (Some(g), Some(m)) => (g - m).abs() <= 1e-12 * (1.0 + g.abs()), (None, None) => true, _ => false };
            if !same { bad.push(format!("{e} at {x}: grid {grid:?}, tree {mine:?}")); }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    // the one difference on purpose: (−8)^(1/3) = −2 in the tree, nothing in the reader
    assert_eq!(super::eval("x^(1/3)", -8.0).filter(|v| v.is_finite()), None);
    assert!((ex::eval(&ex::parse("x^(1/3)").unwrap(), -8.0) + 2.0).abs() < 1e-12);
}

// ───────────────────────── round 3: the third check's faults ─────────────────────────

/// A written line in the reader's spelling at a point: the variable and the dependent name replaced by numbers, √n, π and
/// − in the reader's own words.
fn at_point(text: &str, var: &str, dep: &str, x: f64, y: f64) -> String {
    let mut out = String::new();
    let cs: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        let alone = |k: usize| (k == 0 || !cs[k - 1].is_ascii_alphabetic()) && (k + 1 >= cs.len() || !cs[k + 1].is_ascii_alphabetic());
        if c.to_string() == var && alone(i) { out.push_str(&format!("({x:e})")); }
        else if c.to_string() == dep && alone(i) { out.push_str(&format!("({y:e})")); }
        else if c == '\u{221a}' {
            out.push_str("sqrt(");
            i += 1;
            while i < cs.len() && cs[i].is_ascii_digit() { out.push(cs[i]); i += 1; }
            out.push(')');
            continue;
        }
        else if c == '\u{3c0}' { out.push_str("pi"); }
        else if c == '\u{2212}' { out.push('-'); }
        else if c == '\u{b7}' || c == '\u{d7}' { out.push('*'); }
        else { out.push(c); }
        i += 1;
    }
    out.replace("e+", "e").replace("e-", "e-")
}
/// Whether an equation "L = R" written in the panel holds at (x, y).
fn holds(eq: &str, var: &str, dep: &str, x: f64, y: f64) -> Option<bool> {
    let (l, r) = eq.split_once(" = ")?;
    let v = |t: &str| ex::parse(&at_point(t, var, dep, x, y)).map(|n| ex::eval(&n, 0.0));
    let (lv, rv) = (v(l)?, v(r)?);
    Some((lv - rv).abs() <= 1e-9 * (1.0 + lv.abs().max(rv.abs())))
}

/// An a or h typed as a sum, with or without spaces, in a, h, more tangents and more secants: every equation written with
/// "=" passes through its own point with its own slope (checked at the point and one unit along), the worked line's
/// denominator is h, and sums are written with their spaces. By hand for x²: the tangent at a has slope 2a, the secant
/// (a + h)² − a² over h = 2a + h, the normal slope −1/(2a).
#[test]
fn typed_sums_are_bracketed_and_spaced() {
    let sums = ["sqrt(2)-1", "sqrt(2) - 1", "-1-sqrt(2)", "- 1 - sqrt(2)", "pi/4+0.1", "pi/4 + 0.1", "1+sqrt(2)", "2-sqrt(3)", "1/2+sqrt(3)"];
    let read = |t: &str| ex::eval(&ex::parse(&t.replace(' ', "")).unwrap(), 0.0);
    let mut bad = vec![];
    let mut checked = 0;
    for (var, fname, func, dep) in [("x", "f", "x^2", "y"), ("t", "s", "t^2", "s")] {
        for sp in sums {
            for place in ["a", "h", "more_a", "more_h"] {
                let (a, h) = match place { "a" => (sp, "1"), "h" => ("1", sp), _ => ("1", "1") };
                let mut o: Vec<(&str, Value)> = vec![("func", s(func)), ("var", s(var)), ("fname", s(fname)), ("a", s(a)), ("h", s(h)), ("tangent_eq", s("Point-slope")), ("secant_eq", s("Point-slope")), ("normal", s("With its equation")), ("worked", b(true)), ("table", b(true)), ("key_line", b(true)), ("xmin", f(-4.0)), ("xmax", f(4.0)), ("ymin", f(-4.0)), ("ymax", f(12.0))];
                if place == "more_a" { o.push(("more_a", s(sp))); }
                if place == "more_h" { o.push(("more_h", s(sp))); }
                let p = with(&o);
                let svg = build(&p).svg;
                let mut lines = texts(&svg, "panel^");
                for k in texts(&svg, "key^") { for part in k.trim_start_matches("Key: ").trim_end_matches('.').split("; ") { lines.push(part.to_string()); } }
                let av = read(a);
                let hv = read(h);
                // which line: follow the headings
                let mut cur: Option<(f64, f64)> = None; // (point x, slope)
                for l in &lines {
                    let l = l.trim();
                    if l.starts_with("Tangent at P") { cur = Some((av, 2.0 * av)); continue; }
                    if l.starts_with("Normal at P") { cur = Some((av, -1.0 / (2.0 * av))); continue; }
                    if l.starts_with("Secant PQ") { cur = Some((av, 2.0 * av + hv)); continue; }
                    if let Some(rest) = l.strip_prefix(&format!("Tangent at {var} = ")) { let c = ex::eval(&ex::parse(&at_point(rest.split(':').next().unwrap(), "", "", 0.0, 0.0).replace(' ', "")).unwrap(), 0.0); cur = Some((c, 2.0 * c)); continue; }
                    if l.starts_with("Secant,") { cur = None; continue; }
                    let eq = l.strip_prefix("tangent ").map(|e| { cur = Some((av, 2.0 * av)); e }).unwrap_or(l);
                    if !(eq.starts_with(&format!("{dep} ")) && eq.contains(" = ")) { continue; }
                    let Some((x0, m)) = cur else { continue };
                    checked += 1;
                    let ok = holds(eq, var, dep, x0, x0 * x0) == Some(true) && holds(eq, var, dep, x0 + 1.0, x0 * x0 + m) == Some(true);
                    if !ok { bad.push(format!("{place}={sp} ({var}): {eq}")); }
                }
                // the worked line divides by h, read back as h
                if let Some(w) = lines.iter().find(|l| l.starts_with("= [")) {
                    let den = w.rsplit("]/").next().unwrap();
                    let dv = ex::parse(&at_point(den, var, dep, 0.0, 0.0)).map(|n| ex::eval(&n, 0.0));
                    if dv.is_none_or(|d| (d - hv).abs() > 1e-9) { bad.push(format!("{place}={sp}: worked denominator {den} is not {hv}")); }
                }
                // sums are written with their spaces: no √, π or digit run straight into a − or +
                for l in &lines {
                    let cs: Vec<char> = l.chars().collect();
                    for k in 1..cs.len().saturating_sub(1) {
                        if matches!(cs[k], '+' | '\u{2212}') && cs[k - 1] != ' ' && cs[k - 1] != '(' && cs[k - 1] != '^' && cs[k + 1] != ' ' { bad.push(format!("{place}={sp}: unspaced sum in {l}")); break; }
                    }
                }
            }
        }
    }
    println!("typed-sum equations checked: {checked}");
    assert!(checked >= 250, "{checked}");
    assert!(bad.is_empty(), "{} faults:\n{}", bad.len(), bad.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
}

/// A stacked power is one function everywhere: x^(1/3)^2 is x^((1/3)^2) = x^(1/9) for the grid (after its odd-root rewrite)
/// and the tree, the title is typeset with the second power inside the first, and so for the other spellings the rewrite
/// touches. By hand: x^(1/9) at 512 is 2; its slope (1/9)x^(−8/9) at 1 is 1/9.
#[test]
fn stacked_powers_and_rewritten_spellings_agree() {
    let exprs = ["x^(1/3)^2", "x^2^3", "2^x^2", "x^(1/3)^(1/3)", "(x^(1/3))^2", "x^(2/3)", "x^(-1/3)", "(x-1)^(1/3)", "sin(x)^(1/3)", "x^(1/3)x", "x^0.2", "x^(0.2)", "x^-0.6", "x^0.5", "x^(3/5)+x^(1/5)", "(x^2-4)^(1/3)", "2x^(1/3)", "x^(2/6)", "e^(x/3)", "x^(1/3)^2+1"];
    let xs = [-8.0, -2.5, -1.0, -0.3, 0.0, 0.25, 0.5, 1.0, 1.7, 2.0, 3.3, 512.0];
    let mut bad = vec![];
    for e in exprs {
        let grid_e = tangent::real_odd_roots(e);
        let tree = ex::parse(e).unwrap_or_else(|| panic!("{e}"));
        for x in xs {
            let g = super::eval(&grid_e, x).filter(|v| v.is_finite());
            let t = Some(ex::eval(&tree, x)).filter(|v| v.is_finite());
            let same = match (g, t) { (Some(a), Some(b)) => (a - b).abs() <= 1e-9 * (1.0 + a.abs()), (None, None) => true, _ => false };
            if !same { bad.push(format!("{e} at {x}: drawn {g:?}, read {t:?}")); }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    assert!((ex::eval(&ex::parse("x^(1/3)^2").unwrap(), 512.0) - 2.0).abs() < 1e-12);
    let p = with(&[("func", s("x^(1/3)^2")), ("a", s("1")), ("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-1.0)), ("ymax", f(3.0))]);
    let svg = build(&p).svg;
    assert!(has(&texts(&svg, "panel"), "f\u{2032}(1) = 1/9"), "{:?}", texts(&svg, "panel"));
    // the title: the 2 is a superscript inside the 1/3 superscript
    let title = svg.split(r#"data-tsl="panel">"#).nth(1).unwrap().split("</text>").next().unwrap();
    let first = title.find(r#"baseline-shift="super""#).unwrap();
    let rest = &title[first + 10..];
    let second = rest.find(r#"baseline-shift="super""#).expect("a nested superscript");
    assert!(!rest[..second].contains("</tspan>"), "{title}");
}

/// A decimal typed in the function (or a, h) makes the figure write decimals: exact where they terminate within 6 places,
/// else ≈ at the chosen places; never a fraction or a root beside a decimal in a line. By hand: h(t) = −4.9t² + 19.6t,
/// h′(1) = −9.8 + 19.6 = 9.8, h(1) = 14.7, h(2) = 19.6, secant (19.6 − 14.7)/1 = 4.9; 1000(1.05)^t at 5 to 6: 63.814… ≈ 63.81;
/// x^0.2 at 32: 0.2·32^(−0.8) = 0.2/16 = 0.0125.
#[test]
fn decimals_in_the_function_give_decimals() {
    let phys = with(&[("func", s("-4.9t^2+19.6t")), ("var", s("t")), ("fname", s("h")), ("a", s("1")), ("h", s("1")), ("worked", b(true)), ("tangent_eq", s("Slope-intercept")), ("key_line", b(true)), ("table", b(true)), ("xmin", f(0.0)), ("xmax", f(4.0)), ("ymin", f(0.0)), ("ymax", f(25.0)), ("ystep", f(5.0))]);
    let l = panel(&phys);
    assert!(has(&l, "h\u{2032}(1) = 9.8") && has(&l, &format!("= (19.6 {M} 14.7)/1 = 4.9")) && has(&l, "h = 9.8t + 4.9"), "{l:?}");
    let l2 = panel(&with(&[("func", s("-4.9t^2+19.6t")), ("var", s("t")), ("fname", s("h")), ("a", s("1")), ("h", s("0.5")), ("worked", b(true)), ("xmin", f(0.0)), ("xmax", f(4.0)), ("ymin", f(0.0)), ("ymax", f(25.0))]));
    let l3 = panel(&with(&[("func", s("1000(1.05)^t")), ("fname", s("A")), ("var", s("t")), ("a", s("5")), ("h", s("1")), ("worked", b(true)), ("table", b(true)), ("xmin", f(0.0)), ("xmax", f(10.0)), ("ymin", f(0.0)), ("ymax", f(999.0)), ("ystep", f(100.0))]));
    assert!(has(&l3, "\u{2248} 63.81"), "{l3:?}");
    let l4 = panel(&with(&[("func", s("x^0.2")), ("a", s("32")), ("show_secant", b(false)), ("xmin", f(0.0)), ("xmax", f(40.0)), ("ymin", f(-1.0)), ("ymax", f(3.0))]));
    assert!(has(&l4, "f\u{2032}(32) = 0.0125"), "{l4:?}");
    let l5 = panel(&with(&[("func", s("x^2")), ("a", s("1+sqrt(2)")), ("h", s("0.5")), ("table", b(true)), ("xmin", f(-1.0)), ("xmax", f(4.0)), ("ymin", f(-1.0)), ("ymax", f(12.0))]));
    for line in l.iter().chain(&l2).chain(&l3).chain(&l4).chain(&l5) {
        let cs: Vec<char> = line.chars().collect();
        let frac = (1..cs.len().saturating_sub(1)).any(|k| cs[k] == '/' && cs[k - 1].is_ascii_digit() && cs[k + 1].is_ascii_digit());
        let dec = (1..cs.len().saturating_sub(1)).any(|k| cs[k] == '.' && cs[k - 1].is_ascii_digit() && cs[k + 1].is_ascii_digit());
        let root = line.contains('\u{221a}') && !line.starts_with("Tangent") && !line.starts_with("slope f");
        assert!(!(dec && (frac || root)), "mixed: {line}");
    }
}

/// A function named h writes its step Δt everywhere (worked line, panel, table, h mark, the run's label, the key, the
/// student copy), never "h(a + h)".
#[test]
fn a_function_named_h_steps_by_delta() {
    for answers in [true, false] {
        let p = with(&[("func", s("-4.9t^2+19.6t")), ("var", s("t")), ("fname", s("h")), ("a", s("1")), ("h", s("1")), ("worked", b(true)), ("table", b(true)), ("key_line", b(true)), ("secant_triangle", b(true)), ("h_mark", b(true)), ("show_answers", b(answers)), ("xmin", f(0.0)), ("xmax", f(4.0)), ("ymin", f(0.0)), ("ymax", f(25.0)), ("ystep", f(5.0))]);
        let svg = build(&p).svg;
        let all: Vec<String> = texts(&svg, "panel").into_iter().chain(texts(&svg, "key")).chain(my_labels(&svg).into_iter().map(|x| x.1)).collect();
        let joined = all.join(" | ");
        assert!(!joined.contains("h(a + h)") && !joined.contains("h = 1") && !joined.contains("h \u{2192}"), "{joined}");
        assert!(joined.contains("\u{394}t = 1") && joined.contains("\u{394}t \u{2192} 0") && joined.contains(&format!("m = [h(a + \u{394}t) {M} h(a)]/\u{394}t")), "{joined}");
        assert!(my_labels(&svg).iter().any(|x| x.1 == "\u{394}t"), "the h mark");
    }
}

/// Beside a pole on the far side: f′ is written only to the digits that are right (true 1/sin²(10⁻⁶) = 10¹² + 1/3), no false
/// "jumps" note (there is no pole between π/2 + 10⁻⁶ and π/2 + 0.1), while a real pole between P and Q is still said (1/x²
/// from −0.5 to 0.5, tan from π/4 to 3π/4).
#[test]
fn far_side_of_a_pole() {
    let p = with(&[("func", s("tan(x)")), ("a", s("pi/2+0.000001")), ("h", s("0.1")), ("xmin", f(0.0)), ("xmax", f(3.0)), ("ymin", f(-10.0)), ("ymax", f(10.0))]);
    let o = build(&p);
    let l = texts(&o.svg, "panel");
    let line = l.iter().find(|x| x.contains('\u{2032}')).unwrap();
    let w = line.rsplit(' ').next().unwrap();
    let v: f64 = w.parse().unwrap();
    let places = w.split_once('.').map(|(_, f)| f.len() as i32).unwrap_or(0);
    let trailing = w.trim_end_matches('0').len() as i32 - w.len() as i32;
    let unit = if places > 0 { 10f64.powi(-places) } else { 10f64.powi(-trailing) };
    assert!((v - (1e12 + 1.0 / 3.0)).abs() <= 0.5 * unit.max(1.0) * 10.0, "{line}");
    assert!(!o.notes.join(" ").contains("jumps"), "{:?}", o.notes);
    let n = tangent::warnings(&with(&[("func", s("x^-2")), ("a", s("-0.5")), ("h", s("1")), ("xmin", f(-2.0)), ("xmax", f(2.0)), ("ymin", f(-1.0)), ("ymax", f(10.0))])).join(" ");
    assert!(n.contains("jumps"), "{n}");
}

/// A function defined at a but not on an interval round it ((−2)^x at 2) has no tangent, and the panel says why; x^0.2 is
/// read as x^(1/5), so at 0 its tangent is vertical (not "one side only"); x·|x| at 0 has slope 0.
#[test]
fn no_interval_round_a_is_said() {
    let n = tangent::warnings(&with(&[("func", s("(-2)^x")), ("a", s("2")), ("xmin", f(-1.0)), ("xmax", f(4.0))])).join(" ");
    assert!(n.contains("no slope and no tangent"), "{n}");
    let n = tangent::warnings(&with(&[("func", s("x^0.2")), ("a", s("0")), ("xmin", f(-2.0)), ("xmax", f(2.0)), ("ymin", f(-2.0)), ("ymax", f(2.0))])).join(" ");
    assert!(n.contains("vertical") && !n.contains("one side"), "{n}");
}

/// Scientific h: 1e-5 is written 0.00001 with a worked line in enough places to show the difference; 1e-12 is a step
/// (1×10^−12), not "h cannot be 0"; an h so small that a + h is a in double precision is refused as too small.
#[test]
fn scientific_steps() {
    let lines = panel(&with(&[("h", s("1e-5")), ("worked", b(true))]));
    assert!(has(&lines, "h = 0.00001"), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("1.00002") && l.contains("/0.00001")), "{lines:?}");
    let n = tangent::warnings(&with(&[("h", s("1e-12"))])).join(" | ");
    assert!(!n.contains("cannot be 0") && !n.contains("too small"), "{n}");
    assert!(has(&texts(&build(&with(&[("h", s("1e-12"))])).svg, "panel^"), "h = 1\u{d7}10^\u{2212}12"));
    // the worked line: Q's x in all its places, the step bracketed as a divisor
    let l = texts(&build(&with(&[("h", s("1e-12")), ("worked", b(true))])).svg, "panel^");
    assert!(has(&l, "= [f(1.000000000001) \u{2212} f(1)]/(1\u{d7}10^\u{2212}12)"), "{l:?}");
    let n = tangent::warnings(&with(&[("a", s("1")), ("h", s("1e-17"))])).join(" | ");
    assert!(n.contains("too small to tell Q from P"), "{n}");
    let n = tangent::warnings(&with(&[("a", s("1")), ("h", s("1")), ("more_h", s("1e-17"))])).join(" | ");
    assert!(n.contains("left out (Q would be P)"), "{n}");
}

/// Typed numbers in every spelling a class types: products, quotients, sums, scientific notation, spaces, unary minus,
/// brackets, exact and decimal mixes. Each is typed as a, as h, in More tangents and in More secants, on x², and the panel and
/// key are read back: the value written for a (in f′(…) and "Tangent at x = …") and for h ("Secant PQ, h = …", the worked
/// line's denominator) is the typed value; every "=" equation passes through its point with its slope (by hand for x²:
/// tangent 2a, secant 2a + h, normal −1/(2a)), checked at the point and one unit along; where nothing decimal was typed,
/// no exact line carries a decimal. 3/4*2 is 3/2 (not 3/42), 2*3 is 6, 1e-5 is 0.00001 (not an expression in e), 1 000 is 1000.
#[test]
fn typed_numbers_in_every_spelling() {
    use std::f64::consts::{E, PI};
    let r2 = 2f64.sqrt();
    let table: Vec<(&str, f64)> = vec![
        ("3/4*2", 1.5), ("2*3", 6.0), ("sqrt(3)*2", 2.0 * 3f64.sqrt()), ("pi*2", 2.0 * PI), ("-2*-1", 2.0), ("2 * 3", 6.0),
        ("3*1/2", 1.5), ("1/2*3", 1.5), ("(1/2)*3", 1.5), ("2*(1/2)", 1.0), ("-3/4*2", -1.5), ("2*-0.5", -1.0), ("0.5*3", 1.5),
        ("sqrt(2)*sqrt(2)", 2.0), ("2*pi/8", PI / 4.0), ("1e-5", 1e-5), ("2e-1", 0.2), ("1E-3", 1e-3), ("1e-12", 1e-12),
        ("1.5e0", 1.5), ("5e-1", 0.5), ("-2e-1", -0.2), ("2.5E-2", 0.025), ("1 000", 1000.0), ("1 / 2", 0.5), (" 1/2 ", 0.5),
        ("- 1/2", -0.5), ("--1", 1.0), ("-(1/2)", -0.5), ("(1)", 1.0), ("((2))", 2.0), ("1.", 1.0), ("0.1+0.2", 0.3),
        ("sqrt(2)/2+1/2", r2 / 2.0 + 0.5), ("sqrt(2)/2 + 0.5", r2 / 2.0 + 0.5), ("1/-2", -0.5), ("pi / 4", PI / 4.0),
        ("e/2", E / 2.0), ("2e", 2.0 * E), ("sqrt(4)", 2.0), ("3/6", 0.5), ("0.25*4", 1.0), ("-1.25", -1.25), ("2^-1", 0.5),
        ("2^(-1)", 0.5), ("sqrt(2) * 3/4", 0.75 * r2), ("1/2 - sqrt(2)/2", 0.5 - r2 / 2.0), ("-pi/6", -PI / 6.0),
    ];
    assert!(table.len() >= 40);
    let back = |t: &str| ex::parse(&at_point(t.trim(), "", "", 0.0, 0.0).replace(' ', "")).map(|n| ex::eval(&n, 0.0));
    let near = |a: f64, b: f64| (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()));
    let mut bad = vec![];
    let mut checked = 0;
    for (sp, val) in &table {
        let decimal_typed = sp.contains('.') || sp.to_lowercase().contains("e-") || sp.contains("e0");
        for place in ["a", "h", "more_a", "more_h"] {
            let (a, h) = match place { "a" => (*sp, "1"), "h" => ("1", *sp), _ => ("1", "1") };
            let (av, hv) = match place { "a" => (*val, 1.0), "h" => (1.0, *val), _ => (1.0, 1.0) };
            let mut o: Vec<(&str, Value)> = vec![("func", s("x^2")), ("a", s(a)), ("h", s(h)), ("tangent_eq", s("Point-slope")), ("secant_eq", s("Point-slope")), ("normal", s("With its equation")), ("worked", b(true)), ("table", b(true)), ("key_line", b(true))];
            if place == "more_a" { o.push(("more_a", s(sp))); }
            if place == "more_h" { o.push(("more_h", s(sp))); }
            let p = with(&o);
            let svg = build(&p).svg;
            let notes = tangent::warnings(&p).join(" | ");
            if notes.contains("Type a as") || notes.contains("Type h as") || notes.contains("cannot read") || notes.contains("cannot be 0") { bad.push(format!("{place}={sp}: refused: {notes}")); continue; }
            let mut lines = texts(&svg, "panel^");
            for k in texts(&svg, "key^") { for part in k.trim_start_matches("Key: ").trim_end_matches('.').split("; ") { lines.push(part.to_string()); } }
            let mut cur: Option<(f64, f64)> = None;
            let mut saw_h = false;
            for l in &lines {
                let l = l.trim();
                if let Some(x) = l.strip_prefix("slope f\u{2032}(").and_then(|r| r.split(')').next()) {
                    if !back(x).is_some_and(|v| near(v, av)) { bad.push(format!("{place}={sp}: a written {x}")); }
                }
                if l.starts_with("Tangent at P") { cur = Some((av, 2.0 * av)); continue; }
                if l.starts_with("Normal at P") { cur = Some((av, -1.0 / (2.0 * av))); continue; }
                if let Some(x) = l.strip_prefix("Secant PQ, h = ") {
                    if !back(x).is_some_and(|v| near(v, hv)) { bad.push(format!("{place}={sp}: h written {x}")); }
                    saw_h = true;
                    cur = Some((av, 2.0 * av + hv)); continue;
                }
                if let Some(rest) = l.strip_prefix("Tangent at x = ") {
                    let x = rest.split(':').next().unwrap();
                    match back(x) { Some(c) => { if place == "more_a" && !near(c, *val) { bad.push(format!("{place}={sp}: more a written {x}")); } cur = Some((c, 2.0 * c)); } None => { bad.push(format!("{place}={sp}: cannot read {x}")); cur = None; } }
                    continue;
                }
                if let Some(rest) = l.strip_prefix("Secant, h = ") {
                    let x = rest.split(':').next().unwrap();
                    match back(x) { Some(c) => { if place == "more_h" && !near(c, *val) { bad.push(format!("{place}={sp}: more h written {x}")); } cur = Some((av, 2.0 * av + c)); } None => { cur = None; } }
                    continue;
                }
                if l.starts_with("Secant,") { cur = None; continue; }
                if !decimal_typed && !l.contains('\u{2248}') {
                    let cs: Vec<char> = l.chars().collect();
                    if (1..cs.len().saturating_sub(1)).any(|k| cs[k] == '.' && cs[k - 1].is_ascii_digit() && cs[k + 1].is_ascii_digit()) { bad.push(format!("{place}={sp}: a decimal in an exact line: {l}")); }
                }
                let eq = l.strip_prefix("tangent ").map(|e| { cur = Some((av, 2.0 * av)); e }).unwrap_or(l);
                if !(eq.starts_with("y ") && eq.contains(" = ")) { continue; }
                let Some((x0, m)) = cur else { continue };
                checked += 1;
                let ok = holds(eq, "x", "y", x0, x0 * x0) == Some(true) && holds(eq, "x", "y", x0 + 1.0, x0 * x0 + m) == Some(true);
                if !ok { bad.push(format!("{place}={sp}: {eq} (point {x0}, slope {m})")); }
            }
            if place == "h" && !saw_h { bad.push(format!("h={sp}: no secant written ({notes})")); }
            if let Some(w) = lines.iter().find(|l| l.contains("= [") || l.trim_start().starts_with("\u{2248} (") || l.trim_start().starts_with("= (")) {
                if let Some(den) = w.rsplit(")/").next().filter(|_| w.contains(")/")) {
                    let den = den.split(' ').next().unwrap();
                    if !back(den).is_some_and(|d| (d - hv).abs() <= 1e-9 * hv.abs().max(1e-300)) { bad.push(format!("{place}={sp}: worked denominator {den} is not {hv}")); }
                }
            }
        }
    }
    println!("typed-number spellings: {}, equations checked: {checked}", table.len());
    assert!(checked >= 600, "{checked}");
    assert!(bad.is_empty(), "{} faults:\n{}", bad.len(), bad.iter().take(50).cloned().collect::<Vec<_>>().join("\n"));
}

/// Functions finite only through 1/0 = ∞ in the arithmetic (arctan(1/x), e^(−1/x) at 0), and removable gaps (x²/x at 0, 1/sec x
/// at π/2) have no value at a: no tangent is drawn, the note says the formula has no value there and whether the curve has a
/// hole or a jump, More tangents says so too, and a secant across arctan(1/x)'s jump is said to join separate pieces.
#[test]
fn removable_gaps_and_infinite_steps_are_undefined() {
    let cases = [
        ("arctan(1/x)", "0", "jumps"), ("1/(1+e^(1/x))", "0", "jumps"), ("e^(-1/x)", "0", ""), ("1/(1/x)", "0", "hole"),
        ("1/(1+1/x)", "0", "hole"), ("(1/x)^(-1)", "0", "hole"), ("e^(-1/x^2)", "0", "hole"), ("x^2/x", "0", "hole"),
        ("x^3/x", "0", "hole"), ("sin(x)/sin(x)", "0", "hole"), ("1/sec(x)", "pi/2", "hole"), ("cos(x)/cos(x)", "pi/2", "hole"),
        ("(x^2-1)/(x-1)", "1", "hole"), ("x/abs(x)", "0", "jumps"), ("tan(x)/tan(x)", "0", "hole"),
    ];
    assert!(cases.len() >= 12);
    let mut bad = vec![];
    for (fx, a, kind) in cases {
        let p = with(&[("func", s(fx)), ("a", s(a)), ("h", s("0.5")), ("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-3.0)), ("ymax", f(3.0))]);
        let m = tangent::model(&p);
        let n = tangent::warnings(&p).join(" | ");
        if m.p.is_some() { bad.push(format!("{fx} at {a}: a tangent ({n})")); }
        if !n.contains("is undefined") { bad.push(format!("{fx} at {a}: notes {n}")); }
        if !kind.is_empty() && !n.contains(kind) { bad.push(format!("{fx} at {a}: not said to be a {kind}: {n}")); }
        let p = with(&[("func", s(fx)), ("a", s("1.3")), ("h", s("0.5")), ("more_a", s(a)), ("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-3.0)), ("ymax", f(3.0))]);
        let m = tangent::model(&p);
        let n = tangent::warnings(&p).join(" | ");
        if !m.more.is_empty() || !n.contains("More tangents:") || !n.contains("is undefined") { bad.push(format!("{fx}: more tangents at {a}: {n}")); }
    }
    for fx in ["arctan(1/x)", "1/(1+e^(1/x))", "x/abs(x)"] {
        let n = tangent::warnings(&with(&[("func", s(fx)), ("a", s("-1")), ("h", s("2.3")), ("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-3.0)), ("ymax", f(3.0))])).join(" | ");
        if !n.contains("jumps between P and Q") { bad.push(format!("{fx}: secant across the jump: {n}")); }
    }
    for fx in ["x^2", "sin(x)", "arctan(x)", "e^x"] {
        let n = tangent::warnings(&with(&[("func", s(fx)), ("a", s("-1")), ("h", s("2.3")), ("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-3.0)), ("ymax", f(10.0))])).join(" | ");
        if n.contains("jumps") { bad.push(format!("{fx}: a jump that is not there: {n}")); }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Powers typed with spaces round ^ are one function for both readers: x ^ (1/3) is drawn for negative x as it is read.
#[test]
fn spaced_powers_read_alike() {
    let exprs = ["x ^ (1/3)", "x^ (1/3)", "x ^(1/3)", "x ^ (2/3)", "(x - 1) ^ (1/3)", "x ^ (1/3) ^ 2", "x ^ ((1/3)^2)", "2 * x ^ (1/5)", "x ^ (1/3) + x ^ (1/5)", "x^((1/3)^3)"];
    let mut bad = vec![];
    for e in exprs {
        let plain = tangent::expr_plain(e, "f", 'x').unwrap_or_else(|m| panic!("{e}: {m}"));
        let grid_e = tangent::real_odd_roots(&plain);
        let tree = ex::parse(&plain).unwrap_or_else(|| panic!("{e}"));
        for x in [-8.0, -2.0, -0.5, 0.5, 2.0, 8.0] {
            let g = super::eval(&grid_e, x).filter(|v| v.is_finite());
            let t = Some(ex::eval(&tree, x)).filter(|v| v.is_finite());
            if x < 0.0 && g.is_none() { bad.push(format!("{e} at {x}: not drawn")); }
            let same = match (g, t) { (Some(a), Some(b)) => (a - b).abs() <= 1e-9 * (1.0 + a.abs()), (None, None) => true, _ => false };
            if !same { bad.push(format!("{e} at {x}: drawn {g:?}, read {t:?}")); }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// One rule for names: a function cannot be named after something the reader knows (sin, ln, pi, e …) or a point (P, Q),
/// and is refused with a note; when the function or variable is named h, a or m, the figure's own h, a and m give way to Δx,
/// x₀ and m_PQ, in the worked line, the slope line and the student blank.
#[test]
fn names_do_not_clash() {
    for name in ["sin", "cos", "ln", "log", "exp", "sqrt", "abs", "pi", "e", "P", "Q"] {
        let p = with(&[("fname", s(name))]);
        let n = tangent::warnings(&p).join(" | ");
        assert!(n.contains(&format!("cannot be {name}")), "{name}: {n}");
        assert!(panel(&p)[0].starts_with("f(x) = "), "{name}");
    }
    let w = |o: &[(&str, Value)]| { let mut v = vec![("worked", b(true)), ("secant_eq", s("None"))]; v.extend_from_slice(o); panel(&with(&v)) };
    let l = w(&[("fname", s("a"))]);
    assert!(has(&l, "[a(x0 + h) \u{2212} a(x0)]/h"), "{l:?}");
    let l = w(&[("var", s("a")), ("func", s("a^2"))]);
    assert!(has(&l, "[f(a0 + h) \u{2212} f(a0)]/h"), "{l:?}");
    let l = w(&[("fname", s("m"))]);
    assert!(has(&l, "mPQ = [m(a + h)"), "{l:?}");
    let l = w(&[("var", s("m")), ("func", s("m^2"))]);
    assert!(has(&l, "mPQ = [f(a + h)"), "{l:?}");
    let l = w(&[("var", s("h")), ("func", s("h^2"))]);
    assert!(has(&l, "[f(a + \u{394}h) \u{2212} f(a)]/\u{394}h"), "{l:?}");
    let l = w(&[("fname", s("h")), ("var", s("t")), ("func", s("t^2"))]);
    assert!(has(&l, "\u{394}t"), "{l:?}");
}

/// Small values keep two significant figures, so the slope of x^((1/3)³) at 8, (1/27)·8^(−26/27) ≈ 0.00502, is 0.005 and not
/// 0.01; arccot has its exact values (arccot 0 = π/2, arccot 1 = π/4, arccot √3 = π/6).
#[test]
fn small_values_and_arccot() {
    assert_eq!(tangent::decimal(0.00502, 2), "0.005");
    assert_eq!(tangent::decimal(0.0123, 2), "0.012");
    assert_eq!(tangent::decimal(0.5, 2), "0.5");
    assert_eq!(tangent::decimal(-0.0471, 2), "\u{2212}0.047");
    let l = panel(&with(&[("func", s("x^((1/3)^3)")), ("a", s("8")), ("xmin", f(0.0)), ("xmax", f(10.0)), ("ymin", f(0.0)), ("ymax", f(2.0))]));
    assert!(has(&l, "f\u{2032}(8) \u{2248} 0.005"), "{l:?}");
    assert_eq!(fx_at("arccot(x)", "0").as_deref(), Some("\u{3c0}/2"));
    assert_eq!(fx_at("arccot(x)", "1").as_deref(), Some("\u{3c0}/4"));
    assert_eq!(fx_at("arccot(x)", "sqrt(3)").as_deref(), Some("\u{3c0}/6"));
}

/// No rounded argument under "=": with a or h in ln(2), ln(3), 2^(1/3), 3^(1/3) or sqrt 2 typed without brackets, a + h has
/// no exact form here, so the worked line writes it as typed (f(ln(2) + 0.01)). Every "=" step of the worked line is
/// evaluated: its arguments must be a + h and a to 12 digits, its denominator h, and an "= (A − B)/h = m" step must equal the
/// slope; the "≈" result must agree with the true slope (f(a + h) − f(a))/h to the places shown. The same step is checked in
/// the student copy, which asks for its value. By hand: e^x at ln 2 with h = 0.01 has slope 2(e^0.01 − 1)/0.01 = 2.0100…
#[test]
fn no_rounded_argument_under_equals() {
    let typed = ["ln(2)", "ln(3)", "2^(1/3)", "3^(1/3)", "sqrt 2"];
    let mut combos: Vec<(&str, &str, &str)> = vec![];
    for a in typed { for h in ["0.01", "0.001", "1/2"] { combos.push(("e^x", a, h)); } }
    for h in ["ln(2)", "2^(1/3)", "sqrt 2"] { combos.push(("x^2", "1", h)); combos.push(("e^x", "1", h)); }
    assert!(combos.len() >= 21);
    let val = |t: &str| ex::parse(&at_point(t.trim(), "", "", 0.0, 0.0).replace(' ', "").replace("sqrt2", "sqrt(2)")).map(|n| ex::eval(&n, 0.0));
    let mut bad = vec![];
    let mut steps = 0;
    for (func, a, h) in &combos {
        let fe = ex::parse(&func.replace("e^x", "e^(x)")).unwrap();
        let fv = |x: f64| ex::eval(&fe, x);
        let (av, hv) = (val(a).unwrap(), val(h).unwrap());
        let true_m = (fv(av + hv) - fv(av)) / hv;
        for answers in [true, false] {
            let p = with(&[("func", s(func)), ("a", s(a)), ("h", s(h)), ("worked", b(true)), ("table", b(true)), ("more_h", s("0.1, 0.001")), ("show_answers", b(answers)), ("xmin", f(-2.0)), ("xmax", f(4.0)), ("ymin", f(-1.0)), ("ymax", f(30.0))]);
            let lines = texts_sup(&build(&p).svg);
            let Some(w) = lines.iter().find(|l| l.starts_with("= [")) else { bad.push(format!("{func} a={a} h={h}: no worked step ({lines:?})")); continue };
            steps += 1;
            let inner = w.trim_start_matches("= [");
            let (args, den) = inner.rsplit_once("]/").unwrap();
            let (q, pp) = args.split_once(" \u{2212} f(").unwrap();
            let q = q.strip_prefix("f(").and_then(|q| q.strip_suffix(')')).unwrap_or(q);
            let pp = pp.strip_suffix(')').unwrap_or(pp);
            let close = |t: &str, want: f64| val(t).is_some_and(|v| (v - want).abs() <= 1e-12 * (1.0 + want.abs()));
            if !close(q, av + hv) { bad.push(format!("{func} a={a} h={h} answers={answers}: f({q}) is not a + h = {}", av + hv)); }
            if !close(pp, av) { bad.push(format!("{func} a={a} h={h}: f({pp}) is not a")); }
            if !close(den, hv) { bad.push(format!("{func} a={a} h={h}: denominator {den}")); }
            if answers {
                let k = lines.iter().position(|l| l == w).unwrap();
                let next = &lines[k + 1];
                let last = next.rsplit(['=', '\u{2248}']).next().unwrap().trim();
                let shown = val(last).unwrap_or(f64::NAN);
                let places = last.split('.').nth(1).map(|d| d.len()).unwrap_or(0) as i32;
                if (shown - true_m).abs() > 0.5 * 10f64.powi(-places) * 1.0001 + 1e-12 { bad.push(format!("{func} a={a} h={h}: result {next} is not {true_m}")); }
                if next.starts_with("= (") && !next.contains('\u{2248}') {
                    let (frac, _) = next.trim_start_matches("= ").rsplit_once(" = ").unwrap();
                    if !close(frac, true_m) { bad.push(format!("{func} a={a} h={h}: {next} is not {true_m}")); }
                }
            } else if !lines.iter().any(|l| l.trim() == "=") { bad.push(format!("{func} a={a} h={h}: the student copy asks no value")); }
        }
    }
    println!("worked steps checked: {steps}");
    assert!(bad.is_empty(), "{} faults:\n{}", bad.len(), bad.join("\n"));
}

/// The panel's text with each superscript bracketed: 2<sup>1/3</sup> reads 2^(1/3).
fn texts_sup(svg: &str) -> Vec<String> {
    let pat = r#"data-tsl="panel">"#;
    let open = r#"<tspan baseline-shift="super" font-size="70%">"#;
    svg.match_indices(pat).map(|(i, _)| {
        let rest = &svg[i + pat.len()..];
        let mut body = &rest[..rest.find("</text>").unwrap_or(0)];
        let mut out = String::new();
        let mut depth: Vec<bool> = vec![];
        while !body.is_empty() {
            if body.starts_with(open) { out.push_str("^("); depth.push(true); body = &body[open.len()..]; continue; }
            if body.starts_with("<tspan") { depth.push(false); body = &body[body.find('>').unwrap() + 1..]; continue; }
            if body.starts_with("</tspan>") { if depth.pop() == Some(true) { out.push(')'); } body = &body[8..]; continue; }
            if body.starts_with('<') { body = &body[body.find('>').unwrap() + 1..]; continue; }
            let c = body.chars().next().unwrap();
            out.push(c);
            body = &body[c.len_utf8()..];
        }
        out.replace("&amp;", "&")
    }).collect()
}

/// A worked line with a typed decimal is in decimals throughout: x^0.5 at 1 with h = 1/2 is [f(1.5) − f(1)]/0.5, x² at −1/2
/// with h = 0.01 is (0.2401 − 0.25)/0.01; and with h = 0.0001 its result has the places the table gives (2.0001, not 2).
#[test]
fn worked_lines_keep_one_kind_of_number() {
    let w = |o: &[(&str, Value)]| { let mut v = vec![("worked", b(true))]; v.extend_from_slice(o); texts_sup(&build(&with(&v)).svg) };
    let l = w(&[("func", s("x^0.5")), ("a", s("1")), ("h", s("1/2"))]);
    assert!(has(&l, "[f(1.5) \u{2212} f(1)]/0.5"), "{l:?}");
    let l = w(&[("a", s("-1/2")), ("h", s("0.01"))]);
    assert!(has(&l, "(0.2401 \u{2212} 0.25)/0.01"), "{l:?}");
    let l = w(&[("a", s("1")), ("h", s("0.0001"))]);
    assert!(l.iter().any(|x| x.ends_with("2.0001")), "{l:?}");
}

/// Compound interest with a principal of 1000 is drawn: 1000(1.05)^t at t = 5 (A = 1276.28) on a y-axis to 2000 is numbered
/// in thousands and the curve, P and Q are on it; a y-step of 1 over 2000 is widened to a step giving 5–10 lines, with a
/// note; a window to a million is drawn in millions; only beyond ±1000000 is P refused.
#[test]
fn large_values_are_drawn() {
    let o = |ymax: f64, ystep: f64| with(&[("func", s("1000(1.05)^t")), ("fname", s("A")), ("var", s("t")), ("a", s("5")), ("h", s("1")), ("xmin", f(0.0)), ("xmax", f(10.0)), ("ymin", f(0.0)), ("ymax", f(ymax)), ("ystep", f(ystep))]);
    let p = o(2000.0, 1.0);
    let out = build(&p);
    let n = out.notes.join(" | ");
    assert!(!n.contains("past the largest") && !n.contains("outside the window"), "{n}");
    assert!(n.contains("The y-axis counts by 200"), "{n}");
    assert!(out.svg.contains("(thousands)"), "no thousands label");
    let labs = my_labels(&out.svg);
    assert!(labs.iter().any(|l| l.1 == "P") && labs.iter().any(|l| l.1 == "Q"), "{labs:?}");
    // P's dot sits where the axis says 1276.28: between the gridlines for 1000 and 1500 (in thousands, 1 and 1.5)
    let ax = tangent::axes(&p);
    assert_eq!((ax.k, ax.ystep), (1000.0, 200.0));
    let p2 = o(1_000_000.0, 100_000.0);
    assert!(build(&p2).svg.contains("(millions)"));
    let n = tangent::warnings(&with(&[("func", s("1000(1.05)^t")), ("var", s("t")), ("a", s("200")), ("xmin", f(0.0)), ("xmax", f(300.0)), ("ymin", f(0.0)), ("ymax", f(1_000_000.0))])).join(" | ");
    assert!(n.contains("past the largest y-axis"), "{n}");
    let n = tangent::warnings(&with(&[("ymin", f(0.0)), ("ymax", f(999.0)), ("ystep", f(1.0))])).join(" | ");
    assert!(n.contains("counts by 100"), "{n}");
}

