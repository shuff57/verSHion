//! Paper chromatography with editable ink lanes and a worked Rf key.
use super::{INK, dashed, frame, tx};
use crate::figures::svg::{self, CategoryFills, Doc};
use crate::figures::{
    Example, FigureDef, ParamDef, Params, SHOW_ANSWERS, Subject, Value, answers_param,
};

const ID: &str = "chromatography";
const INKS: [(&str, &str, &str, &str); 6] = [
    ("ink1_name", "ink1_spots", "A", "blue 5.6, yellow 2.4"),
    ("ink2_name", "ink2_spots", "B", "red 4.4"),
    (
        "ink3_name",
        "ink3_spots",
        "C",
        "blue 5.6, red 4.4, yellow 2.4",
    ),
    ("ink4_name", "ink4_spots", "D", "green 4"),
    ("ink5_name", "ink5_spots", "E", "purple 4"),
    ("ink6_name", "ink6_spots", "F", "orange 4"),
];
// The category indices give each dye a distinct hatch in Print.
const DYES: [(&str, usize); 9] = [
    ("black", 9),
    ("blue", 2),
    ("red", 6),
    ("yellow", 1),
    ("green", 4),
    ("purple", 5),
    ("orange", 0),
    ("pink", 8),
    ("brown", 7),
];

pub(super) fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Chromatography",
        blurb: "Draw paper chromatograms from ink and solvent travel distances.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    let mut defs = vec![
        ParamDef::int("n", "Number of inks", 1, INKS.len() as i64, 3),
        ParamDef::float(
            "front",
            "Solvent front above the origin (cm)",
            4.0,
            15.0,
            0.1,
            8.0,
        ),
    ];
    for (name_key, spots_key, name, spots) in INKS {
        defs.push(ParamDef::text(name_key, "Ink name", name));
        defs.push(
            ParamDef::text(spots_key, "Dye and distance (cm)", spots)
                .hint("Up to five comma-separated pairs, such as blue 5.6, yellow 2.4."),
        );
    }
    defs.extend([
        ParamDef::bool("before", "Include paper before development", false),
        ParamDef::bool("beaker", "Show beaker and hanging rod", true),
        ParamDef::bool("measurements", "Show travel distances", false),
        ParamDef::bool("parts", "Label apparatus and guide lines", false),
        frame::title_style(true),
        ParamDef::text("title", "Title text", ""),
        ParamDef {
            default: Value::B(false),
            ..answers_param().hint("Add worked Rf values. Student + key exports both copies.")
        },
        svg::style_param(),
    ]);
    defs
}

pub(super) fn examples(id: &str) -> Option<Vec<Example>> {
    (id == ID).then(|| {
        vec![
            Example {
                name: "Calculate Rf from distances",
                params: vec![
                    ("measurements", Value::B(true)),
                    ("beaker", Value::B(false)),
                ],
            },
            Example {
                name: "Identify dyes in an unknown ink",
                params: vec![
                    ("n", Value::I(4)),
                    ("ink3_name", Value::S("Reference C".into())),
                    ("ink3_spots", Value::S("green 3.2".into())),
                    ("ink4_name", Value::S("Unknown".into())),
                    ("ink4_spots", Value::S("red 4.4, green 3.2".into())),
                ],
            },
            Example {
                name: "Label a before-and-after setup",
                params: vec![("before", Value::B(true)), ("parts", Value::B(true))],
            },
            Example {
                name: "Compare pure inks and mixtures",
                params: vec![("before", Value::B(true)), ("beaker", Value::B(false))],
            },
        ]
    })
}

#[derive(Clone, Copy)]
struct Spot {
    dye: &'static str,
    category: usize,
    cm: f64,
}

struct Ink {
    name: String,
    spots: Vec<Spot>,
}

fn inks(p: &Params, front: f64) -> Vec<Ink> {
    let count = p.i("n").clamp(1, INKS.len() as i64) as usize;
    INKS[..count]
        .iter()
        .map(|&(name_key, spots_key, default, _)| {
            let name = p.s(name_key).trim();
            let name = if name.is_empty() { default } else { name };
            let spots = p
                .s(spots_key)
                .split(',')
                .filter_map(|entry| {
                    let mut fields = entry.split_whitespace();
                    let color = fields.next()?;
                    let cm: f64 = fields.next()?.parse().ok()?;
                    if fields.next().is_some() || !cm.is_finite() {
                        return None;
                    }
                    let &(dye, category) = DYES
                        .iter()
                        .find(|(dye, _)| dye.eq_ignore_ascii_case(color))?;
                    Some(Spot {
                        dye,
                        category,
                        cm: cm.clamp(0.0, front),
                    })
                })
                .take(5)
                .collect();
            Ink {
                name: name.chars().take(18).collect(),
                spots,
            }
        })
        .collect()
}

const PANEL: f64 = 612.0;
const TOP: f64 = 57.0;
const FRONT: f64 = 91.0;
const ORIGIN: f64 = 314.0;
const SOLVENT: f64 = 353.0;
const BOTTOM: f64 = 379.0;
const PAPER_LEFT: f64 = 57.0;
const PAPER_RIGHT: f64 = 501.0;

fn render(p: &Params) -> String {
    let before = p.b("before");
    let width = if before { 2.0 * PANEL } else { PANEL };
    let mut d = Doc::new(width, 439.0);
    let front = p.f("front").max(4.0);
    let inks = inks(p, front);
    let fills = CategoryFills::new(p);
    d.defs(&fills.defs());
    let paper = Paper {
        inks: &inks,
        front,
        fills,
        p,
    };
    if before {
        paper.draw(&mut d, 0.0, false);
    }
    paper.draw(&mut d, width - PANEL, true);
    let key = if p.b(SHOW_ANSWERS) {
        inks.iter()
            .filter(|ink| !ink.spots.is_empty())
            .map(|ink| {
                let values = ink
                    .spots
                    .iter()
                    .map(|s| format!("{} Rf = {:.2}", s.dye, s.cm / front))
                    .collect::<Vec<_>>()
                    .join("   ·   ");
                format!("{}: {values}", ink.name)
            })
            .collect()
    } else {
        Vec::new()
    };
    frame::wrap(p, d.finish(), &key)
}

struct Paper<'a> {
    inks: &'a [Ink],
    front: f64,
    fills: CategoryFills,
    p: &'a Params,
}

impl Paper<'_> {
    fn draw(&self, d: &mut Doc, offset: f64, developed: bool) {
        let left = offset + PAPER_LEFT;
        let right = offset + PAPER_RIGHT;
        let middle = (left + right) / 2.0;
        let beaker = self.p.b("beaker");
        let parts = self.p.b("parts");
        let measures = developed && self.p.b("measurements");
        let glass_left = left - 15.0;
        let glass_right = right + 15.0;
        let glass_bottom = BOTTOM + 18.0;
        let level = if self.fills.is_print() {
            "#e4e4e4"
        } else {
            "#d8ebef"
        };

        d.rect(left, TOP, right - left, BOTTOM - TOP, "#fff", "#555", 1.5);
        if beaker {
            // The origin is above the solvent bath. The paper extends into it.
            d.rect(
                glass_left,
                SOLVENT,
                glass_right - glass_left,
                glass_bottom - SOLVENT,
                level,
                "none",
                0.0,
            );
            d.line(glass_left, SOLVENT, glass_right, SOLVENT, "#777", 1.1);
            d.polyline(
                &[
                    (glass_left - 7.0, TOP + 14.0),
                    (glass_left, TOP + 23.0),
                    (glass_left, glass_bottom),
                    (glass_right, glass_bottom),
                    (glass_right, TOP + 14.0),
                ],
                "#555",
                2.0,
            );
            d.polyline(
                &[
                    (left, SOLVENT),
                    (left, BOTTOM),
                    (right, BOTTOM),
                    (right, SOLVENT),
                ],
                "#777",
                1.2,
            );
            d.line(
                glass_left - 4.0,
                TOP + 2.0,
                glass_right + 4.0,
                TOP + 2.0,
                "#333",
                5.0,
            );
        }
        d.line(left, ORIGIN, right, ORIGIN, "#555", 1.25);
        if developed {
            dashed(d, left, FRONT, right, FRONT, "#555", 1.4);
        }

        let lane = (right - left) / self.inks.len() as f64;
        for (i, ink) in self.inks.iter().enumerate() {
            let center = left + (i as f64 + 0.5) * lane;
            let size = (lane * 1.55 / ink.name.chars().count() as f64).clamp(8.0, 14.0);
            tx(
                d,
                center,
                ORIGIN + 24.0,
                size,
                "middle",
                INK,
                true,
                &ink.name,
            );
            if developed {
                self.draw_spots(d, ink, center, measures);
            } else if !ink.spots.is_empty() {
                d.circle(center, ORIGIN, 6.0, INK, INK, 1.0);
            }
        }

        if parts {
            let label_x = glass_right + 13.0;
            if beaker {
                d.text(label_x, TOP + 6.0, 12.0, "start", "Rod");
                d.text(label_x, SOLVENT + 4.0, 12.0, "start", "Solvent");
            }
            if developed {
                d.text(label_x, FRONT + 4.0, 12.0, "start", "Solvent front");
            }
            d.text(label_x, ORIGIN + 4.0, 12.0, "start", "Origin line");
        }
        if measures {
            let x = left - 26.0;
            d.arrow_with(x, FRONT, x, ORIGIN, INK, 1.0, "", true);
            super::tr(
                d,
                x - 9.0,
                (FRONT + ORIGIN) / 2.0,
                13.0,
                -90.0,
                "middle",
                &format!("{:.1} cm", self.front),
            );
        }
        if self.p.b("before") {
            tx(
                d,
                middle,
                424.0,
                16.0,
                "middle",
                INK,
                true,
                if developed {
                    "After development"
                } else {
                    "Before development"
                },
            );
        }
    }

    fn draw_spots(&self, d: &mut Doc, ink: &Ink, center: f64, measures: bool) {
        let mut spots = ink.spots.clone();
        spots.sort_by(|a, b| b.cm.total_cmp(&a.cm));
        let y_at = |cm| ORIGIN - (ORIGIN - FRONT) * cm / self.front;
        let mut next_label_y = FRONT - 4.0;
        let mut previous_y = f64::NEG_INFINITY;
        let mut cluster = 0;
        for (i, spot) in spots.iter().enumerate() {
            let y = y_at(spot.cm);
            // Jitter crowded marks sideways without changing their measured vertical position.
            cluster = if y - previous_y < 13.0 {
                cluster + 1
            } else {
                0
            };
            previous_y = y;
            let lane_x = center - if measures { 16.0 } else { 0.0 };
            let offset = [0.0, -12.0, 12.0, -24.0, 24.0][cluster];
            let x = lane_x + if measures { offset * 0.64 } else { offset };
            let fill = if spot.dye == "black" && !self.fills.is_print() {
                INK.to_string()
            } else {
                self.fills.fill(spot.category)
            };
            if measures {
                // Reserve one row per remaining mark so even five co-migrating dyes fit.
                let last_y = ORIGIN + 2.0 - (spots.len() - 1 - i) as f64 * 13.0;
                let label_y = y.min(last_y).max(next_label_y);
                next_label_y = label_y + 13.0;
                d.line(x + 5.0, y, center + 4.0, label_y, "#777", 0.9);
                d.text(
                    center + 5.0,
                    label_y + 3.4,
                    9.0,
                    "start",
                    &format!("{:.1} cm", spot.cm),
                );
            }
            d.circle(x, y, 6.0, &fill, "#333", 1.2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{answer_pair, to_png};

    #[test]
    fn chromatography_worked_distances_and_print_pair() {
        let def = defs().remove(0);
        let mut p = Params::defaults(&params());
        p.set_b("before", true);
        p.set_b("measurements", true);
        p.set_b("parts", true);
        p.set_s("style", "Print");
        let (student, key) = answer_pair(&def, &p).unwrap();
        assert!(student.contains("Before development"));
        assert!(student.contains("Solvent front"));
        assert!(student.contains("5.6 cm"));
        assert!(student.contains("8.0 cm"));
        assert!(student.contains("url(#vs-cat2)"));
        assert!(!student.contains("Rf ="));
        assert!(key.contains("blue Rf = 0.70"));
        assert!(key.contains("red Rf = 0.55"));
        assert!(key.contains("yellow Rf = 0.30"));
        assert!(student.contains("cy=\"157.90\"")); // 314 − 223 × 5.6 / 8
        for svg in [&student, &key] {
            let png = to_png(svg, 1.0).unwrap();
            let img = image::load_from_memory(&png).unwrap().to_rgb8();
            assert!(img.pixels().all(|c| c[0] == c[1] && c[1] == c[2]));
        }
    }

    #[test]
    fn chromatography_handles_empty_malformed_and_overlong_lanes() {
        let mut p = Params::defaults(&params());
        p.set_i("n", 6);
        p.set_s("ink1_name", "   ");
        p.set_s(
            "ink1_spots",
            "blue NaN, green -1, red 20, gold 2, yellow 6, purple 4, black 0, brown 1",
        );
        p.set_s("ink2_spots", "");
        let parsed = inks(&p, 8.0);
        assert_eq!(parsed.len(), 6);
        assert_eq!(parsed[0].name, "A");
        assert_eq!(parsed[0].spots.len(), 5);
        assert_eq!(parsed[0].spots[0].cm, 0.0);
        assert_eq!(parsed[0].spots[1].cm, 8.0);
        assert!(parsed[1].spots.is_empty());
        assert!(roxmltree::Document::parse(&render(&p)).is_ok());
    }
}
