use super::*;
use crate::figures::registry;

fn start() -> (Vec<ParamDef>, Params) {
    let d = registry().into_iter().find(|d| d.id == ID).unwrap();
    let defs = (d.params)();
    let p = Params::defaults(&defs);
    (defs, p)
}

fn with(kv: &[(&str, &str)]) -> Params {
    let (defs, mut p) = start();
    for (k, v) in kv {
        let val = match defs.iter().find(|d| d.key == *k).map(|d| &d.default) {
            Some(Value::I(_)) => Value::I(v.parse().unwrap()),
            Some(Value::F(_)) => Value::F(v.parse().unwrap()),
            Some(Value::B(_)) => Value::B(*v == "true"),
            Some(Value::S(_)) => Value::S(v.to_string()),
            None => panic!("no param {k}"),
        };
        p.0.insert(k.to_string(), val);
    }
    p.normalize(&defs, false);
    p
}

/// The key copy's lines under the figure, as drawn.
fn key_text(p: &Params) -> Vec<String> { render_full(p).texts.iter().filter(|t| t.what == "key").map(|t| t.text()).collect() }

fn lin(s: &str) -> (String, String, Option<char>) {
    let v = read_lin(s).unwrap();
    (rat_str(v.a), rat_str(v.b), v.letter)
}

#[test]
fn the_opening_check_solves_for_x() {
    let (_, p) = start();
    let m = model(&p);
    assert_eq!(m.eqs, vec!["AB + BC = AC"]);
    assert_eq!(solution(&m).join(" · "), "x = 6 · AB = 13 · BC = 14 · AC = 27");
    assert!(m.problems.is_empty(), "{:?}", m.problems);
}

/// cap-07a, cap-07c: the key copy writes the brace's equation and the solution under the figure;
/// the student copy leaves them out.
#[test]
fn the_key_copy_writes_the_check_under_the_figure() {
    assert_eq!(ID, "segment-addition");
    let (_, mut p) = start();
    assert_eq!(key_text(&p), vec!["AB + BC = AC", "x = 6 \u{b7} AB = 13 \u{b7} BC = 14 \u{b7} AC = 27"]);
    let r = render_full(&p);
    let key: Vec<_> = r.texts.iter().filter(|t| t.what == "key").collect();
    let lowest_fig = r.texts.iter().filter(|t| t.what != "key").map(|t| t.rect().3).fold(0.0, f64::max);
    assert!(key.iter().all(|t| t.rect().1 > lowest_fig && t.ink == "#1d4ed8"), "under the figure, in the key's ink");
    assert!(r.svg.contains("x = 6"), "described for a screen reader too");
    p.set_b(SHOW_ANSWERS, false);
    assert!(key_text(&p).is_empty());
    assert!(!render(&p).contains("x = 6"));
}

/// Writes every example's student and key copy to $SEG_DUMP (a folder), for looking at.
#[test]
fn dump_examples_when_asked() {
    let Ok(dir) = std::env::var("SEG_DUMP") else { return };
    let (defs, base) = start();
    for (i, ex) in examples(ID).unwrap().iter().enumerate() {
        let mut p = base.clone();
        for (k, v) in &ex.params {
            p.0.insert(k.to_string(), v.clone());
        }
        p.normalize(&defs, false);
        for key in [true, false] {
            p.set_b(SHOW_ANSWERS, key);
            std::fs::write(format!("{dir}/ex{i}_{}.svg", if key { "key" } else { "student" }), render_full(&p).svg).unwrap();
        }
    }
    if let Ok(extra) = std::env::var("SEG_EXTRA") {
        // k=v;k=v|k=v... : one figure per |
        for (i, spec) in extra.split('|').enumerate() {
            let mut p = base.clone();
            for kv in spec.split(';').filter(|s| !s.is_empty()) {
                let (k, v) = kv.split_once('=').unwrap();
                let val = match defs.iter().find(|d| d.key == k).map(|d| &d.default) {
                    Some(Value::I(_)) => Value::I(v.parse().unwrap()),
                    Some(Value::F(_)) => Value::F(v.parse().unwrap()),
                    Some(Value::B(_)) => Value::B(v == "true"),
                    _ => Value::S(v.to_string()),
                };
                p.0.insert(k.to_string(), val);
            }
            p.normalize(&defs, false);
            std::fs::write(format!("{dir}/x{i}.svg"), render_full(&p).svg).unwrap();
        }
    }
}

#[test]
fn labels_read_as_a_number_or_an_expression_in_one_letter() {
    assert_eq!(lin("12"), ("0".into(), "12".into(), None));
    assert_eq!(lin("2x + 3"), ("2".into(), "3".into(), Some('x')));
    assert_eq!(lin("3x \u{2212} 4"), ("3".into(), "\u{2212}4".into(), Some('x')));
    assert_eq!(lin("3(x - 2)"), ("3".into(), "\u{2212}6".into(), Some('x')));
    assert_eq!(lin("x/2"), ("0.5".into(), "0".into(), Some('x')));
    assert_eq!(lin("-y + 10"), ("\u{2212}1".into(), "10".into(), Some('y')));
    assert_eq!(lin("(x+1)/3"), ("1/3".into(), "1/3".into(), Some('x')));
    assert_eq!(lin("2.5n"), ("2.5".into(), "0".into(), Some('n')));
    assert_eq!(lin("x - x + 4"), ("0".into(), "4".into(), None), "a letter that cancels in its own label leaves a number");
    assert_eq!(read_lin("x + y"), Err(Why::Letters(vec!['x', 'y'])));
    assert_eq!(lin("2 1/2"), ("0".into(), "2.5".into(), None), "a mixed number");
    assert_eq!(lin("-1 3/4 + x"), ("1".into(), "\u{2212}1.75".into(), Some('x')));
    assert_eq!(read_lin("half"), Err(Why::Unreadable), "a word, not four letters");
    for bad in ["x^2", "x*x", "?", "5 6", "2/(x+1)", "", "3/0", "1..2", "2 3/2", "xy", "1.5 1/2"] {
        assert_eq!(read_lin(bad), Err(Why::Unreadable), "{bad}");
    }
    assert_eq!(split_unit("12 cm"), ("12".into(), Some("cm".into())));
    assert_eq!(split_unit("12cm"), ("12".into(), Some("cm".into())));
    assert_eq!(split_unit("2x + 3 in"), ("2x + 3".into(), Some("in".into())));
    assert_eq!(split_unit("12m"), ("12m".into(), None), "12m reads as 12 times m");
}

#[test]
fn labels_are_written_as_a_book_writes_them() {
    assert_eq!(pretty("2x+1"), "2x + 1");
    assert_eq!(pretty("3x-4"), "3x \u{2212} 4");
    assert_eq!(pretty("-x+5"), "\u{2212}x + 5");
    assert_eq!(pretty("3(x-2)"), "3(x \u{2212} 2)");
    assert_eq!(pretty("2*x"), "2\u{b7}x");
    assert_eq!(pretty("12cm"), "12 cm");
    assert_eq!(pretty("2 1/2 in"), "2 1/2 in", "a mixed number keeps its space");
    assert_eq!(pretty("x+1 1/2"), "x + 1 1/2");
    assert_eq!(runs_of("2x + 1"), vec![("2".into(), false), ("x".into(), true), (" + 1".into(), false)]);
    assert_eq!(runs_of("12 cm"), vec![("12 cm".into(), false)]);
    assert_eq!(words_of("Very long x"), vec![("Very long ".into(), false), ("x".into(), true)]);
    let r = render_full(&with(&[("p1_lab", "Not to scale")]));
    assert!(r.texts.iter().any(|t| t.what == "part 0" && t.runs == vec![("Not to scale".to_string(), false)]));
}

#[test]
fn numbers_are_exact() {
    let r = |n, d| Rat::new(n, d).unwrap();
    assert_eq!(rat_str(r(7, 2)), "3.5");
    assert_eq!(rat_str(r(1, 8)), "0.125");
    assert_eq!(rat_str(r(7, 3)), "7/3");
    assert_eq!(rat_str(r(-13, 4)), "\u{2212}3.25");
    assert_eq!(rat_str(r(1, 64)), "1/64", "more than four places stays a fraction");
}

#[test]
fn a_midpoint_with_expressions_solves_from_the_ticks() {
    let p = with(&[("p2_name", "M"), ("p3_name", "B"), ("p1_lab", "3x + 2"), ("p2_lab", "5x - 6"), ("p1_ticks", "1 tick"), ("p2_ticks", "1 tick"), ("spans", "0")]);
    let m = model(&p);
    assert_eq!(m.eqs, vec!["AM = MB"]);
    assert_eq!(solution(&m).join(" · "), "x = 4 · AM = 14 · MB = 14");
    assert!(m.problems.is_empty());
    // cap-07b: on the key copy
    assert_eq!(key_text(&p), vec!["AM = MB", "x = 4 \u{b7} AM = 14 \u{b7} MB = 14"]);
    // two pairs with their own tick counts give two equations
    let p = with(&[("n", "5"), ("p1_lab", "x"), ("p2_lab", "4"), ("p3_lab", "2x - 4"), ("p4_lab", "y"), ("p1_ticks", "1 tick"), ("p2_ticks", "1 tick"), ("p3_ticks", "2 ticks"), ("p4_ticks", "2 ticks"), ("spans", "0")]);
    let m = model(&p);
    assert_eq!(m.eqs, vec!["AB = BC", "CD = DE"]);
}

#[test]
fn a_blank_part_is_worked_out_and_written_on_the_key_only() {
    let p = with(&[("n", "4"), ("p1_lab", "5"), ("p2_lab_mode", "Blank line"), ("p3_lab", "4"), ("spans", "2"), ("s1_from", "1"), ("s1_to", "3"), ("s1_lab", "12"), ("s2_from", "1"), ("s2_to", "4"), ("s2_lab_mode", "None")]);
    let m = model(&p);
    assert_eq!(m.eqs, vec!["AB + BC = AC", "AB + BC + CD = AD"]);
    assert_eq!(solution(&m).join(" · "), "AB = 5 · BC = 7 · CD = 4 · AC = 12 · AD = 16", "BC from AC, and the unlabeled AD summed");
    let key = render_full(&p);
    let blank: Vec<_> = key.texts.iter().filter(|t| t.what == "part 1").collect();
    assert_eq!(blank.len(), 1);
    assert_eq!((blank[0].text().as_str(), blank[0].bold, blank[0].ink), ("7", true, "#1d4ed8"));
    let mut q = p.clone();
    q.set_b(SHOW_ANSWERS, false);
    let student = render_full(&q);
    assert!(student.texts.iter().all(|t| t.what != "part 1" && t.what != "key"), "the student copy leaves the blank line empty and the check out");
    assert!(student.svg.contains("<line"), "the blank line is drawn");
    assert_eq!(key.svg.matches("<line").count(), student.svg.matches("<line").count(), "the same blank line on both copies");
    // the same with Print: the key's answers in black
    let mut pr = p.clone();
    pr.set_s("style", "Print");
    assert!(render_full(&pr).texts.iter().filter(|t| t.what == "part 1" || t.what == "key").all(|t| t.ink == "#111827"));
}

#[test]
fn labels_that_dont_agree_say_so() {
    // all numbers: the brace doesn't add up
    let p = with(&[("p1_lab", "5"), ("p2_lab", "6"), ("s1_lab", "12")]);
    let m = model(&p);
    assert_eq!(m.problems, vec!["AB + BC = 11 but AC = 12: the labels don\u{2019}t agree."]);
    assert_eq!(solution(&m).join(" · "), "AB = 5 · BC = 6", "the brace has no one length, so it isn't listed");
    assert!(key_text(&p).join(" ").contains("AB + BC = 11 but AC = 12"), "on the key copy: {:?}", key_text(&p));
    // x from the brace breaks the ticks
    let p = with(&[("p1_ticks", "1 tick"), ("p2_ticks", "1 tick")]);
    let m = model(&p);
    assert_eq!(m.letter_val, Some(Rat::int(6)));
    assert_eq!(m.problems, vec!["With x = 6, AB = 13 but BC = 14, though they have the same ticks: the labels don\u{2019}t agree."]);
    assert!(key_text(&p).join(" ").contains("though they have the same ticks"));
    // a brace over one part
    let m = model(&with(&[("p1_lab", "5"), ("p2_lab", "6"), ("s1_to", "2"), ("s1_lab", "7")]));
    assert_eq!(m.problems, vec!["AB is 5 but the brace under it is labeled 7: the labels don\u{2019}t agree."]);
}

#[test]
fn labels_that_cant_be_solved_say_why() {
    // a length 0 or less
    let p = with(&[("p1_lab", "x - 10"), ("p2_lab", "x"), ("s1_lab", "10")]);
    let m = model(&p);
    assert_eq!(m.problems, vec!["With x = 10, AB comes out 0, but a length must be more than 0."]);
    assert!(key_text(&p).join(" ").contains("a length must be more than 0"), "on the key copy");
    // a brace of 0 over parts already said to be 0 isn't said again
    let m = model(&with(&[("p1_lab", "x"), ("p2_lab", "x"), ("s1_lab", "0")]));
    assert_eq!(m.problems.len(), 2, "{:?}", m.problems);
    // two letters
    let m = model(&with(&[("p2_lab", "3y")]));
    assert_eq!(m.problems, vec!["The labels use x and y: only one letter can be solved for, so the labels aren\u{2019}t solved."]);
    assert!(solution(&m).is_empty());
    // nothing to solve from
    let p = with(&[("spans", "0")]);
    let m = model(&p);
    assert_eq!(m.problems, vec!["There is no equation to find x from: label a brace over parts with x, or give parts the same ticks."]);
    assert!(key_text(&p).join(" ").contains("There is no equation to find"), "on the key copy");
    // the letter cancels
    let m = model(&with(&[("p1_lab", "x + 1"), ("p2_lab", "x + 1"), ("p1_ticks", "2 ticks"), ("p2_ticks", "2 ticks"), ("spans", "0")]));
    assert_eq!(m.problems, vec!["x cancels out of AB = BC, so it can\u{2019}t be solved from it."], "said once");
    // cancels and the numbers left disagree
    let m = model(&with(&[("p1_lab", "x + 3"), ("p2_lab", "10 - x"), ("s1_lab", "14")]));
    assert_eq!(m.problems, vec!["x cancels out of AB + BC = AC, and the numbers left don\u{2019}t agree, so these labels can\u{2019}t all be right."]);
    assert!(solution(&m).is_empty(), "nothing has one length: {:?}", solution(&m));
    // blank lines the key can't fill say so
    let p = with(&[("n", "4"), ("p1_lab", "5"), ("p2_lab_mode", "Blank line"), ("p3_lab_mode", "Blank line"), ("s1_from", "1"), ("s1_to", "4"), ("s1_lab", "20")]);
    assert_eq!(model(&p).problems, vec!["BC and CD can\u{2019}t be worked out from these labels, so their blank lines stay empty on the key."]);
    // two points with one name: on the key, since the check reads oddly
    let p = with(&[("p2_name", "A")]);
    assert!(key_text(&p).join(" ").contains("Two points are named A"));
    // ... but not when x is found from another equation
    let p = with(&[("p2_name", "M"), ("p3_name", "B"), ("p1_lab", "x"), ("p2_lab", "x"), ("p1_ticks", "1 tick"), ("p2_ticks", "1 tick"), ("s1_lab", "20")]);
    let m = model(&p);
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    assert_eq!(key_text(&p), vec!["AM + MB = AB,   AM = MB", "x = 10 \u{b7} AM = 10 \u{b7} MB = 10 \u{b7} AB = 20"]);
    // a brace added twice writes its equation once
    assert_eq!(model(&with(&[("spans", "2"), ("s2_from", "1"), ("s2_to", "3")])).eqs, vec!["AB + BC = AC"]);
    // a label that isn't a number or an expression is left out, and the rest still solve
    let m = model(&with(&[("n", "4"), ("p3_lab", "?"), ("s1_lab", "27")]));
    assert_eq!(m.problems, vec!["\u{201c}?\u{201d} on CD isn\u{2019}t a number or an expression like 2x + 3, so it\u{2019}s left out."]);
    assert_eq!(m.letter_val, Some(Rat::int(6)));
    // the problems are written on the key copy too
    let r = render_full(&with(&[("p1_lab", "5"), ("p2_lab", "6"), ("s1_lab", "12")]));
    let key: String = r.texts.iter().filter(|t| t.what == "key").map(|t| t.text()).collect::<Vec<_>>().join(" ");
    assert!(key.contains("the labels don\u{2019}t agree"), "{key}");
}

#[test]
fn units_are_kept_and_fractions_stay_exact() {
    // labels typed as mixed numbers get mixed-number answers, drawn with their space
    let p = with(&[("p1_lab", "2 1/2 in"), ("p2_lab", "3 1/4 in"), ("s1_lab_mode", "Blank line")]);
    let m = model(&p);
    assert_eq!(solution(&m).join(" · "), "AB = 2 1/2 in · BC = 3 1/4 in · AC = 5 3/4 in");
    let r = render_full(&p);
    assert!(r.texts.iter().any(|t| t.what == "part 0" && t.text() == "2 1/2 in"));
    assert!(r.texts.iter().any(|t| t.what == "span 0" && t.text() == "5 3/4 in"));
    assert!(r.svg.contains("labeled 2 1/2 in"));
    let m = model(&with(&[("p1_lab", "5 cm"), ("p2_lab", "7 in"), ("s1_lab", "12")]));
    assert_eq!(m.problems, vec!["The labels use cm and in: the check adds the numbers as they are, so give every label the same unit."]);
    assert_eq!(solution(&m).join(" · "), "AB = 5 · BC = 7 · AC = 12", "no unit is written when they differ");
    let m = model(&with(&[("p1_lab", "x/3 cm"), ("p2_lab", "x"), ("s1_lab", "7 cm")]));
    assert_eq!(solution(&m).join(" · "), "x = 5.25 · AB = 1.75 cm · BC = 5.25 cm · AC = 7 cm");
    let m = model(&with(&[("p1_lab", "x"), ("p2_lab", "2x"), ("s1_lab", "7")]));
    assert_eq!(solution(&m).join(" · "), "x = 7/3 · AB = 7/3 · BC = 14/3 · AC = 7");
    // a message about labels that don't agree keeps their unit
    let m = model(&with(&[("p1_lab", "5 cm"), ("p2_lab", "7 cm"), ("s1_lab", "15 cm")]));
    assert_eq!(m.problems, vec!["AB + BC = 12 cm but AC = 15 cm: the labels don’t agree."]);
    let m = model(&with(&[("p1_lab", "5 cm"), ("p2_lab", "7 cm"), ("s1_lab", "15")]));
    assert_eq!(m.problems, vec!["AB + BC = 12 cm but AC = 15: the labels don’t agree."], "a label typed without a unit is quoted without one");
    let m = model(&with(&[("p1_lab", "x - 10 cm"), ("p2_lab", "2x cm"), ("s1_lab", "20 cm")]));
    assert!(m.problems.iter().any(|s| s == "With x = 10, AB comes out 0 cm, but a length must be more than 0."), "{:?}", m.problems);
    // two braces with the same wrong label say so once
    let m = model(&with(&[("p1_lab", "5"), ("p2_lab", "7"), ("spans", "2"), ("s1_lab", "15"), ("s2_from", "1"), ("s2_to", "3"), ("s2_lab", "15")]));
    assert_eq!(m.problems.len(), 1, "{:?}", m.problems);
}

#[test]
fn parts_are_drawn_to_scale_unless_typed() {
    let r = draw(&with(&[]), &model(&with(&[])));
    let (a, b, c) = (r.points[0].0, r.points[1].0, r.points[2].0);
    assert!(((b - a) / (c - b) - 13.0 / 14.0).abs() < 1e-9, "AB : BC is 13 : 14");
    // none shorter than a fifth of the longest
    let p = with(&[("p1_lab", "1"), ("p2_lab", "100"), ("s1_lab_mode", "None")]);
    let r = draw(&p, &model(&p));
    let (a, b, c) = (r.points[0].0, r.points[1].0, r.points[2].0);
    assert!(((b - a) / (c - b) - 0.2).abs() < 1e-9);
    // typed sizes, for a figure not drawn to scale
    let p = with(&[("p1_len", "3"), ("p2_len", "1")]);
    let r = draw(&p, &model(&p));
    let (a, b, c) = (r.points[0].0, r.points[1].0, r.points[2].0);
    assert!(((b - a) / (c - b) - 3.0).abs() < 1e-9);
    // a part with no length takes the average of the others
    let p = with(&[("n", "4"), ("p1_lab", "4"), ("p2_lab_mode", "None"), ("p3_lab", "8"), ("spans", "0")]);
    let r = draw(&p, &model(&p));
    let d: Vec<f64> = r.points.windows(2).map(|w| w[1].0 - w[0].0).collect();
    assert!((d[1] / d[0] - 1.5).abs() < 1e-9 && (d[2] / d[0] - 2.0).abs() < 1e-9, "{d:?}");
    // a typed size that isn't a number says so
    assert!(warnings(&with(&[("p1_len", "abc")])).iter().any(|w| w.contains("\u{201c}abc\u{201d}")));
}

#[test]
fn ends_make_a_segment_a_ray_or_a_line() {
    let heads = |kv: &[(&str, &str)]| {
        let p = with(kv);
        draw(&p, &model(&p)).prims.iter().filter(|q| matches!(q, Prim::Poly(_, _))).count()
    };
    assert_eq!(heads(&[]), 0);
    assert_eq!(heads(&[("right_end", "Arrow")]), 1);
    assert_eq!(heads(&[("left_end", "Arrow"), ("right_end", "Line arrow")]), 2);
    let filled = |kv: &[(&str, &str)]| {
        let p = with(kv);
        draw(&p, &model(&p)).prims.iter().filter(|q| matches!(q, Prim::Poly(_, true))).count()
    };
    assert_eq!(filled(&[("left_end", "Line arrow")]), 0, "a line arrow is open");
    assert_eq!(filled(&[("left_end", "Arrow")]), 1);
    assert!(render(&with(&[("right_end", "Arrow")])).contains("Ray AC"));
    assert!(render(&with(&[("left_end", "Arrow"), ("right_end", "Arrow")])).contains("Line AC"));
    assert!(render(&with(&[])).contains("Segment AC"));
}

#[test]
fn a_crossing_line_at_a_point_with_a_right_angle_mark_at_90() {
    let marks = |kv: &[(&str, &str)]| {
        let p = with(kv);
        draw(&p, &model(&p)).prims.iter().filter(|q| matches!(q, Prim::Poly(v, false) if v.len() == 3)).count()
    };
    assert_eq!(marks(&[("cross_at", "2")]), 1);
    assert_eq!(marks(&[("cross_at", "2"), ("cross_right", "false")]), 0);
    assert_eq!(marks(&[("cross_at", "2"), ("cross_ang", "60")]), 0);
    let p = with(&[("cross_at", "2"), ("cross_name", "\u{2113}")]);
    let r = render_full(&p);
    assert!(r.texts.iter().any(|t| t.what == "cross" && t.text() == "\u{2113}"));
    let el = r.svg.split("<text").find(|e| e.contains("\u{2113}</tspan>")).unwrap();
    assert!(el.contains("font-family=\"Source Serif 4, DejaVu Serif, serif\""), "\u{2113} in a face that has it: {el}");
    assert!(r.svg.contains("crosses at B at 90"));
    assert_ne!(render(&with(&[("cross_at", "2"), ("cross_ang", "45")])), render(&with(&[("cross_at", "2")])));
    // a bisector over a brace keeps most of its lower half: no less than 70% of the upper
    let p = with(&[("p2_name", "M"), ("p3_name", "B"), ("p1_ticks", "1 tick"), ("p2_ticks", "1 tick"), ("cross_at", "2"), ("s1_lab_mode", "Blank line")]);
    let d = draw(&p, &model(&p));
    let (a, b) = d.prims.iter().find_map(|q| if let Prim::Line(a, b, w) = q { (*w == 1.8).then_some((*a, *b)) } else { None }).unwrap();
    let y0 = d.points[1].1;
    assert!((a.1 - y0) >= 0.7 * (y0 - b.1) - 1e-9, "lower {} upper {}", a.1 - y0, y0 - b.1);
}

#[test]
fn removing_a_point_moves_the_later_ones_up() {
    let (defs, _) = start();
    let mut p = with(&[("n", "4"), ("p4_name", "D"), ("p3_lab", "9"), ("p2_ticks", "1 tick"), ("p2_lab_mode", "Blank line"), ("p1_ticks", "2 ticks"), ("spans", "2"), ("s2_from", "2"), ("s2_to", "4"), ("cross_at", "4")]);
    assert!(remove_point(&mut p, &defs, 1));
    assert_eq!(n_points(&p), 3);
    assert_eq!((p.s("p1_name"), p.s("p2_name"), p.s("p3_name")), ("A", "C", "D"));
    assert_eq!((p.s("p1_lab"), p.s("p1_ticks"), p.s("p1_lab_mode"), p.s("p2_lab"), p.s("p2_ticks")), ("", "1 tick", "Blank line", "9", "None"), "the joined part AC has no label text but keeps BC's label style and ticks, as the reference does; CD keeps its label");
    assert_eq!((n_spans(&p), p.i("s1_from"), p.i("s1_to")), (1, 1, 2), "AC, over B, still spans A to C; BD, which started at B, is gone");
    assert_eq!(p.i("cross_at"), 3, "the crossing line stays at D");
    // the first point: its part goes with it, the next part keeps its label
    let mut p = with(&[]);
    assert!(remove_point(&mut p, &defs, 0));
    assert_eq!((p.s("p1_name"), p.s("p2_name"), p.s("p1_lab")), ("B", "C", "3x - 4"));
    assert_eq!(n_spans(&p), 0, "the brace from A goes");
    // the last point: so does a crossing line through it
    let mut p = with(&[("spans", "2"), ("s2_from", "1"), ("s2_to", "2"), ("cross_at", "3")]);
    assert!(remove_point(&mut p, &defs, 2));
    assert_eq!((n_spans(&p), p.i("s1_from"), p.i("s1_to"), p.s("p1_lab")), (1, 1, 2, "2x + 1"));
    assert_eq!(p.i("cross_at"), 0);
    // never fewer than two
    assert!(!remove_point(&mut p, &defs, 0));
}

#[test]
fn adding_a_point_or_a_brace_starts_clean() {
    let (defs, _) = start();
    let mut p = with(&[("p2_name", "M")]);
    assert!(add_point(&mut p, &defs));
    assert_eq!((n_points(&p), p.s("p4_name"), p.s("p3_lab")), (4, "B", ""), "the first free letter; an empty part");
    let mut p = with(&[]);
    assert!(add_span(&mut p, &defs));
    assert_eq!((p.i("s2_from"), p.i("s2_to"), p.s("s2_lab")), (1, 2, ""));
    assert!(add_span(&mut p, &defs));
    assert_eq!((p.i("s3_from"), p.i("s3_to")), (2, 3));
    assert!(!add_span(&mut p, &defs), "three at most");
    assert!(remove_span(&mut p, &defs, 0));
    assert_eq!((n_spans(&p), p.i("s1_from"), p.i("s2_from")), (2, 1, 2));
}

#[test]
fn turning_keeps_the_labels_upright_and_the_figure_whole() {
    let a = render_full(&with(&[]));
    let b = render_full(&with(&[("turn", "45")]));
    assert!(!b.svg.contains("rotate("), "labels stay upright");
    assert_ne!(a.svg, b.svg);
    // positive turns counterclockwise: C is above A
    let p = with(&[("turn", "45")]);
    let d = draw(&p, &model(&p));
    assert!(d.points[2].1 < d.points[0].1);
    for t in &b.texts {
        let r = t.rect();
        assert!(r.0 >= 0.0 && r.1 >= 0.0 && r.2 <= b.size.0 && r.3 <= b.size.1, "{} inside the page", t.what);
    }
}

#[test]
fn label_size_scales_the_labels() {
    let size = |v: &str| render_full(&with(&[("label_size", v)])).texts.iter().find(|t| t.what == "name 0").unwrap().size;
    assert!(size("Large") > size("Medium") && size("Medium") > size("Small"));
}

/// No two labels overlap, and no line runs through a label, across the shapes a teacher makes.
#[test]
fn labels_never_overlap_each_other_or_the_crossing_line() {
    let mut cases: Vec<Vec<(&str, String)>> = Vec::new();
    for turn in ["0", "30", "-45", "90", "-90", "15"] {
        for size in ["Small", "Medium", "Large"] {
            for ang in ["20", "45", "90", "135", "160"] {
                for cross in ["0", "1", "2", "3"] {
                    cases.push(vec![("turn", turn.into()), ("label_size", size.into()), ("cross_ang", ang.into()), ("cross_at", cross.into()), ("cross_name", "\u{2113}".into()), ("p1_ticks", "2 ticks".into())]);
                }
            }
        }
    }
    // a slanted crossing line over a short part
    for (cross, ang, size) in [("4", "45", "Medium"), ("2", "135", "Large"), ("3", "150", "Large"), ("4", "20", "Large"), ("2", "160", "Medium")] {
        cases.push(vec![("n", "5".into()), ("p1_lab", "x + 3".into()), ("p2_lab", "4x".into()), ("p3_lab", "20".into()), ("p4_lab", "2x - 1".into()), ("s1_from", "1".into()), ("s1_to", "5".into()), ("s1_lab", "50".into()), ("cross_at", cross.into()), ("cross_ang", ang.into()), ("cross_name", "t".into()), ("label_size", size.into())]);
    }
    cases.push(vec![("p1_lab", "20 cm".into()), ("p2_lab", "x + 1 cm".into()), ("s1_lab", "25 cm".into()), ("cross_at", "3".into()), ("cross_ang", "150".into()), ("cross_name", "k".into()), ("label_size", "Large".into())]);
    // a shallow crossing line's lower half beside a short part stays clear of the far point's name
    for labs in [["4", "12", "3", "9"], ["x/2", "5x", "x", "3x"]] {
        for cross in 1..=5 {
            for ang in ["20", "25", "30", "150", "155", "160"] {
                for size in ["Medium", "Large"] {
                    let mut c: Vec<(&str, String)> = vec![("n", "5".into()), ("s1_from", "1".into()), ("s1_to", "5".into()), ("s1_lab", "40".into())];
                    c.extend(LAB_KEYS.iter().zip(labs).map(|(k, v)| (*k, v.to_string())));
                    c.extend([("cross_at", cross.to_string()), ("cross_ang", ang.into()), ("cross_name", "\u{2113}".into()), ("label_size", size.into())]);
                    cases.push(c);
                }
            }
        }
    }
    // short parts beside a crossing point at 90°: the right-angle mark stays clear of their labels
    cases.push(vec![("n", "7".into()), ("p1_lab", "3x + 2".into()), ("p2_lab", "5x - 6".into()), ("p3_lab", "2x + 10".into()), ("p4_lab", "x - 1".into()), ("p5_lab", "4x + 3".into()), ("p6_lab", "12.5 cm".into()), ("s1_to", "7".into()), ("s1_lab", "100".into()), ("cross_at", "4".into()), ("cross_name", "\u{2113}".into())]);
    cases.push(vec![("n", "4".into()), ("p1_lab", "2x + 1".into()), ("p2_lab", "x - 1".into()), ("p3_lab", "3x".into()), ("p1_len", "1".into()), ("p2_len", "5".into()), ("p3_len", "5".into()), ("s1_lab_mode", "None".into()), ("cross_at", "1".into())]);
    for turn in ["90", "-90", "60"] {
        let mut c: Vec<(&str, String)> = vec![("n", "7".into()), ("turn", turn.into()), ("label_size", "Large".into())];
        for (i, k) in NAME_KEYS.iter().enumerate() {
            c.push((k, ["Mid1", "XYZW", "PQRS", "AB12", "WWWW", "MMMM", "Q7"][i].into()));
        }
        for k in LAB_KEYS.iter().skip(2) {
            c.push((k, "1".into()));
        }
        cases.push(c);
    }
    for n in 2..=7 {
        let mut c = vec![("n", n.to_string()), ("spans", "3".into()), ("s1_from", "1".into()), ("s1_to", n.to_string()), ("s2_from", "1".into()), ("s2_to", "2".into()), ("s3_from", (n - 1).to_string()), ("s3_to", n.to_string())];
        c.push(("s2_lab", "x + 10".into()));
        c.push(("s3_lab_mode", "Blank line".into()));
        cases.push(c);
    }
    for c in &cases {
        let kv: Vec<(&str, &str)> = c.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let p = with(&kv);
        let r = render_full(&p);
        let ts = &r.texts;
        for i in 0..ts.len() {
            for j in i + 1..ts.len() {
                let (a, b) = (ts[i].rect(), ts[j].rect());
                let overlap = a.0 < b.2 - 0.5 && b.0 < a.2 - 0.5 && a.1 < b.3 - 0.5 && b.1 < a.3 - 0.5;
                assert!(!overlap, "{} and {} overlap in {kv:?}", ts[i].what, ts[j].what);
            }
        }
        let m = model(&p);
        let d = draw(&p, &m);
        // each part label stays by its own part: over it (or under it), not up by the crossing line's name
        for q in &d.prims {
            if let Prim::Text(t) = q {
                if let Some(k) = t.what.strip_prefix("part ").and_then(|k| k.parse::<usize>().ok()) {
                    let (a, b) = (d.points[k], d.points[k + 1]);
                    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                    let len2 = dx * dx + dy * dy;
                    let tt = (((t.c.0 - a.0) * dx + (t.c.1 - a.1) * dy) / len2).clamp(0.0, 1.0);
                    let dist = ((t.c.0 - a.0 - tt * dx).powi(2) + (t.c.1 - a.1 - tt * dy).powi(2)).sqrt();
                    assert!(dist <= t.h * 0.5 + t.w * 0.5 + 16.0, "{} is {dist:.0} px from its part in {kv:?}", t.what);
                }
            }
        }
        let mut lines: Vec<(Pt, Pt)> = Vec::new();
        for q in &d.prims {
            match q {
                Prim::Line(a, b, _) => lines.push((*a, *b)),
                Prim::Brace(v) | Prim::Poly(v, _) => lines.extend(v.windows(2).map(|w| (w[0], w[1]))),
                _ => {}
            }
        }
        for q in &d.prims {
            if let Prim::Text(t) = q {
                for (a, b) in &lines {
                    assert!(!seg_hits(*a, *b, t.rect(), -1.0), "a line runs through {} ({}) in {kv:?}", t.what, t.text());
                }
            }
        }
    }
}


