//! The table of hand-worked functions: for each, every left limit, right limit, limit and f(a) asked, the limits at ±∞,
//! each discontinuity in the window with its kind, and the asymptotes, all worked by hand (the working beside the row
//! where it is not plain), asserted against the figure's model and against the key it draws. It includes every function
//! the first independent check worked (~/checker-out/limits-r1).
use super::*;
use crate::figures::{registry, FigureDef, Params};
use serde_json::{json, Value as J};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
fn with(kv: J) -> Params { crate::figures::params_from_json(&def(), &kv) }
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
/// A limit as the key writes it (∞ and -∞; does not exist; undefined; ? where the app cannot determine it).
fn lim_key(l: &Lim) -> String { match l { Lim::Val(_, t) => t.clone(), Lim::PInf => "∞".into(), Lim::NInf => "-∞".into(), Lim::Dne | Lim::Undef => "DNE".into(), Lim::Unknown => "?".into(), Lim::Two => "two".into() } }
fn val_key(l: &Lim) -> String { match l { Lim::Undef => "undef".into(), Lim::Dne => "DNE".into(), o => lim_key(o) } }

/// (name, pieces, single points, values of a, per a "left, right, limit, f(a)" joined by " | ", "limit at ∞, at -∞" (none:
/// not asked, as no piece has values that far; empty: not checked), the discontinuities "x kind" in the window, the
/// vertical asymptotes, the horizontal asymptotes, more params).
type Row = (&'static str, &'static str, &'static str, &'static str, &'static str, &'static str, &'static str, &'static str, &'static str, J);
fn rows() -> Vec<Row> {
    let w = |x0: f64, x1: f64, y0: f64, y1: f64| json!({"xmin": x0, "xmax": x1, "ymin": y0, "ymax": y1});
    let n = || json!({});
    vec![
        // ── holes from 0/0, written exactly ──
        ("M01", "y = (x^2 - 4)/(x - 2)", "", "2", "4, 4, 4, undef", "∞, -∞", "2 removable", "", "", n()),
        ("M02", "y = (3x - 1)(x + 1)/(3x - 1)", "", "1/3", "4/3, 4/3, 4/3, undef", "∞, -∞", "1/3 removable", "", "", n()),
        ("M02b", "y = (3x - 1)(x + 1)/(3x - 1)", "", "", "", "", "1/3 removable", "", "", n()),
        // x^2 - 2 = (x - √2)(x + √2): 2√2 at √2
        ("M03", "y = (x^2 - 2)/(x - sqrt(2))", "", "sqrt(2)", "2√2, 2√2, 2√2, undef", "∞, -∞", "√2 removable", "", "", n()),
        ("M03b", "y = (x^2 - 2)/(x - sqrt(2))", "", "", "", "", "√2 removable", "", "", n()),
        ("M04", "y = (x - 0.37)x/(x - 0.37)", "", "0.37", "0.37, 0.37, 0.37, undef", "∞, -∞", "0.37 removable", "", "", n()),
        ("M04b", "y = (x - 0.37)x/(x - 0.37)", "", "", "", "", "0.37 removable", "", "", n()),
        // sin(π + t) = -sin t: -1 at π; sin x/(x - π) → 0 at ±∞
        ("M05", "y = sin(x)/(x - pi)", "", "pi", "-1, -1, -1, undef", "0, 0", "π removable", "", "0", n()),
        ("M06", "y = x, x != pi", "", "pi", "π, π, π, undef", "∞, -∞", "π removable", "", "", n()),
        ("M07", "y = 2x + 1, x ≠ 1/3", "", "", "", "", "1/3 removable", "", "", n()),
        ("M08", "y = 1/(x - sqrt(3))", "", "sqrt(3)", "-∞, ∞, DNE, undef", "0, 0", "√3 infinite", "√3", "0", n()),
        ("M09", "y = 1/(x - 0.37)^2", "", "0.37", "∞, ∞, ∞, undef", "0, 0", "0.37 infinite", "0.37", "0", n()),
        ("M10", "y = 1/(x - pi/3)^2", "", "", "", "0, 0", "π/3 infinite", "π/3", "0", n()),
        ("M11", "y = 1/(3x + 1)^3", "", "-1/3", "-∞, ∞, DNE, undef", "0, 0", "-1/3 infinite", "-1/3", "0", n()),
        // the first round's "jump": x + 2 and 4 - x meet at 3 at x = 1, so it is continuous there
        ("M12", "y = x + 2, x < 1\ny = 4 - x, x >= 1", "", "1", "3, 3, 3, 3", "-∞, -∞", "", "", "", n()),
        ("M13", "y = x + 1, x < 2\ny = x - 1, x >= 2", "", "2", "3, 1, DNE, 1", "∞, -∞", "2 jump", "", "", n()),
        ("M14", "y = (x^2 - 1)/(x - 1)", "(1, 5)", "1", "2, 2, 2, 5", "∞, -∞", "1 removable", "", "", n()),
        ("M15", "y = sqrt(x)", "", "0", "DNE, 0, DNE, 0", "∞, none", "", "", "", n()),
        ("M16", "y = ln(x)", "", "0", "DNE, -∞, DNE, undef", "∞, none", "", "0", "", n()),
        ("M17", "y = sin(1/x)", "", "0", "DNE, DNE, DNE, undef", "0, 0", "0 oscillating", "", "0", n()),
        // x sin(1/x) = sin(t)/t with t = 1/x → 1 at ±∞
        ("M18", "y = x sin(1/x)", "", "0", "0, 0, 0, undef", "1, 1", "0 removable", "", "1", n()),
        ("M19", "y = sin(x)/x", "", "0", "1, 1, 1, undef", "0, 0", "0 removable", "", "0", n()),
        ("M20", "y = (x^2 - 1)/(10000000(x - 1))", "", "1", "0.0000002, 0.0000002, 0.0000002, undef", "∞, -∞", "1 removable", "", "", n()),
        ("M21", "y = 1000000(x^2 - 1)/(x - 1)", "", "1", "2000000, 2000000, 2000000, undef", "∞, -∞", "1 removable", "", "", n()),
        // ln(1 + t) ≈ t: -∞ from the left of 1, ∞ from the right; 1/ln x → 0 at ∞ (slowly); no values for x ≤ 0
        ("M22", "y = 1/ln(x)", "", "1", "-∞, ∞, DNE, undef", "0, none", "1 infinite", "1", "0", n()),
        ("M23", "y = x/(x + 1000)", "", "", "", "1, 1", "", "", "1", n()),
        ("M24", "y = atan(x)", "", "", "", "π/2, -π/2", "", "", "π/2, -π/2", n()),
        ("M24b", "y = arctan(x)", "", "", "", "π/2, -π/2", "", "", "π/2, -π/2", n()),
        // values only where 1 + 1/x > 0 (x > 0 or x < -1); left of -1 the base tends to 0⁺ and the power to -1: ∞
        ("M25", "y = (1 + 1/x)^x", "", "", "", "e, e", "", "-1", "e", n()),
        ("M26", "y = sin(x)", "", "", "", "DNE, DNE", "", "", "", n()),
        ("M26b", "y = cos(2pi x)", "", "", "", "DNE, DNE", "", "", "", n()),
        ("M26c", "y = sin(pi x)", "", "", "", "DNE, DNE", "", "", "", n()),
        ("M27", "y = (x^2 + 1)/x", "", "0", "-∞, ∞, DNE, undef", "∞, -∞", "0 infinite", "0", "", n()),
        ("M28", "y = x^2, x < 1\ny = 2x - 1, x >= 1", "", "1", "1, 1, 1, 1", "∞, ∞", "", "", "", n()),
        // a gap in the domain: the ends of it are not discontinuities
        ("M29", "y = x, x < 0\ny = x + 1, x > 2", "", "0; 1; 2", "0, DNE, DNE, undef | DNE, DNE, DNE, undef | DNE, 3, DNE, undef", "∞, -∞", "", "", "", n()),
        ("M31", "(-4, 0) (-2, 2) (0, 1) open\nclosed (0, 3) (2, 1) (4, 2)", "", "0; -2", "1, 3, DNE, 3 | 2, 2, 2, 2", "none, none", "0 jump", "", "", n()),
        ("M32", "(0, 1/3) (1, 2/3) open\nopen (1, 1) (2, 0)", "", "1", "2/3, 1, DNE, undef", "none, none", "1 jump", "", "", n()),
        // (x - 1)/((x - 1)(x + 1)) = 1/(x + 1)
        ("M33", "y = (x - 1)/(x^2 - 1)", "", "1; -1", "1/2, 1/2, 1/2, undef | -∞, ∞, DNE, undef", "0, 0", "-1 infinite, 1 removable", "-1", "0", n()),
        ("M34", "y = (x + 2)/(x - 3)^2", "", "3", "∞, ∞, ∞, undef", "0, 0", "3 infinite", "3", "0", n()),
        ("M35", "y = abs(x)/x", "", "0", "-1, 1, DNE, undef", "1, -1", "0 jump", "", "1, -1", n()),
        ("M36", "y = floor(x)", "", "2", "1, 2, DNE, 2", "∞, -∞", "-3 jump, -2 jump, -1 jump, 0 jump, 1 jump, 2 jump, 3 jump", "", "", w(-3.0, 3.0, -4.0, 4.0)),
        // the real cube root: -∛ of a negative number
        ("M37", "y = x^(1/3)", "", "0", "0, 0, 0, 0", "∞, -∞", "", "", "", n()),
        ("M39a", "y = (sqrt(2)x + 1)/(x + 3)", "", "", "", "√2, √2", "-3 infinite", "-3", "√2", n()),
        ("M39b", "y = (2x + 1)/(3x - 5)", "", "5/3", "-∞, ∞, DNE, undef", "2/3, 2/3", "5/3 infinite", "5/3", "2/3", n()),
        // √(x² + x) = |x|√(1 + 1/x): x + 1/2 + … at ∞ (so 1/2), -x - 1/2 at -∞ (so ∞); values for x ≤ -1 and x ≥ 0
        ("M39c", "y = sqrt(x^2 + x) - x", "", "", "", "1/2, ∞", "", "", "1/2", w(-6.0, 10.0, -3.0, 4.0)),
        // 2x² - 1 = 0 at ±√(1/2) = ±√2/2
        ("M39d", "y = (x^2 + 3x)/(2x^2 - 1)", "", "", "", "1/2, 1/2", "-√2/2 infinite, √2/2 infinite", "-√2/2, √2/2", "1/2", n()),
        ("M40", "y = exp(1)x/(x + 1)", "", "", "", "e, e", "-1 infinite", "-1", "e", n()),
        ("M40b", "y = (1.234567x + 2)/x", "", "", "", "1.234567, 1.234567", "0 infinite", "0", "1.234567", n()),
        ("M43", "y = 1/x", "(0, 2)", "0", "-∞, ∞, DNE, 2", "0, 0", "0 infinite", "0", "0", n()),
        ("M44", "y = tan(x)", "", "pi/2", "∞, -∞, DNE, undef", "DNE, DNE", "-3π/2 infinite, -π/2 infinite, π/2 infinite, 3π/2 infinite, 5π/2 infinite", "-3π/2, -π/2, π/2, 3π/2, 5π/2", "", n()),
        ("M45", "y = 1/(x^2 + 1)", "", "", "", "0, 0", "", "", "0", n()),
        ("M46", "y = exp(x)", "", "", "", "∞, 0", "", "", "0", n()),
        ("M47", "y = exp(-1/x^2)", "", "0", "0, 0, 0, undef", "1, 1", "0 removable", "", "1", n()),
        ("M48", "y = (1 - cos(x))/x^2", "", "0", "1/2, 1/2, 1/2, undef", "0, 0", "0 removable", "", "0", n()),
        ("M49", "y = sin(3x)/(5x)", "", "0", "3/5, 3/5, 3/5, undef", "0, 0", "0 removable", "", "0", n()),
        // (√(x + 4) - 2)/x = 1/(√(x + 4) + 2): 1/4 at 0; ~x^(-1/2) → 0 at ∞; no values for x < -4
        ("M50", "y = (sqrt(x + 4) - 2)/x", "", "0", "1/4, 1/4, 1/4, undef", "0, none", "0 removable", "", "0", n()),
        ("M51", "y = (x^3 - 8)/(x - 2)", "", "2", "12, 12, 12, undef", "∞, ∞", "2 removable", "", "", w(-6.0, 10.0, -6.0, 14.0)),
        ("M52", "y = (exp(x) - 1)/x", "", "0", "1, 1, 1, undef", "∞, 0", "0 removable", "", "0", n()),
        // x² sin(1/x) = sin(t)/t² ~ 1/t at ∞: ∞; at -∞ it is -∞
        ("M53", "y = x^2 sin(1/x)", "", "0", "0, 0, 0, undef", "∞, -∞", "0 removable", "", "", n()),
        ("M54", "y = 3/(x - 2) + 1, x > 2", "", "2", "DNE, ∞, DNE, undef", "1, none", "", "2", "1", n()),
        ("M55", "y = (x^2 - 9)/(x + 3), x <= 1\ny = 5, x > 1", "", "-3; 1", "-6, -6, -6, undef | -2, 5, DNE, -2", "5, -∞", "-3 removable, 1 jump", "", "5", n()),
        ("M56", "y = 1/(x - 2.5)", "", "2.5", "-∞, ∞, DNE, undef", "0, 0", "2.5 infinite", "2.5", "0", n()),
        // (x - 3)(x + 2)/((x - 3)(x + 3)) = (x + 2)/(x + 3): 5/6 at 3; at -3⁻ (-1)/(0⁻) = ∞, at -3⁺ -∞
        ("M57", "y = (x^2 - x - 6)/(x^2 - 9)", "", "3; -3", "5/6, 5/6, 5/6, undef | ∞, -∞, DNE, undef", "1, 1", "-3 infinite, 3 removable", "-3", "1", n()),
        ("M58", "y = sqrt(4 - x^2)", "", "2; -2", "0, DNE, DNE, 0 | DNE, 0, DNE, 0", "none, none", "", "", "", n()),
        ("M59", "y = 2^x", "", "", "", "∞, 0", "", "", "0", n()),
        // x - floor(x): 1 from the left of each whole number, 0 from the right
        ("M60", "y = x - floor(x)", "", "1", "1, 0, DNE, 0", "DNE, DNE", "-3 jump, -2 jump, -1 jump, 0 jump, 1 jump, 2 jump, 3 jump", "", "", w(-3.0, 3.0, -2.0, 2.0)),
        ("M61", "y = 1/(x - 0.001)", "", "0.001", "-∞, ∞, DNE, undef", "0, 0", "0.001 infinite", "0.001", "0", n()),
        ("M62", "y = (x^2 - 0.1369)/(x - 0.37)", "", "0.37", "0.74, 0.74, 0.74, undef", "∞, -∞", "0.37 removable", "", "", n()),
        ("M63", "y = (x - 1)^2/(x - 1)", "", "1", "0, 0, 0, undef", "∞, -∞", "1 removable", "", "", n()),
        ("M64", "y = 5, x != 2", "(2, 1)", "2", "5, 5, 5, 1", "5, 5", "2 removable", "", "5", n()),
        // (x + 2)(x - 1)/(x - 1) = x + 2: 3 at 1; at 3: 3 + 2 = 5... the first piece is x + 2 at 3⁻ = 5? no: (9 + 3 - 2)/2 = 5
        ("M65", "y = (x^2 + x - 2)/(x - 1), x < 3\ny = 1/(x - 4), x >= 3", "", "1; 3; 4", "3, 3, 3, undef | 5, -1, DNE, -1 | -∞, ∞, DNE, undef", "0, -∞", "1 removable, 3 jump, 4 infinite", "4", "0", n()),
        ("N01", "y = 1/(1/x)", "", "0", "0, 0, 0, undef", "∞, -∞", "0 removable", "", "", n()),
        ("N02", "y = atan(1/x)", "", "0", "-π/2, π/2, DNE, undef", "0, 0", "0 jump", "", "0", n()),
        ("N03", "y = exp(1/x)", "", "0", "0, ∞, DNE, undef", "1, 1", "0 infinite", "0", "1", n()),
        ("N04", "y = 2^(1/x)", "", "0", "0, ∞, DNE, undef", "1, 1", "0 infinite", "0", "1", n()),
        ("N05", "y = 1/(x - 0.3)", "", "", "", "0, 0", "0.3 infinite", "0.3", "0", n()),
        ("N06", "y = 1/(x - 1/3)", "", "", "", "0, 0", "1/3 infinite", "1/3", "0", n()),
        ("N07", "y = sqrt(1 - x^2)", "", "0; 1", "1, 1, 1, 1 | 0, DNE, DNE, 0", "none, none", "", "", "", n()),
        ("N08", "y = asin(x)", "", "1/2", "π/6, π/6, π/6, π/6", "none, none", "", "", "", n()),
        // ln(4 - x²) → -∞ at ±2 from inside
        ("N09", "y = ln(4 - x^2)", "", "2", "-∞, DNE, DNE, undef", "none, none", "", "-2, 2", "", n()),
        ("N10", "y = sqrt(x - 7)", "", "7", "DNE, 0, DNE, 0", "∞, none", "", "", "", n()),
        ("N11", "y = 1/ln(x)", "", "", "", "0, none", "1 infinite", "1", "0", n()),
        ("N12", "y = 1/sqrt(x)", "", "", "", "0, none", "", "0", "0", n()),
        ("N13", "y = ln(x)/x", "", "", "", "0, none", "", "0", "0", n()),
        ("N14", "y = x^(1/3)", "", "-1; 0; -8", "-1, -1, -1, -1 | 0, 0, 0, 0 | -2, -2, -2, -2", "∞, -∞", "", "", "", n()),
        ("N15", "y = (x^2 - 1)/(x - 1), x != 1", "", "1", "2, 2, 2, undef", "∞, -∞", "1 removable", "", "", n()),
        // √x sin(1/x) → 0 from the right (squeezed by ±√x); no values on the left
        ("N16", "y = sqrt(x) sin(1/x)", "", "0", "DNE, 0, DNE, undef", "0, none", "", "", "0", n()),
        ("N17", "y = 1/x^2", "", "0", "∞, ∞, ∞, undef", "0, 0", "0 infinite", "0", "0", n()),
        ("N18", "y = 1/(x - sqrt(2))^2", "", "", "", "0, 0", "√2 infinite", "√2", "0", n()),
        ("N19", "y = tan(x)", "", "", "", "DNE, DNE", "-π/2 infinite, π/2 infinite", "-π/2, π/2", "", w(-2.0, 2.0, -6.0, 6.0)),
        ("N20", "y = 1/(x^2 - 2)", "", "sqrt(2)", "-∞, ∞, DNE, undef", "0, 0", "-√2 infinite, √2 infinite", "-√2, √2", "0", n()),
        ("N21", "y = (x - 2)/(x^2 - 4)", "", "2; -2", "1/4, 1/4, 1/4, undef | -∞, ∞, DNE, undef", "0, 0", "-2 infinite, 2 removable", "-2", "0", n()),
        ("N22", "y = 3 - 1/(x - 1)^2", "", "1", "-∞, -∞, -∞, undef", "3, 3", "1 infinite", "1", "3", n()),
        ("N23", "y = (x + 1)/(x + 1)", "", "-1", "1, 1, 1, undef", "1, 1", "-1 removable", "", "1", n()),
        ("N24", "y = 0.5x + 1, x < 2\ny = 0.5x + 1.5, x >= 2", "", "2", "2, 2.5, DNE, 2.5", "∞, -∞", "2 jump", "", "", n()),
        ("N25", "y = cos(x)/x", "", "", "", "0, 0", "0 infinite", "0", "0", n()),
        ("N26", "y = sin(x)/x", "", "", "", "0, 0", "0 removable", "", "0", json!({"xmin": -30.0, "xmax": 30.0, "xstep": 5.0})),
        // values where 1 + 2/x > 0: x > 0 or x < -2; (1 + 2/x)^x → e²
        ("N27", "y = (1 + 2/x)^x", "", "", "", "e^2, e^2", "", "-2", "e^2", n()),
        ("N28", "y = x sin(x)", "", "", "", "DNE, DNE", "", "", "", n()),
        ("N29", "y = sin(x^2)/x", "", "0", "0, 0, 0, undef", "0, 0", "0 removable", "", "0", n()),
        ("N30", "y = (3x^3 - x)/(x^3 + 7)", "", "", "", "3, 3", "-∛7 infinite", "-∛7", "3", n()),
        ("F1", "y = (exp(x) - exp(9))/(x - 9)", "", "9", "e^9, e^9, e^9, undef", "", "9 removable", "", "0", json!({"xmax": 10.0})),
        ("F2", "y = (x - 7)exp(x)/(x - 7)", "", "7", "e^7, e^7, e^7, undef", "∞, 0", "7 removable", "", "0", n()),
        ("F2b", "y = (x - 3)(4 + exp(x))/(x - 3)", "", "3", "4 + e^3, 4 + e^3, 4 + e^3, undef", "∞, 4", "3 removable", "", "4", json!({"ymax": 30.0, "ystep": 5.0})),
        ("F2c", "y = (x - 1)(6 + cos(x))/(x - 1)", "", "1", "6 + cos 1, 6 + cos 1, 6 + cos 1, undef", "DNE, DNE", "1 removable", "", "", n()),
        ("F3", "y = (x - 2)(x^2 + sin(x))/(x - 2)", "", "2", "4 + sin 2, 4 + sin 2, 4 + sin 2, undef", "∞, ∞", "2 removable", "", "", n()),
        ("F4", "y = exp(x)", "", "6", "e^6, e^6, e^6, e^6", "∞, 0", "", "", "0", n()),
        ("F5", "y = 1/(x - 0.3)", "", "0.3", "-∞, ∞, DNE, undef", "0, 0", "0.3 infinite", "0.3", "0", n()),
        ("F6", "y = (x^2 - 0.09)/(x - 0.3)", "", "0.3", "0.6, 0.6, 0.6, undef", "∞, -∞", "0.3 removable", "", "", n()),
        // ── more classroom functions ──
        ("A01", "y = x^2", "", "3", "9, 9, 9, 9", "∞, ∞", "", "", "", n()),
        ("A02", "y = -x^3", "", "-1", "1, 1, 1, 1", "-∞, ∞", "", "", "", n()),
        ("A03", "y = 3", "", "0", "3, 3, 3, 3", "3, 3", "", "", "3", n()),
        ("A04", "y = |x|", "", "0", "0, 0, 0, 0", "∞, ∞", "", "", "", n()),
        ("A05", "y = |x - 2|/(x - 2)", "", "2", "-1, 1, DNE, undef", "1, -1", "2 jump", "", "1, -1", n()),
        ("A06", "y = 1/x", "", "0", "-∞, ∞, DNE, undef", "0, 0", "0 infinite", "0", "0", n()),
        ("A07", "y = 1/x^3", "", "0", "-∞, ∞, DNE, undef", "0, 0", "0 infinite", "0", "0", n()),
        ("A08", "y = -1/x^2", "", "0", "-∞, -∞, -∞, undef", "0, 0", "0 infinite", "0", "0", n()),
        // (x + 3)/((x - 3)(x + 3)) = 1/(x - 3): -1/6 at -3
        ("A09", "y = (x + 3)/(x^2 - 9)", "", "-3; 3", "-1/6, -1/6, -1/6, undef | -∞, ∞, DNE, undef", "0, 0", "-3 removable, 3 infinite", "3", "0", n()),
        ("A10", "y = (x^2 + 5x + 6)/(x + 2)", "", "-2", "1, 1, 1, undef", "∞, -∞", "-2 removable", "", "", n()),
        ("A11", "y = (x^3 + 1)/(x + 1)", "", "-1", "3, 3, 3, undef", "∞, ∞", "-1 removable", "", "", n()),
        ("A12", "y = (x^4 - 16)/(x - 2)", "", "2", "32, 32, 32, undef", "∞, -∞", "2 removable", "", "", w(-6.0, 10.0, -6.0, 40.0)),
        ("A13", "y = (2x^2 - 8)/(x + 2)", "", "-2", "-8, -8, -8, undef", "∞, -∞", "-2 removable", "", "", w(-6.0, 10.0, -10.0, 7.0)),
        ("A14", "y = (x^2 - 4x + 4)/(x - 2)", "", "2", "0, 0, 0, undef", "∞, -∞", "2 removable", "", "", n()),
        // (x - 4)/(√x - 2) = √x + 2: 4 at 4; f(0) = -4/-2 = 2
        ("A15", "y = (x - 4)/(sqrt(x) - 2)", "", "4; 0", "4, 4, 4, undef | DNE, 2, DNE, 2", "∞, none", "4 removable", "", "", n()),
        ("A16", "y = (sqrt(x + 1) - 1)/x", "", "0", "1/2, 1/2, 1/2, undef", "0, none", "0 removable", "", "0", n()),
        ("A17", "y = (sqrt(9 + x) - 3)/x", "", "0", "1/6, 1/6, 1/6, undef", "0, none", "0 removable", "", "0", n()),
        ("A18", "y = sin(5x)/x", "", "0", "5, 5, 5, undef", "0, 0", "0 removable", "", "0", n()),
        ("A19", "y = tan(x)/x", "", "0", "1, 1, 1, undef", "DNE, DNE", "-3π/2 infinite, -π/2 infinite, 0 removable, π/2 infinite, 3π/2 infinite, 5π/2 infinite", "-3π/2, -π/2, π/2, 3π/2, 5π/2", "", n()),
        ("A20", "y = (1 - cos(2x))/x^2", "", "0", "2, 2, 2, undef", "0, 0", "0 removable", "", "0", n()),
        // x/sin x: 1 at 0; at ±π, 2π, 3π a nonzero number over 0
        ("A21", "y = x/sin(x)", "", "0; pi", "1, 1, 1, undef | ∞, -∞, DNE, undef", "DNE, DNE", "-π infinite, 0 removable, π infinite, 2π infinite, 3π infinite", "-π, π, 2π, 3π", "", n()),
        ("A22", "y = (exp(2x) - 1)/x", "", "0", "2, 2, 2, undef", "∞, 0", "0 removable", "", "0", n()),
        // ln(1 + x)/x: 1 at 0; at -1⁺ ln(t)/(-1) → ∞
        ("A23", "y = ln(1 + x)/x", "", "0", "1, 1, 1, undef", "0, none", "0 removable", "-1", "0", n()),
        ("A24", "y = x ln(x)", "", "0", "DNE, 0, DNE, undef", "∞, none", "", "", "", n()),
        // (x - 1)(x + 1)/((x - 1)(x + 2)) = (x + 1)/(x + 2): 2/3 at 1; at -2⁻ (-1)/(0⁻) = ∞
        ("A26", "y = (x^2 - 1)/(x^2 + x - 2)", "", "1; -2", "2/3, 2/3, 2/3, undef | ∞, -∞, DNE, undef", "1, 1", "-2 infinite, 1 removable", "-2", "1", n()),
        ("A27", "y = 1/(x^2 - 4)", "", "2", "-∞, ∞, DNE, undef", "0, 0", "-2 infinite, 2 infinite", "-2, 2", "0", n()),
        ("A28", "y = x/(x^2 + 1)", "", "", "", "0, 0", "", "", "0", n()),
        ("A29", "y = (2x^2 + 3)/(x^2 - 1)", "", "1", "-∞, ∞, DNE, undef", "2, 2", "-1 infinite, 1 infinite", "-1, 1", "2", n()),
        // 5x³/(x(2x² + 1)) = 5x²/(2x² + 1): 0 at 0, 5/2 at ±∞
        ("A30", "y = 5x^3/(2x^3 + x)", "", "0", "0, 0, 0, undef", "5/2, 5/2", "0 removable", "", "5/2", n()),
        ("A31", "y = (x^3 - 2x)/(x^2 + 1)", "", "", "", "∞, -∞", "", "", "", n()),
        ("A32", "y = sqrt(x^2 + 1)/x", "", "0", "-∞, ∞, DNE, undef", "1, -1", "0 infinite", "0", "1, -1", n()),
        ("A33", "y = (3x + 2)/sqrt(4x^2 + 1)", "", "", "", "3/2, -3/2", "", "", "3/2, -3/2", n()),
        // e^(-x) sin x: squeezed to 0 at ∞; at -∞ it swings ever wider
        ("A34", "y = exp(-x) sin(x)", "", "", "", "0, DNE", "", "", "0", n()),
        ("A35", "y = x exp(-x)", "", "", "", "0, -∞", "", "", "0", n()),
        ("A36", "y = x^2 exp(x)", "", "", "", "∞, 0", "", "", "0", n()),
        ("A37", "y = ln(x)/sqrt(x)", "", "", "", "0, none", "", "0", "0", n()),
        ("A38", "y = ln(x)^2/x", "", "", "", "0, none", "", "0", "0", n()),
        ("A39", "y = x^(1/x)", "", "", "", "1, none", "", "", "1", n()),
        ("A40", "y = (1 + 1/x)^(2x)", "", "", "", "e^2, e^2", "", "-1", "e^2", n()),
        ("A41", "y = (1 - 1/x)^x", "", "", "", "e^(-1), e^(-1)", "", "", "e^(-1)", n()),
        ("A42", "y = cos(1/x)", "", "0", "DNE, DNE, DNE, undef", "1, 1", "0 oscillating", "", "1", n()),
        ("A43", "y = x cos(1/x)", "", "0", "0, 0, 0, undef", "∞, -∞", "0 removable", "", "", n()),
        ("A44", "y = x^2, x <= 0\ny = 1/x, x > 0", "", "0", "0, ∞, DNE, 0", "0, ∞", "0 infinite", "0", "0", n()),
        ("A45", "y = 2, x < 1\ny = 2, x > 1", "(1, 4)", "1", "2, 2, 2, 4", "2, 2", "1 removable", "", "2", n()),
        ("A46", "y = 3 - x, x < 2\ny = x - 1, x >= 2", "", "2", "1, 1, 1, 1", "∞, ∞", "", "", "", n()),
        ("A47", "y = sin(x), x <= 0\ny = cos(x), x > 0", "", "0", "0, 1, DNE, 0", "DNE, DNE", "0 jump", "", "", n()),
        ("A48", "y = exp(x), x < 0\ny = x + 1, x >= 0", "", "0", "1, 1, 1, 1", "∞, 0", "", "", "0", n()),
        ("A49", "y = x, x <= 0\ny = ln(x), x > 0", "", "0", "0, -∞, DNE, 0", "∞, -∞", "0 infinite", "0", "", n()),
        ("A50", "y = 1/(x + 1), x < 0\ny = 2, x >= 0", "", "0; -1", "1, 2, DNE, 2 | -∞, ∞, DNE, undef", "2, 0", "-1 infinite, 0 jump", "-1", "2, 0", n()),
        ("A51", "y = x, x < -1\ny = x^2, -1 <= x <= 1\ny = 2 - x, x > 1", "", "-1; 1", "-1, 1, DNE, 1 | 1, 1, 1, 1", "-∞, -∞", "-1 jump", "", "", n()),
        ("A52", "y = 1, 0 <= x < 1\ny = 2, 1 <= x < 2", "", "1", "1, 2, DNE, 2", "none, none", "1 jump", "", "", n()),
        ("A53", "y = (x^2 - 9)/(x - 3), x != 3", "", "3", "6, 6, 6, undef", "∞, -∞", "3 removable", "", "", n()),
        ("A55", "y = x^(2/3)", "", "0; -8", "0, 0, 0, 0 | 4, 4, 4, 4", "∞, ∞", "", "", "", w(-10.0, 10.0, -6.0, 7.0)),
        ("A56", "y = (x - 1)^(1/3)", "", "1", "0, 0, 0, 0", "∞, -∞", "", "", "", n()),
        ("A58", "y = 1/(x - pi)", "", "pi", "-∞, ∞, DNE, undef", "0, 0", "π infinite", "π", "0", n()),
        ("A59", "y = cot(x)", "", "0", "-∞, ∞, DNE, undef", "DNE, DNE", "0 infinite, π infinite", "0, π", "", w(-1.0, 4.0, -6.0, 6.0)),
        ("A60", "y = sec(x)", "", "pi/2", "∞, -∞, DNE, undef", "DNE, DNE", "-π/2 infinite, π/2 infinite", "-π/2, π/2", "", w(-2.0, 2.0, -6.0, 6.0)),
        ("A61", "y = atan(x) + 1", "", "", "", "1 + π/2, 1 - π/2", "", "", "1 + π/2, 1 - π/2", n()),
        ("A62", "y = 2 + exp(-x^2)", "", "0", "3, 3, 3, 3", "2, 2", "", "", "2", n()),
        ("A63", "y = (x^2 - 4)/(x + 2)", "(-2, -4)", "-2", "-4, -4, -4, -4", "∞, -∞", "", "", "", n()),
        ("A64", "y = 1/(x - 1) + 1/(x + 1)", "", "1", "-∞, ∞, DNE, undef", "0, 0", "-1 infinite, 1 infinite", "-1, 1", "0", n()),
        ("A65", "y = (x^2 + 1)/(x^2 - 1)", "", "1", "-∞, ∞, DNE, undef", "1, 1", "-1 infinite, 1 infinite", "-1, 1", "1", n()),
        ("A66", "y = x/abs(x) + x", "", "0", "-1, 1, DNE, undef", "∞, -∞", "0 jump", "", "", n()),
        ("A67", "y = log(x)", "", "10", "1, 1, 1, 1", "∞, none", "", "0", "", w(-2.0, 12.0, -6.0, 7.0)),
        ("A69", "y = (x - 2)/(x - 2)^2", "", "2", "-∞, ∞, DNE, undef", "0, 0", "2 infinite", "2", "0", n()),
        ("A70", "y = (x + 1)^2/(x + 1)", "", "-1", "0, 0, 0, undef", "∞, -∞", "-1 removable", "", "", n()),
        ("A71", "y = 10/(x - 5)^2 + 1", "", "5", "∞, ∞, ∞, undef", "1, 1", "5 infinite", "5", "1", n()),
        ("A72", "y = sqrt(x^2)/x", "", "0", "-1, 1, DNE, undef", "1, -1", "0 jump", "", "1, -1", n()),
        // sin 2x/sin 3x: 2/3 at 0; sin 3x = 0 at ±π/3 where sin 2x ≠ 0
        // (named: the limits of sin 2x/sin 3x at ±∞ are not established by the analysis, so the key leaves lines for them)
        ("A73", "y = sin(2x)/sin(3x)", "", "0", "2/3, 2/3, 2/3, undef", "?, ?", "-π/3 infinite, 0 removable, π/3 infinite", "-π/3, π/3", "", w(-2.0, 2.0, -6.0, 6.0)),
        ("A74", "y = x^3 - x", "", "0", "0, 0, 0, 0", "∞, -∞", "", "", "", n()),
        ("A75", "y = 4 - (x - 1)^2/3, x < 1, x ≠ -2\ny = 2 + 3/(x - 4), x >= 1", "(-2, 3)", "-2; 1; 4", "1, 1, 1, 3 | 4, 1, DNE, 1 | -∞, ∞, DNE, undef", "2, -∞", "-2 removable, 1 jump, 4 infinite", "4", "2", n()),
        ("A76", "y = 1/(x^2 + 2x - 3)", "", "1; -3", "-∞, ∞, DNE, undef | ∞, -∞, DNE, undef", "0, 0", "-3 infinite, 1 infinite", "-3, 1", "0", n()),
        // x² - 2x - 1 = 0 at 1 ± √2
        ("A77", "y = 1/(x^2 - 2x - 1)", "", "", "", "0, 0", "1 - √2 infinite, 1 + √2 infinite", "1 - √2, 1 + √2", "0", n()),
        ("A78", "y = (x^2 - 3x)/(x^2 - 9)", "", "3; -3", "1/2, 1/2, 1/2, undef | ∞, -∞, DNE, undef", "1, 1", "-3 infinite, 3 removable", "-3", "1", n()),
        ("A79", "y = (4x^2 - 1)/(2x - 1)", "", "1/2", "2, 2, 2, undef", "∞, -∞", "1/2 removable", "", "", n()),
        ("A80", "y = (x^2 - 1/4)/(x - 1/2)", "", "1/2", "1, 1, 1, undef", "∞, -∞", "1/2 removable", "", "", n()),
        ("A81", "y = 1/(2x - 3)", "", "3/2", "-∞, ∞, DNE, undef", "0, 0", "3/2 infinite", "3/2", "0", n()),
        ("A82", "y = (x - 5)/(x^2 - 25)", "", "5", "1/10, 1/10, 1/10, undef", "0, 0", "-5 infinite, 5 removable", "-5", "0", n()),
        ("A83", "y = (6x^2 + 1)/(3x^2 - x)", "", "0", "∞, -∞, DNE, undef", "2, 2", "0 infinite, 1/3 infinite", "0, 1/3", "2", n()),
        ("A84", "y = (x^3 - x)/(x - 1)", "", "1", "2, 2, 2, undef", "∞, ∞", "1 removable", "", "", n()),
        ("A85", "y = sin(x - 1)/(x - 1)", "", "1", "1, 1, 1, undef", "0, 0", "1 removable", "", "0", n()),
        ("A86", "y = (cos(x) - 1)/x", "", "0", "0, 0, 0, undef", "0, 0", "0 removable", "", "0", n()),
        ("A87", "y = (exp(x) - exp(-x))/x", "", "0", "2, 2, 2, undef", "∞, ∞", "0 removable", "", "", n()),
        ("A88", "y = (2^x - 1)/x", "", "0", "ln 2, ln 2, ln 2, undef", "∞, 0", "0 removable", "", "0", n()),
        ("A89", "y = ln(x)/(x - 1)", "", "1", "1, 1, 1, undef", "0, none", "1 removable", "0", "0", n()),
        ("A90", "y = (sqrt(x) - 1)/(x - 1)", "", "1", "1/2, 1/2, 1/2, undef", "0, none", "1 removable", "", "0", n()),
        ("A91", "y = x/(sqrt(x + 1) - 1)", "", "0", "2, 2, 2, undef", "∞, none", "0 removable", "", "", n()),
        ("A92", "y = (x^2 + x)/(x^2 - x)", "", "0; 1", "-1, -1, -1, undef | -∞, ∞, DNE, undef", "1, 1", "0 removable, 1 infinite", "1", "1", n()),
        ("A93", "y = abs(x^2 - 4)/(x - 2)", "", "2", "-4, 4, DNE, undef", "∞, -∞", "2 jump", "", "", n()),
        ("A94", "y = 1/abs(x)", "", "0", "∞, ∞, ∞, undef", "0, 0", "0 infinite", "0", "0", n()),
        ("A95", "y = exp(1/(x - 2))", "", "2", "0, ∞, DNE, undef", "1, 1", "2 infinite", "2", "1", n()),
        ("A96", "y = atan(1/(x - 1))", "", "1", "-π/2, π/2, DNE, undef", "0, 0", "1 jump", "", "0", n()),
        ("A97", "y = (x - 1)/abs(x - 1)", "", "1", "-1, 1, DNE, undef", "1, -1", "1 jump", "", "1, -1", n()),
        ("A98", "y = floor(x/2)", "", "2", "0, 1, DNE, 1", "∞, -∞", "-2 jump, 0 jump, 2 jump", "", "", w(-3.0, 3.0, -3.0, 3.0)),
        ("A99", "y = ceil(x)", "", "1", "1, 2, DNE, 1", "∞, -∞", "-2 jump, -1 jump, 0 jump, 1 jump, 2 jump", "", "", w(-2.5, 2.5, -3.0, 3.0)),
        ("B01", "y = x + 1, x < 0\ny = x + 1, x > 0", "", "0", "1, 1, 1, undef", "∞, -∞", "0 removable", "", "", n()),
        ("B02", "y = x + 1, x < 0\ny = x + 1, x > 0", "(0, 1)", "0", "1, 1, 1, 1", "∞, -∞", "", "", "", n()),
        ("B03", "y = 2x, x <= 1\ny = 5, x > 1", "", "1", "2, 5, DNE, 2", "5, -∞", "1 jump", "", "5", n()),
        ("B04", "y = 1/(x - 3), x > 3\ny = x, x <= 3", "", "3", "3, ∞, DNE, 3", "0, -∞", "3 infinite", "3", "0", n()),
        ("B05", "y = sqrt(x), x >= 0\ny = -x, x < 0", "", "0", "0, 0, 0, 0", "∞, ∞", "", "", "", n()),
        ("B06", "y = x^2 - 1, x != 1", "(1, 3)", "1", "0, 0, 0, 3", "∞, ∞", "1 removable", "", "", n()),
        ("B07", "y = 4, x < 2\ny = x^2, x > 2", "(2, 4)", "2", "4, 4, 4, 4", "∞, 4", "", "", "4", n()),
        ("B08", "(-5, 2) (-3, 2) (-1, 4) open\nopen (-1, 4) (2, 3) (4, 3)", "(-1, 1)", "-1", "4, 4, 4, 1", "none, none", "-1 removable", "", "", n()),
        ("B09", "(-6, 0) (-4, 3) (-2, 1) open\nopen (-2, 4) (0, 2) (3, 2)", "", "-2", "1, 4, DNE, undef", "none, none", "-2 jump", "", "", n()),
        ("B10", "closed (-3, 1) (0, 3) (3, 1) closed", "", "-3; 0; 3", "DNE, 1, DNE, 1 | 3, 3, 3, 3 | 1, DNE, DNE, 1", "none, none", "", "", "", n()),
        ("B11", "(-6, 1) (-4, 2) (-2, 0) open\ny = 1/(x + 1), x > -1", "", "-2; -1", "0, DNE, DNE, undef | DNE, ∞, DNE, undef", "0, none", "", "-1", "0", n()),
        ("B12", "y = (x^2 - 1)/(x + 1), x < 2\ny = 5 - x, x >= 2", "", "-1; 2", "-2, -2, -2, undef | 1, 3, DNE, 3", "-∞, -∞", "-1 removable, 2 jump", "", "", n()),
        ("B13", "y = x + 2, x < 1\ny = 4 - 2x, x >= 1", "", "1", "3, 2, DNE, 2", "-∞, -∞", "1 jump", "", "", n()),
        ("B14", "y = 1, x < 0\ny = -1, x > 0", "(0, 0)", "0", "1, -1, DNE, 0", "-1, 1", "0 jump", "", "-1, 1", n()),
        ("B15", "y = x^2 sin(1/x), x != 0", "(0, 0)", "0", "0, 0, 0, 0", "∞, -∞", "", "", "", n()),
        ("B16", "y = (x^2 + 2x + 1)/(x + 1)^2", "", "-1", "1, 1, 1, undef", "1, 1", "-1 removable", "", "1", n()),
        ("B17", "y = (x + 2)/(x^2 + 4x + 4)", "", "-2", "-∞, ∞, DNE, undef", "0, 0", "-2 infinite", "-2", "0", n()),
        ("B18", "y = 1/(x^2 + 1) + 2", "", "0", "3, 3, 3, 3", "2, 2", "", "", "2", n()),
        ("B19", "y = (2x - 1)/(x + 1)", "", "-1", "∞, -∞, DNE, undef", "2, 2", "-1 infinite", "-1", "2", n()),
        ("B20", "y = (x^2 - 4)/(x^2 - x - 2)", "", "2; -1", "4/3, 4/3, 4/3, undef | -∞, ∞, DNE, undef", "1, 1", "-1 infinite, 2 removable", "-1", "1", n()),
        ("B21", "y = 5x/(x^2 + 1)", "", "", "", "0, 0", "", "", "0", n()),
        ("B22", "y = 3 - 2/x^2", "", "0", "-∞, -∞, -∞, undef", "3, 3", "0 infinite", "0", "3", n()),
        ("B23", "y = (4x^2 - 1)/(2x^2 + 3x - 2)", "", "1/2; -2", "4/5, 4/5, 4/5, undef | ∞, -∞, DNE, undef", "2, 2", "-2 infinite, 1/2 removable", "-2", "2", n()),
        ("B24", "y = (x^2 + 1)/(x - 1)", "", "1", "-∞, ∞, DNE, undef", "∞, -∞", "1 infinite", "1", "", n()),
        ("B25", "y = 2x/(x - 1)", "", "1", "-∞, ∞, DNE, undef", "2, 2", "1 infinite", "1", "2", n()),
        ("B26", "y = x^2/(x^2 - 4)", "", "2; -2", "-∞, ∞, DNE, undef | ∞, -∞, DNE, undef", "1, 1", "-2 infinite, 2 infinite", "-2, 2", "1", n()),
        ("B27", "y = 3/(x + 1)^2", "", "-1", "∞, ∞, ∞, undef", "0, 0", "-1 infinite", "-1", "0", n()),
        ("B28", "y = (x + 1)/(x - 3)", "", "3", "-∞, ∞, DNE, undef", "1, 1", "3 infinite", "3", "1", n()),
        ("B29", "y = 1/(x - 2) + 3", "", "2", "-∞, ∞, DNE, undef", "3, 3", "2 infinite", "2", "3", n()),
        ("B30", "y = (3x - 6)/(x + 2)", "", "-2", "∞, -∞, DNE, undef", "3, 3", "-2 infinite", "-2", "3", n()),
        ("B31", "y = e^(-x)", "", "0", "1, 1, 1, 1", "0, ∞", "", "", "0", n()),
        ("B32", "y = ln(x + 2)", "", "-1", "0, 0, 0, 0", "∞, none", "", "-2", "", n()),
        ("B33", "y = (x^2 + 3x + 2)/(x + 1)", "", "-1", "1, 1, 1, undef", "∞, -∞", "-1 removable", "", "", n()),
        ("B34", "y = (x - 2)/(x^2 - 5x + 6)", "", "2; 3", "-1, -1, -1, undef | -∞, ∞, DNE, undef", "0, 0", "2 removable, 3 infinite", "3", "0", n()),
        ("B35", "y = (x^2 - x)/(x^2 - 1)", "", "1; -1", "1/2, 1/2, 1/2, undef | ∞, -∞, DNE, undef", "1, 1", "-1 infinite, 1 removable", "-1", "1", n()),
        ("B36", "y = 1/(1 + exp(-x))", "", "0", "1/2, 1/2, 1/2, 1/2", "1, 0", "", "", "1, 0", n()),
        ("B37", "y = (x^2 - 0.25)/(x - 0.5)", "", "0.5", "1, 1, 1, undef", "∞, -∞", "0.5 removable", "", "", n()),
        ("B38", "y = 0.5/(x - 1.5)", "", "1.5", "-∞, ∞, DNE, undef", "0, 0", "1.5 infinite", "1.5", "0", n()),
    ]
}

/// Results the analysis is allowed to leave as "cannot be determined" (none: every row is an ordinary textbook function).
const UNDETERMINED: &[&str] = &[];

#[test]
fn the_table_of_hand_worked_functions() {
    let rows = rows();
    assert!(rows.len() >= 150, "{} rows", rows.len());
    let mut fails: Vec<String> = vec![];
    let mut checked = 0usize;
    for (name, pieces, dots, asks, ans, inf, discs, vas, has, more) in rows {
        let mut o = json!({"pieces": pieces, "dots": dots, "ask_at": asks.replace(';', ","), "ask_inf": "Both", "ask_disc": true});
        for (k, v) in more.as_object().unwrap() { o.as_object_mut().unwrap().insert(k.clone(), v.clone()); }
        let p = with(o);
        let md = super::model_of(&p);
        let w = words(&def().svg(&p));
        macro_rules! check { ($what:expr, $got:expr, $want:expr) => {{ let (got, want): (String, &str) = ($got, $want); checked += 1; if got != want.trim() { fails.push(format!("{name} {pieces:?}: {}: got {got:?}, hand-worked {want:?}", $what)); } }}; }
        if !asks.is_empty() {
            for (a, group) in asks.split(';').zip(ans.split(" | ")) {
                let k = Num::read(a).unwrap().k;
                let f: Vec<&str> = group.split(", ").collect();
                check!(&format!("left limit at {a}"), lim_key(&md.side(&k, -1.0)), f[0]);
                check!(&format!("right limit at {a}"), lim_key(&md.side(&k, 1.0)), f[1]);
                check!(&format!("limit at {a}"), lim_key(&md.two(&k)), f[2]);
                check!(&format!("f({a})"), val_key(&md.value(&k)), f[3]);
                // and the key drawn says the same
                let at = Num::read(a).unwrap().text;
                // (as the key draws it: e^9 is e with a raised 9, so its text reads e9; e^(-1) reads e-1)
                let shown = |s: &str| match s { "DNE" => "does not exist".to_string(), "undef" => "undefined".to_string(), o => { let mut t = o.to_string(); while let Some(i) = t.find("^(") { let j = t[i..].find(')').map(|j| i + j).unwrap_or(t.len() - 1); t = format!("{}{}{}", &t[..i], &t[i + 2..j], &t[j + 1..]); } t.replace('^', "") } };
                if !w.contains(&format!("f({at}) = {}", shown(f[3]))) { fails.push(format!("{name}: the key does not write f({at}) = {}: {w}", shown(f[3]))); }
                if !w.contains(&format!("lim x→{at}- f(x) = {}", shown(f[0]))) { fails.push(format!("{name}: the key does not write the left limit at {at} as {}", shown(f[0]))); }
            }
        }
        if !inf.is_empty() {
            let f: Vec<&str> = inf.split(", ").collect();
            for (dir, want) in [(1.0, f[0]), (-1.0, f[1])] {
                let got = md.at_inf(dir).map_or("none".to_string(), |l| lim_key(&l));
                check!(&format!("limit at {}∞", if dir < 0.0 { "-" } else { "" }), got, want);
            }
        }
        let (ds, complete) = md.discontinuities();
        if !complete { fails.push(format!("{name}: the list of discontinuities is not sure to be complete")); }
        check!("discontinuities", ds.iter().map(|d| format!("{} {}", d.text, d.kind)).collect::<Vec<_>>().join(", "), discs);
        if ds.iter().any(|d| d.kind == UNSURE) && !UNDETERMINED.contains(&name) { fails.push(format!("{name}: a discontinuity it cannot determine")); }
        check!("vertical asymptotes", md.verticals().iter().map(|v| v.1.clone()).collect::<Vec<_>>().join(", "), vas);
        check!("horizontal asymptotes", md.horizontals().iter().map(|v| v.1.clone()).collect::<Vec<_>>().join(", "), has);
        // "nowhere" only when established
        if discs.is_empty() && !w.contains("nowhere in the window") { fails.push(format!("{name}: the key does not say nowhere: {w}")); }
    }
    println!("{checked} hand-worked values checked");
    assert!(fails.is_empty(), "{} differ:\n{}", fails.len(), fails.join("\n"));
}
