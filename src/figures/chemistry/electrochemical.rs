//! Galvanic metal/metal-ion cells under standard conditions.
use std::cmp::Ordering;

use super::{INK, frame, tx};
use crate::figures::svg::{self, CategoryFills, Doc};
use crate::figures::{
    Example, FigureDef, ParamDef, Params, SHOW_ANSWERS, Subject, Value, answers_param,
};

const ID: &str = "electrochemical-cells";
const SYMBOLS: [&str; 8] = ["Mg", "Al", "Zn", "Fe", "Ni", "Pb", "Cu", "Ag"];

#[derive(Clone, Copy)]
struct Metal {
    symbol: &'static str,
    charge: u8,
    potential_cv: i16, // standard reduction potential in centivolts
    category: usize,
}

const METALS: [Metal; 8] = [
    Metal {
        symbol: "Mg",
        charge: 2,
        potential_cv: -237,
        category: 3,
    },
    Metal {
        symbol: "Al",
        charge: 3,
        potential_cv: -166,
        category: 1,
    },
    Metal {
        symbol: "Zn",
        charge: 2,
        potential_cv: -76,
        category: 5,
    },
    Metal {
        symbol: "Fe",
        charge: 2,
        potential_cv: -44,
        category: 7,
    },
    Metal {
        symbol: "Ni",
        charge: 2,
        potential_cv: -25,
        category: 4,
    },
    Metal {
        symbol: "Pb",
        charge: 2,
        potential_cv: -13,
        category: 2,
    },
    Metal {
        symbol: "Cu",
        charge: 2,
        potential_cv: 34,
        category: 0,
    },
    Metal {
        symbol: "Ag",
        charge: 1,
        potential_cv: 80,
        category: 9,
    },
];

fn metal(symbol: &str, fallback: usize) -> Metal {
    METALS
        .iter()
        .copied()
        .find(|m| m.symbol == symbol)
        .unwrap_or(METALS[fallback])
}

impl Metal {
    fn solid(self) -> String {
        format!("{}(s)", self.symbol)
    }

    fn ion(self) -> String {
        let charge = match self.charge {
            1 => "⁺",
            2 => "²⁺",
            _ => "³⁺",
        };
        format!("{}{charge}(aq)", self.symbol)
    }
}

pub(super) fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Electrochemical Cells",
        blurb: "Build galvanic cells and explore standard cell voltage, electron flow and redox reactions.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("left", "Left electrode", &SYMBOLS, "Zn"),
        ParamDef::choice("right", "Right electrode", &SYMBOLS, "Cu"),
        ParamDef::choice(
            "voltage_source",
            "Meter voltage",
            &["From the table", "Custom"],
            "From the table",
        ),
        ParamDef::float("voltage", "Custom meter voltage (V)", 0.0, 9.99, 0.01, 1.10),
        ParamDef::choice(
            "meter",
            "External path",
            &["Digital voltmeter", "Dial voltmeter", "Coil of wire"],
            "Digital voltmeter",
        ),
        ParamDef::bool("give_reading", "Give meter reading", true)
            .hint("Turn off to leave the student meter blank while the key shows the reading."),
        ParamDef::bool("names", "Label materials and salt bridge", true),
        ParamDef::bool("electrodes", "Label anode and cathode", false),
        ParamDef::bool("flow", "Show electron direction", false),
        frame::title_style(true),
        ParamDef::text("title", "Title text", ""),
        ParamDef {
            default: Value::B(false),
            ..answers_param()
                .hint("Add cell voltage and balanced reactions. Student + key exports both copies.")
        },
        svg::style_param(),
    ]
}

pub(super) fn examples(id: &str) -> Option<Vec<Example>> {
    (id == ID).then(|| {
        vec![
            Example {
                name: "Zinc–copper galvanic cell",
                params: vec![],
            },
            Example {
                name: "Identify the electrodes in copper–silver",
                params: vec![
                    ("left", Value::S("Cu".into())),
                    ("right", Value::S("Ag".into())),
                ],
            },
            Example {
                name: "Calculate standard voltage for zinc–silver",
                params: vec![
                    ("right", Value::S("Ag".into())),
                    ("give_reading", Value::B(false)),
                ],
            },
            Example {
                name: "Aluminum–silver cell with a dial",
                params: vec![
                    ("left", Value::S("Al".into())),
                    ("right", Value::S("Ag".into())),
                    ("meter", Value::S("Dial voltmeter".into())),
                    ("flow", Value::B(true)),
                ],
            },
        ]
    })
}

struct Cell {
    left: Metal,
    right: Metal,
}

impl Cell {
    fn from_params(p: &Params) -> Self {
        Self {
            left: metal(p.s("left"), 2),
            right: metal(p.s("right"), 6),
        }
    }

    // The lower standard reduction potential is the oxidation side.
    fn anode_on_left(&self) -> Option<bool> {
        match self.left.potential_cv.cmp(&self.right.potential_cv) {
            Ordering::Less => Some(true),
            Ordering::Greater => Some(false),
            Ordering::Equal => None,
        }
    }

    fn voltage(&self) -> f64 {
        f64::from((self.right.potential_cv - self.left.potential_cv).abs()) / 100.0
    }

    fn answers(&self) -> Vec<String> {
        let Some(left) = self.anode_on_left() else {
            return vec![
                "E°cell = 0.00 V · Identical standard half-cells have no net reaction.".to_string(),
            ];
        };
        let (anode, cathode) = if left {
            (self.left, self.right)
        } else {
            (self.right, self.left)
        };
        let (a_solid, c_solid) = (anode.solid(), cathode.solid());
        let (a_ion, c_ion) = (anode.ion(), cathode.ion());
        let a_v = f64::from(anode.potential_cv) / 100.0;
        let c_v = f64::from(cathode.potential_cv) / 100.0;
        // In this table charges are 1, 2 or 3, so unequal charges are coprime.
        let (a_count, c_count) = if anode.charge == cathode.charge {
            (1, 1)
        } else {
            (cathode.charge, anode.charge)
        };
        vec![
            format!(
                "Anode: {} (−)   ·   Cathode: {} (+)",
                anode.symbol, cathode.symbol
            ),
            format!("E°cell = {c_v:.2} − ({a_v:.2}) = {:.2} V", self.voltage()),
            format!(
                "Oxidation: {a_solid} → {a_ion} + {}",
                term(anode.charge, "e⁻")
            ),
            format!(
                "Reduction: {c_ion} + {} → {c_solid}",
                term(cathode.charge, "e⁻")
            ),
            format!(
                "Overall: {} + {} → {} + {}",
                term(a_count, &a_solid),
                term(c_count, &c_ion),
                term(a_count, &a_ion),
                term(c_count, &c_solid)
            ),
        ]
    }
}

fn term(count: u8, species: &str) -> String {
    if count == 1 {
        species.to_string()
    } else {
        format!("{count}{species}")
    }
}

const WIDTH: f64 = 684.0;
const CENTER: f64 = WIDTH / 2.0;
const LEFT: f64 = 150.0;
const RIGHT: f64 = WIDTH - LEFT;
const WIRE_Y: f64 = 112.0;
const LEVEL: f64 = 276.0;

fn render(p: &Params) -> String {
    let c = Cell::from_params(p);
    let answers = p.b(SHOW_ANSWERS);
    let custom = p.s("voltage_source") == "Custom";
    let voltage = if custom {
        p.f("voltage").clamp(0.0, 9.99)
    } else {
        c.voltage()
    };
    let reading = (answers || p.b("give_reading")).then_some(voltage);
    let roles = c.anode_on_left();
    let fills = CategoryFills::new(p);
    let mut d = Doc::new(WIDTH, 437.0);
    d.defs(&fills.defs());
    let liquid = if fills.is_print() {
        "#ededed"
    } else {
        "#deedf1"
    };
    for (x, m) in [(LEFT, c.left), (RIGHT, c.right)] {
        beaker(&mut d, x, m, &fills, liquid);
    }

    // The U-shaped bridge joins the electrolytes below both liquid surfaces.
    let bridge = "M226 314 V234 Q226 214 246 214 H438 Q458 214 458 234 V314";
    d.path(bridge, "none", "#555", 17.0);
    d.path(bridge, "none", "#f1f1f1", 12.0);
    for x in [226.0, 458.0] {
        d.line(x - 8.5, 314.0, x + 8.5, 314.0, "#555", 1.2);
    }

    let dial_scale = if voltage > 4.0 { 10 } else { 4 };
    draw_meter(&mut d, p.s("meter"), reading, dial_scale);
    if p.b("flow")
        && let Some(left_to_right) = roles
    {
        for (start, end) in [(176.0, 250.0), (434.0, 508.0)] {
            let (from, to) = if left_to_right {
                (start, end)
            } else {
                (end, start)
            };
            d.arrow(from, 140.0, to, 140.0, INK, 2.0);
            d.text((start + end) / 2.0, 161.0, 15.0, "middle", "e⁻");
        }
    }
    if p.b("names") {
        for (x, m, label_x, anchor) in [
            (LEFT, c.left, LEFT - 13.0, "end"),
            (RIGHT, c.right, RIGHT + 13.0, "start"),
        ] {
            d.text(label_x, 194.0, 17.0, anchor, &m.solid());
            d.text(x, 367.0, 17.0, "middle", &m.ion());
        }
        d.text(CENTER, 199.0, 14.0, "middle", "Salt bridge");
    }
    if p.b("electrodes")
        && let Some(left_is_anode) = roles
    {
        for (x, anode) in [(LEFT, left_is_anode), (RIGHT, !left_is_anode)] {
            let role = if anode { "Anode (−)" } else { "Cathode (+)" };
            tx(&mut d, x, 419.0, 17.0, "middle", INK, true, role);
        }
    }

    let mut key = if answers { c.answers() } else { Vec::new() };
    if answers && custom && p.s("meter") != "Coil of wire" {
        key.insert(0, format!("Custom meter reading: {voltage:.2} V"));
    }
    frame::wrap(p, d.finish(), &key)
}

fn beaker(d: &mut Doc, x: f64, metal: Metal, fills: &CategoryFills, liquid: &str) {
    let left = x - 110.0;
    let right = x + 110.0;
    d.rect(left, LEVEL, right - left, 113.0, liquid, "none", 0.0);
    d.line(left, LEVEL, right, LEVEL, "#777", 1.5);
    d.polyline(
        &[
            (left - 8.0, 241.0),
            (left, 249.0),
            (left, 389.0),
            (right, 389.0),
            (right, 241.0),
        ],
        "#444",
        2.4,
    );
    d.line(x, WIRE_Y, x, 233.0, INK, 2.5);
    d.rect(
        x - 11.0,
        233.0,
        22.0,
        105.0,
        &fills.fill(metal.category),
        "#333",
        1.7,
    );
}

fn draw_meter(d: &mut Doc, meter: &str, reading: Option<f64>, scale: u8) {
    let body_left = CENTER - 71.0;
    let body_right = CENTER + 71.0;
    d.line(LEFT, WIRE_Y, body_left, WIRE_Y, INK, 2.5);
    d.line(body_right, WIRE_Y, RIGHT, WIRE_Y, INK, 2.5);
    match meter {
        "Coil of wire" => {
            d.path("M271 112 h11 q10 -24 20 0 q10 -24 20 0 q10 -24 20 0 q10 -24 20 0 q10 -24 20 0 q10 -24 20 0 h11", "none", INK, 2.5);
        }
        "Dial voltmeter" => {
            d.rect(
                body_left,
                50.0,
                body_right - body_left,
                119.0,
                "#f6f6f6",
                INK,
                2.0,
            );
            let pivot_y = 146.0;
            let point = |fraction: f64, radius: f64| {
                let angle = (150.0 - fraction * 120.0).to_radians();
                (
                    CENTER + radius * angle.cos(),
                    pivot_y - radius * angle.sin(),
                )
            };
            d.arc(CENTER, pivot_y, 52.0, 30.0, 150.0, "#444", 1.5);
            for tick in 0..=scale {
                let fraction = f64::from(tick) / f64::from(scale);
                let (x0, y0) = point(fraction, 46.0);
                let (x1, y1) = point(fraction, 52.0);
                d.line(x0, y0, x1, y1, "#444", 1.3);
                if scale == 4 || tick % 2 == 0 {
                    let (x, y) = point(fraction, 62.0);
                    d.text(x, y + 4.0, 11.0, "middle", &tick.to_string());
                }
            }
            if let Some(value) = reading {
                let (x, y) = point(value / f64::from(scale), 49.0);
                d.line(CENTER, pivot_y, x, y, INK, 2.1);
            }
            d.circle(CENTER, pivot_y, 3.0, INK, "none", 0.0);
            d.text(CENTER, 161.0, 12.0, "middle", "V");
        }
        _ => {
            d.rect(
                body_left,
                77.0,
                body_right - body_left,
                71.0,
                "#ededed",
                INK,
                2.0,
            );
            d.rect(body_left + 13.0, 89.0, 116.0, 45.0, "#fff", "#666", 1.2);
            let value = reading.map_or_else(|| "— V".to_string(), |value| format!("{value:.2} V"));
            tx(d, CENTER, 121.0, 24.0, "middle", INK, true, &value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{answer_pair, to_png};

    #[test]
    fn electrochemical_cells_reverse_roles_and_external_flow() {
        let mut p = Params::defaults(&params());
        p.set_b("electrodes", true);
        p.set_b("flow", true);
        let forward = render(&p);
        assert!(forward.contains("1.10 V"));
        assert!(forward.contains("x=\"150.00\" y=\"419.00\" font-size=\"17\""));
        assert!(forward.find("Anode (−)").unwrap() < forward.find("Cathode (+)").unwrap());
        assert!(forward.contains("250.00,140.00")); // arrowhead on the right
        p.set_s("left", "Cu");
        p.set_s("right", "Zn");
        let reversed = render(&p);
        assert!(reversed.contains("1.10 V"));
        assert!(reversed.find("Cathode (+)").unwrap() < reversed.find("Anode (−)").unwrap());
        assert!(reversed.contains("176.00,140.00")); // arrowhead on the left
        p.set_s("left", "Zn");
        let equal = render(&p);
        assert!(equal.contains("0.00 V"));
        assert!(!equal.contains("Anode (−)"));
        assert!(!equal.contains(">e⁻<"));
    }

    #[test]
    fn electrochemical_cells_balance_aluminum_silver_and_export_print_pair() {
        let def = defs().remove(0);
        let mut p = Params::defaults(&params());
        p.set_s("left", "Al");
        p.set_s("right", "Ag");
        p.set_s("style", "Print");
        p.set_b("give_reading", false);
        let (student, key) = answer_pair(&def, &p).unwrap();
        assert!(student.contains("— V"));
        assert!(!student.contains("2.46 V"));
        assert!(key.contains("0.80 − (-1.66) = 2.46 V"));
        assert!(key.contains("Al(s) → Al³⁺(aq) + 3e⁻"));
        assert!(key.contains("Al(s) + 3Ag⁺(aq) → Al³⁺(aq) + 3Ag(s)"));
        for svg in [&student, &key] {
            let png = to_png(svg, 1.0).unwrap();
            let img = image::load_from_memory(&png).unwrap().to_rgb8();
            assert!(img.pixels().all(|c| c[0] == c[1] && c[1] == c[2]));
        }
    }

    #[test]
    fn electrochemical_cells_meter_types_and_custom_voltage() {
        let mut p = Params::defaults(&params());
        p.set_s("voltage_source", "Custom");
        p.set_f("voltage", 9.99);
        assert!(render(&p).contains("9.99 V"));
        p.set_s("meter", "Dial voltmeter");
        let needle = "x1=\"342.00\" y1=\"146.00\"";
        let dial = render(&p);
        assert!(dial.contains(">10<") && dial.contains(needle));
        p.set_b("give_reading", false);
        let blank_dial = render(&p);
        assert!(blank_dial.contains(">10<") && !blank_dial.contains(needle));
        p.set_b(SHOW_ANSWERS, true);
        let dial_key = render(&p);
        assert!(dial_key.contains(needle));
        assert!(dial_key.contains("Custom meter reading: 9.99 V"));
        assert!(dial_key.contains("= 1.10 V"));
        p.set_b(SHOW_ANSWERS, false);
        p.set_s("meter", "Coil of wire");
        let coil = render(&p);
        assert!(coil.contains("q10 -24 20 0"));
        assert!(!coil.contains(">V<"));
    }
}
