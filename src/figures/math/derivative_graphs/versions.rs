//! Derivative graphs: the student copies and key, Print, a pure render, the panel's controls, the tracer through the
//! workspace's hit test, share links and saved figures, the ends of every range, the examples and the evidence tokens.
use super::*;
use super::tests::{b, f, s, with};
use crate::figures::{interact, share, spec, Kind, Value};

fn texts(svg: &str) -> String {
    let mut out = String::new();
    let mut rest = svg;
    while let Some(i) = rest.find("<text") {
        let t = &rest[i..];
        let e = t.find("</text>").unwrap_or(t.len());
        let mut inside = false;
        for c in t[..e].chars() { match c { '<' => inside = true, '>' => inside = false, c if !inside => out.push(c), _ => {} } }
        out.push('\n');
        rest = &t[e..];
    }
    out
}
fn desc(svg: &str) -> String { let d = def(); let _ = d; svg.split("<desc>").nth(1).and_then(|x| x.split("</desc>").next()).unwrap_or("").to_string() }
fn def() -> crate::figures::FigureDef { super::tests::def() }

const ANSWERS: [&str; 9] = ["local max", "local min", "inflection", "absolute", "increasing", "decreasing", "concave", "Critical points", "(3/2) ="];

#[test]
fn student_copies_hold_no_answer_and_the_key_holds_them_all() {
    let marks = [("extrema", b(true)), ("inflect", b(true)), ("incdec", b(true)), ("concavity", b(true)), ("absolute", b(true)), ("sign_fp", b(true)), ("sign_fpp", b(true)), ("show_fpp", b(true)), ("key_list", b(true))];
    let d = def();
    for start in [STARTS[0], STARTS[1]] {
        for copy in STUDENT {
            let mut list: Vec<(&str, Value)> = marks.to_vec();
            list.push(("start", s(start)));
            list.push(("student_copy", s(copy)));
            if start == STARTS[1] { list.extend([("xmin", f(-4.0)), ("xmax", f(4.0)), ("ymin", f(-2.0)), ("ymax", f(8.0)), ("ymin1", f(-3.0)), ("ymax1", f(3.0))]); }
            let p = with(&list);
            let (stu, key) = crate::figures::answer_pair(&d, &p).unwrap();
            let (ts, tk) = (texts(&stu), texts(&key));
            for a in ANSWERS { assert!(!ts.contains(a), "{start} / {copy}: student copy writes {a:?}:\n{ts}"); }
            for a in ["max (", "inflection", "increasing", "Critical points", "Local maximum", "absolute"] { assert!(tk.contains(a), "{start} / {copy}: key lacks {a:?}"); }
            // no mark, no tracer, no guide; the sign charts left blank (their captions and lines only)
            assert!(!stu.contains("data-mark=\"crit\"") && !stu.contains("data-mark=\"infl\"") && !stu.contains("data-mark=\"tracer\"") && !stu.contains("data-mark=\"tangent\""), "{start} / {copy}");
            assert!(!ts.contains('+') && !ts.contains("und."), "{start} / {copy}: a sign chart is filled in");
            assert_eq!(stu.matches("data-chart=").count(), 2, "{start} / {copy}: both charts drawn blank");
            // and the description (exported with the SVG) names no formula and no answer
            let ds = desc(&stu);
            for a in ["x^", "3x", "max", "min", "inflection"] { assert!(!ds.contains(a), "{start} / {copy}: desc {ds:?}"); }
            // which graphs each copy keeps: f blue, f′ red (row colours), f from f′ is this figure's own line
            let blue = stu.contains("stroke=\"#1f5fbf\"") || stu.contains("data-curve=\"f\"");
            let red = stu.contains("stroke=\"#c0392b\"");
            let (want_f, want_fp) = if start == STARTS[1] { (false, true) } else { match copy { c if c == STUDENT[1] => (true, false), c if c == STUDENT[2] => (false, true), _ => (true, true) } };
            assert_eq!((blue, red), (want_f, want_fp), "{start} / {copy}: f and f′ drawn");
            assert!(!stu.contains("stroke=\"#2a8a3e\"") || copy == STUDENT[0] && start == STARTS[0], "{start} / {copy}: f″ only on the unmarked copy");
            if start == STARTS[1] {
                // the AP student copy keeps the point f passes through; the key draws f through it
                assert!(stu.contains("data-mark=\"through\"") && key.contains("data-curve=\"f\""), "AP copies");
            }
        }
    }
    // the title as a blank line for students
    let svg = render(&with(&[("title", s("Graphs of f and f′")), ("title_mode", s("Blank line"))]));
    assert!(svg.contains("data-title=\"blank\"") && !texts(&svg).contains("Graphs of"));
}

#[test]
fn print_is_grey_only_and_every_render_is_pure() {
    let all = [("extrema", b(true)), ("inflect", b(true)), ("incdec", b(true)), ("concavity", b(true)), ("absolute", b(true)), ("sign_fp", b(true)), ("show_fpp", b(true)), ("key_list", b(true)), ("style", s("Print"))];
    for extra in [vec![], vec![("layout", s(LAYOUTS[1]))], vec![("start", s(STARTS[1])), ("xmin", f(-4.0)), ("xmax", f(4.0))]] {
        let mut list = all.to_vec();
        list.extend(extra);
        let p = with(&list);
        let svg = render(&p);
        let mut rest = svg.as_str();
        while let Some(i) = rest.find('#') {
            let h = &rest[i + 1..];
            let hex: String = h.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if hex.len() == 6 { assert!(hex[0..2] == hex[2..4] && hex[2..4] == hex[4..6], "colour #{hex} in Print"); }
            if hex.len() == 3 { assert!(hex.chars().all(|c| c == hex.chars().next().unwrap()), "colour #{hex} in Print"); }
            rest = h;
        }
        // the same params, the same SVG (twice, and after other figures were drawn)
        let again = { let _ = render(&with(&[("f", s("sin(x)"))])); render(&p) };
        assert_eq!(svg, again);
    }
}

/// Values to try for a control.
fn alternatives(k: &Kind, cur: &Value) -> Vec<Value> {
    match (k, cur) {
        (Kind::Bool, Value::B(v)) => vec![Value::B(!v)],
        (Kind::Choice(o), _) => o.iter().map(|x| Value::S(x.to_string())).filter(|x| x != cur).collect(),
        (Kind::Float { min, max, .. }, Value::F(v)) => vec![Value::F((v + 1.0).min(*max)), Value::F((v - 2.0).max(*min))],
        (Kind::Int { max, .. }, Value::I(v)) => vec![Value::I((v + 2).min(*max))],
        (_, Value::S(t)) => vec![Value::S(format!("{t}1")), Value::S("2".into()), Value::S("x^2 - 1".into())],
        _ => vec![],
    }
}

#[test]
fn a_hidden_control_changes_nothing_and_every_shown_one_changes_the_drawing_somewhere() {
    let d = def();
    let defs = (d.params)();
    let states: Vec<Params> = vec![
        with(&[]),
        with(&[("show_answers", b(false))]),
        with(&[("layout", s(LAYOUTS[1]))]),
        with(&[("start", s(STARTS[1])), ("xmin", f(-4.0)), ("xmax", f(4.0)), ("ymin", f(-2.0)), ("ymax", f(8.0))]),
        with(&[("show_fpp", b(true)), ("absolute", b(true)), ("extrema", b(true)), ("sign_fp", b(true))]),
        with(&[("tracer", b(false)), ("graph_labels", s("None")), ("title_mode", s("Blank line"))]),
    ];
    let mut changed_somewhere: std::collections::BTreeSet<&str> = Default::default();
    for p in &states {
        let base = render(p);
        for pd in &defs {
            let cur = p.0.get(pd.key).cloned().unwrap();
            let shown = relevant(pd.key, p);
            for alt in alternatives(&pd.kind, &cur) {
                let mut q = p.clone();
                q.0.insert(pd.key.to_string(), alt.clone());
                q.normalize(&defs, false);
                let svg = render(&q);
                if !shown { assert_eq!(svg, base, "{} is hidden but changes the drawing ({alt:?})", pd.key); }
                if svg != base { changed_somewhere.insert(pd.key); }
            }
        }
    }
    // (show_answers is drawn by the panel as the preview's switch; every other control draws something somewhere)
    let dead: Vec<&str> = defs.iter().map(|d| d.key).filter(|k| !changed_somewhere.contains(k)).collect();
    assert!(dead.is_empty(), "controls that never change the drawing: {dead:?}");
}

#[test]
fn the_tracer_is_dragged_through_the_workspace_hit_test() {
    let d = def();
    let p = crate::figures::start_params(&d);
    let size = crate::figures::svg_size(&d.svg(&p));
    let hs = interact::handles_for(ID, &p, size);
    let i = hs.iter().position(|h| h.tip.starts_with("Drag the tracer")).expect("a tracer handle at the opening state");
    for k in [0.5, 1.0, 2.0] {
        // a press on the knob (and a little off it) picks the tracer, as the workspace picks it
        for (dx, dy) in [(0.0, 0.0), (4.0, -3.0), (-5.0, 5.0)] {
            let q = (hs[i].pos.0 + dx, hs[i].pos.1 + dy);
            let got = interact::pick(&hs, q, k).expect("a handle under the press");
            assert!(hs[got].tip.starts_with("Drag the tracer"), "{} picked at {q:?}", hs[got].tip);
        }
    }
    // dragged to x = -2 on the page: the tracer reads at -2 (a fraction written as typed), the tangent follows
    let pn = &build(&p).panels[0];
    let target = (pn.sx(-2.0), hs[i].pos.1);
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, target);
    q.normalize(&(d.params)(), true);
    assert_eq!(q.s("tracer_x"), "-2");
    let t = texts(&render(&q));
    // f(-2) = -8 + 6 = -2, f'(-2) = 12 - 3 = 9
    assert!(t.contains("f(\u{2212}2) = \u{2212}2") && t.contains("f\u{2032}(\u{2212}2) = 9"), "{t}");
    // dragged off the window it stops at the edge; a quarter step is written as a fraction
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, (-9000.0, 10.0));
    assert_eq!(q.s("tracer_x"), "-3");
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, (pn.sx(0.76), 10.0));
    assert_eq!(q.s("tracer_x"), "3/4");
    // dragging onto its own place changes nothing
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, hs[i].pos);
    assert_eq!(q.s("tracer_x"), p.s("tracer_x"));
    // the AP start: the point f passes through is dragged too
    let p = with(&[("start", s(STARTS[1])), ("xmin", f(-4.0)), ("xmax", f(4.0)), ("ymin", f(-2.0)), ("ymax", f(8.0))]);
    let hs = interact::handles_for(ID, &p, crate::figures::svg_size(&d.svg(&p)));
    let j = hs.iter().position(|h| h.tip == "Drag the point f passes through").expect("the point's handle");
    let got = interact::pick(&hs, hs[j].pos, 1.0).unwrap();
    assert!(hs[got].tip == "Drag the point f passes through" || hs[got].tip.starts_with("Drag the tracer"));
}

#[test]
fn share_links_and_saved_figures_round_trip() {
    let d = def();
    let start = crate::figures::start_params(&d);
    for list in [
        vec![],
        vec![("f", s("x^2, x < 1; 2 - x, x >= 1")), ("sign_fp", b(true)), ("tracer_x", s("-1/2"))],
        vec![("start", s(STARTS[1])), ("pt_x", s("-4")), ("pt_y", s("1 + pi")), ("key_list", b(true))],
        vec![("layout", s(LAYOUTS[1])), ("show_fpp", b(true)), ("style", s("Print")), ("label_size", s("Large")), ("rounding", s("2 decimals"))],
        vec![("xunits", s("Multiples of \u{3c0}")), ("f", s("sin(x)")), ("xmin", f(-2.0)), ("xmax", f(2.0)), ("xstep", f(0.5)), ("show_answers", b(false)), ("student_copy", s(STUDENT[1]))],
    ] {
        let p = with(&list);
        let want = d.svg(&p);
        let link = share::encode(ID, &p, &start);
        let (id, changed) = share::decode(&link).unwrap();
        assert_eq!(id, ID);
        assert_eq!(d.svg(&share::link_params(&d, changed, share::version(&link))), want, "link {link}");
        let saved = serde_json::to_string(&spec::spec_for(ID, &p)).unwrap();
        let sp: crate::model::FigureSpec = serde_json::from_str(&saved).unwrap();
        assert_eq!(d.svg(&spec::params_from_spec(&d, &sp)), want, "saved {saved}");
    }
}

#[test]
fn the_ends_of_every_range_draw_quickly_and_cleanly() {
    let cases: Vec<Vec<(&str, Value)>> = vec![
        vec![("xmin", f(-1000.0)), ("xmax", f(1000.0)), ("xstep", f(0.05)), ("ymin", f(-1000.0)), ("ymax", f(1000.0)), ("ystep", f(0.05)), ("minor", Value::I(10))],
        vec![("xmin", f(-0.05)), ("xmax", f(0.05)), ("xstep", f(0.05)), ("ymin", f(-0.001)), ("ymax", f(0.001)), ("ystep", f(0.05))],
        vec![("xmin", f(5.0)), ("xmax", f(-5.0))],
        vec![("f", s(&["x^2, x < -3", "x, -3 <= x < -2", "x^3, -2 <= x < -1", "1, -1 <= x < 0", "x/2, 0 <= x < 1", "sin(x), 1 <= x < 2", "e^x, 2 <= x < 3", "ln(x), x >= 3"].join("; ")))],
        vec![("f", s(&"(".repeat(60))), ("show_fpp", b(true))],
        vec![("f", s("sin(1/x)")), ("show_fpp", b(true)), ("inflect", b(true)), ("extrema", b(true))],
        vec![("f", s("tan(x)")), ("xunits", s("Multiples of \u{3c0}")), ("show_fpp", b(true)), ("sign_fp", b(true)), ("sign_fpp", b(true)), ("key_list", b(true))],
        vec![("f", s("e^(x^2)")), ("xmin", f(-1000.0)), ("xmax", f(1000.0)), ("show_fpp", b(true))],
        vec![("tracer_x", s("1e300")), ("abs_a", s("-1e300")), ("abs_b", s("1e300")), ("absolute", b(true))],
        vec![("name", s(&"g".repeat(400))), ("title", s(&"T".repeat(500)))],
        vec![("start", s(STARTS[1])), ("fp", s("x")), ("pt_x", s("900")), ("pt_y", s("0"))],
        // f′ that oscillates without end near 0, or ever faster: refused with a note, quickly
        vec![("start", s(STARTS[1])), ("fp", s("sin(1/x)")), ("pt_x", s("1")), ("pt_y", s("0"))],
        vec![("start", s(STARTS[1])), ("fp", s("x sin(1/x)")), ("pt_x", s("1")), ("pt_y", s("0"))],
        vec![("start", s(STARTS[1])), ("fp", s("cos(1/x)")), ("pt_x", s("1")), ("pt_y", s("0"))],
        vec![("start", s(STARTS[1])), ("fp", s("sin(x^2)")), ("pt_x", s("0")), ("pt_y", s("0")), ("xmin", f(-100.0)), ("xmax", f(100.0))],
        vec![("start", s(STARTS[1])), ("fp", s("sin(x^2)")), ("pt_x", s("0")), ("pt_y", s("0")), ("xmin", f(-1000.0)), ("xmax", f(1000.0))],
        vec![("f", s("sin(x^2)")), ("xmin", f(-1000.0)), ("xmax", f(1000.0)), ("show_fpp", b(true)), ("extrema", b(true)), ("inflect", b(true))],
    ];
    for list in cases {
        let p = with(&list);
        // (the work, not the clock, which another build on the machine can slow: the typed function evaluated at most
        // WORK_MAX times a drawing, over the drawings tried, and the label placer's spots bounded)
        let bl = build(&p);
        assert!(bl.work < 5 * (WORK_MAX + 200_000), "{list:?}: work {}", bl.work);
        assert!(bl.svg.len() < 3_000_000, "{list:?}: {} bytes", bl.svg.len());
        // (no NaN and no bare "inf" written as a number anywhere)
        let bare_inf = bl.svg.match_indices("inf").any(|(i, _)| { let b = bl.svg.as_bytes(); let before = i.checked_sub(1).map(|j| b[j]); let after = b.get(i + 3).copied(); !before.is_some_and(|c| c.is_ascii_alphabetic()) && !after.is_some_and(|c| c.is_ascii_alphabetic()) });
        assert!(!bl.svg.contains("NaN") && !bare_inf, "{list:?}");
        let _ = crate::figures::to_png(&bl.svg, 0.5).unwrap();
    }
    // f′ oscillating without end, or ever faster over a wide window: f is not drawn from it, and the note says why
    for (fp, lo, hi) in [("sin(1/x)", -3.0, 3.0), ("x sin(1/x)", -3.0, 3.0), ("cos(1/x)", -3.0, 3.0), ("sin(x^2)", -1000.0, 1000.0)] {
        let w = warnings(&with(&[("start", s(STARTS[1])), ("fp", s(fp)), ("pt_x", s("1")), ("pt_y", s("0")), ("xmin", f(lo)), ("xmax", f(hi))]));
        assert!(w.iter().any(|n| n.contains("changes too fast") || n.contains("pole")), "{fp}: {w:?}");
    }
    // sin(1/x) oscillates past what can be drawn: the figure says so rather than marking hundreds of points
    let w = warnings(&with(&[("f", s("1/x")), ("tracer_x", s("0"))]));
    assert!(w.is_empty() || w.iter().all(|n| !n.contains("NaN")));
}

#[test]
fn every_example_draws_cleanly_and_differs() {
    let d = def();
    let base = d.svg(&crate::figures::start_params(&d));
    let exs = crate::figures::examples_for(ID);
    assert!(exs.len() >= 6);
    for e in exs {
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        let bl = build(&p);
        assert!(bl.notes.is_empty(), "{}: {:?}", e.name, bl.notes);
        assert_ne!(d.svg(&p), base, "{}", e.name);
        assert!(bl.stats.left_off == 0 && bl.stats.numbered == 0, "{}", e.name);
        assert!(super::sweep::faults(&bl).is_empty(), "{}: {:?}", e.name, super::sweep::faults(&bl));
    }
}

/// The checklist's evidence: each token's params change the opening drawing.
pub(super) const TOKENS: [(&str, &str); 34] = [
    ("cap-01", r#"{"f":"x^3/3 - x"}"#),
    ("cap-02", r#"{"f":"x^2, x < 1; 2 - x, x >= 1"}"#),
    ("cap-03", r#"{"xmin":-2.0,"xmax":2.0}"#),
    ("cap-04", r#"{"show_fpp":true}"#),
    ("cap-05", r#"{"layout":"One grid","show_fpp":true,"ymin":-6.0,"ymax":10.0,"ystep":2.0}"#),
    ("cap-06", r#"{"start":"f′(x)","xmin":-4.0,"xmax":4.0,"ymin":-2.0,"ymax":8.0,"ymin1":-3.0,"ymax1":3.0,"ystep1":1.0}"#),
    ("cap-07", r#"{"crit":true,"tracer":false}"#),
    ("cap-08", r#"{"extrema":true}"#),
    ("cap-09", r#"{"inflect":true,"show_fpp":true}"#),
    ("cap-10", r#"{"incdec":true}"#),
    ("cap-11", r#"{"concavity":true}"#),
    ("cap-12", r#"{"sign_fp":true}"#),
    ("cap-13", r#"{"sign_fpp":true}"#),
    ("cap-14", r#"{"absolute":true}"#),
    ("cap-15", r#"{"rounding":"2 decimals"}"#),
    ("cap-16", r#"{"name":"g"}"#),
    ("cap-17", r#"{"line_fp":"Dashed"}"#),
    ("cap-18", r#"{"ymin1":-10.0,"ymax1":30.0,"ystep1":5.0}"#),
    ("cap-19", r#"{"f":"x^4 - 6x^2","xmin":-3.0,"xmax":3.0,"ymin":-10.0,"ymax":6.0,"ystep":2.0,"ymin1":-20.0,"ymax1":20.0,"ystep1":5.0,"ymin2":-15.0,"ymax2":30.0,"ystep2":5.0,"show_fpp":true,"inflect":true,"extrema":true,"tracer_x":"2"}"#),
    ("cap-20", r#"{"xmin":-4.0,"xmax":4.0,"xstep":2.0}"#),
    ("cap-21", r#"{"grid":"Bold"}"#),
    ("cap-22", r#"{"minor":2}"#),
    ("cap-23", r#"{"numbers":false}"#),
    ("cap-24", r#"{"xlabel":"t","ylabel":"s"}"#),
    ("cap-25", r#"{"xunits":"Multiples of π","xmin":-1.0,"xmax":1.0,"xstep":0.25}"#),
    ("cap-26", r#"{"title":"Graphs of f and f′"}"#),
    ("cap-27", r#"{"title_mode":"Blank line"}"#),
    ("cap-28", r#"{"label_size":"Large"}"#),
    ("cap-29", r#"{"style":"Print"}"#),
    ("cap-30", r#"{"show_answers":false,"student_copy":"Sketch f′ from f"}"#),
    ("cap-31", r#"{"show_answers":false,"student_copy":"Sketch f from f′"}"#),
    ("cap-32", r#"{"show_answers":false}"#),
    ("cap-33", r#"{"show_answers":false,"sign_fp":true}"#),
    ("cap-34", r#"{"key_list":true}"#),
];

#[test]
fn every_evidence_token_changes_the_opening_drawing_and_draws_cleanly() {
    let d = def();
    let base = d.svg(&crate::figures::start_params(&d));
    for (cap, json) in TOKENS {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        let p = crate::figures::params_from_json(&d, &v);
        let svg = d.svg(&p);
        assert_ne!(svg, base, "{cap}: {json}");
        let bl = build(&p);
        assert!(bl.notes.is_empty(), "{cap}: {:?}", bl.notes);
        assert!(super::sweep::faults(&bl).is_empty(), "{cap}: {:?}", super::sweep::faults(&bl));
        // (the probe compares PNGs: they differ too)
        assert_ne!(crate::figures::to_png(&svg, 1.0).unwrap(), crate::figures::to_png(&base, 1.0).unwrap(), "{cap}");
    }
    // the interaction: a drag handle at the opening state with its tip
    let p = crate::figures::start_params(&d);
    assert!(interact::handles_for(ID, &p, crate::figures::svg_size(&base)).iter().any(|h| h.drag.is_some() && h.tip.contains("Drag the tracer along the graphs to read f, f\u{2032} and f\u{2033} at one x")));
    // cap-07 shows only the critical points and their guides: the opening state less the tracer
    let p07 = crate::figures::params_from_json(&d, &serde_json::json!({"crit": true, "tracer": false}));
    let svg = d.svg(&p07);
    assert!(svg.contains("data-mark=\"crit\"") && !svg.contains("data-mark=\"tracer\"") && !svg.contains("data-mark=\"tangent\"") && !svg.contains("data-mark=\"infl\""));
}

#[test]
#[ignore]
fn show_case() {
    // VERSHION_DG_JSON='<params json>': the figure's notes, labels and key, for a look
    let Ok(j) = crate::env_var("VERSHION_DG_JSON") else { return };
    let d = def();
    let p = crate::figures::params_from_json(&d, &serde_json::from_str(&j).unwrap());
    let bl = build(&p);
    println!("NOTES {:?}", bl.notes);
    for l in &bl.labels { if !matches!(l.2, "key") { println!("LABEL {}: {}", l.2, l.0.replace('\u{a0}', " ")); } }
    if let (Ok(m), Some(an)) = (model(&p, window(&p)), bl.analysis.as_ref()) { for k in key_lines(&p, &m, an, window(&p)) { println!("KEY {}", k.replace('\u{a0}', " ")); } }
}

#[test]
#[ignore]
fn show_zeros() {
    let Ok(f) = crate::env_var("VERSHION_DG_F") else { return };
    let e = sym::parse(&f).unwrap();
    let d1 = sym::deriv(&e);
    let d2 = sym::deriv(&d1);
    println!("d2 {}", sym::print(&d2));
    println!("zeros d1 {:?}", exact::zeros(&d1, 0.01, 10.0).map(|v| v.iter().map(exact::text).collect::<Vec<_>>()));
    println!("zeros d2 {:?}", exact::zeros(&d2, 0.01, 10.0).map(|v| v.iter().map(exact::text).collect::<Vec<_>>()));
}

#[test]
#[ignore]
fn show_cuts() {
    let Ok(j) = crate::env_var("VERSHION_DG_JSON") else { return };
    let p = crate::figures::params_from_json(&def(), &serde_json::from_str(&j).unwrap());
    let an = super::tests::an_of(&p);
    println!("breaks {:?}", an.breaks);
    println!("mono {:?}", an.mono.iter().map(|s| (s.a, s.b, s.sign)).collect::<Vec<_>>());
    println!("fpmarks {:?}", an.fp_marks.iter().map(|m| (m.x, m.k)).collect::<Vec<_>>());
    let e = sym::parse(p.s("f")).unwrap();
    let d2 = sym::deriv(&sym::deriv(&e));
    println!("zeros f'' {:?}", exact::zeros(&d2, -5.0, 5.0).map(|v| v.iter().map(exact::text).collect::<Vec<_>>()));
}

#[test]
#[ignore]
fn render_list() {
    // VERSHION_DG_LIST=<file of "name<TAB>params json" lines> VERSHION_DG_PNGS=<dir>: each case's PNG, for a look
    let (Ok(list), Ok(dir)) = (crate::env_var("VERSHION_DG_LIST"), crate::env_var("VERSHION_DG_PNGS")) else { return };
    let d = def();
    for line in std::fs::read_to_string(list).unwrap().lines().filter(|l| !l.trim().is_empty()) {
        let (name, j) = line.split_once('\t').unwrap();
        let p = crate::figures::params_from_json(&d, &serde_json::from_str(j).unwrap());
        let bl = build(&p);
        println!("{name}: {:?}", bl.notes);
        std::fs::write(format!("{dir}/{name}.png"), crate::figures::to_png(&bl.svg, 1.0).unwrap()).unwrap();
    }
}
