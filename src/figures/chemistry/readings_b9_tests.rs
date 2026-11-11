//! Tests for batch 9's reading additions (`readings_b9.rs`, `math::ruler_b9`), read off what is drawn.
use super::*;
use super::super::{measure, num};
use super::super::measure::vol_scale;
use crate::figures::interact::pick;
use crate::figures::{is_random_key, param_relevant, random_key, random_reading, registry, share, spec, start_params, FigureDef, Params, Value};

fn def(id: &str) -> FigureDef { registry().into_iter().find(|d| d.id == id).expect("registered") }
fn with(id: &str, list: &[(&'static str, Value)]) -> Params {
    let d = def(id);
    let mut p = start_params(&d);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }

fn attr(tag: &str, name: &str) -> Option<f64> {
    let k = format!(" {name}=\"");
    let i = tag.find(&k)? + k.len();
    tag[i..].split('"').next()?.parse().ok()
}
fn tags<'a>(svg: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name} ");
    svg.match_indices(&open).map(|(i, _)| { let e = svg[i..].find('>').unwrap(); &svg[i..i + e] }).collect()
}
/// (x, y, font size, text) of every text element.
fn texts(svg: &str) -> Vec<(f64, f64, f64, String)> {
    svg.match_indices("<text ").map(|(i, _)| {
        let e = svg[i..].find('>').unwrap();
        let tag = &svg[i..i + e];
        let close = svg[i + e..].find("</text>").unwrap();
        (attr(tag, "x").unwrap_or(0.0), attr(tag, "y").unwrap_or(0.0), attr(tag, "font-size").unwrap_or(0.0), svg[i + e + 1..i + e + close].to_string())
    }).collect()
}
fn on_grid(v: f64, dp: usize) -> bool { let q = 10f64.powi(dp as i32); ((v * q).round() - v * q).abs() < 1e-6 }

/// A saved figure and its share link redraw `p` exactly.
fn kept(id: &str, p: &Params) {
    let d = def(id);
    let svg = d.svg(p);
    let q = spec::params_from_spec(&d, &spec::spec_for(id, p));
    assert_eq!(d.svg(&q), svg, "{id}: a saved figure redraws the random reading");
    let link = share::encode(id, p, &start_params(&d));
    let (_, ch) = share::decode(&link).unwrap();
    assert_eq!(d.svg(&share::link_params(&d, ch, share::version(&link))), svg, "{id}: its link redraws it");
}

// ---------------------------------------------------------------------------------------------
// Random readings
// ---------------------------------------------------------------------------------------------

/// Every figure of the track has a Random button by its reading, with its own unit and tip.
#[test]
fn every_reading_figure_has_a_random_button() {
    for (id, key, unit) in [
        ("temperature-reading", "temperature", Some("°C")), ("volume-reading", "reading", None), ("gas-syringe", "volume", Some("cm³")),
        ("volume-by-displacement", "after", Some("mL")), ("micropipette-reading", "volume", None), ("spring-scale", "reading", None), ("ruler-reading", "length", Some("cm")),
    ] {
        assert_eq!(random_key(id), Some(key), "{id}");
        assert!(is_random_key(id, key));
        assert!((def(id).params)().iter().any(|d| d.key == key && matches!(d.kind, crate::figures::Kind::Float { .. })), "{id}: {key} is a number box");
        let p = start_params(&def(id));
        assert_eq!(reading_unit(id, &p).as_deref(), unit, "{id}");
        // the opening state's Random changes the figure
        let mut q = p.clone();
        assert!(random_reading(id, &mut q, 1));
        assert_ne!(def(id).svg(&q), def(id).svg(&p), "{id}: Random changes the drawing");
    }
    assert!(!is_random_key("volume-by-displacement", "before"), "one button for the pair");
    assert_eq!(reading_unit("temperature-reading", &with("temperature-reading", &[s("unit", "Kelvin (K)")])).as_deref(), Some("K"));
    assert_eq!(reading_unit("gas-syringe", &with("gas-syringe", &[s("unit", "mL")])).as_deref(), Some("mL"));
    assert_eq!(reading_unit("volume-by-displacement", &with("volume-by-displacement", &[s("unit", "cm³")])).as_deref(), Some("cm³"));
    assert_eq!(reading_unit("ruler-reading", &with("ruler-reading", &[s("marks", "1/4 in")])).as_deref(), Some("in"));
    assert_eq!(random_tip("volume-by-displacement"), "Set random before and after readings");
    assert_eq!(random_tip("temperature-reading"), "Set a random reading");
}

/// The thermometer's random temperature: the glass one's scale to 0.1, the probe's −50 to 150 °C
/// (in each unit) to its places; varied; keyed as drawn; kept by a saved figure and a link; never
/// printed on the student copy.
#[test]
fn random_temperatures_are_on_the_thermometer() {
    let id = "temperature-reading";
    for th in ["Liquid-in-glass", "Digital probe"] {
        for unit in ["Celsius (°C)", "Kelvin (K)", "Fahrenheit (°F)"] {
            for places in ["0", "0.1", "0.01"] {
                let mut seen = std::collections::BTreeSet::new();
                for seed in 0..200u64 {
                    let mut p = with(id, &[s("thermometer", th), s("unit", unit), s("places", places)]);
                    assert!(random_reading(id, &mut p, seed));
                    let t = p.f("temperature");
                    let probe = th == "Digital probe";
                    let dp = if probe { probe_dp(&p) } else { 1 };
                    let (lo, hi) = if probe { probe_range(&p) } else { temp_scale(&p) };
                    assert!(t >= lo - 1e-9 && t <= hi + 1e-9 && on_grid(t, dp), "{th} {unit} {places}: {t}");
                    seen.insert((t * 100.0).round() as i64);
                    let sym = temp_sym(&p);
                    let student = def(id).svg(&p);
                    assert!(!student.contains("Temperature:"), "no key on the student copy");
                    if probe {
                        // the display shows it to its places
                        assert!(texts(&student).iter().any(|t2| t2.3 == num(t, dp) && t2.2 == 38.0), "{th} {unit} {places}: display {}", num(t, dp));
                    }
                    p.set_b("answer", true);
                    let key = def(id).svg(&p);
                    assert!(key.contains(&format!("Temperature: {} {sym}", num(t, dp))), "{th} {unit} {places}: key for {t}");
                    if seed % 50 == 0 { kept(id, &p); }
                }
                assert!(seen.len() > 100, "{th} {unit} {places}: readings vary ({})", seen.len());
            }
        }
    }
}

/// The gas syringe's random volume: 0.1 to its full scale, one decimal place, keyed as drawn, on
/// both set-ups and sizes.
#[test]
fn random_gas_volumes_are_on_the_syringe() {
    let id = "gas-syringe";
    for show in ["Syringe only", "With flask and stand"] {
        for (size, max) in [("50 cm³", 50.0), ("100 cm³", 100.0)] {
            let mut seen = std::collections::BTreeSet::new();
            for seed in 0..200u64 {
                let mut p = with(id, &[s("show", show), s("size", size), b("answer", true)]);
                assert!(random_reading(id, &mut p, seed));
                let v = p.f("volume");
                assert!((0.1..=max).contains(&v) && on_grid(v, 1), "{show} {size}: {v}");
                seen.insert((v * 10.0).round() as i64);
                assert!(def(id).svg(&p).contains(&format!("Volume of gas: {} cm³", num(v, 1))));
                if seed % 50 == 0 { kept(id, &p); }
            }
            assert!(seen.len() > 120, "{show} {size}: {}", seen.len());
        }
    }
}

/// Random displacement readings: the after reading higher, both on the scale at the cylinder's
/// reading places, the object's volume at least three of its smallest marks; the marbles under the
/// liquid; the key giving both readings and their difference as drawn; for every size and marks.
#[test]
fn random_displacement_readings_are_on_the_cylinders() {
    let id = "volume-by-displacement";
    for size in ["10 mL", "25 mL", "50 mL", "100 mL"] {
        for marks in ["Automatic", "0.1 mL", "0.2 mL", "0.5 mL", "1 mL", "2 mL", "5 mL", "10 mL", "100 mL"] {
            let mut seen = std::collections::BTreeSet::new();
            for seed in 0..120u64 {
                let mut p = with(id, &[s("size", size), s("marks", marks), s("object", "Marbles"), i("count", (seed % 5) as i64 + 1), b("answer", true)]);
                assert!(random_reading(id, &mut p, seed));
                let max = size_ml(size);
                let (n0, m0) = cyl_marks(max);
                let (_, minor, _) = marks_from(&p, max, n0, m0);
                let dp = read_dp(minor);
                let (bf, af) = (p.f("before"), p.f("after"));
                let tag = format!("{size} {marks}: {bf} -> {af}");
                assert!(on_grid(bf, dp) && on_grid(af, dp), "places: {tag}");
                assert!(bf >= 0.4 * max - 1e-9 && af > bf && af <= 0.95 * max + 1e-9, "range: {tag}");
                // (marks wider than a tenth of the cylinder leave too few to read between; the volume is still on the scale)
                let fine = max / minor >= 10.0;
                if fine { assert!(af - bf >= (3.0 * minor).min(0.25 * max) - 1e-6, "the object's volume is readable: {tag}"); }
                seen.insert(((bf * 100.0) as i64, (af * 100.0) as i64));
                let svg = def(id).svg(&p);
                assert!(svg.contains(&format!("Before: {} mL · After: {} mL · Object: {} mL", num(bf, dp), num(af, dp), num(af - bf, dp))), "key: {tag}");
                // the marbles lie under the after cylinder's liquid (its 0 mark is at y 472, the top at 72)
                let level = 472.0 - af / max * 400.0;
                let marbles: Vec<(f64, f64)> = tags(&svg, "circle").iter().filter(|t| attr(t, "cx").unwrap() > 172.0).map(|t| (attr(t, "cy").unwrap(), attr(t, "r").unwrap())).collect();
                assert_eq!(marbles.len(), p.i("count") as usize, "{tag}");
                for (cy, r) in marbles { assert!(cy - r > level, "a marble above the liquid: {tag}"); }
                if seed % 40 == 0 { kept(id, &p); }
            }
            let max = size_ml(size);
            let minor = { let (n0, m0) = cyl_marks(max); marks_from(&with(id, &[s("size", size), s("marks", marks)]), max, n0, m0).1 };
            assert!(seen.len() > if max / minor >= 10.0 { 10 } else { 1 }, "{size} {marks}: {}", seen.len());
        }
    }
}

/// The existing Random buttons (volume, micropipette, spring scale) stay on their instruments and
/// keep their reading in a saved figure and a link, keyed as drawn.
#[test]
fn the_older_random_readings_are_kept_and_keyed() {
    for seed in 0..60u64 {
        let mut p = with("spring-scale", &[b("answer", true)]);
        random_reading("spring-scale", &mut p, seed);
        assert!(def("spring-scale").svg(&p).contains(&format!("Force: {:.2} N", p.f("reading"))));
        kept("spring-scale", &p);
        let mut p = with("micropipette-reading", &[b("answer", true)]);
        random_reading("micropipette-reading", &mut p, seed);
        kept("micropipette-reading", &p);
        for ins in ["Graduated cylinder", "Buret", "Beaker"] {
            let mut p = with("volume-reading", &[s("instrument", ins), b("answer", true)]);
            random_reading("volume-reading", &mut p, seed);
            let (max, _, minor, _) = vol_scale(&p);
            let r = p.f("reading");
            assert!((0.0..=max).contains(&r) && on_grid(r, read_dp(minor)), "{ins}: {r}");
            assert!(def("volume-reading").svg(&p).contains(&format!("Reading: {} mL", num(r, read_dp(minor)))), "{ins}: {r}");
            kept("volume-reading", &p);
        }
    }
}

/// The evidence tokens: the opening state's Random with seed 1, printed for the report.
#[test]
fn evidence_tokens() {
    for id in ["temperature-reading", "volume-reading", "gas-syringe", "volume-by-displacement", "micropipette-reading", "spring-scale", "ruler-reading"] {
        let d = def(id);
        let p0 = start_params(&d);
        let mut p = p0.clone();
        random_reading(id, &mut p, 1);
        let changed: Vec<String> = p.0.iter().filter(|(k, v)| p0.0.get(*k) != Some(*v)).map(|(k, v)| format!("{k}={v:?}")).collect();
        eprintln!("EVIDENCE {id}: {}", changed.join(", "));
        assert!(!changed.is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// The digital probe to 2 decimal places
// ---------------------------------------------------------------------------------------------

/// "0.01" shows and keys hundredths; each of the three places draws differently from the others;
/// the places control shows exactly where it changes the drawing (the probe, not the glass one).
#[test]
fn the_probe_shows_two_decimal_places() {
    let id = "temperature-reading";
    let d = def(id);
    let svgs: Vec<String> = ["0", "0.1", "0.01"].iter().map(|pl| d.svg(&with(id, &[s("thermometer", "Digital probe"), f("temperature", 23.456), s("places", pl), b("answer", true)]))).collect();
    assert!(svgs[2].contains(">23.46<") && svgs[2].contains("Temperature: 23.46 °C"), "hundredths shown and keyed");
    assert!(svgs[1].contains(">23.5<") && svgs[0].contains(">23<"));
    assert!(svgs[0] != svgs[1] && svgs[1] != svgs[2] && svgs[0] != svgs[2]);
    for th in ["Liquid-in-glass", "Digital probe"] {
        let p = with(id, &[s("thermometer", th), f("temperature", 23.456)]);
        let renders: std::collections::HashSet<String> = ["0", "0.1", "0.01"].iter().map(|pl| { let mut q = p.clone(); q.set_s("places", pl); d.svg(&q) }).collect();
        let changes = renders.len() > 1;
        assert_eq!(param_relevant(id, "places", &p), changes, "{th}: places shown exactly where it changes the drawing");
    }
    // the ends of the probe's range at 2 places: the display stays inside the screen (x 80 to 280)
    for (unit, t) in [("Celsius (°C)", -50.0), ("Celsius (°C)", 400.0), ("Fahrenheit (°F)", -58.0), ("Kelvin (K)", 423.15)] {
        let svg = d.svg(&with(id, &[s("thermometer", "Digital probe"), s("unit", unit), f("temperature", t), s("places", "0.01")]));
        let shown = texts(&svg).into_iter().find(|t2| t2.2 == 38.0).unwrap();
        // 38 px digits are at most 0.6 em wide
        let w = shown.3.chars().count() as f64 * 0.6 * 38.0;
        assert!(shown.0 - w >= 80.0, "{unit} {t}: {} fits the screen", shown.3);
    }
    let p = with(id, &[s("thermometer", "Digital probe"), s("places", "0.01")]);
    assert_eq!(measure::range_note(id, &p).unwrap(), "From -50 to 400 °C, to 2 decimal places.");
}

// ---------------------------------------------------------------------------------------------
// The 600 mL beaker
// ---------------------------------------------------------------------------------------------

fn beaker(list: &[(&'static str, Value)]) -> Params {
    let mut v = vec![s("instrument", "Beaker"), s("size", "600 mL"), s("mag_show", "Instrument only")];
    v.extend(list.iter().cloned());
    with("volume-reading", &v)
}

/// The 600 mL beaker: marks every 50 mL numbered every 100 mL up to 500 mL, the glass holding 600
/// (its height maps 0 to 600, so each mark sits at its own volume); the liquid's meniscus on the
/// mark of the volume it reads; the key, range note and summary; the drag handle on the drawn level
/// through the hit test; random readings in 5 mL steps to 500.
#[test]
fn the_600_ml_beaker_is_marked_to_500() {
    let id = "volume-reading";
    let d = def(id);
    assert!((d.params)().iter().any(|pd| pd.key == "size" && matches!(&pd.kind, crate::figures::Kind::Choice(o) if o.contains(&"600 mL"))));
    let p = beaker(&[f("reading", 300.0), b("answer", true)]);
    let svg = d.svg(&p);
    let (cx, xr, ybot, ytop) = (140.0, 225.0, 470.0, 90.0);
    let y_of = |v: f64| ybot - v / 600.0 * (ybot - ytop);
    // ticks on the right wall: long ones every 100 (0 to 500), half ones at the fifties between
    let ticks: Vec<(f64, f64)> = tags(&svg, "line").iter().filter(|t| attr(t, "x1") == Some(xr) && attr(t, "y1") == attr(t, "y2")).map(|t| (attr(t, "y1").unwrap(), xr - attr(t, "x2").unwrap())).collect();
    let long: Vec<f64> = ticks.iter().filter(|t| (t.1 - 22.0).abs() < 1e-6).map(|t| t.0).collect();
    let half: Vec<f64> = ticks.iter().filter(|t| (t.1 - 15.0).abs() < 1e-6).map(|t| t.0).collect();
    assert_eq!(ticks.len(), 11, "{ticks:?}");
    for (k, y) in long.iter().enumerate() { assert!((y - y_of(100.0 * k as f64)).abs() < 0.01, "long mark {k}: {y}"); }
    for (k, y) in half.iter().enumerate() { assert!((y - y_of(50.0 + 100.0 * k as f64)).abs() < 0.01, "half mark {k}: {y}"); }
    let nums: Vec<String> = texts(&svg).into_iter().filter(|t| t.0 == xr + 12.0).map(|t| t.3).collect();
    assert_eq!(nums, vec!["100", "200", "300", "400", "500", "mL"]);
    // the top 100 mL is unmarked, below the rim
    assert!(long.iter().all(|y| *y > y_of(500.0) - 0.01) && y_of(500.0) - (ytop - 30.0) > 60.0);
    // the meniscus's lowest point sits on the 300 mark
    assert!(svg.contains(&format!("{cx:.2},{:.2}", y_of(300.0))), "meniscus at 300");
    assert!(svg.contains("Reading: 300 mL"));
    assert_eq!(measure::range_note(id, &p).unwrap(), "From 0 to 500 mL, to whole numbers.");
    assert_eq!(measure::skin_summary(id, "Instrument", &p).unwrap(), "600 mL beaker");
    assert_eq!(measure::skin_summary(id, "Scale", &p).unwrap(), "50 mL marks, numbered every 100 mL");
    // a reading past the last mark is read as 500, the most the marks show
    assert!(d.svg(&beaker(&[f("reading", 580.0), b("answer", true)])).contains("Reading: 500 mL"));
    // the drag handle sits on the drawn level and sets the reading under the pointer
    let hs = d.handles(&p, crate::figures::svg_size(&svg));
    let ix = pick(&hs, (cx, y_of(300.0)), 1.0).expect("a handle on the liquid level");
    assert_eq!(hs[ix].tip, "Drag the liquid level to set the reading");
    let mut q = p.clone();
    (hs[ix].drag.as_ref().unwrap())(&mut q, (cx, y_of(450.0)));
    assert!((q.f("reading") - 450.0).abs() < 0.6, "{}", q.f("reading"));
    (hs[ix].drag.as_ref().unwrap())(&mut q, (cx, ytop - 20.0));
    assert!((q.f("reading") - 500.0).abs() < 1e-9, "the drag stops at the last mark: {}", q.f("reading"));
    // random readings: 5 to 500 mL in 5 mL steps (a tenth of the 50 mL marks), varied, keyed, kept
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..300u64 {
        let mut r = beaker(&[b("answer", true)]);
        assert!(random_reading(id, &mut r, seed));
        let v = r.f("reading");
        assert!((5.0..=500.0).contains(&v) && (v / 5.0 - (v / 5.0).round()).abs() < 1e-9, "{v}");
        seen.insert(v as i64);
        assert!(d.svg(&r).contains(&format!("Reading: {v} mL")));
        if seed % 60 == 0 { kept(id, &r); }
    }
    assert!(seen.len() > 60);
    // the other instruments at 600 mL: a cylinder with 5 mL marks numbered every 50, the buret its usual 50 mL
    let c = with(id, &[s("size", "600 mL")]);
    assert_eq!(vol_scale(&c), (600.0, 50.0, 5.0, true));
    assert_eq!(vol_scale(&with(id, &[s("size", "600 mL"), s("instrument", "Buret")])).0, 50.0);
}

/// The 600 mL beaker at the ends of its controls draws whole: empty and full, every mark and number
/// choice, the magnifier beside and alone, both units, and a long title, with no number off the page
/// and no number on another.
#[test]
fn the_600_ml_beaker_at_its_ends() {
    let d = def("volume-reading");
    for reading in [0.0, 5.0, 500.0, 1000.0] {
        for marks in ["Automatic", "1 mL", "10 mL", "100 mL"] {
            for numbered in ["Automatic", "50 mL", "200 mL", "No numbers"] {
                for mag in ["Instrument only", "Instrument and magnifier", "Magnifier only"] {
                    let p = beaker(&[f("reading", reading), s("marks", marks), s("numbered", numbered), s("mag_show", mag), s("unit", "cm³"), b("answer", true), s("title", "A long title for the 600 mL beaker on the test")]);
                    let svg = d.svg(&p);
                    let tag = format!("{reading} {marks} {numbered} {mag}");
                    assert!(svg.len() < 400_000 && !svg.contains("NaN") && !svg.contains("inf"), "{tag}");
                    let (w, h) = crate::figures::svg_size(&svg);
                    let ts = texts(&svg);
                    if mag == "Instrument only" {
                        // (the magnifier's copy is drawn scaled inside its clipped lens, so only the
                        // instrument's own text is placed in page units)
                        for t in &ts { assert!(t.0 >= 0.0 && t.0 <= w && t.1 > 0.0 && t.1 <= h && t.2 >= 8.0, "{tag}: {t:?} off the page or too small"); }
                        let nums: Vec<&(f64, f64, f64, String)> = ts.iter().filter(|t| t.0 == 237.0 && t.3 != "cm³").collect();
                        for pair in nums.windows(2) { assert!((pair[0].1 - pair[1].1).abs() >= 12.0, "{tag}: numbers on numbers {pair:?}"); }
                        assert!(nums.iter().all(|t| t.3.parse::<f64>().is_ok_and(|v| v <= 500.0)), "{tag}: a number past the last mark");
                    }
                }
            }
        }
    }
}
