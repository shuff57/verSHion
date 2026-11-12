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
        while let Some(Tok::Op(c @ ('+' | '-'))) = self.peek().cloned() {
            self.i += 1;
            // a doubled sign (2 - -3, 2 + -3) is refused: a slip, not a measure
            if matches!(self.peek(), Some(Tok::Op('+' | '-'))) { return None; }
            let r = self.term()?;
            v = if c == '+' { v + r } else { v - r };
        }
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
        // one leading sign: −3/2 (a lean to the left) or +3 (written 3); a doubled sign (--3, +-3) is refused
        if let Some(Tok::Op(c @ ('-' | '+'))) = self.peek().cloned() {
            self.i += 1;
            if matches!(self.peek(), Some(Tok::Op('+' | '-'))) { return None; }
            let v = self.power()?;
            return Some(if c == '-' { -v } else { v });
        }
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

/// A typed measure as written on the figure: √ for sqrt, π for pi, a true minus, no spaces or stars except around + and −, no
/// leading plus (3/2, 2√3, 5π, √(2+3)). A root divided by something is bracketed so the line reads one way only: √13/2 is
/// written (√13)/2 and 2√3/3 as (2√3)/3, while a root of a whole fraction keeps its own brackets, √(13/2).
pub(crate) fn pretty(text: &str) -> String {
    let Some(toks) = lex(text.trim()) else { return text.trim().to_string() };
    let mut out = String::new();
    // per bracket depth: where the current product began in `out`, and whether it holds a root
    let mut runs: Vec<(usize, bool)> = vec![(0, false)];
    let unary = |i: usize| i == 0 || matches!(toks[i - 1], Tok::Open | Tok::Op(_));
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Num(_) => {
                // the digits as typed (2.50 stays 2.50), but 3. is 3 and .5 is 0.5
                let n = num_text(text, i);
                let n = n.strip_suffix('.').unwrap_or(&n).to_string();
                out.push_str(&if n.starts_with('.') { format!("0{n}") } else { n });
            }
            Tok::Sqrt => {
                if let Some(r) = runs.last_mut() { r.1 = true; }
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
            Tok::Op(c @ ('+' | '-')) => {
                let sign = if *c == '+' { "+" } else { "\u{2212}" };
                if unary(i) { if *c == '-' { out.push_str(sign); } } else { out.push_str(&format!(" {sign} ")); }
                if let Some(r) = runs.last_mut() { *r = (out.len(), false); }
            }
            Tok::Op('/') => {
                if let Some(r) = runs.last_mut() && r.1 {
                    out.insert(r.0, '(');
                    out.push(')');
                    r.1 = false;
                }
                out.push('/');
            }
            Tok::Op(c) => out.push(*c),
            Tok::Open => { out.push('('); runs.push((out.len(), false)); }
            Tok::Close => { if runs.len() > 1 { runs.pop(); } out.push(')'); }
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
    let k = PAGE.with(|c| c.get());
    let m = Map::fit_at(all, 420.0 * k, 300.0 * k, 70.0 * k, 40.0 * k);
    let fs = lsz3(p);
    // as wide as `sbeside_r` sets it beside an edge (half an em a character), with its gap (5.4) and a little of its halo
    let widest = ["radius", "diam", "height", "lean", "length", "width", "side"].iter().filter_map(|k| exact(p, k)).map(|t| stw(&t, fs).max(serif_w(&t, fs) / 1.04)).fold(0.0, f64::max);
    if widest == 0.0 { return m; }
    let need = widest + 6.5;
    let (lo, hi) = all.iter().map(|q| m.p(*q).0).fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
    if lo >= need && 560.0 * k - hi >= need { return m; }
    Map::fit_at(all, 560.0 * k - 2.0 * need, 300.0 * k, need, 40.0 * k)
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

/// One drawn stroke as a segment: its ends, whether it is an outline (solid, 2 px or more) and whether it is a mark (under 1.5 px:
/// right-angle squares, not lines a label could be read against).
#[derive(Clone, Copy)]
struct Seg { a: Pt, b: Pt, hard: bool, mark: bool }

/// Every inked stroke of a drawn solid as segments (lines, and the edges of stroked polylines and polygons).
fn strokes(svg: &str) -> Vec<Seg> {
    let faces = faces(svg);
    let mut out = Vec::new();
    for tag in ["<line", "<polyline", "<polygon"] {
        for (i, _) in svg.match_indices(tag) {
            let h = svg[i + tag.len()..].split('>').next().unwrap_or("");
            if !h.starts_with(' ') || attr(h, "stroke") == "none" { continue; }
            let w = attr(h, "stroke-width").parse::<f64>().unwrap_or(1.0);
            if w <= 0.0 { continue; }
            let dashed = !matches!(attr(h, "stroke-dasharray"), "" | "none");
            let (hard, mark) = (!dashed && w >= 2.0, w < 1.5);
            let mut segs: Vec<(Pt, Pt)> = Vec::new();
            if tag == "<line" { segs.push(((num(h, "x1"), num(h, "y1")), (num(h, "x2"), num(h, "y2")))); } else {
                let pts: Vec<Pt> = attr(h, "points").split_whitespace().filter_map(|q| q.split_once(',')).filter_map(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))).collect();
                for w in pts.windows(2) { segs.push((w[0], w[1])); }
                if tag == "<polygon" && pts.len() > 2 { segs.push((pts[pts.len() - 1], pts[0])); }
            }
            for (a, b) in segs { for (a, b) in visible(&faces, i, a, b) { out.push(Seg { a, b, hard, mark }); } }
        }
    }
    out
}
/// The opaque faces of a drawing, in drawing order: (position in the SVG, outline). A stroke drawn before one is painted over
/// where it runs inside it.
fn faces(svg: &str) -> Vec<(usize, Vec<Pt>)> {
    svg.match_indices("<polygon").filter_map(|(i, _)| {
        let h = svg[i + 8..].split('>').next().unwrap_or("");
        if matches!(attr(h, "fill"), "" | "none") || attr(h, "fill-opacity").parse::<f64>().is_ok_and(|o| o < 1.0) { return None; }
        let pts: Vec<Pt> = attr(h, "points").split_whitespace().filter_map(|q| q.split_once(',')).filter_map(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))).collect();
        (pts.len() > 2).then_some((i, pts))
    }).collect()
}
fn inside(q: Pt, poly: &[Pt]) -> bool {
    let mut c = false;
    for k in 0..poly.len() {
        let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
        if (a.1 > q.1) != (b.1 > q.1) && q.0 < a.0 + (q.1 - a.1) * (b.0 - a.0) / (b.1 - a.1) { c = !c; }
    }
    c
}
/// Whether point q, drawn at position `at`, is painted over by a face drawn after it (a point on the face's own edge is not).
fn covered(faces: &[(usize, Vec<Pt>)], at: usize, q: Pt) -> bool {
    faces.iter().any(|(i, poly)| *i > at && inside(q, poly) && (0..poly.len()).all(|k| dist_pt_seg(q, poly[k], poly[(k + 1) % poly.len()]) > 1.0))
}
/// The parts of segment a-b, drawn at position `at`, that no later face paints over.
fn visible(faces: &[(usize, Vec<Pt>)], at: usize, a: Pt, b: Pt) -> Vec<(Pt, Pt)> {
    if !faces.iter().any(|(i, _)| *i > at) { return vec![(a, b)]; }
    let n = (((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()).ceil().max(1.0) as usize;
    let pt = |k: usize| { let t = k as f64 / n as f64; (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t) };
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for k in 0..=n {
        let seen = !covered(faces, at, pt(k));
        match (seen, start) {
            (true, None) => start = Some(k),
            (false, Some(s)) => { if k - 1 > s { out.push((pt(s), pt(k - 1))); } start = None; }
            _ => {}
        }
    }
    if let Some(s) = start && n > s { out.push((pt(s), pt(n))); }
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
/// Boxes closer than `gap` (or overlapping).
fn near(a: Bx, b: Bx, gap: f64) -> bool { !(a.2 + gap <= b.0 || b.2 + gap <= a.0 || a.3 + gap <= b.1 || b.3 + gap <= a.1) }
fn dist_pt_seg(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}
/// Whether stroke s lies along line a-b (collinear and overlapping it: a dashed line drawn on it, or the diameter through a radius).
pub(super) fn along_line(s: (Pt, Pt), a: Pt, b: Pt) -> bool {
    let line_d = |p: Pt| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = (dx * dx + dy * dy).sqrt().max(1e-9);
        ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / l
    };
    if line_d(s.0) > 1.5 || line_d(s.1) > 1.5 { return false; }
    dist_pt_seg(s.0, a, b) < 1.5 || dist_pt_seg(s.1, a, b) < 1.5 || dist_pt_seg(a, s.0, s.1) < 1.5 || dist_pt_seg(b, s.0, s.1) < 1.5
}
/// Where segment p1-p2 meets segment q1-q2 (within a pixel of their ends), if it does.
pub(super) fn cross_at(p1: Pt, p2: Pt, q1: Pt, q2: Pt) -> Option<Pt> {
    let (r, s) = ((p2.0 - p1.0, p2.1 - p1.1), (q2.0 - q1.0, q2.1 - q1.1));
    let den = r.0 * s.1 - r.1 * s.0;
    if den.abs() < 1e-12 { return None; }
    let t = ((q1.0 - p1.0) * s.1 - (q1.1 - p1.1) * s.0) / den;
    let u = ((q1.0 - p1.0) * r.1 - (q1.1 - p1.1) * r.0) / den;
    let (lr, ls) = ((r.0 * r.0 + r.1 * r.1).sqrt().max(1e-9), (s.0 * s.0 + s.1 * s.1).sqrt().max(1e-9));
    let (et, eu) = (1.0 / lr, 1.0 / ls);
    (t >= -et && t <= 1.0 + et && u >= -eu && u <= 1.0 + eu).then(|| (p1.0 + r.0 * t, p1.1 + r.1 * t))
}
fn segs_cross(p1: Pt, p2: Pt, q1: Pt, q2: Pt) -> bool {
    let c = |a: Pt, b: Pt, c: Pt| (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let (d1, d2, d3, d4) = (c(q1, q2, p1), c(q1, q2, p2), c(p1, p2, q1), c(p1, p2, q2));
    d1 * d2 < -1e-9 && d3 * d4 < -1e-9
}
/// The glyph box of one `<text>` element drawn at (x, y), as resvg lays it out with the app's fonts (cached by element).
fn ink_box(text_el: &str, x: f64, y: f64) -> Bx {
    thread_local! { static CACHE: std::cell::RefCell<std::collections::HashMap<String, Bx>> = std::cell::RefCell::new(Default::default()); }
    let key = text_el.replacen(&format!("x=\"{x:.2}\" y=\"{y:.2}\""), "x=\"1000\" y=\"1000\"", 1);
    let rel = CACHE.with(|c| c.borrow().get(&key).copied()).unwrap_or_else(|| {
        let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="2000" height="2000" viewBox="0 0 2000 2000">{key}</svg>"#);
        let mut opt = resvg::usvg::Options::default();
        opt.fontdb = crate::figures::fontdb();
        let mut b = (-10.0, -10.0, 10.0, 10.0);
        if let Ok(tree) = resvg::usvg::Tree::from_str(&crate::figures::serif_stack(&svg), &opt) {
            fn walk(g: &resvg::usvg::Group, out: &mut Option<Bx>) {
                for n in g.children() {
                    match n {
                        resvg::usvg::Node::Group(g) => walk(g, out),
                        resvg::usvg::Node::Text(t) => {
                            let r = t.flattened().bounding_box().transform(t.abs_transform()).unwrap_or(t.flattened().abs_bounding_box());
                            *out = Some((r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64));
                        }
                        _ => {}
                    }
                }
            }
            let mut o = None;
            walk(tree.root(), &mut o);
            if let Some(v) = o { b = (v.0 - 1000.0, v.1 - 1000.0, v.2 - 1000.0, v.3 - 1000.0); }
        }
        CACHE.with(|c| c.borrow_mut().insert(key, b));
        b
    });
    (rel.0 + x, rel.1 + y, rel.2 + x, rel.3 + y)
}
/// The point of box `b` nearest `q`, pulled 1.5 px out from the box.
fn box_point(b: Bx, q: Pt) -> Pt {
    let x = q.0.clamp(b.0, b.2);
    let y = q.1.clamp(b.1, b.3);
    let (dx, dy) = (q.0 - x, q.1 - y);
    let l = (dx * dx + dy * dy).sqrt();
    if l < 1e-9 { (x, y) } else { (x + dx / l * 1.5, y + dy / l * 1.5) }
}

/// Moves each label of a figure with typed measures, in drawing order, so that it crosses no line, keeps a clear gap from every
/// label already placed, stays on the page and is read as its own line's measure: nearer its own line than any other drawn line
/// (outlines and dashed edges included) and close to it. It tries, in turn, where it was put; beside its own line, slid along it,
/// on either side and a little further out; just past either end of that line; any nearby spot that still belongs to it; and
/// last a spot further off with a thin leader drawn from its own line to the text, crossing nothing. It is never put where it
/// would read as another line's measure: with no such spot it stays where it was drawn, by its own line. Corner letters (no line
/// of their own) only keep clear. A figure with no typed measure is returned unchanged.
pub(super) fn clear(svg: String, p: &Params) -> String {
    let own = OWN.with(|o| o.borrow_mut().take()).unwrap_or_default();
    #[cfg(test)]
    LAST_OWN.with(|l| *l.borrow_mut() = own.clone());
    if !any_typed(p) { return svg; }
    let mut lines = strokes(&svg);
    let face_list = faces(&svg);
    let (pw, ph) = crate::figures::svg_size(&svg);
    // the labels (and their leaders) are drawn last, over every face, so nothing drawn after them covers one
    let mut on_top = String::new();
    let mut strain = 0.0;
    let mut placed: Vec<Bx> = Vec::new();
    // how far each placed label is from its own line
    let mut placed_own: Vec<f64> = Vec::new();
    let mut leaders: Vec<(Pt, Pt)> = Vec::new();
    // the boxes of the labels not yet placed (where they were drawn), so a moved label does not land on one
    let mut pending: Vec<(usize, Bx)> = Vec::new();
    {
        let mut rest = svg.as_str();
        let mut off = 0;
        while let Some(i) = rest.find("<g data-lbl=\"") {
            let tail = &rest[i..];
            let Some(end) = tail.find("</g>") else { break };
            let grp = &tail[..end + 4];
            if let Some(ti) = grp.find("<text") {
                let th = grp[ti..].split('>').next().unwrap_or("");
                let (x, y) = (num(th, "x"), num(th, "y"));
                if x.is_finite() && y.is_finite() { pending.push((off + i, ink_box(&grp[ti..grp.rfind("</text>").map(|e| e + 7).unwrap_or(grp.len())], x, y))); }
            }
            off += i + end + 4;
            rest = &tail[end + 4..];
        }
    }
    let mut out = String::with_capacity(svg.len() + 512);
    let mut rest = svg.as_str();
    let mut off = 0;
    while let Some(i) = rest.find("<g data-lbl=\"") {
        out.push_str(&rest[..i]);
        let here = off + i;
        let tail = &rest[i..];
        let Some(end) = tail.find("</g>") else { break };
        let grp = &tail[..end + 4];
        off += i + end + 4;
        rest = &tail[end + 4..];
        let ghead = grp.split('>').next().unwrap_or("");
        let Some(ti) = grp.find("<text") else { on_top.push_str(grp); continue };
        let thead = grp[ti..].split('>').next().unwrap_or("");
        let (x, y, sz) = (num(thead, "x"), num(thead, "y"), num(thead, "font-size"));
        let later: Vec<Bx> = pending.iter().filter(|(at, _)| *at > here).map(|(_, b)| *b).collect();
        // a dragged label stays where the teacher put it
        if ghead.contains("transform=") || !(x.is_finite() && y.is_finite() && sz.is_finite()) { on_top.push_str(grp); continue; }
        // the label's glyph box as drawn (resvg with the app's fonts), 1 px round for its halo
        let ib = ink_box(&grp[ti..grp.rfind("</text>").map(|e| e + 7).unwrap_or(grp.len())], x, y);
        let w = (ib.2 - ib.0) / 2.0;
        let (cx0, cy0) = ((ib.0 + ib.2) / 2.0, (ib.1 + ib.3) / 2.0);
        let hh = (ib.3 - ib.1) / 2.0;
        let bx = |dx: f64, dy: f64| (cx0 + dx - w - 1.0, cy0 + dy - hh - 1.0, cx0 + dx + w + 1.0, cy0 + dy + hh + 1.0);
        let id = attr(ghead, "data-lbl").to_string();
        // its own line where it shows (a face drawn after it may paint part of it over: the top of an oblique cylinder's height)
        let mine_all: Vec<(Pt, Pt)> = own.iter().filter(|o| o.0 == id).map(|o| (o.1, o.2)).collect();
        let mine_seen: Vec<(Pt, Pt)> = mine_all.iter().flat_map(|(a, c)| visible(&face_list, here, *a, *c)).filter(|(a, c)| (c.0 - a.0).abs() + (c.1 - a.1).abs() > 4.0).collect();
        let mine: Vec<(Pt, Pt)> = if mine_seen.is_empty() { mine_all } else { mine_seen };
        let on_page = |b: Bx| b.0 >= 1.0 && b.1 >= 1.0 && b.2 <= pw - 1.0 && b.3 <= ph - 1.0;
        let free = |b: Bx, moved: bool| !placed.iter().any(|q| near(*q, b, 4.0)) && !(moved && later.iter().any(|q| near(*q, b, 4.0))) && !leaders.iter().any(|(a, c)| seg_hits(*a, *c, b));
        // clear of every line and of the right-angle squares
        let ok = |b: Bx, moved: bool| on_page(b) && free(b, moved) && !lines.iter().any(|s| seg_box(s.a, s.b, b) < if s.mark { 1.5 } else { 0.5 });
        // the strokes a label is read against: not marks, not on its own line
        let others: Vec<Seg> = lines.iter().copied().filter(|s| !s.mark && !mine.iter().any(|(a, c)| along_line((s.a, s.b), *a, *c))).collect();
        let d_own = |b: Bx| mine.iter().map(|(a, c)| seg_box(*a, *c, b)).fold(f64::MAX, f64::min);
        let belongs = |b: Bx| { let d = d_own(b); d <= 16.0 && others.iter().all(|s| seg_box(s.a, s.b, b) >= d + 3.0) };
        let good = |b: Bx, moved: bool| ok(b, moved) && (mine.is_empty() || belongs(b));
        let mut best: Option<(f64, f64)> = None;
        let mut leader: Option<(Pt, Pt)> = None;
        if good(bx(0.0, 0.0), false) {
            best = Some((0.0, 0.0));
        } else if let Some(&(a, c)) = mine.first() {
            let len = ((c.0 - a.0).powi(2) + (c.1 - a.1).powi(2)).sqrt().max(1e-9);
            let (u, nrm) = (((c.0 - a.0) / len, (c.1 - a.1) / len), (-(c.1 - a.1) / len, (c.0 - a.0) / len));
            let (mx, my) = (cx0, cy0);
            let t0 = (((mx - a.0) * u.0 + (my - a.1) * u.1) / len).clamp(0.0, 1.0);
            let side0 = if (mx - a.0) * nrm.0 + (my - a.1) * nrm.1 >= 0.0 { 1.0 } else { -1.0 };
            let beside = |ts: &[f64]| {
                for gap in [5.4, 8.0, 12.0] {
                    for side in [side0, -side0] {
                        for &t in ts {
                            let n = (nrm.0 * side, nrm.1 * side);
                            let o = (w + 1.0) * n.0.abs() + (hh + 1.0) * n.1.abs() + gap;
                            let (cx, cy) = (a.0 + (c.0 - a.0) * t + n.0 * o, a.1 + (c.1 - a.1) * t + n.1 * o);
                            let (dx, dy) = (cx - mx, cy - my);
                            if good(bx(dx, dy), true) { return Some((dx, dy)); }
                        }
                    }
                }
                None
            };
            let by = |lo: i32, hi: i32| { let mut ts: Vec<f64> = (lo..=hi).map(|k| k as f64 / 20.0).collect(); ts.sort_by(|p, q| (p - t0).abs().partial_cmp(&(q - t0).abs()).unwrap()); ts };
            // past either end: the label's half length beyond the line's end, along it
            // (only past an end where no other line meets it: past a corner the label would read as the corner's other lines')
            let free_end = |e: Pt| !others.iter().any(|s| dist_pt_seg(e, s.a, s.b) < 3.0);
            let (end_a, end_c) = (free_end(a), free_end(c));
            let past: Vec<f64> = { let e = (w + 8.0) / len; let mut v: Vec<f64> = (0..=8).flat_map(|k| { let d = e * (0.5 + k as f64 * 0.25); [-d, 1.0 + d] }).filter(|t| if *t < 0.0 { end_a } else { end_c }).collect(); v.sort_by(|p, q| (p - t0).abs().partial_cmp(&(q - t0).abs()).unwrap()); v };
            best = beside(&by(0, 20)).or_else(|| beside(&past)).or_else(|| {
                for r in (1..=14).map(|k| k as f64 * 3.0) {
                    for k in 0..48 {
                        let th = k as f64 * std::f64::consts::TAU / 48.0;
                        if good(bx(r * th.cos(), r * th.sin()), true) { return Some((r * th.cos(), r * th.sin())); }
                    }
                }
                None
            });
            if best.is_none() {
                // a leader from its own line: the text further off, clear of every line by 6 px so the leader is the line nearest
                // it, the leader crossing no line and no label
                let foot = (a.0 + (c.0 - a.0) * t0, a.1 + (c.1 - a.1) * t0);
                // crossing no outline if it can; failing that as few as it must (a line inside a crowded solid: the label is then
                // out of the crowd, its leader still drawn from its own line)
                // and clear of the labels not placed yet; only if nothing else is found, through one (it is moved off it later)
                // (and last, passing a placed label a little closer, still further than that label is from its own line)
                // a short leader (40 px at most) wherever one is found, before any longer one
                let tiers: Vec<(usize, bool, f64, f64)> = [40.0, 1e9].iter().flat_map(|&cap| [0usize, 1, 2, 3].map(|m| (m, true, 3.0, cap)).into_iter().chain([0usize, 1, 2, 3].map(|m| (m, false, 3.0, cap))).chain([0usize, 1, 2, 3].map(|m| (m, false, 1.2, cap)))).collect();
                'lead: for (max_cross, spare_later, margin, cap) in tiers {
                    for r in (6..=60).map(|k| k as f64 * 3.0) {
                        for k in 0..64 {
                            let th = k as f64 * std::f64::consts::TAU / 64.0;
                            let (cx, cy) = (foot.0 + r * th.cos(), foot.1 + r * th.sin());
                            let (dx, dy) = (cx - mx, cy - my);
                            let b = bx(dx, dy);
                            if !ok(b, true) || lines.iter().any(|s| !s.mark && seg_box(s.a, s.b, b) < 5.0) { continue; }
                            // from the nearest point of the inner part of its own line (not a corner, where other lines meet)
                            let q = (6..=34).map(|j| { let t = j as f64 / 40.0; (a.0 + (c.0 - a.0) * t, a.1 + (c.1 - a.1) * t) })
                                .min_by(|p, q| { let d = |z: Pt| seg_box(z, z, b); d(*p).partial_cmp(&d(*q)).unwrap() }).unwrap();
                            let e = box_point(b, q);
                            // it may cross a dashed hidden edge (as a drafting leader does)
                            let outlines = lines.iter().filter(|s| s.hard && !mine.iter().any(|(a2, c2)| along_line((s.a, s.b), *a2, *c2)) && segs_cross(q, e, s.a, s.b)).count();
                            // and keeps 6 px from every other label, so no label is read against it
                            // (a placed label: further than it is from its own line, so it is not read against the leader instead)
                            let crosses_label = placed.iter().zip(&placed_own).any(|(pb, d)| seg_box(q, e, *pb) < (d + margin).max(2.0 * margin))
                                || (spare_later && later.iter().any(|pb| seg_box(q, e, *pb) < 4.0));
                            let crosses_leader = leaders.iter().any(|(l1, l2)| segs_cross(q, e, *l1, *l2));
                            // and does not run along an outline or another leader: 3 px clear of it beyond its first 6 px
                            let ll = ((e.0 - q.0).powi(2) + (e.1 - q.1).powi(2)).sqrt().max(1e-9);
                            let hugs = (0..=20).map(|j| { let t = 6.0 / ll + (1.0 - 6.0 / ll) * j as f64 / 20.0; (q.0 + (e.0 - q.0) * t, q.1 + (e.1 - q.1) * t) })
                                .any(|z| others.iter().filter(|s| s.hard || leaders.iter().any(|l| (l.0, l.1) == (s.a, s.b))).any(|s| dist_pt_seg(z, s.a, s.b) < 3.0 && cross_at(q, e, s.a, s.b).is_none_or(|x| ((z.0 - x.0).powi(2) + (z.1 - x.1).powi(2)).sqrt() > 4.0)));
                            let crosses_leader = crosses_leader || ll < 8.0 || ll > cap || hugs;
                            if outlines <= max_cross && !crosses_label && !crosses_leader {
                                strain += (ll - 40.0).max(0.0) + 100.0 * outlines as f64;
                                best = Some((dx, dy));
                                leader = Some((q, e));
                                break 'lead;
                            }
                        }
                    }
                }
            }
        } else {
            // a corner letter: only kept clear
            'spiral: for r in (1..=20).map(|k| k as f64 * 3.0) {
                for k in 0..48 {
                    let th = k as f64 * std::f64::consts::TAU / 48.0;
                    if ok(bx(r * th.cos(), r * th.sin()), true) { best = Some((r * th.cos(), r * th.sin())); break 'spiral; }
                }
            }
        }
        // no spot found: it stays where it was drawn, beside its own line
        if best.is_none() { strain += 1000.0; }
        let best = best.unwrap_or((0.0, 0.0));
        placed.push(bx(best.0, best.1));
        placed_own.push(if leader.is_some() { 1.5 } else if mine.is_empty() { 0.0 } else { d_own(bx(best.0, best.1)) });
        if let Some((q, e)) = leader {
            on_top.push_str(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#111827" stroke-width="1.6" data-leader="{id}"/>"##, q.0, q.1, e.0, e.1));
            leaders.push((q, e));
            lines.push(Seg { a: q, b: e, hard: false, mark: false });
        }
        if best == (0.0, 0.0) { on_top.push_str(grp); continue; }
        let (cx, cy) = { let v: Vec<f64> = attr(ghead, "data-box").split_whitespace().filter_map(|v| v.parse().ok()).collect(); (v.first().copied().unwrap_or(x), v.get(1).copied().unwrap_or(y)) };
        let g2 = grp
            .replacen(&format!("data-box=\"{cx:.2} {cy:.2}"), &format!("data-box=\"{:.2} {:.2}", cx + best.0, cy + best.1), 1)
            .replacen(&format!("<text x=\"{x:.2}\" y=\"{y:.2}\""), &format!("<text x=\"{:.2}\" y=\"{:.2}\"", x + best.0, y + best.1), 1);
        on_top.push_str(&g2);
    }
    // the rest of the drawing, then the labels on top, before the document's end
    match rest.rfind("</svg>") {
        Some(e) => { out.push_str(&rest[..e]); out.push_str(&on_top); out.push_str(&rest[e..]); }
        None => { out.push_str(rest); out.push_str(&on_top); }
    }
    STRAIN.with(|s| s.set(strain));
    out
}

thread_local! {
    /// How hard the last typed solid's labels were to place: leader length over 40 px, outlines its leaders cross (100 each),
    /// labels left where drawn (1000 each). `roomy` draws a figure larger when this is not 0.
    static STRAIN: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    /// The page scale of the solid being drawn (1: the 560 by 380 page; more when its labels need room).
    static PAGE: std::cell::Cell<f64> = const { std::cell::Cell::new(1.0) };
}
/// The page of the solid being drawn (560 by 380, grown by `roomy`).
pub(super) fn page() -> (f64, f64) { let k = PAGE.with(|c| c.get()); (560.0 * k, 380.0 * k) }
/// Draws a solid; a figure with a typed measure whose labels needed long leaders, leaders across outlines, or could not be
/// placed is drawn again on a larger page (1.25 to 2 times), where its lines are longer and its labels relatively smaller,
/// and the drawing with the least of that is kept (the smaller page on a tie). A figure with no typed measure is drawn once, on
/// its usual page, exactly as before.
pub(super) fn roomy(p: &Params, f: fn(&Params) -> String) -> String {
    PAGE.with(|c| c.set(1.0));
    let first = f(p);
    let s0 = STRAIN.with(|s| s.get());
    if !any_typed(p) || s0 == 0.0 { return first; }
    let (mut best, mut best_k, mut best_s) = (first, 1.0, s0);
    for k in [1.25, 1.5, 1.75, 2.0] {
        PAGE.with(|c| c.set(k));
        let svg = f(p);
        let s = STRAIN.with(|s| s.get());
        if s < best_s - 1e-9 { best = svg; best_k = k; best_s = s; }
        if best_s == 0.0 { break; }
    }
    // the kept drawing's map is the last one made (handles read it)
    if PAGE.with(|c| c.get()) != best_k { PAGE.with(|c| c.set(best_k)); best = f(p); }
    PAGE.with(|c| c.set(1.0));
    best
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
