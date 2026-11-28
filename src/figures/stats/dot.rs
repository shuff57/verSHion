//! Dot plot: one dot for each value, stacked above a number line. The dots are what a student is
//! asked to draw, so the student copy (Show answers off) keeps the number line and hides the dots
//! and the mean/median marks.
use std::collections::BTreeMap;

use super::dot_b10::{self as extra, ENDS, FORM, FORMS, NUMBERS, TABLE, TITLE_MODES};

use crate::figures::chart::{self, INK};
use crate::figures::data;
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub fn def() -> FigureDef {
    FigureDef {
        id: extra::ID,
        subject: Subject::Math,
        title: "Dot Plot",
        blurb: "A dot plot of a data list: one dot per value, stacked above a number line, with optional mean and median marks.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        (extra::ID, "axis_min" | "axis_max" | "axis_step") => !p.b("axis_auto"),
        (extra::ID, "data") => !extra::table(p),
        (extra::ID, TABLE) => extra::table(p),
        (extra::ID, "title") => extra::mode(p, "title_mode") == "Text",
        (extra::ID, "x_title") => extra::mode(p, "x_title_mode") == "Text",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "dot-plot" => vec![
            Example { name: "Shoe sizes", params: vec![] },
            Example {
                name: "Two peaks",
                params: vec![
                    ("data", s(&data::column_text(&data::generate(data::Dist::Bimodal, 30, 6, 20.0, 5.0, 0)))),
                    ("x_title", s("Minutes to get to school")),
                    ("marks", s("Mean and median")),
                ],
            },
            Example {
                name: "Grouped measurements",
                params: vec![
                    ("data", s(&data::column_text(&data::generate(data::Dist::Normal, 40, 2, 50.0, 6.0, 1)))),
                    ("group", Value::F(2.0)),
                    ("x_title", s("Height of seedling (mm)")),
                ],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice(FORM, "Data as", &FORMS, FORMS[0]),
        ParamDef::data("data", "Numbers", 1, "5, 6, 6, 7, 7, 7, 8, 8, 8, 8, 9, 9, 9, 10, 10, 11, 12")
            .hint("Fractions like 3/4 and mixed numbers like 1 1/2 work too. Clear the data for a blank number line"),
        ParamDef::data(TABLE, "Value and count", 1, "")
            .hint("Type one value and its count per line, like 3/4, 5, or paste two columns"),
        ParamDef::float("group", "Group size", 0.0, 1000.0, 0.5, 0.0).hint("Values are rounded to the nearest multiple of this, so close values share a stack. 0 keeps exact values"),
        ParamDef::bool("axis_auto", "Choose the number line automatically", true),
        ParamDef::float("axis_min", "Number line minimum", -10000.0, 10000.0, 1.0, 0.0),
        ParamDef::float("axis_max", "Number line maximum", -10000.0, 10000.0, 1.0, 12.0),
        ParamDef::float("axis_step", "Number line step", 0.001, 10000.0, 0.5, 1.0),
        ParamDef::choice("numbers", "Numbers", &NUMBERS, NUMBERS[0]),
        ParamDef::choice("left_end", "Left end", &ENDS, ENDS[0]),
        ParamDef::choice("right_end", "Right end", &ENDS, ENDS[0]),
        ParamDef::choice("mark_shape", "Marks", &["Dots", "X's"], "Dots"),
        ParamDef::choice("dot_size", "Mark size", &["Small", "Medium", "Large"], "Medium"),
        ParamDef::choice("marks", "Mean or median", &["None", "Mean", "Median", "Mean and median"], "None"),
        ParamDef::choice("title_mode", "Chart title", &TITLE_MODES, TITLE_MODES[0]),
        ParamDef::text("title", "Title text", ""),
        ParamDef::choice("x_title_mode", "Axis title", &TITLE_MODES, TITLE_MODES[0]),
        ParamDef::text("x_title", "Axis title text", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the data marks and mean or median, leaving the number line for students to plot on"),
    ]
}

/// The values stacked: each distinct (grouped) value with how many dots it gets, in order.
/// `group` > 0 rounds every value to the nearest multiple of it first.
fn stacks(values: impl IntoIterator<Item = (f64, u64)>, group: f64, table: bool) -> Vec<(f64, u128)> {
    let mut m: BTreeMap<i128, (f64, u128)> = BTreeMap::new();
    for (v, count) in values {
        let (key, shown) = if group > 0.0 && group.is_finite() {
            let k = (v / group).round();
            (k, k * group)
        } else {
            // exact values: values that agree to nine places are the same stack
            ((v * 1e9).round(), v)
        };
        // Keep the original list grouping, including its key range, for saved figures.
        let key = if table { key as i128 } else { key as i64 as i128 };
        m.entry(key).or_insert((shown, 0)).1 += count as u128;
    }
    m.into_values().collect()
}

/// The most marks any stack has before stacks are capped at `MAX_STACK` (the new layout only).
pub(super) fn tallest_stack(p: &Params) -> u128 {
    let is_table = extra::table(p);
    let table = if is_table { extra::read_table(p.s(TABLE)).rows } else { vec![] };
    let list = if is_table { vec![] } else { extra::list_values(p) };
    let counts = table.iter().map(|(v, n)| (v.v, *n)).chain(list.iter().map(|v| (v.v, 1)));
    stacks(counts, p.f("group"), is_table).into_iter().map(|s| s.1).max().unwrap_or(0)
}

pub fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    Some(if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 })
}

fn render(p: &Params) -> String {
    let custom = extra::custom_layout(p);
    let is_table = extra::table(p);
    let typed = if is_table { p.s(TABLE) } else { p.s("data") };
    let mut table = if is_table { extra::read_table(typed).rows } else { vec![] };
    let list = if is_table { vec![] } else { extra::list_values(p) };
    if table.is_empty() && list.is_empty() && !(custom && typed.trim().is_empty()) {
        return chart::message("Enter some data");
    }
    let values: Vec<f64> = list.iter().map(|v| v.v).collect();
    let counts = table.iter().map(|(v, n)| (v.v, *n))
        .chain(values.iter().map(|&v| (v, 1)));
    let stacks = stacks(counts, p.f("group"), is_table);
    let (auto, aligned) = if custom {
        let fit_values: Vec<_> = stacks.iter().map(|&(v, _)| v).collect();
        extra::auto_axis(&fit_values)
    } else {
        let (lo, hi) = (stacks.first().unwrap().0, stacks.last().unwrap().0);
        (chart::axis_range(lo, hi, 8.0), false)
    };
    let manual = !p.b("axis_auto");
    let use_manual = manual && !extra::excessive_axis(p);
    let (a0, a1, step) = if use_manual { extra::manual_axis(p) } else { auto };
    if !(a0.is_finite() && a1.is_finite() && step.is_finite()) {
        return chart::message("Check the number line settings");
    }
    let fraction_data = list.iter().chain(table.iter().map(|(v, _)| v)).any(|v| v.den.is_some());
    let fractions = if use_manual {
        fraction_data || extra::typed_fractions(p)
    } else {
        aligned && fraction_data
    };

    let fills = Fills::new(p);
    let fs = 14.0 * svg::label_factor(p.s("label_size"));
    let (title, x_title) = (p.s("title").trim(), p.s("x_title").trim());
    let (title_mode, x_title_mode) = (extra::mode(p, "title_mode"), extra::mode(p, "x_title_mode"));
    let has_title = |mode: &str, text: &str| mode == "Blank line" || (mode == "Text" && !text.is_empty());
    let keep_phase = use_manual && !p.b(extra::LAYOUT);
    let custom_ticks = custom.then(|| extra::Ticks::new(p, a0, a1, step, fractions, fs, keep_phase));
    let (w, margin) = custom_ticks.as_ref().map_or((640.0, 36.0), |ticks| (ticks.width, ticks.margin));
    let title_lines = |mode: &str, text: &str, size| {
        if custom && mode == "Text" { extra::title_lines(text, size, w - 24.0) } else { vec![text.to_owned()] }
    };
    let title_lines_chart = title_lines(title_mode, title, fs * 1.3);
    let title_lines_axis = title_lines(x_title_mode, x_title, fs * 1.1);
    let title_extra = |lines: &[String]| lines.len().saturating_sub(1) as f64 * fs * 1.6;
    let top = if has_title(title_mode, title) { 36.0 + fs * 1.4 + title_extra(&title_lines_chart) } else { 26.0 };
    let bottom = 24.0 + fs * 1.9
        + if has_title(x_title_mode, x_title) { fs * 1.7 + title_extra(&title_lines_axis) } else { 0.0 }
        + if fractions { fs * 0.4 } else { 0.0 };

    let marks = p.s("marks");
    let mut lines: Vec<(&str, f64)> = Vec::new();
    if marks != "None" {
        let want_mean = matches!(marks, "Mean" | "Mean and median");
        let want_median = matches!(marks, "Median" | "Mean and median");
        let (mean, median) = if is_table {
            extra::table_statistics(&mut table)
        } else {
            (want_mean.then(|| mean(&values)).flatten(), want_median.then(|| median(&values)).flatten())
        };
        if want_mean {
            lines.extend(mean.map(|m| ("mean", m)));
        }
        if want_median {
            lines.extend(median.map(|m| ("median", m)));
        }
    }
    lines.retain(|(_, v)| *v >= a0 && *v <= a1);
    lines.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let summary_space = if custom && !lines.is_empty() { fs * (lines.len() as f64 * 1.3 + 0.4) } else { 0.0 };

    let visible: Vec<_> = stacks.iter().copied()
        .filter(|(v, count)| *count > 0 && (!custom || (*v >= a0 - step * 1e-9 && *v <= a1 + step * 1e-9)))
        .map(|(v, count)| (v, if custom { count.min(extra::MAX_STACK) } else { count }))
        .collect();
    let base: f64 = match p.s("dot_size") {
        "Small" => 4.5,
        "Large" => 9.0,
        _ => 6.5,
    };
    let tallest = visible.iter().map(|s| s.1).max().unwrap_or(1) as f64;
    let mut plot_h: f64 = 190.0;
    let radius = if custom {
        let scale = (w - 2.0 * margin) / (a1 - a0);
        let min_gap = visible.windows(2).map(|pair| (pair[1].0 - pair[0].0) * scale).fold(f64::MAX, f64::min);
        let height_limit = (300.0 / (tallest * 2.2)).max(1.5);
        let radius = base.min(height_limit).min(min_gap * 0.45).max(0.3);
        plot_h = plot_h.max(summary_space + 10.0 + tallest * radius * 2.2);
        radius
    } else {
        // The saved renderer calculates gaps in pixel coordinates and retains its original scaling.
        let f = chart::Frame { w, h: 0.0, l: margin, r: margin, t: top, b: bottom, x0: a0, x1: a1, y0: 0.0, y1: 1.0 };
        let min_gap = visible.windows(2).map(|pair| f.px(pair[1].0) - f.px(pair[0].0)).fold(f64::MAX, f64::min);
        base.min(min_gap * 0.45).min((plot_h - 10.0) / (tallest * 2.2)).max(0.8)
    };
    let h = top + plot_h + bottom;
    let f = chart::Frame { w, h, l: margin, r: margin, t: top, b: bottom, x0: a0, x1: a1, y0: 0.0, y1: 1.0 };

    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    for (i, text) in title_lines_chart.iter().enumerate() {
        let y = 14.0 + fs * 1.4 + i as f64 * fs * 1.6;
        extra::draw_title(&mut d, "title", title_mode, text, w / 2.0, y, fs * 1.3);
    }
    extra::draw_line(&mut d, p, &f);
    if let Some(ticks) = custom_ticks {
        ticks.draw(&mut d, p, &f, fs);
    } else {
        for (v, label) in chart::ticks(a0, a1, step) {
            let x = f.px(v);
            d.line(x, f.bottom() - 6.0, x, f.bottom() + 8.0, INK, 1.8);
            chart::text(&mut d, x, f.bottom() + 12.0 + fs, fs, "middle", &label);
        }
    }
    for (i, text) in title_lines_axis.iter().enumerate() {
        let y = h - 10.0 - title_extra(&title_lines_axis) + i as f64 * fs * 1.6;
        extra::draw_title(&mut d, "x_title", x_title_mode, text, w / 2.0, y, fs * 1.1);
    }

    if p.b(SHOW_ANSWERS) {
        for (i, (name, v)) in lines.iter().enumerate() {
            let x = f.px(*v);
            // labels that would overprint each other go either side of their lines
            let near_next = lines.get(i + 1).is_some_and(|n| f.px(n.1) - x < 130.0);
            let near_prev = i > 0 && x - f.px(lines[i - 1].1) < 130.0;
            let label = format!("{name} = {}", chart::fmt((v * 100.0).round() / 100.0));
            let (anchor, label_x, label_y, line_y) = if custom {
                // A separate row for each statistic leaves room for tall stacks and coincident marks.
                let half_width = fs * 0.7 * label.chars().count() as f64 / 2.0;
                let left = 6.0 + half_width;
                ("middle", x.clamp(left, w - left), f.top() + fs * (1.2 + i as f64 * 1.3), f.top() + summary_space)
            } else {
                let (anchor, dx) = if near_next { ("end", -7.0) } else if near_prev { ("start", 7.0) } else { ("middle", 0.0) };
                let y = f.top() + fs * 1.2;
                (anchor, x + dx, y, y - fs * 0.35)
            };
            d.raw(&format!(r#"<line x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{line_y:.2}" stroke="{INK}" stroke-width="1.8" stroke-dasharray="6 4"/>"#, f.bottom()));
            chart::text(&mut d, label_x, label_y, fs, anchor, &label);
        }
        let r = radius;
        let pitch = r * 2.2;
        let dot_fill = fills.series(0);
        let draw_x = p.s("mark_shape") == "X's";
        let clearance = if draw_x { 8.0 } else { 4.0 };
        for (v, count) in &visible {
            let x = f.px(*v);
            for i in 0..*count {
                let y = f.bottom() - clearance - r - i as f64 * pitch;
                if y < f.top() {
                    break;
                }
                if draw_x {
                    extra::draw_x(&mut d, x, y, r, INK);
                } else {
                    let stroke = if custom { (r * 0.36).min(1.3) } else { 1.3 };
                    d.circle(x, y, r, &dot_fill, INK, stroke);
                }
            }
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { crate::figures::start_params(&def()) }

    #[test]
    fn stacks_count_each_value() {
        assert_eq!(stacks([3.0, 1.0, 3.0, 2.0, 3.0].map(|v| (v, 1)), 0.0, false), vec![(1.0, 1), (2.0, 1), (3.0, 3)]);
        assert!(stacks([], 0.0, false).is_empty());
        let near = stacks([(0.1 + 0.2, 1), (0.3, 1)], 0.0, false);
        assert_eq!(near.len(), 1, "float noise does not split a stack: {near:?}");
    }

    #[test]
    fn grouping_rounds_to_the_nearest_multiple() {
        let s = stacks([1.2, 1.4, 2.9, 3.1, 7.6].map(|v| (v, 1)), 2.0, false);
        assert_eq!(s, vec![(2.0, 3), (4.0, 1), (8.0, 1)], "1.2, 1.4 and 2.9 round to 2; 3.1 to 4; 7.6 to 8");
    }

    #[test]
    fn mean_and_median() {
        assert_eq!(mean(&[]), None);
        assert_eq!(mean(&[1.0, 2.0, 6.0]), Some(3.0));
        assert_eq!(median(&[5.0, 1.0, 3.0]), Some(3.0));
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn default_draws_one_dot_per_value() {
        let p = defaults();
        let n = data::numbers(p.s("data")).len();
        assert_eq!(render(&p).matches("<circle").count(), n);
    }

    #[test]
    fn student_copy_keeps_the_number_line_only() {
        let def = def();
        let mut p = defaults();
        p.set_s("marks", "Mean and median");
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        assert_eq!(student.matches("<circle").count(), 0);
        assert!(!student.contains("mean =") && !student.contains("median ="));
        assert!(student.contains(">12<") && student.contains(">5<"), "the number line is labelled");
        assert!(key.contains("mean = ") && key.contains("median = 8"));
        let size = |s: &str| s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string();
        assert_eq!(size(&student), size(&key), "same size, so the copies can be swapped");
    }

    #[test]
    fn print_style_is_grey() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let svg = render(&p);
        assert!(svg.contains("url(#vs-hatch0)") && !svg.contains("#4f7cd4"));
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24));
    }

    #[test]
    fn manual_number_line_is_used() {
        let mut p = defaults();
        p.set_b("axis_auto", false);
        p.set_f("axis_min", 0.0);
        p.set_f("axis_max", 20.0);
        p.set_f("axis_step", 5.0);
        let svg = render(&p);
        assert!(svg.contains(">20<") && svg.contains(">15<") && !svg.contains(">12<"));
        assert!(relevant("dot-plot", "axis_step", &p) && !relevant("dot-plot", "axis_step", &defaults()));
    }

    #[test]
    fn a_tall_stack_and_tight_values_still_fit() {
        let mut p = defaults();
        p.set_s("data", &vec!["5"; 400].join(", "));
        let svg = render(&p);
        assert!(svg.contains("<circle"), "a 400-high stack is drawn, smaller");
        p.set_s("data", "1, 1.01, 1.02, 1.03, 2");
        assert!(render(&p).contains("<circle"));
    }

    #[test]
    fn odd_input_never_panics_or_prints_nan() {
        let mut p = defaults();
        p.set_s("marks", "Mean and median");
        for text in ["", "abc", "7", "7, 7, 7", "-1e12, 1e12", "1e12, 1e12", "0.0000001, 0.0000002", "1, 2, 3, 1000000000"] {
            for group in [0.0, 0.001, 5.0, 1e9] {
                p.set_s("data", text);
                p.set_f("group", group);
                for answers in [true, false] {
                    p.set_b(SHOW_ANSWERS, answers);
                    let svg = render(&p);
                    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{text:?} {group}");
                    assert!(!svg.contains("NaN") && !svg.contains("inf"), "{text:?} {group}: {svg}");
                }
            }
        }
        p.set_s("data", "1, 2");
        p.set_b("axis_auto", false);
        for (lo, hi, st) in [(5.0, 5.0, 1.0), (10.0, 0.0, 2.0), (0.0, 1e12, 1e-9), (f64::NAN, 1.0, 1.0)] {
            p.set_f("axis_min", lo);
            p.set_f("axis_max", hi);
            p.set_f("axis_step", st);
            let svg = render(&p);
            assert!(svg.starts_with("<svg") && !svg.contains("NaN") && !svg.contains("inf"), "{lo} {hi} {st}");
        }
    }

    #[test]
    fn examples_render() {
        for e in examples("dot-plot").unwrap() {
            let mut p = defaults();
            for (k, v) in e.params {
                p.0.insert(k.to_string(), v);
            }
            assert!(render(&p).contains("<circle"), "{}", e.name);
        }
    }
}
