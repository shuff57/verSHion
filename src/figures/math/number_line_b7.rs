//! Number line, batch 7: a graph row may be a sequence (aₙ = 1/n, its terms for n from one whole
//! number to another drawn as points), each graph row can write its values above the line, points
//! and terms are dots or crosses, lettered points can show their value (P(3/4)), and the range can
//! be typed in multiples of π with the ticks then numbered in π (π/2, π, 3π/2 ...).
//!
//! Every new control's default draws what the figure drew before, and an old figure goes through the
//! same drawing statements in the same order, so old saved figures and share links draw byte for byte
//! as before (`number_line_frozen.rs`). The new labels above the line are placed by `plan`: each sits
//! over its own mark, lifted clear of the others, and dropped (with a panel note) when it cannot be.
use super::*;

/// More ticks than this (a tiny Count by over a long range) are refused with a note.
pub(super) const MAX_TICKS: i64 = 40_001;
/// At most this many terms of a sequence (the n boxes run 0 to 1000).
const N_MAX: i64 = 1000;

// ───────────────────────── params ─────────────────────────

/// The batch 7 controls, appended to the number line's own list (old keys first, unchanged).
pub(super) fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("min_text", "From, typed", "")
            .hint("For example \u{2212}2pi, \u{2212}3/2 or 0. Overrides From; pi is \u{3c0}."),
        ParamDef::text("max_text", "To, typed", "")
            .hint("For example 2pi. Overrides To."),
        ParamDef::choice("ineq_values", "First graph: values above the line", &VALUES, "None")
            .hint("Write puts each end's value, or each term's, above it when it falls between numbered ticks."),
        ParamDef::choice("ineq_mark", "First graph: a sequence's terms as", &MARKS, "Dot"),
        ParamDef::int("n_from", "First graph: first n", 0, N_MAX, 1)
            .hint("For a sequence such as a_n = 1/n: the terms from this n ..."),
        ParamDef::int("n_to", "First graph: last n", 0, N_MAX, 5)
            .hint("... to this one."),
        ParamDef::choice("ineq2_values", "Second graph: values above the line", &VALUES, "None"),
        ParamDef::choice("ineq2_mark", "Second graph: a sequence's terms as", &MARKS, "Dot"),
        ParamDef::int("n2_from", "Second graph: first n", 0, N_MAX, 1),
        ParamDef::int("n2_to", "Second graph: last n", 0, N_MAX, 5),
        ParamDef::choice("pt_mark", "Points drawn as", &MARKS, "Dot")
            .hint("A point typed with an o after it (5o) stays an open dot."),
        ParamDef::choice("pt_labels", "Lettered points labelled", &LABELS, LABELS[0]),
    ]
}
pub(super) const VALUES: [&str; 2] = ["None", "Write"];
pub(super) const MARKS: [&str; 2] = ["Dot", "Cross"];
pub(super) const LABELS: [&str; 2] = ["Letter, like P", "Letter and value, like P(3/4)"];

/// The rows' keys: (text, colour, default colour, values, mark, first n, last n).
const ROWS: [(&str, &str, &str, &str, &str, &str, &str); 2] = [
    ("inequality", "ineq_color", BLUE, "ineq_values", "ineq_mark", "n_from", "n_to"),
    ("inequality2", "ineq2_color", "#c0392b", "ineq2_values", "ineq2_mark", "n2_from", "n2_to"),
];

/// Controls shown only where they act: a row's sequence controls only for a sequence row, its
/// values switch only for a typed row, the point controls only with points (lettered, for the labels).
pub(crate) fn relevant(key: &str, p: &Params) -> bool {
    for (i, r) in ROWS.iter().enumerate() {
        if key == r.3 { return !p.s(r.0).trim().is_empty(); }
        if key == r.4 || key == r.5 || key == r.6 { return matches!(read_row(p, i), Row::Seq(_) | Row::Bad(_)); }
    }
    match key {
        "pt_mark" => !p.s("points").trim().is_empty(),
        "pt_labels" => points(p).iter().any(|q| !q.name.is_empty()),
        _ => true,
    }
}

// ───────────────────────── reading ─────────────────────────

/// A typed From / To: a number, a fraction or a multiple of π, kept within the sliders' range.
fn typed_end(p: &Params, text_key: &str, key: &str) -> f64 {
    let t = p.s(text_key).trim();
    if t.is_empty() { return p.f(key); }
    nl_num(t).filter(|v| v.is_finite()).map(|v| v.clamp(-1000.0, 1000.0)).unwrap_or_else(|| p.f(key))
}
/// The range drawn: From and To, a typed From / To winning over the sliders.
pub(crate) fn range(p: &Params) -> (f64, f64) {
    let (lo, mut hi) = (typed_end(p, "min_text", "min"), typed_end(p, "max_text", "max"));
    if hi <= lo { hi = lo + 1.0; }
    (lo, hi)
}
/// Text that names π (pi, π).
pub(super) fn has_pi(t: &str) -> bool { let l = t.to_lowercase(); l.contains("pi") || l.contains('\u{3c0}') }
/// `q` as a fraction a/b in lowest terms with b at most `max_den`.
fn ratio(q: f64, max_den: i64) -> Option<(i64, i64)> {
    if !q.is_finite() { return None; }
    for b in 1..=max_den {
        let a = (q * b as f64).round();
        if a.abs() <= 1e7 && (q * b as f64 - a).abs() < 1e-7 { return Some((a as i64, b)); }
    }
    None
}
/// Count by typed as a multiple of π (pi/2, 2pi/3, π): (a, b) with the step aπ/b in lowest terms.
pub(super) fn pi_step(p: &Params) -> Option<(i64, i64)> {
    let st = p.s("step_text").trim();
    if !has_pi(st) { return None; }
    let v = nl_num(st).filter(|v| *v > 1e-9)?;
    ratio(v / PI, 24).filter(|(a, _)| *a > 0)
}

/// What a graph row draws.
#[derive(Clone)]
pub(super) enum Row {
    /// An inequality or equation (what every old row was read as; possibly no intervals).
    Ineq(Vec<Interval>),
    /// A sequence such as aₙ = 1/n.
    Seq(Seq),
    /// A sequence the figure cannot read, with what to type instead.
    Bad(String),
}
#[derive(Clone)]
pub(super) struct Seq {
    pub letter: String,
    /// (n, aₙ) for each defined term with n in the row's range, in order of n.
    pub terms: Vec<(i64, f64)>,
    /// The n at which the formula has no value (divides by zero, a root of a negative ...).
    pub undefined: Vec<i64>,
}

/// "a_n = 1/n", "aₙ = 1/n", "a(n) = 1/n", "b_{n} = n^2" -> (letter, formula).
fn seq_form(text: &str) -> Option<(String, String)> {
    let t = text.trim().replace('\u{2099}', "_n").replace('\u{2212}', "-");
    let (lhs, rhs) = t.split_once('=')?;
    let l: String = lhs.chars().filter(|c| !c.is_whitespace()).collect();
    let letter = l.strip_suffix("_n").or_else(|| l.strip_suffix("_{n}")).or_else(|| l.strip_suffix("(n)"))?;
    let mut cs = letter.chars();
    match (cs.next(), cs.next()) { (Some(c), None) if c.is_alphabetic() => Some((c.to_string(), rhs.trim().to_string())), _ => None }
}
/// A sequence's formula in n made into the evaluator's x; Err when it uses another letter.
fn formula(rhs: &str) -> Result<String, ()> {
    let cs: Vec<char> = rhs.chars().collect();
    let (mut out, mut i) = (String::new(), 0);
    while i < cs.len() {
        if cs[i].is_ascii_alphabetic() {
            let st = i;
            while i < cs.len() && cs[i].is_ascii_alphabetic() { i += 1; }
            let w: String = cs[st..i].iter().collect::<String>().to_lowercase();
            match w.as_str() {
                "n" => out.push('x'),
                // a letter other than n (x, k, y) is not this sequence's variable
                "x" | "y" => return Err(()),
                _ => out.push_str(&w),
            }
        } else { out.push(cs[i]); i += 1; }
    }
    if rhs.contains(['<', '>', '=']) { return Err(()); }
    Ok(out)
}
/// Row `i` (0 or 1) as the figure reads it. A row the old figure drew as an inequality is read
/// exactly as before; only a row it drew nothing for (no intervals) can be a sequence.
pub(super) fn read_row(p: &Params, i: usize) -> Row {
    let r = ROWS[i];
    let text = p.s(r.0);
    let ivs = nl_intervals(text);
    if !ivs.is_empty() { return Row::Ineq(ivs); }
    let Some((letter, rhs)) = seq_form(text) else { return Row::Ineq(ivs) };
    let help = format!("Write the sequence in n with numbers, like {letter}\u{2099} = 1/n, {letter}\u{2099} = n^2 \u{2212} 3 or {letter}\u{2099} = (\u{2212}1)^n/n.");
    let Ok(f) = formula(&rhs) else { return Row::Bad(help) };
    if f.trim().is_empty() { return Row::Bad(help); }
    let (a, b) = (p.i(r.5).clamp(0, N_MAX), p.i(r.6).clamp(0, N_MAX));
    let (from, to) = (a.min(b), a.max(b));
    // the formula must read at all: try a few n (one may be outside its domain, as 1/n at 0)
    if [1.5, 2.5, 7.25].iter().all(|x| eval(&f, *x).is_none()) && (from..=to).all(|n| eval(&f, n as f64).is_none()) { return Row::Bad(help); }
    let (mut terms, mut undefined) = (vec![], vec![]);
    for n in from..=to {
        match eval(&f, n as f64) { Some(v) if v.is_finite() => terms.push((n, v)), _ => undefined.push(n) }
    }
    Row::Seq(Seq { letter, terms, undefined })
}

/// A typed point: value, open or not, letter, and the number as typed.
#[derive(Clone, Debug)]
pub(super) struct Point { pub v: f64, pub open: bool, pub name: String, pub typed: String }
/// The points list as the figure has always read it (in order; unreadable items left out).
pub(super) fn points(p: &Params) -> Vec<Point> {
    let mut out = vec![];
    for t in p.s("points").split(',') {
        let t = t.trim();
        if t.is_empty() { continue; }
        let (t, open) = match t.strip_suffix('o') { Some(s) if !s.ends_with(|c: char| c.is_alphabetic()) => (s, true), _ => (t, false) };
        let (name, num) = match (t.find('('), t.rfind(')')) {
            (Some(a), Some(b)) if b > a && a > 0 => (t[..a].trim(), &t[a + 1..b]),
            _ => ("", t),
        };
        let Some(v) = nl_num(num) else { continue };
        out.push(Point { v, open, name: name.to_string(), typed: num.trim().to_string() });
    }
    out
}

// ───────────────────────── text and its size as drawn ─────────────────────────

/// Width of `s` at `size` in the sans face the labels are drawn in (DejaVu Sans advance widths).
pub(super) fn sans_w(s: &str, size: f64, bold: bool) -> f64 {
    let k = if bold { 1.1 } else { 1.0 };
    s.chars().map(|c| match c {
        '0'..='9' => if bold { 0.70 } else { 0.64 },
        '.' | ',' | ' ' => 0.32 * k,
        '\u{2212}' | '+' | '=' => 0.84,
        '-' => 0.36 * k,
        '(' | ')' => 0.39 * k,
        '/' => 0.34 * k,
        '\u{3c0}' => 0.62 * k,
        c if c.is_uppercase() => 0.72 * k,
        _ => 0.6 * k,
    }).sum::<f64>() * size
}
/// Width of a letter set in the serif italic bold the figure names its points in.
fn serif_w(s: &str, size: f64) -> f64 { s.chars().count() as f64 * size * 0.7 }

/// A stacked fraction: sign, whole part (mixed numbers), numerator over denominator.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Stack { pub neg: bool, pub whole: String, pub num: String, pub den: String }
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Txt { Plain(String), Stack(Stack), Named(String, String) }
impl Txt {
    /// Width and height as drawn at `size` (Named: the letter at `size`, the value at 14/17 of it).
    fn dims(&self, size: f64, bold: bool) -> (f64, f64) {
        match self {
            Txt::Plain(s) => (sans_w(s, size, bold), 0.96 * size),
            Txt::Stack(st) => (stack_w(st, size, bold).2, 3.0 + 1.57 * size),
            Txt::Named(n, rest) => (serif_w(n, size) + sans_w(rest, size * 14.0 / 17.0, true), 0.96 * size),
        }
    }
    /// The plain text a reader sees (numerator/denominator, π), for notes and tests.
    pub(crate) fn plain(&self) -> String {
        match self {
            Txt::Plain(s) => s.clone(),
            Txt::Stack(st) => format!("{}{}{}{}/{}", if st.neg { "\u{2212}" } else { "" }, st.whole, if st.whole.is_empty() { "" } else { " " }, st.num, st.den),
            Txt::Named(n, r) => format!("{n}{r}"),
        }
    }
}
/// (sign and whole part width, fraction width, total) of a stacked fraction.
fn stack_w(st: &Stack, size: f64, bold: bool) -> (f64, f64, f64) {
    let fw = sans_w(&st.num, size * 0.8, bold).max(sans_w(&st.den, size * 0.8, bold)).max(size * 0.6);
    // the minus's ink ends at 0.74 of its advance: the fraction starts there, not after the side bearing
    let a = sans_w(&st.whole, size, bold) + if st.neg { 0.74 * size } else { 0.0 };
    (a, fw, a + fw + 2.0)
}
/// Draws a stacked fraction centred at x with its bar just under y (the old fraction labels' layout).
fn draw_stack(d: &mut Doc, x: f64, y: f64, st: &Stack, size: f64, bold: bool) {
    let (a, fw, total) = stack_w(st, size, bold);
    let ws = format!("{}{}", if st.neg { "\u{2212}" } else { "" }, st.whole);
    let left = x - total / 2.0;
    if !ws.is_empty() { txtw(d, left, y + size * 0.34, size, "start", &ws, bold); }
    let cx = left + a + fw / 2.0 + 1.0;
    txtw(d, cx, y - 2.0, size * 0.8, "middle", &st.num, bold);
    d.line(cx - fw / 2.0, y + 1.0, cx + fw / 2.0, y + 1.0, INK, 1.0);
    txtw(d, cx, y + size * 0.8 + 1.0, size * 0.8, "middle", &st.den, bold);
}
/// Top of a stacked fraction's box above its `y`, and bottom below it.
fn stack_extent(size: f64) -> (f64, f64) { (2.0 + 0.74 * 0.8 * size, 1.0 + 0.8 * size + 0.22 * 0.8 * size) }

/// k/b times π in lowest terms: 0, π, −2π, π/2, −3π/2.
pub(super) fn pi_txt(k: i64, b: i64) -> Txt {
    let g = { let (mut x, mut y) = (k.abs(), b.abs()); while y != 0 { let t = x % y; x = y; y = t; } x.max(1) };
    let (n, d) = (k / g, b / g);
    let top = if n.abs() == 1 { "\u{3c0}".to_string() } else { format!("{}\u{3c0}", n.abs()) };
    if n == 0 { return Txt::Plain("0".into()); }
    if d == 1 { return Txt::Plain(format!("{}{top}", if n < 0 { "\u{2212}" } else { "" })); }
    Txt::Stack(Stack { neg: n < 0, whole: String::new(), num: top, den: d.to_string() })
}
/// n/d in lowest terms (mixed: with its whole part), or a whole number.
fn frac_txt(n: i64, d: i64, mixed: bool) -> Txt {
    let g = { let (mut x, mut y) = (n.abs(), d.abs()); while y != 0 { let t = x % y; x = y; y = t; } x.max(1) };
    let (n, d) = (n / g, d / g);
    if d == 1 { return Txt::Plain(fmt(n as f64, 0)); }
    let (whole, rem) = if mixed { (n.abs() / d, n.abs() % d) } else { (0, n.abs()) };
    Txt::Stack(Stack { neg: n < 0, whole: if whole > 0 { whole.to_string() } else { String::new() }, num: rem.to_string(), den: d.to_string() })
}
/// A value written above the line, in the line's own terms: in π when the ticks are, as a fraction
/// of the Count by fraction, else as a decimal (exact to 3 places) or a fraction (`prefer_frac`
/// writes a sequence's 1/3 and 1/4 as fractions).
fn value_txt(v: f64, pl: &Plan, prefer_frac: bool) -> Txt {
    if pl.pi.is_some() && let Some((a, b)) = ratio(v / PI, 24) { return pi_txt(a, b); }
    if let Some(dd) = pl.den { let x = v * dd as f64; if (x - x.round()).abs() < 1e-6 { return frac_txt(x.round() as i64, dd, pl.mixed); } }
    let exact3 = ((v * 1000.0) - (v * 1000.0).round()).abs() < 1e-7;
    if !prefer_frac && exact3 { return Txt::Plain(fmt(v, 3)); }
    if v.abs() < 1e6 && let Some((a, b)) = ratio(v, 64) { return frac_txt(a, b, pl.mixed); }
    Txt::Plain(fmt(v, 3))
}

// ───────────────────────── layout ─────────────────────────

#[derive(Clone, Copy, Debug)]
pub(super) struct Bx { pub l: f64, pub t: f64, pub r: f64, pub b: f64 }
impl Bx {
    fn hit(&self, o: &Bx, pad: f64) -> bool { self.l < o.r + pad && o.l < self.r + pad && self.t < o.b + pad && o.t < self.b + pad }
}
/// A new label above the line, placed: its text, centre x, box (y relative to the axis), its mark.
#[derive(Clone, Debug)]
pub(super) struct Placed { pub txt: Txt, pub size: f64, pub bx: Bx, pub mark: (f64, f64), pub row: usize }

/// Everything the drawing needs that is not a plain param: the range and step, how the rows read,
/// the second row's height above the line, the new labels' places and the notes. Pure; shared by
/// the drawing, the canvas size (`nl_geom`) and the handles.
pub(super) struct Plan {
    pub lo: f64, pub hi: f64, pub step: f64, pub den: Option<i64>, pub pi: Option<(i64, i64)>, pub mixed: bool,
    pub k0: i64, pub k1: i64, pub every: i64, pub too_many: bool,
    /// π numbers at the ends that would run off the page (left off; the far ends of a long line).
    pub pi_off: Vec<i64>,
    pub rows: [Row; 2],
    /// The second row's height above the axis (18 as before, more when the first row's labels need the room).
    pub dy2: f64,
    pub labels: Vec<Placed>,
    /// Labels that found no clear place (dropped from the drawing; the panel says so).
    pub dropped: usize,
    /// Extra room above the old canvas and below it (notes in the figure).
    pub top: f64, pub bottom: f64,
    /// Notes drawn under the line (a row the figure cannot read, too many ticks), and their height.
    pub fig_notes: Vec<String>, pub notes_h: f64,
}

pub(super) fn plan(p: &Params) -> Plan {
    let (lo, hi) = range(p);
    let (step, den) = nl_step(p);
    let pi = pi_step(p);
    let mixed = p.s("frac_style").starts_with("Mixed");
    let (w, _, x0, x1, y_old) = super::nl_geom_old(p);
    let sx = |v: f64| x0 + (v - lo) / (hi - lo) * (x1 - x0);
    let k0 = (lo / step - 1e-9).ceil() as i64;
    let k1 = ((hi + 1e-9) / step).floor() as i64;
    let too_many = k1.saturating_sub(k0) >= MAX_TICKS;
    let fs = 16.0 * lscale(p);
    let mut every = p.i("label_every").max(1);
    // π numbers: every n-th tick as asked, more apart when neighbours would touch
    if let (Some((a, b)), false, true) = (pi, too_many, p.b("numbers")) {
        let wide = |k: i64| pi_txt(k * a, b).dims(fs, true).0;
        let gap = sx(step) - sx(0.0);
        let fits = |e: i64| { let mut k = k0; while k + e <= k1 { if gap * e as f64 - (wide(k) + wide(k + e)) / 2.0 < 8.0 { return false; } k += e; } true };
        let mut m = 1;
        while !fits(every * m) && every * m <= (k1 - k0).max(1) { m += 1; }
        if m > 1 {
            // thinned: rather a step of whole or half turns of π (2π, π, π/2, 4π ...) than 7π/6, if one
            // fits within four times as far
            let nice = |e: i64| { let (mut x, mut y) = (e * a, b); while y != 0 { let t = x % y; x = y; y = t; } let (n, d) = (e * a / x, b / x); d <= 2 && [1, 2, 4, 5, 10, 20, 25, 50, 100, 200, 250, 500].contains(&n) };
            let e0 = every * m;
            every = (e0..=e0 * 4).find(|e| nice(*e) && fits(*e)).unwrap_or(e0);
        } else { every *= m; }
    }
    let mut pi_off = vec![];
    if let (Some((a, b)), false, true) = (pi, too_many, p.b("numbers")) {
        let mut k = k0;
        while k <= k1 {
            let hw = pi_txt(k * a, b).dims(fs, true).0 / 2.0;
            let x = sx(k as f64 * step);
            if x - hw < 1.0 || x + hw > w - 1.0 { pi_off.push(k); }
            k += every;
        }
    }
    let rows = [read_row(p, 0), read_row(p, 1)];
    let mut pl = Plan { lo, hi, step, den, pi, mixed, k0, k1, every, too_many, pi_off: pi_off.clone(), rows, dy2: 18.0, labels: vec![], dropped: 0, top: 0.0, bottom: 0.0, fig_notes: vec![], notes_h: 0.0 };
    let numbered = |v: f64| p.b("numbers") && !too_many && { let k = (v / step).round() as i64; (v - k as f64 * step).abs() < 1e-9 && k >= k0 && k <= k1 && (k - k0) % every == 0 && !pi_off.contains(&k) };
    let ls = lscale(p);
    let vsize = 13.0 * ls;

    // what each band asks for: (mark x, text, size, row, prefer the mark's band)
    let mut wants: [Vec<(f64, Txt, f64, usize)>; 2] = [vec![], vec![]];
    // every mark on each band (a value label must stand over its own mark and no other)
    let mut marks: [Vec<f64>; 2] = [vec![], vec![]];
    for i in 0..2 {
        let write = p.s(ROWS[i].3) == "Write";
        match &pl.rows[i] {
            Row::Ineq(ivs) => for iv in ivs {
                for v in [iv.lo, iv.hi] {
                    if !v.is_finite() || v < lo - 1e-9 || v > hi + 1e-9 { continue; }
                    marks[i].push(sx(v));
                    if write && !numbered(v) && !wants[i].iter().any(|w| (w.0 - sx(v)).abs() < 1e-9) { wants[i].push((sx(v), value_txt(v, &pl, false), vsize, i)); }
                }
            },
            Row::Seq(s) => for (_, v) in &s.terms {
                if *v < lo - 1e-9 || *v > hi + 1e-9 { continue; }
                marks[i].push(sx(*v));
                if write && !numbered(*v) && !wants[i].iter().any(|w| (w.0 - sx(*v)).abs() < 1e-9) { wants[i].push((sx(*v), value_txt(*v, &pl, true), vsize, i)); }
            },
            Row::Bad(_) => {}
        }
    }
    // points sit on the line (band 0); old names and values are drawn as before and only kept clear of
    let mut fixed: Vec<Bx> = vec![];
    let named_value = p.s("pt_labels") == LABELS[1];
    for q in points(p) {
        if q.v < lo - 1e-9 || q.v > hi + 1e-9 { continue; }
        let x = sx(q.v);
        marks[0].push(x);
        if !q.name.is_empty() {
            if named_value {
                let shown = q.typed.replace('-', "\u{2212}").replace("pi", "\u{3c0}");
                wants[0].push((x, Txt::Named(q.name.clone(), format!("({shown})")), 17.0 * ls, 2));
            } else {
                let wn = serif_w(&q.name, 17.0);
                fixed.push(Bx { l: x - wn / 2.0, t: -16.0 - 0.74 * 17.0, r: x + wn / 2.0, b: -16.0 + 0.22 * 17.0 });
            }
        } else if p.b("pt_values") && !numbered(q.v) {
            let hw = sans_w(&fmt(q.v, 3), 13.0, false).max(16.0) / 2.0 + 1.0;
            fixed.push(Bx { l: x - hw, t: -38.0, r: x + hw, b: -8.0 });
        }
    }
    // an axis label to the right of the line
    if !p.s("label").is_empty() { fixed.push(Bx { l: x1 + 26.0, t: -12.0, r: x1 + 26.0 + stw(p.s("label"), 16.0) + 8.0, b: 6.0 }); }
    let any_new = !wants[0].is_empty() || !wants[1].is_empty();

    // place one band: each label centred over its mark, lifted clear of what is placed (in the order
    // asked: a sequence's first terms, the lower end before the upper). A value is left off when another
    // mark stands under it (it would read as that one's); a lettered point's label names its point, so
    // like the reference it is stacked above its neighbours' instead
    let marks = &marks;
    let place = |pl: &mut Plan, band: usize, base: f64, obstacles: &[Bx]| {
        let mut done: Vec<Bx> = vec![];
        let mut labelled: Vec<f64> = vec![];
        for (mx, txt, size, row) in wants[band].iter().cloned() {
            if labelled.iter().any(|l| (l - mx).abs() < 6.0) { pl.dropped += 1; continue; }
            let (lw, lh) = txt.dims(size, matches!(txt, Txt::Named(..)));
            let cx = mx.clamp(4.0 + lw / 2.0, w - 4.0 - lw / 2.0);
            if (cx - mx).abs() > lw / 2.0 - 2.0 { pl.dropped += 1; continue; }
            let named = matches!(txt, Txt::Named(..));
            if !named && marks[band].iter().any(|m| (m - mx).abs() > 0.5 && *m >= cx - lw / 2.0 - 2.0 && *m <= cx + lw / 2.0 + 2.0) { pl.dropped += 1; continue; }
            let mut bottom = base - 10.5;
            let lowest = bottom;
            let mut ok = None;
            for _ in 0..40 {
                let bx = Bx { l: cx - lw / 2.0, t: bottom - lh, r: cx + lw / 2.0, b: bottom };
                match done.iter().chain(obstacles.iter()).filter(|o| o.hit(&bx, 2.0)).map(|o| o.t).reduce(f64::min) {
                    Some(t) => bottom = bottom.min(t - 2.5),
                    None => { ok = Some(bx); break; }
                }
                if lowest - bottom > 66.0 { break; }
            }
            match ok {
                Some(bx) => { done.push(bx); labelled.push(mx); pl.labels.push(Placed { txt, size, bx, mark: (mx, base), row }); }
                None => pl.dropped += 1,
            }
        }
        done
    };
    let band0 = place(&mut pl, 0, 0.0, &fixed);
    let row2_on = match &pl.rows[1] { Row::Ineq(v) => !v.is_empty(), Row::Seq(_) => true, Row::Bad(_) => false };
    if row2_on && any_new {
        let top0 = band0.iter().chain(if band0.is_empty() { [].iter() } else { fixed.iter() }).map(|b| b.t).fold(0.0f64, f64::min);
        pl.dy2 = 18f64.max(-top0 + 10.0);
    }
    let mut obstacles = fixed.clone();
    obstacles.extend(band0.iter().copied());
    let base2 = -pl.dy2;
    let band1 = place(&mut pl, 1, base2, &obstacles);
    if any_new || matches!(pl.rows[0], Row::Seq(_)) || matches!(pl.rows[1], Row::Seq(_)) {
        let mut min_top = band0.iter().chain(band1.iter()).map(|b| b.t).fold(0.0f64, f64::min);
        if row2_on { min_top = min_top.min(base2 - 8.0); }
        pl.top = (6.0 - (y_old + min_top)).max(0.0).ceil();
    }
    // notes drawn in the figure: a row it cannot read, ticks it will not draw
    for (i, r) in pl.rows.iter().enumerate() {
        if let Row::Bad(_) = r { pl.fig_notes.push(format!("{} graph: write the sequence in n, like a_n = 1/n.", if i == 0 { "First" } else { "Second" })); }
    }
    if too_many { pl.fig_notes.push("Too many ticks: count by a larger step for this range.".into()); }
    pl.notes_h = if pl.fig_notes.is_empty() { 0.0 } else { 16.0 * pl.fig_notes.len() as f64 + 4.0 };
    // stacked π numbers need the room the old fraction numbers had
    pl.bottom = pl.notes_h + if pi.is_some_and(|(_, b)| b > 1) && den.is_none() { 8.0 } else { 0.0 };
    pl
}

/// Extra canvas (top, bottom) over the old number line's.
pub(crate) fn extra(p: &Params) -> (f64, f64) { let pl = plan(p); (pl.top, pl.bottom) }

/// The panel's notes: rows it cannot read, terms it cannot draw, labels left off, typed ends it cannot read.
pub(crate) fn warnings(p: &Params) -> Vec<String> {
    let pl = plan(p);
    let mut out = vec![];
    let (lo, hi) = (pl.lo, pl.hi);
    for (i, r) in pl.rows.iter().enumerate() {
        let which = if i == 0 { "First graph" } else { "Second graph" };
        match r {
            Row::Bad(m) => out.push(format!("{which}: {m}")),
            Row::Seq(s) => {
                let l = &s.letter;
                if !s.undefined.is_empty() {
                    let ns: Vec<String> = s.undefined.iter().take(6).map(|n| n.to_string()).collect();
                    let more = if s.undefined.len() > 6 { " ..." } else { "" };
                    out.push(format!("{which}: {l}\u{2099} has no value at n = {}{more} (it divides by zero or takes a root or logarithm of a negative there), so no point is drawn for it.", ns.join(", ")));
                }
                let off: Vec<&(i64, f64)> = s.terms.iter().filter(|(_, v)| *v < lo - 1e-9 || *v > hi + 1e-9).collect();
                if !off.is_empty() {
                    let (n, v) = off[0];
                    out.push(format!("{which}: {} of the {} terms are off the line from {} to {} (the first is {l}{} = {}). Widen From and To to show them.", off.len(), s.terms.len(), fmt(lo, 3), fmt(hi, 3), sub(*n), fmt(*v, 3)));
                }
                if s.terms.is_empty() && s.undefined.is_empty() { out.push(format!("{which}: no terms; set the first and last n.")); }
            }
            Row::Ineq(_) => {}
        }
    }
    if pl.dropped > 0 {
        out.push(format!("{} value{} above the line {} left off: {} too close to another one to be told apart. Count by a smaller step, or widen the line.", pl.dropped, if pl.dropped == 1 { "" } else { "s" }, if pl.dropped == 1 { "is" } else { "are" }, if pl.dropped == 1 { "it is" } else { "each is" }));
    }
    for (k, what) in [("min_text", "From"), ("max_text", "To")] {
        let t = p.s(k).trim();
        if !t.is_empty() && nl_num(t).is_none_or(|v| !v.is_finite()) { out.push(format!("Type {what} as a number, a fraction or a multiple of \u{3c0}, like \u{2212}2pi or 3/2.")); }
        else if !t.is_empty() && nl_num(t).is_some_and(|v| v.abs() > 1000.0) { out.push(format!("{what} is kept between \u{2212}1000 and 1000.")); }
    }
    if has_pi(p.s("step_text")) && pl.pi.is_none() { out.push("Count by is not a simple multiple of \u{3c0} (like pi/6, pi/2, 2pi/3 or pi), so the ticks are numbered in decimals.".into()); }
    if pl.too_many { out.push("Too many ticks for this range: count by a larger step.".into()); }
    out
}
/// n as subscript digits (a₁₂).
fn sub(n: i64) -> String { n.to_string().chars().map(|c| char::from_u32(0x2080 + c.to_digit(10).unwrap_or(0)).unwrap_or(c)).collect() }

// ───────────────────────── drawing ─────────────────────────

/// A cross mark (two strokes 2.6 wide), the reference's "Cross" point.
fn cross(d: &mut Doc, x: f64, y: f64, col: &str) {
    d.line(x - 5.5, y - 5.5, x + 5.5, y + 5.5, col, 2.6);
    d.line(x - 5.5, y + 5.5, x + 5.5, y - 5.5, col, 2.6);
}

pub(super) fn render(p: &Params) -> String {
    let pl = plan(p);
    let (lo, hi, step, den, mixed) = (pl.lo, pl.hi, pl.step, pl.den, pl.mixed);
    let (w, h, x0, x1, y) = nl_geom(p);
    let fs = 16.0 * lscale(p);
    let sx = |v: f64| x0 + (v - lo) / (hi - lo) * (x1 - x0);
    let mut d = new_doc(w, h);
    if p.b("arrows") { axis_heads(&mut d, x0, x1, y); }
    d.line(x0, y, x1, y, INK, 2.4);
    let (k0, k1, every) = (pl.k0, pl.k1, pl.every);
    let minor = p.i("minor");
    let mut numbered: Vec<f64> = vec![];
    if !pl.too_many {
        for k in k0..=k1 {
            let v = k as f64 * step;
            d.line(sx(v), y - 9.0, sx(v), y + 9.0, INK, 2.0);
            if p.b("numbers") && (k - k0) % every == 0 && !pl.pi_off.contains(&k) {
                numbered.push(v);
                match (pl.pi, den) {
                    (Some((a, b)), _) => match pi_txt(k * a, b) {
                        Txt::Stack(st) => draw_stack(&mut d, sx(v), y + 24.0, &st, fs, true),
                        // whole multiples sit where a fraction line's whole numbers sit (one lower when there are stacks)
                        t => txtw(&mut d, sx(v), y + if b > 1 { 24.0 } else { 23.0 } + fs * 0.34, fs, "middle", &t.plain(), true),
                    },
                    (None, Some(dd)) => frac_label_w(&mut d, sx(v), y + 24.0, v, dd, mixed, fs, true),
                    (None, None) => txtw(&mut d, sx(v), y + 23.0 + fs * 0.34, fs, "middle", &fmt(v, 3), true),
                }
            }
            if minor > 0 && k < k1 {
                for m in 1..=minor { let mv = v + step * m as f64 / (minor + 1) as f64; d.line(sx(mv), y - 4.0, sx(mv), y + 4.0, INK, 1.0); }
            }
        }
    }
    if !p.s("label").is_empty() { math_text(&mut d, x1 + 26.0, y, 16.0, "start", p.s("label"), false); }
    // graphs: row 1 on the line, row 2 (its own colour) drawn separately just above it
    for (i, r) in ROWS.iter().enumerate() {
        let col = colp(p, r.1, r.2);
        let col = col.as_str();
        let yy = if i == 0 { y } else { y - pl.dy2 };
        match &pl.rows[i] {
            Row::Ineq(ivs) => for iv in ivs {
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
            },
            Row::Seq(s) => {
                let crossed = p.s(r.4) == "Cross";
                for (_, v) in &s.terms {
                    if *v < lo - 1e-9 || *v > hi + 1e-9 { continue; }
                    if crossed { cross(&mut d, sx(*v), yy, col); } else { d.circle(sx(*v), yy, 6.0, col, col, 2.2); }
                }
            }
            Row::Bad(_) => {}
        }
    }
    let red_s = colp(p, "pt_color", "#c0392b");
    let red = red_s.as_str();
    let (crossed, named_value) = (p.s("pt_mark") == "Cross", p.s("pt_labels") == LABELS[1]);
    for q in points(p) {
        let (v, open) = (q.v, q.open);
        if v < lo - 1e-9 || v > hi + 1e-9 { continue; }
        if crossed && !open { cross(&mut d, sx(v), y, red); } else { d.circle(sx(v), y, 6.0, if open { "#fff" } else { red }, red, 2.2); }
        if !q.name.is_empty() {
            if !named_value {
                d.raw(&format!(r##"<text x="{:.2}" y="{:.2}" font-size="17" text-anchor="middle" font-family="{SERIF}" font-style="italic" font-weight="bold" fill="#000">{}</text>"##, sx(v), y - 16.0, super::super::svg::esc(&q.name)));
            }
        } else if p.b("pt_values") && !numbered.iter().any(|n| (n - v).abs() < 1e-9) {
            match den { Some(dd) if ((v * dd as f64) - (v * dd as f64).round()).abs() < 1e-6 => frac_label(&mut d, sx(v), y - 26.0, v, dd, mixed, 13.0), _ => put(&mut d, sx(v), y - 18.0, &fmt(v, 3), 13.0) }
        }
    }
    // the new labels, each over its own mark
    for l in &pl.labels {
        let cx = (l.bx.l + l.bx.r) / 2.0;
        match &l.txt {
            Txt::Plain(s) => txtw(&mut d, cx, y + l.bx.b - 0.22 * l.size, l.size, "middle", s, false),
            Txt::Stack(st) => { let (_, below) = stack_extent(l.size); draw_stack(&mut d, cx, y + l.bx.b - below, st, l.size, false) }
            Txt::Named(n, rest) => {
                let sz = l.size;
                d.raw(&format!(r##"<text x="{cx:.2}" y="{:.2}" font-size="{sz}" text-anchor="middle" font-family="{SERIF}" font-weight="bold" fill="#000"><tspan font-style="italic">{}</tspan><tspan font-size="{:.2}" font-family="DejaVu Sans, Arial, Liberation Sans, Noto Sans, Helvetica, sans-serif">{}</tspan></text>"##,
                    y + l.bx.b - 0.22 * sz, super::super::svg::esc(n), sz * 14.0 / 17.0, super::super::svg::esc(rest)));
            }
        }
    }
    for (i, n) in pl.fig_notes.iter().enumerate() {
        d.raw(&format!(r##"<text x="16" y="{:.2}" font-size="12" font-family="DejaVu Sans, Arial, Liberation Sans, Noto Sans, Helvetica, sans-serif" fill="#8a1c1c">{}</text>"##, h - pl.notes_h + 13.0 + 16.0 * i as f64, super::super::svg::esc(n)));
    }
    d.finish()
}
