//! Discrete sequences and finite series. Exact values are propagated, never inferred from floats.
use super::{
    svg::{self, Doc},
    *,
};
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
        ParamDef::bool("convergence", "Write convergence", false),
        ParamDef::bool("limit", "Draw limit line", false),
        ParamDef::bool("band", "Epsilon band", false),
        ParamDef::text("epsilon", "Epsilon", "1/4"),
        ParamDef::float("from", "Vertical axis: From", -100.0, 99.0, 1.0, -2.0),
        ParamDef::float("to", "Vertical axis: To", -99.0, 100.0, 1.0, 12.0),
        ParamDef::float("step", "Vertical axis: Count by", 0.25, 20.0, 0.25, 2.0),
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
impl N {
    fn int(n: i128) -> Self {
        Self {
            v: n as f64,
            q: Some((n, 1)),
            dec: false,
        }
    }
    fn op(self, b: Self, c: char) -> Self {
        let q = (|| {
            let (a, d) = self.q?;
            let (e, f) = b.q?;
            match c {
                '+' => rat(
                    a.checked_mul(f)?.checked_add(e.checked_mul(d)?)?,
                    d.checked_mul(f)?,
                ),
                '-' => rat(
                    a.checked_mul(f)?.checked_sub(e.checked_mul(d)?)?,
                    d.checked_mul(f)?,
                ),
                '*' => rat(a.checked_mul(e)?, d.checked_mul(f)?),
                '/' => rat(a.checked_mul(f)?, d.checked_mul(e)?),
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
        })();
        Self {
            v: match c {
                '+' => self.v + b.v,
                '-' => self.v - b.v,
                '*' => self.v * b.v,
                '/' => self.v / b.v,
                _ => self.v.powf(b.v),
            },
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
                return Some(a);
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
        let a = self.atom()?;
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
        let q = if dec {
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
    for digits in 0..=18u32 {
        let scale = 10i128.checked_pow(digits)?;
        if scale % b == 0 {
            let scaled = a.checked_mul(scale / b)?;
            let abs = scaled.checked_abs()?.to_string();
            let sign = if scaled < 0 { "-" } else { "" };
            if digits == 0 {
                return Some(format!("{sign}{abs}.0"));
            }
            let padded = format!("{:0>width$}", abs, width = digits as usize + 1);
            let k = padded.len() - digits as usize;
            let frac = padded[k..].trim_end_matches('0');
            return Some(format!(
                "{sign}{}.{}",
                &padded[..k],
                if frac.is_empty() { "0" } else { frac }
            ));
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
fn read(s: &str, n: N, a: N, b: N) -> Option<N> {
    if s.len() > 256 {
        return None;
    }
    let s = s
        .replace("a_(n-1)", "u")
        .replace("a_(n-2)", "v")
        .replace('−', "-");
    let mut p = P {
        s: s.as_bytes(),
        i: 0,
        n,
        a,
        b,
    };
    if let Some(v) = p.expr() {
        p.ws();
        if p.i == p.s.len() && v.v.is_finite() {
            return Some(v);
        }
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
        .filter(|v| v.is_finite())
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
    if s.chars()
        .all(|c| c.is_ascii_digit() || "eE.+-*/^() ".contains(c))
    {
        read(s, N::int(0), N::int(0), N::int(0))
    } else {
        super::math::eval_typed(s).map(|v| N {
            v,
            q: None,
            dec: true,
        })
    }
}
struct Data {
    a: Vec<N>,
    s: Vec<N>,
    first: N,
    k: N,
}
fn data(p: &Params) -> Result<Data, String> {
    let mode = p.s("mode");
    let first = if mode == "Explicit" {
        N::int(0)
    } else {
        number(p.s("first")).ok_or("First term is not a valid number")?
    };
    let k = match mode {
        "Arithmetic" => number(p.s("difference")),
        "Geometric" => number(p.s("ratio")),
        _ => Some(N::int(0)),
    }
    .ok_or("Enter a valid difference or ratio")?;
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
                } else if j == 1 {
                    number(p.s("second"))
                } else {
                    read(p.s("recurrence"), n, a[j as usize - 1], a[j as usize - 2])
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
            "Cannot evaluate term at index {}. Check the expression and its domain.",
            j + start
        ))?;
        if x.v.abs() > 1e9 {
            return Err("A term is too large to draw; use fewer terms or a smaller value.".into());
        }
        total = total.op(x, '+');
        if !total.v.is_finite() || total.v.abs() > 1e9 {
            return Err(
                "A partial sum is too large to display; use fewer terms or smaller values.".into(),
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
        "Explicit"
            if matches!(
                p.s("formula").replace(' ', "").as_str(),
                "1/n" | "1/(n+1)" | "(-1)^n/n"
            ) =>
        {
            (Some(zero), None, "converges".into())
        }
        _ => (None, None, "cannot determine from this formula".into()),
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
        "first" => p.s("mode") != "Explicit",
        "difference" => p.s("mode") == "Arithmetic",
        "ratio" => p.s("mode") == "Geometric",
        "formula" => p.s("mode") == "Explicit",
        "recurrence" | "second" => p.s("mode") == "Recursive",
        "title" => !p.b("blank_title") && p.b(SHOW_ANSWERS),
        "blank_title" => p.b(SHOW_ANSWERS),
        "limit" => p.b(SHOW_ANSWERS) && can_limit(p),
        "point_values" => p.s("layout") != "Number line",
        "display" => p.s("layout") != "Two aligned graphs",
        "grid" | "step" => p.s("layout") != "Number line",
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
        "missing" => !p.b(SHOW_ANSWERS) && p.s("task") == "Missing terms",
        "given" => !p.b(SHOW_ANSWERS) && p.s("task") == "Continue",
        _ => true,
    }
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
    if p.f("to") <= p.f("from") {
        w.push("Vertical To must be greater than From.".into())
    }
    if p.s("layout") != "Number line"
        && p.f("to") > p.f("from")
        && p.f("step") / (p.f("to") - p.f("from")) * 250.0
            < 14.0 * svg::label_factor(p.s("label_size")) * 1.5
    {
        w.push("Vertical tick labels do not fit at this Count by; increase Count by.".into());
    }
    match data(p) {
        Err(e) => w.push(e),
        Ok(d) => {
            if (0..if p.s("layout") == "Two aligned graphs" {
                2
            } else {
                1
            })
                .any(|g| point_labels(p, &d, g).1 > 0)
            {
                w.push(
                    "Some point labels need more room; use fewer terms or two aligned graphs."
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
                    "Some terms or sums lie beyond the vertical window; change From and To.".into(),
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
                    w.push("A limit lies outside the window; widen From and To.".into());
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
                                "The epsilon band extends beyond the window; widen From and To."
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
    for k in ["xlabel", "ylabel", "title"] {
        if relevant(k, p) && p.s(k).chars().count() > 48 {
            w.push(format!("Shorten {k} to at most 48 characters."))
        }
    }
    for k in ["xlabel", "ylabel"] {
        if axis_name(p, k).is_empty() && !p.s(k).is_empty() {
            w.push("Axis names must fit beside the arrow and must not clash with sequence symbols; shorten the name or choose smaller labels.".into());
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
    if circle::tw(t, 14.0 * svg::label_factor(p.s("label_size")), false) > 60.0
        || t.chars().count() > 6
        || matches!(t.trim(), "n" | "a" | "r" | "d" | "S" | "e" | "pi" | "π")
    {
        ""
    } else {
        t
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
                t.trim().to_string()
            }
        }
        _ => fmt_formula(t),
    }
}
fn fmt_formula(s: &str) -> String {
    let s = super::math::pretty(s)
        .replace("a_(n−1)", "aₙ₋₁")
        .replace("a_(n−2)", "aₙ₋₂")
        .replace("·√", "√")
        .replace("·π", "π")
        .replace('·', " × ");
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
}
fn point_labels(p: &Params, d: &Data, g: usize) -> (Vec<PointLabel>, usize) {
    if !p.b("point_values") || p.s("layout") == "Number line" {
        return (vec![], 0);
    }
    let fs = 14.0 * svg::label_factor(p.s("label_size"));
    let count = d.a.len();
    let width = canvas_width(p, count);
    let (left, right) = (85.0, width - 85.0);
    let (top, bottom) = (160.0 + g as f64 * 340.0, 410.0 + g as f64 * 340.0);
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
    for s in series {
        let values = if s == 0 { &d.a } else { &d.s };
        let mut prev = None;
        for (j, v) in values.iter().enumerate() {
            if hidden(p, j)
                || (!p.b(SHOW_ANSWERS) && p.s("task") != "Find formula" && s == 1)
                || v.v < lo
                || v.v > hi
            {
                prev = None;
                continue;
            }
            let pos = (
                120.0 + j as f64 * (width - 240.0) / (count - 1) as f64,
                bottom - (v.v - lo) / (hi - lo) * 250.0,
            );
            points.push((s, j, pos, v.text()));
            if p.b("join") {
                if let Some(a) = prev {
                    lines.push((a, pos));
                }
            }
            if p.s("marks") == "Spikes" {
                lines.push((
                    (
                        pos.0,
                        bottom - (0.0f64.clamp(lo, hi) - lo) / (hi - lo) * 250.0,
                    ),
                    pos,
                ));
            }
            prev = Some(pos);
        }
    }
    let mut placed: Vec<PointLabel> = vec![];
    let mut refused = 0;
    for (s, j, pos, text) in &points {
        let w = circle::tw(text, fs, false);
        let mut found = None;
        for (x, y, anchor) in [
            (pos.0, pos.1 - 13.0, "middle"),
            (pos.0, pos.1 + fs + 8.0, "middle"),
            (pos.0 + 11.0, pos.1 + fs * 0.3, "start"),
            (pos.0 - 11.0, pos.1 + fs * 0.3, "end"),
        ] {
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
            if r.x0 < left + 8.0 || r.x1 > right + 50.0 || r.y0 < top - 20.0 || r.y1 > bottom + 35.0
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
            if points
                .iter()
                .any(|(_, _, pt, _)| distance(*pt) + 0.01 < distance(*pos))
            {
                continue;
            }
            if lo <= 0.0
                && hi >= 0.0
                && circle::seg_hits(
                    (left, bottom - (0.0 - lo) / (hi - lo) * 250.0),
                    (right, bottom - (0.0 - lo) / (hi - lo) * 250.0),
                    &r,
                )
            {
                continue;
            }
            if can_limit(p) && p.b("limit") && p.b(SHOW_ANSWERS) {
                let (a, b, _) = limits(p, d);
                let limit = if *s == 0 { a } else { b };
                if limit.is_some_and(|l| {
                    circle::seg_hits(
                        (left, bottom - (l.v - lo) / (hi - lo) * 250.0),
                        (right, bottom - (l.v - lo) / (hi - lo) * 250.0),
                        &r,
                    )
                }) {
                    continue;
                }
            }
            if placed.iter().any(|a| overlaps(&r, &a.rect))
                || points.iter().any(|(_, _, pt, _)| {
                    overlaps(
                        &r,
                        &circle::Rect {
                            x0: pt.0 - 5.0,
                            x1: pt.0 + 5.0,
                            y0: pt.1 - 5.0,
                            y1: pt.1 + 5.0,
                        },
                    )
                })
                || lines.iter().any(|(a, b)| circle::seg_hits(*a, *b, &r))
            {
                continue;
            }
            found = Some(PointLabel {
                series: *s,
                j: *j,
                x,
                y,
                anchor,
                text: text.clone(),
                rect: r,
            });
            break;
        }
        if let Some(l) = found {
            placed.push(l)
        } else {
            refused += 1
        }
    }
    (placed, refused)
}
fn canvas_width(p: &Params, count: usize) -> f64 {
    let fs = 14.0 * svg::label_factor(p.s("label_size"));
    let base =
        (count as f64 * 100.0 + 160.0).max(900.0) * svg::label_factor(p.s("label_size")).max(1.0);
    let expr = match p.s("mode") {
        "Explicit" => p.s("formula"),
        "Recursive" => p.s("recurrence"),
        _ => "",
    };
    base.max(expr.chars().count() as f64 * fs * 0.85 + 240.0)
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
    let left = 85.0;
    let right = width - 85.0;
    let px = |j: usize| left + 35.0 + j as f64 * (right - left - 70.0) / (count - 1) as f64;
    let (lo, hi) = (p.f("from"), p.f("to"));
    if hi <= lo {
        return Doc::new(650.0, 100.0).finish();
    }
    let split = p.s("layout") == "Two aligned graphs";
    let numberline = p.s("layout") == "Number line";
    let plot_bottom = if split { 820.0 } else { 480.0 };
    let mut rows = vec![format!("Task: {}", p.s("task"))];
    let (lim, slim, state) = limits(p, &d);
    let idx = subscript(start);
    let formula = match p.s("mode") {
        "Arithmetic" => format!(
            "aₙ = ({}) + (n − {start}) × ({})",
            coefficient(p, "first"),
            coefficient(p, "difference")
        ),
        "Geometric" => format!(
            "aₙ = ({}) × ({})^(n − {start})",
            coefficient(p, "first"),
            coefficient(p, "ratio")
        ),
        "Explicit" => format!("aₙ = {}", fmt_formula(p.s("formula"))),
        _ => "Explicit formula: cannot determine".into(),
    };
    if p.b("explicit") {
        rows.push(if ans {
            formula.clone()
        } else {
            "Formula: ________________________".into()
        })
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
                "Recursive" => format!(
                    "aₙ = {}; initial terms {}, {}",
                    fmt_formula(p.s("recurrence")),
                    d.a[0].text(),
                    d.a[1].text()
                ),
                _ => "Recurrence: cannot determine".into(),
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
                    if d.k.q.is_some_and(|(a, b)| a == b) {
                        format!("Sum to infinity {}", l.eq())
                    } else {
                        format!(
                            "S∞ = ({})/(1 − ({})) {}",
                            coefficient(p, "first"),
                            coefficient(p, "ratio"),
                            l.eq()
                        )
                    }
                })
                .unwrap_or(if state.contains("cannot determine") {
                    "Sum to infinity: cannot determine".into()
                } else {
                    "Series diverges".into()
                })
            } else {
                "Sum to infinity: __________".into()
            })
        }
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
            format!(
                "{}: {state}; limit {}",
                if display(p) == "Partial sums" {
                    "Series"
                } else {
                    "Sequence"
                },
                (if display(p) == "Partial sums" {
                    slim
                } else {
                    lim
                })
                .map(N::eq)
                .unwrap_or("cannot determine a finite limit".into())
            )
        } else {
            "Convergence and limit: ________________________".into()
        })
    }
    if p.b("terms") || p.b("sums") || p.s("task") == "Missing terms" {
        for j in 0..count {
            let hide = hidden(p, j);
            let term = if d.a[j].v < lo || d.a[j].v > hi {
                "outside window".into()
            } else if hide {
                "= _____".into()
            } else {
                d.a[j].eq()
            };
            let sum = if d.s[j].v < lo || d.s[j].v > hi {
                "outside window".into()
            } else if !ans && p.s("task") != "Find formula" {
                "= _____".into()
            } else {
                d.s[j].eq()
            };
            let mut row = String::new();
            if p.b("terms") || p.s("task") == "Missing terms" {
                row.push_str(&format!("a{} {term}", subscript(j + start)))
            }
            if p.b("sums") {
                row.push_str(&format!("     S{} {sum}", subscript(j + start)))
            }
            rows.push(row)
        }
    }
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
        let top = 160.0 + g as f64 * 340.0;
        let bottom = top + 250.0;
        let py = |v: f64| bottom - (v - lo) / (hi - lo) * 250.0;
        let nlx = |v: f64| left + (v - lo) / (hi - lo) * (right - left);
        let valid = |v: f64| v >= lo && v <= hi;
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
                for k in 0..=800 {
                    let v = lo + k as f64 * p.f("step").max(0.25);
                    if v > hi {
                        break;
                    }
                    doc.line(left, py(v), right, py(v), "#cccccc", sw);
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
            if lo <= 0.0 && hi >= 0.0 {
                doc.line(left, py(0.0), right, py(0.0), "#777777", 1.0);
            }
            // Numbers have a dedicated gutter and row, away from every data label.
            for j in 0..count {
                doc.text(px(j), bottom + 65.0, fs, "middle", &(j + start).to_string());
            }
            for k in 0..=800 {
                let v = lo + k as f64 * p.f("step").max(0.25);
                if v > hi {
                    break;
                }
                if p.f("step") / (hi - lo) * 250.0 >= fs * 1.5 {
                    doc.text(left - 10.0, py(v) + 4.0, fs, "end", &format!("{v}"));
                }
            }
            doc.text(right + 20.0, bottom + 45.0, fs, "start", p.s("xlabel"));
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
            doc.text(left, bottom + 65.0, fs, "start", &format!("{lo}"));
            doc.text(right, bottom + 65.0, fs, "end", &format!("{hi}"));
            doc.text(
                right + 20.0,
                bottom + 40.0,
                fs,
                "start",
                axis_name(p, "ylabel"),
            );
            doc.text(left, top - 30.0, fs, "start", p.s("xlabel"));
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
                                nlx(l.v),
                                bottom + 75.0,
                                fs,
                                "middle",
                                &format!("Limit {}", l.eq()),
                            );
                        } else {
                            if p.b("band") {
                                if let Some(e) = number(p.s("epsilon")).filter(|e| e.v > 0.0) {
                                    let a = (l.v - e.v).max(lo);
                                    let b = (l.v + e.v).min(hi);
                                    doc.rect(
                                        left,
                                        py(b),
                                        right - left,
                                        py(a) - py(b),
                                        "#eeeeee",
                                        "none",
                                        0.0,
                                    );
                                }
                            }
                            doc.line(left, py(l.v), right, py(l.v), "#555555", 1.5);
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
                        }
                    }
                }
            }
            let mut prev = None;
            for (j, v) in vals.iter().enumerate() {
                if hidden(p, j)
                    || (!ans && p.s("task") != "Find formula" && series == 1)
                    || !valid(v.v)
                {
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
                        doc.line(a, b, x, y, color, 0.8);
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
                    doc.rect(x - 3.5, y - 3.5, 7.0, 7.0, "#ffffff", color, 1.5);
                }
                doc.raw("</g>");
                prev = Some((x, y));
            }
            doc.raw("</g>");
        }
        for l in point_labels(p, &d, g).0 {
            doc.raw(&format!(r##"<text data-point-label="{}:{}" x="{}" y="{}" font-size="{fs}" text-anchor="{}" fill="{}" stroke="#ffffff" stroke-width="3" paint-order="stroke">{}</text>"##,l.series,l.j,l.x,l.y,l.anchor,colors[l.series],svg::esc(&l.text)));
        }
    }
    for (i, row) in rows.iter().enumerate() {
        // powers in generated equations are typeset with tspan; other expressions use the shared pretty reader.
        let escaped = svg::esc(row);
        let mut rest = escaped.as_str();
        let mut text = String::new();
        while let Some(k) = rest.find("^(") {
            text.push_str(&rest[..k]);
            let Some(end) = rest[k + 2..].find(')') else {
                break;
            };
            text.push_str(&format!(
                "<tspan baseline-shift=\"super\" font-size=\"70%\">{}</tspan>",
                &rest[k + 2..k + 2 + end]
            ));
            rest = &rest[k + 3 + end..];
        }
        text.push_str(rest);
        doc.raw(&format!(
            r##"<text x="{left}" y="{}" font-size="{fs}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="#111111">{text}</text>"##,
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
    let px = |j: usize| 120.0 + j as f64 * (width - 240.0) / (count - 1) as f64;
    let (lo, hi) = (p.f("from"), p.f("to"));
    if hi <= lo {
        return vec![];
    }
    let py = |v: f64| 410.0 - (v - lo) / (hi - lo) * 250.0;
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
        let spacing = (width - 240.0) / (count - 1) as f64;
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
                        (((pt.0 - 120.0) / spacing).round() as i64 + 1).clamp(3, 24),
                    );
                    return;
                }
                let v = (lo + (410.0 - pt.1) / 250.0 * (hi - lo)).clamp(lo, hi);
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
                if (pt.1 - py_value(original, lo, hi)).abs() < 1e-8 {
                    return;
                }
                p.set_s(key, &format!("{}", (v * 1000.0).round() / 1000.0));
            },
        ));
    }
    out
}
fn py_value(v: f64, lo: f64, hi: f64) -> f64 {
    410.0 - (v - lo) / (hi - lo) * 250.0
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
