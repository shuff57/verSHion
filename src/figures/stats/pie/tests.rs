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
    let t = svg(&[("data", s("A, 1\nB, 1")), ("own", s("_")), ("show_answers", Value::B(false))]);
    assert!(t.contains("<line") && !t.contains(">A<") && t.contains(">B<"));
    // the key writes the answer where the student writes on the line
    let t = svg(&[("data", s("A, 1\nB, 1")), ("own", s("_"))]);
    assert!(t.contains(">1<"), "{t}");
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
fn a_sector_over_half_labels_inside() {
    let t = svg(&[("data", s("Big, 80\nSmall, 20")), ("labels", s("Name and percent"))]);
    assert!(!t.contains("<polyline"), "no leader line: the big sector's label is inside");
}

#[test]
fn a_long_title_wraps_to_the_figure_and_the_image_stays_narrow() {
    let title = "Favourite sports of the sixty students in every one of the four classes that took part in the spring survey of the whole school";
    let t = svg(&[("title_kind", s("Chart title")), ("title", s(title))]);
    let w: f64 = t.split("width=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
    assert!(w < 900.0, "width {w}");
    assert!(t.matches("font-weight=\"600\"").count() >= 2, "wrapped onto several lines");
    let long = "x".repeat(MAX_TITLE + 5);
    assert!(warnings(&params(&[("title_kind", s("Chart title")), ("title", s(&long))])).iter().any(|w| w.contains("cut")));
}

#[test]
fn outside_labels_have_a_leader_each() {
    let data: String = (1..=20).map(|i| format!("Name number {i}, {}\n", if i % 2 == 0 { 1 } else { 40 })).collect();
    let t = svg(&[("data", s(&data)), ("labels", s("Name and percent"))]);
    assert!(t.starts_with("<svg") && t.matches("<polyline").count() >= 10);
}

#[test]
fn the_student_copy_description_never_leaks_a_hidden_value() {
    let over = |show: bool| {
        vec![("data", s("Soccer, 20\nBasketball\nTennis, 10")), ("total", s("60")), ("labels", s("Name and value")), ("show_answers", Value::B(show))]
    };
    let desc = |show: bool| {
        let t = svg(&over(show));
        t.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap().to_string()
    };
    let stu = desc(false);
    assert!(stu.contains("Soccer 33.3") && stu.contains("Tennis 16.7") && stu.contains("Basketball (empty)"), "{stu}");
    assert!(!stu.contains("50"), "{stu}");
    assert!(desc(true).contains("Basketball 50"));
    // two empty rows: no per-row values, an unnamed remainder is just an empty sector
    let two = svg(&[("data", s("Soccer, 20\nBasketball\nHockey")), ("total", s("60")), ("show_answers", Value::B(true))]);
    let d = two.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
    assert!(d.contains("Basketball (empty)") && d.contains("Hockey (empty)") && d.contains("an empty sector"), "{d}");
    assert!(!d.contains("Basketball 0") && !d.contains("66.7"), "{d}");
}

#[test]
fn thousands_groups_are_read_and_never_silently_truncated() {
    let (rows, notes) = parse("Apples\t1,200\nPears\t2 300\nPlums\t2,300");
    assert_eq!(rows.iter().map(|r| r.value).collect::<Vec<_>>(), vec![Some(1200.0), Some(2300.0), Some(2300.0)]);
    assert!(notes.is_empty(), "{notes:?}");
    let (rows, _) = parse("Apples, 1,200\nPears, \"2,300\"");
    assert_eq!(rows[0].value, Some(1200.0));
    assert_eq!(rows[1].value, Some(2300.0));
    let (rows, notes) = parse("Apples, 1, 200");
    assert_eq!(rows[0].value, Some(1.0));
    assert!(notes.iter().any(|n| n.contains("only the first two cells")), "{notes:?}");
    // a name, a space, then a grouped number: not "Walk 1" and 200
    let (rows, notes) = parse("Walk 1,200\nCar 800\nNew Town 12,345,678.5");
    assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["Walk", "Car", "New Town"]);
    assert_eq!(rows.iter().map(|r| r.value).collect::<Vec<_>>(), vec![Some(1200.0), Some(800.0), Some(12345678.5)]);
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn a_negative_is_refused_with_a_note_and_is_not_a_heading() {
    let (rows, notes) = parse("A, -5\nB, 3");
    assert_eq!(rows.len(), 1, "the negative row is ignored, not turned into an empty sector");
    assert!(notes.iter().any(|n| n.contains("cannot be negative")) && !notes.iter().any(|n| n.contains("headings")), "{notes:?}");
}

#[test]
fn all_zero_values_still_draw_an_empty_circle_and_say_so() {
    let t = svg(&[("data", s("A, 0\nB, 0"))]);
    assert!(t.contains("<circle"), "{t}");
    assert!(warnings(&params(&[("data", s("A, 0\nB, 0"))])).iter().any(|w| w.contains("zero")));
}

#[test]
fn twenty_black_and_white_fills_are_all_different() {
    let set: std::collections::BTreeSet<String> = (0..20).map(|i| bw_at(i, 20).css).collect();
    assert_eq!(set.len(), 20);
    let data: String = (1..=20).map(|i| format!("C{i}, 1\n")).collect();
    let t = svg(&[("data", s(&data)), ("look", s("Black and white"))]);
    for css in &set {
        if let Some(id) = css.strip_prefix("url(#") {
            let id = id.trim_end_matches(')');
            assert!(t.contains(&format!("id=\"{id}\"")), "{id} defined");
        }
    }
}

#[test]
fn one_decimal_count_per_chart_and_a_readable_value_and_percent_outside() {
    let t = svg(&[("data", s("A, 7\nB, 5.5\nC, 87.5")), ("values", s("Percents")), ("labels", s("Percent"))]);
    assert!(t.contains(">7.0%<") && t.contains(">5.5%<"), "{t}");
    let data: String = (1..=20).map(|i| format!("N{i}, {}\n", if i % 2 == 0 { 1 } else { 40 })).collect();
    let t = svg(&[("data", s(&data)), ("labels", s("Value and percent"))]);
    assert!(t.contains(">1, "), "joined with a separator: {t}");
}

fn desc_of(over: &[(&'static str, Value)]) -> String {
    let t = svg(over);
    t.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap().to_string()
}

#[test]
fn the_student_description_leaks_nothing_an_own_label_or_no_label_hides() {
    let d = desc_of(&[("data", s("Red, 110\nYellow, 70\nBlue, 180")), ("values", s("Degrees")), ("labels", s("Name and angle")), ("own", s("\nx\u{b0}")), ("show_answers", Value::B(false))]);
    assert!(d.contains("Red 30.6") && d.contains("Yellow") && !d.contains("Yellow 19.4"), "{d}");
    let d = desc_of(&[("data", s("A, 70\nB, 30")), ("values", s("Percents")), ("labels", s("Name and percent")), ("own", s("\n_")), ("show_answers", Value::B(false))]);
    assert!(d.contains("A 70") && !d.contains("B 30"), "{d}");
    let d = desc_of(&[("data", s("A, 70\nB, 30")), ("values", s("Percents")), ("labels", s("None")), ("show_answers", Value::B(false))]);
    assert!(!d.contains("70") && !d.contains("30"), "{d}");
}

#[test]
fn the_key_shows_the_answer_for_a_letter_label() {
    let over = [("data", s("Red, 110\nBlue, 80\nGreen, 100\nYellow")), ("values", s("Degrees")), ("labels", s("Name and angle")), ("own", s("\n\n\nx\u{b0}"))];
    let t = svg(&over);
    assert!(t.contains(">x = 70\u{b0}<"), "{t}");
    let mut stu = over.to_vec();
    stu.push(("show_answers", Value::B(false)));
    let stu = svg(&stu);
    assert!(stu.contains(">x\u{b0}<") && !stu.contains(">x = "), "{stu}");
}


#[test]
fn fractions_stay_fractions_across_the_chart() {
    let t = svg(&[("data", s("A, 250\nB, 137\nC, 613")), ("labels", s("Fraction"))]);
    assert!(t.contains(">1/4<") && t.contains(">137/1000<") && t.contains(">613/1000<"), "{t}");
}

#[test]
fn a_negative_percent_is_never_the_remainder() {
    let m = model(&params(&[("data", s("A, 60\nB, -10\nC, 30")), ("values", s("Percents"))]));
    assert!(m.rows.iter().all(|r| r.name != "B"), "the negative row is gone");
    assert!(m.notes.iter().any(|n| n.contains("ignored")));
}

#[test]
fn tiny_values_spaced_thousands_and_contrast() {
    assert_eq!(fixed(0.0004, 3), "<0.001");
    assert_eq!(fixed(0.5, 0), "1");
    let (rows, notes) = parse("New York 1 200");
    assert_eq!(rows[0].name, "New York 1");
    assert!(notes.iter().any(|n| n.contains("put a comma")), "{notes:?}");
    assert!(!hex_dark("#17becf"), "dark type on cyan");
    assert!(hex_dark("#000000"));
}
/// Every label drawn inside the pie (middle-anchored text): its box against the circle and the
/// radii of the sector holding its centre. Returns the worst overhang in px (0 when it fits).
fn worst_overhang(t: &str) -> (f64, Vec<(String, bool)>) {
    let attr = |tag: &str, name: &str| -> Option<String> {
        let pat = format!(" {name}=\"");
        let i = tag.find(&pat)? + pat.len();
        Some(tag[i..].split('"').next()?.to_string())
    };
    let num = |v: Option<String>| v.and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    // sectors: M cx,cy L x0,y0 A r,r 0 l s x1,y1 Z
    let mut sectors: Vec<(f64, f64, f64, f64)> = Vec::new(); // cx, cy, start deg, span deg
    let mut centre = (0.0, 0.0);
    for seg in t.split("<path d=\"M").skip(1) {
        let d = seg.split('"').next().unwrap();
        let nums: Vec<f64> = d.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter(|x| !x.is_empty()).filter_map(|x| x.parse().ok()).collect();
        if nums.len() < 10 || !d.contains('L') {
            continue;
        }
        let (cx, cy, x0, y0, x1, y1) = (nums[0], nums[1], nums[2], nums[3], nums[nums.len() - 2], nums[nums.len() - 1]);
        let ang = |x: f64, y: f64| (x - cx).atan2(-(y - cy)).to_degrees().rem_euclid(360.0);
        let (a0, a1) = (ang(x0, y0), ang(x1, y1));
        centre = (cx, cy);
        sectors.push((cx, cy, a0, (a1 - a0).rem_euclid(360.0)));
    }
    let r = 150.0;
    let (mut worst, mut seen) = (0.0f64, Vec::new());
    for tag in t.split("<text ").skip(1) {
        let head_s = format!(" {}", tag.split('>').next().unwrap());
        let head = head_s.as_str();
        if attr(head, "text-anchor").as_deref() != Some("middle") {
            continue;
        }
        let content = tag.split_once('>').unwrap().1.split("</text>").next().unwrap().replace("&amp;", "&");
        let (x, y, size) = (num(attr(head, "x")), num(attr(head, "y")), num(attr(head, "font-size")));
        if (x - centre.0).hypot(y - centre.1) > r {
            continue; // the title
        }
        let bold = head.contains("font-weight");
        let w = crate::figures::stats::bar::tw(&content, size) * if bold { 1.1 } else { 1.0 };
        let mid = (x, y - size * 0.35);
        let ang = |px: f64, py: f64| (px - centre.0).atan2(-(py - centre.1)).to_degrees().rem_euclid(360.0);
        let home = sectors.iter().find(|s| (ang(mid.0, mid.1) - s.2).rem_euclid(360.0) <= s.3);
        let mut ok = true;
        for (dx, dy) in [(-1.0, -0.65), (1.0, -0.65), (-1.0, 0.35), (1.0, 0.35)] {
            let (px, py) = (x + dx * w / 2.0, y + dy * size);
            let dist = (px - centre.0).hypot(py - centre.1);
            worst = worst.max(dist - r);
            if dist > r {
                ok = false;
            }
            if let Some(h) = home {
                if (ang(px, py) - h.2).rem_euclid(360.0) > h.3 + 0.5 && dist > 6.0 {
                    ok = false;
                    worst = worst.max(1.0);
                }
            }
        }
        seen.push((content, ok));
    }
    (worst, seen)
}

#[test]
fn a_long_name_inside_a_sector_stays_inside_the_circle_and_its_radii() {
    let t = svg(&[("data", s("Walking with friends, 14\nCar, 12\nBus, 10\nBike, 4")), ("labels", s("Name and percent"))]);
    let (worst, seen) = worst_overhang(&t);
    assert!(!seen.is_empty() && worst <= 0.0 && seen.iter().all(|x| x.1), "overhang {worst}: {seen:?}");
    // a wrapped name keeps the name's weight on every line; the value line is regular
    let bold: Vec<&str> = t.split("<text ").skip(1).filter(|x| x.split('>').next().unwrap().contains("font-weight")).map(|x| x.split_once('>').unwrap().1.split("</text>").next().unwrap()).collect();
    assert!(bold.contains(&"Walking with") && bold.contains(&"friends"), "{bold:?}");
    assert!(!bold.iter().any(|b| b.contains('%')), "{bold:?}");
    let t = svg(&[("data", s("Tom & Jerry, 43\nOthers, 57")), ("values", s("Percents")), ("labels", s("Name and percent"))]);
    let (worst, seen) = worst_overhang(&t);
    assert!(seen.iter().any(|x| x.0 == "Tom & Jerry") && worst <= 0.0 && seen.iter().all(|x| x.1), "{worst} {seen:?}");
}

fn key_text(over: &[(&'static str, Value)]) -> String {
    svg(over)
}

#[test]
fn a_letter_label_is_answered_in_the_unit_the_chart_labels_show() {
    // counts data, angle labels: Bike is 4 of 40, a tenth of the circle
    let counts = |mode: &str, own: &str| key_text(&[("data", s("Walk, 14\nCar, 12\nBus, 10\nBike, 4")), ("labels", s(mode)), ("own", s(&format!("\n\n\n{own}")))]);
    assert!(counts("Name and angle", "x\u{b0}").contains(">x = 36\u{b0}<"));
    assert!(counts("Angle", "x").contains(">x = 36\u{b0}<"));
    assert!(counts("Name and percent", "x").contains(">x = 10%<"));
    assert!(counts("Percent", "x").contains(">x = 10%<"));
    assert!(counts("Name and value", "x").contains(">x = 4<"));
    assert!(counts("Value and percent", "x").contains(">x = 4<"));
    assert!(counts("Name and fraction", "x").contains(">x = 1/10<"));
    // the letter's own unit wins
    assert!(counts("Name and percent", "x\u{b0}").contains(">x = 36\u{b0}<"));
    assert!(counts("Name and angle", "x%").contains(">x = 10%<"));
    // percents data, angle labels
    let pct = key_text(&[("data", s("Walk, 40\nCar, 30\nBus, 20\nBike, 10")), ("values", s("Percents")), ("labels", s("Name and angle")), ("own", s("\n\n\nx"))]);
    assert!(pct.contains(">x = 36\u{b0}<"), "{pct}");
    let pct = key_text(&[("data", s("Walk, 40\nCar, 20\nBus, 20\nBike, 20")), ("values", s("Percents")), ("labels", s("Name and angle")), ("own", s("\n\n\nx"))]);
    assert!(pct.contains(">x = 72\u{b0}<"), "{pct}");
    // degrees data, percent labels
    let deg = key_text(&[("data", s("A, 90\nB, 270")), ("values", s("Degrees")), ("labels", s("Name and percent")), ("own", s("x"))]);
    assert!(deg.contains(">x = 25%<"), "{deg}");
    // an empty sector: total 60, A 20, B 10, so C is half the circle
    let empty = |mode: &str| key_text(&[("data", s("A, 20\nB, 10\nC")), ("total", s("60")), ("labels", s(mode)), ("own", s("\n\nx"))]);
    assert!(empty("Name and angle").contains(">x = 180\u{b0}<"));
    assert!(empty("Name and percent").contains(">x = 50.0%<"));
    assert!(empty("Name and value").contains(">x = 30<"));
    assert!(empty("Name and fraction").contains(">x = 1/2<"));
}

#[test]
fn a_blank_line_is_answered_in_the_label_unit_on_the_key() {
    let t = key_text(&[("data", s("A, 14\nB, 12\nC, 14")), ("labels", s("Angle")), ("own", s("\n_"))]);
    assert!(t.contains(">108\u{b0}<"), "{t}");
    let t = key_text(&[("data", s("A, 14\nB, 12\nC, 14")), ("labels", s("Percent")), ("own", s("\n_"))]);
    assert!(t.contains(">30%<"), "{t}");
}

/// The leader polylines of a drawing, as lists of points.
fn leaders(t: &str) -> Vec<Vec<(f64, f64)>> {
    t.split("<polyline points=\"").skip(1).map(|seg| {
        seg.split('"').next().unwrap().split(' ').map(|p| {
            let (x, y) = p.split_once(',').unwrap();
            (x.parse::<f64>().unwrap(), y.parse::<f64>().unwrap())
        }).collect()
    }).collect()
}

fn crosses(a: ((f64, f64), (f64, f64)), b: ((f64, f64), (f64, f64))) -> bool {
    let o = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(b.0, b.1, a.0), o(b.0, b.1, a.1), o(a.0, a.1, b.0), o(a.0, a.1, b.1));
    d1 * d2 < -1e-9 && d3 * d4 < -1e-9
}

fn seg_dist(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
}

fn assert_clean_leaders(t: &str) {
    let ls = leaders(t);
    assert!(ls.len() >= 10, "{} leaders", ls.len());
    // the pie's centre is the apex of the first sector path
    let c: Vec<f64> = t.split("<path d=\"M").skip(1).find(|x| x.split('"').next().unwrap().contains(" L")).unwrap().split(' ').next().unwrap().split(',').map(|v| v.parse().unwrap()).collect();
    let centre = (c[0], c[1]);
    let segs: Vec<(usize, usize, ((f64, f64), (f64, f64)))> = ls.iter().enumerate().flat_map(|(i, l)| l.windows(2).enumerate().map(move |(j, w)| (i, j, (w[0], w[1])))).collect();
    for (ai, a) in segs.iter().enumerate() {
        for b in &segs[ai + 1..] {
            assert!(a.0 == b.0 || !crosses(a.2, b.2), "leaders {} and {} cross: {:?} {:?}", a.0, b.0, a.2, b.2);
        }
        // beyond the first segment (which leaves the rim) every leader stays 4 px off the circle
        if a.1 >= 1 {
            let nearest = seg_dist(centre, a.2 .0, a.2 .1);
            assert!(nearest >= 150.0 + 4.0, "leader {} within {} of the pie centre", a.0, nearest);
        }
    }
}

#[test]
fn leaders_never_cross_and_stay_off_the_rim_with_many_tiny_slices() {
    let mut data = String::from("Big, 82\n");
    for i in 1..=18 {
        data.push_str(&format!("S{i}, 1\n"));
    }
    assert_clean_leaders(&svg(&[("data", s(&data)), ("labels", s("Name"))]));
    let twenty: String = (1..=20).map(|i| format!("Item {i}, {}\n", if i == 10 || i == 20 { 30 } else { 1 })).collect();
    assert_clean_leaders(&svg(&[("data", s(&twenty)), ("labels", s("Name")), ("label_size", s("Large"))]));
    assert_clean_leaders(&svg(&[("data", s(&twenty)), ("labels", s("Name"))]));
}

fn sector_fills(t: &str) -> Vec<String> {
    t.split("<path d=\"M").skip(1).filter(|seg| seg.split('"').next().unwrap().contains(" L")).map(|seg| seg.split("fill=\"").nth(1).unwrap().split('"').next().unwrap().to_string()).collect()
}

#[test]
fn picked_fills_never_collapse_or_repeat_an_automatic_one() {
    let t = svg(&[("data", s("A, 1\nB, 1\nC, 1\nD, 1")), ("fill_list", s("Blue, Red, Black, Gray")), ("style", s("Print"))]);
    let f = sector_fills(&t);
    let set: std::collections::BTreeSet<&String> = f.iter().collect();
    assert_eq!(f.len(), 4);
    assert_eq!(set.len(), 4, "{f:?}");
    let t = svg(&[("data", s("A, 1\nB, 1\nC, 1")), ("fill_list", s(", Blue")), ("style", s("Print"))]);
    let f = sector_fills(&t);
    assert_eq!(f.iter().collect::<std::collections::BTreeSet<_>>().len(), 3, "{f:?}");
    // in colour a picked colour is not repeated by an automatic fill either
    let t = svg(&[("data", s("A, 1\nB, 1\nC, 1")), ("fill_list", s(", Blue"))]);
    let f = sector_fills(&t);
    assert_eq!(f.iter().collect::<std::collections::BTreeSet<_>>().len(), 3, "{f:?}");
}

#[test]
fn every_fill_reaches_aa_contrast_for_its_label() {
    let fills = svg::Fills::new(&params(&[]));
    let mut all: Vec<String> = (0..6).map(|i| fills.series(i)).collect();
    all.extend(EXTRA.iter().map(|c| c.to_string()));
    all.extend(["#4f7cd4", "#c9494f", "#4aa36b", "#e0803c", "#8a62c4", "#9aa0a6", "#7f7f7f"].iter().map(|c| c.to_string()));
    for hex in all {
        let css = solid(&hex).css;
        let c = best_contrast(lum(rgb(&css).unwrap()));
        assert!(c >= 4.5, "{hex} -> {css}: {c}");
    }
    assert_eq!(bw(5).css, BW_MID);
    assert!(best_contrast(lum(rgb(BW_MID).unwrap())) >= 4.5 && best_contrast(lum(rgb(BW_DARK).unwrap())) >= 4.5);
}

#[test]
fn with_every_value_zero_the_key_swatches_are_white() {
    let t = svg(&[("data", s("A, 0\nB, 0")), ("key", s("Right"))]);
    let blue = solid("#4f7cd4").css;
    assert!(!t.contains(&blue), "{t}");
}
