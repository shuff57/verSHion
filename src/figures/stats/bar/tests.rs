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
                if c == GRID || c == "#cccccc" || c == "#999999" || c == PRINT_GRID {
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
fn thousands_separators_are_one_count_not_a_second_group() {
    // J1: the figure prints counts with thousands commas (fmt_count), so a teacher retyping its
    // own output must get the same chart back -- and MAJOR 1 says which reading it used
    let data = "Walk, 1,200\nCar, 2,500\nBus, 3,000";
    let t = parse(data).unwrap();
    assert_eq!(t.groups, 1, "a thousands comma does not start a second group");
    assert_eq!(t.cats.iter().map(|c| c.counts[0]).collect::<Vec<_>>(), [1200, 2500, 3000]);
    let n = warnings(&with(&[("data", data)]));
    assert_eq!(n.len(), 1, "one note for the table: {n:?}");
    assert!(n[0].contains("Lines 1, 2 and 3 give a count"), "names every line: {}", n[0]);
    assert!(n[0].contains("one count") && n[0].contains("Two counts would be \u{201c}Walk, 1, 200\u{201d}"), "{}", n[0]);
    assert!(n[0].contains("change line 1 to \u{201c}Walk, 1200\u{201d}"), "{}", n[0]);
    // the "Red 12" form: the count rides in the first cell, and could also be the name plus a count
    let t = parse("Walk 1,200\nCar 950").unwrap();
    assert_eq!(t.cats.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Walk", "Car"]);
    assert_eq!(t.cats.iter().map(|c| c.counts[0]).collect::<Vec<_>>(), [1200, 950]);
    let n = warnings(&with(&[("data", "Walk 1,200\nCar 950")]));
    assert!(n[0].contains("could also be the category \u{201c}Walk 1\u{201d} with count 200"), "{n:?}");
    // mixed: one line with a thousands comma, one without
    let t = parse("Walk, 1,200\nCar, 950").unwrap();
    assert_eq!(t.cats.iter().map(|c| c.counts[0]).collect::<Vec<_>>(), [1200, 950]);
    assert!(warnings(&with(&[("data", "Walk, 1,200\nCar, 950")])).len() == 1, "only the ambiguous line is noted");
    // two counts, the first written with a thousands comma: no other reading is possible
    let t = parse("Walk, 1,200, 3").unwrap();
    assert_eq!(t.cats[0].counts, [1200, 3]);
    assert!(warnings(&with(&[("data", "Walk, 1,200, 3")])).is_empty(), "three cells are three cells");
    // a comma with a space after it starts the next cell, so two groups still read as typed
    let two = parse("A, 3, 100\nB, 5, 20").unwrap();
    assert_eq!(two.groups, 2);
    assert_eq!(two.cats[0].counts, [3, 100]);
    assert!(warnings(&with(&[("data", "A, 3, 100\nB, 5, 20")])).is_empty());
    // a leading group of three digits is a thousands group too (MED 4: pins `j - i <= 3`)
    let big = parse("A, 150,000\nB, 250,000").unwrap();
    assert_eq!(big.groups, 1, "150,000 is one count");
    assert_eq!(big.cats.iter().map(|c| c.counts[0]).collect::<Vec<_>>(), [150000, 250000]);
    // tab-pasted thousands groups read as one count and are not ambiguous: a tab is a cell
    assert_eq!(parse("Walk\t1 200\nCar\t950").unwrap().cats[0].counts, [1200]);
    assert!(warnings(&with(&[("data", "Walk\t1 200\nCar\t950")])).is_empty(), "a tab leaves no other reading");
    assert_eq!(parse("Walk 12 345\nCar 950").unwrap().cats[0].counts, [12345]);
    // a quoted name is left exactly as typed (LOW): no thousands pass inside quotes
    let q = parse("\"Bus, 12,300 riders\", 4\nWalk, 3").unwrap();
    assert_eq!(q.cats[0].name, "Bus, 12,300 riders");
    assert_eq!(q.cats[0].counts, [4]);
    assert!(warnings(&with(&[("data", "\"Bus, 12,300 riders\", 4\nWalk, 3")])).is_empty());
    // a decimal comma after the name is refused, never read as two groups
    let n = warnings(&with(&[("data", "Walk; 12,5")]));
    assert!(n[0].contains("has a comma inside a number") && n[0].contains("12.5"), "{n:?}");
    // ... and with commas it is noted, not silent (LOW)
    let n = warnings(&with(&[("data", "Walk, 12,5\nCar, 9, 3")]));
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].contains("read as two counts, 12 and 5") && n[0].contains("change line 1 to \u{201c}Walk, 12, 5\u{201d}"), "{}", n[0]);
    // and the advice never pads a thousands line with a ", 0" count
    for d in ["Walk, 1,200\nCar, 2,500", "Walk 1,200\nCar 950", "Walk, 1,200\nCar, 950", "Walk\t1 200\nCar\t950", "A, 3,100\nB, 2,050"] {
        for n in warnings(&with(&[("data", d)])) {
            assert!(!n.contains(", 0"), "{d}: {n}");
        }
    }
}

#[test]
fn retyping_the_figures_own_printed_counts_gives_the_same_chart() {
    let data = "Walk, 1200\nCar, 2500\nBus, 3000";
    let p = with(&[("data", data)]);
    let printed: Vec<String> = texts(&render(&p)).into_iter().filter(|e| e.a("font-size").as_deref() == Some("12.60")).map(|e| e.text).collect();
    assert_eq!(printed, ["1,200", "2,500", "3,000"], "the figure prints its counts with thousands commas");
    let before = parse(data).unwrap();
    let retyped: String = before.cats.iter().zip(&printed).map(|(c, l)| format!("{}, {l}\n", c.name)).collect();
    // MAJOR 1: the retyped table is read as one count per line and says so; its fix keeps them
    let mut p = with(&[("data", &retyped)]);
    let (steps, stuck) = settle(&mut p);
    assert!(steps >= 1 && stuck.is_none(), "{steps} {stuck:?}");
    let after = parse(p.s("data")).unwrap();
    assert_eq!(after.cats, before.cats, "the printed form reads back as the same counts, fix and all");
    assert_eq!(after.groups, 1);
}

#[test]
fn the_thousands_reading_is_always_stated_and_can_be_rewritten_both_ways() {
    // MAJOR 1: `A, 3,100` is 3100 here, and the note says so, names the other reading (3 and 100)
    // and offers a rewrite that keeps the chart
    let p = with(&[("data", "A, 3,100\nB, 2,050")]);
    let t = parse(p.s("data")).unwrap();
    assert_eq!(t.groups, 1);
    assert_eq!(t.cats.iter().map(|c| c.counts[0]).collect::<Vec<_>>(), [3100, 2050]);
    let notes = warnings(&p);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("Lines 1 and 2 give a count in thousands, so each is one count (3,100 and 2,050)"), "{}", notes[0]);
    assert!(notes[0].contains("a comma with no space after it, between groups of three digits, is a thousands separator"), "{}", notes[0]);
    assert!(notes[0].contains("Two counts would be \u{201c}A, 3, 100\u{201d}"), "{}", notes[0]);
    assert!(notes[0].contains("change line 1 to \u{201c}A, 3100\u{201d}"), "{}", notes[0]);
    // the fix keeps the chart and clears the note for every line
    let mut fixed = p.clone();
    let (steps, stuck) = settle(&mut fixed);
    assert!(steps == 2 && stuck.is_none(), "{steps} {stuck:?}");
    assert_eq!(parse(fixed.s("data")).unwrap().cats, t.cats);
    // the other spelling draws the two groups the note named, unchanged in meaning
    let two = parse("A, 3, 100\nB, 2, 50").unwrap();
    assert_eq!(two.groups, 2);
    assert_eq!(two.cats[0].counts, [3, 100]);
    assert_eq!(two.cats[1].counts, [2, 50]);
    // the space form: one count, with the name-and-count reading also named
    let p = with(&[("data", "A 1 200\nB 950")]);
    let notes = warnings(&p);
    assert!(notes[0].contains("could also be the category \u{201c}A 1\u{201d} with count 200"), "{}", notes[0]);
    assert!(notes[0].contains("change line 1 to \u{201c}A, 1200\u{201d}"), "{}", notes[0]);
    // unambiguous lines carry no note: a group of exactly three digits in a cell on its own, and
    // two counts where the first was written with a thousands comma
    for d in ["Walk\t1 200\nCar\t950", "Walk, 1,200, 3", "A, 150,000, 900\nB, 2, 3", "A, 1,200, 5,000\nB, 800, 950"] {
        assert!(warnings(&with(&[("data", d)])).is_empty(), "{d}: {:?}", warnings(&with(&[("data", d)])));
    }
}

#[test]
fn the_too_many_counts_refusal_keeps_every_count() {
    // MAJOR: a line with more than two counts is refused with the limit stated and every typed
    // count named; there is no rewrite that would drop one (the old advice kept only two)
    for (data, line, counts) in [
        ("A, 5, 7, 3\nB, 1, 2", "Line 1 (\u{201c}A, 5, 7, 3\u{201d})", ["5", "7", "3"]),
        ("A, 9, 12,5", "Line 1 (\u{201c}A, 9, 12,5\u{201d})", ["9", "12", "5"]),
        ("B, 3, 4, 9", "Line 1 (\u{201c}B, 3, 4, 9\u{201d})", ["3", "4", "9"]),
        // a thousands count is quoted as it was typed, in every spelling (LOW)
        ("A, 1,200, 3, 4", "Line 1 (\u{201c}A, 1,200, 3, 4\u{201d})", ["1,200", "3", "4"]),
        ("A, 1 200, 3, 4", "Line 1 (\u{201c}A, 1 200, 3, 4\u{201d})", ["1 200", "3", "4"]),
        ("A, 1200, 3, 4", "Line 1 (\u{201c}A, 1200, 3, 4\u{201d})", ["1200", "3", "4"]),
    ] {
        let e = parse(data).unwrap_err();
        assert!(e.contains("this figure takes at most two counts per category"), "{data}: {e}");
        assert!(e.contains(line), "{data}: the line and its cells are named: {e}");
        for c in counts {
            assert!(e.contains(&format!("\u{201c}{c}\u{201d}")), "{data}: {c} is named, not dropped: {e}");
        }
        assert!(!e.contains("change line") && !e.contains("Change line"), "{data}: no rewrite: {e}");
        assert!(!e.contains(", 0"), "{data}: no invented count: {e}");
        assert!(e.contains("second chart"), "{data}: a route that keeps the data: {e}");
    }
    // the refused text still holds every count: nothing was edited on the way
    let data = "A, 5, 7, 3\nB, 1, 2";
    assert!(parse(data).is_err());
    assert!(data.contains(", 3"));
}

#[test]
fn a_counts_mismatch_names_every_line_and_offers_no_rewrite() {
    // MAJOR 2: the refusal must not drop or invent a count (no "change line n to …", no ", 0"), so
    // it names both lines with their counts and offers the two shapes; both routes are checked
    let e = parse("A, 5, 7\nB, 1,200").unwrap_err();
    assert!(e.contains("Line 2 (\u{201c}B, 1,200\u{201d}) holds 1 count (1200), but line 1 (\u{201c}A, 5, 7\u{201d}) holds 2 counts (5 and 7)"), "{e}");
    assert!(e.contains("Either") || e.contains("either add the missing count on the lines that hold fewer, or write one count on every line"), "{e}");
    assert!(e.contains("two counts would be \u{201c}B, 1, 200\u{201d}"), "{e}");
    assert!(!e.contains("change line") && !e.contains("Change line") && !e.contains(", 0"), "{e}");
    // route one: two counts on every line, the thousands group written with a space
    let two = parse("A, 5, 7\nB, 1, 200").unwrap();
    assert_eq!(two.groups, 2);
    assert_eq!(two.cats[0].counts, [5, 7]);
    assert_eq!(two.cats[1].counts, [1, 200]);
    // route two: one count on every line
    assert_eq!(parse("A, 5\nB, 1200").unwrap().groups, 1);
    // the same the other way round, where the thousands line comes first
    let e = parse("A, 1,200\nB, 5, 7").unwrap_err();
    assert!(e.contains("Line 2 (\u{201c}B, 5, 7\u{201d}) holds 2 counts (5 and 7), but line 1 (\u{201c}A, 1,200\u{201d}) holds 1 count (1200)"), "{e}");
    assert!(!e.contains("change line") && !e.contains(", 0"), "a typed count must not be dropped: {e}");
    assert!(e.contains("\u{201c}A, 1,200\u{201d} is one count, 1,200; two counts would be \u{201c}A, 1, 200\u{201d}"), "the line that needs respelling is named too: {e}");
    let two = parse("A, 1, 200\nB, 5, 7").unwrap();
    assert_eq!(two.cats[0].counts, [1, 200]);
    assert_eq!(two.cats[1].counts, [5, 7]);
}

#[test]
fn the_table_the_bars_and_the_answers_print_one_number() {
    // MED 4: one rounding rule everywhere. The table cell for a category, the value on its bar
    // and the key's answer are the same number, and the column's Total row is what its cells add
    // to (100% only when they add up).
    let cases: [(&str, u32); 4] = [
        ("A, 1\nB, 1\nC, 1", 0),
        ("A, 1\nB, 1\nC, 1", 2),
        ("A, 1\nB, 2\nC, 3\nD, 2", 1),
        ("Pet, Class A, Class B\nDog, 11, 8\nCat, 7, 9\nFish, 3, 2\nNone, 4, 9", 1),
    ];
    for (data, d) in cases {
        let mut p = with(&[("data", data), ("measure", PERCENT), ("blank", BLANKS[1]), ("questions", "true")]);
        p.set_i("decimals", d as i64);
        let svg = render(&p);
        let t = parse(data).unwrap();
        let n = t.cats.len();
        // the table's percent cells: fs 14, centred, in columns (same x) and rows (y order)
        let mut cells: Vec<(f64, f64, String)> = texts(&svg)
            .into_iter()
            .filter(|e| e.a("font-size").as_deref() == Some("14.00") && e.a("text-anchor").as_deref() == Some("middle") && e.text.ends_with('%'))
            .map(|e| (e.n("x"), e.n("y"), e.text))
            .collect();
        cells.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap()));
        let mut xs: Vec<f64> = Vec::new();
        for (x, _, _) in &cells {
            if !xs.iter().any(|v| (v - x).abs() < 0.5) {
                xs.push(*x);
            }
        }
        assert_eq!(xs.len(), t.groups, "{data} {d}: one percent column per group: {xs:?}");
        // labels are drawn category by category, so row `row` of group `g` is labels[row * groups + g]
        let labels: Vec<String> = texts(&svg).into_iter().filter(|e| e.a("font-size").as_deref() == Some("12.60") && e.text.ends_with('%')).map(|e| e.text).collect();
        for (g, x) in xs.iter().enumerate() {
            let col: Vec<String> = cells.iter().filter(|(cx, _, _)| (cx - x).abs() < 0.5).map(|(_, _, t)| t.clone()).collect();
            assert_eq!(col.len(), n + 1, "{data} {d}: {col:?}");
            let sum: f64 = col[..n].iter().map(|c| c.trim_end_matches('%').parse::<f64>().unwrap()).sum();
            let total: f64 = col[n].trim_end_matches('%').parse().unwrap();
            assert!((total - sum).abs() < 1e-9, "{data} {d}: the Total {} is what its cells add to ({sum})", col[n]);
            for row in 0..n {
                assert_eq!(&col[row], &labels[row * t.groups + g], "{data} {d}: row {row} of group {g}: table against label");
            }
        }
        // question 3 asks about the first drawn category: its answer is the number in that cell
        let ans: Vec<String> = texts(&svg).into_iter().filter(|e| e.n("x") == 36.0).map(|e| e.text).collect();
        let first: Vec<String> = xs.iter().map(|x| cells.iter().filter(|(cx, _, _)| (cx - x).abs() < 0.5).map(|(_, _, t)| t.clone()).next().unwrap()).collect();
        if t.groups == 1 {
            assert_eq!(ans[2], format!("Answer: {}", first[0]), "{data} {d}");
        } else {
            for (g, cell) in first.iter().enumerate() {
                assert!(ans[2].contains(cell), "{data} {d}: group {g} answer {:?} against cell {cell}", ans[2]);
            }
        }
    }
    // a sweep: every cell is its own value's half-up rounding, so a column that does not add up
    // shows that plainly in its Total (no silent adjustment)
    for total in 1..=30u64 {
        for k in 1..=5u64 {
            let cs: Vec<u64> = (0..k).map(|i| total / k + u64::from(i < total % k)).collect();
            let total: u64 = cs.iter().sum();
            for d in 0..=2u32 {
                let col = pct_column(&cs, total, d);
                for (c, s) in cs.iter().zip(&col) {
                    assert_eq!(*s, pct_scaled(*c, total, d), "{cs:?} {d}");
                }
            }
        }
    }
}

#[test]
fn the_percent_axis_ticks_carry_the_percent_sign() {
    // MED 4: a mutant that drops the sign from the tick must fail
    let p = with(&[("data", "A, 9\nB, 1"), ("measure", PERCENT)]);
    let ticks: Vec<String> = texts(&render(&p)).into_iter().filter(|e| e.a("text-anchor").as_deref() == Some("end")).map(|e| e.text).collect();
    assert!(ticks.len() >= 3, "{ticks:?}");
    assert!(ticks.iter().all(|t| t.ends_with('%')), "{ticks:?}");
    assert!(ticks.contains(&"100%".to_string()) && ticks.contains(&"0%".to_string()), "{ticks:?}");
}

/// The counts at the end of a line as a reader who ignores quotes would split them: what a rewrite
/// of that line must not lose.
fn trailing_counts(line: &str) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    for cell in line.split(',').map(str::trim).rev() {
        match cell.trim_matches('"').parse::<u64>() {
            Ok(v) => out.push(v),
            Err(_) => break,
        }
    }
    out.reverse();
    out
}

#[test]
fn no_advice_rewrite_drops_a_typed_count() {
    // MAJOR: an unmatched quote is a literal character, so these keep their count and their name
    for (data, name, count) in [
        ("Don\"t know, 5\nB, 4", "Don\"t know", 5u64),
        ("55\" TV, 12\nB, 4", "55\" TV", 12),
        ("a\"b, 3\nB, 4", "a\"b", 3),
    ] {
        let t = parse(data).unwrap_or_else(|e| panic!("{data}: {e}"));
        assert_eq!(t.cats[0].name, name, "{data}");
        assert_eq!(t.cats[0].counts[0], count, "{data}: the count is not folded into the name");
        assert_eq!(t.cats[1].counts, [4], "{data}");
        assert!(warnings(&with(&[("data", data)])).is_empty(), "{data}: {:?}", warnings(&with(&[("data", data)])));
    }
    // the same names inside a balanced quoted cell still read as one name
    assert_eq!(parse("\"Train, fast\", 2\nWalk, 3").unwrap().cats[0].name, "Train, fast");
    // and no rewrite in any state may lose a count: applying one keeps every trailing count
    let long = "x".repeat(55);
    let states: Vec<Vec<(&str, String)>> = vec![
        vec![("data", ", 12\nBlue, 3".into())],
        vec![("data", "Grade 9 12\nGrade 10 7".into())],
        vec![("data", "Red, 4\nred, 2\nRED, 1\nBlue, 3".into())],
        vec![("data", format!("{long}, 3\nBlue, 4"))],
        vec![("data", "Dog, 11, 8\nCat, 7, 9".into()), ("name1", "N".repeat(45))],
    ];
    let mut checked = 0;
    for st in states {
        let pairs: Vec<(&str, &str)> = st.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let mut p = with(&pairs);
        for _ in 0..10 {
            let notes = warnings(&p);
            let Some(note) = notes.iter().find(|n| n.contains("change line ") || n.contains("Change line ")).cloned() else { break };
            let i = note.find("change line ").or_else(|| note.find("Change line ")).unwrap();
            let num: usize = note[i + 12..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
            let old_line = p.s("data").lines().nth(num - 1).unwrap().to_string();
            let before = trailing_counts(&old_line);
            assert!(apply(&note, &mut p), "{note}");
            let new_line = p.s("data").lines().nth(num - 1).unwrap().to_string();
            let after = trailing_counts(&new_line);
            for v in &before {
                assert!(after.contains(v), "the rewrite {note:?} lost the count {v}: {old_line:?} -> {new_line:?}");
            }
            checked += 1;
        }
    }
    assert!(checked >= 5, "the sweep applied {checked} rewrites");
}

#[test]
fn a_typos_count_with_a_word_is_refused_not_swallowed() {
    // MED: a first line whose cell is a mistyped count plus a word, or a vague quantity, must be
    // refused by line -- it never becomes the headings while its row disappears from the chart
    for (data, cell) in [
        ("A, l23 students\nB, 5", "l23 students"),
        ("A, 1O students\nB, 5", "1O students"),
        ("A, \u{2014} students\nB, 5", "\u{2014} students"),
        ("A, several\nB, 5", "several"),
        ("A, lots\nB, 5", "lots"),
    ] {
        let t = parse(data);
        assert!(t.is_err(), "{data}: refused, never the headings");
        let e = t.unwrap_err();
        assert!(e.contains(cell), "{data}: the cell is named: {e}");
        assert!(!e.contains("read as headings"), "{data}: {e}");
        assert!(e.contains("Group 1 name") || e.contains("is not a number"), "{data}: a route that works: {e}");
        assert!(!render(&with(&[("data", data)])).contains("<rect x="), "{data}: no chart is drawn");
    }
    // `12 students` is a number and a word: data, with the fix that keeps the count
    let mut p = with(&[("data", "A, 12 students\nB, 5")]);
    let (steps, stuck) = settle(&mut p);
    assert_eq!((steps, stuck.is_none()), (1, true), "{steps} {stuck:?}");
    assert_eq!(state_pairs(&parse(p.s("data")).unwrap()), rows(&[("A", &[12]), ("B", &[5])]));
    // a genuine headings line still reads as headings
    let t = parse("Grade, 9th grade, 10th grade\nA, 1, 2").unwrap();
    assert_eq!(t.head.unwrap().2, ["Grade", "9th grade", "10th grade"]);
}

#[test]
fn a_names_quotes_are_kept() {
    // LOW: a balanced literal quote stays in the printed name; only a properly quoted cell loses
    // its outer pair
    let data = "5'2\" - 5'4\", 6\n5'4\" - 5'6\", 9";
    let t = parse(data).unwrap();
    assert_eq!(t.cats[0].name, "5'2\" - 5'4\"");
    assert_eq!(t.cats[1].name, "5'4\" - 5'6\"");
    assert_eq!(t.cats.iter().map(|c| c.counts[0]).collect::<Vec<_>>(), [6, 9]);
    assert!(warnings(&with(&[("data", data)])).is_empty(), "{:?}", warnings(&with(&[("data", data)])));
    let joined: String = texts(&render(&with(&[("data", data)]))).into_iter().map(|e| e.text).collect();
    assert!(joined.contains("5'2\""), "the printed name keeps its quote: {joined:?}");
    // a properly quoted cell still loses its outer pair, and an inner pair stays
    assert_eq!(parse("\"Train, fast\", 2\nWalk, 3").unwrap().cats[0].name, "Train, fast");
    assert_eq!(parse("a\"b\"c, 3\nB, 4").unwrap().cats[0].name, "a\"b\"c");
}

#[test]
fn an_ambiguous_thousands_fix_keeps_the_name() {
    // LOW: the merged line's fix used to read `change line 2 to ", 3400"`, which has no name
    let mut p = with(&[("data", "A, 1200\nA, 3,400")]);
    let notes = warnings(&p);
    let amb = notes.iter().find(|n| n.contains("Two counts would be")).unwrap_or_else(|| panic!("{notes:?}"));
    assert!(amb.contains("change line 2 to \u{201c}A, 3400\u{201d}"), "{amb}");
    let (steps, stuck) = settle(&mut p);
    assert!(steps >= 2 && stuck.is_none(), "{steps} {stuck:?}");
    let cats = parse(p.s("data")).unwrap().cats;
    assert!(cats.iter().all(|c| !c.name.trim().is_empty()), "every row keeps a name: {cats:?}");
}

#[test]
fn the_too_large_advice_names_a_unit_that_works() {
    // LOW: the unit must bring the count under the cap, else the plain limit (a mutant that always
    // says "thousands" must fail)
    let note = |v: &str| warnings(&with(&[("data", &format!("A, {v}\nB, 3"))]))[0].clone();
    assert!(note("5e9").contains("count in thousands"), "{}", note("5e9"));
    assert!(note("1e15").contains("count in millions"), "{}", note("1e15"));
    assert!(note("1e17").contains("count in billions"), "{}", note("1e17"));
    assert!(note("9999999999999999999999").contains("type counts of at most 1000000000"), "{}", note("9999999999999999999999"));
}

#[test]
fn a_first_line_that_cannot_be_read_is_refused_not_swallowed() {
    // MED 3: a first line is headings only when every cell is a clear name; otherwise it is data and
    // the bad cell is refused by line and text, with both routes that keep the data
    for (data, cell) in [
        ("A, \u{2014}\nB, 5", "\u{2014}"),
        ("A, \u{ff11}\u{ff12}\nB, 5", "\u{ff11}\u{ff12}"),
        ("A, N/A\nB, 5", "N/A"),
        ("Smith; Jones, 3\nB, 5", "Jones, 3"),
        ("Grade, 1st, 2nd\nA, 3, 4", "1st"),
        ("Age, 0-9, 10-19\nA, 3, 4", "0-9"),
        ("Quarter, Q1, Q2\nA, 3, 4", "Q1"),
        ("Illness, COVID-19, Flu\nA, 3, 4", "COVID-19"),
        ("Grade, K, 1\nA, 3", "K"),
    ] {
        let t = parse(data);
        assert!(t.is_err(), "{data}: not read as headings");
        let e = t.unwrap_err();
        assert!(e.starts_with("Line 1 (") || e.contains("Line 1 ("), "{data}: {e}");
        assert!(e.contains(cell), "{data}: the bad cell {cell:?} is named: {e}");
        assert!(!e.contains("read as headings"), "{data}: {e}");
        assert!(!e.contains("change line") && !e.contains("Change line"), "{data}: no data is rewritten away: {e}");
        assert!(e.contains("type the group names in \u{201c}Group 1 name\u{201d} and \u{201c}Group 2 name\u{201d}") || e.contains("is not a number"), "{data}: a route that works: {e}");
        // the row is never silently lost: the figure shows the refusal
        let svg = render(&with(&[("data", data)]));
        assert!(!svg.contains("<rect x="), "{data}: no chart is drawn");
    }
    // a number word is a count with a typo, and its fix keeps the value it names (MED 3)
    let e = parse("A, dozen\nB, 5").unwrap_err();
    assert!(e.contains("\u{201c}dozen\u{201d} is a number written as a word") && e.contains("change line 1 to \u{201c}A, 12\u{201d}"), "{e}");
    let mut p = with(&[("data", "A, dozen\nB, 5")]);
    let (steps, stuck) = settle(&mut p);
    assert!(steps == 1 && stuck.is_none(), "{steps} {stuck:?}");
    assert_eq!(state_pairs(&parse(p.s("data")).unwrap()), rows(&[("A", &[12]), ("B", &[5])]));
    // a genuine headings line still reads as headings, digits in a name and all
    for (data, cells) in [
        ("Grade, 9th grade, 10th grade\nA, 1, 2", vec!["Grade", "9th grade", "10th grade"]),
        ("Group, Grade 9\nA, 3, 4", vec!["Group", "Grade 9"]),
        ("Response, None, One or more\nA, 3, 4", vec!["Response", "None", "One or more"]),
        ("Class, none of these, all of these\nA, 3, 4", vec!["Class", "none of these", "all of these"]),
    ] {
        let t = parse(data).unwrap_or_else(|e| panic!("{data}: {e}"));
        assert_eq!(t.head.expect("headings").2, cells, "{data}");
    }
}

#[test]
fn a_first_line_with_numbers_is_data_never_headings() {
    // M1: a heading cell holds no number, so a row with a mistyped count is refused by line
    // instead of becoming the headings and losing its category
    for data in ["Walk, l2\nCar, 9", "Walk, 1O\nCar, 9", "A, inf\nB, 3", "A, NaN\nB, 3"] {
        let n = warnings(&with(&[("data", data)]));
        assert_eq!(n.len(), 1, "{data}: {n:?}");
        assert!(n[0].starts_with("Line 1 ("), "{data}: {}", n[0]);
        assert!(n[0].contains("is not a number"), "{data}: {}", n[0]);
        assert!(!n[0].contains("read as headings"), "{data}: {}", n[0]);
        assert!(!n[0].contains(", 0"), "{data}: no count is invented: {}", n[0]);
        assert!(!n[0].contains("change line"), "{data}: no data is changed: {}", n[0]);
        assert!(parse(data).is_err(), "{data} is refused");
    }
    // MED 1: a heading cell is a name whenever the whole cell is not a number, so these are lines
    // of headings -- never a category, and never a name turned into a count of 0
    let cases: Vec<(&str, Vec<&str>, Vec<(String, Vec<u64>)>)> = vec![
        ("Grade, 9th grade, 10th grade\nWalk, 12, 15", vec!["Grade", "9th grade", "10th grade"], rows(&[("Walk", &[12, 15])])),
        ("Grade, Grade 9, Grade 10\nWalk, 12, 15", vec!["Grade", "Grade 9", "Grade 10"], rows(&[("Walk", &[12, 15])])),
        ("Group, None, One or more\nA, 1, 2", vec!["Group", "None", "One or more"], rows(&[("A", &[1, 2])])),
        ("Fruit, Two-year-olds\nApple, 3", vec!["Fruit", "Two-year-olds"], rows(&[("Apple", &[3])])),
    ];
    for (data, cells, pairs) in cases {
        let t = parse(data).unwrap_or_else(|e| panic!("{data}: {e}"));
        let head = t.head.as_ref().unwrap_or_else(|| panic!("{data} reads as headings"));
        assert_eq!(head.2, cells, "{data}");
        assert_eq!(state_pairs(&t), pairs, "{data}");
        for n in warnings(&with(&[("data", data)])) {
            assert!(!n.contains(", 0"), "{data}: a name is never turned into 0: {n}");
            assert!(!n.contains("is not a number"), "{data}: {n}");
        }
    }
    // a heading line naming a group `None` draws that name, not a zero bar
    let p = with(&[("data", "Group, None, One or more\nA, 1, 2")]);
    let names: Vec<String> = texts(&render(&p)).into_iter().map(|e| e.text).collect();
    assert!(names.iter().any(|t| t == "None") && names.iter().any(|t| t == "One or more"), "{names:?}");
    // the name1/name2 route draws the groups those headings named
    let p = with(&[("data", "Walk, 12, 15\nCar, 9, 11"), ("name1", "2019"), ("name2", "2020")]);
    assert!(warnings(&p).is_empty(), "{:?}", warnings(&p));
    let names: Vec<String> = texts(&render(&p)).into_iter().map(|e| e.text).collect();
    assert!(names.iter().any(|t| t == "2019") && names.iter().any(|t| t == "2020"), "{names:?}");
    // MED 1: the same numbers as a first line of data are read as counts, and the note says the
    // line may have been headings with the route to the names
    let p = with(&[("data", "Year, 2019, 2020\nWalk, 12, 15\nCar, 9, 11")]);
    let n = warnings(&p);
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].contains("may be a line of headings") && n[0].contains("delete line 1"), "{}", n[0]);
    assert!(n[0].contains("\u{201c}Group 1 name\u{201d} and \u{201c}Group 2 name\u{201d}"), "{}", n[0]);
    let t = parse("Year, 2019, 2020\nWalk, 12, 15\nCar, 9, 11").unwrap();
    assert_eq!(t.cats[0].name, "Year");
    // the same after deleting it and naming the groups
    let p = with(&[("data", "Walk, 12, 15\nCar, 9, 11"), ("name1", "2019"), ("name2", "2020")]);
    assert_eq!(parse(p.s("data")).unwrap().cats.len(), 2);
    // a count then a word is a data row, and the number is the count
    let data = "Walk, 12 students\nCar, 9\nBus, 6";
    let n = warnings(&with(&[("data", data)]));
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].contains("change line 1 to \u{201c}Walk, 12\u{201d}"), "{}", n[0]);
    let t = parse("Walk, 12 students\nCar, 9\nBus, 6").unwrap_err();
    assert!(t.contains("Walk, 12"), "{t}");
    // a heading line still works when it really holds only words, even a digit in its first cell
    let head = parse("Pet, Class A, Class B\nDog, 11, 8\nCat, 7, 9").unwrap();
    assert_eq!(head.head.unwrap().2, ["Pet", "Class A", "Class B"]);
    let head = parse("Pets 2024, Boys, Girls\nDog, 3, 4").unwrap();
    assert_eq!(head.head.unwrap().2, ["Pets 2024", "Boys", "Girls"]);
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
    note.contains("was read as headings") || note.contains("may be a line of headings")
}

/// Applies every fix until none is left; each applied note must go away and never come back.
/// Stops at a refusal that offers no fix (nothing can be changed without inventing data) and
/// returns that note.
fn settle(p: &mut Params) -> (usize, Option<String>) {
    let mut seen: Vec<String> = Vec::new();
    for step in 0..40 {
        let notes = warnings(p);
        for n in &notes {
            assert!(!seen.contains(n), "a fixed note came back: {n:?}");
        }
        let Some(n) = notes.iter().find(|n| !info_only(n)).cloned() else { return (step, None) };
        if !apply(&n, p) {
            return (step, Some(n));
        }
        assert!(!warnings(p).contains(&n), "applying the fix left the note: {n:?}\ndata now {:?}", p.s("data"));
        seen.push(n);
    }
    panic!("fixes do not settle: {:?}", warnings(p));
}

/// What a state must reach once every recommended fix is applied: the pairs the teacher meant,
/// or a refusal that names the line and offers no invented count.
#[derive(Clone, Debug)]
enum End {
    /// At least one note, then a chart with these (name, counts) pairs.
    Chart(Vec<(String, Vec<u64>)>),
    /// Correct as typed: no note, and these pairs.
    Quiet(Vec<(String, Vec<u64>)>),
    /// Stays refused: the message contains this, and it offers no "change line" fix.
    Refused(&'static str),
}

fn rows(v: &[(&str, &[u64])]) -> Vec<(String, Vec<u64>)> {
    v.iter().map(|(n, c)| (n.to_string(), c.to_vec())).collect()
}

fn state_pairs(t: &Table) -> Vec<(String, Vec<u64>)> {
    t.cats.iter().map(|c| (c.name.clone(), c.counts.clone())).collect()
}

#[test]
fn every_refusal_and_note_has_a_fix_that_keeps_the_meaning() {
    let long = "x".repeat(55);
    let shortened = "x".repeat(40);
    let defaults_pairs = rows(&[("Walk", &[12]), ("Car", &[9]), ("Bus", &[6]), ("Bike", &[3])]);
    let many45: String = (1..=45).map(|i| format!("C{i}, {}\n", (i * 7) % 10)).collect();
    let combined = {
        // the test's own reading of the rule the too-many message gives
        let vals: Vec<u64> = (1..=45).map(|i| ((i * 7) % 10) as u64).collect();
        let mut by: Vec<usize> = (0..45).collect();
        by.sort_by(|&a, &b| vals[a].cmp(&vals[b]).then(b.cmp(&a)));
        let gone = &by[..16];
        let sum: u64 = gone.iter().map(|&i| vals[i]).sum();
        let mut keep: Vec<usize> = (0..45).filter(|i| !gone.contains(i)).collect();
        keep.sort();
        let mut out: Vec<(String, Vec<u64>)> = keep.into_iter().map(|i| (format!("C{}", i + 1), vec![vals[i]])).collect();
        out.push(("Other".into(), vec![sum]));
        out
    };
    let one = |s: &str| vec![("data", s.to_string())];
    let states: Vec<(Vec<(&str, String)>, End)> = vec![
        (one(""), End::Chart(rows(&[("Walk", &[12])]))),
        (one("Red, twelve\nBlue, 3"), End::Chart(rows(&[("Red", &[12]), ("Blue", &[3])]))),
        (one("Red, 12 people\nBlue, 3"), End::Chart(rows(&[("Red", &[12]), ("Blue", &[3])]))),
        (one("Red, 25%\nBlue, 75%"), End::Chart(rows(&[("Red", &[25]), ("Blue", &[75])]))),
        (one("Red, -3\nBlue, 3"), End::Chart(rows(&[("Red", &[3]), ("Blue", &[3])]))),
        (one("Red, 12.5\nBlue, 3"), End::Chart(rows(&[("Red", &[12]), ("Blue", &[3])]))),
        (one("Red, NaN\nBlue, inf"), End::Refused("is not a number")),
        (one("Red\nBlue, 3\nGreen, 2"), End::Chart(rows(&[("Blue", &[3]), ("Green", &[2])]))),
        (one(", 12\nBlue, 3"), End::Chart(rows(&[("Category 1", &[12]), ("Blue", &[3])]))),
        (one("Grade 9 12\nGrade 10 7"), End::Chart(rows(&[("Grade 9", &[12]), ("Grade 10", &[7])]))),
        // MAJOR: the >2-count refusal states the limit and drops nothing, so there is no fix to apply
        (one("Red, 1, 2, 3\nBlue, 3, 4"), End::Refused("at most two counts per category")),
        // MAJOR 2: a counts mismatch names the lines and offers the two shapes, never a rewrite
        (one("Red, 1, 2\nBlue, 3\nGreen, 4, 5"), End::Refused("holds 1 count (3), but line 1")),
        (one("Red, 1\nBlue, 3, 5"), End::Refused("holds 2 counts (3 and 5), but line 1")),
        (one("A, 5, 7\nB, 1,200"), End::Refused("holds 1 count (1200), but line 1")),
        (one("A, 1,200\nB, 5, 7"), End::Refused("holds 2 counts (5 and 7), but line 1")),
        (one(&format!("{long}, 3\nBlue, 4")), End::Chart(rows(&[(&shortened, &[3]), ("Blue", &[4])]))),
        (one(&many45), End::Chart(combined)),
        (one("Red, 0\nBlue, 0"), End::Chart(rows(&[("Red", &[1]), ("Blue", &[0])]))),
        (one("Red, 1, 0\nBlue, 2, 0"), End::Chart(rows(&[("Red", &[1, 1]), ("Blue", &[2, 0])]))),
        (one("Red, 4\nred, 2\nRED, 1\nBlue, 3"), End::Chart(rows(&[("Red", &[4]), ("Red 2", &[2]), ("Red 3", &[1]), ("Blue", &[3])]))),
        (vec![("data", "Red, 1\nBlue, 2\nGreen, 3\nGrey, 2".into()), ("measure", PERCENT.into())], End::Chart(rows(&[("Red", &[1]), ("Blue", &[2]), ("Green", &[3]), ("Grey", &[2])]))),
        (vec![("data", "A, 1\nB, 1\nC, 1".into()), ("measure", PERCENT.into())], End::Chart(rows(&[("A", &[1]), ("B", &[1]), ("C", &[1])]))),
        (vec![("scale_auto", "false".into()), ("scale_min", "5".into()), ("scale_max", "8".into())], End::Chart(defaults_pairs.clone())),
        (vec![("scale_auto", "false".into()), ("scale_min", "9".into()), ("scale_max", "3".into())], End::Chart(defaults_pairs.clone())),
        (vec![("scale_auto", "false".into()), ("scale_min", "40".into()), ("scale_max", "30".into()), ("measure", PERCENT.into())], End::Chart(defaults_pairs.clone())),
        (vec![("order", PARETO.into()), ("scale_auto", "false".into())], End::Chart(defaults_pairs.clone())),
        (vec![("order", PARETO.into()), ("orient", "Horizontal".into())], End::Chart(defaults_pairs.clone())),
        (vec![("order", PARETO.into()), ("data", "Dog, 11, 8\nCat, 7, 9".into())], End::Chart(rows(&[("Dog", &[11, 8]), ("Cat", &[7, 9])]))),
        (vec![("values", "false".into())], End::Chart(defaults_pairs.clone())),
        (vec![("values", "false".into()), ("blank", BLANKS[2].into())], End::Chart(defaults_pairs.clone())),
        (vec![("title", "T".repeat(170))], End::Chart(defaults_pairs.clone())),
        (vec![("cat_title", "C".repeat(90)), ("val_title", "V".repeat(85))], End::Chart(defaults_pairs.clone())),
        (vec![("data", "Dog, 11, 8\nCat, 7, 9".into()), ("name1", "N".repeat(45))], End::Chart(rows(&[("Dog", &[11, 8]), ("Cat", &[7, 9])]))),
        // J1 + MAJOR 1: thousands separators are one count, and the note says so; applying its
        // fix keeps the counts, and the other spelling is the two-count table
        (one("Walk, 1,200\nCar, 2,500\nBus, 3,000"), End::Chart(rows(&[("Walk", &[1200]), ("Car", &[2500]), ("Bus", &[3000])]))),
        (one("Walk, 1,200\nCar, 950"), End::Chart(rows(&[("Walk", &[1200]), ("Car", &[950])]))),
        (one("Walk 1,200\nCar 950"), End::Chart(rows(&[("Walk", &[1200]), ("Car", &[950])]))),
        (one("Walk\t1 200\nCar\t950"), End::Quiet(rows(&[("Walk", &[1200]), ("Car", &[950])]))),
        (one("A, 3,100\nB, 2,050"), End::Chart(rows(&[("A", &[3100]), ("B", &[2050])]))),
        (one("A 1 200\nB 950"), End::Chart(rows(&[("A", &[1200]), ("B", &[950])]))),
        (one("A, 150,000\nB, 250,000"), End::Chart(rows(&[("A", &[150000]), ("B", &[250000])]))),
        (one("Walk, 12,5\nCar, 9, 3"), End::Chart(rows(&[("Walk", &[12, 5]), ("Car", &[9, 3])]))),
        // J1: a decimal comma is refused, never read as two groups
        (one("Walk; 12,5"), End::Refused("has a comma inside a number")),
        // M2 (a): too large is refused for the whole table, not rescaled line by line
        (one("A, 1e15\nB, 3"), End::Refused("divide every line")),
        // M2 (b): counts written as words use the number-word list
        (one("Walk, twelve\nCar, nine"), End::Chart(rows(&[("Walk", &[12]), ("Car", &[9])]))),
        // M2 (d) + MED 1: a first line whose cells are names is a line of headings, never a
        // category and never a name turned into a count of 0
        (one("Fruit, Two-year-olds\nApple, 3"), End::Chart(rows(&[("Apple", &[3])]))),
        (one("Grade, Grade 9, Grade 10\nWalk, 12, 15"), End::Chart(rows(&[("Walk", &[12, 15])]))),
        (one("Group, None, One or more\nA, 1, 2"), End::Chart(rows(&[("A", &[1, 2])]))),
        (one("Grade, 9th grade, 10th grade\nWalk, 12, 15"), End::Chart(rows(&[("Walk", &[12, 15])]))),
        // MED 1: years as a first line are read as a category, and the note names the headings
        // route; the chart is unchanged because deleting a line is the teacher's choice
        (one("Year, 2019, 2020\nWalk, 12, 15\nCar, 9, 11"), End::Chart(rows(&[("Year", &[2019, 2020]), ("Walk", &[12, 15]), ("Car", &[9, 11])]))),
        // M1: these are data rows, not a headings line
        (one("Walk, 12 students\nCar, 9\nBus, 6"), End::Chart(rows(&[("Walk", &[12]), ("Car", &[9]), ("Bus", &[6])]))),
        (one("Walk, 1 200\nCar, 950"), End::Quiet(rows(&[("Walk", &[1200]), ("Car", &[950])]))),
        (one("Walk, l2\nCar, 3"), End::Refused("is not a number")),
        (one("Walk, 1O\nCar, 3"), End::Refused("is not a number")),
        (one("A, inf\nB, 3"), End::Refused("is not a number")),
        (one("X, N students\nY, M students"), End::Refused("is not a number")),
    ];
    let mut total_steps = 0;
    for (st, want) in states {
        let pairs: Vec<(&str, &str)> = st.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let mut p = with(&pairs);
        let before = warnings(&p);
        match &want {
            End::Quiet(_) => assert!(before.is_empty(), "{pairs:?} is read as typed, not noted: {before:?}"),
            _ => assert!(!before.is_empty(), "{pairs:?} raises a note"),
        }
        let (steps, stuck) = settle(&mut p);
        total_steps += steps;
        match &want {
            End::Chart(w) | End::Quiet(w) => {
                assert!(stuck.is_none(), "{pairs:?} stops at a refusal: {stuck:?}");
                let t = parse(p.s("data")).unwrap_or_else(|e| panic!("{pairs:?} -> {:?}: {e}", p.s("data")));
                assert_eq!(&state_pairs(&t), w, "{pairs:?} -> {:?}", p.s("data"));
                assert!(render(&p).contains("<rect x="), "{pairs:?} draws bars after the fixes");
            }
            End::Refused(sub) => {
                let msg = stuck.unwrap_or_else(|| panic!("{pairs:?} was changed instead of refused"));
                assert!(msg.contains(sub), "{pairs:?}: {msg}");
                assert!(!msg.contains("change line") && !msg.contains("Change line"), "{pairs:?}: a refusal must not change the data: {msg}");
                assert!(!msg.contains(", 0"), "{pairs:?}: a refusal must not invent a count: {msg}");
                assert!(parse(p.s("data")).is_err(), "{pairs:?} still refuses");
            }
        }
    }
    assert!(total_steps >= 38, "every state in this table needed a fix: {total_steps}");
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

#[test]
fn a_to_z_is_case_insensitive_and_natural() {
    // M8: case does not decide the order, and 2 comes before 10 (LOW 3)
    let t = parse("Zebra, 1\napple, 2\nEclair, 3\nItem 10, 4\nItem 2, 5").unwrap();
    let names: Vec<String> = order(&t, false, "A to Z").into_iter().map(|i| t.cats[i].name.clone()).collect();
    assert_eq!(names, ["apple", "Eclair", "Item 2", "Item 10", "Zebra"]);
    // a tie falls back to the typed order, whatever order the sort happens to see
    let t = parse("b, 1\nBeta, 2\nA, 3").unwrap();
    assert_eq!(order(&t, false, "A to Z"), [2, 0, 1]);
}

#[test]
fn two_groups_sort_by_the_sum_of_both_bars() {
    // M23: group 1 alone would put A first; the two bars together put B first
    let t = parse("A, 10, 0\nB, 4, 9\nC, 1, 1").unwrap();
    assert_eq!(order(&t, false, "Tallest first"), [1, 0, 2]);
    assert_eq!(order(&t, false, "Shortest first"), [2, 0, 1]);
    // relative frequency is each group's own percent, so the order follows those too
    assert_eq!(order(&t, true, "Tallest first"), [1, 0, 2], "B is 27% + 90%");
}

#[test]
fn equal_bars_keep_their_typed_order_in_both_measures() {
    // LOW 1: Alpha and Beta both sum to 15/18 of a percent-pair, but as f64 their sums are
    // 83.33333333333333 and 83.33333333333334, which a float key sorts the other way round. An
    // exact integer key (c0*T1 + c1*T0) ties them, so the typed order holds.
    let t = parse("Alpha, 5, 0\nBeta, 1, 2\nGamma, 0, 1").unwrap();
    assert_eq!(order(&t, true, "Tallest first"), [0, 1, 2]);
    assert_eq!(order(&t, true, "Shortest first"), [2, 0, 1]);
    assert_eq!(order(&t, false, "Tallest first"), [0, 1, 2], "5, 3, 1 by count");
    // one more in either group breaks the tie
    let t = parse("Alpha, 5, 0\nBeta, 2, 2\nGamma, 0, 1").unwrap();
    assert_eq!(order(&t, true, "Tallest first")[0], 1, "Beta is now above Alpha");
}

#[test]
fn thirty_one_categories_is_refused_with_the_rule_to_combine() {
    // M20: the cap is 30 categories, and 31 is refused rather than drawn
    let data: String = (1..=31).map(|i| format!("C{i}, {i}\n")).collect();
    assert_eq!(MAX_CATS, 30);
    let e = parse(&data).unwrap_err();
    assert!(e.contains("There are 31 categories") && e.contains("at most 30"), "{e}");
    let thirty: String = (1..=30).map(|i| format!("C{i}, {i}\n")).collect();
    assert_eq!(parse(&thirty).unwrap().cats.len(), 30);
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
                    // the exact gap fraction, not just "wide enough": a mutant that makes Wide
                    // 0.9 of the slot must fail (LOW). Only when the bars are not width-capped.
                    let want = match gap {
                        "Narrow" => 0.25,
                        "Wide" => 0.6,
                        _ => 0.4,
                    };
                    if t.groups == 1 && thick < 170.0 - 0.01 {
                        // the slot pitch: the closest start-to-start distance (a zero bar is not drawn,
                        // so some pairs skip a slot)
                        let mut pitches: Vec<f64> = spans.windows(2).map(|w| w[1].0 - w[0].0).collect();
                        pitches.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        let slot = pitches.first().copied().unwrap_or(0.0);
                        for w in spans.windows(2) {
                            if ((w[1].0 - w[0].0) - slot).abs() > 0.02 {
                                continue;
                            }
                            let frac = (w[1].0 - w[0].1) / slot;
                            assert!((frac - want).abs() < 0.01, "{name} {orient} {gap}: the gap is {frac} of a slot, not {want}");
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
fn pareto_right_axis_ticks_sit_exactly_on_their_percents() {
    // M10: the right axis is 0-100% over the same height as the bars, so every tick is exactly
    // pc/100 up from the bottom, and its number is on it
    let p = with(&[("data", "Forgot, 14\nNo time, 9\nToo hard, 5\nSick, 3\nOther, 2"), ("order", PARETO)]);
    let svg = render(&p);
    let es = elements(&svg);
    let axes: Vec<El> = es.iter().filter(|e| e.tag == "line" && e.n("x1") == e.n("x2") && e.n("stroke-width") == 2.0).cloned().collect();
    assert_eq!(axes.len(), 2, "the value axis and the cumulative axis");
    let right = axes.iter().max_by(|a, b| a.n("x1").partial_cmp(&b.n("x1")).unwrap()).unwrap();
    let (x, top, bottom) = (right.n("x1"), right.n("y1"), right.n("y2"));
    let mut ticks: Vec<f64> = es
        .iter()
        .filter(|e| e.tag == "line" && e.n("stroke-width") == 1.6 && (e.n("y1") - e.n("y2")).abs() < 1e-9 && (e.n("x1") - x).abs() < 0.01)
        .map(|e| e.n("y1"))
        .collect();
    ticks.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(ticks.len(), 6, "0, 20, 40, 60, 80 and 100%");
    let texts_here: Vec<El> = texts(&svg);
    for (i, pc) in [0.0, 20.0, 40.0, 60.0, 80.0, 100.0].iter().enumerate() {
        let want = bottom - pc / 100.0 * (bottom - top);
        let got = ticks[5 - i];
        assert!((got - want).abs() < 0.011, "{pc}% tick at {got}, expected {want}");
        let label = texts_here.iter().find(|e| e.text == format!("{pc:.0}%")).unwrap_or_else(|| panic!("a {pc:.0}% number"));
        assert!((label.n("y") - (want + 14.0 * 0.35)).abs() < 0.02, "the {pc:.0}% number sits on its tick");
        assert!((label.n("x") - (x + 10.0)).abs() < 0.01);
    }
    assert!(texts_here.iter().any(|e| e.text == "Cumulative percent" && e.raw.contains("rotate(-90)")));
    // LOW 5: the student copy has no cumulative axis, because no question asks about the line
    let mut student = p.clone();
    student.set_b(SHOW_ANSWERS, false);
    let stu = render(&student);
    assert_eq!(size(&stu), size(&svg), "the same size, so the two copies line up");
    assert!(!stu.contains("Cumulative percent") && !texts(&stu).iter().any(|e| e.text.ends_with('%') && e.text.len() <= 5 && e.n("x") > x - 1.0));
    // LOW: with Relative frequency the left axis is already 0-100%, so there is one axis, not two
    let pct = with(&[("data", "Forgot, 14\nNo time, 9\nToo hard, 5\nSick, 3"), ("order", PARETO), ("measure", PERCENT)]);
    let svg = render(&pct);
    let axes = elements(&svg).into_iter().filter(|e| e.tag == "line" && e.n("x1") == e.n("x2") && e.n("stroke-width") == 2.0).count();
    assert_eq!(axes, 1, "one vertical axis: the percent axis already reads 0-100%");
    assert!(!svg.contains("Cumulative percent") && svg.contains("<polyline"), "the line is still drawn");
    assert!(texts(&svg).iter().any(|e| e.text == "100%"), "the left axis ends at 100%");
    // LOW: the student copy places labels as on any bar chart: no chip, no lift from the hidden line
    let mut student = with(&[("data", "Forgot, 14\nNo time, 9\nToo hard, 5\nSick, 3\nOther, 2"), ("order", PARETO), ("blank", BLANKS[2])]);
    student.set_b(SHOW_ANSWERS, false);
    let stu = render(&student);
    assert!(!stu.contains("stroke=\"none\""), "no white chip in the student copy");
    let b = bars(&stu, 1.0);
    let labels: Vec<(El, Bx)> = ink(&stu).into_iter().filter(|(e, _)| e.a("font-size").as_deref() == Some("12.60")).collect();
    assert_eq!(labels.len(), b.len(), "a label per bar");
    for (e, bx) in &labels {
        let cx = (bx.0 + bx.2) / 2.0;
        let bar = b.iter().find(|r| (r.0 + r.2 / 2.0 - cx).abs() < 1.5).unwrap_or_else(|| panic!("a bar under {:?}", e.text));
        let gap = bar.1 - bx.3;
        assert!((gap - (5.0 + 0.3 * 12.6)).abs() < 0.6, "{:?} is {gap} px above its bar, not lifted by the hidden line", e.text);
    }
}

#[test]
fn question_answers_are_recomputed_independently() {
    // M19/M24: every answer the key prints is checked against this test's own arithmetic, at
    // each decimals setting, including ties, totals with thousands commas and zero bars
    let cases = ["Walk, 12\nCar, 9\nBus, 6\nNone, 0\nOther, 9", "Dog, 11, 8\nCat, 7, 9\nFish, 3, 2", "A, 5\nB, 5\nC, 5", "Big, 1234567\nSmall, 3"];
    let commas = |v: u64| {
        let s = v.to_string();
        let mut out = String::new();
        for (i, ch) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(ch);
        }
        out
    };
    let pct = |c: u64, total: u64, d: u32| {
        let t = total.max(1) as u128;
        let s = (2 * c as u128 * 100 * 10u128.pow(d) + t) / (2 * t);
        let p = 10u128.pow(d);
        if d == 0 { format!("{s}%") } else { format!("{}.{:0w$}%", s / p, s % p, w = d as usize) }
    };
    let and = |v: &[String]| match v.len() {
        0 => String::new(),
        1 => v[0].clone(),
        n => format!("{} and {}", v[..n - 1].join(", "), v[n - 1]),
    };
    for data in cases {
        let t = parse(data).unwrap();
        for d in 0..=2i64 {
            let mut p = with(&[("data", data), ("questions", "true")]);
            p.set_i("decimals", d);
            let svg = render(&p);
            let ans: Vec<String> = texts(&svg).into_iter().filter(|e| e.n("x") == 36.0).map(|e| e.text).collect();
            assert_eq!(ans.len(), 3, "{data}: three answers");
            for a in &ans {
                assert!(a.starts_with("Answer: "), "{a}");
            }
            let totals: Vec<u64> = (0..t.groups).map(|g| t.cats.iter().map(|c| c.counts[g]).sum()).collect();
            let most: Vec<String> = (0..t.groups)
                .map(|g| {
                    let m = t.cats.iter().map(|c| c.counts[g]).max().unwrap();
                    let names: Vec<String> = t.cats.iter().filter(|c| c.counts[g] == m).map(|c| c.name.clone()).collect();
                    if names.len() > 1 { format!("{} (tied)", and(&names)) } else { names[0].clone() }
                })
                .collect();
            let per = |v: Vec<String>| if t.groups == 1 { v[0].clone() } else { format!("Group 1: {}; Group 2: {}", v[0], v[1]) };
            assert_eq!(ans[0], format!("Answer: {}", per(most)), "{data} decimals {d}");
            assert_eq!(ans[1], format!("Answer: {}", per(totals.iter().map(|v| commas(*v)).collect())), "{data} decimals {d}");
            let first = &t.cats[0];
            let q3: Vec<String> = (0..t.groups).map(|g| pct(first.counts[g], totals[g], d as u32)).collect();
            assert_eq!(ans[2], format!("Answer: {}", per(q3)), "{data} decimals {d}");
        }
    }
    // a three-way tie names all three, and the question asks about the first drawn category
    let p = with(&[("data", "A, 5\nB, 5\nC, 5"), ("questions", "true")]);
    let ans: Vec<String> = texts(&render(&p)).into_iter().filter(|e| e.n("x") == 36.0).map(|e| e.text).collect();
    assert_eq!(ans[0], "Answer: A, B and C (tied)");
    assert_eq!(ans[1], "Answer: 15");
    assert_eq!(ans[2], "Answer: 33%");
    // Q3 follows Decimals in percents, and percents are of each group's own total
    let mut p = with(&[("data", "Dog, 11, 8\nCat, 7, 9\nFish, 3, 2"), ("questions", "true")]);
    p.set_i("decimals", 2);
    let ans: Vec<String> = texts(&render(&p)).into_iter().filter(|e| e.n("x") == 36.0).map(|e| e.text).collect();
    assert_eq!(ans[2], "Answer: Group 1: 52.38%; Group 2: 42.11%", "11 of 21 and 8 of 19");
}

#[test]
fn the_rounding_note_settles_on_both_groups_at_once() {
    // MED 2/3: the note searches every Decimals setting for EVERY group; when none leaves every
    // column adding to 100% it says so and offers the counts, and applying that clears it
    let pet = "Pet, Class A, Class B\nDog, 8, 1\nCat, 9, 2\nFish, 2, 3\nNone, 9, 2";
    let all = warnings(&with(&[("data", pet), ("measure", PERCENT), ("decimals", "1")]));
    let d1: Vec<String> = all.iter().filter(|n| n.contains("printed percents")).cloned().collect();
    assert_eq!(d1.len(), 1, "{all:?}");
    assert!(d1[0].contains("add to 99.9% for \u{201c}Class A\u{201d} and 100.0% for \u{201c}Class B\u{201d}, not 100% in every group"), "{}", d1[0]);
    assert!(d1[0].contains("No Decimals setting makes every group add to 100%. Choose \u{201c}Frequency (count)\u{201d} for \u{201c}Bar height\u{201d}"), "{}", d1[0]);
    let mut p = with(&[("data", pet), ("measure", PERCENT), ("decimals", "1")]);
    let (steps, stuck) = settle(&mut p);
    assert_eq!((steps, stuck.is_none()), (1, true), "{steps} {stuck:?}");
    assert!(warnings(&p).iter().all(|n| info_only(n)), "{:?}", warnings(&p));
    assert_eq!(p.s("measure"), COUNT, "the offered fix is the counts");
    // the same at Decimals 0: Class B is the group that is out, and the note still offers the counts
    let all = warnings(&with(&[("data", pet), ("measure", PERCENT), ("decimals", "0")]));
    let d0: Vec<String> = all.iter().filter(|n| n.contains("printed percents")).cloned().collect();
    assert_eq!(d0.len(), 1, "{all:?}");
    assert!(d0[0].contains("add to 100% for \u{201c}Class A\u{201d} and 101% for \u{201c}Class B\u{201d}"), "{}", d0[0]);
    assert!(d0[0].contains("No Decimals setting makes every group add to 100%."), "{}", d0[0]);
    // one group: the setting that works is offered (8, 9, 2, 9 of 28 at one decimal -> 0)
    let p = with(&[("data", "A, 8\nB, 9\nC, 2\nD, 9"), ("measure", PERCENT), ("decimals", "1")]);
    assert_eq!(warnings(&p), ["The printed percents add to 99.9%, not 100%, because each is rounded. Set \u{201c}Decimals in percents\u{201d} to 0."]);
    assert!(warnings(&with(&[("data", "A, 8\nB, 9\nC, 2\nD, 9"), ("measure", PERCENT), ("decimals", "0")])).is_empty());
    // thirds: no setting works, so the note offers the counts and names the setting it searched
    let thirds = warnings(&with(&[("data", "A, 1\nB, 1\nC, 1"), ("measure", PERCENT)]));
    assert_eq!(thirds, ["The printed percents add to 99%, not 100%, because each is rounded. No Decimals setting makes them add to 100%. Choose \u{201c}Frequency (count)\u{201d} for \u{201c}Bar height\u{201d} if they must add up exactly."]);
    // eighths: 1 decimal adds exactly, so that is the offer
    let eighths = warnings(&with(&[("data", "A, 1\nB, 2\nC, 3\nD, 2"), ("measure", PERCENT)]));
    assert_eq!(eighths, ["The printed percents add to 101%, not 100%, because each is rounded. Set \u{201c}Decimals in percents\u{201d} to 1."]);
    assert!(warnings(&with(&[("data", "A, 1\nB, 2\nC, 3\nD, 2"), ("measure", PERCENT), ("decimals", "1")])).is_empty());
    // every advice clears in one step and never comes back
    for (data, d) in [("A, 1\nB, 1\nC, 1", "0"), ("A, 1\nB, 2\nC, 3\nD, 2", "0"), ("A, 8\nB, 9\nC, 2\nD, 9", "1"), (pet, "1"), (pet, "0")] {
        let mut p = with(&[("data", data), ("measure", PERCENT), ("decimals", d)]);
        let (steps, stuck) = settle(&mut p);
        assert!(steps >= 1 && stuck.is_none(), "{data} {d}: {steps} {stuck:?}");
        assert!(warnings(&p).iter().all(|n| info_only(n)), "{data} {d}: {:?}", warnings(&p));
    }
    // the table-only copy gets the same note: its Total row shows the rounded sum it is about
    let table_only = warnings(&with(&[("data", "A, 1\nB, 1\nC, 1"), ("measure", PERCENT), ("values", "false"), ("blank", BLANKS[1])]));
    assert_eq!(table_only.len(), 1, "{table_only:?}");
}
#[test]
fn truncated_axis_and_cut_off_bars_are_flagged() {
    let p = with(&[("data", "Brand A, 46\nBrand B, 44\nBrand C, 42"), ("scale_auto", "false"), ("scale_min", "40"), ("scale_max", "45")]);
    let n = warnings(&p);
    assert!(n.iter().any(|n| n.contains("starts at 40, not 0") && n.contains("twice")), "{n:?}");
    assert!(n.iter().any(|n| n.contains("Brand A") && n.contains("cut off") && n.contains("at least 46")), "{n:?}");
    let ok = with(&[("scale_auto", "false"), ("scale_min", "0"), ("scale_max", "20")]);
    assert!(warnings(&ok).is_empty(), "a manual scale from 0 that holds every bar is fine");
    // M14: any start above 0 cuts the bars short, including one below 1
    let small = with(&[("data", "Brand A, 46\nBrand B, 44\nBrand C, 42"), ("scale_auto", "false"), ("scale_min", "0.5"), ("scale_max", "50")]);
    let n = warnings(&small);
    assert!(n.iter().any(|n| n.contains("starts at 0.5, not 0")), "a start of 0.5 is still a truncated axis: {n:?}");
    let tiny = with(&[("data", "Brand A, 46\nBrand B, 44"), ("scale_auto", "false"), ("scale_min", "0.25"), ("scale_max", "20")]);
    assert!(warnings(&tiny).iter().any(|n| n.contains("starts at 0.25, not 0")), "{:?}", warnings(&tiny));
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
fn a_value_label_clears_its_bar_by_five_pixels() {
    // M15: the label's box sits 5 px above its bar (and 6 px past its end when the bars are
    // horizontal), not 1 px and not on it
    let data = "Walk, 12\nCar, 9\nBus, 6\nBike, 3";
    let svg = render(&with(&[("data", data)]));
    let b = bars(&svg, 1.0);
    let labels: Vec<(El, Bx)> = ink(&svg).into_iter().filter(|(e, _)| e.a("font-size").as_deref() == Some("12.60")).collect();
    assert_eq!(labels.len(), 4);
    for (e, bx) in &labels {
        let cx = (bx.0 + bx.2) / 2.0;
        let bar = b.iter().find(|r| (r.0 + r.2 / 2.0 - cx).abs() < 1.5).unwrap_or_else(|| panic!("a bar under {:?}", e.text));
        // digits have no descender: their ink bottom is the text's baseline, which is DES below
        // the box bottom, so the ink clears the bar by 5 + DES * 12.6 px
        let gap = bar.1 - bx.3;
        assert!((gap - (5.0 + 0.3 * 12.6)).abs() < 0.6, "{:?} is {gap} px above its bar", e.text);
    }
    let svg = render(&with(&[("data", data), ("orient", "Horizontal")]));
    let b = bars(&svg, 1.0);
    let labels: Vec<(El, Bx)> = ink(&svg).into_iter().filter(|(e, _)| e.a("font-size").as_deref() == Some("12.60")).collect();
    for (e, bx) in &labels {
        let cy = (bx.1 + bx.3) / 2.0;
        let bar = b.iter().find(|r| cy > r.1 && cy < r.1 + r.3).unwrap_or_else(|| panic!("a bar beside {:?}", e.text));
        let gap = bx.0 - (bar.0 + bar.2);
        assert!((5.0..=7.5).contains(&gap), "{:?} starts {gap} px past its bar", e.text);
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
                                let my_tenths = |c: u64, n: u64| -> u128 { (2 * c as u128 * 1000 + n as u128) / (2 * n as u128) };
                                let my_pct = |c: u64, n: u64| {
                                    let s = my_tenths(c, n);
                                    format!("{}.{}%", s / 10, s % 10)
                                };
                                let vlab: Vec<String> = t.cats.iter().flat_map(|c| (0..t.groups).map(|g| if percent { my_pct(c.counts[g], tot[g]) } else { fmt_count(c.counts[g]) }).collect::<Vec<_>>()).collect();
                                let ktexts = texts(&key);
                                let stexts = texts(&stu);
                                let mut answers: Vec<String> = Vec::new();
                                let bi = BLANKS.iter().position(|b| *b == blank).unwrap();
                                let idx_order = super::order(&t, percent, order);
                                if values == "true" && bi != 2 {
                                    answers.extend(vlab.clone());
                                }
                                if bi == 1 {
                                    answers.extend(tot.iter().map(|v| fmt_count(*v)));
                                    if percent {
                                        // MED 4: the table cells are the same numbers as the labels,
                                        // and the Total cell is what they add to
                                        answers.extend(vlab.clone());
                                        for g in 0..t.groups {
                                            let tenths: u128 = idx_order.iter().map(|&i| my_tenths(t.cats[i].counts[g], tot[g])).sum();
                                            answers.push(format!("{}.{}%", tenths / 10, tenths % 10));
                                        }
                                    }
                                }
                                if bi == 2 {
                                    answers.extend(ktexts.iter().filter(|e| tick_label(e, &key, orient == "Horizontal")).map(|e| e.text.clone()));
                                }
                                let qa: Vec<String> = ktexts.iter().filter(|e| e.n("x") == 36.0).map(|e| e.text.clone()).collect();
                                assert!(qa[0].starts_with("Answer: "), "{tag}");
                                answers.extend(qa.iter().cloned());
                                // LOW 5: a Pareto key's right axis belongs to the cumulative line, so
                                // the student copy has neither the numbers nor the axis title
                                if order == PARETO && t.groups == 1 && orient == "Vertical" && !percent {
                                    answers.extend(
                                        ktexts
                                            .iter()
                                            .filter(|e| e.a("text-anchor").as_deref() == Some("start") && (0..=100).step_by(20).any(|pc| e.text == format!("{pc}%")))
                                            .map(|e| e.text.clone()),
                                    );
                                    answers.push("Cumulative percent".into());
                                }
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
                                let blanks: Vec<f64> = elements(&stu).iter().filter(|e| e.tag == "line" && e.n("stroke-width") == 1.0 && e.n("y1") == e.n("y2") && e.n("x1") > 40.0 && e.a("stroke").as_deref() != Some(GRID) && e.a("stroke").as_deref() != Some("#cccccc") && e.a("stroke").as_deref() != Some(PRINT_GRID)).map(|e| e.n("x2") - e.n("x1")).collect();
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

/// A properly downsampled copy: render at 1x, then resize by `pct` per cent with a Lanczos filter,
/// as a photocopier's reduction does (MED 3: sampling one pixel of an enlarged render hid the
/// loss). Returns the image and the exact scale used.
fn copy_at(svg: &str, pct: u32) -> (image::GrayImage, f64) {
    let png = crate::figures::to_png(svg, 1.0).unwrap();
    let img = image::load_from_memory(&png).unwrap().to_luma8();
    let f = pct as f64 / 100.0;
    let (w, h) = ((img.width() as f64 * f).round() as u32, (img.height() as f64 * f).round() as u32);
    (image::imageops::resize(&img, w, h, image::imageops::FilterType::Lanczos3), f)
}

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
        if b.4 == PRINT_BAR { g0.push(frac(b)) } else { g1.push(frac(b)) }
        // the outline stays: the middle of the top edge is black
        assert!(black(b.0 + b.2 / 2.0, b.1), "{b:?} outline");
    }
    // LOW: the solid first series and the grid both survive the copy (they used to be too light)
    assert!(g0.iter().all(|f| *f > 0.9), "the solid series survives a copy: {g0:?}");
    assert!(g1.iter().all(|f| *f > 0.12 && *f < 0.9), "the hatched series survives and stays apart: {g1:?}");
    // MED 2: every gridline, on narrow and wide charts, at 66% and 58%, 1x render then resize and
    // thresholded at 168: the min is taken per line (a global min hid one dead line of six).
    // Measured with 3 px #8c8c8c: 127-142 at 66% and 129-152 at 58% over 4, 10 and 30 categories.
    let mut worst_all = 255u8;
    let mut seen: Vec<u8> = Vec::new();
    for (label, data_line) in [("4 cats", "Walk, 12\nCar, 9\nBus, 6\nBike, 3".to_string()), ("10", many(10)), ("30", many(30))] {
        let mut bare = with(&[("data", &data_line), ("style", "Print"), ("blank", BLANKS[1])]);
        bare.set_b(SHOW_ANSWERS, false);
        let bsvg = render(&bare);
        let axis_y = elements(&bsvg).into_iter().find(|e| e.tag == "line" && e.n("y1") == e.n("y2") && e.n("stroke-width") == 2.0).map(|e| e.n("y1")).unwrap();
        let grid: Vec<El> = elements(&bsvg).into_iter().filter(|e| e.tag == "line" && e.a("stroke").as_deref() == Some(PRINT_GRID) && e.n("y1") == e.n("y2") && (e.n("y1") - axis_y).abs() > 3.0).collect();
        assert!(grid.len() >= 3, "{label}: the grid is drawn: {}", grid.len());
        for pct in [66u32, 58] {
            let (img, f) = copy_at(&bsvg, pct);
            for g in &grid {
                let (x0, x1, y) = (g.n("x1"), g.n("x2"), g.n("y1"));
                let mut worst = 255u8;
                let mut x = x0 + 3.0;
                while x < x1 - 3.0 {
                    for dy in [-1.0f64, 0.0, 1.0] {
                        let px = ((x * f) as u32).min(img.width() - 1);
                        let py = (((y * f) + dy).round().max(0.0) as u32).min(img.height() - 1);
                        worst = worst.min(img.get_pixel(px, py)[0]);
                    }
                    x += 1.0;
                }
                worst_all = worst_all.min(worst);
                assert!(worst <= 160, "{label} at {pct}%: the gridline at y={y} has min {worst} against the 168 threshold (measured 127-152)");
                seen.push(worst);
            }
        }
    }
    assert!(seen.len() >= 20, "every line of every chart is measured: {}", seen.len());
    assert!(worst_all >= 120 && worst_all <= 160, "the measured darkest gridline pixel is {worst_all}");
    // one series: its fill is the solid grey, so its bars keep their tone in the copy
    let one = with(&[("data", "Walk, 12\nCar, 9\nBus, 6\nBike, 3"), ("style", "Print")]);
    let svg1 = render(&one);
    let png1 = crate::figures::to_png(&svg1, sc as f32).unwrap();
    let img1 = image::load_from_memory(&png1).unwrap().to_luma8();
    let solid: Vec<_> = elements(&svg1).into_iter().filter(|e| e.tag == "rect" && e.a("width").as_deref() != Some("100%") && e.a("stroke").as_deref() != Some("none")).map(|e| (e.n("x"), e.n("y"), e.n("width"), e.n("height"), e.a("fill").unwrap())).filter(|b| b.4 == PRINT_BAR).collect();
    assert!(!solid.is_empty());
    for b in &solid {
        let (x0, y0, x1, y1) = ((b.0 + 3.0) * sc, (b.1 + 3.0) * sc, (b.0 + b.2 - 3.0) * sc, (b.1 + b.3 - 3.0) * sc);
        let (mut on, mut all) = (0, 0);
        for x in x0 as u32..x1 as u32 {
            for y in y0 as u32..y1 as u32 {
                all += 1;
                on += (img1.get_pixel(x, y)[0] < 168) as u32;
            }
        }
        assert!(on as f64 / all.max(1) as f64 > 0.9, "a single-series Print bar keeps its tone: {b:?}");
    }
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

