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
