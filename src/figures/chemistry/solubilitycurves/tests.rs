//! Tests of the solubility curves. The model tests keep their own copy of the published tables and their own
//! interpolation (a Bezier-form cubic Hermite with the Fritsch-Butland slopes), and recompute every printed value from
//! the printed givens; the label audit measures every text with the glyphs the PNG renderer draws (usvg's own layout).
use super::*;
use crate::figures::{registry, to_png};
use std::collections::{BTreeMap, BTreeSet, HashMap};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("solubility-curves is registered") }
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
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
fn svg_of(list: &[(&'static str, Value)]) -> String { def().svg(&with(list)) }
fn rep_of(list: &[(&'static str, Value)]) -> Report { build(&with(list)).1 }
fn key(list: &[(&'static str, Value)]) -> Report { let mut l = list.to_vec(); l.push(b(SHOW_ANSWERS, true)); rep_of(&l) }
fn notes_of(list: &[(&'static str, Value)]) -> Vec<String> { warnings(&with(list)) }
fn has(notes: &[String], what: &str) -> bool { notes.iter().any(|n| n.contains(what)) }
fn layout_only(n: &[String]) -> bool { n.iter().all(|x| x.contains("could not be placed") || x.contains("lie almost on the axis")) }

const READ: &str = "Read a solubility";
const CLASSIFY: &str = "Saturated, unsaturated or supersaturated";
const CRYST: &str = "Crystallisation on cooling";
const TEMP: &str = "Temperature of saturation";

// ---------------------------------------------------------------------------------------------
// The published data, copied again here from the source named in the module (CRC Handbook of Chemistry and Physics,
// as reproduced in Wikipedia's "Solubility table"; CO2 from Lange's Handbook, 10th ed.), one row per substance:
// (param key, name, data). Only the cells that are published.
// ---------------------------------------------------------------------------------------------
type Row = (&'static str, &'static str, bool, &'static [(f64, f64)]);
const DATA: [Row; 10] = [
    ("s_kno3", "KNO₃", false, &[(0.0, 13.3), (10.0, 20.9), (20.0, 31.6), (30.0, 45.8), (40.0, 63.9), (50.0, 85.5), (60.0, 110.0), (70.0, 138.0), (80.0, 169.0), (90.0, 202.0), (100.0, 246.0)]),
    ("s_nano3", "NaNO₃", false, &[(0.0, 73.0), (10.0, 80.8), (20.0, 87.6), (30.0, 94.9), (40.0, 102.0), (60.0, 122.0), (80.0, 148.0), (100.0, 180.0)]),
    ("s_nacl", "NaCl", false, &[(0.0, 35.65), (10.0, 35.72), (20.0, 35.89), (30.0, 36.09), (40.0, 36.37), (50.0, 36.69), (60.0, 37.04), (70.0, 37.46), (80.0, 37.93), (90.0, 38.47), (100.0, 38.99)]),
    ("s_kcl", "KCl", false, &[(0.0, 28.0), (10.0, 31.2), (20.0, 34.2), (30.0, 37.2), (40.0, 40.1), (50.0, 42.6), (60.0, 45.8), (80.0, 51.3), (90.0, 53.9), (100.0, 56.3)]),
    ("s_kclo3", "KClO₃", false, &[(0.0, 3.3), (10.0, 5.2), (20.0, 7.3), (30.0, 10.1), (40.0, 13.9), (60.0, 23.8), (80.0, 37.5), (90.0, 46.0), (100.0, 56.3)]),
    ("s_nh4cl", "NH₄Cl", false, &[(0.0, 29.4), (10.0, 33.2), (20.0, 37.2), (30.0, 41.4), (40.0, 45.8), (50.0, 50.4), (60.0, 55.3), (70.0, 60.2), (80.0, 65.6), (90.0, 71.2), (100.0, 77.3)]),
    ("s_pbno32", "Pb(NO₃)₂", false, &[(0.0, 37.5), (10.0, 46.2), (20.0, 54.3), (30.0, 63.4), (40.0, 72.1), (60.0, 91.6), (80.0, 111.0), (100.0, 133.0)]),
    ("s_ce2so43", "Ce₂(SO₄)₃", false, &[(0.0, 21.4), (20.0, 9.84), (30.0, 7.24), (40.0, 5.63), (60.0, 3.87)]),
    ("s_co2", "CO₂", true, &[(0.0, 0.3346), (10.0, 0.2318), (20.0, 0.1688), (30.0, 0.1257), (40.0, 0.0973), (50.0, 0.0761), (60.0, 0.0576)]),
    ("s_o2", "O₂", true, &[(0.0, 0.00146), (10.0, 0.00113), (20.0, 0.00091), (30.0, 0.00076), (40.0, 0.00065)]),
];
fn row(name: &str) -> &'static Row { DATA.iter().find(|r| r.1 == name).unwrap() }

/// The reference interpolation: Fritsch-Butland slopes (zero at a turning point), the three-point end slopes of
/// Moler's `pchip`, and de Casteljau on the Bezier form of each cubic piece.
fn ref_eval(data: &[(f64, f64)], x: f64) -> f64 {
    let n = data.len();
    let h: Vec<f64> = (0..n - 1).map(|k| data[k + 1].0 - data[k].0).collect();
    let del: Vec<f64> = (0..n - 1).map(|k| (data[k + 1].1 - data[k].1) / h[k]).collect();
    let mut d = vec![0.0; n];
    for k in 1..n - 1 {
        if del[k - 1] * del[k] > 0.0 {
            d[k] = 3.0 * (h[k - 1] + h[k]) / ((2.0 * h[k] + h[k - 1]) / del[k - 1] + (h[k] + 2.0 * h[k - 1]) / del[k]);
        }
    }
    let end = |h0: f64, h1: f64, m0: f64, m1: f64| {
        let mut e = ((2.0 * h0 + h1) * m0 - h0 * m1) / (h0 + h1);
        if e * m0 <= 0.0 { e = 0.0 } else if m0 * m1 <= 0.0 && e.abs() > 3.0 * m0.abs() { e = 3.0 * m0 }
        e
    };
    if n == 2 { d[0] = del[0]; d[1] = del[0]; } else {
        d[0] = end(h[0], h[1], del[0], del[1]);
        d[n - 1] = end(h[n - 2], h[n - 3], del[n - 2], del[n - 3]);
    }
    let mut k = 0;
    while k + 2 < n && x >= data[k + 1].0 { k += 1; }
    let (x0, y0, x1, y1) = (data[k].0, data[k].1, data[k + 1].0, data[k + 1].1);
    let t = (x - x0) / (x1 - x0);
    let (p0, p1, p2, p3) = (y0, y0 + (x1 - x0) * d[k] / 3.0, y1 - (x1 - x0) * d[k + 1] / 3.0, y1);
    let l = |a: f64, c: f64| a + (c - a) * t;
    let (q0, q1, q2) = (l(p0, p1), l(p1, p2), l(p2, p3));
    let (r0, r1) = (l(q0, q1), l(q1, q2));
    l(r0, r1)
}
fn ref_s(name: &str, t: f64) -> f64 { ref_eval(row(name).3, t) }

/// Decimal rounding by integer arithmetic on the value to 9 places: `dec` places from 1 up, 3 significant figures
/// below 1; half away from zero.
fn ref_round(v: f64, dec: usize) -> String {
    let a = v.abs();
    if a == 0.0 { return format!("{:.*}", dec, 0.0); }
    let places: i32 = if a >= 1.0 { dec as i32 } else { 2 - a.log10().floor() as i32 };
    let scaled = (a * 1e9).round() as i128;
    let unit = 10i128.pow((9 - places).max(0) as u32);
    let r = (scaled + unit / 2) / unit;
    let s = format!("{:0>width$}", r, width = (places + 1) as usize);
    let (ip, fp) = s.split_at(s.len() - places as usize);
    let out = if places > 0 { format!("{ip}.{fp}") } else { ip.to_string() };
    if v < 0.0 { format!("\u{2212}{out}") } else { out }
}

/// `v` to `places` decimals, half away from zero, by integer arithmetic on the value to 9 places.
fn ref_round_at(v: f64, places: usize) -> String {
    let scaled = (v.abs() * 1e9).round() as i128;
    let unit = 10i128.pow((9 - places as i32).max(0) as u32);
    let r = (scaled + unit / 2) / unit;
    let s = format!("{:0>width$}", r, width = places + 1);
    let (ip, fp) = s.split_at(s.len() - places);
    let out = if places > 0 { format!("{ip}.{fp}") } else { ip.to_string() };
    if v < 0.0 { format!("\u{2212}{out}") } else { out }
}

fn close(a: f64, b: f64, rel: f64) -> bool { (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-300) }

// ---------------------------------------------------------------------------------------------
// Reading the SVG back
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct T { x: f64, y: f64, fs: f64, anchor: String, text: String, kind: String, feat: String, group: String, fill: String, rot: Option<(f64, f64)> }

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
        if tag.contains(r#"fill="none""#) { continue; }
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
            fill: attr(tag, "fill").unwrap_or_default(),
            rot,
        });
    }
    out
}
fn all_text(svg: &str) -> String { texts(svg).into_iter().map(|t| t.text).collect::<Vec<_>>().join("\n") }
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

type Seg = (Pt, Pt);
/// Strokes (kind, feature, segment), whole paths (kind, feature, points, style) and markers (feature, kind of shape, box).
struct Geo { strokes: Vec<(String, String, Seg)>, paths: Vec<(String, String, Vec<Pt>, String)>, marks: Vec<(String, String, Bx)>, keys: Vec<(String, String, String, Bx)> }

fn geo(svg: &str) -> Geo {
    let mut g = Geo { strokes: Vec::new(), paths: Vec::new(), marks: Vec::new(), keys: Vec::new() };
    for part in svg.split('<').skip(1) {
        let tag = part.split('>').next().unwrap_or("");
        let kind = attr(tag, "data-o").unwrap_or_default();
        let feat = attr(tag, "data-f").unwrap_or_default();
        if tag.starts_with("line ") {
            let (x1, y1, x2, y2) = (attr_f(tag, "x1").unwrap(), attr_f(tag, "y1").unwrap(), attr_f(tag, "x2").unwrap(), attr_f(tag, "y2").unwrap());
            g.strokes.push((kind, feat, ((x1, y1), (x2, y2))));
        } else if tag.starts_with("path ") {
            let pts = path_pts(&attr(tag, "d").unwrap());
            if kind == "mark" || kind == "key" {
                let bx = pts.iter().fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0), a.2.min(p.1), a.3.max(p.1)));
                if kind == "key" { g.keys.push((feat, attr(tag, "data-g").unwrap_or_default(), attr(tag, "data-s").unwrap_or_default(), bx)); } else { g.marks.push((feat, attr(tag, "data-s").unwrap_or_default(), bx)); }
                continue;
            }
            for w in pts.windows(2) { g.strokes.push((kind.clone(), feat.clone(), (w[0], w[1]))); }
            let style = format!("{}|{}|{}", attr(tag, "stroke").unwrap_or_default(), attr(tag, "stroke-width").unwrap_or_default(), attr(tag, "stroke-dasharray").unwrap_or_default());
            g.paths.push((kind, feat, pts, style));
        } else if tag.starts_with("circle ") && (kind == "mark" || kind == "key") {
            let (cx, cy, r) = (attr_f(tag, "cx").unwrap(), attr_f(tag, "cy").unwrap(), attr_f(tag, "r").unwrap());
            if kind == "key" { g.keys.push((feat, attr(tag, "data-g").unwrap_or_default(), attr(tag, "data-s").unwrap_or_default(), (cx - r, cx + r, cy - r, cy + r))); } else { g.marks.push((feat, attr(tag, "data-s").unwrap_or_default(), (cx - r, cx + r, cy - r, cy + r))); }
        } else if tag.starts_with("polygon ") && (kind == "mark" || kind == "key") {
            let pts: Vec<Pt> = attr(tag, "points").unwrap().split_whitespace().filter_map(|t| { let (a, b) = t.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
            let bx = pts.iter().fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0), a.2.min(p.1), a.3.max(p.1)));
            if kind == "key" { g.keys.push((feat, attr(tag, "data-g").unwrap_or_default(), attr(tag, "data-s").unwrap_or_default(), bx)); } else { g.marks.push((feat, attr(tag, "data-s").unwrap_or_default(), bx)); }
        }
    }
    g
}

// ---------------------------------------------------------------------------------------------
// Real glyphs
// ---------------------------------------------------------------------------------------------

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
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="6000" height="600"><text x="3000" y="300" font-size="{:.2}" text-anchor="{}" font-family="{FONT}">{}</text></svg>"#,
            t.fs,
            t.anchor,
            esc(&t.text)
        );
        fn find(g: &resvg::usvg::Group) -> Option<resvg::usvg::Rect> {
            for n in g.children() {
                match n {
                    resvg::usvg::Node::Text(t) => return Some(t.flattened().abs_bounding_box()),
                    resvg::usvg::Node::Group(g2) => {
                        if let Some(r) = find(g2) { return Some(r); }
                    }
                    _ => {}
                }
            }
            None
        }
        let out = resvg::usvg::Tree::from_str(&svg, &self.opt)
            .ok()
            .and_then(|tree| find(tree.root()))
            .filter(|r| r.width() > 0.0)
            .map(|r| (r.left() as f64 - 3000.0, r.right() as f64 - 3000.0, r.top() as f64 - 300.0, r.bottom() as f64 - 300.0));
        self.memo.insert(key, out);
        out
    }
    fn ink(&mut self, t: &T) -> Option<Bx> {
        let r = self.rel(t)?;
        Some(match t.rot {
            None => (t.x + r.0, t.x + r.1, t.y + r.2, t.y + r.3),
            Some((cx, cy)) => (cx + r.2, cx + r.3, cy - r.1, cy - r.0),
        })
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
fn pt_poly(p: Pt, pts: &[Pt]) -> f64 { pts.windows(2).map(|w| seg_box(w[0], w[1], (p.0, p.0, p.1, p.1))).fold(f64::MAX, f64::min) }

/// Every problem with the text of one render, measured in real glyphs: text off the canvas, text on text, a label on
/// a stroke (a curve, the axes, another feature's leader line) or a marker, and a label that does not read as its own
/// feature's: more than 40 px x size from it (or from the leader line that joins it), or more than 15 px from it while
/// 6 px nearer another labelled feature; and a leader line that does not start on its own curve and end at its label.
/// The dotted guides and the grid are thin lines a haloed label may cross.
fn audit(svg: &str, g: &mut Glyphs, lab: f64) -> Vec<String> {
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
            if ts[a].kind == "label" && ts[c].kind == "label" && ts[a].group == ts[c].group { continue; }
            if let (Some(x), Some(y)) = (inks[a], inks[c]) {
                if hits(x, y) { bad.push(format!("{:?} on {:?}", ts[a].text, ts[c].text)); }
            }
        }
    }
    let mut groups: BTreeMap<String, (String, Bx, String)> = BTreeMap::new();
    for (t, bx) in ts.iter().zip(&inks) {
        if t.kind != "label" { continue; }
        let Some(bx) = bx else { continue };
        for (kind, feat, (p, q)) in &gm.strokes {
            if kind == "grid" { continue; }
            if kind == "leader" && *feat == t.feat { continue; }
            if seg_hits(*p, *q, *bx) { bad.push(format!("{:?} on a {kind} stroke of {feat} {p:?}-{q:?}", t.text)); }
        }
        for (feat, _, mb) in &gm.marks {
            if hits(*bx, *mb) { bad.push(format!("{:?} on the {feat} marker", t.text)); }
        }
        let e = groups.entry(t.group.clone()).or_insert((t.feat.clone(), *bx, t.text.clone()));
        e.1 = (e.1 .0.min(bx.0), e.1 .1.max(bx.1), e.1 .2.min(bx.2), e.1 .3.max(bx.3));
    }
    // a name's own marker (Print) is part of the name; no other name may be on it
    for (kf, kg, _, kb) in &gm.keys {
        if let Some(e) = groups.get_mut(kg) {
            if e.0 != *kf { bad.push(format!("the key marker of {kf} sits on the label of {}", e.0)); }
            e.1 = (e.1 .0.min(kb.0), e.1 .1.max(kb.1), e.1 .2.min(kb.2), e.1 .3.max(kb.3));
        }
        for (t, bx) in ts.iter().zip(&inks) {
            if t.kind == "label" && t.group != *kg { if let Some(bx) = bx { if hits(*bx, *kb) { bad.push(format!("{:?} on the key marker of {kf}", t.text)); } } }
        }
        for (kind, feat, (p, q)) in &gm.strokes {
            if kind != "grid" && !(kind == "leader" && feat == kf) && seg_hits(*p, *q, *kb) { bad.push(format!("the key marker of {kf} is on a {kind} stroke of {feat}")); }
        }
    }
    let curves: Vec<String> = { let mut v: Vec<String> = gm.strokes.iter().filter(|x| x.0 == "curve").map(|x| x.1.clone()).collect(); v.sort(); v.dedup(); v };
    let named: Vec<String> = { let mut v: Vec<String> = groups.values().map(|g| g.0.clone()).filter(|f| !f.starts_with("ann")).collect(); v.sort(); v.dedup(); v };
    let dist = |feat: &str, bx: Bx| -> f64 {
        let mut d = f64::MAX;
        for (k, f, (p, q)) in &gm.strokes { if f == feat && k != "leader" && k != "guide" { d = d.min(seg_box(*p, *q, bx)); } }
        for (f, _, mb) in &gm.marks { if f == feat { d = d.min(pt_box(((mb.0 + mb.1) / 2.0, (mb.2 + mb.3) / 2.0), bx)); } }
        d
    };
    for (own, bx, name) in groups.values().filter(|g| !g.0.starts_with("ann")) {
        let leader = gm.paths.iter().find(|p| p.0 == "leader" && &p.1 == own);
        if let Some((_, _, pts, _)) = leader {
            // the leader joins the label to its own feature
            let (a, z) = (pts[0], pts[pts.len() - 1]);
            let ds = dist(own, (a.0, a.0, a.1, a.1));
            if ds > 4.0 { bad.push(format!("the leader of {name:?} starts {ds:.1} px from its {own} feature")); }
            let de = pt_box(z, *bx);
            if de > 6.0 { bad.push(format!("the leader of {name:?} ends {de:.1} px from its label")); }
            // a leader may not run through, or end beside, a marker that is not its own feature's
            for (mf, _, mb) in &gm.marks {
                if mf == own { continue; }
                let starts_in = a.0 >= mb.0 - 0.5 && a.0 <= mb.1 + 0.5 && a.1 >= mb.2 - 0.5 && a.1 <= mb.3 + 0.5;
                let own_curve = mf.starts_with("s_") && gm.marks.iter().any(|m2| &m2.0 == own && (m2.2 .0 - mb.0).abs() < 0.6 && (m2.2 .2 - mb.2).abs() < 0.6);
                if own_curve { continue; }
                if !starts_in && pts.windows(2).any(|w| seg_hits(w[0], w[1], *mb)) { bad.push(format!("the leader of {name:?} runs through the {mf} marker")); }
                if !starts_in && pt_box(z, *mb) < 2.0 { bad.push(format!("the leader of {name:?} ends beside the {mf} marker")); }
            }
            let d_other = named.iter().filter(|o| *o != own).map(|o| dist(o, (a.0, a.0, a.1, a.1))).fold(f64::MAX, f64::min);
            if d_other < ds - 0.25 { bad.push(format!("the leader of {name:?} starts nearer another feature")); }
            continue;
        }
        let d = dist(own, *bx);
        if d > 40.0 * lab { bad.push(format!("{name:?} is {d:.1} px from its {own} feature")); continue; }
        if own.starts_with("s_") {
            // a name with no leader line must be nearer its own curve than any other curve
            let rival = curves.iter().filter(|o| *o != own).map(|o| (dist(o, *bx), o)).fold((f64::MAX, None), |a, (x, o)| if x < a.0 { (x, Some(o)) } else { a });
            if d > rival.0 - 0.25 { bad.push(format!("{name:?} is {d:.1} px from its {own} curve but {:.1} px from {:?}, with no leader line", rival.0, rival.1)); }
            continue;
        }
        let other = named.iter().filter(|o| *o != own).map(|o| (dist(o, *bx), o)).fold((f64::MAX, None), |a, (x, o)| if x < a.0 { (x, Some(o)) } else { a });
        if d > 15.0 && other.0 < d - 6.0 { bad.push(format!("{name:?} is {d:.1} px from its {own} feature but {:.1} px from {:?}", other.0, other.1)); }
    }
    let _ = pt_poly;
    bad
}

// ---------------------------------------------------------------------------------------------
// Numbers in the text
// ---------------------------------------------------------------------------------------------

fn tokens(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < c.len() {
        if c[k].is_ascii_digit() {
            let start = if k > 0 && c[k - 1] == '\u{2212}' { k - 1 } else { k };
            let mut e = k;
            while e < c.len() && (c[e].is_ascii_digit() || (c[e] == '.' && e + 1 < c.len() && c[e + 1].is_ascii_digit())) { e += 1; }
            out.push(c[start..e].iter().collect());
            k = e;
        } else {
            k += 1;
        }
    }
    out
}
fn parse_num(s: &str) -> f64 { s.trim().replace('\u{2212}', "-").parse().unwrap_or_else(|_| panic!("not a number: {s:?}")) }
/// The number right after `pat` in `s`.
fn num_after(s: &str, pat: &str) -> String {
    let rest = s.split(pat).nth(1).unwrap_or_else(|| panic!("no {pat:?} in {s:?}"));
    let w: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '\u{2212}').collect();
    w.trim_end_matches('.').to_string()
}
fn line_starting<'a>(lines: &'a [String], head: &str) -> Option<&'a String> { lines.iter().find(|l| l.starts_with(head)) }
fn joined(r: &Report) -> String { r.readout.join("\n") }
fn tfmt(t: f64) -> String { crate::figures::chart::fmt(t) }

// ---------------------------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------------------------

#[test]
fn it_is_registered_with_a_faq_examples_and_its_controls() {
    let d = def();
    assert_eq!(d.subject, Subject::Chemistry);
    assert!((4..=6).contains(&crate::figures::faq::faq_for(ID).len()));
    for q in crate::figures::faq::faq_for(ID) { assert!(q.q.len() > 12 && q.a.len() > 60, "{}", q.q); }
    let names: Vec<&str> = crate::figures::examples_for(ID).iter().map(|e| e.name).collect();
    for want in ["Read a solubility", "supersaturated", "crystallises on cooling", "gas"] {
        assert!(names.iter().any(|n| n.contains(want)), "no {want} example in {names:?}");
    }
    let keys: Vec<&str> = (d.params)().iter().map(|p| p.key).collect();
    for k in [
        "mode", "substance", "t1", "t2", "water", "solute", "n_points", "pb_t", "pb_m", "pc_t", "pc_m", "points", "grid", "dec", "readout", SHOW_ANSWERS, "style", "label_size",
    ] {
        assert!(keys.contains(&k), "no {k} param");
    }
    for r in DATA.iter() { assert!(keys.contains(&r.0), "no {} toggle", r.0); }
    assert!(crate::figures::chemistry::skin_intro(ID).is_some());
    assert!(!crate::figures::notes_for(ID, &with(&[f("water", 0.0)])).is_empty());
    for e in crate::figures::examples_for(ID) {
        let p = with(&e.params);
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
        assert!(def().svg(&p).contains("<svg"));
        for ans in [true, false] {
            let mut q = p.clone();
            q.set_b(SHOW_ANSWERS, ans);
            assert!(warnings(&q).is_empty(), "{}: {:?}", e.name, warnings(&q));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The data and the interpolation
// ---------------------------------------------------------------------------------------------

#[test]
fn the_tables_are_the_published_ones_and_nothing_is_added() {
    assert_eq!(SUBS.len(), DATA.len());
    for (sb, r) in SUBS.iter().zip(DATA.iter()) {
        assert_eq!((sb.key, sb.name, sb.gas), (r.0, r.1, r.2));
        assert_eq!(sb.data, r.3, "{}", r.1);
    }
    // spot checks against the sources by hand
    assert_eq!(row("KNO₃").3[4], (40.0, 63.9));
    assert_eq!(row("KNO₃").3[10], (100.0, 246.0));
    assert_eq!(row("Ce₂(SO₄)₃").3.len(), 5);
    // the data are monotone in temperature: rising for the salts but one, falling for cerium sulfate and the gases
    for r in DATA.iter() {
        let rising = r.3[r.3.len() - 1].1 > r.3[0].1;
        assert_eq!(rising, !(r.1 == "Ce₂(SO₄)₃" || r.2), "{}", r.1);
        for w in r.3.windows(2) { assert!(w[0].0 < w[1].0 && (w[1].1 > w[0].1) == rising, "{}", r.1); }
    }
}

#[test]
fn evaluating_at_the_table_points_reproduces_the_table_exactly() {
    for (k, sb) in SUBS.iter().enumerate() {
        for &(t, v) in sb.data { assert_eq!(value(k, t), v, "{} at {t}", sb.name); }
    }
}

#[test]
fn the_interpolation_matches_the_reference_is_monotone_and_never_leaves_the_data() {
    for (k, sb) in SUBS.iter().enumerate() {
        let (lo, hi) = (sb.data[0].0, sb.data[sb.data.len() - 1].0);
        let mut prev: Option<f64> = None;
        let mut t = lo;
        while t <= hi + 1e-9 {
            let v = value(k, t);
            assert!(close(v, ref_eval(sb.data, t), 1e-12), "{} at {t}: {v} against {}", sb.name, ref_eval(sb.data, t));
            // between two neighbouring table values
            let seg = sb.data.windows(2).find(|w| t >= w[0].0 && t <= w[1].0).unwrap();
            let (a, c) = (seg[0].1.min(seg[1].1), seg[0].1.max(seg[1].1));
            assert!(v >= a - 1e-12 && v <= c + 1e-12, "{} at {t} leaves {a}..{c}: {v}", sb.name);
            if let Some(pv) = prev {
                let rising = sb.data[sb.data.len() - 1].1 > sb.data[0].1;
                assert!(if rising { v >= pv - 1e-12 } else { v <= pv + 1e-12 }, "{} not monotone at {t}", sb.name);
            }
            prev = Some(v);
            t += 0.25;
        }
    }
}

#[test]
fn the_inverse_finds_the_temperature_of_a_solubility() {
    for (k, sb) in SUBS.iter().enumerate() {
        let (lo, hi) = (sb.data[0].0, sb.data[sb.data.len() - 1].0);
        for n in 0..=40 {
            let t = lo + (hi - lo) * n as f64 / 40.0;
            let v = value(k, t);
            let back = temperature_at(k, v).unwrap_or_else(|| panic!("{} {v}", sb.name));
            assert!((value(k, back) - v).abs() <= 1e-9 * v.abs().max(1e-3), "{} {t} -> {back}", sb.name);
        }
        let (a, z) = (sb.data[0].1, sb.data[sb.data.len() - 1].1);
        let (smin, smax) = (a.min(z), a.max(z));
        assert!(temperature_at(k, smin * 0.999).is_none() && temperature_at(k, smax * 1.001).is_none(), "{}", sb.name);
    }
}

// ---------------------------------------------------------------------------------------------
// The four questions on the key
// ---------------------------------------------------------------------------------------------

fn sub_params(name: &str) -> Vec<(&'static str, Value)> {
    let mut v = vec![s("substance", name)];
    for r in DATA.iter() { v.push(b(r.0, r.1 == name)); }
    v
}

#[test]
fn reading_a_solubility_is_the_curve_value_rounded_once_at_every_temperature() {
    for r in DATA.iter() {
        for dec in 0..=2usize {
            let (lo, hi) = (r.3[0].0, r.3[r.3.len() - 1].0);
            let mut t = lo;
            while t <= hi + 1e-9 {
                let mut l = sub_params(r.1);
                l.extend([s("mode", READ), f("t1", t), i("dec", dec as i64)]);
                let rep = key(&l);
                let line = line_starting(&rep.readout, &format!("Read from the curve at {} °C: solubility = ", tfmt(t))).unwrap_or_else(|| panic!("{} {t}: {:?}", r.1, rep.readout));
                let got = num_after(line, "solubility = ");
                assert_eq!(got, ref_round(ref_s(r.1, t), dec), "{} at {t} dec {dec}", r.1);
                t += 2.5;
            }
        }
    }
}

#[test]
fn rounding_ties_go_away_from_zero_in_decimals() {
    // NaCl at 0 deg C is 35.65 in the table: 35.7 to one decimal place, 36 to none, 35.65 to two
    for (dec, want) in [(0, "36"), (1, "35.7"), (2, "35.65")] {
        let mut l = sub_params("NaCl");
        l.extend([s("mode", READ), f("t1", 0.0), i("dec", dec)]);
        let rep = key(&l);
        assert_eq!(num_after(line_starting(&rep.readout, "Read from the curve").unwrap(), "solubility = "), want);
    }
    // KNO3 at 70: 138 exactly; at 70 with dec 2 it is 138.00
    let mut l = sub_params("KNO₃");
    l.extend([s("mode", READ), f("t1", 70.0), i("dec", 2)]);
    assert_eq!(num_after(line_starting(&key(&l).readout, "Read from the curve").unwrap(), "solubility = "), "138.00");
    // small values keep three significant figures whatever the decimal places
    let mut l = sub_params("CO₂");
    l.extend([s("mode", READ), f("t1", 0.0), i("dec", 1)]);
    let rep = key(&l);
    assert_eq!(num_after(line_starting(&rep.readout, "Read from the curve").unwrap(), "solubility = "), "0.335");
    assert!(joined(&rep).contains("3 significant figures"), "{}", joined(&rep));
    let mut l = sub_params("O₂");
    l.extend([s("mode", READ), f("t1", 0.0)]);
    assert_eq!(num_after(line_starting(&key(&l).readout, "Read from the curve").unwrap(), "solubility = "), "0.00146");
}

#[test]
fn a_table_point_says_what_the_table_has_and_a_point_between_says_nothing_of_the_kind() {
    let mut l = sub_params("NaCl");
    l.extend([s("mode", READ), f("t1", 0.0)]);
    assert!(joined(&key(&l)).contains("The table value at 0 °C is 35.65 g per 100 g water"), "{}", joined(&key(&l)));
    l.push(f("t1", 5.0));
    let t = joined(&key(&l));
    assert!(!t.contains("The table value at"), "{t}");
}

fn classify_list(name: &str, t: f64, m: f64, w: f64, dec: i64) -> Vec<(&'static str, Value)> {
    let mut l = sub_params(name);
    l.extend([s("mode", CLASSIFY), f("t1", t), f("solute", m), f("water", w), i("dec", dec)]);
    l
}

#[test]
fn a_point_is_classified_from_the_printed_values_and_the_neighbours_of_the_curve_are_right() {
    // KNO3 at 40: S = 63.9 on one decimal place
    for (m, want, rel) in [(63.8, "unsaturated", "<"), (63.9, "saturated", "="), (64.0, "supersaturated", ">"), (0.0, "unsaturated", "<"), (50.0, "unsaturated", "<"), (200.0, "supersaturated", ">")] {
        let rep = key(&classify_list("KNO₃", 40.0, m, 100.0, 1));
        let t = joined(&rep);
        let l = line_starting(&rep.readout, "Point A: ").map(|_| ()).and(rep.readout.iter().find(|x| x.contains(" so A lies "))).unwrap_or_else(|| panic!("{t}"));
        assert!(l.contains(&format!("the solution is {want}.")), "{m}: {l}");
        assert!(l.contains(&format!(" {rel} 63.9,")), "{m}: {l}");
    }
    // with a different water mass: 150 g in 300 g water is 50 per 100 g
    let rep = key(&classify_list("KNO₃", 40.0, 150.0, 300.0, 1));
    let t = joined(&rep);
    assert!(t.contains("150 × 100 / 300 = 50.0 g"), "{t}");
    assert!(t.contains("is unsaturated."), "{t}");
    // exactly on the curve with a different water mass: 191.7 g in 300 g water is 63.9 per 100 g
    let rep = key(&classify_list("KNO₃", 40.0, 191.7, 300.0, 1));
    assert!(joined(&rep).contains("is saturated."), "{}", joined(&rep));
    // a sweep of every substance, with the printed numbers read back and the classification recomputed
    for r in DATA.iter() {
        let t = r.3[r.3.len() / 2].0;
        let sv = ref_s(r.1, t);
        for dec in 0..=2usize {
            for fac in [0.0, 0.5, 0.9, 0.99, 1.0, 1.01, 1.1, 2.0, 7.0] {
                let m = sig4(sv * fac);
                let rep = key(&classify_list(r.1, t, m, 100.0, dec as i64));
                let text = rep.readout.iter().find(|x| x.contains(" so A lies ")).unwrap().clone();
                let dl = rep.readout.iter().find(|x| x.starts_with("Point A: dissolved per 100 g")).unwrap().clone();
                let ys = dl.split("= ").last().unwrap().trim_end_matches(" g").to_string();
                // the places shown: the chosen ones, and one more each time the two readings would still look alike
                let nd = |s: &str| if s.contains('.') { s.split('.').nth(1).unwrap().len() } else { 0 };
                let sigfig = |s: &str| s.trim_start_matches("0.").trim_start_matches('0').len();
                let (yv, rv) = (m * 100.0 / 100.0, sv);
                let exact = (yv - rv).abs() <= 1e-9 * yv.abs().max(rv.abs());
                let want = if exact { "saturated" } else if yv < rv { "unsaturated" } else { "supersaturated" };
                assert!(text.contains(&format!("the solution is {want}.")), "{} m={m} dec={dec}: {text} (ref {sv})", r.1);
                // every printed value is the exact one rounded once to the places shown, and the two differ when they are not equal
                let first = text.strip_prefix("Point A: ").unwrap();
                let shown_y = first.split(' ').next().unwrap().to_string();
                let rs = text.split(" so A lies ").next().unwrap().trim_end_matches(',').rsplit(' ').next().unwrap().to_string();
                assert_eq!(shown_y, ys, "{} {m}: the comparison and the division print the same number", r.1);
                let places = if sv.abs() >= 1.0 { nd(&rs) } else { 0 };
                if yv == 0.0 {
                    assert_eq!(parse_num(&ys), 0.0, "{text}");
                } else if sv.abs() >= 1.0 {
                    assert!(places >= dec, "{text}");
                    assert_eq!(ys, ref_round_at(yv, places), "{} {m}", r.1);
                    assert_eq!(rs, ref_round_at(rv, places), "{} {m}", r.1);
                } else {
                    // three significant figures, and more when they would agree
                    assert!(sigfig(&rs) >= 3 && sigfig(&ys) == sigfig(&rs), "{text}");
                }
                if !exact { assert_ne!(ys, rs, "{} m={m} dec={dec}: unequal readings shown alike: {text}", r.1); }
            }
        }
    }
}

#[test]
fn up_to_three_points_each_get_their_own_lines_and_a_student_blank() {
    let mut l = classify_list("KNO₃", 40.0, 50.0, 100.0, 1);
    l.extend([i("n_points", 3), f("pb_t", 60.0), f("pb_m", 110.0), f("pc_t", 20.0), f("pc_m", 60.0)]);
    let rep = key(&l);
    let t = joined(&rep);
    for (pt, cls) in [("A", "unsaturated"), ("B", "saturated"), ("C", "supersaturated")] {
        assert!(rep.readout.iter().any(|x| x.starts_with(&format!("Point {pt}: ")) && x.contains(&format!("the solution is {cls}."))), "{pt}: {t}");
    }
    let mut sl = l.clone();
    sl.push(b(SHOW_ANSWERS, false));
    let st = rep_of(&sl);
    for pt in ["A", "B", "C"] { assert!(st.readout.iter().any(|x| x.starts_with(&format!("Point {pt} is ______"))), "{:?}", st.readout); }
    let all = all_text(&svg_of(&sl));
    assert!(!all.contains("is unsaturated.") && !all.contains("is supersaturated.") && !all.contains("lies below"), "{all}");
    // the points sit where their numbers put them
    let svg = svg_of(&l);
    let feats: BTreeSet<String> = geo(&svg).marks.iter().filter(|m| m.0.starts_with("pt")).map(|m| m.0.clone()).collect();
    assert_eq!(feats.len(), 3, "{feats:?}");
}

fn cryst_list(name: &str, t1: f64, t2: f64, w: f64, dec: i64) -> Vec<(&'static str, Value)> {
    let mut l = sub_params(name);
    l.extend([s("mode", CRYST), f("t1", t1), f("t2", t2), f("water", w), i("dec", dec)]);
    l
}

#[test]
fn crystallising_on_cooling_is_worked_from_the_two_printed_readings() {
    // the worked example: KNO3 from 80 to 20 deg C with 200 g of water
    let rep = key(&cryst_list("KNO₃", 80.0, 20.0, 200.0, 1));
    let t = joined(&rep);
    assert!(t.contains("solubility = 169.0 g per 100 g water"), "{t}");
    assert!(t.contains("solubility = 31.6 g per 100 g water"), "{t}");
    assert!(t.contains("Mass that crystallises = (169.0 − 31.6) × 200 / 100 = 274.8 g"), "{t}");
    // an independent sweep: every rising salt, many pairs, three precisions, three water masses
    for r in DATA.iter().filter(|r| !r.2 && r.1 != "Ce₂(SO₄)₃") {
        let (lo, hi) = (r.3[0].0, r.3[r.3.len() - 1].0);
        for dec in 0..=2usize {
            for w in [100.0, 250.0, 37.5] {
                for (t1, t2) in [(hi, lo), (80.0f64.min(hi), 20.0f64.max(lo)), (60.0f64.min(hi), 33.0f64.max(lo)), (lo, hi), (45.0f64.min(hi), 45.0f64.min(hi))] {
                    let rep = key(&cryst_list(r.1, t1, t2, w, dec as i64));
                    let r1s = ref_round(ref_s(r.1, t1), dec);
                    let r2s = ref_round(ref_s(r.1, t2), dec);
                    let (r1, r2) = (parse_num(&r1s), parse_num(&r2s));
                    let txt = joined(&rep);
                    assert!(txt.contains(&format!("Read from the curve at {} °C: solubility = {r1s} g", tfmt(t1))), "{txt}");
                    assert!(txt.contains(&format!("Read from the curve at {} °C: solubility = {r2s} g", tfmt(t2))), "{txt}");
                    if t1 == t2 {
                        assert!(txt.contains("same, so nothing changes and no solute crystallises"), "{txt}");
                    } else if r1 > r2 {
                        let line = line_starting(&rep.readout, "Mass that crystallises = (").unwrap_or_else(|| panic!("{txt}"));
                        let m = (r1 - r2) * w / 100.0;
                        assert!(line.starts_with(&format!("Mass that crystallises = ({r1s} − {r2s}) × {} / 100 = ", tfmt(w))), "{line}");
                        assert!(line.ends_with(&format!("{} g", ref_round(m, dec))), "{line} want {}", ref_round(m, dec));
                    } else {
                        assert!(txt.contains("Mass that crystallises = 0 g"), "{txt}");
                        assert!(txt.contains("unsaturated"), "{txt}");
                        if r2 > r1 {
                            let more = (r2 - r1) * w / 100.0;
                            assert!(txt.contains(&format!("can dissolve ({r2s} − {r1s}) × {} / 100 = {} g more", tfmt(w), ref_round(more, dec))), "{txt}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_falling_solubility_crystallises_on_warming_and_a_gas_comes_out_as_gas() {
    // cerium(III) sulfate: 20 -> 40 deg C, 100 g water: 9.8 - 5.6 = 4.2 g on one decimal place
    let rep = key(&cryst_list("Ce₂(SO₄)₃", 20.0, 40.0, 100.0, 1));
    let t = joined(&rep);
    assert!(t.contains("Mass that crystallises = (9.8 − 5.6) × 100 / 100 = 4.2 g"), "{t}");
    assert!(t.contains("less soluble in hot water") && t.contains("warming"), "{t}");
    // cooled instead: more dissolves, nothing crystallises
    let t = joined(&key(&cryst_list("Ce₂(SO₄)₃", 40.0, 20.0, 100.0, 1)));
    assert!(t.contains("Mass that crystallises = 0 g") && t.contains("can dissolve (9.8 − 5.6) × 100 / 100 = 4.2 g more"), "{t}");
    // gases: warming drives gas out; cooling lets more dissolve
    let t = joined(&key(&cryst_list("CO₂", 10.0, 50.0, 1000.0, 1)));
    assert!(t.contains("Mass that comes out of solution = (0.232 − 0.0761) × 1000 / 100 = 1.6 g"), "{t}");
    assert!(t.contains("gas") && !t.contains("crystallises ="), "{t}");
    let t = joined(&key(&cryst_list("CO₂", 50.0, 10.0, 1000.0, 1)));
    assert!(t.contains("Mass that comes out of solution = 0 g") && t.contains("more gas can dissolve") || t.contains("can dissolve") , "{t}");
    // warming a rising salt: nothing crystallises
    let t = joined(&key(&cryst_list("KNO₃", 20.0, 80.0, 200.0, 1)));
    assert!(t.contains("Mass that crystallises = 0 g") && t.contains("can dissolve (169.0 − 31.6) × 200 / 100 = 274.8 g more"), "{t}");
}

#[test]
fn the_neighbours_of_equal_temperatures_and_equal_readings() {
    // T2 = T1: nothing changes
    let t = joined(&key(&cryst_list("KNO₃", 50.0, 50.0, 100.0, 1)));
    assert!(t.contains("same, so nothing changes"), "{t}");
    // T2 just below T1 on a curve that rises a little: the readings can agree to the decimal places shown
    let t = joined(&key(&cryst_list("NaCl", 31.0, 30.0, 100.0, 0)));
    assert!(t.contains("Mass that crystallises = 0 g") && t.contains("same at both temperatures") || t.contains("the same"), "{t}");
    let t = joined(&key(&cryst_list("NaCl", 31.0, 30.0, 100.0, 2)));
    assert!(t.contains("= (36.10 − 36.09) × 100 / 100 = 0.01 g") || t.contains("(36.1"), "{t}");
}

fn temp_list(name: &str, m: f64, w: f64, dec: i64) -> Vec<(&'static str, Value)> {
    let mut l = sub_params(name);
    l.extend([s("mode", TEMP), f("solute", m), f("water", w), i("dec", dec)]);
    l
}

#[test]
fn the_saturation_temperature_is_read_back_from_the_curve() {
    for r in DATA.iter() {
        let (lo, hi) = (r.3[0].0, r.3[r.3.len() - 1].0);
        let (a, z) = (r.3[0].1, r.3[r.3.len() - 1].1);
        for n in 0..=20 {
            let target = a.min(z) + (a.max(z) - a.min(z)) * n as f64 / 20.0;
            let m = sig4(target * 2.5);
            // the amount as given: m grams in 250 g of water, not rounded to the places shown
            let y = m * 100.0 / 250.0;
            if y < a.min(z) - 1e-9 * a.min(z) || y > a.max(z) + 1e-9 * a.max(z) { continue; }
            for dec in [0i64, 1, 2] {
                let rep = key(&temp_list(r.1, m, 250.0, dec));
                let txt = joined(&rep);
                assert!(txt.contains(&format!("{} × 100 / 250 = ", tfmt(m))), "{txt}");
                // independent inversion by scanning the reference curve
                let mut best = (f64::MAX, lo);
                let mut tt = lo;
                while tt <= hi + 1e-9 { let e = (ref_eval(r.3, tt) - y).abs(); if e < best.0 { best = (e, tt); } tt += 0.01; }
                // the local slope of the reference curve, per degree, against its value
                let (v0, v1) = (ref_eval(r.3, (best.1 - 0.5).max(lo)), ref_eval(r.3, (best.1 + 0.5).min(hi)));
                let rel_slope = ((v1 - v0) / ((best.1 + 0.5).min(hi) - (best.1 - 0.5).max(lo))).abs() / (0.5 * (v0 + v1));
                if rel_slope < 0.0014 {
                    assert!(txt.contains("almost flat here, so the temperature cannot be read reliably") && !txt.contains("when T ="), "{} y={y}: {txt}", r.1);
                } else if rel_slope > 0.0016 {
                    let line = line_starting(&rep.readout, "The curve is at ").unwrap_or_else(|| panic!("{} {m}: {txt}", r.1));
                    let t = parse_num(&num_after(line, "when T = "));
                    assert!((t - best.1).abs() <= 0.51, "{} y={y}: printed {t}, scan {}", r.1, best.1);
                    assert!(t == t.round() && t >= lo && t <= hi, "{t}");
                }
            }
        }
    }
    // the example: 60 g KNO3 in 100 g water just saturates a little below 40 degrees
    let t = joined(&key(&temp_list("KNO₃", 60.0, 100.0, 1)));
    assert!(t.contains("60 × 100 / 100 = 60.0 g") && t.contains("when T = 38 °C"), "{t}");
}

#[test]
fn a_solution_that_never_just_saturates_in_the_graph_says_so() {
    // far below the curve: unsaturated at every temperature shown
    let t = joined(&key(&temp_list("KNO₃", 5.0, 100.0, 1)));
    assert!(t.contains("less than the lowest solubility on the graph") && t.contains("unsaturated at every temperature"), "{t}");
    let t = joined(&key(&temp_list("KNO₃", 0.0, 100.0, 1)));
    assert!(t.contains("unsaturated at every temperature"), "{t}");
    // far above: supersaturated at every temperature shown
    let t = joined(&key(&temp_list("KNO₃", 300.0, 100.0, 1)));
    assert!(t.contains("more than the highest solubility on the graph") && t.contains("supersaturated at every temperature"), "{t}");
    // exactly the end values
    let t = joined(&key(&temp_list("KNO₃", 13.3, 100.0, 1)));
    assert!(t.contains("when T = 0 °C"), "{t}");
    let t = joined(&key(&temp_list("KNO₃", 246.0, 100.0, 1)));
    assert!(t.contains("when T = 100 °C"), "{t}");
    // a falling curve is saturated by warming
    let t = joined(&key(&temp_list("Ce₂(SO₄)₃", 7.24, 100.0, 2)));
    assert!(t.contains("when T = 30 °C") && t.contains("warmed"), "{t}");
    let t = joined(&key(&temp_list("KNO₃", 60.0, 100.0, 1)));
    assert!(t.contains("cooled below it"), "{t}");
}

// ---------------------------------------------------------------------------------------------
// The student copy
// ---------------------------------------------------------------------------------------------

fn student_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut v = Vec::new();
    for name in ["KNO₃", "NaCl", "Ce₂(SO₄)₃", "CO₂", "Pb(NO₃)₂"] {
        for mode in [READ, CLASSIFY, CRYST, TEMP] {
            for (t1, t2) in [(40.0, 20.0), (80.0, 30.0), (35.0, 55.0)] {
                for style in ["Colour", "Print"] {
                    let mut l = sub_params(name);
                    l.extend([s("mode", mode), f("t1", t1), f("t2", t2), s("style", style), f("water", 200.0), f("solute", 55.0), i("n_points", 3), b("s_kno3", true), b("s_nacl", true)]);
                    v.push(l);
                }
            }
        }
    }
    v
}

#[test]
fn the_student_copy_has_no_computed_value_anywhere() {
    for c in student_cases() {
        let mut kl = c.clone();
        kl.push(b(SHOW_ANSWERS, true));
        let mut sl = c.clone();
        sl.push(b(SHOW_ANSWERS, false));
        let (k, st) = (build(&with(&kl)), build(&with(&sl)));
        let ssvg = &st.0;
        // numbers the key prints that the student copy's own readout does not
        let stu_ro: BTreeSet<String> = tokens(&joined(&st.1)).into_iter().collect();
        let only: BTreeSet<String> = tokens(&joined(&k.1)).into_iter().filter(|t| !stu_ro.contains(t)).collect();
        let places: Vec<T> = texts(ssvg).into_iter().filter(|t| t.kind != "axis").collect();
        for t in &places {
            for tok in tokens(&t.text) {
                assert!(!only.contains(&tok), "student copy prints {tok:?} (a key-only number) in {:?}\ncase {c:?}", t.text);
            }
        }
        let low = all_text(ssvg).replace("(saturated, unsaturated or supersaturated)", "");
        for w in ["unsaturated", "supersaturated", "Read from the curve", "Mass that crystallises = (", "Mass that comes out of solution = (", "lies below", "lies above"] {
            assert!(!low.contains(w), "student copy says {w:?}: {low}\ncase {c:?}");
        }
        // nothing in the title, the description or an aria text names an answer
        assert!(title_of(ssvg).is_ascii() && !desc_of(ssvg).chars().any(|ch| ch.is_ascii_digit()));
        assert!(st.1.readout.iter().any(|l| l.contains("______")), "no blank on the student copy: {:?}", st.1.readout);
    }
}

#[test]
fn the_student_copy_keeps_the_curves_names_and_givens_and_the_key_adds_marks() {
    for mode in [READ, CLASSIFY, CRYST, TEMP] {
        let l = vec![s("mode", mode), f("t1", 40.0), f("t2", 20.0), f("solute", 60.0), b("s_pbno32", true)];
        let (mut kl, mut sl) = (l.clone(), l.clone());
        kl.push(b(SHOW_ANSWERS, true));
        sl.push(b(SHOW_ANSWERS, false));
        let (ksvg, ssvg) = (svg_of(&kl), svg_of(&sl));
        let (gk, gs) = (geo(&ksvg), geo(&ssvg));
        let curves = |g: &Geo| g.paths.iter().filter(|p| p.0 == "curve").map(|p| format!("{:?}", p.2)).collect::<Vec<_>>();
        assert_eq!(curves(&gk).len(), 7, "{mode}");
        assert_eq!(curves(&gk), curves(&gs), "{mode}");
        let names = |svg: &str| texts(svg).into_iter().filter(|t| t.kind == "label" && !t.feat.starts_with("ann")).map(|t| (t.text, t.x.to_bits(), t.y.to_bits())).collect::<Vec<_>>();
        // the curve names are where they were on both copies
        for n in ["KNO₃", "NaNO₃", "NaCl", "KCl", "KClO₃", "NH₄Cl", "Pb(NO₃)₂"] {
            assert!(names(&ssvg).iter().any(|x| x.0 == n) && names(&ksvg).iter().any(|x| x.0 == n), "{mode}: {n}");
        }
        // the key has at least as many guides as the student copy, and more in every mode
        let guides = |g: &Geo| g.strokes.iter().filter(|x| x.0 == "guide").count();
        assert!(guides(&gk) > guides(&gs), "{mode}: {} against {}", guides(&gk), guides(&gs));
        assert_eq!(crate::figures::svg_size(&ksvg).0, crate::figures::svg_size(&ssvg).0);
    }
}

#[test]
fn the_description_is_short_ascii_and_answer_free() {
    for c in student_cases().into_iter().step_by(7) {
        for ans in [true, false] {
            let mut c = c.clone();
            c.push(b(SHOW_ANSWERS, ans));
            let p = with(&c);
            let (svg, _) = build(&p);
            let d = desc_of(&svg);
            assert!(!d.is_empty() && d.is_ascii() && d.len() <= 300, "{d:?}");
            assert!(!d.chars().any(|c| c.is_ascii_digit()), "a number in the description: {d}");
            let low = d.to_lowercase();
            assert!(!low.contains("unsaturated") && !low.contains("supersaturated") && !low.contains("crystallises"), "{d}");
            let t = title_of(&svg);
            assert!(t.is_ascii() && !t.chars().any(|c| c.is_ascii_digit()) && !t.is_empty(), "{t:?}");
        }
    }
    let svg = def().svg(&with(&[s("label_size", "Large")]));
    let (w, h) = crate::figures::svg_size(&svg);
    assert!(w > 800.0 && h > 450.0, "{w} {h}");
}

// ---------------------------------------------------------------------------------------------
// Style, determinism, the scale
// ---------------------------------------------------------------------------------------------

fn is_grey(h: &str) -> bool {
    let h: String = if h.len() == 4 { h[1..].chars().flat_map(|c| [c, c]).collect() } else { h[1..].to_string() };
    h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()) && { let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap(); c(0) == c(2) && c(2) == c(4) }
}

fn all_on() -> Vec<(&'static str, Value)> { DATA.iter().map(|r| b(r.0, true)).collect() }

#[test]
fn print_is_grey_only_and_every_curve_has_its_own_dash_and_marker() {
    for mode in [READ, CLASSIFY, CRYST, TEMP] {
        for ans in [true, false] {
            let mut list = all_on();
            list.extend([s("style", "Print"), s("mode", mode), b(SHOW_ANSWERS, ans), i("n_points", 3)]);
            let svg = svg_of(&list);
            for part in svg.split('"').filter(|x| x.starts_with('#')) { assert!(is_grey(part), "Print draws a hue: {part}"); }
            let g = geo(&svg);
            let curves: Vec<&(String, String, Vec<Pt>, String)> = g.paths.iter().filter(|p| p.0 == "curve").collect();
            assert_eq!(curves.len(), 10);
            // grey on grey: the dash must tell them apart, not the shade
            let dashes: Vec<String> = curves.iter().map(|c| c.3.rsplit('|').next().unwrap().to_string()).collect();
            let uniq: BTreeSet<&String> = dashes.iter().collect();
            assert_eq!(uniq.len(), 10, "{dashes:?}");
            // no curve is dashed like a guide
            let guide: BTreeSet<String> = g.paths.iter().filter(|p| p.0 == "guide").map(|p| p.3.rsplit('|').next().unwrap().to_string()).collect();
            for d in &dashes { assert!(!guide.contains(d), "{d} is the guides' dash"); }
            // and each has its own marker shape at its data points
            let mut shape: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for m in g.marks.iter().filter(|m| !m.0.starts_with("pt")) { shape.entry(m.0.clone()).or_default().insert(m.1.clone()); }
            assert_eq!(shape.len(), 10, "{shape:?}");
            assert!(shape.values().all(|v| v.len() == 1), "{shape:?}");
            let shapes: BTreeSet<&String> = shape.values().flatten().collect();
            assert_eq!(shapes.len(), 10, "{shape:?}");
            // each marker lies on its own curve
            for m in g.marks.iter().filter(|m| !m.0.starts_with("pt")) {
                let c = curves.iter().find(|c| c.1 == m.0).unwrap();
                let ctr = ((m.2 .0 + m.2 .1) / 2.0, (m.2 .2 + m.2 .3) / 2.0);
                assert!(pt_poly(ctr, &c.2) < 1.6, "{} marker off its curve", m.0);
            }
        }
    }
    // colour does use colour, one per curve
    let mut list = all_on();
    list.push(s("style", "Colour"));
    let g = geo(&svg_of(&list));
    let cols: BTreeSet<String> = g.paths.iter().filter(|p| p.0 == "curve").map(|p| p.3.split('|').next().unwrap().to_string()).collect();
    assert_eq!(cols.len(), 10);
    assert!(cols.iter().any(|c| !is_grey(c)));
}

#[test]
fn rendering_is_deterministic() {
    for l in [vec![], vec![s("mode", CRYST), s("style", "Print")], vec![s("label_size", "Large"), s("mode", CLASSIFY), i("n_points", 3)], vec![b(SHOW_ANSWERS, false)]] {
        assert_eq!(svg_of(&l), svg_of(&l));
    }
}

#[test]
fn the_axis_fits_the_largest_curve_shown() {
    let top = |list: &[(&'static str, Value)]| rep_of(list).frame.y1;
    let only = |name: &str| { let mut l = sub_params(name); l.push(s("mode", READ)); l };
    for r in DATA.iter() {
        let max = r.3.iter().map(|p| p.1).fold(0.0, f64::max);
        let y1 = top(&only(r.1));
        assert!(y1 >= max && y1 <= max * 1.4, "{}: axis top {y1} for a curve of {max}", r.1);
        assert_eq!(rep_of(&only(r.1)).frame.y0, 0.0);
    }
    // adding a bigger curve widens it; dropping it narrows it again
    assert!(top(&all_on()) >= 246.0);
    // the whole x range is 0 to 100
    let fr = rep_of(&[]).frame;
    assert_eq!((fr.x0, fr.x1), (0.0, 100.0));
}

#[test]
fn salts_and_gases_together_say_the_gases_lie_on_the_axis_and_the_pressure_is_stated() {
    let mut l = vec![s("mode", READ)];
    l.extend([b("s_co2", true), b("s_o2", true)]);
    let n = notes_of(&l);
    assert!(has(&n, "CO₂") && has(&n, "lie almost on the axis"), "{n:?}");
    let svg = svg_of(&l);
    let txt = all_text(&svg);
    assert!(txt.contains("1 atm") && txt.contains("21 kPa"), "{txt}");
    // gases alone are readable
    let mut l = sub_params("CO₂");
    l.push(s("mode", READ));
    assert!(notes_of(&l).is_empty(), "{:?}", notes_of(&l));
    // no gas, no pressure statement
    let txt = all_text(&svg_of(&[]));
    assert!(!txt.contains("partial pressure"), "{txt}");
}

// ---------------------------------------------------------------------------------------------
// Clamps, refusals and odd input
// ---------------------------------------------------------------------------------------------

#[test]
fn clamps_and_refusals_come_with_a_note() {
    let n = warnings(&raw(&[f("t1", 140.0)]));
    assert!(has(&n, "outside the 0 to 100 °C") && has(&n, "drawn at 100 °C"), "{n:?}");
    let n = warnings(&raw(&[f("t1", -30.0), s("mode", READ)]));
    assert!(has(&n, "outside the 0 to 100 °C"), "{n:?}");
    // beyond the data of a substance that stops short
    let mut l = sub_params("Ce₂(SO₄)₃");
    l.extend([f("t1", 80.0), s("mode", READ)]);
    let n = notes_of(&l);
    assert!(has(&n, "The data for Ce₂(SO₄)₃ stop at 60 °C"), "{n:?}");
    assert_eq!(rep_of(&l).t1, 60.0);
    let mut l = sub_params("O₂");
    l.extend([f("t1", 45.0), f("t2", 90.0), s("mode", CRYST)]);
    let n = notes_of(&l);
    assert!(has(&n, "The data for O₂ stop at 40 °C"), "{n:?}");
    // water mass
    for w in [0.0, -5.0] {
        let r = raw(&[f("water", w), s("mode", CLASSIFY)]);
        let n = warnings(&r);
        assert!(has(&n, "mass of water must be greater than 0"), "{n:?}");
        assert_eq!(build(&r).1.water, 100.0);
    }
    // negative solute
    let r = raw(&[f("solute", -3.0), s("mode", CLASSIFY)]);
    assert!(has(&warnings(&r), "mass dissolved cannot be negative"), "{:?}", warnings(&r));
    assert_eq!(build(&r).1.solute, 0.0);
    // zero solute is allowed
    let n = notes_of(&[f("solute", 0.0), s("mode", CLASSIFY)]);
    assert!(n.is_empty(), "{n:?}");
    // four significant figures
    let n = notes_of(&[f("solute", 12.345678), s("mode", CLASSIFY)]);
    assert!(has(&n, "kept to 4 significant figures"), "{n:?}");
    // the substance of the question is always drawn
    let mut l = sub_params("KClO₃");
    l.push(b("s_kclo3", false));
    assert!(has(&notes_of(&l), "is the substance of the question"), "{:?}", notes_of(&l));
    assert!(rep_of(&l).shown.contains(&4));
    // nothing shown but the question's own substance is fine
    let rep = rep_of(&{ let mut l = sub_params("NaCl"); l.push(b("s_kno3", false)); l });
    assert_eq!(rep.shown, vec![2]);
    // an off-scale point is drawn at the top with a note
    let n = notes_of(&[s("mode", CLASSIFY), f("solute", 100000.0), f("water", 100.0)]);
    assert!(has(&n, "off the scale"), "{n:?}");
    let n = notes_of(&[s("mode", TEMP), f("solute", 100000.0), f("water", 100.0)]);
    assert!(layout_only(&n) || has(&n, "off the scale"), "{n:?}");
    // a close point is on the scale and has no note
    assert!(notes_of(&[s("mode", CLASSIFY), f("solute", 200.0)]).is_empty());
}

#[test]
fn odd_inputs_never_panic_or_print_nan() {
    let nan = f64::NAN;
    let inf = f64::INFINITY;
    let vals = [nan, inf, -inf, -1e300, -5.0, 0.0, 1e-300, 0.5, 33.0, 100.0, 1e6, 1e300];
    for mode in [READ, CLASSIFY, CRYST, TEMP, "bogus"] {
        for &a in &vals {
            for &c in &vals {
                for ans in [true, false] {
                    let r = raw(&[s("mode", mode), f("t1", a), f("t2", c), f("water", c), f("solute", a), f("pb_t", c), f("pb_m", a), f("pc_t", a), f("pc_m", c), i("n_points", 3), b(SHOW_ANSWERS, ans), i("dec", 7), s("substance", if a > 1.0 { "O₂" } else { "KNO₃" })]);
                    let (svg, rep) = build(&r);
                    let svg = svg.replace("NaNO₃", "");
                    let at = svg.find("NaN").or_else(|| svg.find("inf")).unwrap_or(0);
                    assert!(!svg.contains("NaN") && !svg.contains("inf"), "{mode} {a} {c}: ...{}...", &svg[at.saturating_sub(60)..(at + 60).min(svg.len())]);
                    assert!(!joined(&rep).replace("NaNO₃", "").contains("NaN") && !joined(&rep).contains("inf"), "{mode} {a} {c}: {}", joined(&rep));
                    let _ = handles(ID, &r, (rep.frame.w, rep.frame.h));
                }
            }
        }
    }
    // wrong value types
    let mut r = raw(&[]);
    for k in ["t1", "water", "solute", "mode", "substance", "dec", "s_kno3", "n_points"] { r.0.insert(k.to_string(), Value::S("x".into())); }
    let _ = build(&r);
    let mut r = raw(&[]);
    for k in ["t1", "water", "solute", "mode", "substance", "dec", "s_kno3", "n_points"] { r.0.insert(k.to_string(), Value::B(true)); }
    let _ = build(&r);
    // an unknown substance falls back to the default
    assert_eq!(build(&raw(&[s("substance", "Unobtainium")])).1.sub, 0);
}

#[test]
fn huge_masses_print_without_a_wall_of_digits() {
    let rep = key(&cryst_list("KNO₃", 100.0, 0.0, 1e6, 1));
    let t = joined(&rep);
    assert!(t.contains("Mass that crystallises = (246.0 − 13.3) × 1000000 / 100 = "), "{t}");
    assert!(t.lines().all(|l| l.len() < 220), "{t}");
}

// ---------------------------------------------------------------------------------------------
// Labels: the sweep, the audit's own check, the large labels
// ---------------------------------------------------------------------------------------------

fn sweep_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases: Vec<Vec<(&'static str, Value)>> = Vec::new();
    let names: Vec<&str> = DATA.iter().map(|r| r.1).collect();
    // the classroom grid
    for mode in [READ, CLASSIFY, CRYST, TEMP] {
        for style in ["Colour", "Print"] {
            for size in ["Small", "Medium", "Large"] {
                for ans in [true, false] {
                    for points in [true, false] {
                        for set in 0..9usize {
                            let mut c: Vec<(&'static str, Value)> = vec![s("mode", mode), s("style", style), s("label_size", size), b(SHOW_ANSWERS, ans), b("points", points), b("grid", set % 2 == 0)];
                            let shown: Vec<usize> = match set {
                                0 => vec![0, 1, 2, 3, 4, 5],
                                1 => (0..10).collect(),
                                2 => (0..8).collect(),
                                3 => vec![2, 3, 5],
                                4 => vec![8, 9],
                                5 => vec![0],
                                6 => vec![0, 1, 6],
                                7 => vec![7, 3, 2, 4, 6],
                                _ => vec![2, 3, 5, 1, 0, 4],
                            };
                            for (k, r) in DATA.iter().enumerate() { c.push(b(r.0, shown.contains(&k))); }
                            c.push(s("substance", names[shown[set % shown.len()]]));
                            cases.push(c);
                        }
                    }
                }
            }
        }
    }
    // random over the whole space
    let mut r: u64 = 29;
    let mut next = move || { r = r.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (r >> 11) as f64 / (1u64 << 53) as f64 };
    let pick = |o: &[&'static str], x: f64| o[((x * o.len() as f64) as usize).min(o.len() - 1)];
    while cases.len() < 10_500 {
        let mut c: Vec<(&'static str, Value)> = vec![
            s("mode", pick(&[READ, CLASSIFY, CRYST, TEMP], next())), s("style", pick(&["Colour", "Print"], next())), s("label_size", pick(&["Small", "Medium", "Large"], next())), b(SHOW_ANSWERS, next() < 0.5),
            b("points", next() < 0.6), b("grid", next() < 0.8), f("t1", (next() * 100.0).round()), f("t2", (next() * 100.0).round()), f("water", 10f64.powf(next() * 3.0).round()), f("solute", (next() * 300.0).round()),
            i("n_points", 1 + (next() * 3.0) as i64), f("pb_t", (next() * 100.0).round()), f("pb_m", (next() * 300.0).round()), f("pc_t", (next() * 100.0).round()), f("pc_m", (next() * 300.0).round()), i("dec", (next() * 3.0) as i64),
            s("substance", pick(&names, next())),
        ];
        for r in DATA.iter() { c.push(b(r.0, next() < 0.55)); }
        cases.push(c);
    }
    cases
}

#[test]
fn labels_clear_and_attributed_across_ten_thousand_cases_in_real_glyphs() {
    let step: usize = std::env::var("VERSHION_SWEEP_STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let cases: Vec<_> = sweep_cases().into_iter().step_by(step).collect();
    assert!(cases.len() >= 10_000 / step);
    let chunks: Vec<&[Vec<(&'static str, Value)>]> = cases.chunks(cases.len().div_ceil(4)).collect();
    let results: Vec<(usize, usize, Vec<String>, usize)> = std::thread::scope(|sc| {
        let hs: Vec<_> = chunks.iter().map(|ch| sc.spawn(move || {
            let mut g = Glyphs::new();
            let mut bad = Vec::new();
            let mut labels = 0;
            let mut leaders = 0;
            let mut dropped = 0;
            for c in ch.iter() {
                let p = with(c);
                let (svg, rep) = build(&p);
                dropped += rep.notes.iter().filter(|n| n.contains("could not be placed")).count();
                let ts = texts(&svg);
                labels += ts.iter().filter(|t| t.kind == "label").count();
                leaders += svg.matches(r#"data-o="leader""#).count();
                let lab = svg::label_factor(p.s("label_size"));
                for x in audit(&svg, &mut g, lab) { bad.push(format!("{x}  in {c:?}")); }
            }
            (labels, leaders, bad, dropped)
        })).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let labels: usize = results.iter().map(|r| r.0).sum();
    let leaders: usize = results.iter().map(|r| r.1).sum();
    let bad: Vec<&String> = results.iter().flat_map(|r| r.2.iter()).collect();
    if let Ok(out) = std::env::var("VERSHION_SOL_AUDIT_OUT") { std::fs::write(out, bad.iter().map(|x| x.as_str()).collect::<Vec<_>>().join("\n")).unwrap(); }
    let dropped: usize = results.iter().map(|r| r.3).sum();
    println!("labels {labels}, leader lines {leaders}, dropped {dropped}, problems {}", bad.len());
    assert!(labels > 60_000 / step, "only {labels} labels placed");
    assert!(leaders > 0, "the sweep never needed a leader line");
    assert!(bad.is_empty(), "{} problems in {labels} labels, first: {:#?}", bad.len(), &bad[..bad.len().min(12)]);
}

/// Every name of every swept case is placed (a leader line where it has no room beside its curve).
#[test]
fn no_name_is_left_out_in_any_swept_case() {
    let mut lost: Vec<String> = Vec::new();
    let mut total = 0;
    let step: usize = std::env::var("VERSHION_SWEEP_STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    for c in sweep_cases().into_iter().step_by(step) {
        let rep = build(&with(&c)).1;
        total += 1;
        for n in rep.notes.iter().filter(|n| n.contains("could not be placed")) { lost.push(format!("{n} in {c:?}")); }
    }
    if let Ok(out) = std::env::var("VERSHION_SOL_DROP_OUT") { std::fs::write(out, lost.join("\n")).unwrap(); }
    assert!(total >= 10_000 / step);
    assert!(lost.is_empty(), "{} names left out of {total} cases, first: {:#?}", lost.len(), &lost[..lost.len().min(6)]);
}

#[test]
fn the_defaults_name_every_curve_and_a_dropped_label_is_reported() {
    for size in ["Small", "Medium", "Large"] {
        for ans in [true, false] {
            for mode in [READ, CLASSIFY, CRYST, TEMP] {
                let mut l = all_on();
                l.extend([s("label_size", size), b(SHOW_ANSWERS, ans), s("mode", mode), i("n_points", 3)]);
                let p = with(&l);
                let (svg, rep) = build(&p);
                for r in DATA.iter() {
                    let named = texts(&svg).iter().any(|t| t.kind == "label" && t.text == r.1);
                    assert!(named || has(&rep.notes, "could not be placed"), "{size} {ans} {mode}: {} unnamed and no note: {:?}", r.1, rep.notes);
                }
            }
        }
    }
    // the plain defaults need no note at all
    for size in ["Small", "Medium", "Large"] {
        let n = rep_of(&[s("label_size", size)]).notes;
        assert!(n.is_empty(), "{size} {n:?}");
    }
}

#[test]
fn the_audit_catches_what_it_should() {
    let mut g = Glyphs::new();
    let svg = svg_of(&[s("mode", CLASSIFY)]);
    assert!(audit(&svg, &mut g, 1.0).is_empty(), "{:?}", audit(&svg, &mut g, 1.0));
    let ts = texts(&svg);
    let group: Vec<T> = ts.iter().filter(|t| t.kind == "label" && t.feat == "s_kcl").cloned().collect();
    assert!(!group.is_empty(), "{:?}", ts.iter().map(|t| (&t.text, &t.feat)).collect::<Vec<_>>());
    let lab = group[0].clone();
    let gm = geo(&svg);
    let curve: Vec<Pt> = gm.paths.iter().find(|p| p.0 == "curve" && p.1 == "s_kcl").unwrap().2.clone();
    let on = curve[curve.len() / 3];
    let moved = |x: f64, y: f64| {
        let mut out = svg.clone();
        for t in &group {
            let from = format!(r#"x="{:.2}" y="{:.2}""#, t.x, t.y);
            let to = format!(r#"x="{:.2}" y="{:.2}""#, t.x + x - lab.x, t.y + y - lab.y);
            out = out.replace(&from, &to);
        }
        out
    };
    assert!(audit(&moved(on.0, on.1 + 4.0), &mut g, 1.0).iter().any(|x| x.contains("stroke")), "a label on the curve");
    assert!(audit(&moved(120.0, 60.0), &mut g, 1.0).iter().any(|x| x.contains("from its s_kcl feature") || x.contains("stroke") || x.contains("leader")), "a label far off: {:?}", audit(&moved(120.0, 60.0), &mut g, 1.0));
    assert!(audit(&moved(-30.0, 40.0), &mut g, 1.0).iter().any(|x| x.contains("leaves")), "a label off the canvas");
    // two names on each other
    let other: Vec<T> = ts.iter().filter(|t| t.kind == "label" && t.feat == "s_nacl").cloned().collect();
    let mut out = svg.clone();
    for t in &other {
        let from = format!(r#"x="{:.2}" y="{:.2}""#, t.x, t.y);
        let to = format!(r#"x="{:.2}" y="{:.2}""#, lab.x, lab.y);
        out = out.replace(&from, &to);
    }
    assert!(audit(&out, &mut g, 1.0).iter().any(|x| x.contains(" on ")), "text on text");
}

#[test]
fn large_labels_grow_the_canvas_and_nothing_is_clipped() {
    let mut g = Glyphs::new();
    let mut last = 0.0;
    for size in ["Small", "Medium", "Large"] {
        let mut l = all_on();
        l.extend([s("label_size", size), s("mode", CRYST)]);
        let svg = svg_of(&l);
        let (w, h) = crate::figures::svg_size(&svg);
        assert!(w * h > last, "{size}");
        last = w * h;
        assert!(audit(&svg, &mut g, svg::label_factor(size)).is_empty(), "{size}: {:?}", audit(&svg, &mut g, svg::label_factor(size)));
    }
}

#[test]
fn leader_lines_join_names_that_have_no_room_beside_their_curve() {
    // crowded ends: NaCl, KCl and KClO3 end within a few pixels of each other at the right edge, and the gases lie on the axis
    let mut l = all_on();
    l.push(s("mode", READ));
    let svg = svg_of(&l);
    let g = geo(&svg);
    assert!(g.paths.iter().any(|p| p.0 == "leader"), "no leader line with ten curves");
    let mut gl = Glyphs::new();
    assert!(audit(&svg, &mut gl, 1.0).is_empty(), "{:?}", audit(&svg, &mut gl, 1.0));
    // two curves ending at the same height still get two names, apart
    let mut l = sub_params("KCl");
    l.extend([b("s_kclo3", true), s("mode", READ)]);
    let svg = svg_of(&l);
    assert!(texts(&svg).iter().filter(|t| t.kind == "label" && (t.text == "KCl" || t.text == "KClO₃")).count() == 2);
    assert!(audit(&svg, &mut gl, 1.0).is_empty(), "{:?}", audit(&svg, &mut gl, 1.0));
}

#[test]
fn two_curves_crossing_at_the_question_point_are_told_apart() {
    // KCl and NH4Cl cross near 80? no: NaNO3 and KNO3 cross between 40 and 50 deg C
    let mut l = sub_params("KNO₃");
    l.extend([b("s_nano3", true), s("mode", READ), f("t1", 44.0)]);
    let rep = key(&l);
    let svg = svg_of(&{ let mut q = l.clone(); q.push(b(SHOW_ANSWERS, true)); q });
    let g = geo(&svg);
    // the question's point sits on the question's curve, not the other
    let pt = g.marks.iter().find(|m| m.0.starts_with("pt")).expect("a point mark");
    let c = ((pt.2 .0 + pt.2 .1) / 2.0, (pt.2 .2 + pt.2 .3) / 2.0);
    let own = g.paths.iter().find(|p| p.0 == "curve" && p.1 == "s_kno3").unwrap();
    assert!(pt_poly(c, &own.2) < 1.2);
    // the reading is KNO3's, whichever curve is higher
    assert!(joined(&rep).contains(&ref_round(ref_s("KNO₃", 44.0), 1)), "{}", joined(&rep));
    let mut gl = Glyphs::new();
    assert!(audit(&svg, &mut gl, 1.0).is_empty(), "{:?}", audit(&svg, &mut gl, 1.0));
}

// ---------------------------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------------------------

#[test]
fn handles_drag_the_temperatures_and_the_plotted_point() {
    let vx = |fr: &Frame, x: f64| fr.x0 + (x - fr.left()) / (fr.right() - fr.left()) * (fr.x1 - fr.x0);
    // reading: one knob on the curve at T1
    let base = with(&[s("mode", READ), f("t1", 40.0)]);
    let rep = build(&base).1;
    let fr = rep.frame;
    let hs = handles(ID, &base, (fr.w, fr.h)).unwrap();
    assert_eq!(hs.len(), 1);
    assert!((hs[0].pos.0 - fr.px(40.0)).abs() < 1e-6 && (hs[0].pos.1 - fr.py(63.9)).abs() < 1e-6, "{:?}", hs[0].pos);
    let mut q = base.clone();
    (hs[0].drag.clone().unwrap())(&mut q, (fr.px(72.4), fr.py(10.0)));
    assert!((q.f("t1") - 72.0).abs() < 1e-9, "{}", q.f("t1"));
    // dragging beyond the graph holds to the curve's data
    (hs[0].drag.clone().unwrap())(&mut q, (fr.px(400.0), 0.0));
    assert_eq!(q.f("t1"), 100.0);
    (hs[0].drag.clone().unwrap())(&mut q, (-50.0, 0.0));
    assert_eq!(q.f("t1"), 0.0);
    // a drag to where it is changes nothing
    let mut q = base.clone();
    (hs[0].drag.clone().unwrap())(&mut q, (fr.px(40.0), fr.py(63.9)));
    assert_eq!(q.0, base.0);
    // crystallisation: T1 and T2
    let base = with(&[s("mode", CRYST), f("t1", 80.0), f("t2", 20.0)]);
    let fr = build(&base).1.frame;
    let hs = handles(ID, &base, (fr.w, fr.h)).unwrap();
    assert_eq!(hs.len(), 2);
    let mut q = base.clone();
    (hs.iter().find(|h| h.tip.contains("T₂")).unwrap().drag.clone().unwrap())(&mut q, (fr.px(35.0), 0.0));
    assert_eq!((q.f("t1"), q.f("t2")), (80.0, 35.0));
    (hs.iter().find(|h| h.tip.contains("T₁")).unwrap().drag.clone().unwrap())(&mut q, (fr.px(90.0), 0.0));
    assert_eq!((q.f("t1"), q.f("t2")), (90.0, 35.0));
    let _ = vx;
    // classification: a knob at each point moves its temperature and its mass per 100 g
    let base = with(&[s("mode", CLASSIFY), f("water", 200.0), f("solute", 100.0), i("n_points", 2), f("pb_t", 60.0), f("pb_m", 150.0)]);
    let fr = build(&base).1.frame;
    let hs = handles(ID, &base, (fr.w, fr.h)).unwrap();
    assert_eq!(hs.len(), 2);
    assert!((hs[0].pos.0 - fr.px(40.0)).abs() < 1e-6 && (hs[0].pos.1 - fr.py(50.0)).abs() < 1e-6, "{:?}", hs[0].pos);
    let mut q = base.clone();
    (hs[0].drag.clone().unwrap())(&mut q, (fr.px(55.0), fr.py(80.0)));
    assert_eq!(q.f("t1"), 55.0);
    assert!((q.f("solute") - 160.0).abs() < 0.6 && q.f("water") == 200.0, "{}", q.f("solute"));
    (hs[1].drag.clone().unwrap())(&mut q, (fr.px(10.0), fr.py(20.0)));
    assert_eq!(q.f("pb_t"), 10.0);
    assert!((q.f("pb_m") - 40.0).abs() < 0.2, "{}", q.f("pb_m"));
    assert!(layout_only(&warnings(&q)), "{:?}", warnings(&q));
    // temperature of saturation: the knob moves the line's height only
    let base = with(&[s("mode", TEMP), f("water", 100.0), f("solute", 60.0)]);
    let fr = build(&base).1.frame;
    let hs = handles(ID, &base, (fr.w, fr.h)).unwrap();
    assert_eq!(hs.len(), 1);
    let mut q = base.clone();
    (hs[0].drag.clone().unwrap())(&mut q, (fr.px(10.0), fr.py(120.0)));
    assert!((q.f("solute") - 120.0).abs() < 0.5 && q.f("t1") == 40.0, "{} {}", q.f("solute"), q.f("t1"));
    // every handle's position is where the figure draws its mark
    for mode in [READ, CLASSIFY, CRYST, TEMP] {
        let p = with(&[s("mode", mode), i("n_points", 3), f("solute", 60.0)]);
        let (svg, rep) = build(&p);
        let g = geo(&svg);
        for h in handles(ID, &p, (rep.frame.w, rep.frame.h)).unwrap() {
            assert!(g.marks.iter().any(|m| m.0.starts_with("pt") && ((m.2 .0 + m.2 .1) / 2.0 - h.pos.0).abs() < 1.0 && ((m.2 .2 + m.2 .3) / 2.0 - h.pos.1).abs() < 1.0) || mode == TEMP, "{mode}: {:?}", h.pos);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Pictures for looking at
// ---------------------------------------------------------------------------------------------

#[test]
fn solubility_pngs() {
    let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
    let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
    for (tag, mode) in [("read", READ), ("classify", CLASSIFY), ("cryst", CRYST), ("temp", TEMP)] {
        cases.push((format!("{tag}-six"), vec![s("mode", mode), i("n_points", 3), f("t2", 20.0), f("t1", 60.0), f("solute", 60.0)]));
        let mut all = all_on();
        all.extend([s("mode", mode), i("n_points", 3), f("t1", 60.0), f("t2", 20.0), f("solute", 60.0)]);
        cases.push((format!("{tag}-ten"), all));
        let mut one = sub_params("KNO₃");
        one.extend([s("mode", mode), i("n_points", 3), f("t1", 80.0), f("t2", 20.0), f("water", 200.0), f("solute", 90.0)]);
        cases.push((format!("{tag}-one"), one));
        let mut ce = sub_params("Ce₂(SO₄)₃");
        ce.extend([s("mode", mode), f("t1", 20.0), f("t2", 50.0), f("water", 100.0), f("solute", 7.24), i("n_points", 3), f("pb_t", 30.0), f("pb_m", 7.24), f("pc_t", 50.0), f("pc_m", 8.0)]);
        cases.push((format!("{tag}-falling"), ce));
        let mut gas = sub_params("CO₂");
        gas.extend([b("s_o2", true), s("mode", mode), f("t1", 50.0), f("t2", 10.0), f("water", 1000.0), f("solute", 2.0), i("n_points", 2), f("pb_t", 20.0), f("pb_m", 1.688)]);
        cases.push((format!("{tag}-gas"), gas));
    }
    cases.push(("fixsix".into(), vec![s("mode", READ)]));
    cases.push(("fixcrystten".into(), { let mut g = all_on(); g.extend([s("mode", CRYST), f("t1", 40.0), f("t2", 20.0)]); g }));
    cases.push(("fixclassco2".into(), { let mut g = sub_params("CO₂"); g.extend([s("mode", CLASSIFY), i("n_points", 3), f("t1", 20.0), f("solute", 0.169), f("pb_t", 40.0), f("pb_m", 0.2), f("pc_t", 10.0), f("pc_m", 0.05)]); g }));
    cases.push(("fixclassoff".into(), { let mut g = sub_params("KNO₃"); g.extend([s("mode", CLASSIFY), i("n_points", 3), f("t1", 40.0), f("solute", 900.0), f("pb_t", 60.0), f("pb_m", 5000.0), f("pc_t", 20.0), f("pc_m", 20.0)]); g }));
    cases.push(("fixtempa".into(), { let mut g = sub_params("KNO₃"); g.extend([s("mode", TEMP), f("solute", 60.0), f("water", 100.0)]); g }));
    cases.push(("fixtempb".into(), { let mut g = sub_params("KNO₃"); g.extend([s("mode", TEMP), f("solute", 5.0), f("water", 100.0)]); g }));
    cases.push(("fixtempnacl".into(), { let mut g = sub_params("NaCl"); g.extend([s("mode", TEMP), f("solute", 35.66), f("water", 100.0)]); g }));
    cases.push(("gas-cooled".into(), { let mut g = sub_params("CO₂"); g.extend([s("mode", CRYST), f("t1", 10.0), f("t2", 50.0), f("water", 1000.0)]); g }));
    cases.push(("mixed".into(), vec![s("mode", READ), b("s_co2", true), b("s_o2", true)]));
    for (i_, e) in examples(ID).unwrap().into_iter().enumerate() { cases.push((format!("ex{i_}"), e.params)); }
    for (tag, list) in cases {
        for size in ["Small", "Medium", "Large"] {
            for (kind, extra) in [("key", vec![b(SHOW_ANSWERS, true)]), ("student", vec![b(SHOW_ANSWERS, false)]), ("print", vec![s("style", "Print")]), ("printstudent", vec![s("style", "Print"), b(SHOW_ANSWERS, false)])] {
                if size != "Medium" && !(tag.ends_with("-ten") || tag == "cryst-six") { continue; }
                if kind == "printstudent" && size != "Medium" { continue; }
                let mut l = list.clone();
                l.extend(extra);
                l.push(s("label_size", size));
                let svg = svg_of(&l);
                std::fs::write(format!("{dir}/sol-{tag}-{size}-{kind}.png"), to_png(&svg, 1.5).unwrap()).unwrap();
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fix round: gas wording, the student copy of the temperature question, attribution, leaders, flat curves
// ---------------------------------------------------------------------------------------------

fn every_text(l: &[(&'static str, Value)]) -> String {
    let p = with(l);
    let (svg, rep) = build(&p);
    format!("{}\n{}\n{}\n{}", all_text(&svg), rep.notes.join("\n"), rep.readout.join("\n"), desc_of(&svg))
}

#[test]
fn a_gas_never_crystallises_in_any_text_of_any_mode() {
    let mut n = 0;
    for name in ["CO₂", "O₂"] {
        let top = if name == "CO₂" { 0.5 } else { 0.002 };
        for mode in [READ, CLASSIFY, CRYST, TEMP] {
            for ans in [true, false] {
                for (t1, t2) in [(20.0, 40.0), (40.0, 20.0), (30.0, 30.0)] {
                    for dec in 0..=2 {
                        for fac in [0.0, 0.3, 0.7, 1.0, 1.4, 3.0] {
                            let mut l = sub_params(name);
                            l.extend([s("mode", mode), f("t1", t1), f("t2", t2), i("dec", dec), b(SHOW_ANSWERS, ans), i("n_points", 3), f("water", 100.0), f("solute", sig4(top * 0.34 * fac)), f("pb_t", 40.0), f("pb_m", sig4(top * 0.2 * fac)), f("pc_t", 10.0), f("pc_m", sig4(top * 0.4 * fac))]);
                            let t = every_text(&l);
                            n += 1;
                            assert!(!t.to_lowercase().contains("crystallis"), "{name} {mode} ans={ans} t1={t1} t2={t2} dec={dec} fac={fac}: {t}");
                        }
                    }
                }
            }
        }
    }
    assert!(n > 800);
    // the reported case: a supersaturated gas says the gas comes out, in the words of the other modes
    let mut l = sub_params("CO₂");
    l.extend([s("mode", CLASSIFY), i("n_points", 3), f("t1", 20.0), f("solute", 0.169), f("pb_t", 40.0), f("pb_m", 0.2), b(SHOW_ANSWERS, true)]);
    let t = every_text(&l);
    assert!(t.contains("supersaturated") && t.contains("gas") && t.contains("comes out of solution"), "{t}");
    // a salt still says it crystallises, so the words are not simply removed
    let mut l = sub_params("KNO₃");
    l.extend([s("mode", CLASSIFY), f("solute", 90.0), b(SHOW_ANSWERS, true)]);
    assert!(every_text(&l).contains("crystallise"), "{}", every_text(&l));
}

/// What the student copy draws that is not a curve, a table point, the grid or the axes: kind, feature and the
/// horizontal extent, rounded (the height of a guide is the given amount, the same whatever the answer).
fn skeleton(svg: &str) -> Vec<String> {
    let g = geo(svg);
    let mut v: Vec<String> = Vec::new();
    for (kind, feat, pts, style) in &g.paths {
        if kind == "guide" {
            let (a, z) = (pts.iter().map(|p| p.0).fold(f64::MAX, f64::min), pts.iter().map(|p| p.0).fold(f64::MIN, f64::max));
            v.push(format!("guide {feat} x {a:.1}..{z:.1} {style}"));
        }
    }
    for (feat, shape, bx) in &g.marks {
        if feat.starts_with("pt") { v.push(format!("mark {feat} {shape} x {:.1}", (bx.0 + bx.1) / 2.0)); }
    }
    v.sort();
    v
}

#[test]
fn the_student_copy_of_the_temperature_question_draws_the_same_whatever_the_answer() {
    let only = |name: &str, m: f64| { let mut l = sub_params(name); l.extend([s("mode", TEMP), f("solute", m), f("water", 100.0), b(SHOW_ANSWERS, false)]); l };
    let with_answer = svg_of(&only("KNO₃", 60.0));
    // the answer is a dot on the key and nothing on the student copy; the guide runs the whole width in every case
    for (name, m) in [("KNO₃", 60.0), ("KNO₃", 5.0), ("KNO₃", 300.0), ("KNO₃", 13.3), ("KNO₃", 246.0), ("KClO₃", 20.0), ("Ce₂(SO₄)₃", 7.24)] {
        let svg = svg_of(&only(name, m));
        let right = rep_of(&only(name, m)).frame.right();
        let g = geo(&svg);
        assert!(!g.marks.iter().any(|x| x.0 == "ptT"), "{name} {m}: a dot on the student copy");
        assert!(!g.paths.iter().any(|x| x.1 == "gTv"), "{name} {m}: a vertical guide on the student copy");
        let gy = g.paths.iter().find(|x| x.0 == "guide" && x.1 == "gy").unwrap_or_else(|| panic!("{name} {m}: no guide"));
        let x_end = gy.2.iter().map(|p| p.0).fold(f64::MIN, f64::max);
        assert!((x_end - right).abs() < 0.6, "{name} {m}: the guide stops at {x_end}, the frame ends at {right}");
    }
    // the same geometry with an answer and without one
    let a = skeleton(&with_answer);
    // (the amounts written alike, so the margin that holds the given amount is the same width)
    for m in [10.0, 12.0, 11.5, 99.0] {
        let b_ = skeleton(&svg_of(&only("KNO₃", m)));
        assert_eq!(a, b_, "KNO3 {m}");
    }
    let a2 = skeleton(&svg_of(&only("KNO₃", 100.0)));
    for m in [260.0, 270.0, 300.0] { assert_eq!(a2, skeleton(&svg_of(&only("KNO₃", m))), "KNO3 {m}"); }
    // the key still marks the answer
    let mut k = only("KNO₃", 60.0);
    k.push(b(SHOW_ANSWERS, true));
    assert!(geo(&svg_of(&k)).marks.iter().any(|x| x.0 == "ptT"));
    // and an amount that never saturates stays a full line on the key
    let mut k = only("KNO₃", 300.0);
    k.push(b(SHOW_ANSWERS, true));
    assert!(!geo(&svg_of(&k)).marks.iter().any(|x| x.0 == "ptT"));
}

fn audit_case(l: &[(&'static str, Value)]) -> Vec<String> {
    let p = with(l);
    let svg = build(&p).0;
    audit(&svg, &mut Glyphs::new(), svg::label_factor(p.s("label_size")))
}

#[test]
fn the_default_six_curves_are_named_by_colour_marker_and_a_leader_where_the_end_is_crowded() {
    for size in ["Small", "Medium", "Large"] {
        for style in ["Colour", "Print"] {
            for mode in [READ, CLASSIFY, CRYST, TEMP] {
                for ans in [true, false] {
                    let l = vec![s("label_size", size), s("style", style), s("mode", mode), b(SHOW_ANSWERS, ans), i("n_points", 3)];
                    let bad = audit_case(&l);
                    assert!(bad.is_empty(), "{size} {style} {mode} ans={ans}: {bad:?}");
                }
            }
        }
    }
    let six = ["KNO₃", "NaNO₃", "NaCl", "KCl", "KClO₃", "NH₄Cl"];
    // colour: each name has the colour of its curve (darkened to read on white), and no two are alike
    let svg = svg_of(&[s("style", "Colour")]);
    let ts: Vec<T> = texts(&svg).into_iter().filter(|t| t.kind == "label" && six.contains(&t.text.as_str())).collect();
    assert_eq!(ts.len(), 6);
    let fills: BTreeSet<String> = ts.iter().map(|t| t.fill.clone()).collect();
    assert_eq!(fills.len(), 6, "{fills:?}");
    for t in &ts {
        assert!(t.fill != "#1b1b1b" && t.fill != "#111111", "{} is black", t.text);
        let k = SUBS.iter().find(|x| x.name == t.text).unwrap();
        let (cr, cg, cb) = hex(k.colour);
        let (lr, lg, lb) = hex(&t.fill);
        // the same hue: each channel no brighter than the curve's, and the order of the channels the same
        assert!(lr <= cr + 1 && lg <= cg + 1 && lb <= cb + 1, "{} {} against {}", t.text, t.fill, k.colour);
        assert!(((cr as i32 - cg as i32) * (lr as i32 - lg as i32)) >= 0 && ((cg as i32 - cb as i32) * (lg as i32 - lb as i32)) >= 0, "{} {} against {}", t.text, t.fill, k.colour);
        // dark enough to read: contrast of at least 4.5 against white
        assert!(contrast(lr, lg, lb) >= 4.5, "{} {}: {:.2}", t.text, t.fill, contrast(lr, lg, lb));
    }
    // print: grey only, and each name carries the marker shape of its curve
    let svg = svg_of(&[s("style", "Print")]);
    let g = geo(&svg);
    for t in texts(&svg).into_iter().filter(|t| t.kind == "label" && six.contains(&t.text.as_str())) {
        let k = SUBS.iter().find(|x| x.name == t.text).unwrap();
        let km: Vec<_> = g.keys.iter().filter(|x| x.1 == t.group).collect();
        assert_eq!(km.len(), 1, "{}: {} key markers", t.text, km.len());
        assert_eq!(km[0].2, k.marker, "{}", t.text);
        assert_eq!(km[0].0, k.key, "{}", t.text);
        assert!(is_grey(&t.fill), "{} {}", t.text, t.fill);
    }
    // the end labels read in the order of the curves' ends, from the top, and keep clear of each other
    for style in ["Colour", "Print"] {
        let svg = svg_of(&[s("style", style)]);
        let mut ends: Vec<(f64, f64, String)> = Vec::new();
        for t in texts(&svg).into_iter().filter(|t| t.kind == "label" && six.contains(&t.text.as_str())) {
            let k = SUBS.iter().position(|x| x.name == t.text).unwrap();
            let rep = rep_of(&[s("style", style)]);
            let end_y = rep.frame.py(SUBS[k].data[SUBS[k].data.len() - 1].1);
            ends.push((end_y, t.y, t.text.clone()));
        }
        ends.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
        for w in ends.windows(2) {
            if (w[0].0 - w[1].0).abs() > 0.5 { assert!(w[0].1 < w[1].1, "{style}: {} should be above {}: {ends:?}", w[0].2, w[1].2); }
            assert!((w[1].1 - w[0].1).abs() >= 13.0, "{style}: {} and {} are too close: {ends:?}", w[0].2, w[1].2);
        }
    }
}

fn hex(h: &str) -> (u8, u8, u8) { let v = |a: usize| u8::from_str_radix(&h[a..a + 2], 16).unwrap(); (v(1), v(3), v(5)) }
fn contrast(r: u8, g: u8, b: u8) -> f64 {
    let lin = |c: u8| { let c = c as f64 / 255.0; if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) } };
    let l = 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
    1.05 / (l + 0.05)
}

#[test]
fn crystallising_with_every_curve_on_keeps_the_temperature_leaders_clear_of_other_markers() {
    for (t1, t2) in [(40.0, 20.0), (60.0, 20.0), (80.0, 20.0), (50.0, 10.0), (40.0, 30.0)] {
        for size in ["Small", "Medium", "Large"] {
            for style in ["Colour", "Print"] {
                for ans in [true, false] {
                    let mut l = all_on();
                    l.extend([s("mode", CRYST), f("t1", t1), f("t2", t2), s("label_size", size), s("style", style), b(SHOW_ANSWERS, ans)]);
                    let bad = audit_case(&l);
                    assert!(bad.is_empty(), "{t1}->{t2} {size} {style} ans={ans}: {bad:?}");
                    let svg = svg_of(&l);
                    // the two temperatures are named at the foot of their guides, on the axis
                    for t in ["T₁ = ", "T₂ = "] { assert!(texts(&svg).iter().any(|x| x.text.starts_with(t)), "{t1}->{t2}: no {t}"); }
                }
            }
        }
    }
}

#[test]
fn the_audit_sees_a_leader_through_another_curves_marker_and_a_name_nearer_another_curve() {
    let mut g = Glyphs::new();
    let svg = svg_of(&[s("mode", CLASSIFY), f("t1", 40.0), f("solute", 50.0)]);
    assert!(audit(&svg, &mut g, 1.0).is_empty(), "{:?}", audit(&svg, &mut g, 1.0));
    // a hand-made leader from the point A straight up through the markers above it
    let gm = geo(&svg);
    let p2 = gm.marks.iter().find(|m| m.0 == "ptA").unwrap().2;
    let (x, y) = ((p2.0 + p2.1) / 2.0, (p2.2 + p2.3) / 2.0);
    let leader = format!(r##"<path d="M{x:.2},{y:.2} L{x:.2},{:.2}" fill="none" stroke="#444" stroke-width="1" data-o="leader" data-f="ptA"/>"##, y - 60.0);
    let bad = svg.replace("</svg>", &format!("{leader}</svg>"));
    let out = audit(&bad, &mut g, 1.0);
    assert!(out.iter().any(|m| m.contains("runs through") || m.contains("ends beside") || m.contains("ends ")), "{out:?}");
    // the KCl name moved under the cluster, nearer the NaCl curve
    let svg = svg_of(&[]);
    let kcl = texts(&svg).into_iter().find(|t| t.text == "KCl").unwrap();
    let nacl_curve = geo(&svg).paths.iter().find(|p| p.0 == "curve" && p.1 == "s_nacl").unwrap().2.clone();
    let q = nacl_curve[nacl_curve.len() / 3];
    let moved = svg.replace(&format!(r#"x="{:.2}" y="{:.2}""#, kcl.x, kcl.y), &format!(r#"x="{:.2}" y="{:.2}""#, q.0 + 8.0, q.1 + 24.0));
    let out = audit(&moved, &mut g, 1.0);
    assert!(out.iter().any(|m| m.contains("no leader line") || m.contains("stroke") || m.contains("marker") || m.contains("leader")), "{out:?}");
}

#[test]
fn a_flat_curve_does_not_give_a_false_temperature() {
    // NaCl rises 3.3 g per 100 g over 100 degrees: the temperature of a given amount is not determined by the data
    for (m, dec) in [(35.66, 1), (35.66, 2), (36.0, 1), (36.0, 0), (36.5, 2), (37.0, 1), (38.0, 1), (35.7, 0)] {
        let t = joined(&key(&temp_list("NaCl", m, 100.0, dec)));
        assert!(t.contains("almost flat here, so the temperature cannot be read reliably"), "{m} dec {dec}: {t}");
        assert!(!t.contains("when T ="), "{m} dec {dec}: {t}");
    }
    // the given is inverted as typed, not as rounded to the places shown: 35.66 g is at 1 degree, not at 8
    // (the old key printed "8 °C" for it)
    let t = joined(&key(&temp_list("NaCl", 35.66, 100.0, 1)));
    assert!(!t.contains("8 °C") && !t.contains(" T = 8"), "{t}");
    // a steep curve still gives a degree, from the typed amount
    for (name, m, want) in [("KNO₃", 60.0, 38.0), ("KCl", 40.1, 40.0), ("KCl", 34.2, 20.0), ("KCl", 37.2, 30.0), ("NH₄Cl", 45.8, 40.0), ("NaNO₃", 87.6, 20.0)] {
        let t = joined(&key(&temp_list(name, m, 100.0, 1)));
        assert!(t.contains(&format!("when T = {want} °C")), "{name} {m}: {t}");
    }
    // typed 4-figure amounts are not rounded first: 61.57 g of KNO3 in 100 g of water, dec 0 (62 would be 39 degrees)
    let exact = temperature_at(0, 61.57).unwrap();
    let t = joined(&key(&temp_list("KNO₃", 61.57, 100.0, 0)));
    assert!(t.contains(&format!("when T = {} °C", exact.round())), "{exact}: {t}");
    // and the neighbours: just above the highest value of the curve, and a gas (small numbers, not flat)
    let t = joined(&key(&temp_list("NaCl", 40.0, 100.0, 1)));
    assert!(t.contains("more than the highest solubility"), "{t}");
    let t = joined(&key(&temp_list("CO₂", 0.1688, 100.0, 1)));
    assert!(t.contains("when T = 20 °C"), "{t}");
    // the student copy never carries the sentence
    let mut l = temp_list("NaCl", 35.66, 100.0, 1);
    l.push(b(SHOW_ANSWERS, false));
    assert!(!every_text(&l).contains("almost flat"));
}



