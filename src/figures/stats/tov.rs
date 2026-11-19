//! Table of values: a rule in x (y = 2x + 1, y = x^2 − 3, y = 3(2)^x, y = sin x, y = 1/x) worked
//! out over x-values typed as a range (From, To, Step) or a list, or the pairs typed by hand, words
//! too. Worked-out values are exact where they can be (whole numbers, fractions in lowest terms,
//! multiples of π, sin 30° = 1/2) and rounded to the chosen places where they cannot (√2); where the
//! rule has no value the cell says undefined. x-values typed as fractions, decimals, multiples of π
//! or degrees are written that way, and degrees make the rule's trig read degrees.
//!
//! Any value can be a blank for students: click it on the figure (the `blanks` list holds the cells,
//! `y3` is the y of the third column), or blank every y at once; an empty typed cell is a blank too.
//! The student copy (Show answers off) leaves the blanks empty. The key copy writes each blank's
//! value in bold and lists the answers under the table, worded with the headings, with the rule when
//! it is a blank line above the table and whether the table is linear (its rate of change, or the
//! line typed pairs follow).
//!
//! Original design on the shared pieces (`Fills`, `show_answers`, `label_size`).
use crate::figures::interact::Handle;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "table-of-values";
/// Most columns (x-values or typed rows) drawn.
pub const MAX_COLS: usize = 25;
/// Longest typed cell, heading or title, in characters.
pub const MAX_CELL: usize = 40;
pub const MAX_TITLE: usize = 120;
pub const MAX_RULE: usize = 120;

pub const SOURCES: [&str; 2] = ["From a rule", "Typed"];
pub const XS: [&str; 2] = ["A range", "A list"];
pub const HEADS: [&str; 5] = ["x and y", "Input / Output", "x / f(x)", "n / aₙ", "Your own"];
pub const TITLES: [&str; 3] = ["None", "Text", "Blank line"];
pub const RULE_SHOWN: [&str; 3] = ["None", "Written", "Blank line"];
pub const AS: [&str; 2] = ["Fractions", "Decimals"];
pub const ROUNDS: [&str; 5] = ["Whole numbers", "1 decimal place", "2 decimal places", "3 decimal places", "4 decimal places"];
pub const RUNS: [&str; 2] = ["Across", "Down"];
pub const HEAD_LOOKS: [&str; 2] = ["Gray", "Plain"];

const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const INK: &str = "#111827";
/// The key copy's answers (Colour style); Print writes them black, in bold like every answer.
const KEY_INK: &str = "#1d4ed8";

pub fn def() -> FigureDef {
    FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Table of Values",
        blurb: "A table of x and y values worked out from a rule, or typed: exact fractions, π and degrees, undefined where the rule has no value, any value a blank for students, and the answers listed on the key.",
        params,
        render,
    }
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("source", "Values", &SOURCES, SOURCES[0]).hint("From a rule works out each y; Typed takes the pairs as you type them, words too"),
        ParamDef::text("rule", "Rule", "y = 2x + 1")
            .hint("A rule in x, like y = 2x + 1, y = x^2 − 3, y = 3(2)^x, y = sin x or y = 1/x. Type ^ for a power, sqrt for √ and pi for π. n works as the letter too, for a sequence (a_n = 3n + 2)"),
        ParamDef::choice("xs", "x-values from", &XS, XS[0]),
        ParamDef::text("from", "From", "-2").hint("A number, a fraction (1/2), a multiple of π (pi/6) or degrees (30°). The x-values are written the way you type them"),
        ParamDef::text("to", "To", "2"),
        ParamDef::text("step", "Step", "1").hint("Type 1/2 for fractions, 0.5 for decimals, pi/6 or 30° to step in π or degrees"),
        ParamDef::text("list", "x-values", "-2, 0, 1/2, 3").hint("Separated by commas: −2, 0, 1/2, 3. Fractions, decimals, π (pi/2) or degrees (90°) are written as typed"),
        ParamDef::long("typed", "Pairs", "1, 5\n2, 8\n3, 11\n4, 14")
            .hint("One pair per line: x, a comma (or a tab, from a spreadsheet paste), then y. Words work too. Leave a value out (1, or , 8) for a blank"),
        ParamDef::text("blanks", "Blanks", "").hint("Click a value on the figure to make it a blank, and click it again to fill it back in. Or list the cells: y3 is the y in the third column, x2 the x in the second"),
        ParamDef::bool("blank_y", "Blank every y", false),
        ParamDef::choice("heads", "Headings", &HEADS, HEADS[0]),
        ParamDef::text("head1", "First heading", "x").hint("Leave it empty for a blank heading"),
        ParamDef::text("head2", "Second heading", "y").hint("Leave it empty for a blank heading"),
        ParamDef::choice("title_kind", "Table title", &TITLES, TITLES[0]),
        ParamDef::text("title", "Title", ""),
        ParamDef::choice("show_rule", "The rule, above the table", &RULE_SHOWN, RULE_SHOWN[0]).hint("A blank line leaves the rule for students to find; the key copy writes it in"),
        ParamDef::choice("as", "Worked-out values as", &AS, AS[0]).hint("Fractions keeps whole numbers, fractions in lowest terms and multiples of π exact; the rest are rounded. A rule or x-values typed with decimals (y = 0.1x, a step of 0.5) are worked out in decimals"),
        ParamDef::choice("round", "Round the rest to", &ROUNDS, ROUNDS[2]),
        ParamDef::choice("runs", "The table runs", &RUNS, RUNS[0]).hint("Across in two rows, or down in two columns (a T-chart)"),
        ParamDef::choice("head_look", "Headings", &HEAD_LOOKS, HEAD_LOOKS[0]),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off makes the student copy: the blanks are left empty and the answers under the table are left out. Student + key saves both"),
    ]
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    let rule = p.s("source") != SOURCES[1];
    match (fig, key) {
        (ID, "rule" | "xs" | "as" | "round" | "blank_y") => rule,
        (ID, "from" | "to" | "step") => rule && p.s("xs") != XS[1],
        (ID, "list") => rule && p.s("xs") == XS[1],
        (ID, "typed") => !rule,
        (ID, "show_rule") => rule,
        (ID, "head1" | "head2") => p.s("heads") == HEADS[4],
        (ID, "title") => p.s("title_kind") == TITLES[1],
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        ID => vec![
            Example { name: "y = 2x + 1", params: vec![] },
            Example { name: "Complete the table for y = 2x + 1", params: vec![("blanks", s("y4, y5")), ("show_rule", s("Written"))] },
            Example { name: "A quadratic: y = x² − 3", params: vec![("rule", s("y = x^2 - 3")), ("from", s("-3")), ("to", s("3")), ("show_rule", s("Written"))] },
            Example {
                name: "Exponential: y = 3(2)ˣ",
                params: vec![("rule", s("y = 3(2)^x")), ("from", s("-2")), ("to", s("3")), ("show_rule", s("Written")), ("blank_y", Value::B(true))],
            },
            Example {
                name: "Fractions: y = x/2 + 1",
                params: vec![("rule", s("y = x/2 + 1")), ("from", s("0")), ("to", s("2")), ("step", s("1/2")), ("show_rule", s("Written")), ("blank_y", Value::B(true))],
            },
            Example { name: "y = sin x in degrees", params: vec![("rule", s("y = sin x")), ("from", s("0°")), ("to", s("180°")), ("step", s("30°")), ("show_rule", s("Written")), ("runs", s("Down"))] },
            Example {
                name: "Input-output table: find the rule",
                params: vec![("rule", s("y = 3x - 2")), ("xs", s("A list")), ("list", s("1, 2, 3, 5, 10")), ("heads", s("Input / Output")), ("show_rule", s("Blank line")), ("blanks", s("y4, y5"))],
            },
            Example {
                name: "Pattern table",
                params: vec![
                    ("source", s("Typed")),
                    ("typed", s("1, 5\n2, 8\n3, 11\n4,\n10,")),
                    ("heads", s("Your own")),
                    ("head1", s("Figure number")),
                    ("head2", s("Number of tiles")),
                    ("title_kind", s("Text")),
                    ("title", s("Growing tile pattern")),
                ],
            },
        ],
        _ => return None,
    })
}

// ───────────────────────── exact numbers ─────────────────────────

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// A fraction in lowest terms with a positive denominator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rat {
    pub n: i128,
    pub d: i128,
}

/// Numerators and denominators past this are treated as approximate (so no arithmetic overflows).
const BIG: i128 = 1_000_000_000_000_000_000_000_000_000_000;

impl Rat {
    fn new(n: i128, d: i128) -> Option<Rat> {
        if d == 0 {
            return None;
        }
        let g = gcd(n, d).max(1);
        let (mut n, mut d) = (n / g, d / g);
        if d < 0 {
            (n, d) = (-n, -d);
        }
        (n.abs() <= BIG && d <= BIG).then_some(Rat { n, d })
    }
    fn int(n: i128) -> Rat { Rat { n, d: 1 } }
    fn f(self) -> f64 { self.n as f64 / self.d as f64 }
    fn is_int(self) -> bool { self.d == 1 }
    fn add(self, o: Rat) -> Option<Rat> { Rat::new(self.n.checked_mul(o.d)?.checked_add(o.n.checked_mul(self.d)?)?, self.d.checked_mul(o.d)?) }
    fn mul(self, o: Rat) -> Option<Rat> { Rat::new(self.n.checked_mul(o.n)?, self.d.checked_mul(o.d)?) }
    fn neg(self) -> Rat { Rat { n: -self.n, d: self.d } }
    fn recip(self) -> Option<Rat> { Rat::new(self.d, self.n) }
    fn powi(self, k: i64) -> Option<Rat> {
        if k < 0 {
            return self.recip()?.powi(-k);
        }
        let (mut n, mut d) = (1i128, 1i128);
        for _ in 0..k {
            n = n.checked_mul(self.n)?;
            d = d.checked_mul(self.d)?;
            if n.abs() > BIG || d > BIG {
                return None;
            }
        }
        Rat::new(n, d)
    }
}

fn isqrt(v: i128) -> Option<i128> {
    if v < 0 {
        return None;
    }
    let r = (v as f64).sqrt().round() as i128;
    (r.checked_mul(r)? == v).then_some(r)
}

/// A worked-out value: exact (a fraction, or a fraction times π), approximate, or no value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum N {
    Q(Rat),
    Pi(Rat),
    F(f64),
    Undef,
}

impl N {
    fn q(n: i128) -> N { N::Q(Rat::int(n)) }
    pub fn f(self) -> f64 {
        match self {
            N::Q(r) => r.f(),
            N::Pi(r) => r.f() * std::f64::consts::PI,
            N::F(v) => v,
            N::Undef => f64::NAN,
        }
    }
    fn pi(r: Rat) -> N { if r.n == 0 { N::Q(r) } else { N::Pi(r) } }
    fn approx(v: f64) -> N { if v.is_finite() { N::F(v) } else { N::Undef } }
    fn is_zero(self) -> bool {
        match self {
            N::Q(r) | N::Pi(r) => r.n == 0,
            N::F(v) => v == 0.0,
            N::Undef => false,
        }
    }
    fn add(self, o: N) -> N {
        match (self, o) {
            (N::Undef, _) | (_, N::Undef) => N::Undef,
            (N::Q(a), N::Q(b)) => a.add(b).map(N::Q).unwrap_or_else(|| N::approx(a.f() + b.f())),
            (N::Pi(a), N::Pi(b)) => a.add(b).map(N::pi).unwrap_or_else(|| N::approx(self.f() + o.f())),
            (N::Q(z), x @ N::Pi(_)) | (x @ N::Pi(_), N::Q(z)) if z.n == 0 => x,
            _ => N::approx(self.f() + o.f()),
        }
    }
    fn neg(self) -> N {
        match self {
            N::Q(r) => N::Q(r.neg()),
            N::Pi(r) => N::Pi(r.neg()),
            N::F(v) => N::F(-v),
            N::Undef => N::Undef,
        }
    }
    fn mul(self, o: N) -> N {
        match (self, o) {
            (N::Undef, _) | (_, N::Undef) => N::Undef,
            (N::Q(a), N::Q(b)) => a.mul(b).map(N::Q).unwrap_or_else(|| N::approx(a.f() * b.f())),
            (N::Q(a), N::Pi(b)) | (N::Pi(b), N::Q(a)) => a.mul(b).map(N::pi).unwrap_or_else(|| N::approx(self.f() * o.f())),
            _ => N::approx(self.f() * o.f()),
        }
    }
    fn div(self, o: N) -> N {
        if o.is_zero() || matches!(o, N::Undef) || matches!(self, N::Undef) {
            return N::Undef;
        }
        match (self, o) {
            (N::Q(a), N::Q(b)) | (N::Pi(a), N::Pi(b)) => b.recip().and_then(|r| a.mul(r)).map(N::Q).unwrap_or_else(|| N::approx(self.f() / o.f())),
            (N::Pi(a), N::Q(b)) => b.recip().and_then(|r| a.mul(r)).map(N::pi).unwrap_or_else(|| N::approx(self.f() / o.f())),
            _ => N::approx(self.f() / o.f()),
        }
    }
    fn pow(self, e: N) -> N {
        if matches!(self, N::Undef) || matches!(e, N::Undef) {
            return N::Undef;
        }
        if let N::Q(k) = e {
            if k.is_int() && k.n.abs() <= 64 {
                match self {
                    N::Q(b) => {
                        if b.n == 0 && k.n < 0 {
                            return N::Undef;
                        }
                        if let Some(r) = b.powi(k.n as i64) {
                            return N::Q(r);
                        }
                    }
                    N::Pi(b) if k.n == 1 => return N::Pi(b),
                    _ => {}
                }
            }
            // a square root of a perfect square stays exact: 4^(1/2), (9/4)^(3/2)
            if k.d == 2 && k.n.abs() <= 64 {
                if let N::Q(b) = self {
                    if b.n < 0 {
                        return N::Undef;
                    }
                    if let (Some(a), Some(c)) = (isqrt(b.n), isqrt(b.d)) {
                        if let Some(r) = Rat::new(a, c).and_then(|r| r.powi(k.n as i64)) {
                            return N::Q(r);
                        }
                    }
                }
            }
            // an odd root of a negative number is real: (−8)^(1/3) = −2
            let b = self.f();
            if b < 0.0 && k.d % 2 == 1 {
                let m = (-b).powf(k.f());
                return N::approx(if k.n % 2 == 0 { m } else { -m });
            }
        }
        N::approx(self.f().powf(e.f()))
    }
}

/// An approximate result that is a simple fraction to within rounding error (sin²(π/4) = 0.5000000000000001)
/// is that fraction, so it is written exactly like its neighbours. A true irrational such as √2 is never
/// this close to a fraction with a denominator of 60 or less.
pub fn snap(v: N) -> N {
    let N::F(f) = v else { return v };
    if !f.is_finite() || f.abs() >= 1e9 {
        return v;
    }
    // only small denominators, and only below a million: a large irrational (e^10) is close to some
    // fraction with a big denominator by chance, never to one of these within rounding error
    if f.abs() >= 1e6 {
        return v;
    }
    // float noise around zero (1e-16) is zero; anything larger snaps only relative to its own size, so
    // e^-30 stays the tiny number it is
    if f.abs() < 1e-15 {
        return N::q(0);
    }
    let tol = 1e-12 * f.abs();
    for d in 1..=60i128 {
        let n = (f * d as f64).round();
        if (f - n / d as f64).abs() <= tol {
            return Rat::new(n as i128, d).map(N::Q).unwrap_or(v);
        }
    }
    v
}

/// sin, cos and tan of `a` radians, exact at the multiples of π/6 and π/4 where the value is rational.
fn trig(name: &str, a: N) -> N {
    let q = match a {
        N::Pi(r) => Some(r),
        N::Q(r) if r.n == 0 => Some(r),
        _ => None,
    };
    if let Some(q) = q {
        // q as a whole number of twelfths of π (π/6 = 2, π/4 = 3)
        if (12 % q.d) == 0 || q.n == 0 {
            let t = (q.n * (12 / q.d.max(1))).rem_euclid(24);
            let sin12 = |t: i128| -> Option<N> {
                match t {
                    0 | 12 => Some(N::q(0)),
                    2 | 10 => Some(N::Q(Rat { n: 1, d: 2 })),
                    6 => Some(N::q(1)),
                    14 | 22 => Some(N::Q(Rat { n: -1, d: 2 })),
                    18 => Some(N::q(-1)),
                    _ => None,
                }
            };
            let s = sin12(t);
            let c = sin12((t + 6).rem_euclid(24));
            match name {
                "sin" => {
                    if let Some(v) = s {
                        return v;
                    }
                }
                "cos" => {
                    if let Some(v) = c {
                        return v;
                    }
                }
                "tan" => {
                    match t {
                        0 | 12 => return N::q(0),
                        6 | 18 => return N::Undef,
                        3 | 15 => return N::q(1),
                        9 | 21 => return N::q(-1),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    let v = a.f();
    match name {
        "sin" => N::approx(v.sin()),
        "cos" => N::approx(v.cos()),
        _ => {
            if v.cos().abs() < 1e-12 { N::Undef } else { N::approx(v.tan()) }
        }
    }
}

/// A rule read into a tree once: the same tree works out the values and writes the rule above the
/// table, so what is written is always what is worked out.
#[derive(Clone, Debug, PartialEq)]
pub enum Ex {
    /// A number and its digits as typed.
    Num(Rat, String),
    Var(char),
    Pi,
    E,
    Neg(Box<Ex>),
    Add(Box<Ex>, Box<Ex>),
    Sub(Box<Ex>, Box<Ex>),
    /// Times, and whether a sign was typed (2*x) or not (2x).
    Mul(Box<Ex>, Box<Ex>, bool),
    Div(Box<Ex>, Box<Ex>),
    Pow(Box<Ex>, Box<Ex>),
    /// A function: its name, its argument, whether the argument was in brackets, and a power on the
    /// name (sin^2 x is the square of sin x).
    Func(&'static str, Box<Ex>, bool, Option<Box<Ex>>),
    Paren(Box<Ex>),
}

/// Longest names first, so "sqrtx" is sqrt then x and "sec" is not s, e, c.
const FUNCS: [&str; 13] = ["sqrt", "cbrt", "root", "abs", "sin", "cos", "tan", "sec", "csc", "cot", "log", "exp", "ln"];

/// Reads typed math into an `Ex`.
struct Rd<'a> {
    s: &'a [char],
    i: usize,
    /// The letter x (or n) is allowed.
    var: bool,
}

impl<'a> Rd<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i] == ' ' {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<char> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn expr(&mut self, d: u32) -> Option<Ex> {
        if d > 60 {
            return None;
        }
        let mut v = self.term(d)?;
        loop {
            match self.peek() {
                Some('+') => {
                    self.i += 1;
                    v = Ex::Add(Box::new(v), Box::new(self.term(d)?));
                }
                Some('-') => {
                    self.i += 1;
                    v = Ex::Sub(Box::new(v), Box::new(self.term(d)?));
                }
                _ => return Some(v),
            }
        }
    }
    fn term(&mut self, d: u32) -> Option<Ex> {
        let mut v = self.unary(d)?;
        loop {
            match self.peek() {
                Some('*') => {
                    self.i += 1;
                    v = Ex::Mul(Box::new(v), Box::new(self.unary(d)?), true);
                }
                Some('/') => {
                    self.i += 1;
                    v = Ex::Div(Box::new(v), Box::new(self.unary(d)?));
                }
                Some(c) if c.is_ascii_digit() || c == '.' || c == '(' || c.is_alphabetic() => v = Ex::Mul(Box::new(v), Box::new(self.power(d)?), false),
                _ => return Some(v),
            }
        }
    }
    fn unary(&mut self, d: u32) -> Option<Ex> {
        if d > 200 {
            return None;
        }
        match self.peek() {
            Some('-') => {
                self.i += 1;
                Some(Ex::Neg(Box::new(self.unary(d + 1)?)))
            }
            Some('+') => {
                self.i += 1;
                self.unary(d + 1)
            }
            _ => self.power(d),
        }
    }
    /// After ^: the exponent. A number straight after the ^ takes the letter right after it (e^2x is
    /// e to the 2x); a space ends the power (x^2 x is x^2 times x).
    fn exponent(&mut self, d: u32, absorb: bool) -> Option<Ex> {
        let mut e = self.unary(d + 1)?;
        let digit_before = self.i > 0 && self.s[self.i - 1].is_ascii_digit();
        if absorb && self.var && digit_before && matches!(self.s.get(self.i), Some('x' | 'n')) {
            let c = self.s[self.i];
            self.i += 1;
            e = Ex::Mul(Box::new(e), Box::new(Ex::Var(c)), false);
        }
        Some(e)
    }
    fn power(&mut self, d: u32) -> Option<Ex> {
        let b = self.atom(d)?;
        if self.peek() == Some('^') {
            self.i += 1;
            let e = self.exponent(d, true)?;
            return Some(Ex::Pow(Box::new(b), Box::new(e)));
        }
        Some(b)
    }
    /// A function's argument: only the bracket (sin(x)^2 squares the sine), or the next factor
    /// without brackets (sin x, sin 2x, sqrt x).
    fn arg(&mut self, d: u32) -> Option<(Ex, bool)> {
        if self.peek()? == '(' {
            return Some((self.atom(d + 1)?, true));
        }
        let mut v = self.power(d + 1)?;
        while let Some(c) = self.peek() {
            if (c == 'x' || c == 'n') && self.var {
                let save = self.i;
                match self.power(d + 1) {
                    Some(w) => v = Ex::Mul(Box::new(v), Box::new(w), false),
                    None => {
                        self.i = save;
                        break;
                    }
                }
            } else {
                break;
            }
        }
        Some((v, false))
    }
    fn atom(&mut self, d: u32) -> Option<Ex> {
        if d > 60 {
            return None;
        }
        match self.peek()? {
            '(' => {
                self.i += 1;
                let v = self.expr(d + 1)?;
                if self.peek()? != ')' {
                    return None;
                }
                self.i += 1;
                Some(Ex::Paren(Box::new(v)))
            }
            c if c.is_ascii_digit() || c == '.' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == '.') {
                    self.i += 1;
                }
                let t: String = self.s[st..self.i].iter().collect();
                // 1e5, 2.5e-3: a number straight followed by e and digits is in powers of ten
                if matches!(self.s.get(self.i), Some('e')) {
                    let mut j = self.i + 1;
                    let neg = matches!(self.s.get(j), Some('-'));
                    if matches!(self.s.get(j), Some('-' | '+')) {
                        j += 1;
                    }
                    let ds = j;
                    while j < self.s.len() && self.s[j].is_ascii_digit() {
                        j += 1;
                    }
                    if j > ds && j - ds <= 3 {
                        let k: i64 = self.s[ds..j].iter().collect::<String>().parse().ok()?;
                        let typed: String = self.s[st..j].iter().collect();
                        self.i = j;
                        let ten = Rat::int(10).powi(if neg { -k } else { k })?;
                        return Some(Ex::Num(decimal(&t)?.mul(ten)?, typed));
                    }
                }
                Some(Ex::Num(decimal(&t)?, t))
            }
            c if c.is_alphabetic() => {
                let st = self.i;
                let mut end = st;
                while end < self.s.len() && self.s[end].is_ascii_alphabetic() {
                    end += 1;
                }
                let name: String = self.s[st..end].iter().collect();
                if name.starts_with("pi") {
                    self.i = st + 2;
                    return Some(Ex::Pi);
                }
                // a function name, alone or run into what follows (sinx, sqrtx)
                if let Some(f) = FUNCS.iter().find(|f| name.starts_with(**f)) {
                    self.i = st + f.len();
                    // a power on the name: sin^2 x (a negative one would be an inverse, which this does not read)
                    let np = if self.peek() == Some('^') {
                        self.i += 1;
                        // sin^2x: the x is the sine's, not the power's
                        let e = self.exponent(d, false)?;
                        if matches!(e, Ex::Neg(_)) {
                            return None;
                        }
                        Some(Box::new(e))
                    } else {
                        None
                    };
                    let (a, br) = self.arg(d + 1)?;
                    return Some(Ex::Func(f, Box::new(a), br, np));
                }
                // a run of single letters is split from the left: 2xe, xn
                self.i = st + 1;
                match name.chars().next()? {
                    c @ ('x' | 'n') if self.var => Some(Ex::Var(c)),
                    'e' => Some(Ex::E),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// Reads typed math (a rule's right-hand side, or a number) into a tree; None when it cannot be read.
pub fn read_ex(t: &str, var: bool) -> Option<Ex> {
    let cs = norm(t);
    if cs.len() > 400 {
        return None;
    }
    let mut r = Rd { s: &cs, i: 0, var };
    let v = r.expr(0)?;
    r.ws();
    (r.i == cs.len()).then_some(v)
}

/// An angle for trig: radians, or degrees read as a fraction of π when the x-values are in degrees.
fn angle(a: N, deg: bool) -> N {
    if !deg {
        return a;
    }
    match a {
        N::Q(r) => Rat::new(r.n, r.d.saturating_mul(180)).map(N::pi).unwrap_or_else(|| N::approx(a.f().to_radians())),
        _ => N::approx(a.f().to_radians()),
    }
}

fn func(name: &str, a: N, deg: bool) -> N {
    if matches!(a, N::Undef) {
        return N::Undef;
    }
    match name {
        "sqrt" => {
            if a.f() < 0.0 {
                N::Undef
            } else {
                a.pow(N::Q(Rat { n: 1, d: 2 }))
            }
        }
        "cbrt" | "root" => a.pow(N::Q(Rat { n: 1, d: 3 })),
        "abs" => match a {
            N::Q(r) => N::Q(Rat { n: r.n.abs(), d: r.d }),
            N::Pi(r) => N::Pi(Rat { n: r.n.abs(), d: r.d }),
            _ => N::approx(a.f().abs()),
        },
        "sin" | "cos" | "tan" => trig(name, angle(a, deg)),
        "sec" => N::q(1).div(trig("cos", angle(a, deg))),
        "csc" => N::q(1).div(trig("sin", angle(a, deg))),
        "cot" => match trig("tan", angle(a, deg)) {
            // cot is 0 where tan has no value (cos = 0)
            N::Undef => {
                if trig("cos", angle(a, deg)).is_zero() {
                    N::q(0)
                } else {
                    N::Undef
                }
            }
            t => N::q(1).div(t),
        },
        "ln" => {
            if a.f() <= 0.0 {
                N::Undef
            } else if a == N::q(1) {
                N::q(0)
            } else {
                N::approx(a.f().ln())
            }
        }
        "log" => {
            let v = a.f();
            if v <= 0.0 {
                return N::Undef;
            }
            // log of a power of ten is exact
            if let N::Q(r) = a {
                for k in -18i32..=18 {
                    let p = if k >= 0 { Rat::new(10i128.pow(k as u32), 1) } else { Rat::new(1, 10i128.pow((-k) as u32)) };
                    if p == Some(r) {
                        return N::q(k as i128);
                    }
                }
            }
            N::approx(v.log10())
        }
        "exp" => {
            if a.is_zero() {
                N::q(1)
            } else {
                N::approx(a.f().exp())
            }
        }
        _ => N::Undef,
    }
}

/// Works the tree out at x.
pub fn eval(e: &Ex, x: N, deg: bool) -> N {
    match e {
        Ex::Num(r, _) => N::Q(*r),
        Ex::Var(_) => x,
        Ex::Pi => N::Pi(Rat::int(1)),
        Ex::E => N::approx(std::f64::consts::E),
        Ex::Neg(a) => eval(a, x, deg).neg(),
        Ex::Add(a, b) => eval(a, x, deg).add(eval(b, x, deg)),
        Ex::Sub(a, b) => eval(a, x, deg).add(eval(b, x, deg).neg()),
        Ex::Mul(a, b, _) => eval(a, x, deg).mul(eval(b, x, deg)),
        Ex::Div(a, b) => eval(a, x, deg).div(eval(b, x, deg)),
        Ex::Pow(a, b) => eval(a, x, deg).pow(eval(b, x, deg)),
        Ex::Paren(a) => eval(a, x, deg),
        Ex::Func(name, a, _, np) => {
            let v = func(name, eval(a, x, deg), deg);
            match np {
                Some(k) => v.pow(eval(k, x, deg)),
                None => v,
            }
        }
    }
}

/// One run of a written rule, drawn as a text element of its own: plain text at `scale` of the size,
/// raised by `rise` of the size (a power, a power in a power, a subscript below), or a radical sign.
/// No tspans: the renderer fits each run to its measured width with `textLength`, which lays a run
/// out wrongly when it holds shifted tspans, and a √ is in a serif face (the sans faces have none).
#[derive(Clone, Debug, PartialEq)]
pub enum Run {
    Text { text: String, em: f64, scale: f64, rise: f64 },
    Radical { scale: f64, rise: f64 },
}

/// Builds the runs of a written rule.
struct Wr {
    runs: Vec<Run>,
    text: String,
    em: f64,
    /// Size and rise of the text now being written (a power is 70% of its base, raised 0.42 of it).
    scale: f64,
    rise: f64,
}

impl Wr {
    fn new() -> Wr { Wr { runs: Vec::new(), text: String::new(), em: 0.0, scale: 1.0, rise: 0.0 } }
    fn text(&mut self, s: &str) {
        self.text.push_str(s);
        self.em += em_of(s) * self.scale;
    }
    fn flush(&mut self) {
        if !self.text.is_empty() {
            self.runs.push(Run::Text { text: std::mem::take(&mut self.text), em: self.em, scale: self.scale, rise: self.rise });
            self.em = 0.0;
        }
    }
    fn radical(&mut self) {
        self.flush();
        self.runs.push(Run::Radical { scale: self.scale, rise: self.rise });
    }
    fn shifted(&mut self, by: f64, f: impl FnOnce(&mut Wr)) {
        self.flush();
        let (scale, rise) = (self.scale, self.rise);
        self.rise += by * self.scale;
        self.scale *= 0.7;
        f(self);
        self.flush();
        self.scale = scale;
        self.rise = rise;
    }
    fn sup(&mut self, f: impl FnOnce(&mut Wr)) { self.shifted(0.42, f) }
    fn sub(&mut self, f: impl FnOnce(&mut Wr)) { self.shifted(-0.18, f) }
    fn finish(mut self) -> Vec<Run> {
        self.flush();
        self.runs
    }
}

/// Writes the tree the way it was typed, with − for minus, superscript powers, √ and π.
fn write_ex(e: &Ex, w: &mut Wr) {
    match e {
        Ex::Num(_, t) => w.text(t),
        Ex::Var(c) => w.text(&c.to_string()),
        Ex::Pi => w.text("\u{3c0}"),
        Ex::E => w.text("e"),
        Ex::Neg(a) => {
            w.text("\u{2212}");
            write_ex(a, w);
        }
        Ex::Add(a, b) => {
            write_ex(a, w);
            w.text(" + ");
            write_ex(b, w);
        }
        Ex::Sub(a, b) => {
            write_ex(a, w);
            w.text(" \u{2212} ");
            write_ex(b, w);
        }
        Ex::Mul(a, b, explicit) => {
            write_ex(a, w);
            if *explicit {
                w.text(" \u{b7} ");
            } else if matches!(**b, Ex::Num(..)) {
                // x2 or (x + 1)3: a number after something else, typed with nothing between, is a product
                w.text(" \u{b7} ");
            } else if matches!(**b, Ex::Func(name, ..) if name != "sqrt" && name != "abs") {
                // 2 sin x
                w.text(" ");
            }
            write_ex(b, w);
        }
        Ex::Div(a, b) => {
            write_ex(a, w);
            w.text("/");
            write_ex(b, w);
        }
        Ex::Pow(a, b) => {
            write_ex(a, w);
            // a bracketed exponent is grouped by the superscript itself
            let inner = match &**b {
                Ex::Paren(i) => &**i,
                other => other,
            };
            w.sup(|w| write_ex(inner, w));
        }
        Ex::Paren(a) => {
            w.text("(");
            write_ex(a, w);
            w.text(")");
        }
        Ex::Func(name, a, br, np) => {
            match *name {
                "sqrt" => w.radical(),
                "abs" => w.text("|"),
                n => w.text(n),
            }
            if let Some(k) = np {
                w.sup(|w| write_ex(k, w));
            }
            let inner = match (&**a, *name) {
                (Ex::Paren(i), "abs") => &**i,
                (other, _) => other,
            };
            if *name == "abs" {
                write_ex(inner, w);
                w.text("|");
            } else if *br {
                write_ex(a, w);
            } else {
                if *name != "sqrt" {
                    w.text(" ");
                }
                write_ex(a, w);
            }
        }
    }
}

/// A decimal like 2, 0.5 or .25 as an exact fraction.
fn decimal(t: &str) -> Option<Rat> {
    if t.is_empty() || t == "." || t.matches('.').count() > 1 {
        return None;
    }
    let (a, b) = t.split_once('.').unwrap_or((t, ""));
    if a.len() + b.len() > 30 {
        return None;
    }
    let digits = format!("{a}{b}");
    let n: i128 = digits.parse().ok()?;
    Rat::new(n, 10i128.checked_pow(b.len() as u32)?)
}

/// Typed math normalised for the reader: unicode minus and times, π, √, superscript digits, |a|.
fn norm(t: &str) -> Vec<char> {
    let mut out = String::new();
    let mut open = false;
    for c in t.to_lowercase().chars() {
        match c {
            '\u{2212}' | '\u{2013}' => out.push('-'),
            '\u{d7}' | '\u{b7}' | '\u{22c5}' => out.push('*'),
            '\u{f7}' => out.push('/'),
            '\u{3c0}' => out.push_str("pi"),
            '\u{221a}' => out.push_str("sqrt"),
            '\u{b2}' => out.push_str("^2"),
            '\u{b3}' => out.push_str("^3"),
            '\u{2099}' => {}
            '|' => {
                out.push_str(if open { ")" } else { "abs(" });
                open = !open;
            }
            '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    out.chars().collect()
}

/// The rule split into its left side as typed (y, f(x), a_n; empty when none) and what it equals.
pub fn split_rule(rule: &str) -> (String, String) {
    match rule.split_once('=') {
        Some((l, r)) => (l.trim().to_string(), r.trim().to_string()),
        None => (String::new(), rule.trim().to_string()),
    }
}

/// The rule's value at x, or None when the rule cannot be read.
pub fn eval_rule(rhs: &str, x: N, deg: bool) -> Option<N> { read_ex(rhs, true).map(|e| eval(&e, x, deg)) }

// ───────────────────────── x-values ─────────────────────────

/// How an x-value is written: as typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Style {
    Int,
    Dec,
    Frac,
    Pi,
    Deg,
}

/// One typed x-value: its value and how it is written. A mixed number (1 1/2) is a fraction.
pub fn read_x(t: &str) -> Option<(N, Style)> {
    let t = t.trim();
    if t.is_empty() {
        return None;
    }
    let t = t.replace('\u{ba}', "\u{b0}");
    let t = t.as_str();
    let deg = t.contains('°') || t.to_lowercase().ends_with("deg");
    let body = t.replace('°', "").to_lowercase().replace("deg", "");
    let style = if deg {
        Style::Deg
    } else if body.contains("pi") || body.contains('\u{3c0}') {
        Style::Pi
    } else if body.contains('/') {
        Style::Frac
    } else if body.contains('.') {
        Style::Dec
    } else {
        Style::Int
    };
    // a mixed number: whole, space, fraction
    let parts: Vec<&str> = body.split_whitespace().collect();
    // "5 6" is two values with a comma missing, not 30
    let plain = |s: &str| !s.is_empty() && s.trim_start_matches(['-', '\u{2212}']).chars().all(|c| c.is_ascii_digit() || c == '.');
    if parts.windows(2).any(|w| plain(w[0]) && plain(w[1])) {
        return None;
    }
    if parts.len() == 2 && parts[1].contains('/') && !parts[0].contains('/') {
        let w = eval_const(parts[0])?;
        let f = eval_const(parts[1])?;
        let v = if w.f() < 0.0 || parts[0].starts_with('-') || parts[0].starts_with('\u{2212}') { w.add(f.neg()) } else { w.add(f) };
        return Some((v, Style::Frac));
    }
    let v = eval_const(&body)?;
    if matches!(v, N::Undef) {
        return None;
    }
    Some((v, style))
}

fn eval_const(t: &str) -> Option<N> { read_ex(t, false).map(|e| eval(&e, N::q(0), false)) }

/// The list box split into its values: by commas or semicolons, else by spaces.
fn list_items(t: &str) -> Vec<String> {
    if t.contains(',') || t.contains(';') {
        t.split([',', ';']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    } else {
        t.split_whitespace().map(|s| s.to_string()).collect()
    }
}

// ───────────────────────── writing values ─────────────────────────

/// A cell's text: plain, or a stacked fraction (with a sign in front).
#[derive(Clone, Debug, PartialEq)]
pub enum Txt {
    Plain(String),
    Frac { neg: bool, num: String, den: String },
}

impl Txt {
    /// The same value on one line, for the answer list and the description: 3/2, −π/4.
    pub fn inline(&self) -> String {
        match self {
            Txt::Plain(s) => s.clone(),
            Txt::Frac { neg, num, den } => format!("{}{num}/{den}", if *neg { "\u{2212}" } else { "" }),
        }
    }
    fn empty() -> Txt { Txt::Plain(String::new()) }
}

fn minus(s: &str) -> String { s.replace('-', "\u{2212}") }

/// A number in powers of ten: 1.2158 × 10¹⁹.
pub fn sci(v: f64) -> String {
    let e = v.abs().log10().floor() as i32;
    let mant = v / 10f64.powi(e);
    let sup: String = e.to_string().chars().map(|c| match c {
        '0' => '\u{2070}', '1' => '\u{b9}', '2' => '\u{b2}', '3' => '\u{b3}', '4' => '\u{2074}', '5' => '\u{2075}', '6' => '\u{2076}', '7' => '\u{2077}', '8' => '\u{2078}', '9' => '\u{2079}', _ => '\u{207b}',
    }).collect();
    minus(&format!("{} \u{d7} 10{sup}", round_str(mant, 4)))
}

/// A decimal rounded to `places` with a real minus sign. A value that was rounded keeps every place
/// (0.40 beside 0.43); one that needs fewer places is written without trailing zeros (0.5). An
/// approximate value past 10^15 is written as 1.2158 × 10¹⁹, since its last digits are not known.
pub fn round_str(v: f64, places: usize) -> String {
    // a double knows about 15 significant figures: past that, its last digits would be made up
    if v.is_finite() && v != 0.0 {
        let int_digits = (v.abs().log10().floor() as i64 + 1).max(1);
        if int_digits + places as i64 > 15 {
            return sci(v);
        }
    }
    let f = 10f64.powi(places as i32);
    let r = (v * f).round() / f;
    let rounded = (v - r).abs() > 1e-12 * v.abs();
    let mut s = format!("{r:.places$}");
    if s.contains('.') && !rounded {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    // a small negative rounded to nothing is unsigned: 0.00, never −0.00
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s.remove(0);
    }
    minus(&s)
}

fn rat_txt(r: Rat, pi: bool) -> Txt {
    let p = if pi { "\u{3c0}" } else { "" };
    let num = match (r.n.abs(), pi) {
        (1, true) => String::new(),
        (n, _) => n.to_string(),
    };
    if r.d == 1 {
        return Txt::Plain(format!("{}{}{p}", if r.n < 0 { "\u{2212}" } else { "" }, if num.is_empty() && !pi { "1".into() } else { num }));
    }
    Txt::Frac { neg: r.n < 0, num: format!("{num}{p}"), den: r.d.to_string() }
}

/// A fraction rounded to `places` exactly (1.005 is 1.01, never 1.00 from a binary near-miss), with
/// all its places when it was rounded and without trailing zeros when it fits.
pub fn rat_round(r: Rat, places: usize) -> String {
    let scale = 10i128.pow(places as u32);
    let (Some(num), true) = (r.n.checked_mul(scale), r.d > 0) else { return round_str(r.f(), places) };
    let q = num.abs() / r.d;
    let rem = num.abs() % r.d;
    let q = if rem * 2 >= r.d { q + 1 } else { q };
    let exact = rem == 0;
    let neg = r.n < 0 && q != 0;
    let mut s = q.to_string();
    if places > 0 {
        while s.len() <= places {
            s.insert(0, '0');
        }
        s.insert(s.len() - places, '.');
        if exact {
            while s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.pop();
            }
        }
    }
    format!("{}{s}", if neg { "\u{2212}" } else { "" })
}

/// A worked-out y: exact in Fractions, rounded in Decimals or where it is not exact.
pub fn y_txt(v: N, fractions: bool, places: usize) -> Txt {
    match v {
        N::Undef => Txt::Plain("undefined".into()),
        N::Q(r) if fractions => rat_txt(r, false),
        N::Pi(r) if fractions => rat_txt(r, true),
        N::Q(r) => Txt::Plain(rat_round(r, places)),
        _ => Txt::Plain(round_str(v.f(), places)),
    }
}

/// A rate, ratio or coefficient in the answers: never rounded into something false. In Fractions it is
/// a fraction; in Decimals a terminating decimal in full, else the fraction; an approximation to 4 places.
fn exact_str(v: N, fractions: bool) -> String {
    match v {
        N::Q(r) if fractions => rat_txt(r, false).inline(),
        N::Q(r) => {
            let mut d = r.d;
            for p in [2i128, 5] {
                while d % p == 0 {
                    d /= p;
                }
            }
            if d == 1 { rat_round(r, 12) } else { rat_txt(r, false).inline() }
        }
        N::Pi(r) => rat_txt(r, true).inline(),
        N::F(f) if f != 0.0 && f.abs() < 1e-4 => format!("about {}", sci(f)),
        N::F(f) => format!("about {}", round_str(f, 4)),
        N::Undef => "undefined".into(),
    }
}

/// An x-value written in its style.
pub fn x_txt(v: N, style: Style) -> Txt {
    match (style, v) {
        (Style::Deg, N::Q(r)) if r.is_int() => Txt::Plain(format!("{}°", minus(&r.n.to_string()))),
        (Style::Deg, _) => Txt::Plain(format!("{}°", round_str(v.f(), 2))),
        (Style::Pi, N::Pi(r)) => rat_txt(r, true),
        (_, N::Q(r)) if r.is_int() => Txt::Plain(minus(&r.n.to_string())),
        (Style::Dec, N::Q(r)) => Txt::Plain(rat_round(r, 12)),
        (_, N::Q(r)) => rat_txt(r, false),
        (_, N::Pi(r)) => rat_txt(r, true),
        _ => Txt::Plain(round_str(v.f(), 4)),
    }
}

/// A typed cell: a simple fraction is stacked, anything else is written as typed.
fn typed_txt(s: &str) -> Txt {
    let t = s.trim();
    if let Some((a, b)) = t.split_once('/') {
        let (neg, a) = match a.trim().strip_prefix(['-', '\u{2212}']) {
            Some(r) => (true, r.trim()),
            None => (false, a.trim()),
        };
        let b = b.trim();
        if !a.is_empty() && !b.is_empty() && a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()) {
            return Txt::Frac { neg, num: a.to_string(), den: b.to_string() };
        }
    }
    Txt::Plain(minus(&clip(t, MAX_CELL)))
}

fn clip(s: &str, n: usize) -> String { s.chars().take(n).collect() }

/// Longest heading of your own, in characters.
pub const MAX_HEAD: usize = 60;

/// Cut to `n` characters at a word break where there is one, with … to show the cut.
fn cut_words(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let head: String = s.chars().take(n.saturating_sub(1)).collect();
    let at = head.rfind(' ').filter(|i| *i > n / 2).unwrap_or(head.len());
    format!("{}\u{2026}", head[..at].trim_end())
}

// ───────────────────────── the model ─────────────────────────

/// One column of the table (one x and its y).
#[derive(Clone, Debug)]
pub struct Col {
    pub x: Txt,
    pub y: Txt,
    /// Exact or approximate values when they are numbers (for the answers and the linear check).
    pub xv: Option<N>,
    pub yv: Option<N>,
    /// The cell is a blank (chosen, every y, or an empty typed cell).
    pub x_blank: bool,
    pub y_blank: bool,
    /// An empty typed cell: a blank with no answer.
    pub x_empty: bool,
    pub y_empty: bool,
}

pub struct Model {
    pub cols: Vec<Col>,
    pub heads: [String; 2],
    pub lhs: String,
    pub rhs: String,
    pub rule_ok: bool,
    pub from_rule: bool,
    pub notes: Vec<String>,
    /// How worked-out numbers are written (the values, and the rates in the answers).
    pub fractions: bool,
    /// The table runs down (its x-values are rows).
    pub down: bool,
}

/// The blank cells the `blanks` box lists: (column from 0, is it the y).
pub fn blank_cells(t: &str) -> Vec<(usize, bool)> {
    let mut v = Vec::new();
    for tok in t.split([',', ';', ' ']).map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
        let mut it = tok.chars();
        let y = match it.next() {
            Some('y') => true,
            Some('x') => false,
            _ => continue,
        };
        let rest = it.as_str();
        if let Ok(c) = rest.parse::<usize>() {
            if (1..=MAX_COLS).contains(&c) && !v.contains(&(c - 1, y)) {
                v.push((c - 1, y));
            }
        }
    }
    v
}

fn write_blanks(cells: &[(usize, bool)]) -> String {
    let mut c = cells.to_vec();
    c.sort_by_key(|&(col, y)| (y, col));
    c.iter().map(|&(col, y)| format!("{}{}", if y { "y" } else { "x" }, col + 1)).collect::<Vec<_>>().join(", ")
}

/// The headings, as written: a pair, or your own (empty is a blank heading).
pub fn heads(p: &Params) -> [String; 2] {
    match p.s("heads") {
        "Input / Output" => ["Input".into(), "Output".into()],
        "x / f(x)" => ["x".into(), "f(x)".into()],
        "n / aₙ" => ["n".into(), "a_n".into()],
        "Your own" => [cut_words(p.s("head1").trim(), MAX_HEAD), cut_words(p.s("head2").trim(), MAX_HEAD)],
        _ => ["x".into(), "y".into()],
    }
}

/// The x-values from the range or the list, each with its style, and the notes on them.
fn x_values(p: &Params, notes: &mut Vec<String>) -> Vec<(N, Style, Option<String>)> {
    if p.s("xs") == XS[1] {
        let mut v = Vec::new();
        for item in list_items(p.s("list")) {
            match read_x(&item) {
                Some((x, st)) => v.push((x, st, Some(item.clone()))),
                None => notes.push(format!("Can't read the x-value \u{201c}{}\u{201d}, so it is left out.", clip(&item, 20))),
            }
        }
        if v.len() > MAX_COLS {
            notes.push(format!("Only the first {MAX_COLS} x-values are used."));
            v.truncate(MAX_COLS);
        }
        if v.iter().any(|x| x.1 == Style::Deg) && v.iter().any(|x| x.1 == Style::Pi) {
            notes.push("The list mixes degrees and \u{3c0}; the rule reads every x-value in degrees.".into());
        }
        return v;
    }
    let (from, to, step) = (read_x(p.s("from")), read_x(p.s("to")), read_x(p.s("step")));
    let (Some((a, sa)), Some((b, sb)), Some((h, sh))) = (from, to, step) else {
        for (k, name) in [("from", "From"), ("to", "To"), ("step", "Step")] {
            if read_x(p.s(k)).is_none() {
                notes.push(format!("Can't read {name}: type a number, a fraction, a multiple of \u{3c0} or degrees."));
            }
        }
        return Vec::new();
    };
    // the range is written in the most particular style typed in it
    let style = sa.max(sb).max(sh);
    let any_deg = [sa, sb, sh].contains(&Style::Deg);
    if any_deg && [sa, sb, sh].iter().any(|s| *s == Style::Pi) {
        notes.push("The range mixes degrees and \u{3c0}; it is read in degrees.".into());
    }
    if h.is_zero() || h.f() == 0.0 {
        notes.push("The step is 0, so the table has just the From value.".into());
        return vec![(a, style, None)];
    }
    let (af, bf, hf) = (a.f(), b.f(), h.f());
    if (bf - af) * hf < 0.0 {
        notes.push("The step goes the wrong way to reach To, so the table has just the From value.".into());
        return vec![(a, style, None)];
    }
    let count = ((bf - af) / hf + 1e-9).floor() as i64 + 1;
    if count > MAX_COLS as i64 {
        notes.push(format!("That range has {count} x-values; only the first {MAX_COLS} are used. Make the step bigger to reach To."));
    }
    let mut v = Vec::new();
    let mut x = a;
    for _ in 0..count.clamp(1, MAX_COLS as i64) {
        v.push((x, style, None));
        x = x.add(h);
    }
    if count <= MAX_COLS as i64 {
        if let Some(&(last, st, _)) = v.last() {
            if (last.f() - bf).abs() > 1e-9 * (1.0 + bf.abs()) {
                notes.push(format!("The step does not land on To, so the last x-value is {}.", x_txt(last, st).inline()));
            }
        }
    }
    v
}

pub fn model(p: &Params) -> Model {
    let mut notes = Vec::new();
    let from_rule = p.s("source") != SOURCES[1];
    let heads = heads(p);
    let (lhs, rhs) = split_rule(&clip(p.s("rule"), MAX_RULE));
    let mut rule_ok = true;
    let mut cols = Vec::new();
    let chosen = blank_cells(p.s("blanks"));
    let places = ROUNDS.iter().position(|r| *r == p.s("round")).unwrap_or(2);
    // a rule or values typed with decimals (y = 0.1x, a step of 0.5, typed 1.25) are worked out in decimals
    let typed_decimals = if from_rule {
        rhs.contains('.') || (p.s("xs") == XS[1] && list_items(p.s("list")).iter().any(|t| t.contains('.')))
            || (p.s("xs") != XS[1] && [p.s("from"), p.s("to"), p.s("step")].iter().any(|t| t.contains('.')))
    } else {
        p.s("typed").contains('.')
    };
    let fractions = (p.s("as") != AS[1] || !from_rule) && !typed_decimals;
    if from_rule {
        let xs = x_values(p, &mut notes);
        let deg = xs.iter().any(|(_, s, _)| *s == Style::Deg);
        if rhs.is_empty() {
            rule_ok = false;
            notes.push("Type a rule, like y = 2x + 1, to work out the y-values.".into());
        } else if eval_rule(&rhs, N::q(1), deg).is_none() && eval_rule(&rhs, N::q(2), deg).is_none() {
            rule_ok = false;
            notes.push(format!("Can't read the rule \u{201c}{}\u{201d}. Type it in x, like y = 2x + 1 or y = x^2 \u{2212} 3.", clip(&rhs, 40)));
        }
        for (i, (x, st, raw)) in xs.into_iter().enumerate() {
            let yv = if rule_ok { eval_rule(&rhs, x, deg).map(snap) } else { None };
            let y = match yv {
                Some(v) => y_txt(v, fractions, places),
                None => Txt::empty(),
            };
            let x_blank = chosen.contains(&(i, false));
            let y_blank = p.b("blank_y") || chosen.contains(&(i, true));
            // a typed value that is not exact (e, sqrt 2) is written as typed
            let xt = match (x, raw) {
                (N::F(_), Some(r)) => Txt::Plain(minus(&r.replace("pi", "\u{3c0}").replace("sqrt ", "\u{221a}").replace("sqrt", "\u{221a}"))),
                // a mixed number keeps its spelling: 1 1/2
                (N::Q(_), Some(r)) if st == Style::Frac && r.trim().split_whitespace().count() == 2 => Txt::Plain(minus(r.trim())),
                _ => x_txt(x, st),
            };
            cols.push(Col { x: xt, y, xv: Some(x), yv, x_blank, y_blank, x_empty: false, y_empty: yv.is_none() });
        }
    } else {
        let mut rows: Vec<(String, String)> = Vec::new();
        for line in p.s("typed").lines() {
            if line.trim().is_empty() {
                continue;
            }
            let (a, b) = if line.contains('\t') {
                line.split_once('\t').unwrap_or((line, ""))
            } else {
                line.split_once(',').unwrap_or((line, ""))
            };
            rows.push((a.trim().to_string(), b.trim().trim_start_matches(['\t', ',']).trim().to_string()));
        }
        let lines: Vec<&str> = p.s("typed").lines().filter(|l| !l.trim().is_empty()).collect();
        if !lines.is_empty() && lines.iter().all(|l| !l.contains(',') && !l.contains('\t')) && lines.iter().any(|l| l.trim().contains(' ')) {
            notes.push("Separate each x from its y with a comma or a tab (1, 5); with only spaces the whole line is the x.".into());
        }
        if rows.len() >= 2 {
            let num = |t: &str| read_x(t).is_some();
            let (a, b) = (&rows[0].0, &rows[0].1);
            if !num(a) && !num(b) && !a.is_empty() && rows[1..].iter().all(|(x, y)| num(x) && num(y)) {
                notes.push(format!("The first row ({a}, {b}) looks like headings: type them under Headings, Your own, and leave them out of the pairs."));
            }
        }
        if rows.len() > MAX_COLS {
            notes.push(format!("Only the first {MAX_COLS} pairs are used."));
            rows.truncate(MAX_COLS);
        }
        if rows.iter().any(|(a, b)| a.chars().count() > MAX_CELL || b.chars().count() > MAX_CELL) {
            notes.push(format!("A cell is longer than {MAX_CELL} characters, so it is cut."));
        }
        for (i, (a, b)) in rows.into_iter().enumerate() {
            let xv = read_x(&a).map(|v| v.0);
            let yv = read_x(&b).map(|v| v.0);
            let (xe, ye) = (a.is_empty(), b.is_empty());
            cols.push(Col {
                x: typed_txt(&a),
                y: typed_txt(&b),
                xv,
                yv,
                x_blank: xe || chosen.contains(&(i, false)),
                y_blank: ye || chosen.contains(&(i, true)),
                x_empty: xe,
                y_empty: ye,
            });
        }
    }
    if chosen.iter().any(|&(c, _)| c >= cols.len()) && !cols.is_empty() {
        notes.push(format!("A blank names a column past the last one ({}); it is ignored.", cols.len()));
    }
    Model { cols, heads, lhs, rhs, rule_ok, from_rule, notes, fractions, down: p.s("runs") == RUNS[1] }
}

pub fn warnings(p: &Params) -> Vec<String> {
    let m = model(p);
    let mut w = m.notes.clone();
    if p.s("title_kind") != TITLES[0] && p.s("title").trim().chars().count() > MAX_TITLE {
        w.push(format!("The title is longer than {MAX_TITLE} characters, so it is cut."));
    }
    if p.s("heads") == HEADS[4] && [p.s("head1"), p.s("head2")].iter().any(|h| h.trim().chars().count() > MAX_HEAD) {
        w.push(format!("A heading is longer than {MAX_HEAD} characters, so it is cut."));
    }
    if p.s("rule").chars().count() > MAX_RULE {
        w.push(format!("The rule is longer than {MAX_RULE} characters, so it is cut."));
    }
    w
}

// ───────────────────────── answers ─────────────────────────

/// What the rule's left side is called: as typed, else the second heading when it is a letter name.
fn lhs_name(m: &Model) -> String {
    if !m.lhs.is_empty() {
        return m.lhs.clone();
    }
    match m.heads[1].as_str() {
        "f(x)" => "f(x)".into(),
        "a_n" => "a_n".into(),
        _ => "y".into(),
    }
}

/// A heading as a word in the answers (a blank heading reads x or y).
fn head_word(h: &str, y: bool) -> String {
    let h = h.trim();
    if h.is_empty() {
        return if y { "y".into() } else { "x".into() };
    }
    h.to_string()
}


/// The line y = mx + b, written plainly.
fn line_str(md: &Model, m: Rat, b: Rat) -> String {
    let mx = match (m.n, m.d) {
        (0, _) => String::new(),
        (1, 1) => "x".into(),
        (-1, 1) => "\u{2212}x".into(),
        _ if m.n < 0 => format!("\u{2212}{}x", coef(md, m.neg())),
        _ => format!("{}x", coef(md, m)),
    };
    if mx.is_empty() {
        return format!("y = {}", num_inline(md, N::Q(b)));
    }
    match b.n.signum() {
        0 => format!("y = {mx}"),
        1 => format!("y = {mx} + {}", num_inline(md, N::Q(b))),
        _ => format!("y = {mx} \u{2212} {}", num_inline(md, N::Q(b.neg()))),
    }
}

/// The rate of change between neighbouring columns when every one is the same, exact where it can be.
enum Shape {
    Linear(N),
    Ratio(N),
    Not,
}

fn shape(pts: &[(N, N)]) -> Option<Shape> {
    // two points always lie on a line, so say nothing about fewer than three
    if pts.len() < 3 {
        return None;
    }
    if pts.iter().any(|(x, y)| matches!(x, N::Undef) || matches!(y, N::Undef)) {
        return Some(Shape::Not);
    }
    let rates: Vec<N> = pts.windows(2).map(|w| w[1].1.add(w[0].1.neg()).div(w[1].0.add(w[0].0.neg()))).collect();
    if rates.iter().any(|r| matches!(r, N::Undef)) {
        return Some(Shape::Not);
    }
    // equal relative to their size, so tiny values are compared as carefully as large ones
    let same = |a: N, b: N| match (a, b) {
        (N::Q(x), N::Q(y)) | (N::Pi(x), N::Pi(y)) => x == y,
        _ => (a.f() - b.f()).abs() <= 1e-9 * a.f().abs().max(b.f().abs()) + 1e-15,
    };
    // a rate that is a simple fraction to within rounding error is that fraction (x + 1 over π x-values)
    let rates: Vec<N> = rates.into_iter().map(snap).collect();
    if rates.windows(2).all(|w| same(w[0], w[1])) {
        return Some(Shape::Linear(rates[0]));
    }
    // evenly spaced x and a constant ratio: exponential
    let steps: Vec<N> = pts.windows(2).map(|w| w[1].0.add(w[0].0.neg())).collect();
    if pts.iter().all(|(_, y)| !y.is_zero()) && steps.windows(2).all(|w| same(w[0], w[1])) {
        let ratios: Vec<N> = pts.windows(2).map(|w| w[1].1.div(w[0].1)).collect();
        if ratios.windows(2).all(|w| same(w[0], w[1])) {
            return Some(Shape::Ratio(ratios[0]));
        }
    }
    Some(Shape::Not)
}

/// A rate or ratio in the answers, in the table's own number form.
fn num_inline(m: &Model, v: N) -> String { exact_str(v, m.fractions) }

/// A coefficient before x: (5/2)x, not 5/2x, which reads as 5 over 2x.
fn coef(m: &Model, r: Rat) -> String {
    let t = num_inline(m, N::Q(r));
    if t.contains('/') { format!("({t})") } else { t }
}

/// The lines of the answer key, in order: each blank and its value, the rule, linear or not.
pub fn answers(m: &Model) -> Vec<String> {
    let mut out = Vec::new();
    let (xw, yw) = (head_word(&m.heads[0], false), head_word(&m.heads[1], true));
    for (i, c) in m.cols.iter().enumerate() {
        let (xs, ys) = (c.x.inline(), c.y.inline());
        if c.x_blank && !c.x_empty && c.y_blank && !c.y_empty {
            out.push(format!("{} {}: {xw} = {xs}, {yw} = {ys}", if m.down { "Row" } else { "Column" }, i + 1));
        } else if c.y_blank && !c.y_empty && !ys.is_empty() {
            out.push(format!("{yw} when {xw} = {}: {ys}", if c.x_empty { "?".to_string() } else { xs }));
        } else if c.x_blank && !c.x_empty {
            let place = if m.down { "row" } else { "column" };
            if ys.is_empty() || ys == "undefined" || c.y_blank {
                out.push(format!("{xw} in {place} {}: {xs}", i + 1));
            } else {
                out.push(format!("{xw} when {yw} = {ys}: {xs}"));
            }
        }
    }
    out.truncate(30);
    if m.from_rule && m.rule_ok {
        let pts: Vec<(N, N)> = m.cols.iter().filter_map(|c| Some((c.xv?, c.yv?))).collect();
        match shape(&pts) {
            Some(Shape::Linear(r)) => out.push(format!("Linear: the rate of change is {}.", num_inline(m, r))),
            Some(Shape::Ratio(r)) => out.push(format!("Not linear: each y is {} times the one before.", num_inline(m, r))),
            Some(Shape::Not) => out.push("Not linear: the rate of change is not the same all along.".into()),
            None => {}
        }
    } else if !m.from_rule {
        let pts: Option<Vec<(Rat, Rat)>> = m
            .cols
            .iter()
            .filter(|c| !(c.x_empty || c.y_empty))
            .map(|c| match (c.xv?, c.yv?) {
                (N::Q(a), N::Q(b)) => Some((a, b)),
                _ => None,
            })
            .collect();
        if let Some(pts) = pts.filter(|v| v.len() >= 3) {
            let qs: Vec<(N, N)> = pts.iter().map(|&(a, b)| (N::Q(a), N::Q(b))).collect();
            match shape(&qs) {
                Some(Shape::Linear(N::Q(r))) => {
                    let b = r.mul(pts[0].0).and_then(|mx| pts[0].1.add(mx.neg()));
                    match b {
                        Some(b) => out.push(format!("Linear: {}, a rate of change of {}.", line_str(m, r, b), num_inline(m, N::Q(r)))),
                        None => out.push(format!("Linear: a rate of change of {}.", num_inline(m, N::Q(r)))),
                    }
                }
                Some(Shape::Ratio(r)) => out.push(format!("Not linear: each y is {} times the one before.", num_inline(m, r))),
                Some(_) => out.push("Not linear.".into()),
                None => {}
            }
        }
    }
    out
}

/// The rule as written above the table: the left side, =, and the right side.
pub fn rule_text(m: &Model) -> String {
    format!("{} = {}", lhs_name(m), m.rhs)
}

// ───────────────────────── drawing ─────────────────────────

/// Approximate width of a run of text at `size`.
fn tw(s: &str, size: f64) -> f64 { crate::figures::stats::bar::tw(s, size) }

/// The answer line that gives a blanked rule (written with the rule's own markup).
const RULE_KEY: &str = "The rule: ";

/// A heading, the rule's left side or an answer line as SVG text: a_n as a subscript, the words as typed.
pub fn math_svg(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '_' && i + 1 < cs.len() && cs[i + 1].is_alphanumeric() {
            out.push_str(&format!(r#"<tspan baseline-shift="sub" font-size="70%">{}</tspan>"#, esc(&cs[i + 1].to_string())));
            i += 2;
            continue;
        }
        out.push_str(&esc(&cs[i].to_string()));
        i += 1;
    }
    out
}

/// The visible width of such text.
fn math_w(s: &str, size: f64) -> f64 { tw(&s.replace('_', ""), size) }

/// Width of text in ems of its size (`tw` adds a pixel of slack per run).
fn em_of(s: &str) -> f64 {
    // the shared table counts every non-ASCII sign as wide; the rule's math signs are as wide as these
    let ascii: String = s.chars().map(|c| match c {
        '\u{2212}' | '\u{d7}' => '+',
        '\u{3c0}' => 'n',
        '\u{b7}' => '.',
        '\u{b0}' => '*',
        c => c,
    }).collect();
    (tw(&ascii, 100.0) - 1.0) / 100.0
}

/// The rule's left side and " = " (y = , f(x) = , aₙ = ).
fn lhs_runs(m: &Model) -> Vec<Run> {
    let l = format!("{} = ", lhs_name(m));
    let mut w = Wr::new();
    let cs: Vec<char> = l.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '_' && i + 1 < cs.len() && cs[i + 1].is_alphanumeric() {
            let c = cs[i + 1].to_string();
            w.sub(|w| w.text(&c));
            i += 2;
        } else {
            w.text(&cs[i].to_string());
            i += 1;
        }
    }
    w.finish()
}

/// The rule's right side, written from the same tree that works it out (as typed when it can't be read).
fn rhs_runs(m: &Model) -> Vec<Run> {
    match read_ex(&m.rhs, true) {
        Some(e) => {
            let mut w = Wr::new();
            write_ex(&e, &mut w);
            w.finish()
        }
        None => vec![Run::Text { text: m.rhs.clone(), em: em_of(&m.rhs), scale: 1.0, rise: 0.0 }],
    }
}

fn rule_runs(m: &Model) -> Vec<Run> {
    let mut v = lhs_runs(m);
    v.extend(rhs_runs(m));
    v
}

const RADICAL_EM: f64 = 0.62;
fn runs_em(r: &[Run]) -> f64 {
    r.iter().map(|r| match r {
        Run::Text { em, .. } => *em,
        Run::Radical { scale, .. } => RADICAL_EM * scale,
    }).sum()
}

/// A cell's box on the canvas, and what it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Column of the table (from 0) and whether it is the y.
    pub col: usize,
    pub is_y: bool,
}

pub struct Layout {
    pub w: f64,
    pub h: f64,
    pub fs: f64,
    pub title_y: f64,
    pub rule_y: f64,
    /// Heading boxes: x then y.
    pub head: [CellBox; 2],
    pub cells: Vec<CellBox>,
    pub key_y: f64,
    pub key_lines: Vec<String>,
    pub across: bool,
    /// Rows are tall enough for stacked fractions (some value students see is one).
    pub stacked: bool,
}

fn txt_w(t: &Txt, fs: f64) -> f64 {
    match t {
        Txt::Plain(s) => tw(s, fs) * 1.05,
        Txt::Frac { neg, num, den } => tw(num, fs * 0.86).max(tw(den, fs * 0.86)) + if *neg { tw("\u{2212}", fs) + 2.0 } else { 0.0 } + 6.0,
    }
}

fn head_w(h: &str, fs: f64) -> f64 { if h.trim().is_empty() { fs * 2.4 } else { math_w(h, fs) } }

pub fn layout(p: &Params, m: &Model) -> Layout {
    let k = svg::label_factor(p.s("label_size"));
    let fs = 18.0 * k;
    let across = p.s("runs") != RUNS[1];
    let key_on = p.b(SHOW_ANSWERS);
    let pad = fs * 0.9;
    let margin = 24.0;
    // sized from what the student copy shows, so a blank's hidden answer never widens its column or
    // makes its row taller (a blank takes a fixed width on both copies; the key fits its answer in)
    let seen = |t: &Txt, blank: bool| if blank { None } else { Some(t.clone()) };
    let stacked = m.cols.iter().any(|c| [seen(&c.x, c.x_blank), seen(&c.y, c.y_blank)].iter().flatten().any(|t| matches!(t, Txt::Frac { .. })));
    // a blank is as wide as the widest value students see (at least BLANK_EM), never its own answer
    let widest_seen = m.cols.iter().flat_map(|c| [seen(&c.x, c.x_blank), seen(&c.y, c.y_blank)]).flatten().map(|t| txt_w(&t, fs)).fold(fs * BLANK_EM, f64::max);
    let cell_w = |t: &Txt, blank: bool| if blank { widest_seen } else { txt_w(t, fs) };
    let rowh = if stacked { fs * 2.9 } else { fs * 2.1 };
    let mut top = margin;
    let title_on = p.s("title_kind") != TITLES[0];
    let title_y = top + fs * 1.1;
    if title_on {
        top += fs * 2.2;
    }
    let rule_on = m.from_rule && p.s("show_rule") != RULE_SHOWN[0];
    let rule_y = top + fs * 1.0;
    if rule_on {
        top += fs * 2.0;
    }
    // each column as wide as its own values (so one "undefined" does not widen them all), at least a
    // short number's width; down, both columns share the widest
    let col_w: Vec<f64> = m.cols.iter().map(|c| cell_w(&c.x, c.x_blank).max(cell_w(&c.y, c.y_blank)).max(fs * 1.6) + 2.0 * pad).collect();
    let value_w = col_w.iter().copied().fold(fs * 1.6 + 2.0 * pad, f64::max);
    let hw = head_w(&m.heads[0], fs).max(head_w(&m.heads[1], fs)) + 2.0 * pad;
    let n = m.cols.len();
    let x0 = margin;
    let y0 = top;
    let mut cells = Vec::new();
    let (head, table_w, table_h);
    if across {
        head = [CellBox { x: x0, y: y0, w: hw, h: rowh, col: 0, is_y: false }, CellBox { x: x0, y: y0 + rowh, w: hw, h: rowh, col: 0, is_y: true }];
        let mut x = x0 + hw;
        for (i, &w) in col_w.iter().enumerate() {
            cells.push(CellBox { x, y: y0, w, h: rowh, col: i, is_y: false });
            cells.push(CellBox { x, y: y0 + rowh, w, h: rowh, col: i, is_y: true });
            x += w;
        }
        table_w = x - x0;
        table_h = 2.0 * rowh;
    } else {
        let cw = value_w.max(hw);
        let hh = fs * 2.1;
        head = [CellBox { x: x0, y: y0, w: cw, h: hh, col: 0, is_y: false }, CellBox { x: x0 + cw, y: y0, w: cw, h: hh, col: 0, is_y: true }];
        for i in 0..n {
            let y = y0 + hh + rowh * i as f64;
            cells.push(CellBox { x: x0, y, w: cw, h: rowh, col: i, is_y: false });
            cells.push(CellBox { x: x0 + cw, y, w: cw, h: rowh, col: i, is_y: true });
        }
        table_w = 2.0 * cw;
        table_h = hh + rowh * n as f64;
    }
    let key_lines = if key_on {
        let mut v = answers(m);
        if rule_on && p.s("show_rule") == RULE_SHOWN[2] && m.rule_ok {
            v.insert(0, format!("{RULE_KEY}{}", rule_text(m)));
        }
        v
    } else {
        Vec::new()
    };
    let kfs = fs * 0.85;
    let key_y = y0 + table_h + fs * 1.6;
    let mut w = table_w + 2.0 * margin;
    for l in &key_lines {
        w = w.max(tw(l, kfs) + 2.0 * margin);
    }
    if title_on && p.s("title_kind") == TITLES[1] {
        // bold runs about a tenth wider than the regular widths `tw` measures
        w = w.max(tw(&clip(p.s("title").trim(), MAX_TITLE), fs * 1.15) * 1.12 + 2.0 * margin);
    }
    if rule_on {
        let need = if p.s("show_rule") == RULE_SHOWN[2] { blank_rule_w(m, fs, key_on) } else { runs_em(&rule_runs(m)) * fs };
        w = w.max(need + 2.0 * margin + fs * 2.0);
    }
    let w = w.max(320.0);
    let h = if key_lines.is_empty() { y0 + table_h + margin } else { key_y + kfs * 1.45 * (key_lines.len() as f64 - 1.0) + margin };
    // centre the table when the title or the key lines are wider
    let shift = ((w - table_w) / 2.0 - x0).max(0.0);
    let mv = |b: CellBox| CellBox { x: b.x + shift, ..b };
    Layout {
        w,
        h,
        fs,
        title_y,
        rule_y,
        head: [mv(head[0]), mv(head[1])],
        cells: cells.into_iter().map(mv).collect(),
        key_y,
        key_lines,
        across,
        stacked,
    }
}

/// How wide a blank cell is drawn, in ems of the label size, on both copies.
const BLANK_EM: f64 = 2.0;
/// The blank line for the rule: the same length whatever the rule (so it does not hint at the rule's
/// length); the key copy lengthens it only when the written rule needs more room.
fn blank_line_w(m: &Model, fs: f64, key_on: bool) -> f64 {
    let base = fs * 8.0;
    if key_on && m.rule_ok { base.max(runs_em(&rhs_runs(m)) * fs * 1.1 + fs) } else { base }
}
fn blank_rule_w(m: &Model, fs: f64, key_on: bool) -> f64 { runs_em(&lhs_runs(m)) * fs + blank_line_w(m, fs, key_on) }

fn text_el(x: f64, y: f64, size: f64, anchor: &str, body: &str, ink: &str, bold: bool) -> String {
    let weight = if bold { r#" font-weight="700""# } else { "" };
    // spaces are kept: a run of the rule that ends in " + " is followed by a √ drawn on its own
    format!(r#"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{ink}" xml:space="preserve"{weight}>{body}</text>"#)
}

/// The sans faces have no radical sign, and the renderer then draws the whole text element in a serif
/// face. So a rule with √ is drawn as separate pieces side by side, only the √ itself in a serif face.
const RADICAL_FONT: &str = "Source Serif 4, Noto Serif, DejaVu Serif, serif";

/// Draws runs side by side from x (by `anchor`). One plain run is one text element at its natural
/// width. Otherwise each run is its own element, fitted to its measured width with `textLength` (the
/// renderer then lays it out exactly that wide), so powers and each √ land flush against their
/// neighbours, with no gap or overlap.
fn draw_runs(x: f64, y: f64, size: f64, anchor: &str, runs: &[Run], ink: &str, bold: bool) -> String {
    let weight = if bold { r#" font-weight="700""# } else { "" };
    // neighbouring text runs at the same size and height are one element
    let mut merged: Vec<Run> = Vec::new();
    for r in runs {
        match (merged.last_mut(), r) {
            (Some(Run::Text { text: a, em: ea, scale: sa, rise: ra }), Run::Text { text: b, em: eb, scale: sb, rise: rb }) if sa == sb && ra == rb => {
                a.push_str(b);
                *ea += eb;
            }
            _ => merged.push(r.clone()),
        }
    }
    if let [Run::Text { text, scale, rise, .. }] = merged.as_slice() {
        if *scale == 1.0 && *rise == 0.0 {
            return text_el(x, y, size, anchor, &esc(text), ink, bold);
        }
    }
    // bold runs a little wider than the regular widths measured
    let k = if bold { 1.08 } else { 1.0 };
    let width = |r: &Run| match r {
        Run::Text { em, .. } => em * size * k,
        Run::Radical { scale, .. } => RADICAL_EM * scale * size,
    };
    let total: f64 = merged.iter().map(width).sum();
    let mut cx = match anchor {
        "middle" => x - total / 2.0,
        "end" => x - total,
        _ => x,
    };
    let mut out = String::new();
    for r in &merged {
        let w = width(r);
        match r {
            Run::Radical { scale, rise } => {
                let (rs, ry) = (size * scale, y - rise * size);
                out.push_str(&format!(r#"<text x="{cx:.2}" y="{ry:.2}" font-size="{rs:.2}" text-anchor="start" font-family="{RADICAL_FONT}" fill="{ink}"{weight}>√</text>"#));
            }
            Run::Text { text, scale, rise, .. } => {
                let el = text_el(cx, y - rise * size, size * scale, "start", &esc(text), ink, bold);
                out.push_str(&el.replacen("<text ", &format!(r#"<text textLength="{w:.2}" lengthAdjust="spacing" "#), 1));
            }
        }
        cx += w;
    }
    out
}

fn draw_txt(d: &mut Doc, b: &CellBox, t: &Txt, fs: f64, ink: &str, bold: bool) {
    let (cx, cy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
    match t {
        Txt::Plain(s) => {
            d.raw(&text_el(cx, cy + fs * 0.35, fs, "middle", &esc(s), ink, bold));
        }
        Txt::Frac { neg, num, den } => {
            let f = fs * 0.86;
            let half = tw(num, f).max(tw(den, f)) / 2.0 + 3.0;
            let off = if *neg { (tw("\u{2212}", fs) + 2.0) / 2.0 } else { 0.0 };
            let fx = cx + off;
            if *neg {
                d.raw(&text_el(fx - half - 2.0, cy + fs * 0.35, fs, "end", "\u{2212}", ink, bold));
            }
            d.raw(&text_el(fx, cy - f * 0.3, f, "middle", &esc(num), ink, bold));
            d.raw(&text_el(fx, cy + f * 1.05, f, "middle", &esc(den), ink, bold));
            d.line(fx - half, cy, fx + half, cy, ink, if bold { 2.0 } else { 1.4 });
        }
    }
}

fn render(p: &Params) -> String {
    let m = model(p);
    let l = layout(p, &m);
    let key_on = p.b(SHOW_ANSWERS);
    let fills = Fills::new(p);
    let print = fills.is_print();
    let ink = if print { "#000000" } else { INK };
    let key_ink = if print { "#000000" } else { KEY_INK };
    let fs = l.fs;
    let mut d = Doc::new(l.w, l.h);

    // title
    match p.s("title_kind") {
        "Text" => {
            let t = clip(p.s("title").trim(), MAX_TITLE);
            d.raw(&text_el(l.w / 2.0, l.title_y, fs * 1.15, "middle", &esc(&t), ink, true));
        }
        "Blank line" => {
            let half = (l.w * 0.22).max(fs * 5.0).min(l.w / 2.0 - 24.0);
            d.line(l.w / 2.0 - half, l.title_y + fs * 0.25, l.w / 2.0 + half, l.title_y + fs * 0.25, ink, 1.4);
        }
        _ => {}
    }
    // the rule above the table
    let rule_mode = if m.from_rule { p.s("show_rule") } else { RULE_SHOWN[0] };
    if rule_mode == RULE_SHOWN[1] {
        d.raw(&draw_runs(l.w / 2.0, l.rule_y, fs, "middle", &rule_runs(&m), ink, false));
    } else if rule_mode == RULE_SHOWN[2] {
        let left = lhs_runs(&m);
        let lw = runs_em(&left) * fs;
        let line_w = blank_line_w(&m, fs, key_on);
        let x = l.w / 2.0 - (lw + line_w) / 2.0;
        d.raw(&draw_runs(x, l.rule_y, fs, "start", &left, ink, false));
        d.line(x + lw, l.rule_y + fs * 0.25, x + lw + line_w, l.rule_y + fs * 0.25, ink, 1.4);
        if key_on && m.rule_ok {
            d.raw(&draw_runs(x + lw + line_w / 2.0, l.rule_y - fs * 0.05, fs, "middle", &rhs_runs(&m), key_ink, true));
        }
    }

    // headings
    let gray = p.s("head_look") != HEAD_LOOKS[1];
    let tint = if print { "#e5e5e5" } else { "#e5e7eb" };
    for (i, b) in l.head.iter().enumerate() {
        if gray {
            d.rect(b.x, b.y, b.w, b.h, tint, "none", 0.0);
        }
        let h = &m.heads[i];
        if !h.trim().is_empty() {
            d.raw(&text_el(b.x + b.w / 2.0, b.y + b.h / 2.0 + fs * 0.35, fs, "middle", &math_svg(h), ink, true));
        }
    }
    // values
    for b in &l.cells {
        let c = &m.cols[b.col];
        let (t, blank, empty) = if b.is_y { (&c.y, c.y_blank, c.y_empty) } else { (&c.x, c.x_blank, c.x_empty) };
        if blank {
            if key_on && !empty {
                let mut t = if l.stacked { t.clone() } else { Txt::Plain(t.inline()) };
                let fit = |t: &Txt| (b.w - 8.0) / (txt_w(t, fs) * 1.1);
                // past half size, a long number is written in powers of ten (the answer list has it in full)
                let v = if b.is_y { c.yv } else { c.xv };
                if fit(&t) < 0.5 {
                    if let Some(v) = v.filter(|v| v.f().is_finite() && v.f().abs() >= 1e6) {
                        t = Txt::Plain(sci(v.f()));
                    }
                }
                // a long word answer is cut to its box with … (the answer list has it in full)
                if fit(&t) < 0.5 {
                    if let Txt::Plain(full) = &t {
                        let mut cut: String = full.clone();
                        while cut.chars().count() > 1 && fit(&Txt::Plain(format!("{cut}\u{2026}"))) < 0.5 {
                            cut.pop();
                        }
                        t = Txt::Plain(format!("{}\u{2026}", cut.trim_end()));
                    }
                }
                let f = (fs * fit(&t).min(1.0)).max(fs * 0.5);
                draw_txt(&mut d, b, &t, f, key_ink, true);
            }
        } else {
            draw_txt(&mut d, b, t, fs, ink, false);
        }
    }
    // grid lines over the cells
    let all: Vec<CellBox> = l.head.iter().copied().chain(l.cells.iter().copied()).collect();
    for b in &all {
        d.rect(b.x, b.y, b.w, b.h, "none", ink, 1.4);
    }
    let (minx, miny) = all.iter().fold((f64::MAX, f64::MAX), |a, b| (a.0.min(b.x), a.1.min(b.y)));
    let (maxx, maxy) = all.iter().fold((f64::MIN, f64::MIN), |a, b| (a.0.max(b.x + b.w), a.1.max(b.y + b.h)));
    d.rect(minx, miny, maxx - minx, maxy - miny, "none", ink, 2.2);

    // the answers
    let kfs = fs * 0.85;
    for (i, line) in l.key_lines.iter().enumerate() {
        let y = l.key_y + kfs * 1.45 * i as f64;
        match line.strip_prefix(RULE_KEY) {
            Some(_) => {
                // "The rule: " and the rule as written above, centred together
                let mut runs = vec![Run::Text { text: RULE_KEY.to_string(), em: em_of(RULE_KEY), scale: 1.0, rise: 0.0 }];
                runs.extend(rule_runs(&m));
                d.raw(&draw_runs(l.w / 2.0, y, kfs, "middle", &runs, key_ink, false));
            }
            None => {
                d.raw(&text_el(l.w / 2.0, y, kfs, "middle", &math_svg(line), key_ink, false));
            }
        }
    }

    d.describe("Table of values", &describe(p, &m, key_on));
    d.finish()
}

/// The figure's description: the headings and every value, with blanks as "blank" on the student copy
/// (the rule only where it is written on the figure).
fn describe(p: &Params, m: &Model, key_on: bool) -> String {
    let (xw, yw) = (head_word(&m.heads[0], false), head_word(&m.heads[1], true));
    let shown = |t: &Txt, blank: bool, empty: bool| if blank && (!key_on || empty) { "blank".to_string() } else { t.inline() };
    let vals: Vec<String> = m.cols.iter().map(|c| format!("{} {}, {} {}", xw, shown(&c.x, c.x_blank, c.x_empty), yw, shown(&c.y, c.y_blank, c.y_empty))).collect();
    let rule = match (m.from_rule, p.s("show_rule")) {
        (true, "Written") => format!(" for {}", rule_text(m)),
        (true, "Blank line") if key_on => format!(" for {}", rule_text(m)),
        _ => String::new(),
    };
    format!("A table of values{rule}: {}.", if vals.is_empty() { "no values".to_string() } else { vals.join("; ") }).replace("a_n", "a\u{2099}")
}

// ───────────────────────── on-figure handles ─────────────────────────

/// Each value cell: click to make it a blank, or to fill a blank back in.
pub fn handles(p: &Params) -> Vec<Handle> {
    let m = model(p);
    let l = layout(p, &m);
    let (xw, yw) = (head_word(&m.heads[0], false), head_word(&m.heads[1], true));
    let n = m.cols.len();
    l.cells
        .iter()
        .filter_map(|b| {
            let c = &m.cols[b.col];
            let (blank, empty) = if b.is_y { (c.y_blank, c.y_empty) } else { (c.x_blank, c.x_empty) };
            // an empty typed cell is a blank by what is typed; there is nothing to fill in
            if empty && !m.from_rule {
                return None;
            }
            let place = if l.across { "column" } else { "row" };
            let what = if b.is_y && !c.x.inline().is_empty() { format!("{yw} for {xw} = {}", c.x.inline()) } else if b.is_y { format!("{yw} in {place} {}", b.col + 1) } else { format!("{xw} in {place} {}", b.col + 1) };
            let tip = if blank { format!("{what}: click to show it again") } else { format!("{what}: click to make a blank") };
            let (col, is_y) = (b.col, b.is_y);
            Some(Handle::zone((b.x + b.w / 2.0, b.y + b.h / 2.0), (b.w - 4.0, b.h - 4.0), &tip, move |q| toggle(q, col, is_y, n)))
        })
        .collect()
}

/// Makes a cell a blank, or fills it back in. With every y blank, clicking a y turns that off and
/// keeps the other y's blank.
pub fn toggle(p: &mut Params, col: usize, is_y: bool, n: usize) {
    let mut cells = blank_cells(p.s("blanks"));
    if is_y && p.b("blank_y") {
        p.set_b("blank_y", false);
        for c in 0..n {
            if c != col && !cells.contains(&(c, true)) {
                cells.push((c, true));
            }
        }
        cells.retain(|&c| c != (col, true));
    } else if let Some(i) = cells.iter().position(|&c| c == (col, is_y)) {
        cells.remove(i);
    } else {
        cells.push((col, is_y));
    }
    p.set_s("blanks", &write_blanks(&cells));
}

#[cfg(test)]
#[path = "tov/tests.rs"]
mod tests;
