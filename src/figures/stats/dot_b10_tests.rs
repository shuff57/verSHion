use super::*;
use crate::figures::{FigureDef, Value, registry, start_params};
use crate::model::FigureSpec;
use serde_json::{Value as Json, json};

fn fig() -> FigureDef {
    registry().into_iter().find(|d| d.id == "dot-plot").unwrap()
}

fn svg(over: Json) -> String {
    let d = fig();
    d.svg(&crate::figures::params_from_json(&d, &over))
}

fn dots(s: &str) -> usize {
    s.matches("<circle").count() - s.matches("data-end=\"circle\"").count()
}

fn xs(s: &str) -> usize {
    s.matches("data-mark=\"x\"").count()
}

fn texts(s: &str) -> Vec<&str> {
    s.split("<text")
        .skip(1)
        .map(|t| t.split('>').nth(1).unwrap().split('<').next().unwrap())
        .collect()
}

#[test]
fn cells_read_decimals_fractions_and_mixed_numbers() {
    let v = |c: &str| read_cell(c).map(|x| (x.v, x.den));
    assert_eq!(v("-2.5"), Some((-2.5, None)));
    assert_eq!(v("3/4"), Some((0.75, Some(4))));
    assert_eq!(v("2/4"), Some((0.5, Some(2))));
    assert_eq!(v("4/2"), Some((2.0, None)));
    assert_eq!(v("1 1/2"), Some((1.5, Some(2))));
    assert_eq!(v("-1 1/4"), Some((-1.25, Some(4))));
    assert_eq!(v("−3/8"), Some((-0.375, Some(8))));
    assert_eq!(v("1 5/4"), Some((2.25, Some(4))));
    for bad in ["1/0", "a/b", "1 1/2/3", "x", "", "1e13"] {
        assert_eq!(v(bad), None, "{bad:?}");
    }
}

#[test]
fn a_list_without_fractions_reads_exactly_as_before() {
    for text in [
        "5, 6, 6, 7",
        "1 2 3\n4\t5",
        "3, 72, abc, 4",
        "\"1\";\"2\"",
        "",
        "1e12, 1e13",
    ] {
        let mut p = start_params(&fig());
        p.set_s("data", text);
        assert_eq!(
            list_values(&p).iter().map(|x| x.v).collect::<Vec<_>>(),
            data::numbers(text),
            "{text:?}"
        );
    }
}

#[test]
fn fractions_are_plotted_and_the_line_is_numbered_in_them() {
    let text = "1/4, 1/2, 1/2, 3/4, 1 1/4\n1 1/2 1 1/2";
    let ListRead { values: v, skipped, truncated, .. } = read_list(text);
    assert_eq!((v.len(), skipped, truncated), (7, 0, false));
    let s = svg(json!({"data": text}));
    assert_eq!(dots(&s), 7);
    let t = texts(&s);
    assert_eq!(
        t.iter().filter(|x| **x == "4").count(),
        3,
        "quarters stacked over 4: {t:?}"
    );
    assert!(!t.iter().any(|x| x.contains('.')), "no decimals: {t:?}");
    assert!(t.contains(&"1"));
    assert!(s.contains("stroke-width=\"1.1\""));
}

#[test]
fn fraction_axis_counts_by_a_fraction_and_stays_readable() {
    assert_eq!(auto_axis(&[0.25, 1.5]), ((0.25, 1.5, 0.25), true));
    assert_eq!(auto_axis(&[0.25, 0.5]), ((0.0, 1.0, 0.25), true));
    assert_eq!(auto_axis(&[0.5, 30.5]), ((0.0, 35.0, 5.0), false));
    assert_eq!(frac_label(1.25, 4), Some(("1".into(), Some((1, 4)))));
    assert_eq!(frac_label(-0.5, 4), Some(("−".into(), Some((1, 2)))));
    assert_eq!(frac_label(2.0, 4), Some(("2".into(), None)));
    assert_eq!(frac_label(0.3, 4), None);
}

#[test]
fn values_and_counts_stack_each_value_that_many_times() {
    let text = "Value, Count\n3, 2\n4 5\n5\t1\n\"1 1/2\", 3\n8, 0\n9, -1";
    let TableRead { rows, skipped } = read_table(text);
    assert_eq!(skipped, 2);
    let count = |x: f64| -> u64 { rows.iter().filter(|(y, _)| y.v == x).map(|(_, n)| n).sum() };
    assert_eq!(
        (count(3.0), count(4.0), count(5.0), count(1.5)),
        (2, 5, 1, 3)
    );
    assert_eq!(
        dots(&svg(
            json!({"data_form":"Values and counts", "data_table":text})
        )),
        11
    );
    assert_eq!(summary(text, true), "11 values (2 rows skipped)");
    assert_eq!(
        dots(&svg(
            json!({"data_form":"Values and counts", "data_table":"1, 999999999"})
        )),
        100
    );
}

#[test]
fn switching_data_forms_only_fills_an_empty_destination() {
    let mut p = start_params(&fig());
    let original = "1 1/2, 2, 3/4, 2";
    p.set_s("data", original);
    p.set_s(FORM, FORMS[1]);
    panel_changed(FORM, &Value::S(FORMS[0].into()), &mut p);
    assert_eq!(p.s(TABLE), "3/4, 1\n1 1/2, 1\n2, 2");
    let edit = "  3/4, 2\n  1 1/2, 1";
    p.set_s(TABLE, edit);
    p.set_s(FORM, FORMS[0]);
    panel_changed(FORM, &Value::S(FORMS[1].into()), &mut p);
    assert_eq!(p.s("data"), original);
    p.set_s(FORM, FORMS[1]);
    panel_changed(FORM, &Value::S(FORMS[0].into()), &mut p);
    assert_eq!(p.s(TABLE), edit);
    p.set_s("data", "");
    p.set_s(FORM, FORMS[0]);
    panel_changed(FORM, &Value::S(FORMS[1].into()), &mut p);
    assert_eq!(p.s("data"), "3/4, 3/4, 1 1/2");
}

#[test]
fn empty_data_is_a_blank_number_line() {
    for show in [true, false] {
        let s = svg(json!({"data":"", "show_answers":show, "title_mode":"Blank line"}));
        assert_eq!(dots(&s), 0);
        let t = texts(&s);
        assert!(t.contains(&"0") && t.contains(&"10"), "0 to 10: {t:?}");
        assert!(!s.contains("Enter some data"));
        assert!(s.contains("data-blank=\"title\""));
    }
    assert!(svg(json!({"data":"abc"})).contains("Enter some data"));
    let manual = svg(json!({"data":"", "axis_auto":false}));
    assert!(texts(&manual).contains(&"12"));
}

#[test]
fn x_marks_replace_the_dots() {
    let d = fig();
    let n = data::numbers(start_params(&d).s("data")).len();
    let s = svg(json!({"mark_shape":"X's"}));
    assert_eq!((dots(&s), xs(&s)), (0, n));
    let student = svg(json!({"mark_shape":"X's", "show_answers":false}));
    assert_eq!((dots(&student), xs(&student)), (0, 0));
}

#[test]
fn titles_as_text_blank_line_or_none() {
    let text = svg(json!({"title":"Shoe sizes", "x_title":"Size"}));
    assert!(text.contains(">Shoe sizes<") && text.contains(">Size<"));
    let blank = svg(json!({
        "title":"Shoe sizes", "x_title":"Size", "title_mode":"Blank line", "x_title_mode":"Blank line",
        "show_answers":false
    }));
    assert!(!blank.contains(">Shoe sizes<") && !blank.contains(">Size<"));
    assert_eq!(blank.matches("data-blank=\"title\"").count(), 1);
    assert_eq!(blank.matches("data-blank=\"x_title\"").count(), 1);
    let none = svg(
        json!({"title":"Shoe sizes", "x_title":"Size", "title_mode":"None", "x_title_mode":"None"}),
    );
    assert!(
        !none.contains(">Shoe sizes<") && !none.contains(">Size<") && !none.contains("data-blank")
    );
}

#[test]
fn which_ticks_are_numbered() {
    for (choice, expected) in [
        ("Every 2nd tick", vec!["−2", "0", "2", "4", "6", "8", "10"]),
        ("Every 4th tick", vec!["0", "4", "8"]),
        ("Every 5th tick", vec!["0", "5", "10"]),
        ("Every 10th tick", vec!["0", "10"]),
        ("No numbers", vec![]),
    ] {
        let s = svg(
            json!({"axis_auto":false, "axis_min":-3, "axis_max":10, "numbers":choice, "show_answers":false}),
        );
        assert_eq!(texts(&s), expected, "{choice}");
        assert_eq!(
            s.matches("<line").count(),
            15,
            "all 14 ticks and the number line stay"
        );
    }
}

#[test]
fn the_fraction_line_fits_dense_mixed_labels() {
    let s = svg(json!({
        "data":"-1 3/4, 1/8, 1 1/8", "label_size":"Large", "x_title":"Inches",
        "left_end":"Circle", "right_end":"Triangle arrow"
    }));
    let t = texts(&s);
    assert_eq!(dots(&s), 3);
    assert_eq!(t.last(), Some(&"Inches"));
    assert_eq!(s.matches("stroke-width=\"1.8\"").count(), 24, "all eighth ticks stay");
    assert!(!t.contains(&"8") && t.contains(&"4"), "crowded eighth labels thin to quarters: {t:?}");
    let width: f64 = s
        .split("viewBox=\"")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();
    assert!(width > 640.0 && width <= 880.0);
}

#[test]
fn typed_manual_fraction_range_starts_at_the_entered_value() {
    let d = fig();
    let mut p = start_params(&d);
    p.set_s("data", "1/8, 3/4");
    p.set_b("axis_auto", false);
    for (key, text) in [
        ("axis_min", "1/8"),
        ("axis_max", "1 1/8"),
        ("axis_step", "1/4"),
    ] {
        let value = crate::figures::typed_float("dot-plot", key, text, &mut p).unwrap();
        p.set_f(key, value);
    }
    let s = d.svg(&p);
    assert_eq!(dots(&s), 2);
    assert_eq!(
        texts(&s),
        ["1", "8", "3", "8", "5", "8", "7", "8", "1", "1", "8"]
    );
}

#[test]
fn each_end_finishes_as_chosen() {
    let plain = svg(json!({}));
    assert!(!plain.contains("data-end") && !plain.contains("<polygon"));
    let s = svg(json!({"left_end":"Circle", "right_end":"Triangle arrow"}));
    assert_eq!(s.matches("data-end=\"circle\"").count(), 1);
    assert_eq!(s.matches("<polygon").count(), 1);
    let s = svg(json!({"left_end":"Line arrow"}));
    assert_eq!(s.matches("data-end=\"line\"").count(), 1);
    let s = svg(json!({"left_end":"Circle", "right_end":"Circle", "show_answers":false}));
    assert_eq!(s.matches("data-end=\"circle\"").count(), 2);
    let x: Vec<f64> = s
        .split("cx=\"")
        .skip(1)
        .map(|t| t.split('"').next().unwrap().parse().unwrap())
        .collect();
    assert_eq!(x.len(), 2);
    assert!(x[0] > 4.5 && x[1] < 640.0 - 4.5);
}

#[test]
fn old_figures_match_head_svg() {
    let d = fig();
    // FNV-1a of annotated SVGs rendered with the unmodified 90f806d binary's library.
    for (over, expected) in [
        (json!({}), 0xbf00470ec7dd43be),
        (
            json!({"title":"Shoe sizes", "x_title":"Size"}),
            0x872ad04206fd4b77,
        ),
        (
            json!({"style":"Print", "dot_size":"Large", "label_size":"Large"}),
            0x9e0effd7b3ac52c2,
        ),
        (
            json!({"show_answers":false, "marks":"Mean and median", "title":"Shoe sizes", "x_title":"Size"}),
            0x5b4a92988788ce41,
        ),
        (
            json!({"axis_auto":false, "axis_min":0, "axis_max":15, "axis_step":0.5, "data":"5.5; 7; 8; 8.15; 8.20"}),
            0xa73b495a5bd0d1b3,
        ),
        (
            json!({"data":"1.2, 1.4, 2.9, 3.1, 7.6", "group":2, "marks":"Mean and median"}),
            0x9a444b3260719d3d,
        ),
        (json!({"data":"abc"}), 0xe8d2d1ff09536812),
        (json!({"data":""}), 0xe8d2d1ff09536812),
        (json!({"data":"1/2, 1 1/4, 4"}), 0x6c4c0a35aa382a03),
    ] {
        let changed = over.as_object().unwrap().clone();
        let spec = FigureSpec {
            id: d.id.into(),
            params: changed.clone().into_iter().collect(),
        };
        let current = crate::figures::spec::params_from_spec(&d, &spec);
        let link = crate::figures::share::link_params(&d, changed, 7);
        for p in [&current, &link] {
            let h = d.svg(p).bytes().fold(0xcbf29ce484222325_u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
            });
            assert_eq!(h, expected, "{over}");
        }
    }
}

#[test]
fn new_plots_keep_their_layout_on_save_and_share() {
    use crate::figures::{share, spec};
    let d = fig();
    let base = start_params(&d);
    for data in [base.s("data"), "", "1/2, 1 1/4, 4"] {
        let mut p = base.clone();
        p.set_s("data", data);
        let link = share::encode(d.id, &p, &base);
        let (_, changed) = share::decode(&link).unwrap();
        let reloaded = share::link_params(&d, changed, share::version(&link));
        assert_eq!(d.svg(&reloaded), d.svg(&p));
        assert_eq!(d.svg(&spec::params_from_spec(&d, &spec::spec_for(d.id, &p))), d.svg(&p));
    }
}

#[test]
fn fractional_data_leaves_room_for_mean_and_median_near_an_end() {
    let s = svg(json!({"data":"0, 0, 1/8", "marks":"Mean and median"}));
    let ys: Vec<f64> = s
        .split("<text")
        .filter(|el| el.contains(">mean =") || el.contains(">median ="))
        .map(|el| {
            el.split("y=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap()
                .parse()
                .unwrap()
        })
        .collect();
    assert_eq!(ys.len(), 2);
    assert!(ys[1] - ys[0] > 14.0, "{ys:?}");
}

#[test]
fn print_stays_grey_with_x_marks_and_ends() {
    let s = svg(
        json!({"style":"Print", "mark_shape":"X's", "left_end":"Circle", "right_end":"Line arrow"}),
    );
    let png = crate::figures::to_png(&s, 1.0).unwrap();
    let img = image::load_from_memory(&png).unwrap().to_rgba8();
    let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
    assert!(img.pixels().all(|px| hue(px) <= 24));
}

#[test]
fn zero_counts_fit_the_axis_without_adding_marks_or_affecting_statistics() {
    for (data, labels, count) in [
        (
            "1, 3\n2, 4\n3, 0\n4, 0\n5, 0",
            vec!["1", "2", "3", "4", "5", "mean = 1.57"],
            7,
        ),
        ("1, 0\n2, 0", vec!["0", "1", "2", "3", "4"], 0),
    ] {
        let s = svg(json!({"data_form":"Values and counts", "data_table":data, "marks":"Mean"}));
        assert_eq!(texts(&s), labels);
        assert_eq!(dots(&s), count);
    }
}

#[test]
fn ticks_use_the_same_manual_grid_with_all_mark_and_line_options() {
    for (min, max, step, expected) in [
        (
            0.5,
            10.0,
            1.0,
            vec![
                "0.5", "1.5", "2.5", "3.5", "4.5", "5.5", "6.5", "7.5", "8.5", "9.5",
            ],
        ),
        (
            1.0,
            19.0,
            2.0,
            vec!["1", "3", "5", "7", "9", "11", "13", "15", "17", "19"],
        ),
    ] {
        let d = fig();
        let mut p = crate::figures::params_from_json(
            &d,
            &json!({
                "axis_auto":false, "axis_min":min, "axis_max":max, "axis_step":step, "show_answers":false
            }),
        );
        let tick_x = |s: &str| -> Vec<String> {
            s.split("<line ")
                .filter(|tag| tag.contains("stroke-width=\"1.8\""))
                .map(|tag| tag.split('"').nth(1).unwrap().into())
                .collect()
        };
        let dots = d.svg(&p);
        assert_eq!(texts(&dots), expected);
        p.set_s("mark_shape", "X's");
        p.set_s("numbers", "No numbers");
        p.set_s("left_end", "Circle");
        p.set_s("right_end", "Triangle arrow");
        assert_eq!(tick_x(&dots), tick_x(&d.svg(&p)));
    }
}

#[test]
fn automatic_fraction_axes_use_an_aligned_grid_or_a_decimal_fallback() {
    for (data, expected) in [
        (
            "1/4, 12 3/4, 6 1/2, 3",
            vec!["0", "2", "4", "6", "8", "10", "12", "14"],
        ),
        (
            "1/3, 10, 5 2/3, 7",
            vec!["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"],
        ),
        (
            "1/16, 2 15/16, 1 3/16",
            vec!["0", "0.5", "1", "1.5", "2", "2.5", "3"],
        ),
    ] {
        let s = svg(json!({"data":data, "label_size":"Large"}));
        assert_eq!(texts(&s), expected, "{data}");
        assert_eq!(dots(&s), data.split(',').count());
    }
}

#[test]
fn manual_fraction_text_drives_labels_and_round_trips() {
    use crate::figures::{float_text, share, spec, typed_float};
    let d = fig();
    let base = start_params(&d);
    let mut p = base.clone();
    p.set_s("data", "");
    p.set_b("axis_auto", false);
    p.set_f("axis_max", 2.0);
    let typed = " 2 / 8 ";
    let step = typed_float(d.id, "axis_step", typed, &mut p).unwrap();
    p.set_f("axis_step", step);
    let figure = spec::spec_for(d.id, &p);
    let link = share::encode(d.id, &p, &base);
    let (_, changed) = share::decode(&link).unwrap();
    for mut p in [
        p,
        spec::params_from_spec(&d, &figure),
        share::link_params(&d, changed, share::version(&link)),
    ] {
        assert_eq!(float_text(d.id, "axis_step", &p).as_deref(), Some(typed));
        let s = d.svg(&p);
        assert_eq!(s.matches("stroke-width=\"1.1\"").count(), 6);
        assert!(!texts(&s).contains(&"0.25"));
        let step = typed_float(d.id, "axis_step", "0.25", &mut p).unwrap();
        p.set_f("axis_step", step);
        assert!(texts(&d.svg(&p)).contains(&"0.25"));
    }
}

#[test]
fn titles_keep_their_size_until_the_measured_text_needs_more_room() {
    let chart_title = "Number of siblings in our class";
    let axis_title = "Number of siblings reported by each student";
    let s = svg(json!({"mark_shape":"X's", "title":chart_title, "x_title":axis_title}));
    for (title, size) in [(chart_title, 18.2), (axis_title, 15.4)] {
        let text = s
            .split("<text ")
            .find(|t| t.contains(&format!(">{title}<")))
            .unwrap();
        let actual: f64 = text
            .split("font-size=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!((actual - size).abs() < 1e-9, "{text}");
    }
    let label = "Wide title for the class ".repeat(16);
    let s = svg(json!({"mark_shape":"X's", "title":label}));
    assert!(texts(&s).iter().filter(|v| v.contains("Wide")).count() > 1);
}

#[test]
fn crowded_numbering_thins_by_an_even_stride() {
    for (max, numbers, requested) in [(50, "Every tick", 1), (100, "Every 2nd tick", 2)] {
        let s = svg(json!({
            "axis_auto":false,"axis_min":0,"axis_max":max,"axis_step":1,
            "label_size":"Large","mark_shape":"X's","numbers":numbers,"show_answers":false
        }));
        let labels: Vec<i32> = texts(&s).into_iter().map(|t| t.parse().unwrap()).collect();
        let stride = labels[1] - labels[0];
        assert!(stride > requested && stride % requested == 0, "{labels:?}");
        assert!(
            labels.windows(2).all(|w| w[1] - w[0] == stride),
            "{labels:?}"
        );
        assert_eq!(s.matches("stroke-width=\"1.8\"").count(), max as usize + 1);
    }
}

#[test]
fn dense_fraction_numbering_prefers_coarser_subdivisions() {
    let s = svg(json!({
        "data":"1/8","axis_auto":false,"axis_min":0,"axis_max":5,"axis_step":0.125,
        "label_size":"Large","show_answers":false
    }));
    assert_eq!(s.matches("stroke-width=\"1.8\"").count(), 41);
    assert!(!texts(&s).contains(&"8"), "{}", texts(&s).join(", "));
    assert!(texts(&s).contains(&"2"));
}

#[test]
fn frequency_rows_use_the_first_two_cells() {
    let s = svg(json!({"data_form":"Values and counts", "data_table":"1,3,\n2,4,\n3\t2\tnote"}));
    assert_eq!(dots(&s), 9);
}

#[test]
fn negative_proper_fraction_stays_together_when_switching_forms() {
    let mut p = start_params(&fig());
    p.set_s("data", "-3/4, -1 1/4");
    p.set_s(FORM, FORMS[1]);
    panel_changed(FORM, &Value::S(FORMS[0].into()), &mut p);
    assert_eq!(p.s(TABLE), "-1 1/4, 1\n-3/4, 1");
}

#[test]
fn spaces_around_a_fraction_slash_keep_it_one_value() {
    let text = "3 / 4 -1 1 / 2 2";
    let s = svg(json!({"data":text}));
    assert_eq!(dots(&s), 3);
    assert_eq!(summary(text, false), "3 values");
}

#[test]
fn tall_stacks_keep_distinct_marks_inside_the_page() {
    let s = svg(json!({
        "data_form":"Values and counts","data_table":"1, 101","marks":"Mean and median"
    }));
    let circles: Vec<_> = s
        .split("<circle ")
        .skip(1)
        .map(|tag| {
            let get = |attr: &str| -> f64 {
                tag.split(&format!("{attr}=\""))
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap()
            };
            (get("cy"), get("r"))
        })
        .collect();
    assert_eq!(circles.len(), 100);
    assert!(circles.iter().all(|&(y, r)| r >= 1.5 && y > r));
    assert!(
        circles
            .windows(2)
            .all(|w| w[0].0 - w[1].0 > w[0].1 + w[1].1)
    );
}

#[test]
fn oversized_manual_grid_warns_in_the_editor_only() {
    let d = fig();
    let p = crate::figures::params_from_json(&d, &json!({
        "data":"1, 2, 5","mark_shape":"X's","axis_auto":false,"axis_min":0,"axis_max":1000,"axis_step":2
    }));
    let s = d.svg(&p);
    assert!(!s.contains("100 intervals"));
    assert!(crate::figures::notes_for(d.id, &p).iter().any(|note| note.contains("100 intervals")));
    assert_eq!(&texts(&s)[..5], ["1", "2", "3", "4", "5"]);
    assert_eq!(s.matches("stroke-width=\"1.8\"").count(), 5);
}

#[test]
fn crowded_whole_numbers_use_familiar_strides_and_label_aligned_ends() {
    for (max, step, size) in [
        (100, 1, "Small"), (100, 1, "Medium"), (100, 1, "Large"),
        (75, 1, "Large"), (90, 1, "Large"), (200, 2, "Large"),
    ] {
        let s = svg(json!({
            "axis_auto":false, "axis_max":max, "axis_step":step,
            "label_size":size, "show_answers":false
        }));
        let labels: Vec<i32> = texts(&s).into_iter().map(|t| t.parse().unwrap()).collect();
        let stride = labels[1] - labels[0];
        assert!([1, 2, 5, 10, 20, 50, 100].contains(&(stride / step)), "{labels:?}");
        assert_eq!(labels, (0..=max).step_by(stride as usize).collect::<Vec<_>>());
        if max % stride == 0 {
            assert_eq!(labels.last(), Some(&max));
        }
    }
}

#[test]
fn frequency_counts_keep_later_stacks_and_use_full_weights() {
    let s = svg(json!({
        "data_form":"Values and counts", "data_table":"1, 1000000\n2, 40\n5, 1000000",
        "marks":"Mean and median"
    }));
    assert_eq!(dots(&s), 240);
    assert!(texts(&s).contains(&"mean = 3"));
    assert!(texts(&s).contains(&"median = 2"));
    assert_eq!(summary("1, 1000000\n2, 40", true), "1000040 values");
}

#[test]
fn mixed_number_labels_get_extra_separation() {
    let d = fig();
    let p = start_params(&d);
    let fs = 14.0 * crate::figures::svg::label_factor("Large");
    let ticks = Ticks::new(&p, 10.0, 17.0, 0.5, true, fs, false);
    let mut last = None;
    for (v, label) in ticks.positions.iter().filter_map(|(v, n)| n.as_ref().map(|n| (v, n))) {
        let x = (v - 10.0) * (ticks.width - 2.0 * ticks.margin) / 7.0;
        let (whole, _, fraction) = label.width(fs);
        let mixed = whole > 0.0 && fraction > 0.0;
        let half = label.half_width(fs);
        if let Some((end, was_mixed)) = last
            && (mixed || was_mixed)
        {
            assert!(x - half - end >= 8.0 - 1e-8);
        }
        last = Some((x + half, mixed));
    }
}

#[test]
fn crowded_number_line_stays_within_880_units() {
    let p = start_params(&fig());
    assert!(Ticks::new(&p, 0.0, 100.0, 1.0, false, 14.0, false).width <= 880.0);
}

#[test]
fn small_x_marks_clear_the_tick_tops() {
    let s = svg(json!({"data":"5", "mark_shape":"X's", "dot_size":"Small"}));
    let attr = |tag: &str, name: &str| -> f64 {
        tag.split(&format!("{name}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap()
    };
    let tick = s.split("<line ").find(|tag| tag.contains("stroke-width=\"1.8\"")).unwrap();
    let mark = s.split("<path ").find(|tag| tag.contains("data-mark=\"x\"")).unwrap();
    let lower_y: f64 = mark.split("L").nth(1).unwrap().split('M').next().unwrap()
        .split(',').nth(1).unwrap().parse().unwrap();
    assert!(lower_y + attr(mark, "stroke-width") / 2.0 < attr(tick, "y1"));
}

#[test]
fn a_new_option_preserves_a_saved_manual_tick_phase() {
    use crate::figures::{after_panel_change, share, spec};

    let d = fig();
    let saved = FigureSpec {
        id: d.id.into(),
        params: serde_json::from_value(json!({
            "data":"5, 6, 6, 7, 8, 9, 10", "axis_auto":false,
            "axis_min":0.5, "axis_max":10, "axis_step":1
        })).unwrap(),
    };
    let mut p = spec::params_from_spec(&d, &saved);
    let labels: Vec<_> = (1..=10).map(|i| i.to_string()).collect();
    assert_eq!(texts(&d.svg(&p)), labels);
    for (key, value) in [("mark_shape", "X's"), ("data_form", "Values and counts")] {
        let old = p.0[key].clone();
        p.set_s(key, value);
        after_panel_change(d.id, key, &old, &mut p);
        let drawn = d.svg(&p);
        assert_eq!(texts(&drawn), labels);
        assert_eq!(xs(&drawn), 7);
        assert_eq!(d.svg(&spec::params_from_spec(&d, &spec::spec_for(d.id, &p))), drawn);
        let link = share::encode(d.id, &p, &start_params(&d));
        let (_, changed) = share::decode(&link).unwrap();
        assert_eq!(d.svg(&share::link_params(&d, changed, share::version(&link))), drawn);
    }
}

#[test]
fn old_fraction_like_text_keeps_its_legacy_parsing_until_a_fraction_is_added() {
    let d = fig();
    let saved = FigureSpec { id: d.id.into(), params: [("data".into(), json!("1/2, 4"))].into() };
    let mut p = crate::figures::spec::params_from_spec(&d, &saved);
    let edit = |p: &mut Params, text| {
        let old = p.0["data"].clone();
        p.set_s("data", text);
        crate::figures::after_panel_change(d.id, "data", &old, p);
    };
    edit(&mut p, "1/2, 4, 5");
    assert_eq!(dots(&d.svg(&p)), 2);
    edit(&mut p, "1/2, 4, 5, 1 1/4");
    let drawn = d.svg(&p);
    assert_eq!(dots(&drawn), 4);
    assert!(drawn.contains("stroke-width=\"1.1\""));
}

#[test]
fn big_table_values_stay_in_distinct_stacks() {
    let s = svg(json!({
        "data_form":"Values and counts", "data_table":"10000000000, 1\n20000000000, 2"
    }));
    let centers: Vec<_> = s.split("<circle cx=\"").skip(1)
        .map(|t| t.split('"').next().unwrap()).collect();
    assert_eq!(centers.len(), 3);
    assert_ne!(centers[0], centers[1]);
    assert_eq!(centers[1], centers[2]);
}

#[test]
fn a_capped_stack_says_so_in_the_notes() {
    // checker 3, round 413: counts of 1,000,000 and 3,000,000 drew as two equal columns of 100 marks with nothing to say so
    let d = fig();
    let p = crate::figures::params_from_json(&d, &json!({"data_form": "Values and counts", "data_table": "1, 1000000\n2, 3000000"}));
    let notes = crate::figures::notes_for("dot-plot", &p);
    assert!(notes.iter().any(|n| n.contains("at most 100 marks") && n.contains("3000000")), "{notes:?}");
    let q = crate::figures::params_from_json(&d, &json!({"data_form": "Values and counts", "data_table": "1, 100\n2, 3"}));
    assert!(crate::figures::notes_for("dot-plot", &q).iter().all(|n| !n.contains("at most 100 marks")));
}
