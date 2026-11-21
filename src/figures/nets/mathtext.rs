//! A typed measure set as a book sets it: a fraction stacked over a bar, a root under a drawn radical with its bar over
//! what it covers, π and the digits upright. Read from the written form (5/2, √3, (3√2)/2, 2π, √(13/2)).
//!
//! Each piece is its own `<text>` (the renderer can't mix a missing glyph into one), and the bars and the radical are
//! drawn lines, so nothing depends on a font having √.

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Text(String),
    /// A letter standing for a number, set in italic.
    Var(String),
    Frac(Box<Node>, Box<Node>),
    Sqrt(Box<Node>),
    /// Brackets, dropped when they only group a fraction's top or bottom or a root's inside.
    Paren(Box<Node>),
    Row(Vec<Node>),
}

/// Whether the written form needs setting (a fraction or a root); plain numbers and π stay as text.
pub fn needs_setting(s: &str) -> bool { s.contains('/') || s.contains('\u{221a}') || s.to_lowercase().contains("sqrt") }

/// Whether a Text label is math to set (x, 2x + 1, x − 1, x/2, √x, 6pi, 2pir): no word but pi and sqrt beside single
/// letters, and something there ("1/2 base", "Base length" and "spine" stay words).
pub fn text_is_math(s: &str) -> bool {
    let l = s.to_lowercase();
    !l.trim().is_empty() && l.split(|c: char| !c.is_ascii_alphabetic()).all(|w| w.replace("sqrt", "").replace("pi", "").chars().count() <= 1)
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(String),
    Var(char),
    Pi,
    Sqrt,
    Op(char),
    Slash,
    Open,
    Close,
}

fn lex(s: &str) -> Option<Vec<Tok>> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || c == '.' {
            let st = i;
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                i += 1;
            }
            out.push(Tok::Num(cs[st..i].iter().collect()));
            continue;
        }
        if c.is_ascii_alphabetic() {
            let rest: String = cs[i..].iter().take(4).collect::<String>().to_lowercase();
            if rest.starts_with("sqrt") {
                out.push(Tok::Sqrt);
                i += 4;
            } else if rest.starts_with("pi") {
                out.push(Tok::Pi);
                i += 2;
            } else {
                out.push(Tok::Var(c));
                i += 1;
            }
            continue;
        }
        out.push(match c {
            '*' => Tok::Op('\u{d7}'),
            '\u{3c0}' => Tok::Pi,
            '\u{221a}' => Tok::Sqrt,
            '+' | '\u{2212}' | '-' | '\u{d7}' | '\u{b7}' => Tok::Op(if c == '-' { '\u{2212}' } else { c }),
            '/' => Tok::Slash,
            '(' => Tok::Open,
            ')' => Tok::Close,
            _ => return None,
        });
        i += 1;
    }
    Some(out)
}

struct Rd {
    t: Vec<Tok>,
    i: usize,
}

fn unwrap(n: Node) -> Node {
    match n {
        Node::Paren(x) => *x,
        n => n,
    }
}

impl Rd {
    fn peek(&self) -> Option<&Tok> { self.t.get(self.i) }
    fn expr(&mut self) -> Option<Node> {
        let mut items = vec![self.term()?];
        while let Some(Tok::Op(c @ ('+' | '\u{2212}'))) = self.peek().cloned() {
            self.i += 1;
            items.push(Node::Text(format!(" {c} ")));
            items.push(self.term()?);
        }
        Some(if items.len() == 1 { items.pop().unwrap() } else { Node::Row(items) })
    }
    fn term(&mut self) -> Option<Node> {
        let mut items = vec![self.unary()?];
        loop {
            match self.peek().cloned() {
                Some(Tok::Slash) => {
                    // the whole product so far is the top (3π/2 is 3π over 2, as (3√2)/2 is)
                    self.i += 1;
                    let den = self.unary()?;
                    let num = if items.len() == 1 { items.pop()? } else { Node::Row(std::mem::take(&mut items)) };
                    items.push(Node::Frac(Box::new(unwrap(num)), Box::new(unwrap(den))));
                }
                Some(Tok::Op(c @ ('\u{d7}' | '\u{b7}'))) => {
                    self.i += 1;
                    items.push(Node::Text(c.to_string()));
                    items.push(self.unary()?);
                }
                Some(Tok::Num(_) | Tok::Var(_) | Tok::Pi | Tok::Sqrt | Tok::Open) => items.push(self.unary()?),
                _ => break,
            }
        }
        Some(if items.len() == 1 { items.pop().unwrap() } else { Node::Row(items) })
    }
    fn unary(&mut self) -> Option<Node> {
        if let Some(Tok::Op('\u{2212}')) = self.peek() {
            self.i += 1;
            return Some(Node::Row(vec![Node::Text("\u{2212}".into()), self.power()?]));
        }
        self.power()
    }
    fn power(&mut self) -> Option<Node> {
        let t = self.peek()?.clone();
        self.i += 1;
        match t {
            Tok::Num(s) => Some(Node::Text(s)),
            Tok::Var(c) => Some(Node::Var(c.to_string())),
            Tok::Pi => Some(Node::Text("\u{3c0}".into())),
            Tok::Sqrt => Some(Node::Sqrt(Box::new(unwrap(self.power()?)))),
            Tok::Open => {
                let e = self.expr()?;
                (self.peek() == Some(&Tok::Close)).then(|| {
                    self.i += 1;
                    Node::Paren(Box::new(e))
                })
            }
            _ => None,
        }
    }
}

/// The written form read for setting; None when it isn't one this can set (it's then written flat).
pub fn parse(s: &str) -> Option<Node> {
    let mut r = Rd { t: lex(s)?, i: 0 };
    let n = r.expr()?;
    (r.i == r.t.len()).then_some(n)
}

/// A set piece's size: width, height above the baseline and depth below it.
#[derive(Clone, Copy, Debug)]
pub struct Bx {
    pub w: f64,
    pub asc: f64,
    pub desc: f64,
}

#[derive(Clone, Debug)]
pub enum Draw {
    /// Text starting at (x, baseline y).
    Text { x: f64, y: f64, size: f64, s: String, italic: bool },
    Line { a: (f64, f64), b: (f64, f64), w: f64 },
    Path(Vec<(f64, f64)>, f64),
}

/// Lays a node out with its baseline at y = 0 and its left edge at x = 0.
pub fn layout(n: &Node, size: f64, width: &dyn Fn(&str, f64) -> f64) -> (Bx, Vec<Draw>) {
    match n {
        Node::Text(s) => (Bx { w: width(s, size), asc: 0.72 * size, desc: 0.24 * size }, vec![Draw::Text { x: 0.0, y: 0.0, size, s: s.clone(), italic: false }]),
        Node::Var(s) => (Bx { w: width(s, size) * 1.08, asc: 0.72 * size, desc: 0.24 * size }, vec![Draw::Text { x: 0.0, y: 0.0, size, s: s.clone(), italic: true }]),
        Node::Row(v) => {
            let mut x = 0.0;
            let mut b = Bx { w: 0.0, asc: 0.0, desc: 0.0 };
            let mut out = Vec::new();
            for c in v {
                let (cb, d) = layout(c, size, width);
                out.extend(d.into_iter().map(|q| shift(q, x, 0.0)));
                x += cb.w;
                b.asc = b.asc.max(cb.asc);
                b.desc = b.desc.max(cb.desc);
            }
            b.w = x;
            (b, out)
        }
        Node::Paren(x) => layout(&Node::Row(vec![Node::Text("(".into()), (**x).clone(), Node::Text(")".into())]), size, width),
        Node::Frac(a, b) => {
            let sub = (size * 0.8).max(11.0).min(size);
            let (na, da) = layout(a, sub, width);
            let (nb, db) = layout(b, sub, width);
            let pad = 0.12 * size;
            let w = na.w.max(nb.w) + 2.0 * pad;
            let axis = 0.3 * size;
            let gap = 0.1 * size;
            let top = -axis - gap - na.desc;
            let bot = -axis + gap + nb.asc;
            let mut out: Vec<Draw> = da.into_iter().map(|q| shift(q, (w - na.w) / 2.0, top)).collect();
            out.extend(db.into_iter().map(|q| shift(q, (w - nb.w) / 2.0, bot)));
            out.push(Draw::Line { a: (pad * 0.5, -axis), b: (w - pad * 0.5, -axis), w: (0.06 * size).max(1.0) });
            (Bx { w, asc: axis + gap + na.desc + na.asc, desc: (nb.asc + nb.desc + gap - axis).max(0.24 * size) }, out)
        }
        Node::Sqrt(x) => {
            let (cb, d) = layout(x, size, width);
            let sw = 0.55 * size;
            let lw = (0.06 * size).max(1.0);
            let bar = -(cb.asc + 0.1 * size);
            let mut out: Vec<Draw> = d.into_iter().map(|q| shift(q, sw + 0.04 * size, 0.0)).collect();
            let w = sw + cb.w + 0.12 * size;
            out.push(Draw::Path(vec![(0.02 * size, -0.32 * size), (0.14 * size, -0.4 * size), (0.28 * size, cb.desc * 0.6), (sw, bar), (w, bar)], lw));
            (Bx { w, asc: -bar + lw, desc: cb.desc }, out)
        }
    }
}

fn shift(q: Draw, dx: f64, dy: f64) -> Draw {
    match q {
        Draw::Text { x, y, size, s, italic } => Draw::Text { x: x + dx, y: y + dy, size, s, italic },
        Draw::Line { a, b, w } => Draw::Line { a: (a.0 + dx, a.1 + dy), b: (b.0 + dx, b.1 + dy), w },
        Draw::Path(v, w) => Draw::Path(v.into_iter().map(|p| (p.0 + dx, p.1 + dy)).collect(), w),
    }
}

/// The SVG of a laid-out label whose box's top-left corner is at (x0, y0) and whose baseline is `asc` below that.
pub fn svg(draws: &[Draw], x0: f64, base: f64, ink: &str, font: &str) -> String {
    let mut s = String::new();
    // the white halo first, under everything, so a line it sits over is cut cleanly
    for d in draws {
        match d {
            Draw::Line { a, b, w } => s.push_str(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#fff" stroke-width="{:.2}" stroke-linecap="round"/>"##, a.0 + x0, a.1 + base, b.0 + x0, b.1 + base, w + 4.0)),
            Draw::Path(v, w) => s.push_str(&format!(r##"<polyline points="{}" fill="none" stroke="#fff" stroke-width="{:.2}" stroke-linejoin="round" stroke-linecap="round"/>"##, pts(v, x0, base), w + 4.0)),
            _ => {}
        }
    }
    for d in draws {
        match d {
            Draw::Text { x, y, size, s: t, italic } => s.push_str(&format!(
                r##"<text x="{:.2}" y="{:.2}" font-size="{size:.1}" text-anchor="start" font-family="{font}"{} fill="{ink}" stroke="#fff" stroke-width="{:.1}" stroke-linejoin="round" paint-order="stroke" xml:space="preserve">{}</text>"##,
                x + x0,
                y + base,
                if *italic { r#" font-style="italic""# } else { "" },
                size * 0.22,
                crate::figures::svg::esc(t)
            )),
            Draw::Line { a, b, w } => s.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{ink}" stroke-width="{w:.2}" stroke-linecap="round"/>"#, a.0 + x0, a.1 + base, b.0 + x0, b.1 + base)),
            Draw::Path(v, w) => s.push_str(&format!(r#"<polyline points="{}" fill="none" stroke="{ink}" stroke-width="{w:.2}" stroke-linejoin="round" stroke-linecap="round"/>"#, pts(v, x0, base))),
        }
    }
    s
}

fn pts(v: &[(f64, f64)], x0: f64, base: f64) -> String { v.iter().map(|p| format!("{:.2},{:.2}", p.0 + x0, p.1 + base)).collect::<Vec<_>>().join(" ") }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_forms_read_as_a_book_sets_them() {
        let t = |s: &str| Node::Text(s.into());
        assert_eq!(parse("5/2"), Some(Node::Frac(Box::new(t("5")), Box::new(t("2")))));
        assert_eq!(parse("\u{221a}3"), Some(Node::Sqrt(Box::new(t("3")))));
        // the brackets round a root before a division only group it: the bar does that
        assert_eq!(parse("(3\u{221a}2)/2"), Some(Node::Frac(Box::new(Node::Row(vec![t("3"), Node::Sqrt(Box::new(t("2")))])), Box::new(t("2")))));
        assert_eq!(parse("\u{221a}(13/2)"), Some(Node::Sqrt(Box::new(Node::Frac(Box::new(t("13")), Box::new(t("2")))))));
        assert_eq!(parse("2\u{3c0}"), Some(Node::Row(vec![t("2"), t("\u{3c0}")])));
        assert_eq!(parse("2 + \u{221a}3"), Some(Node::Row(vec![t("2"), t(" + "), Node::Sqrt(Box::new(t("3")))])));
        assert_eq!(parse("3\u{3c0}/2"), Some(Node::Frac(Box::new(Node::Row(vec![t("3"), t("\u{3c0}")])), Box::new(t("2")))));
        assert_eq!(parse("x/2"), Some(Node::Frac(Box::new(Node::Var("x".into())), Box::new(t("2")))));
        assert_eq!(parse("sqrt(x)"), Some(Node::Sqrt(Box::new(Node::Var("x".into())))));
        assert!(parse("x?").is_none());
        assert!(text_is_math("6pi") && text_is_math("2pir") && text_is_math("x/2") && text_is_math("sqrt(x)"));
        assert!(!text_is_math("1/2 base") && !text_is_math("spine") && !text_is_math("Base length") && text_is_math("x") && text_is_math("x - 1"));
        assert_eq!(parse("x - 1"), Some(Node::Row(vec![Node::Var("x".into()), t(" \u{2212} "), t("1")])));
        assert_eq!(parse("2pir"), Some(Node::Row(vec![t("2"), t("\u{3c0}"), Node::Var("r".into())])));
        assert!(needs_setting("5/2") && needs_setting("\u{221a}3") && !needs_setting("2\u{3c0}"));
    }

    #[test]
    fn a_fraction_stands_taller_than_a_number() {
        let w = |s: &str, size: f64| s.chars().count() as f64 * size * 0.5;
        let (f, _) = layout(&parse("5/2").unwrap(), 18.0, &w);
        let (n, _) = layout(&parse("5").unwrap(), 18.0, &w);
        assert!(f.asc + f.desc > 1.3 * (n.asc + n.desc));
        let (r, d) = layout(&parse("\u{221a}3").unwrap(), 18.0, &w);
        assert!(r.w > n.w && d.iter().any(|x| matches!(x, Draw::Path(..))));
    }
}
