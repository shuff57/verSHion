//! Heating and cooling curve, batch 8: segment lengths computed from the mass, the substance's specific heats and its
//! enthalpies of fusion and vaporization (q = m·c·ΔT for a sloped part, q = n·ΔH for a plateau, n = m ÷ M), drawn along
//! the x-axis as the heat itself (kJ) or as the time it takes at a heating rate the teacher picks (q ÷ rate, in min).
//!
//! The mode is a third Segment lengths option, so the two old ones keep meaning what they meant ("Schematic": fixed equal
//! parts; "To scale": the typed minutes) and every new param's default draws nothing different until it is picked.
use super::curves::Ph;
use crate::figures::{ParamDef, Params};

/// The new Segment lengths option.
pub const COMPUTED: &str = "Computed from the heats";

/// Under this many pixels along the x-axis a segment cannot be told from a corner (a panel note says so).
pub const MIN_SEEN_PX: f64 = 3.0;
/// The whole curve must run between these many axis units (min or kJ): shorter, the fitted axis would number in steps
/// finer than its labels print (0.0025 printed as 0.003); longer, its numbers run off the page.
pub const MIN_TOTAL: f64 = 0.1;
pub const MAX_TOTAL: f64 = 1.0e6;

/// A substance's heats: specific heats in J/(g·K), enthalpies in kJ/mol, molar mass in g/mol.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heats { pub c_solid: f64, pub c_liquid: f64, pub c_gas: f64, pub dh_fus: f64, pub dh_vap: f64, pub molar: f64 }

/// The built-in substances' heats. Each heat capacity is the value near the substance's own melting or boiling point
/// (where the curve runs), divided by the molar mass where the source gives J/(mol·K).
pub fn table(name: &str) -> Option<Heats> {
    Some(match name {
        // Water. Ice 2.09, liquid 4.18 (4.184), steam 2.01 J/(g·K): the values general-chemistry textbooks use in their
        // heating-curve examples (ice near 0 °C, steam near 100 °C; CRC Handbook of Chemistry and Physics gives ice 2.05 to
        // 2.11 and steam 2.01 to 2.08 there). ΔHfus 6.01 kJ/mol at 0 °C and ΔHvap 40.7 kJ/mol at 100 °C: OpenStax
        // Chemistry 2e, §10.3 (Phase transitions). M 18.02 g/mol.
        "Water" => Heats { c_solid: 2.09, c_liquid: 4.18, c_gas: 2.01, dh_fus: 6.01, dh_vap: 40.7, molar: 18.02 },
        // Ethanol, C2H5OH, M 46.07. ΔHfus 4.93 kJ/mol and ΔHvap 38.56 kJ/mol at the boiling point (CRC Handbook of
        // Chemistry and Physics, 97th ed., "Enthalpy of fusion" and "Enthalpy of vaporization" tables). Liquid Cp 112.3
        // J/(mol·K) = 2.44 J/(g·K), gas Cp 65.6 J/(mol·K) = 1.42 J/(g·K) (NIST Chemistry WebBook). Solid 0.97 J/(g·K),
        // the solid near its melting point (NIST WebBook condensed-phase Cp, about 45 J/(mol·K)); approximate.
        "Ethanol" => Heats { c_solid: 0.97, c_liquid: 2.44, c_gas: 1.42, dh_fus: 4.93, dh_vap: 38.56, molar: 46.07 },
        // Acetone, (CH3)2CO, M 58.08. ΔHfus 5.77 kJ/mol, ΔHvap 29.1 kJ/mol at the boiling point (CRC Handbook, as above).
        // Liquid Cp 125.5 J/(mol·K) = 2.16 J/(g·K), gas Cp 75 J/(mol·K) = 1.29 J/(g·K) (NIST Chemistry WebBook). Solid
        // about 96 J/(mol·K) = 1.65 J/(g·K) near its melting point (NIST WebBook condensed-phase Cp); approximate.
        "Acetone" => Heats { c_solid: 1.65, c_liquid: 2.16, c_gas: 1.29, dh_fus: 5.77, dh_vap: 29.1, molar: 58.08 },
        // Mercury, Hg, M 200.59. ΔHfus 2.29 kJ/mol, ΔHvap 59.11 kJ/mol (CRC Handbook, as above). Solid about 28.3 and liquid
        // 27.98 J/(mol·K) = 0.141 and 0.139 J/(g·K) (CRC "Heat capacity of the elements"); the vapour is monatomic, Cp = 5/2 R
        // = 20.79 J/(mol·K) = 0.104 J/(g·K).
        "Mercury" => Heats { c_solid: 0.141, c_liquid: 0.139, c_gas: 0.104, dh_fus: 2.29, dh_vap: 59.11, molar: 200.59 },
        // Sodium chloride, NaCl, M 58.44. ΔHfus 28.16 kJ/mol (CRC Handbook, as above); ΔHvap about 170 kJ/mol at the boiling
        // point (NIST-JANAF thermochemical tables, NaCl liquid to gas). Solid about 66 J/(mol·K) = 1.13 J/(g·K) near the
        // melting point, liquid 77.8 J/(mol·K) = 1.33 J/(g·K), gas about 37.7 J/(mol·K) = 0.64 J/(g·K) (NIST-JANAF); approximate.
        "Sodium chloride" => Heats { c_solid: 1.13, c_liquid: 1.33, c_gas: 0.64, dh_fus: 28.16, dh_vap: 170.0, molar: 58.44 },
        // Substance X is made up for worksheets (it melts at 20 °C and boils at 90 °C): round numbers a student can work with.
        "Substance X" => Heats { c_solid: 1.5, c_liquid: 2.5, c_gas: 1.0, dh_fus: 10.0, dh_vap: 30.0, molar: 100.0 },
        _ => return None,
    })
}

/// The heats the computed curve uses: the named substance's, or the typed ones for Custom.
pub fn heats(p: &Params) -> Heats {
    table(p.s("substance")).unwrap_or(Heats {
        c_solid: p.f("c_solid"), c_liquid: p.f("c_liquid"), c_gas: p.f("c_gas"), dh_fus: p.f("dh_fus"), dh_vap: p.f("dh_vap"), molar: p.f("molar_mass"),
    })
}

/// The new params (appended to the figure's list). Their defaults are water's, a 100 g sample and 20 kJ a minute; none
/// acts until Segment lengths is "Computed from the heats".
pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("mass", "Mass (g)", 0.0, 10000.0, 1.0, 100.0)
            .hint("The sample's mass. q = m·c·ΔT for a sloped part and q = n·ΔH for a plateau, with n = m ÷ molar mass."),
        ParamDef::float("rate", "Heating rate (kJ/min)", 0.0, 10000.0, 0.5, 20.0)
            .hint("Heat put in (or taken out) each minute: each segment takes q ÷ rate minutes."),
        ParamDef::float("molar_mass", "Molar mass (g/mol)", 0.0, 1000.0, 0.01, 18.02),
        ParamDef::float("c_solid", "Specific heat, solid (J/g·K)", 0.0, 50.0, 0.01, 2.09),
        ParamDef::float("c_liquid", "Specific heat, liquid (J/g·K)", 0.0, 50.0, 0.01, 4.18),
        ParamDef::float("c_gas", "Specific heat, gas (J/g·K)", 0.0, 50.0, 0.01, 2.01),
        ParamDef::float("dh_fus", "Enthalpy of fusion (kJ/mol)", 0.0, 1000.0, 0.01, 6.01),
        ParamDef::float("dh_vap", "Enthalpy of vaporization (kJ/mol)", 0.0, 1000.0, 0.01, 40.7),
    ]
}

pub fn computed(p: &Params) -> bool { p.s("scale") == COMPUTED }
fn time_axis(p: &Params) -> bool { p.s("xaxis") != "Heat added" }

/// Whether a heating-curve control acts now: the computed mode's inputs only in that mode (the custom heats only for a
/// Custom substance, the rate only along a time axis), the typed minutes only in the typed mode.
pub fn relevant(key: &str, p: &Params) -> bool {
    let comp = computed(p);
    match key {
        "mass" => comp,
        "rate" => comp && time_axis(p),
        "molar_mass" | "c_solid" | "c_liquid" | "c_gas" | "dh_fus" | "dh_vap" => comp && p.s("substance") == "Custom",
        "t_solid" | "t_melt" | "t_liquid" | "t_boil" | "t_gas" => !comp, // Schematic reads them too (they set the minutes its time axis spans)
        _ => true,
    }
}

/// One part of the computed curve, in heating order: its phase, its temperatures and its heat (kJ).
#[derive(Clone, Copy, Debug)]
pub struct Seg { pub ph: Ph, pub t0: f64, pub t1: f64, pub q: f64 }

/// The computed curve in heating order (a cooling curve runs the same parts backwards), or why it cannot be drawn.
pub fn plan(p: &Params) -> Result<Vec<Seg>, String> {
    let custom = p.s("substance") == "Custom";
    let (mp, bp) = if custom { (p.f("mp"), p.f("bp")) } else { super::curves::heat_points(p) };
    if custom && mp >= bp {
        return Err(format!("The melting point ({} \u{b0}C) must be below the boiling point ({} \u{b0}C).", temp(mp), temp(bp)));
    }
    let m = p.f("mass");
    if !(m > 0.0) { return Err("The mass must be more than 0 g.".into()); }
    let (a, z) = (p.f("start"), p.f("end"));
    let cooling = p.s("mode") == "Cooling";
    if a == z { return Err("The starting and ending temperatures are the same, so no heat goes in or out.".into()); }
    if !cooling && a > z {
        return Err(format!("A heating curve starts below where it ends: {} \u{b0}C is above {} \u{b0}C. Swap them or pick Cooling.", temp(a), temp(z)));
    }
    if cooling && a < z {
        return Err(format!("A cooling curve starts above where it ends: {} \u{b0}C is below {} \u{b0}C. Swap them or pick Heating.", temp(a), temp(z)));
    }
    let (lo, hi) = (a.min(z), a.max(z));
    let h = heats(p);
    let n = if h.molar > 0.0 { m / h.molar } else { 0.0 };
    let mut v = Vec::new();
    // the same parts the typed curve draws, at the true temperatures
    if lo < mp { v.push((Ph::Solid, lo, mp.min(hi))); }
    if lo <= mp && hi > mp { v.push((Ph::Melt, mp, mp)); }
    let (l0, l1) = (lo.max(mp), hi.min(bp));
    if l1 > l0 && lo < bp && hi > mp { v.push((Ph::Liquid, l0, l1)); }
    if lo <= bp && hi > bp { v.push((Ph::Boil, bp, bp)); }
    if hi > bp { v.push((Ph::Gas, lo.max(bp), hi)); }
    let mut out = Vec::new();
    for (ph, t0, t1) in v {
        let (val, what) = match ph {
            Ph::Solid => (h.c_solid, "The solid's specific heat"),
            Ph::Liquid => (h.c_liquid, "The liquid's specific heat"),
            Ph::Gas => (h.c_gas, "The gas's specific heat"),
            Ph::Melt => (h.dh_fus, "The enthalpy of fusion"),
            Ph::Boil => (h.dh_vap, "The enthalpy of vaporization"),
        };
        if !(val > 0.0) { return Err(format!("{what} must be more than 0.")); }
        if matches!(ph, Ph::Melt | Ph::Boil) && !(h.molar > 0.0) { return Err("The molar mass must be more than 0 g/mol.".into()); }
        // q = m·c·ΔT (J, so ÷ 1000 for kJ); q = n·ΔH (kJ)
        let q = match ph { Ph::Melt | Ph::Boil => n * val, _ => m * val * (t1 - t0) / 1000.0 };
        out.push(Seg { ph, t0, t1, q });
    }
    let total: f64 = out.iter().map(|s| s.q).sum();
    let unit_total = if time_axis(p) {
        let r = p.f("rate");
        if !(r > 0.0) { return Err(format!("The {} rate must be more than 0 kJ/min.", if cooling { "cooling" } else { "heating" })); }
        total / r
    } else { total };
    let unit = if time_axis(p) { "min" } else { "kJ" };
    if unit_total < MIN_TOTAL {
        let len = if unit_total < 0.001 { "under 0.001".to_string() } else { sig(unit_total) };
        return Err(format!("The whole curve is {len} {unit}, too short to number along the x-axis: raise the mass{}.", if time_axis(p) { " or lower the rate" } else { "" }));
    }
    if unit_total > MAX_TOTAL {
        return Err(format!("The whole curve is over a million {unit}, too long to number along the x-axis: lower the mass{}.", if time_axis(p) { " or raise the rate" } else { "" }));
    }
    Ok(out)
}

/// What one part is called in a note, heating or cooling.
pub fn seg_name(ph: Ph, cooling: bool) -> &'static str {
    match (ph, cooling) {
        (Ph::Solid, false) => "Solid warms", (Ph::Solid, true) => "Solid cools",
        (Ph::Melt, false) => "Melting", (Ph::Melt, true) => "Freezing",
        (Ph::Liquid, false) => "Liquid warms", (Ph::Liquid, true) => "Liquid cools",
        (Ph::Boil, false) => "Boiling", (Ph::Boil, true) => "Condensing",
        (Ph::Gas, false) => "Gas warms", (Ph::Gas, true) => "Gas cools",
    }
}

/// A temperature as the panel writes it (a minus sign, no float noise).
pub fn temp(v: f64) -> String { super::num(v, if v.fract() == 0.0 { 0 } else { 1 }).replace('-', "\u{2212}") }

/// Three significant figures, no float noise and no exponent (0.0418, 4.18, 41.8, 226, 3090).
pub fn sig(v: f64) -> String {
    if v == 0.0 || !v.is_finite() { return "0".into(); }
    let mag = v.abs().log10().floor() as i32;
    let places = (2 - mag).max(0) as usize;
    let r = format!("{:.*}", places, v);
    let r = if r.contains('.') { r.trim_end_matches('0').trim_end_matches('.').to_string() } else { r };
    if places == 0 {
        // round to three figures (4093 -> 4090)
        let f = 10f64.powi(mag - 2);
        return format!("{}", ((v / f).round() * f) as i64);
    }
    r
}

/// The heats, the times and any segment too short to see, read off the frame the figure draws.
pub struct Reading { pub rows: Vec<(Ph, f64, f64)>, pub short: Vec<Ph>, pub past_end: Option<(f64, f64)> }

/// Each drawn part's heat (kJ) and length along the x-axis (in its unit), from the frame the renderer uses.
pub fn reading(p: &Params) -> Option<Reading> {
    if !computed(p) { return None; }
    let hf = super::curves::heat_frame(p);
    if hf.note.is_some() { return None; }
    let rate = if time_axis(p) { p.f("rate") } else { 1.0 };
    let g = &hf.g;
    let px_per = (g.x1 - g.x0) / (g.x.to - g.x.from).max(1e-12);
    let mut rows = Vec::new();
    let mut short = Vec::new();
    for s in &hf.seq {
        let len = s.3 * hf.k;
        rows.push((s.0, len * rate, len));
        if len * px_per < MIN_SEEN_PX { short.push(s.0); }
    }
    let total: f64 = hf.seq.iter().map(|s| s.3 * hf.k).sum();
    let past_end = if total > g.x.to + 1e-9 { Some((total, g.x.to)) } else { None };
    Some(Reading { rows, short, past_end })
}

/// The panel's notes for the heating curve: why a computed curve is refused, and what is drawn but hard to see.
pub fn warnings(p: &Params) -> Vec<String> {
    let mut v = Vec::new();
    if !computed(p) { return v; }
    if let Err(e) = plan(p) { v.push(format!("Not drawn: {e}")); return v; }
    let cooling = p.s("mode") == "Cooling";
    let unit = if time_axis(p) { "min" } else { "kJ" };
    if let Some(r) = reading(p) {
        for ph in &r.short {
            let row = r.rows.iter().find(|q| q.0 == *ph).unwrap();
            v.push(format!("{} is {} {unit} against {} {unit} for the whole curve, under {MIN_SEEN_PX} px wide at this scale: it is drawn to scale but hard to see.",
                seg_name(*ph, cooling), sig(row.2), sig(r.rows.iter().map(|q| q.2).sum::<f64>())));
        }
        if p.b("labels") && p.s("seg_labels") != "None" {
            let off = super::curves::heat_draw(p).1;
            for ph in off.parts {
                v.push(format!("{} is too short at this scale to label clear of the curve: its label is left off.", seg_name(ph, cooling)));
            }
            for ph in off.temps {
                let what = match (ph, cooling) { (Ph::Melt, false) => "melting point", (Ph::Melt, true) => "freezing point", (_, false) => "boiling point", (_, true) => "condensing point" };
                v.push(format!("No room for the {what}'s label beside the corner letters: it is left off."));
            }
        } else if p.b("letters") && p.b("labels") {
            for ph in super::curves::heat_draw(p).1.temps {
                let what = match (ph, cooling) { (Ph::Melt, false) => "melting point", (Ph::Melt, true) => "freezing point", (_, false) => "boiling point", (_, true) => "condensing point" };
                v.push(format!("No room for the {what}'s label beside the corner letters: it is left off."));
            }
        }
        if let Some((t, end)) = r.past_end {
            v.push(format!("The curve runs to {} {unit}, past the x-axis's end at {}: turn on Fit the axes to the curve or raise To.", sig(t), super::tick(end)));
        }
    }
    v
}

/// The note under Segment lengths in the computed mode: the formulas, and each part's heat and length as drawn.
pub fn scale_note(p: &Params) -> Option<String> {
    if !computed(p) { return None; }
    let h = heats(p);
    let cooling = p.s("mode") == "Cooling";
    let mut s = "Each sloped part takes q = m·c·ΔT and each plateau q = n·ΔH (n = m ÷ M)".to_string();
    s.push_str(if time_axis(p) { "; at the heating rate it takes q ÷ rate minutes." } else { "." });
    if p.s("substance") != "Custom" {
        s.push_str(&format!(" {}: c = {} (solid), {} (liquid), {} (gas) J/g·K; \u{394}Hfus {} and \u{394}Hvap {} kJ/mol; M = {} g/mol.",
            p.s("substance"), h.c_solid, h.c_liquid, h.c_gas, h.dh_fus, h.dh_vap, h.molar));
    }
    if let Some(r) = reading(p) {
        let rows: Vec<String> = r.rows.iter().map(|(ph, q, len)| {
            if time_axis(p) { format!("{} {} kJ, {} min", seg_name(*ph, cooling), sig(*q), sig(*len)) } else { format!("{} {} kJ", seg_name(*ph, cooling), sig(*q)) }
        }).collect();
        s.push_str(&format!(" {}.", rows.join(" · ")));
    }
    Some(s)
}

/// Copies a named substance's heats into the Custom boxes (a drag that turns the substance into Custom keeps them).
pub fn carry_heats(p: &mut Params) {
    if let Some(h) = table(p.s("substance")) {
        for (k, v) in [("c_solid", h.c_solid), ("c_liquid", h.c_liquid), ("c_gas", h.c_gas), ("dh_fus", h.dh_fus), ("dh_vap", h.dh_vap), ("molar_mass", h.molar)] { p.set_f(k, v); }
    }
}

/// The figure's own note when a computed curve is refused: the axes stay, no curve, the reason in words across the plot.
pub fn draw_refusal(d: &mut crate::figures::svg::Doc, g: &super::Graph, why: &str) {
    let size = 15.4;
    // wrapped at the size it is drawn (the label size scales the type afterwards)
    let per = ((g.x1 - g.x0 - 40.0) / (size * g.lf * 0.6)).max(12.0) as usize;
    let mut lines: Vec<String> = vec!["Not drawn to scale:".into()];
    let mut cur = String::new();
    for w in why.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > per { lines.push(std::mem::take(&mut cur)); }
        if !cur.is_empty() { cur.push(' '); }
        cur.push_str(w);
    }
    if !cur.is_empty() { lines.push(cur); }
    let pitch = size * g.lf * 1.35;
    let y0 = (g.y0 + g.y1) / 2.0 - pitch * (lines.len() as f64 - 1.0) / 2.0;
    for (i, l) in lines.iter().enumerate() {
        d.raw(&format!(r##"<text x="{:.2}" y="{:.2}" font-size="{size}" text-anchor="middle" font-family="{}" font-weight="{}" fill="{}" stroke="#fff" stroke-width="4" paint-order="stroke">{}</text>"##,
            (g.x0 + g.x1) / 2.0, y0 + pitch * i as f64, super::FONT, if i == 0 { "bold" } else { "normal" }, super::INK, crate::figures::svg::esc(l)));
    }
}

#[cfg(test)]
#[path = "heatcurve_b8_tests.rs"]
mod tests;
