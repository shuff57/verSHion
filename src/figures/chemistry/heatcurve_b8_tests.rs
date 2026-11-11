//! Tests for the computed segment lengths: the heats checked by hand, the drawn lengths in the ratio of the heats, cooling
//! the same heats backwards, refusals, the ends of every new number, the controls shown only where they act, the drags
//! through the real hit test and the labels on crowded computed curves.
use super::*;
use crate::figures::{registry, start_params, FigureDef, Params, Value};

const ID: &str = "heating-cooling-curve";

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
fn with(list: &[(&str, Value)]) -> Params {
    let d = def();
    let mut p = start_params(&d);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn f(k: &str, v: f64) -> (&str, Value) { (k, Value::F(v)) }
fn s<'a>(k: &'a str, v: &str) -> (&'a str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &str, v: bool) -> (&str, Value) { (k, Value::B(v)) }
fn comp() -> (&'static str, Value) { s("scale", COMPUTED) }
fn render(p: &Params) -> String { def().svg(p) }

/// The curve as drawn: the points of the polyline in the curve's colour.
fn curve(svg: &str, p: &Params) -> Vec<(f64, f64)> {
    let cc = p.s("curve_color");
    for part in svg.split("<polyline points=\"").skip(1) {
        let (pts, rest) = part.split_once('"').unwrap();
        if !rest.contains(&format!("stroke=\"{cc}\"")) || rest.find("/>").unwrap() < rest.find("stroke=").unwrap() { continue; }
        return pts.split_whitespace().map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect();
    }
    Vec::new()
}

/// Water's heats, checked by hand for 100 g from -20 °C to 120 °C.
/// q = m·c·ΔT: ice 100 × 2.09 × 20 = 4180 J; water 100 × 4.18 × 100 = 41 800 J; steam 100 × 2.01 × 20 = 4020 J.
/// q = n·ΔH with n = 100 ÷ 18.02 = 5.549 mol: melting 5.549 × 6.01 = 33.35 kJ; boiling 5.549 × 40.7 = 225.9 kJ.
const WATER_Q: [f64; 5] = [4.18, 33.352, 41.8, 225.86, 4.02];

#[test]
fn waters_heats_by_hand() {
    let h = table("Water").unwrap();
    assert_eq!((h.c_solid, h.c_liquid, h.dh_fus, h.dh_vap), (2.09, 4.18, 6.01, 40.7));
    assert!((h.c_gas - 2.0).abs() < 0.05, "steam about 2.0 J/g·K");
    let p = with(&[comp(), f("mass", 100.0), f("start", -20.0), f("end", 120.0)]);
    let segs = plan(&p).unwrap();
    let phs: Vec<Ph> = segs.iter().map(|s| s.ph).collect();
    assert_eq!(phs, vec![Ph::Solid, Ph::Melt, Ph::Liquid, Ph::Boil, Ph::Gas]);
    for (s, want) in segs.iter().zip(WATER_Q) { assert!((s.q - want).abs() < 0.01, "{:?}: {} kJ, by hand {want}", s.ph, s.q); }
    // the boiling plateau is almost 7 times the melting one (40.7 / 6.01 = 6.77), as the reference says
    assert!((segs[3].q / segs[1].q - 40.7 / 6.01).abs() < 1e-9);
}

#[test]
fn drawn_lengths_are_in_the_ratio_of_the_heats() {
    for xa in ["Time", "Heat added"] {
        for mode in ["Heating", "Cooling"] {
            let (a, z) = if mode == "Heating" { (-20.0, 120.0) } else { (120.0, -20.0) };
            let p = with(&[comp(), s("xaxis", xa), s("mode", mode), f("start", a), f("end", z)]);
            let svg = render(&p);
            let pts = curve(&svg, &p);
            assert_eq!(pts.len(), 6, "{xa} {mode}: five parts drawn");
            let lens: Vec<f64> = pts.windows(2).map(|w| w[1].0 - w[0].0).collect();
            let want: Vec<f64> = if mode == "Heating" { WATER_Q.to_vec() } else { WATER_Q.iter().rev().cloned().collect() };
            let total: f64 = lens.iter().sum();
            let wt: f64 = want.iter().sum();
            for (l, q) in lens.iter().zip(&want) {
                // 0.01 px rounding at each end
                assert!((l / total - q / wt).abs() < 0.02 / total + 1e-6, "{xa} {mode}: drawn {l} of {total} px against {q} of {wt} kJ");
            }
            // the plateaus are flat at 0 and 100 °C, the slopes go the right way
            assert!((pts[1].1 - pts[2].1).abs() < 0.01 && (pts[3].1 - pts[4].1).abs() < 0.01);
            if mode == "Heating" { assert!(pts[5].1 < pts[0].1) } else { assert!(pts[5].1 > pts[0].1) }
            // the x-axis reads in the drawn unit: the whole curve ends at the total heat (kJ) or the total ÷ 20 kJ/min
            let hf = super::super::curves::heat_frame(&p);
            let end = hf.g.vx(pts[5].0);
            let want_end = if xa == "Time" { wt / 20.0 } else { wt };
            assert!((end - want_end).abs() < want_end * 0.002, "{xa}: ends at {end}, by hand {want_end}");
        }
    }
}

#[test]
fn the_note_states_what_is_drawn() {
    let p = with(&[comp()]);
    let n = scale_note(&p).unwrap();
    for t in ["Solid warms 4.18 kJ, 0.209 min", "Melting 33.4 kJ, 1.67 min", "Liquid warms 41.8 kJ, 2.09 min", "Boiling 226 kJ, 11.3 min", "Gas warms 4.02 kJ, 0.201 min"] {
        assert!(n.contains(t), "{t} in {n}");
    }
    // read back off the drawing: each part's minutes from its pixel length and the axis
    let svg = render(&p);
    let pts = curve(&svg, &p);
    let hf = super::super::curves::heat_frame(&p);
    let r = reading(&p).unwrap();
    for (i, w) in pts.windows(2).enumerate() {
        let drawn = hf.g.vx(w[1].0) - hf.g.vx(w[0].0);
        assert_eq!(sig(drawn), sig(r.rows[i].2), "part {i}: the note's minutes are the drawn ones");
    }
    let c = with(&[comp(), s("mode", "Cooling"), f("start", 120.0), f("end", -20.0), s("xaxis", "Heat added")]);
    let n = scale_note(&c).unwrap();
    assert!(n.contains("Gas cools 4.02 kJ · Condensing 226 kJ · Liquid cools 41.8 kJ · Freezing 33.4 kJ · Solid cools 4.18 kJ"), "{n}");
    assert!(!n.contains("min"), "no minutes along a heat axis: {n}");
    assert_eq!(crate::figures::label_for(ID, "rate", &c), Some("Cooling rate (kJ/min)"));
    assert!(render(&c).contains("Heat removed (kJ)"));
}

#[test]
fn every_built_in_substance_draws_its_own_lengths() {
    let mut seen = Vec::new();
    for (sub, a, z) in [("Water", -20.0, 120.0), ("Ethanol", -140.0, 100.0), ("Acetone", -120.0, 80.0), ("Mercury", -60.0, 400.0), ("Sodium chloride", 700.0, 1500.0), ("Substance X", 0.0, 110.0)] {
        let p = with(&[comp(), s("substance", sub), f("start", a), f("end", z)]);
        let segs = plan(&p).unwrap_or_else(|e| panic!("{sub}: {e}"));
        assert_eq!(segs.len(), 5, "{sub}");
        let h = table(sub).unwrap();
        let n = 100.0 / h.molar;
        assert!((segs[1].q - n * h.dh_fus).abs() < 1e-9 && (segs[3].q - n * h.dh_vap).abs() < 1e-9, "{sub}");
        // vaporizing takes more than melting for every one of them
        assert!(segs[3].q > segs[1].q, "{sub}");
        let svg = render(&p);
        assert_eq!(curve(&svg, &p).len(), 6, "{sub}");
        assert!(!seen.contains(&svg));
        seen.push(svg);
    }
}

#[test]
fn custom_heats_mass_and_rate_each_change_the_curve() {
    let base = vec![comp(), s("substance", "Custom")];
    let p0 = with(&base);
    // Custom opens on water's heats and points, so it draws water's lengths
    let w = with(&[comp()]);
    assert_eq!(curve(&render(&p0), &p0), curve(&render(&w), &w));
    let r0 = render(&p0);
    for (k, v) in [("mass", 50.0), ("rate", 5.0), ("molar_mass", 40.0), ("c_solid", 1.0), ("c_liquid", 2.0), ("c_gas", 4.0), ("dh_fus", 20.0), ("dh_vap", 10.0)] {
        let mut l = base.clone();
        l.push(f(k, v));
        let p = with(&l);
        let r = render(&p);
        assert_ne!(r, r0, "{k} changes the drawing");
        assert!(relevant(k, &p), "{k} is shown in the computed mode for Custom");
    }
    // a heat axis does not depend on the rate (and hides it)
    let h0 = with(&[comp(), s("xaxis", "Heat added")]);
    let h1 = with(&[comp(), s("xaxis", "Heat added"), f("rate", 3.0)]);
    assert_eq!(render(&h0), render(&h1));
    assert!(!relevant("rate", &h0));
}

#[test]
fn the_old_modes_are_untouched_and_the_new_one_differs() {
    let d = def();
    let open = start_params(&d);
    assert_eq!(open.s("scale"), "To scale");
    let sch = with(&[s("scale", "Schematic")]);
    let c = with(&[comp()]);
    let (r1, r2, r3) = (render(&open), render(&sch), render(&c));
    assert!(r1 != r2 && r2 != r3 && r1 != r3);
    // the new params do nothing outside the computed mode
    for mode in [open.clone(), sch.clone()] {
        let mut q = mode.clone();
        for (k, v) in [("mass", 5.0), ("rate", 1.0), ("c_solid", 9.0), ("dh_vap", 1.0), ("molar_mass", 3.0)] { q.set_f(k, v); }
        q.set_s("substance", "Custom");
        let mut m = mode.clone();
        m.set_s("substance", "Custom");
        assert_eq!(render(&q), render(&m));
        for k in ["mass", "rate", "molar_mass", "c_solid", "c_liquid", "c_gas", "dh_fus", "dh_vap"] { assert!(!crate::figures::param_relevant(ID, k, &q), "{k} hidden outside the computed mode"); }
    }
    // the typed minutes are shown only for "To scale"
    assert!(crate::figures::param_relevant(ID, "t_boil", &open));
    assert!(!crate::figures::param_relevant(ID, "t_boil", &c) && !crate::figures::param_relevant(ID, "t_boil", &sch));
    // the custom heats only for Custom
    assert!(!crate::figures::param_relevant(ID, "c_solid", &c) && crate::figures::param_relevant(ID, "mass", &c));
}

#[test]
fn other_figures_keep_their_mass_and_rate_controls() {
    for d in registry() {
        if d.id == ID { continue; }
        let p = start_params(&d);
        for pd in (d.params)() {
            if ["mass", "rate", "c_solid", "c_liquid", "c_gas", "dh_fus", "dh_vap", "molar_mass", "t_boil"].contains(&pd.key) {
                // the hook is matched on this figure's id only: what another figure showed before it still shows
                let mut q = p.clone();
                q.set_s("scale", "Schematic");
                assert_eq!(crate::figures::param_relevant(d.id, pd.key, &p), crate::figures::param_relevant(d.id, pd.key, &q), "{} {}", d.id, pd.key);
            }
        }
    }
}

#[test]
fn refused_inputs_say_why_in_the_figure_and_the_panel() {
    let cases: Vec<(Vec<(&str, Value)>, &str)> = vec![
        (vec![f("mass", 0.0)], "mass must be more than 0"),
        (vec![f("rate", 0.0)], "heating rate must be more than 0"),
        (vec![s("mode", "Cooling"), f("start", 120.0), f("end", -20.0), f("rate", 0.0)], "cooling rate must be more than 0"),
        (vec![s("substance", "Custom"), f("c_solid", 0.0)], "solid's specific heat"),
        (vec![s("substance", "Custom"), f("c_liquid", 0.0)], "liquid's specific heat"),
        (vec![s("substance", "Custom"), f("c_gas", 0.0)], "gas's specific heat"),
        (vec![s("substance", "Custom"), f("dh_fus", 0.0)], "enthalpy of fusion"),
        (vec![s("substance", "Custom"), f("dh_vap", 0.0)], "enthalpy of vaporization"),
        (vec![s("substance", "Custom"), f("molar_mass", 0.0)], "molar mass must be more than 0"),
        (vec![f("start", 120.0), f("end", -20.0)], "heating curve starts below"),
        (vec![s("mode", "Cooling")], "cooling curve starts above"),
        (vec![f("start", 30.0), f("end", 30.0)], "are the same"),
        (vec![s("substance", "Custom"), f("mp", 80.0), f("bp", 20.0)], "melting point (80 °C) must be below the boiling point (20 °C)"),
        (vec![s("substance", "Custom"), f("mp", 50.0), f("bp", 50.0)], "must be below the boiling point"),
        (vec![f("mass", 0.001), f("rate", 10000.0)], "too short to number"),
        (vec![f("mass", 10000.0), f("rate", 0.001)], "too long to number"),
    ];
    for (mut l, why) in cases {
        l.push(comp());
        let p = with(&l);
        let w = warnings(&p);
        assert!(w.iter().any(|x| x.starts_with("Not drawn") && x.contains(why)), "{why}: {w:?}");
        let svg = render(&p);
        assert!(curve(&svg, &p).is_empty(), "{why}: no curve drawn");
        assert!(svg.contains("Not drawn to scale:"), "{why}: said in the figure");
        // the words in the figure are the panel's, wrapped
        let words: String = svg.split("paint-order=\"stroke\">").skip(1).map(|t| t.split('<').next().unwrap()).collect::<Vec<_>>().join(" ");
        let first = why.split_whitespace().next().unwrap();
        assert!(words.contains(first), "{why}: {words}");
        assert!(def().handles(&p, (750.0, 480.0)).is_empty(), "{why}: nothing to drag on a refused curve");
    }
    // the same inputs in the old modes draw as before (they are not refused there)
    let old = with(&[s("scale", "Schematic"), f("start", 120.0), f("end", -20.0)]);
    assert!(warnings(&old).is_empty() && !curve(&render(&old), &old).is_empty());
}

#[test]
fn a_part_too_short_to_see_is_noted() {
    // 1 degree of ice against the whole curve to 200 °C: 0.209 kJ of about 313 kJ, a fraction of a pixel
    let p = with(&[comp(), f("start", -1.0), f("end", 200.0), s("xaxis", "Heat added")]);
    let w = warnings(&p);
    assert!(w.iter().any(|x| x.starts_with("Solid warms is 0.209 kJ") && x.contains("hard to see")), "{w:?}");
    // still drawn (to scale)
    assert_eq!(curve(&render(&p), &p).len(), 6);
    // 2 degrees of ice and of steam at Large: no clear place for the solid's label, so it is left off and the panel says so
    let q = with(&[comp(), f("start", -2.0), f("end", 102.0), s("label_size", "Large"), s("plateau_labels", "Both states (Solid + liquid)")]);
    let w = warnings(&q);
    assert!(w.iter().any(|x| x == "Solid warms is too short at this scale to label clear of the curve: its label is left off."), "{w:?}");
    assert!(!render(&q).contains(">Solid<"));
    // the old typed mode never leaves a label off
    let o = with(&[f("start", -2.0), f("end", 102.0), s("label_size", "Large"), s("plateau_labels", "Both states (Solid + liquid)")]);
    assert!(render(&o).contains(">Solid<") && warnings(&o).is_empty());
    // nothing to note on the opening water curve
    assert!(warnings(&with(&[comp()])).is_empty());
    // a typed axis that stops short of the curve says so
    let t = with(&[comp(), b("xaxis_fit", false), f("xaxis_to", 5.0)]);
    assert!(warnings(&t).iter().any(|x| x.contains("past the x-axis's end at 5")), "{:?}", warnings(&t));
}

/// Every number printed in the SVG's text is clean (no float noise like 0.30000000000000004 or 1e-7).
fn clean_numbers(svg: &str) -> bool {
    svg.split('>').filter_map(|t| t.split('<').next()).all(|t| !t.contains("e-") && !t.contains("000000") && !t.contains("NaN") && !t.contains("inf"))
}

#[test]
fn the_ends_of_every_new_number() {
    let d = def();
    let defs = (d.params)();
    let keys = ["mass", "rate", "molar_mass", "c_solid", "c_liquid", "c_gas", "dh_fus", "dh_vap"];
    let ends = |k: &str| { let pd = defs.iter().find(|x| x.key == k).unwrap(); match pd.kind { crate::figures::Kind::Float { min, max, .. } => (min, max), _ => unreachable!() } };
    // clamped where typed past the ends
    let p = with(&[comp(), f("mass", 1e30), f("rate", -5.0), f("dh_vap", f64::NAN)]);
    assert_eq!((p.f("mass"), p.f("rate"), p.f("dh_vap")), (ends("mass").1, 0.0, 0.0));
    for k in keys {
        let (lo, hi) = ends(k);
        for v in [lo, hi] {
            for sub in ["Water", "Custom"] {
                for xa in ["Time", "Heat added"] {
                    for ls in ["Small", "Large"] {
                        let p = with(&[comp(), s("substance", sub), s("xaxis", xa), f(k, v), s("label_size", ls), b("letters", true), f("start", -300.0), f("end", 5000.0)]);
                        let svg = render(&p);
                        assert!(svg.len() < 200_000, "{k}={v}: {} bytes", svg.len());
                        assert!(clean_numbers(&svg), "{k}={v} {sub} {xa}");
                        let pts = curve(&svg, &p);
                        assert!(pts.is_empty() || pts.len() <= 8, "{k}={v}");
                        assert!(pts.is_empty() == plan(&p).is_err(), "{k}={v} {sub} {xa}: drawn exactly when planned");
                        let ticks = svg.matches("<text").count();
                        assert!(ticks < 200, "{k}={v}: {ticks} texts");
                    }
                }
            }
        }
    }
    // the biggest sample at the slowest rate and the smallest at the fastest are refused, never drawn huge
    let big = with(&[comp(), f("mass", 10000.0), f("rate", 0.5), f("start", -273.0), f("end", 4000.0)]);
    let small = with(&[comp(), f("mass", 1.0), f("rate", 10000.0)]);
    assert!(plan(&big).is_err() || curve(&render(&big), &big).len() == 6);
    assert!(plan(&small).is_err());
}

#[test]
fn forty_odd_inputs_never_draw_wrong() {
    let mut seed: u64 = 0x4ea7_b8;
    let mut next = |n: u64| { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (seed >> 33) % n };
    let subs = ["Water", "Ethanol", "Acetone", "Mercury", "Sodium chloride", "Substance X", "Custom"];
    let vals = [-5.0, 0.0, 1e-9, 0.01, 0.5, 1.0, 18.0, 100.0, 999.0, 1e9];
    for i in 0..400 {
        let mut l = vec![comp(), s("substance", subs[next(7) as usize]), s("mode", ["Heating", "Cooling"][next(2) as usize]), s("xaxis", ["Time", "Heat added"][next(2) as usize])];
        for k in ["mass", "rate", "molar_mass", "c_solid", "c_liquid", "c_gas", "dh_fus", "dh_vap"] { if next(2) == 0 { l.push(f(k, vals[next(10) as usize])); } }
        for k in ["start", "end", "mp", "bp"] { if next(2) == 0 { l.push(f(k, [-300.0, -40.0, 0.0, 20.0, 100.0, 101.0, 1500.0, 5000.0][next(8) as usize])); } }
        if next(3) == 0 { l.push(b("supercool", true)); }
        let p = with(&l);
        let svg = render(&p);
        assert!(clean_numbers(&svg), "case {i}: {:?}", svg.split('>').filter_map(|t| t.split('<').next()).filter(|t| t.contains("e-") || t.contains("000000") || t.contains("NaN") || t.contains("inf")).collect::<Vec<_>>());
        match plan(&p) {
            Err(_) => assert!(curve(&svg, &p).is_empty() && svg.contains("Not drawn to scale:"), "case {i}: refused, not drawn"),
            Ok(segs) => {
                let pts = curve(&svg, &p);
                assert!(!pts.is_empty(), "case {i}: drawn");
                // heats are positive, plateaus at the substance's points, the parts in temperature order
                assert!(segs.iter().all(|s| s.q > 0.0 && s.q.is_finite()), "case {i}: {segs:?}");
                assert!(segs.windows(2).all(|w| w[0].t1 == w[1].t0), "case {i}: {segs:?}");
                let cooling = p.s("mode") == "Cooling";
                let (a, z) = (p.f("start"), p.f("end"));
                assert!(if cooling { a > z } else { a < z }, "case {i}");
                // left to right: the x of every corner never goes back
                assert!(pts.windows(2).all(|w| w[1].0 >= w[0].0 - 0.01), "case {i}");
            }
        }
    }
}

#[test]
fn dragging_a_plateau_through_the_hit_test_keeps_the_heats() {
    use crate::figures::interact::pick;
    let d = def();
    for sub in ["Water", "Ethanol", "Substance X"] {
        let (a, z) = match sub { "Ethanol" => (-140.0, 100.0), "Substance X" => (0.0, 110.0), _ => (-20.0, 120.0) };
        let p = with(&[comp(), s("substance", sub), f("start", a), f("end", z), s("xaxis", "Heat added")]);
        let hs = d.handles(&p, (750.0, 480.0));
        let i = hs.iter().position(|h| h.tip.starts_with("Boiling point")).unwrap();
        let (hx, hy) = hs[i].pos;
        // a sweep of pixels round the handle finds it
        let mut hits = 0;
        for dx in -3..=3 { for dy in -3..=3 { if pick(&hs, (hx + dx as f64, hy + dy as f64), 1.0) == Some(i) { hits += 1; } } }
        assert!(hits >= 25, "{sub}: {hits}");
        let k = pick(&hs, (hx, hy), 1.0).unwrap();
        let mut q = p.clone();
        let before = plan(&p).unwrap();
        (hs[k].drag.clone().unwrap())(&mut q, (hx, hy - 30.0));
        assert_eq!(q.s("substance"), "Custom");
        let after = plan(&q).unwrap();
        let h0 = table(sub).unwrap();
        assert_eq!(heats(&q), h0, "{sub}: the heats went with it");
        // the plateaus' heats are unchanged; only the boiling point moved
        assert!((before[1].q - after[1].q).abs() < 1e-9 && (before[3].q - after[3].q).abs() < 1e-9, "{sub}");
        assert!(q.f("bp") > super::super::curves::heat_points(&p).1, "{sub}");
    }
    // the end point still drags the ending temperature in the computed mode
    let p = with(&[comp()]);
    let hs = d.handles(&p, (750.0, 480.0));
    let i = hs.iter().position(|h| h.tip.starts_with("End point")).unwrap();
    let k = pick(&hs, hs[i].pos, 1.0).unwrap();
    let mut q = p.clone();
    (hs[k].drag.clone().unwrap())(&mut q, (hs[i].pos.0, hs[i].pos.1 + 20.0));
    assert!(q.f("end") < 120.0 && q.f("t_gas") == p.f("t_gas"));
}

/// The text boxes in an SVG (bold 15.4 px halo labels) as drawn, from their x, baseline and length.
fn label_boxes(svg: &str) -> Vec<([f64; 4], String)> {
    let mut v = Vec::new();
    for part in svg.split("<text ").skip(1) {
        let attr = |n: &str| part.split(&format!("{n}=\"")).nth(1).and_then(|r| r.split('"').next()).and_then(|x| x.parse::<f64>().ok());
        let (Some(x), Some(y), Some(sz)) = (attr("x"), attr("y"), attr("font-size")) else { continue };
        if !part.contains("stroke=\"#fff\"") { continue; }
        let t = part.split('>').nth(1).unwrap_or("").split('<').next().unwrap_or("").to_string();
        let w = t.chars().count() as f64 * sz * 0.55;
        let anchor = part.split("text-anchor=\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("start");
        let x0 = match anchor { "middle" => x - w / 2.0, "end" => x - w, _ => x };
        v.push(([x0, y - sz * 0.75, x0 + w, y + sz * 0.2], t));
    }
    v
}

/// The label rules a checker applies, read off the drawn SVG: every label inside the page, none on the curve (the line
/// itself, 1 px in from the text box), none on another.
fn check_labels(what: &str, p: &Params) {
    let svg = render(p);
    let (w, h) = crate::figures::svg_size(&svg);
    let bx = label_boxes(&svg);
    for (r, t) in &bx {
        assert!(r[0] >= -0.5 && r[2] <= w + 0.5 && r[1] >= -0.5 && r[3] <= h + 0.5, "{what}: {t} off the page {r:?} in {w}x{h}");
    }
    let pts = curve(&svg, p);
    for (r, t) in &bx {
        let r = [r[0] + 1.0, r[1] + 1.0, r[2] - 1.0, r[3] - 1.0];
        for seg in pts.windows(2) {
            let hit = (0..=100).any(|k| { let u = k as f64 / 100.0; let (x, y) = (seg[0].0 + (seg[1].0 - seg[0].0) * u, seg[0].1 + (seg[1].1 - seg[0].1) * u); x > r[0] && x < r[2] && y > r[1] && y < r[3] });
            assert!(!hit, "{what}: {t} on the curve {r:?} {seg:?}");
        }
    }
    for i in 0..bx.len() {
        for j in i + 1..bx.len() {
            let (p1, p2) = (&bx[i].0, &bx[j].0);
            let ov = p1[0] < p2[2] - 0.5 && p2[0] < p1[2] - 0.5 && p1[1] < p2[3] - 0.5 && p2[1] < p1[3] - 0.5;
            assert!(!ov, "{what}: {} on {} {:?} {:?}", bx[i].1, bx[j].1, p1, p2);
        }
    }
}

#[test]
fn crowded_computed_curves_keep_their_labels_apart_and_on_the_page() {
    // the cases where computed lengths crowd the labels: tiny solid and gas parts, short plateaus, Large type, letters
    let mut n = 0;
    for (sub, a, z) in [("Water", -20.0, 120.0), ("Water", -2.0, 102.0), ("Mercury", -60.0, 400.0), ("Sodium chloride", 700.0, 1500.0), ("Ethanol", -140.0, 100.0)] {
        for mode in ["Heating", "Cooling"] {
            for ls in ["Medium", "Large"] {
                for pl in ["Both states (Solid + liquid)", "The change (Melting)"] {
                    let (a, z) = if mode == "Heating" { (a, z) } else { (z, a) };
                    let p = with(&[comp(), s("substance", sub), s("mode", mode), f("start", a), f("end", z), s("label_size", ls), s("plateau_labels", pl), b("letters", true)]);
                    check_labels(&format!("{sub} {mode} {ls} {pl}"), &p);
                    n += 1;
                }
            }
        }
    }
    assert_eq!(n, 40);
    // a seeded sweep over every label choice, both axes, the sizes and the substances
    let mut seed: u64 = 0x1abe_15b8;
    let mut next = |n: u64| { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (seed >> 33) % n };
    let subs = ["Water", "Ethanol", "Acetone", "Mercury", "Sodium chloride", "Substance X"];
    for i in 0..1500 {
        let sub = subs[next(6) as usize];
        let (mp, bp) = super::super::curves::heat_points(&with(&[s("substance", sub)]));
        let lo = mp - [1.0, 5.0, 20.0, 60.0][next(4) as usize];
        let hi = bp + [1.0, 5.0, 20.0, 60.0][next(4) as usize];
        let cool = next(2) == 0;
        let (a, z) = if cool { (hi, lo) } else { (lo, hi) };
        let p = with(&[comp(), s("substance", sub), s("mode", if cool { "Cooling" } else { "Heating" }), f("start", a), f("end", z),
            s("label_size", ["Small", "Medium", "Large"][next(3) as usize]), s("plateau_labels", ["Both states (Solid + liquid)", "The change (Melting)"][next(2) as usize]),
            s("seg_labels", ["States", "Blank lines", "None"][next(3) as usize]), s("temp_labels", ["Temperatures", "m.p. and b.p.", "Blank lines", "None"][next(4) as usize]),
            b("letters", next(2) == 0), b("plateau_dashes", next(2) == 0), s("xaxis", ["Time", "Heat added"][next(2) as usize]), f("mass", [1.0, 10.0, 100.0, 2500.0][next(4) as usize]), b("supercool", next(3) == 0)]);
        check_labels(&format!("sweep {i} {:?}", p.0.iter().filter(|(k, _)| ["substance", "mode", "start", "end", "label_size", "plateau_labels", "seg_labels", "temp_labels", "letters", "plateau_dashes", "xaxis", "mass", "supercool"].contains(&k.as_str())).collect::<Vec<_>>()), &p);
    }
}

#[test]
fn the_student_copy_hides_no_heats_and_the_description_holds_none() {
    let d = def();
    let p = with(&[comp(), s("seg_labels", "Blank lines"), s("temp_labels", "Blank lines")]);
    let (student, _key) = crate::figures::answer_pair(&d, &p).unwrap();
    for q in ["4.18", "33.4", "41.8", "226", "4.02", "kJ,"] { assert!(!student.contains(q), "{q} on the student copy"); }
    let desc: String = student.split("<desc>").nth(1).map(|r| r.split("</desc>").next().unwrap().to_string()).unwrap_or_default();
    let title: String = student.split("<title>").nth(1).map(|r| r.split("</title>").next().unwrap().to_string()).unwrap_or_default();
    assert!(!desc.contains("Melting") && !title.contains("Melting") && !desc.contains("°C"), "{title} / {desc}");
}

#[test]
fn print_and_large_draw() {
    // each new option at Large and in grey alone renders cleanly (the PNGs are looked at by hand)
    for ls in ["Small", "Medium", "Large"] {
        let p = with(&[comp(), s("label_size", ls), s("curve_color", "#111111"), b("letters", true)]);
        let svg = render(&p);
        assert!(clean_numbers(&svg) && curve(&svg, &p).len() == 6);
    }
}
