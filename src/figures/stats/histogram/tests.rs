//! The histogram's tests: old figures unchanged, the frequency table (unequal bins, pasted
//! worksheet rows, class limits), frequency density and areas, the polygon, the axes and titles.
use super::*;
use crate::figures::chart::{self, AxesOpts, Frame, INK};
use crate::figures::stats::{auto_start, auto_width, bins, measure_values};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{data, Params, Value};

/// The histogram's render as it was before the frequency table (main at 6d3a640), kept verbatim
/// so old saved figures and links can be checked against it byte for byte.
fn old_render(p: &Params) -> String {
    let values = data::numbers(p.s("data"));
    if values.is_empty() {
        return chart::message("Enter some data");
    }
    let left_closed = p.s("closed") != "In the bin to its left";
    let (width, start) = if p.b("auto_bins") {
        let w = auto_width(&values);
        (w, auto_start(&values, w, left_closed))
    } else {
        (p.f("bin_width"), p.f("bin_start"))
    };
    let Some(bs) = bins(&values, start, width, left_closed) else {
        return chart::message("Too many bars: make the bins wider");
    };
    if bs.is_empty() {
        return chart::message("No data at or above the first bin");
    }
    let measure = p.s("measure");
    let heights = measure_values(&bs, measure);
    let hmax = heights.iter().cloned().fold(0.0, f64::max);
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;

    // the vertical scale: nice ticks up to a top that holds the tallest bar
    let (ystep, ytop) = if p.b("auto_y") {
        let mut step = chart::nice_step(hmax.max(1e-9), 5.0);
        if measure == "Frequency" {
            step = step.max(1.0);
        }
        (step, (hmax / step - 1e-9).ceil().max(1.0) * step)
    } else {
        let top = p.f("y_max").max(1e-9);
        (chart::nice_step(top, 5.0), top)
    };
    let yticks = chart::ticks(0.0, ytop, ystep);
    let y_title = match p.s("y_title").trim() {
        "" => measure,
        t => t,
    };
    let x_title = p.s("x_title").trim();
    let title = p.s("title").trim();

    // every edge is a tick; a long run is thinned so the labels never touch
    let every = (bs.len() as f64 / 12.0).ceil().max(1.0) as usize;
    let mut xticks: Vec<chart::Tick> = bs.iter().enumerate().filter(|(i, _)| i % every == 0).map(|(_, b)| (b.lo, chart::fmt(b.lo))).collect();
    if (bs.len() % every == 0) || every == 1 {
        let last = bs.last().unwrap();
        xticks.push((last.hi, chart::fmt(last.hi)));
    }

    let label_chars = yticks.iter().map(|t| t.1.chars().count()).max().unwrap_or(1) as f64;
    let w = 640.0;
    let h = 420.0 + (k - 1.0) * 60.0;
    let frame = Frame {
        w,
        h,
        l: 34.0 + fs * 1.4 + label_chars * fs * 0.62,
        r: 28.0,
        t: (if title.is_empty() { 24.0 } else { 34.0 + fs * 1.4 }) + if p.b("values") && p.b(crate::figures::SHOW_ANSWERS) { fs * 1.1 } else { 0.0 },
        b: 22.0 + fs * 1.9 + if x_title.is_empty() { 0.0 } else { fs * 1.7 },
        x0: bs[0].lo,
        x1: bs.last().unwrap().hi,
        y0: 0.0,
        y1: ytop,
    };

    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.4, fs * 1.3, "middle", title);
    }
    chart::axes(&mut d, &frame, &AxesOpts { xticks: &xticks, yticks: &yticks, grid: p.b("grid"), x_title, y_title, fs });
    if p.b(crate::figures::SHOW_ANSWERS) {
        for (b, v) in bs.iter().zip(&heights) {
            let (x0, x1, y) = (frame.px(b.lo), frame.px(b.hi), frame.py(v.min(ytop)));
            d.rect(x0, y, x1 - x0, frame.bottom() - y, &fills.series(0), INK, 1.6);
        }
        // the axes are drawn again over the bars' feet so the baseline stays crisp
        d.line(frame.left(), frame.bottom(), frame.right(), frame.bottom(), INK, 2.0);
        if p.b("values") {
            for (b, v) in bs.iter().zip(&heights) {
                if *v > 0.0 {
                    let text = if measure == "Frequency" { b.n.to_string() } else { chart::fmt((v * 1000.0).round() / 1000.0) };
                    let (cx, cy) = ((frame.px(b.lo) + frame.px(b.hi)) / 2.0, frame.py(v.min(ytop)) - 6.0);
                    chart::text(&mut d, cx, cy, fs * 0.9, "middle", &text);
                }
            }
        }
    }
    d.finish()
}

fn def() -> crate::figures::FigureDef { crate::figures::stats::defs().into_iter().find(|d| d.id == ID).unwrap() }

fn defaults() -> Params { Params::defaults(&params()) }

fn table(start: &str, rows: &str) -> Params {
    let mut p = defaults();
    p.set_s("input", TABLE);
    p.set_s("table_start", start);
    p.set_s("table", rows);
    p
}

fn attr(tag: &str, name: &str) -> Option<f64> {
    let tag = format!(" {tag}");
    let key = format!(" {name}=\"");
    let i = tag.find(&key)? + key.len();
    tag[i..].split('"').next()?.parse().ok()
}

/// The bars as drawn: (x, y, width, height) of every rect with the bar outline.
fn bars_of(svg: &str) -> Vec<(f64, f64, f64, f64)> {
    svg.split("<rect ")
        .skip(1)
        .filter(|t| t.split("/>").next().unwrap().contains("stroke-width=\"1.6\""))
        .map(|t| (attr(t, "x").unwrap(), attr(t, "y").unwrap(), attr(t, "width").unwrap(), attr(t, "height").unwrap()))
        .collect()
}

/// Every text element: (x, y, font size, anchor, text).
fn texts_of(svg: &str) -> Vec<(f64, f64, f64, String, String)> {
    svg.split("<text ")
        .skip(1)
        .map(|t| {
            let head = t.split('>').next().unwrap();
            let body = t.split('>').nth(1).unwrap().split("</text").next().unwrap().to_string();
            let anchor = head.split("text-anchor=\"").nth(1).map(|s| s.split('"').next().unwrap().to_string()).unwrap_or_default();
            let (x, y) = match (attr(head, "x"), attr(head, "y")) {
                (Some(x), Some(y)) => (x, y),
                _ => {
                    let tr = head.split("translate(").nth(1).unwrap().split(')').next().unwrap();
                    let mut it = tr.split(' ').map(|v| v.parse::<f64>().unwrap());
                    (it.next().unwrap(), it.next().unwrap())
                }
            };
            (x, y, attr(head, "font-size").unwrap(), anchor, body)
        })
        .collect()
}

fn polylines_of(svg: &str, stroke: &str) -> Vec<Vec<(f64, f64)>> {
    svg.split("<polyline ")
        .skip(1)
        .filter(|t| t.contains(&format!("stroke=\"{stroke}\"")))
        .map(|t| {
            let pts = t.split("points=\"").nth(1).unwrap().split('"').next().unwrap();
            pts.split(' ')
                .map(|xy| {
                    let mut c = xy.split(',').map(|v| v.parse::<f64>().unwrap());
                    (c.next().unwrap(), c.next().unwrap())
                })
                .collect()
        })
        .collect()
}

fn axis_numbers(svg: &str) -> Vec<String> { texts_of(svg).into_iter().filter(|t| t.3 == "end").map(|t| t.4).collect() }

#[test]
fn old_figures_render_byte_for_byte() {
    // every old control, in combinations, with the new controls left at their defaults
    let datas = [
        "52, 58, 61, 63, 65, 66, 68, 70, 71, 72, 74, 75, 75, 77, 79, 82, 84, 85, 88, 93",
        "1, 2, 2, 3, 4, 4, 4, 9",
        "0.12, 0.5, 0.51, 0.77, 0.9",
        "-3, -1, 0, 0, 2, 5, 5, 6",
        "10, 20, 20, 30",
        "",
        "5",
        "1, 2, 3, 1000000000",
    ];
    let mut n = 0;
    for data_text in datas {
        for measure in ["Frequency", "Relative frequency", "Density"] {
            for (auto_bins, width, start) in [(true, 10.0, 50.0), (false, 2.5, 0.0), (false, 5.0, -5.0)] {
                for closed in ["In the bin to its right", "In the bin to its left"] {
                    for (auto_y, y_max) in [(true, 10.0), (false, 7.0), (false, 0.35)] {
                        for (values, answers, grid) in [(false, true, true), (true, true, false), (true, false, true)] {
                            for (style, size, titles) in [("Colour", "Medium", ("", "", "")), ("Print", "Large", ("Test scores", "Score", "Students")), ("Colour", "Small", ("A & B <c>", "", ""))] {
                                let mut p = defaults();
                                p.set_s("data", data_text);
                                p.set_s("measure", measure);
                                p.set_b("auto_bins", auto_bins);
                                p.set_f("bin_width", width);
                                p.set_f("bin_start", start);
                                p.set_s("closed", closed);
                                p.set_b("auto_y", auto_y);
                                p.set_f("y_max", y_max);
                                p.set_b("values", values);
                                p.set_b("show_answers", answers);
                                p.set_b("grid", grid);
                                p.set_s("style", style);
                                p.set_s("label_size", size);
                                p.set_s("title", titles.0);
                                p.set_s("x_title", titles.1);
                                p.set_s("y_title", titles.2);
                                assert_eq!(render(&p), old_render(&p), "{data_text:?} {measure} {auto_bins} {closed} {auto_y} {values} {style}");
                                n += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(n > 3800, "{n}");
}

#[test]
fn old_saved_specs_and_share_links_render_as_before() {
    let d = def();
    // a spec saved before the new controls: only the old keys
    let old_keys = [
        ("data", serde_json::json!("3, 4, 4, 5, 7, 8, 8, 8, 9, 12")),
        ("auto_bins", serde_json::json!(false)),
        ("bin_width", serde_json::json!(2.0)),
        ("bin_start", serde_json::json!(2.0)),
        ("closed", serde_json::json!("In the bin to its left")),
        ("measure", serde_json::json!("Relative frequency")),
        ("auto_y", serde_json::json!(true)),
        ("y_max", serde_json::json!(10.0)),
        ("values", serde_json::json!(true)),
        ("grid", serde_json::json!(false)),
        ("title", serde_json::json!("Goals")),
        ("x_title", serde_json::json!("Goals a game")),
        ("y_title", serde_json::json!("")),
        ("style", serde_json::json!("Print")),
        ("label_size", serde_json::json!("Large")),
        ("show_answers", serde_json::json!(true)),
    ];
    let spec = crate::model::FigureSpec { id: ID.into(), params: old_keys.iter().map(|(k, v)| (k.to_string(), v.clone())).collect() };
    let p = crate::figures::spec::params_from_spec(&d, &spec);
    let mut old = Params::defaults(&params());
    for (k, v) in &old_keys {
        match v {
            serde_json::Value::Bool(b) => old.set_b(k, *b),
            serde_json::Value::Number(x) => old.set_f(k, x.as_f64().unwrap()),
            serde_json::Value::String(s) => old.set_s(k, s),
            _ => {}
        }
    }
    assert_eq!(render(&p), old_render(&old), "a saved spec");
    // a share link made before: the payload holds only old keys that differ from the opening state
    let start = crate::figures::start_params(&d);
    let link = crate::figures::share::encode(ID, &old, &start);
    let (id, changed) = crate::figures::share::decode(&link).unwrap();
    assert_eq!(id, ID);
    assert!(changed.keys().all(|k| old_keys.iter().any(|(o, _)| o == k)), "{changed:?}");
    let lp = crate::figures::share::link_params(&d, changed, crate::figures::share::version(&link));
    assert_eq!(render(&lp), old_render(&old), "a share link");
    // and the opening state is the old one
    assert_eq!(render(&start), old_render(&start));
}

#[test]
fn the_opening_table_is_the_data_sets_bins() {
    let mut p = defaults();
    let from_data = render(&p);
    p.set_s("input", TABLE);
    assert_eq!(render(&p), from_data, "switching to the opening table keeps the same bars");
    assert_eq!(classes(&p).unwrap(), data_classes(&defaults()).unwrap());
    let cs = classes(&p).unwrap();
    assert_eq!(cs.iter().map(|c| c.f.unwrap()).collect::<Vec<_>>(), [2.0, 5.0, 8.0, 4.0, 1.0]);
    assert_eq!((cs[0].lo, cs[4].hi), (50.0, 100.0));
}

#[test]
fn switching_to_a_table_fills_it_from_the_data_set() {
    let mut p = defaults();
    p.set_s("data", "1, 2, 2, 3, 4, 4, 4, 9");
    let from_data = render(&p);
    p.set_s("input", TABLE);
    crate::figures::after_panel_change(ID, "input", &Value::S(DATA_SET.into()), &mut p);
    assert_eq!(p.s("table_start"), "0");
    assert_eq!(p.s("table"), "2, 1\n4, 3\n6, 3\n8, 0\n10, 1");
    assert_eq!(render(&p), from_data);
    // a table the teacher typed is kept
    p.set_s("table", "5, 3\n10, 4");
    p.set_s("input", DATA_SET);
    p.set_s("input", TABLE);
    crate::figures::after_panel_change(ID, "input", &Value::S(DATA_SET.into()), &mut p);
    assert_eq!(p.s("table"), "5, 3\n10, 4");
}

#[test]
fn unequal_bins_draw_frequency_density_and_areas_stand_for_frequency() {
    // 0-10: 4, 10-15: 8, 15-30: 6 -> densities 0.4, 1.6, 0.4
    let p = table("0", "10, 4\n15, 8\n30, 6");
    let cs = classes(&p).unwrap();
    assert_eq!(cs, vec![Class { lo: 0.0, hi: 10.0, f: Some(4.0) }, Class { lo: 10.0, hi: 15.0, f: Some(8.0) }, Class { lo: 15.0, hi: 30.0, f: Some(6.0) }]);
    let hs = heights(&cs, "Frequency");
    assert_eq!(hs.name, "Frequency density");
    assert_eq!(hs.h, [Some(0.4), Some(1.6), Some(0.4)]);
    let svg = render(&p);
    assert!(svg.contains(">Frequency density</text>"), "the axis says what the height is");
    let bars = bars_of(&svg);
    assert_eq!(bars.len(), 3);
    // the bars touch, and their widths follow the class widths 10 : 5 : 15
    assert!((bars[0].0 + bars[0].2 - bars[1].0).abs() < 0.02 && (bars[1].0 + bars[1].2 - bars[2].0).abs() < 0.02);
    assert!((bars[1].2 / bars[0].2 - 0.5).abs() < 0.01 && (bars[2].2 / bars[0].2 - 1.5).abs() < 0.01);
    // heights in proportion to density, areas in proportion to frequency
    assert!((bars[1].3 / bars[0].3 - 4.0).abs() < 0.01 && (bars[2].3 / bars[0].3 - 1.0).abs() < 0.01);
    let area = |b: &(f64, f64, f64, f64)| b.2 * b.3;
    assert!((area(&bars[1]) / area(&bars[0]) - 2.0).abs() < 0.01, "8 is twice 4");
    assert!((area(&bars[2]) / area(&bars[0]) - 1.5).abs() < 0.01, "6 is 1.5 times 4");
    // read a bar off the axis: the tallest bar's top sits at 1.6 on the drawn scale
    let ticks: Vec<(f64, f64)> = texts_of(&svg).iter().filter(|t| t.3 == "end").map(|t| (t.4.parse::<f64>().unwrap(), t.1)).collect();
    let (z, one) = (ticks.iter().find(|t| t.0 == 0.0).unwrap().1, ticks.iter().find(|t| t.0 == 1.0).unwrap().1);
    let per_unit = z - one; // px per unit of density (label baselines share the offset)
    assert!((bars[1].3 / per_unit - 1.6).abs() < 0.01, "{} px for 1.6", bars[1].3);
    // relative frequency with unequal bins is also per unit, and Density's areas add to 1
    assert_eq!(heights(&cs, "Relative frequency").name, "Relative frequency density");
    let dens = heights(&cs, "Density").h;
    assert!((cs.iter().zip(&dens).map(|(c, d)| (c.hi - c.lo) * d.unwrap()).sum::<f64>() - 1.0).abs() < 1e-12);
    let pct = heights(&cs, "Relative frequency (%)");
    assert!((cs.iter().zip(&pct.h).map(|(c, d)| (c.hi - c.lo) * d.unwrap()).sum::<f64>() - 100.0).abs() < 1e-9);
    // equal typed bins stay plain frequency
    assert_eq!(heights(&classes(&table("0", "10, 4\n20, 8")).unwrap(), "Frequency").name, "Frequency");
}

#[test]
fn values_above_density_bars_are_the_frequencies_the_areas_show() {
    let mut p = table("0", "10, 4\n15, 8\n30, 6");
    p.set_b("values", true);
    let svg = render(&p);
    let bars = bars_of(&svg);
    let bottom = bars[0].1 + bars[0].3;
    let vals: Vec<String> = texts_of(&svg).iter().filter(|t| t.3 == "middle" && t.0 > 40.0 && t.1 < bottom - 5.0).map(|t| t.4.clone()).collect();
    assert_eq!(vals, ["4", "8", "6"]);
    // area / (area of the 4 bar) * 4 gives each written frequency back
    let a0 = bars[0].2 * bars[0].3;
    for (b, v) in bars.iter().zip(&vals) {
        assert!((b.2 * b.3 / a0 * 4.0 - v.parse::<f64>().unwrap()).abs() < 0.02);
    }
    // percent: a share of the total, with its sign
    p.set_s("measure", "Relative frequency (%)");
    let svg = render(&p);
    let t: Vec<String> = texts_of(&svg).into_iter().map(|t| t.4).collect();
    for s in ["22.2%", "44.4%", "33.3%"] {
        assert!(t.contains(&s.to_string()), "{s} in {t:?}");
    }
    assert!(t.contains(&"Relative frequency density (%)".to_string()));
}

#[test]
fn relative_frequency_as_a_percent() {
    let mut p = defaults();
    p.set_s("measure", "Relative frequency (%)");
    p.set_b("values", true);
    let svg = render(&p);
    let t: Vec<String> = texts_of(&svg).into_iter().map(|t| t.4).collect();
    // 2, 5, 8, 4, 1 of 20
    for s in ["10%", "25%", "40%", "20%", "5%"] {
        assert!(t.contains(&s.to_string()), "{s} in {t:?}");
    }
    assert!(t.contains(&"Relative frequency (%)".to_string()));
    assert_eq!(axis_numbers(&svg), ["0", "10", "20", "30", "40"]);
    let bars = bars_of(&svg);
    assert!((bars[2].3 / bars[0].3 - 4.0).abs() < 0.01);
}

#[test]
fn pasted_worksheet_rows_and_class_limits() {
    // inequality rows from a worksheet, tab-separated, with a heading row
    let p = table("", "Height h (cm)\tFrequency\n140 < h \u{2264} 150\t7\n150 < h \u{2264} 165\t12\n165 < h \u{2264} 170\t4");
    let cs = classes(&p).unwrap();
    assert_eq!(cs, vec![Class { lo: 140.0, hi: 150.0, f: Some(7.0) }, Class { lo: 150.0, hi: 165.0, f: Some(12.0) }, Class { lo: 165.0, hi: 170.0, f: Some(4.0) }]);
    assert!(warnings(&p).is_empty(), "{:?}", warnings(&p));
    // <= typed as two characters, commas
    let q = table("", "0 <= t < 5, 3\n5 <= t < 10, 6");
    assert_eq!(classes(&q).unwrap()[1], Class { lo: 5.0, hi: 10.0, f: Some(6.0) });
    // integer class limits 10-19, 20-29, 30-39: drawn at the boundaries 9.5 .. 39.5 so the bars touch
    let r = table("", "10-19, 4\n20\u{2013}29, 6\n30 - 39, 5");
    let cs = classes(&r).unwrap();
    assert_eq!(cs.iter().map(|c| (c.lo, c.hi)).collect::<Vec<_>>(), [(9.5, 19.5), (19.5, 29.5), (29.5, 39.5)]);
    assert!(warnings(&r).iter().any(|n| n.contains("class boundaries (9.5, 19.5, 29.5, 39.5)")), "{:?}", warnings(&r));
    let svg = render(&r);
    let labels: Vec<String> = texts_of(&svg).into_iter().map(|t| t.4).collect();
    assert!(labels.contains(&"9.5".to_string()) && labels.contains(&"39.5".to_string()), "{labels:?}");
    // lower, upper, frequency as three numbers (the ogive's form), and negative values
    let s = table("", "-10, 0, 3\n0, 10, 5");
    assert_eq!(classes(&s).unwrap()[0], Class { lo: -10.0, hi: 0.0, f: Some(3.0) });
    let t = table("", "-10 < x \u{2264} -5, 2\n-5 < x \u{2264} 0, 1");
    assert_eq!(classes(&t).unwrap()[0], Class { lo: -10.0, hi: -5.0, f: Some(2.0) });
    // up to and frequency separated by spaces only
    let u = table("0", "10 4\n20 6");
    assert_eq!(classes(&u).unwrap()[1], Class { lo: 10.0, hi: 20.0, f: Some(6.0) });
}

#[test]
fn a_blank_start_makes_the_first_bin_as_wide_as_the_next() {
    let cs = classes(&table("", "20, 3\n30, 5\n50, 2")).unwrap();
    assert_eq!((cs[0].lo, cs[0].hi), (10.0, 20.0));
    assert!(render(&table("", "20, 3")).contains("Type where the first bin starts"));
}

#[test]
fn an_empty_frequency_leaves_its_bar_out() {
    let mut p = table("0", "10, 4\n20,\n30, 6\n40, 2");
    let cs = classes(&p).unwrap();
    assert_eq!(cs[1], Class { lo: 10.0, hi: 20.0, f: None });
    assert_eq!(bars_of(&render(&p)).len(), 3, "a bar for students to draw");
    // the polygon breaks there rather than guessing the missing height
    p.set_s("polygon", "Over the bars");
    assert_eq!(polylines_of(&render(&p), INK).len(), 2);
    let runs = polygon_runs(&cs, &heights(&cs, "Frequency").h, true);
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0], vec![(-5.0, 0.0), (5.0, 4.0)]);
    assert_eq!(runs[1], vec![(25.0, 6.0), (35.0, 2.0), (45.0, 0.0)]);
}

#[test]
fn bad_tables_say_what_is_wrong() {
    assert!(render(&table("0", "")).contains("Type a frequency table"));
    assert!(render(&table("0", "20, 3\n10, 4")).contains("each bin must end above where it starts"));
    assert!(render(&table("", "0 < x \u{2264} 10, 3\n5 < x \u{2264} 15, 4")).contains("overlap"));
    let p = table("0", "10, 3\nten, 4\n20, -2\n30, many");
    let w = warnings(&p);
    assert_eq!(w.len(), 3, "{w:?}");
    assert!(w[0].contains("Line 2") && w[1].contains("Line 3") && w[2].contains("Line 4"), "{w:?}");
    assert_eq!(bars_of(&render(&p)).len(), 1);
    assert!(warnings(&table("x", "10, 3\n20, 4")).iter().any(|n| n.contains("not a number")));
    assert!(warnings(&defaults()).is_empty(), "no notes for a data set");
    // only zeros or nothing typed: the axes still have a usable scale
    for rows in ["10, 0\n20, 0", "10,\n20,"] {
        for m in MEASURES {
            let mut q = table("0", rows);
            q.set_s("measure", m);
            let nums: Vec<f64> = axis_numbers(&render(&q)).iter().map(|t| t.parse::<f64>().unwrap()).collect();
            assert!(nums.len() >= 3 && nums.iter().cloned().fold(0.0, f64::max) >= 0.5, "{rows:?} {m}: {nums:?}");
        }
    }
    for odd in ["1e400, 3", "\u{2264}\u{2264}", "-, -", "1\n1\n1", "5 to 3, 2", "0.1, 1\n0.1000000001, 2"] {
        let svg = render(&table("", odd));
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{odd:?}");
    }
}

#[test]
fn every_fill_changes_the_bars_and_print_stays_grey() {
    let base = render(&defaults());
    let mut seen = std::collections::HashSet::new();
    for f in FILLS {
        let mut p = defaults();
        p.set_s("fill", f);
        let svg = render(&p);
        if f == "Blue" {
            assert_eq!(svg, base);
        } else {
            assert_ne!(svg, base, "{f}");
        }
        assert!(seen.insert(svg.clone()), "{f} differs from the others");
        if f.contains("atching") || f == "Dots" {
            assert!(svg.contains("<pattern id=\"hg-"), "{f} defines its pattern");
        }
        // Print: no colour anywhere but the ink
        p.set_s("style", "Print");
        let print = render(&p);
        for hex in ["#4f7cd4", "#c9494f", "#4aa36b", "#e0803c", "#8a62c4", "#9aa0a6"] {
            assert!(!print.contains(hex), "{f} in Print has {hex}");
        }
        assert!(crate::figures::to_png(&print, 1.0).is_ok());
    }
    let mut w = defaults();
    w.set_s("fill", "White");
    let svg = render(&w);
    assert!(bars_of(&svg).len() == 5 && svg.contains("fill=\"#ffffff\" stroke=\"#111827\" stroke-width=\"1.6\""));
}

#[test]
fn the_polygon_joins_the_bar_tops_at_the_bin_middles() {
    let mut p = table("0", "10, 4\n15, 8\n30, 6");
    p.set_s("polygon", "Over the bars");
    let svg = render(&p);
    let bars = bars_of(&svg);
    let line = &polylines_of(&svg, INK)[0];
    assert_eq!(line.len(), 5, "two feet and three vertices");
    for (b, v) in bars.iter().zip(&line[1..4]) {
        assert!((v.0 - (b.0 + b.2 / 2.0)).abs() < 0.02 && (v.1 - b.1).abs() < 0.02, "vertex {v:?} on bar {b:?}");
    }
    // closed at the middle of an empty bin of the same width each side: -5 and 37.5, on the axis
    let base = bars[0].1 + bars[0].3;
    let px_per = bars[0].2 / 10.0;
    assert!((line[0].1 - base).abs() < 0.02 && (line[4].1 - base).abs() < 0.02);
    assert!(((bars[0].0 - line[0].0) / px_per - 5.0).abs() < 0.01);
    assert!(((line[4].0 - (bars[2].0 + bars[2].2)) / px_per - 7.5).abs() < 0.01);
    // open: the vertices only
    p.set_b("poly_closed", false);
    assert_eq!(polylines_of(&render(&p), INK)[0].len(), 3);
    // alone: no bars
    p.set_s("polygon", "Alone");
    let alone = render(&p);
    assert!(bars_of(&alone).is_empty());
    assert_eq!(polylines_of(&alone, INK)[0].len(), 3);
    assert_eq!(alone.matches("<circle").count(), 3, "a dot at each vertex");
    // the student copy hides the polygon with the bars
    p.set_b("show_answers", false);
    let student = render(&p);
    assert!(polylines_of(&student, INK).is_empty() && !student.contains("<circle"));
}

#[test]
fn numbers_beside_the_polygon_stay_clear_of_its_lines() {
    let tables = [("0", "10, 4\n15, 8\n30, 6"), ("0", "2, 1\n4, 9\n6, 2\n8, 12\n10, 3"), ("50", "60, 2\n70, 5\n80, 8\n90, 4\n100, 1"), ("0", "1, 3\n2, 20\n3, 1\n4, 18\n5, 2\n6, 19")];
    for (start, rows) in tables {
        for poly in ["Over the bars", "Alone"] {
            for size in ["Medium", "Large"] {
                let mut p = table(start, rows);
                p.set_s("polygon", poly);
                p.set_b("values", true);
                p.set_s("label_size", size);
                let svg = render(&p);
                let lines = polylines_of(&svg, INK);
                let vals: Vec<String> = svg.split("<text ").skip(1).filter(|t| t.contains("paint-order")).map(|t| t.to_string()).collect();
                assert!(!vals.is_empty());
                let mut rects: Vec<(f64, f64, f64, f64)> = Vec::new();
                for t in &vals {
                    let head = t.split('>').next().unwrap();
                    let body = t.split('>').nth(1).unwrap().split("</text").next().unwrap();
                    let (x, y, sz) = (attr(head, "x").unwrap(), attr(head, "y").unwrap(), attr(head, "font-size").unwrap());
                    assert!(sz >= 8.0);
                    let tw = text_w(body, sz);
                    let left = if head.contains("\"end\"") { x - tw } else if head.contains("\"start\"") { x } else { x - tw / 2.0 };
                    let r = (left, y - sz * 0.78, left + tw, y + sz * 0.22);
                    assert!(r.0 >= 0.0 && r.2 <= 640.0 && r.1 >= 0.0, "{body} inside the page");
                    for run in &lines {
                        for s in run.windows(2) {
                            assert!(!seg_hits(s[0], s[1], r, 0.0), "{rows:?} {poly} {size}: {body} crosses the polygon");
                        }
                    }
                    for q in &rects {
                        assert!(!overlaps(r, *q), "{body} overlaps another number");
                    }
                    rects.push(r);
                }
            }
        }
    }
}

#[test]
fn the_horizontal_axis_numbered_every_step() {
    let mut p = table("0", "10, 4\n15, 8\n30, 6");
    p.set_s("polygon", "Over the bars");
    p.set_s("x_numbers", "Every step");
    let middle = |p: &Params| texts_of(&render(p)).into_iter().filter(|t| t.3 == "middle").map(|t| t.4).collect::<Vec<_>>();
    let nums = middle(&p);
    // fits the bins and the polygon's feet (-5 to 37.5): -5 to 40 by 5
    for s in ["\u{2212}5", "0", "5", "10", "20", "35", "40"] {
        assert!(nums.contains(&s.to_string()), "{s} in {nums:?}");
    }
    // a typed range
    p.set_b("x_auto", false);
    p.set_f("x_from", 0.0);
    p.set_f("x_to", 30.0);
    p.set_f("x_step", 4.0);
    let nums = middle(&p);
    for s in ["0", "4", "8", "24", "28"] {
        assert!(nums.contains(&s.to_string()), "{s} in {nums:?}");
    }
    assert!(!nums.contains(&"15".to_string()) && !nums.contains(&"10".to_string()), "the bin edges give way to the steps");
    // a crowded range thins its numbers so they never touch
    p.set_f("x_step", 0.5);
    p.set_s("label_size", "Large");
    let t = texts_of(&render(&p));
    let xs: Vec<(f64, f64)> = t.iter().filter(|t| t.3 == "middle" && t.1 > 380.0 && t.1 < 430.0).map(|t| (t.0, text_w(&t.4, t.2))).collect();
    assert!(xs.len() > 3);
    for w in xs.windows(2) {
        assert!(w[1].0 - w[1].1 / 2.0 > w[0].0 + w[0].1 / 2.0, "{xs:?}");
    }
    // unequal bin edges close together keep apart too
    let q = table("0", "1, 3\n1.5, 4\n2, 5\n10, 2\n10.2, 1");
    let t = texts_of(&render(&q));
    let xs: Vec<(f64, f64)> = t.iter().filter(|t| t.3 == "middle" && t.0 > 40.0).map(|t| (t.0, text_w(&t.4, t.2))).collect();
    assert!(xs.len() >= 3);
    for w in xs.windows(2) {
        assert!(w[1].0 - w[1].1 / 2.0 > w[0].0 + w[0].1 / 2.0, "{xs:?}");
    }
}

#[test]
fn the_vertical_axis_count_by_and_numbered_lines() {
    let mut p = defaults();
    p.set_b("auto_y", false);
    p.set_f("y_max", 20.0);
    p.set_f("y_step", 4.0);
    assert_eq!(axis_numbers(&render(&p)), ["0", "4", "8", "12", "16", "20"]);
    p.set_s("y_numbers", "Every 2nd line");
    assert_eq!(axis_numbers(&render(&p)), ["0", "8", "16"]);
    p.set_s("y_numbers", "No numbers");
    assert!(axis_numbers(&render(&p)).is_empty());
    // the gridlines stay at every count-by line
    assert_eq!(render(&p).matches(&format!("stroke=\"{GRID}\"")).count(), 6);
    // a count-by of 0 is automatic (as before), a silly one falls back
    p.set_f("y_step", 0.0);
    p.set_s("y_numbers", "Every line");
    assert_eq!(axis_numbers(&render(&p)), ["0", "5", "10", "15", "20"]);
    p.set_f("y_step", 0.001);
    assert_eq!(axis_numbers(&render(&p)), ["0", "5", "10", "15", "20"]);
}

#[test]
fn gridline_weights_and_minor_lines() {
    let base = render(&defaults());
    let mut p = defaults();
    for (wt, col) in [("Standard", "#a9b2bd"), ("Bold", "#6f7d8c")] {
        p.set_s("grid_weight", wt);
        let svg = render(&p);
        assert!(svg.contains(&format!("stroke=\"{col}\"")) && !svg.contains(&format!("stroke=\"{GRID}\"")), "{wt}");
        p.set_s("style", "Print");
        assert!(!render(&p).contains(col), "Print is grey: {wt}");
        p.set_s("style", "Colour");
    }
    p.set_s("grid_weight", "Light");
    assert_eq!(render(&p), base);
    p.set_s("minor", "4 per block");
    let svg = render(&p);
    // the top is 8, counted by 2: 4 blocks of 3 minor lines
    assert_eq!(axis_numbers(&svg), ["0", "2", "4", "6", "8"]);
    assert_eq!(svg.matches("stroke=\"#eef1f5\"").count(), 12);
    p.set_b("grid", false);
    let off = render(&p);
    assert!(!off.contains("#eef1f5") && !off.contains(GRID));
}

#[test]
fn titles_as_text_a_blank_line_or_none() {
    let mut p = defaults();
    p.set_s("title", "Test scores");
    p.set_s("x_title", "Score");
    let txt = |p: &Params| texts_of(&render(p)).into_iter().map(|t| t.4).collect::<Vec<_>>();
    assert!(txt(&p).contains(&"Test scores".to_string()));
    let lines = |s: &str| s.matches("stroke-width=\"1.3\"").count();
    for (mode_key, text) in [("title_mode", "Test scores"), ("x_title_mode", "Score"), ("y_title_mode", "Frequency")] {
        let mut q = p.clone();
        q.set_s(mode_key, "Blank line");
        let svg = render(&q);
        assert!(!txt(&q).contains(&text.to_string()), "{mode_key}: the title is hidden from students");
        assert_eq!(lines(&svg), 1, "{mode_key}: a line to write on");
        q.set_s(mode_key, "None");
        assert!(!txt(&q).contains(&text.to_string()) && lines(&render(&q)) == 0, "{mode_key}");
    }
    // a blank line is on the student copy too, and inside the page
    let mut q = p.clone();
    q.set_s("y_title_mode", "Blank line");
    q.set_s("label_size", "Large");
    q.set_b("show_answers", false);
    let svg = render(&q);
    let l = svg.split("stroke-width=\"1.3\"").next().unwrap().rsplit("<line ").next().unwrap();
    let (x1, y1, y2) = (attr(l, "x1").unwrap(), attr(l, "y1").unwrap(), attr(l, "y2").unwrap());
    assert!(x1 > 0.0 && y1 > 0.0 && y2 < 420.0 + 0.4 * 60.0);
}

#[test]
fn text_is_readable_and_inside_the_page() {
    for size in ["Small", "Medium", "Large"] {
        for (input, measure) in [(DATA_SET, "Frequency"), (TABLE, "Relative frequency (%)"), (TABLE, "Frequency")] {
            let mut p = table("0", "10, 4\n15, 8\n30, 6\n32, 1");
            p.set_s("input", input);
            p.set_s("measure", measure);
            p.set_s("label_size", size);
            p.set_b("values", true);
            p.set_s("polygon", "Over the bars");
            p.set_s("title", "Heights of the sunflowers grown by Year 9");
            p.set_s("x_title", "Height of the sunflower (cm)");
            let svg = render(&p);
            let (w, h) = crate::figures::svg_size(&svg);
            for (x, y, sz, anchor, body) in texts_of(&svg) {
                assert!(sz >= 8.0, "{body} at {sz}px");
                let tw = text_w(&body, sz);
                if x < 40.0 && anchor == "middle" && svg.contains(&format!("rotate(-90)\" font-size=\"{sz}\" text-anchor=\"middle\" font-family=\"Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif\" fill=\"#111827\">{body}<")) {
                    // the y title is turned a quarter: its length runs up the page
                    assert!(y - tw / 2.0 >= 0.0 && y + tw / 2.0 <= h, "{body}");
                    continue;
                }
                let (l, r) = match anchor.as_str() {
                    "end" => (x - tw, x),
                    "start" => (x, x + tw),
                    _ => (x - tw / 2.0, x + tw / 2.0),
                };
                assert!(l >= 0.0 && r <= w && y <= h && y - sz >= 0.0, "{size} {input} {measure}: {body:?} at {x},{y}");
            }
        }
    }
}

#[test]
fn the_student_copy_hides_every_answer() {
    let d = def();
    let mut p = table("0", "10, 4\n15, 8\n30, 6");
    p.set_b("values", true);
    p.set_s("polygon", "Over the bars");
    let (student, key) = crate::figures::answer_pair(&d, &p).unwrap();
    assert!(bars_of(&student).is_empty() && !bars_of(&key).is_empty());
    let st: Vec<String> = texts_of(&student).into_iter().map(|t| t.4).collect();
    for v in ["4", "8", "6"] {
        assert!(!st.contains(&v.to_string()), "{v} on the student copy");
    }
    assert!(st.contains(&"Frequency density".to_string()), "the axes stay");
    // the alt text is the figure's fixed blurb: no frequency, no height
    let mut s = p.clone();
    s.set_b("show_answers", false);
    let full = d.svg(&s);
    let desc = full.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
    assert!(!desc.chars().any(|c| c.is_ascii_digit()), "{desc}");
}

#[test]
fn relevance_follows_the_choices_and_only_this_figure() {
    let mut p = defaults();
    assert!(relevant(ID, "data", &p) && !relevant(ID, "table", &p) && !relevant(ID, "poly_closed", &p) && !relevant(ID, "x_from", &p));
    p.set_s("input", TABLE);
    assert!(!relevant(ID, "data", &p) && !relevant(ID, "closed", &p) && !relevant(ID, "bin_width", &p) && relevant(ID, "table", &p) && relevant(ID, "table_start", &p));
    p.set_s("polygon", "Alone");
    assert!(relevant(ID, "poly_closed", &p) && !relevant(ID, "fill", &p));
    p.set_s("x_numbers", "Every step");
    p.set_b("x_auto", false);
    assert!(relevant(ID, "x_from", &p) && relevant(ID, "x_step", &p));
    p.set_s("title_mode", "Blank line");
    assert!(!relevant(ID, "title", &p));
    // another figure's keys with the same names are untouched (stats::relevant ANDs every module)
    for fig in ["ogive", "bar-chart", "frequency-table", "scatterplot", "pie-chart"] {
        for key in ["data", "table", "closed", "fill", "title", "x_from", "grid_weight", "minor", "values"] {
            assert!(relevant(fig, key, &p), "{fig} {key}");
        }
    }
    assert!(crate::figures::param_relevant(ID, "table", &p));
    assert!(!crate::figures::param_relevant(ID, "data", &p));
}

#[test]
fn every_new_control_changes_the_drawing() {
    let base = render(&defaults());
    let cases: Vec<(&str, Value)> = vec![
        ("input", Value::S(TABLE.into())),
        ("fill", Value::S("Hatching".into())),
        ("polygon", Value::S("Over the bars".into())),
        ("measure", Value::S("Relative frequency (%)".into())),
        ("measure", Value::S("Frequency density".into())),
        ("y_numbers", Value::S("Every 2nd line".into())),
        ("x_numbers", Value::S("Every step".into())),
        ("grid_weight", Value::S("Bold".into())),
        ("minor", Value::S("2 per block".into())),
        ("title_mode", Value::S("Blank line".into())),
        ("x_title_mode", Value::S("Blank line".into())),
        ("y_title_mode", Value::S("None".into())),
    ];
    for (k, v) in cases {
        let mut p = defaults();
        if k == "input" {
            p.set_s("table", "60, 2\n70, 5\n90, 8");
        }
        p.0.insert(k.into(), v);
        assert_ne!(render(&p), base, "{k}");
    }
    // the ones that act inside another choice
    let mut p = defaults();
    p.set_s("polygon", "Over the bars");
    let on = render(&p);
    p.set_b("poly_closed", false);
    assert_ne!(render(&p), on);
    let mut p = defaults();
    p.set_b("auto_y", false);
    let a = render(&p);
    p.set_f("y_step", 2.5);
    assert_ne!(render(&p), a);
    let mut p = defaults();
    p.set_s("x_numbers", "Every step");
    let a = render(&p);
    p.set_b("x_auto", false);
    p.set_f("x_from", 40.0);
    assert_ne!(render(&p), a);
    let mut p = defaults();
    p.set_s("input", TABLE);
    let a = render(&p);
    p.set_s("table_start", "45");
    assert_ne!(render(&p), a);
}

#[test]
fn the_figure_renders_to_png_in_every_new_state() {
    let mut p = table("0", "10, 4\n15, 8\n30, 6");
    p.set_s("polygon", "Over the bars");
    p.set_s("fill", "Cross-hatching");
    p.set_s("minor", "5 per block");
    p.set_s("grid_weight", "Standard");
    p.set_b("values", true);
    p.set_s("title_mode", "Blank line");
    let svg = render(&p);
    assert!(crate::figures::to_png(&svg, 1.0).is_ok());
}

#[test]
fn a_frequency_in_thousands_groups_or_with_an_exponent_reads_as_one_number() {
    let read = |t: &str| { let mut n = Vec::new(); let r = rows(t, &mut n); (r.iter().map(|r| (r.lo, r.hi, r.f)).collect::<Vec<_>>(), n) };
    // the checker found "10, 1,000" refused as overlapping bins and 2e3 read as 2
    assert_eq!(read("10, 1,000\n20, 1,250").0, vec![(None, 10.0, Some(1000.0)), (None, 20.0, Some(1250.0))]);
    assert_eq!(read("10, 2e3").0, vec![(None, 10.0, Some(2000.0))]);
    assert_eq!(read("10, 1.5E1").0, vec![(None, 10.0, Some(15.0))]);
    // too big to hold is said, not read as something else
    let (r, n) = read("10, 1e400");
    assert_eq!(r, vec![(None, 10.0, None)]);
    assert!(n.iter().any(|m| m.contains("not a number")), "{n:?}");
    // a class and a frequency separated by commas read as before
    assert_eq!(read("60,100,5").0, vec![(Some(60.0), 100.0, Some(5.0))]);
    assert_eq!(read("60,100, 5").0, vec![(Some(60.0), 100.0, Some(5.0))]);
    assert_eq!(read("60, 100, 5").0, vec![(Some(60.0), 100.0, Some(5.0))]);
    assert_eq!(read("140 < h \u{2264} 150\t1,000\n150 < h \u{2264} 165\t1,200").0, vec![(Some(140.0), 150.0, Some(1000.0)), (Some(150.0), 165.0, Some(1200.0))]);
    assert_eq!(read("140 < h \u{2264} 150\t7").0, vec![(Some(140.0), 150.0, Some(7.0))]);
}
