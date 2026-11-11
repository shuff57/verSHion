//! Tests for batch 10: the particle diagram's lattice rows, columns and spacing and the Bohr
//! model's protons by hand, read off what is drawn.
use super::b10::{plan, Lat, No, Plan};
use super::*;
use crate::figures::interact::pick;
use crate::figures::{after_panel_change, answer_pair, param_relevant, registry, share, spec, start_params, FigureDef, Params, Value};

fn def(id: &str) -> FigureDef { registry().into_iter().find(|d| d.id == id).expect("registered") }
fn with(id: &str, list: &[(&'static str, Value)]) -> Params {
    let d = def(id);
    let mut p = start_params(&d);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }

fn attr(tag: &str, name: &str) -> Option<f64> {
    let k = format!(" {name}=\"");
    let i = tag.find(&k)? + k.len();
    tag[i..].split('"').next()?.parse().ok()
}
fn sattr<'a>(tag: &'a str, name: &str) -> &'a str {
    let k = format!(" {name}=\"");
    tag.find(&k).map(|i| tag[i + k.len()..].split('"').next().unwrap_or("")).unwrap_or("")
}
/// (cx, cy, r, fill) of every circle.
fn circles(svg: &str) -> Vec<(f64, f64, f64, String)> {
    svg.match_indices("<circle ").map(|(i, _)| {
        let tag = &svg[i..i + svg[i..].find('>').unwrap()];
        (attr(tag, "cx").unwrap(), attr(tag, "cy").unwrap(), attr(tag, "r").unwrap(), sattr(tag, "fill").to_string())
    }).collect()
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
fn kept(id: &str, p: &Params) {
    let d = def(id);
    let svg = d.svg(p);
    let q = spec::params_from_spec(&d, &spec::spec_for(id, p));
    assert_eq!(d.svg(&q), svg, "{id}: a saved figure redraws it");
    let link = share::encode(id, p, &start_params(&d));
    let (_, ch) = share::decode(&link).unwrap();
    assert_eq!(d.svg(&share::link_params(&d, ch, share::version(&link))), svg, "{id}: its link redraws it");
}

const PD: &str = "particle-diagram";
const BOHR: &str = "bohr-model";

/// The opening state's lattice (4 large light gray − ions, 4 small white + ions) with `more`.
fn lattice(more: &[(&'static str, Value)]) -> Params {
    let mut v = vec![s("layout", "Lattice"), s("state", "Solid")];
    v.extend(more.iter().cloned());
    with(PD, &v)
}
/// Particles drawn in the (first) box: circles inside it, the border excluded.
fn particles(svg: &str, x0: f64, x1: f64) -> Vec<(f64, f64, f64, String)> {
    circles(svg).into_iter().filter(|c| c.0 > x0 && c.0 < x1).collect()
}
fn in_box(svg: &str) -> Vec<(f64, f64, f64, String)> { particles(svg, 16.0, 316.0) }

// ---------------------------------------------------------------------------------------------
// Lattice size
// ---------------------------------------------------------------------------------------------

/// The plan: 0 by 0 sizes from the counts as before, a set size keeps the counts' ratio, one side
/// set takes the fewest of the other, and sizes that cannot keep the ratio are refused.
#[test]
fn a_set_lattice_keeps_the_counts_ratio() {
    let at = |r, c| Lat { rows: r, cols: c, spaced: false };
    assert_eq!(plan(&[4, 4], &at(0, 0)).unwrap(), Plan { rows: 3, cols: 3, counts: vec![4, 4], full: false });
    assert_eq!(plan(&[4, 4], &at(6, 6)).unwrap().counts, vec![18, 18]);
    assert_eq!(plan(&[12, 12], &at(2, 3)).unwrap().counts, vec![3, 3]);
    assert_eq!(plan(&[14, 2], &at(12, 12)).unwrap().counts, vec![126, 18]);
    assert_eq!(plan(&[4], &at(12, 12)).unwrap().counts, vec![144]);
    assert_eq!(plan(&[9, 3, 3], &at(5, 4)).unwrap().counts, vec![12, 4, 4]);
    assert_eq!(plan(&[4, 4], &at(5, 5)), Err(No::Ratio { rows: 5, cols: 5, unit: 2 }));
    assert_eq!(plan(&[2, 1], &at(4, 4)), Err(No::Ratio { rows: 4, cols: 4, unit: 3 }));
    // one side set: 3 rows of a 1 : 1 lattice of 8 need 4 columns (12 places, 6 of each)
    let p = plan(&[4, 4], &at(3, 0)).unwrap();
    assert_eq!((p.rows, p.cols, p.counts.clone()), (3, 4, vec![6, 6]));
    let p = plan(&[4, 4], &at(0, 5)).unwrap();
    assert_eq!((p.rows, p.cols, p.counts.clone()), (2, 5, vec![5, 5]));
    assert!(matches!(plan(&[24, 24, 24, 24], &at(1, 0)), Err(No::TooLong { need: 96, .. })));
    // every planned lattice is full, in the ratio, and within 12 by 12
    for a in 1..=24usize {
        for bb in 0..=24usize {
            let counts: Vec<usize> = [a, bb].into_iter().filter(|&k| k > 0).collect();
            for r in 0..=12 {
                for c in 0..=12 {
                    if let Ok(pl) = plan(&counts, &at(r, c)) {
                        assert!(pl.rows <= 12 && pl.cols <= 12);
                        if r + c > 0 {
                            assert_eq!(pl.counts.iter().sum::<usize>(), pl.rows * pl.cols, "{counts:?} {r}x{c}");
                            if counts.len() == 2 { assert_eq!(pl.counts[0] * counts[1], pl.counts[1] * counts[0], "{counts:?} {r}x{c}: ratio"); }
                            if r > 0 { assert_eq!(pl.rows, r); }
                            if c > 0 { assert_eq!(pl.cols, c); }
                        }
                    }
                }
            }
        }
    }
}

/// Through the renderer: a 6 by 6 lattice of the opening ions draws 36 particles, 18 of each,
/// alternating (every neighbour the other kind), and a 12 by 12 lattice 144, inside the box.
#[test]
fn rows_and_columns_draw_that_lattice() {
    let d = def(PD);
    let (light, white) = (shade("Light gray"), shade("White"));
    for (r, c) in [(6, 6), (2, 4), (12, 12), (12, 1), (1, 12), (4, 10)] {
        let p = lattice(&[i("lat_rows", r), i("lat_cols", c)]);
        let svg = d.svg(&p);
        let ps = in_box(&svg);
        assert_eq!(ps.len() as i64, r * c, "{r}x{c}");
        let nl = ps.iter().filter(|q| q.3 == light).count();
        assert_eq!(nl * 2, ps.len(), "{r}x{c}: half are the light ions");
        assert_eq!(ps.iter().filter(|q| q.3 == white).count(), nl);
        let xs: std::collections::BTreeSet<i64> = ps.iter().map(|q| (q.0 * 10.0).round() as i64).collect();
        let ys: std::collections::BTreeSet<i64> = ps.iter().map(|q| (q.1 * 10.0).round() as i64).collect();
        assert_eq!((ys.len() as i64, xs.len() as i64), (r, c), "{r}x{c}: rows and columns as drawn");
        for q in &ps {
            assert!(q.0 - q.2 >= 16.0 && q.0 + q.2 <= 316.0 && q.1 - q.2 >= 16.0 && q.1 + q.2 <= 316.0, "{r}x{c}: {q:?} leaves the box");
        }
        // alternating: the nearest neighbours of each are of the other kind
        let pitch = ps.iter().enumerate().flat_map(|(k, a)| ps.iter().skip(k + 1).map(move |bb| (a.0 - bb.0).hypot(a.1 - bb.1))).fold(f64::MAX, f64::min);
        for a in &ps {
            for bb in &ps {
                if ((a.0 - bb.0).hypot(a.1 - bb.1) - pitch).abs() < 0.01 { assert_ne!(a.3, bb.3, "{r}x{c}: two neighbours of one kind"); }
            }
        }
        kept(PD, &p);
    }
}

/// A substitutional alloy keeps its ratio and spread at a set size: 14 : 2 on 12 by 12 is 126 : 18,
/// no two of the 18 side by side.
#[test]
fn an_alloy_lattice_keeps_its_ratio_and_spread() {
    let d = def(PD);
    let p = lattice(&[i("a_count", 14), i("b_count", 2), s("a_size", "L"), s("b_size", "L"), s("a_shade", "Light gray"), s("b_shade", "Black"), s("a_charge", "None"), s("b_charge", "None"), i("lat_rows", 12), i("lat_cols", 12)]);
    let ps = in_box(&d.svg(&p));
    let dark: Vec<_> = ps.iter().filter(|q| q.3 == shade("Black")).collect();
    assert_eq!((ps.len(), dark.len()), (144, 18));
    let pitch = (ps[1].0 - ps[0].0).abs().max((ps[1].1 - ps[0].1).abs());
    for (k, a) in dark.iter().enumerate() {
        for bb in dark.iter().skip(k + 1) { assert!((a.0 - bb.0).hypot(a.1 - bb.1) > 1.5 * pitch, "two minority atoms side by side"); }
    }
}

/// A size that cannot hold the counts' ratio draws no particles; the panel note says why and what
/// would work, and nothing addressed to the teacher is drawn.
#[test]
fn a_size_that_breaks_the_ratio_is_refused_in_the_panel() {
    let d = def(PD);
    let p = lattice(&[i("lat_rows", 5), i("lat_cols", 5)]);
    let svg = d.svg(&p);
    assert!(in_box(&svg).is_empty(), "a refused lattice draws nothing");
    let n = crate::figures::chemistry::notes::key_note(PD, "lat_cols", &p).expect("a note");
    assert!(n.warn && n.text.contains("Nothing is drawn") && n.text.contains("multiple of 2"), "{}", n.text);
    for t in texts(&svg) { assert!(!n.text.contains(t.3.trim()) || t.3.trim().is_empty(), "note text drawn: {}", t.3); }
    assert!(!svg.contains("Nothing is drawn") && !svg.contains("multiple of"));
    // one side set too long for 12
    let p = lattice(&[i("a_count", 24), i("b_count", 24), i("lat_rows", 1)]);
    assert!(in_box(&d.svg(&p)).is_empty());
    let n = crate::figures::chemistry::notes::key_note(PD, "lat_cols", &p).unwrap();
    assert!(n.warn && n.text.contains("12 at most"), "{}", n.text);
    // a drawn lattice states what it holds, by the kind's name
    let p = lattice(&[i("lat_rows", 6), i("lat_cols", 6), s("a_name", "Cl⁻ ion")]);
    let n = crate::figures::chemistry::notes::key_note(PD, "lat_cols", &p).unwrap();
    assert!(!n.warn);
    assert_eq!(n.text, "6 rows by 6 columns: 18 Cl⁻ ion and 18 Ion 2, in the counts’ 4 : 4 ratio.");
    // before and after: each box on its own
    let p = lattice(&[s("boxes", "Before and after"), i("a2_count", 3), i("b2_count", 0), i("lat_rows", 2), i("lat_cols", 3)]);
    let svg = d.svg(&p);
    assert_eq!(in_box(&svg).len(), 6, "before: 3 and 3");
    assert_eq!(particles(&svg, 426.0, 726.0).len(), 6, "after: 6 of the one kind");
    let p = lattice(&[s("boxes", "Before and after"), i("a2_count", 2), i("b2_count", 1), i("lat_rows", 2), i("lat_cols", 2)]);
    let svg = d.svg(&p);
    assert_eq!(in_box(&svg).len(), 4);
    assert!(particles(&svg, 426.0, 726.0).is_empty(), "after: 2 : 1 does not fit 4 places");
    let n = crate::figures::chemistry::notes::key_note(PD, "lat_cols", &p).unwrap();
    assert!(n.warn && n.text.starts_with("Before: 2 rows by 2 columns") && n.text.contains("After: Nothing is drawn"), "{}", n.text);
}

// ---------------------------------------------------------------------------------------------
// Touching or spaced apart
// ---------------------------------------------------------------------------------------------

/// Gap between the nearest two particles of a lattice, edge to edge.
fn min_gap(ps: &[(f64, f64, f64, String)]) -> f64 {
    ps.iter().enumerate().flat_map(|(k, a)| ps.iter().skip(k + 1).map(move |q| (a.0 - q.0).hypot(a.1 - q.1) - a.2 - q.2)).fold(f64::MAX, f64::min)
}

/// Touching neighbours meet; spaced ones leave a clear gap, at every size from 1 by 2 to 12 by 12
/// and every particle size, and the two always draw differently.
#[test]
fn touching_or_spaced_apart() {
    let d = def(PD);
    for (r, c) in [(0, 0), (1, 2), (2, 4), (6, 6), (12, 12)] {
        for (sa, sb) in [("L", "S"), ("XL", "XL"), ("XS", "XS"), ("M", "XL")] {
            let base = [i("lat_rows", r), i("lat_cols", c), s("a_size", sa), s("b_size", sb)];
            let t = lattice(&[&base[..], &[s("lat_spacing", "Touching")]].concat());
            let sp = lattice(&[&base[..], &[s("lat_spacing", "Spaced apart")]].concat());
            let (st, ss) = (d.svg(&t), d.svg(&sp));
            assert_ne!(st, ss, "{r}x{c} {sa}/{sb}: the spacing changes nothing");
            let (pt, pss) = (in_box(&st), in_box(&ss));
            assert_eq!(pt.len(), pss.len());
            let (gt, gs) = (min_gap(&pt), min_gap(&pss));
            let rt = pt.iter().map(|q| q.2).fold(0.0, f64::max);
            assert!(gt < 0.05 * rt, "{r}x{c} {sa}/{sb}: touching leaves a gap of {gt} (radius {rt})");
            let rmax = pss.iter().map(|q| q.2).fold(0.0, f64::max);
            assert!(gs >= 0.25 * rmax && gs >= 3.0, "{r}x{c} {sa}/{sb}: spaced apart by only {gs} (radius {rmax})");
        }
    }
}

/// 12 by 12 at every chart-title style and both spacings: inside the page, charges at 8 px or
/// more, and the kinds told apart without colour (their greys differ).
#[test]
fn twelve_by_twelve_stays_readable() {
    let d = def(PD);
    let pd = (d.params)();
    let styles = match &pd.iter().find(|q| q.key == "title_style").unwrap().kind { crate::figures::Kind::Choice(o) => o.clone(), _ => unreachable!() };
    for st in styles {
        for spacing in ["Touching", "Spaced apart"] {
            for (ca, cb) in [("−", "+"), ("2−", "2+"), ("3−", "None")] {
                let p = lattice(&[i("lat_rows", 12), i("lat_cols", 12), s("lat_spacing", spacing), s("title_style", st), s("title", "Sodium chloride, a lattice of ions"), s("a_charge", ca), s("b_charge", cb), s("key_show", "Box and key")]);
                let svg = d.svg(&p);
                let (w, h) = crate::figures::svg_size(&svg);
                for q in circles(&svg) { assert!(q.0 - q.2 >= 0.0 && q.0 + q.2 <= w && q.1 - q.2 >= 0.0 && q.1 + q.2 <= h, "{st} {spacing}: {q:?} off the page"); }
                for t in texts(&svg) { assert!(t.2 >= 8.0, "{st} {spacing} {ca}: {t:?} under 8 px"); }
                let fills: std::collections::BTreeSet<String> = in_box(&svg).iter().map(|q| q.3.clone()).collect();
                assert_eq!(fills.len(), 2);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Shown exactly when they change the drawing
// ---------------------------------------------------------------------------------------------

/// Each lattice control is shown exactly when some value of it changes the drawing.
#[test]
fn lattice_controls_are_shown_when_they_act() {
    let d = def(PD);
    let modes: Vec<Vec<(&'static str, Value)>> = vec![
        vec![],
        vec![s("layout", "Lattice")],
        vec![s("layout", "Lattice"), s("key_show", "Key only")],
        vec![s("layout", "Lattice"), s("key_show", "Box and key")],
        vec![s("layout", "Lattice"), i("a_count", 0), i("b_count", 0)],
        vec![s("layout", "Lattice"), s("boxes", "Before and after"), i("a_count", 0), i("b_count", 0), i("a2_count", 3)],
        vec![s("layout", "Lattice"), s("boxes", "Before and after"), i("a_count", 0), i("b_count", 0), i("a2_count", 0), i("b2_count", 0)],
        vec![s("layout", "In a box"), s("state", "Solid")],
        vec![s("layout", "Lattice"), s("boxes", "Before and after")],
    ];
    for m in modes {
        let p = with(PD, &m);
        let svg = d.svg(&p);
        for (key, vals) in [("lat_rows", vec![i("lat_rows", 2), i("lat_rows", 12)]), ("lat_cols", vec![i("lat_cols", 2), i("lat_cols", 12)]), ("lat_spacing", vec![s("lat_spacing", "Spaced apart")])] {
            let changes = vals.iter().any(|v| { let mut q = p.clone(); q.0.insert(v.0.to_string(), v.1.clone()); d.svg(&q) != svg });
            assert_eq!(param_relevant(PD, key, &p), changes, "{key} in {m:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Bohr model: protons by hand
// ---------------------------------------------------------------------------------------------

fn bohr(more: &[(&'static str, Value)]) -> Params { with(BOHR, more) }

/// Balls of the proton colour and of the neutron colour in the nucleus.
fn nucleus(svg: &str) -> (usize, usize) {
    let cs = circles(svg);
    (cs.iter().filter(|c| c.3 == color9("Red")).count(), cs.iter().filter(|c| c.3 == color9("Gray")).count())
}

/// The teacher's proton count is drawn as set, whatever the element: balls, the written count, the
/// element's symbol and the shells unchanged.
#[test]
fn protons_by_hand_draw_as_set() {
    let d = def(BOHR);
    let carbon = d.svg(&bohr(&[]));
    assert_eq!(nucleus(&carbon), (6, 6));
    let electrons = |svg: &str| circles(svg).iter().filter(|c| c.3 == color9("Blue")).count();
    for pr in [0usize, 1, 5, 7, 26, 118, 119, 150, 200] {
        let p = bohr(&[i("protons", pr as i64)]);
        let svg = d.svg(&p);
        assert_eq!(nucleus(&svg), (pr, 6), "{pr} protons, carbon's 6 neutrons");
        assert_eq!(electrons(&svg), 6, "{pr}: the shells stay carbon's");
        let w = d.svg(&bohr(&[i("protons", pr as i64), s("nucleus", "Protons and neutrons")]));
        assert!(texts(&w).iter().any(|t| t.3 == format!("{pr} p⁺")), "{pr}: the written count");
        let sym = d.svg(&bohr(&[i("protons", pr as i64), s("nucleus", "Symbol")]));
        assert!(texts(&sym).iter().any(|t| t.3 == "C"), "the symbol is the element picked: the wrong atom draws as set");
        kept(BOHR, &p);
        // the balls stay in the nucleus, inside the first shell, even at 200 protons and 200 neutrons
        let big = d.svg(&bohr(&[i("protons", pr as i64), i("neutrons", 200)]));
        let g = bohr_geom(&bohr(&[i("protons", pr as i64), i("neutrons", 200)]));
        for c in circles(&big).iter().filter(|c| c.3 == color9("Red") || c.3 == color9("Gray")) {
            assert!((c.0 - g.cx).hypot(c.1 - g.cy) + c.2 < g.radii[0] - 2.0, "{pr}: a nucleus ball reaches the first shell");
        }
    }
    // -1 follows the element exactly
    assert_eq!(d.svg(&bohr(&[i("protons", -1)])), carbon);
    assert_eq!(d.svg(&bohr(&[i("protons", 6)])), carbon, "carbon's own 6 draws carbon");
    // out of range clamps
    assert_eq!(bohr(&[i("protons", 5000)]).i("protons"), 200);
}

/// The charge in brackets and the caption is protons minus the electrons drawn.
#[test]
fn the_stated_charge_is_counted_from_the_drawing() {
    let d = def(BOHR);
    let txt = |p: &Params| -> Vec<String> { let mut q = p.clone(); q.set_s("nucleus", "Symbol"); texts(&d.svg(&q)).into_iter().map(|t| t.3).collect() };
    // carbon's 6 electrons with 7 protons: 1+
    let t = txt(&bohr(&[s("nucleus", "Symbol"), i("protons", 7), b("brackets", true), b("caption", true)]));
    assert!(t.contains(&"+".to_string()) && t.contains(&"Carbon (C +)".to_string()), "{t:?}");
    // 0 protons: 6−
    let t = txt(&bohr(&[i("protons", 0), b("brackets", true), b("caption", true)]));
    assert!(t.contains(&"6−".to_string()) && t.contains(&"Carbon (C 6-)".to_string()), "{t:?}");
    // the charge control sets the shells; the charge stated follows the protons: 8 protons, C²⁺ shells (4 electrons) = 4+
    let t = txt(&bohr(&[i("protons", 8), i("charge", 2), b("brackets", true)]));
    assert!(t.contains(&"4+".to_string()), "{t:?}");
    // protons equal to electrons: neutral, no brackets
    let t = txt(&bohr(&[i("protons", 4), i("charge", 2), b("brackets", true), b("caption", true)]));
    assert!(t.contains(&"Carbon (C)".to_string()) && !t.iter().any(|x| x.ends_with('+') || x.ends_with('−')), "{t:?}");
    // typed shells are counted too: 2, 8 with 9 protons is 1−
    let t = txt(&bohr(&[i("protons", 9), s("shells_custom", "2, 8"), b("brackets", true)]));
    assert!(t.contains(&"−".to_string()), "{t:?}");
    // the stated charge stays on the page, up to three digits
    for pr in [0, 7, 30, 200] {
        let svg = d.svg(&bohr(&[s("nucleus", "Symbol"), i("protons", pr), b("brackets", true)]));
        let (w, _) = crate::figures::svg_size(&svg);
        for t in texts(&svg) {
            if t.3.ends_with('+') || t.3.ends_with('−') { assert!(t.0 + 10.5 * t.3.chars().count() as f64 <= w, "{pr}: {t:?} runs off the page {w}"); }
        }
    }
    // the panel note works it out the same way, and is never drawn
    let p = bohr(&[i("protons", 7), b("brackets", true)]);
    let n = crate::figures::chemistry::notes::key_note(BOHR, "protons", &p).unwrap();
    assert_eq!(n.text, "7 protons make nitrogen (N), not carbon: the atom is drawn as set, for students to spot. The charge shown is protons − electrons: 7 − 6 = 1+.");
    assert!(!d.svg(&p).contains("students to spot"));
    for (pr, want) in [(0, "No element has 0 protons."), (150, "No known element has 150 protons"), (6, "6 protons, as carbon has.")] {
        let n = crate::figures::chemistry::notes::key_note(BOHR, "protons", &bohr(&[i("protons", pr)])).unwrap();
        assert!(n.text.starts_with(want), "{}", n.text);
    }
    assert!(crate::figures::chemistry::notes::key_note(BOHR, "protons", &bohr(&[])).is_none());
}

/// Protons is shown exactly when it changes the drawing: a nucleus showing protons, or a charge stated.
#[test]
fn protons_are_shown_when_they_act() {
    let d = def(BOHR);
    for nuc in ["Symbol", "Protons and neutrons", "Blank", "Balls"] {
        for (cap, br) in [(false, false), (true, false), (false, true)] {
            let p = bohr(&[s("nucleus", nuc), b("caption", cap), b("brackets", br)]);
            let svg = d.svg(&p);
            let changes = [0, 3, 7, 200].iter().any(|&v| d.svg(&bohr(&[s("nucleus", nuc), b("caption", cap), b("brackets", br), i("protons", v)])) != svg);
            assert_eq!(param_relevant(BOHR, "protons", &p), changes, "{nuc} caption {cap} brackets {br}");
        }
    }
}

/// Picking an element sets its protons: the panel's element list and the nucleus click (through
/// the real hit test) drop a count typed by hand.
#[test]
fn picking_an_element_sets_its_protons() {
    let d = def(BOHR);
    let mut p = bohr(&[i("protons", 9)]);
    let old = Value::S(p.s("element").to_string());
    p.set_s("element", "8 O - Oxygen");
    after_panel_change(BOHR, "element", &old, &mut p);
    assert_eq!(p.i("protons"), -1);
    assert_eq!(nucleus(&d.svg(&p)).0, 8);
    let p = bohr(&[i("protons", 9)]);
    let svg = d.svg(&p);
    let hs = d.handles(&p, crate::figures::svg_size(&svg));
    let g = bohr_geom(&p);
    let ix = pick(&hs, (g.cx, g.cy), 1.0).expect("the nucleus");
    assert!(hs[ix].tip.starts_with("Click: next element"));
    let mut q = p.clone();
    (hs[ix].click.as_ref().unwrap())(&mut q);
    assert_eq!((q.s("element"), q.i("protons")), ("7 N - Nitrogen", -1));
    // the other figures' element changes leave them alone
    let mut o = with("orbital-diagram", &[]);
    let before = o.clone();
    after_panel_change("orbital-diagram", "element", &Value::S(String::new()), &mut o);
    assert!(o.0 == before.0);
}

/// The student copy hides the nucleus (and so the protons) as before.
#[test]
fn the_student_copy_hides_the_hand_set_protons() {
    let d = def(BOHR);
    let (student, key) = answer_pair(&d, &bohr(&[i("protons", 7), s("nucleus", "Protons and neutrons")])).unwrap();
    assert_eq!(nucleus(&student), (0, 0));
    assert!(!student.contains("7 p⁺") && !student.contains("6 n⁰"));
    assert!(key.contains("7 p⁺"), "the key shows the protons as set");
}
