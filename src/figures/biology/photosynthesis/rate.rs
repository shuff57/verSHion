//! The photosynthesis figure's second view, "Rate graphs": a generator of the textbook rate-of-photosynthesis graphs,
//! drawn instead of the chloroplast when the View is set to it.
//!
//! Graphs: the rate against light intensity (or against the distance from the lamp), against CO2 concentration or
//! against temperature, with up to three curves, each at its own CO2 concentration, temperature or light intensity;
//! the CO2 in a closed chamber over time (one curve per light level, the light can go off part way); and the CO2 and
//! O2 around one plant, over time or against each other.
//!
//! The model is a textbook one, original here, with each constant's source beside it:
//! * light: a non-rectangular hyperbola (Thornley 1976), the gross rate `P` the smaller root of
//!   `THETA P^2 - (ALPHA I + Pmax) P + ALPHA I Pmax = 0`: it rises with slope `ALPHA` in dim light and levels off at
//!   `Pmax`, the rate CO2 and temperature allow (Blackman's law of limiting factors, 1905);
//! * CO2: `Pmax` is a rectangular hyperbola in CO2 (`C / (KC + C)`), the shape of the Rubisco-limited rate
//!   (Farquhar, von Caemmerer and Berry 1980);
//! * temperature: `Pmax` doubles every 10 degrees C (Q10 = 2) while the enzymes are intact and falls as they denature
//!   (a logistic loss of active enzyme), so it rises to an optimum near 30 degrees C and then falls steeply;
//! * respiration doubles every 10 degrees C (Q10 = 2; Atkin and Tjoelker 2003) and goes on in the light and in the
//!   dark. The net rate is the gross rate less respiration: it is below zero in dim light and at low CO2, and zero at
//!   the compensation points;
//! * light intensity falls as `1/d^2` with the distance `d` from a small lamp (the inverse-square law);
//! * in a sealed chamber the CO2 changes at a rate proportional to the plant's net exchange: in the light it falls
//!   towards the CO2 compensation point (where photosynthesis just balances respiration) and levels off there; in the
//!   dark it rises steadily. O2 changes by the same amount the other way (a photosynthetic and a respiratory
//!   quotient of 1 for carbohydrate).
//!
//! Everything is in arbitrary units except CO2 (% of the air or ppm), O2 (% of the air), temperature, distance and
//! time. Numbers are textbook shapes, not data from one species; the FAQ says so.

use super::*;

pub const VIEWS: [&str; 2] = ["Chloroplast", "Rate graphs"];
pub const GRAPHS: [&str; 6] = ["Rate against light", "Rate against CO2", "Rate against temperature", "CO2 in a closed chamber", "CO2 and O2 over time", "CO2 against O2"];
pub const LIGHT_X: [&str; 2] = ["Light intensity", "Distance from the lamp"];
pub const RATES: [&str; 2] = ["Gross", "Net"];
pub const UNITS: [&str; 2] = ["Arbitrary units", "Bubbles a minute"];
pub const CO2_UNITS: [&str; 2] = ["%", "ppm"];
pub const MARKS: [&str; 6] = ["Nothing", "Saturation point", "Limiting factors", "Lettered points", "Optimum", "Compensation point"];
pub const MARK_LABS: [&str; 3] = ["Labelled", "Blank line", "No label"];
pub const POINTS: [&str; 3] = ["None", "On the curves", "Points only"];
pub const TITLE_MODES: [&str; 3] = ["Text", "Blank line", "None"];
pub const CHART_TITLE_MODES: [&str; 3] = ["None", "Text", "Blank line"];
pub const AXES_MODES: [&str; 2] = ["Numbered", "Shape only"];

const K_CO2: [&str; 3] = ["c1_co2", "c2_co2", "c3_co2"];
const K_TEMP: [&str; 3] = ["c1_temp", "c2_temp", "c3_temp"];
const K_LIGHT: [&str; 3] = ["c1_light", "c2_light", "c3_light"];
const K_NAME: [&str; 3] = ["c1_name", "c2_name", "c3_name"];
/// The three curves of the classic limiting-factor graph (0.1 % CO2 at 25 and 15 degrees C, 0.04 % at 25), each with a
/// light intensity for the graphs that hold light fixed (bright, medium, dim).
const D_CO2: [f64; 3] = [0.1, 0.1, 0.04];
const D_TEMP: [f64; 3] = [25.0, 15.0, 25.0];
const D_LIGHT: [f64; 3] = [80.0, 30.0, 10.0];

pub fn is_rate(p: &Params) -> bool { p.s("view") == VIEWS[1] }

pub fn view_param() -> ParamDef {
    ParamDef::choice("view", "View", &VIEWS, VIEWS[0]).hint("Chloroplast: where the stages happen, with an optional single rate graph. Rate graphs: the textbook rate-of-photosynthesis graphs, up to three curves, the closed chamber and the gases around a plant")
}

pub fn params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("rg_graph", "Graph", &GRAPHS, GRAPHS[0]).hint("Textbook shapes, not data. The rate rises with light and levels off at a height set by CO2 and temperature (highest near 30 \u{b0}C). Respiration doubles every 10 \u{b0}C"),
        ParamDef::choice("rg_light_x", "Along the x-axis", &LIGHT_X, LIGHT_X[0]).hint("Distance from the lamp: light intensity falls as 1/d\u{b2}, so twice as far gives a quarter of the light"),
        ParamDef::choice("rg_rate", "Rate", &RATES, RATES[0]).hint("Net is photosynthesis less respiration: below zero in dim light, zero at the compensation point"),
        ParamDef::choice("rg_unit", "Measured in", &UNITS, UNITS[0]).hint("Bubbles of oxygen a minute from pondweed: a count, never below zero"),
        ParamDef::choice("rg_co2_unit", "CO2 in", &CO2_UNITS, CO2_UNITS[0]).hint("0.04 % is 400 ppm, about the CO2 in air"),
        ParamDef::int("rg_curves", "Curves", 1, 3, 3),
    ];
    for i in 0..3 {
        v.push(ParamDef::float(K_CO2[i], "CO2 (%)", 0.01, 1.0, 0.01, D_CO2[i]).hint("Air is about 0.04 %. Pondweed experiments add sodium hydrogencarbonate for more"));
        v.push(ParamDef::float(K_TEMP[i], "Temperature (\u{b0}C)", 0.0, 50.0, 1.0, D_TEMP[i]));
        v.push(ParamDef::float(K_LIGHT[i], "Light intensity", 0.0, 100.0, 1.0, D_LIGHT[i]).hint("Arbitrary units: 0 is dark, 100 a bright lamp close to the plant"));
        v.push(ParamDef::text(K_NAME[i], "Name in the key", "").hint("Leave empty to name the curve by its settings"));
    }
    v.extend([
        ParamDef::float("rg_c0", "CO2 at the start (%)", 0.01, 1.0, 0.01, 0.04).hint("Air is about 0.04 %"),
        ParamDef::float("rg_tmax", "Time shown (minutes)", 10.0, 240.0, 5.0, 60.0),
        ParamDef::bool("rg_light_off", "The light goes off part way", false).hint("The dark part is shaded"),
        ParamDef::float("rg_off_at", "Light goes off at (minutes)", 1.0, 240.0, 1.0, 30.0),
        ParamDef::choice("rg_mark", "Marked", &MARKS, MARKS[0]).hint("Lettered points A, B and C are for questions; the others are the textbook features of each graph"),
        ParamDef::choice("rg_mark_lab", "Mark's label", &MARK_LABS, MARK_LABS[0]).hint("A blank line is for the student to write on. The student copy turns every label into a blank line"),
        ParamDef::choice("rg_points", "Data points", &POINTS, POINTS[0]).hint("Points read off the curves and scattered a little, as if measured; Points only leaves the line for the student to draw"),
        ParamDef::int("rg_seed", "Scatter (another set)", 1, 999, 1),
        ParamDef::choice("rg_title_mode", "Chart title", &CHART_TITLE_MODES, CHART_TITLE_MODES[0]),
        ParamDef::text("rg_title", "Chart title text", "").hint("Leave empty for a title that says what is plotted"),
        ParamDef::choice("rg_xt_mode", "x-axis title", &TITLE_MODES, TITLE_MODES[0]),
        ParamDef::text("rg_xt", "x-axis title text", "").hint("Leave empty for the quantity and its unit"),
        ParamDef::choice("rg_yt_mode", "y-axis title", &TITLE_MODES, TITLE_MODES[0]),
        ParamDef::text("rg_yt", "y-axis title text", "").hint("Leave empty for the quantity and its unit"),
        ParamDef::choice("rg_axes", "Axes", &AXES_MODES, AXES_MODES[0]).hint("Shape only: no numbers, for describing a trend"),
        ParamDef::text("rg_xmax", "x-axis right end", "").hint("Leave empty to fit the graph. In the axis's own unit (ppm when CO2 is in ppm)"),
        ParamDef::text("rg_ymax", "y-axis top end", "").hint("Leave empty to fit the graph"),
        ParamDef::bool("rg_grid", "Gridlines", true),
    ]);
    v
}

/// Whether a control applies: the chloroplast's controls in the Chloroplast view, the graph's in the Rate graphs
/// view, and of those only the ones the chosen graph uses.
pub fn relevant(key: &str, p: &Params) -> bool {
    if matches!(key, "view" | "show_answers" | "style" | "label_size") { return true; }
    let rate = is_rate(p);
    if matches!(key, "show_parts" | "show_stages" | "show_io" | "show_shuttle" | "show_equation" | "show_graph" | "graph_x" | "pmax" | "xval" | "mark" | "dec") { return !rate; }
    if !rate { return false; }
    let g = graph_of(p);
    let rategraph = matches!(g, G::Light | G::Distance | G::Co2 | G::Temp);
    let gas = matches!(g, G::GasTime | G::GasXy);
    match key {
        "rg_light_x" => matches!(g, G::Light | G::Distance),
        "rg_rate" | "rg_unit" => rategraph,
        "rg_curves" => !gas,
        "rg_c0" | "rg_tmax" | "rg_light_off" => !rategraph,
        "rg_off_at" => !rategraph && p.b("rg_light_off"),
        "rg_mark_lab" => !matches!(p.s("rg_mark"), "Nothing" | "Lettered points"),
        "rg_seed" => p.s("rg_points") != POINTS[0],
        "rg_title" => p.s("rg_title_mode") == "Text",
        "rg_xt" => p.s("rg_xt_mode") == "Text",
        "rg_yt" => p.s("rg_yt_mode") == "Text",
        _ => match curve_key(key) {
            Some((i, what)) => {
                let n = if gas { 1 } else { p.i("rg_curves").clamp(1, 3) as usize };
                i < n && match what {
                    "co2" => matches!(g, G::Light | G::Distance | G::Temp),
                    "temp" => g != G::Temp,
                    "light" => matches!(g, G::Co2 | G::Temp | G::Chamber | G::GasTime | G::GasXy),
                    "name" => !gas,
                    _ => false,
                }
            }
            None => true,
        },
    }
}

/// `c2_light` -> (1, "light").
fn curve_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix('c')?;
    let (n, what) = rest.split_once('_')?;
    let i: usize = n.parse().ok()?;
    (1..=3).contains(&i).then_some((i - 1, what))
}

/// The six classic setups (the panel's Classic setups list), over the plain defaults.
pub fn classics() -> Vec<Example> {
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let i = |k: &'static str, v: i64| (k, Value::I(v));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    let rv = || s("view", VIEWS[1]);
    vec![
        Example { name: "Limiting factors: light, CO2 and temperature", params: vec![rv(), s("rg_graph", GRAPHS[0]), i("rg_curves", 3), f("c1_co2", 0.1), f("c1_temp", 25.0), f("c2_co2", 0.1), f("c2_temp", 15.0), f("c3_co2", 0.04), f("c3_temp", 25.0), s("rg_mark", "Limiting factors")] },
        Example { name: "Pondweed bubbles and lamp distance", params: vec![rv(), s("rg_graph", GRAPHS[0]), s("rg_light_x", LIGHT_X[1]), s("rg_unit", UNITS[1]), s("rg_rate", RATES[1]), i("rg_curves", 1), f("c1_co2", 0.1), f("c1_temp", 20.0), s("rg_points", POINTS[1])] },
        Example { name: "Rate against CO2 in bright and dim light", params: vec![rv(), s("rg_graph", GRAPHS[1]), i("rg_curves", 2), f("c1_light", 50.0), f("c1_temp", 25.0), f("c2_light", 20.0), f("c2_temp", 25.0), s("rg_mark", "Saturation point")] },
        Example { name: "Rate against temperature in bright and dim light", params: vec![rv(), s("rg_graph", GRAPHS[2]), i("rg_curves", 2), f("c1_light", 80.0), f("c1_co2", 0.1), f("c2_light", 15.0), f("c2_co2", 0.1), s("rg_mark", "Optimum")] },
        Example { name: "Closed chamber: dark, dim and bright light", params: vec![rv(), s("rg_graph", GRAPHS[3]), i("rg_curves", 3), f("c1_light", 0.0), f("c1_temp", 25.0), f("c2_light", 10.0), f("c2_temp", 25.0), f("c3_light", 80.0), f("c3_temp", 25.0), f("rg_c0", 0.04), f("rg_tmax", 60.0), s("rg_mark", "Compensation point")] },
        Example { name: "CO2 and O2: light, then dark", params: vec![rv(), s("rg_graph", GRAPHS[4]), f("c1_light", 80.0), f("c1_temp", 25.0), f("rg_c0", 0.04), f("rg_tmax", 60.0), b("rg_light_off", true), f("rg_off_at", 30.0)] },
    ]
}

// ================================================================== the model

/// Initial slope of the light response, arbitrary rate units per unit of light: the light-limited rate, which hardly
/// depends on temperature because the photochemistry does not (Berry and Bjorkman 1980). Set so that the bend of the
/// classic graph falls inside a 0 to 100 light axis.
pub const ALPHA: f64 = 0.2;
/// Curvature of the non-rectangular hyperbola (Thornley 1976). Leaves measure 0.7 to 0.95 (Ogren and Evans 1993); a
/// textbook graph draws a sharper bend between the light-limited rise and the plateau, so 0.98.
pub const THETA: f64 = 0.98;
/// The light-saturated gross rate with CO2 saturating and at the optimum temperature (arbitrary units).
pub const PM: f64 = 20.0;
/// CO2 (% of the air) that gives half the CO2-saturated rate: the Rubisco-limited rate is a hyperbola in CO2
/// (Farquhar, von Caemmerer and Berry 1980). 0.04 % is Rubisco's Kc for CO2 at 25 degrees C, about 405 umol/mol
/// (von Caemmerer 2000), and puts air on the steep part of the curve, as textbooks draw it.
pub const KC: f64 = 0.04;
/// The CO2 (% of the air) at which Rubisco's carboxylation just balances the CO2 photorespiration gives off
/// (Gamma*, about 43 umol/mol at 25 degrees C; Bernacchi et al. 2001): below it there is no net uptake of CO2 however
/// bright the light. With respiration on top, the CO2 compensation point of the whole plant is a little higher,
/// about 0.007 % (70 ppm) at 25 degrees C, inside the 40 to 100 ppm measured for C3 plants.
pub const GSTAR: f64 = 0.0043;
/// Q10 of the enzyme-controlled reactions and of respiration: the rate doubles every 10 degrees C.
pub const Q10: f64 = 2.0;
/// Temperature (degrees C) at which half the enzyme is denatured, and the width of the change. Photosynthesis of most
/// C3 plants falls steeply above about 35 degrees C (Berry and Bjorkman 1980; Yamori et al. 2014): with these, the
/// optimum is at about 30 degrees C and the rate is down to a fifth by 40.
pub const T_DENAT: f64 = 34.0;
pub const T_WIDTH: f64 = 2.5;
/// Respiration at 25 degrees C, about a twentieth of the largest gross rate (leaf respiration is typically 5 to 10 %
/// of light-saturated photosynthesis; Atkin and Tjoelker 2003).
pub const R25: f64 = 1.0;
/// The lamp: light intensity 100 at 10 cm, falling as 1/d^2 (inverse-square law for a small source).
pub const LAMP_K: f64 = 100.0 * 10.0 * 10.0;
/// The distance graph starts this close to the lamp (cm).
pub const DIST_MIN: f64 = 2.0;
/// Bubbles of oxygen a minute per arbitrary unit of rate (a bubble is a fixed small volume of gas).
pub const BUBBLES_PER_AU: f64 = 3.0;
/// Change of the chamber's CO2 (% of the air) a minute per arbitrary unit of net exchange: a small chamber, so that
/// bright light draws the CO2 down to the compensation point in about half an hour.
pub const K_CH: f64 = 0.0006;
/// O2 in air, % by volume (20.95 %, rounded as textbooks give it).
pub const O2_AIR: f64 = 21.0;

/// The light-limited rate `i` (= ALPHA x light) combined with the light-saturated rate `pmax`: the smaller root of
/// the non-rectangular hyperbola. Zero when either is zero; never above either.
pub fn nrh(i: f64, pmax: f64) -> f64 {
    if i <= 0.0 || pmax <= 0.0 { return 0.0; }
    let s = i + pmax;
    let disc = (s * s - 4.0 * THETA * i * pmax).max(0.0);
    // the stable form of the smaller root, (s - sqrt(disc)) / (2 THETA) = 2 i pmax / (s + sqrt(disc))
    2.0 * i * pmax / (s + disc.sqrt())
}

/// Fraction of the enzyme still active at `t` degrees C (a logistic fall around T_DENAT).
fn active(t: f64) -> f64 { 1.0 / (1.0 + ((t - T_DENAT) / T_WIDTH).exp()) }

fn temp_raw(t: f64) -> f64 { Q10.powf((t - 30.0) / 10.0) * active(t) }

/// The temperature at which the enzyme-limited rate is highest. ln f = (t - 30) ln Q10 / 10 + ln a(t), and
/// d ln a / dt = -(1 - a) / T_WIDTH, so the peak is where 1 - a = T_WIDTH ln Q10 / 10.
pub fn t_opt() -> f64 {
    let a = 1.0 - T_WIDTH * Q10.ln() / 10.0;
    T_DENAT + T_WIDTH * (1.0 / a - 1.0).ln()
}

/// The light-saturated rate's dependence on temperature, 1 at the optimum.
pub fn temp_factor(t: f64) -> f64 { temp_raw(t) / temp_raw(t_opt()) }

/// The light-saturated rate's dependence on CO2 (% of the air): zero up to GSTAR, approaching 1 at high CO2
/// (Farquhar et al. 1980: (C - Gamma*) / (C + Kc)).
pub fn co2_factor(c: f64) -> f64 { co2_factor_signed(c).max(0.0) }

/// The same, below zero under GSTAR (photorespiration gives off more CO2 than is fixed).
pub fn co2_factor_signed(c: f64) -> f64 { let c = c.max(0.0); (c - GSTAR) / (KC + c) }

/// The light-saturated gross rate CO2 and temperature allow.
pub fn pmax(co2: f64, t: f64) -> f64 { PM * temp_factor(t) * co2_factor(co2) }

/// Respiration at `t` degrees C (arbitrary units), doubling every 10 degrees.
pub fn resp(t: f64) -> f64 { R25 * Q10.powf((t - 25.0) / 10.0) }

/// CO2 fixed less CO2 given off by photorespiration, at a light intensity, CO2 (%) and temperature. Below GSTAR the
/// photorespiration wins and this is below zero (it then grows with light as the hyperbola's rate does near zero,
/// and is zero in the dark), so the net rate runs smoothly through the compensation point.
pub fn assim(light: f64, co2: f64, t: f64) -> f64 {
    let pm = PM * temp_factor(t) * co2_factor_signed(co2);
    let i = ALPHA * light;
    if pm >= 0.0 { nrh(i, pm) } else if i <= 0.0 { 0.0 } else { pm * i / (i + pm.abs()) }
}

/// Gross rate of photosynthesis at a light intensity, CO2 (%) and temperature: never below zero.
pub fn gross(light: f64, co2: f64, t: f64) -> f64 { assim(light, co2, t).max(0.0) }

/// Net rate: the CO2 fixed less photorespiration and respiration (equal to gross less respiration wherever CO2 is
/// above GSTAR, which is everywhere on the graphs but the first 43 ppm).
pub fn net(light: f64, co2: f64, t: f64) -> f64 { assim(light, co2, t) - resp(t) }

/// Light intensity `d` cm from the lamp.
pub fn light_at(d: f64) -> f64 { LAMP_K / (d * d).max(1e-9) }

/// Solves `f(x) = 0` for an increasing `f` on [lo, hi] by bisection; None when there is no sign change.
fn root(lo: f64, hi: f64, f: impl Fn(f64) -> f64) -> Option<f64> {
    let (mut a, mut b) = (lo, hi);
    let (fa, fb) = (f(a), f(b));
    if fa.is_nan() || fb.is_nan() || fa.signum() == fb.signum() && fa != 0.0 && fb != 0.0 { return None; }
    if fa == 0.0 { return Some(a); }
    let up = fa < 0.0;
    for _ in 0..200 {
        let m = 0.5 * (a + b);
        let fm = f(m);
        if (fm < 0.0) == up { a = m; } else { b = m; }
    }
    Some(0.5 * (a + b))
}

/// The light compensation point: the light intensity at which gross photosynthesis equals respiration.
pub fn light_comp(co2: f64, t: f64) -> Option<f64> { root(0.0, 1e5, |l| net(l, co2, t)) }

/// The CO2 compensation point (%): the CO2 at which gross photosynthesis equals respiration at this light.
pub fn co2_comp(light: f64, t: f64) -> Option<f64> { root(0.0, 20.0, |c| net(light, c, t)) }

/// The high-temperature compensation point: above the optimum, where respiration catches up with photosynthesis.
pub fn temp_comp(light: f64, co2: f64) -> Option<f64> { root(t_opt(), 80.0, |t| -net(light, co2, t)) }

/// The CO2 (%) in a sealed chamber over `tmax` minutes, starting at `c0`, the light off from `off` on: Runge-Kutta
/// steps of dC/dt = -K_CH net(light, C, T), with a step boundary exactly where the light goes off.
pub fn chamber_series(c: &Cond, c0: f64, tmax: f64, off: Option<f64>, steps: usize) -> Vec<P> {
    let mut out = vec![(0.0, c0)];
    let mut co2 = c0;
    let mut t0 = 0.0;
    let cut = off.filter(|o| *o > 0.0 && *o < tmax);
    let parts: Vec<(f64, f64, f64)> = match cut { Some(o) => vec![(0.0, o, c.light), (o, tmax, 0.0)], None => vec![(0.0, tmax, if off.is_some_and(|o| o <= 0.0) { 0.0 } else { c.light })] };
    for (a, b, light) in parts {
        let n = ((steps as f64 * (b - a) / tmax).ceil() as usize).max(1);
        let dt = (b - a) / n as f64;
        let f = |x: f64| -K_CH * net(light, x.max(0.0), c.temp);
        for _ in 0..n {
            let k1 = f(co2);
            let k2 = f(co2 + 0.5 * dt * k1);
            let k3 = f(co2 + 0.5 * dt * k2);
            let k4 = f(co2 + dt * k3);
            co2 = (co2 + dt / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4)).max(0.0);
            t0 += dt;
            out.push((t0, co2));
        }
        t0 = b;
    }
    out
}

// ================================================================== settings

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum G { Light, Distance, Co2, Temp, Chamber, GasTime, GasXy }

pub fn graph_of(p: &Params) -> G {
    let g = p.s("rg_graph");
    if g == GRAPHS[1] { G::Co2 } else if g == GRAPHS[2] { G::Temp } else if g == GRAPHS[3] { G::Chamber } else if g == GRAPHS[4] { G::GasTime } else if g == GRAPHS[5] { G::GasXy } else if p.s("rg_light_x") == LIGHT_X[1] { G::Distance } else { G::Light }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cond { pub co2: f64, pub temp: f64, pub light: f64 }

pub fn cond(p: &Params, i: usize) -> Cond {
    let f = |k: &str, lo: f64, hi: f64, d: f64| { let v = p.f(k); if v.is_finite() { v.clamp(lo, hi) } else { d } };
    Cond { co2: f(K_CO2[i], 0.01, 1.0, D_CO2[i]), temp: f(K_TEMP[i], 0.0, 50.0, D_TEMP[i]), light: f(K_LIGHT[i], 0.0, 100.0, D_LIGHT[i]) }
}

pub struct S {
    pub g: G,
    pub net: bool,
    pub bubbles: bool,
    pub ppm: bool,
    pub conds: Vec<Cond>,
    pub typed_names: Vec<String>,
    pub c0: f64,
    pub tmax: f64,
    pub off: Option<f64>,
    pub mark: String,
    pub mark_lab: String,
    pub points: String,
    pub seed: u64,
    pub key: bool,
    pub lab: f64,
    pub print: bool,
    pub shape_only: bool,
    pub grid: bool,
}

pub fn settings(p: &Params) -> S {
    let g = graph_of(p);
    let n = if matches!(g, G::GasTime | G::GasXy) { 1 } else { p.i("rg_curves").clamp(1, 3) as usize };
    let fin = |k: &str, lo: f64, hi: f64, d: f64| { let v = p.f(k); if v.is_finite() { v.clamp(lo, hi) } else { d } };
    S {
        g,
        net: p.s("rg_rate") == RATES[1],
        bubbles: p.s("rg_unit") == UNITS[1],
        ppm: p.s("rg_co2_unit") == CO2_UNITS[1],
        conds: (0..n).map(|i| cond(p, i)).collect(),
        typed_names: (0..n).map(|i| p.s(K_NAME[i]).split_whitespace().collect::<Vec<_>>().join(" ")).collect(),
        c0: fin("rg_c0", 0.01, 1.0, 0.04),
        tmax: fin("rg_tmax", 10.0, 240.0, 60.0),
        off: p.b("rg_light_off").then(|| fin("rg_off_at", 1.0, 240.0, 30.0)),
        mark: p.s("rg_mark").to_string(),
        mark_lab: p.s("rg_mark_lab").to_string(),
        points: p.s("rg_points").to_string(),
        seed: p.i("rg_seed").clamp(1, 999) as u64,
        key: p.b(SHOW_ANSWERS),
        lab: svg::label_factor(p.s("label_size")),
        print: p.s("style") == "Print",
        shape_only: p.s("rg_axes") == AXES_MODES[1],
        grid: p.b("rg_grid"),
    }
}

impl S {
    fn rate_graph(&self) -> bool { matches!(self.g, G::Light | G::Distance | G::Co2 | G::Temp) }
    /// Converts a CO2 value in % to the unit shown.
    fn co2_shown(&self, c: f64) -> f64 { if self.ppm { c * 1e4 } else { c } }
    fn co2_text(&self, c: f64) -> String { if self.ppm { format!("{} ppm", num(round_to(c * 1e4, 1.0))) } else { format!("{} %", num(c)) } }
    fn co2_unit(&self) -> &'static str { if self.ppm { "ppm" } else { "%" } }
    fn rate_unit(&self) -> &'static str { if self.bubbles { "bubbles a minute" } else { "arbitrary units" } }
    /// The rate as drawn: gross or net, in arbitrary units or bubbles (never below zero: a count).
    fn shown_rate(&self, au: f64) -> f64 { if self.bubbles { (au * BUBBLES_PER_AU).max(0.0) } else { au } }
    /// The conditions at x (in the model's units) for curve `c` on a rate graph: (light, CO2 %, temperature).
    fn at(&self, c: &Cond, x: f64) -> (f64, f64, f64) {
        match self.g {
            G::Light => (x, c.co2, c.temp),
            G::Distance => (light_at(x), c.co2, c.temp),
            G::Co2 => (c.light, x, c.temp),
            _ => (c.light, c.co2, x),
        }
    }
    /// The rate drawn at x for curve `c`.
    pub fn rate_at(&self, c: &Cond, x: f64) -> f64 {
        let (l, co2, t) = self.at(c, x);
        let r = if self.net { net(l, co2, t) } else { gross(l, co2, t) };
        self.shown_rate(r)
    }
    /// The x axis in display units when not typed (x from, x to).
    fn x_default(&self) -> (f64, f64) {
        match self.g {
            G::Light => (0.0, 100.0),
            G::Distance => (0.0, 50.0),
            G::Co2 => (0.0, self.co2_shown(0.4)),
            G::Temp => (0.0, 50.0),
            G::Chamber | G::GasTime => (0.0, self.tmax),
            G::GasXy => (0.0, 0.0),
        }
    }
    /// A display x back to the model's unit.
    fn x_model(&self, x: f64) -> f64 { if matches!(self.g, G::Co2 | G::GasXy) && self.ppm { x / 1e4 } else { x } }
    fn x_shown(&self, x: f64) -> f64 { if matches!(self.g, G::Co2 | G::GasXy) { self.co2_shown(x) } else { x } }
    /// The name of curve i in the key: the typed one, or its settings.
    pub fn name(&self, i: usize) -> String {
        if let Some(t) = self.typed_names.get(i).filter(|t| !t.is_empty()) { return t.clone(); }
        let c = &self.conds[i];
        let co2 = format!("{} CO~2", self.co2_text(c.co2));
        let temp = format!("{} \u{b0}C", num(c.temp));
        let light = format!("light {}", num(c.light));
        match self.g {
            G::Light | G::Distance => format!("{co2}, {temp}"),
            G::Co2 => format!("{}, {temp}", cap(&light)),
            G::Temp => format!("{}, {co2}", cap(&light)),
            _ => if c.light <= 0.0 { format!("Dark, {temp}") } else { format!("{}, {temp}", cap(&light)) },
        }
    }
}

fn cap(s: &str) -> String { let mut c = s.chars(); match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() } }
fn num(v: f64) -> String { crate::figures::chart::fmt(v) }
fn round_to(v: f64, step: f64) -> f64 { (v / step).round() * step }

/// Two significant figures (41.2 -> 41, 9.83 -> 9.8, 0.00615 -> 0.0062), as text.
pub fn sig2(v: f64) -> String {
    if !v.is_finite() { return "?".into(); }
    if v == 0.0 { return "0".into(); }
    let m = 10f64.powf(v.abs().log10().floor() - 1.0);
    num((v / m).round() * m)
}

// ================================================================== the features of each graph

/// The plateau a rate graph levels off at (gross, arbitrary units), for the light, distance and CO2 graphs.
pub fn plateau(s: &S, c: &Cond) -> f64 {
    match s.g {
        G::Co2 => nrh(ALPHA * c.light, PM * temp_factor(c.temp)),
        _ => pmax(c.co2, c.temp),
    }
}

/// The share of its plateau at which a curve counts as levelled off: the saturation point.
pub const SAT_FRAC: f64 = 0.95;

/// The saturation point in the model's x unit: where the gross rate reaches SAT_FRAC of the plateau it levels off
/// at, so that past it the factor on the x axis hardly raises the rate and another factor limits it. Light: solved
/// from the hyperbola; CO2: by bisection; distance: the distance that gives the saturating light.
pub fn sat_x(s: &S, c: &Cond) -> Option<f64> {
    let pl = plateau(s, c);
    if pl <= 0.0 { return None; }
    let target = SAT_FRAC * pl;
    match s.g {
        G::Light => root(0.0, 1e5, |l| gross(l, c.co2, c.temp) - target),
        G::Distance => root(0.0, 1e5, |l| gross(l, c.co2, c.temp) - target).filter(|l| *l > 0.0).map(|l| (LAMP_K / l).sqrt()),
        G::Co2 => root(0.0, 50.0, |x| gross(c.light, x, c.temp) - target),
        _ => None,
    }
}

/// The compensation point in the model's x unit on a rate graph: where gross photosynthesis equals respiration.
pub fn comp_x(s: &S, c: &Cond) -> Option<f64> {
    match s.g {
        G::Light => light_comp(c.co2, c.temp),
        G::Distance => light_comp(c.co2, c.temp).filter(|l| *l > 0.0).map(|l| (LAMP_K / l).sqrt()),
        G::Co2 => co2_comp(c.light, c.temp),
        G::Temp => temp_comp(c.light, c.co2),
        _ => None,
    }
}

/// The temperature at which curve `c`'s drawn rate is highest (to 0.01 degrees C).
pub fn opt_x(s: &S, c: &Cond) -> f64 {
    let mut best = (0.0, f64::NEG_INFINITY);
    for k in 0..=5000 { let t = k as f64 * 0.01; let r = s.rate_at(c, t); if r > best.1 + 1e-12 { best = (t, r); } }
    best.0
}

/// Which factor limits the plateau of curve `c`, as a label.
fn plateau_limit(s: &S, c: &Cond) -> &'static str {
    match s.g {
        G::Light | G::Distance => {
            let (g, f) = (co2_factor(c.co2), temp_factor(c.temp));
            // two factors that are about as far from their best both limit (raising either raises the plateau)
            if g.min(f) > 0.88 * g.max(f) { "CO~2 and temperature limiting" } else if g < f { "CO~2 limiting" } else { "Temperature limiting" }
        }
        G::Co2 => if ALPHA * c.light < PM * temp_factor(c.temp) { "Light limiting" } else { "Temperature limiting" },
        _ => "",
    }
}

fn plateau_limit_words(s: &S, c: &Cond) -> &'static str {
    match plateau_limit(s, c) { "CO~2 limiting" => "CO~2 concentration", "Light limiting" => "light intensity", "CO~2 and temperature limiting" => "CO~2 concentration and temperature", _ => "temperature" }
}

// ================================================================== drawing

const W: f64 = 680.0;
const PLOT_H: f64 = 300.0;
const GRID_C: &str = "#d3d8df";
const GRID_P: &str = "#c8c8c8";
const GUIDE: &str = "#444444";
const SHADE: &str = "#e4e4e4";

struct Ax { lo: f64, hi: f64, step: f64 }

impl Ax {
    fn ticks(&self) -> Vec<f64> {
        let mut v = Vec::new();
        let mut k = (self.lo / self.step - 1e-9).ceil();
        while k * self.step <= self.hi + self.step * 1e-6 && v.len() < 60 { v.push((k * self.step * 1e9).round() / 1e9 + 0.0); k += 1.0; }
        v
    }
}

/// A nice axis holding [lo, hi]: the step from about six intervals, the ends on steps, with a little room above.
fn nice(lo: f64, hi: f64, zero: bool) -> Ax {
    let (lo, hi) = if zero { (lo.min(0.0), hi.max(0.0)) } else { (lo, hi) };
    let (lo, hi) = if !(hi - lo).is_finite() || hi - lo < 1e-12 { if hi.abs() < 1e-12 { (0.0, 1.0) } else { (lo - hi.abs() * 0.1, hi + hi.abs() * 0.1) } } else { (lo, hi) };
    let step = crate::figures::axes::nice_step((hi - lo) * 1.06, 6.0);
    let top = ((hi + (hi - lo) * 0.04) / step - 1e-9).ceil() * step;
    let bot = if zero && lo >= -1e-12 { 0.0 } else { ((lo - (hi - lo) * 0.04) / step + 1e-9).floor() * step };
    Ax { lo: bot, hi: top.max(bot + step), step }
}

/// A typed axis end: a number (a trailing unit is ignored), or None for fitted. Err with the reason when it is not usable.
fn typed_end(raw: &str, what: &str, lo: f64) -> Result<Option<f64>, String> {
    let t = raw.trim();
    if t.is_empty() { return Ok(None); }
    let digits: String = t.trim_end_matches(|c: char| c.is_alphabetic() || c == '%' || c == '\u{b0}' || c.is_whitespace()).replace('\u{2212}', "-");
    match digits.parse::<f64>() {
        Ok(v) if v.is_finite() && v > lo && v < 1e7 => Ok(Some(v)),
        Ok(v) if v.is_finite() => Err(format!("{what}: {} is not above the other end of the axis ({}), so the axis is fitted to the graph.", num(v), num(lo))),
        _ => Err(format!("{what}: \"{}\" is not a number, so the axis is fitted to the graph.", clip(t))),
    }
}

/// A line drawn on the graph, in display units.
struct Line { pts: Vec<P>, idx: usize }

pub struct Out { pub svg: String, pub notes: Vec<String> }

fn ln(s: &mut String, a: P, b: P, col: &str, sw: f64, dash: &str) {
    s.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{col}" stroke-width="{sw}"{}/>"#, a.0, a.1, b.0, b.1, if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) }));
}

/// Whether segments a-b and c-d cross (touching counts).
fn crosses(a: P, b: P, c: P, d: P) -> bool {
    let o = |p: P, q: P, r: P| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    ((d1 > 0.0) != (d2 > 0.0) || d1 == 0.0 || d2 == 0.0) && ((d3 > 0.0) != (d4 > 0.0) || d3 == 0.0 || d4 == 0.0) && !(d1 == 0.0 && d2 == 0.0 && d3 == 0.0 && d4 == 0.0 && {
        // collinear: they cross only if they overlap
        let (lo, hi) = (a.0.min(b.0), a.0.max(b.0));
        !(c.0.max(d.0) >= lo && c.0.min(d.0) <= hi)
    })
}

fn boxes_meet(a: Bx, b: Bx) -> bool { a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3 }

/// Puts a box of `w` x `h` near `anchor`: the first clear spot (inside `area`, clear of every box and segment) on
/// rings of growing radius, trying the `prefer` directions (degrees, screen y down) first.
fn place(anchor: P, w: f64, h: f64, prefer: &[f64], segs: &[(P, P)], boxes: &[Bx], area: Bx) -> Option<Bx> {
    let mut dirs: Vec<f64> = prefer.to_vec();
    for k in 0..16 { let a = k as f64 * 22.5; if !dirs.iter().any(|d| (d - a).abs() < 1e-9) { dirs.push(a); } }
    for rad in [10.0, 15.0, 21.0, 28.0, 37.0, 48.0, 62.0, 80.0] {
        for a in &dirs {
            let (ux, uy) = (a.to_radians().cos(), a.to_radians().sin());
            // the box's nearest edge sits `rad` from the anchor along the direction
            let (cx, cy) = (anchor.0 + ux * (rad + w / 2.0 * ux.abs()), anchor.1 + uy * (rad + h / 2.0 * uy.abs()));
            let bx = (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0);
            if bx.0 < area.0 || bx.2 > area.2 || bx.1 < area.1 || bx.3 > area.3 { continue; }
            let eb = expand(bx, 2.5);
            if boxes.iter().any(|b| boxes_meet(*b, eb)) || segs.iter().any(|(p, q)| seg_hits_box(*p, *q, eb)) { continue; }
            return Some(bx);
        }
    }
    None
}

/// The marker of series `i` for data points: circle, square, triangle.
fn marker(s: &mut String, i: usize, c: P, r: f64, col: &str) {
    match i % 3 {
        0 => s.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="{r:.2}" fill="#ffffff" stroke="{col}" stroke-width="1.8"/>"##, c.0, c.1)),
        1 => s.push_str(&format!(r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="#ffffff" stroke="{col}" stroke-width="1.8"/>"##, c.0 - r * 0.9, c.1 - r * 0.9, 1.8 * r, 1.8 * r)),
        _ => s.push_str(&format!(r##"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="#ffffff" stroke="{col}" stroke-width="1.8" stroke-linejoin="round"/>"##, c.0, c.1 - r * 1.15, c.0 + r * 1.05, c.1 + r * 0.7, c.0 - r * 1.05, c.1 + r * 0.7)),
    }
}

/// Clips a text to `max` px with an ellipsis.
fn fit(t: &str, max: f64, size: f64) -> String {
    if rw(t, size) <= max { return t.to_string(); }
    let mut s: String = t.to_string();
    while !s.is_empty() && rw(&format!("{s}\u{2026}"), size) > max { s.pop(); }
    format!("{}\u{2026}", s.trim_end())
}

/// The automatic titles: (chart, x, y, right y).
fn auto_titles(s: &S) -> (String, String, String, String) {
    let ru = s.rate_unit();
    let rate = if s.net { "Net rate of photosynthesis" } else { "Rate of photosynthesis" };
    let y = format!("{rate} ({ru})");
    let cu = s.co2_unit();
    match s.g {
        G::Light => (format!("{rate} against light intensity"), "Light intensity (arbitrary units)".into(), y, String::new()),
        G::Distance => (format!("{rate} against distance from the lamp"), "Distance from the lamp (cm)".into(), y, String::new()),
        G::Co2 => (format!("{rate} against CO~2 concentration"), format!("CO~2 concentration ({cu})"), y, String::new()),
        G::Temp => (format!("{rate} against temperature"), "Temperature (\u{b0}C)".into(), y, String::new()),
        G::Chamber => ("CO~2 in a closed chamber".into(), "Time (minutes)".into(), format!("CO~2 in the chamber ({cu})"), String::new()),
        G::GasTime => ("CO~2 and O~2 around a plant".into(), "Time (minutes)".into(), format!("CO~2 ({cu})"), "O~2 (%)".into()),
        G::GasXy => ("O~2 against CO~2 around a plant".into(), format!("CO~2 ({cu})"), "O~2 (%)".into(), String::new()),
    }
}

/// The figure in the Rate graphs view, and what it could not draw as asked.
pub fn render(p: &Params) -> Out {
    let s = settings(p);
    let lab = s.lab;
    // a subscript (CO2) is drawn at 0.72 of its text, so no text is smaller than 8 / 0.72 px
    let floor = 8.0 / 0.72 + 0.05;
    let fs = (13.0 * lab).max(floor); // tick numbers, labels, the key
    let ft = (14.0 * lab).max(floor); // axis titles
    let fc = (12.0 * lab).max(floor); // captions
    let mut notes: Vec<String> = Vec::new();
    let fills = svg::Fills::new(p);
    let col = |i: usize| -> &'static str { if s.print { INK } else { fills.line(i) } };
    let dash = |i: usize| -> &'static str { if s.print { ["", "7 4", "2 3"][i % 3] } else { "" } };
    let (auto_chart, auto_x, auto_y, auto_y2) = auto_titles(&s);
    let title_of = |mode: &str, typed: &str, auto: &str| -> Option<Option<String>> {
        match mode { "None" => None, "Blank line" => Some(None), _ => Some(Some(if typed.trim().is_empty() { auto.to_string() } else { typed.split_whitespace().collect::<Vec<_>>().join(" ") })) }
    };
    let chart_t = title_of(p.s("rg_title_mode"), p.s("rg_title"), &auto_chart);
    let x_t = title_of(p.s("rg_xt_mode"), p.s("rg_xt"), &auto_x);
    let y_t = title_of(p.s("rg_yt_mode"), p.s("rg_yt"), &auto_y);
    let y2_t = (s.g == G::GasTime).then(|| auto_y2.clone());

    // ---------- the lines, in display units
    let (dx0, dx1) = s.x_default();
    let xmax_typed = match typed_end(p.s("rg_xmax"), "x-axis right end", if s.g == G::GasXy { f64::NEG_INFINITY } else { dx0 }) { Ok(v) => v, Err(e) => { notes.push(e); None } };
    let mut lines: Vec<Line> = Vec::new();
    let mut extra_y: Vec<f64> = Vec::new(); // values the y axis must hold (the respiration line)
    let mut o2_line: Option<Line> = None;
    let mut path_times: Vec<f64> = Vec::new();
    let x_hi_d = xmax_typed.unwrap_or(dx1);
    if s.rate_graph() {
        let (m0, m1) = (s.x_model(dx0).max(if s.g == G::Distance { DIST_MIN } else { 0.0 }), s.x_model(x_hi_d));
        for (i, c) in s.conds.iter().enumerate() {
            let n = 360;
            let pts = (0..=n).map(|k| { let x = m0 + (m1 - m0) * k as f64 / n as f64; (s.x_shown(x), s.rate_at(c, x)) }).collect();
            lines.push(Line { pts, idx: i });
        }
    } else {
        let steps = 600;
        for (i, c) in s.conds.iter().enumerate() {
            let ser = chamber_series(c, s.c0, s.tmax, s.off, steps);
            match s.g {
                G::GasXy => {
                    path_times = ser.iter().map(|q| q.0).collect();
                    lines.push(Line { pts: ser.iter().map(|q| (s.co2_shown(q.1), O2_AIR + (s.c0 - q.1))).collect(), idx: i });
                }
                _ => {
                    lines.push(Line { pts: ser.iter().map(|q| (q.0, s.co2_shown(q.1))).collect(), idx: i });
                    if s.g == G::GasTime { o2_line = Some(Line { pts: ser.iter().map(|q| (q.0, O2_AIR + (s.c0 - q.1))).collect(), idx: 1 }); }
                }
            }
        }
        if let Some(o) = s.off { if o >= s.tmax { notes.push(format!("The light goes off at {} minutes, after the {} minutes shown, so there is no dark part. Make the time shown longer or the light go off sooner.", num(o), num(s.tmax))); } }
    }
    // ---------- the marks this graph has (in display units), worked out before the axes so the axes hold them
    let mark = s.mark.as_str();
    let applies = match mark {
        "Saturation point" => matches!(s.g, G::Light | G::Distance | G::Co2),
        "Limiting factors" => s.rate_graph(),
        "Optimum" => s.g == G::Temp,
        "Compensation point" => s.rate_graph() || matches!(s.g, G::Chamber | G::GasTime),
        _ => true,
    };
    if !applies {
        notes.push(match mark {
            "Saturation point" => "A saturation point is marked on the graphs of rate against light, distance or CO2; this graph has none, so nothing is marked.".to_string(),
            "Limiting factors" => "Limiting factors are marked on the rate graphs (against light, distance, CO2 or temperature); this graph has none, so nothing is marked.".to_string(),
            "Optimum" => "An optimum is marked on the graph of rate against temperature; this graph has none, so nothing is marked.".to_string(),
            _ => "A compensation point is marked on the rate graphs and the closed chamber; this graph has none, so nothing is marked.".to_string(),
        });
    }
    let resp_line = applies && mark == "Compensation point" && s.rate_graph() && !s.net;
    if resp_line { for c in &s.conds { extra_y.push(s.shown_rate(resp(c.temp))); } }
    let (x_ax, y_ax, y2_ax) = {
        let all: Vec<P> = lines.iter().flat_map(|l| l.pts.iter().copied()).collect();
        let ymin = all.iter().map(|q| q.1).chain(extra_y.iter().copied()).fold(f64::INFINITY, f64::min);
        let ymax = all.iter().map(|q| q.1).chain(extra_y.iter().copied()).fold(f64::NEG_INFINITY, f64::max);
        let x_ax = if s.g == G::GasXy {
            let xmin = all.iter().map(|q| q.0).fold(f64::INFINITY, f64::min);
            let xmax = all.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max);
            let a = nice(xmin, xmax, true);
            match xmax_typed { Some(v) if v > a.lo => Ax { hi: v, step: crate::figures::axes::nice_step((v - a.lo) * 1.06, 6.0), lo: a.lo }, _ => a }
        } else {
            Ax { lo: dx0, hi: x_hi_d, step: crate::figures::axes::nice_step((x_hi_d - dx0) * 1.0, 7.0) }
        };
        let fitted = if s.g == G::GasXy { nice(ymin, ymax, false) } else { nice(ymin, ymax, true) };
        let y_ax = match typed_end(p.s("rg_ymax"), "y-axis top end", fitted.lo) {
            Ok(Some(v)) => Ax { lo: fitted.lo, hi: v, step: crate::figures::axes::nice_step((v - fitted.lo) * 1.06, 6.0) },
            Ok(None) => fitted,
            Err(e) => { notes.push(e); fitted }
        };
        let y2 = o2_line.as_ref().map(|o| {
            let lo = o.pts.iter().map(|q| q.1).fold(f64::INFINITY, f64::min);
            let hi = o.pts.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max);
            nice(lo, hi, false)
        });
        (x_ax, y_ax, y2)
    };
    // ---------- layout
    let numbered = !s.shape_only;
    let tick_text = |v: f64| num(v);
    let y_ticks = y_ax.ticks();
    let y_w = if numbered { y_ticks.iter().map(|v| rw(&tick_text(*v), fs)).fold(0.0, f64::max) } else { 0.0 };
    let y2_w = match (&y2_ax, numbered) { (Some(a), true) => a.ticks().iter().map(|v| rw(&tick_text(*v), fs)).fold(0.0, f64::max), _ => 0.0 };
    let x_ticks = x_ax.ticks();
    let last_half = if numbered { x_ticks.last().map_or(0.0, |v| rw(&tick_text(*v), fs) / 2.0) } else { 0.0 };
    // a long y title takes two rotated lines; past that it is cut short with an ellipsis
    let y_title_lines: Vec<String> = match &y_t {
        Some(Some(tt)) => { let mut v = wrap_rich(tt, PLOT_H - 10.0, ft); if v.len() > 2 { let rest = v[1..].join(" "); v.truncate(1); v.push(fit(&rest, PLOT_H - 10.0, ft)); } v }
        _ => vec![],
    };
    let y_title_w = match &y_t { Some(Some(_)) => y_title_lines.len() as f64 * 1.25 * ft + 0.25 * ft + 6.0, Some(None) => 1.5 * ft + 6.0, None => 0.0 };
    let l = MARGIN + y_title_w + if numbered { y_w + 10.0 } else { 4.0 };
    let r = W - MARGIN - if y2_ax.is_some() { (if numbered { y2_w + 10.0 } else { 4.0 }) + 1.5 * ft + 6.0 } else { last_half.max(6.0) };
    let mut y = MARGIN;
    let chart_lines: Vec<String> = match &chart_t { Some(Some(t)) => wrap_rich(t, W - 2.0 * MARGIN, 16.0 * lab), _ => vec![] };
    let chart_y = y;
    match &chart_t { Some(Some(_)) => y += chart_lines.len() as f64 * 1.3 * 16.0 * lab + 6.0, Some(None) => y += 1.6 * 16.0 * lab + 6.0, None => {} }
    let t = y + 0.6 * fs + 2.0;
    let b = t + PLOT_H;
    let px = |v: f64| l + (v - x_ax.lo) / (x_ax.hi - x_ax.lo) * (r - l);
    let py = |v: f64| b - (v - y_ax.lo) / (y_ax.hi - y_ax.lo) * (b - t);
    let py2 = |v: f64| match &y2_ax { Some(a) => b - (v - a.lo) / (a.hi - a.lo) * (b - t), None => b };
    let below_ticks = b + if numbered { 7.0 + fs + 4.0 } else { 6.0 };
    let x_title_lines: Vec<String> = match &x_t { Some(Some(tt)) => wrap_rich(tt, r - l, ft), _ => vec![] };
    let x_title_h = match &x_t { Some(Some(_)) => x_title_lines.len() as f64 * 1.25 * ft + 4.0, Some(None) => 1.4 * ft + 4.0, None => 0.0 };
    let mut yy = below_ticks + x_title_h + 8.0;
    // the key
    let show_lines = s.points != POINTS[2];
    let show_points = s.points != POINTS[0];
    let mut key_items: Vec<(usize, String, bool)> = Vec::new(); // (series, name, is O2)
    if s.g == G::GasTime { key_items.push((0, "CO~2".into(), false)); key_items.push((1, "O~2".into(), true)); }
    else if s.conds.len() > 1 || s.typed_names.iter().any(|t| !t.is_empty()) { for i in 0..s.conds.len() { key_items.push((i, s.name(i), false)); } }
    let sample_w = 34.0;
    let max_name = W - 2.0 * MARGIN - sample_w - 8.0;
    let key_names: Vec<String> = key_items.iter().map(|k| fit(&k.1, max_name, fs)).collect();
    let mut key_rows: Vec<Vec<(usize, f64)>> = vec![vec![]];
    {
        let mut x = 0.0;
        for (k, nm) in key_names.iter().enumerate() {
            let w = sample_w + 8.0 + rw(nm, fs);
            if x > 0.0 && x + w > W - 2.0 * MARGIN { key_rows.push(vec![]); x = 0.0; }
            key_rows.last_mut().unwrap().push((k, x));
            x += w + 22.0;
        }
    }
    let key_y = yy;
    if !key_items.is_empty() { yy += key_rows.len() as f64 * 1.6 * fs + 6.0; }

    // ---------- drawing the plot
    let mut g = String::new();
    let area: Bx = (l + 2.0, t + 2.0, r - 2.0, b - 2.0);
    // the dark part
    let off_x = s.off.filter(|o| !s.rate_graph() && s.g != G::GasXy && *o < s.tmax);
    let mut boxes: Vec<Bx> = Vec::new();
    if let Some(o) = off_x {
        let x0 = px(o).clamp(l, r);
        if x0 < r { g.push_str(&format!(r#"<rect x="{x0:.2}" y="{t:.2}" width="{:.2}" height="{:.2}" fill="{SHADE}"/>"#, r - x0, b - t)); }
    }
    // gridlines
    let grid_col = if s.print { GRID_P } else { GRID_C };
    if s.grid {
        for v in &x_ticks { let x = px(*v); if x > l + 0.5 && x <= r + 0.01 { ln(&mut g, (x, t), (x, b), grid_col, 1.0, ""); } }
        for v in &y_ticks { let yv = py(*v); if yv < b - 0.5 && yv >= t - 0.01 { ln(&mut g, (l, yv), (r, yv), grid_col, 1.0, ""); } }
    }
    // axes, ticks and numbers
    ln(&mut g, (l, b), (r, b), INK, 1.8, "");
    ln(&mut g, (l, t), (l, b), INK, 1.8, "");
    let zero_line = (y_ax.lo < 0.0 && y_ax.hi > 0.0).then(|| ((l, py(0.0)), (r, py(0.0))));
    if let Some((a, z)) = zero_line { ln(&mut g, a, z, INK, 1.4, ""); }
    if y2_ax.is_some() { ln(&mut g, (r, t), (r, b), INK, 1.8, ""); }
    if numbered {
        for v in &x_ticks {
            let x = px(*v);
            ln(&mut g, (x, b), (x, b + 5.0), INK, 1.4, "");
            g.push_str(&rich(x, b + 7.0 + 0.8 * fs, fs, "middle", INK, false, &tick_text(*v)));
        }
        for v in &y_ticks {
            let yv = py(*v);
            ln(&mut g, (l - 5.0, yv), (l, yv), INK, 1.4, "");
            g.push_str(&rich(l - 8.0, yv + 0.35 * fs, fs, "end", INK, false, &tick_text(*v)));
        }
        if let Some(a) = &y2_ax {
            for v in a.ticks() {
                let yv = py2(v);
                ln(&mut g, (r, yv), (r + 5.0, yv), INK, 1.4, "");
                g.push_str(&rich(r + 8.0, yv + 0.35 * fs, fs, "start", INK, false, &tick_text(v)));
            }
        }
    }
    // axis titles
    let xt_y = below_ticks + 0.85 * ft;
    match &x_t {
        Some(Some(_)) => for (k, tl) in x_title_lines.iter().enumerate() { g.push_str(&rich((l + r) / 2.0, xt_y + k as f64 * 1.25 * ft, ft, "middle", INK, false, tl)); },
        Some(None) => { let w = 200.0 * lab; ln(&mut g, ((l + r) / 2.0 - w / 2.0, xt_y + 0.3 * ft), ((l + r) / 2.0 + w / 2.0, xt_y + 0.3 * ft), INK, 1.0, ""); }
        None => {}
    }
    let yt_x = MARGIN + 0.85 * ft;
    let rot = |x: f64, yc: f64, txt: &str| -> String {
        let txt = fit(txt, b - t, ft);
        format!(r#"<g transform="translate({x:.2} {yc:.2}) rotate(-90)">{}</g>"#, rich(0.0, 0.0, ft, "middle", INK, false, &txt))
    };
    match &y_t {
        Some(Some(_)) => for (k, tl) in y_title_lines.iter().enumerate() { g.push_str(&rot(yt_x + k as f64 * 1.25 * ft, (t + b) / 2.0, tl)); },
        Some(None) => { let h = 160.0 * lab; ln(&mut g, (yt_x, (t + b) / 2.0 - h / 2.0), (yt_x, (t + b) / 2.0 + h / 2.0), INK, 1.0, ""); }
        None => {}
    }
    if let Some(tt) = &y2_t { g.push_str(&rot(W - MARGIN - 0.35 * ft, (t + b) / 2.0, tt)); }
    // the lines (clipped to the plot)
    let mut segs: Vec<(P, P)> = zero_line.into_iter().collect();
    let screen = |q: &P| (px(q.0), py(q.1));
    let mut curves = String::new();
    let in_plot = |q: P| q.0 >= l - 0.5 && q.0 <= r + 0.5 && q.1 >= t - 0.5 && q.1 <= b + 0.5;
    if show_lines {
        for line in &lines {
            let sp: Vec<P> = line.pts.iter().map(screen).collect();
            for w2 in sp.windows(2) { if in_plot(w2[0]) || in_plot(w2[1]) { segs.push((w2[0], w2[1])); } }
            curves.push_str(&poly(&sp, col(line.idx), 2.8, dash(line.idx)));
        }
        if let Some(o) = &o2_line {
            let sp: Vec<P> = o.pts.iter().map(|q| (px(q.0), py2(q.1))).collect();
            for w2 in sp.windows(2) { segs.push((w2[0], w2[1])); }
            curves.push_str(&poly(&sp, col(1), 2.8, if s.print { "7 4" } else { "" }));
        }
    }
    let clip_id = "ps-rate-plot";
    g.push_str(&format!(r#"<clipPath id="{clip_id}"><rect x="{l:.2}" y="{t:.2}" width="{:.2}" height="{:.2}"/></clipPath>"#, r - l, b - t));
    let mut over = String::new(); // guides, dots and labels, on top of the lines
    let mut guides = String::new();
    let lbl_answers = s.key && s.mark_lab == MARK_LABS[0];
    let lbl_blank = s.mark_lab == MARK_LABS[1] || (!s.key && s.mark_lab == MARK_LABS[0]);
    // the respiration line for a compensation point on a gross graph
    let mut deferred: Vec<(P, String, Vec<f64>)> = Vec::new(); // (anchor, label text, preferred directions)
    let dot = |o: &mut String, q: P, c: &str| o.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="4.4" fill="{c}" stroke="#ffffff" stroke-width="1.4"/>"##, q.0, q.1));
    let mut dots: Vec<P> = Vec::new();
    let x_in = |x: f64| x >= x_ax.lo - 1e-9 && x <= x_ax.hi + 1e-9;
    // the dashed guides the marks add (from here to the end of the marks) are soft: a leader may cross them
    let soft_from = segs.len();
    if applies {
        match mark {
            "Saturation point" => for (i, c) in s.conds.iter().enumerate() {
                match sat_x(&s, c) {
                    Some(xm) if x_in(s.x_shown(xm)) && xm >= if s.g == G::Distance { DIST_MIN } else { 0.0 } => {
                        let q = (px(s.x_shown(xm)), py(s.rate_at(c, xm)));
                        ln(&mut guides, (l, q.1), (q.0, q.1), GUIDE, 1.3, "5 4");
                        ln(&mut guides, (q.0, q.1), (q.0, b), GUIDE, 1.3, "5 4");
                        segs.push(((l, q.1), (q.0, q.1)));
                        segs.push(((q.0, q.1), (q.0, b)));
                        dots.push(q);
                        dot(&mut over, q, col(i));
                        // one label names the kind of mark; the other curves' dots and guides read the same way
                        if !deferred.iter().any(|d| d.1 == "Saturation point") { deferred.push((q, "Saturation point".into(), if s.g == G::Distance { vec![315.0, 45.0] } else { vec![225.0, 315.0, 135.0] })); }
                    }
                    _ => notes.push(format!("The saturation point of {} is beyond the end of the x-axis, so it is not marked.", curve_word(&s, i))),
                }
            },
            "Limiting factors" => {
                // the rise: on the curve whose rise runs furthest
                if let Some((i, xs)) = (s.g != G::Temp).then(|| s.conds.iter().enumerate().filter_map(|(i, c)| sat_x(&s, c).map(|x| (i, x))).max_by(|a, b| if s.g == G::Distance { b.1.total_cmp(&a.1) } else { a.1.total_cmp(&b.1) })).flatten() {
                    let c = &s.conds[i];
                    let xr = if s.g == G::Distance { xs + 0.4 * (s.x_model(x_ax.hi) - xs) } else { 0.45 * xs };
                    let rise = match s.g { G::Co2 => "CO~2 limiting", _ => "Light limiting" };
                    if x_in(s.x_shown(xr)) { deferred.push(((px(s.x_shown(xr)), py(s.rate_at(c, xr))), rise.into(), vec![300.0, 240.0, 60.0, 120.0])); }
                }
                if s.g == G::Temp {
                    let c = &s.conds[0];
                    let to = opt_x(&s, c);
                    let (l0, c0, _) = s.at(c, to);
                    // below the optimum the enzymes are slow (temperature limits) where light does not cap the rate
                    let tr = 0.55 * to;
                    if pmax(c0, tr) < 0.9 * ALPHA * l0 { deferred.push(((px(tr), py(s.rate_at(c, tr))), "Temperature limiting".into(), vec![300.0, 240.0, 330.0])); }
                    let tf = to + 0.45 * (temp_comp(l0, c0).unwrap_or(48.0).min(48.0) - to).max(2.0);
                    deferred.push(((px(tf), py(s.rate_at(c, tf))), "Enzymes denaturing".into(), vec![330.0, 300.0, 30.0]));
                    for c in &s.conds {
                        if ALPHA * c.light < 0.9 * pmax(c.co2, t_opt()) {
                            let tl = t_opt() - 3.0;
                            deferred.push(((px(tl), py(s.rate_at(c, tl))), "Light limiting".into(), vec![270.0, 90.0]));
                        }
                    }
                } else {
                    for (i, c) in s.conds.iter().enumerate() {
                        let Some(xs) = sat_x(&s, c) else { continue };
                        let hi = s.x_model(x_ax.hi);
                        let xp = if s.g == G::Distance { (DIST_MIN + xs) / 2.0 } else { xs + 0.55 * (hi - xs) };
                        if xs < hi && x_in(s.x_shown(xp)) {
                            deferred.push(((px(s.x_shown(xp)), py(s.rate_at(c, xp))), plateau_limit(&s, c).into(), vec![270.0, 90.0]));
                        } else { notes.push(format!("The plateau of {} is beyond the end of the x-axis, so what limits it is not marked.", curve_word(&s, i))); }
                    }
                }
            }
            "Optimum" => {
                let c = &s.conds[0];
                let to = opt_x(&s, c);
                let q = (px(to), py(s.rate_at(c, to)));
                ln(&mut guides, (q.0, q.1), (q.0, b), GUIDE, 1.3, "5 4");
                ln(&mut guides, (l, q.1), (q.0, q.1), GUIDE, 1.3, "5 4");
                segs.push(((q.0, q.1), (q.0, b)));
                segs.push(((l, q.1), (q.0, q.1)));
                dots.push(q);
                dot(&mut over, q, col(0));
                deferred.push((q, "Optimum".into(), vec![300.0, 240.0, 270.0]));
            }
            "Compensation point" => {
                if s.rate_graph() {
                    for (i, c) in s.conds.iter().enumerate() {
                        let rr = s.shown_rate(resp(c.temp));
                        if resp_line {
                            let yr = py(rr);
                            ln(&mut guides, (l, yr), (r, yr), col(i), 1.4, "3 3");
                            segs.push(((l, yr), (r, yr)));
                            if i == 0 { deferred.push(((r - 40.0, yr), "Respiration".into(), vec![270.0, 90.0])); }
                        }
                        let lo = if s.g == G::Distance { DIST_MIN } else { 0.0 };
                        match comp_x(&s, c) {
                            Some(xc) if x_in(s.x_shown(xc)) && xc >= lo => {
                                let q = (px(s.x_shown(xc)), py(if s.net { 0.0 } else { rr }));
                                if !s.net || s.bubbles { ln(&mut guides, q, (q.0, b), GUIDE, 1.3, "5 4"); segs.push((q, (q.0, b))); }
                                // curves whose compensation points coincide share one label
                                let shared = dots.iter().any(|d| (d.0 - q.0).hypot(d.1 - q.1) < 6.0);
                                dots.push(q);
                                dot(&mut over, q, col(i));
                                if !shared && !deferred.iter().any(|d| d.1 == "Compensation point") { deferred.push((q, "Compensation point".into(), vec![300.0, 240.0, 330.0, 210.0, 30.0])); }
                            }
                            _ => notes.push(format!("The compensation point of {} is not on the x-axis shown, so it is not marked.", curve_word(&s, i))),
                        }
                    }
                } else {
                    let mut levels: Vec<f64> = Vec::new();
                    for (i, c) in s.conds.iter().enumerate() {
                        if c.light <= 0.0 { continue; }
                        match co2_comp(c.light, c.temp) {
                            Some(gm) if s.co2_shown(gm) <= y_ax.hi => {
                                let yg = py(s.co2_shown(gm));
                                // two light levels with (nearly) the same compensation point share one line and one label
                                if levels.iter().any(|v| (v - yg).abs() < 3.0) { continue; }
                                levels.push(yg);
                                ln(&mut guides, (l, yg), (r, yg), GUIDE, 1.4, "3 3");
                                segs.push(((l, yg), (r, yg)));
                                deferred.push(((l + 0.62 * (r - l), yg), "Compensation point".into(), vec![270.0, 90.0]));
                            }
                            _ => notes.push(format!("At light {} the CO2 never falls (the light is below the compensation point), so {} has no compensation level to mark.", num(c.light), curve_word(&s, i))),
                        }
                    }
                }
            }
            "Lettered points" => {
                let c = &s.conds[0];
                let xs: Vec<f64> = match s.g {
                    G::Light | G::Co2 => { let hi = s.x_model(x_ax.hi); let xs = sat_x(&s, c).unwrap_or(hi / 2.0).min(hi * 0.8); vec![0.4 * xs, xs, xs + 0.6 * (hi - xs)] }
                    G::Distance => { let hi = s.x_model(x_ax.hi); let ds = sat_x(&s, c).unwrap_or(hi / 3.0).clamp(DIST_MIN + 2.0, hi * 0.6); vec![(DIST_MIN + ds) / 2.0 + 1.0, ds, ds + 0.6 * (hi - ds)] }
                    G::Temp => { let to = opt_x(&s, c); vec![to * 0.5, to, (to + 8.0).min(x_ax.hi)] }
                    _ => match s.off.filter(|o| *o < s.tmax) { Some(o) => vec![o * 0.3, o, o + 0.7 * (s.tmax - o)], None => vec![0.1 * s.tmax, 0.4 * s.tmax, 0.85 * s.tmax] },
                };
                for (k, xv) in xs.iter().enumerate() {
                    let q = match s.g {
                        G::Chamber | G::GasTime => { let pts = &lines[0].pts; let j = pts.iter().position(|q| q.0 >= *xv - 1e-9).unwrap_or(pts.len() - 1); (px(pts[j].0), py(pts[j].1)) }
                        G::GasXy => { let j = path_times.iter().position(|q| *q >= *xv - 1e-9).unwrap_or(path_times.len().saturating_sub(1)); let q = lines[0].pts[j]; (px(q.0), py(q.1)) }
                        _ => (px(s.x_shown(*xv)), py(s.rate_at(c, *xv))),
                    };
                    dots.push(q);
                    dot(&mut over, q, if s.print { INK } else { "#c0392b" });
                    let letter = ["A", "B", "C"][k];
                    let f = 1.1 * fs;
                    let w = rw(letter, f);
                    let all_boxes: Vec<Bx> = boxes.iter().copied().chain(dots.iter().map(|d| (d.0 - 5.0, d.1 - 5.0, d.0 + 5.0, d.1 + 5.0))).collect();
                    match place(q, w, f, &[225.0, 315.0, 135.0, 45.0], &segs, &all_boxes, area) {
                        Some(bx) => { over.push_str(&backing(bx)); over.push_str(&rich((bx.0 + bx.2) / 2.0, bx.3 - 0.2 * f, f, "middle", INK, true, letter)); boxes.push(bx); }
                        None => notes.push(format!("The letter {letter} could not be placed clear of the lines, so only its dot is drawn.")),
                    }
                }
            }
            _ => {}
        }
    }
    let soft_to = segs.len();
    // the dark and light words over the shading
    if let Some(o) = off_x {
        let f = 0.95 * fs;
        for (word, x0, x1) in [("Light", l, px(o)), ("Dark", px(o), r)] {
            let w = rw(word, f);
            if x1 - x0 > w + 8.0 {
                let cx = (x0 + x1) / 2.0;
                let bx = (cx - w / 2.0, t + 4.0, cx + w / 2.0, t + 4.0 + f);
                let clear = !segs.iter().any(|(a, c)| seg_hits_box(*a, *c, expand(bx, 1.5)));
                let bx = if clear { bx } else { (cx - w / 2.0, b - 4.0 - f, cx + w / 2.0, b - 4.0) };
                if !segs.iter().any(|(a, c)| seg_hits_box(*a, *c, expand(bx, 1.5))) {
                    over.push_str(&rich(cx, bx.3 - 0.22 * f, f, "middle", INK, false, word));
                    boxes.push(bx);
                }
            }
        }
    }
    // the direction of travel on the CO2 against O2 graph
    if s.g == G::GasXy && show_lines && lines[0].pts.len() > 2 {
        let pts = &lines[0].pts;
        let jo = s.off.filter(|o| *o < s.tmax).and_then(|o| path_times.iter().position(|q| *q >= o - 1e-9)).unwrap_or(pts.len() - 1);
        let legs: Vec<(usize, usize, &str)> = if jo < pts.len() - 1 { vec![(0, jo, "In the light"), (jo, pts.len() - 1, "In the dark")] } else { vec![(0, pts.len() - 1, "In the light")] };
        // the side the light leg's arrow is offset to; the dark leg's goes on the other side (they run along the same line)
        let (p0, p1) = ((px(pts[0].0), py(pts[0].1)), (px(pts[jo].0), py(pts[jo].1)));
        let l0 = (p1.0 - p0.0).hypot(p1.1 - p0.1).max(1e-9);
        let n0 = (-(p1.1 - p0.1) / l0, (p1.0 - p0.0) / l0);
        for (k, (a, z, word)) in legs.iter().enumerate() {
            let (pa, pz) = ((px(pts[*a].0), py(pts[*a].1)), (px(pts[*z].0), py(pts[*z].1)));
            let len = (pz.0 - pa.0).hypot(pz.1 - pa.1);
            if len < 30.0 { continue; }
            let (ux, uy) = ((pz.0 - pa.0) / len, (pz.1 - pa.1) / len);
            let side = if k == 0 { 1.0 } else { -1.0 };
            let (nx, ny) = (n0.0 * side * 18.0, n0.1 * side * 18.0);
            let m = ((pa.0 + pz.0) / 2.0, (pa.1 + pz.1) / 2.0);
            let half = (len * 0.3).min(70.0);
            let (a0, a1) = ((m.0 + nx - ux * half, m.1 + ny - uy * half), (m.0 + nx + ux * half, m.1 + ny + uy * half));
            let mut d2 = Doc::new(1.0, 1.0);
            d2.arrow(a0.0, a0.1, a1.0, a1.1, if s.print { INK } else { "#555555" }, 1.8);
            over.push_str(&d2.cut(0));
            segs.push((a0, a1));
            let f = 0.95 * fs;
            let w = rw(word, f);
            let all_boxes: Vec<Bx> = boxes.clone();
            let mid = ((a0.0 + a1.0) / 2.0 + nx * 0.6, (a0.1 + a1.1) / 2.0 + ny * 0.6);
            let away = (ny.atan2(nx)).to_degrees().rem_euclid(360.0);
            if let Some(bx) = place(mid, w, f, &[away, away - 30.0, away + 30.0], &segs, &all_boxes, area) {
                over.push_str(&backing(bx));
                over.push_str(&rich((bx.0 + bx.2) / 2.0, bx.3 - 0.22 * f, f, "middle", INK, false, word));
                boxes.push(bx);
            }
        }
    }
    // data points (scattered a little, as if measured)
    let mut pts_svg = String::new();
    if show_points {
        let span = y_ax.hi - y_ax.lo;
        for line in &lines {
            let c = &line.pts;
            let n = c.len();
            if n < 2 { continue; }
            let (x0, x1) = (c[0].0, c[n - 1].0);
            for k in 1..=8 {
                let xt = x0 + (x1 - x0) * k as f64 / 9.0;
                let j = c.iter().position(|q| q.0 >= xt - 1e-12).unwrap_or(n - 1);
                let (xq, yq) = c[j];
                let u = crate::figures::unit_random(s.seed * 7919 + line.idx as u64 * 131 + k as u64);
                let mut yv = yq + (u - 0.5) * 2.0 * 0.03 * span;
                if s.bubbles && s.rate_graph() { yv = yv.round().max(0.0); }
                let q = (px(xq), py(yv));
                if in_plot(q) { marker(&mut pts_svg, line.idx, q, 4.2, col(line.idx)); boxes.push((q.0 - 5.5, q.1 - 5.5, q.0 + 5.5, q.1 + 5.5)); }
            }
        }
        if let Some(o) = &o2_line {
            let n = o.pts.len();
            for k in 1..=8 {
                let j = (n - 1) * k / 9;
                let (xq, yq) = o.pts[j];
                let a = y2_ax.as_ref().unwrap();
                let u = crate::figures::unit_random(s.seed * 7919 + 977 + k as u64);
                let q = (px(xq), py2(yq + (u - 0.5) * 2.0 * 0.03 * (a.hi - a.lo)));
                if in_plot(q) { marker(&mut pts_svg, 1, q, 4.2, col(1)); boxes.push((q.0 - 5.5, q.1 - 5.5, q.0 + 5.5, q.1 + 5.5)); }
            }
        }
    }
    // the marks' labels
    for (anchor, text, prefer) in &deferred {
        if s.mark_lab == MARK_LABS[2] { break; }
        let blank = lbl_blank || !lbl_answers;
        let w = if blank { 76.0 * lab } else { rw(text, fs) };
        let h = 1.15 * fs;
        let all_boxes: Vec<Bx> = boxes.iter().copied().chain(dots.iter().map(|d| (d.0 - 5.0, d.1 - 5.0, d.0 + 5.0, d.1 + 5.0))).collect();
        match place(*anchor, w, h, prefer, &segs, &all_boxes, area) {
            Some(bx) => {
                // a label set away from its mark gets a thin leader to it, unless the leader would cross a line
                let near = (anchor.0.clamp(bx.0, bx.2), anchor.1.clamp(bx.1, bx.3));
                let dist = (near.0 - anchor.0).hypot(near.1 - anchor.1);
                if dist > 16.0 {
                    let u = ((near.0 - anchor.0) / dist, (near.1 - anchor.1) / dist);
                    let a = (anchor.0 + u.0 * 7.0, anchor.1 + u.1 * 7.0);
                    let z = (near.0 - u.0 * 2.0, near.1 - u.1 * 2.0);
                    if !segs.iter().enumerate().any(|(k, (p, q))| !(soft_from..soft_to).contains(&k) && crosses(a, z, *p, *q)) && !all_boxes.iter().any(|b2| seg_hits_box(a, z, *b2)) {
                        ln(&mut over, a, z, INK, 0.9, "");
                        segs.push((a, z));
                    }
                }
                if blank { ln(&mut over, (bx.0, bx.3 - 1.0), (bx.2, bx.3 - 1.0), INK, 1.1, ""); } else { over.push_str(&backing(bx)); over.push_str(&rich((bx.0 + bx.2) / 2.0, bx.3 - 0.27 * fs, fs, "middle", INK, false, text)); }
                boxes.push(bx);
            }
            None => notes.push(format!("A label ({}) could not be placed clear of the lines, so only the mark is drawn.", plain(text))),
        }
    }

    // ---------- the key and the captions
    let mut tail = String::new();
    for (ri, row) in key_rows.iter().enumerate() {
        let row_w: f64 = row.iter().map(|(k, _)| sample_w + 8.0 + rw(&key_names[*k], fs)).sum::<f64>() + 22.0 * (row.len().saturating_sub(1)) as f64;
        let x_start = (W - row_w) / 2.0;
        let ry = key_y + ri as f64 * 1.6 * fs + 0.8 * fs;
        for (k, x) in row {
            let (series, _, is_o2) = &key_items[*k];
            let x0 = x_start + x;
            if show_lines { tail.push_str(&poly(&[(x0, ry), (x0 + sample_w, ry)], col(*series), 2.8, if *is_o2 { if s.print { "7 4" } else { "" } } else { dash(*series) })); }
            if show_points { marker(&mut tail, *series, (x0 + sample_w / 2.0, ry), 4.2, col(*series)); }
            tail.push_str(&rich(x0 + sample_w + 8.0, ry + 0.35 * fs, fs, "start", INK, false, &key_names[*k]));
        }
    }
    let caps: Vec<String> = if s.key { readouts(&s) } else { vec![] };
    let cap_lines: Vec<String> = caps.iter().flat_map(|c| wrap_rich(c, W - 2.0 * MARGIN, fc)).collect();
    let cap_y = yy;
    for (k, cl) in cap_lines.iter().enumerate() { tail.push_str(&rich(MARGIN, cap_y + fc + k as f64 * 1.35 * fc, fc, "start", "#222222", false, cl)); }
    let total_h = cap_y + if cap_lines.is_empty() { 0.0 } else { cap_lines.len() as f64 * 1.35 * fc + 6.0 } + MARGIN;

    // ---------- assemble
    let mut d = Doc::new(W, total_h.ceil());
    d.defs(&fills.defs());
    let (ttl, desc) = describe(&s);
    d.describe(&ttl, &desc);
    match &chart_t {
        Some(Some(_)) => for (k, tl) in chart_lines.iter().enumerate() { d.raw(&rich(W / 2.0, chart_y + 16.0 * lab + k as f64 * 1.3 * 16.0 * lab, 16.0 * lab, "middle", INK, true, tl)); },
        Some(None) => { let w = 240.0 * lab; d.line(W / 2.0 - w / 2.0, chart_y + 1.3 * 16.0 * lab, W / 2.0 + w / 2.0, chart_y + 1.3 * 16.0 * lab, INK, 1.0); }
        None => {}
    }
    d.raw(&g);
    d.raw(&guides);
    d.raw(&format!(r#"<g clip-path="url(#{clip_id})">{curves}</g>"#));
    d.raw(&pts_svg);
    d.raw(&over);
    d.raw(&tail);
    Out { svg: d.finish(), notes }
}

fn poly(pts: &[P], col: &str, sw: f64, dash: &str) -> String {
    format!(r#"<polyline points="{}" fill="none" stroke="{col}" stroke-width="{sw}" stroke-linejoin="round" stroke-linecap="round"{}/>"#, pts.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect::<Vec<_>>().join(" "), if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) })
}

fn curve_word(s: &S, i: usize) -> String { if s.conds.len() > 1 { format!("curve {} ({})", i + 1, plain(&s.name(i))) } else { "the curve".to_string() } }

/// The accessible name and description: what is plotted, never a value a student is asked for.
pub fn describe(s: &S) -> (String, String) {
    let n = s.conds.len();
    let curves = if n == 1 { "one curve".to_string() } else { format!("{n} curves") };
    let what = match s.g {
        G::Light => format!("A graph of the rate of photosynthesis against light intensity, {curves}"),
        G::Distance => format!("A graph of the rate of photosynthesis against the distance from the lamp, {curves}"),
        G::Co2 => format!("A graph of the rate of photosynthesis against CO2 concentration, {curves}"),
        G::Temp => format!("A graph of the rate of photosynthesis against temperature, {curves}"),
        G::Chamber => format!("A graph of the CO2 in a closed chamber over time, {curves}"),
        G::GasTime => "A graph of the CO2 and O2 around a plant over time".to_string(),
        G::GasXy => "A graph of the O2 against the CO2 around a plant".to_string(),
    };
    let dark = if s.off.is_some_and(|o| o < s.tmax) && !s.rate_graph() { ", with the light going off part way (the dark part shaded)" } else { "" };
    ("Rate of photosynthesis graph".to_string(), format!("{what}{dark}."))
}

/// What the key says under the graph: per curve, the features a student reads off it, in the units the graph shows.
pub fn readouts(s: &S) -> Vec<String> {
    let ru = s.rate_unit();
    let mut v = Vec::new();
    let tag = |i: usize| if s.conds.len() > 1 || s.typed_names.iter().any(|t| !t.is_empty()) { format!("{}: ", s.name(i)) } else { String::new() };
    let resp_note = |c: &Cond| if s.net { format!(" Respiration at {} \u{b0}C is {} {ru}, so the net rate is that much below the gross rate.", num(c.temp), sig2(s.shown_rate(resp(c.temp)))) } else { String::new() };
    for (i, c) in s.conds.iter().enumerate() {
        let lvl = |gross_au: f64| sig2(s.shown_rate(if s.net { gross_au - resp(c.temp) } else { gross_au }));
        let line = match s.g {
            G::Light => {
                let xs = sat_x(s, c).unwrap_or(0.0);
                let comp = if s.net || s.mark == "Compensation point" { light_comp(c.co2, c.temp).map_or(String::new(), |l| format!(" Photosynthesis just balances respiration (the net rate is zero) at a light intensity of about {}, the compensation point.", sig2(l))) } else { String::new() };
                format!("{}light limits the rate as it rises; by a light intensity of about {} (the saturation point) the rate has nearly levelled off, and it levels off at about {} {ru}, limited by {}.{comp}{}", tag(i), sig2(xs), lvl(plateau(s, c)), plateau_limit_words(s, c), resp_note(c))
            }
            G::Distance => {
                let ds = sat_x(s, c).unwrap_or(0.0);
                format!("{}closer to the lamp than about {} cm (the saturation point) the rate has levelled off at about {} {ru}, limited by {}; further away light limits it. Light intensity falls as 1/d\u{b2}: at twice the distance a quarter of the light.{}", tag(i), sig2(ds), lvl(plateau(s, c)), plateau_limit_words(s, c), resp_note(c))
            }
            G::Co2 => {
                let xs = sat_x(s, c).unwrap_or(0.0);
                let comp = if s.net || s.mark == "Compensation point" { co2_comp(c.light, c.temp).map_or(String::new(), |g| format!(" Photosynthesis just balances respiration (the net rate is zero) at about {} {} CO~2, the compensation point.", sig2(s.co2_shown(g)), s.co2_unit())) } else { String::new() };
                format!("{}CO~2 limits the rate as it rises; by about {} {} (the saturation point) the rate has nearly levelled off, and it levels off at about {} {ru}, limited by {}.{comp}{}", tag(i), sig2(s.co2_shown(xs)), s.co2_unit(), lvl(plateau(s, c)), plateau_limit_words(s, c), resp_note(c))
            }
            G::Temp => {
                let to = opt_x(s, c);
                let flat = ALPHA * c.light < 0.9 * pmax(c.co2, t_opt());
                let comp = if s.net || s.mark == "Compensation point" { temp_comp(c.light, c.co2).map_or(String::new(), |t| format!(" Above about {} \u{b0}C respiration is faster than photosynthesis (the compensation point is there).", sig2(t))) } else { String::new() };
                let (to, r) = (sig2(to), sig2(s.rate_at(c, to)));
                let dn = num(T_DENAT);
                match (flat, s.net) {
                    (false, _) => format!("{}the rate is highest at about {to} \u{b0}C ({r} {ru}); below that, enzymes work more slowly; above it the rate falls steeply as enzymes denature.{comp}", tag(i)),
                    (true, false) => format!("{}light limits the rate over most of the range, so the curve is flat-topped at about {r} {ru}; it falls steeply above about {dn} \u{b0}C as enzymes denature.{comp}", tag(i)),
                    (true, true) => format!("{}light limits photosynthesis over most of the range, so the net rate is highest at about {to} \u{b0}C ({r} {ru}); warmer than that respiration, which doubles every 10 \u{b0}C, takes more of it, and above about {dn} \u{b0}C enzymes denature.{comp}", tag(i)),
                }
            }
            G::Chamber => {
                let rise = sig2(s.co2_shown(K_CH * resp(c.temp)));
                if c.light <= 0.0 { format!("{}in the dark the plant only respires, so the CO~2 rises steadily, by about {rise} {} a minute.", tag(i), s.co2_unit()) }
                else {
                    let gm = co2_comp(c.light, c.temp);
                    let mut t = match gm { Some(gm) if gm < s.c0 => format!("{}in the light photosynthesis is faster than respiration, so the CO~2 falls, levelling off at about {} {} (the compensation point, where photosynthesis just balances respiration).", tag(i), sig2(s.co2_shown(gm)), s.co2_unit()), Some(gm) => format!("{}at this light the compensation point, {} {}, is above the starting CO~2, so the CO~2 rises towards it.", tag(i), sig2(s.co2_shown(gm)), s.co2_unit()), None => format!("{}the light is too dim for photosynthesis to keep up with respiration, so the CO~2 rises.", tag(i)) };
                    if let Some(o) = s.off.filter(|o| *o < s.tmax) { t.push_str(&format!(" After the light goes off at {} minutes the plant only respires and the CO~2 rises by about {rise} {} a minute.", num(o), s.co2_unit())); }
                    t
                }
            }
            G::GasTime | G::GasXy => {
                let mut t = if c.light > 0.0 { "In the light the plant takes in CO~2 and gives out the same amount of O~2, so CO~2 falls and O~2 rises.".to_string() } else { "In the dark the plant only respires, so CO~2 rises and O~2 falls by the same amount.".to_string() };
                if c.light > 0.0 && s.off.is_some_and(|o| o < s.tmax) { t.push_str(" After the light goes off it only respires: CO~2 rises and O~2 falls by the same amount."); }
                if s.g == G::GasXy { t.push_str(" Each CO~2 taken in matches one O~2 given out, so the points lie on a straight line sloping down."); }
                t
            }
        };
        v.push(cap(&line));
        if matches!(s.g, G::GasTime | G::GasXy) { break; }
    }
    v
}

/// The note under a curve's section in the panel: what that curve shows (its readout), in plain text.
pub fn curve_help(title: &str, p: &Params) -> String {
    if !is_rate(p) { return String::new(); }
    let Some(i) = title.strip_prefix("Curve ").and_then(|n| n.trim().parse::<usize>().ok()).and_then(|n| n.checked_sub(1)) else { return String::new() };
    let s = settings(p);
    if i >= s.conds.len() || matches!(s.g, G::GasTime | G::GasXy) { return String::new(); }
    let all = readouts(&s);
    let Some(line) = all.get(i) else { return String::new() };
    let tag = format!("{}: ", s.name(i));
    let body = line.strip_prefix(&tag).map(cap).unwrap_or_else(|| line.clone());
    panel_text(&body)
}

/// Marked-up text (`CO~2`) as plain text with subscript digits, for the panel.
pub fn panel_text(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '~' => while let Some(d) = it.peek().copied().filter(|d| d.is_ascii_digit()) { out.push(char::from_u32(0x2080 + d.to_digit(10).unwrap_or(0)).unwrap_or(d)); it.next(); },
            '^' => { if let Some(d) = it.next() { out.push(d); } }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests;
