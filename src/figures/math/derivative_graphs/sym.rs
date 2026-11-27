//! Derivative graphs: the typed function as a tree, so its derivatives can be written out as expressions the
//! app's reader (`math::eval`, the reader behind the coordinate grid) draws.
//!
//! The tree is read by a port of that reader (`math::Ev`): the same precedence, implicit products, letter runs
//! (xe is x times e, 2ex is 2 times e times x), scientific numbers, log_b, sin^-1 and |a|. A test checks that every
//! classroom expression reads to the same value both ways. Derivatives are taken by the usual rules and printed
//! back fully bracketed, with every product written with *, so the reader reads them exactly as built.
//!
//! One difference, on purpose: the reader gives no value for a negative number to a fractional power (x^(1/3) at
//! x < 0). A power whose exponent is a fraction with an odd denominator (1/3, 2/3, 5/3, -1/3 ...) is written for the
//! reader as the real odd root, |u|^(p/q) with the sign of u when p is odd, so x^(1/3) and x^(2/3) are drawn on both
//! sides of 0 as a textbook draws them.
use std::f64::consts::{E as EULER, PI};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum F { Sqrt, Abs, Sin, Cos, Tan, Sec, Csc, Cot, Asin, Acos, Atan, Asec, Acsc, Acot, Ln, Log, Exp }

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Ex {
    Num(f64),
    X,
    /// pi and e, kept by name so they print as typed
    Pi,
    E,
    Neg(Box<Ex>),
    Add(Box<Ex>, Box<Ex>),
    Sub(Box<Ex>, Box<Ex>),
    Mul(Box<Ex>, Box<Ex>),
    Div(Box<Ex>, Box<Ex>),
    Pow(Box<Ex>, Box<Ex>),
    Fun(F, Box<Ex>),
    /// log_b(a): (base, argument)
    LogB(Box<Ex>, Box<Ex>),
}
use Ex::*;

fn bx(e: Ex) -> Box<Ex> { Box::new(e) }

// ───────────────────────── reading (a port of math::Ev) ─────────────────────────

struct Rd<'a> { s: &'a [char], i: usize }
impl<'a> Rd<'a> {
    fn ws(&mut self) { while self.i < self.s.len() && self.s[self.i] == ' ' { self.i += 1; } }
    fn peek(&mut self) -> Option<char> { self.ws(); self.s.get(self.i).copied() }
    fn expr(&mut self) -> Option<Ex> {
        let mut v = self.term()?;
        loop {
            match self.peek() {
                Some('+') => { self.i += 1; v = Add(bx(v), bx(self.term()?)); }
                Some('-') => { self.i += 1; v = Sub(bx(v), bx(self.term()?)); }
                _ => return Some(v),
            }
        }
    }
    fn term(&mut self) -> Option<Ex> {
        let mut v = self.unary()?;
        loop {
            match self.peek() {
                Some('*') => { self.i += 1; v = Mul(bx(v), bx(self.unary()?)); }
                Some('/') => { self.i += 1; v = Div(bx(v), bx(self.unary()?)); }
                Some(c) if c.is_ascii_digit() || c == '.' || c == '(' || c.is_alphabetic() => { v = Mul(bx(v), bx(self.power()?)); }
                _ => return Some(v),
            }
        }
    }
    fn unary(&mut self) -> Option<Ex> {
        match self.peek() {
            Some('-') => { self.i += 1; Some(Neg(bx(self.unary()?))) }
            Some('+') => { self.i += 1; self.unary() }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Option<Ex> {
        let b = self.atom()?;
        if self.peek() == Some('^') { self.i += 1; let e = self.unary()?; return Some(Pow(bx(b), bx(e))); }
        Some(b)
    }
    fn arg(&mut self) -> Option<Ex> {
        if self.peek()? != '(' { return None; }
        self.i += 1; let a = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1;
        Some(a)
    }
    fn atom(&mut self) -> Option<Ex> {
        match self.peek()? {
            '(' => { self.i += 1; let v = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1; Some(v) }
            c if c.is_ascii_digit() || c == '.' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == '.') { self.i += 1; }
                let after_caret = self.s[..st].iter().rev().find(|c| !c.is_whitespace()).is_some_and(|c| *c == '^');
                if !after_caret && self.i < self.s.len() && (self.s[self.i] == 'e' || self.s[self.i] == 'E') {
                    let mut j = self.i + 1;
                    if j < self.s.len() && (self.s[j] == '+' || self.s[j] == '-' || self.s[j] == '\u{2212}') { j += 1; }
                    if j < self.s.len() && self.s[j].is_ascii_digit() {
                        while j < self.s.len() && self.s[j].is_ascii_digit() { j += 1; }
                        let t: String = self.s[st..j].iter().map(|c| if *c == '\u{2212}' { '-' } else { *c }).collect();
                        self.i = j;
                        return t.parse().ok().map(Num);
                    }
                }
                self.s[st..self.i].iter().collect::<String>().parse().ok().map(Num)
            }
            '\u{3c0}' => { self.i += 1; Some(Pi) }
            c if c.is_alphabetic() => {
                let st = self.i;
                while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() { self.i += 1; }
                let name: String = self.s[st..self.i].iter().collect();
                match name.as_str() {
                    "x" => return Some(X),
                    // (a y has no value here: f is a function of x alone)
                    "y" => return None,
                    "pi" => return Some(Pi),
                    "e" => return Some(E),
                    _ => {}
                }
                if name == "log" && self.peek() == Some('_') {
                    self.i += 1;
                    let base = self.atom()?;
                    let a = self.arg()?;
                    return Some(LogB(bx(base), bx(a)));
                }
                let name = super::super::grid_b7::inverse_spelling(self.s, &mut self.i, name);
                let known = ["sqrt", "abs", "sin", "cos", "tan", "sec", "csc", "cot", "arcsin", "arccos", "arctan", "asin", "acos", "atan", "ln", "log", "exp", "arcsec", "arccsc", "arccot", "asec", "acsc", "acot"];
                if !known.contains(&name.as_str()) {
                    if name.starts_with('x') { self.i = st + 1; return Some(X); }
                    if name.starts_with('e') && name != "exp" { self.i = st + 1; return Some(E); }
                    return None;
                }
                let a = self.arg()?;
                let f = match name.as_str() {
                    "sqrt" => F::Sqrt, "abs" => F::Abs, "ln" => F::Ln, "log" => F::Log, "exp" => F::Exp,
                    "sin" => F::Sin, "cos" => F::Cos, "tan" => F::Tan, "sec" => F::Sec, "csc" => F::Csc, "cot" => F::Cot,
                    "arcsin" | "asin" => F::Asin, "arccos" | "acos" => F::Acos, "arctan" | "atan" => F::Atan,
                    "arcsec" | "asec" => F::Asec, "arccsc" | "acsc" => F::Acsc, _ => F::Acot,
                };
                Some(Fun(f, bx(a)))
            }
            _ => None,
        }
    }
}

/// The typed expression as a tree, read as the app's reader reads it (None where the reader cannot read it).
pub(crate) fn parse(text: &str) -> Option<Ex> {
    let cs = super::super::norm_expr(text);
    // the reader's own guard against pathological strings
    if cs.len() > 4000 || (cs.len() > 200 && super::super::nest_depth(cs.iter()) > 200) { return None; }
    let mut r = Rd { s: &cs, i: 0 };
    let v = r.expr()?;
    r.ws();
    (r.i == cs.len()).then_some(v)
}

// ───────────────────────── values ─────────────────────────

/// A constant subtree's value (None when it holds x).
pub(crate) fn konst(e: &Ex) -> Option<f64> { if has_x(e) { None } else { Some(ev_raw(e, 0.0)) } }
pub(crate) fn has_x(e: &Ex) -> bool {
    match e {
        X => true,
        Num(_) | Pi | E => false,
        Neg(a) | Fun(_, a) => has_x(a),
        Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) | LogB(a, b) => has_x(a) || has_x(b),
    }
}

/// p/q in lowest terms when `v` is one with q up to `max_q` (to 1e-12).
pub(crate) fn ratio(v: f64, max_q: i64) -> Option<(i64, i64)> {
    if !v.is_finite() { return None; }
    for q in 1..=max_q {
        let n = v * q as f64;
        if (n - n.round()).abs() < 1e-11 * (1.0 + n.abs()) && n.abs() < 1e12 { return Some((n.round() as i64, q)); }
    }
    None
}
/// An exponent that is a fraction with an odd denominator above 1: (p, q).
fn odd_root(c: f64) -> Option<(i64, i64)> {
    let (p, q) = ratio(c, 15)?;
    (q > 1 && q % 2 == 1).then_some((p, q))
}

/// The value as the reader gives it (no odd-root reading): the test's reference.
pub(crate) fn ev_raw(e: &Ex, x: f64) -> f64 { ev_with(e, x, false) }
/// The value as the figure draws it: the reader's, with the real odd root for a fractional power with an odd denominator.
pub(crate) fn ev(e: &Ex, x: f64) -> f64 { ev_with(e, x, true) }
fn ev_with(e: &Ex, x: f64, odd: bool) -> f64 {
    let r = |a: &Ex| ev_with(a, x, odd);
    match e {
        Num(v) => *v,
        X => x,
        Pi => PI,
        E => EULER,
        Neg(a) => -r(a),
        Add(a, b) => r(a) + r(b),
        Sub(a, b) => r(a) - r(b),
        Mul(a, b) => r(a) * r(b),
        // (the figure's own values: a division by exactly 0 has no value, as e^(-1/x²) at 0, where the reader's 1/0 = ∞ would
        // give e^(-∞) = 0; the reader's own values are left as the reader gives them)
        Div(a, b) => { let d = r(b); if odd && d == 0.0 { f64::NAN } else { r(a) / d } }
        Pow(a, b) => {
            let (u, c) = (r(a), r(b));
            // A typed partial expression must have a value before applying a power. IEEE NaN^0=1 and
            // 0^negative=∞ must not turn an undefined logarithm/reciprocal into a real function value.
            if odd && (u.is_nan() || c.is_nan() || (u == 0.0 && c < 0.0)) { return f64::NAN; }
            if odd && u < 0.0 && !has_x(b) {
                if let Some((p, _)) = odd_root(c) { let m = (-u).powf(c); return if p % 2 == 0 { m } else { -m }; }
            }
            u.powf(c)
        }
        Fun(f, a) => {
            let a = r(a);
            match f {
                // (the figure's own values: ln 0 has no value, so e^(ln x) has none at 0)
                F::Ln if odd && a == 0.0 => f64::NAN, F::Log if odd && a == 0.0 => f64::NAN,
                F::Sqrt => a.sqrt(), F::Abs => a.abs(), F::Ln => a.ln(), F::Log => a.log10(), F::Exp => a.exp(),
                F::Sin => a.sin(), F::Cos => a.cos(), F::Tan => a.tan(),
                F::Sec => 1.0 / a.cos(), F::Csc => 1.0 / a.sin(), F::Cot => 1.0 / a.tan(),
                F::Asin => a.asin(), F::Acos => a.acos(), F::Atan => a.atan(),
                F::Asec => super::super::grid_b7::arc_recip("arcsec", a),
                F::Acsc => super::super::grid_b7::arc_recip("arccsc", a),
                F::Acot => super::super::grid_b7::arc_recip("arccot", a),
            }
        }
        LogB(b, a) => r(a).ln() / r(b).ln(),
    }
}

// ───────────────────────── building, with the plain simplifications ─────────────────────────

fn is_num(e: &Ex, v: f64) -> bool { matches!(e, Num(n) if *n == v) }
pub(crate) fn num(v: f64) -> Ex { if v < 0.0 { Neg(bx(Num(-v))) } else { Num(v) } }
fn val(e: &Ex) -> Option<f64> { match e { Num(v) => Some(*v), Neg(a) => match a.as_ref() { Num(v) => Some(-v), _ => None }, _ => None } }
pub(crate) fn add(a: Ex, b: Ex) -> Ex {
    // (two whole numbers only: 1/3 stays a ratio, 0.1 stays as typed, so exact values read back exactly)
    if let (Some(x), Some(y)) = (val(&a), val(&b)) { if x.fract() == 0.0 && y.fract() == 0.0 && (x + y).abs() < 1e15 { return num(x + y); } }
    if val(&a) == Some(0.0) { return b; }
    if val(&b) == Some(0.0) { return a; }
    if let Neg(nb) = b { return sub(a, *nb); }
    Add(bx(a), bx(b))
}
pub(crate) fn sub(a: Ex, b: Ex) -> Ex {
    // (two whole numbers only: 1/3 stays a ratio, 0.1 stays as typed, so exact values read back exactly)
    if let (Some(x), Some(y)) = (val(&a), val(&b)) { if x.fract() == 0.0 && y.fract() == 0.0 && (x - y).abs() < 1e15 { return num(x - y); } }
    if val(&b) == Some(0.0) { return a; }
    if val(&a) == Some(0.0) { return neg(b); }
    if let Neg(nb) = b { return add(a, *nb); }
    Sub(bx(a), bx(b))
}
pub(crate) fn neg(a: Ex) -> Ex {
    if let Some(v) = val(&a) { return num(-v); }
    if let Neg(x) = a { return *x; }
    Neg(bx(a))
}
pub(crate) fn mul(a: Ex, b: Ex) -> Ex {
    // (two whole numbers only: 1/3 stays a ratio, 0.1 stays as typed, so exact values read back exactly)
    if let (Some(x), Some(y)) = (val(&a), val(&b)) { if x.fract() == 0.0 && y.fract() == 0.0 && (x * y).abs() < 1e15 { return num(x * y); } }
    if val(&a) == Some(0.0) || val(&b) == Some(0.0) { return Num(0.0); }
    if val(&a) == Some(1.0) { return b; }
    if val(&b) == Some(1.0) { return a; }
    if val(&a) == Some(-1.0) { return neg(b); }
    if val(&b) == Some(-1.0) { return neg(a); }
    // a number always leads, and two numbers meet: 2 * (3 * u) is 6 * u
    if val(&a).is_none() && val(&b).is_some() { return mul(b, a); }
    if let (Some(x), Mul(c, d)) = (val(&a), &b) { if let Some(y) = val(c) { if x.fract() == 0.0 && y.fract() == 0.0 { return mul(num(x * y), (**d).clone()); } } }
    if let Neg(x) = a { return neg(mul(*x, b)); }
    if let Neg(y) = b { return neg(mul(a, *y)); }
    Mul(bx(a), bx(b))
}
pub(crate) fn div(a: Ex, b: Ex) -> Ex {
    if val(&b) == Some(1.0) { return a; }
    if val(&a) == Some(0.0) && val(&b).is_some_and(|v| v != 0.0) { return Num(0.0); }
    if let (Some(x), Some(y)) = (val(&a), val(&b)) {
        // a ratio of whole numbers stays a ratio (1/3 is never 0.333...)
        if y != 0.0 && (x / y).fract() == 0.0 { return num(x / y); }
    }
    Div(bx(a), bx(b))
}
pub(crate) fn pow(a: Ex, b: Ex) -> Ex {
    if val(&b) == Some(1.0) { return a; }
    if val(&b) == Some(0.0) { return Num(1.0); }
    Pow(bx(a), bx(b))
}
fn fun(f: F, a: Ex) -> Ex { Fun(f, bx(a)) }

// ───────────────────────── derivatives ─────────────────────────

/// d/dx of `e`, by the usual rules. A constant subtree gives 0.
pub(crate) fn deriv(e: &Ex) -> Ex {
    if !has_x(e) { return Num(0.0); }
    match e {
        X => Num(1.0),
        Num(_) | Pi | E => Num(0.0),
        Neg(a) => neg(deriv(a)),
        Add(a, b) => add(deriv(a), deriv(b)),
        Sub(a, b) => sub(deriv(a), deriv(b)),
        Mul(a, b) => {
            if !has_x(a) { return mul((**a).clone(), deriv(b)); }
            if !has_x(b) { return mul((**b).clone(), deriv(a)); }
            add(mul(deriv(a), (**b).clone()), mul((**a).clone(), deriv(b)))
        }
        Div(a, b) => {
            if !has_x(b) { return div(deriv(a), (**b).clone()); }
            // (a'b - ab') / b^2
            div(sub(mul(deriv(a), (**b).clone()), mul((**a).clone(), deriv(b))), pow((**b).clone(), Num(2.0)))
        }
        Pow(a, b) => {
            if !has_x(b) {
                // c u^(c - 1) u', the exponent kept exact: x^(1/3) gives (1/3) x^((1/3) - 1)
                let c = (**b).clone();
                let cm1 = match val(&c) { Some(v) => num(v - 1.0), None => sub(c.clone(), Num(1.0)) };
                return mul(mul(c, pow((**a).clone(), cm1)), deriv(a));
            }
            if !has_x(a) {
                // a^v ln(a) v'
                return mul(mul(e.clone(), fun(F::Ln, (**a).clone())), deriv(b));
            }
            // u^v (v' ln u + v u'/u)
            mul(e.clone(), add(mul(deriv(b), fun(F::Ln, (**a).clone())), div(mul((**b).clone(), deriv(a)), (**a).clone())))
        }
        LogB(base, a) => div(deriv(a), mul((**a).clone(), fun(F::Ln, (**base).clone()))),
        Fun(f, a) => {
            let u = (**a).clone();
            let du = deriv(a);
            let outer = match f {
                F::Sqrt => div(Num(1.0), mul(Num(2.0), fun(F::Sqrt, u))),
                // u/|u|: no value where u = 0, where |u| has a corner
                F::Abs => div(u.clone(), fun(F::Abs, u)),
                F::Sin => fun(F::Cos, u),
                F::Cos => neg(fun(F::Sin, u)),
                F::Tan => pow(fun(F::Sec, u), Num(2.0)),
                F::Sec => mul(fun(F::Sec, u.clone()), fun(F::Tan, u)),
                F::Csc => neg(mul(fun(F::Csc, u.clone()), fun(F::Cot, u))),
                F::Cot => neg(pow(fun(F::Csc, u), Num(2.0))),
                F::Asin => div(Num(1.0), fun(F::Sqrt, sub(Num(1.0), pow(u, Num(2.0))))),
                F::Acos => neg(div(Num(1.0), fun(F::Sqrt, sub(Num(1.0), pow(u, Num(2.0)))))),
                F::Atan => div(Num(1.0), add(Num(1.0), pow(u, Num(2.0)))),
                F::Asec => div(Num(1.0), mul(fun(F::Abs, u.clone()), fun(F::Sqrt, sub(pow(u, Num(2.0)), Num(1.0))))),
                F::Acsc => neg(div(Num(1.0), mul(fun(F::Abs, u.clone()), fun(F::Sqrt, sub(pow(u, Num(2.0)), Num(1.0)))))),
                F::Acot => neg(div(Num(1.0), add(Num(1.0), pow(u, Num(2.0))))),
                F::Ln => div(Num(1.0), u),
                F::Log => div(Num(1.0), mul(u, fun(F::Ln, Num(10.0)))),
                F::Exp => fun(F::Exp, u),
            };
            mul(outer, du)
        }
    }
}

// ───────────────────────── printing for the reader ─────────────────────────

fn num_text(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 { return format!("{}", v as i64); }
    if let Some((p, q)) = ratio(v, 1000) { return format!("({p}/{q})"); }
    // shortest form that reads back to the same number ({:?} may use an exponent, which the reader reads)
    format!("({v:?})")
}
fn fname(f: F) -> &'static str {
    match f {
        F::Sqrt => "sqrt", F::Abs => "abs", F::Sin => "sin", F::Cos => "cos", F::Tan => "tan", F::Sec => "sec", F::Csc => "csc", F::Cot => "cot",
        F::Asin => "arcsin", F::Acos => "arccos", F::Atan => "arctan", F::Asec => "arcsec", F::Acsc => "arccsc", F::Acot => "arccot",
        F::Ln => "ln", F::Log => "log", F::Exp => "exp",
    }
}
/// The expression for the reader: every operand bracketed and every product starred, so it reads exactly as built;
/// a fractional power with an odd denominator written as the real odd root (see the module note).
pub(crate) fn print(e: &Ex) -> String {
    match e {
        Num(v) => if *v < 0.0 { format!("(-{})", num_text(-v)) } else { num_text(*v) },
        X => "x".into(),
        Pi => "pi".into(),
        E => "e".into(),
        Neg(a) => format!("(-({}))", print(a)),
        Add(a, b) => format!("({})+({})", print(a), print(b)),
        Sub(a, b) => format!("({})-({})", print(a), print(b)),
        Mul(a, b) => format!("({})*({})", print(a), print(b)),
        Div(a, b) => format!("({})/({})", print(a), print(b)),
        Pow(a, b) => {
            let (u, c) = (print(a), print(b));
            match konst(b).and_then(odd_root) {
                // |u|^c, with the sign of u for an odd p (u/(|u| + 1e-300) is the sign, and 0 at u = 0)
                Some((p, _)) if p % 2 == 0 => format!("(abs({u}))^({c})"),
                Some(_) => format!("(({u})/(abs({u})+1e-300))*(abs({u}))^({c})"),
                None => format!("({u})^({c})"),
            }
        }
        Fun(f, a) => format!("{}({})", fname(*f), print(a)),
        LogB(b, a) => format!("ln({})/ln({})", print(a), print(b)),
    }
}

/// Every argument of |u| and every base of an odd root with a power under 1 (where a corner or a cusp can be).
pub(crate) fn kink_args(e: &Ex, out: &mut Vec<Ex>) {
    match e {
        Fun(F::Abs, a) => { out.push((**a).clone()); kink_args(a, out); }
        Pow(a, b) => {
            if let Some(c) = konst(b) { if c < 1.0 && c > 0.0 && has_x(a) { out.push((**a).clone()); } }
            kink_args(a, out); kink_args(b, out);
        }
        Neg(a) | Fun(_, a) => kink_args(a, out),
        Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | LogB(a, b) => { kink_args(a, out); kink_args(b, out); }
        _ => {}
    }
}

/// The expressions whose zeros make a derivative singular or kinked: the base of a power with a non-whole exponent (of any
/// sign: sin(x)^(−1/3) in sin(x)^(5/3)″), the inside of |u|, and a denominator.
pub(crate) fn singular_bases(e: &Ex, out: &mut Vec<Ex>) {
    match e {
        Fun(F::Abs, a) => { if has_x(a) { out.push((**a).clone()); } singular_bases(a, out); }
        Fun(F::Sqrt, a) => { if has_x(a) { out.push((**a).clone()); } singular_bases(a, out); }
        Pow(a, b) => {
            if let Some(c) = konst(b) { if c.fract() != 0.0 && has_x(a) { out.push((**a).clone()); } }
            singular_bases(a, out); singular_bases(b, out);
        }
        Div(a, b) => { if has_x(b) { out.push((**b).clone()); } singular_bases(a, out); singular_bases(b, out); }
        Neg(a) | Fun(_, a) => singular_bases(a, out),
        Add(a, b) | Sub(a, b) | Mul(a, b) | LogB(a, b) => { singular_bases(a, out); singular_bases(b, out); }
        _ => {}
    }
}

/// How many nodes: a guard against derivatives too long to draw.
pub(crate) fn size(e: &Ex) -> usize {
    match e {
        Num(_) | X | Pi | E => 1,
        Neg(a) | Fun(_, a) => 1 + size(a),
        Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) | LogB(a, b) => 1 + size(a) + size(b),
    }
}


// ───────────────────────── plain identities, and the rounding in a value ─────────────────────────

/// The plain identities that keep a function's domain: √(u²) = |u|, ln(e^u) = u. With `deriv` (a derivative, whose
/// domain is f's own), also e^(ln u) = u (u > 0 wherever ln u has a value), u − u = 0 and u/u = 1. A derivative typed in
/// a roundabout form (√(x²), e^(ln x)) is then exact where its rounding would be noise.
pub(crate) fn simplify(e: &Ex, deriv: bool) -> Ex {
    let s = |a: &Ex| simplify(a, deriv);
    match e {
        Num(_) | X | Pi | E => e.clone(),
        Neg(a) => neg(s(a)),
        Add(a, b) => add(s(a), s(b)),
        Sub(a, b) => { let (a, b) = (s(a), s(b)); if deriv && a == b { Num(0.0) } else { sub(a, b) } }
        Mul(a, b) => {
            let (a,b)=(s(a),s(b));
            // 0·ln x is zero only where ln x exists. Preserve this obligation in the typed function;
            // derivatives may simplify it, since their rows retain the typed domain separately.
            if !deriv && ((val(&a)==Some(0.0) && super::domain::has_constraints(&b)) || (val(&b)==Some(0.0) && super::domain::has_constraints(&a))) {
                Mul(bx(a),bx(b))
            } else { merge_powers(mul(a,b),deriv) }
        },
        Div(a, b) => { let (a, b) = (s(a), s(b)); if deriv && a == b && has_x(&a) { Num(1.0) } else { div(a, b) } }
        Pow(a, b) => {
            let (a, b) = (s(a), s(b));
            if !deriv && val(&b)==Some(0.0) && super::domain::has_constraints(&a) { return Pow(bx(a),bx(b)); }
            if deriv && a == E { if let Fun(F::Ln, u) = &b { return (**u).clone(); } }
            // (√u to an even power, where √u has a value: (√x)² = x)
            if deriv { if let (Fun(F::Sqrt, u), Some(k)) = (&a, val(&b)) { if k > 0.0 && k.fract() == 0.0 && (k as i64) % 2 == 0 { return if k == 2.0 { (**u).clone() } else { pow((**u).clone(), Num(k / 2.0)) }; } } }
            // (|u| to an even power is u to it: |x|² = x²)
            if let (Fun(F::Abs, u), Some(k)) = (&a, val(&b)) { if k > 0.0 && k.fract() == 0.0 && (k as i64) % 2 == 0 { return pow((**u).clone(), b.clone()); } }
            pow(a, b)
        }
        LogB(b, a) => LogB(bx(s(b)), bx(s(a))),
        Fun(f, a) => {
            let a = s(a);
            match (f, &a) {
                (F::Sqrt, Pow(u, k)) if val(k) == Some(2.0) => Fun(F::Abs, u.clone()),
                // (√(u⁴) = u², √(u⁶) = |u|³: an even power under the root, defined everywhere either way)
                (F::Sqrt, Pow(u, k)) if val(k).is_some_and(|k| k > 2.0 && k.fract() == 0.0 && (k as i64) % 2 == 0) => {
                    let h = val(k).unwrap() / 2.0;
                    if (h as i64) % 2 == 0 { pow((**u).clone(), Num(h)) } else { pow(Fun(F::Abs, u.clone()), Num(h)) }
                }
                (F::Ln, Fun(F::Exp, u)) => (**u).clone(),
                (F::Ln, Pow(base, u)) if **base == E => (**u).clone(),
                (F::Exp, Fun(F::Ln, u)) if deriv => (**u).clone(),
                _ => Fun(*f, bx(a)),
            }
        }
    }
}

/// A product's factors with one base gathered into one power: x·x^(1/3) is x^(4/3), (x − 2)^(1/3)(x − 2) is
/// (x − 2)^(4/3), x²·x^(1/3) is x^(7/3), with the exponent kept as a ratio. Only where the domain is the same: both
/// powers' denominators odd (real odd roots), or the sum's still even (√x·x = x^(3/2), both for x ≥ 0); for a derivative
/// (whose domain is f's own) always. Positive powers only. Unchanged when nothing gathers.
fn merge_powers(e: Ex, deriv: bool) -> Ex {
    fn factors(e: &Ex, out: &mut Vec<Ex>) { match e { Mul(a, b) => { factors(a, out); factors(b, out); } _ => out.push(e.clone()) } }
    let mut fs = vec![];
    factors(&e, &mut fs);
    if fs.len() < 2 { return e; }
    // (base, exponent as p/q)
    let split = |f: &Ex| -> Option<(Ex, i64, i64)> {
        if !has_x(f) { return None; }
        match f {
            Pow(u, k) => { let (p, q) = ratio(konst(k)?, 60)?; (p > 0).then(|| ((**u).clone(), p, q)) }
            Fun(F::Sqrt, u) => Some(((**u).clone(), 1, 2)),
            _ => Some((f.clone(), 1, 1)),
        }
    };
    fn gcd(a: i64, b: i64) -> i64 { if b == 0 { a.abs() } else { gcd(b, a % b) } }
    let mut done = vec![false; fs.len()];
    let mut out: Vec<Ex> = vec![];
    let mut merged = false;
    for i in 0..fs.len() {
        if done[i] { continue; }
        let Some((base, mut p, mut q)) = split(&fs[i]) else { out.push(fs[i].clone()); continue };
        let mut any = false;
        for j in i + 1..fs.len() {
            if done[j] { continue; }
            let Some((b2, p2, q2)) = split(&fs[j]) else { continue };
            if b2 != base { continue; }
            let (np, nq) = (p * q2 + p2 * q, q * q2);
            let g = gcd(np, nq);
            let (np, nq) = (np / g, nq / g);
            let ok = deriv || (q % 2 == 1 && q2 % 2 == 1) || nq % 2 == 0;
            if !ok || nq > 60 { continue; }
            p = np; q = nq; done[j] = true; any = true;
        }
        if any {
            merged = true;
            out.push(if q == 1 { pow(base, Num(p as f64)) } else { Pow(bx(base), bx(Div(bx(Num(p as f64)), bx(Num(q as f64))))) });
        } else { out.push(fs[i].clone()); }
    }
    if !merged { return e; }
    // (numbers first, as mul writes them)
    out.sort_by_key(|f| has_x(f));
    out.into_iter().reduce(mul).unwrap_or(e)
}

/// The value as the figure evaluates it, with a bound on the rounding in it: each operation's own rounding (half a unit
/// in the last place of its result, taken as a whole unit) and the rounding of its operands carried through it, as the
/// operation's slope magnifies them. A value smaller than its bound has no sign that can be trusted.
pub(crate) fn ev_err(e: &Ex, x: f64) -> (f64, f64) {
    const U: f64 = f64::EPSILON;
    let r = |a: &Ex| ev_err(a, x);
    let (v, err) = match e {
        Num(c) => (*c, c.abs() * U),
        X => (x, 0.0),
        Pi => (PI, PI * U),
        E => (EULER, EULER * U),
        Neg(a) => { let (va, ea) = r(a); (-va, ea) }
        Add(a, b) => { let ((va, ea), (vb, eb)) = (r(a), r(b)); let v = va + vb; (v, ea + eb + v.abs() * U) }
        Sub(a, b) => { let ((va, ea), (vb, eb)) = (r(a), r(b)); let v = va - vb; (v, ea + eb + v.abs() * U) }
        Mul(a, b) => { let ((va, ea), (vb, eb)) = (r(a), r(b)); let v = va * vb; (v, va.abs() * eb + vb.abs() * ea + ea * eb + v.abs() * U) }
        Div(a, b) => {
            let ((va, ea), (vb, eb)) = (r(a), r(b));
            if vb == 0.0 { (f64::NAN, f64::INFINITY) } else {
                let v = va / vb;
                (v, if vb.abs() <= eb { f64::INFINITY } else { (ea + v.abs() * eb) / (vb.abs() - eb) + v.abs() * U })
            }
        }
        Pow(a, b) => {
            let ((u, ea), (c, eb)) = (r(a), r(b));
            let v = if u < 0.0 && !has_x(b) { match odd_root(c) { Some((p, _)) => { let m = (-u).powf(c); if p % 2 == 0 { m } else { -m } } None => u.powf(c) } } else { u.powf(c) };
            let err = if !has_x(b) {
                if u == 0.0 { if c > 0.0 { ea.powf(c) } else { f64::INFINITY } } else { v.abs() * c.abs() * ea / u.abs() + v.abs() * U * (1.0 + c.abs()) }
            } else if u == 0.0 { f64::INFINITY } else {
                let l = u.abs().ln();
                v.abs() * (l.abs() * eb + c.abs() * ea / u.abs()) + v.abs() * U * (1.0 + (c * l).abs())
            };
            (v, err)
        }
        LogB(b, a) => {
            let ((va, ea), (vb, eb)) = (r(a), r(b));
            let (la, lb) = (va.ln(), vb.ln());
            let v = la / lb;
            (v, if lb == 0.0 { f64::INFINITY } else { (ea / va.abs() + v.abs() * eb / vb.abs()) / lb.abs() + v.abs() * U })
        }
        Fun(f, a) => {
            let (va, ea) = r(a);
            let v = match f {
                F::Ln | F::Log if va == 0.0 => f64::NAN,
                F::Sqrt => va.sqrt(), F::Abs => va.abs(), F::Ln => va.ln(), F::Log => va.log10(), F::Exp => va.exp(),
                F::Sin => va.sin(), F::Cos => va.cos(), F::Tan => va.tan(),
                F::Sec => 1.0 / va.cos(), F::Csc => 1.0 / va.sin(), F::Cot => 1.0 / va.tan(),
                F::Asin => va.asin(), F::Acos => va.acos(), F::Atan => va.atan(),
                F::Asec => super::super::grid_b7::arc_recip("arcsec", va),
                F::Acsc => super::super::grid_b7::arc_recip("arccsc", va),
                F::Acot => super::super::grid_b7::arc_recip("arccot", va),
            };
            // the slope of the outer function at va carries the operand's rounding through
            let slope = match f {
                F::Sqrt => if va > 0.0 { 0.5 / va.sqrt() } else { return (v, ea.sqrt() + v.abs() * U) },
                F::Abs => 1.0,
                F::Sin => va.cos().abs(),
                F::Cos => va.sin().abs(),
                F::Tan | F::Sec => { let c = va.cos(); (1.0 + va.sin().abs()) / (c * c) }
                F::Csc | F::Cot => { let s = va.sin(); (1.0 + va.cos().abs()) / (s * s) }
                F::Asin | F::Acos => 1.0 / (1.0 - va * va).abs().sqrt(),
                F::Atan | F::Acot => 1.0 / (1.0 + va * va),
                F::Asec | F::Acsc => 1.0 / (va.abs() * (va * va - 1.0).abs().sqrt()),
                F::Ln => 1.0 / va.abs(),
                F::Log => 1.0 / (va.abs() * std::f64::consts::LN_10),
                F::Exp => v.abs(),
            };
            (v, slope * ea + v.abs() * U)
        }
    };
    (v, if err.is_nan() { f64::INFINITY } else { err })
}
