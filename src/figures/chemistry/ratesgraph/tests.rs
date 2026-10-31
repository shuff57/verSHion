//! Tests of the reaction rate graph. The label audit measures every text with the glyphs the PNG renderer draws
//! (usvg's own layout of the bundled and system faces), not with the placer's width estimate.
use super::*;
use crate::figures::{registry, to_png};
use std::collections::{BTreeMap, HashMap};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("reaction-rate-graph is registered") }
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

const CONC: &str = "Concentration vs time";
const LIN: &str = "Linearised (integrated rate law)";
const RATE: &str = "Rate vs concentration";
const CURVE_GIVEN: &str = "Curve given";

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

/// The vertices of a `M x,y L x,y …` path.
fn path_pts(d: &str) -> Vec<Pt> {
    d.split_whitespace()
        .filter_map(|tok| {
            let t = tok.trim_start_matches(|c: char| c.is_ascii_alphabetic());
            let (a, b) = t.split_once(',')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .collect()
}

/// The strokes (kind, feature, segment) and markers (feature, (cx, cy, r)) drawn.
struct Geo { strokes: Vec<(String, String, Seg)>, marks: Vec<(String, (f64, f64, f64))>, paths: Vec<(String, String, Vec<Pt>, String)> }

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
        } else if tag.starts_with("circle ") {
            g.marks.push((feat, (attr_f(tag, "cx").unwrap(), attr_f(tag, "cy").unwrap(), attr_f(tag, "r").unwrap())));
        }
    }
    g
}

fn path_of(svg: &str, feat: &str) -> Vec<Pt> {
    geo(svg).paths.into_iter().find(|p| p.1 == feat).map(|p| p.2).unwrap_or_else(|| panic!("no {feat} path"))
}

/// The pixel points of a path as data values through the report's frame.
fn to_data(pts: &[Pt], fr: &Frame) -> Vec<Pt> {
    pts.iter()
        .map(|(x, y)| (fr.x0 + (x - fr.left()) / (fr.right() - fr.left()) * (fr.x1 - fr.x0), fr.y0 + (fr.bottom() - y) / (fr.bottom() - fr.top()) * (fr.y1 - fr.y0)))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Real glyphs
// ---------------------------------------------------------------------------------------------

/// Ink boxes of texts as the PNG renderer lays them out (usvg with the figures' font database), memoised by text,
/// size and anchor.
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
    /// The canvas ink box of a drawn text (a rotated title mapped through its rotate(-90)); None for blank text.
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
/// a solid stroke or a marker, and a label that does not read as its own feature's (more than 40 px × size from it,
/// or more than 15 px from it while 6 px nearer another labelled feature). The dotted guides and the grid are thin
/// lines a haloed label may cross.
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
    let owned: Vec<String> = { let mut v: Vec<String> = groups.values().map(|g| g.0.clone()).collect(); v.sort(); v.dedup(); v };
    let dist = |feat: &str, bx: Bx| -> f64 {
        let mut d = f64::MAX;
        for (_, f, (p, q)) in &gm.strokes { if f == feat { d = d.min(seg_box(*p, *q, bx)); } }
        for (f, (cx, cy, _)) in &gm.marks { if f == feat { d = d.min(pt_box((*cx, *cy), bx)); } }
        d
    };
    for (own, bx) in groups.values() {
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

/// A printed number back to a value: "−6.07 × 10⁻²" is −0.0607.
fn parse_num(s: &str) -> f64 {
    let s = s.trim().replace('\u{2212}', "-");
    if let Some((m, e)) = s.split_once(" × 10") {
        let e: String = e.chars().map(|c| match c { '⁻' => '-', '⁰' => '0', '¹' => '1', '²' => '2', '³' => '3', '⁴' => '4', '⁵' => '5', '⁶' => '6', '⁷' => '7', '⁸' => '8', '⁹' => '9', c => c }).collect();
        return m.trim().parse::<f64>().unwrap() * 10f64.powi(e.trim().parse::<i32>().unwrap());
    }
    s.parse().unwrap_or_else(|_| panic!("not a number: {s:?}"))
}

/// A printed number to an exact decimal (mantissa, power of ten): "1.235 × 10⁻⁷" is (1235, −10).
fn exact(s: &str) -> (i128, i32) {
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
    let mant: i128 = format!("{ip}{fp}").parse().unwrap();
    (if neg { -mant } else { mant }, e - fp.len() as i32)
}

/// num/den rounded half away from zero to `dec` places in the figure's rule: fixed notation from 0.1 up to 100000,
/// "m × 10ⁿ" with `dec` decimals in the mantissa outside it. Exact integer arithmetic throughout.
fn round_rational(num: i128, den: i128, dec: usize) -> String {
    assert!(den > 0);
    let neg = num < 0;
    let n = num.abs();
    let sign = if neg { "\u{2212}" } else { "" };
    let p10 = |e: u32| 10i128.pow(e);
    let round_div = |a: i128, b: i128| (2 * a + b) / (2 * b);
    // fixed: 0.1 <= n/den < 1e5
    if n == 0 { return format!("{:.*}", dec, 0.0); }
    if 10 * n >= den && n < 100_000 * den {
        let r = round_div(n * p10(dec as u32), den);
        let ip = r / p10(dec as u32);
        let fp = r % p10(dec as u32);
        return format!("{sign}{ip}.{:0w$}", fp, w = dec);
    }
    // the power of ten e with 1 <= n/den/10^e < 10
    let mut e: i32 = 0;
    let (mut a, mut c) = (n, den);
    while a >= 10 * c { c *= 10; e += 1; }
    while a < c { a *= 10; e -= 1; }
    let mut r = round_div(a * p10(dec as u32), c);
    if r >= 10 * p10(dec as u32) { r /= 10; e += 1; }
    let ip = r / p10(dec as u32);
    let fp = r % p10(dec as u32);
    let sup: String = e.to_string().chars().map(|ch| match ch { '-' => '⁻', '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴', '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹', c => c }).collect();
    format!("{sign}{ip}.{:0w$} × 10{sup}", fp, w = dec)
}

/// (num, den) of an exact decimal.
fn rat((m, e): (i128, i32)) -> (i128, i128) { if e >= 0 { (m * 10i128.pow(e as u32), 1) } else { (m, 10i128.pow((-e) as u32)) } }

/// The last "= value unit" of a working line: the printed result.
fn result_of(line: &str) -> String {
    let tail = line.rsplit(" = ").next().unwrap();
    num_after(&format!("= {tail}"), "= ")
}

/// The printed number right after `pat`: "0.1", or "1.235 × 10⁻⁷".
fn num_after(s: &str, pat: &str) -> String {
    let rest = s.split(pat).nth(1).unwrap_or_else(|| panic!("no {pat:?} in {s:?}"));
    let w: Vec<&str> = rest.split(' ').collect();
    let first = w[0].trim_end_matches(|c: char| c == ';' || c == ',' || c == ':' || c == ')');
    if w.len() >= 3 && w[1] == "×" && w[2].starts_with("10") { format!("{first} × {}", w[2].trim_end_matches(|c: char| c == ';' || c == ',' || c == ':' || c == ')')) } else { first.to_string() }
}

/// A value written at an axis (in units of 10^exp) agrees with the readout's value: they differ by no more than the
/// rounding of the two (half a unit in the last place of each), and with no power of ten they are the same string.
fn agrees(label: &str, exp: i32, readout: &str, what: &str) {
    if exp == 0 { assert_eq!(label, readout, "{what}"); return; }
    let ulp = |s: &str| -> f64 {
        let (m, e) = match s.split_once(" × 10") { Some((m, e)) => (m, exact(&format!("1 × 10{e}")).1), None => (s, 0) };
        let dec = m.split_once('.').map_or(0, |x| x.1.len()) as i32;
        10f64.powi(e - dec)
    };
    let (a, b) = (parse_num(label) * 10f64.powi(exp), parse_num(readout));
    let tol = 0.5 * ulp(label) * 10f64.powi(exp) + 0.5 * ulp(readout);
    assert!((a - b).abs() <= tol * (1.0 + 1e-9), "{what}: {label} (x 10^{exp}) vs {readout}");
}

fn line_starting<'a>(lines: &'a [String], head: &str) -> Option<&'a String> { lines.iter().find(|l| l.starts_with(head)) }

// ---------------------------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------------------------

#[test]
fn it_is_registered_with_a_faq_examples_and_its_controls() {
    let d = def();
    assert_eq!(d.subject, Subject::Chemistry);
    assert!((4..=6).contains(&crate::figures::faq::faq_for(ID).len()));
    let names: Vec<&str> = crate::figures::examples_for(ID).iter().map(|e| e.name).collect();
    for want in ["First-order", "Second order", "Zero order", "Find the order"] {
        assert!(names.iter().any(|n| n.contains(want)), "no {want} example in {names:?}");
    }
    let keys: Vec<&str> = (d.params)().iter().map(|p| p.key).collect();
    for k in ["mode", "order", "given", "k", "a0", "tmax", "half_lives", "tangent", "t_tan", "initial", "product", "a_rate", "dec", SHOW_ANSWERS, "style", "label_size"] {
        assert!(keys.contains(&k), "no {k} param");
    }
    assert!(crate::figures::chemistry::skin_intro(ID).is_some());
    // the editor notes reach the panel
    assert!(!crate::figures::notes_for(ID, &with(&[f("k", 0.0)])).is_empty());
    // and every example renders with no note
    for e in crate::figures::examples_for(ID) {
        let p = with(&e.params);
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
    }
}

// ---------------------------------------------------------------------------------------------
// The chemistry
// ---------------------------------------------------------------------------------------------

#[test]
fn half_life_slope_and_rate_follow_the_rate_laws_over_a_grid() {
    let ln2 = std::f64::consts::LN_2;
    for k in [1e-4, 0.0013, 0.1, 0.25, 3.0, 47.0] {
        for a0 in [0.002, 0.1, 0.5, 1.0, 2.5] {
            let z = Kin { order: Order::Zero, k, a0 };
            let o = Kin { order: Order::First, k, a0 };
            let q = Kin { order: Order::Second, k, a0 };
            let close = |x: f64, y: f64| (x - y).abs() <= 1e-12 * y.abs().max(1e-300);
            // zero order: t½ = [A]0/(2k), and each half-life is half the one before
            assert!(close(z.half_life(), a0 / (2.0 * k)));
            assert!(close(z.t_frac(2) - z.t_frac(1), a0 / (4.0 * k)));
            assert!(close(z.t_end().unwrap(), a0 / k));
            assert_eq!(z.conc(a0 / k * 1.5), 0.0, "zero order stays at 0 after the end");
            assert!(close(z.rate(0.0), k) && z.rate(a0 / k * 1.01) == 0.0);
            assert!(close(z.slope(), -k) && close(z.intercept(), a0));
            // first order: t½ = ln 2/k, the same every time
            assert!(close(o.half_life(), ln2 / k));
            assert!(close(o.t_frac(2) - o.t_frac(1), o.t_frac(1)) || (o.t_frac(2) - 2.0 * o.t_frac(1)).abs() < 1e-12 * o.t_frac(2));
            assert!(close(o.conc(ln2 / k), a0 / 2.0));
            assert!(close(o.rate(3.0 / k), k * a0 * (-3.0f64).exp()));
            assert!(close(o.slope(), -k) && close(o.intercept(), a0.ln()));
            assert!(o.t_end().is_none());
            // second order: t½ = 1/(k[A]0), each twice the one before
            assert!(close(q.half_life(), 1.0 / (k * a0)));
            assert!(close(q.t_frac(2) - q.t_frac(1), 2.0 / (k * a0)));
            assert!(close(q.conc(1.0 / (k * a0)), a0 / 2.0));
            assert!(close(q.rate(2.0 / (k * a0)), k * (a0 / 3.0) * (a0 / 3.0)));
            assert!(close(q.slope(), k) && close(q.intercept(), 1.0 / a0));
            assert!(q.conc(1e300) > 0.0 || q.conc(1e300) == 0.0 && q.t_end().is_none());
            // rate = k[A]^n against [A]
            for a in [0.01, 0.3, a0] {
                assert!(close(z.rate_of(a), k) && close(o.rate_of(a), k * a) && close(q.rate_of(a), k * a * a));
            }
            assert_eq!(z.rate_of(0.0), 0.0);
        }
    }
}

/// The printed half-lives, rates, slopes and intercepts of the key, recomputed in exact arithmetic (zero and second
/// order are rational in the printed givens) or to 1e-12 (first order) from the givens printed on the same page.
#[test]
fn the_key_prints_the_working_from_the_printed_givens() {
    let mut checked = 0;
    for order in ORDERS {
        for (k, a0, tt) in [(0.1, 1.0, 5.0), (0.0235, 0.75, 12.5), (2.0, 0.125, 0.35), (0.004, 2.0, 100.0), (37.5, 0.0123, 0.001), (0.3333, 0.6667, 1.0)] {
            for dec in 1..=4 {
                for mode in [CONC, LIN, RATE] {
                    let p = with(&[s("mode", mode), s("order", order), f("k", k), f("a0", a0), b("tangent", true), f("t_tan", tt), b("initial", true), i("dec", dec), f("a_rate", a0 / 3.0)]);
                    let (svg, rep) = build(&p);
                    let lines = &rep.readout;
                    // what is printed under the graph is these lines, wrapped
                    let joined = lines.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
                    assert_eq!(notes_text(&svg).split_whitespace().collect::<Vec<_>>().join(" ").replace("  ", " ").contains(&joined), true, "{lines:?}");
                    let law = line_starting(lines, "Rate law:").unwrap_or_else(|| panic!("no rate law line in {lines:?}"));
                    let ks = num_after(law, "k = ");
                    let as_ = num_after(law, "[A]₀ = ");
                    let (kn, kd) = rat(exact(&ks));
                    let (an, ad) = rat(exact(&as_));
                    let (kv, av) = (parse_num(&ks), parse_num(&as_));
                    let d = dec as usize;
                    let ord = rep.kin.order;
                    assert_eq!((kv, av), (rep.kin.k, rep.kin.a0), "the printed givens are the drawn ones");
                    if mode == CONC {
                        let t = line_starting(lines, "t½ = ").unwrap_or_else(|| panic!("{lines:?}"));
                        let want = match ord {
                            Order::Zero => round_rational(an * kd, 2 * ad * kn, d),
                            Order::Second => round_rational(kd * ad, kn * an, d),
                            Order::First => res_fmt(std::f64::consts::LN_2 / kv, d),
                        };
                        assert_eq!(result_of(t.split(". ").next().unwrap()), want, "{t}");
                        // the in-plot half-life label says the same
                        if let Some(l) = labels_of(&svg, "hl1v").first() { agrees(l, rep.x_exp, &want, "the half-life written at the axis"); }
                        if ord == Order::Zero {
                            let e = line_starting(lines, "[A] reaches 0 at t = ").unwrap();
                            assert_eq!(result_of(e.split(':').next().unwrap()), round_rational(an * kd, ad * kn, d), "{e}");
                        }
                        // the instantaneous rate at the tangent, from the givens
                        if let Some(r) = line_starting(lines, "Rate at t = ") {
                            let tts = r.trim_start_matches("Rate at t = ").split(" s").next().unwrap().to_string();
                            let tv = parse_num(&tts);
                            let want = match ord {
                                Order::Zero => if tv * kv >= av { format!("{:.*}", d, 0.0) } else { res_fmt(kv, d) },
                                Order::First => res_fmt(kv * av * (-kv * tv).exp(), d),
                                Order::Second => { let c = av / (1.0 + kv * av * tv); res_fmt(kv * c * c, d) }
                            };
                            assert_eq!(result_of(r), want, "{r}");
                            // the legend gives the tangent's slope: minus the rate, to the same digits
                            let leg = notes_text(&svg);
                            if parse_num(&want) != 0.0 { assert!(leg.contains(&format!("slope = {} M/s", res_fmt(-parse_num(&want), d))), "{leg} vs rate {want}"); }
                            checked += 1;
                        }
                        if let Some(r) = line_starting(lines, "Initial rate = ") {
                            if ord != Order::Zero { assert!(notes_text(&svg).contains(&format!("Initial-rate tangent: slope = \u{2212}{} M/s", result_of(r))), "{r}"); }
                            let want = match ord {
                                Order::Zero => res_fmt(kv, d),
                                Order::First => round_rational(kn * an, kd * ad, d),
                                Order::Second => round_rational(kn * an * an, kd * ad * ad, d),
                            };
                            assert_eq!(result_of(r), want, "{r}");
                        }
                        // a later half-life line: zero order 3[A]0/(4k), first 2 ln2/k, second 3/(k[A]0)
                        if let Some(r) = lines.iter().find(|l| l.contains("[A]₀/4 at")) {
                            let seg = r.split("[A]₀/4 at").nth(1).unwrap();
                            let seg = seg.split(" s").next().unwrap().to_string() + " s";
                            let want = match ord {
                                Order::Zero => round_rational(3 * an * kd, 4 * ad * kn, d),
                                Order::First => res_fmt(2.0 * std::f64::consts::LN_2 / kv, d),
                                Order::Second => round_rational(3 * kd * ad, kn * an, d),
                            };
                            assert_eq!(result_of(seg.trim_end_matches(" s")), want, "{r}");
                        }
                    }
                    if mode == LIN {
                        let sl = line_starting(lines, "Slope = ").unwrap();
                        let want = match ord { Order::Second => given_fmt(kv), _ => format!("\u{2212}{}", given_fmt(kv)) };
                        assert_eq!(result_of(sl), want, "{sl}");
                        let ic = line_starting(lines, "Intercept = ").unwrap();
                        let want = match ord { Order::Zero => given_fmt(av), Order::First => res_fmt(av.ln(), d), Order::Second => round_rational(ad, an, d) };
                        assert_eq!(result_of(ic), want, "{ic}");
                        assert!(labels_of(&svg, "tri").join(" ").contains(&format!("slope = {}", result_of(sl))));
                        agrees(&labels_of(&svg, "icpt")[0], rep.y_exp, &want, "the intercept at the axis");
                    }
                    if mode == RATE {
                        let r = line_starting(lines, "At [A] = ").unwrap();
                        let ars = r.trim_start_matches("At [A] = ").split(" M").next().unwrap();
                        let (rn, rd) = rat(exact(ars));
                        let want = match ord {
                            Order::Zero => res_fmt(kv, d),
                            Order::First => round_rational(kn * rn, kd * rd, d),
                            Order::Second => round_rational(kn * rn * rn, kd * rd * rd, d),
                        };
                        assert_eq!(result_of(r), want, "{r}");
                        agrees(&labels_of(&svg, "rgh")[0], rep.y_exp, &want, "the rate at the axis");
                        agrees(&labels_of(&svg, "rg")[0], rep.x_exp, &given_fmt(rep.a_rate), "the marked [A] at the axis");
                    }
                }
            }
        }
    }
    assert!(checked >= 72, "{checked}");
}

/// Only the first-order half-life is constant; the key never says otherwise for zero or second order.
#[test]
fn the_half_life_is_called_constant_only_for_first_order() {
    for order in ORDERS {
        let work = build(&with(&[s("order", order)])).1.readout.join(" ");
        if order == "First" {
            assert!(work.contains("is constant"), "{work}");
        } else {
            assert!(work.contains("not constant") && !work.contains("is constant"), "{work}");
        }
    }
    let faq: String = crate::figures::faq::faq_for(ID).iter().map(|q| q.a).collect::<Vec<_>>().join(" ");
    assert!(faq.contains("1/(k[A]₀)") && faq.contains("[A]₀/(2k)") && faq.contains("ln 2/k"), "{faq}");
}

#[test]
fn units_follow_the_order() {
    for (order, ku, lin_y) in [("Zero", "M/s", "[A] (M)"), ("First", "1/s", "ln [A]"), ("Second", "1/(M s)", "1/[A] (1/M)")] {
        let key = build(&with(&[s("order", order)])).1.readout.join(" ");
        assert!(key.contains(&format!("k = 0.1 {ku}")), "{key}");
        let svg = svg_of(&[s("order", order), s("mode", LIN)]);
        let axis: Vec<String> = texts(&svg).into_iter().filter(|t| t.kind == "axis").map(|t| t.text).collect();
        assert!(axis.contains(&lin_y.to_string()), "{axis:?}");
        assert!(axis.contains(&"Time (s)".to_string()), "{axis:?}");
        let svg = svg_of(&[s("order", order)]);
        let axis: Vec<String> = texts(&svg).into_iter().filter(|t| t.kind == "axis").map(|t| t.text).collect();
        assert!(axis.contains(&"[A] (M)".to_string()), "{axis:?}");
        let svg = svg_of(&[s("order", order), s("mode", RATE)]);
        let axis: Vec<String> = texts(&svg).into_iter().filter(|t| t.kind == "axis").map(|t| t.text).collect();
        assert!(axis.contains(&"Rate (M/s)".to_string()) && axis.contains(&"[A] (M)".to_string()), "{axis:?}");
    }
    // a product curve makes the axis a concentration of either
    let svg = svg_of(&[b("product", true)]);
    assert!(all_text(&svg).contains("Concentration (M)"));
}

// ---------------------------------------------------------------------------------------------
// The drawing
// ---------------------------------------------------------------------------------------------

/// Read the drawn [A] curve back to data through the frame: it starts at [A]0, passes through [A]0/2ⁿ at each marked
/// half-life (exact vertices), a zero-order curve meets 0 at [A]0/k and stays there, a second-order one never does.
#[test]
fn the_curve_reads_back_through_the_stated_values() {
    for order in ORDERS {
        for (k, a0) in [(0.1, 1.0), (0.023, 0.5), (2.5, 0.08), (1e-5, 2.0), (4000.0, 0.001), (0.5, 3.0)] {
            for tmax in [0.0, 1e9] {
                let p = with(&[s("order", order), f("k", k), f("a0", a0), f("tmax", tmax)]);
                let (svg, rep) = build(&p);
                let fr = rep.frame;
                let px_y = (fr.y1 - fr.y0) / (fr.bottom() - fr.top());
                let px_x = (fr.x1 - fr.x0) / (fr.right() - fr.left());
                let tol_y = 0.0011 * px_y;
                let pts = to_data(&path_of(&svg, "curveA"), &fr);
                let kin = rep.kin;
                assert!(pts[0].0.abs() < 0.0011 * px_x && (pts[0].1 - kin.a0).abs() < tol_y, "{order}: starts at {:?}", pts[0]);
                assert!((pts.last().unwrap().0 - fr.x1).abs() < 0.0011 * px_x, "the curve runs to the end of the axis");
                for w in pts.windows(2) { assert!(w[1].0 > w[0].0 && w[1].1 <= w[0].1 + tol_y, "{order}: not falling at {w:?}"); }
                for q in &pts {
                    assert!((q.1 - kin.conc(q.0)).abs() < tol_y + (kin.conc((q.0 - 0.0011 * px_x).max(0.0)) - kin.conc(q.0 + 0.0011 * px_x)).abs(), "{order}: off the curve at {q:?}");
                    assert!(q.1 > -tol_y, "{order}: below 0 at {q:?}");
                }
                assert!(!rep.halves.is_empty(), "{order} k={k}: no half-life marked");
                for (n, (t, c)) in rep.halves.iter().enumerate() {
                    let want = kin.a0 / 2f64.powi(n as i32 + 1);
                    assert!((c - want).abs() < 1e-12 * want && (t - kin.t_frac(n as u32 + 1)).abs() < 1e-12 * t, "{order}: half {n}");
                    assert!(pts.iter().any(|q| (q.0 - t).abs() < 0.0011 * px_x && (q.1 - want).abs() < tol_y), "{order}: no vertex at the half-life {n} ({t}, {want})");
                }
                match kin.order {
                    Order::Zero => {
                        let te = kin.t_end().unwrap();
                        if te <= fr.x1 {
                            assert!(pts.iter().any(|q| (q.0 - te).abs() < 0.0011 * px_x && q.1.abs() < tol_y), "no corner at the end");
                            for q in pts.iter().filter(|q| q.0 > te + 0.002 * px_x) { assert!(q.1.abs() < tol_y, "zero order rises off 0 at {q:?}"); }
                            assert!(geo(&svg).marks.iter().any(|m| m.0 == "end"), "the end of the reaction is not marked");
                        }
                    }
                    Order::Second => assert!(pts.last().unwrap().1 > 10.0 * tol_y, "second order reached 0"),
                    Order::First => {}
                }
                // and the first half-life marker's guide ends on the curve
                let g = geo(&svg);
                let h1 = g.strokes.iter().find(|x| x.1 == "hl1h").expect("the [A]0/2 guide");
                let end = to_data(&[h1.2 .1], &fr)[0];
                assert!((end.1 - kin.a0 / 2.0).abs() < tol_y && (end.0 - kin.half_life()).abs() < 0.0011 * px_x, "{end:?}");
            }
        }
    }
}

#[test]
fn the_linearised_plot_is_a_straight_line_for_its_order() {
    for order in ORDERS {
        for (k, a0) in [(0.1, 1.0), (0.023, 0.5), (2.5, 0.08), (40.0, 2.0)] {
            let (svg, rep) = build(&with(&[s("mode", LIN), s("order", order), f("k", k), f("a0", a0)]));
            let kin = rep.kin;
            let fr = rep.frame;
            let mut pts = to_data(&path_of(&svg, "line"), &fr);
            if let Some(te) = kin.t_end() { pts.retain(|q| q.0 < te * 0.999); }
            let n = pts.len() as f64;
            let (mx, my) = (pts.iter().map(|q| q.0).sum::<f64>() / n, pts.iter().map(|q| q.1).sum::<f64>() / n);
            let sxy: f64 = pts.iter().map(|q| (q.0 - mx) * (q.1 - my)).sum();
            let sxx: f64 = pts.iter().map(|q| (q.0 - mx) * (q.0 - mx)).sum();
            let slope = sxy / sxx;
            let icpt = my - slope * mx;
            let px_y = (fr.y1 - fr.y0) / (fr.bottom() - fr.top());
            let worst = pts.iter().map(|q| (q.1 - (icpt + slope * q.0)).abs()).fold(0.0, f64::max);
            assert!(worst < 0.003 * px_y, "{order}: not straight, {worst} off ({} px)", worst / px_y);
            assert!((slope - kin.slope()).abs() < 2e-5 * kin.slope().abs() + 0.003 * px_y / (fr.x1 - fr.x0), "{order}: slope {slope} want {}", kin.slope());
            assert!((icpt - kin.intercept()).abs() < 0.003 * px_y, "{order}: intercept {icpt} want {}", kin.intercept());
            // the y axis says which plot it is
            let want = match kin.order { Order::Zero => "[A] (M)", Order::First => "ln [A]", Order::Second => "1/[A] (1/M)" };
            assert!(texts(&svg).iter().any(|t| t.kind == "axis" && t.text == want), "{order}");
        }
    }
    // the concentration-time curve of a first-order reaction is not a straight line (the test above can fail)
    let (svg, rep) = build(&with(&[]));
    let pts = to_data(&path_of(&svg, "curveA"), &rep.frame);
    let (a, z) = (pts[0], *pts.last().unwrap());
    let mid = pts[pts.len() / 2];
    let chord = a.1 + (z.1 - a.1) * (mid.0 - a.0) / (z.0 - a.0);
    assert!((mid.1 - chord).abs() > 0.1 * rep.kin.a0);
}

#[test]
fn the_rate_plot_follows_the_rate_law() {
    for order in ORDERS {
        let (svg, rep) = build(&with(&[s("mode", RATE), s("order", order), f("k", 0.4), f("a0", 2.0)]));
        let kin = rep.kin;
        let fr = rep.frame;
        let px_y = (fr.y1 - fr.y0) / (fr.bottom() - fr.top());
        let pts = to_data(&path_of(&svg, "rcurve"), &fr);
        for q in pts.iter().filter(|q| q.0 > 1e-9) { assert!((q.1 - kin.rate_of(q.0)).abs() < 0.0011 * px_y, "{order}: {q:?}"); }
        assert!((pts.last().unwrap().0 - fr.x1).abs() < 1e-6 * fr.x1);
        let pt = geo(&svg).marks.into_iter().find(|m| m.0 == "rpt").expect("the marked rate");
        let at = to_data(&[(pt.1 .0, pt.1 .1)], &fr)[0];
        assert!((at.0 - rep.a_rate).abs() < 0.0011 * (fr.x1 - fr.x0) / (fr.right() - fr.left()) && (at.1 - kin.rate_of(rep.a_rate)).abs() < 0.0011 * px_y);
        assert!((rep.a_rate - 1.0).abs() < 1e-12, "the automatic [A] for the rate is [A]0/2");
    }
}

#[test]
fn print_is_grey_only_and_tells_the_lines_apart_by_their_stroke() {
    let mut styles: Vec<String> = Vec::new();
    for mode in [CONC, LIN, RATE] {
        for order in ORDERS {
            let svg = svg_of(&[s("style", "Print"), s("mode", mode), s("order", order), b("product", true), b("tangent", true), b("initial", true)]);
            for part in svg.split('"').filter(|x| x.starts_with('#')) {
                let h: String = if part.len() == 4 { part[1..].chars().flat_map(|c| [c, c]).collect() } else { part[1..].to_string() };
                let h = h.as_str();
                assert!(h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()), "{part}");
                let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap();
                assert!(c(0) == c(2) && c(2) == c(4), "Print draws a hue: {part}");
            }
            if mode == CONC && order != "Zero" {
                let g = geo(&svg);
                let st = |feat: &str| g.paths.iter().find(|p| p.1 == feat).map(|p| p.3.clone()).unwrap_or_else(|| panic!("no {feat}"));
                let four = [st("curveA"), st("curveP"), st("tan"), st("tan0")];
                // grey on grey: the width and the dash must tell them apart, not the shade
                let shape: Vec<String> = four.iter().map(|x| x.split_once('|').unwrap().1.to_string()).collect();
                for a in 0..4 { for c in a + 1..4 { assert_ne!(shape[a], shape[c], "{four:?}"); } }
                // and none is drawn like the dotted guides
                let guide = g.strokes.iter().find(|x| x.0 == "guide").map(|_| ()).is_some();
                assert!(guide);
                styles = four.to_vec();
                // the legend names each one
                let t = all_text(&svg);
                for want in ["[A]", "[P]", "Tangent at t = 10 s", "Initial-rate tangent"] { assert!(t.contains(want), "{want} not in the legend"); }
            }
        }
    }
    assert_eq!(styles.len(), 4);
    // colour does use colour
    let svg = svg_of(&[b("product", true)]);
    let g = geo(&svg);
    let col = |feat: &str| g.paths.iter().find(|p| p.1 == feat).unwrap().3.split('|').next().unwrap().to_string();
    assert_ne!(col("curveA"), col("curveP"));
}

#[test]
fn rendering_is_deterministic() {
    for l in [vec![], vec![s("mode", LIN), s("order", "Second")], vec![s("label_size", "Large"), b("tangent", true), b("product", true), s("style", "Print")], vec![s("mode", RATE), s("given", CURVE_GIVEN), b(SHOW_ANSWERS, false)]] {
        assert_eq!(svg_of(&l), svg_of(&l));
    }
}

#[test]
fn student_and_key_draw_the_same_curve_and_axes() {
    for mode in [CONC, LIN, RATE] {
        for order in ORDERS {
            for given in GIVENS {
                let key = svg_of(&[s("mode", mode), s("order", order), s("given", given), b(SHOW_ANSWERS, true)]);
                let stu = svg_of(&[s("mode", mode), s("order", order), s("given", given), b(SHOW_ANSWERS, false)]);
                let (gk, gs) = (geo(&key), geo(&stu));
                let curve = |g: &Geo| g.paths.iter().filter(|p| p.0 == "curve").map(|p| format!("{:?}", p.2)).collect::<Vec<_>>();
                assert!(!curve(&gk).is_empty(), "{mode} {order}: no curve");
                assert_eq!(curve(&gk), curve(&gs), "{mode} {order} {given}");
                let titles = |svg: &str| texts(svg).into_iter().filter(|t| t.kind == "axis" && t.text.contains('(')).map(|t| t.text).collect::<Vec<_>>();
                assert_eq!(titles(&key), titles(&stu));
                assert_eq!(crate::figures::svg_size(&key).0, crate::figures::svg_size(&stu).0);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The student copy
// ---------------------------------------------------------------------------------------------

/// Every value a student is asked to work out for this render, formatted as the key prints it.
fn answers(rep: &Report, dec: usize) -> Vec<String> {
    let kin = rep.kin;
    let r = |v: f64| res_fmt(v, dec);
    let mut v = vec![r(kin.half_life()), r(kin.t_frac(2)), r(kin.t_frac(3)), r(kin.t_frac(2) - kin.t_frac(1)), r(kin.conc(rep.t_tan)), r(kin.rate(rep.t_tan)), r(-kin.rate(rep.t_tan)), r(kin.rate(0.0)), r(-kin.rate(0.0)), r(kin.intercept()), r(kin.rate_of(rep.a_rate))];
    if let Some(te) = kin.t_end() { v.push(r(te)); }
    v.push(given_fmt(kin.slope()));
    v.retain(|x| !x.trim_start_matches('\u{2212}').chars().all(|c| c == '0' || c == '.'));
    v
}

fn student_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases = Vec::new();
    for mode in [CONC, LIN, RATE] {
        for order in ORDERS {
            for given in GIVENS {
                for style in ["Colour", "Print"] {
                    for size in ["Small", "Medium", "Large"] {
                        for (k, a0, tt) in [(0.1, 1.0, 10.0), (0.0347, 0.8, 3.3), (5.0, 0.04, 0.02), (2e-5, 1.5, 20000.0)] {
                            for marks in 0..4 {
                                cases.push(vec![
                                    s("mode", mode), s("order", order), s("given", given), s("style", style), s("label_size", size), f("k", k), f("a0", a0), f("t_tan", tt),
                                    b("half_lives", marks != 1), b("tangent", marks >= 2), b("initial", marks == 3), b("product", marks == 2), i("dec", 1 + (marks as i64 % 3)), b(SHOW_ANSWERS, false),
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
        let dec = p.i("dec").clamp(1, 4) as usize;
        let ans = answers(&rep, dec);
        let kin = rep.kin;
        let curve_given = p.s("given") == CURVE_GIVEN;
        // what the page prints as givens: k (unless the curve is the given), [A]0, the tangent's t and the marked [A]
        let mut givens = vec![given_fmt(kin.a0), given_fmt(rep.t_tan), given_fmt(rep.a_rate)];
        if !curve_given { givens.push(given_fmt(kin.k)); }
        let printed: Vec<String> = texts(&svg).into_iter().filter(|t| t.kind != "axis").map(|t| t.text).chain([title_of(&svg), desc_of(&svg)]).collect();
        for line in &printed {
            for tok in tokens(line) {
                if givens.contains(&tok) { continue; }
                assert!(!ans.contains(&tok), "the student copy prints the answer {tok} in {line:?} ({c:?})");
            }
            let low = line.to_lowercase();
            assert!(!low.contains("slope =") || line.contains("= ______"), "{line}");
            if curve_given {
                assert!(!tokens(line).contains(&given_fmt(kin.k)) || givens.contains(&given_fmt(kin.k)), "k printed on a curve-given student copy: {line}");
                for w in ["zero order", "first order", "second order", "zero-order", "first-order", "second-order", "rate = k", "1/s", "1/(m s)", "end of the reaction", "reaction is over"] {
                    assert!(!low.contains(w), "the order shows on a curve-given student copy: {w:?} in {line:?}");
                }
            }
        }
        // the same case's key does print answers from this list, so the check above has teeth
        let mut kc = c.clone();
        kc.push(b(SHOW_ANSWERS, true));
        let key = def().svg(&with(&kc));
        let key_tokens: Vec<String> = texts(&key).into_iter().filter(|t| t.kind != "axis").flat_map(|t| tokens(&t.text)).collect();
        if ans.iter().any(|a| key_tokens.contains(a)) { with_answers += 1; }
        // the blanks are there to fill
        let notes = notes_text(&svg);
        if p.b("readout") { assert!(notes.contains("______"), "{notes}"); }
        if curve_given && p.b("readout") { assert!(notes.contains("Order") && notes.contains("k = ______"), "{notes}"); }
        n += 1;
    }
    assert!(n >= 1700, "{n}");
    assert!(with_answers * 10 >= n * 8, "only {with_answers} of {n} keys print an answer from the list");
}

#[test]
fn the_curve_given_key_prints_the_order_and_k() {
    for mode in [CONC, LIN, RATE] {
        for order in ORDERS {
            let rep = build(&with(&[s("mode", mode), s("order", order), s("given", CURVE_GIVEN)])).1;
            let work = rep.readout.join(" ");
            assert!(work.contains(&format!("Order: {}", order.to_lowercase())), "{work}");
            assert!(work.contains(&format!("k = 0.1 {}", rep.kin.order.k_units())), "{work}");
        }
    }
}

#[test]
fn the_description_is_short_ascii_and_answer_free() {
    for c in student_cases().into_iter().step_by(7) {
        for ans in [true, false] {
            let mut c = c.clone();
            c.push(b(SHOW_ANSWERS, ans));
            let p = with(&c);
            let (svg, rep) = build(&p);
            let d = desc_of(&svg);
            assert!(!d.is_empty() && d.is_ascii() && d.len() <= 300, "{d:?}");
            assert!(!d.chars().any(|c| c.is_ascii_digit()), "a number in the description: {d}");
            // svg_size reads the first 400 bytes, which the description follows
            assert_eq!(crate::figures::svg_size(&svg).1, rep.frame.h.max(crate::figures::svg_size(&svg).1));
            if p.s("given") == CURVE_GIVEN && !ans {
                let low = d.to_lowercase();
                assert!(!low.contains("zero") && !low.contains("first") && !low.contains("second"), "{d}");
            }
        }
    }
    // the size reads right after an annotate too
    let svg = def().svg(&with(&[s("label_size", "Large"), b("tangent", true), b("initial", true), b("product", true)]));
    let (w, h) = crate::figures::svg_size(&svg);
    assert!(w > 700.0 && h > 400.0, "{w} {h}");
}

// ---------------------------------------------------------------------------------------------
// Clamps, refusals and odd input
// ---------------------------------------------------------------------------------------------

#[test]
fn clamps_and_refusals_come_with_a_note() {
    assert!(warnings(&with(&[])).is_empty(), "the defaults need no note: {:?}", warnings(&with(&[])));
    for mode in [CONC, LIN, RATE] { for order in ORDERS { assert!(warnings(&with(&[s("mode", mode), s("order", order)])).is_empty()); } }
    let note = |l: &[(&'static str, Value)], want: &str| {
        let w = warnings(&raw(l));
        assert!(w.iter().any(|n| n.contains(want)), "{l:?}: no note with {want:?} in {w:?}");
    };
    // k must be positive: zero and negative are refused, and the graph keeps a drawable k
    for k in [0.0, -2.0, -1e-9] {
        note(&[f("k", k)], "greater than 0");
        assert_eq!(setup(&raw(&[f("k", k)])).kin.k, 0.1);
    }
    note(&[f("k", 1e9)], "larger than");
    assert_eq!(setup(&raw(&[f("k", 1e9)])).kin.k, 1e6);
    note(&[f("k", 1e-12)], "smaller than");
    note(&[f("k", 0.123456)], "4 significant figures");
    assert_eq!(setup(&raw(&[f("k", 0.123456)])).kin.k, 0.1235);
    // [A]0
    note(&[f("a0", 0.0)], "no A");
    assert_eq!(setup(&raw(&[f("a0", 0.0)])).kin.a0, 1.0);
    note(&[f("a0", -3.0)], "no A");
    note(&[f("a0", 5000.0)], "larger than");
    // the time range: a sane multiple of the half-life
    for order in ORDERS {
        let t1 = setup(&raw(&[s("order", order)])).kin.half_life();
        note(&[s("order", order), f("tmax", 1e-3 * t1)], "too short");
        assert!(setup(&raw(&[s("order", order), f("tmax", 1e-3 * t1)])).x1 >= 0.5 * t1 - 1e-9);
        note(&[s("order", order), f("tmax", 1e6 * t1)], "too long");
        assert!(setup(&raw(&[s("order", order), f("tmax", 1e6 * t1)])).x1 <= 25.0 * t1);
    }
    note(&[f("tmax", -5.0)], "automatic");
    // the tangent's time
    note(&[b("tangent", true), f("t_tan", 1e5)], "past the end");
    let st = setup(&raw(&[b("tangent", true), f("t_tan", 1e5)]));
    assert!(st.t_tan <= st.x1 && st.t_tan > 0.9 * st.x1, "{} {}", st.t_tan, st.x1);
    note(&[b("tangent", true), f("t_tan", -1.0)], "before the reaction starts");
    note(&[b("tangent", true), f("t_tan", 2.123456)], "4 significant figures");
    // a zero-order tangent is the line itself, and after the end there is no reaction left
    note(&[b("tangent", true), s("order", "Zero"), f("t_tan", 2.0)], "the line itself");
    note(&[b("tangent", true), s("order", "Zero"), f("k", 0.05), f("t_tan", 20.0)], "is over");
    note(&[b("initial", true), s("order", "Zero")], "the line itself");
    // the rate mark
    note(&[s("mode", RATE), f("a_rate", 7.0)], "never above");
    assert_eq!(setup(&raw(&[s("mode", RATE), f("a_rate", 7.0)])).a_rate, 0.5);
    // a note on something not drawn would be noise
    assert!(warnings(&raw(&[f("t_tan", 1e5)])).is_empty());
    assert!(warnings(&raw(&[f("a_rate", 7.0)])).is_empty());
    // notes never carry a non-finite value
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for key in ["k", "a0", "tmax", "t_tan", "a_rate"] {
            for n in warnings(&raw(&[(key, Value::F(v)), b("tangent", true), s("mode", if key == "a_rate" { RATE } else { CONC })])) {
                let low = n.to_lowercase();
                assert!(!low.contains("nan") && !low.contains("inf"), "{n}");
            }
        }
    }
}

#[test]
fn odd_inputs_never_panic_or_print_nan() {
    let mut cases: Vec<Params> = Vec::new();
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1e300, 1e300, 0.0, -0.0, 1e-300, 7e-7] {
        for key in ["k", "a0", "tmax", "t_tan", "a_rate"] {
            for mode in [CONC, LIN, RATE] {
                for order in ORDERS {
                    cases.push(raw(&[(key, Value::F(v)), s("mode", mode), s("order", order), b("tangent", true), b("initial", true), b("product", true)]));
                }
            }
        }
    }
    cases.push(Params(Default::default()));
    cases.push(raw(&[s("mode", "<<bogus>>"), s("order", ""), s("given", "x"), s("label_size", "Huge"), s("style", "x"), ("k", Value::S("lots".into())), ("a0", Value::B(true)), ("dec", Value::F(f64::NAN))]));
    cases.push(raw(&[("dec", Value::I(i64::MAX)), ("k", Value::I(i64::MAX)), ("a0", Value::I(-5)), ("tmax", Value::I(i64::MIN))]));
    // extremes that fit the controls: a huge second-order k, a tiny zero-order one, a reaction over before the axis
    for order in ORDERS {
        for (k, a0, tmax) in [(1e6, 1000.0, 0.0), (1e-9, 1e-6, 0.0), (1e6, 1e-6, 1e15), (1e-9, 1000.0, 1e-15), (0.5, 1.0, 1.0)] {
            for mode in [CONC, LIN, RATE] {
                cases.push(with(&[s("order", order), f("k", k), f("a0", a0), f("tmax", tmax), s("mode", mode), b("tangent", true), f("t_tan", 1e15), b("initial", true), b("product", true), f("a_rate", 1e-6)]));
            }
        }
    }
    for p in &cases {
        let svg = (def().render)(p);
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{p:?}");
        let low = svg.to_lowercase();
        assert!(!low.contains("nan") && !low.contains("inf") && !low.contains('?'), "{p:?} prints a non-finite value");
        to_png(&svg, 0.3).unwrap_or_else(|e| panic!("{p:?}: {e}"));
        let _ = warnings(p);
        let _ = handles(ID, p, crate::figures::svg_size(&svg));
    }
}

// ---------------------------------------------------------------------------------------------
// Labels: real-glyph collision and attribution audit
// ---------------------------------------------------------------------------------------------

fn sweep_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases: Vec<Vec<(&'static str, Value)>> = Vec::new();
    // the classroom grid: every mode, order, given, style, size and answers state, with the marks in 16 ways
    for mode in [CONC, LIN, RATE] {
        for order in ORDERS {
            for given in GIVENS {
                for style in ["Colour", "Print"] {
                    for size in ["Small", "Medium", "Large"] {
                        for ans in [true, false] {
                            for m in 0..16 {
                                if mode != CONC && m >= 2 { continue; }
                                cases.push(vec![s("mode", mode), s("order", order), s("given", given), s("style", style), s("label_size", size), b(SHOW_ANSWERS, ans), b("half_lives", m & 1 == 0), b("tangent", m & 2 != 0), b("initial", m & 4 != 0), b("product", m & 8 != 0)]);
                            }
                        }
                    }
                }
            }
        }
    }
    // random over the whole space
    let mut r: u64 = 11;
    let mut next = move || { r = r.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (r >> 11) as f64 / (1u64 << 53) as f64 };
    let pick = |o: &[&'static str], x: f64| o[((x * o.len() as f64) as usize).min(o.len() - 1)];
    while cases.len() < 10_400 {
        let k = 10f64.powf(-5.0 + 8.0 * next());
        let a0 = 10f64.powf(-3.0 + 4.0 * next());
        let tmax = if next() < 0.5 { 0.0 } else { 10f64.powf(-3.0 + 9.0 * next()) };
        let tt = if next() < 0.3 { 10f64.powf(-3.0 + 9.0 * next()) } else { next() * 30.0 / k.max(0.01) };
        cases.push(vec![
            s("mode", pick(&[CONC, CONC, LIN, RATE], next())), s("order", pick(&ORDERS, next())), s("given", pick(&GIVENS, next())),
            s("style", pick(&["Colour", "Print"], next())), s("label_size", pick(&["Small", "Medium", "Large"], next())),
            f("k", k), f("a0", a0), f("tmax", tmax), f("t_tan", tt), f("a_rate", if next() < 0.5 { 0.0 } else { a0 * next() }),
            b("half_lives", next() < 0.8), b("tangent", next() < 0.5), b("initial", next() < 0.4), b("product", next() < 0.4), b("grid", next() < 0.8),
            i("dec", 1 + (next() * 4.0) as i64), b(SHOW_ANSWERS, next() < 0.5),
        ]);
    }
    cases
}

#[test]
fn labels_clear_and_attributed_across_ten_thousand_cases_in_real_glyphs() {
    let cases = sweep_cases();
    assert!(cases.len() >= 10_000);
    let chunks: Vec<&[Vec<(&'static str, Value)>]> = cases.chunks(cases.len().div_ceil(4)).collect();
    let results: Vec<(usize, Vec<String>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = chunks.iter().map(|ch| sc.spawn(move || {
            let mut g = Glyphs::new();
            let mut bad = Vec::new();
            let mut labels = 0;
            for c in ch.iter() {
                let p = with(c);
                let svg = def().svg(&p);
                labels += texts(&svg).iter().filter(|t| t.kind == "label").count();
                let lab = svg::label_factor(p.s("label_size"));
                for x in audit(&svg, &mut g, lab) { bad.push(format!("{x}  in {c:?}")); }
            }
            (labels, bad)
        })).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let labels: usize = results.iter().map(|r| r.0).sum();
    let bad: Vec<&String> = results.iter().flat_map(|r| r.1.iter()).collect();
    if let Ok(out) = std::env::var("VERSHION_AUDIT_OUT") { std::fs::write(out, bad.iter().map(|x| x.as_str()).collect::<Vec<_>>().join("\n")).unwrap(); }
    assert!(labels > 20_000, "only {labels} labels placed");
    assert!(bad.is_empty(), "{} problems, first: {:#?}", bad.len(), &bad[..bad.len().min(12)]);
}

/// The audit is not blind: it sees a label put on the curve, one put away from its feature, and text off the canvas.
#[test]
fn the_audit_catches_what_it_should() {
    let mut g = Glyphs::new();
    let svg = svg_of(&[s("mode", LIN)]);
    assert!(audit(&svg, &mut g, 1.0).is_empty(), "{:?}", audit(&svg, &mut g, 1.0));
    let ts = texts(&svg);
    let group: Vec<T> = ts.iter().filter(|t| t.kind == "label" && t.feat == "tri").cloned().collect();
    let lab = group[0].clone();
    let pts = path_of(&svg, "line");
    let on = pts[pts.len() / 3];
    // every line of the label moved by the same step
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
    assert!(audit(&moved(lab.x - 150.0, lab.y - 150.0), &mut g, 1.0).iter().any(|x| x.contains("from its tri feature")), "a label far off");
    assert!(audit(&moved(-30.0, 40.0), &mut g, 1.0).iter().any(|x| x.contains("leaves")), "a label off the canvas");
}

#[test]
fn large_labels_grow_the_canvas_and_nothing_is_clipped() {
    let mut g = Glyphs::new();
    let mut last = 0.0;
    for size in ["Small", "Medium", "Large"] {
        let svg = svg_of(&[s("label_size", size), b("tangent", true), b("initial", true), b("product", true)]);
        let (w, h) = crate::figures::svg_size(&svg);
        assert!(w * h > last, "{size}");
        last = w * h;
        assert!(audit(&svg, &mut g, svg::label_factor(size)).is_empty(), "{size}: {:?}", audit(&svg, &mut g, svg::label_factor(size)));
    }
}

// ---------------------------------------------------------------------------------------------
// Handles, examples, PNGs
// ---------------------------------------------------------------------------------------------

#[test]
fn handles_drag_a0_k_the_tangent_and_the_rate_point() {
    let p = with(&[b("tangent", true)]);
    let rep = build(&p).1;
    let fr = rep.frame;
    let hs = handles(ID, &p, (fr.w, fr.h)).unwrap();
    let start = hs.iter().find(|h| h.tip.contains("[A]₀")).expect("an [A]0 handle");
    assert!((start.pos.0 - fr.left()).abs() < 1e-9 && (start.pos.1 - fr.py(1.0)).abs() < 1e-9);
    let mut q = p.clone();
    (start.drag.clone().unwrap())(&mut q, (fr.left(), fr.py(0.5)));
    assert!((q.f("a0") - 0.5).abs() < 1e-9, "{}", q.f("a0"));
    let half = hs.iter().find(|h| h.tip.contains("half-life")).expect("a half-life handle");
    assert!((half.pos.0 - fr.px(rep.kin.half_life())).abs() < 1e-9);
    let mut q = p.clone();
    (half.drag.clone().unwrap())(&mut q, (fr.px(10.0), half.pos.1));
    assert!((q.f("k") - 0.0693).abs() < 1e-9, "k = ln 2/10 to 3 figures: {}", q.f("k"));
    let tan = hs.iter().find(|h| h.tip.contains("tangent")).expect("a tangent handle");
    let mut q = p.clone();
    (tan.drag.clone().unwrap())(&mut q, (fr.px(4.0), tan.pos.1));
    assert!((q.f("t_tan") - 4.0).abs() < 1e-9);
    // a click that does not move leaves the params alone
    let mut q = p.clone();
    (start.drag.clone().unwrap())(&mut q, start.pos);
    assert_eq!(format!("{q:?}"), format!("{p:?}"));
    // second order: the half-life sets k = 1/([A]0 t)
    let p = with(&[s("order", "Second"), f("a0", 2.0)]);
    let rep = build(&p).1;
    let hs = handles(ID, &p, (rep.frame.w, rep.frame.h)).unwrap();
    let half = hs.iter().find(|h| h.tip.contains("half-life")).unwrap();
    let mut q = p.clone();
    (half.drag.clone().unwrap())(&mut q, (rep.frame.px(4.0), half.pos.1));
    assert!((q.f("k") - 0.125).abs() < 1e-9, "{}", q.f("k"));
    // linearised first order: the intercept is ln [A]0
    let p = with(&[s("mode", LIN)]);
    let rep = build(&p).1;
    let hs = handles(ID, &p, (rep.frame.w, rep.frame.h)).unwrap();
    let ic = hs.iter().find(|h| h.tip.contains("intercept")).unwrap();
    let mut q = p.clone();
    (ic.drag.clone().unwrap())(&mut q, (rep.frame.left(), rep.frame.py((0.5f64).ln())));
    assert!((q.f("a0") - 0.5).abs() < 1e-9, "{}", q.f("a0"));
    // rate mode: the marked [A]
    let p = with(&[s("mode", RATE)]);
    let rep = build(&p).1;
    let hs = handles(ID, &p, (rep.frame.w, rep.frame.h)).unwrap();
    let pt = hs.iter().find(|h| h.tip.contains("rate")).unwrap();
    let mut q = p.clone();
    (pt.drag.clone().unwrap())(&mut q, (rep.frame.px(0.25), pt.pos.1));
    assert!((q.f("a_rate") - 0.25).abs() < 1e-9, "{}", q.f("a_rate"));
    // handles go through the subject dispatch
    assert!(!crate::figures::interact::handles_for(ID, &with(&[]), (fr.w, fr.h)).is_empty());
}

#[test]
fn reaction_rate_pngs() {
    let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
    let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
    for mode in [(CONC, "conc"), (LIN, "lin"), (RATE, "rate")] {
        for order in ORDERS { cases.push((format!("{}-{}", mode.1, order.to_lowercase()), vec![s("mode", mode.0), s("order", order)])); }
    }
    cases.push(("marks".into(), vec![b("tangent", true), b("initial", true), b("product", true)]));
    cases.push(("marks2".into(), vec![s("order", "Second"), b("tangent", true), b("initial", true), b("product", true), f("t_tan", 4.0), f("k", 0.5)]));
    cases.push(("nohalf".into(), vec![b("half_lives", false), b("tangent", true)]));
    cases.push(("curvegiven".into(), vec![s("given", CURVE_GIVEN), s("order", "Second"), f("k", 0.5)]));
    cases.push(("tiny".into(), vec![f("k", 2e-7), f("a0", 0.004)]));
    cases.push(("huge".into(), vec![s("order", "Second"), f("k", 1e6), f("a0", 1000.0), b("tangent", true), f("t_tan", 1.0)]));
    cases.push(("zeroshort".into(), vec![s("order", "Zero"), f("k", 0.05), f("tmax", 15.0), b("tangent", true), f("t_tan", 8.0)]));
    for (i, e) in examples(ID).unwrap().into_iter().enumerate() { cases.push((format!("ex{i}"), e.params)); }
    for (tag, list) in cases {
        for size in ["Small", "Medium", "Large"] {
            for (kind, extra) in [("key", vec![b(SHOW_ANSWERS, true)]), ("student", vec![b(SHOW_ANSWERS, false)]), ("print", vec![s("style", "Print")])] {
                if size != "Medium" && !(tag == "conc-first" || tag == "marks") { continue; }
                let mut l = list.clone();
                l.extend(extra);
                l.push(s("label_size", size));
                let svg = svg_of(&l);
                std::fs::write(format!("{dir}/rr-{tag}-{size}-{kind}.png"), to_png(&svg, 1.5).unwrap()).unwrap();
            }
        }
    }
}
