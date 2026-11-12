//! Free-body diagram, batch 11: the Forces card's "Add a force" buttons (gravity, normal, friction, tension, applied,
//! spring, air resistance, and a plain force) add a force pointing the usual way for the figure as it is and
//! labelled with its usual symbol. The figure draws a body with no surface or string of its own, so "the usual way"
//! is the textbook one for a body on level ground pulled up by a string: gravity straight down, the normal force
//! straight up off the ground, tension up along the string, an applied push or pull and a spring's force along the
//! ground; friction runs along the ground against the motion and air resistance straight against it, read from the
//! velocity arrow when one is drawn (else against a body moving right, the figure's usual motion).
use crate::figures::{ParamDef, Params, Value};

/// The direction (degrees, counterclockwise from right) a force added by the button `chip` points, for `p`.
pub fn usual_direction(chip: &str, p: &Params) -> Option<f64> {
    let m = p.s("motion");
    let moving = (m == "Velocity" || m == "Both").then(|| p.f("mdir").rem_euclid(360.0));
    Some(match chip {
        "Gravity" => 270.0,
        "Normal" | "Tension" => 90.0,
        "Applied" | "Spring" => 0.0,
        "Friction" => match moving {
            // along the ground, against the horizontal part of the motion (straight up or down: no sliding to oppose)
            Some(v) if v.to_radians().cos() < -1e-9 => 0.0,
            _ => 180.0,
        },
        "Air resistance" => match moving {
            Some(v) => (v + 180.0).rem_euclid(360.0),
            None => 180.0,
        },
        "Force" => 45.0,
        _ => return None,
    })
}

/// After a button added force `n` (1-based): it points the usual way, and its other settings (angle mark, components)
/// start fresh instead of keeping a removed force's.
pub fn added(chip: &str, n: usize, p: &mut Params, defs: &[ParamDef]) {
    if !(1..=8).contains(&n) {
        return;
    }
    let pre = format!("f{n}_");
    for d in defs.iter().filter(|d| d.key.starts_with(&pre)) {
        let rest = &d.key[pre.len()..];
        if matches!(rest, "mark" | "from" | "alab_mode" | "alab" | "comps") {
            p.0.insert(d.key.to_string(), d.default.clone());
        }
    }
    if let Some(a) = usual_direction(chip, p) {
        p.0.insert(format!("f{n}_ang"), Value::F(a));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{chips_for, registry, start_params, FigureDef};

    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "free-body-diagram").unwrap() }

    /// What the panel's button does (figuresui::apply_chip, then `added`), as a function of the params.
    fn press(p: &mut Params, label: &str) {
        let d = def();
        let defs = (d.params)();
        let card = chips_for("free-body-diagram").unwrap();
        let chip = card.chips.iter().find(|c| c.label == label).unwrap();
        let n = p.i("n") as usize + 1;
        for (k, v) in &chip.set { p.0.insert(k.replace("{n}", &n.to_string()), v.clone()); }
        p.set_i("n", n as i64);
        added(label, n, p, &defs);
    }

    /// The seven named buttons and the plain one, each labelled with its usual symbol.
    #[test]
    fn eight_add_buttons_with_their_usual_labels() {
        let card = chips_for("free-body-diagram").unwrap();
        let got: Vec<(&str, String)> = card.chips.iter().map(|c| (c.label, c.set.iter().find_map(|(k, v)| match v { Value::S(t) if *k == "f{n}_lab" => Some(t.clone()), _ => None }).unwrap())).collect();
        let want = [("Gravity", "F_g"), ("Normal", "F_N"), ("Friction", "F_f"), ("Tension", "F_T"), ("Applied", "F_app"), ("Spring", "F_s"), ("Air resistance", "F_air"), ("Force", "F")];
        assert_eq!(got.iter().map(|(a, b)| (*a, b.as_str())).collect::<Vec<_>>(), want);
    }

    /// Each force points the usual way: gravity down, normal and tension up, applied and spring along the ground,
    /// friction along the ground against the motion, air resistance straight against it.
    #[test]
    fn each_force_points_the_usual_way() {
        let base = start_params(&def());
        let at = |m: &str, v: f64, chip: &str| {
            let mut p = base.clone();
            p.set_s("motion", m);
            p.set_f("mdir", v);
            usual_direction(chip, &p).unwrap()
        };
        for (m, v) in [("None", 0.0), ("Velocity", 0.0), ("Both", 200.0), ("Acceleration", 180.0)] {
            assert_eq!(at(m, v, "Gravity"), 270.0);
            assert_eq!(at(m, v, "Normal"), 90.0);
            assert_eq!(at(m, v, "Tension"), 90.0);
            assert_eq!(at(m, v, "Applied"), 0.0);
            assert_eq!(at(m, v, "Spring"), 0.0);
        }
        // no velocity drawn (or only an acceleration): against a body moving right
        assert_eq!(at("None", 0.0, "Friction"), 180.0);
        assert_eq!(at("Acceleration", 180.0, "Friction"), 180.0);
        assert_eq!(at("None", 0.0, "Air resistance"), 180.0);
        // moving right, left, up a slope, falling
        assert_eq!(at("Velocity", 0.0, "Friction"), 180.0);
        assert_eq!(at("Velocity", 180.0, "Friction"), 0.0);
        assert_eq!(at("Both", 150.0, "Friction"), 0.0);
        assert_eq!(at("Velocity", 30.0, "Air resistance"), 210.0);
        assert_eq!(at("Velocity", 270.0, "Air resistance"), 90.0, "a falling body: air resistance up");
        assert_eq!(at("Both", 180.0, "Air resistance"), 0.0);
    }

    /// From the opening state, every button adds one force, drawn, pointing its way, labelled, with a fresh slot
    /// (a removed force's angle mark and components do not come back).
    #[test]
    fn a_button_adds_a_clean_force_that_changes_the_drawing() {
        let d = def();
        let open = start_params(&d);
        for label in ["Gravity", "Normal", "Friction", "Tension", "Applied", "Spring", "Air resistance", "Force"] {
            let mut p = open.clone();
            let n0 = p.i("n") as usize;
            // a slot left over from a removed force
            p.set_b(&format!("f{}_mark", n0 + 1), true);
            p.set_b(&format!("f{}_comps", n0 + 1), true);
            press(&mut p, label);
            let k = n0 + 1;
            assert_eq!(p.i("n") as usize, k);
            assert!(!p.b(&format!("f{k}_mark")) && !p.b(&format!("f{k}_comps")), "{label}: a fresh slot");
            assert_eq!(p.f(&format!("f{k}_ang")), usual_direction(label, &p).unwrap());
            assert_eq!(p.s(&format!("f{k}_lab_mode")), "Text");
            assert_ne!(d.svg(&p), d.svg(&open), "{label}: drawn");
        }
        // up to eight, then the buttons stop
        let mut p = open.clone();
        while (p.i("n") as usize) < 8 { press(&mut p, "Friction"); }
        assert_eq!(p.i("n"), 8);
    }
}
