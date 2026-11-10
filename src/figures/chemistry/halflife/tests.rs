//! Half-life graph tests: old figures frozen, and the batch 6 controls (custom half-life unit,
//! counts per minute, guide labels, axis titles, ranges, numbering and minor gridlines) checked
//! against the physics and against what is drawn.
use super::*;
use crate::figures::{registry, Params, Value};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("half-life-graph is registered") }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.into())) }
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
/// The opening state with `list` set on top.
fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = crate::figures::start_params(&d);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
/// FNV-1a, so the hashes in the fixture are the same in any build.
fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
    format!("{h:016x}")
}

/// Half-life graphs saved and shared before batch 6 (every old choice and switch, alone and with a
/// few others): their saved spec and share link with the hash of what they drew, written once from
/// main e0d5cd4 with VERSHION_HL_OLD_BLESS=1. The specs and links are those old ones; 67 hashes were
/// renewed once in batch 6 for one intended change only: the answer key gave a listed isotope's
/// half-life rounded to whole years or switched units (Cs-137 "30 years", F-18 "1.83 hours") where the
/// curve is drawn with the listed 30.1 years and 110 minutes, and now states the listed value.
fn old_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
    let mut opts: Vec<(&'static str, Value)> = vec![];
    for c in CHOICES { opts.push(s("iso1", c)); }
    for y in Y_KINDS { opts.push(s("y", y)); }
    for u in UNITS { opts.push(s("unit", u)); }
    for g in 0..=4 { opts.push(i("guides", g)); }
    opts.extend([
        i("count", 2), i("count", 3), f("span", 1.0), f("span", 2.5), f("span", 8.0), f("amount", 50.0), f("amount", 1e12), f("amount", 0.001),
        b("curves", false), b("dots", false), b("daughter", true), b("hide_names", true), s("title", "Decay of carbon-14"),
        b("show_answers", false), s("style", "Print"), s("label_size", "Large"), s("label_size", "Small"),
        s("custom_name", "Zed-1"), f("custom_half", 3.0), f("custom_half", 0.001), f("custom_half", 1e9),
    ]);
    for o in &opts {
        cases.push(vec![o.clone()]);
        cases.push(vec![i("count", 3), s("iso2", "Custom"), s("iso3", "Fluorine-18 (110 minutes)"), b("daughter", true), o.clone()]);
        cases.push(vec![s("y", "Activity"), f("amount", 800.0), s("unit", "Days"), s("iso1", "Iodine-131 (8.02 days)"), i("guides", 4), o.clone()]);
    }
    cases.push(vec![s("iso1", "Custom"), f("custom_half", 3.0), s("custom_name", "Zed-1"), s("y", "Mass remaining"), f("amount", 50.0), b("hide_names", true), s("label_size", "Large")]);
    cases
}

fn old_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/half_life_old.txt") }

#[test]
fn old_saved_figures_and_links_render_as_before() {
    use crate::figures::{share, spec, start_params};
    let d = def();
    if std::env::var("VERSHION_HL_OLD_BLESS").is_ok_and(|v| v == "1") {
        let mut out = String::new();
        for c in old_cases() {
            let p = with(&c);
            let saved = serde_json::to_string(&spec::spec_for(ID, &p)).unwrap();
            let link = share::encode(ID, &p, &start_params(&d));
            out.push_str(&format!("{}\t{saved}\t{link}\n", fnv(&d.svg(&p))));
        }
        std::fs::write(old_fixture(), out).unwrap();
        return;
    }
    let text = std::fs::read_to_string(old_fixture()).expect("tests/fixtures/half_life_old.txt");
    let mut n = 0;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let mut it = line.split('\t');
        let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
        let p = spec::params_from_spec(&d, &sp);
        assert_eq!(fnv(&d.svg(&p)), hash, "an old saved figure draws differently: {saved}");
        let (id, changed) = share::decode(link).unwrap();
        assert_eq!(id, ID);
        let p = share::link_params(&d, changed, share::version(link));
        assert_eq!(fnv(&d.svg(&p)), hash, "an old share link draws differently: {link}");
        n += 1;
    }
    assert!(n >= 150, "only {n} old cases");
}

// ---------------------------------------------------------------------------------------------
// Batch 6: reading what is drawn
// ---------------------------------------------------------------------------------------------

fn svg_of(list: &[(&'static str, Value)]) -> String { def().svg(&with(list)) }

fn attr(tag: &str, name: &str) -> Option<f64> {
    let k = format!(" {name}=\"");
    let i = tag.find(&k)? + k.len();
    tag[i..].split('"').next()?.parse().ok()
}
/// (x, y, font size, text, anchor) of every text in `svg` (rotated titles at their translate point).
fn texts(svg: &str) -> Vec<(f64, f64, f64, String, String)> {
    let mut out = vec![];
    for (i, _) in svg.match_indices("<text ") {
        let end = i + svg[i..].find('>').unwrap();
        let tag = &svg[i..end];
        let body = &svg[end + 1..end + 1 + svg[end + 1..].find("</text>").unwrap()];
        let anchor = tag.split("text-anchor=\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("start").to_string();
        let (x, y) = match tag.split("translate(").nth(1) {
            Some(t) => { let mut it = t.split([' ', ')']); (it.next().unwrap().parse().unwrap(), it.next().unwrap().parse().unwrap()) }
            None => (attr(tag, "x").unwrap_or(0.0), attr(tag, "y").unwrap_or(0.0)),
        };
        out.push((x, y, attr(tag, "font-size").unwrap_or(0.0), body.replace("&amp;", "&"), anchor));
    }
    out
}
fn polylines(svg: &str) -> Vec<Vec<(f64, f64)>> {
    svg.match_indices("<polyline points=\"").map(|(i, k)| {
        let rest = &svg[i + k.len()..];
        rest[..rest.find('"').unwrap()].split(' ').map(|p| { let (x, y) = p.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect()
    }).collect()
}
/// The x of every dashed half-life guide.
fn guides(svg: &str) -> Vec<f64> {
    svg.match_indices("<line ").filter_map(|(i, _)| {
        let tag = &svg[i..i + svg[i..].find("/>").unwrap()];
        tag.contains("stroke-dasharray=\"5 4\"").then(|| attr(tag, "x1").unwrap())
    }).collect()
}
/// Lines (x1, y1, x2, y2) of one stroke colour.
fn lines_of(svg: &str, stroke: &str) -> Vec<(f64, f64, f64, f64)> {
    svg.match_indices("<line ").filter_map(|(i, _)| {
        let tag = &svg[i..i + svg[i..].find("/>").unwrap()];
        tag.contains(&format!("stroke=\"{stroke}\"")).then(|| (attr(tag, "x1").unwrap(), attr(tag, "y1").unwrap(), attr(tag, "x2").unwrap(), attr(tag, "y2").unwrap()))
    }).collect()
}
fn num(s: &str) -> Option<f64> { s.replace('\u{2212}', "-").replace(',', "").trim().parse().ok() }

const BOTTOM: f64 = 330.0 - 52.0;
/// The x-axis numbers as drawn: (pixel x, value).
fn x_numbers(svg: &str) -> Vec<(f64, f64)> {
    texts(svg).into_iter().filter(|t| t.4 == "middle" && (t.1 - (BOTTOM + 8.0 + 12.6)).abs() < 0.01).filter_map(|t| num(&t.3).map(|v| (t.0, v))).collect()
}
/// The y-axis numbers as drawn: (pixel y of the line they name, value).
fn y_numbers(svg: &str) -> Vec<(f64, f64)> {
    let tx = texts(svg);
    let ends: Vec<_> = tx.iter().filter(|t| t.4 == "end" && t.0 < 200.0).collect();
    let x = ends.iter().map(|t| t.0).fold(f64::MIN, f64::max);
    ends.iter().filter(|t| (t.0 - x).abs() < 0.01).filter_map(|t| num(&t.3).map(|v| (t.1 - 12.0 * 0.35, v))).collect()
}
/// A linear pixel-to-value map through the first and last numbers on an axis.
fn map(nums: Vec<(f64, f64)>) -> impl Fn(f64) -> f64 {
    let (a, b) = (nums[0], nums[nums.len() - 1]);
    assert!(a.1 != b.1, "an axis needs two different numbers");
    move |px| a.1 + (px - a.0) / (b.0 - a.0) * (b.1 - a.1)
}
/// The curve's y at pixel x, by straight lines between its points.
fn curve_at(pts: &[(f64, f64)], x: f64) -> f64 {
    for w in pts.windows(2) {
        if (w[0].0 - x) * (w[1].0 - x) <= 0.0 {
            if (w[1].0 - w[0].0).abs() < 1e-9 { return w[0].1; }
            return w[0].1 + (x - w[0].0) / (w[1].0 - w[0].0) * (w[1].1 - w[0].1);
        }
    }
    panic!("x = {x} is off the curve");
}

/// Hand-worked cases: (params, the longest half-life in the axis unit, the starting value N₀). The
/// first curve drawn is isotope 1's, so each case lists that one first or as the longest.
/// Half-lives from the textbook values in the figure; a year is 365.25 days.
fn physics_cases() -> Vec<(Vec<(&'static str, Value)>, f64, f64)> {
    vec![
        (vec![], 5730.0, 100.0),
        (vec![s("iso1", "Iodine-131 (8.02 days)"), s("unit", "Days"), s("y", "Mass remaining"), f("amount", 80.0)], 8.02, 80.0),
        // 110 min shown over 5 half-lives is 9.2 h, so the fitted unit is hours: 110/60 h
        (vec![s("iso1", "Fluorine-18 (110 minutes)"), s("y", "Fraction remaining")], 110.0 / 60.0, 1.0),
        (vec![s("iso1", "Uranium-238 (4.47 billion years)"), s("y", "Number of nuclei"), f("amount", 1e6)], 4.47, 1e6),
        (vec![s("iso1", "Technetium-99m (6.01 hours)"), s("unit", "Minutes")], 6.01 * 60.0, 100.0),
        (vec![s("iso1", "Phosphorus-32 (14.3 days)"), s("unit", "Hours")], 14.3 * 24.0, 100.0),
        (vec![s("iso1", "Cobalt-60 (5.27 years)"), s("unit", "Days")], 5.27 * 365.25, 100.0),
        // a custom isotope typed in seconds: 30 s over 5 half-lives is 2.5 min, fitted to minutes
        (vec![s("iso1", "Custom"), f("custom_half", 30.0), s("custom_unit", "Seconds")], 0.5, 100.0),
        (vec![s("iso1", "Custom"), f("custom_half", 2.0), s("custom_unit", "Hours"), s("unit", "Seconds")], 7200.0, 100.0),
        (vec![s("iso1", "Custom"), f("custom_half", 5.73), s("custom_unit", "Thousand years"), s("unit", "Years")], 5730.0, 100.0),
        (vec![s("iso1", "Custom"), f("custom_half", 3.0), s("custom_unit", "Days"), s("unit", "Days")], 3.0, 100.0),
        (vec![s("iso1", "Custom"), f("custom_half", 90.0), s("custom_unit", "Minutes"), s("unit", "Hours")], 1.5, 100.0),
        (vec![s("iso1", "Custom"), f("custom_half", 2.0), s("custom_unit", "Billion years"), s("unit", "Thousand years")], 2e6, 100.0),
        (vec![s("y", "Activity"), s("act_unit", "Counts per minute"), f("amount", 2400.0)], 5730.0, 2400.0),
        // the longest-lived of three sets the guides
        (vec![i("count", 3), s("iso2", "Iodine-131 (8.02 days)"), s("iso3", "Cesium-137 (30.1 years)")], 5730.0, 100.0),
        (vec![i("count", 2), s("iso1", "Strontium-90 (28.8 years)"), s("iso2", "Hydrogen-3 (12.3 years)")], 28.8, 100.0),
        // typed ranges
        (vec![b("fit", false), f("x_from", 0.0), f("x_to", 40000.0), f("x_by", 2000.0), f("y_from", 0.0), f("y_to", 100.0), f("y_by", 10.0)], 5730.0, 100.0),
        (vec![b("fit", false), f("x_from", 6000.0), f("x_to", 24000.0), f("x_by", 3000.0), f("y_from", 0.0), f("y_to", 60.0), f("y_by", 5.0)], 5730.0, 100.0),
        (vec![b("fit", false), s("y", "Mass remaining"), f("amount", 50.0), f("x_from", 0.0), f("x_to", 20000.0), f("x_by", 1000.0), f("y_from", 0.0), f("y_to", 50.0), f("y_by", 2.0), s("label_size", "Large")], 5730.0, 50.0),
    ]
}

/// N = N₀(½)^(t/T½): every curve point, read off the drawn axes, and each half-life guide at
/// t = k·T½ meets the curve at N₀/2^k. Read from the axis numbers as drawn, so this checks the
/// curve, the guides, the time unit and the numbering together.
#[test]
fn curves_and_guides_follow_the_decay_law() {
    for (case, half, n0) in physics_cases() {
        for extra in [vec![i("guides", 4)], vec![i("guides", 4), s("label_size", "Large"), s("minor_grid", "4 per block")]] {
            let mut list = case.clone();
            list.extend(extra);
            let svg = svg_of(&list);
            let (xn, yn) = (x_numbers(&svg), y_numbers(&svg));
            assert!(xn.len() >= 2 && yn.len() >= 2, "{list:?}: axis numbers {xn:?} {yn:?}");
            let (tx, vy) = (map(xn.clone()), map(yn.clone()));
            let px_t = (xn[xn.len() - 1].1 - xn[0].1) / (xn[xn.len() - 1].0 - xn[0].0);
            let px_v = (yn[yn.len() - 1].1 - yn[0].1) / (yn[0].0 - yn[yn.len() - 1].0);
            let curve = &polylines(&svg)[0];
            // every point of the curve
            for &(x, y) in curve {
                let t = tx(x);
                let want = n0 * 0.5f64.powf(t / half);
                assert!((vy(y) - want).abs() <= 0.02 * px_v.abs() + 1e-9 * n0, "{list:?}: at t = {t} the curve reads {} not {want}", vy(y));
            }
            // the guides: at k half-lives, meeting the curve at N₀/2^k
            let gs = guides(&svg);
            assert!(!gs.is_empty(), "{list:?}: no guides");
            for g in gs {
                let t = tx(g);
                let k = (t / half).round();
                assert!(k >= 1.0 && k <= 4.0 && (t - k * half).abs() <= 0.02 * px_t.abs(), "{list:?}: a guide at t = {t}, not a whole number of half-lives of {half}");
                let at = vy(curve_at(curve, g));
                let want = n0 / 2f64.powf(k);
                assert!((at - want).abs() <= 0.6 * px_v.abs(), "{list:?}: guide {k} meets the curve at {at}, not N0/2^k = {want}");
            }
        }
    }
}

/// The guides labelled with their times: each time is the guide's position on the drawn axis and k·T½
/// in the axis unit (to the 4 figures written), with the axis unit's short name.
#[test]
fn guide_times_agree_with_the_drawn_guides() {
    for (case, half, _) in physics_cases() {
        for size in ["Medium", "Large", "Small"] {
            let mut list = case.clone();
            list.extend([i("guides", 4), s("guide_labels", "Times"), s("label_size", size)]);
            let p = with(&list);
            let unit = axis_unit(&p);
            let svg = def().svg(&p);
            let tx = map(x_numbers(&svg));
            let gs = guides(&svg);
            let labels: Vec<_> = texts(&svg).into_iter().filter(|t| t.3.ends_with(&format!(" {}", short_unit(unit))) && t.1 < 100.0).collect();
            assert_eq!(labels.len(), gs.len(), "{list:?}: one time per guide");
            for (g, l) in gs.iter().zip(&labels) {
                let t = tx(*g);
                let k = (t / half).round();
                let written = l.3.rsplit_once(' ').unwrap().0;
                assert_eq!(written, time_num(k * half), "{list:?}: guide {k} says {written}");
                let v = num(written).unwrap_or_else(|| { let (m, e) = written.split_once(" \u{00d7} 10").unwrap(); m.parse::<f64>().unwrap() * 10f64.powi(e.chars().map(|c| match c { '\u{207b}' => '-', '\u{2070}' => '0', '\u{00b9}' => '1', '\u{00b2}' => '2', '\u{00b3}' => '3', c => (c as u32 - 0x2074 + '4' as u32) as u8 as char }).collect::<String>().parse().unwrap()) });
                assert!((v - t).abs() <= 0.002 * t.abs() + 1e-9, "{list:?}: guide at {t} labelled {v}");
            }
        }
    }
    // the short names
    for (u, s_) in [("Seconds", "s"), ("Minutes", "min"), ("Hours", "h"), ("Days", "d"), ("Years", "yr"), ("Thousand years", "kyr"), ("Billion years", "Gyr")] {
        assert_eq!(short_unit(u), s_);
    }
    assert_eq!(time_num(5730.0), "5,730");
    assert_eq!(time_num(24.06), "24.06");
    assert_eq!(time_num(0.5), "0.5");
    assert_eq!(time_num(1.8333333), "1.833");
    assert_eq!(time_num(17190.0), "17,190");
}

/// Units by hand: a minute is 60 s, an hour 3,600 s, a day 86,400 s, a year 365.25 days. The same
/// half-life typed in different units draws the same curve, and a typed one matches the isotope
/// with that half-life.
#[test]
fn a_custom_half_life_in_any_unit_draws_the_same_decay() {
    assert_eq!(unit_scale("Seconds"), 1.0);
    assert_eq!(unit_scale("Minutes"), 60.0);
    assert_eq!(unit_scale("Hours"), 3600.0);
    assert_eq!(unit_scale("Days"), 86_400.0);
    assert_eq!(unit_scale("Years"), 365.25 * 86_400.0);
    assert_eq!(unit_scale("Thousand years"), 1000.0 * 365.25 * 86_400.0);
    assert_eq!(unit_scale("Billion years"), 1e9 * 365.25 * 86_400.0);
    let cu = |v: f64, u: &str| vec![s("iso1", "Custom"), f("custom_half", v), s("custom_unit", u)];
    let draw = |l: Vec<(&'static str, Value)>| { let svg = svg_of(&l); (polylines(&svg), guides(&svg), x_numbers(&svg)) };
    let same = [
        (cu(30.0, "Seconds"), cu(0.5, "Minutes")),
        (cu(30.0, "Seconds"), cu(1.0 / 120.0, "Hours")),
        (cu(36.0, "Hours"), cu(1.5, "Days")),
        (cu(1.0, "Years"), cu(365.25, "Days")),
        (cu(8.02, "Days"), vec![s("iso1", "Iodine-131 (8.02 days)")]),
        (cu(110.0, "Minutes"), vec![s("iso1", "Fluorine-18 (110 minutes)")]),
        (cu(5.73, "Thousand years"), vec![]),
        (cu(4.47, "Billion years"), vec![s("iso1", "Uranium-238 (4.47 billion years)")]),
        (cu(10.0, "Years"), vec![s("iso1", "Isotope X (10 years)")]),
    ];
    for (a, b_) in same {
        assert_eq!(draw(a.clone()), draw(b_.clone()), "{a:?} and {b_:?} should draw the same decay");
    }
    // a different unit changes the drawing (the control is live)
    // (on a fitted axis 10 days and 10 years both show 0 to 50, so fix the axis unit to see it)
    let mut a = cu(10.0, "Days");
    a.push(s("unit", "Years"));
    let mut b_ = cu(10.0, "Years");
    b_.push(s("unit", "Years"));
    assert_ne!(draw(a), draw(b_));
    assert_ne!(svg_of(&cu(10.0, "Days")), svg_of(&cu(10.0, "Years")));
    // the key states the half-life as typed, with λ = ln 2 / T½ per that unit
    let key = |l: Vec<(&'static str, Value)>| texts(&svg_of(&l)).into_iter().map(|t| t.3).find(|t| t.contains("\u{03bb} = ")).unwrap();
    assert_eq!(key(cu(30.0, "Seconds")), "Isotope Y: T\u{00bd} = 30 seconds, \u{03bb} = ln 2 / T\u{00bd} = 0.023 per second");
    assert_eq!(key(cu(1.0, "Days")), "Isotope Y: T\u{00bd} = 1 day, \u{03bb} = ln 2 / T\u{00bd} = 0.693 per day");
    assert_eq!(key(cu(2.5, "Hours")), "Isotope Y: T\u{00bd} = 2.5 hours, \u{03bb} = ln 2 / T\u{00bd} = 0.277 per hour");
    assert_eq!(key(cu(5.73, "Thousand years")), "Isotope Y: T\u{00bd} = 5.73 thousand years, \u{03bb} = ln 2 / T\u{00bd} = 0.121 per thousand years");
    // in years, as before
    assert_eq!(key(cu(3.0, "Years")), "Isotope Y: T\u{00bd} = 3 years, \u{03bb} = ln 2 / T\u{00bd} = 0.231 per year");
    // ln 2 / 30 = 0.0231 per second, by hand
    assert!((std::f64::consts::LN_2 / 30.0 - 0.0231).abs() < 1e-4);
}

/// Activity in counts per minute: the same curve as in becquerels from the same starting number
/// (a counter catches a fixed share of the decays), titled in counts per minute.
#[test]
fn counts_per_minute_fall_along_the_activity_curve() {
    let bq = svg_of(&[s("y", "Activity"), f("amount", 2400.0)]);
    let cpm = svg_of(&[s("y", "Activity"), s("act_unit", "Counts per minute"), f("amount", 2400.0)]);
    assert_eq!(polylines(&bq), polylines(&cpm));
    assert!(cpm.contains(">Activity (counts per minute)</text>") && !cpm.contains("Activity (Bq)"));
    assert!(bq.contains(">Activity (Bq)</text>"));
    let vy = map(y_numbers(&cpm));
    assert!((vy(polylines(&cpm)[0][0].1) - 2400.0).abs() < 1.0, "starts at the typed count rate");
    // the unit only matters for activity
    assert_eq!(svg_of(&[s("act_unit", "Counts per minute")]), svg_of(&[]));
    assert!(!relevant("act_unit", &with(&[])) && relevant("act_unit", &with(&[s("y", "Activity")])));
}

/// The guide labels: times on the answer copy (blank lines on the student copy, since the first
/// time is the half-life), blank lines, or none; the guides stay.
#[test]
fn guide_labels_as_times_blank_lines_or_none() {
    let times = svg_of(&[i("guides", 4), s("guide_labels", "Times")]);
    for t in ["5,730 yr", "11,460 yr", "17,190 yr", "22,920 yr"] { assert!(times.contains(&format!(">{t}</text>")), "{t}"); }
    assert!(!times.contains(">T\u{00bd}</text>"));
    let tops = |svg: &str| lines_of(svg, INK).into_iter().filter(|l| l.1 < 60.0 && l.0 > 70.0 && (l.1 - l.3).abs() < 1e-9).count();
    for list in [vec![i("guides", 4), s("guide_labels", "Times"), b("show_answers", false)], vec![i("guides", 4), s("guide_labels", "Blank lines")], vec![i("guides", 4), s("guide_labels", "Blank lines"), b("show_answers", false)]] {
        let svg = svg_of(&list);
        assert_eq!(guides(&svg).len(), 4);
        assert_eq!(tops(&svg), 4, "{list:?}: a blank line per guide");
        // nothing written at the guides (the answer copy's key below the chart still gives T½)
        let top_text: Vec<String> = texts(&svg).into_iter().filter(|t| t.1 < 120.0 && t.0 > 70.0).map(|t| t.3).collect();
        assert!(top_text.is_empty(), "{list:?}: {top_text:?} written at the guides");
    }
    // the student copy, alt text included, gives no half-life away
    let student = svg_of(&[i("guides", 4), s("guide_labels", "Times"), b("show_answers", false)]);
    assert!(!student.contains("5,730") && !student.contains("ln 2"));
    let none = svg_of(&[i("guides", 4), s("guide_labels", "None")]);
    assert_eq!(guides(&none).len(), 4);
    assert_eq!(tops(&none), 0);
    assert!(!none.contains("T\u{00bd}<") && !none.contains(" yr<"));
}

/// Axis titles: the automatic ones, typed text, a blank line to write on, or none.
#[test]
fn axis_titles_as_text_a_blank_line_or_none() {
    let tx = |l: &[(&'static str, Value)]| texts(&svg_of(l)).into_iter().map(|t| t.3).collect::<Vec<_>>();
    let open = tx(&[]);
    assert!(open.contains(&"Time (years)".to_string()) && open.contains(&"Percent remaining (%)".to_string()));
    let typed = tx(&[s("xtitle", "Age of sample (years)"), s("ytitle", "Carbon-14 left (%)")]);
    assert!(typed.contains(&"Age of sample (years)".to_string()) && typed.contains(&"Carbon-14 left (%)".to_string()));
    assert!(!typed.contains(&"Time (years)".to_string()));
    for mode in ["Blank line", "None"] {
        let svg = svg_of(&[s("xtitle_mode", mode), s("ytitle_mode", mode), s("xtitle", "Age")]);
        let t: Vec<String> = texts(&svg).into_iter().map(|t| t.3).collect();
        assert!(!t.iter().any(|t| t.starts_with("Time") || t.starts_with("Percent") || t == "Age"), "{mode}: {t:?}");
        let ink = lines_of(&svg, INK);
        let x_blank = ink.iter().any(|l| (l.1 - 318.0).abs() < 0.01 && (l.3 - 318.0).abs() < 0.01 && l.2 - l.0 > 150.0);
        let y_blank = ink.iter().any(|l| (l.0 - 20.4).abs() < 0.01 && (l.2 - 20.4).abs() < 0.01 && l.3 - l.1 > 150.0);
        assert_eq!((x_blank, y_blank), (mode == "Blank line", mode == "Blank line"), "{mode}");
    }
    // a very long typed title shrinks, never under 8 px as drawn, and stays on the page
    for size in ["Large", "Medium", "Small"] {
        let long = "A very long axis title that a teacher might type for a worksheet about decay and dating ".repeat(4);
        let svg = svg_of(&[s("xtitle", &long), s("ytitle", &long), s("label_size", size)]);
        let lf = svg::label_factor(size);
        for t in texts(&svg).into_iter().filter(|t| t.3.starts_with("A very")) {
            assert!(t.2 >= 8.0 - 1e-9, "{size}: {} px", t.2);
            let wdt = text_w(&t.3, t.2);
            assert!(wdt <= 640.0, "{size}: {wdt} px wide");
            let _ = lf;
        }
    }
}

/// Every new control changes the drawing from the opening state (the evidence tokens).
#[test]
fn every_new_control_changes_the_drawing() {
    let open = svg_of(&[]);
    for list in [
        vec![s("iso1", "Custom"), s("custom_unit", "Days")],
        vec![s("y", "Activity"), s("act_unit", "Counts per minute")],
        vec![s("guide_labels", "Times")],
        vec![s("guide_labels", "Blank lines")],
        vec![s("guide_labels", "None")],
        vec![s("xtitle_mode", "Blank line")],
        vec![s("ytitle_mode", "None")],
        vec![s("xtitle", "Age (years)")],
        vec![b("fit", false)],
        vec![b("fit", false), f("x_to", 40000.0)],
        vec![b("fit", false), f("y_from", 20.0)],
        vec![s("x_num", "Every 2nd line")],
        vec![s("y_num", "No numbers")],
        vec![s("minor_grid", "2 per block")],
        vec![s("minor_grid", "5 per block")],
    ] {
        assert_ne!(svg_of(&list), open, "{list:?} draws the opening state");
    }
}

/// Numbering every line, every 2nd, 5th or 10th line from the first, or none; the lines stay.
#[test]
fn axis_numbering_every_nth_line() {
    let base = [b("fit", false), f("x_from", 0.0), f("x_to", 30000.0), f("x_by", 1000.0), f("y_from", 0.0), f("y_to", 100.0), f("y_by", 5.0)];
    let ticks_of = |svg: &str| lines_of(svg, INK).into_iter().filter(|l| (l.1 - BOTTOM).abs() < 0.01 && (l.3 - BOTTOM - 6.0).abs() < 0.01).count();
    for (choice, n) in [("Every line", 1usize), ("Every 2nd line", 2), ("Every 5th line", 5), ("Every 10th line", 10)] {
        let mut l = base.to_vec();
        l.extend([s("x_num", choice), s("y_num", choice)]);
        let svg = svg_of(&l);
        assert_eq!(ticks_of(&svg), 31, "{choice}: every line still has its tick");
        let xs: Vec<f64> = x_numbers(&svg).into_iter().map(|t| t.1).collect();
        // 31 lines a thousand years apart are too close to number each one ("30000" is wider than a
        // block): thinned to a multiple of the choice, still from the first line
        let step = xs[1] - xs[0];
        assert!(xs[0] == 0.0 && step >= 2000.0 && (step / (n as f64 * 1000.0)).fract().abs() < 1e-9, "{choice}: {xs:?}");
        assert!(xs.windows(2).all(|w| (w[1] - w[0] - step).abs() < 1e-9), "{choice}: {xs:?}");
        let ys: Vec<f64> = y_numbers(&svg).into_iter().map(|t| t.1).collect();
        // 21 lines 10 px apart are too close to number each one: every 2nd at least
        let ystep = ys[1] - ys[0];
        assert!(ys[0] == 0.0 && ystep >= 10.0 && (ystep / (n as f64 * 5.0)).fract().abs() < 1e-9, "{choice}: {ys:?}");
        assert_eq!(ys.len(), (100.0 / ystep) as usize + 1, "{choice}: {ys:?}");
    }
    let mut l = base.to_vec();
    l.extend([s("x_num", "No numbers"), s("y_num", "No numbers")]);
    let svg = svg_of(&l);
    assert!(x_numbers(&svg).is_empty() && y_numbers(&svg).is_empty());
    assert_eq!(ticks_of(&svg), 31);
    // on the fitted axes too
    let svg = svg_of(&[s("x_num", "Every 2nd line"), s("y_num", "Every 5th line")]);
    assert_eq!(x_numbers(&svg).into_iter().map(|t| t.1).collect::<Vec<_>>(), vec![0.0, 20000.0]);
    assert_eq!(y_numbers(&svg).into_iter().map(|t| t.1).collect::<Vec<_>>(), vec![0.0, 100.0]);
}

/// Minor gridlines split each block 2, 4 or 5 ways (a faint line at each division), on both axes.
#[test]
fn minor_gridlines_split_each_block() {
    // the opening axes: 0 to 28,650 years numbered every 10,000; 0 to 100 % every 20
    for (choice, n) in [("2 per block", 2usize), ("4 per block", 4), ("5 per block", 5)] {
        let svg = svg_of(&[s("minor_grid", choice)]);
        let faint = lines_of(&svg, "#eceff4");
        let vert: Vec<f64> = faint.iter().filter(|l| l.0 == l.2).map(|l| l.0).collect();
        let horiz = faint.iter().filter(|l| l.1 == l.3).count();
        let tx = map(x_numbers(&svg));
        let want_x: Vec<f64> = (0..3).flat_map(|m| (1..n).map(move |j| m as f64 * 10000.0 + 10000.0 * j as f64 / n as f64)).filter(|v| *v < 28650.0).collect();
        assert_eq!(vert.len(), want_x.len(), "{choice}");
        for (x, w) in vert.iter().zip(&want_x) { assert!((tx(*x) - w).abs() < 30.0, "{choice}: {} not {w}", tx(*x)); }
        assert_eq!(horiz, 5 * (n - 1), "{choice}");
    }
    assert!(lines_of(&svg_of(&[]), "#eceff4").is_empty());
}

/// Typed ranges: the axes run from, to and count by as typed, the curves are cut at the frame,
/// and impossible ranges are repaired rather than drawn broken.
#[test]
fn typed_axis_ranges() {
    let svg = svg_of(&[b("fit", false), f("x_from", 2000.0), f("x_to", 20000.0), f("x_by", 2000.0), f("y_from", 10.0), f("y_to", 80.0), f("y_by", 10.0)]);
    let xs: Vec<f64> = x_numbers(&svg).into_iter().map(|t| t.1).collect();
    assert_eq!(xs, (1..=10).map(|k| k as f64 * 2000.0).collect::<Vec<_>>());
    let ys: Vec<f64> = y_numbers(&svg).into_iter().map(|t| t.1).collect();
    assert_eq!(ys, (1..=8).map(|k| k as f64 * 10.0).collect::<Vec<_>>());
    assert!(svg.contains("clip-path=\"url(#hlplot)\""));
    // the curve starts at the axis's left end, not at t = 0
    let tx = map(x_numbers(&svg));
    assert!((tx(polylines(&svg)[0][0].0) - 2000.0).abs() < 1.0);
    // to below from, count by 0, a count by too fine to draw: still an axis with at most 61 lines
    for list in [
        vec![b("fit", false), f("x_from", 500.0), f("x_to", 100.0)],
        vec![b("fit", false), f("x_by", 0.0), f("y_by", 0.0)],
        vec![b("fit", false), f("x_to", 1e9), f("x_by", 1e-9), f("y_to", 1e12), f("y_by", 1e-9)],
        vec![b("fit", false), f("x_from", -5000.0), f("x_to", -1000.0)],
        vec![b("fit", false), f("x_from", -1e30), f("x_to", 1e30), f("y_from", -1e30), f("y_to", 1e30)],
    ] {
        let svg = svg_of(&list);
        assert!(!svg.contains("NaN") && !svg.contains("inf"), "{list:?}");
        let ticks = lines_of(&svg, INK).into_iter().filter(|l| (l.1 - BOTTOM).abs() < 0.01 && (l.3 - BOTTOM - 6.0).abs() < 0.01).count();
        assert!((2..=61).contains(&ticks), "{list:?}: {ticks} lines");
        texts_fit(&svg, &format!("{list:?}"));
    }
}

/// No text off the page, under 8 px as drawn, or on top of other text.
fn texts_fit(svg: &str, what: &str) {
    let (w, _) = crate::figures::svg_size(svg);
    let tx = texts(svg);
    let boxes: Vec<(f64, f64, f64, f64, &str)> = tx.iter().filter(|t| !svg.contains(&format!("rotate(-90)\" font-size=\"{}\" text-anchor=\"middle\" font-family=\"Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif\" fill=\"{INK}\">{}<", t.2, t.3))).map(|t| {
        // digits about 0.55 em wide, text on average about 0.5 em
        let wd = t.3.chars().count() as f64 * t.2 * if t.3.chars().all(|c| c.is_ascii_digit() || ",.\u{2212}e".contains(c)) { 0.55 } else { 0.5 };
        let x0 = match t.4.as_str() { "middle" => t.0 - wd / 2.0, "end" => t.0 - wd, _ => t.0 };
        (x0, x0 + wd, t.1 - t.2 * 0.75, t.1 + t.2 * 0.2, t.3.as_str())
    }).collect();
    for t in &tx { assert!(t.2 >= 8.0 - 1e-9, "{what}: {:?} is {} px", t.3, t.2); }
    for b_ in &boxes { assert!(b_.0 >= -0.5 && b_.1 <= w + 0.5, "{what}: {:?} runs off the page ({:.1} to {:.1})", b_.4, b_.0, b_.1); }
    for (k, a) in boxes.iter().enumerate() {
        for c in &boxes[k + 1..] {
            let overlap = a.0 < c.1 - 0.5 && c.0 < a.1 - 0.5 && a.2 < c.3 - 0.5 && c.2 < a.3 - 0.5;
            assert!(!overlap, "{what}: {:?} and {:?} overlap", a.4, c.4);
        }
    }
}

/// The ends of every range together: label sizes, three isotopes, four guides with times, the
/// shortest and longest half-lives in the smallest and largest units, huge amounts, dense typed axes.
#[test]
fn crowded_and_extreme_figures_stay_readable() {
    for size in ["Large", "Medium", "Small"] {
        for list in [
            vec![i("count", 3), s("iso2", "Iodine-131 (8.02 days)"), s("iso3", "Cesium-137 (30.1 years)"), i("guides", 4), s("guide_labels", "Times"), f("span", 8.0), s("minor_grid", "5 per block")],
            vec![i("guides", 4), s("guide_labels", "Times"), f("span", 1.0)],
            vec![i("guides", 4), s("guide_labels", "Times"), s("iso1", "Uranium-238 (4.47 billion years)"), s("unit", "Seconds")],
            vec![i("guides", 4), s("guide_labels", "Times"), s("iso1", "Custom"), f("custom_half", 0.001), s("custom_unit", "Seconds")],
            vec![i("guides", 4), s("guide_labels", "Times"), s("iso1", "Custom"), f("custom_half", 1e9), s("custom_unit", "Billion years")],
            vec![i("guides", 4), s("guide_labels", "Times"), b("fit", false), f("x_from", 0.0), f("x_to", 1e7), f("x_by", 1e5), s("y", "Number of nuclei"), f("amount", 1e12), f("y_from", 0.0), f("y_to", 1e12), f("y_by", 1e9)],
            vec![i("guides", 4), s("guide_labels", "Times"), b("fit", false), f("x_from", 0.0), f("x_to", 30000.0), f("x_by", 100.0), f("y_from", 0.0), f("y_to", 100.0), f("y_by", 0.5)],
            vec![i("guides", 4), s("guide_labels", "Times"), b("fit", false), f("x_from", 0.0), f("x_to", 1e6), f("x_by", 1e5)],
            vec![i("guides", 4), s("guide_labels", "Blank lines"), b("fit", false), f("x_from", 0.0), f("x_to", 1e6), f("x_by", 1e5)],
            vec![i("guides", 4), b("fit", false), f("x_from", 0.0), f("x_to", 1e6), f("x_by", 1e5)],
            vec![s("y", "Activity"), s("act_unit", "Counts per minute"), s("xtitle_mode", "Blank line"), s("ytitle", "Count rate measured by the Geiger counter in counts per minute")],
        ] {
            let mut l = list.clone();
            l.push(s("label_size", size));
            let svg = svg_of(&l);
            assert!(!svg.contains("NaN"), "{l:?}");
            // guide labels apart from each other and inside the page
            let p = with(&l);
            let unit = axis_unit(&p);
            let what = format!("{l:?}");
            let lab: Vec<_> = texts(&svg).into_iter().filter(|t| t.1 < 120.0 && t.3.ends_with(&format!(" {}", short_unit(unit)))).collect();
            for (k, a) in lab.iter().enumerate() {
                let wa = text_w(&a.3, a.2);
                assert!(a.0 >= 0.0 && a.0 + wa <= 640.0, "{what}: {} off the page", a.3);
                for c in &lab[k + 1..] {
                    let wc = text_w(&c.3, c.2);
                    let apart = a.0 + wa <= c.0 + 0.5 || c.0 + wc <= a.0 + 0.5 || (a.1 - c.1).abs() >= a.2.max(c.2) * 0.9;
                    assert!(apart, "{what}: {} and {} overlap", a.3, c.3);
                }
            }
            if !p.b("fit") { texts_fit(&svg, &what); }
        }
    }
}

/// Unticking "Fit the axes" fills the boxes with the fitted axes, so nothing moves; with the ranges
/// typed, a new time unit keeps the same times in view and a new y quantity refits the y range.
#[test]
fn unticking_fit_starts_from_the_fitted_axes() {
    let d = def();
    let mut p = with(&[s("iso1", "Iodine-131 (8.02 days)")]);
    let before = d.svg(&p);
    p.set_b("fit", false);
    crate::figures::after_panel_change(ID, "fit", &Value::B(true), &mut p);
    assert_eq!(p.s("unit"), "Days", "the fitted unit is pinned");
    assert!(p.f("x_from") == 0.0 && (p.f("x_to") - 8.02 * 5.0).abs() < 1e-9 && p.f("x_by") == 10.0, "{:?}", (p.f("x_to"), p.f("x_by")));
    assert_eq!((p.f("y_from"), p.f("y_to"), p.f("y_by")), (0.0, 100.0, 20.0));
    let after = d.svg(&p);
    let read = |svg: &str| {
        let (tx, vy) = (map(x_numbers(svg)), map(y_numbers(svg)));
        polylines(svg)[0].iter().step_by(24).map(|&(x, y)| ((tx(x) * 1e3).round(), (vy(y) * 10.0).round())).collect::<Vec<_>>()
    };
    assert_eq!(read(&before), read(&after), "the curve stays where it was");
    assert_eq!(x_numbers(&before).iter().map(|t| t.1).collect::<Vec<_>>(), x_numbers(&after).iter().map(|t| t.1).collect::<Vec<_>>());
    // Days to Hours: the same 40.1 days, now 962.4 hours
    p.set_s("unit", "Hours");
    crate::figures::after_panel_change(ID, "unit", &Value::S("Days".into()), &mut p);
    assert!((p.f("x_to") - 8.02 * 5.0 * 24.0).abs() < 1e-9);
    let svg = d.svg(&p);
    assert!(svg.contains(">Time (hours)</text>"));
    let tx = map(x_numbers(&svg));
    assert!((tx(guides(&svg)[0]) - 8.02 * 24.0).abs() < 1.0);
    // a new y quantity refits the y range
    p.set_s("y", "Mass remaining");
    p.set_f("amount", 40.0);
    crate::figures::after_panel_change(ID, "amount", &Value::F(100.0), &mut p);
    assert_eq!((p.f("y_from"), p.f("y_to")), (0.0, 40.0));
    // with the axes fitted nothing is rewritten
    let mut q = with(&[]);
    let keep = q.clone();
    crate::figures::after_panel_change(ID, "unit", &Value::S("Fitted".into()), &mut q);
    assert_eq!(q.0, keep.0);
}

/// The panel shows only what applies; another figure's controls are untouched.
#[test]
fn controls_show_only_where_they_apply() {
    use crate::figures::param_relevant as r;
    let p = with(&[]);
    for k in ["custom_name", "custom_half", "custom_unit", "act_unit", "amount", "x_from", "x_to", "x_by", "y_from", "y_to", "y_by", "iso2", "iso3"] {
        assert!(!r(ID, k, &p), "{k} shown at the opening state");
    }
    for k in ["count", "iso1", "y", "guides", "guide_labels", "span", "unit", "fit", "xtitle_mode", "xtitle", "x_num", "y_num", "minor_grid", "title"] {
        assert!(r(ID, k, &p), "{k} hidden at the opening state");
    }
    let q = with(&[b("fit", false), i("count", 2), s("iso2", "Custom"), s("y", "Activity")]);
    for k in ["custom_name", "custom_half", "custom_unit", "act_unit", "amount", "x_from", "y_by", "iso2"] { assert!(r(ID, k, &q), "{k}"); }
    assert!(!r(ID, "span", &q) && !r(ID, "iso3", &q));
    assert!(!r(ID, "guide_labels", &with(&[i("guides", 0)])));
    // the titration curve keeps its own x-axis boxes
    let t = registry().into_iter().find(|d| d.id == "titration-curve").unwrap();
    let tp = crate::figures::start_params(&t);
    assert!(r("titration-curve", "xaxis_from", &tp) == r("titration-curve", "xaxis_from", &tp.clone()));
    assert!(r("titration-curve", "amount", &tp) && r("titration-curve", "fit", &tp));
}

/// The span knob, found through the workspace's own hit test: there with the axes fitted, gone
/// with the ranges typed (the span is not drawn then).
#[test]
fn the_span_knob_only_while_the_axes_are_fitted() {
    use crate::figures::interact::pick;
    let d = def();
    let size = (640.0, 366.0);
    let p = with(&[]);
    let hs = d.handles(&p, size);
    let mut hit = 0;
    for x in (0..640).step_by(4) {
        for y in (0..120).step_by(4) {
            if let Some(k) = pick(&hs, (x as f64, y as f64), 1.0) { assert!(hs[k].tip.contains("half-lives")); hit += 1; }
        }
    }
    assert!(hit > 0, "the span knob is found by the hit test");
    let q = with(&[b("fit", false)]);
    let hs = d.handles(&q, size);
    for x in (0..640).step_by(4) { for y in (0..366).step_by(4) { assert!(pick(&hs, (x as f64, y as f64), 1.0).is_none()); } }
}

/// No input can draw an unbounded number of lines or labels (a −10³⁰ to 10³⁰ axis counted by 10⁻⁹
/// once made millions): every typed range is held to MAX_LINES blocks, minor lines to 4 per block,
/// and the fitted axes at the ends of every range stay small too.
#[test]
fn the_ends_of_every_range_draw_a_bounded_figure() {
    let mut cases: Vec<Vec<(&'static str, Value)>> = vec![];
    let ends = [-AXIS_LIM, -1.0, 0.0, 1e-9, 1.0, 1e12, AXIS_LIM];
    let bys = [1e-9, 1e-3, 1.0, 1e9, AXIS_LIM];
    for &from in &ends {
        for &to in &ends {
            for &by in &bys {
                cases.push(vec![b("fit", false), f("x_from", from), f("x_to", to), f("x_by", by), f("y_from", from), f("y_to", to), f("y_by", by), s("minor_grid", "5 per block"), i("guides", 4), s("guide_labels", "Times")]);
            }
        }
    }
    for (iso, half, unit) in [("Custom", 0.001, "Seconds"), ("Custom", 1e9, "Billion years"), ("Custom", 0.001, "Billion years"), ("Custom", 1e9, "Seconds")] {
        for axis in ["Seconds", "Billion years", "Fitted"] {
            for span in [1.0, 8.0] {
                cases.push(vec![s("iso1", iso), f("custom_half", half), s("custom_unit", unit), s("unit", axis), f("span", span), s("minor_grid", "5 per block"), i("guides", 4), s("guide_labels", "Times"), s("y", "Number of nuclei"), f("amount", 1e12)]);
            }
        }
    }
    for list in cases {
        let svg = svg_of(&list);
        assert!(svg.len() < 400_000, "{list:?}: {} bytes", svg.len());
        assert!(!svg.contains("NaN"), "{list:?}");
        let ticks = lines_of(&svg, INK).into_iter().filter(|l| (l.1 - BOTTOM).abs() < 0.01 && (l.3 - BOTTOM - 6.0).abs() < 0.01).count();
        let faint = lines_of(&svg, "#eceff4").len();
        let fit = list.iter().all(|(k, _)| *k != "fit");
        let cap = if fit { 401 } else { MAX_LINES as usize + 1 };
        assert!(ticks <= cap, "{list:?}: {ticks} x lines");
        assert!(faint <= 2 * cap * 4, "{list:?}: {faint} minor lines");
        assert!(texts(&svg).len() <= 2 * cap + 20, "{list:?}: {} texts", texts(&svg).len());
    }
}

/// The key's half-life is the one the curve is drawn with, as the isotope list writes it (Cs-137 is
/// 30.1 years, not 30), and a typed range's numbers are readable at the ends of the range.
#[test]
fn the_key_states_the_half_life_drawn() {
    for (choice, _) in ISOTOPES {
        let (name, given) = choice.split_once(" (").unwrap();
        let given = given.trim_end_matches(')');
        let key = texts(&svg_of(&[s("iso1", choice)])).into_iter().map(|t| t.3).find(|t| t.contains("\u{03bb} = ")).unwrap();
        assert!(key.starts_with(&format!("{name}: T\u{00bd} = {given}, ")), "{key}");
    }
    assert_eq!(human_half(YEAR), "1 year");
    assert_eq!(human_half(DAY), "1 day");
    assert_eq!(human_half(2.0 * DAY), "2 days");
    assert_eq!(axis_num(1e30), "1e30");
    assert_eq!(axis_num(-2.5e29), "\u{2212}2.5e29");
    assert_eq!(axis_num(25000.0), "25000");
    let svg = svg_of(&[b("fit", false), f("x_from", -AXIS_LIM), f("x_to", AXIS_LIM), f("x_by", 1e-9), f("y_from", -AXIS_LIM), f("y_to", AXIS_LIM)]);
    let xs: Vec<String> = texts(&svg).into_iter().filter(|t| t.4 == "middle" && (t.1 - (BOTTOM + 20.6)).abs() < 0.01).map(|t| t.3).collect();
    assert!(xs.contains(&"0".to_string()) && xs.iter().all(|t| t.chars().count() <= 8), "{xs:?}");
}
