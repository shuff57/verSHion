//! Physics figure generators (original re-creations of the common physics teaching figures).
use super::*;

mod handles;
mod circuit_solve;
mod common;
mod graphs;
mod instr;
mod mech;
mod pulley;
pub(crate) use pulley::{pulley_relevant, pulley_setup_changed};
mod examples;

/// Editor notes for the instrument figures: what the circuit could not work out, and why.
pub fn instr_warnings(p: &Params) -> Vec<String> { instr::circuit_warnings(p) }

/// The collapsed-row summary of a section on the physics page skin, where the page has one.
pub fn skin_summary(id: &str, title: &str, p: &Params) -> Option<String> {
    match id {
        "waves" => graphs::wave_summary(title, p),
        _ => None,
    }
}

/// "Start from an example" entries for this subject's figures.
pub fn examples(id: &str) -> Option<Vec<super::Example>> {
    examples::examples(id)
}

/// "Add a force / vector / segment / part / object" chips for the figures that keep a numbered list of items.
pub fn chips(id: &str) -> Option<super::ChipsCard> {
    use super::{Chip, ChipsCard, Value};
    let force = |label, ang: f64, lab: &'static str| Chip {
        label,
        set: vec![("f{n}_ang", Value::F(ang)), ("f{n}_len", Value::F(1.0)), ("f{n}_lab", Value::S(lab.into())), ("f{n}_lab_mode", Value::S("Text".into()))],
    };
    match id {
        "free-body-diagram" => Some(ChipsCard {
            title: "Add a force",
            count_key: "n",
            max: 8,
            chips: vec![
                force("Gravity", 270.0, "F_g"),
                force("Normal", 90.0, "F_N"),
                force("Friction", 180.0, "F_f"),
                force("Tension", 90.0, "F_T"),
                force("Applied", 0.0, "F_app"),
                force("Spring", 0.0, "F_s"),
                force("Air resistance", 180.0, "F_air"),
                force("Force", 45.0, "F"),
            ],
        }),
        "vector-diagram" => Some(ChipsCard {
            title: "Add a vector",
            count_key: "n",
            max: 4,
            chips: vec![Chip {
                label: "Vector",
                set: vec![("v{n}_mag", Value::F(3.0)), ("v{n}_ang", Value::F(90.0)), ("v{n}_lab", Value::S("{L}".into())), ("v{n}_lab_mode", Value::S("Text".into()))],
            }],
        }),
        "motion-graphs" => {
            // the reference's "Add a segment" buttons: kind, speed, 3 s, forward
            let seg = |label: &'static str, v: f64| Chip {
                label,
                set: vec![("s{n}_kind", Value::S(label.into())), ("s{n}_v", Value::F(v)), ("s{n}_t", Value::F(3.0)), ("s{n}_dir", Value::S("Forward".into()))],
            };
            Some(ChipsCard {
                title: "Add a segment",
                count_key: "n",
                max: 6,
                chips: vec![
                    seg("At rest", 0.0),
                    seg("Constant velocity forward", 4.0),
                    seg("Constant velocity back", 4.0),
                    seg("Speeding up", 4.0),
                    seg("Slowing down", 0.0),
                ],
            })
        }
        "circuit-diagram" => {
            // a new part keeps the slot's own value (2, 4, 6, 8 ohms) and an automatic name
            let part = |label: &'static str| Chip {
                label,
                set: vec![("r{n}_kind", Value::S(label.into())), ("r{n}_name_mode", Value::S("Text".into())), ("r{n}_name", Value::S(String::new())), ("r{n}_val_mode", Value::S("Text".into()))],
            };
            Some(ChipsCard { title: "Add a resistor or bulb", count_key: "n", max: 4, chips: vec![part("Resistor"), part("Bulb")] })
        }
        "inclined-plane" => {
            // objects 2 and 3 sit above object 1 (the legacy single-object keys)
            let obj = |label: &'static str| Chip {
                label,
                set: vec![("obj{n}_kind", Value::S(label.into())), ("obj{n}_lab_mode", Value::S("Text".into())), ("obj{n}_size", Value::I(100))],
            };
            Some(ChipsCard { title: "Add an object above", count_key: "n", max: 3, chips: vec![obj("Block"), obj("Ball"), obj("Cart")] })
        }
        _ => None,
    }
}

/// Direct-manipulation handles for this subject's figures (None = not one of ours).
pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<interact::Handle>> {
    handles::handles(id, p, size)
}

/// The spring scale's "Random" button: a force between 5% and 95% of the capacity, to 2 decimal
/// places (the reference's readings, e.g. 3.48, 8.27 N on the 10 N scale).
pub fn random_reading(id: &str, p: &mut Params, seed: u64) -> bool {
    if id != "spring-scale" {
        return false;
    }
    let cap = p.s("cap").trim_end_matches(" N").parse::<f64>().unwrap_or(10.0);
    let v = cap * (0.05 + 0.9 * super::unit_random(seed));
    p.set_f("reading", (v * 100.0).round() / 100.0);
    true
}

pub fn defs() -> Vec<FigureDef> {
    let mut v = mech::defs();
    v.extend(graphs::defs());
    v.extend(instr::defs());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUT: &str = "/tmp/claude-1000/-home-shuff57/4fa1e9b4-a716-475e-b753-2c60208b3c78/scratchpad/physics";

    fn check(d: &FigureDef, p: &Params, tag: &str) {
        let svg = (d.render)(p);
        let png = to_png(&svg, 1.0).unwrap_or_else(|e| panic!("{} [{tag}] failed: {e}", d.id));
        assert!(png.len() > 500, "{} [{tag}] empty", d.id);
        if std::fs::create_dir_all(OUT).is_ok() {
            let _ = std::fs::write(format!("{OUT}/{}_{tag}.png", d.id), png);
        }
    }

    /// Round-5 cosmetics: bold arrowheads, tenths on the spring scale's dial, and the vector
    /// diagram opening with a dashed resultant (its plain default, used by saved figures and
    /// examples, stays drawn).
    #[test]
    fn cosmetics_match_the_reference() {
        assert!(common::head_len(3.5) >= 19.0 && common::head_len(1.0) >= 12.0);
        let get = |id: &str| defs().into_iter().find(|d| d.id == id).unwrap();
        let sp = get("spring-scale");
        let svg = (sp.render)(&Params::defaults(&(sp.params)()));
        // 0..10 N in tenths (101 marks) where fifths gave 51
        assert!(svg.matches("<line").count() >= 101, "tenths drawn: {}", svg.matches("<line").count());
        let vd = get("vector-diagram");
        assert_eq!(Params::defaults(&(vd.params)()).s("res_style"), "Drawn");
        assert_eq!(start_params(&vd).s("res_style"), "Dashed");
        let dashed = (vd.render)(&start_params(&vd));
        assert!(dashed.contains("stroke-dasharray"), "the resultant opens dashed");
    }

    #[test]
    fn the_spring_scale_caption_can_be_blanked_for_a_worksheet() {
        // the answer key prints the reading; a typed caption replaces it, so a worksheet gets a blank instead
        let sp = defs().into_iter().find(|d| d.id == "spring-scale").unwrap();
        let mut p = Params::defaults(&(sp.params)());
        p.0.insert("answer".into(), Value::B(true));
        let key = (sp.render)(&p);
        assert!(key.contains("Force: 3.47 N"), "the key prints the reading");
        p.0.insert("answer_text".into(), Value::S("Force: ________ N".into()));
        let sheet = (sp.render)(&p);
        assert!(sheet.contains("Force: ________ N"), "the typed caption wins");
        assert!(!sheet.contains("3.47"), "and the reading is gone from the figure");
        if let Ok(dir) = std::env::var("VERSHION_PNG_DIR") {
            // the worksheet copy, to be looked at: a caption, a pointer, and no answer anywhere
            let _ = std::fs::write(format!("{dir}/spring-scale-worksheet.png"), to_png(&sheet, 1.0).unwrap());
        }
    }

    #[test]
    fn a_capacitor_circuit_reads_in_farads_and_has_no_meters() {
        let cd = defs().into_iter().find(|d| d.id == "circuit-diagram").unwrap();
        let cap = |list: &[(&'static str, Value)]| {
            let mut p = Params::defaults(&(cd.params)());
            for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
            p
        };
        let series = cap(&[
            ("components", Value::S("Capacitors".into())), ("n", Value::I(2)),
            ("r1_val", Value::S("10 µF".into())), ("r2_val", Value::S("10 µF".into())),
            ("cell_val", Value::S("12 V".into())), ("solve", Value::S("Readings and results".into())),
        ]);
        let svg = (cd.render)(&series);
        // C_eq = 5 µF, Q = 60 µC, 6 V across each
        assert!(svg.contains("5 µF"), "5 µF in series:\n{svg}");
        assert!(svg.contains("60 µC"), "the charge");
        assert!(!svg.contains("Ω"), "no resistance in a capacitor circuit");
        // no meters, whatever the meter switches say
        let mut with_meters = series.clone();
        with_meters.0.insert("ammeter".into(), Value::B(true));
        with_meters.0.insert("voltmeter".into(), Value::S("Across component 1".into()));
        let lay = instr::circuit_layout(&with_meters);
        assert!(lay.amm.is_none(), "an ammeter was placed on a capacitor circuit");
        assert!(lay.vm_part.is_none(), "a voltmeter was placed on a capacitor circuit");
        // neither meter control changes the figure at all
        let mut plain = with_meters.clone();
        plain.0.insert("ammeter".into(), Value::B(false));
        plain.0.insert("voltmeter".into(), Value::S("None".into()));
        let base = (cd.render)(&plain);
        for vm in ["Across component 1", "Across component 2", "Across the battery"] {
            let mut q = plain.clone();
            q.0.insert("voltmeter".into(), Value::S(vm.into()));
            assert_eq!((cd.render)(&q), base, "the voltmeter changed a capacitor circuit ({vm})");
        }
        let mut a = plain.clone();
        a.0.insert("ammeter".into(), Value::B(true));
        assert_eq!((cd.render)(&a), base, "the ammeter changed a capacitor circuit");
        if let Ok(dir) = std::env::var("VERSHION_PNG_DIR") {
            for (tag, list) in [
                ("caps-series-key", vec![]),
                ("caps-series-student", vec![(SHOW_ANSWERS, Value::B(false))]),
                ("caps-parallel-key", vec![("layout", Value::S("In parallel".into())), ("n", Value::I(2)), ("r2_val", Value::S("22 µF".into()))]),
            ] {
                let mut q = with_meters.clone();
                for (k, v) in list { q.0.insert(k.to_string(), v); }
                let _ = std::fs::write(format!("{dir}/circuit-{tag}.png"), to_png(&(cd.render)(&q), 1.0).unwrap());
            }
        }
    }

    #[test]
    fn a_capacitor_value_in_ohms_is_left_out_rather_than_read_as_farads() {
        use super::instr::circuit_warnings;
        let cd = defs().into_iter().find(|d| d.id == "circuit-diagram").unwrap();
        let mut p = Params::defaults(&(cd.params)());
        p.0.insert("components".into(), Value::S("Capacitors".into()));
        p.0.insert("solve".into(), Value::S("Readings and results".into()));
        p.0.insert("n".into(), Value::I(1));
        // the default value is still in ohms
        assert_eq!(p.s("r1_val"), "2 Ω");
        assert!(!circuit_warnings(&p).is_empty(), "the note explains why nothing is worked out");
        let svg = (cd.render)(&p);
        assert!(!svg.contains("2 µF"), "2 Ω must not be read as 2 farads");
        assert!(svg.contains('?'), "the value is unknown, not a number");
        // typed as a capacitance, it works out
        p.0.insert("r1_val".into(), Value::S("2 µF".into()));
        assert!(circuit_warnings(&p).is_empty(), "no note once the values are capacitances");
        assert!((cd.render)(&p).contains("2 µF"));
        // an open switch leaves them uncharged: zero, not sixty microcoulombs
        p.0.insert("r1_val".into(), Value::S("10 µF".into()));
        p.0.insert("switch".into(), Value::S("Open".into()));
        let open = (cd.render)(&p);
        assert!(open.contains("0 C") || open.contains("0 µC"), "an open switch charges nothing:\n{open}");
        assert!(!open.contains("120 µC"), "the battery is not charging an open circuit");
    }

    #[test]
    fn a_combination_circuit_is_one_part_in_series_with_a_parallel_pair() {
        let cd = defs().into_iter().find(|d| d.id == "circuit-diagram").unwrap();
        let mut p = Params::defaults(&(cd.params)());
        for (k, v) in [
            ("layout", Value::S("In series and parallel".into())), ("n", Value::I(3)),
            ("r1_val", Value::S("2 Ω".into())), ("r2_val", Value::S("4 Ω".into())), ("r3_val", Value::S("4 Ω".into())),
            ("cell_val", Value::S("12 V".into())), ("solve", Value::S("Readings and results".into())),
        ] { p.0.insert(k.into(), v); }
        // 2 + (4 ∥ 4) = 4 Ω, so 3 A from 12 V: 6 V on the series part and 6 V across each branch, 1.5 A each
        let svg = (cd.render)(&p);
        assert!(svg.contains("3 A"), "the total current:\n{svg}");
        assert!(svg.contains("1.5 A"), "each branch takes half");
        assert!(svg.contains("6 V"), "the voltages");
        // the layout puts the first part on the top wire and the rest in branches
        let l = instr::circuit_layout(&p);
        assert!(l.combo, "the layout knows it is a combination");
        assert!((l.comps[0].1 - l.top_y).abs() < 1e-9, "the series part sits on the top wire");
        assert!(l.comps[1].1 > l.top_y && l.comps[2].1 > l.top_y, "the branches hang below it");
        // the same with capacitors: 10 µF in series with two 10 µF in parallel
        let mut c = p.clone();
        c.0.insert("components".into(), Value::S("Capacitors".into()));
        for k in ["r1_val", "r2_val", "r3_val"] { c.0.insert(k.into(), Value::S("10 µF".into())); }
        let cs = (cd.render)(&c);
        // 10 µF in series with (10 + 10) µF is 20/3 µF, and 12 V puts 80 µC on each
        assert!(cs.contains("6.67 µF"), "the equivalent capacitance:\n{cs}");
        assert!(cs.contains("80 µC"), "the charge");
        // the layout's own fallback is spoken, not silent
        let mut one = p.clone();
        one.0.insert("n".into(), Value::I(1));
        let w = instr::circuit_warnings(&one);
        assert!(w.iter().any(|s| s.contains("drawn as a series circuit")), "{w:?}");
        assert!(instr::circuit_warnings(&p).is_empty(), "no note for a real combination");
        if let Ok(dir) = std::env::var("VERSHION_PNG_DIR") {
            for (tag, q) in [("combo-key", p.clone()), ("combo-student", { let mut s = p.clone(); s.0.insert(SHOW_ANSWERS.into(), Value::B(false)); s }), ("combo-caps", c.clone())] {
                let _ = std::fs::write(format!("{dir}/circuit-{tag}.png"), to_png(&(cd.render)(&q), 1.0).unwrap());
            }
        }
    }

    #[test]
    fn the_circuit_keeps_its_meters_and_labels_inside_the_canvas() {
        let cd = defs().into_iter().find(|d| d.id == "circuit-diagram").unwrap();
        let mk = |list: &[(&'static str, Value)]| {
            let mut p = Params::defaults(&(cd.params)());
            for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
            p
        };
        // the combination layout puts the top wire high, so a meter above it would leave the canvas
        for vm in ["Across component 1", "Across component 2", "Across component 4"] {
            let p = mk(&[("layout", Value::S("In series and parallel".into())), ("n", Value::I(4)),
                ("voltmeter", Value::S(vm.into())), ("cell_val", Value::S("12 V".into())),
                ("solve", Value::S("Readings and results".into()))]);
            let svg = (cd.render)(&p);
            assert!(!svg.contains("cy=\"-"), "{vm}: a circle was drawn above the canvas");
            // nothing is clipped off the right: every circle centre is inside the canvas width
            let w: f64 = svg.split("width=\"").nth(1).and_then(|s| s.split('"').next()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            for part in svg.split("<circle cx=\"").skip(1) {
                let cx: f64 = part.split('"').next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                assert!(cx + 18.0 <= w, "{vm}: a circle at {cx} runs off the {w} px canvas");
            }
        }
        // long values must not print into each other in a combination
        let p = mk(&[("layout", Value::S("In series and parallel".into())), ("n", Value::I(3)),
            ("r1_val", Value::S("4.7 kΩ".into())), ("r2_val", Value::S("4.7 kΩ".into())), ("r3_val", Value::S("4.7 kΩ".into()))]);
        let l = instr::circuit_layout(&p);
        let gap = l.comps[2].0 - l.comps[1].0;
        assert!(gap >= 56.0 + 6.0 * 11.0 + 3.0 * 11.0, "branch pitch {gap} too small for 4.7 kΩ and R_1");
        // a capacitor is never labelled in ohms
        let c = mk(&[("components", Value::S("Capacitors".into())), ("n", Value::I(2)),
            ("r1_val", Value::S("10 µF".into())), ("r2_val", Value::S("4 Ω".into()))]);
        let svg = (cd.render)(&c);
        assert!(!svg.contains("4 Ω"), "a capacitor was labelled with a resistance:\n{svg}");
        assert!(svg.contains("10 µF"), "the real capacitance stays");
        // and a battery the figure does not print cannot be used to work out the answers
        let h = mk(&[("cell_val_mode", Value::S("None".into())), ("solve", Value::S("Readings and results".into())), ("n", Value::I(2))]);
        let hs = (cd.render)(&h);
        assert!(!hs.contains("6 A") && !hs.contains("12 Ω"), "answers came from a hidden battery:\n{hs}");
        if let Ok(dir) = std::env::var("VERSHION_PNG_DIR") {
            for (tag, q) in [
                ("combo-vm1", mk(&[("layout", Value::S("In series and parallel".into())), ("n", Value::I(4)), ("voltmeter", Value::S("Across component 1".into())), ("solve", Value::S("Readings and results".into()))])),
                ("combo-vm4", mk(&[("layout", Value::S("In series and parallel".into())), ("n", Value::I(4)), ("voltmeter", Value::S("Across component 4".into())), ("solve", Value::S("Readings and results".into()))])),
                ("combo-long", mk(&[("layout", Value::S("In series and parallel".into())), ("n", Value::I(3)), ("r1_val", Value::S("4.7 kΩ".into())), ("r2_val", Value::S("4.7 kΩ".into())), ("r3_val", Value::S("4.7 kΩ".into()))])),
                ("series-long", mk(&[("n", Value::I(4)), ("switch", Value::S("Closed".into())), ("r1_val", Value::S("4.7 kΩ".into())), ("r2_val", Value::S("4.7 kΩ".into())), ("r3_val", Value::S("4.7 kΩ".into())), ("r4_val", Value::S("4.7 kΩ".into()))])),
            ] {
                let _ = std::fs::write(format!("{dir}/circuit-{tag}.png"), to_png(&(cd.render)(&q), 1.0).unwrap());
            }
        }
    }

    #[test]
    fn nine_physics_generators() {
        let ids: Vec<_> = defs().iter().map(|d| d.id).collect();
        assert_eq!(ids.len(), 9);
        assert!(defs().iter().all(|d| d.subject == Subject::Physics));
    }

    /// Defaults, every choice option, and min/max of every numeric param render validly.
    #[test]
    fn physics_sweeps_params() {
        for d in defs() {
            let defs_ = (d.params)();
            let base = Params::defaults(&defs_);
            check(&d, &base, "default");
            for pd in &defs_ {
                match &pd.kind {
                    Kind::Choice(opts) => for o in opts {
                        let mut p = base.clone();
                        p.0.insert(pd.key.into(), Value::S((*o).into()));
                        check(&d, &p, "sweep");
                    },
                    Kind::Float { min, max, .. } => for v in [*min, *max] {
                        let mut p = base.clone();
                        p.0.insert(pd.key.into(), Value::F(v));
                        check(&d, &p, "sweep");
                    },
                    Kind::Int { min, max } => for v in [*min, *max] {
                        let mut p = base.clone();
                        p.0.insert(pd.key.into(), Value::I(v));
                        check(&d, &p, "sweep");
                    },
                    Kind::Bool => {
                        let mut p = base.clone();
                        p.0.insert(pd.key.into(), Value::B(true));
                        check(&d, &p, "sweep");
                    }
                    Kind::Text | Kind::Long | Kind::List | Kind::Data { .. } | Kind::Color(_) => {
                        let mut p = base.clone();
                        p.0.insert(pd.key.into(), Value::S(String::new()));
                        check(&d, &p, "sweep");
                    }
                }
            }
        }
    }

    /// Every chip key resolves to a real param for every slot it can fill, and the count param's
    /// range matches the card's maximum.
    #[test]
    fn physics_chips_fill_real_params() {
        for d in defs() {
            let Some(card) = chips(d.id) else { continue };
            let pdefs = (d.params)();
            let count = pdefs.iter().find(|p| p.key == card.count_key).unwrap_or_else(|| panic!("{}: no count key", d.id));
            match count.kind { Kind::Int { max, .. } => assert_eq!(max, card.max, "{}", d.id), _ => panic!("{}: count not int", d.id) }
            for slot in 2..=card.max {
                for chip in &card.chips {
                    for (k, v) in &chip.set {
                        let key = k.replace("{n}", &slot.to_string());
                        let pd = pdefs.iter().find(|p| p.key == key).unwrap_or_else(|| panic!("{}: chip {} sets unknown {key}", d.id, chip.label));
                        if let (Kind::Choice(opts), Value::S(s)) = (&pd.kind, v) {
                            assert!(opts.contains(&s.as_str()), "{}: {key} has no option {s}", d.id);
                        }
                    }
                }
            }
        }
        for id in ["free-body-diagram", "vector-diagram", "motion-graphs", "circuit-diagram", "inclined-plane"] {
            assert!(chips(id).is_some(), "{id}");
        }
        assert_eq!(chips("free-body-diagram").unwrap().max, 8);
        assert_eq!(chips("motion-graphs").unwrap().max, 6);
    }

    /// Old two-block specs still give two blocks; `n` gives 1-3 objects foot first, in order.
    #[test]
    fn incline_rows() {
        let all = defs();
        let d = all.iter().find(|d| d.id == "inclined-plane").unwrap();
        let p = with(d, &[("two", Value::B(true))]);
        assert_eq!(mech::incline_count(&p), 2);
        let objs = mech::incline_objects(&p, 400.0);
        assert!(objs.iter().all(|o| o.kind == "Block"));
        for (n, joined) in [(1, "By strings"), (2, "By strings"), (3, "By strings"), (3, "Touching")] {
            let p = with(d, &[("n", Value::I(n)), ("joined", s(joined)), ("obj2_kind", s("Cart")), ("obj3_kind", s("Ball"))]);
            let o = mech::incline_objects(&p, 550.0);
            assert_eq!(o.len(), n as usize);
            for w in o.windows(2) {
                let gap = (w[1].s - mech::obj_half_len(w[1].kind, w[1].sz)) - (w[0].s + mech::obj_half_len(w[0].kind, w[0].sz));
                if joined == "Touching" { assert!(gap.abs() < 1e-6); } else { assert!(gap > 7.0); }
            }
        }
        // single object: unchanged centre at pos * slope length
        let p = with(d, &[]);
        let o = mech::incline_objects(&p, 400.0);
        assert!((o[0].s - 0.55 * 400.0).abs() < 1e-9);
    }

    fn with(d: &FigureDef, kv: &[(&str, Value)]) -> Params {
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in kv { p.0.insert((*k).into(), v.clone()); }
        p
    }
    fn s(x: &str) -> Value { Value::S(x.into()) }

    /// Showcase renders used for visual checks (written to the scratch dir).
    #[test]
    fn physics_showcase() {
        let all = defs();
        let get = |id: &str| all.iter().find(|d| d.id == id).unwrap().clone();
        let d = get("free-body-diagram");
        check(&d, &with(&d, &[("body", s("Block")), ("n", Value::I(4)), ("notation", s("Arrow")), ("motion", s("Both")), ("mdir", Value::F(30.0))]), "show1");
        let d = get("vector-diagram");
        check(&d, &with(&d, &[("n", Value::I(3)), ("v3_ang", Value::F(200.0)), ("mark_angle", Value::B(true)), ("components", Value::B(true))]), "show1");
        let d = get("inclined-plane");
        check(&d, &with(&d, &[("angle", Value::F(40.0)), ("gravity", Value::B(true)), ("normal", Value::B(true)), ("friction", s("Up the ramp")), ("applied", s("Up the ramp")), ("velocity", s("Down the ramp")), ("mark_len", Value::B(true)), ("mark_h", Value::B(true)), ("surface", s("Rough"))]), "show1");
        let d = get("pulley");
        check(&d, &with(&d, &[("tension", Value::B(true)), ("gravity", Value::B(true)), ("accel", s("Right falls")), ("third", Value::B(true))]), "show1");
        for su in ["Table", "Ramp", "Block & tackle"] {
            check(&d, &with(&d, &[("setup", s(su)), ("tension", Value::B(true)), ("accel", s("Right falls"))]), &format!("show_{}", su.replace(' ', "").replace('&', "")));
        }
        let d = get("projectile-motion");
        check(&d, &with(&d, &[("from", s("A cliff")), ("comps", Value::B(true)), ("steps", Value::I(6)), ("mark_h", Value::B(true)), ("mark_r", Value::B(true)), ("show_g", Value::B(true))]), "show1");
        let d = get("motion-graphs");
        check(&d, &with(&d, &[("show", s("All three, stacked")), ("letters", Value::B(true))]), "show1");
        let d = get("waves");
        check(&d, &with(&d, &[("kind", s("Both")), ("mark_ct", Value::B(true))]), "show1");
        let d = get("spring-scale");
        check(&d, &with(&d, &[("units", s("Both")), ("reading", Value::F(6.2)), ("answer", Value::B(true))]), "show1");
        let d = get("circuit-diagram");
        check(&d, &with(&d, &[("switch", s("Open")), ("ammeter", Value::B(true)), ("voltmeter", s("Across component 2")), ("polarity", Value::B(true))]), "show1");
        check(&d, &with(&d, &[("layout", s("In parallel")), ("n", Value::I(3)), ("r2_kind", s("Bulb")), ("ammeter", Value::B(true)), ("switch", s("Closed")), ("voltmeter", s("Across the battery"))]), "show2");
    }
}
#[cfg(test)]
mod dbg_svg {
    #[test]
    fn dump_svg() {
        let Ok(spec) = std::env::var("PHYS_SVG") else { return };
        let (id, js) = spec.split_once('|').unwrap();
        let def = super::defs().into_iter().find(|d| d.id == id).unwrap();
        let p = crate::figures::params_from_json(&def, &serde_json::from_str(js).unwrap());
        println!("{}", (def.render)(&p));
    }
}
#[cfg(test)]
mod dbg_examples {
    use super::*;
    /// Every example applies only known keys and renders; with PHYS_EX_DIR set the renders
    /// are written there as <id>_ex<k>.png for eyeballing.
    #[test]
    fn physics_examples_render() {
        let dir = std::env::var("PHYS_EX_DIR").ok();
        for d in defs() {
            let defs_ = (d.params)();
            let ex = examples(d.id).unwrap_or_default();
            for (k, e) in ex.iter().enumerate() {
                let mut p = Params::defaults(&defs_);
                for (key, v) in &e.params {
                    assert!(defs_.iter().any(|pd| pd.key == *key), "{}: example '{}' uses unknown key {key}", d.id, e.name);
                    p.0.insert((*key).into(), v.clone());
                }
                p.normalize(&defs_, false);
                let png = to_png(&(d.render)(&p), 1.0).unwrap();
                if let Some(dir) = &dir {
                    let _ = std::fs::create_dir_all(dir);
                    std::fs::write(format!("{dir}/{}_ex{k}.png", d.id), png).unwrap();
                }
            }
        }
    }
}
