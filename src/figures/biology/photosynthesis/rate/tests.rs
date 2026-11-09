//! The Rate graphs view: the biology (checked by hand against the formulas and the textbook facts), the student copy,
//! the text, Print, every control, and that the chloroplast view and old saved figures are untouched.
use super::*;

fn def() -> FigureDef { crate::figures::registry().into_iter().find(|d| d.id == super::super::ID).unwrap() }
fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    p.set_s("view", VIEWS[1]);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn svg_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }
fn notes(list: &[(&'static str, Value)]) -> Vec<String> { crate::figures::notes_for(super::super::ID, &with(list)) }
fn close(a: f64, b: f64, tol: f64) -> bool { (a - b).abs() <= tol }

// ---------------------------------------------------------------- the biology

#[test]
fn the_hyperbola_is_the_smaller_root_and_never_above_either_limit() {
    for &(i, pm) in &[(0.5, 10.0), (5.0, 5.0), (20.0, 3.0), (100.0, 0.1), (1e-6, 7.0)] {
        let p = nrh(i, pm);
        // THETA p^2 - (i + pm) p + i pm = 0
        assert!((THETA * p * p - (i + pm) * p + i * pm).abs() < 1e-9 * (i * pm).max(1.0), "{i} {pm}");
        assert!(p <= i.min(pm) + 1e-12 && p > 0.0);
        // the other root is larger than both limits, so it is not the rate
        let other = (i + pm) / THETA - p;
        assert!(other > i.min(pm));
    }
    assert_eq!(nrh(0.0, 5.0), 0.0);
    assert_eq!(nrh(5.0, 0.0), 0.0);
}

#[test]
fn light_raises_the_rate_until_it_levels_off_at_the_plateau_co2_and_temperature_set() {
    let curves = [(0.1, 25.0), (0.1, 15.0), (0.04, 25.0)];
    for (co2, t) in curves {
        assert_eq!(gross(0.0, co2, t), 0.0, "no light, no photosynthesis");
        let mut last = -1.0;
        for k in 0..=200 {
            let r = gross(k as f64, co2, t);
            assert!(r > last || k == 0 && r == 0.0, "rises with light");
            last = r;
        }
        // in dim light the rate is light-limited: the starting slope is ALPHA whatever the CO2 and temperature
        assert!(close(gross(0.5, co2, t) / 0.5, ALPHA, 0.01 * ALPHA));
        // and it levels off at the plateau, which it approaches and never passes
        let pl = pmax(co2, t);
        assert!(gross(1e4, co2, t) < pl && gross(1e4, co2, t) > 0.995 * pl);
    }
    // more CO2 or a warmer temperature (below the optimum) raises the plateau: both are limiting factors
    assert!(pmax(0.1, 25.0) > pmax(0.04, 25.0));
    assert!(pmax(0.1, 25.0) > pmax(0.1, 15.0));
    // and in dim light they make no difference: light limits
    assert!(close(gross(3.0, 0.1, 25.0), gross(3.0, 0.04, 25.0), 0.02));
}

#[test]
fn the_saturation_point_is_where_the_curve_reaches_95_percent_of_its_plateau() {
    let p = with(&[]);
    let st = settings(&p);
    for c in &st.conds {
        let xs = sat_x(&st, c).unwrap();
        let pl = pmax(c.co2, c.temp);
        assert!(close(gross(xs, c.co2, c.temp), SAT_FRAC * pl, 1e-6));
        // by hand from the hyperbola: ALPHA I = P (Pmax - THETA P) / (Pmax - P) with P = 0.95 Pmax
        let by_hand = SAT_FRAC * (1.0 - THETA * SAT_FRAC) / (1.0 - SAT_FRAC) * pl / ALPHA;
        assert!(close(xs, by_hand, 1e-6 * by_hand), "{xs} {by_hand}");
    }
    // the classic three: 0.1 % CO2 at 25, at 15, and 0.04 % at 25 degrees C
    let xs: Vec<f64> = st.conds.iter().map(|c| sat_x(&st, c).unwrap()).collect();
    assert!(xs[0] > xs[2] && xs[2] > xs[1], "{xs:?}");
}

#[test]
fn co2_raises_the_rate_until_light_or_temperature_limits_it() {
    for light in [10.0, 30.0, 80.0] {
        let mut last = f64::NEG_INFINITY;
        for k in 0..=200 {
            let c = k as f64 * 0.005;
            let r = net(light, c, 25.0);
            assert!(r >= last - 1e-12, "the rate rises with CO2");
            last = r;
        }
        // the plateau is set by light: dimmer light, lower plateau
        assert!(gross(light, 5.0, 25.0) <= ALPHA * light + 1e-9);
    }
    assert!(gross(10.0, 1.0, 25.0) < gross(80.0, 1.0, 25.0));
    // no CO2 above the photorespiratory point, no gross photosynthesis
    assert_eq!(gross(80.0, GSTAR, 25.0), 0.0);
    assert!(net(80.0, 0.0, 25.0) < -resp(25.0), "below Gamma* photorespiration gives off CO2 on top of respiration");
}

#[test]
fn temperature_rises_to_an_optimum_near_30_then_falls_steeply_as_enzymes_denature() {
    let to = t_opt();
    assert!(close(to, 30.09, 0.02), "{to}");
    // the closed form is the numerical maximum
    let mut best = (0.0, 0.0);
    for k in 0..=6000 { let t = k as f64 * 0.01; let v = temp_factor(t); if v > best.1 { best = (t, v); } }
    assert!(close(best.0, to, 0.011) && close(best.1, 1.0, 1e-6));
    // doubling every 10 degrees well below the optimum (Q10 = 2, the enzymes intact)
    assert!(close(temp_factor(15.0) / temp_factor(5.0), 2.0, 0.01));
    // a fifth by 40 (the FAQ says so), almost nothing by 45, and steeper on the way down than on the way up
    assert!(close(temp_factor(40.0), 0.2, 0.01), "{}", temp_factor(40.0));
    assert!(temp_factor(45.0) < 0.05);
    assert!(temp_factor(to - 10.0) > temp_factor(to + 10.0) * 2.0);
    for k in 1..300 { let t = k as f64 * 0.1; if t < to - 0.01 { assert!(temp_factor(t) > temp_factor(t - 0.1)); } }
    for k in 0..200 { let t = to + 0.05 + k as f64 * 0.1; assert!(temp_factor(t + 0.1) < temp_factor(t)); }
    // the graph: the rate peaks at the optimum
    let p = with(&[s("rg_graph", GRAPHS[2])]);
    let st = settings(&p);
    assert!(close(opt_x(&st, &st.conds[0]), to, 0.02));
}

#[test]
fn respiration_doubles_every_ten_degrees_and_net_is_gross_less_respiration() {
    assert!(close(resp(25.0), R25, 1e-12));
    assert!(close(resp(35.0), 2.0 * resp(25.0), 1e-12) && close(resp(15.0), 0.5 * resp(25.0), 1e-12));
    for (l, c, t) in [(10.0, 0.04, 20.0), (80.0, 0.1, 30.0), (0.0, 0.1, 25.0)] { assert!(close(net(l, c, t), gross(l, c, t) - resp(t), 1e-12)); }
    // in the dark the net rate is minus respiration
    assert!(close(net(0.0, 0.1, 25.0), -1.0, 1e-12));
}

#[test]
fn the_compensation_points_are_where_net_exchange_is_zero() {
    let l = light_comp(0.1, 25.0).unwrap();
    assert!(close(net(l, 0.1, 25.0), 0.0, 1e-9));
    // in dim light the rate is ALPHA x light, so the light compensation point is about respiration / ALPHA
    assert!(close(l, resp(25.0) / ALPHA, 0.02 * l), "{l}");
    assert!(net(l * 0.9, 0.1, 25.0) < 0.0 && net(l * 1.1, 0.1, 25.0) > 0.0);
    // CO2 compensation point at 25 degrees C in bright light: about 0.007 % (70 ppm), inside the measured 40 to 100 ppm
    let g = co2_comp(80.0, 25.0).unwrap();
    assert!(close(net(80.0, g, 25.0), 0.0, 1e-9) && g > 0.004 && g < 0.01, "{g}");
    // above the optimum respiration catches up
    let t = temp_comp(80.0, 0.1).unwrap();
    assert!(t > t_opt() && close(net(80.0, 0.1, t), 0.0, 1e-9));
}

#[test]
fn distance_from_the_lamp_follows_the_inverse_square_law() {
    for d in [5.0, 10.0, 17.0, 30.0] {
        assert!(close(light_at(2.0 * d), light_at(d) / 4.0, 1e-9));
        assert!(close(light_at(d) * d * d, LAMP_K, 1e-6));
    }
    assert!(close(light_at(10.0), 100.0, 1e-9));
    let p = with(&[s("rg_light_x", LIGHT_X[1])]);
    let st = settings(&p);
    assert_eq!(st.g, G::Distance);
    let c = st.conds[0];
    for d in [4.0, 12.0, 25.0, 40.0] { assert!(close(st.rate_at(&c, d), gross(LAMP_K / (d * d), c.co2, c.temp), 1e-12)); }
    // the rate falls with distance, and the saturation distance gives the saturating light
    assert!(st.rate_at(&c, 40.0) < st.rate_at(&c, 20.0));
    let ds = sat_x(&st, &c).unwrap();
    let lst = settings(&with(&[]));
    assert!(close(light_at(ds), sat_x(&lst, &lst.conds[0]).unwrap(), 1e-6));
}

#[test]
fn a_closed_chamber_loses_co2_in_the_light_to_the_compensation_point_and_gains_it_in_the_dark() {
    let dark = Cond { co2: 0.1, temp: 25.0, light: 0.0 };
    let ser = chamber_series(&dark, 0.04, 60.0, None, 600);
    // in the dark it rises steadily: respiration only, K_CH x R a minute
    assert!(close(ser.last().unwrap().1, 0.04 + K_CH * resp(25.0) * 60.0, 1e-9));
    let bright = Cond { co2: 0.1, temp: 25.0, light: 80.0 };
    let ser = chamber_series(&bright, 0.04, 120.0, None, 1200);
    for w in ser.windows(2) { assert!(w[1].1 <= w[0].1 + 1e-15, "falls in the light"); }
    let g = co2_comp(80.0, 25.0).unwrap();
    assert!(ser.last().unwrap().1 > g && close(ser.last().unwrap().1, g, 2e-4), "levels off at the compensation point");
    // the light going off part way: falls, then rises at the dark rate
    let ser = chamber_series(&bright, 0.04, 60.0, Some(30.0), 600);
    let at = |t: f64| ser.iter().find(|q| q.0 >= t - 1e-9).unwrap().1;
    assert!(at(30.0) < at(0.0) && at(60.0) > at(30.0));
    assert!(close(at(60.0) - at(30.0), K_CH * resp(25.0) * 30.0, 1e-9));
    // the time steps land exactly on the light going off
    assert!(ser.iter().any(|q| close(q.0, 30.0, 1e-9)));
}

#[test]
fn o2_mirrors_co2_one_for_one_on_both_gas_graphs() {
    let svg = svg_of(&[s("rg_graph", GRAPHS[4]), b("rg_light_off", true)]);
    assert_eq!(svg.matches("<polyline").count(), 2 + 2, "CO2 and O2, and their two key samples");
    let p = with(&[s("rg_graph", GRAPHS[5]), b("rg_light_off", true)]);
    let st = settings(&p);
    let ser = chamber_series(&st.conds[0], st.c0, st.tmax, st.off, 600);
    // the points of the O2 against CO2 graph lie on one line of slope -1
    for q in &ser { let o2 = O2_AIR + (st.c0 - q.1); assert!(close(o2 + q.1, O2_AIR + st.c0, 1e-12)); }
    assert!(svg_of(&[s("rg_graph", GRAPHS[5]), b("rg_light_off", true)]).contains("In the dark"));
}

// ---------------------------------------------------------------- the readings the key prints

#[test]
fn the_key_reads_off_each_curve_in_the_units_the_graph_shows() {
    let p = with(&[]);
    let st = settings(&p);
    let r = readouts(&st, x_end_shown(&st, &p));
    assert_eq!(r.len(), 3);
    let x0 = sat_x(&st, &st.conds[0]).unwrap();
    assert!(r[0].contains(&format!("about {}", sig2(x0))) && r[0].contains(&format!("about {} arbitrary units", sig2(pmax(0.1, 25.0)))), "{}", r[0]);
    assert!(r[0].contains("limited by CO~2 concentration") && r[1].contains("limited by temperature") && r[2].contains("limited by CO~2 concentration"));
    // bubbles: three a unit, net
    let p = with(&[s("rg_unit", UNITS[1]), s("rg_rate", RATES[1]), i("rg_curves", 1)]);
    let st = settings(&p);
    let r = readouts(&st, x_end_shown(&st, &p));
    assert!(r[0].contains(&format!("{} bubbles a minute", sig2(3.0 * (pmax(0.1, 25.0) - resp(25.0))))), "{}", r[0]);
    // ppm on the CO2 graph
    let p = with(&[s("rg_graph", GRAPHS[1]), s("rg_co2_unit", "ppm"), i("rg_curves", 1), f("c1_light", 20.0)]);
    let st = settings(&p);
    let xs = sat_x(&st, &st.conds[0]).unwrap();
    assert!(readouts(&st, x_end_shown(&st, &p))[0].contains(&format!("{} ppm", sig2(xs * 1e4))));
    assert_eq!(sig2(41.2), "41");
    assert_eq!(sig2(9.83), "9.8");
    assert_eq!(sig2(0.00615), "0.0062");
}

#[test]
fn the_panel_note_under_each_curve_gives_its_reading_in_plain_text() {
    let p = with(&[]);
    let h = curve_help("Curve 2", &p);
    assert!(h.starts_with("Light limits the rate") && h.contains("limited by temperature"), "{h}");
    assert!(curve_help("Curve 1", &p).contains("CO\u{2082}") && !curve_help("Curve 1", &p).contains('~'));
    assert!(curve_help("Curve 3", &with(&[i("rg_curves", 2)])).is_empty());
    let mut chloro = p.clone();
    chloro.set_s("view", VIEWS[0]);
    assert!(curve_help("Curve 1", &chloro).is_empty());
}

// ---------------------------------------------------------------- what the student copy leaves out

fn all_graph_states() -> Vec<Vec<(&'static str, Value)>> {
    let mut v = Vec::new();
    for g in GRAPHS {
        for lx in LIGHT_X {
            if g != GRAPHS[0] && lx != LIGHT_X[0] { continue; }
            for m in MARKS {
                for (rate, unit) in [(RATES[0], UNITS[0]), (RATES[1], UNITS[1])] {
                    v.push(vec![s("rg_graph", g), s("rg_light_x", lx), s("rg_mark", m), s("rg_rate", rate), s("rg_unit", unit), b("rg_light_off", true)]);
                }
            }
        }
    }
    v
}

fn texts(svg: &str) -> String {
    let doc = roxmltree::Document::parse(svg).unwrap();
    // the drawing's text: not the accessible title and description (tested apart)
    doc.descendants().filter(|n| n.is_text() && n.parent().is_some_and(|p| !p.has_tag_name("title") && !p.has_tag_name("desc"))).map(|n| n.text().unwrap_or("")).collect::<Vec<_>>().join("|")
}

#[test]
fn a_student_copy_has_no_answer_in_the_drawing_or_the_description() {
    let words = ["Saturation point", "Compensation point", "Optimum", "limiting", "Enzymes denaturing", "levels off", "levelling off", "compensation", "saturation"];
    for st in all_graph_states() {
        let mut l = st.clone();
        l.push(b(SHOW_ANSWERS, false));
        let svg = svg_of(&l);
        let t = texts(&svg);
        for w in words { assert!(!t.contains(w), "{st:?}: the student copy says {w:?}: {t}"); }
        // no number of the key's readings either: the description names the graph only
        let desc = svg.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
        let bare = desc.replace("2 curves", "").replace("3 curves", "").replace("CO2", "").replace("O2", "");
        assert!(!bare.chars().any(|c| c.is_ascii_digit()), "{desc}");
        assert!(!desc.to_lowercase().contains("saturation") && !desc.to_lowercase().contains("compensation") && !desc.contains("about"), "{desc}");
        // the key of the same state does label (where the graph has the mark) and prints the readings
        let mut k = st.clone();
        k.push(b(SHOW_ANSWERS, true));
        let key = svg_of(&k);
        assert!(texts(&key).len() > t.len(), "{st:?}: the key prints more than the student copy");
    }
}

#[test]
fn a_label_can_be_a_blank_line_or_left_off_and_the_student_copy_blanks_them() {
    let rules = |svg: &str| svg.matches(r#"stroke-width="1.1"/>"#).count();
    let base = [s("rg_mark", "Limiting factors")];
    let key = svg_of(&base);
    assert!(texts(&key).contains("Temperature limiting") && rules(&key) == 0);
    let blank = svg_of(&[s("rg_mark", "Limiting factors"), s("rg_mark_lab", "Blank line")]);
    assert!(!texts(&blank).contains("limiting") && rules(&blank) == 4, "one rule for the rise and one per plateau: {}", rules(&blank));
    let none = svg_of(&[s("rg_mark", "Limiting factors"), s("rg_mark_lab", "No label")]);
    assert!(!texts(&none).contains("limiting") && rules(&none) == 0);
    let student = svg_of(&[s("rg_mark", "Limiting factors"), b(SHOW_ANSWERS, false)]);
    assert_eq!(rules(&student), 4);
    // titles can be blank lines too
    let t = svg_of(&[s("rg_title_mode", "Blank line"), s("rg_xt_mode", "Blank line"), s("rg_yt_mode", "Blank line")]);
    assert!(!texts(&t).contains("Light intensity") && !texts(&t).contains("Rate of photosynthesis"));
    let n = svg_of(&[s("rg_xt_mode", "None"), s("rg_yt_mode", "None")]);
    assert!(!texts(&n).contains("Light intensity (") && !texts(&n).contains("Rate of photosynthesis"));
    let typed = svg_of(&[s("rg_title_mode", "Text"), s("rg_title", "Elodea at three settings"), s("rg_xt", "Lamp power"), s("rg_yt", "Bubbles counted")]);
    for w in ["Elodea at three settings", "Lamp power", "Bubbles counted"] { assert!(texts(&typed).contains(w), "{w}"); }
}

#[test]
fn points_only_leaves_the_line_for_the_student_and_points_scatter_a_little_about_it() {
    let lines = |svg: &str| svg.matches("<polyline").count();
    let plain = svg_of(&[i("rg_curves", 1)]);
    let on = svg_of(&[i("rg_curves", 1), s("rg_points", POINTS[1])]);
    let only = svg_of(&[i("rg_curves", 1), s("rg_points", POINTS[2])]);
    assert_eq!(lines(&plain), 1);
    assert_eq!(lines(&on), 1);
    assert_eq!(lines(&only), 0);
    assert_eq!(on.matches("<circle").count(), 8);
    assert_eq!(only.matches("<circle").count(), 8);
    // another seed, another scatter; the same seed, the same points
    assert_ne!(on, svg_of(&[i("rg_curves", 1), s("rg_points", POINTS[1]), i("rg_seed", 2)]));
    assert_eq!(on, svg_of(&[i("rg_curves", 1), s("rg_points", POINTS[1])]));
    // every point is within 3 % of the y axis of the curve at its x (read back from the SVG)
    let p = with(&[i("rg_curves", 1)]);
    let st = settings(&p);
    let doc = roxmltree::Document::parse(&on).unwrap();
    let poly: Vec<(f64, f64)> = doc.descendants().find(|n| n.has_tag_name("polyline")).unwrap().attribute("points").unwrap().split(' ').map(|q| { let (a, c) = q.split_once(',').unwrap(); (a.parse().unwrap(), c.parse().unwrap()) }).collect();
    let (y_top, y_bot) = (poly.iter().map(|q| q.1).fold(f64::INFINITY, f64::min), poly[0].1);
    let _ = st;
    for c in doc.descendants().filter(|n| n.has_tag_name("circle")) {
        let (cx, cy): (f64, f64) = (c.attribute("cx").unwrap().parse().unwrap(), c.attribute("cy").unwrap().parse().unwrap());
        let near = poly.iter().min_by(|a, z| (a.0 - cx).abs().total_cmp(&(z.0 - cx).abs())).unwrap();
        assert!((near.1 - cy).abs() <= 0.03 * 300.0 + 1.0, "{cx},{cy} vs {near:?} ({y_top}..{y_bot})");
    }
}

// ---------------------------------------------------------------- the controls

#[test]
fn every_rate_control_changes_the_drawing_where_it_applies() {
    let pd = (def().params)();
    // a state where the control is relevant, and a value for it that differs from the default
    let cases: Vec<(Vec<(&'static str, Value)>, (&'static str, Value))> = vec![
        (vec![], s("rg_graph", GRAPHS[1])),
        (vec![], s("rg_light_x", LIGHT_X[1])),
        (vec![], s("rg_rate", RATES[1])),
        (vec![], s("rg_unit", UNITS[1])),
        (vec![s("rg_graph", GRAPHS[1])], s("rg_co2_unit", "ppm")),
        (vec![], s("rg_co2_unit", "ppm")),
        (vec![], i("rg_curves", 2)),
        (vec![], f("c1_co2", 0.3)), (vec![], f("c2_co2", 0.3)), (vec![], f("c3_co2", 0.3)),
        (vec![], f("c1_temp", 30.0)), (vec![], f("c2_temp", 30.0)), (vec![], f("c3_temp", 30.0)),
        (vec![s("rg_graph", GRAPHS[1])], f("c1_light", 40.0)), (vec![s("rg_graph", GRAPHS[1])], f("c2_light", 40.0)), (vec![s("rg_graph", GRAPHS[1])], f("c3_light", 40.0)),
        (vec![], s("c1_name", "Pondweed A")), (vec![], s("c2_name", "B")), (vec![], s("c3_name", "C")),
        (vec![s("rg_graph", GRAPHS[3])], f("rg_c0", 0.1)),
        (vec![s("rg_graph", GRAPHS[3])], f("rg_tmax", 90.0)),
        (vec![s("rg_graph", GRAPHS[3])], b("rg_light_off", true)),
        (vec![s("rg_graph", GRAPHS[3]), b("rg_light_off", true)], f("rg_off_at", 20.0)),
        (vec![], s("rg_mark", "Saturation point")),
        (vec![s("rg_mark", "Saturation point")], s("rg_mark_lab", "Blank line")),
        (vec![], s("rg_points", POINTS[1])),
        (vec![s("rg_points", POINTS[1])], i("rg_seed", 5)),
        (vec![], s("rg_title_mode", "Text")),
        (vec![s("rg_title_mode", "Text")], s("rg_title", "My title")),
        (vec![], s("rg_xt_mode", "None")),
        (vec![], s("rg_xt", "My x")),
        (vec![], s("rg_yt_mode", "None")),
        (vec![], s("rg_yt", "My y")),
        (vec![], s("rg_axes", AXES_MODES[1])),
        (vec![], s("rg_xmax", "60")),
        (vec![], s("rg_ymax", "20")),
        (vec![], b("rg_grid", false)),
        (vec![], s("style", "Print")),
        (vec![], s("label_size", "Large")),
        (vec![s("rg_mark", "Saturation point")], b(SHOW_ANSWERS, false)),
    ];
    let mut covered: Vec<&str> = Vec::new();
    for (base, (k, v)) in &cases {
        let p0 = with(base);
        assert!(relevant(k, &p0), "{k} is shown in {base:?}");
        let before = (def().render)(&p0);
        let mut l = base.clone();
        l.push((k, v.clone()));
        let after = svg_of(&l);
        assert_ne!(before, after, "{k} = {v:?} changed nothing over {base:?}");
        covered.push(k);
    }
    // every rate-view control is in the list
    for d in &pd { if d.key.starts_with("rg_") || curve_key(d.key).is_some() { assert!(covered.contains(&d.key), "{} is not tested", d.key); } }
}

#[test]
fn only_the_chosen_views_and_graphs_controls_show() {
    let chloro = { let mut p = with(&[]); p.set_s("view", VIEWS[0]); p };
    let rate = with(&[]);
    for k in ["show_parts", "graph_x", "pmax", "mark"] { assert!(relevant(k, &chloro) && !relevant(k, &rate), "{k}"); }
    for k in ["rg_graph", "c1_co2", "rg_mark"] { assert!(!relevant(k, &chloro) && relevant(k, &rate), "{k}"); }
    for k in ["view", "show_answers", "style", "label_size"] { assert!(relevant(k, &chloro) && relevant(k, &rate)); }
    // the light graph takes CO2 and temperature per curve; the CO2 graph light and temperature; the temperature graph light and CO2
    assert!(relevant("c1_co2", &rate) && relevant("c1_temp", &rate) && !relevant("c1_light", &rate));
    let co2 = with(&[s("rg_graph", GRAPHS[1])]);
    assert!(!relevant("c1_co2", &co2) && relevant("c1_temp", &co2) && relevant("c1_light", &co2));
    let temp = with(&[s("rg_graph", GRAPHS[2])]);
    assert!(relevant("c1_co2", &temp) && !relevant("c1_temp", &temp) && relevant("c1_light", &temp));
    // curves past the count are hidden; the gas graphs are one plant
    let two = with(&[i("rg_curves", 2)]);
    assert!(relevant("c2_co2", &two) && !relevant("c3_co2", &two));
    let gas = with(&[s("rg_graph", GRAPHS[4])]);
    assert!(!relevant("rg_curves", &gas) && relevant("c1_light", &gas) && !relevant("c2_light", &gas) && relevant("rg_c0", &gas) && !relevant("rg_rate", &gas));
    assert!(!relevant("rg_off_at", &gas) && relevant("rg_off_at", &with(&[s("rg_graph", GRAPHS[4]), b("rg_light_off", true)])));
    // the panel shows the same thing (param_relevant goes through here)
    assert!(!crate::figures::param_relevant(super::super::ID, "pmax", &rate) && crate::figures::param_relevant(super::super::ID, "rg_graph", &rate));
}

#[test]
fn a_mark_that_the_graph_does_not_have_says_so_and_draws_nothing() {
    let n = notes(&[s("rg_mark", "Optimum")]);
    assert!(n.iter().any(|t| t.contains("temperature")), "{n:?}");
    assert_eq!(svg_of(&[s("rg_mark", "Optimum")]), svg_of(&[]).replace("", ""));
    let n = notes(&[s("rg_graph", GRAPHS[2]), s("rg_mark", "Saturation point")]);
    assert_eq!(n.len(), 1, "{n:?}");
    let n = notes(&[s("rg_xmax", "abc")]);
    assert!(n.iter().any(|t| t.contains("\"abc\" is not a number")));
    let n = notes(&[s("rg_graph", GRAPHS[3]), b("rg_light_off", true), f("rg_off_at", 200.0)]);
    assert!(n.iter().any(|t| t.contains("after the 60 minutes shown")), "{n:?}");
}

#[test]
fn the_classic_setups_draw_what_they_name_with_nothing_to_warn_about() {
    let setups = classics();
    assert_eq!(setups.len(), 6);
    assert_eq!(crate::figures::classic_setups(super::super::ID).len(), 6);
    let d = def();
    let pd = (d.params)();
    for e in &setups {
        let mut p = Params::defaults(&pd);
        for (k, v) in &e.params { assert!(p.0.contains_key(*k), "{}: unknown {k}", e.name); p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&pd, false);
        assert!(is_rate(&p), "{}", e.name);
        let n = crate::figures::notes_for(d.id, &p);
        assert!(n.is_empty(), "{}: {n:?}", e.name);
        let svg = (d.render)(&p);
        roxmltree::Document::parse(&svg).unwrap();
    }
    let g = |k: usize| { let mut p = Params::defaults(&pd); for (a, v) in &setups[k].params { p.0.insert(a.to_string(), v.clone()); } graph_of(&p) };
    assert_eq!([g(0), g(1), g(2), g(3), g(4), g(5)], [G::Light, G::Distance, G::Co2, G::Temp, G::Chamber, G::GasTime]);
}

// ---------------------------------------------------------------- text and Print

fn walk(g: &resvg::usvg::Group, out: &mut Vec<(String, (f64, f64, f64, f64), f64)>) {
    for n in g.children() {
        match n {
            resvg::usvg::Node::Group(g) => walk(g, out),
            resvg::usvg::Node::Text(t) => {
                let r = t.flattened().bounding_box().transform(t.abs_transform()).unwrap_or(t.flattened().abs_bounding_box());
                let txt: String = t.chunks().iter().map(|c| c.text()).collect();
                let size = t.chunks().iter().flat_map(|c| c.spans().iter().map(|s| s.font_size().get() as f64)).fold(f64::INFINITY, f64::min);
                out.push((txt, (r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64), size));
            }
            _ => {}
        }
    }
}

fn ink(svg: &str) -> Vec<(String, (f64, f64, f64, f64), f64)> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(&crate::figures::serif_stack(svg), &opt).unwrap();
    let mut v = Vec::new();
    walk(tree.root(), &mut v);
    v.retain(|(t, b, _)| !t.trim().is_empty() && b.2 > b.0);
    v
}

#[test]
fn text_is_readable_inside_the_page_and_never_overlaps_at_any_size() {
    let long = "Pondweed in a beaker of water with sodium hydrogencarbonate added, at the far end of the bench";
    let mut n = 0;
    for size in ["Small", "Medium", "Large"] {
        for st in all_graph_states().into_iter().step_by(3) {
            for extra in [vec![], vec![s("c1_name", long), s("rg_title_mode", "Text"), s("rg_title", long), s("rg_yt", long), s("rg_points", POINTS[1])]] {
                let mut l = st.clone();
                l.extend(extra.clone());
                l.push(s("label_size", size));
                let svg = svg_of(&l);
                let (w, h) = crate::figures::svg_size(&svg);
                let t = ink(&svg);
                for (txt, bx, fs) in &t {
                    assert!(*fs >= 8.0, "{txt:?} drawn at {fs} px");
                    assert!(bx.0 >= -0.5 && bx.1 >= -0.5 && bx.2 <= w + 0.5 && bx.3 <= h + 0.5, "{txt:?} {bx:?} outside {w}x{h} ({l:?})");
                }
                for a in 0..t.len() {
                    for z in a + 1..t.len() {
                        let (p, q) = (t[a].1, t[z].1);
                        let ov = (p.2.min(q.2) - p.0.max(q.0)).max(0.0) * (p.3.min(q.3) - p.1.max(q.1)).max(0.0);
                        assert!(ov < 2.0, "{:?} overlaps {:?} ({l:?})", t[a].0, t[z].0);
                    }
                }
                n += 1;
            }
        }
    }
    assert!(n > 100);
}

#[test]
fn print_is_grey_only() {
    let grey = |c: &str| -> bool {
        let c = c.trim().to_ascii_lowercase();
        if matches!(c.as_str(), "none" | "#fff" | "#000") || c.starts_with("url(") { return true; }
        c.len() == 7 && c.starts_with('#') && c[1..3] == c[3..5] && c[3..5] == c[5..7]
    };
    for st in all_graph_states() {
        let mut l = st.clone();
        l.extend([s("style", "Print"), s("rg_points", POINTS[1])]);
        let svg = svg_of(&l);
        let doc = roxmltree::Document::parse(&svg).unwrap();
        for nd in doc.descendants().filter(|n| n.is_element()) {
            for a in ["fill", "stroke"] { if let Some(v) = nd.attribute(a) { assert!(grey(v), "{a}={v} in Print ({st:?})"); } }
        }
    }
}

#[test]
fn label_contrast_is_enough_on_the_dark_shading() {
    // the words over the shading are INK on SHADE
    let lum = |h: &str| { let c = |i: usize| { let v = u8::from_str_radix(&h[i..i + 2], 16).unwrap() as f64 / 255.0; if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) } }; 0.2126 * c(1) + 0.7152 * c(3) + 0.0722 * c(5) };
    let ratio = (lum(SHADE) + 0.05) / (lum(INK) + 0.05);
    assert!(ratio >= 4.5, "{ratio}");
}

#[test]
fn nothing_panics_and_every_render_is_well_formed() {
    for st in all_graph_states() {
        for pts in POINTS {
            for ax in AXES_MODES {
                let mut l = st.clone();
                l.extend([s("rg_points", pts), s("rg_axes", ax), s("rg_xmax", ["", "7", "-3", "x", "1e9"][pts.len() % 5]), s("rg_ymax", ["", "0.001", "abc"][ax.len() % 3])]);
                let svg = svg_of(&l);
                roxmltree::Document::parse(&svg).unwrap_or_else(|e| panic!("{e}: {l:?}"));
                let (w, h) = crate::figures::svg_size(&svg);
                assert!(w > 400.0 && h > 300.0 && h < 3000.0, "{w}x{h}");
            }
        }
    }
}

// ---------------------------------------------------------------- the chloroplast and saved figures are untouched

#[test]
fn the_chloroplast_view_and_old_saved_figures_render_exactly_as_before() {
    let d = def();
    // a figure saved before the Rate graphs view existed has none of its keys: it opens in the Chloroplast view
    let old = crate::model::FigureSpec { id: d.id.to_string(), params: serde_json::json!({"show_graph": true, "graph_x": "Temperature", "pmax": "8", "xval": "28", "mark": "36", "show_answers": false, "style": "Print", "label_size": "Large"}).as_object().unwrap().clone().into_iter().collect() };
    let p = crate::figures::spec::params_from_spec(&d, &old);
    assert!(!is_rate(&p));
    // whatever the rate controls are set to, the chloroplast view draws the same
    let svg = (d.render)(&p);
    let mut q = p.clone();
    for (k, v) in [("rg_graph", Value::S(GRAPHS[3].into())), ("c1_co2", Value::F(0.5)), ("rg_mark", Value::S("Optimum".into())), ("rg_points", Value::S(POINTS[2].into())), ("rg_grid", Value::B(false))] { q.0.insert(k.into(), v); }
    assert_eq!(svg, (d.render)(&q));
    assert!(svg.contains("Chloroplast diagram") || svg.contains("chloroplast"));
    // and with the view the old spec renders as the old code did: the view switch is the first thing draw looks at
    assert!(svg.len() > 1000 && !svg.contains("ps-rate-plot"));
}

#[test]
fn nothing_to_drag_in_either_view() {
    let d = def();
    assert!(d.handles(&with(&[]), (800.0, 600.0)).is_empty());
}

#[test]
fn mark_labels_and_letters_sit_clear_of_every_curve() {
    let mut n = 0;
    for size in ["Small", "Medium", "Large"] {
        for st in all_graph_states() {
            let mut l = st.clone();
            l.push(s("label_size", size));
            let svg = svg_of(&l);
            let doc = roxmltree::Document::parse(&svg).unwrap();
            // the curves: polylines drawn 2.8 wide inside the plot's clip group
            let segs: Vec<((f64, f64), (f64, f64))> = doc.descendants().filter(|n| n.has_tag_name("polyline") && n.attribute("stroke-width") == Some("2.8") && n.ancestors().any(|a| a.attribute("clip-path").is_some())).flat_map(|n| {
                let pts: Vec<(f64, f64)> = n.attribute("points").unwrap().split(' ').map(|q| { let (a, c) = q.split_once(',').unwrap(); (a.parse().unwrap(), c.parse().unwrap()) }).collect();
                pts.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>()
            }).collect();
            let marks = ["Saturation point", "Compensation point", "Optimum", "limiting", "Enzymes denaturing", "Respiration", "A", "B", "C", "In the light", "In the dark", "Light", "Dark"];
            for (txt, bx, _) in ink(&svg) {
                if !marks.iter().any(|m| txt == *m || (m.len() > 3 && txt.contains(m))) { continue; }
                for (a, z) in &segs {
                    let hit = (0..=40).any(|k| { let t = k as f64 / 40.0; let q = (a.0 + (z.0 - a.0) * t, a.1 + (z.1 - a.1) * t); q.0 > bx.0 - 1.0 && q.0 < bx.2 + 1.0 && q.1 > bx.1 - 1.0 && q.1 < bx.3 + 1.0 });
                    assert!(!hit, "{txt:?} at {bx:?} touches a curve ({l:?})");
                }
                n += 1;
            }
        }
    }
    assert!(n > 100, "{n}");
}


// ---------------------------------------------------------------- the drop on the temperature graph, read off each curve

/// The drawn curve of a one-curve render as (T, rate) pairs read back from the SVG: the polyline spans the axis from
/// 0 to its right end, and the x-axis line (1.8 wide) is rate 0 on a gross graph.
fn drawn_curve(svg: &str, x_hi: f64, top_value: f64) -> Vec<(f64, f64)> {
    let doc = roxmltree::Document::parse(svg).unwrap();
    let pl = doc.descendants().find(|n| n.has_tag_name("polyline") && n.attribute("stroke-width") == Some("2.8") && n.ancestors().any(|a| a.attribute("clip-path").is_some())).unwrap();
    let pts: Vec<(f64, f64)> = pl.attribute("points").unwrap().split(' ').map(|q| { let (a, c) = q.split_once(',').unwrap(); (a.parse().unwrap(), c.parse().unwrap()) }).collect();
    let base = doc.descendants().filter(|n| n.has_tag_name("line") && n.attribute("stroke-width") == Some("1.8") && n.attribute("y1") == n.attribute("y2")).map(|n| n.attribute("y1").unwrap().parse::<f64>().unwrap()).fold(f64::NEG_INFINITY, f64::max);
    let (x0, x1) = (pts[0].0, pts[pts.len() - 1].0);
    let ymin = pts.iter().map(|q| q.1).fold(f64::INFINITY, f64::min);
    pts.iter().map(|q| ((q.0 - x0) / (x1 - x0) * x_hi, (base - q.1) / (base - ymin) * top_value)).collect()
}

#[test]
fn the_steep_fall_is_read_off_each_temperature_curve_as_drawn_and_the_key_and_note_say_the_same() {
    for light in [10.0, 15.0, 20.0, 30.0, 50.0, 80.0, 100.0] {
        let l = [s("rg_graph", GRAPHS[2]), i("rg_curves", 1), f("c1_light", light), f("c1_co2", 0.1), s("rg_mark", "Limiting factors")];
        let p = with(&l);
        let st = settings(&p);
        let c = st.conds[0];
        let drop = drop_x(&st, &c);
        // read off the drawn curve: where it falls below 95 % of its own top, past the peak
        let svg = svg_of(&l);
        let top = st.rate_at(&c, opt_x(&st, &c));
        let curve = drawn_curve(&svg, 50.0, top);
        let peak = curve.iter().enumerate().max_by(|a, z| a.1 .1.total_cmp(&z.1 .1)).unwrap().0;
        let t95 = curve[peak..].iter().find(|q| q.1 < 0.95 * top).unwrap().0;
        assert!((drop - t95).abs() < 0.6, "light {light}: drop {drop:.2} but the drawn curve leaves 95 % of its top at {t95:.2}");
        // dim light stays flat for longer: the drop is later the dimmer the light, and none of them is the fixed 34
        if light <= 20.0 { assert!(drop > 37.0, "light {light}: {drop}"); }
        // the key and the panel note give that temperature, to two figures
        let key = readouts(&st, 50.0);
        let want = format!("above about {} \u{b0}C", sig2(drop));
        assert!(key[0].contains(&want), "light {light}: {}", key[0]);
        assert!(curve_help("Curve 1", &p).contains(&want.replace("~", "")), "{}", curve_help("Curve 1", &p));
        // the Enzymes denaturing label sits by the steep part of this curve, not before the drop
        let (cx0, cx1) = { let (pts, _) = curves_and_grid(&svg); (pts[0][0].0, pts[0][pts[0].len() - 1].0) };
        let rows: Vec<(f64, f64, f64, f64)> = ink(&svg).into_iter().filter(|t| ["Enzymes denaturing", "Enzymes", "denaturing"].contains(&t.0.as_str())).map(|t| t.1).collect();
        assert!(!rows.is_empty(), "light {light}: the label is drawn");
        let left = rows.iter().map(|r| r.0).fold(f64::INFINITY, f64::min);
        let t_left = (left - cx0) / (cx1 - cx0) * 50.0;
        assert!(t_left > drop - 1.0, "light {light}: the label starts at {t_left:.1} degrees, before the drop at {drop:.1}");
    }
    // net: the steep fall is the photosynthesis part plunging, not respiration's gentle climb
    for light in [10.0, 15.0, 30.0] {
        let p = with(&[s("rg_graph", GRAPHS[2]), i("rg_curves", 1), f("c1_light", light), f("c1_co2", 0.1), s("rg_rate", RATES[1])]);
        let st = settings(&p);
        let c = st.conds[0];
        let drop = drop_x(&st, &c);
        let slope = |t: f64| (st.rate_at(&c, t + 0.1) - st.rate_at(&c, t - 0.1)) / 0.2;
        let top = (opt_x(&st, &c), st.rate_at(&c, opt_x(&st, &c))).1;
        assert!(drop > 35.0, "light {light}: {drop}");
        let _ = top;
        // before the drop the net curve falls only about as fast as respiration climbs (it doubles every 10 degrees,
        // so its slope is R ln 2 / 10); after it, at least three times faster
        let resp_slope = resp(drop - 3.0) * 2f64.ln() / 10.0;
        assert!(slope(drop - 3.0) > -1.5 * resp_slope && slope(drop + 1.5) < 3.0 * slope(drop - 3.0).min(-1e-3), "light {light}: {} then {} (respiration {resp_slope})", slope(drop - 3.0), slope(drop + 1.5));
        assert!(readouts(&st, 50.0)[0].contains(&format!("above about {} \u{b0}C enzymes denature", sig2(drop))));
    }
}

#[test]
fn the_chamber_key_says_what_the_curve_reaches_in_the_time_shown() {
    let l = [s("rg_graph", GRAPHS[3]), i("rg_curves", 2), f("c1_light", 10.0), f("c1_temp", 25.0), f("c2_light", 80.0), f("c2_temp", 25.0)];
    let p = with(&l);
    let st = settings(&p);
    let r = readouts(&st, 60.0);
    let dim_end = chamber_series(&st.conds[0], st.c0, 60.0, None, 600).last().unwrap().1;
    assert!(r[0].contains("still falling") && r[0].contains(&format!("to about {} % by 60 minutes", sig2(dim_end))) && !r[0].contains("levelling off"), "{}", r[0]);
    let g = co2_comp(80.0, 25.0).unwrap();
    assert!(r[1].contains(&format!("levelling off at about {} %", sig2(g))), "{}", r[1]);
    // long enough, the dim one levels off too
    let p = with(&[s("rg_graph", GRAPHS[3]), i("rg_curves", 1), f("c1_light", 10.0), f("c1_temp", 25.0), f("rg_tmax", 240.0)]);
    assert!(readouts(&settings(&p), 240.0)[0].contains("levelling off"));
}

#[test]
fn a_saturation_point_past_the_end_of_the_axis_is_said_to_be_off_the_graph() {
    let p = with(&[s("rg_graph", GRAPHS[1]), s("rg_co2_unit", "ppm")]);
    let st = settings(&p);
    let end = x_end_shown(&st, &p);
    assert_eq!(end, 4000.0);
    let r = readouts(&st, end);
    let xs = sat_x(&st, &st.conds[0]).unwrap() * 1e4;
    assert!(xs > end && r[0].contains("off the graph") && r[0].contains(&format!("about {} ppm", sig2(xs))), "{}", r[0]);
    assert!(!r[2].contains("off the graph"), "{}", r[2]);
    // typing a longer axis brings it on
    let p = with(&[s("rg_graph", GRAPHS[1]), s("rg_co2_unit", "ppm"), s("rg_xmax", "6000")]);
    let st = settings(&p);
    assert!(!readouts(&st, x_end_shown(&st, &p))[0].contains("off the graph"));
}

#[test]
fn one_of_a_unit_is_singular() {
    let p = with(&[s("rg_graph", GRAPHS[1]), s("rg_rate", RATES[1])]);
    let r = readouts(&settings(&p), 0.4).join(" ");
    assert!(r.contains("is 1 arbitrary unit,") && !r.contains(" 1 arbitrary units"), "{r}");
    assert_eq!(singular("about 1 arbitrary units (1 bubbles a minute) and 11 arbitrary units"), "about 1 arbitrary unit (1 bubble a minute) and 11 arbitrary units");
    assert_eq!(singular("0.1 arbitrary units"), "0.1 arbitrary units");
}

/// The curves of a render, as screen points, and the horizontal gridlines' y.
fn curves_and_grid(svg: &str) -> (Vec<Vec<(f64, f64)>>, Vec<f64>) {
    let doc = roxmltree::Document::parse(svg).unwrap();
    let curves = doc.descendants().filter(|n| n.has_tag_name("polyline") && n.attribute("stroke-width") == Some("2.8") && n.ancestors().any(|a| a.attribute("clip-path").is_some())).map(|n| n.attribute("points").unwrap().split(' ').map(|q| { let (a, c) = q.split_once(',').unwrap(); (a.parse().unwrap(), c.parse().unwrap()) }).collect()).collect();
    let grid = doc.descendants().filter(|n| n.has_tag_name("line") && n.attribute("stroke-width") == Some("1") && n.attribute("y1") == n.attribute("y2")).map(|n| n.attribute("y1").unwrap().parse().unwrap()).collect();
    (curves, grid)
}

fn dist_to(c: &[(f64, f64)], bx: (f64, f64, f64, f64)) -> f64 {
    c.iter().map(|q| (q.0.clamp(bx.0, bx.2) - q.0).hypot(q.1.clamp(bx.1, bx.3) - q.1)).fold(f64::INFINITY, f64::min)
}

#[test]
fn each_plateau_label_and_blank_sits_by_its_own_curve_and_no_blank_lies_on_a_gridline() {
    let mut checked = 0;
    for size in ["Small", "Medium", "Large"] {
        for g in [GRAPHS[0], GRAPHS[1]] {
            for lx in LIGHT_X {
                if g == GRAPHS[1] && lx == LIGHT_X[1] { continue; }
                let base = vec![s("rg_graph", g), s("rg_light_x", lx), s("rg_mark", "Limiting factors"), s("label_size", size)];
                let p = with(&base);
                let st = settings(&p);
                // labels: the curve nearest each plateau label is a curve that label names
                let svg = svg_of(&base);
                let (curves, grid) = curves_and_grid(&svg);
                for (txt, bx, _) in ink(&svg) {
                    let names: Vec<usize> = (0..st.conds.len()).filter(|k| plain(plateau_limit(&st, &st.conds[*k])) == txt).collect();
                    if names.is_empty() { continue; }
                    let near = (0..curves.len()).min_by(|a, z| dist_to(&curves[*a], bx).total_cmp(&dist_to(&curves[*z], bx))).unwrap();
                    assert!(names.contains(&near), "{txt:?} is nearest curve {near}, which it does not name ({base:?})");
                    checked += 1;
                }
                // blanks: each rule is clearly nearer one curve than any other, and not along a gridline
                let mut bl = base.clone();
                bl.push(s("rg_mark_lab", "Blank line"));
                let svg = svg_of(&bl);
                let (curves, grid2) = curves_and_grid(&svg);
                let _ = grid;
                let doc = roxmltree::Document::parse(&svg).unwrap();
                for n in doc.descendants().filter(|n| n.has_tag_name("line") && n.attribute("stroke-width") == Some("1.1")) {
                    let v = |k: &str| n.attribute(k).unwrap().parse::<f64>().unwrap();
                    let bx = (v("x1"), v("y1") - 12.0, v("x2"), v("y1"));
                    let mut d: Vec<f64> = curves.iter().map(|c| dist_to(c, bx)).collect();
                    d.sort_by(|a, z| a.total_cmp(z));
                    if d.len() > 1 { assert!(d[1] > d[0] + 2.0, "a blank between two curves: {d:?} ({bl:?})"); }
                    assert!(grid2.iter().all(|gy| (gy - v("y1")).abs() > 2.0), "a blank on a gridline at y {} ({bl:?})", v("y1"));
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 30, "{checked}");
}

#[test]
fn letters_keep_clear_of_the_direction_arrows_on_co2_against_o2() {
    for size in ["Small", "Medium", "Large"] {
        for off in [20.0, 30.0, 45.0] {
            let l = [s("rg_graph", GRAPHS[5]), s("rg_mark", "Lettered points"), b("rg_light_off", true), f("rg_off_at", off), s("label_size", size)];
            let svg = svg_of(&l);
            let doc = roxmltree::Document::parse(&svg).unwrap();
            // the arrows: the shafts drawn 1.8 wide in grey, and their heads (filled triangles)
            let heads: Vec<(f64, f64, f64, f64)> = doc.descendants().filter(|n| n.has_tag_name("polygon") && n.attribute("stroke-width") == Some("0.5")).map(|n| {
                let pts: Vec<(f64, f64)> = n.attribute("points").unwrap().split(' ').map(|q| { let (a, c) = q.split_once(',').unwrap(); (a.parse().unwrap(), c.parse().unwrap()) }).collect();
                (pts.iter().map(|q| q.0).fold(f64::INFINITY, f64::min), pts.iter().map(|q| q.1).fold(f64::INFINITY, f64::min), pts.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max), pts.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max))
            }).collect();
            let shafts: Vec<((f64, f64), (f64, f64))> = doc.descendants().filter(|n| n.has_tag_name("line") && n.attribute("stroke-width") == Some("1.8") && n.attribute("y1") != n.attribute("y2") && n.attribute("x1") != n.attribute("x2")).map(|n| { let v = |k: &str| n.attribute(k).unwrap().parse::<f64>().unwrap(); ((v("x1"), v("y1")), (v("x2"), v("y2"))) }).collect();
            assert!(!heads.is_empty() && !shafts.is_empty(), "{l:?}");
            let dots: Vec<(f64, f64)> = doc.descendants().filter(|n| n.has_tag_name("circle") && n.attribute("r") == Some("4.4")).map(|n| (n.attribute("cx").unwrap().parse().unwrap(), n.attribute("cy").unwrap().parse().unwrap())).collect();
            for (txt, bx, _) in ink(&svg) {
                if !["A", "B", "C"].contains(&txt.as_str()) { continue; }
                for h in &heads { assert!(!(bx.0 < h.2 + 1.0 && h.0 - 1.0 < bx.2 && bx.1 < h.3 + 1.0 && h.1 - 1.0 < bx.3), "{txt} touches an arrowhead ({l:?})"); }
                for (a, z) in &shafts {
                    let hit = (0..=60).any(|k| { let t = k as f64 / 60.0; let q = (a.0 + (z.0 - a.0) * t, a.1 + (z.1 - a.1) * t); q.0 > bx.0 - 1.0 && q.0 < bx.2 + 1.0 && q.1 > bx.1 - 1.0 && q.1 < bx.3 + 1.0 });
                    assert!(!hit, "{txt} touches an arrow ({l:?})");
                }
            }
            // and no arrowhead is drawn over a lettered point's dot
            for h in &heads { for d in &dots { assert!(!(d.0 > h.0 - 5.0 && d.0 < h.2 + 5.0 && d.1 > h.1 - 5.0 && d.1 < h.3 + 5.0), "an arrowhead over a dot at {d:?} ({l:?})"); } }
        }
    }
}
