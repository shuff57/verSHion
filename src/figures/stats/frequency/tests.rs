//! Tests for the frequency table: every cell recomputed independently from the typed numbers,
//! raw and grouped classes, the histogram's class choice, refusals, the empty box, the student
//! leak scan, Print and resvg ink boxes, a 10,000-input fuzz, purity and CPU time.
use super::*;

fn defaults() -> Params {
    Params::defaults(&params())
}
fn s(k: &'static str, v: &str) -> (&'static str, Value) {
    (k, Value::S(v.to_string()))
}
fn b(k: &'static str, v: bool) -> (&'static str, Value) {
    (k, Value::B(v))
}
fn f(k: &'static str, v: f64) -> (&'static str, Value) {
    (k, Value::F(v))
}
fn with(list: &[(&'static str, Value)]) -> Params {
    let mut p = defaults();
    for (k, v) in list {
        p.0.insert(k.to_string(), v.clone());
    }
    p
}

// ───────────────────────── reading the SVG back ─────────────────────────

fn attr(el: &str, k: &str) -> Option<String> {
    let el = format!(" {el}");
    let at = el.find(&format!(" {k}=\""))? + k.len() + 3;
    el[at..].split('"').next().map(str::to_string)
}
fn num_attr(el: &str, k: &str) -> f64 {
    attr(el, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN)
}
fn unesc(t: &str) -> String {
    t.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&")
}

type Bx = (f64, f64, f64, f64);

#[derive(Clone, Debug)]
struct Txt {
    class: String,
    x: f64,
    y: f64,
    size: f64,
    body: String,
    ink: Bx,
}
#[derive(Clone, Debug)]
struct Ln {
    class: String,
    a: (f64, f64),
    b: (f64, f64),
    width: f64,
}
#[derive(Clone, Debug)]
struct Rc {
    class: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: String,
}

struct G {
    w: f64,
    h: f64,
    texts: Vec<Txt>,
    lines: Vec<Ln>,
    rects: Vec<Rc>,
}

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

fn parse(svg: &str, ink: bool) -> G {
    let root = svg.split('>').next().unwrap();
    let view = attr(root, "viewBox").unwrap_or_default();
    let parts: Vec<f64> = view.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    let (w, h) = if parts.len() == 4 { (parts[2], parts[3]) } else { (num_attr(root, "width"), num_attr(root, "height")) };
    let mut texts = Vec::new();
    for el in svg.split("<text ").skip(1) {
        let head = el.split('>').next().unwrap();
        let body = unesc(el[head.len() + 1..].split("</text>").next().unwrap());
        texts.push(Txt {
            class: attr(head, "class").unwrap_or_default(),
            x: num_attr(head, "x"),
            y: num_attr(head, "y"),
            size: num_attr(head, "font-size"),
            body,
            ink: (0.0, 0.0, 0.0, 0.0),
        });
    }
    if ink {
        let boxes = ink_boxes(svg);
        assert_eq!(boxes.len(), texts.len(), "one ink box per text element");
        for (t, (body, bx)) in texts.iter_mut().zip(boxes) {
            assert_eq!(t.body, body, "a text element and its ink box are in the same order");
            t.ink = bx;
        }
    }
    let lines = svg
        .split("<line ")
        .skip(1)
        .map(|l| {
            let head = l.split('>').next().unwrap();
            Ln {
                class: attr(head, "class").unwrap_or_default(),
                a: (num_attr(head, "x1"), num_attr(head, "y1")),
                b: (num_attr(head, "x2"), num_attr(head, "y2")),
                width: num_attr(head, "stroke-width"),
            }
        })
        .collect();
    let rects = svg
        .split("<rect ")
        .skip(1)
        .map(|r| {
            let head = r.split('>').next().unwrap();
            Rc {
                class: attr(head, "class").unwrap_or_default(),
                x: num_attr(head, "x"),
                y: num_attr(head, "y"),
                w: num_attr(head, "width"),
                h: num_attr(head, "height"),
                fill: attr(head, "fill").unwrap_or_default(),
            }
        })
        .collect();
    G { w, h, texts, lines, rects }
}

fn has_class(t: &Txt, c: &str) -> bool {
    t.class.split_whitespace().any(|x| x == c)
}
fn bodies(g: &G, class: &str) -> Vec<String> {
    g.texts.iter().filter(|t| has_class(t, class) && !has_class(t, "total")).map(|t| t.body.clone()).collect()
}
fn totals(g: &G, class: &str) -> Vec<String> {
    g.texts.iter().filter(|t| has_class(t, class) && has_class(t, "total")).map(|t| t.body.clone()).collect()
}
fn all_text(g: &G) -> Vec<&str> {
    g.texts.iter().map(|t| t.body.as_str()).collect()
}
/// The x positions of the table's vertical rules, left to right.
fn verticals(g: &G) -> Vec<f64> {
    let mut xs: Vec<f64> = g
        .lines
        .iter()
        .filter(|l| l.class == "rule" && (l.a.0 - l.b.0).abs() < 1e-9)
        .map(|l| l.a.0)
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    xs
}

// ───────────────────────── the numbers, written independently ─────────────────────────

/// A class boundary as the table prints it, from its own decimal text (chart-style, 6 places
/// after the 1e-9 snap, exactly as `chart::fmt` does).
fn cfmt(v: f64) -> String {
    let s = format!("{:.6}", (v * 1e9).round() / 1e9);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    let s = if s == "-0" || s.is_empty() { "0" } else { s };
    s.replace('-', "\u{2212}")
}

/// A typed value as the table prints it: shortest round-trip, with a scientific fallback.
fn vfmt(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let plain = format!("{v}");
    let s = if plain.chars().count() > 20 || plain.contains(['e', 'E']) {
        format!("{v:e}")
    } else {
        plain
    };
    s.replace('-', "\u{2212}")
}

/// Distinct values in ascending order and their counts, by sorting and a plain equality scan.
fn recount(values: &[f64]) -> Vec<(String, u64)> {
    let mut v = values.to_vec();
    for x in &mut v {
        if *x == 0.0 {
            *x = 0.0;
        }
    }
    v.sort_by(f64::total_cmp);
    let mut out = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let x = v[i];
        let mut j = i + 1;
        while j < v.len() && v[j] == x {
            j += 1;
        }
        out.push((vfmt(x), (j - i) as u64));
        i = j;
    }
    out
}

fn tgcd(a: u64, b: u64) -> u64 {
    if b == 0 { a.max(1) } else { tgcd(b, a % b) }
}
fn tfrac(f: u64, n: u64) -> String {
    if n == 0 {
        return "\u{2212}".into();
    }
    if f == 0 {
        return "0".into();
    }
    let g = tgcd(f, n);
    let (a, b) = (f / g, n / g);
    if b == 1 { a.to_string() } else { format!("{a}/{b}") }
}
/// Half up, from exact integers: the digit after the last one printed decides, and 5 rounds up.
fn tpct(f: u64, n: u64, d: u32) -> u128 {
    let scale = 10u128.pow(d);
    let num = f as u128 * 100 * scale;
    let q = num / n.max(1) as u128;
    let r = num % n.max(1) as u128;
    q + u128::from(2 * r >= n.max(1) as u128)
}
fn tpct_text(f: u64, n: u64, d: u32) -> String {
    let u = tpct(f, n, d);
    let p = 10u128.pow(d);
    if d == 0 { format!("{u}%") } else { format!("{}.{:0w$}%", u / p, u % p, w = d as usize) }
}
fn tcum(fs: &[u64]) -> Vec<u64> {
    let mut t = 0;
    fs.iter().map(|f| { t += f; t }).collect()
}
/// The decimal proportion c/n at d places, half up, from exact integers.
fn tratio(c: u64, n: u64, d: u32) -> u128 {
    let n = n.max(1) as u128;
    (2 * c as u128 * 10u128.pow(d) + n) / (2 * n)
}
fn tratio_text(c: u64, n: u64, d: u32) -> String {
    let u = tratio(c, n, d);
    let p = 10u128.pow(d);
    if d == 0 { u.to_string() } else { format!("{}.{:0w$}", u / p, u % p, w = d as usize) }
}

/// The histogram's own class choice, recomputed here through the parent module's helpers.
fn expected_groups(values: &[f64]) -> Vec<(String, u64)> {
    let w = super::super::auto_width(values);
    let st = super::super::auto_start(values, w, true);
    let bins = super::super::bins(values, st, w, true).expect("auto classes");
    let n = bins.len();
    bins.iter()
        .enumerate()
        .map(|(i, b)| {
            let label = if i + 1 == n {
                format!("{} to {}", cfmt(b.lo), cfmt(b.hi))
            } else {
                format!("{} to <{}", cfmt(b.lo), cfmt(b.hi))
            };
            (label, b.n as u64)
        })
        .collect()
}

/// Apply the fixed value a refusal/fallback note offers.
fn apply_fix(note: &str, p: &mut Params) -> bool {
    let mut touched = false;
    if note.contains("Turn off Choose the class width automatically") {
        p.set_b("auto_classes", false);
        touched = true;
    }
    if let Some(rest) = note.split("Set Class width to ").nth(1) {
        // a manual width only applies while the automatic choice is off
        p.set_b("auto_classes", false);
        let v: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '\u{2212}')
            .collect();
        if let Ok(x) = v.replace('\u{2212}', "-").parse::<f64>() {
            p.set_f("class_width", x);
            touched = true;
        }
    }
    if let Some(rest) = note.split("Set First class starts at ").nth(1) {
        let v: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '\u{2212}')
            .collect();
        if let Ok(x) = v.replace('\u{2212}', "-").parse::<f64>() {
            p.set_f("class_start", x);
            touched = true;
        }
    }
    if note.contains("Turn on Choose the class width automatically") {
        p.set_b("auto_classes", true);
        touched = true;
    }
    touched
}

fn fix_note(p: &Params) -> Option<String> {
    warnings(p).into_iter().find(|n| n.contains("Set Class width to ") || n.contains("Set First class starts at ") || n.contains("Turn on Choose the class width automatically"))
}

// ───────────────────────── raw values ─────────────────────────

#[test]
fn raw_counts_labels_and_cumulative_values_match_an_independent_recount() {
    let sets = [
        DEFAULT_DATA,
        "1, 1, 2, 3, 3, 3, 4",
        "\u{2212}3, 0, 0, 2.5, 2.5, 10",
        "5",
        "0, -0, 0.0",
        "1.25, 1.25, 1.5, 2, 2, 2, 2",
        "1.0000000001, 1.0000000002, 1.0000000001",
        "1.0000000001e-20, 1.0000000002e-20",
    ];
    for text in sets {
        let p = with(&[s("data", text)]);
        let m = model(&p).unwrap();
        let values = data::numbers(text);
        let want = recount(&values);
        assert_eq!(m.rows.len(), want.len(), "{text}");
        assert_eq!(m.n, values.len() as u64, "{text}");
        for (i, (label, f)) in want.iter().enumerate() {
            assert_eq!(m.rows[i].label, *label, "{text} row {i}");
            assert_eq!(m.rows[i].f, *f, "{text} row {i}");
        }
        let cums = tcum(&want.iter().map(|(_, f)| *f).collect::<Vec<_>>());
        assert_eq!(m.rows.iter().map(|r| r.cum).collect::<Vec<_>>(), cums, "{text}");
        assert_eq!(m.rows.last().unwrap().cum, m.n, "{text}");
        // ascending, one row per distinct value
        let mut sorted = values.clone();
        sorted.sort_by(f64::total_cmp);
        assert_eq!(m.rows.iter().map(|r| r.label.clone()).collect::<Vec<_>>(), recount(&sorted).into_iter().map(|(l, _)| l).collect::<Vec<_>>(), "{text}");
        // the drawn table carries exactly the model's cells
        let g = parse(&render(&p), false);
        assert_eq!(bodies(&g, "tcell"), m.rows.iter().map(|r| r.label.clone()).collect::<Vec<_>>(), "{text}");
        assert_eq!(bodies(&g, "fcell"), m.rows.iter().map(|r| r.f.to_string()).collect::<Vec<_>>(), "{text}");
        assert_eq!(bodies(&g, "ccell"), m.rows.iter().map(|r| r.cum.to_string()).collect::<Vec<_>>(), "{text}");
        assert_eq!(totals(&g, "fcell"), vec![m.n.to_string()], "{text}");
        assert_eq!(totals(&g, "ccell"), vec![m.n.to_string()], "{text}");
        assert_eq!(bodies(&g, "tcell").len(), m.rows.len(), "{text}: one body row per value");
    }
}

#[test]
fn relative_fractions_are_exact_and_in_lowest_terms() {
    let mut checked = 0;
    for n in 1..=40u64 {
        for f in 0..=n {
            assert_eq!(fraction_text(f, n), tfrac(f, n), "{f}/{n}");
            checked += 1;
        }
    }
    assert_eq!(dfrac(12, 18), "2/3");
    assert_eq!(dfrac(20, 20), "1");
    assert_eq!(dfrac(0, 7), "0");
    // the table's own cells agree, and the Total row's fraction is exactly 1
    let p = with(&[s("relative", FRACTION)]);
    let m = model(&p).unwrap();
    let g = parse(&render(&p), false);
    assert_eq!(bodies(&g, "rcell"), m.rows.iter().map(|r| tfrac(r.f, m.n)).collect::<Vec<_>>());
    assert_eq!(totals(&g, "rcell"), vec!["1".to_string()]);
    println!("frequency table: {checked} exact fractions checked");
}
fn dfrac(f: u64, n: u64) -> String {
    fraction_text(f, n)
}

#[test]
fn percents_round_half_up_from_exact_integer_arithmetic_at_every_decimal() {
    let mut checked = 0;
    for n in 1..=31u64 {
        for f in 0..=n {
            for d in 0..=2u32 {
                assert_eq!(pct_scaled(f, n, d), tpct(f, n, d), "{f}/{n} at {d} decimals");
                assert_eq!(pct_text(f, n, d), tpct_text(f, n, d), "{f}/{n} at {d} decimals");
                checked += 1;
            }
        }
    }
    // the ties the critic will try: 12.5, 6.25, 0.5, 0.25 and 66.666...
    assert_eq!(pct_text(1, 8, 0), "13%");
    assert_eq!(pct_text(1, 16, 1), "6.3%");
    assert_eq!(pct_text(1, 200, 0), "1%");
    assert_eq!(pct_text(1, 400, 0), "0%");
    assert_eq!(pct_text(2, 3, 2), "66.67%");
    assert_eq!(pct_text(3, 8, 2), "37.50%");
    assert_eq!(pct_text(5, 7, 2), "71.43%");
    // the table's cells are the same numbers, and the Total row is exactly 100%
    for d in 0..=2 {
        let p = with(&[("decimals", Value::I(d))]);
        let m = model(&p).unwrap();
        let g = parse(&render(&p), false);
        assert_eq!(bodies(&g, "rcell"), m.rows.iter().map(|r| tpct_text(r.f, m.n, d as u32)).collect::<Vec<_>>());
        let exact = 100 * 10u128.pow(d as u32);
        assert_eq!(totals(&g, "rcell"), vec![pct_text_scaled(exact, d as u32)]);
        assert_eq!(pct_scaled(m.n, m.n, d as u32), exact, "n/n is exactly 100%");
    }
    println!("frequency table: {checked} half-up percents checked");
}

#[test]
fn value_text_is_the_shortest_round_trip_and_keeps_tiny_values_short() {
    assert_eq!(value_text(1e-20), "1e\u{2212}20");
    assert_eq!(value_text(1e-300), "1e\u{2212}300");
    assert_eq!(value_text(1.0000000001e-20), "1.0000000001e\u{2212}20");
    assert_eq!(value_text(1.0000000001), "1.0000000001");
    assert_eq!(value_text(-0.0), "0");
    assert_eq!(value_text(1000.0), "1000");
}

#[test]
fn decimal_proportions_and_cumulative_relative_frequency_are_exact() {
    let mut checked = 0;
    for n in 1..=40u64 {
        for c in 0..=n {
            for d in 0..=2u32 {
                assert_eq!(ratio_scaled(c, n, d), tratio(c, n, d), "{c}/{n} at {d} places");
                assert_eq!(ratio_text(c, n, d), tratio_text(c, n, d), "{c}/{n} at {d} places");
                checked += 1;
            }
        }
    }
    assert_eq!(ratio_text(1, 20, 2), "0.05");
    assert_eq!(ratio_text(1, 3, 2), "0.33");
    assert_eq!(ratio_text(20, 20, 2), "1.00");
    assert_eq!(ratio_text(1, 20, 1), "0.1");
    assert_eq!(ratio_text(1, 8, 2), "0.13", "0.125 rounds half up");
    assert_eq!(ratio_text(3, 8, 2), "0.38", "0.375 rounds half up");

    for rel in [FRACTION, DECIMAL, PERCENT] {
        let p = with(&[
            s("mode", GROUPED),
            s("data", DEFAULT_GROUPED),
            s("relative", rel),
            ("decimals", Value::I(2)),
            b("cum_rel", true),
        ]);
        let m = model(&p).unwrap();
        let g = parse(&render(&p), false);
        assert_eq!(bodies(&g, "rcell"), m.rows.iter().map(|r| r.rel.clone()).collect::<Vec<_>>(), "{rel}");
        let want_cr: Vec<String> = m
            .rows
            .iter()
            .map(|r| match rel {
                FRACTION => tfrac(r.cum, m.n),
                DECIMAL => tratio_text(r.cum, m.n, 2),
                PERCENT => tpct_text(r.cum, m.n, 2),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(bodies(&g, "crcell"), want_cr, "{rel}: cumulative relative frequency");
        assert_eq!(totals(&g, "crcell"), vec![want_cr.last().unwrap().clone()], "{rel}: total");
        let head: String = g.texts.iter().filter(|t| has_class(t, "thead")).map(|t| t.body.as_str()).collect::<Vec<_>>().join(" ");
        assert!(head.contains("Cumulative") && head.contains("relative"), "{rel}: {head}");
        // both copies keep the same shape and the student blanks the new column too
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        let (sg, kg) = (parse(&student, false), parse(&key, false));
        assert_eq!(verticals(&sg), verticals(&kg), "{rel}: same column rules");
        assert!(bodies(&sg, "rcell").is_empty() && bodies(&sg, "crcell").is_empty(), "{rel}: student blanks");
    }

    // the OpenStax-style grouped table: 0.10, 0.25, 0.40, 0.20, 0.05 and 0.10..1.00
    let e = examples(ID).unwrap().into_iter().find(|e| e.name == "Heights, decimal proportions").unwrap();
    let mut p = defaults();
    for (k, v) in e.params {
        p.0.insert(k.to_string(), v);
    }
    assert_eq!(p.s("heading"), "Height (cm)");
    assert_eq!(p.s("title"), "Heights of students");
    let m = model(&p).unwrap();
    assert_eq!(m.rows.iter().map(|r| r.f).collect::<Vec<_>>(), [2, 5, 8, 4, 1]);
    assert_eq!(m.rows.iter().map(|r| r.rel.clone()).collect::<Vec<_>>(), ["0.10", "0.25", "0.40", "0.20", "0.05"]);
    assert_eq!(m.rows.iter().map(|r| cum_rel_text(r.cum, m.n, m.rel, m.decimals)).collect::<Vec<_>>(), ["0.10", "0.35", "0.75", "0.95", "1.00"]);
    assert!(warnings(&p).is_empty(), "{:?}", warnings(&p));

    // a Decimal column that rounds to 0.99 is stated, like the Percent column
    let p = with(&[s("data", "1, 2, 3"), s("relative", DECIMAL), ("decimals", Value::I(2))]);
    let m = model(&p).unwrap();
    assert!(m.key_notes.iter().any(|n| n.contains("0.99") && n.contains("exactly 1.00")), "{:?}", m.key_notes);
    let g = parse(&render(&p), false);
    let notes: String = g.texts.iter().filter(|t| has_class(t, "note")).map(|t| t.body.as_str()).collect::<Vec<_>>().join(" ");
    assert!(notes.contains("0.99") && notes.contains("exactly 1.00"), "{notes}");
    let (student, _) = crate::figures::answer_pair(&def(), &p).unwrap();
    assert!(!student.contains("0.99"), "the key note is not in the student copy");
    println!("frequency table: {checked} decimal proportions checked");
}

#[test]
fn the_total_row_adds_to_n_and_to_exactly_one_hundred_percent() {
    for text in [DEFAULT_DATA, "1, 2, 2, 3, 3, 3, 7", "5", "\u{2212}2, \u{2212}2, 0, 4"] {
        for rel in [OFF, FRACTION, DECIMAL, PERCENT] {
            for total in [true, false] {
                let p = with(&[s("data", text), s("relative", rel), b("total", total)]);
                let m = model(&p).unwrap();
                let g = parse(&render(&p), false);
                assert_eq!(m.rows.iter().map(|r| r.f).sum::<u64>(), m.n, "{text}");
                if total {
                    assert_eq!(totals(&g, "fcell"), vec![m.n.to_string()], "{text} {rel}");
                    assert_eq!(totals(&g, "ccell"), vec![m.n.to_string()], "{text} {rel}");
                    assert_eq!(
                        totals(&g, "crcell"),
                        if m.cum_rel { vec![cum_rel_text(m.n, m.n, m.rel, m.decimals)] } else { Vec::new() },
                        "{text} {rel}"
                    );
                    match rel {
                        FRACTION => assert_eq!(totals(&g, "rcell"), vec!["1".to_string()], "{text}"),
                        DECIMAL => assert_eq!(totals(&g, "rcell"), vec![ratio_text(m.n, m.n, m.decimals)], "{text}"),
                        PERCENT => {
                            let d = m.decimals;
                            assert_eq!(totals(&g, "rcell"), vec![pct_text_scaled(100 * 10u128.pow(d), d)], "{text}");
                        }
                        _ => assert!(totals(&g, "rcell").is_empty(), "{text}: no relative column"),
                    }
                } else {
                    assert!(totals(&g, "fcell").is_empty() && totals(&g, "ccell").is_empty(), "{text}: no Total row");
                    assert!(totals(&g, "crcell").is_empty(), "{text}: no cumulative relative total");
                }
            }
        }
    }
    // a rounded column that does not add to 100 is stated, never hidden
    let p = with(&[s("data", "1, 2, 3"), s("relative", PERCENT), ("decimals", Value::I(0))]);
    let m = model(&p).unwrap();
    let printed: u128 = m.rows.iter().map(|r| pct_scaled(r.f, m.n, 0)).sum();
    assert_ne!(printed, 100, "33 + 33 + 33: the rounded cells do not add to 100");
    assert!(m.key_notes.iter().any(|n| n.contains("99%") && n.contains("Total row is exactly 100%")), "{:?}", m.key_notes);
    let g = parse(&render(&p), false);
    let notes: String = g.texts.iter().filter(|t| has_class(t, "note")).map(|t| t.body.as_str()).collect::<Vec<_>>().join(" ");
    assert!(notes.contains("99%") && notes.contains("exactly 100%"), "{notes}");
    assert!(!notes.contains("%%"), "one percent sign only: {notes}");
    assert!(!m.key_notes.iter().any(|n| n.contains("%%")), "{:?}", m.key_notes);

    // many rounded cells can miss 100 by more than one point; the note gives the actual sum
    let fifteen: String = (1..=15).map(|i| i.to_string()).collect::<Vec<_>>().join(", ");
    let p = with(&[s("data", &fifteen)]);
    let m = model(&p).unwrap();
    assert!(m.key_notes.iter().any(|n| n.contains("100.5%")), "{:?}", m.key_notes);
}

// ───────────────────────── grouped classes ─────────────────────────

#[test]
fn grouped_auto_classes_are_the_histograms_own_and_cover_every_value_once() {
    let sets = [
        DEFAULT_GROUPED,
        DEFAULT_DATA,
        "0, 0, 0, 1, 1, 2, 9, 10, 10, 30, 41, 41, 41, 100",
        "\u{2212}5.5, \u{2212}1, 0, 0.25, 0.25, 3.75, 8.5",
        "1000000, 2000000, 2500000, 3000000",
    ];
    for text in sets {
        let p = with(&[s("mode", GROUPED), s("data", text)]);
        let m = model(&p).unwrap();
        let values = data::numbers(text);
        let want = expected_groups(&values);
        assert_eq!(m.rows.len(), want.len(), "{text}");
        for (i, (label, f)) in want.iter().enumerate() {
            assert_eq!(m.rows[i].label, *label, "{text} class {i}");
            assert_eq!(m.rows[i].f, *f, "{text} class {i}");
        }
        assert_eq!(m.rows.iter().map(|r| r.f).sum::<u64>(), values.len() as u64, "{text}");
        assert_eq!(m.n, values.len() as u64, "{text}");
        assert!(m.edge.as_ref().unwrap().contains("a is included and b is not"), "{text}");
        assert!(m.edge.as_ref().unwrap().contains("wide, starting at"), "{text}");
        assert!(m.edge.as_ref().unwrap().contains("The last class is a to b"), "{text}");
        assert!(m.rows.iter().take(m.rows.len() - 1).all(|r| r.label.contains(" to <")), "{text}: the a to <b convention");
        assert!(!m.rows.last().unwrap().label.contains('<'), "{text}: the last class is closed");
    }
    println!("frequency table: {} grouped data sets match the histogram", sets.len());
}

#[test]
fn grouped_boundaries_print_at_the_widths_precision_and_never_repeat() {
    // the blocker: auto width 1e-7 used to print 0 to <0, 0 to <0, 0 to 0 and a note saying 0 wide
    let tiny = with(&[s("mode", GROUPED), s("data", "0.0000001, 0.0000002, 0.0000003")]);
    let m = model(&tiny).unwrap();
    let labels: Vec<&str> = m.rows.iter().map(|r| r.label.as_str()).collect();
    assert!(labels.iter().all(|l| *l != "0 to <0" && *l != "0 to 0"), "{labels:?}");
    assert!(labels.iter().any(|l| l.contains("0.0000001")), "{labels:?}");
    assert!(labels.windows(2).all(|w| w[0] != w[1]), "adjacent labels repeat: {labels:?}");
    assert_eq!(m.rows.iter().map(|r| r.f).collect::<Vec<_>>(), vec![1, 1, 1]);
    let edge = m.edge.unwrap();
    assert!(edge.contains("0.0000001 wide"), "{edge}");
    assert!(!edge.contains("are 0 wide"), "{edge}");
    assert!(!edge.contains("upper boundary 0,"), "{edge}");

    // huge, decimal, negative/decimal, and sub-1e-19 classes: all labels distinct, all notes precise
    let cases: Vec<(&str, Option<f64>, Option<f64>)> = vec![
        ("1000000000, 1000000010, 1000000020", None, None),
        ("0.1, 0.2, 0.3, 0.4", None, None),
        ("\u{2212}1, \u{2212}0.5, 0, 0.5, 1.5", Some(0.25), Some(-1.0)),
        ("1e-20, 2e-20, 3e-20", None, None),
        ("1.2345, 1.2346, 1.2347", None, None),
        ("0.5, 10.5, 20.5", Some(10.0), Some(0.5)),
    ];
    for (data_text, width, start) in cases {
        let mut l = vec![s("mode", GROUPED), s("data", data_text)];
        if let Some(w) = width {
            l.push(b("auto_classes", false));
            l.push(f("class_width", w));
        }
        if let Some(st) = start {
            l.push(f("class_start", st));
        }
        let p = with(&l);
        let m = model(&p).unwrap();
        let labels: Vec<&str> = m.rows.iter().map(|r| r.label.as_str()).collect();
        assert!(labels.windows(2).all(|w| w[0] != w[1]), "{data_text}: repeated labels {labels:?}");
        let edge = m.edge.unwrap();
        assert!(edge.contains("wide, starting at"), "{data_text}: {edge}");
        // the printed boundaries are not rounded to a false zero
        assert!(!labels.iter().any(|l| l == &"0 to <0" || l == &"0 to 0"), "{data_text}: {labels:?}");
    }
    // an integer width with a decimal start keeps the start's decimal
    let p = with(&[
        s("mode", GROUPED),
        s("data", "0.5, 10.5, 20.5"),
        b("auto_classes", false),
        f("class_width", 10.0),
        f("class_start", 0.5),
    ]);
    let m = model(&p).unwrap();
    assert_eq!(m.rows[0].label, "0.5 to <10.5", "{:?}", m.rows);
    assert!(m.edge.unwrap().contains("starting at 0.5"), "the start is printed");
}

#[test]
fn grouped_tiny_differences_fall_back_to_a_width_that_really_resolves_the_boundaries() {
    // the MED repro: auto width near 1 is below one ulp, so boundaries collapse; the old fallback
    // retried the same width and produced no table while advising the switch that was already on
    for (label, one, two, three) in [
        ("near 1", "1.0000000000000002", "1.0000000000000004", "1.0000000000000007"),
        ("near 1, more", "1.0000000000000002", "1.0000000000000009", "1.0000000000000013"),
        ("near 1e9", "1000000000.0000001", "1000000000.0000002", "1000000000.0000004"),
    ] {
        let data_text = std::iter::repeat([one, two, three]).take(10).flatten().collect::<Vec<_>>().join(", ");
        let p = with(&[s("mode", GROUPED), s("data", &data_text)]);
        let notes = warnings(&p).join(" ");
        assert!(!notes.contains("could not be made"), "{label}: no table: {notes}");
        let m = model(&p).expect(label);
        assert!(!m.rows.is_empty(), "{label}: rows");
        assert!(m.rows.len() > 1, "{label}: the distribution is preserved, not one class: {:?}", m.rows);
        assert_eq!(m.rows.iter().map(|r| r.f).sum::<u64>(), 30, "{label}: every value counted");
        assert!(notes.contains("finer width") || notes.contains("cannot hold") || notes.contains("0.01"), "{label}: a resolving width is named: {notes}");
        assert!(notes.contains("distinct boundaries") || notes.contains("more than 200 classes"), "{label}: the reason is named: {notes}");
        assert!(!notes.contains("Turn on Choose the class width automatically"), "{label}: the switch is already on: {notes}");
        // if the note offers a manual width, applying it draws the same table
        if notes.contains("Set Class width to") {
            let mut fixed = p.clone();
            assert!(apply_fix(&notes, &mut fixed), "{label}: a fix to apply");
            let mf = model(&fixed).expect(label);
            assert_eq!(mf.rows.iter().map(|r| r.f).sum::<u64>(), 30, "{label}: fixed counts");
            assert!(!warnings(&fixed).iter().any(|n| n.contains("distinct boundaries")), "{label}: {:?}", warnings(&fixed));
        }
    }
}

#[test]
fn the_widening_fallback_tries_a_failing_candidate_first_and_grows() {
    // the mutant candidate *= 10.0 -> *= 1.0 must fail: the first candidate is one tenth of a
    // representable step and cannot print distinct boundaries, so only growth resolves it
    let values = data::numbers("1.0000000000000002, 1.0000000000000004, 1.0000000000000007");
    let start = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let (bins, w) = resolving_width(&values, start, 1e-20).expect("the loop grows to a resolving width");
    assert!(w > fine_width(&values) / 5.0, "the first failing candidate was grown: {w}");
    assert_eq!(bins.iter().map(|b| b.n as u64).sum::<u64>(), values.len() as u64);
    assert!(bins.iter().filter(|b| b.n > 0).count() > 1, "the distribution survives: {bins:?}");
}

#[test]
fn tiny_ranges_use_a_relative_width_and_do_not_fill_the_table_with_empty_rows() {
    // the MED repro: an absolute +1e-12 in the shared nice step made a 2e-20 range 21 classes
    let p = with(&[s("mode", GROUPED), s("data", "1e-20, 2e-20, 3e-20")]);
    let m = model(&p).unwrap();
    assert_eq!(m.rows.len(), 3, "three values, three one-wide classes: {:?}", m.rows);
    assert!(m.rows.iter().all(|r| r.f > 0), "no empty rows: {:?}", m.rows);
    assert_eq!(m.rows.iter().map(|r| r.f).sum::<u64>(), 3);
    let edge = m.edge.unwrap();
    assert!(edge.contains("0.00000000000000000001 wide"), "{edge}");
    assert!(warnings(&p).iter().any(|n| n.contains("relative-tolerance step") && n.contains("histogram")), "{:?}", warnings(&p));

    // 5e-13 to 1.5e-12: three classes, not the eleven the absolute epsilon produced
    let p = with(&[s("mode", GROUPED), s("data", "5e-13, 1.5e-12")]);
    let m = model(&p).unwrap();
    assert!(m.rows.len() <= 4, "a relative width, not eleven rows: {:?}", m.rows);
    assert_eq!(m.rows.iter().map(|r| r.f).sum::<u64>(), 2);
    assert_eq!(m.rows.iter().filter(|r| r.f > 0).count(), 2, "{:?}", m.rows);

    // a normal range is unchanged and still uses the histogram's own width and classes
    let values = data::numbers(DEFAULT_GROUPED);
    let p = with(&[s("mode", GROUPED), s("data", DEFAULT_GROUPED)]);
    let m = model(&p).unwrap();
    let w = super::super::auto_width(&values);
    let st = super::super::auto_start(&values, w, true);
    let shared = super::super::bins(&values, st, w, true).unwrap();
    assert_eq!(m.rows.iter().map(|r| r.f).collect::<Vec<_>>(), shared.iter().map(|b| b.n as u64).collect::<Vec<_>>());
    assert_eq!(m.rows.len(), 5);
    // the local relative step is not the absolute-epsilon step on a tiny range
    let tiny = [1e-20, 2e-20, 3e-20];
    assert!((auto_width_relative(&tiny) - 1e-20).abs() < 1e-35, "{}", auto_width_relative(&tiny));
    assert!(super::super::auto_width(&tiny) < 5e-21, "the shared helper really is a decade small: {}", super::super::auto_width(&tiny));
}

#[test]
fn grouped_table_prints_the_edge_rule_in_the_header_row() {
    let p = with(&[s("mode", GROUPED), s("data", DEFAULT_GROUPED)]);
    let m = model(&p).unwrap();
    let g = parse(&render(&p), false);
    let caption: String = g.texts.iter().filter(|t| has_class(t, "caption")).map(|t| t.body.as_str()).collect::<Vec<_>>().join(" ");
    assert!(caption.contains("a to <b") && caption.contains("b is not") && caption.contains("last class is a to b"), "{caption:?}");
    let first_body = g.texts.iter().filter(|t| has_class(t, "tcell")).map(|t| t.y).fold(f64::INFINITY, f64::min);
    let cap_y = g.texts.iter().find(|t| has_class(t, "caption")).map(|t| t.y).unwrap();
    assert!(cap_y < first_body, "the caption sits above the first data row: {cap_y} vs {first_body}");
    // both copies carry the rule; the student copy does not lose it with the answers
    let (student, _) = crate::figures::answer_pair(&def(), &p).unwrap();
    assert!(parse(&student, false).texts.iter().any(|t| has_class(t, "caption") && t.body.contains("a to <b")));
    // a raw table has no class-interval caption
    let raw = parse(&render(&with(&[])), false);
    assert!(!raw.texts.iter().any(|t| has_class(t, "caption")));
    assert!(m.edge.is_some());
}

#[test]
fn boundary_precision_comes_from_the_width_not_a_fixed_six_places() {
    // a six-place start would print 0.123457 and lose the typed width; the chosen precision must
    // start from the width itself
    let p = with(&[
        s("mode", GROUPED),
        s("data", "0, 0.1234567, 0.2469134"),
        b("auto_classes", false),
        f("class_width", 0.1234567),
        f("class_start", 0.0),
    ]);
    let m = model(&p).unwrap();
    assert!(m.edge.as_ref().unwrap().contains("0.1234567 wide"), "{:?}", m.edge);
    assert!(m.rows.iter().any(|r| r.label.contains("0.1234567")), "{:?}", m.rows);
    assert!(!m.edge.as_ref().unwrap().contains("0.123457 wide"), "six places is not enough: {:?}", m.edge);
}

#[test]
fn a_value_just_below_a_billion_boundary_stays_in_the_class_below() {
    let p = with(&[s("mode", GROUPED), s("data", "0, 999999999999"), s("relative", OFF)]);
    let m = model(&p).unwrap();
    assert_eq!(m.n, 2);
    assert_eq!(m.rows.iter().map(|r| r.f).sum::<u64>(), 2);
    let row = |label: &str| m.rows.iter().find(|r| r.label == label).unwrap_or_else(|| panic!("no {label}: {:?}", m.rows));
    assert_eq!(row("0 to <500000000000").f, 1, "{:?}", m.rows);
    assert_eq!(row("500000000000 to <1000000000000").f, 1, "999999999999 is below 1e12: {:?}", m.rows);
    assert_eq!(row("1000000000000 to 1500000000000").f, 0, "the class above is empty: {:?}", m.rows);
}

#[test]
fn a_value_exactly_on_a_class_boundary_goes_to_the_class_on_its_right() {
    // the MED repro: ordinary 2-decimal values, auto width 0.01 starting at 12.34
    let data_text = "12.34, 12.35, 12.36, 12.37, 12.38";
    let p = with(&[s("mode", GROUPED), s("data", data_text)]);
    let m = model(&p).unwrap();
    assert_eq!(m.rows.iter().map(|r| r.f).collect::<Vec<_>>(), vec![1, 1, 1, 1, 1], "{:?}", m.rows);
    assert_eq!(m.rows.iter().map(|r| r.f).sum::<u64>(), 5);
    assert_eq!(m.rows[0].label, "12.34 to <12.35");
    assert_eq!(m.rows[4].label, "12.38 to 12.39");
    // the same normal data via the histogram's shared bins: the counts must agree
    let values = data::numbers(data_text);
    let w = super::super::auto_width(&values);
    let st = super::super::auto_start(&values, w, true);
    let shared = super::super::bins(&values, st, w, true).unwrap();
    assert_eq!(m.rows.iter().map(|r| r.f).collect::<Vec<_>>(), shared.iter().map(|b| b.n as u64).collect::<Vec<_>>());

    // manual width 0.01, start 12.34 on 12.34/12.35/12.36: 1, 1, 1, not 2, 1, 0
    let p = with(&[
        s("mode", GROUPED),
        s("data", "12.34, 12.35, 12.36"),
        b("auto_classes", false),
        f("class_width", 0.01),
        f("class_start", 12.34),
    ]);
    let m = model(&p).unwrap();
    assert_eq!(m.rows.iter().map(|r| r.f).collect::<Vec<_>>(), vec![1, 1, 1], "{:?}", m.rows);

    // every boundary of a 5-class table, including the first and the closed last one
    let p = with(&[
        s("mode", GROUPED),
        s("data", "12.34, 12.35, 12.36, 12.37, 12.38"),
        b("auto_classes", false),
        f("class_width", 0.01),
        f("class_start", 12.34),
    ]);
    let m = model(&p).unwrap();
    assert_eq!(m.rows.len(), 5);
    assert_eq!(m.rows.iter().map(|r| r.f).collect::<Vec<_>>(), vec![1, 1, 1, 1, 1], "{:?}", m.rows);

    // negative boundaries and tiny boundaries
    for (start, width, data_text, n) in [
        (-2.02, 0.01, "\u{2212}2.02, \u{2212}2.01, \u{2212}2.00, \u{2212}1.99, \u{2212}1.98", 5),
        (1e-20, 1e-20, "1e-20, 2e-20, 3e-20", 3),
    ] {
        let p = with(&[
            s("mode", GROUPED),
            s("data", data_text),
            b("auto_classes", false),
            f("class_width", width),
            f("class_start", start),
        ]);
        let m = model(&p).unwrap();
        assert_eq!(m.rows.iter().map(|r| r.f).sum::<u64>(), n, "{data_text}");
        assert!(m.rows.iter().all(|r| r.f >= 1), "{data_text}: every value on a boundary: {:?}", m.rows);
    }
}

#[test]
fn the_edge_rule_is_stated_and_a_value_on_a_boundary_goes_right() {
    // 10 and 30 are exactly on boundaries; with [a,b) they belong to the classes starting there
    let p = with(&[
        s("mode", GROUPED),
        s("data", "10, 20, 20, 30"),
        b("auto_classes", false),
        f("class_width", 10.0),
        f("class_start", 10.0),
    ]);
    let m = model(&p).unwrap();
    assert_eq!(
        m.rows.iter().map(|r| (r.label.clone(), r.f)).collect::<Vec<_>>(),
        vec![("10 to <20".to_string(), 1), ("20 to <30".to_string(), 2), ("30 to 40".to_string(), 1)]
    );
    let edge = m.edge.unwrap();
    assert!(edge.contains("The classes are 10 wide, starting at 10"), "{edge}");
    assert!(edge.contains("A value on a boundary goes in the class to its right"), "{edge}");
    assert!(edge.contains("The last class is a to b"), "{edge}");
    assert!(edge.contains("includes its upper boundary 40"), "{edge}");
    assert!(edge.contains("above the largest value (30)"), "{edge}");
    // the classes meet exactly: each upper is the next lower
    let class_bounds = |label: &str| -> (f64, f64) {
        let a = label.split(" to ").next().unwrap().replace('\u{2212}', "-").parse().unwrap();
        let b = label.split(" to ").nth(1).unwrap().trim_start_matches('<').replace('\u{2212}', "-").parse().unwrap();
        (a, b)
    };
    let bounds: Vec<(f64, f64)> = m.rows.iter().map(|r| class_bounds(&r.label)).collect();
    for w in bounds.windows(2) {
        assert_eq!(w[0].1, w[1].0, "the classes meet");
    }
    // the last class covers the largest value and closes above it
    assert!(bounds.last().unwrap().1 > 30.0);
}

#[test]
fn manual_classes_are_used_and_a_refused_width_or_start_is_replaced_with_a_fixed_value() {
    let good = with(&[
        s("mode", GROUPED),
        s("data", "40, 42, 49, 50, 55, 60, 61"),
        b("auto_classes", false),
        f("class_width", 10.0),
        f("class_start", 40.0),
    ]);
    assert!(fix_note(&good).is_none(), "{:?}", warnings(&good));
    let m = model(&good).unwrap();
    assert_eq!(m.rows.iter().map(|r| r.f).collect::<Vec<_>>(), vec![3, 2, 2], "{:?}", m.rows);
    assert!(m.rows[0].label.starts_with("40 to <50"));

    // a width of 0 is refused, named, given a fixed value, and the table still draws
    let mut bad = good.clone();
    bad.set_f("class_width", 0.0);
    let note = fix_note(&bad).expect("a fix for a zero width");
    assert!(note.contains("Class width") && note.contains("Set Class width to"), "{note}");
    assert!(!render(&bad).contains("could not be drawn"), "the table still draws");
    let g = parse(&render(&bad), false);
    assert!(bodies(&g, "fcell").len() >= 2, "the fallback table has rows");
    assert!(apply_fix(&note, &mut bad));
    assert!(fix_note(&bad).is_none(), "the offered fix works: {:?}", warnings(&bad));

    // a start above the smallest value is refused and named with the smallest value
    let mut up = good.clone();
    up.set_f("class_start", 100.0);
    let note = fix_note(&up).expect("a fix for a start above the data");
    assert!(note.contains("First class starts at") && note.contains("above the smallest value 40"), "{note}");
    assert!(note.contains("Set First class starts at 40"), "{note}");
    assert!(apply_fix(&note, &mut up));
    assert!(fix_note(&up).is_none(), "{:?}", warnings(&up));

    // a tiny width would make too many classes: the fix names a width that works
    let mut tiny = good.clone();
    tiny.set_f("class_width", 0.01);
    let note = fix_note(&tiny).expect("a fix for too many classes");
    assert!(note.contains("more than 200 classes") && note.contains("Set Class width to"), "{note}");
    assert!(apply_fix(&note, &mut tiny));
    assert!(fix_note(&tiny).is_none(), "{:?}", warnings(&tiny));

    // a start far below the data is the real cause of too many classes; the note must name it,
    // and following only "Class width 10" cannot clear it
    let mut below = with(&[
        s("mode", GROUPED),
        s("data", DEFAULT_DATA),
        b("auto_classes", false),
        f("class_width", 10.0),
        f("class_start", -10000.0),
    ]);
    let notes = warnings(&below).join(" ");
    assert!(notes.contains("Set First class starts at 4"), "the start is named: {notes}");
    assert!(apply_fix(&notes, &mut below), "both width and start apply");
    let mb = model(&below).expect("the started table draws");
    assert_eq!(mb.rows.iter().map(|r| r.f).sum::<u64>(), 10);
    assert!(!warnings(&below).iter().any(|n| n.contains("more than 200 classes")), "{:?}", warnings(&below));

    // a range too wide for any manual width gets the switch, never a value the control cannot hold
    let mut huge = with(&[
        s("mode", GROUPED),
        s("data", "\u{2212}1000000000000, 1000000000000"),
        b("auto_classes", false),
        f("class_width", 0.0),
        f("class_start", 0.0),
    ]);
    let note = fix_note(&huge).expect("a fix for a range with no usable manual width");
    assert!(note.contains("Turn on Choose the class width automatically"), "{note}");
    assert!(!note.contains("Set Class width to"), "an impossible manual value is not offered: {note}");
    assert!(apply_fix(&note, &mut huge));
    assert!(fix_note(&huge).is_none(), "{:?}", warnings(&huge));

    // the critic's minor: a tiny width on 1..5 should offer the readable automatic width (1),
    // not the narrowest width that merely satisfies the 200-class cap (0.05 -> 101 rows)
    let mut five = with(&[
        s("mode", GROUPED),
        s("data", "1, 2, 3, 4, 5"),
        b("auto_classes", false),
        f("class_width", 0.01),
        f("class_start", 1.0),
    ]);
    let note = fix_note(&five).expect("a fix for five values with a tiny width");
    assert!(note.contains("Set Class width to 1 or more"), "{note}");
    assert!(apply_fix(&note, &mut five));
    let m = model(&five).unwrap();
    assert_eq!(m.rows.len(), 5, "five one-wide classes: {:?}", m.rows);
}

// ───────────────────────── the table itself ─────────────────────────

#[test]
fn every_printed_cell_is_the_models_cell_over_the_option_space() {
    let mut checked = 0;
    for rel in [OFF, FRACTION, DECIMAL, PERCENT] {
        for (tally, cum, cum_rel, total) in [(true, true, true, true), (false, false, false, false), (true, false, true, true), (false, true, false, false)] {
            for (mode, data_text) in [(RAW, DEFAULT_DATA), (GROUPED, DEFAULT_GROUPED)] {
                for d in 0..=2 {
                    let p = with(&[
                        s("mode", mode),
                        s("data", data_text),
                        b("tally", tally),
                        s("relative", rel),
                        ("decimals", Value::I(d)),
                        b("cumulative", cum),
                        b("cum_rel", cum_rel),
                        b("total", total),
                    ]);
                    let m = model(&p).unwrap();
                    let g = parse(&render(&p), false);
                    assert_eq!(bodies(&g, "tcell"), m.rows.iter().map(|r| r.label.clone()).collect::<Vec<_>>(), "{mode:?} {rel} {d}");
                    assert_eq!(bodies(&g, "fcell"), m.rows.iter().map(|r| r.f.to_string()).collect::<Vec<_>>(), "{mode:?} {rel} {d}");
                    if rel == OFF {
                        assert!(bodies(&g, "rcell").is_empty(), "no relative column when off");
                    } else {
                        assert_eq!(bodies(&g, "rcell"), m.rows.iter().map(|r| r.rel.clone()).collect::<Vec<_>>(), "{mode:?} {rel} {d}");
                    }
                    if cum {
                        assert_eq!(bodies(&g, "ccell"), m.rows.iter().map(|r| r.cum.to_string()).collect::<Vec<_>>(), "{mode:?} {rel} {d}");
                    } else {
                        assert!(bodies(&g, "ccell").is_empty(), "no cumulative column when off");
                    }
                    if cum_rel && rel != OFF {
                        assert_eq!(bodies(&g, "crcell"), m.rows.iter().map(|r| cum_rel_text(r.cum, m.n, m.rel, m.decimals)).collect::<Vec<_>>(), "{mode:?} {rel} {d}");
                    } else {
                        assert!(bodies(&g, "crcell").is_empty(), "no cumulative relative column when off");
                    }
                    if total {
                        assert_eq!(totals(&g, "fcell"), vec![m.n.to_string()], "{mode:?} {rel} {d}");
                        assert_eq!(
                            totals(&g, "crcell"),
                            if cum_rel && rel != OFF { vec![cum_rel_text(m.n, m.n, m.rel, m.decimals)] } else { Vec::new() },
                            "{mode:?} {rel} {d}"
                        );
                    } else {
                        assert!(totals(&g, "fcell").is_empty(), "no Total row when off");
                    }
                    let heads: Vec<&str> = g.texts.iter().filter(|t| has_class(t, "thead")).map(|t| t.body.as_str()).collect();
                    assert!(heads.contains(&heading_for(mode)), "{mode:?}: {heads:?}");
                    assert_eq!(heads.contains(&"Tally"), tally, "{mode:?}: tally header");
                    checked += 1;
                }
            }
        }
    }
    println!("frequency table: {checked} cell-by-cell option states checked");
}
fn heading_for(mode: &str) -> &'static str {
    if mode == GROUPED { "Class" } else { "Value" }
}

#[test]
fn the_empty_data_box_draws_an_empty_table_with_an_instruction() {
    for text in ["", "   \n  \t "] {
        for mode in [RAW, GROUPED] {
            let p = with(&[s("mode", mode), s("data", text)]);
            let m = model(&p).unwrap();
            assert!(m.empty && m.rows.is_empty() && m.n == 0, "{text:?} {mode}");
            let svg = render(&p);
            let g = parse(&svg, false);
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{text:?} {mode}");
            assert!(!g.texts.iter().any(|t| has_class(t, "fcell")), "no frequency cells");
            assert!(!g.texts.iter().any(|t| has_class(t, "rcell")), "no relative cells");
            assert!(!g.texts.iter().any(|t| has_class(t, "ccell")), "no cumulative cells");
            assert!(!g.lines.iter().any(|l| l.class == "tally"), "no tally marks");
            assert!(g.texts.iter().any(|t| has_class(t, "instruction") && t.body.contains("Type values in Data")), "{text:?} {mode}: {:?}", all_text(&g));
            assert!(g.texts.iter().filter(|t| has_class(t, "thead")).count() >= 3, "the header is drawn");
            assert!(g.lines.iter().filter(|l| l.class == "rule").count() >= 4, "the table rules are drawn");
        }
    }
    // an empty table is not an editor refusal
    assert!(warnings(&with(&[s("data", "")])).is_empty());
}

#[test]
fn the_student_copy_blanks_every_answer_and_the_whole_source_leaks_none() {
    let cases: Vec<Vec<(&'static str, Value)>> = vec![
        vec![],
        vec![s("mode", GROUPED), s("data", DEFAULT_GROUPED)],
        vec![s("relative", FRACTION), b("tally", false)],
        vec![s("relative", DECIMAL), ("decimals", Value::I(2))],
        vec![("decimals", Value::I(2))],
        vec![s("mode", GROUPED), s("data", "0, 1, 1, 2, 2, 2, 9, 10"), b("auto_classes", false), f("class_width", 2.0), f("class_start", 0.0)],
        vec![s("data", "1, 2, 2")],
        // a state whose key carries answer notes: the student page must still be the same size
        vec![s("data", "1, 2, 3"), ("decimals", Value::I(0))],
        vec![s("data", "1, 2, 3"), s("relative", DECIMAL), ("decimals", Value::I(2))],
    ];
    let mut checked = 0;
    for list in cases {
        for style in ["Colour", "Print"] {
            let mut l = list.clone();
            l.push(s("style", style));
            let p = with(&l);
            let (student, key) = crate::figures::answer_pair(&def(), &p).expect("both copies");
            let vb = |x: &str| attr(x.split('>').next().unwrap(), "viewBox").unwrap();
            assert_eq!(vb(&student), vb(&key), "same page size: {list:?} {style}");
            let (sg, kg) = (parse(&student, false), parse(&key, false));
            // the answer columns are blank in the student copy, in every cell including the Total row
            for class in ["fcell", "rcell", "ccell", "crcell"] {
                assert!(bodies(&sg, class).is_empty(), "{class} body leaked: {list:?} {style}");
                assert!(totals(&sg, class).is_empty(), "{class} total leaked: {list:?} {style}");
            }
            assert!(!sg.lines.iter().any(|l| l.class == "tally"), "tally leaked: {list:?} {style}");
            // every answer string in the key is absent from the student's text, or is a given
            let givens: Vec<String> = kg.texts.iter().filter(|t| !has_class(t, "fcell") && !has_class(t, "rcell") && !has_class(t, "ccell") && !has_class(t, "crcell")).map(|t| t.body.clone()).collect();
            for t in kg.texts.iter().filter(|t| has_class(t, "fcell") || has_class(t, "rcell") || has_class(t, "ccell") || has_class(t, "crcell")) {
                let in_student = sg.texts.iter().filter(|u| u.body == t.body).count();
                let given = givens.iter().filter(|g| **g == t.body).count();
                assert!(in_student <= given, "{:?} leaked: student has it {in_student} times, only {given} givens: {list:?} {style}", t.body);
            }
            // nothing answer-like in the title/desc or a comment
            let desc = student.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
            assert!(desc.contains("blank") || desc.contains("for students"), "the student description says the columns are blank: {desc}");
            for t in kg.texts.iter().filter(|t| has_class(t, "fcell") || has_class(t, "rcell") || has_class(t, "ccell") || has_class(t, "crcell")) {
                assert!(!desc.contains(&t.body), "desc leaks {:?}: {desc}", t.body);
            }
            assert!(!student.contains("<!--"), "no comments in the student source");
            assert!(!student.contains("fcell") && !student.contains("rcell") && !student.contains("ccell") && !student.contains("crcell"), "no answer-cell class in the student source: {list:?} {style}");
            assert!(!student.contains("class=\"tally\""), "no tally stroke class in the student source");
            // the key's answer notes are not in the student copy
            for note in &model(&p).unwrap().key_notes {
                assert!(!student.contains(&svg::esc(note)), "key note leaked: {note}");
            }
            checked += 1;
        }
    }
    println!("frequency table: {checked} student/key pairs leak-scanned");
}

// ───────────────────────── Print and layout ─────────────────────────

fn is_grey(c: &str) -> bool {
    matches!(c, "none" | "#fff" | "#000") || (c.len() == 7 && c.starts_with('#') && c.as_bytes()[1..3].eq_ignore_ascii_case(&c.as_bytes()[3..5]) && c.as_bytes()[3..5].eq_ignore_ascii_case(&c.as_bytes()[5..7]))
}

#[test]
fn print_is_grey_only_the_header_band_is_visible_and_medium_text_survives_a_66_percent_copy() {
    for (mode, text) in [(RAW, DEFAULT_DATA), (GROUPED, DEFAULT_GROUPED)] {
        for key in [true, false] {
            for size in ["Small", "Medium", "Large"] {
            let p = with(&[s("mode", mode), s("data", text), s("style", "Print"), s("label_size", size), b(SHOW_ANSWERS, key)]);
            let svg = render(&p);
            for k in ["fill", "stroke"] {
                for v in svg.split(&format!(" {k}=\"")).skip(1).map(|x| x.split('"').next().unwrap()) {
                    assert!(is_grey(v), "{mode:?} {size} key={key}: {k}={v} in Print");
                }
            }
            let g = parse(&svg, false);
            let band = g.rects.iter().find(|r| r.class == "thead-band").expect("a header band");
            assert!(is_grey(&band.fill) && band.fill != "#fff" && band.fill != "none", "{mode:?} {size}: header band {:?}", band.fill);
            let small = g.texts.iter().map(|t| t.size).fold(f64::INFINITY, f64::min);
            assert!(small * 0.66 >= 8.4, "{mode:?} {size} key={key}: {small} px at 66%");
            for l in &g.lines {
                assert!(l.width * 0.66 >= 0.66, "{mode:?} {size}: hairline {} at 66%", l.width);
            }
            let png = crate::figures::to_png(&svg, 0.66).unwrap();
            let img = image::load_from_memory(&png).unwrap().to_rgba8();
            let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
            assert!(img.pixels().all(|px| hue(px) == 0), "{mode:?} {size}: colour in a Print render");
            // the header band is a grey field, darker than the white page, at 66% too
            let (w, h, px) = crate::figures::to_rgba(&svg, 0.66).unwrap();
            let lum = |x: f64, y: f64| -> f64 {
                let xi = (x * 0.66).round().clamp(0.0, (w - 1) as f64) as u32;
                let yi = (y * 0.66).round().clamp(0.0, (h - 1) as f64) as u32;
                let o = ((yi * w + xi) * 4) as usize;
                0.299 * px[o] as f64 + 0.587 * px[o + 1] as f64 + 0.114 * px[o + 2] as f64
            };
            let band_l = lum(band.x + band.w / 2.0, band.y + band.h / 2.0);
            assert!(band_l < 190.0, "{mode:?} {size}: the header band is too pale at 66% ({band_l:.0})");
            }
        }
    }
    // Colour and Print draw the same geometry
    let a = render(&with(&[]));
    let pr = render(&with(&[s("style", "Print")]));
    let ga = parse(&a, false);
    let gp = parse(&pr, false);
    assert_eq!(ga.lines.iter().map(|l| (l.class.clone(), l.a, l.b, l.width)).collect::<Vec<_>>(), gp.lines.iter().map(|l| (l.class.clone(), l.a, l.b, l.width)).collect::<Vec<_>>(), "the same rules");
    assert_eq!(
        ga.texts.iter().map(|t| (t.class.clone(), t.x, t.y, t.size, t.body.clone())).collect::<Vec<_>>(),
        gp.texts.iter().map(|t| (t.class.clone(), t.x, t.y, t.size, t.body.clone())).collect::<Vec<_>>(),
        "the same text at the same place"
    );
}

fn seg_dist(q: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx * dx + dy * dy;
    let t = if l == 0.0 { 0.0 } else { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l).clamp(0.0, 1.0) };
    (a.0 + t * dx - q.0).hypot(a.1 + t * dy - q.1)
}
fn seg_box(a: (f64, f64), b: (f64, f64), bx: Bx) -> f64 {
    let pb = |q: (f64, f64)| {
        let ex = (bx.0 - q.0).max(0.0).max(q.0 - bx.2);
        let ey = (bx.1 - q.1).max(0.0).max(q.1 - bx.3);
        ex.hypot(ey)
    };
    let far = (bx.0 - a.0.max(b.0)).max(a.0.min(b.0) - bx.2).max(bx.1 - a.1.max(b.1)).max(a.1.min(b.1) - bx.3);
    if far > 50.0 {
        return far;
    }
    let o = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let cut = |c: (f64, f64), d: (f64, f64)| (o(a, b, c) >= 0.0) != (o(a, b, d) >= 0.0) && (o(c, d, a) >= 0.0) != (o(c, d, b) >= 0.0);
    let (p0, p1, p2, p3) = ((bx.0, bx.1), (bx.2, bx.1), (bx.2, bx.3), (bx.0, bx.3));
    if pb(a) == 0.0 || pb(b) == 0.0 || cut(p0, p1) || cut(p1, p2) || cut(p2, p3) || cut(p3, p0) {
        return 0.0;
    }
    [seg_dist((bx.0, bx.1), a, b), seg_dist((bx.2, bx.1), a, b), seg_dist((bx.0, bx.3), a, b), seg_dist((bx.2, bx.3), a, b), pb(a), pb(b)]
        .into_iter()
        .fold(f64::INFINITY, f64::min)
}
fn overlap(a: Bx, c: Bx, pad: f64) -> bool {
    a.0 - pad < c.2 && c.0 - pad < a.2 && a.1 - pad < c.3 && c.1 - pad < a.3
}

#[test]
fn resvg_ink_boxes_never_collide_clip_or_leave_a_cell_over_the_option_space_and_sizes() {
    let sets = [(RAW, DEFAULT_DATA), (GROUPED, DEFAULT_GROUPED), (GROUPED, "\u{2212}5, \u{2212}1, 0, 0.25, 0.25, 3.75, 8.5")];
    let mut renders = 0;
    for (mode, text) in sets {
        for rel in [OFF, FRACTION, DECIMAL, PERCENT] {
            for (tally, cum, cum_rel) in [(true, true, true), (false, false, false)] {
                for total in [true, false] {
                    for size in ["Small", "Medium", "Large"] {
                        for key in [true, false] {
                            let p = with(&[
                                s("mode", mode),
                                s("data", text),
                                s("relative", rel),
                                b("tally", tally),
                                b("cumulative", cum),
                                b("cum_rel", cum_rel),
                                b("total", total),
                                s("label_size", size),
                                b(SHOW_ANSWERS, key),
                            ]);
                            let svg = render(&p);
                            let g = parse(&svg, true);
                            let what = format!("{mode:?} {rel} tally={tally} cum={cum} cum_rel={cum_rel} total={total} {size} key={key}");
                            // every glyph inside the page
                            for t in &g.texts {
                                assert!(t.ink.0 >= -0.5 && t.ink.1 >= -0.5 && t.ink.2 <= g.w + 0.5 && t.ink.3 <= g.h + 0.5, "{what}: {:?} off canvas {:?} in {}x{}", t.body, t.ink, g.w, g.h);
                            }
                            // no two glyphs touch
                            for i in 0..g.texts.len() {
                                for j in i + 1..g.texts.len() {
                                    assert!(!overlap(g.texts[i].ink, g.texts[j].ink, 0.5), "{what}: {:?} and {:?} collide", g.texts[i].body, g.texts[j].body);
                                }
                            }
                            // no glyph touches a table rule or a tally stroke
                            for t in &g.texts {
                                let inner = (t.ink.0 + 0.2, t.ink.1 + 0.2, t.ink.2 - 0.2, t.ink.3 - 0.2);
                                for l in g.lines.iter().filter(|l| l.class == "rule" || l.class == "tally") {
                                    if l.class == "tally" && has_class(t, "tallycell") {
                                        continue;
                                    }
                                    assert!(seg_box(l.a, l.b, inner) >= l.width / 2.0 + 0.4, "{what}: {:?} touches a {} line {:?}-{:?}", t.body, l.class, l.a, l.b);
                                }
                            }
                            // each cell's ink is inside its own column, with the table's padding
                            let mut xs: Vec<f64> = g
                                .lines
                                .iter()
                                .filter(|l| l.class == "rule" && (l.a.0 - l.b.0).abs() < 1e-9)
                                .map(|l| l.a.0)
                                .collect();
                            xs.sort_by(f64::total_cmp);
                            xs.dedup_by(|a, b| (*a - *b).abs() < 0.05);
                            if xs.len() >= 2 {
                                for t in g.texts.iter().filter(|t| has_class(t, "thead") || has_class(t, "tcell") || has_class(t, "fcell") || has_class(t, "rcell") || has_class(t, "ccell") || has_class(t, "crcell")) {
                                    let at = xs.windows(2).find(|w| t.x >= w[0] - 0.5 && t.x <= w[1] + 0.5).unwrap_or_else(|| panic!("{what}: {:?} at x={} is outside {xs:?}", t.body, t.x));
                                    assert!(t.ink.0 >= at[0] - 0.2 && t.ink.2 <= at[1] + 0.2, "{what}: {:?} leaves its column {}..{}: {:?}", t.body, at[0], at[1], t.ink);
                                }
                            }
                            renders += 1;
                        }
                    }
                }
            }
        }
    }
    println!("frequency table: {renders} renders audited with resvg ink boxes");
}

// ───────────────────────── refusals, empty and odd input ─────────────────────────

#[test]
fn refusals_name_the_box_the_reason_and_a_fixed_value_and_odd_input_never_panics() {
    // no numbers at all: the message names Data, says why, and gives a value to type
    let p = with(&[s("data", "absent, absent, n/a")]);
    let note = warnings(&p).join(" ");
    assert!(note.contains("Data has no numbers") && note.contains("for example") && note.contains("4, 5, 5, 6"), "{note}");
    let svg = render(&p);
    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
    assert!(parse(&svg, false).texts.iter().any(|t| t.body.contains("Data has no numbers")));
    let mut fixed = p.clone();
    fixed.set_s("data", "4, 5, 5, 6");
    assert!(model(&fixed).is_ok(), "the fixed value works");

    // numbers outside the figure's range are refused, with the range and an example
    let p = with(&[s("data", "1e308, 2e308")]);
    let note = warnings(&p).join(" ");
    assert!(note.contains("Data has no numbers") && note.contains("1000000000000"), "{note}");

    // an out-of-range number is named as such, not as a word
    let p = with(&[s("data", "1000000000000, 1000000000001"), s("relative", OFF)]);
    let m = model(&p).unwrap();
    assert_eq!(m.n, 1);
    let note = warnings(&p).join(" ");
    assert!(note.contains("beyond the largest number") && note.contains("1000000000000"), "{note}");
    assert!(!note.contains("not a number"), "{note}");
    let only_big = with(&[s("data", "1000000000001")]);
    let note = warnings(&only_big).join(" ");
    assert!(note.contains("beyond the largest number") && note.contains("1000000000000"), "{note}");

    // too many distinct raw values: the advice names the mode that works
    let many: String = (0..(MAX_ROWS + 1)).map(|i| i.to_string()).collect::<Vec<_>>().join(", ");
    let p = with(&[s("data", &many)]);
    let note = warnings(&p).join(" ");
    assert!(note.contains("more than 200 distinct values") && note.contains("Grouped classes"), "{note}");
    assert!(note.contains("at most 200 distinct values"), "a number to type: {note}");
    let mut fixed = p.clone();
    fixed.set_s("mode", GROUPED);
    assert!(model(&fixed).is_ok(), "grouping the same values works");

    // long typed headers are cut with the exact shorter value to type back
    let long_title = "A".repeat(200);
    let long_head = "B".repeat(80);
    let p = with(&[s("title", &long_title), s("heading", &long_head)]);
    let notes = warnings(&p);
    assert!(notes.iter().any(|n| n.contains("Title has 200 characters") && n.contains("Set Title to")), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("First column heading has 80 characters") && n.contains("Set First column heading to")), "{notes:?}");
    assert!(render(&p).starts_with("<svg"));

    // a word in the box is skipped, counted and named, but the numbers still draw
    let p = with(&[s("data", "Score\n4\n5\nabsent\n5")]);
    let m = model(&p).unwrap();
    assert_eq!(m.n, 3);
    let note = warnings(&p).join(" ");
    assert!(note.contains("Data: 2 cells that are not numbers were skipped"), "{note}");
    assert_eq!(bodies(&parse(&render(&p), false), "fcell"), vec!["1", "2"]);
}

#[test]
fn thousands_groups_are_one_number_with_a_note_and_a_decimal_comma_is_refused() {
    // the MED repro: this used to be n=6 with rows 0:3, 1:1, 2:1, 3:1
    let p = with(&[s("data", "1,000, 2,000, 3,000"), s("relative", OFF)]);
    let m = model(&p).unwrap();
    assert_eq!(m.n, 3, "three thousands groups are three values");
    assert_eq!(
        m.rows.iter().map(|r| (r.label.clone(), r.f)).collect::<Vec<_>>(),
        vec![("1000".to_string(), 1), ("2000".to_string(), 1), ("3000".to_string(), 1)]
    );
    let note = warnings(&p).join(" ");
    assert!(note.contains("thousands separator") && note.contains("Line 1"), "{note}");
    assert!(note.contains("1,000") && note.contains("1000"), "{note}");
    assert!(note.contains("1, 000"), "the two-number reading is offered: {note}");

    // a comma with a space is the documented two-number reading, not a thousands group
    let p = with(&[s("data", "1, 000"), s("relative", OFF)]);
    let m = model(&p).unwrap();
    assert_eq!(m.n, 2);
    assert_eq!(m.rows.iter().map(|r| (r.label.clone(), r.f)).collect::<Vec<_>>(), vec![("0".to_string(), 1), ("1".to_string(), 1)]);
    assert!(!warnings(&p).iter().any(|n| n.contains("thousands separator")), "{:?}", warnings(&p));

    // a decimal comma is refused, with both fixed readings
    let p = with(&[s("data", "12,5"), s("relative", OFF)]);
    let note = warnings(&p).join(" ");
    assert!(note.contains("Data line 1") && note.contains("comma inside a number"), "{note}");
    assert!(note.contains("12.5") && note.contains("12, 5"), "{note}");
    assert!(render(&p).starts_with("<svg"));
    let mut fixed = p.clone();
    fixed.set_s("data", "12.5");
    let m = model(&fixed).unwrap();
    assert_eq!(m.n, 1);
    assert_eq!(m.rows[0].label, "12.5");

    // a mistyped group is refused too, and the suggestion keeps the digits
    let p = with(&[s("data", "1,0000"), s("relative", OFF)]);
    let note = warnings(&p).join(" ");
    assert!(note.contains("comma inside a number") && note.contains("10000"), "{note}");

    // a signed thousands group still reads as one number
    let p = with(&[s("data", "\u{2212}1,000, 2,000"), s("relative", OFF)]);
    let m = model(&p).unwrap();
    assert_eq!(m.n, 2);
    assert_eq!(m.rows.iter().map(|r| r.label.clone()).collect::<Vec<_>>(), vec!["\u{2212}1000", "2000"]);
}

#[test]
fn the_value_cap_counts_every_value_even_when_they_are_on_one_line() {
    let same: String = std::iter::repeat("1").take(5_001).collect::<Vec<_>>().join(", ");
    let p = with(&[s("data", &same), s("relative", OFF)]);
    let m = model(&p).unwrap();
    assert_eq!(m.n, 5_000, "the whole box is capped, not each line");
    assert!(warnings(&p).iter().any(|n| n.contains("more than 5000 values") && n.contains("first 5000")), "{:?}", warnings(&p));

    let many: String = (0..5_001).map(|i| i.to_string()).collect::<Vec<_>>().join(", ");
    let note = warnings(&with(&[s("data", &many)])).join(" ");
    assert!(note.contains("at most 200 distinct values"), "{note}");
    assert!(note.contains("Only the first 5000 values are used"), "{note}");
}

#[test]
fn ten_thousand_fuzzed_inputs_render_without_panicking_and_stay_inside_the_numbers() {
    let mut rng = data::Rng::new(20261104);
    let pieces = [
        "", "1", "-1", "0", "1e12", "1e13", "NaN", "inf", "-inf", "1e400", "\u{2212}5", "5%", "7.5", "x", "\u{1F600}", "\u{4E2D}\u{6587}",
        "+", "90+", "under", "-", "--", ",", ";", "\t", " ", "\n", "\r\n", "\u{0}", "<", "&", "\"", "10-20", "\u{2013}", "1,000",
    ];
    let word = |rng: &mut data::Rng, n: usize| -> String {
        (0..n).map(|_| pieces[(rng.unit() * pieces.len() as f64) as usize % pieces.len()]).collect::<Vec<_>>().join(if rng.unit() < 0.5 { ", " } else { " " })
    };
    for i in 0..10_000 {
        let mut p = defaults();
        let len = 1 + (rng.unit() * 10.0) as usize;
        let mut lines = Vec::with_capacity(len);
        for _ in 0..len {
            let n = (rng.unit() * 5.0) as usize;
            lines.push(word(&mut rng, n));
        }
        let text = lines.join("\n");
        p.set_s("data", &text);
        p.set_s("mode", if rng.unit() < 0.5 { RAW } else { GROUPED });
        p.set_s("title", &word(&mut rng, 2));
        p.set_s("heading", &word(&mut rng, 2));
        p.set_s("relative", [OFF, FRACTION, PERCENT, "junk"][i % 4]);
        p.set_i("decimals", [-3, 0, 1, 2, 99][i % 5]);
        p.set_b("tally", i % 3 != 0);
        p.set_b("cumulative", i % 4 != 0);
        p.set_b("total", i % 5 != 0);
        p.set_b("auto_classes", i % 3 != 0);
        p.set_f("class_width", [0.0, -1.0, 1e-9, 0.01, 1.0, 1e12, f64::NAN][i % 7]);
        p.set_f("class_start", [0.0, f64::NAN, -1e12, 5.0, 1e12][i % 5]);
        p.set_s("style", if i % 6 == 0 { "Print" } else { "Colour" });
        p.set_s("label_size", ["Small", "Medium", "Large", "junk"][i % 4]);
        p.set_b(SHOW_ANSWERS, i % 5 != 0);
        let svg = render(&p);
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{i}: {text:?}");
        let g = parse(&svg, false);
        for t in &g.texts {
            if has_class(t, "tcell") || has_class(t, "fcell") || has_class(t, "rcell") || has_class(t, "ccell") || has_class(t, "crcell") || has_class(t, "tallycell") {
                assert!(!t.body.contains("NaN") && !t.body.contains("inf") && t.body != "?", "{i}: {:?} in {text:?}", t.body);
            }
        }
        let _ = warnings(&p);
        if i % 500 == 0 {
            crate::figures::to_png(&svg, 0.25).unwrap_or_else(|e| panic!("{i}: {e}"));
        }
    }
    // a 10,000-value paste and a 200-row table
    let big: String = (0..10_000).map(|i| ((i * 37) % 1000).to_string()).collect::<Vec<_>>().join(", ");
    let p = with(&[s("data", &big)]);
    assert!(render(&p).starts_with("<svg"));
    assert!(warnings(&p).iter().any(|n| n.contains("more than 200 distinct values")), "{:?}", warnings(&p));
    let mut grouped = p.clone();
    grouped.set_s("mode", GROUPED);
    let svg = render(&grouped);
    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
    crate::figures::to_png(&svg, 0.25).unwrap();
    let rows: String = (0..MAX_ROWS).map(|i| format!("{}, {}", i, i % 3)).collect::<Vec<_>>().join("\n");
    let p = with(&[s("data", &rows)]);
    let g = parse(&render(&p), false);
    assert_eq!(bodies(&g, "fcell").len(), MAX_ROWS);
    assert!(warnings(&p).iter().any(|n| n.contains("200 rows") && n.contains("tall page")), "{:?}", warnings(&p));
}

// ───────────────────────── speed, purity, registration ─────────────────────────

fn cpu_now() -> std::time::Duration {
    let ticks = std::fs::read_to_string("/proc/thread-self/stat").ok().and_then(|s| {
        let rest: Vec<&str> = s[s.rfind(')')? + 1..].split_whitespace().collect();
        Some(rest.get(11)?.parse::<u64>().ok()? + rest.get(12)?.parse::<u64>().ok()?)
    });
    match ticks {
        Some(t) => std::time::Duration::from_millis(t * 10),
        None => {
            thread_local! { static T0: std::time::Instant = std::time::Instant::now(); }
            T0.with(|t| t.elapsed())
        }
    }
}

#[test]
fn drawing_is_pure_and_a_two_hundred_row_table_stays_within_the_cpu_budget() {
    let p = with(&[]);
    assert_eq!(render(&p), render(&p), "same params, same SVG");
    let rows: String = (0..200).map(|i| format!("{}, {}", i, i % 3)).collect::<Vec<_>>().join("\n");
    let base = with(&[s("data", &rows), s("label_size", "Large")]);
    let t0 = cpu_now();
    for i in 0..200 {
        let mut q = base.clone();
        q.set_i("decimals", (i % 3) as i64);
        q.set_s("relative", [OFF, FRACTION, PERCENT][i % 3]);
        let svg = render(&q);
        assert!(svg.len() > 2000, "{i}");
        let _ = warnings(&q);
    }
    let cpu = cpu_now() - t0;
    println!("frequency table: 200 draws of a 200-row table, {cpu:?} CPU");
    assert!(cpu < std::time::Duration::from_secs(30), "{cpu:?}");
}

#[test]
fn tallies_group_in_fives_and_are_capped_and_blanked_for_students() {
    for f in 0..=MAX_TALLY {
        let (_, marks) = tally_geom(f, 13.3);
        assert_eq!(marks.len() as u64, f, "one stroke per mark for {f}");
        if f >= 5 {
            let diagonals = marks.iter().filter(|(x1, y1, x2, y2)| (y1 - y2).abs() > 1e-9 && x1 != x2).count();
            assert_eq!(diagonals as u64, f / 5, "a diagonal closes each group of five: {f}");
        }
        if f > 0 {
            assert!(tally_geom(f, 13.3).0 > 0.0, "{f}: a positive box");
        }
    }
    assert_eq!(tally_geom(0, 13.3).1.len(), 0);
    assert_eq!(tally_geom(MAX_TALLY + 1, 13.3).1.len(), 0, "above the cap no tally is drawn");
    let many_same: String = (0..(MAX_TALLY + 1)).map(|_| "1".to_string()).collect::<Vec<_>>().join(", ");
    assert!(
        warnings(&with(&[s("data", &many_same)]))
            .iter()
            .any(|n| n.contains("draws counts up to 25") && n.contains("Turn off Tally column") && !n.contains("Choose Off")),
        "the cap and the checkbox advice are stated"
    );
    // the default table's tally has one stroke per value
    let p = with(&[]);
    let m = model(&p).unwrap();
    let g = parse(&render(&p), false);
    assert_eq!(g.lines.iter().filter(|l| l.class == "tally").count() as u64, m.n);
    let (student, _) = crate::figures::answer_pair(&def(), &p).unwrap();
    assert!(!parse(&student, false).lines.iter().any(|l| l.class == "tally"), "the student tally is blank");
}

#[test]
fn the_tally_column_keeps_one_fixed_width_in_both_copies() {
    let mut seen: Option<Vec<f64>> = None;
    for count in [1usize, 2, 10, 11, 12, 13, 20, 25] {
        let data_text = std::iter::repeat("7").take(count).collect::<Vec<_>>().join(", ");
        let p = with(&[s("data", &data_text)]);
        let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
        let (sg, kg) = (parse(&student, false), parse(&key, false));
        assert_eq!(
            attr(student.split('>').next().unwrap(), "viewBox"),
            attr(key.split('>').next().unwrap(), "viewBox"),
            "count {count}: both copies have the same page"
        );
        assert_eq!(verticals(&sg), verticals(&kg), "count {count}: same column rules");
        let xs = verticals(&kg);
        assert!(xs.len() >= 4, "count {count}: value, tally, frequency columns");
        let tally_w = xs[2] - xs[1];
        if let Some(old) = &seen {
            assert!((old[2] - old[1] - tally_w).abs() < 1e-6, "count {count}: tally width {tally_w} differs from {:?}", old);
        }
        seen = Some(xs);
        // the blanked student column is the same width as the key's
        assert!(bodies(&sg, "fcell").is_empty() && sg.lines.iter().all(|l| l.class != "tally"));
    }
}

#[test]
fn controls_relevance_examples_and_the_faq_are_registered_and_consistent() {
    let mut p = defaults();
    assert!(p.b("cum_rel"), "the cumulative relative column is on by default");
    let dg = parse(&render(&p), false);
    assert!(!bodies(&dg, "crcell").is_empty(), "the default table shows the cumulative relative column");
    assert!(dg.texts.iter().filter(|t| has_class(t, "thead")).any(|t| t.body.contains("Cumulative")), "default header");
    assert!(relevant(ID, "data", &p) && relevant(ID, "tally", &p) && relevant(ID, "relative", &p));
    assert!(!relevant(ID, "auto_classes", &p) && !relevant(ID, "class_width", &p) && !relevant(ID, "class_start", &p));
    assert!(relevant(ID, "decimals", &p) && relevant(ID, "cum_rel", &p));
    p.set_s("mode", GROUPED);
    assert!(relevant(ID, "auto_classes", &p) && !relevant(ID, "class_width", &p));
    p.set_b("auto_classes", false);
    assert!(relevant(ID, "class_width", &p) && relevant(ID, "class_start", &p));
    p.set_s("relative", FRACTION);
    assert!(!relevant(ID, "decimals", &p) && relevant(ID, "cum_rel", &p));
    p.set_s("relative", DECIMAL);
    assert!(relevant(ID, "decimals", &p));
    p.set_s("relative", OFF);
    assert!(!relevant(ID, "decimals", &p) && !relevant(ID, "cum_rel", &p));
    p.set_s("relative", PERCENT);
    assert!(relevant(ID, "decimals", &p));
    assert!(relevant("histogram", "mode", &p), "other figures are untouched");

    for e in examples(ID).unwrap() {
        let mut p = defaults();
        for (k, v) in e.params {
            p.0.insert(k.to_string(), v);
        }
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
        let svg = render(&p);
        assert!(svg.contains("class=\"rule\""), "{}: a real table", e.name);
        assert!(!svg.contains("could not be drawn"), "{}: no refusal", e.name);
        assert!(parse(&svg, false).texts.iter().any(|t| has_class(t, "thead")), "{}", e.name);
    }
    let faq: String = crate::figures::faq::faq_for(ID).iter().map(|q| format!("{} {}", q.q, q.a)).collect::<Vec<_>>().join(" ");
    for must in ["add to 100%", "100.5%", "class boundary", "tally", "fixed at the width of 25", "cumulative frequency", "cumulative relative frequency", "Decimal", "relative-tolerance", "Grouped classes", "round", "half up", "last class"] {
        assert!(faq.contains(must), "FAQ is missing {must:?}");
    }
    assert!((4..=6).contains(&crate::figures::faq::faq_for(ID).len()));
    let d = crate::figures::registry().into_iter().find(|d| d.id == ID).expect("registered");
    assert_eq!(d.subject, Subject::Math);
    assert!(d.title.contains("Frequency") && d.blurb.contains("tally"));
    let start = crate::figures::start_params(&d);
    assert!(warnings(&start).is_empty());
    assert!(render(&start).contains("class=\"rule\""));
    assert_eq!(crate::figures::notes_for(ID, &p), warnings(&p));
}

#[test]
#[ignore]
fn dump_pngs() {
    let dir = std::env::var("FREQUENCY_PNG_DIR").unwrap_or_else(|_| "/tmp/frequency-table-png".into());
    std::fs::create_dir_all(&dir).unwrap();
    let cases: Vec<(&str, Vec<(&str, Value)>)> = vec![
        ("default", vec![]),
        ("grouped", vec![s("mode", GROUPED), s("data", DEFAULT_GROUPED), s("title", "Minutes of exercise"), s("heading", "Minutes")]),
        ("fraction", vec![s("relative", FRACTION), b("tally", false)]),
        ("openstax", vec![s("mode", GROUPED), s("data", DEFAULT_HEIGHTS), s("relative", DECIMAL), ("decimals", Value::I(2)), s("title", "Heights of students"), s("heading", "Height (cm)")]),
        ("tiny_boundaries", vec![s("mode", GROUPED), s("data", "0.0000001, 0.0000002, 0.0000003")]),
        ("tiny_range", vec![s("mode", GROUPED), s("data", "1e-20, 2e-20, 3e-20")]),
        ("billion_edge", vec![s("mode", GROUPED), s("data", "0, 999999999999")]),
        ("rounded_note", vec![s("data", "1, 2, 3"), ("decimals", Value::I(0))]),
        ("decimal_note", vec![s("data", "1, 2, 3"), s("relative", DECIMAL), ("decimals", Value::I(2))]),
        ("boundary_12", vec![s("mode", GROUPED), s("data", "12.34, 12.35, 12.36, 12.37, 12.38")]),
        ("tiny_diff", vec![s("mode", GROUPED), s("data", "1.0000000000000002, 1.0000000000000004, 1.0000000000000007")]),
        ("start_below", vec![s("mode", GROUPED), s("data", DEFAULT_DATA), b("auto_classes", false), f("class_width", 10.0), f("class_start", -10000.0)]),
        ("too_big", vec![s("data", "1000000000000, 1000000000001")]),
        ("empty_grouped", vec![s("mode", GROUPED), s("data", "")]),
        ("thousands", vec![s("data", "1,000, 2,000, 3,000")]),
        ("print", vec![s("style", "Print"), s("mode", GROUPED), s("data", DEFAULT_GROUPED)]),
        ("print_small", vec![s("style", "Print"), s("label_size", "Small")]),
        ("small", vec![s("label_size", "Small")]),
        ("large", vec![s("label_size", "Large")]),
        ("empty", vec![s("data", "")]),
        ("refused", vec![s("data", "absent, n/a")]),
        ("manual_bad", vec![s("mode", GROUPED), b("auto_classes", false), f("class_width", 0.01)]),
    ];
    for (name, list) in cases {
        for key in [true, false] {
            let p = with(&list);
            let mut q = p.clone();
            q.set_b(SHOW_ANSWERS, key);
            let svg = render(&q);
            let tag = if key { "key" } else { "student" };
            std::fs::write(format!("{dir}/{name}-{tag}.png"), crate::figures::to_png(&svg, 2.0).unwrap()).unwrap();
            std::fs::write(format!("{dir}/{name}-{tag}.svg"), &svg).unwrap();
            if name.starts_with("print") {
                std::fs::write(format!("{dir}/{name}-{tag}-66.png"), crate::figures::to_png(&svg, 0.66).unwrap()).unwrap();
            }
        }
    }
}
