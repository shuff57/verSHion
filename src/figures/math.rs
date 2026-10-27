//! Math figure generators (original line-art SVG, modelled on the controls of mathfigures.com).
//! Entry point: `pub fn defs() -> Vec<FigureDef>`.
use super::svg::Doc;
use super::*;
use crate::figures::axes::{nice_step_with, Rule};
use std::f64::consts::PI;

const INK: &str = "#111827";
const GREY: &str = "#6b6b6b";
const TINT: &str = "#e9f1fb";
const BLUE: &str = "#1f5fbf";

type Pt = (f64, f64);

// ───────────────────────── shared helpers ─────────────────────────

pub(super) fn fmt(v: f64, dec: i64) -> String {
    let s = format!("{:.*}", dec.clamp(0, 4) as usize, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s.replace('-', "\u{2212}") }
}
fn fmtu(v: f64, dec: i64, unit: &str) -> String {
    let n = fmt(v, dec);
    if unit.trim().is_empty() { n } else { format!("{n} {}", unit.trim()) }
}
fn tw(s: &str, size: f64) -> f64 { s.chars().count() as f64 * size * 0.6 }
/// Text with a broad font fallback list (svg::Doc::text only names DejaVu/Arial).
fn txt(doc: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str) { txtw(doc, x, y, size, anchor, s, false); }
fn txtw(doc: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str, bold: bool) {
    let wt = if bold { r#" font-weight="bold""# } else { "" };
    doc.raw(&format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="DejaVu Sans, Arial, Liberation Sans, Noto Sans, Helvetica, sans-serif"{wt} fill="#000">{}</text>"##, super::svg::esc(s)));
}
fn put(doc: &mut Doc, x: f64, y: f64, text: &str, size: f64) { txt(doc, x, y + size * 0.34, size, "middle", text); }
fn dline(doc: &mut Doc, a: Pt, b: Pt, color: &str, sw: f64, dash: &str) {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = write!(s, r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{color}" stroke-width="{sw}" stroke-dasharray="{dash}"/>"#, a.0, a.1, b.0, b.1);
    doc.raw(&s);
}
fn dpoly(doc: &mut Doc, pts: &[Pt], color: &str, sw: f64, dash: &str) {
    use std::fmt::Write;
    let p: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
    let mut s = String::new();
    let _ = write!(s, r#"<polyline points="{}" fill="none" stroke="{color}" stroke-width="{sw}" stroke-dasharray="{dash}" stroke-linejoin="round"/>"#, p.join(" "));
    doc.raw(&s);
}
/// Colour param with fallback (older specs have no value for newly added colour keys).
fn colp(p: &Params, key: &str, default: &str) -> String {
    let v = p.s(key);
    if v.starts_with('#') && v.len() >= 4 { v.to_string() } else { default.to_string() }
}
const SWATCHES: [&str; 8] = ["#111111", "#1f5fbf", "#c0392b", "#2a8a3e", "#8e44ad", "#d68910", "#e9f1fb", "#c9def4"];

pub(super) fn nums(s: &str) -> Vec<f64> {
    s.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter_map(|t| t.trim().replace('\u{2212}', "-").parse().ok()).collect()
}
fn tokens(s: &str) -> Vec<String> {
    s.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).map(|t| t.trim().to_uppercase()).filter(|t| !t.is_empty()).collect()
}
fn listed(list: &[String], name: &str) -> bool {
    let rev: String = name.chars().rev().collect();
    list.iter().any(|t| t == name || *t == rev)
}
fn unit_v(a: Pt, b: Pt) -> Pt {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = (dx * dx + dy * dy).sqrt().max(1e-12);
    (dx / l, dy / l)
}
fn mix(c1: (f64, f64, f64), c2: (f64, f64, f64), t: f64) -> String {
    let t = t.clamp(0.0, 1.0);
    format!("#{:02x}{:02x}{:02x}", (c1.0 + (c2.0 - c1.0) * t) as u8, (c1.1 + (c2.1 - c1.1) * t) as u8, (c1.2 + (c2.2 - c1.2) * t) as u8)
}
fn dist(a: Pt, b: Pt) -> f64 { ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() }

/// Uniform fit of model points (y up) into a w x h canvas.
pub(super) struct Map { pub s: f64, pub ox: f64, pub oy: f64 }
thread_local! {
    /// Last layout produced by `Map::fit` on this thread (scale, origin x, origin y). Lets the
    /// interaction layer (`math_geo`) place handles with exactly the renderer's geometry.
    static LAST_MAP: std::cell::Cell<Option<(f64, f64, f64)>> = const { std::cell::Cell::new(None) };
}
/// Renders figure `p` with `render` and returns (scale, ox, oy) of the fit it used, if any.
pub(super) fn probe_map(render: fn(&Params) -> String, p: &Params) -> Option<(f64, f64, f64)> {
    LAST_MAP.with(|c| c.set(None));
    let _ = render(p);
    LAST_MAP.with(|c| c.get())
}
impl Map {
    pub(super) fn fit(pts: &[Pt], w: f64, h: f64, mx: f64, my: f64) -> Map {
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in pts { x0 = x0.min(p.0); x1 = x1.max(p.0); y0 = y0.min(p.1); y1 = y1.max(p.1); }
        if x0 > x1 { x0 = 0.0; x1 = 1.0; y0 = 0.0; y1 = 1.0; }
        let s = ((w - 2.0 * mx) / (x1 - x0).max(1e-9)).min((h - 2.0 * my) / (y1 - y0).max(1e-9));
        let m = Map { s, ox: w / 2.0 - s * (x0 + x1) / 2.0, oy: h / 2.0 + s * (y0 + y1) / 2.0 };
        LAST_MAP.with(|c| c.set(Some((m.s, m.ox, m.oy))));
        m
    }
    /// Like `fit`, but fills exactly `w` x `h` with the top-left of the drawing at (x0, y0): the
    /// reference draws its solids at a fixed 420 x 300 limit and sizes the page around them.
    pub(super) fn fit_at(pts: &[Pt], w: f64, h: f64, x0: f64, y0: f64) -> Map {
        let (mut a0, mut a1, mut b0, mut b1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in pts { a0 = a0.min(p.0); a1 = a1.max(p.0); b0 = b0.min(p.1); b1 = b1.max(p.1); }
        if a0 > a1 { a0 = 0.0; a1 = 1.0; b0 = 0.0; b1 = 1.0; }
        let s = (w / (a1 - a0).max(1e-9)).min(h / (b1 - b0).max(1e-9));
        let m = Map { s, ox: x0 - s * a0, oy: y0 + s * b1 };
        LAST_MAP.with(|c| c.set(Some((m.s, m.ox, m.oy))));
        m
    }
    pub(super) fn p(&self, q: Pt) -> Pt { (self.ox + q.0 * self.s, self.oy - q.1 * self.s) }
}

/// Right-angle square at vertex v (screen), arms toward da and db.
fn square_mark(doc: &mut Doc, v: Pt, da: Pt, db: Pt, size: f64) {
    doc.right_angle(v, unit_v(v, da), unit_v(v, db), size, INK, 1.2);
}

/// Arc of radius r at v between unit directions u and w (screen), interior (short) way.
fn arc(doc: &mut Doc, v: Pt, u: Pt, w: Pt, r: f64) {
    doc.arc_dirs(v, u, w, r, INK, 1.3);
}

fn new_doc(w: f64, h: f64) -> Doc { Doc::new(w, h) }

// ───────────────────────── polygon engine ─────────────────────────

const MSERIF: &str = "Liberation Serif, Noto Serif, DejaVu Serif, Times New Roman, serif";

/// Math-typeset label centred on (x, y): serif face; letters (variables, vertex names) italic;
/// numbers, operators, multi-letter lowercase words (sin, cm) and the unit upright.
fn mlabel(doc: &mut Doc, x: f64, y: f64, size: f64, s: &str, unit: &str) {
    doc.raw(&mlabel_svg(x, y, size, s, unit));
}
/// `mlabel` as a movable label with id `id` (see `labels::wrap`).
fn mlabel_id(doc: &mut Doc, id: &str, x: f64, y: f64, size: f64, s: &str, unit: &str) {
    doc.raw(&super::labels::wrap(id, (x, y), super::labels::text_half(s, size), &mlabel_svg(x, y, size, s, unit)));
}
fn mlabel_svg(x: f64, y: f64, size: f64, s: &str, unit: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let unit = unit.trim();
    let mut body = String::new();
    let mut i = 0;
    while i < chars.len() {
        let alpha = chars[i].is_alphabetic();
        let mut j = i;
        while j < chars.len() && chars[j].is_alphabetic() == alpha { j += 1; }
        let run: String = chars[i..j].iter().collect();
        let upright = !alpha || (!unit.is_empty() && run == unit) || (run.chars().count() > 1 && run.chars().all(|c| c.is_lowercase()));
        let e = super::svg::esc(&run);
        if upright { body.push_str(&e); } else { body.push_str(&format!(r#"<tspan font-style="italic">{e}</tspan>"#)); }
        i = j;
    }
    format!(r##"<text x="{x:.2}" y="{:.2}" font-size="{size:.1}" text-anchor="middle" font-family="{MSERIF}" fill="#111827" stroke="#fff" stroke-width="{:.1}" stroke-linejoin="round" paint-order="stroke">{body}</text>"##, y + size * 0.34, size * 0.2)
}

/// Label-size toolbar setting (Large / Medium / Small) as a font scale.
fn fscale(p: &Params) -> f64 { match p.s("label_size") { "Large" => 1.25, "Small" => 0.8, _ => 1.0 } }
/// Construction-line style (diagonals, heights, radius, apothem) as an SVG dash array.
fn dash_of(p: &Params) -> &'static str { match p.s("line_style") { "Solid" => "none", "Dotted" => "0.1 5", _ => "6 4" } }

/// `BC = 2x + 3; A = y` -> [(BC, 2x + 3), (A, y)] (keys upper-cased).
fn text_map(s: &str) -> Vec<(String, String)> {
    s.split(|c| c == ';' || c == '\n')
        .filter_map(|it| {
            let (k, v) = it.split_once('=')?;
            let k = k.trim().to_uppercase();
            (!k.is_empty()).then(|| (k, v.trim().to_string()))
        })
        .collect()
}
fn text_for(map: &[(String, String)], name: &str) -> Option<String> {
    let name = name.to_uppercase();
    let rev: String = name.chars().rev().collect();
    map.iter().find(|(k, _)| *k == name || *k == rev).map(|(_, v)| v.clone())
}

/// `AB 1, CD 2` / `all 2` / `none` -> per-item counts (sides when `sides`, else corners).
/// None when the text is empty (caller falls back to its default marks).
fn parse_counts(s: &str, names: &[String], sides: bool) -> Option<Vec<u8>> {
    if s.trim().is_empty() { return None; }
    let n = names.len();
    let mut out = vec![0u8; n];
    for item in s.split(|c| c == ',' || c == ';' || c == '\n') {
        let t: Vec<String> = item.split(|c: char| c.is_whitespace() || c == ':' || c == '=').filter(|x| !x.is_empty()).map(|x| x.to_uppercase()).collect();
        let Some(head) = t.first() else { continue };
        let k = t.get(1).and_then(|v| v.parse::<u8>().ok()).unwrap_or(1).min(3);
        for i in 0..n {
            let nm = if sides { format!("{}{}", names[i], names[(i + 1) % n]) } else { names[i].clone() };
            if head == "ALL" || listed(std::slice::from_ref(head), &nm) { out[i] = k; }
        }
    }
    Some(out)
}

/// Label for a construction line from its choice (`<name> = measure` | Measure | Text | None).
/// Returns (label, raw): "=" means the measure; raw labels are drawn as given.
fn named_label(choice: &str, text: &str, name: &str) -> (Option<String>, bool) {
    let t = if text.trim().is_empty() { name.to_string() } else { text.trim().to_string() };
    match choice {
        "Measure" => (Some("=".into()), true),
        "Text" => (Some(t), true),
        "None" => (None, true),
        _ => (Some(t), false),
    }
}

fn vnames(p: &Params, n: usize) -> Vec<String> {
    let custom: Vec<String> = p.s("vnames").split(|c: char| c == ',' || c.is_whitespace()).map(|t| t.trim().to_uppercase()).filter(|t| !t.is_empty()).collect();
    (0..n).map(|i| custom.get(i).cloned().unwrap_or_else(|| ((b'A' + i as u8) as char).to_string())).collect()
}

#[derive(Clone, Default)]
struct Extra { a: Pt, b: Pt, label: Option<String>, t: f64, dash: &'static str, raw: bool, /** movable-label id of its label (numbered when repeated) */ id: &'static str }
type Mark = (Pt, Pt, Pt);

struct PolySpec {
    pts: Vec<Pt>,
    names: Vec<String>,
    side_txt: Vec<Option<String>>,
    ang_txt: Vec<Option<String>>,
    ticks: Vec<u8>,
    arrows: Vec<u8>,
    arcs: Vec<u8>,
    right_marks: bool,
    vlabels: bool,
    extras: Vec<Extra>,
    marks: Vec<Mark>,
    dots: Vec<Pt>,
    /// (model anchor, model direction, text): free labels such as the centre's name.
    notes: Vec<(Pt, Pt, String)>,
    rot: f64,
    flip: bool,
    bottom: Option<usize>,
    fill: String,
    fs: f64,
}

thread_local! {
    /// (flip, total turn in degrees) used by the last `draw_poly` on this thread, so the handle
    /// layer can map model points exactly as the renderer did.
    static LAST_XF: std::cell::Cell<(bool, f64)> = const { std::cell::Cell::new((false, 0.0)) };
}
pub(super) fn last_xf() -> (bool, f64) { LAST_XF.with(|c| c.get()) }

fn rot_deg(p: Pt, d: f64) -> Pt {
    let (c, s) = (d.to_radians().cos(), d.to_radians().sin());
    (p.0 * c - p.1 * s, p.0 * s + p.1 * c)
}

fn auto_ticks(lens: &[f64]) -> Vec<u8> {
    let mut groups: Vec<f64> = Vec::new();
    let mut out = vec![0u8; lens.len()];
    for (i, l) in lens.iter().enumerate() {
        let cnt = lens.iter().filter(|m| (**m - l).abs() < 1e-6 * l.abs().max(1.0)).count();
        if cnt < 2 { continue; }
        let g = match groups.iter().position(|g| (g - l).abs() < 1e-6 * l.abs().max(1.0)) {
            Some(g) => g,
            None => { groups.push(*l); groups.len() - 1 }
        };
        out[i] = (g + 1).min(3) as u8;
    }
    out
}

/// Height from vertex `v` onto line p-q: the dashed height, extensions past the side when the
/// foot falls outside it, and the right-angle mark at the foot.
fn height_from(v: Pt, p: Pt, q: Pt, label: (Option<String>, bool), dash: &'static str) -> (Vec<Extra>, Vec<Mark>, f64) {
    let u = unit_v(p, q);
    let l = dist(p, q);
    let t = (v.0 - p.0) * u.0 + (v.1 - p.1) * u.1;
    let foot = (p.0 + u.0 * t, p.1 + u.1 * t);
    // label nearer the foot, where the shape is usually wider
    let mut ex = vec![Extra { a: v, b: foot, label: label.0, t: 0.62, dash, raw: label.1, id: "h" }];
    if t < 0.0 { ex.push(Extra { a: p, b: foot, t: 0.5, dash, ..Default::default() }); }
    if t > l { ex.push(Extra { a: q, b: foot, t: 0.5, dash, ..Default::default() }); }
    let dir = if t < l / 2.0 { (foot.0 + u.0, foot.1 + u.1) } else { (foot.0 - u.0, foot.1 - u.1) };
    (ex, vec![(foot, v, dir)], dist(v, foot))
}

fn height_label(p: &Params) -> (Option<String>, bool) { named_label(p.s("height_label"), p.s("height_text"), "h") }

/// Renders a polygon spec: flip, side-at-bottom and turn, then marks and math-typeset labels.
fn draw_poly(s: PolySpec, dec: i64, unit: &str) -> String {
    // dragged labels keep their place relative to the shape when it is turned
    let turn = s.rot;
    super::labels::turned(turn, move || draw_poly_inner(s, dec, unit))
}
fn draw_poly_inner(s: PolySpec, dec: i64, unit: &str) -> String {
    let n = s.pts.len();
    let flip = s.flip;
    let fl = move |p: Pt| if flip { (-p.0, p.1) } else { p };
    let mut base = 0.0;
    if let Some(i) = s.bottom.filter(|i| *i < n) {
        let (a, b) = (fl(s.pts[i]), fl(s.pts[(i + 1) % n]));
        base = -(b.1 - a.1).atan2(b.0 - a.0).to_degrees();
        let c = (s.pts.iter().map(|q| fl(*q).0).sum::<f64>() / n as f64, s.pts.iter().map(|q| fl(*q).1).sum::<f64>() / n as f64);
        if rot_deg(c, base).1 < rot_deg(a, base).1 { base += 180.0; }
    }
    let total = base + s.rot;
    LAST_XF.with(|c| c.set((flip, total)));
    let rot = move |p: Pt| rot_deg(fl(p), total);
    let fs = s.fs;
    let pts: Vec<Pt> = s.pts.iter().map(|p| rot(*p)).collect();
    let mut all = pts.clone();
    for e in &s.extras { all.push(rot(e.a)); all.push(rot(e.b)); }
    for d in &s.dots { all.push(rot(*d)); }
    let (w, h) = (480.0, 360.0);
    let k = 0.6 + 0.4 * fs;
    let m = Map::fit(&all, w, h, 50.0 * k, 36.0 * k);
    let sp: Vec<Pt> = pts.iter().map(|p| m.p(*p)).collect();
    let cen = (sp.iter().map(|p| p.0).sum::<f64>() / n as f64, sp.iter().map(|p| p.1).sum::<f64>() / n as f64);
    let mut doc = new_doc(w, h);
    doc.poly(&sp, &s.fill, INK, 1.5);

    // construction lines
    for e in &s.extras {
        let (a, b) = (m.p(rot(e.a)), m.p(rot(e.b)));
        dline(&mut doc, a, b, "#333", 1.6, if e.dash.is_empty() { "6 4" } else { e.dash });
    }
    if s.right_marks {
        for mk in &s.marks {
            let (v, a, b) = (m.p(rot(mk.0)), m.p(rot(mk.1)), m.p(rot(mk.2)));
            square_mark(&mut doc, v, a, b, 10.0);
        }
    }
    for d in &s.dots { let q = m.p(rot(*d)); doc.circle(q.0, q.1, 3.4, INK, INK, 1.0); }

    // congruence ticks / parallel arrows (middle of the side; split when a side has both)
    for i in 0..n {
        let (a, b) = (sp[i], sp[(i + 1) % n]);
        let u = unit_v(a, b);
        let nn = (-u.1, u.0);
        let kt = s.ticks.get(i).copied().unwrap_or(0) as i32;
        let ka = s.arrows.get(i).copied().unwrap_or(0) as i32;
        let both = kt > 0 && ka > 0;
        let at = |t: f64| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        if kt > 0 {
            let mid = at(if both { 0.34 } else { 0.5 });
            for j in 0..kt {
                let o = (j as f64 - (kt - 1) as f64 / 2.0) * 5.0;
                let c = (mid.0 + u.0 * o, mid.1 + u.1 * o);
                doc.line(c.0 - nn.0 * 6.0, c.1 - nn.1 * 6.0, c.0 + nn.0 * 6.0, c.1 + nn.1 * 6.0, INK, 1.7);
            }
        }
        if ka > 0 {
            // parallel sides point their arrows the same way (rightward, or up when vertical)
            let u = if u.0 < -1e-6 || (u.0.abs() <= 1e-6 && u.1 > 0.0) { (-u.0, -u.1) } else { u };
            let mid = at(if both { 0.66 } else { 0.5 });
            for j in 0..ka {
                let o = (j as f64 - (ka - 1) as f64 / 2.0) * 7.0;
                let c = (mid.0 + u.0 * o, mid.1 + u.1 * o);
                let tip = (c.0 + u.0 * 4.0, c.1 + u.1 * 4.0);
                let back = (c.0 - u.0 * 4.0, c.1 - u.1 * 4.0);
                doc.polyline(&[(back.0 + nn.0 * 6.0, back.1 + nn.1 * 6.0), tip, (back.0 - nn.0 * 6.0, back.1 - nn.1 * 6.0)], INK, 1.8);
            }
        }
    }

    // angles: right-angle squares, arcs (one per congruence mark, or one for a labelled angle), labels
    let asz = 17.0 * fs;
    for i in 0..n {
        let v = sp[i];
        let (pa, pb) = (sp[(i + n - 1) % n], sp[(i + 1) % n]);
        let (u, wv) = (unit_v(v, pa), unit_v(v, pb));
        let ang = (u.0 * wv.0 + u.1 * wv.1).clamp(-1.0, 1.0).acos().to_degrees();
        let is_right = (ang - 90.0).abs() < 0.3;
        let txt = s.ang_txt.get(i).cloned().flatten();
        let narc = s.arcs.get(i).copied().unwrap_or(0) as usize;
        let minlen = dist(v, pa).min(dist(v, pb));
        let mut ar = 20.0f64.min(minlen * 0.3).max(8.0);
        if s.right_marks && is_right {
            square_mark(&mut doc, v, pa, pb, 12.0);
            ar = 17.0;
        } else if txt.is_some() || narc > 0 {
            for j in 0..narc.max(1) { arc(&mut doc, v, u, wv, ar + 4.0 * j as f64); }
            ar += 4.0 * (narc.max(1) - 1) as f64;
        }
        if let Some(t) = txt {
            let mut b = (u.0 + wv.0, u.1 + wv.1);
            let bl = (b.0 * b.0 + b.1 * b.1).sqrt();
            if bl < 1e-6 { b = (-u.1, u.0); } else { b = (b.0 / bl, b.1 / bl); }
            let mut r = ar + 6.0 + 0.5 * (tw(&t, asz) * 0.85 * b.0.abs() + asz * b.1.abs()) + (40.0 - ang).max(0.0) * 0.5;
            // in a narrow wedge the reference sits the value clear of both rays (24 degrees: about
            // 0.19 of the base out from the corner), not on top of the nearer one
            let half = (ang / 2.0).to_radians();
            if half.sin() > 0.05 {
                let (hw, hh) = (0.5 * tw(&t, asz) * 0.85, asz * 0.36);
                r = r.max((hh * half.cos() + hw * half.sin() + 6.5 * (asz / 17.0)) / half.sin());
            }
            mlabel_id(&mut doc, &format!("a{i}"), v.0 + b.0 * r, v.1 + b.1 * r, asz, &t, unit);
        }
    }
    // side labels
    let ssz = 17.0 * fs;
    for i in 0..n {
        if let Some(t) = s.side_txt.get(i).and_then(|t| t.clone()) {
            let (a, b) = (sp[i], sp[(i + 1) % n]);
            let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
            let u = unit_v(a, b);
            let mut nn = (-u.1, u.0);
            if nn.0 * (mid.0 - cen.0) + nn.1 * (mid.1 - cen.1) < 0.0 { nn = (-nn.0, -nn.1); }
            let marked = s.ticks.get(i).copied().unwrap_or(0) > 0 || s.arrows.get(i).copied().unwrap_or(0) > 0;
            let off = if marked { 12.0 } else { 9.0 } + 0.5 * (tw(&t, ssz) * 0.85 * nn.0.abs() + ssz * 0.9 * nn.1.abs());
            mlabel_id(&mut doc, &format!("s{i}"), mid.0 + nn.0 * off, mid.1 + nn.1 * off, ssz, &t, unit);
        }
    }
    let ex_scr: Vec<(Pt, Pt)> = s.extras.iter().map(|e| (m.p(rot(e.a)), m.p(rot(e.b)))).collect();
    for (ei, e) in s.extras.iter().enumerate() {
        if let Some(t) = &e.label {
            let hv = dist(e.a, e.b);
            let txt = if t.starts_with('=') { fmtu(hv, dec, unit) } else if e.raw || s.names.is_empty() { t.clone() } else { format!("{t} = {}", fmtu(hv, dec, unit)) };
            let (a, b) = ex_scr[ei];
            let tt = if e.t == 0.0 { 0.5 } else { e.t };
            let mid = (a.0 + (b.0 - a.0) * tt, a.1 + (b.1 - a.1) * tt);
            let u = unit_v(a, b);
            let nn = (-u.1, u.0);
            let off = 6.0 + 0.5 * (tw(&txt, ssz) * 0.85 * nn.0.abs() + ssz * 0.9 * nn.1.abs());
            let mid_in = point_in(mid, &sp);
            let mut best = (f64::MIN, mid);
            for sg in [1.0, -1.0] {
                let c = (mid.0 + nn.0 * off * sg, mid.1 + nn.1 * off * sg);
                let mut dmin = f64::MAX;
                for i in 0..n { dmin = dmin.min(seg_dist(c, sp[i], sp[(i + 1) % n])); }
                for (j, (p1, p2)) in ex_scr.iter().enumerate() { if j != ei { dmin = dmin.min(seg_dist(c, *p1, *p2)); } }
                let ok = point_in(c, &sp) == mid_in;
                let sc = if ok { dmin } else { -1.0 };
                if sc > best.0 { best = (sc, c); }
            }
            let base = if e.id.is_empty() { "x" } else { e.id };
            let nth = s.extras[..ei].iter().filter(|o| o.label.is_some() && (if o.id.is_empty() { "x" } else { o.id }) == base).count();
            let id = if nth == 0 && !e.id.is_empty() { base.to_string() } else { format!("{base}{nth}") };
            mlabel_id(&mut doc, &id, best.1 .0, best.1 .1, ssz, &txt, unit);
        }
    }
    // free notes (centre name, ...)
    for (ni, (at, dir, t)) in s.notes.iter().enumerate() {
        let q = m.p(rot(*at));
        let d = rot_deg(fl(*dir), total);
        let l = (d.0 * d.0 + d.1 * d.1).sqrt().max(1e-9);
        let (dx, dy) = (d.0 / l, -d.1 / l);
        let r = 12.0 + 0.5 * (tw(t, ssz) * 0.85 * dx.abs() + ssz * dy.abs());
        mlabel_id(&mut doc, &format!("n{ni}"), q.0 + dx * r, q.1 + dy * r, ssz, t, unit);
    }
    // vertex letters
    if s.vlabels {
        let vsz = 19.0 * fs;
        for i in 0..n {
            let v = sp[i];
            let (u, wv) = (unit_v(v, sp[(i + n - 1) % n]), unit_v(v, sp[(i + 1) % n]));
            let mut b = (-(u.0 + wv.0), -(u.1 + wv.1));
            let bl = (b.0 * b.0 + b.1 * b.1).sqrt();
            if bl < 1e-6 { b = (u.1, -u.0); } else { b = (b.0 / bl, b.1 / bl); }
            let off = 10.0 + 0.5 * vsz * 0.75;
            mlabel_id(&mut doc, &format!("v{i}"), v.0 + b.0 * off, v.1 + b.1 * off, vsz, &s.names[i], "");
        }
    }
    doc.finish()
}

fn seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = (dx * dx + dy * dy).max(1e-12);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    dist(p, (a.0 + dx * t, a.1 + dy * t))
}
fn point_in(p: Pt, poly: &[Pt]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 { inside = !inside; }
    }
    inside
}

fn poly_angles(pts: &[Pt]) -> Vec<f64> {
    let n = pts.len();
    (0..n).map(|i| {
        let (u, w) = (unit_v(pts[i], pts[(i + n - 1) % n]), unit_v(pts[i], pts[(i + 1) % n]));
        (u.0 * w.0 + u.1 * w.1).clamp(-1.0, 1.0).acos().to_degrees()
    }).collect()
}

/// Side index chosen by the `bottom` param (None = as it stands / upright).
fn bottom_side(p: &Params, n: usize) -> Option<usize> {
    let b = p.s("bottom").trim().to_uppercase();
    if b.is_empty() || b.contains("AS IT STANDS") || b.starts_with("NONE") { return None; }
    (0..n).position(|i| {
        let nm = format!("{}{}", (b'A' + i as u8) as char, (b'A' + ((i + 1) % n) as u8) as char);
        listed(std::slice::from_ref(&b), &nm)
    })
}

/// Height label params for figures that draw heights.
fn height_common() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("height_label", "Height label", &["h = measure", "Measure", "Text", "None"], "h = measure"),
        ParamDef::text("height_text", "Height name / text", "h"),
    ]
}

fn msg(text: &str) -> String {
    let mut d = new_doc(480.0, 120.0);
    put(&mut d, 240.0, 60.0, text, 16.0);
    d.finish()
}

// ───────────────────────── flat shapes: the Measures model (meas-a) ─────────────────────────
//
// Triangle, rectangle, parallelogram, trapezoid, kite and regular polygon are entered the way the
// reference does it: one row per side / angle. A typed value is "given"; an empty box is worked out
// from the others (`computed`). Each row has a label mode ("" = automatic: the measure when given,
// nothing when worked out; or Measure / Text / None), a label text and congruence ticks or arcs.
// Angles and parallel lines use a rays / lines card instead. The solvers report what is wrong in
// the reference's words when the givens are too few, too many or inconsistent.

mod flat {
use super::*;

pub(crate) const TICKS: [&str; 4] = ["None", "1", "2", "3"];
pub(crate) const STYLES: [&str; 3] = ["Solid", "Dashed", "Dotted"];
pub(crate) const NAME_K: [&str; 4] = ["name_a", "name_b", "name_c", "name_d"];
const BAD_NUM: &str = "Type a number, like 12, 2.5, 5/2 or 3\u{221a}2.";

// ----- typed values -----

/// A typed measure: its value, how it prints (as typed, with √ and π) and how far a worked-out
/// value may differ from it before the two disagree (half a unit in its last typed place).
#[derive(Clone, Debug)]
pub(crate) struct Typed { pub v: f64, pub show: String, pub tol: f64 }

/// `5sqrt2` -> 5√2, `sqrt(x+1)` -> √(x+1), `pi` -> π, `*` -> ·, `-` -> −.
pub(crate) fn pretty(t: &str) -> String {
    let s = t.trim().replace('\u{221a}', "sqrt");
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i..].starts_with(&['s', 'q', 'r', 't']) {
            i += 4;
            if i < cs.len() && cs[i] == '(' {
                let st = i;
                let mut depth = 0;
                while i < cs.len() {
                    if cs[i] == '(' { depth += 1; } else if cs[i] == ')' { depth -= 1; if depth == 0 { break; } }
                    i += 1;
                }
                let inner: String = cs[st + 1..i.min(cs.len())].iter().collect();
                i += 1;
                let inner = pretty(&inner);
                if inner.chars().all(|c| c.is_ascii_digit() || c == '.') { out.push('\u{221a}'); out.push_str(&inner); } else { out.push_str(&format!("\u{221a}({inner})")); }
            } else {
                out.push('\u{221a}');
            }
            continue;
        }
        if cs[i..].starts_with(&['p', 'i']) { out.push('\u{3c0}'); i += 2; continue; }
        match cs[i] { '*' => out.push('\u{b7}'), '-' => out.push('\u{2212}'), c => out.push(c) }
        i += 1;
    }
    out
}

/// Value of a typed measure: numbers, fractions, sqrt / √, pi / π and + − × ÷. Letters are refused.
pub(crate) fn eval_typed(t: &str) -> Option<f64> {
    let s = t.trim().replace('\u{221a}', "sqrt").replace('\u{3c0}', "pi").replace('\u{2212}', "-").replace('\u{b7}', "*").replace('\u{d7}', "*").replace('\u{f7}', "/").to_lowercase();
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i..].starts_with(&['s', 'q', 'r', 't']) {
            out.push_str("sqrt");
            i += 4;
            if i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                out.push('(');
                while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') { out.push(cs[i]); i += 1; }
                out.push(')');
            }
            continue;
        }
        out.push(cs[i]);
        i += 1;
    }
    if out.replace("sqrt", "").replace("pi", "").chars().any(|c| c.is_alphabetic()) { return None; }
    eval(&out, 0.0).filter(|v| v.is_finite())
}

/// "" -> Ok(None); a number or expression -> Ok(Some); anything else -> Err.
pub(crate) fn parse_typed(s: &str) -> Result<Option<Typed>, ()> {
    let t = s.trim();
    if t.is_empty() { return Ok(None); }
    let v = eval_typed(t).ok_or(())?;
    let plain = t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '\u{2212}');
    let tol = if plain {
        let d = t.split_once('.').map(|(_, f)| f.len()).unwrap_or(0) as i32;
        0.5 * 10f64.powi(-d) + 1e-9
    } else {
        0.005 + 1e-9 * v.abs()
    };
    Ok(Some(Typed { v, show: pretty(t), tol }))
}

fn typed_of(p: &Params, key: &str) -> Result<Option<Typed>, String> {
    if key.is_empty() { return Ok(None); }
    parse_typed(p.s(key)).map_err(|_| BAD_NUM.to_string())
}

/// Plain ASCII number for writing back into a value box (drag handles, migration).
pub(crate) fn num_str(v: f64, dec: usize) -> String {
    let s = format!("{:.*}", dec, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

/// Corner names from the per-corner boxes (blank = A, B, C, ...).
pub(crate) fn corner_names(p: &Params, n: usize) -> Vec<String> {
    (0..n).map(|i| {
        let v = NAME_K.get(i).map(|k| p.s(k).trim().to_string()).unwrap_or_default();
        if v.is_empty() { ((b'A' + i as u8) as char).to_string() } else { v }
    }).collect()
}

/// What prints for one measure: label mode ("" = automatic), text, typed value, worked-out value.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lab_text(p: &Params, lab: &str, txt: &str, typed: Option<&Typed>, val: Option<f64>, linked: bool, angle: bool, dec: i64, unit: &str) -> Option<String> {
    let mode = p.s(lab);
    let mode = if mode.is_empty() { if typed.is_some() && !linked { "Measure" } else { "None" } } else { mode };
    match mode {
        "None" => None,
        "Text" => {
            let t = p.s(txt).trim();
            Some(if t.is_empty() { "x".to_string() } else { t.to_string() })
        }
        _ => {
            let n = match (typed, val) {
                (Some(t), _) if !linked => t.show.clone(),
                (_, Some(v)) => fmt(v, dec),
                _ => "?".to_string(),
            };
            Some(if angle { format!("{n}\u{b0}") } else if unit.trim().is_empty() { n } else { format!("{n} {}", unit.trim()) })
        }
    }
}

fn tick_n(p: &Params, key: &str) -> u8 { p.s(key).parse::<u8>().unwrap_or(0).min(3) }

/// A message in place of the figure (what the reference shows in the card when the measures fail).
pub(crate) fn note_fig(text: &str) -> String {
    let mut d = new_doc(480.0, 360.0);
    let mut lines: Vec<String> = vec![String::new()];
    for w in text.split(' ') {
        let cur = lines.last_mut().unwrap();
        if !cur.is_empty() && cur.chars().count() + w.chars().count() > 44 { lines.push(w.to_string()); } else { if !cur.is_empty() { cur.push(' '); } cur.push_str(w); }
    }
    let y0 = 180.0 - (lines.len() as f64 - 1.0) * 12.0;
    for (i, l) in lines.iter().enumerate() { put(&mut d, 240.0, y0 + 24.0 * i as f64, l, 16.0); }
    d.finish()
}

fn few(n: usize, what: &str) -> String {
    if n == 1 { format!("Give one more measure to draw the {what}.") } else { format!("Give {n} more measures to draw the {what}.") }
}

fn mismatch(name: &str, would: f64, t: &Typed, angle: bool) -> String {
    let d = if angle { "\u{b0}" } else { "" };
    format!("With the other measures, {name} would be {}{d}, not {}{d}.", fmt(would, 2), t.show)
}

fn range_check(name: &str, t: &Option<Typed>, angle: bool) -> Result<(), String> {
    if let Some(t) = t {
        if angle && (t.v <= 0.0 || t.v >= 180.0) { return Err(format!("{name} has to be between 0\u{b0} and 180\u{b0}.")); }
        if !angle && t.v <= 0.0 { return Err(format!("{name} has to be longer than 0.")); }
    }
    Ok(())
}

// ----- triangle solver -----

/// Why three-of-six measures fail (indices: 0-2 angles A B C, 3 AB, 4 BC, 5 CA).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TriErr { Few(usize), Sum2(usize, usize, f64), Sum3(f64), Ineq(usize), Obtuse(usize, bool), Short(usize, f64), Mismatch(usize, f64) }

#[derive(Debug, Clone)]
pub(crate) struct TriOut {
    pub ang: [f64; 3],
    pub side: Option<[f64; 3]>,
    /// The measures make two triangles (SSA); `other` picks the second.
    #[allow(dead_code)]
    pub two: bool,
}

/// Side opposite each angle (A -> BC, B -> CA, C -> AB) and angle opposite each side.
const OPP_SIDE: [usize; 3] = [4, 5, 3];
const OPP_ANG: [usize; 3] = [2, 0, 1];

/// Solves a triangle from any measures (value, tolerance) that fix it. Two or more angles: the
/// third follows, and the first given side (AB, BC, CA) scales it. Else three sides (SSS), else two
/// sides and an angle: between them (SAS) or not (SSA, acute solution unless `other`). Every given
/// measure the solution did not use is checked against it.
pub(crate) fn tri_core(g: &[Option<(f64, f64)>; 6], other: bool) -> Result<TriOut, TriErr> {
    let v = |i: usize| g[i].map(|x| x.0).unwrap_or(0.0);
    let angs: Vec<usize> = (0..3).filter(|i| g[*i].is_some()).collect();
    let sides: Vec<usize> = (3..6).filter(|i| g[*i].is_some()).collect();
    if angs.len() >= 2 {
        let s2 = v(angs[0]) + v(angs[1]);
        if s2 >= 180.0 - 1e-9 { return Err(TriErr::Sum2(angs[0], angs[1], s2)); }
    }
    if angs.len() == 3 {
        let s: f64 = (0..3).map(v).sum();
        let tol: f64 = (0..3).map(|i| g[i].map(|x| x.1).unwrap_or(0.0)).sum();
        if (s - 180.0).abs() > tol { return Err(TriErr::Sum3(s)); }
    }
    let n = angs.len() + sides.len();
    if n < 3 { return Err(TriErr::Few(3 - n)); }
    let (sd, cd) = (|d: f64| d.to_radians().sin(), |d: f64| d.to_radians().cos());
    let acosd = |x: f64| x.clamp(-1.0, 1.0).acos().to_degrees();
    let mut used = [false; 6];
    let mut two = false;
    let (ang, side): ([f64; 3], Option<[f64; 3]>) = if angs.len() >= 2 {
        let (i, j) = (angs[0], angs[1]);
        let k = 3 - i - j;
        let mut a = [0.0; 3];
        a[i] = v(i);
        a[j] = v(j);
        a[k] = 180.0 - a[i] - a[j];
        for &x in &angs { used[x] = true; }
        let side = sides.first().map(|&s| {
            used[s] = true;
            let kk = v(s) / sd(a[OPP_ANG[s - 3]]);
            [kk * sd(a[2]), kk * sd(a[0]), kk * sd(a[1])]
        });
        (a, side)
    } else if sides.len() == 3 {
        let (c, a, b) = (v(3), v(4), v(5));
        let arr = [(3usize, c), (4, a), (5, b)];
        let (li, lv) = arr.iter().copied().fold((3usize, f64::MIN), |m, x| if x.1 > m.1 { x } else { m });
        let rest: f64 = arr.iter().filter(|x| x.0 != li).map(|x| x.1).sum();
        if lv >= rest - 1e-12 { return Err(TriErr::Ineq(li)); }
        for i in 3..6 { used[i] = true; }
        let aa = acosd((b * b + c * c - a * a) / (2.0 * b * c));
        let bb = acosd((a * a + c * c - b * b) / (2.0 * a * c));
        ([aa, bb, 180.0 - aa - bb], Some([c, a, b]))
    } else {
        // one angle, two sides
        let x = angs[0];
        let xs = OPP_SIDE[x];
        used[x] = true;
        for &s in &sides { used[s] = true; }
        let mut s3 = [0.0f64; 3]; // AB, BC, CA
        for &s in &sides { s3[s - 3] = v(s); }
        if g[xs].is_none() {
            // SAS: the angle lies between the two given sides
            let (p, q): (f64, f64) = { let o: Vec<f64> = sides.iter().map(|&s| v(s)).collect(); (o[0], o[1]) };
            let opp = (p * p + q * q - 2.0 * p * q * cd(v(x))).max(0.0).sqrt();
            s3[xs - 3] = opp;
            let (c, a, b) = (s3[0], s3[1], s3[2]);
            let aa = acosd((b * b + c * c - a * a) / (2.0 * b * c));
            let bb = acosd((a * a + c * c - b * b) / (2.0 * a * c));
            ([aa, bb, 180.0 - aa - bb], Some(s3))
        } else {
            let y = *sides.iter().find(|&&s| s != xs).unwrap();
            let (xa, xv, yv) = (v(x), v(xs), v(y));
            let ya = OPP_ANG[y - 3];
            let yang = if xa >= 90.0 - 1e-9 {
                if xv <= yv + 1e-9 { return Err(TriErr::Obtuse(x, (xa - 90.0).abs() < 1e-9)); }
                (yv * sd(xa) / xv).clamp(-1.0, 1.0).asin().to_degrees()
            } else {
                let h = yv * sd(xa);
                if xv < h - 1e-9 { return Err(TriErr::Short(x, h)); }
                let y1 = (h / xv).clamp(-1.0, 1.0).asin().to_degrees();
                if xv >= yv - 1e-9 || (xv - h).abs() < 1e-9 { y1 } else { two = true; if other { 180.0 - y1 } else { y1 } }
            };
            let mut a = [0.0; 3];
            a[x] = xa;
            a[ya] = yang;
            let za = 3 - x - ya;
            a[za] = 180.0 - xa - yang;
            let kk = xv / sd(xa);
            (a, Some([kk * sd(a[2]), kk * sd(a[0]), kk * sd(a[1])]))
        }
    };
    for i in 0..6 {
        let Some((gv, tol)) = g[i] else { continue };
        if used[i] { continue; }
        let c = if i < 3 { Some(ang[i]) } else { side.map(|s| s[i - 3]) };
        if let Some(c) = c { if (c - gv).abs() > tol { return Err(TriErr::Mismatch(i, c)); } }
    }
    Ok(TriOut { ang, side, two })
}

pub(crate) const TRI_V: [&str; 6] = ["m_a", "m_b", "m_c", "m_ab", "m_bc", "m_ca"];
pub(crate) const TRI_LAB: [&str; 6] = ["a_lab", "b_lab", "c_lab", "ab_lab", "bc_lab", "ca_lab"];
pub(crate) const TRI_TXT: [&str; 6] = ["a_label", "b_label", "c_label", "ab_label", "bc_label", "ca_label"];
pub(crate) const TRI_MARK: [&str; 6] = ["a_arcs", "b_arcs", "c_arcs", "ab_ticks", "bc_ticks", "ca_ticks"];

#[derive(Debug)]
pub(crate) struct TriSolved { pub typed: Vec<Option<Typed>>, pub out: TriOut, pub pts: [Pt; 3] }

fn tri_names(p: &Params) -> [String; 6] {
    let n = corner_names(p, 3);
    [format!("\u{2220}{}", n[0]), format!("\u{2220}{}", n[1]), format!("\u{2220}{}", n[2]), format!("{}{}", n[0], n[1]), format!("{}{}", n[1], n[2]), format!("{}{}", n[2], n[0])]
}

pub(crate) fn tri_solve(p: &Params) -> Result<TriSolved, String> {
    let names = tri_names(p);
    let mut typed = Vec::new();
    for k in TRI_V { typed.push(typed_of(p, k)?); }
    for i in 0..6 { range_check(&names[i], &typed[i], i < 3)?; }
    let g: [Option<(f64, f64)>; 6] = std::array::from_fn(|i| typed[i].as_ref().map(|t| (t.v, t.tol)));
    let out = tri_core(&g, p.b("other")).map_err(|e| match e {
        TriErr::Few(n) => few(n, "triangle"),
        TriErr::Sum2(i, j, s) => format!("{} + {} is already {}\u{b0}, and a triangle's angles add up to 180\u{b0}.", names[i], names[j], fmt(s, 2)),
        TriErr::Sum3(s) => format!("{} + {} + {} is {}\u{b0}, not 180\u{b0}.", names[0], names[1], names[2], fmt(s, 2)),
        TriErr::Ineq(l) => {
            let o: Vec<&String> = (3..6).filter(|i| *i != l).map(|i| &names[i]).collect();
            format!("These sides can't make a triangle: {} + {} has to be longer than {}.", o[0], o[1], names[l])
        }
        TriErr::Obtuse(x, right) => format!("{} has to be the longest side, since {} is {}.", names[OPP_SIDE[x]], names[x], if right { "a right angle" } else { "obtuse" }),
        TriErr::Short(x, m) => format!("{} is too short to close the triangle: with these measures it has to be at least {}.", names[OPP_SIDE[x]], fmt(m, 2)),
        TriErr::Mismatch(i, c) => mismatch(&names[i], c, typed[i].as_ref().unwrap(), i < 3),
    })?;
    let c = out.side.map(|s| s[0]).unwrap_or(10.0);
    let (a, b, cc) = (out.ang[0].to_radians(), out.ang[1].to_radians(), out.ang[2].to_radians());
    let ca = c * b.sin() / cc.sin();
    let pts = [(0.0, 0.0), (c, 0.0), (ca * a.cos(), ca * a.sin())];
    Ok(TriSolved { typed, out, pts })
}

/// Worked-out value of each triangle row (angles, then AB, BC, CA; sides unknown for three angles).
pub(crate) fn tri_values(s: &TriSolved) -> [Option<f64>; 6] {
    let sd = s.out.side;
    [Some(s.out.ang[0]), Some(s.out.ang[1]), Some(s.out.ang[2]), sd.map(|x| x[0]), sd.map(|x| x[1]), sd.map(|x| x[2])]
}

/// The math page's section summaries for the flat shapes ("no unit · rounded to tenths · ...").
/// The coordinate grid's card summaries, worded like the reference's ("0 to 15 · by 1 ·
/// numbered · "x" · triangle arrowheads", "None", "No minor gridlines").
fn grid_skin_summary(title: &str, p: &Params) -> Option<String> {
    let num = |v: f64| { let r = (v * 1000.0).round() / 1000.0; if r == r.trunc() { format!("{}", r as i64) } else { format!("{r}") } };
    let axis = |lo: &str, hi: &str, step: &str, numk: &str, lab: &str| {
        let mut parts = vec![format!("{} to {}", num(p.f(lo)), num(p.f(hi))), format!("by {}", num(p.f(step)))];
        parts.push(if p.s(numk) == "None" { "no numbers".into() } else { "numbered".into() });
        let l = p.s(lab).trim();
        if !l.is_empty() { parts.push(format!("\u{201c}{l}\u{201d}")); }
        if p.b("arrows") { parts.push("triangle arrowheads".into()); }
        parts.join(" \u{b7} ")
    };
    Some(match title {
        "Titles" => {
            let mut parts: Vec<String> = Vec::new();
            if p.s("title_mode") == "Text" && !p.s("title").trim().is_empty() { parts.push(format!("\u{201c}{}\u{201d}", p.s("title").trim())); }
            if p.s("xtitle_mode") == "Text" && !p.s("xtitle").trim().is_empty() { parts.push(format!("x: {}", p.s("xtitle").trim())); }
            if p.s("ytitle_mode") == "Text" && !p.s("ytitle").trim().is_empty() { parts.push(format!("y: {}", p.s("ytitle").trim())); }
            if parts.is_empty() { "None".into() } else { parts.join(" \u{b7} ") }
        }
        "x-axis" => axis("xmin", "xmax", "xstep", "xnum", "xlabel"),
        "y-axis" => axis("ymin", "ymax", "ystep", "ynum", "ylabel"),
        "Grid" => {
            let m = p.i("minor");
            let mut parts = vec![if m <= 0 { "No minor gridlines".to_string() } else { format!("{m} minor per step") }];
            if p.s("grid") != "Standard" { parts.push(p.s("grid").to_lowercase()); }
            parts.join(" \u{b7} ")
        }
        _ => return None,
    })
}

pub(crate) fn flat_skin_summary(id: &str, title: &str, p: &Params) -> Option<String> {
    if id == "coordinate-grid" {
        return grid_skin_summary(title, p);
    }
    if !matches!(id, "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" | "regular-polygon") {
        return None;
    }
    match title {
        "Labels" => {
            let unit = p.s("unit").trim();
            let mut parts = vec![if unit.is_empty() { "no unit".to_string() } else { format!("in {unit}") }];
            parts.push(format!("rounded to {}", match p.i("dec") { 0 => "whole numbers", 1 => "tenths", 2 => "hundredths", _ => "thousandths" }));
            if p.b("right") { parts.push("right-angle marks".into()); }
            Some(parts.join(" \u{b7} "))
        }
        "Heights" if id == "triangle" => Some(p.s("height").to_string()),
        "Position" => {
            let b = p.s("bottom").trim();
            let mut parts = vec![if b.is_empty() || b.starts_with("None") { "standing upright".to_string() } else { format!("{} at the bottom", b.split(',').next().unwrap_or(b)) }];
            if p.f("rotate") != 0.0 { parts.push(format!("turned {}\u{b0}", p.f("rotate"))); }
            if p.b("flip") { parts.push("flipped".into()); }
            Some(parts.join(" \u{b7} "))
        }
        _ => None,
    }
}

fn shared_tail(v: &mut Vec<ParamDef>, right_default: bool, vertices: bool) {
    v.extend([
        ParamDef::text("unit", "Unit (e.g. cm)", ""),
        ParamDef::int("dec", "Round to (decimal places)", 0, 3, 1),
        ParamDef::bool("right", "Right-angle squares at 90\u{b0} angles and where heights meet a side", right_default),
        ParamDef::bool("vertices", "Corner letters", vertices),
        ParamDef::float("rotate", "Turn (degrees)", -180.0, 180.0, 1.0, 0.0),
        ParamDef::color("fill", "Fill colour", &["#e9f1fb", "#fdf0d5", "#e3f4e1", "#fbe3e3", "#efe5f7", "#ffffff"], "#e9f1fb"),
        ParamDef::bool("flip", "Flip (mirror left to right)", false),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        ParamDef::choice("line_style", "Construction lines (diagonals, heights)", &["Solid", "Dashed", "Dotted"], "Dashed"),
    ]);
}

fn label_params(v: &mut Vec<ParamDef>, labs: &[&'static str], txts: &[&'static str], marks: &[&'static str], n_angles_first: bool, angle_count: usize) {
    for i in 0..labs.len() {
        let angle = if n_angles_first { i < angle_count } else { i >= labs.len() - angle_count };
        v.push(ParamDef::choice(labs[i], "Label", &LAB_OPTS, "None"));
        v.push(ParamDef::text(txts[i], "Label text", "x"));
        v.push(ParamDef::choice(marks[i], if angle { "Congruence arcs" } else { "Congruence ticks" }, &TICKS, "None"));
    }
}

pub(crate) fn triangle_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::text("name_a", "Name of corner A", "A"),
        ParamDef::text("name_b", "Name of corner B", "B"),
        ParamDef::text("name_c", "Name of corner C", "C"),
        ParamDef::text("m_a", "Angle A (blank = worked out)", "24"),
        ParamDef::text("m_b", "Angle B (blank = worked out)", "90"),
        ParamDef::text("m_c", "Angle C (blank = worked out)", ""),
        ParamDef::text("m_ab", "Side AB (blank = worked out)", "12"),
        ParamDef::text("m_bc", "Side BC (blank = worked out)", ""),
        ParamDef::text("m_ca", "Side CA (blank = worked out)", ""),
    ];
    label_params(&mut v, &TRI_LAB, &TRI_TXT, &TRI_MARK, true, 3);
    for d in v.iter_mut() {
        match d.key { "a_lab" | "b_lab" | "ab_lab" => d.default = Value::S("Measure".into()), "bc_lab" => d.default = Value::S("Text".into()), _ => {} }
    }
    v.extend([
        ParamDef::bool("other", "Show the other triangle (when the measures make two)", false),
        ParamDef::choice("height", "Height", &["None", "From A", "From B", "From C", "From A and B", "From A and C", "From B and C", "From all three"], "None"),
        ParamDef::choice("bottom", "Side at the bottom", &["AB", "BC", "CA"], "AB"),
    ]);
    shared_tail(&mut v, true, true);
    v.extend(height_common());
    v
}

fn fill_of(p: &Params) -> String { if p.s("fill").starts_with('#') { p.s("fill").to_string() } else { TINT.to_string() } }

pub(crate) fn triangle_render(p: &Params) -> String {
    let s = match tri_solve(p) { Ok(s) => s, Err(e) => return note_fig(&e) };
    let [a, b, c] = s.pts;
    let (dec, unit) = (p.i("dec"), p.s("unit").to_string());
    let vals = tri_values(&s);
    let lab = |i: usize| lab_text(p, TRI_LAB[i], TRI_TXT[i], s.typed[i].as_ref(), vals[i], false, i < 3, dec, &unit);
    let (mut ex, mut mk) = (vec![], vec![]);
    let hs = p.s("height");
    let has = |v: &str| hs == "From all three" || hs.split(|ch: char| !ch.is_alphanumeric()).any(|t| t == v);
    for (on, v, q1, q2) in [(has("A"), a, b, c), (has("B"), b, c, a), (has("C"), c, a, b)] {
        if !on { continue; }
        let (e, m, _) = height_from(v, q1, q2, height_label(p), dash_of(p));
        ex.extend(e);
        mk.extend(m);
    }
    draw_poly(PolySpec {
        pts: vec![a, b, c],
        names: corner_names(p, 3),
        side_txt: (3..6).map(lab).collect(),
        ang_txt: (0..3).map(lab).collect(),
        ticks: (3..6).map(|i| tick_n(p, TRI_MARK[i])).collect(),
        arrows: vec![0; 3],
        arcs: (0..3).map(|i| tick_n(p, TRI_MARK[i])).collect(),
        right_marks: p.b("right"),
        vlabels: p.b("vertices"),
        extras: ex, marks: mk, dots: vec![], notes: vec![],
        rot: p.f("rotate"), flip: p.b("flip"), bottom: bottom_side(p, 3), fill: fill_of(p), fs: fscale(p),
    }, dec, &unit)
}

// ----- quadrilaterals -----

pub(crate) const Q_V: [&str; 8] = ["m_ab", "m_bc", "m_cd", "m_da", "m_a", "m_b", "m_c", "m_d"];
pub(crate) const Q_LAB: [&str; 8] = ["ab_lab", "bc_lab", "cd_lab", "da_lab", "a_lab", "b_lab", "c_lab", "d_lab"];
pub(crate) const Q_TXT: [&str; 8] = ["ab_label", "bc_label", "cd_label", "da_label", "a_label", "b_label", "c_label", "d_label"];
pub(crate) const Q_MARK: [&str; 8] = ["ab_ticks", "bc_ticks", "cd_ticks", "da_ticks", "a_arcs", "b_arcs", "c_arcs", "d_arcs"];
/// Parallel arrows per side (the label popover's second mark): "Auto" keeps the kind's own arrows
/// (or the older `arrow_marks` list), anything else sets that side.
pub(crate) const Q_ARROW: [&str; 4] = ["ab_arrows", "bc_arrows", "cd_arrows", "da_arrows"];
/// Value keys of the card's "Parallel arrows" rows (linked rows: not params, text from `computed`).
pub(crate) const Q_PAR: [&str; 4] = ["ab_par", "bc_par", "cd_par", "da_par"];
const ARROW_OPTS: [&str; 5] = ["Auto", "None", "1", "2", "3"];

/// Which of the eight rows (AB BC CD DA ∠A ∠B ∠C ∠D) have a value box in each quadrilateral;
/// the others are linked ("CD = AB") or fixed (a rectangle's 90°).
pub(crate) fn q_inputs(id: &str) -> [bool; 8] {
    match id {
        "rectangle" => [true, true, false, false, false, false, false, false],
        "parallelogram" => [true, false, false, true, true, true, false, false],
        "kite" => [true, false, false, true, true, true, false, true],
        _ => [true; 8],
    }
}

#[derive(Debug)]
pub(crate) struct QSol { pub pts: [Pt; 4], pub vals: [Option<f64>; 8], pub h: Option<f64>, pub typed: Vec<Option<Typed>> }

fn q_names(p: &Params) -> [String; 8] {
    let n = corner_names(p, 4);
    std::array::from_fn(|i| if i < 4 { format!("{}{}", n[i], n[(i + 1) % 4]) } else { format!("\u{2220}{}", n[i - 4]) })
}

fn q_typed(p: &Params, id: &str) -> Result<Vec<Option<Typed>>, String> {
    let inp = q_inputs(id);
    let names = q_names(p);
    let mut t = Vec::new();
    for i in 0..8 {
        let v = if inp[i] { typed_of(p, Q_V[i])? } else { None };
        range_check(&names[i], &v, i >= 4)?;
        t.push(v);
    }
    Ok(t)
}

/// Checks every given row against the solution.
fn q_check(names: &[String; 8], typed: &[Option<Typed>], vals: &[Option<f64>; 8]) -> Result<(), String> {
    for i in 0..8 {
        if let (Some(t), Some(v)) = (&typed[i], vals[i]) {
            if (t.v - v).abs() > t.tol { return Err(mismatch(&names[i], v, t, i >= 4)); }
        }
    }
    Ok(())
}

fn quad_vals(pts: &[Pt; 4]) -> [Option<f64>; 8] {
    let ang = poly_angles(pts);
    std::array::from_fn(|i| Some(if i < 4 { dist(pts[i], pts[(i + 1) % 4]) } else { ang[i - 4] }))
}

pub(crate) fn rect_solve(p: &Params) -> Result<QSol, String> {
    let names = q_names(p);
    let typed = q_typed(p, "rectangle")?;
    let sq = p.s("kind") == "Square";
    let w = match (&typed[0], &typed[1]) {
        (Some(a), _) => a.v,
        (None, Some(b)) if sq => b.v,
        _ => return Err(format!("Give {} a length.", names[0])),
    };
    let h = if sq { w } else { typed[1].as_ref().map(|t| t.v).ok_or(format!("Give {} a length.", names[1]))? };
    let pts = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];
    let vals = quad_vals(&pts);
    q_check(&names, &typed, &vals)?;
    Ok(QSol { pts, vals, h: None, typed })
}

pub(crate) fn para_solve(p: &Params) -> Result<QSol, String> {
    let names = q_names(p);
    let typed = q_typed(p, "parallelogram")?;
    let rh = p.s("kind") == "Rhombus";
    let ab = match (&typed[0], &typed[3]) {
        (Some(a), _) => a.v,
        (None, Some(d)) if rh => d.v,
        _ => return Err(format!("Give {} a length.", names[0])),
    };
    let da = if rh { ab } else { typed[3].as_ref().map(|t| t.v).ok_or(format!("Give {} a length.", names[3]))? };
    let a = match (&typed[4], &typed[5]) {
        (Some(a), _) => a.v,
        (None, Some(b)) => 180.0 - b.v,
        _ => return Err(format!("Give {} in degrees.", names[4])),
    };
    let t = a.to_radians();
    let d = (da * t.cos(), da * t.sin());
    let pts = [(0.0, 0.0), (ab, 0.0), (ab + d.0, d.1), d];
    let vals = quad_vals(&pts);
    q_check(&names, &typed, &vals)?;
    Ok(QSol { pts, vals, h: None, typed })
}

/// Kite ABCD standing on its axis BD: triangle ABD has ∠A, ½∠B, ½∠D and sides AB, DA, so any
/// three of AB, DA, ∠A, ∠B, ∠D that fix that triangle fix the kite.
pub(crate) fn kite_solve(p: &Params) -> Result<QSol, String> {
    let names = q_names(p);
    let typed = q_typed(p, "kite")?;
    let gv = |i: usize, half: bool| typed[i].as_ref().map(|t| if half { (t.v / 2.0, t.tol / 2.0) } else { (t.v, t.tol) });
    let g = [gv(4, false), gv(5, true), gv(7, true), gv(0, false), None, gv(3, false)];
    let out = tri_core(&g, false).map_err(|e| match e {
        TriErr::Few(n) => few(n, "kite"),
        TriErr::Sum2(..) | TriErr::Sum3(_) => format!("These angles can't make a kite: {} + \u{bd}{} + \u{bd}{} has to be 180\u{b0}.", names[4], names[5], names[7]),
        TriErr::Short(x, m) => format!("{} is too short to reach: with these measures it has to be longer than {}.", if x == 1 { &names[3] } else { &names[0] }, fmt(m, 2)),
        TriErr::Obtuse(..) | TriErr::Ineq(_) => "These measures don't make a kite.".to_string(),
        TriErr::Mismatch(i, c) => {
            let (row, v) = match i { 0 => (4, c), 1 => (5, 2.0 * c), 2 => (7, 2.0 * c), 3 => (0, c), _ => (3, c) };
            mismatch(&names[row], v, typed[row].as_ref().unwrap(), row >= 4)
        }
    })?;
    let (ab, bd) = match out.side { Some(s) => (s[0], s[1]), None => {
        // three angles only: shape without size
        let ab = 5.0;
        let half_d = out.ang[2].to_radians();
        (ab, ab * out.ang[0].to_radians().sin() / half_d.sin())
    } };
    let hb = out.ang[1].to_radians();
    let (b, d) = ((0.0, 0.0), (0.0, -bd));
    let a = (-ab * hb.sin(), -ab * hb.cos());
    let c = (-a.0, a.1);
    let pts = [a, b, c, d];
    let mut vals = quad_vals(&pts);
    if out.side.is_none() { for v in vals.iter_mut().take(4) { *v = None; } }
    Ok(QSol { pts, vals, h: None, typed })
}

/// Base-angle right triangle (height h, run along the base, slanted leg, angle at the base): any
/// two fix the rest. Err when the leg is shorter than the height or the run.
fn rt(h: &mut Option<f64>, run: &mut Option<f64>, leg: &mut Option<f64>, ang: &mut Option<f64>) -> Result<(), ()> {
    let n = [h.is_some(), run.is_some(), leg.is_some(), ang.is_some()].iter().filter(|x| **x).count();
    if !(2..4).contains(&n) { return Ok(()); }
    let (sd, cd) = (|d: f64| d.to_radians().sin(), |d: f64| d.to_radians().cos());
    if let (Some(a), Some(l)) = (*ang, *leg) {
        h.get_or_insert(l * sd(a));
        run.get_or_insert(l * cd(a));
    } else if let (Some(a), Some(hh)) = (*ang, *h) {
        leg.get_or_insert(hh / sd(a));
        run.get_or_insert(hh * cd(a) / sd(a));
    } else if let (Some(a), Some(r)) = (*ang, *run) {
        if cd(a).abs() < 1e-9 { return Ok(()); }
        leg.get_or_insert(r / cd(a));
        h.get_or_insert(r * sd(a) / cd(a));
    } else if let (Some(hh), Some(l)) = (*h, *leg) {
        if l < hh - 1e-9 { return Err(()); }
        let r = (l * l - hh * hh).max(0.0).sqrt();
        run.get_or_insert(r);
        ang.get_or_insert(hh.atan2(r).to_degrees());
    } else if let (Some(hh), Some(r)) = (*h, *run) {
        leg.get_or_insert(hh.hypot(r));
        ang.get_or_insert(hh.atan2(r).to_degrees());
    } else if let (Some(r), Some(l)) = (*run, *leg) {
        if l < r.abs() - 1e-9 { return Err(()); }
        let hh = (l * l - r * r).max(0.0).sqrt();
        h.get_or_insert(hh);
        ang.get_or_insert(hh.atan2(r).to_degrees());
    }
    Ok(())
}

/// Trapezoid with AB ∥ CD (AB at the bottom). Kinds: plain (needs four measures, e.g. AB, CD, ∠A
/// and h), isosceles (three: legs and base angles equal) and right (three: DA at right angles to
/// both bases). The left leg makes the triangle (h, offset of D, DA, ∠A), the right leg (h, run
/// of C, BC, ∠B), and AB = CD + offset + run.
pub(crate) fn trap_solve(p: &Params) -> Result<QSol, String> {
    let names = q_names(p);
    let typed = q_typed(p, "trapezoid")?;
    let ht = typed_of(p, "m_h")?;
    range_check("h", &ht, false)?;
    let kind = match p.s("kind") { "Isosceles trapezoid" => 1, "Right trapezoid" => 2, _ => 0 };
    let tv = |i: usize| typed[i].as_ref().map(|t| t.v);
    // ∠D = 180° − ∠A and ∠C = 180° − ∠B (the bases are parallel)
    let mut aa = tv(4).or(tv(7).map(|d| 180.0 - d));
    let mut bb = tv(5).or(tv(6).map(|c| 180.0 - c));
    let (mut ab, mut cd, mut h, mut da, mut bc) = (tv(0), tv(2), ht.as_ref().map(|t| t.v), tv(3), tv(1));
    let (mut off, mut run): (Option<f64>, Option<f64>) = (None, None);
    let count = [ab.is_some(), cd.is_some(), h.is_some(), da.is_some(), bc.is_some(), aa.is_some(), bb.is_some()].iter().filter(|x| **x).count();
    let need = if kind == 0 { 4 } else { 3 };
    if kind == 2 {
        if let Some(t) = typed[4].as_ref().or(typed[7].as_ref()) { if (t.v - 90.0).abs() > t.tol { return Err(mismatch(if typed[4].is_some() { &names[4] } else { &names[7] }, 90.0, t, true)); } }
        off = Some(0.0);
        aa = Some(90.0);
    }
    let effective = if kind == 2 { count - usize::from(tv(4).is_some() || tv(7).is_some()) } else { count };
    if effective < need { return Err(few(need - effective, "trapezoid")); }
    let short = |leg: &str| format!("{leg} is too short to reach between the bases.");
    for _ in 0..12 {
        rt(&mut h, &mut off, &mut da, &mut aa).map_err(|_| short(&names[3]))?;
        rt(&mut h, &mut run, &mut bc, &mut bb).map_err(|_| short(&names[1]))?;
        if kind == 1 {
            if off.is_none() { off = run; } else if run.is_none() { run = off; }
            if da.is_none() { da = bc; } else if bc.is_none() { bc = da; }
            if aa.is_none() { aa = bb; } else if bb.is_none() { bb = aa; }
            if let (Some(a), Some(c), None) = (ab, cd, off) { off = Some((a - c) / 2.0); run = off; }
            if let (Some(o), None, Some(c)) = (off, ab, cd) { ab = Some(c + 2.0 * o); }
            if let (Some(o), Some(a), None) = (off, ab, cd) { cd = Some(a - 2.0 * o); }
        }
        match (ab, cd, off, run) {
            (None, Some(c), Some(o), Some(r)) => ab = Some(c + o + r),
            (Some(a), None, Some(o), Some(r)) => cd = Some(a - o - r),
            (Some(a), Some(c), None, Some(r)) => off = Some(a - c - r),
            (Some(a), Some(c), Some(o), None) => run = Some(a - c - o),
            _ => {}
        }
    }
    // plain trapezoid with AB, CD and one measure on each leg: find h numerically
    if h.is_none() && kind == 0 {
        if let (Some(a), Some(c)) = (ab, cd) {
            let side = |hh: f64, aa: Option<f64>, leg: Option<f64>| -> Option<f64> {
                if let Some(t) = aa { return Some(hh / t.to_radians().tan()); }
                leg.filter(|l| *l >= hh).map(|l| (l * l - hh * hh).sqrt())
            };
            let f = |hh: f64| Some(side(hh, aa, da)? + side(hh, bb, bc)? - (a - c));
            let hmax = [da, bc].iter().flatten().fold(10.0 * (a + c).abs().max(1.0), |m, x| m.min(*x));
            let mut lo = None;
            let steps = 400;
            let mut prev: Option<(f64, f64)> = None;
            for k in 1..=steps {
                let x = hmax * k as f64 / steps as f64;
                if let Some(fx) = f(x) {
                    if let Some((px, pf)) = prev { if pf.signum() != fx.signum() || fx == 0.0 { lo = Some((px, x)); break; } }
                    prev = Some((x, fx));
                }
            }
            if let Some((mut l, mut r)) = lo {
                for _ in 0..80 {
                    let m = (l + r) / 2.0;
                    match (f(l), f(m)) { (Some(fl), Some(fm)) if fl.signum() == fm.signum() => l = m, _ => r = m }
                }
                h = Some((l + r) / 2.0);
                for _ in 0..3 {
                    rt(&mut h, &mut off, &mut da, &mut aa).map_err(|_| short(&names[3]))?;
                    rt(&mut h, &mut run, &mut bc, &mut bb).map_err(|_| short(&names[1]))?;
                }
            }
        }
    }
    let (Some(ab), Some(cd), Some(h), Some(off)) = (ab, cd, h, off) else { return Err("These measures don't fix the trapezoid.".to_string()) };
    if cd <= 1e-9 { return Err(format!("{} has to be longer than 0: these measures leave no room for it.", names[2])); }
    if ab <= 1e-9 { return Err(format!("{} has to be longer than 0: these measures leave no room for it.", names[0])); }
    let pts = [(0.0, 0.0), (ab, 0.0), (off + cd, h), (off, h)];
    let vals = quad_vals(&pts);
    q_check(&names, &typed, &vals)?;
    if let Some(t) = &ht { if (t.v - h).abs() > t.tol { return Err(mismatch("h", h, t, false)); } }
    Ok(QSol { pts, vals, h: Some(h), typed })
}

pub(crate) fn quad_solve(id: &str, p: &Params) -> Result<QSol, String> {
    match id {
        "rectangle" => rect_solve(p),
        "parallelogram" => para_solve(p),
        "kite" => kite_solve(p),
        _ => trap_solve(p),
    }
}

/// Rows of each quadrilateral that are linked to another row (no value box, never "given").
fn q_linked(id: &str) -> [bool; 8] {
    let inp = q_inputs(id);
    std::array::from_fn(|i| !inp[i])
}

fn quad_params(id: &str, vals: &[(&'static str, &'static str)]) -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::text("name_a", "Name of corner A", "A"),
        ParamDef::text("name_b", "Name of corner B", "B"),
        ParamDef::text("name_c", "Name of corner C", "C"),
        ParamDef::text("name_d", "Name of corner D", "D"),
    ];
    let inp = q_inputs(id);
    for (i, &k) in Q_V.iter().enumerate() {
        if !inp[i] { continue; }
        let d = vals.iter().find(|x| x.0 == k).map(|x| x.1).unwrap_or("");
        let label = ["Side AB", "Side BC", "Side CD", "Side DA", "Angle A", "Angle B", "Angle C", "Angle D"][i];
        v.push(ParamDef { key: k, label, hint: "", kind: Kind::Text, default: Value::S(d.into()) });
    }
    label_params(&mut v, &Q_LAB, &Q_TXT, &Q_MARK, false, 4);
    for (k, d) in vals {
        if let Some(pd) = v.iter_mut().find(|pd| pd.key == *k) { pd.default = Value::S(d.to_string()); }
    }
    v
}

fn diag_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("diagonals", "Diagonals", &["None", "Drawn", "Drawn and labelled", "AC only", "BD only"], "None"),
        ParamDef::choice("ac_lab", "Diagonal AC label", &LAB_OPTS, "None"),
        ParamDef::text("ac_label", "Diagonal AC label text", "d"),
        ParamDef::choice("bd_lab", "Diagonal BD label", &LAB_OPTS, "None"),
        ParamDef::text("bd_label", "Diagonal BD label text", "e"),
        ParamDef::list("arrow_marks", "Parallel arrows per side, e.g. AB 1, CD 1 (or none; blank = the figure's default)", ""),
        ParamDef::choice("ab_arrows", "Parallel arrows on AB", &ARROW_OPTS, "Auto"),
        ParamDef::choice("bc_arrows", "Parallel arrows on BC", &ARROW_OPTS, "Auto"),
        ParamDef::choice("cd_arrows", "Parallel arrows on CD", &ARROW_OPTS, "Auto"),
        ParamDef::choice("da_arrows", "Parallel arrows on DA", &ARROW_OPTS, "Auto"),
    ]
}

/// Arrows drawn on each side: the per-side choice, else the `arrow_marks` list, else the kind's own.
pub(crate) fn quad_arrows(p: &Params, default_arrows: &[u8]) -> Vec<u8> {
    let names = corner_names(p, 4);
    let base = parse_counts(p.s("arrow_marks"), &names, true).unwrap_or_else(|| default_arrows.to_vec());
    (0..4).map(|i| match p.s(Q_ARROW[i]) { "None" => 0, "1" => 1, "2" => 2, "3" => 3, _ => base.get(i).copied().unwrap_or(0) }).collect()
}
/// Each kind's own parallel arrows (drawn while a side is on "Auto").
pub(crate) fn quad_default_arrows(id: &str) -> Vec<u8> {
    match id { "parallelogram" => vec![1, 2, 1, 2], "trapezoid" => vec![1, 0, 1, 0], _ => vec![0; 4] }
}

/// Diagonals AC / BD with their own label settings ("Drawn and labelled" forces the measure) and a
/// right-angle mark where they cross at 90°.
fn diagonals(p: &Params, pts: &[Pt], ex: &mut Vec<Extra>, mk: &mut Vec<Mark>) {
    let sel = p.s("diagonals");
    let (ac, bd) = match sel { "" | "None" => (false, false), "AC only" => (true, false), "BD only" => (false, true), _ => (true, true) };
    let forced = sel.contains("labelled");
    let dash = dash_of(p);
    for (on, i, j, t, lab, txt) in [(ac, 0usize, 2usize, 0.3, "ac_lab", "ac_label"), (bd, 1, 3, 0.72, "bd_lab", "bd_label")] {
        if !on { continue; }
        let label = if forced { Some("=".to_string()) } else {
            match p.s(lab) {
                "Measure" => Some("=".to_string()),
                "Text" => Some({ let t = p.s(txt).trim(); if t.is_empty() { "x".to_string() } else { t.to_string() } }),
                _ => None,
            }
        };
        ex.push(Extra { a: pts[i], b: pts[j], label, t, dash, raw: true, id: if i == 0 { "ac" } else { "bd" } });
    }
    if ac && bd {
        let (u, w) = (unit_v(pts[0], pts[2]), unit_v(pts[1], pts[3]));
        if (u.0 * w.0 + u.1 * w.1).abs() < 0.005 {
            let den = u.0 * w.1 - u.1 * w.0;
            let s = ((pts[1].0 - pts[0].0) * w.1 - (pts[1].1 - pts[0].1) * w.0) / den;
            let x = (pts[0].0 + u.0 * s, pts[0].1 + u.1 * s);
            mk.push((x, pts[2], pts[1]));
        }
    }
}

fn quad_draw(id: &str, p: &Params, s: &QSol, default_arrows: Vec<u8>, mut ex: Vec<Extra>, mut mk: Vec<Mark>) -> String {
    let (dec, unit) = (p.i("dec"), p.s("unit").to_string());
    let linked = q_linked(id);
    let lab = |i: usize| lab_text(p, Q_LAB[i], Q_TXT[i], s.typed[i].as_ref(), s.vals[i], linked[i], i >= 4, dec, &unit);
    let pts = s.pts.to_vec();
    diagonals(p, &pts, &mut ex, &mut mk);
    let names = corner_names(p, 4);
    let arrows = quad_arrows(p, &default_arrows);
    draw_poly(PolySpec {
        pts,
        names,
        side_txt: (0..4).map(lab).collect(),
        ang_txt: (4..8).map(lab).collect(),
        ticks: (0..4).map(|i| tick_n(p, Q_MARK[i])).collect(),
        arrows,
        arcs: (4..8).map(|i| tick_n(p, Q_MARK[i])).collect(),
        right_marks: p.b("right"),
        vlabels: p.b("vertices"),
        extras: ex, marks: mk, dots: vec![], notes: vec![],
        rot: p.f("rotate"), flip: p.b("flip"), bottom: bottom_side(p, 4), fill: fill_of(p), fs: fscale(p),
    }, dec, &unit)
}

const QUAD_BOTTOM2: [&str; 5] = ["AB, as it stands", "AB", "BC", "CD", "DA"];

pub(crate) fn rectangle_params() -> Vec<ParamDef> {
    let mut v = vec![ParamDef::choice("kind", "Kind", &["Rectangle", "Square"], "Rectangle")];
    v.extend(quad_params("rectangle", &[("m_ab", "10"), ("m_bc", "6"), ("ab_lab", "Measure"), ("bc_lab", "Measure")]));
    v.extend(diag_params());
    v.push(ParamDef::choice("bottom", "Side at the bottom", &QUAD_BOTTOM2, "AB, as it stands"));
    shared_tail(&mut v, true, true);
    v
}

pub(crate) fn rectangle_render(p: &Params) -> String {
    match rect_solve(p) { Ok(s) => quad_draw("rectangle", p, &s, vec![0; 4], vec![], vec![]), Err(e) => note_fig(&e) }
}

fn heights_of(p: &Params, list: Vec<(Pt, Pt, Pt)>) -> (Vec<Extra>, Vec<Mark>) {
    let (mut ex, mut mk) = (vec![], vec![]);
    for (v, q1, q2) in list {
        let (e, m, _) = height_from(v, q1, q2, height_label(p), dash_of(p));
        ex.extend(e);
        mk.extend(m);
    }
    (ex, mk)
}

pub(crate) fn parallelogram_params() -> Vec<ParamDef> {
    let mut v = vec![ParamDef::choice("kind", "Kind", &["Parallelogram", "Rhombus"], "Parallelogram")];
    v.extend(quad_params("parallelogram", &[("m_ab", "10"), ("m_da", "6"), ("m_a", "60"), ("ab_lab", "Measure"), ("da_lab", "Measure"), ("a_lab", "Measure")]));
    v.push(ParamDef::choice("height", "Height", &["None", "Onto AB (from D)", "Onto AD (from B)", "Onto AB (from C)", "Onto AB (from D and C)"], "None"));
    v.extend(diag_params());
    v.push(ParamDef::choice("bottom", "Side at the bottom", &QUAD_BOTTOM2, "AB, as it stands"));
    shared_tail(&mut v, true, true);
    v.extend(height_common());
    v
}

pub(crate) fn parallelogram_render(p: &Params) -> String {
    let s = match para_solve(p) { Ok(s) => s, Err(e) => return note_fig(&e) };
    let [a, b, c, d] = s.pts;
    let hs: Vec<(Pt, Pt, Pt)> = match p.s("height") {
        "Onto AB (from D)" => vec![(d, a, b)],
        "Onto AD (from B)" => vec![(b, a, d)],
        "Onto AB (from C)" => vec![(c, a, b)],
        "Onto AB (from D and C)" => vec![(d, a, b), (c, a, b)],
        _ => vec![],
    };
    let (ex, mk) = heights_of(p, hs);
    quad_draw("parallelogram", p, &s, vec![1, 2, 1, 2], ex, mk)
}

pub(crate) fn trapezoid_params() -> Vec<ParamDef> {
    let mut v = vec![ParamDef::choice("kind", "Kind", &["Trapezoid", "Isosceles trapezoid", "Right trapezoid"], "Right trapezoid")];
    v.extend(quad_params("trapezoid", &[("m_ab", "12"), ("m_cd", "7"), ("m_da", "5"), ("ab_lab", "Measure"), ("cd_lab", "Measure"), ("da_lab", "Measure"), ("bc_lab", "Text")]));
    v.push(ParamDef::text("m_h", "Height between the bases (blank = worked out)", ""));
    v.extend([
        ParamDef::bool("show_height", "Draw the height", false),
        ParamDef::choice("hfrom", "Height drawn from", &["D", "C", "D and C"], "D"),
    ]);
    v.extend(diag_params());
    v.push(ParamDef::choice("bottom", "Side at the bottom", &QUAD_BOTTOM2, "AB, as it stands"));
    shared_tail(&mut v, true, true);
    v.extend(height_common());
    v
}

pub(crate) fn trapezoid_render(p: &Params) -> String {
    let s = match trap_solve(p) { Ok(s) => s, Err(e) => return note_fig(&e) };
    let [a, b, c, d] = s.pts;
    let h = s.h.unwrap_or(d.1);
    let ab = b.0;
    let off = d.0;
    let (mut ex, mut mk) = (vec![], vec![]);
    if p.b("show_height") {
        let hf = p.s("hfrom");
        let (from_d, from_c) = (hf != "C", hf == "C" || hf == "D and C");
        let (lab, dash) = (height_label(p), dash_of(p));
        if from_d && off.abs() > 1e-9 {
            let (e, m, _) = height_from(d, a, b, lab.clone(), dash);
            ex.extend(e);
            mk.extend(m);
        } else if from_d {
            ex.push(Extra { a: (ab / 2.0, 0.0), b: (ab / 2.0, h), label: lab.0.clone(), t: 0.5, dash, raw: lab.1, id: "h" });
            mk.push(((ab / 2.0, 0.0), (ab / 2.0, h), (ab, 0.0)));
        }
        if from_c && (c.0 - ab).abs() > 1e-9 {
            let (e, m, _) = height_from(c, a, b, lab, dash);
            ex.extend(e);
            mk.extend(m);
        }
    }
    quad_draw("trapezoid", p, &s, vec![1, 0, 1, 0], ex, mk)
}

pub(crate) fn kite_params() -> Vec<ParamDef> {
    let mut v = quad_params("kite", &[("m_ab", "5"), ("m_da", "9"), ("m_b", "100"), ("ab_lab", "Measure"), ("da_lab", "Measure"), ("b_lab", "Measure"), ("ab_ticks", "1"), ("bc_ticks", "1"), ("cd_ticks", "2"), ("da_ticks", "2")]);
    v.push(ParamDef::choice("height", "Height", &["None", "From D to AB", "From C to AB", "From D and C to AB"], "None"));
    v.extend(diag_params());
    v.push(ParamDef::choice("bottom", "Side at the bottom", &["None (standing upright)", "AB", "BC", "CD", "DA"], "None (standing upright)"));
    shared_tail(&mut v, true, true);
    v.extend(height_common());
    v
}

pub(crate) fn kite_render(p: &Params) -> String {
    let s = match kite_solve(p) { Ok(s) => s, Err(e) => return note_fig(&e) };
    let [a, b, c, d] = s.pts;
    let hs: Vec<(Pt, Pt, Pt)> = match p.s("height") { "From D to AB" => vec![(d, a, b)], "From C to AB" => vec![(c, a, b)], "From D and C to AB" => vec![(d, a, b), (c, a, b)], _ => vec![] };
    let (ex, mk) = heights_of(p, hs);
    quad_draw("kite", p, &s, vec![0; 4], ex, mk)
}

/// (value_key, text) for every row of a quadrilateral that is worked out or linked.
pub(crate) fn quad_computed(id: &str, p: &Params) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    // the "Parallel arrows" rows' buttons: the arrows each side gets
    let arr = quad_arrows(p, &quad_default_arrows(id));
    for i in 0..4 {
        let chev = "\u{203a}".repeat(arr[i] as usize);
        let auto = matches!(p.s(Q_ARROW[i]), "" | "Auto");
        out.push((Q_PAR[i], match (auto, arr[i]) { (true, 0) => "auto".to_string(), (true, _) => format!("{chev} auto"), _ => chev }));
    }
    // the solver's reason (too few / too many / inconsistent givens) shows under the card
    let s = match quad_solve(id, p) { Ok(s) => s, Err(e) => { out.push(("__msg", e)); return out; } };
    let dec = p.i("dec");
    let inp = q_inputs(id);
    for i in 0..8 {
        let Some(v) = s.vals[i] else { continue };
        if inp[i] && s.typed[i].is_some() { continue; }
        out.push((Q_V[i], fmt(v, dec)));
    }
    if id == "trapezoid" { if let Some(h) = s.h { out.push(("m_h", fmt(h, dec))); } }
    out
}

/// Writes worked-out measures from dragged geometry back into the rows that are given.
pub(crate) fn write_givens(p: &mut Params, keys: &[&str], vals: &[Option<f64>]) {
    for (k, v) in keys.iter().zip(vals) {
        if k.is_empty() || p.s(k).trim().is_empty() { continue; }
        if let Some(v) = v { if v.is_finite() && *v > 0.0 { p.set_s(k, &num_str(*v, 2)); } }
    }
}

pub(crate) fn quad_vals_of(pts: &[Pt; 4]) -> [Option<f64>; 8] { quad_vals(pts) }

// ----- regular polygon -----

pub(crate) fn regular_polygon_params() -> Vec<ParamDef> {
    vec![
        ParamDef::int("n", "Sides", 3, 20, 6),
        ParamDef::choice("from", "Size from", &["Side", "Radius", "Apothem"], "Side"),
        ParamDef::float("value", "Size", 0.5, 100.0, 0.5, 8.0),
        ParamDef::choice("side_lab", "Side label", &LAB_OPTS, "Measure"),
        ParamDef::text("side_label", "Side label text", "s"),
        ParamDef::choice("side_ticks", "Congruence ticks (every side)", &TICKS, "None"),
        ParamDef::choice("ang_lab", "Angle label", &LAB_OPTS, "None"),
        ParamDef::text("ang_label", "Angle label text", "x"),
        ParamDef::choice("ang_arcs", "Congruence arcs (every angle)", &TICKS, "None"),
        ParamDef::bool("center_dot", "Center dot", true),
        ParamDef::text("center_name", "Name the center (e.g. O)", ""),
        ParamDef::bool("apothem", "Apothem (center to the middle of the bottom side)", true),
        ParamDef::choice("apothem_label", "Apothem label", &["a = measure", "Measure", "Text", "None"], "Text"),
        ParamDef::text("apothem_text", "Apothem name / text", "a"),
        ParamDef::bool("radius", "Radius (center to the bottom right corner)", false),
        ParamDef::choice("radius_label", "Radius label", &["r = measure", "Measure", "Text", "None"], "Text"),
        ParamDef::text("radius_text", "Radius name / text", "r"),
        ParamDef::bool("all_radii", "All the radii (center to every corner)", false),
        ParamDef::bool("vertices", "Letter the corners A, B, C... from the bottom left", false),
        ParamDef::text("vnames", "Corner letters in order (e.g. P, Q, R; blank = A, B, C)", ""),
        ParamDef::text("unit", "Unit (e.g. cm)", ""),
        ParamDef::int("dec", "Round to (decimal places)", 0, 3, 1),
        ParamDef::bool("right", "Right-angle squares where the apothem meets its side", true),
        ParamDef::float("rotate", "Turn (degrees)", -180.0, 180.0, 1.0, 0.0),
        ParamDef::color("fill", "Fill colour", &["#e9f1fb", "#fdf0d5", "#e3f4e1", "#fbe3e3", "#efe5f7", "#ffffff"], "#e9f1fb"),
        ParamDef::bool("flip", "Flip (mirror left to right)", false),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        ParamDef::choice("line_style", "Construction lines (radius, apothem)", &["Solid", "Dashed", "Dotted"], "Dashed"),
    ]
}

/// (side, radius, apothem, interior angle, central angle) of the regular polygon.
pub(crate) fn rp_measures(p: &Params) -> (f64, f64, f64, f64, f64) {
    let n = p.i("n").clamp(3, 20) as f64;
    let v = p.f("value");
    let half = PI / n;
    let r = match p.s("from") { "Radius" => v, "Apothem" => v / half.cos(), _ => v / (2.0 * half.sin()) };
    (2.0 * r * half.sin(), r, r * half.cos(), (n - 2.0) * 180.0 / n, 360.0 / n)
}

pub(crate) fn regular_polygon_pts(p: &Params) -> Vec<Pt> {
    let n = p.i("n").clamp(3, 20) as usize;
    let (_, r, _, _, _) = rp_measures(p);
    let half = PI / n as f64;
    let phi0 = -PI / 2.0 - half;
    (0..n).map(|k| { let t = phi0 + 2.0 * PI * k as f64 / n as f64; (r * t.cos(), r * t.sin()) }).collect()
}

pub(crate) fn regular_polygon_render(p: &Params) -> String {
    let pts = regular_polygon_pts(p);
    let n = pts.len();
    let o = (0.0, 0.0);
    let dash = dash_of(p);
    let (dec, unit) = (p.i("dec"), p.s("unit").to_string());
    let (side, _, _, ang, _) = rp_measures(p);
    let (mut ex, mut mk, mut dots, mut notes) = (vec![], vec![], vec![], vec![]);
    if p.b("center_dot") { dots.push(o); }
    let rad_at = 1usize.min(n - 1); // bottom right corner
    if p.b("all_radii") {
        for (k, q) in pts.iter().enumerate() {
            if k == rad_at && p.b("radius") { continue; }
            ex.push(Extra { a: o, b: *q, t: 0.5, dash, ..Default::default() });
        }
    }
    if p.b("radius") {
        let (label, raw) = named_label(p.s("radius_label"), p.s("radius_text"), "r");
        ex.push(Extra { a: o, b: pts[rad_at], label, t: 0.5, dash, raw, id: "r" });
    }
    if p.b("apothem") {
        let mid = ((pts[0].0 + pts[1].0) / 2.0, (pts[0].1 + pts[1].1) / 2.0);
        let (label, raw) = named_label(p.s("apothem_label"), p.s("apothem_text"), "a");
        ex.push(Extra { a: o, b: mid, label, t: 0.5, dash, raw, id: "ap" });
        mk.push((mid, o, pts[1]));
    }
    let cname = p.s("center_name").trim();
    if !cname.is_empty() {
        let k = (n / 2) as f64;
        let t = -PI / 2.0 + 2.0 * PI * k / n as f64;
        notes.push((o, (t.cos(), t.sin()), cname.to_string()));
    }
    let given_side = p.s("from") == "Side";
    let side_mode = if p.s("side_lab").is_empty() { if given_side { "Measure" } else { "None" } } else { p.s("side_lab") };
    let side_txt = match side_mode {
        "None" => None,
        "Text" => Some({ let t = p.s("side_label").trim(); if t.is_empty() { "s".to_string() } else { t.to_string() } }),
        _ => Some(fmtu(side, dec, &unit)),
    };
    let ang_txt = match if p.s("ang_lab").is_empty() { "None" } else { p.s("ang_lab") } {
        "None" => None,
        "Text" => Some({ let t = p.s("ang_label").trim(); if t.is_empty() { "x".to_string() } else { t.to_string() } }),
        _ => Some(format!("{}\u{b0}", fmt(ang, dec))),
    };
    let names = vnames(p, n);
    let st = tick_n(p, "side_ticks");
    let sa = tick_n(p, "ang_arcs");
    draw_poly(PolySpec {
        pts, names,
        side_txt: (0..n).map(|i| if i == 0 { side_txt.clone() } else { None }).collect(),
        ang_txt: (0..n).map(|i| if i == 0 { ang_txt.clone() } else { None }).collect(),
        ticks: vec![st; n], arrows: vec![0; n], arcs: vec![sa; n],
        right_marks: p.b("right"), vlabels: p.b("vertices"),
        extras: ex, marks: mk, dots, notes,
        rot: p.f("rotate"), flip: p.b("flip"), bottom: None, fill: fill_of(p), fs: fscale(p),
    }, dec, &unit)
}

pub(crate) fn rp_computed(p: &Params) -> Vec<(&'static str, String)> {
    let dec = p.i("dec");
    let (side, r, a, ang, _) = rp_measures(p);
    vec![("m_side", fmt(side, dec)), ("m_angle", format!("{}\u{b0}", fmt(ang, dec))), ("m_apothem", fmt(a, dec)), ("m_radius", fmt(r, dec))]
}

// ----- angles (rays card) -----

pub(crate) const RAY_V: [&str; 6] = ["ray1", "ray2", "ray3", "ray4", "ray5", "ray6"];
pub(crate) const RAY_STYLE: [&str; 6] = ["ray1_style", "ray2_style", "ray3_style", "ray4_style", "ray5_style", "ray6_style"];
pub(crate) const RAY_LINE: [&str; 6] = ["ray1_line", "ray2_line", "ray3_line", "ray4_line", "ray5_line", "ray6_line"];

pub(crate) fn angles_params() -> Vec<ParamDef> {
    let mut v = vec![];
    for (i, &k) in RAY_V.iter().enumerate().skip(1) {
        v.push(ParamDef { key: k, label: ["", "Ray 2 direction", "Ray 3 direction", "Ray 4 direction", "Ray 5 direction", "Ray 6 direction"][i], hint: "", kind: Kind::Text, default: Value::S(if i == 1 { "60".into() } else { String::new() }) });
    }
    for i in 0..6 {
        v.push(ParamDef::choice(RAY_STYLE[i], "Line", &STYLES, "Solid"));
        v.push(ParamDef::choice(RAY_LINE[i], "Drawn as", &["Ray", "Line"], "Ray"));
    }
    v.extend([
        ParamDef::bool("around", "Angle back around to the baseline (angles around a point)", false),
        ParamDef::list("unknown", "Sector numbers shown as x (counterclockwise from baseline, e.g. 2)", ""),
        ParamDef::list("hide", "Sector numbers left unlabelled", ""),
        ParamDef::long("sector_text", "Text labels for sectors, e.g. 1 = 2x + 10; 3 = y", ""),
        ParamDef::bool("right", "Right-angle squares", true),
        ParamDef::bool("letters", "Point letters on the rays", true),
        ParamDef::int("dec", "Round measures to (decimal places)", 0, 3, 1),
        ParamDef::float("turn", "Turn the figure (degrees)", -180.0, 180.0, 1.0, 0.0),
        ParamDef::choice("ray_ends", "Ray ends", &["Arrows", "Plain", "Points"], "Arrows"),
        ParamDef::bool("shade", "Shade the angles", true),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        // "Add ray" (chips card) counts the rays in use; an empty row within the count is drawn
        // in the middle of the widest gap, as the reference places a new ray.
        ParamDef::int("ray_count", "Rays in use, the baseline included", 2, 6, 2),
    ]);
    v
}

/// The middle of the widest gap between `dirs` (degrees), searched over 0-180° while every ray is
/// in the upper half, else round the whole circle. Whole degrees, like the reference's new rays.
pub(crate) fn widest_gap_mid(dirs: &[f64]) -> f64 {
    let upper = if dirs.iter().all(|d| *d <= 180.0 + 1e-9) { 180.0 } else { 360.0 };
    let mut pts: Vec<f64> = dirs.iter().copied().filter(|d| *d <= upper).collect();
    pts.extend([0.0, upper]);
    pts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut best = (0.0, upper);
    for w in pts.windows(2) {
        if w[1] - w[0] > best.1 - best.0 + 1e-9 || best == (0.0, upper) { best = (w[0], w[1]); }
    }
    ((best.0 + best.1) / 2.0).round()
}

/// Ray directions with their source row: typed rows first, then each empty row within
/// `ray_count` placed in the widest gap left by the rays before it (bool: placed automatically).
pub(crate) fn ray_rows(p: &Params) -> Vec<(usize, f64, bool)> {
    let cnt = p.i("ray_count").clamp(2, 6) as usize;
    let mut out: Vec<(usize, f64, bool)> = vec![];
    for i in 1..6 {
        if let Ok(Some(t)) = parse_typed(p.s(RAY_V[i])) { out.push((i, t.v.rem_euclid(360.0), false)); }
    }
    for i in 1..cnt {
        if !p.s(RAY_V[i]).trim().is_empty() { continue; }
        let mut have: Vec<f64> = out.iter().map(|r| r.1).collect();
        have.push(0.0);
        out.push((i, widest_gap_mid(&have), true));
    }
    out
}

/// Ray directions in degrees ccw from the baseline, sorted: (direction, source row 0-5, is the
/// back half of a ray drawn as a line), and whether the last sector wraps back to the baseline.
pub(crate) fn angle_dirs(p: &Params) -> (Vec<(f64, usize, bool)>, bool) {
    let mut d: Vec<(f64, usize, bool)> = vec![(0.0, 0, false)];
    for (i, a, _) in ray_rows(p) { d.push((a, i, false)); }
    let mut lines = false;
    for k in 0..d.len() {
        let (a, i, _) = d[k];
        if p.s(RAY_LINE[i]) == "Line" { d.push(((a + 180.0).rem_euclid(360.0), i, true)); lines = true; }
    }
    d.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    d.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-6);
    (d, lines || p.b("around"))
}

fn style_dash(s: &str) -> &'static str { match s { "Dashed" => "8 5", "Dotted" => "0.1 5", _ => "none" } }

pub(crate) fn angles_render(p: &Params) -> String {
    let (dirs, wrap) = angle_dirs(p);
    if dirs.len() < 2 { return note_fig("Give ray 2 a direction in degrees."); }
    let rays: Vec<f64> = dirs.iter().map(|d| d.0).collect();
    let turn = p.f("turn");
    let fs = fscale(p);
    let (w, h) = (480.0, 360.0);
    let c = (w / 2.0, h / 2.0);
    let len = 140.0;
    let unk: Vec<usize> = nums(p.s("unknown")).iter().map(|v| *v as usize).collect();
    let hide: Vec<usize> = nums(p.s("hide")).iter().map(|v| *v as usize).collect();
    let texts = num_text_map(p.s("sector_text"));
    let mut d = new_doc(w, h);
    let mut secs: Vec<(f64, f64)> = rays.windows(2).map(|w| (w[0], w[1])).collect();
    if wrap { secs.push((*rays.last().unwrap(), rays[0] + 360.0)); }
    if p.b("shade") {
        for (a0, a1) in &secs {
            let n = 24;
            let mut pts = vec![c];
            for i in 0..=n { let t = (turn + a0 + (a1 - a0) * i as f64 / n as f64).to_radians(); pts.push((c.0 + 42.0 * t.cos(), c.1 - 42.0 * t.sin())); }
            d.poly(&pts, "#eef4fb", "none", 0.0);
        }
    }
    for (i, (r, src, _)) in dirs.iter().enumerate() {
        let t = (r + turn).to_radians();
        let e = (c.0 + len * t.cos(), c.1 - len * t.sin());
        let dash = style_dash(p.s(RAY_STYLE[*src]));
        dline(&mut d, c, e, INK, 2.0, dash);
        match p.s("ray_ends") {
            "Plain" => {}
            "Points" => { d.circle(e.0, e.1, 3.4, INK, INK, 1.0); }
            _ => {
                let u = unit_v(c, e);
                let nn = (-u.1, u.0);
                let back = (e.0 - u.0 * 12.0, e.1 - u.1 * 12.0);
                d.poly(&[e, (back.0 + nn.0 * 5.0, back.1 + nn.1 * 5.0), (back.0 - nn.0 * 5.0, back.1 - nn.1 * 5.0)], INK, INK, 1.0);
            }
        }
        if p.b("letters") {
            let name = ((b'B' + i as u8) as char).to_string();
            mlabel(&mut d, c.0 + (len + 16.0) * t.cos(), c.1 - (len + 16.0) * t.sin(), 17.0 * fs, &name, "");
        }
    }
    d.circle(c.0, c.1, 3.0, INK, INK, 1.0);
    if p.b("letters") && !wrap {
        let back = ((rays[0] + rays[rays.len() - 1]) / 2.0 + 180.0 + turn).to_radians();
        mlabel(&mut d, c.0 + 18.0 * back.cos(), c.1 - 18.0 * back.sin(), 17.0 * fs, "A", "");
    }
    for (i, (a0, a1)) in secs.iter().enumerate() {
        let idx = i + 1;
        let span = a1 - a0;
        let txt = if hide.contains(&idx) { String::new() }
            else if let Some((_, t)) = texts.iter().find(|(k, _)| *k == idx) { t.clone() }
            else if unk.contains(&idx) { "x".into() } else { format!("{}\u{b0}", fmt(span, p.i("dec"))) };
        let rad = if span < 28.0 { 48.0 + (i % 2) as f64 * 10.0 } else { 36.0 + (i % 2) as f64 * 6.0 };
        sector(&mut d, c, turn + a0, turn + a1, rad, &txt, p.b("right"), 15.0 * fs);
    }
    d.finish()
}

pub(crate) fn angles_computed(p: &Params) -> Vec<(&'static str, String)> {
    let mut out = vec![("ray1", "0".to_string())];
    // a row added with "Add ray" and left empty shows where its ray is drawn
    for (i, a, auto) in ray_rows(p) { if auto { out.push((RAY_V[i], fmt(a, 1))); } }
    if angle_dirs(p).0.len() < 2 { out.push(("__msg", "Give ray 2 a direction in degrees.".to_string())); }
    out
}

// ----- parallel lines (lines card) -----

pub(crate) const PL_NAMES: [&str; 5] = ["name_m", "name_n", "name_p", "name_t", "name_u"];
pub(crate) const PL_STYLE: [&str; 5] = ["m_style", "n_style", "p_style", "t_style", "u_style"];
pub(crate) const PL_ARROWS: [&str; 5] = ["m_arrows", "n_arrows", "p_arrows", "t_arrows", "u_arrows"];

pub(crate) fn parallel_lines_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::int("lines", "Parallel lines", 2, 3, 2),
        ParamDef::int("transversals", "Transversals", 1, 2, 1),
        ParamDef::text("name_m", "Name of the first line", "m"),
        ParamDef::text("name_n", "Name of the second line", "n"),
        ParamDef::text("name_p", "Name of the third line", "p"),
        ParamDef::text("name_t", "Name of the transversal", "t"),
        ParamDef::text("name_u", "Name of the second transversal", "u"),
        ParamDef::float("angle", "Angle above m, right of the transversal (degrees)", 10.0, 170.0, 1.0, 65.0),
        ParamDef::float("angle_b", "Second transversal angle (degrees; its angles number on after the first's)", 10.0, 170.0, 1.0, 115.0),
    ];
    for i in 0..5 {
        v.push(ParamDef::choice(PL_STYLE[i], "Line", &STYLES, "Solid"));
        v.push(ParamDef::choice(PL_ARROWS[i], "Parallel arrows", &TICKS, if i < 3 { "1" } else { "None" }));
    }
    v.extend([
        ParamDef::float("gap", "Gap between lines", 0.5, 3.0, 0.1, 1.0),
        ParamDef::choice("labels", "Label style", &["Degrees", "Numbers"], "Degrees"),
        ParamDef::list("marked", "Angles to label (1-8 at the first two crossings, 5-8 at m and n, 9-12 at the third line), e.g. 2, 6", "2, 6"),
        ParamDef::list("unknown", "Marked angles shown as x", "6"),
        ParamDef::long("angle_text", "Text labels for marked angles, e.g. 2 = 3x + 5; 6 = y", ""),
        ParamDef::bool("names", "Line names (m, n, t)", true),
        ParamDef::int("dec", "Round measures to (decimal places)", 0, 3, 1),
        ParamDef::float("turn", "Turn the figure (degrees)", -180.0, 180.0, 1.0, 0.0),
        ParamDef::choice("line_ends", "Line ends", &["Arrowheads", "Plain"], "Arrowheads"),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
    ]);
    v
}

/// `k` parallel-arrow chevrons centred at c pointing along unit u.
fn chevrons(d: &mut Doc, c: Pt, u: Pt, k: u8) {
    let nn = (-u.1, u.0);
    for j in 0..k {
        let o = (j as f64 - (k as f64 - 1.0) / 2.0) * 8.0;
        let m = (c.0 + u.0 * o, c.1 + u.1 * o);
        let tip = (m.0 + u.0 * 3.0, m.1 + u.1 * 3.0);
        let back = (m.0 - u.0 * 3.0, m.1 - u.1 * 3.0);
        d.polyline(&[(back.0 + nn.0 * 5.0, back.1 + nn.1 * 5.0), tip, (back.0 - nn.0 * 5.0, back.1 - nn.1 * 5.0)], INK, 1.6);
    }
}

pub(crate) fn parallel_lines_render(p: &Params) -> String {
    let (w, h, ys) = pl_layout(p);
    let nl = ys.len();
    let turn = p.f("turn");
    let fs = fscale(p);
    let cc = (w / 2.0, h / 2.0);
    let t = |q: Pt| turn_scr(q, cc, turn);
    let mut d = new_doc(w, h);
    let unk = nums(p.s("unknown")).iter().map(|v| *v as usize).collect::<Vec<_>>();
    let marked = nums(p.s("marked")).iter().map(|v| *v as usize).collect::<Vec<_>>();
    let texts = num_text_map(p.s("angle_text"));
    let nm = |k: &str, dflt: &str| { let v = p.s(k).trim(); if v.is_empty() { dflt.to_string() } else { v.to_string() } };
    let names = [nm("name_m", "m"), nm("name_n", "n"), nm("name_p", "p")];
    let ends = p.s("line_ends") != "Plain";
    let (x0, x1) = ((w - 300.0) / 2.0, (w + 300.0) / 2.0);
    // the first transversal's crossing x on line y, for placing the line's parallel marks like the reference
    let cx_first = pl_cx0(p, 0, &ys, w);
    let cot0 = { let r = p.f("angle").clamp(10.0, 170.0).to_radians(); r.cos() / r.sin() };
    for (i, y) in ys.iter().enumerate() {
        let (a, b) = (t((x0, *y)), t((x1, *y)));
        dline(&mut d, a, b, INK, 2.2, style_dash(p.s(PL_STYLE[i])));
        if ends { line_heads(&mut d, a, b); }
        let k = tick_n(p, PL_ARROWS[i]);
        let cross_x = cx_first - cot0 * (*y - ys[0]);
        if k > 0 { chevrons(&mut d, t(((cross_x - 86.6).max(x0 + 20.0), *y)), unit_v(a, b), k); }
        if p.b("names") { let q = t((x1 + 8.0 + 10.0 * fs, *y)); mlabel(&mut d, q.0, q.1, 21.0 * fs, &names[i], ""); }
    }
    let nt = p.i("transversals").clamp(1, 2) as usize;
    let rad = 26.0;
    let lsz = 15.0 * fs;
    for ti in 0..nt {
        let th = if ti == 0 { p.f("angle").clamp(10.0, 170.0) } else { p.f("angle_b").clamp(10.0, 170.0) };
        let cx0 = pl_cx0(p, ti, &ys, w);
        let tr = th.to_radians();
        let slope = (tr.cos(), -tr.sin());
        let cross: Vec<Pt> = ys.iter().map(|y| { let s = (*y - ys[0]) / slope.1; (cx0 + slope.0 * s, *y) }).collect();
        let tl = 90.0 / slope.1.abs().max(0.2);
        let a = (cross[0].0 + slope.0 * tl, cross[0].1 + slope.1 * tl);
        let last = cross[nl - 1];
        let b = (last.0 - slope.0 * tl, last.1 - slope.1 * tl);
        let (ta, tb) = (t(a), t(b));
        dline(&mut d, ta, tb, INK, 2.0, style_dash(p.s(PL_STYLE[3 + ti])));
        if ends { line_heads(&mut d, ta, tb); }
        let k = tick_n(p, PL_ARROWS[3 + ti]);
        if k > 0 {
            let m = t((a.0 * 0.45 + cross[0].0 * 0.55, a.1 * 0.45 + cross[0].1 * 0.55));
            chevrons(&mut d, m, unit_v(tb, ta), k);
        }
        if p.b("names") {
            let tn = if ti == 0 { nm("name_t", "t") } else { nm("name_u", "u") };
            let q = t((a.0 + slope.0 * 18.0, a.1 + slope.1 * 18.0));
            mlabel(&mut d, q.0, q.1, 18.0 * fs, &tn, "");
        }
        for (i, c) in cross.iter().enumerate() {
            let secs = [(th, 180.0, 1usize), (0.0, th, 2), (180.0 + th, 360.0, 3), (180.0, 180.0 + th, 4)];
            let c = t(*c);
            for (a0, a1, k) in secs {
                let num = ti * 4 * nl + i * 4 + k;
                let (a0t, a1t) = (a0 + turn, a1 + turn);
                if !marked.contains(&num) {
                    if p.s("labels") == "Numbers" { sector(&mut d, c, a0t, a1t, rad, &num.to_string(), false, lsz); }
                    continue;
                }
                let span = a1 - a0;
                let txt = if p.s("labels") == "Numbers" { num.to_string() }
                    else if let Some((_, s)) = texts.iter().find(|(k, _)| *k == num) { s.clone() }
                    else if unk.contains(&num) { "x".to_string() } else { format!("{}\u{b0}", fmt(span, p.i("dec"))) };
                let r = if span < 50.0 { rad + 8.0 } else { rad };
                sector(&mut d, c, a0t, a1t, r, &txt, true, lsz);
            }
        }
    }
    d.finish()
}

pub(crate) fn pl_computed(p: &Params) -> Vec<(&'static str, String)> {
    let keys = ["m_line", "n_line", "p_line"];
    (0..3).map(|i| {
        let k = tick_n(p, PL_ARROWS[i]);
        let arrows = "\u{203a}".repeat(k as usize);
        (keys[i], format!("{} {arrows}", p.s(PL_STYLE[i])).trim().to_string())
    }).collect()
}

// ----- Measures cards -----

pub(crate) fn card(id: &str) -> Option<MeasureCard> {
    let row = |name: &'static str, value_key: &'static str, unit: &'static str, label_key: &'static str, text_key: &'static str, tick_key: Option<&'static str>| MeasureRow { name, name_key: None, value_key, unit, label_key, text_key, tick_key, equals: None };
    let side_names = ["AB", "BC", "CD", "DA"];
    let quad_rows = |id: &str| -> Vec<MeasureGroup> {
        let inp = q_inputs(id);
        let links: [Option<&'static str>; 8] = match id {
            "rectangle" => [None, None, Some("AB"), Some("BC"), Some("90\u{b0}"), Some("90\u{b0}"), Some("90\u{b0}"), Some("90\u{b0}")],
            "parallelogram" => [None, Some("DA"), Some("AB"), None, None, None, Some("\u{2220}A"), Some("\u{2220}B")],
            "kite" => [None, Some("AB"), Some("DA"), None, None, None, Some("\u{2220}A"), None],
            _ => [None; 8],
        };
        let mut sides = Vec::new();
        let mut angs = Vec::new();
        for i in 0..8 {
            let mut r = row(if i < 4 { side_names[i] } else { "\u{2220}" }, Q_V[i], if i < 4 { "$unit" } else { "\u{b0}" }, Q_LAB[i], Q_TXT[i], Some(Q_MARK[i]));
            if !inp[i] { r.equals = links[i]; }
            if i >= 4 { r.name_key = Some(NAME_K[i - 4]); }
            if i < 4 { sides.push(r); } else { angs.push(r); }
        }
        if id == "trapezoid" { sides.push(row("h", "m_h", "$unit", "height_label", "height_text", None)); }
        // the side popover's second mark (parallel arrows) as its own rows: the card's rows hold
        // one mark each
        let arrows = (0..4).map(|i| MeasureRow { equals: Some("\u{203a}"), ..row(side_names[i], Q_PAR[i], "", Q_ARROW[i], "", None) }).collect();
        vec![MeasureGroup { title: "Sides", rows: sides }, MeasureGroup { title: "Angles", rows: angs }, MeasureGroup { title: "Parallel arrows", rows: arrows }]
    };
    let help_quad = "Fill in the kind's measures; the rest are worked out and shown faintly. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}. Rename a corner in the box before its angle. The button after each measure sets its label and congruence marks.";
    Some(match id {
        "triangle" => {
            let mut angs = Vec::new();
            for i in 0..3 {
                let mut r = row("\u{2220}", TRI_V[i], "\u{b0}", TRI_LAB[i], TRI_TXT[i], Some(TRI_MARK[i]));
                r.name_key = Some(NAME_K[i]);
                angs.push(r);
            }
            let sides = (3..6).map(|i| row(["AB", "BC", "CA"][i - 3], TRI_V[i], "$unit", TRI_LAB[i], TRI_TXT[i], Some(TRI_MARK[i]))).collect();
            MeasureCard {
                title: "Measures",
                help: "Fill in any three: two angles and a side, two sides and an angle, or three sides. The rest are worked out and shown faintly. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}. Rename a corner in the box before its angle.",
                groups: vec![MeasureGroup { title: "Angles", rows: angs }, MeasureGroup { title: "Sides", rows: sides }],
            }
        }
        "rectangle" | "parallelogram" | "trapezoid" | "kite" => MeasureCard { title: "Measures", help: help_quad, groups: quad_rows(id) },
        "regular-polygon" => {
            let rows = vec![
                row("Sides", "n", "", "", "", None),
                row("Size", "value", "$unit", "", "", None),
            ];
            let mut side = row("Side", "m_side", "$unit", "side_lab", "side_label", Some("side_ticks"));
            side.equals = Some("worked out");
            let mut ang = row("Angle", "m_angle", "", "ang_lab", "ang_label", Some("ang_arcs"));
            ang.equals = Some("(n \u{2212} 2)\u{b7}180\u{b0}/n");
            let mut apo = row("Apothem", "m_apothem", "$unit", "apothem_label", "apothem_text", None);
            apo.equals = Some("worked out");
            let mut rad = row("Radius", "m_radius", "$unit", "radius_label", "radius_text", None);
            rad.equals = Some("worked out");
            MeasureCard {
                title: "Polygon",
                help: "Pick the number of sides, then give one measure: the side, the radius (center to a corner) or the apothem (center to the middle of a side) under Size from. The rest are worked out. The side's label goes on the bottom side and the angle's in the bottom left corner; their marks go on every side or angle.",
                groups: vec![MeasureGroup { title: "", rows }, MeasureGroup { title: "Measures", rows: vec![side, ang, apo, rad] }],
            }
        }
        "angles" => {
            let mut base = row("Baseline", "ray1", "\u{b0}", RAY_STYLE[0], "", Some(RAY_LINE[0]));
            base.equals = Some("0\u{b0}");
            let mut rows = vec![base];
            for i in 1..6 { rows.push(row(["", "Ray 2", "Ray 3", "Ray 4", "Ray 5", "Ray 6"][i], RAY_V[i], "\u{b0}", RAY_STYLE[i], "", Some(RAY_LINE[i]))); }
            MeasureCard {
                title: "Rays",
                help: "Every ray starts at the vertex. The baseline points right, and each other ray's direction is its angle from the baseline, counterclockwise, as a protractor reads it: 90\u{b0} points straight up. Leave a direction blank for no ray. The button after a ray sets its line style and whether it runs through the vertex as a line. Click an angle on the figure to label it.",
                groups: vec![MeasureGroup { title: "", rows }],
            }
        }
        "parallel-lines" => {
            let line_keys = ["m_line", "n_line", "p_line"];
            let lines = (0..3).map(|i| {
                let mut r = row(["Line 1", "Line 2", "Line 3"][i], line_keys[i], "", PL_STYLE[i], "", Some(PL_ARROWS[i]));
                r.name_key = Some(PL_NAMES[i]);
                r.equals = Some("\u{2225}");
                r
            }).collect();
            let trans = (0..2).map(|i| {
                let mut r = row(["Transversal 1", "Transversal 2"][i], ["angle", "angle_b"][i], "\u{b0}", PL_STYLE[3 + i], "", Some(PL_ARROWS[3 + i]));
                r.name_key = Some(PL_NAMES[3 + i]);
                r
            }).collect();
            MeasureCard {
                title: "Parallel lines",
                help: "The parallel lines run across the page, evenly spaced. Each transversal crosses them at the angle you give: the angle above the top line, to the right of the transversal. The button after a line sets its style and parallel arrows. Click an angle on the figure to label it.",
                groups: vec![MeasureGroup { title: "Parallel lines", rows: lines }, MeasureGroup { title: "Transversals", rows: trans }],
            }
        }
        _ => return None,
    })
}

pub(crate) fn computed(id: &str, p: &Params) -> Option<Vec<(&'static str, String)>> {
    Some(match id {
        "triangle" => {
            let s = match tri_solve(p) {
                Ok(s) => s,
                // The card shows this line under the rows ("__msg" is not a row).
                Err(e) => return Some(vec![("__msg", e.to_string())]),
            };
            let vals = tri_values(&s);
            let dec = p.i("dec");
            (0..6).filter(|i| s.typed[*i].is_none()).filter_map(|i| vals[i].map(|v| (TRI_V[i], fmt(v, dec)))).collect()
        }
        "rectangle" | "parallelogram" | "trapezoid" | "kite" => quad_computed(id, p),
        "regular-polygon" => rp_computed(p),
        "angles" => angles_computed(p),
        "parallel-lines" => pl_computed(p),
        _ => return None,
    })
}

// ----- legacy specs -----

type Spec = crate::model::FigureSpec;
fn js(spec: &Spec, k: &str) -> Option<String> { spec.params.get(k).and_then(|v| v.as_str()).map(|s| s.to_string()) }
fn jf(spec: &Spec, k: &str) -> Option<f64> { spec.params.get(k).and_then(|v| v.as_f64()) }
fn jb(spec: &Spec, k: &str) -> Option<bool> { spec.params.get(k).and_then(|v| v.as_bool()) }
fn has(spec: &Spec, k: &str) -> bool { spec.params.contains_key(k) }

/// Old label lists (`sides`, `angles`, `unknown`, `side_text`) -> one row's mode and text.
fn old_label(name: &str, angle: bool, show: &[String], unk: &[String], tmap: &[(String, String)]) -> (&'static str, String) {
    if let Some(t) = text_for(tmap, name) { return ("Text", t); }
    let up = name.to_uppercase();
    let hit = |l: &[String]| if angle { l.contains(&up) } else { listed(l, &up) };
    if hit(unk) { ("Text", "x".into()) } else if hit(show) { ("Measure", String::new()) } else { ("None", String::new()) }
}

fn set_label(p: &mut Params, lab: &str, txt: &str, v: (&'static str, String)) {
    p.set_s(lab, v.0);
    if v.0 == "Text" { p.set_s(txt, &v.1); }
}

/// Shared part of the polygon migrations: corner names, label modes from the old lists, ticks
/// (explicit `marks`, or the old automatic equal-side ticks) and arcs.
fn migrate_labels(spec: &Spec, p: &mut Params, n: usize, rows: &[(usize, bool, &str, &str, &str)], d_sides: &str, d_angles: &str, d_unknown: &str, lens: &[f64]) {
    let custom: Vec<String> = js(spec, "vnames").unwrap_or_default().split(|c: char| c == ',' || c.is_whitespace()).map(|t| t.trim().to_uppercase()).filter(|t| !t.is_empty()).collect();
    let names: Vec<String> = (0..n).map(|i| custom.get(i).cloned().unwrap_or_else(|| ((b'A' + i as u8) as char).to_string())).collect();
    for i in 0..n { p.set_s(NAME_K[i], &names[i]); }
    let show = tokens(&js(spec, "sides").unwrap_or_else(|| d_sides.into()));
    let angs = tokens(&js(spec, "angles").unwrap_or_else(|| d_angles.into()));
    let unk = tokens(&js(spec, "unknown").unwrap_or_else(|| d_unknown.into()));
    let tmap = text_map(&js(spec, "side_text").unwrap_or_default());
    let ticks = parse_counts(&js(spec, "marks").unwrap_or_default(), &names, true)
        .unwrap_or_else(|| if jb(spec, "ticks").unwrap_or(true) { auto_ticks(lens) } else { vec![0; n] });
    let arcs = parse_counts(&js(spec, "arcs").unwrap_or_default(), &names, false).unwrap_or_else(|| vec![0; n]);
    for &(i, angle, lab, txt, mark) in rows {
        let name = if angle { names[i].clone() } else { format!("{}{}", names[i], names[(i + 1) % n]) };
        set_label(p, lab, txt, old_label(&name, angle, if angle { &angs } else { &show }, &unk, &tmap));
        let k = if angle { arcs[i] } else { ticks[i] };
        p.set_s(mark, if k == 0 { "None" } else { TICKS[k as usize] });
    }
    // diagonals: the old lists also labelled AC / BD
    for (dk, lab, txt) in [("AC", "ac_lab", "ac_label"), ("BD", "bd_lab", "bd_label")] {
        if n == 4 && p.0.contains_key(lab) {
            let dn = if dk == "AC" { format!("{}{}", names[0], names[2]) } else { format!("{}{}", names[1], names[3]) };
            set_label(p, lab, txt, old_label(&dn, false, &show, &unk, &tmap));
        }
    }
}

fn ns(v: f64) -> String { num_str(v, 4) }

pub(crate) fn migrate(id: &str, spec: &Spec, p: &mut Params) {
    match id {
        "triangle" => {
            if has(spec, "m_ab") || !(has(spec, "mode") || has(spec, "ab")) { return; }
            let mode = js(spec, "mode").unwrap_or_else(|| "Right triangle (legs AB, BC)".into());
            let (ab, bc, ca) = (jf(spec, "ab").unwrap_or(12.0), jf(spec, "bc").unwrap_or(5.0), jf(spec, "ca").unwrap_or(13.0));
            let (aa, bb) = (jf(spec, "ang_a").unwrap_or(60.0), jf(spec, "ang_b").unwrap_or(60.0));
            for k in TRI_V { p.set_s(k, ""); }
            let set: Vec<(&str, f64)> = if mode.starts_with("Three") { vec![("m_ab", ab), ("m_bc", bc), ("m_ca", ca)] }
                else if mode.starts_with("Two sides and an angle not") { vec![("m_ab", ab), ("m_bc", bc), ("m_a", aa)] }
                else if mode.starts_with("Two sides") { vec![("m_ab", ab), ("m_ca", ca), ("m_a", aa)] }
                else if mode.starts_with("Two angles and a side not") { vec![("m_a", aa), ("m_b", bb), ("m_bc", bc)] }
                else if mode.starts_with("Two angles") { vec![("m_a", aa), ("m_b", bb), ("m_ab", ab)] }
                else if mode.starts_with("Isosceles") { vec![("m_ab", ab), ("m_bc", bc), ("m_ca", bc)] }
                else if mode.starts_with("Equilateral") { vec![("m_ab", ab), ("m_bc", ab), ("m_ca", ab)] }
                else { vec![("m_ab", ab), ("m_bc", bc), ("m_b", 90.0)] };
            for (k, v) in set { p.set_s(k, &ns(v)); }
            let lens: Vec<f64> = tri_solve(p).ok().and_then(|s| s.out.side.map(|x| x.to_vec())).unwrap_or_else(|| vec![1.0, 2.0, 3.0]);
            let rows: Vec<(usize, bool, &str, &str, &str)> = (0..3).map(|i| (i, true, TRI_LAB[i], TRI_TXT[i], TRI_MARK[i])).chain((0..3).map(|i| (i, false, TRI_LAB[i + 3], TRI_TXT[i + 3], TRI_MARK[i + 3]))).collect();
            migrate_labels(spec, p, 3, &rows, "AB, BC, CA", "A", "BC", &lens);
        }
        "rectangle" | "parallelogram" | "trapezoid" | "kite" => {
            if has(spec, "m_ab") || !has(spec, "ab") { return; }
            for k in Q_V { if p.0.contains_key(k) { p.set_s(k, ""); } }
            let f = |k: &str, d: f64| jf(spec, k).unwrap_or(d);
            let kind = js(spec, "kind").unwrap_or_default();
            let (d_sides, d_angles, d_unknown) = match id {
                "rectangle" => {
                    p.set_s("m_ab", &ns(f("ab", 10.0)));
                    if kind != "Square" { p.set_s("m_bc", &ns(f("bc", 6.0))); }
                    ("AB, BC", "", "")
                }
                "parallelogram" => {
                    p.set_s("m_ab", &ns(f("ab", 10.0)));
                    if kind != "Rhombus" { p.set_s("m_da", &ns(f("da", 6.0))); }
                    p.set_s("m_a", &ns(f("ang_a", 60.0)));
                    ("AB, DA", "A", "")
                }
                "kite" => {
                    let (a_len, d_len, b) = (f("ab", 5.0), f("da", 9.0), f("ang_b", 100.0));
                    // the old kite stretched DA to at least just past the half-width
                    let w = a_len * (b.to_radians() / 2.0).sin();
                    p.set_s("m_ab", &ns(a_len));
                    p.set_s("m_da", &ns(d_len.max(w * 1.02)));
                    p.set_s("m_b", &ns(b));
                    ("AB, DA", "B", "")
                }
                _ => {
                    let (ab, cd, h) = (f("ab", 12.0), f("cd", 7.0), f("height", 5.0));
                    p.set_s("m_ab", &ns(ab));
                    p.set_s("m_cd", &ns(cd));
                    match kind.as_str() {
                        "Isosceles trapezoid" => p.set_s("m_h", &ns(h)),
                        "Trapezoid" => {
                            let off = f("offset", 2.0);
                            p.set_s("m_h", &ns(h));
                            p.set_s("m_a", &ns(h.atan2(off).to_degrees()));
                        }
                        _ => p.set_s("m_da", &ns(h)),
                    }
                    // the old height choice was a number; the drawn-height label stays
                    ("AB, CD, DA", "", "BC")
                }
            };
            let lens: Vec<f64> = quad_solve(id, p).ok().map(|s| (0..4).map(|i| s.vals[i].unwrap_or(0.0)).collect()).unwrap_or_else(|| vec![1.0, 2.0, 3.0, 4.0]);
            let rows: Vec<(usize, bool, &str, &str, &str)> = (0..4).map(|i| (i, false, Q_LAB[i], Q_TXT[i], Q_MARK[i])).chain((0..4).map(|i| (i, true, Q_LAB[i + 4], Q_TXT[i + 4], Q_MARK[i + 4]))).collect();
            migrate_labels(spec, p, 4, &rows, d_sides, d_angles, d_unknown, &lens);
        }
        "regular-polygon" => {
            if has(spec, "side_lab") || !has(spec, "n") { return; }
            let n = jf(spec, "n").unwrap_or(6.0).clamp(3.0, 20.0) as usize;
            let custom: Vec<String> = js(spec, "vnames").unwrap_or_default().split(|c: char| c == ',' || c.is_whitespace()).map(|t| t.trim().to_uppercase()).filter(|t| !t.is_empty()).collect();
            let names: Vec<String> = (0..n).map(|i| custom.get(i).cloned().unwrap_or_else(|| ((b'A' + i as u8) as char).to_string())).collect();
            let show = tokens(&js(spec, "sides").unwrap_or_else(|| "AB".into()));
            let angs = tokens(&js(spec, "angles").unwrap_or_default());
            let unk = tokens(&js(spec, "unknown").unwrap_or_default());
            let tmap = text_map(&js(spec, "side_text").unwrap_or_default());
            let s0 = format!("{}{}", names[0], names[1 % n]);
            set_label(p, "side_lab", "side_label", old_label(&s0, false, &show, &unk, &tmap));
            set_label(p, "ang_lab", "ang_label", old_label(&names[0], true, &angs, &unk, &tmap));
            let ticks = parse_counts(&js(spec, "marks").unwrap_or_default(), &names, true)
                .unwrap_or_else(|| if jb(spec, "ticks").unwrap_or(true) { vec![1; n] } else { vec![0; n] });
            let arcs = parse_counts(&js(spec, "arcs").unwrap_or_default(), &names, false).unwrap_or_else(|| vec![0; n]);
            let t0 = ticks.iter().copied().max().unwrap_or(0);
            let a0 = arcs.iter().copied().max().unwrap_or(0);
            p.set_s("side_ticks", TICKS[t0.min(3) as usize]);
            p.set_s("ang_arcs", TICKS[a0.min(3) as usize]);
            // the old figure drew the radius to A (bottom left) by default and lettered the corners
            p.set_b("radius", jb(spec, "radius").unwrap_or(true));
            p.set_b("apothem", jb(spec, "apothem").unwrap_or(false));
            p.set_b("vertices", jb(spec, "vertices").unwrap_or(true));
            if !has(spec, "apothem_label") { p.set_s("apothem_label", "a = measure"); }
            if !has(spec, "radius_label") { p.set_s("radius_label", "r = measure"); }
        }
        "angles" => {
            if has(spec, "ray2") || !(has(spec, "mode") || has(spec, "angle")) { return; }
            let mode = js(spec, "mode").unwrap_or_else(|| "Angles on a straight line".into());
            let (a, a2) = (jf(spec, "angle").unwrap_or(60.0), jf(spec, "angle2").unwrap_or(110.0));
            for k in &RAY_V[1..] { p.set_s(k, ""); }
            let mut turn = jf(spec, "turn").unwrap_or(0.0);
            match mode.as_str() {
                "Single angle" => p.set_s("ray2", &ns(a)),
                "Angles around a point" => { p.set_s("ray2", &ns(a)); p.set_s("ray3", &ns((a + a2).min(358.0))); p.set_b("around", true); }
                "Vertical angles" => { p.set_s("ray2", &ns(a)); p.set_s("ray1_line", "Line"); p.set_s("ray2_line", "Line"); }
                "Custom rays" => {
                    let mut r: Vec<f64> = nums(&js(spec, "rays").unwrap_or_else(|| "0, 50, 130, 250".into())).iter().map(|x| x.rem_euclid(360.0)).collect();
                    r.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
                    r.dedup();
                    if r.len() >= 2 {
                        let wrap = r.len() > 2 && r[r.len() - 1] - r[0] < 340.0;
                        let base = r[0];
                        turn = (turn + base + 180.0).rem_euclid(360.0) - 180.0;
                        for (i, x) in r.iter().skip(1).take(5).enumerate() { p.set_s(RAY_V[i + 1], &ns(x - base)); }
                        p.set_b("around", wrap);
                    }
                }
                _ => { p.set_s("ray2", &ns(a)); p.set_s("ray3", "180"); }
            }
            p.set_f("turn", turn);
        }
        "parallel-lines" => {
            if has(spec, "m_arrows") || !has(spec, "angle") { return; }
            // the old figure drew double chevrons on every parallel line when `arrows` was on
            let k = if jb(spec, "arrows").unwrap_or(true) { "2" } else { "None" };
            for key in &PL_ARROWS[..3] { p.set_s(key, k); }
        }
        _ => {}
    }
}

// ----- examples (mirror the reference's "Start from an example") -----

pub(crate) fn examples(id: &str) -> Option<Vec<super::super::Example>> {
    use Value::{B, F, I, S};
    let s = |v: &str| S(v.to_string());
    let ex = |name: &'static str, params: Vec<(&'static str, Value)>| super::super::Example { name, params };
    let tri = |m: [&str; 6]| -> Vec<(&'static str, Value)> { (0..6).map(|i| (TRI_V[i], s(m[i]))).collect() };
    let with = |mut a: Vec<(&'static str, Value)>, b: Vec<(&'static str, Value)>| { a.extend(b); a };
    let v = match id {
        "triangle" => vec![
            ex("Right triangle with legs 3 and 4: find the hypotenuse", with(tri(["", "90", "", "4", "3", ""]), vec![("a_lab", s("None")), ("b_lab", s("None")), ("bc_lab", s("Measure")), ("ca_lab", s("Text")), ("ca_label", s("x"))])),
            ex("30-60-90 triangle with hypotenuse 10", with(tri(["30", "90", "", "", "5", ""]), vec![("b_lab", s("None")), ("c_lab", s("Measure")), ("ab_lab", s("Text")), ("ab_label", s("y")), ("bc_lab", s("Text")), ("bc_label", s("x")), ("ca_lab", s("Measure"))])),
            ex("SOHCAHTOA: find the side opposite a 35\u{b0} angle", with(tri(["35", "90", "", "10", "", ""]), vec![("b_lab", s("None")), ("bc_lab", s("Text")), ("bc_label", s("x"))])),
            ex("Law of cosines: two sides and the angle between them", with(tri(["50", "", "", "9", "", "7"]), vec![("b_lab", s("None")), ("ca_lab", s("Measure")), ("bc_lab", s("Text")), ("bc_label", s("x"))])),
            ex("Isosceles triangle with its height drawn, for area", with(tri(["", "", "", "10", "13", "13"]), vec![("height", s("From C")), ("a_lab", s("None")), ("b_lab", s("None")), ("bc_lab", s("None")), ("ca_lab", s("None")), ("bc_ticks", s("1")), ("ca_ticks", s("1"))])),
            ex("Ambiguous case: two sides and an angle not between them", with(tri(["40", "", "", "10", "7", ""]), vec![("b_lab", s("None")), ("bc_lab", s("Measure")), ("other", B(true))])),
        ],
        "rectangle" => vec![
            ex("Rectangle 12 cm by 5 cm for area and perimeter", vec![("m_ab", s("12")), ("m_bc", s("5")), ("unit", s("cm"))]),
            ex("Rectangle with a diagonal labeled x", vec![("m_ab", s("8")), ("m_bc", s("6")), ("diagonals", s("AC only")), ("ac_lab", s("Text")), ("ac_label", s("x"))]),
            ex("Rectangle with sides labeled 2x + 3 and x", vec![("m_ab", s("10")), ("m_bc", s("4")), ("ab_lab", s("Text")), ("ab_label", s("2x + 3")), ("bc_lab", s("Text")), ("bc_label", s("x"))]),
            ex("Square with its diagonals drawn", vec![("kind", s("Square")), ("m_ab", s("6")), ("m_bc", s("")), ("bc_lab", s("None")), ("diagonals", s("Drawn")), ("ab_ticks", s("1")), ("bc_ticks", s("1")), ("cd_ticks", s("1")), ("da_ticks", s("1"))]),
        ],
        "parallelogram" => vec![
            ex("Parallelogram with base and height for area", vec![("m_ab", s("10")), ("m_da", s("6")), ("m_a", s("60")), ("height", s("Onto AB (from D)")), ("da_lab", s("None")), ("a_lab", s("None"))]),
            ex("Parallelogram: find the missing angles", vec![("m_a", s("65")), ("ab_lab", s("None")), ("da_lab", s("None")), ("b_lab", s("Text")), ("b_label", s("x")), ("c_lab", s("Text")), ("c_label", s("y")), ("d_lab", s("Text")), ("d_label", s("z"))]),
            ex("Rhombus with its diagonals drawn", vec![("kind", s("Rhombus")), ("m_ab", s("5")), ("m_da", s("")), ("m_a", s("70")), ("diagonals", s("Drawn")), ("a_lab", s("None")), ("da_lab", s("None")), ("ab_ticks", s("1")), ("bc_ticks", s("1")), ("cd_ticks", s("1")), ("da_ticks", s("1")), ("arrow_marks", s("none"))]),
        ],
        "trapezoid" => vec![
            ex("Isosceles trapezoid with bases and height for area", vec![("kind", s("Isosceles trapezoid")), ("m_ab", s("12")), ("m_cd", s("6")), ("m_da", s("")), ("m_h", s("4")), ("show_height", B(true)), ("height_label", s("Measure")), ("da_lab", s("None")), ("bc_lab", s("None"))]),
            ex("Right trapezoid: find the slanted side", vec![("kind", s("Right trapezoid")), ("m_ab", s("12")), ("m_cd", s("7")), ("m_da", s("5")), ("bc_lab", s("Text")), ("bc_label", s("x"))]),
            ex("Trapezoid with an angle and its height", vec![("kind", s("Trapezoid")), ("m_ab", s("11")), ("m_cd", s("6")), ("m_da", s("")), ("m_a", s("53")), ("m_h", s("4")), ("show_height", B(true)), ("da_lab", s("None")), ("bc_lab", s("None"))]),
            ex("Isosceles trapezoid with its diagonals drawn", vec![("kind", s("Isosceles trapezoid")), ("m_ab", s("12")), ("m_cd", s("6")), ("m_da", s("")), ("m_h", s("5")), ("diagonals", s("Drawn")), ("da_lab", s("None")), ("bc_lab", s("None")), ("bc_ticks", s("1")), ("da_ticks", s("1"))]),
        ],
        "kite" => vec![
            ex("Kite with diagonals 6 and 12 for area", vec![("m_ab", s("3sqrt2")), ("m_da", s("3sqrt10")), ("m_b", s("90")), ("diagonals", s("Drawn and labelled")), ("ab_lab", s("None")), ("da_lab", s("None")), ("b_lab", s("None")), ("dec", I(0))]),
            ex("Kite: find the missing angle", vec![("m_b", s("110")), ("d_lab", s("Measure")), ("a_lab", s("Text")), ("a_label", s("x")), ("ab_lab", s("None")), ("da_lab", s("None"))]),
            ex("Kite with its line of symmetry", vec![("diagonals", s("BD only")), ("b_lab", s("None"))]),
        ],
        "regular-polygon" => vec![
            ex("Regular hexagon with its apothem, for area", vec![("n", I(6)), ("value", F(8.0)), ("apothem", B(true)), ("apothem_label", s("Measure")), ("radius", B(false))]),
            ex("Regular pentagon: find the interior angle", vec![("n", I(5)), ("apothem", B(false)), ("center_dot", B(false)), ("side_lab", s("None")), ("ang_lab", s("Text")), ("ang_label", s("x")), ("ang_arcs", s("1")), ("side_ticks", s("1"))]),
            ex("Regular octagon split into triangles by its radii", vec![("n", I(8)), ("all_radii", B(true)), ("apothem", B(false)), ("side_lab", s("None"))]),
            ex("Regular decagon with congruence marks", vec![("n", I(10)), ("apothem", B(false)), ("center_dot", B(false)), ("side_ticks", s("1")), ("ang_arcs", s("1")), ("side_lab", s("None"))]),
        ],
        "angles" => vec![
            ex("Single angle of 60\u{b0}", vec![("ray2", s("60"))]),
            ex("Angles on a straight line: find x", vec![("ray2", s("125")), ("ray3", s("180")), ("unknown", s("2"))]),
            ex("Vertical angles", vec![("ray2", s("50")), ("ray1_line", s("Line")), ("ray2_line", s("Line")), ("unknown", s("3")), ("hide", s("2, 4"))]),
            ex("Angles around a point with expressions", vec![("ray2", s("100")), ("ray3", s("220")), ("around", B(true)), ("sector_text", s("3 = 2x + 20"))]),
            ex("Complementary angles", vec![("ray2", s("35")), ("ray3", s("90")), ("unknown", s("2"))]),
        ],
        _ => return None,
    };
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(v: &[(usize, f64)]) -> [Option<(f64, f64)>; 6] {
        let mut a = [None; 6];
        for (i, x) in v { a[*i] = Some((*x, 0.005)); }
        a
    }
    fn near(a: f64, b: f64) -> bool { (a - b).abs() < 0.05 }

    #[test]
    fn triangle_solver_matches_reference() {
        // reference default: A 24, B 90, AB 12 -> BC 5.3, CA 13.1, C 66
        let o = tri_core(&g(&[(0, 24.0), (1, 90.0), (3, 12.0)]), false).unwrap();
        let s = o.side.unwrap();
        assert!(near(s[1], 5.34) && near(s[2], 13.14) && near(o.ang[2], 66.0));
        // SSS 3-4-5
        let o = tri_core(&g(&[(3, 3.0), (4, 4.0), (5, 5.0)]), false).unwrap();
        assert!(near(o.ang[0], 53.13) && near(o.ang[1], 90.0));
        // SSA two triangles: A 40, AB 10, BC 7 -> B 73.3 C 66.7 CA 10.4; other: C 113.3
        let o = tri_core(&g(&[(0, 40.0), (3, 10.0), (4, 7.0)]), false).unwrap();
        assert!(o.two && near(o.ang[1], 73.3) && near(o.ang[2], 66.7) && near(o.side.unwrap()[2], 10.43));
        let o2 = tri_core(&g(&[(0, 40.0), (3, 10.0), (4, 7.0)]), true).unwrap();
        assert!(near(o2.ang[2], 113.3));
        // SSA one triangle, and too short
        assert!(!tri_core(&g(&[(0, 40.0), (3, 10.0), (4, 12.0)]), false).unwrap().two);
        assert!(matches!(tri_core(&g(&[(0, 40.0), (3, 10.0), (4, 5.0)]), false), Err(TriErr::Short(0, m)) if near(m, 6.43)));
        assert!(matches!(tri_core(&g(&[(0, 100.0), (3, 10.0), (4, 8.0)]), false), Err(TriErr::Obtuse(0, false))));
        // AAS: A 40, B 60, BC 10 -> AB 15.3, CA 13.5
        let s = tri_core(&g(&[(0, 40.0), (1, 60.0), (4, 10.0)]), false).unwrap().side.unwrap();
        assert!(near(s[0], 15.32) && near(s[2], 13.47));
        // too few, sums, inequality
        assert_eq!(tri_core(&g(&[(3, 10.0)]), false).unwrap_err(), TriErr::Few(2));
        assert!(matches!(tri_core(&g(&[(0, 40.0), (1, 150.0), (4, 10.0)]), false), Err(TriErr::Sum2(0, 1, _))));
        assert!(matches!(tri_core(&g(&[(0, 30.0), (1, 60.0), (2, 91.0)]), false), Err(TriErr::Sum3(_))));
        assert_eq!(tri_core(&g(&[(3, 3.0), (4, 4.0), (5, 8.0)]), false).unwrap_err(), TriErr::Ineq(5));
        // three angles: shape only
        assert!(tri_core(&g(&[(0, 40.0), (1, 60.0), (2, 80.0)]), false).unwrap().side.is_none());
        // over-determined: angles + AB win, BC is checked (reference: "BC would be 6.53, not 7")
        assert!(matches!(tri_core(&g(&[(0, 40.0), (1, 60.0), (3, 10.0), (4, 7.0)]), false), Err(TriErr::Mismatch(4, c)) if near(c, 6.53)));
        // SSS + one angle: the sides win, the angle is checked
        assert!(matches!(tri_core(&g(&[(0, 40.0), (3, 10.0), (4, 7.0), (5, 12.0)]), false), Err(TriErr::Mismatch(0, c)) if near(c, 35.66)));
    }

    #[test]
    fn typed_values() {
        assert!((eval_typed("5sqrt2").unwrap() - 7.0711).abs() < 1e-3);
        assert!((eval_typed("7/2").unwrap() - 3.5).abs() < 1e-9);
        assert!((eval_typed("\u{221a}3").unwrap() - 1.732).abs() < 1e-3);
        assert!((eval_typed("2pi").unwrap() - 6.2832).abs() < 1e-3);
        assert!(eval_typed("x").is_none() && eval_typed("abc").is_none());
        assert_eq!(pretty("5sqrt2"), "5\u{221a}2");
        assert_eq!(pretty("sqrt(10)"), "\u{221a}10");
        assert_eq!(parse_typed("37").unwrap().unwrap().tol, 0.5 + 1e-9);
        // 37 accepted against 36.87 (reference), 40 refused against 40.54
        assert!((parse_typed("37").unwrap().unwrap().v - 36.87).abs() <= 0.5);
    }

    /// Old spec (keys as saved before the Measures rework) -> params the way figuresui loads it.
    fn load_old(id: &str, old: serde_json::Value) -> Params {
        let d = super::super::defs().into_iter().find(|d| d.id == id).unwrap();
        let spec = crate::model::FigureSpec { id: id.into(), params: old.as_object().unwrap().clone().into_iter().collect() };
        let defs = (d.params)();
        let mut p = Params::defaults(&defs);
        for pd in &defs {
            if let Some(j) = spec.params.get(pd.key) {
                let v = match &pd.default {
                    Value::F(_) => j.as_f64().map(Value::F),
                    Value::I(_) => j.as_i64().map(Value::I),
                    Value::B(_) => j.as_bool().map(Value::B),
                    Value::S(_) => j.as_str().map(|x| Value::S(x.into())),
                };
                if let Some(v) = v { p.0.insert(pd.key.into(), v); }
            }
        }
        super::super::migrate(id, &spec, &mut p);
        p.normalize(&defs, false);
        p
    }

    #[test]
    fn migrate_triangle() {
        let p = load_old("triangle", serde_json::json!({"mode": "Two angles and a side not between (AAS: A, B and BC)", "ab": 12.0, "bc": 5.0, "ca": 13.0, "ang_a": 30.0, "ang_b": 90.0,
            "sides": "RP", "angles": "P, R", "unknown": "QR", "side_text": "PQ = y", "ticks": true, "marks": "", "arcs": "Q 2", "vnames": "P, Q, R", "dec": 1}));
        assert_eq!((p.s("m_a"), p.s("m_b"), p.s("m_bc"), p.s("m_ab"), p.s("m_ca")), ("30", "90", "5", "", ""));
        assert_eq!((p.s("ca_lab"), p.s("bc_lab"), p.s("bc_label"), p.s("ab_lab"), p.s("ab_label")), ("Measure", "Text", "x", "Text", "y"));
        assert_eq!((p.s("a_lab"), p.s("b_lab"), p.s("c_lab"), p.s("b_arcs")), ("Measure", "None", "Measure", "2"));
        assert_eq!((p.s("name_a"), p.s("name_b"), p.s("name_c")), ("P", "Q", "R"));
        let s = tri_solve(&p).unwrap();
        assert!((s.out.side.unwrap()[2] - 10.0).abs() < 1e-6);
        // right triangle mode (the old default) -> AB, BC and B = 90
        let p = load_old("triangle", serde_json::json!({"mode": "Right triangle (legs AB, BC)", "ab": 4.0, "bc": 3.0}));
        assert_eq!((p.s("m_ab"), p.s("m_bc"), p.s("m_b")), ("4", "3", "90"));
        assert!((tri_solve(&p).unwrap().out.side.unwrap()[2] - 5.0).abs() < 1e-9);
    }

    #[test]
    fn migrate_quads() {
        let p = load_old("rectangle", serde_json::json!({"kind": "Rectangle", "ab": 8.0, "bc": 6.0, "sides": "AB, BC", "unknown": "AC", "diagonals": "AC only", "ticks": true}));
        assert_eq!((p.s("m_ab"), p.s("m_bc"), p.s("ab_lab"), p.s("cd_lab"), p.s("ac_lab"), p.s("ac_label")), ("8", "6", "Measure", "None", "Text", "x"));
        assert_eq!((p.s("ab_ticks"), p.s("bc_ticks"), p.s("cd_ticks"), p.s("da_ticks")), ("1", "2", "1", "2"));
        let p = load_old("parallelogram", serde_json::json!({"kind": "Rhombus", "ab": 5.0, "da": 9.0, "ang_a": 70.0, "sides": "AB", "angles": "A"}));
        assert_eq!((p.s("m_ab"), p.s("m_da"), p.s("m_a"), p.s("a_lab")), ("5", "", "70", "Measure"));
        let s = para_solve(&p).unwrap();
        assert!((s.vals[3].unwrap() - 5.0).abs() < 1e-9);
        let p = load_old("trapezoid", serde_json::json!({"kind": "Trapezoid", "ab": 11.0, "cd": 6.0, "height": 4.0, "offset": 3.0, "sides": "AB, CD", "unknown": ""}));
        let s = trap_solve(&p).unwrap();
        assert!((s.pts[3].0 - 3.0).abs() < 1e-3 && (s.pts[3].1 - 4.0).abs() < 1e-3 && (s.pts[2].0 - 9.0).abs() < 1e-3);
        let p = load_old("trapezoid", serde_json::json!({"kind": "Isosceles trapezoid", "ab": 12.0, "cd": 6.0, "height": 4.0}));
        let s = trap_solve(&p).unwrap();
        assert!((s.pts[3].0 - 3.0).abs() < 1e-9 && (s.pts[3].1 - 4.0).abs() < 1e-9);
        let p = load_old("trapezoid", serde_json::json!({"kind": "Right trapezoid", "ab": 12.0, "cd": 7.0, "height": 5.0, "unknown": "BC"}));
        let s = trap_solve(&p).unwrap();
        assert!((s.vals[1].unwrap() - 50f64.sqrt()).abs() < 1e-9 && p.s("bc_lab") == "Text");
        let p = load_old("kite", serde_json::json!({"ab": 5.0, "da": 9.0, "ang_b": 100.0, "sides": "AB, DA", "angles": "B"}));
        let s = kite_solve(&p).unwrap();
        assert!((s.vals[4].unwrap() - 104.8).abs() < 0.05 && (s.vals[7].unwrap() - 50.4).abs() < 0.05);
        assert_eq!((p.s("ab_ticks"), p.s("bc_ticks"), p.s("cd_ticks"), p.s("da_ticks")), ("1", "1", "2", "2"));
    }

    #[test]
    fn migrate_poly_angles_lines() {
        let p = load_old("regular-polygon", serde_json::json!({"n": 5, "from": "Side", "value": 8.0, "sides": "", "side_text": "A = x", "arcs": "all 1", "ticks": true, "radius": true}));
        assert_eq!((p.s("side_lab"), p.s("ang_lab"), p.s("ang_label"), p.s("ang_arcs"), p.s("side_ticks")), ("None", "Text", "x", "1", "1"));
        assert!(p.b("radius") && !p.b("apothem") && p.b("vertices"));
        let p = load_old("angles", serde_json::json!({"mode": "Vertical angles", "angle": 50.0, "unknown": "3", "hide": "2, 4"}));
        let (d, wrap) = angle_dirs(&p);
        assert!(wrap && d.iter().map(|x| x.0).collect::<Vec<_>>() == vec![0.0, 50.0, 180.0, 230.0]);
        let p = load_old("angles", serde_json::json!({"mode": "Custom rays", "rays": "30, 100, 200", "turn": 0.0}));
        let (d, wrap) = angle_dirs(&p);
        assert!(wrap && (p.f("turn") - 30.0).abs() < 1e-9 && d.iter().map(|x| x.0).collect::<Vec<_>>() == vec![0.0, 70.0, 170.0]);
        let p = load_old("angles", serde_json::json!({"mode": "Angles on a straight line", "angle": 125.0}));
        assert_eq!((p.s("ray2"), p.s("ray3")), ("125", "180"));
        assert!(!angle_dirs(&p).1);
        let p = load_old("parallel-lines", serde_json::json!({"angle": 65.0, "arrows": false}));
        assert_eq!(p.s("m_arrows"), "None");
        let p = load_old("parallel-lines", serde_json::json!({"angle": 65.0, "arrows": true}));
        assert_eq!(p.s("n_arrows"), "2");
    }

    #[test]
    fn quad_solvers() {
        let mut p = Params::defaults(&trapezoid_params());
        // reference: right trapezoid 12, 7, DA 5 -> BC 7.1, B 45, C 135
        let s = trap_solve(&p).unwrap();
        assert!((s.vals[1].unwrap() - 7.07).abs() < 0.01 && (s.vals[5].unwrap() - 45.0).abs() < 1e-6);
        // plain: AB 12, CD 7, A 65, h 5 -> DA 5.5, BC 5.7, B 61.9 (reference)
        for (k, v) in [("kind", "Trapezoid"), ("m_da", ""), ("m_a", "65"), ("m_h", "5")] { p.set_s(k, v); }
        let s = trap_solve(&p).unwrap();
        assert!((s.vals[3].unwrap() - 5.52).abs() < 0.01 && (s.vals[1].unwrap() - 5.67).abs() < 0.01 && (s.vals[5].unwrap() - 61.9).abs() < 0.05);
        // plain with one measure on each leg, h found numerically
        for (k, v) in [("m_a", "60"), ("m_b", "45"), ("m_h", "")] { p.set_s(k, v); }
        let s = trap_solve(&p).unwrap();
        let h = s.h.unwrap();
        assert!((h / 60f64.to_radians().tan() + h - 5.0).abs() < 1e-6);
        // isosceles: AB 12, CD 6, h 5 -> A 59 (reference)
        for (k, v) in [("kind", "Isosceles trapezoid"), ("m_a", ""), ("m_b", ""), ("m_cd", "6"), ("m_h", "5")] { p.set_s(k, v); }
        let s = trap_solve(&p).unwrap();
        assert!((s.vals[4].unwrap() - 59.04).abs() < 0.01);
        // too few / mismatch
        p.set_s("m_h", "");
        assert!(trap_solve(&p).unwrap_err().contains("one more"));
        for (k, v) in [("m_h", "5"), ("m_bc", "9")] { p.set_s(k, v); }
        assert!(trap_solve(&p).unwrap_err().contains("would be"));
        // kite: reference DA 3 too short, DA 4.5 -> A 71.7, D 116.7
        let mut k = Params::defaults(&kite_params());
        k.set_s("m_da", "3");
        assert!(kite_solve(&k).unwrap_err().contains("3.83"));
        k.set_s("m_da", "4.5");
        let s = kite_solve(&k).unwrap();
        assert!((s.vals[4].unwrap() - 71.7).abs() < 0.05 && (s.vals[7].unwrap() - 116.7).abs() < 0.05);
        // parallelogram from angle B
        let mut q = Params::defaults(&parallelogram_params());
        q.set_s("m_a", "");
        q.set_s("m_b", "120");
        assert!((para_solve(&q).unwrap().vals[4].unwrap() - 60.0).abs() < 1e-9);
        q.set_s("m_a", "70");
        assert!(para_solve(&q).unwrap_err().contains("would be"));
    }
}
}

pub(super) use flat::{flat_skin_summary, angle_dirs, quad_solve, quad_vals_of, regular_polygon_pts, tri_solve, write_givens, RAY_V, TRI_V, Q_V};
use flat::{angles_params, angles_render, kite_params, kite_render, parallel_lines_params, parallel_lines_render, parallelogram_params, parallelogram_render, rectangle_params, rectangle_render, regular_polygon_params, regular_polygon_render, trapezoid_params, trapezoid_render, triangle_params, triangle_render};

// ───────────────────────── coordinate grid ─────────────────────────

/// Expression evaluator: + - * / ^, implicit multiplication, x (and y for implicit linear
/// equations), pi, e, sqrt abs sin cos tan sec csc cot arcsin arccos arctan ln log log_b exp.
/// With `deg`, trig functions read (and inverse trig return) degrees.
struct Ev<'a> { s: &'a [char], i: usize, x: f64, y: Option<f64>, deg: bool }
impl<'a> Ev<'a> {
    fn ws(&mut self) { while self.i < self.s.len() && self.s[self.i] == ' ' { self.i += 1; } }
    fn peek(&mut self) -> Option<char> { self.ws(); self.s.get(self.i).copied() }
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            match self.peek() {
                Some('+') => { self.i += 1; v += self.term()?; }
                Some('-') => { self.i += 1; v -= self.term()?; }
                _ => return Some(v),
            }
        }
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        loop {
            match self.peek() {
                Some('*') => { self.i += 1; v *= self.unary()?; }
                Some('/') => { self.i += 1; v /= self.unary()?; }
                Some(c) if c.is_ascii_digit() || c == '.' || c == '(' || c.is_alphabetic() => { v *= self.power()?; }
                _ => return Some(v),
            }
        }
    }
    fn unary(&mut self) -> Option<f64> {
        match self.peek() {
            Some('-') => { self.i += 1; Some(-self.unary()?) }
            Some('+') => { self.i += 1; self.unary() }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Option<f64> {
        let b = self.atom()?;
        if self.peek() == Some('^') { self.i += 1; let e = self.unary()?; return Some(b.powf(e)); }
        Some(b)
    }
    fn arg(&mut self) -> Option<f64> {
        if self.peek()? != '(' { return None; }
        self.i += 1; let a = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1;
        Some(a)
    }
    fn atom(&mut self) -> Option<f64> {
        match self.peek()? {
            '(' => { self.i += 1; let v = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1; Some(v) }
            c if c.is_ascii_digit() || c == '.' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == '.') { self.i += 1; }
                // scientific notation: 1e12, 2.5E-3 (an e not followed by digits stays Euler's number: 2e = 2*e).
                // Not directly after a ^ : x^2e3 is x^2 times e times 3, never x^(2000)
                let after_caret = self.s[..st].iter().rev().find(|c| !c.is_whitespace()).is_some_and(|c| *c == '^');
                if !after_caret && self.i < self.s.len() && (self.s[self.i] == 'e' || self.s[self.i] == 'E') {
                    let mut j = self.i + 1;
                    if j < self.s.len() && (self.s[j] == '+' || self.s[j] == '-' || self.s[j] == '\u{2212}') { j += 1; }
                    if j < self.s.len() && self.s[j].is_ascii_digit() {
                        while j < self.s.len() && self.s[j].is_ascii_digit() { j += 1; }
                        let t: String = self.s[st..j].iter().map(|c| if *c == '\u{2212}' { '-' } else { *c }).collect();
                        self.i = j;
                        return t.parse().ok();
                    }
                }
                self.s[st..self.i].iter().collect::<String>().parse().ok()
            }
            '\u{3c0}' => { self.i += 1; Some(PI) }
            c if c.is_alphabetic() => {
                let st = self.i;
                while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() { self.i += 1; }
                let name: String = self.s[st..self.i].iter().collect();
                // a run of single-letter factors such as "xy" or "pix" is split from the left
                match name.as_str() {
                    "x" => return Some(self.x),
                    "y" => return self.y,
                    "pi" => return Some(PI),
                    "e" => return Some(std::f64::consts::E),
                    _ => {}
                }
                let (to_r, from_r) = if self.deg { (PI / 180.0, 180.0 / PI) } else { (1.0, 1.0) };
                if name == "log" && self.peek() == Some('_') {
                    self.i += 1;
                    let base = self.atom()?;
                    let a = self.arg()?;
                    return Some(a.ln() / base.ln());
                }
                let known = ["sqrt", "abs", "sin", "cos", "tan", "sec", "csc", "cot", "arcsin", "arccos", "arctan", "asin", "acos", "atan", "ln", "log", "exp"];
                if !known.contains(&name.as_str()) {
                    // implicit product of a known prefix, e.g. "xsin(x)" is rare; give up cleanly
                    if name.starts_with('x') { self.i = st + 1; return Some(self.x); }
                    // (2ex is 2 times e times x, as xe is x times e)
                    if name.starts_with('e') && name != "exp" { self.i = st + 1; return Some(std::f64::consts::E); }
                    // the same for a run starting with y ("yx", "y2" is not reached): only meaningful in implicit rows
                    if name.starts_with('y') && self.y.is_some() { self.i = st + 1; return self.y; }
                    return None;
                }
                let a = self.arg()?;
                Some(match name.as_str() {
                    "sqrt" => a.sqrt(), "abs" => a.abs(), "ln" => a.ln(), "log" => a.log10(), "exp" => a.exp(),
                    "sin" => (a * to_r).sin(), "cos" => (a * to_r).cos(), "tan" => (a * to_r).tan(),
                    "sec" => 1.0 / (a * to_r).cos(), "csc" => 1.0 / (a * to_r).sin(), "cot" => 1.0 / (a * to_r).tan(),
                    "arcsin" | "asin" => a.asin() * from_r, "arccos" | "acos" => a.acos() * from_r, "arctan" | "atan" => a.atan() * from_r,
                    _ => return None,
                })
            }
            _ => None,
        }
    }
}
/// Normalises typed math: unicode minus, middle dot, |a| -> abs(a), "sqrt x" stays as typed.
fn norm_expr(expr: &str) -> Vec<char> {
    let s = expr.replace('\u{2212}', "-").replace('\u{b7}', "*").replace('\u{d7}', "*").to_lowercase();
    let mut out = String::new();
    let mut open = false;
    for c in s.chars() {
        if c == '|' { out.push_str(if open { ")" } else { "abs(" }); open = !open; } else { out.push(c); }
    }
    out.chars().collect()
}
fn eval_full(expr: &str, x: f64, y: Option<f64>, deg: bool) -> Option<f64> {
    eval_cs(&norm_expr(expr), x, y, deg)
}
/// Deepest bracket nesting of a string.
fn nest_depth<'a>(cs: impl Iterator<Item = &'a char>) -> i32 {
    let (mut d, mut m) = (0i32, 0i32);
    for c in cs { if *c == '(' { d += 1; m = m.max(d); } else if *c == ')' { d -= 1; } }
    m
}
/// `eval_full` on an already normalised expression (the implicit-curve sampler calls this ~10^5 times).
fn eval_cs(cs: &[char], x: f64, y: Option<f64>, deg: bool) -> Option<f64> {
    // a pathological string (thousands of nested brackets) must not run the recursive reader out of stack
    if cs.len() > 4000 || (cs.len() > 200 && nest_depth(cs.iter()) > 200) { return None; }
    let mut e = Ev { s: cs, i: 0, x, y, deg };
    let v = e.expr()?;
    e.ws();
    if e.i == cs.len() { Some(v) } else { None }
}
pub(super) fn eval(expr: &str, x: f64) -> Option<f64> { eval_full(expr, x, None, false) }

/// Domain restriction of an implicit row, `, x > 0, y < 3, x^2 + y^2 < 25` (any mix, applied to each point).
#[derive(Default)]
struct ImpDom { x: Vec<Vec<Interval>>, y: Vec<Vec<Interval>>, extra: Vec<(Vec<char>, bool)>, deg: bool }
impl ImpDom {
    fn parse(dom: Option<&str>, deg: bool) -> ImpDom {
        let mut out = ImpDom { deg, ..ImpDom::default() };
        let Some(mut rest) = dom.map(|s| s.to_lowercase().replace(" and ", ",").replace('\u{2264}', "<=").replace('\u{2265}', ">=")) else { return out };
        loop {
            let (a, b) = split_domain(&rest);
            let a = a.trim().to_string();
            if !a.is_empty() { out.add(&a); }
            match b { Some(t) => rest = t.to_string(), None => break }
        }
        out
    }
    /// One piece: a bound on x alone or y alone ("0 < y <= 3") becomes an interval; anything else
    /// ("x^2 + y^2 < 25", "y > x + 1") is checked as a sign condition on left - right at each point.
    fn add(&mut self, a: &str) {
        let squeezed = a.replace("pi", "").replace("or", "");
        let letters: Vec<char> = squeezed.chars().filter(|c| c.is_ascii_alphabetic()).collect();
        let plain = letters.len() == 1 && (letters[0] == 'x' || letters[0] == 'y')
            && !a.contains(['^', '*', '/', '(', '+', '|'])
            && !a.chars().zip(a.chars().skip(1)).any(|(p, c)| c == '-' && false && p.is_alphanumeric());
        // "x - 2 > 0" has the variable and a minus after it: not a plain bound
        let minus_after_var = { let t: Vec<char> = a.chars().filter(|c| !c.is_whitespace()).collect(); t.windows(2).any(|w| (w[0] == 'x' || w[0] == 'y' || w[0].is_ascii_digit()) && w[1] == '-') };
        if plain && !minus_after_var {
            if a.contains('y') { self.y.push(parse_ineq(&a.replace('y', "x"))); } else { self.x.push(parse_ineq(a)); }
            return;
        }
        // split at the relational operators: operands a op b op c ...
        let cs: Vec<char> = a.chars().collect();
        let (mut operands, mut ops, mut cur, mut i) = (vec![], vec![], String::new(), 0);
        while i < cs.len() {
            if cs[i] == '<' || cs[i] == '>' {
                let gt = cs[i] == '>';
                if i + 1 < cs.len() && cs[i + 1] == '=' { i += 1; }
                operands.push(std::mem::take(&mut cur));
                ops.push(gt);
            } else { cur.push(cs[i]); }
            i += 1;
        }
        operands.push(cur);
        for (k, &gt) in ops.iter().enumerate() {
            if operands[k].trim().is_empty() || operands[k + 1].trim().is_empty() { continue; }
            self.extra.push((norm_expr(&format!("({})-({})", operands[k], operands[k + 1])), gt));
        }
    }
    fn is_empty(&self) -> bool { self.x.is_empty() && self.y.is_empty() && self.extra.is_empty() }
    fn has(ivs: &[Interval], v: f64, eps: f64) -> bool {
        ivs.iter().any(|iv| if eps > 0.0 { v >= iv.lo - eps && v <= iv.hi + eps } else { (v > iv.lo || (iv.lo_c && v >= iv.lo)) && (v < iv.hi || (iv.hi_c && v <= iv.hi)) })
    }
    /// Every typed restriction holds at (x, y); `eps` > 0 treats the limits as closed (for end dots).
    fn ok(&self, x: f64, y: f64, eps: f64) -> bool {
        self.x.iter().all(|c| Self::has(c, x, eps)) && self.y.iter().all(|c| Self::has(c, y, eps))
            && self.extra.iter().all(|(cs, gt)| eval_cs(cs, x, Some(y), self.deg).map_or(false, |v| if *gt { v >= -eps } else { v <= eps }))
    }
    /// (is_x, value, closed) of every finite limit of the plain bounds.
    fn limits(&self) -> Vec<(bool, f64, bool)> {
        let mut v = vec![];
        for (is_x, cs) in [(true, &self.x), (false, &self.y)] {
            for c in cs { for iv in c { for (b, cl) in [(iv.lo, iv.lo_c), (iv.hi, iv.hi_c)] { if b.is_finite() { v.push((is_x, b, cl)); } } } }
        }
        v
    }
}

/// A label's text with a white halo so it stays readable over a gridline or a curve.
fn math_text_halo(x: f64, y: f64, size: f64, s: &str) -> String { math_text_halo_in(x, y, size, s, "#000", SERIF) }
/// The same in a colour and a font: equation labels are set in the embedded Source Serif, whose italic x cannot be
/// mistaken for a lambda.
fn math_text_halo_in(x: f64, y: f64, size: f64, s: &str, fill: &str, family: &str) -> String {
    // two copies: the white halo of every glyph first, then the letters over all of it (a halo drawn glyph by glyph would
    // whiten the edge of the bracket before it)
    let (ty, body) = (y + size * 0.34, math_tspans(s));
    format!(r##"<text x="{x:.2}" y="{ty:.2}" font-size="{size}" text-anchor="start" font-family="{family}" fill="#fff" stroke="#fff" stroke-width="6" stroke-linejoin="round" paint-order="stroke">{body}</text><text x="{x:.2}" y="{ty:.2}" font-size="{size}" text-anchor="start" font-family="{family}" fill="{fill}">{body}</text>"##)
}
const LABEL_SERIF: &str = "Source Serif 4, Liberation Serif, DejaVu Serif, Times New Roman, serif";

/// Rough size of a 15 px serif label, for keeping labels apart.
fn label_w(s: &str) -> f64 { 7.2 * s.chars().filter(|c| !matches!(c, '^' | '*' | ' ')).count() as f64 + 6.0 }
/// The printed form of a typed equation: the proper inequality signs, and no multiplication
/// star between factors (x*y reads xy, 2*x reads 2x; 2*3 keeps its star).
/// The same spacing for every curve label, whatever was typed: one space round =, <, > and a binary + or -, a space after a comma.
fn tidy_spacing(s: &str) -> String {
    let cs: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect();
    let mut out = String::with_capacity(s.len() + 8);
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        let prev = i.checked_sub(1).map(|k| cs[k]);
        match c {
            '=' | '<' | '>' | '\u{2264}' | '\u{2265}' => {
                let mut op = c.to_string();
                if (c == '<' || c == '>') && cs.get(i + 1) == Some(&'=') { op.push('='); i += 1; }
                // (an = after ! belongs to it)
                out.push_str(&format!(" {op} "));
            }
            ',' => out.push_str(", "),
            '+' | '-' | '\u{2212}' => {
                let binary = prev.is_some_and(|q| q.is_alphanumeric() || q == ')' || q == '|' || q == '.') && !(matches!(prev, Some('e') | Some('E')) && i >= 2 && cs[i - 2].is_ascii_digit() && cs.get(i + 1).is_some_and(|n| n.is_ascii_digit()));
                if binary { out.push_str(&format!(" {c} ")); } else { out.push(c); }
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out
}
fn label_text(s: &str) -> String {
    let t = s.replace("<=", "\u{2264}").replace(">=", "\u{2265}");
    let cs: Vec<char> = t.chars().collect();
    let mut out = String::with_capacity(t.len());
    for (i, &c) in cs.iter().enumerate() {
        if c == '*' {
            let prev = cs[..i].iter().rev().find(|c| !c.is_whitespace()).copied();
            let next = cs[i + 1..].iter().find(|c| !c.is_whitespace()).copied();
            if let (Some(a), Some(b)) = (prev, next) {
                let digit_pair = a.is_ascii_digit() && b.is_ascii_digit();
                // x*3 must not read x3: a number after a letter or bracket keeps a visible multiplication dot
                if !digit_pair && b.is_ascii_digit() && (a.is_alphabetic() || a == ')') { out.push('\u{b7}'); continue; }
                if !digit_pair && (a.is_alphanumeric() || a == ')') && (b.is_alphanumeric() || b == '(') { continue; }
            }
        }
        out.push(c);
    }
    // a product of two variables (xy) is set as two italic letters, as in a book, not as a word
    let chars: Vec<char> = out.chars().collect();
    let mut spaced = String::with_capacity(out.len() + 4);
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_alphabetic() {
            let st = i;
            while i < chars.len() && chars[i].is_alphabetic() { i += 1; }
            let w: String = chars[st..i].iter().collect();
            if w.chars().count() == 2 && w.chars().all(|c| c == 'x' || c == 'y' || c == 'e') && w != "ee" { spaced.extend([chars[st], '\u{2009}', chars[st + 1]]); } else { spaced.push_str(&w); }
        } else { spaced.push(chars[i]); i += 1; }
    }
    spaced
}

/// Do the segments a-b and c-d cross?
fn seg_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(a, b, c), o(a, b, d), o(c, d, a), o(c, d, b));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}
/// Does the segment a-b touch the rectangle?
fn rect_seg(r: &[f64; 4], a: Pt, b: Pt) -> bool {
    if rect_has_point(r, a) || rect_has_point(r, b) { return true; }
    let (tl, tr, br, bl) = ((r[0], r[1]), (r[2], r[1]), (r[2], r[3]), (r[0], r[3]));
    seg_cross(a, b, tl, tr) || seg_cross(a, b, tr, br) || seg_cross(a, b, br, bl) || seg_cross(a, b, bl, tl)
}
fn rect_hit(r: &[f64; 4], a: &[f64; 4]) -> bool { r[0] < a[2] && a[0] < r[2] && r[1] < a[3] && a[1] < r[3] }
fn rect_has_point(r: &[f64; 4], p: Pt) -> bool { p.0 >= r[0] && p.0 <= r[2] && p.1 >= r[1] && p.1 <= r[3] }

fn poly_bbox(c: &[Pt]) -> [f64; 4] {
    c.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, p| [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)])
}

/// How many obstacles a label rectangle runs into (curves are sampled every <= 3 px).
#[cfg(test)]
fn label_conflicts(r: &[f64; 4], curves: &[Vec<Pt>], rects: &[[f64; 4]]) -> usize {
    let bbs: Vec<[f64; 4]> = curves.iter().map(|c| poly_bbox(c)).collect();
    label_conflicts_bb(r, curves, &bbs, rects)
}

/// [`label_conflicts`] with each curve's bounding box already known (the solver asks thousands of times).
fn label_conflicts_bb(r: &[f64; 4], curves: &[Vec<Pt>], bbs: &[[f64; 4]], rects: &[[f64; 4]]) -> usize {
    let mut n = rects.iter().filter(|a| rect_hit(r, a)).count() * 50;
    let g = [r[0] - 2.0, r[1] - 2.0, r[2] + 2.0, r[3] + 2.0];
    for (c, bb) in curves.iter().zip(bbs) {
        if !rect_hit(&g, bb) { continue; }
        for w in c.windows(2) {
            let (a, b) = (w[0], w[1]);
            if a.0.max(b.0) < g[0] || a.0.min(b.0) > g[2] || a.1.max(b.1) < g[1] || a.1.min(b.1) > g[3] { continue; }
            let len = (b.0 - a.0).hypot(b.1 - a.1);
            let k = (len / 3.0).ceil().max(1.0) as usize;
            if (0..=k).any(|i| { let t = i as f64 / k as f64; rect_has_point(&g, (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)) }) { n += 1; break; }
        }
    }
    n
}

/// One equation label waiting for its place; `start..end` are its own curves in the shared list.
struct LabelReq { ci: usize, text: String, default_anchor: Option<Pt>, start: usize, end: usize, halo: bool, colour: String }

/// How labels are drawn: the size, and whether every label sits on a white plate (a figure with a
/// conic always does; a plain figure keeps its old unplated labels).
#[derive(Clone, Copy)]
struct LabelStyle { size: f64, plates: bool }

/// Places every label once all curves are drawn, in row order. For each label: an explicit row first
/// tries its usual spot (so figures that were fine stay as they were); otherwise anchors are taken
/// along its own curve (preferring the end that leaves the window, the upper half and a spot away
/// from other curves), each at several offsets, and the cheapest box that clears every other label,
/// point label, axis number, curve (its own included) and the plot edge wins. If nothing is clear a
/// short leader line (dark, ending in a dot on the curve) carries the label to a free box; if even
/// that fails the least crowded spot is used. Work is bounded: at most 40 anchors per label.
/// Returns (row, svg) for each.
fn solve_labels(reqs: &[LabelReq], all: &[Vec<Pt>], placed: &mut Vec<[f64; 4]>, bounds: [f64; 4], st: LabelStyle) -> Vec<(usize, String)> {
    let mut out = vec![];
    let bbs: Vec<[f64; 4]> = all.iter().map(|c| poly_bbox(c)).collect();
    let conflicts = |b: &[f64; 4], placed: &[[f64; 4]]| label_conflicts_bb(b, all, &bbs, placed);
    let inside = |b: &[f64; 4]| b[0] >= bounds[0] + 2.0 && b[2] <= bounds[2] - 2.0 && b[1] >= bounds[1] + 2.0 && b[3] <= bounds[3] - 2.0;
    let hh = st.size * 0.6;
    // the curves that never reach the window edge (circles, ellipses ...): their labels are spread round the figure
    let exits_of = |r: &LabelReq| -> Vec<Pt> {
        all[r.start..r.end].iter().flat_map(|c| [c.first().copied(), c.last().copied()]).flatten()
            .filter(|p| (p.0 - bounds[0]).abs() < 3.0 || (p.0 - bounds[2]).abs() < 3.0 || (p.1 - bounds[1]).abs() < 3.0 || (p.1 - bounds[3]).abs() < 3.0).take(8).collect()
    };
    let closed_flags: Vec<bool> = reqs.iter().map(|r| r.end > r.start && exits_of(r).is_empty()).collect();
    let closed_total = closed_flags.iter().filter(|c| **c).count().max(1);
    for (ri, r) in reqs.iter().enumerate() {
        let closed_rank = closed_flags[..ri].iter().filter(|c| **c).count();
        let w = label_w(&r.text) * st.size / 15.0 * if st.plates { 1.12 } else { 1.0 };
        let bx = |a: Pt, ox: f64, oy: f64| [a.0 + ox, a.1 + oy - hh, a.0 + ox + w, a.1 + oy + hh];
        let emit_with = |x: f64, y: f64, plated: bool| -> String {
            if plated {
                format!(r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="2" fill="#fff" fill-opacity="0.9"/>{}"##, x - 2.0, y - hh - 0.5, w, 2.0 * hh + 1.0, if st.plates { math_text_halo_in(x, y, st.size, &r.text, &r.colour, LABEL_SERIF) } else { math_text_halo(x, y, st.size, &r.text) })
            } else { math_text_str(x, y, st.size, "start", &r.text, false) }
        };
        let emit_home = |x: f64, y: f64| emit_with(x, y, st.plates);
        let emit = |x: f64, y: f64| emit_with(x, y, st.plates || r.halo);
        // 1. the explicit row's usual spot
        if let Some(a) = r.default_anchor {
            let b = bx(a, 8.0, -12.0);
            if inside(&b) && conflicts(&b, placed) == 0 {
                placed.push(b);
                out.push((r.ci, emit_home(a.0 + 8.0, a.1 - 12.0)));
                continue;
            }
        }
        // 2. anchors along its own curves
        let mine = &all[r.start..r.end];
        let exits: Vec<Pt> = exits_of(r);
        let closed = closed_flags[ri];
        // (a closed curve's centre, and the direction its label prefers: the labels of several rings fan out)
        let (ccx, ccy, npts) = mine.iter().flatten().fold((0.0, 0.0, 0usize), |a, q| (a.0 + q.0, a.1 + q.1, a.2 + 1));
        let centre = (ccx / npts.max(1) as f64, ccy / npts.max(1) as f64);
        let pref = -(35.0f64.to_radians() + closed_rank as f64 * std::f64::consts::TAU / closed_total as f64);
        let mid_y = 0.5 * (bounds[1] + bounds[3]);
        let target_x = bounds[0] + (bounds[2] - bounds[0]) * (0.55 + 0.08 * (r.ci % 3) as f64);
        let mut cands: Vec<(Pt, f64)> = vec![];
        for c in mine {
            let mut acc = 0.0;
            for (k, pt) in c.iter().enumerate() {
                if k > 0 { acc += (pt.0 - c[k - 1].0).hypot(pt.1 - c[k - 1].1); }
                if k > 0 && acc < 5.0 { continue; }
                acc = 0.0;
                if pt.0 < bounds[0] + 6.0 || pt.0 > bounds[2] - 6.0 || pt.1 < bounds[1] + 6.0 || pt.1 > bounds[3] - 6.0 { continue; }
                let near_exit = exits.iter().map(|e| (e.0 - pt.0).hypot(e.1 - pt.1)).fold(f64::INFINITY, f64::min);
                let mut sc = if closed {
                    let dv = ((pt.1 - centre.1).atan2(pt.0 - centre.0) - pref).rem_euclid(std::f64::consts::TAU);
                    dv.min(std::f64::consts::TAU - dv) / std::f64::consts::PI
                } else if exits.is_empty() { (pt.0 - target_x).abs() / (bounds[2] - bounds[0]) } else { near_exit / (bounds[2] - bounds[0] + bounds[3] - bounds[1]) * 2.0 };
                if pt.1 > mid_y && !closed { sc += 0.3; }
                cands.push((*pt, sc));
                if cands.len() > 4000 { break; }
            }
        }
        cands.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut chosen: Vec<(Pt, f64)> = vec![];
        for (a, sc) in cands {
            if chosen.iter().any(|c| (c.0.0 - a.0).hypot(c.0.1 - a.1) < 22.0) { continue; }
            // away from crossings with other rows' curves
            let near_other = all[..r.start].iter().chain(all[r.end..].iter()).zip(bbs[..r.start].iter().chain(bbs[r.end..].iter()))
                .any(|(c, bb)| a.0 > bb[0] - 14.0 && a.0 < bb[2] + 14.0 && a.1 > bb[1] - 14.0 && a.1 < bb[3] + 14.0 && c.iter().any(|q| (q.0 - a.0).hypot(q.1 - a.1) < 14.0));
            chosen.push((a, sc + if near_other { 0.5 } else { 0.0 }));
            if chosen.len() >= 40 { break; }
        }
        chosen.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let offs = [(8.0, -12.0), (-w - 8.0, -12.0), (8.0, 14.0), (-w - 8.0, 14.0), (-0.5 * w, -17.0), (-0.5 * w, 23.0)];
        let mut best: Option<(f64, Pt, f64, f64)> = None;
        for (a, sc) in &chosen {
            for (k, (ox, oy)) in offs.iter().enumerate() {
                let b = bx(*a, *ox, *oy);
                if !inside(&b) || conflicts(&b, placed) > 0 { continue; }
                let cost = sc + 0.04 * k as f64;
                if best.map_or(true, |t| cost < t.0) { best = Some((cost, *a, *ox, *oy)); }
            }
        }
        if let Some((_, a, ox, oy)) = best {
            placed.push(bx(a, ox, oy));
            out.push((r.ci, emit(a.0 + ox, a.1 + oy)));
            continue;
        }
        // 3. a leader line to a free box: the cheapest (shortest, then fewest crossings of other curves or labels) in
        //    eight directions at growing distances
        let mut found: Option<(f64, Pt, [f64; 4])> = None;
        for dist in [34.0, 48.0, 64.0, 84.0, 108.0, 136.0, 170.0, 210.0] {
            for (a, _) in chosen.iter().take(16) {
                for k in 0..9 {
                    // (the ninth direction is straight out of a closed curve, the cleanest way across its neighbours)
                    let ang = if k < 8 { k as f64 * std::f64::consts::FRAC_PI_4 } else if closed { (a.1 - centre.1).atan2(a.0 - centre.0) } else { continue };
                    let (cx, cy) = (a.0 + dist * ang.cos(), a.1 + dist * ang.sin());
                    let b = [cx - 0.5 * w, cy - hh, cx + 0.5 * w, cy + hh];
                    if !inside(&b) || conflicts(&b, placed) > 0 { continue; }
                    let end = (a.0.clamp(b[0], b[2]), a.1.clamp(b[1], b[3]));
                    let mut cross = 0usize;
                    for (ci2, (c, bb)) in all.iter().zip(&bbs).enumerate() {
                        if ci2 >= r.start && ci2 < r.end { continue; }
                        if end.0.min(a.0) > bb[2] || end.0.max(a.0) < bb[0] || end.1.min(a.1) > bb[3] || end.1.max(a.1) < bb[1] { continue; }
                        cross += c.windows(2).filter(|w2| seg_cross(*a, end, w2[0], w2[1])).count().min(3);
                    }
                    cross += placed.iter().filter(|pb| rect_seg(pb, *a, end)).count() * 2;
                    let cost = dist + 300.0 * cross as f64;
                    if found.map_or(true, |f| cost < f.0) { found = Some((cost, *a, b)); }
                }
            }
            if found.is_some_and(|f| f.0 < dist + 1.0) { break; }
        }
        if let Some((_, a, b)) = found {
            placed.push(b);
            // the leader runs from a dot on the curve to the nearest edge point of the box
            let end = (a.0.clamp(b[0], b[2]), a.1.clamp(b[1], b[3]));
            out.push((r.ci, format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{lc}" stroke-width="1.2"/><circle cx="{:.2}" cy="{:.2}" r="2.6" fill="{lc}"/>{}"##, a.0, a.1, end.0, end.1, a.0, a.1, emit(b[0], 0.5 * (b[1] + b[3])), lc = r.colour)));
            continue;
        }
        // 4. nothing is free: the least crowded
        let mut least: Option<(usize, Pt, f64, f64)> = None;
        for (a, _) in chosen.iter().take(20) {
            for (ox, oy) in offs {
                let b = bx(*a, ox, oy);
                let n = conflicts(&b, placed) + if inside(&b) { 0 } else { 100 };
                if least.map_or(true, |t| n < t.0) { least = Some((n, *a, ox, oy)); }
            }
        }
        if let Some((_, a, ox, oy)) = least { placed.push(bx(a, ox, oy)); out.push((r.ci, emit(a.0 + ox, a.1 + oy))); }
    }
    out
}

/// Hard cap on gridlines, ticks and numbers per axis (a window and step that would make more is drawn cut off).
const MAX_LINES: usize = 2100;
/// At most this many non-linear implicit rows are drawn per figure (bounds the sampling work).
/// Most rows drawn (the rest are ignored with one note); a typed row longer than this or nested deeper is not parsed.
pub(super) const MAX_ROWS: usize = 40;
/// Most points drawn (and given drag handles).
pub(super) const MAX_POINTS: usize = 500;
/// The tint of the common solution of several inequalities (no row's own colour, so it reads as the answer).
const GROUP_TINT: &str = "#f2a900";
/// Opacity of that tint (clear over white, and the blue, red and green boundaries read on it).
const GROUP_TINT_OPACITY: f64 = 0.30;
/// A scientific literal directly after a ^ (`x^2e3`): it is not read as one number; the literal is returned.
fn caret_sci(raw: &str) -> Option<String> {
    let ch: Vec<char> = raw.chars().collect();
    for i in 1..ch.len() {
        if ch[i] != '^' { continue; }
        let mut j = i + 1;
        while j < ch.len() && ch[j].is_whitespace() { j += 1; }
        let st = j;
        while j < ch.len() && (ch[j].is_ascii_digit() || ch[j] == '.') { j += 1; }
        if j > st && j < ch.len() && (ch[j] == 'e' || ch[j] == 'E') {
            let mut k = j + 1;
            if k < ch.len() && (ch[k] == '+' || ch[k] == '-' || ch[k] == '\u{2212}') { k += 1; }
            if k < ch.len() && ch[k].is_ascii_digit() {
                while k < ch.len() && ch[k].is_ascii_digit() { k += 1; }
                return Some(ch[st..k].iter().collect());
            }
        }
    }
    None
}
/// True when F is of one sign over a window and much larger ones round it (so x^2 + y^2 = -4 has no real points anywhere,
/// where x^2 + y^2 = 400 only has none in a small window). `positive`: for an inequality, the sign it asks for.
fn no_solutions(f: &dyn Fn(f64, f64) -> Option<f64>, (x0, x1, y0, y1): (f64, f64, f64, f64), positive: Option<bool>, strict: bool) -> bool {
    let (cx, cy, hx, hy) = (0.5 * (x0 + x1), 0.5 * (y0 + y1), 0.5 * (x1 - x0), 0.5 * (y1 - y0));
    let (mut any, mut pos, mut neg) = (false, false, false);
    for m in [1.0, 4.0, 16.0, 100.0, 1e4, 1e6] {
        for i in 0..=40 {
            for j in 0..=40 {
                let (x, y) = (cx + hx * m * (i as f64 / 20.0 - 1.0), cy + hy * m * (j as f64 / 20.0 - 1.0));
                if let Some(v) = f(x, y).filter(|v| v.is_finite()) {
                    any = true;
                    // (a zero satisfies an equation or a <= but not a strict inequality: x^2 < 0 holds nowhere)
                    if v == 0.0 { if strict { continue; } return false; }
                    if v > 0.0 { pos = true; } else { neg = true; }
                }
            }
        }
    }
    any && match positive { None => !(pos && neg), Some(true) => !pos, Some(false) => !neg }
}
/// A straight row: y = m x + c, or the vertical x = k.
#[derive(Clone, Copy)]
enum Lin { Slope(f64, f64), Vert(f64) }
/// The line a function of x is, when it is one (collinear at seven x values).
fn line_fit(f: &dyn Fn(f64) -> Option<f64>) -> Option<Lin> {
    let (v0, v1) = (f(0.0)?, f(1.0)?);
    let m = v1 - v0;
    for x in [-1.0, 3.0, -7.5, 40.0, -123.0] {
        let v = f(x)?;
        if !v.is_finite() || (v - (v0 + m * x)).abs() > 1e-9 * (1.0 + v.abs() + (m * x).abs()) { return None; }
    }
    (v0.is_finite() && m.is_finite()).then_some(Lin::Slope(m, v0))
}
/// A curve the straight rows may be asymptotes of: an implicit F(x, y) = 0 (conics), or one that can be solved for y.
enum Curve { Imp(Vec<char>), Func(String), Solved(Vec<char>) }
fn curve_y(c: &Curve, x: f64, deg: bool) -> Option<f64> {
    match c {
        Curve::Func(r) => eval_full(r, x, None, deg),
        Curve::Solved(cs) => { let (a, b) = (eval_cs(cs, x, Some(0.0), deg)?, eval_cs(cs, x, Some(1.0), deg)?); let sl = b - a; (sl.abs() > 1e-12).then(|| -a / sl) }
        Curve::Imp(_) => None,
    }
}
/// Is the line an asymptote of the curve? The curve has to come ever closer to it: along the line the distance to the curve
/// (|F| / |grad F| for F = 0, or the gap to the line for y = g(x), or g running off for x = a) must shrink, so a parallel line
/// ((x - y)^2 = 4 and y = x) or a tangent is not one.
fn asymptote_of(c: &Curve, l: Lin, deg: bool) -> bool {
    match c {
        Curve::Imp(cs) => {
            let at = |t: f64| match l { Lin::Slope(m, k) => (t, m * t + k), Lin::Vert(a) => (a, t) };
            let dist = |t: f64| -> Option<f64> {
                let (px, py) = at(t);
                let h = 1e-4 * (1.0 + t.abs());
                let f = |x: f64, y: f64| eval_cs(cs, x, Some(y), deg).filter(|v| v.is_finite());
                let (v, fx, fy) = (f(px, py)?, (f(px + h, py)? - f(px - h, py)?) / (2.0 * h), (f(px, py + h)? - f(px, py - h)?) / (2.0 * h));
                let g = fx.hypot(fy);
                (g > 1e-12).then(|| v.abs() / g)
            };
            [1.0, -1.0].into_iter().any(|sg| {
                let Some(d10) = dist(10.0 * sg) else { return false };
                let (Some(d1k), Some(d10k)) = (dist(1000.0 * sg), dist(10000.0 * sg)) else { return false };
                d10 > 1e-9 && d1k <= 0.2 * d10 && d10k <= 0.5 * d1k + 1e-12
            })
        }
        _ => {
            let g = |x: f64| curve_y(c, x, deg).filter(|v| v.is_finite());
            match l {
                Lin::Vert(a) => [1.0, -1.0].into_iter().any(|sg| {
                    let v: Vec<Option<f64>> = [1e-2, 1e-4, 1e-6].iter().map(|dx| g(a + sg * dx).map(f64::abs)).collect();
                    // (a pole, or a logarithm that grows without bound at an even pace)
                    matches!((v[0], v[1], v[2]), (Some(p), Some(q), Some(r)) if (q > 5.0 * p && r > 5.0 * q && r > 1e4) || (q > p + 1.0 && r > q + 1.0 && r > 10.0)) && g(a).is_none()
                }),
                Lin::Slope(m, k) => [1.0, -1.0].into_iter().any(|sg| {
                    let r = |t: f64| g(sg * t).map(|v| (v - (m * sg * t + k)).abs());
                    matches!((r(1e2), r(1e3), r(1e5)), (Some(a), Some(b), Some(e)) if a > 1e-9 && b <= 0.5 * a && e <= 0.02 * a && e < 0.01)
                }),
            }
        }
    }
}
/// The sampling grid for a row of `chars` characters: `(nx, ny)` cut down together until the work fits `share`.
fn scaled_cells((nx, ny): (usize, usize), chars: usize, share: u64) -> (usize, usize) {
    let cost = (chars as u64 + 8) * nx as u64 * ny as u64;
    if cost <= share { return (nx, ny); }
    let k = (share as f64 / cost as f64).sqrt();
    (((nx as f64 * k) as usize).max(64).min(nx), ((ny as f64 * k) as usize).max(64).min(ny))
}
fn readable_len(raw: &str) -> bool { raw.len() <= 200 && nest_depth(raw.chars().collect::<Vec<_>>().iter()) <= 30 }
/// A member of the system of inequalities that shades once: F as chars, shade where F > 0 (or an interval set of x).
struct GroupMember { ci: usize, cs: Vec<char>, pos: bool, dom: ImpDom, band: Option<Vec<(f64, f64)>> }

/// A trailing line-style word on a row ("y = 2x/3 dashed"): the row without it and its dash pattern.
fn strip_style(raw: &str) -> (&str, Option<&'static str>) {
    let t = raw.trim_end();
    let lower = t.to_ascii_lowercase();
    for (kw, dash) in [("dash-dot", "9 4 2 4"), ("asymptote", "asym"), ("dashed", "9 6"), ("dotted", "2 4"), ("solid", "none")] {
        if lower.ends_with(kw) {
            let cut = t.len() - kw.len();
            let head = t[..cut].trim_end();
            // something must precede the word: "y = dashed" is an equation (an unknown name), not a style
            if cut > 0 && t[..cut].ends_with(char::is_whitespace) && head.chars().last().is_some_and(|c| c.is_alphanumeric() || c == ')' || c == '.') { return (head, Some(dash)); }
        }
    }
    (raw, None)
}
/// [`strip_style`] on the end of the row, or on the equation before its domain (`x^2 + y^2 = 25 dashed, x > 0`):
/// the row without the word, and its dash pattern.
fn unstyle(raw: &str) -> (String, Option<&'static str>) {
    let (t, d) = strip_style(raw.trim());
    if d.is_some() { return (t.to_string(), d); }
    let (main, dom) = split_domain(t);
    let (m2, d2) = strip_style(main);
    match (d2, dom) { (Some(dd), Some(dm)) => (format!("{}, {}", m2.trim(), dm.trim()), Some(dd)), (Some(dd), None) => (m2.to_string(), Some(dd)), _ => (t.to_string(), None) }
}
/// The looser bound for the straight-line test (which samples the row 400 times): padded linear rows still draw.
fn cheap_len(raw: &str) -> bool { raw.len() <= 1000 && nest_depth(raw.chars().collect::<Vec<_>>().iter()) <= 30 }
const IMPLICIT_MAX: usize = 4;
/// Work one figure may spend evaluating typed rows, in characters read ((row length + 8) x cells sampled): about two seconds.
/// Rows past it are skipped with a note.
const WORK_MAX: u64 = 40_000_000;
/// The most one row may cost: a longer row is sampled on a coarser grid (never fewer than 64 cells a side).
const ROW_WORK_MAX: u64 = 12_000_000;
/// The share of that for the common solution of the inequalities (members that would not fit shade on their own).
const GROUP_WORK_MAX: u64 = 20_000_000;
/// At most this many implicit rows are even tried (each costs a full sample, drawn or not).
const IMPLICIT_TRY: usize = 8;
/// Cap on the SVG the implicit rows of one figure may add.
const IMP_BYTES_MAX: usize = 3_000_000;
/// Drops points that lie within 0.04 px of the polyline kept (Douglas-Peucker: every dropped point is that close to the
/// final line, however many were dropped in a row).
/// The Print hatch: diagonal lines about 8 px apart, drawn straight into an axis-aligned tile (no rotated pattern, which
/// is resampled and loses its darkness) so they are as dark on a photocopy as on screen. `rising`: the lines climb to the right.
fn print_hatch(id: &str, rising: bool) -> String {
    let path = if rising { "M0,11 L11,0 M-1,1 L1,-1 M10,12 L12,10" } else { "M0,0 L11,11 M-1,10 L1,12 M10,-1 L12,1" };
    format!(r##"<defs><pattern id="{id}" patternUnits="userSpaceOnUse" width="11" height="11"><path d="{path}" fill="none" stroke="#555555" stroke-width="2"/></pattern></defs>"##)
}
/// The stretches of a polyline that lie along an axis (within 2 px of it): an excluded boundary there is cut out of the axis, so its
/// dashes show.
fn axis_runs(pts: &[Pt], axx: f64, axy: f64) -> Vec<Vec<Pt>> {
    let mut out: Vec<Vec<Pt>> = vec![];
    for horizontal in [true, false] {
        let on = |q: &Pt| if horizontal { (q.1 - axy).abs() < 2.0 } else { (q.0 - axx).abs() < 2.0 };
        let mut run: Vec<Pt> = vec![];
        for q in pts {
            if on(q) { run.push(*q); } else { if run.len() > 1 { out.push(std::mem::take(&mut run)); } run.clear(); }
        }
        if run.len() > 1 { out.push(run); }
    }
    // (a curve only crossing or brushing the axis is no boundary along it: a stretch must be at least 12 px long)
    out.retain(|r| { let (a, b) = (r[0], r[r.len() - 1]); (a.0 - b.0).abs().max((a.1 - b.1).abs()) >= 12.0 });
    out
}
fn thin_polyline(p: &[Pt]) -> Vec<Pt> {
    if p.len() < 3 { return p.to_vec(); }
    let mut keep = vec![false; p.len()];
    keep[0] = true;
    keep[p.len() - 1] = true;
    let mut stack = vec![(0usize, p.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 { continue; }
        let (dx, dy) = (p[b].0 - p[a].0, p[b].1 - p[a].1);
        let len = dx.hypot(dy);
        let (mut worst, mut at) = (0.0f64, a);
        for i in a + 1..b {
            let dist = if len < 1e-9 { (p[i].0 - p[a].0).hypot(p[i].1 - p[a].1) } else { ((p[i].0 - p[a].0) * dy - (p[i].1 - p[a].1) * dx).abs() / len };
            if dist > worst { worst = dist; at = i; }
        }
        if worst > 0.04 { keep[at] = true; stack.push((a, at)); stack.push((at, b)); }
    }
    p.iter().zip(keep).filter(|(_, k)| *k).map(|(q, _)| *q).collect()
}

const NUM_EVERY_LINE: [&str; 5] = ["Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"];

fn coordinate_grid_params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("xmin", "x-axis minimum (from)", -1000.0, 1000.0, 1.0, 0.0),
        ParamDef::float("xmax", "x-axis maximum (to)", -1000.0, 1000.0, 1.0, 15.0),
        ParamDef::float("xstep", "x-axis step (count by)", 0.05, 180.0, 0.5, 1.0),
        ParamDef::float("ymin", "y-axis minimum (from)", -1000.0, 1000.0, 1.0, 0.0),
        ParamDef::float("ymax", "y-axis maximum (to)", -1000.0, 1000.0, 1.0, 15.0),
        ParamDef::float("ystep", "y-axis step (count by)", 0.05, 180.0, 0.5, 1.0),
        ParamDef::int("minor", "Minor gridlines per step (0 = none)", 0, 10, 0),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::bool("arrows", "Arrowheads on axes", true),
        ParamDef::bool("square", "Square cells", true),
        ParamDef::text("xlabel", "x-axis label (at the arrow; empty = none)", "x"),
        ParamDef::text("ylabel", "y-axis label (at the arrow; empty = none)", "y"),
        ParamDef::text("title", "Title", ""),
        ParamDef::long("equations", "Equations, one per ; - y = 2x + 1; 2x + 3y = 6; x = 4; y > x - 2; -2 < x <= 3; y = x + 2, x < 1 (piece with a domain); x^2 + y^2 = 25 (circle or any conic, x^2 + y^2 < 25 shades it, x^2 + y^2 = 25, y > 0 keeps the top half)", "y = 2x + 1"),
        ParamDef::bool("eq_labels", "Write each equation on its line", false),
        ParamDef::long("points", "Points - A(1, 2), B(-3, 4)  or  (2,3) A; (-1,4) B", ""),
        ParamDef::color("eq1_color", "Equation 1 colour", &SWATCHES, "#1f5fbf"),
        ParamDef::color("eq2_color", "Equation 2 colour", &SWATCHES, "#c0392b"),
        ParamDef::color("eq3_color", "Equation 3 colour", &SWATCHES, "#2a8a3e"),
        ParamDef::color("eq4_color", "Equation 4 colour", &SWATCHES, "#8e44ad"),
        ParamDef::color("pt_color", "Point colour", &SWATCHES, "#111111"),
        ParamDef::choice("title_mode", "Chart title", &TITLE_MODES, "Text"),
        ParamDef::choice("xtitle_mode", "x-axis title", &TITLE_MODES, "None"),
        ParamDef::text("xtitle", "x-axis title text", "Time (s)"),
        ParamDef::choice("ytitle_mode", "y-axis title", &TITLE_MODES, "None"),
        ParamDef::text("ytitle", "y-axis title text", "Distance (m)"),
        ParamDef::choice("xnum", "x-axis numbers", &NUM_EVERY_LINE, "Every line"),
        ParamDef::choice("ynum", "y-axis numbers", &NUM_EVERY_LINE, "Every line"),
        ParamDef::choice("xunits", "x-axis units (range and step typed in these)", &["Numbers", "Multiples of \u{3c0}", "Degrees"], "Numbers"),
        ParamDef::choice("grid", "Gridlines (bold for photocopies)", &["Light", "Standard", "Bold", "None"], "Standard"),
        ParamDef::choice("line_width", "Graph line thickness", &["Thin", "Medium", "Thick"], "Medium"),
        ParamDef::choice("shading", "Inequality shading", &["Tint", "Hatching"], "Tint"),
        ParamDef::bool("asymptotes", "Show vertical asymptotes (dotted)", false),
        ParamDef::bool("endpoints", "Endpoint circles on pieces with a domain", true),
        ParamDef::choice("pt_style", "Point style", &["Dot", "Cross"], "Dot"),
        ParamDef::choice("pt_label", "Point labels", &["Letter", "Letter and coordinates", "None"], "Letter"),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        // Print: black curves told apart by dash pattern, hatched shading; show_answers off hides the equation labels and shading (student copy)
        super::svg::style_param(),
        super::answers_param(),
        ParamDef::bool("student_shading", "Keep the shading on the student copy (when the question asks students to identify the region)", false),
        ParamDef::bool("show_notes", "Write notes on the figure (a curve outside the window, rows that could not be drawn)", false),
    ]
}

/// The reference's three label sizes scale every label of the coordinate grid and the number
/// line by 1.4 / 1 / 0.8 (Large / Medium / Small).
fn lscale(p: &Params) -> f64 { match p.s("label_size") { "Large" => 1.4, "Small" => 0.8, _ => 1.0 } }

const PALETTE: [&str; 5] = ["#1f5fbf", "#c0392b", "#2a8a3e", "#8e44ad", "#d68910"];

/// Grid layout shared with the drag handles (math_plot::Cg): model ranges (x already in real
/// units, i.e. multiplied by pi in pi mode), scales, margins and plot size.
pub(super) struct GridL { pub x0: f64, pub x1: f64, pub y0: f64, pub y1: f64, pub xs: f64, pub ys: f64, pub ux: f64, pub uy: f64, pub ml: f64, pub mt: f64, pub pw: f64, pub ph: f64 }
pub(super) fn grid_layout(p: &Params) -> GridL {
    let k = if p.s("xunits") == "Multiples of \u{3c0}" { PI } else { 1.0 };
    let (x0, mut x1, y0, mut y1) = (p.f("xmin") * k, p.f("xmax") * k, p.f("ymin"), p.f("ymax"));
    if x1 <= x0 { x1 = x0 + k; }
    if y1 <= y0 { y1 = y0 + 1.0; }
    let (xs, ys) = (p.f("xstep").max(1e-6) * k, p.f("ystep").max(1e-6));
    // the reference's 15 x 15 grid is 480 px square (32 px a unit)
    let (maxw, maxh) = (480.0, 480.0);
    let (ux, uy) = if p.b("square") && k == 1.0 && p.s("xunits") != "Degrees" {
        let u = (maxw / (x1 - x0)).min(maxh / (y1 - y0)); (u, u)
    } else { (maxw / (x1 - x0), maxh / (y1 - y0)) };
    let (pw, ph) = ((x1 - x0) * ux, (y1 - y0) * uy);
    let title = match p.s("title_mode") { "None" => false, "Blank line" => true, _ => !p.s("title").is_empty() };
    // the margins grow with the labels: 1.2 / 1.3 px for each px the tick numbers grow
    let dn = 14.0 * (lscale(p) - 1.0);
    let ml = 38.8 + 1.2 * dn + if p.s("ytitle_mode") == "Text" || p.s("ytitle_mode") == "Blank line" { 26.0 } else { 0.0 };
    GridL { x0, x1, y0, y1, xs, ys, ux, uy, ml, mt: (if title { 88.2 } else { 56.2 }) + 1.3 * dn, pw, ph }
}

/// "3π/4"-style label for v (in real units) when the axis counts in multiples of π.
fn pi_label(v: f64) -> String {
    let r = v / PI;
    if r.abs() < 1e-9 { return "0".into(); }
    for den in 1..=12i64 {
        let num = r * den as f64;
        if (num - num.round()).abs() < 1e-6 {
            let n = num.round() as i64;
            let sign = if n < 0 { "\u{2212}" } else { "" };
            let a = n.abs();
            let top = if a == 1 { "\u{3c0}".to_string() } else { format!("{a}\u{3c0}") };
            return if den == 1 { format!("{sign}{top}") } else { format!("{sign}{top}/{den}") };
        }
    }
    fmt(v, 2)
}
fn every_n(s: &str) -> i64 { match s { "Every 2nd line" => 2, "Every 5th line" => 5, "Every 10th line" => 10, "No numbers" => 0, _ => 1 } }

/// One typed row of the Equations box.
enum Row { Func(String), Vert(f64), Implicit(String), ImplicitIneq { e: String, positive: bool, strict: bool, edges: Option<[(String, bool); 2]> }, Ineq { rhs: String, above: bool, strict: bool }, Band(Vec<Interval>) }
/// Splits "y = x + 2, x < 1" into (main, domain) at the first comma outside brackets.
fn split_domain(s: &str) -> (&str, Option<&str>) {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c { '(' => depth += 1, ')' => depth -= 1, ',' if depth == 0 => return (&s[..i], Some(&s[i + 1..])), _ => {} }
    }
    (s, None)
}
/// The leftmost comparison sign in `s` (position, length): `<=` and `>=` are one sign.
fn first_cmp(s: &str) -> Option<(usize, usize)> {
    let b = s.as_bytes();
    let i = b.iter().position(|c| *c == b'<' || *c == b'>')?;
    Some((i, if b.get(i + 1) == Some(&b'=') { 2 } else { 1 }))
}
fn parse_row(main: &str) -> Option<Row> {
    let m = main.replace('\u{2264}', "<=").replace('\u{2265}', ">=").replace('\u{2212}', "-");
    let lower = m.to_lowercase();
    if lower.contains('<') || lower.contains('>') {
        if !lower.contains('y') {
            let ivs = parse_ineq(&lower);
            // x^2 < 0, 1 < x^2 < 4: no bare x to read limits from, so it is an inequality in x alone (shaded from its sign like the rest)
            if !(ivs.is_empty() && lower.contains('x')) { return Some(Row::Band(ivs)); }
        }
        let (pos, len) = first_cmp(&lower)?;
        let op = &lower[pos..pos + len];
        let (l, r) = (lower[..pos].trim(), lower[pos + len..].trim());
        let strict = !op.contains('=');
        // a chain such as 4 < x^2 + y^2 < 25 (or >, <=, >= both ways): the middle part lies between the
        // two ends exactly where (E - lo)(hi - E) > 0, so it is shaded as one implicit inequality
        if let Some((p2, l2)) = first_cmp(r) {
            let op2 = &r[p2..p2 + l2];
            if op2.starts_with('>') == op.starts_with('>') {
                let (mid, far) = (r[..p2].trim(), r[p2 + l2..].trim());
                let (lo, hi) = if op.starts_with('<') { (l, far) } else { (far, l) };
                let strict2 = strict || !op2.contains('=');
                // each limit keeps its own strictness: the first sign binds the limit before the middle, the second the one after it
                let (s1, s2) = (strict, !op2.contains('='));
                let (lo_strict, hi_strict) = if op.starts_with('<') { (s1, s2) } else { (s2, s1) };
                let ordered = match (eval(lo, 0.0), eval(hi, 0.0)) { (Some(a), Some(b)) => a < b || (a == b && !strict2), _ => true };
                let e = if ordered { format!("(({mid})-({lo}))*(({hi})-({mid}))") } else { "-1".to_string() };
                let edges = ordered.then(|| [(format!("({mid})-({lo})"), lo_strict), (format!("({hi})-({mid})"), hi_strict)]);
                return Some(Row::ImplicitIneq { e, positive: true, strict: strict2, edges });
            }
        }
        let gt = op.starts_with('>');
        // y < f(x) (no y on the other side) shades above or below the curve; anything else with a y
        // (x^2 + y^2 < 25, y > x + y^2) is shaded from the sampled sign of left - right
        return if l == "y" && !r.contains('y') { Some(Row::Ineq { rhs: r.to_string(), above: gt, strict }) }
            else if r == "y" && !l.contains('y') { Some(Row::Ineq { rhs: l.to_string(), above: !gt, strict }) }
            else { Some(Row::ImplicitIneq { e: format!("({l})-({r})"), positive: gt, strict, edges: None }) };
    }
    match lower.find('=') {
        Some(i) => {
            let (l, r) = (lower[..i].trim(), lower[i + 1..].trim());
            // x = 4 is a vertical line; x = y^2 (a y on the right, or anything eval cannot read) is implicit
            if l == "x" { Some(eval(r, 0.0).map(Row::Vert).unwrap_or_else(|| Row::Implicit(format!("({l})-({r})")))) }
            else if (l == "y" || l == "f(x)") && !r.contains('y') { Some(Row::Func(r.to_string())) }
            else if r == "y" && !l.contains('y') { Some(Row::Func(l.to_string())) }
            else { Some(Row::Implicit(format!("({l})-({r})"))) }
        }
        None => Some(Row::Func(lower.trim().to_string())),
    }
}

/// True when a typed row is a conic or other implicit curve (not a line in y), which has no drag handles.
pub(super) fn is_curve_row(raw: &str, x0: f64, x1: f64, y0: f64, y1: f64, deg: bool) -> bool {
    // a pathological string has no handles (and is never parsed here)
    if !cheap_len(raw) { return true; }
    let (raw, _) = unstyle(raw);
    let (main, _) = split_domain(raw.trim());
    match parse_row(main) {
        Some(Row::Implicit(e)) => { let cs = norm_expr(&e); !implicit::linear_in_y(&|x, y| eval_cs(&cs, x, Some(y), deg), x0, x1, y0, y1) }
        Some(Row::ImplicitIneq { .. }) => true,
        _ => false,
    }
}

/// A window so much wider than tall that Square cells squeezes the plot into a strip.
fn put_shade(d: &mut Doc, shade: &mut String, late: bool, s: String) { if late { shade.push_str(&s); } else { d.raw(&s); } }
/// The blue-tinted inks of the colour figure as neutral greys (equal red, green and blue), for the Print style.
fn greyify(svg: &str) -> String {
    let mut s = svg.to_string();
    for (from, to) in [("#111827", "#111111"), ("#6b7280", "#6b6b6b"), ("#9ca3af", "#9a9a9a"), ("#dfe3ea", "#dedede"), ("#888b93", "#888888"), ("#e6ebf1", "#e8e8e8"), ("#dde3ea", "#dddddd"), ("#eef1f5", "#eeeeee"), ("#6f7d8c", "#707070"), ("#b9c2cc", "#bbbbbb")] { s = s.replace(from, to); }
    s
}
fn wide_flat(p: &Params, g: &GridL) -> bool { p.b("square") && g.pw / g.ph.max(1.0) > 6.0 }

fn coordinate_grid_render(p: &Params) -> String { coordinate_grid_build(p, &mut vec![]) }

/// Problems with the typed equations that only the editor shows (a curve that cannot be read, too
/// many curves, a curve outside the window); they are never drawn into the figure, so exports
/// stay clean.
pub fn grid_warnings(p: &Params) -> Vec<String> {
    let mut notes = vec![];
    let _ = coordinate_grid_build(p, &mut notes);
    notes
}

fn coordinate_grid_build(p: &Params, notes: &mut Vec<String>) -> String {
    // like the UI, clamp every control first: the render is safe even for a spec that never was
    let mut pn = p.clone();
    pn.normalize(&coordinate_grid_params(), false);
    let p = &pn;
    let g = grid_layout(p);
    let (x0, x1, y0, y1, xs, ys, ux, uy, ml, mt, pw, ph) = (g.x0, g.x1, g.y0, g.y1, g.xs, g.ys, g.ux, g.uy, g.ml, g.mt, g.pw, g.ph);
    let pi_mode = p.s("xunits") == "Multiples of \u{3c0}";
    let deg = p.s("xunits") == "Degrees";
    let fills = super::svg::Fills::new(p);
    let print = fills.is_print();
    let answers = p.b("show_answers");
    // (the student copy hides the shading too, unless the question asks students to find it)
    let shade_on = answers || p.b("student_shading");
    let has_conic = p.s("equations").split(';').take(MAX_ROWS).any(|r| { let r = r.trim(); !r.is_empty() && is_curve_row(r, x0, x1, y0, y1, deg) });
    // beside a conic (or in Print) the shading goes under the gridlines and the tick numbers over the curves, so
    // neither the hatching nor a curve can hide a number
    let late = has_conic || print;
    let mut shade = String::new();
    if p.b("numbers") && (xs * ux < 2.0 && every_n(p.s("xnum")) == 1 || ys * uy < 2.0 && every_n(p.s("ynum")) == 1) {
        notes.push("A line every pixel or two cannot be numbered: choose Every 10th line for the axis numbers, or a larger step".to_string());
    }
    if wide_flat(p, &g) { notes.push("The window is much wider than it is tall, so Square cells squeezes the plot into a strip: turn off Square cells".to_string()); }
    let xtitle = p.s("xtitle_mode") == "Text" || p.s("xtitle_mode") == "Blank line";
    let ls = lscale(p);
    let dn = 14.0 * (ls - 1.0);
    let (mr, mb) = (52.5 + 0.75 * dn, 36.0 + dn + if xtitle { 26.0 } else { 0.0 });
    let (w, h) = (pw + ml + mr, ph + mt + mb);
    let mut d = new_doc(w, h);
    let sx = |x: f64| ml + (x - x0) * ux;
    let sy = |y: f64| mt + ph - (y - y0) * uy;
    let f = |expr: &str, x: f64| eval_full(expr, x, None, deg);
    let minor = p.i("minor");
    // (a window already squeezed into a strip by Square cells is only warned about, as it always has been)
    let squeezed = p.b("square") && { let r = pw / ph.max(1.0); !(1.0 / 6.0..=6.0).contains(&r) };
    // major lines closer than 6 px smear into a block: then only every 2nd, 5th, 10th ... line is drawn; minor lines under 4 px are left out
    let stride = |unit_px: f64| -> i64 { if !late || squeezed || unit_px >= 6.0 { return 1; } for m in [2i64, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000, 50000, 100000, 1000000] { if m as f64 * unit_px >= 6.0 { return m; } } 1000000 };
    // a window so wide that neighbouring numbers would touch: show every 2nd, 5th, 10th ... (of the chosen step) instead, enough
    // for a clear gap between any two; a window whose numbers merely sit close is left as it is
    let cw = |t: &str| t.chars().map(|c| match c { '-' | '\u{2212}' => 4.7, '.' | ',' => 3.9, c if c.is_ascii_digit() => 7.8, _ => 9.0 }).sum::<f64>() * ls;
    let thin = |every: i64, unit_px: f64, widths: &[f64], gap: f64| -> i64 {
        // (a plain window already squeezed into a strip by Square cells is only warned about, as it always has been)
        if every <= 0 || widths.len() < 2 || (squeezed && !late) { return every; }
        let tight = widths.windows(2).any(|w| every as f64 * unit_px - 0.5 * (w[0] + w[1]) < gap);
        if !tight { return every; }
        let widest = widths.iter().cloned().fold(0.0f64, f64::max);
        for m in [2i64, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000, 50000, 100000, 1000000] { if (every * m) as f64 * unit_px - widest >= 6.0 { return every * m; } }
        every * 1000000
    };
    let lab_of = |v: f64| if pi_mode { pi_label(v) } else if deg { format!("{}\u{b0}", fmt(v, 3)) } else { fmt(v, 3) };
    let (xe_eff, ye_eff) = if !p.b("numbers") { (0, 0) } else {
    let (xe, ye) = (every_n(p.s("xnum")), every_n(p.s("ynum")));
    let xe = if xe > 0 {
        let ws: Vec<f64> = (0..MAX_LINES).map(|i| (x0 / xs - 1e-9).ceil() + i as f64).take_while(|k| k * xs <= x1 + 1e-9).filter(|k| (*k as i64).rem_euclid(xe) == 0).map(|k| cw(&lab_of(k * xs))).collect();
        // (widths of the numbers `xe` lines apart, so the gap is measured between numbers that are really drawn)
        // (numbers beside a conic or in Print keep 4 px between them, the plain figure 0.5 px)
        thin(xe, xs * ux, &ws, if late { 1.0 } else { 0.5 })
    } else { xe };
    // (a number is about 10 px tall: the rows keep 6 px between them, so a row at least 16 px from the next)
    let ye = thin(ye, ys * uy, &[15.5, 15.5], 0.5);
    (xe, ye) };

    // An x number that meets a y number (the x row / y column corner) drops below its row to a clear spot (see below). Where the
    // y numbers sit too close for any spot (under about 24 px per unit) the x axis is thinned by the same step list, as the one
    // case where an x/y collision thins anything.
    let xe_eff = if late && xe_eff > 0 && ye_eff > 0 {
        let ay = sy(if y0 <= 0.0 && y1 >= 0.0 { 0.0 } else { y0 });
        let ax = sx(if x0 <= 0.0 && x1 >= 0.0 { 0.0 } else { x0 });
        let xrow = if ay + 18.0 > mt + ph + 30.0 { ay - 9.0 } else { ay + 20.0 + dn };
        let bx = |x: f64, y: f64, an: &str, t: &str| -> [f64; 4] { let w = cw(t); let (a, b) = match an { "end" => (x - w, x), "start" => (x, x + w), _ => (x - 0.5 * w, x + 0.5 * w) }; [a, y - 10.0 * ls, b, y + 0.5 * ls] };
        let ybs: Vec<[f64; 4]> = (0..MAX_LINES).map(|i| (y0 / ys - 1e-9).ceil() + i as f64).take_while(|k| k * ys <= y1 + 1e-9).filter(|k| (*k as i64).rem_euclid(ye_eff) == 0 && k.abs() > 1e-9 && (sy(k * ys) - ay).abs() < 90.0).map(|k| bx(ax - 6.0, sy(k * ys) + 4.1 + 0.35 * dn, "end", &fmt(k * ys, 3))).collect();
        let mut chosen = xe_eff;
        for m in [1i64, 2, 5, 10, 20, 50, 100, 1000] {
            let e = xe_eff * m;
            chosen = e;
            let hit = (0..MAX_LINES).map(|i| (x0 / xs - 1e-9).ceil() + i as f64).take_while(|k| k * xs <= x1 + 1e-9).filter(|k| (*k as i64).rem_euclid(e) == 0 && k.abs() > 1e-9 && (sx(k * xs) - ax).abs() < 90.0).any(|k| {
                let t = lab_of(k * xs);
                let meets = |dy: f64| { let b = bx(sx(k * xs), xrow + dy, "middle", &t); ybs.iter().any(|o| rect_hit(&b, o)) };
                meets(0.0) && (13..=26).all(|s| meets(s as f64))
            });
            if !hit { break; }
        }
        chosen
    } else { xe_eff };
    let numbers_thinned = { let (xb, yb) = if p.b("numbers") { (every_n(p.s("xnum")), every_n(p.s("ynum"))) } else { (0, 0) }; (xb > 0 && xe_eff != xb) || (yb > 0 && ye_eff != yb) };

    // the major lines follow the numbers: a line stride that divides the number stride, so every numbered tick has a line under it
    let line_stride = |unit_px: f64, every: i64| -> i64 {
        let need = stride(unit_px);
        if need <= 1 || every <= 0 { return need; }
        (need..=every.max(need)).find(|d| every % d == 0).unwrap_or(need)
    };
    let (gsx, gsy) = (line_stride(xs * ux, xe_eff), line_stride(ys * uy, ye_eff));
    let gstyle = p.s("grid");
    // the reference's standard grid is near-black 1 px crisp lines under 2.4 px axes
    let (mut gcol, mut gw, mut mcol, axw) = match gstyle { "Light" => ("#dde3ea", 0.9, "#eef1f5", 2.4), "Bold" => ("#6f7d8c", 1.4, "#b9c2cc", 2.6), "None" => ("none", 0.0, "none", 2.4), _ => ("#111827", 1.0, "#e6ebf1", 2.4) };
    // (beside a conic the standard grid is a mid grey-blue, so it does not outweigh the 2 px curves)
    if !print && has_conic && gstyle == "Standard" { gcol = "#8794a3"; }
    if print { (gcol, gw, mcol) = match gstyle { "Light" => ("#8c8c8c", 1.0, "#d2d2d2"), "Bold" => ("#4a4a4a", 1.8, "#aaaaaa"), "None" => ("none", 0.0, "none"), _ => ("#5a5a5a", 1.4, "#aaaaaa") }; }
    // (a crispEdges group: figures::to_rgba widens its 1 px lines when the preview shrinks, as a browser does)
    let crisp = gstyle == "Standard" || gstyle.is_empty() || print;
    let gl = |d: &mut Doc, x1: f64, y1: f64, x2: f64, y2: f64, col: &str, w: f64| { d.raw(&format!(r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{col}" stroke-width="{w}"/>"#)); };
    // (shading that goes under the gridlines is put here once every row is known)
    if late { d.raw("<!--SHADE-->"); }
    // gridlines
    if gstyle != "None" {
        // (minor lines closer than 3 px smear, so they are left out)
        if minor > 1 && (!late || squeezed || xs / minor as f64 * ux >= 4.0) {
            let ms = xs / minor as f64;
            let mut k = (x0 / ms).ceil();
            let mut n = 0; while k * ms <= x1 + 1e-9 && n < MAX_LINES { d.line(sx(k * ms), mt, sx(k * ms), mt + ph, mcol, if print { 1.0 } else { 0.8 }); k += 1.0; n += 1; }
        }
        if minor > 1 && (!late || squeezed || ys / minor as f64 * uy >= 4.0) {
            let ms = ys / minor as f64;
            let mut k = (y0 / ms).ceil();
            let mut n = 0; while k * ms <= y1 + 1e-9 && n < MAX_LINES { d.line(ml, sy(k * ms), ml + pw, sy(k * ms), mcol, if print { 1.0 } else { 0.8 }); k += 1.0; n += 1; }
        }
        if crisp { d.raw(r#"<g shape-rendering="crispEdges" data-op="math">"#); }
        let mut k = (x0 / xs - 1e-9).ceil();
        let mut n = 0; while k * xs <= x1 + 1e-9 && n < MAX_LINES { if (k as i64).rem_euclid(gsx) == 0 { gl(&mut d, sx(k * xs), mt, sx(k * xs), mt + ph, gcol, gw); } k += 1.0; n += 1; }
        let mut k = (y0 / ys - 1e-9).ceil();
        let mut n = 0; while k * ys <= y1 + 1e-9 && n < MAX_LINES { if (k as i64).rem_euclid(gsy) == 0 { gl(&mut d, ml, sy(k * ys), ml + pw, sy(k * ys), gcol, gw); } k += 1.0; n += 1; }
        if crisp { d.raw("</g>"); }
    }
    // axes: the reference runs each 20 px past the grid, with a triangle head at both ends
    let ax_y = if y0 <= 0.0 && y1 >= 0.0 { 0.0 } else { y0 };
    let ax_x = if x0 <= 0.0 && x1 >= 0.0 { 0.0 } else { x0 };
    let ext = if p.b("arrows") { 20.0 } else { 0.0 };
    let (axy, axx) = (sy(ax_y), sx(ax_x));
    if p.b("arrows") {
        let head = |d: &mut Doc, x: f64, y: f64, dx: f64, dy: f64| {
            // marker: 13 px triangle, tip 2.6 px past the line end, base 10.4 px behind it
            let (tx, ty) = (x + dx * 2.6, y + dy * 2.6);
            let (bx, by) = (x - dx * 10.4, y - dy * 10.4);
            d.raw(&format!(r##"<polygon points="{tx:.2},{ty:.2} {:.2},{:.2} {:.2},{:.2}" fill="#111827"/>"##, bx - dy * 6.5, by + dx * 6.5, bx + dy * 6.5, by - dx * 6.5));
        };
        d.raw(&format!(r##"<line x1="{:.2}" y1="{axy:.2}" x2="{:.2}" y2="{axy:.2}" stroke="#111827" stroke-width="{axw}"/>"##, ml - ext, ml + pw + ext));
        d.raw(&format!(r##"<line x1="{axx:.2}" y1="{:.2}" x2="{axx:.2}" y2="{:.2}" stroke="#111827" stroke-width="{axw}"/>"##, mt + ph + ext, mt - ext));
        head(&mut d, ml + pw + ext, axy, 1.0, 0.0);
        head(&mut d, ml - ext, axy, -1.0, 0.0);
        head(&mut d, axx, mt - ext, 0.0, -1.0);
        head(&mut d, axx, mt + ph + ext, 0.0, 1.0);
    } else {
        d.line(ml, axy, ml + pw, axy, "#111827", axw);
        d.line(axx, mt + ph, axx, mt, "#111827", axw);
    }
    // numbers (bold 14 with a white halo, no tick marks, as the reference)
    // mode 0: halo and digits together; 1: only the white halo plates (placed under the curves); 2: only the digits (over them)
    let draw_numbers = |d: &mut Doc, obst: &[Vec<Pt>], mode: u8| { if p.b("numbers") {
        let origin = ax_y == 0.0 && ax_x == 0.0;
        let bbs: Vec<[f64; 4]> = obst.iter().map(|c| poly_bbox(c)).collect();
        let (xe, ye) = (xe_eff, ye_eff);
        // Every number sits at its own tick in the standard row (x numbers under the x-axis, y numbers left of the y-axis), never
        // moved. (x, y, anchor, text, kind: 0 along the x-axis, 1 along the y-axis, 2 the origin zero)
        let mut items: Vec<(f64, f64, &'static str, String, u8)> = vec![];
        let mut k = (x0 / xs - 1e-9).ceil();
        let mut n = 0;
        while k * xs <= x1 + 1e-9 && n < MAX_LINES {
            n += 1;
            let v = k * xs;
            let numbered = xe > 0 && (k as i64).rem_euclid(xe) == 0;
            if numbered && !(v.abs() < 1e-9 && origin) { let yy = if axy + 18.0 > mt + ph + 30.0 { axy - 9.0 } else { axy + 20.0 + dn }; items.push((sx(v), yy, "middle", lab_of(v), 0)); }
            k += 1.0;
        }
        let mut k = (y0 / ys - 1e-9).ceil();
        let mut n = 0;
        while k * ys <= y1 + 1e-9 && n < MAX_LINES {
            n += 1;
            let v = k * ys;
            let numbered = ye > 0 && (k as i64).rem_euclid(ye) == 0;
            if numbered && !(v.abs() < 1e-9 && origin) { items.push((axx - 6.0, sy(v) + 4.1 + 0.35 * dn, "end", fmt(v, 3), 1)); }
            k += 1.0;
        }
        // the box a number fills (about 10 px of digits above the baseline, a little below), a pixel wider each side
        let box_of = |x: f64, y: f64, an: &str, t: &str| -> [f64; 4] {
            let w = cw(t);
            let (a, b) = match an { "end" => (x - w, x), "start" => (x, x + w), _ => (x - 0.5 * w, x + 0.5 * w) };
            [a - 1.0, y - 12.0 * ls, b + 1.0, y + 3.0 * ls]
        };
        let on_curve = |r: &[f64; 4]| !obst.is_empty() && label_conflicts_bb(r, obst, &bbs, &[]) > 0;
        // an x number whose box meets a y number's (the corner of the x row and the y column) drops one digit height below its row,
        // still centred on its own tick; nothing is thinned for it
        if late {
            // (the digit boxes here are the tight ones, 10 px tall: the digits are what must not touch)
            let tight = |x: f64, y: f64, an: &str, t: &str| { let b = box_of(x, y, an, t); [b[0] + 1.0, y - 10.0 * ls, b[2] - 1.0, y + 0.5 * ls] };
            let yb: Vec<[f64; 4]> = items.iter().filter(|it| it.4 == 1).map(|(x, y, an, t, _)| tight(*x, *y, an, t)).collect();
            for it in items.iter_mut().filter(|it| it.4 == 0) {
                if yb.iter().any(|o| rect_hit(&tight(it.0, it.1, it.2, &it.3), o)) {
                    let y0s = it.1;
                    for s in 13..=26 { if !yb.iter().any(|o| rect_hit(&tight(it.0, y0s + s as f64, it.2, &it.3), o)) { it.1 = y0s + s as f64; break; } }
                }
            }
        }
        // the origin zero: below left of the origin, else below right, left out only when neither is clear of the other numbers
        if origin && xe > 0 {
            let boxes: Vec<[f64; 4]> = items.iter().map(|(x, y, an, t, _)| box_of(*x, *y, an, t)).collect();
            let clear = |x: f64, y: f64, an: &str| { let b = box_of(x, y, an, "0"); let b = [b[0] - 2.0, b[1], b[2] + 2.0, b[3]]; !boxes.iter().any(|o| rect_hit(&b, o)) };
            let row = axy + 20.0 + dn;
            if !late || clear(axx - 8.0, row, "end") { items.push((axx - 8.0, row, "end", "0".into(), 2)); }
            else if clear(axx + 5.5, row, "start") { items.push((axx + 5.5, row, "start", "0".into(), 2)); }
        }
        // a number whose box touches a curve is drawn over it with a thin white halo (the curve is only interrupted by the halo)
        let put: Vec<(f64, f64, &str, bool)> = items.iter().map(|(x, y, an, t, _)| (*x, *y, *an, late && on_curve(&box_of(*x, *y, an, t)))).collect();
        let halo = if print { 5 } else { 4 };
        for ((_, _, _, t, kind), (x, y, anchor, over)) in items.iter().zip(&put) {
            let paint = match (mode, *over) {
                (1, true) => continue,
                (1, false) => format!(r##"fill="#fff" stroke="#fff" stroke-width="{halo}" stroke-linejoin="round""##),
                // (a number over a curve gets only a thin halo: the curve is never erased, and a small circle keeps its whole outline)
                (2, true) => format!(r##"fill="#111827" stroke="#fff" stroke-width="2" paint-order="stroke" stroke-linejoin="round" data-num="1" data-plate="over" data-axis="{kind}""##),
                (2, false) => format!(r##"fill="#111827" data-num="1" data-axis="{kind}""##),
                _ => format!(r##"fill="#111827" stroke="#fff" stroke-width="{halo}" paint-order="stroke" stroke-linejoin="round""##),
            };
            d.raw(&format!(r##"<text x="{x:.2}" y="{y:.2}" text-anchor="{anchor}" font-family="Arial, Helvetica, Liberation Sans, sans-serif" font-size="{}" font-weight="bold" {paint}>{}</text>"##, 14.0 * ls, super::svg::esc(t)));
        }
    } };
    // (with curves the numbers wait: their white plates go under the curves, so no curve ever shows a gap, the digits over them)
    if late { d.raw("<!--NUMS-->"); } else { draw_numbers(&mut d, &[], 0); }
    let axis_label = |d: &mut Doc, x: f64, y: f64, anchor: &str, t: &str| {
        d.raw(&format!(r##"<text x="{x:.2}" y="{y:.2}" text-anchor="{anchor}" font-family="{SERIF}" font-weight="bold" font-size="{}" fill="#111827">{}</text>"##, 19.6 * ls, math_tspans(t)));
    };
    if !p.s("xlabel").is_empty() { axis_label(&mut d, ml + pw + ext + 6.0, axy + 5.6 + 0.4 * dn, "start", p.s("xlabel")); }
    if !p.s("ylabel").is_empty() { axis_label(&mut d, axx, mt - ext - 6.0, "middle", p.s("ylabel")); }
    match p.s("title_mode") {
        "None" => {}
        "Blank line" => { d.line(w / 2.0 - 90.0, 28.0, w / 2.0 + 90.0, 28.0, INK, 1.4); }
        _ => if !p.s("title").is_empty() { math_text(&mut d, w / 2.0, 18.0, 18.0, "middle", p.s("title"), true); },
    }
    match p.s("xtitle_mode") {
        "Text" => math_text(&mut d, ml + pw / 2.0, h - 16.0, 15.0, "middle", p.s("xtitle"), true),
        "Blank line" => { d.line(ml + pw / 2.0 - 70.0, h - 12.0, ml + pw / 2.0 + 70.0, h - 12.0, INK, 1.4); }
        _ => {}
    }
    match p.s("ytitle_mode") {
        "Text" => {
            let (cx, cy) = (16.0, mt + ph / 2.0);
            d.raw(&format!(r##"<g transform="rotate(-90 {cx:.2} {cy:.2})">"##));
            math_text(&mut d, cx, cy, 15.0, "middle", p.s("ytitle"), true);
            d.raw("</g>");
        }
        "Blank line" => { d.line(12.0, mt + ph / 2.0 - 70.0, 12.0, mt + ph / 2.0 + 70.0, INK, 1.4); }
        _ => {}
    }
    // graphs
    d.raw(&format!(r#"<clipPath id="plot"><rect x="{ml}" y="{mt}" width="{pw}" height="{ph}"/></clipPath>"#));
    let lw = match p.s("line_width") { "Thin" => 1.6, "Thick" => 3.6, _ => 2.6 };
    let hatch = p.s("shading") == "Hatching" || print;
    let span_y = y1 - y0;
    let mut implicit_used = 0;
    let mut implicit_tried = 0;
    let mut imp_bytes = 0usize;
    let mut reqs: Vec<LabelReq> = vec![];
    // inequalities shade together, as the solution of the whole system, when one of them is curved: a curved
    // inequality (circle, ellipse ...) with at least one other (curved, a straight x + y < 3, or y > f(x)).
    // Each member is (row, F as chars, shade where F > 0, its own domain).
    let group_cells = (((pw / 1.2).ceil().max(16.0) as usize).min(implicit::MAX_CELLS), ((ph / 1.2).ceil().max(16.0) as usize).min(implicit::MAX_CELLS));
    let mut group_dropped: Vec<usize> = vec![];
    let mut work = 0u64;
    let ineq_group: Vec<GroupMember> = {
        let mut v: Vec<GroupMember> = vec![];
        let mut curved = 0;
        let mut cost = 0u64;
        for (ci, r) in p.s("equations").split(';').enumerate() {
            // (the same length and depth guard as drawing a row, then the share of the work budget)
            if ci >= MAX_ROWS || v.len() >= 8 || !readable_len(r) { continue; }
            let (r, _) = unstyle(r);
            let (main, dom) = split_domain(r.trim());
            match parse_row(main) {
                Some(Row::ImplicitIneq { e, positive, .. }) => {
                    let cs = norm_expr(&e);
                    let c = (cs.len() as u64 + 8) * 128 * 128;
                    if cost + c > GROUP_WORK_MAX { group_dropped.push(ci + 1); continue; }
                    cost += c;
                    if !implicit::linear_in_y(&|x, y| eval_cs(&cs, x, Some(y), deg), x0, x1, y0, y1) { curved += 1; }
                    v.push(GroupMember { ci, cs, pos: positive, dom: ImpDom::parse(dom, deg), band: None });
                }
                Some(Row::Ineq { rhs, above, .. }) if dom.is_none() => {
                    let cs = norm_expr(&format!("(y)-({rhs})"));
                    let c = (cs.len() as u64 + 8) * 128 * 128;
                    if cost + c > GROUP_WORK_MAX { group_dropped.push(ci + 1); continue; }
                    cost += c;
                    v.push(GroupMember { ci, cs, pos: above, dom: ImpDom::default(), band: None });
                }
                Some(Row::Band(ivs)) if dom.is_none() => v.push(GroupMember { ci, cs: vec![], pos: true, dom: ImpDom::default(), band: Some(ivs.iter().map(|iv| (iv.lo, iv.hi)).collect()) }),
                _ => {}
            }
        }
        if curved >= 1 && v.len() >= 2 { v } else { vec![] }
    };
    // what a label must keep clear of: curves drawn so far (px), other labels, the axes, point labels
    let mut obst: Vec<Vec<Pt>> = vec![];
    let mut placed: Vec<[f64; 4]> = vec![];
    if y0 <= 0.0 && y1 >= 0.0 { placed.push([ml, sy(0.0) - 9.0, ml + pw, sy(0.0) + 24.0]); }
    if x0 <= 0.0 && x1 >= 0.0 { placed.push([sx(0.0) - if has_conic { 40.0 } else { 28.0 }, mt, sx(0.0) + 9.0, mt + ph]); }
    for seg in p.s("points").split(';').take(MAX_POINTS) {
        let mut at = 0;
        while let Some(a) = seg[at..].find('(') {
            let a = at + a;
            let Some(b) = seg[a..].find(')').map(|b| a + b) else { break };
            let n = nums(&seg[a + 1..b]);
            if n.len() == 2 { placed.push([sx(n[0]) - 7.0, sy(n[1]) - 24.0, sx(n[0]) + 90.0, sy(n[1]) + 8.0]); }
            at = b + 1;
        }
    }
    let mut skipped_rows = 0usize;
    for n in &group_dropped { notes.push(format!("Row {n} is too long to shade together with the other inequalities (it is shaded on its own)")); }
    let group_chars: usize = ineq_group.iter().map(|g| g.cs.len() + 8).sum();
    let gcells = scaled_cells(group_cells, group_chars, GROUP_WORK_MAX);
    if shade_on && !ineq_group.is_empty() { work += group_chars as u64 * gcells.0 as u64 * gcells.1 as u64; }
    // the solution of the system: one tint (an outlined, hatched one in Print) where every member holds
    if shade_on && !ineq_group.is_empty() {
        let gf = |x: f64, y: f64| -> Option<f64> {
            let mut m = f64::INFINITY;
            for g in &ineq_group {
                if let Some(ivs) = &g.band {
                    // inside an interval of x: how far from its nearer end (so the boundary stays exact)
                    let v = ivs.iter().map(|&(lo, hi)| (if lo.is_finite() { x - lo } else { 1e6 }).min(if hi.is_finite() { hi - x } else { 1e6 })).fold(-1e6, f64::max);
                    m = m.min(v);
                    continue;
                }
                let v = eval_cs(&g.cs, x, Some(y), deg)?;
                m = m.min(if !g.dom.ok(x, y, 0.0) { -1.0 } else if g.pos { v } else { -v });
            }
            Some(m)
        };
        if let Some(g2) = implicit::sample(&gf, x0, x1, y0, y1, gcells.0, gcells.1) {
            let polys = g2.fill(true, &|_, _| true);
            // (slivers of zero area, from nodes that sit exactly on a zero, are no region: nothing is drawn and the editor says so)
            let area: f64 = polys.iter().map(|pl| (0..pl.len()).map(|k| { let (u, v) = (pl[k], pl[(k + 1) % pl.len()]); u.0 * v.1 - v.0 * u.1 }).sum::<f64>().abs() / 2.0).sum();
            if area <= 0.5 * g2.dx * g2.dy {
                let rows: Vec<String> = ineq_group.iter().map(|g| (g.ci + 1).to_string()).collect();
                notes.push(format!("Rows {}: no points satisfy all of these inequalities together, so nothing is shaded", rows.join(", ")));
            } else if polys.len() * 48 > IMP_BYTES_MAX {
                notes.push("The common region is too detailed to shade".to_string());
            } else if !polys.is_empty() {
                let mut dd = String::new();
                for poly in &polys {
                    for (k, &(x, y)) in poly.iter().enumerate() { dd.push_str(&format!("{}{:.1} {:.1} ", if k == 0 { "M" } else { "L" }, sx(x), sy(y))); }
                    dd.push('Z');
                }
                imp_bytes += dd.len();
                let (gfill, gop, _gcol) = if print {
                    put_shade(&mut d, &mut shade, late, print_hatch("shg", true));
                    ("url(#shg)".to_string(), 1.0, "#777")
                } else { (GROUP_TINT.to_string(), GROUP_TINT_OPACITY, GROUP_TINT) };
                put_shade(&mut d, &mut shade, late, format!(r##"<path d="{dd}" fill="{gfill}" fill-opacity="{gop}" stroke="none"/>"##));
            }
        }
    }
    // the curved rows (conics and other implicit curves) the straight ones may be asymptotes of, and how heavy they are drawn
    let mut conic_cs: Vec<Curve> = vec![];
    let mut conic_rows: Vec<usize> = vec![];
    for (ci, r) in p.s("equations").split(';').enumerate().take(MAX_ROWS) {
        if !readable_len(r) { continue; }
        let (r, _) = unstyle(r);
        let (main, dom) = split_domain(r.trim());
        match parse_row(main) {
            Some(Row::Implicit(e)) => {
                let cs = norm_expr(&e);
                if !implicit::linear_in_y(&|x, y| eval_cs(&cs, x, Some(y), deg), x0, x1, y0, y1) { conic_rows.push(ci); if conic_cs.len() < 2 * IMPLICIT_MAX { conic_cs.push(Curve::Imp(cs)); } }
                else if conic_cs.len() < 2 * IMPLICIT_MAX && dom.is_none() {
                    // (xy = 4, y (x - 1) = 4: solved for y, a curve the axes can be asymptotes of unless it is itself a line)
                    let c = Curve::Solved(cs);
                    if line_fit(&|x| curve_y(&c, x, deg)).is_none() { conic_cs.push(c); }
                }
            }
            Some(Row::Func(r)) if dom.is_none() && conic_cs.len() < 2 * IMPLICIT_MAX => {
                let c = Curve::Func(r);
                if line_fit(&|x| curve_y(&c, x, deg)).is_none() { conic_cs.push(c); }
            }
            Some(Row::ImplicitIneq { .. }) => conic_rows.push(ci),
            _ => {}
        }
    }
    // (in Print every row is at least 1.8 times the grid, and rows differ by weight above that)
    let print_w = |ci: usize| 1.8 * (if gw > 0.0 { gw } else { 1.0 }) * (lw / 2.6f64).max(1.0) * [1.0, 1.12, 1.4, 1.25, 1.5][ci % 5];
    let conic_lw = conic_rows.iter().map(|&c| print_w(c)).fold(f64::INFINITY, f64::min);
    for (ci, raw_row) in p.s("equations").split(';').enumerate() {
        let (raw_owned, user_dash) = unstyle(raw_row);
        let word_asym = user_dash == Some("asym");
        let user_dash = if word_asym { None } else { user_dash };
        let raw_row = raw_owned.as_str();
        if raw_row.is_empty() { continue; }
        if ci >= MAX_ROWS { skipped_rows += 1; continue; }
        // a row too long or nested too deeply is refused before anything reads it
        if !cheap_len(raw_row) { notes.push(format!("Row {} is too long to draw", ci + 1)); continue; }
        if let Some(t) = caret_sci(raw_row) { notes.push(format!("Row {}: {t} after ^ is read as a product with e; write ^({t}) for scientific notation", ci + 1)); }
        let (main, dom) = split_domain(raw_row);
        let Some(row) = parse_row(main) else { continue };
        // a straight row that is an asymptote of a curve in the figure (or says "asymptote") is drawn fine, dotted and grey, whatever
        // else was typed after it: dashed is for excluded boundaries only
        let lin = match &row {
            Row::Vert(v) if dom.is_none() => Some(Lin::Vert(*v)),
            Row::Func(r) if dom.is_none() => line_fit(&|x| eval_full(r, x, None, deg)),
            Row::Implicit(e) if dom.is_none() => { let cs = norm_expr(e); line_fit(&|x| { let (a, b) = (eval_cs(&cs, x, Some(0.0), deg)?, eval_cs(&cs, x, Some(1.0), deg)?); let sl = b - a; (sl.abs() > 1e-12).then(|| -a / sl) }) }
            _ => None,
        };
        let asym_row = lin.is_some_and(|l| word_asym || conic_cs.iter().any(|c| asymptote_of(c, l, deg)));
        if asym_row && user_dash.is_some_and(|d| d != "none") { notes.push(format!("Row {} is an asymptote, so it is drawn fine and dotted, not dashed: {}", ci + 1, main.trim())); }
        // (in Print every row is at least 1.8 times as heavy as the gridlines, so a row never vanishes into a gridline it lies on)
        let lw = if print { let w = print_w(ci); (if !conic_rows.contains(&ci) && conic_lw.is_finite() { w.min(conic_lw) } else { w }).max(1.8 * if gw > 0.0 { gw } else { 1.0 }) } else { lw };
        let lw = if asym_row { if print { 1.6 } else { 1.4 } } else { lw };
        let col_s = if asym_row { (if print { "#666666" } else { "#6b7280" }).to_string() } else if print { ["#000000", "#444444", "#1c1c1c", "#3a3a3a", "#2e2e2e"][ci % 5].to_string() } else if ci < 4 { colp(p, &format!("eq{}_color", ci + 1), PALETTE[ci]) } else { PALETTE[ci % PALETTE.len()].to_string() };
        let rdash = if asym_row { if print { "7 5" } else { "2 4" } } else { user_dash.map(|d| if d == "none" { "" } else { d }).unwrap_or("") };
        // SOLID is an included boundary and every plain curve, DASHED only an excluded (strict) one; a style word overrides
        let strict_pat = if asym_row { rdash } else { user_dash.map(|d| if d == "none" { "" } else { d }).unwrap_or("9 6") };
        let col = col_s.as_str();
        let sline = |d: &mut Doc, pts: &[Pt], pat: &str| { if pat.is_empty() { d.polyline(pts, col, lw); } else { dpoly(d, pts, col, lw, pat); } };
        let dom_iv: Vec<Interval> = dom.map(|s| parse_ineq(&s.to_lowercase().replace('y', "x"))).unwrap_or_default();
        let in_dom = |v: f64| dom_iv.is_empty() || dom_iv.iter().any(|iv| (v > iv.lo || (iv.lo_c && v >= iv.lo)) && (v < iv.hi || (iv.hi_c && v <= iv.hi)));
        let fill_id = format!("sh{ci}");
        let fill = if hatch {
            // Print hatching is a close diagonal screen of dark lines (it has to survive a photocopy); the tick numbers carry a halo over it
            if print {
                put_shade(&mut d, &mut shade, late, print_hatch(&fill_id, ci % 2 == 0));
            } else {
                put_shade(&mut d, &mut shade, late, format!(r##"<defs><pattern id="{fill_id}" patternUnits="userSpaceOnUse" width="9" height="9" patternTransform="rotate({})"><line x1="0" y1="0" x2="0" y2="9" stroke="{col}" stroke-width="1.6"/></pattern></defs>"##, if ci % 2 == 0 { 45 } else { -45 }));
            }
            format!("url(#{fill_id})")
        } else { col.to_string() };
        let fill_op = if hatch { 1.0 } else { 0.18 };
        d.raw(r#"<g clip-path="url(#plot)">"#);
        // sampled curve y = g(x) on the domain, split at gaps and jumps (asymptotes)
        let sample = |gf: &dyn Fn(f64) -> Option<f64>| -> (Vec<Vec<(f64, f64)>>, Vec<f64>) {
            let steps = 900;
            let valid = |x: f64| if in_dom(x) { gf(x).filter(|y| y.is_finite() && y.abs() < 1e6) } else { None };
            let mut runs: Vec<Vec<(f64, f64)>> = vec![vec![]];
            // where each run's two ends stop short of: the x of a sample that is a pole or has no value, and whether it is a pole
            let mut lead: Vec<Option<(f64, bool)>> = vec![None];
            let mut tail: Vec<Option<(f64, bool)>> = vec![None];
            let mut asym = vec![];
            let mut prev: Option<(f64, f64)> = None;
            let mut bad_before: Option<f64> = None;
            let mut flips = 0usize;
            // where the curve stops having a value between a sample that has one and the next that has not (ln(x) at 0):
            // the end of the gap nearest the value, found by halving
            let edge = |good: f64, bad: f64| -> f64 {
                let (mut g, mut b) = (good, bad);
                for _ in 0..60 { let m = 0.5 * (g + b); if m == g || m == b { break; } if valid(m).is_some() { g = m; } else { b = m; } }
                b
            };
            for i in 0..=steps {
                let x = x0 + (x1 - x0) * i as f64 / steps as f64;
                let yv = valid(x);
                match yv {
                    Some(y) => {
                        let mut new_lead = bad_before.take().map(|b| (edge(x, b), false));
                        if let Some((px, py)) = prev {
                            let jump = (y - py).abs() > 2.0 * span_y && (y.max(py) > y1 || y.min(py) < y0) && (y - py).signum() != 0.0;
                            let steep_flip = jump && ((py > y1 && y < y0) || (py < y0 && y > y1));
                            // a pole: the sign flips between two large values that grow toward the gap, so the
                            // line across it would be false (the curve leaves one way and returns the other)
                            let mut ab = (px, x);
                            let pole = py.signum() != y.signum() && { flips += 1; flips <= 3000 } && {
                                // narrow the sign change down: at a root both ends tend to zero, at a pole (or a jump) they stay large
                                let (mut a, mut b, mut fa, mut fb) = (px, x, py, y);
                                for _ in 0..50 {
                                    let m = 0.5 * (a + b);
                                    match gf(m).filter(|v| v.is_finite()) {
                                        Some(v) if v.signum() == fa.signum() => { a = m; fa = v; }
                                        Some(v) => { b = m; fb = v; }
                                        // the middle fell exactly on the pole (1/x at 0): that is the pole
                                        None => { fa = f64::INFINITY; fb = f64::INFINITY; a = m; b = m; break; }
                                    }
                                }
                                ab = (a, b);
                                fa.abs() > 1e-3 * span_y && fb.abs() > 1e-3 * span_y
                            };
                            if steep_flip { asym.push((px + x) / 2.0); runs.push(vec![]); lead.push(None); tail.push(None); }
                            else if pole {
                                // each side runs on toward the pole (see below), so no false chord and no gap before the edge
                                *tail.last_mut().unwrap() = Some((ab.0, true));
                                runs.push(vec![]); lead.push(None); tail.push(None);
                                new_lead = Some((ab.1, true));
                            }
                        }
                        if runs.last().unwrap().is_empty() { *lead.last_mut().unwrap() = new_lead; }
                        runs.last_mut().unwrap().push((x, y));
                        prev = Some((x, y));
                    }
                    None => {
                        if !runs.last().unwrap().is_empty() {
                            // the curve stops at a sample with no value: it runs on toward it (a pole at a sample, such as 1/x at 0)
                            if in_dom(x) { let last = runs.last().unwrap().last().unwrap().0; *tail.last_mut().unwrap() = Some((edge(last, x), false)); }
                            runs.push(vec![]); lead.push(None); tail.push(None);
                        }
                        bad_before = if in_dom(x) { Some(x) } else { None };
                        prev = None;
                    }
                }
            }
            // run each end on toward its gap until the curve leaves the window (it only needs doing when it stops inside it)
            let lim = (y0 - span_y, y1 + span_y);
            let toward = |from: (f64, f64), to: f64, pole: bool| -> Vec<(f64, f64)> {
                let mut out = vec![];
                if !(from.1 >= y0 && from.1 <= y1) { return out; }
                for k in 1..=160 {
                    let xk = from.0 + (to - from.0) * (1.0 - 0.88f64.powi(k));
                    let Some(v) = valid(xk) else { break };
                    if pole && v.signum() != from.1.signum() { break; }
                    out.push((xk, v));
                    if v < lim.0 || v > lim.1 { break; }
                }
                // only a curve that is running away (the steps in value keep growing, as toward a pole) is carried on; one that
                // settles (a hole, a jump, a square-root end) stays where the samples left it
                let step = |i: usize| (out[i].1 - if i == 0 { from.1 } else { out[i - 1].1 }).abs();
                let leaves = out.last().is_some_and(|q| q.1 < lim.0 || q.1 > lim.1);
                // (a logarithm runs off at an even pace, so it is the leaving of the window that keeps the extension, not the growth)
                let exits = out.iter().any(|q| q.1 < y0 || q.1 > y1);
                let running = match out.len() { 0 => false, n if n < 4 => leaves || exits, _ => exits || (step(2) > step(1) * 1.01 && step(3) > step(2) * 1.01) };
                if !running { out.clear(); }
                out
            };
            for k in 0..runs.len() {
                if runs[k].is_empty() { continue; }
                if let Some((t, pole)) = tail[k] { let e = toward(*runs[k].last().unwrap(), t, pole); runs[k].extend(e); }
                if let Some((t, pole)) = lead[k] { let mut e = toward(runs[k][0], t, pole); e.reverse(); e.extend(std::mem::take(&mut runs[k])); runs[k] = e; }
            }
            runs.retain(|r| r.len() > 1);
            (runs, asym)
        };
        let mut label_at: Option<(f64, f64)> = None;
        let row_start = obst.len();
        let imp: Option<(&str, Option<(bool, bool)>)> = match &row {
            Row::Implicit(e) if !cheap_len(raw_row) => { let _ = e; Some((e.as_str(), None)) }
            Row::Implicit(e) => {
                let cs = norm_expr(e);
                (!implicit::linear_in_y(&|x, y| eval_cs(&cs, x, Some(y), deg), x0, x1, y0, y1)).then_some((e.as_str(), None))
            }
            Row::ImplicitIneq { e, positive, strict, .. } => Some((e.as_str(), Some((*positive, *strict)))),
            _ => None,
        };
        match &row {
            Row::Vert(v) => {
                let (ya, yb) = if dom_iv.is_empty() { (y0 - span_y, y1 + span_y) } else { (dom_iv[0].lo.max(y0 - span_y), dom_iv[0].hi.min(y1 + span_y)) };
                if rdash.is_empty() { d.line(sx(*v), sy(ya), sx(*v), sy(yb), col, lw); } else { dline(&mut d, (sx(*v), sy(ya)), (sx(*v), sy(yb)), col, lw, rdash); }
                obst.push(vec![(sx(*v), sy(ya)), (sx(*v), sy(yb))]);
                label_at = Some((*v, y0 + span_y * 0.9));
                if p.b("endpoints") {
                    for iv in &dom_iv { for (b, c) in [(iv.lo, iv.lo_c), (iv.hi, iv.hi_c)] { if b.is_finite() { d.circle(sx(*v), sy(b), 5.0, if c { col } else { "#fff" }, col, 2.0); } } }
                }
            }
            Row::Band(ivs) => {
                for iv in ivs {
                    let (a, b) = (iv.lo.max(x0 - 1.0), iv.hi.min(x1 + 1.0));
                    if b > a && shade_on && !ineq_group.iter().any(|g| g.ci == ci) { put_shade(&mut d, &mut shade, late, format!(r##"<rect x="{:.2}" y="{mt}" width="{:.2}" height="{ph}" fill="{fill}" fill-opacity="{fill_op}"/>"##, sx(a), sx(b) - sx(a))); }
                    for (e, c) in [(iv.lo, iv.lo_c), (iv.hi, iv.hi_c)] {
                        // an excluded limit on the y-axis (x > 0): the axis is cut out under the dashes, so the gaps show
                        if e.is_finite() && !c && (sx(e) - axx).abs() < 2.0 && strict_pat != "" { d.line(sx(e), mt, sx(e), mt + ph, "#fff", axw + 1.6); }
                        if e.is_finite() { dline(&mut d, (sx(e), mt), (sx(e), mt + ph), col, lw, if c { if rdash.is_empty() { "none" } else { rdash } } else { strict_pat }); }
                    }
                }
            }
            // F(x, y) = G(x, y) that is not a straight line in y (circles, ellipses, hyperbolas, x = y^2 ...), or an
            // inequality whose boundary is such a curve: sampled once, drawn as chained polylines, shaded by cell
            _ if imp.is_some() => {
                let (e, ineq) = imp.unwrap();
                let typed: Vec<char> = raw_row.chars().collect();
                let n_row = ci + 1;
                if typed.len() > 200 || nest_depth(typed.iter()) > 30 {
                    notes.push(format!("Row {n_row} is too long to draw"));
                } else if e.trim() == "-1" && matches!(ineq, Some((true, _))) {
                    notes.push(format!("Row {n_row}: no points satisfy both limits: {}", main.trim()));
                } else if work + { let (a, b) = scaled_cells(group_cells, e.len(), ROW_WORK_MAX); (e.len() as u64 + 8) * a as u64 * b as u64 } > WORK_MAX && implicit_tried > 0 {
                    notes.push(format!("Row {n_row} is too long to draw together with the other rows: {}", main.trim()));
                } else if implicit_used >= IMPLICIT_MAX || implicit_tried >= IMPLICIT_TRY {
                    let n = format!("Only {IMPLICIT_MAX} curves are drawn: delete a row, or turn one into a straight line, to see row {n_row}");
                    if !notes.iter().any(|x| x.starts_with("Only")) { notes.push(n); }
                } else {
                    implicit_tried += 1;
                    let cs = norm_expr(e);
                    let f = |x: f64, y: f64| eval_cs(&cs, x, Some(y), deg);
                    let (nx, ny) = scaled_cells(group_cells, cs.len(), ROW_WORK_MAX);
                    work += (cs.len() as u64 + 8) * nx as u64 * ny as u64;
                    if let Some(grid) = implicit::sample(&f, x0, x1, y0, y1, nx, ny) {
                        let idom = ImpDom::parse(dom, deg);
                        let mut drew = false;
                        let mut told = false;
                        if !grid.any_valid() {
                            notes.push(format!("Row {n_row}: Cannot read: {}", main.trim()));
                            told = true;
                        } else if grid.crossings() > implicit::MAX_CROSSINGS {
                            notes.push(format!("Row {n_row}: Too detailed to draw: {}", main.trim()));
                            told = true;
                        } else {
                            // shading of F > 0 or F < 0 (one path; whole cells merged into rectangles). Several
                            // non-linear inequalities shade once, where all of them hold.
                            if let (Some((positive, _)), true) = (ineq, shade_on) {
                                let in_group = ineq_group.iter().any(|g| g.ci == ci);
                                if !in_group {
                                    let polys = grid.fill(positive, &|x, y| idom.ok(x, y, 0.0));
                                    if polys.len() * 48 + imp_bytes > IMP_BYTES_MAX {
                                        notes.push(format!("Row {n_row}: Too detailed to shade: {}", main.trim()));
                                        told = true;
                                    } else if !polys.is_empty() {
                                        let mut dd = String::new();
                                        for poly in &polys {
                                            for (k, &(x, y)) in poly.iter().enumerate() { dd.push_str(&format!("{}{:.1} {:.1} ", if k == 0 { "M" } else { "L" }, sx(x), sy(y))); }
                                            dd.push('Z');
                                        }
                                        imp_bytes += dd.len();
                                        put_shade(&mut d, &mut shade, late, format!(r##"<path d="{dd}" fill="{fill}" fill-opacity="{fill_op}" stroke="none"/>"##));
                                        // (slivers of zero area, from nodes that sit exactly on a zero, are no shading)
                                        let area: f64 = polys.iter().map(|pl| (0..pl.len()).map(|k| { let (u, v) = (pl[k], pl[(k + 1) % pl.len()]); u.0 * v.1 - v.0 * u.1 }).sum::<f64>().abs() / 2.0).sum();
                                        if area > 0.5 * grid.dx * grid.dy { drew = true; }
                                    }
                                }
                            }
                            let strict_dash = matches!(ineq, Some((_, true)));
                            let mut mine: Vec<Vec<Pt>> = vec![];
                            let mut chains = grid.contour(&f);
                            // a chain whose limits differ in strictness (4 < E <= 25): each ring is traced on its own, so the
                            // excluded limit is dashed and the included one solid
                            let mut chain_strict: Vec<bool> = vec![strict_dash; chains.len()];
                            if let Row::ImplicitIneq { edges: Some(ed), .. } = &row {
                                if ed[0].1 != ed[1].1 {
                                    let mut sep: Vec<Vec<Pt>> = vec![];
                                    let mut flags: Vec<bool> = vec![];
                                    for (ex, st) in ed {
                                        let cs_e = norm_expr(ex);
                                        let fe = |x: f64, y: f64| eval_cs(&cs_e, x, Some(y), deg);
                                        if let Some(ge) = implicit::sample(&fe, x0, x1, y0, y1, nx, ny) {
                                            for c in ge.contour(&fe) { sep.push(c); flags.push(*st); }
                                        }
                                    }
                                    if !sep.is_empty() { chains = sep; chain_strict = flags; }
                                }
                            }
                            // (a zero-length chain at a node that is exactly zero is no curve)
                            let nonzero = |c: &Vec<Pt>| { let ex = |k: usize| { let v = c.iter().map(|q| if k == 0 { q.0 } else { q.1 }); v.clone().fold(f64::NEG_INFINITY, f64::max) - v.fold(f64::INFINITY, f64::min) }; ex(0) > 1e-6 * grid.dx || ex(1) > 1e-6 * grid.dy };
                            { let mut k = 0; chain_strict.retain(|_| { let keep = nonzero(&chains[k]); k += 1; keep }); }
                            chains.retain(|c| nonzero(c));
                            // a squared or repeated factor touches zero without crossing: its zero set is found as the
                            // minima of |F| (an inequality that holds only on such a set, such as x^2 + y^2 <= 0, draws it too)
                            let strict_ineq = matches!(ineq, Some((_, true)));
                            if !strict_ineq && (ineq.is_none() || !drew) {
                                for c in grid.ridge_chains(&f, &chains) {
                                    let (xa, xb) = c.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, q| (a.0.min(q.0), a.1.max(q.0)));
                                    let (ya, yb) = c.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, q| (a.0.min(q.1), a.1.max(q.1)));
                                    if (xb - xa) <= 3.0 * grid.dx && (yb - ya) <= 3.0 * grid.dy {
                                        let (qx, qy) = (0.5 * (xa + xb), 0.5 * (ya + yb));
                                        if idom.ok(qx, qy, 0.0) {
                                            d.circle(sx(qx), sy(qy), 4.0, col, col, 1.0);
                                            mine.push(vec![(sx(qx) - 0.5, sy(qy)), (sx(qx) + 0.5, sy(qy))]);
                                            notes.push(format!("Row {n_row} is a single point: {}", main.trim()));
                                            drew = true;
                                        }
                                    } else if c.len() > 1 { chains.push(c); chain_strict.push(strict_dash); }
                                }
                            }
                            if chains.is_empty() && ineq.is_none() && !drew {
                                match grid.touching_curve() {
                                    Some(implicit::Touch::Line(l)) => { chains.push(l); chain_strict.push(strict_dash); }
                                    Some(implicit::Touch::Point((qx, qy))) if idom.ok(qx, qy, 0.0) => {
                                        d.circle(sx(qx), sy(qy), 4.0, col, col, 1.0);
                                        mine.push(vec![(sx(qx) - 0.5, sy(qy)), (sx(qx) + 0.5, sy(qy))]);
                                        notes.push(format!("Row {n_row} is a single point: {}", main.trim()));
                                        drew = true;
                                    }
                                    _ => {}
                                }
                            }
                            for (li, l) in chains.iter().enumerate() {
                                let strict_dash = chain_strict.get(li).copied().unwrap_or(strict_dash);
                                // the part of the chain the typed domain keeps, cut exactly at the limit
                                let mut run: Vec<Pt> = vec![];
                                let mut runs: Vec<Vec<Pt>> = vec![];
                                let mut prev: Option<(f64, f64)> = None;
                                for &(x, y) in l {
                                    let okp = idom.ok(x, y, 0.0);
                                    if let Some((px, py)) = prev {
                                        if idom.ok(px, py, 0.0) != okp {
                                            let (mut lo, mut hi) = (0.0f64, 1.0f64);
                                            for _ in 0..24 { let m = 0.5 * (lo + hi); if idom.ok(px + (x - px) * m, py + (y - py) * m, 0.0) == !okp { lo = m; } else { hi = m; } }
                                            let t = if okp { hi } else { lo };
                                            let cut = (sx(px + (x - px) * t), sy(py + (y - py) * t));
                                            if okp { run = vec![cut]; } else { run.push(cut); runs.push(std::mem::take(&mut run)); }
                                        }
                                    }
                                    if okp { run.push((sx(x), sy(y))); }
                                    prev = Some((x, y));
                                }
                                if run.len() > 1 { runs.push(run); }
                                for r in runs {
                                    if r.len() < 2 { continue; }
                                    let pts = thin_polyline(&r);
                                    imp_bytes += pts.len() * 14;
                                    if imp_bytes > IMP_BYTES_MAX { if !told { notes.push(format!("Row {n_row}: Too detailed to draw: {}", main.trim())); told = true; } break; }
                                    if strict_dash && strict_pat != "" { for run in axis_runs(&pts, axx, axy) { d.polyline(&run, "#fff", axw + 1.6); } }
                                    sline(&mut d, &pts, if strict_dash { strict_pat } else { rdash });
                                    mine.push(pts);
                                    drew = true;
                                }
                                // open or closed dots where the domain cuts the curve
                                if p.b("endpoints") && !idom.is_empty() {
                                    for w in l.windows(2) {
                                        for (is_x, b, cl) in idom.limits() {
                                            let (ca, cb) = if is_x { (w[0].0, w[1].0) } else { (w[0].1, w[1].1) };
                                            if (ca - b) * (cb - b) < 0.0 || (ca == b && cb != b) {
                                                let t = (b - ca) / (cb - ca);
                                                let (qx, qy) = (w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t);
                                                if idom.ok(qx, qy, 1e-9 * (1.0 + b.abs())) { d.circle(sx(qx), sy(qy), 5.0, if cl { col } else { "#fff" }, col, 2.0); }
                                            }
                                        }
                                    }
                                }
                            }
                            // a branch that runs flat into the left or right edge approaches a horizontal asymptote there
                            if p.b("asymptotes") && ineq.is_none() {
                                let mut found: Vec<f64> = vec![];
                                for l in &chains {
                                    if l.len() < 10 || l.first() == l.last() { continue; }
                                    for (e, q) in [(l[0], l[6]), (l[l.len() - 1], l[l.len() - 7])] {
                                        let on_x = (e.0 - x0).abs() < 1e-6 * (x1 - x0) || (e.0 - x1).abs() < 1e-6 * (x1 - x0);
                                        if on_x && (q.0 - e.0).abs() > 1e-9 && ((q.1 - e.1) / (q.0 - e.0)).abs() < 0.002 && e.1 > y0 && e.1 < y1 {
                                            let yv = if e.1.abs() < 0.005 * span_y { 0.0 } else { e.1 };
                                            if !found.iter().any(|f| (f - yv).abs() < 0.01 * span_y) { found.push(yv); }
                                        }
                                    }
                                }
                                for yv in found.into_iter().take(12) {
                                    if print { dline(&mut d, (ml, sy(yv)), (ml + pw, sy(yv)), "#666666", 1.6, "7 5"); } else { dline(&mut d, (ml, sy(yv)), (ml + pw, sy(yv)), col, 1.4, "2 4"); }
                                }
                            }
                            obst.extend(mine);
                        }
                        if drew { implicit_used += 1; }
                        else if !told {
                            // (a curve that exists somewhere else, as against one that has no real points at all)
                            if no_solutions(&f, (x0, x1, y0, y1), ineq.map(|(pos, _)| pos), ineq.is_some_and(|(_, st)| st)) { notes.push(format!("Row {n_row}: no real points satisfy this: {}", main.trim())); }
                            else { notes.push(format!("Row {n_row}: No part of this curve is inside the window: {}", main.trim())); }
                        }
                    }
                }
            }
            _ => {
                let expr: Box<dyn Fn(f64) -> Option<f64>> = match &row {
                    Row::Func(r) | Row::Ineq { rhs: r, .. } => { let r = r.clone(); Box::new(move |x| eval_full(&r, x, None, deg)) }
                    Row::Implicit(e) => {
                        let e = e.clone();
                        Box::new(move |x| {
                            let (a, b) = (eval_full(&e, x, Some(0.0), deg)?, eval_full(&e, x, Some(1.0), deg)?);
                            let slope = b - a;
                            (slope.abs() > 1e-12).then(|| -a / slope)
                        })
                    }
                    _ => unreachable!(),
                };
                work += (raw_row.len() as u64 + 8) * 1500;
                let (runs, asym) = if work > WORK_MAX {
                    notes.push(format!("Row {} is too long to draw together with the other rows: {}", ci + 1, main.trim()));
                    (vec![], vec![])
                } else { sample(&*expr) };
                // a curve that never comes into the window (xy = 4 in a window round the origin) is not drawn as scraps beyond it
                let (runs, mut asym) = {
                    let (mut runs, mut asym) = (runs, asym);
                    let seen = |r: &Vec<(f64, f64)>| r.iter().any(|q| q.1 >= y0 && q.1 <= y1) || (r.iter().any(|q| q.1 > y1) && r.iter().any(|q| q.1 < y0));
                    if matches!(&row, Row::Func(_) | Row::Implicit(_)) && !runs.is_empty() && !runs.iter().any(seen) {
                        notes.push(format!("Row {}: No part of this curve is inside the window: {}", ci + 1, main.trim()));
                        runs.clear();
                        asym.clear();
                    }
                    (runs, asym)
                };
                // a typo (an unknown function, a missing bracket) reads nothing anywhere: say which row
                if runs.is_empty() && !matches!(&row, Row::Implicit(_)) && (0..9).all(|k| expr(x0 + (x1 - x0) * k as f64 / 8.0).is_none()) {
                    notes.push(format!("Row {}: Cannot read: {}", ci + 1, main.trim()));
                }
                // holes and asymptotes sitting on grid points (x = k * step/4)
                let mut holes = vec![];
                if let Row::Func(r) = &row {
                    let q = xs / 4.0;
                    let mut k = (x0 / q).ceil();
                    let eps = (x1 - x0) * 1e-7;
                    let mut n = 0;
                    while k * q <= x1 + 1e-9 && n < MAX_LINES {
                        n += 1;
                        let c = k * q;
                        if in_dom(c) && f(r, c).map_or(true, |v| !v.is_finite()) {
                            if let (Some(l), Some(rr)) = (f(r, c - eps).filter(|v| v.is_finite()), f(r, c + eps).filter(|v| v.is_finite())) {
                                if (l - rr).abs() < 1e-3 * (1.0 + l.abs()) { holes.push((c, (l + rr) / 2.0)); }
                                else if !asym.iter().any(|a| (a - c).abs() < (x1 - x0) / 200.0) { asym.push(c); }
                            }
                        }
                        k += 1.0;
                    }
                }
                let strict = matches!(row, Row::Ineq { strict: true, .. });
                if let (Row::Ineq { above, .. }, true) = (&row, shade_on && !ineq_group.iter().any(|g| g.ci == ci)) {
                    let above = *above;
                    let edge = if above { y1 + span_y } else { y0 - span_y };
                    for r in &runs {
                        let mut poly: Vec<Pt> = r.iter().map(|(x, y)| (sx(*x), sy(y.clamp(y0 - span_y, y1 + span_y)))).collect();
                        poly.push((sx(r.last().unwrap().0), sy(edge)));
                        poly.push((sx(r[0].0), sy(edge)));
                        let pts: Vec<String> = poly.iter().map(|(a, b)| format!("{a:.2},{b:.2}")).collect();
                        put_shade(&mut d, &mut shade, late, format!(r##"<polygon points="{}" fill="{fill}" fill-opacity="{fill_op}" stroke="none"/>"##, pts.join(" ")));
                    }
                }
                for r in &runs {
                    let pts: Vec<Pt> = r.iter().map(|(x, y)| (sx(*x), sy(y.clamp(y0 - span_y, y1 + span_y)))).collect();
                    // an excluded boundary lying on the x-axis (y > 0): the axis is cut out under the dashes, so the gaps show
                    if strict && strict_pat != "" && pts.iter().all(|q| (q.1 - axy).abs() < 2.0) { d.polyline(&pts, "#fff", axw + 1.6); }
                    sline(&mut d, &pts, if strict { strict_pat } else { rdash });
                    obst.push(pts);
                }
                if p.b("asymptotes") {
                    for a in &asym { if print { dline(&mut d, (sx(*a), mt), (sx(*a), mt + ph), "#666666", 1.6, "7 5"); } else { dline(&mut d, (sx(*a), mt), (sx(*a), mt + ph), col, 1.4, "2 4"); } }
                }
                for (hx, hy) in &holes { d.circle(sx(*hx), sy(*hy), 4.5, "#fff", col, 2.0); }
                // piece endpoints: closed for <=, open for <
                if p.b("endpoints") {
                    for iv in &dom_iv {
                        for (b, c) in [(iv.lo, iv.lo_c), (iv.hi, iv.hi_c)] {
                            if b.is_finite() && b >= x0 - 1e-9 && b <= x1 + 1e-9 {
                                if let Some(yb) = expr(b).filter(|v| v.is_finite() && *v >= y0 && *v <= y1) { d.circle(sx(b), sy(yb), 5.0, if c { col } else { "#fff" }, col, 2.0); }
                            }
                        }
                    }
                }
                let xl = x0 + (x1 - x0) * (0.55 + 0.08 * (ci % 3) as f64);
                if let Some(yv) = expr(xl).filter(|v| *v > y0 && *v < y1) { label_at = Some((xl, yv)); }
            }
        }
        d.raw("</g>");
        if p.b("eq_labels") && answers {
            // the label is placed once every curve is known (see solve_labels); a marker keeps its place in the document
            let drawn_implicit = imp.is_some() && obst.len() > row_start;
            if label_at.is_some() || drawn_implicit {
                d.raw(&format!("<!--LBL{ci}-->"));
                reqs.push(LabelReq { ci, text: if imp.is_some() { label_text(&tidy_spacing(raw_row)) } else { label_text(raw_row) }, default_anchor: label_at.map(|(lx, ly)| (sx(lx), sy(ly))), start: row_start, end: obst.len(), halo: imp.is_some(), colour: col.to_string() });
            }
        }
    }
    let mut num_halos = String::new();
    if late { let from = d.len(); draw_numbers(&mut d, &obst, 1); num_halos = d.cut(from); draw_numbers(&mut d, &obst, 2); }
    if numbers_thinned { notes.push(if p.b("square") { "Some axis numbers were thinned because the window is very tall or wide; turn off Square cells or widen the window" } else { "Some axis numbers were thinned because the window is very tall or wide; widen the window or show every Nth number" }.to_string()); }
    // points: "A(1, 2), B(-3, 4)" or the older "(2,3) A; (-1,4) B"
    let pcol = colp(p, "pt_color", INK);
    let mut pt_count = 0usize;
    for seg in p.s("points").split(';') {
        let seg = seg.trim();
        if pt_count >= MAX_POINTS { notes.push(format!("Only the first {MAX_POINTS} points are drawn")); break; }
        let groups: Vec<(usize, usize)> = {
            let mut v = vec![];
            let mut at = 0;
            while let Some(a) = seg[at..].find('(') {
                let a = at + a;
                let Some(b) = seg[a..].find(')').map(|b| a + b) else { break };
                v.push((a, b));
                at = b + 1;
            }
            v
        };
        for (gi, (a, b)) in groups.iter().enumerate() {
            let n = nums(&seg[a + 1..*b]);
            if n.len() != 2 { continue; }
            pt_count += 1;
            if pt_count > MAX_POINTS { break; }
            let prev_end = if gi == 0 { 0 } else { groups[gi - 1].1 + 1 };
            let prefix: String = seg[prev_end..*a].trim().trim_start_matches(',').trim().to_string();
            let name = if !prefix.is_empty() { prefix } else if groups.len() == 1 { seg[b + 1..].trim().to_string() } else { String::new() };
            let (px, py) = (sx(n[0]), sy(n[1]));
            if p.s("pt_style") == "Cross" {
                d.line(px - 5.0, py - 5.0, px + 5.0, py + 5.0, &pcol, 2.2);
                d.line(px - 5.0, py + 5.0, px + 5.0, py - 5.0, &pcol, 2.2);
            } else { d.circle(px, py, 4.5, &pcol, "#fff", 1.2); }
            let coord = format!("({}, {})", fmt(n[0], 3), fmt(n[1], 3));
            let lab = match p.s("pt_label") {
                "None" => String::new(),
                "Letter and coordinates" => if name.is_empty() { coord } else { format!("{name}{coord}") },
                _ => name,
            };
            if !lab.is_empty() {
                if has_conic {
                    // keep clear of the x-axis numbers, and read over a gridline or a curve
                    let strip = (axy + 6.0, axy + 32.0 + dn);
                    let hits = |ly: f64| p.b("numbers") && y0 <= 0.0 && y1 >= 0.0 && ly + 9.0 > strip.0 && ly - 9.0 < strip.1;
                    let ly = [py - 11.0, py + 24.0, py - 28.0].into_iter().find(|ly| !hits(*ly)).unwrap_or(py - 11.0);
                    d.raw(&math_text_halo(px + 7.0, ly, 15.0, &lab));
                } else { math_text(&mut d, px + 7.0, py - 11.0, 15.0, "start", &lab, false); }
            }
        }
    }
    if skipped_rows > 0 { notes.push(format!("Only the first {MAX_ROWS} rows are drawn; {skipped_rows} more were ignored")); }
    // many problems are one problem: keep the first few and count the rest
    if notes.len() > 4 {
        let (keep, rest): (Vec<String>, Vec<String>) = notes.drain(..).partition(|n| n.starts_with("Only"));
        notes.extend(rest.iter().take(3).cloned());
        if rest.len() > 3 { notes.push(format!("and {} more rows could not be drawn", rest.len() - 3)); }
        notes.extend(keep);
    }
    if p.b("show_notes") && !notes.is_empty() {
        // a plate in the top corner that has no tick numbers: left of the y-axis numbers, else right of them
        let left = axx - ml >= 170.0;
        let (bx, avail) = if left { (ml + 6.0, axx - ml - 46.0) } else { (axx + 46.0, ml + pw - axx - 52.0) };
        let per_line = ((avail / 6.4) as usize).max(14);
        // (line, is math): the prose wraps at spaces; the row the note is about (after the last ": ") stands on its own line, set as math
        let mut lines: Vec<(String, bool)> = vec![];
        for n in notes.iter() {
            let (prose, expr) = match n.rfind(": ") { Some(i) if n.starts_with("Row ") => (&n[..i + 1], Some(n[i + 2..].trim())), _ => (n.as_str(), None) };
            let mut cur = String::new();
            for word in prose.split_whitespace() {
                if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > per_line { lines.push((std::mem::take(&mut cur), false)); }
                if !cur.is_empty() { cur.push(' '); }
                cur.push_str(word);
            }
            if !cur.is_empty() { lines.push((cur, false)); }
            if let Some(e) = expr {
                let mut cur = String::new();
                for word in e.split_whitespace() {
                    if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > per_line { lines.push((std::mem::take(&mut cur), true)); }
                    if !cur.is_empty() { cur.push(' '); }
                    cur.push_str(word);
                }
                if !cur.is_empty() { lines.push((cur, true)); }
            }
        }
        lines.truncate(10);
        let wd = lines.iter().map(|l| l.0.chars().count()).max().unwrap_or(0) as f64 * 6.4 + 12.0;
        let top = mt + 8.0;
        d.raw(&format!(r##"<rect x="{bx:.2}" y="{top:.2}" width="{wd:.2}" height="{:.2}" rx="3" fill="#fff" fill-opacity="0.95" stroke="#999" stroke-width="0.8"/>"##, 16.0 * lines.len() as f64 + 6.0));
        for (k, (l, is_math)) in lines.iter().enumerate() {
            if *is_math {
                d.raw(&format!(r##"<text x="{:.2}" y="{:.2}" font-size="13" font-family="{LABEL_SERIF}" fill="#111">{}</text>"##, bx + 6.0, top + 17.0 + 16.0 * k as f64, math_tspans(&label_text(l))));
            } else {
                d.raw(&format!(r##"<text x="{:.2}" y="{:.2}" font-size="12" font-family="Arial, Helvetica, sans-serif" fill="#111">{}</text>"##, bx + 6.0, top + 17.0 + 16.0 * k as f64, super::svg::esc(l)));
            }
        }
    }
    let mut out = d.finish();
    if late { out = out.replace("<!--SHADE-->", &format!(r#"<g clip-path="url(#plot)">{shade}</g>"#)).replace("<!--NUMS-->", &num_halos); }
    // equation labels, now that every curve is known (each marker holds its place in the document)
    if !reqs.is_empty() {
        for (ci, svg) in solve_labels(&reqs, &obst, &mut placed, [ml, mt, ml + pw, mt + ph], { let conic = reqs.iter().any(|r| r.halo); LabelStyle { size: if conic { 16.5 } else { 15.0 }, plates: conic } }) { out = out.replace(&format!("<!--LBL{ci}-->"), &svg); }
        for r in &reqs { out = out.replace(&format!("<!--LBL{}-->", r.ci), ""); }
    }
    if print { out = greyify(&out); }
    out
}

// ───────────────────────── number line ─────────────────────────

#[derive(Clone, Copy)]
pub(super) struct Interval { pub lo: f64, pub lo_c: bool, pub hi: f64, pub hi_c: bool }

pub(super) fn parse_ineq(s: &str) -> Vec<Interval> {
    let s = s.replace('\u{2264}', "<=").replace('\u{2265}', ">=").replace('\u{2212}', "-").replace(" ", "").to_lowercase();
    let mut out = vec![];
    let s = s.replace("or", "|").replace('\u{222a}', "|").replace("and", "&");
    for part in s.split('|') {
        let mut iv = Interval { lo: f64::NEG_INFINITY, lo_c: false, hi: f64::INFINITY, hi_c: false };
        for sub in part.split('&') {
            // tokenise into operands and ops
            let cs: Vec<char> = sub.chars().collect();
            let (mut ops, mut opnds, mut cur) = (vec![], vec![], String::new());
            let mut i = 0;
            while i < cs.len() {
                let c = cs[i];
                if c == '<' || c == '>' || c == '=' {
                    let mut op = c.to_string();
                    if i + 1 < cs.len() && cs[i + 1] == '=' && c != '=' { op.push('='); i += 1; }
                    opnds.push(std::mem::take(&mut cur)); ops.push(op);
                } else { cur.push(c); }
                i += 1;
            }
            opnds.push(cur);
            if ops.is_empty() { continue; }
            // build constraints between consecutive operands
            for (k, op) in ops.iter().enumerate() {
                let (l, r) = (&opnds[k], &opnds[k + 1]);
                let (xl, val) = if l == "x" { (true, r) } else if r == "x" { (false, l) } else { continue };
                let v: f64 = match val.parse() { Ok(v) => v, Err(_) => match eval(val, 0.0) { Some(v) => v, None => continue } };
                let closed = op.contains('=');
                let gt = if xl { op.starts_with('>') } else { op.starts_with('<') };
                if op == "=" { iv.lo = v; iv.hi = v; iv.lo_c = true; iv.hi_c = true; }
                else if gt { if v > iv.lo || (v == iv.lo && !closed) { iv.lo = v; iv.lo_c = closed; } }
                else if v < iv.hi || (v == iv.hi && !closed) { iv.hi = v; iv.hi_c = closed; }
            }
        }
        if iv.lo.is_finite() || iv.hi.is_finite() { out.push(iv); }
    }
    out
}

fn number_line_params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("min", "Minimum (from)", -1000.0, 1000.0, 1.0, -10.0),
        ParamDef::float("max", "Maximum (to)", -1000.0, 1000.0, 1.0, 10.0),
        ParamDef::float("step", "Tick step (count by)", 0.05, 100.0, 0.5, 1.0),
        ParamDef::int("label_every", "Number every n-th tick", 1, 20, 1),
        ParamDef::int("minor", "Minor ticks between ticks (0 = none)", 0, 9, 0),
        ParamDef::bool("numbers", "Show numbers", true),
        ParamDef::bool("arrows", "Arrowheads", true),
        ParamDef::text("inequality", "Inequality to graph (e.g. -2 < x <= 5, x > 3, x < -4 or x >= 1, x != 2, all real numbers, no solution)", "-2 < x <= 5"),
        ParamDef::list("points", "Points to mark: 3, -4, 5o (open), P(3/4), Q(3/2)", ""),
        ParamDef::text("label", "Axis label", ""),
        ParamDef::color("ineq_color", "Inequality colour", &SWATCHES, "#1f5fbf"),
        ParamDef::color("pt_color", "Point colour", &SWATCHES, "#c0392b"),
        ParamDef::text("step_text", "Count by as a fraction, e.g. 1/4 (overrides the tick step; numbers become fractions)", ""),
        ParamDef::choice("frac_style", "Fractions written as", &["Improper (5/4)", "Mixed (1 1/4)"], "Improper (5/4)"),
        ParamDef::text("inequality2", "Second graph, drawn separately above the line (its own colour)", ""),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        ParamDef::color("ineq2_color", "Second graph colour", &SWATCHES, "#c0392b"),
        ParamDef::bool("pt_values", "Write the value above unlettered points that are off the numbered ticks", false),
    ]
}

/// A typed number: 3, -2.5, 3/4, 1 1/4, pi/2.
pub(super) fn nl_num(s: &str) -> Option<f64> {
    let t = s.trim().replace('\u{2212}', "-");
    if t.is_empty() { return None; }
    let parts: Vec<&str> = t.split_whitespace().collect();
    if parts.len() == 2 && parts[1].contains('/') {
        let (w, f) = (parts[0].parse::<f64>().ok()?, eval(parts[1], 0.0)?);
        return Some(if w < 0.0 { w - f } else { w + f });
    }
    t.parse::<f64>().ok().or_else(|| eval(&t, 0.0))
}
/// Tick step and, when counted by a fraction a/b, its denominator b.
pub(super) fn nl_step(p: &Params) -> (f64, Option<i64>) {
    let st = p.s("step_text").trim();
    if let Some(v) = nl_num(st).filter(|v| *v > 1e-9) {
        let den = st.split('/').nth(1).and_then(|d| d.trim().parse::<i64>().ok()).filter(|d| *d > 1);
        return (v, den);
    }
    (p.f("step").max(1e-6), None)
}
/// Stacked fraction label n/d (already reduced) centred at x; `mixed` writes whole parts.
fn frac_label(d: &mut Doc, x: f64, y: f64, v: f64, den: i64, mixed: bool, size: f64) { frac_label_w(d, x, y, v, den, mixed, size, false); }
fn frac_label_w(d: &mut Doc, x: f64, y: f64, v: f64, den: i64, mixed: bool, size: f64, bold: bool) {
    let mut n = (v * den as f64).round() as i64;
    let mut dd = den;
    let g = { let (mut a, mut b) = (n.abs(), dd); while b != 0 { let t = a % b; a = b; b = t; } a.max(1) };
    n /= g; dd /= g;
    if dd == 1 { txtw(d, x, y + size * 0.34, size, "middle", &fmt(n as f64, 0), bold); return; }
    let neg = n < 0;
    let (whole, rem) = if mixed { (n.abs() / dd, n.abs() % dd) } else { (0, n.abs()) };
    let sign = if neg { "\u{2212}" } else { "" };
    let ws = if whole > 0 { format!("{sign}{whole}") } else { sign.to_string() };
    let fw = stw(&rem.max(dd).to_string(), size * 0.8).max(size * 0.6);
    let total = stw(&ws, size) + fw;
    let left = x - total / 2.0;
    if !ws.is_empty() { txtw(d, left, y + size * 0.34, size, "start", &ws, bold); }
    let cx = left + stw(&ws, size) + fw / 2.0 + 1.0;
    txtw(d, cx, y - 2.0, size * 0.8, "middle", &rem.to_string(), bold);
    d.line(cx - fw / 2.0, y + 1.0, cx + fw / 2.0, y + 1.0, INK, 1.0);
    txtw(d, cx, y + size * 0.8 + 1.0, size * 0.8, "middle", &dd.to_string(), bold);
}

/// Intervals for one graph row, with the extra words the reference accepts.
fn nl_intervals(s: &str) -> Vec<Interval> {
    let l = s.trim().to_lowercase();
    if l.is_empty() || l.contains("no solution") { return vec![]; }
    if l.contains("all real") { return vec![Interval { lo: f64::NEG_INFINITY, lo_c: false, hi: f64::INFINITY, hi_c: false }]; }
    let ne = l.replace('\u{2260}', "!=");
    if let Some(i) = ne.find("!=") {
        if let Some(v) = nl_num(&ne[i + 2..]) {
            return vec![Interval { lo: f64::NEG_INFINITY, lo_c: false, hi: v, hi_c: false }, Interval { lo: v, lo_c: false, hi: f64::INFINITY, hi_c: false }];
        }
    }
    parse_ineq(&l)
}

/// Canvas of the number line (width, height, left and right end of the scale, axis y), shared with
/// its drag handles. The reference's is 676 wide: the axis runs 16 to 660 (arrow tips), its ticks
/// 22 in from each end; the axis sits 25 down on a 78 tall canvas and the canvas grows only when
/// a second graph row or point names need room above it.
pub(super) fn nl_geom(p: &Params) -> (f64, f64, f64, f64, f64) {
    let (_, den) = nl_step(p);
    let above = if !p.s("inequality2").trim().is_empty() { 18.0 } else if !p.s("points").trim().is_empty() || !p.s("inequality").trim().is_empty() { 8.0 } else { 0.0 };
    let y = 25.0 + above;
    let label = p.s("label");
    let w = 676.0 + if label.is_empty() { 0.0 } else { (stw(label, 16.0) + 12.0).max(0.0) };
    (w, y + 53.0 + 16.0 * (lscale(p) - 1.0) + if den.is_some() { 8.0 } else { 0.0 }, 38.0, 638.0, y)
}

/// The reference's axis ends: the line runs 22 past the scale and ends in a 13 x 13 solid triangle.
fn axis_heads(d: &mut Doc, x0: f64, x1: f64, y: f64) {
    d.line(x0 - 22.0, y, x0, y, INK, 2.4);
    d.line(x1, y, x1 + 22.0, y, INK, 2.4);
    d.poly(&[(x0 - 24.6, y), (x0 - 11.6, y - 6.5), (x0 - 11.6, y + 6.5)], INK, INK, 1.0);
    d.poly(&[(x1 + 24.6, y), (x1 + 11.6, y - 6.5), (x1 + 11.6, y + 6.5)], INK, INK, 1.0);
}

fn number_line_render(p: &Params) -> String {
    let (lo, mut hi) = (p.f("min"), p.f("max"));
    if hi <= lo { hi = lo + 1.0; }
    let (step, den) = nl_step(p);
    let mixed = p.s("frac_style").starts_with("Mixed");
    let (w, h, x0, x1, y) = nl_geom(p);
    let fs = 16.0 * lscale(p);
    let sx = |v: f64| x0 + (v - lo) / (hi - lo) * (x1 - x0);
    let mut d = new_doc(w, h);
    if p.b("arrows") { axis_heads(&mut d, x0, x1, y); }
    d.line(x0, y, x1, y, INK, 2.4);
    let k0 = (lo / step - 1e-9).ceil() as i64;
    let k1 = ((hi + 1e-9) / step).floor() as i64;
    let every = p.i("label_every").max(1);
    let minor = p.i("minor");
    let mut numbered: Vec<f64> = vec![];
    for k in k0..=k1 {
        let v = k as f64 * step;
        d.line(sx(v), y - 9.0, sx(v), y + 9.0, INK, 2.0);
        if p.b("numbers") && (k - k0) % every == 0 {
            numbered.push(v);
            match den { Some(dd) => frac_label_w(&mut d, sx(v), y + 24.0, v, dd, mixed, fs, true), None => txtw(&mut d, sx(v), y + 23.0 + fs * 0.34, fs, "middle", &fmt(v, 3), true) }
        }
        if minor > 0 && k < k1 {
            for m in 1..=minor { let mv = v + step * m as f64 / (minor + 1) as f64; d.line(sx(mv), y - 4.0, sx(mv), y + 4.0, INK, 1.0); }
        }
    }
    if !p.s("label").is_empty() { math_text(&mut d, x1 + 26.0, y, 16.0, "start", p.s("label"), false); }
    // graphs: row 1 on the line, row 2 (its own colour) drawn separately just above it
    let rows = [(p.s("inequality").to_string(), colp(p, "ineq_color", BLUE), 0.0), (p.s("inequality2").to_string(), colp(p, "ineq2_color", "#c0392b"), -18.0)];
    for (text, col, dy) in rows.iter() {
        let col = col.as_str();
        let yy = y + dy;
        for iv in nl_intervals(text) {
            let a = if iv.lo.is_finite() { iv.lo.max(lo) } else { lo };
            let b = if iv.hi.is_finite() { iv.hi.min(hi) } else { hi };
            if b >= a && iv.hi != iv.lo { d.line(sx(a), yy, sx(b), yy, col, 5.0); }
            if !iv.lo.is_finite() && p.b("arrows") { d.arrow(sx(lo) + 4.0, yy, x0 - 24.0, yy, col, 3.5); }
            if !iv.hi.is_finite() && p.b("arrows") { d.arrow(x1 - 4.0, yy, x1 + 24.0, yy, col, 3.5); }
            for (v, closed) in [(iv.lo, iv.lo_c), (iv.hi, iv.hi_c)] {
                if v.is_finite() && v >= lo - 1e-9 && v <= hi + 1e-9 {
                    d.circle(sx(v), yy, 6.5, if closed { col } else { "#fff" }, col, 2.4);
                }
            }
        }
    }
    let red_s = colp(p, "pt_color", "#c0392b");
    let red = red_s.as_str();
    for t in p.s("points").split(',') {
        let t = t.trim();
        if t.is_empty() { continue; }
        let (t, open) = match t.strip_suffix('o') { Some(s) if !s.ends_with(|c: char| c.is_alphabetic()) => (s, true), _ => (t, false) };
        // "P(3/4)": a letter names the point
        let (name, num) = match (t.find('('), t.rfind(')')) {
            (Some(a), Some(b)) if b > a && a > 0 => (t[..a].trim(), &t[a + 1..b]),
            _ => ("", t),
        };
        let Some(v) = nl_num(num) else { continue };
        if v < lo - 1e-9 || v > hi + 1e-9 { continue; }
        d.circle(sx(v), y, 6.0, if open { "#fff" } else { red }, red, 2.2);
        if !name.is_empty() {
            d.raw(&format!(r##"<text x="{:.2}" y="{:.2}" font-size="17" text-anchor="middle" font-family="{SERIF}" font-style="italic" font-weight="bold" fill="#000">{}</text>"##, sx(v), y - 16.0, super::svg::esc(name)));
        } else if p.b("pt_values") && !numbered.iter().any(|n| (n - v).abs() < 1e-9) {
            match den { Some(dd) if ((v * dd as f64) - (v * dd as f64).round()).abs() < 1e-6 => frac_label(&mut d, sx(v), y - 26.0, v, dd, mixed, 13.0), _ => put(&mut d, sx(v), y - 18.0, &fmt(v, 3), 13.0) }
        }
    }
    d.finish()
}

// ───────────────────────── angles & parallel lines ─────────────────────────

/// Screen point `q` turned `deg` counterclockwise (as seen) about screen point `c`.
pub(super) fn turn_scr(q: Pt, c: Pt, deg: f64) -> Pt {
    let (s, co) = (deg.to_radians().sin(), deg.to_radians().cos());
    let (dx, dy) = (q.0 - c.0, q.1 - c.1);
    (c.0 + dx * co + dy * s, c.1 - dx * s + dy * co)
}

/// `2 = 2x + 10; 3 = y` -> [(2, "2x + 10"), (3, "y")].
fn num_text_map(s: &str) -> Vec<(usize, String)> {
    s.split(|c| c == ';' || c == '\n')
        .filter_map(|it| { let (k, v) = it.split_once('=')?; Some((k.trim().parse().ok()?, v.trim().to_string())) })
        .collect()
}

/// Angular sector mark (math degrees ccw, y-up) at screen center c, with a math-typeset label.
fn sector(d: &mut Doc, c: Pt, a0: f64, a1: f64, r: f64, text: &str, right: bool, size: f64) {
    let dir = |a: f64| (a.to_radians().cos(), -a.to_radians().sin());
    let span = a1 - a0;
    if right && (span - 90.0).abs() < 0.3 {
        let (u, w) = (dir(a0), dir(a1));
        d.right_angle(c, u, w, r * 0.55, INK, 1.3);
    } else {
        let n = 24;
        let pts: Vec<Pt> = (0..=n).map(|i| { let a = a0 + span * i as f64 / n as f64; let q = dir(a); (c.0 + q.0 * r, c.1 + q.1 * r) }).collect();
        d.polyline(&pts, INK, 1.4);
    }
    if !text.is_empty() {
        let mid = dir((a0 + a1) / 2.0);
        let tr = r + 8.0 + 0.5 * (tw(text, size) * 0.85 * mid.0.abs() + size * mid.1.abs());
        mlabel(d, c.0 + mid.0 * tr, c.1 + mid.1 * tr, size, text, "");
    }
}

/// Canvas size and parallel-line rows (screen y) for the parallel-lines figure.
/// x where transversal `ti` crosses the first line: the transversal passes through the middle of
/// the lines at the figure's centre (the reference centres the crossings, not the first one).
pub(super) fn pl_cx0(p: &Params, ti: usize, ys: &[f64], w: f64) -> f64 {
    let nt = p.i("transversals").clamp(1, 2);
    let th = (if ti == 0 { p.f("angle").clamp(10.0, 170.0) } else { p.f("angle_b").clamp(10.0, 170.0) }).to_radians();
    let mid = if nt == 1 { w / 2.0 } else if ti == 0 { w / 2.0 - 70.0 } else { w / 2.0 + 70.0 };
    mid + th.cos() / th.sin() * (ys[ys.len() - 1] - ys[0]) / 2.0
}

pub(super) fn pl_layout(p: &Params) -> (f64, f64, Vec<f64>) {
    let nl = p.i("lines").clamp(2, 3) as usize;
    // the reference's lines are 300 long and 100 apart, the transversal runs 90 past the outer lines
    let gap = 100.0 * p.f("gap").clamp(0.5, 3.0);
    let (w, h) = (400.0, 240.0 + gap * (nl as f64 - 1.0));
    let ys = (0..nl).map(|i| h / 2.0 - gap * (nl as f64 - 1.0) / 2.0 + gap * i as f64).collect();
    (w, h, ys)
}

/// Filled arrowheads at both ends of line a-b (a line, not a segment).
fn line_heads(d: &mut Doc, a: Pt, b: Pt) {
    for (from, to) in [(a, b), (b, a)] {
        let u = unit_v(from, to);
        let nn = (-u.1, u.0);
        let back = (to.0 - u.0 * 12.0, to.1 - u.1 * 12.0);
        d.poly(&[to, (back.0 + nn.0 * 5.0, back.1 + nn.1 * 5.0), (back.0 - nn.0 * 5.0, back.1 - nn.1 * 5.0)], INK, INK, 1.0);
    }
}

// ───────────────────────── 3D solids ─────────────────────────

type P3 = (f64, f64, f64);
const OBL_K: f64 = 0.5;
const OBL_A: f64 = 45.0;
/// Cabinet-style oblique projection; `sx` = +1 sends depth back to the right, -1 to the left.
pub(super) fn oblp(p: P3, sx: f64) -> Pt {
    let a = OBL_A.to_radians();
    (p.0 + sx * p.1 * OBL_K * a.cos(), p.2 + p.1 * OBL_K * a.sin())
}
/// Direction toward the viewer for `oblp` (faces whose normal points this way are visible).
fn oblv(sx: f64) -> P3 { let a = OBL_A.to_radians(); (sx * OBL_K * a.cos(), -1.0, OBL_K * a.sin()) }
pub(super) fn depth_sx(p: &Params) -> f64 { if p.s("depth") == "Left" { -1.0 } else { 1.0 } }
fn sub3(a: P3, b: P3) -> P3 { (a.0 - b.0, a.1 - b.1, a.2 - b.2) }
fn dot3(a: P3, b: P3) -> f64 { a.0 * b.0 + a.1 * b.1 + a.2 * b.2 }
fn dist3(a: P3, b: P3) -> f64 { dot3(sub3(a, b), sub3(a, b)).sqrt() }
fn mid3(a: P3, b: P3) -> P3 { ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0, (a.2 + b.2) / 2.0) }

// ----- solid labels: serif, sized, per-measure Measure / Text / None -----
const SERIF: &str = "Liberation Serif, DejaVu Serif, Times New Roman, Times, serif";
fn lsz(p: &Params) -> f64 { match p.s("label_size") { "Large" => 22.0, "Small" => 14.0, _ => 18.0 } }
fn label_size_param() -> ParamDef { ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium") }
/// Short letter labels (r, h, x, A, B1) are set in italic like math symbols.
fn is_var(s: &str) -> bool {
    let t = s.trim();
    let mut ch = t.chars();
    matches!(ch.next(), Some(c) if c.is_alphabetic()) && t.chars().count() <= 3 && t.chars().all(|c| c.is_alphanumeric() || c == '\'')
}
fn stw(s: &str, size: f64) -> f64 { s.chars().count() as f64 * size * 0.5 }
/// Solid label centred on (x, y). A non-empty `id` makes it a movable label (`labels::wrap`).
fn sput(d: &mut Doc, id: &str, x: f64, y: f64, text: &str, size: f64) { sput_k(d, id, x, y, text, size, 0.34); }
/// `sput` with the baseline `k` em below the label's centre (the reference sets it 0.24 em below).
fn sput_k(d: &mut Doc, id: &str, x: f64, y: f64, text: &str, size: f64, k: f64) {
    let style = if is_var(text) { r#" font-style="italic""# } else { "" };
    let t = format!(r##"<text x="{x:.2}" y="{:.2}" font-size="{size}" text-anchor="middle" font-family="{SERIF}"{style} fill="#111827" stroke="#fff" stroke-width="{}" stroke-linejoin="round" paint-order="stroke">{}</text>"##, y + size * k, size * 0.2, super::svg::esc(text));
    if id.is_empty() { d.raw(&t); } else { d.raw(&super::labels::wrap(id, (x, y), super::labels::text_half(text, size), &t)); }
}
/// Medium 20, Large 28, Small 16: the reference's solid label sizes (in its own drawing units).
fn lsz3(p: &Params) -> f64 { match p.s("label_size") { "Large" => 28.0, "Small" => 16.0, _ => 20.0 } }
/// `sbeside` with the reference's spacing: 5.4 units clear of the edge sideways, 5.8 above or below, baseline 0.24 em under the centre.
fn sbeside_r(d: &mut Doc, id: &str, a: Pt, b: Pt, t: f64, center: Pt, text: &str, size: f64) {
    let m = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
    let u = unit_v(a, b);
    let mut n = (-u.1, u.0);
    if n.0 * (m.0 - center.0) + n.1 * (m.1 - center.1) < 0.0 { n = (-n.0, -n.1); }
    let off = 0.5 * (stw(text, size) * n.0.abs() + size * 0.9 * n.1.abs()) + 5.4 * n.0.abs() + 5.8 * n.1.abs();
    sput_k(d, id, m.0 + n.0 * off, m.1 + n.1 * off, text, size, 0.24);
}
/// Label for measure `key` of value `v`: `<key>_lab` picks Measure / Text / None (`def` when the
/// key is absent, e.g. older saved figures) and `<key>_label` holds the text. `hidden` comes from
/// the older "Dimensions to hide" list and always wins.
fn mtext(p: &Params, key: &str, def: &str, v: f64, hidden: bool) -> Option<String> {
    if hidden { return None; }
    let mode = match p.s(&format!("{key}_lab")) { "" => def, m => m };
    match mode {
        "Text" => { let t = p.s(&format!("{key}_label")).trim(); (!t.is_empty()).then(|| t.to_string()) }
        "None" => None,
        _ => Some(fmtu(v, p.i("dec"), p.s("unit"))),
    }
}
const LAB_OPTS: [&str; 3] = ["Measure", "Text", "None"];
fn hide_list(p: &Params) -> Vec<String> { tokens(p.s("hide")).iter().map(|s| s.to_lowercase()).collect() }
fn corner_name(i: usize) -> String {
    let c = (b'A' + (i % 26) as u8) as char;
    if i < 26 { c.to_string() } else { format!("{c}{}", i / 26) }
}
/// Corner letter just outside vertex `q` (screen), pushed away from `center`.
fn name_at(d: &mut Doc, id: &str, q: Pt, center: Pt, name: &str, size: f64) {
    let u = unit_v(center, q);
    let u = if u.0.is_finite() { u } else { (0.0, -1.0) };
    sput(d, id, q.0 + u.0 * (size * 0.9), q.1 + u.1 * (size * 0.9), name, size);
}

struct Solid { v: Vec<P3>, faces: Vec<Vec<usize>> }

/// Draw a convex solid: shaded visible faces, solid visible edges, dashed hidden edges (when
/// `hidden`).
fn draw_solid(d: &mut Doc, m: &Map, s: &Solid, proj: &dyn Fn(P3) -> Pt, view: P3, hidden: bool) {
    let n = s.v.len() as f64;
    let cen = (s.v.iter().map(|p| p.0).sum::<f64>() / n, s.v.iter().map(|p| p.1).sum::<f64>() / n, s.v.iter().map(|p| p.2).sum::<f64>() / n);
    let mut vis = vec![];
    let mut normals = vec![];
    for f in &s.faces {
        let mut nrm = (0.0, 0.0, 0.0);
        for i in 0..f.len() {
            let (a, b) = (s.v[f[i]], s.v[f[(i + 1) % f.len()]]);
            nrm = (nrm.0 + (a.1 - b.1) * (a.2 + b.2), nrm.1 + (a.2 - b.2) * (a.0 + b.0), nrm.2 + (a.0 - b.0) * (a.1 + b.1));
        }
        let fc = f.iter().fold((0.0, 0.0, 0.0), |acc, i| (acc.0 + s.v[*i].0, acc.1 + s.v[*i].1, acc.2 + s.v[*i].2));
        let fc = (fc.0 / f.len() as f64, fc.1 / f.len() as f64, fc.2 / f.len() as f64);
        if dot3(nrm, sub3(fc, cen)) < 0.0 { nrm = (-nrm.0, -nrm.1, -nrm.2); }
        let l = dot3(nrm, nrm).sqrt().max(1e-12);
        nrm = (nrm.0 / l, nrm.1 / l, nrm.2 / l);
        vis.push(dot3(nrm, view) > 1e-6);
        normals.push(nrm);
    }
    for (fi, f) in s.faces.iter().enumerate() {
        if !vis[fi] { continue; }
        let l = dot3(normals[fi], (-0.35, -0.45, 0.82));
        let col = mix((196.0, 216.0, 240.0), (246.0, 250.0, 255.0), (l + 1.0) / 2.0);
        let pts: Vec<Pt> = f.iter().map(|i| m.p(proj(s.v[*i]))).collect();
        d.poly(&pts, &col, &col, 0.8);
    }
    let mut edges: std::collections::BTreeMap<(usize, usize), bool> = Default::default();
    for (fi, f) in s.faces.iter().enumerate() {
        for i in 0..f.len() {
            let (a, b) = (f[i], f[(i + 1) % f.len()]);
            let e = edges.entry((a.min(b), a.max(b))).or_insert(false);
            if vis[fi] { *e = true; }
        }
    }
    if hidden {
        for ((a, b), visible) in &edges {
            if !*visible { dline(d, m.p(proj(s.v[*a])), m.p(proj(s.v[*b])), INK, 1.6, "6 5"); }
        }
    }
    for ((a, b), visible) in &edges {
        if *visible { let (p, q) = (m.p(proj(s.v[*a])), m.p(proj(s.v[*b]))); d.line(p.0, p.1, q.0, q.1, INK, 2.4); }
    }
}

pub(super) fn poly_base(kind: &str, l: f64, w: f64, n: usize, side: f64) -> Vec<P3> {
    match kind {
        "Right triangle" => vec![(0.0, 0.0, 0.0), (l, 0.0, 0.0), (l, w, 0.0)],
        "Isosceles triangle" => vec![(0.0, 0.0, 0.0), (l, 0.0, 0.0), (l / 2.0, w, 0.0)],
        "Regular polygon" => {
            let half = PI / n as f64;
            let r = side / (2.0 * half.sin());
            (0..n).map(|k| { let t = -PI / 2.0 - half + 2.0 * PI * k as f64 / n as f64; (r * t.cos(), r * t.sin(), 0.0) }).collect()
        }
        _ => vec![(0.0, 0.0, 0.0), (l, 0.0, 0.0), (l, w, 0.0), (0.0, w, 0.0)],
    }
}

/// Right-angle square at 3D point o with arms toward o+e1 and o+e2 (screen size fixed).
fn rmark3(d: &mut Doc, m: &Map, proj: &dyn Fn(P3) -> Pt, o: P3, e1: P3, e2: P3) {
    let (po, p1, p2) = (m.p(proj(o)), m.p(proj((o.0 + e1.0, o.1 + e1.1, o.2 + e1.2))), m.p(proj((o.0 + e2.0, o.1 + e2.1, o.2 + e2.2))));
    let (u, w) = (unit_v(po, p1), unit_v(po, p2));
    let s = 10.0;
    let a = (po.0 + u.0 * s, po.1 + u.1 * s);
    let c = (po.0 + w.0 * s, po.1 + w.1 * s);
    d.polyline(&[a, (a.0 + w.0 * s, a.1 + w.1 * s), c], INK, 1.2);
}

fn solid_common(unit: &str) -> Vec<ParamDef> {
    vec![
        ParamDef::bool("hidden", "Show hidden edges (dashed)", true),
        ParamDef::bool("right", "Right-angle squares", true),
        ParamDef::text("unit", "Unit", unit),
        ParamDef::int("dec", "Decimal places", 0, 3, 1),
    ]
}

// ----- solids' Measures card: typed (given) vs worked-out values -----

/// A solid's optional measure kept as text ("" = worked out from the others): the typed value
/// when it is a positive number or expression (sqrt, pi, fractions), else None.
pub(super) fn typed(p: &Params, k: &str) -> Option<f64> {
    let t = p.s(k).trim();
    if t.is_empty() { return None; }
    eval(&t.replace('\u{221a}', "sqrt").replace('\u{3c0}', "pi"), 0.0).filter(|v| v.is_finite() && *v > 0.0)
}
/// Label for a measure whose value box is text (`vk`): Measure prints what was typed ("as you
/// typed it") or the worked-out value rounded; Text and None as usual.
fn tlabel(p: &Params, key: &str, vk: &str, def: &str, v: f64) -> Option<String> {
    let mode = match p.s(&format!("{key}_lab")) { "" => def, m => m };
    if mode == "Measure" && typed(p, vk).is_some() {
        let t = p.s(vk).trim().replace("sqrt", "\u{221a}").replace("pi", "\u{3c0}");
        let u = p.s("unit").trim();
        return Some(if u.is_empty() { t } else { format!("{t} {u}") });
    }
    mtext(p, key, def, v, false)
}
/// Height of an oblique prism/cylinder: typed, or worked out from a typed slanted edge and the
/// lean (any two of height, lean and slanted edge, as the reference allows).
pub(super) fn edge_height(p: &Params, oblique: bool) -> f64 {
    if oblique && let Some(e) = typed(p, "edge") {
        let l = p.f("lean");
        if e > l.abs() + 1e-6 { return (e * e - l * l).sqrt(); }
    }
    p.f("height")
}
/// Worked-out text of a measure (number rounded to the figure's decimals, no unit).
fn ctext(p: &Params, v: f64) -> String { fmt(v, p.i("dec")) }
/// Diameter row of the round solids: a linked row (= 2 x radius) with its own label.
fn diameter_params() -> [ParamDef; 2] {
    [
        ParamDef::choice("d_lab", "Diameter label (drawn right across the circle)", &LAB_OPTS, "None"),
        ParamDef::text("d_label", "Diameter label text", "d"),
    ]
}
/// The reference's SHAPE/Measures card of each solid. The card is static, so rows that only apply
/// to one base or to oblique solids are grouped and say so.
fn measures_solid(id: &str) -> Option<super::MeasureCard> {
    use super::{MeasureCard, MeasureGroup, MeasureRow};
    let row = |name, value_key, label_key, text_key| MeasureRow { name, name_key: None, value_key, unit: "$unit", label_key, text_key, tick_key: None, equals: None };
    let link = |name, value_key, label_key, text_key, eq| MeasureRow { equals: Some(eq), ..row(name, value_key, label_key, text_key) };
    let g = |title, rows| MeasureGroup { title, rows };
    // the Diameter row (value "diam") replaces Radius with "Give the diameter instead" (card_visible)
    let radius = || vec![row("Radius", "radius", "r_lab", "r_label"), row("Diameter", "diam", "r_lab", "d_label"), link("Diameter", "diameter", "d_lab", "d_label", "2 \u{d7} radius")];
    let (help, groups) = match id {
        "sphere" => ("Give the radius. The diameter is worked out; its label button draws it right across the circle.", vec![g("", radius())]),
        "cylinder" => (
            "Give every measure. Worked-out measures are shown faintly. Oblique: give the slanted edge to have the height worked out from it and the lean. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}.",
            vec![g("", { let mut r = radius(); r.push(row("Height", "height", "h_lab", "h_label")); r }), g("Oblique", vec![row("Lean", "lean", "lean_lab", "lean_label"), row("Slanted edge", "edge", "edge_lab", "edge_label")])],
        ),
        "cone" => (
            "Give the radius and the height, or type the slant height to have the height worked out. Worked-out measures are shown faintly. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}.",
            vec![
                g("", { let mut r = radius(); r.extend([row("Height", "height", "h_lab", "h_label"), row("Slant height", "slant", "l_lab", "l_label")]); r }),
                g("Oblique", vec![row("Lean", "lean", "lean_lab", "lean_label")]),
            ],
        ),
        "pyramid" => (
            "Give every measure. Type the slant height to have the height worked out. Worked-out measures are shown faintly. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}.",
            vec![
                g("Rectangle base", vec![row("Length", "length", "length_lab", "length_label"), row("Width", "width", "width_lab", "width_label")]),
                g("Regular polygon base", vec![row("Side", "side", "side_lab", "side_label"), link("Apothem", "apothem", "apothem_lab", "apothem_label", "worked out")]),
                g("", vec![row("Height", "height", "height_lab", "height_label"), row("Slant height", "slant", "slant_lab", "slant_label")]),
                g("Oblique", vec![row("Lean", "lean", "lean_lab", "lean_label")]),
            ],
        ),
        "prism" => (
            "Give every measure. For a triangle base, Length is the triangle's base and Width the triangle's height; when the prism lies on its side, Height is its length. Oblique: give the slanted edge to have the height worked out. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}.",
            vec![
                g("Rectangle or triangle base", vec![
                    row("Length", "length", "length_lab", "length_label"),
                    row("Width", "width", "width_lab", "width_label"),
                    link("Slanted side", "hyp", "hyp_lab", "hyp_label", "worked out"),
                ]),
                g("Regular polygon base", vec![row("Side", "side", "side_lab", "side_label"), link("Apothem", "apothem", "apothem_lab", "apothem_label", "worked out")]),
                g("", vec![row("Height", "height", "height_lab", "height_label")]),
                g("Oblique", vec![row("Lean", "lean", "lean_lab", "lean_label"), row("Slanted edge", "edge", "edge_lab", "edge_label")]),
            ],
        ),
        _ => return None,
    };
    Some(MeasureCard { title: "Shape", help, groups })
}

/// Worked-out values of the solids (the grey text in empty boxes and on linked rows).
fn computed_solid(id: &str, p: &Params) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let c = |v: f64| ctext(p, v);
    if matches!(id, "sphere" | "cylinder" | "cone") { out.push(("diameter", c(2.0 * solid_r(p)))); }
    let oblique = p.s("stands") == "Oblique";
    match id {
        "cylinder" => {
            let h = edge_height(p, oblique);
            if typed(p, "edge").is_none() { out.push(("edge", c((h * h + p.f("lean").powi(2)).sqrt()))); } else { out.push(("height", c(h))); }
        }
        "cone" => {
            let (r, h) = (solid_r(p), cone_height(p));
            let lean = if oblique { p.f("lean") } else { 0.0 };
            if typed(p, "slant").is_none() { out.push(("slant", c(((r - lean).powi(2) + h * h).sqrt()))); } else { out.push(("height", c(h))); }
        }
        "pyramid" => {
            let a = pyramid_apothem(p);
            let h = pyramid_height(p);
            let poly = poly_base(p.s("base"), p.f("length"), p.f("width"), p.i("n").clamp(3, 12) as usize, p.f("side"));
            out.push(("apothem", c(dist3(mid3(poly[0], poly[1]), centroid3(&poly)))));
            let lean = if oblique { p.f("lean") } else { 0.0 };
            if typed(p, "slant").is_none() { out.push(("slant", c((a * a + h * h + lean * lean).sqrt()))); } else { out.push(("height", c(h))); }
        }
        "prism" => {
            let (l, w) = (p.f("length"), p.f("width"));
            let hyp = if p.s("base") == "Isosceles triangle" { ((l / 2.0).powi(2) + w * w).sqrt() } else { (l * l + w * w).sqrt() };
            out.push(("hyp", c(hyp)));
            let poly = poly_base("Regular polygon", l, w, p.i("n").clamp(3, 12) as usize, p.f("side"));
            out.push(("apothem", c(dist3(mid3(poly[0], poly[1]), centroid3(&poly)))));
            let h = edge_height(p, oblique);
            if typed(p, "edge").is_none() { out.push(("edge", c((h * h + p.f("lean").powi(2)).sqrt()))); } else { out.push(("height", c(h))); }
        }
        _ => {}
    }
    if let Some(m) = solid_msg(id, p) { out.push(("__msg", m)); }
    out
}

/// The reference's red line under a solid's measures when the typed ones can't all hold.
pub(super) fn solid_msg(id: &str, p: &Params) -> Option<String> {
    let oblique = p.s("stands") == "Oblique";
    let bad = |k: &str| !p.s(k).trim().is_empty() && typed(p, k).is_none();
    match id {
        "cylinder" | "prism" => {
            let lying = id == "prism" && p.s("base") != "Rectangle" && p.s("sits") == "Lying on its side";
            if !oblique || lying { return None; }
            if bad("edge") { return Some("Type a number for the slanted edge, like 12, 2.5, 5/2 or 3\u{221a}2.".into()); }
            let e = typed(p, "edge")?;
            (e <= p.f("lean").abs() + 1e-6).then(|| "The height, lean and slanted edge don\u{2019}t fit together: the slanted edge has to be longer than the lean. Clear one to have it worked out.".into())
        }
        "cone" | "pyramid" => {
            if bad("slant") { return Some("Type a number for the slant height, like 12, 2.5, 5/2 or 3\u{221a}2.".into()); }
            let s = typed(p, "slant")?;
            if oblique {
                let what = if id == "cone" { "cone" } else { "pyramid" };
                return Some(format!("An oblique {what}\u{2019}s slant height is worked out from its height and lean. Clear the slant height to have it worked out."));
            }
            let (min, what) = if id == "cone" { (solid_r(p), "radius") } else if p.s("base") == "Regular polygon" { (pyramid_apothem(p), "apothem") } else { (pyramid_apothem(p), "half the width") };
            (s <= min + 1e-6).then(|| format!("The height and slant height don\u{2019}t fit together: the slant height has to be longer than the {what}. Clear one to have it worked out."))
        }
        _ => None,
    }
}

/// Old specs: the `hide` list and `measure` choice become per-measure label modes.
fn migrate_solid(id: &str, spec: &crate::model::FigureSpec, p: &mut Params) {
    let old = |k: &str| spec.params.get(k);
    let olds = |k: &str| old(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let oldb = |k: &str| old(k).and_then(|v| v.as_bool());
    let oldf = |k: &str| old(k).and_then(|v| v.as_f64());
    // hide list -> label None
    for t in tokens(&olds("hide")) {
        let key = match t.to_lowercase().as_str() {
            "length" => "length_lab", "width" => "width_lab", "height" => if matches!(id, "cylinder" | "cone") { "h_lab" } else { "height_lab" },
            "side" => "side_lab", "slant" => if id == "cone" { "l_lab" } else { "slant_lab" }, "radius" => "r_lab",
            "hypotenuse" | "slanted" => "hyp_lab", "apothem" => "apothem_lab", "lean" => "lean_lab", "edge" => "edge_lab",
            _ => continue,
        };
        p.set_s(key, "None");
    }
    // radius / diameter
    if olds("measure") == "Diameter" {
        let (m, t) = (p.s("r_lab").to_string(), p.s("r_label").to_string());
        p.set_s("d_lab", &m);
        p.set_s("d_label", &t);
        p.set_s("r_lab", "None");
    }
    // the slant height was given instead of the height
    if matches!(id, "cone" | "pyramid") {
        let given = olds("given") == "Slant height";
        let sl = oldf("slant");
        if old("given").is_some() || sl.is_some() {
            p.set_s("slant", &if given { sl.map(|v| fmt(v, 3)).unwrap_or_default() } else { String::new() });
        }
        let lab = if id == "cone" { "l_lab" } else { "slant_lab" };
        if oldb("show_slant") == Some(false) { p.set_s(lab, "None"); }
        if id == "cone" && oldb("show_slant") == Some(true) && old("l_lab").is_none() { p.set_s(lab, "Measure"); }
    }
    // lean labels did not exist / defaulted to None before the rework
    if old("stands").is_some() && old("lean_lab").is_none() && id != "sphere" { p.set_s("lean_lab", "None"); }
}

/// Prism vertices: (base ring then top ring, ring size, lying on its side, lean actually used).
/// Lying: the base polygon is the front face (x-z plane) and `height` is the depth.
pub(super) fn prism_geom(p: &Params) -> (Vec<P3>, usize, bool, f64) {
    let kind = p.s("base");
    let base = poly_base(kind, p.f("length"), p.f("width"), p.i("n").clamp(3, 12) as usize, p.f("side"));
    let k = base.len();
    let lying = kind != "Rectangle" && p.s("sits") == "Lying on its side";
    let oblique = !lying && p.s("stands") == "Oblique";
    let hh = edge_height(p, oblique);
    let lean = if oblique { p.f("lean") } else { 0.0 };
    let mut v: Vec<P3>;
    if lying {
        let vmin = base.iter().map(|q| q.1).fold(f64::MAX, f64::min);
        // right triangle: put the right angle on the side away from the depth so the hypotenuse
        // face does not fold onto the back edges (b1 stays the right-angle corner)
        let flip = kind == "Right triangle" && depth_sx(p) > 0.0;
        let fx = |x: f64| if flip { p.f("length") - x } else { x };
        v = base.iter().map(|q| (fx(q.0), 0.0, q.1 - vmin)).collect();
        v.extend(base.iter().map(|q| (fx(q.0), hh, q.1 - vmin)));
    } else {
        v = base.clone();
        v.extend(base.iter().map(|q| (q.0 + lean, q.1, hh)));
    }
    (v, k, lying, lean)
}

fn prism_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("base", "Base", &["Rectangle", "Right triangle", "Isosceles triangle", "Regular polygon"], "Rectangle"),
        ParamDef::choice("stands", "Stands", &["Right", "Oblique"], "Right"),
        ParamDef::float("lean", "Lean of the top when oblique", -20.0, 20.0, 0.5, 2.0),
        ParamDef::float("length", "Length", 0.5, 50.0, 0.5, 5.0),
        ParamDef::float("width", "Width (depth, or triangle height)", 0.5, 50.0, 0.5, 3.0),
        ParamDef::int("n", "Sides of regular-polygon base", 3, 12, 5),
        ParamDef::float("side", "Side of regular-polygon base", 0.5, 50.0, 0.5, 3.0),
        ParamDef::float("height", "Height", 0.5, 50.0, 0.5, 4.0),
    ];
    v.extend(solid_common("cm"));
    v.extend([
        ParamDef::choice("sits", "Sits (triangle or polygon base)", &["Standing on its base", "Lying on its side"], "Standing on its base"),
        ParamDef::choice("depth", "Position: depth goes back to the", &["Right", "Left"], "Right"),
        ParamDef::bool("names", "Name the corners around the base, then around the top", false),
        label_size_param(),
        ParamDef::choice("length_lab", "Length (triangle base) label", &LAB_OPTS, "Measure"),
        ParamDef::text("length_label", "Length label text", "l"),
        ParamDef::choice("width_lab", "Width (triangle height) label", &LAB_OPTS, "Measure"),
        ParamDef::text("width_label", "Width label text", "w"),
        ParamDef::choice("height_lab", "Height label", &LAB_OPTS, "Measure"),
        ParamDef::text("height_label", "Height label text", "h"),
        ParamDef::choice("side_lab", "Polygon side label", &LAB_OPTS, "Measure"),
        ParamDef::text("side_label", "Polygon side label text", "s"),
        ParamDef::choice("hyp_lab", "Hypotenuse / slanted side label (worked out)", &LAB_OPTS, "None"),
        ParamDef::text("hyp_label", "Hypotenuse label text", "x"),
        ParamDef::choice("apothem_lab", "Apothem label (worked out)", &LAB_OPTS, "None"),
        ParamDef::text("apothem_label", "Apothem label text", "a"),
        ParamDef::choice("lean_lab", "Lean label (oblique)", &LAB_OPTS, "Measure"),
        ParamDef::text("lean_label", "Lean label text", "d"),
        ParamDef::text("edge", "Slanted edge (oblique; empty = worked out, typed = the height is worked out)", ""),
        ParamDef::choice("edge_lab", "Slanted edge label (oblique)", &LAB_OPTS, "None"),
        ParamDef::text("edge_label", "Slanted edge label text", "e"),
    ]);
    v
}

fn prism_render(p: &Params) -> String {
    let (l, wd) = (p.f("length"), p.f("width"));
    let kind = p.s("base");
    let sx = depth_sx(p);
    let (v, k, lying, lean) = prism_geom(p);
    let hh = edge_height(p, !lying && p.s("stands") == "Oblique");
    let mut faces = vec![(0..k).collect::<Vec<_>>(), (k..2 * k).collect()];
    for i in 0..k { faces.push(vec![i, (i + 1) % k, k + (i + 1) % k, k + i]); }
    let solid = Solid { v, faces };
    let proj = |q: P3| oblp(q, sx);
    let b: Vec<P3> = solid.v[..k].to_vec();
    let t: Vec<P3> = solid.v[k..].to_vec();
    let mut all: Vec<Pt> = solid.v.iter().map(|q| proj(*q)).collect();
    let cenp = (all.iter().map(|q| q.0).sum::<f64>() / all.len() as f64, all.iter().map(|q| q.1).sum::<f64>() / all.len() as f64);
    // oblique: height dropped from the top of the front edge's leading corner to the extended base line
    let e = if lean >= 0.0 { b[1] } else { b[0] };
    let foot = (e.0 + lean, e.1, 0.0);
    if lean.abs() > 1e-9 { all.push(proj(foot)); }
    let m = Map::fit_at(&all, 420.0, 300.0, 70.0, 40.0);
    let mut d = new_doc(560.0, 380.0);
    draw_solid(&mut d, &m, &solid, &proj, oblv(sx), p.b("hidden"));
    let fs = lsz3(p);
    let hide = hide_list(p);
    let hid = |n: &str| hide.iter().any(|h| h == n);
    let mc = m.p(cenp);
    let lab = |d: &mut Doc, id: &str, a: P3, bb: P3, s: Option<String>| if let Some(s) = s { sbeside_r(d, id, m.p(proj(a)), m.p(proj(bb)), 0.5, mc, &s, fs); };
    // labels inside the base face sit away from the base's own centre
    let mb = m.p(proj(centroid3(&b)));
    let labb = |d: &mut Doc, id: &str, a: P3, bb: P3, t: f64, s: Option<String>| if let Some(s) = s { sbeside_r(d, id, m.p(proj(a)), m.p(proj(bb)), t, mb, &s, fs); };
    let right = p.b("right");
    let poly = kind == "Regular polygon";
    // base edge (front bottom) b0-b1
    if poly { lab(&mut d, "side", b[0], b[1], mtext(p, "side", "Measure", p.f("side"), hid("side"))); }
    else { lab(&mut d, "length", b[0], b[1], mtext(p, "length", "Measure", l, hid("length"))); }
    // the base face's own measures: on the bottom face when standing, the front face when lying
    let wtxt = mtext(p, "width", "Measure", wd, hid("width"));
    match kind {
        "Rectangle" => lab(&mut d, "width", b[1], b[2], wtxt),
        "Right triangle" => {
            labb(&mut d, "width", b[1], b[2], 0.5, wtxt);
            if right { rmark3(&mut d, &m, &proj, b[1], sub3(b[0], b[1]), sub3(b[2], b[1])); }
            labb(&mut d, "hyp", b[0], b[2], 0.5, mtext(p, "hyp", "None", dist3(b[0], b[2]), hid("hypotenuse")));
        }
        "Isosceles triangle" => {
            if let Some(s) = wtxt {
                let f0 = mid3(b[0], b[1]);
                dline(&mut d, m.p(proj(f0)), m.p(proj(b[2])), INK, 1.6, "6 5");
                labb(&mut d, "width", f0, b[2], 0.6, Some(s));
                if right { rmark3(&mut d, &m, &proj, f0, sub3(b[1], f0), sub3(b[2], f0)); }
            }
            labb(&mut d, "hyp", b[1], b[2], 0.5, mtext(p, "hyp", "None", dist3(b[1], b[2]), hid("slanted")));
        }
        _ => {
            if let Some(s) = mtext(p, "apothem", "None", dist3(mid3(b[0], b[1]), centroid3(&b)), hid("apothem")) {
                let (c0, f0) = (centroid3(&b), mid3(b[0], b[1]));
                dline(&mut d, m.p(proj(c0)), m.p(proj(f0)), INK, 1.6, "6 5");
                d.circle(m.p(proj(c0)).0, m.p(proj(c0)).1, 2.4, INK, INK, 1.0);
                labb(&mut d, "apothem", c0, f0, 0.4, Some(s));
                if right { rmark3(&mut d, &m, &proj, f0, sub3(b[1], f0), sub3(c0, f0)); }
            }
        }
    }
    let htxt = mtext(p, "height", "Measure", hh, hid("height"));
    if lying {
        // prism length runs back along the depth from the front corner nearest the depth side
        let vi = (0..k).filter(|i| b[*i].2 < 1e-9).max_by(|x, y| (sx * b[*x].0).partial_cmp(&(sx * b[*y].0)).unwrap()).unwrap_or(1);
        lab(&mut d, "height", b[vi], t[vi], htxt);
    } else if lean.abs() < 1e-9 {
        // vertical edge at the front corner away from the depth side
        let ymin = b.iter().map(|q| q.1).fold(f64::MAX, f64::min);
        let vi = (0..k).filter(|i| b[*i].1 < ymin + 1e-9).min_by(|x, y| (sx * b[*x].0).partial_cmp(&(sx * b[*y].0)).unwrap()).unwrap_or(0);
        lab(&mut d, "height", b[vi], t[vi], htxt);
    } else {
        let top = (foot.0, foot.1, hh);
        dline(&mut d, m.p(proj(top)), m.p(proj(foot)), INK, 1.6, "6 5");
        let outside = (lean > 0.0 && foot.0 > e.0) || (lean < 0.0 && foot.0 < e.0);
        if outside { dline(&mut d, m.p(proj(e)), m.p(proj(foot)), INK, 1.6, "6 5"); }
        if htxt.is_some() || right {
            if right { rmark3(&mut d, &m, &proj, foot, sub3(e, foot), (0.0, 0.0, 1.0)); }
        }
        if let Some(s) = htxt { sbeside_r(&mut d, "height", m.p(proj(foot)), m.p(proj(top)), 0.7, mc, &s, fs); }
        lab(&mut d, "lean", e, foot, mtext(p, "lean", "None", lean.abs(), hid("lean")));
        let et = (e.0 + lean, e.1, hh);
        if let Some(s) = tlabel(p, "edge", "edge", "None", dist3(e, et)) { sbeside_r(&mut d, "edge", m.p(proj(e)), m.p(proj(et)), 0.3, mc, &s, fs); }
    }
    if p.b("names") {
        let fsn = fs * 0.95;
        for (i, q) in solid.v.iter().enumerate() { name_at(&mut d, &format!("v{i}"), m.p(proj(*q)), mc, &corner_name(i), fsn); }
    }
    d.finish()
}
fn centroid3(v: &[P3]) -> P3 {
    let n = v.len().max(1) as f64;
    (v.iter().map(|q| q.0).sum::<f64>() / n, v.iter().map(|q| q.1).sum::<f64>() / n, v.iter().map(|q| q.2).sum::<f64>() / n)
}

/// Pyramid height actually drawn: the typed height, or worked out from the slant height.
pub(super) fn pyramid_height(p: &Params) -> f64 {
    if p.s("stands") != "Oblique" && let Some(s) = typed(p, "slant") {
        let a = pyramid_apothem(p);
        if s > a + 1e-6 { return (s * s - a * a).sqrt(); }
    }
    p.f("height")
}
/// Distance from the base's centre to the middle of the front base edge (where the slant height
/// meets the base): half the width for a rectangle, the apothem for a regular polygon.
pub(super) fn pyramid_apothem(p: &Params) -> f64 {
    let b = poly_base(p.s("base"), p.f("length"), p.f("width"), p.i("n").clamp(3, 12) as usize, p.f("side"));
    dist3(mid3(b[0], b[1]), centroid3(&b))
}

fn pyramid_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("base", "Base", &["Rectangle", "Regular polygon"], "Rectangle"),
        ParamDef::choice("stands", "Stands", &["Right", "Oblique"], "Right"),
        ParamDef::float("lean", "Lean of the apex when oblique", -20.0, 20.0, 0.5, 2.0),
        ParamDef::float("length", "Length", 0.5, 50.0, 0.5, 6.0),
        ParamDef::float("width", "Width", 0.5, 50.0, 0.5, 6.0),
        ParamDef::int("n", "Sides of regular-polygon base", 3, 12, 5),
        ParamDef::float("side", "Side of regular-polygon base", 0.5, 50.0, 0.5, 4.0),
        ParamDef::float("height", "Height", 0.5, 50.0, 0.5, 4.0),
        ParamDef::bool("show_height", "Draw the height", true),
    ];
    v.extend(solid_common("cm"));
    v.extend([
        ParamDef::text("slant", "Slant height (empty = worked out; typed = the height is worked out)", ""),
        ParamDef::choice("depth", "Position: depth goes back to the", &["Right", "Left"], "Right"),
        ParamDef::bool("names", "Name the corners around the base, then the tip", false),
        label_size_param(),
        ParamDef::choice("length_lab", "Length label", &LAB_OPTS, "Measure"),
        ParamDef::text("length_label", "Length label text", "l"),
        ParamDef::choice("width_lab", "Width label", &LAB_OPTS, "Measure"),
        ParamDef::text("width_label", "Width label text", "w"),
        ParamDef::choice("side_lab", "Polygon side label", &LAB_OPTS, "Measure"),
        ParamDef::text("side_label", "Polygon side label text", "s"),
        ParamDef::choice("height_lab", "Height label", &LAB_OPTS, "Measure"),
        ParamDef::text("height_label", "Height label text", "h"),
        ParamDef::choice("slant_lab", "Slant height label (drawn apex to the middle of the front edge)", &LAB_OPTS, "None"),
        ParamDef::text("slant_label", "Slant height label text", "l"),
        ParamDef::choice("apothem_lab", "Apothem label (worked out)", &LAB_OPTS, "None"),
        ParamDef::text("apothem_label", "Apothem label text", "a"),
        ParamDef::choice("lean_lab", "Lean label (oblique)", &LAB_OPTS, "Measure"),
        ParamDef::text("lean_label", "Lean label text", "d"),
    ]);
    v
}

fn pyramid_render(p: &Params) -> String {
    let (l, wd) = (p.f("length"), p.f("width"));
    let hh = pyramid_height(p);
    let kind = p.s("base");
    let sx = depth_sx(p);
    let base = poly_base(kind, l, wd, p.i("n").clamp(3, 12) as usize, p.f("side"));
    let k = base.len();
    let c3 = centroid3(&base);
    let lean = if p.s("stands") == "Oblique" { p.f("lean") } else { 0.0 };
    let apex = (c3.0 + lean, c3.1, hh);
    let mut v = base.clone();
    v.push(apex);
    let mut faces = vec![(0..k).collect::<Vec<_>>()];
    for i in 0..k { faces.push(vec![i, (i + 1) % k, k]); }
    let solid = Solid { v, faces };
    let proj = |q: P3| oblp(q, sx);
    let all: Vec<Pt> = solid.v.iter().map(|q| proj(*q)).collect();
    let cenp = (all.iter().map(|q| q.0).sum::<f64>() / all.len() as f64, all.iter().map(|q| q.1).sum::<f64>() / all.len() as f64);
    let m = Map::fit_at(&all, 420.0, 300.0, 70.0, 40.0);
    let mut d = new_doc(560.0, 380.0);
    draw_solid(&mut d, &m, &solid, &proj, oblv(sx), p.b("hidden"));
    let fs = lsz3(p);
    let hide = hide_list(p);
    let hid = |n: &str| hide.iter().any(|h| h == n);
    let mc = m.p(cenp);
    let lab = |d: &mut Doc, id: &str, a: P3, bb: P3, s: Option<String>| if let Some(s) = s { sbeside_r(d, id, m.p(proj(a)), m.p(proj(bb)), 0.5, mc, &s, fs); };
    let right = p.b("right");
    let foot = (c3.0, c3.1, 0.0);
    let hfoot = (apex.0, apex.1, 0.0);
    if p.b("show_height") {
        dline(&mut d, m.p(proj(apex)), m.p(proj(hfoot)), INK, 1.6, "6 5");
        if lean.abs() > 1e-9 { dline(&mut d, m.p(proj(foot)), m.p(proj(hfoot)), INK, 1.6, "6 5"); }
        // the reference sets the height beside its line on the side facing away from the depth
        if let Some(s) = mtext(p, "height", "Measure", hh, hid("height")) {
            let (pf, pa) = (m.p(proj(hfoot)), m.p(proj(apex)));
            sbeside_r(&mut d, "height", pf, pa, 0.5, (pf.0 + sx * 100.0, pf.1), &s, fs);
        }
        if right { rmark3(&mut d, &m, &proj, hfoot, (-sx, 0.0, 0.0), (0.0, 0.0, 1.0)); }
    }
    let mid = mid3(base[0], base[1]);
    if let Some(s) = tlabel(p, "slant", "slant", "None", dist3(mid, apex)).filter(|_| !hid("slant")) {
        dline(&mut d, m.p(proj(mid)), m.p(proj(apex)), INK, 1.6, "6 5");
        let (pa, pm) = (m.p(proj(apex)), m.p(proj(mid)));
        sbeside_r(&mut d, "slant", pm, pa, 0.3, mc, &s, fs);
        if right && lean.abs() < 1e-9 { rmark3(&mut d, &m, &proj, mid, sub3(base[1], mid), sub3(apex, mid)); }
    }
    if lean.abs() > 1e-9 && let Some(s) = mtext(p, "lean", "Measure", lean.abs(), false) {
        // lean: from the base centre to the foot of the height, along the base
        lab(&mut d, "lean", foot, hfoot, Some(s));
    }
    if let Some(s) = mtext(p, "apothem", "None", dist3(mid, foot), hid("apothem")) {
        dline(&mut d, m.p(proj(foot)), m.p(proj(mid)), INK, 1.6, "6 5");
        lab(&mut d, "apothem", foot, mid, Some(s));
        if right { rmark3(&mut d, &m, &proj, mid, sub3(base[1], mid), sub3(foot, mid)); }
    }
    if kind == "Regular polygon" {
        lab(&mut d, "side", base[0], base[1], mtext(p, "side", "Measure", p.f("side"), hid("side")));
    } else {
        lab(&mut d, "length", base[0], base[1], mtext(p, "length", "Measure", l, hid("length")));
        let (w0, w1) = if sx > 0.0 { (base[1], base[2]) } else { (base[0], base[3]) };
        lab(&mut d, "width", w0, w1, mtext(p, "width", "Measure", wd, hid("width")));
    }
    if p.b("names") {
        for (i, q) in solid.v.iter().enumerate() { name_at(&mut d, &format!("v{i}"), m.p(proj(*q)), mc, &corner_name(i), fs * 0.95); }
    }
    d.finish()
}

// ----- round solids (elevated orthographic view) -----

pub(super) fn ell(cx: f64, cy: f64, rx: f64, ry: f64, t0: f64, t1: f64) -> Vec<Pt> {
    let n = 64;
    (0..=n).map(|i| { let t = (t0 + (t1 - t0) * i as f64 / n as f64).to_radians(); (cx + rx * t.cos(), cy + ry * t.sin()) }).collect()
}
fn mpts(m: &Map, pts: &[Pt]) -> Vec<Pt> { pts.iter().map(|q| m.p(*q)).collect() }

fn round_common() -> Vec<ParamDef> {
    vec![
        ParamDef::float("view", "View elevation (degrees above the horizon)", 8.0, 60.0, 0.1, 20.7),
        ParamDef::bool("hidden", "Show hidden edges (dashed)", true),
        ParamDef::bool("right", "Right-angle squares", true),
        ParamDef::text("unit", "Unit", "cm"),
        ParamDef::int("dec", "Decimal places", 0, 3, 1),
        // the reference's "Give the diameter instead": the Radius row becomes a Diameter row and the
        // diameter is drawn right across the circle with the radius row's label
        ParamDef::bool("give_d", "Give the diameter instead (drawn right across the circle)", false),
        ParamDef::float("diam", "Diameter", 1.0, 100.0, 0.5, 6.0),
    ]
}
/// Radius of a round solid: the Radius row, or half the Diameter row with "Give the diameter instead".
pub(super) fn solid_r(p: &Params) -> f64 { if p.b("give_d") { p.f("diam") / 2.0 } else { p.f("radius") } }
/// Writes a dragged radius into whichever row is given.
pub(super) fn set_solid_r(p: &mut Params, r: f64) { if p.b("give_d") { p.set_f("diam", 2.0 * r) } else { p.set_f("radius", r) } }
/// The diameter's label: with "Give the diameter instead" it takes the radius row's label mode
/// (Measure by default) and the diameter text; otherwise its own linked row's label.
fn diam_text(p: &Params, v: f64) -> Option<String> {
    if !p.b("give_d") { return mtext(p, "d", "None", v, false); }
    match match p.s("r_lab") { "" => "Measure", m => m } {
        "None" => None,
        "Text" => { let t = p.s("d_label").trim(); Some(if t.is_empty() { "d".to_string() } else { t.to_string() }) }
        _ => Some(fmtu(v, p.i("dec"), p.s("unit"))),
    }
}
fn round_label_params() -> [ParamDef; 5] {
    let [d0, d1] = diameter_params();
    [
        label_size_param(),
        ParamDef::choice("r_lab", "Radius label", &LAB_OPTS, "Measure"),
        ParamDef::text("r_label", "Radius label text", "r"),
        d0,
        d1,
    ]
}
/// The diameter line is drawn (its label is shown) instead of / as well as the radius.
fn diam_shown(p: &Params) -> bool { diam_text(p, 1.0).is_some() }

/// Radius (or diameter) across the circle centred at model point o, drawn as the reference does:
/// 1.6 px line (solid on a top face or a sphere, dashed on a base), a dot at the centre for a
/// radius, the label 20 units clear above or below it. `dir` is the side the radius line runs
/// to (+1 right, -1 left).
fn base_measure(d: &mut Doc, m: &Map, o: Pt, r: f64, p: &Params, solid: bool, above: bool, dir: f64) {
    let fs = lsz3(p);
    let c = m.p(o);
    let dash = if solid { "none" } else { "6 5" };
    let rt = if p.b("give_d") { None } else { mtext(p, "r", "Measure", r, false) };
    let dt = diam_text(p, 2.0 * r);
    // nothing is drawn across the circle when neither label prints (as the reference)
    if rt.is_none() && dt.is_none() { return; }
    let diam = dt.is_some();
    let rb = m.p((o.0 + dir * r, o.1));
    let (a, b) = if diam { (m.p((o.0 - r, o.1)), m.p((o.0 + r, o.1))) } else { (c, rb) };
    dline(d, a, b, INK, 1.6, dash);
    let away = if above { 100.0 } else { -100.0 };
    let mid = ((a.0 + b.0) / 2.0, c.1);
    if let Some(s) = dt { sbeside_r(d, "d", a, b, 0.5, (mid.0, mid.1 + away), &s, fs); }
    if let Some(s) = rt {
        // with the diameter also labelled, the radius sits on the other side of the line
        let side = if diam { -away } else { away };
        sbeside_r(d, "r", c, rb, 0.5, (mid.0, mid.1 + side), &s, fs);
        d.circle(c.0, c.1, 3.0, INK, INK, 1.0);
    }
}

fn cylinder_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("stands", "Stands", &["Right", "Oblique"], "Right"),
        ParamDef::float("lean", "Lean of the top when oblique", -20.0, 20.0, 0.5, 2.0),
        ParamDef::float("radius", "Radius", 0.5, 50.0, 0.5, 3.0),
        ParamDef::float("height", "Height", 0.5, 50.0, 0.5, 4.0),
        ParamDef::bool("show_height", "Draw the height", false),
    ];
    v.extend(round_common());
    v.extend(round_label_params());
    v.extend([
        ParamDef::choice("h_lab", "Height label", &LAB_OPTS, "Measure"),
        ParamDef::text("h_label", "Height label text", "h"),
        ParamDef::choice("lean_lab", "Lean label (oblique)", &LAB_OPTS, "Measure"),
        ParamDef::text("lean_label", "Lean label text", "d"),
        ParamDef::text("edge", "Slanted edge (oblique; empty = worked out, typed = the height is worked out)", ""),
        ParamDef::choice("edge_lab", "Slanted edge label (oblique)", &LAB_OPTS, "None"),
        ParamDef::text("edge_label", "Slanted edge label text", "e"),
    ]);
    v
}

/// Tangent points of a round solid's silhouette: the base ellipse's parameter (radians) whose
/// tangent runs along the base-to-top translation `(lean, h)`; the other tangent is `+ PI`.
fn tangent_t(s: f64, lean: f64, h: f64) -> f64 { (-s * lean).atan2(h) }

fn cylinder_render(p: &Params) -> String {
    let oblique = p.s("stands") == "Oblique";
    let (r, h) = (solid_r(p), edge_height(p, oblique));
    let s = p.f("view").to_radians().sin();
    let lean = if oblique { p.f("lean") } else { 0.0 };
    let mut all = ell(0.0, 0.0, r, r * s, 0.0, 360.0);
    all.extend(ell(lean, h, r, r * s, 0.0, 360.0));
    // the reference draws a solid at a fixed 420 x 300 limit, no fill, 2.4 outline
    let m = Map::fit_at(&all, 420.0, 300.0, 70.0, 40.0);
    let mut d = new_doc(560.0, 380.0);
    let fs = lsz3(p);
    let t0 = tangent_t(s, lean, h);
    let t0d = t0.to_degrees();
    let tp = |t: f64, cx: f64, cy: f64| (cx + r * t.cos(), cy + r * s * t.sin());
    let (lb, rb) = (tp(t0 + std::f64::consts::PI, 0.0, 0.0), tp(t0, 0.0, 0.0));
    let (lt, rt) = (tp(t0 + std::f64::consts::PI, lean, h), tp(t0, lean, h));
    let (l0, l1, r0, r1) = (m.p(lb), m.p(lt), m.p(rb), m.p(rt));
    // tinted faces (White shading, the opening state, turns them white); the outline is unfilled
    let mut body = ell(0.0, 0.0, r, r * s, t0d + 180.0, t0d + 360.0);
    body.extend(ell(lean, h, r, r * s, t0d + 360.0, t0d + 180.0));
    d.poly(&mpts(&m, &body), "#d9e7f6", "none", 0.0);
    if p.b("hidden") { dpoly(&mut d, &mpts(&m, &ell(0.0, 0.0, r, r * s, t0d, t0d + 180.0)), INK, 1.6, "6 5"); }
    // the radius runs across the top face, away from the lean
    let dir: f64 = if lean > 0.0 { -1.0 } else { 1.0 };
    let tc = m.p((lean, h));
    let foot = m.p((lean, 0.0));
    let base_c = m.p((0.0, 0.0));
    let center = m.p((lean / 2.0, h / 2.0));
    let draw_h = oblique || p.b("show_height");
    let htxt = mtext(p, "h", "Measure", h, false);
    if draw_h {
        dline(&mut d, tc, foot, INK, 1.6, "6 5");
        if lean.abs() > 1e-9 { dline(&mut d, base_c, foot, INK, 1.6, "6 5"); }
        if p.b("right") && !diam_shown(p) {
            let toward = if lean.abs() > 1e-9 { base_c } else { (foot.0 + 10.0, foot.1) };
            square_mark(&mut d, foot, (foot.0, foot.1 - 10.0), toward, 12.0);
        }
        if let Some(txt) = &htxt { sbeside_r(&mut d, "h", foot, tc, 0.5, (tc.0 - dir * 100.0, tc.1), txt, fs); }
    }
    d.poly(&mpts(&m, &ell(lean, h, r, r * s, 0.0, 360.0)), "#f4f9ff", INK, 2.4);
    d.line(l0.0, l0.1, l1.0, l1.1, INK, 2.4);
    d.line(r0.0, r0.1, r1.0, r1.1, INK, 2.4);
    d.polyline(&mpts(&m, &ell(0.0, 0.0, r, r * s, t0d + 180.0, t0d + 360.0)), INK, 2.4);
    base_measure(&mut d, &m, (lean, h), r, p, true, true, dir);
    if !draw_h && let Some(txt) = &htxt { sbeside_r(&mut d, "h", r0, r1, 0.5, center, txt, fs); }
    if lean.abs() > 1e-9 {
        if let Some(txt) = mtext(p, "lean", "None", lean.abs(), false) {
            sbeside_r(&mut d, "lean", base_c, foot, 0.5, ((base_c.0 + foot.0) / 2.0, base_c.1 - 100.0), &txt, fs);
        }
        let side = if lean > 0.0 { (r0, r1) } else { (l0, l1) };
        if let Some(txt) = tlabel(p, "edge", "edge", "None", (h * h + lean * lean).sqrt()) { sbeside_r(&mut d, "edge", side.0, side.1, 0.5, center, &txt, fs); }
    }
    d.finish()
}

/// Cone height actually drawn: typed, or worked out from a given slant height (right cones).
pub(super) fn cone_height(p: &Params) -> f64 {
    if p.s("stands") != "Oblique" && let Some(s) = typed(p, "slant") {
        let r = solid_r(p);
        if s > r + 1e-6 { return (s * s - r * r).sqrt(); }
    }
    p.f("height")
}

fn cone_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("stands", "Stands", &["Right", "Oblique"], "Right"),
        ParamDef::float("lean", "Lean of the apex when oblique", -20.0, 20.0, 0.5, 2.0),
        ParamDef::float("radius", "Radius", 0.5, 50.0, 0.5, 3.0),
        ParamDef::float("height", "Height", 0.5, 50.0, 0.5, 4.0),
        ParamDef::bool("show_height", "Draw the height", true),
    ];
    v.extend(round_common());
    v.extend(round_label_params());
    v.extend([
        ParamDef::text("slant", "Slant height (empty = worked out; typed = the height is worked out)", ""),
        ParamDef::choice("h_lab", "Height label", &LAB_OPTS, "Measure"),
        ParamDef::text("h_label", "Height label text", "h"),
        ParamDef::choice("l_lab", "Slant height label", &LAB_OPTS, "None"),
        ParamDef::text("l_label", "Slant height label text", "l"),
        ParamDef::choice("lean_lab", "Lean label (oblique)", &LAB_OPTS, "Measure"),
        ParamDef::text("lean_label", "Lean label text", "d"),
    ]);
    v
}

fn cone_render(p: &Params) -> String {
    let r = solid_r(p);
    let h = cone_height(p);
    let s = p.f("view").to_radians().sin();
    let lean = if p.s("stands") == "Oblique" { p.f("lean") } else { 0.0 };
    let apex = (lean, h);
    // tangents from apex to base ellipse
    let pp = (apex.0, apex.1 / s);
    let dd = (pp.0 * pp.0 + pp.1 * pp.1).sqrt().max(r * 1.001);
    let (phi, del) = (pp.1.atan2(pp.0), (r / dd).acos());
    let tp = |a: f64| (r * a.cos(), r * s * a.sin());
    let (t1, t2) = (phi + del, phi - del);
    let (mut tl, mut tr) = (t1, t2);
    if tp(tl).0 > tp(tr).0 { std::mem::swap(&mut tl, &mut tr); }
    let (pl, pr) = (tp(tl), tp(tr));
    let mut all = ell(0.0, 0.0, r, r * s, 0.0, 360.0);
    all.push(apex);
    let m = Map::fit_at(&all, 420.0, 300.0, 70.0, 40.0);
    let mut d = new_doc(560.0, 380.0);
    let fs = lsz3(p);
    let (a0, a1) = (tl.to_degrees().rem_euclid(360.0), tr.to_degrees().rem_euclid(360.0));
    let (front0, front1) = (a0, if a1 < a0 { a1 + 360.0 } else { a1 });
    let front = ell(0.0, 0.0, r, r * s, front0, front1);
    // tinted face (White shading, the opening state, turns it white); the outline is unfilled
    let mut body = vec![apex];
    body.extend(front.iter().copied());
    d.poly(&mpts(&m, &body), "#d9e7f6", "none", 0.0);
    if p.b("hidden") {
        let back = ell(0.0, 0.0, r, r * s, front1, front0 + 360.0);
        dpoly(&mut d, &mpts(&m, &back), INK, 1.6, "6 5");
    }
    let (ap, bc, foot) = (m.p(apex), m.p((0.0, 0.0)), m.p((lean, 0.0)));
    // the radius runs along the base, away from the lean
    let dir: f64 = if lean > 0.0 { -1.0 } else { 1.0 };
    if p.b("show_height") {
        dline(&mut d, ap, foot, INK, 1.6, "6 5");
        if lean.abs() > 1e-9 { dline(&mut d, bc, foot, INK, 1.6, "6 5"); }
        if p.b("right") && !diam_shown(p) {
            let toward = if lean.abs() > 1e-9 { bc } else { (foot.0 + 10.0, foot.1) };
            square_mark(&mut d, foot, (foot.0, foot.1 - 10.0), toward, 12.0);
        }
        if let Some(txt) = mtext(p, "h", "Measure", h, false) { sbeside_r(&mut d, "h", foot, ap, 0.5, (ap.0 - dir * 100.0, ap.1), &txt, fs); }
        if lean.abs() > 1e-9 && let Some(txt) = mtext(p, "lean", "None", lean.abs(), false) {
            sbeside_r(&mut d, "lean", bc, foot, 0.5, ((bc.0 + foot.0) / 2.0, bc.1 - 100.0), &txt, fs);
        }
    }
    d.line(ap.0, ap.1, m.p(pl).0, m.p(pl).1, INK, 2.4);
    d.line(ap.0, ap.1, m.p(pr).0, m.p(pr).1, INK, 2.4);
    d.polyline(&mpts(&m, &front), INK, 2.4);
    base_measure(&mut d, &m, (0.0, 0.0), r, p, false, false, dir);
    let sl = ((r - lean).powi(2) + h * h).sqrt();
    // an oblique cone has no slant height to give (its slant edge is always worked out)
    let ltxt = if lean.abs() > 1e-9 { mtext(p, "l", "None", sl, false) } else { tlabel(p, "l", "slant", "None", sl) };
    if let Some(txt) = ltxt {
        let (a, b) = (ap, m.p(pr));
        sbeside_r(&mut d, "l", a, b, 0.5, m.p((lean * 0.5, h * 0.4)), &txt, fs);
    }
    d.finish()
}
fn sphere_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("draw", "Draw", &["A whole sphere", "A hemisphere"], "A whole sphere"),
        ParamDef::float("radius", "Radius", 0.5, 50.0, 0.5, 3.0),
    ];
    v.extend(round_common());
    v.extend(round_label_params());
    v.push(ParamDef::choice("face", "Position (hemisphere): flat face", &["Down, like a dome", "Up, like a bowl"], "Down, like a dome"));
    v
}

fn sphere_render(p: &Params) -> String {
    let r = solid_r(p);
    let s = p.f("view").to_radians().sin();
    let half = p.s("draw") == "A hemisphere";
    let bowl = half && p.s("face") == "Up, like a bowl";
    let all = if bowl {
        let mut a = ell(0.0, 0.0, r, r * s, 0.0, 360.0);
        a.extend(ell(0.0, 0.0, r, r, 180.0, 360.0));
        a
    } else if half {
        let mut a = ell(0.0, 0.0, r, r, 0.0, 180.0);
        a.extend(ell(0.0, 0.0, r, r * s, 180.0, 360.0));
        a
    } else { ell(0.0, 0.0, r, r, 0.0, 360.0) };
    let m = Map::fit_at(&all, 420.0, 300.0, 70.0, 40.0);
    let mut d = new_doc(560.0, 380.0);
    if bowl {
        let mut sil = ell(0.0, 0.0, r, r, 180.0, 360.0);
        sil.extend(ell(0.0, 0.0, r, r * s, 0.0, 180.0));
        d.poly(&mpts(&m, &sil), "#e3eef9", INK, 2.4);
        d.poly(&mpts(&m, &ell(0.0, 0.0, r, r * s, 0.0, 360.0)), "#f4f9ff", INK, 2.4);
    } else if half {
        let mut sil = ell(0.0, 0.0, r, r, 0.0, 180.0);
        sil.extend(ell(0.0, 0.0, r, r * s, 180.0, 360.0));
        d.poly(&mpts(&m, &sil), "#e3eef9", INK, 2.4);
        if p.b("hidden") { dpoly(&mut d, &mpts(&m, &ell(0.0, 0.0, r, r * s, 0.0, 180.0)), INK, 1.6, "6 5"); }
    } else {
        d.poly(&mpts(&m, &ell(0.0, 0.0, r, r, 0.0, 360.0)), "#e3eef9", INK, 2.4);
        d.polyline(&mpts(&m, &ell(0.0, 0.0, r, r * s, 180.0, 360.0)), INK, 2.4);
        if p.b("hidden") { dpoly(&mut d, &mpts(&m, &ell(0.0, 0.0, r, r * s, 0.0, 180.0)), INK, 1.6, "6 5"); }
    }
    base_measure(&mut d, &m, (0.0, 0.0), r, p, true, true, 1.0);
    d.finish()
}

/// "Start from an example" entries for math-b's figures (solids and plots).
fn examples_b(id: &str) -> Option<Vec<super::Example>> {
    use Value::{B, F, I, S};
    let s = |v: &str| S(v.to_string());
    let ex = |name: &'static str, params: Vec<(&'static str, Value)>| super::Example { name, params };
    let v = match id {
        "sphere" => vec![
            ex("Sphere with radius 6 cm", vec![("radius", F(6.0))]),
            ex("Sphere with diameter 14 inches", vec![("radius", F(7.0)), ("r_lab", s("None")), ("d_lab", s("Measure")), ("unit", s("in"))]),
            ex("Hemisphere with radius 5 m", vec![("draw", s("A hemisphere")), ("radius", F(5.0)), ("unit", s("m"))]),
            ex("Hemisphere bowl with radius 8 cm", vec![("draw", s("A hemisphere")), ("face", s("Up, like a bowl")), ("radius", F(8.0))]),
            ex("Sphere with radius r", vec![("unit", s("")), ("r_lab", s("Text")), ("r_label", s("r"))]),
        ],
        "cylinder" => vec![
            ex("Cylinder with radius 4 cm and height 10 cm", vec![("radius", F(4.0)), ("height", F(10.0))]),
            ex("Cylinder with diameter 12 inches and height 9 inches", vec![("radius", F(6.0)), ("height", F(9.0)), ("r_lab", s("None")), ("d_lab", s("Measure")), ("unit", s("in"))]),
            ex("Oblique cylinder with its height and lean", vec![("stands", s("Oblique")), ("lean", F(4.0)), ("radius", F(3.0)), ("height", F(8.0))]),
            ex("Cylinder with radius r and height h", vec![("unit", s("")), ("r_lab", s("Text")), ("r_label", s("r")), ("h_lab", s("Text")), ("h_label", s("h"))]),
        ],
        "cone" => vec![
            ex("Cone with radius 4 cm and height 9 cm", vec![("radius", F(4.0)), ("height", F(9.0))]),
            ex("Cone with the slant height labeled x", vec![("radius", F(6.0)), ("height", F(8.0)), ("l_lab", s("Text")), ("l_label", s("x"))]),
            ex("Cone with diameter 10 cm and slant height 13 cm", vec![("radius", F(5.0)), ("r_lab", s("None")), ("d_lab", s("Measure")), ("slant", s("13")), ("l_lab", s("Measure")), ("h_lab", s("None"))]),
            ex("Cone with radius r and height h", vec![("unit", s("")), ("r_lab", s("Text")), ("r_label", s("r")), ("h_lab", s("Text")), ("h_label", s("h"))]),
            ex("Oblique cone with its height and lean", vec![("stands", s("Oblique")), ("lean", F(2.5))]),
        ],
        "prism" => vec![
            ex("Rectangular prism 8 cm by 3 cm by 5 cm", vec![("length", F(8.0)), ("width", F(3.0)), ("height", F(5.0))]),
            ex("Cube with 6-inch edges", vec![("length", F(6.0)), ("width", F(6.0)), ("height", F(6.0)), ("unit", s("in"))]),
            ex("Triangular prism with a right triangle base", vec![("base", s("Right triangle")), ("sits", s("Lying on its side")), ("length", F(6.0)), ("width", F(8.0)), ("height", F(12.0)), ("hyp_lab", s("Measure"))]),
            ex("Hexagonal prism with its apothem", vec![("base", s("Regular polygon")), ("n", I(6)), ("side", F(4.0)), ("height", F(10.0)), ("apothem_lab", s("Measure"))]),
            ex("Rectangular prism with a missing height x", vec![("length", F(12.0)), ("width", F(4.0)), ("height", F(5.0)), ("height_lab", s("Text")), ("height_label", s("x")), ("unit", s("ft"))]),
            ex("Oblique prism with its lean", vec![("stands", s("Oblique")), ("lean", F(2.0))]),
        ],
        "pyramid" => vec![
            ex("Square pyramid with slant height 10 cm", vec![("length", F(12.0)), ("width", F(12.0)), ("slant", s("10")), ("slant_lab", s("Measure")), ("height_lab", s("None")), ("show_height", B(false))]),
            ex("Rectangular pyramid 8 cm by 6 cm with height 9 cm", vec![("length", F(8.0)), ("width", F(6.0)), ("height", F(9.0))]),
            ex("Hexagonal pyramid with its apothem and height", vec![("base", s("Regular polygon")), ("n", I(6)), ("side", F(4.0)), ("height", F(9.0)), ("apothem_lab", s("Measure")), ("depth", s("Left")), ("hidden", B(false))]),
            ex("Pyramid with named corners", vec![("names", B(true))]),
        ],
        _ => return examples_b_plots(id),
    };
    Some(v)
}

#[cfg(test)]
mod examples_b_tests {
    use super::*;
    /// Every math-b example names only real params, keeps choice values valid and renders.
    #[test]
    fn math_b_examples_render() {
        for id in ["sphere", "cylinder", "cone", "prism", "pyramid", "coordinate-grid", "number-line", "box-plot", "mapping-diagram", "ruler-reading"] {
            let def = defs().into_iter().find(|d| d.id == id).unwrap();
            let pdefs = (def.params)();
            let exs = examples_b(id).unwrap_or_else(|| panic!("{id}: no examples"));
            assert!(exs.len() >= 3, "{id}: only {} examples", exs.len());
            for e in exs {
                let mut p = Params::defaults(&pdefs);
                for (k, v) in &e.params {
                    let pd = pdefs.iter().find(|d| d.key == *k).unwrap_or_else(|| panic!("{id}/{}: unknown key {k}", e.name));
                    if let (Kind::Choice(opts), Value::S(s)) = (&pd.kind, v) { assert!(opts.contains(&s.as_str()), "{id}/{}: bad choice {k}={s}", e.name); }
                    p.0.insert(k.to_string(), v.clone());
                }
                let svg = (def.render)(&p);
                super::super::to_png(&svg, 0.3).unwrap_or_else(|er| panic!("{id}/{}: {er}", e.name));
            }
        }
    }
}

#[cfg(test)]
mod solids_measures_tests {
    use super::*;
    use serde_json::json;

    fn def(id: &str) -> FigureDef { defs().into_iter().find(|d| d.id == id).unwrap() }
    fn with(id: &str, set: &[(&str, Value)]) -> Params {
        let mut p = Params::defaults(&(def(id).params)());
        for (k, v) in set { p.0.insert(k.to_string(), v.clone()); }
        p
    }
    fn comp(id: &str, p: &Params, key: &str) -> String {
        computed(id, p).into_iter().find(|(k, _)| *k == key).map(|(_, v)| v).unwrap_or_else(|| panic!("{id}: no computed {key}"))
    }
    /// An old saved spec loaded through the real load path (defaults + spec + migrate).
    fn load(id: &str, old: serde_json::Value) -> Params {
        let spec = crate::model::FigureSpec { id: id.into(), params: old.as_object().unwrap().clone().into_iter().collect() };
        crate::figures::spec::params_from_spec(&def(id), &spec)
    }
    fn svg_has(id: &str, p: &Params, text: &str) -> bool { (def(id).render)(p).contains(text) }
    fn s(v: &str) -> Value { Value::S(v.into()) }

    #[test]
    fn solvers_match_the_reference() {
        assert_eq!(comp("sphere", &with("sphere", &[]), "diameter"), "6");
        // cylinder: slanted edge worked out (4.5), or given -> height worked out (sqrt 21 = 4.6)
        let p = with("cylinder", &[("stands", s("Oblique")), ("lean", Value::F(2.0))]);
        assert_eq!(comp("cylinder", &p, "edge"), "4.5");
        let p = with("cylinder", &[("stands", s("Oblique")), ("lean", Value::F(2.0)), ("edge", s("5"))]);
        assert_eq!(comp("cylinder", &p, "height"), "4.6");
        assert!((edge_height(&p, true) - 21f64.sqrt()).abs() < 1e-9);
        assert!(svg_has("cylinder", &with("cylinder", &[("stands", s("Oblique")), ("lean", Value::F(2.0)), ("edge", s("5")), ("edge_lab", s("Measure"))]), "5 cm"));
        // cone: slant 5 worked out; slant 6 given -> height 5.2; diameter 10 + slant 13 -> height 12
        assert_eq!(comp("cone", &with("cone", &[]), "slant"), "5");
        assert_eq!(comp("cone", &with("cone", &[("slant", s("6"))]), "height"), "5.2");
        let p = with("cone", &[("radius", Value::F(5.0)), ("slant", s("13"))]);
        assert!((cone_height(&p) - 12.0).abs() < 1e-9);
        // a slant height no longer than the radius cannot be used: the typed height stays
        assert_eq!(cone_height(&with("cone", &[("slant", s("2"))])), 4.0);
        // pyramid: 6x6 h4 -> slant 5; width 8 + slant 5 -> height 3; hexagon side 4 h4 -> 5.3, apothem 3.5
        assert_eq!(comp("pyramid", &with("pyramid", &[]), "slant"), "5");
        assert!((pyramid_height(&with("pyramid", &[("width", Value::F(8.0)), ("slant", s("5"))])) - 3.0).abs() < 1e-9);
        let hex = with("pyramid", &[("base", s("Regular polygon")), ("n", Value::I(6))]);
        assert_eq!(comp("pyramid", &hex, "slant"), "5.3");
        assert_eq!(comp("pyramid", &hex, "apothem"), "3.5");
        // expressions are accepted as typed values
        assert!((pyramid_height(&with("pyramid", &[("slant", s("sqrt(25)"))])) - 4.0).abs() < 1e-9);
        // prism: right triangle 6 x 4 -> hypotenuse 7.2, isosceles -> slanted side 5, hexagon apothem 3.5
        assert_eq!(comp("prism", &with("prism", &[("base", s("Right triangle")), ("length", Value::F(6.0)), ("width", Value::F(4.0))]), "hyp"), "7.2");
        assert_eq!(comp("prism", &with("prism", &[("base", s("Isosceles triangle")), ("length", Value::F(6.0)), ("width", Value::F(4.0))]), "hyp"), "5");
        assert_eq!(comp("prism", &with("prism", &[("base", s("Regular polygon")), ("n", Value::I(6)), ("side", Value::F(4.0))]), "apothem"), "3.5");
        assert_eq!(comp("prism", &with("prism", &[("stands", s("Oblique")), ("lean", Value::F(2.0))]), "edge"), "4.5");
    }

    #[test]
    fn cards_name_real_params() {
        for id in ["cylinder", "cone", "sphere", "pyramid", "prism"] {
            let card = measures(id).unwrap_or_else(|| panic!("{id}: no card"));
            let pdefs = (def(id).params)();
            let has = |k: &str| pdefs.iter().any(|d| d.key == k);
            let comp: Vec<&str> = computed(id, &Params::defaults(&pdefs)).into_iter().map(|(k, _)| k).collect();
            for r in card.groups.iter().flat_map(|g| g.rows.iter()) {
                assert!(has(r.label_key) && has(r.text_key), "{id}/{}: label keys", r.name);
                if r.equals.is_some() { assert!(comp.contains(&r.value_key), "{id}/{}: linked row not computed", r.name); } else { assert!(has(r.value_key), "{id}/{}: value key", r.name); }
            }
        }
    }

    #[test]
    fn new_specs_round_trip_unchanged() {
        for id in ["cylinder", "cone", "sphere", "pyramid", "prism"] {
            let p = with(id, &[("stands", s("Oblique")), ("slant", s("7")), ("edge", s("6")), ("d_lab", s("Text"))]);
            let p2 = crate::figures::spec::params_from_spec(&def(id), &crate::figures::spec::spec_for(id, &p));
            for (k, v) in &p.0 { if (def(id).params)().iter().any(|d| d.key == k) { assert_eq!(p2.0.get(k), Some(v), "{id}: {k}"); } }
        }
    }

    #[test]
    fn old_specs_migrate() {
        // cone: diameter + slant height given instead of the height
        let p = load("cone", json!({"radius": 5.0, "height": 4.0, "measure": "Diameter", "given": "Slant height", "slant": 13.0, "show_slant": true, "r_lab": "Measure", "l_lab": "Measure", "show_height": false, "stands": "Right", "lean_lab": "None"}));
        assert_eq!((p.s("slant"), p.s("d_lab"), p.s("r_lab"), p.s("l_lab")), ("13", "Measure", "None", "Measure"));
        assert!((cone_height(&p) - 12.0).abs() < 1e-9);
        assert!(svg_has("cone", &p, "10 cm") && svg_has("cone", &p, "13 cm"));
        // cone: height given, slant label switched off
        let p = load("cone", json!({"given": "Height", "slant": 5.0, "show_slant": false, "l_lab": "Measure", "stands": "Right"}));
        assert_eq!((p.s("slant"), p.s("l_lab")), ("", "None"));
        // cylinder: diameter 12 in
        let p = load("cylinder", json!({"radius": 6.0, "measure": "Diameter", "r_lab": "Measure", "unit": "in", "stands": "Right", "lean_lab": "None"}));
        assert_eq!((p.s("d_lab"), p.s("r_lab")), ("Measure", "None"));
        assert!(svg_has("cylinder", &p, "12 in") && !svg_has("cylinder", &p, ">6 in"));
        // sphere: diameter with a text label
        let p = load("sphere", json!({"measure": "Diameter", "r_lab": "Text", "r_label": "d"}));
        assert_eq!((p.s("d_lab"), p.s("d_label"), p.s("r_lab")), ("Text", "d", "None"));
        // pyramid: slant height given, hide list
        let p = load("pyramid", json!({"length": 12.0, "width": 12.0, "given": "Slant height", "slant": 10.0, "show_slant": true, "slant_lab": "Measure", "height_lab": "None", "hide": "width", "stands": "Right"}));
        assert_eq!((p.s("slant"), p.s("slant_lab"), p.s("width_lab"), p.s("lean_lab")), ("10", "Measure", "None", "None"));
        assert!((pyramid_height(&p) - 8.0).abs() < 1e-9);
        assert!(svg_has("pyramid", &p, "10 cm"));
        // pyramid made before the slant height could be given: no slant line
        let p = load("pyramid", json!({"show_slant": false, "slant_lab": "Measure", "stands": "Right", "lean_lab": "Measure"}));
        assert_eq!((p.s("slant"), p.s("slant_lab"), p.s("lean_lab")), ("", "None", "Measure"));
        // prism: hide list + oblique without a lean label (pre-label spec)
        let p = load("prism", json!({"hide": "height, hypotenuse", "stands": "Oblique", "lean": 2.0}));
        assert_eq!((p.s("height_lab"), p.s("hyp_lab"), p.s("lean_lab")), ("None", "None", "None"));
        assert!(!svg_has("prism", &p, ">4 cm<"));
    }
}

/// Plot figures' examples (mirroring the reference's "Start from an example" lists).
fn examples_b_plots(id: &str) -> Option<Vec<super::Example>> {
    use Value::{B, F, I, S};
    let s = |v: &str| S(v.to_string());
    let ex = |name: &'static str, params: Vec<(&'static str, Value)>| super::Example { name, params };
    let q4 = |eq: &str| vec![("xmin", F(-10.0)), ("xmax", F(10.0)), ("ymin", F(-10.0)), ("ymax", F(10.0)), ("equations", S(eq.to_string()))];
    let with = |mut v: Vec<(&'static str, Value)>, more: Vec<(&'static str, Value)>| { v.extend(more); v };
    Some(match id {
        "coordinate-grid" => vec![
            ex("Graph of y = 2x + 1 on a coordinate plane", q4("y = 2x + 1")),
            ex("Plotting points in all four quadrants", with(q4(""), vec![("points", s("A(3, 4), B(-5, 2), C(-4, -6), D(6, -3)"))])),
            ex("First-quadrant graph with axis titles: distance over time", vec![("xmin", F(0.0)), ("xmax", F(10.0)), ("ymin", F(0.0)), ("ymax", F(50.0)), ("ystep", F(5.0)), ("square", B(false)), ("equations", s("y = 5x")), ("xtitle_mode", s("Text")), ("xtitle", s("Time (s)")), ("ytitle_mode", s("Text")), ("ytitle", s("Distance (m)")), ("xlabel", s("")), ("ylabel", s(""))]),
            ex("Graph of the parabola y = x\u{b2} \u{2212} 2x \u{2212} 3", q4("y = x^2 - 2x - 3")),
            ex("Piecewise function graph with open and closed circles", q4("y = x + 2, x < 1; y = -2x + 4, x >= 1")),
            ex("Graph of y = sin x from \u{2212}2\u{3c0} to 2\u{3c0}", vec![("xunits", s("Multiples of \u{3c0}")), ("xmin", F(-2.0)), ("xmax", F(2.0)), ("xstep", F(0.5)), ("ymin", F(-2.0)), ("ymax", F(2.0)), ("square", B(false)), ("xnum", s("Every 2nd line")), ("equations", s("y = sin(x)"))]),
            ex("Graph of y = 3 sin 2x in degrees, from 0\u{b0} to 360\u{b0}", vec![("xunits", s("Degrees")), ("xmin", F(0.0)), ("xmax", F(360.0)), ("xstep", F(90.0)), ("ymin", F(-4.0)), ("ymax", F(4.0)), ("equations", s("y = 3sin(2x)"))]),
            ex("Graphs of y = 2\u{2e3} and y = log\u{2082} x, reflected in y = x", with(q4("y = 2^x; y = log_2(x); y = x"), vec![("xmin", F(-6.0)), ("xmax", F(6.0)), ("ymin", F(-6.0)), ("ymax", F(6.0))])),
            ex("Rational function with a hole and dotted asymptotes", with(q4("y = (x^2 - 1)/((x - 1)(x + 3))"), vec![("asymptotes", B(true))])),
            ex("System of linear inequalities: y > x \u{2212} 2 and y \u{2264} \u{2212}x + 3", with(q4("y > x - 2; y <= -x + 3"), vec![("shading", s("Hatching"))])),
            ex("Circle with centre (2, \u{2212}1) and radius 3: (x \u{2212} 2)\u{b2} + (y + 1)\u{b2} = 9", with(q4("(x-2)^2 + (y+1)^2 = 9"), vec![("xmin", F(-6.0)), ("xmax", F(8.0)), ("ymin", F(-7.0)), ("ymax", F(7.0)), ("points", s("C(2, -1)"))])),
            ex("Ellipse x\u{b2}/16 + y\u{b2}/9 = 1", with(q4("x^2/16 + y^2/9 = 1"), vec![("xmin", F(-6.0)), ("xmax", F(6.0)), ("ymin", F(-5.0)), ("ymax", F(5.0))])),
            ex("Hyperbola x\u{b2}/9 \u{2212} y\u{b2}/4 = 1 with its asymptotes y = \u{b1}2x/3", with(q4("x^2/9 - y^2/4 = 1; y = 2x/3; y = -2x/3"), vec![("xmin", F(-8.0)), ("xmax", F(8.0)), ("ymin", F(-6.0)), ("ymax", F(6.0))])),
            ex("Circle centred at the origin: x\u{b2} + y\u{b2} = 25", q4("x^2 + y^2 = 25")),
            ex("Shading inside a circle: x\u{b2} + y\u{b2} < 25 (dashed boundary)", q4("x^2 + y^2 < 25")),
            ex("Half circle: x\u{b2} + y\u{b2} = 25 with y > 0 (open dots at the ends)", q4("x^2 + y^2 = 25, y > 0")),
            ex("Hyperbola x\u{b2}/9 \u{2212} y\u{b2}/4 = 1 with its asymptotes drawn fine and dotted (add the word asymptote after a row)", q4("x^2/9 - y^2/4 = 1; y = 2x/3 asymptote; y = -2x/3 asymptote")),
            ex("The ring between two circles: 4 < x\u{b2} + y\u{b2} < 25", q4("4 < x^2 + y^2 < 25")),
        ],
        "number-line" => vec![
            ex("Graphing the inequality x \u{2265} \u{2212}2 on a number line", vec![("inequality", s("x >= -2"))]),
            ex("Compound inequality \u{2212}3 < x \u{2264} 4 on a number line", vec![("inequality", s("-3 < x <= 4"))]),
            ex("Graph of x < \u{2212}1 or x \u{2265} 3 on a number line", vec![("inequality", s("x < -1 or x >= 3"))]),
            ex("Fraction number line from 0 to 2 with lettered points", vec![("min", F(0.0)), ("max", F(2.0)), ("step_text", s("1/4")), ("label_every", I(4)), ("inequality", s("")), ("points", s("P(3/4), Q(3/2), R(7/4)"))]),
            ex("The sequence a\u{2099} = 1/n on a number line", vec![("min", F(0.0)), ("max", F(1.0)), ("step", F(0.1)), ("inequality", s("")), ("points", s("1, 1/2, 1/3, 1/4, 1/5, 1/6"))]),
        ],
        "box-plot" => vec![
            ex("Box plot of test scores", vec![("data", s("62, 68, 70, 71, 74, 75, 78, 80, 82, 85, 88, 91, 95")), ("axis_title", s("Test score")), ("values", B(false))]),
            ex("Comparing two box plots: Class A and Class B", vec![("data", s("62, 70, 71, 75, 78, 80, 82, 85, 88, 95")), ("name1", s("Class A")), ("data2", s("55, 60, 68, 72, 74, 79, 84, 90")), ("name2", s("Class B")), ("values", B(false))]),
            ex("Box plot from a five-number summary, with Q1, the median and Q3 labeled", vec![("data", s("")), ("summary", s("12, 18, 25, 31, 40")), ("min_lab", s("None")), ("max_lab", s("None"))]),
            ex("Box plot with outliers", vec![("data", s("3, 21, 22, 24, 25, 26, 27, 28, 30, 31, 48")), ("outliers", B(true)), ("values", B(false))]),
            ex("Parts of a box plot: label the five-number summary", vec![("min_lab", s("Text")), ("min_label", s("minimum")), ("q1_lab", s("Text")), ("q1_label", s("Q_1")), ("med_lab", s("Text")), ("med_label", s("median")), ("q3_lab", s("Text")), ("q3_label", s("Q_3")), ("max_lab", s("Text")), ("max_label", s("maximum"))]),
        ],
        "mapping-diagram" => vec![
            ex("Mapping diagram of a function", vec![("inputs", s("1, 2, 3, 4")), ("outputs", s("3, 5, 7, 9")), ("pairs", s("(1,3); (2,5); (3,7); (4,9)"))]),
            ex("Mapping diagram of a relation that is not a function", vec![("inputs", s("1, 2, 3")), ("outputs", s("4, 5, 6")), ("pairs", s("(1,4); (1,5); (2,6); (3,6)"))]),
            ex("Mapping diagram of y = x\u{b2}, a function that isn\u{2019}t one-to-one", vec![("inputs", s("-2, -1, 1, 2")), ("outputs", s("1, 4")), ("pairs", s("(-2,4); (-1,1); (1,1); (2,4)")), ("title_in", s("x")), ("title_out", s("y")), ("title_mode", s("Text")), ("title", s("y = x^2"))]),
            ex("Mapping diagram with words: students and their sports", vec![("inputs", s("Ana, Ben, Carla, Dev")), ("outputs", s("soccer, tennis, chess")), ("pairs", s("(Ana,soccer); (Ben,chess); (Carla,soccer); (Dev,tennis)")), ("title_in", s("Student")), ("title_out", s("Sport"))]),
            ex("Mapping diagram without arrows, for students to draw", vec![("inputs", s("0, 1, 2, 3")), ("outputs", s("1, 3, 5, 7")), ("pairs", s(""))]),
        ],
        "ruler-reading" => vec![
            ex("Measuring to the nearest quarter inch", vec![("marks", s("1/4 in")), ("ruler", I(6)), ("length", F(3.25)), ("read_to", s("Nearest mark")), ("object", s("Metal cylinder")), ("answer", B(true))]),
            ex("Measuring to the nearest eighth inch", vec![("marks", s("1/8 in")), ("ruler", I(6)), ("length", F(1.75)), ("read_to", s("Nearest mark")), ("object", s("Marbles")), ("marbles", I(3)), ("magnify", s("Ruler and magnifiers")), ("mag_marks", I(1))]),
            ex("Measuring to the nearest centimeter", vec![("marks", s("1 cm")), ("ruler", I(15)), ("length", F(7.0)), ("read_to", s("Nearest mark")), ("object", s("Rock")), ("guides", B(true))]),
            ex("Measuring in centimeters and millimeters: 8.4 cm", vec![("marks", s("1 mm")), ("ruler", I(15)), ("length", F(8.4)), ("read_to", s("Nearest mark")), ("object", s("Cube"))]),
            ex("Measuring with a ruler not starting at zero, in inches", vec![("marks", s("1/8 in")), ("ruler", I(6)), ("start", F(1.0)), ("length", F(2.375)), ("read_to", s("Nearest mark")), ("magnify", s("Ruler and magnifiers")), ("answer", B(true))]),
            ex("Reading a 4.37 cm metal cylinder with magnifiers", vec![("object", s("Metal cylinder")), ("magnify", s("Ruler and magnifiers")), ("ruler_color", s("#f2f2f2"))]),
        ],
        _ => return None,
    })
}

// ───────────────────────── box plot ─────────────────────────

fn median(v: &[f64]) -> f64 {
    let n = v.len();
    if n == 0 { 0.0 } else if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}
pub(super) fn five_number(data: &[f64]) -> [f64; 5] {
    let mut v = data.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    let (lo, hi) = (&v[..n / 2], &v[(n + 1) / 2..]);
    [v[0], median(lo), median(&v), median(hi), v[n - 1]]
}

/// (data key, summary key, name key) of the up-to-four data sets.
pub(super) const BOX_SETS: [(&str, &str, &str); 4] = [("data", "summary", "name1"), ("data2", "summary2", "name2"), ("data3", "summary3", "name3"), ("data4", "summary4", "name4")];
const BOX_STATS: [(&str, &str); 5] = [("min_lab", "min_label"), ("q1_lab", "q1_label"), ("med_lab", "med_label"), ("q3_lab", "q3_label"), ("max_lab", "max_label")];

fn box_plot_params() -> Vec<ParamDef> {
    vec![
        ParamDef::list("data", "Data set 1", "2, 4, 5, 5, 6, 7, 8, 9, 10"),
        ParamDef::list("summary", "Or five-number summary instead: min, Q1, median, Q3, max", ""),
        ParamDef::text("name1", "Name of set 1", ""),
        ParamDef::list("data2", "Data set 2 (optional, to compare)", ""),
        ParamDef::list("summary2", "Or five-number summary for set 2", ""),
        ParamDef::text("name2", "Name of set 2", ""),
        ParamDef::bool("values", "Write the five values", false),
        ParamDef::bool("outliers", "Mark outliers (beyond 1.5 x IQR)", false),
        ParamDef::bool("auto", "Choose the number line automatically", false),
        ParamDef::float("axis_min", "Number line minimum (if not automatic)", -1000.0, 1000.0, 1.0, 0.0),
        ParamDef::float("axis_max", "Number line maximum (if not automatic)", -1000.0, 1000.0, 1.0, 10.0),
        ParamDef::float("axis_step", "Number line step (if not automatic)", 0.05, 100.0, 0.5, 1.0),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("axis_title", "Number line label", ""),
        ParamDef::color("fill", "Box colour", &SWATCHES, "#e9f1fb"),
        ParamDef::list("data3", "Data set 3 (optional)", ""),
        ParamDef::list("summary3", "Or five-number summary for set 3", ""),
        ParamDef::text("name3", "Name of set 3", ""),
        ParamDef::list("data4", "Data set 4 (optional)", ""),
        ParamDef::list("summary4", "Or five-number summary for set 4", ""),
        ParamDef::text("name4", "Name of set 4", ""),
        ParamDef::bool("read5", "Read exactly five numbers as a five-number summary", false),
        ParamDef::choice("min_lab", "Minimum label", &["None", "Value", "Text"], "Value"),
        ParamDef::text("min_label", "Minimum label text", "min"),
        ParamDef::choice("q1_lab", "Q1 label", &["None", "Value", "Text"], "Value"),
        ParamDef::text("q1_label", "Q1 label text", "Q1"),
        ParamDef::choice("med_lab", "Median label", &["None", "Value", "Text"], "Value"),
        ParamDef::text("med_label", "Median label text", "median"),
        ParamDef::choice("q3_lab", "Q3 label", &["None", "Value", "Text"], "Value"),
        ParamDef::text("q3_label", "Q3 label text", "Q3"),
        ParamDef::choice("max_lab", "Maximum label", &["None", "Value", "Text"], "Value"),
        ParamDef::text("max_label", "Maximum label text", "max"),
        ParamDef::bool("caps", "Lines at the whisker ends", true),
        ParamDef::choice("title_mode", "Chart title", &TITLE_MODES, "Text"),
        ParamDef::choice("axis_title_mode", "Axis title", &TITLE_MODES, "Text"),
        ParamDef::choice("numbers", "Numbers", &NUMBER_EVERY, "Every tick"),
        label_size_param(),
    ]
}
const NUMBER_EVERY: [&str; 6] = ["Every tick", "Every 2nd tick", "Every 4th tick", "Every 5th tick", "Every 10th tick", "No numbers"];
/// Number every n-th tick (0 = no numbers).
fn number_every(p: &Params) -> i64 {
    match p.s("numbers") { "Every 2nd tick" | "Every 2nd line" => 2, "Every 4th tick" => 4, "Every 5th tick" | "Every 5th line" => 5, "Every 10th tick" | "Every 10th line" => 10, "No numbers" => 0, _ => 1 }
}

pub(super) fn summary_for(data: &str, summ: &str, outliers: bool) -> Option<([f64; 5], Vec<f64>)> {
    let s = nums(summ);
    if s.len() == 5 { return Some(([s[0], s[1], s[2], s[3], s[4]], vec![])); }
    let d = nums(data);
    if d.is_empty() { return None; }
    let mut f = five_number(&d);
    let mut out = vec![];
    if outliers {
        let iqr = f[3] - f[1];
        let (lo, hi) = (f[1] - 1.5 * iqr, f[3] + 1.5 * iqr);
        out = d.iter().copied().filter(|x| *x < lo || *x > hi).collect();
        let inl: Vec<f64> = d.iter().copied().filter(|x| *x >= lo && *x <= hi).collect();
        if !inl.is_empty() {
            f[0] = inl.iter().cloned().fold(f64::MAX, f64::min);
            f[4] = inl.iter().cloned().fold(f64::MIN, f64::max);
        }
    }
    Some((f, out))
}

/// One set as drawn: (index into BOX_SETS, five numbers, outliers).
pub(super) fn box_set(p: &Params, i: usize) -> Option<([f64; 5], Vec<f64>)> {
    let (dk, sk, _) = BOX_SETS[i];
    if p.b("read5") && nums(p.s(sk)).len() != 5 {
        let d = nums(p.s(dk));
        if d.len() == 5 { let mut s = d.clone(); s.sort_by(|a, b| a.partial_cmp(b).unwrap()); return Some(([s[0], s[1], s[2], s[3], s[4]], vec![])); }
    }
    summary_for(p.s(dk), p.s(sk), p.b("outliers"))
}

pub(super) struct BoxG { pub sets: Vec<(usize, [f64; 5], Vec<f64>)>, pub amin: f64, pub amax: f64, pub step: f64, pub x0: f64, pub x1: f64, pub top: f64, pub rowh: f64, pub w: f64, pub h: f64, pub ay: f64 }
fn shows_title(p: &Params, mode_key: &str, text_key: &str) -> bool {
    match p.s(mode_key) { "None" => false, "Blank line" => true, _ => !p.s(text_key).trim().is_empty() }
}
/// Layout shared with the drag handles (math_plot::box_plot).
pub(super) fn box_geom(p: &Params) -> Option<BoxG> {
    let sets: Vec<(usize, [f64; 5], Vec<f64>)> = (0..4).filter_map(|i| box_set(p, i).map(|(f, o)| (i, f, o))).collect();
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for (_, f, o) in &sets { for v in f.iter().chain(o.iter()) { lo = lo.min(*v); hi = hi.max(*v); } }
    // no data: a bare number line for students to draw on (the reference's blank box plot,
    // 0 to 10 by 1 when the line is automatic), with one empty row above it
    let blank = sets.is_empty();
    let (amin, amax, step) = if blank && p.b("auto") {
        (0.0, 10.0, 1.0)
    } else if p.b("auto") {
        let st = nice_step_with((hi - lo).max(1e-6), 8.0, Rule::Plain);
        ((lo / st).floor() * st - if lo % st == 0.0 { st } else { 0.0 }, (hi / st).ceil() * st + if hi % st == 0.0 { st } else { 0.0 }, st)
    } else { (p.f("axis_min"), p.f("axis_max").max(p.f("axis_min") + 1e-3), p.f("axis_step").max(1e-3)) };
    let named = sets.iter().any(|(i, _, _)| !p.s(BOX_SETS[*i].2).is_empty());
    // the reference's canvas is 676 wide: axis 16 to 660 with arrow tips, ticks 22 in from each end,
    // and a blank plot is just that axis 25 down on a 78 tall canvas (no empty row above it)
    let w = 676.0;
    let titled = shows_title(p, "title_mode", "title");
    let top = if titled { 46.0 } else if blank { 19.0 } else { 20.0 };
    let rowh = 84.0;
    let rows = sets.len() as f64;
    let ay = top + rowh * rows + 6.0;
    let h = ay + 53.0 + if shows_title(p, "axis_title_mode", "axis_title") { 22.0 } else { 0.0 };
    Some(BoxG { sets, amin, amax, step, x0: if named { 110.0 } else { 38.0 }, x1: w - 38.0, top, rowh, w, h, ay })
}

fn box_plot_render(p: &Params) -> String {
    let Some(g) = box_geom(p) else { return msg("Enter some data or a five-number summary") };
    let (amin, amax, step, x0, x1, w) = (g.amin, g.amax, g.step, g.x0, g.x1, g.w);
    let fs = lsz(p);
    let sx = |v: f64| x0 + (v - amin) / (amax - amin) * (x1 - x0);
    let mut d = new_doc(w, g.h);
    match p.s("title_mode") {
        "None" => {}
        "Blank line" => { d.line(w / 2.0 - 90.0, 30.0, w / 2.0 + 90.0, 30.0, INK, 1.4); }
        _ => if !p.s("title").is_empty() { math_text(&mut d, w / 2.0, 22.0, 18.0, "middle", p.s("title"), true); },
    }
    let ay = g.ay;
    axis_heads(&mut d, x0, x1, ay);
    d.line(x0, ay, x1, ay, INK, 2.4);
    let every = number_every(p);
    let k0 = (amin / step).ceil() as i64;
    let k1 = ((amax + 1e-9) / step).floor() as i64;
    for k in k0..=k1 {
        let v = k as f64 * step;
        d.line(sx(v), ay - 9.0, sx(v), ay + 9.0, INK, 2.0);
        if every > 0 && (k - k0) % every == 0 { put(&mut d, sx(v), ay + 23.0, &fmt(v, 3), 16.0); }
    }
    match p.s("axis_title_mode") {
        "None" => {}
        "Blank line" => { d.line((x0 + x1) / 2.0 - 80.0, ay + 52.0, (x0 + x1) / 2.0 + 80.0, ay + 52.0, INK, 1.4); }
        _ => if !p.s("axis_title").is_empty() { math_text(&mut d, (x0 + x1) / 2.0, ay + 46.0, 15.0, "middle", p.s("axis_title"), true); },
    }
    let caps = !matches!(p.0.get("caps"), Some(Value::B(false)));
    for (row, (si, f, outl)) in g.sets.iter().enumerate() {
        let cy = g.top + g.rowh * (row as f64 + 0.5) + 6.0;
        let bh = 36.0;
        d.line(sx(f[0]), cy, sx(f[1]), cy, INK, 2.0);
        d.line(sx(f[3]), cy, sx(f[4]), cy, INK, 2.0);
        if caps {
            d.line(sx(f[0]), cy - 11.0, sx(f[0]), cy + 11.0, INK, 2.0);
            d.line(sx(f[4]), cy - 11.0, sx(f[4]), cy + 11.0, INK, 2.0);
        }
        d.rect(sx(f[1]), cy - bh / 2.0, (sx(f[3]) - sx(f[1])).max(1.0), bh, &colp(p, "fill", TINT), INK, 2.0);
        d.line(sx(f[2]), cy - bh / 2.0, sx(f[2]), cy + bh / 2.0, INK, 3.4);
        for o in outl { d.circle(sx(*o), cy, 3.6, "#fff", INK, 1.8); }
        let name = p.s(BOX_SETS[*si].2);
        if !name.is_empty() { txt(&mut d, x0 - 14.0, cy + 5.0, 15.0, "end", name); }
        let mut last_x = -1e9;
        let mut lrow = 0;
        for (j, v) in f.iter().enumerate() {
            let (mk, tk) = BOX_STATS[j];
            let mode = match p.s(mk) { "" => "Value", m => m };
            let text = match mode {
                "Text" => p.s(tk).trim().to_string(),
                "Value" if p.b("values") => fmt(*v, 2),
                _ => String::new(),
            };
            if text.is_empty() { continue; }
            let x = sx(*v);
            if x - last_x < stw(&text, fs * 0.8) + 8.0 { lrow = 1 - lrow; } else { lrow = 0; }
            last_x = x;
            let y = cy - bh / 2.0 - 10.0 - lrow as f64 * (fs * 0.85);
            if mode == "Text" { math_text(&mut d, x, y, fs * 0.8, "middle", &text, false); } else { sput(&mut d, "", x, y, &text, fs * 0.8); }
        }
    }
    d.finish()
}

/// Inline math typesetting for plot labels: single letters (x, y, a in "2x + a") are italic,
/// `^` raises the next token (digits, a letter or a (group)), `_` lowers it, and '-' becomes a
/// minus sign. Words of two or more letters (names, sin, log) stay upright. Returns the escaped
/// content of a <text> element.
/// Is the `e` at `i` the exponent mark of a scientific literal (`2e3`, `1.5e-4`)? The literal must start with a digit or
/// dot, not follow a ^ or a letter, and the e must be directly followed by digits (after an optional sign).
pub(super) fn is_sci_e(ch: &[char], i: usize) -> bool {
    if !(ch[i] == 'e' || ch[i] == 'E') || i == 0 || !(ch[i - 1].is_ascii_digit() || ch[i - 1] == '.') { return false; }
    let mut j = i + 1;
    if j < ch.len() && (ch[j] == '+' || ch[j] == '-' || ch[j] == '\u{2212}') { j += 1; }
    if !(j < ch.len() && ch[j].is_ascii_digit()) { return false; }
    let mut st = i;
    while st > 0 && (ch[st - 1].is_ascii_digit() || ch[st - 1] == '.') { st -= 1; }
    let before = ch[..st].iter().rev().find(|c| !c.is_whitespace());
    // (as the reader does: a literal is scientific anywhere but straight after a ^, even right behind a letter: x2e3 is x times 2000)
    !before.is_some_and(|c| *c == '^' || *c == '_')
}

pub(super) fn math_tspans(s: &str) -> String {
    let ch: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let push_plain = |out: &mut String, t: &str| out.push_str(&super::svg::esc(t));
    let mut prose = false; // inside a word: keep '-' as a hyphen
    while i < ch.len() {
        let c = ch[i];
        if c == '^' || c == '_' {
            i += 1;
            let st = i;
            if i < ch.len() && ch[i] == '(' {
                let mut depth = 0;
                while i < ch.len() { if ch[i] == '(' { depth += 1; } if ch[i] == ')' { depth -= 1; if depth == 0 { i += 1; break; } } i += 1; }
            } else if i < ch.len() && (ch[i] == '-' || ch[i] == '\u{2212}') { i += 1; while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') { i += 1; } }
            else if i < ch.len() && ch[i].is_alphabetic() { i += 1; }
            else { while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') { i += 1; } }
            let mut tok: String = ch[st..i].iter().collect();
            if tok.starts_with('(') && tok.ends_with(')') && tok.len() >= 2 { tok = tok[1..tok.len() - 1].to_string(); }
            let shift = if c == '^' { "super" } else { "sub" };
            out.push_str(&format!(r#"<tspan baseline-shift="{shift}" font-size="70%">{}</tspan>"#, math_tspans(&tok)));
            continue;
        }
        if c.is_alphabetic() {
            let st = i;
            while i < ch.len() && ch[i].is_alphabetic() { i += 1; }
            let w: String = ch[st..i].iter().collect();
            // the e of a literal such as 2e3 is part of the number, not Euler's e
            if (w == "e" || w == "E") && is_sci_e(&ch, st) { push_plain(&mut out, &w); continue; }
            if w.chars().count() == 1 && !prose {
                out.push_str(&format!(r#"<tspan font-style="italic">{}</tspan>"#, super::svg::esc(&w)));
            } else {
                prose = w.chars().count() > 3 || prose;
                push_plain(&mut out, &w);
            }
            continue;
        }
        if c == '-' && !prose { out.push('\u{2212}'); } else { push_plain(&mut out, &c.to_string()); }
        i += 1;
    }
    out
}

/// Text with math typesetting (see `math_tspans`) in the serif face used for math figures.
pub(super) fn math_text(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str, bold: bool) {
    d.raw(&math_text_str(x, y, size, anchor, s, bold));
}
fn math_text_str(x: f64, y: f64, size: f64, anchor: &str, s: &str, bold: bool) -> String {
    let (fam, wt) = if bold { ("DejaVu Sans, Arial, Liberation Sans, Helvetica, sans-serif", r#" font-weight="bold""#) } else { (SERIF, "") };
    format!(r##"<text x="{x:.2}" y="{:.2}" font-size="{size}" text-anchor="{anchor}" font-family="{fam}"{wt} fill="#000">{}</text>"##, y + size * 0.34, math_tspans(s))
}

pub(super) fn list_items(s: &str) -> Vec<String> {
    let mut v: Vec<String> = vec![];
    // commas, semicolons, or one item a line / a column pasted from a spreadsheet
    for t in s.split(|c| c == ',' || c == ';' || c == '\n' || c == '\t').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
        if !v.contains(&t) { v.push(t); }
    }
    v
}

const TITLE_MODES: [&str; 3] = ["Text", "Blank line", "None"];
/// `pairs` value that joins the i-th input to the i-th output ("Match in order").
pub(super) const MATCH_IN_ORDER: &str = "in order";

fn mapping_diagram_params() -> Vec<ParamDef> {
    vec![
        ParamDef::list("inputs", "Inputs (order is top to bottom)", "1, 2, 3, 4"),
        ParamDef::list("outputs", "Outputs", "3, 5, 7, 9"),
        ParamDef::long("pairs", "Arrows as ordered pairs, e.g. (1,3); (2,5)  or  1->3; 2->5", "(1,3); (2,5); (3,7); (4,9)"),
        ParamDef::text("title_in", "Input title", "Input"),
        ParamDef::text("title_out", "Output title", "Output"),
        ParamDef::choice("shape", "Draw each side in", &["Ovals", "Boxes", "None"], "Ovals"),
        ParamDef::color("arrow_color", "Arrow colour", &SWATCHES, "#333333"),
        ParamDef::choice("in_mode", "Input title", &TITLE_MODES, "Text"),
        ParamDef::choice("out_mode", "Output title", &TITLE_MODES, "Text"),
        ParamDef::choice("title_mode", "Diagram title", &TITLE_MODES, "None"),
        ParamDef::text("title", "Diagram title text (type ^ for a power, e.g. y = x^2)", "y = 2x + 1"),
        ParamDef::choice("fill", "Side fill", &["Tinted", "White"], "Tinted"),
        label_size_param(),
        ParamDef::long("paste", "Ordered pairs, e.g. (1, 3), (2, 5), (3, 7): they replace both lists and the arrows", ""),
    ]
}

/// One arrow of a mapping diagram: input, output, and where the output sits in the source text
/// (None for "Match in order" arrows, which have no text of their own).
pub(super) struct MapPair { pub a: String, pub b: String, pub b_span: Option<(usize, usize)> }
pub(super) struct MapData { pub ins: Vec<String>, pub outs: Vec<String>, pub pairs: Vec<MapPair>, pub key: &'static str }

/// Pieces of a pairs text with their byte offsets: `(1,3), (2,5)`, `1->3; 2->5`, or one pair a line.
fn pair_segments(ps: &str) -> Vec<(String, usize)> {
    let mut segs = vec![];
    let mut off = 0;
    for piece in ps.split(|c| c == ';' || c == '\n') {
        if piece.contains('(') {
            let mut at = 0;
            while let Some(i) = piece[at..].find('(') {
                let st = at + i + 1;
                let en = piece[st..].find(')').map(|j| st + j).unwrap_or(piece.len());
                segs.push((piece[st..en].to_string(), off + st));
                at = en.min(piece.len());
                if at >= piece.len() { break; }
            }
        } else {
            segs.push((piece.to_string(), off));
        }
        off += piece.len() + 1;
    }
    segs
}

/// Inputs, outputs and arrows actually drawn: the pasted pairs when there are any (they replace
/// both lists and the arrows), else the lists plus the arrows (a pair may add a new item).
pub(super) fn mapping_parse(p: &Params) -> MapData {
    let pasted = !p.s("paste").trim().is_empty();
    let key = if pasted { "paste" } else { "pairs" };
    let (mut ins, mut outs) = if pasted { (vec![], vec![]) } else { (list_items(p.s("inputs")), list_items(p.s("outputs"))) };
    let ps = p.s(key);
    let mut pairs = vec![];
    if !pasted && ps.trim().eq_ignore_ascii_case(MATCH_IN_ORDER) {
        for (a, b) in ins.iter().zip(outs.iter()) { pairs.push(MapPair { a: a.clone(), b: b.clone(), b_span: None }); }
        return MapData { ins, outs, pairs, key };
    }
    for (inner, off) in pair_segments(ps) {
        let sep = if let Some(i) = inner.find("->") { Some((i, 2)) } else if let Some(i) = inner.find('\u{2192}') { Some((i, 3)) } else { inner.find(',').map(|i| (i, 1)) };
        let Some((i, l)) = sep else { continue };
        let (a, braw) = (inner[..i].trim().to_string(), &inner[i + l..]);
        let b = braw.trim().to_string();
        if a.is_empty() || b.is_empty() { continue; }
        let st = off + i + l + (braw.len() - braw.trim_start().len());
        if !ins.contains(&a) { ins.push(a.clone()); }
        if !outs.contains(&b) { outs.push(b.clone()); }
        if !pairs.iter().any(|q: &MapPair| q.a == a && q.b == b) { pairs.push(MapPair { a, b: b.clone(), b_span: Some((st, st + b.len())) }); }
    }
    MapData { ins, outs, pairs, key }
}

/// The reference's check under the arrows: every input needs exactly one arrow to be a function
/// (only once some arrows are drawn; a diagram with none is for students to finish).
pub(super) fn mapping_msg(p: &Params) -> Option<String> {
    let m = mapping_parse(p);
    if m.pairs.is_empty() { return None; }
    let n = |a: &str| m.pairs.iter().filter(|q| q.a == a).count();
    if let Some(a) = m.ins.iter().find(|a| n(a) > 1) { return Some(format!("Not a function: {a} has {} outputs. Every input needs exactly one.", n(a))); }
    if let Some(a) = m.ins.iter().find(|a| n(a) == 0) { return Some(format!("Not a function yet: {a} has no output. Every input needs exactly one.")); }
    None
}

/// A title in one of the Text / Blank line / None modes.
fn mode_title(d: &mut Doc, mode: &str, x: f64, y: f64, text: &str, size: f64) {
    match mode {
        "None" => {}
        "Blank line" => { d.line(x - 50.0, y + size * 0.45, x + 50.0, y + size * 0.45, INK, 1.4); }
        _ => math_text(d, x, y, size, "middle", text, true),
    }
}

fn mapping_diagram_render(p: &Params) -> String {
    let MapData { ins, outs, pairs, .. } = mapping_parse(p);
    let pairs: Vec<(String, String)> = pairs.into_iter().map(|q| (q.a, q.b)).collect();
    let fs = lsz(p);
    // no pairs: two empty ovals for students to fill in (the reference's blank diagram)
    let rows = if ins.is_empty() && outs.is_empty() { 4.0 } else { ins.len().max(outs.len()).max(1) as f64 };
    let spacing = 34.8;
    let has_title = p.s("title_mode") == "Text" || p.s("title_mode") == "Blank line";
    let top = if has_title { 40.0 } else { 0.0 };
    // the reference's canvas: two ovals 49.4 wide, 65.4 in from the sides, the headings 34 down
    let ry0 = rows * spacing / 2.0 + 14.0;
    let cy = top + 49.4 + ry0;
    let h = cy + ry0 + 16.0;
    let shape = p.s("shape");
    let fill = if p.s("fill") == "White" { "#ffffff" } else { TINT };
    let iw = |v: &[String]| v.iter().map(|t| stw(t, fs)).fold(0.0, f64::max);
    let (wi, wo) = (iw(&ins) / 2.0 + 24.0, iw(&outs) / 2.0 + 24.0);
    let (rxi, rxo) = (wi.max(49.4), wo.max(49.4));
    let lx = 16.0 + rxi;
    let rx = lx + rxi + 150.0 + rxo;
    let w = rx + rxo + 16.0;
    let mut d = new_doc(w, h);
    let draw_side = |d: &mut Doc, x: f64, n: usize, half_w: f64| {
        let _ = n;
        let ry = rows * spacing / 2.0 + 14.0;
        let rxx = half_w;
        match shape {
            "Ovals" => { d.raw(&format!(r##"<ellipse cx="{x}" cy="{cy}" rx="{rxx}" ry="{ry}" fill="{fill}" stroke="{INK}" stroke-width="2.2"/>"##)); }
            "Boxes" | "Rectangles" => { d.raw(&format!(r##"<rect x="{}" y="{}" width="{}" height="{}" rx="4" fill="{fill}" stroke="{INK}" stroke-width="2.2"/>"##, x - rxx, cy - ry, 2.0 * rxx, 2.0 * ry)); }
            _ => {}
        }
    };
    draw_side(&mut d, lx, ins.len(), rxi);
    draw_side(&mut d, rx, outs.len(), rxo);
    if has_title { mode_title(&mut d, p.s("title_mode"), w / 2.0, 26.0, p.s("title"), 19.0); }
    let im = match p.s("in_mode") { "" => "Text", m => m };
    let om = match p.s("out_mode") { "" => "Text", m => m };
    mode_title(&mut d, im, lx, top + 28.0, p.s("title_in"), 18.0);
    mode_title(&mut d, om, rx, top + 28.0, p.s("title_out"), 18.0);
    // both sides are the same height; a shorter list spreads out over it
    let pos = |i: usize, n: usize| { let g = rows * spacing / n.max(1) as f64; cy + (i as f64 - (n as f64 - 1.0) / 2.0) * g };
    let tw_in: Vec<f64> = ins.iter().map(|t| stw(t, fs)).collect();
    let tw_out: Vec<f64> = outs.iter().map(|t| stw(t, fs)).collect();
    for (a, b) in &pairs {
        let (i, j) = (ins.iter().position(|x| x == a).unwrap(), outs.iter().position(|x| x == b).unwrap());
        let (ya, yb) = (pos(i, ins.len()), pos(j, outs.len()));
        let (xa, xb) = (lx + (tw_in[i] / 2.0 + 8.0).max(26.0), rx - (tw_out[j] / 2.0 + 10.0).max(28.0));
        d.arrow(xa, ya, xb, yb, &colp(p, "arrow_color", "#333"), 1.8);
    }
    for (i, t) in ins.iter().enumerate() { math_text(&mut d, lx, pos(i, ins.len()), fs, "middle", t, false); }
    for (j, t) in outs.iter().enumerate() { math_text(&mut d, rx, pos(j, outs.len()), fs, "middle", t, false); }
    d.finish()
}

// ───────────────────────── length reading (ruler) ─────────────────────────

fn length_reading_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("marks", "Smallest marks", &["1 cm", "5 mm", "1 mm", "1 in", "1/2 in", "1/4 in", "1/8 in", "1/16 in", "10 cm", "5 cm"], "1 mm"),
        ParamDef::int("ruler", "Ruler length (cm or in)", 3, 100, 15),
        ParamDef::choice("object", "Object", &["Bar", "Pencil", "None", "Marbles", "Rock", "Cube", "Metal cylinder"], "Bar"),
        ParamDef::float("start", "Left end of the object (on the ruler)", 0.0, 100.0, 0.05, 0.0),
        ParamDef::float("length", "Length of the object", 0.1, 100.0, 0.01, 4.37),
        ParamDef::bool("guides", "Dotted guides from the object's ends to the ruler", true),
        ParamDef::bool("show_value", "Write the length above the object", false),
        ParamDef::int("dec", "Decimals in the written length", 0, 3, 2),
        ParamDef::color("ruler_color", "Ruler colour", &["#fbf1c7", "#e9f1fb", "#c9def4", "#f3c969", "#ffffff", "#d9d9d9", "#f2f2f2"], "#fbf1c7"),
        ParamDef::color("obj_color", "Bar colour", &["#c9def4", "#f3c969", "#e48a9a", "#b9e0b5", "#d9d9d9", "#ffffff"], "#c9def4"),
        ParamDef::choice("read_to", "Read to", &["One estimated digit", "Nearest mark"], "One estimated digit"),
        ParamDef::int("marbles", "Marbles: how many, in a row", 1, 5, 3),
        ParamDef::choice("magnify", "Magnifiers", &["Ruler only", "Ruler and magnifiers"], "Ruler only"),
        ParamDef::int("mag_marks", "Numbered marks in the magnifier", 1, 6, 2),
        ParamDef::choice("title_mode", "Chart title", &["Text", "None"], "None"),
        ParamDef::text("title", "Chart title text", "How long is the object?"),
        ParamDef::bool("answer", "Answer key: print the length under the figure", false),
    ]
}

/// Units between the smallest marks, and whether the ruler is metric (cm).
fn ruler_step(marks: &str) -> (f64, bool) {
    match marks {
        "10 cm" => (10.0, true), "5 cm" => (5.0, true), "1 cm" => (1.0, true), "5 mm" => (0.5, true), "1 mm" => (0.1, true),
        "1 in" => (1.0, false), "1/2 in" => (0.5, false), "1/4 in" => (0.25, false), "1/8 in" => (0.125, false), _ => (0.0625, false),
    }
}

/// Decimal places a reading carries: the smallest mark's, plus one estimated digit.
fn ruler_read_dec(step: f64, estimate: bool) -> i64 {
    let base = if step >= 1.0 { 0 } else if step >= 0.1 - 1e-9 { 1 } else { 2 };
    base + if estimate { 1 } else { 0 }
}

/// (start, length) as drawn: snapped to the "Read to" precision when that is set.
pub(super) fn ruler_values(p: &Params) -> (f64, f64) {
    let (s, l) = (p.f("start"), p.f("length"));
    let (step, metric) = ruler_step(p.s("marks"));
    match p.s("read_to") {
        "Nearest mark" => { let q = |v: f64| (v / step).round() * step; (q(s), q(l).max(step)) }
        "One estimated digit" => {
            let r = if metric { 10f64.powi(ruler_read_dec(step, true) as i32) } else { 100.0 };
            let q = |v: f64| (v * r).round() / r;
            (q(s), q(l).max(1.0 / r))
        }
        _ => (s, l),
    }
}

/// A reading written for the answer key: decimals for cm, a mixed fraction for inches read to a mark.
fn ruler_fmt(v: f64, p: &Params) -> String {
    let (step, metric) = ruler_step(p.s("marks"));
    let near_mark = p.s("read_to") == "Nearest mark";
    if metric {
        let dec = ruler_read_dec(step, !near_mark) as usize;
        return format!("{:.*} cm", dec, v);
    }
    if near_mark {
        let den = (1.0 / step).round() as i64;
        let n = (v * den as f64).round() as i64;
        let (whole, mut num, mut den) = (n / den, n % den, den);
        while num > 0 && num % 2 == 0 && den % 2 == 0 { num /= 2; den /= 2; }
        return match (whole, num) { (w, 0) => format!("{w} in"), (0, n) => format!("{n}/{den} in"), (w, n) => format!("{w} {n}/{den} in") };
    }
    format!("{:.2} in", v)
}

pub(super) struct RulerL { pub w: f64, pub h: f64, pub x0: f64, pub x1: f64, pub ppu: f64, pub ry: f64, pub top: f64, pub bot: f64, pub title_y: f64, pub mag_cy: f64 }
const MAG_R: f64 = 92.0;

/// Object height on screen for the new objects (sitting on the ruler).
fn ruler_obj_h(p: &Params, len_px: f64) -> f64 {
    match p.s("object") {
        "Marbles" => len_px / p.i("marbles").clamp(1, 5) as f64,
        "Rock" => (len_px * 0.55).clamp(14.0, 90.0),
        "Cube" => len_px.clamp(10.0, 110.0),
        "Metal cylinder" => (len_px * 0.45).clamp(16.0, 50.0),
        _ => 26.0,
    }
}

/// Layout shared with the drag handles (math_plot::ruler).
pub(super) fn ruler_layout(p: &Params) -> RulerL {
    let len = p.i("ruler").max(1) as f64;
    let (w, x0, x1) = (680.0, 30.0, 650.0);
    let ppu = (x1 - x0) / len;
    let (_, l) = ruler_values(p);
    let new_obj = matches!(p.s("object"), "Marbles" | "Rock" | "Cube" | "Metal cylinder");
    let oh = if new_obj { ruler_obj_h(p, l * ppu) } else { 0.0 };
    let title = if p.s("title_mode") == "Text" && !p.s("title").trim().is_empty() { 34.0 } else { 0.0 };
    let mag = if p.s("magnify") == "Ruler and magnifiers" && p.s("object") != "None" { 2.0 * MAG_R + 44.0 } else { 0.0 };
    let extra = (oh - 70.0).max(0.0);
    let ry = 96.0 + title + mag + extra;
    let (top, bot) = if new_obj { (ry - oh, ry) } else { (ry - 54.0, ry - 28.0) };
    let h = 190.0 + title + mag + extra + if p.b("answer") { 30.0 } else { 0.0 };
    RulerL { w, h, x0, x1, ppu, ry, top, bot, title_y: 24.0, mag_cy: title + 18.0 + MAG_R }
}

/// Draws ticks, numbers and the object. `xv` maps a ruler value to x, `k` scales tick heights
/// and text (magnifier), values outside [vmin, vmax] are skipped.
fn ruler_scene(d: &mut Doc, p: &Params, xv: &dyn Fn(f64) -> f64, ry: f64, k: f64, vmin: f64, vmax: f64, ppu: f64, top: f64, bot: f64) {
    let (step0, metric) = ruler_step(p.s("marks"));
    let len = p.i("ruler").max(1) as f64;
    // drop to coarser marks when they would crowd (main ruler only; k > 1 is a magnifier)
    let mut step = step0;
    while ppu * step <= 3.2 && k <= 1.0 {
        step = if metric { match step { s if s < 0.5 => 0.5, s if s < 1.0 => 1.0, s if s < 5.0 => 5.0, _ => 10.0 } } else { step * 2.0 };
        if step >= 10.0 { break; }
    }
    let label_every = if step >= 10.0 { 10.0 } else if step >= 5.0 { 5.0 } else if ppu * k >= 22.0 { 1.0 } else if ppu * k >= 9.0 { 5.0 } else { 10.0 };
    let total = (len / step).round() as i64;
    let isn = |v: f64, m: f64| ((v / m) - (v / m).round()).abs() < 1e-6;
    for i in 0..=total {
        let v = i as f64 * step;
        if v < vmin - 1e-9 || v > vmax + 1e-9 { continue; }
        let x = xv(v);
        let th = if metric {
            if step >= 1.0 { if step == 5.0 && !isn(v, 10.0) { 20.0 } else { 26.0 } }
            else if isn(v, 1.0) { 26.0 } else if isn(v, 0.5) { 18.0 } else { 11.0 }
        } else {
            let lg = (1.0 / step).log2();
            let mut tz = 0.0;
            while tz < lg && isn(v, step * 2f64.powf(tz + 1.0)) { tz += 1.0; }
            (26.0 - 4.5 * (lg - tz)).max(7.0)
        };
        d.line(x, ry, x, ry + th * k, INK, if th >= 26.0 { 1.6 } else { 1.1 } * k.sqrt());
        if isn(v, 1.0) && isn(v, label_every) { put(d, x, ry + 42.0 * k, &fmt(v, 0), 14.0 * k); }
    }
    // object
    let (s, l) = ruler_values(p);
    let (ox0, ox1) = (xv(s), xv(s + l));
    let oh = bot - top;
    let col = colp(p, "obj_color", "#c9def4");
    match p.s("object") {
        "Bar" => { d.rect(ox0, top, ox1 - ox0, oh, &col, INK, 1.8); }
        "Pencil" => {
            let tip = (16.0 * k).min((ox1 - ox0) * 0.4);
            let mid = (top + bot) / 2.0;
            d.poly(&[(ox0, top), (ox1 - tip, top), (ox1, mid), (ox1 - tip, bot), (ox0, bot)], "#f3c969", INK, 1.8);
            d.poly(&[(ox1 - tip, top), (ox1, mid), (ox1 - tip, bot)], "#f1dcb4", INK, 1.8);
            d.rect(ox0, top, 22.0 * k, oh, "#e48a9a", INK, 1.6);
        }
        "Metal cylinder" => {
            let r = (oh * 0.18).min(8.0 * k);
            d.raw(&format!(r##"<rect x="{ox0:.2}" y="{top:.2}" width="{:.2}" height="{oh:.2}" rx="{r:.2}" fill="#a6a6a6" stroke="none"/>"##, ox1 - ox0));
            d.rect(ox0 + r * 0.3, top + oh * 0.18, (ox1 - ox0 - r * 0.6).max(0.0), oh * 0.12, "#d6d6d6", "none", 0.0);
            d.rect(ox0 + r * 0.3, top + oh * 0.66, (ox1 - ox0 - r * 0.6).max(0.0), oh * 0.16, "#7d7d7d", "none", 0.0);
            d.raw(&format!(r##"<rect x="{ox0:.2}" y="{top:.2}" width="{:.2}" height="{oh:.2}" rx="{r:.2}" fill="none" stroke="#111" stroke-width="1.8"/>"##, ox1 - ox0));
        }
        "Cube" => {
            d.rect(ox0, top, ox1 - ox0, oh, "#b3b3b3", INK, 1.8);
            d.rect(ox0 + 1.0, top + 1.0, (ox1 - ox0 - 2.0).max(0.0), oh * 0.12, "#d9d9d9", "none", 0.0);
            d.line(ox0, top + oh * 0.12, ox1, top + oh * 0.12, "#555", 1.0);
        }
        "Rock" => {
            let (w, mid) = (ox1 - ox0, top + oh * 0.55);
            let path = format!(
                "M{:.2},{:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} Z",
                ox0, mid,
                ox0, top + oh * 0.2, ox0 + w * 0.25, top, ox0 + w * 0.5, top,
                ox0 + w * 0.8, top, ox1, top + oh * 0.25, ox1, mid,
                ox1, top + oh * 0.85, ox0 + w * 0.75, bot, ox0 + w * 0.5, bot,
                ox0 + w * 0.2, bot, ox0, top + oh * 0.9, ox0, mid);
            d.path(&path, "#a3a3a3", INK, 1.8);
            d.path(&format!("M{:.2},{:.2} Q{:.2},{:.2} {:.2},{:.2}", ox0 + w * 0.15, top + oh * 0.35, ox0 + w * 0.5, top + oh * 0.25, ox0 + w * 0.85, top + oh * 0.45), "none", INK, 1.0);
            d.path(&format!("M{:.2},{:.2} Q{:.2},{:.2} {:.2},{:.2}", ox0 + w * 0.62, top + oh * 0.32, ox0 + w * 0.55, top + oh * 0.7, ox0 + w * 0.66, bot - oh * 0.05), "none", INK, 1.0);
        }
        "Marbles" => {
            let n = p.i("marbles").clamp(1, 5) as usize;
            let dm = (ox1 - ox0) / n as f64;
            for i in 0..n {
                let cx = ox0 + dm * (i as f64 + 0.5);
                let cy = bot - dm / 2.0;
                d.circle(cx, cy, dm / 2.0, "#8f8f8f", INK, 1.4);
                d.raw(&format!(r##"<ellipse cx="{:.2}" cy="{:.2}" rx="{:.2}" ry="{:.2}" transform="rotate(-30 {:.2} {:.2})" fill="#fff" stroke="#111" stroke-width="0.8"/>"##, cx - dm * 0.2, cy - dm * 0.18, dm * 0.11, dm * 0.065, cx - dm * 0.2, cy - dm * 0.18));
            }
        }
        _ => {}
    }
}

fn length_reading_render(p: &Params) -> String {
    let lay = ruler_layout(p);
    let (x0, x1, ppu, ry) = (lay.x0, lay.x1, lay.ppu, lay.ry);
    let len = p.i("ruler").max(1) as f64;
    let (_, metric) = ruler_step(p.s("marks"));
    let mut d = new_doc(lay.w, lay.h);
    if p.s("title_mode") == "Text" && !p.s("title").trim().is_empty() {
        txt(&mut d, lay.w / 2.0, lay.title_y, 17.0, "middle", p.s("title").trim());
    }
    let rc = colp(p, "ruler_color", "#fbf1c7");
    d.rect(x0 - 8.0, ry, x1 - x0 + 16.0, 62.0, &rc, INK, 1.8);
    let xv = |v: f64| x0 + v * ppu;
    let xclip = |v: f64| xv(v).min(x1 + 8.0);
    ruler_scene(&mut d, p, &xclip, ry, 1.0, 0.0, len, ppu, lay.top, lay.bot);
    txt(&mut d, x0 + 2.0, ry + 56.0, 12.0, "start", if metric { "cm" } else { "in" });
    let (s, l) = ruler_values(p);
    let (ox0, ox1) = (xv(s), xclip(s + l));
    let obj = p.s("object");
    if obj != "None" {
        if p.b("guides") {
            match obj {
                "Bar" => { dline(&mut d, (ox0, lay.bot), (ox0, ry), "#555", 1.2, "2 3"); dline(&mut d, (ox1, lay.bot), (ox1, ry), "#555", 1.2, "2 3"); }
                "Pencil" => { dline(&mut d, (ox0, lay.bot), (ox0, ry), "#555", 1.2, "2 3"); dline(&mut d, (ox1, (lay.top + lay.bot) / 2.0), (ox1, ry), "#555", 1.2, "2 3"); }
                "Marbles" | "Rock" => {
                    let ym = if obj == "Rock" { lay.top + (lay.bot - lay.top) * 0.55 } else { (lay.top + lay.bot) / 2.0 };
                    dline(&mut d, (ox0, ym), (ox0, ry), "#555", 1.2, "5 4");
                    dline(&mut d, (ox1, ym), (ox1, ry), "#555", 1.2, "5 4");
                }
                _ => {}
            }
        }
        if p.b("show_value") { put(&mut d, (ox0 + ox1) / 2.0, lay.top - 16.0, &fmtu(l, p.i("dec"), if metric { "cm" } else { "in" }), 16.0); }
        if p.s("magnify") == "Ruler and magnifiers" {
            // one on the right end, one on the left end unless it is lined up with 0
            let mut ends = vec![s + l];
            if s > 1e-9 { ends.insert(0, s); }
            // span about `mag_marks` numbered marks, but always enlarge at least 2.5 times
            let (step, _) = ruler_step(p.s("marks"));
            let num_gap = if step >= 1.0 { step } else { 1.0 };
            let ppm = (2.0 * MAG_R / ((p.i("mag_marks").clamp(1, 6) as f64 + 0.6) * num_gap)).max(1.6 * ppu);
            let units = MAG_R / ppm + step;
            let k = 2.0;
            let mut cxs: Vec<f64> = ends.iter().map(|v| xv(*v).clamp(MAG_R + 6.0, lay.w - MAG_R - 6.0)).collect();
            if cxs.len() == 2 && cxs[1] - cxs[0] < 2.0 * MAG_R + 12.0 {
                let mid = (cxs[0] + cxs[1]) / 2.0;
                let left = (mid - MAG_R - 6.0).clamp(MAG_R + 6.0, lay.w - 3.0 * MAG_R - 18.0);
                cxs = vec![left, left + 2.0 * MAG_R + 12.0];
            }
            for (j, (v, cx)) in ends.iter().zip(cxs).enumerate() {
                let cy = lay.mag_cy;
                let rs = MAG_R * ppu / ppm; // radius of the circled spot on the ruler
                let (sx, sy) = (xv(*v), ry + 6.0);
                d.circle(sx, sy, rs, "none", INK, 1.2);
                dline(&mut d, (sx - rs, sy), (cx - MAG_R * 0.95, cy + MAG_R * 0.3), "#333", 1.0, "3 3");
                dline(&mut d, (sx + rs, sy), (cx + MAG_R * 0.95, cy + MAG_R * 0.3), "#333", 1.0, "3 3");
                let id = format!("rmag{j}");
                d.raw(&format!(r##"<defs><clipPath id="{id}"><circle cx="{cx:.2}" cy="{cy:.2}" r="{MAG_R}"/></clipPath></defs><circle cx="{cx:.2}" cy="{cy:.2}" r="{MAG_R}" fill="#fff"/><g clip-path="url(#{id})">"##));
                let mry = cy - MAG_R * 0.3;
                let mx = |u: f64| cx + (u - v) * ppm;
                d.rect(cx - MAG_R - 4.0, mry, 2.0 * MAG_R + 8.0, 2.0 * MAG_R, &rc, INK, 2.0);
                let (top, bot) = (mry - (lay.bot - lay.top) * ppm / ppu, mry);
                let (top, bot) = if matches!(obj, "Bar" | "Pencil") { (mry - 54.0 * 1.3, mry - 28.0 * 1.3) } else { (top, bot) };
                ruler_scene(&mut d, p, &mx, mry, k, v - units, v + units, ppm, top, bot);
                d.raw("</g>");
                d.circle(cx, cy, MAG_R, "none", INK, 2.2);
            }
        }
    }
    if p.b("answer") && obj != "None" {
        let t = if s > 1e-9 {
            format!("Left end: {}, right end: {}, length: {}", ruler_fmt(s, p), ruler_fmt(s + l, p), ruler_fmt(l, p))
        } else { format!("Length: {}", ruler_fmt(l, p)) };
        txt(&mut d, lay.w / 2.0, lay.h - 12.0, 15.0, "middle", &t);
    }
    d.finish()
}

// ───────────────────────── composite shape ─────────────────────────

/// A filled piece of a composite shape, with its own shading when one is named.
enum Piece { Poly(Vec<Pt>, Option<&'static str>) }

/// The "Start with" chips (the reference's first screen): each loads the preset's pieces into the
/// Pieces card, so they can be edited, dragged and added to.
pub const COMPOSITE_STARTS: [&str; 7] = ["Rectangle", "Triangle", "Semicircle", "Circle", "L-shape", "House", "Arch"];
pub const COMPOSITE_CUSTOM: &str = "Custom (use the text below)";
const COMPOSITE_HELP: &str = "One piece or mark a row. Pieces: rect X Y W H | tri X1 Y1 X2 Y2 X3 Y3 | poly X Y X Y ... | circle CX CY R | semi CX CY R u/d/l/r (the way it bulges) | quarter CX CY R ne/nw/se/sw. End a piece with blue, red, green or yellow to shade it, and with | TEXT to write a label inside it (rect 0 0 6 4 blue | A). Marks: dim X1 Y1 X2 Y2 TEXT (a dimension; no text = its length; drawn right of the direction of travel) | dash X1 Y1 X2 Y2 (a height or other line) | ticks X1 Y1 X2 Y2 N (congruence ticks) | arrows X1 Y1 X2 Y2 N (parallel arrows) | text X Y WORDS | ra X Y NE/NW/SE/SW (right angle) | side X1 Y1 X2 Y2 dashed/dotted/hidden/solid thin/regular/thick (the line of an outline side) | angle X Y [ext] [measure] [arcs N] [square] [blue/red/green/yellow] [| TEXT] (the angle at a corner, or its exterior angle with ext: its measure or TEXT, 1-3 congruence arcs, a right-angle square, shading) | point X Y NAME (a point on a corner, named). On the canvas: pick a tool to draw a piece, click a piece to select it, drag it to move, drag its handles to resize, click + beside a side to add a piece there; the Label tool labels, marks and styles sides, angles and corners, and turns the figure.";

/// Piece shading named at the end of a row (the reference's None / Blue / Red / Green / Yellow).
#[cfg(test)]
pub(super) fn shade_word(tk: &[&str]) -> Option<&'static str> {
    tk.iter().skip(1).find_map(|t| super::composite::shade_hex(t))
}

/// Arc points of a quarter circle sector piece (`quarter CX CY R ne`), centre first.
pub(super) fn quarter_pts(x: f64, y: f64, r: f64, dir: &str) -> Vec<Pt> {
    let a0 = match dir.to_lowercase().as_str() { "nw" => 90.0, "sw" => 180.0, "se" => 270.0, _ => 0.0 };
    let mut v = vec![(x, y)];
    v.extend(ell(x, y, r, r, a0, a0 + 90.0));
    v
}

fn composite_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("preset", "Start from", &[COMPOSITE_CUSTOM, "L-shape", "House", "Arch", "T-shape", "Rectangle", "Triangle", "Semicircle", "Circle"], COMPOSITE_CUSTOM),
        ParamDef::long("spec",
            "Pieces separated by ;  rect X Y W H | tri X1 Y1 X2 Y2 X3 Y3 | poly X Y X Y ... | circle CX CY R | semi CX CY R DIR(u/d/l/r bulge) | quarter CX CY R ne/nw/se/sw | dash X1 Y1 X2 Y2 | dim X1 Y1 X2 Y2 TEXT (label drawn to the right of travel, go counterclockwise) | ticks X1 Y1 X2 Y2 N | arrows X1 Y1 X2 Y2 N | text X Y WORDS | ra X Y NE/NW/SE/SW (right angle) | side X1 Y1 X2 Y2 dashed/dotted/hidden thin/thick | angle X Y [ext] [measure] [arcs N] [square] [shade] [| TEXT] | point X Y NAME; end a piece with blue/red/green/yellow to shade it and | TEXT to label it; @DX,DY on a dim, angle, point or text is a dragged label's offset",
            "rect 0 0 6 4; semi 3 4 3 u; ra 0 0 NE; ra 6 0 NW"),
        ParamDef::bool("shade", "Shade the shape", true),
        ParamDef::color("fill_color", "Shade colour", &SWATCHES, "#e9f1fb"),
        ParamDef::text("unit", "Unit for dims given without text (e.g. cm)", ""),
        ParamDef::int("dec", "Round measured dims to (decimal places)", 0, 3, 1),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        ParamDef::int("angle_dec", "Round angles to (decimal places)", 0, 2, 0),
        ParamDef::float("rotate", "Turn (degrees)", -180.0, 180.0, 1.0, 0.0),
    ]
}

pub fn preset_spec(name: &str) -> Option<&'static str> {
    Some(match name {
        "L-shape" => "rect 0 0 8 3; rect 0 3 3 5; dim 0 0 8 0 8 cm; dim 8 0 8 3 3 cm; dim 8 3 3 3 5 cm; dim 3 3 3 8 5 cm; dim 3 8 0 8 3 cm; dim 0 8 0 0 8 cm; ra 0 0 NE; ra 8 0 NW; ra 0 8 SE",
        "House" => "rect 0 0 6 4; tri 0 4 6 4 3 7; dash 3 4 3 7; text 3.5 5.4 3 cm; dim 0 0 6 0 6 cm; dim 0 4 0 0 4 cm; ra 0 0 NE; ra 6 0 NW",
        "Arch" => "rect 0 0 6 4; semi 3 4 3 u; dash 3 4 6 4; text 4.5 4.6 r = 3; dim 0 0 6 0 6 cm; dim 6 0 6 4 4 cm; ra 0 0 NE; ra 6 0 NW",
        "T-shape" => "rect 3 0 3 6; rect 0 6 9 3; dim 3 0 6 0 3 cm; dim 6 0 6 6 6 cm; dim 9 9 0 9 9 cm; dim 9 6 9 9 3 cm; ra 3 0 NE; ra 6 0 NW",
        "Rectangle" => "rect 0 0 8 5; dim 0 0 8 0; dim 8 0 8 5; ra 0 0 NE; ra 8 0 NW; ra 8 5 SW; ra 0 5 SE",
        "Triangle" => "tri 0 0 8 0 3 5; dash 3 5 3 0; dim 0 0 8 0; text 3.6 2.2 h = 5; ra 3 0 NE",
        "Semicircle" => "semi 0 0 4 u; dash 0 0 4 0; dim -4 0 4 0; text 2 0.6 r = 4",
        "Circle" => "circle 0 0 4; dash 0 0 4 0; text 2 0.6 r = 4",
        _ => return None,
    })
}

fn composite_render(p: &Params) -> String {
    composite_svg(p, None)
}

/// An explicit model -> canvas mapping for the canvas editor: canvas size (w, h) and the map
/// (scale, origin). The editor's render has no background and draws nothing when empty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompositeView { pub w: f64, pub h: f64, pub s: f64, pub ox: f64, pub oy: f64 }

/// The composite shape drawn by the canvas editor at its own pan / zoom.
pub fn composite_view_svg(p: &Params, v: CompositeView) -> String {
    composite_svg(p, Some(v))
}

/// The spec the figure draws: a "Start from" preset's, or the typed `spec`.
pub fn composite_spec(p: &Params) -> String {
    preset_spec(p.s("preset")).unwrap_or(p.s("spec")).to_string()
}

/// A label the canvas editor lets you drag: spec row `item`, centre and half size (canvas px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelBox { pub item: usize, pub c: Pt, pub half: Pt }

/// The labels of the composite shape drawn at the canvas editor's view, for hit tests.
pub fn composite_label_boxes(p: &Params, v: CompositeView) -> Vec<LabelBox> {
    composite_svg_boxes(p, Some(v)).1
}

fn composite_svg(p: &Params, view: Option<CompositeView>) -> String {
    composite_svg_boxes(p, view).0
}

fn composite_svg_boxes(p: &Params, view: Option<CompositeView>) -> (String, Vec<LabelBox>) {
    use super::composite::{self as cs, Shape};
    let mut boxes: Vec<LabelBox> = vec![];
    let spec = composite_spec(p);
    let items = cs::parse(&spec);
    let mut pieces: Vec<Piece> = vec![];
    let mut labels: Vec<(Pt, String, Option<(usize, Pt)>)> = vec![];
    let mut dashes: Vec<(Pt, Pt)> = vec![];
    let mut dims: Vec<(Pt, Pt, String, usize, Pt)> = vec![];
    let mut texts: Vec<(Pt, String, Option<(usize, Pt)>)> = vec![];
    let mut rights: Vec<(Pt, String)> = vec![];
    let mut marks: Vec<(Pt, Pt, u8, bool)> = vec![];
    let mut styled = false;
    let mut angles: Vec<(cs::AngleGeo, bool, u8, bool, Option<&'static str>, String, usize, Pt)> = vec![];
    let mut points: Vec<(Pt, String, usize, Pt)> = vec![];
    for (ix, it) in items.iter().enumerate() {
        let off = it.off.unwrap_or((0.0, 0.0));
        let Some(sh) = &it.shape else { continue };
        match sh {
            Shape::Dash(a, b) => dashes.push((*a, *b)),
            Shape::Dim(a, b, t) => dims.push((*a, *b, t.clone(), ix, off)),
            Shape::Ticks(a, b, k) => marks.push((*a, *b, *k, false)),
            Shape::Arrows(a, b, k) => marks.push((*a, *b, *k, true)),
            Shape::Text(a, t) => texts.push((*a, t.clone(), Some((ix, off)))),
            Shape::Ra(a, d) => rights.push((*a, d.clone())),
            Shape::SideStyle(..) => styled = true,
            Shape::Angle { v, ext, measure, arcs, square } => {
                // an angle whose corner is gone (the shape was changed) draws nothing
                if let Some(g) = cs::angle_geo(&items, *v, *ext) {
                    let lab = if *measure { format!("{}\u{b0}", fmt(g.deg, p.i("angle_dec"))) } else { it.label.trim().to_string() };
                    angles.push((g, *ext, *arcs, *square, it.shade.as_deref().and_then(cs::shade_hex), lab, ix, off));
                }
            }
            Shape::Point(a, n) => points.push((*a, n.trim().to_string(), ix, off)),
            _ => {
                let v = cs::outline(sh);
                if !it.label.trim().is_empty() { labels.push((cs::centroid(&v), it.label.clone(), None)); }
                pieces.push(Piece::Poly(v, it.shade.as_deref().and_then(cs::shade_hex)));
            }
        }
    }
    if pieces.is_empty() && view.is_none() { return (msg("No pieces: start the text with rect, tri, circle, semi or poly"), vec![]); }
    // the figure's Turn: every point turns about the middle of the pieces; labels stay upright
    let turn = p.f("rotate");
    let tc = cs::turn_centre(&items);
    let tr = |q: Pt| cs::rot(q, tc, turn);
    let mut all: Vec<Pt> = vec![];
    for Piece::Poly(v, _) in &pieces { all.extend(v.iter().map(|q| tr(*q))); }
    for (a, b) in &dashes { all.push(tr(*a)); all.push(tr(*b)); }
    for (a, b, ..) in &dims { all.push(tr(*a)); all.push(tr(*b)); }
    let (w, h, m) = match view {
        Some(v) => (v.w, v.h, Map { s: v.s, ox: v.ox, oy: v.oy }),
        None => (480.0, 360.0, Map::fit(&all, 480.0, 360.0, 70.0, 56.0)),
    };
    let mp = |q: Pt| m.p(tr(q));
    // screen direction of model direction `u` at model point `q`
    let sdir = |q: Pt, u: Pt| unit_v(mp(q), mp((q.0 + u.0, q.1 + u.1)));
    let mut d = new_doc(w, h);
    let fill_s = colp(p, "fill_color", TINT);
    let fill = if p.b("shade") { fill_s.as_str() } else { "#fff" };
    // angle wedges: radius of the shading and of the k-th congruence arc (screen px)
    let arc_r = |k: u8| 16.0 + 5.0 * k as f64;
    let shade_r = |arcs: u8| arc_r(arcs.max(1) - 1) + 9.0;
    let wedge = |g: &cs::AngleGeo, r: f64| -> Vec<Pt> {
        let c = mp(g.v);
        let a1 = sdir(g.v, g.r1);
        let f1 = a1.1.atan2(a1.0);
        let n = ((g.deg / 6.0).ceil() as usize).max(2);
        // counter-clockwise in the model is clockwise on screen (y down)
        (0..=n).map(|i| { let t = f1 - (g.deg * i as f64 / n as f64).to_radians(); (c.0 + r * t.cos(), c.1 + r * t.sin()) }).collect()
    };
    // exterior wedges sit outside the shape: under its outline
    for (g, ext, arcs, _, sh, ..) in &angles {
        if let (true, Some(f)) = (ext, sh) { let mut pts = vec![mp(g.v)]; pts.extend(wedge(g, shade_r(*arcs))); d.poly(&pts, f, "none", 0.0); }
    }
    // the outline: a thick stroke under the fills shows only on the outside, so sides shared
    // by two pieces vanish. With side lines set, each outline side is drawn on its own instead.
    let sides = if styled { cs::outer_sides(&items) } else { vec![] };
    let style_of = |s: &cs::Side| cs::side_style(&items, s.a, s.b);
    if !styled {
        for Piece::Poly(v, _) in &pieces { d.poly(&v.iter().map(|q| mp(*q)).collect::<Vec<_>>(), INK, INK, 5.0); }
    } else {
        for it in items.iter().filter(|it| it.is_piece()) {
            let Some(sh) = &it.shape else { continue };
            for run in cs::curved_edges(sh) { d.polyline(&run.iter().map(|q| mp(*q)).collect::<Vec<_>>(), INK, 5.0); }
        }
        for s in &sides {
            if style_of(s) == ("solid".to_string(), "regular".to_string()) { let (a, b) = (mp(s.a), mp(s.b)); d.line(a.0, a.1, b.0, b.1, INK, 5.0); }
        }
    }
    for Piece::Poly(v, sh) in &pieces { let f = sh.unwrap_or(fill); d.poly(&v.iter().map(|q| mp(*q)).collect::<Vec<_>>(), f, f, 1.2); }
    for (g, ext, arcs, _, sh, ..) in &angles {
        if let (false, Some(f)) = (ext, sh) { let mut pts = vec![mp(g.v)]; pts.extend(wedge(g, shade_r(*arcs))); d.poly(&pts, f, "none", 0.0); }
    }
    // styled sides on top, centred on the side
    for s in &sides {
        let (st, th) = style_of(s);
        if st == "hidden" || (st == "solid" && th == "regular") { continue; }
        let sw = match th.as_str() { "thin" => 1.4, "thick" => 4.5, _ => 2.5 };
        let (a, b) = (mp(s.a), mp(s.b));
        match st.as_str() {
            "dashed" => dline(&mut d, a, b, INK, sw, &format!("{:.1} {:.1}", sw * 3.0 + 4.0, sw * 2.0 + 3.0)),
            "dotted" => dline(&mut d, a, b, INK, sw, &format!("0.01 {:.1}", sw * 2.0 + 3.5)),
            _ => { d.line(a.0, a.1, b.0, b.1, INK, sw); }
        }
    }
    for (a, b) in &dashes { dline(&mut d, mp(*a), mp(*b), "#444", 1.6, "6 4"); }
    for (a, dirs) in &rights {
        let e1 = sdir(*a, (if dirs.contains('W') { -1.0 } else { 1.0 }, 0.0));
        let e2 = sdir(*a, (0.0, if dirs.contains('S') { -1.0 } else { 1.0 }));
        let v = mp(*a);
        d.polyline(&[(v.0 + e1.0 * 12.0, v.1 + e1.1 * 12.0), (v.0 + e1.0 * 12.0 + e2.0 * 12.0, v.1 + e1.1 * 12.0 + e2.1 * 12.0), (v.0 + e2.0 * 12.0, v.1 + e2.1 * 12.0)], INK, 1.3);
    }
    // congruence ticks / parallel arrows across the middle of a side
    for (a, b, k, arrows) in &marks {
        let (pa, pb) = (mp(*a), mp(*b));
        let u = unit_v(pa, pb);
        let nn = (-u.1, u.0);
        let c = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
        for j in 0..*k {
            let o = (j as f64 - (*k as f64 - 1.0) / 2.0) * if *arrows { 8.0 } else { 6.0 };
            let q = (c.0 + u.0 * o, c.1 + u.1 * o);
            if *arrows {
                let (tip, back) = ((q.0 + u.0 * 3.0, q.1 + u.1 * 3.0), (q.0 - u.0 * 3.0, q.1 - u.1 * 3.0));
                d.polyline(&[(back.0 + nn.0 * 5.0, back.1 + nn.1 * 5.0), tip, (back.0 - nn.0 * 5.0, back.1 - nn.1 * 5.0)], INK, 1.6);
            } else {
                d.line(q.0 + nn.0 * 6.0, q.1 + nn.1 * 6.0, q.0 - nn.0 * 6.0, q.1 - nn.1 * 6.0, INK, 1.6);
            }
        }
    }
    let lsz = 15.0 * fscale(p);
    // a dragged label sits `off` model units (screen axes) from its own place
    let sh_off = |o: Pt| { let t = cs::turn_off(o, turn); (t.0 * m.s, t.1 * m.s) };
    // angle marks: the exterior angle's extended side, arcs or the right-angle square, the label
    for (g, _, arcs, square, sh, lab, ix, off) in &angles {
        let c = mp(g.v);
        let reach = if sh.is_some() { shade_r(*arcs) } else if *arcs > 0 { arc_r(*arcs - 1) } else if *square { 12.0 } else { 14.0 };
        if let Some(t) = g.ext_ray {
            let u = sdir(g.v, t);
            dline(&mut d, c, (c.0 + u.0 * (reach + 16.0), c.1 + u.1 * (reach + 16.0)), "#444", 1.3, "5 4");
        }
        if *square {
            let (u1, u2) = (sdir(g.v, g.r1), sdir(g.v, g.r2));
            d.polyline(&[(c.0 + u1.0 * 12.0, c.1 + u1.1 * 12.0), (c.0 + u1.0 * 12.0 + u2.0 * 12.0, c.1 + u1.1 * 12.0 + u2.1 * 12.0), (c.0 + u2.0 * 12.0, c.1 + u2.1 * 12.0)], INK, 1.3);
        } else {
            for k in 0..*arcs { d.polyline(&wedge(g, arc_r(k)), INK, 1.4); }
        }
        if !lab.is_empty() {
            let mid = { let a1 = sdir(g.v, g.r1); let t = a1.1.atan2(a1.0) - (g.deg / 2.0).to_radians(); (t.cos(), t.sin()) };
            let o = reach + 8.0 + 0.5 * (tw(lab, lsz) * 0.8 * mid.0.abs() + lsz * mid.1.abs());
            let so = sh_off(*off);
            let q = (c.0 + mid.0 * o + so.0, c.1 + mid.1 * o + so.1);
            mlabel(&mut d, q.0, q.1, lsz, lab, "");
            boxes.push(LabelBox { item: *ix, c: q, half: (tw(lab, lsz) * 0.5 + 3.0, lsz * 0.6 + 2.0) });
        }
    }
    // units typed in a label stay upright: the trailing word after a number
    let unit_of = |t: &str| -> String {
        let w: Vec<&str> = t.split_whitespace().collect();
        if w.len() >= 2 && w[w.len() - 2].chars().any(|c| c.is_ascii_digit()) { w[w.len() - 1].to_string() } else { p.s("unit").to_string() }
    };
    for (a, b, t, ix, doff) in &dims {
        let (pa, pb) = (mp(*a), mp(*b));
        let u = unit_v(pa, pb);
        let right = (-u.1, u.0);
        let off = 16.0;
        let (qa, qb) = ((pa.0 + right.0 * off, pa.1 + right.1 * off), (pb.0 + right.0 * off, pb.1 + right.1 * off));
        d.line(qa.0, qa.1, qb.0, qb.1, GREY, 1.3);
        for q in [qa, qb] { d.line(q.0 - right.0 * 5.0, q.1 - right.1 * 5.0, q.0 + right.0 * 5.0, q.1 + right.1 * 5.0, GREY, 1.3); }
        let mid = ((qa.0 + qb.0) / 2.0, (qa.1 + qb.1) / 2.0);
        // a dim with no text shows its measured length
        let t = if t.trim().is_empty() { fmtu(dist(*a, *b), p.i("dec"), p.s("unit")) } else { t.clone() };
        let o = 10.0 + 0.5 * (tw(&t, lsz) * 0.85 * right.0.abs() + lsz * right.1.abs());
        let so = sh_off(*doff);
        let q = (mid.0 + right.0 * o + so.0, mid.1 + right.1 * o + so.1);
        mlabel(&mut d, q.0, q.1, lsz, &t, &unit_of(&t));
        boxes.push(LabelBox { item: *ix, c: q, half: (tw(&t, lsz) * 0.5 + 3.0, lsz * 0.6 + 2.0) });
    }
    for (a, t, mv) in texts.iter().chain(&labels) {
        let so = mv.map(|(_, o)| sh_off(o)).unwrap_or((0.0, 0.0));
        let q = mp(*a);
        let q = (q.0 + so.0, q.1 + so.1);
        mlabel(&mut d, q.0, q.1, lsz, t, &unit_of(t));
        if let Some((ix, _)) = mv { boxes.push(LabelBox { item: *ix, c: q, half: (tw(t, lsz) * 0.5 + 3.0, lsz * 0.6 + 2.0) }); }
    }
    // corner points, named away from the shape
    for (a, n, ix, off) in &points {
        let c = mp(*a);
        d.circle(c.0, c.1, 4.5, INK, "none", 0.0);
        if !n.is_empty() {
            let u = sdir(*a, cs::outside_dir(&items, *a));
            let o = 12.0 + 0.5 * (tw(n, lsz * 1.2) * 0.8 * u.0.abs() + lsz * 1.2 * u.1.abs());
            let so = sh_off(*off);
            let q = (c.0 + u.0 * o + so.0, c.1 + u.1 * o + so.1);
            mlabel(&mut d, q.0, q.1, lsz * 1.2, n, "");
            boxes.push(LabelBox { item: *ix, c: q, half: (tw(n, lsz * 1.2) * 0.5 + 3.0, lsz * 0.72 + 2.0) });
        }
    }
    let svg = d.finish();
    let svg = if view.is_some() { svg.replacen(r##"<rect width="100%" height="100%" fill="#fff"/>"##, "", 1) } else { svg };
    (svg, boxes)
}

/// "Start from an example" entries for math-a's flat-shape figures (mirrors mathfigures.com).
fn examples_a(id: &str) -> Option<Vec<super::Example>> {
    use Value::{I, S};
    let s = |v: &str| S(v.to_string());
    let ex = |name: &'static str, params: Vec<(&'static str, Value)>| super::Example { name, params };
    let v = match id {
        "parallel-lines" => vec![
            ex("Corresponding angles", vec![("marked", s("2, 6")), ("unknown", s("6"))]),
            ex("Alternate interior angles", vec![("marked", s("4, 6")), ("unknown", s("6"))]),
            ex("Same-side interior angles with expressions", vec![("marked", s("3, 6")), ("unknown", s("")), ("angle_text", s("3 = 2x + 10; 6 = 3x - 5"))]),
            ex("All eight angles numbered", vec![("labels", s("Numbers")), ("marked", s("1, 2, 3, 4, 5, 6, 7, 8"))]),
            ex("Two transversals", vec![("transversals", I(2)), ("marked", s("2, 10")), ("unknown", s("10"))]),
        ],
        "composite-shape" => vec![
            // examples load the pieces into the Pieces card (preset stays Custom) so they can be edited
            ex("L-shape for area", vec![("preset", s(COMPOSITE_CUSTOM)), ("spec", s(preset_spec("L-shape").unwrap_or("")))]),
            ex("House: rectangle and triangle", vec![("preset", s(COMPOSITE_CUSTOM)), ("spec", s(preset_spec("House").unwrap_or("")))]),
            ex("Arch: rectangle and semicircle", vec![("preset", s(COMPOSITE_CUSTOM)), ("spec", s(preset_spec("Arch").unwrap_or("")))]),
            ex("T-shape", vec![("preset", s(COMPOSITE_CUSTOM)), ("spec", s(preset_spec("T-shape").unwrap_or("")))]),
            ex("Semicircle", vec![("preset", s(COMPOSITE_CUSTOM)), ("spec", s(preset_spec("Semicircle").unwrap_or(""))), ("unit", s("cm"))]),
            ex("Shaded pieces with marks", vec![("preset", s(COMPOSITE_CUSTOM)), ("shade", Value::B(false)), ("spec", s("rect 0 0 6 4 blue; tri 6 0 9 0 6 4 yellow; quarter 0 4 2 ne green; dim 0 0 6 0; dim 6 0 9 0; dim 0 6 0 0; arrows 0 0 6 0 1; arrows 2 4 6 4 1; ra 0 0 NE"))]),
        ],
        _ => return None,
    };
    Some(v)
}

// ───────────────────────── registry ─────────────────────────

/// Direct-manipulation handles for this subject's figures (None = not one of ours).
/// Renderer lookup for the interaction layer (`math_geo`, `math_plot`).
pub(super) fn render_for(id: &str) -> Option<fn(&Params) -> String> {
    Some(match id {
        "triangle" => triangle_render,
        "rectangle" => rectangle_render,
        "parallelogram" => parallelogram_render,
        "trapezoid" => trapezoid_render,
        "kite" => kite_render,
        "regular-polygon" => regular_polygon_render,
        "prism" => prism_render,
        "pyramid" => pyramid_render,
        "cylinder" => cylinder_render,
        "cone" => cone_render,
        "sphere" => sphere_render,
        _ => return None,
    })
}

/// "Start from an example" entries for this subject's figures. Solids and plots (math-b) live in
/// `examples_b`; flat shapes (math-a) chain their own `examples_a` here with `.or_else`.
pub fn examples(id: &str) -> Option<Vec<super::Example>> {
    flat::examples(id).or_else(|| examples_a(id)).or_else(|| examples_b(id))
}

/// The Measures card of the figure (none yet): rows for the shape's sides/angles/dimensions.
pub fn measures(id: &str) -> Option<super::MeasureCard> {
    use super::{MeasureCard, MeasureGroup, MeasureRow};
    let row = |name, value_key, label_key, text_key| MeasureRow { name, name_key: None, value_key, unit: "$unit", label_key, text_key, tick_key: None, equals: None };
    match id {
        // solids (meas-b): Shape card rows
        "cylinder" | "cone" | "sphere" | "pyramid" | "prism" => measures_solid(id),
        // flat shapes, angles and parallel lines (meas-a)
        "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" | "regular-polygon" | "angles" | "parallel-lines" => flat::card(id),
        // the reference's INPUTS AND OUTPUTS card (its function check shows under it)
        "mapping-diagram" => Some(MeasureCard {
            title: "Inputs and outputs",
            help: "Type or paste each list with commas or one item a line, like 1, 2, 3 or a column copied from a spreadsheet. Items can be numbers, math like 2x or 1/2, or words. Each side shows an item once, in the order typed.",
            groups: vec![MeasureGroup {
                title: "",
                rows: vec![
                    MeasureRow { unit: "", ..row("Inputs", "inputs", "", "") },
                    MeasureRow { unit: "", ..row("Outputs", "outputs", "", "") },
                ],
            }],
        }),
        _ => {
            let _ = (row, MeasureCard { title: "", help: "", groups: Vec::<MeasureGroup>::new() });
            None
        }
    }
}

/// The rows card (equations / data sets) of a figure, if it has one.
pub fn rows(id: &str) -> Option<super::RowsCard> {
    use super::{RowSlot, RowsCard, RowsSource};
    let slot = |value_key, name_key, color_key| RowSlot { value_key, name_key, color_key };
    Some(match id {
        "coordinate-grid" => RowsCard {
            title: "Equations",
            help: "One per row: y = 2x + 1; 2x + 3y = 6; x = 4; y > x - 2; -2 < x <= 3; y = x + 2, x < 1 (a piece with a domain). Circles and conics work too: x^2 + y^2 = 25; (x-2)^2 + (y+1)^2 = 9; x^2/16 + y^2/9 = 1; x^2/9 - y^2/4 = 1; x = y^2. Use < or > to shade (x^2 + y^2 < 25; several of them shade only where all hold; 4 < x^2 + y^2 < 25 is a ring), end a row with dashed, dotted or solid to change its line (x^2 + y^2 = 25 dashed), end a straight line with asymptote to draw it fine and dotted (y = 2x/3 asymptote; lines that are asymptotes of a curve are recognised anyway), write numbers such as 2e3 for 2000 (Euler's number is e, or 2*e^3), and a limit after a comma to keep part of a curve (x^2 + y^2 = 25, y > 0). Up to 4 curves are drawn; the editor lists any it could not draw.",
            hint: "y = 2x + 1   or   x^2 + y^2 = 25",
            add_label: "Add equation",
            source: RowsSource::Joined { key: "equations", sep: ";", color_keys: vec!["eq1_color", "eq2_color", "eq3_color", "eq4_color"] },
        },
        "number-line" => RowsCard {
            title: "Equations",
            help: "An inequality such as -2 < x <= 5, or x < -1 or x >= 3. Leave empty to draw no graph.",
            hint: "-2 < x <= 5",
            add_label: "Add equation",
            source: RowsSource::Slots(vec![slot("inequality", None, Some("ineq_color")), slot("inequality2", None, Some("ineq2_color"))]),
        },
        // one row per arrow; "Match in order" / "Clear arrows" are the chips under it
        "mapping-diagram" => RowsCard {
            title: "Arrows",
            help: "One arrow a row, from an input to an output: 1 -> 3 or (1, 3). An input can go to more than one output, and an output can come from more than one input. Leave the arrows off for students to draw.",
            hint: "1 -> 3",
            add_label: "Add an arrow",
            // no colours; the empty keys only lift the card's 8-row limit for longer lists
            source: RowsSource::Joined { key: "pairs", sep: ";", color_keys: vec![""; 24] },
        },
        "composite-shape" => RowsCard {
            title: "Pieces",
            help: COMPOSITE_HELP,
            hint: "rect 0 0 6 4",
            add_label: "Add a piece or mark",
            source: RowsSource::Joined { key: "spec", sep: ";", color_keys: vec![""; 40] },
        },
        "box-plot" => RowsCard {
            title: "Data",
            help: "Type the numbers of each data set, separated by commas. Name a set to label its box.",
            hint: "62, 68, 70, 75, 80",
            add_label: "Add a data set",
            source: RowsSource::Slots(vec![slot("data", Some("name1"), None), slot("data2", Some("name2"), None), slot("data3", Some("name3"), None), slot("data4", Some("name4"), None)]),
        },
        _ => return None,
    })
}

/// Visibility of a card group / row for the current params (default: everything is shown).
pub fn card_visible(id: &str, group: &str, row: Option<&str>, p: &Params) -> bool {
    match (id, group, row) {
        // solids: the Oblique rows only for an oblique solid, the base rows only for that base
        (_, "Oblique", _) => p.s("stands") == "Oblique",
        ("prism", "Rectangle or triangle base", Some("hyp")) => p.s("base").contains("triangle"),
        ("prism", "Rectangle or triangle base", _) | ("pyramid", "Rectangle base", _) => p.s("base") != "Regular polygon",
        ("prism" | "pyramid", "Regular polygon base", _) => p.s("base") == "Regular polygon",
        // "Give the diameter instead" swaps the Radius row for a Diameter row
        ("cylinder" | "cone" | "sphere", _, Some("radius" | "diameter")) => !p.b("give_d"),
        ("cylinder" | "cone" | "sphere", _, Some("diam")) => p.b("give_d"),
        // angles: Baseline and Ray 2, then one more row per "Add ray" (a typed row always shows)
        ("angles", _, Some(k)) if k != "ray1" => {
            let i = flat::RAY_V.iter().position(|r| *r == k).unwrap_or(0);
            i < p.i("ray_count").clamp(2, 6) as usize || !p.s(k).trim().is_empty()
        }
        // parallel lines: the third line / second transversal rows once added
        ("parallel-lines", _, Some("p_line")) => p.i("lines") >= 3,
        ("parallel-lines", _, Some("angle_b")) => p.i("transversals") >= 2,
        _ => true,
    }
}

/// (value_key, text) for every measure the figure worked out from the typed ones.
pub fn computed(id: &str, p: &Params) -> Vec<(&'static str, String)> {
    match id {
        "cylinder" | "cone" | "sphere" | "pyramid" | "prism" => computed_solid(id, p),
        "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" | "regular-polygon" | "angles" | "parallel-lines" => flat::computed(id, p).unwrap_or_default(),
        "mapping-diagram" => mapping_msg(p).map(|m| vec![("__msg", m)]).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Maps a saved spec's legacy keys onto the reworked inputs.
pub fn migrate(id: &str, spec: &crate::model::FigureSpec, p: &mut Params) {
    match id {
        "cylinder" | "cone" | "sphere" | "pyramid" | "prism" => migrate_solid(id, spec, p),
        "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" | "regular-polygon" | "angles" | "parallel-lines" => flat::migrate(id, spec, p),
        // the reference calls the rectangle sides "Boxes"
        "mapping-diagram" => if spec.params.get("shape").and_then(|v| v.as_str()) == Some("Rectangles") { p.set_s("shape", "Boxes"); },
        _ => {}
    }
}

/// "Add ..." chips / action buttons for this subject's figures.
pub fn chips(id: &str) -> Option<super::ChipsCard> {
    use super::{Chip, ChipsCard};
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        // a new ray row (cleared, so it is drawn in the widest gap until a direction is typed)
        "angles" => ChipsCard { title: "Add a ray", count_key: "ray_count", max: 6, chips: vec![Chip { label: "Add ray", set: vec![("ray{n}", s(""))] }] },
        "parallel-lines" => ChipsCard {
            title: "Add a line",
            count_key: "",
            max: 0,
            chips: vec![
                Chip { label: "Add parallel line", set: vec![("lines", Value::I(3))] },
                Chip { label: "Add transversal", set: vec![("transversals", Value::I(2))] },
            ],
        },
        "mapping-diagram" => ChipsCard {
            title: "Quick arrows",
            count_key: "",
            max: 0,
            chips: vec![
                Chip { label: "Match in order", set: vec![("pairs", s(MATCH_IN_ORDER))] },
                Chip { label: "Clear arrows", set: vec![("pairs", s(""))] },
            ],
        },
        "composite-shape" => ChipsCard {
            title: "Start with",
            count_key: "",
            max: 0,
            chips: COMPOSITE_STARTS
                .iter()
                .map(|&name| Chip { label: name, set: vec![("preset", s(COMPOSITE_CUSTOM)), ("spec", s(preset_spec(name).unwrap_or("")))] })
                .collect(),
        },
        _ => return None,
    })
}

pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<interact::Handle>> {
    super::math_plot::handles(id, p, size).or_else(|| super::math_geo::handles(id, p, size)).or_else(|| super::circle::handles(id, p, size)).or_else(|| super::trig::handles(id, p, size))
}

/// Solids' "Shading" choice: Tinted (the shaded faces drawn so far) or White, the reference's
/// black-and-white look. Saved solids without the key keep their tint.
fn with_solid_fill(mut v: Vec<ParamDef>) -> Vec<ParamDef> {
    v.push(ParamDef::choice("fill", "Shading", &["Tinted", "White"], "Tinted"));
    v
}

/// White shading: every light tint (the face and cap fills, all channels >= 0xc0) becomes white.
/// Ink, dashed hidden edges, black text and white label halos are left as drawn.
fn solid_fill(svg: String, p: &Params) -> String {
    if p.s("fill") != "White" {
        return svg;
    }
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg.as_str();
    while let Some(i) = rest.find("=\"#") {
        let (head, tail) = rest.split_at(i + 3);
        out.push_str(head);
        let hex = tail.get(..6).filter(|h| h.chars().all(|c| c.is_ascii_hexdigit()) && tail[6..].starts_with('"'));
        let light = hex.is_some_and(|h| (0..3).all(|k| u8::from_str_radix(&h[k * 2..k * 2 + 2], 16).is_ok_and(|c| c >= 0xc0)));
        match hex {
            Some(_) if light => out.push_str("ffffff"),
            Some(h) => out.push_str(h),
            None => {}
        }
        rest = if hex.is_some() { &tail[6..] } else { tail };
    }
    out.push_str(rest);
    out
}

pub fn defs() -> Vec<FigureDef> {
    let f = |id, title, blurb, params: fn() -> Vec<ParamDef>, render: fn(&Params) -> String| FigureDef { id, subject: Subject::Math, title, blurb, params, render };
    vec![
        f("coordinate-grid", "Coordinate Grid", "Graph paper with numbered axes, minor gridlines, plotted equations and labelled points.", coordinate_grid_params, coordinate_grid_render),
        f("composite-shape", "Composite Shape", "Build a compound figure from rectangles, triangles, circles and semicircles, with dimension marks.", composite_params, composite_render),
        f("number-line", "Number Line", "A number line with adjustable range and ticks, graphed inequalities and marked points.", number_line_params, number_line_render),
        f("triangle", "Triangle", "A triangle drawn to scale from sides, angles or right-triangle legs, with optional height.", || super::labels::with_param(triangle_params()), |p| super::labels::rendering(p, || triangle_render(p))),
        f("rectangle", "Rectangle", "A rectangle or square drawn to scale with labelled sides and optional diagonals.", || super::labels::with_param(rectangle_params()), |p| super::labels::rendering(p, || rectangle_render(p))),
        f("parallelogram", "Parallelogram", "A parallelogram or rhombus from its sides and an angle, with height and diagonals.", || super::labels::with_param(parallelogram_params()), |p| super::labels::rendering(p, || parallelogram_render(p))),
        f("trapezoid", "Trapezoid", "A plain, isosceles or right trapezoid from its bases and height.", || super::labels::with_param(trapezoid_params()), |p| super::labels::rendering(p, || trapezoid_render(p))),
        f("kite", "Kite", "A kite from its two side lengths and the angle between the short sides.", || super::labels::with_param(kite_params()), |p| super::labels::rendering(p, || kite_render(p))),
        f("regular-polygon", "Regular Polygon", "Any regular polygon from 3 to 20 sides, sized by side, radius or apothem.", || super::labels::with_param(regular_polygon_params()), |p| super::labels::rendering(p, || regular_polygon_render(p))),
        f("angles", "Angles", "Single, adjacent, straight-line, around-a-point or vertical angles with measures.", angles_params, angles_render),
        f("parallel-lines", "Parallel Lines and Transversal", "Parallel lines cut by a transversal with chosen angles marked in degrees or by number.", parallel_lines_params, parallel_lines_render),
        f("prism", "Prism", "A prism on a rectangle, triangle or regular-polygon base, right or oblique, with dashed hidden edges.", || super::labels::with_param(with_solid_fill(prism_params())), |p| super::labels::rendering(p, || solid_fill(prism_render(p), p))),
        f("cylinder", "Cylinder", "A cylinder, right or oblique, with radius or diameter and height.", || super::labels::with_param(with_solid_fill(cylinder_params())), |p| super::labels::rendering(p, || solid_fill(cylinder_render(p), p))),
        f("pyramid", "Pyramid", "A pyramid on a rectangle or regular-polygon base with optional height and slant height.", || super::labels::with_param(with_solid_fill(pyramid_params())), |p| super::labels::rendering(p, || solid_fill(pyramid_render(p), p))),
        f("cone", "Cone", "A cone, right or oblique, with radius or diameter, height and slant height.", || super::labels::with_param(with_solid_fill(cone_params())), |p| super::labels::rendering(p, || solid_fill(cone_render(p), p))),
        f("sphere", "Sphere", "A sphere or hemisphere with its radius or diameter.", || super::labels::with_param(with_solid_fill(sphere_params())), |p| super::labels::rendering(p, || solid_fill(sphere_render(p), p))),
        f("box-plot", "Box Plot", "A box plot from a data list or five-number summary; compare two sets.", box_plot_params, box_plot_render),
        f("mapping-diagram", "Mapping Diagram", "Input and output sets joined by arrows, from ordered pairs.", mapping_diagram_params, mapping_diagram_render),
        f("ruler-reading", "Ruler Reading", "A metric or inch ruler with an object to measure at any position and length.", length_reading_params, length_reading_render),
    ]
}

// ───────────────────────── tests ─────────────────────────

#[cfg(test)]
mod solid_fill_tests {
    use super::*;

    /// Solids are tinted by default (saved figures unchanged) and white on request.
    #[test]
    fn solids_white_shading() {
        for id in ["prism", "cylinder", "pyramid", "cone", "sphere"] {
            let d = defs().into_iter().find(|d| d.id == id).unwrap();
            let mut p = Params::defaults(&(d.params)());
            assert_eq!(p.s("fill"), "Tinted", "{id}");
            let tinted = (d.render)(&p);
            p.set_s("fill", "White");
            let white = (d.render)(&p);
            assert_ne!(tinted, white, "{id}: white shading changed nothing");
            for h in ["#d9e7f6", "#e3eef9", "#f4f9ff"] {
                assert!(!white.contains(h), "{id}: {h} left in white shading");
            }
            assert!(white.contains(INK), "{id}: edges kept");
            to_png(&white, 0.5).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_svg(set: &[(&str, Value)]) -> String {
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in set { assert!(p.0.contains_key(*k), "unknown key {k}"); p.0.insert(k.to_string(), v.clone()); }
        // like the UI: params are clamped before any render
        p.normalize(&(d.params)(), false);
        (d.render)(&p)
    }
    fn q(eq: &str) -> Vec<(&'static str, Value)> {
        vec![("xmin", Value::F(-10.0)), ("xmax", Value::F(10.0)), ("ymin", Value::F(-10.0)), ("ymax", Value::F(10.0)), ("equations", Value::S(eq.to_string()))]
    }
    fn curve_lines(svg: &str) -> Vec<Vec<(f64, f64)>> { curve_lines_min(svg, 8) }
    fn curve_lines_min(svg: &str, min: usize) -> Vec<Vec<(f64, f64)>> {
        svg.split("<polyline points=\"").skip(1).filter_map(|t| t.split('"').next()).map(|pts| {
            pts.split(' ').filter_map(|c| { let (a, b) = c.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect::<Vec<(f64, f64)>>()
        }).filter(|l: &Vec<(f64, f64)>| l.len() > min).collect()
    }

    #[test]
    fn conic_rows_draw_smooth_curves() {
        // circle: every drawn point within 1 px of the true radius (24 px a unit at 20 units / 480 px)
        let svg = grid_svg(&q("x^2 + y^2 = 25"));
        let ls = curve_lines(&svg);
        assert_eq!(ls.len(), 1, "one closed circle");
        let g = grid_layout(&{ let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap(); let mut p = Params::defaults(&(d.params)()); for (k, v) in q("") { p.0.insert(k.to_string(), v); } p });
        let (cx, cy) = (g.ml + 10.0 * g.ux, g.mt + 10.0 * g.uy);
        for &(x, y) in &ls[0] { assert!(((x - cx).hypot(y - cy) - 5.0 * g.ux).abs() < 1.0, "off the circle"); }
        // shifted circle, ellipse, x = y^2, y^2 = x, hyperbola (two branches)
        assert_eq!(curve_lines(&grid_svg(&q("(x-2)^2 + (y+1)^2 = 9"))).len(), 1);
        assert_eq!(curve_lines(&grid_svg(&q("x^2/16 + y^2/9 = 1"))).len(), 1);
        assert_eq!(curve_lines(&grid_svg(&q("x = y^2"))).len(), 1);
        assert_eq!(curve_lines(&grid_svg(&q("y^2 = x"))).len(), 1);
        assert_eq!(curve_lines(&grid_svg(&q("x^2/9 - y^2/4 = 1"))).len(), 2);
        // general F = G
        assert_eq!(curve_lines(&grid_svg(&q("x^2 + y^2 = 2x + 4y"))).len(), 1);
        // the old linear and explicit rows are still the old code path (no marching squares polyline of 20+ points beyond the function)
        assert_eq!(curve_lines(&grid_svg(&q("2x + 3y = 6"))).len(), 1);
        assert_eq!(curve_lines(&grid_svg(&q("xy = 6"))).len(), 2);
    }

    #[test]
    fn empty_conics_and_odd_windows_do_not_panic() {
        let svg = grid_svg(&q("x^2 + y^2 = -1"));
        assert!(curve_lines(&svg).is_empty());
        for eq in ["x^2 + y^2 =", "y^2 = ", "x = y^", "sqrt(x) + sqrt(y) = 1", "tan(y) = x", "1/(x^2+y^2) = 1", "x^2 + y^2 = 1/0", "ln(x) = ln(y)^2", "y^2 = x^2; y^2 = 2x; x^2 = 3y; x^2+y^2 = 4; x^2 = y^2 + 1"] {
            let _ = grid_svg(&q(eq));
        }
        for (a, b) in [(1e300, 1e300), (-1000.0, 1000.0), (0.0, 1e-300), (-1000.0, -999.0)] {
            let _ = grid_svg(&[("xmin", Value::F(a)), ("xmax", Value::F(b)), ("ymin", Value::F(a)), ("ymax", Value::F(b)), ("equations", Value::S("x^2 + y^2 = 25; x = y^2; x^2/9 - y^2/4 = 1; sin(x*y) = 0".into()))]);
        }
    }

    #[test]
    fn conic_rendering_is_time_bounded() {
        let t = std::time::Instant::now();
        let svg = grid_svg(&q("x^2 + y^2 = 25; x^2/16 + y^2/9 = 1; x^2/9 - y^2/4 = 1; sin(x) * cos(y) = x * y / 50; x^2 + y^2 = 1"));
        assert!(!curve_lines(&svg).is_empty());
        let ms = t.elapsed().as_millis();
        // generous: a guard against a runaway loop (seconds), not a benchmark that a busy machine could trip
        let limit = if cfg!(debug_assertions) { 120_000 } else { 15_000 };
        assert!(ms < limit, "took {ms} ms");
    }

    fn grid_layout_of(set: &[(&str, Value)]) -> GridL {
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in set { p.0.insert(k.to_string(), v.clone()); }
        grid_layout(&p)
    }
    /// (text x, text y, content) of every halo label.
    fn halo_labels(svg: &str) -> Vec<(f64, f64, String)> {
        svg.split("<text ").skip(1).filter(|t| t.contains("paint-order") && (t.contains("font-size=\"15\"") || t.contains("font-size=\"16.5\""))).filter_map(|t| {
            let num = |k: &str| -> Option<f64> { let i = t.find(&format!("{k}=\""))? + k.len() + 2; t[i..].split('"').next()?.parse().ok() };
            let body = t.split_once('>')?.1.split("</text").next()?.to_string();
            Some((num("x")?, num("y")?, body))
        }).collect()
    }

    #[test]
    fn conic_rows_keep_label_domain_and_colour() {
        let mut set = q("x^2 + y^2 = 25, x > 0");
        set.push(("eq_labels", Value::B(true)));
        let svg = grid_svg(&set);
        assert!(svg.contains("#1f5fbf"), "equation colour slot");
        let labels = halo_labels(&svg);
        assert_eq!(labels.len(), 1, "one label with a halo");
        assert!(labels[0].2.contains("25"), "label text {}", labels[0].2);
        // the domain keeps only the right half: no point left of the y axis
        let ls = curve_lines(&svg);
        assert!(!ls.is_empty());
        let g = grid_layout_of(&q(""));
        let cx = g.ml + 10.0 * g.ux;
        for l in &ls { for &(x, _) in l { assert!(x >= cx - 0.5, "domain ignored"); } }
    }

    #[test]
    fn y_domains_and_mixed_domains_apply_to_conics() {
        let g = grid_layout_of(&q(""));
        let (cx, cy) = (g.ml + 10.0 * g.ux, g.mt + 10.0 * g.uy);
        // y > 0: the top half (smaller pixel y)
        let ls = curve_lines(&grid_svg(&q("x^2 + y^2 = 25, y > 0")));
        assert_eq!(ls.len(), 1);
        for &(_, y) in &ls[0] { assert!(y <= cy + 0.5, "lower half drawn"); }
        assert!(ls[0].iter().any(|p| p.0 < cx - 3.0 * g.ux) && ls[0].iter().any(|p| p.0 > cx + 3.0 * g.ux), "both sides of the top half");
        // x = y^2, y > 0: one branch only (the upper half of the parabola)
        let ls = curve_lines(&grid_svg(&q("x = y^2, y > 0")));
        assert_eq!(ls.len(), 1, "{:?}", curve_lines_min(&grid_svg(&q("x = y^2, y > 0")), 0).iter().map(|l| l.len()).collect::<Vec<_>>());
        for &(_, y) in &ls[0] { assert!(y <= cy + 0.5); }
        // x > 0 and y > 0, as separate pieces or with "and": the quarter arc
        for eq in ["x^2 + y^2 = 25, x > 0, y > 0", "x^2 + y^2 = 25, x > 0 and y > 0"] {
            let ls = curve_lines(&grid_svg(&q(eq)));
            assert_eq!(ls.len(), 1, "{eq}");
            for &(x, y) in &ls[0] { assert!(x >= cx - 0.5 && y <= cy + 0.5, "{eq}: outside the quadrant"); }
        }
        // the arc ends at the limit (within a pixel), not a cell short
        let ls = curve_lines(&grid_svg(&q("x^2 + y^2 = 25, y > 0")));
        let lowest = ls[0].iter().map(|p| p.1).fold(f64::MIN, f64::max);
        assert!((lowest - cy).abs() < 1.0, "arc stops {} px from the axis", (lowest - cy).abs());
    }

    #[test]
    fn domain_cuts_get_open_and_closed_dots() {
        let count = |svg: &str, fill: &str| svg.matches(&format!("fill=\"{fill}\" stroke=\"#1f5fbf\" stroke-width=\"2\"")).count();
        let open = grid_svg(&q("x^2 + y^2 = 25, y > 0"));
        assert_eq!(count(&open, "#fff"), 2, "two open dots");
        let closed = grid_svg(&q("x^2 + y^2 = 25, y >= 0"));
        assert_eq!(count(&closed, "#1f5fbf"), 2, "two closed dots");
        let mut off = q("x^2 + y^2 = 25, y > 0");
        off.push(("endpoints", Value::B(false)));
        assert_eq!(count(&grid_svg(&off), "#fff"), 0, "endpoints switch");
    }

    #[test]
    fn inequalities_with_a_curved_boundary_are_shaded() {
        let g = grid_layout_of(&q(""));
        let area_of = |svg: &str| -> f64 {
            // the shading path: M x y L x y ... Z, many subpaths
            let i = svg.find("<path d=\"M").expect("a shading path");
            let d = svg[i + 9..].split('"').next().unwrap();
            let mut total = 0.0;
            for sub in d.split('Z').filter(|t| !t.trim().is_empty()) {
                let nums: Vec<f64> = sub.split(|c: char| c == 'M' || c == 'L' || c == ' ').filter_map(|t| t.parse().ok()).collect();
                let pts: Vec<(f64, f64)> = nums.chunks(2).map(|c| (c[0], c[1])).collect();
                total += (0..pts.len()).map(|i| { let (a, b) = (pts[i], pts[(i + 1) % pts.len()]); a.0 * b.1 - b.0 * a.1 }).sum::<f64>().abs() / 2.0;
            }
            total
        };
        let disc = std::f64::consts::PI * 25.0 * g.ux * g.uy;
        let strict = grid_svg(&q("x^2 + y^2 < 25"));
        assert!((area_of(&strict) - disc).abs() < 0.02 * disc, "shaded {} vs {disc}", area_of(&strict));
        assert!(strict.contains("stroke-dasharray=\"9 6\""), "strict: dashed boundary");
        let solid = grid_svg(&q("x^2 + y^2 <= 25"));
        assert!(!solid.contains("stroke-dasharray=\"9 6\""), "non-strict: solid boundary");
        // outside: the window minus the disc
        let out = grid_svg(&q("x^2 + y^2 > 25"));
        assert!((area_of(&out) - (g.pw * g.ph - disc)).abs() < 0.02 * g.pw * g.ph);
        // ellipse, hatching, and the old explicit ineq path is untouched
        assert!(grid_svg(&q("x^2/9 + y^2/4 <= 1")).contains("fill-opacity=\"0.18\""));
        let mut h = q("x^2 + y^2 < 25");
        h.push(("shading", Value::S("Hatching".into())));
        assert!(grid_svg(&h).contains("url(#sh0)"));
        assert!(grid_svg(&q("y > x^2")).contains("<polygon"), "explicit y > f(x) still a polygon, not the cell path");
    }

    fn warns(eq: &str) -> Vec<String> {
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in q(eq) { p.0.insert(k.to_string(), v); }
        grid_warnings(&p)
    }

    #[test]
    fn notes_go_to_the_editor_and_never_into_the_svg() {
        let five = "x^2+y^2=1; x^2+y^2=4; x^2+y^2=9; x^2+y^2=16; x^2+y^2=25";
        assert!(warns(five).iter().any(|w| w.contains("Only 4 curves are drawn")));
        let svg = grid_svg(&q(five));
        for t in ["Only 4 curves", "Cannot read", "too long", "No part of", "Too detailed"] { assert!(!svg.contains(t), "{t} leaked into the SVG"); }
        // rows that draw nothing do not use the drawing budget, but each is an attempt
        let empties = "x^2+y^2=-1; x^2+y^2=-2; x^2+y^2=-3; x^2+y^2=-4; x^2+y^2=25";
        assert!(!warns(empties).iter().any(|w| w.contains("Only 4")));
        assert_eq!(curve_lines(&grid_svg(&q(empties))).len(), 1);
        let nine = (0..9).map(|k| format!("x^2+y^2=-{}", k + 1)).collect::<Vec<_>>().join("; ");
        assert!(warns(&nine).iter().any(|w| w.contains("Only 4")), "at most 8 rows are even tried");
        assert!(warns("x^2 + y^2 = abc").iter().any(|w| w.contains("Cannot read")));
        // outside the window
        assert!(warns("x^2 + y^2 = 400").iter().any(|w| w.contains("No part of this curve is inside the window")));
        assert!(warns("x^2 + y^2 = 25").is_empty());
        // pathological rows: refused, quickly, and what the user typed is what is counted
        let long = format!("x^2 + y^2 = {}", "1+".repeat(95) + "1");
        assert!(long.chars().count() > 200 && warns(&long).iter().any(|w| w.contains("too long")));
        let ok_len = format!("x^2 + y^2 = {}", "1+".repeat(90) + "1");
        assert!(ok_len.chars().count() <= 200 && !warns(&ok_len).iter().any(|w| w.contains("too long")));
        let deep = format!("x^2 + y^2 = {}1{}", "(".repeat(31), ")".repeat(31));
        assert!(warns(&deep).iter().any(|w| w.contains("too long")));
        let fine = format!("x^2 + y^2 = {}25{}", "(".repeat(30), ")".repeat(30));
        assert!(!warns(&fine).iter().any(|w| w.contains("too long")), "30 deep is allowed");
    }

    fn shaded_area(svg: &str) -> f64 {
        let i = svg.find("<path d=\"M").expect("a shading path");
        let d = svg[i + 9..].split('"').next().unwrap();
        let mut total = 0.0;
        for sub in d.split('Z').filter(|t| !t.trim().is_empty()) {
            let nums: Vec<f64> = sub.split(|c: char| c == 'M' || c == 'L' || c == ' ').filter_map(|t| t.parse().ok()).collect();
            let pts: Vec<(f64, f64)> = nums.chunks(2).map(|c| (c[0], c[1])).collect();
            total += (0..pts.len()).map(|i| { let (a, b) = (pts[i], pts[(i + 1) % pts.len()]); a.0 * b.1 - b.0 * a.1 }).sum::<f64>().abs() / 2.0;
        }
        total
    }

    #[test]
    fn absurd_windows_cannot_run_away() {
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        // straight into render, no normalise by the caller: every window below must return quickly
        let t = std::time::Instant::now();
        for (a, b, step) in [(-1e11, 1e11, 1.0), (-1e300, 1e300, 1e-300), (1e9, 1e9 + 15.0, 0.05), (f64::NAN, f64::INFINITY, 1.0), (-1000.0, 1000.0, 0.05)] {
            let mut p = Params::defaults(&(d.params)());
            for k in ["xmin", "ymin"] { p.0.insert(k.into(), Value::F(a)); }
            for k in ["xmax", "ymax"] { p.0.insert(k.into(), Value::F(b)); }
            for k in ["xstep", "ystep"] { p.0.insert(k.into(), Value::F(step)); }
            p.0.insert("minor".into(), Value::I(10));
            p.0.insert("equations".into(), Value::S("y = 2x + 1; x^2 + y^2 = 25".into()));
            let svg = (d.render)(&p);
            assert!(svg.len() < 4_000_000, "{a}..{b}: {} bytes", svg.len());
        }
        assert!(t.elapsed().as_millis() < 10_000, "took {} ms", t.elapsed().as_millis());
        // a JSON integer for a Float control is clamped like any number, and y = 2x + 1 still draws, fast
        let t = std::time::Instant::now();
        let p = crate::figures::params_from_json(&d, &serde_json::json!({"xmin": -100000000000i64, "xmax": 100000000000i64, "equations": "y = 2x + 1"}));
        assert_eq!((p.f("xmin"), p.f("xmax")), (-1000.0, 1000.0));
        let svg = (d.render)(&p);
        // generous: a guard against a runaway loop, not a benchmark a busy machine could trip
        let limit = if cfg!(debug_assertions) { 60_000 } else { 10_000 };
        assert!(svg.contains("<polyline") && t.elapsed().as_millis() < limit, "{} ms", t.elapsed().as_millis());
    }

    #[test]
    fn conic_comma_form_double_roots_and_tiny_circles_draw() {
        let g = grid_layout_of(&q(""));
        // x^2 + y^2 > 4, x^2 + y^2 < 25: the ring, shaded, with its limit read as a condition
        let svg = grid_svg(&q("x^2 + y^2 > 4, x^2 + y^2 < 25"));
        let ring = std::f64::consts::PI * (25.0 - 4.0) * g.ux * g.uy;
        assert!((shaded_area(&svg) - ring).abs() < 0.04 * ring, "ring {} vs {ring}", shaded_area(&svg));
        // double roots are lines
        for eq in ["x^2 = 0", "(x-1)^2 = 0", "y^2 = 0"] {
            assert_eq!(curve_lines_min(&grid_svg(&q(eq)), 1).len(), 1, "{eq}");
        }
        // a radius of 0.02 is drawn at 0.02 (0.48 px on this grid), not a quarter of it
        let svg = grid_svg(&q("x^2 + y^2 = 0.0004"));
        let ls = curve_lines_min(&svg, 1);
        assert_eq!(ls.len(), 1);
        let (cx, cy) = (g.ml + 10.0 * g.ux, g.mt + 10.0 * g.uy);
        let r = ls[0].iter().map(|p| (p.0 - cx).hypot(p.1 - cy)).fold(0.0, f64::max);
        assert!((r - 0.02 * g.ux).abs() < 0.15 * g.ux * 0.02 + 0.1, "radius {r} px vs {}", 0.02 * g.ux);
    }

    #[test]
    fn two_curved_inequalities_shade_the_intersection_once() {
        let g = grid_layout_of(&q(""));
        let svg = grid_svg(&q("x^2 + y^2 > 4; x^2 + y^2 < 25"));
        assert_eq!(svg.matches("<path d=\"M").count(), 1, "one tint, not stacked");
        let ring = std::f64::consts::PI * (25.0 - 4.0) * g.ux * g.uy;
        assert!((shaded_area(&svg) - ring).abs() < 0.03 * ring);
        // boundaries of both rows are still drawn, one dashed each
        let dashed: Vec<usize> = svg.split("<polyline").skip(1).filter(|t| t.contains("9 6")).map(|t| t.len()).collect();
        assert_eq!(dashed.len(), 2, "dashed polylines {dashed:?}");
        // a single non-linear inequality and linear inequalities are untouched
        assert_eq!(grid_svg(&q("y > x - 2; y <= -x + 3")).split("<polygon").filter(|t| t.contains("fill-opacity")).count(), 2);
    }

    #[test]
    fn busy_curves_are_capped_in_size_and_time() {
        let (t, c) = (std::time::Instant::now(), thread_cpu_ms());
        let eq = "sin(40x)*sin(40y) < 0; sin(40x)*sin(40y) > 0.5; sin(40x+y) < 0; sin(30x)*sin(30y) < 0.2";
        let svg = grid_svg(&q(eq));
        assert!(svg.len() < 4_000_000, "{} bytes", svg.len());
        assert!(warns(eq).iter().any(|w| w.contains("Too detailed")));
        // (CPU time of this thread where the system gives it: wall time flakes when the suite runs hundreds of tests at once)
        let ms = match (c, thread_cpu_ms()) { (Some(a), Some(b)) => b - a, _ => t.elapsed().as_millis() };
        let limit = if cfg!(debug_assertions) { 40_000 } else { 8_000 };
        assert!(ms < limit, "{ms} ms");
    }

    /// All the equation labels of a render: (box, text length), and every curve polyline in pixels.
    fn label_boxes_and_curves(svg: &str) -> (Vec<[f64; 4]>, Vec<Vec<(f64, f64)>>) {
        let boxes = svg.split("<text ").skip(1).filter(|t| t.contains("font-size=\"15\"") || t.contains("font-size=\"16.5\"")).filter_map(|t| {
            let t = format!(" {t}");
            let num = |k: &str| -> Option<f64> { let i = t.find(&format!(" {k}=\""))? + k.len() + 3; t[i..].split('"').next()?.parse().ok() };
            let body = t.split_once('>')?.1.split("</text").next()?;
            let mut plain = String::new();
            let mut intag = false;
            for c in body.chars() { match c { '<' => intag = true, '>' => intag = false, _ if !intag => plain.push(c), _ => {} } }
            let w = label_w(&plain);
            let (x, y) = (num("x")?, num("y")? - 5.1);
            Some([x, y - 9.0, x + w, y + 9.0])
        }).collect::<Vec<[f64; 4]>>();
        // (a halo label is two text elements at one place: the white outline, then the letters)
        let mut boxes = boxes;
        boxes.dedup();
        (boxes, curve_lines_min(svg, 1))
    }

    #[test]
    fn labels_never_overprint_or_sit_on_curves() {
        let cases = [
            ("tangent", "x^2 + y^2 = 25; (x-8)^2 + y^2 = 9; y = -x + 7.07"),
            ("concentric", "x^2 + y^2 = 1; x^2 + y^2 = 4; x^2 + y^2 = 9; x^2 + y^2 = 16"),
            ("hyperbola", "x^2/9 - y^2/4 = 1; y = 2x/3; y = -2x/3"),
            ("ellipse+line", "x^2/16 + y^2/9 = 1; y = x/2"),
            ("ineq", "x^2/9 + y^2/4 <= 1; y > x^2 - 6"),
        ];
        for (name, eq) in cases {
            let mut set = q(eq);
            set.push(("eq_labels", Value::B(true)));
            let svg = grid_svg(&set);
            let (boxes, curves) = label_boxes_and_curves(&svg);
            assert_eq!(boxes.len(), eq.split(';').count(), "{name}: every row labelled");
            for (i, a) in boxes.iter().enumerate() {
                for b in &boxes[i + 1..] { assert!(!rect_hit(a, b), "{name}: two labels overprint: {a:?} {b:?}"); }
                assert_eq!(label_conflicts(a, &curves, &[]), 0, "{name}: label {a:?} sits on a curve");
            }
        }
    }

    #[test]
    fn padded_linear_rows_and_deep_strings_are_safe() {
        // (5) a linear row padded far past 200 characters still draws as it did
        let pad = format!("y = 2x + 1{}", " ".repeat(300));
        assert!(grid_svg(&q(&pad)).contains("<polyline"));
        let pad2 = format!("2x + 3y = 6 + {}", "0+".repeat(150) + "0");
        assert!(grid_svg(&q(&pad2)).contains("<polyline"));
        // (4) thousands of nested brackets: no stack overflow in the render or in the drag handles
        let deep = format!("y = {}x{}", "(".repeat(20000), ")".repeat(20000));
        let _ = grid_svg(&q(&deep));
        let deep2 = format!("x^2 + y^2 = {}1{}", "(".repeat(20000), ")".repeat(20000));
        let _ = grid_svg(&q(&deep2));
        assert!(is_curve_row(&deep2, -10.0, 10.0, -10.0, 10.0, false));
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        let mut p = Params::defaults(&(d.params)());
        p.0.insert("equations".into(), Value::S(deep.clone()));
        let _ = crate::figures::math_plot::handles("coordinate-grid", &p, (600.0, 600.0));
        p.0.insert("equations".into(), Value::S(deep2));
        let _ = crate::figures::math_plot::handles("coordinate-grid", &p, (600.0, 600.0));
    }

    #[test]
    fn degenerate_rows_now_draw_and_lines_keep_the_old_path() {
        // x^2 = 4: two vertical lines; (x-1)(y+2) = 0: the vertical line is no longer lost
        assert_eq!(curve_lines_min(&grid_svg(&q("x^2 = 4")), 1).len(), 2);
        let g = grid_layout_of(&q(""));
        let ls = curve_lines_min(&grid_svg(&q("(x-1)(y+2) = 0")), 1);
        let sx1 = g.ml + 11.0 * g.ux;
        assert!(ls.iter().flatten().any(|p| (p.0 - sx1).abs() < 2.0 && p.1 < g.mt + 6.0), "vertical line x = 1 reaches the top");
        assert!(ls.iter().flatten().any(|p| (p.0 - sx1).abs() < 2.0 && p.1 > g.mt + g.ph - 6.0), "and the bottom");
        // linear and explicit rows are still one polyline each
        for eq in ["2x + 3y = 6", "y/x = 2", "2y = x", "y = 2x + 1", "x = 4"] {
            let svg = grid_svg(&q(eq));
            assert!(svg.contains("<polyline") || svg.contains("<line"), "{eq}");
            assert!(!svg.contains("<path d=\"M"), "{eq}: not the cell path");
        }
    }

    #[test]
    fn labels_on_conics_have_a_halo_and_do_not_collide() {
        // two tangent circles, a hyperbola with its asymptotes, with labels on
        for eq in ["x^2 + y^2 = 25; (x-8)^2 + y^2 = 9; (x+8)^2 + y^2 = 9", "x^2/9 - y^2/4 = 1; y = 2x/3; y = -2x/3", "x^2 + y^2 = 4; x^2 + y^2 = 9; x^2 + y^2 = 16; x^2 + y^2 = 25"] {
            let mut set = q(eq);
            set.push(("eq_labels", Value::B(true)));
            set.push(("points", Value::S("A(1, 1), B(-2, 3)".into())));
            let svg = grid_svg(&set);
            let ls = halo_labels(&svg);
            assert!(!ls.is_empty(), "{eq}: labels with a halo");
            let rect = |l: &(f64, f64, String)| [l.0, l.1 - 9.0, l.0 + 7.2 * l.2.matches("<tspan").count().max(1) as f64 * 0.0 + 30.0, l.1 + 9.0];
            for (i, a) in ls.iter().enumerate() { for b in &ls[i + 1..] {
                assert!(!rect_hit(&rect(a), &rect(b)), "{eq}: labels collide: {:?} {:?}", (a.0, a.1), (b.0, b.1));
            } }
        }
        // a curve with no point in the central 6-94%: still labelled (a circle mostly off the window)
        let mut set = q("(x-12)^2 + y^2 = 9");
        set.push(("eq_labels", Value::B(true)));
        assert_eq!(halo_labels(&grid_svg(&set)).len(), 1, "edge-only curve is labelled");
    }

    #[test]
    fn label_solver_avoids_obstacles_uses_leaders_and_never_fails() {
        let bounds = [0.0, 0.0, 400.0, 400.0];
        let curve = vec![(100.0, 100.0), (200.0, 200.0), (300.0, 300.0)];
        let req = |a: Option<Pt>| LabelReq { ci: 0, text: "x^2 + y^2 = 25".into(), default_anchor: a, start: 0, end: 1, halo: true, colour: "#000".into() };
        // free: placed beside the curve, clear of it
        let mut placed = vec![];
        let out = solve_labels(&[req(None)], &[curve.clone()], &mut placed, bounds, LabelStyle { size: 15.0, plates: false });
        assert_eq!(out.len(), 1);
        assert!(label_conflicts(&placed[0], &[curve.clone()], &[]) == 0, "label box clear of its own curve");
        // an explicit default that is free is kept exactly
        let mut placed = vec![];
        let out = solve_labels(&[req(Some((50.0, 300.0)))], &[curve.clone()], &mut placed, bounds, LabelStyle { size: 15.0, plates: false });
        assert!(out[0].1.contains("x=\"58.00\"") && !out[0].1.contains("paint-order"), "{}", out[0].1);
        // an explicit default on its own curve moves
        let mut placed = vec![];
        let out = solve_labels(&[req(Some((100.0, 100.0)))], &[curve.clone()], &mut placed, bounds, LabelStyle { size: 15.0, plates: false });
        assert!(!out[0].1.contains("x=\"108.00\" y=\"88"), "moved off the curve");
        // a label already holding the best spots does not get overlapped
        let mut placed = vec![[0.0, 0.0, 400.0, 190.0]];
        solve_labels(&[req(None)], &[curve.clone()], &mut placed, bounds, LabelStyle { size: 15.0, plates: false });
        assert!(!rect_hit(&placed[1], &placed[0]));
        // a leader line is used when the near boxes are all taken
        let mut placed = vec![[0.0, 0.0, 400.0, 340.0]];
        let out = solve_labels(&[req(None)], &[curve.clone()], &mut placed, bounds, LabelStyle { size: 15.0, plates: false });
        assert!(out[0].1.contains("<line"), "leader line: {}", out[0].1);
        // never fails, even when everything is blocked
        let mut placed = vec![[-1000.0, -1000.0, 1000.0, 1000.0]];
        assert_eq!(solve_labels(&[req(None)], &[curve], &mut placed, bounds, LabelStyle { size: 15.0, plates: false }).len(), 1);
    }

    #[test]
    fn saved_figure_params_are_clamped_on_load() {
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        let p = crate::figures::params_from_json(&d, &serde_json::json!({"xmin": 1e300, "xmax": -1e300, "xstep": 1e-300, "equations": "x^2 + y^2 = 25"}));
        assert!(p.f("xmin") <= 1000.0 && p.f("xmax") >= -1000.0 && p.f("xstep") >= 0.05);
        let _ = (d.render)(&p);
    }

    #[test]
    fn handles_skip_conic_rows_with_the_real_degree_flag() {
        assert!(is_curve_row("x^2 + y^2 = 25", -10.0, 10.0, -10.0, 10.0, false));
        assert!(is_curve_row("x^2 + y^2 < 25", -10.0, 10.0, -10.0, 10.0, true));
        assert!(!is_curve_row("2x + 3y = 6", -10.0, 10.0, -10.0, 10.0, false));
        assert!(!is_curve_row("y = 2x + 1", -10.0, 10.0, -10.0, 10.0, true));
        // sin(y) = x in degrees over -360..360 is not a line, but over a plain window it is only samples; both are curves
        assert!(is_curve_row("sin(y) = x", -360.0, 360.0, -360.0, 360.0, true));
    }

    fn with(eq: &str, extra: &[(&'static str, Value)]) -> String {
        let mut set = q(eq);
        set.push(("eq_labels", Value::B(true)));
        for (k, v) in extra { set.push((k, v.clone())); }
        grid_svg(&set)
    }

    #[test]
    fn chained_inequalities_shade_the_ring_between() {
        for eq in ["4 < x^2 + y^2 < 25", "4 <= x^2 + y^2 <= 25", "25 > x^2 + y^2 > 4"] {
            let svg = with(eq, &[]);
            assert_eq!(curve_lines(&svg).len(), 2, "{eq}: both rings are drawn");
            assert_eq!(svg.matches("fill-opacity=\"0.18\"").count(), 1, "{eq}: one shaded path");
            assert!(warns(eq).is_empty(), "{eq}: {:?}", warns(eq));
        }
        assert!(matches!(parse_row("4 < x^2 + y^2 < 25"), Some(Row::ImplicitIneq { strict: true, positive: true, .. })));
        assert!(matches!(parse_row("4 <= x^2 + y^2 <= 25"), Some(Row::ImplicitIneq { strict: false, .. })));
        // a chain whose ends are the wrong way round holds nowhere
        assert!(!with("25 < x^2 + y^2 < 4", &[]).contains("fill-opacity=\"0.18\""));
    }

    #[test]
    fn print_style_is_black_dashed_and_hatched_and_the_student_copy_hides_the_answer() {
        let eq = "x^2 + y^2 = 25; (x-8)^2 + y^2 = 9; y = -x + 7.07; x^2/9 + y^2/4 <= 1";
        let colour = with(eq, &[]);
        assert!(colour.contains("#1f5fbf"));
        let print = with(eq, &[("style", Value::S("Print".into()))]);
        for c in ["#1f5fbf", "#c0392b", "#2a8a3e", "#8e44ad"] { assert!(!print.contains(c), "{c} in Print"); }
        // solid is an included boundary and every plain curve: Print never varies the dashes by row
        assert!(!print.contains("stroke-dasharray=\"6 3\"") && !print.contains("stroke-dasharray=\"2 3\"") && !print.contains("stroke-dasharray=\"1 4\""), "no dotted or row-numbered dashes");
        let widths: std::collections::BTreeSet<String> = print.split("<polyline").skip(1).filter_map(|t| t.split("stroke-width=\"").nth(1).and_then(|r| r.split('"').next()).map(|w| w.to_string())).collect();
        assert!(widths.len() >= 3, "rows differ by line weight: {widths:?}");
        // only an excluded boundary is dashed; a style word overrides, on a strict inequality too
        let strict = with("x^2 + y^2 < 25; x^2 + y^2 <= 9", &[("style", Value::S("Print".into()))]);
        assert_eq!(strict.matches("stroke-dasharray=\"9 6\"").count(), 1, "one dashed (strict) boundary");
        let over = with("x^2 + y^2 < 25 solid; x^2 + y^2 <= 9 dotted", &[]);
        assert!(!over.contains("stroke-dasharray=\"9 6\"") && over.contains("stroke-dasharray=\"2 4\""));
        assert!(print.contains("url(#sh3)"), "the inequality is hatched");
        let student = with(eq, &[("style", Value::S("Print".into())), ("show_answers", Value::B(false))]);
        assert!(!student.contains("url(#sh3)") && halo_labels(&student).is_empty(), "no shading, no labels");
        assert!(!halo_labels(&print).is_empty(), "the key keeps its labels");
        assert!(curve_lines(&student).len() >= 4, "the curves stay");
        // the colour figure is unchanged by the new switches at their defaults
        assert_eq!(colour, with(eq, &[("show_answers", Value::B(true)), ("show_notes", Value::B(false))]));
    }

    #[test]
    fn notes_can_be_written_on_the_figure_and_many_are_collapsed() {
        let eq = "x^2 + y^2 = 400; x^2 + y^2 = -4";
        assert!(!with(eq, &[]).contains("No part of this curve"));
        let shown = with(eq, &[("show_notes", Value::B(true))]);
        assert!(shown.contains("No part of this curve"), "notes on the figure when asked");
        let many = (0..12).map(|k| format!("x^2 + y^2 = -{}", k + 1)).collect::<Vec<_>>().join("; ");
        let w = warns(&many);
        assert!(w.len() <= 5 && w.iter().any(|x| x.contains("more rows")), "{w:?}");
        assert!(w[0].starts_with("Row 1:"), "{w:?}");
    }

    #[test]
    fn rows_are_capped_and_typos_are_named_by_row() {
        let many = (0..60).map(|k| format!("y = x + {k}")).collect::<Vec<_>>().join("; ");
        assert!(warns(&many).iter().any(|w| w.contains("Only the first 40 rows")));
        assert!(curve_lines(&grid_svg(&q(&many))).len() <= MAX_ROWS);
        let w = warns("y = x; y = sinn(x)");
        assert!(w.iter().any(|x| x.starts_with("Row 2:") && x.contains("Cannot read")), "{w:?}");
        assert!(warns("y = x; y = 1/x").is_empty());
    }

    #[test]
    fn a_style_word_dashes_a_row_and_a_single_point_is_drawn_and_named() {
        assert_eq!(strip_style("y = 2x/3 dashed"), ("y = 2x/3", Some("9 6")));
        assert_eq!(strip_style("y = 2x dotted"), ("y = 2x", Some("2 4")));
        assert_eq!(strip_style("y = x"), ("y = x", None));
        assert_eq!(strip_style("y = dashed"), ("y = dashed", None));
        let svg = with("x^2 + y^2 = 25 dashed; y = x dotted", &[]);
        assert!(svg.contains("stroke-dasharray=\"9 6\"") && svg.contains("stroke-dasharray=\"2 4\""));
        assert!(!svg.contains("dashed"), "the word is not part of the label");
        let w = warns("(x-3)^2 + (y+2)^2 = 0");
        assert!(w.iter().any(|x| x.contains("single point")) && !w.iter().any(|x| x.contains("No part")), "{w:?}");
        assert!(with("(x-3)^2 + (y+2)^2 = 0", &[]).contains("<circle"));
    }

    /// Lines that are asymptotes of a conic in the figure (or say so) are drawn fine, dotted and grey, all alike, and never heavier
    /// than the curve, whatever else was typed after them: dashed is for excluded boundaries only.
    #[test]
    fn asymptotes_are_recognised_and_drawn_fine_dotted_and_alike() {
        let hyp = "x^2/9 - y^2/4 = 1";
        for style in ["Colour", "Print"] {
            for rows in [format!("{hyp}; y = 2x/3 dashed; y = -2x/3 dashed"), format!("{hyp}; y = 2x/3; y = -2x/3"), format!("{hyp}; y = 2x/3 asymptote; y = -2x/3 asymptote"), format!("{hyp}; 2x - 3y = 0 dotted; y = -2x/3 dash-dot"), format!("{hyp}; y = 4 asymptote; y = 0.5x asymptote")] {
                let mut set = q(&rows);
                set.push(("style", Value::S(style.into())));
                let svg = grid_svg(&set);
                let lines: Vec<&str> = svg.split("<polyline ").skip(1).map(|t| t.split("/>").next().unwrap()).filter(|t| t.contains("stroke-dasharray")).collect();
                assert_eq!(lines.len(), 2, "{style} {rows}: {lines:?}");
                let attr = |t: &str, k: &str| t.split(&format!("{k}=\"")).nth(1).unwrap().split('"').next().unwrap().to_string();
                let l: Vec<(String, String, String)> = lines.iter().map(|t| (attr(t, "stroke"), attr(t, "stroke-width"), attr(t, "stroke-dasharray"))).collect();
                assert_eq!(l[0], l[1], "{style} {rows}: both asymptotes alike");
                assert!(!l[0].2.contains("9 6") && l[0].2 != "9 4 2 4", "{style} {rows}: not dashed");
                // thinner than the curve
                let curve_w: f64 = svg.split("<polyline ").nth(1).unwrap().split("stroke-width=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
                assert!(l[0].1.parse::<f64>().unwrap() < curve_w, "{style} {rows}: lighter than the curve");
                if style == "Print" { assert_eq!((l[0].0.as_str(), l[0].1.as_str(), l[0].2.as_str()), ("#666666", "1.6", "7 5"), "{rows}: Print asymptote style"); assert!(curve_w >= 2.4, "{rows}: Print curve {curve_w} px"); }
                else { assert!(l[0].1.parse::<f64>().unwrap() >= 1.4, "{rows}: Colour asymptote at least 1.4 px"); }
            }
        }
        // a line that is not an asymptote keeps what was typed
        let svg = with(&format!("{hyp}; y = x dashed"), &[]);
        assert!(svg.contains("stroke-dasharray=\"9 6\""));
        // the warning says so
        assert!(warns(&format!("{hyp}; y = 2x/3 dashed")).iter().any(|w| w.contains("asymptote")));
        // plain typed lines are never heavier than a conic they accompany (Print weights differ by row)
        let mut set = q("x^2 + y^2 = 4; y = x; y = -x + 1; x = 3; y = 2");
        set.push(("style", Value::S("Print".into())));
        let svg = grid_svg(&set);
        let poly_w: Vec<f64> = svg.split("<polyline ").skip(1).map(|t| t.split("stroke-width=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap()).collect();
        let circle_w = poly_w[0];
        assert!(poly_w.len() >= 3 && poly_w.iter().all(|w| *w <= circle_w + 1e-9), "{poly_w:?}");
    }

    #[test]
    fn products_of_variables_are_set_as_two_letters() {
        assert_eq!(label_text("xy = 4"), "x\u{2009}y = 4");
        assert_eq!(label_text("x*y = 4"), "x\u{2009}y = 4");
        assert_eq!(label_text("sin(x) = y"), "sin(x) = y");
        assert!(with("xy = 4", &[]).contains("font-style=\"italic\">y"));
    }

    #[test]
    fn a_curved_and_a_straight_inequality_shade_their_common_part_once() {
        let svg = with("x^2 + y^2 < 25; y > x - 2", &[]);
        assert_eq!(svg.matches(GROUP_TINT).count(), 1, "one tinted path (no outline strokes: they show as a mesh over the cells)");
        assert!(!svg.contains("fill-opacity=\"0.18\""), "no row tints of its own");
        let svg2 = with("x^2 + y^2 < 25; x + y < 3", &[]);
        assert_eq!(svg2.matches(GROUP_TINT).count(), 1);
        // two straight inequalities keep their separate tints (as before)
        assert!(!with("y > x - 2; y < -x + 6", &[]).contains(GROUP_TINT));
        // the zero sits right of the origin beside a conic, left of it in a plain figure
        assert!(with("x^2 + y^2 = 25", &[]).contains("text-anchor=\"start\""));
    }

    fn neutral_hexes(svg: &str) -> Vec<String> {
        let b = svg.as_bytes();
        let mut bad = vec![];
        for i in 0..b.len().saturating_sub(7) {
            if b[i] == b'#' && b[i + 1..i + 7].iter().all(|c| c.is_ascii_hexdigit()) && !b.get(i + 7).is_some_and(|c| c.is_ascii_hexdigit()) {
                let h = &svg[i + 1..i + 7];
                if h[0..2] != h[2..4] || h[2..4] != h[4..6] { bad.push(format!("#{h}")); }
            }
        }
        bad.sort();
        bad.dedup();
        bad
    }

    #[test]
    fn print_is_neutral_grey_for_every_example_and_light_in_the_grid() {
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        let exs = examples_b_plots("coordinate-grid").unwrap();
        assert!(exs.len() >= 18);
        for (i, e) in exs.iter().enumerate() {
            for answers in [true, false] {
                let mut p = Params::defaults(&(d.params)());
                for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
                p.0.insert("style".into(), Value::S("Print".into()));
                p.0.insert("show_answers".into(), Value::B(answers));
                p.0.insert("eq_labels".into(), Value::B(true));
                p.0.insert("show_notes".into(), Value::B(true));
                let bad = neutral_hexes(&(d.render)(&p));
                assert!(bad.is_empty(), "example {i} ({}): Print colours with a tint: {bad:?}", e.name);
            }
        }
        // the standard grid is light and thin in Print (the colour figure keeps its near-black 1 px lines)
        let plain = with("x^2 + y^2 = 25", &[("style", Value::S("Print".into()))]);
        assert!(plain.contains("stroke=\"#5a5a5a\"") && !plain.contains("stroke=\"#111827\""));
        assert!(with("x^2 + y^2 = 25", &[]).contains("stroke=\"#111827\""));
    }

    #[test]
    fn scientific_notation_is_one_number_and_e_alone_stays_euler() {
        assert_eq!(eval("1e12", 0.0), Some(1e12));
        assert_eq!(eval("2e3", 0.0), Some(2000.0));
        assert!((eval("1.5e-3", 0.0).unwrap() - 0.0015).abs() < 1e-15);
        assert!((eval("2.5E+2", 0.0).unwrap() - 250.0).abs() < 1e-9);
        assert!((eval("2e", 0.0).unwrap() - 2.0 * std::f64::consts::E).abs() < 1e-12);
        assert!((eval("e^2", 0.0).unwrap() - std::f64::consts::E.powi(2)).abs() < 1e-12);
        assert!((eval("2e^x", 1.0).unwrap() - 2.0 * std::f64::consts::E).abs() < 1e-12);
        // x^2 + y^2 = 1e2 is the circle of radius 10
        let ls = curve_lines(&with("x^2 + y^2 = 1e2", &[]));
        assert!(!ls.is_empty());
    }

    #[test]
    fn chains_work_with_every_mix_of_strictness_and_direction() {
        for eq in ["4<x^2+y^2<=25", "4<=x^2+y^2<25", "25>x^2+y^2>=4", "25>=x^2+y^2>4", "4<=x^2+y^2<=25", "4<x^2+y^2<25"] {
            let w = warns(eq);
            assert!(w.is_empty(), "{eq}: {w:?}");
            assert_eq!(curve_lines(&with(eq, &[])).len(), 2, "{eq}");
        }
        assert!(matches!(parse_row("4<x^2+y^2<=25"), Some(Row::ImplicitIneq { strict: true, .. })));
        assert!(matches!(parse_row("4<=x^2+y^2<=25"), Some(Row::ImplicitIneq { strict: false, .. })));
        // equal limits are the one circle; reversed limits hold nowhere, and say so
        let ls = curve_lines_min(&with("4 <= x^2 + y^2 <= 4", &[]), 1);
        assert_eq!(ls.len(), 1, "{:?}", ls.iter().map(|l| (l.len(), l[0], l[l.len() - 1])).collect::<Vec<_>>());
        assert!(warns("4 <= x^2 + y^2 <= 4").is_empty());
        assert!(warns("25 < x^2 + y^2 < 4").iter().any(|w| w.contains("no points satisfy both limits")));
    }

    #[test]
    fn a_band_joins_the_common_solution() {
        let with_band = with("x^2 + y^2 < 25; y > x^2 - 4; x > 0", &[]);
        let without = with("x^2 + y^2 < 25; y > x^2 - 4", &[]);
        let area = |svg: &str| -> f64 {
            // area of the tinted path (shoelace over each subpath)
            let d = svg.split(&format!("fill=\"{GROUP_TINT}\"")).next().unwrap().rsplit("<path d=\"").next().unwrap().split('"').next().unwrap().to_string();
            let mut total = 0.0;
            for sub in d.split('Z') {
                let pts: Vec<(f64, f64)> = sub.split(['M', 'L']).filter_map(|t| { let mut it = t.split_whitespace(); Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?)) }).collect();
                if pts.len() < 3 { continue; }
                let mut a = 0.0;
                for k in 0..pts.len() { let (p, q) = (pts[k], pts[(k + 1) % pts.len()]); a += p.0 * q.1 - q.0 * p.1; }
                total += a / 2.0;
            }
            total.abs()
        };
        // the circle and the parabola share 34.24 square units; right of the y axis it is half of that (17.12)
        let ratio = area(&with_band) / area(&without);
        assert!(ratio > 0.4 && ratio < 0.6, "ratio {ratio}");
        assert_eq!(with_band.matches(&format!("fill=\"{GROUP_TINT}\"")).count(), 1, "one tinted path");
    }

    #[test]
    fn squared_rows_draw_and_only_a_real_empty_set_is_called_empty() {
        for eq in ["(x-1)^2*(y-2)^2 = 0", "(x^2+y^2-25)^2 = 0", "(x^2-1)^2 = 0", "(x-y)^2*(x+y)^2 = 0"] {
            let svg = with(eq, &[]);
            assert!(!curve_lines_min(&svg, 1).is_empty(), "{eq} draws");
            assert!(!warns(eq).iter().any(|w| w.contains("No part")), "{eq}: {:?}", warns(eq));
        }
        // x^2 + y^2 <= 0 is the origin
        assert!(with("x^2 + y^2 <= 0", &[]).contains("<circle"));
        assert!(warns("x^2 + y^2 <= 0").iter().any(|w| w.contains("single point")));
        // a strict one holds nowhere
        assert!(!with("x^2 + y^2 < 0", &[]).contains("<circle"));
    }

    #[test]
    fn style_words_go_before_a_domain_and_labels_keep_the_multiplication() {
        let svg = with("x^2 + y^2 = 25 dashed, x > 0", &[]);
        assert!(warns("x^2 + y^2 = 25 dashed, x > 0").is_empty());
        assert!(svg.contains("stroke-dasharray=\"9 6\"") && !svg.contains("dashed"));
        assert_eq!(label_text("x*3 = y^2"), "x\u{b7}3 = y^2");
        assert_eq!(label_text("2*3 = 6"), "2*3 = 6");
        assert_eq!(label_text("x^2*3"), "x^2\u{b7}3".replace('\u{b7}', "*").replace("x^2*3", "x^2*3"));
        assert_eq!(label_text("x*y = 4"), "x\u{2009}y = 4");
    }

    #[test]
    fn explicit_poles_do_not_draw_false_verticals_in_wide_windows() {
        let wide = [("xmin", Value::F(-100.0)), ("xmax", Value::F(100.0)), ("ymin", Value::F(-100.0)), ("ymax", Value::F(100.0)), ("xstep", Value::F(20.0)), ("ystep", Value::F(20.0)), ("square", Value::B(false))];
        for eq in ["y = tan(x)", "y = 1/x", "y = 1/(x^2 - 1)"] {
            let mut set = q(eq);
            for (k, v) in &wide { set.push((k, v.clone())); }
            let gl = grid_layout_of(&set);
            let mid = gl.mt + gl.ph / 2.0;
            let svg = grid_svg(&set);
            for l in curve_lines_min(&svg, 1) {
                for w in l.windows(2) {
                    // a long near-vertical segment that crosses the x axis joins the two sides of a pole (one that stays on its
                    // side is just the steep branch)
                    let (dx, dy) = ((w[1].0 - w[0].0).abs(), (w[1].1 - w[0].1).abs());
                    assert!(!(dy > 120.0 && dx < 4.0 && (w[0].1 - mid) * (w[1].1 - mid) < 0.0), "{eq}: a {dy}px line across a pole");
                }
            }
        }
    }

    /// Distance in px from `pt` to the nearest segment of the polylines (looked up in 16 px buckets).
    struct Near { buckets: std::collections::HashMap<(i64, i64), Vec<((f64, f64), (f64, f64))>> }
    impl Near {
        fn new(lines: &[Vec<(f64, f64)>]) -> Near {
            let mut buckets: std::collections::HashMap<(i64, i64), Vec<((f64, f64), (f64, f64))>> = Default::default();
            for l in lines {
                for w in l.windows(2) {
                    let (x0, x1, y0, y1) = (w[0].0.min(w[1].0), w[0].0.max(w[1].0), w[0].1.min(w[1].1), w[0].1.max(w[1].1));
                    for bx in (x0 / 16.0).floor() as i64..=(x1 / 16.0).floor() as i64 { for by in (y0 / 16.0).floor() as i64..=(y1 / 16.0).floor() as i64 { buckets.entry((bx, by)).or_default().push((w[0], w[1])); } }
                }
            }
            Near { buckets }
        }
        fn dist(&self, p: (f64, f64)) -> f64 {
            let mut best = f64::INFINITY;
            for bx in (p.0 / 16.0).floor() as i64 - 1..=(p.0 / 16.0).floor() as i64 + 1 {
                for by in (p.1 / 16.0).floor() as i64 - 1..=(p.1 / 16.0).floor() as i64 + 1 {
                    for &(a, b) in self.buckets.get(&(bx, by)).into_iter().flatten() {
                        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                        let l2 = dx * dx + dy * dy;
                        let t = if l2 < 1e-12 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
                        best = best.min((p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy));
                    }
                }
            }
            best
        }
    }

    /// Explicit rows with poles must run all the way to the window edge on both sides of each pole (not stop at the last
    /// steep sample): at most 1% of the true points are more than 2 px from the drawing, none more than 6 px.
    #[test]
    fn explicit_branches_reach_the_window_edge_at_every_width() {
        use std::f64::consts::PI;
        type Cv = (&'static str, Box<dyn Fn(f64) -> f64>, Box<dyn Fn(f64) -> Vec<f64>>);
        let curves: Vec<Cv> = vec![
            ("y = tan(x)", Box::new(|x| x.tan()), Box::new(|y| (-700..=700).map(|k| y.atan() + k as f64 * PI).collect())),
            ("tan(x) = y", Box::new(|x| x.tan()), Box::new(|y| (-700..=700).map(|k| y.atan() + k as f64 * PI).collect())),
            ("y = 1/x", Box::new(|x| 1.0 / x), Box::new(|y| vec![1.0 / y])),
            ("y = 1/(x^2 - 1)", Box::new(|x| 1.0 / (x * x - 1.0)), Box::new(|y| { let t = 1.0 + 1.0 / y; if t >= 0.0 { vec![t.sqrt(), -t.sqrt()] } else { vec![] } })),
        ];
        for (eq, f, xs_of_y) in &curves {
            for half in [50.0, 100.0, 200.0, 1000.0] {
                // (tan has a period of 1.5 px at +-1000: aliased by any sampler, so it is checked only to +-200)
                if half > 500.0 && eq.contains("tan") { continue; }
                let mut set = q(eq);
                for (k, v) in [("xmin", Value::F(-half)), ("xmax", Value::F(half)), ("ymin", Value::F(-half)), ("ymax", Value::F(half)), ("square", Value::B(false)), ("xstep", Value::F(half / 5.0)), ("ystep", Value::F(half / 5.0))] { set.push((k, v)); }
                let g = grid_layout_of(&set);
                let svg = grid_svg(&set);
                let near = Near::new(&curve_lines_min(&svg, 1));
                let to_px = |x: f64, y: f64| (g.ml + (x + half) * g.ux, g.mt + g.ph - (y + half) * g.uy);
                let (mut total, mut far, mut worst) = (0usize, 0usize, 0.0f64);
                let mut bad = vec![];
                let mut pts: Vec<(f64, f64)> = vec![];
                for i in 0..4000 { let x = -half + 2.0 * half * (i as f64 + 0.37) / 4000.0; pts.push((x, f(x))); }
                for j in 0..1500 { let y = -half + 2.0 * half * (j as f64 + 0.41) / 1500.0; for x in xs_of_y(y) { if x.abs() < half { pts.push((x, y)); } } }
                for (x, y) in pts {
                    if !y.is_finite() || y.abs() > half * 0.999 || x.abs() > half * 0.999 { continue; }
                    total += 1;
                    let d = near.dist(to_px(x, y));
                    worst = worst.max(d);
                    if d > 2.0 { far += 1; if bad.len() < 5 { bad.push((x, y, d)); } }
                }
                assert!(total > 100);
                assert!(far as f64 <= 0.01 * total as f64, "{eq} +-{half}: {far} of {total} true points are over 2 px off, e.g. {bad:?}");
                assert!(worst <= 6.0, "{eq} +-{half}: a true point is {worst} px off");
            }
        }
    }

    /// How well the drawing follows a curve whose points are known: (points inside the window, over 2 px off, worst px off).
    fn cover(set: &[(&str, Value)], pts: &[(f64, f64)]) -> (usize, usize, f64, Vec<(f64, f64, f64)>) {
        let g = grid_layout_of(set);
        let svg = grid_svg(set);
        let near = Near::new(&curve_lines_min(&svg, 1));
        let (mut total, mut far, mut worst, mut bad) = (0usize, 0usize, 0.0f64, vec![]);
        for &(x, y) in pts {
            if !(x > g.x0 + 1e-9 && x < g.x1 - 1e-9 && y > g.y0 + 1e-9 && y < g.y1 - 1e-9) { continue; }
            total += 1;
            let d = near.dist((g.ml + (x - g.x0) * g.ux, g.mt + g.ph - (y - g.y0) * g.uy));
            worst = worst.max(d);
            if d > 2.0 { far += 1; if bad.len() < 5 { bad.push((x, y, d)); } }
        }
        (total, far, worst, bad)
    }

    /// Conics are followed to the edge of the window and in every part of it: at most 1% of the true points more than 2 px off,
    /// none more than 6 px (the arms of xy = 4 hugging the axes, a hyperbola in a flat strip).
    #[test]
    fn conics_follow_the_curve_everywhere_in_the_window() {
        let xy4 = |half: f64| -> Vec<(f64, f64)> {
            let mut v = vec![];
            for i in 1..=20000 { let x = half * (i as f64 + 0.37) / 20000.0; if x != 0.0 { v.push((x, 4.0 / x)); v.push((-x, -4.0 / x)); } }
            for j in 1..=20000 { let y = half * (j as f64 + 0.41) / 20000.0; v.push((4.0 / y, y)); v.push((-4.0 / y, -y)); }
            v
        };
        let hyp = || -> Vec<(f64, f64)> { let mut v = vec![]; for i in -4000..=4000 { let t = i as f64 / 1000.0; for s in [1.0, -1.0] { v.push((s * 3.0 * t.cosh(), 2.0 * t.sinh())); } } v };
        let win = |half: f64| vec![("xmin", Value::F(-half)), ("xmax", Value::F(half)), ("ymin", Value::F(-half)), ("ymax", Value::F(half)), ("square", Value::B(false))];
        let mut cases: Vec<(String, Vec<(&'static str, Value)>, Vec<(f64, f64)>)> = vec![];
        for half in [10.0, 50.0, 200.0, 1000.0] { let mut set = q("xy = 4"); set.extend(win(half)); cases.push((format!("xy = 4 at +-{half}"), set, xy4(half))); }
        for sq in [true, false] {
            let mut set = q("x^2/9 - y^2/4 = 1");
            set.extend([("xmin", Value::F(-50.0)), ("xmax", Value::F(50.0)), ("ymin", Value::F(-5.0)), ("ymax", Value::F(5.0)), ("square", Value::B(sq))]);
            cases.push((format!("hyperbola at +-50 x +-5, equal scale {sq}"), set, hyp()));
        }
        for (name, set, pts) in cases {
            let (total, far, worst, bad) = cover(&set, &pts);
            assert!(total > 100, "{name}: only {total} true points in the window");
            assert!(far as f64 <= 0.01 * total as f64, "{name}: {far} of {total} true points are over 2 px off, e.g. {bad:?}");
            assert!(worst <= 6.0, "{name}: a true point is {worst} px off, e.g. {bad:?}");
        }
        // nothing inside the window: no clamped scraps of curve, and the note says so
        let mut set = q("xy = 4");
        set.extend([("xmin", Value::F(-0.1)), ("xmax", Value::F(0.1)), ("ymin", Value::F(-0.1)), ("ymax", Value::F(0.1))]);
        let svg = grid_svg(&set);
        assert!(curve_lines_min(&svg, 0).is_empty(), "{:?}", curve_lines_min(&svg, 0).iter().map(|l| (l.len(), l[0])).collect::<Vec<_>>());
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in &set { p.0.insert(k.to_string(), v.clone()); }
        assert!(grid_warnings(&p).iter().any(|w| w.contains("No part of this curve is inside the window")), "{:?}", grid_warnings(&p));
    }

    /// CPU time this thread has used, in ms (Linux: /proc/thread-self/stat, 100 ticks a second; 10 ms steps).
    fn thread_cpu_ms() -> Option<u128> {
        let s = std::fs::read_to_string("/proc/thread-self/stat").ok()?;
        let f: Vec<&str> = s[s.rfind(')')? + 2..].split(' ').collect();
        let (u, k): (u128, u128) = (f.get(11)?.parse().ok()?, f.get(12)?.parse().ok()?);
        Some((u + k) * 10)
    }

    fn timed(eq: &str, extra: &[(&'static str, Value)]) -> (u128, Vec<String>) {
        let mut set = q(eq);
        set.push(("show_answers", Value::B(true)));
        set.extend(extra.iter().cloned());
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in &set { p.0.insert(k.to_string(), v.clone()); }
        // CPU time of this thread where the system gives it (the suite runs hundreds of tests at once, which stretches the wall time)
        let (t, c) = (std::time::Instant::now(), thread_cpu_ms());
        let svg = (d.render)(&p);
        let ms = match (c, thread_cpu_ms()) { (Some(a), Some(b)) => b - a, _ => t.elapsed().as_millis() };
        assert!(svg.len() < 12_000_000);
        // (the editor's warnings come from the same build: not timed again here)
        (ms, grid_warnings(&p))
    }

    /// Long or deeply nested inequality rows used to cost 6 to 21 s: a row longer than 200 characters is refused before any
    /// evaluation, the rest share one work budget (rows x characters x cells).
    #[test]
    fn long_nested_and_many_inequality_rows_stay_within_the_time_bound() {
        let limit = if cfg!(debug_assertions) { 120_000 } else { 6_000 };
        let long = format!("x^2+y^2<{}1;", "1+".repeat(490));
        let mid = format!("x^2+y^2<{}1;", "1+".repeat(85));
        let sins = format!("x^2+y^2<{}x{};", "sin(".repeat(25), ")".repeat(25));
        let cases = [
            ("8 rows of 990 chars", long.repeat(8)),
            ("4 rows of 990 chars", long.repeat(4)),
            ("8 rows of 180 chars", mid.repeat(8)),
            ("8 nested sin rows", sins.repeat(8)),
            ("20000 long inequality rows", long.repeat(20000)),
            ("40 rows of 180 chars", mid.repeat(40)),
            ("mixed with plain rows", format!("{}y > x; x^2 + y^2 < 30;{}", mid.repeat(6), sins.repeat(6))),
        ];
        for (name, eq) in &cases {
            let (ms, w) = timed(eq, &[]);
            println!("{name}: {ms} ms, {} notes", w.len());
            assert!(ms < limit, "{name}: {ms} ms");
        }
        // a refused row says so
        let (_, w) = timed(&long.repeat(2), &[]);
        assert!(w.iter().any(|n| n.contains("too long")), "{w:?}");
        // and the group still shades when its rows are of ordinary length
        let svg = grid_svg(&{ let mut s = q("x^2 + y^2 < 25; y > x - 3; x > -4"); s.push(("show_answers", Value::B(true))); s });
        assert!(svg.contains(GROUP_TINT) || svg.contains("url(#shg)"));
    }

    /// A 200000-deep bracket is refused by length and depth before anything evaluates it.
    #[test]
    fn a_hugely_nested_row_is_refused_quickly() {
        let limit = if cfg!(debug_assertions) { 20_000 } else { 800 };
        for eq in ["(".repeat(200_000) + "x" + &")".repeat(200_000), format!("y = {}x{}", "(".repeat(200_000), ")".repeat(200_000)), format!("x^2+y^2<{}x{}", "(".repeat(200_000), ")".repeat(200_000)), "sin(".repeat(50_000)] {
            let (ms, w) = timed(&eq, &[]);
            println!("deep row ({} chars): {ms} ms", eq.len());
            assert!(ms < limit, "{ms} ms");
            assert!(w.iter().any(|n| n.contains("too long")), "{w:?}");
        }
    }

    /// (is dashed, width in px) of every polyline that is a curve of the figure (a polyline of the figure).
    fn dashed_polys(svg: &str) -> Vec<(bool, f64)> {
        svg.split("<polyline ").skip(1).filter_map(|t| {
            let tag = t.split("/>").next()?;
            let pts = tag.split("points=\"").nth(1)?.split('"').next()?;
            let xs: Vec<f64> = pts.split(' ').filter_map(|c| c.split_once(',')?.0.parse().ok()).collect();
            if xs.len() < 2 { return None; }
            let w = xs.iter().cloned().fold(f64::MIN, f64::max) - xs.iter().cloned().fold(f64::MAX, f64::min);
            Some((tag.contains("stroke-dasharray=\"9 6\""), w))
        }).collect()
    }

    /// Each limit of a chain keeps its own strictness: the excluded ring is dashed, the included one solid.
    #[test]
    fn mixed_strictness_chains_dash_only_the_excluded_limit() {
        // (row, the inner ring (radius 2, ~96 px wide) is dashed, the outer ring (radius 5, ~240 px) is dashed)
        for (row, inner, outer) in [("4 < x^2 + y^2 <= 25", true, false), ("4 <= x^2 + y^2 < 25", false, true), ("25 > x^2 + y^2 >= 4", false, true), ("25 >= x^2 + y^2 > 4", true, false), ("4 < x^2 + y^2 < 25", true, true), ("4 <= x^2 + y^2 <= 25", false, false)] {
            let polys = dashed_polys(&grid_svg(&q(row)));
            assert_eq!(polys.len(), 2, "{row}: {polys:?}");
            let (small, big) = if polys[0].1 < polys[1].1 { (polys[0], polys[1]) } else { (polys[1], polys[0]) };
            assert_eq!((small.0, big.0), (inner, outer), "{row}: {polys:?}");
        }
        // an explicit-y chain, and the same in Print
        let polys = dashed_polys(&grid_svg(&q("1 < y <= 4")));
        assert_eq!(polys.iter().filter(|p| p.0).count(), 1, "1 < y <= 4: {polys:?}");
        assert_eq!(polys.len(), 2);
        let polys = dashed_polys(&grid_svg(&{ let mut s = q("4 < x^2 + y^2 <= 25"); s.push(("style", Value::S("Print".into()))); s }));
        assert_eq!(polys.iter().filter(|p| p.0).count(), 1, "{polys:?}");
        // a user style word still wins for both
        let polys = dashed_polys(&grid_svg(&q("4 < x^2 + y^2 <= 25 solid")));
        assert_eq!(polys.iter().filter(|p| p.0).count(), 0, "{polys:?}");
    }

    fn lum_image(svg: &str) -> (usize, usize, Vec<f64>) {
        let (w, h, px) = super::super::to_rgba(svg, 1.0).unwrap();
        let lum = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a) }).collect();
        (w as usize, h as usize, lum)
    }

    /// Print has to survive a bitonal photocopy: at a threshold of 50% to 75% the gridlines are still all there and the shaded
    /// region still reads as hatching (a diagonal screen of dark lines, more of it than the grid alone).
    #[test]
    fn print_gridlines_and_hatching_survive_a_bitonal_copy() {
        let cases = [("x^2 + y^2 < 25; y > x - 3", "circle and half-plane"), ("4 < x^2 + y^2 <= 25", "annulus")];
        for (row, name) in cases {
            let mut set = q(row);
            set.push(("style", Value::S("Print".into())));
            let g = grid_layout_of(&set);
            let svg = grid_svg(&set);
            let (w, h, lum) = lum_image(&svg);
            // an unshaded stretch at the top of the plot: every vertical gridline there must be black over it
            let (top, bottom) = ((g.mt + 4.0) as usize, (g.mt + 3.0 * g.uy) as usize);
            for thr in [0.50, 0.58, 0.66, 0.75] {
                let dark = |x: usize, y: usize| lum[y * w + x] < thr * 255.0;
                for k in 1..=9 {
                    let gx = g.ml + (k as f64) * 2.0 * g.ux; // x = -10 + 2k
                    let col = (-1..=1).map(|d| (gx as i64 + d) as usize).max_by_key(|&x| (top..bottom).filter(|&y| dark(x, y)).count()).unwrap();
                    let n = (top..bottom).filter(|&y| dark(col, y)).count();
                    assert!(n as f64 >= 0.9 * (bottom - top) as f64, "{name} at {thr}: gridline {k} only {n}/{} dark", bottom - top);
                }
                // inside the shading (away from curves and gridlines): hatching is a good share of dark pixels, and it is diagonal
                let (cx, cy) = (g.ml + 10.0 * g.ux + 1.0 * g.ux, g.mt + 10.0 * g.uy - 0.5 * g.uy);
                let near_grid = |x: usize, y: usize| { let fx = ((x as f64 - g.ml) / g.ux).fract(); let fy = ((y as f64 - g.mt) / g.uy).fract(); let t = 2.0 / g.ux; fx < t || fx > 1.0 - t || fy < t || fy > 1.0 - t };
                let (mut total, mut black) = (0, 0);
                let (mut same_dir, mut other_dir) = (0, 0);
                for y in (cy as usize - 40)..(cy as usize + 40) { for x in (cx as usize - 40)..(cx as usize + 40) {
                    if near_grid(x, y) || y + 1 >= h || x + 1 >= w { continue; }
                    total += 1;
                    if dark(x, y) {
                        black += 1;
                        // a hatch line runs along one diagonal: a dark pixel's dark neighbours lie on that diagonal
                        let (d1, d2) = (dark(x + 1, y + 1), dark(x + 1, y.saturating_sub(1)));
                        if d1 != d2 { if d1 { same_dir += 1 } else { other_dir += 1 } }
                    }
                } }
                let frac = black as f64 / total as f64;
                assert!(frac >= 0.06 && frac <= 0.5, "{name} at {thr}: {:.3} of the shaded pixels are dark", frac);
                assert!(same_dir + other_dir > 0, "{name} at {thr}: no diagonal runs");
                // outside the shading there is none of it
                let (ox, oy) = (g.ml + 18.0, g.mt + 30.0);
                let out_black = (oy as usize..oy as usize + 30).flat_map(|y| (ox as usize..ox as usize + 30).map(move |x| (x, y))).filter(|&(x, y)| !near_grid(x, y) && dark(x, y)).count();
                assert!(out_black == 0, "{name} at {thr}: {out_black} dark pixels in an unshaded corner");
            }
        }
    }

    /// Every leader line of a label ends in a dot that sits on its own curve.
    #[test]
    fn label_leaders_touch_their_own_curve() {
        for (rows, print) in [("x^2 + y^2 = 1; x^2 + y^2 = 4; x^2 + y^2 = 9; x^2 + y^2 = 16", true), ("x^2 + y^2 = 1; x^2 + y^2 = 4; x^2 + y^2 = 9; x^2 + y^2 = 16", false), ("x^2 + y^2 = 25; (x-8)^2 + y^2 = 9; y = -x + 7.07", true), ("x^2/9 + y^2/4 <= 1; y > x^2 - 6; x^2 + y^2 = 4", false)] {
            let mut set = q(rows);
            set.push(("eq_labels", Value::B(true)));
            if print { set.push(("style", Value::S("Print".into()))); }
            let svg = grid_svg(&set);
            // curves by colour
            let mut by_col: Vec<(String, Vec<Vec<(f64, f64)>>)> = vec![];
            for t in svg.split("<polyline ").skip(1) {
                let tag = t.split("/>").next().unwrap();
                let col = tag.split("stroke=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
                let pts: Vec<(f64, f64)> = tag.split("points=\"").nth(1).unwrap().split('"').next().unwrap().split(' ').filter_map(|c| { let (a, b) = c.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
                match by_col.iter_mut().find(|c| c.0 == col) { Some(c) => c.1.push(pts), None => by_col.push((col, vec![pts])) }
            }
            let mut dots = 0;
            for t in svg.split("<circle ").skip(1) {
                let tag = t.split("/>").next().unwrap();
                if !tag.contains("r=\"2.6\"") { continue; }
                dots += 1;
                let num = |k: &str| -> f64 { tag.split(&format!("{k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
                let col = tag.split("fill=\"").nth(1).unwrap().split('"').next().unwrap();
                let near = Near::new(&by_col.iter().find(|c| c.0 == col).unwrap_or_else(|| panic!("no curve in {col}")).1);
                let d = near.dist((num("cx"), num("cy")));
                assert!(d <= 1.0, "{rows}: a leader dot is {d} px from its curve");
            }
            let _ = dots;
        }
    }

    /// The editor caches a figure's warnings under a key of every parameter: any control a note depends on (the title and axis-title
    /// modes change the plot height, so the "window much wider than tall" note) changes the key.
    #[test]
    fn the_warning_cache_key_covers_every_control() {
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        let base = Params::defaults(&(d.params)());
        let k0 = super::super::params_key(&base);
        assert_eq!(k0, super::super::params_key(&base.clone()));
        for def in (d.params)() {
            let mut p = base.clone();
            let changed = match &def.kind {
                Kind::Bool => { p.set_b(def.key, !base.b(def.key)); true }
                Kind::Choice(opts) => match opts.iter().find(|o| **o != base.s(def.key)) { Some(o) => { p.set_s(def.key, o); true } None => false },
                Kind::Float { .. } => { p.set_f(def.key, base.f(def.key) + 1.0); true }
                Kind::Int { .. } => { p.set_i(def.key, base.i(def.key) + 1); true }
                _ => { p.set_s(def.key, "q"); true }
            };
            if changed { assert_ne!(k0, super::super::params_key(&p), "changing {} leaves the key as it was", def.key); }
        }
    }

    /// Scientific notation is a literal that starts with a digit (2e3 = 2000, 2e-1 = 0.2); after a ^ it is not (x^2e3 is
    /// x^2 times e times 3, never a power of 2000), and only Euler's e is set in italic.
    #[test]
    fn scientific_literals_are_not_exponents_and_only_euler_e_is_italic() {
        assert_eq!(eval("2e3", 0.0), Some(2000.0));
        assert!((eval("2e-1", 0.0).unwrap() - 0.2).abs() < 1e-12);
        assert!((eval("2*e^3", 0.0).unwrap() - 2.0 * std::f64::consts::E.powi(3)).abs() < 1e-9);
        assert!((eval("2 e^3", 0.0).unwrap() - 2.0 * std::f64::consts::E.powi(3)).abs() < 1e-9);
        assert!((eval("x^2e3", 2.0).unwrap() - 4.0 * std::f64::consts::E * 3.0).abs() < 1e-9, "{:?}", eval("x^2e3", 2.0));
        assert_eq!(eval("x^(2e3)", 1.0), Some(1.0));
        assert!(eval("x^2e3", 2.0).unwrap().is_finite());
        // the label: 2e3 and 1.5e-4 are numbers; the e of 2*e^3 and of x^2e3 is Euler's
        let plain = |t: &str| t.contains("font-style=\"italic\">e<");
        assert!(!plain(&math_tspans("x^2 + y^2 = 2e3")) && !plain(&math_tspans("1.5e-4 x")));
        assert!(plain(&math_tspans("2*e^3")) && plain(&math_tspans("y = e^x")) && plain(&math_tspans("x^2e3")));
        // a literal after ^ is said so
        assert!(warns("y = x^2e3").iter().any(|w| w.contains("after ^")), "{:?}", warns("y = x^2e3"));
        assert!(!warns("y = 2e3 x").iter().any(|w| w.contains("after ^")));
        // x^2 + y^2 = 2e1 is a circle of radius sqrt(20)
        assert!(!curve_lines(&with("x^2 + y^2 = 2e1", &[])).is_empty());
    }

    /// An excluded boundary on an axis (x > 0 over the y-axis, y > 0 over the x-axis) is drawn over a white cut in the axis, so
    /// the dashes show, in Colour and in Print.
    #[test]
    fn an_excluded_boundary_on_an_axis_shows_its_dashes() {
        for style in ["Colour", "Print"] {
            for row in ["x > 0", "y > 0", "x^2 + y^2 < 25; x > 0"] {
                let mut set = q(row);
                set.push(("style", Value::S(style.into())));
                let svg = grid_svg(&set);
                let cut = svg.find("stroke=\"#fff\" stroke-width=\"4\" stroke-line").unwrap_or_else(|| panic!("{style} {row}: no cut in the axis"));
                let dashed = svg[cut..].find("stroke-dasharray=\"9 6\"").unwrap_or_else(|| panic!("{style} {row}: no dashes"));
                assert!(dashed > 0);
            }
        }
        // an included one is solid and needs no cut
        assert!(!grid_svg(&q("x >= 0")).contains("stroke=\"#fff\" stroke-width=\"4\" stroke-line"));
    }

    /// A conic with no real points anywhere says so; one that only lies outside the window says that.
    #[test]
    fn empty_and_outside_conics_are_told_apart() {
        assert!(warns("x^2 + y^2 = -4").iter().any(|w| w.contains("no real points satisfy this")), "{:?}", warns("x^2 + y^2 = -4"));
        assert!(warns("x^2 + y^2 < -4").iter().any(|w| w.contains("no real points satisfy this")), "{:?}", warns("x^2 + y^2 < -4"));
        let w = warns("x^2 + y^2 = 400");
        assert!(w.iter().any(|w| w.contains("No part of this curve")) && !w.iter().any(|w| w.contains("no real points")), "{w:?}");
        // the explicit row of a curve beyond the window: no clamped scraps, a note
        let svg = grid_svg(&q("y = 400"));
        assert!(curve_lines_min(&svg, 0).is_empty());
        assert!(warns("y = 400").iter().any(|w| w.contains("No part of this curve")));
    }

    /// The Print grid is a mid grey at least a pixel thick, with lighter minor lines; the hatch is darker and heavier than the grid.
    #[test]
    fn print_grid_and_hatch_inks() {
        let svg = grid_svg(&{ let mut s = q("x^2 + y^2 < 25; y > x"); s.push(("style", Value::S("Print".into()))); s.push(("minor", Value::I(2))); s });
        assert!(svg.contains("stroke=\"#5a5a5a\" stroke-width=\"1.4\"") && svg.contains("#aaaaaa"), "mid-grey grid, lighter minor lines");
        assert!(svg.contains("stroke=\"#555555\" stroke-width=\"2\""), "hatch heavier than the grid");
        assert!(!svg.contains("#9b9b9b") && !svg.contains("stroke-width=\"0.8\" stroke=\"#5a5a5a\""));
    }

    #[test]
    fn many_points_are_capped_and_a_wide_window_is_warned_about() {
        let pts = (0..100_000).map(|k| format!("({}, {})", k % 10, k % 7)).collect::<Vec<_>>().join("; ");
        let mut set = q("");
        set.push(("points", Value::S(pts.clone())));
        let svg = grid_svg(&set);
        assert!(svg.matches("<circle").count() <= MAX_POINTS + 10, "{} circles", svg.matches("<circle").count());
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        let mut p = Params::defaults(&(d.params)());
        p.set_s("points", &pts);
        assert!(grid_warnings(&p).iter().any(|w| w.contains("Only the first 500 points")));
        assert!(super::super::interact::handles_for("coordinate-grid", &p, (600.0, 600.0)).len() <= 2 * MAX_POINTS + 130);
        let mut wide = Params::defaults(&(d.params)());
        for (k, v) in [("xmin", Value::F(-50.0)), ("xmax", Value::F(50.0)), ("ymin", Value::F(-5.0)), ("ymax", Value::F(5.0)), ("square", Value::B(true))] { wide.0.insert(k.into(), v); }
        assert!(grid_warnings(&wide).iter().any(|w| w.contains("Square cells")));
        wide.0.insert("square".into(), Value::B(false));
        assert!(!grid_warnings(&wide).iter().any(|w| w.contains("Square cells")));
    }

    #[test]
    fn tick_numbers_come_after_the_curves_and_shading_goes_under_the_grid() {
        let svg = with("x^2 + y^2 < 25; x^2 + y^2 = 16", &[]);
        let (shade, grid, curve, num) = (svg.find("fill-opacity=\"0.18\"").unwrap(), svg.find("<line").unwrap(), svg.find("<polyline").unwrap(), svg.rfind("data-num=\"1\"").unwrap());
        let plate = svg.find("fill=\"#fff\" stroke=\"#fff\" stroke-width=\"4\"").unwrap();
        assert!(plate < curve, "the white plates behind the numbers are under the curves, so no curve is cut");
        assert!(shade < grid, "shading is under the gridlines");
        assert!(curve < num, "numbers are drawn over the curves, with a white halo");
        // a plain figure keeps its old order
        let plain = grid_svg(&q("y = x"));
        assert!(plain.find("font-weight=\"bold\" fill=\"#111827\" stroke=\"#fff\"").unwrap() < plain.find("<polyline").unwrap());
    }

    /// The tick numbers of a figure: (left, right, baseline, text), the white halo plates left out.
    fn tick_numbers(svg: &str) -> Vec<(f64, f64, f64, String)> {
        let cw = |t: &str| t.chars().map(|c| match c { '-' | '\u{2212}' => 4.7, '.' | ',' => 3.9, c if c.is_ascii_digit() => 7.8, _ => 9.0 }).sum::<f64>();
        svg.split("<text ").skip(1).filter_map(|t| {
            let tag = t.split('>').next()?;
            if !tag.contains("font-family=\"Arial") || !tag.contains("font-size=\"14\"") || tag.contains("fill=\"#fff\"") { return None; }
            let attr = |k: &str| tag.split(&format!("{k}=\"")).nth(1).and_then(|r| r.split('"').next()).map(|s| s.to_string());
            let (x, y): (f64, f64) = (attr("x")?.parse().ok()?, attr("y")?.parse().ok()?);
            let txt = t.split('>').nth(1)?.split('<').next()?.to_string();
            let w = cw(&txt);
            let (a, b) = match attr("text-anchor")?.as_str() { "end" => (x - w, x), "start" => (x, x + w), _ => (x - w / 2.0, x + w / 2.0) };
            Some((a, b, y, txt))
        }).collect()
    }
    /// Numbers on one row of the axis, left to right, never closer than `gap` px.
    fn assert_numbers_clear(svg: &str, gap: f64, what: &str) {
        let nums = tick_numbers(svg);
        let mut rows: std::collections::BTreeMap<i64, Vec<(f64, f64, String)>> = Default::default();
        for (a, b, y, t) in nums { rows.entry(y.round() as i64).or_default().push((a, b, t)); }
        let row = rows.values().max_by_key(|r| r.len()).expect("numbers");
        let mut row = row.clone();
        row.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap());
        assert!(row.len() >= 3, "{what}: only {} numbers", row.len());
        for w in row.windows(2) { assert!(w[1].0 - w[0].1 >= gap, "{what}: \"{}\" and \"{}\" are {:.1} px apart", w[0].2, w[1].2, w[1].0 - w[0].1); }
    }

    /// A wide window thins its numbers (every 2nd, 5th, 10th ...) so that no two touch; a window whose numbers merely sit close
    /// is left alone, and the origin 0 never runs into a 1 or a -1.
    #[test]
    fn wide_windows_thin_their_numbers_and_the_origin_zero_keeps_clear() {
        for (half, eq) in [(20.0, ""), (20.0, "x^2 + y^2 = 16"), (40.0, ""), (100.0, "y = 1/x"), (10.0, "x^2 + y^2 = 16; y = x"), (10.0, "x^2/9 - y^2/4 = 1")] {
            for style in ["Colour", "Print"] {
                for square in [true, false] {
                    let mut set = q(eq);
                    for (k, v) in [("xmin", Value::F(-half)), ("xmax", Value::F(half)), ("ymin", Value::F(-half)), ("ymax", Value::F(half)), ("square", Value::B(square)), ("style", Value::S(style.into()))] { set.push((k, v)); }
                    let svg = grid_svg(&set);
                    // 4 px: the window of -10 to 10 on a plain grid has always had numbers 3 px apart ("-10" beside "-9"), which stays as it was
                    assert_numbers_clear(&svg, if half > 10.0 { 6.0 } else { 3.0 }, &format!("+-{half} {eq:?} {style} square {square}"));
                }
            }
        }
        // the 0 of a 21 by 21 window with a conic keeps 6 px from its neighbours
        let svg = grid_svg(&q("x^2 + y^2 = 16"));
        assert_numbers_clear(&svg, 5.0, "21 by 21 with a conic");
    }

    #[test]
    fn the_wide_window_warning_names_the_real_control() {
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        assert!((d.params)().iter().any(|p| p.label == "Square cells"));
        let mut wide = Params::defaults(&(d.params)());
        for (k, v) in [("xmin", Value::F(-50.0)), ("xmax", Value::F(50.0)), ("ymin", Value::F(-5.0)), ("ymax", Value::F(5.0)), ("square", Value::B(true))] { wide.0.insert(k.into(), v); }
        let w = grid_warnings(&wide);
        assert!(w.iter().any(|x| x.contains("Square cells")) && !w.iter().any(|x| x.contains("Equal scale")), "{w:?}");
    }

    /// Logarithms run on to the window edge (they leave it at an even pace, which used to lose the tail): at most 1% of the true
    /// points more than 2 px off, none more than 6 px.
    #[test]
    fn logarithms_reach_the_window_edge() {
        type Case = (&'static str, (f64, f64, f64, f64), Box<dyn Fn(f64) -> Vec<f64>>);
        let cases: Vec<Case> = vec![
            ("y = ln(x)", (-5.0, 5.0, -10.0, 10.0), Box::new(|y| vec![y.exp()])),
            ("y = ln(x)", (-1.0, 20.0, -10.0, 10.0), Box::new(|y| vec![y.exp()])),
            ("y = ln(x-1)", (-5.0, 10.0, -8.0, 8.0), Box::new(|y| vec![1.0 + y.exp()])),
            ("y = log(x)", (-5.0, 5.0, -3.0, 3.0), Box::new(|y| vec![10f64.powf(y)])),
            ("y = ln(abs(x))", (-5.0, 5.0, -5.0, 5.0), Box::new(|y| vec![y.exp(), -y.exp()])),
        ];
        for (eq, (x0, x1, y0, y1), xs_of_y) in cases {
            for square in [true, false] {
                let mut set = q(eq);
                for (k, v) in [("xmin", Value::F(x0)), ("xmax", Value::F(x1)), ("ymin", Value::F(y0)), ("ymax", Value::F(y1)), ("square", Value::B(square))] { set.push((k, v)); }
                let mut pts = vec![];
                for j in 0..6000 { let y = y0 + (y1 - y0) * (j as f64 + 0.41) / 6000.0; for x in xs_of_y(y) { pts.push((x, y)); } }
                let (total, far, worst, bad) = cover(&set, &pts);
                assert!(total > 100, "{eq} {x0}..{x1}: only {total} true points in the window");
                assert!(far as f64 <= 0.01 * total as f64, "{eq} {x0}..{x1} square {square}: {far} of {total} true points are over 2 px off, e.g. {bad:?}");
                assert!(worst <= 6.0, "{eq} {x0}..{x1} square {square}: a true point is {worst} px off, e.g. {bad:?}");
            }
        }
    }

    /// The tags of the figure drawn as an asymptote (the dotted grey of the Colour style).
    fn asymptote_tags(svg: &str) -> usize { svg.split('<').filter(|t| t.contains("stroke=\"#6b7280\"") && t.contains("stroke-dasharray=\"2 4\"")).count() }

    /// A line is an asymptote when the curve comes ever closer to it, whichever way the curve is drawn (solved for y or not); a
    /// parallel line or a tangent is not one.
    #[test]
    fn asymptotes_are_found_for_solved_curves_and_parallels_are_not_asymptotes() {
        for rows in ["x*y = 4; x = 0", "x*y = 4; y = 0", "(x-1)*(y-2) = 4; x = 1", "(x-1)*(y-2) = 4; y = 2", "y*(x-1) = 4; x = 1", "y*(x-1) = 4; y = 0", "y = 1/x; x = 0", "y = 1/x; y = 0", "y = x + 1/x; y = x", "y = 1/(x^2-1); y = 0", "y = ln(x); x = 0", "x^2/9 - y^2/4 = 1; y = 2x/3"] {
            assert_eq!(asymptote_tags(&grid_svg(&q(rows))), 1, "{rows}: the line is an asymptote");
        }
        for rows in ["(x-y)^2 = 4; y = x", "(x-y)^2 = 4; y = x + 1", "(x+y)^2 = 9; y = -x", "x*y = 4; x = 1", "x*y = 4; y = 1", "y = 1/x; y = x", "x^2 + y^2 = 25; x = 5", "x^2 + y^2 = 25; y = 0", "y = ln(x); y = 0", "y = x^2; y = 0"] {
            assert_eq!(asymptote_tags(&grid_svg(&q(rows))), 0, "{rows}: not an asymptote");
        }
    }

    /// An excluded boundary on an axis is cut out of the axis in every kind of row that can have one.
    #[test]
    fn every_excluded_boundary_on_an_axis_shows_its_dashes() {
        for style in ["Colour", "Print"] {
            for row in ["0 < y < 3", "-3 < y < 0", "0 < y <= 3", "x*y > 0", "x*y < 0", "-3 < x < 0", "x^2 + y^2 < 25; y > 0", "y > 0, x > 0"] {
                let mut set = q(row);
                set.push(("style", Value::S(style.into())));
                let svg = grid_svg(&set);
                let cut = svg.find("stroke=\"#fff\" stroke-width=\"4\" stroke-line").unwrap_or_else(|| panic!("{style} {row}: no cut in the axis"));
                assert!(svg[cut..].contains("stroke-dasharray=\"9 6\""), "{style} {row}: no dashes over the cut");
            }
        }
    }

    /// x2e3 is read as x times 2000 and set that way; xe and 2ex are products with Euler's e, set in italic.
    #[test]
    fn labels_follow_the_reader_for_numbers_next_to_letters() {
        assert_eq!(eval("x2e3", 2.0), Some(4000.0));
        assert!((eval("xe", 2.0).unwrap() - 2.0 * std::f64::consts::E).abs() < 1e-12);
        assert!((eval("2ex", 3.0).unwrap() - 6.0 * std::f64::consts::E).abs() < 1e-12, "{:?}", eval("2ex", 3.0));
        let italic_e = |t: &str| math_tspans(&label_text(t)).contains("font-style=\"italic\">e<");
        assert!(!italic_e("x2e3 = y"), "{}", math_tspans(&label_text("x2e3 = y")));
        assert!(italic_e("xe = y") && italic_e("2ex = y") && italic_e("y = e^x"));
        assert!(math_tspans(&label_text("2ex")).contains("font-style=\"italic\">x<"));
    }

    /// Several ways of having no common region all say so, and draw nothing.
    #[test]
    fn empty_common_regions_say_so_and_draw_no_slivers() {
        for (rows, what) in [("x^2 < 0", "no real points"), ("x^2 + y^2 < 4; x^2 + y^2 > 9", "no points satisfy all"), ("x^2 + y^2 <= 25; x^2 + y^2 >= 25", "no points satisfy all"), ("x^2 + y^2 < 25; x^2 + y^2 > 25", "no points satisfy all"), ("x^2 + y^2 < -4", "no real points")] {
            let w = warns(rows);
            assert!(w.iter().any(|t| t.contains(what)), "{rows}: {w:?}");
        }
        let svg = grid_svg(&q("x^2 + y^2 <= 25; x^2 + y^2 >= 25"));
        assert!(!svg.contains(GROUP_TINT), "no sliver is drawn as a region");
        // a real common region is still shaded and not complained of
        assert!(warns("x^2 + y^2 < 25; x^2 + y^2 > 4").is_empty());
        assert!(grid_svg(&q("x^2 + y^2 < 25; x^2 + y^2 > 4")).contains(GROUP_TINT));
    }

    /// Print rows are heavier and darker than the grid on every row; the Colour grid is lighter than the curves.
    #[test]
    fn print_rows_outweigh_the_grid_and_the_colour_grid_gives_way_to_conics() {
        for lw in ["Thin", "Normal", "Thick"] {
            for grid in ["Standard", "Bold", "Light"] {
                let mut set = q("x^2 + y^2 = 4; x^2 + y^2 = 9; y = x; y = 3; x^2/16 + y^2/4 = 1");
                set.extend([("style", Value::S("Print".into())), ("line_width", Value::S(lw.into())), ("grid", Value::S(grid.into()))]);
                let svg = grid_svg(&set);
                let gw = match grid { "Bold" => 1.4, _ => 1.0 };
                let rows: Vec<(f64, String)> = svg.split("<polyline ").skip(1).map(|t| t.split("/>").next().unwrap()).filter(|t| !t.contains("stroke-dasharray")).map(|t| {
                    let a = |k: &str| t.split(&format!("{k}=\"")).nth(1).unwrap().split('"').next().unwrap().to_string();
                    (a("stroke-width").parse().unwrap(), a("stroke"))
                }).collect();
                assert!(rows.len() >= 5, "{lw} {grid}: {} rows", rows.len());
                for (w, ink) in rows {
                    assert!(w >= 1.8 * gw - 1e-9, "{lw} {grid}: a row of {w} px beside a {gw} px grid");
                    let v = u8::from_str_radix(&ink[1..3], 16).unwrap();
                    assert!(v <= 0x44, "{lw} {grid}: ink {ink} is lighter than #444444");
                }
            }
        }
        assert!(grid_svg(&q("x^2 + y^2 = 16")).contains("stroke=\"#8794a3\""));
        assert!(!grid_svg(&q("y = x")).contains("#8794a3"));
    }

    /// The student copy shades only when asked to; the key always does.
    #[test]
    fn the_student_copy_keeps_the_shading_only_when_asked() {
        for style in ["Colour", "Print"] {
            let shaded = |answers: bool, keep: bool| {
                let mut set = q("x^2 + y^2 < 25; y > x - 3");
                set.extend([("style", Value::S(style.into())), ("show_answers", Value::B(answers)), ("student_shading", Value::B(keep))]);
                let svg = grid_svg(&set);
                svg.contains(GROUP_TINT) || svg.contains("fill=\"url(#shg)\"")
            };
            assert!(shaded(true, false), "{style}: key");
            assert!(!shaded(false, false), "{style}: student copy has no shading");
            assert!(shaded(false, true), "{style}: student copy keeps it when asked");
        }
    }

    /// Curve labels are spaced alike whatever was typed.
    #[test]
    fn curve_labels_are_spaced_alike() {
        assert_eq!(tidy_spacing("x^2+y^2=16"), "x^2 + y^2 = 16");
        assert_eq!(tidy_spacing("x^2   +y^2= 16"), "x^2 + y^2 = 16");
        assert_eq!(tidy_spacing("x^2+y^2<=25"), "x^2 + y^2 <= 25");
        assert_eq!(tidy_spacing("x^2+y^2=-4"), "x^2 + y^2 = -4");
        assert_eq!(tidy_spacing("y=2e-3x"), "y = 2e-3x");
        assert_eq!(tidy_spacing("x^2+y^2=25,x>0"), "x^2 + y^2 = 25, x > 0");
        let a = with("x^2+y^2=16", &[]);
        let b = with("x^2   +   y^2 = 16", &[]);
        assert_eq!(a, b);
    }

    /// (left, right, top, bottom, text, plate_over) of every tick number, the white plates left out.
    fn number_boxes(svg: &str) -> Vec<(f64, f64, f64, f64, String, bool)> {
        let cw = |t: &str| t.chars().map(|c| match c { '-' | '\u{2212}' => 4.7, '.' | ',' => 3.9, c if c.is_ascii_digit() => 7.8, _ => 9.0 }).sum::<f64>();
        svg.split("<text ").skip(1).filter_map(|t| {
            let tag = t.split('>').next()?;
            if !tag.contains("font-family=\"Arial") || !tag.contains("font-size=\"14\"") || tag.contains("fill=\"#fff\"") { return None; }
            let attr = |k: &str| tag.split(&format!("{k}=\"")).nth(1).and_then(|r| r.split('"').next()).map(|s| s.to_string());
            let (x, y): (f64, f64) = (attr("x")?.parse().ok()?, attr("y")?.parse().ok()?);
            let txt = t.split('>').nth(1)?.split('<').next()?.to_string();
            let w = cw(&txt);
            let (a, b) = match attr("text-anchor")?.as_str() { "end" => (x - w, x), "start" => (x, x + w), _ => (x - w / 2.0, x + w / 2.0) };
            Some((a, b, y - 10.0, y + 0.5, txt, tag.contains("data-plate=\"over\"")))
        }).collect()
    }
    fn example_set(i: usize, style: &str) -> Vec<(&'static str, Value)> {
        let exs = examples_b_plots("coordinate-grid").unwrap();
        let mut set: Vec<(&'static str, Value)> = vec![];
        for (k, v) in &exs[i].params { set.push((k, v.clone())); }
        set.push(("style", Value::S(style.into())));
        set
    }

    /// No tick number is struck by a curve: its box meets no drawn curve, or (when no free spot exists) its plate is painted over
    /// the curve and marked so.
    #[test]
    fn tick_numbers_are_clear_of_every_curve() {
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![];
        for style in ["Colour", "Print"] {
            for eq in ["x^2 + y^2 = 25", "x^2/9 - y^2/4 = 1", "4 < x^2 + y^2 < 25", "x^2/16 + y^2/9 = 1", "x^2 + y^2 > 4; x^2 + y^2 < 25"] {
                let mut set = q(eq);
                set.push(("style", Value::S(style.into())));
                cases.push((format!("{eq} {style}"), set));
            }
            for i in [10usize, 11, 17] { cases.push((format!("example {i} {style}"), example_set(i, style))); }
        }
        for (name, set) in cases {
            let svg = grid_svg(&set);
            let near = Near::new(&curve_lines_min(&svg, 1));
            for (a, b, t, bt, txt, plate_over) in number_boxes(&svg) {
                if plate_over { continue; }
                for i in 0..=((b - a) as usize + 1) { for j in 0..=((bt - t) as usize + 1) {
                    let (px, py) = (a + i as f64, t + j as f64);
                    assert!(near.dist((px, py)) > 0.5, "{name}: the curve runs through \"{txt}\" at ({px:.0}, {py:.0})");
                } }
            }
        }
    }

    /// No two tick numbers overlap, origin zero included, in the windows teachers use.
    #[test]
    fn no_two_tick_numbers_overlap() {
        let wins: Vec<(f64, f64, f64, f64)> = vec![(-10.0, 10.0, -10.0, 10.0), (-20.0, 20.0, -20.0, 20.0), (-50.0, 50.0, -50.0, 50.0), (-100.0, 100.0, -100.0, 100.0), (0.0, 20.0, -4.0, 4.0), (-20.0, 20.0, -20.0, 20.0)];
        for (x0, x1, y0, y1) in wins {
            for eq in ["y = x", "y = ln(x)", "y = x^2/20", "x^2 + y^2 = 25"] {
                // (Colour windows without a conic keep the numbers where the earlier figures had them, which sit 2 px close at the origin)
                for style in ["Colour", "Print"] {
                    if style == "Colour" && !eq.contains("y^2") { continue; }
                    let mut set = q(eq);
                    for (k, v) in [("xmin", x0), ("xmax", x1), ("ymin", y0), ("ymax", y1)] { set.push((k, Value::F(v))); }
                    set.push(("style", Value::S(style.into())));
                    let svg = grid_svg(&set);
                    let bx = number_boxes(&svg);
                    for i in 0..bx.len() { for j in i + 1..bx.len() {
                        let (p, r) = (&bx[i], &bx[j]);
                        assert!(!(p.0 < r.1 && r.0 < p.1 && p.2 < r.3 && r.2 < p.3), "{eq} {style} {x0}..{x1} x {y0}..{y1}: \"{}\" and \"{}\" overlap", p.4, r.4);
                    } }
                }
            }
        }
    }

    /// A very wide window still gets readable numbers (every 100th ...) and a grid that is not a smear.
    #[test]
    fn very_wide_windows_thin_numbers_and_gridlines() {
        for half in [200.0, 1000.0] {
            let mut set = q("y = x");
            // (Print: the coarser grid and the number thinning are for Print and curve figures; a plain Colour grid draws every line)
            set.push(("style", Value::S("Print".into())));
            for (k, v) in [("xmin", -half), ("xmax", half), ("ymin", -half), ("ymax", half)] { set.push((k, Value::F(v))); }
            let svg = grid_svg(&set);
            assert_numbers_clear(&svg, 5.0, &format!("+-{half}"));
            let n = number_boxes(&svg).len();
            assert!(n >= 6 && n <= 60, "+-{half}: {n} numbers");
            let lines = svg.matches("<line").count();
            assert!(lines < 400, "+-{half}: {lines} lines");
            // the figure now has 6 px between numbers on the other axes too, with step 100 on y
            set.push(("ystep", Value::F(100.0)));
            assert!(number_boxes(&grid_svg(&set)).len() < 120);
        }
    }

    /// The Print grid is a continuous black line on a bitonal copy at 1x and 2x, at every threshold from 50 to 75%.
    #[test]
    fn the_print_grid_is_continuous_on_a_bitonal_copy() {
        for scale in [1.0f32, 2.0] {
            let sc = scale as f64;
            let mut set = q("y = x");
            set.push(("style", Value::S("Print".into())));
            let g = grid_layout_of(&set);
            let svg = grid_svg(&set);
            let (w, _h, px) = super::super::to_rgba(&svg, scale).unwrap();
            let lum = |x: usize, y: usize| { let c = &px[(y * w as usize + x) * 4..]; let a = c[3] as f64 / 255.0; (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a) };
            for thr in [0.50, 0.58, 0.66, 0.75] {
                // a stretch of plot above the curve and clear of numbers: each vertical gridline must be dark all along it
                let (top, bottom) = (((g.mt + 4.0) * sc) as usize, ((g.mt + 3.0 * g.uy) * sc) as usize);
                for k in 1..=19 {
                    let gx = (g.ml + k as f64 * g.ux) * sc;
                    if (k - 10) % 10 == 0 { continue; } // the axis
                    let col = (-2..=2).map(|d| (gx as i64 + d) as usize).max_by_key(|&x| (top..bottom).filter(|&y| lum(x, y) < thr * 255.0).count()).unwrap();
                    let n = (top..bottom).filter(|&y| lum(col, y) < thr * 255.0).count();
                    assert!(n == bottom - top, "scale {scale} at {thr}: gridline {k} is dark on {n}/{} pixels", bottom - top);
                }
            }
        }
    }

    /// Windows with thousands of tick numbers (Square cells in a wide strip) stay fast: the number search is local, not quadratic.
    #[test]
    fn thousands_of_numbers_do_not_make_the_render_slow() {
        let win = |x: f64, y: f64| -> Vec<(&'static str, Value)> { vec![("xmin", Value::F(-x)), ("xmax", Value::F(x)), ("ymin", Value::F(-y)), ("ymax", Value::F(y))] };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![];
        for style in ["Colour", "Print"] {
            let mut w = win(1000.0, 1.0); w.push(("style", Value::S(style.into()))); cases.push(("x^2 + y^2 = 640000".into(), w.clone())); cases.push(("".into(), w));
            let mut w = win(100000.0, 1.0); w.push(("style", Value::S(style.into())));
            cases.push((vec!["x^2 + y^2 = 1"; 40].join("; "), w));
            let mut w = win(1e6, 10.0); w.push(("style", Value::S(style.into()))); w.push(("xunits", Value::S("Numbers".into()))); cases.push(("x^2 + y^2 = 4".into(), w));
        }
        for (eq, extra) in cases {
            let (ms, _) = timed(&eq, &extra);
            // (release keeps the 2 s bound; a debug build runs this code about ten times slower, as the sibling timing tests allow)
            let limit = if cfg!(debug_assertions) { 20_000 } else { 2_000 };
            assert!(ms < limit, "{eq:?} {extra:?}: {ms} ms of CPU");
        }
    }

    /// Small circles keep their whole outline: no white plate covers any part of a curve (a number over a curve has only a thin halo).
    #[test]
    fn small_circles_are_not_chopped_by_number_plates() {
        let cw = |t: &str| t.chars().map(|c| match c { '-' | '\u{2212}' => 4.7, '.' | ',' => 3.9, c if c.is_ascii_digit() => 7.8, _ => 9.0 }).sum::<f64>();
        let rows = ["x^2 + y^2 = 1", "x^2 + y^2 = 4", "x^2 + y^2 = 9", "x^2 + y^2 = 1; x^2 + y^2 = 4; x^2 + y^2 = 9; x^2 + y^2 = 16", "x^2 + y^2 = 25"];
        for style in ["Colour", "Print"] { for r in rows {
            let mut set = q(r);
            for (k, v) in [("xmin", -10.0), ("xmax", 10.0), ("ymin", -10.0), ("ymax", 10.0)] { set.push((k, Value::F(v))); }
            set.push(("style", Value::S(style.into())));
            let svg = grid_svg(&set);
            let near = Near::new(&curve_lines_min(&svg, 1));
            for t in svg.split("<text ").skip(1) {
                let tag = t.split('>').next().unwrap();
                let attr = |k: &str| tag.split(&format!("{k}=\"")).nth(1).and_then(|r| r.split('"').next()).map(|s| s.to_string());
                // a number drawn over a curve has at most a thin white halo (digits carry data-num; a plate is a white-filled text)
                if tag.contains("data-num") && tag.contains("stroke=\"#fff\"") {
                    let sw: f64 = attr("stroke-width").unwrap().parse().unwrap();
                    assert!(sw <= 2.5, "{style} {r}: a number over a curve has a {sw} px white halo");
                }
                if !(tag.contains("fill=\"#fff\"") && tag.contains("stroke=\"#fff\"")) || !tag.contains("font-size=\"14\"") { continue; }
                let halo: f64 = attr("stroke-width").unwrap().parse().unwrap();
                let (x, y): (f64, f64) = (attr("x").unwrap().parse().unwrap(), attr("y").unwrap().parse().unwrap());
                let w = cw(t.split('>').nth(1).unwrap().split('<').next().unwrap());
                let (a, b) = match attr("text-anchor").unwrap().as_str() { "end" => (x - w, x), "start" => (x, x + w), _ => (x - w / 2.0, x + w / 2.0) };
                let (a, b) = (a - halo / 2.0, b + halo / 2.0);
                for i in 0..=((b - a) as usize) { for j in 0..=(11.0 + halo) as usize {
                    let pt = (a + i as f64, y - 10.0 - halo / 2.0 + j as f64);
                    assert!(near.dist(pt) > 0.5, "{style} {r}: a white plate covers the curve at {pt:?}");
                } }
            }
        } }
    }

    /// A Print asymptote survives a bitonal photocopy (at 2x): along its dashes most pixels stay dark at 58%, 66% and 75%.
    #[test]
    fn print_asymptotes_survive_a_bitonal_copy() {
        for ex in [12usize, 16] {
            let set = example_set(ex, "Print");
            let svg = grid_svg(&set);
            let (w, _h, px) = super::super::to_rgba(&svg, 2.0).unwrap();
            let lum = |x: i64, y: i64| { if x < 0 || y < 0 || x >= w as i64 || (y * w as i64 + x) as usize * 4 + 3 >= px.len() { return 255.0; } let c = &px[((y * w as i64 + x) * 4) as usize..]; let a = c[3] as f64 / 255.0; (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a) };
            let paths: Vec<Vec<(f64, f64)>> = svg.split("<polyline ").skip(1).map(|t| t.split("/>").next().unwrap()).filter(|t| t.contains("stroke-dasharray=\"7 5\"")).map(|t| {
                let pts = t.split("points=\"").nth(1).unwrap().split('"').next().unwrap();
                pts.split_whitespace().map(|p| { let mut it = p.split(','); (it.next().unwrap().parse().unwrap(), it.next().unwrap().parse().unwrap()) }).collect()
            }).collect();
            assert!(!paths.is_empty(), "example {ex}: no Print asymptote drawn");
            for thr in [0.58, 0.66, 0.75] {
                let (mut on, mut dark) = (0usize, 0usize);
                for path in &paths {
                    let mut s_len = 0.0f64;
                    for wv in path.windows(2) {
                        let len = ((wv[1].0 - wv[0].0).powi(2) + (wv[1].1 - wv[0].1).powi(2)).sqrt();
                        let mut t = 0.0;
                        while t < len {
                            if (s_len + t) % 12.0 < 7.0 {
                                let (x, y) = (((wv[0].0 + (wv[1].0 - wv[0].0) * t / len) * 2.0).round() as i64, ((wv[0].1 + (wv[1].1 - wv[0].1) * t / len) * 2.0).round() as i64);
                                on += 1;
                                if (-1..=1).any(|dy| (-1..=1).any(|dx| lum(x + dx, y + dy) < thr * 255.0)) { dark += 1; }
                            }
                            t += 0.5;
                        }
                        s_len += len;
                    }
                }
                assert!(on > 100 && dark as f64 >= 0.6 * on as f64, "example {ex} at {thr}: only {dark}/{on} asymptote pixels survive");
            }
        }
    }

    /// The number rule, over every window shape (half-widths 2 to 30 on both axes, Square on and off, three conics, Colour and Print):
    /// (a) each number sits within 1 px of its own tick, all x numbers in one row and all y numbers in one column, no leader lines;
    /// (b) no two number boxes overlap (the same width estimator as the code); (c) the numbers shown on an
    /// axis are symmetric about 0 and have no holes; (d) where the numbers are at least 26 px apart nothing is thinned (the count
    /// equals the plain figure's); (g) the editor's thinning note appears exactly when an axis shows fewer numbers than it has ticks;
    /// and every shown number has a gridline under it.
    #[test]
    fn number_rule_holds_in_every_window_shape() {
        let rows = ["x^2 + y^2 = 25", "x^2 - y^2 = 1", "x^2/16 + y^2/9 = 1"];
        let halves = [2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 15.0, 20.0, 30.0];
        let attr = |tag: &str, k: &str| tag.split(&format!("{k}=\"")).nth(1).and_then(|r| r.split('"').next()).and_then(|s| s.parse::<f64>().ok());
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        for style in ["Colour", "Print"] { for square in [true, false] { for r in rows { for hx in halves { for hy in halves {
            let mut set = q(r);
            for (k, v) in [("xmin", -hx), ("xmax", hx), ("ymin", -hy), ("ymax", hy)] { set.push((k, Value::F(v))); }
            set.push(("style", Value::S(style.into())));
            set.push(("square", Value::B(square)));
            let name = format!("{style} {r} x+-{hx} y+-{hy} square {square}");
            let g = grid_layout_of(&set);
            let svg = grid_svg(&set);
            assert!(!svg.contains("stroke=\"#808080\""), "{name}: a leader line");
            let (mut vx, mut hyl) = (vec![], vec![]);
            for t in svg.split("<line ").skip(1) {
                let tag = t.split("/>").next().unwrap();
                if let (Some(x1), Some(x2), Some(y1), Some(y2)) = (attr(tag, "x1"), attr(tag, "x2"), attr(tag, "y1"), attr(tag, "y2")) {
                    if (x1 - x2).abs() < 1e-6 { vx.push(x1); } else if (y1 - y2).abs() < 1e-6 { hyl.push(y1); }
                }
            }
            let mut axes: [Vec<f64>; 2] = [vec![], vec![]];
            let (mut xrow, mut ycol): (Vec<f64>, Vec<f64>) = (vec![], vec![]);
            for t in svg.split("<text ").skip(1) {
                let tag = t.split('>').next().unwrap();
                if !tag.contains("data-num") || tag.contains("data-axis=\"2\"") { continue; }
                let v: f64 = match t.split('>').nth(1).unwrap().split('<').next().unwrap().replace('\u{2212}', "-").parse() { Ok(v) => v, Err(_) => continue };
                let ax = if tag.contains("data-axis=\"0\"") { 0 } else { 1 };
                axes[ax].push(v);
                let (x, y) = (attr(tag, "x").unwrap(), attr(tag, "y").unwrap());
                if ax == 0 {
                    xrow.push(y);
                    assert!((x - (g.ml + (v - g.x0) * g.ux)).abs() <= 1.0, "{name}: x number {v} is off its tick");
                    if v != 0.0 { assert!(vx.iter().any(|l| (l - x).abs() < 0.6), "{name}: x tick {v} has no gridline"); }
                } else {
                    ycol.push(x);
                    assert!((y - 4.1 - (g.mt + g.ph - (v - g.y0) * g.uy)).abs() <= 1.0, "{name}: y number {v} is off its tick");
                    if v != 0.0 { assert!(hyl.iter().any(|l| (l - (y - 4.1)).abs() < 0.6), "{name}: y tick {v} has no gridline"); }
                }
            }
            assert!(ycol.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-6), "{name}: y numbers are not in one column");
            for (ax, half) in [(0usize, hx), (1usize, hy)] {
                let set_v = &axes[ax];
                for v in set_v { if v.abs() < half - 1e-9 { assert!(set_v.iter().any(|w| (w + v).abs() < 1e-9), "{name}: {v} shown on axis {ax} without {}", -v); } }
                let pos: Vec<f64> = set_v.iter().cloned().filter(|v| *v > 1e-9).collect();
                if let Some(s0) = pos.iter().cloned().reduce(f64::min) {
                    let top = pos.iter().cloned().fold(0.0f64, f64::max);
                    let mut m = s0;
                    while m <= top + 1e-9 { assert!(set_v.iter().any(|w| (w - m).abs() < 1e-9), "{name}: hole at {m} on axis {ax}: {set_v:?}"); m += s0; }
                }
            }
            let bx = number_boxes(&svg);
            for i in 0..bx.len() { for j in i + 1..bx.len() {
                let (p, r2) = (&bx[i], &bx[j]);
                assert!(!(p.0 < r2.1 && r2.0 < p.1 && p.2 < r2.3 && r2.2 < p.3), "{name}: \"{}\" and \"{}\" overlap", p.4, r2.4);
            } }
            // (the corner: only an x number at the y column may sit lower than the x row)
            let lowered = xrow.iter().filter(|r| **r > xrow.iter().cloned().fold(f64::MAX, f64::min) + 0.5).count();
            assert!(lowered <= 1, "{name}: {lowered} x numbers off the row");
            // (d) and (g)
            let full = |h: f64| 2 * (h as usize);
            let thinned = axes[0].len() < full(hx) || axes[1].len() < full(hy);
            let mut p = Params::defaults(&(d.params)());
            for (k, v) in &set { p.0.insert(k.to_string(), v.clone()); }
            assert_eq!(grid_warnings(&p).iter().any(|w| w.contains("axis numbers were thinned")), thinned, "{name}: the thinning note");
            if g.ux >= 26.0 { assert!(axes[0].len() == full(hx) || (hx as usize) * 2 > 40, "{name}: the x numbers were thinned at {} px", g.ux); }
            if g.uy >= 16.0 && g.ux >= 26.0 { assert!(axes[1].len() == full(hy), "{name}: the y numbers were thinned at {} px", g.uy); }
        } } } } }
    }

    /// The examples keep their numbers: a figure with curves shows as many tick numbers as the same window without them.
    #[test]
    fn examples_keep_their_numbers() {
        let mut bad: Vec<String> = vec![];
        for i in 0..18 { for style in ["Colour", "Print"] {
            let Some(exs) = examples_b_plots("coordinate-grid") else { break };
            if i >= exs.len() { continue; }
            let set = example_set(i, style);
            if grid_layout_of(&set).ux < 26.0 { continue; }
            let mut plain = set.clone();
            plain.retain(|(k, _)| *k != "equations" && *k != "style");
            plain.push(("equations", Value::S(String::new())));
            let (a, b) = (number_boxes(&grid_svg(&set)).len(), number_boxes(&grid_svg(&plain)).len());
            if a != b { bad.push(format!("example {i} {style}: {a} numbers, {b} expected")); }
        } }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Plain Colour figures without equations draw every gridline, as before any conic work, in any window (the counts are the
    /// ticks of the window: one line per step on each axis).
    #[test]
    fn plain_colour_grids_are_unchanged() {
        for half in [30.0, 50.0, 100.0, 1000.0] { for square in [true, false] { for minor in [0i64, 5] {
            if half > 100.0 && minor > 0 { continue; } // (minor lines stop at the 2100-line safety cap)
            let mut set: Vec<(&'static str, Value)> = vec![("xmin", Value::F(-half)), ("xmax", Value::F(half)), ("ymin", Value::F(-half)), ("ymax", Value::F(half)), ("square", Value::B(square)), ("minor", Value::I(minor))];
            set.push(("equations", Value::S(String::new())));
            let svg = grid_svg(&set);
            let crisp = svg.split("<g shape-rendering=\"crispEdges\"").nth(1).unwrap().split("</g>").next().unwrap();
            let major = crisp.matches("<line").count();
            assert_eq!(major, 2 * (2 * half as usize + 1), "+-{half} square {square}: major lines");
            let all = svg.matches("<line").count();
            let minors = if minor > 1 { 2 * (2 * half as usize * minor as usize + 1) } else { 0 };
            assert_eq!(all, major + minors + 2, "+-{half} square {square} minor {minor}: all lines");
        } } }
    }

    /// The editor's note says so when numbers were thinned, in a very wide Square window and not in an ordinary one.
    #[test]
    fn the_thinning_note_fires_for_crowded_windows_only() {
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        let notes = |w: [f64; 4]| { let mut p = Params::defaults(&(d.params)()); for (k, v) in [("xmin", w[0]), ("xmax", w[1]), ("ymin", w[2]), ("ymax", w[3])] { p.0.insert(k.into(), Value::F(v)); } p.0.insert("equations".into(), Value::S("x^2 + y^2 = 25".into())); p.0.insert("square".into(), Value::B(true)); grid_warnings(&p).iter().any(|m| m.contains("axis numbers were thinned")) };
        assert!(notes([-20.0, 20.0, -3.0, 3.0]));
        for w in [[-10.0, 10.0, -10.0, 10.0], [-11.0, 11.0, -11.0, 11.0], [-8.0, 8.0, -10.0, 10.0]] { assert!(!notes(w), "{w:?}"); }
    }

    /// Where the y numbers leave room (a window up to 10 by 10 at Square cells and any window with at least 24 px per unit) the x row is
    /// complete and only the corner x number drops below its row; in a tall window with 16 px rows nothing fits between the y numbers,
    /// so the x axis is thinned evenly (symmetric) and the editor says so. Nothing overlaps either way.
    #[test]
    fn corner_x_number_drops_below_its_row_and_tall_windows_thin_evenly() {
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        for (hx, hy, complete) in [(10.0, 10.0, true), (8.0, 10.0, true), (5.0, 8.0, true), (5.0, 15.0, false), (8.0, 15.0, false)] { for style in ["Colour", "Print"] { for r in ["x^2 + y^2 = 4", "x^2 + y^2 = 25", "x^2 - y^2 = 1", "x^2/16 + y^2/9 = 1"] {
            let mut set = q(r);
            for (k, v) in [("xmin", -hx), ("xmax", hx), ("ymin", -hy), ("ymax", hy)] { set.push((k, Value::F(v))); }
            set.push(("style", Value::S(style.into()))); set.push(("square", Value::B(true)));
            let g = grid_layout_of(&set);
            let svg = grid_svg(&set);
            let mut xs: Vec<f64> = vec![];
            for t in svg.split("<text ").skip(1) {
                let tag = t.split('>').next().unwrap();
                if !tag.contains("data-axis=\"0\"") { continue; }
                let v: f64 = t.split('>').nth(1).unwrap().split('<').next().unwrap().replace('\u{2212}', "-").parse().unwrap();
                let x: f64 = tag.split("x=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
                assert!((x - (g.ml + (v - g.x0) * g.ux)).abs() <= 1.0, "{r} {style}: {v} off its tick");
                xs.push(v);
            }
            let mut p = Params::defaults(&(d.params)());
            for (k, v) in &set { p.0.insert(k.to_string(), v.clone()); }
            let note = grid_warnings(&p).iter().any(|m| m.contains("axis numbers were thinned"));
            if complete { assert_eq!(xs.len(), 2 * hx as usize, "{r} {style} x+-{hx} y+-{hy}: {xs:?}"); assert!(!note); }
            else { assert!(xs.iter().all(|v| xs.contains(&-v)) && note, "{r} {style} x+-{hx} y+-{hy}: {xs:?}"); }
            let bx = number_boxes(&svg);
            for i in 0..bx.len() { for j in i + 1..bx.len() { let (p, r2) = (&bx[i], &bx[j]); assert!(!(p.0 < r2.1 && r2.0 < p.1 && p.2 < r2.3 && r2.2 < p.3), "{r} {style} x+-{hx} y+-{hy}: \"{}\" and \"{}\" overlap", p.4, r2.4); } }
        } } }
    }

    /// Every numbered tick has a gridline under it, however the grid was thinned; minor lines closer than 3 px are left out.
    #[test]
    fn numbered_ticks_always_have_a_gridline_and_dense_minor_lines_are_dropped() {
        for half in [100.0, 110.0, 120.0, 1000.0] {
            for every in ["Every line", "Every 2nd line", "Every 5th line", "Every 10th line"] {
                let mut set = q("y = x");
                set.push(("style", Value::S("Print".into())));
                for (k, v) in [("xmin", -half), ("xmax", half), ("ymin", -half), ("ymax", half)] { set.push((k, Value::F(v))); }
                set.push(("xnum", Value::S(every.into())));
                set.push(("ynum", Value::S(every.into())));
                let g = grid_layout_of(&set);
                let svg = grid_svg(&set);
                // gridline x positions (vertical lines inside the crisp group) and y positions
                let attr = |t: &str, k: &str| t.split(&format!("{k}=\"")).nth(1).and_then(|r| r.split('"').next()).and_then(|s| s.parse::<f64>().ok());
                let (mut vx, mut hy) = (vec![], vec![]);
                for t in svg.split("<line ").skip(1) {
                    let tag = t.split("/>").next().unwrap();
                    if let (Some(x1), Some(x2), Some(y1), Some(y2)) = (attr(tag, "x1"), attr(tag, "x2"), attr(tag, "y1"), attr(tag, "y2")) {
                        if (x1 - x2).abs() < 1e-6 { vx.push(x1); } else if (y1 - y2).abs() < 1e-6 { hy.push(y1); }
                    }
                }
                for (a, b, t, _bt, txt, _) in number_boxes(&svg) {
                    let v: f64 = txt.replace('\u{2212}', "-").parse().unwrap_or(f64::NAN);
                    if v.is_nan() || v == 0.0 { continue; }
                    let tx = g.ml + (v - g.x0) * g.ux;
                    let ty = g.mt + g.ph - (v - g.y0) * g.uy;
                    // a number is on the x axis (its box centre at the tick) or on the y axis (its baseline at the tick)
                    let on_x = vx.iter().any(|x| (x - tx).abs() < 0.6) && ((a + b) / 2.0 - tx).abs() < 3.5;
                    let on_y = hy.iter().any(|y| (y - ty).abs() < 0.6) && ((t + 10.0 - 4.1) - ty).abs() < 3.5;
                    assert!(on_x || on_y, "+-{half} {every}: \"{txt}\" has no gridline under it");
                }
            }
        }
        // minor lines: none closer than 3 px at +-50 with 5 between each pair
        let mut set = q("y = x");
        set.push(("style", Value::S("Print".into())));
        for (k, v) in [("xmin", -50.0), ("xmax", 50.0), ("ymin", -50.0), ("ymax", 50.0)] { set.push((k, Value::F(v))); }
        set.push(("minor", Value::I(5)));
        let n = grid_svg(&set).matches("<line").count();
        assert!(n < 400, "{n} lines at +-50 with minor 5");
    }

    /// Writes the new examples and the review cases to $VERSHION_PNG_DIR for a visual check.
    #[test]
    #[ignore]
    fn conic_example_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let _ = std::fs::create_dir_all(&dir);
        let exs = examples_b_plots("coordinate-grid").unwrap();
        let d = defs().into_iter().find(|d| d.id == "coordinate-grid").unwrap();
        for (i, e) in exs.iter().enumerate().skip(10) {
            let mut p = Params::defaults(&(d.params)());
            for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
            for (tag, lab) in [("plain", false), ("labels", true), ("print", true)] {
                p.0.insert("style".into(), Value::S(if tag == "print" { "Print".into() } else { "Colour".into() }));
                p.0.insert("eq_labels".into(), Value::B(lab));
                std::fs::write(format!("{dir}/conic-{i}-{tag}.png"), to_png(&(d.render)(&p), 1.5).unwrap()).unwrap();
            }
        }
        let cases: [(&str, &str, bool); 9] = [
            ("tangent-labels", "x^2 + y^2 = 25; (x-8)^2 + y^2 = 9; y = -x + 7.07", true),
            ("concentric-labels", "x^2 + y^2 = 1; x^2 + y^2 = 4; x^2 + y^2 = 9; x^2 + y^2 = 16", true),
            ("hyperbola-labels", "x^2/9 - y^2/4 = 1; y = 2x/3; y = -2x/3", true),
            ("ellipse-line-labels", "x^2/16 + y^2/9 = 1; y = x/2", true),
            ("ineq-labels", "x^2/9 + y^2/4 <= 1; y > x^2 - 6", true),
            ("annulus", "x^2 + y^2 > 4; x^2 + y^2 < 25", true),
            ("outside", "x^2 + y^2 = 400", false),
            ("tan-wide", "x = tan(y)", false),
            ("half-circle", "x^2 + y^2 = 25, y > 0; x = y^2, y > 0, x < 9", true),
        ];
        for (tag, eq, lab) in cases {
            let mut p = Params::defaults(&(d.params)());
            for (k, v) in q(eq) { p.0.insert(k.to_string(), v); }
            p.0.insert("eq_labels".into(), Value::B(lab));
            if tag.contains("hatch") { p.0.insert("shading".into(), Value::S("Hatching".into())); }
            if tag == "tan-wide" { for (k, v) in [("xmin", -50.0), ("xmax", 50.0), ("ymin", -5.0), ("ymax", 5.0), ("xstep", 10.0), ("square", 0.0)] { p.0.insert(k.into(), if k == "square" { Value::B(false) } else { Value::F(v) }); } }
            p.0.insert("points".into(), Value::S(if tag == "tangent-labels" { "P(5, 0), Q(-5, 0)".into() } else { String::new() }));
            std::fs::write(format!("{dir}/{tag}.png"), to_png(&(d.render)(&p), 1.2).unwrap()).unwrap();
        }
        let tan = "x^2 + y^2 = 25; (x-8)^2 + y^2 = 9; y = -x + 7.07";
        let extra: Vec<(&str, &str, Vec<(&str, Value)>)> = vec![
            ("tangent-print", tan, vec![("style", Value::S("Print".into()))]),
            ("tangent-student", tan, vec![("style", Value::S("Print".into())), ("show_answers", Value::B(false))]),
            ("tangent-notes", "x^2 + y^2 = 400; x^2 + y^2 = 25; x^2 + y^2 = -4", vec![("show_notes", Value::B(true))]),
            ("annulus-chain", "4 < x^2 + y^2 < 25", vec![]),
            ("annulus-chain-print", "4 <= x^2 + y^2 <= 25", vec![("style", Value::S("Print".into()))]),
            ("ineq-print", "x^2/9 + y^2/4 <= 1; y > x^2 - 6", vec![("style", Value::S("Print".into()))]),
            ("point", "(x-3)^2 + (y+2)^2 = 0; x^2 + y^2 = 25", vec![]),
            ("hyperbola-print", "x^2/9 - y^2/4 = 1; y = 2x/3; y = -2x/3", vec![("style", Value::S("Print".into()))]),
            ("xy4", "xy = 4", vec![]),
            ("group-linear", "x^2 + y^2 < 25; y > x - 2", vec![]),
            ("group-print", "x^2 + y^2 < 25; y > x - 2", vec![("style", Value::S("Print".into()))]),
            ("dashed-hyperbola", "x^2/9 - y^2/4 = 1; y = 2x/3 dashed; y = -2x/3 dashed", vec![]),
            ("explicit-poles", "y = 1/x; y = tan(x); y = 1/(x^2 - 1)", vec![]),
            ("tan-wide-window", "x = tan(y)", vec![("asymptotes", Value::B(true)), ("xmin", Value::F(-50.0)), ("xmax", Value::F(50.0)), ("ymin", Value::F(-5.0)), ("ymax", Value::F(5.0)), ("xstep", Value::F(10.0)), ("square", Value::B(false))]),
            ("tan-pole-100", "y = tan(x)", vec![("xmin", Value::F(-100.0)), ("xmax", Value::F(100.0)), ("ymin", Value::F(-100.0)), ("ymax", Value::F(100.0)), ("xstep", Value::F(20.0)), ("ystep", Value::F(20.0)), ("square", Value::B(false))]),
            ("tail-100", "x = tan(y)", vec![("xmin", Value::F(-100.0)), ("xmax", Value::F(100.0)), ("ymin", Value::F(-5.0)), ("ymax", Value::F(5.0)), ("xstep", Value::F(20.0)), ("square", Value::B(false))]),
            ("sci-radius", "x^2 + y^2 = 1e2", vec![]),
            ("chain-mixed", "4 < x^2 + y^2 <= 25", vec![]),
            ("chain-mixed-print", "25 > x^2 + y^2 >= 4", vec![("style", Value::S("Print".into()))]),
            ("group-bands", "x^2 + y^2 < 25; y > x^2 - 4; x > 0", vec![]),
            ("group-bands-print", "x^2 + y^2 < 25; y > x^2 - 4; x > 0", vec![("style", Value::S("Print".into()))]),
            ("double-roots", "(x-1)^2*(y-2)^2 = 0; (x^2+y^2-25)^2 = 0", vec![]),
            ("zero-ineq", "x^2 + y^2 <= 0", vec![]),
            ("equal-limits", "4 <= x^2 + y^2 <= 4", vec![]),
            ("empty-limits", "25 < x^2 + y^2 < 4", vec![("show_notes", Value::B(true))]),
            ("star-label", "x*3 = y^2", vec![]),
            ("style-before-domain", "x^2 + y^2 = 25 dashed, x > 0", vec![]),
            ("wide-notes", "x^2 + y^2 = 400; xx = 3y", vec![("xmin", Value::F(-50.0)), ("xmax", Value::F(50.0)), ("ymin", Value::F(-5.0)), ("ymax", Value::F(5.0)), ("show_notes", Value::B(true))]),
            ("half-circle-print", "x^2 + y^2 = 25, y > 0", vec![("style", Value::S("Print".into()))]),
            ("concentric-print", "x^2 + y^2 = 1; x^2 + y^2 = 4; x^2 + y^2 = 9; x^2 + y^2 = 16", vec![("style", Value::S("Print".into()))]),
            ("ineq-stack-print", "x^2 + y^2 < 25; y > x - 3; x > 0", vec![("style", Value::S("Print".into()))]),
            ("ineq-stack", "x^2 + y^2 < 25; y > x - 3; x > 0", vec![]),
            ("annulus-print", "4 < x^2 + y^2 <= 25", vec![("style", Value::S("Print".into()))]),
            ("annulus-colour", "x^2 + y^2 > 4; x^2 + y^2 < 25", vec![]),
            ("hyperbola-asym", "x^2/9 - y^2/4 = 1; y = 2x/3 asymptote; y = -2x/3 asymptote", vec![]),
            ("hyperbola-asym-print", "x^2/9 - y^2/4 = 1; y = 2x/3 dashed; y = -2x/3 dashed", vec![("style", Value::S("Print".into()))]),
            ("strict-x-axis", "x^2 + y^2 < 25; x > 0", vec![]),
            ("strict-x-axis-print", "x^2 + y^2 < 25; x > 0; y > 0", vec![("style", Value::S("Print".into()))]),
            ("no-real", "x^2 + y^2 = -4; x^2 + y^2 = 400", vec![("show_notes", Value::B(true))]),
            ("tick-on-curve", "x^2 + y^2 > 4; x^2 + y^2 < 25", vec![("xmin", Value::F(-10.0)), ("xmax", Value::F(10.0))]),
            ("small-cramped", "x^2 + y^2 = 4", vec![("xmin", Value::F(-10.0)), ("xmax", Value::F(10.0)), ("ymin", Value::F(-10.0)), ("ymax", Value::F(10.0))]),
            ("xy4-wide", "xy = 4", vec![("xmin", Value::F(-1000.0)), ("xmax", Value::F(1000.0)), ("ymin", Value::F(-1000.0)), ("ymax", Value::F(1000.0)), ("xstep", Value::F(200.0)), ("ystep", Value::F(200.0))]),
            ("xy4-tiny", "xy = 4", vec![("xmin", Value::F(-0.1)), ("xmax", Value::F(0.1)), ("ymin", Value::F(-0.1)), ("ymax", Value::F(0.1)), ("xstep", Value::F(0.05)), ("ystep", Value::F(0.05)), ("show_notes", Value::B(true))]),
            ("hyperbola-strip", "x^2/9 - y^2/4 = 1", vec![("xmin", Value::F(-50.0)), ("xmax", Value::F(50.0)), ("ymin", Value::F(-5.0)), ("ymax", Value::F(5.0)), ("xstep", Value::F(10.0))]),
            ("long-notes", "x^2 + y^2 = -4; xx = 3y; x^2 + y^2 = 400; 1 < y <", vec![("show_notes", Value::B(true))]),
            ("tan-wide-print", "x = tan(y)", vec![("style", Value::S("Print".into())), ("xmin", Value::F(-50.0)), ("xmax", Value::F(50.0)), ("ymin", Value::F(-5.0)), ("ymax", Value::F(5.0)), ("xstep", Value::F(10.0)), ("square", Value::B(false))]),
            ("wide-20", "", vec![("xmin", Value::F(-20.0)), ("xmax", Value::F(20.0)), ("ymin", Value::F(-20.0)), ("ymax", Value::F(20.0))]),
            ("wide-20-unsquare", "y = x", vec![("xmin", Value::F(-20.0)), ("xmax", Value::F(20.0)), ("ymin", Value::F(-5.0)), ("ymax", Value::F(5.0)), ("square", Value::B(false))]),
            ("wide-20-print", "x^2 + y^2 = 100", vec![("style", Value::S("Print".into())), ("xmin", Value::F(-20.0)), ("xmax", Value::F(20.0)), ("ymin", Value::F(-20.0)), ("ymax", Value::F(20.0))]),
            ("ln-a", "y = ln(x)", vec![("xmin", Value::F(-5.0)), ("xmax", Value::F(5.0)), ("ymin", Value::F(-10.0)), ("ymax", Value::F(10.0))]),
            ("ln-b", "y = ln(x)", vec![("xmin", Value::F(-1.0)), ("xmax", Value::F(20.0)), ("ymin", Value::F(-10.0)), ("ymax", Value::F(10.0))]),
            ("ln-c", "y = ln(x-1)", vec![("xmin", Value::F(-5.0)), ("xmax", Value::F(10.0)), ("ymin", Value::F(-8.0)), ("ymax", Value::F(8.0))]),
            ("ln-d", "y = log(x)", vec![("xmin", Value::F(-5.0)), ("xmax", Value::F(5.0)), ("ymin", Value::F(-3.0)), ("ymax", Value::F(3.0))]),
            ("ln-e", "y = ln(abs(x))", vec![("xmin", Value::F(-5.0)), ("xmax", Value::F(5.0)), ("ymin", Value::F(-5.0)), ("ymax", Value::F(5.0))]),
            ("ineq-stack-print-wide", "x^2 + y^2 < 25; y > x - 3; x > 0", vec![("style", Value::S("Print".into())), ("xmin", Value::F(-10.0)), ("xmax", Value::F(10.0)), ("ymin", Value::F(-10.0)), ("ymax", Value::F(10.0))]),
            ("xy-asym", "x*y = 4; x = 0; y = 0", vec![]),
            ("parallel", "(x-y)^2 = 4; y = x", vec![]),
            ("band-axis", "0 < y < 3; x*y > 0", vec![]),
            ("annulus-print-21", "4 < x^2 + y^2 <= 25", vec![("style", Value::S("Print".into())), ("xmin", Value::F(-10.0)), ("xmax", Value::F(10.0)), ("ymin", Value::F(-10.0)), ("ymax", Value::F(10.0))]),
            ("concentric-lab-print", "x^2 + y^2 = 4; x^2 + y^2 = 9; x^2 + y^2 = 16; y = 3; y = x", vec![("style", Value::S("Print".into()))]),
            ("labels-spacing", "x^2+y^2=16; x^2 + y^2 = 25", vec![]),
            ("circle-25", "x^2 + y^2 = 25", vec![]),
            ("circle-25-print", "x^2 + y^2 = 25", vec![("style", Value::S("Print".into()))]),
            ("grid-print", "y = x", vec![("style", Value::S("Print".into()))]),
            ("ln-origin", "y = ln(x)", vec![("xmin", Value::F(0.0)), ("xmax", Value::F(20.0)), ("ymin", Value::F(-4.0)), ("ymax", Value::F(4.0)), ("style", Value::S("Print".into()))]),
            ("r8-conc10", "x^2 + y^2 = 1; x^2 + y^2 = 4; x^2 + y^2 = 9; x^2 + y^2 = 16; x^2 + y^2 = 25", vec![("xmin", Value::F(-10.0)), ("xmax", Value::F(10.0)), ("ymin", Value::F(-10.0)), ("ymax", Value::F(10.0))]),
            ("r8-conc15-print", "x^2 + y^2 = 1; x^2 + y^2 = 4; x^2 + y^2 = 9; x^2 + y^2 = 16; x^2 + y^2 = 25", vec![("xmin", Value::F(-15.0)), ("xmax", Value::F(15.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0)), ("style", Value::S("Print".into()))]),
            ("r8-ell15", "x^2/16 + y^2/9 = 1", vec![("xmin", Value::F(-15.0)), ("xmax", Value::F(15.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r8-c1000", "x^2 + y^2 = 640000", vec![("xmin", Value::F(-1000.0)), ("xmax", Value::F(1000.0)), ("ymin", Value::F(-1000.0)), ("ymax", Value::F(1000.0))]),
            ("r8-c1000-print", "x^2 + y^2 = 640000", vec![("xmin", Value::F(-1000.0)), ("xmax", Value::F(1000.0)), ("ymin", Value::F(-1000.0)), ("ymax", Value::F(1000.0)), ("style", Value::S("Print".into()))]),
            ("r8-h1000", "x^2/640000 - y^2/160000 = 1", vec![("xmin", Value::F(-1000.0)), ("xmax", Value::F(1000.0)), ("ymin", Value::F(-1000.0)), ("ymax", Value::F(1000.0))]),
            ("r9-unit10", "x^2 + y^2 = 1", vec![("xmin", Value::F(-10.0)), ("xmax", Value::F(10.0)), ("ymin", Value::F(-10.0)), ("ymax", Value::F(10.0))]),
            ("r9-unit10-print", "x^2 + y^2 = 1", vec![("xmin", Value::F(-10.0)), ("xmax", Value::F(10.0)), ("ymin", Value::F(-10.0)), ("ymax", Value::F(10.0)), ("style", Value::S("Print".into()))]),
            ("r9-conc10", "x^2 + y^2 = 1; x^2 + y^2 = 4; x^2 + y^2 = 9; x^2 + y^2 = 16", vec![("xmin", Value::F(-10.0)), ("xmax", Value::F(10.0)), ("ymin", Value::F(-10.0)), ("ymax", Value::F(10.0))]),
            ("r9-tall", "x^2 + y^2 = 25", vec![("xmin", Value::F(-5.0)), ("xmax", Value::F(5.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r9-tall-print", "x^2 + y^2 = 25", vec![("xmin", Value::F(-5.0)), ("xmax", Value::F(5.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0)), ("style", Value::S("Print".into()))]),
            ("r10-c5-15", "x^2 + y^2 = 25", vec![("xmin", Value::F(-5.0)), ("xmax", Value::F(5.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r10-c8-15", "x^2 + y^2 = 25", vec![("xmin", Value::F(-8.0)), ("xmax", Value::F(8.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r10-h8-15", "x^2 - y^2 = 1", vec![("xmin", Value::F(-8.0)), ("xmax", Value::F(8.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r10-c20-3", "x^2 + y^2 = 25", vec![("xmin", Value::F(-20.0)), ("xmax", Value::F(20.0)), ("ymin", Value::F(-3.0)), ("ymax", Value::F(3.0))]),
            ("r10-c2-20-print", "x^2 + y^2 = 25", vec![("xmin", Value::F(-2.0)), ("xmax", Value::F(2.0)), ("ymin", Value::F(-20.0)), ("ymax", Value::F(20.0)), ("style", Value::S("Print".into()))]),
            ("r11-c2-5-15", "x^2 + y^2 = 4", vec![("xmin", Value::F(-5.0)), ("xmax", Value::F(5.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r11-c2-8-15", "x^2 + y^2 = 4", vec![("xmin", Value::F(-8.0)), ("xmax", Value::F(8.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r11-h5-15", "x^2 - y^2 = 1", vec![("xmin", Value::F(-5.0)), ("xmax", Value::F(5.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r11-e8-15", "x^2/16 + y^2/9 = 1", vec![("xmin", Value::F(-8.0)), ("xmax", Value::F(8.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r11-50", "x^2 + y^2 = 900", vec![("xmin", Value::F(-50.0)), ("xmax", Value::F(50.0)), ("ymin", Value::F(-50.0)), ("ymax", Value::F(50.0))]),
            ("r11-c5-5-15", "x^2 + y^2 = 25", vec![("xmin", Value::F(-5.0)), ("xmax", Value::F(5.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r12-c5-5-15", "x^2 + y^2 = 25", vec![("xmin", Value::F(-5.0)), ("xmax", Value::F(5.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r12-c2-8-15", "x^2 + y^2 = 4", vec![("xmin", Value::F(-8.0)), ("xmax", Value::F(8.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r12-h5-15", "x^2 - y^2 = 1", vec![("xmin", Value::F(-5.0)), ("xmax", Value::F(5.0)), ("ymin", Value::F(-15.0)), ("ymax", Value::F(15.0))]),
            ("r12-c5-11", "x^2 + y^2 = 25", vec![("xmin", Value::F(-11.0)), ("xmax", Value::F(11.0)), ("ymin", Value::F(-11.0)), ("ymax", Value::F(11.0))]),
            ("r12-c5-10-13", "x^2 + y^2 = 25", vec![("xmin", Value::F(-10.0)), ("xmax", Value::F(10.0)), ("ymin", Value::F(-13.0)), ("ymax", Value::F(13.0))]),
            ("r12-plain50", "y = x", vec![("xmin", Value::F(-50.0)), ("xmax", Value::F(50.0)), ("ymin", Value::F(-50.0)), ("ymax", Value::F(50.0))]),
            ("wide-1000", "y = x", vec![("xmin", Value::F(-1000.0)), ("xmax", Value::F(1000.0)), ("ymin", Value::F(-1000.0)), ("ymax", Value::F(1000.0))]),
        ];
        for (tag, eq, set) in extra {
            let mut p = Params::defaults(&(d.params)());
            for (k, v) in q(eq) { p.0.insert(k.to_string(), v); }
            p.0.insert("eq_labels".into(), Value::B(true));
            for (k, v) in set { p.0.insert(k.to_string(), v); }
            std::fs::write(format!("{dir}/{tag}.png"), to_png(&(d.render)(&p), 1.2).unwrap()).unwrap();
            std::fs::write(format!("{dir}/{tag}.svg"), (d.render)(&p)).unwrap();
            // a bitonal photocopy at several thresholds of the Print renders
            if tag.ends_with("-print") {
                let (w, h, px) = super::super::to_rgba(&(d.render)(&p), 1.2).unwrap();
                for thr in [50u32, 58, 66, 75] {
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < thr as f64 * 2.55 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/{tag}-bw{thr}.png")).unwrap();
                }
            }
        }
    }

    fn run(id: &str, tag: &str, set: &[(&str, Value)]) {
        let d = defs().into_iter().find(|d| d.id == id).unwrap();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in set {
            assert!(p.0.contains_key(*k), "{id}: unknown key {k}");
            p.0.insert(k.to_string(), v.clone());
        }
        let svg = (d.render)(&p);
        let png = to_png(&svg, 1.5).unwrap_or_else(|e| panic!("{id}/{tag}: {e}"));
        let dir = "/tmp/claude-1000/-home-shuff57/4fa1e9b4-a716-475e-b753-2c60208b3c78/scratchpad/math";
        let _ = std::fs::create_dir_all(dir);
        std::fs::write(format!("{dir}/{id}-{tag}.png"), png).unwrap();
    }
    fn s(x: &str) -> Value { Value::S(x.into()) }
    fn f(x: f64) -> Value { Value::F(x) }
    fn i(x: i64) -> Value { Value::I(x) }
    fn b(x: bool) -> Value { Value::B(x) }

    #[test]
    fn evaluator() {
        assert!((eval("2x + 1", 3.0).unwrap() - 7.0).abs() < 1e-9);
        assert!((eval("x^2 - 3", 2.0).unwrap() - 1.0).abs() < 1e-9);
        assert!((eval("-(x+1)/2", 3.0).unwrap() + 2.0).abs() < 1e-9);
        assert!(eval("2x +", 1.0).is_none());
        assert_eq!(parse_ineq("-2 < x <= 5").len(), 1);
        assert_eq!(parse_ineq("x < -4 or x >= 1").len(), 2);
    }

    #[test]
    fn all_math_defaults() {
        for d in defs() { run(d.id, "default", &[]); }
    }

    #[test]
    fn grid() {
        run("coordinate-grid", "neg", &[("xmin", f(-6.0)), ("xmax", f(6.0)), ("ymin", f(-5.0)), ("ymax", f(7.0)), ("minor", i(4)), ("equations", s("y = x^2 - 3; y = -x + 2; x = 4")), ("points", s("(2,3) A; (-4,-2) B")), ("eq_labels", b(true)), ("title", s("Graphs"))]);
    }
    #[test]
    fn composite() {
        for pr in ["L-shape", "House", "Arch", "T-shape"] { run("composite-shape", &pr.to_lowercase(), &[("preset", s(pr))]); }
    }
    #[test]
    fn numline() {
        run("number-line", "ineq", &[("inequality", s("x < -4 or x >= 1")), ("points", s("-8, 6o")), ("min", f(-10.0)), ("max", f(10.0)), ("minor", i(1))]);
        run("number-line", "dec", &[("min", f(0.0)), ("max", f(2.0)), ("step", f(0.25)), ("inequality", s("x > 0.5")), ("label_every", i(2))]);
    }
    #[test]
    fn polys() {
        run("triangle", "sss", &[("m_a", s("")), ("m_b", s("")), ("m_ab", s("7")), ("m_bc", s("5")), ("m_ca", s("6")), ("height", s("From C")), ("a_lab", s("Measure")), ("b_lab", s("Measure")), ("c_lab", s("Measure")), ("unit", s("cm")), ("bc_lab", s("Measure"))]);
        run("triangle", "obtuse-h", &[("m_a", s("120")), ("m_b", s("")), ("m_ab", s("8")), ("m_ca", s("5")), ("height", s("From C")), ("rotate", f(20.0))]);
        run("triangle", "asa", &[("m_a", s("40")), ("m_b", s("70")), ("m_ab", s("9")), ("c_lab", s("Measure"))]);
        run("triangle", "iso", &[("m_a", s("")), ("m_b", s("")), ("m_ab", s("6")), ("m_bc", s("5")), ("m_ca", s("5")), ("bc_ticks", s("1")), ("ca_ticks", s("1"))]);
        run("triangle", "sqrt", &[("m_a", s("")), ("m_b", s("90")), ("m_ab", s("5")), ("m_bc", s("5")), ("ca_lab", s("Measure"))]);
        run("rectangle", "sq", &[("kind", s("Square")), ("m_ab", s("4")), ("diagonals", s("Drawn and labelled")), ("unit", s("m"))]);
        run("parallelogram", "rh", &[("kind", s("Rhombus")), ("height", s("Onto AB (from D)")), ("m_a", s("110")), ("diagonals", s("Drawn"))]);
        run("parallelogram", "h", &[("height", s("Onto AB (from D)")), ("m_a", s("")), ("m_b", s("55"))]);
        run("trapezoid", "iso", &[("kind", s("Isosceles trapezoid")), ("m_da", s("")), ("m_h", s("5")), ("show_height", b(true)), ("a_lab", s("Measure")), ("b_lab", s("Measure"))]);
        run("trapezoid", "plain", &[("kind", s("Trapezoid")), ("m_da", s("")), ("m_a", s("60")), ("m_h", s("5")), ("show_height", b(true))]);
        run("kite", "diag", &[("diagonals", s("Drawn")), ("a_lab", s("Measure")), ("c_lab", s("Measure")), ("d_lab", s("Measure"))]);
        run("regular-polygon", "n8", &[("n", i(8)), ("apothem", b(true)), ("from", s("Radius")), ("value", f(5.0))]);
        run("regular-polygon", "n3", &[("n", i(3)), ("side_lab", s("Measure")), ("ang_lab", s("Measure"))]);
        run("regular-polygon", "n17", &[("n", i(17)), ("radius", b(false))]);
    }
    #[test]
    fn angle_figs() {
        run("angles", "single", &[("ray2", s("35"))]);
        run("angles", "straight", &[("ray2", s("35")), ("ray3", s("180")), ("unknown", s("2"))]);
        run("angles", "around", &[("ray2", s("35")), ("ray3", s("145")), ("around", b(true)), ("unknown", s("2"))]);
        run("angles", "vertical", &[("ray2", s("35")), ("ray1_line", s("Line")), ("ray2_line", s("Line")), ("ray2_style", s("Dashed"))]);
        run("angles", "custom", &[("ray2", s("50")), ("ray3", s("130")), ("ray4", s("250"))]);
        run("angles", "right", &[("ray2", s("90")), ("ray3", s("180"))]);
        run("parallel-lines", "num", &[("labels", s("Numbers")), ("marked", s("1,2,3,4,5,6,7,8")), ("angle", f(55.0))]);
        run("parallel-lines", "three", &[("lines", i(3)), ("angle", f(110.0)), ("marked", s("2, 6, 10")), ("unknown", s("10")), ("t_arrows", s("2")), ("n_style", s("Dashed"))]);
    }
    #[test]
    fn flat_shapes_parity() {
        run("triangle", "ssa", &[("m_a", s("40")), ("m_b", s("")), ("m_ab", s("10")), ("m_bc", s("7")), ("other", b(true)), ("height", s("From all three")), ("bottom", s("BC")), ("flip", b(true))]);
        run("triangle", "aas", &[("m_a", s("35")), ("m_b", s("95")), ("m_ab", s("")), ("m_bc", s("5")), ("height", s("From A")), ("height_label", s("Text"))]);
        run("triangle", "bad-ssa", &[("m_a", s("40")), ("m_b", s("")), ("m_ab", s("10")), ("m_bc", s("2"))]);
        run("rectangle", "diag", &[("diagonals", s("AC only")), ("ac_lab", s("Text")), ("ac_label", s("x")), ("ab_lab", s("Text")), ("ab_label", s("2x + 3")), ("ab_ticks", s("1")), ("cd_ticks", s("1")), ("label_size", s("Large")), ("bottom", s("BC"))]);
        run("parallelogram", "h2", &[("height", s("Onto AB (from D and C)")), ("diagonals", s("BD only")), ("line_style", s("Dotted")), ("arrow_marks", s("none"))]);
        run("trapezoid", "hc", &[("kind", s("Trapezoid")), ("m_da", s("")), ("m_a", s("65")), ("m_h", s("5")), ("show_height", b(true)), ("hfrom", s("D and C")), ("a_arcs", s("2")), ("b_arcs", s("1"))]);
        run("kite", "h", &[("height", s("From D and C to AB")), ("diagonals", s("Drawn")), ("bottom", s("CD"))]);
        run("regular-polygon", "all", &[("n", i(9)), ("all_radii", b(true)), ("apothem", b(true)), ("center_name", s("O")), ("apothem_label", s("Text")), ("radius_label", s("None")), ("flip", b(true))]);
        run("angles", "turn", &[("ray2", s("50")), ("ray1_line", s("Line")), ("ray2_line", s("Line")), ("turn", f(40.0)), ("ray_ends", s("Points")), ("sector_text", s("1 = 2x")), ("shade", b(false))]);
        run("parallel-lines", "two", &[("lines", i(3)), ("transversals", i(2)), ("turn", f(-20.0)), ("line_ends", s("Plain")), ("angle_text", s("14 = y")), ("marked", s("2, 14"))]);
        for pr in ["Rectangle", "Triangle", "Semicircle", "Circle"] { run("composite-shape", &pr.to_lowercase(), &[("preset", s(pr)), ("unit", s("m"))]); }
        for id in ["triangle", "rectangle", "parallelogram", "trapezoid", "kite", "regular-polygon", "angles", "parallel-lines", "composite-shape"] {
            let exs = examples(id).expect(id);
            assert!((3..=6).contains(&exs.len()), "{id}");
            for e in exs {
                run(id, &format!("ex-{}", e.name.len()), &e.params);
                // every example solves (no message in place of the figure)
                let d = defs().into_iter().find(|d| d.id == id).unwrap();
                let mut p = Params::defaults(&(d.params)());
                for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
                assert!(!(d.render)(&p).contains("Give ") && !(d.render)(&p).contains("would be"), "{id}: {}", e.name);
            }
        }
        // handles still build for the new layouts
        for (id, set) in [("triangle", vec![("flip", b(true))]), ("kite", vec![("bottom", s("AB"))]), ("parallel-lines", vec![("turn", f(30.0)), ("transversals", i(2))]), ("angles", vec![("turn", f(10.0))]), ("trapezoid", vec![]), ("rectangle", vec![]), ("parallelogram", vec![])] {
            let d = defs().into_iter().find(|d| d.id == id).unwrap();
            let mut p = Params::defaults(&(d.params)());
            for (k, v) in set { p.0.insert(k.to_string(), v); }
            assert!(!handles(id, &p, (480.0, 360.0)).unwrap_or_default().is_empty(), "{id}");
        }
    }
    #[test]
    fn solids() {
        run("prism", "tri", &[("base", s("Right triangle"))]);
        run("prism", "iso", &[("base", s("Isosceles triangle"))]);
        run("prism", "hex", &[("base", s("Regular polygon")), ("n", i(6))]);
        run("prism", "oblique", &[("stands", s("Oblique"))]);
        run("pyramid", "slant", &[("slant_lab", s("Measure"))]);
        run("pyramid", "hex", &[("base", s("Regular polygon")), ("n", i(6)), ("stands", s("Oblique")), ("slant_lab", s("Measure"))]);
        run("cylinder", "oblique", &[("stands", s("Oblique")), ("d_lab", s("Measure"))]);
        run("cone", "oblique", &[("stands", s("Oblique")), ("view", f(35.0))]);
        run("sphere", "hemi", &[("draw", s("A hemisphere")), ("d_lab", s("Measure")), ("r_lab", s("None"))]);
    }
    #[test]
    fn data_figs() {
        run("box-plot", "two", &[("data2", s("10, 12, 15, 15, 18, 20, 21, 40")), ("name1", s("Class A")), ("name2", s("Class B")), ("outliers", b(true)), ("title", s("Scores")), ("axis_title", s("Points"))]);
        run("box-plot", "summary", &[("summary", s("20, 35, 40, 55, 90"))]);
        run("mapping-diagram", "many", &[("inputs", s("a, b, c")), ("outputs", s("1, 2")), ("pairs", s("a->1; b->1; c->2")), ("shape", s("Rectangles"))]);
        run("ruler-reading", "pencil", &[("object", s("Pencil")), ("start", f(2.0)), ("length", f(6.45)), ("show_value", b(true))]);
        run("ruler-reading", "inch", &[("marks", s("1/8 in")), ("ruler", i(6)), ("length", f(3.625)), ("start", f(0.0))]);
        run("ruler-reading", "long", &[("ruler", i(30)), ("marks", s("1 mm")), ("length", f(17.3))]);
    }
}

#[cfg(test)]
mod cfg_math_tests {
    use super::*;
    use super::flat::*;
    fn params(id: &str, set: &[(&str, Value)]) -> Params {
        let d = defs().into_iter().find(|d| d.id == id).unwrap();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in set { p.0.insert(k.to_string(), v.clone()); }
        p
    }
    fn s(x: &str) -> Value { Value::S(x.into()) }

    #[test]
    fn new_rays_go_in_the_widest_gap() {
        assert_eq!(widest_gap_mid(&[0.0, 60.0]), 120.0);
        assert_eq!(widest_gap_mid(&[0.0]), 90.0);
        assert_eq!(widest_gap_mid(&[0.0, 200.0]), 100.0);
        let p = params("angles", &[("ray_count", Value::I(3))]);
        let rows = ray_rows(&p);
        assert!(rows.iter().any(|r| r.0 == 2 && r.1 == 120.0 && r.2), "{rows:?}");
        assert!(angles_computed(&p).iter().any(|(k, v)| *k == "ray3" && v == "120"));
        // typed rows always show; empty rows beyond the count are hidden
        assert!(card_visible("angles", "", Some("ray3"), &p) && !card_visible("angles", "", Some("ray4"), &p));
        let p = params("angles", &[("ray5", s("200"))]);
        assert!(card_visible("angles", "", Some("ray5"), &p));
    }

    #[test]
    fn chips_name_real_params() {
        for id in ["angles", "parallel-lines", "mapping-diagram", "composite-shape"] {
            let d = defs().into_iter().find(|d| d.id == id).unwrap();
            let keys: Vec<&str> = (d.params)().iter().map(|p| p.key).collect();
            let card = chips(id).unwrap();
            assert!(card.count_key.is_empty() || keys.contains(&card.count_key), "{id}");
            for c in &card.chips {
                for (k, _) in &c.set { assert!(keys.contains(&k.replace("{n}", "3").as_str()), "{id}: {k}"); }
            }
        }
        // every "Start with" chip loads a spec that draws
        for name in COMPOSITE_STARTS { assert!(preset_spec(name).is_some(), "{name}"); }
    }

    #[test]
    fn mapping_pairs_paste_and_order() {
        let p = params("mapping-diagram", &[("pairs", s("1->3; (2,5)"))]);
        let m = mapping_parse(&p);
        assert_eq!(m.pairs.len(), 2);
        assert_eq!(mapping_msg(&p).unwrap(), "Not a function yet: 3 has no output. Every input needs exactly one.");
        let p = params("mapping-diagram", &[("pairs", s(MATCH_IN_ORDER))]);
        let m = mapping_parse(&p);
        assert_eq!(m.pairs.len(), 4);
        assert!(m.pairs.iter().all(|q| q.b_span.is_none()) && mapping_msg(&p).is_none());
        let p = params("mapping-diagram", &[("paste", s("(a, 1), (a, 2), (b, 2)"))]);
        let m = mapping_parse(&p);
        assert_eq!((m.ins.clone(), m.outs.clone(), m.key), (vec!["a".to_string(), "b".into()], vec!["1".to_string(), "2".into()], "paste"));
        assert!(mapping_msg(&p).unwrap().starts_with("Not a function: a has 2 outputs"));
        assert!(mapping_msg(&params("mapping-diagram", &[("pairs", s(""))])).is_none());
        assert_eq!(list_items("1\n2\t3, 3"), vec!["1", "2", "3"]);
    }

    #[test]
    fn quad_side_arrows_and_messages() {
        let p = params("parallelogram", &[("ab_arrows", s("3")), ("bc_arrows", s("None"))]);
        assert_eq!(quad_arrows(&p, &quad_default_arrows("parallelogram")), vec![3, 0, 1, 2]);
        let c = quad_computed("parallelogram", &p);
        assert!(c.iter().any(|(k, v)| *k == "cd_par" && v.ends_with("auto")));
        // the older list still applies to sides left on Auto
        let p = params("trapezoid", &[("arrow_marks", s("AB 2, CD 2"))]);
        assert_eq!(quad_arrows(&p, &quad_default_arrows("trapezoid")), vec![2, 0, 2, 0]);
        let p = params("rectangle", &[("m_ab", s("")), ("m_bc", s(""))]);
        assert!(quad_computed("rectangle", &p).iter().any(|(k, _)| *k == "__msg"));
    }

    #[test]
    fn solids_diameter_and_conflicts() {
        let p = params("cylinder", &[("give_d", Value::B(true)), ("diam", Value::F(3.0))]);
        assert_eq!(solid_r(&p), 1.5);
        assert!(card_visible("cylinder", "", Some("diam"), &p) && !card_visible("cylinder", "", Some("radius"), &p));
        assert!(!card_visible("cylinder", "Oblique", None, &p));
        assert!(diam_text(&p, 3.0).is_some());
        let p = params("cylinder", &[("stands", s("Oblique")), ("edge", s("1"))]);
        assert!(solid_msg("cylinder", &p).unwrap().contains("don\u{2019}t fit together"));
        assert!(card_visible("cylinder", "Oblique", None, &p));
        let p = params("cone", &[("slant", s("2"))]);
        assert!(solid_msg("cone", &p).is_some());
        assert!(solid_msg("cone", &params("cone", &[("slant", s("5"))])).is_none());
        let p = params("prism", &[("base", s("Regular polygon"))]);
        assert!(card_visible("prism", "Regular polygon base", None, &p) && !card_visible("prism", "Rectangle or triangle base", None, &p));
        assert!(!card_visible("prism", "Rectangle or triangle base", Some("hyp"), &params("prism", &[])));
    }

    #[test]
    fn composite_side_lines_angles_points_and_turn() {
        let base = "rect 0 0 6 4; tri 0 4 6 4 3 7";
        let svg = |extra: &str, more: &[(&str, Value)]| {
            let mut kv: Vec<(&str, Value)> = vec![("spec", s(&format!("{base}{extra}")))];
            kv.extend(more.iter().cloned());
            composite_render(&params("composite-shape", &kv))
        };
        // no Label-mode rows: the outline is the one thick stroke under the fills, as before
        let plain = svg("", &[]);
        assert!(plain.contains(r#"stroke-width="5" stroke-linejoin="round"/>"#) && !plain.contains("stroke-dasharray"));
        // side lines: dashed and dotted dash arrays, a hidden side not drawn at all
        let st = svg("; side 6 0 6 4 dashed thick; side 0 0 6 0 dotted thin; side 0 4 0 0 hidden", &[]);
        assert!(st.contains(r#"stroke-width="4.5" stroke-dasharray="17.5 12.0""#), "{st}");
        assert!(st.contains(r#"stroke-dasharray="0.01 6.3""#), "{st}");
        let solid5 = st.matches(r#"stroke-width="5" stroke-linecap="round"/>"#).count();
        assert_eq!(solid5, 2, "only the two roof sides keep the plain line: {st}");
        // angles: measure rounded by Round angles, arcs, shading, the exterior's extended side
        let a = svg("; angle 0 4 measure arcs 2 red; angle 6 0 ext measure", &[]);
        assert!(a.contains(">135\u{b0}<") && a.contains(">90\u{b0}<"), "{a}");
        assert!(a.contains(super::super::composite::shade_hex("red").unwrap()));
        assert_eq!(a.matches(r#"stroke-width="1.4" stroke-linejoin"#).count(), 2, "two arcs: {a}");
        assert!(a.contains(r##"stroke="#444" stroke-width="1.3" stroke-dasharray="5 4""##), "{a}");
        let t = |dec: i64| composite_render(&params("composite-shape", &[("spec", s("tri 0 0 4 0 0 3; angle 4 0 measure")), ("angle_dec", Value::I(dec))]));
        assert!(t(0).contains(">37\u{b0}<") && t(1).contains(">36.9\u{b0}<") && t(2).contains(">36.87\u{b0}<"));
        // a typed angle label, and an angle whose corner is gone draws nothing
        assert!(svg("; angle 3 7 | x", &[]).contains(r#"font-style="italic">x<"#));
        assert_eq!(svg("; angle 9 9 measure", &[]), plain);
        // corner points with names
        let pt = svg("; point 0 0 A", &[]);
        assert!(pt.contains(r#"r="4.50""#) && pt.contains(r#"font-style="italic">A<"#), "{pt}");
        // Turn: a quarter turn stands the wide rectangle on end; labels stay upright text
        let poly = |svg: &str| -> (f64, f64) {
            let i = svg.find("<polygon points=\"").unwrap() + 17;
            let pts: Vec<(f64, f64)> = svg[i..svg[i..].find('"').unwrap() + i].split(' ').map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect();
            let (x0, x1) = pts.iter().fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q.0), a.1.max(q.0)));
            let (y0, y1) = pts.iter().fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q.1), a.1.max(q.1)));
            (x1 - x0, y1 - y0)
        };
        let wide = composite_render(&params("composite-shape", &[("spec", s("rect 0 0 8 2; dim 0 0 8 0"))]));
        let turned = composite_render(&params("composite-shape", &[("spec", s("rect 0 0 8 2; dim 0 0 8 0")), ("rotate", Value::F(90.0))]));
        let (w0, h0) = poly(&wide);
        let (w1, h1) = poly(&turned);
        assert!(w0 > h0 * 2.0 && h1 > w1 * 2.0, "{w0}x{h0} -> {w1}x{h1}");
        assert!(!turned.contains("rotate("), "text is never rotated");
        assert_eq!(composite_render(&params("composite-shape", &[("spec", s("rect 0 0 8 2; dim 0 0 8 0")), ("rotate", Value::F(0.0))])), wide);
    }

    #[test]
    fn composite_add_piece_and_shading() {
        assert_eq!(super::super::math_plot::piece_on_side((0.0, 4.0), (6.0, 4.0), (3.0, 2.0)), "rect 0 4 6 4.5");
        assert_eq!(super::super::math_plot::piece_on_side((6.0, 0.0), (6.0, 4.0), (3.0, 2.0)), "rect 6 0 3 4");
        assert_eq!(shade_word(&["rect", "0", "0", "1", "1", "Blue"]), Some("#cfe1fa"));
        let p = params("composite-shape", &[("spec", s("rect 0 0 6 4 red; quarter 0 4 2 ne; ticks 0 0 6 0 2; arrows 0 4 6 4 1"))]);
        assert!(composite_render(&p).contains("#fbd0d0"));
        // the side handles add a piece when clicked
        let hs = handles("composite-shape", &p, (480.0, 360.0)).unwrap();
        let add = hs.iter().find(|h| h.click.is_some()).expect("an add-piece handle");
        let mut q = p.clone();
        (add.click.as_ref().unwrap())(&mut q);
        assert_eq!(q.s("spec").matches(';').count(), p.s("spec").matches(';').count() + 1);
    }
}
