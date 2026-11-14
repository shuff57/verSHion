//! Algebraic limit proofs for the sequence figure. No sampled-value recognition.
use super::*;
#[derive(Clone)]
struct R {
    a: Vec<N>,
    b: Vec<N>,
}
fn trim(mut a: Vec<N>) -> Vec<N> {
    while a.len() > 1 && a.last().unwrap().q == Some((0, 1)) {
        a.pop();
    }
    a
}
fn add(a: &[N], b: &[N], minus: bool) -> Option<Vec<N>> {
    let mut out = vec![N::int(0); a.len().max(b.len())];
    for (i, v) in out.iter_mut().enumerate() {
        *v = a.get(i).copied().unwrap_or(N::int(0)).op(
            b.get(i).copied().unwrap_or(N::int(0)),
            if minus { '-' } else { '+' },
        );
        v.q?;
    }
    Some(trim(out))
}
fn mul(a: &[N], b: &[N]) -> Option<Vec<N>> {
    if a.len() + b.len() > 34 {
        return None;
    }
    let mut out = vec![N::int(0); a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            out[i + j] = out[i + j].op(x.op(*y, '*'), '+');
            out[i + j].q?;
        }
    }
    Some(trim(out))
}
impl R {
    fn constant(n: N) -> Self {
        Self {
            a: vec![n],
            b: vec![N::int(1)],
        }
    }
    fn op(self, r: Self, c: char) -> Option<Self> {
        let out = match c {
            '+' | '-' => Self {
                a: add(&mul(&self.a, &r.b)?, &mul(&r.a, &self.b)?, c == '-')?,
                b: mul(&self.b, &r.b)?,
            },
            '*' => Self {
                a: mul(&self.a, &r.a)?,
                b: mul(&self.b, &r.b)?,
            },
            '/' => Self {
                a: mul(&self.a, &r.b)?,
                b: mul(&self.b, &r.a)?,
            },
            _ => return None,
        };
        if out.b.iter().all(|x| x.q == Some((0, 1))) {
            None
        } else {
            Some(out)
        }
    }
}
struct L<'a> {
    s: &'a [u8],
    i: usize,
    var: u8,
}
impl L<'_> {
    fn eat(&mut self, c: u8) -> bool {
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Option<R> {
        let mut a = self.term()?;
        loop {
            if self.eat(b'+') {
                a = a.op(self.term()?, '+')?
            } else if self.eat(b'-') {
                a = a.op(self.term()?, '-')?
            } else {
                return Some(a);
            }
        }
    }
    fn term(&mut self) -> Option<R> {
        let mut a = self.unary()?;
        loop {
            if self.eat(b'*') {
                a = a.op(self.unary()?, '*')?
            } else if self.eat(b'/') {
                a = a.op(self.unary()?, '/')?
            } else if self
                .s
                .get(self.i)
                .is_some_and(|c| *c == self.var || *c == b'(')
            {
                a = a.op(self.unary()?, '*')?
            } else {
                return Some(a);
            }
        }
    }
    fn unary(&mut self) -> Option<R> {
        if self.eat(b'-') {
            return R::constant(N::int(0)).op(self.unary()?, '-');
        }
        if self.eat(b'+') {
            return self.unary();
        }
        let a = self.atom()?;
        if !self.eat(b'^') {
            return Some(a);
        }
        let par = self.eat(b'(');
        let negative = self.eat(b'-');
        let begin = self.i;
        while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
            self.i += 1;
        }
        let k = std::str::from_utf8(&self.s[begin..self.i])
            .ok()?
            .parse::<usize>()
            .ok()?;
        if k > 16 || par && !self.eat(b')') {
            return None;
        }
        let mut out = R::constant(N::int(1));
        for _ in 0..k {
            out = out.op(a.clone(), '*')?;
        }
        if negative {
            R::constant(N::int(1)).op(out, '/')
        } else {
            Some(out)
        }
    }
    fn atom(&mut self) -> Option<R> {
        if self.eat(b'(') {
            let a = self.expr()?;
            return if self.eat(b')') { Some(a) } else { None };
        }
        if self.eat(self.var) {
            return Some(R {
                a: vec![N::int(0), N::int(1)],
                b: vec![N::int(1)],
            });
        }
        let begin = self.i;
        while self
            .s
            .get(self.i)
            .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
        {
            self.i += 1;
        }
        if self.s.get(self.i).is_some_and(|c| matches!(c, b'e' | b'E')) {
            self.i += 1;
            if self.s.get(self.i).is_some_and(|c| matches!(c, b'+' | b'-')) {
                self.i += 1;
            }
            while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                self.i += 1;
            }
        }
        let n = number(std::str::from_utf8(&self.s[begin..self.i]).ok()?)?;
        n.q?;
        Some(R::constant(n))
    }
}
fn canonical(s: &str) -> String {
    recurrence_spelling(s)
        .replace(' ', "")
        .replace('²', "^2")
        .replace('³', "^3")
        .replace('π', "pi")
        .replace('×', "*")
        .replace('·', "*")
}
fn rational(s: &str, var: u8) -> Option<R> {
    let mut l = L {
        s: s.as_bytes(),
        i: 0,
        var,
    };
    let r = l.expr()?;
    if l.i == l.s.len() { Some(r) } else { None }
}
fn peel(mut s: &str) -> &str {
    loop {
        if !s.starts_with('(') || !s.ends_with(')') {
            return s;
        }
        let mut d = 0;
        let mut whole = true;
        for (i, c) in s.char_indices() {
            if c == '(' {
                d += 1;
            } else if c == ')' {
                d -= 1;
                if d == 0 && i + 1 < s.len() {
                    whole = false;
                    break;
                }
            }
        }
        if !whole {
            return s;
        }
        s = &s[1..s.len() - 1];
    }
}
fn finite(n: N) -> (Option<N>, String) {
    (Some(n), "converges".into())
}
fn explicit(s: &str) -> (Option<N>, String) {
    let source = canonical(s);
    let s = peel(&source);
    if let Some(r) = rational(s, b'n') {
        let a = r.a.last().unwrap();
        let b = r.b.last().unwrap();
        return if r.a.len() < r.b.len() || r.a.iter().all(|n| n.q == Some((0, 1))) {
            finite(N::int(0))
        } else if r.a.len() == r.b.len() {
            finite(a.op(*b, '/'))
        } else {
            (None, "diverges".into())
        };
    }
    // Each identity is a standard algebraic/squeeze limit, not a fit to the plotted terms.
    match s {
        "(1+1/n)^n" => {
            return finite(N {
                v: std::f64::consts::E,
                q: None,
                dec: true,
            });
        }
        "n^(1/n)" => return finite(N::int(1)),
        "sqrt(n+1)-sqrt(n)" | "√(n+1)-√n" | "ln(n)/n" | "exp(-n)" | "e^(-n)" | "1/2^n"
        | "n/2^n" | "(-1)^n/n" => return finite(N::int(0)),
        "n*sin(1/n)" | "1-1/2^n" => return finite(N::int(1)),
        "n*sin(pi/n)" => {
            return finite(N {
                v: std::f64::consts::PI,
                q: None,
                dec: true,
            });
        }
        "(-1)^n" | "sin(n)" | "cos(n)" => return (None, "diverges (oscillates)".into()),
        _ => (),
    }
    if let Some(base) = s.strip_suffix("^n").and_then(|b| number(peel(b))) {
        if base.q.is_some_and(|(a, b)| a == b) {
            return finite(N::int(1));
        }
        if base
            .q
            .map_or(base.v.abs() < 1.0 - 1e-12, |(a, b)| a.abs() < b)
        {
            return finite(N::int(0));
        }
        if base.q.is_some() || base.v.abs() > 1.0 + 1e-12 {
            return (None, "diverges".into());
        }
    }
    if !s
        .split(|c: char| !c.is_ascii_alphabetic())
        .any(|t| t == "n" || t == "npi")
    {
        if let Some(v) = number(s) {
            return finite(v);
        }
    }
    (None, "cannot determine from this formula".into())
}
fn linear(p: &Params) -> Option<(N, N)> {
    if recurrence_order(p) != 1 {
        return None;
    }
    let s = canonical(p.s("recurrence")).replace("a_(n-1)", "u");
    let r = rational(&s, b'u')?;
    if r.b.len() != 1 || r.a.len() > 2 {
        return None;
    }
    let c = r.a[0].op(r.b[0], '/');
    let k = r.a.get(1).copied().unwrap_or(N::int(0)).op(r.b[0], '/');
    Some((k, c))
}
pub(super) fn analyse(p: &Params, d: &Data) -> (Option<N>, Option<N>, String) {
    if p.s("mode") == "Explicit" {
        let (a, state) = explicit(p.s("formula"));
        let sum = if a.is_some_and(|n| n.q == Some((0, 1)))
            && rational(&canonical(p.s("formula")), b'n')
                .is_some_and(|r| r.a.iter().all(|n| n.q == Some((0, 1))))
        {
            Some(N::int(0))
        } else {
            None
        };
        return (a, sum, state);
    }
    if let Some((r, c)) = linear(p) {
        let one = N::int(1);
        if r.q == Some((1, 1)) {
            return if c.q == Some((0, 1)) {
                (
                    Some(d.first),
                    if d.first.q == Some((0, 1)) {
                        Some(N::int(0))
                    } else {
                        None
                    },
                    "constant".into(),
                )
            } else {
                (None, None, "diverges".into())
            };
        }
        let fixed = c.op(one.op(r, '-'), '/');
        if d.first.q.is_some() && d.first.q == fixed.q {
            return (
                Some(fixed),
                if fixed.q == Some((0, 1)) {
                    Some(N::int(0))
                } else {
                    None
                },
                "constant".into(),
            );
        }
        if r.q.is_some_and(|(a, b)| a.abs() < b) {
            return (
                Some(fixed),
                if c.q == Some((0, 1)) {
                    Some(d.first.op(one.op(r, '-'), '/'))
                } else {
                    None
                },
                "converges".into(),
            );
        }
        return (None, None, "diverges".into());
    }
    (None, None, "cannot determine from this formula".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn textbook_limit_proofs_r1() {
        for (formula, expect, state) in [
            ("n/(n+1)", Some((1, 1)), "converges"),
            ("1-1/n", Some((1, 1)), "converges"),
            ("(n^2+1)/(2n^2)", Some((1, 2)), "converges"),
            ("n^2", None, "diverges"),
            ("(-1)^n", None, "diverges"),
            ("(2/3)^n", Some((0, 1)), "converges"),
            ("n^(1/n)", Some((1, 1)), "converges"),
            ("sqrt(n+1)-sqrt(n)", Some((0, 1)), "converges"),
            ("n*sin(1/n)", Some((1, 1)), "converges"),
            ("7/3", Some((7, 3)), "converges"),
        ] {
            let mut p = Params::defaults(&params());
            p.set_s("mode", "Explicit");
            p.set_s("formula", formula);
            let (a, _, s) = limits(&p, &data(&p).unwrap());
            assert_eq!(a.and_then(|n| n.q), expect, "{formula}");
            assert!(s.contains(state), "{formula}: {s}");
            if let Some(target) = a {
                let error = |n| {
                    (read(formula, N::int(n), N::int(0), N::int(0))
                        .unwrap_or_else(|| panic!("numeric check {formula} n={n}"))
                        .v
                        - target.v)
                        .abs()
                };
                let (near, far, tolerance) = if formula == "(2/3)^n" {
                    (40, 160, 2e-5)
                } else if formula.contains("sqrt") {
                    (1000, 1_000_000, 1e-3)
                } else {
                    (1000, 1_000_000, 2e-5)
                };
                assert!(error(far) < error(near) + 1e-12, "{formula}");
                assert!(error(far) < tolerance, "{formula}");
            }
        }
        for (formula, target) in [
            ("(1+1/n)^n", std::f64::consts::E),
            ("n*sin(pi/n)", std::f64::consts::PI),
        ] {
            let mut p = Params::defaults(&params());
            p.set_s("mode", "Explicit");
            p.set_s("formula", formula);
            let lim = limits(&p, &data(&p).unwrap()).0.unwrap();
            assert!((lim.v - target).abs() < 1e-15);
            assert!(
                (read(formula, N::int(1_000_000), N::int(0), N::int(0))
                    .unwrap()
                    .v
                    - target)
                    .abs()
                    < 2e-5
            );
        }
        for (r, c, seed, expected, state) in [
            ("1/2", "3", "2", Some((6, 1)), "converges"),
            ("-1/2", "3", "2", Some((2, 1)), "constant"),
            ("2", "1", "1", None, "diverges"),
            ("-1", "0", "1", None, "diverges"),
            ("1", "0", "3", Some((3, 1)), "constant"),
        ] {
            let mut p = Params::defaults(&params());
            p.set_s("mode", "Recursive");
            p.set_s("first", seed);
            p.set_s("recurrence", &format!("({r})*a_(n-1)+({c})"));
            let (a, _, s) = limits(&p, &data(&p).unwrap());
            assert_eq!(a.and_then(|a| a.q), expected);
            assert!(s.contains(state));
            if let Some(target) = a {
                let mut v = number(seed).unwrap().v;
                for _ in 0..160 {
                    v = number(r).unwrap().v * v + number(c).unwrap().v;
                }
                assert!((v - target.v).abs() < 1e-12);
            }
        }
    }
}
