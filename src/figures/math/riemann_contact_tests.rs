//! Check 15: wide flat contacts (f − g flat to rounding over a stretch: sin x against its Taylor
//! polynomials, x⁸ + 1 against 1 on a narrow window) are one meet settled exactly, or refused honestly;
//! never listed sample by sample. Scientific notation (2e-1, which reads like Euler's e) is refused in
//! every typed field. The truth for every case is worked by hand in the comments.
use super::*;
use super::tests::{s, with};

fn p(f: &str, g: &str, x0: f64, x1: f64) -> Params {
    with(&[("region", s("Between f(x) and g(x)")), ("f", s(f)), ("g", s(g)), ("bounds", s("Where f and g cross")), ("xmin", Value::F(x0)), ("xmax", Value::F(x1)),
        ("ymin", Value::F(-2.)), ("ymax", Value::F(3.)), ("strips", s("Vertical")), ("rule", s("Left")), ("n", Value::I(4)),
        ("sum_line", Value::B(true)), ("integral_line", Value::B(true)), ("exact_line", Value::B(true)), ("error_line", Value::B(true)), ("show_dx", Value::B(true)), ("key_line", Value::B(true))])
}
fn line_texts(svg: &str) -> Vec<String> {
    let mut out = vec![];
    let mut rest = svg;
    while let Some(k) = rest.find("<text data-rs=\"line\"") {
        rest = &rest[k..];
        let end = rest.find("</text>").unwrap();
        let mut t = String::new(); let mut tag = false;
        for c in rest[rest.find('>').unwrap() + 1..end].chars() { match c { '<' => tag = true, '>' => tag = false, _ if !tag => t.push(c), _ => {} } }
        out.push(t);
        rest = &rest[end..];
    }
    out
}
const REFUSED: [&str; 2] = ["agree to within rounding", "too flatly"];

/// What a case must give: one touch at r, one crossing, or exactly these meets. `strict`: the honest
/// "cannot be listed" refusal is not enough (f − g is a polynomial with a rational root: settled exactly).
enum Want { Touch(f64, bool), Cross(bool), Meets(Vec<f64>) }

fn wide_contacts() -> Vec<(&'static str, &'static str, f64, f64, Want)> {
    use Want::*;
    let t5 = "x-x^3/6+x^5/120";
    let t7 = "x-x^3/6+x^5/120-x^7/5040";
    let t9 = "x-x^3/6+x^5/120-x^7/5040+x^9/362880";
    let c4 = "1-x^2/2+x^4/24";
    let c6 = "1-x^2/2+x^4/24-x^6/720";
    let e5 = "1+x+x^2/2+x^3/6+x^4/24+x^5/120";
    let e7 = "1+x+x^2/2+x^3/6+x^4/24+x^5/120+x^6/720+x^7/5040";
    let mut v = vec![];
    // sin x − T₂ₖ₊₁(x) = ±x^(2k+3)/(2k+3)! (1 − x²/((2k+4)(2k+5)) + …): one sign change, at 0, for |x| ≤ 4
    for (x0, x1) in [(0., 1.), (-0.5, 0.5), (-0.25, 0.25), (-1., 1.)] { v.push(("sin(x)", t5, x0, x1, Cross(false))); }
    for (x0, x1) in [(-2., 2.), (-1., 1.), (-0.5, 0.5), (0., 1.), (-0.25, 0.25)] { v.push(("sin(x)", t7, x0, x1, Cross(false))); }
    for (x0, x1) in [(-1., 1.), (-2., 2.), (-4., 4.)] { v.push(("sin(x)", t9, x0, x1, Cross(false))); }
    // cos x − T₆ = −x⁸/8! (1 − …) ≤ 0, cos x − T₄ = −x⁶/6! (…) ≤ 0, eˣ − Tₙ = e^ξ x^(n+1)/(n+1)! ≥ 0 for odd n: touches at 0
    for (x0, x1) in [(-1., 1.), (-2.5, 3.5)] { v.push(("cos(x)", c6, x0, x1, Touch(0., false))); }
    for (x0, x1) in [(-1., 1.), (-2., 2.)] { v.push(("e^x", e7, x0, x1, Touch(0., false))); }
    v.push(("cos(x)", c4, -0.25, 0.25, Touch(0., false)));
    v.push(("e^x", e5, -0.25, 0.25, Touch(0., false)));
    // polynomials: f − g = x⁸, x⁷, x¹⁰ … with the one rational root 0 (or 1, or 1/2): settled exactly
    v.push(("x^8+1", "1", -0.5, 0.5, Touch(0., true)));
    v.push(("x^7+1", "1", -0.25, 0.25, Cross(true)));
    for (x0, x1) in [(0., 1.), (-0.5, 0.5)] { v.push(("x^10", "0", x0, x1, Touch(0., true))); }
    v.push(("x^8", "0", -0.25, 0.25, Touch(0., true)));
    for (f, g) in [("(x-1)^10", "0"), ("(x-1)^10+1", "1"), ("(x-1)^10+2", "2"), ("(x-1)^10+3", "3"), ("(x-1)^10+1/2", "1/2"), ("(x-1)^10-1", "-1"), ("(x-1)^10+5", "5"), ("(x-1)^10+1/4", "1/4")] {
        v.push((f, g, -1., 1., Touch(1., true)));
    }
    for (f, g) in [("(x-1/2)^10/3", "0"), ("(x-1/2)^10/3+1", "1"), ("(x-1/2)^10/3+2", "2")] { v.push((f, g, -1., 1., Touch(0.5, true))); }
    // x⁸ − x⁶ = x⁶(x − 1)(x + 1): −1, 0 (a touch of order 6) and 1, listed exactly
    v.push(("x^8", "x^6", -2., 2., Meets(vec![-1., 0., 1.])));
    // (x − 1)(x − 1.001): two crossings 0.001 apart, never merged
    v.push(("(x-1)(x-1.001)", "0", 0., 2., Meets(vec![1., 1.001])));
    v
}

#[test]
fn riemann_wide_contacts_settle_or_refuse() {
    let cases = wide_contacts();
    assert!(cases.len() >= 30, "{}", cases.len());
    let mut bad = vec![];
    for (f, g, x0, x1, want) in &cases {
        let ctx = format!("{f} vs {g} on [{x0}, {x1}]");
        let t0 = std::time::Instant::now();
        let got = model(&p(f, g, *x0, *x1));
        let el = t0.elapsed();
        if el.as_secs_f64() > 0.5 { bad.push(format!("{ctx}: {el:?}")); }
        let refused = |e: &str| REFUSED.iter().any(|r| e.contains(r));
        match (want, got) {
            (Want::Touch(r, strict), Err(e)) => {
                let ok = e.contains(&format!("only touch at x \u{2248} {},", rounded(*r, 4))) || (!strict && refused(&e));
                if !ok { bad.push(format!("{ctx}: {e}")); }
            }
            (Want::Cross(strict), Err(e)) => { if !(e.contains("cross only once") || (!strict && refused(&e))) { bad.push(format!("{ctx}: {e}")); } }
            (Want::Meets(xs), Ok(m)) => {
                let got: Vec<f64> = m.crossings.iter().map(|c| c.v).collect();
                if got.len() != xs.len() || got.iter().zip(xs).any(|(a, b)| (a - b).abs() > 1e-9) { bad.push(format!("{ctx}: {got:?}")); }
            }
            (_, Ok(m)) => bad.push(format!("{ctx}: a region was built on {:?}", m.crossings.iter().map(|c| c.v).collect::<Vec<_>>())),
            (_, Err(e)) => bad.push(format!("{ctx}: refused: {e}")),
        }
    }
    assert!(bad.is_empty(), "{} wrong:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn riemann_near_misses_and_close_meets() {
    // x⁴ + 10⁻⁵ > 0, x² + 10⁻⁵ > 0, sin x < 1.00001: no meet at all
    for (f, g, x0, x1) in [("x^4+1/100000", "0", -2., 2.), ("x^2+1/100000", "0", -2., 2.), ("sin(x)", "1.00001", 0., 3.)] {
        let e = model(&p(f, g, x0, x1)).err().unwrap_or_default();
        assert!(e.contains("do not meet"), "{f} vs {g}: {e}");
    }
}

/// Every spelling of scientific notation, in every typed field: refused with a note naming it, and nothing
/// written. Spellings that are not scientific notation (e follows no digit, or no digit follows it) are read.
#[test]
fn riemann_scientific_notation_is_refused_in_every_field() {
    let spellings = ["2e-1", "1e-6", "2E5", "3e+2", "2.5e-1", "25e-2", "1e3", "5e0", "7e-3", "4e-01", "6E-2", "1.0e1", "9e9", ".5e1", "2e-2", "3E+1", "1e-1", "8e2", "1.5E-3", "2e10"];
    let base = |set: &[(&str, Value)]| { let mut v = vec![("region", s("Between f(x) and g(x)")), ("f", s("x^2")), ("g", s("x")), ("bounds", s("From a to b")), ("a", s("0")), ("b", s("1")), ("sum_line", Value::B(true)), ("integral_line", Value::B(true)), ("exact_line", Value::B(true)), ("key_line", Value::B(true))]; v.extend(set.iter().cloned()); with(&v) };
    let mut checked = 0;
    for sp in spellings {
        let fx = format!("x^2+{sp}");
        let table = format!("0, {sp}; 1, 2; 2, 3");
        for (field, params) in [
            ("f", base(&[("f", s(&fx))])), ("g", base(&[("g", s(sp))])), ("a", base(&[("a", s(sp)), ("b", s("100000000000"))])), ("b", base(&[("b", s(sp))])),
            ("under f", base(&[("region", s("Under f(x)")), ("f", s(sp))])), ("table", base(&[("region", s("From a table of values")), ("table", s(&table))])),
        ] {
            let notes = warnings(&params);
            assert!(notes.iter().any(|n| n.contains("scientific notation") && n.contains(sp)), "{field} = {sp}: {notes:?}");
            assert!(line_texts(&render(&params)).is_empty(), "{field} = {sp}: something was written");
            checked += 1;
        }
    }
    assert_eq!(checked, 120);
    // not scientific notation: Euler's e, read as it is
    for f in ["2*e-1+x", "2 e-1+x", "e-1+x", "2e^x", "2e-x", "x^2+2e", "exp(1)x"] {
        let notes = warnings(&base(&[("f", s(f))]));
        assert!(!notes.iter().any(|n| n.contains("scientific notation")), "{f}: {notes:?}");
    }
}

/// For the mpmath sweep (scripts outside the repo): with RIEMANN_R16_PAIRS naming a file of
/// "f|g|x0|x1" lines, writes each pair's meets or note to RIEMANN_R16_DUMP as JSON lines.
#[test]
fn riemann_meet_dump_for_sweep() {
    let (Ok(pairs), Ok(out)) = (std::env::var("RIEMANN_R16_PAIRS"), std::env::var("RIEMANN_R16_DUMP")) else { return };
    let mut lines = vec![];
    for row in std::fs::read_to_string(pairs).unwrap().lines().filter(|l| !l.trim().is_empty()) {
        let parts: Vec<&str> = row.split('|').collect();
        let (x0, x1) = (parts[2].parse::<f64>().unwrap(), parts[3].parse::<f64>().unwrap());
        let t0 = std::time::Instant::now();
        let r = model(&p(parts[0], parts[1], x0, x1));
        let ms = t0.elapsed().as_secs_f64() * 1000.;
        let j = match r {
            Ok(m) => serde_json::json!({"f": parts[0], "g": parts[1], "x0": x0, "x1": x1, "ok": true, "meets": m.crossings.iter().map(|c| c.v).collect::<Vec<_>>(), "open": m.meet_open, "ms": ms}),
            Err(e) => serde_json::json!({"f": parts[0], "g": parts[1], "x0": x0, "x1": x1, "ok": false, "note": e, "ms": ms}),
        };
        lines.push(j.to_string());
    }
    std::fs::write(out, lines.join("\n") + "\n").unwrap();
}

#[test]
fn riemann_check15_evidence() {
    let Ok(dir) = std::env::var("RIEMANN_R16_EVIDENCE_DIR") else { return };
    std::fs::create_dir_all(&dir).unwrap();
    let all = serde_json::json!({"sum_line":true,"integral_line":true,"exact_line":true,"error_line":true,"show_dx":true,"key_line":true,"strip_labels":"Heights"});
    for (name, q) in [
        ("taylor_sin_t5", serde_json::json!({"region":"Between f(x) and g(x)","bounds":"Where f and g cross","f":"sin(x)","g":"x-x^3/6+x^5/120","xmin":0,"xmax":1,"ymin":-1,"ymax":2,"strips":"Vertical","rule":"Left","n":4})),
        ("sci_a_2e-1", serde_json::json!({"region":"Under f(x)","bounds":"From a to b","a":"2e-1","b":"1","f":"x^2","xmin":-1,"xmax":2,"ymin":-1,"ymax":2,"strips":"Vertical","rule":"Left","n":4})),
        ("print_x8_x6", serde_json::json!({"region":"Between f(x) and g(x)","bounds":"Where f and g cross","f":"x^8","g":"x^6","xmin":-2,"xmax":2,"ymin":-0.5,"ymax":1.5,"strips":"Vertical","rule":"Midpoint","n":4,"style":"Print"})),
        ("coincident_b0_2", serde_json::json!({"region":"Between f(x) and g(x)","bounds":"Where f and g cross","f":"abs(x-1)+abs(x+1)","g":"2","xmin":-4,"xmax":4,"ymin":-1,"ymax":5,"strips":"Vertical","rule":"Trapezoid","n":100})),
        ("student_x8_x6", serde_json::json!({"region":"Between f(x) and g(x)","bounds":"Where f and g cross","f":"x^8","g":"x^6","xmin":-2,"xmax":2,"ymin":-0.5,"ymax":1.5,"strips":"Vertical","rule":"Midpoint","n":4,"show_answers":false})),
    ] {
        let mut q = q.clone();
        for (k, v) in all.as_object().unwrap() { q[k] = v.clone(); }
        let t = std::time::Instant::now();
        let (_, bytes) = crate::figure_agent::render_png(ID, &q).unwrap();
        eprintln!("{name}: {:?}", t.elapsed());
        std::fs::write(format!("{dir}/{name}.png"), bytes).unwrap();
    }
}
