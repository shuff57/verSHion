//! Discrete sequences and finite series. Exact values are propagated, never inferred from floats.
use super::{
    svg::{self, Doc},
    *,
};
#[path = "sequences_limits.rs"]
mod limit_math;
#[path = "sequences_series.rs"]
mod series_math;
pub const ID: &str = "sequences-series";
pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Sequences and series",
        blurb: "Discrete terms and partial sums, with formula and continuation tasks.",
        params,
        render,
    }]
}
fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice(
            "mode",
            "Sequence",
            &["Arithmetic", "Geometric", "Explicit", "Recursive"],
            "Arithmetic",
        ),
        ParamDef::text("first", "First term", "2"),
        ParamDef::text("difference", "Common difference", "1"),
        ParamDef::text("ratio", "Common ratio", "1/2"),
        ParamDef::text("formula", "Explicit f(n)", "1/n"),
        ParamDef::text("recurrence", "Recursive expression", "a_(n-1)+a_(n-2)"),
        ParamDef::text("second", "Second initial term", "1"),
        ParamDef::int("count", "Number of terms", 3, 24, 8),
        ParamDef::choice("start", "First index", &["0", "1"], "1"),
        ParamDef::choice(
            "display",
            "Draw",
            &["Terms", "Partial sums", "Both"],
            "Terms",
        ),
        ParamDef::choice(
            "layout",
            "Graphs",
            &["One grid", "Two aligned graphs", "Number line"],
            "One grid",
        ),
        ParamDef::bool("join", "Join points", false),
        ParamDef::choice("marks", "Term marks", &["Dots", "Spikes"], "Dots"),
        ParamDef::bool("point_values", "Label point values", false),
        ParamDef::bool("terms", "List terms", true),
        ParamDef::bool("sums", "List partial sums", false),
        ParamDef::bool("explicit", "Write explicit formula", true),
        ParamDef::bool("recursive", "Write recursive formula", false),
        ParamDef::bool("sum", "Write series sum", false),
        ParamDef::bool("sum_n", "Sum first N terms", false),
        ParamDef::int("sum_count", "N for requested sum", 1, 1000, 50),
        ParamDef::bool("sigma", "Write sigma notation", false),
        ParamDef::bool("convergence", "Write convergence", false),
        ParamDef::bool("limit", "Draw limit line", false),
        ParamDef::bool("band", "Epsilon band", false),
        ParamDef::text("epsilon", "Epsilon", "1/4"),
        ParamDef::float("from", "Vertical axis: From", -100.0, 99.0, 0.000001, -2.0),
        ParamDef::float("to", "Vertical axis: To", -99.0, 100.0, 0.000001, 12.0),
        ParamDef::float(
            "step",
            "Vertical axis: Count by",
            0.000001,
            20.0,
            0.000001,
            2.0,
        ),
        ParamDef::choice(
            "grid",
            "Gridlines",
            &["None", "Light", "Standard", "Bold"],
            "Light",
        ),
        ParamDef::text("xlabel", "Index axis label", "n"),
        ParamDef::text("ylabel", "Value axis label", "aₙ"),
        ParamDef::text("title", "Title", "Sequences and series"),
        ParamDef::bool("blank_title", "Blank title", false),
        ParamDef::choice(
            "task",
            "Student task",
            &["Find formula", "Missing terms", "Continue"],
            "Find formula",
        ),
        ParamDef::text("missing", "Indices to blank", "5,7"),
        ParamDef::int("given", "Terms given", 1, 6, 3),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}
// Reduced checked rationals. Overflow drops exactness, never guesses a fraction.
#[derive(Clone, Copy, Debug)]
struct N {
    v: f64,
    q: Option<(i128, i128)>,
    dec: bool,
}
fn rat(a: i128, b: i128) -> Option<(i128, i128)> {
    if b == 0 {
        return None;
    }
    let (mut x, mut y) = (a.checked_abs()?, b.checked_abs()?);
    while y != 0 {
        (x, y) = (y, x % y)
    }
    let g = x.max(1);
    Some((
        a.checked_div(g)?.checked_mul(b.signum())?,
        b.checked_abs()? / g,
    ))
}
fn gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        (a, b) = (b, a % b)
    }
    a.max(1)
}
impl N {
    fn int(n: i128) -> Self {
        Self {
            v: n as f64,
            q: Some((n, 1)),
            dec: false,
        }
    }
    fn op(self, b: Self, c: char) -> Self {
        let q = if (c == '*' && (self.q == Some((0, 1)) || b.q == Some((0, 1))))
            || (c == '/' && self.q == Some((0, 1)) && b.v != 0.0)
        {
            Some((0, 1))
        } else {
            (|| {
                let (a, d) = self.q?;
                let (e, f) = b.q?;
                match c {
                    '+' | '-' => {
                        let g = gcd(d, f);
                        let x = a.checked_mul(f / g)?;
                        let y = e.checked_mul(d / g)?;
                        rat(
                            if c == '+' {
                                x.checked_add(y)?
                            } else {
                                x.checked_sub(y)?
                            },
                            (d / g).checked_mul(f)?,
                        )
                    }
                    '*' => {
                        let g = gcd(a.checked_abs()?, f);
                        let h = gcd(e.checked_abs()?, d);
                        rat((a / g).checked_mul(e / h)?, (d / h).checked_mul(f / g)?)
                    }
                    '/' => {
                        let g = gcd(a.checked_abs()?, e.checked_abs()?);
                        let h = gcd(d, f);
                        rat((a / g).checked_mul(f / h)?, (d / h).checked_mul(e / g)?)
                    }
                    '^' => {
                        if f != 1 || e.checked_abs().is_none_or(|v| v > 128) {
                            return None;
                        }
                        let k = e.unsigned_abs() as u32;
                        if e < 0 {
                            rat(d.checked_pow(k)?, a.checked_pow(k)?)
                        } else {
                            rat(a.checked_pow(k)?, d.checked_pow(k)?)
                        }
                    }
                    _ => None,
                }
            })()
        };
        let mut value = q
            .map(|(a, b)| a as f64 / b as f64)
            .unwrap_or_else(|| match c {
                '+' => self.v + b.v,
                '-' => self.v - b.v,
                '*' => self.v * b.v,
                '/' => self.v / b.v,
                _ => self.v.powf(b.v),
            });
        if q.is_none()
            && value == 0.0
            && (matches!(c, '*' | '/' | '^') && self.v != 0.0 && b.v != 0.0)
        {
            value = f64::NAN;
        }
        Self {
            v: value,
            q,
            dec: self.dec || b.dec,
        }
    }
    fn text(self) -> String {
        if let Some((a, b)) = self.q {
            if self.dec {
                if let Some(s) = exact_decimal(a, b) {
                    return s;
                }
            }
            if !self.dec {
                return if b == 1 {
                    a.to_string()
                } else {
                    format!("{a}/{b}")
                };
            }
        }
        format!("≈ {}", approximation(self.v))
    }
    fn eq(self) -> String {
        if !self.text().starts_with('≈') {
            format!("= {}", self.text())
        } else {
            self.text()
        }
    }
}
// Exact parser for arithmetic and recurrence expressions; transcendental functions use the existing reader.
struct P<'a> {
    s: &'a [u8],
    i: usize,
    n: N,
    a: N,
    b: N,
}
impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Option<N> {
        let mut a = self.term()?;
        loop {
            if self.eat(b'+') {
                a = a.op(self.term()?, '+')
            } else if self.eat(b'-') {
                a = a.op(self.term()?, '-')
            } else {
                return Some(a);
            }
        }
    }
    fn term(&mut self) -> Option<N> {
        let mut a = self.unary()?;
        loop {
            if self.eat(b'*') {
                a = a.op(self.unary()?, '*')
            } else if self.eat(b'/') {
                a = a.op(self.unary()?, '/')
            } else {
                self.ws();
                if self
                    .s
                    .get(self.i)
                    .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'(')
                {
                    a = a.op(self.unary()?, '*')
                } else {
                    return Some(a);
                }
            }
        }
    }
    fn unary(&mut self) -> Option<N> {
        if self.eat(b'-') {
            return Some(N::int(0).op(self.unary()?, '-'));
        }
        if self.eat(b'+') {
            return self.unary();
        }
        let mut a = self.atom()?;
        while self.eat(b'!') {
            let (k, d) = a.q?;
            if d != 1 || !(0..=170).contains(&k) {
                return None;
            }
            a = N::int(1);
            for j in 2..=k {
                a = a.op(N::int(j), '*');
            }
        }
        if self.eat(b'^') {
            Some(a.op(self.unary()?, '^'))
        } else {
            Some(a)
        }
    }
    fn atom(&mut self) -> Option<N> {
        self.ws();
        if self.eat(b'(') {
            let a = self.expr()?;
            return if self.eat(b')') { Some(a) } else { None };
        }
        let begin = self.i;
        if self.s.get(self.i).is_some_and(u8::is_ascii_alphabetic) {
            while self.s.get(self.i).is_some_and(u8::is_ascii_alphabetic) {
                self.i += 1;
            }
            let name = std::str::from_utf8(&self.s[begin..self.i]).ok()?;
            match name {
                "n" => return Some(self.n),
                "u" => return Some(self.a),
                "v" => return Some(self.b),
                "pi" => {
                    return Some(N {
                        v: std::f64::consts::PI,
                        q: None,
                        dec: true,
                    });
                }
                "e" => {
                    return Some(N {
                        v: std::f64::consts::E,
                        q: None,
                        dec: true,
                    });
                }
                _ => (),
            }
            if !self.eat(b'(') {
                return None;
            }
            let arg = self.expr()?;
            if !self.eat(b')') {
                return None;
            }
            let full = std::str::from_utf8(&self.s[begin..self.i]).ok()?;
            if let Some(v) = proved_function(full, self.n, self.a, self.b) {
                return v;
            }
            let v = match name {
                "sin" => arg.v.sin(),
                "cos" => arg.v.cos(),
                "tan" => arg.v.tan(),
                "sqrt" => arg.v.sqrt(),
                "ln" => arg.v.ln(),
                "log" => arg.v.log10(),
                "exp" => arg.v.exp(),
                "abs" => arg.v.abs(),
                "floor" => arg.v.floor(),
                "ceil" => arg.v.ceil(),
                _ => return None,
            };
            return if v.is_finite() {
                Some(N {
                    v,
                    q: None,
                    dec: true,
                })
            } else {
                None
            };
        }
        for (name, v) in [("n", self.n), ("u", self.a), ("v", self.b)] {
            if self.s.get(self.i..self.i + name.len()) == Some(name.as_bytes()) {
                self.i += name.len();
                return Some(v);
            }
        }
        let start = self.i;
        while self
            .s
            .get(self.i)
            .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
        {
            self.i += 1
        }
        if self.s.get(self.i).is_some_and(|c| *c == b'e' || *c == b'E') {
            self.i += 1;
            if self.s.get(self.i).is_some_and(|c| *c == b'+' || *c == b'-') {
                self.i += 1
            }
            while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                self.i += 1
            }
        }
        let t = std::str::from_utf8(&self.s[start..self.i]).ok()?;
        let v = t.parse::<f64>().ok()?;
        let dec = t.contains('.') || t.contains('e') || t.contains('E');
        let q = if v == 0.0
            && !t
                .chars()
                .take_while(|c| *c != 'e' && *c != 'E')
                .any(|c| matches!(c, '1'..='9'))
        {
            Some((0, 1))
        } else if dec {
            decimal_rational(t)
        } else {
            t.parse::<i128>().ok().map(|a| (a, 1))
        };
        if v == 0.0
            && t.chars()
                .take_while(|c| *c != 'e' && *c != 'E')
                .any(|c| matches!(c, '1'..='9'))
        {
            return None;
        }
        Some(N { v, q, dec })
    }
}
fn subscript(n: usize) -> String {
    n.to_string()
        .chars()
        .map(|c| match c {
            '0' => '₀',
            '1' => '₁',
            '2' => '₂',
            '3' => '₃',
            '4' => '₄',
            '5' => '₅',
            '6' => '₆',
            '7' => '₇',
            '8' => '₈',
            _ => '₉',
        })
        .collect()
}
fn exact_decimal(a: i128, b: i128) -> Option<String> {
    for digits in 0..=37u32 {
        let scale = 10i128.checked_pow(digits)?;
        if scale % b == 0 {
            let scaled = a.checked_mul(scale / b)?;
            let abs = scaled.checked_abs()?.to_string();
            let sign = if scaled < 0 { "-" } else { "" };
            if digits == 0 {
                return Some(format!("{sign}{abs}"));
            }
            let padded = format!("{:0>width$}", abs, width = digits as usize + 1);
            let k = padded.len() - digits as usize;
            let frac = padded[k..].trim_end_matches('0');
            return Some(if frac.is_empty() {
                format!("{sign}{}", &padded[..k])
            } else {
                format!("{sign}{}.{}", &padded[..k], frac)
            });
        }
    }
    None
}
fn decimal_rational(t: &str) -> Option<(i128, i128)> {
    let parts: Vec<&str> = t.split(['e', 'E']).collect();
    let exp = if parts.len() == 2 {
        parts[1].parse::<i32>().ok()?
    } else {
        0
    };
    let mant = parts[0];
    let decimals = mant.split('.').nth(1).map_or(0, |s| s.len() as i32);
    let num = mant.replace('.', "").parse::<i128>().ok()?;
    let power = exp - decimals;
    if power.unsigned_abs() > 37 {
        return None;
    }
    if power >= 0 {
        rat(num.checked_mul(10i128.checked_pow(power as u32)?)?, 1)
    } else {
        rat(num, 10i128.checked_pow((-power) as u32)?)
    }
}
fn approximation(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let exp = v.abs().log10().floor() as i32;
    let digits = (7 - exp).clamp(0, 15) as usize;
    let s = if exp < -7 || exp > 8 {
        format!("{v:.7e}")
    } else {
        format!("{v:.digits$}")
    };
    if s.contains('e') || !s.contains('.') {
        s
    } else {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
fn pi_coefficient(s: &str, n: N, a: N, b: N) -> Option<N> {
    let t = s.replace("npi", "n*pi").replace("pin", "pi*n");
    if t.matches("pi").count() != 1 {
        return None;
    }
    let cs = t.as_bytes();
    let i = t.find("pi")?;
    if t[..i].contains('/') || t[i + 2..].contains('^') {
        return None;
    }
    // Only products, quotients and parentheses: addition would not be a pi multiple.
    if cs.iter().enumerate().any(|(j, c)| {
        matches!(c, b'+' | b'-')
            && j > 0
            && cs[j - 1] != b'('
            && cs[j - 1] != b'*'
            && cs[j - 1] != b'/'
    }) {
        return None;
    }
    let t = t.replace("pi", "(1)");
    let mut p = P {
        s: t.as_bytes(),
        i: 0,
        n,
        a,
        b,
    };
    let v = p.expr()?;
    p.ws();
    if p.i == p.s.len() { Some(v) } else { None }
}
fn sqrt_spelling(s: &str) -> String {
    let cs = s.chars().collect::<Vec<_>>();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '√' {
            out.push_str("sqrt");
            i += 1;
            if cs.get(i) != Some(&'(') {
                out.push('(');
                while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '.') {
                    out.push(cs[i]);
                    i += 1;
                }
                out.push(')');
            }
        } else {
            out.push(cs[i]);
            i += 1;
        }
    }
    out
}
fn proved_function(s: &str, n: N, a: N, b: N) -> Option<Option<N>> {
    let s = sqrt_spelling(s)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .replace('π', "pi")
        .replace('−', "-")
        .replace('√', "sqrt");
    for name in [
        "sin", "cos", "tan", "sqrt", "exp", "ln", "log", "abs", "floor", "ceil",
    ] {
        let Some(inner) = s
            .strip_prefix(&format!("{name}("))
            .and_then(|v| v.strip_suffix(')'))
        else {
            continue;
        };
        let mut depth = 0i32;
        let mut balanced = true;
        for c in inner.chars() {
            if c == '(' {
                depth += 1
            } else if c == ')' {
                depth -= 1;
                if depth < 0 {
                    balanced = false;
                    break;
                }
            }
        }
        if !balanced || depth != 0 {
            continue;
        }
        if matches!(name, "sin" | "cos" | "tan") {
            if let Some((num, den)) = pi_coefficient(inner, n, a, b).and_then(|v| v.q) {
                let period = den.checked_mul(2)?;
                let r = num.rem_euclid(period);
                let (mut t, mut parity) = if r >= den { (r - den, -1.0) } else { (r, 1.0) };
                if t > den / 2 {
                    t -= den;
                    parity = -parity
                }
                if t == 0 {
                    return Some(Some(N::int(if name == "cos" { parity as i128 } else { 0 })));
                }
                let absolute = t.checked_abs()?;
                let complement = den - absolute.checked_mul(2)?;
                if complement == 0 {
                    return Some(match name {
                        "tan" => None,
                        "cos" => Some(N::int(0)),
                        _ => Some(N::int((parity * t.signum() as f64) as i128)),
                    });
                }
                let special = if name == "cos" && absolute.checked_mul(3) == Some(den) {
                    Some((parity as i128, 2))
                } else if name == "sin" && absolute.checked_mul(6) == Some(den) {
                    Some(((parity * t.signum() as f64) as i128, 2))
                } else {
                    None
                };
                if let Some((x, y)) = special {
                    return Some(Some(N {
                        v: x as f64 / y as f64,
                        q: rat(x, y),
                        dec: false,
                    }));
                }
                let angle;
                if absolute <= den / 4 {
                    angle = std::f64::consts::PI * (absolute as f64 / den as f64)
                } else {
                    angle = std::f64::consts::PI * (complement as f64 / period as f64)
                }
                let (sm, cm) = if absolute <= den / 4 {
                    (angle.sin(), angle.cos())
                } else {
                    (angle.cos(), angle.sin())
                };
                let sine = parity * t.signum() as f64 * sm;
                let cosine = parity * cm;
                let v = match name {
                    "sin" => sine,
                    "cos" => cosine,
                    _ => sine / cosine,
                };
                return Some(Some(N {
                    v,
                    q: None,
                    dec: true,
                }));
            }
        }
        if let Some(arg) = read(inner, n, a, b) {
            if let Some((p, q)) = arg.q {
                let exact = match name {
                    "sin" | "tan" if p == 0 => Some(N::int(0)),
                    "cos" | "exp" if p == 0 => Some(N::int(1)),
                    "ln" | "log" if p == q => Some(N::int(0)),
                    "abs" => p.checked_abs().and_then(|v| rat(v, q)).map(|r| N {
                        v: arg.v.abs(),
                        q: Some(r),
                        dec: arg.dec,
                    }),
                    "sqrt" if p >= 0 => {
                        let x = p.isqrt();
                        let y = q.isqrt();
                        if x * x == p && y * y == q {
                            Some(N {
                                v: x as f64 / y as f64,
                                q: rat(x, y),
                                dec: arg.dec,
                            })
                        } else {
                            None
                        }
                    }
                    "floor" => Some(N::int(p.div_euclid(q))),
                    "ceil" => Some(N::int(
                        p.div_euclid(q) + if p.rem_euclid(q) > 0 { 1 } else { 0 },
                    )),
                    _ => None,
                };
                if let Some(v) = exact {
                    return Some(Some(v));
                }
            }
        }
    }
    None
}
fn ambiguous_fraction(s: &str) -> bool {
    let s = s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let cs = s.chars().collect::<Vec<_>>();
    for i in 0..cs.len() {
        if cs[i] != '/' {
            continue;
        }
        let mut j = i + 1;
        while j < cs.len() && (cs[j].is_ascii_digit() || cs[j] == '.') {
            j += 1;
        }
        if j > i + 1
            && j < cs.len()
            && (cs[j] == 'n' || cs[j] == 'x' || cs[j] == 'π' || cs[j] == '(')
        {
            return true;
        }
    }
    false
}
fn recurrence_spelling(s: &str) -> String {
    s.replace('−', "-")
        .replace("a_{n-1}", "a_(n-1)")
        .replace("a_{n-2}", "a_(n-2)")
        .replace("a(n-1)", "a_(n-1)")
        .replace("a(n-2)", "a_(n-2)")
        .replace("aₙ₋₁", "a_(n-1)")
        .replace("aₙ₋₂", "a_(n-2)")
}
fn recurrence_order(p: &Params) -> usize {
    if recurrence_spelling(p.s("recurrence"))
        .replace(' ', "")
        .contains("a_(n-2)")
    {
        2
    } else {
        1
    }
}
fn read(s: &str, n: N, a: N, b: N) -> Option<N> {
    if s.len() > 256 || ambiguous_fraction(s) {
        return None;
    }
    if s.split(|c: char| !c.is_ascii_alphabetic())
        .any(|t| !t.is_empty() && t != "n" && t.chars().all(|c| "nuvx".contains(c)))
    {
        return None;
    }
    if let Some(value) = proved_function(s, n, a, b) {
        return value;
    }
    let s = recurrence_spelling(&sqrt_spelling(s))
        .replace('×', "*")
        .replace('÷', "/")
        .replace('·', "*")
        .replace('²', "^2")
        .replace('³', "^3")
        .replace('π', "pi")
        .replace("npi", "n*pi")
        .replace("pin", "pi*n")
        .replace('√', "sqrt")
        .replace("a_(n-1)", "(u)")
        .replace("a_(n-2)", "(v)")
        .replace("aₙ₋₁", "(u)")
        .replace("aₙ₋₂", "(v)");
    let mut p = P {
        s: s.as_bytes(),
        i: 0,
        n,
        a,
        b,
    };
    if let Some(v) = p.expr() {
        p.ws();
        if p.i == p.s.len() {
            return if v.v.is_finite() { Some(v) } else { None };
        }
    }
    if s.chars().all(|c| c.is_ascii()) {
        return None;
    }
    let mut exp = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_alphabetic() {
            let begin = i;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1
            }
            let token: String = chars[begin..i].iter().collect();
            exp.push_str(&match token.as_str() {
                "n" => "x".into(),
                "u" => format!("({})", a.v),
                "v" => format!("({})", b.v),
                _ => token,
            });
        } else {
            exp.push(chars[i]);
            i += 1
        }
    }
    super::math::eval(&exp, n.v)
        .or_else(|| super::math::eval_typed(&s))
        .filter(|v| {
            v.is_finite()
                && *v != 0.0
                && !(v.abs() < 1e-12
                    && (s.contains("sin")
                        || s.contains("cos")
                        || s.contains("tan")
                        || s.contains("pi")))
        })
        .map(|v| N {
            v,
            q: None,
            dec: true,
        })
}

fn number(s: &str) -> Option<N> {
    if s.len() > 256 {
        return None;
    }
    let normalized = s
        .replace('−', "-")
        .replace('×', "*")
        .replace('÷', "/")
        .replace('·', "*");
    let s = normalized.as_str();
    if s.chars()
        .all(|c| c.is_ascii_digit() || "eE.+-*/^() ".contains(c))
    {
        read(s, N::int(0), N::int(0), N::int(0))
    } else {
        // A coefficient is a number, never an expression in the index or previous terms.
        if s.split(|c: char| !c.is_ascii_alphabetic())
            .any(|t| matches!(t, "n" | "u" | "v" | "x" | "a"))
        {
            return None;
        }
        read(s, N::int(0), N::int(0), N::int(0))
    }
}
// Pair rational-angle trig terms by reflection identities, so complete periods sum exactly.
// Keys represent the formal sin/cos of a reduced angle; no floating value is recognised.
fn trig_sum(formula: &str, start: usize, count: usize) -> Option<N> {
    let source = formula
        .replace(' ', "")
        .replace('π', "pi")
        .replace("npi", "n*pi");
    let (cos, inner) = if let Some(v) = source
        .strip_prefix("sin(")
        .and_then(|v| v.strip_suffix(')'))
    {
        (false, v)
    } else if let Some(v) = source
        .strip_prefix("cos(")
        .and_then(|v| v.strip_suffix(')'))
    {
        (true, v)
    } else {
        return None;
    };
    let mut keys = std::collections::BTreeMap::<(i128, i128), i128>::new();
    let mut rational = N::int(0);
    for j in 0..count {
        let n = N::int((start + j) as i128);
        let angle = pi_coefficient(inner, n, N::int(0), N::int(0))?;
        let (num, den) = angle.q?;
        let value = read(formula, n, N::int(0), N::int(0))?;
        if value.q.is_some() {
            rational = rational.op(value, '+');
            continue;
        }
        let period = den.checked_mul(2)?;
        let mut r = num.rem_euclid(period);
        let mut sign = 1;
        if r > den {
            r = period - r;
            if !cos {
                sign = -sign;
            }
        }
        if r.checked_mul(2)? > den {
            r = den - r;
            if cos {
                sign = -sign;
            }
        }
        let key = rat(r, den)?;
        *keys.entry(key).or_default() += sign;
    }
    keys.retain(|_, weight| *weight != 0);
    if keys.is_empty() {
        return Some(rational);
    }
    let mut value = rational.v;
    for ((a, b), weight) in keys {
        let angle = std::f64::consts::PI * (a as f64 / b as f64);
        value += weight as f64 * if cos { angle.cos() } else { angle.sin() };
    }
    Some(N {
        v: value,
        q: None,
        dec: true,
    })
}
struct Data {
    a: Vec<N>,
    s: Vec<N>,
    first: N,
    k: N,
}
fn data(p: &Params) -> Result<Data, String> {
    let mode = p.s("mode");
    let expression = if mode == "Explicit" {
        p.s("formula")
    } else if mode == "Recursive" {
        p.s("recurrence")
    } else {
        ""
    };
    if ambiguous_fraction(expression) {
        return Err(
            "Use brackets to make the fraction clear: 1/(2*n) or (1/2)*n, rather than 1/2n.".into(),
        );
    }
    if mode == "Explicit"
        && p.s("formula")
            .split(|c: char| !c.is_ascii_alphabetic())
            .any(|s| s == "a")
    {
        return Err("Use n in an explicit formula; choose Recursive for previous terms.".into());
    }
    let first = if mode == "Explicit" {
        N::int(0)
    } else {
        number(p.s("first")).ok_or("The app cannot read First term as a supported number")?
    };
    let k = match mode {
        "Arithmetic" => number(p.s("difference")),
        "Geometric" => number(p.s("ratio")),
        _ => Some(N::int(0)),
    }
    .ok_or("The app cannot read the difference or ratio as a supported number")?;
    let start = if p.s("start") == "0" { 0 } else { 1 };
    let mut a: Vec<N> = vec![];
    let mut sums = vec![];
    let mut total = N::int(0);
    for j in 0..p.i("count").clamp(3, 24) {
        let n = N::int((j + start) as i128);
        let x = match mode {
            "Explicit" => read(p.s("formula"), n, N::int(0), N::int(0)),
            "Recursive" => {
                if j == 0 {
                    Some(first)
                } else if j == 1 && recurrence_order(p)==2 {
                    number(p.s("second"))
                } else {
                    read(p.s("recurrence"), n, a[j as usize - 1], if j>=2 {a[j as usize - 2]}else{N::int(0)})
                }
            }
            "Geometric" => Some(if j == 0 {
                first
            } else {
                first.op(k.op(N::int(j as i128), '^'), '*')
            }),
            _ => Some(if j == 0 {
                first
            } else {
                first.op(k.op(N::int(j as i128), '*'), '+')
            }),
        }
        .ok_or(format!(
            "The app cannot evaluate term at index {}. Check the expression and its domain. Previous terms may be written a_(n-1), a_(n-2), a_{{n-1}} or a(n-1); negative-base fractional powers are not supported.",
            j + start
        ))?;
        if !x.v.is_finite() {
            return Err("A term cannot be represented accurately; use less extreme values.".into());
        }
        if x.v.abs() > 1e9 {
            return Err("A term is too large to draw; use fewer terms or a smaller value.".into());
        }
        total = if mode == "Geometric" && k.q == Some((-1, 1)) {
            if j % 2 == 0 { first } else { N::int(0) }
        } else {
            total.op(x, '+')
        };
        if mode == "Explicit" {
            if let Some(exact) = trig_sum(p.s("formula"), start as usize, j as usize + 1) {
                total = exact;
            }
        }
        if !total.v.is_finite() || total.v.abs() > 1e9 {
            return Err(
                "A partial sum cannot be displayed accurately; use fewer terms or less extreme values.".into(),
            );
        }
        a.push(x);
        sums.push(total);
    }
    Ok(Data {
        a,
        s: sums,
        first,
        k,
    })
}
fn limits(p: &Params, d: &Data) -> (Option<N>, Option<N>, String) {
    let zero = N::int(0);
    if (d.first.q.is_none() && d.first.v.abs() < 1e-12 && p.s("mode") != "Explicit")
        || (d.k.q.is_none()
            && ((d.k.v.abs() - 1.0).abs() < 1e-12 || d.k.v.abs() < 1e-12)
            && matches!(p.s("mode"), "Arithmetic" | "Geometric"))
    {
        return (
            None,
            None,
            "cannot determine from the precision of this expression".into(),
        );
    }
    match p.s("mode") {
        "Arithmetic" => {
            if d.k.q.is_some_and(|(a, _)| a == 0) {
                (
                    Some(d.first),
                    if d.first.q.is_some_and(|(a, _)| a == 0) {
                        Some(zero)
                    } else {
                        None
                    },
                    "constant".into(),
                )
            } else {
                (None, None, "diverges".into())
            }
        }
        "Geometric" => {
            if d.first.q.is_some_and(|(a, _)| a == 0) {
                (Some(zero), Some(zero), "converges".into())
            } else if d
                .k
                .q
                .map_or(d.k.v.abs() < 1.0 - 1e-12, |(a, b)| a.abs() < b)
            {
                (
                    Some(zero),
                    Some(d.first.op(N::int(1).op(d.k, '-'), '/')),
                    "converges".into(),
                )
            } else if d.k.q.is_some_and(|(a, b)| a == b) {
                (Some(d.first), None, "constant; series diverges".into())
            } else {
                (None, None, "diverges".into())
            }
        }
        _ => limit_math::analyse(p, d),
    }
}
fn in_window(v: f64, lo: f64, hi: f64) -> bool {
    let e = (hi - lo).abs() * 1e-10 + f64::EPSILON * lo.abs().max(hi.abs()) * 16.0;
    v >= lo - e && v <= hi + e
}
fn axis_text(v: f64) -> String {
    if v != 0.0 && v.abs() < 0.001 * (1.0 - 1e-10) {
        let raw = format!("{v:.4e}");
        let parts: Vec<&str> = raw.split('e').collect();
        let m = parts[0].trim_end_matches('0').trim_end_matches('.');
        let e = parts[1].parse::<i32>().unwrap_or(0);
        let value = format!("{m}e{e}");
        if (value.parse::<f64>().unwrap_or(v) - v).abs() > v.abs() * 1e-12 {
            format!("≈ {value}")
        } else {
            value
        }
    } else {
        approximation(v)
    }
}
fn display(p: &Params) -> &str {
    if p.s("layout") == "Two aligned graphs" {
        "Both"
    } else {
        p.s("display")
    }
}
fn can_limit(p: &Params) -> bool {
    let Ok(d) = data(p) else { return false };
    let (a, b, _) = limits(p, &d);
    match display(p) {
        "Both" => a.is_some() || b.is_some(),
        "Partial sums" => b.is_some(),
        _ => a.is_some(),
    }
}
pub fn relevant(key: &str, p: &Params) -> bool {
    match key {
        "sum_count" => p.b("sum_n") || p.b("sigma"),
        "first" => p.s("mode") != "Explicit",
        "difference" => p.s("mode") == "Arithmetic",
        "ratio" => p.s("mode") == "Geometric",
        "formula" => p.s("mode") == "Explicit",
        "recurrence" => p.s("mode") == "Recursive",
        "second" => p.s("mode") == "Recursive" && recurrence_order(p) == 2,
        "title" => !p.b("blank_title") && p.b(SHOW_ANSWERS),
        "blank_title" => p.b(SHOW_ANSWERS),
        "limit" => p.b(SHOW_ANSWERS) && can_limit(p),
        "point_values" => p.s("layout") != "Number line",
        "display" => p.s("layout") != "Two aligned graphs",
        "grid" | "step" | "xlabel" => p.s("layout") != "Number line",
        "epsilon" => {
            can_limit(p)
                && p.b("band")
                && p.b("limit")
                && p.b(SHOW_ANSWERS)
                && p.s("layout") != "Number line"
        }
        "band" => {
            can_limit(p) && p.b("limit") && p.b(SHOW_ANSWERS) && p.s("layout") != "Number line"
        }
        "missing" => p.s("task") == "Missing terms",
        "terms" => p.b(SHOW_ANSWERS) || p.s("task") != "Missing terms",
        "given" => !p.b(SHOW_ANSWERS) && p.s("task") == "Continue",
        _ => true,
    }
}
fn hide_partial_sums(p: &Params) -> bool {
    !p.b(SHOW_ANSWERS) && (p.s("task") != "Find formula" || p.b("sum") || p.b("sum_n"))
}
fn missing(p: &Params) -> Vec<i64> {
    p.s("missing")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}
fn hidden(p: &Params, j: usize) -> bool {
    !p.b(SHOW_ANSWERS)
        && match p.s("task") {
            "Missing terms" => {
                missing(p).contains(&(j as i64 + if p.s("start") == "0" { 0 } else { 1 }))
            }
            "Continue" => {
                j >= p
                    .i("given")
                    .clamp(1, (p.i("count").clamp(3, 24) - 1).min(6)) as usize
            }
            _ => false,
        }
}
pub fn warnings(p: &Params) -> Vec<String> {
    let mut w = vec![];
    if !p.b(SHOW_ANSWERS) && p.s("task") == "Continue" && p.i("given") >= p.i("count") {
        w.push(
            "Terms given is limited to one fewer than Number of terms, leaving a term to continue."
                .into(),
        );
    }
    if p.s("layout") != "Number line"
        && p.s("grid") != "None"
        && (p.f("to") - p.f("from")) / p.f("step").max(0.000001) > 800.0
    {
        w.push("There are too many horizontal gridlines to draw; increase Count by.".into());
    }
    if p.f("to") <= p.f("from") {
        w.push("Vertical To must be greater than From.".into())
    }
    if p.s("layout") != "Number line"
        && p.f("to") > p.f("from")
        && p.f("step") / (p.f("to") - p.f("from")) * plot_height(p)
            < 14.0 * svg::label_factor(p.s("label_size")) * 1.5
    {
        w.push("Vertical tick labels do not fit at this Count by; increase Count by.".into());
    }
    match data(p) {
        Err(e) => w.push(e),
        Ok(d) => {
            let (_, infinite, _) = limits(p, &d);
            if (p.b("sum") || p.b("convergence") || p.b("limit"))
                && p.b(SHOW_ANSWERS)
                && infinite.is_some_and(|v| v.v.abs() > 1e9)
            {
                w.push("The infinite sum exceeds the supported display range; its value is left blank.".into());
            }
            if p.s("marks") == "Spikes"
                && p.s("layout") != "Number line"
                && (p.f("from") > 0.0 || p.f("to") < 0.0)
            {
                w.push(
                    "The zero axis is outside the window; spikes stop at the window edge.".into(),
                );
            }
            if (0..if p.s("layout") == "Two aligned graphs" {
                2
            } else {
                1
            })
                .any(|g| point_labels(p, &d, g).1 > 0)
            {
                w.push(
                    "Some point labels cannot fit without collisions. Turn off Label point values."
                        .into(),
                );
            }
            if d.a
                .iter()
                .chain(
                    if display(p) != "Terms" || p.s("layout") == "Two aligned graphs" {
                        d.s.iter()
                    } else {
                        d.a.iter()
                    },
                )
                .any(|x| x.v < p.f("from") || x.v > p.f("to"))
            {
                w.push(
                    "Some terms or sums lie beyond the window. Axis bounds support −100 to 100; adjust the bounds or sequence values.".into(),
                )
            }
            if can_limit(p) && p.b("limit") && p.b(SHOW_ANSWERS) {
                let (l, s, _) = limits(p, &d);
                let requested = match display(p) {
                    "Both" => vec![l, s],
                    "Partial sums" => vec![s],
                    _ => vec![l],
                };
                if requested.iter().any(Option::is_none) {
                    w.push(
                        "A requested limit cannot be established; its limit line is omitted."
                            .into(),
                    );
                }
                if requested
                    .iter()
                    .flatten()
                    .any(|v| v.v < p.f("from") || v.v > p.f("to"))
                {
                    w.push("A limit lies outside the window. Axis bounds support −100 to 100; adjust the bounds or sequence.".into());
                }
                if relevant("epsilon", p) {
                    if let Some(e) = number(p.s("epsilon")) {
                        if e.v > 0.0
                            && requested
                                .iter()
                                .flatten()
                                .any(|v| v.v - e.v < p.f("from") || v.v + e.v > p.f("to"))
                        {
                            w.push(
                                "The epsilon band extends beyond the window; adjust the bounds or turn off Epsilon band."
                                    .into(),
                            );
                        }
                    }
                }
            }
        }
    }
    if relevant("epsilon", p) && number(p.s("epsilon")).is_none_or(|x| x.v <= 0.0) {
        w.push("Epsilon must be a positive number.".into())
    }
    if p.b("sum_n") {
        if let Ok(d) = data(p) {
            match series_math::total(p,&d){Err(e)=>w.push(e),Ok(a)if a.digits>110=>w.push("The exact requested sum is too long for a readable line; the result is marked approximate. Arithmetic/geometric general formulas remain exact.".into()),_=>()}
        }
    }
    if p.b("point_values")
        && data(p).is_ok_and(|d| d.a.iter().chain(d.s.iter()).any(|n| n.text().len() > 24))
    {
        w.push("Long point values are marked approximate for readability; the key lists exact values where available.".into());
    }
    for k in ["xlabel", "ylabel", "title"] {
        if relevant(k, p) && p.s(k).chars().count() > 48 {
            w.push(format!("Shorten {k} to at most 48 characters."))
        }
    }
    for k in ["xlabel", "ylabel"] {
        if relevant(k, p) && axis_name(p, k) != p.s(k) {
            w.push("The axis name is shortened to a fitting word, or omitted if no word fits. Use a short name without sequence symbols beside the arrow.".into());
        }
    }
    if !p.b(SHOW_ANSWERS) && p.s("task") == "Missing terms" {
        let start = if p.s("start") == "0" { 0 } else { 1 };
        if p.s("missing").split(',').any(|v| {
            v.trim()
                .parse::<i64>()
                .map_or(true, |v| v < start || v >= start + p.i("count"))
        }) {
            w.push(
                "Enter comma-separated indices within the displayed sequence for Missing terms."
                    .into(),
            );
        }
    }
    w
}
fn axis_name<'a>(p: &'a Params, key: &str) -> &'a str {
    let t = p.s(key);
    let default = if key == "xlabel" { "n" } else { "aₙ" };
    if t == default {
        return t;
    }
    if matches!(t.trim(), "n" | "a" | "r" | "d" | "S" | "e" | "pi" | "π") {
        return "";
    }
    let fits = |s: &str| {
        circle::tw(s, 14.0 * svg::label_factor(p.s("label_size")), false) <= 60.0
            && s.chars().count() <= 8
    };
    if fits(t) {
        t
    } else {
        t.split_whitespace().find(|word| fits(word)).unwrap_or("")
    }
}
fn coefficient(p: &Params, key: &str) -> String {
    let t = p.s(key);
    match number(t) {
        Some(n) if n.q.is_some() && !n.dec => n.text(),
        Some(n) if t.parse::<f64>().is_ok() => {
            if t.contains('e') || t.contains('E') {
                n.v.to_string()
            } else {
                n.text()
            }
        }
        Some(n) if n.q.is_some() && !n.text().starts_with('≈') => n.text(),
        _ => fmt_formula(t),
    }
}
fn fmt_formula(s: &str) -> String {
    let s = super::math::pretty(&recurrence_spelling(s))
        .replace("a_(n−1)", "aₙ₋₁")
        .replace("a_(n−2)", "aₙ₋₂");
    let s = if s.contains('/') {
        s
    } else {
        s.replace("·√", "√").replace("·π", "π")
    };
    let s = s.replace('·', " × ");
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '^' {
            i += 1;
            let mut power = String::new();
            if cs.get(i) == Some(&'(') {
                i += 1;
                while i < cs.len() && cs[i] != ')' {
                    power.push(cs[i]);
                    i += 1
                }
                if i < cs.len() {
                    i += 1
                }
            } else {
                while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == 'n' || cs[i] == '−') {
                    power.push(cs[i]);
                    i += 1
                }
            }
            if !power.is_empty() {
                out.push_str(&format!("^({power})"));
            } else {
                out.push('^');
            }
        } else {
            out.push(cs[i]);
            i += 1
        }
    }
    out
}
struct PointLabel {
    series: usize,
    j: usize,
    x: f64,
    y: f64,
    anchor: &'static str,
    text: String,
    rect: circle::Rect,
    leader: Option<((f64, f64), (f64, f64))>,
}
fn segments_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let r = (b.0 - a.0, b.1 - a.1);
    let t = (d.0 - c.0, d.1 - c.1);
    let v = (c.0 - a.0, c.1 - a.1);
    let cross = |a: (f64, f64), b: (f64, f64)| a.0 * b.1 - a.1 * b.0;
    let den = cross(r, t);
    if den.abs() < 1e-8 {
        if cross(v, r).abs() > 1e-8 {
            return false;
        }
        let len = r.0 * r.0 + r.1 * r.1;
        if len < 1e-8 {
            return false;
        }
        let p = (v.0 * r.0 + v.1 * r.1) / len;
        let q = ((d.0 - a.0) * r.0 + (d.1 - a.1) * r.1) / len;
        return p.max(q) > 0.04 && p.min(q) < 1.0;
    }
    let f = cross(v, t) / den;
    let g = cross(v, r) / den;
    f > 0.04 && f <= 1.0 && g >= 0.0 && g <= 1.0
}
fn plot_height(p: &Params) -> f64 {
    (if p.s("layout") == "Number line" {
        220.0
    } else {
        560.0
    }) * svg::label_factor(p.s("label_size")).max(1.0)
}
fn point_text(n: N, fs: f64) -> String {
    let text = n.text();
    if circle::tw(&text, fs, false) > 115.0 {
        format!("≈ {}", approximation(n.v))
    } else {
        text
    }
}
fn point_labels(p: &Params, d: &Data, g: usize) -> (Vec<PointLabel>, usize) {
    if !p.b("point_values") || p.s("layout") == "Number line" {
        return (vec![], 0);
    }
    let fs = 14.0 * svg::label_factor(p.s("label_size"));
    let count = d.a.len();
    let width = canvas_width(p, count);
    let (left, right) = (150.0, width - 85.0);
    let top = 160.0 + g as f64 * (plot_height(p) + 160.0);
    let bottom = top + plot_height(p);
    let (lo, hi) = (p.f("from"), p.f("to"));
    if hi <= lo {
        return (vec![], 0);
    }
    let series: Vec<usize> = if p.s("layout") == "Two aligned graphs" {
        vec![g]
    } else {
        match display(p) {
            "Both" => vec![0, 1],
            "Partial sums" => vec![1],
            _ => vec![0],
        }
    };
    let mut points = vec![];
    let mut lines = vec![];
    let mut guides = vec![];
    for s in series.clone() {
        let values = if s == 0 { &d.a } else { &d.s };
        let mut prev = None;
        for (j, v) in values.iter().enumerate() {
            if hidden(p, j) || (hide_partial_sums(p) && s == 1) || !in_window(v.v, lo, hi) {
                prev = None;
                continue;
            }
            let pos = (
                185.0 + j as f64 * (width - 305.0) / (count - 1) as f64,
                bottom - (v.v.clamp(lo, hi) - lo) / (hi - lo) * plot_height(p),
            );
            points.push((s, j, pos, point_text(*v, fs)));
            if p.b("join") {
                if let Some(a) = prev {
                    guides.push((a, pos));
                }
            }
            if p.s("marks") == "Spikes" {
                lines.push((
                    (
                        pos.0,
                        bottom - (0.0f64.clamp(lo, hi) - lo) / (hi - lo) * plot_height(p),
                    ),
                    pos,
                ));
            }
            prev = Some(pos);
        }
    }
    if lo <= 0.0 && hi >= 0.0 {
        let y = bottom - (0.0 - lo) / (hi - lo) * plot_height(p);
        lines.push(((left, y), (right, y)));
    }
    if can_limit(p) && p.b("limit") && p.b(SHOW_ANSWERS) {
        let (a, b, _) = limits(p, d);
        for l in series
            .iter()
            .filter_map(|s| if *s == 0 { a } else { b })
            .filter(|l| in_window(l.v, lo, hi))
        {
            let y = bottom - (l.v - lo) / (hi - lo) * plot_height(p);
            lines.push(((left, y), (right, y)));
        }
    }
    points.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then_with(|| a.2.1.total_cmp(&b.2.1))
            .then_with(|| a.0.cmp(&b.0))
    });
    let mut placed: Vec<PointLabel> = vec![];
    let mut refused = 0;
    for (s, j, pos, text) in &points {
        if placed
            .iter()
            .any(|l| l.j == *j && l.series != *s && l.text == *text && d.a[*j].v == d.s[*j].v)
        {
            continue;
        }
        let w = circle::tw(text, fs, false);
        let mut found = None;
        for allow_guide_gaps in [false, true] {
            if found.is_some() {
                break;
            }
            for (candidate, (x, y, anchor)) in [
                (pos.0, pos.1 - fs, "middle"),
                (pos.0, pos.1 + fs * 1.5, "middle"),
                (pos.0, pos.1 - fs * 0.25 - 13.0, "middle"),
                (pos.0, pos.1 + fs + 8.0, "middle"),
                (pos.0 + 11.0, pos.1 + fs * 0.3, "start"),
                (pos.0 - 11.0, pos.1 + fs * 0.3, "end"),
                (pos.0 + 11.0, pos.1 - fs * 0.5, "start"),
                (pos.0 - 11.0, pos.1 - fs * 0.5, "end"),
                (pos.0 + 11.0, pos.1 + fs * 1.15, "start"),
                (pos.0 - 11.0, pos.1 + fs * 1.15, "end"),
                (pos.0 + 8.0, pos.1 - fs * 1.85, "start"),
                (pos.0 - 8.0, pos.1 - fs * 1.85, "end"),
                (pos.0 + 8.0, pos.1 - fs * 1.95, "start"),
                (pos.0 - 8.0, pos.1 - fs * 1.95, "end"),
                (pos.0 + 8.0, pos.1 - fs * 1.8, "start"),
                (pos.0 - 8.0, pos.1 - fs * 1.8, "end"),
                (pos.0 + 8.0, pos.1 - fs * 1.8, "start"),
                (pos.0 - 8.0, pos.1 - fs * 1.8, "end"),
                (pos.0 + 2.0 * fs + 4.0, pos.1 + fs * 0.375, "start"),
                (pos.0 - 2.0 * fs - 4.0, pos.1 + fs * 0.375, "end"),
                (pos.0 + 2.0 * fs + 4.0, pos.1 - fs * 0.05, "start"),
                (pos.0 - 2.0 * fs - 4.0, pos.1 - fs * 0.05, "end"),
                (pos.0 + 2.0 * fs + 4.0, pos.1 - fs * 0.45, "start"),
                (pos.0 - 2.0 * fs - 4.0, pos.1 - fs * 0.45, "end"),
                (pos.0 + 2.0 * fs + 4.0, pos.1 + fs * 1.05, "start"),
                (pos.0 - 2.0 * fs - 4.0, pos.1 + fs * 1.05, "end"),
            ]
            .into_iter()
            .enumerate()
            {
                if allow_guide_gaps && candidate >= 10 {
                    continue;
                }
                let x0 = match anchor {
                    "middle" => x - w / 2.0,
                    "end" => x - w,
                    _ => x,
                };
                let r = circle::Rect {
                    x0,
                    y0: y - fs,
                    x1: x0 + w,
                    y1: y + fs * 0.25,
                };
                let overlaps = |a: &circle::Rect, b: &circle::Rect| {
                    a.x0 < b.x1 + 2.0 && a.x1 + 2.0 > b.x0 && a.y0 < b.y1 + 2.0 && a.y1 + 2.0 > b.y0
                };
                if r.x0 < left + 8.0
                    || r.x1 > right + 50.0
                    || r.y0 < top - 20.0
                    || r.y1 > bottom + 35.0
                {
                    continue;
                }
                let axis_label = if g == 1 && p.s("layout") == "Two aligned graphs" {
                    "Sₙ"
                } else {
                    axis_name(p, "ylabel")
                };
                let axis_rect = circle::Rect {
                    x0: left,
                    x1: left + circle::tw(axis_label, fs, false),
                    y0: top - 30.0 - fs,
                    y1: top - 30.0 + fs * 0.25,
                };
                if overlaps(&r, &axis_rect) {
                    continue;
                }
                let center = ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0);
                let distance = |pt: (f64, f64)| (pt.0 - center.0).hypot(pt.1 - center.1);
                if points.iter().any(|(_, _, pt, _)| {
                    (pt.0 - pos.0).hypot(pt.1 - pos.1) > 0.1 && distance(*pt) < distance(*pos) + 2.0
                }) {
                    continue;
                }
                if lo <= 0.0
                    && hi >= 0.0
                    && circle::seg_hits(
                        (left, bottom - (0.0 - lo) / (hi - lo) * plot_height(p)),
                        (right, bottom - (0.0 - lo) / (hi - lo) * plot_height(p)),
                        &r,
                    )
                {
                    continue;
                }
                if can_limit(p) && p.b("limit") && p.b(SHOW_ANSWERS) {
                    let (a, b, _) = limits(p, d);
                    if series
                        .iter()
                        .filter_map(|s| if *s == 0 { a } else { b })
                        .any(|l| {
                            l.v >= lo
                                && l.v <= hi
                                && circle::seg_hits(
                                    (left, bottom - (l.v - lo) / (hi - lo) * plot_height(p)),
                                    (right, bottom - (l.v - lo) / (hi - lo) * plot_height(p)),
                                    &r,
                                )
                        })
                    {
                        continue;
                    }
                }
                if placed.iter().any(|a| {
                    overlaps(&r, &a.rect)
                        || a.leader.is_some_and(|(a, b)| circle::seg_hits(a, b, &r))
                }) || points.iter().any(|(_, _, pt, _)| {
                    overlaps(
                        &r,
                        &circle::Rect {
                            x0: pt.0 - 5.0,
                            x1: pt.0 + 5.0,
                            y0: pt.1 - 5.0,
                            y1: pt.1 + 5.0,
                        },
                    )
                }) || lines.iter().any(|(a, b)| circle::seg_hits(*a, *b, &r))
                {
                    continue;
                }
                let leader = if candidate >= 10 {
                    Some((
                        *pos,
                        (
                            if anchor == "end" {
                                r.x1 + 2.0
                            } else {
                                r.x0 - 2.0
                            },
                            (r.y0 + r.y1) / 2.0,
                        ),
                    ))
                } else {
                    None
                };
                if let Some((a, b)) = leader {
                    if lines
                        .iter()
                        .chain(guides.iter())
                        .any(|(c, d)| segments_cross(a, b, *c, *d))
                        || placed.iter().any(|l| {
                            circle::seg_hits(a, b, &l.rect)
                                || l.leader.is_some_and(|(c, d)| segments_cross(a, b, c, d))
                        })
                        || points.iter().any(|(_, _, pt, _)| {
                            (pt.0 - pos.0).hypot(pt.1 - pos.1) > 10.0
                                && circle::seg_hits(
                                    a,
                                    b,
                                    &circle::Rect {
                                        x0: pt.0 - 4.0,
                                        x1: pt.0 + 4.0,
                                        y0: pt.1 - 4.0,
                                        y1: pt.1 + 4.0,
                                    },
                                )
                        })
                    {
                        continue;
                    }
                }
                found = Some(PointLabel {
                    series: *s,
                    j: *j,
                    x,
                    y,
                    anchor,
                    text: text.clone(),
                    rect: r,
                    leader,
                });
                break;
            }
        }
        if let Some(l) = found {
            placed.push(l)
        } else {
            refused += 1
        }
    }
    (placed, refused)
}
fn join_segment(d: &mut Doc, a: (f64, f64), b: (f64, f64), color: &str, labels: &[PointLabel]) {
    let mut pieces = vec![(0.0f64, 1.0f64)];
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    for label in labels {
        let r = label.rect;
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        let mut hit = true;
        for (p, q) in [
            (-dx, a.0 - r.x0 + 2.0),
            (dx, r.x1 + 2.0 - a.0),
            (-dy, a.1 - r.y0 + 2.0),
            (dy, r.y1 + 2.0 - a.1),
        ] {
            if p.abs() < 1e-12 {
                if q < 0.0 {
                    hit = false;
                    break;
                }
            } else {
                let t = q / p;
                if p < 0.0 {
                    lo = lo.max(t)
                } else {
                    hi = hi.min(t)
                }
                if lo > hi {
                    hit = false;
                    break;
                }
            }
        }
        if hit {
            let mut next = vec![];
            for (x, y) in pieces {
                if hi <= x || lo >= y {
                    next.push((x, y))
                } else {
                    if x < lo {
                        next.push((x, lo))
                    }
                    if hi < y {
                        next.push((hi, y))
                    }
                }
            }
            pieces = next;
        }
    }
    for (x, y) in pieces {
        if y - x > 1e-8 {
            d.line(
                a.0 + x * dx,
                a.1 + x * dy,
                a.0 + y * dx,
                a.1 + y * dy,
                color,
                0.8,
            );
        }
    }
}
fn rows_for(p: &Params, d: &Data) -> Vec<String> {
    let ans = p.b(SHOW_ANSWERS);
    let start = if p.s("start") == "0" { 0 } else { 1 };
    let count = d.a.len();
    let (lo, hi) = (p.f("from"), p.f("to"));
    let mut rows = vec![format!("Task: {}", p.s("task"))];
    let (lim, slim, state) = limits(p, &d);
    let idx = subscript(start);
    let formula = if p.s("mode") == "Recursive" {
        "Explicit formula: __________ (cannot determine)".into()
    } else {
        format!("aₙ = {}", series_math::formula(p, d, "n"))
    };
    if p.b("explicit") {
        rows.push(if ans {
            formula.clone()
        } else {
            "Formula: ________________________".into()
        })
    }
    if !ans && p.s("task") == "Find formula" && !p.b("explicit") && !p.b("recursive") {
        rows.push("Formula: ________________________".into());
    }
    if p.b("recursive") {
        rows.push(if !ans {
            "Recurrence: ________________________".into()
        } else {
            match p.s("mode") {
                "Arithmetic" => format!(
                    "aₙ = aₙ₋₁ + ({}); a{idx} {}",
                    coefficient(p, "difference"),
                    d.first.eq()
                ),
                "Geometric" => format!(
                    "aₙ = aₙ₋₁ × ({}); a{idx} {}",
                    coefficient(p, "ratio"),
                    d.first.eq()
                ),
                "Recursive" => {
                    let mut row = format!(
                        "aₙ = {} (n ≥ {}); a{} {}",
                        fmt_formula(p.s("recurrence")),
                        start + recurrence_order(p),
                        subscript(start),
                        d.a[0].eq()
                    );
                    if recurrence_order(p) == 2 {
                        row.push_str(&format!("; a{} {}", subscript(start + 1), d.a[1].eq()));
                    }
                    row
                }
                _ => "Recurrence: __________ (cannot determine)".into(),
            }
        })
    }
    if p.b("sum") {
        if ans && matches!(p.s("mode"), "Arithmetic" | "Geometric") {
            let a = coefficient(p, "first");
            let k = coefficient(
                p,
                if p.s("mode") == "Arithmetic" {
                    "difference"
                } else {
                    "ratio"
                },
            );
            rows.push(if p.s("mode") == "Arithmetic" {
                format!("For N terms: S = N[2 × ({a}) + (N − 1) × ({k})]/2")
            } else if d.k.q.is_some_and(|(a, b)| a == b) {
                format!("For N terms: S = N × ({a})")
            } else {
                format!("For N terms: S = ({a})[1 − ({k})^(N)]/[1 − ({k})]")
            });
        }
        rows.push(if ans {
            format!("Sum of {count} terms {}", d.s[count - 1].eq())
        } else {
            "Sum of terms = __________".into()
        });
        if p.s("mode") == "Geometric" {
            rows.push(if ans {
                slim.map(|l| {
                    if l.v.abs() > 1e9 {
                        return "Sum to infinity: __________ (beyond display range)".into();
                    }
                    if d.first.q == Some((0, 1)) || d.k.q.is_some_and(|(a, b)| a == b) {
                        format!("Sum to infinity {}", l.eq())
                    } else {
                        format!(
                            "S∞ = ({})/(1 − ({})) {}",
                            coefficient(p, "first"),
                            if d.k.v < 0.0 {
                                format!("({})", coefficient(p, "ratio"))
                            } else {
                                coefficient(p, "ratio")
                            },
                            l.eq()
                        )
                    }
                })
                .unwrap_or(if state.contains("cannot determine") {
                    "Sum to infinity: __________ (cannot determine)".into()
                } else {
                    "Series diverges".into()
                })
            } else {
                "Sum to infinity: __________".into()
            })
        }
    }
    if p.b("sum_n") {
        let n = p.i("sum_count").clamp(1, 1000);
        rows.push(if ans {
            match series_math::total(p, d) {
                Ok(a) => format!("Sum of first {n} terms {}", a.eq()),
                Err(_) => format!("Sum of first {n} terms: __________ (cannot determine)"),
            }
        } else {
            format!("Sum of first {n} terms: __________")
        });
    }
    if p.b("sigma") {
        let n = p.i("sum_count").clamp(1, 1000) as usize;
        rows.push(format!(
            "Σ_(k = {start})^({}) {}",
            start + n - 1,
            if ans {
                format!("[{}]", series_math::formula(p, d, "k"))
            } else {
                "[__________]".into()
            }
        ));
    }
    if p.b("convergence") {
        let state = if display(p) == "Partial sums" {
            if slim.is_some() {
                "converges"
            } else if matches!(p.s("mode"), "Arithmetic" | "Geometric")
                && !state.contains("cannot determine")
            {
                "diverges"
            } else {
                "cannot determine"
            }
        } else {
            state.as_str()
        };
        rows.push(if ans {
            let selected = if display(p) == "Partial sums" {
                slim
            } else {
                lim
            };
            let kind = if display(p) == "Partial sums" {
                "Series"
            } else {
                "Sequence"
            };
            if state.contains("cannot determine") {
                "Convergence and limit: __________ (cannot determine)".into()
            } else if let Some(value) = selected {
                if value.v.abs() > 1e9 {
                    format!("{kind}: {state}; limit __________ (beyond display range)")
                } else {
                    format!("{kind}: {state}; limit {}", value.eq())
                }
            } else {
                format!("{kind}: {state}; no finite limit")
            }
        } else {
            "Convergence and limit: ________________________".into()
        })
    }
    if p.b("terms") || p.b("sums") || p.s("task") == "Missing terms" {
        for j in 0..count {
            let hide = hidden(p, j);
            let term = if hide {
                "= _____".into()
            } else if !ans && !in_window(d.a[j].v, lo, hi) {
                "outside window".into()
            } else {
                d.a[j].eq()
            };
            let sum = if hide_partial_sums(p) {
                "= _____".into()
            } else if !ans && !in_window(d.s[j].v, lo, hi) {
                "outside window".into()
            } else {
                d.s[j].eq()
            };
            let mut row = String::new();
            if p.b("terms") || p.s("task") == "Missing terms" {
                row.push_str(&format!(
                    "a{} {term}{}",
                    subscript(j + start),
                    if ans
                        && p.s("task") == "Missing terms"
                        && missing(p).contains(&((j + start) as i64))
                    {
                        " (asked)"
                    } else {
                        ""
                    }
                ))
            }
            if p.b("sums") {
                row.push_str(&format!(
                    "\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}S{} {sum}",
                    subscript(j + start)
                ))
            }
            rows.push(row)
        }
    }
    rows
}
fn canvas_width(p: &Params, count: usize) -> f64 {
    let fs = 14.0 * svg::label_factor(p.s("label_size"));
    let mut spacing: f64 = 100.0;
    if p.b("point_values") && p.s("layout") != "Number line" {
        if let Ok(d) = data(p) {
            for n in d.a.iter().chain(d.s.iter()) {
                spacing = spacing.max(circle::tw(&point_text(*n, fs), fs, false) + 45.0);
            }
        }
    }
    let base =
        (count as f64 * spacing + 225.0).max(965.0) * svg::label_factor(p.s("label_size")).max(1.0);
    let expr = match p.s("mode") {
        "Explicit" => p.s("formula"),
        "Recursive" => p.s("recurrence"),
        _ => "",
    };
    let recursive_width = if p.s("mode") == "Recursive" {
        let seeds = ["first", "second"]
            .iter()
            .filter(|k| **k != "second" || recurrence_order(p) == 2)
            .filter_map(|k| number(p.s(k)))
            .map(|v| v.text().chars().count())
            .sum::<usize>();
        (expr.chars().count() + seeds + 55) as f64 * fs * 0.8 + 170.0
    } else {
        0.0
    };
    let mut content_width: f64 = 0.0;
    if let Ok(d) = data(p) {
        for row in rows_for(p, &d) {
            content_width = content_width.max(circle::tw(&row, fs, false) + 235.0);
        }
        if p.b(SHOW_ANSWERS) && p.b("limit") {
            let (a, b, _) = limits(p, &d);
            let requested = match display(p) {
                "Both" => vec![a, b],
                "Partial sums" => vec![b],
                _ => vec![a],
            };
            for l in requested
                .into_iter()
                .flatten()
                .filter(|l| in_window(l.v, p.f("from"), p.f("to")))
            {
                let caption = format!("Term limit {}", l.eq());
                content_width = content_width.max(
                    circle::tw(&caption, fs, false)
                        + circle::tw("● terms    □ partial sums", fs, false)
                        + 275.0,
                );
            }
        }
    }
    base.max(content_width)
        .max(expr.chars().count() as f64 * fs * 0.85 + 240.0)
        .max(recursive_width)
        .max(if matches!(p.s("mode"), "Arithmetic" | "Geometric") {
            (coefficient(p, "first").chars().count()
                + coefficient(
                    p,
                    if p.s("mode") == "Arithmetic" {
                        "difference"
                    } else {
                        "ratio"
                    },
                )
                .chars()
                .count()) as f64
                * fs
                * 0.75
                + 500.0
        } else {
            0.0
        })
}
fn render(p: &Params) -> String {
    let Ok(d) = data(p) else {
        let mut doc = Doc::new(650.0, 100.0);
        doc.describe(
            "Sequences and series",
            "Discrete points and worksheet prompts",
        );
        return doc.finish();
    };
    let ans = p.b(SHOW_ANSWERS);
    let start = if p.s("start") == "0" { 0 } else { 1 };
    let count = d.a.len();
    let fs = 14.0 * svg::label_factor(p.s("label_size"));
    let width = canvas_width(p, count);
    let left = 150.0;
    let right = width - 85.0;
    let px = |j: usize| left + 35.0 + j as f64 * (right - left - 70.0) / (count - 1) as f64;
    let (lo, hi) = (p.f("from"), p.f("to"));
    if hi <= lo {
        return Doc::new(650.0, 100.0).finish();
    }
    let split = p.s("layout") == "Two aligned graphs";
    let numberline = p.s("layout") == "Number line";
    let plot_bottom = if split {
        390.0 + 2.0 * plot_height(p)
    } else {
        230.0 + plot_height(p)
    };
    let rows = rows_for(p, &d);
    let (lim, slim, _) = limits(p, &d);
    let height = plot_bottom + 75.0 + rows.len() as f64 * fs * 1.9;
    let mut doc = Doc::new(width, height);
    doc.raw(&format!("<!--{}-->", " ".repeat(500)));
    doc.describe(
        "Sequences and series",
        "Discrete points and worksheet prompts",
    );
    let fills = svg::Fills::new(p);
    doc.defs(&fills.defs());
    let colors = [fills.line(0), fills.line(1)];
    doc.text(
        left,
        65.0,
        fs,
        "start",
        if split || display(p) == "Both" {
            "● terms    □ partial sums"
        } else if display(p) == "Partial sums" {
            "□ partial sums"
        } else {
            "● terms"
        },
    );
    doc.text(
        left,
        35.0,
        fs * 1.2,
        "start",
        if p.b("blank_title") || !ans {
            "________________________________"
        } else {
            if p.s("title").chars().count() > 48 {
                ""
            } else {
                p.s("title")
            }
        },
    );
    let graphs = if split { 2 } else { 1 };
    for g in 0..graphs {
        let top = 160.0 + g as f64 * (plot_height(p) + 160.0);
        let bottom = top + plot_height(p);
        let py = |v: f64| bottom - (v - lo) / (hi - lo) * plot_height(p);
        let nlx = |v: f64| left + (v - lo) / (hi - lo) * (right - left);
        let valid = |v: f64| in_window(v, lo, hi);
        if !numberline {
            let sw = match p.s("grid") {
                "None" => 0.0,
                "Light" => 0.5,
                "Bold" => 1.6,
                _ => 1.0,
            };
            if sw > 0.0 {
                for j in 0..count {
                    doc.line(px(j), top, px(j), bottom, "#cccccc", sw);
                }
                if (hi - lo) / p.f("step").max(0.000001) <= 800.0 {
                    for k in 0..=800 {
                        let step = p.f("step").max(0.000001);
                        let v = (lo / step).ceil() * step + k as f64 * step;
                        if v > hi + (hi - lo).abs() * 1e-12 {
                            break;
                        }
                        doc.line(left, py(v), right, py(v), "#cccccc", sw);
                    }
                }
            }
            doc.arrow(
                left,
                bottom + 40.0,
                right + 15.0,
                bottom + 40.0,
                "#333333",
                1.3,
            );
            doc.arrow(left, bottom + 40.0, left, top - 15.0, "#333333", 1.3);
            let zero_limit = ans
                && p.b("limit")
                && match display(p) {
                    "Both" => [lim, slim].iter().flatten().any(|l| l.q == Some((0, 1))),
                    "Partial sums" => slim.is_some_and(|l| l.q == Some((0, 1))),
                    _ => lim.is_some_and(|l| l.q == Some((0, 1))),
                };
            if lo <= 0.0 && hi >= 0.0 && !zero_limit {
                doc.line(left, py(0.0), right, py(0.0), "#777777", 1.0);
            }
            // Numbers have a dedicated gutter and row, away from every data label.
            for j in 0..count {
                doc.text(px(j), bottom + 65.0, fs, "middle", &(j + start).to_string());
            }
            for k in 0..=800 {
                let step = p.f("step").max(0.000001);
                let v = (lo / step).ceil() * step + k as f64 * step;
                if v > hi + (hi - lo).abs() * 1e-12 {
                    break;
                }
                if p.f("step") / (hi - lo) * plot_height(p) >= fs * 1.5 {
                    doc.text(left - 10.0, py(v) + 4.0, fs, "end", &axis_text(v));
                }
            }
            doc.text(
                right + 20.0,
                bottom + 45.0,
                fs,
                "start",
                axis_name(p, "xlabel"),
            );
            doc.text(
                left,
                top - 30.0,
                fs,
                "start",
                if (split && g == 1)
                    || (!split && display(p) == "Partial sums" && p.s("ylabel") == "aₙ")
                {
                    "Sₙ"
                } else {
                    axis_name(p, "ylabel")
                },
            );
        } else {
            doc.arrow(left, bottom, right + 15.0, bottom, "#333333", 1.5);
            doc.text(left, bottom + 65.0, fs, "start", &axis_text(lo));
            doc.text(right, bottom + 65.0, fs, "end", &axis_text(hi));
            doc.text(
                right + 20.0,
                bottom + 40.0,
                fs,
                "start",
                axis_name(p, "ylabel"),
            );
        }
        let which: Vec<usize> = if split {
            vec![g]
        } else {
            match display(p) {
                "Partial sums" => vec![1],
                "Both" => vec![0, 1],
                _ => vec![0],
            }
        };
        if ans && can_limit(p) && p.b("limit") && p.b("band") && !numberline {
            if let Some(e) = number(p.s("epsilon")).filter(|e| e.v > 0.0) {
                for s in &which {
                    if let Some(l) = if *s == 0 { lim } else { slim } {
                        if valid(l.v) {
                            let a = (l.v - e.v).max(lo);
                            let b = (l.v + e.v).min(hi);
                            doc.raw("<g opacity=\"0.5\">");
                            doc.rect(
                                left,
                                py(b),
                                right - left,
                                py(a) - py(b),
                                "#dddddd",
                                "none",
                                0.0,
                            );
                            doc.raw("</g>");
                        }
                    }
                }
            }
        }
        let placed = point_labels(p, &d, g).0;
        for series in which {
            let vals = if series == 0 { &d.a } else { &d.s };
            let color = colors[series];
            doc.raw(&format!(
                r#"<g data-series="{series}" stroke-dasharray="{}">"#,
                if series == 1 { "5 3" } else { "none" }
            ));
            if ans && can_limit(p) && p.b("limit") {
                if let Some(l) = if series == 0 { lim } else { slim } {
                    if valid(l.v) {
                        if numberline {
                            doc.line(
                                nlx(l.v),
                                bottom - 10.0,
                                nlx(l.v),
                                bottom + 10.0,
                                "#555555",
                                1.5,
                            );
                            doc.text(
                                right,
                                top - 60.0 - series as f64 * fs * 1.5,
                                fs,
                                "end",
                                &format!(
                                    "{} limit {}",
                                    if series == 0 { "Term" } else { "Sum" },
                                    l.eq()
                                ),
                            );
                        } else {
                            doc.raw(&format!(r##"<line data-limit="true" x1="{left}" y1="{}" x2="{right}" y2="{}" stroke="{}" stroke-width="2" stroke-dasharray="6 4"/>"##,py(l.v),py(l.v),if p.s("style")=="Print"{"#444444"}else{"#007d6a"}));
                            doc.text(
                                right,
                                top - 60.0
                                    - if !split && display(p) == "Both" {
                                        series as f64 * fs * 1.5
                                    } else {
                                        0.0
                                    },
                                fs,
                                "end",
                                &format!(
                                    "{} limit {}",
                                    if series == 0 { "Term" } else { "Sum" },
                                    l.eq()
                                ),
                            );
                        }
                    }
                }
            }
            let mut prev = None;
            for (j, v) in vals.iter().enumerate() {
                if hidden(p, j) || (hide_partial_sums(p) && series == 1) || !valid(v.v) {
                    prev = None;
                    continue;
                }
                let (x, y) = if numberline {
                    (nlx(v.v), bottom - 20.0 - j as f64 * 7.0)
                } else {
                    (px(j), py(v.v))
                };
                if numberline {
                    doc.line(x, bottom, x, y, color, 0.8);
                    doc.circle(x, bottom, 2.5, color, color, 1.0);
                }
                if p.b("join") {
                    if let Some((a, b)) = prev {
                        join_segment(&mut doc, (a, b), (x, y), color, &placed);
                    }
                }
                if p.s("marks") == "Spikes" {
                    doc.line(
                        x,
                        if numberline {
                            bottom
                        } else {
                            py(0.0f64.clamp(lo, hi))
                        },
                        x,
                        y,
                        color,
                        1.5,
                    );
                }
                doc.raw("<g stroke-dasharray=\"none\">");
                if series == 0 {
                    doc.circle(x, y, 3.5, color, color, 1.0);
                } else {
                    doc.rect(x - 3.5, y - 3.5, 7.0, 7.0, "none", color, 1.5);
                }
                doc.raw("</g>");
                prev = Some((x, y));
            }
            doc.raw("</g>");
        }
        for l in placed {
            if let Some((a, b)) = l.leader {
                doc.line(a.0, a.1, b.0, b.1, "#444444", 1.0);
            }
            doc.raw(&format!(r##"<text data-point-label="{}:{}" x="{}" y="{}" font-size="{fs}" font-family="DejaVu Sans, Noto Sans, Liberation Sans, Arial, sans-serif" text-anchor="{}" fill="{}" stroke="#ffffff" stroke-width="3" paint-order="stroke">{}</text>"##,l.series,l.j,l.x,l.y,l.anchor,"#111111",svg::esc(&l.text.replace('-',"−"))));
        }
    }
    for (i, row) in rows.iter().enumerate() {
        // powers in generated equations are typeset with tspan; other expressions use the shared pretty reader.
        let escaped = svg::esc(&row.replace('-', "−"));
        let mut rest = escaped.as_str();
        let mut text = String::new();
        while let Some((k, shift)) = [(rest.find("^("), "super"), (rest.find("_("), "sub")]
            .into_iter()
            .filter_map(|(k, s)| k.map(|k| (k, s)))
            .min_by_key(|(k, _)| *k)
        {
            text.push_str(&rest[..k]);
            let Some(end) = rest[k + 2..].find(')') else {
                break;
            };
            text.push_str(&format!(
                "<tspan baseline-shift=\"{shift}\" font-size=\"70%\">{}</tspan>",
                &rest[k + 2..k + 2 + end]
            ));
            rest = &rest[k + 3 + end..];
        }
        text.push_str(rest);
        doc.raw(&format!(
            r##"<text x="{left}" y="{}" font-size="{fs}" font-family="DejaVu Sans, Noto Sans, Liberation Sans, Arial, sans-serif" fill="#111111">{text}</text>"##,
            plot_bottom + 65.0 + i as f64 * fs * 1.9
        ));
    }
    doc.finish()
}
pub fn handles(p: &Params) -> Vec<interact::Handle> {
    if !matches!(p.s("mode"), "Arithmetic" | "Geometric")
        || p.s("layout") != "One grid"
        || display(p) == "Partial sums"
    {
        return vec![];
    }
    let Ok(d) = data(p) else { return vec![] };
    let count = d.a.len();
    let width = canvas_width(p, count);
    let px = |j: usize| 185.0 + j as f64 * (width - 305.0) / (count - 1) as f64;
    let (lo, hi) = (p.f("from"), p.f("to"));
    if hi <= lo {
        return vec![];
    }
    let height = plot_height(p);
    let py = |v: f64| 160.0 + height - (v.clamp(lo, hi) - lo) / (hi - lo) * height;
    let mut out = vec![];
    for j in [0, 1, count - 1] {
        if (j == 1 && p.s("mode") == "Geometric" && d.first.v == 0.0)
            || hidden(p, j)
            || d.a[j].v < lo
            || d.a[j].v > hi
        {
            continue;
        }
        let original = d.a[j].v;
        let mode = p.s("mode").to_string();
        let first = d.first.v;
        let spacing = (width - 305.0) / (count - 1) as f64;
        let tip = match j {
            0 => "Drag the first term up or down to change the first value",
            1 => "Drag the second term up or down to change the difference or ratio",
            _ => "Drag the last term left or right to change the number of terms",
        };
        out.push(interact::Handle::knob(
            (px(j), py(d.a[j].v)),
            tip,
            move |p, pt| {
                if j == count - 1 {
                    p.set_i(
                        "count",
                        (((pt.0 - 185.0) / spacing).round() as i64 + 1).clamp(3, 24),
                    );
                    return;
                }
                let v = (lo + (160.0 + height - pt.1) / height * (hi - lo)).clamp(lo, hi);
                let key = if j == 0 {
                    "first"
                } else if mode == "Geometric" {
                    "ratio"
                } else {
                    "difference"
                };
                let v = if j == 0 {
                    v
                } else if mode == "Geometric" {
                    if first == 0.0 {
                        return;
                    }
                    v / first
                } else {
                    v - first
                };
                if (pt.1 - py_value(original, lo, hi, height)).abs() < 1e-8 {
                    return;
                }
                let quantum = if key == "ratio" {
                    0.0001
                } else {
                    10f64.powi((hi - lo).abs().log10().floor() as i32 - 3)
                };
                p.set_s(key, &decimal_step(v, quantum));
            },
        ));
    }
    out
}
// Quantize to integer decimal units, then insert the decimal point without a floating product.
fn decimal_step(value: f64, quantum: f64) -> String {
    let places = (-quantum.log10().round() as i32).clamp(0, 12) as usize;
    let units = (value * 10f64.powi(places as i32)).round() as i128;
    if places == 0 {
        return units.to_string();
    }
    let digits = format!("{:0>width$}", units.unsigned_abs(), width = places + 1);
    let k = digits.len() - places;
    let frac = digits[k..].trim_end_matches('0');
    format!(
        "{}{}{}{}",
        if units < 0 { "-" } else { "" },
        &digits[..k],
        if frac.is_empty() { "" } else { "." },
        frac
    )
}
fn py_value(v: f64, lo: f64, hi: f64, height: f64) -> f64 {
    160.0 + height - (v.clamp(lo, hi) - lo) / (hi - lo) * height
}
pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID {
        return None;
    }
    let s = |k, v: &str| (k, Value::S(v.into()));
    Some(vec![
        Example {
            name: "Arithmetic",
            params: vec![s("mode", "Arithmetic")],
        },
        Example {
            name: "Geometric halving",
            params: vec![s("mode", "Geometric"), s("first", "8"), s("ratio", "1/2")],
        },
        Example {
            name: "Alternating halves",
            params: vec![s("mode", "Geometric"), s("ratio", "-1/2")],
        },
        Example {
            name: "Fibonacci",
            params: vec![
                s("mode", "Recursive"),
                s("first", "1"),
                s("second", "1"),
                ("from", Value::F(0.0)),
                ("to", Value::F(25.0)),
                ("step", Value::F(5.0)),
            ],
        },
        Example {
            name: "Reciprocals",
            params: vec![s("mode", "Explicit"), s("formula", "1/n")],
        },
        Example {
            name: "Geometric partial sums",
            params: vec![
                s("mode", "Geometric"),
                s("first", "1"),
                s("display", "Partial sums"),
                ("limit", Value::B(true)),
            ],
        },
    ])
}
#[cfg(test)]
#[path = "sequences_tests.rs"]
mod tests;
