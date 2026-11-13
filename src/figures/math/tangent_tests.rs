//! Tests of the tangent and secant lines figure (`tangent-secant-lines`). Every number checked here was worked by hand
//! (the working is in the comments), never read back from the figure's own code.
use super::tangent::{self, build, exact_form, slope_at, Slope, ID};
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
    let pat = format!(r#"data-tsl="{tag}">"#);
    svg.match_indices(&pat).map(|(i, _)| {
        let rest = &svg[i + pat.len()..];
        let body = &rest[..rest.find("</text>").unwrap_or(0)];
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

#[test]
fn exact_forms_are_the_textbook_ones() {
    let t = 1e-12;
    assert_eq!(exact_form(3.0, t, false).as_deref(), Some("3"));
    assert_eq!(exact_form(-0.5, t, false).as_deref(), Some("\u{2212}1/2"));
    assert_eq!(exact_form(-0.5, t, true).as_deref(), Some("\u{2212}0.5"));
    assert_eq!(exact_form(2.1, 1e-12, true).as_deref(), Some("2.1"));
    assert_eq!(exact_form(1.0 / 3.0, t, true).as_deref(), Some("1/3"));
    // √3/2 = sin(π/3); 1/(2√2) = √2/4; 3√2
    assert_eq!(exact_form(3f64.sqrt() / 2.0, t, false).as_deref(), Some("\u{221a}3/2"));
    assert_eq!(exact_form(1.0 / (2.0 * 2f64.sqrt()), t, false).as_deref(), Some("\u{221a}2/4"));
    assert_eq!(exact_form(-3.0 * 2f64.sqrt(), t, false).as_deref(), Some("\u{2212}3\u{221a}2"));
    assert_eq!(exact_form(std::f64::consts::PI / 4.0, t, false).as_deref(), Some("\u{3c0}/4"));
    assert_eq!(exact_form(std::f64::consts::E, t, false).as_deref(), Some("e"));
    assert_eq!(exact_form(std::f64::consts::E.powi(2) / 2.0, t, false).as_deref(), Some("e^2/2"));
    assert_eq!(exact_form(1.0 / std::f64::consts::E, t, false).as_deref(), Some("1/e"));
    // ln 2 and e^0.3 have none of these forms
    assert_eq!(exact_form(2f64.ln(), t, false), None);
    assert_eq!(exact_form(0.3f64.exp(), t, false), None);
}

#[test]
fn slopes_of_the_curve_by_hand() {
    let fx = |e: &'static str| move |x: f64| super::eval(e, x).filter(|v| v.is_finite());
    let d = |e: &'static str, a: f64| match slope_at(&fx(e), a) { Slope::Finite(v, _) => v, o => panic!("{e} at {a}: {o:?}") };
    // d/dx x^2 = 2x; x^3 = 3x^2; sin = cos; e^x = e^x; ln = 1/x; sqrt = 1/(2 sqrt x); 1/x = -1/x^2
    assert!((d("x^2", 1.0) - 2.0).abs() < 1e-10);
    assert!((d("x^3", 2.0) - 12.0).abs() < 1e-9);
    assert!((d("sin(x)", std::f64::consts::PI / 3.0) - 0.5).abs() < 1e-10);
    assert!((d("e^x", 0.0) - 1.0).abs() < 1e-10);
    assert!((d("ln(x)", 2.0) - 0.5).abs() < 1e-10);
    assert!((d("sqrt(x)", 4.0) - 0.25).abs() < 1e-10);
    assert!((d("1/x", 2.0) + 0.25).abs() < 1e-10);
    assert!((d("x^3 - 3x", 1.0)).abs() < 1e-10);
    assert!(matches!(slope_at(&fx("abs(x)"), 0.0), Slope::Corner(l, r) if (l + 1.0).abs() < 1e-6 && (r - 1.0).abs() < 1e-6));
    assert!(matches!(slope_at(&fx("sqrt(abs(x))"), 0.0), Slope::Corner(..)));
    assert_eq!(slope_at(&fx("sqrt(x)"), 0.0), Slope::OneSided);
    assert_eq!(slope_at(&fx("ln(x)"), 0.0), Slope::Undefined);
    assert_eq!(slope_at(&|x: f64| Some(x.cbrt()), 0.0), Slope::Vertical);
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
    assert!(has(&l, "f\u{2032}(\u{3c0}/3) = 1/2") && has(&l, &format!("= [f(5\u{3c0}/6) {M} f(\u{3c0}/3)]/(\u{3c0}/2)")) && has(&l, &format!("= (1/2 {M} \u{221a}3/2)/(\u{3c0}/2) \u{2248} {M}0.23")), "{l:?}");
    // its intercept √3/2 - π/6 = 0.3424 has no exact form: the whole equation is in decimals, y ≈ 0.5x + 0.34
    assert!(has(&l, "y \u{2248} 0.5x + 0.34"), "{l:?}");
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
    assert!(labs.iter().any(|l| l == "1") && labs.iter().any(|l| l == "2"));
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
fn attr<'a>(h: &'a str, n: &str) -> Option<&'a str> { h.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()) }
fn numa(h: &str, n: &str) -> f64 { attr(h, n).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }

/// Every stroke a label must keep off (curves, the axes, the lines and the slope triangles), clipped to what is seen.
fn strokes(svg: &str) -> Vec<((f64, f64), (f64, f64))> {
    let clip = svg.split(r#"<clipPath id="plot"><rect "#).nth(1).map(|r| { let h = r.split('>').next().unwrap(); (numa(h, "x"), numa(h, "y"), numa(h, "x") + numa(h, "width"), numa(h, "y") + numa(h, "height")) }).unwrap();
    let mut v = vec![];
    for (i, _) in svg.match_indices("<line ") {
        let h = &svg[i..i + svg[i..].find('>').unwrap()];
        if matches!(attr(h, "data-tsl"), Some("swatch" | "blank")) { continue; }
        if numa(h, "stroke-width") < 1.9 && attr(h, "data-tsl") != Some("hmark") { continue; }
        v.push(((numa(h, "x1"), numa(h, "y1")), (numa(h, "x2"), numa(h, "y2"))));
    }
    for (i, _) in svg.match_indices("<polyline ") {
        let h = &svg[i..i + svg[i..].find('>').unwrap()];
        if numa(h, "stroke-width") < 1.9 { continue; }
        let pts: Vec<(f64, f64)> = attr(h, "points").unwrap_or("").split(' ').filter_map(|q| { let (a, b) = q.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
        for w in pts.windows(2) { if let Some(s) = clip_seg(w[0], w[1], clip) { v.push(s); } }
    }
    v
}
fn clip_seg(a: (f64, f64), b: (f64, f64), r: Bx) -> Option<((f64, f64), (f64, f64))> {
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
fn on(b: Bx, s: ((f64, f64), (f64, f64))) -> bool { clip_seg(s.0, s.1, (b.0 + 0.5, b.1 + 0.5, b.2 - 0.5, b.3 - 0.5)).is_some() }
fn hit(a: Bx, b: Bx) -> bool { a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3 }

/// The faults of one render: (kind of label, what is wrong).
fn faults(svg: &str) -> Vec<(String, String)> {
    let (w, h) = crate::figures::svg_size(svg);
    let all = crate::figures::math::flat_b9::text_boxes(svg);
    let mut mine: Vec<(String, Bx)> = vec![];
    for (i, _) in svg.match_indices("<g data-lbl=\"") {
        let end = svg[i..].find("</g>").map(|e| i + e + 4).unwrap();
        mine.extend(crate::figures::math::flat_b9::text_boxes(&svg[i..end]));
    }
    let lines = strokes(svg);
    let dots: Vec<(f64, f64, f64)> = svg.match_indices("<circle ").map(|(i, _)| { let hd = &svg[i..i + svg[i..].find('>').unwrap()]; (numa(hd, "cx"), numa(hd, "cy"), numa(hd, "r")) }).filter(|d| d.2 >= 3.0).collect();
    let mut out = vec![];
    for (t, bx) in &mine {
        if bx.0 < 0.0 || bx.1 < 0.0 || bx.2 > w || bx.3 > h { out.push((t.clone(), "off the page".into())); }
        if lines.iter().any(|s| on(*bx, *s)) { out.push((t.clone(), "on a line".into())); }
        if dots.iter().any(|d| d.0 + d.2 > bx.0 && d.0 - d.2 < bx.2 && d.1 + d.2 > bx.1 && d.1 - d.2 < bx.3) { out.push((t.clone(), "on a dot".into())); }
        let others = all.iter().filter(|(_, o)| (o.0 - bx.0).abs() > 0.01 || (o.1 - bx.1).abs() > 0.01).count();
        let _ = others;
        if all.iter().any(|(_, o)| (o.0 - bx.0).abs() + (o.1 - bx.1).abs() > 0.02 && hit(*bx, *o)) { out.push((t.clone(), "on other text".into())); }
    }
    // a leader runs from its own point to its own label through no line and no other text
    for (i, _) in svg.match_indices(r#"data-tsl="leader""#) {
        let st = svg[..i].rfind("<line ").unwrap();
        let hd = &svg[st..i];
        let (a, e) = ((numa(hd, "x1"), numa(hd, "y1")), (numa(hd, "x2"), numa(hd, "y2")));
        let n = ((e.0 - a.0).powi(2) + (e.1 - a.1).powi(2)).sqrt().max(1e-9);
        let a6 = (a.0 + (e.0 - a.0) / n * 6.0, a.1 + (e.1 - a.1) / n * 6.0);
        if lines.iter().any(|s| tangent::seg_cross(a6, e, s.0, s.1)) { out.push(("leader".into(), "crosses a line".into())); }
        if !mine.iter().any(|(_, b)| e.0 >= b.0 - 0.6 && e.0 <= b.2 + 0.6 && e.1 >= b.1 - 0.6 && e.1 <= b.3 + 0.6) { out.push(("leader".into(), "not at its label".into())); }
        if all.iter().filter(|(_, b)| !(e.0 >= b.0 - 0.6 && e.0 <= b.2 + 0.6 && e.1 >= b.1 - 0.6 && e.1 <= b.3 + 0.6)).any(|(_, b)| clip_seg(a6, (e.0 - (e.0 - a.0) / n * 1.0, e.1 - (e.1 - a.1) / n * 1.0), (b.0 + 0.5, b.1 + 0.5, b.2 - 0.5, b.3 - 0.5)).is_some()) { out.push(("leader".into(), "through text".into())); }
    }
    // the panel and the key: never on each other, inside the page
    for (t, bx) in all.iter().filter(|(t, _)| !mine.iter().any(|(m, _)| m == t)) {
        if bx.2 > w + 0.5 || bx.3 > h + 0.5 { out.push((t.clone(), "panel text off the page".into())); }
    }
    out
}

/// Classroom examples of tangents and secants (the functions, points and windows a textbook uses), with a
/// y step giving at most about 12 numbered lines.
fn classroom() -> Vec<(&'static str, &'static str, &'static str, &'static str, &'static str, [f64; 4], bool)> {
    // (function, name, variable, a, h, [xmin, xmax, ymin, ymax], π units)
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
    ]
}

fn ystep(lo: f64, hi: f64) -> f64 { let r = (hi - lo) / 10.0; [0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0].into_iter().find(|s| *s >= r).unwrap_or(20.0) }

fn case(c: &(&str, &str, &str, &str, &str, [f64; 4], bool), size: &str, opts: &[(&str, Value)]) -> Params {
    let (fx, fname, var, a, h, w, pi) = c;
    let mut o: Vec<(&str, Value)> = vec![("func", s(fx)), ("fname", s(fname)), ("var", s(var)), ("a", s(a)), ("h", s(h)), ("xmin", f(w[0])), ("xmax", f(w[1])), ("ymin", f(w[2])), ("ymax", f(w[3])), ("ystep", f(ystep(w[2], w[3]))), ("label_size", s(size))];
    if *pi { o.extend([("xunits", s("Multiples of \u{3c0}")), ("xstep", f(0.25))]); }
    o.extend(opts.iter().cloned());
    with(&o)
}

fn option_sets() -> Vec<(&'static str, Vec<(&'static str, Value)>)> {
    let all: Vec<(&'static str, Value)> = vec![("secant_triangle", b(true)), ("tangent_triangle", b(true)), ("h_mark", b(true)), ("pt_labels", s("Letters and coordinates")), ("normal", s("With its equation")), ("more_h", s("0.5, 0.1")), ("table", b(true)), ("worked", b(true)), ("tangent_eq", s("Slope-intercept")), ("secant_eq", s("Point-slope")), ("key_line", b(true))];
    let mut v: Vec<(&'static str, Vec<(&'static str, Value)>)> = vec![("plain", vec![])];
    for (k, val) in &all { v.push((k, vec![(*k, val.clone())])); }
    v.push(("more_a", vec![("more_a", s("0.25"))]));
    v.push(("everything", all));
    v.push(("student", vec![("show_answers", b(false)), ("secant_triangle", b(true)), ("tangent_triangle", b(true)), ("table", b(true)), ("worked", b(true))]));
    v.push(("print", vec![("style", s("Print")), ("secant_triangle", b(true)), ("h_mark", b(true))]));
    v
}

/// The sweep: every classroom example, at every size, with each optional part alone and all together. Read from the
/// drawn SVG: no label off the page, on a line, on a dot or on other text, and none drawn without a clear place.
#[test]
fn classroom_sweep_labels_are_clear() {
    let list = classroom();
    assert!(list.len() >= 60);
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
                for (t, why) in fs {
                    *by.entry((size.to_string(), why.clone())).or_default() += 1;
                    if !(size == "Large" && LARGE_EXCEPTIONS.contains(&(c.0, c.3, name))) { fails.push(format!("{size} {} a={} h={} [{name}]: {t} {why}", c.0, c.3, c.4)); }
                }
            }
        }
    }
    println!("tangent sweep: {total} figures; faults by size and kind: {by:?}");
    assert!(fails.is_empty(), "{} faults:\n{}", fails.len(), fails.iter().take(60).cloned().collect::<Vec<_>>().join("\n"));
}

/// Classroom cases at Large where a label may be left on a line (named; none at Small or Medium).
const LARGE_EXCEPTIONS: &[(&str, &str, &str)] = &[
    // every option at once at Large: the tangent's rise (≈ 0.69) has no room between the curve, the normal and the secants
    ("2^x", "0", "everything"),
    // every option at once at Large: the h mark's label between P and Q only 0.25 apart (Q's coordinates, the triangles)
    ("x^3", "1/2", "everything"),
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

/// A control shows in the panel exactly when changing it changes the drawing, in each kind of state.
#[test]
fn controls_show_exactly_when_they_act() {
    let d = def();
    let states: Vec<Vec<(&str, Value)>> = vec![
        vec![],
        vec![("show_answers", b(false))],
        vec![("show_answers", b(false)), ("student_copy", s("Curve and P only, to draw the tangent"))],
        vec![("show_secant", b(false))],
        vec![("show_tangent", b(false)), ("normal", s("With its slope"))],
        vec![("slope_form", s("Decimals")), ("func", s("ln(x)")), ("xmin", f(-1.0))],
        vec![("title_mode", s("None")), ("show_slopes", b(false))],
    ];
    for st in states {
        let p = with(&st);
        let base = (d.render)(&p);
        for pd in (d.params)() {
            if pd.key == "label_offsets" { continue; }
            let alts: Vec<Value> = match &pd.kind {
                crate::figures::Kind::Bool => vec![b(!p.b(pd.key))],
                crate::figures::Kind::Choice(o) => o.iter().filter(|o| **o != p.s(pd.key)).map(|o| s(o)).collect(),
                crate::figures::Kind::Int { .. } => vec![Value::I(p.i(pd.key) + 2)],
                crate::figures::Kind::Float { .. } => vec![f(p.f(pd.key) + if pd.key.contains("min") { -1.0 } else { 1.0 })],
                _ => match pd.key { "func" => vec![s("x^3")], "fname" => vec![s("g")], "var" => vec![s("u")], "a" => vec![s("2")], "h" => vec![s("2")], "more_h" => vec![s("0.5")], "more_a" => vec![s("-1"), s("2")], "xlabel" => vec![s("time")], "ylabel" => vec![s("dist")], "title" => vec![s("Tangents")], _ => vec![s("zz")] },
            };
            let changed = alts.iter().any(|v| { let mut q = p.clone(); q.0.insert(pd.key.to_string(), v.clone()); (d.render)(&q) != base });
            assert_eq!(crate::figures::param_relevant(ID, pd.key, &p), changed, "{} in {:?}", pd.key, st);
        }
    }
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
