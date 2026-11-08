use super::*;

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }
fn mdl(list: &[(&'static str, Value)]) -> Model { model(&with(list), None) }

#[test]
fn tonicity_counts_only_solutes_that_cannot_cross() {
    assert_eq!(mdl(&[f("s1_in", 0.9), f("s1_out", 0.45)]).tonic, Tonic::Hypo);
    assert_eq!(mdl(&[f("s1_in", 0.9), f("s1_out", 0.9)]).tonic, Tonic::Iso);
    assert_eq!(mdl(&[f("s1_in", 0.9), f("s1_out", 1.8)]).tonic, Tonic::Hyper);
    // a second solute that crosses changes nothing; one that cannot, does
    let crossing = [f("s1_in", 0.9), f("s1_out", 0.9), b("s2_on", true), b("s2_cross", true), f("s2_in", 5.0), f("s2_out", 0.0)];
    assert_eq!(mdl(&crossing).tonic, Tonic::Iso);
    let stuck = [f("s1_in", 0.9), f("s1_out", 0.9), b("s2_on", true), f("s2_in", 5.0), f("s2_out", 0.0)];
    assert_eq!(mdl(&stuck).tonic, Tonic::Hypo);
    assert_eq!(mdl(&stuck).flow(), Flow::In);
    assert_eq!(mdl(&[f("s1_in", 0.9), f("s1_out", 5.0)]).flow(), Flow::Out);
}

#[test]
fn the_three_up_is_half_same_and_double() {
    let p = with(&[f("s1_in", 0.9), f("s1_out", 7.0)]);
    let outs: Vec<Tonic> = [0.5, 1.0, 2.0].iter().map(|m| model(&p, Some(*m)).tonic).collect();
    assert_eq!(outs, vec![Tonic::Hypo, Tonic::Iso, Tonic::Hyper]);
    let svg = render_of(&[f("s1_out", 7.0)]);
    for t in ["Hypotonic solution", "Isotonic solution", "Hypertonic solution", "Swells", "Normal", "Shrinks (crenated)", "0.45% NaCl", "0.9% NaCl", "1.8% NaCl"] {
        assert!(svg.contains(t), "missing {t}");
    }
}

#[test]
fn each_scene_names_its_result_for_each_tonicity() {
    let words = |scene: &str, out: f64| result_word(&mdl(&[s("scene", scene), f("s1_in", 0.9), f("s1_out", out)]));
    assert_eq!(words("Plant cell", 0.45), "Turgid");
    assert_eq!(words("Plant cell", 0.9), "Flaccid");
    assert_eq!(words("Plant cell", 1.8), "Plasmolyzed");
    assert_eq!(words("Dialysis bag", 5.0), "Bag shrinks");
    assert_eq!(words("Dialysis bag", 0.1), "Bag swells");
    assert_eq!(words("Split beaker", 5.0), "Level rises on side B");
    assert_eq!(words("U-tube", 0.1), "Level rises on side A");
    assert_eq!(words("U-tube", 0.9), "Levels stay equal");
}

#[test]
fn a_red_blood_cell_bursts_only_when_asked_and_only_when_very_hypotonic() {
    assert_eq!(result_word(&mdl(&[f("s1_out", 0.45)])), "Swells");
    assert_eq!(result_word(&mdl(&[f("s1_out", 0.45), b("burst", true)])), "Bursts (lysis)");
    assert_eq!(result_word(&mdl(&[f("s1_out", 0.8), b("burst", true)])), "Swells");
    assert_eq!(result_word(&mdl(&[f("s1_out", 1.8), b("burst", true)])), "Shrinks (crenated)");
}

fn count_fill(svg: &str, fill: &str) -> usize { svg.matches(&format!("fill=\"{fill}\" stroke=\"#333\" stroke-width=\"0.9\"")).count() }

#[test]
fn particles_are_as_crowded_as_the_concentration() {
    // an outside twice as concentrated has about twice the solute particles per area
    let one = render_of(&[s("show", "Start"), f("s1_in", 0.9), f("s1_out", 0.9)]);
    let two = render_of(&[s("show", "Start"), f("s1_in", 0.9), f("s1_out", 1.8)]);
    let (a, c) = (count_fill(&one, "#a3a3a3"), count_fill(&two, "#a3a3a3"));
    assert!(c > a + a / 4, "{a} vs {c}");
    // zero concentration has no particles of that solute at all (the key's sample is the one left)
    let none = render_of(&[s("show", "Start"), f("s1_in", 0.0), f("s1_out", 0.0)]);
    assert_eq!(count_fill(&none, "#a3a3a3"), 2, "the key sample and its scale box");
}

#[test]
fn a_solute_that_crosses_evens_out_and_a_stuck_one_does_not() {
    let base = vec![s("scene", "Split beaker"), f("s1_in", 0.9), f("s1_out", 0.9), b("s2_on", true), f("s2_in", 3.0), f("s2_out", 0.0), s("s2_shade", "Black"), s("s2_size", "S")];
    let with_extra = |show: &str, cross: bool| {
        let mut l = base.clone();
        l.push(s("show", show));
        l.push(b("s2_cross", cross));
        render_of(&l)
    };
    // the particle count of the start is kept in the result when the solute cannot cross
    assert_eq!(count_fill(&with_extra("Start", false), "#1e1e1e"), count_fill(&with_extra("Result", false), "#1e1e1e"));
    // when it crosses, the total is kept but it is now spread over both sides
    assert_eq!(count_fill(&with_extra("Start", true), "#1e1e1e"), count_fill(&with_extra("Result", true), "#1e1e1e"));
    assert_ne!(with_extra("Result", false), with_extra("Result", true));
}

#[test]
fn the_beaker_level_rises_on_the_concentrated_side() {
    let g = |out: f64| {
        let m = mdl(&[s("scene", "Split beaker"), f("s1_out", out)]);
        let pal = palette(&with(&[]));
        let g = geo(&m, Kind::Result, &pal);
        (g.inr.y0, g.outr.y0)
    };
    let (a, bb) = g(5.0);
    assert!(bb < a, "side B's surface is higher (smaller y): {a} {bb}");
    let (a, bb) = g(0.1);
    assert!(a < bb);
    let (a, bb) = g(0.9);
    assert!((a - bb).abs() < 1e-9);
}

#[test]
fn blank_labels_are_empty_in_the_student_copy_and_filled_in_the_key() {
    let list = vec![s("lab_tonic", "Blank"), s("lab_result", "Blank"), s("lab_conc", "Blank"), s("arr_water", "Blank boxes"), b("answer_key", true)];
    let mut key = list.clone();
    key.push(b(SHOW_ANSWERS, true));
    let mut student = list.clone();
    student.push(b(SHOW_ANSWERS, false));
    let (k, st) = (render_of(&key), render_of(&student));
    for t in ["Hypotonic solution", "Swells", "0.9% NaCl", "Answer key"] {
        assert!(k.contains(t), "key lacks {t}");
        assert!(!st.contains(t), "student copy shows {t}");
    }
    // the explanation names the effect and the reason
    assert!(k.contains("crenation") && k.contains("hypertonic side"));
    assert!(!st.contains("osmosis"));
    // boxes are drawn in both; the key also draws the arrows in them
    assert!(st.contains("<rect") && k.matches("<polygon").count() > st.matches("<polygon").count());
}

#[test]
fn arrows_follow_the_water_and_can_be_turned_off() {
    let arrows = |l: &[(&'static str, Value)]| render_of(l).matches("<polygon points").count();
    let on = arrows(&[s("show", "Start")]);
    let off = arrows(&[s("show", "Start"), s("arr_water", "None")]);
    assert!(on >= off + 2);
    // an isotonic cell has an equal pair each way (two arrows per side), a hypotonic one a single arrow per side
    let iso = arrows(&[s("show", "Start"), f("s1_out", 0.9)]);
    let hypo = arrows(&[s("show", "Start")]);
    assert!(iso > hypo);
}

#[test]
fn solute_arrows_need_a_crossing_solute_and_can_be_turned_off() {
    let base = vec![s("scene", "Split beaker"), s("show", "Start"), b("s2_on", true), f("s2_in", 2.0), f("s2_out", 0.0)];
    let polys = |l: &[(&'static str, Value)]| render_of(l).matches("<polygon points").count();
    let none = polys(&base);
    let mut cross = base.clone();
    cross.push(b("s2_cross", true));
    assert!(polys(&cross) > none);
    cross.push(s("arr_solute", "None"));
    assert_eq!(polys(&cross), none);
}

#[test]
fn every_scene_and_show_renders_and_print_is_grey_only() {
    for scene in SCENES {
        for show in ["Start", "Result", "Both", "Three-up"] {
            let svg = render_of(&[s("scene", scene), s("show", show), s("style", "Print"), b("s2_on", true), b("s3_on", true), s("s1_charge", "2+"), s("s2_charge", "\u{2212}"), s("lab_struct", "Text"), b("answer_key", true)]);
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{scene} {show}");
            for hex in svg.to_lowercase().split('#').skip(1) {
                let h: String = hex.chars().take(6).collect();
                if h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()) {
                    assert!(h[0..2] == h[2..4] && h[2..4] == h[4..6], "{scene} {show}: colour #{h} in Print");
                }
            }
            let colour = render_of(&[s("scene", scene), s("show", show)]);
            assert_ne!(colour, svg);
        }
    }
}

#[test]
fn the_figure_is_deterministic_and_the_seed_moves_the_particles() {
    let a = render_of(&[]);
    assert_eq!(a, render_of(&[]));
    assert_ne!(a, render_of(&[("seed", Value::I(8))]));
}

#[test]
fn a_subscript_in_a_name_and_a_title_are_drawn() {
    let svg = render_of(&[s("s1_name", "C_6H_12O_6"), s("title", "Cells in salt"), s("show", "Start")]);
    assert!(svg.contains("Cells in salt"));
    assert!(svg.contains("<tspan"), "subscript runs");
    assert!(!svg.contains("C_6"));
}

#[test]
fn nonsense_values_still_render() {
    let svg = render_of(&[f("s1_in", 1e30), f("s1_out", f64::NAN), s("scene", "Nonsense"), s("show", "???"), s("title", "a < b & c")]);
    assert!(svg.starts_with("<svg"));
    let none = render_of(&[b("s1_on", false), b("show_water", false), s("show", "Both"), s("scene", "U-tube")]);
    assert!(none.starts_with("<svg"));
}

#[test]
fn examples_render_and_cover_every_scene() {
    let ex = examples(ID).unwrap();
    let mut scenes = std::collections::HashSet::new();
    for e in &ex {
        let mut p = Params::defaults(&params());
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&params(), false);
        scenes.insert(p.s("scene").to_string());
        assert!(render(&p).starts_with("<svg"), "{}", e.name);
    }
    for sc in SCENES { assert!(scenes.contains(sc), "no example for {sc}"); }
}

#[test]
fn the_shuffle_chip_sets_a_fresh_layout() {
    let c = chips();
    assert_eq!(c.chips[0].set[0], ("seed", Value::I(i64::MIN)));
}

fn grey_count(svg: &str, fill: &str) -> usize { svg.matches(&format!("fill=\"{fill}\" stroke=\"#333\" stroke-width=\"0.9\"")).count() }

#[test]
fn density_keeps_rising_with_concentration_and_water_is_still_there() {
    // three solutes of one size outside only, at 1, 2.5 and 5: more particles each time, and water at the highest
    let svg = render_of(&[s("show", "Start"), s("scene", "Split beaker"), f("s1_in", 0.0), f("s1_out", 1.0), b("s2_on", true), f("s2_in", 0.0), f("s2_out", 2.5), b("s3_on", true), f("s3_in", 0.0), f("s3_out", 5.0),
        s("s1_size", "M"), s("s2_size", "M"), s("s3_size", "M"), s("s1_shade", "Grey"), s("s2_shade", "Dark grey"), s("s3_shade", "Black"), b("key", false)]);
    let (a, bb, c) = (grey_count(&svg, "#a3a3a3"), grey_count(&svg, "#666666"), grey_count(&svg, "#1e1e1e"));
    assert!(a < bb && bb < c, "{a} {bb} {c}");
    assert!(svg.matches("fill=\"#6ba6dd\"").count() > 5, "water disappeared at the highest concentration");
    // big particles at a high concentration too
    for size in ["L", "XL"] {
        let hi = render_of(&[s("show", "Start"), s("s1_size", size), f("s1_in", 8.0), f("s1_out", 12.0), b("key", false)]);
        let lo = render_of(&[s("show", "Start"), s("s1_size", size), f("s1_in", 8.0), f("s1_out", 5.0), b("key", false)]);
        assert!(grey_count(&hi, "#a3a3a3") > grey_count(&lo, "#a3a3a3"), "{size}");
    }
    // M units: 0.9 M and 1.8 M differ
    let m09 = render_of(&[s("show", "Start"), s("unit", "M"), f("s1_in", 0.9), f("s1_out", 0.9), b("key", false)]);
    let m18 = render_of(&[s("show", "Start"), s("unit", "M"), f("s1_in", 0.9), f("s1_out", 1.8), b("key", false)]);
    assert!(grey_count(&m18, "#a3a3a3") > grey_count(&m09, "#a3a3a3") + 3);
}

#[test]
fn the_key_says_what_one_particle_is() {
    let svg = render_of(&[]);
    assert!(svg.contains("In a box this size,") && svg.contains("1 particle = 0.25%"), "{}", svg.len());
    let m = render_of(&[s("unit", "mM"), f("s1_in", 150.0), f("s1_out", 300.0)]);
    assert!(m.contains("1 particle = ") && m.contains(" mM"));
}

#[test]
fn a_crossing_solute_is_keyed_in_each_panel_of_a_three_up() {
    let svg = render_of(&[b("s2_on", true), b("s2_cross", true), s("s2_name", "Urea"), f("s2_in", 2.0), f("s2_out", 1.0), b("answer_key", true)]);
    // in the three-up the outside is half, the same and double the inside: out, nothing, in
    assert!(svg.contains("A: Urea can cross the membrane, so it moves from inside the cell to outside the cell"), "A");
    assert!(svg.contains("B: Urea can cross the membrane, so it is already equal on both sides"), "B");
    assert!(svg.contains("C: Urea can cross the membrane, so it moves from outside the cell to inside the cell"), "C");
}

#[test]
fn the_key_uses_the_scenes_side_names_capitalised() {
    let beaker = render_of(&[s("scene", "U-tube"), s("show", "Both"), f("s1_out", 5.0), b("answer_key", true)]);
    assert!(beaker.contains("Side A 0.9%, Side B 5%"), "split names");
    assert!(!beaker.contains("side a") && !beaker.contains("side b"));
    let bag = render_of(&[s("scene", "Dialysis bag"), s("show", "Both"), b("s2_on", true), b("s2_cross", true), f("s2_in", 0.0), f("s2_out", 2.0), b("answer_key", true)]);
    assert!(bag.contains("moves from the beaker to the bag until equal"), "bag");
    assert!(!bag.contains("in the beaker to in the bag"));
    let split = render_of(&[s("scene", "Split beaker"), s("show", "Both"), b("s2_on", true), b("s2_cross", true), f("s2_in", 0.0), f("s2_out", 2.0), b("answer_key", true)]);
    assert!(split.contains("moves from Side B to Side A until equal"));
}

#[test]
fn a_swollen_bag_stays_inside_the_beaker() {
    for out in [0.0, 0.01, 0.1, 0.45] {
        let svg = render_of(&[s("scene", "Dialysis bag"), s("show", "Result"), f("s1_out", out), b("key", false)]);
        let at = svg.find("stroke-width=\"1.8\" stroke-linejoin").unwrap();
        let start = svg[..at].rfind("<polygon points=\"").unwrap() + 17;
        let pts = svg[start..].split('"').next().unwrap();
        let maxy = pts.split(' ').map(|p| p.split(',').nth(1).unwrap().parse::<f64>().unwrap()).fold(0.0, f64::max);
        assert!(maxy < 174.0, "out {out}: bag reaches y {maxy} (floor 176)");
    }
}

#[test]
fn a_plasmolyzed_vacuole_still_shows_solute() {
    let svg = render_of(&[s("scene", "Plant cell"), s("show", "Result"), f("s1_in", 0.9), f("s1_out", 4.0), b("key", false)]);
    let pal = palette(&with(&[]));
    let m = mdl(&[s("scene", "Plant cell"), f("s1_in", 0.9), f("s1_out", 4.0)]);
    let g = geo(&m, Kind::Result, &pal);
    let mut inside = 0;
    for el in svg.split("<circle ").skip(1) {
        if !el.contains("fill=\"#a3a3a3\"") { continue; }
        let num = |k: &str| el.split(&format!("{k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse::<f64>().unwrap();
        let (x, y) = (num("cx"), num("cy"));
        if x > g.inr.x0 && x < g.inr.x1 && y > g.inr.y0 && y < g.inr.y1 { inside += 1; }
    }
    assert!(inside >= 1, "no solute in the vacuole {:?} n={}", (g.inr.x0, g.inr.x1, g.inr.y0, g.inr.y1), svg.matches("fill=\"#a3a3a3\"").count());
}
