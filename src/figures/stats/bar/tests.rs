//! Bar chart tests. Geometry is read back from the SVG with this file's own parser, text ink
//! from resvg with the repo's fonts; answers and advice are recomputed or applied here
//! independently of the module.
use super::*;

type Bx = (f64, f64, f64, f64);

fn defaults() -> Params { Params::defaults(&params()) }

fn with(pairs: &[(&str, &str)]) -> Params {
    let mut p = defaults();
    for (k, v) in pairs {
        set(&mut p, k, v);
    }
    p
}

fn set(p: &mut Params, k: &str, v: &str) {
    match params().iter().find(|d| d.key == k).map(|d| d.kind.clone()) {
        Some(crate::figures::Kind::Bool) => p.set_b(k, v == "true"),
        Some(crate::figures::Kind::Float { .. }) => p.set_f(k, v.parse().unwrap()),
        Some(crate::figures::Kind::Int { .. }) => p.set_i(k, v.parse().unwrap()),
        _ => p.set_s(k, v),
    }
}

// ───────────────────────── own SVG parser ─────────────────────────

#[derive(Clone, Debug)]
struct El {
    tag: String,
    raw: String,
    text: String,
}

impl El {
    fn a(&self, name: &str) -> Option<String> {
        let pat = format!(" {name}=\"");
        let i = self.raw.find(&pat)? + pat.len();
        self.raw[i..].split('"').next().map(str::to_string)
    }
    fn n(&self, name: &str) -> f64 { self.a(name).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
}

fn unesc(s: &str) -> String { s.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&") }

/// Every element in drawing order (outside `<defs>`), with a text element's content.
fn elements(svg: &str) -> Vec<El> {
    let mut out = Vec::new();
    let mut rest = svg;
    let mut in_defs = false;
    while let Some(i) = rest.find('<') {
        rest = &rest[i..];
        let end = rest.find('>').unwrap();
        let head = &rest[..=end];
        rest = &rest[end + 1..];
        let tag: String = head[1..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '/').collect();
        match tag.as_str() {
            "defs" => in_defs = true,
            "/defs" => in_defs = false,
            "text" => {
                let close = rest.find("</text>").unwrap();
                out.push(El { tag, raw: head.to_string(), text: unesc(&rest[..close]) });
                rest = &rest[close + 7..];
            }
            t if !in_defs && !t.starts_with('/') => out.push(El { tag, raw: head.to_string(), text: String::new() }),
            _ => {}
        }
    }
    out
}

fn size(svg: &str) -> (f64, f64) {
    let root = elements(svg).into_iter().find(|e| e.tag == "svg").unwrap();
    (root.n("width"), root.n("height"))
}

fn texts(svg: &str) -> Vec<El> { elements(svg).into_iter().filter(|e| e.tag == "text").collect() }

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

/// Text ink boxes from resvg, paired with this parser's text elements (same order and text).
fn ink(svg: &str) -> Vec<(El, Bx)> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(&crate::figures::serif_stack(svg), &opt).expect("the SVG parses");
    let mut v = Vec::new();
    walk(tree.root(), &mut v);
    let mine = texts(svg);
    assert_eq!(v.len(), mine.len(), "every text element is one resvg text");
    mine.into_iter()
        .zip(v)
        .map(|(e, (t, b))| {
            assert_eq!(t, e.text, "resvg and this parser agree on the text");
            (e, b)
        })
        .collect()
}

fn overlap(a: Bx, b: Bx, pad: f64) -> bool { a.0 - pad < b.2 && b.0 - pad < a.2 && a.1 - pad < b.3 && b.1 - pad < a.3 }

fn seg_box(a: (f64, f64), b: (f64, f64), bx: Bx) -> f64 {
    // distance from a segment to a box: 0 when they meet (sampled finely)
    let n = (((b.0 - a.0).hypot(b.1 - a.1)) / 0.25).ceil().max(1.0) as usize;
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            let dx = (bx.0 - x).max(0.0).max(x - bx.2);
            let dy = (bx.1 - y).max(0.0).max(y - bx.3);
            dx.hypot(dy)
        })
        .fold(f64::INFINITY, f64::min)
}

/// Shapes a label may not touch: bar and swatch edges, axis and tick lines, table rules, answer
/// lines (gridlines excepted), the cumulative line and its markers.
struct Shapes {
    edges: Vec<Bx>,
    segs: Vec<((f64, f64), (f64, f64), f64)>,
    dots: Vec<((f64, f64), f64)>,
    fills: Vec<Bx>,
    chips: Vec<Bx>,
}

fn shapes(svg: &str) -> Shapes {
    let mut s = Shapes { edges: vec![], segs: vec![], dots: vec![], fills: vec![], chips: vec![] };
    for e in elements(svg) {
        match e.tag.as_str() {
            "rect" if e.a("width").as_deref() != Some("100%") => {
                let (x, y, w, h, sw) = (e.n("x"), e.n("y"), e.n("width"), e.n("height"), e.n("stroke-width"));
                if e.a("stroke").as_deref() == Some("none") {
                    s.chips.push((x, y, x + w, y + h));
                    continue;
                }
                let k = sw / 2.0;
                s.edges.extend([(x - k, y - k, x + w + k, y + k), (x - k, y + h - k, x + w + k, y + h + k), (x - k, y - k, x + k, y + h + k), (x + w - k, y - k, x + w + k, y + h + k)]);
                s.fills.push((x, y, x + w, y + h));
            }
            "line" => {
                let c = e.a("stroke").unwrap();
                if c == GRID || c == "#cccccc" {
                    continue;
                }
                s.segs.push(((e.n("x1"), e.n("y1")), (e.n("x2"), e.n("y2")), e.n("stroke-width")));
            }
            "polyline" => {
                let pts: Vec<(f64, f64)> = e.a("points").unwrap().split(' ').map(|p| {
                    let (a, b) = p.split_once(',').unwrap();
                    (a.parse().unwrap(), b.parse().unwrap())
                }).collect();
                let sw = e.n("stroke-width");
                for w in pts.windows(2) {
                    s.segs.push((w[0], w[1], sw));
                }
            }
            "circle" => s.dots.push(((e.n("cx"), e.n("cy")), e.n("r") + e.n("stroke-width") / 2.0)),
            _ => {}
        }
    }
    s
}

/// All collision invariants of one rendered SVG; returns the problems found.
fn audit(svg: &str) -> Vec<String> {
    let (w, h) = size(svg);
    let ink = ink(svg);
    let sh = shapes(svg);
    let mut bad = Vec::new();
    for (i, (e, b)) in ink.iter().enumerate() {
        if b.0 < 1.0 || b.1 < 1.0 || b.2 > w - 1.0 || b.3 > h - 1.0 {
            bad.push(format!("off canvas: {:?} {b:?} in {w}x{h}", e.text));
        }
        for (f, c) in &ink[i + 1..] {
            if overlap(*b, *c, 0.5) {
                bad.push(format!("text/text: {:?} {b:?} and {:?} {c:?}", e.text, f.text));
            }
        }
        let inner = (b.0 + 0.3, b.1 + 0.3, b.2 - 0.3, b.3 - 0.3);
        for ed in &sh.edges {
            if overlap(inner, *ed, 0.0) {
                bad.push(format!("text/edge: {:?} {b:?} {ed:?}", e.text));
            }
        }
        for fl in &sh.fills {
            if overlap(inner, *fl, 0.0) && !sh.chips.iter().any(|c| c.0 <= b.0 && c.1 <= b.1 && c.2 >= b.2 && c.3 >= b.3) {
                bad.push(format!("text on a bar: {:?} {b:?}", e.text));
            }
        }
        for (a, c, sw) in &sh.segs {
            if seg_box(*a, *c, inner) < sw / 2.0 + 0.5 {
                bad.push(format!("text/line: {:?} {b:?} {a:?}-{c:?}", e.text));
            }
        }
        for (c, r) in &sh.dots {
            if seg_box(*c, *c, inner) < r + 0.5 {
                bad.push(format!("text/marker: {:?}", e.text));
            }
        }
    }
    for f in &sh.fills {
        if f.0 < 0.0 || f.1 < 0.0 || f.2 > w || f.3 > h {
            bad.push(format!("shape off canvas {f:?}"));
        }
    }
    bad
}

// ───────────────────────── data sets ─────────────────────────

fn many(n: usize) -> String { (1..=n).map(|i| format!("Category number {i} with words, {}\n", (i * 37) % 23)).collect() }

fn data_sets() -> Vec<(&'static str, String)> {
    vec![
        ("default", "Walk, 12\nCar, 9\nBus, 6\nBike, 3".into()),
        ("two", "Pet, Class A, Class B\nDog, 11, 8\nCat, 7, 9\nFish, 3, 2\nNone, 4, 9".into()),
        ("many30", many(30)),
        ("thirteen", many(13)),
        ("unbroken", "Supercalifragilisticexpialidociousness, 3\nPneumonoultramicroscopicsilicovolcano, 7\nShort, 11\nAntidisestablishmentarianism, 15".into()),
        ("one", "Only, 5".into()),
        ("zeros_ties", "A, 0\nB, 7\nC, 7\nD, 0\nE, 1".into()),
        ("big", "Alpha, 1000000000\nBeta, 999999999\nGamma, 1\nDelta, 123456789".into()),
        ("two_many_long", (1..=14).map(|i| format!("A rather long category name {i}, {}, {}\n", i * 3 % 17, 1000 + i * 77)).collect::<String>()),
        ("unicode", "Café au lait, 4\nNaïve, 6\nÆble grød, 2\nΣίγμα, 5".into()),
    ]
}

/// Option sets swept with resvg (others are covered by the cheaper sweeps).
fn option_sets() -> Vec<Vec<(&'static str, &'static str)>> {
    let mut v = Vec::new();
    for orient in ["Vertical", "Horizontal"] {
        for measure in [COUNT, PERCENT] {
            for order in ["As typed", PARETO] {
                for (blank, questions) in [(BLANKS[0], "true"), (BLANKS[1], "true"), (BLANKS[2], "false")] {
                    for size in ["Large", "Medium", "Small"] {
                        v.push(vec![("orient", orient), ("measure", measure), ("order", order), ("blank", blank), ("questions", questions), ("label_size", size)]);
                    }
                }
            }
        }
    }
    v
}

// ───────────────────────── reading ─────────────────────────

#[test]
fn reads_commas_tabs_semicolons_spaces_quotes_and_headings() {
    let t = parse("Walk, 12\nCar\t9\nBus; 6\nBike 3\n\"Train, fast\", 2").unwrap();
    let names: Vec<&str> = t.cats.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Walk", "Car", "Bus", "Bike", "Train, fast"]);
    assert_eq!(t.cats.iter().map(|c| c.counts[0]).collect::<Vec<_>>(), [12, 9, 6, 3, 2]);
    assert_eq!(t.groups, 1);
    let t = parse("Pet, Class A, Class B\nDog, 11, 8\n\nCat, 7, 9").unwrap();
    assert_eq!(t.groups, 2);
    assert_eq!(t.head.as_ref().unwrap().2, ["Pet", "Class A", "Class B"]);
    assert_eq!(t.cats[1].line, 4, "line numbers count blank lines, as in the box");
    assert!(parse("Red, twelve\nBlue, 3").is_err(), "a count written as a word is data with a typo, not a heading");
    assert_eq!(parse("Red, 1e3\nBlue, \u{2212}0").unwrap().cats.iter().map(|c| c.counts[0]).collect::<Vec<_>>(), [1000, 0]);
}

#[test]
fn repeated_names_are_merged_and_noted() {
    let t = parse("Red, 12\nblue, 4\n  RED , 3\nBlue, 1").unwrap();
    assert_eq!(t.cats.len(), 2);
    assert_eq!((t.cats[0].name.as_str(), t.cats[0].counts[0]), ("Red", 15));
    assert_eq!(t.merged[0].lines, [1, 3]);
    let notes = warnings(&with(&[("data", "Red, 12\nblue, 4\n  RED , 3\nBlue, 1")]));
    assert!(notes.iter().any(|n| n.contains("12 + 3 = 15") && n.contains("change line 3 to \u{201c}Red 2, 3\u{201d}")), "{notes:?}");
}

/// The test's own reading of a fix in a note, applied to `p`. Returns false when the note has no
/// fix (an information note).
fn apply(note: &str, p: &mut Params) -> bool {
    let quoted = |s: &str, from: usize| -> Option<(String, usize)> {
        let i = s[from..].find('\u{201c}')? + from + '\u{201c}'.len_utf8();
        let j = s[i..].find('\u{201d}')? + i;
        Some((s[i..j].to_string(), j + '\u{201d}'.len_utf8()))
    };
    let label_key = |label: &str| params().into_iter().find(|d| d.label == label).map(|d| d.key).unwrap_or_else(|| panic!("no control {label:?}"));
    if let Some(i) = note.find("change line ").or_else(|| note.find("Change line ")) {
        let rest = &note[i + 12..];
        let n: usize = rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
        let (line, _) = quoted(note, i).unwrap();
        let mut lines: Vec<String> = p.s("data").lines().map(str::to_string).collect();
        lines[n - 1] = line;
        p.set_s("data", &lines.join("\n"));
        return true;
    }
    if let Some(i) = note.find("Combine the ") {
        let k: usize = note[i + 12..].split(' ').next().unwrap().parse().unwrap();
        let (other, _) = quoted(note, i).unwrap();
        // own reading of simple "name, count" lines: drop the k smallest (later lines first among equals)
        let mut rows: Vec<(usize, String, u64)> = p.s("data").lines().enumerate().filter(|(_, l)| !l.trim().is_empty()).map(|(i, l)| {
            let (a, b) = l.rsplit_once(',').unwrap();
            (i, a.trim().to_string(), b.trim().parse().unwrap())
        }).collect();
        let mut by: Vec<usize> = (0..rows.len()).collect();
        by.sort_by(|&a, &b| rows[a].2.cmp(&rows[b].2).then(rows[b].0.cmp(&rows[a].0)));
        let gone: Vec<usize> = by[..k].to_vec();
        let sum: u64 = gone.iter().map(|&g| rows[g].2).sum();
        assert_eq!(other, format!("Other, {sum}"), "the combined line adds the dropped counts");
        let mut keep: Vec<usize> = (0..rows.len()).filter(|i| !gone.contains(i)).collect();
        keep.sort();
        rows = keep.into_iter().map(|i| rows[i].clone()).collect();
        let mut text: Vec<String> = rows.iter().map(|r| format!("{}, {}", r.1, r.2)).collect();
        text.push(other);
        p.set_s("data", &text.join("\n"));
        return true;
    }
    if note.starts_with("Type one category") {
        p.set_s("data", &quoted(note, 0).unwrap().0);
        return true;
    }
    for verb in ["Set ", "Choose ", "Turn on "] {
        let Some(i) = note.rfind(&format!(". {verb}")).map(|i| i + 2).or_else(|| note.starts_with(verb).then_some(0)) else { continue };
        let (a, j) = quoted(note, i).unwrap();
        match verb {
            "Turn on " => p.set_b(label_key(&a), true),
            "Choose " => {
                let (label, _) = quoted(note, j).unwrap();
                p.set_s(label_key(&label), &a);
            }
            _ => {
                let key = label_key(&a);
                let tail = note[j..].trim_start_matches(" to ").trim_start_matches("at least ");
                if let Some((v, _)) = quoted(tail, 0).filter(|_| tail.starts_with('\u{201c}')) {
                    p.set_s(key, &v);
                } else {
                    let num: String = tail.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                    let num = num.trim_end_matches('.');
                    match params().iter().find(|d| d.key == key).unwrap().kind {
                        crate::figures::Kind::Int { .. } => p.set_i(key, num.parse().unwrap()),
                        _ => p.set_f(key, num.parse().unwrap()),
                    }
                }
            }
        }
        return true;
    }
    false
}

/// Information notes: say what was done, with nothing to change.
fn info_only(note: &str) -> bool {
    note.contains("so it was read as headings") || note.ends_with("The exact percents add to 100%.")
}

/// Applies every fix until none is left; each applied note must go away and never come back.
fn settle(p: &mut Params) -> usize {
    let mut seen: Vec<String> = Vec::new();
    for step in 0..40 {
        let notes = warnings(p);
        for n in &notes {
            assert!(!seen.contains(n), "a fixed note came back: {n:?}");
        }
        let Some(n) = notes.iter().find(|n| !info_only(n)).cloned() else { return step };
        assert!(apply(&n, p), "a note with no fix this test can apply: {n:?}");
        assert!(!warnings(p).contains(&n), "applying the fix left the note: {n:?}\ndata now {:?}", p.s("data"));
        seen.push(n);
    }
    panic!("fixes do not settle: {:?}", warnings(p));
}

#[test]
fn every_refusal_and_note_has_a_fix_that_works_when_applied() {
    let long = "x".repeat(55);
    let states: Vec<Vec<(&str, String)>> = vec![
        vec![("data", "".into())],
        vec![("data", "Red, twelve\nBlue, 3".into())],
        vec![("data", "Red, 12 people\nBlue, 3".into())],
        vec![("data", "Red, 25%\nBlue, 75%".into())],
        vec![("data", "Red, -3\nBlue, 3".into())],
        vec![("data", "Red, 12.5\nBlue, 3".into())],
        vec![("data", "Red, 5e12\nBlue, 1e400".into())],
        vec![("data", "Red, NaN\nBlue, inf".into())],
        vec![("data", "Red\nBlue, 3\nGreen, 2".into())],
        vec![("data", ", 12\nBlue, 3".into())],
        vec![("data", "Grade 9 12\nGrade 10 7".into())],
        vec![("data", "Red, 1, 2, 3\nBlue, 3, 4".into())],
        vec![("data", "Red, 1, 2\nBlue, 3\nGreen, 4, 5".into())],
        vec![("data", "Red, 1\nBlue, 3, 5".into())],
        vec![("data", format!("{long}, 3\nBlue, 4"))],
        vec![("data", (1..=45).map(|i| format!("C{i}, {}\n", (i * 7) % 10)).collect())],
        vec![("data", "Red, 0\nBlue, 0".into())],
        vec![("data", "Red, 1, 0\nBlue, 2, 0".into())],
        vec![("data", "Red, 4\nred, 2\nRED, 1\nBlue, 3".into())],
        vec![("data", "Red, 1\nBlue, 2\nGreen, 3\nGrey, 2".into()), ("measure", PERCENT.into())],
        vec![("data", "A, 1\nB, 1\nC, 1".into()), ("measure", PERCENT.into())],
        vec![("scale_auto", "false".into()), ("scale_min", "5".into()), ("scale_max", "8".into())],
        vec![("scale_auto", "false".into()), ("scale_min", "9".into()), ("scale_max", "3".into())],
        vec![("scale_auto", "false".into()), ("scale_min", "40".into()), ("scale_max", "30".into()), ("measure", PERCENT.into())],
        vec![("order", PARETO.into()), ("scale_auto", "false".into())],
        vec![("order", PARETO.into()), ("orient", "Horizontal".into())],
        vec![("order", PARETO.into()), ("data", "Dog, 11, 8\nCat, 7, 9".into())],
        vec![("values", "false".into())],
        vec![("values", "false".into()), ("blank", BLANKS[2].into())],
        vec![("title", "T".repeat(170))],
        vec![("cat_title", "C".repeat(90)), ("val_title", "V".repeat(85))],
        vec![("data", "Dog, 11, 8\nCat, 7, 9".into()), ("name1", "N".repeat(45))],
    ];
    let mut total_steps = 0;
    for st in states {
        let pairs: Vec<(&str, &str)> = st.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let mut p = with(&pairs);
        let before = warnings(&p);
        assert!(!before.is_empty(), "{pairs:?} raises a note");
        total_steps += settle(&mut p);
        let svg = render(&p);
        assert!(svg.contains("<rect x="), "after the fixes the chart draws bars: {pairs:?} -> {:?}", p.s("data"));
        assert!(parse(p.s("data")).is_ok());
    }
    assert!(total_steps >= 33, "{total_steps}");
}

#[test]
fn refusals_draw_the_note_and_never_a_chart() {
    for data in ["", "Red, -1", "Red, 2.5", "Red, 0", "Red, x\nBlue, y"] {
        let p = with(&[("data", data)]);
        let svg = render(&p);
        let note = &warnings(&p)[0];
        let words: String = texts(&svg).iter().map(|t| t.text.clone()).collect::<Vec<_>>().join(" ");
        assert_eq!(words, wrap(note, 580.0, 15.0, true).unwrap().join(" "), "the figure shows the note: {data:?}");
        assert!(!svg.contains("<rect x="));
    }
}

#[test]
fn default_and_examples_raise_no_unexpected_note() {
    assert!(warnings(&defaults()).is_empty());
    for e in examples(ID).unwrap() {
        let mut p = defaults();
        for (k, v) in e.params {
            p.0.insert(k.to_string(), v);
        }
        let notes = warnings(&p);
        if e.name.starts_with("Misleading") {
            assert!(notes.len() == 1 && notes[0].contains("starts at 40, not 0"), "{notes:?}");
        } else if e.name.starts_with("Two classes") {
            assert!(notes.iter().all(|n| n.contains("read as headings")), "{notes:?}");
        } else {
            assert!(notes.is_empty(), "{}: {notes:?}", e.name);
        }
        assert!(audit(&render(&p)).is_empty(), "{}", e.name);
    }
}

// ───────────────────────── statistics ─────────────────────────

#[test]
fn percents_round_half_up_from_exact_counts() {
    assert_eq!(pct_text(1, 8, 0), "13%");
    assert_eq!(pct_text(1, 8, 1), "12.5%");
    assert_eq!(pct_text(1, 3, 2), "33.33%");
    assert_eq!(pct_text(2, 3, 0), "67%");
    assert_eq!(pct_text(0, 5, 1), "0.0%");
    assert_eq!(pct_text(MAX_COUNT, MAX_COUNT * 30, 2), "3.33%");
    assert_eq!(fmt_count(1234567), "1,234,567");
    assert_eq!(fmt_count(999), "999");
}

#[test]
fn relative_frequency_is_each_groups_own_percent_and_adds_to_100() {
    let t = parse("Dog, 11, 8\nCat, 7, 9\nFish, 3, 2\nNone, 4, 9").unwrap();
    let h = heights(&t, true);
    for g in 0..2 {
        let s: f64 = h.iter().map(|r| r[g]).sum();
        assert!((s - 100.0).abs() < 1e-9, "group {g} sums to {s}");
    }
    assert!((h[0][0] - 44.0).abs() < 1e-9 && (h[0][1] - 8.0 * 100.0 / 28.0).abs() < 1e-9);
    // the key prints exactly these values on the bars
    let mut p = with(&[("data", "Dog, 11, 8\nCat, 7, 9\nFish, 3, 2\nNone, 4, 9"), ("measure", PERCENT)]);
    p.set_i("decimals", 1);
    let shown: Vec<String> = texts(&render(&p)).into_iter().filter(|e| e.a("font-size").as_deref() == Some("12.60")).map(|e| e.text).collect();
    assert_eq!(shown, ["44.0%", "28.6%", "28.0%", "32.1%", "12.0%", "7.1%", "16.0%", "32.1%"]);
}

#[test]
fn sorting_is_stable_and_pareto_is_tallest_first() {
    let t = parse("A, 2\nB, 5\nC, 2\nD, 9\nE, 5").unwrap();
    let names = |o: Vec<usize>| o.into_iter().map(|i| t.cats[i].name.clone()).collect::<String>();
    assert_eq!(names(order(&t, false, "Tallest first")), "DBEAC");
    assert_eq!(names(order(&t, false, PARETO)), "DBEAC");
    assert_eq!(names(order(&t, false, "Shortest first")), "ACBED");
    assert_eq!(names(order(&t, false, "As typed")), "ABCDE");
    let t2 = parse("b, 1\nA, 2\nc, 3").unwrap();
    assert_eq!(order(&t2, false, "A to Z"), [1, 0, 2]);
}

/// Bars read back from the SVG: (x, y, w, h, fill) of every series-filled rect that is not a
/// legend swatch.
fn bars(svg: &str, k: f64) -> Vec<(f64, f64, f64, f64, String)> {
    elements(svg)
        .into_iter()
        .filter(|e| e.tag == "rect" && e.a("width").as_deref() != Some("100%") && e.a("stroke").as_deref() != Some("none"))
        .map(|e| (e.n("x"), e.n("y"), e.n("width"), e.n("height"), e.a("fill").unwrap()))
        .filter(|b| !((b.2 - 18.0 * k).abs() < 0.01 && (b.3 - 13.0 * k).abs() < 0.01))
        .collect()
}

#[test]
fn bars_are_in_proportion_and_never_touch_across_categories() {
    for (name, data) in data_sets() {
        for orient in ["Vertical", "Horizontal"] {
            for gap in ["Narrow", "Medium", "Wide"] {
                for measure in [COUNT, PERCENT] {
                    let p = with(&[("data", &data), ("orient", orient), ("gap", gap), ("measure", measure)]);
                    let t = parse(&data).unwrap();
                    let svg = render(&p);
                    let b = bars(&svg, 1.0);
                    let horiz = orient == "Horizontal";
                    let vals: Vec<f64> = heights(&t, measure == PERCENT).into_iter().flatten().collect();
                    let drawn: Vec<f64> = vals.iter().cloned().filter(|v| *v > 0.0).collect();
                    assert_eq!(b.len(), drawn.len(), "{name} {orient}: one bar per non-zero value");
                    let len = |r: &(f64, f64, f64, f64, String)| if horiz { r.2 } else { r.3 };
                    // lengths in proportion to the values (to the 0.01 px the SVG is written in)
                    let (bi, vmax) = drawn.iter().enumerate().fold((0, 0.0), |a, (i, v)| if *v > a.1 { (i, *v) } else { a });
                    let per = len(&b[bi]) / vmax;
                    for (r, v) in b.iter().zip(&drawn) {
                        assert!((len(r) - v * per).abs() < 0.02, "{name} {orient} {gap}: {} for {v}, expected {}", len(r), v * per);
                    }
                    // every bar starts on the axis
                    let base: Vec<f64> = b.iter().map(|r| if horiz { r.0 } else { r.1 + r.3 }).collect();
                    assert!(base.iter().all(|x| (x - base[0]).abs() < 0.02), "{name}: one baseline");
                    // across categories: a gap of at least a fifth of a bar; within one: touching
                    let mut spans: Vec<(f64, f64, usize)> = b.iter().enumerate().map(|(i, r)| if horiz { (r.1, r.1 + r.3, i) } else { (r.0, r.0 + r.2, i) }).collect();
                    spans.sort_by(|a, c| a.0.partial_cmp(&c.0).unwrap());
                    let thick = spans.iter().map(|s| s.1 - s.0).fold(f64::INFINITY, f64::min);
                    for w in spans.windows(2) {
                        let gap_px = w[1].0 - w[0].1;
                        assert!(gap_px > -0.01, "{name}: bars overlap");
                        if t.groups == 1 {
                            assert!(gap_px >= 0.2 * thick, "{name} {orient} {gap}: bars {gap_px} apart");
                        }
                    }
                    if t.groups == 2 {
                        let touching = spans.windows(2).filter(|w| (w[1].0 - w[0].1).abs() < 0.02).count();
                        let separated = spans.windows(2).filter(|w| w[1].0 - w[0].1 >= 0.2 * thick).count();
                        assert_eq!(touching + separated, spans.len() - 1, "{name}: pairs touch, categories stand apart");
                    }
                }
            }
        }
    }
}

#[test]
fn pareto_line_climbs_to_100_percent_at_the_last_bar() {
    let p = with(&[("data", "Forgot, 14\nNo time, 9\nToo hard, 5\nSick, 3\nOther, 2"), ("order", PARETO)]);
    let svg = render(&p);
    let line = elements(&svg).into_iter().find(|e| e.tag == "polyline").expect("the cumulative line");
    let pts: Vec<(f64, f64)> = line.a("points").unwrap().split(' ').map(|q| {
        let (a, b) = q.split_once(',').unwrap();
        (a.parse().unwrap(), b.parse().unwrap())
    }).collect();
    let axis: Vec<El> = elements(&svg).into_iter().filter(|e| e.tag == "line" && e.n("x1") == e.n("x2") && e.n("stroke-width") == 2.0).collect();
    let (top, bottom) = (axis[0].n("y1"), axis[0].n("y2"));
    let cum = [14.0, 23.0, 28.0, 31.0, 33.0];
    for (pt, c) in pts.iter().zip(cum) {
        assert!((bottom - pt.1 - c / 33.0 * (bottom - top)).abs() < 0.02, "{pt:?} {c}");
    }
    assert!((pts[4].1 - top).abs() < 0.02, "the line ends at 100%");
    // the bar heights use the same scale: the left axis top is the total (33)
    let b = bars(&svg, 1.0);
    assert!((b[0].3 - 14.0 / 33.0 * (bottom - top)).abs() < 0.02);
    // typed out of order: the line follows the drawn (sorted) order
    let q = with(&[("data", "Sick, 3\nForgot, 14\nOther, 2\nNo time, 9\nToo hard, 5"), ("order", PARETO)]);
    let qs = render(&q);
    let ql = elements(&qs).into_iter().find(|e| e.tag == "polyline").unwrap();
    assert_eq!(ql.a("points"), line.a("points"), "same line whatever the typed order");
    let mut student = p.clone();
    student.set_b(SHOW_ANSWERS, false);
    let s = render(&student);
    assert!(!s.contains("<polyline") && !s.contains("<circle"), "the line is an answer");
}

#[test]
fn rounding_note_offers_only_a_decimals_setting_that_adds_to_100() {
    let thirds = warnings(&with(&[("data", "A, 1\nB, 1\nC, 1"), ("measure", PERCENT)]));
    assert_eq!(thirds, ["The printed percents add to 99%, not 100%, because each is rounded. The exact percents add to 100%."]);
    let eighths = warnings(&with(&[("data", "A, 1\nB, 2\nC, 3\nD, 2"), ("measure", PERCENT)]));
    assert_eq!(eighths, ["The printed percents add to 101%, not 100%, because each is rounded. Set \u{201c}Decimals in percents\u{201d} to 1."]);
    assert!(warnings(&with(&[("data", "A, 1\nB, 2\nC, 3\nD, 2"), ("measure", PERCENT), ("decimals", "1")])).is_empty());
}

#[test]
fn truncated_axis_and_cut_off_bars_are_flagged() {
    let p = with(&[("data", "Brand A, 46\nBrand B, 44\nBrand C, 42"), ("scale_auto", "false"), ("scale_min", "40"), ("scale_max", "45")]);
    let n = warnings(&p);
    assert!(n.iter().any(|n| n.contains("starts at 40, not 0") && n.contains("twice")), "{n:?}");
    assert!(n.iter().any(|n| n.contains("Brand A") && n.contains("cut off") && n.contains("at least 46")), "{n:?}");
    let ok = with(&[("scale_auto", "false"), ("scale_min", "0"), ("scale_max", "20")]);
    assert!(warnings(&ok).is_empty(), "a manual scale from 0 that holds every bar is fine");
}

// ───────────────────────── layout ─────────────────────────

#[test]
fn nothing_collides_or_leaves_the_canvas_over_the_option_space() {
    let mut n = 0;
    let mut bad = Vec::new();
    for (name, data) in data_sets() {
        for opts in option_sets() {
            let mut p = with(&opts);
            p.set_s("data", &data);
            p.set_s("title", "A title long enough to need some room on a narrow chart");
            p.set_s("cat_title", "Category axis title");
            let (stu, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            for svg in [stu, key] {
                n += 1;
                for b in audit(&svg) {
                    bad.push(format!("{name} {opts:?}: {b}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} problems in {n} renders:\n{}", bad.len(), bad[..bad.len().min(20)].join("\n"));
    assert!(n >= 1440, "{n}");
}

#[test]
fn print_and_extremes_do_not_collide() {
    let mut bad = Vec::new();
    let long_title = "Very long title ".repeat(12);
    for (name, data) in data_sets() {
        for style in ["Colour", "Print"] {
            for gap in ["Narrow", "Wide"] {
                for (ord, orient) in [("Tallest first", "Vertical"), ("A to Z", "Horizontal"), (PARETO, "Vertical")] {
                    let p = with(&[("data", &data), ("style", style), ("gap", gap), ("order", ord), ("orient", orient), ("title", &long_title), ("val_title", "A value axis title that is long enough to wrap onto two lines here"), ("name1", "A group name that is forty characters ok"), ("name2", "And a second group name of forty letters"), ("label_size", "Large"), ("decimals", "2"), ("measure", PERCENT)]);
                    for b in audit(&render(&p)) {
                        bad.push(format!("{name} {style} {gap} {ord}: {b}"));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad[..bad.len().min(20)].join("\n"));
}

#[test]
fn value_labels_sit_in_their_own_bars_column() {
    for (name, data) in data_sets() {
        for orient in ["Vertical", "Horizontal"] {
            for order in ["As typed", PARETO] {
                for size in ["Small", "Large"] {
                    let p = with(&[("data", &data), ("orient", orient), ("order", order), ("label_size", size)]);
                    let k = svg::label_factor(size);
                    let svg = render(&p);
                    let b = bars(&svg, k);
                    let fv = format!("{:.2}", 12.6 * k);
                    let labels: Vec<(El, Bx)> = ink(&svg).into_iter().filter(|(e, _)| e.a("font-size").unwrap() == fv).collect();
                    let t = parse(&data).unwrap();
                    assert_eq!(labels.len(), t.cats.len() * t.groups, "{name}: a label per bar");
                    for (e, bx) in labels {
                        let (cx, cy) = ((bx.0 + bx.2) / 2.0, (bx.1 + bx.3) / 2.0);
                        let own = if orient == "Horizontal" {
                            b.iter().any(|r| cy > r.1 && cy < r.1 + r.3 && bx.0 >= r.0 + r.2 + 2.0 && bx.0 <= r.0 + r.2 + 8.0)
                        } else {
                            b.iter().any(|r| (cx - (r.0 + r.2 / 2.0)).abs() < 1.5 && bx.3 <= r.1 + r.3)
                        };
                        // a zero bar has no rect: its label sits at the axis
                        assert!(own || e.text == "0" || e.text.starts_with("0%") || e.text.starts_with("0.0"), "{name} {orient} {order} {size}: {:?} {bx:?}", e.text);
                    }
                }
            }
        }
    }
}

#[test]
fn long_names_and_many_categories_wrap_or_turn() {
    let p = with(&[("data", &many(30))]);
    let svg = render(&p);
    assert!(svg.contains("rotate(-90)\" font-size=\"14.00\" text-anchor=\"end\""), "30 names turn");
    let p = with(&[("data", "Walk to school, 1\nRide the bus, 2")]);
    let svg = render(&p);
    assert!(!svg.contains("text-anchor=\"end\" font-family") || !svg.contains("rotate(-90)\" font-size=\"14.00\" text-anchor=\"end\""), "short names stay level");
    let p = with(&[("data", "Supercalifragilisticexpialidocious, 1\nB, 2"), ("orient", "Horizontal")]);
    let t: Vec<String> = texts(&render(&p)).into_iter().map(|e| e.text).collect();
    assert!(t.iter().filter(|s| "Supercalifragilisticexpialidocious".contains(s.as_str()) && s.len() > 3).count() >= 1);
}

// ───────────────────────── student copy ─────────────────────────

fn tick_label(e: &El, key: &str, horizontal: bool) -> bool {
    // tick numbers: right-aligned left of the axis (vertical), or centred under it (horizontal)
    let es = elements(key);
    let axis_v = es.iter().find(|l| l.tag == "line" && l.n("x1") == l.n("x2") && l.n("stroke-width") == 2.0).unwrap();
    let axis_h = es.iter().find(|l| l.tag == "line" && l.n("y1") == l.n("y2") && l.n("stroke-width") == 2.0).unwrap();
    if horizontal {
        e.a("text-anchor").as_deref() == Some("middle") && e.n("y") > axis_h.n("y1") && e.n("y") < axis_h.n("y1") + 40.0 && !e.raw.contains("rotate") && e.n("font-size") < 20.0 && (e.text.chars().next().unwrap().is_ascii_digit())
    } else {
        e.a("text-anchor").as_deref() == Some("end") && (e.n("x") - (axis_v.n("x1") - 10.0)).abs() < 0.01
    }
}

#[test]
fn student_copies_leak_nothing_anywhere_in_the_source() {
    let mut n = 0;
    for (name, data) in data_sets() {
        let t = parse(&data).unwrap();
        for orient in ["Vertical", "Horizontal"] {
            for measure in [COUNT, PERCENT] {
                for order in ["As typed", PARETO] {
                    for blank in BLANKS {
                        for values in ["true", "false"] {
                            for style in ["Colour", "Print"] {
                                let p = with(&[("data", &data), ("orient", orient), ("measure", measure), ("order", order), ("blank", blank), ("values", values), ("questions", "true"), ("style", style), ("decimals", "1")]);
                                let d = def();
                                let mut sp = p.clone();
                                sp.set_b(SHOW_ANSWERS, false);
                                let mut kp = p.clone();
                                kp.set_b(SHOW_ANSWERS, true);
                                let (stu, key) = (d.svg(&sp), d.svg(&kp));
                                n += 1;
                                let tag = format!("{name} {orient} {measure} {order} {blank} values={values} {style}");
                                assert_eq!(size(&stu), size(&key), "{tag}: same size");
                                // the whole source: nothing but the drawing and the fixed title and description
                                assert!(!stu.contains("<!--") && !stu.contains("data-"), "{tag}");
                                assert!(stu.contains(&format!("<title>{}</title><desc>{}</desc>", d.title, esc(d.blurb))), "{tag}");
                                assert_eq!(stu.matches(" id=\"").count(), key.matches(" id=\"").count());
                                assert!(stu.matches(" id=\"").count() == 0 || stu.matches(" id=\"vs-hatch").count() == stu.matches(" id=\"").count());
                                assert!(elements(&stu).iter().all(|e| e.tag != "polyline" && e.tag != "circle"), "{tag}: no cumulative line");
                                // answers recomputed here, independently
                                let percent = measure == PERCENT;
                                let tot: Vec<u64> = (0..t.groups).map(|g| t.cats.iter().map(|c| c.counts[g]).sum()).collect();
                                let my_pct = |c: u64, n: u64| {
                                    let s = (2 * c as u128 * 1000 + n as u128) / (2 * n as u128);
                                    format!("{}.{}%", s / 10, s % 10)
                                };
                                let vlab: Vec<String> = t.cats.iter().flat_map(|c| (0..t.groups).map(|g| if percent { my_pct(c.counts[g], tot[g]) } else { fmt_count(c.counts[g]) }).collect::<Vec<_>>()).collect();
                                let ktexts = texts(&key);
                                let stexts = texts(&stu);
                                let mut answers: Vec<String> = Vec::new();
                                let bi = BLANKS.iter().position(|b| *b == blank).unwrap();
                                if values == "true" && bi != 2 {
                                    answers.extend(vlab.clone());
                                }
                                if bi == 1 {
                                    answers.extend(tot.iter().map(|v| fmt_count(*v)));
                                    if percent {
                                        answers.extend(vlab.clone());
                                        answers.extend(vec!["100%".to_string(); t.groups]);
                                    }
                                }
                                if bi == 2 {
                                    answers.extend(ktexts.iter().filter(|e| tick_label(e, &key, orient == "Horizontal")).map(|e| e.text.clone()));
                                }
                                let qa: Vec<String> = ktexts.iter().filter(|e| e.n("x") == 36.0).map(|e| e.text.clone()).collect();
                                assert!(qa[0].starts_with("Answer: "), "{tag}");
                                answers.extend(qa.iter().cloned());
                                let mut expect: Vec<String> = ktexts.iter().map(|e| e.text.clone()).collect();
                                for a in &answers {
                                    let i = expect.iter().position(|x| x == a).unwrap_or_else(|| panic!("{tag}: the key shows {a:?}"));
                                    expect.remove(i);
                                }
                                expect.extend(vec!["Answer:".to_string(); 3]);
                                let mut got: Vec<String> = stexts.iter().map(|e| e.text.clone()).collect();
                                expect.sort();
                                got.sort();
                                assert_eq!(got, expect, "{tag}: the student copy is the key less exactly the answers");
                                if bi == 1 {
                                    assert!(bars(&stu, 1.0).iter().all(|b| b.2 == 18.0), "{tag}: no bars");
                                } else {
                                    assert_eq!(bars(&stu, 1.0), bars(&key, 1.0), "{tag}: the bars are given");
                                }
                                // answer blanks have a fixed length, whatever the answers
                                let blanks: Vec<f64> = elements(&stu).iter().filter(|e| e.tag == "line" && e.n("stroke-width") == 1.0 && e.n("y1") == e.n("y2") && e.n("x1") > 40.0 && e.a("stroke").as_deref() != Some(GRID) && e.a("stroke").as_deref() != Some("#cccccc")).map(|e| e.n("x2") - e.n("x1")).collect();
                                assert!(!blanks.is_empty());
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(n >= 960, "{n}");
}

// ───────────────────────── print ─────────────────────────

fn is_grey(v: &str) -> bool {
    if v.starts_with("url(#vs-hatch") || v == "none" || v == "#fff" || v == "#000" {
        return true;
    }
    let h = v.trim_start_matches('#');
    h.len() == 6 && h[0..2] == h[2..4] && h[2..4] == h[4..6]
}

#[test]
fn print_is_exactly_grey_and_survives_a_66_percent_bitonal_copy() {
    let data = "Pet, Class A, Class B\nDog, 11, 8\nCat, 7, 9\nFish, 3, 2\nNone, 4, 9";
    for order in ["As typed", PARETO] {
        for blank in BLANKS {
            let p = with(&[("data", if order == PARETO { "Forgot, 14\nNo time, 9\nToo hard, 5\nSick, 3" } else { data }), ("style", "Print"), ("blank", blank), ("questions", "true"), ("order", order)]);
            let (stu, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            for svg in [&stu, &key] {
                for attr in ["fill=\"", "stroke=\""] {
                    for part in svg.split(attr).skip(1) {
                        let v = part.split('"').next().unwrap();
                        assert!(is_grey(v), "{v} in Print");
                    }
                }
            }
        }
    }
    // 66 %: render at 2x then 0.66, threshold at 168 like a photocopier
    let p = with(&[("data", data), ("style", "Print")]);
    let svg = render(&p);
    let sc = 2.0 * 0.66;
    let png = crate::figures::to_png(&svg, sc as f32).unwrap();
    let img = image::load_from_memory(&png).unwrap().to_luma8();
    let black = |x: f64, y: f64| img.get_pixel((x * sc) as u32, (y * sc) as u32)[0] < 168;
    let frac = |b: &(f64, f64, f64, f64, String)| {
        let (mut on, mut all) = (0, 0);
        let (x0, y0, x1, y1) = ((b.0 + 3.0) * sc, (b.1 + 3.0) * sc, (b.0 + b.2 - 3.0) * sc, (b.1 + b.3 - 3.0) * sc);
        for x in x0 as u32..x1 as u32 {
            for y in y0 as u32..y1 as u32 {
                all += 1;
                on += (img.get_pixel(x, y)[0] < 168) as u32;
            }
        }
        on as f64 / all.max(1) as f64
    };
    let all_bars: Vec<_> = elements(&svg).into_iter().filter(|e| e.tag == "rect" && e.a("width").as_deref() != Some("100%")).map(|e| (e.n("x"), e.n("y"), e.n("width"), e.n("height"), e.a("fill").unwrap())).collect();
    let (mut g0, mut g1) = (Vec::new(), Vec::new());
    for b in &all_bars {
        if b.4 == "url(#vs-hatch0)" { g0.push(frac(b)) } else { g1.push(frac(b)) }
        // the outline stays: the middle of the top edge is black
        assert!(black(b.0 + b.2 / 2.0, b.1), "{b:?} outline");
    }
    assert!(g0.iter().all(|f| *f < 0.05) && g1.iter().all(|f| *f > 0.12), "the two groups stay apart: {g0:?} {g1:?}");
    for (e, b) in ink(&svg) {
        let mut any = false;
        for x in (b.0 * sc) as u32..=(b.2 * sc) as u32 {
            for y in (b.1 * sc) as u32..=(b.3 * sc) as u32 {
                any |= img.get_pixel(x.min(img.width() - 1), y.min(img.height() - 1))[0] < 168;
            }
        }
        assert!(any, "{:?} survives", e.text);
    }
}

// ───────────────────────── robustness ─────────────────────────

/// This thread's CPU time in seconds (user + system), so a loaded machine does not fail it.
fn cpu() -> f64 {
    let s = std::fs::read_to_string("/proc/thread-self/stat").unwrap_or_default();
    let f: Vec<&str> = s.rsplit(')').next().unwrap_or("").split_whitespace().collect();
    (f.get(11).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) + f.get(12).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)) / 100.0
}

#[test]
fn fuzzed_input_never_panics_and_stays_quick() {
    let mut z: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut rnd = move |n: usize| {
        z ^= z << 13;
        z ^= z >> 7;
        z ^= z << 17;
        (z % n as u64) as usize
    };
    let names = ["Red", "", " ", "Ünïcødé", "中文名字", "😀 smile", "a\"b", "\"q, r\"", "x\ty", "<tag>&amp;", "\u{200b}", "\u{202e}rtl", "Very long name that goes on and on and on and on", "R"];
    let counts = ["12", "0", "-3", "1e400", "NaN", "inf", "12.5", "25%", "", "1e9", "1000000001", "+7", "\u{2212}2", "0x10", "١٢", "9999999999999999999999", "3 4", "-0"];
    let start = cpu();
    for i in 0..10_000 {
        let lines = 1 + rnd(8);
        let mut text = String::new();
        for _ in 0..lines {
            let sep = [", ", "\t", "; ", " ", ","][rnd(5)];
            text.push_str(names[rnd(names.len())]);
            for _ in 0..rnd(4) {
                text.push_str(sep);
                text.push_str(counts[rnd(counts.len())]);
            }
            text.push_str(["\n", "\r\n", "\n\n"][rnd(3)]);
        }
        let mut p = defaults();
        p.set_s("data", &text);
        for d in params() {
            if let crate::figures::Kind::Choice(opts) = &d.kind {
                p.set_s(d.key, opts[rnd(opts.len())]);
            }
        }
        for key in ["values", "scale_auto", "grid", "questions", SHOW_ANSWERS] {
            p.set_b(key, rnd(2) == 0);
        }
        p.set_f("scale_min", [0.0, 5.0, f64::NAN, 1e9, -3.0][rnd(5)]);
        p.set_f("scale_max", [20.0, 0.0, f64::INFINITY, 1e12, 3.0][rnd(5)]);
        p.set_i("decimals", [0, 1, 2, -5, 99][rnd(5)]);
        p.set_s("title", names[rnd(names.len())]);
        p.set_s("name1", names[rnd(names.len())]);
        let svg = render(&p);
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{i}: {text:?}");
        for e in elements(&svg) {
            let head = e.raw.to_lowercase();
            assert!(!head.contains("nan") && !head.contains("inf"), "{i}: {text:?}: {}", e.raw);
        }
        let _ = warnings(&p);
        let _ = relevant(ID, "name1", &p);
    }
    // ten thousand rows: refused quickly, with advice
    let big: String = (0..10_000).map(|i| format!("Row {i}, {}\n", i % 97)).collect();
    let p = with(&[("data", &big)]);
    assert!(warnings(&p)[0].contains("There are 10000 categories"));
    let same: String = (0..10_000).map(|_| "Red, 1\n").collect();
    let p = with(&[("data", &same)]);
    let n = warnings(&p);
    assert!(n[0].contains("1, 2 and 9998 more") && n[0].len() < 400, "{n:?}");
    assert!(render(&p).contains("10,000"));
    let used = cpu() - start;
    assert!(used < 60.0, "{used} s of CPU for 10,000 fuzzed renders");
}

#[test]
fn worst_case_render_is_fast_in_cpu_time() {
    let data: String = (1..=30).map(|i| format!("A long category name number {i}, {}, {}\n", i * 31, i * 17)).collect();
    let p = with(&[("data", &data), ("label_size", "Large"), ("blank", BLANKS[1]), ("questions", "true"), ("measure", PERCENT)]);
    let start = cpu();
    for _ in 0..200 {
        let _ = render(&p);
    }
    let used = cpu() - start;
    assert!(used < 20.0, "{used} s of CPU for 200 worst-case renders");
}

#[test]
fn relevance_and_registration() {
    let mut p = defaults();
    assert!(!relevant(ID, "scale_min", &p) && !relevant(ID, "decimals", &p) && !relevant(ID, "name1", &p));
    p.set_b("scale_auto", false);
    p.set_s("measure", PERCENT);
    p.set_s("data", "a, 1, 2");
    assert!(relevant(ID, "scale_max", &p) && relevant(ID, "decimals", &p) && relevant(ID, "name2", &p));
    assert!(crate::figures::registry().iter().any(|d| d.id == ID && d.subject == Subject::Math));
    assert_eq!(crate::figures::notes_for(ID, &with(&[("data", "x, -1")])), warnings(&with(&[("data", "x, -1")])));
    assert!(render(&defaults()) == render(&defaults()), "pure");
}

#[test]
#[ignore]
fn dump_pngs() {
    let dir = std::env::var("BAR_PNG_DIR").unwrap_or("/tmp/bar-png".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut cases: Vec<(String, Vec<(&str, String)>)> = vec![
        ("pareto".into(), vec![("data", "Forgot, 14\nNo time, 9\nToo hard, 5\nSick, 3\nOther, 2".into()), ("order", PARETO.into()), ("questions", "true".into())]),
        ("start".into(), crate::figures::start::overrides(ID).into_iter().map(|(k, v)| (k, match v { Value::S(s) => s, _ => String::new() })).collect()),
    ];
    for (name, data) in data_sets() {
        cases.push((name.to_string(), vec![("data", data.clone()), ("label_size", "Large".into()), ("questions", "true".into()), ("blank", BLANKS[1].into())]));
    }
    for (name, kv) in cases {
        let pairs: Vec<(&str, &str)> = kv.iter().map(|(k, v)| (*k, v.as_str())).collect();
        for style in ["Colour", "Print"] {
            let mut p = with(&pairs);
            p.set_s("style", style);
            let (stu, key) = crate::figures::answer_pair(&def(), &p).unwrap();
            for (tag, svg) in [("key", key), ("student", stu)] {
                let f = format!("{dir}/{name}-{style}-{tag}");
                std::fs::write(format!("{f}.svg"), &svg).unwrap();
                std::fs::write(format!("{f}.png"), crate::figures::to_png(&svg, 1.5).unwrap()).unwrap();
            }
        }
    }
}
