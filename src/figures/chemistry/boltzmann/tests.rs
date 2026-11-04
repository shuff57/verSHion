//! Tests of the Boltzmann distribution. The science tests recompute the speeds, the density and the fraction from
//! their own constants and their own numerical integration; the drawing tests read the SVG back and check the points
//! against the formula; the label audit measures every text with the real glyphs the PNG renderer draws.
use super::*;
use crate::figures::chemistry::reactionenergy::{hits, seg_hits};
use crate::figures::{registry, svg_size, to_png};
use std::collections::{BTreeMap, BTreeSet, HashMap};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("boltzmann-distribution is registered") }
fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
/// Never normalised: render must survive what a hostile link or an old file holds.
fn raw(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p
}
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
fn svg_of(list: &[(&'static str, Value)]) -> String { def().svg(&with(list)) }
fn rep_of(list: &[(&'static str, Value)]) -> Report { build(&with(list)).1 }
fn key(list: &[(&'static str, Value)]) -> Report { let mut l = list.to_vec(); l.push(b(SHOW_ANSWERS, true)); rep_of(&l) }
fn student(list: &[(&'static str, Value)]) -> Report { let mut l = list.to_vec(); l.push(b(SHOW_ANSWERS, false)); rep_of(&l) }
fn notes_of(list: &[(&'static str, Value)]) -> Vec<String> { warnings(&with(list)) }
fn has(notes: &[String], what: &str) -> bool { notes.iter().any(|n| n.contains(what)) }
fn joined(r: &Report) -> String { r.readout.join("\n") }

fn close(a: f64, b: f64, rel: f64) -> bool { (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-300) }

// ---------------------------------------------------------------------------------------------
// Independent science
// ---------------------------------------------------------------------------------------------

/// Simpson's rule with an even number of intervals.
fn simpson(fun: impl Fn(f64) -> f64, a: f64, b: f64, n: usize) -> f64 {
    let n = n + n % 2;
    let h = (b - a) / n as f64;
    let mut s = fun(a) + fun(b);
    for k in 1..n {
        s += fun(a + k as f64 * h) * if k % 2 == 0 { 2.0 } else { 4.0 };
    }
    s * h / 3.0
}

const T_CASES: [f64; 6] = [50.0, 100.0, 273.15, 300.0, 777.0, 3000.0];
const M_CASES: [f64; 6] = [0.001, 0.002016, 0.004003, 0.028014, 0.031998, 0.04401];

#[test]
fn it_is_registered_with_a_faq_examples_and_its_controls() {
    let d = def();
    assert_eq!(d.id, "boltzmann-distribution");
    assert_eq!(d.subject, Subject::Chemistry);
    assert!((4..=6).contains(&crate::figures::faq::faq_for(ID).len()));
    let keys: Vec<&str> = (d.params)().iter().map(|p| p.key).collect();
    for k in ["gas", "molar_mass", "curves", "t1", "t2", "t3", "ea", "mark", "dec", "grid", SHOW_ANSWERS, "style", "label_size"] {
        assert!(keys.contains(&k), "no {k} param");
    }
    assert!(crate::figures::chemistry::skin_intro(ID).is_some());
    assert!(!crate::figures::notes_for(ID, &raw(&[f("t1", 0.0)])).is_empty());
    // the custom mass box says when it does nothing
    assert!(crate::figures::chemistry::notes::key_note(ID, "molar_mass", &with(&[])).is_some());
    assert!(crate::figures::chemistry::notes::key_note(ID, "molar_mass", &with(&[s("gas", "Custom")])).is_none());
    let names: Vec<&str> = crate::figures::examples_for(ID).iter().map(|e| e.name).collect();
    assert!(names.len() >= 3, "{names:?}");
    assert!(names.iter().any(|n| n.contains("Three temperatures")), "{names:?}");
    for e in crate::figures::examples_for(ID) {
        let p = with(&e.params);
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
        for ans in [true, false] {
            let mut q = p.clone();
            q.set_b(SHOW_ANSWERS, ans);
            assert!(warnings(&q).is_empty(), "{}: {:?}", e.name, warnings(&q));
            let svg = def().svg(&q);
            assert!(svg.contains("<svg") && svg.len() > 1500, "{}", e.name);
        }
    }
}

#[test]
fn the_three_speeds_are_the_closed_formulas_and_are_ordered() {
    for &m in &M_CASES {
        for &t in &T_CASES {
            let want_vp = (2.0 * R * t / m).sqrt();
            let want_mean = (8.0 * R * t / (PI * m)).sqrt();
            let want_rms = (3.0 * R * t / m).sqrt();
            assert!(close(vp(t, m), want_vp, 1e-15), "{m} {t}");
            assert!(close(v_mean(t, m), want_mean, 1e-15), "{m} {t}");
            assert!(close(v_rms(t, m), want_rms, 1e-15), "{m} {t}");
            assert!(vp(t, m) < v_mean(t, m) && v_mean(t, m) < v_rms(t, m), "{m} {t}");
            // the ratios every course quotes
            assert!(close(v_mean(t, m) / vp(t, m), (4.0 / PI).sqrt(), 1e-15));
            assert!(close(v_rms(t, m) / vp(t, m), 1.5f64.sqrt(), 1e-15));
        }
    }
    // a bad temperature or molar mass has no speed and does not panic
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(vp(bad, 0.028), 0.0);
        assert_eq!(v_mean(300.0, bad), 0.0);
        assert!(v_rms(300.0, 0.028).is_finite());
    }
}

#[test]
fn the_peak_of_the_density_is_exactly_at_the_most_probable_speed() {
    for &m in &M_CASES {
        for &t in &T_CASES {
            let v = vp(t, m);
            let peak = maxwell(v, t, m);
            assert!(peak > 0.0);
            for d in [0.5, 0.95, 0.99, 1.01, 1.05, 1.5] {
                assert!(maxwell(v * d, t, m) < peak, "M {m} T {t}: peak is not at vp ({d})");
            }
            // the derivative is zero at vp (central difference, scaled by the peak and vp)
            let h = 1e-4 * v;
            let slope = (maxwell(v + h, t, m) - maxwell(v - h, t, m)) / (2.0 * h);
            assert!(slope.abs() * v / peak < 1e-5, "M {m} T {t}: slope {slope}");
            // and the closed-form peak 4/(sqrt(pi e) vp)
            assert!(close(peak, 4.0 / (PI.sqrt() * std::f64::consts::E * v), 1e-12), "M {m} T {t}");
        }
    }
}

#[test]
fn every_curve_encloses_area_one_and_a_drawn_curve_is_within_a_thousandth() {
    for &m in &M_CASES {
        for &t in &[50.0, 300.0, 3000.0] {
            let v = vp(t, m);
            let area = simpson(|x| maxwell(x, t, m), 0.0, 12.0 * v, 20_000);
            assert!(close(area, 1.0, 1e-7), "M {m} T {t}: area {area}");
        }
    }
    // the plotted range (to 3.2 vp) is the whole area bar the far tail
    for list in [vec![], vec![s("gas", "H2"), f("t1", 3000.0), s("curves", "1")], vec![s("gas", "CO2"), f("t1", 50.0), s("curves", "1")]] {
        let r = key(&list);
        let c = &r.curves[0];
        let area = simpson(|x| maxwell(x, c.t, r.setup.m_kg()), 0.0, r.x_top, 20_000);
        assert!((area - 1.0).abs() < 1e-3, "{:?}: drawn area {area}", list);
        assert!(r.x_top >= 3.19 * c.vp, "{} {}", r.x_top, c.vp);
    }
}

#[test]
fn erfc_matches_known_values_and_the_abramowitz_stegun_bound() {
    assert_eq!(erfc(0.0), 1.0);
    assert!((erfc(1.0) - 0.157_299_207_050_285_13).abs() < 2e-7, "{}", erfc(1.0));
    assert!((erfc(2.0) - 0.004_677_734_981_029_65).abs() < 2e-7, "{}", erfc(2.0));
    assert!((erfc(0.5) - 0.479_500_122_186_953_5).abs() < 2e-7, "{}", erfc(0.5));
    assert!((erfc(-1.0) - (2.0 - erfc(1.0))).abs() < 1e-15);
    // the stated bound: no value is further than 1.5e-7 from a fine reference (the known values above, and symmetry)
    for x in [0.0, 0.1, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0] {
        let e = erfc(x);
        assert!((0.0..=2.0).contains(&e), "{x}: {e}");
        assert!((e + erfc(-x) - 2.0).abs() < 1e-15, "{x}");
    }
    assert_eq!(erfc(f64::INFINITY), 0.0);
    assert!(erfc(f64::NAN).is_nan());
}

#[test]
fn the_fraction_is_one_at_zero_energy_and_falls_as_the_energy_rises() {
    for &t in &T_CASES {
        assert_eq!(fraction_above(0.0, t), 1.0, "T {t}");
        assert!(fraction_above(1e-9, t) > 0.999_99, "T {t}");
        let mut prev = 1.0;
        for ea in [0.0, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0] {
            let f = fraction_above(ea * 1000.0, t);
            assert!(f <= prev + 1e-15 && (0.0..=1.0).contains(&f), "T {t} Ea {ea}: {f} after {prev}");
            prev = f;
        }
        assert!(fraction_above(500_000.0, t) < 1e-5, "T {t}");
    }
    assert_eq!(fraction_above(-1.0, 300.0), 1.0);
    assert_eq!(fraction_above(10.0, 0.0), 0.0);
}

#[test]
fn the_fraction_above_an_energy_matches_a_numerical_integral_of_the_speed_curve() {
    let mut cases = 0;
    for &t in &[300.0, 900.0] {
        for &m in &[0.002016, 0.028014, 0.04401] {
            let v = vp(t, m);
            for ea_kj in [0.0, 1.0, 5.0, 10.0, 20.0, 40.0] {
                let va = (2.0 * ea_kj * 1000.0 / m).sqrt();
                let integral = simpson(|x| maxwell(x, t, m), va, va + 12.0 * v, 20_000);
                let f = fraction_above(ea_kj * 1000.0, t);
                assert!((integral - f).abs() < 2e-7, "M {m} T {t} Ea {ea_kj}: integral {integral} against F {f}");
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 36);
}

#[test]
fn the_fraction_rises_with_temperature_at_a_fixed_activation_energy() {
    for ea in [1.0, 5.0, 20.0, 50.0] {
        let mut prev = 0.0;
        for t in [50.0, 100.0, 200.0, 300.0, 600.0, 1200.0, 3000.0] {
            let f = fraction_above(ea * 1000.0, t);
            assert!(f > prev, "Ea {ea} T {t}: {f} against {prev}");
            prev = f;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Reading the SVG back
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct T { x: f64, y: f64, fs: f64, anchor: String, text: String, kind: String, feat: String, fill: String, rot: Option<(f64, f64)> }

fn attr(tag: &str, k: &str) -> Option<String> {
    let pat = format!(" {k}=\"");
    let at = tag.find(&pat)? + pat.len();
    tag[at..].split('"').next().map(|v| v.to_string())
}
fn attr_f(tag: &str, k: &str) -> Option<f64> { attr(tag, k)?.parse().ok() }
fn unesc(s: &str) -> String { s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&") }

fn texts(svg: &str) -> Vec<T> {
    let mut out = Vec::new();
    for part in svg.split("<text").skip(1) {
        let Some((tag, rest)) = part.split_once('>') else { continue };
        if tag.contains(r#"fill="none""#) { continue; }
        let text = unesc(rest.split("</text>").next().unwrap_or(""));
        let rot = attr(tag, "transform").and_then(|t| {
            let inner = t.strip_prefix("rotate(-90 ")?.strip_suffix(')')?;
            let (a, b) = inner.split_once(' ')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        });
        out.push(T {
            x: attr_f(tag, "x").unwrap_or(0.0),
            y: attr_f(tag, "y").unwrap_or(0.0),
            fs: attr_f(tag, "font-size").unwrap_or(12.0),
            anchor: attr(tag, "text-anchor").unwrap_or_else(|| "start".into()),
            text,
            kind: attr(tag, "data-o").unwrap_or_default(),
            feat: attr(tag, "data-f").unwrap_or_default(),
            fill: attr(tag, "fill").unwrap_or_default(),
            rot,
        });
    }
    out
}

fn path_pts(d: &str) -> Vec<Pt> {
    d.split_whitespace()
        .filter_map(|tok| {
            let t = tok.trim_start_matches(|c: char| c.is_ascii_alphabetic());
            let (a, b) = t.split_once(',')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .collect()
}

type Seg = (Pt, Pt);

struct Geo {
    lines: Vec<(String, String, Seg)>,
    paths: Vec<(String, String, Vec<Pt>, String)>,
    marks: Vec<(String, String, Pt, f64)>,
    rects: Vec<String>,
}

fn geo(svg: &str) -> Geo {
    let mut g = Geo { lines: Vec::new(), paths: Vec::new(), marks: Vec::new(), rects: Vec::new() };
    for part in svg.split('<').skip(1) {
        let tag = part.split('>').next().unwrap_or("");
        let kind = attr(tag, "data-o").unwrap_or_default();
        let feat = attr(tag, "data-f").unwrap_or_default();
        if tag.starts_with("line ") {
            let (x1, y1, x2, y2) = (attr_f(tag, "x1").unwrap(), attr_f(tag, "y1").unwrap(), attr_f(tag, "x2").unwrap(), attr_f(tag, "y2").unwrap());
            g.lines.push((kind, feat, ((x1, y1), (x2, y2))));
        } else if tag.starts_with("path ") {
            let pts = path_pts(&attr(tag, "d").unwrap());
            let style = format!("{}|{}|{}", attr(tag, "stroke").unwrap_or_default(), attr(tag, "stroke-width").unwrap_or_default(), attr(tag, "stroke-dasharray").unwrap_or_default());
            g.paths.push((kind, feat, pts, style));
        } else if tag.starts_with("circle ") && kind == "mark" {
            let (cx, cy, r) = (attr_f(tag, "cx").unwrap(), attr_f(tag, "cy").unwrap(), attr_f(tag, "r").unwrap());
            g.marks.push((kind, feat, (cx, cy), r));
        } else if tag.starts_with("rect ") && kind == "legend" {
            g.rects.push(attr(tag, "fill").unwrap_or_default());
        }
    }
    g
}

fn data_xy(fr: &Frame, p: Pt) -> (f64, f64) {
    let x = fr.x0 + (p.0 - fr.left()) / (fr.right() - fr.left()) * (fr.x1 - fr.x0);
    let y = fr.y0 + (fr.bottom() - p.1) / (fr.bottom() - fr.top()) * (fr.y1 - fr.y0);
    (x, y)
}

// ---------------------------------------------------------------------------------------------
// The drawing
// ---------------------------------------------------------------------------------------------

#[test]
fn the_curve_points_lie_on_the_maxwell_boltzmann_formula() {
    let list = [f("t1", 300.0), f("t2", 600.0), f("ea", 10.0), f("mark", 845.3)];
    let r = key(&list);
    let svg = svg_of(&list);
    let g = geo(&svg);
    let curves: Vec<&(String, String, Vec<Pt>, String)> = g.paths.iter().filter(|p| p.0 == "curve").collect();
    assert_eq!(curves.len(), r.curves.len());
    for (k, c) in curves.iter().enumerate() {
        let feat = &c.1;
        let pts = &c.2;
        assert_eq!(feat, &format!("c{k}"));
        assert!(pts.len() >= SAMPLES, "{} points", pts.len());
        let cc = &r.curves[k];
        let tol = 1.5 * r.y_top / (r.frame.bottom() - r.frame.top());
        for &p in pts.iter() {
            let (v, y) = data_xy(&r.frame, p);
            assert!((y - maxwell(v, cc.t, r.setup.m_kg())).abs() <= tol, "c{k}: at {v} m/s, {y} against {}", maxwell(v, cc.t, r.setup.m_kg()));
        }
        // the peak vertex is the most probable speed, and the peak height is exact to the pixel
        assert!(pts.iter().any(|p| (data_xy(&r.frame, *p).0 - cc.vp).abs() < 0.05), "c{k} has no vertex at vp");
        let top = pts.iter().map(|p| p.1).fold(f64::MAX, f64::min);
        assert!((top - r.frame.py(cc.peak)).abs() < 0.01, "c{k}: peak {top} against {}", r.frame.py(cc.peak));
        assert!(close(cc.peak, 4.0 / (PI.sqrt() * std::f64::consts::E * cc.vp), 1e-12));
    }
}

#[test]
fn the_tail_shading_starts_at_the_activation_energy_and_is_drawn_on_the_key_only() {
    let list = [f("t1", 300.0), f("t2", 600.0), f("ea", 10.0)];
    let k = key(&list);
    let kg = geo(&svg_of(&list));
    let tail = kg.paths.iter().find(|p| p.0 == "tail").expect("the key shades the tail");
    assert_eq!(tail.1, "c1", "the hottest curve");
    let va = (2.0 * 10_000.0 / k.setup.m_kg()).sqrt();
    assert!(close(k.ea_speed.unwrap(), va, 1e-12));
    assert!(close(tail.2[0].0, k.frame.px(va), 0.06), "tail starts at {} against {}", tail.2[0].0, k.frame.px(va));
    let top = tail.2.iter().map(|p| p.1).fold(f64::MAX, f64::min);
    assert!(close(top, k.frame.py(maxwell(va, k.curves[1].t, k.setup.m_kg())), 0.06), "the shaded edge is not the hottest curve at Ea's speed");
    let bottom = tail.2.iter().map(|p| p.1).fold(f64::MIN, f64::max);
    assert!(close(bottom, k.frame.bottom(), 0.06));
    // Ea's line is a given: both copies draw it, at the same speed
    for svg in [svg_of(&list), svg_of(&[f("t1", 300.0), f("t2", 600.0), f("ea", 10.0), b(SHOW_ANSWERS, false)])] {
        let g = geo(&svg);
        let ea = g.lines.iter().find(|l| l.0 == "ea").expect("Ea line on both copies");
        assert!(close(ea.2 .0 .0, k.frame.px(va), 0.06));
        assert!(close(ea.2 .1 .0, k.frame.px(va), 0.06));
    }
    // the student copy does not shade it
    let sg = geo(&svg_of(&[f("t1", 300.0), f("t2", 600.0), f("ea", 10.0), b(SHOW_ANSWERS, false)]));
    assert!(sg.paths.iter().all(|p| p.0 != "tail"), "the student copy has the shaded answer");
    // Ea = 0 shades the whole area and the fraction is exactly 1
    let r0 = key(&[f("ea", 0.0)]);
    assert_eq!(r0.curves[0].above, 1.0);
    let g0 = geo(&svg_of(&[f("ea", 0.0)]));
    let t0 = g0.paths.iter().find(|p| p.0 == "tail").unwrap();
    assert!(close(t0.2[0].0, r0.frame.px(0.0), 0.06));
    assert!(joined(&r0).contains("100.000 %"), "{}", joined(&r0));
}

#[test]
fn the_marked_speed_guide_lands_on_the_hottest_curve() {
    let list = [f("t1", 600.0), f("t2", 300.0), f("mark", 700.0)];
    let r = key(&list);
    assert_eq!(r.curves[0].t, 300.0, "the curves are drawn coolest first");
    assert_eq!(r.curves[1].t, 600.0);
    let hot = r.curves.last().unwrap().t;
    let (mx, my) = r.mark_point.expect("the mark is on the axis");
    assert!(close(mx, r.frame.px(700.0), 1e-12));
    assert!(close(my, r.frame.py(maxwell(700.0, hot, r.setup.m_kg())), 1e-12));
    let g = geo(&svg_of(&list));
    let guide = g.lines.iter().find(|l| l.0 == "guide" && l.1 == "mark").expect("the guide");
    assert!((guide.2 .0 .0 - mx).abs() < 0.02 && (guide.2 .1 .0 - mx).abs() < 0.02);
    assert!((guide.2 .1 .1 - my).abs() < 0.02, "the guide stops on the curve");
    let dot = g.marks.iter().find(|m| m.1 == "mark").expect("the dot");
    assert!((dot.2 .0 - mx).abs() < 0.01 && (dot.2 .1 - my).abs() < 0.01);
    // the value is an answer: the key prints it, the student has a blank and no value
    let kt = joined(&r);
    assert!(kt.contains(&format!("At 700 m/s the {hot} K curve has f = ")), "{kt}");
    let st = student(&list);
    assert!(joined(&st).contains("______ s/m"), "{}", joined(&st));
    let stj = joined(&st);
    let fl = stj.lines().find(|l| l.contains("curve has f")).expect("the marked density line");
    assert!(fl.contains("f = ______ s/m"), "{fl}");
}

#[test]
fn the_student_copy_blanks_the_table_and_prints_no_computed_value() {
    let list = [f("t1", 300.0), f("t2", 600.0), f("ea", 10.0), f("mark", 845.3), i("dec", 3)];
    let k = key(&list);
    let kv = svg_of(&list);
    let sv = {
        let mut l = list.to_vec();
        l.push(b(SHOW_ANSWERS, false));
        svg_of(&l)
    };
    let kt = texts(&kv);
    let st = texts(&sv);
    assert_eq!(kt.iter().filter(|t| t.kind == "answer").count(), k.curves.len() * 3);
    assert_eq!(st.iter().filter(|t| t.kind == "answer").count(), 0, "the student copy prints an answer text");
    assert_eq!(st.iter().filter(|t| t.kind == "blank").count(), k.curves.len() * 3);
    assert!(st.iter().all(|t| t.fill != "#111111" || t.kind != "answer"));
    assert!(!sv.contains("data-o=\"answer\""), "an answer-only text is on the student copy");
    assert!(st.iter().filter(|t| t.text.contains(BLANK)).count() >= k.curves.len() * 3 + 2, "blanks for the table, the fractions and f");
    // no answer string from the key appears as printed text anywhere in the student source
    for c in &k.curves {
        for v in [c.vp, c.mean, c.rms] {
            let t = res_fmt(v, 3);
            assert!(!sv.contains(&format!(">{t}<")), "student copy prints the table value {t}");
        }
        let pct = res_fmt(c.above * 100.0, 3);
        assert!(!sv.contains(&format!("{pct} %")), "student copy prints the fraction {pct} %");
    }
    let fval = res_fmt(maxwell(845.3, k.curves[1].t, k.setup.m_kg()), 3);
    assert!(!sv.contains(&format!("f = {fval} s/m")), "student copy prints f = {fval}");
    // the student note lines keep only the givens
    let above = st.iter().find(|t| t.text.contains("Above Ea")).expect("the fraction line");
    assert!(above.text.contains(BLANK), "{}", above.text);
    for g in ["10", "300", "600"] { assert!(above.text.contains(g), "{}", above.text); }
    let fl = st.iter().find(|t| t.text.contains("curve has f")).expect("the marked density line");
    assert!(fl.text.contains(BLANK), "{}", fl.text);
    for g in ["845.3", "600"] { assert!(fl.text.contains(g), "{}", fl.text); }
}

#[test]
fn print_is_grey_only_and_holds_up_at_66_percent() {
    fn grey(h: &str) -> bool {
        let h = h.trim_start_matches('#');
        let h: Vec<char> = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.chars().collect() };
        h.len() == 6 && h.iter().all(|c| c.is_ascii_hexdigit()) && {
            let c = |i: usize| u8::from_str_radix(&h[i..i + 2].iter().collect::<String>(), 16).unwrap_or(0);
            c(0) == c(2) && c(2) == c(4)
        }
    }
    for ans in [true, false] {
        let list = [s("curves", "3"), f("t1", 300.0), f("t2", 600.0), f("t3", 900.0), f("ea", 20.0), s("style", "Print"), b(SHOW_ANSWERS, ans)];
        let svg = svg_of(&list);
        for k in ["fill", "stroke"] {
            for v in svg.split(&format!(" {k}=\"")).skip(1).map(|x| x.split('"').next().unwrap()) {
                assert!(v == "none" || !v.starts_with('#') || grey(v), "Print draws a hue: {k}={v}");
            }
        }
        let g = geo(&svg);
        let dashes: Vec<String> = g.paths.iter().filter(|p| p.0 == "curve").map(|p| p.3.rsplit('|').next().unwrap().to_string()).collect();
        assert_eq!(dashes.len(), 3);
        let uniq: BTreeSet<&String> = dashes.iter().collect();
        assert_eq!(uniq.len(), 3, "three curves share a dash: {dashes:?}");
        // each series has its own hatch too (the legend swatches)
        let hatches: BTreeSet<&str> = g.rects.iter().filter(|r| r.starts_with("url(#vs-hatch")).map(|r| r.as_str()).collect();
        assert_eq!(hatches.len(), 3, "{:?}", g.rects);
        // 66 %: no hue survives, the type and the thin strokes are still there
        let png = to_png(&svg, 0.66).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) == 0), "colour in a Print render");
        let small = texts(&svg).iter().map(|t| t.fs).fold(f64::INFINITY, f64::min);
        assert!(small * 0.66 >= 8.4, "the smallest type is {small} px");
        for w in svg.split(" stroke-width=\"").skip(1).map(|x| x.split('"').next().unwrap().parse::<f64>().unwrap()) {
            assert!(w * 0.66 >= 0.66, "hairline {w}");
        }
    }
    // Colour uses colour, and it comes from the series palette, not from ad-hoc hues
    let svg = svg_of(&[s("curves", "3"), f("t1", 300.0), f("t2", 600.0), f("t3", 900.0)]);
    let g = geo(&svg);
    let cols: BTreeSet<String> = g.paths.iter().filter(|p| p.0 == "curve").map(|p| p.3.split('|').next().unwrap().to_string()).collect();
    assert_eq!(cols.len(), 3);
    assert!(cols.iter().any(|c| !grey(c)));
}

#[test]
fn label_size_scales_the_type_and_grows_the_canvas() {
    let mut sizes = Vec::new();
    let mut minfs = Vec::new();
    for name in ["Small", "Medium", "Large"] {
        let svg = svg_of(&[s("label_size", name), f("mark", 845.3)]);
        let (w, h) = svg_size(&svg);
        sizes.push((w, h));
        minfs.push(texts(&svg).iter().map(|t| t.fs).fold(f64::INFINITY, f64::min));
        assert!(w > 600.0 && h > 400.0, "{name}: {w}x{h}");
    }
    assert!(sizes[0].0 < sizes[1].0 && sizes[1].0 < sizes[2].0, "{sizes:?}");
    assert!(minfs[0] < minfs[1] && minfs[1] < minfs[2], "{minfs:?}");
    assert!(close(minfs[1] / minfs[0], 1.25, 1e-9), "{minfs:?}");
    assert!(close(minfs[2] / minfs[1], 1.4, 1e-9), "{minfs:?}");
}

#[test]
fn the_axes_are_the_shared_nice_step_and_cover_the_range() {
    for list in [vec![], vec![s("gas", "H2"), f("t1", 3000.0), s("curves", "1")], vec![f("ea", 500.0), s("gas", "CO2")]] {
        let r = key(&list);
        assert!(r.x_top > 0.0 && r.y_top > 0.0);
        let g = geo(&svg_of(&list));
        let mut px: Vec<f64> = g.lines.iter().filter(|l| l.0 == "axis" && close(l.2 .0 .0, l.2 .1 .0, 1e-12)).map(|l| l.2 .0 .0).collect();
        px.sort_by(|a, b| a.total_cmp(b));
        px.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        assert!(px.len() >= 3, "{:?}: {px:?}", list);
        assert!(px[0] >= r.frame.left() - 0.02 && *px.last().unwrap() <= r.frame.right() + 0.02);
        // the step is 1, 2 or 5 times a power of ten, and every tick is a multiple of it
        let vals: Vec<f64> = px.iter().map(|x| r.frame.x0 + (x - r.frame.left()) / (r.frame.right() - r.frame.left()) * (r.frame.x1 - r.frame.x0)).collect();
        let step = vals[1] - vals[0];
        let e = step.log10().floor();
        let m = step / 10f64.powf(e);
        assert!([1.0, 2.0, 5.0].iter().any(|v| close(*v, m, 1e-9)), "step {step} is not a nice step");
        for v in &vals {
            let k = v / step;
            assert!((k - k.round()).abs() < 1e-6, "{v} is not a multiple of {step}");
        }
        // the frame maps its corners
        assert!(close(r.frame.px(0.0), r.frame.left(), 1e-12));
        assert!(close(r.frame.px(r.x_top), r.frame.right(), 1e-12));
        assert!(close(r.frame.py(0.0), r.frame.bottom(), 1e-12));
        assert!(close(r.frame.py(r.y_top), r.frame.top(), 1e-12));
    }
}

#[test]
fn temperatures_are_sorted_deduplicated_and_the_count_chooses_the_curves() {
    let r = key(&[s("curves", "3"), f("t1", 900.0), f("t2", 300.0), f("t3", 600.0)]);
    assert_eq!(r.curves.iter().map(|c| c.t).collect::<Vec<_>>(), vec![300.0, 600.0, 900.0]);
    assert_eq!(r.curves.len(), 3);
    let r = key(&[s("curves", "1"), f("t1", 450.0), f("t2", 0.0), f("t3", -5.0)]);
    assert_eq!(r.curves.len(), 1);
    assert_eq!(r.curves[0].t, 450.0);
    assert!(r.notes.is_empty(), "an unused temperature raises a note: {:?}", r.notes);
    let r = key(&[s("curves", "2"), f("t1", 300.0), f("t2", 300.0)]);
    assert_eq!(r.curves.len(), 1);
    assert!(r.notes.iter().any(|n| n.contains("Temperature 2 is the same as temperature 1")), "{:?}", r.notes);
    // a difference below the tolerance is still one curve, not a ghost beside it
    let r = key(&[s("curves", "2"), f("t1", 300.0), f("t2", 300.000_000_000_1)]);
    assert_eq!(r.curves.len(), 1, "{:?}", r.curves);
    // an out-of-range or unparsable temperature is repaired and the rest still draws
    let r = key(&[s("curves", "2"), f("t1", 4000.0), f("t2", f64::NAN)]);
    assert_eq!(r.curves.len(), 2);
    assert!(r.curves.iter().all(|c| (T_LO..=T_HI).contains(&c.t)), "{:?}", r.curves);
}

// ---------------------------------------------------------------------------------------------
// Refusals and hostile input
// ---------------------------------------------------------------------------------------------

#[test]
fn refusals_name_the_box_the_reason_and_a_fixed_value_and_the_rest_still_draws() {
    let cases: Vec<(&str, Vec<(&'static str, Value)>, &[&str])> = vec![
        ("t1", vec![f("t1", 0.0)], &["Temperature 1", "absolute zero", "50 K is used"]),
        ("t2", vec![f("t2", 4000.0)], &["Temperature 2", "above the 3000 K", "3000 K is used"]),
        ("t1nan", vec![f("t1", f64::NAN)], &["Temperature 1", "not a number", "300 K is used"]),
        ("mass0", vec![s("gas", "Custom"), f("molar_mass", 0.0)], &["Custom molar mass", "below the 1 g/mol", "1 g/mol is used"]),
        ("massbig", vec![s("gas", "Custom"), f("molar_mass", 5000.0)], &["Custom molar mass", "above the 1000 g/mol", "1000 g/mol is used"]),
        ("eaneg", vec![f("ea", -5.0)], &["Activation energy Ea", "below 0", "0 kJ/mol is used"]),
        ("markneg", vec![f("mark", -10.0)], &["Marked speed", "no speed is marked"]),
        ("gas", vec![s("gas", "Unobtainium")], &["Gas \"Unobtainium\"", "N2 is used"]),
    ];
    for (tag, list, wants) in cases {
        let p = raw(&list);
        let notes = warnings(&p);
        for w in wants { assert!(has(&notes, w), "{tag}: {notes:?} lacks {w:?}"); }
        // the same text reaches the editor note
        assert_eq!(crate::figures::notes_for(ID, &p), notes, "{tag}");
        let svg = def().svg(&p);
        assert!(svg.contains("<svg") && svg.matches("data-o=\"curve\"").count() >= 1, "{tag}: the rest of the figure is missing");
        assert!(svg.matches("data-o=\"answer\"").count() >= 3, "{tag}: the key still draws its answers");
    }
    // the custom mass is not read while a preset gas is chosen
    assert!(notes_of(&[s("gas", "N2"), f("molar_mass", 5000.0)]).is_empty());
    // a refused value is not used
    let r = key(&[f("t1", 0.0), s("curves", "1")]);
    assert_eq!(r.curves[0].t, T_LO);
}

#[test]
fn odd_inputs_never_panic_and_never_print_nan_or_infinity() {
    let bad_vals = [
        Value::F(f64::NAN), Value::F(f64::INFINITY), Value::F(f64::NEG_INFINITY), Value::F(10f64.powi(400)),
        Value::F(-10f64.powi(400)), Value::F(1e300), Value::F(-1e300), Value::S(String::new()),
        Value::S("√π".into()), Value::S("x".repeat(10_000)), Value::B(true), Value::I(i64::MAX),
    ];
    let keys = ["gas", "molar_mass", "curves", "t1", "t2", "t3", "ea", "mark", "dec", "grid", "style", "label_size", SHOW_ANSWERS];
    for k in keys {
        for v in &bad_vals {
            let mut p = raw(&[]);
            p.0.insert(k.to_string(), v.clone());
            for ans in [true, false] {
                p.set_b(SHOW_ANSWERS, ans);
                let (svg, rep) = build(&p);
                assert!(svg.contains("<svg"), "{k} {v:?}");
                assert!(!svg.contains("NaN") && !svg.contains("inf") && !svg.contains("Infinity"), "{k} {v:?}");
                assert!(!rep.readout.iter().any(|l| l.contains("NaN") || l.contains("inf")), "{k} {v:?}: {:?}", rep.readout);
                for t in texts(&svg) {
                    assert!(!t.text.contains("NaN") && !t.text.contains("inf"), "{k} {v:?}: {:?}", t.text);
                }
            }
        }
    }
}

#[test]
fn drawing_is_pure_and_within_a_cpu_budget() {
    let base = [s("curves", "3"), f("ea", 20.0), f("mark", 1200.0), s("label_size", "Large")];
    let a = svg_of(&base);
    let _ = svg_of(&[b(SHOW_ANSWERS, false)]);
    assert_eq!(a, svg_of(&base));
    assert_eq!(a, svg_of(&base));
    let mut worst = std::time::Duration::ZERO;
    let mut total = std::time::Duration::ZERO;
    let mut n = 0;
    for gas in GAS_CHOICES {
        for curves in ["1", "2", "3"] {
            for key in [true, false] {
                let list = [s("gas", gas), s("curves", curves), f("ea", 20.0), f("mark", 845.3), b(SHOW_ANSWERS, key), s("label_size", "Large")];
                let p = with(&list);
                let t = std::time::Instant::now();
                let svg = (def().render)(&p);
                let dt = t.elapsed();
                assert!(svg.len() > 1500);
                worst = worst.max(dt);
                total += dt;
                n += 1;
            }
        }
    }
    println!("boltzmann: {n} draws, worst {worst:?}, mean {:?} (debug build)", total / n);
    assert!(worst < std::time::Duration::from_millis(1500), "{worst:?}");
}

#[test]
fn the_description_is_ascii_and_names_no_number_and_no_answer() {
    for list in [vec![], vec![f("ea", 100.0), f("mark", 900.0), s("curves", "3")], vec![b(SHOW_ANSWERS, false)]] {
        let svg = svg_of(&list);
        let desc = svg.split("<desc>").nth(1).and_then(|x| x.split("</desc>").next()).unwrap_or("").to_string();
        let title = svg.split("<title>").nth(1).and_then(|x| x.split("</title>").next()).unwrap_or("").to_string();
        assert!(desc.is_ascii() && !desc.is_empty() && desc.len() <= 320, "{desc}");
        assert!(title.is_ascii(), "{title}");
        assert!(!desc.chars().any(|c| c.is_ascii_digit()), "{desc}");
        for w in ["vp", "rms", "mean", "=", "%"] { assert!(!desc.contains(w), "{desc}"); }
    }
}

// ---------------------------------------------------------------------------------------------
// Real glyphs: the collision audit over the option space
// ---------------------------------------------------------------------------------------------

struct Glyphs { opt: resvg::usvg::Options<'static>, memo: HashMap<String, Option<Bx>> }

impl Glyphs {
    fn new() -> Self {
        let mut opt = resvg::usvg::Options::default();
        opt.fontdb = crate::figures::fontdb();
        Glyphs { opt, memo: HashMap::new() }
    }
    fn rel(&mut self, t: &T) -> Option<Bx> {
        let key = format!("{}|{:.2}|{}", t.text, t.fs, t.anchor);
        if let Some(v) = self.memo.get(&key) { return *v; }
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="6000" height="600"><text x="3000" y="300" font-size="{:.2}" text-anchor="{}" font-family="{FONT}">{}</text></svg>"#,
            t.fs, t.anchor, crate::figures::svg::esc(&t.text)
        );
        fn find(g: &resvg::usvg::Group) -> Option<resvg::usvg::Rect> {
            for n in g.children() {
                match n {
                    resvg::usvg::Node::Text(t) => return Some(t.flattened().abs_bounding_box()),
                    resvg::usvg::Node::Group(g2) => { if let Some(r) = find(g2) { return Some(r); } }
                    _ => {}
                }
            }
            None
        }
        let out = resvg::usvg::Tree::from_str(&svg, &self.opt).ok().and_then(|tree| find(tree.root())).filter(|r| r.width() > 0.0)
            .map(|r| (r.left() as f64 - 3000.0, r.right() as f64 - 3000.0, r.top() as f64 - 300.0, r.bottom() as f64 - 300.0));
        self.memo.insert(key, out);
        out
    }
    fn ink(&mut self, t: &T) -> Option<Bx> {
        let r = self.rel(t)?;
        Some(match t.rot {
            None => (t.x + r.0, t.x + r.1, t.y + r.2, t.y + r.3),
            Some((cx, cy)) => (cx + r.2, cx + r.3, cy - r.1, cy - r.0),
        })
    }
}

fn audit(svg: &str, g: &mut Glyphs) -> Vec<String> {
    let (w, h) = svg_size(svg);
    let ts = texts(svg);
    let gm = geo(svg);
    let mut bad = Vec::new();
    let inks: Vec<Option<Bx>> = ts.iter().map(|t| g.ink(t)).collect();
    for (t, bx) in ts.iter().zip(&inks) {
        let Some(bx) = bx else { continue };
        if bx.0 < 0.0 || bx.1 > w || bx.2 < 0.0 || bx.3 > h {
            bad.push(format!("{:?} leaves the {w}x{h} canvas: {bx:?}", t.text));
        }
    }
    for a in 0..ts.len() {
        for c in a + 1..ts.len() {
            if let (Some(x), Some(y)) = (inks[a], inks[c]) {
                if hits(x, y) { bad.push(format!("{:?} on {:?}", ts[a].text, ts[c].text)); }
            }
        }
    }
    let mut segs: Vec<(String, Seg)> = Vec::new();
    for (kind, feat, s) in &gm.lines { segs.push((format!("{kind} {feat}"), *s)); }
    for (kind, feat, pts, _) in &gm.paths {
        if kind == "tail" || kind == "grid" { continue; }
        for w2 in pts.windows(2) { segs.push((format!("{kind} {feat}"), (w2[0], w2[1]))); }
    }
    for (t, bx) in ts.iter().zip(&inks) {
        let Some(bx) = bx else { continue };
        for (what, seg) in &segs {
            if seg_hits(seg.0, seg.1, *bx) { bad.push(format!("{:?} box {bx:?} on a {what} stroke {seg:?}", t.text)); }
        }
        for (_, feat, c, r) in &gm.marks {
            let mb = (c.0 - r, c.0 + r, c.1 - r, c.1 + r);
            if hits(*bx, mb) { bad.push(format!("{:?} on the {feat} marker", t.text)); }
        }
    }
    bad
}

fn scenario_cases() -> Vec<(String, Vec<(&'static str, Value)>)> {
    let gas = |g: &'static str, list: &mut Vec<(&'static str, Value)>| list.push(s("gas", g));
    let mut v: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
    v.push(("defaults".into(), vec![]));
    let mut l = vec![f("t1", 100.0), f("t2", 300.0), f("ea", 5.0), f("mark", 3000.0)];
    gas("H2", &mut l); v.push(("h2".into(), l));
    let mut l = vec![s("curves", "1"), f("ea", 0.0)];
    gas("He", &mut l); v.push(("he".into(), l));
    let mut l = vec![s("curves", "3"), f("t1", 300.0), f("t2", 600.0), f("t3", 900.0), f("ea", 20.0)];
    gas("O2", &mut l); v.push(("o2".into(), l));
    let mut l = vec![f("t1", 300.0), f("t2", 700.0), f("mark", 500.0), i("dec", 2)];
    gas("CO2", &mut l); v.push(("co2".into(), l));
    v.push(("custom1".into(), vec![s("gas", "Custom"), f("molar_mass", 1.0), f("t1", 50.0), f("t2", 3000.0), f("ea", 500.0)]));
    v.push(("custom1000".into(), vec![s("gas", "Custom"), f("molar_mass", 1000.0), f("t1", 3000.0), f("ea", 10.0)]));
    v.push(("unsorted".into(), vec![f("t1", 600.0), f("t2", 300.0)]));
    v.push(("duplicate".into(), vec![f("t1", 300.0), f("t2", 300.0)]));
    v.push(("crowded".into(), vec![s("curves", "3"), f("t1", 50.0), f("t2", 50.0), f("t3", 3000.0), f("ea", 100.0)]));
    v.push(("ea0markmax".into(), vec![f("ea", 0.0), f("mark", 20000.0)]));
    v.push(("eamax".into(), vec![f("ea", 500.0), f("mark", 1.0)]));
    v.push(("dec0".into(), vec![f("mark", 0.0), i("dec", 0)]));
    v.push(("nogrid".into(), vec![b("grid", false), f("mark", 845.3)]));
    v.push(("three100".into(), vec![s("curves", "3"), f("t1", 300.0), f("t2", 600.0), f("t3", 900.0), f("ea", 100.0), f("mark", 12000.0)]));
    v.push(("custom999".into(), vec![s("gas", "Custom"), f("molar_mass", 999.999), f("t1", 300.0), f("t2", 600.0)]));
    v.push(("hot".into(), vec![f("t1", 2999.9), f("t2", 3000.0), f("ea", 30.0)]));
    v.push(("h2hot".into(), vec![s("gas", "H2"), s("curves", "3"), f("t1", 50.0), f("t2", 150.0), f("t3", 3000.0), f("ea", 40.0)]));
    v.push(("mark1ea0".into(), vec![f("mark", 1.0), f("ea", 0.0)]));
    v.push(("markoff".into(), vec![s("gas", "He"), f("ea", 300.0), f("mark", 30000.0)]));
    v
}

#[test]
fn boltzmann_pngs() {
    let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
    let cases: Vec<(&str, Vec<(&'static str, Value)>)> = vec![
        ("defaults-key", vec![f("mark", 845.3)]),
        ("defaults-student", vec![f("mark", 845.3), b(SHOW_ANSWERS, false)]),
        ("three-large-key", vec![s("curves", "3"), f("t1", 300.0), f("t2", 600.0), f("t3", 900.0), f("ea", 20.0), f("mark", 1200.0), s("label_size", "Large")]),
        ("three-large-print", vec![s("curves", "3"), f("t1", 300.0), f("t2", 600.0), f("t3", 900.0), f("ea", 20.0), f("mark", 1200.0), s("label_size", "Large"), s("style", "Print")]),
        ("three-large-print-student", vec![s("curves", "3"), f("t1", 300.0), f("t2", 600.0), f("t3", 900.0), f("ea", 20.0), f("mark", 1200.0), s("label_size", "Large"), s("style", "Print"), b(SHOW_ANSWERS, false)]),
        ("co2", vec![s("gas", "CO2"), f("t1", 300.0), f("t2", 700.0), f("ea", 30.0), f("mark", 500.0)]),
        ("h2-cold", vec![s("gas", "H2"), f("t1", 100.0), f("t2", 300.0), f("ea", 5.0), f("mark", 3000.0)]),
        ("small", vec![s("label_size", "Small")]),
    ];
    for (tag, list) in cases {
        let svg = svg_of(&list);
        std::fs::write(format!("{dir}/boltzmann-{tag}.png"), to_png(&svg, 1.5).unwrap()).unwrap();
    }
}

#[test]
fn labels_clear_every_text_every_stroke_and_the_canvas_across_the_option_space() {
    let mut g = Glyphs::new();
    let mut problems: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut n = 0;
    for (name, base) in scenario_cases() {
        for style in ["Colour", "Print"] {
            for size in ["Small", "Medium", "Large"] {
                for ans in [true, false] {
                    let mut l = base.clone();
                    l.extend([s("style", style), s("label_size", size), b(SHOW_ANSWERS, ans)]);
                    let p = with(&l);
                    let svg = build(&p).0;
                    n += 1;
                    for m in audit(&svg, &mut g) {
                        let key: String = m.chars().filter(|c| !c.is_ascii_digit()).take(60).collect();
                        let e = problems.entry(key).or_default();
                        if e.len() < 3 { e.push(format!("{name}/{style}/{size}/{ans}: {m}")); }
                    }
                }
            }
        }
    }
    assert!(n >= 200, "{n} renders");
    assert!(problems.is_empty(), "{} kinds of problem:\n{}", problems.len(), problems.values().map(|v| v.join("\n")).collect::<Vec<_>>().join("\n\n"));
}
