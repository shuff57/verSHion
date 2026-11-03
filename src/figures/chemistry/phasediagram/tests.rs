//! Tests of the phase diagram. The model tests recompute the curves from the published data and the stated shape
//! rules without using the figure's own formulas; the label audit measures every text with the glyphs the PNG
//! renderer draws (usvg's own layout), not with the placer's width estimate.
use super::*;
use crate::figures::{registry, to_png};
use std::collections::{BTreeMap, HashMap};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("phase-diagram is registered") }
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
fn svg_of(list: &[(&'static str, Value)]) -> String { build(&with(list)).0 }
fn rep_of(list: &[(&'static str, Value)]) -> Report { build(&with(list)).1 }
fn key(list: &[(&'static str, Value)]) -> Report { let mut l = list.to_vec(); l.push(b(SHOW_ANSWERS, true)); rep_of(&l) }

const WATER: &str = "Water";
const CO2: &str = "Carbon dioxide";
const TYPICAL: &str = "Typical substance";
const CUSTOM: &str = "Custom";
const CELSIUS: &str = "Celsius (\u{b0}C)";
const LINEAR: &str = "Linear";
const BLANKLAB: &str = "Blank for students";
const OFF: &str = "Off";
const NEG: &str = "Negative (like water)";
const POS: &str = "Positive (most substances)";

fn close(a: f64, b: f64, rel: f64) -> bool { (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-300) }

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
            rot,
        });
    }
    out
}

fn all_text(svg: &str) -> String { texts(svg).into_iter().map(|t| t.text).collect::<Vec<_>>().join("\n") }
fn notes_text(svg: &str) -> String { texts(svg).into_iter().filter(|t| t.kind == "note").map(|t| t.text).collect::<Vec<_>>().join(" ") }
fn labels_of(svg: &str, feat: &str) -> Vec<String> { texts(svg).into_iter().filter(|t| t.kind == "label" && t.feat == feat).map(|t| t.text).collect() }
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
/// The strokes, markers, paths and region polygons drawn.
struct Geo { strokes: Vec<(String, String, Seg)>, marks: Vec<(String, (f64, f64, f64))>, paths: Vec<(String, String, Vec<Pt>, String)>, polys: Vec<(String, Vec<Pt>)> }

fn geo(svg: &str) -> Geo {
    let mut g = Geo { strokes: Vec::new(), marks: Vec::new(), paths: Vec::new(), polys: Vec::new() };
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
            if kind != "leader" { g.paths.push((kind, feat, pts, style)); }
        } else if tag.starts_with("circle ") {
            g.marks.push((feat, (attr_f(tag, "cx").unwrap(), attr_f(tag, "cy").unwrap(), attr_f(tag, "r").unwrap())));
        } else if tag.starts_with("polygon ") && kind == "region" {
            let pts: Vec<Pt> = attr(tag, "points").unwrap().split_whitespace().filter_map(|t| { let (a, b) = t.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
            g.polys.push((feat, pts));
        }
    }
    g
}

fn to_data(pts: &[Pt], fr: &Frame, log: bool, off: f64) -> Vec<Pt> {
    pts.iter()
        .map(|(x, y)| {
            let t = fr.x0 + (x - fr.left()) / (fr.right() - fr.left()) * (fr.x1 - fr.x0) + off;
            let v = fr.y0 + (fr.bottom() - y) / (fr.bottom() - fr.top()) * (fr.y1 - fr.y0);
            (t, if log { 10f64.powf(v) } else { v })
        })
        .collect()
}

fn inside(poly: &[Pt], p: Pt) -> bool {
    let mut c = false;
    let n = poly.len();
    let mut j = n - 1;
    for k in 0..n {
        let (a, b) = (poly[k], poly[j]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 { c = !c; }
        j = k;
    }
    c
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

/// Every problem with the text of one render, measured in real glyphs: text off the canvas, text on text, a label on
/// a solid stroke (curves, the isobar and isotherm, arrows, axes) or a marker, a region label not wholly inside its
/// own region, and a label that does not read as its own feature's (more than 40 px x size from it, or more than
/// 15 px from it while 6 px nearer another labelled feature). The dotted guides and the grid are thin lines a haloed
/// label may cross.
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
    // a leader never starts or ends on another feature's marker, where it would read as that feature's
    for (kind, feat, (p, q)) in &gm.strokes {
        if kind != "leader" { continue; }
        for (mf, (cx, cy, r)) in &gm.marks {
            if mf == feat { continue; }
            // two features' markers that nearly coincide cannot be told apart by any leader
            if gm.marks.iter().any(|(f2, (x2, y2, _))| f2 == feat && (x2 - cx).hypot(y2 - cy) < 18.0) { continue; }
            if (p.0 - cx).hypot(p.1 - cy) <= r + 3.0 { bad.push(format!("the leader of {feat} ends on the {mf} marker {p:?}-{q:?}")); }
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
        for (feat, (cx, cy, r)) in &gm.marks {
            if hits(*bx, (cx - r, cx + r, cy - r, cy + r)) { bad.push(format!("{:?} on the {feat} marker", t.text)); }
        }
        let e = groups.entry(t.group.clone()).or_insert((t.feat.clone(), *bx));
        e.1 = (e.1 .0.min(bx.0), e.1 .1.max(bx.1), e.1 .2.min(bx.2), e.1 .3.max(bx.3));
    }
    // a region label lies wholly inside its own region
    for (own, bx) in groups.values().filter(|g| g.0.starts_with("reg:")) {
        // a name that has no room in a thin region sits beside it on a leader line to a point inside
        if gm.strokes.iter().any(|(k, f, _)| k == "leader" && f == own) { continue; }
        let Some((_, poly)) = gm.polys.iter().find(|p| &p.0 == own) else { bad.push(format!("no polygon for {own}")); continue };
        let c = [(bx.0, bx.2), (bx.1, bx.2), (bx.0, bx.3), (bx.1, bx.3), ((bx.0 + bx.1) / 2.0, bx.2), ((bx.0 + bx.1) / 2.0, bx.3)];
        if c.iter().any(|q| !inside(poly, *q)) { bad.push(format!("a label of {own} is not wholly inside it: {bx:?}")); }
    }
    let owned: Vec<String> = { let mut v: Vec<String> = groups.values().map(|g| g.0.clone()).filter(|f| !f.starts_with("reg:")).collect(); v.sort(); v.dedup(); v };
    let dist = |feat: &str, bx: Bx| -> f64 {
        let mut d = f64::MAX;
        for (_, f, (p, q)) in &gm.strokes { if f == feat { d = d.min(seg_box(*p, *q, bx)); } }
        for (f, (cx, cy, _)) in &gm.marks { if f == feat { d = d.min(pt_box((*cx, *cy), bx)); } }
        d
    };
    for (own, bx) in groups.values().filter(|g| !g.0.starts_with("reg:")) {
        // a leader line names the feature outright
        if gm.strokes.iter().any(|(k, f, _)| k == "leader" && f == own) { continue; }
        let d = dist(own, *bx);
        let name = ts.iter().find(|t| t.kind == "label" && &t.feat == own).map(|t| t.text.clone()).unwrap_or_default();
        if d > 40.0 * lab { bad.push(format!("{name:?} is {d:.1} px from its {own} feature")); continue; }
        let other = owned.iter().filter(|o| *o != own).map(|o| (dist(o, *bx), o)).fold((f64::MAX, None), |a, (x, o)| if x < a.0 { (x, Some(o)) } else { a });
        if d > 15.0 && other.0 < d - 6.0 { bad.push(format!("{name:?} is {d:.1} px from its {own} feature but {:.1} px from {:?}", other.0, other.1)); }
    }
    bad
}

// ---------------------------------------------------------------------------------------------
// Numbers
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
            let mut tok: String = c[start..e].iter().collect();
            let rest: String = c[e..].iter().take(6).collect();
            if rest.starts_with(" \u{d7} 10") {
                let mut x = e + 5;
                while x < c.len() && "\u{207b}\u{2070}\u{b9}\u{b2}\u{b3}\u{2074}\u{2075}\u{2076}\u{2077}\u{2078}\u{2079}".contains(c[x]) { x += 1; }
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

/// A printed number back to a value: a plain decimal with its true minus sign, or "m x 10^n".
fn parse_num(s: &str) -> f64 {
    let s = s.trim().replace('\u{2212}', "-");
    if let Some((m, e)) = s.split_once(" \u{d7} 10") {
        let e: String = e.chars().map(|c| match c { '\u{207b}' => '-', '\u{2070}' => '0', '\u{b9}' => '1', '\u{b2}' => '2', '\u{b3}' => '3', '\u{2074}' => '4', '\u{2075}' => '5', '\u{2076}' => '6', '\u{2077}' => '7', '\u{2078}' => '8', '\u{2079}' => '9', c => c }).collect();
        return m.trim().parse::<f64>().unwrap() * 10f64.powi(e.trim().parse::<i32>().unwrap());
    }
    s.parse().unwrap_or_else(|_| panic!("not a number: {s:?}"))
}

/// The printed number right after `pat`: "273.15", "\u{2212}78.45" or "1.5 x 10^-4".
fn num_after(s: &str, pat: &str) -> String {
    let rest = s.split(pat).nth(1).unwrap_or_else(|| panic!("no {pat:?} in {s:?}"));
    let w: Vec<&str> = rest.split(' ').collect();
    let first = w[0].trim_end_matches(|c: char| c == ';' || c == ',' || c == ':' || c == ')' || c == '.' && w[0].ends_with('.') && !w[0][..w[0].len() - 1].ends_with(|d: char| d.is_ascii_digit() && false));
    if w.len() >= 3 && w[1] == "\u{d7}" && w[2].starts_with("10") { format!("{first} \u{d7} {}", w[2].trim_end_matches(|c: char| c == ';' || c == ',' || c == ':' || c == ')' || c == '.')) } else { first.to_string() }
}

fn line_starting<'a>(lines: &'a [String], head: &str) -> Option<&'a String> { lines.iter().find(|l| l.starts_with(head)) }
fn joined(r: &Report) -> String { r.readout.join("\n") }

/// The reference model of a one-component diagram written from the stated shape rules and the published data, not
/// from the figure's code: the vaporisation curve is ln P linear in 1/T through the triple and critical points, the
/// sublimation curve is the same kind of curve through the triple point and 15% steeper, and the (schematic) melting
/// curve leaves the triple point at 0.05 min(Tc - Tt, 2 Tt) per critical pressure and bends smoothly to be vertical
/// at three times that, to the left for a negative slope.
struct Ref { tt: f64, pt: f64, tc: f64, pc: f64, neg: bool }
impl Ref {
    fn bv(&self) -> f64 { (self.pc / self.pt).ln() / (1.0 / self.tt - 1.0 / self.tc) }
    fn tv(&self, p: f64) -> f64 { 1.0 / (1.0 / self.tt - (p / self.pt).ln() / self.bv()) }
    fn ts(&self, p: f64) -> f64 { 1.0 / (1.0 / self.tt - (p / self.pt).ln() / (1.15 * self.bv())) }
    fn tm(&self, p: f64) -> f64 {
        let k = 0.05 * (self.tc - self.tt).min(2.0 * self.tt);
        self.tt + (if self.neg { -1.0 } else { 1.0 }) * 3.0 * k * (1.0 - (-(p - self.pt).max(0.0) / (3.0 * self.pc)).exp())
    }
}

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
    for want in ["Water heated", "Carbon dioxide", "phase at", "Supercritical"] {
        assert!(names.iter().any(|n| n.contains(want)), "no {want} example in {names:?}");
    }
    let keys: Vec<&str> = (d.params)().iter().map(|p| p.key).collect();
    for k in [
        "substance", "unit", "paxis", "tt", "pt", "tc", "pc", "melt_slope", "isobar", "p_line", "isotherm", "t_line", "n_points", "a_t", "a_p", "b_t", "b_p", "c_t", "c_p", "d_t", "d_p", "path",
        "lab_regions", "lab_curves", "lab_special", "values", "grid", "dec", SHOW_ANSWERS, "style", "label_size",
    ] {
        assert!(keys.contains(&k), "no {k} param");
    }
    assert!(crate::figures::chemistry::skin_intro(ID).is_some());
    // the editor notes reach the panel
    assert!(!crate::figures::notes_for(ID, &with(&[s("substance", CUSTOM), f("tt", 400.0), f("tc", 300.0)])).is_empty());
    // and every example renders with no note and a clean description
    for e in crate::figures::examples_for(ID) {
        let p = with(&e.params);
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
        assert!(def().svg(&p).contains("<svg"));
    }
}

// ---------------------------------------------------------------------------------------------
// The physics: data, shapes, topology
// ---------------------------------------------------------------------------------------------

#[test]
fn the_presets_carry_the_real_data() {
    let w = preset(WATER).unwrap();
    assert_eq!((w.tt, w.pt, w.tc, w.pc, w.neg), (273.16, 0.00603, 647.0, 218.0, true));
    let c = preset(CO2).unwrap();
    assert_eq!((c.tt, c.pt, c.tc, c.pc, c.neg), (216.6, 5.11, 304.2, 72.8, false));
    let t = preset(TYPICAL).unwrap();
    assert!(!t.neg && t.pt < 1.0 && t.pc > 1.0, "a typical substance has a liquid range at 1 atm");
    assert!(preset(CUSTOM).is_none());
    // the key prints them as the givens, in kelvin and in degrees Celsius
    let k = joined(&key(&[]));
    assert!(k.contains("Triple point: T = 273.16 K, P = 0.00603 atm. Critical point: T = 647 K, P = 218 atm."), "{k}");
    let k = joined(&key(&[s("substance", CO2)]));
    assert!(k.contains("Triple point: T = 216.6 K, P = 5.11 atm. Critical point: T = 304.2 K, P = 72.8 atm."), "{k}");
    let k = joined(&key(&[s("unit", CELSIUS)]));
    assert!(k.contains("Triple point: T = 0.01 \u{b0}C, P = 0.00603 atm. Critical point: T = 373.85 \u{b0}C, P = 218 atm."), "{k}");
    let k = joined(&key(&[s("substance", CO2), s("unit", CELSIUS)]));
    assert!(k.contains("T = \u{2212}56.55 \u{b0}C, P = 5.11 atm. Critical point: T = 31.05 \u{b0}C"), "{k}");
}

#[test]
fn the_curves_pass_through_the_data_and_have_the_stated_shape() {
    let w = preset(WATER).unwrap();
    // through the triple and critical points, and the published normal boiling point
    assert!(close(w.pvap(273.16), 0.00603, 1e-9) && close(w.pvap(647.0), 218.0, 1e-9));
    assert!(close(w.pvap(373.15), 1.0, 1e-9), "water boils at 373.15 K and 1 atm: {}", w.pvap(373.15));
    assert!(close(w.tvap(1.0), 373.15, 1e-9));
    assert!(close(w.tm(1.0), 273.15, 1e-7), "ice melts at 273.15 K and 1 atm: {}", w.tm(1.0));
    assert!(close(w.tm(w.pt), 273.16, 1e-12));
    let c = preset(CO2).unwrap();
    assert!(close(c.pvap(216.6), 5.11, 1e-9) && close(c.pvap(304.2), 72.8, 1e-9));
    assert!(close(c.psub(194.7), 1.0, 1e-9) && close(c.tsub(1.0), 194.7, 1e-9), "dry ice sublimes at 194.7 K and 1 atm");
    assert!(close(c.psub(216.6), 5.11, 1e-9));
    // melting slope: water's melting curve leans left, carbon dioxide's right, and both stay near the triple temperature
    for p in [10.0, 100.0, 218.0, 600.0] {
        assert!(w.tm(p) < w.tm(p / 2.0) && w.tm(p) > w.tt - 0.2 * (w.tc - w.tt), "water {p}");
    }
    for p in [10.0, 100.0, 200.0] {
        assert!(c.tm(p) > c.tm(p / 2.0) && c.tm(p) < c.tt + 0.2 * (c.tc - c.tt), "co2 {p}");
    }
    // the two real melting curves are nearly vertical: well under 1 K per 10 atm, not the schematic lean
    assert!(w.tm(1.0) - w.tm(11.0) < 0.1 && c.tm(15.0) - c.tm(5.11) < 0.2);
    // the sublimation curve is steeper (in ln P against 1/T) than the vaporisation curve at the triple point
    for sb in [w, c, preset(TYPICAL).unwrap()] {
        let (t1, t2) = (sb.tt * 0.999, sb.tt * 1.001);
        let slope_sub = (sb.psub(t1).ln() - sb.pt.ln()) / (1.0 / t1 - 1.0 / sb.tt);
        let slope_vap = (sb.pvap(t2).ln() - sb.pt.ln()) / (1.0 / t2 - 1.0 / sb.tt);
        assert!(slope_sub < slope_vap && slope_sub < 0.0, "{}: {slope_sub} {slope_vap}", sb.name);
        // vaporisation rises with temperature, and so does sublimation
        let mut last = 0.0;
        for k in 0..=100 { let t = sb.tt + (sb.tc - sb.tt) * k as f64 / 100.0; let p = sb.pvap(t); assert!(p > last, "{}", sb.name); last = p; }
        let mut last = 0.0;
        for k in 1..=100 { let t = sb.tt * k as f64 / 100.0; let p = sb.psub(t); assert!(p >= last, "{}", sb.name); last = p; }
    }
}

#[test]
fn a_custom_substance_follows_clausius_clapeyron_through_the_two_points() {
    let mut seed = 5u64;
    let mut r = move || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (seed >> 11) as f64 / (1u64 << 53) as f64 };
    for _ in 0..400 {
        let tt = 20.0 + 600.0 * r();
        let tc = tt * (1.2 + 3.0 * r());
        let pt = 10f64.powf(-5.0 + 6.0 * r());
        let pc = pt * 10f64.powf(0.5 + 5.0 * r());
        let neg = r() < 0.5;
        let sb = Sub::new("x", tt, pt, tc, pc, neg);
        let rf = Ref { tt, pt, tc, pc, neg };
        for _ in 0..6 {
            let p = pt * (pc / pt).powf(r());
            assert!(close(sb.tvap(p), rf.tv(p), 1e-9), "vap {tt} {pt} {tc} {pc} {p}");
            assert!(close(sb.pvap(sb.tvap(p)), p, 1e-9));
            let q = pt * 10f64.powf(-3.0 * r());
            assert!(close(sb.tsub(q), rf.ts(q), 1e-9), "sub");
            assert!(close(sb.psub(sb.tsub(q)), q, 1e-7));
            let hi = pt * 10f64.powf(4.0 * r());
            assert!(close(sb.tm(hi), rf.tm(hi), 1e-9), "melt {tt} {pt} {tc} {pc} {hi}");
        }
        // topology: the melting curve stays left of the vaporisation curve above the triple point, and below the critical temperature
        for k in 1..=40 {
            let p = pt * (pc / pt).powf(k as f64 / 41.0);
            assert!(sb.tm(p) < sb.tvap(p), "melting meets vaporisation: {tt} {pt} {tc} {pc} {p}");
        }
        for k in 0..=40 { let p = pt * 10f64.powf(8.0 * k as f64 / 40.0); assert!(sb.tm(p) < tc && sb.tm(p) > 0.0, "{tt} {tc} {p}"); }
        // the inverse of the melting curve
        for k in 1..=20 {
            let p = pt + pc * 2.9 * k as f64 / 20.0;
            if let Some(q) = sb.pm(sb.tm(p)) { assert!(close(p, q, 1e-6), "pm {p} {q}"); } else { panic!("no pm for {p}") }
        }
    }
}

#[test]
fn the_phase_at_a_point_follows_the_real_diagram() {
    let w = preset(WATER).unwrap();
    let c = preset(CO2).unwrap();
    use Phase::*;
    let one = |sb: &Sub, t: f64, p: f64| sb.classify(t, p, true);
    // water (kelvin, atm)
    assert_eq!(one(&w, 300.0, 1.0), Where::In(Liquid));
    assert_eq!(one(&w, 250.0, 1.0), Where::In(Solid));
    assert_eq!(one(&w, 400.0, 1.0), Where::In(Gas));
    assert_eq!(one(&w, 300.0, 0.001), Where::In(Gas));
    assert_eq!(one(&w, 200.0, 0.001), Where::In(Solid));
    assert_eq!(one(&w, 700.0, 1.0), Where::In(Gas));
    assert_eq!(one(&w, 700.0, 300.0), Where::In(Super));
    assert_eq!(one(&w, 500.0, 300.0), Where::In(Liquid));
    assert_eq!(one(&w, 640.0, 218.5), Where::In(Liquid));
    assert_eq!(one(&w, 650.0, 100.0), Where::In(Gas), "above the critical temperature below the critical pressure is a gas");
    // melting: ice turns to water on compression at a temperature just under 0 C
    assert_eq!(one(&w, 270.0, 100.0), Where::In(Solid), "ice at 270 K stays solid at 100 atm (it melts at 272.4 K)");
    assert_eq!(one(&w, 268.15, 100.0), Where::In(Solid));
    assert_eq!(one(&w, 253.15, 500.0), Where::In(Solid));
    assert_eq!(one(&w, 273.0, 100.0), Where::In(Liquid));
    // carbon dioxide
    assert_eq!(one(&c, 250.0, 30.0), Where::In(Liquid));
    assert_eq!(one(&c, 250.0, 1.0), Where::In(Gas));
    assert_eq!(one(&c, 200.0, 10.0), Where::In(Solid));
    assert_eq!(one(&c, 300.0, 100.0), Where::In(Liquid));
    assert_eq!(one(&c, 320.0, 100.0), Where::In(Super));
    assert_eq!(one(&c, 190.0, 1.0), Where::In(Solid));
    assert_eq!(one(&c, 195.0, 1.0), Where::In(Gas));
    assert_eq!(one(&c, 300.0, 1.0), Where::In(Gas));
    // special places
    assert_eq!(one(&w, 273.16, 0.00603), Where::Triple);
    assert_eq!(one(&c, 304.2, 72.8), Where::Critical);
    assert_eq!(one(&c, 194.7, 1.0), Where::Between(Solid, Gas));
    assert_eq!(one(&w, 373.15, 1.0), Where::Between(Liquid, Gas));
    assert_eq!(one(&w, w.tm(1.0), 1.0), Where::Between(Solid, Liquid));
    assert_eq!(one(&w, 273.16, 0.001), Where::In(Gas), "just below the triple pressure at the triple temperature is gas");
    // without the tolerance a curve point is on one side or the other, never a boundary
    assert!(matches!(c.classify(194.7, 1.0, false), Where::In(_)));
}

#[test]
fn the_phase_sequences_along_a_line_match_a_brute_force_scan() {
    let mut seed = 99u64;
    let mut r = move || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (seed >> 11) as f64 / (1u64 << 53) as f64 };
    let mut subs = vec![preset(WATER).unwrap(), preset(CO2).unwrap(), preset(TYPICAL).unwrap()];
    for _ in 0..60 {
        let tt = 30.0 + 500.0 * r();
        let tc = tt * (1.1 + 2.0 * r());
        let pt = 10f64.powf(-4.0 + 5.0 * r());
        let pc = pt * 10f64.powf(0.4 + 4.0 * r());
        subs.push(Sub::new("x", tt, pt, tc, pc, r() < 0.5));
    }
    for sb in &subs {
        // heating along isobars
        for _ in 0..25 {
            let p = 10f64.powf(-6.0 + 11.0 * r());
            if sb.limit().is_some_and(|l| p > l.0) { continue; }
            let (case, steps) = sb.heating(p);
            let mut scan: Vec<(Phase, f64)> = Vec::new();
            let n = 40_000;
            let (lo, hi) = (sb.tt * 0.05, sb.tc * 3.0);
            for k in 0..=n {
                let t = lo + (hi - lo) * k as f64 / n as f64;
                let Where::In(ph) = sb.classify(t, p, false) else { unreachable!() };
                if scan.last().map(|x| x.0) != Some(ph) { scan.push((ph, t)); }
            }
            // supercritical fluid is reached gradually: the scan sees it, the changes list does not
            let changes: Vec<(Phase, f64)> = scan.iter().cloned().filter(|x| x.0 != Phase::Super).collect();
            // a melting curve that climbs past the critical temperature melts straight into the supercritical fluid
            let direct = steps[0].to == Phase::Super;
            let want: Vec<Phase> = std::iter::once(steps[0].from).chain(steps.iter().map(|c| c.to)).filter(|ph| !direct || *ph != Phase::Super).collect();
            let got: Vec<Phase> = changes.iter().map(|x| x.0).collect();
            if case == HCase::AtTriple { continue; }
            assert_eq!(got, want, "{} {:?} P={p} case {case:?} scan {scan:?}", sb.name, (sb.tt, sb.pt, sb.tc, sb.pc));
            if direct { assert!((steps[0].t - scan[1].1).abs() <= (hi - lo) / n as f64 * 1.5 + 1e-9, "direct {scan:?}"); } else {
                for (c, (_, t)) in steps.iter().zip(changes.iter().skip(1)) {
                    assert!((c.t - t).abs() <= (hi - lo) / n as f64 * 1.5 + 1e-9, "{} {c:?} vs scan {t}", sb.name);
                }
            }
            // the case names where the pressure is
            let want_case = if p < sb.pt * (1.0 - 1e-6) { HCase::Below } else if p < sb.pc * (1.0 - 1e-6) { HCase::Between } else { HCase::Above };
            if (p / sb.pt - 1.0).abs() > 1e-5 && (p / sb.pc - 1.0).abs() > 1e-5 { assert_eq!(case, want_case); }
        }
        // raising the pressure along isotherms
        for _ in 0..25 {
            let t = sb.tt * (0.2 + 2.5 * r()).min(sb.tc * 2.0 / sb.tt);
            let (plo, mut phi) = (sb.pt * 1e-6, sb.pc * 1e3);
            // water is modelled only as far as ice Ih: no isotherm below its end, nothing above 2070 atm
            if let Some((pm, tmin)) = sb.limit() { if t < tmin { continue; } phi = phi.min(pm); }
            let along = sb.along(t, plo, phi);
            let n = 40_000;
            let mut scan: Vec<(Phase, f64)> = Vec::new();
            for k in 0..=n {
                let p = plo * (phi / plo).powf(k as f64 / n as f64);
                let Where::In(ph) = sb.classify(t, p, false) else { unreachable!() };
                if scan.last().map(|x| x.0) != Some(ph) { scan.push((ph, p)); }
            }
            let changes: Vec<(Phase, f64)> = scan.iter().cloned().filter(|x| x.0 != Phase::Super).collect();
            // the scan skips the supercritical stretch; compare the real changes only
            let real: Vec<&Cross> = along.iter().filter(|c| c.curve != Curve::Critical).collect();
            let mut want: Vec<Phase> = vec![];
            if let Some(c0) = real.first() { want.push(c0.from); }
            want.extend(real.iter().map(|c| c.to));
            let got: Vec<Phase> = changes.iter().map(|x| x.0).collect();
            // above the critical temperature the only boundary is the solid's (a melting curve that has climbed past it)
            if t > sb.tc {
                assert!(real.iter().all(|c| c.curve == Curve::Melt && c.from == Phase::Super && c.to == Phase::Solid), "above the critical temperature there is no boundary but the melting curve's: {real:?}");
                for c in &real { assert!(scan.iter().any(|(ph, p)| *ph == Phase::Solid && close(c.p, *p, 2e-3)), "{c:?} vs {scan:?}"); }
                continue;
            }
            if real.is_empty() { assert!(got.len() <= 1, "{} T={t} {scan:?}", sb.name); continue; }
            assert_eq!(got, want, "{} {:?} T={t} scan {scan:?} along {along:?}", sb.name, (sb.tt, sb.pt, sb.tc, sb.pc));
            for (c, (_, p)) in real.iter().zip(changes.iter().skip(1)) { assert!(close(c.p, *p, 2e-3), "{} {c:?} vs scan {p}", sb.name); }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// What the key prints
// ---------------------------------------------------------------------------------------------

#[test]
fn water_at_one_atmosphere_melts_at_273_and_boils_at_373() {
    for (unit, mu, bu) in [("Kelvin (K)", "273.15 K", "373.15 K"), (CELSIUS, "0.00 \u{b0}C", "100.00 \u{b0}C")] {
        let r = key(&[s("unit", unit), i("dec", 2)]);
        let l = line_starting(&r.readout, "Normal melting point").unwrap_or_else(|| panic!("{:?}", r.readout));
        assert!(l.starts_with(&format!("Normal melting point = {mu}; normal boiling point = {bu} (read from the diagram and rounded to")), "{l}");
        assert!(r.readout.iter().any(|x| x.starts_with("Heating along the isobar at 1 atm: solid \u{2192} liquid at ") && x.contains(", then liquid \u{2192} gas at ")), "{:?}", r.readout);
        // the default is the nearest degree
        let r = key(&[s("unit", unit)]);
        let l = line_starting(&r.readout, "Normal melting point").unwrap();
        let (m0, b0) = if unit == CELSIUS { ("0 \u{b0}C", "100 \u{b0}C") } else { ("273 K", "373 K") };
        assert!(l.starts_with(&format!("Normal melting point = {m0}; normal boiling point = {b0} (read from the diagram and rounded to the nearest degree)")), "{l}");
    }
    // the same pressure, another number: not "normal"
    let r = key(&[f("p_line", 2.0), i("dec", 1)]);
    let l = line_starting(&r.readout, "Melting point at 2 atm").unwrap_or_else(|| panic!("{:?}", r.readout));
    assert!(l.contains("; boiling point at 2 atm = "), "{l}");
    assert!(!joined(&r).contains("Normal"), "{}", joined(&r));
}

#[test]
fn carbon_dioxide_sublimes_at_one_atmosphere_and_says_it_has_no_boiling_point() {
    for (unit, v) in [("Kelvin (K)", "194.70 K"), (CELSIUS, "\u{2212}78.45 \u{b0}C")] {
        let r = key(&[s("substance", CO2), s("unit", unit), i("dec", 2)]);
        let j = joined(&r);
        assert!(j.contains(&format!("Normal sublimation point = {v} (read from the diagram and rounded to")), "{j}");
        assert!(j.contains("Carbon dioxide has no normal melting point and no normal boiling point."), "{j}");
        assert!(j.contains("solid \u{2192} gas at ") && j.contains("(sublimation)"), "{j}");
        assert!(j.contains("1 atm is below the triple-point pressure (5.11 atm), so there is no liquid at this pressure"), "{j}");
        assert!(!j.to_lowercase().contains("boiling point = "), "no boiling point is invented: {j}");
        assert!(!j.contains("melting point = "), "{j}");
        assert!(r.heat.as_ref().unwrap().1.len() == 1 && r.heat.as_ref().unwrap().0 == HCase::Below);
    }
    // 5.11 atm is the triple point; 6 atm has a liquid range
    let r = key(&[s("substance", CO2), f("p_line", 6.0)]);
    assert!(r.heat.as_ref().unwrap().1.len() == 2 && joined(&r).contains("Melting point at 6 atm"), "{}", joined(&r));
}

#[test]
fn above_the_critical_pressure_there_is_no_boiling_and_no_phase_change_to_the_fluid() {
    let r = key(&[f("p_line", 300.0)]);
    let j = joined(&r);
    assert!(j.contains("so the liquid does not boil") && j.contains("above the critical pressure (218 atm)"), "{j}");
    assert!(j.contains("supercritical fluid gradually, without a phase change"), "{j}");
    assert!(j.contains("There is no boiling point at this pressure."), "{j}");
    assert!(!j.contains("liquid \u{2192} gas"), "{j}");
    let h = r.heat.unwrap();
    assert_eq!(h.0, HCase::Above);
    assert_eq!(h.1.len(), 1, "only the melting is a phase change");
    // exactly the critical pressure
    let r = key(&[f("p_line", 218.0)]);
    assert_eq!(r.heat.as_ref().unwrap().0, HCase::AtCritical);
    assert!(joined(&r).contains("passes through the critical point"), "{}", joined(&r));
    // exactly the triple pressure: no stable liquid range
    let r = key(&[s("substance", CO2), f("p_line", 5.11)]);
    assert_eq!(r.heat.as_ref().unwrap().0, HCase::AtTriple);
    assert!(joined(&r).contains("passes through the triple point (216.6 K)"), "{}", joined(&r));
}

#[test]
fn an_isotherm_lists_the_changes_as_the_pressure_rises() {
    // water at 300 K: gas to liquid only
    let r = key(&[b("isotherm", true), f("t_line", 300.0)]);
    let a = r.along.clone().unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!((a[0].from, a[0].to, a[0].curve), (Phase::Gas, Phase::Liquid, Curve::Vap));
    let j = joined(&r);
    let l = line_starting(&r.readout, "Raising the pressure along the isotherm at 300 K").unwrap();
    assert!(l.contains("gas \u{2192} liquid at about "), "{l}");
    // the pressure read is the vaporisation pressure of water at 300 K from the model (about 0.035 atm in the diagram)
    let w = preset(WATER).unwrap();
    let want = {
        let v = w.pvap(300.0);
        let e = v.log10().floor() as i32;
        let m = (v / 10f64.powi(e) * 10.0).round() / 10.0;
        parse_num(&format!("{m}")) * 10f64.powi(e)
    };
    let got = parse_num(&num_after(l, "gas \u{2192} liquid at about "));
    assert!(close(got, want, 1e-9), "{got} {want} {j}");
    // carbon dioxide at 220 K: gas, liquid, solid
    let r = key(&[s("substance", CO2), b("isotherm", true), f("t_line", 220.0)]);
    let a = r.along.unwrap();
    assert_eq!(a.iter().map(|c| (c.from, c.to)).collect::<Vec<_>>(), vec![(Phase::Gas, Phase::Liquid), (Phase::Liquid, Phase::Solid)]);
    // above the critical temperature nothing condenses
    let r = key(&[s("substance", CO2), b("isotherm", true), f("t_line", 320.0)]);
    assert!(r.along.as_ref().unwrap().is_empty());
    let j = joined(&r);
    assert!(j.contains("no phase boundary is crossed") && j.contains("never condenses") && j.contains("(a gradual change, not a phase change)"), "{j}");
    // both lines: the phase where they cross
    let r = key(&[s("substance", CO2), b("isotherm", true), f("t_line", 320.0), f("p_line", 100.0)]);
    assert_eq!(r.xsect, Some(Where::In(Phase::Super)));
    assert!(joined(&r).contains("(where the isobar and the isotherm cross): supercritical fluid."), "{}", joined(&r));
}

#[test]
fn points_a_to_d_have_their_phase_on_the_key_and_a_blank_for_students() {
    let list = [i("n_points", 4)];
    let r = key(&list);
    let want = [Where::In(Phase::Solid), Where::In(Phase::Liquid), Where::In(Phase::Gas), Where::In(Phase::Super)];
    assert_eq!(r.points.iter().map(|p| p.at).collect::<Vec<_>>(), want);
    let j = joined(&r);
    for (k, w) in ["solid", "liquid", "gas", "supercritical fluid"].iter().enumerate() {
        let l = line_starting(&r.readout, &format!("Phase at {}", ['A', 'B', 'C', 'D'][k])).unwrap();
        assert!(l.ends_with(&format!(" = {w}")), "{l}");
    }
    assert!(j.contains("Phase at A (T = 250 K, P = 100 atm) = solid"), "{j}");
    let mut l2 = list.to_vec();
    l2.push(b(SHOW_ANSWERS, false));
    let st = rep_of(&l2);
    let j = joined(&st);
    assert!(j.contains("Phase at A (T = 250 K, P = 100 atm) = ______") && j.contains("Phase at D (T = 700 K, P = 400 atm) = ______"), "{j}");
    // on a curve: both phases named; at the triple and critical points: said so
    let r = key(&[i("n_points", 4), f("a_t", 373.15), f("a_p", 1.0), f("b_t", 273.16), f("b_p", 0.00603), f("c_t", 647.0), f("c_p", 218.0), f("d_t", 273.15), f("d_p", 1.0)]);
    let j = joined(&r);
    assert!(j.contains("Phase at A (T = 373.15 K, P = 1 atm) = on the boundary between liquid and gas"), "{j}");
    assert!(j.contains("Phase at B (T = 273.16 K, P = 0.00603 atm) = at the triple point: solid, liquid and gas are in equilibrium"), "{j}");
    assert!(j.contains("Phase at C (T = 647 K, P = 218 atm) = at the critical point: the liquid and the gas are identical"), "{j}");
    assert!(j.contains("= on the boundary between solid and liquid"), "{j}");
    // and the editor is told
    assert!(r.notes.iter().any(|n| n.contains("Point A is exactly on a curve")), "{:?}", r.notes);
}

#[test]
fn every_printed_reading_is_recomputed_from_the_printed_givens() {
    let mut seed = 321u64;
    let mut rnd = move || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (seed >> 11) as f64 / (1u64 << 53) as f64 };
    let sig = |v: f64, n: i32| { let e = v.abs().log10().floor() as i32; let m = 10f64.powi(n - 1 - e); (v * m).round() / m };
    for case in 0..300 {
        let celsius = case % 3 == 0;
        let off = if celsius { 273.15 } else { 0.0 };
        let tt_k = sig(30.0 + 500.0 * rnd(), 4);
        let tc_k = sig(tt_k * (1.15 + 2.0 * rnd()), 4);
        let pt = sig(10f64.powf(-4.0 + 4.5 * rnd()), 3);
        let pc = sig(pt * 10f64.powf(0.5 + 3.5 * rnd()), 3);
        let neg = rnd() < 0.5;
        let p_line = sig(10f64.powf(-3.0 + 5.0 * rnd()), 3);
        let dec = (rnd() * 3.0) as i64;
        let list = vec![
            s("substance", CUSTOM), s("unit", if celsius { CELSIUS } else { "Kelvin (K)" }), f("tt", sig(tt_k - off, 6)), f("tc", sig(tc_k - off, 6)), f("pt", pt), f("pc", pc),
            s("melt_slope", if neg { NEG } else { POS }), f("p_line", p_line), i("dec", dec), b(SHOW_ANSWERS, true),
        ];
        let r = rep_of(&list);
        let j = joined(&r);
        // the givens line is the typed values
        let u = if celsius { "\u{b0}C" } else { "K" };
        let gv = |v: f64| -> String { gfmt(sig(v, 6)) };
        assert!(j.contains(&format!("Triple point: T = {} {u}, P = {} atm. Critical point: T = {} {u}, P = {} atm.", gv(tt_k - off), gv(pt), gv(tc_k - off), gv(pc))), "{j}");
        // the readings use only those printed numbers
        let (tt, tc) = (sig(sig(tt_k - off, 6) + off, 9), sig(sig(tc_k - off, 6) + off, 9));
        let rf = Ref { tt, pt, tc, pc, neg };
        let round = |v: f64| -> String {
            let v = v - off;
            let m = 10f64.powi(dec as i32);
            let x = (v.abs() * m * (1.0 + 1e-13)).round() / m;
            let sgn = if v < 0.0 && x != 0.0 { "\u{2212}" } else { "" };
            format!("{sgn}{:.*}", dec as usize, x)
        };
        let normal = p_line == 1.0;
        let pl = gfmt(p_line);
        let (_, steps) = rf_heat(&rf, p_line);
        let _ = pl;
        match steps.len() {
            1 if p_line < pt => {
                let want = round(rf.ts(p_line));
                let head = if normal { "Normal sublimation point = ".to_string() } else { format!("Sublimation point at {} atm = ", gfmt(p_line)) };
                let l = line_starting(&r.readout, &head).unwrap_or_else(|| panic!("{:?}", r.readout));
                assert_eq!(num_after(l, &head), want, "{l} for {list:?}");
            }
            2 => {
                let (m, v) = (round(rf.tm(p_line)), round(rf.tv(p_line)));
                let head = if normal { "Normal melting point = ".to_string() } else { format!("Melting point at {} atm = ", gfmt(p_line)) };
                let l = line_starting(&r.readout, &head).unwrap_or_else(|| panic!("{:?}", r.readout));
                assert_eq!(num_after(l, &head), m, "melting {l} for {list:?}");
                let bh = if normal { "normal boiling point = ".to_string() } else { format!("boiling point at {} atm = ", gfmt(p_line)) };
                assert_eq!(num_after(l, &bh), v, "boiling {l} for {list:?}");
            }
            _ => {
                let head = if normal { "Normal melting point = ".to_string() } else { format!("Melting point at {} atm = ", gfmt(p_line)) };
                let l = line_starting(&r.readout, &head).unwrap_or_else(|| panic!("{:?}", r.readout));
                assert_eq!(num_after(l, &head), round(rf.tm(p_line)), "{l}");
            }
        }
    }
}

/// The heating changes by the reference model: (case, temperatures).
fn rf_heat(rf: &Ref, p: f64) -> (u8, Vec<f64>) {
    if p < rf.pt { (0, vec![rf.ts(p)]) } else if p < rf.pc { (1, vec![rf.tm(p), rf.tv(p)]) } else { (2, vec![rf.tm(p)]) }
}

#[test]
fn the_number_formats_round_once_and_agree_with_their_rules() {
    assert_eq!(gfmt(273.16), "273.16");
    assert_eq!(gfmt(0.00603), "0.00603");
    assert_eq!(gfmt(218.0), "218");
    assert_eq!(gfmt(-56.55), "\u{2212}56.55");
    assert_eq!(gfmt(100000.0), "1 \u{d7} 10\u{2075}");
    assert_eq!(gfmt(0.0005), "5 \u{d7} 10\u{207b}\u{2074}");
    assert_eq!(gfmt(1234567.0), "1.23457 \u{d7} 10\u{2076}");
    assert_eq!(gfmt(0.0), "0");
    assert_eq!(tfmt(373.15, 0), "373");
    assert_eq!(tfmt(373.15, 1), "373.2", "a decimal tie rounds away from zero");
    assert_eq!(tfmt(-78.45, 1), "\u{2212}78.5");
    assert_eq!(tfmt(-0.004, 2), "0.00", "no minus sign on zero");
    assert_eq!(tfmt(0.0, 0), "0");
    assert_eq!(pread(0.0357), "0.036");
    assert_eq!(pread(72.8), "73");
    assert_eq!(pread(1.0), "1");
    assert_eq!(pread(0.000143), "1.4 \u{d7} 10\u{207b}\u{2074}");
    assert_eq!(pread(250000.0), "2.5 \u{d7} 10\u{2075}");
    assert_eq!(pread(9.96), "10");
}

// ---------------------------------------------------------------------------------------------
// Drawing: units, axes, styles
// ---------------------------------------------------------------------------------------------

#[test]
fn the_curves_read_back_through_the_axes() {
    for (name, log) in [(WATER, true), (CO2, true), (TYPICAL, true), (WATER, false), (CO2, false)] {
        for celsius in [false, true] {
            let sb = preset(name).unwrap();
            let list = [s("substance", name), s("paxis", if log { "Logarithmic" } else { LINEAR }), s("unit", if celsius { CELSIUS } else { "Kelvin (K)" })];
            let svg = svg_of(&list);
            let rep = rep_of(&list);
            let fr = rep.frame;
            let off = if celsius { 273.15 } else { 0.0 };
            for (feat, which) in [("vap", 0), ("sub", 1), ("melt", 2)] {
                let g = geo(&svg);
                let pts: Vec<Pt> = g.paths.iter().filter(|p| p.1 == feat).flat_map(|p| p.2.clone()).collect();
                assert!(pts.len() > 5, "{name} {feat}");
                let data = to_data(&pts, &fr, log, off);
                let pok = |a: f64, b: f64| close(a, b, 2e-3) || (!log && (a - b).abs() < 2e-3 * fr.y1);
                for (t, p) in data {
                    let ok = match which {
                        0 => (sb.tt - 0.05..=sb.tc + 0.05).contains(&t) && pok(sb.pvap(t), p),
                        1 => t <= sb.tt + 0.05 && pok(sb.psub(t), p),
                        _ => p >= sb.pt * (1.0 - 1e-3) && close(sb.tm(p), t, 1e-3),
                    };
                    assert!(ok, "{name} {feat} at ({t}, {p}) log={log} celsius={celsius}");
                }
            }
            // the triple and critical points sit on the curves
            let to_px = |t: f64, p: f64| (fr.px(t - off), fr.py(if log { p.log10() } else { p }));
            let (tp, cp) = (to_px(sb.tt, sb.pt), to_px(sb.tc, sb.pc));
            assert!((rep.tp.0 - tp.0).abs() < 1e-6 && (rep.tp.1 - tp.1).abs() < 1e-6, "{name} triple point {:?} vs {tp:?}", rep.tp);
            assert!((rep.cp.0 - cp.0).abs() < 1e-6 && (rep.cp.1 - cp.1).abs() < 1e-6, "{name} critical point");
        }
    }
}

#[test]
fn celsius_redraws_the_same_picture_with_new_numbers() {
    let k = svg_of(&[]);
    let c = svg_of(&[s("unit", CELSIUS)]);
    let curves = |svg: &str| geo(svg).paths.iter().filter(|p| p.0 == "curve").map(|p| format!("{:?}", p.2)).collect::<Vec<_>>();
    // the frames differ in their tick labels only a little, so compare the drawn curves in data space instead
    let (rk, rc) = (rep_of(&[]), rep_of(&[s("unit", CELSIUS)]));
    let dk = to_data(&geo(&k).paths.iter().find(|p| p.1 == "vap").unwrap().2, &rk.frame, true, 0.0);
    let dc = to_data(&geo(&c).paths.iter().find(|p| p.1 == "vap").unwrap().2, &rc.frame, true, 273.15);
    assert_eq!(dk.len(), dc.len());
    for (a, b) in dk.iter().zip(&dc) { assert!(close(a.0, b.0, 2e-4) && close(a.1, b.1, 2e-4), "{a:?} {b:?}"); }
    assert!(!curves(&k).is_empty());
    let tk = texts(&k);
    let tc = texts(&c);
    assert!(tk.iter().any(|t| t.text == "Temperature (K)") && tc.iter().any(|t| t.text == "Temperature (\u{b0}C)"));
    assert!(tc.iter().any(|t| t.kind == "axis" && t.text == "100"), "a Celsius axis has a tick at 100");
    assert!(tk.iter().any(|t| t.kind == "axis" && t.text == "400") || tk.iter().any(|t| t.kind == "axis" && t.text == "200") || tk.iter().any(|t| t.kind == "axis" && t.text == "300"), "{:?}", tk.iter().filter(|t| t.kind == "axis").map(|t| t.text.clone()).collect::<Vec<_>>());
    // the Celsius axis never runs below absolute zero
    assert!(rc.frame.x0 >= -273.15 - 1e-9);
    assert!(rep_of(&[s("substance", CUSTOM), s("unit", CELSIUS), f("tt", -270.0), f("tc", -200.0), f("pt", 0.01), f("pc", 10.0)]).frame.x0 >= -273.15 - 1e-9);
}

#[test]
fn the_pressure_axis_is_log_or_linear_and_says_atm() {
    let svg = svg_of(&[]);
    let t = texts(&svg);
    assert!(t.iter().any(|x| x.text == "Pressure (atm, log scale)"), "{:?}", t.iter().map(|x| &x.text).collect::<Vec<_>>());
    for tick in ["0.001", "0.01", "0.1", "1", "10", "100", "1000"] { assert!(t.iter().any(|x| x.kind == "axis" && x.text == tick), "no tick {tick}"); }
    let lin = svg_of(&[s("paxis", LINEAR)]);
    let t = texts(&lin);
    assert!(t.iter().any(|x| x.text == "Pressure (atm)"));
    assert!(t.iter().any(|x| x.kind == "axis" && x.text == "0"), "a linear axis starts at 0");
    assert!(!t.iter().any(|x| x.kind == "axis" && x.text == "0.001"));
    let fr = rep_of(&[s("paxis", LINEAR)]).frame;
    assert_eq!(fr.y0, 0.0);
    assert!(fr.y1 >= 218.0 * 1.3, "room above the critical pressure: {}", fr.y1);
    // log axis range: whole decades, around the data
    let fr = rep_of(&[]).frame;
    assert_eq!((fr.y0, fr.y1), (-3.0, 3.0));
    let fr = rep_of(&[s("substance", CO2)]).frame;
    assert_eq!((fr.y0, fr.y1), (-1.0, 3.0));
    // 1 atm and the data are always inside
    for name in [WATER, CO2, TYPICAL] {
        let rep = rep_of(&[s("substance", name)]);
        assert!(rep.isobar.unwrap().1 > rep.frame.top() && rep.isobar.unwrap().1 < rep.frame.bottom());
        assert!(rep.tp.1 < rep.frame.bottom() && rep.cp.1 > rep.frame.top());
    }
}

fn is_grey(h: &str) -> bool {
    let h: String = if h.len() == 4 { h[1..].chars().flat_map(|c| [c, c]).collect() } else { h[1..].to_string() };
    h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()) && { let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap(); c(0) == c(2) && c(2) == c(4) }
}

#[test]
fn print_is_grey_only_and_tells_the_lines_apart_by_their_stroke() {
    for name in [WATER, CO2, TYPICAL] {
        for ans in [true, false] {
            let list = [s("style", "Print"), s("substance", name), b("isotherm", true), i("n_points", 4), b(SHOW_ANSWERS, ans)];
            let svg = svg_of(&list);
            for part in svg.split('"').filter(|x| x.starts_with('#')) { assert!(is_grey(part), "Print draws a hue: {part}"); }
            let g = geo(&svg);
            let st = |feat: &str| g.paths.iter().find(|p| p.1 == feat).map(|p| p.3.clone()).unwrap_or_else(|| panic!("no {feat}"));
            let styles = [st("sub"), st("melt"), st("vap"), st("isobar"), st("isotherm")];
            // grey on grey: the width and the dash must tell them apart, not the shade
            let shape: Vec<String> = styles.iter().map(|x| x.split_once('|').unwrap().1.to_string()).collect();
            for a in 0..shape.len() { for c in a + 1..shape.len() { assert_ne!(shape[a], shape[c], "{styles:?}"); } }
            // none is drawn like the dotted guides or the dashed supercritical lines
            let guide = g.paths.iter().find(|p| p.0 == "guide").map(|p| p.3.split_once('|').unwrap().1.to_string()).unwrap();
            assert!(!shape.contains(&guide), "{guide} {shape:?}");
        }
    }
    // colour does use colour, and the three curves differ in it
    let g = geo(&svg_of(&[]));
    let col = |feat: &str| g.paths.iter().find(|p| p.1 == feat).unwrap().3.split('|').next().unwrap().to_string();
    assert!(col("sub") != col("melt") && col("melt") != col("vap") && col("sub") != col("vap"));
    assert!(!is_grey(&col("vap")));
}

#[test]
fn rendering_is_deterministic() {
    for l in [vec![], vec![s("substance", CO2), s("unit", CELSIUS)], vec![s("label_size", "Large"), i("n_points", 4), s("style", "Print")], vec![b("isotherm", true), b(SHOW_ANSWERS, false)]] {
        assert_eq!(svg_of(&l), svg_of(&l));
    }
}

#[test]
fn student_and_key_draw_the_same_curves_axes_and_labels() {
    for name in [WATER, CO2, TYPICAL] {
        for paxis in ["Logarithmic", LINEAR] {
            let base = [s("substance", name), s("paxis", paxis), b("isotherm", true), i("n_points", 4)];
            let mut kl = base.to_vec();
            kl.push(b(SHOW_ANSWERS, true));
            let mut sl = base.to_vec();
            sl.push(b(SHOW_ANSWERS, false));
            let (key, stu) = (svg_of(&kl), svg_of(&sl));
            let (gk, gs) = (geo(&key), geo(&stu));
            let curve = |g: &Geo| g.paths.iter().filter(|p| p.0 == "curve" || p.0 == "line").map(|p| format!("{:?}", p.2)).collect::<Vec<_>>();
            assert_eq!(curve(&gk).len(), 5, "{name}: three curves and two lines");
            assert_eq!(curve(&gk), curve(&gs), "{name} {paxis}");
            let axis_titles = |svg: &str| texts(svg).into_iter().filter(|t| t.kind == "axis" && t.text.contains('(')).map(|t| t.text).collect::<Vec<_>>();
            assert_eq!(axis_titles(&key), axis_titles(&stu));
            assert_eq!(crate::figures::svg_size(&key).0, crate::figures::svg_size(&stu).0);
            // the labels of the plot that are not answers are where they were
            let lab = |svg: &str, feat: &str| texts(svg).into_iter().filter(|t| t.kind == "label" && t.feat == feat).map(|t| (t.text, t.x.to_bits(), t.y.to_bits())).collect::<Vec<_>>();
            for feat in ["ptA", "ptB", "ptC", "ptD", "tp", "cp", "isobar", "isotherm"] { assert_eq!(lab(&key, feat), lab(&stu, feat), "{name} {feat}"); }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The student copy
// ---------------------------------------------------------------------------------------------

/// Every value or word a student is asked to work out for this render, as the key prints it.
fn answers(rep: &Report, dec: usize) -> Vec<String> {
    let mut v = Vec::new();
    if let Some((_, steps)) = &rep.heat { for c in steps { v.push(tfmt(c.t - rep.off, dec)); } }
    if let Some(a) = &rep.along { for c in a { v.push(pread(c.p)); } }
    v.retain(|x| !x.trim_start_matches('\u{2212}').chars().all(|c| c == '0' || c == '.'));
    v
}

const PHASE_WORDS: [&str; 6] = ["solid", "liquid", "gas", "supercritical", "sublim", "boundary"];

fn student_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases = Vec::new();
    for name in [WATER, CO2, TYPICAL] {
        for celsius in [false, true] {
            for (pl, tl) in [(1.0, 300.0), (5.11, 216.6), (400.0, 700.0), (0.001, 150.0), (30.0, 320.0)] {
                for regs in ["Labelled", BLANKLAB, OFF] {
                    for style in ["Colour", "Print"] {
                        for dec in [0, 1, 2] {
                            for np in [0, 4] {
                                cases.push(vec![
                                    s("substance", name), s("unit", if celsius { CELSIUS } else { "Kelvin (K)" }), f("p_line", pl), b("isotherm", true), f("t_line", if celsius { tl - 273.15 } else { tl }),
                                    s("lab_regions", regs), s("lab_curves", if regs == "Labelled" { BLANKLAB } else { regs }), s("style", style), i("dec", dec), i("n_points", np), b(SHOW_ANSWERS, false),
                                ]);
                            }
                        }
                    }
                }
            }
        }
    }
    cases
}

#[test]
fn the_student_copy_has_no_computed_value_anywhere() {
    let mut n = 0;
    let mut with_answers = 0;
    for c in student_cases() {
        let p = with(&c);
        let (svg, rep) = build(&p);
        let svg = svg::annotate(&svg, def().title, def().blurb);
        let dec = p.i("dec").clamp(0, 2) as usize;
        let ans = answers(&rep, dec);
        // what the page prints as givens: the four values, the lines, the points
        let u = |v: f64| gfmt(sig6(v));
        let mut givens = vec![u(rep.giv.tt), u(rep.giv.pt), u(rep.giv.tc), u(rep.giv.pc), u(p.f("p_line")), u(p.f("t_line"))];
        for k in ["a", "b", "c", "d"] { givens.push(u(p.f(&format!("{k}_t")))); givens.push(u(p.f(&format!("{k}_p")))); }
        let printed: Vec<String> = texts(&svg).into_iter().filter(|t| t.kind != "axis").map(|t| t.text).chain([title_of(&svg), desc_of(&svg)]).collect();
        for line in &printed {
            // the number on a blank ("3 ______") and the numbered answer lines ("3. ______") are labels, not readings
            let head = line.split([' ', '.']).next().unwrap_or("");
            if !head.is_empty() && head.chars().all(|c| c.is_ascii_digit()) && line.contains("______") && line.len() < 80 { continue; }
            for tok in tokens(line) {
                if givens.contains(&tok) { continue; }
                assert!(!ans.contains(&tok), "the student copy prints the answer {tok} in {line:?} ({c:?})");
            }
        }
        // no phase word anywhere a region label is not the one speaking (and never in the notes, title or description)
        let low_notes = notes_text(&svg).to_lowercase();
        for w in PHASE_WORDS { assert!(!low_notes.contains(w), "{w} in the notes of the student copy: {low_notes}"); }
        let low_d = format!("{} {}", title_of(&svg), desc_of(&svg)).to_lowercase();
        for w in ["sublim", "boundary", "supercritical"] { assert!(!low_d.contains(w) || w == "supercritical" && !low_d.contains("fluid"), "{w}: {low_d}"); }
        if p.s("lab_regions") != "Labelled" {
            for t in texts(&svg).iter().filter(|t| t.kind != "axis") {
                let low = t.text.to_lowercase();
                // the curves' names may be on the plot; the regions' names may not
                for w in ["solid", "liquid", "gas", "supercritical"] { assert!(!low.contains(w), "{w} in {:?} with the region labels {}", t.text, p.s("lab_regions")); }
            }
        }
        for t in texts(&svg).iter().filter(|t| t.kind == "label") {
            for w in ["melts", "boils", "sublimes"] { assert_ne!(t.text, w, "a key-only label on the student copy"); }
        }
        // the same case's key does print answers from this list, so the check above has teeth
        let mut kc = c.clone();
        kc.push(b(SHOW_ANSWERS, true));
        let keyp = with(&kc);
        let key = build(&keyp).0;
        let key_tokens: Vec<String> = texts(&key).into_iter().filter(|t| t.kind != "axis").flat_map(|t| tokens(&t.text)).collect();
        if ans.iter().any(|a| key_tokens.contains(a)) { with_answers += 1; }
        // the blanks are there to fill
        let notes = notes_text(&svg);
        assert!(notes.contains("______"), "{notes}");
        n += 1;
    }
    assert!(n >= 1000, "{n}");
    assert!(with_answers * 10 >= n * 8, "only {with_answers} of {n} keys print an answer from the list");
}

#[test]
fn the_student_copy_hides_or_shows_what_the_toggles_say() {
    let stu = |extra: &[(&'static str, Value)]| { let mut l = extra.to_vec(); l.push(b(SHOW_ANSWERS, false)); svg_of(&l) };
    // regions labelled, blank, off
    let t = stu(&[]);
    for (feat, w) in [("reg:solid", "Solid"), ("reg:liquid", "Liquid"), ("reg:gas", "Gas"), ("reg:super", "Supercritical fluid")] {
        let got = labels_of(&t, feat).join(" ");
        assert_eq!(got.replace("Supercritical fluid", "Supercritical fluid"), w.to_string().replace("Supercritical fluid", "Supercritical fluid"), "{feat}: {got}");
    }
    let t = stu(&[s("lab_regions", BLANKLAB)]);
    for (n, feat) in ["reg:solid", "reg:liquid", "reg:gas", "reg:super"].iter().enumerate() { assert_eq!(labels_of(&t, feat), vec![format!("{} ______", n + 1)], "{feat}"); }
    let k = svg_of(&[s("lab_regions", BLANKLAB), b(SHOW_ANSWERS, true)]);
    assert_eq!(labels_of(&k, "reg:solid"), vec!["Solid".to_string()]);
    assert_eq!(labels_of(&k, "reg:liquid"), vec!["Liquid".to_string()]);
    let t = stu(&[s("lab_regions", OFF)]);
    for feat in ["reg:solid", "reg:liquid", "reg:gas", "reg:super"] { assert!(labels_of(&t, feat).is_empty()); }
    assert!(!geo(&t).polys.is_empty(), "the region tints are still drawn");
    // curves and the two special points
    let t = stu(&[s("lab_curves", BLANKLAB), s("lab_special", BLANKLAB)]);
    for (n, feat) in ["sub", "melt", "vap", "tp", "cp"].iter().enumerate() { assert_eq!(labels_of(&t, feat), vec![format!("{} ______", n + 1)], "{feat}"); }
    let k = svg_of(&[s("lab_curves", BLANKLAB), s("lab_special", BLANKLAB), b(SHOW_ANSWERS, true)]);
    assert_eq!(labels_of(&k, "vap"), vec!["Vaporisation".to_string()]);
    assert_eq!(labels_of(&k, "melt"), vec!["Melting".to_string()]);
    assert_eq!(labels_of(&k, "sub"), vec!["Sublimation".to_string()]);
    assert_eq!(labels_of(&k, "tp"), vec!["Triple point".to_string()]);
    assert_eq!(labels_of(&k, "cp"), vec!["Critical point".to_string()]);
    let t = stu(&[s("lab_curves", OFF), s("lab_special", OFF)]);
    for feat in ["sub", "melt", "vap", "tp", "cp"] { assert!(labels_of(&t, feat).is_empty(), "{feat}"); }
    // the key shows the heating path and the readings at the axis; the student copy shows neither
    let k = svg_of(&[b(SHOW_ANSWERS, true)]);
    let t = stu(&[]);
    assert!(geo(&k).paths.iter().any(|p| p.0 == "arrow") && !geo(&t).paths.iter().any(|p| p.0 == "arrow"));
    assert!(texts(&k).iter().any(|x| x.kind == "axis" && x.text == "273") && !texts(&t).iter().any(|x| x.text == "273"));
    assert!(texts(&k).iter().any(|x| x.kind == "axis" && x.text == "373"));
    assert!(texts(&k).iter().any(|x| x.kind == "label" && x.feat.starts_with('x') && ["melts", "boils"].contains(&x.text.as_str())));
    // the crossing markers are drawn on both copies
    assert!(geo(&t).marks.iter().any(|m| m.0 == "xm") && geo(&t).marks.iter().any(|m| m.0 == "xv"));
    // `path` off removes the arrows from the key
    assert!(!geo(&svg_of(&[b("path", false), b(SHOW_ANSWERS, true)])).paths.iter().any(|p| p.0 == "arrow"));
    // the values of the triple and critical points may be left for the students
    let t = stu(&[b("values", false)]);
    let n = notes_text(&t);
    assert!(n.contains("Triple point: T = ______ K, P = ______ atm") && n.contains("Critical point: T = ______ K, P = ______ atm"), "{n}");
    assert!(!n.contains("273.16") && !n.contains("0.00603") && !n.contains("647"));
    assert!(joined(&key(&[b("values", false)])).contains("Triple point: T = 273.16 K"));
    // the isobar and the isotherm only when asked
    assert!(!notes_text(&stu(&[b("isobar", false)])).contains("isobar"));
    assert!(!geo(&stu(&[b("isobar", false)])).paths.iter().any(|p| p.1 == "isobar"));
    assert!(geo(&stu(&[b("isotherm", true)])).paths.iter().any(|p| p.1 == "isotherm"));
    assert!(!geo(&stu(&[])).paths.iter().any(|p| p.1 == "isotherm"));
}

#[test]
fn the_description_is_short_ascii_and_answer_free() {
    for c in student_cases().into_iter().step_by(11) {
        for ans in [true, false] {
            let mut c = c.clone();
            c.push(b(SHOW_ANSWERS, ans));
            let p = with(&c);
            let (svg, rep) = build(&p);
            let d = desc_of(&svg);
            assert!(!d.is_empty() && d.is_ascii() && d.len() <= 300, "{d:?}");
            assert!(!d.chars().any(|c| c.is_ascii_digit()), "a number in the description: {d}");
            let low = d.to_lowercase();
            assert!(!low.contains("sublim") && !low.contains("melts") && !low.contains("boils"), "{d}");
            if !ans { assert!(!low.contains("arrow"), "the heating path is an answer: {d}"); }
            let t = title_of(&svg);
            assert!(t.is_ascii() && !t.chars().any(|c| c.is_ascii_digit()) && !t.is_empty(), "{t:?}");
            // svg_size reads the first 400 bytes, which the description follows
            assert_eq!(crate::figures::svg_size(&svg).1, rep.frame.h.max(crate::figures::svg_size(&svg).1));
        }
    }
    let svg = build(&with(&[s("label_size", "Large"), i("n_points", 4), b("isotherm", true)])).0;
    let (w, h) = crate::figures::svg_size(&svg);
    assert!(w > 700.0 && h > 450.0, "{w} {h}");
}

// ---------------------------------------------------------------------------------------------
// Clamps, refusals and odd input
// ---------------------------------------------------------------------------------------------

/// Notes about where labels went, which a crowded picture may carry, and nothing else (no refusal or clamp).
fn layout_only(n: &[String]) -> bool { n.iter().all(|x| x.contains("could not be placed") || x.contains("too close together")) }
fn notes_of(list: &[(&'static str, Value)]) -> Vec<String> { warnings(&with(list)) }
fn has(notes: &[String], what: &str) -> bool { notes.iter().any(|n| n.contains(what)) }

#[test]
fn clamps_and_refusals_come_with_a_note() {
    // custom values out of order or out of range fall back to the typical substance, with the reason
    let n = notes_of(&[s("substance", CUSTOM), f("tt", 400.0), f("tc", 300.0)]);
    assert!(has(&n, "lower temperature than the critical point") && has(&n, "Typical substance is drawn"), "{n:?}");
    let n = notes_of(&[s("substance", CUSTOM), f("pt", 100.0), f("pc", 10.0)]);
    assert!(has(&n, "lower pressure than the critical point"), "{n:?}");
    let n = notes_of(&[s("substance", CUSTOM), f("pt", 1e-6), f("pc", 1e-6)]);
    assert!(has(&n, "pressure"), "{n:?}");
    let n = notes_of(&[s("substance", CUSTOM), f("tt", 0.0)]);
    assert!(has(&n, "greater than 0 K"), "{n:?}");
    let n = notes_of(&[s("substance", CUSTOM), s("unit", CELSIUS), f("tt", -300.0), f("tc", 50.0)]);
    assert!(has(&n, "absolute zero") || has(&n, "greater than 0 K"), "{n:?}");
    let n = notes_of(&[s("substance", CUSTOM), f("tt", 300.0), f("tc", 305.0)]);
    assert!(has(&n, "too close"), "{n:?}");
    let n = notes_of(&[s("substance", CUSTOM), f("tt", 100.0), f("tc", 9000.0)]);
    assert!(has(&n, "too far apart") || has(&n, "5000"), "{n:?}");
    let n = notes_of(&[s("substance", CUSTOM), f("pt", 1.0), f("pc", 1.5)]);
    assert!(has(&n, "too close"), "{n:?}");
    // a good custom substance has no note
    let n = notes_of(&[s("substance", CUSTOM), f("tt", 190.0), f("tc", 500.0), f("pt", 0.2), f("pc", 60.0)]);
    assert!(n.is_empty(), "{n:?}");
    // melting slope against the substance
    let n = notes_of(&[s("melt_slope", POS)]);
    assert!(has(&n, "Water's melting curve slopes to the left") && has(&n, "only for a custom"), "{n:?}");
    let n = notes_of(&[s("substance", CO2), s("melt_slope", NEG)]);
    assert!(has(&n, "Carbon dioxide's melting curve slopes to the right"), "{n:?}");
    assert!(notes_of(&[s("melt_slope", NEG)]).is_empty() && notes_of(&[s("substance", CO2), s("melt_slope", POS)]).is_empty());
    // the isobar and isotherm
    let n = warnings(&raw(&[f("p_line", 0.0)]));
    assert!(has(&n, "must be greater than 0 atm") && has(&n, "P = 1 atm"), "{n:?}");
    let n = warnings(&raw(&[f("p_line", 1e9)]));
    assert!(has(&n, "larger than this diagram can draw"), "{n:?}");
    let n = notes_of(&[f("p_line", 1.23456789)]);
    assert!(has(&n, "kept to 6 significant figures") && has(&n, "1.23457"), "{n:?}");
    let n = notes_of(&[b("isotherm", true), f("t_line", 0.0)]);
    assert!(has(&n, "above 0 K"), "{n:?}");
    let n = notes_of(&[b("isotherm", true), s("unit", CELSIUS), f("t_line", -300.0)]);
    assert!(has(&n, "absolute zero"), "{n:?}");
    // the line notes only when the line is drawn
    assert!(notes_of(&[b("isotherm", false), f("t_line", 0.0)]).is_empty());
    assert!(notes_of(&[b("isobar", false), f("p_line", 0.0)]).is_empty());
    // points: omitted with the reason
    let n = warnings(&raw(&[i("n_points", 2), f("a_p", 0.0), f("b_t", -5.0)]));
    assert!(has(&n, "Point A is not drawn") && has(&n, "greater than 0 atm") && has(&n, "Point B is not drawn") && has(&n, "above 0 K"), "{n:?}");
    let r = build(&raw(&[i("n_points", 4), f("a_p", 0.0), f("b_t", -5.0)])).1;
    assert_eq!(r.points.iter().map(|p| p.i).collect::<Vec<_>>(), vec![2, 3]);
    // only the points that are on show are checked
    assert!(warnings(&raw(&[i("n_points", 0), f("a_p", 0.0)])).is_empty());
    assert!(has(&warnings(&raw(&[i("n_points", 1), f("a_p", 7.5e5)])), "Point A is not drawn"));
    // pressure of exactly zero on the log axis is the same refusal; the linear axis refuses it too (no pressure to speak of)
    assert!(has(&warnings(&raw(&[i("n_points", 1), f("a_p", 0.0), s("paxis", LINEAR)])), "Point A is not drawn"));
    // a point's value is kept to 6 figures
    assert!(has(&notes_of(&[i("n_points", 1), f("a_t", 250.1234567)]), "kept to 6 significant figures"));
}

#[test]
fn odd_inputs_never_panic_or_print_nan() {
    let mut cases: Vec<Params> = Vec::new();
    let keys = ["tt", "pt", "tc", "pc", "p_line", "t_line", "a_t", "a_p", "b_t", "b_p", "c_t", "c_p", "d_t", "d_p"];
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1e300, 1e300, 0.0, -0.0, 1e-300, 7e-7, 1e12, -273.15, -274.0] {
        for key in keys {
            for substance in [WATER, CUSTOM] {
                for paxis in ["Logarithmic", LINEAR] {
                    cases.push(raw(&[(key, Value::F(v)), s("substance", substance), s("paxis", paxis), s("unit", CELSIUS), b("isotherm", true), i("n_points", 4), b("isobar", true)]));
                    cases.push(raw(&[(key, Value::F(v)), s("substance", substance), s("paxis", paxis), b("isotherm", true), i("n_points", 4), b(SHOW_ANSWERS, false)]));
                }
            }
        }
    }
    cases.push(Params(Default::default()));
    cases.push(raw(&[s("substance", "<<bogus>>"), s("unit", ""), s("paxis", "x"), s("melt_slope", "y"), s("lab_regions", "z"), s("label_size", "Huge"), s("style", "x"), ("tt", Value::S("lots".into())), ("pc", Value::B(true)), ("dec", Value::F(f64::NAN))]));
    cases.push(raw(&[("dec", Value::I(i64::MAX)), ("n_points", Value::I(i64::MIN)), ("tt", Value::I(i64::MAX)), ("p_line", Value::I(-5))]));
    cases.push(raw(&[("n_points", Value::I(99)), ("dec", Value::I(-4))]));
    // extremes that fit the controls
    for (tt, pt, tc, pc) in [(1.0, 1e-6, 5000.0, 1e5), (4000.0, 1e4, 5000.0, 1e5), (10.0, 1e-6, 12.0, 1e-5), (300.0, 1e-3, 315.0, 1e-2), (20.0, 0.1, 400.0, 0.3)] {
        for paxis in ["Logarithmic", LINEAR] {
            for neg in [NEG, POS] {
                cases.push(with(&[s("substance", CUSTOM), f("tt", tt), f("pt", pt), f("tc", tc), f("pc", pc), s("paxis", paxis), s("melt_slope", neg), f("p_line", 1e5), b("isotherm", true), f("t_line", 10000.0), i("n_points", 4), f("a_p", 1e-6), f("b_p", 1e5), f("c_t", 0.001), f("d_t", 9999.0)]));
                cases.push(with(&[s("substance", CUSTOM), f("tt", tt), f("pt", pt), f("tc", tc), f("pc", pc), s("paxis", paxis), s("melt_slope", neg), f("p_line", 1e-6), b("isotherm", true), f("t_line", 1.0), i("n_points", 4)]));
            }
        }
    }
    for p in &cases {
        let (svg, _) = build(p);
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{p:?}");
        let low = svg.to_lowercase();
        assert!(!low.contains("nan") && !low.contains("inf") && !low.contains("e+") && !low.contains('?'), "{p:?} prints a non-finite value");
        to_png(&svg, 0.3).unwrap_or_else(|e| panic!("{p:?}: {e}"));
        let _ = handles(ID, p, crate::figures::svg_size(&svg));
    }
}

#[test]
fn edge_cases_come_out_as_a_note_not_a_panic() {
    // exactly at the triple pressure, the critical pressure, a huge pressure on the linear axis, a tiny one on the log axis
    for pl in [5.11, 72.8, 1e5, 1e-6, 3.0] {
        for paxis in ["Logarithmic", LINEAR] {
            let r = key(&[s("substance", CO2), f("p_line", pl), s("paxis", paxis), b("isotherm", true), f("t_line", 216.6)]);
            let fr = r.frame;
            let y = r.isobar.unwrap().1;
            assert!(y >= fr.top() - 1e-6 && y <= fr.bottom() + 1e-6, "the isobar is inside the plot for {pl}: {y}");
            assert!(!r.readout.is_empty());
        }
    }
    // temperature far above critical
    let r = key(&[s("substance", CO2), b("isotherm", true), f("t_line", 9000.0)]);
    assert!(r.along.unwrap().is_empty() && r.isotherm.is_some());
    // a log axis with a pressure of zero is refused for the isobar and the points
    let r = build(&raw(&[f("p_line", 0.0), i("n_points", 1), f("a_p", 0.0)])).1;
    assert!(r.points.is_empty() && r.isobar.is_some());
    // custom with the triple point above the critical point does not panic and draws the typical substance
    let r = rep_of(&[s("substance", CUSTOM), f("tt", 700.0), f("tc", 300.0), f("pt", 50.0), f("pc", 1.0)]);
    assert_eq!((r.sub.name, r.sub.tt), ("Typical substance", preset(TYPICAL).unwrap().tt));
    assert!(r.notes.len() == 1);
    // a custom substance that has no liquid at 1 atm says so as carbon dioxide does
    let j = joined(&key(&[s("substance", CUSTOM), f("tt", 200.0), f("pt", 2.0), f("tc", 400.0), f("pc", 80.0)]));
    assert!(j.contains("This substance has no normal melting point and no normal boiling point."), "{j}");
    // a custom critical pressure below 1 atm: the isobar at 1 atm is above it
    let j = joined(&key(&[s("substance", CUSTOM), f("tt", 100.0), f("pt", 0.001), f("tc", 300.0), f("pc", 0.5)]));
    assert!(j.contains("above the critical pressure (0.5 atm)"), "{j}");
}

// ---------------------------------------------------------------------------------------------
// Labels: real-glyph collision and attribution audit
// ---------------------------------------------------------------------------------------------

fn sweep_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases: Vec<Vec<(&'static str, Value)>> = Vec::new();
    // the classroom grid: every substance, axis, unit, style, size and answers state, with the extras in 12 ways
    for name in [WATER, CO2, TYPICAL] {
        for paxis in ["Logarithmic", LINEAR] {
            for celsius in [false, true] {
                for style in ["Colour", "Print"] {
                    for size in ["Small", "Medium", "Large"] {
                        for ans in [true, false] {
                            for m in 0..12 {
                                cases.push(vec![
                                    s("substance", name), s("paxis", paxis), s("unit", if celsius { CELSIUS } else { "Kelvin (K)" }), s("style", style), s("label_size", size), b(SHOW_ANSWERS, ans),
                                    b("isobar", m % 4 != 3), f("p_line", [1.0, 6.0, 0.01, 300.0][m % 4]), b("isotherm", m % 3 != 0), f("t_line", if celsius { [26.85, -50.0, 150.0][m % 3] } else { [300.0, 230.0, 420.0][m % 3] }),
                                    i("n_points", [0, 4, 2, 4][m % 4]), b("path", m % 5 != 4), s("lab_regions", ["Labelled", BLANKLAB, "Labelled"][m % 3]), s("lab_curves", ["Labelled", "Labelled", BLANKLAB, OFF][m % 4]),
                                ]);
                            }
                        }
                    }
                }
            }
        }
    }
    // random over the whole space, presets and customs
    let mut r: u64 = 17;
    let mut next = move || { r = r.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (r >> 11) as f64 / (1u64 << 53) as f64 };
    let pick = |o: &[&'static str], x: f64| o[((x * o.len() as f64) as usize).min(o.len() - 1)];
    while cases.len() < 10_500 {
        let celsius = next() < 0.4;
        let off = if celsius { 273.15 } else { 0.0 };
        let custom = next() < 0.5;
        let tt = 20.0 + 700.0 * next();
        let tc = tt * (1.1 + 2.5 * next());
        let pt = 10f64.powf(-5.0 + 6.0 * next());
        let pc = pt * 10f64.powf(0.4 + 5.0 * next());
        let mut c = vec![
            s("substance", if custom { CUSTOM } else { pick(&[WATER, CO2, TYPICAL], next()) }), s("paxis", pick(&["Logarithmic", "Logarithmic", LINEAR], next())), s("unit", if celsius { CELSIUS } else { "Kelvin (K)" }),
            s("style", pick(&["Colour", "Print"], next())), s("label_size", pick(&["Small", "Medium", "Large"], next())), b(SHOW_ANSWERS, next() < 0.5),
            s("melt_slope", pick(&[NEG, POS, "Automatic"], next())), b("isobar", next() < 0.8), f("p_line", 10f64.powf(-4.0 + 8.0 * next())), b("isotherm", next() < 0.6), f("t_line", 20.0 + 800.0 * next() - off),
            i("n_points", (next() * 5.0) as i64), b("path", next() < 0.8), b("grid", next() < 0.8), b("values", next() < 0.8), i("dec", (next() * 3.0) as i64),
            s("lab_regions", pick(&["Labelled", BLANKLAB, OFF, "Labelled"], next())), s("lab_curves", pick(&["Labelled", BLANKLAB, OFF, "Labelled"], next())), s("lab_special", pick(&["Labelled", BLANKLAB, OFF], next())),
        ];
        if custom { c.extend([f("tt", tt - off), f("tc", tc - off), f("pt", pt), f("pc", pc)]); }
        for k in ["a", "b", "c", "d"] {
            let kk: &'static str = match k { "a" => "a_t", "b" => "b_t", "c" => "c_t", _ => "d_t" };
            let pk: &'static str = match k { "a" => "a_p", "b" => "b_p", "c" => "c_p", _ => "d_p" };
            c.push(f(kk, 20.0 + 800.0 * next() - off));
            c.push(f(pk, 10f64.powf(-4.0 + 8.0 * next())));
        }
        cases.push(c);
    }
    cases
}

#[test]
fn labels_clear_and_attributed_across_ten_thousand_cases_in_real_glyphs() {
    let cases = sweep_cases();
    assert!(cases.len() >= 10_000);
    let chunks: Vec<&[Vec<(&'static str, Value)>]> = cases.chunks(cases.len().div_ceil(4)).collect();
    let results: Vec<(usize, usize, Vec<String>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = chunks.iter().map(|ch| sc.spawn(move || {
            let mut g = Glyphs::new();
            let mut bad = Vec::new();
            let mut labels = 0;
            let mut regions = 0;
            for c in ch.iter() {
                let p = with(c);
                let svg = build(&p).0;
                let ts = texts(&svg);
                labels += ts.iter().filter(|t| t.kind == "label").count();
                regions += ts.iter().filter(|t| t.kind == "label" && t.feat.starts_with("reg:")).count();
                let lab = svg::label_factor(p.s("label_size"));
                for x in audit(&svg, &mut g, lab) { bad.push(format!("{x}  in {c:?}")); }
            }
            (labels, regions, bad)
        })).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let labels: usize = results.iter().map(|r| r.0).sum();
    let regions: usize = results.iter().map(|r| r.1).sum();
    let bad: Vec<&String> = results.iter().flat_map(|r| r.2.iter()).collect();
    if let Ok(out) = std::env::var("VERSHION_AUDIT_OUT") { std::fs::write(out, bad.iter().map(|x| x.as_str()).collect::<Vec<_>>().join("\n")).unwrap(); }
    assert!(labels > 80_000, "only {labels} labels placed");
    assert!(regions > 20_000, "only {regions} region labels placed");
    // The strict standard is none. A label that cannot sit beside its feature, or a leader line that cannot avoid every
    // label, in the crowded corners of the random space (extreme customs on a linear axis) is tolerated to one label
    // in two thousand, so that a regression in the placer still fails this test.
    assert!(bad.len() * 2000 <= labels, "{} problems in {labels} labels, first: {:#?}", bad.len(), &bad[..bad.len().min(12)]);
}

/// How often a label had to be left out for want of room: every default has all of its labels, and the cases that
/// drop one say so.
#[test]
fn the_defaults_label_everything_and_a_dropped_label_is_reported() {
    for name in [WATER, CO2, TYPICAL] {
        for size in ["Small", "Medium", "Large"] {
            for ans in [true, false] {
                for paxis in ["Logarithmic", LINEAR] {
                    let p = with(&[s("substance", name), s("label_size", size), b(SHOW_ANSWERS, ans), s("paxis", paxis), b("isotherm", true), i("n_points", 4)]);
                    let (svg, rep) = build(&p);
                    for feat in ["reg:solid", "reg:liquid", "reg:gas", "reg:super", "sub", "melt", "vap", "tp", "cp", "isobar", "isotherm", "ptA", "ptB", "ptC", "ptD"] {
                        assert!(!labels_of(&svg, feat).is_empty() || has(&rep.notes, "could not be placed"), "{name} {size} {ans} {paxis}: no {feat} label and no note: {:?}", rep.notes);
                    }
                }
            }
        }
    }
    // the plain defaults need no note at all
    for name in [WATER, CO2, TYPICAL] {
        for size in ["Small", "Medium", "Large"] {
            let n = rep_of(&[s("substance", name), s("label_size", size)]).notes;
            assert!(n.is_empty(), "{name} {size} {n:?}");
        }
    }
}

/// The audit is not blind: it sees a label put on a curve, one put away from its feature, one in the wrong region,
/// and text off the canvas.
#[test]
fn the_audit_catches_what_it_should() {
    let mut g = Glyphs::new();
    let svg = svg_of(&[]);
    assert!(audit(&svg, &mut g, 1.0).is_empty(), "{:?}", audit(&svg, &mut g, 1.0));
    let ts = texts(&svg);
    let group: Vec<T> = ts.iter().filter(|t| t.kind == "label" && t.feat == "vap").cloned().collect();
    let lab = group[0].clone();
    let gm = geo(&svg);
    let vap: Vec<Pt> = gm.paths.iter().find(|p| p.1 == "vap").unwrap().2.clone();
    let on = vap[vap.len() / 3];
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
    assert!(audit(&moved(110.0, 330.0), &mut g, 1.0).iter().any(|x| x.contains("from its vap feature") || x.contains("stroke")), "a label far off");
    assert!(audit(&moved(-30.0, 40.0), &mut g, 1.0).iter().any(|x| x.contains("leaves")), "a label off the canvas");
    // a region label moved into another region
    let reg: Vec<T> = ts.iter().filter(|t| t.kind == "label" && t.feat == "reg:gas").cloned().collect();
    let solid: Vec<T> = ts.iter().filter(|t| t.kind == "label" && t.feat == "reg:solid").cloned().collect();
    let mut out = svg.clone();
    for t in &reg {
        let from = format!(r#"x="{:.2}" y="{:.2}""#, t.x, t.y);
        let to = format!(r#"x="{:.2}" y="{:.2}""#, solid[0].x, solid[0].y);
        out = out.replace(&from, &to);
    }
    assert!(audit(&out, &mut g, 1.0).iter().any(|x| x.contains("not wholly inside") || x.contains(" on ")), "a label in the wrong region: {:?}", audit(&out, &mut g, 1.0));
}

#[test]
fn large_labels_grow_the_canvas_and_nothing_is_clipped() {
    let mut g = Glyphs::new();
    let mut last = 0.0;
    for size in ["Small", "Medium", "Large"] {
        let svg = svg_of(&[s("label_size", size), i("n_points", 4), b("isotherm", true)]);
        let (w, h) = crate::figures::svg_size(&svg);
        assert!(w * h > last, "{size}");
        last = w * h;
        assert!(audit(&svg, &mut g, svg::label_factor(size)).is_empty(), "{size}: {:?}", audit(&svg, &mut g, svg::label_factor(size)));
    }
}

#[test]
fn region_labels_lie_inside_their_regions_in_the_default_pictures() {
    for name in [WATER, CO2, TYPICAL] {
        for paxis in ["Logarithmic", LINEAR] {
            let svg = svg_of(&[s("substance", name), s("paxis", paxis)]);
            let g = geo(&svg);
            assert_eq!(g.polys.len(), 4, "{name}");
            let mut gl = Glyphs::new();
            for t in texts(&svg).iter().filter(|t| t.kind == "label" && t.feat.starts_with("reg:")) {
                let poly = &g.polys.iter().find(|p| p.0 == t.feat).unwrap().1;
                let bx = gl.ink(t).unwrap();
                assert!(inside(poly, ((bx.0 + bx.1) / 2.0, (bx.2 + bx.3) / 2.0)), "{name} {paxis} {}", t.feat);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Handles, examples, PNGs
// ---------------------------------------------------------------------------------------------

#[test]
fn handles_drag_the_triple_and_critical_points_the_lines_and_the_points() {
    let base = with(&[b("isotherm", true), i("n_points", 4)]);
    let rep = build(&base).1;
    let fr = rep.frame;
    let hs = handles(ID, &base, (fr.w, fr.h)).unwrap();
    let vy = |y: f64, log: bool| { let v = fr.y0 + (fr.bottom() - y) / (fr.bottom() - fr.top()) * (fr.y1 - fr.y0); if log { 10f64.powf(v) } else { v } };
    let vx = |x: f64| fr.x0 + (x - fr.left()) / (fr.right() - fr.left()) * (fr.x1 - fr.x0);
    let tp = hs.iter().find(|h| h.tip.contains("triple")).expect("a triple point handle");
    assert!((tp.pos.0 - rep.tp.0).abs() < 1e-9 && (tp.pos.1 - rep.tp.1).abs() < 1e-9);
    // dragging it switches to Custom and carries the other three values along
    let mut q = base.clone();
    (tp.drag.clone().unwrap())(&mut q, (fr.px(250.0), fr.py(-1.0)));
    assert_eq!(q.s("substance"), CUSTOM);
    assert!((q.f("tt") - 250.0).abs() < 1e-9 && (q.f("pt") - 0.1).abs() < 1e-9, "{} {}", q.f("tt"), q.f("pt"));
    assert!(close(q.f("tc"), 647.0, 1e-9) && close(q.f("pc"), 218.0, 1e-9), "the critical point is kept: {} {}", q.f("tc"), q.f("pc"));
    assert_eq!(q.s("melt_slope"), NEG, "water's slope is kept as a custom setting");
    assert!(warnings(&q).iter().all(|n| n.contains("could not be placed")), "{:?}", warnings(&q));
    let cp = hs.iter().find(|h| h.tip.contains("critical")).expect("a critical point handle");
    let mut q = base.clone();
    (cp.drag.clone().unwrap())(&mut q, (fr.px(600.0), fr.py(2.0)));
    assert_eq!(q.s("substance"), CUSTOM);
    assert!((q.f("tc") - 600.0).abs() < 1e-9 && (q.f("pc") - 100.0).abs() < 1e-9 && close(q.f("tt"), 273.16, 1e-9), "{} {}", q.f("tc"), q.f("pc"));
    // a drag that would put the points out of order is held to a drawable one
    for target in [(fr.px(100.0), fr.py(2.0)), (fr.px(1000.0), fr.py(-2.0)), (fr.px(700.0), fr.py(-3.0))] {
        let mut q = base.clone();
        (tp.drag.clone().unwrap())(&mut q, target);
        assert!(layout_only(&warnings(&q)), "{target:?}: {:?} {:?}", warnings(&q), q);
        let mut q = base.clone();
        (cp.drag.clone().unwrap())(&mut q, target);
        assert!(layout_only(&warnings(&q)), "{target:?}: {:?}", warnings(&q));
    }
    // the isobar's pressure
    let ib = hs.iter().find(|h| h.tip.contains("isobar")).expect("an isobar handle");
    let mut q = base.clone();
    (ib.drag.clone().unwrap())(&mut q, (ib.pos.0, fr.py(2.0)));
    assert!((q.f("p_line") - 100.0).abs() < 1e-9, "{}", q.f("p_line"));
    assert_eq!(q.s("substance"), WATER);
    let it = hs.iter().find(|h| h.tip.contains("isotherm")).expect("an isotherm handle");
    let mut q = base.clone();
    (it.drag.clone().unwrap())(&mut q, (fr.px(450.0), it.pos.1));
    assert!((q.f("t_line") - 450.0).abs() < 1e-9, "{}", q.f("t_line"));
    // points A to D
    for (k, (name, (t, p))) in [("A", (250.0, 100.0)), ("B", (300.0, 20.0)), ("C", (500.0, 0.1)), ("D", (700.0, 400.0))].iter().enumerate() {
        let h = hs.iter().find(|h| h.tip.contains(&format!("point {name}"))).unwrap_or_else(|| panic!("no handle for {name}"));
        assert!((h.pos.0 - fr.px(*t)).abs() < 1e-9 && (h.pos.1 - fr.py((*p as f64).log10())).abs() < 1e-9);
        let mut q = base.clone();
        (h.drag.clone().unwrap())(&mut q, (fr.px(400.0), fr.py(0.0)));
        let (kt, kp) = (format!("{}_t", name.to_lowercase()), format!("{}_p", name.to_lowercase()));
        assert!((q.f(&kt) - 400.0).abs() < 1e-9 && (q.f(&kp) - 1.0).abs() < 1e-9, "{k} {} {}", q.f(&kt), q.f(&kp));
    }
    // a drag that does not move leaves everything alone, preset included
    for h in &hs {
        let mut q = base.clone();
        (h.drag.clone().unwrap())(&mut q, h.pos);
        assert_eq!(format!("{q:?}"), format!("{base:?}"), "{}", h.tip);
    }
    let _ = (vx(0.0), vy(0.0, true));
    // linear axis and Celsius invert the drawing too
    for (paxis, celsius) in [(LINEAR, false), ("Logarithmic", true), (LINEAR, true)] {
        let p = with(&[s("paxis", paxis), s("unit", if celsius { CELSIUS } else { "Kelvin (K)" }), i("n_points", 1)]);
        let rep = build(&p).1;
        let fr = rep.frame;
        let hs = handles(ID, &p, (fr.w, fr.h)).unwrap();
        let h = hs.iter().find(|h| h.tip.contains("point A")).unwrap();
        let mut q = p.clone();
        let tgt_t = if celsius { 20.0 } else { 300.0 };
        let tgt_p = if paxis == LINEAR { 120.0 } else { 3.0 };
        (h.drag.clone().unwrap())(&mut q, (fr.px(tgt_t), fr.py(if paxis == LINEAR { tgt_p } else { tgt_p.log10() })));
        assert!((q.f("a_t") - tgt_t).abs() < 1e-9 && (q.f("a_p") - tgt_p).abs() < 1e-9, "{paxis} {celsius}: {} {}", q.f("a_t"), q.f("a_p"));
    }
    // handles go through the subject dispatch
    assert!(!crate::figures::interact::handles_for(ID, &base, (fr.w, fr.h)).is_empty());
    // wild drags keep every value in range
    for h in &hs {
        for target in [(0.0, 0.0), (1e9, -1e9), (-5000.0, 9000.0)] {
            let mut q = base.clone();
            (h.drag.clone().unwrap())(&mut q, target);
            q.normalize(&(def().params)(), true);
            assert!((def().render)(&q).contains("<svg"));
        }
    }
}

#[test]
fn phase_diagram_pngs() {
    let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
    let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
    for (tag, name) in [("water", WATER), ("co2", CO2), ("typical", TYPICAL)] {
        cases.push((format!("{tag}-log"), vec![s("substance", name), b("isotherm", true), f("t_line", if tag == "water" { 450.0 } else if tag == "co2" { 260.0 } else { 300.0 }), i("n_points", 4)]));
        cases.push((format!("{tag}-linear"), vec![s("substance", name), s("paxis", LINEAR), b("isotherm", true), f("t_line", if tag == "water" { 450.0 } else if tag == "co2" { 260.0 } else { 300.0 }), i("n_points", 4)]));
        cases.push((format!("{tag}-celsius"), vec![s("substance", name), s("unit", CELSIUS)]));
    }
    cases.push(("below-triple".into(), vec![s("substance", CO2), f("p_line", 2.0)]));
    cases.push(("below-triple-water".into(), vec![f("p_line", 0.002)]));
    cases.push(("above-critical".into(), vec![f("p_line", 400.0), b("isotherm", true), f("t_line", 700.0)]));
    cases.push(("at-triple".into(), vec![s("substance", CO2), f("p_line", 5.11)]));
    cases.push(("at-critical".into(), vec![f("p_line", 218.0)]));
    cases.push(("custom-neg".into(), vec![s("substance", CUSTOM), f("tt", 250.0), f("pt", 0.5), f("tc", 520.0), f("pc", 90.0), s("melt_slope", NEG), i("n_points", 4), f("a_t", 260.0), f("a_p", 30.0), f("b_t", 250.0), f("b_p", 5.0), f("c_t", 400.0), f("c_p", 2.0), f("d_t", 600.0), f("d_p", 90.0)]));
    cases.push(("marks".into(), vec![i("n_points", 4), b("isotherm", true), f("t_line", 350.0), f("p_line", 10.0)]));
    for (i_, e) in examples(ID).unwrap().into_iter().enumerate() { cases.push((format!("ex{i_}"), e.params)); }
    for (tag, list) in cases {
        for size in ["Small", "Medium", "Large"] {
            for (kind, extra) in [("key", vec![b(SHOW_ANSWERS, true)]), ("student", vec![b(SHOW_ANSWERS, false)]), ("print", vec![s("style", "Print")])] {
                if size != "Medium" && !(tag == "water-log" || tag == "marks" || tag == "co2-log") { continue; }
                let mut l = list.clone();
                l.extend(extra);
                l.push(s("label_size", size));
                let svg = svg_of(&l);
                std::fs::write(format!("{dir}/pd-{tag}-{size}-{kind}.png"), to_png(&svg, 1.5).unwrap()).unwrap();
            }
        }
    }
}


#[test]
fn the_real_melting_curves_follow_published_data() {
    let w = preset(WATER).unwrap();
    // water: IAPWS melting pressure of ice Ih (Wagner et al. 2011), up to 209.9 MPa
    for (p, t, tol) in [(1.0, 273.15, 0.001), (10.0, 273.14, 0.1), (100.0, 272.39, 0.3), (218.0, 271.5, 0.5), (500.0, 268.9, 1.0), (2072.0, 251.17, 0.3)] {
        assert!((w.tm(p) - t).abs() <= tol, "water melts at {} K at {p} atm, not {t}", w.tm(p));
    }
    // the slope near 1 atm is the textbook -0.0075 K per atm
    assert!(((w.tm(1.0) - w.tm(101.0)) / 100.0 - 0.0075).abs() < 0.0005);
    // and the inverse agrees with it
    for p in [1.0, 50.0, 218.0, 1000.0, 2000.0] { assert!(close(w.pm(w.tm(p)).unwrap(), p, 1e-6), "{p}"); }
    // carbon dioxide: Span-Wagner, p/pt = 1 + 1955.539 ((T/Tt)^1.83 - 1)
    let c = preset(CO2).unwrap();
    for p in [5.11f64, 20.0, 100.0, 1000.0, 2000.0, 7900.0] {
        let want = 216.6 * (1.0 + (p / 5.11 - 1.0) / 1955.539).powf(1.0 / 1.83);
        assert!((c.tm(p) - want).abs() < 0.01, "co2 {p}: {} vs {want}", c.tm(p));
    }
    assert!((c.tm(1000.0) - 228.2).abs() < 1.5, "{}", c.tm(1000.0));
    // what the page prints comes from the same curve
    for (pl, want) in [(218.0, 271.5), (500.0, 268.9)] {
        let r = key(&[f("p_line", pl)]);
        let l = line_starting(&r.readout, "Melting point").unwrap_or_else(|| panic!("{:?}", r.readout));
        let v = l.split('=').nth(1).unwrap().trim().trim_end_matches(" K").split_whitespace().next().unwrap().parse::<f64>().unwrap();
        assert!((v - want).abs() < 1.0, "{l}");
    }
}

#[test]
fn the_isotherm_at_the_triple_or_critical_temperature_says_so() {
    let r = key(&[b("isotherm", true), f("t_line", 273.16)]);
    let l = line_starting(&r.readout, "Raising the pressure along the isotherm at 273.16 K").unwrap();
    assert!(l.contains("gas \u{2192} liquid") && l.contains("triple point"), "{l}");
    let r = key(&[s("substance", CO2), b("isotherm", true), f("t_line", 216.6)]);
    let l = line_starting(&r.readout, "Raising the pressure along the isotherm at 216.6 K").unwrap();
    assert!(l.contains("gas \u{2192} solid") && l.contains("triple point"), "{l}");
    let r = key(&[s("substance", CO2), b("isotherm", true), f("t_line", 304.2)]);
    let l = line_starting(&r.readout, "Raising the pressure along the isotherm at 304.2 K").unwrap();
    assert!(l.contains("This is the critical temperature") && !l.contains("Above the critical temperature"), "{l}");
}


// ---------------------------------------------------------------------------------------------
// Essential labels
// ---------------------------------------------------------------------------------------------

/// The labels a diagram must always carry: each region's name when the regions are labelled, and the triple and
/// critical points' when the special points are. Returns those that are missing from the render.
fn missing_essentials(p: &Params) -> Vec<&'static str> {
    let svg = build(p).0;
    let mut want: Vec<&'static str> = Vec::new();
    if p.s("lab_regions") != OFF { want.extend(["reg:solid", "reg:liquid", "reg:gas", "reg:super"]); }
    if p.s("lab_special") != OFF { want.extend(["tp", "cp"]); }
    want.into_iter().filter(|f| labels_of(&svg, f).is_empty()).collect()
}

fn dropped_notes(p: &Params) -> Vec<String> { build(p).1.notes.into_iter().filter(|n| n.contains("could not be placed")).collect() }

/// The settings a class would use: the classroom grid, then random presets with lines and points anywhere a teacher
/// would put them (pressures from 0.001 to 1000 atm, temperatures from 150 to 800 K) and customs shaped like real
/// substances (a critical pressure 10 to 100,000 times the triple pressure).
fn classroom_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases: Vec<Vec<(&'static str, Value)>> = sweep_cases().into_iter().take(1728).collect();
    let mut r: u64 = 4242;
    let mut next = move || { r = r.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (r >> 11) as f64 / (1u64 << 53) as f64 };
    let pick = |o: &[&'static str], x: f64| o[((x * o.len() as f64) as usize).min(o.len() - 1)];
    for n in 0..2100 {
        let custom = n % 7 == 6;
        let celsius = next() < 0.4;
        let off = if celsius { 273.15 } else { 0.0 };
        let tt = 100.0 + 400.0 * next();
        let tc = tt * (1.4 + 1.1 * next());
        let pt = 10f64.powf(-3.0 + 4.0 * next());
        let pc = pt * 10f64.powf(1.0 + 4.0 * next());
        let mut c = vec![
            s("substance", if custom { CUSTOM } else { pick(&[WATER, CO2, TYPICAL], next()) }), s("paxis", pick(&["Logarithmic", "Logarithmic", LINEAR], next())), s("unit", if celsius { CELSIUS } else { "Kelvin (K)" }),
            s("style", pick(&["Colour", "Print"], next())), s("label_size", pick(&["Small", "Medium", "Large"], next())), b(SHOW_ANSWERS, next() < 0.5),
            s("melt_slope", pick(&[NEG, POS, "Automatic"], next())), b("isobar", next() < 0.8), f("p_line", 10f64.powf(-3.0 + 6.0 * next())), b("isotherm", next() < 0.6), f("t_line", 150.0 + 650.0 * next() - off),
            i("n_points", (next() * 5.0) as i64), b("path", next() < 0.8),
            s("lab_regions", pick(&["Labelled", BLANKLAB, "Labelled"], next())), s("lab_curves", pick(&["Labelled", BLANKLAB, OFF, "Labelled"], next())), s("lab_special", pick(&["Labelled", BLANKLAB, "Labelled"], next())),
        ];
        if custom { c.extend([f("tt", tt - off), f("tc", tc - off), f("pt", pt), f("pc", pc)]); }
        for (kk, pk) in [("a_t", "a_p"), ("b_t", "b_p"), ("c_t", "c_p"), ("d_t", "d_p")] {
            c.push(f(kk, 150.0 + 650.0 * next() - off));
            c.push(f(pk, 10f64.powf(-3.0 + 6.0 * next())));
        }
        cases.push(c);
    }
    cases
}

#[test]
fn the_region_and_special_point_labels_survive_the_sweep() {
    let cases = classroom_cases();
    assert!(cases.len() >= 2000);
    let chunks: Vec<&[Vec<(&'static str, Value)>]> = cases.chunks(cases.len().div_ceil(4)).collect();
    let results: Vec<Vec<String>> = std::thread::scope(|sc| {
        let hs: Vec<_> = chunks.iter().map(|ch| sc.spawn(move || ch.iter().filter_map(|c| { let m = missing_essentials(&with(c)); (!m.is_empty()).then(|| format!("{m:?} in {c:?}")) }).collect::<Vec<_>>())).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let bad: Vec<&String> = results.iter().flatten().collect();
    if let Ok(out) = std::env::var("VERSHION_ESSENTIAL_OUT") { std::fs::write(out, bad.iter().map(|x| x.as_str()).collect::<Vec<_>>().join("\n")).unwrap(); }
    let share = 1.0 - bad.len() as f64 / cases.len() as f64;
    assert!(share >= 0.99, "{} of {} cases lose an essential label ({:.2}% placed), first: {:#?}", bad.len(), cases.len(), share * 100.0, &bad[..bad.len().min(8)]);
}

#[test]
fn the_examples_and_the_reported_cases_lose_no_label() {
    let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
    for e in crate::figures::examples_for(ID) { cases.push((e.name.to_string(), e.params.clone())); }
    cases.push(("water p 500".into(), vec![f("p_line", 500.0)]));
    cases.push(("water isotherm 800".into(), vec![b("isotherm", true), f("t_line", 800.0)]));
    cases.push(("co2 print large isotherm 250".into(), vec![s("substance", CO2), s("style", "Print"), s("label_size", "Large"), b("isotherm", true), f("t_line", 250.0), i("n_points", 4)]));
    cases.push(("water linear".into(), vec![s("paxis", LINEAR)]));
    cases.push(("water celsius isotherm 4 points".into(), vec![s("unit", CELSIUS), b("isotherm", true), f("t_line", 26.85), i("n_points", 4)]));
    for (name, c) in &cases {
        for ans in [true, false] {
            let mut l = c.clone();
            l.push(b(SHOW_ANSWERS, ans));
            let p = with(&l);
            assert!(missing_essentials(&p).is_empty(), "{name} (answers {ans}): {:?}", missing_essentials(&p));
            assert!(dropped_notes(&p).is_empty(), "{name} (answers {ans}): {:?}", dropped_notes(&p));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Numbered blanks
// ---------------------------------------------------------------------------------------------

/// The numbers written on the blanks of a student copy's plot, by feature.
fn plot_numbers(svg: &str) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for t in texts(svg).into_iter().filter(|t| t.kind == "label") {
        if let Some(n) = t.text.strip_suffix(" ______").and_then(|n| n.parse::<usize>().ok()) { out.insert(t.feat.clone(), n); }
    }
    out
}

fn numbered_lines(notes: &str, key: bool) -> Vec<usize> {
    let mut v = Vec::new();
    for l in notes.lines() {
        let head = l.split_once(". ").map(|x| x.0).unwrap_or("");
        if let Ok(n) = head.parse::<usize>() { if (key && !l.contains("______")) || (!key && l.contains("______")) { v.push(n); } }
    }
    v
}

#[test]
fn every_blank_on_a_student_copy_is_numbered_and_listed_in_the_working() {
    let all = [s("lab_regions", BLANKLAB), s("lab_curves", BLANKLAB), s("lab_special", BLANKLAB)];
    let mut l = all.to_vec();
    l.push(b(SHOW_ANSWERS, false));
    let (svg, rep) = build(&with(&l));
    let nums = plot_numbers(&svg);
    // nine blanks, numbered 1 to 9 once each, and no name of a phase or a feature anywhere
    let mut got: Vec<usize> = nums.values().copied().collect();
    got.sort();
    assert_eq!(got, (1..=9).collect::<Vec<_>>(), "{nums:?}");
    let low = texts(&svg).into_iter().filter(|t| t.kind == "label").map(|t| t.text).collect::<Vec<_>>().join("\n").to_lowercase();
    for name in ["solid", "liquid", "triple point", "critical point", "sublimation", "melting", "vaporisation", "supercritical"] { assert!(!low.contains(name), "{name} is printed on the student copy"); }
    // the working has an answer line for each number
    let work = rep.readout.join("\n");
    assert_eq!(numbered_lines(&work, false), (1..=9).collect::<Vec<_>>(), "{work}");
    for n in 1..=9 { assert!(work.contains(&format!("{n}. ______")), "{n}"); }
    // the key prints the names against the same numbers, in one place
    let mut k = all.to_vec();
    k.push(b(SHOW_ANSWERS, true));
    let kr = build(&with(&k)).1;
    let kw = kr.readout.join("\n");
    assert!(kw.contains("1. Solid; 2. Liquid; 3. Gas; 4. Supercritical fluid; 5. Sublimation; 6. Melting; 7. Vaporisation; 8. Triple point; 9. Critical point"), "{kw}");
    // only the groups set to Blank are numbered, in the same order
    let (svg2, rep2) = build(&with(&[s("lab_curves", BLANKLAB), b(SHOW_ANSWERS, false)]));
    let n2 = plot_numbers(&svg2);
    assert_eq!(n2.get("sub"), Some(&1));
    assert_eq!(n2.get("vap"), Some(&3));
    assert!(rep2.readout.join("\n").contains("3. ______"));
    // no blanks, no list
    assert!(!rep_of(&[b(SHOW_ANSWERS, false)]).readout.join("\n").contains("Name each numbered label"));
}

/// Over a sweep, the student copy's list has exactly one line for each label the key has, and a label that could
/// not be put on the plot says so in its line instead of vanishing.
#[test]
fn a_dropped_blank_still_has_its_numbered_line() {
    let cases: Vec<_> = sweep_cases().into_iter().step_by(7).take(1200).collect();
    for c in cases {
        let mut st = c.clone();
        st.push(b(SHOW_ANSWERS, false));
        let mut ke = c.clone();
        ke.push(b(SHOW_ANSWERS, true));
        let (ps, pk) = (with(&st), with(&ke));
        let (svg, rs) = build(&ps);
        let rk = build(&pk).1;
        let work_s = rs.readout.join("\n");
        let work_k = rk.readout.join("\n");
        let listed_s = numbered_lines(&work_s, false);
        let listed_k = {
            let l = work_k.lines().find(|l| l.starts_with("Numbered labels on the student copy:"));
            l.map_or(0, |l| l.matches(". ").count())
        };
        assert_eq!(listed_s.len(), listed_k, "{c:?}\n{work_s}\n{work_k}");
        let on_plot = plot_numbers(&svg);
        for n in on_plot.values() { assert!(listed_s.contains(n), "{c:?}"); }
        let missing = listed_s.len() - on_plot.len();
        assert_eq!(work_s.matches("could not be marked on the diagram").count(), missing, "{c:?}\n{work_s}");
    }
}


// ---------------------------------------------------------------------------------------------
// Fix round 2
// ---------------------------------------------------------------------------------------------

const IH_NOTE: &str = "outside the range this diagram models (ice Ih, up to 2070 atm)";

#[test]
fn water_is_modelled_only_as_far_as_ice_ih() {
    let w = preset(WATER).unwrap();
    // inside the range: the readout is there and none of it is a refusal
    for p in [1.0, 218.0, 500.0, 2000.0] {
        let r = key(&[s("substance", WATER), f("p_line", p)]);
        assert!(r.isobar.is_some() && r.heat.is_some(), "{p}");
        assert!(!r.notes.iter().any(|n| n.contains(IH_NOTE)), "{p}: {:?}", r.notes);
        let tm = w.tm(p);
        assert!((250.9..=273.2).contains(&tm), "{p}: {tm}");
    }
    // beyond it: no isobar, no reading, a note
    for p in [2100.0, 3000.0, 10000.0] {
        let r = key(&[s("substance", WATER), f("p_line", p)]);
        assert!(r.isobar.is_none() && r.heat.is_none(), "{p}");
        assert!(r.notes.iter().any(|n| n.contains(IH_NOTE)), "{p}: {:?}", r.notes);
        let j = joined(&r);
        assert!(!j.contains("solid \u{2192} liquid") && !j.contains("\u{2212}36") && !j.contains("\u{2212}123"), "{p}: {j}");
    }
    // an isotherm below the end of the ice Ih curve has no melting to report
    for t in [250.0, 200.0] {
        let r = key(&[s("substance", WATER), b("isotherm", true), f("t_line", t)]);
        assert!(r.isotherm.is_none() && r.along.is_none(), "{t}");
        assert!(r.notes.iter().any(|n| n.contains(IH_NOTE)), "{t}: {:?}", r.notes);
        assert!(!joined(&r).contains("isotherm"), "{t}: {}", joined(&r));
    }
    let r = key(&[s("substance", WATER), b("isotherm", true), f("t_line", 270.0)]);
    assert!(r.isotherm.is_some() && r.along.as_ref().is_some_and(|a| a.iter().any(|c| c.curve == Curve::Melt)), "270 K");
    // a point beyond the range is not classified
    let r = key(&[s("substance", WATER), i("n_points", 1), f("a_t", 300.0), f("a_p", 3000.0)]);
    assert!(r.points.is_empty() && r.notes.iter().any(|n| n.contains(IH_NOTE)), "{:?}", r.notes);
    // the axis stops at the end of the range, on both axes, in both units; no curve is drawn beyond it
    for paxis in ["Logarithmic", LINEAR] {
        for unit in ["Kelvin (K)", CELSIUS] {
            for pl in [218.0, 500.0, 2000.0] {
                let r = key(&[s("substance", WATER), s("paxis", paxis), s("unit", unit), f("p_line", pl), i("n_points", 2), f("a_p", 2000.0), f("b_p", 400.0)]);
                let top = if paxis == "Logarithmic" { 10f64.powf(r.frame.y1) } else { r.frame.y1 };
                assert!(top <= 2070.0 * (1.0 + 1e-9) && top >= 2000.0, "{paxis} {unit} {pl}: {top}");
                assert!(r.isobar.is_some());
            }
        }
    }
    // the caption no longer claims the curve is vertical at every pressure drawn for water
    let j = joined(&key(&[s("substance", WATER)]));
    assert!(!j.contains("at every pressure drawn"), "{j}");
    // neighbours: custom and typical are not limited
    for name in [TYPICAL, CUSTOM] {
        let r = key(&[s("substance", name), f("p_line", 3000.0), b("isotherm", true), f("t_line", 150.0)]);
        assert!(r.isobar.is_some() && r.isotherm.is_some(), "{name}");
        assert!(!r.notes.iter().any(|n| n.contains("ice Ih")), "{name}");
    }
}

#[test]
fn carbon_dioxide_says_where_its_melting_fit_stops_being_measured() {
    let r = key(&[s("substance", CO2), f("p_line", 10000.0)]);
    assert!(r.isobar.is_some());
    assert!(r.notes.iter().any(|n| n.contains("7900 atm")), "{:?}", r.notes);
    let r = key(&[s("substance", CO2), f("p_line", 5000.0)]);
    assert!(!r.notes.iter().any(|n| n.contains("7900 atm")), "{:?}", r.notes);
}

#[test]
fn the_public_student_copy_does_not_name_what_the_blanks_hide() {
    let hidden = ["reg:solid", "reg:liquid", "reg:gas", "reg:super", "\"melt\"", "\"sub\"", "\"vap\"", "\"tp\"", "\"cp\"", "\"ptA\"", "\"xm\"", "\"xv\"", "\"xs\"", "\"ym\"", "\"yv\"", "\"ys\""];
    for group in ["lab_regions", "lab_curves", "lab_special"] {
        let p = with(&[s(group, BLANKLAB), b(SHOW_ANSWERS, false), i("n_points", 4), b("isotherm", true), f("t_line", 250.0), s("substance", CO2)]);
        let svg = def().svg(&p);
        for h in hidden {
            assert!(!svg.contains(&format!("data-f={h}")) && !svg.contains(&format!("data-f=\"{}", h.trim_matches('"'))), "{group}: data-f names {h}");
        }
        let desc = svg.split("<desc").nth(1).unwrap_or("").split("</desc>").next().unwrap_or("").to_lowercase();
        for w in ["solid", "liquid", "gas", "triple", "critical", "curves between"] { assert!(!desc.contains(w), "{group}: desc names {w}: {desc}"); }
        // the numbers are still on the plot, and the opaque ids still tie each label's text to one feature
        assert!(texts(&svg).iter().any(|t| t.text.ends_with("______")));
    }
    // the key and a student copy with nothing blank are unchanged
    let key_svg = def().svg(&with(&[s("lab_regions", BLANKLAB), b(SHOW_ANSWERS, true)]));
    assert!(key_svg.contains("data-f=\"reg:solid\""));
    let open = def().svg(&with(&[b(SHOW_ANSWERS, false)]));
    assert!(open.contains("data-f=\"reg:solid\""));
}

#[test]
fn no_leader_ends_on_another_features_marker() {
    // the reported case: an isotherm on the triple temperature, an isobar on the triple pressure, everything blank
    let case = vec![
        s("substance", CO2), s("paxis", LINEAR), s("unit", CELSIUS), s("label_size", "Large"), s("style", "Print"), b(SHOW_ANSWERS, false),
        s("lab_regions", BLANKLAB), s("lab_curves", BLANKLAB), s("lab_special", BLANKLAB), b("isotherm", true), f("t_line", -56.55), f("p_line", 5.11), i("n_points", 4),
    ];
    let p = with(&case);
    let svg = build(&p).0;
    let g = geo(&svg);
    let leaders: Vec<_> = g.strokes.iter().filter(|x| x.0 == "leader").collect();
    for (_, feat, (a, c)) in &leaders {
        for (mf, (cx, cy, r)) in &g.marks {
            if mf == feat || g.marks.iter().any(|(f2, (x2, y2, _))| f2 == feat && (x2 - cx).hypot(y2 - cy) < 18.0) { continue; }
            // the end of the leader on the feature (the other end is at the label)
            assert!((a.0 - cx).hypot(a.1 - cy) > r + 3.0, "the leader of {feat} ends on the {mf} marker: {a:?}-{c:?}");
        }
    }
    let mut gl = Glyphs::new();
    let bad = audit(&svg, &mut gl, svg::label_factor("Large"));
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn pressures_read_from_the_diagram_have_two_significant_figures() {
    assert_eq!(pread(10423.0), "10000");
    assert_eq!(pread(3971.0), "4000");
    assert_eq!(pread(123.4), "120");
    assert_eq!(pread(99.6), "100");
    assert_eq!(pread(24.4), "24");
    let j = joined(&key(&[s("substance", CO2), f("p_line", 10000.0), b("isotherm", true), f("t_line", 320.0)]));
    assert!(!j.contains("10423"), "{j}");
    let svg = svg_of(&[s("substance", CO2), f("p_line", 10000.0), b("isotherm", true), f("t_line", 320.0), b(SHOW_ANSWERS, true)]);
    assert!(!svg.contains(">10423<"), "an axis annotation keeps a third figure");
}

#[test]
fn the_vapour_pressure_of_water_follows_the_steam_tables() {
    let w = preset(WATER).unwrap();
    for (c, want) in [(100.0, 1.013), (200.0, 15.5), (300.0, 84.8)] {
        let got = w.pvap(c + 273.15);
        assert!(close(got, want, 0.015), "{c} C: {got} vs {want}");
    }
    assert!(close(w.pvap(w.tt), w.pt, 1e-9) && close(w.pvap(w.tc), w.pc, 1e-9));
    assert!(close(w.tvap(1.0), 373.15, 1e-6), "{}", w.tvap(1.0));
    let mut last = 0.0;
    for k in 0..=200 { let t = w.tt + (w.tc - w.tt) * k as f64 / 200.0; let p = w.pvap(t); assert!(p > last, "{t}"); last = p; assert!(close(w.tvap(p), t, 1e-7), "{t}"); }
}
