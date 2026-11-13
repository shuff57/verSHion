//! Population growth, checker round 3: a phase blank line for every phase name Text mode draws (narrow brackets
//! included), no inflection mark or blank line for a point off the graph (Blank line mode; a panel note in every mode),
//! leaders clear of census dots; figures without a Blank line label draw exactly as before.
use super::popdyn_b9_tests::{dark_segments, def, ink_texts, params, s, texts};
use crate::figures::Value;

const PG: &str = "population-growth";
const PHASES: [&str; 3] = ["Lag phase", "Exponential phase", "Stationary phase"];
const MODES: [&str; 3] = ["Text", "Blank line", "None"];
const VIEWS: [&str; 9] = ["Population size over time", "Growth rate over time", "Per capita growth rate over time", "Population size and growth rate",
    "Population size and per capita rate", "Growth rate and per capita rate", "Growth rate against population size", "Per capita rate against population size", "Both rates against population size"];

type Case = Vec<(&'static str, Value)>;
fn f(v: f64) -> Value { Value::F(v) }
fn b(v: bool) -> Value { Value::B(v) }

struct Rng(u64);
impl Rng {
    fn next(&mut self, n: usize) -> usize { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); ((self.0 >> 33) % n as u64) as usize }
    fn pick<T: Clone>(&mut self, v: &[T]) -> T { v[self.next(v.len())].clone() }
}

/// The checker's figures.
fn checker_cases() -> Vec<Case> {
    let lg = || ("growth", s("Logistic (S-curve)"));
    vec![
        vec![("mark_infl", b(true)), ("mark_phases", b(true)), ("phase_label_style", s("Blank line")), ("n0", f(300.0))],
        vec![lg(), ("mark_infl", b(true)), ("mark_phases", b(true)), ("infl_label_style", s("Blank line")), ("phase_label_style", s("Blank line")), ("k_label_style", s("Text")), ("k", f(100000.0)), ("n0", f(5000.0)), ("r", f(1.498)), ("tspan", f(1000.0))],
        vec![lg(), ("mark_infl", b(true)), ("infl_label_style", s("Blank line")), ("k", f(1000000.0)), ("n0", f(450000.0)), ("r", f(0.01)), ("tspan", f(10.0))],
        vec![lg(), ("mark_infl", b(true)), ("mark_phases", b(true)), ("infl_label_style", s("Blank line")), ("k", f(1000.0)), ("n0", f(300.0)), ("r", f(0.048)), ("tspan", f(1000.0)), ("minor", s("2 per block")), ("census_auto", b(true)), ("census_every", f(1.0)), ("census_scatter", s("About 20%"))],
    ]
}

/// Logistic, both and exponential growth over K 10 to 1e6, r 0.01 to 5, N0 0.1% to 45% of K and spans 1 to 1000, each
/// with a seeded graph view, label size, gridlines, census counts and the three label modes; then the classic setups at
/// every view and label size. `modes`: the label modes the inflection point and the phases may take.
fn sweep(seed: u64, modes: &[&'static str]) -> Vec<Case> {
    let mut g = Rng(seed);
    let mut out = vec![];
    let mut dress = |c: &mut Case, g: &mut Rng, tspan: f64| {
        c.extend([
            ("graph", s(if g.next(3) == 0 { "Population size over time" } else { g.pick(&VIEWS) })),
            ("label_size", s(g.pick(&["Small", "Medium", "Large"]))),
            ("minor", s(g.pick(&["None", "2 per block", "4 per block", "5 per block"]))),
            ("k_label_style", s(g.pick(&MODES))),
            ("infl_label_style", s(g.pick(modes))),
            ("phase_label_style", s(g.pick(modes))),
            ("mark_infl", b(g.next(8) != 0)),
            ("mark_phases", b(g.next(8) != 0)),
            ("mark_k", b(g.next(5) != 0)),
        ]);
        if g.next(3) == 0 {
            c.extend([("census_auto", b(true)), ("census_every", f((tspan / g.pick(&[1000.0, 100.0, 10.0])).clamp(0.1, 100.0))),
                ("census_scatter", s(g.pick(&["None, on the curve", "About 5%", "About 10%", "About 20%"])))]);
        }
    };
    for growth in ["Logistic (S-curve)", "Exponential and logistic", "Exponential (J-curve)"] {
        for k in [10.0, 100.0, 1000.0, 1e4, 1e5, 1e6] {
            for r in [0.01, 0.048, 0.3, 1.0, 1.498, 5.0] {
                for fr in [0.001, 0.01, 0.05, 0.13, 0.3, 0.45] {
                    for tspan in [1.0, 10.0, 100.0, 1000.0] {
                        let mut c: Case = vec![("growth", s(growth)), ("k", f(k)), ("r", f(r)), ("n0", f((k * fr).max(1.0))), ("tspan", f(tspan))];
                        dress(&mut c, &mut g, tspan);
                        out.push(c);
                    }
                }
            }
        }
    }
    for e in crate::figures::classic_setups(PG) {
        for view in VIEWS {
            for ls in ["Small", "Medium", "Large"] {
                let mut c = e.params.clone();
                let tspan = c.iter().find(|q| q.0 == "tspan").and_then(|q| if let Value::F(v) = q.1 { Some(v) } else { None }).unwrap_or(20.0);
                dress(&mut c, &mut g, tspan);
                c.retain(|q| q.0 != "graph" && q.0 != "label_size");
                c.extend([("graph", s(view)), ("label_size", s(ls))]);
                out.push(c);
            }
        }
    }
    out
}

fn attr<'a>(el: &'a str, k: &str) -> Option<&'a str> { Some(el.split(&format!(" {k}=\"")).nth(1)?.split('"').next()?) }
fn tagged(svg: &str, what: &str) -> Vec<(String, (f64, f64), (f64, f64))> {
    svg.split("<line").skip(1).filter_map(|el| {
        let el = el.split('>').next().unwrap();
        let t = attr(el, what)?.to_string();
        let g = |k| attr(el, k).unwrap().parse::<f64>().unwrap();
        Some((t, (g("x1"), g("y1")), (g("x2"), g("y2"))))
    }).collect()
}
fn pt_seg(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = (dx * dx + dy * dy).max(1e-12);
    let u = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    ((p.0 - a.0 - dx * u).powi(2) + (p.1 - a.1 - dy * u).powi(2)).sqrt()
}
fn seg_seg(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> f64 {
    let n = (((b.0 - a.0).hypot(b.1 - a.1)).ceil() as usize).clamp(4, 2000);
    (0..=n).map(|i| { let t = i as f64 / n as f64; pt_seg((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t), c, d) }).fold(f64::MAX, f64::min)
        .min(pt_seg(c, a, b)).min(pt_seg(d, a, b))
}
fn seg_box(a: (f64, f64), b: (f64, f64), bx: (f64, f64, f64, f64), pad: f64) -> bool {
    let n = (((b.0 - a.0).hypot(b.1 - a.1)).ceil() as usize).clamp(4, 2000);
    (0..=n).any(|i| { let t = i as f64 / n as f64; let q = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t); q.0 > bx.0 - pad && q.0 < bx.2 + pad && q.1 > bx.1 - pad && q.1 < bx.3 + pad })
}

/// Every phase bracket drawn above the population graph (a four-point polyline with two 6 px feet), however narrow:
/// (left, right, bracket y, middle), left to right by middle. The middle is the bracket's own, kept inside the plot.
fn brackets(svg: &str, p: &crate::figures::Params) -> Vec<(f64, f64, f64, f64)> {
    let Some(ax) = super::axes_of(p, false).into_iter().find(|a| a.kind == 0) else { return vec![] };
    let mut out = vec![];
    for el in svg.split("<polyline").skip(1) {
        let el = el.split('>').next().unwrap();
        let pts: Vec<(f64, f64)> = attr(el, "points").unwrap().split_whitespace().map(|q| { let (a, b) = q.split_once(',').unwrap(); (a.parse().unwrap(), b.parse().unwrap()) }).collect();
        if pts.len() == 4 && pts[0].0 == pts[1].0 && pts[2].0 == pts[3].0 && pts[1].1 == pts[2].1 && (pts[0].1 - pts[1].1 - 6.0).abs() < 1e-6 && pts[0].1 < ax.y {
            let (l, r) = (pts[1].0.min(pts[2].0), pts[1].0.max(pts[2].0));
            out.push((l, r, pts[1].1, ((pts[1].0 + pts[2].0) / 2.0).clamp(ax.x, ax.x + ax.w)));
        }
    }
    out.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap());
    out
}

/// The phase names Text mode draws.
fn names_drawn(svg: &str) -> usize { texts(svg).iter().filter(|t| PHASES.contains(&t.as_str())).count() }

fn with(c: &Case, extra: &[(&'static str, Value)]) -> Case { let mut v = c.clone(); v.retain(|q| !extra.iter().any(|e| e.0 == q.0)); v.extend(extra.iter().cloned()); v }

/// Wherever Text mode draws a phase's name, Blank line mode draws that phase's blank line (same count), each its own
/// bracket's: over it and nearer it than any other, or with a leader standing on it; no blank line or leader on a
/// curve, the K line, a word or number, another blank line or leader, or a census dot; inside the page, leaders that
/// run into the plot inside it. A phase Text mode leaves unlabelled is named in the same panel note in both modes.
#[test]
fn phase_blanks_match_the_names_text_draws() {
    let d = def(PG);
    let mut cases = checker_cases();
    cases.extend(sweep(0x9_0b3, &MODES));
    let (mut n_fig, mut n_blank, mut n_narrow, mut n_lead, mut n_noted) = (0, 0, 0, 0, 0);
    for c in cases {
        let pb = params(PG, &with(&c, &[("phase_label_style", s("Blank line"))]));
        let pt = params(PG, &with(&c, &[("phase_label_style", s("Text"))]));
        let (bs, ts) = (d.svg(&pb), d.svg(&pt));
        let blanks = tagged(&bs, "data-blank");
        let leaders = tagged(&bs, "data-leader");
        let ph: Vec<_> = blanks.iter().filter(|q| q.0.starts_with("phase-")).cloned().collect();
        assert_eq!(ph.len(), names_drawn(&ts), "phase blank lines and Text mode's names in {c:?}");
        // the same panel note in both modes
        assert_eq!(crate::figures::notes_for(PG, &pb), crate::figures::notes_for(PG, &pt), "notes in {c:?}");
        let notes = crate::figures::notes_for(PG, &pb);
        let br = brackets(&bs, &pb);
        assert_eq!(br.len(), names_drawn(&ts), "a name for every bracket in {c:?}");
        if super::growth_b9::phases_drawn(&pb) {
            let unlabelled = 3 - br.len();
            assert_eq!(notes.iter().filter(|n| n.contains("phase") && n.contains("no bracket")).count(), (unlabelled > 0) as usize, "the note on {unlabelled} phases without a bracket in {c:?}: {notes:?}");
            if unlabelled > 0 { n_noted += 1; }
        }
        if ph.is_empty() { continue; }
        n_fig += 1;
        let ax = super::axes_of(&pb, false).into_iter().find(|a| a.kind == 0).unwrap();
        let vb: Vec<f64> = attr(&bs, "viewBox").unwrap().split_whitespace().map(|v| v.parse().unwrap()).collect();
        let words = ink_texts(&bs);
        let segs = dark_segments(&bs);
        let curves: Vec<_> = segs.iter().filter(|q| attr(&q.2, "points").is_some_and(|ps| ps.split_whitespace().count() > 50)).collect();
        let kline = (pb.b("mark_k") && pb.s("growth") != "Exponential (J-curve)").then(|| ax.py(pb.f("k").max(1.0))).filter(|y| *y >= ax.y && *y <= ax.y + ax.h);
        let dots: Vec<(f64, f64)> = bs.split("<circle").skip(1).filter_map(|el| {
            let g = |k: &str| attr(el, k).and_then(|v| v.parse::<f64>().ok());
            ((g("r")? - 4.2).abs() < 1e-6).then(|| (g("cx").unwrap(), g("cy").unwrap()))
        }).filter(|q| ax.inside((ax.vx(q.0), ax.vy(q.1)))).collect();
        // every line and leader drawn for a label, with its tag
        let mut drawn: Vec<(String, (f64, f64), (f64, f64), bool)> = blanks.iter().map(|q| (q.0.clone(), q.1, q.2, false)).collect();
        drawn.extend(leaders.iter().map(|q| (q.0.clone(), q.1, q.2, true)));
        for (k, (tag, a, e, is_leader)) in drawn.iter().enumerate() {
            if !tag.starts_with("phase-") { continue; }
            let (x0, x1) = (a.0.min(e.0), a.0.max(e.0));
            assert!(x0 >= 0.0 && x1 <= vb[2] && a.1.min(e.1) >= 0.0 && a.1.max(e.1) <= vb[3], "{tag} off the page in {c:?}");
            let clear = if *is_leader { 2.0 } else { 3.0 };
            for cv in &curves { assert!(seg_seg(*a, *e, cv.0, cv.1) >= clear, "{tag} on a curve in {c:?}"); }
            if let Some(ky) = kline { assert!(seg_seg(*a, *e, (ax.x, ky), (ax.x + ax.w, ky)) >= clear, "{tag} on the K line in {c:?}"); }
            for dot in &dots { assert!(pt_seg(*dot, *a, *e) >= 6.7, "{tag} on a census dot in {c:?}"); }
            for (t, tb) in &words { assert!(!seg_box(*a, *e, *tb, 1.0), "{tag} on {t:?} in {c:?}"); }
            for (j, o) in drawn.iter().enumerate() {
                if j == k || o.0 == *tag && (*is_leader != o.3) { continue; }
                // two leaders from neighbouring narrow brackets stand side by side at their feet, then part
                let trim = |p: (f64, f64), q: (f64, f64)| { let n = 4.0 / (q.0 - p.0).hypot(q.1 - p.1).max(4.0); (p.0 + (q.0 - p.0) * n, p.1 + (q.1 - p.1) * n) };
                let (a2, o2) = if *is_leader && o.3 { (trim(*a, *e), trim(o.1, o.2)) } else { (*a, o.1) };
                assert!(seg_seg(a2, *e, o2, o.2) >= 2.0, "{tag} on {}'s {} in {c:?}: {drawn:?} brackets {br:?}", o.0, if o.3 { "leader" } else { "line" });
            }
        }
        for (tag, a, e) in &ph {
            n_blank += 1;
            let i: usize = tag.trim_start_matches("phase-").parse().unwrap();
            let present: Vec<usize> = ph.iter().map(|q| q.0.trim_start_matches("phase-").parse().unwrap()).collect::<std::collections::BTreeSet<usize>>().into_iter().collect();
            let own = br[present.iter().position(|v| *v == i).unwrap()];
            if own.1 - own.0 < 4.0 + 1e-9 { n_narrow += 1; }
            if let Some(l) = leaders.iter().find(|l| l.0 == *tag) {
                n_lead += 1;
                // the leader's foot on its own bracket, nearer its middle than any other bracket's
                assert!((l.1 .1 - own.2).abs() <= 3.0 && l.1 .0 >= own.0.min(own.3) - 0.5 && l.1 .0 <= own.1.max(own.3) + 0.5, "{tag}: the leader stands on its own bracket in {c:?}");
                for o in &br { if o.3 != own.3 { assert!((l.1 .0 - own.3).abs() < (l.1 .0 - o.3).abs(), "{tag}: the leader stands nearer another bracket in {c:?}"); } }
                assert!(pt_seg(l.2, *a, *e) <= 4.0, "{tag}: the leader reaches its line in {c:?}");
            } else {
                let mid = (a.0 + e.0) / 2.0;
                assert!(mid >= own.0 && mid <= own.1, "{tag}: over its own bracket in {c:?}");
                let dist = |q: &(f64, f64, f64, f64)| seg_seg(*a, *e, (q.0, q.2), (q.1, q.2));
                for o in &br { if o.3 != own.3 { assert!(dist(&own) < dist(o), "{tag}: nearer another bracket in {c:?}"); } }
            }
        }
    }
    assert!(n_fig >= 600 && n_narrow >= 50, "{n_fig} figures, {n_blank} phase lines, {n_narrow} on narrow brackets, {n_lead} leaders");
    eprintln!("{n_fig} figures with phase blank lines: {n_blank} lines, {n_narrow} on brackets under 4 px, {n_lead} with leaders; {n_noted} with a phase noted as having no bracket");
}

/// A point past the span shown (or outside fixed axes): Blank line mode draws neither the mark nor its blank line; a
/// panel note says where the point is, in every mode, and only then.
#[test]
fn inflection_off_the_graph_is_not_marked() {
    let d = def(PG);
    let mut cases = checker_cases();
    cases.extend(sweep(0x0ff_9a, &MODES));
    let (mut off, mut on) = (0, 0);
    for c in cases {
        let p0 = params(PG, &c);
        if !super::growth_b9::infl_labelled(&p0) { continue; }
        let ax = super::axes_of(&p0, false).into_iter().find(|a| a.kind == 0).unwrap();
        let ti = super::growth_b9::t_inf(&p0).unwrap();
        let inside = ax.inside((ti, p0.f("k").max(1.0) / 2.0));
        for m in MODES {
            let p = params(PG, &with(&c, &[("infl_label_style", s(m))]));
            let notes = crate::figures::notes_for(PG, &p);
            let noted = notes.iter().filter(|n| n.contains("inflection point") && n.contains("outside the graph")).count();
            assert_eq!(noted, (!inside) as usize, "{m}: the note in {c:?}: {notes:?}");
            if m != "Blank line" { continue; }
            let svg = d.svg(&p);
            let blank = svg.matches("data-blank=\"infl\"").count();
            // the point's circle and its dashed guides, drawn in the population graph's clip group (the first)
            let g0 = svg.split(r#"<g clip-path="url(#clip0)">"#).nth(1).unwrap().split("</g>").next().unwrap();
            let circles = g0.matches(r##"r="5.50" fill="#fff""##).count();
            let guides = g0.matches(r#"stroke-dasharray="5 4""#).count();
            if inside {
                on += 1;
                assert_eq!(blank, 1, "the point's blank line in {c:?}");
                assert_eq!((circles, guides), (1, 1), "the point and its guides in {c:?}");
            } else {
                off += 1;
                assert_eq!(blank, 0, "a blank line for a point off the graph in {c:?}");
                assert_eq!(svg.matches("data-leader=\"infl\"").count(), 0);
                assert_eq!((circles, guides), (0, 0), "no mark for a point off the graph in {c:?}");
            }
        }
    }
    assert!(off >= 20 && on >= 300, "{off} off the graph, {on} on it");
}

/// The point's leader keeps clear of census dots (6.7 px from a dot's centre: 1.5 px from its edge) wherever a clear
/// route exists: the checker's graph with 400 counts, and the sweep.
#[test]
fn infl_leader_clear_of_census_dots() {
    let d = def(PG);
    let mut cases = checker_cases();
    cases.extend(sweep(0xd07, &MODES).into_iter().filter(|c| c.iter().any(|q| q.0 == "census_auto")));
    let (mut n, mut led, mut boxed) = (0, 0, 0);
    for c in cases {
        let p = params(PG, &with(&c, &[("infl_label_style", s("Blank line"))]));
        let svg = d.svg(&p);
        let dots: Vec<(f64, f64)> = svg.split("<circle").skip(1).filter_map(|el| {
            let g = |k: &str| attr(el, k).and_then(|v| v.parse::<f64>().ok());
            ((g("r")? - 4.2).abs() < 1e-6).then(|| (g("cx").unwrap(), g("cy").unwrap()))
        }).collect();
        if dots.is_empty() { continue; }
        n += 1;
        // a clear route: some straight way out from the point (9 to 60 px along it) keeping 7 px from every count; where
        // the counts crowd round the point on all sides, no straight leader can keep off them and only the line's own
        // clearance is checked (counted)
        let pt = super::growth_b9::t_inf(&p).and_then(|ti| super::axes_of(&p, false).into_iter().find(|a| a.kind == 0).map(|ax| (ax.px(ti), ax.py(p.f("k").max(1.0) / 2.0))));
        let ring_free = pt.is_some_and(|pt| (0..144).any(|j| {
            let a = j as f64 * std::f64::consts::TAU / 144.0;
            dots.iter().all(|d| pt_seg(*d, (pt.0 + 9.0 * a.cos(), pt.1 + 9.0 * a.sin()), (pt.0 + 60.0 * a.cos(), pt.1 + 60.0 * a.sin())) >= 7.0)
        }));
        if !ring_free { boxed += 1; }
        // the checker's graph of 400 counts has a way out: its leader is checked
        if format!("{c:?}") == format!("{:?}", checker_cases()[3]) { assert!(ring_free); }
        for (tag, a, e) in tagged(&svg, "data-leader").into_iter().filter(|_| ring_free).chain(tagged(&svg, "data-blank")) {
            if tag != "infl" { continue; }
            led += 1;
            for dot in &dots { assert!(pt_seg(*dot, a, e) >= 6.7, "{tag} {:.1} px from a census dot's centre {dot:?} line {a:?} {e:?} in {c:?}", pt_seg(*dot, a, e)); }
        }
    }
    assert!(n >= 100, "{n} figures with counts ({led} lines and leaders)");
    eprintln!("{n} figures with counts: {led} lines and leaders checked; {boxed} with counts crowding every spot round the point");
}

// ------------------------------------------------------------------ no Blank line label: drawn exactly as before

fn fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/popdyn_r3_no_blank.txt") }
fn enc(c: &Case) -> String {
    c.iter().map(|(k, v)| match v { Value::F(x) => format!("{k}=F{x:?}"), Value::I(x) => format!("{k}=I{x}"), Value::B(x) => format!("{k}=B{x}"), Value::S(x) => format!("{k}=S{x}") }).collect::<Vec<_>>().join("|")
}
fn dec(line: &str) -> Vec<(String, Value)> {
    line.split('|').map(|kv| {
        let (k, v) = kv.split_once('=').unwrap();
        let (t, r) = v.split_at(1);
        (k.to_string(), match t { "F" => Value::F(r.parse().unwrap()), "I" => Value::I(r.parse().unwrap()), "B" => Value::B(r == "true"), _ => Value::S(r.to_string()) })
    }).collect()
}

/// A seeded set of figures whose inflection and phase labels are Text or None (any K label), and the checker's, hashed
/// from the code before this round (written once to tests/fixtures/popdyn_r3_no_blank.txt); each draws the same bytes.
#[test]
fn figures_with_no_blank_line_label_draw_as_before() {
    let d = def(PG);
    if !fixture().exists() {
        let mut cs = sweep(0x5eed_3, &["Text", "None"]);
        for c in checker_cases() {
            for m in ["Text", "None"] { cs.push(with(&c, &[("infl_label_style", s(m)), ("phase_label_style", s(m))])); }
        }
        let text: String = cs.iter().map(|c| format!("{}\t{}\n", crate::figures::versions_b9_frozen::fnv(&d.svg(&params(PG, c))), enc(c))).collect();
        std::fs::write(fixture(), text).unwrap();
    }
    let text = std::fs::read_to_string(fixture()).unwrap();
    let mut n = 0;
    for line in text.lines() {
        let (h, spec) = line.split_once('\t').unwrap();
        let c = dec(spec);
        let over: Vec<(&str, Value)> = c.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        assert_eq!(crate::figures::versions_b9_frozen::fnv(&d.svg(&params(PG, &over))), h, "{spec}");
        n += 1;
    }
    assert!(n >= 2800, "{n} figures");
}
