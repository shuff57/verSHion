//! Distance–energy diagrams for teaching bond length and dissociation energy.

use super::frame::{NUMS, TITLE3, answer_param};
use super::ratesgraph::{halo_el, text_w};
use crate::figures::chart::{self, AxesOpts, Frame, INK, Tick, fmt, nice_step};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{Example, FigureDef, ParamDef, Params, Subject, Value};

pub const ID: &str = "bond-energy-curve";

const NAMES: [&str; 3] = ["Bond X", "Bond Y", "Bond Z"];
const LETTERS: [&str; 3] = ["Bond A", "Bond B", "Bond C"];
const NAME_KEYS: [&str; 3] = ["name1", "name2", "name3"];
const LENGTH_KEYS: [&str; 3] = ["length1", "length2", "length3"];
const ENERGY_KEYS: [&str; 3] = ["energy1", "energy2", "energy3"];
const X_KEYS: [&str; 3] = ["x_from", "x_to", "x_by"];
const Y_KEYS: [&str; 3] = ["y_from", "y_to", "y_by"];
const GUIDE_LABELS: [&str; 4] = ["Values", "Bond length and energy", "Blank lines", "None"];
const REGIONS: [&str; 3] = ["None", "Words", "Blank lines"];
const DASHES: [&str; 3] = ["", "9 5", "2 4"];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Bond Energy Curve",
        blurb: "Plot potential energy against internuclear distance for up to three bonds. Mark the minima to teach bond length and dissociation energy.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    let mut out = vec![
        ParamDef::int("count", "Bonds shown", 1, 3, 1),
        ParamDef::choice("length_unit", "Lengths in", &["pm", "Å", "nm"], "pm"),
        ParamDef::choice(
            "energy_unit",
            "Energies in",
            &["kJ/mol", "kJ mol⁻¹"],
            "kJ/mol",
        ),
    ];
    for i in 0..3 {
        out.extend([
            ParamDef::text(
                NAME_KEYS[i],
                ["Bond 1 name", "Bond 2 name", "Bond 3 name"][i],
                NAMES[i],
            ),
            ParamDef::float(
                LENGTH_KEYS[i],
                "Bond length (pm)",
                1.0,
                1000.0,
                0.1,
                [150.0, 180.0, 120.0][i],
            ),
            ParamDef::float(
                ENERGY_KEYS[i],
                "Bond energy (kJ/mol)",
                1.0,
                2000.0,
                0.1,
                [300.0, 200.0, 400.0][i],
            )
            .hint("Enter a positive dissociation energy. The minimum is that far below zero."),
        ]);
    }
    out.extend([
        ParamDef::bool("fit", "Fit the axes to the curves", true)
            .hint("Off lets you set the axis ranges and tick spacing."),
        ParamDef::choice("axes", "Axes", &["Numbered", "Shape only"], "Numbered"),
        ParamDef::float("x_from", "x-axis start", 0.0, 10_000.0, 0.1, 0.0),
        ParamDef::float("x_to", "x-axis end", 1.0, 10_000.0, 0.1, 450.0),
        ParamDef::float("x_by", "x-axis step", 0.1, 10_000.0, 0.1, 50.0),
        ParamDef::choice("x_num", "Number the x-axis", &NUMS, "Every line"),
        ParamDef::float(
            "y_from",
            "y-axis start (kJ/mol)",
            -10_000.0,
            10_000.0,
            0.1,
            -350.0,
        ),
        ParamDef::float(
            "y_to",
            "y-axis end (kJ/mol)",
            -10_000.0,
            10_000.0,
            0.1,
            100.0,
        ),
        ParamDef::float("y_by", "y-axis step (kJ/mol)", 0.1, 10_000.0, 0.1, 50.0),
        ParamDef::choice("y_num", "Number the y-axis", &NUMS, "Every line"),
        ParamDef::bool("grid", "Gridlines", true),
        ParamDef::bool("guides", "Guide lines from minima to axes", false),
        ParamDef::choice("guide_labels", "Guide labels", &GUIDE_LABELS, "Values"),
        ParamDef::bool("dots", "Dots at the minima", false),
        ParamDef::bool("hide_names", "Hide bond names", false)
            .hint("Use Bond A, B and C in the legend. The answer key gives the matching names."),
        ParamDef::choice("regions", "Repulsion and attraction", &REGIONS, "None"),
        ParamDef::choice("title_style", "Chart title", &TITLE3, "Text"),
        ParamDef::text("title", "Chart title text", ""),
        ParamDef::choice("xtitle_mode", "x-axis title", &TITLE3, "Text"),
        ParamDef::text("xtitle", "x-axis title text", "")
            .hint("Leave empty for Internuclear distance."),
        ParamDef::choice("ytitle_mode", "y-axis title", &TITLE3, "Text"),
        ParamDef::text("ytitle", "y-axis title text", "").hint("Leave empty for Potential energy."),
        answer_param("Show bond names, lengths and dissociation energies in the legend."),
        svg::style_param().hint("Print uses black curves distinguished by their line patterns."),
        svg::label_size_param(),
    ]);
    out
}

fn bond_count(p: &Params) -> usize {
    p.i("count").clamp(1, 3) as usize
}

pub fn relevant(key: &str, p: &Params) -> bool {
    for i in bond_count(p)..3 {
        if [NAME_KEYS[i], LENGTH_KEYS[i], ENERGY_KEYS[i]].contains(&key) {
            return false;
        }
    }
    match key {
        "regions" => bond_count(p) == 1,
        "guide_labels" => p.b("guides"),
        "x_num" | "y_num" | "grid" => p.s("axes") != "Shape only",
        "x_from" | "x_to" | "x_by" | "y_from" | "y_to" | "y_by" => !p.b("fit"),
        "title" => p.s("title_style") == "Text",
        "xtitle" => p.s("xtitle_mode") == "Text",
        "ytitle" => p.s("ytitle_mode") == "Text",
        _ => true,
    }
}

// Keep lengths and horizontal ranges in pm in saved state. The editor and axis
// labels convert only at display and input, so changing units leaves the well fixed.
fn length_unit(p: &Params) -> (&'static str, f64) {
    match p.s("length_unit") {
        "Å" => ("Å", 100.0),
        "nm" => ("nm", 1000.0),
        _ => ("pm", 1.0),
    }
}

fn energy_unit(p: &Params) -> &'static str {
    if p.s("energy_unit") == "kJ mol⁻¹" {
        "kJ mol⁻¹"
    } else {
        "kJ/mol"
    }
}

fn is_length(key: &str) -> bool {
    LENGTH_KEYS.contains(&key) || X_KEYS.contains(&key)
}

pub fn control_label(key: &str, p: &Params) -> Option<String> {
    let name = match key {
        "length1" | "length2" | "length3" => "Bond length",
        "x_from" => "x-axis start",
        "x_to" => "x-axis end",
        "x_by" => "x-axis step",
        "energy1" | "energy2" | "energy3" => "Bond energy",
        "y_from" => "y-axis start",
        "y_to" => "y-axis end",
        "y_by" => "y-axis step",
        _ => return None,
    };
    let unit = if is_length(key) {
        length_unit(p).0
    } else {
        energy_unit(p)
    };
    Some(format!("{name} ({unit})"))
}

pub fn float_text(key: &str, p: &Params) -> Option<String> {
    is_length(key).then(|| fmt(p.f(key) / length_unit(p).1))
}

pub fn typed_float(key: &str, text: &str, p: &Params) -> Option<f64> {
    let v = text.trim().parse::<f64>().ok()?;
    v.is_finite().then(|| {
        if is_length(key) {
            v * length_unit(p).1
        } else {
            v
        }
    })
}

struct Bond<'a> {
    name: &'a str,
    length: f64,
    depth: f64,
}

impl Bond<'_> {
    // A Morse potential with a fixed dimensionless width for textbook diagrams.
    // exp_m1 evaluates to exactly zero at the equilibrium length.
    const WIDTH: f64 = 3.0;

    fn energy(&self, r: f64) -> f64 {
        let displacement = Self::WIDTH * (1.0 - r / self.length);
        self.depth * (displacement.exp_m1().powi(2) - 1.0)
    }

    fn repulsive_crossing(&self, energy: f64) -> f64 {
        self.length * (1.0 - (1.0 + energy / self.depth).sqrt().ln_1p() / Self::WIDTH)
    }

    fn values(&self, p: &Params) -> (String, String) {
        let (unit, scale) = length_unit(p);
        (
            format!("{} {unit}", fmt(self.length / scale)),
            format!("{} {}", fmt(self.depth), energy_unit(p)),
        )
    }
}

fn bonds(p: &Params) -> Vec<Bond<'_>> {
    (0..bond_count(p))
        .map(|i| {
            let name = p.s(NAME_KEYS[i]).trim();
            Bond {
                name: if name.is_empty() { NAMES[i] } else { name },
                length: bounded(p.f(LENGTH_KEYS[i]), 1.0, 1000.0, 150.0),
                depth: bounded(p.f(ENERGY_KEYS[i]), 1.0, 2000.0, 300.0),
            }
        })
        .collect()
}

fn bounded(v: f64, lo: f64, hi: f64, fallback: f64) -> f64 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        fallback
    }
}

pub fn readout(p: &Params) -> String {
    bonds(p)
        .iter()
        .enumerate()
        .map(|(i, bond)| {
            let (length, energy) = bond.values(p);
            let name = if p.b("hide_names") {
                format!("{} is {}", LETTERS[i], bond.name)
            } else {
                bond.name.to_string()
            };
            format!("{name}: bond length {length}, bond energy {energy}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn fitted_axes(bonds: &[Bond<'_>]) -> ([f64; 3], [f64; 3]) {
    let longest = bonds.iter().map(|b| b.length).fold(0.0, f64::max);
    let deepest = bonds.iter().map(|b| b.depth).fold(0.0, f64::max);
    let x_extent = 3.0 * longest;
    let x_step = nice_step(x_extent, 11.0);
    let y_step = nice_step(1.25 * deepest, 10.0);
    let x_end = (x_extent / x_step).ceil() * x_step;
    let y_start = -((deepest / y_step).floor() + 1.0) * y_step;
    let y_end = (0.25 * deepest / y_step).ceil() * y_step;
    ([0.0, x_end, x_step], [y_start, y_end, y_step])
}

pub fn panel_changed(key: &str, old: &Value, p: &mut Params) {
    if key == "fit" && matches!(old, Value::B(true)) && !p.b("fit") {
        let (x, y) = fitted_axes(&bonds(p));
        for (key, value) in X_KEYS.into_iter().zip(x).chain(Y_KEYS.into_iter().zip(y)) {
            p.set_f(key, value);
        }
    }
}

fn ranges(p: &Params, bonds: &[Bond<'_>]) -> ([f64; 3], [f64; 3]) {
    let (mut x, mut y) = fitted_axes(bonds);
    if !p.b("fit") {
        let xf = bounded(p.f("x_from"), 0.0, 10_000.0, x[0]);
        let xt = bounded(p.f("x_to"), 1.0, 10_000.0, x[1]);
        let yf = bounded(p.f("y_from"), -10_000.0, 10_000.0, y[0]);
        let yt = bounded(p.f("y_to"), -10_000.0, 10_000.0, y[1]);
        // Degenerate manual ranges fall back to the corresponding fitted axis.
        if xt - xf >= 0.1 {
            x = [xf, xt, bounded(p.f("x_by"), 0.1, 10_000.0, x[2])];
        }
        if yt - yf >= 0.1 {
            y = [yf, yt, bounded(p.f("y_by"), 0.1, 10_000.0, y[2])];
        }
    }
    (x, y)
}

fn numbering_interval(numbering: &str) -> usize {
    match numbering {
        "Every 2nd line" => 2,
        "Every 5th line" => 5,
        "Every 10th line" => 10,
        "No numbers" => 0,
        _ => 1,
    }
}

fn effective_tick_step([from, to, by]: [f64; 3], every: usize) -> f64 {
    by.max(nice_step(to - from, 20.0 * every.clamp(1, 4) as f64))
}

pub fn warnings(p: &Params) -> Vec<String> {
    if p.b("fit") || p.s("axes") == "Shape only" {
        return Vec::new();
    }
    let (x, y) = ranges(p, &bonds(p));
    let (x_unit, scale) = length_unit(p);
    [("x", x, "x_num", x_unit, scale), ("y", y, "y_num", energy_unit(p), 1.0)]
        .into_iter()
        .filter_map(|(axis, range, key, unit, scale)| {
            let step = effective_tick_step(range, numbering_interval(p.s(key)));
            (step > range[2]).then(|| format!(
                "The {axis}-axis uses {} {unit} tick spacing because the requested {} {unit} step is too dense.",
                fmt(step / scale), fmt(range[2] / scale)
            ))
        })
        .collect()
}

fn ticks(range @ [from, to, _]: [f64; 3], scale: f64, numbering: &str) -> Vec<Tick> {
    let every = numbering_interval(numbering);
    let by = effective_tick_step(range, every);
    chart::ticks(from, to, by)
        .into_iter()
        .enumerate()
        .map(|(i, (v, _))| {
            let label = if every > 0 && i % every == 0 {
                fmt(v / scale)
            } else {
                String::new()
            };
            (v, label)
        })
        .collect()
}

fn axis_title(mode: &str, text: &str, default: String) -> String {
    match mode {
        "None" => String::new(),
        "Blank line" => "________________".into(),
        _ if text.trim().is_empty() => default,
        _ => text.trim().into(),
    }
}

fn series(doc: &mut Doc, points: &[(f64, f64)], fills: &Fills, i: usize) {
    let dash = if fills.is_print() {
        fills.dash(i)
    } else {
        DASHES[i]
    };
    if !dash.is_empty() {
        doc.raw(&format!(r#"<g stroke-dasharray="{dash}">"#));
    }
    doc.polyline(points, fills.line(i), 2.8);
    if !dash.is_empty() {
        doc.raw("</g>");
    }
}

fn curve(doc: &mut Doc, frame: &Frame, bond: &Bond<'_>, fills: &Fills, i: usize) {
    if frame.y1 <= -bond.depth {
        return;
    }
    let start = frame.x0.max(bond.repulsive_crossing(frame.y1));
    if start >= frame.x1 {
        return;
    }
    // Include the exact minimum as a vertex and sample the well independently
    // of the long tail. A short bond stays resolved on a wide comparison plot.
    let stop = frame.x1;
    let anchors = [
        start,
        bond.length.clamp(start, stop),
        (3.0 * bond.length).clamp(start, stop),
        stop,
    ];
    let mut points = vec![(frame.px(start), frame.py(bond.energy(start)))];
    for segment in anchors.windows(2).filter(|pair| pair[0] < pair[1]) {
        for step in 1..=100 {
            let r = if step == 100 {
                segment[1]
            } else {
                segment[0] + (segment[1] - segment[0]) * step as f64 / 100.0
            };
            points.push((frame.px(r), frame.py(bond.energy(r))));
        }
    }
    series(doc, &points, fills, i);
}

fn legend(doc: &mut Doc, p: &Params, bonds: &[Bond<'_>], fills: &Fills, lf: f64) {
    let col = 650.0 / bonds.len() as f64;
    for (i, bond) in bonds.iter().enumerate() {
        let x = 35.0 + col * i as f64;
        let label = if p.b("hide_names") {
            LETTERS[i]
        } else {
            bond.name
        };
        series(doc, &[(x, 57.0), (x + 27.0, 57.0)], fills, i);
        let name = if p.b("answer") && p.b("hide_names") {
            format!("{label} = {}", bond.name)
        } else {
            label.to_string()
        };
        let font = (13.0 * lf).min((col - 40.0) / (name.chars().count() as f64 * 0.59));
        doc.text(x + 34.0, 61.0, font, "start", &name);
        if p.b("answer") {
            let (length, energy) = bond.values(p);
            doc.text(
                x + 34.0,
                80.0,
                12.0 * lf,
                "start",
                &format!("{length} · {energy}"),
            );
        }
    }
}

fn render(p: &Params) -> String {
    let bonds = bonds(p);
    let fills = Fills::new(p);
    let (x, y) = ranges(p, &bonds);
    let (lu, scale) = length_unit(p);
    let eu = energy_unit(p);
    let shape = p.s("axes") == "Shape only";
    let lf = svg::label_factor(p.s("label_size"));
    let frame = Frame {
        w: 720.0,
        h: 490.0,
        l: 126.0,
        r: 49.0,
        t: 99.0,
        b: 111.0,
        x0: x[0],
        x1: x[1],
        y0: y[0],
        y1: y[1],
    };
    let (l, r, t, b) = (frame.left(), frame.right(), frame.top(), frame.bottom());
    let mut doc = Doc::new(frame.w, frame.h);
    doc.defs(&format!(
        r#"<clipPath id="bond-plot"><rect x="{l}" y="{t}" width="{}" height="{}"/></clipPath>"#,
        r - l,
        b - t
    ));

    let xt = if shape {
        vec![]
    } else {
        ticks(x, scale, p.s("x_num"))
    };
    let yt = if shape {
        if frame.y0 <= 0.0 && frame.y1 >= 0.0 {
            vec![(0.0, "0".into())]
        } else {
            vec![]
        }
    } else {
        ticks(y, 1.0, p.s("y_num"))
    };
    if p.b("grid") && !shape {
        for (v, _) in &xt {
            let x = frame.px(*v);
            doc.line(x, t, x, b, chart::GRID, 1.0);
        }
    }
    let default_xt = if shape {
        "Internuclear distance".into()
    } else {
        format!("Internuclear distance ({lu})")
    };
    let default_yt = if shape {
        "Potential energy".into()
    } else {
        format!("Potential energy ({eu})")
    };
    let xtitle = axis_title(p.s("xtitle_mode"), p.s("xtitle"), default_xt);
    let ytitle = axis_title(p.s("ytitle_mode"), p.s("ytitle"), default_yt);
    chart::axes(
        &mut doc,
        &frame,
        &AxesOpts {
            xticks: &xt,
            yticks: &yt,
            grid: p.b("grid") && !shape,
            x_title: &xtitle,
            y_title: &ytitle,
            fs: 12.0 * lf,
        },
    );
    if shape {
        doc.arrow(r - 11.0, b, r + 12.0, b, INK, 2.0);
        doc.arrow(l, t + 11.0, l, t - 12.0, INK, 2.0);
    }
    if frame.y0 < 0.0 && frame.y1 >= 0.0 {
        let zero = frame.py(0.0);
        doc.line(l, zero, r, zero, "#898989", 1.1);
    }

    let visible_minima: Vec<_> = bonds
        .iter()
        .enumerate()
        .filter(|(_, bond)| {
            (frame.x0..=frame.x1).contains(&bond.length)
                && (frame.y0..=frame.y1).contains(&-bond.depth)
        })
        .collect();
    if p.b("guides") {
        for (_, bond) in &visible_minima {
            let x = frame.px(bond.length);
            let y = frame.py(-bond.depth);
            super::dashed(&mut doc, x, y, l, y, "#595959", 1.3);
            super::dashed(&mut doc, x, y, x, b, "#595959", 1.3);
        }
    }
    // Where each horizontal guide ends (x of its minimum) and its height, for the label halo test below.
    let guides: Vec<(f64, f64)> = if p.b("guides") {
        visible_minima.iter().map(|(_, bond)| (frame.px(bond.length), frame.py(-bond.depth))).collect()
    } else {
        Vec::new()
    };
    doc.raw(r#"<g clip-path="url(#bond-plot)">"#);
    for (i, bond) in bonds.iter().enumerate() {
        curve(&mut doc, &frame, bond, &fills, i);
    }
    doc.raw("</g>");
    let mut energy_labels = Vec::new();
    for (i, bond) in visible_minima {
        let x = frame.px(bond.length);
        let y = frame.py(-bond.depth);
        if p.b("dots") {
            doc.circle(x, y, 4.6, fills.line(i), "#fff", 1.2);
        }
        if p.b("guides") {
            let (lx, ly) = match p.s("guide_labels") {
                "Values" => {
                    let (length, energy) = bond.values(p);
                    (length, format!("−{energy}"))
                }
                "Bond length and energy" => ("Bond length".into(), "Bond energy".into()),
                "Blank lines" => ("________".into(), "________".into()),
                _ => continue,
            };
            doc.text(x, b + 41.0 + i as f64 * 17.0, 11.0 * lf, "middle", &lx);
            energy_labels.push((y, ly));
        }
    }
    energy_labels.sort_by(|a, b| a.0.total_cmp(&b.0));
    let gap = 14.0 * lf;
    let mut next_baseline = t + 13.0 * lf;
    let mut positioned = Vec::with_capacity(energy_labels.len());
    for (guide_y, label) in energy_labels {
        let target = (guide_y - 6.0).max(t + 13.0 * lf);
        let baseline = target.max(next_baseline);
        next_baseline = baseline + gap;
        positioned.push((guide_y, target, baseline, label));
    }
    let overflow = positioned.last().map_or(0.0, |(_, _, y, _)| (y - b).max(0.0));
    for (i, &(guide_y, target, baseline, ref label)) in positioned.iter().enumerate() {
        let baseline = baseline - overflow;
        if (baseline - target).abs() > 0.5 {
            doc.line(l + 1.0, guide_y, l + 6.0, baseline - 4.0, "#595959", 0.8);
        }
        // A label pushed off its own guide can land on a neighbour's (or still on its own): give any label a
        // dashed guide crosses the white halo the other chemistry plots use, so the text is never struck through.
        let fs = 11.0 * lf;
        let (top, bottom, right) = (baseline - 0.78 * fs, baseline + 0.25 * fs, l + 7.0 + text_w(label, fs));
        let struck = guides.iter().any(|&(gx, gy)| (top..=bottom).contains(&gy) && gx > l + 7.0 && l < right);
        if struck {
            doc.raw(&halo_el(l + 7.0, baseline, fs, "start", "#000", label, "guide-label", i));
        } else {
            doc.text(l + 7.0, baseline, fs, "start", label);
        }
    }
    if bonds.len() == 1 && p.s("regions") != "None" {
        let x = frame.px(bonds[0].length).clamp(l, r);
        for (label, a, z) in [("Repulsion", l, x), ("Attraction", x, r)] {
            if z - a > 45.0 {
                let text = if p.s("regions") == "Blank lines" {
                    "________"
                } else {
                    label
                };
                let font = (13.0 * lf).min((z - a - 10.0) / 6.0);
                doc.text((a + z) / 2.0, t + 22.0, font, "middle", text);
            }
        }
    }
    let title = axis_title(p.s("title_style"), p.s("title"), String::new());
    if !title.is_empty() {
        let font = (19.0 * lf).min(670.0 / (title.chars().count() as f64 * 0.59));
        doc.text(frame.w / 2.0, 28.0, font, "middle", &title);
    }
    legend(&mut doc, p, &bonds, &fills, lf);
    doc.finish()
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID {
        return None;
    }
    Some(vec![
        Example {
            name: "Compare Two Bonds",
            params: vec![
                ("count", Value::I(2)),
                ("length1", Value::F(120.0)),
                ("energy1", Value::F(400.0)),
                ("length2", Value::F(180.0)),
                ("energy2", Value::F(200.0)),
                ("guides", Value::B(true)),
                ("dots", Value::B(true)),
            ],
        },
        Example {
            name: "Repulsion / Attraction",
            params: vec![
                ("axes", Value::S("Shape only".into())),
                ("regions", Value::S("Words".into())),
            ],
        },
    ])
}

#[cfg(test)]
mod tests;
