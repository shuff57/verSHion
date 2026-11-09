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
    assert_eq!(words("Split beaker", 5.0), "Level rises on Side B");
    assert_eq!(words("U-tube", 0.1), "Level rises on Side A");
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
    for size in ["XS", "M", "XL"] {
        for unit in ["%", "M"] {
            let svg = render_of(&[s("show", "Start"), s("scene", "Split beaker"), s("unit", unit), s("s1_size", size), f("s1_in", 0.9), f("s1_out", 1.8), b("key", false)]);
            let (l, r) = side_counts(&svg, "#a3a3a3");
            assert!(r as f64 >= 1.6 * l as f64 && l >= 2, "{size} {unit}: side A {l}, side B {r}");
        }
    }
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
    let svg = render_of(&[s("show", "Start"), s("scene", "Split beaker"), s("s1_name", "Aa"), s("s2_name", "Bb"), s("s3_name", "Cc"), f("s1_in", 0.0), f("s1_out", 1.0), b("s2_on", true), f("s2_in", 0.0), f("s2_out", 2.5), b("s3_on", true), f("s3_in", 0.0), f("s3_out", 5.0),
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
}

/// Solute particles (by fill) left and right of the membrane (x = 120) in a split scene's first group.
fn side_counts(svg: &str, fill: &str) -> (usize, usize) {
    let (mut l, mut r) = (0, 0);
    for el in svg.split("<circle ").skip(1) {
        if !el.contains(&format!("fill=\"{fill}\" stroke=\"#333\"")) { continue; }
        let cx: f64 = el.split("cx=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
        if cx < 120.0 { l += 1 } else { r += 1 }
    }
    (l, r)
}

#[test]
fn no_solutes_means_no_scale_box_in_the_key() {
    let none = render_of(&[b("s1_on", false), s("show", "Start")]);
    assert!(!none.contains("stroke-dasharray=\"3 2\"") && !none.contains("1 particle ="));
    assert!(render_of(&[s("show", "Start")]).contains("1 particle ="));
}

#[test]
fn the_membrane_label_takes_the_teachers_words() {
    for (scene, dflt) in [("Red blood cell", "Cell membrane"), ("Split beaker", "Selectively permeable membrane"), ("Dialysis bag", "Dialysis tubing")] {
        let base = render_of(&[s("scene", scene), s("show", "Start")]);
        assert!(base.contains(dflt), "{scene}");
        let own = flat(&render_of(&[s("scene", scene), s("show", "Start"), s("lab_membrane_text", "Plasma membrane (lipid bilayer)")]));
        assert!(own.contains("Plasma membrane (lipid bilayer)") && !own.contains(dflt), "{scene}");
        // blank line: the wording is the answer, shown in the key only
        let blank = |ans: bool| render_of(&[s("scene", scene), s("show", "Start"), s("lab_membrane", "Blank line"), s("lab_membrane_text", "Plasma membrane"), b(SHOW_ANSWERS, ans)]);
        assert!(flat(&blank(true)).contains("Plasma membrane") && !flat(&blank(false)).contains("Plasma membrane"), "{scene}");
    }
}

#[test]
fn every_particle_size_still_shows_in_a_plasmolyzed_vacuole() {
    let pal = palette(&with(&[]));
    let fills = [("XS", "#a3a3a3"), ("S", "#a3a3a3"), ("M", "#a3a3a3"), ("L", "#a3a3a3"), ("XL", "#a3a3a3")];
    for (size, fill) in fills {
        for show in ["Result", "Three-up"] {
            let svg = render_of(&[s("scene", "Plant cell"), s("show", show), s("s1_size", size), f("s1_in", 0.9), f("s1_out", 4.0), b("key", false)]);
            // the plasmolyzed panel: the only one for Result, the third group (C, outside double) for the three-up
            let groups: Vec<&str> = svg.split("<g transform=\"translate(").skip(1).collect();
            let (panel, mult) = if show == "Result" { (groups[0], None) } else { (groups[2], Some(2.0)) };
            let m = model(&with(&[s("scene", "Plant cell"), s("s1_size", size), f("s1_in", 0.9), f("s1_out", 4.0)]), mult);
            assert_eq!(m.tonic, Tonic::Hyper);
            let g = geo(&m, Kind::Result, &pal);
            let mut inside = 0;
            for el in panel.split("<circle ").skip(1) {
                if !el.contains(&format!("fill=\"{fill}\" stroke=\"#333\"")) { continue; }
                let num = |k: &str| el.split(&format!("{k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse::<f64>().unwrap();
                let (x, y) = (num("cx"), num("cy"));
                if x > g.inr.x0 && x < g.inr.x1 && y > g.inr.y0 && y < g.inr.y1 { inside += 1; }
            }
            // the start has at least this many inside the cell; none may be lost when the vacuole shrinks
            let start = model(&with(&[s("scene", "Plant cell"), s("s1_size", size), f("s1_in", 0.9), f("s1_out", 4.0)]), mult);
            let want = want_count(start.sols[0].cin, geo(&start, Kind::Start, &pal).inr.area(), pick_pp_for(&start), start.sols[0].r);
            assert!(inside >= 1 && inside >= want.min(2), "{size} {show}: {inside} particles in the vacuole (start {want})");
        }
    }
}

fn pick_pp_for(m: &Model) -> f64 {
    let sl = Slot { m: m.clone(), kind: Kind::Result, letter: None, lead: true, arrows: true, result_blank: false, conc: true, caption: true };
    pick_pp(&[sl])
}

#[test]
fn up_to_three_crossing_arrows_each_cross_the_membrane_and_keep_their_labels_clear() {
    let pal = palette(&with(&[]));
    for (scene, size) in ["Red blood cell", "Plant cell", "Split beaker", "U-tube", "Dialysis bag"].into_iter().flat_map(|sc| ["Medium", "Large"].into_iter().map(move |sz| (sc, sz))) {
        let p = with(&[s("scene", scene), s("label_size", size), s("show", "Start"), b("s1_on", true), b("s2_on", true), b("s2_cross", true), f("s2_in", 2.0), f("s2_out", 0.0), b("s3_on", true), b("s3_cross", true), f("s3_in", 0.0), f("s3_out", 2.0), b("s1_cross", true), f("s1_in", 3.0), f("s1_out", 0.0)]);
        let c = cfg(&p);
        let m = model(&p, None);
        let g = geo(&m, Kind::Start, &pal);
        let pl = plan(&c, &m, &g, m.flow());
        assert_eq!(pl.sol.len(), 3, "{scene}");
        let (_, _, _, _, _) = g.shape;
        for (i, a) in pl.sol.iter().enumerate() {
            // each label is inside the panel and clear of every other arrow's box and label
            let r = a.lab.rect();
            assert!(r.0 >= 0.0 && r.2 <= PW + 1.0 && r.3 <= g.ph, "{scene} label {i} outside the panel: {r:?}");
            for (j, o) in pl.sol.iter().enumerate().filter(|(j, _)| *j != i) {
                let q = o.lab.rect();
                assert!(r.2 < q.0 || q.2 < r.0 || r.3 < q.1 || q.3 < r.1, "{scene}: labels {i} and {j} overlap");
                let bx = o.boxr;
                assert!(r.2 < bx.0 || bx.2 < r.0 || r.3 < bx.1 || bx.3 < r.1, "{scene}: label {i} sits on arrow {j}");
            }
            // the arrow crosses its membrane: its two ends are on opposite sides of the outline
            let crossing = match scene {
                "Split beaker" | "U-tube" => a.a.0 < 120.0 && a.b.0 > 120.0 || a.a.0 > 120.0 && a.b.0 < 120.0,
                "Dialysis bag" => { let (cx, _, rx, _, _) = g.shape; (a.a.0 - cx).abs().min((a.b.0 - cx).abs()) < rx + 1.0 && (a.a.0 - cx).abs().max((a.b.0 - cx).abs()) > rx * 0.5 }
                "Red blood cell" => { let (cx, cy, r, _, _) = g.shape; let d = |p: (f64, f64)| ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt(); d(a.a).min(d(a.b)) < r && d(a.a).max(d(a.b)) > r }
                _ => { let (_, cy, _, ry, _) = g.shape; (a.a.1 - cy).abs().min((a.b.1 - cy).abs()) < ry && (a.a.1 - cy).abs().max((a.b.1 - cy).abs()) > ry }
            };
            assert!(crossing, "{scene}: arrow {i} does not cross");
        }
        // the beaker's water arrow and caption are not covered either
        if scene == "Split beaker" { assert!(pl.sol[2].boxr.3 < 176.0); }
    }
}

#[test]
fn a_large_label_size_keeps_the_key_and_the_answer_key_inside_the_page() {
    for scene in ["Red blood cell", "Plant cell", "Split beaker", "Dialysis bag"] {
        let svg = render_of(&[s("scene", scene), s("label_size", "Large"), b("answer_key", true), b("s2_on", true), b("s3_on", true), s("s3_name", "A long solute name")]);
        let w: f64 = svg.split("width=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
        // every text's right edge (start x plus estimated width at its scaled size) stays inside the page
        for t in svg.split("<text ").skip(1) {
            let num = |k: &str| t.split(&format!("{k}=\"")).nth(1).and_then(|v| v.split('"').next()).and_then(|v| v.parse::<f64>().ok());
            let (x, fs) = (num("x").unwrap(), num("font-size").unwrap());
            let anchor = t.split("text-anchor=\"").nth(1).unwrap().split('"').next().unwrap();
            let body = t.split('>').nth(1).unwrap_or("").split('<').next().unwrap_or("");
            let width = text_w(body, fs);
            let right = match anchor { "middle" => x + width / 2.0, "end" => x, _ => x + width };
            let left = match anchor { "middle" => x - width / 2.0, "end" => x - width, _ => x };
            assert!(right <= w + 1.0 && left >= -1.0, "{scene}: text {body:?} runs to {left:.0}..{right:.0} of {w}");
        }
    }
}


#[test]
fn the_key_says_what_one_particle_is() {
    let svg = render_of(&[]);
    assert!(svg.contains("In a box this size,") && svg.contains("1 particle = "), "{}", svg.len());
    let m = render_of(&[s("unit", "mM"), f("s1_in", 150.0), f("s1_out", 300.0)]);
    assert!(m.contains("1 particle = ") && m.contains(" mOsm/L"));
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
    let beaker = flat(&render_of(&[s("scene", "U-tube"), s("show", "Both"), f("s1_out", 5.0), b("answer_key", true)]));
    assert!(beaker.contains("Side A is about 308 mOsm/L and Side B about 1711 mOsm/L"), "split names");
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

#[test]
fn flat_text_joins_the_wrapped_key_lines() {
    let svg = render_of(&[b("answer_key", true)]);
    assert!(flat(&svg).contains("moves from hypotonic to hypertonic."));
}

/// All the text of an SVG on one line (the answer key is wrapped over several <text> elements).
fn flat(svg: &str) -> String {
    svg.split("<text ").skip(1).map(|t| t.split('>').nth(1).unwrap_or("").split("</text").next().unwrap_or("").to_string()).collect::<Vec<_>>().join(" ")
}

#[test]
fn osmolarity_of_known_solutes_in_each_unit() {
    let near = |a: f64, b: f64| (a - b).abs() < 0.6;
    // percent (w/v): 0.9 % NaCl is 154 mM, two particles each
    assert!(near(mosm(0.9, "%", basis_of("NaCl")), 308.0));
    assert!(near(mosm(0.154, "M", basis_of("NaCl")), 308.0));
    assert!(near(mosm(154.0, "mM", basis_of("NaCl")), 308.0));
    assert!(near(mosm(0.9, "%", basis_of("KCl")), 241.4));
    assert!(near(mosm(1.0, "%", basis_of("CaCl_2")), 270.3));
    assert!(near(mosm(5.0, "%", basis_of("C_6H_12O_6")), 277.5));
    assert!(near(mosm(5.0, "%", basis_of("glucose")), 277.5));
    assert!(near(mosm(10.0, "%", basis_of("Sucrose")), 292.1));
    assert!(near(mosm(1.8, "%", basis_of("urea")), 299.7));
    // starch and protein count for almost nothing
    assert!(mosm(5.0, "%", basis_of("Starch")) < 2.0 && mosm(5.0, "%", basis_of("protein")) < 2.0);
    // not in the table: one particle per molecule, 100 g/mol assumed for a percent
    assert_eq!(basis_of("Mystery").how, How::Unknown);
    assert!(near(mosm(1.0, "%", basis_of("Mystery")), 100.0));
    assert!(near(mosm(0.1, "M", basis_of("Mystery")), 100.0));
}

#[test]
fn a_cell_in_dilute_salt_plus_sugar_is_hypotonic_by_osmolarity_not_by_percent() {
    // 0.9 % NaCl inside; outside 0.45 % NaCl + 0.5 % glucose: the percents add to 0.95 % (looks isotonic), the osmolarity is 182 vs 308
    let m = mdl(&[f("s1_in", 0.9), f("s1_out", 0.45), b("s2_on", true), f("s2_in", 0.0), f("s2_out", 0.5)]);
    assert!((m.ci - 308.0).abs() < 0.6 && (m.co - 181.6).abs() < 1.2, "{} {}", m.ci, m.co);
    assert_eq!(m.tonic, Tonic::Hypo);
    // sugar at the same percent is a quarter of the salt's osmolarity but twice... check both directions
    let salt_vs_sugar = mdl(&[s("s1_name", "Glucose"), f("s1_in", 5.0), f("s1_out", 0.9), b("s2_on", true), s("s2_name", "NaCl"), f("s2_in", 0.0), f("s2_out", 0.0)]);
    assert!(salt_vs_sugar.ci > salt_vs_sugar.co);
    // a protein inside, same salt both sides: nothing to move water
    let prot = mdl(&[b("s2_on", true), s("s2_name", "Protein"), f("s2_in", 5.0), f("s2_out", 0.0), f("s1_in", 0.9), f("s1_out", 0.9)]);
    assert_eq!(prot.tonic, Tonic::Iso);
    // 5 % glucose is isotonic with 0.9 % NaCl (277 vs 308, within 10 %) only loosely: it is hypotonic, not equal
    let g = mdl(&[s("s1_name", "NaCl"), f("s1_in", 0.9), b("s2_on", true), s("s2_name", "Glucose"), f("s2_in", 0.0), f("s2_out", 5.0), f("s1_out", 0.0)]);
    assert_eq!(g.tonic, Tonic::Hypo);
}

#[test]
fn the_key_states_the_comparison_in_osmolarity() {
    let svg = flat(&render_of(&[f("s1_out", 0.45), b("s2_on", true), f("s2_in", 0.0), f("s2_out", 0.5), s("show", "Start"), b("answer_key", true)]));
    assert!(svg.contains("inside the cell is about 308 mOsm/L and outside the cell about 182 mOsm/L"), "comparison");
    assert!(svg.contains("hypotonic"));
    assert!(svg.contains("NaCl: 58.44 g/mol, splits into 2 particles"));
    assert!(svg.contains("Glucose: 180.16 g/mol, splits into 1 particle;"));
    let unknown = flat(&render_of(&[s("s1_name", "Mystery"), s("show", "Start"), b("answer_key", true)]));
    assert!(unknown.contains("not in the table of known solutes") && unknown.contains("assumed 100 g/mol"));
    let big = flat(&render_of(&[s("s1_name", "Starch"), s("show", "Start"), b("answer_key", true)]));
    assert!(big.contains("a large molecule, taken as 50000 g/mol, so it adds almost no osmolarity"));
}

#[test]
fn particles_follow_osmolarity_so_salt_is_drawn_denser_than_the_same_percent_of_sugar() {
    // same typed 0.9 %, but NaCl is 308 mOsm/L and glucose 50: salt has about six times the particles
    let svg = render_of(&[s("show", "Start"), s("scene", "Split beaker"), f("s1_in", 0.9), f("s1_out", 0.9), b("s2_on", true), s("s2_name", "Glucose"), f("s2_in", 0.9), f("s2_out", 0.9), s("s2_shade", "Black"), s("s2_size", "M"), b("key", false)]);
    let (salt, sugar) = (grey_count(&svg, "#a3a3a3"), grey_count(&svg, "#1e1e1e"));
    assert!(salt >= 4 * sugar && sugar >= 1, "salt {salt}, sugar {sugar}");
    // a dilute figure in mM has fewer particles than the same numbers in M
    let m = render_of(&[s("show", "Start"), s("unit", "M"), f("s1_in", 0.154), f("s1_out", 0.154), b("key", false)]);
    let mm = render_of(&[s("show", "Start"), s("unit", "mM"), f("s1_in", 0.154), f("s1_out", 0.154), b("key", false)]);
    assert!(grey_count(&m, "#a3a3a3") > 3 * grey_count(&mm, "#a3a3a3"), "M and mM must differ");
}

type Rc = (f64, f64, f64, f64);
fn overlap(a: Rc, b: Rc) -> bool { !(a.2 <= b.0 || b.2 <= a.0 || a.3 <= b.1 || b.3 <= a.1) }

/// (y, left, right, text) of every `<text>` as finally drawn.
fn text_boxes(svg: &str) -> Vec<(f64, f64, f64, String)> {
    let mut out = Vec::new();
    for t in svg.split("<text ").skip(1) {
        let num = |k: &str| t.split(&format!("{k}=\"")).nth(1).and_then(|v| v.split('"').next()).and_then(|v| v.parse::<f64>().ok());
        let (x, y, fs) = (num("x").unwrap(), num("y").unwrap(), num("font-size").unwrap());
        let anchor = t.split("text-anchor=\"").nth(1).unwrap().split('"').next().unwrap();
        let inner = t.split("</text>").next().unwrap().split('>').skip(1).collect::<Vec<_>>().join(">");
        let body: String = inner.split('<').map(|p| p.split('>').last().unwrap_or("")).collect();
        let width = text_w(&body, fs);
        let (l, r) = match anchor { "middle" => (x - width / 2.0, x + width / 2.0), "end" => (x - width, x), _ => (x, x + width) };
        out.push((y, l, r, body));
    }
    out
}

fn assert_inside_page(svg: &str, what: &str) {
    let w: f64 = svg.split("width=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
    for (y, l, r, body) in text_boxes(svg) {
        assert!(r <= w + 1.0 && l >= -1.0 && y >= 0.0, "{what}: text {body:?} runs to {l:.0}..{r:.0} (page {w}) at y {y:.0}");
    }
}

#[test]
fn a_shrunken_cell_and_a_plasmolyzed_plant_keep_every_crossing_arrow_on_the_membrane() {
    let pal = palette(&with(&[]));
    for scene in ["Red blood cell", "Plant cell"] {
        for size in ["Medium", "Large"] {
            for blank in [false, true] {
                let mode = if blank { "Blank boxes" } else { "Arrows" };
                let p = with(&[s("scene", scene), s("label_size", size), s("arr_solute", mode), s("arr_water", mode), s("show", "Result"),
                    b("s1_on", true), b("s1_cross", true), f("s1_in", 3.0), f("s1_out", 0.0), s("s1_name", "Urea"),
                    b("s2_on", true), b("s2_cross", true), f("s2_in", 0.0), f("s2_out", 2.0), s("s2_name", "Glucose"),
                    b("s3_on", true), b("s3_cross", true), f("s3_in", 2.0), f("s3_out", 0.0), s("s3_name", "Ethanol")]);
                let c = cfg(&p);
                let mut m = model(&p, None);
                m.tonic = Tonic::Hyper;
                m.vrel = 0.5;
                let g = geo(&m, Kind::Result, &pal);
                let pl = plan(&c, &m, &g, m.flow());
                let what = format!("{scene} {size} blank={blank}");
                assert_eq!(pl.sol.len(), 3, "{what}");
                let (cx, cy, rx, ry, _) = g.shape;
                // what the leaders and the water arrows occupy
                let mut busy: Vec<Rc> = pl.water.iter().map(|(x, y, _)| (x - 25.0, y - 6.0, x + 25.0, y + 6.0)).collect();
                busy.extend(pl.wbox.iter().copied());
                if scene == "Red blood cell" { busy.push((0.0, cy - 1.0, g.edges.0, cy + 1.0)); }
                if let Some(t) = &pl.tag { busy.push(t.rect()); }
                for (i, a) in pl.sol.iter().enumerate() {
                    let crossing = if scene == "Red blood cell" {
                        let d = |q: (f64, f64)| ((q.0 - cx).powi(2) + (q.1 - cy).powi(2)).sqrt();
                        d(a.a).min(d(a.b)) < rx && d(a.a).max(d(a.b)) > rx
                    } else {
                        // the bottom of the membrane as it is now, well inside the wall
                        a.a.1.min(a.b.1) < cy + ry && a.a.1.max(a.b.1) > cy + ry && cy + ry < g.wall.1 - 12.0 && (a.a.0 - cx).abs() < rx
                    };
                    assert!(crossing, "{what}: arrow {i} is not across the membrane");
                    let lab = a.lab.rect();
                    assert!(lab.0 >= 0.0 && lab.2 <= PW && lab.3 <= g.ph, "{what}: label {i} off the panel {lab:?}");
                    if scene == "Plant cell" { assert!(lab.1 >= g.wall.1, "{what}: label {i} is not below the wall"); }
                    for q in &busy {
                        assert!(!overlap(a.boxr, *q), "{what}: arrow {i} box {:?} runs over {q:?}", a.boxr);
                        assert!(!overlap(lab, *q), "{what}: label {i} {lab:?} runs over {q:?}");
                    }
                    for (j, o) in pl.sol.iter().enumerate().filter(|(j, _)| *j != i) {
                        assert!(!overlap(a.boxr, o.boxr), "{what}: boxes {i} and {j} overlap");
                        assert!(!overlap(lab, o.lab.rect()) && !overlap(lab, o.boxr), "{what}: label {i} meets arrow or label {j}");
                    }
                }
            }
        }
    }
}

#[test]
fn a_plasmolyzed_plants_water_arrows_cross_the_membrane_not_the_wall() {
    let pal = palette(&with(&[]));
    let m = mdl(&[s("scene", "Plant cell"), f("s1_in", 0.9), f("s1_out", 4.0)]);
    assert_eq!(m.tonic, Tonic::Hyper);
    let g = geo(&m, Kind::Result, &pal);
    let c = cfg(&with(&[]));
    let pl = plan(&c, &m, &g, m.flow());
    let (cx, _, mw, _, _) = g.shape;
    assert!(mw < 45.0, "plasmolyzed membrane half-width {mw}");
    for (x, _, _) in &pl.water { assert!(((x - cx).abs() - mw).abs() < 0.01, "water arrow at {x} is not at the membrane ({mw} from {cx})"); }
    // and the tag is clear of the wall
    assert!(pl.tag.unwrap().rect().0 > g.wall.0);
}

#[test]
fn a_long_membrane_label_wraps_and_stays_on_the_page() {
    let long = "Plasma membrane: a phospholipid bilayer that is selectively permeable to water and small molecules";
    for scene in ["Red blood cell", "Plant cell", "Split beaker", "U-tube", "Dialysis bag"] {
        for size in ["Medium", "Large"] {
            for mode in ["Text", "Blank line"] {
                for ans in [false, true] {
                    let svg = render_of(&[s("scene", scene), s("label_size", size), s("show", "Both"), s("lab_membrane", mode), s("lab_membrane_text", long), b(SHOW_ANSWERS, ans)]);
                    assert_inside_page(&svg, &format!("{scene} {size} {mode} answers={ans}"));
                    let words = flat(&svg);
                    if mode == "Text" || ans { assert!(["Plasma", "phospholipid", "molecules"].iter().all(|w| words.contains(w)), "{scene} {size} {mode}: wording missing"); }
                    if !ans && mode == "Blank line" { assert!(!words.contains("phospholipid"), "{scene}: the blank line gave the answer away"); }
                }
            }
        }
    }
}

#[test]
fn in_panel_labels_are_measured_at_the_large_size() {
    let rect = |lf: f64| Lab { x: 100.0, y: 50.0, size: 9.5, anchor: "start", text: "Glucose".into(), lf }.rect();
    let (a, b2) = (rect(1.0), rect(1.4));
    assert!((b2.2 - b2.0) > 1.35 * (a.2 - a.0) - 3.0 && (b2.3 - b2.1) > 1.3 * (a.3 - a.1) - 3.0, "{a:?} {b2:?}");
    // a name too long for its room shrinks until it fits as drawn
    let lab = Lab { x: 100.0, y: 50.0, size: 9.5, anchor: "start", text: "Polyvinylpyrrolidone".into(), lf: 1.4 }.fit(60.0);
    let r = lab.rect();
    assert!(r.2 - r.0 <= 60.5 && lab.size < 9.5, "{r:?} {}", lab.size);
}

#[test]
fn long_solute_names_do_not_make_columns_or_labels_collide() {
    let name = "Polyvinylpyrrolidone solution";
    for scene in ["Red blood cell", "Plant cell", "Split beaker", "Dialysis bag"] {
        for size in ["Medium", "Large"] {
            for show in ["Both", "Three-up"] {
                let svg = render_of(&[s("scene", scene), s("label_size", size), s("show", show), s("s1_name", name), b("s2_on", true), s("s2_name", "Sodium bicarbonate"), b("s2_cross", true), f("s2_in", 2.0), f("s2_out", 0.0), b("answer_key", false)]);
                let what = format!("{scene} {size} {show}");
                assert_inside_page(&svg, &what);
                // texts on one row never overlap
                let mut boxes = text_boxes(&svg);
                boxes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap()));
                for w in boxes.windows(2) {
                    if (w[0].0 - w[1].0).abs() < 0.5 { assert!(w[0].2 <= w[1].1 + 0.5, "{what}: {:?} runs into {:?}", w[0], w[1]); }
                }
            }
        }
    }
}

#[test]
fn the_utube_leader_ends_on_the_membrane_and_blank_water_boxes_keep_clear() {
    let pal = palette(&with(&[]));
    let m = mdl(&[s("scene", "U-tube")]);
    let g = geo(&m, Kind::Start, &pal);
    // the dashed membrane starts at y = 148; the leader stops there
    assert!((g.t_mem.1 + 2.0 - 148.0).abs() < 0.5, "{}", g.t_mem.1);
    for scene in ["Red blood cell", "Plant cell", "Split beaker", "U-tube", "Dialysis bag"] {
        let p = with(&[s("scene", scene), s("arr_water", "Blank boxes"), s("arr_solute", "Blank boxes"), s("show", "Start"),
            b("s1_cross", true), b("s2_on", true), b("s2_cross", true), b("s3_on", true), b("s3_cross", true)]);
        let c = cfg(&p);
        let m = model(&p, None);
        let g = geo(&m, Kind::Start, &pal);
        let pl = plan(&c, &m, &g, m.flow());
        let tag = pl.tag.as_ref().unwrap_or_else(|| panic!("{scene}: blank water boxes have no H2O tag")).rect();
        assert!(!pl.wbox.is_empty(), "{scene}");
        for wb in &pl.wbox {
            assert!(!overlap(tag, *wb), "{scene}: the H2O tag sits on a blank box");
            for sa in &pl.sol { assert!(!overlap(*wb, sa.boxr), "{scene}: water box {wb:?} touches solute box {:?}", sa.boxr); assert!(!overlap(tag, sa.boxr), "{scene}: tag on a solute box"); }
        }
    }
}

fn spike_ratio(svg: &str) -> (f64, usize) {
    let poly = svg.split("<polygon points=\"").nth(1).unwrap().split('"').next().unwrap();
    let pts: Vec<(f64, f64)> = poly.split(' ').map(|p| { let mut it = p.split(','); (it.next().unwrap().parse().unwrap(), it.next().unwrap().parse().unwrap()) }).collect();
    let d: Vec<f64> = pts.iter().map(|(x, y)| ((x - 120.0).powi(2) + (y - 98.0).powi(2)).sqrt()).collect();
    (d.iter().cloned().fold(f64::MAX, f64::min) / d.iter().cloned().fold(0.0, f64::max), pts.len())
}

#[test]
fn crenation_deepens_with_hypertonicity() {
    let at = |out: f64| spike_ratio(&render_of(&[s("scene", "Red blood cell"), s("show", "Result"), f("s1_in", 0.9), f("s1_out", out), b("key", false)]));
    let (mild, strong) = (at(1.2), at(3.6));
    assert!(mild.0 > strong.0 + 0.1, "mild {mild:?} strong {strong:?}");
    assert!(strong.1 > mild.1, "more spikes when more hypertonic: {mild:?} {strong:?}");
}

#[test]
fn doubling_a_concentration_at_the_largest_particle_size_shows() {
    let svg = render_of(&[s("scene", "Split beaker"), s("show", "Start"), s("s1_size", "XL"), f("s1_in", 0.9), f("s1_out", 1.8), b("key", false)]);
    let (l, r) = side_counts(&svg, "#a3a3a3");
    assert!(r as f64 >= 1.5 * l as f64 && l >= 1, "XL: {l} vs {r}");
}
