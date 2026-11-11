//! Batch 6 tests of the "transformations" figure: transformations one after another, the question version, what is
//! shown, each figure's look, vertex letters and the grid's settings; and that every figure saved or shared before
//! them still draws byte for byte as it did (`batch5`, the module as it was at e0d5cd4).
use super::*;
use crate::figures::interact;

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn rel(k: &str, p: &Params) -> bool { crate::figures::param_relevant(ID, k, p) }
fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }

/// Every `<text>` body, one per line, unescaped of nothing (the figure's texts carry no markup).
fn texts(svg: &str) -> String {
    let mut out = String::new();
    let mut rest = svg;
    while let Some(i) = rest.find("<text") {
        rest = &rest[i..];
        let Some(j) = rest.find('>') else { break };
        let body = &rest[j + 1..];
        let Some(k) = body.find("</text>") else { break };
        if !out.is_empty() { out.push('\n'); }
        out.push_str(&body[..k]);
        rest = &body[k..];
    }
    out
}

/// The `<title>` and `<desc>` the SVG carries (alt text, exported with it).
fn title_desc(svg: &str) -> String {
    let grab = |tag: &str| svg.split(&format!("<{tag}>")).nth(1).and_then(|t| t.split(&format!("</{tag}>")).next()).unwrap_or("").to_string();
    format!("{} | {}", grab("title"), grab("desc"))
}

/// A dashed helper line (a mirror line) is drawn: a `<line>` with the figure's dash.
fn mirror_drawn(svg: &str) -> bool { svg.split("<line ").skip(1).any(|l| l.split("/>").next().unwrap_or("").contains("stroke-dasharray=\"7 5\"")) }

fn polylines(svg: &str) -> Vec<String> { svg.split("<polyline ").skip(1).map(|t| t.split("/>").next().unwrap_or("").to_string()).collect() }

/// Params as the batch 5 module made them: its defaults plus `list`.
fn old_params(list: &[(&'static str, Value)]) -> Params {
    let defs = batch5::params();
    let mut p = Params::defaults(&defs);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&defs, false);
    p
}

/// Every kind of figure batch 5 could make: each transformation and its every option, centres, lines, shapes typed,
/// a path, a point, a ten-vertex polygon, an empty pre-image, the fitted grid, a grid range, decimals, a hidden
/// pre-image, the examples.
fn batch5_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
    for (_, l) in batch5::examples(ID).unwrap().into_iter().map(|e| (e.name, e.params)) { cases.push(l); }
    for a in AXES { for la in [-3.5, 0.0, 2.0] { cases.push(vec![s("op", "Reflect"), s("axis", a), f("line_a", la)]); } }
    for a in ANGLES { cases.push(vec![s("op", "Rotate"), s("angle", a)]); cases.push(vec![s("op", "Rotate"), s("angle", a), f("cx", 2.0), f("cy", -1.5)]); }
    for k in [0.5, 1.0, 1.5, 3.0] { cases.push(vec![s("op", "Dilate"), f("scale", k)]); cases.push(vec![s("op", "Dilate"), f("scale", k), f("cx", -2.0), f("cy", 3.0)]); }
    for (dx, dy) in [(-6.0, -5.0), (8.0, 8.0), (0.0, 0.0), (-8.0, 3.0)] { cases.push(vec![s("op", "Translate"), f("dx", dx), f("dy", dy)]); }
    for pre in ["0, 0\n2, 0\n3, 2\n1, 3\n-1, 2", "2, 3", "", "0, 0\n2, 0\n3, 2\n1, 3\n-1, 2\n-2, 0\n-3, -2\n0, -3\n2, -3\n4, -1", "1.5, 2\n-3, 4\n0, -2"] {
        cases.push(vec![s("pre", pre)]);
        cases.push(vec![s("pre", pre), s("op", "Rotate"), f("cx", 1.0), f("cy", -1.0)]);
        cases.push(vec![s("pre", pre), b("fit", true), s("op", "Reflect"), s("axis", AXES[4]), f("line_a", -1.0)]);
    }
    cases.push(vec![s("pre", "-4, 1\n-1, 4\n2, 1"), b("closed", false), s("op", "Reflect"), s("axis", AXES[0])]);
    cases.push(vec![s("pre", ""), b("adding", true)]);
    cases.push(vec![b("fit", true)]);
    cases.push(vec![b("fit", true), s("op", "Dilate"), f("scale", 3.0)]);
    cases.push(vec![("x_from", Value::I(-4)), ("y_to", Value::I(6)), ("x_to", Value::I(25))]);
    cases.push(vec![s("op", "Rotate"), b("show_pre", false)]);
    cases.push(vec![s("op", "Dilate"), f("scale", 2.5), ("dec", Value::I(0))]);
    cases
}

fn versions() -> Vec<Vec<(&'static str, Value)>> {
    vec![vec![], vec![s("style", "Print")], vec![b(SHOW_ANSWERS, false)], vec![s("label_size", "Large")], vec![s("label_size", "Small"), s("style", "Print"), b(SHOW_ANSWERS, false)]]
}

/// A figure saved before batch 6 (its spec holds every batch 5 key, none of the new ones) loads through the real
/// loader and draws byte for byte what batch 5 drew; a share link made then (only the keys that differ from the
/// opening state, at link version VERSION) opens the same; and its handles sit where they sat, with the same tips.
#[test]
fn a_figure_saved_or_shared_before_batch_6_renders_exactly_as_it_did() {
    let d = def();
    let base = old_params(&[]);
    let mut n = 0;
    for case in batch5_cases() {
        for extra in versions() {
            let mut l = case.clone();
            l.extend(extra);
            let old = old_params(&l);
            let was = batch5::render(&old);
            // saved
            let spec = crate::figures::spec::spec_for(ID, &old);
            let loaded = crate::figures::spec::params_from_spec(&d, &spec);
            assert!((d.render)(&loaded) == was, "saved {l:?} renders differently");
            // shared
            let link = crate::figures::share::encode(ID, &old, &base);
            let (id, changed) = crate::figures::share::decode(&link).unwrap();
            assert_eq!(id, ID);
            let opened = crate::figures::share::link_params(&d, changed, crate::figures::share::version(&link));
            assert!((d.render)(&opened) == was, "shared {l:?} renders differently");
            // handles
            let (now, then) = (handles(&loaded), batch5::handles(&old));
            assert_eq!(now.iter().map(|h| (h.tip.clone(), h.pos)).collect::<Vec<_>>(), then.iter().map(|h| (h.tip.clone(), h.pos)).collect::<Vec<_>>(), "{l:?}");
            n += 1;
        }
    }
    assert!(n > 300, "{n} old figures checked");
}

// ---------------------------------------------------------------------------------------------------------------
// transformations one after another

/// The images of each step, worked out by hand for the default triangle A(1, 1), B(4, 1), C(1, 3).
#[test]
fn each_transformation_acts_on_the_last_image() {
    let img = |l: &[(&'static str, Value)]| layout(&with(l)).images;
    // reflect in the y-axis: (-1, 1), (-4, 1), (-1, 3); then 90 clockwise about the origin, (x, y) -> (y, -x)
    let im = img(&[s("op", "Reflect"), s("axis", AXES[1]), s("then", "rotate 90 clockwise")]);
    assert_eq!(im[0], vec![(-1.0, 1.0), (-4.0, 1.0), (-1.0, 3.0)]);
    assert_eq!(im[1], vec![(1.0, 1.0), (1.0, 4.0), (3.0, 1.0)]);
    // translate (3, -2): (4, -1), (7, -1), (4, 1); then reflect in x = 2: x -> 4 - x
    let im = img(&[s("then", "reflect in x = 2")]);
    assert_eq!(im[1], vec![(0.0, -1.0), (-3.0, -1.0), (0.0, 1.0)]);
    // the order matters: reflect in x = 2 first, then translate (3, -2): (3, 1) -> (6, -1)
    let im = img(&[s("op", "Reflect"), s("axis", AXES[4]), f("line_a", 2.0), s("then", "translate 3, -2")]);
    assert_eq!(im[1], vec![(6.0, -1.0), (3.0, -1.0), (6.0, 1.0)]);
    // a half turn about (1, 1), then enlarge by 2 about the origin: B(4, 1) -> (-2, 1) -> (-4, 2)
    let im = img(&[s("op", "Rotate"), s("angle", ANGLES[1]), f("cx", 1.0), f("cy", 1.0), s("then", "dilate 2")]);
    assert_eq!(im[0], vec![(1.0, 1.0), (-2.0, 1.0), (1.0, -1.0)]);
    assert_eq!(im[1], vec![(2.0, 2.0), (-4.0, 2.0), (2.0, -2.0)]);
    // four in all: translate, then reflect in y = -x ((x, y) -> (-y, -x)), turn 90 anticlockwise about (1, 0)
    // ((x, y) -> (1 - y, x - 1)), enlarge by 1/2 about (1, 1)
    let im = img(&[s("then", "reflect in y = -x\nrotate 90 anticlockwise about (1, 0)\ndilate 1/2 about (1, 1)")]);
    assert_eq!(im[0][0], (4.0, -1.0));
    assert_eq!(im[1][0], (1.0, -4.0));
    assert_eq!(im[2][0], (5.0, 0.0));
    assert_eq!(im[3][0], (3.0, 0.5));
    // reflections in y = x + a and y = -x + a, and turns about a point, against the first transformation's own maths
    for (line, keys) in [
        ("reflect in y = x + 2", vec![s("op", "Reflect"), s("axis", AXES[6]), f("line_a", 2.0)]),
        ("reflect in y = -x - 1", vec![s("op", "Reflect"), s("axis", AXES[7]), f("line_a", -1.0)]),
        ("reflect in y = 3", vec![s("op", "Reflect"), s("axis", AXES[5]), f("line_a", 3.0)]),
        ("reflect in the x-axis", vec![s("op", "Reflect"), s("axis", AXES[0])]),
        ("rotate 270 clockwise about (2, -1)", vec![s("op", "Rotate"), s("angle", ANGLES[4]), f("cx", 2.0), f("cy", -1.0)]),
        ("turn -90 about (2, -1)", vec![s("op", "Rotate"), s("angle", ANGLES[3]), f("cx", 2.0), f("cy", -1.0)]),
        ("rotate 180 about (2, -1)", vec![s("op", "Rotate"), s("angle", ANGLES[1]), f("cx", 2.0), f("cy", -1.0)]),
        ("enlarge by 3 from (-1, 2)", vec![s("op", "Dilate"), f("scale", 3.0), f("cx", -1.0), f("cy", 2.0)]),
        ("translate (x, y) \u{2192} (x \u{2212} 4, y + 1)", vec![s("op", "Translate"), f("dx", -4.0), f("dy", 1.0)]),
    ] {
        // as a second step after a do-nothing translation, it must land where it lands as a first step
        let chain = layout(&with(&[f("dx", 0.0), f("dy", 0.0), s("then", line)])).images;
        let alone = layout(&with(&keys)).images;
        assert_eq!(chain[1], alone[0], "{line}");
        let st = &steps(&with(&[s("then", line)]))[1];
        assert_eq!(rule_text(&st.p), rule_text(&with(&keys)), "{line}");
    }
}

/// The rows under the grid: the first rule, then each later one, then each image's vertices, read off the images as
/// drawn; the letters on the grid carry one, two, three, four primes.
#[test]
fn the_key_lists_every_rule_and_every_image_as_drawn() {
    let p = with(&[s("then", "reflect in the y-axis\nrotate 180\ndilate 2")]);
    let (svg, rep) = build(&p);
    let t = texts(&svg);
    assert!(t.contains("Rule  translate: (x, y) \u{2192} (x + 3, y \u{2212} 2)"), "{t}");
    assert!(t.contains("Then  reflect in the y-axis: (x, y) \u{2192} (\u{2212}x, y)"), "{t}");
    assert!(t.contains("Then  turn 180\u{b0} about the origin"), "{t}");
    assert!(t.contains("Then  enlarge by 2 about the origin: (x, y) \u{2192} (2x, 2y)"), "{t}");
    // A(1, 1) -> (4, -1) -> (-4, -1) -> (4, 1) -> (8, 2)
    assert_eq!(rep.images.iter().map(|im| im[0]).collect::<Vec<_>>(), vec![(4.0, -1.0), (-4.0, -1.0), (4.0, 1.0), (8.0, 2.0)]);
    for (k, want) in ["A\u{2032}(4, \u{2212}1)", "A\u{2033}(\u{2212}4, \u{2212}1)", "A\u{2034}(4, 1)", "A\u{2057}(8, 2)"].iter().enumerate() {
        assert!(t.contains(want), "image {k}: {want} not listed: {t}");
        let letter = want.split('(').next().unwrap();
        assert!(t.lines().any(|l| l == letter), "{letter} not on the grid: {t}");
    }
    // every listed coordinate is where the image's vertex is drawn: its dot is at that grid point
    let fr = layout(&p).frame;
    for im in &rep.images {
        for q in im {
            let (x, y) = fr.to_svg(*q);
            assert!(svg.contains(&format!(r#"cx="{x:.2}" cy="{y:.2}""#)) || svg.contains(&format!(r#"cx="{x}" cy="{y}""#)), "no dot at {q:?}");
        }
    }
    // the student copy leaves every image to draw: the rules stay, the coordinates are blanks, no image is drawn
    let stu = render_of(&[s("then", "reflect in the y-axis"), b(SHOW_ANSWERS, false)]);
    let ts = texts(&stu);
    assert!(ts.contains("Then  reflect in the y-axis"), "{ts}");
    assert!(ts.contains("A\u{2033}(_____)") && ts.contains("A\u{2032}(_____)"), "{ts}");
    assert!(!ts.contains("(\u{2212}4, "), "{ts}");
    assert_eq!(polylines(&stu).len(), 1, "only the pre-image is drawn");
}

#[test]
fn then_lines_are_read_leniently_and_bad_ones_are_named() {
    let ok = |l: &str| parse_step(l).unwrap_or_else(|e| panic!("{l}: {e}"));
    let keys = |l: &str| ok(l).0.into_iter().map(|(k, v)| format!("{k}={v:?}")).collect::<Vec<_>>().join(" ");
    assert_eq!(keys("Translate <3, -1>"), "op=S(\"Translate\") dx=F(3.0) dy=F(-1.0)");
    assert_eq!(keys("move \u{27e8}\u{2212}2, 4\u{27e9}"), "op=S(\"Translate\") dx=F(-2.0) dy=F(4.0)");
    assert_eq!(keys("reflect over the line y = \u{2212}x + 2"), "op=S(\"Reflect\") axis=S(\"the line y = -x + a\") line_a=F(2.0)");
    assert_eq!(keys("flip in y=x"), "op=S(\"Reflect\") axis=S(\"the line y = x\") line_a=F(0.0)");
    assert_eq!(keys("reflect in x = -1.5"), "op=S(\"Reflect\") axis=S(\"the line x = a\") line_a=F(-1.5)");
    assert_eq!(keys("reflect in the y axis"), "op=S(\"Reflect\") axis=S(\"the y-axis\") line_a=F(0.0)");
    assert_eq!(keys("rotate 90\u{b0} counterclockwise around (1, -2)"), "op=S(\"Rotate\") angle=S(\"90 degrees anticlockwise\") cx=F(1.0) cy=F(-2.0)");
    assert_eq!(keys("turn 270 cw"), "op=S(\"Rotate\") angle=S(\"270 degrees clockwise\") cx=F(0.0) cy=F(0.0)");
    assert_eq!(keys("dilate 1/3 about (0, 3)"), format!("op=S(\"Dilate\") scale=F({:?}) cx=F(0.0) cy=F(3.0)", 1.0 / 3.0));
    // the look words, anywhere on the line
    let (_, look, _) = ok("reflect in y = x + 1 red dotted thick");
    assert_eq!(look, Look { colour: Some("#c0392b"), stroke: Some(Stroke::Dotted), weight: Some(4.0) });
    // values past the ranges are held, with a note; what cannot be read is named and left out
    assert_eq!(ok("translate 12, 0").2, vec!["a shift of 12 is held at 8".to_string()]);
    for bad in ["rotate 45", "reflect in y = 2x", "dilate 0", "dilate -2", "spin 90", "translate 3", "reflect"] {
        assert!(parse_step(bad).is_err(), "{bad} was read");
    }
    let p = with(&[s("then", "rotate 45\n\nreflect in the x-axis\ntranslate 1, 1\ntranslate 2, 2\ntranslate 3, 3")]);
    assert_eq!(steps(&p).len(), MAX_STEPS, "at most four in all");
    let w = warnings(&p).join("\n");
    assert!(w.contains("Then, line 1: a rotation turns 90, 180 or 270 degrees"), "{w}");
    assert!(w.contains("Then, line 6: at most 4 transformations"), "{w}");
    assert!(!w.contains("line 2"), "a blank line is no line to warn about: {w}");
    // what the figure writes back reads as the same transformation
    for line in ["reflect in y = -x - 1", "rotate 270 anticlockwise about (2, -1.5)", "dilate 1/3 about (-2, 0)", "translate -3, 0.5", "reflect in the y-axis", "rotate 180 about (0, 0) green dashed thin"] {
        let st = &steps(&with(&[s("then", line)]))[1];
        let again = step_line(&st.p, &st.look);
        let st2 = &steps(&with(&[s("then", &again)]))[1];
        assert_eq!(rule_text(&st.p), rule_text(&st2.p), "{line} -> {again}");
        assert_eq!(st.look, st2.look, "{line} -> {again}");
    }
}

/// The 6d edges: the most transformations, the biggest enlargements, ten vertices, Large letters, every image shown:
/// it renders, and an image past the largest grid is cut at its edge with a note (never drawn off the page).
#[test]
fn the_biggest_and_most_crowded_figures_stay_on_the_page() {
    let ten = "0, 0\n2, 0\n3, 2\n1, 3\n-1, 2\n-2, 0\n-3, -2\n0, -3\n2, -3\n4, -1";
    let p = with(&[s("pre", ten), s("op", "Dilate"), f("scale", 3.0), f("cx", -10.0), f("cy", -10.0), s("then", "dilate 5 about (10, 10)\ndilate 5\ndilate 5"), s("label_size", "Large"), s("pt_labels", "Letters and coordinates")]);
    let (svg, rep) = build(&p);
    assert!(layout(&p).clipped);
    assert!(svg.contains("clip-path=\"url(#tf-grid)\""));
    assert!(warnings(&p).iter().any(|w| w.contains("cut off")));
    assert!(rep.bounds.1 <= 120.0 && rep.bounds.0 >= -120.0);
    let (w, h) = svg_size(&svg);
    for r in &rep.labels { assert!(r.x0 >= -1.0 && r.x1 <= w + 1.0 && r.y0 >= -1.0 && r.y1 <= h + 1.0, "{r:?} off a {w} x {h} page"); }
    // the one-step figure never needs the clip: the farthest a first image goes is 110
    for (cx, pre) in [(-10.0, "30, 30"), (10.0, "-30, -30")] {
        assert!(!layout(&with(&[s("pre", pre), s("op", "Dilate"), f("scale", 3.0), f("cx", cx), f("cy", cx)])).clipped);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// the question version

#[test]
fn the_question_version_draws_both_figures_and_gives_nothing_away() {
    for l in [
        vec![s("op", "Reflect"), s("axis", AXES[4]), f("line_a", 2.0)],
        vec![s("op", "Rotate"), f("cx", 2.0), f("cy", -1.0), s("angle", ANGLES[3])],
        vec![s("op", "Dilate"), f("scale", 2.0), f("cx", 1.0), f("cy", 1.0)],
        vec![s("op", "Reflect"), s("axis", AXES[2])],
        vec![f("dx", 3.0), f("dy", -2.0), b("show_arrows", true)],
        vec![s("op", "Reflect"), s("axis", AXES[1]), s("then", "rotate 90 clockwise about (1, 1)\nreflect in y = -x + 1")],
    ] {
        let mut q = l.clone();
        q.extend([b(SHOW_ANSWERS, false), b("question", true)]);
        let p = with(&q);
        let (svg, rep) = build(&p);
        let t = texts(&svg);
        // both figures, the image's letters and coordinates
        // (the last image only: an in-between one would name the steps)
        assert_eq!(polylines(&svg).len(), 2, "{l:?}: the pre-image and the last image drawn");
        let k = rep.images.len() - 1;
        let a = format!("A{}", primes(k));
        assert!(t.lines().any(|x| x == a), "{l:?}: {t}");
        assert!(t.contains(&format!("{a}({}, {})", num(rep.images[k][0].0, 2), num(rep.images[k][0].1, 2))), "{l:?}: {t}");
        assert!(k == 0 || !t.contains("A\u{2032}"), "{l:?}: an in-between image is shown: {t}");
        // no rule, no mirror line or its name, no centre, no arrow: in the drawing or the alt text
        assert!(t.contains(&format!("Rule  {RULE_BLANK}")), "{t}");
        for word in ["translate", "reflect", "turn", "enlarge", "rotate", "dilate", "x = 2", "y = x", "\u{2192}", "about", "clockwise", "y-axis"] {
            assert!(!t.contains(word), "{l:?}: the question copy says {word:?}: {t}");
            assert!(!title_desc(&svg).to_lowercase().contains(word), "{l:?}: the alt text says {word:?}: {}", title_desc(&svg));
        }
        assert!(!mirror_drawn(&svg), "{l:?}: a mirror line is drawn");
        assert!(!svg.contains("fill=\"#ffffff\""), "{l:?}: a centre ring is drawn");
        assert!(!svg.contains("<polygon"), "{l:?}: an arrow is drawn");
        // and the key of the same figure has them all
        let mut k = l.clone();
        k.extend([b(SHOW_ANSWERS, true), b("question", true)]);
        let key = render_of(&k);
        assert!(!texts(&key).contains(RULE_BLANK));
        assert!(texts(&key).contains("Rule  "), "{}", texts(&key));
        // the Student + key export pair makes exactly these two
        let (stu, key2) = answer_pair(&def(), &with(&q)).unwrap();
        assert_eq!(stu, svg);
        assert_eq!(key2, key);
    }
    // without the question switch the student copy is as before: the image left to draw
    let stu = render_of(&[b(SHOW_ANSWERS, false)]);
    assert_eq!(polylines(&stu).len(), 1);
    assert!(texts(&stu).contains("Rule  translate"));
}

// ---------------------------------------------------------------------------------------------------------------
// what is shown

#[test]
fn every_show_switch_shows_or_hides_its_part() {
    let then = s("then", "reflect in x = -2\nrotate 90 about (1, 1)");
    // in-between images: three images drawn and listed, or the last only
    let all = render_of(&[then.clone()]);
    let last = render_of(&[then.clone(), b("show_between", false)]);
    assert_eq!(polylines(&all).len(), 4);
    assert_eq!(polylines(&last).len(), 2);
    assert!(texts(&all).contains("A\u{2032}(4, \u{2212}1)") && !texts(&last).contains("A\u{2032}"), "{}", texts(&last));
    assert!(texts(&last).contains("A\u{2034}("), "{}", texts(&last));
    // mirror lines and centres
    let guides = render_of(&[then.clone()]);
    let none = render_of(&[then.clone(), b("show_guides", false)]);
    assert!(mirror_drawn(&guides) && guides.contains("fill=\"#ffffff\""));
    assert!(texts(&guides).lines().any(|l| l == "x = \u{2212}2"));
    assert!(!mirror_drawn(&none) && !none.contains("fill=\"#ffffff\""));
    assert!(!texts(&none).lines().any(|l| l == "x = \u{2212}2"));
    // translation arrows: one from each vertex to its image, on the key; none on the student copy
    let arrows = render_of(&[b("show_arrows", true)]);
    assert_eq!(arrows.matches("<polygon").count(), 3);
    assert_eq!(render_of(&[b("show_arrows", true), b(SHOW_ANSWERS, false)]).matches("<polygon").count(), 0);
    assert_eq!(render_of(&[]).matches("<polygon").count(), 0);
    // an arrow runs from A(1, 1) towards A'(4, -1)
    let fr = layout(&with(&[])).frame;
    let (a, a2) = (fr.to_svg((1.0, 1.0)), fr.to_svg((4.0, -1.0)));
    let first = arrows.split("<line ").find(|l| l.contains("stroke=\"#1f5fbf\"")).unwrap();
    let num_at = |k: &str| first.split(&format!("{k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse::<f64>().unwrap();
    assert!((num_at("x1") - a.0).abs() < 6.0 && (num_at("y1") - a.1).abs() < 6.0);
    assert!(((num_at("y2") - num_at("y1")) / (num_at("x2") - num_at("x1")) - (a2.1 - a.1) / (a2.0 - a.0)).abs() < 1e-6, "along AA'");
    // point labels: none, letters, letters with coordinates
    let t = texts(&render_of(&[s("pt_labels", "None")]));
    assert!(!t.lines().any(|l| l == "A" || l == "A\u{2032}"), "{t}");
    let t = texts(&render_of(&[s("pt_labels", "Letters and coordinates")]));
    assert!(t.lines().any(|l| l == "A(1, 1)") && t.lines().any(|l| l == "A\u{2032}(4, \u{2212}1)"), "{t}");
    let t = texts(&render_of(&[s("pt_labels", "Letters and coordinates"), b(SHOW_ANSWERS, false)]));
    assert!(t.lines().any(|l| l == "A(1, 1)") && !t.contains("(4, \u{2212}1)"), "the student copy keeps the image's coordinates hidden: {t}");
    // the coordinate list under the grid
    let t = texts(&render_of(&[b("coord_list", false)]));
    assert!(!t.contains("Image") && t.contains("Rule  translate"), "{t}");
}

// ---------------------------------------------------------------------------------------------------------------
// each figure's look, vertex letters

#[test]
fn each_figure_takes_its_own_colour_line_and_thickness() {
    let pl = |l: &[(&'static str, Value)]| polylines(&render_of(l));
    let p = pl(&[s("pre_colour", "#c0392b"), s("pre_line", "Dotted"), s("pre_weight", "Thick"), s("image_colour", "#1e7b34"), s("image_line", "Dashed"), s("image_weight", "Thin")]);
    assert!(p[0].contains("stroke=\"#c0392b\"") && p[0].contains("stroke-width=\"4.0\"") && p[0].contains("stroke-dasharray=\"0.01 9.4\" stroke-linecap=\"round\""), "{}", p[0]);
    assert!(p[1].contains("stroke=\"#1e7b34\"") && p[1].contains("stroke-width=\"1.5\"") && p[1].contains("stroke-dasharray=\"7 5\""), "{}", p[1]);
    // automatic: the pre-image dashed under the image, the image solid; in a chain the in-between images dotted and
    // dash-dotted, the last solid, each its own colour
    let p = pl(&[s("then", "reflect in the y-axis\nrotate 180")]);
    assert!(p[0].contains("stroke-dasharray=\"7 5\""));
    assert!(p[1].contains("stroke-dasharray=\"0.01") && p[1].contains("#1f5fbf"), "{}", p[1]);
    assert!(p[2].contains("stroke-dasharray=\"10 ") && p[2].contains("#7d3c98"), "{}", p[2]);
    assert!(!p[3].contains("dasharray") && p[3].contains("#1e7b34") && p[3].contains("stroke-width=\"3.0\""), "{}", p[3]);
    // a Then line's own words
    let p = pl(&[s("then", "reflect in the y-axis brown dashed thick")]);
    assert!(p[2].contains("#a0522d") && p[2].contains("stroke-dasharray=\"7 5\"") && p[2].contains("stroke-width=\"4.0\""), "{}", p[2]);
    // Print: grey only, whatever colours are picked; the images still told apart by their lines
    let print = render_of(&[s("style", "Print"), s("pre_colour", "#c0392b"), s("image_colour", "#1e7b34"), s("then", "reflect in the y-axis red\nrotate 180 purple"), b("show_arrows", true), s("title_mode", "Text"), s("title", "T")]);
    for part in print.split("=\"#").skip(1) {
        let hex = &part[..6.min(part.len())];
        if hex.len() < 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) { continue; }
        assert!(hex[0..2] == hex[2..4] && hex[2..4] == hex[4..6], "Print draws a hue: {hex}");
    }
    let dashes: Vec<String> = polylines(&print).iter().map(|l| l.split("stroke-dasharray=\"").nth(1).map(|d| d.split(' ').next().unwrap().to_string()).unwrap_or_default()).collect();
    let mut seen = dashes.clone();
    seen.dedup();
    assert_eq!(seen.len(), 4, "four figures, four kinds of line in Print: {dashes:?}");
    // the dots are drawn, not invisible: a round cap on a dash as wide as the line
    let png = to_rgba(&render_of(&[s("pre_line", "Dotted"), s("pre_weight", "Thick")]), 1.0).unwrap();
    let fr = layout(&with(&[])).frame;
    let (a, b2) = (fr.to_svg((1.0, 1.0)), fr.to_svg((4.0, 1.0)));
    let dark = (a.0 as u32 + 6..b2.0 as u32 - 6).filter(|&x| { let i = ((a.1.round() as u32) * png.0 + x) as usize * 4; png.2[i] < 200 }).count();
    assert!(dark > 12, "the dotted side AB shows ({dark} dark px)");
}

#[test]
fn vertex_letters_can_be_typed() {
    let p = with(&[s("names", "P, Q, R"), s("then", "reflect in the y-axis")]);
    let t = texts(&build(&p).0);
    for want in ["P", "Q", "R", "P\u{2032}", "R\u{2033}", "P\u{2033}(\u{2212}4, \u{2212}1)"] { assert!(t.lines().any(|l| l == want) || t.contains(want), "{want}: {t}"); }
    assert!(!t.lines().any(|l| l == "A"), "{t}");
    assert!(handles(&p).iter().any(|h| h.tip == "Drag vertex Q to a grid point"));
    assert!(handles(&p).iter().any(|h| h.tip.contains("Drag P\u{2032}")) || !p.b("show_between"));
    // spaces too; long names cut to three characters; the rest lettered with letters not already used
    assert_eq!(vertex_names(&with(&[s("names", "A1 Bee   Cluster")]))[..4], ["A1".to_string(), "Bee".into(), "Clu".into(), "A".into()]);
    assert_eq!(vertex_names(&with(&[s("names", "B, A")]))[..4], ["B".to_string(), "A".into(), "C".into(), "D".into()]);
    // long letters at Large, on a crowded figure: nothing off the page
    let p = with(&[s("names", "Pa1, Qb2, Rc3, Sd4, Te5"), s("pre", "-10, -10\n10, -10\n10, 10\n-10, 10\n0, 0"), s("label_size", "Large"), s("pt_labels", "Letters and coordinates")]);
    let (svg, rep) = build(&p);
    let (w, h) = svg_size(&svg);
    for r in &rep.labels { assert!(r.x0 >= -1.0 && r.x1 <= w + 1.0 && r.y0 >= -1.0 && r.y1 <= h + 1.0, "{r:?} off a {w} x {h} page"); }
}

// ---------------------------------------------------------------------------------------------------------------
// the grid

#[test]
fn the_grid_counts_numbers_titles_letters_and_ends_as_set() {
    let vlines = |svg: &str, col: &str| svg.matches(&format!("stroke=\"{col}\" stroke-width=\"1\"")).count();
    let plain = render_of(&[]);
    assert_eq!(vlines(&plain, GRID_COLOUR), 42, "21 + 21 lines from -10 to 10");
    // count by 2 on x: -10, -8 ... 10
    assert_eq!(vlines(&render_of(&[("x_step", Value::I(2))]), GRID_COLOUR), 11 + 21);
    // count by 3 on y from -10 to 10: the edges and -9 ... 9
    assert_eq!(vlines(&render_of(&[("y_step", Value::I(3))]), GRID_COLOUR), 21 + 2 + 7);
    // gridlines: none, bold; minor lines between
    assert_eq!(vlines(&render_of(&[s("gridlines", "None")]), GRID_COLOUR), 0);
    assert!(render_of(&[s("gridlines", "Bold")]).contains("stroke=\"#b9c3d0\" stroke-width=\"1.4\""));
    assert_eq!(render_of(&[s("minor", "2 per block")]).matches("stroke-width=\"0.6\"").count(), 40);
    assert_eq!(render_of(&[s("minor", "4 per block"), ("x_step", Value::I(2)), ("y_step", Value::I(2))]).matches("stroke-width=\"0.6\"").count(), 2 * 10 * 3);
    // numbers: automatic every 2nd line; every line; none
    let nums = |svg: &str| texts(svg).lines().filter(|l| !l.is_empty() && l.trim_start_matches('-').chars().all(|c| c.is_ascii_digit())).count();
    let auto = nums(&plain);
    let every = nums(&render_of(&[s("x_num", "Every line"), s("y_num", "Every line"), s("pre", "-9, -9")]));
    assert!(every > auto + 15, "{every} vs {auto}");
    assert_eq!(nums(&render_of(&[s("x_num", "No numbers"), s("y_num", "No numbers")])), 0);
    assert!(texts(&render_of(&[("x_step", Value::I(5)), s("x_num", "Every line")])).lines().any(|l| l == "5"));
    // titles, axis letters and arrowheads, inside the page
    let svg = render_of(&[s("title_mode", "Text"), s("title", "Reflections and rotations"), s("xtitle_mode", "Text"), s("xtitle", "x"), s("ytitle_mode", "Blank line"), s("x_label", "x"), s("y_label", "y"), s("ends", "Both ends"), s("label_size", "Large")]);
    let t = texts(&svg);
    for want in ["Reflections and rotations", "x", "y"] { assert!(t.lines().any(|l| l == want), "{want}: {t}"); }
    assert_eq!(svg.matches("<polygon").count(), 4, "an arrowhead at each end of both axes");
    assert_eq!(render_of(&[s("ends", "Positive ends")]).matches("<polygon").count(), 2);
    let (w, h) = svg_size(&svg);
    for part in svg.split("<text ").skip(1) {
        let attr = |k: &str| part.split(&format!("{k}=\"")).nth(1).and_then(|t| t.split('"').next()).unwrap_or("0").parse::<f64>().unwrap_or(0.0);
        let body = part.split('>').nth(1).unwrap_or("").split("</text").next().unwrap_or("");
        let size = attr("font-size");
        assert!(size >= 8.0, "{body} at {size} px");
        if part.contains("rotate(") { continue; }
        let anchor = part.split("text-anchor=\"").nth(1).and_then(|t| t.split('"').next()).unwrap_or("start");
        let r = text_rect(attr("x"), attr("y"), size, anchor, body);
        assert!(r.x0 >= 0.0 && r.x1 <= w && r.y0 >= 0.0 && r.y1 <= h, "{body} at {r:?} off a {w} x {h} page");
    }
    // the title moves the grid down, and the handles with it
    let p = with(&[s("title_mode", "Text"), s("title", "T"), s("y_label", "y")]);
    let fr = layout(&p).frame;
    let c = handles(&p).into_iter().find(|h| h.tip.contains("vertex C")).unwrap();
    assert_eq!(c.pos, fr.to_svg((1.0, 3.0)));
    assert!(fr.py(10.0) > layout(&with(&[])).frame.py(10.0) + 30.0);
    // a long title is set smaller to fit; a long x-axis letter goes inside the grid's corner
    let svg = render_of(&[s("title_mode", "Text"), s("title", &"A very long title for a figure ".repeat(4)), s("x_label", "distance east (km)")]);
    let (w, _) = svg_size(&svg);
    for part in svg.split("<text ").skip(1) {
        let body = part.split('>').nth(1).unwrap_or("").split("</text").next().unwrap_or("");
        let x: f64 = part.split("x=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
        let size: f64 = part.split("font-size=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
        let anchor = part.split("text-anchor=\"").nth(1).and_then(|t| t.split('"').next()).unwrap_or("start");
        let r = text_rect(x, 20.0, size, anchor, body);
        assert!(r.x0 >= 0.0 && r.x1 <= w, "{body} runs off the page");
    }
    // titles only in the panel when written
    assert!(!rel("title", &with(&[])) && rel("title", &with(&[s("title_mode", "Text")])));
    assert!(!rel("show_between", &with(&[])) && rel("show_between", &with(&[s("then", "rotate 90")])));
    // no switch is offered where it would change nothing
    assert!(!rel("question", &with(&[])) && rel("question", &with(&[b(SHOW_ANSWERS, false)])));
    assert!(rel("show_arrows", &with(&[])) && !rel("show_arrows", &with(&[s("op", "Rotate")])) && rel("show_arrows", &with(&[s("op", "Rotate"), s("then", "translate 1, 1")])));
    assert!(!rel("show_guides", &with(&[])) && rel("show_guides", &with(&[s("then", "rotate 90")])));
    assert!(!rel("image_colour", &with(&[b(SHOW_ANSWERS, false)])) && rel("image_colour", &with(&[b(SHOW_ANSWERS, false), b("question", true)])));
    // and the hook is this figure's own: the mapping diagram's xtitle and the coordinate grid's title text, keys
    // shared with this figure, are not hidden by it
    for id in ["coordinate-grid", "mapping-diagram"] {
        let d = registry().into_iter().find(|d| d.id == id).unwrap();
        let mut gp = Params::defaults(&(d.params)());
        gp.set_s("title_mode", "Text");
        assert!(crate::figures::param_relevant(id, "title", &gp), "{id}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// placing a later transformation's centre or line on the grid, through the workspace's hit test

/// Presses the workspace would make: the handle `interact::pick` gives a press at `at` (SVG px), dragged to `to`.
fn press_drag(p: &mut Params, at: Pt, to: Pt) -> Option<String> {
    let hs = handles(p);
    let i = interact::pick(&hs, at, 1.0)?;
    let tip = hs[i].tip.clone();
    (hs[i].drag.as_ref()?)(p, to);
    Some(tip)
}

#[test]
fn a_later_centre_or_mirror_line_is_dragged_on_the_grid_and_written_back() {
    // the second transformation turns about (2, 1); the third reflects in x = -3
    let mut p = with(&[s("then", "rotate 90 clockwise about (2, 1) red\n\nreflect in x = -3"), s("op", "Reflect"), s("axis", AXES[0])]);
    let fr = layout(&p).frame;
    // every press within the centre's hit box goes to it (nothing else is near (2, 1))
    let c = fr.to_svg((2.0, 1.0));
    for dx in -8..=8 {
        for dy in -8..=8 {
            let hs = handles(&p);
            let i = interact::pick(&hs, (c.0 + dx as f64, c.1 + dy as f64), 1.0).unwrap();
            assert_eq!(hs[i].tip, "Drag the centre of transformation 2 (the turn)", "a press at {dx}, {dy} from the centre");
        }
    }
    let tip = press_drag(&mut p, c, fr.to_svg((3.2, -0.9))).unwrap();
    assert_eq!(tip, "Drag the centre of transformation 2 (the turn)");
    assert_eq!(p.s("then"), "rotate 90 clockwise about (3, -1) red\n\nreflect in x = -3", "only its own line is rewritten, its colour kept");
    // the image follows: A(1, 1) -> (1, -1) -> about (3, -1) clockwise: (x, y) -> (y + 4, -x + 2) -> (3, 1)
    assert_eq!(layout(&p).images[1][0], (3.0, 1.0));
    // the third's mirror line: a bar on x = -3 halfway up the grid, dragged to x = -5
    let knob = fr.to_svg((-3.0, 0.0));
    let tip = press_drag(&mut p, knob, fr.to_svg((-4.8, 2.0))).unwrap();
    assert_eq!(tip, "Drag the mirror line of transformation 3");
    assert_eq!(p.s("then"), "rotate 90 clockwise about (3, -1) red\n\nreflect in x = -5");
    assert_eq!(layout(&p).images[2][0], (-13.0, 1.0), "(3, 1) reflected in x = -5");
    // a press off every handle is the grid's (no handle when the shape is closed and not translated)
    assert!(interact::pick(&handles(&p), fr.to_svg((8.0, 8.0)), 1.0).is_none());
    // dragged onto itself it changes nothing; dragged far off it stays in range and the figure renders
    for h in handles(&p) {
        let Some(drag) = h.drag.as_ref() else { continue };
        let mut q = p.clone();
        drag(&mut q, h.pos);
        assert_eq!(q.s("then"), p.s("then"), "{}", h.tip);
        for t in [(-5000.0, 9000.0), (1e9, -1e9), (f64::NAN, 3.0)] {
            let mut q = p.clone();
            drag(&mut q, t);
            assert!((def().render)(&q).contains("<svg"));
            for st in steps(&q) { let c = centre(&st.p); assert!(c.0.abs() <= 10.0 && c.1.abs() <= 10.0 && line_a(&st.p).abs() <= 10.0, "{}", h.tip); }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// fix round: vertex labels with their coordinates on crowded figures, checked with the renderer's own text boxes

type Bx = (f64, f64, f64, f64);

fn walk(g: &resvg::usvg::Group, out: &mut Vec<(String, Bx)>) {
    for n in g.children() {
        match n {
            resvg::usvg::Node::Group(g) => walk(g, out),
            resvg::usvg::Node::Text(t) => {
                let r = t.flattened().abs_bounding_box();
                let txt: String = t.chunks().iter().map(|c| c.text()).collect();
                out.push((txt, (r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64)));
            }
            _ => {}
        }
    }
}

/// Every text's ink box as resvg lays it out with the repo's fonts, in document order.
fn ink_texts(svg: &str) -> Vec<(String, Bx)> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(&crate::figures::serif_stack(svg), &opt).expect("the SVG parses");
    let mut v = Vec::new();
    walk(tree.root(), &mut v);
    v.retain(|(t, b)| !t.trim().is_empty() && b.2 > b.0);
    v
}

fn attr(tag: &str, k: &str) -> f64 { tag.split(&format!(" {k}=\"")).nth(1).and_then(|t| t.split('"').next()).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }

/// The strokes a label may not sit on: every figure's sides, mirror lines and the axes (not gridlines or leaders).
fn strokes(svg: &str) -> Vec<(Pt, Pt)> {
    let mut v = Vec::new();
    for pl in svg.split("<polyline points=\"").skip(1) {
        let pts: Vec<Pt> = pl.split('"').next().unwrap().split(' ').filter_map(|q| q.split_once(',')).map(|(x, y)| (x.parse().unwrap(), y.parse().unwrap())).collect();
        v.extend(pts.windows(2).map(|w| (w[0], w[1])));
    }
    for l in svg.split("<line ").skip(1) {
        let tag = l.split("/>").next().unwrap();
        let sw = attr(tag, "stroke-width");
        if tag.contains("stroke-dasharray=\"7 5\"") || sw == 1.8 {
            v.push(((attr(tag, "x1"), attr(tag, "y1")), (attr(tag, "x2"), attr(tag, "y2"))));
        }
    }
    v
}

/// Every dot and ring: (centre, radius with its stroke, a ring).
fn dots_and_rings(svg: &str) -> Vec<(Pt, f64, bool)> {
    circles(svg).into_iter().zip(svg.split("<circle ").skip(1)).map(|((c, r), t)| (c, r, t.split("/>").next().unwrap_or("").contains("fill=\"#ffffff\""))).collect()
}

/// The point a label names: (5, 2) from "C′(5, 2)".
fn label_point(t: &str) -> Option<Pt> {
    let (_, xy) = t.rsplit_once('(')?;
    let (x, y) = xy.trim_end_matches(')').split_once(", ")?;
    let n = |v: &str| v.trim().replace('\u{2212}', "-").parse::<f64>().ok();
    Some((n(x)?, n(y)?))
}

/// The distance from point q to segment a-b.
fn pt_seg(q: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    (q.0 - a.0 - t * dx).hypot(q.1 - a.1 - t * dy)
}

/// The distance between segments p-q and a-b (0 when they cross).
fn seg_seg(p: Pt, q: Pt, a: Pt, b: Pt) -> f64 {
    let side = |o: Pt, s: Pt, t: Pt| (s.0 - o.0) * (t.1 - o.1) - (s.1 - o.1) * (t.0 - o.0);
    if side(a, b, p) * side(a, b, q) < 0.0 && side(p, q, a) * side(p, q, b) < 0.0 { return 0.0; }
    pt_seg(p, a, b).min(pt_seg(q, a, b)).min(pt_seg(a, p, q)).min(pt_seg(b, p, q))
}

/// The distance from a point to a box (0 inside it).
fn rect_gap(b: &Bx, q: Pt) -> f64 { (b.0 - q.0).max(q.0 - b.2).max(0.0).hypot((b.1 - q.1).max(q.1 - b.3).max(0.0)) }

/// The thin leader lines from a point to a label standing off it.
fn leaders(svg: &str) -> Vec<(Pt, Pt)> {
    svg.split("<line ").skip(1).map(|l| format!(" {}", l.split("/>").next().unwrap())).filter(|t| attr(t, "stroke-width") == 0.9).map(|t| ((attr(&t, "x1"), attr(&t, "y1")), (attr(&t, "x2"), attr(&t, "y2")))).collect()
}

/// Every dot and ring: (centre, radius with its stroke).
fn circles(svg: &str) -> Vec<(Pt, f64)> {
    svg.split("<circle ").skip(1).map(|c| { let t = format!(" {}", c.split("/>").next().unwrap()); ((attr(&t, "cx"), attr(&t, "cy")), attr(&t, "r") + 0.9) }).collect()
}

fn overlap(a: &Bx, b: &Bx) -> bool { a.0 < b.2 - 0.5 && b.0 < a.2 - 0.5 && a.1 < b.3 - 0.5 && b.1 < a.3 - 0.5 }

/// Checks every vertex label with coordinates on `svg`: on the page, off every other text, every side, mirror line
/// and axis, every dot and ring.
fn coordinate_labels_are_clear(svg: &str, fr: &Frame, what: &str) {
    let (w, h) = svg_size(svg);
    let texts = ink_texts(svg);
    let sides = strokes(svg);
    let rings = circles(svg);
    let is_label = |t: &str| t.contains('(') && !t.starts_with("Rule") && !t.starts_with("Then") && !t.starts_with("Image") && !t.starts_with(' ');
    let mut n = 0;
    for (k, (t, b)) in texts.iter().enumerate() {
        if !is_label(t) { continue; }
        n += 1;
        assert!(b.0 >= 0.0 && b.1 >= 0.0 && b.2 <= w && b.3 <= h, "{what}: {t} at {b:?} is off the {w} x {h} page");
        for (j, (u, c)) in texts.iter().enumerate() {
            if j != k && overlap(b, c) { panic!("{what}: {t} at {b:?} overlaps {u} at {c:?}"); }
        }
        let inner = (b.0 + 0.5, b.1 + 0.5, b.2 - 0.5, b.3 - 0.5);
        for &(a, e) in &sides {
            let steps = ((e.0 - a.0).hypot(e.1 - a.1) / 0.5).ceil().max(1.0) as usize;
            for st in 0..=steps {
                let q = (a.0 + (e.0 - a.0) * st as f64 / steps as f64, a.1 + (e.1 - a.1) * st as f64 / steps as f64);
                assert!(!(q.0 > inner.0 && q.0 < inner.2 && q.1 > inner.1 && q.1 < inner.3), "{what}: {t} at {b:?} is struck by a line from {a:?} to {e:?}");
            }
        }
        for &(c, r) in &rings {
            let (qx, qy) = (c.0.clamp(b.0, b.2), c.1.clamp(b.1, b.3));
            assert!((qx - c.0).hypot(qy - c.1) >= r - 0.5, "{what}: {t} at {b:?} covers a dot or ring at {c:?}");
        }
    }
    assert!(n > 0, "{what}: no coordinate labels");
    // a label reads as its own point's: no other point's dot is nearer it (than 10 px, for a label on a leader)
    let leads = leaders(svg);
    let tagged = dots_and_rings(svg);
    for (t, b) in texts.iter().filter(|(t, _)| is_label(t)) {
        let Some(at) = label_point(t) else { continue };
        let own = fr.to_svg(at);
        // the handles' frame is the drawing's, a grid drawn larger included: the point's dot is where it says
        assert!(tagged.is_empty() || tagged.iter().any(|&(c, _, ring)| !ring && (c.0 - own.0).hypot(c.1 - own.1) < 0.02), "{what}: no dot for {t} at {own:?}");
        let on_leader = leads.iter().any(|&(a, _)| (a.0 - own.0).hypot(a.1 - own.1) < 5.0);
        let lim = if on_leader { 10.0 } else { rect_gap(b, own).min(10.0) };
        for &(c, _, ring) in &tagged {
            if ring || (c.0 - own.0).hypot(c.1 - own.1) < 1.0 { continue; }
            assert!(rect_gap(b, c) >= lim - 2.0, "{what}: {t} at {b:?} sits nearer the dot at {c:?} ({:.1} px) than its own point at {own:?} ({:.1} px)", rect_gap(b, c), rect_gap(b, own));
        }
    }
    // a leader: at most 165 px; not within 3 px of another point's dot or a ring, another leader, nor along a line
    for (n, &(a, e)) in leads.iter().enumerate() {
        let len = (e.0 - a.0).hypot(e.1 - a.1);
        assert!(len <= 165.0, "{what}: the leader from {a:?} to {e:?} is {len:.0} px long");
        // (not straight across or up: it would lie along a gridline, every figure here has them)
        assert!((e.0 - a.0).abs() >= 0.07 * len && (e.1 - a.1).abs() >= 0.07 * len, "{what}: the leader from {a:?} to {e:?} lies along a gridline");
        // (it is drawn from 4 px off its point)
        let own = (a.0 - (e.0 - a.0) * 4.0 / len, a.1 - (e.1 - a.1) * 4.0 / len);
        for &(c, r, _) in &tagged {
            if (c.0 - own.0).hypot(c.1 - own.1) < 1.0 { continue; }
            let gap = pt_seg(c, a, e) - r;
            assert!(gap >= 3.0, "{what}: the leader from {a:?} to {e:?} passes {gap:.1} px from the dot or ring at {c:?}");
        }
        for (m, &(p, q)) in leads.iter().enumerate() {
            if m != n { let g = seg_seg(a, e, p, q); assert!(g >= 3.0, "{what}: the leaders from {a:?} and {p:?} come {g:.1} px apart"); }
        }
        let k = len.ceil().max(1.0) as usize;
        for &(p, q) in &sides {
            let along = (0..=k).filter(|&t| pt_seg((a.0 + (e.0 - a.0) * t as f64 / k as f64, a.1 + (e.1 - a.1) * t as f64 / k as f64), p, q) < 2.5).count();
            assert!(along <= 12, "{what}: the leader from {a:?} to {e:?} runs {along} px along the line from {p:?} to {q:?}");
        }
    }
    // a leader line (a label standing off its point) runs through no text, its own label's included
    for (a, e) in leads {
        for (t, b) in &texts {
            let inner = (b.0 + 0.5, b.1 + 0.5, b.2 - 0.5, b.3 - 0.5);
            for st in 0..=40 {
                let q = (a.0 + (e.0 - a.0) * st as f64 / 40.0, a.1 + (e.1 - a.1) * st as f64 / 40.0);
                assert!(!(q.0 > inner.0 && q.0 < inner.2 && q.1 > inner.1 && q.1 < inner.3), "{what}: the leader from {a:?} to {e:?} runs through {t} at {b:?}");
            }
        }
    }
}

fn crowded_cases() -> Vec<(String, Vec<(&'static str, Value)>)> {
    vec![
        // the checker's cases
        ("kite in y = x, then up 1 right 1".into(), vec![s("pre", "1, 2\n4, 1\n2, 5\n0, 4"), s("op", "Reflect"), s("axis", AXES[2]), s("then", "translate 1, 1")]),
        ("three steps on a small triangle".into(), vec![s("pre", "1, 1\n3, 1\n2, 3"), s("op", "Dilate"), f("scale", 1.5), f("cx", 1.0), f("cy", 1.0), s("then", "reflect in y = x\ntranslate 0, -1")]),
        ("four steps".into(), vec![s("pre", "0, 0\n2, 0\n2, 2\n0, 2"), s("then", "rotate 90 about (1, 1)\nreflect in x = 2\ndilate 1/2 about (3, 0)")]),
        ("four steps, turns about one point".into(), vec![s("op", "Rotate"), f("cx", 2.0), f("cy", 2.0), s("then", "rotate 90 about (2, 2)\nrotate 90 about (2, 2)\nreflect in y = 2")]),
        ("ten vertices and a reflection".into(), vec![s("pre", "0, 0\n2, 0\n3, 2\n1, 3\n-1, 2\n-2, 0\n-3, -2\n0, -3\n2, -3\n4, -1"), s("op", "Reflect"), s("axis", AXES[0])]),
        ("a point on its own image".into(), vec![s("pre", "0, 0\n3, 0\n0, 3"), s("op", "Reflect"), s("axis", AXES[2]), s("then", "rotate 180")]),
        ("near the grid's corner".into(), vec![s("pre", "8, 8\n10, 8\n10, 10"), s("op", "Rotate"), s("angle", ANGLES[1]), f("cx", 9.0), f("cy", 9.0), s("then", "translate -1, 0")]),
        ("a typed range of 30".into(), vec![("x_to", Value::I(30)), ("y_to", Value::I(30)), s("pre", "1, 1\n2, 1\n1, 2"), s("then", "reflect in y = x\nrotate 90 clockwise")]),
        // each kind, triangle through hexagon, shapes overlapping or sharing points
        ("a quadrilateral slid one unit twice".into(), vec![s("pre", "0, 0\n3, 0\n3, 2\n0, 2"), s("op", "Translate"), f("dx", 1.0), f("dy", 0.0), s("then", "translate 0, 1")]),
        ("a pentagon in x = 2, then y = 1".into(), vec![s("pre", "0, 0\n2, 0\n3, 2\n1, 3\n-1, 2"), s("op", "Reflect"), s("axis", AXES[4]), f("line_a", 2.0), s("then", "reflect in y = 1")]),
        ("a hexagon turned about its own vertex".into(), vec![s("pre", "0, 0\n2, 0\n3, 2\n2, 4\n0, 4\n-1, 2"), s("op", "Rotate"), s("angle", ANGLES[0]), f("cx", 2.0), f("cy", 0.0), s("then", "rotate 180 about (2, 0)")]),
        ("a pentagon enlarged twice about one vertex".into(), vec![s("pre", "1, 1\n3, 1\n4, 2\n2, 3\n0, 2"), s("op", "Dilate"), f("scale", 1.5), f("cx", 1.0), f("cy", 1.0), s("then", "dilate 2 about (1, 1)")]),
        ("a triangle turned, reflected, slid".into(), vec![s("pre", "1, 0\n3, 0\n1, 2"), s("op", "Rotate"), s("angle", ANGLES[1]), s("then", "reflect in y = -x\ntranslate 1, 1")]),
    ]
}

#[test]
fn coordinate_labels_never_overlap_text_lines_dots_or_rings() {
    for (name, case) in crowded_cases() {
        for size in ["Small", "Medium", "Large"] {
            for extra in [vec![], vec![s("style", "Print"), b("show_arrows", true)], vec![s("names", "P1, Q2, R3, S4")], vec![s("title_mode", "Text"), s("title", "T"), s("x_label", "x"), s("y_label", "y"), s("ends", "Both ends")], vec![b("fit", true), b("show_guides", false), b("coord_list", true)]] {
                let mut l = case.clone();
                l.extend([s("pt_labels", "Letters and coordinates"), s("label_size", size)]);
                l.extend(extra.clone());
                let svg = render_of(&l);
                // VERSHION_PNG_DIR=<dir> writes each figure out to look at
                if let Ok(dir) = std::env::var("VERSHION_PNG_DIR") {
                    let tag: String = format!("{name} {size} {}", extra.len()).chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
                    std::fs::write(format!("{dir}/tf-04h-{tag}.png"), crate::figures::to_png(&svg, 1.3).unwrap()).unwrap();
                }
                coordinate_labels_are_clear(&svg, &layout(&with(&l)).frame, &format!("{name} at {size} {extra:?}"));
            }
        }
    }
}

/// A point two figures share is named once, with both letters: "C, B''(2, 5)".
#[test]
fn a_shared_point_is_named_once_with_every_letter() {
    let svg = render_of(&[s("pre", "1, 2\n4, 1\n2, 5\n0, 4"), s("op", "Reflect"), s("axis", AXES[2]), s("then", "translate 1, 1"), s("pt_labels", "Letters and coordinates"), s("label_size", "Large")]);
    // C(2, 5) -> C'(5, 2) -> C''(6, 3); B(4, 1) -> B'(1, 4) -> B''(2, 5): C and B'' share (2, 5)
    let t: Vec<String> = ink_texts(&svg).into_iter().map(|(t, _)| t).collect();
    assert!(t.iter().any(|x| x == "C, B\u{2033}(2, 5)"), "{t:?}");
    assert!(!t.iter().any(|x| x == "C(2, 5)" || x == "B\u{2033}(2, 5)"), "drawn twice: {t:?}");
    // letters alone (no coordinates) are not merged: each figure keeps its own letter, as before
    let t = texts(&render_of(&[s("pre", "1, 2\n4, 1\n2, 5\n0, 4"), s("op", "Reflect"), s("axis", AXES[2]), s("then", "translate 1, 1")]));
    assert!(t.lines().any(|x| x == "C") && t.lines().any(|x| x == "B\u{2033}"), "{t}");
}

/// A centre ring is drawn under the letters: any text it touches comes after it in the drawing.
#[test]
fn a_centre_ring_never_covers_a_letter() {
    let mut cases: Vec<Vec<(&'static str, Value)>> = Vec::new();
    for (cx, cy) in [(1.0, 1.0), (0.0, 1.0), (1.0, 2.0), (4.0, 0.0), (2.0, 1.5), (1.0, 3.5)] {
        for op in ["Rotate", "Dilate"] {
            // (a lone transformation with letters only is a figure batch 5 could save, drawn as it always was)
            for extra in [vec![s("pt_labels", "Letters and coordinates")], vec![s("pt_labels", "Letters and coordinates"), s("label_size", "Large")], vec![s("then", "rotate 180 about (1, 1)")], vec![s("then", "rotate 180 about (1, 1)"), b(SHOW_ANSWERS, false), s("label_size", "Large")], vec![s("then", "dilate 2 about (0, 0)"), s("pt_labels", "Letters and coordinates"), s("label_size", "Large")]] {
                let mut l = vec![s("op", op), f("cx", cx), f("cy", cy)];
                l.extend(extra);
                cases.push(l);
            }
        }
    }
    for l in cases {
        let svg = render_of(&l);
        let texts = ink_texts(&svg);
        let starts: Vec<usize> = svg.match_indices("<text").map(|(i, _)| i).collect();
        assert_eq!(starts.len(), texts.len(), "{l:?}: every text has an ink box");
        for (ring, _) in svg.match_indices("fill=\"#ffffff\"") {
            let tag_start = svg[..ring].rfind("<circle").unwrap();
            let tag = &svg[tag_start..tag_start + svg[tag_start..].find("/>").unwrap()];
            let (c, r) = ((attr(tag, "cx"), attr(tag, "cy")), attr(tag, "r") + 0.9);
            for (k, (t, bx)) in texts.iter().enumerate() {
                let (qx, qy) = (c.0.clamp(bx.0, bx.2), c.1.clamp(bx.1, bx.3));
                if (qx - c.0).hypot(qy - c.1) < r {
                    assert!(starts[k] > tag_start, "{l:?}: the ring at {c:?} is drawn over {t}");
                }
            }
        }
    }
}
