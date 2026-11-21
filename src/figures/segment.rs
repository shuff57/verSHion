//! Segment addition: two to seven points in order on one line. Each part (the piece between two
//! neighbouring points) is labeled above the line with a length (12), an expression in one letter
//! (2x + 3, 3(x − 2), x/2), a blank line for students or nothing, and carries none to three
//! congruence ticks. Up to three spans are braces under the line from a point to a later one,
//! each labeled the same ways. Either end can stop at its point or run on to an arrow (a segment,
//! a ray or a line), a line can cross at any point at any angle (with a right-angle mark at 90°),
//! the parts are drawn to scale from their lengths (or as typed, for a figure not drawn to scale)
//! and the whole figure can be turned.
//!
//! The labels are solved exactly as a check: each brace gives an equation (AB + BC = AC), parts
//! with the same ticks are equal (AM = MB), the one letter is solved for and every part's and
//! brace's length is worked out, a blank part's too. The key copy (Show answers) writes the check
//! under the figure and the worked-out length on each blank line; it also says when the labels
//! don't agree or can't be solved. The student copy leaves the blank lines empty and the check out.
//!
//! Points and parts share numbered slots (`p2_name` is the second point, `p2_lab` the part from it
//! to the third), so removing a point moves the later ones up a slot (`remove_point`).
//!
//! Original design on the shared pieces (`Fills`, `show_answers`, `label_size`).
use crate::figures::layout::item_of;
use crate::figures::stats::tov::Rat;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "segment-addition";
pub const MIN_POINTS: usize = 2;
pub const MAX_POINTS: usize = 7;
pub const MAX_SPANS: usize = 3;
/// Longest point or crossing-line name, in characters.
pub const MAX_NAME: usize = 4;
/// Longest part or brace label, in characters.
pub const MAX_LABEL: usize = 24;

pub const LABEL_MODES: [&str; 3] = ["Text", "Blank line", "None"];
pub const TICKS: [&str; 4] = ["None", "1 tick", "2 ticks", "3 ticks"];
pub const ENDS: [&str; 3] = ["Stops at the point", "Arrow", "Line arrow"];

pub const NAME_KEYS: [&str; 7] = ["p1_name", "p2_name", "p3_name", "p4_name", "p5_name", "p6_name", "p7_name"];
pub const DOT_KEYS: [&str; 7] = ["p1_dot", "p2_dot", "p3_dot", "p4_dot", "p5_dot", "p6_dot", "p7_dot"];
pub const MODE_KEYS: [&str; 6] = ["p1_lab_mode", "p2_lab_mode", "p3_lab_mode", "p4_lab_mode", "p5_lab_mode", "p6_lab_mode"];
pub const LAB_KEYS: [&str; 6] = ["p1_lab", "p2_lab", "p3_lab", "p4_lab", "p5_lab", "p6_lab"];
pub const TICK_KEYS: [&str; 6] = ["p1_ticks", "p2_ticks", "p3_ticks", "p4_ticks", "p5_ticks", "p6_ticks"];
pub const LEN_KEYS: [&str; 6] = ["p1_len", "p2_len", "p3_len", "p4_len", "p5_len", "p6_len"];
pub const FROM_KEYS: [&str; 3] = ["s1_from", "s2_from", "s3_from"];
pub const TO_KEYS: [&str; 3] = ["s1_to", "s2_to", "s3_to"];
pub const SPAN_MODE_KEYS: [&str; 3] = ["s1_lab_mode", "s2_lab_mode", "s3_lab_mode"];
pub const SPAN_LAB_KEYS: [&str; 3] = ["s1_lab", "s2_lab", "s3_lab"];

/// Names given to new points, in order (the first free one is used).
const LETTERS: [&str; 26] = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z"];
const DEFAULT_LABS: [&str; 6] = ["2x + 1", "3x - 4", "", "", "", ""];

const MSERIF: &str = "Liberation Serif, Noto Serif, DejaVu Serif, Times New Roman, serif";
/// For a label with a character the first serif lacks (ℓ): a missing glyph would set the whole
/// label in a thin fallback face, so such a label is set in the bundled Source Serif instead.
const WIDE_SERIF: &str = "Source Serif 4, DejaVu Serif, serif";
const SANS: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const INK: &str = "#111827";
/// The key copy's answers (Colour style); Print writes them black.
const KEY_INK: &str = "#1d4ed8";

pub fn def() -> FigureDef {
    FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Segment Addition",
        blurb: "Points in order on a line, each part labeled with a length, an expression in one letter or a blank line, with congruence ticks, braces for longer spans, arrows for a ray or a line and a crossing line; the labels are solved as a check on the key.",
        params,
        render,
    }
}

fn params() -> Vec<ParamDef> {
    let mut v = vec![ParamDef::int("n", "Points", MIN_POINTS as i64, MAX_POINTS as i64, 3).hint("The points sit in order along one line, from two to seven")];
    for i in 0..MAX_POINTS {
        v.push(ParamDef::text(NAME_KEYS[i], "Name", LETTERS[i]).hint("Up to four characters, such as M for a midpoint"));
        v.push(ParamDef::bool(DOT_KEYS[i], "Dotted", true));
        if i < MAX_POINTS - 1 {
            v.push(ParamDef::choice(MODE_KEYS[i], "Label", &LABEL_MODES, LABEL_MODES[0]));
            v.push(ParamDef::text(LAB_KEYS[i], "Length", DEFAULT_LABS[i]).hint("A length like 12, or an expression in one letter like 2x + 3, 3(x − 2) or x/2"));
            v.push(ParamDef::choice(TICK_KEYS[i], "Congruence ticks", &TICKS, TICKS[0]).hint("Give parts the same ticks to mark them congruent, as at a midpoint"));
            v.push(ParamDef::text(LEN_KEYS[i], "Drawn length", "").hint("Leave it empty to draw the part as labeled (or as solved). Type a size for a figure not drawn to scale"));
        }
    }
    v.push(ParamDef::int("spans", "Spans", 0, MAX_SPANS as i64, 1).hint("A span is a brace under the line from one point to a later one, like AC, with its own label"));
    for j in 0..MAX_SPANS {
        v.push(ParamDef::int(FROM_KEYS[j], "Starts at", 1, MAX_POINTS as i64, 1));
        v.push(ParamDef::int(TO_KEYS[j], "Ends at", 1, MAX_POINTS as i64, if j == 0 { 3 } else { 2 }));
        v.push(ParamDef::choice(SPAN_MODE_KEYS[j], "Label", &LABEL_MODES, LABEL_MODES[0]));
        v.push(ParamDef::text(SPAN_LAB_KEYS[j], "Length", if j == 0 { "27" } else { "" }).hint("A length like 27, or an expression in the same letter as the parts"));
    }
    v.extend([
        ParamDef::choice("left_end", "Left end", &ENDS, ENDS[0]).hint("Stopping at both points makes a segment; one arrow makes a ray, two a line"),
        ParamDef::choice("right_end", "Right end", &ENDS, ENDS[0]),
        ParamDef::int("cross_at", "Crosses at", 0, MAX_POINTS as i64, 0).hint("A line crossing at a point: a bisector crosses at the midpoint, with the same ticks on the parts either side"),
        ParamDef::text("cross_name", "Line name", "").hint("Such as ℓ, up to four characters"),
        ParamDef::float("cross_ang", "Angle (°)", 20.0, 160.0, 5.0, 90.0).hint("Measured from the line's right-hand side, counterclockwise"),
        ParamDef::bool("cross_right", "Right-angle mark", true),
        ParamDef::float("turn", "Turn", -90.0, 90.0, 5.0, 0.0).hint("Turns the whole figure; the labels stay upright"),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off makes the student copy: the blank lines stay empty and the check under the figure is left out. Student + key saves both"),
    ]);
    v
}

pub fn n_points(p: &Params) -> usize { (p.i("n").clamp(MIN_POINTS as i64, MAX_POINTS as i64)) as usize }
pub fn n_spans(p: &Params) -> usize { p.i("spans").clamp(0, MAX_SPANS as i64) as usize }
/// The point the crossing line goes through (0-based), if any.
pub fn cross_at(p: &Params) -> Option<usize> {
    let c = p.i("cross_at");
    (c >= 1 && c as usize <= n_points(p)).then(|| c as usize - 1)
}

/// Whether control `key` draws anything with the current params.
pub fn relevant(key: &str, p: &Params) -> bool {
    let n = n_points(p);
    if let Some((fam, i)) = item_of(key) {
        let rest = key.split_once('_').map(|(_, r)| r).unwrap_or("");
        return match fam {
            "p" => match rest {
                "name" | "dot" => i <= n,
                "lab" => i < n && p.s(&format!("p{i}_lab_mode")) == LABEL_MODES[0],
                _ => i < n,
            },
            "s" => i <= n_spans(p) && (rest != "lab" || p.s(&format!("s{i}_lab_mode")) == LABEL_MODES[0]),
            _ => true,
        };
    }
    match key {
        "cross_name" | "cross_ang" => cross_at(p).is_some(),
        "cross_right" => cross_at(p).is_some() && (p.f("cross_ang") - 90.0).abs() < 1e-9,
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID {
        return None;
    }
    let s = |v: &str| Value::S(v.to_string());
    let i = Value::I;
    Some(vec![
        Example { name: "AB + BC = AC: solve for x", params: vec![] },
        Example {
            name: "Midpoint of a segment",
            params: vec![
                ("n", i(3)),
                ("p2_name", s("M")),
                ("p3_name", s("B")),
                ("p1_lab", s("3x + 2")),
                ("p2_lab", s("5x - 6")),
                ("p1_ticks", s("1 tick")),
                ("p2_ticks", s("1 tick")),
                ("s1_lab_mode", s("Blank line")),
            ],
        },
        Example {
            name: "Find the missing part",
            params: vec![
                ("n", i(4)),
                ("p1_name", s("P")),
                ("p2_name", s("Q")),
                ("p3_name", s("R")),
                ("p4_name", s("S")),
                ("p1_lab", s("5")),
                ("p2_lab_mode", s("Blank line")),
                ("p3_lab", s("4")),
                ("spans", i(2)),
                ("s1_to", i(3)),
                ("s1_lab", s("12")),
                ("s2_from", i(1)),
                ("s2_to", i(4)),
                ("s2_lab_mode", s("None")),
            ],
        },
        Example {
            name: "Overlapping segments",
            params: vec![
                ("n", i(4)),
                ("p1_lab", s("x")),
                ("p2_lab", s("x + 2")),
                ("p3_lab", s("2x")),
                ("spans", i(3)),
                ("s1_from", i(1)),
                ("s1_to", i(3)),
                ("s1_lab", s("10")),
                ("s2_from", i(2)),
                ("s2_to", i(4)),
                ("s2_lab_mode", s("Blank line")),
                ("s3_from", i(1)),
                ("s3_to", i(4)),
                ("s3_lab_mode", s("Blank line")),
            ],
        },
        Example {
            name: "Perpendicular bisector",
            params: vec![
                ("p2_name", s("M")),
                ("p3_name", s("B")),
                ("p1_lab_mode", s("None")),
                ("p2_lab_mode", s("None")),
                ("p1_ticks", s("1 tick")),
                ("p2_ticks", s("1 tick")),
                ("spans", i(0)),
                ("cross_at", i(2)),
                ("cross_name", s("ℓ")),
            ],
        },
        Example {
            name: "A line through four points",
            params: vec![
                ("n", i(4)),
                ("p1_lab_mode", s("None")),
                ("p2_lab_mode", s("None")),
                ("p3_lab_mode", s("None")),
                ("spans", i(0)),
                ("left_end", s("Arrow")),
                ("right_end", s("Arrow")),
            ],
        },
        Example {
            name: "Not drawn to scale",
            params: vec![("p1_lab", s("4x - 3")), ("p2_lab", s("2x + 5")), ("s1_lab", s("50")), ("p1_len", s("1")), ("p2_len", s("3")), ("p1_lab_mode", s("Text"))],
        },
    ])
}

// ───────────────────────── labels as numbers ─────────────────────────

fn sub(a: Rat, b: Rat) -> Option<Rat> { a.add(b.neg()) }
fn zero() -> Rat { Rat::int(0) }

/// A label's value: `a·letter + b`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lin {
    pub a: Rat,
    pub b: Rat,
    pub letter: Option<char>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Why {
    /// More than one letter in one label.
    Letters(Vec<char>),
    /// Not a number or a linear expression (x², a word, 5 6).
    Unreadable,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tok {
    Num(Rat),
    Var(char),
    Op(char),
    Lp,
    Rp,
}

/// Units a length may carry after its number or expression ("12 cm"); not solved for.
const UNITS: [&str; 10] = ["units", "mm", "cm", "km", "in", "ft", "yd", "mi", "m", "u"];

/// The label without a trailing unit, and the unit. Two-letter units may be typed against the
/// number (12cm); one-letter ones (12 m) need a space, since 12m reads as 12 times m.
pub fn split_unit(s: &str) -> (String, Option<String>) {
    let t = s.trim();
    if let Some((head, last)) = t.rsplit_once(char::is_whitespace) {
        if UNITS.contains(&last) && !head.trim().is_empty() {
            return (head.trim().to_string(), Some(last.to_string()));
        }
    }
    for u in UNITS.iter().filter(|u| u.len() >= 2) {
        if let Some(head) = t.strip_suffix(u) {
            if head.chars().last().is_some_and(|c| c.is_ascii_digit()) {
                return (head.to_string(), Some(u.to_string()));
            }
        }
    }
    (t.to_string(), None)
}

fn decimal(s: &str) -> Option<Rat> {
    let (int, frac) = s.split_once('.').unwrap_or((s, ""));
    if int.len() + frac.len() > 24 || (int.is_empty() && frac.is_empty()) {
        return None;
    }
    let digits = format!("{int}{frac}");
    let n: i128 = digits.parse().ok()?;
    Rat::new(n, 10i128.checked_pow(frac.len() as u32)?)
}

fn tokens(s: &str) -> Option<Vec<Tok>> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut gap = false;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            gap = true;
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || c == '.' {
            let mut j = i;
            while j < cs.len() && (cs[j].is_ascii_digit() || cs[j] == '.') {
                j += 1;
            }
            let num = decimal(&cs[i..j].iter().collect::<String>())?;
            // a whole number, a space and a fraction is a mixed number (2 1/2); two numbers side by
            // side otherwise (5 6) is not a product anyone means
            if gap && matches!(out.last(), Some(Tok::Num(_))) {
                let Some(Tok::Num(whole)) = out.pop() else { return None };
                let mut k = j;
                while k < cs.len() && cs[k].is_whitespace() {
                    k += 1;
                }
                let mut e = k + 1;
                while e < cs.len() && cs[e].is_ascii_digit() {
                    e += 1;
                }
                if !(whole.is_int() && num.is_int() && cs.get(k) == Some(&'/') && e > k + 1) {
                    return None;
                }
                let den = decimal(&cs[k + 1..e].iter().collect::<String>())?;
                if den.n == 0 || num.n >= den.n {
                    return None;
                }
                out.push(Tok::Num(whole.add(num.mul(den.recip()?)?)?));
                i = e;
                gap = false;
                continue;
            }
            out.push(Tok::Num(num));
            i = j;
            gap = false;
            continue;
        }
        let t = match c {
            '+' => Tok::Op('+'),
            '-' | '\u{2212}' | '\u{2013}' | '\u{2014}' => Tok::Op('-'),
            '*' | '\u{d7}' | '\u{b7}' | '\u{22c5}' | '\u{2219}' => Tok::Op('*'),
            '/' | '\u{f7}' => Tok::Op('/'),
            '^' => Tok::Op('^'),
            '(' | '[' | '{' => Tok::Lp,
            ')' | ']' | '}' => Tok::Rp,
            c if c.is_alphabetic() => Tok::Var(c),
            _ => return None,
        };
        out.push(t);
        gap = false;
        i += 1;
    }
    Some(out)
}

#[derive(Debug)]
enum E {
    Letters,
    Bad,
}

struct Rd<'a> {
    t: &'a [Tok],
    i: usize,
    letters: Vec<char>,
}

fn join_letter(a: Option<char>, b: Option<char>) -> Result<Option<char>, E> {
    match (a, b) {
        (Some(x), Some(y)) if x != y => Err(E::Letters),
        (Some(x), _) | (_, Some(x)) => Ok(Some(x)),
        _ => Ok(None),
    }
}

fn lin_add(u: Lin, v: Lin) -> Result<Lin, E> { Ok(Lin { a: u.a.add(v.a).ok_or(E::Bad)?, b: u.b.add(v.b).ok_or(E::Bad)?, letter: join_letter(u.letter, v.letter)? }) }
fn lin_neg(u: Lin) -> Lin { Lin { a: u.a.neg(), b: u.b.neg(), letter: u.letter } }
fn lin_mul(u: Lin, v: Lin) -> Result<Lin, E> {
    let letter = join_letter(u.letter, v.letter)?;
    if u.a.n != 0 && v.a.n != 0 {
        return Err(E::Bad);
    }
    let a = u.a.mul(v.b).ok_or(E::Bad)?.add(v.a.mul(u.b).ok_or(E::Bad)?).ok_or(E::Bad)?;
    Ok(Lin { a, b: u.b.mul(v.b).ok_or(E::Bad)?, letter })
}
fn lin_div(u: Lin, v: Lin) -> Result<Lin, E> {
    if v.a.n != 0 || v.b.n == 0 {
        return Err(E::Bad);
    }
    let r = v.b.recip().ok_or(E::Bad)?;
    Ok(Lin { a: u.a.mul(r).ok_or(E::Bad)?, b: u.b.mul(r).ok_or(E::Bad)?, letter: join_letter(u.letter, v.letter)? })
}
fn konst(r: Rat) -> Lin { Lin { a: zero(), b: r, letter: None } }

impl Rd<'_> {
    fn peek(&self) -> Option<Tok> { self.t.get(self.i).copied() }
    fn expr(&mut self) -> Result<Lin, E> {
        let mut v = self.term()?;
        loop {
            match self.peek() {
                Some(Tok::Op('+')) => {
                    self.i += 1;
                    v = lin_add(v, self.term()?)?;
                }
                Some(Tok::Op('-')) => {
                    self.i += 1;
                    v = lin_add(v, lin_neg(self.term()?))?;
                }
                _ => return Ok(v),
            }
        }
    }
    fn term(&mut self) -> Result<Lin, E> {
        let mut v = self.unary()?;
        loop {
            match self.peek() {
                Some(Tok::Op('*')) => {
                    self.i += 1;
                    v = lin_mul(v, self.unary()?)?;
                }
                Some(Tok::Op('/')) => {
                    self.i += 1;
                    v = lin_div(v, self.unary()?)?;
                }
                // 2x, 3(x − 2), x(2): a product written without a sign
                Some(Tok::Var(_) | Tok::Lp) => v = lin_mul(v, self.power()?)?,
                _ => return Ok(v),
            }
        }
    }
    fn unary(&mut self) -> Result<Lin, E> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.i += 1;
                Ok(lin_neg(self.unary()?))
            }
            Some(Tok::Op('+')) => {
                self.i += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Result<Lin, E> {
        let base = self.primary()?;
        if self.peek() != Some(Tok::Op('^')) {
            return Ok(base);
        }
        self.i += 1;
        let e = self.unary()?;
        if e.a.n != 0 || !e.b.is_int() || e.b.n.abs() > 12 {
            return Err(E::Bad);
        }
        match e.b.n {
            1 => Ok(base),
            0 => Ok(konst(Rat::int(1))),
            k if base.a.n == 0 => {
                let mut r = Rat::int(1);
                for _ in 0..k.abs() {
                    r = r.mul(base.b).ok_or(E::Bad)?;
                }
                Ok(konst(if k < 0 { r.recip().ok_or(E::Bad)? } else { r }))
            }
            _ => Err(E::Bad),
        }
    }
    fn primary(&mut self) -> Result<Lin, E> {
        let t = self.peek().ok_or(E::Bad)?;
        self.i += 1;
        match t {
            Tok::Num(r) => Ok(konst(r)),
            Tok::Var(c) => {
                if !self.letters.contains(&c) {
                    self.letters.push(c);
                }
                Ok(Lin { a: Rat::int(1), b: zero(), letter: Some(c) })
            }
            Tok::Lp => {
                let v = self.expr()?;
                if self.peek() != Some(Tok::Rp) {
                    return Err(E::Bad);
                }
                self.i += 1;
                Ok(v)
            }
            _ => Err(E::Bad),
        }
    }
}

/// Reads a label (its unit already split off) as `a·letter + b`.
pub fn read_lin(s: &str) -> Result<Lin, Why> {
    let t = tokens(s).ok_or(Why::Unreadable)?;
    // two letters side by side (half, xy) are a word, not a product of two letters
    if t.is_empty() || t.windows(2).any(|w| matches!((w[0], w[1]), (Tok::Var(_), Tok::Var(_)))) {
        return Err(Why::Unreadable);
    }
    let mut rd = Rd { t: &t, i: 0, letters: Vec::new() };
    let r = rd.expr();
    let letters = rd.letters.clone();
    if letters.len() > 1 {
        return Err(Why::Letters(letters));
    }
    match r {
        Ok(v) if rd.i == t.len() => Ok(if v.a.n == 0 { Lin { letter: None, ..v } } else { v }),
        Err(E::Letters) => Err(Why::Letters(letters)),
        _ => Err(Why::Unreadable),
    }
}

// ───────────────────────── the model ─────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum Lab {
    None,
    Blank,
    Val(Lin),
    /// Typed but not a number or an expression: drawn as typed, left out of the check.
    Text,
}

#[derive(Clone, Debug)]
pub struct Part {
    /// As typed (trimmed, cut to `MAX_LABEL`).
    pub raw: String,
    pub lab: Lab,
    pub ticks: u8,
    pub typed_len: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Span {
    /// The slot it came from (0-based), for its control.
    pub slot: usize,
    /// Point indexes (0-based), `from < to`.
    pub from: usize,
    pub to: usize,
    pub raw: String,
    pub lab: Lab,
}

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub names: Vec<String>,
    /// Whether a label was typed as a mixed number, so answers are written that way too.
    pub mixed: bool,
    /// The names the check uses (a point with no name is called by its place's letter).
    pub call: Vec<String>,
    pub dots: Vec<bool>,
    pub parts: Vec<Part>,
    pub spans: Vec<Span>,
    pub letter: Option<char>,
    pub unit: Option<String>,
    /// The equations the braces and the ticks give, written out (AB + BC = AC, AM = MB).
    pub eqs: Vec<String>,
    pub letter_val: Option<Rat>,
    pub part_len: Vec<Option<Rat>>,
    pub span_len: Vec<Option<Rat>>,
    /// Labels that don't agree or can't be solved: written on the key and in the editor.
    pub problems: Vec<String>,
    /// About the controls (an unreadable drawn length, two points with one name): the editor only.
    pub notes: Vec<String>,
}

fn clip(s: &str, n: usize) -> String { s.chars().take(n).collect() }

fn read_label(raw: &str, mode: &str, units: &mut Vec<String>) -> Lab {
    match mode {
        "Blank line" => return Lab::Blank,
        "None" => return Lab::None,
        _ => {}
    }
    if raw.is_empty() {
        return Lab::None;
    }
    let (body, u) = split_unit(raw);
    match read_lin(&body) {
        Ok(v) => {
            if let Some(u) = u.filter(|u| !units.contains(u)) {
                units.push(u);
            }
            Lab::Val(v)
        }
        Err(_) => Lab::Text,
    }
}

/// A number as the labels write theirs: mixed numbers (5 3/4) when a label was typed as one,
/// else `rat_str`.
pub fn num(m: &Model, r: Rat) -> String {
    if !m.mixed || r.is_int() {
        return rat_str(r);
    }
    let (n, d) = (r.n.abs(), r.d);
    let (w, rest) = (n / d, n % d);
    let body = if w == 0 { format!("{rest}/{d}") } else { format!("{w} {rest}/{d}") };
    if r.n < 0 {
        format!("\u{2212}{body}")
    } else {
        body
    }
}

/// Whether `s` has a mixed number: a whole number, a space and a fraction (2 1/2).
fn has_mixed(s: &str) -> bool {
    let c: Vec<char> = s.chars().collect();
    (0..c.len()).any(|i| {
        if !c[i].is_ascii_digit() || i + 1 >= c.len() || !c[i + 1].is_whitespace() {
            return false;
        }
        let mut j = i + 1;
        while j < c.len() && c[j].is_whitespace() {
            j += 1;
        }
        let k = j;
        while j < c.len() && c[j].is_ascii_digit() {
            j += 1;
        }
        j > k && c.get(j) == Some(&'/')
    })
}

pub fn rat_str(r: Rat) -> String {
    let neg = r.n < 0;
    let (n, d) = (r.n.abs(), r.d);
    let body = if d == 1 {
        n.to_string()
    } else {
        // a denominator of only 2s and 5s ends: 3.5, 0.25
        let (mut e2, mut e5, mut q) = (0u32, 0u32, d);
        while q % 2 == 0 {
            q /= 2;
            e2 += 1;
        }
        while q % 5 == 0 {
            q /= 5;
            e5 += 1;
        }
        let places = e2.max(e5);
        match (q == 1 && places <= 4).then(|| 10i128.pow(places)).and_then(|m| n.checked_mul(m)) {
            Some(scaled) => {
                let v = scaled / d;
                let (i, f) = (v / 10i128.pow(places), v % 10i128.pow(places));
                format!("{i}.{:0w$}", f, w = places as usize).trim_end_matches('0').to_string()
            }
            None => format!("{n}/{d}"),
        }
    };
    if neg {
        format!("\u{2212}{body}")
    } else {
        body
    }
}

/// A linear system solved exactly, one equation at a time (reduced row echelon form).
struct Sys {
    nv: usize,
    /// (pivot column, coefficients, constant): Σ coefficients·v + constant = 0.
    rows: Vec<(usize, Vec<Rat>, Rat)>,
}

enum Added {
    New,
    Redundant,
    Conflict,
}

impl Sys {
    fn reduce(&self, mut co: Vec<Rat>, mut c: Rat) -> Option<(Vec<Rat>, Rat)> {
        for (pc, r, rc) in &self.rows {
            let f = co[*pc];
            if f.n != 0 {
                for k in 0..self.nv {
                    co[k] = sub(co[k], f.mul(r[k])?)?;
                }
                c = sub(c, f.mul(*rc)?)?;
            }
        }
        Some((co, c))
    }
    fn add(&mut self, co: Vec<Rat>, c: Rat) -> Option<Added> {
        let (mut co, mut c) = self.reduce(co, c)?;
        let Some(pc) = co.iter().position(|r| r.n != 0) else { return Some(if c.n == 0 { Added::Redundant } else { Added::Conflict }) };
        let inv = co[pc].recip()?;
        for k in 0..self.nv {
            co[k] = co[k].mul(inv)?;
        }
        c = c.mul(inv)?;
        for (_, r, rc) in &mut self.rows {
            let f = r[pc];
            if f.n != 0 {
                for k in 0..self.nv {
                    r[k] = sub(r[k], f.mul(co[k])?)?;
                }
                *rc = sub(*rc, f.mul(c)?)?;
            }
        }
        self.rows.push((pc, co, c));
        Some(Added::New)
    }
    fn value(&self, v: usize) -> Option<Rat> {
        let (_, r, c) = self.rows.iter().find(|(pc, _, _)| *pc == v)?;
        r.iter().enumerate().all(|(k, x)| k == v || x.n == 0).then(|| c.neg())
    }
    /// Σ co·v + c with the values known so far.
    fn eval(&self, co: &[Rat], c: Rat) -> Option<Rat> {
        let mut s = c;
        for (k, x) in co.iter().enumerate() {
            if x.n != 0 {
                s = s.add(x.mul(self.value(k)?)?)?;
            }
        }
        Some(s)
    }
}

/// A linear form over the unknowns: coefficients and a constant.
type Form = (Vec<Rat>, Rat);

fn form_add(a: &Form, b: &Form) -> Option<Form> {
    let mut co = a.0.clone();
    for (k, x) in b.0.iter().enumerate() {
        co[k] = co[k].add(*x)?;
    }
    Some((co, a.1.add(b.1)?))
}
fn form_neg(a: &Form) -> Form { (a.0.iter().map(|x| x.neg()).collect(), a.1.neg()) }

enum EqKind {
    /// A brace over parts `from..to` (point indexes) and its label.
    Span(usize),
    /// Two parts with the same ticks.
    Ticks(usize, usize),
}

struct Eq {
    kind: EqKind,
    form: Form,
    /// Whether a label in it uses the letter.
    lettered: bool,
}

pub fn model(p: &Params) -> Model {
    let n = n_points(p);
    let mut m = Model::default();
    for i in 0..n {
        let raw = p.s(NAME_KEYS[i]).trim();
        if raw.chars().count() > MAX_NAME {
            m.notes.push(format!("Point {}’s name is cut to {MAX_NAME} characters.", i + 1));
        }
        m.names.push(clip(raw, MAX_NAME));
        m.dots.push(p.b(DOT_KEYS[i]));
    }
    // a point with no name is called by its place's letter in the check
    for i in 0..n {
        let c = if m.names[i].is_empty() { LETTERS[i].to_string() } else { m.names[i].clone() };
        if m.names[i].is_empty() {
            m.notes.push(format!("Point {} has no name, so the check calls it {c}.", i + 1));
        }
        m.call.push(c);
    }
    let mut dup: Vec<String> = Vec::new();
    for i in 0..n {
        if !m.names[i].is_empty() && m.names[..i].contains(&m.names[i]) && !dup.contains(&m.names[i]) {
            dup.push(m.names[i].clone());
        }
    }
    let mut units: Vec<String> = Vec::new();
    for k in 0..n - 1 {
        let raw_full = p.s(LAB_KEYS[k]).trim();
        if raw_full.chars().count() > MAX_LABEL {
            m.notes.push(format!("{}’s label is cut to {MAX_LABEL} characters.", part_name(&m, k)));
        }
        let raw = clip(raw_full, MAX_LABEL);
        let lab = read_label(&raw, p.s(MODE_KEYS[k]), &mut units);
        let ticks = TICKS.iter().position(|t| *t == p.s(TICK_KEYS[k])).unwrap_or(0) as u8;
        let typed = p.s(LEN_KEYS[k]).trim();
        let typed_len = if typed.is_empty() {
            None
        } else {
            let v = read_lin(&split_unit(typed).0).ok().filter(|v| v.letter.is_none()).map(|v| v.b.f()).filter(|v| *v > 0.0 && v.is_finite());
            if v.is_none() {
                m.notes.push(format!("The drawn length “{typed}” for {} isn’t a number more than 0, so {} is drawn as worked out.", part_name(&m, k), part_name(&m, k)));
            }
            v
        };
        m.parts.push(Part { raw, lab, ticks, typed_len });
    }
    for j in 0..n_spans(p) {
        let a = (p.i(FROM_KEYS[j]).clamp(1, n as i64) - 1) as usize;
        let b = (p.i(TO_KEYS[j]).clamp(1, n as i64) - 1) as usize;
        let (from, to) = (a.min(b), a.max(b));
        if from == to {
            m.notes.push(format!("Span {} starts and ends at {}, so it isn’t drawn.", j + 1, m.call[from]));
            continue;
        }
        let raw_full = p.s(SPAN_LAB_KEYS[j]).trim();
        let raw = clip(raw_full, MAX_LABEL);
        let lab = read_label(&raw, p.s(SPAN_MODE_KEYS[j]), &mut units);
        m.spans.push(Span { slot: j, from, to, raw, lab });
    }
    m.unit = (units.len() == 1).then(|| units[0].clone());
    m.mixed = m.parts.iter().map(|pt| (&pt.lab, &pt.raw)).chain(m.spans.iter().map(|s| (&s.lab, &s.raw))).any(|(l, r)| matches!(l, Lab::Val(_)) && has_mixed(r));
    solve(&mut m);
    for d in dup {
        m.problems.push(format!("Two points are named {d}, so the check can’t tell their parts apart."));
    }
    if units.len() > 1 {
        m.problems.push(format!("The labels use {}: the check adds the numbers as they are, so give every label the same unit.", and_list(&units)));
    }
    // two braces with the same wrong label, or a part and its own brace, say the same thing once
    for v in [&mut m.problems, &mut m.notes] {
        let mut seen = std::collections::HashSet::new();
        v.retain(|s| seen.insert(s.clone()));
    }
    m
}

pub fn part_name(m: &Model, k: usize) -> String { format!("{}{}", m.call[k], m.call[k + 1]) }
pub fn span_name(m: &Model, s: &Span) -> String { format!("{}{}", m.call[s.from], m.call[s.to]) }

fn solve(m: &mut Model) {
    // the letters the labels use
    let mut letters: Vec<char> = Vec::new();
    let mut unread: Vec<(String, String)> = Vec::new();
    let labs: Vec<(Lab, String, String)> = m
        .parts
        .iter()
        .enumerate()
        .map(|(k, pt)| (pt.lab.clone(), pt.raw.clone(), part_name(m, k)))
        .chain(m.spans.iter().map(|s| (s.lab.clone(), s.raw.clone(), span_name(m, s))))
        .collect();
    for (lab, raw, name) in &labs {
        match lab {
            Lab::Val(v) => {
                if let Some(c) = v.letter {
                    if !letters.contains(&c) {
                        letters.push(c);
                    }
                }
            }
            Lab::Text => {
                if let Err(Why::Letters(ls)) = read_lin(&split_unit(raw).0) {
                    for c in ls {
                        if !letters.contains(&c) {
                            letters.push(c);
                        }
                    }
                }
                unread.push((raw.clone(), name.clone()));
            }
            _ => {}
        }
    }
    // the equations, written out
    for s in &m.spans {
        if s.to - s.from >= 2 {
            let parts: Vec<String> = (s.from..s.to).map(|k| part_name(m, k)).collect();
            let eq = format!("{} = {}", parts.join(" + "), span_name(m, s));
            if !m.eqs.contains(&eq) {
                m.eqs.push(eq);
            }
        }
    }
    for t in 1..=3u8 {
        let same: Vec<String> = (0..m.parts.len()).filter(|&k| m.parts[k].ticks == t).map(|k| part_name(m, k)).collect();
        if same.len() >= 2 {
            m.eqs.push(same.join(" = "));
        }
    }
    m.part_len = vec![None; m.parts.len()];
    m.span_len = vec![None; m.spans.len()];
    if letters.len() > 1 {
        let ls: Vec<String> = letters.iter().map(|c| c.to_string()).collect();
        m.problems.push(format!("The labels use {}: only one letter can be solved for, so the labels aren’t solved.", and_list(&ls)));
        for (raw, name) in &unread {
            if read_lin(&split_unit(raw).0).is_err() && !matches!(read_lin(&split_unit(raw).0), Err(Why::Letters(_))) {
                m.problems.push(format!("“{raw}” on {name} isn’t a number or an expression like 2x + 3, so it’s left out."));
            }
        }
        return;
    }
    for (raw, name) in &unread {
        match read_lin(&split_unit(raw).0) {
            Err(Why::Letters(ls)) => {
                let ls: Vec<String> = ls.iter().map(|c| c.to_string()).collect();
                m.problems.push(format!("“{raw}” on {name} uses {}: a label can use one letter, so it’s left out.", and_list(&ls)));
            }
            _ => m.problems.push(format!("“{raw}” on {name} isn’t a number or an expression like 2x + 3, so it’s left out.")),
        }
    }
    m.letter = letters.first().copied();
    let parts = m.parts.clone();
    let mut probs: Vec<String> = Vec::new();
    // unknowns: the letter, then every part with no value of its own
    let lv = m.letter.map(|_| 0);
    let mut nv = usize::from(lv.is_some());
    let part_var: Vec<Option<usize>> = parts
        .iter()
        .map(|pt| {
            (!matches!(pt.lab, Lab::Val(_))).then(|| {
                nv += 1;
                nv - 1
            })
        })
        .collect();
    let form_of = |v: &Lin| -> Form {
        let mut co = vec![zero(); nv];
        if let (Some(i), Some(_)) = (lv, v.letter) {
            co[i] = v.a;
        }
        (co, v.b)
    };
    let part_form = |k: usize| -> Form {
        match (&parts[k].lab, part_var[k]) {
            (Lab::Val(v), _) => form_of(v),
            (_, Some(i)) => {
                let mut co = vec![zero(); nv];
                co[i] = Rat::int(1);
                (co, zero())
            }
            _ => (vec![zero(); nv], zero()),
        }
    };
    let lettered = |lab: &Lab| matches!(lab, Lab::Val(v) if v.letter.is_some());
    let mut eqs: Vec<Eq> = Vec::new();
    let mut too_big = false;
    for (si, s) in m.spans.iter().enumerate() {
        let Lab::Val(v) = &s.lab else { continue };
        let mut f = (vec![zero(); nv], zero());
        for k in s.from..s.to {
            match form_add(&f, &part_form(k)) {
                Some(g) => f = g,
                None => too_big = true,
            }
        }
        match form_add(&f, &form_neg(&form_of(v))) {
            Some(g) => eqs.push(Eq { kind: EqKind::Span(si), form: g, lettered: lettered(&s.lab) || (s.from..s.to).any(|k| lettered(&m.parts[k].lab)) }),
            None => too_big = true,
        }
    }
    for t in 1..=3u8 {
        let same: Vec<usize> = (0..m.parts.len()).filter(|&k| m.parts[k].ticks == t).collect();
        for &k in same.iter().skip(1) {
            match form_add(&part_form(same[0]), &form_neg(&part_form(k))) {
                Some(g) => eqs.push(Eq { kind: EqKind::Ticks(same[0], k), form: g, lettered: lettered(&m.parts[same[0]].lab) || lettered(&m.parts[k].lab) }),
                None => too_big = true,
            }
        }
    }
    let mut sys = Sys { nv, rows: Vec::new() };
    let letter_s = m.letter.map(|c| c.to_string()).unwrap_or_default();
    let mut cancelled = false;
    // where the letter cancels out: said only if it then can't be found from the other equations
    let mut cancel_msgs: Vec<String> = Vec::new();
    // braces whose equation broke the others: they have no one length
    let mut conflicted: Vec<usize> = Vec::new();
    let mut cancel_said = false;
    for eq in &eqs {
        let letter_gone = lv.is_some_and(|i| eq.lettered && eq.form.0[i].n == 0);
        let before = lv.and_then(|i| sys.value(i));
        match sys.add(eq.form.0.clone(), eq.form.1) {
            None => {
                too_big = true;
                break;
            }
            Some(Added::Conflict) if letter_gone => {
                cancelled = true;
                if let EqKind::Span(si) = eq.kind {
                    conflicted.push(si);
                }
                cancel_said = true;
                probs.push(format!("{letter_s} cancels out of {}, and the numbers left don’t agree, so these labels can’t all be right.", eq_text(m, eq)));
            }
            Some(Added::Conflict) => {
                if let EqKind::Span(si) = eq.kind {
                    conflicted.push(si);
                }
                probs.push(conflict(m, &sys, eq, &part_form, before));
            }
            Some(Added::Redundant) if letter_gone => {
                cancelled = true;
                cancel_msgs.push(format!("{letter_s} cancels out of {}, so it can’t be solved from it.", eq_text(m, eq)));
            }
            _ => {
                if letter_gone {
                    cancelled = true;
                }
            }
        }
    }
    if too_big {
        m.problems.extend(probs);
        m.problems.push("The numbers are too large to solve exactly, so the labels aren’t solved.".into());
        return;
    }
    m.letter_val = lv.and_then(|i| sys.value(i));
    if m.letter.is_some() && m.letter_val.is_none() {
        let said = !cancel_msgs.is_empty() || cancel_said;
        probs.extend(cancel_msgs);
        let any = eqs.iter().any(|e| e.lettered);
        if !said {
        probs.push(if any && !cancelled {
            format!("{letter_s} can’t be found from these labels: label another brace, or give parts the same ticks.")
        } else if any {
            format!("{letter_s} can’t be found: it cancels out.")
        } else {
            format!("There is no equation to find {letter_s} from: label a brace over parts with {letter_s}, or give parts the same ticks.")
        });
        }
    }
    for k in 0..m.parts.len() {
        let f = part_form(k);
        m.part_len[k] = sys.eval(&f.0, f.1);
    }
    for (si, s) in m.spans.iter().enumerate() {
        let sum = (s.from..s.to).try_fold(zero(), |acc, k| acc.add(m.part_len[k]?));
        let labeled = match &s.lab {
            Lab::Val(v) => {
                let f = form_of(v);
                sys.eval(&f.0, f.1)
            }
            _ => None,
        };
        // a brace whose label disagrees with its parts has no one length (that is reported)
        m.span_len[si] = match (sum, labeled) {
            _ if conflicted.contains(&si) => None,
            (Some(a), Some(b)) if a != b => None,
            (a, b) => a.or(b),
        };
    }
    // blank lines the key can't fill
    let open: Vec<String> = (0..m.parts.len())
        .filter(|&k| parts[k].lab == Lab::Blank && m.part_len[k].is_none())
        .map(|k| part_name(m, k))
        .chain(m.spans.iter().enumerate().filter(|(si, s)| s.lab == Lab::Blank && m.span_len[*si].is_none()).map(|(_, s)| span_name(m, s)))
        .collect();
    if !open.is_empty() {
        probs.push(format!("{} can’t be worked out from these labels, so {} on the key.", and_list(&open), if open.len() == 1 { "its blank line stays empty" } else { "their blank lines stay empty" }));
    }
    let with = |lettered: bool| match (lettered, m.letter_val) {
        (true, Some(x)) => format!("With {letter_s} = {}, ", num(m, x)),
        _ => String::new(),
    };
    let unit = m.unit.as_deref().map(|u| format!(" {u}")).unwrap_or_default();
    for k in 0..m.parts.len() {
        if let Some(l) = m.part_len[k].filter(|l| l.n <= 0) {
            let w = with(lettered(&parts[k].lab));
            probs.push(cap(&format!("{w}{} comes out {}{unit}, but a length must be more than 0.", part_name(m, k), num(m, l))));
        }
    }
    for (si, s) in m.spans.iter().enumerate() {
        if let Some(l) = m.span_len[si].filter(|l| l.n <= 0) {
            // already said of its parts (a brace over one part is that part)
            if s.to - s.from == 1 || (s.from..s.to).any(|k| m.part_len[k].is_some_and(|l| l.n <= 0)) {
                continue;
            }
            let w = with(lettered(&s.lab));
            probs.push(cap(&format!("{w}{} comes out {}{unit}, but a length must be more than 0.", span_name(m, s), num(m, l))));
        }
    }
    m.problems.extend(probs);
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

fn and_list(v: &[String]) -> String {
    match v.len() {
        0 => String::new(),
        1 => v[0].clone(),
        2 => format!("{} and {}", v[0], v[1]),
        _ => format!("{} and {}", v[..v.len() - 1].join(", "), v[v.len() - 1]),
    }
}

fn eq_text(m: &Model, eq: &Eq) -> String {
    match eq.kind {
        EqKind::Span(si) => {
            let s = &m.spans[si];
            let parts: Vec<String> = (s.from..s.to).map(|k| part_name(m, k)).collect();
            if parts.len() == 1 {
                format!("{} and its brace", parts[0])
            } else {
                format!("{} = {}", parts.join(" + "), span_name(m, s))
            }
        }
        EqKind::Ticks(a, b) => format!("{} = {}", part_name(m, a), part_name(m, b)),
    }
}

/// Why equation `eq` breaks the ones before it, with the numbers where they are known.
fn conflict(m: &Model, sys: &Sys, eq: &Eq, part_form: &dyn Fn(usize) -> Form, before: Option<Rat>) -> String {
    let letter = m.letter.map(|c| c.to_string()).unwrap_or_default();
    let pre = match (eq.lettered, before) {
        (true, Some(x)) => format!("With {letter} = {}, ", num(m, x)),
        _ => String::new(),
    };
    let val = |k: usize| {
        let f = part_form(k);
        sys.eval(&f.0, f.1)
    };
    // lengths keep the labels' unit
    let u = m.unit.as_deref().map(|u| format!(" {u}")).unwrap_or_default();
    let len = |r: Rat| format!("{}{u}", num(m, r));
    match eq.kind {
        EqKind::Span(si) => {
            let s = &m.spans[si];
            let parts: Vec<String> = (s.from..s.to).map(|k| part_name(m, k)).collect();
            let lhs = (s.from..s.to).try_fold(zero(), |acc, k| acc.add(val(k)?));
            let rhs = match &s.lab {
                Lab::Val(v) => {
                    let mut co = vec![zero(); sys.nv];
                    if let (true, Some(_)) = (sys.nv > 0 && m.letter.is_some(), v.letter) {
                        co[0] = v.a;
                    }
                    sys.eval(&co, v.b)
                }
                _ => None,
            };
            // the brace's label as typed: with its unit only if it was given one
            let said = |r: Rat| if split_unit(&s.raw).1.is_some() { len(r) } else { num(m, r) };
            match (lhs, rhs) {
                (Some(l), Some(r)) if parts.len() == 1 => cap(&format!("{pre}{} is {} but the brace under it is labeled {}: the labels don’t agree.", parts[0], len(l), said(r))),
                (Some(l), Some(r)) => cap(&format!("{pre}{} = {} but {} = {}: the labels don’t agree.", parts.join(" + "), len(l), span_name(m, s), said(r))),
                _ => format!("{} doesn’t agree with the other labels.", cap(&eq_text(m, eq))),
            }
        }
        EqKind::Ticks(a, b) => match (val(a), val(b)) {
            (Some(x), Some(y)) => cap(&format!("{pre}{} = {} but {} = {}, though they have the same ticks: the labels don’t agree.", part_name(m, a), len(x), part_name(m, b), len(y))),
            _ => format!("{} (the same ticks) doesn’t agree with the other labels.", eq_text(m, eq)),
        },
    }
}

/// The check's solution line: x = 6 · AB = 13 · BC = 14 · AC = 27.
pub fn solution(m: &Model) -> Vec<String> {
    let mut v = Vec::new();
    if let (Some(c), Some(x)) = (m.letter, m.letter_val) {
        v.push(format!("{c} = {}", num(m, x)));
    }
    let u = m.unit.as_deref().map(|u| format!(" {u}")).unwrap_or_default();
    for (k, l) in m.part_len.iter().enumerate() {
        if let Some(l) = l {
            v.push(format!("{} = {}{u}", part_name(m, k), num(m, *l)));
        }
    }
    let mut seen: Vec<String> = Vec::new();
    for (si, s) in m.spans.iter().enumerate() {
        let name = span_name(m, s);
        if s.to - s.from < 2 || seen.contains(&name) {
            continue;
        }
        if let Some(l) = m.span_len[si] {
            v.push(format!("{name} = {}{u}", num(m, l)));
            seen.push(name);
        }
    }
    v
}

/// What a blank line's key copy writes: the worked-out length.
fn blank_value(m: &Model, len: Option<Rat>) -> Option<String> {
    let u = m.unit.as_deref().map(|u| format!(" {u}")).unwrap_or_default();
    len.filter(|l| l.n > 0).map(|l| format!("{}{u}", num(m, l)))
}

pub fn warnings(p: &Params) -> Vec<String> {
    let m = model(p);
    let mut v = m.notes.clone();
    v.extend(m.problems.iter().cloned());
    v
}

// ───────────────────────── editing points and spans ─────────────────────────

fn reset_slot(p: &mut Params, defs: &[ParamDef], fam: &str, slot: usize) {
    for d in defs.iter().filter(|d| item_of(d.key) == Some((fam, slot))) {
        p.0.insert(d.key.to_string(), d.default.clone());
    }
}

fn move_slot(p: &mut Params, defs: &[ParamDef], fam: &str, from: usize, to: usize) {
    for d in defs.iter().filter(|d| item_of(d.key) == Some((fam, from))) {
        let rest = d.key.split_once('_').map(|(_, r)| r).unwrap_or("");
        if let Some(v) = p.0.get(d.key).cloned() {
            p.0.insert(format!("{fam}{to}_{rest}"), v);
        }
    }
}

fn clear_part(p: &mut Params, slot: usize) {
    if (1..MAX_POINTS).contains(&slot) {
        p.set_s(MODE_KEYS[slot - 1], LABEL_MODES[0]);
        p.set_s(LAB_KEYS[slot - 1], "");
        p.set_s(TICK_KEYS[slot - 1], TICKS[0]);
        p.set_s(LEN_KEYS[slot - 1], "");
    }
}

/// Adds a point at the right end with the first free letter, and an empty part up to it.
pub fn add_point(p: &mut Params, defs: &[ParamDef]) -> bool {
    let n = n_points(p);
    if n >= MAX_POINTS {
        return false;
    }
    reset_slot(p, defs, "p", n + 1);
    clear_part(p, n);
    clear_part(p, n + 1);
    let used: Vec<String> = (0..n).map(|i| p.s(NAME_KEYS[i]).trim().to_string()).collect();
    let name = LETTERS.iter().find(|l| !used.iter().any(|u| u == *l)).copied().unwrap_or("");
    p.set_s(NAME_KEYS[n], name);
    p.set_i("n", n as i64 + 1);
    true
}

/// Removes point `i` (0-based), keeping at least two. The later points move up a slot with the
/// parts after them. The two parts beside the removed point become one part with no label text or
/// drawn size of its own (its length was the two together), keeping the later part's label style
/// and ticks. A brace that starts or ends at the
/// removed point is removed, and so is a crossing line through it; a brace over it gets shorter.
pub fn remove_point(p: &mut Params, defs: &[ParamDef], i: usize) -> bool {
    let n = n_points(p);
    if n <= MIN_POINTS || i >= n {
        return false;
    }
    let spans: Vec<(usize, usize)> = (0..n_spans(p)).map(|j| ((p.i(FROM_KEYS[j]).clamp(1, n as i64) - 1) as usize, (p.i(TO_KEYS[j]).clamp(1, n as i64) - 1) as usize)).collect();
    // the part after the removed point lends the joined part its label style and ticks
    let after = (i > 0 && i + 1 < n).then(|| (p.s(MODE_KEYS[i]).to_string(), p.s(TICK_KEYS[i]).to_string()));
    for slot in i + 1..n {
        move_slot(p, defs, "p", slot + 1, slot);
    }
    reset_slot(p, defs, "p", n);
    clear_part(p, n);
    // the part after the new last point is no longer drawn
    clear_part(p, n - 1);
    // the joined part (from the point before to the point after) keeps the later part's label
    // style and ticks, with no label text or drawn size (its length was the two together)
    if let Some((mode, ticks)) = after {
        clear_part(p, i);
        p.set_s(MODE_KEYS[i - 1], &mode);
        p.set_s(TICK_KEYS[i - 1], &ticks);
    }
    p.set_i("n", n as i64 - 1);
    let map = |k: usize| if k > i { k - 1 } else { k };
    let mut j = 0;
    let mut count = spans.len();
    for (a, b) in spans {
        let gone = a == i || b == i;
        let (a, b) = (map(a), map(b));
        if gone || a == b {
            // shift the later spans up over this one
            for s in j + 1..count {
                move_slot(p, defs, "s", s + 1, s);
            }
            reset_slot(p, defs, "s", count);
            count -= 1;
            continue;
        }
        p.set_i(FROM_KEYS[j], a as i64 + 1);
        p.set_i(TO_KEYS[j], b as i64 + 1);
        j += 1;
    }
    p.set_i("spans", count as i64);
    if let Some(c) = p.i("cross_at").checked_sub(1).filter(|c| *c >= 0).map(|c| c as usize) {
        if c == i {
            p.set_i("cross_at", 0);
        } else if c > i {
            p.set_i("cross_at", c as i64);
        }
    }
    true
}

/// Adds a brace: the whole line if no brace has it, else the first part with none.
pub fn add_span(p: &mut Params, defs: &[ParamDef]) -> bool {
    let k = n_spans(p);
    if k >= MAX_SPANS {
        return false;
    }
    let n = n_points(p);
    let used: Vec<(i64, i64)> = (0..k).map(|j| (p.i(FROM_KEYS[j]), p.i(TO_KEYS[j]))).collect();
    let mut cands = vec![(1, n as i64)];
    cands.extend((1..n as i64).map(|a| (a, a + 1)));
    for len in 2..n as i64 {
        cands.extend((1..=n as i64 - len).map(|a| (a, a + len)));
    }
    let (a, b) = cands.into_iter().find(|c| !used.contains(c)).unwrap_or((1, n as i64));
    reset_slot(p, defs, "s", k + 1);
    p.set_i(FROM_KEYS[k], a);
    p.set_i(TO_KEYS[k], b);
    p.set_s(SPAN_LAB_KEYS[k], "");
    p.set_i("spans", k as i64 + 1);
    true
}

/// Removes brace `j` (0-based); the later ones move up.
pub fn remove_span(p: &mut Params, defs: &[ParamDef], j: usize) -> bool {
    let k = n_spans(p);
    if j >= k {
        return false;
    }
    for s in j + 1..k {
        move_slot(p, defs, "s", s + 1, s);
    }
    reset_slot(p, defs, "s", k);
    p.set_i("spans", k as i64 - 1);
    true
}

// ───────────────────────── drawing ─────────────────────────

type Pt = (f64, f64);

/// Local coordinates (u along the line to the right, v down) turned onto the page.
struct Frame {
    c: f64,
    s: f64,
}

impl Frame {
    fn new(turn_deg: f64) -> Self {
        let a = -turn_deg.to_radians();
        Frame { c: a.cos(), s: a.sin() }
    }
    fn p(&self, q: Pt) -> Pt { (q.0 * self.c - q.1 * self.s, q.0 * self.s + q.1 * self.c) }
    /// How far an upright box of `w` × `h` reaches from its centre along local direction `d`.
    fn reach(&self, d: Pt, w: f64, h: f64) -> f64 {
        let s = self.p(d);
        s.0.abs() * w / 2.0 + s.1.abs() * h / 2.0
    }
    /// Centre of an upright box set `gap` beyond local point `at`, in local direction `d`.
    fn place(&self, at: Pt, d: Pt, gap: f64, w: f64, h: f64) -> Pt {
        let a = self.p(at);
        let s = self.p(d);
        let e = gap + self.reach(d, w, h);
        (a.0 + s.0 * e, a.1 + s.1 * e)
    }
}

#[derive(Clone, Debug)]
pub enum Prim {
    Line(Pt, Pt, f64),
    /// A brace: M then quadratic pieces, on the page.
    Brace(Vec<Pt>),
    Poly(Vec<Pt>, bool),
    Dot(Pt, f64),
    Text(TextBox),
    /// A blank line to write on, centred at the point.
    Blank(Pt, f64),
}

#[derive(Clone, Debug)]
pub struct TextBox {
    pub c: Pt,
    pub w: f64,
    pub h: f64,
    pub size: f64,
    pub runs: Vec<(String, bool)>,
    pub ink: &'static str,
    pub bold: bool,
    pub serif: bool,
    /// What it is, for tests: "part 0", "name 1", "span 0", "cross", "key".
    pub what: String,
}

impl TextBox {
    pub fn text(&self) -> String { self.runs.iter().map(|r| r.0.as_str()).collect() }
    pub fn rect(&self) -> (f64, f64, f64, f64) { (self.c.0 - self.w / 2.0, self.c.1 - self.h / 2.0, self.c.0 + self.w / 2.0, self.c.1 + self.h / 2.0) }
}

/// Typed text set for the page: − for minus, spaces around + and − between terms, · for ×, and
/// the unit (12 cm) after a space.
pub fn pretty(raw: &str) -> String {
    let (body, unit) = split_unit(raw);
    let mut out = String::new();
    let mut prev: Option<char> = None;
    let mut gap = false;
    for c in body.chars() {
        if c.is_whitespace() {
            gap = true;
            continue;
        }
        // the space of a mixed number (2 1/2) stays
        if gap && c.is_ascii_digit() && prev.is_some_and(|q| q.is_ascii_digit()) {
            out.push(' ');
        }
        gap = false;
        let c = match c {
            '-' | '\u{2013}' | '\u{2014}' => '\u{2212}',
            '*' | '\u{22c5}' | '\u{2219}' => '\u{b7}',
            c => c,
        };
        let binary = matches!(c, '+' | '\u{2212}') && prev.is_some_and(|q| q.is_alphanumeric() || q == ')' || q == '.' || q == ']' || q == '}');
        if binary {
            out.push(' ');
            out.push(c);
            out.push(' ');
        } else {
            out.push(c);
        }
        prev = Some(c);
    }
    if let Some(u) = unit {
        out.push(' ');
        out.push_str(&u);
    }
    out
}

/// Letters in italic (x, AB), except words of two or more lowercase letters (cm, units).
pub fn runs_of(s: &str) -> Vec<(String, bool)> {
    let cs: Vec<char> = s.chars().collect();
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let alpha = cs[i].is_alphabetic();
        let mut j = i;
        while j < cs.len() && cs[j].is_alphabetic() == alpha {
            j += 1;
        }
        let run: String = cs[i..j].iter().collect();
        let italic = alpha && !(run.chars().count() > 1 && run.chars().all(|c| c.is_lowercase()));
        match out.last_mut() {
            Some(last) if last.1 == italic => last.0.push_str(&run),
            _ => out.push((run, italic)),
        }
        i = j;
    }
    out
}

/// A typed label that isn't an expression (Not to scale, ?): words upright, a lone letter italic.
pub fn words_of(s: &str) -> Vec<(String, bool)> {
    runs_of(s).into_iter().flat_map(|(t, it)| if it && t.chars().count() > 1 { vec![(t, false)] } else { vec![(t, it)] }).fold(Vec::new(), |mut v: Vec<(String, bool)>, r| {
        match v.last_mut() {
            Some(l) if l.1 == r.1 => l.0.push_str(&r.0),
            _ => v.push(r),
        }
        v
    })
}

pub(crate) fn tw(s: &str, size: f64) -> f64 {
    let t: String = s.chars().map(|c| match c {
        '\u{2212}' => '+',
        '\u{b7}' => '.',
        '\u{2113}' => 'l',
        '\u{2019}' => '\'',
        '\u{201c}' | '\u{201d}' => '"',
        c => c,
    }).collect();
    crate::figures::stats::bar::tw(&t, size)
}

fn text_box(c: Pt, s: &str, size: f64, ink: &'static str, bold: bool, serif: bool, what: String) -> TextBox {
    TextBox { c, w: tw(s, size) * if serif { 0.94 } else { 1.0 }, h: size * 1.05, size, runs: if serif { runs_of(s) } else { vec![(s.to_string(), false)] }, ink, bold, serif, what }
}

/// Liang–Barsky: whether segment a-b passes through the box (inflated by `pad`).
pub(crate) fn seg_hits(a: Pt, b: Pt, r: (f64, f64, f64, f64), pad: f64) -> bool {
    let (x0, y0, x1, y1) = (r.0 - pad, r.1 - pad, r.2 + pad, r.3 + pad);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (pp, q) in [(-dx, a.0 - x0), (dx, x1 - a.0), (-dy, a.1 - y0), (dy, y1 - a.1)] {
        if pp.abs() < 1e-12 {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / pp;
            if pp < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

pub struct Drawing {
    pub prims: Vec<Prim>,
    /// Point positions on the page.
    pub points: Vec<Pt>,
}

pub fn draw(p: &Params, m: &Model) -> Drawing {
    let key_on = p.b(SHOW_ANSWERS);
    let print = Fills::new(p).is_print();
    let key_ink: &'static str = if print { INK } else { KEY_INK };
    let fs = 16.0 * svg::label_factor(p.s("label_size"));
    let fr = Frame::new(p.f("turn").clamp(-90.0, 90.0));
    let n = m.names.len();
    let mut prims: Vec<Prim> = Vec::new();

    // what each part's label says (and how wide it is)
    struct PartLab {
        text: Option<String>,
        blank: bool,
        key: Option<String>,
    }
    let part_labs: Vec<PartLab> = m
        .parts
        .iter()
        .enumerate()
        .map(|(k, pt)| match &pt.lab {
            Lab::None => PartLab { text: None, blank: false, key: None },
            Lab::Blank => PartLab { text: None, blank: true, key: if key_on { blank_value(m, m.part_len[k]) } else { None } },
            Lab::Val(_) => PartLab { text: Some(pretty(&pt.raw)), blank: false, key: None },
            Lab::Text => PartLab { text: Some(pt.raw.clone()), blank: false, key: None },
        })
        .collect();
    let blank_w = fs * 2.6;
    let lab_size = |l: &PartLab| -> (f64, f64) {
        let w = match (&l.text, &l.key) {
            (Some(t), _) => tw(t, fs) * 0.94,
            (None, Some(k)) => (tw(k, fs) * 0.94).max(blank_w),
            (None, None) if l.blank => blank_w,
            _ => 0.0,
        };
        (w, fs * 1.05)
    };
    let name_size = |i: usize| (tw(&m.names[i], fs) * 0.94, fs * 1.05);

    // drawn lengths: typed, else worked out, else the others' average; none under a fifth of the longest
    let d: Vec<Option<f64>> = m.parts.iter().enumerate().map(|(k, pt)| pt.typed_len.or_else(|| m.part_len[k].filter(|l| l.n > 0).map(|l| l.f()))).collect();
    let known: Vec<f64> = d.iter().flatten().copied().collect();
    let fill = if known.is_empty() { 1.0 } else { known.iter().sum::<f64>() / known.len() as f64 };
    let mut d: Vec<f64> = d.into_iter().map(|x| x.unwrap_or(fill)).collect();
    let mx = d.iter().cloned().fold(0.0, f64::max);
    for x in &mut d {
        *x = x.max(mx / 5.0);
    }
    let total: f64 = d.iter().sum();
    // pixels per unit: a 460 px line, wider where a label or two names would not fit
    let mut scale = 460.0 / total;
    for k in 0..d.len() {
        let (lw, lh) = lab_size(&part_labs[k]);
        let lab_need = 2.0 * fr.reach((1.0, 0.0), lw, lh) + 12.0;
        let (a, b) = (name_size(k), name_size(k + 1));
        let name_need = fr.reach((1.0, 0.0), a.0, a.1) + fr.reach((1.0, 0.0), b.0, b.1) + 10.0;
        let tick_need = 4.5 * m.parts[k].ticks as f64 + 16.0;
        // a slanted crossing line: the part its upper half leans over needs room for its label
        // past the line (and past the line's name, when that sits as low as the label); the part
        // its lower half leans over needs room for the far point's name to clear the line
        let cross_need = match cross_at(p).filter(|c| *c < n) {
            Some(c) => {
                let th = p.f("cross_ang").clamp(20.0, 160.0).to_radians();
                let (cos, tan) = (th.cos(), th.tan().abs());
                let up = 64.0 + fs;
                let over = if cos > 1e-9 { c == k } else if cos < -1e-9 { c == k + 1 } else { false };
                let under = if cos > 1e-9 { c == k + 1 } else if cos < -1e-9 { c == k } else { false };
                let mut need: f64 = 0.0;
                if over && lw > 0.0 {
                    let gap = if m.parts.iter().any(|pt| pt.ticks > 0) { 10.0 } else { 6.0 };
                    let band = gap + 2.0 * fr.reach((0.0, -1.0), lw, lh) + 4.0;
                    let line_u = band / tan;
                    let name = clip(p.s("cross_name").trim(), MAX_NAME);
                    let (nw, nh) = (tw(&name, fs) * 0.94, fs * 1.05);
                    let du = (cos, -th.sin());
                    // how high the name's lower edge sits above the line
                    let name_low = (up + 4.0 + fr.reach(du, nw, nh)) * th.sin() - fr.reach((0.0, 1.0), nw, nh);
                    let name_u = if name.is_empty() || name_low > band { 0.0 } else { up * cos.abs() + 2.0 * fr.reach((1.0, 0.0), nw, nh) + 14.0 };
                    need = line_u.max(name_u) + 2.0 * fr.reach((1.0, 0.0), lw, lh) + 12.0;
                }
                if under {
                    let far = if cos > 1e-9 { k } else { k + 1 };
                    if !m.names[far].is_empty() {
                        let (nw, nh) = name_size(far);
                        let (rw, rv, pad) = (fr.reach((1.0, 0.0), nw, nh), fr.reach((0.0, 1.0), nw, nh), 8.0);
                        // the line passes below the name, or ends before reaching it
                        let below = (7.0 + 2.0 * rv + pad) / tan + rw + pad;
                        let short = up * cos.abs() + rw + pad;
                        need = need.max(below.min(short));
                    }
                }
                need
            }
            _ => 0.0,
        };
        scale = scale.max(lab_need.max(name_need).max(tick_need).max(cross_need) / d[k]);
    }
    let mut xs = vec![0.0];
    for k in 0..d.len() {
        xs.push(xs[k] + d[k] * scale);
    }
    let x_end = xs[n - 1];

    // the line and its ends
    let ext = 34.0;
    let (lo, hi) = (if p.s("left_end") != ENDS[0] { -ext } else { 0.0 }, if p.s("right_end") != ENDS[0] { x_end + ext } else { x_end });
    prims.push(Prim::Line(fr.p((lo, 0.0)), fr.p((hi, 0.0)), 2.0));
    let head = |prims: &mut Vec<Prim>, tip: Pt, dir: Pt, style: &str| {
        let nrm = (-dir.1, dir.0);
        let back = |k: f64, s: f64| fr.p((tip.0 - dir.0 * k + nrm.0 * s, tip.1 - dir.1 * k + nrm.1 * s));
        match style {
            "Arrow" => prims.push(Prim::Poly(vec![fr.p(tip), back(12.0, 5.5), back(12.0, -5.5)], true)),
            "Line arrow" => prims.push(Prim::Poly(vec![back(11.0, 6.5), fr.p(tip), back(11.0, -6.5)], false)),
            _ => {}
        }
    };
    head(&mut prims, (lo, 0.0), (-1.0, 0.0), p.s("left_end"));
    head(&mut prims, (hi, 0.0), (1.0, 0.0), p.s("right_end"));

    // ticks
    for (k, pt) in m.parts.iter().enumerate() {
        let um = (xs[k] + xs[k + 1]) / 2.0;
        let t = pt.ticks as f64;
        for j in 0..pt.ticks {
            let u = um + (j as f64 - (t - 1.0) / 2.0) * 4.5;
            prims.push(Prim::Line(fr.p((u, -6.5)), fr.p((u, 6.5)), 1.6));
        }
    }
    // dots
    for i in 0..n {
        if m.dots[i] {
            prims.push(Prim::Dot(fr.p((xs[i], 0.0)), 3.8));
        }
    }

    // the crossing line
    let cross = cross_at(p).filter(|c| *c < n);
    let theta = p.f("cross_ang").clamp(20.0, 160.0).to_radians();
    let du = (theta.cos(), -theta.sin());
    // a name beside the crossing line sits on the side its lower half leans away from
    let cross_side = if theta.cos() < -1e-9 { -1.0 } else { 1.0 };

    // point names under the line, clear of the line and of the crossing line and its arrowheads
    // (beside it at its point)
    let up = 64.0 + fs;
    let line_seg = (fr.p((lo, 0.0)), fr.p((hi, 0.0)));
    let cross_reach = cross.map(|c| (fr.p((xs[c] - du.0 * up, -du.1 * up)), fr.p((xs[c] + du.0 * up, du.1 * up))));
    // the right-angle mark, on the side of the crossing point the segment runs to
    let mark: Option<Vec<Pt>> = cross.filter(|_| p.b("cross_right") && (p.f("cross_ang") - 90.0).abs() < 1e-9).map(|c| {
        let (x, sz) = (xs[c], 10.0);
        let su = if c + 1 == n { -1.0 } else { 1.0 };
        vec![fr.p((x + su * sz, 0.0)), fr.p((x + su * sz, -sz)), fr.p((x, -sz))]
    });
    let mut name_reach: f64 = 0.0;
    let mut placed: Vec<(f64, f64, f64, f64)> = Vec::new();
    for i in 0..n {
        if m.names[i].is_empty() {
            continue;
        }
        let (w, h) = name_size(i);
        let s = cross_side;
        let dirs: Vec<Pt> = if cross == Some(i) { vec![(s * 0.8, 0.6), (s * 0.6, 0.8), (s, 0.0), (-s * 0.8, 0.6)] } else { vec![(0.0, 1.0), (0.6, 0.8), (-0.6, 0.8), (0.8, 0.6), (-0.8, 0.6)] };
        let clear = |c: Pt| {
            let r = (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
            // the line, its ticks and its end arrowheads
            let drawn = prims.iter().any(|q| match q {
                Prim::Line(a, b, _) => seg_hits(*a, *b, r, 2.0),
                Prim::Poly(v, _) => v.windows(2).chain(std::iter::once(&[v[v.len() - 1], v[0]][..])).any(|w| seg_hits(w[0], w[1], r, 2.0)),
                _ => false,
            });
            !drawn && !seg_hits(line_seg.0, line_seg.1, r, 2.0) && !cross_reach.is_some_and(|(a, b)| seg_hits(a, b, r, 6.5)) && !mark.as_ref().is_some_and(|v| v.windows(2).any(|w| seg_hits(w[0], w[1], r, 2.0)))
                && !placed.iter().any(|o| r.0 < o.2 + 2.0 && o.0 < r.2 + 2.0 && r.1 < o.3 + 2.0 && o.1 < r.3 + 2.0)
        };
        let mut pick = None;
        'find: for gap in [7.0, 11.0, 16.0, 22.0, 30.0] {
            for d in &dirs {
                let c = fr.place((xs[i], 0.0), *d, gap, w, h);
                if clear(c) {
                    pick = Some((c, *d, gap));
                    break 'find;
                }
            }
        }
        let (c, dir, gap) = pick.unwrap_or_else(|| (fr.place((xs[i], 0.0), dirs[0], 7.0, w, h), dirs[0], 7.0));
        // how far below the line the name reaches, in the line's own frame
        name_reach = name_reach.max(dir.1 * (gap + fr.reach(dir, w, h)) + fr.reach((0.0, 1.0), w, h));
        let tb = text_box(c, &m.names[i], fs, INK, false, true, format!("name {i}"));
        placed.push(tb.rect());
        prims.push(Prim::Text(tb));
    }

    // braces, stacked so overlapping ones don't collide
    let bh = 9.0;
    struct BraceLab {
        text: Option<String>,
        blank: bool,
        key: Option<String>,
    }
    let brace_labs: Vec<BraceLab> = m
        .spans
        .iter()
        .enumerate()
        .map(|(si, s)| match &s.lab {
            Lab::None => BraceLab { text: None, blank: false, key: None },
            Lab::Blank => BraceLab { text: None, blank: true, key: if key_on { blank_value(m, m.span_len[si]) } else { None } },
            Lab::Val(_) => BraceLab { text: Some(pretty(&s.raw)), blank: false, key: None },
            Lab::Text => BraceLab { text: Some(s.raw.clone()), blank: false, key: None },
        })
        .collect();
    let blab_size = |l: &BraceLab| -> (f64, f64) {
        let w = match (&l.text, &l.key) {
            (Some(t), _) => tw(t, fs) * 0.94,
            (None, Some(k)) => (tw(k, fs) * 0.94).max(blank_w),
            (None, None) if l.blank => blank_w,
            _ => 0.0,
        };
        (w, fs * 1.05)
    };
    let mut order: Vec<usize> = (0..m.spans.len()).collect();
    order.sort_by_key(|&si| (m.spans[si].to - m.spans[si].from, m.spans[si].from));
    let mut level = vec![0usize; m.spans.len()];
    let mut taken: Vec<Vec<(f64, f64)>> = Vec::new();
    for &si in &order {
        let s = &m.spans[si];
        let (a, b) = (xs[s.from] + 3.0, xs[s.to] - 3.0);
        let mid = (a + b) / 2.0;
        let (w, h) = blab_size(&brace_labs[si]);
        let half = fr.reach((1.0, 0.0), w, h);
        let occ = (a.min(mid - half) - 3.0, b.max(mid + half) + 3.0);
        let mut l = 0;
        while taken.get(l).is_some_and(|v| v.iter().any(|o| o.0 < occ.1 && occ.0 < o.1)) {
            l += 1;
        }
        if taken.len() <= l {
            taken.resize(l + 1, Vec::new());
        }
        taken[l].push(occ);
        level[si] = l;
    }
    // the crossing line's lower half: as long as the upper one, but no less than 70% of it, the
    // braces going below it
    let min_down = up * 0.7;
    let cross_low = cross.map(|_| theta.sin() * min_down + 12.0).unwrap_or(0.0);
    let top0 = (name_reach.max(7.0 + fs) + 8.0).max(cross_low);
    let mut tops: Vec<f64> = Vec::new();
    let mut t = top0;
    for l in 0..taken.len() {
        tops.push(t);
        let reach = (0..m.spans.len()).filter(|&si| level[si] == l).map(|si| {
            let (w, h) = blab_size(&brace_labs[si]);
            2.0 * fr.reach((0.0, 1.0), w, h)
        });
        t += bh + 4.0 + reach.fold(fs * 0.6, f64::max) + 10.0;
    }
    for (si, s) in m.spans.iter().enumerate() {
        let (a, b) = (xs[s.from] + 3.0, xs[s.to] - 3.0);
        let mid = (a + b) / 2.0;
        let t = tops[level[si]];
        let r = bh.min((b - a) / 4.0);
        let pts = [
            (a, t),
            (a, t + bh / 2.0),
            (a + r, t + bh / 2.0),
            (mid - r, t + bh / 2.0),
            (mid, t + bh / 2.0),
            (mid, t + bh),
            (mid, t + bh / 2.0),
            (mid + r, t + bh / 2.0),
            (b - r, t + bh / 2.0),
            (b, t + bh / 2.0),
            (b, t),
        ];
        prims.push(Prim::Brace(pts.iter().map(|q| fr.p(*q)).collect()));
        let l = &brace_labs[si];
        let (w, h) = blab_size(l);
        let c = fr.place((mid, t + bh), (0.0, 1.0), 4.0, w, h);
        let what = format!("span {}", s.slot);
        match (&l.text, &l.key, l.blank) {
            (Some(tx), _, _) => {
                let mut tb = text_box(c, tx, fs, INK, false, true, what);
                if !tx.is_empty() && read_lin(&split_unit(tx).0).is_err() {
                    // typed words, not an expression: only a lone letter is italic
                    tb.runs = words_of(tx);
                }
                prims.push(Prim::Text(tb));
            }
            (None, k, true) => {
                prims.push(Prim::Blank(c, blank_w));
                if let Some(k) = k {
                    prims.push(Prim::Text(text_box((c.0, c.1 - 2.0), k, fs, key_ink, true, true, what)));
                }
            }
            _ => {}
        }
    }

    // the crossing line, kept above the braces under its point
    let mut cross_seg: Option<(Pt, Pt)> = None;
    let mut cross_name_box: Option<(f64, f64, f64, f64)> = None;
    if let Some(c) = cross {
        let pc = (xs[c], 0.0);
        // the lower half stops above the braces
        let down = if m.spans.is_empty() { up } else { up.min((top0 - 5.0) / theta.sin()).max(min_down) };
        let a = (pc.0 - du.0 * down, pc.1 - du.1 * down);
        let b = (pc.0 + du.0 * up, pc.1 + du.1 * up);
        prims.push(Prim::Line(fr.p(a), fr.p(b), 1.8));
        for (tip, dir) in [(b, du), (a, (-du.0, -du.1))] {
            let nrm = (-dir.1, dir.0);
            let back = |k: f64, s: f64| fr.p((tip.0 - dir.0 * k + nrm.0 * s, tip.1 - dir.1 * k + nrm.1 * s));
            prims.push(Prim::Poly(vec![fr.p(tip), back(11.0, 5.0), back(11.0, -5.0)], true));
        }
        cross_seg = Some((fr.p(a), fr.p(b)));
        let name = clip(p.s("cross_name").trim(), MAX_NAME);
        if !name.is_empty() {
            let (w, h) = (tw(&name, fs) * 0.94, fs * 1.05);
            // past the arrowhead at its upper end, where nothing else is drawn
            let tb = text_box(fr.place(b, du, 4.0, w, h), &name, fs, INK, false, true, "cross".into());
            cross_name_box = Some(tb.rect());
            prims.push(Prim::Text(tb));
        }
        if let Some(v) = &mark {
            prims.push(Prim::Poly(v.clone(), false));
        }
    }

    let mut lab_rects: Vec<(f64, f64, f64, f64)> = Vec::new();
    // part labels above the line, slid along their part clear of the crossing line
    for (k, l) in part_labs.iter().enumerate() {
        let (w, h) = lab_size(l);
        if w <= 0.0 {
            continue;
        }
        // every part label on one line, clear of the ticks when there are any
        let gap = if m.parts.iter().any(|pt| pt.ticks > 0) { 10.0 } else { 6.0 };
        let um = (xs[k] + xs[k + 1]) / 2.0;
        let half = fr.reach((1.0, 0.0), w, h);
        let (umin, umax) = (xs[k] + half + 3.0, xs[k + 1] - half - 3.0);
        let at = |u: f64, lift: f64| fr.place((u, 0.0), (0.0, -1.0), gap + lift, w, h);
        let hits = |c: Pt| {
            let r = (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
            cross_seg.is_some_and(|(a, b)| seg_hits(a, b, r, 3.0))
                || cross_name_box.is_some_and(|o| r.0 < o.2 + 3.0 && o.0 < r.2 + 3.0 && r.1 < o.3 + 3.0 && o.1 < r.3 + 3.0)
                || mark.as_ref().is_some_and(|v| v.windows(2).any(|w| seg_hits(w[0], w[1], r, 3.0)))
        };
        // clear of what is drawn: slide along the part (a little past its ends at most) and lift a
        // little, nearest first; else under the line, between the names; else where it was
        let near = |c: Pt| {
            let r = (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
            hits(c) || lab_rects.iter().any(|o: &(f64, f64, f64, f64)| r.0 < o.2 + 3.0 && o.0 < r.2 + 3.0 && r.1 < o.3 + 3.0 && o.1 < r.3 + 3.0)
        };
        let mut c = at(um, 0.0);
        if near(c) {
            let (lo_u, hi_u) = (umin.min(um) - half * 0.6, umax.max(um) + half * 0.6);
            let mut best: Option<(f64, Pt)> = None;
            for lift in [0.0, 3.0, 6.0, 9.0, 12.0] {
                let mut u = lo_u;
                while u <= hi_u {
                    let cost = (u - um).abs() + 3.0 * lift;
                    if best.is_none_or(|b| cost < b.0) {
                        let cand = at(u, lift);
                        if !near(cand) {
                            best = Some((cost, cand));
                        }
                    }
                    u += 2.0;
                }
            }
            let below = || {
                let cand = fr.place((um, 0.0), (0.0, 1.0), gap, w, h);
                let r = (cand.0 - w / 2.0, cand.1 - h / 2.0, cand.0 + w / 2.0, cand.1 + h / 2.0);
                let clear = !near(cand)
                    && !placed.iter().any(|o| r.0 < o.2 + 3.0 && o.0 < r.2 + 3.0 && r.1 < o.3 + 3.0 && o.1 < r.3 + 3.0)
                    && !prims.iter().any(|q| match q {
                        Prim::Line(a, b, _) => seg_hits(*a, *b, r, 2.0),
                        Prim::Brace(v) | Prim::Poly(v, _) => v.windows(2).any(|w| seg_hits(w[0], w[1], r, 2.0)),
                        _ => false,
                    });
                clear.then_some(cand)
            };
            c = best.map(|b| b.1).or_else(below).unwrap_or(c);
        }
        lab_rects.push((c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0));
        let what = format!("part {k}");
        match (&l.text, &l.key, l.blank) {
            (Some(tx), _, _) => {
                let mut tb = text_box(c, tx, fs, INK, false, true, what);
                if !tx.is_empty() && read_lin(&split_unit(tx).0).is_err() {
                    // typed words, not an expression: only a lone letter is italic
                    tb.runs = words_of(tx);
                }
                prims.push(Prim::Text(tb));
            }
            (None, k, true) => {
                prims.push(Prim::Blank(c, blank_w));
                if let Some(k) = k {
                    prims.push(Prim::Text(text_box((c.0, c.1 - 2.0), k, fs, key_ink, true, true, what)));
                }
            }
            _ => {}
        }
    }
    Drawing { prims, points: xs.iter().map(|x| fr.p((*x, 0.0))).collect() }
}

fn bounds(prims: &[Prim]) -> (f64, f64, f64, f64) {
    let mut b = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut add = |q: Pt, pad: f64| {
        b.0 = b.0.min(q.0 - pad);
        b.1 = b.1.min(q.1 - pad);
        b.2 = b.2.max(q.0 + pad);
        b.3 = b.3.max(q.1 + pad);
    };
    for pr in prims {
        match pr {
            Prim::Line(a, c, w) => {
                add(*a, *w);
                add(*c, *w);
            }
            Prim::Brace(v) | Prim::Poly(v, _) => v.iter().for_each(|q| add(*q, 1.5)),
            Prim::Dot(c, r) => add(*c, *r),
            Prim::Text(t) => {
                let r = t.rect();
                add((r.0, r.1), 1.0);
                add((r.2, r.3), 1.0);
            }
            Prim::Blank(c, w) => {
                add((c.0 - w / 2.0, c.1), 1.0);
                add((c.0 + w / 2.0, c.1 + 8.0), 1.0);
            }
        }
    }
    b
}

/// The key copy's lines under the figure: the equations, the solution, then any problem.
pub fn key_lines(m: &Model) -> Vec<(String, bool)> {
    let mut v: Vec<(String, bool)> = Vec::new();
    if !m.eqs.is_empty() {
        v.push((m.eqs.join(",   "), true));
    }
    let sol = solution(m);
    if !sol.is_empty() {
        v.push((sol.join(" \u{b7} "), true));
    }
    for pr in &m.problems {
        v.push((pr.clone(), false));
    }
    v
}

/// Breaks `s` into lines no wider than `w` at `sep` (or spaces, for a sentence).
fn wrap(s: &str, sep: &str, size: f64, w: f64) -> Vec<String> {
    let pieces: Vec<&str> = s.split(sep).collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for pc in pieces {
        let cand = if cur.is_empty() { pc.to_string() } else { format!("{cur}{sep}{pc}") };
        if !cur.is_empty() && tw(&cand, size) > w {
            out.push(std::mem::take(&mut cur));
            cur = pc.to_string();
        } else {
            cur = cand;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub(crate) fn text_svg(t: &TextBox, off: Pt) -> String {
    let wide = |c: char| (c as u32) > 0x24f && !(0x370..=0x3ff).contains(&(c as u32)) && !matches!(c, '\u{2212}' | '\u{2013}' | '\u{2014}' | '\u{2019}' | '\u{201c}' | '\u{201d}' | '\u{2026}');
    let fam = match t.serif {
        true if t.runs.iter().any(|(s, _)| s.chars().any(wide)) => WIDE_SERIF,
        true => MSERIF,
        false => SANS,
    };
    let mut body = String::new();
    for (s, it) in &t.runs {
        if *it {
            body.push_str(&format!(r#"<tspan font-style="italic">{}</tspan>"#, esc(s)));
        } else {
            body.push_str(&esc(s));
        }
    }
    let wt = if t.bold { r#" font-weight="bold""# } else { "" };
    format!(
        r##"<text x="{:.2}" y="{:.2}" font-size="{:.1}" text-anchor="middle" font-family="{fam}" fill="{}"{wt} stroke="#fff" stroke-width="{:.1}" stroke-linejoin="round" paint-order="stroke" xml:space="preserve">{body}</text>"##,
        t.c.0 + off.0,
        t.c.1 + off.1 + t.size * 0.34,
        t.size,
        t.ink,
        t.size * 0.22
    )
}

pub struct Rendered {
    pub svg: String,
    /// Every text box on the page (figure and key), in page coordinates.
    pub texts: Vec<TextBox>,
    pub size: (f64, f64),
}

pub fn render_full(p: &Params) -> Rendered {
    let m = model(p);
    let dr = draw(p, &m);
    let key_on = p.b(SHOW_ANSWERS);
    let print = Fills::new(p).is_print();
    let key_ink: &'static str = if print { INK } else { KEY_INK };
    let fs = 16.0 * svg::label_factor(p.s("label_size"));
    let (x0, y0, x1, y1) = bounds(&dr.prims);
    let margin = 18.0;
    let fig_w = x1 - x0;
    let ksize = fs * 0.92;
    let mut key: Vec<TextBox> = Vec::new();
    let mut y = y1 + 18.0;
    let wrap_w = fig_w.max(420.0);
    if key_on {
        for (line, math) in key_lines(&m) {
            let pieces = if math { wrap(&line, if line.contains(" \u{b7} ") { " \u{b7} " } else { ",   " }, ksize, wrap_w) } else { wrap(&line, " ", ksize * 0.92, wrap_w) };
            for pc in pieces {
                let size = if math { ksize } else { ksize * 0.92 };
                y += size * 0.6;
                key.push(text_box(((x0 + x1) / 2.0, y), &pc, size, key_ink, false, math, "key".into()));
                y += size * 0.75;
            }
        }
    }
    let key_w = key.iter().map(|t| t.w).fold(0.0, f64::max);
    let content_w = fig_w.max(key_w);
    let w = (content_w + 2.0 * margin).ceil();
    let h = ((if key.is_empty() { y1 } else { y }) - y0 + 2.0 * margin).ceil();
    let off = (margin - x0 + (content_w - fig_w) / 2.0, margin - y0);
    let mut doc = Doc::new(w, h);
    let q = |a: Pt| (a.0 + off.0, a.1 + off.1);
    let mut texts = Vec::new();
    for pr in &dr.prims {
        match pr {
            Prim::Line(a, b, sw) => {
                let (a, b) = (q(*a), q(*b));
                doc.line(a.0, a.1, b.0, b.1, INK, *sw);
            }
            Prim::Brace(v) => {
                let v: Vec<Pt> = v.iter().map(|x| q(*x)).collect();
                let d = format!(
                    "M{:.2},{:.2} Q{:.2},{:.2} {:.2},{:.2} L{:.2},{:.2} Q{:.2},{:.2} {:.2},{:.2} Q{:.2},{:.2} {:.2},{:.2} L{:.2},{:.2} Q{:.2},{:.2} {:.2},{:.2}",
                    v[0].0, v[0].1, v[1].0, v[1].1, v[2].0, v[2].1, v[3].0, v[3].1, v[4].0, v[4].1, v[5].0, v[5].1, v[6].0, v[6].1, v[7].0, v[7].1, v[8].0, v[8].1, v[9].0, v[9].1, v[10].0, v[10].1
                );
                doc.path(&d, "none", INK, 1.5);
            }
            Prim::Poly(v, fill) => {
                let v: Vec<Pt> = v.iter().map(|x| q(*x)).collect();
                if *fill {
                    doc.poly(&v, INK, INK, 1.0);
                } else {
                    doc.polyline(&v, INK, 1.8);
                }
            }
            Prim::Dot(c, r) => {
                let c = q(*c);
                doc.circle(c.0, c.1, *r, INK, "none", 0.0);
            }
            Prim::Blank(c, bw) => {
                let c = q(*c);
                let yl = c.1 + fs * 0.42;
                doc.line(c.0 - bw / 2.0, yl, c.0 + bw / 2.0, yl, INK, 1.3);
            }
            Prim::Text(t) => {
                doc.raw(&text_svg(t, off));
                let mut t = t.clone();
                t.c = q(t.c);
                texts.push(t);
            }
        }
    }
    for t in &key {
        doc.raw(&text_svg(t, off));
        let mut t = t.clone();
        t.c = q(t.c);
        texts.push(t);
    }
    doc.describe("Segment addition", &describe(p, &m, key_on));
    Rendered { svg: doc.finish(), texts, size: (w, h) }
}

fn render(p: &Params) -> String { render_full(p).svg }

fn label_words(m: &Model, lab: &Lab, raw: &str, len: Option<Rat>, key_on: bool) -> Option<String> {
    match lab {
        Lab::None => None,
        Lab::Blank => Some(match (key_on, blank_value(m, len)) {
            (true, Some(v)) => format!("a blank line, {v} on the key"),
            _ => "a blank line".into(),
        }),
        Lab::Val(_) | Lab::Text => Some(pretty(raw)),
    }
}

fn describe(p: &Params, m: &Model, key_on: bool) -> String {
    let kind = match (p.s("left_end") != ENDS[0], p.s("right_end") != ENDS[0]) {
        (false, false) => "Segment",
        (true, true) => "Line",
        _ => "Ray",
    };
    let n = m.names.len();
    let mut s = format!("{kind} {}{} with points {} in order.", m.call[0], m.call[n - 1], and_list(&m.call));
    for (k, pt) in m.parts.iter().enumerate() {
        let mut bits = Vec::new();
        if let Some(w) = label_words(m, &pt.lab, &pt.raw, m.part_len[k], key_on) {
            bits.push(format!("labeled {w}"));
        }
        if pt.ticks > 0 {
            bits.push(format!("{} tick{}", pt.ticks, if pt.ticks > 1 { "s" } else { "" }));
        }
        if !bits.is_empty() {
            s.push_str(&format!(" {} is {}.", part_name(m, k), bits.join(", with ")));
        }
    }
    for (si, sp) in m.spans.iter().enumerate() {
        match label_words(m, &sp.lab, &sp.raw, m.span_len[si], key_on) {
            Some(w) => s.push_str(&format!(" A brace under {} is labeled {w}.", span_name(m, sp))),
            None => s.push_str(&format!(" A brace under {}.", span_name(m, sp))),
        }
    }
    if let Some(c) = cross_at(p).filter(|c| *c < n) {
        let name = clip(p.s("cross_name").trim(), MAX_NAME);
        let ang = p.f("cross_ang");
        s.push_str(&format!(" A line{} crosses at {} at {}\u{b0}.", if name.is_empty() { String::new() } else { format!(" {name}") }, m.call[c], ang.round()));
    }
    if key_on {
        for (l, _) in key_lines(m) {
            s.push_str(&format!(" {l}."));
        }
    }
    s.replace("..", ".")
}

#[cfg(test)]
mod tests;
