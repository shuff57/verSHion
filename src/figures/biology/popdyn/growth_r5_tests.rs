//! Population growth, checker round 4: with fixed axes and y max below K, the K label (or its blank line) is drawn
//! above the plot, up among the phase brackets; no phase or inflection blank line or leader may touch it, the title, a
//! curve, other text or each other.
use super::growth_r3_tests::{attr, pt_seg, seg_box, seg_seg, sweep, tagged, Rng};
use super::popdyn_b9_tests::{dark_segments, def, ink_texts, params, s};
use crate::figures::Value;

const PG: &str = "population-growth";
const MODES: [&str; 3] = ["Text", "Blank line", "None"];
type Case = Vec<(&'static str, Value)>;
fn f(v: f64) -> Value { Value::F(v) }
fn b(v: bool) -> Value { Value::B(v) }

/// The checker's two figures.
fn checker_cases() -> Vec<Case> {
    vec![
        vec![("mark_infl", b(true)), ("mark_phases", b(true)), ("infl_label_style", s("Blank line")), ("phase_label_style", s("Blank line")), ("fit", b(false)), ("y_max", f(900.0)), ("x_max", f(20.0))],
        vec![("mark_infl", b(true)), ("mark_phases", b(true)), ("infl_label_style", s("Blank line")), ("phase_label_style", s("Blank line")), ("k_label_style", s("Blank line")),
            ("k", f(1e6)), ("n0", f(300000.0)), ("r", f(1.862)), ("tspan", f(1.0)), ("fit", b(false)), ("y_max", f(811342.5)), ("x_max", f(1.0)), ("title", s("Growth of yeast"))],
    ]
}

/// Fixed axes with y max below K: random K, N0, r, span, x max, titles, K label text and modes, every combination of
/// the three label modes with a Blank line among the inflection's and the phases'.
fn fixed_axes(seed: u64, n: usize) -> Vec<Case> {
    let mut g = Rng(seed);
    let mut out = checker_cases();
    let combos: Vec<(&str, &str)> = MODES.iter().flat_map(|a| MODES.iter().map(move |c| (*a, *c))).filter(|(a, c)| *a == "Blank line" || *c == "Blank line").collect();
    for i in 0..n {
        let k = g.pick(&[10.0, 100.0, 1000.0, 1e4, 1e5, 1e6]) * g.pick(&[1.0, 2.5, 5.0, 7.3]);
        let k = if k > 1e6 { 1e6 } else { k };
        let n0 = (k * g.pick::<f64>(&[0.001, 0.01, 0.05, 0.13, 0.3, 0.45])).max(1.0);
        let r = g.pick(&[0.01, 0.048, 0.3, 1.0, 1.498, 1.862, 5.0]);
        let tspan = g.pick(&[1.0, 10.0, 20.0, 100.0, 1000.0]);
        let x_max = (tspan * g.pick::<f64>(&[0.25, 0.5, 1.0, 1.5, 3.0])).clamp(1.0, 100_000.0);
        let y_max = k * g.pick(&[0.3, 0.62, 0.75, 0.8, 0.811, 0.84, 0.86, 0.88, 0.9, 0.92, 0.95, 0.97, 0.99]);
        let (im, pm) = combos[i % combos.len()];
        let mut c: Case = vec![("growth", s(g.pick(&["Logistic (S-curve)", "Logistic (S-curve)", "Exponential and logistic"]))),
            ("k", f(k)), ("n0", f(n0)), ("r", f(r)), ("tspan", f(tspan)), ("fit", b(false)), ("x_max", f(x_max)), ("y_max", f(y_max)),
            ("mark_infl", b(g.next(10) != 0)), ("mark_phases", b(g.next(10) != 0)), ("mark_k", b(g.next(8) != 0)),
            ("infl_label_style", s(im)), ("phase_label_style", s(pm)),
            ("k_label_style", s(g.pick(&["Text", "Text", "Blank line", "None"]))),
            ("k_label", s(g.pick(&["", "", "K", "Carrying capacity of the island's deer herd (K)"]))),
            ("title", s(g.pick(&["", "", "Growth of yeast", "Deer on an island, counted every spring since the herd was introduced"]))),
            ("title_style", s(g.pick(&["Text", "Text", "Text", "Blank line"]))),
            ("label_size", s(g.pick(&["Small", "Medium", "Large"]))),
            ("minor", s(g.pick(&["None", "2 per block"]))),
            ("graph", s(if g.next(4) == 0 { g.pick(&["Population size and growth rate", "Population size and per capita rate"]) } else { "Population size over time" })),
        ];
        if g.next(6) == 0 { c.push(("y_end", s("Text"))); }
        if g.next(5) == 0 {
            c.extend([("census_auto", b(true)), ("census_every", f((tspan / g.pick(&[100.0, 10.0])).clamp(0.1, 100.0))), ("census_scatter", s(g.pick(&["None, on the curve", "About 10%"])))]);
        }
        out.push(c);
    }
    out
}

/// Every phase and inflection blank line and leader, read from the drawn SVG: clear of the K label's words (as the
/// renderer draws them) or its blank line, the title's words or blank line, every other word or number, the curves,
/// census dots and each other, and on the page.
#[test]
fn fixed_axes_blank_lines_clear_of_the_k_label_and_title() {
    let d = def(PG);
    let cases = fixed_axes(0xf1_8ed_a5, 480);
    let (mut n_fig, mut n_k_above, mut n_k_blank, mut n_title, mut n_items) = (0, 0, 0, 0, 0);
    let mut bad: Vec<String> = vec![];
    let mut bad_figs = 0;
    for c in &cases {
        let p = params(PG, c);
        let svg = d.svg(&p);
        let blanks = tagged(&svg, "data-blank");
        let leaders = tagged(&svg, "data-leader");
        let mut drawn: Vec<(String, (f64, f64), (f64, f64), bool)> = blanks.iter().filter(|q| q.0 == "infl" || q.0.starts_with("phase-")).map(|q| (q.0.clone(), q.1, q.2, false)).collect();
        drawn.extend(leaders.iter().filter(|q| q.0 == "infl" || q.0.starts_with("phase-")).map(|q| (q.0.clone(), q.1, q.2, true)));
        if drawn.is_empty() { continue; }
        n_fig += 1;
        let ax = super::axes_of(&p, false).into_iter().find(|a| a.kind == 0).unwrap();
        let k = p.f("k").max(1.0);
        let ky = ax.py(k);
        let has_k = p.b("mark_k") && p.s("growth") != "Exponential (J-curve)";
        if has_k && ky < ax.y { n_k_above += 1; }
        let vb: Vec<f64> = attr(&svg, "viewBox").unwrap().split_whitespace().map(|v| v.parse().unwrap()).collect();
        let words = ink_texts(&svg);
        let segs = dark_segments(&svg);
        let curves: Vec<_> = segs.iter().filter(|q| attr(&q.2, "points").is_some_and(|ps| ps.split_whitespace().count() > 50)).collect();
        // the K blank line and the title's blank line, as drawn (untagged plain lines at their known places)
        let flat = |q: &&((f64, f64), (f64, f64), String), x0: f64, x1: f64, y: f64| (q.0 .1 - y).abs() < 0.01 && (q.1 .1 - y).abs() < 0.01 && (q.0 .0 - x0).abs() < 0.01 && (q.1 .0 - x1).abs() < 0.01;
        let mut others: Vec<(String, (f64, f64), (f64, f64))> = vec![];
        if has_k && p.s("k_label_style") == "Blank line" {
            let kb: Vec<_> = segs.iter().filter(|q| flat(q, ax.x + 8.0, ax.x + 170.0, ky + 20.0)).collect();
            assert_eq!(kb.len(), 1, "the K blank line in {c:?}");
            n_k_blank += 1;
            others.push(("the K blank line".into(), kb[0].0, kb[0].1));
        }
        if p.s("title_style") == "Blank line" {
            let tb: Vec<_> = segs.iter().filter(|q| (q.0 .1 - 36.0).abs() < 0.01 && (q.1 .1 - 36.0).abs() < 0.01 && (q.1 .0 - q.0 .0 - 280.0).abs() < 0.01).collect();
            assert_eq!(tb.len(), 1, "the title blank line in {c:?}");
            others.push(("the title blank line".into(), tb[0].0, tb[0].1));
        }
        if p.s("title_style") == "Blank line" || !p.s("title").trim().is_empty() && p.s("title_style") == "Text" { n_title += 1; }
        let dots: Vec<(f64, f64)> = svg.split("<circle").skip(1).filter_map(|el| {
            let g = |k: &str| attr(el, k).and_then(|v| v.parse::<f64>().ok());
            ((g("r")? - 4.2).abs() < 1e-6).then(|| (g("cx").unwrap(), g("cy").unwrap()))
        }).filter(|q| ax.inside((ax.vx(q.0), ax.vy(q.1)))).collect();
        let mut fig_bad = false;
        let mut fail = |m: String| { if bad.len() < 40 { bad.push(m); } fig_bad = true; };
        for (j, (tag, a, e, is_leader)) in drawn.iter().enumerate() {
            n_items += 1;
            let what = format!("{tag} {}", if *is_leader { "leader" } else { "line" });
            if a.0.min(e.0) < 0.0 || a.0.max(e.0) > vb[2] || a.1.min(e.1) < 0.0 || a.1.max(e.1) > vb[3] { fail(format!("{what} off the page in {c:?}")); }
            let clear = if *is_leader { 2.0 } else { 3.0 };
            for cv in &curves { if seg_seg(*a, *e, cv.0, cv.1) < clear { fail(format!("{what} on a curve in {c:?}")); break; } }
            for dot in &dots { if pt_seg(*dot, *a, *e) < 6.7 { fail(format!("{what} on a census dot in {c:?}")); break; } }
            for (t, tb) in &words { if seg_box(*a, *e, *tb, 1.0) { fail(format!("{what} on {t:?} in {c:?}")); } }
            for (t, o0, o1) in &others { if seg_seg(*a, *e, *o0, *o1) < 3.0 { fail(format!("{what} on {t} in {c:?}")); } }
            for (i, o) in drawn.iter().enumerate() {
                if i == j || o.0 == *tag && (*is_leader != o.3) { continue; }
                // two leaders from neighbouring narrow brackets stand side by side at their feet, then part
                let trim = |p: (f64, f64), q: (f64, f64)| { let n = 4.0 / (q.0 - p.0).hypot(q.1 - p.1).max(4.0); (p.0 + (q.0 - p.0) * n, p.1 + (q.1 - p.1) * n) };
                let (a2, o2) = if *is_leader && o.3 { (trim(*a, *e), trim(o.1, o.2)) } else { (*a, o.1) };
                if seg_seg(a2, *e, o2, o.2) < 2.0 { fail(format!("{what} on {}'s {} in {c:?}", o.0, if o.3 { "leader" } else { "line" })); }
            }
        }
        if fig_bad { bad_figs += 1; }
    }
    eprintln!("{n_fig} fixed-axes figures with blank lines ({n_k_above} with K above the plot, {n_k_blank} with K as a blank line, {n_title} with a title): {n_items} lines and leaders; {bad_figs} figures with a touch");
    for m in &bad { eprintln!("  {m}"); }
    assert!(n_fig >= 300 && n_k_above >= 250 && n_k_blank >= 40 && n_title >= 80, "{n_fig} figures, {n_k_above} K above, {n_k_blank} K blank, {n_title} titled");
    assert_eq!(bad_figs, 0, "{bad_figs} figures with a blank line or leader touching something");
}

/// Leaders from neighbouring narrow phase brackets stand apart at their feet (they once left 0.15 to 1.15 px apart and
/// fanned out from one spot): each foot at least 1.95 px from the next (2 px where both brackets are under 4 px: the most their own brackets allow), the checker's figure and the round 3 sweep.
#[test]
fn narrow_bracket_leaders_have_a_gap_at_their_feet() {
    let d = def(PG);
    let mut cases: Vec<Case> = vec![vec![("mark_infl", b(true)), ("mark_phases", b(true)), ("infl_label_style", s("Blank line")), ("phase_label_style", s("Blank line")), ("r", f(5.0)), ("tspan", f(1000.0)), ("n0", f(400.0))]];
    cases.extend(sweep(0x9_0b3, &["Blank line"]));
    let (mut pairs, mut close) = (0, vec![]);
    for (n, c) in cases.iter().enumerate() {
        let svg = d.svg(&params(PG, c));
        let mut feet: Vec<f64> = tagged(&svg, "data-leader").into_iter().filter(|q| q.0.starts_with("phase-")).map(|q| q.1 .0).collect();
        feet.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for w in feet.windows(2) {
            if w[1] - w[0] < 6.0 { pairs += 1; }
            if w[1] - w[0] < 1.95 { close.push(format!("{:.2} px in {c:?}", w[1] - w[0])); }
        }
        if n == 0 { assert_eq!(feet.len(), 2, "the checker's figure has two leaders"); }
    }
    eprintln!("{pairs} pairs of neighbouring feet under 6 px apart; {} under 1.95 px", close.len());
    assert!(pairs >= 20, "{pairs} close pairs");
    assert!(close.is_empty(), "{close:?}");
}
