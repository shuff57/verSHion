//! Tests of the coordinate grid's limit questions (src/figures/math/limits_grid.rs). Every limit, value and kind here was
//! worked by hand (the working is in the comments), not read back from the code.
use super::limits::{grid, Lim, Model};
use crate::figures::{examples_for, registry, start_params, FigureDef, Params};
use serde_json::{json, Value as J};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "coordinate-grid").unwrap() }
fn with(kv: J) -> Params {
    let mut p = start_params(&def());
    let over = crate::figures::params_from_json(&def(), &kv);
    for (k, v) in over.0 { if kv.get(&k).is_some() { p.0.insert(k, v); } }
    p
}
fn svg(p: &Params) -> String { super::coordinate_grid_render(p) }
fn model(kv: J) -> Model { grid::model_for(&with(kv)) }
fn kk(v: f64) -> super::limits::cas::K { let mut k = super::limits::Num::read(&format!("{v}")).unwrap().k; k.dec = false; k }
fn val(l: &Lim) -> String { match l { Lim::Val(_, t) => t.clone(), other => format!("{other:?}") } }
fn kinds(md: &Model) -> Vec<String> { md.discontinuities().0.iter().map(|d| format!("{} {}", d.text, d.kind)).collect() }
/// The text a reader sees in one data-lim group, tags dropped, the true minus read as a hyphen.
fn words_in(svg: &str, group: &str) -> String {
    let tag = format!(r#"<g data-lim="{group}">"#);
    let Some(a) = svg.find(&tag) else { return String::new() };
    let rest = &svg[a + tag.len()..];
    // (the groups hold no nested <g>)
    let body = &rest[..rest.find("</g>").unwrap_or(rest.len())];
    let mut out = String::new();
    let mut inside = false;
    // (a label's white halo repeats its words under it: it is not read)
    let mut halo = false;
    let mut tg = String::new();
    for c in body.chars() {
        match c {
            '<' => { inside = true; tg.clear(); }
            '>' => {
                inside = false;
                if tg.starts_with("text") { halo = tg.contains(r##"fill="#fff""##); }
                if tg.starts_with("/text") { halo = false; }
                if tg.starts_with("/text") || tg.starts_with("text") { out.push(' '); }
            }
            _ if inside => tg.push(c),
            _ if halo => {}
            _ => out.push(if c == '\u{2212}' { '-' } else { c }),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

const JUMP: &str = "y = x + 2, x < 1; y = 4 - 2x, x >= 1";
const THREE: &str = "y = -x - 1, x < -2; y = x^2/2 - 1, -2 <= x <= 2; y = 3, x > 2";
const EVERY: &str = "y = (x^2 - 1)/(x - 1), x < 2; y = x - 3, 2 <= x < 4; y = 2/(x - 5), x >= 4";

/// Off by default: every grid drawing the app ships (its opening state, its examples, rows of every kind) is the grid's
/// own drawing byte for byte.
#[test]
fn off_by_default_the_grid_draws_as_before() {
    let mut cases: Vec<Params> = vec![start_params(&def())];
    for e in examples_for("coordinate-grid") {
        let mut p = start_params(&def());
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        cases.push(p);
    }
    for kv in [
        json!({"equations": JUMP, "xmin": -5.0, "xmax": 5.0, "ymin": -3.0, "ymax": 6.0}),
        json!({"equations": EVERY, "points": "(1, 2)", "xmin": -3.0, "xmax": 9.0, "ymin": -6.0, "ymax": 6.0, "asymptotes": true}),
        json!({"equations": "x^2 + y^2 = 25; y > x - 2; x = 4", "xmin": -6.0, "xmax": 6.0, "ymin": -6.0, "ymax": 6.0}),
        json!({"equations": "y = sin(x)", "xunits": "Multiples of \u{3c0}", "xmin": -2.0, "xmax": 2.0, "xstep": 0.5, "style": "Print"}),
        json!({"equations": "y = 2x + 1", "show_answers": false}),
    ] { cases.push(with(kv)); }
    for p in &cases {
        assert!(!grid::active(p));
        assert_eq!(svg(p), super::coordinate_grid_build(p, &mut vec![]));
        assert!(!svg(p).contains("data-lim"));
    }
}

/// A jump at 1: from the left x + 2 → 3, from the right 4 - 2x → 2, so the limit does not exist; f(1) = 4 - 2 = 2.
#[test]
fn a_jump_worked_by_hand() {
    let kv = json!({"equations": JUMP, "xmin": -5.0, "xmax": 5.0, "ymin": -3.0, "ymax": 6.0, "lim_ask_at": "1"});
    let md = model(kv.clone());
    assert_eq!(val(&md.side(&kk(1.0), -1.0)), "3");
    assert_eq!(val(&md.side(&kk(1.0), 1.0)), "2");
    assert_eq!(md.two(&kk(1.0)), Lim::Dne);
    assert_eq!(val(&md.value(&kk(1.0))), "2");
    assert_eq!(kinds(&md), vec!["1 jump"]);
    let key = words_in(&svg(&with(kv.clone())), "block");
    assert!(key.contains("3") && key.contains("2") && key.contains("does not exist"), "{key}");
    // the student copy: the same questions, no answers
    let mut stu = kv.clone();
    stu["show_answers"] = json!(false);
    let q = words_in(&svg(&with(stu)), "block");
    assert!(q.contains("lim") && !q.contains("does not exist") && !q.contains('3'), "{q}");
}

/// Three pieces, asked at -2, 0 and 2: at -2, -(-2) - 1 = 1 and (-2)²/2 - 1 = 1, continuous with f(-2) = 1 (closed);
/// at 0, 0/2 - 1 = -1 on both sides; at 2, 4/2 - 1 = 1 from the left and 3 from the right (a jump), f(2) = 1.
#[test]
fn several_values_of_a_worked_by_hand() {
    let kv = json!({"equations": THREE, "xmin": -6.0, "xmax": 6.0, "ymin": -4.0, "ymax": 6.0, "lim_ask_at": "-2, 0, 2"});
    let md = model(kv.clone());
    assert_eq!(md.asks.len(), 3);
    for (a, l, r, f) in [(-2.0, "1", "1", "1"), (0.0, "-1", "-1", "-1"), (2.0, "1", "3", "1")] {
        assert_eq!(val(&md.side(&kk(a), -1.0)), l, "left at {a}");
        assert_eq!(val(&md.side(&kk(a), 1.0)), r, "right at {a}");
        assert_eq!(val(&md.value(&kk(a))), f, "f({a})");
    }
    assert_eq!(val(&md.two(&kk(-2.0))), "1");
    assert_eq!(md.two(&kk(2.0)), Lim::Dne);
    assert_eq!(kinds(&md), vec!["2 jump"]);
}

/// 2/(x - 3) at 3: x - 3 → 0⁻ from the left, so 2/(x - 3) → -∞, and +∞ from the right; the limit does not exist and
/// f(3) is undefined. Only row 1 is f (row 2 is another curve on the same grid).
#[test]
fn infinite_limits_and_the_rows_of_f() {
    let kv = json!({"equations": "y = 2/(x - 3); y = 1/(x + 2)^2", "xmin": -6.0, "xmax": 8.0, "ymin": -7.0, "ymax": 7.0, "lim_rows": "1", "lim_ask_at": "3"});
    let md = model(kv.clone());
    assert_eq!(md.pieces.len(), 1);
    assert_eq!(md.side(&kk(3.0), -1.0), Lim::NInf);
    assert_eq!(md.side(&kk(3.0), 1.0), Lim::PInf);
    assert_eq!(md.value(&kk(3.0)), Lim::Undef);
    let key = words_in(&svg(&with(kv.clone())), "block");
    assert!(key.contains("-\u{221e}") && key.contains("\u{221e}") && key.contains("undefined"), "{key}");
    // written as "does not exist" when the teacher asks for it
    let mut dne = kv.clone();
    dne["lim_inf_answer"] = json!("does not exist");
    let key = words_in(&svg(&with(dne)), "block");
    assert!(!key.contains('\u{221e}') && key.contains("does not exist"), "{key}");
    // with no rows named, both y = rows are f, and they overlap: the editor says so
    let mut both = kv.clone();
    both["lim_rows"] = json!("");
    let notes = super::grid_warnings(&with(both));
    assert!(notes.iter().any(|n| n.starts_with("Rows 1 and 2 both cover")), "{notes:?}");
}

/// A circle row is no function of x: with no rows named, f is the y = row only.
#[test]
fn rows_that_are_not_functions_are_not_f() {
    let md = model(json!({"equations": "x^2 + y^2 = 25; y = x + 1", "xmin": -6.0, "xmax": 6.0, "ymin": -6.0, "ymax": 6.0, "lim_ask_at": "0"}));
    assert_eq!(md.pieces.len(), 1);
    assert_eq!(val(&md.two(&kk(0.0))), "1");
    let notes = super::grid_warnings(&with(json!({"equations": "x^2 + y^2 = 25; y = x + 1", "lim_rows": "1", "lim_ask_at": "0"})));
    assert!(notes.iter().any(|n| n.starts_with("Row 1 is not a function of x")), "{notes:?}");
}

/// (2x² + 1)/(x² + 1) = 2 - 1/(x² + 1) → 2 as x → ±∞: one horizontal asymptote y = 2, dashed, labelled.
#[test]
fn the_horizontal_asymptote_from_the_end_behaviour() {
    let kv = json!({"equations": "y = (2x^2 + 1)/(x^2 + 1)", "xmin": -8.0, "xmax": 8.0, "ymin": -3.0, "ymax": 6.0, "lim_hasym": true, "lim_inf": "Both"});
    let md = model(kv.clone());
    assert_eq!(md.at_inf(1.0).map(|l| val(&l)), Some("2".into()));
    assert_eq!(md.at_inf(-1.0).map(|l| val(&l)), Some("2".into()));
    let s = svg(&with(kv));
    assert_eq!(s.matches(r#"data-lim="hasym""#).count(), 1);
    // y = 2 is at the plot's height for 2
    let g = super::grid_layout(&with(json!({"xmin": -8.0, "xmax": 8.0, "ymin": -3.0, "ymax": 6.0})));
    let y2 = g.mt + g.ph - (2.0 - g.y0) * g.uy;
    assert!(s.contains(&format!(r#"y1="{y2:.2}""#)), "the asymptote is not at y = 2");
    assert!(words_in(&s, "labels").contains("y = 2"));
    let key = words_in(&s, "block");
    assert!(key.contains("\u{221e}") && key.contains('2'), "{key}");
}

/// Every kind: (x² - 1)/(x - 1) = x + 1 with a hole at 1 (removable, limit 2, no value); at 2, 2 + 1 = 3 from the left
/// and 2 - 3 = -1 from the right (jump); at 4, 4 - 3 = 1 and 2/(4 - 5) = -2 (jump); at 5, 2/(x - 5) is infinite.
#[test]
fn every_kind_labelled_and_listed() {
    let kv = json!({"equations": EVERY, "xmin": -3.0, "xmax": 9.0, "ymin": -6.0, "ymax": 6.0, "lim_kinds": true, "lim_disc": true});
    let md = model(kv.clone());
    assert_eq!(kinds(&md), vec!["1 removable", "2 jump", "4 jump", "5 infinite"]);
    let s = svg(&with(kv.clone()));
    // (crowded: the hole at (1, 2) is hemmed in by the y-axis, the curve and x = 2's jump, so its label may be written under
    // the graph instead; every kind is on the plot or under it)
    let labels = words_in(&s, "labels");
    let under = words_in(&s, "block");
    for (x, k, n) in [(1, "removable", 1), (2, "jump", 2), (5, "infinite", 1)] {
        assert!(labels.matches(k).count() == n || under.contains(&format!("graph (no clear room): x = {x} ({k})")) || under.contains(&format!("; x = {x} ({k})")) || labels.matches(k).count() + under.matches(&format!("({k})")).count() >= n, "{k}: {labels} / {under}");
    }
    let key = words_in(&s, "block");
    assert!(key.contains("x = 1 (removable)") && key.contains("x = 2 (jump)") && key.contains("x = 4 (jump)") && key.contains("x = 5 (infinite)"), "{key}");
    // the student copy leaves the kinds and the list off
    let mut stu = kv.clone();
    stu["show_answers"] = json!(false);
    let s = svg(&with(stu));
    assert_eq!(words_in(&s, "labels"), "");
    assert!(!words_in(&s, "block").contains("removable"));
}

/// The hole with its point moved: (x² - 4)/(x - 2) = x + 2 away from 2, so the limit at 2 is 4; the point (2, 1) is f(2).
#[test]
fn a_moved_hole_takes_its_value_from_the_point() {
    let md = model(json!({"equations": "y = (x^2 - 4)/(x - 2)", "points": "(2, 1)", "xmin": -4.0, "xmax": 6.0, "ymin": -2.0, "ymax": 7.0, "lim_ask_at": "2"}));
    assert_eq!(val(&md.two(&kk(2.0))), "4");
    assert_eq!(val(&md.value(&kk(2.0))), "1");
    assert_eq!(kinds(&md), vec!["2 removable"]);
    // the points can be left out of f
    let md = model(json!({"equations": "y = (x^2 - 4)/(x - 2)", "points": "(2, 1)", "lim_points": false, "lim_ask_at": "2"}));
    assert_eq!(md.value(&kk(2.0)), Lim::Undef);
}

/// Print stays grey: nothing the limit work adds is in colour.
#[test]
fn print_is_grey() {
    let s = svg(&with(json!({"equations": EVERY, "xmin": -3.0, "xmax": 9.0, "ymin": -6.0, "ymax": 6.0, "lim_kinds": true, "lim_hasym": true, "lim_ask_at": "1, 2", "style": "Print"})));
    let added = &s[s.find(r#"<g data-lim="lines">"#).unwrap()..];
    for hex in added.match_indices('#').map(|(i, _)| &added[i + 1..(i + 7).min(added.len())]) {
        if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) { assert!(hex[0..2] == hex[2..4] && hex[2..4] == hex[4..6], "colour #{hex} in Print"); }
    }
}

/// The four classic setups each ask something, and each draws without a note about f.
#[test]
fn the_classic_setups() {
    let setups = crate::figures::classic_setups("coordinate-grid");
    assert_eq!(setups.len(), 4);
    for e in setups {
        let mut p = start_params(&def());
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        assert!(grid::active(&p), "{}", e.name);
        let s = svg(&p);
        assert!(!words_in(&s, "block").is_empty(), "{}", e.name);
        let notes = super::grid_warnings(&p);
        assert!(notes.iter().all(|n| !n.contains("f ") && !n.starts_with("Row")), "{}: {notes:?}", e.name);
    }
}

/// Check 1: no answer about a curve the grid does not draw as read. The grid draws x^(1/3) only for x ≥ 0 and cannot draw
/// floor x or log10 x, so no question is asked about them (a note says why); a row it draws as read is still f.
#[test]
fn rows_drawn_differently_are_not_asked_about() {
    for eq in ["y = x^(1/3)", "y = floor(x)", "y = log10(x)"] {
        let kv = json!({"equations": eq, "xmin": -9.0, "xmax": 9.0, "ymin": -5.0, "ymax": 5.0, "lim_ask_at": "0", "lim_kinds": true, "lim_disc": true});
        let md = model(kv.clone());
        assert!(md.pieces.is_empty(), "{eq}: still f");
        let s = svg(&with(kv.clone()));
        assert_eq!(words_in(&s, "labels"), "", "{eq}");
        let notes = super::grid_warnings(&with(kv));
        assert!(notes.iter().any(|n| n.starts_with("Row 1 is drawn differently")), "{eq}: {notes:?}");
    }
    assert_eq!(model(json!({"equations": "y = x^2 - 1", "lim_ask_at": "0"})).pieces.len(), 1);
}

/// Check 1: two rows over the same x are not one function; no limit is asked (row 1's limit is no answer for f).
#[test]
fn overlapping_rows_ask_nothing() {
    let kv = json!({"equations": "y = x^2; y = 2x + 1", "xmin": -4.0, "xmax": 4.0, "ymin": -4.0, "ymax": 8.0, "lim_ask_at": "1"});
    let s = svg(&with(kv.clone()));
    assert!(!s.contains("data-lim"), "questions drawn for two overlapping rows");
    let notes = super::grid_warnings(&with(kv));
    assert!(notes.iter().any(|n| n.starts_with("Rows 1 and 2 both cover") && n.contains("no limit question")), "{notes:?}");
}

/// Check 1: the student copy that asks the limits at infinity does not give them away in the asymptote's equation.
#[test]
fn the_student_copy_leaves_off_the_asymptote_s_equation() {
    let kv = json!({"equations": "y = (2x^2 + 1)/(x^2 + 1)", "xmin": -8.0, "xmax": 8.0, "ymin": -3.0, "ymax": 6.0, "lim_hasym": true, "lim_inf": "Both", "show_answers": false});
    let s = svg(&with(kv.clone()));
    assert_eq!(s.matches(r#"data-lim="hasym""#).count(), 1, "the dashed line stays");
    assert!(!words_in(&s, "labels").contains("y = 2"));
    let mut on = kv.clone();
    on["lim_stu_asym"] = json!(false);
    assert!(words_in(&svg(&with(on)), "labels").contains("y = 2"));
}

/// Checks 1 and 2: each kind label's box is nearer its own break's marks than any other break's (the hole at (1, 2); the
/// jump at 2 from 3 to −1; the jump at 4 from 1 to −2; the infinite break along x = 5), and no leader runs along a gridline.
#[test]
fn kind_labels_sit_by_their_own_break() {
    for size in ["Medium", "Large"] {
        let kv = json!({"equations": EVERY, "xmin": -3.0, "xmax": 9.0, "ymin": -6.0, "ymax": 6.0, "lim_kinds": true, "label_size": size});
        let s = svg(&with(kv.clone()));
        let g = super::grid_layout(&with(kv));
        let (sx, sy) = (|x: f64| g.ml + (x - g.x0) * g.ux, |y: f64| g.mt + g.ph - (y - g.y0) * g.uy);
        let fs = 15.0 * if size == "Large" { 1.4 } else { 1.0 };
        let body = &s[s.find(r#"<g data-lim="labels">"#).unwrap()..];
        let body = &body[..body.find("</g>").unwrap()];
        let under = words_in(&s, "block");
        let marks: Vec<(f64, &str, Vec<(f64, f64)>)> = vec![(1.0, "removable", vec![(1.0, 2.0)]), (2.0, "jump", vec![(2.0, 3.0), (2.0, -1.0)]), (4.0, "jump", vec![(4.0, 1.0), (4.0, -2.0)]), (5.0, "infinite", (0..=24).map(|k| (5.0, -6.0 + k as f64 * 0.5)).collect())];
        let dist = |b: [f64; 4], ms: &[(f64, f64)]| ms.iter().map(|(x, y)| { let (px, py) = (sx(*x), sy(*y)); let dx = (b[0] - px).max(px - b[2]).max(0.0); let dy = (b[1] - py).max(py - b[3]).max(0.0); dx.hypot(dy) }).fold(f64::MAX, f64::min);
        let mut placed: Vec<f64> = vec![];
        // (the leaders: start, end)
        let leaders: Vec<((f64, f64), (f64, f64))> = body.split("<line").skip(1).filter(|p| p.contains(r#"data-lim="leader""#)).map(|part| {
            let num = |k: &str| -> f64 { part.split(&format!(" {k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
            ((num("x1"), num("y1")), (num("x2"), num("y2")))
        }).collect();
        for part in body.split("<text").skip(1) {
            if part.contains(r##"fill="#fff""##) { continue; }
            let num = |k: &str| -> f64 { part.split(&format!(" {k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
            let (x, y) = (num("x"), num("y"));
            let word: String = { let mut w = String::new(); let mut inside = true; for c in part.chars() { match c { '<' => inside = true, '>' => inside = false, _ if !inside => w.push(c), _ => {} } } w.trim().to_string() };
            let b = [x, y - 0.75 * fs, x + super::limits::tw(&word, fs), y + 0.25 * fs];
            // (a label on a leader starts at its own mark, and its text, too, is nearest its own break's marks)
            if let Some((st, _)) = leaders.iter().find(|(_, e)| e.0 >= b[0] - 6.0 && e.0 <= b[2] + 6.0 && e.1 >= b[1] - 6.0 && e.1 <= b[3] + 6.0) {
                let from = marks.iter().min_by(|a, c| dist([st.0, st.1, st.0, st.1], &a.2).partial_cmp(&dist([st.0, st.1, st.0, st.1], &c.2)).unwrap()).unwrap();
                assert_eq!(from.1, word, "{size}: the {word} label's leader starts at the {} at x = {}", from.1, from.0);
            }
            let nearest = marks.iter().min_by(|a, c| dist(b, &a.2).partial_cmp(&dist(b, &c.2)).unwrap()).unwrap();
            assert_eq!(nearest.1, word, "{size}: the {word} label {b:?} is nearest the {} at x = {}", nearest.1, nearest.0);
            placed.push(nearest.0);
        }
        for (x, word) in [(1.0, "removable"), (2.0, "jump"), (4.0, "jump"), (5.0, "infinite")] {
            if !placed.contains(&x) { assert!(under.contains(&format!("x = {x} ({word})")), "{size}: the {word} at {x} is neither on the plot nor under it: {placed:?} / {under}"); }
        }
        for part in body.split("<line").skip(1).filter(|p| p.contains(r#"data-lim="leader""#)) {
            let num = |k: &str| -> f64 { part.split(&format!(" {k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
            let (dx, dy) = ((num("x1") - num("x2")).abs(), (num("y1") - num("y2")).abs());
            assert!(dx >= 0.25 * dy && dy >= 0.25 * dx, "{size}: a leader runs along a gridline");
        }
    }
}

/// Renders the kind-label example to PNG for a look (GRID_LIM_OUT, default /tmp/cgl).
#[test]
#[ignore]
fn dump_kind_label_pngs() {
    let dir = std::env::var("GRID_LIM_OUT").unwrap_or_else(|_| "/tmp/cgl".into());
    for size in ["Medium", "Large"] {
        for (name, eq, x0) in [("every", EVERY, -3.0), ("roomy", ROOMY, -5.0)] {
            let s = svg(&with(json!({"equations": eq, "xmin": x0, "xmax": 9.0, "ymin": -6.0, "ymax": 6.0, "lim_kinds": true, "lim_disc": true, "label_size": size})));
            std::fs::write(format!("{dir}/{name}_{size}.png"), crate::figures::to_png(&s, 1.0).unwrap()).unwrap();
        }
    }
}

/// Check 3: a hole at x = 1 beside a jump at x = 1.5 ((x² − 1)/(x − 1) = x + 1 for x < 1.5, so the hole is at (1, 2) and the
/// left limit at 1.5 is 2.5; 5 after): "removable" is never by the jump's open circle at (1.5, 2.5), on a leader or not.
#[test]
fn a_leadered_label_is_not_by_another_break() {
    for (lo, hi) in [(-3.0, 4.0), (-5.0, 5.0)] {
        let kv = json!({"equations": "y = (x^2 - 1)/(x - 1), x < 1.5; y = 5, x >= 1.5", "xmin": lo, "xmax": hi, "ymin": -1.0, "ymax": 6.0, "lim_kinds": true, "lim_disc": true});
        let s = svg(&with(kv.clone()));
        let g = super::grid_layout(&with(kv));
        let (sx, sy) = (|x: f64| g.ml + (x - g.x0) * g.ux, |y: f64| g.mt + g.ph - (y - g.y0) * g.uy);
        let body = &s[s.find(r#"<g data-lim="labels">"#).unwrap()..];
        let body = &body[..body.find("</g>").unwrap()];
        for part in body.split("<text").skip(1).filter(|p| !p.contains(r##"fill="#fff""##) && p.contains("removable")) {
            let num = |k: &str| -> f64 { part.split(&format!(" {k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
            let b = [num("x"), num("y") - 11.0, num("x") + super::limits::tw("removable", 15.0), num("y") + 4.0];
            let d = |px: f64, py: f64| { let dx = (b[0] - px).max(px - b[2]).max(0.0); let dy = (b[1] - py).max(py - b[3]).max(0.0); dx.hypot(dy) };
            let (own, jump) = (d(sx(1.0), sy(2.0)), d(sx(1.5), sy(2.5)).min(d(sx(1.5), sy(5.0))));
            assert!(own + 6.0 <= jump, "[{lo}, {hi}]: removable is {own} px from its hole and {jump} px from the jump");
        }
    }
}

const ROOMY: &str = "y = (x^2 - 4)/(x + 2), x < 1; y = 4 - x, 1 <= x < 3; y = 3/(x - 5), x >= 3";

/// The checklist's example for the kind labels, with room: (x² − 4)/(x + 2) = x − 2 off −2, a hole at (−2, −4); at 1,
/// 1 − 2 = −1 from the left and 4 − 1 = 3 from the right (jump); at 3, 4 − 3 = 1 and 3/(3 − 5) = −3/2 (jump); at 5,
/// 3/(x − 5) is infinite. Every label is on the plot, by its own break.
#[test]
fn every_kind_labelled_on_the_plot_with_room() {
    let kv = json!({"equations": ROOMY, "xmin": -5.0, "xmax": 9.0, "ymin": -6.0, "ymax": 6.0, "lim_kinds": true, "lim_disc": true});
    assert_eq!(kinds(&model(kv.clone())), vec!["-2 removable", "1 jump", "3 jump", "5 infinite"]);
    let s = svg(&with(kv));
    let labels = words_in(&s, "labels");
    assert_eq!((labels.matches("removable").count(), labels.matches("jump").count(), labels.matches("infinite").count()), (1, 2, 1), "{labels}");
    assert!(!words_in(&s, "block").contains("Not labelled on the graph"));
}

/// Check 4: where there is room beside a break, its kind is on the graph, at every size: the roomy example, a hole at
/// (1, 2) beside a jump at 1.5, a hole at (1, −1) beside a vertical asymptote at 2 ((x − 1)/((x − 1)(x − 2)) = 1/(x − 2)
/// off 1), and (x² − 1)/(x² − 3x + 2) = (x + 1)/(x − 2) off 1 (a hole at (1, −2)).
#[test]
fn kinds_with_room_are_on_the_graph() {
    let cases = [
        (ROOMY, -5.0, 9.0, -6.0, 6.0, "Medium"), (ROOMY, -5.0, 9.0, -6.0, 6.0, "Large"),
        ("y = (x^2 - 1)/(x - 1), x < 1.5; y = 5, x >= 1.5", -3.0, 4.0, -1.0, 6.0, "Medium"),
        ("y = (x - 1)/((x - 1)(x - 2))", -4.0, 6.0, -5.0, 5.0, "Medium"),
        ("y = (x^2 - 1)/(x^2 - 3x + 2)", -4.0, 6.0, -6.0, 8.0, "Medium"),
    ];
    for (eq, a, b, c, d, size) in cases {
        let s = svg(&with(json!({"equations": eq, "xmin": a, "xmax": b, "ymin": c, "ymax": d, "lim_kinds": true, "label_size": size})));
        assert!(!words_in(&s, "block").contains("Not labelled"), "{eq} at {size}: {}", words_in(&s, "block"));
        assert!(words_in(&s, "labels").contains("removable"), "{eq} at {size}: {}", words_in(&s, "labels"));
    }
}
