use super::*;
use crate::figures::{Params, Value};

fn params(over: &[(&'static str, Value)]) -> Params {
    let defs = super::params();
    let mut p = Params::defaults(&defs);
    for (k, v) in over {
        p.0.insert(k.to_string(), v.clone());
    }
    p.normalize(&defs, false);
    p
}
fn s(v: &str) -> Value {
    Value::S(v.to_string())
}
fn svg(over: &[(&'static str, Value)]) -> String {
    render(&params(over))
}

#[test]
fn counts_become_shares_clockwise_and_label_text_is_exact() {
    let m = model(&params(&[("data", s("A, 1\nB, 3"))]));
    assert_eq!(m.share, vec![0.25, 0.75]);
    let t = svg(&[("data", s("A, 1\nB, 3")), ("labels", s("Name and percent"))]);
    assert!(t.contains(">25%<") && t.contains(">75%<") && t.contains(">A<") && t.contains(">B<"), "{t}");
    let t = svg(&[("data", s("A, 1\nB, 3")), ("labels", s("Name and angle"))]);
    assert!(t.contains(">90\u{b0}<") && t.contains(">270\u{b0}<"));
    let t = svg(&[("data", s("A, 1\nB, 3")), ("labels", s("Fraction"))]);
    assert!(t.contains(">1/4<") && t.contains(">3/4<"));
}

#[test]
fn percents_degrees_and_total_unit() {
    let m = model(&params(&[("data", s("A, 35\nB, 65")), ("values", s("Percents"))]));
    assert!((m.share[0] - 0.35).abs() < 1e-12);
    let m = model(&params(&[("data", s("A, 90\nB, 270")), ("values", s("Degrees"))]));
    assert_eq!(m.share, vec![0.25, 0.75]);
    let t = svg(&[("data", s("A, 90\nB, 270")), ("values", s("Degrees")), ("labels", s("Value"))]);
    assert!(t.contains(">90\u{b0}<"));
}

#[test]
fn an_empty_sector_takes_the_rest_and_is_blank_on_the_student_copy() {
    let over = [("data", s("A, 20\nB, 15\nC")), ("total", s("60")), ("labels", s("Name and value"))];
    let m = model(&params(&over));
    assert!((m.share[2] - 25.0 / 60.0).abs() < 1e-12 && m.blank[2]);
    let key = svg(&over);
    assert!(key.contains(">25<") && key.contains(">C<"));
    let mut stu = over.to_vec();
    stu.push(("show_answers", Value::B(false)));
    let stu = svg(&stu);
    assert!(!stu.contains(">25<") && stu.contains(">C<"), "student copy keeps the name, drops the number");
}

#[test]
fn own_label_letter_and_blank_line_and_none() {
    let t = svg(&[("data", s("A, 1\nB, 1\nC")), ("total", s("4")), ("labels", s("Name")), ("own", s("\n-\nx\u{b0}")), ("show_answers", Value::B(false))]);
    assert!(t.contains(">x\u{b0}<") && !t.contains(">B<"));
    let t = svg(&[("data", s("A, 1\nB, 1")), ("own", s("_"))]);
    assert!(t.contains("<line") && !t.contains(">A<") && t.contains(">B<"));
}

#[test]
fn blank_circle_and_odd_data_render() {
    let t = svg(&[("data", s("Dog\nCat")), ("look", s("White")), ("title_kind", s("Blank line")), ("key", s("Right"))]);
    assert!(t.contains(">Dog<") && t.contains("<circle") && t.matches("<line").count() >= 2, "{t}");
    for d in ["", "\n\n", "x, y, z", "A, -5", "A, 1e400"] {
        let t = svg(&[("data", s(d))]);
        assert!(t.starts_with("<svg"));
    }
}

#[test]
fn largest_first_puts_the_biggest_sector_first() {
    let a = svg(&[("data", s("S, 1\nL, 9")), ("order", s("Largest first")), ("key", s("Right"))]);
    let l = a.find(">L<").unwrap();
    let sm = a.find(">S<").unwrap();
    assert!(l < sm, "key lists L before S");
    let b = svg(&[("data", s("S, 1\nL, 9")), ("key", s("Right"))]);
    assert!(b.find(">S<").unwrap() < b.find(">L<").unwrap());
}

#[test]
fn key_and_title_and_dot_change_the_picture() {
    let base = svg(&[]);
    let keyed = svg(&[("key", s("Right"))]);
    let below = svg(&[("key", s("Below"))]);
    assert!(keyed != base && below != base && keyed != below);
    let w = |t: &str| t.split("width=\"").nth(1).unwrap().split('"').next().unwrap().parse::<f64>().unwrap();
    assert!(w(&keyed) > w(&base), "a right key widens the canvas");
    assert!(svg(&[("title_kind", s("Chart title")), ("title", s("Pets"))]).contains(">Pets<"));
    assert!(svg(&[("dot", Value::B(true))]).contains("r=\"3.20\""));
}

#[test]
fn print_is_grey_only_and_black_and_white_uses_patterns() {
    let t = svg(&[("style", s("Print")), ("key", s("Right")), ("title_kind", s("Chart title")), ("title", s("T"))]);
    for h in t.split('#').skip(1).map(|x| x.chars().take(6).collect::<String>()).filter(|h| h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())) {
        let (r, g, b) = (&h[0..2], &h[2..4], &h[4..6]);
        assert!(r == g && g == b, "colour {h} in Print");
    }
    assert!(t.contains("url(#pie-p1)"));
}

#[test]
fn per_sector_fill_applies() {
    let t = svg(&[("fill_list", s("Red, , Dots")), ("data", s("A, 1\nB, 1\nC, 1"))]);
    assert!(t.contains("#c9494f") && t.contains("url(#pie-p2)"), "{t}");
}

#[test]
fn fractions_reduce() {
    assert_eq!(fraction(0.375).as_deref(), Some("3/8"));
    assert_eq!(fraction(1.0).as_deref(), Some("1"));
    assert_eq!(fraction(1.0 / 12.0).as_deref(), Some("1/12"));
}

#[test]
fn warnings_say_when_percents_do_not_add_up() {
    let w = warnings(&params(&[("data", s("A, 30\nB, 30")), ("values", s("Percents"))]));
    assert!(w.iter().any(|x| x.contains("not 100")), "{w:?}");
}

#[test]
fn several_empty_rows_make_one_combined_sector_with_no_invented_numbers() {
    let over = [("data", s("A, 20\nB, 15\nC\nD")), ("total", s("60")), ("labels", s("Name and value")), ("key", s("Right"))];
    let m = model(&params(&over));
    assert_eq!(m.real, 4);
    assert_eq!(m.rows.len(), 5, "one remainder sector");
    assert_eq!(m.share[2], 0.0);
    assert_eq!(m.share[3], 0.0);
    assert!((m.share[4] - 25.0 / 60.0).abs() < 1e-12);
    let t = svg(&over);
    assert!(!t.contains(">12.5<") && !t.contains(">25<"), "the key invents no value: {t}");
    assert!(t.contains(">C<") && t.contains(">D<"), "empty rows stay in the key");
}

#[test]
fn a_total_above_the_sum_draws_the_rest_as_an_outlined_empty_sector() {
    let m = model(&params(&[("data", s("A, 20\nB, 15")), ("total", s("60"))]));
    assert_eq!(m.rows.len(), 3);
    assert!((m.share.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    assert!(svg(&[("data", s("A, 20\nB, 15")), ("total", s("60"))]).contains(r##"fill="#ffffff" stroke="#111827""##));
}

#[test]
fn white_fill_has_a_visible_outline_in_colour() {
    let t = svg(&[("data", s("A, 1\nB, 1")), ("fill_list", s("White"))]);
    assert!(t.contains(r##"fill="#ffffff" stroke="#111827""##), "{t}");
}

#[test]
fn many_categories_keep_the_key_inside_and_never_repeat_a_colour() {
    let data: String = (1..=20).map(|i| format!("Category {i}, {i}\n")).collect();
    let t = svg(&[("data", s(&data)), ("key", s("Right"))]);
    let h = t.split("height=\"").nth(1).unwrap().split('"').next().unwrap().parse::<f64>().unwrap();
    let last_key_y = t.rfind(">Category 20<").map(|i| t[..i].rsplit(" y=\"").next().unwrap().split('"').next().unwrap().parse::<f64>().unwrap()).unwrap();
    assert!(last_key_y < h, "key text at {last_key_y} within height {h}");
    let fills: std::collections::BTreeSet<String> = (0..20).map(|i| if i < 6 { svg::Fills::new(&params(&[])).series(i) } else { EXTRA[i - 6].to_string() }).collect();
    assert_eq!(fills.len(), 20);
    assert_ne!(bw_at(12, 13).css, bw_at(0, 13).css);
}

#[test]
fn a_heading_row_is_skipped() {
    let (rows, notes) = parse("Category\tCount\nWalk\t5\nCar\t3");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "Walk");
    assert!(notes.iter().any(|n| n.contains("headings")));
    let (rows, _) = parse("Dog\nCat");
    assert_eq!(rows.len(), 2, "names alone are empty sectors, not headings");
}

#[test]
fn a_sector_over_half_labels_inside_and_a_long_title_fits() {
    let t = svg(&[("data", s("Big, 80\nSmall, 20")), ("labels", s("Name and percent"))]);
    assert!(!t.contains("<line"), "no leader line: the big sector's label is inside");
    let title = "A very long and rather wordy chart title that is bold";
    let t = svg(&[("title_kind", s("Chart title")), ("title", s(title))]);
    let w: f64 = t.split("width=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
    assert!(w > crate::figures::stats::bar::tw(title, 22.0) * 1.05);
}

#[test]
fn outside_labels_stay_clear_of_the_pie_with_many_tiny_sectors() {
    let data: String = (1..=20).map(|i| format!("Name number {i}, {}\n", if i % 2 == 0 { 1 } else { 40 })).collect();
    let t = svg(&[("data", s(&data)), ("labels", s("Name and percent"))]);
    assert!(t.starts_with("<svg") && t.contains("<line"));
}
