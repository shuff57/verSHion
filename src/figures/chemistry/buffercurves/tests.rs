//! Tests of the titration and buffer curves. The pH is checked against an independent solver (bisection on the charge
//! balance written with species fractions, pH as the unknown), not against the solver that drew it. The label audit
//! measures every text with the glyphs the PNG renderer draws (usvg's own layout of the bundled and system faces).
use super::*;
use crate::figures::{registry, to_png};
use std::collections::{BTreeMap, HashMap};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("buffer-curves is registered") }
fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
/// Like `with`, but never normalised: render must survive what a hostile link or an old file holds.
fn raw(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p
}
type L = Vec<(&'static str, Value)>;
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
fn svg_of(list: &[(&'static str, Value)]) -> String { def().svg(&with(list)) }
fn mdl(list: &[(&'static str, Value)]) -> Model { model_of(&with(list)) }
fn show(l: &L) -> String { format!("{l:?}") }

const SASB: &str = "Strong acid titrated with a strong base";
const SBSA: &str = "Strong base titrated with a strong acid";
const WASB: &str = "Weak acid titrated with a strong base";
const WBSA: &str = "Weak base titrated with a strong acid";
const BUF: &str = "Buffer: pH as acid or base is added";
const T_EQ: &str = "Read the equivalence point";
const T_PKA: &str = "Find pKa from the half-equivalence point";
const T_IND: &str = "Choose an indicator";
const T_PH: &str = "Find the pH at a given volume";
const T_BUF: &str = "Buffer pH after adding acid or base";
const MODES_ALL: [&str; 5] = [SASB, SBSA, WASB, WBSA, BUF];
const TASKS_ALL: [&str; 5] = [T_EQ, T_PKA, T_IND, T_PH, T_BUF];

// ---------------------------------------------------------------------------------------------
// Reading the SVG back
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct T { x: f64, y: f64, fs: f64, anchor: String, text: String, kind: String, feat: String, group: String, rot: Option<(f64, f64)> }

fn attr(tag: &str, k: &str) -> Option<String> {
    let pat = format!(" {k}=\"");
    let at = tag.find(&pat)? + pat.len();
    tag[at..].split('"').next().map(|v| v.to_string())
}
fn attr_f(tag: &str, k: &str) -> Option<f64> { attr(tag, k)?.parse().ok() }
fn unesc(s: &str) -> String { s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&") }

fn texts(svg: &str) -> Vec<T> {
    let mut out = Vec::new();
    for part in svg.split("<text").skip(1) {
        let Some((tag, rest)) = part.split_once('>') else { continue };
        let text = unesc(rest.split("</text>").next().unwrap_or(""));
        let rot = attr(tag, "transform").and_then(|t| {
            let inner = t.strip_prefix("rotate(-90 ")?.strip_suffix(')')?;
            let (a, b) = inner.split_once(' ')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        });
        out.push(T {
            x: attr_f(tag, "x").unwrap_or(0.0),
            y: attr_f(tag, "y").unwrap_or(0.0),
            fs: attr_f(tag, "font-size").unwrap_or(12.0),
            anchor: attr(tag, "text-anchor").unwrap_or_else(|| "start".into()),
            text,
            kind: attr(tag, "data-o").unwrap_or_default(),
            feat: attr(tag, "data-f").unwrap_or_default(),
            group: attr(tag, "data-g").unwrap_or_default(),
            rot,
        });
    }
    out
}

fn all_text(svg: &str) -> String { texts(svg).into_iter().map(|t| t.text).collect::<Vec<_>>().join("\n") }
fn note_text(svg: &str) -> String { texts(svg).into_iter().filter(|t| t.kind == "note").map(|t| t.text).collect::<Vec<_>>().join(" ") }
fn title_of(svg: &str) -> String { svg.split("<title>").nth(1).and_then(|x| x.split("</title>").next()).map(unesc).unwrap_or_default() }
fn desc_of(svg: &str) -> String { svg.split("<desc>").nth(1).and_then(|x| x.split("</desc>").next()).map(unesc).unwrap_or_default() }

fn path_pts(d: &str) -> Vec<Pt> {
    d.split_whitespace()
        .filter_map(|tok| {
            let t = tok.trim_start_matches(|c: char| c.is_ascii_alphabetic());
            let (a, b) = t.split_once(',')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .collect()
}

type Bx = (f64, f64, f64, f64);
type Seg = (Pt, Pt);
/// The strokes (kind, feature, segment), markers (feature, box, element) and whole paths (kind, feature, points, style).
struct Geo { strokes: Vec<(String, String, Seg)>, marks: Vec<(String, Bx, String)>, paths: Vec<(String, String, Vec<Pt>, String)>, bands: Vec<String> }

fn geo(svg: &str) -> Geo {
    let mut g = Geo { strokes: Vec::new(), marks: Vec::new(), paths: Vec::new(), bands: Vec::new() };
    for part in svg.split('<').skip(1) {
        let tag = part.split('>').next().unwrap_or("");
        let kind = attr(tag, "data-o").unwrap_or_default();
        let feat = attr(tag, "data-f").unwrap_or_default();
        if kind == "band" { g.bands.push(feat.clone()); }
        if tag.starts_with("line ") {
            let (x1, y1, x2, y2) = (attr_f(tag, "x1").unwrap(), attr_f(tag, "y1").unwrap(), attr_f(tag, "x2").unwrap(), attr_f(tag, "y2").unwrap());
            g.strokes.push((kind, feat, ((x1, y1), (x2, y2))));
        } else if tag.starts_with("path ") {
            let pts = path_pts(&attr(tag, "d").unwrap());
            for w in pts.windows(2) { g.strokes.push((kind.clone(), feat.clone(), (w[0], w[1]))); }
            let style = format!("{}|{}|{}", attr(tag, "stroke").unwrap_or_default(), attr(tag, "stroke-width").unwrap_or_default(), attr(tag, "stroke-dasharray").unwrap_or_default());
            g.paths.push((kind, feat, pts, style));
        } else if kind == "mark" && tag.starts_with("circle ") {
            let (cx, cy, r) = (attr_f(tag, "cx").unwrap(), attr_f(tag, "cy").unwrap(), attr_f(tag, "r").unwrap());
            g.marks.push((feat, (cx - r, cx + r, cy - r, cy + r), "circle".into()));
        } else if kind == "mark" && tag.starts_with("rect ") {
            let (x, y, w, h) = (attr_f(tag, "x").unwrap(), attr_f(tag, "y").unwrap(), attr_f(tag, "width").unwrap(), attr_f(tag, "height").unwrap());
            g.marks.push((feat, (x, x + w, y, y + h), "rect".into()));
        } else if kind == "mark" && tag.starts_with("polygon ") {
            let pts = path_pts(&format!("M{}", attr(tag, "points").unwrap().replace(' ', " L")));
            let xs = pts.iter().map(|p| p.0);
            let ys = pts.iter().map(|p| p.1);
            g.marks.push((feat, (xs.clone().fold(f64::MAX, f64::min), xs.fold(f64::MIN, f64::max), ys.clone().fold(f64::MAX, f64::min), ys.fold(f64::MIN, f64::max)), format!("polygon{}", pts.len())));
        }
    }
    g
}

fn curve_of(svg: &str) -> Option<Vec<Pt>> { geo(svg).paths.into_iter().find(|p| p.0 == "curve").map(|p| p.2) }

// ---------------------------------------------------------------------------------------------
// An independent solver: bisection on pH, with the species as fractions
// ---------------------------------------------------------------------------------------------

/// The pH after `v` mL, by 200 bisections on pH in [−3, 17] of the charge balance written as the positive charge
/// less the negative charge, each weak species from its fraction 1/(1 + 10^(pKa − pH)). Shares no code with the figure.
fn ind_ph(sys: &Sys, pka: f64, v: f64) -> f64 {
    let vt = sys.v_a + v;
    let (mut na, mut cl, mut cw) = (0.0, 0.0, 0.0);
    match sys.mode {
        Mode::SaSb => { na = sys.c_t * v / vt; cl = sys.c_a * sys.v_a / vt; }
        Mode::SbSa => { na = sys.c_a * sys.v_a / vt; cl = sys.c_t * v / vt; }
        Mode::WaSb => { na = sys.c_t * v / vt; cw = sys.c_a * sys.v_a / vt; }
        Mode::WbSa => { cl = sys.c_t * v / vt; cw = sys.c_a * sys.v_a / vt; }
        Mode::Buffer => {
            cw = (sys.c_a + sys.c_s) * sys.v_a / vt;
            if sys.base_pair { cl += sys.c_s * sys.v_a / vt } else { na += sys.c_s * sys.v_a / vt }
            if sys.add_base { na += sys.c_t * v / vt } else { cl += sys.c_t * v / vt }
        }
    }
    let bh = sys.mode == Mode::WbSa || (sys.mode == Mode::Buffer && sys.base_pair);
    let excess = |ph: f64| {
        let base_frac = 1.0 / (1.0 + 10f64.powf(pka - ph));
        let pos = 10f64.powf(-ph) + na + if bh { cw * (1.0 - base_frac) } else { 0.0 };
        let neg = 10f64.powf(ph - 14.0) + cl + if bh { 0.0 } else { cw * base_frac };
        pos - neg
    };
    let (mut lo, mut hi) = (-3.0f64, 17.0f64);
    for _ in 0..200 { let mid = 0.5 * (lo + hi); if excess(mid) > 0.0 { lo = mid } else { hi = mid } }
    0.5 * (lo + hi)
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); self.0 >> 33 }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T { v[(self.next() as usize) % v.len()] }
    fn unit(&mut self) -> f64 { (self.next() % 1_000_000) as f64 / 1_000_000.0 }
    fn logu(&mut self, lo: f64, hi: f64) -> f64 { (lo.ln() + (hi.ln() - lo.ln()) * self.unit()).exp() }
}

fn rand_sys(r: &mut Rng) -> (Sys, f64) {
    let mode = r.pick(&[Mode::SaSb, Mode::SbSa, Mode::WaSb, Mode::WbSa, Mode::Buffer]);
    let pka = 1.0 + 12.0 * r.unit();
    let base_pair = match mode { Mode::WbSa => true, Mode::Buffer => r.next() % 2 == 0, _ => false };
    let sys = Sys {
        mode,
        c_a: r.logu(1e-4, 2.0),
        v_a: 1.0 + 249.0 * r.unit(),
        c_t: r.logu(1e-4, 2.0),
        c_s: if mode == Mode::Buffer { if r.next() % 5 == 0 { 0.0 } else { r.logu(1e-4, 2.0) } } else { 0.0 },
        ka: if mode.weak() { 10f64.powf(-pka) } else { 1.0 },
        base_pair,
        add_base: r.next() % 2 == 0,
    };
    (sys, pka)
}

// ---------------------------------------------------------------------------------------------
// The chemistry is exact
// ---------------------------------------------------------------------------------------------

#[test]
fn the_ph_agrees_with_an_independent_bisection_at_thousands_of_points() {
    let mut r = Rng(11);
    let mut n = 0;
    let mut worst = 0.0f64;
    for _ in 0..600 {
        let (sys, pka) = rand_sys(&mut r);
        let veq = sys.v_eq();
        let span = if veq > 0.0 { 3.0 * veq } else { 50.0 };
        let mut vs = vec![0.0, veq, 0.5 * veq, veq * (1.0 - 1e-6), veq * (1.0 + 1e-6)];
        for _ in 0..5 { vs.push(span * r.unit()); }
        for v in vs {
            let got = sys.ph(v);
            let want = ind_ph(&sys, pka, v);
            assert!(got.is_finite(), "{sys:?} at {v}");
            worst = worst.max((got - want).abs());
            assert!((got - want).abs() < 1e-7, "pH {got} vs independent {want} for {sys:?} at {v} mL");
            n += 1;
        }
    }
    assert!(n >= 6000, "{n}");
    assert!(worst < 1e-7, "{worst}");
}

#[test]
fn textbook_values_come_out() {
    // 25.00 mL of 0.1000 M acetic acid (pKa 4.756) with 0.1000 M NaOH
    let wa = |v: f64| fx(mdl(&[s("mode", WASB), s("acid", "Acetic acid")]).st.sys.ph(v), 2);
    assert_eq!(wa(0.0), "2.88");
    assert_eq!(wa(10.0), "4.58");
    assert_eq!(wa(12.5), "4.76");
    assert_eq!(wa(25.0), "8.73");
    assert_eq!(wa(30.0), "11.96");
    // 0.1 M HCl with 0.1 M NaOH
    let sa = |v: f64| fx(mdl(&[s("mode", SASB)]).st.sys.ph(v), 2);
    assert_eq!(sa(0.0), "1.00");
    assert_eq!(sa(24.9), "3.70");
    assert_eq!(sa(25.0), "7.00");
    assert_eq!(sa(25.1), "10.30");
    // 0.1 M ammonia (pKa of NH4+ 9.25) with 0.1 M HCl
    let wb = |v: f64| fx(mdl(&[s("mode", WBSA)]).st.sys.ph(v), 2);
    assert_eq!(wb(0.0), "11.12");
    assert_eq!(wb(12.5), "9.25");
    assert_eq!(wb(25.0), "5.28");
    // the most dilute strong acid the figure takes: 1e-4 M HCl is pH 4.00
    let d = mdl(&[s("mode", SASB), f("c_an", 0.0001), f("v_an", 1.0), f("c_ti", 2.0)]);
    let ph = d.st.sys.ph(0.0);
    assert_eq!(fx(ph, 2), "4.00");
    // a weak acid so dilute that the square root would put the pH above 7 is held just below it
    let w = Sys { mode: Mode::WaSb, c_a: 1e-9, v_a: 25.0, c_t: 0.1, c_s: 0.0, ka: 10f64.powf(-9.21), base_pair: false, add_base: true };
    assert!(w.ph(0.0) < 7.0 && w.ph(0.0) > 6.99, "{}", w.ph(0.0));
}

#[test]
fn the_key_points_are_the_exact_ones() {
    // the equivalence volume is where the independent curve is steepest (within 0.2 %), for titrations whose jump is
    // sharp (strong ones from 0.01 M; weak ones from 0.05 M with pKa (or pKb) up to 6)
    let mut r = Rng(5);
    let mut checked = 0;
    for _ in 0..400 {
        let (sys, pka) = rand_sys(&mut r);
        if !sys.mode.titration() || sys.c_a < 0.01 || sys.c_t < 0.01 { continue; }
        if sys.mode.weak() && (sys.c_a < 0.05 || sys.c_t < 0.05) { continue; }
        if (sys.mode == Mode::WaSb && pka > 6.0) || (sys.mode == Mode::WbSa && pka < 8.0) { continue; }
        let veq = model_of(&with(&[s("mode", match sys.mode { Mode::SaSb => SASB, Mode::SbSa => SBSA, Mode::WaSb => WASB, _ => WBSA }), s("acid", "Custom"), f("pka", sig4(pka)), s("base", "Custom"), f("pkb", sig4(PKW - pka)), f("c_an", sig4(sys.c_a)), f("v_an", sig4(sys.v_a)), f("c_ti", sig4(sys.c_t))])).veq;
        let sys2 = Sys { c_a: sig4(sys.c_a), v_a: sig4(sys.v_a), c_t: sig4(sys.c_t), ..sys };
        let n = 4000;
        let (mut best_v, mut best_s) = (0.0, 0.0);
        for k in 0..n {
            let (a, c) = (veq * (0.9 + 0.2 * k as f64 / n as f64), veq * (0.9 + 0.2 * (k + 1) as f64 / n as f64));
            let slope = (ind_ph(&sys2, sig4(pka), c) - ind_ph(&sys2, sig4(pka), a)).abs() / (c - a);
            if slope > best_s { best_s = slope; best_v = 0.5 * (a + c); }
        }
        assert!((best_v - veq).abs() <= 0.002 * veq, "steepest at {best_v}, equivalence {veq} for {sys2:?}");
        checked += 1;
    }
    assert!(checked > 25, "{checked}");
    let m = mdl(&[s("mode", WASB), s("task", T_PKA)]);
    assert!((m.veq - 25.0).abs() < 1e-9 && (m.vhalf - 12.5).abs() < 1e-9);
    assert!((m.ph_half - ind_ph(&m.st.sys, 4.756, 12.5)).abs() < 1e-7);
    assert!((m.ph_eq - ind_ph(&m.st.sys, 4.756, 25.0)).abs() < 1e-7);
    // at equivalence: neutral for strong-strong, basic for a weak acid, acidic for a weak base
    assert!((mdl(&[s("mode", SASB)]).ph_eq - 7.0).abs() < 1e-9);
    assert!(mdl(&[s("mode", WASB)]).ph_eq > 7.0 && mdl(&[s("mode", WBSA)]).ph_eq < 7.0);
    // the buffer's equivalence volume is the titrant that uses up the form it reacts with
    let bm = mdl(&[s("mode", BUF), f("c_an", 0.2), f("c_salt", 0.05), f("v_an", 20.0), f("c_ti", 0.1)]);
    assert!((bm.veq - 40.0).abs() < 1e-9, "{}", bm.veq);
    let ba = mdl(&[s("mode", BUF), s("add", "Strong acid"), f("c_an", 0.2), f("c_salt", 0.05), f("v_an", 20.0), f("c_ti", 0.1)]);
    assert!((ba.veq - 10.0).abs() < 1e-9, "{}", ba.veq);
}

#[test]
fn the_buffer_region_and_capacity_agree_with_the_independent_curve() {
    let mut r = Rng(23);
    let mut checked = 0;
    for _ in 0..300 {
        let acid = r.next() % 2 == 0;
        let l: L = vec![
            s("mode", if r.next() % 3 == 0 { if acid { WASB } else { WBSA } } else { BUF }),
            s("pair", if acid { PAIRS[0] } else { PAIRS[1] }),
            s("add", ADDS[(r.next() % 2) as usize]),
            s("acid", ACID_CHOICES[(r.next() % 8) as usize]),
            s("base", BASE_CHOICES[(r.next() % 6) as usize]),
            f("c_an", r.pick(&[0.001, 0.02, 0.1, 0.5])),
            f("c_salt", r.pick(&[0.0, 0.001, 0.05, 0.1, 0.4])),
            f("c_ti", r.pick(&[0.0005, 0.01, 0.1, 1.0])),
            f("v_an", r.pick(&[10.0, 25.0, 50.0])),
            s("task", T_BUF),
        ];
        let m = mdl(&l);
        let sys = m.st.sys;
        let pka = m.st.pka();
        let ph = |v: f64| ind_ph(&sys, pka, v);
        // the region's ends are where the independent pH is pKa +- 1 (or the start)
        if let Some((a, c)) = m.region {
            for e in [a, c] {
                if e == 0.0 { assert!((ph(0.0) - pka).abs() <= 1.0 + 1e-9, "{}", show(&l)); continue; }
                let d = (ph(e) - pka).abs();
                if e < sys.v_eq().max(m.v_lim) * (1.0 - 1e-9) && (e - m.veq).abs() > 1e-9 && (e - m.v_lim).abs() > 1e-9 { assert!((d - 1.0).abs() < 1e-5, "region end {e}: |pH - pKa| = {d} in {}", show(&l)); }
            }
            assert!(a <= c);
        }
        if sys.mode == Mode::Buffer {
            let edge = if sys.rising() { pka + 1.0 } else { pka - 1.0 };
            match m.out {
                Out::At(x) => { assert!((ph(x) - edge).abs() < 1e-5, "capacity {x}: pH {} vs {edge} in {}", ph(x), show(&l)); checked += 1; }
                Out::StartsOut => assert!(if sys.rising() { ph(0.0) > edge - 1e-9 } else { ph(0.0) < edge + 1e-9 }, "{}", show(&l)),
                Out::Never(x) => { for k in 0..=50 { let v = x * k as f64 / 50.0; assert!(if sys.rising() { ph(v) <= edge + 1e-6 } else { ph(v) >= edge - 1e-6 }, "{}", show(&l)); } }
            }
        }
    }
    assert!(checked > 60, "{checked}");
}

#[test]
fn the_indicator_choice_follows_the_steep_part() {
    let mut checked = 0;
    for mode in [SASB, SBSA, WASB, WBSA] {
        for c in [0.001, 0.01, 0.1, 1.0] {
            for set in IND_SETS {
                for (acid, base) in [("Acetic acid", "Ammonia"), ("Hydrocyanic acid", "Pyridine"), ("Hydrofluoric acid", "Methylamine")] {
                    let l: L = vec![s("mode", mode), f("c_an", c), f("c_ti", c), s("ind_set", set), s("task", T_IND), s("acid", acid), s("base", base)];
                    let m = mdl(&l);
                    let sys = m.st.sys;
                    let pka = m.st.pka();
                    let a = ind_ph(&sys, pka, 0.98 * m.veq);
                    let z = ind_ph(&sys, pka, 1.02 * m.veq);
                    let (lo, hi) = (a.min(z), a.max(z));
                    let want: Vec<usize> = m.offered.iter().copied().filter(|&k| INDICATORS[k].lo >= lo - 1e-9 && INDICATORS[k].hi <= hi + 1e-9).collect();
                    assert_eq!(m.suitable, want, "{}", show(&l));
                    let key = readout(&m, true).join("\n");
                    match m.best {
                        Some(k) => { assert!(key.contains(&format!("Best: {}", INDICATORS[k].name)), "{key}"); checked += 1; }
                        None => assert!(key.contains("None of these") || key.contains("no listed indicator"), "{key}"),
                    }
                }
            }
        }
    }
    assert!(checked > 40, "{checked}");
    // the classic answers
    let pick = |l: &[(&'static str, Value)]| mdl(l).best.map(|k| INDICATORS[k].name);
    assert_eq!(pick(&[s("mode", WASB), s("task", T_IND)]), Some("phenolphthalein"));
    assert_eq!(pick(&[s("mode", SASB), s("task", T_IND)]), Some("bromothymol blue"));
    assert_eq!(pick(&[s("mode", WBSA), s("task", T_IND)]), Some("methyl red"));
}

/// The pH printed in the working: every "pH = x.xx" and "gives the same: pH = x.xx" is the independent pH rounded once.
#[test]
fn the_printed_ph_is_the_exact_one_rounded_once_in_every_task() {
    let mut r = Rng(31);
    let mut checked = 0;
    for _ in 0..400 {
        let mode = r.pick(&MODES_ALL);
        let task = r.pick(&TASKS_ALL);
        let dec = 1 + (r.next() % 3) as i64;
        let l: L = vec![s("mode", mode), s("task", task), i("dec", dec), f("c_an", r.pick(&[0.001, 0.05, 0.1, 0.8])), f("c_ti", r.pick(&[0.002, 0.1, 0.25])), f("v_an", r.pick(&[10.0, 25.0, 40.0])), f("v_q", r.pick(&[0.0, 3.0, 12.5, 25.0, 31.0])), f("c_salt", r.pick(&[0.0, 0.03, 0.1])), s("acid", ACID_CHOICES[(r.next() % 9) as usize]), s("base", BASE_CHOICES[(r.next() % 7) as usize]), s("pair", PAIRS[(r.next() % 2) as usize]), s("add", ADDS[(r.next() % 2) as usize])];
        let m = mdl(&l);
        let sys = m.st.sys;
        let pka = m.st.pka();
        let d = m.st.dec;
        let key = readout(&m, true).join("\n");
        let want_at = |v: f64| fx(ind_ph(&sys, pka, v), d);
        match m.st.task {
            Task::Eq => { assert!(key.contains(&format!("pH {}.", want_at(m.veq))), "{key}"); checked += 1; }
            Task::Pka => { assert!(key.contains(&format!("is {}", want_at(m.vhalf))), "{key}"); checked += 1; }
            Task::Indicator => { assert!(key.contains(&format!("(exact): {}.", want_at(m.veq))), "{key}"); checked += 1; }
            Task::PhAt => { assert!(key.contains(&format!("pH = {}", want_at(m.st.v_q))), "{key}"); checked += 1; }
            Task::BufferAdd => { assert!(key.contains(&format!("pH = {}", want_at(0.0))) && key.contains(&format!("pH = {}", want_at(m.st.v_q))), "{key}"); checked += 1; }
        }
        // the key's legend prints the same values as the working
        let svg = svg_of(&l);
        let legend: Vec<String> = texts(&svg).into_iter().filter(|t| t.kind == "label").map(|t| t.text).collect();
        for t in &legend {
            if let Some(rest) = t.strip_prefix("Equivalence point: ") { assert!(rest.contains(&format!("pH {}", want_at(m.veq))), "{t}"); }
            if let Some(rest) = t.strip_prefix("Half-equivalence point: ") { assert!(rest.contains(&format!("pH {}", want_at(m.vhalf))), "{t}"); }
        }
    }
    assert!(checked >= 380, "{checked}");
}

#[test]
fn the_shortcut_lines_are_worked_from_the_givens() {
    // Henderson-Hasselbalch at 10 mL of the acetic acid default
    let m = mdl(&[s("mode", WASB), s("task", T_PH), f("v_q", 10.0)]);
    let t = readout(&m, true).join("\n");
    assert!(t.contains("NaOH added: 0.1 M \u{d7} 10 mL = 1.000 mmol."), "{t}");
    assert!(t.contains("1.500 mmol CH\u{2083}COOH and 1.000 mmol CH\u{2083}COO\u{207b}"), "{t}");
    assert!(t.contains("= 4.756 + log(1.000 / 1.500) = 4.58."), "{t}");
    // after equivalence
    let m = mdl(&[s("mode", WASB), s("task", T_PH), f("v_q", 30.0)]);
    let t = readout(&m, true).join("\n");
    assert!(t.contains("0.5000 mmol OH\u{207b} in excess") && t.contains("pH = 14.00 \u{2212} 2.04 = 11.96."), "{t}");
    // the start of a weak acid: the square root
    let m = mdl(&[s("mode", WASB), s("task", T_PH), f("v_q", 0.0)]);
    let t = readout(&m, true).join("\n");
    assert!(t.contains("sqrt(Ka \u{d7} C)") && t.contains("pH = 2.88."), "{t}");
    // at equivalence for a weak acid: its conjugate base alone
    let m = mdl(&[s("mode", WASB), s("task", T_EQ)]);
    let t = readout(&m, true).join("\n");
    assert!(t.contains("CH\u{2083}COO\u{207b} alone is a weak base") && t.contains("Equivalence: n(NaOH) = n(CH\u{2083}COOH) = 0.1 M \u{d7} 25 mL = 2.500 mmol, so V = 2.500 mmol / 0.1 M = 25.00 mL."), "{t}");
    assert!(t.contains("so it is basic: pH 8.73."), "{t}");
    // where the shortcut fails it says so: near the equivalence point of a dilute strong acid
    let m = mdl(&[s("mode", SASB), s("task", T_PH), f("c_an", 0.0001), f("c_ti", 0.0001), f("v_q", 24.99)]);
    let t = readout(&m, true).join("\n");
    assert!(t.contains("the shortcut's") && t.contains("water's own"), "{t}");
    // the pKa read off agrees with the CRC value for acetic acid, and says how far off it is for hydrofluoric acid
    let t = readout(&mdl(&[s("mode", WASB), s("task", T_PKA)]), true).join("\n");
    assert!(t.contains("gives pKa = 4.756;") && t.contains("the reading agrees"), "{t}");
    let t = readout(&mdl(&[s("mode", WASB), s("task", T_PKA), s("acid", "Hydrofluoric acid")]), true).join("\n");
    assert!(t.contains("gives pKa = 3.2;") && t.contains("the reading is 0.02 higher") && t.contains("some of the HF ionises, adding F\u{207b}"), "{t}");
    // Ka is worked from the printed reading (a value read off a graph), not from the unrounded pH
    assert!(t.contains("Ka = 10^(\u{2212}3.22) = 6.026 \u{d7} 10\u{207b}\u{2074}."), "{t}");
    // a base: pKb from 14.00 - the reading
    let t = readout(&mdl(&[s("mode", WBSA), s("task", T_PKA)]), true).join("\n");
    assert!(t.contains("pKb = 14.00 \u{2212} 9.25 = 4.75"), "{t}");
}

// ---------------------------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------------------------

#[test]
fn it_is_registered_with_a_faq_examples_and_its_controls() {
    let d = def();
    assert_eq!(d.id, "buffer-curves");
    let ps = (d.params)();
    let mut keys: Vec<&str> = ps.iter().map(|p| p.key).collect();
    keys.sort();
    let n = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), n, "duplicate param keys");
    for k in ["mode", "task", "acid", "pka", "base", "pkb", "c_an", "v_an", "c_ti", "pair", "c_salt", "add", "v_q", "v_max", "ind_set", "dec", "show_answers", "style", "label_size"] {
        assert!(keys.contains(&k), "no {k} control");
    }
    assert!((4..=6).contains(&crate::figures::faq::faq_for(ID).len()));
    let ex = examples(ID).unwrap();
    assert!(ex.len() >= 5);
    for e in ex {
        let mut p = Params::defaults(&ps);
        for (k, v) in e.params { p.0.insert(k.to_string(), v); }
        p.normalize(&ps, false);
        let svg = d.svg(&p);
        assert!(svg.contains("<svg") && !all_text(&svg).contains("NaN"), "{}", e.name);
        assert!(to_png(&svg, 0.5).is_ok(), "{}", e.name);
    }
    assert!(crate::figures::chemistry::skin_intro(ID).is_some());
    assert!(!crate::figures::notes_for(ID, &with(&[f("c_an", 9.0)])).is_empty());
}

#[test]
fn the_data_is_the_crc_handbooks() {
    // pKa at 25 C, CRC Handbook (organic and inorganic tables); a base by the pKa of its conjugate acid
    let a: Vec<(&str, f64)> = ACIDS.iter().map(|w| (w.name, w.pka)).collect();
    assert_eq!(a, vec![("acetic acid", 4.756), ("formic acid", 3.75), ("benzoic acid", 4.204), ("propanoic acid", 4.87), ("hydrofluoric acid", 3.20), ("nitrous acid", 3.25), ("hypochlorous acid", 7.40), ("hydrocyanic acid", 9.21)]);
    let bs: Vec<(&str, f64)> = BASES.iter().map(|w| (w.name, w.pka)).collect();
    assert_eq!(bs, vec![("ammonia", 9.25), ("methylamine", 10.66), ("trimethylamine", 9.80), ("pyridine", 5.23), ("hydroxylamine", 5.94), ("aniline", 4.87)]);
    // indicator ranges, CRC Handbook "Acid-Base Indicators"
    let r: Vec<(&str, f64, f64)> = INDICATORS.iter().map(|x| (x.name, x.lo, x.hi)).collect();
    assert_eq!(r, vec![("methyl orange", 3.2, 4.4), ("bromocresol green", 3.8, 5.4), ("methyl red", 4.8, 6.0), ("bromothymol blue", 6.0, 7.6), ("phenol red", 6.6, 8.0), ("phenolphthalein", 8.2, 10.0), ("thymolphthalein", 9.4, 10.6), ("alizarin yellow R", 10.1, 12.0)]);
    // the base's pKb is printed as 14.00 - pKa
    let t = all_text(&svg_of(&[s("mode", WBSA)]));
    assert!(t.contains("NH\u{2083} (ammonia, pKb 4.75)"), "{t}");
    let t = all_text(&svg_of(&[s("mode", WASB)]));
    assert!(t.contains("CH\u{2083}COOH (acetic acid, pKa 4.756)"), "{t}");
}

#[test]
fn odd_inputs_are_noted_not_panicked() {
    let n = warnings(&raw(&[f("c_an", 0.0)]));
    assert!(n.iter().any(|x| x.contains("must be greater than 0")), "{n:?}");
    let n = warnings(&raw(&[f("c_an", 50.0)]));
    assert!(n.iter().any(|x| x.contains("above the most")), "{n:?}");
    let n = warnings(&raw(&[f("c_an", 0.123456)]));
    assert!(n.iter().any(|x| x.contains("4 significant figures")), "{n:?}");
    let n = warnings(&raw(&[s("acid", "Custom"), f("pka", 0.2)]));
    assert!(n.iter().any(|x| x.contains("custom pKa")), "{n:?}");
    // a custom pKa is not mentioned when the acid is a preset
    let n = warnings(&raw(&[f("pka", 0.2)]));
    assert!(!n.iter().any(|x| x.contains("custom pKa")), "{n:?}");
    // task fall-backs are said
    let n = warnings(&raw(&[s("mode", SASB), s("task", T_PKA)]));
    assert!(n.iter().any(|x| x.contains("no pKa")), "{n:?}");
    let n = warnings(&raw(&[s("mode", BUF), s("task", T_IND)]));
    assert!(n.iter().any(|x| x.contains("buffer graph asks")), "{n:?}");
    // the equivalence point beyond a short axis is said in the notes and on the figure
    let l = [f("v_max", 10.0), s("task", T_EQ)];
    let n = warnings(&with(&l));
    assert!(n.iter().any(|x| x.contains("beyond the end of the volume axis")), "{n:?}");
    let stu = all_text(&svg_of(&[f("v_max", 10.0), b(SHOW_ANSWERS, false)]));
    assert!(stu.contains("lies beyond the end of the volume axis"), "{stu}");
    // pH outside 0 to 14 widens the axis and says why
    let n = warnings(&with(&[s("mode", SASB), f("c_an", 2.0)]));
    assert!(n.iter().any(|x| x.contains("beyond 0 to 14")), "{n:?}");
}

#[test]
fn odd_values_never_panic_or_print_nan() {
    let mut r = Rng(3);
    for k in 0..400 {
        let mut l: L = Vec::new();
        for key in ["c_an", "v_an", "c_ti", "c_salt", "v_q", "v_max", "pka", "pkb"] {
            l.push((key, [Value::F(f64::NAN), Value::F(f64::INFINITY), Value::F(-1.0), Value::F(0.0), Value::F(1e300), Value::F(1e-300), Value::F(1.0), Value::I(5), Value::S("a".into())][(r.next() % 9) as usize].clone()));
        }
        l.push(("mode", Value::S(MODES_ALL[k % 5].into())));
        l.push(("task", Value::S(TASKS_ALL[(k / 5) % 5].into())));
        l.push(("dec", [Value::I(-4), Value::I(9), Value::F(f64::NAN), Value::S("q".into())][(r.next() % 4) as usize].clone()));
        l.push(("acid", Value::S(["Custom", "zz", "Acetic acid"][(r.next() % 3) as usize].into())));
        l.push(("base", Value::S(["Custom", "zz", "Pyridine"][(r.next() % 3) as usize].into())));
        for ans in [true, false] {
            let mut p = raw(&l);
            p.0.insert(SHOW_ANSWERS.to_string(), Value::B(ans));
            let svg = def().svg(&p);
            let t = all_text(&svg);
            assert!(svg.contains("</svg>"), "{}", show(&l));
            assert!(!svg.contains("NaN") && !t.contains("inf") && !t.contains("undefined"), "bad number in\n{t}\n{}", show(&l));
            let _ = warnings(&p);
        }
    }
}

#[test]
fn rendering_is_deterministic() {
    for l in student_cases().into_iter().step_by(7) { assert_eq!(svg_of(&l), svg_of(&l)); }
}

// ---------------------------------------------------------------------------------------------
// The student copy
// ---------------------------------------------------------------------------------------------

fn student_cases() -> Vec<L> {
    let mut r = Rng(99);
    let mut out = Vec::new();
    for mode in MODES_ALL {
        for task in TASKS_ALL {
            for _ in 0..6 {
                out.push(vec![
                    s("mode", mode),
                    s("task", task),
                    s("acid", ACID_CHOICES[(r.next() % 9) as usize]),
                    s("base", BASE_CHOICES[(r.next() % 7) as usize]),
                    f("pka", r.pick(&[2.5, 4.76, 9.1])),
                    f("pkb", r.pick(&[3.3, 4.75, 8.8])),
                    f("c_an", r.pick(&[0.001, 0.05, 0.1, 0.5])),
                    f("c_ti", r.pick(&[0.01, 0.1, 0.2])),
                    f("v_an", r.pick(&[10.0, 25.0, 50.0])),
                    f("c_salt", r.pick(&[0.0, 0.05, 0.1])),
                    s("pair", PAIRS[(r.next() % 2) as usize]),
                    s("add", ADDS[(r.next() % 2) as usize]),
                    f("v_q", r.pick(&[0.0, 5.0, 12.0, 25.0, 40.0])),
                    s("ind_set", IND_SETS[(r.next() % 2) as usize]),
                    i("dec", 1 + (r.next() % 3) as i64),
                ]);
            }
        }
    }
    out
}

/// Every value the key prints as an answer, formatted as the key prints it.
fn answer_strings(m: &Model) -> Vec<String> {
    let d = m.st.dec;
    let mut v = vec![s4(m.veq), fx(m.ph_eq, d), s4(m.vhalf), fx(m.ph_half, d), fx(m.ph_q, d), fx(m.ph0, d), s4(reading_k(m).1)];
    if let Out::At(x) = m.out { v.push(s4(x)); }
    if let Some((a, c)) = m.region { v.push(s4(a)); v.push(s4(c)); }
    if m.st.sys.base_pair { v.push(fx(PKW - m.ph_half, d)); }
    v
}

const WORDS: [&str; 13] = ["Henderson", "Hasselbalch", "charge balance", "Suitable", "suitable", "Best", "so it is basic", "so it is acidic", "so it is neutral", "capacity", "read off", "sqrt", "mmol"];

#[test]
fn the_student_copy_hides_every_answer_and_every_law_name() {
    let mut n = 0;
    for l in student_cases() {
        let mut ls = l.clone();
        ls.push(b(SHOW_ANSWERS, false));
        let mut lk = l.clone();
        lk.push(b(SHOW_ANSWERS, true));
        let m = mdl(&lk);
        let stu = svg_of(&ls);
        let key = svg_of(&lk);
        let t = all_text(&stu);
        for w in WORDS { assert!(!t.contains(w) && !title_of(&stu).contains(w) && !desc_of(&stu).contains(w), "{w:?} on the student copy of {}\n{t}", show(&l)); }
        // an answer never appears in a non-axis text, unless the same string is a given
        let givens: String = givens(&m, false).join(" ");
        let non_axis: Vec<String> = texts(&stu).into_iter().filter(|x| x.kind != "axis").map(|x| x.text).collect();
        for a in answer_strings(&m) {
            if a.len() < 3 || givens.contains(&a) { continue; }
            for x in &non_axis { assert!(!x.split(|c: char| c == ' ' || c == '(' || c == ')' || c == ',').any(|w| w == a || w.trim_end_matches('.') == a), "answer {a} on the student copy of {}:\n{x}", show(&l)); }
            for part in stu.split(" data-").skip(1) { assert!(part.split('"').nth(1) != Some(a.as_str()), "{a} in a data attribute of {}", show(&l)); }
            assert!(!title_of(&stu).contains(&a) && !desc_of(&stu).contains(&a));
        }
        // no answer feature: no marker, no band, no guide but the volume asked about
        let g = geo(&stu);
        assert!(g.marks.is_empty() && g.bands.is_empty(), "a marker or band on the student copy of {}", show(&l));
        assert!(g.strokes.iter().all(|x| x.0 != "guide" || x.1 == "vq"), "an answer guide on the student copy of {}", show(&l));
        // the curve is drawn exactly when it is the given
        let has_curve = g.paths.iter().any(|p| p.0 == "curve");
        assert_eq!(has_curve, matches!(m.st.task, Task::Eq | Task::Pka | Task::Indicator), "curve {has_curve} for {:?} in {}", m.st.task, show(&l));
        // finding pKa: the acid or base is not named
        if m.st.task == Task::Pka {
            assert!(!t.contains(m.st.weak.name) && !t.contains(&pretty(m.st.weak.formula)) || m.st.custom, "the analyte is named on the student copy of {}\n{t}", show(&l));
            assert!(!t.contains("pKa ") || t.contains("pKa = ______") || t.contains("pKa of BH"), "{t}");
        }
        assert_ne!(stu, key, "student equals key for {}", show(&l));
        n += 1;
    }
    assert!(n >= 150, "{n}");
}

/// The SVG with the text of notes emptied (the givens differ in their numbers), so only the drawing remains.
fn geometry_only(svg: &str) -> String {
    let mut out = String::new();
    for (k, part) in svg.split("<text").enumerate() {
        if k == 0 { out.push_str(part); continue; }
        let (tag, rest) = part.split_once('>').unwrap();
        let (body, after) = rest.split_once("</text>").unwrap();
        out.push_str("<text");
        out.push_str(tag);
        out.push('>');
        if attr(tag, "data-o").as_deref() != Some("note") { out.push_str(body); }
        out.push_str("</text>");
        out.push_str(after);
    }
    out
}

#[test]
fn student_copies_with_equal_givens_and_different_hidden_answers_are_the_same() {
    let mut compared = 0;
    // 1. finding pKa: a named acid (or base) and a custom one with the same pKa give the same student copy, every
    //    preset; the key differs (it names it)
    for (k, w) in ACIDS.iter().enumerate() {
        for c in [0.01, 0.1] {
            let base = |named: bool| { let mut l: L = vec![s("mode", WASB), s("task", T_PKA), f("c_an", c), b(SHOW_ANSWERS, false)]; if named { l.push(s("acid", ACID_CHOICES[k])); } else { l.push(s("acid", "Custom")); l.push(f("pka", w.pka)); } l };
            assert_eq!(svg_of(&base(true)), svg_of(&base(false)), "{}", w.name);
            let key = |l: L| { let mut l = l; l.retain(|x| x.0 != SHOW_ANSWERS); l.push(b(SHOW_ANSWERS, true)); svg_of(&l) };
            assert_ne!(key(base(true)), key(base(false)), "{}", w.name);
            compared += 1;
        }
    }
    for (k, w) in BASES.iter().enumerate() {
        let named: L = vec![s("mode", WBSA), s("task", T_PKA), s("base", BASE_CHOICES[k]), b(SHOW_ANSWERS, false)];
        let custom: L = vec![s("mode", WBSA), s("task", T_PKA), s("base", "Custom"), f("pkb", sig4(PKW - w.pka)), b(SHOW_ANSWERS, false)];
        assert_eq!(svg_of(&named), svg_of(&custom), "{}", w.name);
        compared += 1;
    }
    // 2. the pH at a volume and the buffer pH: the curve is the answer, so changing pKa (the hidden answer moves) leaves
    //    the drawing unchanged
    let mut r = Rng(77);
    for mode in MODES_ALL {
        for task in [T_PH, T_BUF] {
            for _ in 0..8 {
                let mut base: L = vec![s("mode", mode), s("task", task), f("c_an", r.pick(&[0.01, 0.1, 0.4])), f("c_ti", r.pick(&[0.05, 0.1])), f("v_an", r.pick(&[20.0, 25.0])), f("v_q", r.pick(&[0.0, 6.0, 15.0, 30.0])), f("c_salt", r.pick(&[0.0, 0.1])), s("pair", PAIRS[(r.next() % 2) as usize]), s("add", ADDS[(r.next() % 2) as usize]), b(SHOW_ANSWERS, false)];
                base.extend([s("acid", "Custom"), s("base", "Custom")]);
                let (mut a, mut c) = (base.clone(), base.clone());
                a.extend([f("pka", 3.1), f("pkb", 3.4)]);
                c.extend([f("pka", 9.6), f("pkb", 8.9)]);
                let (ma, mc) = (mdl(&a), mdl(&c));
                // the hidden answer (the curve, and the pH it gives) really differs
                if mode != SASB && mode != SBSA { assert!((ma.ph_q - mc.ph_q).abs().max((ma.ph0 - mc.ph0).abs()) > 0.05, "{}", show(&a)); }
                assert_eq!(geometry_only(&svg_of(&a)), geometry_only(&svg_of(&c)), "the student drawing follows the hidden answer:\n{}\n{}", show(&a), show(&c));
                compared += 1;
            }
        }
    }
    // 3. the decimal places only change how the key rounds: every student copy is the same for 1, 2 and 3
    for l in student_cases().into_iter().step_by(3) {
        let mut l: L = l.into_iter().filter(|x| x.0 != "dec").collect();
        l.push(b(SHOW_ANSWERS, false));
        let one = { let mut x = l.clone(); x.push(i("dec", 1)); svg_of(&x) };
        let three = { let mut x = l.clone(); x.push(i("dec", 3)); svg_of(&x) };
        assert_eq!(one, three, "{}", show(&l));
        compared += 1;
    }
    assert!(compared > 120, "{compared}");
}

#[test]
fn the_axes_depend_on_the_givens_only() {
    // pH 0 to 14 on every copy; the volume axis from the equivalence volume (the givens), the same on key and student copy
    for mode in MODES_ALL {
        for task in TASKS_ALL {
            let ax = |ans: bool, pka: f64| {
                let svg = svg_of(&[s("mode", mode), s("task", task), b(SHOW_ANSWERS, ans), s("acid", "Custom"), f("pka", pka), s("base", "Custom"), f("pkb", pka)]);
                texts(&svg).into_iter().filter(|t| t.kind == "axis").map(|t| format!("{:.2},{:.2},{}", t.x, t.y, t.text)).collect::<Vec<_>>()
            };
            assert_eq!(ax(true, 4.0), ax(false, 4.0), "{mode} {task}");
            assert_eq!(ax(false, 3.0), ax(false, 10.5), "{mode} {task}");
            assert_eq!(crate::figures::svg_size(&svg_of(&[s("mode", mode), s("task", task), b(SHOW_ANSWERS, true)])).0, crate::figures::svg_size(&svg_of(&[s("mode", mode), s("task", task), b(SHOW_ANSWERS, false)])).0);
        }
    }
}

#[test]
fn desc_is_short_ascii_without_numbers_and_neutral_on_the_student_copy() {
    for l in student_cases() {
        for ans in [true, false] {
            let mut l2 = l.clone();
            l2.push(b(SHOW_ANSWERS, ans));
            let svg = svg_of(&l2);
            let d = desc_of(&svg);
            assert!(d.is_ascii() && d.len() <= 300 && !d.is_empty(), "{d:?}");
            assert!(!d.chars().any(|c| c.is_ascii_digit()), "a number in the description: {d}");
            if !ans { assert!(!d.contains("marked") && !d.contains("shaded") && !d.contains("acetic") && !d.contains("ammonia"), "{d}"); }
            let t = title_of(&svg);
            assert!(t.is_ascii() && !t.chars().any(|c| c.is_ascii_digit()), "{t}");
            for part in svg.split(" data-").skip(1) {
                let v = part.split('"').nth(1).unwrap_or("");
                assert!(v.len() <= 8 && v.chars().all(|c| c.is_ascii_alphanumeric()), "data attribute {v:?}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Print, axes, sizes
// ---------------------------------------------------------------------------------------------

#[test]
fn print_is_grey_only_and_tells_the_features_apart_by_dash_and_marker() {
    for (mode, task) in [(WASB, T_PKA), (WASB, T_IND), (BUF, T_BUF), (SASB, T_PH), (WBSA, T_EQ)] {
        let svg = svg_of(&[s("style", "Print"), s("mode", mode), s("task", task), s("ind_set", IND_SETS[1])]);
        for part in svg.split('"').filter(|x| x.starts_with('#')) {
            let h: String = if part.len() == 4 { part[1..].chars().flat_map(|c| [c, c]).collect() } else { part[1..].to_string() };
            assert!(h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()), "{part}");
            let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap();
            assert!(c(0) == c(2) && c(2) == c(4), "Print draws a hue: {part} in {mode}");
        }
        // each marked feature has its own marker shape and its own guide dash
        let g = geo(&svg);
        let mut shapes: BTreeMap<String, String> = BTreeMap::new();
        for (feat, _, el) in &g.marks { shapes.insert(feat.clone(), el.clone()); }
        let mut dashes: BTreeMap<String, String> = BTreeMap::new();
        for (kind, feat, _, style) in &g.paths { if kind == "guide" { dashes.insert(feat.clone(), style.rsplit('|').next().unwrap().to_string()); } }
        let sv: Vec<&String> = shapes.iter().filter(|(k, _)| *k != "reg" && *k != "ind").map(|x| x.1).collect();
        for a in 0..sv.len() { for c in a + 1..sv.len() { assert_ne!(sv[a], sv[c], "two features share a marker in {mode} {task}: {shapes:?}"); } }
        let dv: Vec<&String> = dashes.values().collect();
        for a in 0..dv.len() { for c in a + 1..dv.len() { assert_ne!(dv[a], dv[c], "two features share a dash in {mode} {task}: {dashes:?}"); } }
        assert!(!shapes.is_empty() && !dashes.is_empty(), "{mode} {task}: {shapes:?}");
    }
    // colour uses colour
    let svg = svg_of(&[]);
    assert!(svg.split('"').any(|x| x.starts_with('#') && x.len() == 7 && { let c = |i: usize| u8::from_str_radix(&x[i..i + 2], 16).unwrap_or(0); !(c(1) == c(3) && c(3) == c(5)) }));
}

#[test]
fn the_axes_are_labelled_once_in_order_on_their_ticks() {
    for l in student_cases().into_iter().step_by(2) {
        for ans in [true, false] {
            let mut l2 = l.clone();
            l2.push(b(SHOW_ANSWERS, ans));
            let svg = svg_of(&l2);
            let ts = texts(&svg);
            let gm = geo(&svg);
            let xt: Vec<&T> = ts.iter().filter(|t| t.kind == "axis" && t.anchor == "middle" && t.rot.is_none() && t.text.chars().next().is_some_and(|c| c.is_ascii_digit())).collect();
            let yt: Vec<&T> = ts.iter().filter(|t| t.kind == "axis" && t.anchor == "end").collect();
            assert!(xt.len() >= 3 && yt.len() >= 7, "{}", show(&l2));
            assert!(xt.windows(2).all(|w| w[1].x > w[0].x + 5.0), "x ticks out of order: {:?}", xt.iter().map(|t| &t.text).collect::<Vec<_>>());
            assert!(yt.windows(2).all(|w| w[1].y < w[0].y - 5.0), "y ticks out of order");
            let mut u: Vec<&String> = xt.iter().map(|t| &t.text).collect();
            let n = u.len();
            u.dedup();
            assert_eq!(u.len(), n, "a repeated x tick");
            // each tick label sits on its tick: the x label's centre on a tick line, the y label's baseline by its tick
            let xticks: Vec<f64> = gm.strokes.iter().filter(|s_| s_.0 == "axis" && (s_.2 .0 .0 - s_.2 .1 .0).abs() < 1e-9 && (s_.2 .1 .1 - s_.2 .0 .1 - 5.0).abs() < 1e-6).map(|s_| s_.2 .0 .0).collect();
            for t in &xt { assert!(xticks.iter().any(|x| (x - t.x).abs() < 0.011), "x tick {} at {} has no tick line", t.text, t.x); }
            let ytk: Vec<f64> = gm.strokes.iter().filter(|s_| s_.0 == "axis" && (s_.2 .0 .1 - s_.2 .1 .1).abs() < 1e-9 && (s_.2 .1 .0 - s_.2 .0 .0 - 5.0).abs() < 1e-6).map(|s_| s_.2 .0 .1).collect();
            for t in &yt { assert!(ytk.iter().any(|y| (y + 0.36 * t.fs - t.y).abs() < 0.02), "y tick {} has no tick line", t.text); }
            // the values: x ticks evenly spaced in value and in px; y ticks 0, 2, ..., 14
            let ys: Vec<&str> = yt.iter().map(|t| t.text.as_str()).collect();
            assert_eq!(&ys[..8], &["0", "2", "4", "6", "8", "10", "12", "14"]);
            let titles: Vec<&T> = ts.iter().filter(|t| t.kind == "axis" && t.text.starts_with("Volume of")).collect();
            assert_eq!(titles.len(), 1);
        }
    }
}

#[test]
fn the_answer_is_the_same_in_every_size_and_style() {
    for l in student_cases().into_iter().step_by(5) {
        let mut seen: Option<Vec<String>> = None;
        for size in ["Small", "Medium", "Large"] {
            for style in ["Colour", "Print"] {
                let mut l2 = l.clone();
                l2.extend([s("label_size", size), s("style", style), b(SHOW_ANSWERS, true)]);
                let p = with(&l2);
                let rep = build(&p).1;
                let leg: Vec<String> = texts(&def().svg(&p)).into_iter().filter(|t| t.kind == "label").map(|t| t.text).collect();
                let mut both = rep.readout.clone();
                both.push(leg.join(" "));
                // the legend wraps with the size, so compare its words
                let words: Vec<String> = both.join(" ").split_whitespace().map(|w| w.to_string()).collect();
                match &seen { None => seen = Some(words), Some(w) => assert_eq!(w, &words, "{} {size} {style}", show(&l)) }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Handles and PNGs
// ---------------------------------------------------------------------------------------------

#[test]
fn handles_move_the_equivalence_point_the_pka_and_the_volume_asked_about() {
    let d = def();
    let p = with(&[s("mode", WASB), s("task", T_PKA), s("acid", "Custom")]);
    let (w, h) = crate::figures::svg_size(&d.svg(&p));
    let hs = handles(ID, &p, (w, h)).expect("handles");
    assert_eq!(hs.len(), 2, "{:?}", hs.iter().map(|h| h.tip.clone()).collect::<Vec<_>>());
    for hd in &hs {
        assert!(hd.pos.0 >= 0.0 && hd.pos.0 <= w && hd.pos.1 >= 0.0 && hd.pos.1 <= h, "{} at {:?}", hd.tip, hd.pos);
        let mut q = p.clone();
        (hd.drag.clone().unwrap())(&mut q, hd.pos);
        assert_eq!(format!("{:?}", q.0), format!("{:?}", p.0), "{}: a drag to its own place changed something", hd.tip);
    }
    // dragging the equivalence point right needs a weaker titrant
    let eq = hs.iter().find(|h| h.tip.contains("equivalence point left")).unwrap();
    let mut q = p.clone();
    (eq.drag.clone().unwrap())(&mut q, (eq.pos.0 + 60.0, eq.pos.1));
    assert!(q.f("c_ti") < p.f("c_ti"), "{}", q.f("c_ti"));
    let m2 = model_of(&q);
    assert!(m2.veq > 25.0);
    // dragging the half-equivalence point up raises pKa (a custom acid)
    let half = hs.iter().find(|h| h.tip.contains("half-equivalence")).unwrap();
    let mut q = p.clone();
    (half.drag.clone().unwrap())(&mut q, (half.pos.0, half.pos.1 - 40.0));
    assert_eq!(q.s("acid"), "Custom");
    assert!(q.f("pka") > 4.76, "{}", q.f("pka"));
    // the volume asked about
    let p = with(&[s("mode", WASB), s("task", T_PH)]);
    let hs = handles(ID, &p, (w, h)).unwrap();
    let vq = hs.iter().find(|h| h.tip.contains("dashed line")).expect("a volume handle");
    let mut q = p.clone();
    (vq.drag.clone().unwrap())(&mut q, (vq.pos.0 + 50.0, vq.pos.1));
    assert!(q.f("v_q") > p.f("v_q"));
    assert!(!crate::figures::interact::handles_for(ID, &p, (w, h)).is_empty());
}

#[test]
fn buffer_pngs() {
    let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
    let mut cases: Vec<(String, L)> = Vec::new();
    for (mode, tag) in [(SASB, "sasb"), (SBSA, "sbsa"), (WASB, "wasb"), (WBSA, "wbsa"), (BUF, "buf")] {
        for (task, tt) in [(T_EQ, "eq"), (T_PKA, "pka"), (T_IND, "ind"), (T_PH, "ph"), (T_BUF, "buf")] {
            cases.push((format!("{tag}-{tt}"), vec![s("mode", mode), s("task", task)]));
        }
    }
    cases.push(("buf-base-acid".into(), vec![s("mode", BUF), s("pair", PAIRS[1]), s("add", ADDS[1])]));
    cases.push(("wasb-hcn".into(), vec![s("mode", WASB), s("acid", "Hydrocyanic acid"), s("task", T_IND), s("ind_set", IND_SETS[1])]));
    cases.push(("dilute".into(), vec![s("mode", SASB), f("c_an", 0.0001), f("c_ti", 0.0001), s("task", T_IND)]));
    for (tag, list) in cases {
        for size in ["Small", "Medium", "Large"] {
            for (kind, extra) in [("key", vec![b(SHOW_ANSWERS, true)]), ("student", vec![b(SHOW_ANSWERS, false)]), ("print", vec![s("style", "Print")])] {
                let mut l = list.clone();
                l.extend(extra);
                l.push(s("label_size", size));
                std::fs::write(format!("{dir}/bc-{tag}-{size}-{kind}.png"), to_png(&svg_of(&l), 1.5).unwrap()).unwrap();
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The label audit, in real glyphs
// ---------------------------------------------------------------------------------------------

use crate::figures::chemistry::reactionenergy::{hits, seg_hits};

struct Glyphs { opt: resvg::usvg::Options<'static>, memo: HashMap<String, Option<Bx>> }

impl Glyphs {
    fn new() -> Self {
        let mut opt = resvg::usvg::Options::default();
        opt.fontdb = crate::figures::fontdb();
        Glyphs { opt, memo: HashMap::new() }
    }
    fn rel(&mut self, t: &T) -> Option<Bx> {
        let key = format!("{}|{:.2}|{}", t.text, t.fs, t.anchor);
        if let Some(v) = self.memo.get(&key) { return *v; }
        let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="6000" height="600"><text x="3000" y="300" font-size="{:.2}" text-anchor="{}" font-family="{FONT}">{}</text></svg>"#, t.fs, t.anchor, esc(&t.text));
        fn find(g: &resvg::usvg::Group) -> Option<resvg::usvg::Rect> {
            for n in g.children() {
                match n {
                    resvg::usvg::Node::Text(t) => return Some(t.flattened().abs_bounding_box()),
                    resvg::usvg::Node::Group(g2) => { if let Some(r) = find(g2) { return Some(r); } }
                    _ => {}
                }
            }
            None
        }
        let out = resvg::usvg::Tree::from_str(&svg, &self.opt).ok().and_then(|tree| find(tree.root())).filter(|r| r.width() > 0.0).map(|r| (r.left() as f64 - 3000.0, r.right() as f64 - 3000.0, r.top() as f64 - 300.0, r.bottom() as f64 - 300.0));
        if self.memo.len() > 200_000 { self.memo.clear(); }
        self.memo.insert(key, out);
        out
    }
    fn ink(&mut self, t: &T) -> Option<Bx> {
        let r = self.rel(t)?;
        Some(match t.rot { None => (t.x + r.0, t.x + r.1, t.y + r.2, t.y + r.3), Some((cx, cy)) => (cx + r.2, cx + r.3, cy - r.1, cy - r.0) })
    }
}

fn pt_box(p: Pt, b: Bx) -> f64 { (b.0 - p.0).max(0.0).max(p.0 - b.1).hypot((b.2 - p.1).max(0.0).max(p.1 - b.3)) }
fn seg_box(a: Pt, c: Pt, b: Bx) -> f64 {
    if seg_hits(a, c, b) { return 0.0; }
    let ps = |x: f64, y: f64| {
        let (dx, dy) = (c.0 - a.0, c.1 - a.1);
        let l = dx * dx + dy * dy;
        let t = if l == 0.0 { 0.0 } else { (((x - a.0) * dx + (y - a.1) * dy) / l).clamp(0.0, 1.0) };
        (a.0 + t * dx - x).hypot(a.1 + t * dy - y)
    };
    [pt_box(a, b), pt_box(c, b), ps(b.0, b.2), ps(b.0, b.3), ps(b.1, b.2), ps(b.1, b.3)].into_iter().fold(f64::MAX, f64::min)
}

/// Every problem with the text of one render, in real glyphs: text off the canvas, text on text, any text on the
/// curve, an axis line or a marker, a key entry that does not read as its own marker's (more than 40 px x size from it,
/// or more than 15 px from it while 6 px nearer another entry's), a marker not on the curve, and (student copy) any
/// feature only the key may draw. The dotted guides, the bands and the grid are thin or pale and may be crossed.
fn audit(svg: &str, g: &mut Glyphs, lab: f64, student: bool) -> Vec<String> {
    let (w, h) = crate::figures::svg_size(svg);
    let ts = texts(svg);
    let gm = geo(svg);
    let mut bad = Vec::new();
    let inks: Vec<Option<Bx>> = ts.iter().map(|t| g.ink(t)).collect();
    for (t, bx) in ts.iter().zip(&inks) {
        let Some(bx) = bx else { continue };
        if bx.0 < 0.0 || bx.1 > w || bx.2 < 0.0 || bx.3 > h { bad.push(format!("{:?} leaves the {w}x{h} canvas: {bx:?}", t.text)); }
    }
    for a in 0..ts.len() {
        for c in a + 1..ts.len() {
            if let (Some(x), Some(y)) = (inks[a], inks[c]) { if hits(x, y) { bad.push(format!("{:?} on {:?}", ts[a].text, ts[c].text)); } }
        }
    }
    for (t, bx) in ts.iter().zip(&inks) {
        let Some(bx) = bx else { continue };
        for (kind, _, (p, q)) in &gm.strokes {
            if kind == "guide" || kind == "grid" { continue; }
            // a tick label sits beside its own tick
            if t.kind == "axis" && kind == "axis" { continue; }
            if seg_hits(*p, *q, *bx) { bad.push(format!("{:?} on a {kind} stroke {p:?}-{q:?}", t.text)); }
        }
        for (feat, mb, _) in &gm.marks { if hits(*bx, *mb) { bad.push(format!("{:?} on the {feat} marker", t.text)); } }
    }
    // the axis texts against the axis lines: a tick label clears the axis it labels
    let axis_box: Vec<Seg> = gm.strokes.iter().filter(|s_| s_.0 == "axis").map(|s_| s_.2).collect();
    for (t, bx) in ts.iter().zip(&inks) {
        if t.kind != "axis" { continue; }
        let Some(bx) = bx else { continue };
        for (p, q) in &axis_box { if seg_hits(*p, *q, *bx) { bad.push(format!("axis text {:?} on an axis line", t.text)); } }
    }
    // key entries: near their own marker, nearer it than any other entry's
    let mut groups: BTreeMap<String, (String, Bx)> = BTreeMap::new();
    for (t, bx) in ts.iter().zip(&inks) {
        if t.kind != "label" { continue; }
        let Some(bx) = bx else { continue };
        let e = groups.entry(t.group.clone()).or_insert((t.feat.clone(), *bx));
        e.1 = (e.1 .0.min(bx.0), e.1 .1.max(bx.1), e.1 .2.min(bx.2), e.1 .3.max(bx.3));
    }
    let owned: Vec<String> = { let mut v: Vec<String> = groups.values().map(|g| g.0.clone()).collect(); v.sort(); v.dedup(); v };
    let dist = |feat: &str, bx: Bx| -> f64 { gm.marks.iter().filter(|m| m.0 == feat).map(|m| seg_box((m.1 .0, m.1 .2), (m.1 .1, m.1 .3), bx)).fold(f64::MAX, f64::min) };
    for (own, bx) in groups.values() {
        let d = dist(own, *bx);
        if d > 40.0 * lab { bad.push(format!("the {own} entry is {d:.1} px from its marker")); continue; }
        let other = owned.iter().filter(|o| *o != own).map(|o| dist(o, *bx)).fold(f64::MAX, f64::min);
        if d > 15.0 && other < d - 6.0 { bad.push(format!("the {own} entry is {d:.1} px from its marker but {other:.1} px from another")); }
    }
    // a marker on the plot sits on the curve
    if let Some(c) = gm.paths.iter().find(|p| p.0 == "curve") {
        let plot_marks: Vec<&(String, Bx, String)> = gm.marks.iter().filter(|m| m.1 .1 > 45.0 && m.1 .2 < c.2.iter().map(|p| p.1).fold(f64::MIN, f64::max) + 1.0 && m.0 != "reg" && m.0 != "ind").collect();
        for (feat, mb, _) in plot_marks {
            let ctr = ((mb.0 + mb.1) / 2.0, (mb.2 + mb.3) / 2.0);
            let near = c.2.windows(2).map(|s_| seg_box(s_[0], s_[1], (ctr.0, ctr.0, ctr.1, ctr.1))).fold(f64::MAX, f64::min);
            let in_key = ctr.0 < 30.0 && ctr.0 > 26.0;
            if !in_key && near > 1.5 { bad.push(format!("the {feat} marker is {near:.2} px off the curve")); }
        }
    }
    // a guide stops short of every other feature's marker (the dashed line of the volume asked about runs through its
    // own point)
    for (kind, feat, (p, q)) in &gm.strokes {
        if kind != "guide" { continue; }
        for (mf, mb, _) in &gm.marks {
            if mf == feat || (feat == "vq" && mf == "q") { continue; }
            if seg_hits(*p, *q, (mb.0 + 0.5, mb.1 - 0.5, mb.2 + 0.5, mb.3 - 0.5)) { bad.push(format!("a {feat} guide runs through the {mf} marker")); }
        }
    }
    if student {
        if !gm.marks.is_empty() || !gm.bands.is_empty() { bad.push("the student copy draws a marker or a band".into()); }
        for (kind, feat, _) in &gm.strokes { if kind == "guide" && feat != "vq" { bad.push(format!("the student copy draws the answer guide {feat}")); } }
    }
    bad
}

fn audit_cases(n: usize) -> Vec<(L, bool)> {
    let mut r = Rng(2024);
    let mut out = Vec::new();
    while out.len() < n {
        let ans = r.next() % 2 == 0;
        let l: L = vec![
            s("mode", r.pick(&MODES_ALL)),
            s("task", r.pick(&TASKS_ALL)),
            s("acid", ACID_CHOICES[(r.next() % 9) as usize]),
            s("base", BASE_CHOICES[(r.next() % 7) as usize]),
            f("pka", r.pick(&[1.0, 2.5, 4.76, 7.0, 9.1, 13.0])),
            f("pkb", r.pick(&[1.0, 3.3, 4.75, 8.8, 13.0])),
            s("strong_acid", STRONG_ACIDS[(r.next() % 4) as usize]),
            s("strong_base", STRONG_BASES[(r.next() % 2) as usize]),
            f("c_an", r.pick(&[0.0001, 0.001, 0.02, 0.1, 0.5, 2.0])),
            f("c_ti", r.pick(&[0.0001, 0.005, 0.1, 0.3, 2.0])),
            f("v_an", r.pick(&[1.0, 10.0, 25.0, 100.0, 250.0])),
            f("c_salt", r.pick(&[0.0, 0.001, 0.1, 2.0])),
            s("pair", PAIRS[(r.next() % 2) as usize]),
            s("add", ADDS[(r.next() % 2) as usize]),
            f("v_q", r.pick(&[0.0, 1.0, 12.5, 25.0, 60.0, 999.0])),
            f("v_max", r.pick(&[0.0, 0.0, 0.0, 10.0, 50.0, 1000.0])),
            s("ind_set", IND_SETS[(r.next() % 2) as usize]),
            i("dec", 1 + (r.next() % 3) as i64),
            b("region", r.next() % 4 != 0),
            b("grid", r.next() % 4 != 0),
            s("label_size", r.pick(&["Small", "Medium", "Large"])),
            s("style", r.pick(&["Colour", "Print"])),
            b(SHOW_ANSWERS, ans),
        ];
        out.push((l, ans));
    }
    out
}

#[test]
fn the_label_audit_finds_nothing_over_twelve_thousand_cases() {
    let cases = audit_cases(12_000);
    let chunk = cases.len().div_ceil(4);
    let handles: Vec<_> = cases
        .chunks(chunk)
        .map(|c| c.to_vec())
        .map(|c| std::thread::spawn(move || {
            let mut g = Glyphs::new();
            let mut bad: Vec<String> = Vec::new();
            for (l, ans) in &c {
                let p = with(l);
                let svg = def().svg(&p);
                let lab = svg::label_factor(p.s("label_size"));
                for x in audit(&svg, &mut g, lab, !ans) { bad.push(format!("{x}\n  in {}", show(l))); if bad.len() > 30 { return bad; } }
            }
            bad
        }))
        .collect();
    let mut bad: Vec<String> = Vec::new();
    for h in handles { bad.extend(h.join().unwrap()); }
    assert!(bad.is_empty(), "{} problems:\n{}", bad.len(), bad.iter().take(25).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn the_audit_catches_what_it_is_meant_to_catch() {
    let mut g = Glyphs::new();
    let svg = svg_of(&[s("mode", WASB), s("task", T_PKA)]);
    assert!(audit(&svg, &mut g, 1.0, false).is_empty(), "{:?}", audit(&svg, &mut g, 1.0, false));
    let add = |x: &str| svg.replace("</svg>", &format!("{x}</svg>"));
    // text off the canvas
    assert!(audit(&add(&format!(r##"<text x="5" y="-4" font-size="12" font-family="{FONT}" data-o="note">Gone</text>"##)), &mut g, 1.0, false).iter().any(|p| p.contains("leaves the")));
    // text on text
    let t0 = texts(&svg).into_iter().find(|t| t.kind == "note").unwrap();
    assert!(audit(&add(&format!(r##"<text x="{}" y="{}" font-size="{}" font-family="{FONT}" data-o="note">{}</text>"##, t0.x + 3.0, t0.y + 2.0, t0.fs, esc(&t0.text))), &mut g, 1.0, false).iter().any(|p| p.contains(" on ")));
    // a text on the curve
    let c = curve_of(&svg).unwrap();
    let mid = c[c.len() / 3];
    assert!(audit(&add(&format!(r##"<text x="{}" y="{}" font-size="12" text-anchor="middle" font-family="{FONT}" data-o="note">OnCurve</text>"##, mid.0, mid.1 + 4.0)), &mut g, 1.0, false).iter().any(|p| p.contains("curve stroke")));
    // a text on a marker
    let mk = geo(&svg).marks;
    let bx = mk.iter().find(|m| m.1 .0 > 60.0).unwrap().1;
    assert!(audit(&add(&format!(r##"<text x="{}" y="{}" font-size="12" text-anchor="middle" font-family="{FONT}" data-o="note">M</text>"##, (bx.0 + bx.1) / 2.0, (bx.2 + bx.3) / 2.0 + 4.0)), &mut g, 1.0, false).iter().any(|p| p.contains("marker")));
    // a key entry far from its marker
    assert!(audit(&add(&format!(r##"<text x="300" y="20" font-size="12" font-family="{FONT}" data-o="label" data-f="eq" data-g="9">Far away</text>"##)), &mut g, 1.0, false).iter().any(|p| p.contains("px from its marker")));
    // a marker off the curve
    assert!(audit(&add(r##"<circle cx="300" cy="60" r="4" data-o="mark" data-f="zz"/>"##), &mut g, 1.0, false).iter().any(|p| p.contains("off the curve")));
    // a guide through another feature's marker
    let eqb = mk.iter().find(|m| m.0 == "eq" && m.1 .0 > 60.0).unwrap().1;
    assert!(audit(&add(&format!(r##"<path d="M{},{} L{},{}" data-o="guide" data-f="half"/>"##, eqb.0 - 20.0, (eqb.2 + eqb.3) / 2.0, eqb.1 + 20.0, (eqb.2 + eqb.3) / 2.0)), &mut g, 1.0, false).iter().any(|p| p.contains("runs through")));
    // the student copy with an answer feature
    assert!(audit(&add(r##"<path d="M10,10 L40,10" data-o="guide" data-f="eq"/>"##), &mut g, 1.0, true).iter().any(|p| p.contains("answer guide")));
}

#[test]
fn large_labels_never_clip() {
    let mut g = Glyphs::new();
    for l in student_cases() {
        for ans in [true, false] {
            let mut l2 = l.clone();
            l2.extend([s("label_size", "Large"), s("strong_acid", "HClO4"), b(SHOW_ANSWERS, ans)]);
            let svg = svg_of(&l2);
            let (w, h) = crate::figures::svg_size(&svg);
            for t in texts(&svg) {
                let Some(bx) = g.ink(&t) else { continue };
                assert!(bx.0 >= 0.0 && bx.1 <= w && bx.2 >= 0.0 && bx.3 <= h, "{:?} clips: {bx:?} in {w}x{h} for {}", t.text, show(&l2));
            }
        }
    }
}

/// Every character the figure draws is in the figure font (Noto Sans, bundled). A character that is not there is drawn
/// from another font, which changes the look and the width of the line. The super- and subscripts of formulas and
/// charges fall back, as in the sibling chemistry figures, and look right.
#[test]
fn every_drawn_character_is_in_the_figure_font() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
    let bytes = std::fs::read(&path).expect("the figure font is bundled");
    let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
    let mut cases = student_cases();
    cases.extend(audit_cases(600).into_iter().map(|x| x.0));
    for (k, _) in ACID_CHOICES.iter().enumerate() { cases.push(vec![s("mode", BUF), s("acid", ACID_CHOICES[k])]); }
    for (k, _) in BASE_CHOICES.iter().enumerate() { cases.push(vec![s("mode", BUF), s("pair", PAIRS[1]), s("base", BASE_CHOICES[k])]); }
    let mut missing = std::collections::BTreeSet::new();
    for l in cases {
        for ans in [true, false] {
            let mut ls = l.clone();
            ls.push(b(SHOW_ANSWERS, ans));
            let p = with(&ls);
            let mut all: Vec<String> = texts(&def().svg(&p)).into_iter().map(|t| t.text).collect();
            all.extend(warnings(&p));
            for t in all { for c in t.chars() { if c != ' ' && face.glyph_index(c).is_none() { missing.insert(c); } } }
        }
    }
    let allowed = |c: char| ('\u{2070}'..='\u{209f}').contains(&c) || c == '\u{b2}' || c == '\u{b3}' || c == '\u{b9}';
    let bad: String = missing.iter().filter(|&&c| !allowed(c)).collect();
    assert!(bad.is_empty(), "characters that are not in the figure font and not known to fall back well: {bad}");
}

#[test]
fn notes_mention_the_approximations() {
    let key = note_text(&svg_of(&[]));
    assert!(key.contains("concentrations for activities") && key.contains("25 \u{b0}C") && key.contains("CRC Handbook"), "{key}");
    let stu = note_text(&svg_of(&[b(SHOW_ANSWERS, false)]));
    assert!(!stu.contains("charge balance"), "{stu}");
}

// ---------------------------------------------------------------------------------------------
// Critic round 1
// ---------------------------------------------------------------------------------------------

/// A number as the key prints it: "1.234 × 10⁻⁵", "−0.02", "25.00".
fn num_t(t: &str) -> f64 {
    let t = t.trim().trim_end_matches('.').replace('\u{2212}', "-");
    if let Some((m, e)) = t.split_once(" \u{d7} 10") {
        let e: String = e.chars().map(|c| match c { '\u{207b}' => '-', '\u{2070}' => '0', '\u{b9}' => '1', '\u{b2}' => '2', '\u{b3}' => '3', '\u{2074}' => '4', '\u{2075}' => '5', '\u{2076}' => '6', '\u{2077}' => '7', '\u{2078}' => '8', '\u{2079}' => '9', c => c }).collect();
        return m.trim().parse::<f64>().unwrap_or_else(|_| panic!("not a mantissa {m:?}")) * 10f64.powi(e.trim().parse::<i32>().unwrap_or_else(|_| panic!("not a power {e:?}")));
    }
    t.parse().unwrap_or_else(|_| panic!("not a number {t:?}"))
}

/// The significant figures a printed number shows ("0.05000" has 4, "2.5" has 2).
fn sig_t(t: &str) -> usize {
    let m = t.trim().trim_end_matches('.').split(" \u{d7} ").next().unwrap().replace(['\u{2212}', '-', '.'], "");
    let d = m.trim_start_matches('0');
    if d.is_empty() { 1 } else { d.len() }
}

/// The factors of "a × b" where either may be written "m × 10ⁿ".
fn factors(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in s.split(" \u{d7} ") {
        let pow = p.starts_with("10") && p.chars().nth(2).is_some_and(|c| "\u{207b}\u{2070}\u{b9}\u{b2}\u{b3}\u{2074}\u{2075}\u{2076}\u{2077}\u{2078}\u{2079}".contains(c));
        match out.last_mut() { Some(l) if pow => { l.push_str(" \u{d7} "); l.push_str(p); } _ => out.push(p.to_string()) }
    }
    out
}

/// `v` printed to the significant figures `printed` shows gives `printed` (an integer's trailing zeros may or may
/// not be significant, so either reading counts).
fn rounds_to(v: f64, printed: &str) -> bool {
    let full = sig_t(printed);
    let m = printed.split(" \u{d7} ").next().unwrap();
    let tz = if m.contains('.') { 0 } else { m.len() - m.trim_end_matches('0').len() };
    (full.saturating_sub(tz).max(1)..=full).any(|n| sf(v, n) == printed)
}

fn between<'a>(s: &'a str, a: &str, b: &str) -> Option<&'a str> { let i = s.find(a)? + a.len(); let j = s[i..].find(b)? + i; Some(&s[i..j]) }

/// Every line of working whose arithmetic the key prints is checked: recomputing from the printed numbers gives the
/// printed result. Returns the kinds of line checked.
fn check_arithmetic(lines: &[String], dec: usize, ctx: &str) -> Vec<&'static str> {
    let mut kinds = Vec::new();
    for l in lines {
        if let Some(tail) = l.split_once(")) = ").map(|x| x.1).filter(|_| l.starts_with("Henderson-Hasselbalch")) {
            let (pk, rest) = tail.split_once(" + log(").unwrap();
            let (ratio, r) = rest.split_once(") = ").unwrap();
            let (b_, a_) = ratio.split_once(" / ").unwrap();
            assert_eq!(fx(num_t(pk) + (num_t(b_) / num_t(a_)).log10(), dec), r.trim_end_matches('.'), "{l}\n{ctx}");
            kinds.push("hh");
        }
        if l.starts_with("[H\u{207a}] = ") && l.contains("so pH = \u{2212}log ") {
            let a_ = between(l, "] = ", " mmol").unwrap();
            let b_ = between(l, "mmol / ", " mL").unwrap();
            let c_ = between(l, "mL = ", " M,").unwrap();
            let r = l.rsplit_once(" = ").unwrap().1.trim_end_matches('.');
            assert!(rounds_to(num_t(a_) / num_t(b_), c_), "{l}\n{ctx}");
            assert_eq!(fx(-num_t(c_).log10(), dec), r, "{l}\n{ctx}");
            kinds.push("h");
        }
        if l.starts_with("[OH\u{207b}] = ") {
            let a_ = between(l, "] = ", " mmol").unwrap();
            let b_ = between(l, "mmol / ", " mL").unwrap();
            let c_ = between(l, "mL = ", " M,").unwrap();
            let p = between(l, "pOH = ", " and").unwrap();
            assert!(rounds_to(num_t(a_) / num_t(b_), c_), "{l}\n{ctx}");
            assert_eq!(fx(-num_t(c_).log10(), dec), p, "{l}\n{ctx}");
            if l.contains(&format!("{} \u{2212} {p} = ", fx(PKW, dec))) { assert_eq!(fx(PKW - num_t(p), dec), l.rsplit_once(" = ").unwrap().1.trim_end_matches('.'), "{l}"); }
            kinds.push("oh");
        }
        if l.contains("alone is a weak acid with Ka = ") {
            let inside = between(l, "C) = sqrt(", ")").unwrap();
            let fs_ = factors(inside);
            let (k, c_) = (fs_[0].as_str(), fs_[1].as_str());
            let h_ = between(l, &format!("sqrt({inside}) = "), " M").unwrap();
            let r = l.rsplit_once("pH = ").unwrap().1.trim_end_matches('.');
            assert!(rounds_to((num_t(k) * num_t(c_)).sqrt(), h_), "{l}\n{ctx}");
            assert_eq!(fx(-num_t(h_).log10(), dec), r, "{l}\n{ctx}");
            kinds.push("sqrta");
        }
        if l.contains("alone is a weak base with Kb = ") {
            let inside = between(l, "C) = sqrt(", ")").unwrap();
            let fs_ = factors(inside);
            let (k, c_) = (fs_[0].as_str(), fs_[1].as_str());
            let o_ = between(l, &format!("sqrt({inside}) = "), " M").unwrap();
            let p = between(l, "pOH = ", " and").unwrap();
            assert!(rounds_to((num_t(k) * num_t(c_)).sqrt(), o_), "{l}\n{ctx}");
            assert_eq!(fx(-num_t(o_).log10(), dec), p, "{l}\n{ctx}");
            kinds.push("sqrtb");
        }
        if l.starts_with("Equivalence: n(") {
            let c_ = between(l, ") = ", " M \u{d7}").map(|x| x.rsplit(") = ").next().unwrap()).unwrap();
            let v_ = between(l, "M \u{d7} ", " mL").unwrap();
            let n_ = between(l, " mL = ", " mmol").unwrap();
            let ct = between(l, &format!("V = {n_} mmol / "), " M").unwrap();
            let ve = between(l, &format!("/ {ct} M = "), " mL").unwrap();
            assert!(rounds_to(num_t(c_) * num_t(v_), n_), "{l}\n{ctx}");
            assert!(rounds_to(num_t(n_) / num_t(ct), ve), "{l}\n{ctx}");
            kinds.push("eq");
        }
        if l.starts_with("Half-equivalence: V = ") {
            let ve = between(l, "V = ", " mL / 2").unwrap();
            let vh = between(l, "/ 2 = ", " mL").unwrap();
            assert!(rounds_to(num_t(ve) / 2.0, vh), "{l}\n{ctx}");
            kinds.push("half");
        }
        if let Some(x_) = between(l, "x = (10 \u{d7} ", " mmol") {
            let (a_, rest) = x_.split_once(" \u{2212} ").unwrap();
            let (b_, x) = rest.split_once(") / 11 = ").unwrap();
            assert!(rounds_to((10.0 * num_t(a_) - num_t(b_)) / 11.0, x), "{l}\n{ctx}");
            let ct = between(l, &format!("which is {x} mmol / "), " M").unwrap();
            let v = between(l, &format!("/ {ct} M = "), " mL").unwrap();
            assert!(rounds_to(num_t(x) / num_t(ct), v), "{l}\n{ctx}");
            kinds.push("cap");
        }
    }
    kinds
}

#[test]
fn the_printed_arithmetic_reproduces_every_printed_result() {
    // the critic's repro: a custom pKb with three decimals, three decimals of pH
    let l: L = vec![s("mode", BUF), s("pair", PAIRS[1]), s("base", "Custom"), f("pkb", 3.337), i("dec", 3), f("c_salt", 0.1), s("task", T_BUF)];
    let t = readout(&mdl(&l), true).join("\n");
    assert!(t.contains("= 10.663 + log(2.500 / 2.500) = 10.663."), "{t}");
    let mut r = Rng(404);
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for _ in 0..1500 {
        let l: L = vec![
            s("mode", r.pick(&MODES_ALL)),
            s("task", r.pick(&TASKS_ALL)),
            s("acid", "Custom"), s("base", "Custom"),
            f("pka", r.pick(&[1.234, 2.718, 3.337, 4.756, 6.6, 7.405, 9.999, 12.37])),
            f("pkb", r.pick(&[1.234, 3.337, 3.999, 4.75, 8.765, 11.11])),
            f("c_an", r.pick(&[0.0001, 0.001234, 0.05, 0.1, 0.1234, 0.987, 2.0])),
            f("c_ti", r.pick(&[0.0003, 0.002, 0.1, 0.1111, 0.75])),
            f("v_an", r.pick(&[10.0, 25.0, 25.37, 33.33, 100.0])),
            f("c_salt", r.pick(&[0.0, 0.001, 0.05, 0.1234])),
            s("pair", PAIRS[(r.next() % 2) as usize]),
            s("add", ADDS[(r.next() % 2) as usize]),
            f("v_q", r.pick(&[0.0, 1.234, 10.0, 12.5, 24.99, 25.0, 33.33, 60.0])),
            i("dec", 1 + (r.next() % 3) as i64),
        ];
        let m = mdl(&l);
        for k in check_arithmetic(&readout(&m, true), m.st.dec, &show(&l)) { *seen.entry(k).or_default() += 1; }
    }
    for k in ["hh", "h", "oh", "sqrta", "sqrtb", "eq", "half", "cap"] { assert!(seen.get(k).copied().unwrap_or(0) >= 20, "{k}: {seen:?}"); }
}

#[test]
fn agreement_is_claimed_only_when_the_rounded_values_agree() {
    // the critic's repro: pKb 3.999 at 2 M, three decimals: the pKa of BH+ is 10.001, printed in full
    let t = readout(&mdl(&[s("mode", WBSA), s("task", T_PKA), s("base", "Custom"), f("pkb", 3.999), f("c_an", 2.0), f("c_ti", 2.0), i("dec", 3)]), true).join("\n");
    assert!(t.contains("= 10.001;"), "{t}");
    let mut r = Rng(17);
    let mut agree = 0;
    for _ in 0..600 {
        let base = r.next() % 2 == 0;
        let l: L = vec![s("mode", if base { WBSA } else { WASB }), s("task", T_PKA), s("acid", "Custom"), s("base", "Custom"), f("pka", r.pick(&[1.5, 3.2, 4.756, 6.123, 9.21, 12.5])), f("pkb", r.pick(&[1.5, 3.999, 4.75, 8.77, 12.0])), f("c_an", r.pick(&[0.0001, 0.001, 0.1, 2.0])), f("c_ti", r.pick(&[0.0001, 0.1, 2.0])), i("dec", 1 + (r.next() % 3) as i64)];
        let m = mdl(&l);
        let t = readout(&m, true).join("\n");
        let read = fx(m.ph_half, m.st.dec);
        let truth_txt = between(&t, if base { "the pKa of " } else { "gives pKa = " }, ";").map(|x| x.rsplit(" = ").next().unwrap().to_string()).unwrap_or_else(|| panic!("{t}"));
        assert!((num_t(&truth_txt) - m.st.pka()).abs() < 1e-9, "printed {truth_txt} for pKa {} in {}", m.st.pka(), show(&l));
        let agrees = t.contains("the reading agrees");
        assert_eq!(agrees, fx(m.st.pka(), m.st.dec) == read, "{t}");
        if agrees { agree += 1; }
    }
    assert!(agree > 50, "{agree}");
}

/// The words for what moves a half-equivalence (or ratio) reading off the shortcut: (higher, lower) for an acid pair,
/// then for a base pair.
const DRIFT: [(&str, &str); 2] = [("ionises, adding", "reacts with water to give back"), ("ionises to give back", "reacts with water, adding")];

#[test]
fn the_pka_reading_names_the_reaction_that_moves_it() {
    // the critic's repros
    let t = readout(&mdl(&[s("mode", WASB), s("task", T_PKA), s("acid", "Hydrocyanic acid"), f("c_an", 0.001), f("c_ti", 0.001)]), true).join("\n");
    assert!(t.contains("lower") && t.contains("some of the CN\u{207b} reacts with water to give back HCN"), "{t}");
    let t = readout(&mdl(&[s("mode", WBSA), s("task", T_PKA), s("base", "Aniline"), f("c_an", 0.001), f("c_ti", 0.001)]), true).join("\n");
    assert!(t.contains("higher") && t.contains("ionises to give back C\u{2086}H\u{2085}NH\u{2082}"), "{t}");
    let mut r = Rng(69);
    let mut checked = [0usize; 4];
    for _ in 0..800 {
        let base = r.next() % 2 == 0;
        let pk = 1.0 + (r.next() % 1200) as f64 / 100.0;
        let l: L = vec![s("mode", if base { WBSA } else { WASB }), s("task", T_PKA), s("acid", "Custom"), s("base", "Custom"), f("pka", pk), f("pkb", pk), f("c_an", r.pick(&[0.0001, 0.001, 0.01, 0.1, 1.0])), f("c_ti", r.pick(&[0.0001, 0.001, 0.1])), i("dec", 1 + (r.next() % 3) as i64)];
        let m = mdl(&l);
        let t = readout(&m, true).join("\n");
        if fx(m.st.pka(), m.st.dec) == fx(m.ph_half, m.st.dec) { assert!(t.contains("agrees"), "{t}"); continue; }
        // the cause, from the independent pH at half-equivalence: [H+] above [OH-] means the acid form ionised
        let ph = ind_ph(&m.st.sys, m.st.pka(), m.vhalf);
        let h_over = 10f64.powf(-ph) > 10f64.powf(ph - PKW);
        let (up, down) = DRIFT[base as usize];
        let (want, not) = if h_over { (up, down) } else { (down, up) };
        assert!(t.contains(want) && !t.contains(not), "want {want:?} for {}\n{t}", show(&l));
        // and the direction word matches the numbers
        let higher = m.ph_half > m.st.pka();
        assert_eq!(higher, h_over, "{}", show(&l));
        assert!(t.contains(if higher { " higher" } else { " lower" }), "{t}");
        checked[base as usize * 2 + higher as usize] += 1;
    }
    assert!(checked.iter().all(|c| *c >= 10), "{checked:?}");
}

#[test]
fn a_reading_far_from_pka_is_warned_and_a_tiny_equivalence_volume_noted() {
    let n = warnings(&with(&[s("mode", WASB), s("task", T_PKA), s("acid", "Custom"), f("pka", 13.0), f("c_an", 0.0001), f("c_ti", 0.0001)]));
    assert!(n.iter().any(|x| x.contains("half-equivalence") && x.contains("not the pKa")), "{n:?}");
    let n = warnings(&with(&[s("mode", WASB), s("task", T_PKA)]));
    assert!(!n.iter().any(|x| x.contains("not the pKa")), "{n:?}");
    let n = warnings(&with(&[s("mode", SASB), f("c_an", 0.0001), f("v_an", 1.0), f("c_ti", 2.0)]));
    assert!(n.iter().any(|x| x.contains("only 5.000 \u{d7} 10\u{207b}\u{2075} mL")), "{n:?}");
}

#[test]
fn the_no_indicator_reason_is_true() {
    // the critic's repro: the jump runs from 8.29 to 11.00, wide enough; no listed indicator fits in it
    let t = readout(&mdl(&[s("mode", WASB), s("acid", "Custom"), f("pka", 6.6), s("task", T_IND)]), true).join("\n");
    assert!(!t.contains("too small") && !t.contains("narrower") && t.contains("no listed indicator"), "{t}");
    let mut r = Rng(136);
    let (mut narrow, mut wide) = (0, 0);
    for _ in 0..1500 {
        let l: L = vec![s("mode", r.pick(&[SASB, SBSA, WASB, WBSA])), s("task", T_IND), s("acid", "Custom"), s("base", "Custom"), f("pka", 1.0 + (r.next() % 1200) as f64 / 100.0), f("pkb", 1.0 + (r.next() % 1200) as f64 / 100.0), f("c_an", r.pick(&[0.0001, 0.001, 0.01, 0.1])), f("c_ti", r.pick(&[0.0001, 0.001, 0.01, 0.1])), s("ind_set", IND_SETS[(r.next() % 2) as usize])];
        let m = mdl(&l);
        if m.best.is_some() { continue; }
        let t = readout(&m, true).join("\n");
        let width = m.window.1 - m.window.0;
        let min_w = m.offered.iter().map(|&k| INDICATORS[k].hi - INDICATORS[k].lo).fold(f64::MAX, f64::min);
        if width < min_w { assert!(t.contains("narrower than"), "{t}"); narrow += 1; } else { assert!(!t.contains("narrower") && !t.contains("too small") && t.contains("no listed indicator"), "{t}"); wide += 1; }
        // the jump printed is the one worked out
        assert!(t.contains(&format!("from pH {} to {}", fx(m.window.0, m.st.dec), fx(m.window.1, m.st.dec))), "{t}");
    }
    assert!(narrow >= 20 && wide >= 10, "{narrow} {wide}");
}

#[test]
fn the_capacity_is_worked_from_the_amounts_and_set_beside_the_exact_curve() {
    // the critic's repro: HF buffer, HCl added
    let l: L = vec![s("mode", BUF), s("acid", "Hydrofluoric acid"), s("add", ADDS[1]), f("c_salt", 0.1), s("task", T_BUF)];
    let t = readout(&mdl(&l), true).join("\n");
    assert!(t.contains("20.45 mL") && t.contains("23.52 mL"), "{t}");
    assert!(t.contains("the answer expected"), "{t}");
    let stu = all_text(&svg_of(&[l.clone(), vec![b(SHOW_ANSWERS, false)]].concat()));
    assert!(stu.contains("past 1/10") && stu.contains("none") && !stu.contains("Henderson"), "{stu}");
    let mut r = Rng(88);
    let (mut some, mut none, mut causes) = (0, 0, 0);
    for _ in 0..600 {
        let l: L = vec![s("mode", BUF), s("task", T_BUF), s("pair", PAIRS[(r.next() % 2) as usize]), s("add", ADDS[(r.next() % 2) as usize]), s("acid", ACID_CHOICES[(r.next() % 9) as usize]), s("base", BASE_CHOICES[(r.next() % 7) as usize]), f("c_an", r.pick(&[0.001, 0.01, 0.1, 0.5])), f("c_salt", r.pick(&[0.0, 0.002, 0.01, 0.1, 0.5])), f("c_ti", r.pick(&[0.001, 0.1, 1.0])), f("v_an", 25.0)];
        let m = mdl(&l);
        let sys = m.st.sys;
        let (a0, b0) = if sys.base_pair { (sys.c_s * sys.v_a, sys.c_a * sys.v_a) } else { (sys.c_a * sys.v_a, sys.c_s * sys.v_a) };
        let x = if sys.add_base { (10.0 * a0 - b0) / 11.0 } else { (10.0 * b0 - a0) / 11.0 };
        let t = readout(&m, true).join("\n");
        if x > 0.0 {
            let v = x / sys.c_t;
            assert!(t.contains(&format!("= {} mL. This is the answer expected", s4(v))), "{} mL in {t}", s4(v));
            some += 1;
            if let Out::At(xe) = m.out {
                if s4(xe) != s4(v) {
                    let ph = ind_ph(&sys, m.st.pka(), v);
                    let h_over = 10f64.powf(-ph) > 10f64.powf(ph - PKW);
                    let (up, down) = DRIFT[sys.base_pair as usize];
                    assert!(t.contains(if h_over { up } else { down }), "{t}");
                    causes += 1;
                }
            }
        } else {
            assert!(t.contains("the answer is none"), "{t}");
            none += 1;
        }
    }
    assert!(some > 100 && none > 20 && causes > 50, "{some} {none} {causes}");
    // the FAQ says the same
    let faq: String = crate::figures::faq::faq_for(ID).iter().map(|q| q.a).collect();
    assert!(faq.contains("from the amounts") && faq.contains("exact curve"), "{faq}");
}

#[test]
fn a_solution_with_no_salt_is_not_called_a_buffer() {
    for add in ADDS {
        for pair in PAIRS {
            for ans in [true, false] {
                let svg = svg_of(&[s("mode", BUF), s("task", T_BUF), f("c_salt", 0.0), s("add", add), s("pair", pair), b(SHOW_ANSWERS, ans)]);
                let t = all_text(&svg).replace('\n', " ");
                assert!(!t.contains("of a buffer") && !t.contains("The buffer alone") && !t.contains("pH of the buffer") && !desc_of(&svg).contains("to a buffer"), "{t}\n{}", desc_of(&svg));
                assert!(t.contains("not a buffer"), "{t}");
            }
        }
    }
}

#[test]
fn the_line_at_the_volume_asked_about_is_called_dashed_everywhere() {
    let svg = svg_of(&[s("mode", WASB), s("task", T_PH), b(SHOW_ANSWERS, false)]);
    assert!(all_text(&svg).contains("dashed line") && desc_of(&svg).contains("dashed line"), "{}", desc_of(&svg));
    let hints: String = (def().params)().iter().map(|p| p.hint).collect();
    let faq: String = crate::figures::faq::faq_for(ID).iter().map(|q| q.a).collect();
    let p = with(&[s("mode", WASB), s("task", T_PH)]);
    let tips: String = handles(ID, &p, (800.0, 800.0)).unwrap().iter().map(|h| h.tip.clone()).collect();
    for (what, t) in [("hints", hints), ("faq", faq), ("tips", tips.clone()), ("svg", all_text(&svg))] { assert!(!t.contains("dotted line"), "{what}: {t}"); }
    assert!(tips.contains("dashed line"));
}

#[test]
fn wrapped_lines_never_split_a_word_from_its_value() {
    let mut cases = student_cases();
    cases.extend(audit_cases(500).into_iter().map(|x| x.0));
    let mut n = 0;
    for l in cases {
        for ans in [true, false] {
            let mut l2 = l.clone();
            l2.retain(|x| x.0 != SHOW_ANSWERS);
            l2.push(b(SHOW_ANSWERS, ans));
            let ts: Vec<T> = texts(&svg_of(&l2)).into_iter().filter(|t| t.kind == "note" || t.kind == "label").collect();
            for (k, t) in ts.iter().enumerate() {
                let x = t.text.trim_end();
                for end in [" =", " to", "(", " \u{d7}", " \u{b1}"] { assert!(!x.ends_with(end), "a line ends {end:?}: {x:?} in {}", show(&l2)); }
                for start in ["mL", "mmol", "to ", "M ", "M.", "\u{d7}"] { assert!(!x.starts_with(start), "a line starts {start:?}: {x:?} in {}", show(&l2)); }
                // a quantity's name and its value stay on one line
                if let Some(next) = ts.get(k + 1) {
                    let starts_value = next.text.starts_with(|c: char| c.is_ascii_digit() || c == '=' || c == '\u{2212}');
                    for w in ["pKa", "pKb", "pH", "pOH", "Ka", "Kb", "V"] { assert!(!(starts_value && (x.ends_with(&format!(" {w}")) || x == w)), "{w:?} and its value split: {x:?} / {:?} in {}", next.text, show(&l2)); }
                }
                n += 1;
            }
        }
    }
    assert!(n > 5000, "{n}");
}

#[test]
fn tiny_concentrations_survive_the_editor_snap() {
    let defs = (def().params)();
    for v in [0.0001, 0.0002, 0.0003, 0.0004] {
        let mut p = Params::defaults(&defs);
        p.set_f("c_ti", v);
        p.normalize(&defs, true);
        let c = model_of(&p).st.sys.c_t;
        assert!((c - v).abs() < 1e-12, "{v} became {c}");
    }
    // 0 is held at the least the figure takes, with a note, never the default 0.1 M
    let p = raw(&[f("c_ti", 0.0)]);
    assert_eq!(model_of(&p).st.sys.c_t, C_LO);
    assert!(warnings(&p).iter().any(|x| x.contains("must be greater than 0") && x.contains("0.0001")), "{:?}", warnings(&p));
}

#[test]
fn the_half_equivalence_handle_never_renames_a_named_acid() {
    for task in TASKS_ALL {
        let p = with(&[s("mode", WASB), s("task", task), s("acid", "Benzoic acid")]);
        let hs = handles(ID, &p, (800.0, 800.0)).unwrap();
        assert!(!hs.iter().any(|h| h.tip.contains("half-equivalence")), "{task}");
    }
    let p = with(&[s("mode", WBSA), s("task", T_EQ), s("base", "Custom"), f("pkb", 4.0)]);
    let hs = handles(ID, &p, (800.0, 800.0)).unwrap();
    let h = hs.iter().find(|h| h.tip.contains("half-equivalence")).expect("a custom base can be dragged");
    let mut q = p.clone();
    (h.drag.clone().unwrap())(&mut q, (h.pos.0, h.pos.1 + 30.0));
    assert_eq!(q.s("base"), "Custom");
    assert!(q.f("pkb") > 4.0, "{}", q.f("pkb"));
}

#[test]
fn an_equivalence_ph_of_seven_to_the_decimals_shown_is_called_neutral() {
    let m = mdl(&[s("mode", WASB), s("task", T_EQ), s("acid", "Custom"), f("pka", 1.0), f("c_an", 0.0001), f("c_ti", 0.0001), i("dec", 3)]);
    assert_eq!(fx(m.ph_eq, 3), "7.000");
    let t = readout(&m, true).join("\n");
    assert!(!t.contains("so it is basic") && t.contains("neutral"), "{t}");
    // a clearly basic one is still basic
    let t = readout(&mdl(&[s("mode", WASB), s("task", T_EQ)]), true).join("\n");
    assert!(t.contains("so it is basic"), "{t}");
}

#[test]
fn indicator_colours_are_the_crc_ones_and_turn_the_right_way() {
    // CRC "Acid-Base Indicators" colour change, acid colour then base colour
    let c: Vec<(&str, &str, &str)> = INDICATORS.iter().map(|x| (x.name, x.acid_col, x.base_col)).collect();
    assert_eq!(c, vec![("methyl orange", "red", "yellow"), ("bromocresol green", "yellow", "blue"), ("methyl red", "red", "yellow"), ("bromothymol blue", "yellow", "blue"), ("phenol red", "yellow", "red"), ("phenolphthalein", "colourless", "pink"), ("thymolphthalein", "colourless", "blue"), ("alizarin yellow R", "yellow", "red")]);
    // a base added: acid colour to base colour; an acid added: the other way
    for (mode, want) in [(WASB, "turns from colourless to pink"), (SASB, "turns from yellow to blue"), (WBSA, "turns from yellow to red"), (SBSA, "turns from blue to yellow")] {
        let t = readout(&mdl(&[s("mode", mode), s("task", T_IND)]), true).join("\n");
        assert!(t.contains(want), "{mode}: {t}");
    }
}

#[test]
fn the_volume_axis_is_the_same_for_twenty_five_hidden_pkas() {
    // 0.1 M, 25 mL, 0.1 M: twice the equivalence volume is exactly 50, where a drift of the axis upward would show
    let pks: Vec<f64> = (0..25).map(|k| 1.0 + 0.5 * k as f64).collect();
    let mut compared = 0;
    for mode in [WASB, WBSA, BUF] {
        for task in TASKS_ALL {
            let frame = |pk: f64| {
                let svg = svg_of(&[s("mode", mode), s("task", task), s("acid", "Custom"), f("pka", pk), s("base", "Custom"), f("pkb", pk), b(SHOW_ANSWERS, false)]);
                let mut v: Vec<String> = texts(&svg).into_iter().filter(|t| t.kind == "axis").map(|t| format!("{:.3},{:.3},{}", t.x, t.y, t.text)).collect();
                for part in svg.split('<').filter(|x| x.contains("data-o=\"grid\"") || x.contains("data-o=\"axis\"") || x.contains("data-f=\"vq\"")) { v.push(part.split('>').next().unwrap().to_string()); }
                v.push(format!("{}", crate::figures::svg_size(&svg).0));
                v
            };
            let first = frame(pks[0]);
            for &pk in &pks[1..] { assert_eq!(frame(pk), first, "{mode} {task} pKa {pk}"); compared += 1; }
            // and it is the axis the givens make: twice the 25 mL equivalence volume is 50, a round number, so the ticks
            // run 0 to 50 by 10 (a drift that moved every pKa alike would still show here)
            for &pk in &[pks[0], pks[12], pks[24]] {
                let svg = svg_of(&[s("mode", mode), s("task", task), s("acid", "Custom"), f("pka", pk), s("base", "Custom"), f("pkb", pk), b(SHOW_ANSWERS, false)]);
                let xt: Vec<String> = texts(&svg).into_iter().filter(|t| t.kind == "axis" && t.anchor == "middle" && t.rot.is_none() && t.text.starts_with(|c: char| c.is_ascii_digit())).map(|t| t.text).collect();
                assert_eq!(xt, vec!["0", "10", "20", "30", "40", "50"], "{mode} {task} pKa {pk}");
            }
        }
    }
    assert!(compared >= 300, "{compared}");
}
