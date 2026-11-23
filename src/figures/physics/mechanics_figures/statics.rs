use super::*;
use ParamDef as P;

const LEFT: f64 = 86.0;
const RIGHT: f64 = W - LEFT;

pub(super) fn lever() -> FigureDef {
    FigureDef {
        id: "lever",
        subject: Subject::Physics,
        title: "Lever",
        blurb: "Draw an ideal lever with an effort, a load, and a fulcrum. Print a solution for the effort and mechanical advantage.",
        params: lever_params,
        render: lever_render,
    }
}

fn lever_params() -> Vec<P> {
    presentation(
        vec![
            P::choice(
                "class",
                "Lever class",
                &["Class 1", "Class 2", "Class 3"],
                "Class 1",
            ),
            P::float("effort_arm", "Effort arm", 0.1, 999.0, 0.1, 1.5),
            P::float("load_arm", "Load arm", 0.1, 999.0, 0.1, 0.5),
            P::choice("length_units", "Arm units", &["m", "cm", "in", "ft"], "m"),
            P::float("load", "Load", 0.1, 9999.0, 0.1, 300.0),
            P::choice("load_units", "Load units", &["N", "lbf", "kg", "g"], "N"),
            P::choice(
                "load_shape",
                "Show load as",
                &["Block", "Force arrow"],
                "Block",
            ),
            P::bool("show_arms", "Show arm lengths", true),
            label_param("effort_arm_label", "Effort arm label"),
            label_param("load_arm_label", "Load arm label"),
            label_param("load_label", "Load value label"),
            label_param("effort_label", "Effort value label"),
            label_param("load_name", "Load name"),
            label_param("effort_name", "Effort name"),
            label_param("fulcrum_name", "Fulcrum name"),
            P::bool("mirror", "Mirror diagram", false),
        ],
        true,
    )
}

fn weight(value: f64, units: &str, gravity: f64) -> f64 {
    match units {
        "kg" => value * gravity,
        "g" => value * gravity / 1_000.0,
        "lbf" => value * 4.448_221_615_260_5,
        _ => value,
    }
}

fn moment(arm_m: f64, upward_n: f64) -> f64 {
    arm_m * upward_n
}

fn lever_render(p: &Params) -> String {
    let (mut g, fills) = canvas(
        p,
        355.0,
        "Lever",
        "An ideal lever on a fulcrum with two applied forces.",
    );
    let (ea, la) = (p.f("effort_arm"), p.f("load_arm"));
    let class = p.s("class");
    let same_side = class != "Class 1";
    let longest = ea.max(la);
    if (class == "Class 2" && ea <= la) || (class == "Class 3" && la <= ea) {
        let required = if class == "Class 2" {
            "The effort arm must be longer than the load arm."
        } else {
            "The load arm must be longer than the effort arm."
        };
        text(&mut g, (W / 2.0, 168.0), 18.0, "middle", required);
        return finish(g, p);
    }
    let width = RIGHT - LEFT;
    // Give short spans room for the pivot, load and labels. The calculation uses the actual lengths.
    let drawn_ratio = if same_side {
        ea.min(la) / longest
    } else {
        la / (la + ea)
    };
    let ratio = drawn_ratio.clamp(0.14, 0.86);
    let far = LEFT + width * ratio;
    let (pivot, load, effort) = match class {
        "Class 2" => (LEFT, far, RIGHT),
        "Class 3" => (LEFT, RIGHT, far),
        _ => (far, LEFT, RIGHT),
    };
    let units = p.s("load_units");
    let load_value = p.f("load");
    let w = weight(load_value, units, G_STANDARD);
    let ma = ea / la;
    let effort_n = moment(la, w) / ea;
    let shown = p.b(SHOW_ANSWERS);
    let y = 145.0;
    support(&mut g, pivot, y + 8.0, 21.0, &fills.series(2));
    g.rect(
        LEFT - 24.0,
        y,
        width + 48.0,
        8.0,
        &fills.series(0),
        INK,
        1.5,
    );
    g.d.circle(g.x(pivot), y + 4.0, 3.2, INK, "none", 0.0);
    let scale = 54.0 / w.max(effort_n);
    if p.s("load_shape") == "Block" {
        object(&mut g, (load, y - 19.0), 19.0, "Block", &fills.series(1));
    } else {
        let length = if shown { (w * scale).max(15.0) } else { 44.0 };
        g.arrow(load, y - length, load, y, fills.line(1), 2.5);
    }
    // Use uniform arrow lengths in the student copy so the drawing gives no force ratio.
    let length = if shown {
        (effort_n * scale).max(15.0)
    } else {
        44.0
    };
    if same_side {
        g.arrow(
            effort,
            y + 8.0 + length,
            effort,
            y + 8.0,
            fills.line(3),
            2.5,
        );
    } else {
        g.arrow(effort, y - length, effort, y, fills.line(3), 2.5);
    }
    let effort_auto = if shown {
        format!("{} N", num(effort_n))
    } else {
        "________".into()
    };
    text(
        &mut g,
        (load, 57.0),
        16.0,
        "middle",
        &label(p, "load_name", "Load"),
    );
    text(
        &mut g,
        (load, 77.0),
        15.0,
        "middle",
        &label(p, "load_label", &format!("{} {units}", num(load_value))),
    );
    text(
        &mut g,
        (effort, 57.0),
        16.0,
        "middle",
        &label(p, "effort_name", "Effort"),
    );
    text(
        &mut g,
        (effort, 77.0),
        15.0,
        "middle",
        &label(p, "effort_label", &effort_auto),
    );
    text(
        &mut g,
        (pivot, 205.0),
        15.0,
        "middle",
        &label(p, "fulcrum_name", "Fulcrum"),
    );

    if p.b("show_arms") {
        let arm_units = p.s("length_units");
        for (x, y, arm, key) in [
            (load, 236.0, la, "load_arm_label"),
            (effort, 270.0, ea, "effort_arm_label"),
        ] {
            bracket(&mut g, (pivot, y), (x, y));
            text(
                &mut g,
                ((pivot + x) / 2.0, y + 20.0),
                15.0,
                "middle",
                &label(p, key, &format!("{} {arm_units}", num(arm))),
            );
        }
    }
    if ratio != drawn_ratio {
        text(
            &mut g,
            (W / 2.0, 308.0),
            13.0,
            "middle",
            "Arm lengths are schematized for legibility.",
        );
    }
    if shown {
        let conversion = if units == "N" {
            String::new()
        } else {
            format!("Weight = {} N    •    ", num(w))
        };
        text(
            &mut g,
            (W / 2.0, 337.0),
            16.0,
            "middle",
            &format!(
                "{conversion}MA = {} ÷ {} ≈ {}    •    Effort = weight × load arm ÷ effort arm ≈ {} N",
                num(ea),
                num(la),
                num(ma),
                num(effort_n)
            ),
        );
    }
    finish(g, p)
}

pub(super) fn torque() -> FigureDef {
    FigureDef {
        id: "torque",
        subject: Subject::Physics,
        title: "Torque",
        blurb: "Draw a loaded beam and find its signed moments, support reactions, or cable tension.",
        params: torque_params,
        render: torque_render,
    }
}

const LOAD_KEYS: [[&str; 7]; 6] = [
    [
        "f1_pos", "f1_value", "f1_kind", "f1_shape", "f1_size", "f1_label", "f1_arm",
    ],
    [
        "f2_pos", "f2_value", "f2_kind", "f2_shape", "f2_size", "f2_label", "f2_arm",
    ],
    [
        "f3_pos", "f3_value", "f3_kind", "f3_shape", "f3_size", "f3_label", "f3_arm",
    ],
    [
        "f4_pos", "f4_value", "f4_kind", "f4_shape", "f4_size", "f4_label", "f4_arm",
    ],
    [
        "f5_pos", "f5_value", "f5_kind", "f5_shape", "f5_size", "f5_label", "f5_arm",
    ],
    [
        "f6_pos", "f6_value", "f6_kind", "f6_shape", "f6_size", "f6_label", "f6_arm",
    ],
];

fn torque_params() -> Vec<P> {
    let mut params = vec![
        P::choice(
            "support",
            "Support",
            &["Pivot", "Two supports", "Wall"],
            "Pivot",
        ),
        P::float("length", "Beam length", 0.1, 100.0, 0.01, 1.0),
        P::choice("length_units", "Distance units", &["m", "cm"], "m"),
        P::float("pivot", "Pivot from the left end", 0.0, 100.0, 0.01, 0.5),
        P::float(
            "support_a",
            "First support from the left end",
            0.0,
            100.0,
            0.01,
            0.1,
        ),
        P::float(
            "support_b",
            "Second support from the left end",
            0.0,
            100.0,
            0.01,
            0.9,
        ),
        P::bool("cable", "Wall support has a cable", true),
        P::float(
            "cable_at",
            "Cable attachment along the beam",
            0.0,
            100.0,
            0.01,
            1.0,
        ),
        P::float(
            "cable_angle",
            "Cable angle above the beam (°)",
            5.0,
            85.0,
            1.0,
            30.0,
        ),
        label_param("cable_angle_label", "Cable angle label"),
        P::float("beam_weight", "Beam weight or mass", 0.0, 50000.0, 0.1, 0.0),
        P::choice("load_units", "Load units", &["N", "kg", "g"], "N"),
        P::choice("gravity", "Gravity (N/kg)", &["9.8", "9.81", "10"], "9.8"),
        P::int("n", "Number of loads", 0, LOAD_KEYS.len() as i64, 2),
        P::int(
            "balance",
            "Solve for load (0 uses all given loads)",
            0,
            LOAD_KEYS.len() as i64,
            0,
        )
        .hint("For a pivot, solve for a selected load's magnitude using its chosen direction."),
    ];
    for (i, &[pos, value, kind, shape, size, name, arm]) in LOAD_KEYS.iter().enumerate() {
        params.extend([
            P::float(
                pos,
                "Position from the left end",
                0.0,
                100.0,
                0.01,
                if i == 0 { 0.3 } else { 0.9 },
            ),
            P::float(
                value,
                "Weight or mass",
                0.0,
                50000.0,
                0.1,
                if i == 0 { 6.0 } else { 3.0 },
            ),
            P::choice(
                kind,
                "Draw as",
                &["Hanging", "On beam", "Force down", "Force up"],
                "Hanging",
            ),
            P::choice(shape, "Object shape", &["Block", "Ball"], "Block"),
            P::float(size, "Object size", 0.5, 2.0, 0.1, 1.0),
            label_param(name, "Load label"),
            label_param(arm, "Distance label"),
        ]);
    }
    params.extend([
        P::bool("show_arms", "Show distances from support", true),
        P::bool("show_length", "Show whole beam length", false),
        P::bool("show_weights", "Draw weight arrows", false),
        P::bool("show_reactions", "Draw support forces on key", false),
        P::bool("mirror", "Mirror diagram", false),
    ]);
    presentation(params, true)
}

#[derive(Clone, Copy)]
struct Load {
    at: f64, // in the selected distance units
    up: f64, // newtons, positive upward
    direction: f64,
}

struct Beam {
    length: f64,
    metres: f64,
    beam_up: f64,
    loads: Vec<Load>,
}

impl Beam {
    fn moments_about(&self, at: f64) -> impl Iterator<Item = f64> + '_ {
        let beam = moment((self.length / 2.0 - at) * self.metres, self.beam_up);
        std::iter::once(beam).chain(
            self.loads
                .iter()
                .map(move |f| moment((f.at - at) * self.metres, f.up)),
        )
    }

    fn net_about(&self, at: f64) -> f64 {
        self.moments_about(at).sum()
    }

    fn down(&self) -> f64 {
        -self.beam_up - self.loads.iter().map(|f| f.up).sum::<f64>()
    }

    fn balance(&mut self, index: usize, pivot: f64) -> Result<f64, &'static str> {
        let Some(load) = self.loads.get(index).copied() else {
            return Err("Select an existing load to solve for.");
        };
        let arm = (load.at - pivot) * self.metres;
        if arm.abs() < 1e-10 {
            return Err("A force at the pivot cannot balance torque.");
        }
        let up = load.up - self.net_about(pivot) / arm;
        if load.direction * up < -1e-10 {
            return Err("The selected force must point in the opposite direction.");
        }
        self.loads[index].up = up;
        Ok(up.abs())
    }
}

enum Holds {
    Pivot(f64),
    Pair(f64, f64),
    Fixed,
    Cable { at: f64, angle: f64 },
}

impl Holds {
    fn from(p: &Params, length: f64) -> Self {
        let at = |k| p.f(k).min(length);
        match p.s("support") {
            "Two supports" => Holds::Pair(
                at("support_a").min(at("support_b")),
                at("support_a").max(at("support_b")),
            ),
            "Wall" if p.b("cable") => Holds::Cable {
                at: at("cable_at"),
                angle: p.f("cable_angle").to_radians(),
            },
            "Wall" => Holds::Fixed,
            _ => Holds::Pivot(at("pivot")),
        }
    }

    fn origin(&self) -> f64 {
        match *self {
            Holds::Pivot(a) | Holds::Pair(a, _) => a,
            _ => 0.0,
        }
    }
}

fn torque_render(p: &Params) -> String {
    let length = p.f("length");
    let units = p.s("load_units");
    let distance_units = p.s("length_units");
    let gravity = p.s("gravity").parse().unwrap_or(G_STANDARD);
    let holds = Holds::from(p, length);
    let origin = holds.origin();
    let mut beam = Beam {
        length,
        metres: if distance_units == "cm" { 0.01 } else { 1.0 },
        beam_up: -weight(p.f("beam_weight"), units, gravity),
        loads: LOAD_KEYS
            .iter()
            .take(p.i("n") as usize)
            .map(|&[pos, value, kind, ..]| {
                let direction = if p.s(kind) == "Force up" { 1.0 } else { -1.0 };
                Load {
                    at: p.f(pos).min(length),
                    up: direction * weight(p.f(value), units, gravity),
                    direction,
                }
            })
            .collect(),
    };
    let shown = p.b(SHOW_ANSWERS);
    let target = (p.i("balance") as usize)
        .checked_sub(1)
        .filter(|_| matches!(holds, Holds::Pivot(_)));
    let solved = target.map(|i| beam.balance(i, origin));
    let y = 155.0;
    let span = match holds {
        Holds::Cable { at, angle } if at > 0.0 => {
            (RIGHT - LEFT).min(91.0 * length / (at * angle.tan()))
        }
        _ => RIGHT - LEFT,
    };
    let left = (W - span) / 2.0;
    let x = |pos| left + pos / length * span;
    let (mut g, fills) = canvas(
        p,
        530.0,
        "Torque",
        "A horizontal beam with supports and applied loads.",
    );
    g.rect(left, y, span, 8.0, &fills.series(0), INK, 1.5);
    let lower = y + 8.0;
    match holds {
        Holds::Pivot(a) => support(&mut g, x(a), lower, 19.0, &fills.series(2)),
        Holds::Pair(a, b) => {
            for (at, name) in [(a, "A"), (b, "B")] {
                support(&mut g, x(at), lower, 17.0, &fills.series(2));
                text(&mut g, (x(at) - 23.0, lower + 30.0), 14.0, "end", name);
            }
        }
        Holds::Fixed | Holds::Cable { .. } => {
            g.line(left, 52.0, left, 220.0, INK, 3.0);
            for h in (59..220).step_by(10) {
                let h = f64::from(h);
                g.line(left - 11.0, h + 5.0, left, h, GUIDE, 1.2);
            }
            if let Holds::Cable { at, angle } = holds {
                let attach = x(at);
                if at > 0.0 {
                    let wall_y = y - (attach - left) * angle.tan();
                    g.line(left, wall_y, attach, y, fills.line(2), 2.5);
                    g.d.circle(g.x(attach), y, 3.5, INK, "none", 0.0);
                    if p.b("show_arms") {
                        bracket(&mut g, (left, 279.0), (attach, 279.0));
                        text(
                            &mut g,
                            ((left + attach) / 2.0, 275.0),
                            13.0,
                            "middle",
                            &format!("Cable: {} {distance_units}", num(at)),
                        );
                    }
                    text(
                        &mut g,
                        ((left + attach) / 2.0, (wall_y + y) / 2.0 - 12.0),
                        14.0,
                        "middle",
                        &label(
                            p,
                            "cable_angle_label",
                            &format!("{}°", num(angle.to_degrees())),
                        ),
                    );
                }
            }
        }
    }
    for (i, load) in beam.loads.iter().enumerate() {
        let [_, _, kind, shape, size, name, arm] = LOAD_KEYS[i];
        let px = x(load.at);
        let radius = 10.0 * p.f(size);
        let label_auto = if target == Some(i) {
            match (shown, &solved) {
                (true, Some(Ok(force))) => format!("{} N", num(*force)),
                _ => "________".into(),
            }
        } else {
            format!("{} {}", num(p.f(LOAD_KEYS[i][1])), units)
        };
        let caption = label(p, name, &label_auto);
        let color = fills.line(i + 1);
        match p.s(kind) {
            "Force down" | "Force up" => {
                let start = y - 42.0 - (i % 3) as f64 * 20.0;
                if load.direction > 0.0 {
                    g.arrow(px, y, px, start, color, 2.2);
                } else {
                    g.arrow(px, start, px, y, color, 2.2);
                }
                text(&mut g, (px, start - 6.0), 14.0, "middle", &caption);
            }
            kind => {
                let cy = if kind == "On beam" {
                    y - radius
                } else {
                    let end = lower + 50.0 + (i % 2) as f64 * 17.0;
                    g.line(px, lower, px, end - radius, INK, 1.4);
                    end
                };
                object(&mut g, (px, cy), radius, p.s(shape), &fills.series(i + 1));
                let anchor = if px > W - 155.0 { "end" } else { "start" };
                let dx = if anchor == "end" {
                    -(radius + 6.0)
                } else {
                    radius + 6.0
                };
                text(&mut g, (px + dx, cy + 4.0), 14.0, anchor, &caption);
                if p.b("show_weights") {
                    g.arrow(px, cy + radius, px, cy + radius + 25.0, color, 1.7);
                }
            }
        }
        if p.b("show_arms") {
            let row = 299.0 + i as f64 * 22.0;
            bracket(&mut g, (x(origin), row), (px, row));
            let auto = format!("{} {distance_units}", num((load.at - origin).abs()));
            text(
                &mut g,
                ((x(origin) + px) / 2.0, row - 4.0),
                13.0,
                "middle",
                &label(p, arm, &auto),
            );
        }
    }
    if beam.beam_up != 0.0 {
        let center = x(length / 2.0);
        g.arrow(center, y - 37.0, center, y, INK, 1.7);
        text(
            &mut g,
            (center, y - 44.0),
            13.0,
            "middle",
            &format!("Beam: {} {units}", num(p.f("beam_weight"))),
        );
    }
    if p.b("show_length") {
        bracket(&mut g, (left, 430.0), (left + span, 430.0));
        text(
            &mut g,
            (left + span / 2.0, 449.0),
            14.0,
            "middle",
            &format!("{} {distance_units}", num(length)),
        );
    }
    if shown {
        let total = beam.net_about(origin);
        let down = beam.down();
        let arrow = |g: &mut G, at, up: f64| {
            if p.b("show_reactions") && up.abs() > 1e-10 {
                let px = x(at);
                g.arrow(px, y, px, y - up.signum() * 56.0, fills.line(4), 2.6);
            }
        };
        let named_moment = |ccw: f64| {
            if ccw.abs() < 1e-9 {
                "0 N·m".into()
            } else {
                let direction = if (ccw > 0.0) != p.b("mirror") {
                    "counterclockwise"
                } else {
                    "clockwise"
                };
                format!("{} N·m {direction}", num(ccw.abs()))
            }
        };
        let line1 = format!(
            "Net applied moment about {} = {}",
            match holds {
                Holds::Pivot(_) => "pivot",
                Holds::Pair(_, _) => "A",
                _ => "wall",
            },
            named_moment(total)
        );
        let line2 = match holds {
            Holds::Pivot(at) => {
                if total.abs() < 1e-9 && !solved.as_ref().is_some_and(Result::is_err) {
                    arrow(&mut g, at, down);
                }
                if let Some(result) = solved {
                    match result {
                        Ok(force) => format!(
                            "Load {} = {} N   •   Pivot force = {} N (up positive)",
                            target.unwrap_or(0) + 1,
                            num(force),
                            num(down)
                        ),
                        Err(message) => message.into(),
                    }
                } else if total.abs() < 1e-9 {
                    format!("Balanced   •   Pivot force = {} N (up positive)", num(down))
                } else {
                    "The beam is rotationally unbalanced.".into()
                }
            }
            Holds::Pair(a, b) if b - a > 1e-10 => {
                let rb = -total / ((b - a) * beam.metres);
                let ra = down - rb;
                arrow(&mut g, a, ra);
                arrow(&mut g, b, rb);
                let tip = if ra < -1e-9 || rb < -1e-9 {
                    "   •   A contact support lifts"
                } else {
                    ""
                };
                format!(
                    "Upward reactions: A = {} N, B = {} N{tip}",
                    num(ra),
                    num(rb)
                )
            }
            Holds::Pair(_, _) => "Separate the supports to determine the two reactions.".into(),
            Holds::Fixed => {
                arrow(&mut g, 0.0, down);
                if p.b("show_reactions") && total.abs() > 1e-9 {
                    let (start, end) = if total < 0.0 {
                        (30.0, 240.0)
                    } else {
                        (240.0, 30.0)
                    };
                    let arc = super::super::common::arc_pts(left, y + 4.0, 28.0, start, end);
                    g.polyline(&arc, fills.line(4), 2.1);
                    let n = arc.len();
                    g.head(
                        arc[n - 5].0,
                        arc[n - 5].1,
                        arc[n - 1].0,
                        arc[n - 1].1,
                        fills.line(4),
                        2.1,
                    );
                }
                format!(
                    "Wall vertical reaction = {} N   •   Wall moment = {}",
                    num(down),
                    named_moment(-total)
                )
            }
            Holds::Cable { at, angle } if at > 0.0 => {
                let vertical = -total / (at * beam.metres);
                let t = vertical / angle.sin();
                if t < -1e-9 {
                    "A cable cannot push. This setup has no equilibrium.".into()
                } else {
                    let wall_up = down - vertical;
                    arrow(&mut g, 0.0, wall_up);
                    if p.b("show_reactions") && t > 1e-9 {
                        let span = x(at) - left;
                        let arrow_len = (span / angle.cos() * 0.75).min(44.0);
                        let h = y + 4.0;
                        g.arrow(left, h, left + 54.0, h, fills.line(4), 2.2);
                        g.arrow(
                            x(at),
                            y,
                            x(at) - arrow_len * angle.cos(),
                            y - arrow_len * angle.sin(),
                            fills.line(4),
                            2.2,
                        );
                    }
                    format!(
                        "Tension = {} N   •   Wall reaction: {} N along beam, {} N up",
                        num(t),
                        num(t * angle.cos()),
                        num(wall_up)
                    )
                }
            }
            Holds::Cable { .. } => {
                "Place the cable away from the wall to give it a moment arm.".into()
            }
        };
        let mirror_sign = if p.b("mirror") { -1.0 } else { 1.0 };
        let (cw, ccw) = beam.moments_about(origin).fold((0.0, 0.0), |(cw, ccw), m| {
            let signed = mirror_sign * m;
            (cw + (-signed).max(0.0), ccw + signed.max(0.0))
        });
        let totals = format!(
            "Clockwise {} N·m   •   Counterclockwise {} N·m",
            num(cw),
            num(ccw)
        );
        text(&mut g, (W / 2.0, 472.0), 15.0, "middle", &totals);
        text(&mut g, (W / 2.0, 492.0), 15.0, "middle", &line1);
        text(&mut g, (W / 2.0, 512.0), 15.0, "middle", &line2);
    }
    finish(g, p)
}

pub(super) fn relevant(key: &str, p: &Params) -> bool {
    let Some((i, keys)) = LOAD_KEYS
        .iter()
        .enumerate()
        .find(|(_, keys)| keys.contains(&key))
    else {
        return true;
    };
    match key {
        key if key == keys[1] => p.s("support") != "Pivot" || p.i("balance") != i as i64 + 1,
        key if key == keys[3] || key == keys[4] => matches!(p.s(keys[2]), "Hanging" | "On beam"),
        key if key == keys[6] => p.b("show_arms"),
        _ => true,
    }
}
