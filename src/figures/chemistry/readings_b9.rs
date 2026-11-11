//! Batch 9 additions to the reading figures (the rest of them is in `measure.rs`): Random buttons
//! on the thermometer, gas syringe, displacement cylinders, 600 mL beaker and (through
//! `math::ruler_b9`) the ruler; the digital probe to 2 decimal places; a 600 mL beaker.
use super::measure::{cyl_marks, marks_from, read_dp, size_ml, temp_scale};
use crate::figures::{unit_random, Params};

/// The digital probe's decimal places: whole degrees, tenths or hundredths.
pub const PROBE_PLACES: [&str; 3] = ["0", "0.1", "0.01"];

/// Decimal places the digital probe shows and keys ("0.1" for a figure saved before the choice).
pub fn probe_dp(p: &Params) -> usize {
    match p.s("places") { "0" => 0, "0.01" => 2, _ => 1 }
}

/// The temperature's unit symbol.
pub fn temp_sym(p: &Params) -> &'static str {
    match p.s("unit") { "Kelvin (K)" => "K", "Fahrenheit (°F)" => "°F", _ => "°C" }
}

/// A digital probe's working range, −50 to 150 °C, in the chosen unit (the reference's range;
/// a typical lab probe such as Vernier's stainless steel temperature probe reads −40 to 135 °C),
/// in kelvin up to the temperature box's 400 (126.85 °C), the most a figure can store.
fn probe_range(p: &Params) -> (f64, f64) {
    match temp_sym(p) { "K" => (223.15, 400.0), "°F" => (-58.0, 302.0), _ => (-50.0, 150.0) }
}

/// The 600 mL beaker's capacity (its rim) when that is the instrument, else None. Its scale stops
/// at 500 mL (`measure::vol_scale`), so it holds 600 mL with the top 100 mL unmarked.
pub fn beaker_capacity(p: &Params) -> Option<f64> {
    (p.s("instrument") == "Beaker" && size_ml(p.s("size")) == 600.0).then_some(600.0)
}

/// A value on the `dp`-decimal grid from `lo` to `hi` (both rounded inward), picked by `u` in [0, 1).
fn pick(lo: f64, hi: f64, dp: usize, u: f64) -> f64 {
    let q = 10f64.powi(dp as i32);
    let (a, b) = ((lo * q - 1e-9).ceil(), (hi * q + 1e-9).floor());
    let k = (a + (u * (b - a + 1.0)).floor()).min(b).max(a.min(b));
    k / q
}

/// A random reading the instrument can show, stored like a typed one so a saved figure or a link
/// keeps it; false when the figure has no Random button here.
/// - thermometer: the glass one's scale (−10 to 110 °C, 260 to 380 K, 10 to 230 °F) to one
///   decimal place; the digital probe's −50 to 150 °C (or the same in K or °F) to its places;
/// - gas syringe: 0.1 to the full scale, to one decimal place (an estimated tenth of its 1 cm³ marks);
/// - displacement: before 40 % to 70 % of the cylinder, the object 5 % to 25 % of it (at least 3
///   marks) so it sinks below the first level and the second stays on the scale, each to the
///   cylinder's reading places (one estimated digit past its smallest marks);
/// - the 600 mL beaker: 5 to 500 mL in 5 mL steps, a tenth of its 50 mL marks (the other
///   instruments' Random is in `measure::random_reading`);
/// - ruler: see `math::ruler_b9`.
pub fn random(id: &str, p: &mut Params, seed: u64) -> bool {
    let u = unit_random(seed);
    match id {
        "temperature-reading" => {
            let (lo, hi, dp) = if p.s("thermometer") == "Digital probe" {
                let (lo, hi) = probe_range(p);
                (lo, hi, probe_dp(p))
            } else {
                let (lo, hi) = temp_scale(p);
                (lo, hi, 1)
            };
            p.set_f("temperature", pick(lo, hi, dp, u));
        }
        "gas-syringe" => {
            let max = if p.s("size").starts_with("50") { 50.0 } else { 100.0 };
            p.set_f("volume", pick(0.1, max, 1, u));
        }
        "volume-by-displacement" => {
            let max = size_ml(p.s("size"));
            let (n0, m0) = cyl_marks(max);
            let (_, minor, _) = marks_from(p, max, n0, m0);
            let dp = read_dp(minor);
            let q = 10f64.powi(dp as i32);
            let before = pick(0.4 * max, 0.7 * max, dp, u);
            let dv_lo = (0.05 * max).max(3.0 * minor);
            let dv_hi = (0.25 * max).min(0.95 * max - before).max(dv_lo.min(0.25 * max));
            let dv = pick(dv_lo.min(dv_hi), dv_hi, dp, unit_random(seed ^ 0xd15b_1ace_0b9e_c7a1)).max(1.0 / q);
            p.set_f("before", before);
            p.set_f("after", ((before + dv) * q).round() / q);
        }
        "volume-reading" if beaker_capacity(p).is_some() => {
            p.set_f("reading", 5.0 * (1.0 + (u * 100.0).floor()).min(100.0));
        }
        "ruler-reading" => return crate::figures::math::ruler_b9::random(p, seed),
        _ => return false,
    }
    true
}

/// The unit after the reading's text box (beside its Random button) for the figures this batch
/// gives one; None for the others, which keep theirs.
pub fn reading_unit(fig: &str, p: &Params) -> Option<String> {
    let or = |u: &str, d: &str| if u.is_empty() { d.to_string() } else { u.to_string() };
    Some(match fig {
        "temperature-reading" => temp_sym(p).to_string(),
        "gas-syringe" => or(p.s("unit"), "cm³"),
        "volume-by-displacement" => or(p.s("unit"), "mL"),
        "ruler-reading" => crate::figures::math::ruler_b9::unit(p).to_string(),
        _ => return None,
    })
}

/// The Random button's tip where it sets more than one value.
pub fn random_tip(fig: &str) -> &'static str {
    match fig {
        "volume-by-displacement" => "Set random before and after readings",
        "ruler-reading" => "Set a random length and position",
        _ => "Set a random reading",
    }
}

#[cfg(test)]
#[path = "readings_b9_tests.rs"]
mod tests;
