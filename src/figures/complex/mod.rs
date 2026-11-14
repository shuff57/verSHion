//! Argand diagrams. Exact rational arithmetic is separate from numerical geometry.
use super::interact::Handle;
use super::svg::{self, Doc};
use super::*;
use std::f64::consts::PI;
pub const ID: &str = "complex-plane";
pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Complex plane",
        blurb: "Complex numbers and constructions on an Argand diagram.",
        params,
        render,
    }]
}
pub fn params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::text("zr", "z: real part", "3"),
        ParamDef::text("zi", "z: imaginary part", "2"),
        ParamDef::text("wr", "w: real part", "-1"),
        ParamDef::text("wi", "w: imaginary part", "1"),
        ParamDef::choice(
            "input",
            "z input",
            &["Cartesian", "Polar cis", "Polar exponential"],
            "Cartesian",
        ),
        ParamDef::text("radius", "Polar modulus", "2"),
        ParamDef::text("theta", "Polar angle", "pi/3"),
        ParamDef::choice("unit", "Angles", &["Radians", "Degrees"], "Radians"),
        ParamDef::long(
            "extra",
            "More numbers: name, real, imaginary; one per line",
            "",
        ),
        ParamDef::choice(
            "operation",
            "Construction",
            &[
                "None",
                "Conjugate",
                "Sum",
                "Difference",
                "Product",
                "Quotient",
                "Roots",
                "Powers",
            ],
            "Sum",
        ),
        ParamDef::int("n", "n", 2, 12, 3),
        ParamDef::bool("polygon", "Join roots", true),
        ParamDef::choice(
            "marks",
            "Draw numbers as",
            &["Vectors", "Points"],
            "Vectors",
        ),
        ParamDef::choice(
            "labels",
            "Point labels",
            &["Letter", "Cartesian", "Polar"],
            "Letter",
        ),
        ParamDef::bool("modulus", "Show modulus", true),
        ParamDef::bool("argument", "Show principal argument", true),
        ParamDef::bool("polar", "Write polar form", false),
        ParamDef::choice(
            "polar_style",
            "Polar notation",
            &["cis", "Trigonometric", "Exponential"],
            "cis",
        ),
        ParamDef::bool("circle", "Modulus circle", false),
        ParamDef::bool("components", "Components to axes", false),
        ParamDef::choice("accuracy", "Values", &["Exact", "Rounded"], "Exact"),
        ParamDef::int("dec", "Decimal places", 0, 6, 3),
        ParamDef::choice(
            "locus",
            "Locus",
            &["None", "Circle", "Disc", "Argument sector"],
            "None",
        ),
        ParamDef::text("cr", "Centre: real", "0"),
        ParamDef::text("ci", "Centre: imaginary", "2"),
        ParamDef::text("lr", "Locus radius", "3"),
        ParamDef::text("lo", "Argument from", "0"),
        ParamDef::text("hi", "Argument to", "pi/3"),
        ParamDef::text("xlabel", "Real axis name", "Re"),
        ParamDef::text("ylabel", "Imaginary axis name", "Im"),
        ParamDef::text("title", "Title", ""),
        ParamDef::choice(
            "title_mode",
            "Title style",
            &["Text", "Blank line", "None"],
            "Text",
        ),
        ParamDef::choice(
            "grid",
            "Gridlines",
            &["Light", "Standard", "Bold", "None"],
            "Standard",
        ),
        ParamDef::bool("numbers", "Number axes", true),
        ParamDef::choice(
            "question",
            "Student task",
            &["Coordinates", "Modulus and argument", "Construction"],
            "Construction",
        ),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
        labels::param(),
    ];
    for (k, l, a, b, d) in [
        ("xmin", "Real axis from", -100., 100., -6.),
        ("xmax", "Real axis to", -100., 100., 6.),
        ("ymin", "Imaginary axis from", -100., 100., -6.),
        ("ymax", "Imaginary axis to", -100., 100., 6.),
        ("xstep", "Real axis count by", 0.1, 50., 1.),
        ("ystep", "Imaginary axis count by", 0.1, 50., 1.),
    ] {
        v.push(ParamDef::float(k, l, a, b, 0.1, d));
    }
    v
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Q(i64, i64);
fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a.abs().max(1)
    } else {
        gcd(b, a % b)
    }
}
impl Q {
    fn new(a: i64, b: i64) -> Self {
        let g = gcd(a, b);
        Q(a / g * b.signum(), b.abs() / g)
    }
    fn add(self, b: Self) -> Self {
        Q::new(self.0 * b.1 + b.0 * self.1, self.1 * b.1)
    }
    fn mul(self, b: Self) -> Self {
        Q::new(self.0 * b.0, self.1 * b.1)
    }
    fn neg(self) -> Self {
        Q(-self.0, self.1)
    }
    fn div(self, b: Self) -> Self {
        Q::new(self.0 * b.1, self.1 * b.0)
    }
    fn val(self) -> f64 {
        self.0 as f64 / self.1 as f64
    }
    fn text(self) -> String {
        if self.1 == 1 {
            self.0.to_string()
        } else {
            format!("{}/{}", self.0, self.1)
        }
        .replace('-', "−")
    }
}
#[derive(Clone, Debug)]
struct N {
    v: f64,
    s: String,
    q: Option<Q>,
    exact: bool,
}
fn rounded(v: f64, d: i64) -> String {
    if v != 0. && v.abs() < 10f64.powi(-(d as i32)) {
        scientific(&format!("{v:.3e}"))
    } else {
        math::num_str(v, d as usize).replace('-', "−")
    }
}
fn read(s: &str) -> Option<N> {
    let s = s.trim();
    if s.len() > 100 {
        return None;
    }
    let t = s.replace('−', "-");
    let v = if let Ok(v) = t.parse::<f64>() {
        v
    } else {
        math::eval_typed(&t)?
    };
    if (v == 0.
        && t.contains(['e', 'E'])
        && t.split(['e', 'E'])
            .next()
            .and_then(|n| n.parse::<f64>().ok())
            .is_some_and(|n| n != 0.))
        || !v.is_finite()
        || v.abs() > 1e6
    {
        return None;
    }
    let q = if let Ok(a) = t.parse::<i64>() {
        (a.unsigned_abs() <= 10000).then_some(Q(a, 1))
    } else if let Some((a, b)) = t.split_once('/') {
        match (a.parse::<i64>(), b.parse::<i64>()) {
            (Ok(a), Ok(b)) if a.unsigned_abs() <= 10000 && b != 0 && b.unsigned_abs() <= 10000 => {
                Some(Q::new(a, b))
            }
            _ => None,
        }
    } else {
        None
    };
    let simple = t.parse::<f64>().is_ok();
    let root = radical(&t);
    let (s, exact) = if simple {
        (
            if t.contains(['e', 'E']) {
                scientific(&t)
            } else {
                t.replace('-', "−")
            },
            true,
        )
    } else if let Some(q) = q {
        (q.text(), true)
    } else if let Some((sign, n, d)) = root {
        (
            format!(
                "{}√{}{}",
                if sign < 0 { "−" } else { "" },
                n,
                if d == 1 {
                    String::new()
                } else {
                    format!("/{d}")
                }
            ),
            true,
        )
    } else {
        (rounded(v, 6), false)
    };
    Some(N { v, s, q, exact })
}
fn from_q(q: Q) -> N {
    N {
        v: q.val(),
        s: q.text(),
        q: Some(q),
        exact: true,
    }
}
fn approx(v: f64) -> N {
    N {
        v,
        s: rounded(v, 6),
        q: None,
        exact: false,
    }
}
#[derive(Clone, Debug)]
struct Z {
    a: N,
    b: N,
    name: String,
}
impl Z {
    fn xy(&self) -> (f64, f64) {
        (self.a.v, self.b.v)
    }
    fn r(&self) -> f64 {
        self.a.v.hypot(self.b.v)
    }
    fn angle(&self) -> f64 {
        let a = self.b.v.atan2(self.a.v);
        if a <= -PI { PI } else { a }
    }
    fn cart(&self, p: &Params) -> String {
        let f = |n: &N| {
            if n.exact && p.s("accuracy") == "Exact" {
                n.s.clone()
            } else {
                rounded(n.v, p.i("dec"))
            }
        };
        let b = f(&self.b);
        let b = b.trim_start_matches('−');
        let b = if b.contains('/') {
            format!("({b})")
        } else {
            b.to_string()
        };
        format!(
            "{} {} {}i",
            f(&self.a),
            if self.b.v < 0. { "−" } else { "+" },
            b
        )
    }
    fn exact(&self, p: &Params) -> bool {
        p.s("accuracy") == "Exact" && self.a.exact && self.b.exact
    }
}
fn polar(z: &Z, p: &Params) -> String {
    let r = modtext(z, p);
    let a = angtext(z, p);
    let approx = r.starts_with('≈') || a.starts_with('≈');
    let r = r.trim_start_matches("≈ ");
    let a = a.trim_start_matches("≈ ");
    let form = match p.s("polar_style") {
        "Exponential" => format!("{r}e⟦i({a})⟧"),
        "Trigonometric" => format!("{r}(cos({a}) + i sin({a}))"),
        _ => format!("{r} cis({a})"),
    };
    format!("{} {form}", if approx { "≈" } else { "=" })
}
fn modtext(z: &Z, p: &Params) -> String {
    if z.name == "z" && p.s("input") != "Cartesian" && p.s("accuracy") == "Exact" {
        if let Some(r) = read(p.s("radius")) {
            if r.exact {
                return r.s;
            }
        }
    }
    if p.s("accuracy") == "Exact" {
        if let (Some(a), Some(b)) = (square_exact(&z.a), square_exact(&z.b)) {
            let Some(q) = safe_sum(a, b) else {
                return format!("≈ {}", rounded(z.r(), p.i("dec")));
            };
            let n = (q.0 as f64).sqrt() as i64;
            let d = (q.1 as f64).sqrt() as i64;
            if (n as i128) * (n as i128) == q.0 as i128 && (d as i128) * (d as i128) == q.1 as i128
            {
                return Q::new(n, d).text();
            }
            return if q.1 == 1 {
                format!("√{}", q.0)
            } else {
                if d * d == q.1 {
                    format!("√{}/{}", q.0, d)
                } else {
                    format!("√({})", q.text())
                }
            };
        }
    }
    format!("≈ {}", rounded(z.r(), p.i("dec")))
}
fn angtext(z: &Z, p: &Params) -> String {
    if z.r() == 0. {
        return "undefined".into();
    }
    let cart_exact = match (z.a.q, z.b.q) {
        (Some(a), Some(b)) => {
            let (x, y) = (a.0, b.0);
            if y == 0 {
                Some(if x < 0 { Q(1, 1) } else { Q(0, 1) })
            } else if x == 0 {
                Some(Q(y.signum(), 2))
            } else if a.0.unsigned_abs() == b.0.unsigned_abs() && a.1 == b.1 {
                Some(Q(if x > 0 { y.signum() } else { 3 * y.signum() }, 4))
            } else {
                None
            }
        }
        _ => None,
    };
    let exact = if p.s("operation") == "Roots" && z.name.starts_with('u') {
        z.name[1..]
            .parse::<i64>()
            .ok()
            .and_then(|k| root_angle(p, k - 1))
    } else if z.name == "z" && p.s("input") != "Cartesian" {
        angle_fraction(p.s("theta"), p.s("unit"))
    } else {
        cart_exact.or_else(|| {
            let a = square_exact(&z.a)?;
            let b = square_exact(&z.b)?;
            if a.0 == 0 || b.0 == 0 {
                return None;
            }
            let first = if b == a.mul(Q(3, 1)) {
                Q(1, 3)
            } else if a == b.mul(Q(3, 1)) {
                Q(1, 6)
            } else if a == b {
                Q(1, 4)
            } else {
                return None;
            };
            let upper = if z.a.v < 0. {
                Q(1, 1).add(first.neg())
            } else {
                first
            };
            Some(if z.b.v < 0. { upper.neg() } else { upper })
        })
    };
    if p.s("accuracy") == "Exact" {
        if let Some(q) = exact {
            return if p.s("unit") == "Degrees" {
                format!("{}°", q.mul(Q(180, 1)).text())
            } else {
                match (q.0, q.1) {
                    (0, _) => "0".into(),
                    (1, 1) => "π".into(),
                    (1, d) => format!("π/{d}"),
                    (-1, d) => format!("−π/{d}"),
                    (a, d) => format!("{a}π/{d}").replace('-', "−"),
                }
            };
        }
    }
    format!(
        "≈ {}{}",
        rounded(
            if p.s("unit") == "Degrees" {
                z.angle().to_degrees()
            } else {
                z.angle()
            },
            p.i("dec")
        ),
        if p.s("unit") == "Degrees" {
            "°"
        } else {
            " rad"
        }
    )
}
fn op(z: &Z, w: &Z, mode: &str) -> Option<Z> {
    if mode == "Conjugate" {
        let mut t = z.clone();
        t.name = "z̄".into();
        t.b.v = -t.b.v;
        t.b.q = t.b.q.map(Q::neg);
        t.b.s = if let Some(s) = t.b.s.strip_prefix('−') {
            s.to_string()
        } else if t.b.v == 0. {
            t.b.s.clone()
        } else {
            format!("−{}", t.b.s)
        };
        return Some(t);
    }
    let (a, b, c, d) = (z.a.v, z.b.v, w.a.v, w.b.v);
    let xy = match mode {
        "Conjugate" => (a, -b),
        "Sum" => (a + c, b + d),
        "Difference" => (a - c, b - d),
        "Product" => (a * c - b * d, a * d + b * c),
        "Quotient" => {
            let scale = c.abs().max(d.abs());
            if scale == 0. {
                return None;
            }
            let (c, d) = (c / scale, d / scale);
            let den = c * c + d * d;
            (
                ((a / scale) * c + (b / scale) * d) / den,
                ((b / scale) * c - (a / scale) * d) / den,
            )
        }
        _ => return None,
    };
    let qs = match (z.a.q, z.b.q, w.a.q, w.b.q) {
        (Some(a), Some(b), Some(c), Some(d))
            if [a, b, c, d].iter().all(|q| q.0.abs() < 100 && q.1 < 100) =>
        {
            Some(match mode {
                "Conjugate" => (a, b.neg()),
                "Sum" => (a.add(c), b.add(d)),
                "Difference" => (a.add(c.neg()), b.add(d.neg())),
                "Product" => (a.mul(c).add(b.mul(d).neg()), a.mul(d).add(b.mul(c))),
                "Quotient" => {
                    let den = c.mul(c).add(d.mul(d));
                    (
                        a.mul(c).add(b.mul(d)).div(den),
                        b.mul(c).add(a.mul(d).neg()).div(den),
                    )
                }
                _ => return None,
            })
        }
        _ => None,
    };
    let (a, b) = qs
        .map(|(a, b)| (from_q(a), from_q(b)))
        .unwrap_or((approx(xy.0), approx(xy.1)));
    Some(Z {
        a,
        b,
        name: match mode {
            "Conjugate" => "z̄",
            "Sum" => "z + w",
            "Difference" => "z − w",
            "Product" => "zw",
            _ => "z/w",
        }
        .into(),
    })
}
#[derive(Clone, Copy)]
struct Win {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    u: f64,
}
impl Win {
    fn new(p: &Params) -> Option<Self> {
        let (a, b, c, d) = (p.f("xmin"), p.f("xmax"), p.f("ymin"), p.f("ymax"));
        (a < b && c < d && a <= 0. && b >= 0. && c <= 0. && d >= 0.).then(|| Self {
            a,
            b,
            c,
            d,
            u: (480. / (b - a)).min(480. / (d - c)),
        })
    }
    fn pt(self, x: f64, y: f64) -> (f64, f64) {
        (70. + (x - self.a) * self.u, 70. + (self.d - y) * self.u)
    }
    fn inside(self, x: f64, y: f64) -> bool {
        x >= self.a && x <= self.b && y >= self.c && y <= self.d
    }
}
fn numbers(p: &Params, notes: &mut Vec<String>) -> Vec<Z> {
    let mut out = vec![];
    for (name, ak, bk) in [("z", "zr", "zi"), ("w", "wr", "wi")] {
        let pair = if name == "z" && p.s("input") != "Cartesian" {
            match (read(p.s("radius")), read(p.s("theta"))) {
                (Some(r), Some(t)) if r.v >= 0. => {
                    let t = if p.s("unit") == "Degrees" {
                        t.v.to_radians()
                    } else {
                        t.v
                    };
                    let (x, y) = if let Some(q) =
                        angle_fraction(p.s("theta"), p.s("unit")).filter(|q| (2 * q.0) % q.1 == 0)
                    {
                        match ((2 * q.0) / q.1).rem_euclid(4) {
                            0 => (r.v, 0.),
                            1 => (0., r.v),
                            2 => (-r.v, 0.),
                            _ => (0., -r.v),
                        }
                    } else {
                        (r.v * t.cos(), r.v * t.sin())
                    };
                    Some((approx(x), approx(y)))
                }
                _ => None,
            }
        } else {
            read(p.s(ak)).zip(read(p.s(bk)))
        };
        if let Some((a, b)) = pair {
            out.push(Z {
                a,
                b,
                name: name.into(),
            })
        } else {
            notes.push(format!("Invalid {name}: enter finite numbers (magnitude at most 1000000); polar modulus must be nonnegative."))
        }
    }
    for row in p
        .s("extra")
        .lines()
        .filter(|x| !x.trim().is_empty())
        .take(6)
    {
        let v: Vec<_> = row.split(',').map(str::trim).collect();
        if v.len() != 3
            || v[0].len() > 8
            || !v[0].chars().all(|x| x.is_ascii_alphabetic())
            || ["z", "w", "i", "e", "pi", "r", "theta"].contains(&v[0].to_lowercase().as_str())
            || out.iter().any(|z| z.name == v[0])
        {
            notes.push("Extra rows need unique names (up to eight letters), real, imaginary; reserved symbols are not names.".into());
            continue;
        }
        if let Some((a, b)) = read(v[1]).zip(read(v[2])) {
            out.push(Z {
                a,
                b,
                name: v[0].into(),
            })
        } else {
            notes.push(format!("Invalid number in row {row}"))
        }
    }
    if p.s("extra").lines().count() > 6 {
        notes.push("Use at most six extra numbers.".into())
    }
    out
}
fn all(p: &Params, notes: &mut Vec<String>) -> (Vec<Z>, usize) {
    let mut zs = numbers(p, notes);
    let given = zs.len();
    if zs.len() < 2 || zs[0].name != "z" || zs[1].name != "w" {
        return (zs, given);
    }
    let (z, w) = (zs[0].clone(), zs[1].clone());
    let m = p.s("operation");
    if matches!(m, "Product" | "Quotient") && z.r() > 0. && w.r() > 0. {
        let log = if m == "Product" {
            z.r().ln() + w.r().ln()
        } else {
            z.r().ln() - w.r().ln()
        };
        if log < f64::MIN_POSITIVE.ln() {
            notes
                .push("The result is too small to calculate reliably; rescale the numbers.".into());
            return (zs, given);
        }
    }
    if m == "Roots" {
        if z.r() == 0. {
            zs.push(Z {
                a: from_q(Q(0, 1)),
                b: from_q(Q(0, 1)),
                name: "root".into(),
            })
        } else {
            for k in 0..p.i("n") {
                let r = z.r().powf(1. / p.i("n") as f64);
                let a = (z.angle() + 2. * PI * k as f64) / p.i("n") as f64;
                let (x, y) = if let Some(q) = root_angle(p, k).filter(|q| (2 * q.0) % q.1 == 0) {
                    match ((2 * q.0) / q.1).rem_euclid(4) {
                        0 => (r, 0.),
                        1 => (0., r),
                        2 => (-r, 0.),
                        _ => (0., -r),
                    }
                } else {
                    (r * a.cos(), r * a.sin())
                };
                zs.push(Z {
                    a: approx(x),
                    b: approx(y),
                    name: format!("u{}", k + 1),
                })
            }
        }
    } else if m == "Powers" {
        let mut t = z.clone();
        for k in 2..=p.i("n") {
            if t.r() > 0. && z.r() > 0. && t.r().ln() + z.r().ln() < f64::MIN_POSITIVE.ln() {
                notes.push(format!(
                    "Power {k} is too small to calculate reliably; rescale z."
                ));
                break;
            }
            t = op(&t, &z, "Product").unwrap();
            t.name = format!("z{}", sup(k));
            zs.push(t.clone())
        }
    } else if m != "None" {
        if let Some(t) = op(&z, &w, m) {
            zs.push(t)
        } else {
            notes.push("Division by zero: z/w is undefined.".into())
        }
    }
    (zs, given)
}
fn sup(n: i64) -> String {
    n.to_string()
        .chars()
        .map(|c| {
            "⁰¹²³⁴⁵⁶⁷⁸⁹"
                .chars()
                .nth(c.to_digit(10).unwrap() as usize)
                .unwrap()
        })
        .collect()
}
pub fn relevant(k: &str, p: &Params) -> bool {
    let blanks =
        !p.b(SHOW_ANSWERS) && matches!(p.s("question"), "Coordinates" | "Modulus and argument");
    if blanks && matches!(k, "polar" | "circle" | "accuracy" | "dec" | "polar_style") {
        return false;
    }
    if (blanks && k == "labels")
        || (!p.b(SHOW_ANSWERS) && p.s("question") == "Coordinates" && k == "components")
    {
        return false;
    }
    match k {
        "zr" | "zi" => p.s("input") == "Cartesian",
        "radius" | "theta" => p.s("input") != "Cartesian",
        "n" => matches!(p.s("operation"), "Roots" | "Powers"),
        "polygon" => p.s("operation") == "Roots",
        "cr" | "ci" | "lr" => matches!(p.s("locus"), "Circle" | "Disc"),
        "lo" | "hi" => p.s("locus") == "Argument sector",
        "title" => p.s("title_mode") == "Text",
        "polar_style" => p.b("polar") || p.s("labels") == "Polar",
        "question" => !p.b(SHOW_ANSWERS),
        _ => true,
    }
}
pub fn warnings(p: &Params) -> Vec<String> {
    build(p).1
}
pub fn render(p: &Params) -> String {
    labels::rendering(p, || build(p).0)
}
fn build(raw: &Params) -> (String, Vec<String>) {
    let mut p = raw.clone();
    p.normalize(&params(), false);
    let p = &p;
    let mut notes = vec![];
    let (zs, given) = all(p, &mut notes);
    let key = p.b(SHOW_ANSWERS);
    let student = !key;
    let coord = student && p.s("question") == "Coordinates";
    let blank = student && p.s("question") == "Modulus and argument";
    let construct = student && p.s("question") == "Construction";
    let visible = if construct { given } else { zs.len() };
    let fs = 14. * svg::label_factor(p.s("label_size"));
    let row = fs * 1.7;
    let mut doc = Doc::new(1160., (150. + visible as f64 * row * 4.).max(640.));
    doc.describe("Complex plane", "Complex numbers on a plane.");
    let text = |x: f64, y: f64, s: &str| {
        let mut d = Doc::new(1., 1.);
        d.text(x, y, fs, "start", s);
        d.cut(0)
            .replace("⟦", r#"<tspan baseline-shift="sup" font-size="70%">"#)
            .replace("⟧", "</tspan>")
    };
    if p.s("title_mode") == "Blank line" {
        doc.line(70., 32., 550., 32., "#111111", 1.);
    } else if p.s("title_mode") == "Text" {
        if super::circle::tw(p.s("title"), fs, false) > 1020. {
            notes.push("Shorten the title so it fits on the page.".into());
        } else {
            doc.raw(&text(
                70.,
                if p.s("title").is_empty() { 32. } else { 26. },
                p.s("title"),
            ));
        }
    }
    let Some(g) = Win::new(p) else {
        notes.push("Set increasing axis ranges containing zero.".into());
        doc.line(70., 70., 550., 550., "#777777", 1.);
        return (doc.finish(), notes);
    };
    let print = p.s("style") == "Print";
    let color = |i: usize| {
        if print {
            "#222222"
        } else {
            ["#1f5fbf", "#b03a2e", "#287d3c", "#7d3c98"][i % 4]
        }
    };
    let origin = g.pt(0., 0.);
    let end = g.pt(g.b, g.c);
    // Every grid loop has a fixed bound, including hostile count-by input.
    for (axis, lo, hi, step) in [(0, g.a, g.b, p.f("xstep")), (1, g.c, g.d, p.f("ystep"))] {
        let count = ((hi - lo) / step).ceil() as usize;
        if count > 40 {
            notes.push("Count by is too small: use at most 40 intervals on each axis.".into());
            continue;
        }
        for j in 0..=count {
            let t = (lo / step).ceil() * step + j as f64 * step;
            if t > hi + 1e-10 {
                break;
            }
            let (a, b) = if axis == 0 {
                (g.pt(t, g.c), g.pt(t, g.d))
            } else {
                (g.pt(g.a, t), g.pt(g.b, t))
            };
            if p.s("grid") != "None" {
                doc.line(
                    a.0,
                    a.1,
                    b.0,
                    b.1,
                    match p.s("grid") {
                        "Light" => "#eeeeee",
                        "Bold" => "#999999",
                        _ => "#dddddd",
                    },
                    1.,
                );
            }
            if p.b("numbers") {
                let s = rounded(t, 2);
                if axis == 0 {
                    doc.text(a.0, end.1 + 22., 12., "middle", &s);
                } else {
                    doc.text(a.0 - 12., a.1 + 4., 12., "end", &s);
                }
            }
        }
    }
    doc.line(70., origin.1, end.0, origin.1, "#111111", 1.5);
    doc.line(origin.0, 70., origin.0, end.1, "#111111", 1.5);
    if super::circle::tw(p.s("xlabel"), fs, false) <= 608. - end.0 - 12. {
        doc.raw(&text(end.0 + 12., origin.1 - 8., p.s("xlabel")));
    } else {
        notes.push("Shorten the real axis name to fit beside the plot.".into());
    }
    if super::circle::tw(p.s("ylabel"), fs, false) <= 598. - origin.0 - 10. {
        doc.raw(&text(origin.0 + 10., 56., p.s("ylabel")));
    } else {
        notes.push("Shorten the imaginary axis name to fit above the plot.".into());
    }
    // Clip region fills, never point labels. Shading is a teacher's given region.
    doc.defs(&format!(
        "<clipPath id=\"plot\"><rect x=\"70\" y=\"70\" width=\"{}\" height=\"{}\"/></clipPath>",
        end.0 - 70.,
        end.1 - 70.
    ));
    let fills = svg::Fills::new(p);
    doc.defs(&fills.defs());
    doc.raw("<g clip-path=\"url(#plot)\">");
    if matches!(p.s("locus"), "Circle" | "Disc") {
        if let (Some(a), Some(b), Some(r)) = (read(p.s("cr")), read(p.s("ci")), read(p.s("lr"))) {
            if r.v > 0. {
                let c = g.pt(a.v, b.v);
                doc.circle(
                    c.0,
                    c.1,
                    r.v * g.u,
                    if p.s("locus") == "Disc" {
                        if print { "url(#vs-hatch0)" } else { "#e1eaf5" }
                    } else {
                        "none"
                    },
                    color(0),
                    1.6,
                );
                if !g.inside(a.v - r.v, b.v - r.v) || !g.inside(a.v + r.v, b.v + r.v) {
                    notes.push("Part of the locus lies outside the axis window.".into())
                }
            } else {
                notes.push("Locus radius must be positive.".into())
            }
        } else {
            notes.push("Invalid locus centre or radius.".into())
        }
    }
    if p.s("locus") == "Argument sector" {
        if let (Some(a), Some(b)) = (read(p.s("lo")), read(p.s("hi"))) {
            let conv = |v: f64| {
                if p.s("unit") == "Degrees" {
                    v.to_radians()
                } else {
                    v
                }
            };
            let (a, b) = (conv(a.v), conv(b.v));
            if a >= -PI && b <= PI && a < b {
                let r = 3. * (g.b - g.a).hypot(g.d - g.c);
                let mut poly = vec![origin];
                for k in 0..=180 {
                    let t = a + (b - a) * k as f64 / 180.;
                    poly.push(g.pt(r * t.cos(), r * t.sin()));
                }
                doc.poly(
                    &poly,
                    if print { "#dddddd" } else { "#e1eaf5" },
                    color(0),
                    1.,
                );
                doc.circle(origin.0, origin.1, 3., "#ffffff", "#111111", 1.);
                notes.push("The argument sector excludes zero (open circle); its boundary rays are included.".into());
            } else {
                notes.push("Use −π ≤ From < To ≤ π (or −180° to 180°).".into())
            }
        } else {
            notes.push("Invalid sector angles.".into())
        }
    }
    doc.raw("</g>");
    if !construct && p.s("operation") == "Roots" && zs.len() > given {
        let r = zs[given].r();
        let fits = g.inside(-r, -r) && g.inside(r, r);
        if fits {
            doc.circle(origin.0, origin.1, r * g.u, "none", color(2), 1.);
        } else {
            notes.push("The root circle extends outside the window; widen the axes.".into());
        }
        if p.b("polygon") && zs[given..].iter().all(|z| g.inside(z.a.v, z.b.v)) {
            let pts: Vec<_> = zs[given..].iter().map(|z| g.pt(z.a.v, z.b.v)).collect();
            doc.poly(&pts, "none", color(2), 1.4);
        }
    }
    if !construct && p.s("operation") == "Sum" && zs.len() > given {
        let a = g.pt(zs[0].a.v, zs[0].b.v);
        let b = g.pt(zs[1].a.v, zs[1].b.v);
        let c = g.pt(zs[given].a.v, zs[given].b.v);
        if g.inside(zs[given].a.v, zs[given].b.v) && zs[..2].iter().all(|z| g.inside(z.a.v, z.b.v))
        {
            doc.arrow_with(a.0, a.1, c.0, c.1, color(1), 1.3, "5 4", false);
            doc.arrow_with(b.0, b.1, c.0, c.1, color(0), 1.3, "5 4", false);
        }
    }
    if !construct
        && p.s("operation") == "Product"
        && zs.len() > given
        && zs[0].r() > 0.
        && zs[1].r() > 0.
    {
        let a = zs[0].angle();
        let b = zs[1].angle();
        let pts: Vec<_> = (0..=50)
            .map(|k| {
                let t = a + b * k as f64 / 50.;
                (origin.0 + 36. * t.cos(), origin.1 - 36. * t.sin())
            })
            .collect();
        doc.polyline(&pts, color(1), 2.);
    }
    if p.s("input") != "Cartesian" && !coord && !blank {
        if let (Some(r), Some(t)) = (read(p.s("radius")), read(p.s("theta"))) {
            let angle = if p.s("unit") == "Degrees" {
                format!("{}°", t.s)
            } else {
                if angle_fraction(p.s("theta"), p.s("unit")).is_some() {
                    p.s("theta").replace("pi", "π").replace('-', "−")
                } else {
                    t.s.clone()
                }
            };
            let line = if p.s("input") == "Polar exponential" {
                format!("Given z: {}e⟦i({angle})⟧", r.s)
            } else {
                format!("Given z: {}(cos({angle}) + i sin({angle}))", r.s)
            };
            doc.raw(&text(620., 58., &line));
        }
    }
    // Primary geometry blocks label boxes; light coordinate gridlines do not.
    let mut avoid: Vec<((f64, f64), (f64, f64))> = vec![];
    let mut add_circle = |centre: (f64, f64), radius: f64| {
        for k in 0..240 {
            let a = 2. * PI * k as f64 / 240.;
            let b = 2. * PI * (k + 1) as f64 / 240.;
            avoid.push((
                (centre.0 + radius * a.cos(), centre.1 - radius * a.sin()),
                (centre.0 + radius * b.cos(), centre.1 - radius * b.sin()),
            ));
        }
    };
    if !construct && p.s("operation") == "Roots" && zs.len() > given {
        let r = zs[given].r();
        if g.inside(-r, -r) && g.inside(r, r) {
            add_circle(origin, r * g.u);
        }
    }
    if p.b("circle") && !coord && !blank && !zs.is_empty() {
        let r = zs[0].r();
        if g.inside(-r, -r) && g.inside(r, r) {
            add_circle(origin, r * g.u);
        }
    }
    if matches!(p.s("locus"), "Circle" | "Disc") {
        if let (Some(a), Some(b), Some(r)) = (read(p.s("cr")), read(p.s("ci")), read(p.s("lr"))) {
            if r.v > 0. {
                add_circle(g.pt(a.v, b.v), r.v * g.u);
            }
        }
    }
    if !construct && p.s("operation") == "Roots" && p.b("polygon") && zs.len() > given {
        let pts: Vec<_> = zs[given..].iter().map(|z| g.pt(z.a.v, z.b.v)).collect();
        for k in 0..pts.len() {
            avoid.push((pts[k], pts[(k + 1) % pts.len()]));
        }
    }
    if !construct
        && p.s("operation") == "Sum"
        && zs.len() > given
        && zs
            .iter()
            .take(2)
            .chain(zs.get(given))
            .all(|z| g.inside(z.a.v, z.b.v))
    {
        let r = g.pt(zs[given].a.v, zs[given].b.v);
        for z in &zs[..2] {
            avoid.push((g.pt(z.a.v, z.b.v), r));
        }
    }
    if p.s("locus") == "Argument sector" {
        if let (Some(a), Some(b)) = (read(p.s("lo")), read(p.s("hi"))) {
            let conv = |v: f64| {
                if p.s("unit") == "Degrees" {
                    v.to_radians()
                } else {
                    v
                }
            };
            let (a, b) = (conv(a.v), conv(b.v));
            if a >= -PI && b <= PI && a < b {
                let radius = 3. * (g.b - g.a).hypot(g.d - g.c);
                for t in [a, b] {
                    avoid.push((origin, g.pt(radius * t.cos(), radius * t.sin())));
                }
            }
        }
    }
    if let Some(z) = zs.first() {
        if p.b("components") && !coord && g.inside(z.a.v, z.b.v) {
            let q = g.pt(z.a.v, z.b.v);
            avoid.push((q, (q.0, origin.1)));
            avoid.push((q, (origin.0, q.1)));
        }
        if p.b("argument") && !coord && !blank && z.r() > 0. && g.inside(z.a.v, z.b.v) {
            for k in 0..40 {
                let a = z.angle() * k as f64 / 40.;
                let b = z.angle() * (k + 1) as f64 / 40.;
                avoid.push((
                    (origin.0 + 24. * a.cos(), origin.1 - 24. * a.sin()),
                    (origin.0 + 24. * b.cos(), origin.1 - 24. * b.sin()),
                ));
            }
        }
    }
    if !construct
        && p.s("operation") == "Product"
        && zs.len() > given
        && zs[0].r() > 0.
        && zs[1].r() > 0.
    {
        for k in 0..50 {
            let a = zs[0].angle() + zs[1].angle() * k as f64 / 50.;
            let b = zs[0].angle() + zs[1].angle() * (k + 1) as f64 / 50.;
            avoid.push((
                (origin.0 + 36. * a.cos(), origin.1 - 36. * a.sin()),
                (origin.0 + 36. * b.cos(), origin.1 - 36. * b.sin()),
            ));
        }
    }
    let mut boxes: Vec<(f64, f64, f64, f64)> = vec![];
    for (i, z) in zs.iter().take(visible).enumerate() {
        let (x, y) = z.xy();
        if !g.inside(x, y) || !x.is_finite() || !y.is_finite() || z.r() > 1e8 {
            notes.push(format!("{} lies outside the window or is too large; widen the axes or reduce its magnitude.",z.name));
            continue;
        }
        let pt = g.pt(x, y);
        if p.s("marks") == "Vectors" && z.r() > 0. {
            doc.arrow(origin.0, origin.1, pt.0, pt.1, color(i), 2.);
        }
        doc.circle(pt.0, pt.1, 4., color(i), color(i), 1.);
        if i == 0 {
            if p.b("modulus") && p.s("marks") == "Points" && !coord && !blank {
                doc.line(origin.0, origin.1, pt.0, pt.1, color(0), 1.);
            }
            if p.b("circle") && !(blank || coord) {
                if g.inside(-z.r(), -z.r()) && g.inside(z.r(), z.r()) {
                    doc.circle(origin.0, origin.1, z.r() * g.u, "none", color(0), 1.);
                } else {
                    notes.push(
                        "The modulus circle extends beyond the window; widen the axes.".into(),
                    )
                }
            }
            if p.b("components") && !coord {
                doc.arrow_with(pt.0, pt.1, pt.0, origin.1, color(i), 1., "4 4", false);
                doc.arrow_with(pt.0, pt.1, origin.0, pt.1, color(i), 1., "4 4", false);
            }
            if p.b("argument") && !blank && !coord && z.r() > 0. {
                let pts: Vec<_> = (0..=40)
                    .map(|k| {
                        let t = z.angle() * k as f64 / 40.;
                        (origin.0 + 24. * t.cos(), origin.1 - 24. * t.sin())
                    })
                    .collect();
                doc.polyline(&pts, color(i), 1.4);
            }
        }
        let label = if coord {
            String::new()
        } else {
            match p.s("labels") {
                "Cartesian" if !blank => format!(
                    "{} {} {}",
                    z.name,
                    if z.exact(p) { "=" } else { "≈" },
                    z.cart(p)
                ),
                "Polar" if !blank => format!("{} {}", z.name, polar(z, p)),
                _ => z.name.clone(),
            }
        };
        if !label.is_empty() {
            let width = super::circle::tw(&label, fs, false);
            let mut found = None;
            for dy in [-12., fs + 12., -fs - 18., 2. * fs + 18.] {
                for dx in [10., -width - 10.] {
                    let bx = pt.0 + dx;
                    let by = pt.1 + dy;
                    let b = (bx, by - fs, bx + width, by + 3.);
                    if b.0 < 65.
                        || b.2 > 590.
                        || b.1 < 65.
                        || b.3 > end.1 - 2.
                        || ((b.1 < origin.1 + 4.) && (b.3 > origin.1 - 4.))
                        || boxes.iter().any(|q| {
                            b.0 < q.2 + 3. && b.2 > q.0 - 3. && b.1 < q.3 + 3. && b.3 > q.1 - 3.
                        })
                    {
                        continue;
                    }
                    let centre = ((b.0 + b.2) / 2., (b.1 + b.3) / 2.);
                    let own = (centre.0 - pt.0).hypot(centre.1 - pt.1);
                    if zs.iter().take(visible).any(|o| {
                        let q = g.pt(o.a.v, o.b.v);
                        (centre.0 - q.0).hypot(centre.1 - q.1) + 0.1 < own
                    }) {
                        continue;
                    }
                    if zs.iter().take(visible).any(|o| {
                        let q = g.pt(o.a.v, o.b.v);
                        q.0 > b.0 - 5. && q.0 < b.2 + 5. && q.1 > b.1 - 5. && q.1 < b.3 + 5.
                    }) {
                        continue;
                    }
                    if p.s("marks") == "Vectors"
                        && zs.iter().take(visible).any(|o| {
                            let q = g.pt(o.a.v, o.b.v);
                            segment_hits(origin, q, (b.0 - 2., b.1 - 2., b.2 + 2., b.3 + 2.))
                        })
                    {
                        continue;
                    }
                    if avoid
                        .iter()
                        .any(|&(a, c)| segment_hits(a, c, (b.0 - 2., b.1 - 2., b.2 + 2., b.3 + 2.)))
                    {
                        continue;
                    }
                    found = Some((bx, by, b));
                    break;
                }
                if found.is_some() {
                    break;
                }
            }
            if let Some((lx, ly, b)) = found {
                let inner = text(lx, ly, &label);
                doc.raw(&labels::wrap(
                    &format!("p{i}"),
                    ((b.0 + b.2) / 2., (b.1 + b.3) / 2.),
                    (width / 2., fs * 0.65),
                    &inner,
                ));
                boxes.push(b);
            } else {
                notes.push(format!("No clear label position for {}; use letter labels, a smaller label size, or move the number.",z.name));
            }
        }
        let yy = 95. + i as f64 * row * 4.;
        let write = |doc: &mut Doc, off: f64, s: String| {
            doc.raw(&text(620., yy + off * row, &s));
        };
        write(
            &mut doc,
            0.,
            if coord {
                format!("Number {} = _____ + _____i", i + 1)
            } else if blank {
                format!("{}", z.name)
            } else {
                format!(
                    "{} {} {}",
                    z.name,
                    if z.exact(p) { "=" } else { "≈" },
                    z.cart(p)
                )
            },
        );
        if p.b("modulus") {
            write(
                &mut doc,
                1.,
                if coord || blank {
                    format!("|{}| = _____", z.name)
                } else {
                    format!(
                        "|{}| {}",
                        z.name,
                        modtext(z, p)
                            .strip_prefix("≈ ")
                            .map(|v| format!("≈ {v}"))
                            .unwrap_or_else(|| format!("= {}", modtext(z, p)))
                    )
                },
            );
        }
        if p.b("argument") {
            if z.r() == 0. {
                notes.push(format!("arg {} is undefined at zero.", z.name));
            } else {
                write(
                    &mut doc,
                    2.,
                    if coord || blank {
                        format!("arg({}) = _____", z.name)
                    } else {
                        let a = angtext(z, p);
                        format!(
                            "arg({}) {}{}",
                            z.name,
                            if a.starts_with('≈') { "" } else { "= " },
                            a
                        )
                    },
                );
            }
        }
        if p.b("polar") && !coord && !blank && z.r() > 0. {
            write(&mut doc, 3., format!("{} {}", z.name, polar(z, p)));
        }
    }
    if !construct && p.s("operation") == "Product" {
        let line = if zs.len() >= 2 && zs[0].r() > 0. && zs[1].r() > 0. {
            if p.s("unit") == "Degrees" {
                "|zw| = |z||w|; arg(zw) ≡ arg z + arg w (mod 360°)"
            } else {
                "|zw| = |z||w|; arg(zw) ≡ arg z + arg w (mod 2π)"
            }
        } else {
            "|zw| = |z||w|"
        };
        doc.raw(&text(70., end.1 + 60., line));
    }
    if construct && p.s("operation") != "None" {
        doc.raw(&text(
            620.,
            110. + visible as f64 * row * 4.,
            &format!("{}: __________________", p.s("operation")),
        ));
    }
    (doc.finish(), notes)
}
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID {
        return None;
    }
    let Some(g) = Win::new(p) else {
        return Some(vec![]);
    };
    let mut notes = vec![];
    let zs = numbers(p, &mut notes);
    let mut hs = vec![];
    for z in zs.iter().filter(|z| z.name == "z" || z.name == "w") {
        let i = if z.name == "z" { 0 } else { 1 };
        if !g.inside(z.a.v, z.b.v) {
            continue;
        }
        let tip = if i == 0 {
            "Drag z to change its real and imaginary parts and update the construction"
        } else {
            "Drag w to change its real and imaginary parts"
        };
        let pos = g.pt(z.a.v, z.b.v);
        hs.push(Handle::knob(pos, tip, move |p, q| {
            if !q.0.is_finite() || !q.1.is_finite() || (q.0 - pos.0).hypot(q.1 - pos.1) < 1e-7 {
                return;
            }
            let x = (g.a + (q.0 - 70.) / g.u).clamp(g.a, g.b);
            let y = (g.d - (q.1 - 70.) / g.u).clamp(g.c, g.d);
            if i == 0 {
                p.set_s("input", "Cartesian");
            }
            p.set_s(if i == 0 { "zr" } else { "wr" }, &math::num_str(x, 2));
            p.set_s(if i == 0 { "zi" } else { "wi" }, &math::num_str(y, 2));
        }));
    }
    hs.extend(labels::handles(render, p));
    Some(hs)
}
pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID {
        return None;
    }
    let s = |k, v: &str| (k, Value::S(v.into()));
    Some(vec![
        Example {
            name: "Conjugate",
            params: vec![s("operation", "Conjugate")],
        },
        Example {
            name: "Sum by a parallelogram",
            params: vec![s("operation", "Sum"), s("zr", "2"), s("zi", "1")],
        },
        Example {
            name: "Multiplication by i",
            params: vec![s("operation", "Product"), s("wr", "0"), s("wi", "1")],
        },
        Example {
            name: "A product",
            params: vec![
                s("operation", "Product"),
                s("zr", "1"),
                s("zi", "1"),
                s("wr", "2"),
                s("wi", "-1"),
            ],
        },
        Example {
            name: "Cube roots of 8",
            params: vec![
                s("operation", "Roots"),
                s("zr", "8"),
                s("zi", "0"),
                ("xmax", Value::F(10.)),
            ],
        },
        Example {
            name: "Fifth roots of unity",
            params: vec![
                s("operation", "Roots"),
                s("zr", "1"),
                s("zi", "0"),
                ("n", Value::I(5)),
            ],
        },
    ])
}
#[cfg(test)]
mod complex_tests;

fn angle_fraction(s: &str, unit: &str) -> Option<Q> {
    let s = s.trim().replace('π', "pi").replace('−', "-");
    let q = if unit == "Degrees" {
        read(&s)?.q?.div(Q(180, 1))
    } else if s == "0" {
        Q(0, 1)
    } else {
        let (a, b) = s.split_once("pi")?;
        let a = match a {
            "" => 1,
            "-" => -1,
            _ => a.trim_end_matches('*').parse::<i64>().ok()?,
        };
        let b = if b.is_empty() {
            1
        } else {
            b.strip_prefix('/')?.parse::<i64>().ok()?
        };
        if b <= 0 || a.unsigned_abs() > 10000 || b > 10000 {
            return None;
        }
        Q::new(a, b)
    };
    let mut n = q.0 % (2 * q.1);
    if n > q.1 {
        n -= 2 * q.1
    }
    if n <= -q.1 {
        n += 2 * q.1
    }
    Some(Q::new(n, q.1))
}

fn segment_hits(a: (f64, f64), b: (f64, f64), r: (f64, f64, f64, f64)) -> bool {
    let (mut lo, mut hi) = (0f64, 1f64);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    for (p, q) in [
        (-dx, a.0 - r.0),
        (dx, r.2 - a.0),
        (-dy, a.1 - r.1),
        (dy, r.3 - a.1),
    ] {
        if p == 0. {
            if q < 0. {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0. {
                lo = lo.max(t)
            } else {
                hi = hi.min(t)
            }
        }
    }
    lo <= hi
}

fn square_exact(n: &N) -> Option<Q> {
    if let Some(q) = n.q {
        if q.0.abs() > 10000 || q.1 > 10000 {
            return None;
        }
        return Some(q.mul(q));
    }
    let (_, a, d) = radical(&n.s.replace('−', "-"))?;
    Some(Q::new(a, d * d))
}
fn radical(s: &str) -> Option<(i64, i64, i64)> {
    let (sign, s) = if let Some(s) = s.strip_prefix('-') {
        (-1, s)
    } else {
        (1, s)
    };
    let (num, den) = s.split_once('/').unwrap_or((s, "1"));
    let den = den.parse::<i64>().ok()?;
    if den <= 0 || den > 10000 {
        return None;
    }
    let a = num
        .strip_prefix('√')
        .or_else(|| num.strip_prefix("sqrt(").and_then(|s| s.strip_suffix(')')))?
        .parse::<i64>()
        .ok()?;
    if a < 0 || a > 1000000 {
        return None;
    }
    Some((sign, a, den))
}

fn safe_sum(a: Q, b: Q) -> Option<Q> {
    let n = a.0 as i128 * b.1 as i128 + b.0 as i128 * a.1 as i128;
    let d = a.1 as i128 * b.1 as i128;
    fn gcd128(a: i128, b: i128) -> i128 {
        if b == 0 {
            a.abs().max(1)
        } else {
            gcd128(b, a % b)
        }
    }
    let g = gcd128(n, d);
    Some(Q(i64::try_from(n / g).ok()?, i64::try_from(d / g).ok()?))
}

fn root_angle(p: &Params, k: i64) -> Option<Q> {
    let z = numbers(p, &mut vec![])
        .into_iter()
        .find(|z| z.name == "z")?;
    let mut exact = p.clone();
    exact.set_s("accuracy", "Exact");
    let a = angtext(&z, &exact);
    let q = angle_fraction(a.trim_end_matches('°'), p.s("unit"))?;
    let q = q.add(Q(2 * k, 1)).div(Q(p.i("n"), 1));
    let mut n = q.0 % (2 * q.1);
    if n > q.1 {
        n -= 2 * q.1
    }
    if n <= -q.1 {
        n += 2 * q.1
    }
    Some(Q::new(n, q.1))
}

fn scientific(s: &str) -> String {
    let Some((a, b)) = s.split_once(['e', 'E']) else {
        return s.replace('-', "−");
    };
    let exponent = b.parse::<i32>().unwrap_or(0).to_string();
    let exponent: String = exponent
        .chars()
        .map(|c| match c {
            '-' => '⁻',
            c => "⁰¹²³⁴⁵⁶⁷⁸⁹"
                .chars()
                .nth(c.to_digit(10).unwrap() as usize)
                .unwrap(),
        })
        .collect();
    format!(
        "{} × 10{}",
        a.trim_start_matches('+').replace('-', "−"),
        exponent
    )
}
