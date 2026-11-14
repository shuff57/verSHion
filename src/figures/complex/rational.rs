//! Exact certification of rational scalar expressions, supplementing the shared reader.
//! Checked i128 arithmetic deliberately refuses expressions outside this small budget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Rational(pub i128, pub i128);
impl Rational {
    fn new(n: i128, d: i128) -> Option<Self> {
        if d == 0 || n == i128::MIN || d == i128::MIN {
            return None;
        }
        let (mut a, mut b) = (n.abs(), d.abs());
        while b != 0 {
            (a, b) = (b, a % b);
        }
        let g = a.max(1);
        Some(Self(n / g * d.signum(), d.abs() / g))
    }
    fn add(self, r: Self) -> Option<Self> {
        Self::new(
            self.0
                .checked_mul(r.1)?
                .checked_add(r.0.checked_mul(self.1)?)?,
            self.1.checked_mul(r.1)?,
        )
    }
    fn mul(self, r: Self) -> Option<Self> {
        let a = Self::new(self.0, r.1)?;
        let b = Self::new(r.0, self.1)?;
        Self::new(a.0.checked_mul(b.0)?, a.1.checked_mul(b.1)?)
    }
    fn neg(self) -> Option<Self> {
        Self::new(self.0.checked_neg()?, self.1)
    }
    fn div(self, r: Self) -> Option<Self> {
        self.mul(Self::new(r.1, r.0)?)
    }
    pub(super) fn value(self) -> f64 {
        self.0 as f64 / self.1 as f64
    }
    pub(super) fn text(self) -> String {
        if self.1 == 1 {
            self.0.to_string()
        } else {
            format!("{}/{}", self.0, self.1)
        }
        .replace('-', "−")
    }
}
pub(super) fn parse(s: &str) -> Option<Rational> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let mut p = Parser {
        s: s.as_bytes(),
        i: 0,
    };
    let r = p.sum()?;
    (p.i == p.s.len()).then_some(r)
}
struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}
impl Parser<'_> {
    fn eat(&mut self, c: u8) -> bool {
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn sum(&mut self) -> Option<Rational> {
        let mut a = self.product()?;
        loop {
            if self.eat(b'+') {
                a = a.add(self.product()?)?;
            } else if self.eat(b'-') {
                a = a.add(self.product()?.neg()?)?;
            } else {
                return Some(a);
            }
        }
    }
    fn product(&mut self) -> Option<Rational> {
        let mut a = self.unary()?;
        loop {
            if self.eat(b'*') {
                a = a.mul(self.unary()?)?;
            } else if self.eat(b'/') {
                a = a.div(self.unary()?)?;
            } else {
                return Some(a);
            }
        }
    }
    fn unary(&mut self) -> Option<Rational> {
        if self.eat(b'-') {
            self.unary()?.neg()
        } else if self.eat(b'+') {
            self.unary()
        } else {
            self.power()
        }
    }
    fn power(&mut self) -> Option<Rational> {
        let mut a = self.atom()?;
        if self.eat(b'^') {
            let n = self.unary()?;
            if n.1 != 1 || n.0.unsigned_abs() > 32 || (a.0 == 0 && n.0 <= 0) {
                return None;
            }
            if n.0 < 0 {
                a = Rational::new(a.1, a.0)?;
            }
            let mut r = Rational(1, 1);
            for _ in 0..n.0.unsigned_abs() {
                r = r.mul(a)?;
            }
            Some(r)
        } else {
            Some(a)
        }
    }
    fn atom(&mut self) -> Option<Rational> {
        if self.eat(b'(') {
            let r = self.sum()?;
            return self.eat(b')').then_some(r);
        }
        let start = self.i;
        let mut n = 0i128;
        let mut decimals = None;
        let mut digits = 0;
        while let Some(&c) = self.s.get(self.i) {
            if c.is_ascii_digit() {
                n = n.checked_mul(10)?.checked_add((c - b'0') as i128)?;
                digits += 1;
                if let Some(d) = &mut decimals {
                    *d += 1;
                }
                self.i += 1;
            } else if c == b'.' && decimals.is_none() {
                decimals = Some(0u32);
                self.i += 1;
            } else {
                break;
            }
        }
        if self.i == start || digits == 0 {
            return None;
        }
        let mut exponent = 0i32;
        if self.eat(b'e') || self.eat(b'E') {
            let sign = if self.eat(b'-') {
                -1
            } else {
                self.eat(b'+');
                1
            };
            let start = self.i;
            while let Some(&c) = self.s.get(self.i).filter(|c| c.is_ascii_digit()) {
                exponent = exponent.checked_mul(10)?.checked_add((c - b'0') as i32)?;
                self.i += 1;
            }
            if start == self.i {
                return None;
            }
            exponent *= sign;
        }
        let scale = exponent.checked_sub(decimals.unwrap_or(0) as i32)?;
        if scale.unsigned_abs() > 37 {
            return None;
        }
        let p = 10i128.checked_pow(scale.unsigned_abs())?;
        if scale >= 0 {
            Rational::new(n.checked_mul(p)?, 1)
        } else {
            Rational::new(n, p)
        }
    }
}
