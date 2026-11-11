//! Batch 8 tests for the solids: measures typed as fractions or with square roots (read, refused, written exactly on the labels,
//! worked-out measures right by hand), the oblique cylinder's and the prism's height lines switched off, the panel, the drags
//! through the workspace's hit test, and long typed labels at Large kept off lines, off each other and inside the page.
use super::solids_b8::{pretty, read, warnings};
use super::solids_b8_frozen::{def, with, IDS};
use crate::figures::interact;
use crate::figures::{param_relevant, Params, Value};

fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn svg(id: &str, list: &[(&'static str, Value)]) -> String { def(id).svg(&with(id, list)) }
fn comp(id: &str, p: &Params, key: &str) -> String {
    crate::figures::math::computed(id, p).into_iter().find(|x| x.0 == key).map(|x| x.1).unwrap_or_default()
}
fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }

/// The drawn texts: (x, y baseline, size, text).
fn texts(svg: &str) -> Vec<(f64, f64, f64, String)> {
    let mut out = Vec::new();
    for part in svg.split("<text").skip(1) {
        let attr = |n: &str| part.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse::<f64>().ok());
        let Some(body) = part.split('>').nth(1) else { continue };
        let t = body.split('<').next().unwrap_or("").replace("&amp;", "&");
        if let (Some(x), Some(y), Some(sz)) = (attr("x"), attr("y"), attr("font-size")) { out.push((x, y, sz, t)); }
    }
    out
}
fn dashed_lines(svg: &str) -> usize { svg.split("<line").skip(1).filter(|l| l.split('>').next().unwrap_or("").contains("stroke-dasharray=\"6 5\"")).count() }
/// Every text's ink box as drawn (resvg with the app's fonts), in document order: (text, left, top, right, bottom).
fn ink(svg: &str) -> Vec<(String, (f64, f64, f64, f64))> {
    fn walk(g: &resvg::usvg::Group, out: &mut Vec<(String, (f64, f64, f64, f64))>) {
        for n in g.children() {
            match n {
                resvg::usvg::Node::Group(g) => walk(g, out),
                resvg::usvg::Node::Text(t) => {
                    let r = t.flattened().bounding_box().transform(t.abs_transform()).unwrap_or(t.flattened().abs_bounding_box());
                    out.push((t.chunks().iter().map(|c| c.text()).collect(), (r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64)));
                }
                _ => {}
            }
        }
    }
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(&crate::figures::serif_stack(svg), &opt).expect("the SVG parses");
    let mut v = Vec::new();
    walk(tree.root(), &mut v);
    v
}
/// Every inked stroke as segments: lines, and the edges of polylines and polygons drawn with a stroke of 1 px or more (the
/// faces' own thin tint outlines and unstroked fills are not lines).
/// Strokes painted over by a later filled shape (the hidden back of a base under a flat solid's top face) are not seen.
fn strokes(svg: &str) -> Vec<((f64, f64), (f64, f64))> {
    let fills: Vec<(usize, Vec<(f64, f64)>)> = svg.match_indices("<polygon").filter_map(|(i, _)| {
        let head = svg[i..].split('>').next().unwrap_or("");
        let at = |n: &str| head.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()).unwrap_or("");
        (at("fill") != "none").then(|| (i, at("points").split_whitespace().filter_map(|q| q.split_once(',')).filter_map(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))).collect()))
    }).collect();
    let inside = |q: (f64, f64), poly: &[(f64, f64)]| {
        let mut c = false;
        for k in 0..poly.len() {
            let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
            if (a.1 > q.1) != (b.1 > q.1) && q.0 < a.0 + (q.1 - a.1) * (b.0 - a.0) / (b.1 - a.1) { c = !c; }
        }
        c
    };
    let covered = |at: usize, a: (f64, f64), b: (f64, f64)| fills.iter().any(|(i, poly)| *i > at && inside(a, poly) && inside(b, poly));
    let mut out: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for (i, _) in svg.match_indices("<line") {
        let head = svg[i..].split('>').next().unwrap_or("");
        let a = |n: &str| head.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse::<f64>().ok()).unwrap_or(f64::NAN);
        let (p, q) = ((a("x1"), a("y1")), (a("x2"), a("y2")));
        if !covered(i, p, q) { out.push((p, q)); }
    }
    for tag in ["<polyline", "<polygon"] {
        for (i, _) in svg.match_indices(tag) {
            let head = svg[i + tag.len()..].split('>').next().unwrap_or("");
            let at = |n: &str| head.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()).unwrap_or("");
            if at("stroke") == "none" || at("stroke-width").parse::<f64>().unwrap_or(1.0) < 1.0 { continue; }
            let pts: Vec<(f64, f64)> = at("points").split_whitespace().filter_map(|q| q.split_once(',')).filter_map(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))).collect();
            for w in pts.windows(2) { if !covered(i, w[0], w[1]) { out.push((w[0], w[1])); } }
            if tag == "<polygon" && pts.len() > 2 { out.push((pts[pts.len() - 1], pts[0])); }
        }
    }
    out
}
/// Whether segment `s` belongs to an outline: a solid (undashed) stroke 2 px or more wide.
fn outline(svg: &str, s: ((f64, f64), (f64, f64))) -> bool {
    let key = |q: (f64, f64)| format!("{:.2},{:.2}", q.0, q.1);
    let line = format!("x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\"", s.0.0, s.0.1, s.1.0, s.1.1);
    svg.split('<').any(|e| {
        let head = e.split('>').next().unwrap_or("");
        let dashed = head.split(" stroke-dasharray=\"").nth(1).is_some_and(|r| !r.starts_with("none"));
        let w = head.split(" stroke-width=\"").nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        !dashed && w >= 2.0 && (head.contains(&line) || ((e.starts_with("polyline") || e.starts_with("polygon")) && head.contains(&format!("{} {}", key(s.0), key(s.1)))))
    })
}
fn seg_hits_box(a: (f64, f64), c: (f64, f64), bx: (f64, f64, f64, f64)) -> bool {
    (0..=200).any(|i| {
        let t = i as f64 / 200.0;
        let (x, y) = (a.0 + (c.0 - a.0) * t, a.1 + (c.1 - a.1) * t);
        x > bx.0 + 1.0 && x < bx.2 - 1.0 && y > bx.1 + 1.0 && y < bx.3 - 1.0
    })
}

// ───────────────────────── reading and writing ─────────────────────────

#[test]
fn reads_numbers_fractions_and_roots() {
    let r = |t: &str| read(t).unwrap().unwrap();
    assert!(close(r("3/2"), 1.5));
    assert!(close(r("17/3"), 17.0 / 3.0));
    for t in ["2sqrt3", "2\u{221a}3", "2*sqrt(3)", "2 sqrt 3", "2\u{d7}\u{221a}3", "sqrt(12)"] { assert!(close(r(t), 2.0 * 3f64.sqrt()), "{t}"); }
    assert!(close(r("5\u{221a}13"), 5.0 * 13f64.sqrt()));
    assert!(close(r("sqrt(13)/2"), 13f64.sqrt() / 2.0));
    assert!(close(r("\u{221a}13/2"), 13f64.sqrt() / 2.0), "a root binds tighter than the division");
    assert!(close(r("3/sqrt2"), 3.0 / 2f64.sqrt()));
    assert!(close(r("(1+\u{221a}5)/2"), (1.0 + 5f64.sqrt()) / 2.0));
    assert!(close(r("2pi"), 2.0 * std::f64::consts::PI) && close(r("\u{3c0}"), std::f64::consts::PI));
    assert!(close(r(" 7 "), 7.0) && close(r("2.25"), 2.25) && close(r("-3/2"), -1.5) && close(r("\u{2212}3/2"), -1.5));
    assert_eq!(read(""), Ok(None));
    assert_eq!(read("   "), Ok(None));
    for bad in ["abc", "x", "1e3", "3 1/2", "1.2.3", "2/", "/2", "sqrt", "sqrt(-4)", "1/0", "0/0", "(2", "2)", "3cm", "\u{221a}", "5%", "2^3"] {
        assert!(read(bad).is_err(), "{bad} should not read");
    }
}

#[test]
fn writes_the_exact_form() {
    for (t, want) in [
        ("3/2", "3/2"), ("17/3", "17/3"), ("2sqrt3", "2\u{221a}3"), ("2*sqrt(3)", "2\u{221a}3"), ("2 sqrt 3", "2\u{221a}3"), ("sqrt(13)/2", "\u{221a}13/2"),
        ("5\u{221a}13", "5\u{221a}13"), ("2pi", "2\u{3c0}"), ("sqrt(2+3)", "\u{221a}(2 + 3)"), (" 2.50 ", "2.50"), ("-3/2", "\u{2212}3/2"), ("3*2", "3\u{d7}2"),
        ("(1+sqrt5)/2", "(1 + \u{221a}5)/2"), ("4 - sqrt2", "4 \u{2212} \u{221a}2"),
    ] {
        assert_eq!(pretty(t), want, "{t}");
    }
}

// ───────────────────────── on the figure ─────────────────────────

/// Every typed box of every figure: the label shows the typed form with its unit, the drawing is the slider's drawing at the
/// same value (only the label text differs), and the slider is hidden while the box reads.
#[test]
fn typed_measures_draw_and_label_exactly() {
    let cases: [(&str, &str, &str, &[(&'static str, Value)]); 18] = [
        ("cylinder", "radius", "3/2", &[]), ("cylinder", "height", "2\u{221a}3", &[]), ("cylinder", "lean", "3/2", &[("stands", Value::S("Oblique".into()))]),
        ("cylinder", "diam", "7/2", &[("give_d", Value::B(true))]),
        ("cone", "radius", "sqrt(5)", &[]), ("cone", "height", "17/3", &[]), ("cone", "lean", "5/2", &[("stands", Value::S("Oblique".into()))]),
        ("cone", "diam", "4\u{221a}2", &[("give_d", Value::B(true))]),
        ("prism", "length", "15/2", &[]), ("prism", "width", "\u{221a}5", &[]), ("prism", "height", "5\u{221a}2", &[]), ("prism", "lean", "3/2", &[("stands", Value::S("Oblique".into()))]),
        ("prism", "side", "5/2", &[("base", Value::S("Regular polygon".into()))]),
        ("pyramid", "length", "13/2", &[]), ("pyramid", "width", "2\u{221a}3", &[]), ("pyramid", "height", "sqrt(13)", &[]), ("pyramid", "lean", "3/2", &[("stands", Value::S("Oblique".into()))]),
        ("pyramid", "side", "7/3", &[("base", Value::S("Regular polygon".into()))]),
    ];
    for (id, key, typed, extra) in cases {
        let tk: &'static str = Box::leak(format!("{key}_t").into_boxed_str());
        let v = read(typed).unwrap().unwrap();
        let lab = match (id, key) {
            (_, "lean") => "lean_lab", ("cylinder" | "cone", "height") => "h_lab", ("cylinder" | "cone", "radius") => "r_lab",
            ("cylinder" | "cone", "diam") => "r_lab", (_, k) => Box::leak(format!("{k}_lab").into_boxed_str()),
        };
        let mut l = extra.to_vec();
        l.push(s(lab, "Measure"));
        let mut lt = l.clone();
        lt.push(s(tk, typed));
        let st = svg(id, &lt);
        let want = format!("{} cm", pretty(typed));
        assert!(texts(&st).iter().any(|t| t.3 == want), "{id} {key}: no label {want} in {:?}", texts(&st).iter().map(|t| &t.3).collect::<Vec<_>>());
        // the same drawing as the slider at that value, but for the label's text
        let mut ls = l.clone();
        ls.push(f(key, v));
        let ss = svg(id, &ls);
        let strip = |x: &str| texts(x).iter().map(|t| format!("{:.1},{:.1}", t.0, t.1)).collect::<Vec<_>>();
        let geo = |x: &str| x.split('<').filter(|e| ["polygon", "polyline", "line ", "path", "circle"].iter().any(|t| e.starts_with(t))).map(|e| e.to_string()).collect::<Vec<_>>();
        // the same solid as the slider at that value: the very same drawing, or (when a long typed label needs room beside the
        // solid, `solids_b8::fit`) the same elements moved in or drawn a little smaller
        assert!(close(super::sv(&with(id, &lt), key), v));
        let (g1, g2) = (geo(&st), geo(&ss));
        assert!(g1 == g2 || (g1.len() == g2.len() && super::probe_map(super::render_for(id).unwrap(), &with(id, &lt)).unwrap().0 <= super::probe_map(super::render_for(id).unwrap(), &with(id, &ls)).unwrap().0), "{id} {key}={typed} draws a different shape from the slider at {v}");
        assert_eq!(strip(&st).len(), strip(&ss).len(), "{id} {key}");
        // the slider is hidden while the box reads, shown otherwise
        assert!(!param_relevant(id, key, &with(id, &lt)), "{id}: {key} slider still shown");
        assert!(param_relevant(id, key, &with(id, &l)), "{id}: {key} slider hidden with an empty box");
        assert!(warnings(id, &with(id, &lt)).is_empty(), "{id} {key}: {:?}", warnings(id, &with(id, &lt)));
    }
}

/// A typed lean to the left (−3/2) leans the solid left and its label has no sign.
#[test]
fn typed_lean_left() {
    for id in IDS {
        let p = with(id, &[s("stands", "Oblique"), s("lean_t", "-3/2"), s("lean_lab", "Measure")]);
        assert!(close(super::sv(&p, "lean"), -1.5));
        let st = def(id).svg(&p);
        assert!(texts(&st).iter().any(|t| t.3 == "3/2 cm"), "{id}");
        assert!(!st.contains("\u{2212}3/2"), "{id}: the lean's label carries a sign");
        let shapes = |x: &str| x.split('<').filter(|e| ["polygon", "polyline", "line ", "path", "circle"].iter().any(|t| e.starts_with(t))).map(|e| e.to_string()).collect::<Vec<_>>();
        assert_eq!(shapes(&st), shapes(&svg(id, &[s("stands", "Oblique"), f("lean", -1.5), s("lean_lab", "Measure")])), "{id}: not the slider's lean of \u{2212}1.5");
    }
}

/// Worked-out measures from typed exact values, checked by hand, rounded to the figure's decimals as before.
#[test]
fn worked_out_measures_by_hand() {
    // cylinder: height 4, lean 3 -> slanted edge 5 (3-4-5)
    let p = with("cylinder", &[s("stands", "Oblique"), s("height_t", "4"), s("lean_t", "3")]);
    assert_eq!(comp("cylinder", &p, "edge"), "5");
    // a typed slanted edge sqrt(41) with lean 4 works the height out to 5, shown in the height box and rounded on the label
    let p = with("cylinder", &[s("stands", "Oblique"), s("lean_t", "4"), s("edge", "sqrt(41)"), s("height_t", "9/2"), s("h_lab", "Measure")]);
    assert_eq!(comp("cylinder", &p, "height_t"), "5");
    assert!(texts(&def("cylinder").svg(&p)).iter().any(|t| t.3 == "5 cm"), "the worked-out height is rounded, not the ignored 9/2");
    // cone: radius 3/2, height 2 -> slant 5/2
    let p = with("cone", &[s("radius_t", "3/2"), s("height_t", "2")]);
    assert_eq!(comp("cone", &p, "slant"), "2.5");
    assert_eq!(comp("cone", &p, "diameter"), "3");
    // cone: radius √2 -> diameter 2√2 = 2.83 -> 2.8 to tenths
    let p = with("cone", &[s("radius_t", "sqrt2")]);
    assert_eq!(comp("cone", &p, "diameter"), "2.8");
    // cone: radius 2√3, slant 4√3 -> height 6 (right triangle 2√3, 6, 4√3)
    let p = with("cone", &[s("radius_t", "2sqrt3"), s("slant", "4sqrt3")]);
    assert!(close(super::cone_height(&p), 6.0));
    assert_eq!(comp("cone", &p, "height_t"), "6");
    // pyramid: square base 6, height 4 -> slant height 5 (3-4-5)
    let p = with("pyramid", &[s("length_t", "6"), s("width_t", "6"), s("height_t", "4")]);
    assert_eq!(comp("pyramid", &p, "slant"), "5");
    // pyramid: hexagon side 2√3 -> apothem 3
    let p = with("pyramid", &[s("base", "Regular polygon"), ("n", Value::I(6)), s("side_t", "2sqrt3")]);
    assert_eq!(comp("pyramid", &p, "apothem"), "3");
    // prism: right-triangle base 3/2 by 2 -> hypotenuse 5/2
    let p = with("prism", &[s("base", "Right triangle"), s("length_t", "3/2"), s("width_t", "2")]);
    assert_eq!(comp("prism", &p, "hyp"), "2.5");
    // prism: isosceles base 8/1 by 3 -> slanted side 5
    let p = with("prism", &[s("base", "Isosceles triangle"), s("length_t", "8/1"), s("width_t", "3")]);
    assert_eq!(comp("prism", &p, "hyp"), "5");
    // prism oblique: height 5√3 and lean 5 -> slanted edge 10
    let p = with("prism", &[s("stands", "Oblique"), s("height_t", "5sqrt3"), s("lean_t", "5")]);
    assert_eq!(comp("prism", &p, "edge"), "10");
    // the grey hint of an empty box is the slider's value, unrounded to the figure's decimals
    let p = with("prism", &[f("length", 7.25)]);
    assert_eq!(comp("prism", &p, "length_t"), "7.25");
}

/// What cannot be drawn is refused with a note, and the slider's value is drawn (the figure is the empty box's figure).
#[test]
fn refuses_and_falls_back_to_the_slider() {
    let bad = ["0", "-2", "abc", "1/0", "1000", "3 1/2", "0.01", "sqrt(-1)", "2^3", "1e3", "x", "5cm"];
    for id in IDS {
        for (key, lab) in [("height", if matches!(id, "cylinder" | "cone") { "h_lab" } else { "height_lab" }), ("lean", "lean_lab")] {
            let tk: &'static str = Box::leak(format!("{key}_t").into_boxed_str());
            let base = [s("stands", "Oblique"), s(lab, "Measure")];
            let plain = svg(id, &base);
            for t in bad.iter().copied().filter(|t| !(key == "lean" && matches!(*t, "-2" | "0.01"))).chain(if key == "lean" { Some("-50") } else { None }) {
                let mut l = base.to_vec();
                l.push(s(tk, t));
                let p = with(id, &l);
                assert_eq!(def(id).svg(&p), plain, "{id} {key}={t} drew something other than the slider");
                let w = warnings(id, &p);
                assert_eq!(w.len(), 1, "{id} {key}={t}: {w:?}");
                assert!(w[0].contains("slider"), "{w:?}");
                assert!(param_relevant(id, key, &p), "{id}: the slider must show while {t} is refused");
            }
        }
        // a zero lean is a right solid, said so
        let p = with(id, &[s("stands", "Oblique"), s("lean_t", "0")]);
        assert!(warnings(id, &p)[0].contains("right solid"));
        // a lean typed on a right solid draws nothing and says nothing
        let p = with(id, &[s("lean_t", "abc")]);
        assert!(warnings(id, &p).is_empty());
    }
    // the radius and diameter boxes count only when that row is given
    let p = with("cylinder", &[s("diam_t", "abc")]);
    assert!(warnings("cylinder", &p).is_empty());
    let p = with("cylinder", &[s("diam_t", "abc"), b("give_d", true)]);
    assert_eq!(warnings("cylinder", &p).len(), 1);
}

/// The ends of the typed range draw a bounded figure with clean numbers.
#[test]
fn typed_extremes_stay_bounded() {
    for id in IDS {
        for (a, bb) in [("100", "0.1"), ("0.1", "100"), ("1/10", "100"), ("100", "100")] {
            let l = [s("radius_t", a), s("length_t", a), s("width_t", bb), s("side_t", a), s("height_t", bb), s("stands", "Oblique"), s("lean_t", "40"), s("label_size", "Large")];
            let st = svg(id, &l);
            assert!(st.len() < 60_000, "{id}: {} bytes", st.len());
            assert!(!st.contains("NaN") && !st.contains("inf"), "{id}");
            for t in texts(&st) { assert!(!t.3.contains("0000") && !t.3.contains("9999"), "{id}: {}", t.3); }
            assert!(warnings(id, &with(id, &l)).is_empty(), "{id} {a} {bb}: {:?}", warnings(id, &with(id, &l)));
        }
    }
}

// ───────────────────────── height lines ─────────────────────────

#[test]
fn oblique_cylinder_height_switches_off() {
    let on = svg("cylinder", &[s("stands", "Oblique")]);
    let off = svg("cylinder", &[s("stands", "Oblique"), b("obl_height", false)]);
    assert_ne!(on, off, "a dead switch");
    assert_eq!(dashed_lines(&on), 2, "height and lean lines");
    assert_eq!(dashed_lines(&off), 0);
    assert!(texts(&on).iter().any(|t| t.3 == "4 cm") && !texts(&off).iter().any(|t| t.3 == "4 cm"), "the height's label goes with its line");
    // the slanted edge (here labelled) stays, and the panel says what went
    let p = with("cylinder", &[s("stands", "Oblique"), b("obl_height", false), s("edge_lab", "Measure")]);
    assert!(texts(&def("cylinder").svg(&p)).iter().any(|t| t.3.starts_with("4.5")));
    assert!(warnings("cylinder", &p)[0].contains("height line off"));
    // shown under Oblique only, where the old switch (dead there) is hidden
    assert!(param_relevant("cylinder", "obl_height", &p) && !param_relevant("cylinder", "show_height", &p));
    let r = with("cylinder", &[]);
    assert!(!param_relevant("cylinder", "obl_height", &r) && param_relevant("cylinder", "show_height", &r));
    // a right cylinder is untouched by it
    assert_eq!(svg("cylinder", &[b("obl_height", false)]), svg("cylinder", &[]));
    assert!(warnings("cylinder", &with("cylinder", &[b("obl_height", false)])).is_empty());
}

#[test]
fn prism_height_switches_off() {
    for base in ["Rectangle", "Right triangle", "Isosceles triangle", "Regular polygon"] {
        let on = svg("prism", &[s("base", base), s("stands", "Oblique")]);
        let off = svg("prism", &[s("base", base), s("stands", "Oblique"), b("show_height", false)]);
        assert_ne!(on, off, "{base}: a dead switch");
        assert!(dashed_lines(&on) > dashed_lines(&off), "{base}");
        assert!(texts(&on).iter().any(|t| t.3 == "4 cm") && !texts(&off).iter().any(|t| t.3 == "4 cm"), "{base}");
    }
    let p = with("prism", &[s("stands", "Oblique"), b("show_height", false)]);
    assert!(param_relevant("prism", "show_height", &p));
    assert!(!warnings("prism", &p).is_empty());
    // a right prism's height is its edge: the switch is hidden and changes nothing
    assert!(!param_relevant("prism", "show_height", &with("prism", &[])));
    assert_eq!(svg("prism", &[b("show_height", false)]), svg("prism", &[]));
    let lying = [s("base", "Right triangle"), s("sits", "Lying on its side"), s("stands", "Oblique")];
    assert!(!param_relevant("prism", "show_height", &with("prism", &lying)));
}

// ───────────────────────── panel and other figures ─────────────────────────

#[test]
fn other_figures_untouched() {
    // a sphere has no typed box: a stray key is dropped when the figure is loaded
    let d = def("sphere");
    let mut spec = crate::figures::spec::spec_for("sphere", &with_sphere(&[]));
    spec.params.insert("radius_t".into(), serde_json::json!("3/2"));
    let p = crate::figures::spec::params_from_spec(&d, &spec);
    assert!(!p.0.contains_key("radius_t"));
    assert_eq!(d.svg(&p), d.svg(&with_sphere(&[])));
    assert!(param_relevant("sphere", "radius", &p));
    assert!(crate::figures::param_relevant("number-line", "min", &Params::default()));
    assert!(!crate::figures::typed_over_slider("radius") && crate::figures::typed_over_slider("radius_t") && !crate::figures::typed_over_slider("edge"));
    // the card's rows: the four solids' boxes are typed, the sphere keeps its number box
    for id in IDS {
        let c = crate::figures::measures_for(id).unwrap();
        let keys: Vec<&str> = c.groups.iter().flat_map(|g| g.rows.iter().map(|r| r.value_key)).collect();
        for (_, tk) in super::solids_b8::keys(id) { assert!(keys.contains(tk), "{id}: no {tk} row"); }
        for k in ["radius", "diam", "height", "lean", "length", "width", "side"] { assert!(!keys.contains(&k), "{id}: {k} still a number row"); }
    }
    let c = crate::figures::measures_for("sphere").unwrap();
    assert!(c.groups.iter().flat_map(|g| g.rows.iter()).any(|r| r.value_key == "radius"));
}
fn with_sphere(list: &[(&'static str, Value)]) -> Params {
    let d = def("sphere");
    let mut p = crate::figures::start_params(&d);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p
}

// ───────────────────────── drags ─────────────────────────

/// A knob dragged through the workspace's hit test while its measure is typed: the drawing follows the drag, the box empties
/// and the slider shows again.
#[test]
fn drags_hand_back_to_the_slider() {
    for (id, list, tip, tk) in [
        ("cylinder", vec![s("radius_t", "3/2")], "Drag to change the radius", "radius_t"),
        ("cone", vec![s("height_t", "17/3")], "Drag to change the height", "height_t"),
        ("prism", vec![s("length_t", "15/2")], "Drag to change the length", "length_t"),
        ("pyramid", vec![s("height_t", "2sqrt3")], "Drag the apex", "height_t"),
        ("pyramid", vec![s("base", "Regular polygon"), s("side_t", "5/2")], "Drag to change the side", "side_t"),
    ] {
        let p = with(id, &list);
        let hs = def(id).handles(&p, (560.0, 380.0));
        let h = hs.iter().find(|h| h.tip.starts_with(tip)).unwrap_or_else(|| panic!("{id}: no {tip}"));
        // the knob sits on the typed measure as drawn: the hit test finds it there
        let i = interact::pick(&hs, h.pos, 1.0).expect("a handle under the press");
        assert!(hs[i].tip.starts_with(tip), "{id}: {} under the {tip} knob", hs[i].tip);
        let mut r = p.clone();
        (hs[i].drag.as_ref().unwrap())(&mut r, (h.pos.0 + 12.0, h.pos.1 - 12.0));
        r.normalize(&(def(id).params)(), true);
        assert_eq!(r.s(tk), "", "{id}: the typed box keeps {}", r.s(tk));
        let key = tk.trim_end_matches("_t");
        assert!(param_relevant(id, key, &r), "{id}: slider hidden after the drag");
        assert_ne!(def(id).svg(&r), def(id).svg(&p), "{id}: the drag moved nothing");
    }
}

// ───────────────────────── label placement ─────────────────────────

/// Long typed forms (17/3, 5√13, √13/2 ...) at Large on crowded figures, from ordinary proportions to a solid ten times as tall
/// as it is wide: every label inside the page, no text on text, no label crossed by a drawn line. Read off the drawn SVG.
#[test]
fn long_typed_labels_at_large_stay_clear() { sweep(&["17/3", "5\u{221a}13", "\u{221a}13/2", "15/2", "(1+\u{221a}5)/2"], false); }

/// The same with every measure of a like size (as a worksheet's are, here 5.7 to 7.5 long): each typed label also stays by its
/// own line, nearer it than any other labelled line.
#[test]
fn long_typed_labels_stay_by_their_lines() { sweep(&["17/3", "5\u{221a}2", "15/2", "2\u{221a}13", "13/2"], true); }

fn sweep(forms: &[&str], owned: bool) {
    let mut checked = 0;
    let mut bad: Vec<String> = Vec::new();
    let mut soft = 0;
    for id in IDS {
        for (stands, size) in [("Right", "Large"), ("Oblique", "Large"), ("Right", "Medium"), ("Oblique", "Small")] {
            for (k, form) in forms.iter().enumerate() {
                let mut l = vec![s("stands", stands), s("label_size", size), s("lean_t", "5/2")];
                let all = ["radius_t", "height_t", "length_t", "width_t", "side_t"];
                for (j, tk) in all.iter().enumerate() { l.push(s(tk, forms[(k + j) % forms.len()])); }
                for pd in (def(id).params)() { if pd.key.ends_with("_lab") && !matches!(pd.key, "hyp_lab" | "apothem_lab" | "edge_lab" | "l_lab" | "slant_lab" | "d_lab") { l.push(s(pd.key, "Measure")); } }
                for base in if matches!(id, "prism" | "pyramid") { vec!["Rectangle", "Regular polygon"] } else { vec![""] } {
                    let mut l = l.clone();
                    if !base.is_empty() { l.push(s("base", base)); }
                    let p = with(id, &l);
                    let st = def(id).svg(&p);
                    let ts = ink(&st);
                    let own = super::solids_b8::last_own();
                    // each text's label id, in document order (every solid label is wrapped)
                    let ids: Vec<String> = st.split("<text").skip(1).enumerate().map(|(i, _)| {
                        let before: String = st.split("<text").take(i + 1).collect::<Vec<_>>().join("<text");
                        before.rsplit("<g data-lbl=\"").next().and_then(|r| r.split('"').next()).unwrap_or("").to_string()
                    }).collect();
                    assert_eq!(ids.len(), ts.len());
                    for ((t, bx), lid) in ts.iter().zip(&ids) {
                        if !t.contains(['/', '\u{221a}']) { continue; }
                        let Some(mine) = own.iter().find(|o| &o.0 == lid) else { continue };
                        let d = super::solids_b8::seg_box_dist(mine.1, mine.2, *bx);
                        if !owned { continue; }
                        if d > 24.0 { bad.push(format!("{id} {stands} {base} {form}: {t} is {d:.0} px from its own line")); }
                        for o in own.iter().filter(|o| &o.0 != lid) {
                            if super::solids_b8::seg_box_dist(o.1, o.2, *bx) < d { bad.push(format!("{id} {stands} {base} {form}: {t} is nearer the {} line than its own", o.0)); }
                        }
                    }
                    let (w, h) = crate::figures::svg_size(&st);
                    let ls = strokes(&st);
                    let hard: Vec<((f64, f64), (f64, f64))> = ls.iter().copied().filter(|s| outline(&st, *s)).collect();
                    for t in texts(&st) { assert!(t.2 >= 8.0); }
                    for (t, bx) in &ts {
                        if !(bx.0 >= 0.0 && bx.1 >= 0.0 && bx.2 <= w && bx.3 <= h) { bad.push(format!("{id} {stands} {base} {form}: {t} off the page {bx:?}")); }
                        // a label may lie over a dashed hidden edge (white halo) when no spot beside its own line avoids one;
                        // never over an outline
                        for (a, c) in &hard { if seg_hits_box(*a, *c, *bx) { bad.push(format!("{id} {stands} {base} {form}: an outline {a:?}-{c:?} crosses {t}")); break; } }
                        if ls.iter().any(|(a, c)| seg_hits_box(*a, *c, *bx)) { soft += 1; }
                    }
                    for (i, (a, x)) in ts.iter().enumerate() {
                        for (bb, y) in &ts[i + 1..] {
                            if !(x.2 <= y.0 || y.2 <= x.0 || x.3 <= y.1 || y.3 <= x.1) { bad.push(format!("{id} {stands} {base} {form}: {a} on {bb}")); }
                        }
                    }
                    assert!(ts.iter().any(|t| t.0.contains(['/', '\u{221a}'])), "{id}: no typed label drawn");
                    checked += 1;
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} label problems:\n{}", bad.len(), bad.join("\n"));
    assert!(soft * 10 <= checked, "{soft} of {checked} figures have a label over a dashed edge");
    assert!(checked >= 80);
}

/// The student copy and the SVG's own title and description: the solids state no answer to hide (their worked-out measures are
/// labels the teacher chooses), and the alt text never carries a typed or worked-out value.
#[test]
fn alt_text_leaks_nothing() {
    for id in IDS {
        let p = with(id, &[s("radius_t", "17/3"), s("height_t", "5sqrt13"), s("length_t", "15/2"), s("stands", "Oblique"), s("lean_t", "5/2"), s("edge_lab", "Measure"), s("l_lab", "Measure"), s("slant_lab", "Measure")]);
        let d = def(id);
        assert!(!(d.params)().iter().any(|q| q.key == crate::figures::SHOW_ANSWERS), "{id} has a student copy switch now: test it");
        let st = d.svg(&p);
        let meta: String = ["<title>", "<desc>"].iter().filter_map(|t| st.split(t).nth(1).and_then(|r| r.split('<').next())).collect();
        for leak in ["17/3", "13", "15/2", "5/2", "cm", "\u{221a}"] { assert!(!meta.contains(leak), "{id}: alt text says {leak}: {meta}"); }
    }
}
