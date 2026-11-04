//! Tests for the ogive. The SVG is read back with this file's own parser (no drawing code shared),
//! text extents come from resvg's real glyph boxes, and every number is recomputed here by a
//! second implementation.
use super::*;

type P = (f64, f64);

fn defaults() -> Params { Params::defaults(&params()) }
fn s(k: &str, v: &str) -> (String, Value) { (k.to_string(), Value::S(v.to_string())) }
fn b(k: &str, v: bool) -> (String, Value) { (k.to_string(), Value::B(v)) }
fn f(k: &str, v: f64) -> (String, Value) { (k.to_string(), Value::F(v)) }
fn with(list: &[(String, Value)]) -> Params {
    let mut p = defaults();
    for (k, v) in list {
        p.0.insert(k.clone(), v.clone());
    }
    p
}

// ───────────────────────── reading the SVG back ─────────────────────────

fn attr(el: &str, k: &str) -> Option<String> {
    let el = format!(" {el}");
    let at = el.find(&format!(" {k}=\""))? + k.len() + 3;
    el[at..].split('"').next().map(str::to_string)
}
fn num_attr(el: &str, k: &str) -> f64 { attr(el, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
fn unesc(t: &str) -> String { t.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&") }

#[derive(Clone, Debug)]
struct Txt { class: String, x: f64, y: f64, size: f64, anchor: String, body: String, ink: Bx }
#[derive(Clone, Debug)]
struct Ln { class: String, a: P, b: P, stroke: String }
#[derive(Clone, Debug)]
struct Pl { class: String, pts: Vec<P>, stroke: String, dash: Option<String> }

struct Svg { src: String, w: f64, h: f64, texts: Vec<Txt>, lines: Vec<Ln>, plines: Vec<Pl>, dots: Vec<(P, f64)>, plates: Vec<Bx> }

fn ink_boxes(svg: &str) -> Vec<(String, Bx)> {
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
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).expect("the SVG parses");
    let mut v = Vec::new();
    walk(tree.root(), &mut v);
    v
}

fn parse(svg: &str, ink: bool) -> Svg {
    let root = svg.split('>').next().unwrap();
    let (w, h) = (num_attr(root, "width"), num_attr(root, "height"));
    let mut texts = Vec::new();
    for el in svg.split("<text ").skip(1) {
        let head = el.split('>').next().unwrap();
        let body = unesc(el[head.len() + 1..].split("</text>").next().unwrap());
        let class = attr(head, "class").unwrap_or_default();
        let (x, y) = if class == "ytitle" { (f64::NAN, f64::NAN) } else { (num_attr(head, "x"), num_attr(head, "y")) };
        texts.push(Txt { class, x, y, size: num_attr(head, "font-size"), anchor: attr(head, "text-anchor").unwrap_or_default(), body, ink: (0.0, 0.0, 0.0, 0.0) });
    }
    if ink {
        let boxes = ink_boxes(svg);
        assert_eq!(boxes.len(), texts.len(), "one ink box per text element");
        for (t, (body, bx)) in texts.iter_mut().zip(boxes) {
            assert_eq!(t.body, body);
            t.ink = bx;
        }
    }
    let lines = svg.split("<line ").skip(1).map(|l| { let l = l.split('>').next().unwrap(); Ln { class: attr(l, "class").unwrap_or_default(), a: (num_attr(l, "x1"), num_attr(l, "y1")), b: (num_attr(l, "x2"), num_attr(l, "y2")), stroke: attr(l, "stroke").unwrap_or_default() } }).collect();
    let plines = svg.split("<polyline ").skip(1).map(|l| {
        let l = l.split('>').next().unwrap();
        let pts = attr(l, "points").unwrap().split_whitespace().map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect();
        Pl { class: attr(l, "class").unwrap_or_default(), pts, stroke: attr(l, "stroke").unwrap_or_default(), dash: attr(l, "stroke-dasharray") }
    }).collect();
    let dots = svg.split("<circle ").skip(1).map(|c| { let c = c.split('>').next().unwrap(); ((num_attr(c, "cx"), num_attr(c, "cy")), num_attr(c, "r")) }).collect();
    let plates = svg.split("<rect ").skip(1).filter_map(|r| {
        let r = r.split('>').next().unwrap();
        (attr(r, "class").as_deref() == Some("plate")).then(|| { let x = num_attr(r, "x"); (x, num_attr(r, "y"), x + num_attr(r, "width"), num_attr(r, "y") + num_attr(r, "height")) })
    }).collect();
    Svg { src: svg.to_string(), w, h, texts, lines, plines, dots, plates }
}

fn seg_dist(q: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx * dx + dy * dy;
    let t = if l == 0.0 { 0.0 } else { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l).clamp(0.0, 1.0) };
    (a.0 + t * dx - q.0).hypot(a.1 + t * dy - q.1)
}
/// Distance from a segment to a box: 0 when the segment crosses it (tested by sampling, independent of the
/// module's clip test), else the least of corner-to-segment and end-to-box distances. A far box is skipped early.
fn seg_box(a: P, b: P, bx: Bx) -> f64 {
    let pb = |q: P| { let ex = (bx.0 - q.0).max(0.0).max(q.0 - bx.2); let ey = (bx.1 - q.1).max(0.0).max(q.1 - bx.3); ex.hypot(ey) };
    let far = (bx.0 - a.0.max(b.0)).max(a.0.min(b.0) - bx.2).max(bx.1 - a.1.max(b.1)).max(a.1.min(b.1) - bx.3);
    if far > 50.0 {
        return far;
    }
    // crossing: an end inside, or the segment cuts one of the four edges (orientation test)
    let o = |p: P, q: P, r: P| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let cut = |c: P, d: P| (o(a, b, c) >= 0.0) != (o(a, b, d) >= 0.0) && (o(c, d, a) >= 0.0) != (o(c, d, b) >= 0.0);
    let (p0, p1, p2, p3) = ((bx.0, bx.1), (bx.2, bx.1), (bx.2, bx.3), (bx.0, bx.3));
    if pb(a) == 0.0 || pb(b) == 0.0 || cut(p0, p1) || cut(p1, p2) || cut(p2, p3) || cut(p3, p0) {
        return 0.0;
    }
    [seg_dist((bx.0, bx.1), a, b), seg_dist((bx.2, bx.1), a, b), seg_dist((bx.0, bx.3), a, b), seg_dist((bx.2, bx.3), a, b), pb(a), pb(b)].into_iter().fold(f64::INFINITY, f64::min)
}
fn overlap(a: Bx, c: Bx, pad: f64) -> bool { a.0 - pad < c.2 && c.0 - pad < a.2 && a.1 - pad < c.3 && c.1 - pad < a.3 }
fn inside(a: Bx, c: Bx) -> bool { a.0 >= c.0 - 0.01 && a.1 >= c.1 - 0.01 && a.2 <= c.2 + 0.01 && a.3 <= c.3 + 0.01 }

// ───────────────────────── a second implementation of the numbers ─────────────────────────

/// (lower, upper, frequency) rows of a good table, typed one per line with commas.
fn rows_of(text: &str) -> Vec<(f64, f64, f64)> {
    let mut v: Vec<(f64, f64, f64)> = text.lines().filter(|l| !l.trim().is_empty()).map(|l| { let c: Vec<f64> = l.split(',').map(|x| x.trim().parse().unwrap()).collect(); (c[0], c[1], c[2]) }).collect();
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    v
}
/// Running totals by plain addition.
fn running(fs: &[f64]) -> Vec<f64> {
    let mut t = 0.0;
    fs.iter().map(|x| { t += x; t }).collect()
}
/// C(x): the cumulative frequency at x, straight between the points at the upper boundaries.
fn c_of(rows: &[(f64, f64, f64)], x: f64) -> f64 {
    let mut below = 0.0;
    for &(lo, hi, fr) in rows {
        if x >= hi {
            below += fr;
        } else if x > lo {
            below += fr * (x - lo) / (hi - lo);
        }
    }
    below
}
/// The smallest x with C(x) >= t, by bisection on C (a different method from the module's scan).
fn inverse(rows: &[(f64, f64, f64)], t: f64) -> f64 {
    let (mut lo, mut hi) = (rows[0].0, rows[rows.len() - 1].1);
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        if c_of(rows, mid) >= t { hi = mid } else { lo = mid }
    }
    hi
}

#[test]
fn cumulative_points_and_readings_agree_with_a_second_implementation() {
    let tables = [
        DEFAULT_TABLE,
        "0, 10, 4\n10, 20, 9\n20, 40, 14\n40, 60, 8\n60, 100, 5",
        "0, 5, 0\n5, 10, 0\n10, 15, 6\n15, 20, 0\n20, 25, 4\n25, 30, 0",
        "-3.5, -1.25, 7\n-1.25, 0, 0\n0, 0.5, 13\n0.5, 4, 1",
        "1000000, 2000000, 999999937\n2000000, 3000000, 3",
        "10, 20, 1",
        "50, 60, 7\n40, 50, 3",
    ];
    for t in tables {
        let cs = parse_table(t).unwrap();
        let rows = rows_of(t);
        assert_eq!(cs.len(), rows.len());
        let mine = running(&rows.iter().map(|r| r.2).collect::<Vec<_>>());
        let cum: Vec<f64> = cumulative(&cs).iter().map(|c| *c as f64).collect();
        assert_eq!(cum, mine, "{t}");
        assert!(cum.windows(2).all(|w| w[1] >= w[0]), "never decreases");
        let pts = points(&cs);
        assert_eq!(pts[0], (rows[0].0, 0.0), "starts at 0 on the first lower boundary");
        for (i, r) in rows.iter().enumerate() {
            assert_eq!(pts[i + 1].0, r.1, "plotted at the UPPER boundary, not the midpoint");
        }
        let n: f64 = rows.iter().map(|r| r.2).sum();
        assert_eq!(pts.last().unwrap().1, n, "ends at the total");
        for k in 1..100 {
            let target = k as f64 / 100.0 * n;
            let x = value_at(&pts, target);
            let want = inverse(&rows, target);
            assert!((x - want).abs() <= 1e-6 * (1.0 + want.abs()), "{t}: P{k} {x} vs {want}");
            assert!((cum_at(&pts, x).unwrap() - target).abs() <= 1e-6 * n, "reading back the curve");
        }
        let span = rows[rows.len() - 1].1 - rows[0].0;
        for k in 0..=50 {
            let x = rows[0].0 + span * k as f64 / 50.0;
            let c = cum_at(&pts, x).unwrap();
            assert!((c - c_of(&rows, x)).abs() <= 1e-6 * n.max(1.0), "{t}: C({x})");
        }
        assert_eq!(cum_at(&pts, rows[0].0 - span * 1e-6), None);
        assert_eq!(cum_at(&pts, rows[rows.len() - 1].1 + span * 1e-6), None);
    }
}

// ───────────────────────── exact arithmetic, written independently of the figure ─────────────────────────

/// A fraction of two i128s, written here from scratch (the figure has its own): the printed
/// readings must be the exact half-up rounding of the typed digits, not of the binary float.
#[derive(Clone, Copy, Debug)]
struct X { n: i128, d: i128 }
fn g(a: u128, b: u128) -> u128 { if b == 0 { a } else { g(b, a % b) } }
fn xn(n: i128, d: i128) -> X {
    assert!(d != 0);
    let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
    let k = g(n.unsigned_abs(), d.unsigned_abs()).max(1) as i128;
    X { n: n / k, d: d / k }
}
fn xint(v: i128) -> X { X { n: v, d: 1 } }
/// A typed decimal exactly, digit by digit.
fn xdec(s: &str) -> X {
    let s = s.trim().replace('\u{2212}', "-");
    let s = s.as_str();
    let (neg, body) = match s.strip_prefix('-') { Some(r) => (true, r), None => (false, s) };
    let (ip, fp) = match body.split_once('.') { Some((a, b)) => (a, b), None => (body, "") };
    let mut n: i128 = if ip.is_empty() { 0 } else { ip.parse().unwrap() };
    let mut d: i128 = 1;
    for ch in fp.chars() {
        n = n * 10 + ch.to_digit(10).unwrap() as i128;
        d *= 10;
    }
    xn(if neg { -n } else { n }, d)
}
fn xadd(a: X, b: X) -> X { xn(a.n * b.d + b.n * a.d, a.d * b.d) }
fn xsub(a: X, b: X) -> X { xn(a.n * b.d - b.n * a.d, a.d * b.d) }
fn xmul(a: X, b: X) -> X { xn(a.n * b.n, a.d * b.d) }
fn xdiv(a: X, b: X) -> X { xn(a.n * b.d, a.d * b.n) }
fn xle(a: X, b: X) -> bool { a.n * b.d <= b.n * a.d }
/// Rounded half up (a tie rounds the magnitude up), to `dec` decimals.
fn xround(a: X, dec: usize) -> String {
    let k = 10i128.pow(dec as u32);
    let num = a.n * k;
    let (q, r) = (num / a.d, num % a.d);
    let v = q + if r.abs() * 2 >= a.d.abs() { num.signum() } else { 0 };
    let m = v.unsigned_abs();
    let kk = k as u128;
    let body = if dec == 0 { (m / kk).to_string() } else { format!("{}.{:0w$}", m / kk, m % kk, w = dec) };
    if m == 0 { body } else { format!("{}{body}", if v < 0 { "\u{2212}" } else { "" }) }
}

/// The table's rows as exact fractions, from the text.
fn rows_exact(text: &str) -> Vec<(X, X, i128)> {
    let mut v: Vec<(X, X, i128)> = text.lines().filter(|l| !l.trim().is_empty()).map(|l| {
        let c: Vec<&str> = l.split(',').map(str::trim).collect();
        (xdec(c[0]), xdec(c[1]), c[2].parse::<i128>().unwrap())
    }).collect();
    v.sort_by(|a, b| (a.0.n as f64 / a.0.d as f64).total_cmp(&(b.0.n as f64 / b.0.d as f64)));
    for w in v.windows(2) {
        assert!(xle(w[0].1, w[1].0), "the classes meet");
    }
    v
}
/// The exact value at the height `t`: the smallest x with C(x) >= t, straight between the points.
fn exact_at(rows: &[(X, X, i128)], t: X) -> X {
    let mut y0 = xint(0);
    let mut x0 = rows[0].0;
    if xle(t, y0) {
        return x0;
    }
    for &(_, hi, f) in rows {
        let y1 = xadd(y0, xint(f));
        if xle(t, y1) {
            return if xle(y1, y0) { x0 } else { xadd(x0, xdiv(xmul(xsub(t, y0), xsub(hi, x0)), xsub(y1, y0))) };
        }
        y0 = y1;
        x0 = hi;
    }
    x0
}
/// The value a printed reading states, from its "(accept A to B)" clause.
fn accept_of(answer: &str) -> Option<(String, String)> {
    let a = answer.split("accept ").nth(1)?;
    let (lo, rest) = a.split_once(" to ")?;
    let hi = rest.split(|c: char| c == ',' || c == ')').next()?;
    Some((lo.to_string(), hi.to_string()))
}

#[test]
fn printed_readings_match_the_recomputation() {
    // the default: 50 values, the 40th percentile is 20 values: 60 + (20 - 10) / 12 * 10
    let m = model(&with(&[s("read", READ_PCT)])).unwrap();
    assert!((m.readings[0].x - (60.0 + 10.0 / 12.0 * 10.0)).abs() < 1e-12);
    assert_eq!(m.readings[0].foot, "P40 = 68.3");
    assert_eq!(m.readings[0].side, "P40: 20 = 40% of 50");
    assert_eq!(m.readings[0].side_short, "P40: 20");
    let rows = rows_exact(DEFAULT_TABLE);
    for d in 0..=4i64 {
        for pct in ["1", "2.5", "25", "33", "50", "75", "99"] {
            let p = with(&[s("read", READ_PCT), f("pct", pct.parse().unwrap()), ("decimals".into(), Value::I(d)), s("marks", "Median and quartiles")]);
            let m = model(&p).unwrap();
            let t = xdiv(xmul(xdec(pct), xint(50)), xint(100));
            let exact = xround(exact_at(&rows, t), d as usize);
            assert_eq!(m.readings[0].foot, format!("P{pct} = {exact}"), "d{d}");
            let ans = m.questions.iter().find(|q| q.0.contains("percentile")).unwrap().1.clone().unwrap();
            assert_eq!(after(&ans, "is about "), exact, "{ans}");
            assert!(accept_of(&ans).is_some(), "{ans}: an accepted range");
            let qa = m.questions.last().unwrap().1.clone().unwrap();
            for (name, qp) in [("Q1 about ", 25), ("median about ", 50), ("Q3 about ", 75)] {
                let want = xround(exact_at(&rows, xdiv(xmul(xint(qp), xint(50)), xint(100))), d as usize);
                assert_eq!(after(&qa, name), want, "{qa} / {name}");
            }
        }
    }
    // a value: 75 is half way through 70 to 80, so 22 + 7.5 = 29.5 of 50 = 59%
    let m = model(&with(&[s("read", READ_VALUE), f("value", 75.0)])).unwrap();
    assert_eq!(m.readings[0].t, 29.5);
    assert_eq!(m.readings[0].side, "29.5 (59.0%)");
    assert!(m.questions[1].1.as_ref().unwrap().contains("about 29.5 of the 50 values, so about 59.0%"));
    assert_eq!(model(&with(&[s("read", READ_VALUE), f("value", 75.0), s("y_axis", Y_PCT)])).unwrap().readings[0].side, "59.0%");
    assert_eq!(model(&with(&[s("read", READ_VALUE), f("value", 75.0), s("y_axis", Y_REL)])).unwrap().readings[0].side, "0.590");
    // a percentage-of-a-value answer carries an accepted range too, half a fine vertical gridline
    let ans = m.questions.iter().find(|q| q.0.contains("percentage")).unwrap().1.clone().unwrap();
    let (lo, hi) = accept_of(&ans).unwrap_or_else(|| panic!("no range in {ans}"));
    let (lo, hi) = (lo.trim_end_matches('%').parse::<f64>().unwrap(), hi.trim_end_matches('%').parse::<f64>().unwrap());
    assert!(lo <= 59.0 && 59.0 <= hi && lo < hi, "the range {lo} to {hi} misses 59%: {ans}");
    // a percentile's guide names the reading, not just its height
    let m = model(&with(&[s("read", READ_PCT), f("pct", 40.0), s("marks", "Median and quartiles")])).unwrap();
    assert!(m.readings[0].side.starts_with("P40: "), "{:?}", m.readings[0].side);
    assert!(m.readings[1].side.starts_with("Q1: "), "{:?}", m.readings[1].side);
    assert!(m.readings[1].side.contains("25% of 50"), "{:?}", m.readings[1].side);
    // the table's third column and the first question name what the vertical axis shows
    for (y, head, q) in [(Y_FREQ, ">frequency<", "cumulative frequency column"), (Y_REL, ">rel. frequency<", "cumulative relative frequency column"), (Y_PCT, ">percent<", "cumulative percent column")] {
        let svg = render(&with(&[s("y_axis", y)]));
        assert!(svg.contains(&format!("class=\"thead\"")) && svg.matches(head).count() == 1 && svg.contains(q), "{y}");
        assert!(svg.contains(&format!(">{y}<")), "{y}: the vertical axis title");
    }
    assert_eq!(ordinal(1.0), "1st");
    assert_eq!(ordinal(12.0), "12th");
    assert_eq!(ordinal(22.0), "22nd");
    assert_eq!(ordinal(93.0), "93rd");
    assert_eq!(ordinal(2.5), "2.5th");
}

/// The token after `label`, up to a separator (a printed number).
fn after(text: &str, label: &str) -> String {
    let t = text.split(label).nth(1).unwrap_or_else(|| panic!("no {label:?} in {text:?}"));
    t.split(|c: char| c == ' ' || c == ',' || c == ')' || c == ';').next().unwrap().to_string()
}

/// Tables with a value that lands exactly on a rounding tie, plus the ordinary ones.
const EXACT_TABLES: &[&str] = &[
    DEFAULT_TABLE,
    // P37 of 20 values is 8.85 exactly (the binary float holds 8.8499...)
    "6, 8, 4\n8, 10, 8\n10, 20, 8",
    // the median is 1.625 exactly
    "0, 1, 2\n1, 2, 8\n2, 3, 4",
    // the lower quartile is 12.5 exactly
    "10, 20, 5",
    "0, 10, 4\n10, 20, 9\n20, 40, 14\n40, 60, 8\n60, 100, 5",
    "-3.5, -1.25, 7\n-1.25, 0, 0\n0, 0.5, 13\n0.5, 4, 1",
];

#[test]
fn printed_readings_are_the_exact_half_up_rounding() {
    for table in EXACT_TABLES {
        let rows = rows_exact(table);
        let n: i128 = rows.iter().map(|r| r.2).sum();
        for d in 0..=4usize {
            for pct in [1i64, 5, 10, 25, 33, 37, 40, 50, 60, 67, 75, 90, 99] {
                let pr = xdec(&pct.to_string());
                let t = xdiv(xmul(pr, xint(n)), xint(100));
                let want = xround(exact_at(&rows, t), d);
                let p = with(&[s("table", table), s("read", READ_PCT), f("pct", pct as f64), ("decimals".into(), Value::I(d as i64)), s("marks", "Median and quartiles")]);
                let m = model(&p).unwrap();
                assert_eq!(m.readings[0].foot, format!("P{pct} = {want}"), "{table} P{pct} d{d}");
                let ans = m.questions.iter().find(|q| q.0.contains("percentile")).unwrap().1.clone().unwrap();
                assert_eq!(after(&ans, "is about "), want, "{table} P{pct} d{d}: {ans}");
                // the accepted range holds the exact reading (the key says how far a read may be)
                let (lo, hi) = accept_of(&ans).unwrap_or_else(|| panic!("no range in {ans}"));
                let exact = exact_at(&rows, t);
                assert!(xle(xdec(&lo), exact) && xle(exact, xdec(&hi)), "{table} P{pct} d{d}: {lo}..{hi} misses {exact:?}");
                // quartiles and median, each the exact half-up rounding
                let qa = m.questions.last().unwrap().1.clone().unwrap();
                for (name, qp) in [("Q1 about ", 25i64), ("median about ", 50), ("Q3 about ", 75)] {
                    let qt = xdiv(xmul(xdec(&qp.to_string()), xint(n)), xint(100));
                    assert_eq!(after(&qa, name), xround(exact_at(&rows, qt), d), "{table} {name} d{d}: {qa}");
                }
                // the IQR is the printed Q3 minus the printed Q1
                let (q1, q3, iqr) = (after(&qa, "Q1 about "), after(&qa, "Q3 about "), after(&qa, "IQR about "));
                assert!(xle(xdec(&iqr), xsub(xdec(&q3), xdec(&q1))) && xle(xsub(xdec(&q3), xdec(&q1)), xdec(&iqr)), "{table} d{d}: Q1 {q1}, Q3 {q3}, IQR {iqr} in {qa}");
            }
        }
    }
    // the cases the critic named
    let m = model(&with(&[s("table", "10, 20, 5"), s("marks", "Median and quartiles"), ("decimals".into(), Value::I(0))])).unwrap();
    let qa = m.questions.last().unwrap().1.clone().unwrap();
    assert_eq!(after(&qa, "Q1 about "), "13", "12.5 rounds up: {qa}");
    assert_eq!(after(&qa, "IQR about "), "5", "{qa}");
    let m = model(&with(&[s("table", "6, 8, 4\n8, 10, 8\n10, 20, 8"), s("read", READ_PCT), f("pct", 37.0), ("decimals".into(), Value::I(1))])).unwrap();
    assert_eq!(m.readings[0].foot, "P37 = 8.9", "8.85 rounds up, the float 8.8499 does not");
    let m = model(&with(&[s("table", "0, 1, 2\n1, 2, 8\n2, 3, 4"), s("marks", "Median"), ("decimals".into(), Value::I(2))])).unwrap();
    assert_eq!(after(m.questions.last().unwrap().1.as_ref().unwrap(), "median about "), "1.63");
    let m = model(&with(&[s("marks", "Median and quartiles"), ("decimals".into(), Value::I(0))])).unwrap();
    let qa = m.questions.last().unwrap().1.clone().unwrap();
    let (q1, q3, iqr) = (after(&qa, "Q1 about "), after(&qa, "Q3 about "), after(&qa, "IQR about "));
    assert_eq!((q1.as_str(), q3.as_str(), iqr.as_str()), ("62", "81", "19"), "{qa}");
    // the table's running totals are exact halves too (100 x 3 / 8 is 37.5 at one decimal, and
    // 3 / 8 is 0.375 at three)
    for (c, n) in [(1u64, 8u64), (3, 8), (5, 16), (7, 40), (13, 32)] {
        let pc = xround(xdiv(xmul(xint(c as i128), xint(100)), xint(n as i128)), 1);
        let rel = xround(xdiv(xint(c as i128), xint(n as i128)), 3);
        assert_eq!(cum_text(Y_PCT, c, n), pc, "cumulative percent {c}/{n}");
        assert_eq!(cum_text(Y_REL, c, n), rel, "cumulative relative frequency {c}/{n}");
        assert_eq!(cum_text(Y_FREQ, c, n), c.to_string());
    }
}

#[test]
fn the_printed_iqr_is_the_printed_q3_minus_the_printed_q1() {
    // a student subtracts the two printed quartiles, so the key's IQR must be that difference
    for table in EXACT_TABLES {
        for d in 0..=4i64 {
            let p = with(&[s("table", table), s("marks", "Median and quartiles"), ("decimals".into(), Value::I(d))]);
            let m = model(&p).unwrap();
            let qa = m.questions.last().unwrap().1.clone().unwrap();
            let (q1, q3, iqr) = (xdec(&after(&qa, "Q1 about ")), xdec(&after(&qa, "Q3 about ")), xdec(&after(&qa, "IQR about ")));
            let diff = xsub(q3, q1);
            assert!(xle(diff, iqr) && xle(iqr, diff), "{table} d{d}: Q3 - Q1 is not the IQR: {qa}");
        }
    }
}

#[test]
fn a_low_percentile_read_alone_is_labelled_at_every_size() {
    // the most common teacher use: one reading, no marks. Every percentile in 1..99 must get a
    // label in the key (the corner near the axis is cramped at low percentiles)
    let mut total = 0;
    for (name, base) in datasets().into_iter().take(5) {
        for size in ["Small", "Medium", "Large"] {
            for k in 1..=99 {
                let mut l = base.clone();
                l.extend([s("read", READ_PCT), f("pct", k as f64), s("label_size", size)]);
                let p = with(&l);
                let mut key = p.clone();
                key.set_b(SHOW_ANSWERS, true);
                let what = format!("{name} P{k} {size}");
                let bare = draw(&key).1;
                assert!(bare.is_empty(), "{what}: no label for {bare:?}");
                let g = parse(&render(&key), false);
                let m = model(&key).unwrap();
                for r in &m.readings {
                    let has = g.texts.iter().any(|t| (t.class == "side" && (t.body == r.side || t.body == r.side_short)) || (t.class == "foot" && (t.body == r.foot || t.body == r.short)));
                    assert!(has, "{what}: {:?} has no label", r.short);
                }
                assert!(!warnings(&p).iter().any(|n| n.contains("no room")), "{what}: {:?}", warnings(&p));
                total += 1;
            }
        }
    }
    println!("ogive: {total} single-reading keys, every one labelled");
}

#[test]
fn stated_limits_of_width_two_are_shifted_not_padded() {
    // integer limits 0 to 1, 2 to 3, ... are stated limits (a gap of 1, classes 1 wide): the
    // boundaries are -0.5, 1.5, 3.5, 5.5, 7.5, not an empty class between each pair
    let t = "0-1, 4\n2-3, 7\n4-5, 2\n6-7, 1";
    let note = parse_table(t).unwrap_err();
    assert!(note.contains("stated class limits") && !note.contains("Add an empty class"), "{note}");
    let mut text = t.to_string();
    for _ in 0..10 {
        match parse_table(&text) {
            Ok(cs) => {
                let b: Vec<f64> = std::iter::once(cs[0].lo).chain(cs.iter().map(|c| c.hi)).collect();
                assert_eq!(b, vec![-0.5, 1.5, 3.5, 5.5, 7.5], "{text}");
                assert_eq!(cs.iter().map(|c| c.f).collect::<Vec<_>>(), vec![4, 7, 2, 1]);
                return;
            }
            Err(n) => text = apply_table_advice(&text, &n).unwrap_or_else(|| panic!("no step in {n}")),
        }
    }
    panic!("the advice never settled: {text}");
}

#[test]
fn common_table_formats_are_read_and_the_advice_keeps_the_data() {
    // every way of writing a range the figure itself uses is two boundaries
    for (t, want) in [
        ("40 - 49, 3\n50 - 59, 7", "stated"),
        ("40 \u{2013} 49, 3\n50 \u{2013} 59, 7", "stated"),
        ("40\u{2014}50, 3\n50\u{2014}60, 7", "ok"),
        ("40 to 50, 3\n50 to 60, 7", "ok"),
        ("40 through 50, 3\n50 through 60, 7", "ok"),
        // a sign run into the number ("40 -49"), a dash run into the space ("40- 49"), the true
        // minus (U+2212) and a spreadsheet paste of thousands ("1,000") all keep their data
        ("40 -49, 3\n50 -59, 7", "stated"),
        ("40- 49, 3\n50- 59, 7", "stated"),
        ("40 \u{2212} 50, 3\n50 \u{2212} 60, 7", "ok"),
    ] {
        match parse_table(t) {
            Ok(cs) => {
                assert_eq!(want, "ok", "{t:?} was read: {cs:?}");
                assert_eq!(cs.iter().map(|c| (c.lo, c.hi, c.f)).collect::<Vec<_>>(), vec![(40.0, 50.0, 3), (50.0, 60.0, 7)], "{t:?}");
            }
            Err(e) => assert_eq!(want, "stated", "{t:?}: {e}"),
        }
    }
    // a spreadsheet paste and a header that carries a number read as the table they name
    let cs = parse_table("1,000, 2,000, 5\n2,000, 3,000, 7").expect("thousands separators");
    assert_eq!(cs.iter().map(|c| (c.lo, c.hi, c.f)).collect::<Vec<_>>(), vec![(1000.0, 2000.0, 5), (2000.0, 3000.0, 7)]);
    let cs = parse_table("Class 1, Frequency\n40, 50, 3\n50, 60, 7").expect("a header with a number");
    assert_eq!(cs.iter().map(|c| (c.lo, c.hi, c.f)).collect::<Vec<_>>(), vec![(40.0, 50.0, 3), (50.0, 60.0, 7)], "the header must not become a class");
    // a total row is a footer: say so and say to delete it
    let t = "40, 50, 3\n50, 60, 7\nTotal, 50";
    let note = parse_table(t).unwrap_err();
    assert!(note.contains("is a total") && note.contains("delete line 3"), "{note}");
    let fixed = apply_table_advice(t, &note).expect("a delete step");
    assert_eq!(parse_table(&fixed).unwrap().iter().map(|c| c.f).collect::<Vec<_>>(), vec![3, 7]);
    // two columns of midpoints and frequencies: the boundaries are halfway to the next midpoint,
    // and following the advice three times gives the classes the teacher meant
    let t = "midpoint, f\n45, 3\n55, 7\n65, 4";
    let mut text = t.to_string();
    for _ in 0..6 {
        match parse_table(&text) {
            Ok(cs) => {
                assert_eq!(cs.iter().map(|c| (c.lo, c.hi, c.f)).collect::<Vec<_>>(), vec![(40.0, 50.0, 3), (50.0, 60.0, 7), (60.0, 70.0, 4)], "{text}");
                return;
            }
            Err(n) => {
                assert!(n.contains("not a class"), "{n}");
                text = apply_table_advice(&text, &n).unwrap_or_else(|| panic!("no line in {n}"));
            }
        }
    }
    panic!("the midpoint advice never settled: {text}");
}

#[test]
fn raw_values_use_the_histograms_classes_and_count_every_value_once() {
    let sets: Vec<Vec<f64>> = vec![
        data::numbers(DEFAULT_DATA),
        data::generate(data::Dist::SkewedRight, 60, 4, 52.0, 18.0, 0),
        data::generate(data::Dist::Normal, 300, 7, 0.0, 0.004, 4),
        vec![10.0, 20.0, 20.0, 30.0, 40.0],
        vec![5.0; 9],
    ];
    for v in &sets {
        for lc in [true, false] {
            let w = super::super::auto_width(v);
            let st = super::super::auto_start(v, w, lc);
            let hist = super::super::bins(v, st, w, lc).unwrap();
            let cs = raw_classes(v, true, 0.0, 0.0, lc).unwrap();
            assert_eq!(cs.iter().map(|c| (c.lo, c.hi, c.f as usize)).collect::<Vec<_>>(), hist.iter().map(|b| (b.lo, b.hi, b.n)).collect::<Vec<_>>(), "same classes as the histogram");
            assert_eq!(cs.iter().map(|c| c.f).sum::<u64>(), v.len() as u64, "every value counted once");
            // counted by hand: a value is in [lo, hi) or (lo, hi]
            for c in &cs {
                let n = v.iter().filter(|x| if lc { **x >= c.lo - 1e-9 && **x < c.hi - 1e-9 } else { **x > c.lo + 1e-9 && **x <= c.hi + 1e-9 }).count();
                assert_eq!(c.f as usize, n, "{c:?}");
            }
        }
    }
    // the edge rule, counted by hand: [10,20) [20,30) [30,40) [40,50) and (0,10] (10,20] (20,30] (30,40]
    let v = [10.0, 20.0, 20.0, 30.0, 40.0];
    let right = raw_classes(&v, false, 10.0, 10.0, true).unwrap();
    assert_eq!(right.iter().map(|c| c.f).collect::<Vec<_>>(), [1, 2, 1, 1]);
    let left = raw_classes(&v, false, 10.0, 0.0, false).unwrap();
    assert_eq!(left.iter().map(|c| c.f).collect::<Vec<_>>(), [1, 2, 1, 1]);
    assert_eq!(left[0].lo, 0.0);
    // the point at 20 counts the values below 20 (left-closed) or at or below 20 (right-closed)
    assert_eq!(points(&right)[1], (20.0, 1.0));
    assert_eq!(points(&left)[2], (20.0, 3.0));
    // a value on the first boundary of a right-closed class is before the first class: refused, with advice
    let e = raw_classes(&v, false, 10.0, 10.0, false).unwrap_err();
    assert!(e.contains("1 value is before the first class") && e.contains("set First class starts at to 0 or less"), "{e}");
    // labels say the edge rule
    let svg = render(&with(&[s("input", RAW), s("data", "10, 20, 20, 30, 40"), b("auto_classes", false), f("class_width", 10.0), f("class_start", 10.0)]));
    assert!(svg.contains(">10 to &lt;20<") && svg.contains(">40 to &lt;50<"), "{svg}");
    let svg = render(&with(&[s("input", RAW), s("data", "10, 20, 20, 30, 40"), b("auto_classes", false), f("class_width", 10.0), f("class_start", 0.0), s("closed", RIGHT_CLOSED), s("read", READ_VALUE), f("value", 25.0)]));
    assert!(svg.contains(">&gt;0 to 10<") && svg.contains("values at or below"), "right-closed wording");
}

// ───────────────────────── refusals and the advice they give ─────────────────────────

/// Applies a table note's advice as written and returns the new table text.
fn apply_table_advice(text: &str, note: &str) -> Option<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let line_no = |pat: &str| -> Option<usize> { note.split(pat).nth(1)?.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok() };
    if note.contains("is a total") || note.contains("beyond the largest number") {
        let n = line_no("delete line ")?;
        lines.remove(n - 1);
        return Some(lines.join("\n"));
    }
    let quoted = note.split("for example \"").nth(1)?.split('"').next()?.to_string();
    if note.contains(" off every lower limit") {
        let h: f64 = note.split("take ").nth(1)?.split(' ').next()?.parse().ok()?;
        return Some(lines.iter().map(|l| {
            let c: Vec<f64> = cells(l).iter().filter_map(|x| x.trim().parse().ok()).collect();
            if c.len() == 3 { format!("{}, {}, {}", c[0] - h, c[1] + h, c[2]) } else { l.clone() }
        }).collect::<Vec<_>>().join("\n"));
    }
    if note.contains("Add an empty class between them") {
        lines.push(quoted);
        return Some(lines.join("\n"));
    }
    if note.contains("Merge them") {
        let keep = line_no("add its frequency to line ")?;
        let del = line_no("and delete line ")?;
        lines[keep - 1] = quoted;
        lines.remove(del - 1);
        return Some(lines.join("\n"));
    }
    let n = if note.contains("\" on line ") { line_no("\" on line ")? } else { line_no("Line ")? };
    lines[n - 1] = quoted;
    Some(lines.join("\n"))
}

/// Applies the advice until the table is accepted: what the teacher ends up with.
fn settled(text: &str) -> Vec<(f64, f64, u64)> {
    let mut text = text.to_string();
    for _ in 0..12 {
        match parse_table(&text) {
            Ok(cs) => return cs.iter().map(|c| (c.lo, c.hi, c.f)).collect(),
            Err(note) => text = apply_table_advice(&text, &note).unwrap_or_else(|| panic!("no step in {note}")),
        }
    }
    panic!("the advice never settled: {text}");
}

#[test]
fn the_advice_ends_at_the_table_the_teacher_meant() {
    // not just any drawing: the data the teacher typed, under the one reading the note gives
    let cases: &[(&str, &[(f64, f64, u64)])] = &[
        ("40, 49, 3\n50, 59, 7\n60, 69, 2", &[(39.5, 49.5, 3), (49.5, 59.5, 7), (59.5, 69.5, 2)]),
        ("0-1, 4\n2-3, 7\n4-5, 2\n6-7, 1", &[(-0.5, 1.5, 4), (1.5, 3.5, 7), (3.5, 5.5, 2), (5.5, 7.5, 1)]),
        ("40 - 49, 3\n50 - 59, 7", &[(39.5, 49.5, 3), (49.5, 59.5, 7)]),
        ("40 to 50, 3\n50 to 60, 3", &[(40.0, 50.0, 3), (50.0, 60.0, 3)]),
        ("40\u{2014}49, 3\n50\u{2014}59, 7", &[(39.5, 49.5, 3), (49.5, 59.5, 7)]),
        ("40, 50, 3\n50, 60, 7\nTotal, 50", &[(40.0, 50.0, 3), (50.0, 60.0, 7)]),
        ("midpoint, f\n45, 3\n55, 7\n65, 4", &[(40.0, 50.0, 3), (50.0, 60.0, 7), (60.0, 70.0, 4)]),
        ("40, 50, 3\nfifty, 60, 2", &[(40.0, 50.0, 3), (50.0, 60.0, 2)]),
        ("40, 50, 3\n50, 60, 2.5", &[(40.0, 50.0, 3), (50.0, 60.0, 3)]),
        ("40, 50, 3\n60, 50, 4", &[(40.0, 50.0, 3), (50.0, 60.0, 4)]),
        ("40, 50, 3\n52, 60, 4\n60, 70, 1", &[(40.0, 50.0, 3), (50.0, 52.0, 0), (52.0, 60.0, 4), (60.0, 70.0, 1)]),
        ("40-50, 3\n50-60 7\n60-70, 4\n80, 90, 1", &[(40.0, 50.0, 3), (50.0, 60.0, 7), (60.0, 70.0, 4), (70.0, 80.0, 0), (80.0, 90.0, 1)]),
        ("40, 50, 3\n50, 60, -2\n60, 70, 1", &[(40.0, 50.0, 3), (50.0, 60.0, 0), (60.0, 70.0, 1)]),
        ("40, 50, 3\n45, 55, 4", &[(40.0, 50.0, 3), (50.0, 55.0, 4)]),
        ("40, 60, 3\n45, 55, 4", &[(40.0, 60.0, 7)]),
        // a gap bigger than one limit unit is a missing class, not stated limits: padding keeps
        // every count where it was, rescaling moved them
        ("0, 10, 3\n20, 30, 4", &[(0.0, 10.0, 3), (10.0, 20.0, 0), (20.0, 30.0, 4)]),
        ("40, 50, 3\n52, 60, 4", &[(40.0, 50.0, 3), (50.0, 52.0, 0), (52.0, 60.0, 4)]),
        ("0, 0.9, 3\n1.0, 1.9, 4", &[(-0.05, 0.95, 3), (0.95, 1.95, 4)]),
        // the same class however the range is written
        ("40 -49, 3\n50 -59, 7", &[(39.5, 49.5, 3), (49.5, 59.5, 7)]),
        ("40- 49, 3\n50- 59, 7", &[(39.5, 49.5, 3), (49.5, 59.5, 7)]),
        ("40 \u{2212} 50, 3\n50 \u{2212} 60, 7", &[(40.0, 50.0, 3), (50.0, 60.0, 7)]),
        // a spreadsheet paste
        ("1,000, 2,000, 5\n2,000, 3,000, 7", &[(1000.0, 2000.0, 5), (2000.0, 3000.0, 7)]),
        // a number beyond the limit: the note names it and deletion is the route
        ("0, 500, 3\n1000, 2000, 5\n1000000000000, 2000000000000, 1", &[(0.0, 500.0, 3), (500.0, 1000.0, 0), (1000.0, 2000.0, 5)]),
    ];
    for (text, want) in cases {
        let got = settled(text);
        assert_eq!(got.len(), want.len(), "{text:?}: {got:?}");
        for (a, b) in got.iter().zip(want.iter()) {
            assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9 && a.2 == b.2, "{text:?}: {got:?} vs {want:?}");
        }
    }
}

#[test]
fn every_table_refusal_names_a_fix_that_works_when_applied() {
    let bad = [
        "40, 49, 3\n50, 59, 7\n60, 69, 2",
        "1, 2.9, 3\n3, 4.9, 1",
        "40, 50, 3\n50, 60, -2\n60, 70, 1",
        "40, 50, 3\n50, 60, 2.5",
        "40, 50, 3\n60, 50, 4",
        "40, 50, 3\n50, 50, 4\n50, 60, 1",
        "40, 50, 3\n45, 55, 4",
        "40, 60, 3\n45, 55, 4",
        "40, 50, 3\n40, 50, 4",
        "40, 50, 3\n52, 60, 4\n60, 70, 1",
        "40, 50, 3\n50, 60, 1\n90+, 4",
        "under 40, 2\n40, 50, 3",
        "40, 50, 3\nfifty, 60, 2",
        "40, 50, 0\n50, 60, 0",
        "Class, Freq\n40, 50, 3\n50, 60, x",
        "40-50, 3\n50-60 7\n60-70, 4\n80, 90, 1",
        "0, 1, 1\n1, 2, 1\n2, 3, -1\n5, 9, 1\n9, 9, 2\n8, 20, 1",
        "0, 500, 3\n1000, 2000, 5\n1000000000000, 2000000000000, 1",
        "40 -49, 3\n50 -59, 7",
        "40- 49, 3\n50- 59, 7",
        "",
        "\n\n",
    ];
    for start in bad {
        let mut text = start.to_string();
        let mut steps = 0;
        loop {
            match parse_table(&text) {
                Ok(cs) => {
                    assert!(cs.windows(2).all(|w| w[0].hi == w[1].lo), "{start}: classes meet");
                    break;
                }
                Err(note) => {
                    steps += 1;
                    assert!(steps <= 10, "{start}: the advice goes round in circles, at {text:?}: {note}");
                    let p = with(&[s("table", &text)]);
                    assert_eq!(warnings(&p), vec![note.clone()], "the editor note is the refusal");
                    let shown: String = parse(&render(&p), false).texts.iter().map(|t| t.body.clone()).collect::<Vec<_>>().join(" ");
                    assert_eq!(shown.split_whitespace().collect::<Vec<_>>(), note.split_whitespace().collect::<Vec<_>>(), "the canvas says the same");
                    // any example the note quotes must itself be a class the figure accepts: the
                    // note at the 1e12 limit used to hand back a line that was refused for ever
                    if let Some(ex) = note.split("for example \"").nth(1).and_then(|x| x.split('"').next()).filter(|_| !text.trim().is_empty()) {
                        assert!(parse_table(ex).is_ok() || parse_table(ex).err() != Some(note.clone()), "{start}: the example {ex:?} is refused the same way");
                        // and typing it into the line it names changes the table
                        let next = apply_table_advice(&text, &note).unwrap_or_else(|| panic!("{start}: no edit in {note}"));
                        assert_ne!(parse_table(&next).err(), Some(note.clone()), "{start}: the advice repeats itself");
                    }
                    text = if text.trim().is_empty() { note.split("for example \"").nth(1).unwrap().split('"').next().unwrap().to_string() } else { apply_table_advice(&text, &note).unwrap_or_else(|| panic!("{start}: no edit in {note}")) };
                }
            }
        }
        assert!(steps >= 1, "{start} should have been refused");
    }
    // the specific statistics in the wording
    let e = parse_table("40, 49, 3\n50, 59, 7").unwrap_err();
    assert!(e.contains("stated class limits") && e.contains("\"39.5, 49.5, 3\""), "{e}");
    let e = parse_table("40, 50, 3\n50, 60, 1\n90+, 4").unwrap_err();
    assert!(e.contains("open-ended") && e.contains("\"90, 100, 4\""), "{e}");
    let e = parse_table("under 40, 2\n40, 50, 3").unwrap_err();
    assert!(e.contains("open-ended") && e.contains("\"30, 40, 2\""), "{e}");
    assert!(parse_table("40, 50, 3\n50, 60, -1").unwrap_err().contains("cannot be negative"));
    assert!(parse_table("40, 50, 3\n52, 60, 4\n60, 70, 1").unwrap_err().contains("\"50, 52, 0\""));
}

#[test]
fn raw_refusals_and_other_notes_have_advice_that_works() {
    // values before the first class
    let p = with(&[s("input", RAW), b("auto_classes", false), f("class_start", 60.0), f("class_width", 10.0)]);
    let n = warnings(&p);
    assert_eq!(n.len(), 1, "{n:?}");
    let x: f64 = n[0].split("set First class starts at to ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
    let mut q = p.clone();
    q.set_f("class_start", x);
    assert!(warnings(&q).is_empty(), "{:?}", warnings(&q));
    assert!(x <= 52.0, "the advice is at or below the smallest value");
    // too many classes
    let p = with(&[s("input", RAW), b("auto_classes", false), f("class_start", 0.0), f("class_width", 0.01), s("data", "0, 3, 1000")]);
    let n = warnings(&p);
    let w: f64 = n[0].split("Make the class width ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
    let mut q = p.clone();
    q.set_f("class_width", w);
    assert!(classes(&q).is_ok() && classes(&q).unwrap().list.len() <= MAX_CLASSES);
    q.set_f("class_width", w / 2.0);
    assert!(classes(&q).is_err(), "the advised width is the smallest 1-2-5 width that works (10, not 5)");
    // a width the control cannot take: the advice switches to automatic classes
    let p = with(&[s("input", RAW), b("auto_classes", false), f("class_start", 0.0), f("class_width", 1.0), s("data", "0, 900000000")]);
    let n = warnings(&p);
    assert!(n[0].contains("Turn on Choose the classes automatically"), "{n:?}");
    let mut q = p.clone();
    q.set_b("auto_classes", true);
    assert!(warnings(&q).is_empty());
    let p = with(&[s("input", RAW), b("auto_classes", false), f("class_start", -10000.0), f("class_width", 1000.0), s("data", "-50000, 0")]);
    assert!(warnings(&p)[0].contains("turn on Choose the classes automatically"), "{:?}", warnings(&p));
    // too many rows for the table
    let many: String = (0..20).map(|i| format!("{}, {}, {}", i * 5, i * 5 + 5, i % 4 + 1)).collect::<Vec<_>>().join("\n");
    let p = with(&[s("table", &many)]);
    let n = warnings(&p);
    assert!(n.len() == 1 && n[0].contains("Turn off Frequency table beside the graph"), "{n:?}");
    let mut q = p.clone();
    q.set_b("show_table", false);
    assert!(warnings(&q).is_empty());
    // a value outside the classes
    let p = with(&[s("read", READ_VALUE), f("value", 120.0)]);
    let n = warnings(&p);
    assert!(n[0].contains("Type a value between 40 and 100"), "{n:?}");
    for v in [55.0, 70.0, 100.0] {
        let mut q = p.clone();
        q.set_f("value", v);
        assert!(warnings(&q).is_empty(), "{v}");
    }
}

#[test]
fn a_flat_stretch_is_noted_and_read_at_its_lowest_value() {
    let t = "0, 5, 0\n5, 10, 0\n10, 15, 6\n15, 20, 0\n20, 25, 4\n25, 30, 0";
    // 60% of 10 is 6: flat from 15 to 20
    let p = with(&[s("table", t), s("read", READ_PCT), f("pct", 60.0)]);
    let m = model(&p).unwrap();
    assert_eq!(m.readings[0].x, 15.0);
    let n = warnings(&p);
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].contains("from 15 to 20") && n[0].contains("reads the lowest, 15"));
    // no note away from a flat stretch, and none for a strictly rising curve
    assert!(warnings(&with(&[s("table", t), s("read", READ_PCT), f("pct", 30.0)])).is_empty());
    assert!(warnings(&with(&[s("read", READ_PCT), s("marks", "Median and quartiles")])).is_empty());
    // the note says every value from 15 to 20 fits, so the key's accepted range must cover them:
    // reading 18 is right, and 14.2 to 15.8 (half a gridline either side of 15) said it was wrong
    let ans = m.questions.iter().find(|q| q.0.contains("percentile")).unwrap().1.clone().unwrap();
    let (lo, hi) = accept_of(&ans).unwrap_or_else(|| panic!("no range in {ans}"));
    let (lo, hi) = (lo.parse::<f64>().unwrap(), hi.parse::<f64>().unwrap());
    assert!(lo <= 15.0 && hi >= 20.0, "the range {lo} to {hi} contradicts the note: {ans}");
    assert!(lo <= 18.0 && 18.0 <= hi, "a student reading 18 is inside the range: {ans}");
}

// ───────────────────────── the drawing, over the option space ─────────────────────────

/// Every rule a drawing must keep, checked on its SVG. `key` is whether answers show.
fn audit(svg: &str, key: bool, what: &str) -> Svg {
    let g = parse(svg, true);
    let canvas = (0.0, 0.0, g.w, g.h);
    for t in &g.texts {
        assert!(inside(t.ink, canvas), "{what}: {:?} off the page {:?}", t.body, t.ink);
        if t.class != "ytitle" {
            let est = text_box(t.x, t.y, t.size, &t.anchor, &t.body);
            assert!(inside(t.ink, (est.0 - 0.5, est.1 - 0.5, est.2 + 0.5, est.3 + 0.5)), "{what}: {:?} ink {:?} outside its estimate {est:?}", t.body, t.ink);
        }
    }
    for (i, a) in g.texts.iter().enumerate() {
        for c in &g.texts[i + 1..] {
            assert!(!overlap(a.ink, c.ink, 1.0), "{what}: {:?} touches {:?}", a.body, c.body);
        }
    }
    let mut segs: Vec<(String, P, P)> = g.lines.iter().filter(|l| l.class != "grid").map(|l| (l.class.clone(), l.a, l.b)).collect();
    for pl in &g.plines {
        for w in pl.pts.windows(2) {
            segs.push((pl.class.clone(), w[0], w[1]));
        }
    }
    for t in &g.texts {
        for (class, a, b) in &segs {
            let gap = seg_box(*a, *b, t.ink);
            assert!(gap >= 1.0, "{what}: {class} line touches {:?} ({gap:.2} px)", t.body);
        }
        for (c, r) in &g.dots {
            let ex = (t.ink.0 - c.0).max(0.0).max(c.0 - t.ink.2);
            let ey = (t.ink.1 - c.1).max(0.0).max(c.1 - t.ink.3);
            assert!(ex.hypot(ey) >= r + 1.0, "{what}: a point touches {:?}", t.body);
        }
        // a gridline under a text only where a white plate covers the text (a horizontal line at
        // the text's height, or a vertical one at its x, and the plate over the whole ink box)
        for l in g.lines.iter().filter(|l| l.class == "grid") {
            if seg_box(l.a, l.b, t.ink) < 0.5 {
                let crossed = g.plates.iter().any(|pl| inside(t.ink, *pl) && {
                    if (l.a.1 - l.b.1).abs() < 1e-9 { l.a.1 > pl.1 && l.a.1 < pl.3 } else { l.a.0 > pl.0 && l.a.0 < pl.2 }
                });
                assert!(crossed, "{what}: gridline through {:?}", t.body);
            }
        }
    }
    // plates hide gridlines only
    for pl in &g.plates {
        for (class, a, b) in &segs {
            assert!(seg_box(*a, *b, *pl) > 0.5, "{what}: a label's plate covers a {class} line");
        }
        for (c, r) in &g.dots {
            let ex = (pl.0 - c.0).max(0.0).max(c.0 - pl.2);
            let ey = (pl.1 - c.1).max(0.0).max(c.1 - pl.3);
            assert!(ex.hypot(ey) > *r, "{what}: a plate covers a point");
        }
    }
    for q in g.plines.iter().flat_map(|l| l.pts.iter()).chain(g.lines.iter().flat_map(|l| [&l.a, &l.b])) {
        assert!(q.0 >= 0.0 && q.0 <= g.w && q.1 >= 0.0 && q.1 <= g.h, "{what}: a line off the page");
    }
    if !key {
        for class in ["\"guide\"", "\"plate\"", "\"side\"", "\"foot\"", "\"tcum\"", "\"answer\""] {
            assert!(!g.src.contains(class), "{what}: student copy has {class}");
        }
    }
    g
}

/// The axes read back: the x pixel of a value from the boundary ticks, the y pixel of a height from the y ticks.
struct Axes { left: f64, bottom: f64, bx: Vec<f64>, y0: f64, ypx_per: f64 }
fn axes_of(g: &Svg, bounds: &[f64]) -> Axes {
    let left = g.lines.iter().find(|l| l.class == "axis" && l.a.0 == l.b.0).unwrap().a.0;
    let bottom = g.lines.iter().find(|l| l.class == "axis" && l.a.1 == l.b.1).unwrap().a.1;
    let xt: Vec<f64> = g.lines.iter().filter(|l| l.class == "tick" && l.a.0 == l.b.0).map(|l| l.a.0).collect();
    assert_eq!(xt.len(), bounds.len(), "one tick per class boundary");
    let yt: Vec<(f64, f64)> = g.lines.iter().filter(|l| l.class == "tick" && l.a.1 == l.b.1).map(|l| l.a.1).zip(g.texts.iter().filter(|t| t.class == "ytick").map(|t| t.body.replace('\u{2212}', "-").parse::<f64>().unwrap())).collect();
    let (a, z) = (yt[0], yt[yt.len() - 1]);
    Axes { left, bottom, bx: xt, y0: a.0 - a.1 * (z.0 - a.0) / (z.1 - a.1), ypx_per: (z.0 - a.0) / (z.1 - a.1) }
}
impl Axes {
    fn px(&self, bounds: &[f64], x: f64) -> f64 {
        let (b0, bk) = (bounds[0], bounds[bounds.len() - 1]);
        self.bx[0] + (x - b0) / (bk - b0) * (self.bx[self.bx.len() - 1] - self.bx[0])
    }
    fn py(&self, v: f64) -> f64 { self.y0 + v * self.ypx_per }
}

/// The curve, the points and every guide checked against the recomputed numbers.
fn check_geometry(g: &Svg, p: &Params, key: bool, what: &str) {
    let m = model(p).unwrap();
    let rows: Vec<(f64, f64, f64)> = m.cs.iter().map(|c| (c.lo, c.hi, c.f as f64)).collect();
    let bounds: Vec<f64> = std::iter::once(rows[0].0).chain(rows.iter().map(|r| r.1)).collect();
    let ax = axes_of(g, &bounds);
    let n: f64 = rows.iter().map(|r| r.2).sum();
    let scale = match p.s("y_axis") { Y_REL => 1.0 / n, Y_PCT => 100.0 / n, _ => 1.0 };
    let cum = running(&rows.iter().map(|r| r.2).collect::<Vec<_>>());
    let curve = g.plines.iter().find(|l| l.class == "curve");
    let drawn = key || p.b("student_curve");
    assert_eq!(curve.is_some(), drawn, "{what}: curve shown");
    let tol = 0.011 + 2e-9 * (ax.bx[ax.bx.len() - 1] - ax.bx[0]);
    if let Some(c) = curve {
        assert_eq!(c.pts.len(), bounds.len());
        for (i, q) in c.pts.iter().enumerate() {
            let want = (ax.px(&bounds, bounds[i]), ax.py(if i == 0 { 0.0 } else { cum[i - 1] } * scale));
            assert!((q.0 - want.0).abs() < 0.03 && (q.1 - want.1).abs() < 0.03, "{what}: point {i} {q:?} vs {want:?}");
            assert!((q.0 - ax.bx[i]).abs() < tol, "{what}: point {i} is on its boundary tick");
        }
        assert!((c.pts[0].1 - ax.bottom).abs() < 0.02, "{what}: starts on the axis at 0");
        assert!((ax.py(n * scale) - c.pts[c.pts.len() - 1].1).abs() < 0.03, "{what}: ends at the total");
        assert!(c.pts.windows(2).all(|w| w[1].1 <= w[0].1 + 1e-9 && w[1].0 > w[0].0), "{what}: rises left to right");
        assert!(c.dash.is_none());
    }
    let guides: Vec<&Pl> = g.plines.iter().filter(|l| l.class == "guide").collect();
    if !key {
        assert!(guides.is_empty());
        return;
    }
    assert_eq!(guides.len(), m.readings.len(), "{what}");
    let c = curve.unwrap();
    for (gd, r) in guides.iter().zip(&m.readings) {
        assert_eq!(gd.pts.len(), 3);
        let (start, corner, foot) = (gd.pts[0], gd.pts[1], gd.pts[2]);
        assert!((start.0 - ax.left).abs() < 0.011, "{what}: guide starts on the vertical axis");
        assert!((foot.1 - ax.bottom).abs() < 0.011, "{what}: guide ends on the horizontal axis");
        assert!(start.1 == corner.1 && corner.0 == foot.0, "{what}: across then down");
        let on = c.pts.windows(2).map(|w| seg_dist(corner, w[0], w[1])).fold(f64::INFINITY, f64::min);
        assert!(on < 0.02, "{what}: the guide meets the curve ({on})");
        // the reading recomputed independently
        let value_read = p.s("read") == READ_VALUE && r.foot == plain(p.f("value")).replace('-', "\u{2212}") && r.side != "M";
        let (x, t) = if value_read && !matches!(r.short.as_str(), "Q1" | "M" | "Q3") { (p.f("value"), c_of(&rows, p.f("value"))) } else { let t = r.pct / 100.0 * n; (inverse(&rows, t), t) };
        assert!((corner.0 - ax.px(&bounds, x)).abs() < 0.03 && (corner.1 - ax.py(t * scale)).abs() < 0.03, "{what}: guide at {corner:?}, want x {x} t {t}");
        assert!(gd.dash.is_some(), "{what}: guides are dashed, the curve solid");
    }
    // each label sits beside its own guide: nearer it than any other guide
    let leaders: Vec<&Ln> = g.lines.iter().filter(|l| l.class == "leader").collect();
    for t in g.texts.iter().filter(|t| t.class == "side" || t.class == "foot") {
        let mine = guides.iter().zip(&m.readings).filter(|(_, r)| if t.class == "side" { r.side == t.body || r.side_short == t.body } else { r.foot == t.body || r.short == t.body }).map(|(gd, _)| gd).collect::<Vec<_>>();
        assert!(!mine.is_empty(), "{what}: {:?} belongs to no reading", t.body);
        let leg = |gd: &Pl| { let (a, k, z) = (gd.pts[0], gd.pts[1], gd.pts[2]); if t.class == "side" { seg_box(a, k, t.ink) } else { seg_box(k, z, t.ink) } };
        let own = mine.iter().map(|gd| leg(gd)).fold(f64::INFINITY, f64::min);
        // the box keeps OWN_GAP from its line; digits have no descenders, so the ink may sit up to 0.3 em further
        if own > OWN_GAP + 0.3 * t.size + 1.0 {
            // no room beside the guide: a short leader ties the label to it, and the leader must be
            // short and must not pass a rival guide (or the label reads as that guide's)
            let lead = leaders.iter().filter(|l| seg_box(l.a, l.b, t.ink) <= OWN_GAP + 0.3 * t.size + 1.0).min_by(|a, b| seg_box(a.a, a.b, t.ink).total_cmp(&seg_box(b.a, b.b, t.ink)));
            let lead = lead.unwrap_or_else(|| panic!("{what}: {:?} sits {own:.1} px from its guide and has no leader", t.body));
            let len = (lead.b.0 - lead.a.0).hypot(lead.b.1 - lead.a.1);
            assert!(len <= 64.0, "{what}: a {len:.1} px leader");
            let joins = mine.iter().any(|gd| {
                let segs = [(gd.pts[0], gd.pts[1]), (gd.pts[1], gd.pts[2])];
                let near = |q: P| segs.iter().map(|(a, b)| seg_dist(q, *a, *b)).fold(f64::INFINITY, f64::min);
                near(lead.a) <= PLATE + 2.0 || near(lead.b) <= PLATE + 2.0
            });
            assert!(joins, "{what}: the leader of {:?} does not reach its own guide", t.body);
            for gd in &guides {
                let all = gd.pts.windows(2).map(|w| seg_box(w[0], w[1], t.ink)).fold(f64::INFINITY, f64::min);
                assert!(all >= own - 1e-9, "{what}: {:?} is nearer another guide ({all:.1} vs own {own:.1})", t.body);
            }
        } else {
            assert!(own >= OWN_GAP - 1.0, "{what}: {:?} crowds its own guide ({own:.1} px)", t.body);
        }
        // a vertical-axis label always sits above its line (one rule for every guide, so a reader never has to guess)
        if t.class == "side" {
            let line_y = guides.iter().zip(&m.readings).find(|(_, r)| r.side == t.body || r.side_short == t.body).unwrap().0.pts[0].1;
            assert!(t.ink.3 < line_y, "{what}: {:?} is below its line", t.body);
        }
        // every label is nearest its own line, raised ones included
        for gd in &guides {
            let all = gd.pts.windows(2).map(|w| seg_box(w[0], w[1], t.ink)).fold(f64::INFINITY, f64::min);
            assert!(all >= own - 1e-9, "{what}: {:?} is nearer another guide", t.body);
        }
    }
}

fn datasets() -> Vec<(&'static str, Vec<(String, Value)>)> {
    let sixteen: String = (0..16).map(|i| format!("{}, {}, {}", i as f64 * 2.5, (i + 1) as f64 * 2.5, [3, 0, 7, 12, 1, 0, 0, 9][i % 8])).collect::<Vec<_>>().join("\n");
    vec![
        ("default", vec![]),
        ("unequal", vec![s("table", "0, 10, 4\n10, 20, 9\n20, 40, 14\n40, 60, 8\n60, 100, 5"), f("value", 30.0)]),
        ("flat", vec![s("table", "0, 5, 0\n5, 10, 0\n10, 15, 6\n15, 20, 0\n20, 25, 4\n25, 30, 0"), f("value", 12.0)]),
        ("sixteen", vec![s("table", &sixteen), f("value", 21.0)]),
        ("raw", vec![s("input", RAW), f("value", 81.0)]),
        ("raw small", vec![s("input", RAW), s("data", &data::column_text(&data::generate(data::Dist::Normal, 300, 7, 0.0, 0.004, 4))), f("value", 0.0)]),
        ("big", vec![s("table", "1000000, 2000000, 999999937\n2000000, 3000000, 3\n3000000, 4000000, 400000000"), f("value", 2500000.0)]),
        ("one", vec![s("table", "10, 20, 1"), f("value", 15.0)]),
    ]
}

#[test]
fn nothing_collides_and_every_guide_ends_on_the_curve_and_the_axes_over_the_option_space() {
    let mut n = 0;
    for (name, base) in datasets() {
        for y in [Y_FREQ, Y_REL, Y_PCT] {
            for read in ["None", READ_PCT, READ_VALUE] {
                for marks in ["None", "Median", "Median and quartiles"] {
                    for table in [true, false] {
                        for size in ["Small", "Medium", "Large"] {
                            for (key, curve) in [(true, true), (false, true), (false, false)] {
                                let mut l = base.clone();
                                l.extend([s("y_axis", y), s("read", read), s("marks", marks), b("show_table", table), s("label_size", size), b(SHOW_ANSWERS, key), b("student_curve", curve)]);
                                let p = with(&l);
                                let what = format!("{name} {y} {read} {marks} table={table} {size} key={key} curve={curve}");
                                let g = audit(&render(&p), key, &what);
                                check_geometry(&g, &p, key, &what);
                                n += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    println!("ogive: {n} renders audited");
}

#[test]
fn every_percentile_draws_cleanly_at_every_size() {
    for size in ["Small", "Medium", "Large"] {
        for marks in ["None", "Median and quartiles"] {
            for (name, base) in datasets().into_iter().take(4) {
                for k in (1..100).step_by(2) {
                    let mut l = base.clone();
                    l.extend([s("read", READ_PCT), f("pct", k as f64), s("marks", marks), s("label_size", size)]);
                    let p = with(&l);
                    let what = format!("{name} P{k} {marks} {size}");
                    let g = audit(&render(&p), true, &what);
                    check_geometry(&g, &p, true, &what);
                }
            }
        }
    }
}

#[test]
fn a_guide_with_no_room_for_a_label_is_noted_only_with_a_step_that_labels_it() {
    let (mut noted, mut crowded) = (0, 0);
    for (name, base) in datasets() {
        for size in ["Small", "Medium", "Large"] {
            for k in (1..100).step_by(3) {
                for read in [READ_PCT, READ_VALUE] {
                    let mut l = base.clone();
                    l.extend([s("read", read), f("pct", k as f64), s("marks", "Median and quartiles"), s("label_size", size), b(SHOW_ANSWERS, false)]);
                    let p = with(&l);
                    let mut key = p.clone();
                    key.set_b(SHOW_ANSWERS, true);
                    let bare = draw(&key).1;
                    let notes = warnings(&p);
                    let note = notes.iter().find(|x| x.contains("no room for a label"));
                    if !bare.is_empty() {
                        crowded += 1;
                    }
                    // the in-plot labels of the key, read back: a guide without one is what the note names.
                    // Two readings can share a label text (P25 and Q1 sit at the same height), so each
                    // text is matched to one labelled reading rather than to a name.
                    let g = parse(&render(&key), false);
                    let m = model(&key).unwrap();
                    let mut avail: Vec<String> = g.texts.iter().filter(|t| t.class == "side" || t.class == "foot").map(|t| t.body.clone()).collect();
                    for r in &m.readings {
                        if bare.contains(&r.short) {
                            continue;
                        }
                        let hit = avail.iter().position(|b| *b == r.side || *b == r.side_short || *b == r.foot || *b == r.short);
                        let hit = hit.unwrap_or_else(|| panic!("{name} {size} P{k}: {} is labelled but has no text", r.short));
                        avail.remove(hit);
                    }
                    assert!(avail.is_empty(), "{name} {size} P{k}: labels that belong to no reading: {avail:?}");
                    if !bare.is_empty() {
                        // a note may be silent when no one-control change can label the named
                        // guide, but a note that fires must leave every guide it names drawn and
                        // now labelled: "set Mark to None" cannot be the fix for the M guide
                        if let Some(note) = note {
                            noted += 1;
                            for b in &bare {
                                assert!(note.contains(b.as_str()), "{note}");
                            }
                            let step = note.split(" To label it, ").nth(1).unwrap_or_else(|| panic!("{name} {size} P{k}: a note with no step: {note}"));
                            let mut q = p.clone();
                            if let Some(sz) = step.strip_prefix("set Label size to ") { q.set_s("label_size", sz.trim_end_matches('.')); }
                            else if let Some(mk) = step.strip_prefix("set Mark to ") { q.set_s("marks", mk.trim_end_matches('.')); }
                            else { panic!("unknown step {step}"); }
                            let qm = model(&q).unwrap();
                            let mut qk = q.clone();
                            qk.set_b(SHOW_ANSWERS, true);
                            let qg = parse(&render(&qk), false);
                            for b in &bare {
                                let r = m.readings.iter().find(|r| r.short == *b).unwrap();
                                assert!(
                                    qm.readings.iter().any(|o| o.short == *b && (o.t - r.t).abs() < 1e-9 && (o.x - r.x).abs() < 1e-9),
                                    "{name} {size} P{k}: \"{step}\" deletes the {b} guide the note names"
                                );
                                assert!(!draw(&q).1.contains(b), "{name} {size} P{k}: {b} is still unlabelled after \"{step}\"");
                                let has = qg.texts.iter().any(|t| (t.class == "side" && (t.body == r.side || t.body == r.side_short)) || (t.class == "foot" && (t.body == r.foot || t.body == r.short)));
                                assert!(has, "{name} {size} P{k}: {b} has no label after \"{step}\"");
                            }
                            assert!(!warnings(&q).iter().any(|x| x.contains("no room")), "{name} {size} P{k}: {step} does not work");
                        }
                    } else {
                        assert!(note.is_none(), "{name} {size} P{k}: a note with nothing unlabelled: {notes:?}");
                    }
                }
            }
        }
    }
    // the critic's case: the M guide at P49 with quartiles, Medium. A note that names a quartile
    // may not offer a Mark change, which deletes the very guide it names (naming P49 instead and
    // dropping the marks is a real fix for P49, and the loop above checks that it is labelled)
    let p = with(&[s("read", READ_PCT), f("pct", 49.0), s("marks", "Median and quartiles"), s("label_size", "Medium")]);
    if let Some(note) = warnings(&p).into_iter().find(|n| n.contains("no room")) {
        let marks_named = [" for M ", " for Q1 ", " for Q3 "].iter().any(|n| note.contains(n));
        assert!(!marks_named || !note.contains("set Mark to"), "the step deletes the guide it names: {note}");
    }
    println!("ogive: {noted} notes, all with a working step, out of {crowded} crowded keys");
}

#[test]
fn a_label_is_never_tied_to_another_guides_line() {
    // the critic's case (unequal classes, value 13, quartiles, Large) plus the neighbouring sizes:
    // a label may sit above its own guide only while its own guide is the nearest one
    let table = "0, 10, 4\n10, 20, 9\n20, 40, 14\n40, 60, 8\n60, 100, 5";
    for value in [13.0, 30.0, 75.0] {
        for size in ["Small", "Medium", "Large"] {
            let p = with(&[s("table", table), s("read", READ_VALUE), f("value", value), s("marks", "Median and quartiles"), s("label_size", size)]);
            let g = parse(&render(&p), true);
            let m = model(&p).unwrap();
            let guides: Vec<&Pl> = g.plines.iter().filter(|l| l.class == "guide").collect();
            assert_eq!(guides.len(), m.readings.len());
            // a leader that ties a label to its guide stays short
            for l in g.lines.iter().filter(|l| l.class == "leader") {
                let len = (l.b.0 - l.a.0).hypot(l.b.1 - l.a.1);
                assert!(len <= 64.0, "{value} {size}: a {len:.1} px leader");
            }
            for t in g.texts.iter().filter(|t| t.class == "side" || t.class == "foot") {
                let leg = |gd: &Pl| { let (a, k, z) = (gd.pts[0], gd.pts[1], gd.pts[2]); if t.class == "side" { seg_box(a, k, t.ink) } else { seg_box(k, z, t.ink) } };
                let mine: Vec<f64> = guides.iter().zip(&m.readings).filter(|(_, r)| if t.class == "side" { r.side == t.body || r.side_short == t.body } else { r.foot == t.body || r.short == t.body }).map(|(gd, _)| leg(gd)).collect();
                assert!(!mine.is_empty(), "{value} {size}: {:?} belongs to no reading", t.body);
                let own = mine.iter().cloned().fold(f64::INFINITY, f64::min);
                for (gd, r) in guides.iter().zip(&m.readings) {
                    let other = leg(gd);
                    if other + 1e-9 < own {
                        panic!("{value} {size}: \"{}\" is {other:.1} px from {}'s guide but {own:.1} px from its own", t.body, r.short);
                    }
                }
            }
        }
    }
}

#[test]
fn the_student_copy_is_the_key_with_the_answers_taken_out() {
    let ans = ["guide", "plate", "side", "foot", "tcum", "answer", "leader"];
    for (name, base) in datasets() {
        for marks in ["None", "Median and quartiles"] {
            for read in ["None", READ_PCT, READ_VALUE] {
                for curve in [true, false] {
                    for style in ["Colour", "Print"] {
                        let mut l = base.clone();
                        l.extend([s("marks", marks), s("read", read), b("student_curve", curve), s("style", style)]);
                        let p = with(&l);
                        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
                        let what = format!("{name} {marks} {read} curve={curve} {style}");
                        let vb = |s: &str| attr(s.split('>').next().unwrap(), "viewBox").unwrap();
                        assert_eq!(vb(&student), vb(&key), "{what}: same size");
                        // element by element: drop the answer parts from the key and the blanks from the student copy
                        let elems = |s: &str, drop: &[&str]| -> Vec<String> {
                            s.split('<').filter(|e| !e.is_empty() && !e.starts_with('/')).filter(|e| { let c = attr(e.split('>').next().unwrap(), "class").unwrap_or_default(); !drop.contains(&c.as_str()) }).map(str::to_string).collect()
                        };
                        let mut drop_key: Vec<&str> = ans.to_vec();
                        if !curve {
                            drop_key.extend(["curve", "point"]);
                        }
                        assert_eq!(elems(&student, &["answer-blank", "blank"]), elems(&key, &drop_key), "{what}");
                        // and no answer text anywhere in the source: title, desc, attributes, comments
                        let m = model(&p).unwrap();
                        let kp = parse(&key, false);
                        let mut secrets: Vec<String> = m.questions.iter().filter_map(|q| q.1.clone()).collect();
                        // read-off values (a given value or the names Q1, M, Q3 are not answers)
                        secrets.extend(m.readings.iter().filter(|r| r.foot.contains('=')).map(|r| r.foot.clone()));
                        for sec in &secrets {
                            assert!(!student.contains(&svg::esc(sec)[..]), "{what}: {sec}");
                        }
                        // a cumulative entry may equal a given number (a frequency, a tick): never more often than the givens
                        let sp = parse(&student, false);
                        for t in kp.texts.iter().filter(|t| t.class == "tcum") {
                            let givens = kp.texts.iter().filter(|u| u.class != "tcum" && u.body == t.body).count();
                            assert!(sp.texts.iter().filter(|u| u.body == t.body).count() <= givens, "{what}: {}", t.body);
                        }
                        for word in ["about", "interpolation)", "<!--"] {
                            assert!(!student.contains(word), "{what}: {word}");
                        }
                        // the description says nothing that is an answer (not even the total, the last cumulative entry)
                        let desc = student.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
                        let given: Vec<String> = vec![m.cs.len().to_string(), plain(m.pts[0].0), plain(m.pts[m.pts.len() - 1].0)];
                        let total = m.n.to_string();
                        assert!(given.contains(&total) || !desc.split(|c: char| !c.is_ascii_digit() && c != '.').any(|tok| tok.trim_end_matches('.') == total), "{what}: {desc}");
                    }
                }
            }
        }
    }
}

#[test]
fn print_is_grey_only_and_holds_up_at_66_percent() {
    let grey = |c: &str| matches!(c, "none" | "#fff" | "#000") || (c.len() == 7 && c.starts_with('#') && c[1..3] == c[3..5] && c[3..5] == c[5..7]);
    for (name, base) in datasets() {
        for key in [true, false] {
            let mut l = base.clone();
            l.extend([s("style", "Print"), s("read", READ_PCT), s("marks", "Median and quartiles"), b(SHOW_ANSWERS, key)]);
            let svg = render(&with(&l));
            for k in ["fill", "stroke"] {
                for v in svg.split(&format!(" {k}=\"")).skip(1).map(|x| x.split('"').next().unwrap()) {
                    assert!(grey(v), "{name}: {k}={v} in Print");
                }
            }
            let png = crate::figures::to_png(&svg, 0.66).unwrap();
            let img = image::load_from_memory(&png).unwrap().to_rgba8();
            let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
            assert!(img.pixels().all(|px| hue(px) == 0), "{name}: colour in a Print render");
            let g = parse(&svg, false);
            // at 66% every text is at least 8.4 px (the Medium tick labels at 0.66 x 0.95 x 14) and every stroke at least 0.66 px
            let small = g.texts.iter().map(|t| t.size).fold(f64::INFINITY, f64::min);
            assert!(small * 0.66 >= 8.4, "{name}: {small}");
            for w in svg.split(" stroke-width=\"").skip(1).map(|x| x.split('"').next().unwrap().parse::<f64>().unwrap()) {
                assert!(w * 0.66 >= 0.66, "{name}: hairline {w}");
            }
            // the guides are dashed black, the curve solid black: told apart without colour
            assert!(g.plines.iter().filter(|l| l.class == "guide").all(|l| l.stroke == "#000" && l.dash.is_some()));
            assert!(g.plines.iter().filter(|l| l.class == "curve").all(|l| l.stroke == "#000" && l.dash.is_none()));
            assert!(g.texts.iter().all(|t| !svg.contains("fill=\"#b4410f\"") && !t.body.is_empty()));
        }
    }
    // Colour and Print draw the same geometry
    let strip = |s: &str| s.split('"').enumerate().filter(|(i, _)| i % 2 == 0).map(|(_, x)| x).collect::<String>();
    for (_, base) in datasets() {
        let mut l = base.clone();
        l.extend([s("read", READ_PCT), s("marks", "Median and quartiles")]);
        let a = render(&with(&l));
        l.push(s("style", "Print"));
        let pr = render(&with(&l));
        assert_eq!(strip(&a), strip(&pr));
        assert_eq!(parse(&a, false).plines.iter().map(|l| l.pts.clone()).collect::<Vec<_>>(), parse(&pr, false).plines.iter().map(|l| l.pts.clone()).collect::<Vec<_>>());
    }
}

/// How a key's answer is read off the grid: fine lines between the labelled ones, a vertical line at
/// every class boundary, and Print greys dark enough to survive a 66 % copy.
#[test]
fn the_grid_is_there_to_read_and_to_plot_against() {
    let p = with(&[s("read", READ_PCT), s("marks", "Median and quartiles")]);
    let g = parse(&render(&p), false);
    let grid: Vec<&Ln> = g.lines.iter().filter(|l| l.class == "grid").collect();
    let vertical: Vec<&&Ln> = grid.iter().filter(|l| (l.a.0 - l.b.0).abs() < 1e-9).collect();
    let horizontal = grid.iter().filter(|l| (l.a.1 - l.b.1).abs() < 1e-9).count();
    // 0 to 50 by 10 labelled, each cut in five fine steps: 4 fine lines between each pair, plus the top
    assert_eq!(horizontal, 20 + 5, "fine lines between the labelled ones: {horizontal}");
    // six class boundaries past the axis, each class cut in five: four fine lines inside every class
    assert_eq!(vertical.iter().filter(|l| l.stroke == GRID_PRINT || l.stroke == chart::GRID).count(), 6, "one line per class boundary");
    assert_eq!(vertical.iter().filter(|l| l.stroke == FINE_GRID).count(), 6 * (MINOR_X - 1), "five x-subdivisions per class");
    // each fine vertical is a fifth of a class, so the key's accepted range is half a division
    let class_x: Vec<f64> = g.lines.iter().filter(|l| l.class == "tick" && l.a.0 == l.b.0).map(|l| l.a.0).collect();
    let mut fx: Vec<f64> = vertical.iter().filter(|l| l.stroke == FINE_GRID).map(|l| l.a.0).collect();
    fx.sort_by(f64::total_cmp);
    let division = (class_x[1] - class_x[0]) / MINOR_X as f64;
    assert!(fx.windows(2).all(|w| { let d = (w[1] - w[0]) / division; (d - 1.0).abs() < 0.01 || (d - 2.0).abs() < 0.01 }), "fine x lines every {division:.2} px: {fx:?}");
    assert!((fx[0] - (class_x[0] + division)).abs() < 0.06, "the first fine line is one division past the axis");
    // the fine lines are the paler kind, so a reader can tell a labelled step from a fine one
    assert_eq!(grid.iter().filter(|l| l.stroke == FINE_GRID).count(), 20 + 6 * (MINOR_X - 1));
    // turn the grid off and none of it is drawn
    let mut off = p.clone();
    off.set_b("grid", false);
    assert_eq!(parse(&render(&off), false).lines.iter().filter(|l| l.class == "grid").count(), 0, "grid=false draws no grid");
    // the student copy keeps the grid (there is nothing to plot on without it)
    let mut q = p.clone();
    q.set_b(SHOW_ANSWERS, false);
    let sg = parse(&render(&q), false);
    assert_eq!(sg.lines.iter().filter(|l| l.class == "grid").count(), grid.len());
    // Print: both greys are darker than 66 % grey, and every gridline leaves dark pixels at 3x
    let lum = |c: &str| u8::from_str_radix(&c[1..3], 16).unwrap() as f64;
    for c in [GRID_PRINT, FINE_PRINT] {
        assert!(lum(c) < 0.66 * 255.0, "{c} is paler than 66 % grey");
    }
    assert!(lum(GRID_PRINT) < lum(FINE_PRINT), "the labelled lines are darker than the fine ones");
    for (name, base) in datasets().into_iter().take(4) {
        let mut l = base.clone();
        l.extend([s("style", "Print"), s("read", READ_PCT), s("marks", "Median and quartiles")]);
        let svg = render(&with(&l));
        let gp = parse(&svg, false);
        let (w, _h, px) = crate::figures::to_rgba(&svg, 2.0).unwrap();
        let mut seen = 0;
        let mut missing = 0;
        for line in gp.lines.iter().filter(|l| l.class == "grid" && (l.a.0 - l.b.0).abs() > 1e-9) {
            seen += 1;
            let dark_at = |x: i32, y: i32| {
                if x < 0 || y < 0 { return false; }
                let o = ((y as u32 * w + x as u32) * 4) as usize;
                o + 3 <= px.len() && (0.299 * px[o] as f64 + 0.587 * px[o + 1] as f64 + 0.114 * px[o + 2] as f64) < 0.66 * 255.0
            };
            let (x0, y0, x1, y1) = (line.a.0 * 2.0, line.a.1 * 2.0, line.b.0 * 2.0, line.b.1 * 2.0);
            let n = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt().ceil().max(1.0) as i32;
            let mut dark = 0;
            for i in (0..n).step_by(5) {
                let f = i as f64 / n as f64;
                let (x, y) = ((x0 + (x1 - x0) * f).round() as i32, (y0 + (y1 - y0) * f).round() as i32);
                if dark_at(x, y) || dark_at(x + 1, y) || dark_at(x, y + 1) || dark_at(x - 1, y) || dark_at(x, y - 1) {
                    dark += 1;
                }
            }
            if dark == 0 {
                missing += 1;
            }
        }
        assert!(seen >= 20 && missing == 0, "{name}: {missing} of {seen} gridlines vanish at 66 % grey");
    }
}

#[test]
fn every_character_drawn_is_in_the_font() {
    // resvg swaps a whole line to another face when one glyph is missing (it did, for the approx and
    // less-or-equal signs), so every character the figure can print must be in Noto Sans
    let db = crate::figures::fontdb();
    let id = db.query(&resvg::usvg::fontdb::Query { families: &[resvg::usvg::fontdb::Family::Name("Noto Sans")], ..Default::default() }).expect("Noto Sans is loaded");
    let mut chars = std::collections::BTreeSet::new();
    for (_, base) in datasets() {
        for read in [READ_PCT, READ_VALUE] {
            for closed in [LEFT_CLOSED, RIGHT_CLOSED] {
                for y in [Y_FREQ, Y_REL, Y_PCT] {
                    let mut l = base.clone();
                    l.extend([s("read", read), s("marks", "Median and quartiles"), s("closed", closed), s("y_axis", y), s("title", "Scores"), s("y_title", "")]);
                    for key in [true, false] {
                        l.push(b(SHOW_ANSWERS, key));
                        for t in parse(&render(&with(&l)), false).texts {
                            chars.extend(t.body.chars());
                        }
                    }
                }
            }
        }
    }
    for p in [with(&[s("table", "40, 49, 3\n50, 59, 7")]), with(&[s("table", "90+, 4")]), with(&[s("input", RAW), s("data", "-5, -1"), b("auto_classes", false), f("class_start", 0.0)])] {
        chars.extend(warnings(&p).concat().chars());
    }
    let missing: Vec<char> = db.with_face_data(id, |d, i| { let face = ttf_parser::Face::parse(d, i).unwrap(); chars.iter().filter(|c| face.glyph_index(**c).is_none()).copied().collect() }).unwrap();
    assert!(missing.is_empty(), "not in Noto Sans: {missing:?}");
}

#[test]
fn label_sizes_scale_the_text() {
    let size_of = |sz: &str| parse(&render(&with(&[s("label_size", sz), s("read", READ_PCT)])), false).texts.iter().find(|t| t.class == "xtick").unwrap().size;
    assert_eq!((size_of("Small"), size_of("Medium"), size_of("Large")), (11.2, 14.0, 19.6));
}

#[test]
fn odd_input_never_panics() {
    let mut rng = data::Rng::new(7);
    let pieces = ["", "1", "-1", "0", "1e12", "1e13", "NaN", "inf", "-inf", "1e400", "\u{2212}5", "5%", "7.5", "x", "\u{1F600}", "\u{4E2D}\u{6587}", "+", "90+", "under", "-", "--", ",", ";", "\t", " ", "\n", "\r\n", "\u{0}", "<", "&", "\"", "10-20", "\u{2013}"];
    let word = |rng: &mut data::Rng, n: usize| -> String { (0..n).map(|_| pieces[(rng.unit() * pieces.len() as f64) as usize % pieces.len()]).collect::<Vec<_>>().join(if rng.unit() < 0.5 { ", " } else { " " }) };
    for i in 0..10_000 {
        let mut p = defaults();
        let len = 1 + (rng.unit() * 12.0) as usize;
        let table = (0..len).map(|_| { let n = (rng.unit() * 5.0) as usize; word(&mut rng, n) }).collect::<Vec<_>>().join("\n");
        p.set_s("table", &table);
        let d = word(&mut rng, len * 3);
        p.set_s("data", &d);
        let t = word(&mut rng, 3);
        p.set_s("title", &t);
        let t = word(&mut rng, 2);
        p.set_s("x_title", &t);
        let t = word(&mut rng, 2);
        p.set_s("y_title", &t);
        p.set_s("input", if i % 2 == 0 { GROUPED } else { RAW });
        p.set_b("auto_classes", i % 3 != 0);
        p.set_f("class_width", [0.0, -1.0, 1e-9, 0.01, 1.0, 1e12, f64::NAN][i % 7]);
        p.set_f("class_start", [0.0, f64::NAN, -1e12, 5.0][i % 4]);
        p.set_f("pct", [f64::NAN, -5.0, 0.0, 50.0, 100.0, 1e9][i % 6]);
        p.set_f("value", [f64::NAN, f64::INFINITY, 0.0, 1e12][i % 4]);
        p.set_i("decimals", [-3, 0, 2, 99][i % 4]);
        p.set_s("read", ["None", READ_PCT, READ_VALUE, "junk"][i % 4]);
        p.set_s("marks", ["None", "Median", "Median and quartiles", "junk"][(i / 4) % 4]);
        p.set_s("y_axis", [Y_FREQ, Y_REL, Y_PCT, "junk"][(i / 3) % 4]);
        p.set_b(SHOW_ANSWERS, i % 5 != 0);
        let svg = render(&p);
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{i}");
        for t in parse(&svg, false).texts.iter().filter(|t| !matches!(t.class.as_str(), "title" | "xtitle" | "ytitle" | "refusal")) {
            assert!(!t.body.contains("NaN") && !t.body.contains("inf") && t.body != "?", "{i}: {:?}", t.body);
        }
        let _ = warnings(&p);
        if i % 50 == 0 {
            crate::figures::to_png(&svg, 0.25).unwrap_or_else(|e| panic!("{i}: {e}"));
        }
    }
    // 10,000 rows, and a 200-class table
    let big: String = (0..10_000).map(|i| format!("{i}, {}, 1", i + 1)).collect::<Vec<_>>().join("\n");
    assert!(warnings(&with(&[s("table", &big)]))[0].contains("at most 200 classes"));
    let ok: String = (0..200).map(|i| format!("{i}, {}, {}", i + 1, i % 3)).collect::<Vec<_>>().join("\n");
    let p = with(&[s("table", &ok), s("read", READ_PCT), s("marks", "Median and quartiles"), b("show_table", false)]);
    let g = audit(&render(&p), true, "200 classes");
    check_geometry(&g, &p, true, "200 classes");
    let raw: String = (0..10_000).map(|i| ((i * 37) % 1000).to_string()).collect::<Vec<_>>().join(", ");
    assert!(render(&with(&[s("input", RAW), s("data", &raw)])).contains("class=\"curve\""));
}

/// CPU time this thread has used (utime + stime), so a loaded machine cannot fail the speed test.
fn cpu_now() -> std::time::Duration {
    let ticks = std::fs::read_to_string("/proc/thread-self/stat").ok().and_then(|s| {
        let rest: Vec<&str> = s[s.rfind(')')? + 1..].split_whitespace().collect();
        Some(rest.get(11)?.parse::<u64>().ok()? + rest.get(12)?.parse::<u64>().ok()?)
    });
    match ticks {
        Some(t) => std::time::Duration::from_millis(t * 10),
        None => { thread_local! { static T0: std::time::Instant = std::time::Instant::now(); } T0.with(|t| t.elapsed()) }
    }
}

#[test]
fn drawing_is_pure_and_cheap() {
    let p = with(&[s("read", READ_PCT), s("marks", "Median and quartiles"), s("label_size", "Large")]);
    assert_eq!(render(&p), render(&p));
    let big: String = (0..200).map(|i| format!("{i}, {}, {}", i + 1, i % 3)).collect::<Vec<_>>().join("\n");
    let t0 = cpu_now();
    for i in 0..200 {
        let mut q = p.clone();
        if i % 2 == 0 {
            q.set_s("table", &big);
        }
        q.set_f("pct", (i % 99 + 1) as f64);
        assert!(render(&q).len() > 1000);
        let _ = warnings(&q);
    }
    let cpu = cpu_now() - t0;
    println!("ogive: 200 draws + notes, {cpu:?} CPU");
    assert!(cpu < std::time::Duration::from_secs(20), "{cpu:?}");
}

#[test]
fn controls_examples_and_faq() {
    let mut p = defaults();
    assert!(relevant(ID, "table", &p) && !relevant(ID, "data", &p) && !relevant(ID, "pct", &p) && !relevant(ID, "decimals", &p));
    p.set_s("input", RAW);
    assert!(!relevant(ID, "table", &p) && relevant(ID, "data", &p) && !relevant(ID, "class_width", &p));
    p.set_b("auto_classes", false);
    p.set_s("read", READ_VALUE);
    assert!(relevant(ID, "class_width", &p) && relevant(ID, "value", &p) && !relevant(ID, "pct", &p) && relevant(ID, "decimals", &p));
    assert!(relevant("histogram", "table", &p));
    for e in examples(ID).unwrap() {
        let mut p = defaults();
        for (k, v) in e.params {
            p.0.insert(k.to_string(), v);
        }
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
        let mut key = p.clone();
        key.set_b(SHOW_ANSWERS, true);
        assert!(render(&key).contains("class=\"curve\""), "{}", e.name);
    }
    let faq: String = crate::figures::faq::faq_for(ID).iter().map(|q| format!("{} {}", q.q, q.a)).collect();
    for must in ["upper boundary", "midpoints", "estimate", "straight-line interpolation", "stated limits", "open-ended", "same classes"] {
        assert!(faq.contains(must), "FAQ: {must}");
    }
    // the opening state reads a percentile with nothing to fix
    let start = crate::figures::start_params(&def());
    assert_eq!(start.s("read"), READ_PCT);
    assert!(warnings(&start).is_empty());
    // the registry lists it with the stats figures, on the Math tab
    let d = crate::figures::registry().into_iter().find(|d| d.id == ID).unwrap();
    assert_eq!(d.subject, Subject::Math);
}

#[test]
#[ignore]
fn dump_pngs() {
    let dir = std::env::var("OGIVE_PNG_DIR").unwrap_or_else(|_| "/tmp/ogive-png".into());
    std::fs::create_dir_all(&dir).unwrap();
    let ds = datasets();
    let cases: Vec<(&str, Vec<(String, Value)>)> = vec![
        ("default", vec![]),
        ("start", vec![s("read", READ_PCT)]),
        ("quartiles_pct", vec![s("marks", "Median and quartiles"), s("y_axis", Y_PCT), s("read", READ_PCT)]),
        ("value_rel", vec![s("read", READ_VALUE), s("y_axis", Y_REL)]),
        ("raw", vec![s("input", RAW), s("marks", "Median and quartiles")]),
        ("large_print", vec![s("read", READ_PCT), s("marks", "Median and quartiles"), s("label_size", "Large"), s("style", "Print")]),
        ("small", vec![s("read", READ_PCT), s("label_size", "Small")]),
        ("low_p5", vec![s("read", READ_PCT), f("pct", 5.0)]),
        ("refused", vec![s("table", "40, 49, 3\n50, 59, 7\n60, 69, 2")]),
        ("unequal", vec![s("table", "0, 10, 4\n10, 20, 9\n20, 40, 14\n40, 60, 8\n60, 100, 5"), s("read", READ_VALUE), f("value", 30.0)]),
        ("sixteen_large", vec![ds[3].1[0].clone(), s("read", READ_PCT), f("pct", 90.0), s("marks", "Median and quartiles"), s("label_size", "Large")]),
        ("flat_p60", vec![ds[2].1[0].clone(), s("read", READ_PCT), f("pct", 60.0), s("marks", "Median and quartiles")]),
        ("plot_student", vec![b("student_curve", false), s("read", READ_PCT), f("pct", 75.0)]),
        ("big", vec![ds[6].1[0].clone(), s("read", READ_PCT), s("y_axis", Y_REL)]),
        ("raw_small", vec![ds[5].1[0].clone(), ds[5].1[1].clone(), s("read", READ_VALUE), f("value", 0.0)]),
        ("example_raw", examples(ID).unwrap()[3].params.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()),
    ];
    for (name, list) in cases {
        for key in [true, false] {
            let mut p = with(&list);
            p.set_b(SHOW_ANSWERS, key);
            let svg = render(&p);
            let tag = if key { "key" } else { "student" };
            std::fs::write(format!("{dir}/{name}-{tag}.png"), crate::figures::to_png(&svg, 2.0).unwrap()).unwrap();
            std::fs::write(format!("{dir}/{name}-{tag}.svg"), &svg).unwrap();
            if name == "large_print" {
                std::fs::write(format!("{dir}/{name}-{tag}-66.png"), crate::figures::to_png(&svg, 0.66).unwrap()).unwrap();
            }
        }
    }
}


