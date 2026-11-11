//! Batch 9 tests: predator-prey's blank axes (Neither), and population growth's blank-line labels and setups.
use crate::figures::{registry, start_params, FigureDef, Params, Value};

pub(super) fn def(id: &str) -> FigureDef { registry().into_iter().find(|d| d.id == id).unwrap() }
pub(super) fn params(id: &str, over: &[(&str, Value)]) -> Params {
    let d = def(id);
    let mut p = start_params(&d);
    for (k, v) in over { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
pub(super) fn s(v: &str) -> Value { Value::S(v.into()) }
pub(super) fn b(v: bool) -> Value { Value::B(v) }

/// The text of every <text> element, in order.
pub(super) fn texts(svg: &str) -> Vec<String> {
    let mut out = vec![];
    for part in svg.split("<text").skip(1) {
        let body = part.split_once('>').map(|x| x.1).unwrap_or("");
        let body = body.split("</text>").next().unwrap_or("");
        // drop tspans' tags, keep their text
        let mut t = String::new();
        let mut inside = false;
        for c in body.chars() { match c { '<' => inside = true, '>' => inside = false, _ if !inside => t.push(c), _ => {} } }
        out.push(t);
    }
    out
}

/// Polylines and paths with many points: the drawn curves.
fn curves(svg: &str) -> usize {
    svg.split("<polyline").skip(1).filter(|e| e.split("points=\"").nth(1).map(|x| x.split('"').next().unwrap().split_whitespace().count() > 60).unwrap_or(false)).count()
        + svg.split("<path").skip(1).filter(|e| e.split(" d=\"").nth(1).map(|x| x.split('"').next().unwrap().matches('L').count() > 60).unwrap_or(false)).count()
}

const PP: &str = "predator-prey";

#[test]
fn predator_prey_neither_draws_blank_axes() {
    let d = def(PP);
    let mut seen = vec![];
    for show in ["Both", "Prey only", "Predators only", super::NEITHER] {
        let svg = d.svg(&params(PP, &[("show", s(show))]));
        assert!(!seen.contains(&svg), "{show} draws like another choice");
        seen.push(svg);
    }
    for pair in ["Snowshoe hare and Canada lynx", "Moose and wolf (Isle Royale)", "Aphid and ladybug"] {
        for yax in ["One for both", "Predators on the right"] {
            for data in ["Smooth curves", "Census counts", "Points each time unit"] {
                let mut over = vec![("pair", s(pair)), ("y_axis", s(yax)), ("data", s(data))];
                for (k, v) in super::pair_params(pair) { over.push((k, v)); }
                let both = d.svg(&params(PP, &over));
                over.push(("show", s(super::NEITHER)));
                let blank = d.svg(&params(PP, &over));
                // the axes, their numbers, the titles and the legend's key are those of both populations
                assert_eq!(texts(&blank), texts(&both), "{pair} {yax} {data}: the blank axes keep every number and title");
                assert!(curves(&both) >= 1 || data != "Smooth curves");
                assert_eq!(curves(&blank), 0, "{pair} {yax} {data}: no curve");
                assert!(!blank.contains("<circle") || data == "Smooth curves" && !blank.contains("<circle"), "no census dots or points");
                // nothing that could give an answer away, whatever is marked
                let mut marked = over.clone();
                marked.extend([("marks", s("Peaks")), ("mark_lag", b(true)), ("mark_period", b(true)), ("lag_labels", s("Name and length"))]);
                assert_eq!(d.svg(&params(PP, &marked)), blank, "{pair} {yax} {data}: peaks, lag and period are not marked on blank axes");
            }
        }
    }
}

#[test]
fn predator_prey_neither_keeps_both_axis_ranges() {
    // one population's range would be narrower: the blank graph has room for both, as Both does
    let d = def(PP);
    for pair in ["Snowshoe hare and Canada lynx", "Sea lion and orca"] {
        let mut over: Vec<(&str, Value)> = vec![("pair", s(pair)), ("y_axis", s("One for both"))];
        for (k, v) in super::pair_params(pair) { over.push((k, v)); }
        let both = d.svg(&params(PP, &over));
        let prey = { let mut o = over.clone(); o.push(("show", s("Prey only"))); d.svg(&params(PP, &o)) };
        let pred = { let mut o = over.clone(); o.push(("show", s("Predators only"))); d.svg(&params(PP, &o)) };
        over.push(("show", s(super::NEITHER)));
        let blank = d.svg(&params(PP, &over));
        let nums = |svg: &str| texts(svg).into_iter().filter(|t| t.chars().all(|c| c.is_ascii_digit() || c == '.')).collect::<Vec<_>>();
        assert_eq!(nums(&blank), nums(&both));
        assert!(nums(&blank) != nums(&pred) || nums(&blank) != nums(&prey));
    }
}

// ------------------------------------------------------------ population growth: inflection and phase labels

const PG: &str = "population-growth";
type Bx = (f64, f64, f64, f64);

fn walk(g: &resvg::usvg::Group, out: &mut Vec<(String, Bx)>) {
    for n in g.children() {
        match n {
            resvg::usvg::Node::Group(g) => walk(g, out),
            resvg::usvg::Node::Text(t) => {
                let r = t.flattened().bounding_box().transform(t.abs_transform()).unwrap_or(t.flattened().abs_bounding_box());
                let txt: String = t.chunks().iter().map(|c| c.text()).collect();
                out.push((txt, (r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64)));
            }
            _ => {}
        }
    }
}
/// Text ink boxes from the real renderer.
pub(super) fn ink_texts(svg: &str) -> Vec<(String, Bx)> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(&crate::figures::serif_stack(svg), &opt).expect("the SVG parses");
    let mut v = Vec::new();
    walk(tree.root(), &mut v);
    v.retain(|(t, bx)| !t.trim().is_empty() && bx.2 > bx.0);
    v
}

fn attr<'a>(el: &'a str, k: &str) -> Option<&'a str> { Some(el.split(&format!(" {k}=\"")).nth(1)?.split('"').next()?) }
/// Dark strokes as segments (lines and polylines), with the element's text: light gridlines left out. A stroke inside
/// a clip group keeps only what the clip shows (the curves run past the plot and are clipped to it).
pub(super) fn dark_segments(svg: &str) -> Vec<((f64, f64), (f64, f64), String)> {
    let dark = |el: &str| {
        let Some(c) = attr(el, "stroke") else { return false };
        if c == "none" { return false; }
        let h = c.trim_start_matches('#');
        if h.len() != 6 { return true; }
        let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0) as u32;
        v(0) + v(2) + v(4) < 3 * 0xa0
    };
    // clip rectangles by id, and the stack of open groups (Some(rect) for a clip group)
    let mut clips: std::collections::HashMap<String, (f64, f64, f64, f64)> = Default::default();
    for part in svg.split("<clipPath").skip(1) {
        let id = attr(part, "id").unwrap().to_string();
        let r = part.split("<rect").nth(1).unwrap();
        let f = |k| attr(r, k).unwrap().parse::<f64>().unwrap();
        clips.insert(id, (f("x"), f("y"), f("x") + f("width"), f("y") + f("height")));
    }
    let mut stack: Vec<Option<(f64, f64, f64, f64)>> = vec![];
    let mut out = vec![];
    for tok in svg.split('<').skip(1) {
        let el = tok.split('>').next().unwrap();
        if el.starts_with("g ") || el == "g" {
            let c = attr(el, "clip-path").and_then(|u| clips.get(u.trim_start_matches("url(#").trim_end_matches(')')).copied());
            stack.push(c);
            continue;
        }
        if el.starts_with("/g") { stack.pop(); continue; }
        let tag = if el.starts_with("line ") { "line" } else if el.starts_with("polyline ") { "polyline" } else { continue };
        if !dark(el) { continue; }
        let clip = stack.iter().flatten().last().copied();
        let pts: Vec<(f64, f64)> = if tag == "line" {
            let f = |k| attr(el, k).unwrap().parse::<f64>().unwrap();
            vec![(f("x1"), f("y1")), (f("x2"), f("y2"))]
        } else {
            attr(el, "points").unwrap().split_whitespace().map(|q| { let (a, b) = q.split_once(',').unwrap(); (a.parse().unwrap(), b.parse().unwrap()) }).collect()
        };
        for w in pts.windows(2) {
            match clip {
                None => out.push((w[0], w[1], el.to_string())),
                Some((x0, y0, x1, y1)) => {
                    // keep the shown pieces, cut finely
                    let n = (((w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs()) / 2.0).ceil().max(1.0) as usize;
                    let at = |i: usize| { let t = i as f64 / n as f64; (w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t) };
                    let inside = |q: (f64, f64)| q.0 >= x0 && q.0 <= x1 && q.1 >= y0 && q.1 <= y1;
                    for i in 0..n { let (a, b) = (at(i), at(i + 1)); if inside(a) && inside(b) { out.push((a, b, el.to_string())); } }
                }
            }
        }
    }
    out
}
fn seg_dist(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> f64 {
    let mut m = f64::MAX;
    for i in 0..=60 {
        let t = i as f64 / 60.0;
        let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        let (dx, dy) = (d.0 - c.0, d.1 - c.1);
        let l2 = (dx * dx + dy * dy).max(1e-12);
        let u = (((p.0 - c.0) * dx + (p.1 - c.1) * dy) / l2).clamp(0.0, 1.0);
        let q = (c.0 + dx * u, c.1 + dy * u);
        m = m.min(((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt());
    }
    m
}

const PHASES: [&str; 3] = ["Lag phase", "Exponential phase", "Stationary phase"];
const INFL: &str = "Inflection point (N = K/2)";

/// Setups that draw the inflection point and phases on the population graph: the examples and classic setups, and
/// a sweep of rates, capacities, starts and spans at every label size.
fn growth_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut v: Vec<Vec<(&'static str, Value)>> = vec![];
    for e in super::growth_examples().into_iter().chain(super::growth_classics()) { v.push(e.params); }
    for r in [0.05, 0.3, 0.5, 1.2, 5.0] {
        for (n0, k) in [(1.0, 1000.0), (20.0, 1000.0), (400.0, 1000.0), (10.0, 1_000_000.0)] {
            for tspan in [5.0, 20.0, 200.0] {
                v.push(vec![("r", Value::F(r)), ("n0", Value::F(n0)), ("k", Value::F(k)), ("tspan", Value::F(tspan))]);
            }
        }
    }
    for g in ["Population size and growth rate", "Population size and per capita rate"] { v.push(vec![("graph", s(g))]); }
    v.push(vec![("growth", s("Exponential and logistic")), ("n0", Value::F(10.0))]);
    v.push(vec![("y_end", s("Text")), ("y_hi_end", s("Triangle arrow")), ("title", s("Yeast in a flask"))]);
    let mut out = vec![];
    for c in v {
        for ls in ["Small", "Medium", "Large"] {
            let mut x = c.clone();
            x.extend([("mark_infl", b(true)), ("mark_phases", b(true)), ("label_size", s(ls))]);
            out.push(x);
        }
    }
    out
}

#[test]
fn growth_labels_text_is_the_old_drawing() {
    let d = def(PG);
    for c in growth_cases().into_iter().take(12) {
        let a = d.svg(&params(PG, &c));
        let mut x = c.clone();
        x.extend([("infl_label_style", s("Text")), ("phase_label_style", s("Text"))]);
        assert_eq!(d.svg(&params(PG, &x)), a);
    }
}

#[test]
fn growth_inflection_and_phase_labels_blank_or_none_leave_nothing_of_the_words() {
    let d = def(PG);
    let mut n = 0;
    for c in growth_cases() {
        let p = params(PG, &c);
        if !super::growth_b9::infl_labelled(&p) || !super::growth_b9::phases_drawn(&p) { continue; }
        let full = d.svg(&p);
        let drawn_phases: Vec<&str> = PHASES.iter().copied().filter(|ph| full.contains(ph)).collect();
        assert!(full.contains(INFL));
        for (key, words) in [("infl_label_style", vec![INFL]), ("phase_label_style", drawn_phases.clone())] {
            let mut bl = c.clone();
            bl.push((key, s("Blank line")));
            let bs = d.svg(&params(PG, &bl));
            let mut no = c.clone();
            no.push((key, s("None")));
            let ns = d.svg(&params(PG, &no));
            assert!(bs != full && ns != full && bs != ns, "{key}: three different drawings in {c:?}");
            for svg in [&bs, &ns] {
                // nowhere in the SVG, its title and description included
                for w in &words { assert!(!svg.contains(w), "{key}: {w} left in {c:?}"); }
            }
            // the blank lines: one for the point, one per phase drawn; None draws none
            let lines = |svg: &str| svg.matches("<line").count();
            // (a phase's line only over a part of its bracket at least 20 px wide inside the plot)
            let want = if key == "infl_label_style" { 1 } else { drawn_phases.len() };
            if key == "infl_label_style" { assert_eq!(lines(&bs), lines(&full) + want, "{key}: blank lines in {c:?}"); }
            else { assert!(lines(&bs) > lines(&full) && lines(&bs) <= lines(&full) + want, "{key}: blank lines in {c:?}"); }
            assert_eq!(lines(&ns), lines(&full), "{key}: None draws no line in {c:?}");
            // the other mark's label is untouched
            let other: Vec<&str> = if key == "infl_label_style" { drawn_phases.clone() } else { vec![INFL] };
            for w in other { assert!(bs.contains(w) && ns.contains(w)); }
        }
        n += 1;
    }
    assert!(n >= 60, "{n} setups drew both marks");
}

/// Every blank line is clear of every dark stroke (the curve, the guides, the point, the brackets, the K line, the
/// axes) by 3 px, of every word and number as drawn, of each other, and inside the page; each phase's line is over its
/// own bracket and the point's line nearer the point than any other blank.
#[test]
fn growth_blank_lines_clear_of_strokes_and_text() {
    let d = def(PG);
    let mut n = 0;
    for c in growth_cases() {
        let mut x = c.clone();
        x.extend([("infl_label_style", s("Blank line")), ("phase_label_style", s("Blank line"))]);
        let p = params(PG, &x);
        if !super::growth_b9::infl_labelled(&p) { continue; }
        let svg = d.svg(&p);
        let none = { let mut y = c.clone(); y.extend([("infl_label_style", s("None")), ("phase_label_style", s("None"))]); d.svg(&params(PG, &y)) };
        let before = dark_segments(&none);
        let blanks: Vec<((f64, f64), (f64, f64))> = dark_segments(&svg).into_iter().filter(|q| !before.iter().any(|b| b.2 == q.2)).map(|q| (q.0, q.1)).collect();
        assert!(!blanks.is_empty());
        let vb: Vec<f64> = attr(&svg, "viewBox").unwrap().split_whitespace().map(|v| v.parse().unwrap()).collect();
        let texts = ink_texts(&svg);
        for (i, bl) in blanks.iter().enumerate() {
            assert!(bl.0 .0 >= 0.0 && bl.1 .0 <= vb[2] && bl.0 .1 >= 0.0 && bl.0 .1 <= vb[3], "blank {bl:?} off the page in {c:?}");
            for o in &before {
                // a gridline (1 px or 0.5 px, upright or level) may cross a blank line at right angles, as it crosses
                // every word on the graph paper, but never run along it; every other stroke stays 3 px away
                let w = attr(&o.2, "stroke-width").unwrap_or("1");
                let upright = (o.0 .0 - o.1 .0).abs() < 1e-6;
                let grid = matches!(w, "1" | "0.5") && (upright || (o.0 .1 - o.1 .1).abs() < 1e-6);
                if grid && upright { continue; }
                let dist = seg_dist(bl.0, bl.1, o.0, o.1);
                assert!(dist >= 3.0, "blank {bl:?} {dist:.1} px from a stroke {:?}-{:?} (width {w}) in {c:?}", o.0, o.1);
            }
            for (j, other) in blanks.iter().enumerate() {
                if i != j { assert!(seg_dist(bl.0, bl.1, other.0, other.1) >= 3.0, "two blank lines touch in {c:?}"); }
            }
            let bb = (bl.0 .0.min(bl.1 .0), bl.0 .1 - 1.0, bl.0 .0.max(bl.1 .0), bl.0 .1 + 1.0);
            for (t, tb) in &texts {
                assert!(!(bb.0 < tb.2 && tb.0 < bb.2 && bb.1 < tb.3 && tb.1 < bb.3), "blank {bl:?} on {t:?} {tb:?} in {c:?}");
            }
        }
        n += 1;
    }
    assert!(n >= 60, "{n}");
}

#[test]
fn growth_label_modes_offered_where_they_draw() {
    let d = def(PG);
    let cases: Vec<Vec<(&str, Value)>> = vec![
        vec![],
        vec![("mark_infl", b(true)), ("mark_phases", b(true))],
        vec![("mark_infl", b(true)), ("mark_phases", b(true)), ("graph", s("Growth rate over time"))],
        vec![("mark_infl", b(true)), ("mark_phases", b(true)), ("graph", s("Growth rate and per capita rate"))],
        vec![("mark_infl", b(true)), ("mark_phases", b(true)), ("growth", s("Exponential (J-curve)"))],
        vec![("mark_infl", b(true)), ("mark_phases", b(true)), ("n0", Value::F(600.0))],
        vec![("mark_infl", b(true)), ("mark_phases", b(true)), ("blank", b(true))],
        vec![("mark_infl", b(true)), ("mark_phases", b(true)), ("graph", s("Population size and growth rate"))],
        vec![("mark_infl", b(true))],
        vec![("mark_phases", b(true))],
    ];
    for c in cases {
        let p = params(PG, &c);
        let a = d.svg(&p);
        for key in ["infl_label_style", "phase_label_style"] {
            for v in ["Blank line", "None"] {
                let mut q = p.clone();
                q.set_s(key, v);
                assert_eq!(crate::figures::param_relevant(PG, key, &p), d.svg(&q) != a, "{key} -> {v} in {c:?}");
            }
        }
    }
}
