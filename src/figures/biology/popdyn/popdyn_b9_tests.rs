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
                    // the part the clip shows (Liang-Barsky; a curve can run to huge coordinates far off the plot)
                    let (p0, d) = (w[0], (w[1].0 - w[0].0, w[1].1 - w[0].1));
                    if !(p0.0.is_finite() && p0.1.is_finite() && d.0.is_finite() && d.1.is_finite()) { continue; }
                    let (mut t0, mut t1) = (0.0f64, 1.0f64);
                    let mut ok = true;
                    for (pp, q) in [(-d.0, p0.0 - x0), (d.0, x1 - p0.0), (-d.1, p0.1 - y0), (d.1, y1 - p0.1)] {
                        if pp.abs() < 1e-12 { if q < 0.0 { ok = false; } continue; }
                        let r = q / pp;
                        if pp < 0.0 { t0 = t0.max(r); } else { t1 = t1.min(r); }
                    }
                    if ok && t0 < t1 { out.push(((p0.0 + d.0 * t0, p0.1 + d.1 * t0), (p0.0 + d.0 * t1, p0.1 + d.1 * t1), el.to_string())); }
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
            // the blank lines: one for the point, one per phase bracket shown; None draws none
            let tag = if key == "infl_label_style" { "data-blank=\"infl\"" } else { "data-blank=\"phase-" };
            let want = if key == "infl_label_style" { 1 } else { phase_brackets(&bs, &p).len() };
            assert_eq!(bs.matches(tag).count(), want, "{key}: blank lines in {c:?}");
            assert_eq!(ns.matches("data-blank").count(), 0, "{key}: None draws no line in {c:?}");
            // the other mark's label is untouched
            let other: Vec<&str> = if key == "infl_label_style" { drawn_phases.clone() } else { vec![INFL] };
            for w in other { assert!(bs.contains(w) && ns.contains(w)); }
        }
        n += 1;
    }
    assert!(n >= 60, "{n} setups drew both marks");
}

/// The phase brackets as drawn on the population graph (four-point polylines: two 6 px feet and a top), with the part
/// of each inside the plot: (left, right, top y, visible left, visible right), left to right.
fn phase_brackets(svg: &str, p: &Params) -> Vec<(f64, f64, f64, f64, f64)> {
    let ax = super::axes_of(p, false).into_iter().find(|a| a.kind == 0).expect("the population graph");
    let mut out = vec![];
    for el in svg.split("<polyline").skip(1) {
        let el = el.split('>').next().unwrap();
        let pts: Vec<(f64, f64)> = attr(el, "points").unwrap().split_whitespace().map(|q| { let (a, b) = q.split_once(',').unwrap(); (a.parse().unwrap(), b.parse().unwrap()) }).collect();
        if pts.len() == 4 && pts[0].0 == pts[1].0 && pts[2].0 == pts[3].0 && pts[1].1 == pts[2].1 && (pts[0].1 - pts[1].1 - 6.0).abs() < 1e-6 && pts[0].1 < ax.y {
            let (va, vb) = (pts[1].0.max(ax.x), pts[2].0.min(ax.x + ax.w));
            if vb > va { out.push((pts[1].0, pts[2].0, pts[1].1, va, vb)); }
        }
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    out
}

/// The tagged blank lines and leaders: (tag, (x1, y1), (x2, y2)).
fn tagged(svg: &str, what: &str) -> Vec<(String, (f64, f64), (f64, f64))> {
    svg.split("<line").skip(1).filter_map(|el| {
        let el = el.split('>').next().unwrap();
        let t = attr(el, what)?.to_string();
        let f = |k| attr(el, k).unwrap().parse::<f64>().unwrap();
        Some((t, (f("x1"), f("y1")), (f("x2"), f("y2"))))
    }).collect()
}

fn pt_seg(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 { seg_dist(p, p, a, b) }

const VIEWS: [&str; 9] = ["Population size over time", "Growth rate over time", "Per capita growth rate over time", "Population size and growth rate",
    "Population size and per capita rate", "Growth rate and per capita rate", "Growth rate against population size", "Per capita rate against population size", "Both rates against population size"];

/// The checker's sweep: every graph view, every minor gridline setting, every label size, the ten setups, and a seeded
/// mix of the controls, with both marks and both blank lines on. Wherever a mark is drawn, its Blank line is drawn, at
/// least 40 px long, inside the page, clear of the curves (3 px, as clipped) and of every word and number as drawn,
/// not on another blank line, and its own: the point's line nearer the point than any phase bracket or the K line, or
/// tied to it by a leader starting at the point; each phase's line over its own bracket and nearer it than any other,
/// or tied to it by a leader standing on its own bracket.
#[test]
fn growth_blank_lines_always_drawn_long_clear_and_own() {
    let d = def(PG);
    let mut cases: Vec<Vec<(&'static str, Value)>> = vec![];
    let setups = crate::figures::classic_setups(PG);
    for view in VIEWS {
        for minor in ["None", "2 per block", "4 per block", "5 per block"] {
            for ls in ["Small", "Medium", "Large"] {
                for e in &setups {
                    let mut c = e.params.clone();
                    c.extend([("graph", s(view)), ("minor", s(minor)), ("label_size", s(ls))]);
                    cases.push(c);
                }
            }
        }
    }
    // each case again with two seeded mixes of the setup's rate, start, capacity and span
    let mut seed: u64 = 0xb9_52;
    let mut next = |n: u64| { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (seed >> 33) % n };
    let own: Vec<Vec<(&'static str, Value)>> = cases.clone();
    for c in own {
        for _ in 0..2 {
            let mut m = c.clone();
            m.retain(|(k, _)| !matches!(*k, "r" | "n0" | "k" | "tspan"));
            let k = [50.0, 100.0, 665.0, 1000.0, 6000.0][next(5) as usize];
            m.extend([
                ("r", Value::F([0.05, 0.2, 0.54, 1.0, 1.879, 2.861, 5.0][next(7) as usize])),
                ("n0", Value::F(k * [0.005, 0.02, 0.131, 0.3][next(4) as usize])),
                ("k", Value::F(k)),
                ("tspan", Value::F([5.0, 18.0, 20.0, 40.0, 100.0][next(5) as usize])),
            ]);
            cases.push(m);
        }
    }
    // the checkers' own cases
    let yeast = setups.iter().find(|e| e.name == "Yeast in a flask").unwrap().params.clone();
    let with = |base: &Vec<(&'static str, Value)>, extra: Vec<(&'static str, Value)>| { let mut v = base.clone(); v.retain(|q| !extra.iter().any(|e| e.0 == q.0)); v.extend(extra); v };
    cases.push(with(&yeast, vec![("graph", s("Population size over time")), ("minor", s("4 per block")), ("label_size", s("Small")), ("r", Value::F(1.879)), ("n0", Value::F(13.1)), ("k", Value::F(100.0)), ("tspan", Value::F(5.0))]));
    cases.push(with(&yeast, vec![("graph", s("Population size and per capita rate")), ("label_size", s("Large")), ("r", Value::F(2.861)), ("n0", Value::F(4.0)), ("k", Value::F(1000.0)), ("tspan", Value::F(20.0))]));
    cases.push(vec![("r", Value::F(0.2)), ("n0", Value::F(2.0)), ("k", Value::F(50.0)), ("tspan", Value::F(100.0))]);
    cases.push(vec![("graph", s("Population size and per capita rate")), ("minor", s("4 per block"))]);
    assert!(cases.len() >= 3240);
    let (mut n_visible, mut n_far, mut n_point_leader, mut unowned, mut n_cross) = (0, 0, 0, 0, 0);
    let n_cases = cases.len();
    let (mut n_infl, mut n_phase, mut n_lead) = (0, 0, 0);
    for c in cases {
        let mut x = c.clone();
        x.extend([("mark_infl", b(true)), ("mark_phases", b(true)), ("infl_label_style", s("Blank line")), ("phase_label_style", s("Blank line"))]);
        let p = params(PG, &x);
        let svg = d.svg(&p);
        let blanks = tagged(&svg, "data-blank");
        let leaders = tagged(&svg, "data-leader");
        let vb: Vec<f64> = attr(&svg, "viewBox").unwrap().split_whitespace().map(|v| v.parse().unwrap()).collect();
        let texts = ink_texts(&svg);
        let curves: Vec<_> = dark_segments(&svg).into_iter().filter(|q| attr(&q.2, "points").is_some_and(|ps| ps.split_whitespace().count() > 50)).collect();
        let brackets = if super::growth_b9::phases_drawn(&p) { phase_brackets(&svg, &p) } else { vec![] };
        let ax = super::axes_of(&p, false).into_iter().find(|a| a.kind == 0);
        // every mark drawn has its line
        let want_infl = super::growth_b9::infl_labelled(&p) as usize;
        assert_eq!(blanks.iter().filter(|q| q.0 == "infl").count(), want_infl, "the point's blank line in {c:?}");
        assert_eq!(blanks.iter().filter(|q| q.0.starts_with("phase")).count(), brackets.len(), "the phases' blank lines in {c:?}");
        for (k, (tag, a, bb)) in blanks.iter().enumerate() {
            let len = (bb.0 - a.0).abs();
            assert!(len >= 40.0 - 1e-6, "{tag}: {len:.1} px long in {c:?}");
            assert!(a.0.min(bb.0) >= 0.0 && a.0.max(bb.0) <= vb[2] && a.1 >= 0.0 && a.1 <= vb[3], "{tag} off the page in {c:?}");
            for cv in &curves { assert!(seg_dist(*a, *bb, cv.0, cv.1) >= 3.0, "{tag} on a curve in {c:?}"); }
            let bx = (a.0.min(bb.0), a.1 - 1.0, a.0.max(bb.0), a.1 + 1.0);
            for (t, tb) in &texts { assert!(!(bx.0 < tb.2 && tb.0 < bx.2 && bx.1 < tb.3 && tb.1 < bx.3), "{tag} on {t:?} in {c:?}"); }
            for (j, o) in blanks.iter().enumerate() { if j != k { assert!(seg_dist(*a, *bb, o.1, o.2) >= 3.0, "{tag} on {} in {c:?}", o.0); } }
            let lead = leaders.iter().find(|l| l.0 == *tag);
            if let Some(l) = lead {
                // a leader: one end on the line, the other on its own mark
                assert!(pt_seg(l.2, *a, *bb) <= 4.0, "{tag}: the leader reaches its line in {c:?}");
                // (the point's leader may cross a curve only where every spot needs that: the point boxed in by the
                // curves, the K line and the plot's edge; counted)
                if curves.iter().any(|cv| seg_dist(l.1, l.2, cv.0, cv.1) < 2.0) {
                    assert!(tag == "infl", "{tag}: the leader crosses a curve in {c:?}");
                    n_cross += 1;
                }
                n_lead += 1;
            }
            if tag == "infl" {
                n_infl += 1;
                let ax = ax.as_ref().unwrap();
                let ti = super::growth_b9::t_inf(&p).unwrap();
                let pt = (ax.px(ti), ax.py(p.f("k").max(1.0) / 2.0));
                if !ax.inside((ti, p.f("k").max(1.0) / 2.0)) { continue; }
                n_visible += 1;
                let dd = |q: (f64, f64)| ((q.0 - pt.0).powi(2) + (q.1 - pt.1).powi(2)).sqrt();
                // within reach: its nearest end at most 60 px from the point
                let near_end = dd(*a).min(dd(*bb));
                if near_end > 60.0 { n_far += 1; }
                // (farther only where nothing within 60 px is clear, and then always with a leader from the point)
                if near_end > 60.0 { assert!(lead.is_some(), "the point's line's nearest end is {near_end:.0} px from it, with no leader, in {c:?}"); }
                // a leader from the point whenever the line is not right beside it
                if near_end > 14.5 { assert!(lead.is_some(), "the point's line {near_end:.0} px away has no leader in {c:?}"); }
                if let Some(l) = lead {
                    n_point_leader += 1;
                    assert!(dd(l.1) <= 9.0, "the point's leader starts at the point in {c:?}");
                }
                // its own: nearer the point than any census count (never over one) or the K line
                let own = pt_seg(pt, *a, *bb);
                for el in svg.split("<circle").skip(1) {
                    let f = |k: &str| -> f64 { el.split(&format!(" {k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
                    if (f("r") - 4.2).abs() > 1e-6 { continue; }
                    let dot = (f("cx"), f("cy"));
                    let dl = pt_seg(dot, *a, *bb);
                    assert!(dl >= 5.2, "the point's line over a census count in {c:?}");
                    // nearer its point than any count; where no spot on the graph is (counts all round the point), the
                    // leader from the point says whose line it is
                    if dl <= own { assert!(lead.is_some(), "the point's line nearer a census count ({dl:.0} px) than its point ({own:.0} px), with no leader, in {c:?}"); unowned += 1; }
                }
                if p.b("mark_k") && p.s("growth") != "Exponential (J-curve)" {
                    let ky = ax.py(p.f("k").max(1.0));
                    if ky >= ax.y && ky <= ax.y + ax.h && own >= seg_dist(*a, *bb, (ax.x, ky), (ax.x + ax.w, ky)) {
                        assert!(lead.is_some(), "the point's line nearer the K line, with no leader, in {c:?}");
                        unowned += 1;
                    }
                }
                if lead.is_none() {
                    for (ba, bbx, by, _, _) in &brackets { assert!(own < seg_dist(*a, *bb, (*ba, *by), (*bbx, *by)), "the point's line nearer a phase bracket in {c:?}"); }
                }
            } else {
                n_phase += 1;
                let i: usize = tag.trim_start_matches("phase-").parse().unwrap();
                // the brackets drawn are the phases in order; this line's is the one with its index among those present
                let present: Vec<usize> = blanks.iter().filter_map(|q| q.0.strip_prefix("phase-").map(|v| v.parse().unwrap())).collect::<std::collections::BTreeSet<usize>>().into_iter().collect();
                let own = brackets[present.iter().position(|v| *v == i).unwrap()];
                if let Some(l) = lead {
                    assert!((l.1 .1 - own.2).abs() <= 3.0 && l.1 .0 >= own.3 - 0.5 && l.1 .0 <= own.4 + 0.5, "{tag}: the leader stands on its own bracket in {c:?}");
                } else {
                    let mid = (a.0 + bb.0) / 2.0;
                    assert!(mid >= own.3 && mid <= own.4, "{tag}: over its own bracket in {c:?}");
                    let dist = |q: &(f64, f64, f64, f64, f64)| seg_dist(*a, *bb, (q.3, q.2), (q.4, q.2));
                    for o in &brackets { if o.0 != own.0 { assert!(dist(&own) < dist(o), "{tag}: nearer another bracket in {c:?}"); } }
                }
            }
        }
    }
    assert!(n_infl >= 500 && n_phase >= 1500, "{n_infl} point lines, {n_phase} phase lines ({n_lead} with leaders)");
    eprintln!("{} cases: {n_infl} point lines ({n_visible} with the point on the graph, {n_far} more than 60 px away (each with a leader), {n_point_leader} with a leader, {unowned} count or K-line checks met only by the leader, {n_cross} leaders across a curve), {n_phase} phase lines, {n_lead} leaders in all", n_cases);
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

/// Predator-prey's Show is offered exactly where it changes the drawing: over time (each choice its own drawing), not
/// on the phase plane (every choice draws the same loop).
#[test]
fn predator_prey_show_offered_exactly_where_it_draws() {
    let d = def(PP);
    for graph in ["Over time", "Predators against prey"] {
        for pair in ["Snowshoe hare and Canada lynx", "Your own pair"] {
            for data in ["Smooth curves", "Census counts"] {
                let p = params(PP, &[("graph", s(graph)), ("pair", s(pair)), ("data", s(data))]);
                let shown = crate::figures::param_relevant(PP, "show", &p);
                let drawings: std::collections::BTreeSet<String> = ["Both", "Prey only", "Predators only", super::NEITHER].iter().map(|v| { let mut q = p.clone(); q.set_s("show", v); d.svg(&q) }).collect();
                assert_eq!(shown, drawings.len() > 1, "{graph} {pair} {data}: shown == changes the drawing");
                assert_eq!(shown, graph == "Over time");
            }
        }
    }
}

