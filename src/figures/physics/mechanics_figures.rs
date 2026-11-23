//! Worksheet diagrams for statics, oscillations and energy.
use super::common::{G, Pt};
use crate::figures::{Example, FigureDef, ParamDef, Params, SHOW_ANSWERS, Subject, Value, svg};

mod coaster;
mod pendulum;
mod spring;
mod statics;

const INK: &str = "#202020";
const GUIDE: &str = "#757575";
const W: f64 = 680.0;
const G_STANDARD: f64 = 9.8;

pub(super) fn defs() -> Vec<FigureDef> {
    vec![
        statics::lever(),
        pendulum::def(),
        coaster::def(),
        spring::def(),
        statics::torque(),
    ]
}

pub(super) fn examples(id: &str) -> Option<Vec<Example>> {
    let (name, values) = match id {
        "lever" => (
            "Wheelbarrow",
            vec![
                ("class", s("Class 2")),
                ("load", Value::F(450.0)),
                ("title", s("Wheelbarrow")),
            ],
        ),
        "pendulum" => (
            "Release forces",
            vec![("picture", s("One bob")), ("forces", s("Components"))],
        ),
        "roller-coaster" => (
            "Loop after the first drop",
            vec![
                ("loop_at", Value::I(2)),
                ("loop_radius", Value::F(8.0)),
                ("velocity", s("Forward")),
            ],
        ),
        "spring" => (
            "Compressed springs in parallel",
            vec![
                ("orientation", s("Horizontal")),
                ("arrangement", s("2 in parallel")),
                ("extension", Value::F(-30.0)),
            ],
        ),
        "torque" => (
            "Hanging sign and cable",
            vec![
                ("support", s("Wall")),
                ("cable", Value::B(true)),
                ("title", s("Hanging sign")),
            ],
        ),
        _ => return None,
    };
    let mut list = vec![Example {
        name,
        params: values,
    }];
    if id == "lever" {
        list.push(Example {
            name: "Forearm",
            params: vec![
                ("class", s("Class 3")),
                ("effort_arm", Value::F(0.2)),
                ("load_arm", Value::F(0.6)),
            ],
        });
    }
    Some(list)
}

fn s(value: &str) -> Value {
    Value::S(value.into())
}

fn presentation(mut params: Vec<ParamDef>, answers: bool) -> Vec<ParamDef> {
    params.push(ParamDef::text("title", "Title", ""));
    if answers {
        let mut switch = crate::figures::answers_param();
        switch.default = Value::B(false);
        params.push(switch);
    }
    let mut style = svg::style_param();
    style.default = s("Print");
    params.extend([style, svg::label_size_param()]);
    params
}

fn canvas(p: &Params, height: f64, title: &str, description: &str) -> (G, svg::Fills) {
    let mut g = G::new(W, height, p.b("mirror"));
    let fills = svg::Fills::new(p);
    g.d.describe(title, description).defs(&fills.defs());
    super::common::draw_title(&mut g, p, W / 2.0, 29.0);
    (g, fills)
}

fn finish(g: G, p: &Params) -> String {
    svg::with_label_size(g.finish(), p)
}

// Use a dash for a student writing line and the word "none" to omit a label.
fn label(p: &Params, key: &str, auto: &str) -> String {
    let text = p.s(key).trim();
    match text {
        "" => auto.into(),
        "-" => "________".into(),
        text if text.eq_ignore_ascii_case("none") => String::new(),
        _ => text.into(),
    }
}

const LABEL_HINT: &str =
    "Empty uses the default, a dash draws a blank line, and None hides the label.";
fn label_param(key: &'static str, title: &'static str) -> ParamDef {
    ParamDef::text(key, title, "").hint(LABEL_HINT)
}

fn text(g: &mut G, (x, y): Pt, size: f64, anchor: &str, value: &str) {
    if !value.is_empty() {
        let max_width = match g.anc(anchor) {
            "start" => W - g.x(x) - 8.0,
            "end" => g.x(x) - 8.0,
            _ => 2.0 * g.x(x).min(W - g.x(x)) - 16.0,
        };
        let size = size
            .min(max_width / (0.62 * value.chars().count() as f64))
            .max(5.0);
        g.d.text(g.x(x), y, size, g.anc(anchor), value);
    }
}

fn num(v: f64) -> String {
    if v.abs() < 1e-10 {
        return "0".into();
    }
    let decimals = (2.0 - v.abs().log10().floor()).max(0.0) as usize;
    let factor = 10.0_f64.powi(2 - v.abs().log10().floor() as i32);
    let rounded = (v * factor).round() / factor;
    let value = format!("{rounded:.decimals$}");
    if decimals == 0 {
        value
    } else {
        value.trim_end_matches('0').trim_end_matches('.').into()
    }
}

fn support(g: &mut G, x: f64, y: f64, half: f64, fill: &str) {
    g.poly(
        &[(x, y), (x - half, y + 30.0), (x + half, y + 30.0)],
        fill,
        INK,
        2.0,
    );
    g.line(x - half - 5.0, y + 30.0, x + half + 5.0, y + 30.0, INK, 2.0);
}

fn bracket(g: &mut G, from: Pt, to: Pt) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len = dx.hypot(dy);
    if len < 0.01 {
        return;
    }
    let (nx, ny) = (-4.0 * dy / len, 4.0 * dx / len);
    g.line(from.0, from.1, to.0, to.1, GUIDE, 1.25);
    for (x, y) in [from, to] {
        g.line(x - nx, y - ny, x + nx, y + ny, GUIDE, 1.25);
    }
}

fn object(g: &mut G, (x, y): Pt, half: f64, shape: &str, fill: &str) {
    if shape == "Ball" {
        g.d.circle(g.x(x), y, half, fill, INK, 2.0);
    } else {
        g.rect(x - half, y - half, half * 2.0, half * 2.0, fill, INK, 2.0);
    }
}

pub(crate) fn lever_class_changed(p: &mut Params) {
    let (effort, load) = (p.f("effort_arm"), p.f("load_arm"));
    let (short, long) = (effort.min(load), effort.max(load));
    let short = if short == long {
        (short * 0.5).max(0.1)
    } else {
        short
    };
    let long = if long == short {
        (short * 2.0).min(999.0)
    } else {
        long
    };
    match p.s("class") {
        "Class 2" => {
            p.set_f("effort_arm", long);
            p.set_f("load_arm", short);
        }
        "Class 3" => {
            p.set_f("effort_arm", short);
            p.set_f("load_arm", long);
        }
        _ => {}
    }
}

pub(crate) fn relevant(id: &str, key: &str, p: &Params) -> bool {
    match (id, key) {
        ("lever", "effort_arm_label" | "load_arm_label") => p.b("show_arms"),
        ("pendulum", "angle_label") => p.b("show_angle"),
        ("pendulum", "height_label") => p.b("show_height"),
        ("pendulum", "show_letters" | "show_velocity") => p.s("picture") == "Swing",
        ("pendulum", "velocity_label") => p.s("picture") == "Swing" && p.b("show_velocity"),
        ("pendulum", "forces") => p.s("picture") == "One bob",
        ("pendulum", "weight_label" | "tension_label") => {
            p.s("picture") == "One bob" && p.s("forces") != "None"
        }
        ("pendulum", "radial_label" | "tangent_label") => {
            p.s("picture") == "One bob" && p.s("forces") == "Components"
        }
        ("roller-coaster", "loop_radius" | "radius_label" | "loop_top_label") => p.i("loop_at") > 0,
        ("roller-coaster", "object_on_loop") => p.s("object") != "None" && p.i("loop_at") > 0,
        ("roller-coaster", "object_at") => {
            p.s("object") != "None" && !(p.b("object_on_loop") && p.i("loop_at") > 0)
        }
        ("roller-coaster", "object_label" | "object_size" | "velocity") => p.s("object") != "None",
        ("roller-coaster", _) => coaster::relevant(key, p),
        ("spring", "masses") => p.s("shape") == "Slotted masses",
        ("spring", "extension_label") => p.b("show_extension"),
        ("spring", "amplitude" | "motion_arrows" | "show_equilibrium") => {
            p.s("picture") == "Three positions"
        }
        ("spring", "motion_direction" | "velocity_label") => {
            p.s("picture") == "Three positions"
                && matches!(p.s("motion_arrows"), "Velocity" | "Both")
        }
        ("spring", "acceleration_label") => {
            p.s("picture") == "Three positions"
                && matches!(p.s("motion_arrows"), "Acceleration" | "Both")
        }
        ("spring", "extension") => p.s("picture") != "Three positions",
        ("spring", "show_applied") => p.s("picture") != "Three positions",
        ("torque", "cable") => p.s("support") == "Wall",
        ("torque", "cable_at" | "cable_angle" | "cable_angle_label") => {
            p.s("support") == "Wall" && p.b("cable")
        }
        ("torque", "pivot" | "balance") => p.s("support") == "Pivot",
        ("torque", "support_a" | "support_b") => p.s("support") == "Two supports",
        ("torque", "gravity") => p.s("load_units") != "N",
        ("torque", _) => statics::relevant(key, p),
        _ => true,
    }
}

#[cfg(test)]
mod tests;
