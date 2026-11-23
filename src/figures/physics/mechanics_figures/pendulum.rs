use super::*;
use ParamDef as P;

pub(super) fn def() -> FigureDef {
    FigureDef {
        id: "pendulum",
        subject: Subject::Physics,
        title: "Pendulum",
        blurb: "Show a bob at release or at three points in its swing, with height, angle, velocity, or release forces.",
        params,
        render,
    }
}

fn params() -> Vec<P> {
    presentation(
        vec![
            P::choice("picture", "Positions", &["Swing", "One bob"], "Swing"),
            P::float(
                "angle",
                "Release angle from vertical (°)",
                0.0,
                75.0,
                1.0,
                30.0,
            )
            .hint("The three-position view uses at least 15° to keep the bobs apart."),
            P::choice("side", "Release side", &["Left", "Right"], "Right"),
            P::bool("show_path", "Dashed swing path", true),
            P::bool("show_angle", "Mark angle", true),
            label_param("angle_label", "Angle label"),
            label_param("length_label", "String length label"),
            P::bool("show_height", "Mark height above lowest point", true),
            label_param("height_label", "Height label"),
            P::bool("show_letters", "Name positions A, B, C", false),
            P::bool("show_velocity", "Velocity at the lowest point", false),
            P::choice(
                "forces",
                "Release forces (one bob)",
                &["None", "Weight and tension", "Components"],
                "None",
            ),
            label_param("velocity_label", "Velocity label"),
            label_param("weight_label", "Weight label"),
            label_param("tension_label", "Tension label"),
            label_param("radial_label", "Component along the string"),
            label_param("tangent_label", "Component along the path"),
            label_param("bob_label", "Bob label"),
        ],
        false,
    )
}

const PIVOT: Pt = (W / 2.0, 74.0);
const R: f64 = 162.0;
const BOB: f64 = 14.0;

fn bob_at(angle: f64) -> Pt {
    (PIVOT.0 + R * angle.sin(), PIVOT.1 + R * angle.cos())
}

fn render(p: &Params) -> String {
    let swing = p.s("picture") == "Swing";
    let theta = if swing {
        p.f("angle").max(15.0)
    } else {
        p.f("angle")
    }
    .to_radians();
    let (_, low) = bob_at(0.0);
    let (bx, by) = bob_at(theta);
    let (mut g, fills) = canvas(
        p,
        354.0,
        "Pendulum",
        "A string pendulum released from rest.",
    );
    g.mir = p.s("side") == "Left";
    g.line(PIVOT.0 - 35.0, PIVOT.1, PIVOT.0 + 35.0, PIVOT.1, INK, 3.0);
    for i in -3..=3 {
        let x = PIVOT.0 + 9.0 * f64::from(i);
        g.line(x, PIVOT.1, x + 7.0, PIVOT.1 - 8.0, GUIDE, 1.3);
    }
    g.d.circle(g.x(PIVOT.0), PIVOT.1, 3.0, INK, "none", 0.0);
    g.dash(PIVOT.0, PIVOT.1, PIVOT.0, low, GUIDE, 1.2);
    if p.b("show_path") && theta > 0.0 {
        let sign = if g.mir { -1.0 } else { 1.0 };
        let x1 = g.x(bob_at(-theta).0);
        let x2 = g.x(bx);
        g.d.raw(&format!(
            r#"<path d="M{x1:.2},{by:.2} A{R},{R} 0 0,{} {x2:.2},{by:.2}" fill="none" stroke="{GUIDE}" stroke-width="1.4" stroke-dasharray="5 4"/>"#,
            u8::from(sign < 0.0)
        ));
    }

    let marked_height = p.b("show_height") && low - by > 0.01;
    if marked_height {
        let dim = bx + 64.0;
        g.dash(bx, by, dim, by, GUIDE, 1.15);
        g.dash(
            PIVOT.0 + if swing { BOB } else { 0.0 },
            low,
            dim,
            low,
            GUIDE,
            1.15,
        );
        bracket(&mut g, (dim, by), (dim, low));
        text(
            &mut g,
            (dim + 10.0, (by + low) / 2.0 + 4.0),
            16.0,
            "start",
            &label(p, "height_label", "h"),
        );
    }

    if swing {
        for (angle, name) in [(-theta, "C"), (0.0, "B")] {
            let (x, y) = bob_at(angle);
            g.line(PIVOT.0, PIVOT.1, x, y, GUIDE, 1.5);
            object(&mut g, (x, y), BOB, "Ball", &fills.series(0));
            if p.b("show_letters") {
                text(&mut g, (x, y + 34.0), 16.0, "middle", name);
            }
        }
        if p.b("show_velocity") {
            let x = PIVOT.0;
            g.arrow(x, low + 49.0, x - 49.0, low + 49.0, fills.line(1), 2.2);
            text(
                &mut g,
                (x - 25.0, low + 70.0),
                15.0,
                "middle",
                &label(p, "velocity_label", "v"),
            );
        }
    }

    g.line(PIVOT.0, PIVOT.1, bx, by, INK, 2.0);
    if p.b("show_angle") {
        if theta > 0.0 {
            let dirs = (
                (0.0, 1.0),
                (theta.sin() * if g.mir { -1.0 } else { 1.0 }, theta.cos()),
            );
            g.d.arc_dirs((g.x(PIVOT.0), PIVOT.1), dirs.0, dirs.1, 48.0, GUIDE, 1.25);
        }
        let x = PIVOT.0 + 66.0 * (theta / 2.0).sin();
        let y = PIVOT.1 + 66.0 * (theta / 2.0).cos();
        text(
            &mut g,
            (x + if theta < 0.35 { 11.0 } else { 0.0 }, y),
            16.0,
            "middle",
            &label(p, "angle_label", "θ"),
        );
    }
    text(
        &mut g,
        ((bx + PIVOT.0) / 2.0 + 12.0, (by + PIVOT.1) / 2.0 + 4.0),
        16.0,
        "start",
        &label(p, "length_label", "L"),
    );
    object(&mut g, (bx, by), BOB, "Ball", &fills.series(1));
    let (bob_text, anchor) = if !swing && p.s("forces") != "None" {
        ((bx + BOB + 8.0, by - 9.0), "start")
    } else {
        ((bx, by + 5.0), "middle")
    };
    text(&mut g, bob_text, 14.0, anchor, &label(p, "bob_label", "m"));
    if swing && p.b("show_letters") {
        text(&mut g, (bx, by + 34.0), 16.0, "middle", "A");
    }

    if !swing && p.s("forces") != "None" {
        let (sin, cos) = theta.sin_cos();
        // The vector scale grows at wide angles to leave room for the short tension arrow.
        let weight = 58.0 + 25.0 * sin;
        let radial = (sin * weight * cos, cos * weight * cos);
        let tangent = (-cos * weight * sin, sin * weight * sin);
        g.arrow(bx, by, bx, by + weight, fills.line(1), 2.3);
        g.arrow(bx, by, bx - radial.0, by - radial.1, fills.line(0), 2.3);
        text(
            &mut g,
            (bx + 7.0, by + weight + 15.0),
            15.0,
            "start",
            &label(p, "weight_label", "mg"),
        );
        text(
            &mut g,
            (bx - radial.0 - 10.0, by - radial.1 - 5.0),
            15.0,
            "end",
            &label(p, "tension_label", "T"),
        );
        if p.s("forces") == "Components" && theta > 0.0 {
            for ((dx, dy), key, caption, anchor) in [
                (radial, "radial_label", "mg cos θ", "start"),
                (tangent, "tangent_label", "mg sin θ", "end"),
            ] {
                g.dash(bx + dx, by + dy, bx, by + weight, GUIDE, 1.2);
                g.arrow(bx, by, bx + dx, by + dy, fills.line(3), 1.5);
                let pad = if anchor == "start" { 5.0 } else { -5.0 };
                text(
                    &mut g,
                    (bx + dx + pad, by + dy + 6.0),
                    13.0,
                    anchor,
                    &label(p, key, caption),
                );
            }
        }
    }
    finish(g, p)
}
