//! Solids, batch 8: every measure of the cylinder, prism, cone and pyramid (radius or diameter, height, lean, length, width,
//! side) can be typed as a number, a fraction or with a square root (3/2, 2√3, sqrt(13)/2, 5pi), and the typed form is the one
//! written on the figure's label (exact, as the reference keeps it); worked-out measures stay rounded as before. A typed box
//! overrides its slider, and the slider is hidden while the box is read. The oblique cylinder's height line and the prism's
//! height line can be switched off.
//!
//! Every new control's default (an empty box, the height line on) draws what the figure drew before, through the same drawing
//! statements, so old saved figures and share links draw byte for byte as before (`solids_b8_frozen.rs`).
use super::*;

/// The measures that can be typed: (slider key, typed key) per figure.
pub(crate) fn keys(id: &str) -> &'static [(&'static str, &'static str)] {
    const ROUND: [(&str, &str); 4] = [("radius", "radius_t"), ("diam", "diam_t"), ("height", "height_t"), ("lean", "lean_t")];
    const FLAT: [(&str, &str); 5] = [("length", "length_t"), ("width", "width_t"), ("side", "side_t"), ("height", "height_t"), ("lean", "lean_t")];
    match id { "cylinder" | "cone" => &ROUND, "prism" | "pyramid" => &FLAT, _ => &[] }
}
/// The typed box of slider `key` on figure `id`, if it has one.
fn tkey(id: &str, key: &str) -> Option<&'static str> { keys(id).iter().find(|k| k.0 == key).map(|k| k.1) }
/// Whether `key` is one of the typed boxes (any of the four figures).
pub(crate) fn is_typed_key(id: &str, key: &str) -> bool { keys(id).iter().any(|k| k.1 == key) }

/// The new controls of figure `id`, appended to its list (old keys first, unchanged).
pub(super) fn params(id: &str) -> Vec<ParamDef> {
    let mut v: Vec<ParamDef> = keys(id).iter().map(|(k, t)| {
        let (name, hint) = match *k {
            "radius" => ("Radius, typed", "For example 3/2, 2\u{221a}3 or sqrt(13). Overrides the slider and is written as typed; empty = the slider."),
            "diam" => ("Diameter, typed", "For example 7/2 or 4\u{221a}2. Overrides the slider and is written as typed; empty = the slider."),
            "height" => ("Height, typed", "For example 17/3 or 5\u{221a}13. Overrides the slider and is written as typed; empty = the slider."),
            "lean" => ("Lean, typed", "For example 3/2, or \u{2212}3/2 to lean left. Overrides the slider and is written as typed; empty = the slider."),
            "length" => ("Length, typed", "For example 15/2 or 2\u{221a}3. Overrides the slider and is written as typed; empty = the slider."),
            "width" => ("Width, typed", "For example 9/4 or \u{221a}5. Overrides the slider and is written as typed; empty = the slider."),
            _ => ("Side, typed", "For example 5/2 or 2\u{221a}2. Overrides the slider and is written as typed; empty = the slider."),
        };
        ParamDef::text(t, name, "").hint(hint)
    }).collect();
    match id {
        "cylinder" => v.push(ParamDef::bool("obl_height", "Height dashed", true).hint("Oblique: the height straight down to the base, with the lean.")),
        "prism" => v.push(ParamDef::bool("show_height", "Height line", true).hint("Oblique: the height straight down to the base, with the lean.")),
        _ => {}
    }
    v
}

// ───────────────────────── reading ─────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum Tok { Num(f64), Sqrt, Pi, Op(char), Open, Close }

/// Splits a typed measure into tokens; None when it holds anything but numbers, sqrt / √, pi / π, + − × * / and brackets.
fn lex(t: &str) -> Option<Vec<Tok>> {
    let cs: Vec<char> = t.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() { i += 1; continue; }
        if c.is_ascii_digit() || c == '.' {
            let st = i;
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') { i += 1; }
            let s: String = cs[st..i].iter().collect();
            out.push(Tok::Num(s.parse().ok()?));
            continue;
        }
        let rest: String = cs[i..].iter().take(4).collect::<String>().to_lowercase();
        if rest.starts_with("sqrt") { out.push(Tok::Sqrt); i += 4; continue; }
        if rest.starts_with("pi") { out.push(Tok::Pi); i += 2; continue; }
        out.push(match c {
            '\u{221a}' => Tok::Sqrt,
            '\u{3c0}' => Tok::Pi,
            '+' => Tok::Op('+'),
            '-' | '\u{2212}' | '\u{2013}' => Tok::Op('-'),
            '*' | '\u{d7}' | '\u{b7}' => Tok::Op('*'),
            '/' | '\u{f7}' => Tok::Op('/'),
            '(' => Tok::Open,
            ')' => Tok::Close,
            _ => return None,
        });
        i += 1;
    }
    Some(out)
}

/// expr := term (('+'|'-') term)*; term := unary (('*'|'/'|implicit) unary)*; unary := '-' unary | power;
/// power := '√' power | number | 'π' | '(' expr ')'.
struct Rd { t: Vec<Tok>, i: usize }
impl Rd {
    fn peek(&self) -> Option<&Tok> { self.t.get(self.i) }
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        while let Some(Tok::Op(c @ ('+' | '-'))) = self.peek().cloned() { self.i += 1; let r = self.term()?; v = if c == '+' { v + r } else { v - r }; }
        Some(v)
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        loop {
            match self.peek() {
                Some(Tok::Op('*')) => { self.i += 1; v *= self.unary()?; }
                Some(Tok::Op('/')) => { self.i += 1; v /= self.unary()?; }
                // 2√3, 3π, 2(1/2): a number then a root, π or bracket multiplies (two numbers side by side, as in a
                // mixed number 3 1/2, are not read: refused rather than drawn as 3 × 1/2)
                Some(Tok::Sqrt | Tok::Pi | Tok::Open) => v *= self.unary()?,
                _ => return Some(v),
            }
        }
    }
    fn unary(&mut self) -> Option<f64> {
        if let Some(Tok::Op('-')) = self.peek() { self.i += 1; return Some(-self.unary()?); }
        self.power()
    }
    fn power(&mut self) -> Option<f64> {
        let t = self.peek()?.clone();
        self.i += 1;
        match t {
            Tok::Sqrt => { let v = self.power()?; (v >= 0.0).then(|| v.sqrt()) }
            Tok::Num(v) => Some(v),
            Tok::Pi => Some(std::f64::consts::PI),
            Tok::Open => { let v = self.expr()?; (self.peek() == Some(&Tok::Close)).then(|| { self.i += 1; v }) }
            _ => None,
        }
    }
}

/// The value of a typed measure: Ok(None) for an empty box, Err for what cannot be read.
pub(crate) fn read(text: &str) -> Result<Option<f64>, ()> {
    let t = text.trim();
    if t.is_empty() { return Ok(None); }
    let toks = lex(t).ok_or(())?;
    let mut r = Rd { t: toks, i: 0 };
    let v = r.expr().ok_or(())?;
    if r.i != r.t.len() || !v.is_finite() { return Err(()); }
    Ok(Some(v))
}

/// The range a typed measure may take: lengths from 0.1 to 100 (the sliders run 0.5 to 50), a diameter twice that, a lean
/// up to 40 either way. Outside it the figure would be a flat line or a needle.
fn limits(key: &str) -> (f64, f64) { match key { "diam" => (0.2, 200.0), "lean" => (-40.0, 40.0), _ => (0.1, 100.0) } }

/// What is wrong with the typed box of `key`, or None when it reads (or is empty).
fn problem(p: &Params, key: &str, tk: &str) -> Option<String> {
    let t = p.s(tk).trim();
    let name = match key { "radius" => "radius", "diam" => "diameter", "height" => "height", "lean" => "lean", "length" => "length", "width" => "width", _ => "side" };
    let (lo, hi) = limits(key);
    match read(t) {
        Ok(None) => None,
        Err(()) => Some(format!("The {name} \u{201c}{t}\u{201d} can\u{2019}t be read: type a number, a fraction or a square root, like 5, 3/2, 2\u{221a}3 or sqrt(13). The slider\u{2019}s value is drawn.")),
        Ok(Some(v)) if key == "lean" && v.abs() < 1e-9 => Some("A lean of 0 is a right solid: choose Stands: Right instead. The slider\u{2019}s lean is drawn.".into()),
        Ok(Some(v)) if key != "lean" && v <= 0.0 => Some(format!("The {name} has to be more than 0 (\u{201c}{t}\u{201d} isn\u{2019}t). The slider\u{2019}s value is drawn.")),
        Ok(Some(v)) if v < lo || v > hi => Some(format!("The {name} \u{201c}{t}\u{201d} is too {} to draw: keep it between {} and {}. The slider\u{2019}s value is drawn.", if v.abs() > hi { "big" } else { "small" }, fmt(if key == "lean" { 0.0 } else { lo }, 1), fmt(hi, 0))),
        Ok(Some(_)) => None,
    }
}

/// The typed value of `key` when its box reads and is in range.
fn typed_val(p: &Params, key: &str, tk: &str) -> Option<f64> {
    let v = read(p.s(tk)).ok().flatten()?;
    problem(p, key, tk).is_none().then_some(v)
}

/// The value drawn for measure `key`: the typed box when it reads, else the slider (for the sphere and any figure without the
/// box, always the slider, so what they draw is unchanged).
pub(crate) fn val(p: &Params, key: &str) -> f64 {
    for id in ["cylinder", "prism"] {
        if let Some(tk) = tkey(id, key) && p.0.contains_key(tk) && let Some(v) = typed_val(p, key, tk) { return v; }
    }
    p.f(key)
}
/// Whether measure `key`'s typed box is being read (its slider then moves nothing).
pub(crate) fn reads(p: &Params, key: &str) -> bool {
    ["cylinder", "prism"].iter().any(|id| tkey(id, key).is_some_and(|tk| p.0.contains_key(tk) && typed_val(p, key, tk).is_some()))
}

// ───────────────────────── writing ─────────────────────────

/// A typed measure as written on the figure: √ for sqrt, π for pi, a true minus, no spaces or stars except around + and −
/// (3/2, 2√3, √13/2, 5π, √(2+3)).
pub(crate) fn pretty(text: &str) -> String {
    let Some(toks) = lex(text.trim()) else { return text.trim().to_string() };
    let mut out = String::new();
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Num(_) => {
                // keep the digits as typed
                out.push_str(&num_text(text, i));
            }
            Tok::Sqrt => {
                out.push('\u{221a}');
                // √(13) is written √13: brackets round a single number go
                if let (Some(Tok::Open), Some(Tok::Num(_)), Some(Tok::Close)) = (toks.get(i + 1), toks.get(i + 2), toks.get(i + 3)) {
                    out.push_str(&num_text(text, i + 2));
                    i += 4;
                    continue;
                }
            }
            Tok::Pi => out.push('\u{3c0}'),
            Tok::Op('*') => {
                let next_implicit = matches!(toks.get(i + 1), Some(Tok::Sqrt | Tok::Pi | Tok::Open));
                if !next_implicit { out.push('\u{d7}'); }
            }
            Tok::Op('+') => out.push_str(" + "),
            Tok::Op('-') => out.push_str(if i == 0 || matches!(toks[i - 1], Tok::Open | Tok::Op(_)) { "\u{2212}" } else { " \u{2212} " }),
            Tok::Op(c) => out.push(*c),
            Tok::Open => out.push('('),
            Tok::Close => out.push(')'),
        }
        i += 1;
    }
    out
}
/// The digits of the `k`-th token of `text` exactly as typed (2.50 stays 2.50).
fn num_text(text: &str, k: usize) -> String {
    let cs: Vec<char> = text.trim().chars().collect();
    let (mut i, mut n) = (0, 0);
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() { i += 1; continue; }
        let st = i;
        if c.is_ascii_digit() || c == '.' {
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') { i += 1; }
        } else {
            let rest: String = cs[i..].iter().take(4).collect::<String>().to_lowercase();
            i += if rest.starts_with("sqrt") { 4 } else if rest.starts_with("pi") { 2 } else { 1 };
        }
        if n == k { return cs[st..i].iter().collect(); }
        n += 1;
    }
    String::new()
}

/// The label text of measure `key` when it is drawn from its typed box: the exact form as typed (a lean without its sign, the
/// side it leans to is drawn), with the unit. None when the slider is drawn.
pub(crate) fn exact(p: &Params, key: &str) -> Option<String> {
    let tk = ["cylinder", "prism"].iter().find_map(|id| tkey(id, key))?;
    if !p.0.contains_key(tk) || typed_val(p, key, tk).is_none() { return None; }
    let mut t = pretty(p.s(tk));
    if key == "lean" && let Some(r) = t.strip_prefix('\u{2212}') { t = r.to_string(); }
    let u = p.s("unit").trim();
    Some(if u.is_empty() { t } else { format!("{t} {u}") })
}

// ───────────────────────── panel ─────────────────────────

/// Controls shown only where they act: a slider whose typed box is read moves nothing and is hidden; the height switches only
/// where they draw (the oblique cylinder's under Oblique, the right cylinder's otherwise; the prism's for an oblique prism).
pub(crate) fn relevant(id: &str, key: &str, p: &Params) -> bool {
    if !matches!(id, "cylinder" | "prism" | "cone" | "pyramid") { return true; }
    if tkey(id, key).is_some() && reads(p, key) { return false; }
    let oblique = p.s("stands") == "Oblique";
    match (id, key) {
        ("cylinder", "obl_height") => oblique,
        ("cylinder", "show_height") => !oblique,
        ("prism", "show_height") => oblique && !(p.s("base") != "Rectangle" && p.s("sits") == "Lying on its side"),
        _ => true,
    }
}

/// The panel's notes: typed measures refused (the slider's value is drawn instead) and labels a switched-off height line takes
/// away with it.
pub(crate) fn warnings(id: &str, p: &Params) -> Vec<String> {
    let mut out = Vec::new();
    for (k, tk) in keys(id) {
        // the diameter box only counts with "Give the diameter instead", the radius box only without it
        if (*k == "diam" && !p.b("give_d")) || (*k == "radius" && p.b("give_d")) { continue; }
        if *k == "lean" && p.s("stands") != "Oblique" { continue; }
        if let Some(m) = problem(p, k, tk) { out.push(m); }
    }
    if height_off(id, p) {
        let on = |k: &str| p.s(k) != "None";
        let mut gone = Vec::new();
        if on(if id == "cylinder" { "h_lab" } else { "height_lab" }) { gone.push("height"); }
        if on("lean_lab") { gone.push("lean"); }
        if !gone.is_empty() { out.push(format!("With the height line off, the {} label{} not drawn: {} on the height line. Switch Height on to show {}.", gone.join(" and "), if gone.len() > 1 { "s are" } else { " is" }, if gone.len() > 1 { "they sit" } else { "it sits" }, if gone.len() > 1 { "them" } else { "it" })); }
    }
    out
}

/// The oblique cylinder's or prism's height line is switched off.
pub(crate) fn height_off(id: &str, p: &Params) -> bool {
    let oblique = p.s("stands") == "Oblique" && val(p, "lean").abs() > 1e-9;
    match id {
        "cylinder" => oblique && p.0.contains_key("obl_height") && !p.b("obl_height"),
        "prism" => oblique && !(p.s("base") != "Rectangle" && p.s("sits") == "Lying on its side") && p.0.contains_key("show_height") && !p.b("show_height"),
        _ => false,
    }
}

/// Grey hints of the typed boxes: the slider's value, so an empty box shows what is drawn.
pub(crate) fn hints(id: &str, p: &Params) -> Vec<(&'static str, String)> {
    keys(id).iter().map(|(k, tk)| (*tk, fmt(p.f(k), 3))).collect()
}

/// A drag on the figure sets measure `key` to `v`: the slider takes the dragged value and the typed box is emptied (a drag
/// gives a decimal, not the exact form), so the slider shows again and what is drawn follows the drag.
pub(crate) fn set(p: &mut Params, key: &str, v: f64) {
    p.set_f(key, v);
    for id in ["cylinder", "prism"] {
        if let Some(tk) = tkey(id, key) && p.0.contains_key(tk) { p.set_s(tk, ""); }
    }
}

/// The solid's fit on its 560 by 380 page: the usual 420 by 300 box 70 in from the left, narrowed only when a typed label is
/// wider than the room left beside the solid (a 5\u{221a}13 cm beside a wide, flat solid at Large), so a label beside an outer edge
/// stays on the page. A figure with no typed measure, or with room for it, is fitted exactly as before.
pub(super) fn fit(all: &[Pt], p: &Params) -> Map {
    let m = Map::fit_at(all, 420.0, 300.0, 70.0, 40.0);
    let fs = lsz3(p);
    let widest = ["radius", "diam", "height", "lean", "length", "width", "side"].iter().filter_map(|k| exact(p, k)).map(|t| serif_w(&t, fs)).fold(0.0, f64::max);
    if widest == 0.0 { return m; }
    // the label's gap from its edge (5.4) and a little of its white halo
    let need = widest / 1.04 + 6.5;
    let (lo, hi) = all.iter().map(|q| m.p(*q).0).fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
    if lo >= need && 560.0 - hi >= need { return m; }
    Map::fit_at(all, 560.0 - 2.0 * need, 300.0, need, 40.0)
}

/// Width of `text` set in the solids' serif at `size`: Liberation Serif / Times advance widths (digits 0.5 em, the slash and
/// space 0.25 to 0.28, √ and the operators about 0.55, m 0.78), a little over for safety.
pub(crate) fn serif_w(text: &str, size: f64) -> f64 {
    text.chars().map(|c| match c {
        '0'..='9' => 0.5, '/' => 0.28, ' ' | '.' => 0.25, '(' | ')' => 0.333, '\u{221a}' => 0.55, '\u{3c0}' => 0.55, '\u{2212}' | '+' | '\u{d7}' => 0.564,
        'm' => 0.778, 'w' => 0.722, 'i' | 'l' | 'j' | 't' | 'f' => 0.3, 'r' => 0.333, 'c' | 'e' | 's' | 'z' => 0.444, _ => 0.5,
    }).sum::<f64>() * size * 1.04
}

// ───────────────────────── keeping typed figures' labels clear ─────────────────────────

/// A figure with a typed measure in use (its labels are then cleared by `clear`; others are drawn exactly as before).
pub(crate) fn any_typed(p: &Params) -> bool { ["radius", "diam", "height", "lean", "length", "width", "side"].iter().any(|k| reads(p, k)) }

type Bx = (f64, f64, f64, f64);
fn attr<'a>(head: &'a str, n: &str) -> &'a str { head.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()).unwrap_or("") }
fn num(head: &str, n: &str) -> f64 { attr(head, n).parse().unwrap_or(f64::NAN) }

/// Every inked stroke of a drawn solid as segments (lines, and the edges of stroked polylines and polygons).
fn strokes(svg: &str) -> Vec<(Pt, Pt)> {
    let mut out = Vec::new();
    for (i, _) in svg.match_indices("<line") {
        let h = svg[i..].split('>').next().unwrap_or("");
        out.push(((num(h, "x1"), num(h, "y1")), (num(h, "x2"), num(h, "y2"))));
    }
    for tag in ["<polyline", "<polygon"] {
        for (i, _) in svg.match_indices(tag) {
            let h = svg[i + tag.len()..].split('>').next().unwrap_or("");
            if attr(h, "stroke") == "none" || attr(h, "stroke-width").parse::<f64>().unwrap_or(1.0) < 1.0 { continue; }
            let pts: Vec<Pt> = attr(h, "points").split_whitespace().filter_map(|q| q.split_once(',')).filter_map(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))).collect();
            for w in pts.windows(2) { out.push((w[0], w[1])); }
            if tag == "<polygon" && pts.len() > 2 { out.push((pts[pts.len() - 1], pts[0])); }
        }
    }
    out
}
/// The outlines only: solid strokes 2 px or more wide.
fn strokes_hard(svg: &str) -> Vec<(Pt, Pt)> {
    let mut out = Vec::new();
    for tag in ["<line", "<polyline", "<polygon"] {
        for (i, _) in svg.match_indices(tag) {
            let h = svg[i + tag.len()..].split('>').next().unwrap_or("");
            if !matches!(attr(h, "stroke-dasharray"), "" | "none") || attr(h, "stroke") == "none" || attr(h, "stroke-width").parse::<f64>().unwrap_or(1.0) < 2.0 { continue; }
            if tag == "<line" { out.push(((num(h, "x1"), num(h, "y1")), (num(h, "x2"), num(h, "y2")))); continue; }
            let pts: Vec<Pt> = attr(h, "points").split_whitespace().filter_map(|q| q.split_once(',')).filter_map(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))).collect();
            for w in pts.windows(2) { out.push((w[0], w[1])); }
            if tag == "<polygon" && pts.len() > 2 { out.push((pts[pts.len() - 1], pts[0])); }
        }
    }
    out
}
fn seg_hits(a: Pt, b: Pt, bx: Bx) -> bool {
    let n = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / 2.0).ceil().max(1.0) as usize;
    (0..=n).any(|i| {
        let t = i as f64 / n as f64;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        x > bx.0 && x < bx.2 && y > bx.1 && y < bx.3
    })
}
fn overlap(a: Bx, b: Bx) -> bool { !(a.2 <= b.0 || b.2 <= a.0 || a.3 <= b.1 || b.3 <= a.1) }

/// Moves each label of a figure with typed measures, in drawing order, to the nearest spot where it crosses no line, sits on no
/// label already placed and stays on the page (a small spiral round where it was put, so it stays by its own line). A figure
/// with no typed measure is returned unchanged.
pub(super) fn clear(svg: String, p: &Params) -> String {
    let own = OWN.with(|o| o.borrow_mut().take()).unwrap_or_default();
    #[cfg(test)]
    LAST_OWN.with(|l| *l.borrow_mut() = own.clone());
    if !any_typed(p) { return svg; }
    let lines = strokes(&svg);
    // the outlines (solid strokes of 2 px or more); the rest are dashed hidden edges and construction lines
    let hard: Vec<(Pt, Pt)> = strokes_hard(&svg);
    let (pw, ph) = crate::figures::svg_size(&svg);
    let mut placed: Vec<Bx> = Vec::new();
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg.as_str();
    while let Some(i) = rest.find("<g data-lbl=\"") {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(end) = tail.find("</g>") else { break };
        let grp = &tail[..end + 4];
        rest = &tail[end + 4..];
        let ghead = grp.split('>').next().unwrap_or("");
        let Some(ti) = grp.find("<text") else { out.push_str(grp); continue };
        let thead = grp[ti..].split('>').next().unwrap_or("");
        let text = grp[ti..].split('>').nth(1).and_then(|r| r.split('<').next()).unwrap_or("").replace("&amp;", "&");
        let (x, y, sz) = (num(thead, "x"), num(thead, "y"), num(thead, "font-size"));
        // a dragged label stays where the teacher put it
        if ghead.contains("transform=") || !(x.is_finite() && y.is_finite() && sz.is_finite()) { out.push_str(grp); continue; }
        let w = serif_w(&text, sz) / 2.0 + 1.5;
        let bx = |dx: f64, dy: f64| (x + dx - w, y + dy - sz * 0.8, x + dx + w, y + dy + sz * 0.27);
        let ok = |b: Bx| b.0 >= 1.0 && b.1 >= 1.0 && b.2 <= pw - 1.0 && b.3 <= ph - 1.0 && !lines.iter().any(|(a, c)| seg_hits(*a, *c, b)) && !placed.iter().any(|q| overlap(*q, b));
        // a label beside a line stays by that line: nearer it than any other labelled line, and close to it
        let id = attr(ghead, "data-lbl");
        let mine = own.iter().find(|o| o.0 == id).map(|o| (o.1, o.2));
        let others: Vec<(Pt, Pt)> = own.iter().filter(|o| o.0 != id).map(|o| (o.1, o.2)).collect();
        let belongs = |b: Bx| mine.is_none_or(|(a, c)| { let d = seg_box(a, c, b); d <= 16.0 && others.iter().all(|(x, y)| seg_box(*x, *y, b) > d + 4.0) });
        let mut best = (0.0, 0.0);
        if !(ok(bx(0.0, 0.0)) && belongs(bx(0.0, 0.0))) {
            // first beside its own line: slid along it from where it was put, on its side then the other, a little further out
            let along = |strict: bool| mine.and_then(|(a, c)| {
                let len = ((c.0 - a.0).powi(2) + (c.1 - a.1).powi(2)).sqrt().max(1e-9);
                let (u, nrm) = (((c.0 - a.0) / len, (c.1 - a.1) / len), (-(c.1 - a.1) / len, (c.0 - a.0) / len));
                let t0 = (((x - a.0) * u.0 + (y - sz * 0.27 - a.1) * u.1) / len).clamp(0.0, 1.0);
                let side0 = if (x - a.0) * nrm.0 + (y - sz * 0.27 - a.1) * nrm.1 >= 0.0 { 1.0 } else { -1.0 };
                // along the line; when nothing is clear there, also a little past either end
                let mut ts: Vec<f64> = if strict { (0..=20).map(|k| k as f64 / 20.0).collect() } else { (-8..=28).map(|k| k as f64 / 20.0).collect() };
                ts.sort_by(|p, q| ((p - t0).abs() + if (0.0..=1.0).contains(p) { 0.0 } else { 2.0 }).partial_cmp(&((q - t0).abs() + if (0.0..=1.0).contains(q) { 0.0 } else { 2.0 })).unwrap());
                let (hw, hh) = (w, sz * 0.535);
                // clear of every line; failing that, over a dashed hidden edge (its white halo keeps it readable) but never on an
                // outline or its own line, and never moved off its own line to where it would read as another's
                let soft_ok = |b: Bx| b.0 >= 1.0 && b.1 >= 1.0 && b.2 <= pw - 1.0 && b.3 <= ph - 1.0 && !hard.iter().any(|(p, q)| seg_hits(*p, *q, b)) && !seg_hits(a, c, b) && !placed.iter().any(|q| overlap(*q, b));
                {
                    for side in [side0, -side0] {
                        for gap in [5.4, 9.0, 14.0] {
                            for &t in &ts {
                                let n = (nrm.0 * side, nrm.1 * side);
                                let off = hw * n.0.abs() + hh * n.1.abs() + gap;
                                let (cx, cy) = (a.0 + (c.0 - a.0) * t + n.0 * off, a.1 + (c.1 - a.1) * t + n.1 * off);
                                // the box's centre is 0.27 em under the baseline's centre line
                                let (dx, dy) = (cx - x, cy + sz * 0.265 - y);
                                let b = bx(dx, dy);
                                if (if strict { ok(b) } else { soft_ok(b) }) && belongs(b) { return Some((dx, dy)); }
                            }
                        }
                    }
                }
                None
            });
            let search = |strict: bool| {
                for r in (1..=60).map(|k| k as f64 * 3.0) {
                    for k in 0..48 {
                        let t = k as f64 * std::f64::consts::TAU / 48.0;
                        let (dx, dy) = (r * t.cos(), r * t.sin());
                        let b = bx(dx, dy);
                        if ok(b) && (!strict || belongs(b)) { return Some((dx, dy)); }
                    }
                }
                None
            };
            best = along(true).or_else(|| search(true)).or_else(|| along(false)).or_else(|| if ok(bx(0.0, 0.0)) { Some((0.0, 0.0)) } else { search(false) }).unwrap_or((0.0, 0.0));
        }
        placed.push(bx(best.0, best.1));
        if best == (0.0, 0.0) { out.push_str(grp); continue; }
        let (cx, cy) = { let v: Vec<f64> = attr(ghead, "data-box").split_whitespace().filter_map(|v| v.parse().ok()).collect(); (v.first().copied().unwrap_or(x), v.get(1).copied().unwrap_or(y)) };
        let g2 = grp
            .replacen(&format!("data-box=\"{cx:.2} {cy:.2}"), &format!("data-box=\"{:.2} {:.2}", cx + best.0, cy + best.1), 1)
            .replacen(&format!("<text x=\"{x:.2}\" y=\"{y:.2}\""), &format!("<text x=\"{:.2}\" y=\"{:.2}\"", x + best.0, y + best.1), 1);
        out.push_str(&g2);
    }
    out.push_str(rest);
    out
}

thread_local! {
    /// The line each label of the solid being drawn sits beside (label id, ends), recorded by `sbeside_r` between `begin` and
    /// `clear`; nothing is recorded for other figures.
    static OWN: std::cell::RefCell<Option<Vec<(String, Pt, Pt)>>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
thread_local! { static LAST_OWN: std::cell::RefCell<Vec<(String, Pt, Pt)>> = const { std::cell::RefCell::new(Vec::new()) }; }
/// The labels' own lines recorded for the last solid drawn on this thread (tests).
#[cfg(test)]
pub(super) fn last_own() -> Vec<(String, Pt, Pt)> { LAST_OWN.with(|l| l.borrow().clone()) }
/// Distance from a segment to a box (tests).
#[cfg(test)]
pub(super) fn seg_box_dist(a: Pt, b: Pt, bx: Bx) -> f64 { seg_box(a, b, bx) }
/// Starts recording the labels' own lines for one solid's drawing.
pub(super) fn begin() { OWN.with(|o| *o.borrow_mut() = Some(Vec::new())); }
/// Records that label `id` sits beside the line from `a` to `b` (only while a solid is being drawn).
pub(super) fn own_line(id: &str, a: Pt, b: Pt) {
    if id.is_empty() { return; }
    OWN.with(|o| if let Some(v) = o.borrow_mut().as_mut() { v.push((id.to_string(), a, b)); });
}
/// Distance from the segment a-b to a box (0 when it touches).
fn seg_box(a: Pt, b: Pt, bx: Bx) -> f64 {
    let n = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / 2.0).ceil().max(1.0) as usize;
    (0..=n).map(|i| {
        let t = i as f64 / n as f64;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        let dx = (bx.0 - x).max(0.0).max(x - bx.2);
        let dy = (bx.1 - y).max(0.0).max(y - bx.3);
        (dx * dx + dy * dy).sqrt()
    }).fold(f64::MAX, f64::min)
}
