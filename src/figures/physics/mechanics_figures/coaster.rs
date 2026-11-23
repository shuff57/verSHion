use super::*;
use ParamDef as P;

pub(super) fn def() -> FigureDef {
    FigureDef {
        id: "roller-coaster",
        subject: Subject::Physics,
        title: "Roller Coaster",
        blurb: "Draw an energy problem with high and low points, an optional vertical loop, and rough stretches.",
        params,
        render,
    }
}

const POINT_KEYS: [[&str; 5]; 6] = [
    [
        "p1_height",
        "p1_label",
        "p1_height_label",
        "p1_rough",
        "p1_rough_label",
    ],
    [
        "p2_height",
        "p2_label",
        "p2_height_label",
        "p2_rough",
        "p2_rough_label",
    ],
    [
        "p3_height",
        "p3_label",
        "p3_height_label",
        "p3_rough",
        "p3_rough_label",
    ],
    [
        "p4_height",
        "p4_label",
        "p4_height_label",
        "p4_rough",
        "p4_rough_label",
    ],
    [
        "p5_height",
        "p5_label",
        "p5_height_label",
        "p5_rough",
        "p5_rough_label",
    ],
    [
        "p6_height",
        "p6_label",
        "p6_height_label",
        "p6_rough",
        "p6_rough_label",
    ],
];
const NAMES: [&str; 6] = ["A", "B", "C", "D", "E", "F"];
const GROUND: f64 = 293.0;

fn params() -> Vec<P> {
    let mut defs = vec![
        P::int("n", "Track points", 2, POINT_KEYS.len() as i64, 4),
        P::choice("units", "Height and radius units", &["m", "cm", "ft"], "m"),
    ];
    for (i, &[height, name, mark, rough, length]) in POINT_KEYS.iter().enumerate() {
        defs.extend([
            P::float(
                height,
                "Height",
                0.0,
                100.0,
                0.5,
                [40.0, 10.0, 25.0, 5.0, 15.0, 0.0][i],
            ),
            label_param(name, "Point label"),
            label_param(mark, "Height label"),
            P::bool(rough, "Rough track to the next point", false),
            label_param(length, "Horizontal distance label for rough track"),
        ]);
    }
    defs.extend([
        P::int(
            "loop_at",
            "Loop at point (0 for no loop)",
            0,
            POINT_KEYS.len() as i64,
            0,
        ),
        P::float("loop_radius", "Loop radius", 0.1, 50.0, 0.1, 8.0),
        label_param("radius_label", "Loop radius label"),
        label_param("loop_top_label", "Label above the loop"),
        P::bool("object_on_loop", "Object upside down at top of loop", false),
        P::choice(
            "object",
            "Object",
            &["Cart", "Ball", "Block", "None"],
            "Cart",
        ),
        P::int(
            "object_at",
            "Object at point",
            1,
            POINT_KEYS.len() as i64,
            1,
        ),
        label_param("object_label", "Object label"),
        P::float("object_size", "Object size (%)", 30.0, 120.0, 5.0, 60.0),
        P::choice(
            "velocity",
            "Velocity direction",
            &["None", "Forward", "Back"],
            "None",
        ),
        P::bool("show_ground", "Ground line", true),
        P::bool("mirror", "Mirror diagram", false),
    ]);
    presentation(defs, false)
}

struct Track {
    points: Vec<Pt>,
    loop_over: Option<(usize, f64)>,
}

impl Track {
    fn from(p: &Params) -> Self {
        let n = p.i("n") as usize;
        let heights: Vec<f64> = POINT_KEYS.iter().take(n).map(|keys| p.f(keys[0])).collect();
        let loop_index = (p.i("loop_at") as usize).checked_sub(1).filter(|&i| i < n);
        let radius = p.f("loop_radius");
        let loop_top = loop_index.map_or(0.0, |i| heights[i] + 2.0 * radius);
        let top = heights.iter().copied().fold(loop_top, f64::max);
        let dx = (W - 190.0) / (n - 1) as f64;
        let mut scale = 199.0 / top.max(1.0);
        if loop_index.is_some() {
            scale = scale.min((dx * 0.41).min(67.0) / radius);
        }
        let points = heights
            .iter()
            .enumerate()
            .map(|(i, height)| (95.0 + dx * i as f64, GROUND - height * scale))
            .collect();
        Self {
            points,
            loop_over: loop_index.map(|i| (i, radius * scale)),
        }
    }
}

/// A cubic with horizontal tangents at both ends has no overshoot, including on a level stretch.
fn stretch(a: Pt, b: Pt, t: f64) -> Pt {
    let y = a.1 + (b.1 - a.1) * t * t * (3.0 - 2.0 * t);
    (a.0 + (b.0 - a.0) * t, y)
}

fn render(p: &Params) -> String {
    let track = Track::from(p);
    let points = &track.points;
    let units = p.s("units");
    let (mut g, fills) = canvas(
        p,
        416.0,
        "Roller coaster",
        "A side view of the track above a horizontal reference level.",
    );
    if p.b("show_ground") {
        g.line(64.0, GROUND, W - 64.0, GROUND, GUIDE, 1.2);
    }

    for (i, segment) in points.windows(2).enumerate() {
        let (a, b) = (segment[0], segment[1]);
        let x1 = g.x(a.0);
        let x2 = g.x(b.0);
        let (c1, c2) = ((2.0 * x1 + x2) / 3.0, (x1 + 2.0 * x2) / 3.0);
        g.d.path(
            &format!(
                "M{x1:.2},{:.2} C{c1:.2},{:.2} {c2:.2},{:.2} {x2:.2},{:.2}",
                a.1, a.1, b.1, b.1
            ),
            "none",
            fills.line(0),
            3.0,
        );
        if p.b(POINT_KEYS[i][3]) {
            let ticks = ((b.0 - a.0) / 11.0).round() as usize;
            for j in 1..ticks {
                let t = j as f64 / ticks as f64;
                let (x, y) = stretch(a, b, t);
                let dy = (b.1 - a.1) * 6.0 * t * (1.0 - t);
                let dx = b.0 - a.0;
                let norm = dx.hypot(dy);
                g.line(
                    x,
                    y,
                    x + (dx - dy) * 6.0 / norm,
                    y + (dx + dy) * 6.0 / norm,
                    fills.line(1),
                    1.5,
                );
            }
            let caption = label(p, POINT_KEYS[i][4], "");
            if !caption.is_empty() {
                bracket(&mut g, (a.0, 356.0), (b.0, 356.0));
                text(&mut g, ((a.0 + b.0) / 2.0, 379.0), 14.0, "middle", &caption);
            }
        }
    }
    if let Some((i, r)) = track.loop_over {
        let (x, base) = points[i];
        let cy = base - r;
        g.d.circle(g.x(x), cy, r, "none", fills.line(0), 3.0);
        text(
            &mut g,
            (x, cy - r - 11.0),
            14.0,
            "middle",
            &label(p, "loop_top_label", ""),
        );
        let auto = format!("r = {} {units}", num(p.f("loop_radius")));
        let caption = label(p, "radius_label", &auto);
        if !caption.is_empty() {
            g.d.circle(g.x(x), cy, 2.0, INK, "none", 0.0);
            g.line(x, cy, x + r, cy, GUIDE, 1.1);
            text(&mut g, (x, cy - 7.0), 13.0, "middle", &caption);
        }
    } else if p.i("loop_at") > p.i("n") {
        text(
            &mut g,
            (W / 2.0, 400.0),
            14.0,
            "middle",
            "Choose an existing track point for the loop.",
        );
    }
    for (i, &(x, y)) in points.iter().enumerate() {
        let [height, name, mark, ..] = POINT_KEYS[i];
        let caption = label(p, mark, &format!("{} {units}", num(p.f(height))));
        let position = x - 12.0;
        if y < GROUND - 3.0 && !caption.is_empty() {
            g.dash(x, y + 3.0, x, GROUND, GUIDE, 1.0);
            text(
                &mut g,
                (position, (y + GROUND) / 2.0 + 4.0),
                14.0,
                "end",
                &caption,
            );
        } else {
            text(&mut g, (position, GROUND - 14.0), 14.0, "end", &caption);
        }
        g.d.circle(g.x(x), y, 3.6, INK, "white", 1.0);
        text(
            &mut g,
            (x, 320.0),
            17.0,
            "middle",
            &label(p, name, NAMES[i]),
        );
    }
    let on_top = p.b("object_on_loop") && track.loop_over.is_some();
    let i = (p.i("object_at") as usize).saturating_sub(1);
    let object_point = if on_top {
        track
            .loop_over
            .map(|(i, radius)| (points[i].0, points[i].1 - 2.0 * radius))
    } else {
        points.get(i).copied()
    };
    if p.s("object") != "None" {
        if let Some((x, y)) = object_point {
            let full_size = 13.0 * p.f("object_size") / 60.0;
            let r = if let Some((_, radius)) = track.loop_over.filter(|_| on_top) {
                full_size.min(radius * 0.38)
            } else {
                full_size
            };
            let side = if on_top { 1.0 } else { -1.0 };
            if p.s("object") == "Cart" {
                g.poly(
                    &[
                        (x - r, y + side * r * 0.45),
                        (x - r * 1.25, y + side * r * 1.3),
                        (x + r * 1.25, y + side * r * 1.3),
                        (x + r, y + side * r * 0.45),
                    ],
                    &fills.series(1),
                    INK,
                    2.0,
                );
                for dx in [-0.68, 0.68] {
                    g.d.circle(
                        g.x(x + dx * r),
                        y + side * r * 0.22,
                        r * 0.22,
                        "white",
                        INK,
                        1.4,
                    );
                }
            } else {
                object(
                    &mut g,
                    (x, y + side * r),
                    r,
                    p.s("object"),
                    &fills.series(1),
                );
            }
            text(
                &mut g,
                (x, if on_top { y - 27.0 } else { y - 2.0 * r - 8.0 }),
                15.0,
                "middle",
                &label(p, "object_label", "m"),
            );
            if p.s("velocity") != "None" {
                // A cart entering the loop in the forward direction travels the other way at its crest.
                let dir = if (p.s("velocity") == "Back") != on_top {
                    -1.0
                } else {
                    1.0
                };
                let y = y + side * r;
                g.arrow(
                    x + dir * (r + 7.0),
                    y,
                    x + dir * (r + 46.0),
                    y,
                    fills.line(3),
                    2.2,
                );
                text(&mut g, (x + dir * (r + 27.0), y - 7.0), 14.0, "middle", "v");
            }
        } else {
            text(
                &mut g,
                (W / 2.0, 400.0),
                14.0,
                "middle",
                "Choose an existing track point for the object.",
            );
        }
    }
    finish(g, p)
}

pub(super) fn relevant(key: &str, p: &Params) -> bool {
    let Some((i, keys)) = POINT_KEYS
        .iter()
        .enumerate()
        .find(|(_, keys)| keys.contains(&key))
    else {
        return true;
    };
    match key {
        key if key == keys[3] => i + 1 < p.i("n") as usize,
        key if key == keys[4] => i + 1 < p.i("n") as usize && p.b(keys[3]),
        _ => true,
    }
}
