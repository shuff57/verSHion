use super::*;
#[derive(Clone)]
enum K {
    Atom,
    Unary(char, Box<E>),
    Binary(char, Box<E>, Box<E>),
    Function(String, Box<E>),
    Factorial(Box<E>),
}
#[derive(Clone)]
struct E {
    lo: usize,
    hi: usize,
    k: K,
}
struct Parser<'a> {
    s: &'a str,
    i: usize,
}
impl Parser<'_> {
    fn eat(&mut self, c: u8) -> bool {
        if self.s.as_bytes().get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Option<E> {
        let mut a = self.term()?;
        loop {
            let op = if self.eat(b'+') {
                '+'
            } else if self.eat(b'-') {
                '-'
            } else {
                break;
            };
            let b = self.term()?;
            a = E {
                lo: a.lo,
                hi: b.hi,
                k: K::Binary(op, Box::new(a), Box::new(b)),
            };
        }
        Some(a)
    }
    fn term(&mut self) -> Option<E> {
        let mut a = self.unary()?;
        loop {
            let op = if self.eat(b'*') {
                '*'
            } else if self.eat(b'/') {
                '/'
            } else if self
                .s
                .as_bytes()
                .get(self.i)
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'(')
            {
                '*'
            } else {
                break;
            };
            let b = self.unary()?;
            a = E {
                lo: a.lo,
                hi: b.hi,
                k: K::Binary(op, Box::new(a), Box::new(b)),
            };
        }
        Some(a)
    }
    fn unary(&mut self) -> Option<E> {
        let lo = self.i;
        if self.eat(b'-') || self.eat(b'+') {
            let c = self.s.as_bytes()[lo] as char;
            let a = self.unary()?;
            return Some(E {
                lo,
                hi: a.hi,
                k: K::Unary(c, Box::new(a)),
            });
        }
        let mut a = self.atom()?;
        while self.eat(b'!') {
            a = E {
                lo,
                hi: self.i,
                k: K::Factorial(Box::new(a)),
            };
        }
        if self.eat(b'^') {
            let b = self.unary()?;
            a = E {
                lo,
                hi: b.hi,
                k: K::Binary('^', Box::new(a), Box::new(b)),
            };
        }
        Some(a)
    }
    fn atom(&mut self) -> Option<E> {
        let lo = self.i;
        if self.eat(b'(') {
            let a = self.expr()?;
            if !self.eat(b')') {
                return None;
            }
            return Some(E {
                lo,
                hi: self.i,
                k: K::Unary('+', Box::new(a)),
            });
        }
        let bytes = self.s.as_bytes();
        if bytes.get(self.i).is_some_and(u8::is_ascii_alphabetic) {
            while bytes.get(self.i).is_some_and(u8::is_ascii_alphabetic) {
                self.i += 1;
            }
            let name = &self.s[lo..self.i];
            if self.eat(b'(') {
                let a = self.expr()?;
                if !self.eat(b')') {
                    return None;
                }
                return Some(E {
                    lo,
                    hi: self.i,
                    k: K::Function(name.into(), Box::new(a)),
                });
            }
        } else {
            while bytes
                .get(self.i)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
            {
                self.i += 1;
            }
            if bytes.get(self.i).is_some_and(|c| matches!(c, b'e' | b'E')) {
                self.i += 1;
                if bytes.get(self.i).is_some_and(|c| matches!(c, b'+' | b'-')) {
                    self.i += 1;
                }
                while bytes.get(self.i).is_some_and(u8::is_ascii_digit) {
                    self.i += 1;
                }
            }
        }
        if self.i == lo {
            None
        } else {
            Some(E {
                lo,
                hi: self.i,
                k: K::Atom,
            })
        }
    }
}
#[derive(Clone)]
enum A {
    Finite(N),
    Grow(i32, f64, i8),
    BoundedOsc,
    BoundedUnknown,
    UnboundedOsc,
    Unknown,
}
fn calc(e: &E, s: &str) -> A {
    let text = &s[e.lo..e.hi];
    if let Some(r) = rational(peel(text), b'n') {
        let da = r.a.len() as i32 - r.b.len() as i32;
        let a = *r.a.last().unwrap();
        let b = *r.b.last().unwrap();
        if da < 0 || r.a.iter().all(|n| n.q == Some((0, 1))) {
            return A::Finite(N::int(0));
        }
        if da == 0 {
            return A::Finite(a.op(b, '/'));
        }
        return A::Grow(0, da as f64, if a.v * b.v < 0.0 { -1 } else { 1 });
    }
    let (v, state) = atomic_limit(text);
    if let Some(v) = v {
        return A::Finite(v);
    }
    if state.contains("oscillat") {
        return A::BoundedOsc;
    }
    match &e.k {
        K::Atom => A::Unknown,
        K::Unary(c, a) => {
            let x = calc(a, s);
            if *c == '-' {
                match x {
                    A::Finite(n) => A::Finite(N::int(0).op(n, '-')),
                    A::Grow(r, p, sign) => A::Grow(r, p, -sign),
                    _ => x,
                }
            } else {
                x
            }
        }
        K::Factorial(a) => {
            if matches!(calc(a,s),A::Grow(0,p,1)if p==1.0) {
                A::Grow(2, 1.0, 1)
            } else {
                A::Unknown
            }
        }
        K::Function(f, a) => match calc(a, s) {
            A::Finite(n) => read(&format!("{f}({})", n.v), N::int(0), N::int(0), N::int(0))
                .map_or(A::Unknown, A::Finite),
            A::Grow(r, p, 1) => match f.as_str() {
                "sqrt" => A::Grow(r, p / 2.0, 1),
                "ln" | "log" => A::Grow(-1, 1.0, 1),
                "sin" | "cos" => A::BoundedUnknown,
                _ => A::Unknown,
            },
            _ => A::Unknown,
        },
        K::Binary('^', a, b) => {
            if let (Some(base), Some(r)) = (
                number(peel(&s[a.lo..a.hi])),
                rational(peel(&s[b.lo..b.hi]), b'n'),
            ) {
                if r.b.len() == 1 && r.a.len() == 2 {
                    let slope = r.a[1].op(r.b[0], '/').v;
                    if slope != 0.0 && base.v != 0.0 {
                        let log = base.v.abs().ln() * slope;
                        if log < 0.0 {
                            return A::Finite(N::int(0));
                        }
                        if base.q == Some((1, 1)) {
                            return A::Finite(N::int(1));
                        }
                        if base.v < 0.0 {
                            if base.q == Some((-1, 1)) {
                                let slope = r.a[1].op(r.b[0], '/');
                                let offset = r.a[0].op(r.b[0], '/');
                                if let (Some((a, 1)), Some((b, 1))) = (slope.q, offset.q) {
                                    if a % 2 == 0 {
                                        return A::Finite(N::int(if b % 2 == 0 { 1 } else { -1 }));
                                    }
                                }
                            }
                            return if base.q == Some((-1, 1)) {
                                A::BoundedOsc
                            } else {
                                A::UnboundedOsc
                            };
                        }
                        if log > 0.0 {
                            return A::Grow(1, log, 1);
                        }
                    }
                }
            }
            match (calc(a, s), calc(b, s)) {
                (A::Finite(a), A::Finite(b)) if a.v != 0.0 || b.v > 0.0 => {
                    let n = a.op(b, '^');
                    if n.v.is_finite() {
                        A::Finite(n)
                    } else {
                        A::Unknown
                    }
                }
                (A::Grow(0, p, 1), A::Finite(n)) => {
                    if n.v < 0.0 {
                        A::Finite(N::int(0))
                    } else if n.v > 0.0 {
                        A::Grow(0, p * n.v, 1)
                    } else {
                        A::Unknown
                    }
                }
                _ => A::Unknown,
            }
        }
        K::Binary(op, a, b) => {
            if *op == '*'
                && [a, b]
                    .iter()
                    .any(|e| number(peel(&s[e.lo..e.hi])).is_some_and(|n| n.q == Some((0, 1))))
            {
                return A::Finite(N::int(0));
            }
            let x = calc(a, s);
            let mut y = calc(b, s);
            let op = if *op == '-' {
                y = match y {
                    A::Finite(n) => A::Finite(N::int(0).op(n, '-')),
                    A::Grow(r, p, sg) => A::Grow(r, p, -sg),
                    a => a,
                };
                '+'
            } else {
                *op
            };
            match (op, x, y) {
                ('+', A::Finite(a), A::Finite(b)) => A::Finite(a.op(b, '+')),
                ('*', A::Finite(a), A::Finite(b)) => A::Finite(a.op(b, '*')),
                ('/', A::Finite(a), A::Finite(b)) if b.v != 0.0 => A::Finite(a.op(b, '/')),
                ('+', A::Finite(_), g @ A::Grow(..)) | ('+', g @ A::Grow(..), A::Finite(_)) => g,
                ('+', A::Finite(_), A::BoundedOsc) | ('+', A::BoundedOsc, A::Finite(_)) => {
                    A::BoundedOsc
                }
                ('+', A::Finite(_), A::UnboundedOsc) | ('+', A::UnboundedOsc, A::Finite(_)) => {
                    A::UnboundedOsc
                }
                ('*', A::Finite(n), A::Grow(r, p, sg)) | ('*', A::Grow(r, p, sg), A::Finite(n))
                    if n.v != 0.0 =>
                {
                    A::Grow(r, p, if n.v < 0.0 { -sg } else { sg })
                }
                ('*', A::Finite(n), A::BoundedOsc) | ('*', A::BoundedOsc, A::Finite(n))
                    if n.v != 0.0 =>
                {
                    A::BoundedOsc
                }
                ('*', A::Finite(n), A::UnboundedOsc) | ('*', A::UnboundedOsc, A::Finite(n))
                    if n.v != 0.0 =>
                {
                    A::UnboundedOsc
                }
                ('*', A::Grow(..), A::BoundedOsc) | ('*', A::BoundedOsc, A::Grow(..)) => {
                    A::UnboundedOsc
                }
                ('/', A::Finite(_), A::Grow(..))
                | ('/', A::BoundedOsc, A::Grow(..))
                | ('/', A::BoundedUnknown, A::Grow(..)) => A::Finite(N::int(0)),
                ('/', A::Grow(r, p, sg), A::Finite(n)) if n.v != 0.0 => {
                    A::Grow(r, p, if n.v < 0.0 { -sg } else { sg })
                }
                ('/', A::Grow(r, p, _), A::Grow(t, q, _)) if r < t || (r == t && p < q) => {
                    A::Finite(N::int(0))
                }
                ('/', A::Grow(r, p, sg), A::Grow(t, q, sg2)) if r > t || (r == t && p > q) => {
                    A::Grow(r, if r == t { p - q } else { p }, sg * sg2)
                }
                _ => A::Unknown,
            }
        }
    }
}
pub(super) fn analyse(input: &str) -> (Option<N>, String) {
    let s = canonical(&sqrt_spelling(input));
    let mut p = Parser { s: &s, i: 0 };
    let Some(e) = p.expr().filter(|_| p.i == s.len()) else {
        return (None, "cannot determine from this formula".into());
    };
    match calc(&e, &s) {
        A::Finite(n) if n.v.is_finite() => {
            // Verification uses the same reader as the plotted terms. Underflow refusals are skipped,
            // never converted to zero. At least one large-index value must confirm the proof.
            let mut confirmed = false;
            for index in [
                128,
                512,
                2048,
                8192,
                1_000_000,
                1_000_000_000,
                1_000_000_000_000,
            ] {
                if let Some(v) = read(input, N::int(index), N::int(0), N::int(0)) {
                    confirmed = (v.v - n.v).abs() <= 1e-3 * n.v.abs().max(1.0);
                }
            }
            if confirmed {
                finite(n)
            } else {
                (
                    None,
                    "cannot determine: large-index verification is inconclusive".into(),
                )
            }
        }
        A::Grow(..) | A::BoundedOsc | A::UnboundedOsc => (None, "diverges".into()),
        _ => (None, "cannot determine from this formula".into()),
    }
}
// Derive a constant ratio from the syntax, never from adjacent sampled values.
pub(in crate::figures::sequences) fn geometric(
    input: &str,
    start: i128,
) -> Option<(num_rational::BigRational, num_rational::BigRational)> {
    fn form(e: &E, s: &str) -> Option<(N, N)> {
        match &e.k {
            K::Unary('+', a) => form(a, s),
            K::Binary('^', base, power) => {
                let base = number(peel(&s[base.lo..base.hi]))?;
                let r = rational(peel(&s[power.lo..power.hi]), b'n')?;
                if r.b.len() != 1 || r.a.len() != 2 {
                    return None;
                }
                let slope = r.a[1].op(r.b[0], '/');
                let offset = r.a[0].op(r.b[0], '/');
                if slope.q?.1 != 1 || offset.q?.1 != 1 {
                    return None;
                }
                Some((base.op(offset, '^'), base.op(slope, '^')))
            }
            K::Binary('*', a, b) => {
                if let Some(c) = number(peel(&s[a.lo..a.hi])) {
                    let (x, r) = form(b, s)?;
                    Some((c.op(x, '*'), r))
                } else {
                    let c = number(peel(&s[b.lo..b.hi]))?;
                    let (x, r) = form(a, s)?;
                    Some((c.op(x, '*'), r))
                }
            }
            _ => None,
        }
    }
    let s = canonical(input);
    let mut p = Parser { s: &s, i: 0 };
    let e = p.expr()?;
    if p.i != s.len() {
        return None;
    }
    let (a, r) = form(&e, &s)?;
    let a = a.op(r.op(N::int(start), '^'), '*');
    let cv = |n: N| {
        let (a, b) = n.q?;
        Some(num_rational::BigRational::new(a.into(), b.into()))
    };
    Some((cv(a)?, cv(r)?))
}
