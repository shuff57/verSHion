//! Tests for the batch 4 controls: the cumulative table, crosses, the smooth curve, unjoined
//! points, not starting from 0, how each reading shows, blank-line titles, typed axes, numbered
//! lines, gridline weight and minor gridlines. The SVG is read back with this file's own parser
//! and every number is recomputed here by hand or by a second method.
use super::*;

type P = (f64, f64);

fn with(list: &[(&str, Value)]) -> Params {
    let mut p = Params::defaults(&params());
    for (k, v) in list {
        p.0.insert(k.to_string(), v.clone());
    }
    p
}
fn s(v: &str) -> Value { Value::S(v.to_string()) }

fn attr(el: &str, k: &str) -> Option<String> {
    let el = format!(" {el}");
    let at = el.find(&format!(" {k}=\""))? + k.len() + 3;
    el[at..].split('"').next().map(str::to_string)
}
fn num_attr(el: &str, k: &str) -> f64 { attr(el, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }

#[derive(Clone, Debug)]
struct Txt { class: String, x: f64, y: f64, body: String }
fn texts(svg: &str) -> Vec<Txt> {
    svg.split("<text ").skip(1).map(|el| {
        let head = el.split('>').next().unwrap();
        let body = el[head.len() + 1..].split("</text>").next().unwrap().replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
        Txt { class: attr(head, "class").unwrap_or_default(), x: num_attr(head, "x"), y: num_attr(head, "y"), body }
    }).collect()
}
/// `<line>` elements of a class: (x1, y1, x2, y2, stroke, width).
fn lines(svg: &str, class: &str) -> Vec<(f64, f64, f64, f64, String, String)> {
    svg.split("<line ").skip(1).map(|l| l.split('>').next().unwrap()).filter(|l| attr(l, "class").as_deref() == Some(class))
        .map(|l| (num_attr(l, "x1"), num_attr(l, "y1"), num_attr(l, "x2"), num_attr(l, "y2"), attr(l, "stroke").unwrap_or_default(), attr(l, "stroke-width").unwrap_or_default())).collect()
}
fn polylines(svg: &str, class: &str) -> Vec<Vec<P>> {
    svg.split("<polyline ").skip(1).map(|l| l.split('>').next().unwrap()).filter(|l| attr(l, "class").as_deref() == Some(class))
        .map(|l| attr(l, "points").unwrap().split_whitespace().map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect()).collect()
}
/// The smooth curve read back: its cubic pieces in pixels.
fn bezier_curve(svg: &str) -> Option<Vec<[P; 4]>> {
    let el = svg.split("<path ").skip(1).map(|l| l.split('>').next().unwrap()).find(|l| attr(l, "class").as_deref() == Some("curve"))?;
    let d = attr(el, "d")?;
    let nums: Vec<f64> = d.replace(['M', 'C', ','], " ").split_whitespace().map(|t| t.parse().unwrap()).collect();
    let mut at = (nums[0], nums[1]);
    let mut out = Vec::new();
    for c in nums[2..].chunks(6) {
        let piece = [at, (c[0], c[1]), (c[2], c[3]), (c[4], c[5])];
        at = piece[3];
        out.push(piece);
    }
    Some(out)
}
fn bez(b: &[P; 4], t: f64) -> P {
    let u = 1.0 - t;
    let f = |i: usize| -> f64 { let c = [b[0], b[1], b[2], b[3]]; let v = [c[0], c[1], c[2], c[3]].map(|q| if i == 0 { q.0 } else { q.1 }); u * u * u * v[0] + 3.0 * u * u * t * v[1] + 3.0 * u * t * t * v[2] + t * t * t * v[3] };
    (f(0), f(1))
}
/// The drawn smooth curve as dense pixel points.
fn dense(curve: &[[P; 4]]) -> Vec<P> {
    let mut v = vec![curve[0][0]];
    for b in curve {
        for j in 1..=400 {
            v.push(bez(b, j as f64 / 400.0));
        }
    }
    v
}

/// The axes read back from the drawing: x from the numbered horizontal ticks, y from the tick
/// marks paired with their numbers (every line numbered).
struct Axes { xa: P, xb: P, ya: P, yb: P }
fn axes(svg: &str) -> Axes {
    let xt: Vec<Txt> = texts(svg).into_iter().filter(|t| t.class == "xtick").collect();
    let yt: Vec<Txt> = texts(svg).into_iter().filter(|t| t.class == "ytick").collect();
    let num = |t: &Txt| t.body.replace('\u{2212}', "-").parse::<f64>().unwrap();
    let ticks: Vec<f64> = lines(svg, "tick").into_iter().filter(|l| l.1 == l.3).map(|l| l.1).collect();
    assert_eq!(ticks.len(), yt.len(), "every y tick numbered for this read-back");
    Axes {
        xa: (num(&xt[0]), xt[0].x),
        xb: (num(&xt[xt.len() - 1]), xt[xt.len() - 1].x),
        ya: (num(&yt[0]), ticks[0]),
        yb: (num(&yt[yt.len() - 1]), ticks[ticks.len() - 1]),
    }
}
impl Axes {
    fn px(&self, x: f64) -> f64 { self.xa.1 + (x - self.xa.0) / (self.xb.0 - self.xa.0) * (self.xb.1 - self.xa.1) }
    fn x_of(&self, px: f64) -> f64 { self.xa.0 + (px - self.xa.1) / (self.xb.1 - self.xa.1) * (self.xb.0 - self.xa.0) }
    fn py(&self, v: f64) -> f64 { self.ya.1 + (v - self.ya.0) / (self.yb.0 - self.ya.0) * (self.yb.1 - self.ya.1) }
}

fn seg_dist(q: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx * dx + dy * dy;
    let t = if l == 0.0 { 0.0 } else { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l).clamp(0.0, 1.0) };
    (a.0 + t * dx - q.0).hypot(a.1 + t * dy - q.1)
}

fn key_text(p: &Params) -> String { model(p).unwrap().questions.iter().filter_map(|q| q.1.clone()).collect::<Vec<_>>().join(" | ") }

/// The number after `label` in a key ("median about 72.0" -> 72.0).
fn after(text: &str, label: &str) -> f64 {
    let at = text.find(label).unwrap_or_else(|| panic!("{label:?} in {text}")) + label.len();
    text[at..].trim_start().split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '\u{2212}' || c == '-')).next().unwrap().replace('\u{2212}', "-").parse().unwrap()
}
/// "accept a to b" after `label`.
fn accept_after(text: &str, label: &str) -> (f64, f64) {
    let at = text.find(label).unwrap() + label.len();
    let rest = &text[at..];
    let a = rest.find("accept ").unwrap() + 7;
    let mut it = rest[a..].split(" to ");
    let lo: f64 = it.next().unwrap().trim_end_matches('%').replace('\u{2212}', "-").parse().unwrap();
    let hi: f64 = it.next().unwrap().split(|c: char| c == ')' || c == '%').next().unwrap().replace('\u{2212}', "-").parse().unwrap();
    (lo, hi)
}

// ───────────────────────── the cumulative table (cap-01b) ─────────────────────────

#[test]
fn a_cumulative_table_gives_the_same_classes_as_its_frequency_table() {
    let (cum, note) = cumtable::parse_cum_table(cumtable::DEFAULT_CUM).unwrap();
    assert_eq!(cum, parse_table(DEFAULT_TABLE).unwrap());
    assert!(note.is_none());
    // the frequencies are the differences of the running totals, by hand: 3, 7, 12, 15, 9, 4
    assert_eq!(cum.iter().map(|c| c.f).collect::<Vec<_>>(), vec![3, 7, 12, 15, 9, 4]);
    // the graph is the frequency table's graph: same points, same readings
    let a = model(&with(&[("input", s(CUM)), ("marks", s("Median and quartiles"))])).unwrap();
    let b = model(&with(&[("marks", s("Median and quartiles"))])).unwrap();
    assert_eq!(a.pts, b.pts);
    assert_eq!(key_text(&with(&[("input", s(CUM)), ("marks", s("Median and quartiles"))])), key_text(&with(&[("marks", s("Median and quartiles"))])));
    // a pasted worksheet: a header, inequalities, "up to" words
    let pasted = "Height h (cm), Cumulative frequency\n140 < h \u{2264} 150, 4\n150 < h \u{2264} 160, 16\n\u{2264} 170, 42\nup to 180, 66";
    let (cs, note) = cumtable::parse_cum_table(pasted).unwrap();
    assert_eq!(cs.iter().map(|c| (c.lo, c.hi, c.f)).collect::<Vec<_>>(), vec![(140.0, 150.0, 4), (150.0, 160.0, 16 - 4), (160.0, 170.0, 26), (170.0, 180.0, 24)]);
    // no 0 line: the first class is as wide as the next (140 = 150 - 10), and the note says so
    assert!(note.unwrap().contains("start at 140"));
    // a first line with 0 says where it starts, even for a narrower first class
    let (cs, _) = cumtable::parse_cum_table("145, 0\n150, 4\n160, 16").unwrap();
    assert_eq!((cs[0].lo, cs[0].hi), (145.0, 150.0));
    // the line order does not matter
    assert_eq!(cumtable::parse_cum_table("60, 10\n40, 0\n50, 3").unwrap().0, cumtable::parse_cum_table("40, 0\n50, 3\n60, 10").unwrap().0);
}

#[test]
fn a_cumulative_table_refusal_names_a_fix_that_works() {
    for (bad, fix_line, line_no) in [
        ("40, 0\n50, 3\n60, 2\n70, 9", r#""60, 5""#, 3),
        ("40, 0\n50, 3.5\n60, 9", r#""50, 4""#, 2),
        ("40, 0\n50, -1\n60, 9", r#""50, 0""#, 2),
    ] {
        let err = cumtable::parse_cum_table(bad).unwrap_err();
        assert!(err.contains(fix_line), "{bad:?}: {err}");
        // apply the fix: it now reads
        let mut ls: Vec<String> = bad.lines().map(str::to_string).collect();
        ls[line_no - 1] = fix_line.trim_matches('"').to_string();
        assert!(cumtable::parse_cum_table(&ls.join("\n")).is_ok(), "{bad:?}: the fix {fix_line} did not work");
    }
    assert!(cumtable::parse_cum_table("").unwrap_err().contains("50, 3"));
    assert!(cumtable::parse_cum_table("40, 0\n50, 0").unwrap_err().contains("Every cumulative frequency is 0"));
    assert!(cumtable::parse_cum_table("50, 3").unwrap_err().contains("\"40, 0\""));
    assert!(cumtable::parse_cum_table("40, 0\n50, 3\n50, 4").unwrap_err().contains("both end at 50"));
    // a refusal is drawn as a note on the canvas, not a crash
    let svg = render(&with(&[("input", s(CUM)), ("cum_table", s("40, 0\n50, 3\n60, 2"))]));
    assert!(svg.contains("refusal") && svg.contains("never fall"));
}

// ───────────────────────── the numbers by hand ─────────────────────────

#[test]
fn median_quartiles_iqr_and_a_percentile_by_hand() {
    // the test-score table: totals 0, 3, 10, 22, 37, 46, 50 at 40, 50, ..., 100; n = 50
    // median: n/2 = 25 lies in 70-80 (22 to 37): 70 + 3/15 * 10 = 72
    // Q1: n/4 = 12.5 lies in 60-70 (10 to 22): 60 + 2.5/12 * 10 = 62.083 -> 62.1
    // Q3: 3n/4 = 37.5 lies in 80-90 (37 to 46): 80 + 0.5/9 * 10 = 80.556 -> 80.6
    // IQR as a student gets it: 80.6 - 62.1 = 18.5
    for input in [GROUPED, CUM] {
        let key = key_text(&with(&[("input", s(input)), ("marks", s("Median and quartiles"))]));
        assert_eq!(after(&key, "median about"), 72.0);
        assert_eq!(after(&key, "Q1 about"), 62.1);
        assert_eq!(after(&key, "Q3 about"), 80.6);
        assert_eq!(after(&key, "IQR about"), 18.5);
        // P40: 0.4 * 50 = 20 lies in 60-70: 60 + 10/12 * 10 = 68.33 -> 68.3, read across then down
        let key = key_text(&with(&[("input", s(input)), ("read", s(READ_PCT)), ("pct", Value::F(40.0))]));
        assert!(key.contains("40% of 50 is 20"), "{key}");
        assert_eq!(after(&key, "40th percentile is about"), 68.3);
        // up from 75: 22 + (75 - 70)/10 * 15 = 29.5 values = 59%
        let key = key_text(&with(&[("input", s(input)), ("read", s(READ_VALUE)), ("value", Value::F(75.0))]));
        assert!(key.contains("about 29.5 of the 50 values, so about 59.0%"), "{key}");
    }
}

// ───────────────────────── the smooth curve (cap-02b) ─────────────────────────

const TABLES: [&str; 4] = [
    DEFAULT_TABLE,
    "0, 10, 4\n10, 20, 9\n20, 40, 14\n40, 60, 8\n60, 100, 5",
    "0, 5, 0\n5, 10, 0\n10, 15, 6\n15, 20, 0\n20, 25, 4\n25, 30, 0",
    "140, 150, 4\n150, 160, 12\n160, 170, 26\n170, 180, 24\n180, 190, 10\n190, 200, 4",
];

#[test]
fn the_smooth_curve_goes_through_every_point_never_falls_and_never_overshoots() {
    for t in TABLES {
        let cs = parse_table(t).unwrap();
        let pts = points(&cs);
        let sm = smooth::Smooth::new(&pts);
        for q in &pts {
            assert_eq!(sm.y(q.0), Some(q.1), "{t}: through {q:?}");
        }
        for w in pts.windows(2) {
            let mut prev = w[0].1;
            for j in 0..=1000 {
                let x = w[0].0 + (w[1].0 - w[0].0) * j as f64 / 1000.0;
                let y = sm.y(x).unwrap();
                assert!(y >= prev - 1e-9, "{t}: falls at {x}");
                assert!(y >= w[0].1 - 1e-9 && y <= w[1].1 + 1e-9, "{t}: leaves its two points at {x} ({y})");
                prev = y;
            }
        }
        // the inverse agrees with the curve
        let n = pts[pts.len() - 1].1;
        for k in 1..100 {
            let h = n * k as f64 / 100.0;
            let x = sm.x_at(h);
            assert!((sm.y(x).unwrap() - h).abs() < 1e-6 * n, "{t}: x_at({h})");
        }
    }
}

#[test]
fn smooth_readings_agree_with_the_curve_as_drawn() {
    for t in TABLES {
        for (read, extra) in [("None", None), (READ_PCT, Some(("pct", Value::F(30.0)))), (READ_VALUE, None)] {
            for y in [Y_FREQ, Y_PCT] {
                let cs = parse_table(t).unwrap();
                let (a, b) = (cs[0].lo, cs[cs.len() - 1].hi);
                let mut l = vec![("table", s(t)), ("join", s(JOIN_SMOOTH)), ("marks", s("Median and quartiles")), ("read", s(read)), ("y_axis", s(y)), ("value", Value::F(a + 0.37 * (b - a))), ("decimals", Value::I(2))];
                if let Some(e) = extra.clone() { l.push(e); }
                let p = with(&l);
                let svg = render(&p);
                let ax = axes(&svg);
                let curve = dense(&bezier_curve(&svg).expect("a smooth curve"));
                assert!(polylines(&svg, "curve").is_empty(), "no straight curve as well");
                let n: f64 = cs.iter().map(|c| c.f as f64).sum();
                let sc = if y == Y_PCT { 100.0 / n } else { 1.0 };
                // every point is on the drawn curve
                for q in points(&cs) {
                    let (qx, qy) = (ax.px(q.0), ax.py(q.1 * sc));
                    let d = curve.iter().map(|c| (c.0 - qx).hypot(c.1 - qy)).fold(f64::INFINITY, f64::min);
                    assert!(d < 0.05, "{t}: point {q:?} off the curve by {d}");
                }
                // the drawn curve, read across at a height: the leftmost pixel where it reaches it
                let read_across = |h: f64| -> f64 {
                    let py = ax.py(h * sc);
                    let i = curve.iter().position(|c| c.1 <= py + 1e-6).unwrap();
                    if i == 0 {
                        return ax.x_of(curve[0].0);
                    }
                    // between the two samples that straddle the height
                    let (c0, c1) = (curve[i - 1], curve[i]);
                    let f = if c1.1 == c0.1 { 0.0 } else { (py - c0.1) / (c1.1 - c0.1) };
                    ax.x_of(c0.0 + f * (c1.0 - c0.0))
                };
                let key = key_text(&p);
                let d = 2;
                let half = 0.5 * 10f64.powi(-d) + 0.02 * (b - a) / 520.0 * 1.06 * 2.0;
                for (label, pc) in [("median about", 50.0), ("Q1 about", 25.0), ("Q3 about", 75.0)] {
                    let drawn = read_across(pc / 100.0 * n);
                    let printed = after(&key, label);
                    assert!((printed - drawn).abs() <= half + 0.01, "{t} {y}: {label} printed {printed}, drawn {drawn}");
                    let (lo, hi) = accept_after(&key, label);
                    assert!(lo <= printed && printed <= hi, "{t}: {label} range");
                    assert!(key.contains("read off the smooth curve") && !key.contains("straight-line"), "{key}");
                }
                if read == READ_PCT {
                    let drawn = read_across(0.3 * n);
                    assert!((after(&key, "30th percentile is about") - drawn).abs() <= half + 0.01, "{t}: P30");
                }
                if read == READ_VALUE {
                    // up from the value to the drawn curve and across: the percent the key gives
                    let v = a + 0.37 * (b - a);
                    let i = curve.iter().position(|c| c.0 >= ax.px(v)).unwrap().max(1);
                    let (c0, c1) = (curve[i - 1], curve[i]);
                    let cy = c0.1 + (ax.px(v) - c0.0) / (c1.0 - c0.0) * (c1.1 - c0.1);
                    let h = (cy - ax.ya.1) / (ax.yb.1 - ax.ya.1) * (ax.yb.0 - ax.ya.0) + ax.ya.0;
                    let pct = if y == Y_PCT { h } else { 100.0 * h / n };
                    let printed = after(&key, "so about");
                    assert!((printed - pct).abs() < 0.006 + 0.01, "{t} {y}: up from {v}: printed {printed}%, drawn {pct}%");
                }
                // every guide's corner lies on the drawn curve
                for g in polylines(&svg, "guide") {
                    let c = g[1];
                    let dd = curve.windows(2).map(|w| seg_dist(c, w[0], w[1])).fold(f64::INFINITY, f64::min);
                    assert!(dd < 0.1, "{t}: a guide meets the smooth curve {dd} px away");
                }
            }
        }
    }
}

// ───────────────────────── points, joins, from 0 (cap-02a, 02c, 02d) ─────────────────────────

#[test]
fn crosses_dots_and_unjoined_points() {
    let base = render(&with(&[]));
    let dots = base.matches("<circle class=\"point\"").count();
    assert_eq!(dots, 7, "seven points at 40, 50, ..., 100");
    let crosses = render(&with(&[("points", s("Crosses"))]));
    assert_eq!(crosses.matches("<circle").count(), 0);
    assert_eq!(crosses.matches("<path class=\"point\"").count(), 7);
    // each cross is centred on its point: its two strokes cross at the dot's centre
    let cx: Vec<P> = base.split("<circle ").skip(1).map(|c| (num_attr(c, "cx"), num_attr(c, "cy"))).collect();
    for (el, c) in crosses.split("<path class=\"point\"").skip(1).zip(&cx) {
        let d = attr(el, "d").unwrap();
        let v: Vec<f64> = d.replace(['M', 'L', ','], " ").split_whitespace().map(|t| t.parse().unwrap()).collect();
        assert!(((v[0] + v[2]) / 2.0 - c.0).abs() < 0.02 && ((v[1] + v[3]) / 2.0 - c.1).abs() < 0.02);
    }
    let none = render(&with(&[("join", s("None")), ("read", s(READ_PCT))]));
    assert!(!none.contains("class=\"curve\""));
    assert_eq!(none.matches("<circle class=\"point\"").count(), 7);
    // the opening state still reads the 40th percentile off where the straight line would run
    assert!(none.contains("class=\"guide\""));
    // the student copy's drawing task follows the choice
    let q = |l: &[(&str, Value)]| model(&with(l)).unwrap().questions.iter().map(|q| q.0.clone()).collect::<Vec<_>>().join(" ");
    let no_curve = ("student_curve", Value::B(false));
    assert!(q(&[no_curve.clone()]).contains("start at 0 on the lower boundary of the first class, and join the points with straight lines."));
    assert!(q(&[no_curve.clone(), ("join", s(JOIN_SMOOTH))]).contains("join the points with a smooth curve"));
    assert!(q(&[no_curve.clone(), ("join", s("None"))]).contains("Plot the ogive's points"));
    assert!(!q(&[no_curve, ("from_zero", Value::B(false))]).contains("start at 0"));
}

#[test]
fn not_starting_from_zero_starts_at_the_first_upper_boundary() {
    let p = with(&[("from_zero", Value::B(false)), ("marks", s("Median and quartiles")), ("read", s("None"))]);
    let m = model(&p).unwrap();
    assert_eq!(m.curve_pts[0], (50.0, 3.0), "the first point is the first class's upper boundary and its total");
    assert_eq!(m.curve_pts.len(), 6);
    let svg = render(&p);
    let ax = axes(&svg);
    let c = &polylines(&svg, "curve")[0];
    assert_eq!(c.len(), 6);
    assert!((c[0].0 - ax.px(50.0)).abs() < 0.03 && (c[0].1 - ax.py(3.0)).abs() < 0.03);
    // the readings above the first point are as before (the curve is the same there)
    assert_eq!(key_text(&p), key_text(&with(&[("marks", s("Median and quartiles")), ("read", s("None"))])));
    // a percentile below the first point cannot be read: a note, and no guide or question for it
    let low = with(&[("from_zero", Value::B(false)), ("read", s(READ_PCT)), ("pct", Value::F(4.0))]);
    let m = model(&low).unwrap();
    assert!(m.readings.is_empty() && !m.questions.iter().any(|q| q.0.contains("percentile")));
    assert!(warnings(&low).iter().any(|w| w.contains("4th percentile cannot be read") && w.contains("Start from 0")));
    assert!(!render(&low).contains("class=\"guide\""));
    // a value below where the curve starts: a note that says why
    let v = with(&[("from_zero", Value::B(false)), ("read", s(READ_VALUE)), ("value", Value::F(45.0))]);
    assert!(warnings(&v).iter().any(|w| w.contains("below where the curve starts, at 50")));
    // a first class holding over a quarter of the values: Q1 cannot be read, the median can, no IQR
    let big = with(&[("table", s("0, 10, 20\n10, 20, 5\n20, 30, 15")), ("from_zero", Value::B(false)), ("marks", s("Median and quartiles"))]);
    let key = key_text(&big);
    assert!(key.contains("Q1 cannot be read") && !key.contains("IQR"), "{key}");
    // median: 20 of 40 is the first point itself, at 10
    assert_eq!(after(&key, "median about"), 10.0);
}

// ───────────────────────── how each reading shows (cap-04e) ─────────────────────────

#[test]
fn each_reading_shows_its_value_lines_only_or_a_blank() {
    let marks = ("marks", s("Median and quartiles"));
    let none = ("read", s("None"));
    let student = (SHOW_ANSWERS, Value::B(false));
    let feet = |svg: &str| texts(svg).into_iter().filter(|t| t.class == "foot" || t.class == "side").map(|t| t.body).collect::<Vec<_>>();
    // Value (as before): every guide labelled in the key, nothing in the student copy
    let key = render(&with(&[marks.clone(), none.clone()]));
    assert_eq!(polylines(&key, "guide").len(), 3);
    let stu = render(&with(&[marks.clone(), none.clone(), student.clone()]));
    assert!(polylines(&stu, "guide").is_empty());
    // the median as a blank: its lines, and "M =" with a blank line under the axis below the foot
    // of its guide, in both copies; the quartiles as before
    for (copy, want_guides) in [(true, 3), (false, 1)] {
        let svg = render(&with(&[marks.clone(), none.clone(), ("median_label", s(SHOW_BLANK)), (SHOW_ANSWERS, Value::B(copy))]));
        assert_eq!(polylines(&svg, "guide").len(), want_guides, "key={copy}");
        let f = feet(&svg);
        assert!(f.iter().any(|t| t == "M ="), "key={copy}: {f:?}");
        // the median's value is nowhere on the drawing (72.0 is only in the key's answer line)
        let drawn: String = texts(&svg).into_iter().filter(|t| t.class != "answer").map(|t| t.body).collect::<Vec<_>>().join(" ");
        assert!(!drawn.contains("72"), "key={copy}: {drawn}");
        // the guide meets the curve at the median: across at 25, down at 72
        let ax = axes(&svg);
        let g = polylines(&svg, "guide").into_iter().find(|g| (g[1].1 - ax.py(25.0)).abs() < 0.05).expect("the median's guide");
        assert!((g[1].0 - ax.px(72.0)).abs() < 0.05);
        // the blank is centred under the guide, below the axis numbers, with "M =" just before it
        let axis_y = lines(&svg, "axis").into_iter().find(|l| l.1 == l.3).unwrap().1;
        let blank = lines(&svg, "blank").into_iter().find(|l| l.1 == l.3 && l.1 > axis_y && ((l.0 + l.2) / 2.0 - g[1].0).abs() < 0.05).expect("a blank under the median's guide");
        let m = texts(&svg).into_iter().find(|t| t.body == "M =").unwrap();
        let xt = texts(&svg).into_iter().find(|t| t.class == "xtick").unwrap();
        assert!(m.x < blank.0 && blank.0 - m.x < 40.0 && blank.1 > xt.y + 8.0, "{m:?} {blank:?}");
    }
    // lines only: the guides in both copies, no label for them
    let svg = render(&with(&[marks.clone(), none.clone(), ("median_label", s(SHOW_LINES)), ("q1_label", s(SHOW_LINES)), ("q3_label", s(SHOW_LINES)), student.clone()]));
    assert_eq!(polylines(&svg, "guide").len(), 3);
    assert!(feet(&svg).is_empty());
    // a value read up from: the blank is the height (the answer), the value stays (the question)
    let svg = render(&with(&[("read", s(READ_VALUE)), ("value", Value::F(75.0)), ("read_label", s(SHOW_BLANK)), student.clone()]));
    let f = feet(&svg);
    assert!(f.contains(&"______".to_string()) || f.iter().any(|t| t.ends_with("______")), "{f:?}");
    assert!(!f.iter().any(|t| t.contains("29.5") || t.contains("59")), "{f:?}");
    // the student copy's description never gives a value
    let desc = svg.split("<desc>").nth(1).map(|d| d.split("</desc>").next().unwrap().to_string()).unwrap_or_default();
    assert!(!desc.contains("59") && !desc.contains("29.5"), "{desc}");
    // a percentile set to blank: "P40 =" and its blank in the student copy
    let svg = render(&with(&[("read", s(READ_PCT)), ("read_label", s(SHOW_BLANK)), student]));
    assert!(feet(&svg).iter().any(|t| t == "P40 ="), "{:?}", feet(&svg));
}

// ───────────────────────── titles (cap-06c) ─────────────────────────

#[test]
fn a_title_can_be_a_blank_line_or_left_out() {
    let base = with(&[("title", s("Heights")), ("x_title", s("Height (cm)")), ("y_title", s("Number of plants"))]);
    let has = |svg: &str, class: &str| texts(svg).iter().any(|t| t.class == class);
    let svg = render(&base);
    assert!(has(&svg, "title") && has(&svg, "xtitle") && svg.contains("Number of plants"));
    for copy in [true, false] {
        let mut p = base.clone();
        for k in ["title_mode", "x_title_mode", "y_title_mode"] {
            p.set_s(k, "Blank line");
        }
        p.set_b(SHOW_ANSWERS, copy);
        let svg = render(&p);
        assert!(!has(&svg, "title") && !has(&svg, "xtitle") && !svg.contains("Number of plants") && !svg.contains("Heights"), "the blanked titles are nowhere, the description included");
        let blanks = lines(&svg, "blank");
        // a horizontal line for the chart and horizontal-axis titles, a vertical one for the vertical
        assert!(blanks.iter().filter(|l| l.1 == l.3 && l.2 - l.0 >= 150.0).count() >= 2, "{blanks:?}");
        assert!(blanks.iter().any(|l| l.0 == l.2 && l.3 - l.1 >= 150.0));
        // the blanks lie on the page, clear of the tick numbers
        let w = num_attr(svg.split('>').next().unwrap(), "width");
        for l in &blanks {
            assert!(l.0 >= 0.0 && l.2 <= w);
        }
    }
    let mut p = base.clone();
    for k in ["title_mode", "x_title_mode", "y_title_mode"] {
        p.set_s(k, "None");
    }
    let svg = render(&p);
    assert!(!has(&svg, "title") && !has(&svg, "xtitle") && !svg.contains("class=\"ytitle\"") && lines(&svg, "blank").is_empty());
    // with no titles the plot moves up to where it was with an empty title
    assert_eq!(svg, render(&with(&[("title", s("")), ("x_title", s("")), ("y_title_mode", s("None"))])));
}

// ───────────────────────── typed axes and numbered lines (cap-07a, 07b, 07c) ─────────────────────────

#[test]
fn typed_axes_put_the_points_and_readings_where_the_numbers_say() {
    let p = with(&[("x_auto", Value::B(false)), ("x_min", Value::F(30.0)), ("x_max", Value::F(110.0)), ("x_step", Value::F(10.0)), ("y_auto", Value::B(false)), ("y_min", Value::F(0.0)), ("y_max", Value::F(60.0)), ("y_step", Value::F(5.0)), ("marks", s("Median and quartiles"))]);
    let svg = render(&p);
    let xt: Vec<String> = texts(&svg).into_iter().filter(|t| t.class == "xtick").map(|t| t.body).collect();
    assert_eq!(xt, (3..=11).map(|k| (k * 10).to_string()).collect::<Vec<_>>());
    let yt: Vec<String> = texts(&svg).into_iter().filter(|t| t.class == "ytick").map(|t| t.body).collect();
    assert_eq!(yt, (0..=12).map(|k| (k * 5).to_string()).collect::<Vec<_>>());
    let ax = axes(&svg);
    // the axis runs exactly From to To: the vertical axis is at 30, the frame ends at 110
    let axis_x = lines(&svg, "axis").into_iter().find(|l| l.0 == l.2).unwrap().0;
    assert!((ax.px(30.0) - axis_x).abs() < 0.02);
    // every point at its boundary and running total on the typed scale
    let c = &polylines(&svg, "curve")[0];
    for (q, want) in c.iter().zip([(40.0, 0.0), (50.0, 3.0), (60.0, 10.0), (70.0, 22.0), (80.0, 37.0), (90.0, 46.0), (100.0, 50.0)]) {
        assert!((q.0 - ax.px(want.0)).abs() < 0.03 && (q.1 - ax.py(want.1)).abs() < 0.03, "{q:?} vs {want:?}");
    }
    // the guides still meet the curve where the key says (median 72 at 25)
    let g = polylines(&svg, "guide").into_iter().find(|g| (g[1].1 - ax.py(25.0)).abs() < 0.05).unwrap();
    assert!((g[1].0 - ax.px(72.0)).abs() < 0.05);
    // the key's range is half the drawn fine grid: count-by 10 cut in 5 (13 px each) is 2 across
    // (1 either way), plus half a fine step up (5/5/2 = 0.5 values) over the class slope 15/10
    let key = key_text(&p);
    let (lo, hi) = accept_after(&key, "median about");
    assert!((lo - 70.6).abs() < 1e-9 && (hi - 73.4).abs() < 1e-9, "{lo} {hi}: 72 -/+ (1 + 0.5 * 10 / 15) = 70.67 to 73.33, out to the tenth");
    let fine_x = lines(&svg, "grid").into_iter().filter(|l| l.0 == l.2 && l.4 == FINE_GRID).count();
    assert_eq!(fine_x, 8 * 4, "four fine lines in each of the 8 blocks");
    // a typed range that cuts the curve off says so; one that cannot be drawn falls back with a note
    let cut = with(&[("x_auto", Value::B(false)), ("x_min", Value::F(50.0)), ("x_max", Value::F(90.0)), ("x_step", Value::F(10.0))]);
    assert!(warnings(&cut).iter().any(|w| w.contains("part of the ogive is cut off")));
    assert!(render(&cut).contains("clip-path"));
    let bad = with(&[("x_auto", Value::B(false)), ("x_min", Value::F(90.0)), ("x_max", Value::F(50.0))]);
    assert!(warnings(&bad).iter().any(|w| w.contains("To must be above From")));
    assert_eq!(render(&bad), render(&with(&[])));
    let many = with(&[("y_auto", Value::B(false)), ("y_max", Value::F(1000.0)), ("y_step", Value::F(1.0))]);
    assert!(warnings(&many).iter().any(|w| w.contains("count by 10 or more")), "{:?}", warnings(&many));
}

#[test]
fn which_lines_are_numbered() {
    let nums = |l: &[(&str, Value)], class: &str| texts(&render(&with(l))).into_iter().filter(|t| t.class == class).map(|t| t.body).collect::<Vec<_>>();
    assert_eq!(nums(&[], "xtick"), ["40", "50", "60", "70", "80", "90", "100"]);
    assert_eq!(nums(&[("x_numbers", s("Every 2nd line"))], "xtick"), ["40", "60", "80", "100"]);
    assert!(nums(&[("x_numbers", s("No numbers"))], "xtick").is_empty());
    assert_eq!(nums(&[], "ytick"), ["0", "10", "20", "30", "40", "50"]);
    assert_eq!(nums(&[("y_numbers", s("Every 5th line"))], "ytick"), ["0", "50"]);
    let typed = [("x_auto", Value::B(false)), ("x_min", Value::F(40.0)), ("x_max", Value::F(100.0)), ("x_step", Value::F(5.0)), ("x_numbers", s("Every 2nd line"))];
    assert_eq!(nums(&typed, "xtick"), ["40", "50", "60", "70", "80", "90", "100"]);
    // the ticks stay at every line
    let svg = render(&with(&typed));
    assert_eq!(lines(&svg, "tick").into_iter().filter(|l| l.0 == l.2).count(), 13);
}

// ───────────────────────── gridlines (cap-07e, 07f) ─────────────────────────

#[test]
fn gridline_weight_and_minor_gridlines() {
    let grid = |l: &[(&str, Value)]| lines(&render(&with(l)), "grid");
    let std = grid(&[]);
    let (gc, fc, gw, fw) = grid_look("Standard", false);
    assert_eq!((gc, fc, gw, fw), (chart::GRID, FINE_GRID, 1.0, 1.0), "Standard is the look it always had");
    for weight in ["Light", "Bold"] {
        let g = grid(&[("grid_weight", s(weight))]);
        assert_eq!(g.len(), std.len());
        assert!(g.iter().zip(&std).any(|(a, b)| a.4 != b.4), "{weight} changes the colour");
    }
    // Bold is darker than Standard, Light paler, in both looks
    let lum = |c: &str| { let v = u32::from_str_radix(&c[1..], 16).unwrap(); ((v >> 16) & 255) + ((v >> 8) & 255) + (v & 255) };
    for print in [false, true] {
        let (l, s_, b) = (grid_look("Light", print), grid_look("Standard", print), grid_look("Bold", print));
        assert!(lum(l.0) > lum(s_.0) && lum(s_.0) > lum(b.0));
        assert!(l.2 <= s_.2 && s_.2 <= b.2);
        if print {
            for c in [l.0, l.1, b.0, b.1] {
                assert!(c[1..3] == c[3..5] && c[3..5] == c[5..7], "Print greys only: {c}");
            }
        }
    }
    // minor gridlines: none, or each block cut in 2, 4 or 5, on both axes
    let fine = |g: &[(f64, f64, f64, f64, String, String)], vertical: bool| g.iter().filter(|l| l.4 == FINE_GRID && (l.0 == l.2) == vertical).count();
    assert_eq!(fine(&grid(&[("minor", s("None"))]), true), 0);
    assert_eq!(fine(&grid(&[("minor", s("None"))]), false), 0);
    // 6 classes, 5 blocks of 10 up the side (0 to 50)
    for (choice, n) in [("2 per block", 2), ("4 per block", 4), ("5 per block", 5)] {
        let g = grid(&[("minor", s(choice))]);
        assert_eq!(fine(&g, true), 6 * (n - 1), "{choice} across");
        assert_eq!(fine(&g, false), 5 * (n - 1), "{choice} up");
    }
    // the key's accepted range follows the grid the student has: wider with no minor lines
    let range = |m: &str| { let k = key_text(&with(&[("marks", s("Median")), ("minor", s(m))])); let (a, b) = accept_after(&k, "median about"); b - a };
    assert!(range("None") > range("Automatic") && range("Automatic") >= range("5 per block"));
    // no minor lines: half a class (5) across plus half a block up (5 values) over the class's
    // 15 values per 10: 72 -/+ 8.33 = 63.67 to 80.33, out to the tenth
    let k = key_text(&with(&[("marks", s("Median")), ("minor", s("None"))]));
    assert_eq!(accept_after(&k, "median about"), (63.6, 80.4), "{k}");
}

// ───────────────────────── every new control is live; old figures unchanged ─────────────────────────

#[test]
fn every_new_control_changes_the_drawing() {
    let base_l: Vec<(&str, Value)> = vec![("marks", s("Median and quartiles"))];
    let base = render(&with(&base_l));
    let cases: Vec<Vec<(&str, Value)>> = vec![
        vec![("input", s(CUM)), ("cum_table", s("40, 0\n50, 5\n60, 10\n70, 22\n80, 37\n90, 46\n100, 50"))],
        vec![("points", s("Crosses"))],
        vec![("join", s(JOIN_SMOOTH))],
        vec![("join", s("None"))],
        vec![("from_zero", Value::B(false))],
        vec![("median_label", s(SHOW_BLANK))],
        vec![("q1_label", s(SHOW_LINES))],
        vec![("q3_label", s(SHOW_BLANK))],
        vec![("read", s(READ_PCT)), ("read_label", s(SHOW_BLANK))],
        vec![("title_mode", s("Blank line"))],
        vec![("x_title_mode", s("Blank line"))],
        vec![("y_title_mode", s("Blank line"))],
        vec![("x_title_mode", s("None"))],
        vec![("y_title_mode", s("None"))],
        vec![("x_auto", Value::B(false))],
        vec![("y_auto", Value::B(false))],
        vec![("x_numbers", s("Every 2nd line"))],
        vec![("y_numbers", s("Every 2nd line"))],
        vec![("grid_weight", s("Light"))],
        vec![("grid_weight", s("Bold"))],
        vec![("minor", s("None"))],
        vec![("minor", s("2 per block"))],
    ];
    for c in cases {
        let mut l = base_l.clone();
        l.extend(c.iter().cloned());
        assert_ne!(render(&with(&l)), base, "{c:?} changes nothing");
    }
    // a read label set without a reading changes nothing (and its control is hidden then)
    let p = with(&[("read", s(READ_VALUE)), ("read_label", s(SHOW_BLANK))]);
    assert!(!relevant(ID, "read_label", &with(&[("read", s("None"))])));
    assert!(relevant(ID, "read_label", &p));
}

#[test]
fn the_hooks_only_touch_the_ogive() {
    let p = with(&[]);
    for key in ["table", "cum_table", "x_min", "y_step", "q1_label", "title", "minor"] {
        assert!(relevant("bar-chart", key, &p) || key == "never", "{key} on another figure");
        assert!(relevant("histogram", key, &p));
    }
    assert!(relevant(ID, "table", &p) && !relevant(ID, "cum_table", &p));
    let c = with(&[("input", s(CUM))]);
    assert!(!relevant(ID, "table", &c) && relevant(ID, "cum_table", &c));
    assert!(!relevant(ID, "x_min", &p) && relevant(ID, "x_min", &with(&[("x_auto", Value::B(false))])));
    assert!(!relevant(ID, "q1_label", &with(&[("marks", s("Median"))])) && relevant(ID, "median_label", &with(&[("marks", s("Median"))])));
}

/// FNV-1a, so the hashes below are the same in any build.
fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Share-link payloads made before batch 4 (only the old keys): a sweep of every old control.
pub(super) fn old_payloads() -> Vec<serde_json::Map<String, serde_json::Value>> {
    use serde_json::json;
    let mut v = Vec::new();
    let tables = ["", "0, 10, 4\n10, 20, 9\n20, 40, 14\n40, 60, 8\n60, 100, 5", "0, 5, 0\n5, 10, 0\n10, 15, 6\n15, 20, 0\n20, 25, 4\n25, 30, 0"];
    for (ti, t) in tables.iter().enumerate() {
        for y in [Y_FREQ, Y_REL, Y_PCT] {
            for (read, marks) in [("None", "None"), (READ_PCT, "Median"), (READ_VALUE, "Median and quartiles"), ("None", "Median and quartiles")] {
                for (ans, curve, style, size, grid) in [(true, true, "Colour", "Medium", true), (false, false, "Print", "Large", false), (false, true, "Colour", "Small", true)] {
                    let mut m = serde_json::Map::new();
                    if !t.is_empty() { m.insert("table".into(), json!(t)); }
                    m.insert("y_axis".into(), json!(y));
                    m.insert("read".into(), json!(read));
                    m.insert("marks".into(), json!(marks));
                    m.insert("show_answers".into(), json!(ans));
                    m.insert("student_curve".into(), json!(curve));
                    m.insert("style".into(), json!(style));
                    m.insert("label_size".into(), json!(size));
                    m.insert("grid".into(), json!(grid));
                    if ti == 1 { m.insert("value".into(), json!(30.0)); m.insert("pct".into(), json!(65.0)); m.insert("decimals".into(), json!(2)); }
                    if !ans { m.insert("title".into(), json!("Heights")); m.insert("x_title".into(), json!("")); m.insert("y_title".into(), json!("Count")); m.insert("show_table".into(), json!(false)); }
                    v.push(m);
                }
            }
        }
    }
    let mut raw = serde_json::Map::new();
    raw.insert("input".into(), json!(RAW));
    raw.insert("marks".into(), json!("Median and quartiles"));
    v.push(raw.clone());
    raw.insert("auto_classes".into(), json!(false));
    raw.insert("class_width".into(), json!(5.0));
    raw.insert("class_start".into(), json!(50.0));
    raw.insert("closed".into(), json!(RIGHT_CLOSED));
    v.push(raw);
    v
}

/// The FNV hashes of the old payloads' SVGs as main (6d3a640) drew them, before batch 4.
const OLD_HASHES: &[u64] = &[
    11062500887887041294, 8269907293943861662, 7290132104779138035, 16181202030363365180, 5685390722651907864, 1143123580790762256,
    7180184614031928474, 4867828856134837590, 14256995484280671958, 5863507671668446919, 10545325157429088297, 159075374095461801,
    1013104240094743066, 10283411250545932851, 17051477225949780521, 15640005496231147369, 15505443934395977943, 11972902538223916832,
    1568584622169895988, 8891000318404133065, 17624838444302008454, 15512240613091846071, 8342316101226302814, 11076261187554208361,
    3657249630289276997, 864417514985462136, 12165692063935895547, 8245843399680065951, 2757975374851891710, 18283075470317941626,
    4343805976533759298, 15600539978813411462, 9299107121465787124, 9551498040999579226, 3438719614110268039, 4750542642521857979,
    1198465321664397943, 14187666338330233751, 4763044749597764197, 14237933086066882980, 403574811073374214, 2833036189082959759,
    17325293409374980501, 2322029861489015992, 3887875930188562731, 15122723478183475439, 3107042840033331498, 6302713294007563563,
    7847842848054901707, 9428685740790675604, 4484063212271161951, 11629920471939873737, 15626058030616224661, 8844233314196342715,
    5343264245747402158, 12677436246318217017, 2197050653482344717, 17033210994087410174, 12199078823878614175, 6495714080730067995,
    16462201762138392607, 14973651732716721205, 15692367905498050134, 7521653266118047046, 18011966256747767158, 1705696814143071964,
    5464609360346199022, 10279591851153800628, 5199227737663140644, 9330776020999715292, 16647505758187679908, 14374810444648416478,
    2251925577371282841, 17608988761378216370, 6531356361495785103, 14225006855659695800, 8175809508572887804, 12498088980567617516,
    6383048064026339784, 17164359401453647821, 7684983449748820013, 6383048064026339784, 17164359401453647821, 7684983449748820013,
    8775960728561747953, 16157704452501004845, 2658593135741985951, 5493107366228119584, 17988008414054661485, 7589143921754369558,
    4622854897003061236, 12090356376576381032, 16349788433557585939, 4622854897003061236, 12090356376576381032, 16349788433557585939,
    8120375829170804841, 6743816136904753854, 7198467058864067155, 4880549220106083501, 2115655908709633088, 6097440867171104862,
    9548287626518996852, 8416967712108448925, 12406054046357438375, 9548287626518996852, 8416967712108448925, 12406054046357438375,
    7070390345299472663, 9876747175762122724,
];

#[test]
fn old_share_links_render_byte_identically() {
    let def = def();
    let got: Vec<u64> = old_payloads().into_iter().map(|m| fnv(&render(&crate::figures::share::link_params(&def, m, crate::figures::share::VERSION)))).collect();
    if OLD_HASHES.is_empty() {
        println!("OLD_HASHES {got:?}");
        panic!("fill in OLD_HASHES from main");
    }
    assert_eq!(got.len(), OLD_HASHES.len());
    for (i, (g, o)) in got.iter().zip(OLD_HASHES).enumerate() {
        assert_eq!(g, o, "old link {i} ({:?}) renders differently", old_payloads()[i]);
    }
    // a saved figure stores every old param; the new ones come in at their defaults
    let spec = crate::model::FigureSpec { id: ID.into(), params: old_payloads()[7].iter().map(|(k, v)| (k.clone(), v.clone())).collect() };
    let p = crate::figures::spec::params_from_spec(&def, &spec);
    assert_eq!(p.s("join"), JOIN_STRAIGHT);
    assert_eq!(p.s("median_label"), SHOW_VALUE);
    assert!(p.b("x_auto") && p.b("from_zero"));
}

// ───────────────────────── nothing collides with the new options ─────────────────────────

type Bx4 = (f64, f64, f64, f64);
fn ink_boxes(svg: &str) -> Vec<(String, Bx4)> {
    fn walk(g: &resvg::usvg::Group, out: &mut Vec<(String, Bx4)>) {
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
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).expect("the SVG parses");
    let mut v = Vec::new();
    walk(tree.root(), &mut v);
    v
}
fn box_seg(a: P, b: P, bx: Bx4) -> f64 {
    let inside = |q: P| q.0 >= bx.0 && q.0 <= bx.2 && q.1 >= bx.1 && q.1 <= bx.3;
    let mut best = f64::INFINITY;
    for j in 0..=40 {
        let t = j as f64 / 40.0;
        let q = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        if inside(q) {
            return 0.0;
        }
        let ex = (bx.0 - q.0).max(0.0).max(q.0 - bx.2);
        let ey = (bx.1 - q.1).max(0.0).max(q.1 - bx.3);
        best = best.min(ex.hypot(ey));
    }
    best
}

/// Every text on the page, apart from every other and clear of every drawn line and point.
fn audit(svg: &str, what: &str) {
    let w = num_attr(svg.split('>').next().unwrap(), "width");
    let h = num_attr(svg.split('>').next().unwrap(), "height");
    let boxes = ink_boxes(svg);
    for (t, b) in &boxes {
        assert!(b.0 >= -0.01 && b.1 >= -0.01 && b.2 <= w + 0.01 && b.3 <= h + 0.01, "{what}: {t:?} off the page");
    }
    for (i, (ta, a)) in boxes.iter().enumerate() {
        for (tb, b) in &boxes[i + 1..] {
            let apart = a.2 + 1.0 <= b.0 || b.2 + 1.0 <= a.0 || a.3 + 1.0 <= b.1 || b.3 + 1.0 <= a.1;
            assert!(apart, "{what}: {ta:?} touches {tb:?}");
        }
    }
    // every non-grid stroke: lines, polylines, the smooth curve, crosses
    let mut segs: Vec<(String, P, P)> = Vec::new();
    for class in ["axis", "tick", "blank", "leader"] {
        for l in lines(svg, class) {
            segs.push((class.into(), (l.0, l.1), (l.2, l.3)));
        }
    }
    for class in ["curve", "guide"] {
        for pl in polylines(svg, class) {
            for w2 in pl.windows(2) {
                segs.push((class.into(), w2[0], w2[1]));
            }
        }
    }
    if let Some(c) = bezier_curve(svg) {
        let d = dense(&c);
        for w2 in d.windows(8).step_by(7) {
            segs.push(("curve".into(), w2[0], w2[7]));
        }
    }
    for el in svg.split("<path class=\"point\"").skip(1) {
        let d = attr(el, "d").unwrap();
        let v: Vec<f64> = d.replace(['M', 'L', ','], " ").split_whitespace().map(|t| t.parse().unwrap()).collect();
        segs.push(("cross".into(), (v[0], v[1]), (v[2], v[3])));
        segs.push(("cross".into(), (v[4], v[5]), (v[6], v[7])));
    }
    let dots: Vec<(P, f64)> = svg.split("<circle ").skip(1).map(|c| ((num_attr(c, "cx"), num_attr(c, "cy")), num_attr(c, "r"))).collect();
    for (t, b) in &boxes {
        for (class, a, z) in &segs {
            // the answer blanks under the questions are lines beside their own "Answer:" word
            if class == "blank" && t == "Answer:" {
                continue;
            }
            let gap = box_seg(*a, *z, *b);
            assert!(gap >= 1.0, "{what}: a {class} line touches {t:?} ({gap:.2} px)");
        }
        for (c, r) in &dots {
            let ex = (b.0 - c.0).max(0.0).max(c.0 - b.2);
            let ey = (b.1 - c.1).max(0.0).max(c.1 - b.3);
            assert!(ex.hypot(ey) >= r + 1.0, "{what}: a point touches {t:?}");
        }
    }
}

#[test]
fn nothing_collides_with_the_new_options() {
    let tables = [DEFAULT_TABLE, "0, 10, 4\n10, 20, 9\n20, 40, 14\n40, 60, 8\n60, 100, 5", "0, 5, 0\n5, 10, 0\n10, 15, 6\n15, 20, 0\n20, 25, 4\n25, 30, 0"];
    let mut n = 0;
    for t in tables {
        let cs = parse_table(t).unwrap();
        let (a, b) = (cs[0].lo, cs[cs.len() - 1].hi);
        let typed = [("x_auto", Value::B(false)), ("x_min", Value::F(a)), ("x_max", Value::F(b + (b - a) / 6.0)), ("x_step", Value::F((b - a) / 6.0))];
        for join in [JOIN_STRAIGHT, JOIN_SMOOTH, "None"] {
            for (pts, from0) in [("Dots", true), ("Crosses", false)] {
                for size in ["Small", "Large"] {
                    for (ml, ql) in [(SHOW_VALUE, SHOW_VALUE), (SHOW_BLANK, SHOW_BLANK), (SHOW_BLANK, SHOW_LINES), (SHOW_LINES, SHOW_BLANK)] {
                        for key in [true, false] {
                            for axes_typed in [false, true] {
                                for titles in ["Text", "Blank line"] {
                                    let mut l: Vec<(&str, Value)> = vec![
                                        ("table", s(t)), ("join", s(join)), ("points", s(pts)), ("from_zero", Value::B(from0)), ("label_size", s(size)),
                                        ("marks", s("Median and quartiles")), ("median_label", s(ml)), ("q1_label", s(ql)), ("q3_label", s(ql)),
                                        ("read", s(READ_PCT)), ("pct", Value::F(90.0)), ("read_label", s(ml)),
                                        (SHOW_ANSWERS, Value::B(key)), ("title", s("Heights of plants")), ("title_mode", s(titles)), ("x_title_mode", s(titles)), ("y_title_mode", s(titles)),
                                    ];
                                    if axes_typed {
                                        l.extend(typed.iter().cloned());
                                    }
                                    let what = format!("{t:?} {join} {pts} {size} M={ml} Q={ql} key={key} typed={axes_typed} {titles}");
                                    audit(&render(&with(&l)), &what);
                                    n += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("{n} renders audited");
}
