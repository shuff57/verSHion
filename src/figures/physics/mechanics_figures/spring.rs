use super::*;
use ParamDef as P;

pub(super) fn def() -> FigureDef {
    FigureDef {
        id: "spring",
        subject: Subject::Physics,
        title: "Spring",
        blurb: "Compare natural length, extension, compression, and oscillation for a spring or an arrangement of springs.",
        params,
        render,
    }
}

fn params() -> Vec<P> {
    presentation(
        vec![
            P::choice(
                "orientation",
                "Orientation",
                &["Hanging", "Horizontal"],
                "Hanging",
            ),
            P::choice(
                "picture",
                "Positions",
                &["One spring", "Before and after", "Three positions"],
                "Before and after",
            ),
            P::choice(
                "arrangement",
                "Springs",
                &[
                    "1",
                    "2 in series",
                    "2 in parallel",
                    "3 in series",
                    "3 in parallel",
                ],
                "1",
            ),
            P::float("extension", "Change in length (%)", -50.0, 100.0, 5.0, 40.0)
                .hint("Negative compresses a horizontal spring."),
            P::float(
                "amplitude",
                "Amplitude (% of natural length)",
                0.0,
                80.0,
                5.0,
                40.0,
            ),
            P::choice("coil", "Coil", &["Zigzag", "Loops"], "Zigzag"),
            P::choice(
                "shape",
                "Object",
                &["Block", "Slotted masses", "Ball"],
                "Block",
            ),
            P::int("masses", "Number of slotted masses", 1, 5, 3),
            label_param("object_label", "Object label"),
            label_param("spring_label", "Spring label"),
            P::bool("show_extension", "Mark extension or compression", true),
            label_param("extension_label", "Extension label"),
            P::bool("show_natural", "Mark natural length", false),
            P::bool("show_equilibrium", "Draw equilibrium guide", true),
            P::bool("show_applied", "Draw applied force (static)", false),
            P::choice(
                "motion_arrows",
                "Motion arrows",
                &["None", "Velocity", "Acceleration", "Both"],
                "Both",
            ),
            P::choice(
                "motion_direction",
                "Motion at equilibrium",
                &["Away from support", "Toward support"],
                "Away from support",
            ),
            label_param("velocity_label", "Velocity label"),
            label_param("acceleration_label", "Acceleration label"),
        ],
        false,
    )
}

/// A local axis runs from the support across each spring. For hanging springs the axis points down.
#[derive(Clone, Copy)]
struct Axis {
    anchor: Pt,
    vertical: bool,
}

impl Axis {
    fn point(self, axial: f64, lateral: f64) -> Pt {
        let (x, y) = self.anchor;
        if self.vertical {
            (x + lateral, y + axial)
        } else {
            (x + axial, y + lateral)
        }
    }

    fn line(self, g: &mut G, a: Pt, b: Pt, stroke: &str, width: f64) {
        let (a, b) = (self.point(a.0, a.1), self.point(b.0, b.1));
        g.line(a.0, a.1, b.0, b.1, stroke, width);
    }
}

fn lengths(vertical: bool, oscillation: bool, extension: f64) -> (f64, [f64; 3]) {
    if oscillation {
        let amplitude = extension.abs();
        let eq = if vertical {
            1.0 + amplitude.max(0.35)
        } else {
            1.0
        };
        (eq, [eq - amplitude, eq, eq + amplitude])
    } else {
        let extension = if vertical {
            extension.max(0.0)
        } else {
            extension
        };
        (1.0, [1.0, 1.0 + extension, 1.0 + extension])
    }
}

fn coil(g: &mut G, axis: Axis, a: f64, b: f64, lateral: f64, loops: bool, stroke: &str) {
    let len = b - a;
    let lead = (len * 0.16).min(6.0);
    let width = (b - a - 2.0 * lead).max(0.0);
    let mut pts = vec![axis.point(a, lateral), axis.point(a + lead, lateral)];
    if loops {
        for i in 0..=64 {
            let t = f64::from(i) / 64.0;
            let angle = t * std::f64::consts::TAU * 4.0;
            let u = a + lead + width * t + (width * 0.07).min(5.0) * angle.sin();
            pts.push(axis.point(u, lateral + 6.0 * angle.sin() + 2.0 * (1.0 - angle.cos())));
        }
    } else {
        for i in 0..10 {
            let axial = a + lead + width * (f64::from(i) + 0.5) / 10.0;
            let offset = if i % 2 == 0 { -6.0 } else { 6.0 };
            pts.push(axis.point(axial, lateral + offset));
        }
    }
    pts.extend([axis.point(b - lead, lateral), axis.point(b, lateral)]);
    g.polyline(&pts, stroke, 2.1);
}

fn render(p: &Params) -> String {
    let vertical = p.s("orientation") == "Hanging";
    let picture = p.s("picture");
    let oscillation = picture == "Three positions";
    let extension = p.f(if oscillation {
        "amplitude"
    } else {
        "extension"
    }) / 100.0;
    let (equilibrium, extents) = lengths(vertical, oscillation, extension);
    let indices: &[usize] = match picture {
        "One spring" => &[1],
        "Three positions" => &[0, 1, 2],
        _ => &[0, 1],
    };
    let n = indices.len() as f64;
    let slots: Vec<Axis> = (0..indices.len())
        .map(|i| {
            let i = i as f64;
            let anchor = if vertical {
                (W / 2.0 + (i - (n - 1.0) / 2.0) * 190.0, 76.0)
            } else {
                (120.0, 92.0 + i * 108.0)
            };
            Axis { anchor, vertical }
        })
        .collect();
    let (mut g, fills) = canvas(
        p,
        if vertical { 430.0 } else { 106.0 + n * 108.0 },
        "Spring",
        "Spring coils shown at one or more lengths.",
    );
    let natural = if vertical { 85.0 } else { 151.0 };
    let assembly = p.s("arrangement");
    let count = if assembly.starts_with('3') {
        3
    } else if assembly.starts_with('2') {
        2
    } else {
        1
    };
    let (series, parallel) = if assembly.ends_with("parallel") {
        (1, count)
    } else {
        (count, 1)
    };
    let extent = (f64::from(parallel) - 1.0) * 9.0;
    let guide = |g: &mut G, u: f64| {
        let a = slots[0].point(u, -58.0);
        let b = slots[slots.len() - 1].point(u, 58.0);
        g.dash(a.0, a.1, b.0, b.1, GUIDE, 1.2);
    };
    if (oscillation && p.b("show_equilibrium"))
        || (!oscillation && n > 1.0 && p.b("show_extension"))
    {
        guide(&mut g, equilibrium * natural);
    }
    if p.b("show_natural") {
        let axis = slots[0];
        bracket(&mut g, axis.point(0.0, -52.0), axis.point(natural, -52.0));
        text(
            &mut g,
            axis.point(natural / 2.0, -60.0),
            14.0,
            "middle",
            "L₀",
        );
    }
    for (&i, &axis) in indices.iter().zip(&slots) {
        let end = natural * extents[i];
        let object_visible = oscillation || picture == "One spring" || i != 0;
        let mass = 16.0_f64.max(extent + 3.0);
        let axis_color = fills.line(0);
        if !vertical {
            axis.line(
                &mut g,
                (0.0, mass),
                (end.max(natural) + 111.0, mass),
                GUIDE,
                1.2,
            );
        }
        axis.line(&mut g, (0.0, -25.0), (0.0, mass * 2.0), INK, 2.5);
        for j in -3..=3 {
            let lateral = f64::from(j) * 7.0;
            axis.line(&mut g, (-7.0, lateral - 4.0), (0.0, lateral), GUIDE, 1.15);
        }

        for j in 0..parallel {
            let lateral = f64::from(j) * 18.0 - extent;
            for k in 0..series {
                let a = end * f64::from(k) / f64::from(series);
                let b = end * f64::from(k + 1) / f64::from(series);
                coil(
                    &mut g,
                    axis,
                    a,
                    b,
                    lateral,
                    p.s("coil") == "Loops",
                    axis_color,
                );
                if j + 1 == parallel {
                    let side = if vertical { 15.0 + extent } else { -13.0 };
                    text(
                        &mut g,
                        axis.point((a + b) / 2.0, side),
                        14.0,
                        "middle",
                        &label(p, "spring_label", "k"),
                    );
                }
                if k + 1 < series {
                    let (x, y) = axis.point(b, lateral);
                    g.d.circle(g.x(x), y, 2.6, INK, "none", 0.0);
                }
            }
        }
        if parallel > 1 {
            axis.line(&mut g, (end, -extent - 3.0), (end, extent + 3.0), INK, 2.0);
        }
        if object_visible {
            let center = axis.point(end + mass, 0.0);
            if p.s("shape") == "Slotted masses" {
                let (cx, cy) = center;
                let count = p.i("masses") as f64;
                let step = 2.0 * mass / count;
                for row in 0..p.i("masses") {
                    g.rect(
                        cx - mass,
                        cy - mass + row as f64 * step,
                        2.0 * mass,
                        step,
                        &fills.series(1),
                        INK,
                        1.15,
                    );
                }
            } else {
                object(&mut g, center, mass, p.s("shape"), &fills.series(1));
            }
            let label_pos = if vertical {
                axis.point(end + mass + 4.0, -mass - 8.0)
            } else {
                axis.point(end + mass, -mass - 6.0)
            };
            text(
                &mut g,
                label_pos,
                15.0,
                if vertical { "end" } else { "middle" },
                &label(p, "object_label", "m"),
            );

            let arrow = if oscillation && extension > 0.0 {
                let arrows = p.s("motion_arrows");
                match i {
                    1 if matches!(arrows, "Both" | "Velocity") => {
                        let sign = if p.s("motion_direction") == "Toward support" {
                            -1.0
                        } else {
                            1.0
                        };
                        Some((sign, label(p, "velocity_label", "v")))
                    }
                    0 | 2 if matches!(arrows, "Both" | "Acceleration") => {
                        let sign = if i == 0 { 1.0 } else { -1.0 };
                        Some((sign, label(p, "acceleration_label", "a")))
                    }
                    _ => None,
                }
            } else if !oscillation
                && p.b("show_applied")
                && extension.abs() > 0.0
                && (!vertical || extension > 0.0)
            {
                Some((extension.signum(), "F".into()))
            } else {
                None
            };
            if let Some((direction, caption)) = arrow {
                let lateral = if vertical { mass + 14.0 } else { mass + 13.0 };
                let origin = end + mass;
                let a = axis.point(origin - direction * 18.0, lateral);
                let b = axis.point(origin + direction * 18.0, lateral);
                g.arrow(a.0, a.1, b.0, b.1, fills.line(3), 2.1);
                text(
                    &mut g,
                    axis.point(origin, lateral + if vertical { 13.0 } else { 18.0 }),
                    15.0,
                    "middle",
                    &caption,
                );
            }
        }
        if p.b("show_extension") && (end - equilibrium * natural).abs() > 0.1 {
            let lateral = if vertical { -44.0 } else { -36.0 };
            bracket(
                &mut g,
                axis.point(equilibrium * natural, lateral),
                axis.point(end, lateral),
            );
            let caption = if oscillation {
                if i == 0 { "−A" } else { "+A" }
            } else {
                "x"
            };
            let position = axis.point((equilibrium * natural + end) / 2.0, lateral - 8.0);
            text(
                &mut g,
                position,
                14.0,
                if vertical { "end" } else { "middle" },
                &label(p, "extension_label", caption),
            );
        }
    }
    finish(g, p)
}
