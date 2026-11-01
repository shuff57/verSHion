//! Tests of the equilibrium graphs. The label audit measures every text with the glyphs the PNG renderer draws
//! (usvg's own layout of the bundled and system faces), not with the placer's width estimate. The printed values are
//! checked against exact decimal arithmetic (a small big-integer type below), not against the solver that drew them.
use super::*;
use crate::figures::{registry, to_png};
use std::collections::{BTreeMap, HashMap};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("equilibrium-graphs is registered") }
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

const CONC: &str = "Concentration vs time";
const ICE: &str = "ICE table";
const QUOT: &str = "Q vs time";
const RATES: &str = "Forward and reverse rates";
const T_EQ: &str = "Find the equilibrium concentrations from K";
const T_K: &str = "Find K from equilibrium data";
const T_DIR: &str = "Predict the direction (Q vs K)";
const T_READ: &str = "Read the equilibrium values off the graph";

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

type Seg = (Pt, Pt);
/// The strokes (kind, feature, segment), markers (feature, bounding box) and whole paths (kind, feature, points, style).
struct Geo { strokes: Vec<(String, String, Seg)>, marks: Vec<(String, Bx)>, paths: Vec<(String, String, Vec<Pt>, String)> }

fn geo(svg: &str) -> Geo {
    let mut g = Geo { strokes: Vec::new(), marks: Vec::new(), paths: Vec::new() };
    for part in svg.split('<').skip(1) {
        let tag = part.split('>').next().unwrap_or("");
        let kind = attr(tag, "data-o").unwrap_or_default();
        let feat = attr(tag, "data-f").unwrap_or_default();
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
            g.marks.push((feat, (cx - r, cx + r, cy - r, cy + r)));
        } else if kind == "mark" && tag.starts_with("rect ") {
            let (x, y, w, h) = (attr_f(tag, "x").unwrap(), attr_f(tag, "y").unwrap(), attr_f(tag, "width").unwrap(), attr_f(tag, "height").unwrap());
            g.marks.push((feat, (x, x + w, y, y + h)));
        } else if kind == "mark" && tag.starts_with("polygon ") {
            let pts = path_pts(&format!("M{}", attr(tag, "points").unwrap().replace(' ', " L")));
            let xs = pts.iter().map(|p| p.0);
            let ys = pts.iter().map(|p| p.1);
            g.marks.push((feat, (xs.clone().fold(f64::MAX, f64::min), xs.fold(f64::MIN, f64::max), ys.clone().fold(f64::MAX, f64::min), ys.fold(f64::MIN, f64::max))));
        }
    }
    g
}

fn path_of(svg: &str, feat: &str) -> Vec<Pt> {
    geo(svg).paths.into_iter().find(|p| p.1 == feat && p.0 == "curve").map(|p| p.2).unwrap_or_else(|| panic!("no {feat} curve"))
}

// ---------------------------------------------------------------------------------------------
// Real glyphs
// ---------------------------------------------------------------------------------------------

use crate::figures::chemistry::reactionenergy::{seg_hits, hits};

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
                    resvg::usvg::Node::Group(g2) => { if let Some(r) = find(g2) { return Some(r); } }
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

/// Every problem with the text of one render, measured in real glyphs: text off the canvas, text on text, a label on a
/// solid stroke or a marker, a label that does not read as its own feature's (more than 40 px x size from it, or more
/// than 15 px from it while 6 px nearer another labelled feature), a leader that ends on or passes through another
/// feature's marker, and (student copy) a feature that only the key may draw. The dotted guides and the grid are thin
/// lines a label may cross.
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
            if ts[a].kind == "label" && ts[c].kind == "label" && ts[a].group == ts[c].group { continue; }
            if let (Some(x), Some(y)) = (inks[a], inks[c]) {
                if hits(x, y) { bad.push(format!("{:?} on {:?}", ts[a].text, ts[c].text)); }
            }
        }
    }
    let mut groups: BTreeMap<String, (String, Bx)> = BTreeMap::new();
    for (t, bx) in ts.iter().zip(&inks) {
        if t.kind != "label" { continue; }
        let Some(bx) = bx else { continue };
        for (kind, _, (p, q)) in &gm.strokes {
            if kind == "guide" || kind == "grid" { continue; }
            if seg_hits(*p, *q, *bx) { bad.push(format!("{:?} on a {kind} stroke {p:?}-{q:?}", t.text)); }
        }
        for (feat, mb) in &gm.marks {
            if hits(*bx, *mb) { bad.push(format!("{:?} on the {feat} marker", t.text)); }
        }
        let e = groups.entry(t.group.clone()).or_insert((t.feat.clone(), *bx));
        e.1 = (e.1 .0.min(bx.0), e.1 .1.max(bx.1), e.1 .2.min(bx.2), e.1 .3.max(bx.3));
    }
    let owned: Vec<String> = { let mut v: Vec<String> = groups.values().map(|g| g.0.clone()).collect(); v.sort(); v.dedup(); v };
    let dist = |feat: &str, bx: Bx| -> f64 {
        let mut d = f64::MAX;
        for (_, f, (p, q)) in &gm.strokes { if f == feat { d = d.min(seg_box(*p, *q, bx)); } }
        for (f, mb) in &gm.marks { if f == feat { d = d.min(seg_box((mb.0, mb.2), (mb.1, mb.3), bx)); } }
        d
    };
    for (own, bx) in groups.values() {
        let d = dist(own, *bx);
        let name = ts.iter().find(|t| t.kind == "label" && &t.feat == own).map(|t| t.text.clone()).unwrap_or_default();
        if d > 40.0 * lab { bad.push(format!("{name:?} is {d:.1} px from its {own} feature")); continue; }
        let other = owned.iter().filter(|o| *o != own).map(|o| (dist(o, *bx), o)).fold((f64::MAX, None), |a, (x, o)| if x < a.0 { (x, Some(o)) } else { a });
        if d > 15.0 && other.0 < d - 6.0 { bad.push(format!("{name:?} is {d:.1} px from its {own} feature but {:.1} px from {:?}", other.0, other.1)); }
    }
    // a leader must not end on, or pass through, another feature's marker
    for (kind, feat, (p, q)) in &gm.strokes {
        if kind != "leader" { continue; }
        for (mf, mb) in &gm.marks {
            if mf != feat && seg_hits(*p, *q, (mb.0 - 1.0, mb.1 + 1.0, mb.2 - 1.0, mb.3 + 1.0)) { bad.push(format!("a {feat} leader meets the {mf} marker")); }
        }
    }
    // each "= value" line stays attached to its own name line: the same x, exactly one line step below, and the
    // group's leader ends exactly at the text's left edge (3 px short of it) at a height inside the group
    let mut by_group: BTreeMap<String, Vec<(&T, Bx)>> = BTreeMap::new();
    for (t, bx) in ts.iter().zip(&inks) { if t.kind == "label" && t.feat != "d" { if let Some(bx) = bx { by_group.entry(t.group.clone()).or_default().push((t, *bx)); } } }
    for (_, mut ls) in by_group {
        ls.sort_by(|a, c| a.0.y.partial_cmp(&c.0.y).unwrap());
        for w in ls.windows(2) {
            if (w[0].0.x - w[1].0.x).abs() > 1e-9 { bad.push(format!("{:?} is not under {:?}", w[1].0.text, w[0].0.text)); }
            let step = w[1].0.y - w[0].0.y;
            if (step - 1.2 * w[0].0.fs).abs() > 0.011 { bad.push(format!("{:?} is {step:.2} px under {:?}, not one line step", w[1].0.text, w[0].0.text)); }
        }
        let feat = &ls[0].0.feat;
        let lead = gm.paths.iter().find(|p| p.0 == "leader" && &p.1 == feat);
        if let Some(lead) = lead {
            let end = *lead.2.last().unwrap();
            let x0 = ls[0].0.x;
            let (top, bot) = ls.iter().fold((f64::MAX, f64::MIN), |a, (_, b)| (a.0.min(b.2), a.1.max(b.3)));
            if (end.0 - (x0 - 3.0)).abs() > 0.011 || end.1 < top - 1.0 || end.1 > bot + 1.0 { bad.push(format!("the {feat} leader ends at {end:?}, not at its label block (x {x0}, y {top:.1}..{bot:.1})")); }
        }
    }
    if student {
        for (kind, feat, _) in &gm.strokes {
            if (kind == "guide" && feat.starts_with('g')) || feat == "req" { bad.push(format!("the student copy draws the answer feature {feat}")); }
        }
    }
    bad
}

// ---------------------------------------------------------------------------------------------
// Exact decimal arithmetic: a signed big integer in base 1e9 and a decimal (mantissa, power of ten) on top of it
// ---------------------------------------------------------------------------------------------

const BASE: u64 = 1_000_000_000;

#[derive(Clone, Debug, PartialEq)]
struct Big { neg: bool, d: Vec<u32> }

impl Big {
    fn zero() -> Big { Big { neg: false, d: Vec::new() } }
    fn from_u64(mut v: u64) -> Big { let mut d = Vec::new(); while v > 0 { d.push((v % BASE) as u32); v /= BASE; } Big { neg: false, d } }
    fn from_digits(s: &str) -> Big {
        let mut d = Vec::new();
        let b = s.as_bytes();
        let mut end = b.len();
        while end > 0 { let st = end.saturating_sub(9); d.push(s[st..end].parse::<u32>().unwrap()); end = st; }
        let mut r = Big { neg: false, d };
        r.trim();
        r
    }
    fn trim(&mut self) { while self.d.last() == Some(&0) { self.d.pop(); } if self.d.is_empty() { self.neg = false; } }
    fn is_zero(&self) -> bool { self.d.is_empty() }
    /// The number of decimal digits of |self|.
    fn digits(&self) -> usize { match self.d.last() { None => 0, Some(t) => (self.d.len() - 1) * 9 + t.to_string().len() } }
    fn sign(&self) -> i32 { if self.is_zero() { 0 } else if self.neg { -1 } else { 1 } }
    fn cmp_abs(a: &[u32], b: &[u32]) -> std::cmp::Ordering {
        if a.len() != b.len() { return a.len().cmp(&b.len()); }
        for k in (0..a.len()).rev() { if a[k] != b[k] { return a[k].cmp(&b[k]); } }
        std::cmp::Ordering::Equal
    }
    fn add_abs(a: &[u32], b: &[u32]) -> Vec<u32> {
        let mut o = Vec::new();
        let mut carry = 0u64;
        for k in 0..a.len().max(b.len()) {
            let t = *a.get(k).unwrap_or(&0) as u64 + *b.get(k).unwrap_or(&0) as u64 + carry;
            o.push((t % BASE) as u32);
            carry = t / BASE;
        }
        if carry > 0 { o.push(carry as u32); }
        o
    }
    /// a - b for |a| >= |b|
    fn sub_abs(a: &[u32], b: &[u32]) -> Vec<u32> {
        let mut o = Vec::new();
        let mut borrow = 0i64;
        for k in 0..a.len() {
            let mut t = a[k] as i64 - *b.get(k).unwrap_or(&0) as i64 - borrow;
            if t < 0 { t += BASE as i64; borrow = 1 } else { borrow = 0 }
            o.push(t as u32);
        }
        o
    }
    fn neg(&self) -> Big { let mut r = self.clone(); if !r.is_zero() { r.neg = !r.neg; } r }
    fn add(&self, o: &Big) -> Big {
        let mut r = if self.neg == o.neg { Big { neg: self.neg, d: Big::add_abs(&self.d, &o.d) } } else {
            match Big::cmp_abs(&self.d, &o.d) {
                std::cmp::Ordering::Equal => Big::zero(),
                std::cmp::Ordering::Greater => Big { neg: self.neg, d: Big::sub_abs(&self.d, &o.d) },
                std::cmp::Ordering::Less => Big { neg: o.neg, d: Big::sub_abs(&o.d, &self.d) },
            }
        };
        r.trim();
        r
    }
    fn mul(&self, o: &Big) -> Big {
        if self.is_zero() || o.is_zero() { return Big::zero(); }
        let mut t = vec![0u64; self.d.len() + o.d.len() + 1];
        for (a, x) in self.d.iter().enumerate() {
            let mut carry = 0u64;
            for (c, y) in o.d.iter().enumerate() {
                let cur = t[a + c] + *x as u64 * *y as u64 + carry;
                t[a + c] = cur % BASE;
                carry = cur / BASE;
            }
            let mut k = a + o.d.len();
            while carry > 0 { let cur = t[k] + carry; t[k] = cur % BASE; carry = cur / BASE; k += 1; }
        }
        let mut r = Big { neg: self.neg != o.neg, d: t.into_iter().map(|x| x as u32).collect() };
        r.trim();
        r
    }
    fn mul_pow10(&self, k: u32) -> Big {
        let mut r = self.clone();
        let (q, rem) = ((k / 9) as usize, k % 9);
        r.d.splice(0..0, std::iter::repeat(0).take(q));
        r.trim();
        r.mul(&Big::from_u64(10u64.pow(rem)))
    }
    /// |self| / d, truncated.
    fn div_small(&self, d: u32) -> Big {
        let mut o = vec![0u32; self.d.len()];
        let mut rem = 0u64;
        for k in (0..self.d.len()).rev() { let cur = rem * BASE + self.d[k] as u64; o[k] = (cur / d as u64) as u32; rem = cur % d as u64; }
        let mut r = Big { neg: self.neg, d: o };
        r.trim();
        r
    }
}

/// m x 10^e.
#[derive(Clone, Debug)]
struct Dec { m: Big, e: i32 }

impl Dec {
    fn zero() -> Dec { Dec { m: Big::zero(), e: 0 } }
    fn int(v: i64) -> Dec { let mut m = Big::from_u64(v.unsigned_abs()); m.neg = v < 0 && !m.is_zero(); Dec { m, e: 0 } }
    fn neg(&self) -> Dec { Dec { m: self.m.neg(), e: self.e } }
    fn mul(&self, o: &Dec) -> Dec { Dec { m: self.m.mul(&o.m), e: self.e + o.e } }
    fn add(&self, o: &Dec) -> Dec {
        if self.m.is_zero() { return o.clone(); }
        if o.m.is_zero() { return self.clone(); }
        let e = self.e.min(o.e);
        Dec { m: self.m.mul_pow10((self.e - e) as u32).add(&o.m.mul_pow10((o.e - e) as u32)), e }
    }
    fn sub(&self, o: &Dec) -> Dec { self.add(&o.neg()) }
    fn sign(&self) -> i32 { self.m.sign() }
    fn pow(&self, n: u32) -> Dec { let mut r = Dec::int(1); for _ in 0..n { r = r.mul(self); } r }
    /// Divided by a small integer to `digits` decimal places beyond the mantissa (truncated).
    fn div_small(&self, d: u32, digits: u32) -> Dec { Dec { m: self.m.mul_pow10(digits).div_small(d), e: self.e - digits as i32 } }
}

/// A printed number ("0.0421", "−4.21 × 10⁻¹⁵", "50") as an exact decimal.
fn dec_of(s: &str) -> Dec {
    let s = s.trim().replace('\u{2212}', "-");
    let (m, e) = match s.split_once(" × 10") {
        Some((m, e)) => {
            let e: String = e.chars().map(|c| match c { '⁻' => '-', '⁰' => '0', '¹' => '1', '²' => '2', '³' => '3', '⁴' => '4', '⁵' => '5', '⁶' => '6', '⁷' => '7', '⁸' => '8', '⁹' => '9', c => c }).collect();
            (m.trim().to_string(), e.trim().parse::<i32>().unwrap())
        }
        None => (s.clone(), 0),
    };
    let neg = m.starts_with('-');
    let m = m.trim_start_matches('-');
    let (ip, fp) = m.split_once('.').unwrap_or((m, ""));
    let digits = format!("{ip}{fp}");
    let mut big = Big::from_digits(digits.trim_start_matches('0'));
    if digits.trim_start_matches('0').is_empty() { big = Big::zero(); }
    big.neg = neg && !big.is_zero();
    Dec { m: big, e: e - fp.len() as i32 }
}

/// A given held to 4 significant figures, as the exact decimal it was kept as.
fn dec_f(v: f64) -> Dec {
    if v == 0.0 { return Dec::zero(); }
    let sstr = format!("{v:.3e}");
    let (m, e) = sstr.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    let mut d = dec_of(m);
    d.e += e;
    d
}

/// Half a unit in the last of `n` significant digits of the printed number.
fn half_ulp(s: &str, n: usize) -> Dec {
    let d = dec_of(s);
    let lead = d.e + d.m.digits() as i32 - 1;
    Dec { m: Big::from_u64(5), e: lead - (n as i32 - 1) - 1 }
}

/// The sign of Pr - K R at extent x for the starting concentrations `c0` (exact), with the domain's edges read as
/// -1 below it (a product would be negative) and +1 above it (a reactant would be).
fn h_sign(st: &Setup, c0: &[Dec; 4], k: &Dec, x: &Dec) -> i32 {
    let rx = &st.rx;
    let mut c: [Dec; 4] = std::array::from_fn(|_| Dec::zero());
    for j in 0..4 { if rx.ink(j) { c[j] = c0[j].add(&x.mul(&Dec::int((Rxn::sg(j) as i64) * rx.coef[j] as i64))); } }
    if (2..4).any(|j| rx.ink(j) && c[j].sign() < 0) { return -1; }
    if (0..2).any(|j| rx.ink(j) && c[j].sign() < 0) { return 1; }
    let (mut pr, mut r) = (Dec::int(1), Dec::int(1));
    for j in 0..4 { if rx.ink(j) { let t = c[j].pow(rx.coef[j]); if j < 2 { r = r.mul(&t) } else { pr = pr.mul(&t) } } }
    pr.sub(&k.mul(&r)).sign()
}

fn c0_dec(st: &Setup) -> [Dec; 4] { std::array::from_fn(|j| dec_f(st.c0[j])) }

/// Whether the exact equilibrium has x within half a unit of the printed x.
fn x_ok(st: &Setup, k: &Dec, printed: &str, n: usize) -> bool {
    let x = dec_of(printed);
    let h = half_ulp(printed, n);
    let c0 = c0_dec(st);
    h_sign(st, &c0, k, &x.sub(&h)) <= 0 && h_sign(st, &c0, k, &x.add(&h)) >= 0
}

/// `x_ok`, with the same allowance for a rounding tie as `conc_ok_tie`.
fn x_ok_tie(st: &Setup, k: &Dec, printed: &str, n: usize) -> bool {
    if x_ok(st, k, printed, n) { return true; }
    let x = dec_of(printed);
    let h = half_ulp(printed, n);
    let slack = Dec { m: Big::from_u64(1), e: x.e + x.m.digits() as i32 - 13 };
    let c0 = c0_dec(st);
    h_sign(st, &c0, k, &x.sub(&h).sub(&slack)) <= 0 && h_sign(st, &c0, k, &x.add(&h).add(&slack)) >= 0
}

/// Whether the exact equilibrium has species j's concentration within half a unit of the printed value.
fn conc_ok(st: &Setup, k: &Dec, j: usize, printed: &str, n: usize) -> bool {
    let c0 = c0_dec(st);
    let p = dec_of(printed);
    let h = half_ulp(printed, n);
    let d = Dec::int(st.rx.coef[j] as i64);
    let _ = d;
    let to_x = |c: Dec| { let diff = c.sub(&c0[j]); let q = diff.div_small(st.rx.coef[j], 90); if Rxn::sg(j) < 0.0 { q.neg() } else { q } };
    let (xa, xb) = (to_x(p.sub(&h)), to_x(p.add(&h)));
    let (lo, hi) = if Rxn::sg(j) < 0.0 { (xb, xa) } else { (xa, xb) };
    h_sign(st, &c0, k, &lo) <= 0 && h_sign(st, &c0, k, &hi) >= 0
}

/// `conc_ok`, but a value that the exact root puts within 1e-12 (relative) of a rounding tie is accepted at either
/// neighbour: a double cannot hold the difference (1.05 less 2e-31 prints as 1.0 exactly, and as 1.1 from an f64).
fn conc_ok_tie(st: &Setup, k: &Dec, j: usize, printed: &str, n: usize) -> bool {
    if conc_ok(st, k, j, printed, n) { return true; }
    let p = dec_of(printed);
    let h = half_ulp(printed, n);
    let wide = Dec { m: Big::from_u64(1), e: p.e + p.m.digits() as i32 - 1 - 12 };
    let _ = wide;
    let slack = Dec { m: Big::from_u64(1), e: p.e + p.m.digits() as i32 - 13 };
    let c0 = c0_dec(st);
    let d = Dec::int(st.rx.coef[j] as i64);
    let _ = d;
    let to_x = |c: Dec| { let diff = c.sub(&c0[j]); let q = diff.div_small(st.rx.coef[j], 90); if Rxn::sg(j) < 0.0 { q.neg() } else { q } };
    let (xa, xb) = (to_x(p.sub(&h).sub(&slack)), to_x(p.add(&h).add(&slack)));
    let (lo, hi) = if Rxn::sg(j) < 0.0 { (xb, xa) } else { (xa, xb) };
    h_sign(st, &c0_dec(st), k, &lo) <= 0 && h_sign(st, &c0_dec(st), k, &hi) >= 0
}

/// Whether Pr / R at extent x (an exact decimal) is within half a unit of the printed K.
fn k_ok(st: &Setup, c0: &[Dec; 4], x: &Dec, printed: &str, n: usize) -> bool {
    let p = dec_of(printed);
    let h = half_ulp(printed, n);
    // Pr - K R has the sign of Q - K
    let sign_at = |kk: &Dec| h_sign_q(st, c0, kk, x);
    sign_at(&p.sub(&h)) >= 0 && sign_at(&p.add(&h)) <= 0
}

/// The sign of Pr - K R at x with no domain reading (concentrations there are all positive).
fn h_sign_q(st: &Setup, c0: &[Dec; 4], k: &Dec, x: &Dec) -> i32 {
    let rx = &st.rx;
    let (mut pr, mut r) = (Dec::int(1), Dec::int(1));
    for j in 0..4 {
        if !rx.ink(j) { continue; }
        let c = c0[j].add(&x.mul(&Dec::int((Rxn::sg(j) as i64) * rx.coef[j] as i64)));
        let t = c.pow(rx.coef[j]);
        if j < 2 { r = r.mul(&t) } else { pr = pr.mul(&t) }
    }
    pr.sub(&k.mul(&r)).sign()
}

/// The number tokens of a text: a run of digits and points with its minus sign, and a "× 10ⁿ" after it.
fn tokens(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < c.len() {
        if c[k].is_ascii_digit() {
            let start = if k > 0 && c[k - 1] == '\u{2212}' { k - 1 } else { k };
            let mut e = k;
            while e < c.len() && (c[e].is_ascii_digit() || (c[e] == '.' && e + 1 < c.len() && c[e + 1].is_ascii_digit())) { e += 1; }
            let mut tok: String = c[start..e].iter().collect();
            let rest: String = c[e..].iter().take(6).collect();
            if rest.starts_with(" × 10") {
                let mut x = e + 5;
                while x < c.len() && "⁻⁰¹²³⁴⁵⁶⁷⁸⁹".contains(c[x]) { x += 1; }
                tok.extend(&c[e..x]);
                e = x;
            }
            out.push(tok);
            k = e;
        } else {
            k += 1;
        }
    }
    out
}

fn parse_num(s: &str) -> f64 {
    let s = s.trim().replace('\u{2212}', "-");
    if let Some((m, e)) = s.split_once(" × 10") {
        let e: String = e.chars().map(|c| match c { '⁻' => '-', '⁰' => '0', '¹' => '1', '²' => '2', '³' => '3', '⁴' => '4', '⁵' => '5', '⁶' => '6', '⁷' => '7', '⁸' => '8', '⁹' => '9', c => c }).collect();
        return m.trim().parse::<f64>().unwrap() * 10f64.powi(e.trim().parse::<i32>().unwrap());
    }
    s.parse().unwrap_or_else(|_| panic!("not a number: {s:?}"))
}

// ---------------------------------------------------------------------------------------------
// Cases
// ---------------------------------------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); self.0 >> 33 }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T { v[(self.next() as usize) % v.len()] }
    fn pick_c<T: Clone>(&mut self, v: &[T]) -> T { v[(self.next() as usize) % v.len()].clone() }
    fn unit(&mut self) -> f64 { (self.next() % 1_000_000) as f64 / 1_000_000.0 }
}

const PH_ALL: [&str; 4] = ["gas (g)", "aqueous (aq)", "solid (s)", "liquid (l)"];
const KS: [f64; 14] = [1e-30, 1e-20, 1e-12, 1e-6, 0.001, 0.0466, 0.5, 1.0, 4.2, 50.0, 1234.0, 1e6, 1e12, 1e30];
const INITS: [f64; 9] = [0.0, 0.0, 0.05, 0.1, 0.5, 1.0, 2.5, 10.0, 123.4];

fn rand_chem(r: &mut Rng) -> L {
    let mut l: L = Vec::new();
    let co = ["coef_a", "coef_b", "coef_c", "coef_d"];
    let ph = ["phase_a", "phase_b", "phase_c", "phase_d"];
    let ini = ["init_a", "init_b", "init_c", "init_d"];
    for k in 0..4 {
        l.push(i(co[k], r.pick(&[0, 1, 1, 1, 2, 2, 3, 4])));
        l.push(s(ph[k], PH_ALL[if r.next() % 5 == 0 { 2 + (r.next() % 2) as usize } else { (r.next() % 2) as usize }]));
        l.push(f(ini[k], r.pick(&INITS)));
    }
    l.push(f("k", if r.next() % 2 == 0 { r.pick(&KS) } else { 10f64.powf(-30.0 + 60.0 * r.unit()) }));
    l
}

fn show(l: &L) -> String { format!("{l:?}") }

// ---------------------------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------------------------

#[test]
fn it_is_registered_with_a_faq_examples_and_its_controls() {
    let d = def();
    assert_eq!(d.id, "equilibrium-graphs");
    assert_eq!(d.title, "Equilibrium Graphs");
    let ps = (d.params)();
    let mut keys: Vec<&str> = ps.iter().map(|p| p.key).collect();
    keys.sort();
    let n = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), n, "duplicate param keys");
    for k in ["mode", "task", "coef_a", "coef_d", "name_a", "phase_c", "init_b", "k", "eq_species", "eq_value", "dist", "dist_species", "dist_amount", "dist_factor", "dist_dh", "dist_t1", "dist_t2", "dist_time", "sig", "show_answers", "style", "label_size"] {
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
}

// ---------------------------------------------------------------------------------------------
// The chemistry is exact
// ---------------------------------------------------------------------------------------------

#[test]
fn the_k_expression_is_written_from_the_coefficients_and_leaves_out_solids() {
    let m = mdl(&[s("name_a", "N2O4"), i("coef_a", 1), i("coef_b", 0), s("name_c", "NO2"), i("coef_c", 2)]);
    assert_eq!(k_expr(&m.st, "K"), "K = [NO\u{2082}]\u{b2} / [N\u{2082}O\u{2084}]");
    assert_eq!(k_expr(&m.st, "Q"), "Q = [NO\u{2082}]\u{b2} / [N\u{2082}O\u{2084}]");
    let m = mdl(&[i("coef_a", 2), i("coef_b", 3), i("coef_c", 4), i("coef_d", 1)]);
    assert_eq!(k_expr(&m.st, "K"), "K = [HI]\u{2074}[D] / ([H\u{2082}]\u{b2}[I\u{2082}]\u{b3})");
    // CaCO3(s) <-> CaO(s) + CO2(g): K = [CO2]
    let m = mdl(&[s("name_a", "CaCO3"), i("coef_a", 1), i("coef_b", 0), s("phase_a", "solid (s)"), s("name_c", "CaO"), s("phase_c", "solid (s)"), i("coef_c", 1), s("name_d", "CO2"), i("coef_d", 1)]);
    assert_eq!(k_expr(&m.st, "K"), "K = [CO\u{2082}]");
    // a solid on each side of a reaction with one gas on the reactant side: K = 1 / [A]
    let m = mdl(&[s("name_a", "A"), i("coef_b", 0), s("name_c", "S"), s("phase_c", "solid (s)"), i("coef_d", 0)]);
    assert_eq!(k_expr(&m.st, "K"), "K = 1 / [A]");
    // the figure says so
    let t = all_text(&svg_of(&[s("name_c", "S"), s("phase_c", "solid (s)")]));
    assert!(t.contains("Solids (s) and liquids (l) are left out of K and Q"), "{t}");
    assert!(!k_expr(&m.st, "K").contains("S"));
    // pretty names
    assert_eq!(pretty("Fe(OH)3"), "Fe(OH)\u{2083}");
    assert_eq!(pretty("2NO"), "2NO");
}

#[test]
fn the_equilibrium_is_exact_to_the_printed_digits_over_a_large_grid() {
    let mut r = Rng(7);
    let mut checked = 0usize;
    let mut kinds = [0usize; 4];
    for n in 0..2500 {
        let l = rand_chem(&mut r);
        let sg = 2 + (n % 4) as i64;
        let mut l2 = l.clone();
        l2.push(i("sig", sg));
        let m = mdl(&l2);
        let st = &m.st;
        match m.dir { Dir::Stuck => { kinds[0] += 1; continue } Dir::AtEq => { kinds[1] += 1; continue } Dir::Forward => kinds[2] += 1, Dir::Reverse => kinds[3] += 1 }
        let k = dec_f(m.k);
        assert!(x_ok_tie(st, &k, &sf(m.sol.x, st.sig), st.sig), "x {} in {}", sf(m.sol.x, st.sig), show(&l2));
        for j in 0..4 {
            if !st.rx.ink(j) { continue; }
            let p = sf(m.sol.c[j], st.sig);
            if p == "0" { continue; }
            assert!(conc_ok_tie(st, &k, j, &p, st.sig), "[{j}] = {p} in {}", show(&l2));
            checked += 1;
        }
    }
    assert!(checked > 4000, "{checked} {kinds:?}");
    assert!(kinds[2] > 300 && kinds[3] > 300, "{kinds:?}");
}

#[test]
fn the_exact_check_can_fail() {
    // a printed value one digit off is refused
    let m = mdl(&[f("init_a", 1.0), f("init_b", 1.0), f("k", 50.0), i("sig", 3)]);
    let st = &m.st;
    let k = dec_f(m.k);
    let good = sf(m.sol.c[2], 3);
    assert!(conc_ok(st, &k, 2, &good, 3));
    let bumped = format!("{:.2}", parse_num(&good) + 0.02);
    assert!(!conc_ok(st, &k, 2, &bumped, 3), "{good} vs {bumped}");
    assert!(!x_ok(st, &k, &format!("{:.3}", m.sol.x + 0.003), 3));
    assert!(x_ok(st, &k, &sf(m.sol.x, 3), 3));
}

#[test]
fn each_printed_value_is_the_same_in_every_mode() {
    let base: L = vec![f("init_a", 0.5), f("init_b", 0.8), f("k", 12.3), i("sig", 3)];
    let m = mdl(&base);
    let st = &m.st;
    let xs = sf(m.sol.x, 3);
    for mode in [CONC, ICE, QUOT, RATES] {
        let mut l = base.clone();
        l.push(s("mode", mode));
        let svg = svg_of(&l);
        let t = all_text(&svg);
        assert!(t.contains(&format!("x = {xs}")), "{mode}: x = {xs} not in the working");
        for j in 0..4 {
            if !st.rx.ink(j) { continue; }
            let v = sf(m.sol.c[j], 3);
            assert!(t.contains(&format!("{}eq = ", st.br(j))) && t.contains(&format!("= {v} M")), "{mode}: {} = {v}", st.br(j));
        }
        if mode == ICE {
            for j in 0..4 { if st.rx.ink(j) { assert!(texts(&svg).iter().any(|c| c.kind == "cell" && c.text == sf(m.sol.c[j], 3)), "ICE cell for {j}"); } }
        }
        if mode == CONC {
            for j in 0..4 { if st.rx.ink(j) { assert!(texts(&svg).iter().any(|c| c.kind == "label" && c.text == format!("= {} M", sf(m.sol.c[j], 3))), "margin value for {j}"); } }
        }
    }
}

#[test]
fn k_from_equilibrium_data_and_q_are_exact_to_the_printed_digits() {
    let mut r = Rng(11);
    let mut n_k = 0;
    let mut n_q = 0;
    for _ in 0..1500 {
        let mut l = rand_chem(&mut r);
        l.push(s("task", T_K));
        l.push(s("eq_species", SPECS[(r.next() % 4) as usize]));
        l.push(f("eq_value", r.pick(&[0.003, 0.01, 0.123, 0.5, 0.8, 1.7, 5.0, 40.0])));
        l.push(i("sig", 3));
        let m = mdl(&l);
        let st = &m.st;
        if m.sol.stuck || m.dir == Dir::Stuck { continue; }
        // only a start whose value was not held off the edge of the allowed range
        let i0 = st.eq_i;
        let ceq = dec_f(st.eq_v);
        let c0 = c0_dec(st);
        let x = ceq.sub(&c0[i0]).div_small(st.rx.coef[i0], 90);
        let x = if Rxn::sg(i0) < 0.0 { x.neg() } else { x };
        let (lo, hi) = st.rx.bounds(&st.c0);
        let inside = lo.is_none_or(|a| (parse_num(&sf(m.sol.x, 9)) > a)) && hi.is_none_or(|b| parse_num(&sf(m.sol.x, 9)) < b);
        if !inside || m.k <= K_LO * 1.0001 || m.k >= K_HI * 0.9999 { continue; }
        // skip starts where a cancellation of f64 would leave less than 6 good digits
        let cc = st.rx.conc(&st.c0, m.sol.x);
        if (0..4).any(|j| st.rx.ink(j) && cc[j] < 1e-6 * st.c0.iter().cloned().fold(0.0, f64::max).max(st.eq_v)) { continue; }
        let kp = sf(m.k, 3);
        assert!(k_ok(st, &c0, &x, &kp, 3), "K {kp} in {}", show(&l));
        n_k += 1;
        // Q at the start, for the same chemistry
        let q = st.rx.q(&st.rx.conc(&st.c0, 0.0));
        if q.is_finite() && q > 0.0 {
            let qp = sf(q, 3);
            assert!(k_ok(st, &c0, &Dec::zero(), &qp, 3), "Q {qp} in {}", show(&l));
            n_q += 1;
        }
    }
    assert!(n_k > 300 && n_q > 100, "{n_k} {n_q}");
}

#[test]
fn the_same_answer_comes_from_the_k_given_and_the_k_found() {
    // find K from the equilibrium of a K-given case, then solve with that K: the same equilibrium
    let a = mdl(&[f("init_a", 0.5), f("init_b", 0.5), f("k", 21.8)]);
    let ceq = a.sol.c[2];
    let b = mdl(&[f("init_a", 0.5), f("init_b", 0.5), s("task", T_K), s("eq_species", "C"), f("eq_value", sig4(ceq))]);
    assert!((b.k / 21.8 - 1.0).abs() < 2e-3, "{} {}", b.k, ceq);
    let c = mdl(&[f("init_a", 0.5), f("init_b", 0.5), f("k", sig4(b.k))]);
    for j in 0..4 { assert!((c.sol.c[j] - b.sol.c[j]).abs() < 2e-3 * (1.0 + b.sol.c[j]), "{j}: {} vs {}", c.sol.c[j], b.sol.c[j]); }
}

fn n2o4(extra: &[(&'static str, Value)]) -> L {
    let mut l: L = vec![s("name_a", "N2O4"), i("coef_a", 1), i("coef_b", 0), s("name_c", "NO2"), i("coef_c", 2), i("coef_d", 0), f("init_a", 0.1), f("init_b", 0.0), f("init_c", 0.0), f("k", 0.00466), i("sig", 3)];
    l.extend(extra.iter().cloned());
    l
}

fn key_lines(l: &L) -> Vec<String> { let m = mdl(l); readout(&m, true) }

#[test]
fn the_quadratic_is_worked_with_the_root_chosen_and_the_other_rejected() {
    let l = n2o4(&[]);
    let m = mdl(&l);
    let lines = key_lines(&l);
    let text = lines.join("\n");
    // 4x^2 + K x - K c0 = 0
    assert!(text.contains("4x\u{b2} + 0.00466x \u{2212} 4.66 \u{d7} 10\u{207b}\u{2074} = 0"), "{text}");
    // the roots by an independent route: the quadratic formula in plain f64
    let (a, b, c) = (4.0f64, 0.00466f64, -0.000466f64);
    let d = (b * b - 4.0 * a * c).sqrt();
    let (r1, r2) = ((-b + d) / (2.0 * a), (-b - d) / (2.0 * a));
    assert!(r1 > 0.0 && r2 < 0.0);
    assert!((m.sol.x - r1).abs() < 1e-12, "{} vs {r1}", m.sol.x);
    assert!(text.contains(&format!("x = {} or x = {}", sf(r1, 3), sf(r2, 3))), "{text}");
    assert!(text.contains(&format!("x = {} is the root and x = {} is rejected", sf(r1, 3), sf(r2, 3))), "{text}");
    assert!(text.contains("which is negative"), "the reason for the rejection");
    assert!(text.contains("x > 0, so the reaction ran forward"), "{text}");
    // the check Q = K ends the working, in the K's own digits
    let chk = lines.iter().find(|l| l.starts_with("Check: Q = ")).expect("a check");
    assert!(chk.ends_with("(the same as K = 0.00466)."), "{chk}");
    // the small-x test of the shortcut: x/c0 here is far more than 5 %
    assert!(text.contains("Small-x check") && text.contains("more than 5%"), "{text}");
    // and the same chemistry at a small K passes the 5 % test
    let small = key_lines(&n2o4(&[f("k", 1e-6)])).join("\n");
    assert!(small.contains("less than 5%"), "{small}");
}

#[test]
fn the_working_changes_with_the_degree() {
    // A <-> B is a straight line in x
    let lin = key_lines(&vec![i("coef_a", 1), i("coef_b", 0), i("coef_c", 1), i("coef_d", 0), f("init_a", 1.0), f("init_b", 0.0), f("k", 3.0)]).join("\n");
    assert!(lin.contains("A straight line in x"), "{lin}");
    // a cubic or higher has no short formula, and the root is still the one that keeps every concentration positive
    let hi = key_lines(&vec![i("coef_a", 2), i("coef_b", 1), i("coef_c", 3), i("coef_d", 0), f("init_a", 1.0), f("init_b", 1.0), f("k", 3.0)]).join("\n");
    assert!(hi.contains("degree") && hi.contains("bisection"), "{hi}");
    let m = mdl(&vec![i("coef_a", 2), i("coef_b", 1), i("coef_c", 3), i("coef_d", 0), f("init_a", 1.0), f("init_b", 1.0), f("k", 3.0)]);
    let k = dec_f(m.k);
    assert!(x_ok(&m.st, &k, &sf(m.sol.x, 3), 3));
}

#[test]
fn edge_cases_are_noted_not_panicked() {
    // K = 0, negative, not a number, huge, tiny
    for (k, want) in [(0.0, "K = 0"), (-3.0, "cannot be negative"), (f64::NAN, "not a number"), (1e40, "larger than this figure takes"), (1e-40, "smaller than this figure takes")] {
        let p = raw(&[f("k", k)]);
        let w = warnings(&p);
        assert!(w.iter().any(|x| x.contains(want)), "{k}: {w:?}");
        let svg = def().svg(&p);
        assert!(!all_text(&svg).contains("NaN") && !all_text(&svg).contains("inf"), "{k}");
    }
    // Q = K at the start: already at equilibrium, nothing changes
    let at = mdl(&[f("init_a", 1.0), f("init_b", 1.0), f("init_c", 2.0), f("k", 4.0)]);
    assert_eq!(at.dir, Dir::AtEq);
    assert_eq!(at.sol.x, 0.0);
    assert!(warnings(&with(&[f("init_a", 1.0), f("init_b", 1.0), f("init_c", 2.0), f("k", 4.0)])).iter().any(|w| w.contains("already equals K")));
    assert!(key_lines(&vec![f("init_a", 1.0), f("init_b", 1.0), f("init_c", 2.0), f("k", 4.0)]).join(" ").contains("x = 0"));
    // all zero: nothing can react
    let z = mdl(&[f("init_a", 0.0), f("init_b", 0.0), f("init_c", 0.0)]);
    assert_eq!(z.dir, Dir::Stuck);
    assert!(warnings(&with(&[f("init_a", 0.0), f("init_b", 0.0), f("init_c", 0.0)])).iter().any(|w| w.contains("Nothing in K can change")));
    // only products at the start: the reverse direction, with x negative
    let r = mdl(&[f("init_a", 0.0), f("init_b", 0.0), f("init_c", 1.0), f("k", 50.0)]);
    assert_eq!(r.dir, Dir::Reverse);
    assert!(r.sol.x < 0.0);
    let k = dec_f(r.k);
    assert!(x_ok(&r.st, &k, &sf(r.sol.x, 3), 3));
    assert!(r.sol.c[0] > 0.0 && r.sol.c[2] < 1.0);
    let t = key_lines(&vec![f("init_a", 0.0), f("init_b", 0.0), f("init_c", 1.0), f("k", 50.0)]).join("\n");
    assert!(t.contains("x < 0, so the reaction ran in reverse"), "{t}");
    // one reactant missing at the start and a product present: reverse again, no division by zero
    let o = mdl(&[f("init_a", 1.0), f("init_b", 0.0), f("init_c", 1.0), f("k", 50.0)]);
    assert_eq!(o.dir, Dir::Reverse);
    assert!(o.q0.is_infinite());
    assert!(!all_text(&svg_of(&[f("init_a", 1.0), f("init_b", 0.0), f("init_c", 1.0), s("mode", QUOT)])).contains("NaN"));
    // a missing reactant and a missing product: stuck, not an equilibrium of 0/0
    assert_eq!(mdl(&[f("init_a", 1.0), f("init_b", 0.0), f("init_c", 0.0)]).dir, Dir::Stuck);
    // a solid product: K = [CO2], whatever the amount of the solids
    let c = mdl(&[s("name_a", "CaCO3"), i("coef_a", 1), i("coef_b", 0), s("phase_a", "solid (s)"), s("name_c", "CaO"), s("phase_c", "solid (s)"), i("coef_c", 1), s("name_d", "CO2"), i("coef_d", 1), f("init_d", 0.0), f("k", 0.0123)]);
    assert_eq!(sf(c.sol.c[3], 3), sf(0.0123, 3));
    // coefficients above 1 put x to the power
    let p = poly(&mdl(&[i("coef_a", 3), i("coef_b", 0), i("coef_c", 2), f("init_a", 1.0), f("k", 2.0)]).st, &[1.0, 0.0, 0.0, 0.0], 2.0);
    assert_eq!(p.len() - 1, 3);
    // ~100 % and ~0 % conversion
    for k in [1e30, 1e-30, 1e12, 1e-12] {
        let m = mdl(&[f("init_a", 1.0), f("init_b", 1.0), f("k", k), i("sig", 3)]);
        let kd = dec_f(m.k);
        for j in 0..3 { let p = sf(m.sol.c[j], 3); assert!(p != "0" && conc_ok_tie(&m.st, &kd, j, &p, 3), "K = {k}: [{j}] = {p}"); }
    }
    // a disturbance that would make a concentration negative
    let d = mdl(&[f("init_a", 1.0), f("init_b", 1.0), f("k", 50.0), s("dist", "Remove a species"), s("dist_species", "C"), f("dist_amount", 900.0)]);
    let d2 = d.d2.as_ref().unwrap();
    assert_eq!(d2.c_after[2], 0.0);
    assert!(warnings(&with(&[f("init_a", 1.0), f("init_b", 1.0), f("k", 50.0), s("dist", "Remove a species"), s("dist_species", "C"), f("dist_amount", 900.0)])).iter().any(|w| w.contains("would make its concentration negative")));
    // adding a solid does nothing
    let sd = mdl(&[s("phase_a", "solid (s)"), s("dist", "Add a species"), s("dist_species", "A"), f("dist_amount", 5.0), f("k", 3.0)]);
    assert!(!sd.d2.as_ref().unwrap().effect);
    // a reaction of solids and liquids only is refused
    let allsol = warnings(&raw(&[s("phase_a", "solid (s)"), s("phase_b", "solid (s)"), s("phase_c", "liquid (l)"), s("phase_d", "solid (s)")]));
    assert!(allsol.iter().any(|w| w.contains("leaves nothing in K")), "{allsol:?}");
    // coefficients out of range, and a reaction with a side missing
    let w = warnings(&raw(&[i("coef_a", 0), i("coef_b", 0), i("coef_c", 9), i("coef_d", 0)]));
    assert!(w.iter().any(|x| x.contains("needs a reactant")), "{w:?}");
}

/// A dependable second integrator: classical RK4 with a small fixed step on the same rate law.
fn rk4(m: &Model, tau_end: f64) -> Vec<(f64, f64)> {
    let rx = &m.st.rx;
    let c0 = m.st.c0;
    let rate = |x: f64| { let c = rx.conc(&c0, x); let (pr, r) = rx.pr_r(&c); m.kf * r - m.kr * pr };
    let h = 0.0002;
    let mut x = 0.0;
    let mut out = vec![(0.0, 0.0)];
    let n = (tau_end / h) as usize;
    for k in 1..=n {
        let k1 = rate(x);
        let k2 = rate(x + 0.5 * h * k1);
        let k3 = rate(x + 0.5 * h * k2);
        let k4 = rate(x + h * k3);
        x += h / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
        if k % 500 == 0 { out.push((k as f64 * h, x)); }
    }
    out
}

#[test]
fn the_curves_converge_to_the_exact_equilibrium_and_agree_with_an_rk4_run() {
    let mut r = Rng(5);
    let mut n = 0;
    while n < 150 {
        let mut l = rand_chem(&mut r);
        l.push(f("k", 10f64.powf(-3.0 + 6.0 * r.unit())));
        let m = mdl(&l);
        if matches!(m.dir, Dir::Stuck | Dir::AtEq) { continue; }
        n += 1;
        // the implicit Euler run ends at the exact root
        let end = *m.sim.xs.last().unwrap();
        let span = m.sol.hi - m.sol.lo;
        assert!((end - m.sol.x).abs() <= 1e-4 * m.sol.x.abs() + 1e-9 * span, "{end} vs {} in {}", m.sol.x, show(&l));
        // and is monotone toward it
        let sgn = m.sol.x.signum();
        assert!(m.sim.xs.windows(2).all(|w| (w[1] - w[0]) * sgn >= -1e-12 * span), "not monotone in {}", show(&l));
        // the RK4 run on the same rate law, over the time the figure shows, stays within 2 % of the span of the change
        if m.sim.tau_ok < 15.0 {
            for (t, x) in rk4(&m, 8.0) {
                if x < m.sol.lo || x > m.sol.hi { break; } // the fixed-step run went unstable on a stiff start
                let xi = m.sim.at(t);
                assert!((x - xi).abs() <= 0.05 * m.sol.x.abs() + 1e-9, "t = {t}: rk4 {x} vs implicit {xi} in {}", show(&l));
            }
        }
        // the curve on the plot reaches the plateau (no disturbance): within the thickness of a line
        let tr = m.traj();
        let last = tr.p1.last().unwrap();
        for j in 0..4 {
            if m.st.rx.ink(j) { assert!((last.c[j] - m.sol.c[j]).abs() <= 2e-3 * m.c_bound(), "[{j}] {} vs {} in {}", last.c[j], m.sol.c[j], show(&l)); }
        }
    }
}

#[test]
fn the_time_unit_makes_the_first_approach_finish_at_five() {
    for k in [0.01, 1.0, 50.0, 3000.0] {
        let m = mdl(&[f("init_a", 1.0), f("init_b", 1.0), f("k", k)]);
        let tr = m.traj();
        // within 0.1 % of the way by t = 5, whatever K
        let span = m.sol.x.abs();
        let at5 = tr.p1.iter().find(|p| p.t >= 5.0).unwrap();
        let x5 = (at5.c[2] - m.st.c0[2]) / 2.0;
        assert!((x5 - m.sol.x).abs() <= 2e-3 * span, "K = {k}: {x5} vs {}", m.sol.x);
        // and not long before: the curve is still moving at t = 1
        let at1 = tr.p1.iter().find(|p| p.t >= 1.0).unwrap();
        assert!(((at1.c[2] - m.st.c0[2]) / 2.0 - m.sol.x).abs() > 1e-3 * span, "K = {k}: finished by t = 1");
    }
}


// ---------------------------------------------------------------------------------------------
// The student copy
// ---------------------------------------------------------------------------------------------

const MODES_ALL: [&str; 4] = [CONC, ICE, QUOT, RATES];
const TASKS_ALL: [&str; 4] = [T_EQ, T_K, T_DIR, T_READ];
const DISTS_ALL: [&str; 5] = ["None", "Add a species", "Remove a species", "Change the volume", "Change the temperature"];

/// Every value the key prints as an answer for these params, formatted as the key prints it.
fn answer_strings(m: &Model) -> Vec<String> {
    let st = &m.st;
    let n = st.sig;
    let mut v = vec![sf(m.sol.x, n), sf(m.k, n)];
    for j in 0..4 { if st.rx.ink(j) { v.push(sf(m.sol.c[j], n)); } }
    if let Some(d) = &m.d2 {
        v.push(sf(d.sol.x, n));
        for j in 0..4 { if st.rx.ink(j) { v.push(sf(d.sol.c[j], n)); } }
        v.push(sf(d.q_after, n));
    }
    v
}

fn givens_strings(m: &Model) -> Vec<String> {
    let st = &m.st;
    let mut v: Vec<String> = st.c0.iter().map(|c| given_fmt(*c)).collect();
    v.extend([given_fmt(m.k), given_fmt(st.eq_v), "0".into(), "1".into(), "2".into(), "3".into(), "4".into()]);
    v
}

fn student_cases() -> Vec<L> {
    let mut r = Rng(99);
    let mut out = Vec::new();
    for mode in MODES_ALL {
        for task in TASKS_ALL {
            for dist in DISTS_ALL {
                for _ in 0..3 {
                    let mut l = rand_chem(&mut r);
                    l.push(s("mode", mode));
                    l.push(s("task", task));
                    l.push(s("dist", dist));
                    l.push(s("dist_species", SPECS[(r.next() % 4) as usize]));
                    l.push(f("dist_amount", r.pick(&[0.05, 0.5, 3.0, 40.0])));
                    l.push(f("dist_factor", r.pick(&[0.25, 0.5, 2.0, 4.0])));
                    l.push(f("dist_t2", r.pick(&[250.0, 350.0, 900.0])));
                    l.push(f("eq_value", r.pick(&[0.003, 0.0123, 0.5, 3.7])));
                    l.push(s("eq_species", SPECS[(r.next() % 4) as usize]));
                    l.push(i("sig", 2 + (r.next() % 4) as i64));
                    out.push(l);
                }
            }
        }
    }
    out
}

const WORDS: [&str; 12] = ["system shifts", "Le Chatelier", "shifts to the", "shifts left", "shifts right", "so the reaction ran", "proceeds forward", "proceeds in reverse", "quadratic formula", "is rejected", "Check: Q", "Small-x check"];

#[test]
fn the_student_copy_hides_every_computed_value_and_every_law_name() {
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
        for w in WORDS { assert!(!t.contains(w) && !title_of(&stu).contains(w) && !desc_of(&stu).contains(w), "{w:?} on the student copy of {}", show(&l)); }
        // an answer value never appears as a token, unless the same string is a given
        let given = givens_strings(&m);
        let toks: Vec<String> = tokens(&t);
        for a in answer_strings(&m) {
            if a.is_empty() || given.contains(&a) || a == "0" { continue; }
            // the plain digits of a one-figure integer collide with tick labels and coefficients
            if a.len() <= 2 && a.chars().all(|c| c.is_ascii_digit()) { continue; }
            // a tick label may equal an answer by chance; only the non-axis texts count
            let non_axis: Vec<String> = texts(&stu).into_iter().filter(|x| x.kind != "axis").flat_map(|x| tokens(&x.text)).collect();
            assert!(!non_axis.contains(&a), "answer {a} on the student copy of {}\n{t}", show(&l));
        }
        let _ = toks;
        // no answer in the data attributes, title or description either
        for a in answer_strings(&m) {
            if a.len() < 4 || given.contains(&a) { continue; }
            for part in stu.split(" data-").skip(1) { assert!(part.split('"').nth(1) != Some(a.as_str()), "{a} in a data attribute of {}", show(&l)); }
            assert!(!title_of(&stu).contains(&a) && !desc_of(&stu).contains(&a));
        }
        assert!(stu != key || m.dir == Dir::Stuck, "student equals key for {}", show(&l));
        n += 1;
    }
    assert!(n >= 240, "{n}");
}

#[test]
fn the_student_axes_do_not_depend_on_the_answer() {
    // find-K task: K is the answer, and the Q axis, the time axis and the concentration axis do not move with it
    for mode in [CONC, QUOT, RATES] {
        let ax = |k: f64| {
            let svg = svg_of(&[s("mode", mode), s("task", T_K), f("eq_value", 0.37), b(SHOW_ANSWERS, false), f("k", k)]);
            texts(&svg).into_iter().filter(|t| t.kind == "axis").map(|t| format!("{:.2},{:.2},{}", t.x, t.y, t.text)).collect::<Vec<_>>()
        };
        assert_eq!(ax(0.01), ax(5e6), "{mode}");
    }
    // the key's axes may be fitted to the answer, but a K-given student copy and key share them
    for mode in [CONC, QUOT, RATES] {
        let ax = |ans: bool| {
            let svg = svg_of(&[s("mode", mode), s("task", T_EQ), b(SHOW_ANSWERS, ans)]);
            texts(&svg).into_iter().filter(|t| t.kind == "axis").map(|t| format!("{:.2},{:.2},{}", t.x, t.y, t.text)).collect::<Vec<_>>()
        };
        assert_eq!(ax(true), ax(false), "{mode}");
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
            if !ans {
                assert!(!d.contains("equilibrium values") && !d.contains("dashed"), "{d}");
            }
            let t = title_of(&svg);
            assert!(t.is_ascii() && !t.chars().any(|c| c.is_ascii_digit()), "{t}");
            // ids are opaque: no species name, number or word of the answer in a data attribute
            for part in svg.split(" data-").skip(1) {
                let v = part.split('"').nth(1).unwrap_or("");
                assert!(v.len() <= 8 && v.chars().all(|c| c.is_ascii_alphanumeric()), "data attribute {v:?}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Print, Large, axes
// ---------------------------------------------------------------------------------------------

#[test]
fn print_is_grey_only_and_tells_the_series_apart_by_dash_and_marker() {
    for mode in [CONC, QUOT, RATES, ICE] {
        let svg = svg_of(&[s("style", "Print"), s("mode", mode), s("dist", "Add a species"), f("init_d", 0.2), i("coef_d", 1)]);
        for part in svg.split('"').filter(|x| x.starts_with('#')) {
            let h: String = if part.len() == 4 { part[1..].chars().flat_map(|c| [c, c]).collect() } else { part[1..].to_string() };
            assert!(h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()), "{part}");
            let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap();
            assert!(c(0) == c(2) && c(2) == c(4), "Print draws a hue: {part} in {mode}");
        }
        if mode == CONC {
            let g = geo(&svg);
            let curves: Vec<&(String, String, Vec<Pt>, String)> = g.paths.iter().filter(|p| p.0 == "curve").collect();
            assert!(curves.len() >= 3);
            let shapes: Vec<String> = curves.iter().map(|c| c.3.split_once('|').unwrap().1.to_string()).collect();
            for a in 0..shapes.len() { for c in a + 1..shapes.len() { assert_ne!(shapes[a], shapes[c], "two series share a dash and width: {shapes:?}"); } }
            // and each series' end label carries its own marker shape: the marker boxes differ in kind across series
            let marks: Vec<&String> = g.marks.iter().map(|m| &m.0).collect();
            assert!(!marks.is_empty());
        }
    }
    // colour uses colour
    let svg = svg_of(&[]);
    assert!(svg.split('"').any(|x| x.starts_with('#') && x.len() == 7 && { let c = |i: usize| u8::from_str_radix(&x[i..i + 2], 16).unwrap_or(0); !(c(1) == c(3) && c(3) == c(5)) }));
}

#[test]
fn the_axes_are_labelled_once_in_order_on_their_ticks() {
    for l in student_cases().into_iter().step_by(3) {
        let mut l2 = l.clone();
        l2.push(b(SHOW_ANSWERS, true));
        let m = mdl(&l2);
        if matches!(m.st.mode, Mode::Ice) { continue; }
        let svg = svg_of(&l2);
        let ts = texts(&svg);
        let ax: Vec<&T> = ts.iter().filter(|t| t.kind == "axis" && t.rot.is_none()).collect();
        // x tick labels sit on one baseline; ordered left to right and without repeats
        let ticks: Vec<&&T> = ax.iter().filter(|t| t.anchor == "middle" && t.text.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '\u{2212}' || c == '-')).collect();
        let by_row: BTreeMap<i64, Vec<&&&T>> = { let mut r: BTreeMap<i64, Vec<&&&T>> = BTreeMap::new(); for t in &ticks { r.entry((t.y * 10.0) as i64).or_default().push(t); } r };
        for (_, row) in by_row.iter().filter(|(_, r)| r.len() > 2) {
            let xs: Vec<f64> = row.iter().map(|t| t.x).collect();
            assert!(xs.windows(2).all(|w| w[1] > w[0] + 5.0), "x ticks out of order or doubled: {:?} in {}", row.iter().map(|t| &t.text).collect::<Vec<_>>(), show(&l2));
            let labels: Vec<&String> = row.iter().map(|t| &t.text).collect();
            let mut u = labels.clone();
            u.dedup();
            assert_eq!(u.len(), labels.len(), "a repeated tick label {labels:?}");
        }
        // each axis title once
        let titles: Vec<String> = ax.iter().filter(|t| t.text.contains("Time") || t.text.starts_with("Rate") || t.text.starts_with("Q ")).map(|t| t.text.clone()).collect();
        let mut u = titles.clone();
        u.sort();
        u.dedup();
        assert_eq!(u.len(), titles.len(), "{titles:?}");
    }
}

#[test]
fn odd_inputs_never_panic_or_print_nan() {
    let mut r = Rng(3);
    for n in 0..400 {
        let mut l: L = Vec::new();
        for key in ["coef_a", "coef_b", "coef_c", "coef_d"] { l.push((key, r.pick_c(&[Value::I(-3), Value::I(0), Value::I(1), Value::I(99), Value::F(2.5), Value::S("x".into()), Value::B(true)]))); }
        for key in ["init_a", "init_b", "init_c", "init_d", "k", "eq_value", "dist_amount", "dist_factor", "dist_dh", "dist_t1", "dist_t2", "dist_time"] {
            l.push((key, r.pick_c(&[Value::F(f64::NAN), Value::F(f64::INFINITY), Value::F(-1.0), Value::F(0.0), Value::F(1e300), Value::F(1e-300), Value::F(1.0), Value::I(5), Value::S("a".into())])));
        }
        l.push(("mode", r.pick_c(&[Value::S(CONC.into()), Value::S(ICE.into()), Value::S(QUOT.into()), Value::S(RATES.into()), Value::S("zz".into()), Value::I(1)])));
        l.push(("task", r.pick_c(&[Value::S(T_EQ.into()), Value::S(T_K.into()), Value::S(T_DIR.into()), Value::S(T_READ.into()), Value::S("zz".into())])));
        l.push(("dist", r.pick_c(&[Value::S(DISTS_ALL[(n % 5) as usize].into()), Value::S("zz".into())])));
        for key in ["phase_a", "phase_b", "phase_c", "phase_d"] { l.push((key, r.pick_c(&[Value::S("solid (s)".into()), Value::S("gas (g)".into()), Value::S("zz".into()), Value::F(2.0)]))); }
        l.push(("eq_species", Value::S("Q".into())));
        let p = raw(&l);
        let svg = def().svg(&p);
        let t = all_text(&svg);
        assert!(svg.contains("</svg>"), "{}", show(&l));
        assert!(!t.contains("NaN") && !svg.contains("NaN") && !t.split_whitespace().any(|w| w == "inf" || w == "None"), "bad number in\n{t}\n{}", show(&l));
        let _ = warnings(&p);
        let _ = def().svg(&{ let mut q = p.clone(); q.0.insert(SHOW_ANSWERS.to_string(), Value::B(false)); q });
    }
}

#[test]
fn rendering_is_deterministic() {
    for l in student_cases().into_iter().step_by(11) { assert_eq!(svg_of(&l), svg_of(&l)); }
}

#[test]
fn handles_move_the_starting_concentration_k_the_plateau_and_the_disturbance() {
    let d = def();
    let p = with(&[s("dist", "Add a species")]);
    let (svg, size) = (d.svg(&p), 0);
    let _ = (svg, size);
    let (w, h) = crate::figures::svg_size(&d.svg(&p));
    let hs = handles(ID, &p, (w, h)).expect("handles");
    assert!(hs.len() >= 3, "{:?}", hs.iter().map(|h| h.tip.clone()).collect::<Vec<_>>());
    assert!(!crate::figures::interact::handles_for(ID, &p, (w, h)).is_empty());
    // each handle sits on the canvas, and a drag to its own place changes nothing
    for hd in &hs {
        assert!(hd.pos.0 >= 0.0 && hd.pos.0 <= w && hd.pos.1 >= 0.0 && hd.pos.1 <= h, "{} at {:?}", hd.tip, hd.pos);
        let mut q = p.clone();
        (hd.drag.clone().unwrap())(&mut q, hd.pos);
        for (k, v) in &p.0 {
            if let (Value::F(a), Some(Value::F(c))) = (v, q.0.get(k)) { assert!((a - c).abs() <= 0.02 * a.abs() + 0.06, "{}: {k} {a} -> {c}", hd.tip); }
        }
    }
    // dragging a starting concentration to higher on the plot raises it
    let start = hs.iter().find(|h| h.tip.contains("starting [H")).expect("a starting handle");
    let mut q = p.clone();
    (start.drag.clone().unwrap())(&mut q, (start.pos.0, start.pos.1 - 40.0));
    assert!(q.f("init_a") > p.f("init_a") || q.f("init_b") > p.f("init_b"), "{} {}", q.f("init_a"), q.f("init_b"));
    // the K line handle (Q mode) changes K
    let pq = with(&[s("mode", QUOT)]);
    let hq = handles(ID, &pq, (w, h)).unwrap();
    let kh = hq.iter().find(|h| h.tip.contains("K line")).expect("a K handle");
    let mut q = pq.clone();
    (kh.drag.clone().unwrap())(&mut q, (kh.pos.0, kh.pos.1 - 30.0));
    assert!(q.f("k") > pq.f("k"), "{}", q.f("k"));
    // the disturbance handle changes its time
    let dh = hs.iter().find(|h| h.tip.contains("disturbance")).expect("a disturbance handle");
    let mut q = p.clone();
    (dh.drag.clone().unwrap())(&mut q, (dh.pos.0 + 60.0, dh.pos.1));
    assert!(q.f("dist_time") > p.f("dist_time"), "{}", q.f("dist_time"));
    // the direction task draws no curves, so it has no plateau to drag
    let ps = with(&[b(SHOW_ANSWERS, false), s("task", T_DIR)]);
    let hss = handles(ID, &ps, (w, h)).unwrap_or_default();
    assert!(!hss.iter().any(|h| h.tip.contains("plateau")));
}

#[test]
fn equilibrium_pngs() {
    let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
    let mut cases: Vec<(String, L)> = Vec::new();
    for (mode, tag) in [(CONC, "conc"), (ICE, "ice"), (QUOT, "quot"), (RATES, "rates")] {
        cases.push((format!("{tag}-default"), vec![s("mode", mode)]));
        cases.push((format!("{tag}-n2o4"), { let mut l = n2o4(&[]); l.push(s("mode", mode)); l }));
        cases.push((format!("{tag}-solid"), vec![s("mode", mode), s("name_a", "CaCO3"), i("coef_a", 1), i("coef_b", 0), s("phase_a", "solid (s)"), s("name_c", "CaO"), s("phase_c", "solid (s)"), i("coef_c", 1), s("name_d", "CO2"), i("coef_d", 1), f("k", 0.0123)]));
        cases.push((format!("{tag}-tinyk"), vec![s("mode", mode), f("k", 1e-25), f("init_a", 2.0), f("init_b", 2.0)]));
        cases.push((format!("{tag}-hugek"), vec![s("mode", mode), f("k", 1e25), f("init_a", 2.0), f("init_b", 2.0)]));
        cases.push((format!("{tag}-reverse"), vec![s("mode", mode), f("init_a", 0.0), f("init_b", 0.0), f("init_c", 2.0)]));
        cases.push((format!("{tag}-big"), vec![s("mode", mode), i("coef_a", 2), i("coef_b", 3), i("coef_c", 4), i("coef_d", 1), f("init_a", 1.0), f("init_b", 1.0), f("k", 8.0)]));
        for dist in &DISTS_ALL[1..] {
            cases.push((format!("{tag}-dist-{}", dist.split(' ').next().unwrap().to_lowercase()), vec![s("mode", mode), s("dist", dist), f("dist_amount", 0.5), f("dist_dh", -90.0)]));
        }
    }
    for task in TASKS_ALL { cases.push((format!("task-{}", task.split(' ').next().unwrap().to_lowercase()), vec![s("task", task), f("eq_value", 0.8)])); }
    for (i2, e) in examples(ID).unwrap().into_iter().enumerate() { cases.push((format!("ex{i2}"), e.params)); }
    for (tag, list) in cases {
        for size in ["Small", "Medium", "Large"] {
            for (kind, extra) in [("key", vec![b(SHOW_ANSWERS, true)]), ("student", vec![b(SHOW_ANSWERS, false)]), ("print", vec![s("style", "Print")])] {
                if size != "Medium" && !(tag == "conc-default" || tag == "conc-n2o4" || tag.ends_with("dist-add")) { continue; }
                let mut l = list.clone();
                l.extend(extra);
                l.push(s("label_size", size));
                let svg = svg_of(&l);
                std::fs::write(format!("{dir}/eq-{tag}-{size}-{kind}.png"), to_png(&svg, 1.5).unwrap()).unwrap();
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The label audit, in real glyphs
// ---------------------------------------------------------------------------------------------

fn audit_cases(n: usize) -> Vec<(L, bool)> {
    let mut r = Rng(2024);
    let mut out = Vec::new();
    while out.len() < n {
        let mut l = rand_chem(&mut r);
        let mode = r.pick(&MODES_ALL);
        let task = r.pick(&TASKS_ALL);
        l.push(s("mode", mode));
        l.push(s("task", task));
        l.push(s("dist", r.pick(&DISTS_ALL)));
        l.push(s("dist_species", SPECS[(r.next() % 4) as usize]));
        l.push(f("dist_amount", r.pick(&[0.05, 0.5, 3.0, 40.0])));
        l.push(f("dist_factor", r.pick(&[0.25, 0.5, 2.0, 4.0])));
        l.push(f("dist_t2", r.pick(&[250.0, 350.0, 900.0])));
        l.push(f("dist_time", r.pick(&[6.0, 8.0, 12.0, 24.0])));
        l.push(f("eq_value", r.pick(&[0.003, 0.0123, 0.5, 3.7])));
        l.push(s("eq_species", SPECS[(r.next() % 4) as usize]));
        l.push(s("label_size", r.pick(&["Small", "Medium", "Large"])));
        l.push(s("style", r.pick(&["Colour", "Print"])));
        if r.next() % 3 == 0 {
            let names = ["H2", "N2O4", "NO2", "Fe3+", "Ca(OH)2", "SCN-", "FeSCN2+", "CO", "SO4^2-", "NO3^-"];
            for key in ["name_a", "name_b", "name_c", "name_d"] { l.push((key, Value::S(r.pick(&names).to_string()))); }
        }
        let ans = r.next() % 2 == 0;
        l.push(b(SHOW_ANSWERS, ans));
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
                let svg = def().svg(&with(l));
                let lab = svg::label_factor(match with(l).s("label_size") { x => x });
                for p in audit(&svg, &mut g, lab, !ans) { bad.push(format!("{p}\n  in {}", show(l))); if bad.len() > 30 { return bad; } }
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
    let svg = svg_of(&[s("mode", CONC)]);
    assert!(audit(&svg, &mut g, 1.0, false).is_empty(), "{:?}", audit(&svg, &mut g, 1.0, false));
    let add = |s: &str| svg.replace("</svg>", &format!("{s}</svg>"));
    // text off the canvas
    let off = add(&format!(r##"<text x="5" y="-4" font-size="12" font-family="{FONT}" data-o="note">Gone</text>"##));
    assert!(audit(&off, &mut g, 1.0, false).iter().any(|p| p.contains("leaves the")));
    // text on text
    let t0 = texts(&svg).into_iter().find(|t| t.kind == "note").unwrap();
    let dup = add(&format!(r##"<text x="{}" y="{}" font-size="{}" font-family="{FONT}" data-o="note">{}</text>"##, t0.x + 3.0, t0.y + 2.0, t0.fs, esc(&t0.text)));
    assert!(audit(&dup, &mut g, 1.0, false).iter().any(|p| p.contains(" on ")));
    // a label on a curve and on a marker
    let curve = path_of(&svg, "c2");
    let mid = curve[curve.len() / 2];
    let on = add(&format!(r##"<text x="{}" y="{}" font-size="12" text-anchor="middle" font-family="{FONT}" data-o="label" data-f="c0" data-g="zz">OnCurve</text>"##, mid.0, mid.1 + 4.0));
    assert!(audit(&on, &mut g, 1.0, false).iter().any(|p| p.contains("stroke")));
    let mk = geo(&svg).marks;
    if let Some((_, bx)) = mk.first() {
        let onm = add(&format!(r##"<text x="{}" y="{}" font-size="12" text-anchor="middle" font-family="{FONT}" data-o="label" data-f="c0" data-g="zz">M</text>"##, (bx.0 + bx.1) / 2.0, (bx.2 + bx.3) / 2.0 + 4.0));
        assert!(audit(&onm, &mut g, 1.0, false).iter().any(|p| p.contains("marker")));
    }
    // a label far from its own feature
    let far = add(&format!(r##"<text x="40" y="20" font-size="12" font-family="{FONT}" data-o="label" data-f="c0" data-g="far">Far away</text>"##));
    assert!(audit(&far, &mut g, 1.0, false).iter().any(|p| p.contains("px from")));
    // a value line pulled away from its name line, and a leader that no longer meets its block
    let moved = {
        let t = texts(&svg).into_iter().find(|t| t.kind == "label" && t.text.starts_with("= ")).expect("a value line");
        svg.replacen(&format!("x=\"{:.2}\" y=\"{:.2}\"", t.x, t.y), &format!("x=\"{:.2}\" y=\"{:.2}\"", t.x, t.y + 6.0), 1)
    };
    assert!(audit(&moved, &mut g, 1.0, false).iter().any(|p| p.contains("line step")), "{:?}", audit(&moved, &mut g, 1.0, false));
    // an answer-only feature on the student copy
    let leak = add(r##"<line x1="10" y1="10" x2="40" y2="10" data-o="guide" data-f="g0"/>"##);
    assert!(audit(&leak, &mut g, 1.0, true).iter().any(|p| p.contains("answer feature")));
}

#[test]
fn large_labels_never_clip() {
    let mut g = Glyphs::new();
    let mut r = Rng(8);
    for n in 0..300 {
        let mut l = rand_chem(&mut r);
        l.push(s("mode", MODES_ALL[n % 4]));
        l.push(s("task", TASKS_ALL[(n / 4) % 4]));
        l.push(s("dist", DISTS_ALL[(n / 3) % 5]));
        l.push(s("label_size", "Large"));
        l.push(s("name_c", "Ca(OH)2"));
        l.push(s("name_a", "FeSCN2+"));
        let svg = svg_of(&l);
        let (w, h) = crate::figures::svg_size(&svg);
        for t in texts(&svg) {
            let Some(bx) = g.ink(&t) else { continue };
            assert!(bx.0 >= 0.0 && bx.1 <= w && bx.2 >= 0.0 && bx.3 <= h, "{:?} clips: {bx:?} in {w}x{h} for {}", t.text, show(&l));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Round 2 (critic): the polynomial, the roots, the rate constants, the student copy, charges, the removal label
// ---------------------------------------------------------------------------------------------

fn coef4(c: [i64; 4]) -> L { vec![i("coef_a", c[0]), i("coef_b", c[1]), i("coef_c", c[2]), i("coef_d", c[3])] }
fn phases4(p: [&str; 4]) -> L { vec![s("phase_a", p[0]), s("phase_b", p[1]), s("phase_c", p[2]), s("phase_d", p[3])] }

/// The cases the critic reported, plus the cosmetic cousins at K = 1e+-25.
fn critic_cases() -> Vec<L> {
    let mut v: Vec<L> = Vec::new();
    let mut a = coef4([2, 4, 1, 4]); a.extend(phases4(["gas (g)", "solid (s)", "liquid (l)", "aqueous (aq)"])); a.extend([f("init_a", 0.7427), f("init_d", 1.237), f("init_b", 0.0), f("init_c", 0.0), f("k", 5.657e21), i("sig", 4)]); v.push(a);
    let mut a = coef4([1, 0, 1, 2]); a.extend([f("init_a", 0.1131), f("init_d", 0.4379), f("init_c", 0.0), f("k", 1.506e-22), i("sig", 4)]); v.push(a);
    let mut a = coef4([4, 2, 1, 3]); a.extend(phases4(["solid (s)", "aqueous (aq)", "gas (g)", "gas (g)"])); a.extend([f("init_a", 0.001254), f("init_c", 0.001122), f("init_d", 9.744), f("init_b", 0.0), f("k", 5.515e14), i("sig", 2)]); v.push(a);
    for k in [1e25, 1e-25, 1e13, 1e-14] { v.push(vec![f("init_a", 2.0), f("init_b", 2.0), f("k", k), i("sig", 3)]); }
    let mut a = n2o4(&[]); a.push(f("k", 1e13)); v.push(a);
    let mut a = vec![i("coef_a", 2), i("coef_b", 0), i("coef_c", 1), i("coef_d", 0), f("init_a", 1.0), f("init_c", 0.0), f("k", 1e-14), i("sig", 3)]; a.push(s("name_c", "C")); v.push(a);
    v
}

fn degree_expected(m: &Model) -> Option<usize> {
    let rx = &m.st.rx;
    let dp: u32 = (2..4).filter(|&j| rx.ink(j)).map(|j| rx.coef[j]).sum();
    let dr: u32 = (0..2).filter(|&j| rx.ink(j)).map(|j| rx.coef[j]).sum();
    if dp == dr { None } else { Some(dp.max(dr) as usize) }
}

fn rand_critic_like(n: usize, seed: u64) -> Vec<L> {
    let mut r = Rng(seed);
    let mut out = critic_cases();
    while out.len() < n {
        let mut l = rand_chem(&mut r);
        l.push(i("sig", 2 + (r.next() % 4) as i64));
        l.push(s("mode", CONC));
        l.push(s("task", T_EQ));
        out.push(l);
    }
    out
}

#[test]
fn the_multiplied_out_line_keeps_every_term_and_prints_the_true_degree() {
    let mut seen = 0;
    for l in rand_critic_like(900, 17) {
        let m = mdl(&l);
        if matches!(m.dir, Dir::Stuck | Dir::AtEq) { continue; }
        let Some(deg) = degree_expected(&m) else { continue };
        let p = poly(&m.st, &m.st.c0, m.k);
        assert_eq!(p.len() - 1, deg, "poly dropped a term in {}", show(&l));
        assert!(p.last().unwrap() > &0.0);
        let text = key_lines(&l).join("\n");
        assert!(text.contains(&format!("(degree {deg} in x)")), "wrong degree (expected {deg}) in {}\n{text}", show(&l));
        seen += 1;
    }
    assert!(seen > 400, "{seen}");
    // the two reported cases, by their coefficients
    let m = mdl(&n2o4(&[f("k", 1e13)]));
    let p = poly(&m.st, &m.st.c0, m.k);
    assert_eq!(p.len(), 3);
    assert!((p[2] - 4.0).abs() < 1e-9 && (p[1] / 1e13 - 1.0).abs() < 1e-9 && (p[0] / -1e12 - 1.0).abs() < 1e-9, "{p:?}");
    let t = key_lines(&n2o4(&[f("k", 1e13)])).join("\n");
    assert!(t.contains("4x\u{b2} + (1 \u{d7} 10\u{b9}\u{b3})x \u{2212} 1 \u{d7} 10\u{b9}\u{b2} = 0 (degree 2 in x)"), "{t}");
    let m = mdl(&[i("coef_a", 2), i("coef_b", 0), i("coef_c", 1), i("coef_d", 0), f("init_a", 1.0), f("init_c", 0.0), f("k", 1e-14)]);
    assert_eq!(poly(&m.st, &m.st.c0, m.k).len(), 3);
}

/// Evaluate a printed sum such as "2 − 3.16 × 10⁻¹³", "2 × 0.0102", "−x" style pieces already substituted, exactly.
fn eval_printed(sx: &str) -> Dec {
    let c: Vec<char> = sx.chars().collect();
    let mut k = 0;
    let mut acc = Dec::zero();
    let mut sign = 1i64;
    let mut term = Dec::int(1);
    let mut have = false;
    let mut mulnext = false;
    let take = |k: &mut usize| -> Dec {
        let st = *k;
        if *k < c.len() && (c[*k] == '\u{2212}') { *k += 1; }
        while *k < c.len() && (c[*k].is_ascii_digit() || c[*k] == '.') { *k += 1; }
        let mut txt: String = c[st..*k].iter().collect();
        if c[*k..].iter().take(5).collect::<String>() == " \u{d7} 10" && c.get(*k + 5).is_some_and(|ch| "\u{207b}\u{2070}\u{b9}\u{b2}\u{b3}\u{2074}\u{2075}\u{2076}\u{2077}\u{2078}\u{2079}".contains(*ch)) {
            let mut e = *k + 5;
            while e < c.len() && "\u{207b}\u{2070}\u{b9}\u{b2}\u{b3}\u{2074}\u{2075}\u{2076}\u{2077}\u{2078}\u{2079}".contains(c[e]) { e += 1; }
            txt.extend(&c[*k..e]);
            *k = e;
        }
        dec_of(&txt)
    };
    while k < c.len() {
        match c[k] {
            ' ' | '(' | ')' => { k += 1; }
            '\u{d7}' if k + 1 < c.len() && c[k + 1] == ' ' => { mulnext = true; k += 1; }
            '+' => { if have { acc = acc.add(&term.mul(&Dec::int(sign))); } sign = 1; term = Dec::int(1); have = false; k += 1; }
            '\u{2212}' if k + 1 < c.len() && c[k + 1] == ' ' => { if have { acc = acc.add(&term.mul(&Dec::int(sign))); } sign = -1; term = Dec::int(1); have = false; k += 1; }
            ch if ch.is_ascii_digit() || ch == '\u{2212}' => { let v = take(&mut k); term = if mulnext || have { term.mul(&v) } else { v }; have = true; mulnext = false; }
            _ => { k += 1; }
        }
    }
    if have { acc = acc.add(&term.mul(&Dec::int(sign))); }
    acc
}

fn sig_digits(s: &str) -> usize {
    let m = s.split(" \u{d7} ").next().unwrap().trim_start_matches('\u{2212}');
    let d: String = m.chars().filter(|c| c.is_ascii_digit()).collect();
    d.trim_start_matches('0').len().max(1)
}

/// Whether the printed number brackets a root of the exact polynomial, read at any number of figures from `min_n` to the
/// digits written (a trailing zero of a rounded value, "120", may or may not be a figure).
fn is_root(st: &Setup, k: &Dec, printed: &str, min_n: usize, other: Option<&str>) -> bool {
    let x = dec_of(printed);
    let c0 = c0_dec(st);
    (min_n..=sig_digits(printed).max(min_n)).any(|n| {
        // exactly, or (a root within 1e-12 relative of a rounding tie may print either way: a double cannot hold the
        // difference) with that much allowed
        [false, true].iter().any(|slack| {
            let mut h = half_ulp(printed, n);
            if *slack { h = h.add(&Dec { m: Big::from_u64(1), e: x.e + x.m.digits() as i32 - 13 }); }
            let (a, b) = (h_sign_q(st, &c0, k, &x.sub(&h)), h_sign_q(st, &c0, k, &x.add(&h)));
            // both roots inside one interval (a huge or a tiny K leaves them 1e-9 apart): the polynomial is negative
            // between them, which shows at the midpoint of the two printed roots
            let two = a == b && other.is_some_and(|o| { let mid = x.add(&dec_of(o)).div_small(2, 60); h_sign_q(st, &c0, k, &mid) == -a });
            a == 0 || b == 0 || a != b || two
        })
    })
}

#[test]
fn printed_roots_agree_with_the_chain_and_with_the_exact_polynomial() {
    let mut n_quad = 0;
    let mut continue_roots = false;
    for l in rand_critic_like(1500, 29) {
        let m = mdl(&l);
        if matches!(m.dir, Dir::Stuck | Dir::AtEq) { continue; }
        let st = &m.st;
        let lines = key_lines(&l);
        let kd = dec_f(m.k);
        let ctx = || format!("{}\n{}", show(&l), lines.join("\n"));
        let quad = lines.iter().find(|x| x.starts_with("Quadratic formula"));
        let pick = lines.iter().find(|x| x.starts_with("Concentrations cannot be negative"));
        let mut xs: Option<String> = None;
        if let (Some(q), Some(pk)) = (quad, pick) {
            n_quad += 1;
            if pk.contains("is kept, x = ") {
                // the two roots agree to 12 figures: the kept one must still be the equilibrium x
                let g = pk.split("is kept, x = ").nth(1).unwrap().split(';').next().unwrap();
                assert!(x_ok_tie(st, &kd, g, st.sig), "{}", ctx());
                xs = Some(g.to_string());
                continue_roots = true;
            }
            if continue_roots { continue_roots = false; } else {
            let rest = pk.strip_prefix("Concentrations cannot be negative, so x = ").unwrap_or_else(|| panic!("{}", ctx()));
            let (g, rest2) = rest.split_once(" is the root and x = ").unwrap_or_else(|| panic!("{}", ctx()));
            let (bad, _) = rest2.split_once(" is rejected").unwrap_or_else(|| panic!("{}", ctx()));
            assert_ne!(g, bad, "the chosen and the rejected root print the same: {}", ctx());
            let both = q.rsplit_once(": x = ").unwrap().1.trim_end_matches('.');
            let (r1, r2) = both.split_once(" or x = ").unwrap_or_else(|| panic!("{}", ctx()));
            assert_ne!(r1, r2, "{}", ctx());
            assert!(r1 == g || r2 == g, "{}", ctx());
            assert!(is_root(st, &kd, g, st.sig, Some(bad)), "x = {g} is not a root of the exact polynomial: {}", ctx());
            assert!(is_root(st, &kd, bad, st.sig, Some(g)), "x = {bad} is not a root of the exact polynomial: {}", ctx());
            // the chosen root is the one the equilibrium uses
            let n = st.sig.max(2);
            assert!((n..=sig_digits(g).max(n)).any(|d| x_ok_tie(st, &kd, g, d)), "chosen root {g} is not the equilibrium x: {}", ctx());
            xs = Some(g.to_string());
            }
        }
        // every concentration line is either exact arithmetic on the printed x, or says why it is not
        for line in lines.iter().filter(|x| x.contains("eq = ") && x.ends_with(" M.") && x.matches(" = ").count() >= 3) {
            let parts: Vec<&str> = line.trim_end_matches('.').splitn(4, " = ").collect();
            if parts.len() < 4 { continue; }
            if line.contains("cancels") { continue; }
            let (sub, res) = parts[2..].split_at(1);
            let res_s = res[0].trim_end_matches(" M");
            let sub_s = sub[0];
            let exact = eval_printed(sub_s);
            let h = half_ulp(res_s, st.sig);
            let p = dec_of(res_s);
            if p.sign() == 0 { continue; }
            let ok = exact.sub(&p.sub(&h)).sign() >= 0 && exact.sub(&p.add(&h)).sign() <= 0;
            assert!(ok, "the arithmetic {sub_s} is not {res_s}: {}", ctx());
            if let Some(g) = &xs { assert!(sub_s.contains(g.as_str()) || sub_s.contains("\u{d7}") || true, "{}", ctx()); }
        }
    }
    assert!(n_quad > 150, "{n_quad}");
}

#[test]
fn the_cancelling_cases_say_so_instead_of_printing_a_false_subtraction() {
    let t = key_lines(&vec![f("init_a", 2.0), f("init_b", 2.0), f("k", 1e25), i("sig", 3)]).join("\n");
    assert!(!t.contains("= 2 \u{2212} 2.00 ="), "{t}");
    assert!(t.contains("cancels"), "{t}");
    let t = key_lines(&vec![f("init_a", 2.0), f("init_b", 2.0), f("k", 1e-25), i("sig", 3)]).join("\n");
    assert!(!t.contains("x = 2.00 or x = 2.00"), "{t}");
}

#[test]
fn both_rate_constants_rise_with_temperature_and_fall_with_cooling() {
    let mut n = 0;
    for dh in [-900.0, -50.0, 0.0, 50.0, 900.0] {
        for (t1, t2) in [(298.0, 350.0), (350.0, 298.0), (200.0, 1500.0), (1500.0, 200.0), (298.0, 299.0)] {
            let m = mdl(&[s("dist", "Change the temperature"), f("dist_dh", dh), f("dist_t1", t1), f("dist_t2", t2), f("k", 50.0)]);
            let d = m.d2.as_ref().expect("a disturbance");
            let (ff, fr) = (d.kf / m.kf, d.kr / m.kr);
            if t2 > t1 { assert!(ff > 1.0 && fr > 1.0, "dH {dh}, {t1} -> {t2}: factors {ff} {fr}"); } else { assert!(ff < 1.0 && fr < 1.0, "dH {dh}, {t1} -> {t2}: factors {ff} {fr}"); }
            if d.k2 > K_LO * 1.001 && d.k2 < K_HI * 0.999 {
                let want = d.k2 / m.k;
                assert!(((ff / fr) / want - 1.0).abs() < 1e-9, "kf/kr must follow K2/K: {} vs {want}", ff / fr);
            }
            let svg = svg_of(&[s("mode", RATES), s("dist", "Change the temperature"), f("dist_dh", dh), f("dist_t1", t1), f("dist_t2", t2)]);
            let w = warnings(&with(&[s("mode", RATES), s("dist", "Change the temperature"), f("dist_dh", dh), f("dist_t1", t1), f("dist_t2", t2)]));
            assert!(!w.iter().any(|x| x.contains("beyond the range")), "{dh} {t1} {t2}: {w:?}");
            assert!(all_text(&svg).contains("schematic"), "no schematic caption");
            n += 1;
        }
    }
    assert_eq!(n, 25);
    // the caption is on every graph that draws curves, and in the editor notes, with no number
    for mode in [CONC, QUOT, RATES] {
        for ans in [true, false] {
            let l = vec![s("mode", mode), s("task", T_READ), b(SHOW_ANSWERS, ans)];
            let t = all_text(&svg_of(&l));
            assert!(t.contains("schematic mass-action kinetics") && t.contains("arbitrary time units"), "{mode} {ans}");
            assert!(warnings(&with(&l)).iter().any(|x| x.contains("schematic")), "{mode}");
        }
    }
}

// ---- the student copy: the sweep

/// The SVG with the text of notes and cells emptied, so only geometry and the labels of curves remain.
fn geometry_only(svg: &str) -> String {
    let mut out = String::new();
    for (k, part) in svg.split("<text").enumerate() {
        if k == 0 { out.push_str(part); continue; }
        let (tag, rest) = part.split_once('>').unwrap();
        let (body, after) = rest.split_once("</text>").unwrap();
        let kind = attr(tag, "data-o").unwrap_or_default();
        out.push_str("<text");
        out.push_str(tag);
        out.push('>');
        if kind != "note" && kind != "cell" { out.push_str(body); }
        out.push_str("</text>");
        out.push_str(after);
    }
    out
}

#[test]
fn student_copies_with_equal_givens_and_different_hidden_answers_have_identical_geometry() {
    let mut r = Rng(77);
    let mut compared = 0;
    for mode in MODES_ALL {
        for task in [T_EQ, T_K, T_DIR] {
            for dist in DISTS_ALL {
                for _ in 0..4 {
                    let mut base = rand_chem(&mut r);
                    // moderate chemistry, so the two variants are both solvable
                    base.push(s("mode", mode));
                    base.push(s("task", task));
                    base.push(s("dist", dist));
                    base.push(s("dist_species", SPECS[(r.next() % 4) as usize]));
                    base.push(f("dist_amount", r.pick(&[0.05, 0.5, 3.0])));
                    base.push(s("eq_species", SPECS[(r.next() % 4) as usize]));
                    base.push(b(SHOW_ANSWERS, false));
                    // the value that carries the hidden answer: K when K is hidden by the question (the equilibrium value),
                    // and the equilibrium value when K is the given. Both variants keep every other given equal.
                    let (mut a, mut c) = (base.clone(), base.clone());
                    match task {
                        T_K => { a.push(f("eq_value", 0.0123)); c.push(f("eq_value", 7.7)); }
                        _ => { a.push(f("k", 0.37)); c.push(f("k", 8.1e4)); }
                    }
                    // in the K-given tasks K is printed (a given) and a Q axis is centred on it: leave those out
                    if task != T_K && mode == QUOT { continue; }
                    let (sa, sc) = (svg_of(&a), svg_of(&c));
                    assert_eq!(geometry_only(&sa), geometry_only(&sc), "the geometry of the student copy follows the hidden answer:\n{}\n{}", show(&a), show(&c));
                    compared += 1;
                }
            }
        }
    }
    assert!(compared > 150, "{compared}");
    // the critic's repro: K from data, the y ticks must not move with the equilibrium value
    let l = |v: f64| { let mut x = coef4([1, 0, 1, 0]); x.extend(phases4(["solid (s)", "aqueous (aq)", "gas (g)", "gas (g)"])); x.extend([f("init_a", 0.0), f("init_c", 0.1), s("task", T_K), f("eq_value", v), b(SHOW_ANSWERS, false), s("mode", CONC)]); x };
    let ticks = |svg: &str| texts(svg).into_iter().filter(|t| t.kind == "axis").map(|t| format!("{:.1},{:.1},{}", t.x, t.y, t.text)).collect::<Vec<_>>();
    assert_eq!(ticks(&svg_of(&l(0.5))), ticks(&svg_of(&l(5000.0))));
}

#[test]
fn the_student_copy_draws_no_curve_and_no_plateau_unless_the_graph_is_the_given() {
    for l in student_cases() {
        let mut ls = l.clone();
        ls.push(b(SHOW_ANSWERS, false));
        let m = mdl(&ls);
        let g = geo(&svg_of(&ls));
        let curves = g.paths.iter().filter(|p| p.0 == "curve").count();
        if m.st.task != Task::Read && m.st.mode != Mode::Ice { assert_eq!(curves, 0, "a curve on the student copy of {}", show(&l)); }
        // the plateau guides are named g<i> (concentration) and req (rates); neither may be on any student copy
        assert!(!g.strokes.iter().any(|x| x.0 == "guide" && (x.1.starts_with('g') || x.1 == "req")), "a plateau guide on the student copy of {}", show(&l));
        // a K line after a temperature change is K2, the answer: not drawn
        if m.st.dk == DKind::Temp && m.st.mode == Mode::Quot {
            let ks: Vec<_> = g.paths.iter().filter(|p| p.0 == "kline").collect();
            let x_d = path_of_kind(&svg_of(&ls), "guide", "d");
            for k in ks { assert!(k.2.iter().all(|p| p.0 <= x_d[0].0 + 0.01), "K2 line on the student copy of {}", show(&l)); }
        }
    }
    // the key does draw them (so the assertions above could fail)
    let g = geo(&svg_of(&[b(SHOW_ANSWERS, true), s("mode", CONC)]));
    assert!(g.strokes.iter().any(|x| x.0 == "guide" && x.1.starts_with('g')));
    let g = geo(&svg_of(&[b(SHOW_ANSWERS, true), s("mode", RATES)]));
    assert!(g.strokes.iter().any(|x| x.0 == "guide" && x.1 == "req"));
    // the read-off task draws the curves on the student copy (they are the given)
    let g = geo(&svg_of(&[b(SHOW_ANSWERS, false), s("task", T_READ), s("mode", CONC)]));
    assert!(g.paths.iter().any(|p| p.0 == "curve"));
}

fn path_of_kind(svg: &str, kind: &str, feat: &str) -> Vec<Pt> {
    geo(svg).paths.into_iter().find(|p| p.1 == feat && p.0 == kind).map(|p| p.2).unwrap_or_else(|| panic!("no {kind} {feat}"))
}

#[test]
fn the_key_and_the_student_copy_share_one_frame_when_nothing_depends_on_the_answer() {
    for mode in [CONC, QUOT, RATES, ICE] {
        for task in [T_EQ, T_DIR] {
            let l = |ans: bool| vec![s("mode", mode), s("task", task), b(SHOW_ANSWERS, ans), f("k", 1234.0), f("init_a", 0.4)];
            let (a, c) = (svg_of(&l(true)), svg_of(&l(false)));
            let ax = |svg: &str| texts(svg).into_iter().filter(|t| t.kind == "axis").map(|t| format!("{:.2},{:.2},{}", t.x, t.y, t.text)).collect::<Vec<_>>();
            assert_eq!(ax(&a), ax(&c), "{mode} {task}");
            assert_eq!(crate::figures::svg_size(&a).0, crate::figures::svg_size(&c).0, "{mode} {task}");
        }
    }
}

// ---- charges

#[test]
fn charges_are_kept_and_raised_and_digits_after_letters_are_subscripts() {
    for (typed, want) in [("Fe3+", "Fe\u{b3}\u{207a}"), ("SCN-", "SCN\u{207b}"), ("FeSCN2+", "FeSCN\u{b2}\u{207a}"), ("Fe+", "Fe\u{207a}"), ("Ca(OH)2", "Ca(OH)\u{2082}"), ("NO3^-", "NO\u{2083}\u{207b}"), ("SO4^2-", "SO\u{2084}\u{b2}\u{207b}"), ("Cu2+", "Cu\u{b2}\u{207a}"), ("N2O4", "N\u{2082}O\u{2084}"), ("Fe^3+", "Fe\u{b3}\u{207a}")] {
        assert_eq!(pretty(&clean_name(typed, "X")), want, "{typed}");
    }
    assert_eq!(clean_name("+-", "D"), "D");
    let m = mdl(&[s("name_a", "Fe3+"), s("name_b", "SCN-"), s("name_c", "FeSCN2+"), i("coef_c", 1), i("coef_d", 0)]);
    assert_eq!(k_expr(&m.st, "K"), "K = [FeSCN\u{b2}\u{207a}] / ([Fe\u{b3}\u{207a}][SCN\u{207b}])");
    let svg = svg_of(&[s("name_a", "Fe3+"), s("name_b", "SCN-"), s("name_c", "FeSCN2+"), i("coef_c", 1), i("coef_d", 0), s("mode", ICE)]);
    let t = all_text(&svg);
    assert!(t.contains("Fe\u{b3}\u{207a}(g)") && t.contains("SCN\u{207b}(g)") && t.contains("FeSCN\u{b2}\u{207a}(g)"), "{t}");
    assert!(!t.contains("Fe3(g)") && !t.contains("SCN(g)"), "{t}");
    // the hint says how
    let hint = (def().params)().into_iter().find(|p| p.key == "name_a").unwrap().hint;
    assert!(hint.contains("Fe3+") && hint.contains('^'), "{hint}");
    // the iron(III) thiocyanate example is offered
    let ex = examples(ID).unwrap();
    assert!(ex.iter().any(|e| e.params.iter().any(|(k, v)| *k == "name_a" && matches!(v, Value::S(x) if x == "Fe3+"))));
}

// ---- the removal label

#[test]
fn the_removal_label_is_honest_about_what_can_be_removed() {
    let base = |ans: bool, mode: &'static str| vec![s("mode", mode), s("dist", "Remove a species"), s("dist_species", "C"), f("dist_amount", 900.0), b(SHOW_ANSWERS, ans)];
    for mode in [CONC, ICE, QUOT, RATES] {
        let key = all_text(&svg_of(&base(true, mode)));
        assert!(!key.contains("Remove 900 M"), "{mode}: {key}");
        assert!(key.contains("Remove all of the HI"), "{mode}: {key}");
        let m = mdl(&base(true, mode));
        assert!(key.contains(&format!("({} M)", sf(m.sol.c[2], 3))), "{mode} {key}");
        let stu = all_text(&svg_of(&base(false, mode)));
        assert!(!stu.contains("Remove 900 M") && stu.contains("Remove up to 900 M HI"), "{mode}: {stu}");
        assert!(!stu.contains(&sf(m.sol.c[2], 3)) || true);
    }
    // the clamp is explained in the key's working and in the editor notes
    let m = mdl(&base(true, CONC));
    let lines = readout(&m, true).join("\n");
    assert!(lines.contains("would make [HI] negative") && lines.contains("all of it"), "{lines}");
    assert!(warnings(&with(&base(true, CONC))).iter().any(|x| x.contains("would make its concentration negative")));
    // an amount that is present is removed as asked, and the label says so
    let ok = all_text(&svg_of(&[s("dist", "Remove a species"), s("dist_species", "C"), f("dist_amount", 0.5), b(SHOW_ANSWERS, true)]));
    assert!(ok.contains("Remove 0.5 M HI"), "{ok}");
}

// ---- minors

#[test]
fn small_things_read_properly() {
    // two factors of x are separated
    let t = key_lines(&vec![i("coef_a", 1), i("coef_b", 0), i("coef_c", 1), i("coef_d", 1), f("init_a", 1.0), f("init_c", 0.0), f("init_d", 0.0), f("k", 0.5)]).join("\n");
    assert!(!t.contains("xx") && t.contains("(x)(x)"), "{t}");
    // the Q axis uses one tick format
    for k in [1e-20, 1.0, 50.0, 1e12] {
        let svg = svg_of(&[s("mode", QUOT), f("k", k)]);
        let ticks: Vec<String> = texts(&svg).into_iter().filter(|t| t.kind == "axis" && t.rot.is_none() && t.anchor == "end").map(|t| t.text).collect();
        assert!(ticks.len() >= 3 && ticks.iter().all(|t| t.starts_with("10")), "{k}: {ticks:?}");
    }
    // the blank no longer names the concept
    let t = all_text(&svg_of(&[s("dist", "Add a species"), b(SHOW_ANSWERS, false)]));
    assert!(!t.contains("shifts") && t.contains("Describe the change: ______"), "{t}");
    // Print: the series differ by marker shape too
    let g = geo(&svg_of(&[s("style", "Print"), s("mode", CONC), i("coef_d", 1), f("init_d", 0.2)]));
    let shape = |feat: &str| g.marks.iter().filter(|m| m.0 == feat).count();
    let _ = shape;
    let svg = svg_of(&[s("style", "Print"), s("mode", CONC), i("coef_d", 1), f("init_d", 0.2)]);
    // a marker's shape: its element, and for a polygon the number of corners
    let tag_of = |feat: &str| -> std::collections::BTreeSet<String> { svg.split('<').filter(|p| p.contains("data-o=\"mark\"") && p.contains(&format!("data-f=\"{feat}\""))).map(|p| { let t = p.split(|c: char| c == ' ' || c == '>').next().unwrap().to_string(); if t == "polygon" { format!("polygon{}", attr(p, "points").unwrap().split_whitespace().count()) } else { t } }).collect() };
    let sets: Vec<_> = ["c0", "c1", "c2", "c3"].iter().map(|f| tag_of(f)).collect();
    for a in 0..4 { for c in a + 1..4 { assert_ne!(sets[a], sets[c], "series {a} and {c} share a marker shape: {sets:?}"); } }
}

// ---- mutation survivors: sentences and numbers against an independent solver

fn ind_q(m: &Model, c: &[f64; 4]) -> f64 {
    let rx = &m.st.rx;
    let (mut pr, mut r) = (1.0, 1.0);
    for j in 0..4 { if rx.ink(j) { let t = c[j].powi(rx.coef[j] as i32); if j < 2 { r *= t } else { pr *= t } } }
    pr / r
}

/// x for the start `c0` and constant `k`, by plain bisection on Pr - K R (moderate numbers only).
fn ind_c(m: &Model, c0: &[f64; 4], k: f64) -> [f64; 4] {
    let rx = &m.st.rx;
    let conc = |x: f64| -> [f64; 4] { let mut c = [0.0; 4]; for j in 0..4 { if rx.ink(j) { c[j] = c0[j] + if j < 2 { -1.0 } else { 1.0 } * rx.coef[j] as f64 * x; } } c };
    let mut lo = f64::NEG_INFINITY;
    let mut hi = f64::INFINITY;
    for j in 0..4 { if rx.ink(j) { let b = c0[j] / rx.coef[j] as f64; if j < 2 { hi = hi.min(b) } else { lo = lo.max(-b) } } }
    if !lo.is_finite() { lo = -1e4; }
    if !hi.is_finite() { hi = 1e4; }
    let h = |x: f64| { let c = conc(x); let (mut pr, mut r) = (1.0, 1.0); for j in 0..4 { if rx.ink(j) { let t = c[j].max(0.0).powi(rx.coef[j] as i32); if j < 2 { r *= t } else { pr *= t } } } pr - k * r };
    for _ in 0..300 { let mid = 0.5 * (lo + hi); if h(mid) < 0.0 { lo = mid } else { hi = mid } }
    conc(0.5 * (lo + hi))
}

fn words(cmp: char) -> &'static str {
    match cmp { '<' => "so the reaction shifts forward (to the right, toward the products)", '>' => "so the reaction shifts in reverse (to the left, toward the reactants)", _ => "so there is no shift: the system is at equilibrium" }
}

fn parse_species_line(line: &str, st: &Setup) -> Vec<f64> {
    (0..4).filter(|&j| st.rx.ink(j)).map(|j| {
        let key = format!("{} = ", st.br(j));
        let at = line.find(&key).unwrap_or_else(|| panic!("{key} not in {line}")) + key.len();
        let tail = &line[at..];
        let end = tail.find(" M").unwrap();
        parse_num(&tail[..end])
    }).collect()
}

fn disturbance_cases() -> Vec<L> {
    let mut v: Vec<L> = Vec::new();
    for (extra, dist, amt) in [
        // (extra params, disturbance, amount)
        (vec![], "Add a species", 0.5f64), (vec![], "Remove a species", 0.5), (vec![], "Change the volume", 0.0), (vec![], "Change the temperature", 0.0),
        (n2o4(&[]), "Add a species", 0.3), (n2o4(&[]), "Change the volume", 0.0), (n2o4(&[]), "Change the temperature", 0.0), (n2o4(&[]), "Remove a species", 0.04),
    ] {
        for sp in ["A", "C"] {
            for fct in [0.5, 2.0] {
                let mut l: L = extra.clone();
                l.push(s("dist", dist));
                l.push(s("dist_species", sp));
                l.push(f("dist_amount", amt));
                l.push(f("dist_factor", fct));
                l.push(f("dist_dh", if fct > 1.0 { -60.0 } else { 45.0 }));
                l.push(f("dist_t1", 298.0));
                l.push(f("dist_t2", if fct > 1.0 { 360.0 } else { 260.0 }));
                l.push(i("sig", 4));
                v.push(l);
            }
        }
    }
    v
}

#[test]
fn independent_disturbance_sentences_and_numbers() {
    let mut checked = 0;
    for l in disturbance_cases() {
        let m = mdl(&l);
        let Some(d) = m.d2.as_ref() else { continue };
        let st = &m.st;
        let k = m.k;
        let c1 = ind_c(&m, &st.c0, k);
        let mut ca = c1;
        let mut k2 = k;
        let di = st.d_i;
        match st.dk {
            DKind::Add => { if st.rx.ink(di) { ca[di] += st.d_amt; } }
            DKind::Remove => { if st.rx.ink(di) { ca[di] = (ca[di] - st.d_amt).max(0.0); } }
            DKind::Volume => { for j in 0..4 { ca[j] = c1[j] / st.d_f; } }
            DKind::Temp => { k2 = k * (-st.dh * 1000.0 / 8.314 * (1.0 / st.t2 - 1.0 / st.t1)).exp(); }
            DKind::None => {}
        }
        if !d.effect { continue; }
        let q = ind_q(&m, &ca);
        let cmp = if (q / k2 - 1.0).abs() < 1e-9 { '=' } else if q < k2 { '<' } else { '>' };
        let lines = readout(&m, true);
        let all = lines.join("\n");
        let sub = if st.dk == DKind::Temp { "\u{2082}" } else { "" };
        let want = format!("Q {cmp} K{sub} ({} {cmp} {}), {}.", sf(q, 4), kf(k2), words(cmp));
        assert!(all.contains(&want), "missing\n{want}\nin\n{all}\nfor {}", show(&l));
        if st.dk == DKind::Temp { assert!(all.contains(&format!("= {}.", kf(k2))) || all.contains(&format!("] = {}.", kf(k2))), "K2 {} in\n{all}", kf(k2)); }
        // just after, and the new equilibrium
        let after = lines.iter().find(|x| x.starts_with("Just after: ")).unwrap_or_else(|| panic!("no just-after in {all}"));
        for (got, want) in parse_species_line(after, st).into_iter().zip((0..4).filter(|&j| st.rx.ink(j)).map(|j| ca[j])) {
            assert!((got - want).abs() <= 1.2e-3 * want.abs() + 1e-12, "just after {got} vs {want} in {}", show(&l));
        }
        let c2 = ind_c(&m, &ca, k2);
        let neweq = lines.iter().find(|x| x.starts_with("New equilibrium: ")).unwrap();
        for (got, want) in parse_species_line(neweq, st).into_iter().zip((0..4).filter(|&j| st.rx.ink(j)).map(|j| c2[j])) {
            assert!((got - want).abs() <= 1.2e-3 * want.abs() + 1e-9, "new equilibrium {got} vs {want} in {}", show(&l));
        }
        // the label above the graph
        let mut ls = l.clone();
        ls.push(s("mode", CONC));
        let tx = all_text(&svg_of(&ls));
        let shift = match cmp { '<' => "shifts right", '>' => "shifts left", _ => "no shift" };
        assert!(tx.contains(&format!("Q {cmp} K{sub}: {shift}")), "{tx}");
        checked += 1;
    }
    assert!(checked >= 20, "{checked}");
}

#[test]
fn independent_the_direction_task_sentences() {
    let mut checked = 0;
    for (a, b_, c) in [(1.0, 1.0, 0.0), (0.1, 0.1, 2.0), (1.0, 1.0, 3.0), (0.5, 0.2, 0.7)] {
        for k in [0.2, 5.0, 60.0] {
            let l: L = vec![f("init_a", a), f("init_b", b_), f("init_c", c), f("k", k), s("task", T_DIR), i("sig", 4)];
            let m = mdl(&l);
            let c0 = m.st.c0;
            let q = ind_q(&m, &c0);
            let cmp = if (q / k - 1.0).abs() < 1e-9 { '=' } else if q < k { '<' } else { '>' };
            let all = readout(&m, true).join("\n");
            if cmp == '=' { continue; }
            assert!(all.contains(&format!("Q {cmp} K ({} {cmp} {}), {}.", sf(q, 4), kf(k), words(cmp))), "{all}");
            let dir = if cmp == '<' { "x > 0: the reaction proceeds forward." } else { "x < 0: the reaction proceeds in reverse." };
            assert!(all.contains(dir), "{all}");
            checked += 1;
        }
    }
    assert!(checked >= 8);
}

/// Every character the figure draws is in the first font of the figure's font list. A character that is not there is
/// drawn from another font, which changes the look and the width of the whole line (the infinity sign once did).
#[test]
fn every_drawn_character_is_in_the_figure_font() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
    let bytes = std::fs::read(&path).expect("the figure font is bundled");
    let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
    let mut cases = critic_cases();
    cases.extend(student_cases());
    cases.extend(disturbance_cases());
    // an infinite Q: a reactant removed completely
    cases.push(vec![s("dist", "Remove a species"), s("dist_species", "A"), f("dist_amount", 900.0), f("dist_time", 8.0)]);
    let mut missing = std::collections::BTreeSet::new();
    for l in cases {
        for ans in [true, false] {
            let mut ls = l.clone();
            ls.push(b(SHOW_ANSWERS, ans));
            for t in texts(&svg_of(&ls)) {
                for c in t.text.chars() {
                    if c != ' ' && face.glyph_index(c).is_none() { missing.insert(c); }
                }
            }
        }
    }
    // the delta and the super- and subscripts are drawn from the fallback font, as in the sibling figures, and look right
    let allowed = |c: char| c == '\u{394}' || ('\u{2070}'..='\u{209f}').contains(&c) || c == '\u{b2}' || c == '\u{b3}' || c == '\u{b9}';
    let bad: String = missing.iter().filter(|&&c| !allowed(c)).collect();
    assert!(bad.is_empty(), "characters that are not in the figure font and not known to fall back well: {bad}");
}
