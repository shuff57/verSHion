//! Tests for the Riemann sums figure: hand-worked values, refusals, the student copy and key, Print, the panel, the
//! handles through the real hit test, links, and the label sweep over a written classroom list.
use super::*;
use crate::figures::{registry, FigureDef, Kind as PKind};

pub(super) fn with(set: &[(&str, Value)]) -> Params {
    let defs = params();
    let mut p = Params::defaults(&defs);
    for (k, v) in set { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&defs, false);
    p
}
pub(super) fn s(v: &str) -> Value { Value::S(v.to_string()) }
fn bo(v: bool) -> Value { Value::B(v) }
fn i(v: i64) -> Value { Value::I(v) }
fn fl(v: f64) -> Value { Value::F(v) }
fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }

/// The plain text of every `<text data-rs=kind>` (tspans joined, scripts inline: "∫02 (x2 + 1) dx = 14/3").
fn texts(svg: &str, kind: &str) -> Vec<String> {
    let mut out = vec![];
    let pat = format!(r#"<text data-rs="{kind}""#);
    let mut rest = svg;
    while let Some(k) = rest.find(&pat) {
        rest = &rest[k..];
        let e = rest.find("</text>").unwrap();
        let inner = &rest[rest.find('>').unwrap() + 1..e];
        let mut t = String::new();
        let mut tag = false;
        for c in inner.chars() { match c { '<' => tag = true, '>' => tag = false, _ if !tag => t.push(c), _ => {} } }
        out.push(t.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">"));
        rest = &rest[e..];
    }
    out
}
fn lines(p: &Params) -> Vec<String> { texts(&render(p), "line") }
fn has_line(p: &Params, want: &str) {
    let ls = lines(p);
    assert!(ls.iter().any(|l| l.contains(want)), "want {want:?} in {ls:#?}");
}
const ALL_LINES: [(&str, bool); 5] = [("show_dx", true), ("sum_line", true), ("integral_line", true), ("exact_line", true), ("error_line", true)];
fn lined(set: &[(&str, Value)]) -> Params {
    let mut v: Vec<(&str, Value)> = ALL_LINES.iter().map(|(k, x)| (*k, bo(*x))).collect();
    v.extend(set.iter().cloned());
    with(&v)
}

#[test]
fn opening_state_is_the_briefs() {
    let d = def();
    let p = crate::figures::start_params(&d);
    assert_eq!((p.s("f"), p.s("a"), p.s("b"), p.i("n"), p.s("rule"), p.s("region"), p.s("strips")), ("x^2 + 1", "0", "2", 4, "Left", "Under f(x)", "Vertical"));
    assert!(p.b("mark_bounds") && p.b("show_answers"));
    let svg = (d.render)(&p);
    assert_eq!(svg.matches(r#"data-rs="strip""#).count(), 4);
    assert_eq!(texts(&svg, "bound"), vec!["a = 0", "b = 2"]);
    assert!(lines(&p).is_empty(), "the opening writes nothing under the graph");
    assert!(warnings(&p).is_empty(), "{:?}", warnings(&p));
}

/// Every number the figure writes, worked by hand.
#[test]
fn hand_worked_sums_integrals_and_errors() {
    // f(x) = x² + 1 on [0, 2], n = 4: Δx = 1/2, x = 0, 1/2, 1, 3/2, 2, f = 1, 5/4, 2, 13/4, 5; ∫ = 8/3 + 2 = 14/3
    let p = lined(&[]);
    has_line(&p, "\u{394}x = (b \u{2212} a)/n = (2 \u{2212} 0)/4 = 1/2");
    has_line(&p, "L4 = \u{394}x[f(x0) + f(x1) + f(x2) + f(x3)]");
    has_line(&p, "= (1/2)(1 + 5/4 + 2 + 13/4) = 15/4");
    has_line(&p, "\u{222b}02 (x2 + 1) dx = 14/3");
    // 15/4 − 14/3 = 45/12 − 56/12 = −11/12
    has_line(&p, "Error = L4 \u{2212} exact = 15/4 \u{2212} 14/3 = \u{2212}11/12");
    // right: (1/2)(5/4 + 2 + 13/4 + 5) = (1/2)(23/2) = 23/4
    has_line(&lined(&[("rule", s("Right"))]), "= (1/2)(5/4 + 2 + 13/4 + 5) = 23/4");
    // midpoint: f(1/4) = 17/16, f(3/4) = 25/16, f(5/4) = 41/16, f(7/4) = 65/16; (1/2)(148/16) = 37/8
    has_line(&lined(&[("rule", s("Midpoint"))]), "= (1/2)(17/16 + 25/16 + 41/16 + 65/16) = 37/8");
    has_line(&lined(&[("rule", s("Midpoint"))]), "M4 = \u{394}x[f(c1) + f(c2) + f(c3) + f(c4)]");
    // trapezoid: (1/4)(1 + 5/2 + 4 + 13/2 + 5) = (1/4)(19) = 19/4
    has_line(&lined(&[("rule", s("Trapezoid"))]), "= (1/4)(1 + 2\u{b7}5/4 + 2\u{b7}2 + 2\u{b7}13/4 + 5) = 19/4");
    has_line(&lined(&[("rule", s("Trapezoid"))]), "T4 = (\u{394}x/2)[f(x0) + 2f(x1) + 2f(x2) + 2f(x3) + f(x4)]");
    // the key line gives all three
    has_line(&with(&[("key_line", bo(true))]), "Key: L4 = 15/4;  \u{222b}02 (x2 + 1) dx = 14/3;  error = \u{2212}11/12");
    // Δx with π: (π − 0)/4 = π/4
    has_line(&lined(&[("f", s("sin(x)")), ("b", s("pi"))]), "(\u{3c0} \u{2212} 0)/4 = \u{3c0}/4");
    // sin x on [0, π]: heights 0, √2/2, 1, √2/2; ∫ = 2
    has_line(&lined(&[("f", s("sin(x)")), ("b", s("pi"))]), "(0 + \u{221a}2/2 + 1 + \u{221a}2/2)");
    has_line(&lined(&[("f", s("sin(x)")), ("b", s("pi"))]), "sin(x) dx = 2");
    // signed: ∫₀² (x² − 1) dx = [x³/3 − x]: from 1 to 2 it is 2/3 − (−2/3) = 4/3, from 0 to 1 it is −2/3; total 2/3
    has_line(&lined(&[("f", s("x^2 - 1")), ("signed", bo(true))]), "\u{222b}02 (x2 \u{2212} 1) dx = 4/3 \u{2212} 2/3 = 2/3");
    // sin x on [0, 2π]: 2 above, 2 below
    let sin = with(&[("f", s("sin(x)")), ("b", s("2pi")), ("strips", s("None (shade the exact area)")), ("signed", bo(true)), ("integral_line", bo(true)), ("exact_line", bo(true)), ("xunits", s("Multiples of \u{3c0}")), ("xmin", fl(-0.5)), ("xmax", fl(2.5)), ("xstep", fl(0.5)), ("ymin", fl(-1.5)), ("ymax", fl(1.5)), ("ystep", fl(0.5))]);
    has_line(&sin, "\u{222b}0(2\u{3c0}) sin(x) dx = 2 \u{2212} 2 = 0".replace("(2\u{3c0})", "2\u{3c0}").as_str());
    // between y = x and y = x²: they cross at 0 and 1; ∫₀¹ (x − x²) dx = 1/2 − 1/3 = 1/6
    let bt = with(&[("region", s("Between f(x) and g(x)")), ("f", s("x")), ("g", s("x^2")), ("bounds", s("Where f and g cross")), ("strips", s("None (shade the exact area)")), ("integral_line", bo(true)), ("exact_line", bo(true)), ("xmin", fl(-0.5)), ("xmax", fl(1.5)), ("xstep", fl(0.5)), ("ymin", fl(-0.5)), ("ymax", fl(1.5)), ("ystep", fl(0.5))]);
    has_line(&bt, "f and g cross at x = 0 and x = 1");
    has_line(&bt, "\u{222b}01 (x \u{2212} x2) dx = 1/6");
    // vertical strips between them, left, n = 4: heights x − x² at 0, 1/4, 1/2, 3/4 = 0, 3/16, 1/4, 3/16; (1/4)(5/8) = 5/32
    let mut bv = bt.clone(); bv.set_s("strips", "Vertical"); bv.set_b("sum_line", true);
    has_line(&bv, "L4 = (1/4)(0 + 3/16 + 1/4 + 3/16) = 5/32");
    // ∫₁⁴ dx/x = ln 4 = 2 ln 2; midpoint, n = 6: Δx = 1/2, c = 5/4, 7/4, …, 15/4, f = 4/5, 4/7, 4/9, 4/11, 4/13, 4/15
    let inv = lined(&[("f", s("1/x")), ("a", s("1")), ("b", s("4")), ("rule", s("Midpoint")), ("n", i(6)), ("xmin", fl(-0.5)), ("xmax", fl(5.0)), ("ymax", fl(1.5)), ("ymin", fl(-0.25)), ("ystep", fl(0.25)), ("xstep", fl(0.5))]);
    has_line(&inv, "(1/2)(4/5 + 4/7 + 4/9 + 4/11 + 4/13 + 4/15)");
    has_line(&inv, "dx = 2 ln 2");
    // √x on [0, 4]: ∫ = (2/3)·8 = 16/3; trapezoid n = 4: (1/2)(0 + 2 + 2√2 + 2√3 + 2) = 2 + √2 + √3 ≈ 5.1463
    let sq = lined(&[("f", s("sqrt(x)")), ("b", s("4")), ("rule", s("Trapezoid")), ("xmax", fl(5.0)), ("ymax", fl(3.0))]);
    has_line(&sq, "= (1/2)(0 + 2\u{b7}1 + 2\u{b7}\u{221a}2 + 2\u{b7}\u{221a}3 + 2) \u{2248} 5.1463");
    has_line(&sq, "dx = 16/3");
    // the error 5.14626… − 5.33333… = −0.18707…, to 3 places
    let mut sqd = sq.clone(); sqd.set_s("values", "Decimals"); sqd.set_i("dec", 3);
    has_line(&sqd, "Error = T4 \u{2212} exact = 5.146 \u{2212} 5.333 \u{2248} \u{2212}0.187");
    has_line(&sqd, "= 0.5(0 + 2\u{b7}1 + 2\u{b7}1.414 + 2\u{b7}1.732 + 2) \u{2248} 5.146");
    // ∫₀^{π/4} cos x dx = √2/2; ∫₀¹ eˣ dx = e − 1; ∫₀^π x dx = π²/2
    has_line(&with(&[("f", s("cos(x)")), ("b", s("pi/4")), ("exact_line", bo(true))]), "Exact value = \u{221a}2/2");
    has_line(&with(&[("f", s("e^x")), ("b", s("1")), ("exact_line", bo(true))]), "Exact value = e \u{2212} 1");
    has_line(&with(&[("f", s("x")), ("b", s("pi")), ("exact_line", bo(true))]), "Exact value = \u{3c0}2/2");
    // no exact form: ∫₀¹ e^(−x²) dx = 0.746824…
    has_line(&with(&[("f", s("e^(-x^2)")), ("b", s("1")), ("exact_line", bo(true))]), "Exact value \u{2248} 0.7468");
}

/// Upper and lower sums take the greatest and least value anywhere on the strip (owner's decision), not an end value.
#[test]
fn upper_and_lower_sums_use_the_true_extremes() {
    // f(x) = 3x − x² on [0, 3], 3 strips: the middle strip [1, 2] has f(1) = f(2) = 2 at its ends but 9/4 at x = 3/2.
    // Upper = 1·(2 + 9/4 + 2) = 25/4 (end values alone would give 6); lower = 1·(0 + 2 + 0) = 2.
    let up = with(&[("f", s("3x - x^2")), ("b", s("3")), ("rule", s("Upper")), ("n", i(3)), ("sum_line", bo(true))]);
    has_line(&up, "= 1\u{b7}(2 + 9/4 + 2) = 25/4");
    has_line(&up, "Upper sum = \u{394}x[M1 + M2 + M3]");
    let m = model(&up).unwrap();
    assert!((m.strips[1].top - 2.25).abs() < 1e-12, "the middle strip is 9/4 tall: {}", m.strips[1].top);
    assert!(m.samples.iter().any(|q| (q.0 - 1.5).abs() < 1e-9), "its sample point is the vertex x = 3/2");
    let lo = with(&[("f", s("3x - x^2")), ("b", s("3")), ("rule", s("Lower")), ("n", i(3)), ("sum_line", bo(true))]);
    has_line(&lo, "= 1\u{b7}(0 + 2 + 0) = 2");
    // f(x) = (x − 3/2)² + 1 on [0, 3] has its least value 1 inside the middle strip: lower = 1·(5/4 + 1 + 5/4) = 7/2,
    // upper = 1·(13/4 + 5/4 + 13/4) = 31/4; with one strip the lower sum is 3·1 = 3 (the ends give 13/4)
    let v = |rule: &str, n: i64| with(&[("f", s("(x - 1.5)^2 + 1")), ("b", s("3")), ("rule", s(rule)), ("n", i(n)), ("sum_line", bo(true)), ("xmax", fl(4.0))]);
    has_line(&v("Lower", 3), "= 1\u{b7}(5/4 + 1 + 5/4) = 7/2");
    has_line(&v("Upper", 3), "= 1\u{b7}(13/4 + 5/4 + 13/4) = 31/4");
    has_line(&v("Lower", 1), "= 3\u{b7}(1) = 3");
    // a strip whose least value is at an end stays at the end
    let m = model(&v("Lower", 3)).unwrap();
    assert!((m.samples[0].0 - 1.0).abs() < 1e-9 && (m.samples[2].0 - 2.0).abs() < 1e-9, "{:?}", m.samples);
    // sin x on [0, π], 2 strips: each strip's greatest value is 1 at π/2 (shared end); the least is 0 at 0 and π
    let s2 = with(&[("f", s("sin(x)")), ("b", s("pi")), ("rule", s("Upper")), ("n", i(2)), ("sum_line", bo(true))]);
    has_line(&s2, "= (\u{3c0}/2)(1 + 1) = \u{3c0}");
    // a cubic whose interior extremum is irrational: x³ − 3x on [−2, 0], one strip: greatest value 2 at x = −1
    has_line(&with(&[("f", s("x^3 - 3x")), ("a", s("-2")), ("b", s("0")), ("rule", s("Upper")), ("n", i(1)), ("sum_line", bo(true)), ("xmin", fl(-3.0)), ("ymin", fl(-3.0)), ("ymax", fl(3.0))]), "= 2\u{b7}(2) = 4");
}

#[test]
fn tables_and_horizontal_strips() {
    // table 0,2; 1,5; 3,6; 4,4; 7,3: widths 1, 2, 1, 3
    let t = |rule: &str| with(&[("region", s("From a table of values")), ("rule", s(rule)), ("sum_line", bo(true)), ("integral_line", bo(true)), ("xmax", fl(8.0)), ("ymax", fl(7.0))]);
    has_line(&t("Left"), "L4 = 1\u{b7}2 + 2\u{b7}5 + 1\u{b7}6 + 3\u{b7}4 = 30");
    has_line(&t("Right"), "R4 = 1\u{b7}5 + 2\u{b7}6 + 1\u{b7}4 + 3\u{b7}3 = 30");
    // trapezoids: 1·(2+5)/2 + 2·(5+6)/2 + 1·(6+4)/2 + 3·(4+3)/2 = 7/2 + 11 + 5 + 21/2 = 30
    has_line(&t("Trapezoid"), "T4 = 1\u{b7}(7/2) + 2\u{b7}(11/2) + 1\u{b7}5 + 3\u{b7}(7/2) = 30");
    has_line(&t("Left"), "\u{222b}07 f(x) dx \u{2248} L4 = 30");
    // a midpoint sum from 5 rows: strips [0, 2] and [2, 4] at heights 3 and 2: 2·3 + 2·2 = 10
    let mut mid = t("Midpoint"); mid.set_s("table", "0, 1; 1, 3; 2, 4; 3, 2; 4, 5");
    has_line(&mid, "M2 = 2\u{b7}3 + 2\u{b7}2 = 10");
    // horizontal strips between √x and x/2 (crossing at 0 and 4): at height y the region runs from x = y² to x = 2y,
    // width 2y − y²; midpoints 1/4, 3/4, 5/4, 7/4 give 7/16, 15/16, 15/16, 7/16, times Δy = 1/2: 11/8.
    // ∫₀² (2y − y²) dy = 4 − 8/3 = 4/3; error 11/8 − 4/3 = 1/24
    let h = lined(&[("region", s("Between f(x) and g(x)")), ("f", s("sqrt(x)")), ("g", s("x/2")), ("bounds", s("Where f and g cross")), ("strips", s("Horizontal")), ("rule", s("Midpoint")), ("xmax", fl(5.0)), ("ymax", fl(3.0))]);
    has_line(&h, "\u{394}y = (d \u{2212} c)/n = (2 \u{2212} 0)/4 = 1/2");
    has_line(&h, "= (1/2)(7/16 + 15/16 + 15/16 + 7/16) = 11/8");
    has_line(&h, "\u{222b}02 (xR \u{2212} xL) dy = 4/3");
    has_line(&h, "Error = M4 \u{2212} exact = 11/8 \u{2212} 4/3 = 1/24");
    // bottom of each strip (Left): widths at 0, 1/2, 1, 3/2 = 0, 3/4, 1, 3/4: (1/2)(5/2) = 5/4
    let mut hl = h.clone(); hl.set_s("rule", "Left");
    has_line(&hl, "= (1/2)(0 + 3/4 + 1 + 3/4) = 5/4");
}

#[test]
fn refusals_are_said_and_never_drawn_wrong() {
    let cases: Vec<(Params, &str)> = vec![
        (with(&[("f", s("x^^2"))]), "cannot be read"),
        (with(&[("f", s("y = x + y"))]), "one expression in x"),
        (with(&[("f", s(""))]), "Type f(x)"),
        (with(&[("a", s("3"))]), "b must be greater than a"),
        (with(&[("a", s("pie"))]), "a cannot be read"),
        (with(&[("f", s("1/x")), ("a", s("-1")), ("b", s("1"))]), "no value at x"),
        (with(&[("f", s("sqrt(x)")), ("a", s("-1"))]), "no value at x"),
        (with(&[("f", s("tan(x)")), ("b", s("2"))]), "no value at x"),
        (with(&[("region", s("From a table of values")), ("table", s("0, 1; 2, 3; 1, 2"))]), "must increase"),
        (with(&[("region", s("From a table of values")), ("table", s("0, 1; 2"))]), "cannot be read"),
        (with(&[("region", s("From a table of values")), ("rule", s("Upper"))]), "no upper or lower sum"),
        (with(&[("region", s("From a table of values")), ("rule", s("Midpoint")), ("table", s("0, 1; 1, 2; 2, 2; 3, 4"))]), "odd number of rows"),
        (with(&[("region", s("From a table of values")), ("rule", s("Midpoint")), ("table", s("0, 1; 1, 2; 3, 2"))]), "halfway"),
        (with(&[("region", s("Between f(x) and g(x)")), ("f", s("x")), ("g", s("x^2")), ("strips", s("Horizontal")), ("rule", s("Trapezoid"))]), "Horizontal strips use"),
        (with(&[("region", s("Between f(x) and g(x)")), ("g", s("x - 5")), ("bounds", s("Where f and g cross"))]), "do not cross twice"),
    ];
    for (p, want) in cases {
        let n = warnings(&p);
        assert!(n.iter().any(|x| x.contains(want)), "want {want:?} in {n:?}");
        let svg = render(&p);
        assert!(!svg.contains(r#"data-rs="strip""#) && !svg.contains(r#"data-rs="line""#), "{want}: nothing is drawn as if it were right");
        assert!(svg.contains("<svg") && svg.contains("<polygon"), "the axes are still drawn");
    }
}

#[test]
fn student_copy_and_key() {
    let d = def();
    let mut p = lined(&[("key_line", bo(true)), ("strip_labels", s("Heights"))]);
    let (student, key) = crate::figures::answer_pair(&d, &p).unwrap();
    for ans in ["15/4", "14/3", "11/12", "1/2)"] {
        assert!(key.contains(ans), "the key gives {ans}");
        assert!(!texts(&student, "line").iter().any(|l| l.contains(ans)), "the student copy hides {ans}");
    }
    assert_eq!(student.matches(r#"data-rs="blank""#).count(), 4, "Δx, the sum, the integral and the error are blank lines");
    assert!(!student.contains("Key:"), "no key line on the student copy");
    // the given heights stay; the SVG's title and description carry no answer
    assert_eq!(texts(&student, "strip-label"), vec!["1", "5/4", "2", "13/4"]);
    let full = d.svg(&{ let mut q = p.clone(); q.set_b("show_answers", false); q });
    let head = &full[..full.find("</desc>").unwrap()];
    assert!(!head.contains("15/4") && !head.contains("14/3"));
    // curve and bounds only: no strips, points, labels or x marks; the curve and a, b stay
    p.set_s("student", "Curve and bounds only");
    p.set_b("sample_pts", true);
    p.set_b("x_marks", true);
    p.set_b("show_answers", false);
    let only = render(&p);
    assert!(!only.contains(r#"data-rs="strip""#) && !only.contains(r#"data-rs="dot""#) && !only.contains(r#"data-rs="strip-label""#) && !only.contains(r#"data-rs="xmark""#));
    assert_eq!(texts(&only, "bound"), vec!["a = 0", "b = 2"]);
    assert!(only.contains("<polyline"), "the curve");
    // strip heights blank: a blank on each strip; the key writes the heights
    p.set_s("student", "Strip heights blank");
    let st = render(&p);
    assert_eq!(st.matches(r#"data-rs="blank""#).count(), 4 + 4);
    assert!(texts(&st, "strip-label").is_empty());
    p.set_b("show_answers", true);
    assert_eq!(texts(&render(&p), "strip-label"), vec!["1", "5/4", "2", "13/4"]);
    // without height labels the student copy still has the blanks, and the panel asks for Heights for the key
    let q = with(&[("show_answers", bo(false)), ("student", s("Strip heights blank"))]);
    assert_eq!(render(&q).matches(r#"data-rs="blank""#).count(), 4);
    assert!(warnings(&q).iter().any(|n| n.contains("Heights")));
    // crossings found are answers: blank on the student copy, a and b named only
    let bt = with(&[("region", s("Between f(x) and g(x)")), ("f", s("x")), ("g", s("x^2")), ("bounds", s("Where f and g cross")), ("show_answers", bo(false)), ("xmin", fl(-0.5)), ("xmax", fl(1.5))]);
    let sv = render(&bt);
    assert_eq!(texts(&sv, "bound"), vec!["a", "b"]);
    assert_eq!(sv.matches(r#"data-rs="blank""#).count(), 2);
}

#[test]
fn print_is_grey_only() {
    let mut cases: Vec<Params> = examples().iter().map(|e| with(&e.params)).collect();
    cases.push(lined(&[("f", s("x^2 - 1")), ("signed", bo(true)), ("sample_pts", bo(true)), ("x_marks", bo(true)), ("strip_labels", s("Areas")), ("key_line", bo(true))]));
    cases.push(lined(&[("f", s("x^2 - 1")), ("signed", bo(true)), ("shading", s("Hatching"))]));
    for mut p in cases {
        p.set_s("style", "Print");
        let svg = render(&p);
        let bad = crate::figures::census::non_grey_paints(&svg);
        assert!(bad.is_empty(), "{bad:?}");
        // above and below the axis are told apart without colour: tint against hatching
        if p.b("signed") && p.s("f") == "x^2 - 1" { assert!(svg.contains("url(#rs-hn)") && (svg.contains("#a8a8a8") || svg.contains("url(#rs-hp)"))); }
    }
}

/// Each control is shown in the panel exactly when changing it changes the drawing (hidden: no effect at all).
#[test]
fn controls_show_exactly_when_they_change_the_drawing() {
    let defs = params();
    let mut states: Vec<Params> = vec![
        with(&[]),
        lined(&[("key_line", bo(true)), ("strip_labels", s("Heights")), ("x_marks", bo(true)), ("sample_pts", bo(true))]),
        lined(&[("show_answers", bo(false))]),
        with(&[("show_answers", bo(false)), ("student", s("Curve and bounds only"))]),
        lined(&[("f", s("x^2 - 1")), ("values", s("Decimals"))]),
        with(&[("strips", s("None (shade the exact area)")), ("f", s("x^2 - 1")), ("b", s("pi")), ("exact_line", bo(true))]),
        lined(&[("region", s("Between f(x) and g(x)")), ("f", s("x")), ("g", s("x^2")), ("bounds", s("Where f and g cross"))]),
        lined(&[("region", s("Between f(x) and g(x)")), ("f", s("sqrt(x)")), ("g", s("x/2")), ("strips", s("Horizontal")), ("b", s("4")), ("xmax", fl(5.0))]),
        lined(&[("region", s("From a table of values")), ("rule", s("Trapezoid")), ("xmax", fl(8.0)), ("ymax", fl(7.0)), ("strip_labels", s("Heights"))]),
        with(&[("grid", s("None")), ("title_mode", s("Blank line"))]),
    ];
    // every student copy with each line option on alone (the sums of x² + 1 are fractions, so Values always shows)
    for mode in STUDENT {
        for opt in ["sum_line", "show_dx", "exact_line", "integral_line", "error_line", "key_line", "strip_labels", "x_marks", "sample_pts"] {
            let mut q = with(&[("show_answers", bo(false)), ("student", s(mode))]);
            if opt == "strip_labels" { q.set_s("strip_labels", "Heights"); } else { q.set_b(opt, true); }
            states.push(q);
        }
    }
    let alts = |pd: &ParamDef| -> Vec<Value> {
        match &pd.kind {
            PKind::Bool => vec![bo(true), bo(false)],
            PKind::Choice(o) => o.iter().map(|x| s(x)).collect(),
            PKind::Int { min, max } => vec![i(*min), i(*max), i((min + max) / 2 + 1)],
            PKind::Float { .. } => vec![],
            _ => match pd.key {
                "f" => vec![s("x^3"), s("2x + 1")],
                "g" => vec![s("x"), s("2x - x^2")],
                "a" => vec![s("1/2"), s("1")],
                "b" => vec![s("3"), s("3/2")],
                "table" => vec![s("0, 1; 2, 3; 4, 2"), s("0, -1; 1, 2; 3, 1")],
                "title" => vec![s("Area")],
                "xlabel" | "ylabel" => vec![s("t"), s("")],
                _ => vec![],
            },
        }
    };
    let mut checked = 0;
    for st in &states {
        let base = render(st);
        for pd in &defs {
            if pd.key == "show_answers" || matches!(pd.kind, PKind::Float { .. }) { continue; }
            let rel = relevant(pd.key, st);
            let mut changed = false;
            for v in alts(pd) {
                let mut q = st.clone();
                if q.0.get(pd.key) == Some(&v) { continue; }
                q.0.insert(pd.key.to_string(), v.clone());
                q.normalize(&defs, false);
                let other = render(&q);
                if other != base { changed = true; }
                if !rel { assert_eq!(other, base, "{} = {v:?} is hidden but changes the drawing in state {:?}", pd.key, st.0); }
            }
            if rel { assert!(changed, "{} is shown but no value changes the drawing in state {:?}", pd.key, st.0); }
            checked += 1;
        }
    }
    assert!(checked > 1000);
}

#[test]
fn handles_through_the_real_hit_test() {
    let d = def();
    let p = crate::figures::start_params(&d);
    let svg = (d.render)(&p);
    let size = crate::figures::svg_size(&svg);
    let hs = d.handles(&p, size);
    let tips: Vec<&str> = hs.iter().map(|h| h.tip.as_str()).collect();
    assert!(tips.contains(&"Drag a along the x-axis") && tips.contains(&"Drag b along the x-axis") && tips.contains(&"Scroll over the strips to add or take away strips"), "{tips:?}");
    let g = grid_layout(&grid_params(&p));
    let sx = |x: f64| g.ml + (x - g.x0) * g.ux;
    let sy = |y: f64| g.mt + g.ph - (y - g.y0) * g.uy;
    // a press on a's knob (anywhere within a few px) goes to a; dragging it to x = 1/2 types 1/2
    for (dx, dy) in [(0.0, 0.0), (3.0, -4.0), (-5.0, 5.0)] {
        let k = crate::figures::interact::pick(&hs, (sx(0.0) + dx, sy(0.0) + dy), 1.0).unwrap();
        assert_eq!(hs[k].tip, "Drag a along the x-axis");
    }
    let k = crate::figures::interact::pick(&hs, (sx(0.0), sy(0.0)), 1.0).unwrap();
    let mut q = p.clone();
    (hs[k].drag.as_ref().unwrap())(&mut q, (sx(0.52), sy(0.0)));
    assert_eq!(q.s("a"), "1/2");
    // a can never pass b: dragged far right it stops a quarter step short of b
    let mut q = p.clone();
    (hs[k].drag.as_ref().unwrap())(&mut q, (sx(2.9), sy(0.0)));
    assert_eq!(q.s("a"), "7/4");
    let k = crate::figures::interact::pick(&hs, (sx(2.0), sy(0.0)), 1.0).unwrap();
    assert_eq!(hs[k].tip, "Drag b along the x-axis");
    let mut q = p.clone();
    (hs[k].drag.as_ref().unwrap())(&mut q, (sx(2.76), sy(0.3)));
    assert_eq!(q.s("b"), "11/4");
    // over the strips the press goes to the scroll zone; scrolling up adds a strip, down takes one away
    for (x, y) in [(1.0, 1.0), (0.3, 0.5), (1.7, 2.5)] {
        let k = crate::figures::interact::pick(&hs, (sx(x), sy(y)), 1.0).unwrap();
        assert_eq!(hs[k].tip, "Scroll over the strips to add or take away strips");
        let mut q = p.clone();
        (hs[k].scroll.as_ref().unwrap())(&mut q, 1.0);
        assert_eq!(q.i("n"), 5);
        (hs[k].scroll.as_ref().unwrap())(&mut q, -1.0);
        (hs[k].scroll.as_ref().unwrap())(&mut q, -1.0);
        assert_eq!(q.i("n"), 3);
    }
    // in π units a dragged bound is typed as a multiple of π
    let pp = with(&[("f", s("sin(x)")), ("b", s("pi")), ("xunits", s("Multiples of \u{3c0}")), ("xmin", fl(-0.5)), ("xmax", fl(1.5)), ("xstep", fl(0.5))]);
    let hs = d.handles(&pp, crate::figures::svg_size(&render(&pp)));
    let g = grid_layout(&grid_params(&pp));
    let sxp = |x: f64| g.ml + (x - g.x0) * g.ux;
    let k = crate::figures::interact::pick(&hs, (sxp(PI), g.mt + g.ph - (0.0 - g.y0) * g.uy), 1.0).unwrap();
    let mut q = pp.clone();
    (hs[k].drag.as_ref().unwrap())(&mut q, (sxp(3.0 * PI / 4.0 + 0.05), 0.0));
    assert_eq!(q.s("b"), "3pi/4");
    assert!(read_bound(q.s("b"), "b").is_ok_and(|(v, t)| (v - 3.0 * PI / 4.0).abs() < 1e-12 && t == "3\u{3c0}/4"));
}

#[test]
fn links_and_saved_figures_round_trip() {
    let d = def();
    let base = crate::figures::start_params(&d);
    for p in [
        lined(&[("f", s("sin(x)")), ("a", s("pi/6")), ("b", s("5pi/6")), ("rule", s("Upper")), ("n", i(7)), ("values", s("Decimals")), ("dec", i(3))]),
        with(&[("region", s("From a table of values")), ("table", s("0, 1/2; 1, 3; 5/2, 4")), ("rule", s("Trapezoid")), ("title", s("Water flow")), ("show_answers", bo(false))]),
        with(&[("region", s("Between f(x) and g(x)")), ("f", s("x")), ("g", s("x^2")), ("bounds", s("Where f and g cross")), ("strips", s("Horizontal")), ("style", s("Print"))]),
    ] {
        let link = crate::figures::share::encode(ID, &p, &base);
        let (id, m) = crate::figures::share::decode(&link).unwrap();
        assert_eq!(id, ID);
        let back = crate::figures::share::link_params(&d, m, crate::figures::share::VERSION);
        assert_eq!(render(&back), render(&p), "{link}");
        // a saved spec stores every param as JSON and loads it back the same
        let json = serde_json::to_value(p.0.iter().map(|(k, v)| (k.clone(), match v { Value::F(x) => serde_json::json!(x), Value::I(x) => serde_json::json!(x), Value::B(x) => serde_json::json!(x), Value::S(x) => serde_json::json!(x) })).collect::<serde_json::Map<_, _>>()).unwrap();
        let loaded = crate::figures::params_from_json(&d, &json);
        assert_eq!(render(&loaded), render(&p));
    }
    // the same params always draw the same figure, and a setting that changes nothing moves nothing
    let p = lined(&[]);
    assert_eq!(render(&p), render(&p));
}

// ───────────────────────── the label sweep ─────────────────────────

type Seg = ((f64, f64), (f64, f64));
fn nums_of(s: &str) -> Vec<f64> { s.split([' ', ',']).filter_map(|t| t.parse().ok()).collect() }
fn tag_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> { let pat = format!(" {name}=\""); let i = tag.find(&pat)? + pat.len(); tag[i..].split('"').next() }
fn tags<'a>(svg: &'a str, open: &str) -> Vec<&'a str> {
    let mut out = vec![];
    let mut rest = svg;
    while let Some(k) = rest.find(open) { rest = &rest[k..]; let e = rest.find('>').unwrap(); out.push(&rest[..e]); rest = &rest[e..]; }
    out
}
fn clip(a: (f64, f64), b: (f64, f64), r: [f64; 4]) -> Option<Seg> {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    for (p, q) in [(-dx, a.0 - r[0]), (dx, r[2] - a.0), (-dy, a.1 - r[1]), (dy, r[3] - a.1)] {
        if p == 0.0 { if q < 0.0 { return None; } continue; }
        let t = q / p;
        if p < 0.0 { t0 = t0.max(t); } else { t1 = t1.min(t); }
    }
    (t0 <= t1).then(|| ((a.0 + t0 * dx, a.1 + t0 * dy), (a.0 + t1 * dx, a.1 + t1 * dy)))
}

/// Problems with the labels of one drawn figure, read from its SVG: a label on a stroke (strip edges, bound lines, ticks,
/// the axes, the curves), on a dot, on another label or text, or off the page.
fn label_faults(svg: &str) -> Vec<String> {
    let (w, h) = crate::figures::svg_size(svg);
    let at = svg.find(r#"<clipPath id="rsclip">"#).unwrap();
    let clip_tag = tags(&svg[at..], "<rect");
    let r: Vec<f64> = clip_tag.first().map(|t| ["x", "y", "width", "height"].iter().map(|k| tag_attr(t, k).unwrap().parse().unwrap()).collect()).unwrap();
    let plot = [r[0], r[1], r[0] + r[2], r[1] + r[3]];
    let mut segs: Vec<Seg> = vec![];
    for t in tags(svg, r#"<polygon data-rs="strip""#) {
        let v = nums_of(tag_attr(t, "points").unwrap());
        let pts: Vec<(f64, f64)> = v.chunks(2).map(|c| (c[0], c[1])).collect();
        for k in 0..pts.len() { if let Some(sg) = clip(pts[k], pts[(k + 1) % pts.len()], plot) { segs.push(sg); } }
    }
    for t in tags(svg, "<line ") {
        let get = |k: &str| tag_attr(t, k).and_then(|v| v.parse::<f64>().ok());
        let (Some(x1), Some(y1), Some(x2), Some(y2)) = (get("x1"), get("y1"), get("x2"), get("y2")) else { continue };
        let rs = tag_attr(t, "data-rs");
        let axis = matches!(tag_attr(t, "stroke-width"), Some("2.4") | Some("2.6"));
        if matches!(rs, Some("bound") | Some("tick")) || axis { segs.push(((x1, y1), (x2, y2))); }
    }
    for t in tags(svg, "<polyline ") {
        let v = nums_of(tag_attr(t, "points").unwrap_or(""));
        let pts: Vec<(f64, f64)> = v.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).collect();
        for k in 1..pts.len() { if let Some(sg) = clip(pts[k - 1], pts[k], plot) { segs.push(sg); } }
    }
    let mut boxes: Vec<[f64; 4]> = text_boxes(svg);
    for t in tags(svg, r#"<circle data-rs="dot""#) {
        let (cx, cy): (f64, f64) = (tag_attr(t, "cx").unwrap().parse().unwrap(), tag_attr(t, "cy").unwrap().parse().unwrap());
        boxes.push([cx - 3.5, cy - 3.5, cx + 3.5, cy + 3.5]);
    }
    let mut labels: Vec<(String, [f64; 4], bool)> = vec![];
    for open in [r#"<text data-rs="#, r#"<line data-rs="blank""#] {
        for t in tags(svg, open) {
            let Some(bx) = tag_attr(t, "data-box") else { continue };
            let v = nums_of(bx);
            labels.push((tag_attr(t, "data-rs").unwrap().to_string(), [v[0], v[1], v[2], v[3]], open.contains("blank")));
        }
    }
    let mut out = vec![];
    for (k, (kind, bx, blank)) in labels.iter().enumerate() {
        if bx[0] < 0.0 || bx[1] < 0.0 || bx[2] > w || bx[3] > h { out.push(format!("{kind} off the page {bx:?}")); }
        if !blank && let Some(sg) = segs.iter().find(|(a, c)| rect_seg(bx, *a, *c)) { out.push(format!("{kind} {bx:?} on a stroke {sg:?}")); }
        if *blank { let inner = [bx[0] + 0.5, bx[1], bx[2] - 0.5, bx[3]]; if let Some(sg) = segs.iter().find(|(a, c)| rect_seg(&inner, *a, *c)) { out.push(format!("blank {bx:?} on a stroke {sg:?}")); } }
        if let Some(o) = boxes.iter().find(|o| rect_hit(bx, o)) { out.push(format!("{kind} {bx:?} on a text or dot {o:?}")); }
        for (kind2, b2, _) in &labels[k + 1..] {
            // (a written line and its own blank sit side by side)
            if rect_hit(bx, b2) && !(kind == "line" && kind2 == "blank" || kind == "blank" && kind2 == "line") { out.push(format!("{kind} {bx:?} on {kind2} {b2:?}")); }
        }
    }
    out
}

/// The written classroom list: the functions, intervals, strip counts and rules a calculus textbook uses.
fn classroom() -> Vec<(String, Vec<(&'static str, Value)>)> {
    let win = |x0: f64, x1: f64, xs: f64, y0: f64, y1: f64, ys: f64| vec![("xmin", fl(x0)), ("xmax", fl(x1)), ("xstep", fl(xs)), ("ymin", fl(y0)), ("ymax", fl(y1)), ("ystep", fl(ys))];
    let pi_win = |x0: f64, x1: f64, y0: f64, y1: f64| { let mut v = win(x0, x1, 0.5, y0, y1, 0.5); v.push(("xunits", s("Multiples of \u{3c0}"))); v };
    let fns: Vec<(&str, &str, &str, Vec<(&'static str, Value)>)> = vec![
        ("x^2", "0", "2", win(-1.0, 3.0, 1.0, -1.0, 5.0, 1.0)),
        ("x^2 + 1", "0", "2", win(-1.0, 3.0, 1.0, -1.0, 6.0, 1.0)),
        ("x^3", "0", "2", win(-1.0, 3.0, 1.0, -1.0, 9.0, 1.0)),
        ("2x + 1", "0", "3", win(-1.0, 4.0, 1.0, -1.0, 8.0, 1.0)),
        ("4 - x^2", "0", "2", win(-1.0, 3.0, 1.0, -1.0, 5.0, 1.0)),
        ("sqrt(x)", "0", "4", win(-1.0, 5.0, 1.0, -1.0, 3.0, 0.5)),
        ("1/x", "1", "4", win(-0.5, 5.0, 0.5, -0.25, 1.5, 0.25)),
        ("e^x", "0", "2", win(-1.0, 3.0, 1.0, -1.0, 8.0, 1.0)),
        ("sin(x)", "0", "pi", pi_win(-0.5, 1.5, -0.5, 1.5)),
        ("cos(x)", "0", "pi/2", pi_win(-0.5, 1.0, -0.5, 1.5)),
        ("x^2 - 4x + 5", "0", "4", win(-1.0, 5.0, 1.0, -1.0, 6.0, 1.0)),
        ("3x - x^2", "0", "3", win(-1.0, 4.0, 1.0, -1.0, 3.0, 0.5)),
        ("ln(x)", "1", "3", win(-0.5, 3.5, 0.5, -1.0, 2.0, 0.5)),
        ("1/(1 + x^2)", "0", "1", win(-0.5, 1.5, 0.5, -0.25, 1.25, 0.25)),
        ("x^2 - 1", "0", "2", win(-1.0, 3.0, 1.0, -2.0, 4.0, 1.0)),
        ("x^3 - 3x", "-2", "2", win(-3.0, 3.0, 1.0, -3.0, 3.0, 1.0)),
    ];
    let mut out = vec![];
    for (f, a, b, w) in &fns {
        for rule in RULES {
            let n = if *f == "x^3 - 3x" { 4 } else { 4 };
            let mut v: Vec<(&'static str, Value)> = vec![("f", s(f)), ("a", s(a)), ("b", s(b)), ("rule", s(rule)), ("n", i(n))];
            v.extend(w.iter().cloned());
            if *f == "x^2 - 1" || *f == "x^3 - 3x" { v.push(("signed", bo(true))); }
            out.push((format!("{f} [{a}, {b}] {rule} n=4"), v));
        }
    }
    // other strip counts
    for (f, a, b, n, w) in [("x^2", "0", "2", 2, win(-1.0, 3.0, 1.0, -1.0, 5.0, 1.0)), ("x^2", "0", "3", 6, win(-1.0, 4.0, 1.0, -1.0, 10.0, 1.0)), ("sqrt(x)", "0", "4", 8, win(-1.0, 5.0, 1.0, -1.0, 3.0, 0.5)), ("1/x", "1", "4", 6, win(-0.5, 5.0, 0.5, -0.25, 1.5, 0.25)), ("x^3", "0", "1", 5, win(-0.5, 1.5, 0.5, -0.25, 1.25, 0.25))] {
        for rule in ["Left", "Midpoint", "Trapezoid"] {
            let mut v: Vec<(&'static str, Value)> = vec![("f", s(f)), ("a", s(a)), ("b", s(b)), ("rule", s(rule)), ("n", i(n))];
            v.extend(w.clone());
            out.push((format!("{f} [{a}, {b}] {rule} n={n}"), v));
        }
    }
    // tables
    for (t, w) in [("0, 2; 1, 5; 3, 6; 4, 4; 7, 3", win(-1.0, 8.0, 1.0, -1.0, 7.0, 1.0)), ("0, 10; 2, 14; 4, 20; 7, 18; 10, 12", win(-1.0, 11.0, 1.0, -2.0, 22.0, 2.0)), ("0, 1; 1, 3; 2, 4; 3, 2; 4, 5", win(-1.0, 5.0, 1.0, -1.0, 6.0, 1.0))] {
        for rule in ["Left", "Right", "Trapezoid", "Midpoint"] {
            if rule == "Midpoint" && t.starts_with("0, 2") { continue; }
            let mut v: Vec<(&'static str, Value)> = vec![("region", s("From a table of values")), ("table", s(t)), ("rule", s(rule))];
            v.extend(w.clone());
            out.push((format!("table {t} {rule}"), v));
        }
    }
    // between two curves, the exact area and horizontal strips
    let bt: Vec<(&str, &str, &str, Vec<(&'static str, Value)>)> = vec![
        ("x", "x^2", "cross", win(-0.5, 1.5, 0.5, -0.5, 1.5, 0.5)),
        ("sqrt(x)", "x/2", "cross", win(-1.0, 5.0, 1.0, -1.0, 3.0, 0.5)),
        ("x + 2", "x^2", "cross", win(-2.0, 3.0, 1.0, -1.0, 5.0, 1.0)),
        ("4 - x^2", "x^2 - 4", "cross", win(-3.0, 3.0, 1.0, -5.0, 5.0, 1.0)),
        ("x^2 + 1", "x", "0 to 2", win(-1.0, 3.0, 1.0, -1.0, 6.0, 1.0)),
    ];
    for (f, g, how, w) in &bt {
        for (strips, rule) in [("Vertical", "Left"), ("Vertical", "Midpoint"), ("None (shade the exact area)", "Left"), ("Horizontal", "Midpoint"), ("Horizontal", "Left")] {
            if *how == "0 to 2" && strips == "Horizontal" && rule == "Left" { continue; }
            let mut v: Vec<(&'static str, Value)> = vec![("region", s("Between f(x) and g(x)")), ("f", s(f)), ("g", s(g)), ("strips", s(strips)), ("rule", s(rule)), ("n", i(4))];
            if *how == "cross" { v.push(("bounds", s("Where f and g cross"))); } else { v.push(("a", s("0"))); v.push(("b", s("2"))); }
            v.extend(w.iter().cloned());
            out.push((format!("between {f} and {g} {strips} {rule}"), v));
        }
    }
    for (f, a, b, w) in [("sin(x)", "0", "2pi", pi_win(-0.5, 2.5, -1.5, 1.5)), ("x^2 - 1", "0", "2", win(-1.0, 3.0, 1.0, -2.0, 4.0, 1.0)), ("x^2", "0", "3", win(-1.0, 4.0, 1.0, -1.0, 10.0, 1.0)), ("cos(x)", "-pi/2", "pi/2", pi_win(-1.0, 1.0, -0.5, 1.5))] {
        let mut v: Vec<(&'static str, Value)> = vec![("f", s(f)), ("a", s(a)), ("b", s(b)), ("strips", s("None (shade the exact area)")), ("signed", bo(true))];
        v.extend(w);
        out.push((format!("area {f} [{a}, {b}]"), v));
    }
    out
}

const OPTIONS: [&str; 11] = ["sample_pts", "strip_heights", "strip_areas", "x_marks", "show_dx", "sum_line", "integral_line", "exact_line", "error_line", "key_line", "student"];
fn option(p: &mut Params, o: &str) {
    match o {
        "strip_heights" => p.set_s("strip_labels", "Heights"),
        "strip_areas" => p.set_s("strip_labels", "Areas"),
        "student" => { for (k, _) in ALL_LINES { p.set_b(k, true); } p.set_b("show_answers", false); p.set_s("student", "Strip heights blank"); }
        k => p.set_b(k, true),
    }
}

/// Labels left off at Large on the classroom list (each refused with a panel note): strips too narrow for their labels.
/// (x₀ … xₙ under strips that straddle the axis or with every label on at once: only x₀ and xₙ, or none, marked with a note;
/// one set of strip labels between y = x and y = x² with everything on.)
const LARGE_EXCEPTIONS: &[&str] = &[
    "sin(x) [0, pi] Midpoint n=4 / all: x marks",
    "cos(x) [0, pi/2] Midpoint n=4 / all: x marks",
    "sqrt(x) [0, 4] Left n=8 / all: x marks",
    "between x and x^2 Vertical Midpoint / all: strip labels",
    "between 4 - x^2 and x^2 - 4 Vertical Left / x_marks: x marks",
    "between 4 - x^2 and x^2 - 4 Vertical Left / all: x marks",
    "between 4 - x^2 and x^2 - 4 Vertical Midpoint / x_marks: x marks",
    "between 4 - x^2 and x^2 - 4 Vertical Midpoint / all: x marks",
];

#[test]
fn label_sweep_over_the_classroom_list() {
    let list = classroom();
    assert!(list.len() >= 60, "{} cases", list.len());
    let mut renders = 0;
    let mut faults: Vec<String> = vec![];
    let mut refused: std::collections::BTreeMap<(String, &str), usize> = Default::default();
    let mut large_left_off: Vec<String> = vec![];
    for (name, set) in &list {
        let base = with(set);
        assert!(warnings(&base).is_empty(), "{name}: {:?}", warnings(&base));
        for size in ["Small", "Medium", "Large"] {
            let mut combos: Vec<(String, Params)> = vec![("bare".into(), base.clone())];
            for o in OPTIONS { let mut q = base.clone(); option(&mut q, o); combos.push((o.to_string(), q)); }
            let mut all = base.clone();
            for o in OPTIONS { if o != "student" && o != "strip_areas" { option(&mut all, o); } }
            combos.push(("all".into(), all));
            for (what, mut q) in combos {
                q.set_s("label_size", size);
                let (svg, notes) = build(&q);
                renders += 1;
                for f in label_faults(&svg) { faults.push(format!("{name} / {what} / {size}: {f}")); }
                for n in notes.iter().filter(|n| n.contains("no room") || n.contains("do not fit")) {
                    let kind = if n.contains("strip labels") { "strip labels" } else if n.contains("x\u{2080}") { "x marks" } else { "a and b" };
                    *refused.entry((size.to_string(), kind)).or_default() += 1;
                    if size == "Large" { large_left_off.push(format!("{name} / {what}: {kind}")); }
                    else { faults.push(format!("{name} / {what} / {size}: left off: {n}")); }
                }
            }
        }
    }
    println!("label sweep: {} cases, {renders} renders, labels left off (refused with a note) by size and kind: {refused:?}", list.len());
    for l in &large_left_off { println!("  Large, left off: {l}"); }
    assert!(faults.is_empty(), "{} faults:\n{}", faults.len(), faults.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
    let unnamed: Vec<&String> = large_left_off.iter().filter(|l| !LARGE_EXCEPTIONS.contains(&l.as_str())).collect();
    assert_eq!(large_left_off.len(), LARGE_EXCEPTIONS.len(), "every named exception still happens: {large_left_off:#?}");
    assert!(unnamed.is_empty(), "left off at Large and not named: {unnamed:#?}");
}

/// Every example draws with nothing refused, and there are at least six.
#[test]
fn examples_draw_cleanly() {
    let ex = examples();
    assert!(ex.len() >= 6);
    for e in &ex {
        let p = with(&e.params);
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
        assert!(label_faults(&render(&p)).is_empty(), "{}: {:?}", e.name, label_faults(&render(&p)));
    }
}

/// The ends of every range: 100 strips, the largest window, every option on at Large, still quick and bounded.
#[test]
fn ends_of_the_ranges() {
    let mut p = lined(&[("n", i(MAX_N)), ("x_marks", bo(true)), ("sample_pts", bo(true)), ("strip_labels", s("Heights")), ("key_line", bo(true)), ("label_size", s("Large"))]);
    for rule in RULES {
        p.set_s("rule", rule);
        let t = std::time::Instant::now();
        let (svg, notes) = build(&p);
        assert!(t.elapsed().as_secs_f64() < 5.0, "{rule}: {:?}", t.elapsed());
        assert_eq!(svg.matches(r#"data-rs="strip""#).count(), if rule == "Trapezoid" { 100 } else { 100 });
        assert!(notes.iter().any(|n| n.contains("strip labels do not fit")), "{notes:?}");
        assert!(label_faults(&svg).is_empty(), "{:?}", label_faults(&svg));
        assert!(svg.len() < 2_000_000);
    }
    let mut h = lined(&[("region", s("Between f(x) and g(x)")), ("f", s("x")), ("g", s("x^2")), ("bounds", s("Where f and g cross")), ("strips", s("Horizontal")), ("n", i(MAX_N))]);
    for rule in ["Upper", "Lower", "Midpoint"] {
        h.set_s("rule", rule);
        let t = std::time::Instant::now();
        let _ = build(&h);
        assert!(t.elapsed().as_secs_f64() < 1.0, "horizontal {rule}: {:?}", t.elapsed());
    }
    let big = with(&[("xmin", fl(-1000.0)), ("xmax", fl(1000.0)), ("xstep", fl(0.05)), ("a", s("-999")), ("b", s("999")), ("n", i(MAX_N))]);
    assert!(render(&big).contains("<svg"));
}

/// A pole or other unbounded point anywhere in [a, b], also between the 2,001 samples, is refused with a note and never
/// summed; bounded functions with sharp peaks, kinks or fast wiggles are not refused. Decision: an integrable singularity
/// (ln|x| at 0, 1/√|x − 2/5|) is refused like a pole, since a Riemann sum samples f and f has no value there.
#[test]
fn poles_and_unbounded_points_are_refused() {
    let lined_with = |f: &str, a: &str, b: &str| lined(&[("f", s(f)), ("a", s(a)), ("b", s(b)), ("rule", s("Midpoint")), ("xmin", fl(-6.0)), ("xmax", fl(6.0)), ("ymin", fl(-10.0)), ("ymax", fl(10.0)), ("ystep", fl(2.0))]);
    let unbounded = [
        ("1/x^2", "-2", "5"), ("1/x^2", "-3", "2.1"), ("1/(x - sqrt(2))^2", "0", "3"), ("1/(x - pi/3)^2", "0", "2"), ("1/(x - e)^2", "0", "4"),
        ("1/(x - 0.3)^2", "0", "1.1"), ("1/(x - 1/7)^4", "0", "1"), ("1/(x - 2.71)^2", "-1", "5"), ("1/(x - sqrt(3))", "0", "3"), ("1/x", "-1.3", "2.9"),
        ("(x + 1)/(x - 0.77)", "0", "2"), ("1/(x - pi/5)^3", "0", "2"), ("tan(x)", "0", "2"), ("tan(x)", "1", "5"), ("sec(x)^2", "0", "2"),
        ("1/sin(x)", "-1", "1.3"), ("1/cos(x)^2", "1", "2"), ("1/sin(x)^2", "2", "4"), ("ln(abs(x))", "-1", "2"), ("ln(abs(x - sqrt(2)))", "0", "3"),
        ("ln((x - 0.3)^2)", "0", "1"), ("1/sqrt(abs(x - 0.4))", "0", "1"), ("1/(x^2 - 2)", "0", "3"),
    ];
    for (f, a, b) in unbounded {
        let p = lined_with(f, a, b);
        let n = warnings(&p);
        assert!(n.iter().any(|x| x.contains("no value at x")), "{f} on [{a}, {b}] is not refused: {n:?}");
        assert!(!render(&p).contains(r#"data-rs="line""#), "{f} on [{a}, {b}]: nothing is written for it");
    }
    let bounded = [("sin(50x)", "0", "3"), ("e^(-100x^2)", "-1", "1"), ("1/(1 + 100x^2)", "-2", "2"), ("sqrt(x)", "0", "4"), ("abs(x - 1)", "0", "3"),
        ("x^3", "-2", "2"), ("1/(x^2 + 0.01)", "-1", "1"), ("1 - abs(x)^(1/3)", "-1", "1"), ("ln(x)", "1", "5"), ("tan(x)", "-1.5", "1.5")];
    for (f, a, b) in bounded {
        let n = warnings(&lined_with(f, a, b));
        assert!(!n.iter().any(|x| x.contains("no value")), "{f} on [{a}, {b}] is refused: {n:?}");
    }
}

/// An integral with a kink is worked to the places shown and exactly where it can be: ∫₀³ |x − 1| dx = 1/2 + 2 = 5/2.
#[test]
fn kinks_and_written_places() {
    let p = with(&[("f", s("abs(x - 1)")), ("b", s("3")), ("integral_line", bo(true)), ("exact_line", bo(true)), ("xmax", fl(4.0))]);
    has_line(&p, "\u{222b}03 |x \u{2212} 1| dx = 5/2");
    let mut q = p.clone(); q.set_s("values", "Decimals"); q.set_i("dec", 6);
    has_line(&q, "dx = 2.5");
    // a value known only roughly is never written to more places than it is known, and an inexact value keeps its places
    let mut e = with(&[("f", s("e^x")), ("b", s("1")), ("strip_labels", s("Heights")), ("sum_line", bo(true))]);
    has_line(&e, "= (1/4)(1 + 1.2840 + 1.6487 + 2.1170) \u{2248} 1.5124");
    e.set_s("f", "e^(-x)");
    // (1, e^(−1/4), e^(−1/2), e^(−3/4): all rounded alike, never 1 + 0.7788 + …)
    assert!(lines(&e).iter().any(|l| l.contains("(1 + 0.7788 + 0.6065 + 0.4724)")), "{:?}", lines(&e));
    // book typesetting: √x, |x − 1|, no double brackets, a power in a power
    has_line(&with(&[("f", s("sqrt(x)")), ("b", s("4")), ("integral_line", bo(true))]), "\u{222b}04 \u{221a}x dx");
    has_line(&with(&[("f", s("1/(1 + x^2)")), ("b", s("1")), ("integral_line", bo(true))]), "\u{222b}01 1/(1 + x2) dx");
    has_line(&with(&[("f", s("e^(-x^2)")), ("b", s("1")), ("integral_line", bo(true))]), "e\u{2212}x\u{b2} dx");
    // b = e is read
    has_line(&with(&[("f", s("1/x")), ("a", s("1")), ("b", s("e")), ("integral_line", bo(true)), ("exact_line", bo(true))]), "\u{222b}1e 1/x dx = 1");
}

/// Without signed area, the value written under a region partly below the axis is said to be the integral, and the
/// area shaded is written too: x³ − 4x on [−2, 3] has ∫ = 25/4 (4 − 4 + 25/4) and area 4 + 4 + 25/4 = 57/4.
#[test]
fn integral_and_shaded_area_are_told_apart() {
    let p = with(&[("f", s("x^3 - 4x")), ("a", s("-2")), ("b", s("3")), ("strips", s("None (shade the exact area)")), ("exact_line", bo(true)), ("xmin", fl(-3.0)), ("xmax", fl(4.0)), ("ymin", fl(-5.0)), ("ymax", fl(16.0)), ("ystep", fl(2.0))]);
    has_line(&p, "Exact value = 25/4");
    has_line(&p, "Area shaded = 57/4");
    let mut q = p.clone(); q.set_b("signed", true);
    has_line(&q, "= 4 + 25/4 \u{2212} 4 = 25/4".replace("4 + 25/4", "41/4").as_str());
}

/// The window: an interval, a table or strips running past it are said, with the axis value to type.
#[test]
fn past_the_window_is_said() {
    let n = warnings(&with(&[("b", s("5"))]));
    assert!(n.iter().any(|x| x.contains("raise the x-axis To to at least 5")), "{n:?}");
    let n = warnings(&with(&[("region", s("From a table of values"))]));
    assert!(n.iter().any(|x| x.contains("table's x values run past the right of the window: raise the x-axis To to at least 7")), "{n:?}");
    let n = warnings(&with(&[("f", s("x^3")), ("b", s("2"))]));
    assert!(n.iter().any(|x| x.contains("raise the y-axis To to at least")) || n.is_empty(), "{n:?}");
    let n = warnings(&with(&[("f", s("x^3")), ("b", s("2")), ("rule", s("Right"))]));
    assert!(n.iter().any(|x| x.contains("The strips run above the window: raise the y-axis To to at least 8")), "{n:?}");
    assert!(warnings(&with(&[])).is_empty());
}

/// Student copies leak no answer: bounds found from crossings (and a horizontal slice's c, d) are letters; strip areas
/// are blanks; the title and description carry no value.
#[test]
fn student_copies_leak_no_answer() {
    let cross = with(&[("show_answers", bo(false)), ("region", s("Between f(x) and g(x)")), ("f", s("x + 2")), ("g", s("x^2")), ("bounds", s("Where f and g cross")), ("sum_line", bo(true)), ("integral_line", bo(true)), ("exact_line", bo(true)), ("show_dx", bo(true)), ("error_line", bo(true)), ("xmin", fl(-2.0))]);
    let svg = crate::figures::registry().into_iter().find(|d| d.id == ID).unwrap().svg(&cross);
    // (they cross at x = −1 and x = 2; the area is 9/2, the left sum 135/32)
    for l in texts(&svg, "line") { for ans in ["\u{2212}1", "= 2", "9/2", "135/32", "3/4"] { assert!(!l.contains(ans), "{l} gives {ans}"); } }
    assert!(texts(&svg, "line").iter().any(|l| l.contains("\u{222b}ab (x + 2 \u{2212} x2) dx =")));
    assert_eq!(texts(&svg, "bound"), vec!["a", "b"]);
    let h = with(&[("show_answers", bo(false)), ("region", s("Between f(x) and g(x)")), ("f", s("sqrt(x)")), ("g", s("x/2")), ("b", s("4")), ("strips", s("Horizontal")), ("integral_line", bo(true)), ("xmax", fl(5.0))]);
    assert!(lines(&h).iter().any(|l| l.contains("\u{222b}cd (xR \u{2212} xL) dy")), "{:?}", lines(&h));
    let ar = with(&[("show_answers", bo(false)), ("strip_labels", s("Areas"))]);
    assert!(texts(&render(&ar), "strip-label").is_empty());
    assert_eq!(render(&ar).matches(r#"data-rs="blank""#).count(), 4);
}


/// When the strip labels (or x₀ … xₙ) are left off, the note says what helps, and doing it makes them fit.
#[test]
fn refusal_advice_works() {
    let cases = [
        with(&[("f", s("x^3")), ("a", s("1")), ("b", s("2")), ("n", i(10)), ("rule", s("Trapezoid")), ("xmin", fl(-1.0)), ("xmax", fl(3.0)), ("ymax", fl(9.0)), ("strip_labels", s("Heights"))]),
        with(&[("f", s("x^3")), ("a", s("1")), ("b", s("2")), ("n", i(8)), ("rule", s("Midpoint")), ("xmin", fl(-1.0)), ("xmax", fl(3.0)), ("ymax", fl(9.0)), ("x_marks", bo(true))]),
        with(&[("f", s("x^3")), ("a", s("1")), ("b", s("2")), ("n", i(10)), ("xmin", fl(-1.0)), ("xmax", fl(3.0)), ("ymax", fl(9.0)), ("strip_labels", s("Areas"))]),
        with(&[("f", s("x^3")), ("a", s("1")), ("b", s("2")), ("n", i(10)), ("xmin", fl(-1.0)), ("xmax", fl(3.0)), ("ymax", fl(9.0)), ("x_marks", bo(true))]),
        with(&[("f", s("x^2")), ("a", s("0")), ("b", s("1")), ("n", i(8)), ("xmin", fl(-4.0)), ("xmax", fl(4.0)), ("ymin", fl(-1.0)), ("ymax", fl(2.0)), ("ystep", fl(0.5)), ("strip_labels", s("Areas"))]),
    ];
    let mut tried = 0;
    for p in cases {
        let n = warnings(&p);
        let Some(note) = n.iter().find(|x| x.contains("have no room") || x.contains("do not fit")) else { continue };
        tried += 1;
        assert!(!note.contains("wider"), "never advise widening: {note}");
        let k = note.find("(From ").unwrap_or_else(|| panic!("{note}"));
        let v: Vec<f64> = note[k + 6..].split([' ', ')', ',']).filter_map(|t| t.parse().ok()).collect();
        let mut q = p.clone();
        q.set_f("xmin", v[0]);
        q.set_f("xmax", v[1]);
        let after = warnings(&q);
        assert!(!after.iter().any(|x| x.contains("have no room") || x.contains("do not fit")), "{note} -> {after:?}");
    }
    assert!(tried >= 3, "the cases must be refused before the advice is taken ({tried})");
}

/// An odd root of a negative base is read by the figure: ∫₋₁¹ x^(1/3) dx = 0, ∫₋₁¹ x^(2/3) dx = 2·(3/5) = 6/5,
/// ∫₋₁¹ x^(1/5) dx = 0, ∫₀² (x − 1)^(1/3) dx = 0, ∫₋₈¹ x^(1/3) dx = (3/4)(1 − 16) = −45/4; L₂ of x^(1/3) on [−8, 0]:
/// Δx = 4, heights −2 and −∛4 = −1.5874: 4(−3.5874) = −14.3496. The coordinate grid itself is unchanged.
#[test]
fn odd_roots_of_negative_bases() {
    let ex = |f: &str, a: &str, b: &str| with(&[("f", s(f)), ("a", s(a)), ("b", s(b)), ("exact_line", bo(true)), ("xmin", fl(-9.0)), ("xmax", fl(3.0)), ("ymin", fl(-3.0)), ("ymax", fl(3.0))]);
    has_line(&ex("x^(1/3)", "-1", "1"), "Exact value = 0");
    has_line(&ex("x^(2/3)", "-1", "1"), "Exact value = 6/5");
    has_line(&ex("x^(1/5)", "-1", "1"), "Exact value = 0");
    has_line(&ex("(x - 1)^(1/3)", "0", "2"), "Exact value = 0");
    has_line(&ex("x^(1/3)", "-8", "1"), "Exact value = \u{2212}45/4");
    let mut l = ex("x^(1/3)", "-8", "0"); l.set_i("n", 2); l.set_b("sum_line", true);
    has_line(&l, "= 4\u{b7}(\u{2212}2 \u{2212} 1.5874) \u{2248} \u{2212}14.3496");
    assert!(warnings(&ex("x^(1/3)", "-1", "1")).is_empty());
    // drawn by the figure on both sides of 0
    let svg = render(&ex("x^(1/3)", "-1", "1"));
    assert!(svg.contains(r#"data-rs="curve""#));
    // an even root of a negative base still has no value
    assert!(warnings(&ex("x^(1/2)", "-1", "1")).iter().any(|n| n.contains("no value")));
    // the grid draws as before: its reader gives nothing for (−1)^(1/3)
    assert!(super::super::eval("x^(1/3)", -1.0).is_none_or(|v| !v.is_finite()));
}

/// Large values are values; typed decimals stay decimals; an end with no value is said as an end; √|x|.
#[test]
fn values_large_typed_and_ends() {
    let big = with(&[("f", s("x^10")), ("a", s("0")), ("b", s("4")), ("xmax", fl(5.0)), ("ymax", fl(10.0)), ("sum_line", bo(true))]);
    assert!(!warnings(&big).iter().any(|n| n.contains("no value")), "{:?}", warnings(&big));
    // L₄ of x¹⁰ on [0, 4]: 1·(0 + 1 + 1024 + 59049) = 60074
    has_line(&big, "= 1\u{b7}(0 + 1 + 1024 + 59049) = 60074");
    assert!(!warnings(&with(&[("f", s("e^x")), ("b", s("14")), ("xmax", fl(15.0))])).iter().any(|n| n.contains("no value")));
    assert!(warnings(&with(&[("f", s("e^x")), ("b", s("40")), ("xmax", fl(41.0))])).iter().any(|n| n.contains("too large")));
    let d = with(&[("a", s("-1.5")), ("b", s("0.5")), ("xmin", fl(-2.0)), ("xmax", fl(1.0)), ("show_dx", bo(true))]);
    assert_eq!(texts(&render(&d), "bound"), vec!["a = \u{2212}1.5", "b = 0.5"]);
    has_line(&d, "(0.5 \u{2212} (\u{2212}1.5))/4");
    let t = with(&[("region", s("From a table of values")), ("table", s("0, 2.5; 1, 5; 2.5, 6")), ("strip_labels", s("Heights")), ("xmax", fl(3.0))]);
    assert!(texts(&render(&t), "strip-label").contains(&"2.5".to_string()), "{:?}", texts(&render(&t), "strip-label"));
    assert_eq!(texts(&render(&t), "bound"), vec!["a = 0", "b = 2.5"]);
    let e = warnings(&with(&[("f", s("1/sqrt(x)")), ("a", s("0")), ("b", s("1"))]));
    assert!(e.iter().any(|n| n.contains("the end a") && n.contains("move a just inside")), "{e:?}");
    has_line(&with(&[("f", s("sqrt(abs(x))")), ("a", s("-1")), ("b", s("1")), ("integral_line", bo(true))]), "\u{221a}|x| dx");
}
