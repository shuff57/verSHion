//! Flat shapes, batch 9: each construction line has its own style, the point where a height lands and the point where the
//! diagonals cross can be named, and the regular polygon's size can be typed as a fraction or with a square root.
//!
//! - Every height (triangle, parallelogram, trapezoid, kite), every diagonal (the four quadrilaterals) and the regular
//!   polygon's apothem, radius and other radii get a choice of Automatic, Solid, Dashed or Dotted. Automatic, the default,
//!   is what every line drew before: the figure's one "Construction lines" setting (and, for the other radii, the radius's
//!   own style, as the reference draws them "like the radius"). An explicit Dotted is a row of round dots, plainly visible.
//! - A height's foot and the diagonals' crossing are named by typing a letter (empty, the default, names nothing): a dot and
//!   the letter, placed after everything else is drawn, clear of every line, label and dot, inside the page and nearer its own
//!   point than any corner or other point. A name that would mislead (the foot is a corner, the name is a corner's or another
//!   point's) or that has no clear place is left off, with a note in the panel only.
//! - The regular polygon's size has a typed box over its slider, read by the solids' reader (`solids_b8::read`) and written
//!   on the measure's label exactly as typed (`solids_b8::pretty`).
//!
//! Old figures carry none of the new keys, so every default draws through the old statements byte for byte
//! (`flat_b9_frozen.rs`).
use super::*;

pub(crate) const STYLE_OPTS: [&str; 4] = ["Automatic", "Solid", "Dashed", "Dotted"];
/// The dash value of an explicit Dotted line: drawn by `dot_line` as round dots (never handed to the SVG as is).
pub(super) const DOT: &str = "b9-dot";
const IDS: [&str; 6] = ["triangle", "rectangle", "parallelogram", "trapezoid", "kite", "regular-polygon"];
/// Longest name a point may have.
const MAX_NAME: usize = 4;

/// A height a figure can draw: (style key, foot key, the corner it is drawn from).
fn height_keys(id: &str) -> &'static [(&'static str, &'static str, &'static str)] {
    match id {
        "triangle" => &[("ha_style", "ha_foot", "A"), ("hb_style", "hb_foot", "B"), ("hc_style", "hc_foot", "C")],
        "parallelogram" => &[("hd_style", "hd_foot", "D"), ("hb_style", "hb_foot", "B"), ("hc_style", "hc_foot", "C")],
        "trapezoid" | "kite" => &[("hd_style", "hd_foot", "D"), ("hc_style", "hc_foot", "C")],
        _ => &[],
    }
}
fn has_diagonals(id: &str) -> bool { matches!(id, "rectangle" | "parallelogram" | "trapezoid" | "kite") }

/// The new controls of figure `id`, appended to its list (old keys first, unchanged).
pub(super) fn params(id: &str) -> Vec<ParamDef> {
    let mut v = Vec::new();
    for (sk, fk, from) in height_keys(id) {
        let (label, foot): (&'static str, &'static str) = match *from {
            "A" => ("Height from A: line", "Height from A: name where it lands"),
            "B" => ("Height from B: line", "Height from B: name where it lands"),
            "C" => ("Height from C: line", "Height from C: name where it lands"),
            _ => ("Height from D: line", "Height from D: name where it lands"),
        };
        v.push(ParamDef::choice(sk, label, &STYLE_OPTS, "Automatic").hint("Solid, dashed or dotted; Automatic follows Construction lines."));
        v.push(ParamDef::text(fk, foot, "").hint("A letter such as H, drawn with a dot; empty names nothing."));
    }
    if has_diagonals(id) {
        v.push(ParamDef::choice("ac_style", "Diagonal AC: line", &STYLE_OPTS, "Automatic").hint("Solid, dashed or dotted; Automatic follows Construction lines."));
        v.push(ParamDef::choice("bd_style", "Diagonal BD: line", &STYLE_OPTS, "Automatic").hint("Solid, dashed or dotted; Automatic follows Construction lines."));
        v.push(ParamDef::text("cross_name", "Name where the diagonals cross", "").hint("A letter such as E, drawn with a dot; empty names nothing."));
    }
    if id == "regular-polygon" {
        v.push(ParamDef::text("value_t", "Size, typed", "").hint("For example 5/2, 2\u{221a}3 or sqrt(13). Overrides the slider and is written as typed; empty = the slider."));
        v.push(ParamDef::choice("apothem_style", "Apothem: line", &STYLE_OPTS, "Automatic").hint("Solid, dashed or dotted; Automatic follows Construction lines."));
        v.push(ParamDef::choice("radius_style", "Radius: line", &STYLE_OPTS, "Automatic").hint("Solid, dashed or dotted; Automatic follows Construction lines."));
        v.push(ParamDef::choice("radii_style", "All the radii: line", &STYLE_OPTS, "Automatic").hint("Solid, dashed or dotted; Automatic draws them like the radius."));
    }
    v
}

// ───────────────────────── line styles ─────────────────────────

/// The dash of the line whose style is `key`: its own choice, or on Automatic the figure's Construction lines.
pub(super) fn dash(p: &Params, key: &str) -> &'static str {
    match p.s(key) { "Solid" => "none", "Dashed" => "6 4", "Dotted" => DOT, _ => dash_of(p) }
}
/// The regular polygon's other radii: their own choice, or on Automatic the radius's style while the radius is drawn (else
/// Construction lines).
pub(super) fn radii_dash(p: &Params) -> &'static str {
    match p.s("radii_style") { "" | "Automatic" if p.b("radius") => dash(p, "radius_style"), _ => dash(p, "radii_style") }
}
/// An explicit dotted construction line: round dots 2.2 px across, 5 px apart.
pub(super) fn dot_line(a: Pt, b: Pt) -> String {
    format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#333" stroke-width="2.2" stroke-dasharray="0.01 5" stroke-linecap="round" data-style="dotted"/>"##, a.0, a.1, b.0, b.1)
}

// ───────────────────────── typed size (regular polygon) ─────────────────────────

/// The range a typed size may take (the slider runs 0.5 to 100).
const SIZE_LIM: (f64, f64) = (0.1, 100.0);

fn size_problem(p: &Params) -> Option<String> {
    let t = p.s("value_t").trim();
    let what = match p.s("from") { "Radius" => "radius", "Apothem" => "apothem", _ => "side" };
    match solids_b8::read(t) {
        Ok(None) => None,
        Err(()) => Some(format!("The {what} \u{201c}{t}\u{201d} can\u{2019}t be read: type a number, a fraction or a square root, like 5, 3/2, 2\u{221a}3 or sqrt(13). The slider\u{2019}s size is drawn.")),
        Ok(Some(v)) if v <= 0.0 => Some(format!("The {what} has to be more than 0 (\u{201c}{t}\u{201d} isn\u{2019}t). The slider\u{2019}s size is drawn.")),
        Ok(Some(v)) if v < SIZE_LIM.0 || v > SIZE_LIM.1 => Some(format!("The {what} \u{201c}{t}\u{201d} is too {} to draw: keep it between 0.1 and 100. The slider\u{2019}s size is drawn.", if v > SIZE_LIM.1 { "big" } else { "small" })),
        Ok(Some(_)) => None,
    }
}
/// The typed size when its box reads and is in range.
fn typed_size(p: &Params) -> Option<f64> {
    if !p.0.contains_key("value_t") { return None; }
    let v = solids_b8::read(p.s("value_t")).ok().flatten()?;
    size_problem(p).is_none().then_some(v)
}
/// The regular polygon's size as drawn: the typed box when it reads, else the slider.
pub(super) fn rp_size(p: &Params) -> f64 { typed_size(p).unwrap_or_else(|| p.f("value")) }
/// Whether the typed size is read (the slider then moves nothing).
pub(crate) fn size_reads(p: &Params) -> bool { typed_size(p).is_some() }
/// The typed size as written on the label of `which` (Side, Radius or Apothem) when the size is given as that measure and
/// typed: exactly as typed, with the unit.
pub(super) fn rp_exact(p: &Params, which: &str) -> Option<String> {
    let given = match p.s("from") { "Radius" => "Radius", "Apothem" => "Apothem", _ => "Side" };
    if given != which { return None; }
    typed_size(p)?;
    let t = solids_b8::pretty(p.s("value_t"));
    let u = p.s("unit").trim();
    Some(if u.is_empty() { t } else { format!("{t} {u}") })
}
/// A construction line's label (from `named_label`) with the typed size written exactly: Measure gives the typed form,
/// "r = measure" gives "r = " and the typed form.
pub(super) fn exact_label(lab: (Option<String>, bool), p: &Params, which: &str) -> (Option<String>, bool) {
    let Some(ex) = rp_exact(p, which) else { return lab };
    match lab {
        (Some(t), true) if t == "=" => (Some(ex), true),
        (Some(t), false) => (Some(format!("{t} = {ex}")), true),
        other => other,
    }
}
/// A drag on the figure sets the size: the slider takes it and the typed box is emptied.
pub(crate) fn set_size(p: &mut Params, v: f64) {
    p.set_f("value", v);
    if p.0.contains_key("value_t") { p.set_s("value_t", ""); }
}

// ───────────────────────── named points ─────────────────────────

/// A point to be named: where it is (model), the corner its height is drawn from (or the point itself), its text and label id.
#[derive(Clone, Debug)]
pub(super) struct Named { pub at: Pt, pub from: Pt, pub text: String, pub id: &'static str }

thread_local! {
    /// Notes for the panel from the last flat shape drawn on this thread: names refused or left off.
    static NOTES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}
fn note(s: String) { NOTES.with(|n| { let mut n = n.borrow_mut(); if !n.contains(&s) { n.push(s); } }); }
/// Starts a flat shape's drawing (clears the notes of the last one).
pub(super) fn begin() { NOTES.with(|n| n.borrow_mut().clear()); }

/// The named points of a figure: each drawn height's foot with its typed name (`feet`: foot key, foot, the corner it is
/// drawn from), and the diagonals' crossing when both are drawn. Names that would mislead are refused with a note.
pub(super) fn points(id: &str, p: &Params, feet: &[(&'static str, Pt, Pt)], corners: &[Pt], cross: Option<Pt>) -> Vec<Named> {
    begin();
    let names = super::flat::corner_names(p, corners.len());
    let scale = corners.iter().map(|q| q.0.abs().max(q.1.abs())).fold(1e-9, f64::max);
    let mut out: Vec<Named> = Vec::new();
    let mut want: Vec<(&'static str, Pt, Pt, String)> = Vec::new();
    for (fk, foot, v) in feet {
        let t = p.s(fk).trim().to_string();
        if !t.is_empty() { want.push((fk, *foot, *v, t)); }
    }
    if let Some(x) = cross { let t = p.s("cross_name").trim().to_string(); if !t.is_empty() { want.push(("cross_name", x, x, t)); } }
    let _ = id;
    for (key, at, from, t) in want {
        let what = what_of(key);
        if t.chars().count() > MAX_NAME {
            note(format!("The name \u{201c}{t}\u{201d} is too long for a point: give {what} a name of up to {MAX_NAME} characters, such as H."));
            continue;
        }
        if let Some(k) = corners.iter().position(|c| dist(*c, at) < 1e-6 * scale) {
            note(format!("{} is corner {}, so that point is already named {}: the name \u{201c}{t}\u{201d} is not drawn.", cap(what), names[k], names[k]));
            continue;
        }
        if names.iter().any(|n| n.eq_ignore_ascii_case(&t)) {
            note(format!("\u{201c}{t}\u{201d} already names a corner: give {what} another name."));
            continue;
        }
        if let Some(o) = out.iter().find(|o| o.text.eq_ignore_ascii_case(&t)) {
            if dist(o.at, at) < 1e-6 * scale { continue; }
            note(format!("\u{201c}{t}\u{201d} already names another point: give {what} another name."));
            continue;
        }
        out.push(Named { at, from, text: t, id: label_id(key) });
    }
    out
}
fn what_of(key: &str) -> &'static str {
    match key {
        "ha_foot" => "the point where the height from A lands",
        "hb_foot" => "the point where the height from B lands",
        "hc_foot" => "the point where the height from C lands",
        "hd_foot" => "the point where the height from D lands",
        _ => "the point where the diagonals cross",
    }
}
fn label_id(key: &str) -> &'static str {
    match key { "ha_foot" => "pa", "hb_foot" => "pb", "hc_foot" => "pc", "hd_foot" => "pd", _ => "px" }
}
fn cap(s: &str) -> String {
    let s = s.strip_prefix("the point where ").map(|r| format!("the {}", r.trim_start_matches("the "))).unwrap_or_else(|| s.to_string());
    let mut c = s.chars();
    match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => s }
}

/// Where the diagonals AC and BD of `pts` cross.
pub(super) fn crossing(pts: &[Pt]) -> Option<Pt> {
    let (a, b, c, d) = (pts[0], pts[1], pts[2], pts[3]);
    let (u, w) = ((c.0 - a.0, c.1 - a.1), (d.0 - b.0, d.1 - b.1));
    let den = u.0 * w.1 - u.1 * w.0;
    if den.abs() < 1e-12 { return None; }
    let s = ((b.0 - a.0) * w.1 - (b.1 - a.1) * w.0) / den;
    Some((a.0 + u.0 * s, a.1 + u.1 * s))
}
/// Whether diagonal AC and diagonal BD are drawn.
pub(super) fn diagonals_on(p: &Params) -> (bool, bool) {
    match p.s("diagonals") { "" | "None" => (false, false), "AC only" => (true, false), "BD only" => (false, true), _ => (true, true) }
}

// ───────────────────────── placing the names ─────────────────────────

type Bx = (f64, f64, f64, f64);
fn attr<'a>(head: &'a str, n: &str) -> &'a str { head.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()).unwrap_or("") }
fn num(head: &str, n: &str) -> f64 { attr(head, n).parse().unwrap_or(f64::NAN) }
fn pts_of(s: &str) -> Vec<Pt> { s.split_whitespace().filter_map(|q| q.split_once(',')).filter_map(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))).collect() }

/// Every stroke of a drawn figure as segments (lines, polyline and polygon edges, arcs as short chords).
pub(super) fn strokes(svg: &str) -> Vec<(Pt, Pt)> {
    let mut out = Vec::new();
    for (i, _) in svg.match_indices("<line ") {
        let h = svg[i..].split('>').next().unwrap_or("");
        out.push(((num(h, "x1"), num(h, "y1")), (num(h, "x2"), num(h, "y2"))));
    }
    for tag in ["<polyline ", "<polygon "] {
        for (i, _) in svg.match_indices(tag) {
            let h = svg[i..].split('>').next().unwrap_or("");
            if attr(h, "stroke") == "none" { continue; }
            let q = pts_of(attr(h, "points"));
            for w in q.windows(2) { out.push((w[0], w[1])); }
            if tag == "<polygon " && q.len() > 2 { out.push((q[q.len() - 1], q[0])); }
        }
    }
    for (i, _) in svg.match_indices("<path ") {
        let h = svg[i..].split('>').next().unwrap_or("");
        out.extend(arc_chords(attr(h, "d")));
    }
    out
}
/// "M x,y A r,r 0 0 s x,y" as chords of about 3 px.
fn arc_chords(d: &str) -> Vec<(Pt, Pt)> {
    let nums: Vec<f64> = d.split(|c: char| c == ',' || c.is_whitespace() || c == 'M' || c == 'A').filter_map(|t| t.parse().ok()).collect();
    if !d.starts_with('M') || !d.contains('A') || nums.len() < 9 { return Vec::new(); }
    let (p1, r, sweep, p2) = ((nums[0], nums[1]), nums[2], nums[6], (nums[7], nums[8]));
    let (mx, my) = ((p1.0 + p2.0) / 2.0, (p1.1 + p2.1) / 2.0);
    let (dx, dy) = (p2.0 - p1.0, p2.1 - p1.1);
    let l = (dx * dx + dy * dy).sqrt();
    if l < 1e-9 || r <= 0.0 { return Vec::new(); }
    let hgt = (r * r - l * l / 4.0).max(0.0).sqrt();
    // small arc (large-arc flag 0): the centre is on the side that makes the sweep run the short way
    let s = if sweep > 0.5 { 1.0 } else { -1.0 };
    let c = (mx - s * dy / l * hgt, my + s * dx / l * hgt);
    let (a0, mut a1) = ((p1.1 - c.1).atan2(p1.0 - c.0), (p2.1 - c.1).atan2(p2.0 - c.0));
    if sweep > 0.5 { while a1 < a0 { a1 += std::f64::consts::TAU; } } else { while a1 > a0 { a1 -= std::f64::consts::TAU; } }
    let n = ((a1 - a0).abs() * r / 3.0).ceil().max(1.0) as usize;
    let at = |k: usize| { let t = a0 + (a1 - a0) * k as f64 / n as f64; (c.0 + r * t.cos(), c.1 + r * t.sin()) };
    (0..n).map(|k| (at(k), at(k + 1))).collect()
}
/// Width of `text` as set in the labels' serif at `size`: the capitals at their own advance widths (Liberation Serif / Times:
/// M and W 0.83 em, I 0.33, most 0.56 to 0.72), everything else as the solids measure it (`solids_b8::serif_w`), a little over.
pub(super) fn glyph_w(text: &str, size: f64) -> f64 {
    text.chars().map(|c| match c {
        'M' | 'W' => 0.833 * 1.06 * size,
        'I' => 0.333 * 1.06 * size,
        'J' | 'S' => 0.5 * 1.06 * size,
        'L' | 'T' | 'Y' | 'Z' => 0.556 * 1.06 * size,
        'A' | 'B' | 'E' | 'F' | 'P' | 'R' | 'V' | 'X' => 0.611 * 1.06 * size,
        'C' | 'K' | 'N' => 0.667 * 1.06 * size,
        'D' | 'G' | 'H' | 'O' | 'Q' | 'U' => 0.722 * 1.06 * size,
        c => solids_b8::serif_w(&c.to_string(), size),
    }).sum()
}

/// Every drawn text's glyph box (as set: the serif's advance widths, cap height above the baseline, a descender below),
/// moved by its label's drag offset.
pub(super) fn text_boxes(svg: &str) -> Vec<(String, Bx)> {
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find("<text") {
        // the drag offset of the group this text is in, if any
        let before = &rest[..i];
        let g = before.rfind("<g data-lbl=").map(|j| &before[j..]).filter(|g| !g.contains("</g>"));
        let (ox, oy) = g.and_then(|g| {
            let h = g.split('>').next()?;
            let t = attr(h, "transform").strip_prefix("translate(")?.strip_suffix(')')?.to_string();
            let v: Vec<f64> = t.split(|c: char| c == ' ' || c == ',').filter_map(|x| x.parse().ok()).collect();
            Some((v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0)))
        }).unwrap_or((0.0, 0.0));
        let tail = &rest[i..];
        let head = tail.split('>').next().unwrap_or("");
        let end = tail.find("</text>").unwrap_or(tail.len());
        let inner = &tail[head.len().min(end)..end];
        let mut text = String::new();
        let mut inside = false;
        for c in inner.chars() { match c { '<' => inside = true, '>' => inside = false, c if !inside => text.push(c), _ => {} } }
        let text = text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">");
        let (x, y, sz) = (num(head, "x"), num(head, "y"), num(head, "font-size"));
        if x.is_finite() && y.is_finite() && sz.is_finite() && !text.trim().is_empty() {
            let w = glyph_w(&text, sz);
            let x0 = match attr(head, "text-anchor") { "start" => x, "end" => x - w, _ => x - w / 2.0 };
            out.push((text, (x0 + ox, y - 0.72 * sz + oy, x0 + w + ox, y + 0.24 * sz + oy)));
        }
        rest = &tail[end.min(tail.len())..];
    }
    out
}
/// Every drawn dot (filled circles) as (centre, radius).
fn dots(svg: &str) -> Vec<(Pt, f64)> {
    svg.match_indices("<circle ").map(|(i, _)| { let h = svg[i..].split('>').next().unwrap_or(""); ((num(h, "cx"), num(h, "cy")), num(h, "r")) }).filter(|(c, r)| c.0.is_finite() && c.1.is_finite() && r.is_finite()).collect()
}
fn seg_box(a: Pt, b: Pt, bx: Bx) -> f64 {
    let n = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / 1.5).ceil().max(1.0) as usize;
    (0..=n).map(|i| {
        let t = i as f64 / n as f64;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        pt_box(x, y, bx)
    }).fold(f64::MAX, f64::min)
}
fn pt_box(x: f64, y: f64, bx: Bx) -> f64 {
    let dx = (bx.0 - x).max(0.0).max(x - bx.2);
    let dy = (bx.1 - y).max(0.0).max(y - bx.3);
    (dx * dx + dy * dy).sqrt()
}
fn overlap(a: Bx, b: Bx, pad: f64) -> bool { !(a.2 + pad <= b.0 || b.2 + pad <= a.0 || a.3 + pad <= b.1 || b.3 + pad <= a.1) }

/// The dot of a named point.
const DOT_R: f64 = 3.4;

/// Draws the named points onto a finished flat shape: each a dot and its letter (the corner letters' size), the letter put
/// where it crosses no stroke, sits on no text or dot, stays inside the page, and is nearer its own dot than any corner or
/// other point, as close to its dot as that allows and preferably on the far side from the corner its height comes from.
/// `named`: (text, label id, screen point, screen point it is seen from); `corners`: the shape's corners on screen.
pub(super) fn place(svg: String, named: &[(String, &'static str, Pt, Pt)], corners: &[Pt], size: f64) -> String {
    let (pw, ph) = crate::figures::svg_size(&svg);
    let lines = strokes(&svg);
    let mut texts: Vec<Bx> = text_boxes(&svg).into_iter().map(|t| t.1).collect();
    let mut dts: Vec<(Pt, f64)> = dots(&svg);
    // every named dot is drawn first, so no letter is put over another point's dot
    for (_, _, at, _) in named { dts.push((*at, DOT_R)); }
    let others = |me: Pt| -> Vec<Pt> { corners.iter().copied().chain(named.iter().map(|n| n.2)).filter(|q| dist(*q, me) > 1e-6).collect() };
    let mut add = String::new();
    for (text, id, at, from) in named {
        // a dot that would sit on another label's text is not drawn
        if texts.iter().any(|b| pt_box(at.0, at.1, *b) < DOT_R + 0.5) {
            note(format!("A label sits where the point \u{201c}{text}\u{201d} is, so the point is left off. Drag that label away to show it."));
            continue;
        }
        let w = glyph_w(text, size);
        let (hw, up, down) = (w / 2.0, 0.72 * size, 0.24 * size);
        // the text is drawn centred on (x, y) with its baseline 0.34 em below y; the italic letter leans right, its top up to
        // about 0.18 em past its advance width and its foot a little left of it
        let bx = |c: Pt| (c.0 - hw - 0.05 * size, c.1 + 0.34 * size - up, c.0 + hw + 0.18 * size, c.1 + 0.34 * size + down);
        let mut pref = (at.0 - from.0, at.1 - from.1);
        let pl = (pref.0 * pref.0 + pref.1 * pref.1).sqrt();
        if pl < 1e-9 { pref = (0.0, 1.0); } else { pref = (pref.0 / pl, pref.1 / pl); }
        let near = others(*at);
        let mut best: Option<(f64, Pt)> = None;
        for k in 0..36 {
            let t = k as f64 * std::f64::consts::TAU / 36.0;
            let dir = (t.cos(), t.sin());
            let dev = (dir.0 * pref.0 + dir.1 * pref.1).clamp(-1.0, 1.0).acos();
            for step in 0..16 {
                let gap = 3.0 + step as f64 * 2.5;
                // out from the dot by its gap plus the box's half extent along the direction
                let ext = hw * dir.0.abs() + (up + down) / 2.0 * dir.1.abs();
                let c0 = (at.0 + dir.0 * (DOT_R + gap + ext), at.1 + dir.1 * (DOT_R + gap + ext));
                // the box's middle is (up − down)/2 − 0.34 em above c.1's baseline offset
                let c = (c0.0, c0.1 - (0.34 * size + (down - up) / 2.0));
                let b = bx(c);
                let score = gap + 9.0 * dev;
                if best.is_some_and(|(s, _)| s <= score) { continue; }
                if b.0 < 2.0 || b.1 < 2.0 || b.2 > pw - 2.0 || b.3 > ph - 2.0 { continue; }
                // clear of strokes by more than the letter's white halo (0.1 em)
                if lines.iter().any(|(p, q)| seg_box(*p, *q, b) < 2.5) { continue; }
                // a clear gap from other text, so it never reads as part of another label ("E12")
                if texts.iter().any(|o| overlap(*o, b, 5.0)) { continue; }
                if dts.iter().any(|(q, r)| pt_box(q.0, q.1, b) < r + 1.5) { continue; }
                let mine = pt_box(at.0, at.1, b);
                // plainly nearer its own dot than any corner or other point
                if near.iter().any(|q| pt_box(q.0, q.1, b) < (mine + 6.0).max(mine * 1.5)) { continue; }
                best = Some((score, c));
                break;
            }
        }
        let Some((_, c)) = best else {
            note(format!("There is no clear place for the name \u{201c}{text}\u{201d} of a point, so the point is left off. Turn the figure, make the labels smaller or drag a label away to show it."));
            continue;
        };
        texts.push(bx(c));
        add.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="{DOT_R:.2}" fill="{INK}" stroke="{INK}" stroke-width="1" data-point="{id}"/>"##, at.0, at.1));
        add.push_str(&crate::figures::labels::wrap(id, c, crate::figures::labels::text_half(text, size), &mlabel_svg(c.0, c.1, size, text, "")));
    }
    if add.is_empty() { return svg; }
    match svg.rfind("</svg>") { Some(i) => format!("{}{add}{}", &svg[..i], &svg[i..]), None => svg }
}

// ───────────────────────── panel ─────────────────────────

/// The heights figure `id` draws now: (style key, foot key, corner drawn from, the corner, the foot).
fn heights_drawn(id: &str, p: &Params) -> Vec<(&'static str, &'static str, Pt, Pt)> {
    let proj = |v: Pt, a: Pt, b: Pt| { let u = unit_v(a, b); let t = (v.0 - a.0) * u.0 + (v.1 - a.1) * u.1; (a.0 + u.0 * t, a.1 + u.1 * t) };
    let keys = height_keys(id);
    let k = |c: &str| keys.iter().find(|x| x.2 == c).map(|x| (x.0, x.1)).unwrap();
    let mut out = Vec::new();
    match id {
        "triangle" => {
            let Ok(s) = tri_solve(p) else { return out };
            let [a, b, c] = s.pts;
            let hs = p.s("height");
            let has = |v: &str| hs == "From all three" || hs.split(|ch: char| !ch.is_alphanumeric()).any(|t| t == v);
            for (on, n, v, q1, q2) in [(has("A"), "A", a, b, c), (has("B"), "B", b, c, a), (has("C"), "C", c, a, b)] {
                if on { let (sk, fk) = k(n); out.push((sk, fk, v, proj(v, q1, q2))); }
            }
        }
        "parallelogram" | "kite" => {
            let Ok(s) = quad_solve(id, p) else { return out };
            let [a, b, c, d] = s.pts;
            let list: Vec<(&str, Pt, Pt, Pt)> = match (id, p.s("height")) {
                (_, "Onto AB (from D)") | (_, "From D to AB") => vec![("D", d, a, b)],
                (_, "Onto AD (from B)") => vec![("B", b, a, d)],
                (_, "Onto AB (from C)") | (_, "From C to AB") => vec![("C", c, a, b)],
                (_, "Onto AB (from D and C)") | (_, "From D and C to AB") => vec![("D", d, a, b), ("C", c, a, b)],
                _ => vec![],
            };
            for (n, v, q1, q2) in list { let (sk, fk) = k(n); out.push((sk, fk, v, proj(v, q1, q2))); }
        }
        "trapezoid" => {
            let Ok(s) = quad_solve(id, p) else { return out };
            if !p.b("show_height") { return out; }
            let [a, b, c, d] = s.pts;
            let hf = p.s("hfrom");
            let (from_d, from_c) = (hf != "C", hf == "C" || hf == "D and C");
            if from_d {
                let (sk, fk) = k("D");
                if d.0.abs() > 1e-9 { out.push((sk, fk, d, proj(d, a, b))); } else { out.push((sk, fk, (b.0 / 2.0, s.h.unwrap_or(d.1)), (b.0 / 2.0, 0.0))); }
            }
            if from_c && (c.0 - b.0).abs() > 1e-9 { let (sk, fk) = k("C"); out.push((sk, fk, c, proj(c, a, b))); }
        }
        _ => {}
    }
    out
}
fn corners_of(id: &str, p: &Params) -> Vec<Pt> {
    match id {
        "triangle" => tri_solve(p).map(|s| s.pts.to_vec()).unwrap_or_default(),
        "rectangle" | "parallelogram" | "trapezoid" | "kite" => quad_solve(id, p).map(|s| s.pts.to_vec()).unwrap_or_default(),
        _ => Vec::new(),
    }
}
fn solves(id: &str, p: &Params) -> bool { !corners_of(id, p).is_empty() }

/// The construction lines drawn now, by their style keys (the other radii by "radii_style").
fn lines_drawn(id: &str, p: &Params) -> Vec<&'static str> {
    if id == "regular-polygon" {
        let mut v = Vec::new();
        if p.b("apothem") { v.push("apothem_style"); }
        if p.b("radius") { v.push("radius_style"); }
        if p.b("all_radii") { v.push("radii_style"); }
        return v;
    }
    let mut v: Vec<&'static str> = heights_drawn(id, p).iter().map(|h| h.0).collect();
    if has_diagonals(id) && solves(id, p) {
        let (ac, bd) = diagonals_on(p);
        if ac { v.push("ac_style"); }
        if bd { v.push("bd_style"); }
    }
    v
}

/// Controls shown only where they act: a line's style while it is drawn; a foot's name while its height is drawn and does
/// not land on a corner; the crossing's name while both diagonals are drawn; Construction lines while a drawn line follows
/// it; the size slider while its typed box is not read.
pub(crate) fn relevant(id: &str, key: &str, p: &Params) -> bool {
    if !IDS.contains(&id) { return true; }
    let auto = |k: &str| matches!(p.s(k), "" | "Automatic");
    if key == "line_style" {
        // only where some drawn line follows it (the other radii follow the radius on Automatic)
        return lines_drawn(id, p).iter().any(|k| auto(k) && (*k != "radii_style" || auto("radius_style") || !p.b("radius")));
    }
    if id == "regular-polygon" {
        return match key {
            "value" => !size_reads(p),
            "apothem_style" => p.b("apothem"),
            "radius_style" => p.b("radius"),
            "radii_style" => p.b("all_radii"),
            _ => true,
        };
    }
    if let Some(&(sk, fk, _)) = height_keys(id).iter().find(|h| h.0 == key || h.1 == key) {
        let hs = heights_drawn(id, p);
        let Some(h) = hs.iter().find(|h| h.0 == sk) else { return false };
        if key == fk {
            let cs = corners_of(id, p);
            let scale = cs.iter().map(|q| q.0.abs().max(q.1.abs())).fold(1e-9, f64::max);
            return !cs.iter().any(|c| dist(*c, h.3) < 1e-6 * scale);
        }
        return true;
    }
    if has_diagonals(id) {
        let (ac, bd) = diagonals_on(p);
        let ok = solves(id, p);
        return match key { "ac_style" => ok && ac, "bd_style" => ok && bd, "cross_name" => ok && ac && bd, _ => true };
    }
    true
}

/// The panel's notes: a typed size refused, names refused or left off (read from drawing the figure, so they say exactly
/// what was drawn).
pub(crate) fn warnings(id: &str, p: &Params) -> Vec<String> {
    if !IDS.contains(&id) { return Vec::new(); }
    let mut out = Vec::new();
    if id == "regular-polygon" && let Some(m) = size_problem(p) { out.push(m); }
    if let Some(r) = render_for(id) {
        begin();
        let _ = crate::figures::labels::rendering(p, || r(p));
        out.extend(NOTES.with(|n| n.borrow().clone()));
    }
    out
}
