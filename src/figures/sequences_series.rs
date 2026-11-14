//! Requested finite sums and compact, derived textbook notation.
use super::*;
use num_bigint::BigInt;
use num_rational::BigRational as B;
use num_traits::ToPrimitive;
fn big(n: N) -> Option<B> {
    let (a, b) = n.q?;
    Some(B::new(BigInt::from(a), BigInt::from(b)))
}
pub(super) struct Answer {
    pub value: f64,
    pub exact: Option<String>,
    pub digits: usize,
}
impl Answer {
    pub fn eq(&self) -> String {
        if let Some(t) = &self.exact {
            format!("= {t}")
        } else {
            format!("≈ {}", approximation(self.value))
        }
    }
}
pub(super) fn total(p: &Params, d: &Data) -> Result<Answer, String> {
    let count = p.i("sum_count").clamp(1, 1000) as usize;
    let n = N::int(count as i128);
    let one = N::int(1);
    let exact = match p.s("mode") {
        "Arithmetic" => big(d.first).zip(big(d.k)).map(|(a, k)| {
            B::from_integer(BigInt::from(count))
                * (B::from_integer(2.into()) * a + B::from_integer(BigInt::from(count - 1)) * k)
                / B::from_integer(2.into())
        }),
        "Geometric" => big(d.first).zip(big(d.k)).map(|(a, r)| {
            if r == B::from_integer(1.into()) {
                a * B::from_integer(BigInt::from(count))
            } else {
                a * (B::from_integer(1.into()) - r.pow(count as i32))
                    / (B::from_integer(1.into()) - r)
            }
        }),
        _ => None,
    };
    let mut exact = exact;
    let mut value = match p.s("mode") {
        "Arithmetic" => {
            n.op(
                d.first
                    .op(N::int(2), '*')
                    .op(n.op(one, '-').op(d.k, '*'), '+'),
                '*',
            )
            .op(N::int(2), '/')
            .v
        }
        "Geometric" => {
            let delta = one.op(d.k, '-').v;
            if d.k.q == Some((1, 1)) {
                d.first.v * count as f64
            } else if d.first.q == Some((0, 1)) {
                0.0
            } else if d.k.v == 0.0 {
                d.first.v
            } else {
                let small = if d.k.v > 0.0 {
                    delta
                } else {
                    one.op(d.k, '+').v
                };
                let exponent = if small.abs() < 0.5 {
                    (-small).ln_1p() * count as f64
                } else {
                    d.k.v.abs().ln() * count as f64
                };
                let numerator = if d.k.v > 0.0 || count % 2 == 0 {
                    -exponent.exp_m1()
                } else {
                    1.0 + exponent.exp()
                };
                d.first.v * numerator / delta
            }
        }
        _ => {
            let start = if p.s("start") == "0" { 0 } else { 1 };
            let mut last = vec![];
            let mut sum = N::int(0);
            let mut bsum = Some(B::from_integer(0.into()));
            for j in 0..count {
                let v = if p.s("mode") == "Explicit" {
                    read(
                        p.s("formula"),
                        N::int((start + j) as i128),
                        N::int(0),
                        N::int(0),
                    )
                } else if j == 0 {
                    Some(d.first)
                } else if j == 1 && recurrence_order(p) == 2 {
                    number(p.s("second"))
                } else {
                    read(
                        p.s("recurrence"),
                        N::int((start + j) as i128),
                        last[j - 1],
                        if j >= 2 { last[j - 2] } else { N::int(0) },
                    )
                }
                .ok_or_else(|| format!("The requested sum cannot evaluate term {}.", start + j))?;
                if !v.v.is_finite() {
                    return Err("The requested sum exceeds the supported numerical range.".into());
                }
                bsum = bsum.zip(big(v)).map(|(a, b)| a + b);
                sum = sum.op(v, '+');
                last.push(v);
            }
            exact = bsum;
            if p.s("mode") == "Explicit" {
                if let Some(v) = trig_sum(p.s("formula"), start, count) {
                    exact = big(v);
                    sum = v;
                }
            }
            sum.v
        }
    };
    let mut digits = 0;
    let mut text = None;
    if let Some(q) = exact {
        value = q
            .to_f64()
            .ok_or("The requested sum exceeds the supported numerical range.")?;
        let t = q.to_string();
        digits = t.chars().count();
        if digits <= 110 {
            let decimal =
                matches!(p.s("mode"), "Arithmetic" | "Geometric") && (d.first.dec || d.k.dec);
            text = Some(if decimal {
                q.numer()
                    .to_i128()
                    .zip(q.denom().to_i128())
                    .and_then(|(a, b)| exact_decimal(a, b))
                    .unwrap_or(t)
            } else {
                t
            });
        }
    }
    if !value.is_finite() {
        return Err("The requested sum exceeds the supported numerical range.".into());
    }
    Ok(Answer {
        value,
        exact: text,
        digits,
    })
}
fn negative(n: N) -> bool {
    n.q.map_or(n.v < 0.0, |(a, _)| a < 0)
}
fn abs_text(n: N) -> String {
    let s = n.text();
    s.trim_start_matches('-')
        .trim_start_matches('−')
        .to_string()
}
pub(super) fn formula(p: &Params, d: &Data, var: &str) -> String {
    let start = if p.s("start") == "0" { 0 } else { 1 };
    match p.s("mode") {
        "Arithmetic" => {
            let k = d.k;
            let b = d.first.op(N::int(start).op(k, '*'), '-');
            if k.q.is_some() && b.q.is_some() {
                if k.q == Some((0, 1)) {
                    return d.first.text();
                }
                let mut term = match k.q {
                    Some((1, 1)) => var.into(),
                    Some((-1, 1)) => format!("−{var}"),
                    Some((_, 1)) => format!("{}{var}", k.text()),
                    _ => format!("({}){var}", k.text()),
                };
                if b.q != Some((0, 1)) {
                    term.push_str(&format!(
                        " {} {}",
                        if negative(b) { "−" } else { "+" },
                        abs_text(b)
                    ));
                }
                term
            } else {
                format!(
                    "{} + ({var} − {start}) × {}",
                    coefficient(p, "first"),
                    coefficient(p, "difference")
                )
            }
        }
        "Geometric" => {
            if d.first.q == Some((0, 1)) {
                return "0".into();
            }
            if d.k.q == Some((1, 1)) {
                return coefficient(p, "first");
            }
            if d.k.q == Some((0, 1)) {
                return format!(
                    "{} ({var} = {start}); 0 ({var} > {start})",
                    coefficient(p, "first")
                );
            }
            let power = if start == 0 {
                var.into()
            } else {
                format!("{var} − {start}")
            };
            let base = coefficient(p, "ratio");
            let base = if d.k.q.is_some_and(|(a, b)| a >= 0 && b == 1) {
                base
            } else {
                format!("({base})")
            };
            let exponent = format!("{base}^({power})");
            if d.first.q == Some((1, 1)) {
                exponent
            } else {
                format!("{} × {exponent}", coefficient(p, "first"))
            }
        }
        "Explicit" => {
            let t = fmt_formula(p.s("formula"));
            let mut out = String::new();
            let mut token = String::new();
            for c in t.chars().chain(std::iter::once(' ')) {
                if c.is_ascii_alphabetic() {
                    token.push(c);
                } else {
                    out.push_str(if token == "n" { var } else { &token });
                    token.clear();
                    out.push(c);
                }
            }
            out.trim_end().into()
        }
        _ => format!("a{}", if var == "k" { "ₖ" } else { "ₙ" }),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requested_sum_fraction_oracles_r1() {
        let fixtures: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("sequences_large_sum_oracles.json")).unwrap();
        for case in &fixtures {
            let mut p = Params::defaults(&params());
            p.set_s("mode", case["mode"].as_str().unwrap());
            p.set_s("first", case["first"].as_str().unwrap());
            p.set_s(
                if p.s("mode") == "Arithmetic" {
                    "difference"
                } else {
                    "ratio"
                },
                case["change"].as_str().unwrap(),
            );
            p.set_i("sum_count", case["n"].as_i64().unwrap());
            let d = data(&p).unwrap();
            let answer = total(&p, &d).unwrap();
            let want = case["value"].as_f64().unwrap();
            assert!(
                (answer.value - want).abs() <= want.abs() * 2e-14 + 1e-14,
                "{case}"
            );
            let exact = case["exact"].as_str().unwrap();
            if exact.len() <= 110 {
                assert_eq!(answer.exact.as_deref(), Some(exact), "{case}");
            } else {
                assert!(answer.exact.is_none());
                assert!(answer.digits > 110);
            }
        }
        assert_eq!(fixtures.len(), 60);
    }
    #[test]
    fn sigma_and_requested_sum_student_rules_r1() {
        for mode in ["Arithmetic", "Geometric", "Explicit", "Recursive"] {
            let mut p = Params::defaults(&params());
            p.set_s("mode", mode);
            p.set_b("sum_n", true);
            p.set_b("sigma", true);
            p.set_i("sum_count", 50);
            let d = data(&p).unwrap();
            let key = rows_for(&p, &d).join(" ");
            assert!(key.contains("Σ_(k = 1)^(50)"));
            assert!(key.contains("Sum of first 50 terms"));
            p.set_b(SHOW_ANSWERS, false);
            let student = rows_for(&p, &d).join(" ");
            assert!(student.contains("Sum of first 50 terms: __________"));
            assert!(student.contains("Σ_(k = 1)^(50) [__________]"));
            let svg = render(&p);
            assert!(svg.contains("baseline-shift=\"sub\""));
            p.set_b(SHOW_ANSWERS, true);
            p.set_i("sum_count", 1000);
            let now = std::time::Instant::now();
            let _ = render(&p);
            assert!(now.elapsed().as_secs_f64() < 0.5);
        }
    }
    #[test]
    fn compact_textbook_formulas_r1() {
        let mut p = Params::defaults(&params());
        assert_eq!(formula(&p, &data(&p).unwrap(), "n"), "n + 1");
        p.set_s("start", "0");
        assert_eq!(formula(&p, &data(&p).unwrap(), "n"), "n + 2");
        p.set_s("mode", "Geometric");
        p.set_s("first", "0");
        p.set_s("ratio", "2");
        p.set_b("sum", true);
        assert!(
            rows_for(&p, &data(&p).unwrap())
                .iter()
                .any(|s| s == "Sum to infinity = 0")
        );
        assert_eq!(number("3.0").unwrap().text(), "3");
    }
}
