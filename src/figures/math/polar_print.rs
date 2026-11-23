//! Printing a curve's equation from its parsed tree, for the polar figure (`polar-parametric`).
//!
//! Two printers: a precedence-aware one (brackets only where precedence, associativity or the figure's own reading
//! conventions need them: a quotient that is a factor of a product, a denominator that is not one letter, number or
//! function, an exponent that is not one letter or a whole number) and a fully bracketed one (every operation bracketed,
//! `·` between every product, abs(…) for bars, every function's argument bracketed). Neither is trusted on its own: the
//! figure prints only a text that, typed back, its reader accepts and reads as the same curve (`polar::verified_print`).
use super::super::tangent_expr::{Node, R};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum K { Add, Neg, Div, Mul, Pow, Atom, Func }

/// The tree as a printed expression in the variable `var` (θ or t); `full` brackets every operation.
pub(crate) fn tree(n: &Node, var: &str, full: bool) -> String { pr(n, var, full).0 }

fn number(v: f64, r: &Option<R>) -> (String, K) {
    if let Some(R(p, q)) = r {
        let (p, q) = (*p, *q);
        let sign = if p < 0 { "\u{2212}" } else { "" };
        let k = if p < 0 { K::Neg } else { K::Atom };
        if q == 1 { return (format!("{sign}{}", p.abs()), k); }
        // a terminating decimal as a decimal (0.5), else p/q (a quotient)
        let (mut d, mut places) = (q, 0u32);
        while d % 10 == 0 { d /= 10; places += 1; }
        if d == 1 && places <= 12 {
            let scale = 10i128.pow(places);
            let (ip, fp) = (p.abs() / scale, p.abs() % scale);
            return (format!("{sign}{ip}.{}", format!("{fp:0w$}", w = places as usize).trim_end_matches('0')), k);
        }
        return (format!("{sign}{}/{q}", p.abs()), if p < 0 { K::Neg } else { K::Div });
    }
    let s = format!("{}", v.abs());
    (if v < 0.0 { format!("\u{2212}{s}") } else { s }, if v < 0.0 { K::Neg } else { K::Atom })
}
fn br(s: &str) -> String { format!("({s})") }
fn single(s: &str) -> bool { s.chars().count() == 1 }
fn integer(s: &str) -> bool { !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) }

fn pr(n: &Node, var: &str, full: bool) -> (String, K) {
    use Node::*;
    match n {
        Num(v, r) => number(*v, r),
        X => (var.to_string(), K::Atom),
        Pi => ("\u{3c0}".into(), K::Atom),
        Euler => ("e".into(), K::Atom),
        Neg(a) => {
            let (s, k) = pr(a, var, full);
            let s = if full || matches!(k, K::Add | K::Neg) { br(&s) } else { s };
            let out = format!("\u{2212}{s}");
            (if full { br(&out) } else { out }, K::Neg)
        }
        Add(a, b) | Sub(a, b) => {
            let sub = matches!(n, Sub(..));
            let (sa, _) = pr(a, var, full);
            let (sb, kb) = pr(b, var, full);
            // (a + (−b), a − (b + c), a − (−b))
            let sb = if !full && (kb == K::Neg || (sub && kb == K::Add)) { br(&sb) } else { sb };
            let out = format!("{sa} {} {sb}", if sub { "\u{2212}" } else { "+" });
            (if full { br(&out) } else { out }, K::Add)
        }
        Mul(a, b) => {
            let (sa, ka) = pr(a, var, full);
            let (sb, kb) = pr(b, var, full);
            if full { return (br(&format!("{sa}\u{b7}{sb}")), K::Mul); }
            let sa = if matches!(ka, K::Add | K::Neg | K::Div) { br(&sa) } else { sa };
            let sb = if matches!(kb, K::Add | K::Neg | K::Div) { br(&sb) } else { sb };
            // juxtaposed only after a whole number and before a letter, π, a root, a function or a bracket: 2θ, 3 cos(θ)
            let next = sb.chars().next().unwrap_or(' ');
            let jux = integer(&sa) && (next.is_alphabetic() || next == '\u{221a}' || next == '(');
            let sep = if !jux { "\u{b7}" } else if next.is_alphabetic() && sb.chars().nth(1).is_some_and(|c| c.is_alphabetic()) { " " } else { "" };
            (format!("{sa}{sep}{sb}"), K::Mul)
        }
        Div(a, b) => {
            let (sa, ka) = pr(a, var, full);
            let (sb, kb) = pr(b, var, full);
            if full { return (br(&format!("{}/{}", br(&sa), br(&sb))), K::Div); }
            let sa = if matches!(ka, K::Add | K::Neg | K::Div) { br(&sa) } else { sa };
            // the denominator is one letter, a whole number or a function, or it is bracketed
            let plain_den = (kb == K::Atom && (single(&sb) || integer(&sb))) || kb == K::Func;
            let sb = if plain_den { sb } else { br(&sb) };
            (format!("{sa}/{sb}"), K::Div)
        }
        Pow(a, b) => {
            let (sa, ka) = pr(a, var, full);
            let (sb, kb) = pr(b, var, full);
            if full { return (br(&format!("{}^{}", br(&sa), br(&sb))), K::Pow); }
            let sa = if matches!(ka, K::Atom | K::Func) && !sa.starts_with('\u{2212}') { sa } else { br(&sa) };
            let sb = if kb == K::Atom && (single(&sb) || integer(&sb)) { sb } else { br(&sb) };
            (format!("{sa}^{sb}"), K::Pow)
        }
        Func(name, a) => {
            let (s, k) = pr(a, var, full);
            let out = match *name {
                "abs" if full || s.contains('|') => format!("abs({s})"),
                "abs" => format!("|{s}|"),
                "sqrt" if !full && k == K::Atom && (single(&s) || integer(&s)) => format!("\u{221a}{s}"),
                "sqrt" => format!("\u{221a}({s})"),
                other => format!("{other}({s})"),
            };
            (out, K::Func)
        }
        LogB(b, a) => {
            let (sa, _) = pr(a, var, full);
            let (sb, _) = pr(b, var, full);
            let out = format!("ln({sa})/ln({sb})");
            (if full { br(&out) } else { out }, K::Div)
        }
    }
}
