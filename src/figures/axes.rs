//! The one place a figure picks a "nice" tick step. Every tab used to carry its own copy; they differ
//! in rounding, in which multiples they allow and in how they treat an odd range, and each saved
//! figure's SVG depends on its exact choice, so the variants are kept as named [`Rule`]s rather than
//! merged. A new figure should use [`nice_step`] (the 1-2-5 rule with guards).

/// How a range is rounded to a step. Each rule reproduces one historical copy exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// 1, 2, 5, 10 x 10^k, rounding up; 1.0 for a non-finite or zero range. The default.
    Standard,
    /// 1, 2, 5, 10 x 10^k, rounding up; the range is floored at 1e-9 per interval (physics graphs).
    Floor,
    /// 1, 2, 2.5, 5, 10 x 10^k, rounding up, 1e-12 tolerance, 1e-9 floor (chemistry graphs, spectra).
    QuarterTol,
    /// 1, 2, 5, 10 x 10^k, rounding up, no guards or tolerance (box plot, number line).
    Plain,
    /// 1, 2, 5, 10 x 10^k, rounding to the nearest (cut points 1.5, 3, 7), 1e-12 floor (biology).
    Nearest,
}

/// A step of 1, 2 or 5 times a power of ten that puts about `target` intervals across `range`.
pub fn nice_step(range: f64, target: f64) -> f64 { nice_step_with(range, target, Rule::Standard) }

/// [`nice_step`] with an explicit rounding rule.
pub fn nice_step_with(range: f64, target: f64, rule: Rule) -> f64 {
    match rule {
        Rule::Standard => {
            let raw = (range / target.max(1.0)).abs();
            if !(raw.is_finite() && raw > 0.0) {
                return 1.0;
            }
            let mag = 10f64.powf(raw.log10().floor());
            for f in [1.0, 2.0, 5.0, 10.0] {
                if raw <= f * mag + 1e-12 {
                    return f * mag;
                }
            }
            10.0 * mag
        }
        Rule::Floor => {
            let raw = (range / target).max(1e-9);
            let mag = 10f64.powf(raw.log10().floor());
            let f = raw / mag;
            let m = if f <= 1.0 { 1.0 } else if f <= 2.0 { 2.0 } else if f <= 5.0 { 5.0 } else { 10.0 };
            m * mag
        }
        Rule::QuarterTol => {
            let raw = (range / target).max(1e-9);
            let mag = 10f64.powf(raw.log10().floor());
            for m in [1.0, 2.0, 2.5, 5.0, 10.0] {
                if m * mag >= raw - 1e-12 {
                    return m * mag;
                }
            }
            10.0 * mag
        }
        Rule::Plain => {
            let raw = range / target;
            let mag = 10f64.powf(raw.log10().floor());
            for f in [1.0, 2.0, 5.0, 10.0] {
                if raw <= f * mag {
                    return f * mag;
                }
            }
            10.0 * mag
        }
        Rule::Nearest => {
            let raw = (range / target).max(1e-12);
            let p = 10f64.powf(raw.log10().floor());
            let f = raw / p;
            let m = if f <= 1.5 { 1.0 } else if f <= 3.0 { 2.0 } else if f <= 7.0 { 5.0 } else { 10.0 };
            m * p
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_is_1_2_5_with_guards() {
        assert_eq!(nice_step(100.0, 5.0), 20.0);
        assert_eq!(nice_step(7.0, 7.0), 1.0);
        assert_eq!(nice_step(0.9, 9.0), 0.1);
        assert_eq!(nice_step(0.0, 5.0), 1.0);
        assert_eq!(nice_step(f64::NAN, 5.0), 1.0);
        assert_eq!(nice_step(-100.0, 5.0), 20.0);
        assert_eq!(nice_step(f64::INFINITY, 5.0), 1.0);
    }

    #[test]
    fn floor_rule_physics_ranges() {
        let f = |r, t| nice_step_with(r, t, Rule::Floor);
        assert_eq!(f(100.0, 7.0), 20.0);
        assert_eq!(f(12.0, 12.0), 1.0);
        assert_eq!(f(300.0, 20.0), 20.0);
        assert_eq!(f(0.0, 8.0), 1e-9);
        assert_eq!(f(f64::NAN, 8.0), 1e-9);
        assert_eq!(f(-5.0, 8.0), 1e-9);
    }

    #[test]
    fn quarter_rules_allow_2_5() {
        assert_eq!(nice_step_with(100.0, 25.0, Rule::QuarterTol), 5.0);
        assert_eq!(nice_step_with(14.0, 14.0, Rule::QuarterTol), 1.0);
        let t = |r, t| nice_step_with(r, t, Rule::QuarterTol);
        assert_eq!(t(50.0, 20.0), 2.5);
        assert_eq!(t(0.0, 20.0), 1e-9);
        assert_eq!(t(25.0 + 1e-13, 10.0), 2.5);
    }

    #[test]
    fn plain_rule_number_line() {
        let p = |r| nice_step_with(r, 8.0, Rule::Plain);
        assert_eq!(p(10.0), 2.0);
        assert_eq!(p(64.0), 10.0);
        assert_eq!(p(1.0), 0.2);
    }

    #[test]
    fn nearest_rule_biology() {
        let n = |r, t| nice_step_with(r, t, Rule::Nearest);
        assert_eq!(n(100.0, 10.0), 10.0);
        assert_eq!(n(16.0, 10.0), 2.0); // 1.6 rounds to 2, where the up rules also give 2
        assert_eq!(n(14.0, 10.0), 1.0); // 1.4 rounds down to 1; the up rules give 2
        assert_eq!(n(75.0, 10.0), 10.0);
        assert_eq!(n(65.0, 10.0), 5.0);
        assert!((n(0.0, 10.0) - 1e-12).abs() < 1e-20);
    }

    #[test]
    fn rules_differ_where_documented() {
        // 1.4 intervals' worth: rounding up gives 2, rounding to nearest gives 1
        assert_eq!(nice_step_with(14.0, 10.0, Rule::Standard), 2.0);
        assert_eq!(nice_step_with(14.0, 10.0, Rule::Nearest), 1.0);
        // 2.2: the quarter rules give 2.5, the 1-2-5 rules 5
        assert_eq!(nice_step_with(22.0, 10.0, Rule::QuarterTol), 2.5);
        assert_eq!(nice_step_with(22.0, 10.0, Rule::Standard), 5.0);
    }
}
