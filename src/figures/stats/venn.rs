//! Venn diagram of one, two or three sets: text in every region (typed, or sorted from each set's members), any regions
//! shaded, the universal set named, and the set totals worked out underneath. The student copy (Show answers off) keeps the
//! circles and any shading and leaves the regions and totals empty (or gives each region a blank line).
//!
//! On the figure (`handles`): a click in a region shades it (again to clear it), a double-click types what goes in it, and a
//! double-click on a set's name renames it. Shading is one typed list (`shade`): region names or set expressions such as
//! `A and B`, `A or B`, `not A`, `A only`, `(A or B) and not C`. Saved figures from before the arrangements, the universal
//! set's label and the shading colours open as they were (`migrate`).
use std::sync::Arc;

use crate::figures::chart::{self, INK};
use crate::figures::interact::{self, EditAt, Handle, Hover, Pt};
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "venn-diagram";
const ARRANGEMENTS: [&str; 3] = ["Overlapping", "Separate", "B inside A"];
const UNIVERSAL: [&str; 6] = ["U", "\u{3be}", "S", "\u{3b5}", "Word", "None"];
const FILLS: [&str; 10] = ["Blue", "Orange", "Green", "Red", "Purple", "Grey", "Hatching", "Cross-hatching", "Dots", "Strong blue"];
/// What a click's shading is written as when it matches one (else the regions are listed).
const PRESETS: [&str; 15] = ["A", "B", "C", "A and B", "A or B", "A only", "B only", "C only", "Not A", "Not B", "Not C", "Outside", "A and B and C", "A or B or C", "Everything"];
/// The longest text a region takes, and the most members Sets sorts.
const MAX_TEXT: usize = 80;
const MAX_MEMBERS: usize = 200;

pub fn def() -> FigureDef {
    FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Venn diagram",
        // unchanged from the two- and three-set figure: the blurb is written into every saved figure's SVG
        blurb: "A two- or three-set Venn diagram with a count in every region, an optional shaded region and the set totals worked out, or left empty for students.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    if fig != ID {
        return true;
    }
    let sets = p.s("sets");
    let sorted = p.s("mode") == "Sets";
    match key {
        "c_name" => sets == "3",
        "b_name" => sets != "1",
        "arrangement" => sets == "2",
        "counts" => !sorted,
        "u_set" | "a_set" => sorted,
        "b_set" => sorted && sets != "1",
        "c_set" => sorted && sets == "3",
        "u_word" => p.s("universal") == "Word",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        ID => vec![
            Example { name: "Two sets: a class survey", params: vec![] },
            Example {
                name: "Three sets",
                params: vec![("sets", s("3")), ("c_name", s("Art")), ("counts", s("8, 6, 5, 4, 3, 2, 1, 7"))],
            },
            Example { name: "Shade the overlap, blank regions", params: vec![("shade", s("A and B")), ("fill", s("Blue")), ("show_answers", Value::B(false))] },
            Example {
                name: "Elements instead of counts",
                params: vec![("a_name", s("Even")), ("b_name", s("Prime")), ("counts", s("4 6 8, 3 5 7, 2, 1 9")), ("totals", Value::B(false))],
            },
            Example {
                name: "Numbers 1 to 12 sorted for you",
                params: vec![
                    ("mode", s("Sets")),
                    ("a_name", s("Even")),
                    ("b_name", s("Prime")),
                    ("u_set", s("1-12")),
                    ("a_set", s("2, 4, 6, 8, 10, 12")),
                    ("b_set", s("2, 3, 5, 7, 11")),
                    ("universal", s("\u{3be}")),
                ],
            },
            Example {
                name: "Prime factors of 12 and 18",
                params: vec![("mode", s("Sets")), ("a_name", s("12")), ("b_name", s("18")), ("u_set", s("")), ("a_set", s("2, 2, 3")), ("b_set", s("2, 3, 3")), ("universal", s("None")), ("totals", Value::B(false))],
            },
            Example {
                name: "One set inside another",
                params: vec![("arrangement", s("B inside A")), ("a_name", s("Rectangles")), ("b_name", s("Squares")), ("universal", s("Word")), ("u_word", s("Quadrilaterals")), ("counts", s("6, 4, 3")), ("totals", Value::B(false))],
            },
            Example { name: "Shade A or B, not both, hatched", params: vec![("shade", s("A only, B only")), ("fill", s("Hatching")), ("counts", s("")), ("totals", Value::B(false))] },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("mode", "In each region", &["Text", "Sets"], "Text").hint("Text: you type what goes in each region. Sets: you list each set's members and they are sorted into the regions"),
        ParamDef::choice("sets", "Sets", &["1", "2", "3"], "2"),
        ParamDef::choice("arrangement", "Arrangement", &ARRANGEMENTS, "Overlapping"),
        ParamDef::text("a_name", "Set A", "Music"),
        ParamDef::text("b_name", "Set B", "Sport"),
        ParamDef::text("c_name", "Set C", "Art"),
        ParamDef::text("counts", "Region values", "12, 9, 5, 4").hint("Separated by commas, in this order. One set: A, outside. Two sets: A only, B only, both, neither (separate sets: A, B, neither; B inside A: A only, B, neither). Three sets: A only, B only, C only, A and B only, A and C only, B and C only, all three, none. A number, letter, expression or words; a semicolon starts a new line in a region. Or double-click a region on the figure to type in it"),
        ParamDef::text("u_set", "Universal set", "Ana, Ben, Cara, Dev, Eli, Fay, Gus").hint("Every member, with commas, or a range such as 1-20. Members in no set go outside the circles; leave it empty to put nothing there"),
        ParamDef::text("a_set", "Members of A", "Ana, Ben, Cara, Dev").hint("With commas, or a range such as 1-20. Repeat a member to put it in twice (prime factors: 2, 2, 3)"),
        ParamDef::text("b_set", "Members of B", "Cara, Dev, Eli"),
        ParamDef::text("c_set", "Members of C", "Ben, Dev, Fay"),
        ParamDef::text("shade", "Shaded regions", "None").hint("Click a region on the figure to shade it, or type regions or set expressions with commas between: A only, A and B, A or B, not A, outside, (A or B) and not C. A, B and C are the sets in order, whatever their names. None shades nothing"),
        // the plain default is the strong blue every Venn figure was shaded in before the colours (saved figures and share
        // links from then have no `fill`); a new figure opens on the light blue tint (start.rs, share links v7)
        ParamDef::choice("fill", "Shading", &FILLS, "Strong blue").hint("A colour tint, grey, or a black-and-white pattern. Print style turns the tints grey. Strong blue is the deeper fill of Venn figures made before November 2026"),
        ParamDef::choice("universal", "Universal set label", &UNIVERSAL, "U"),
        ParamDef::text("u_word", "Universal set word", "Students"),
        ParamDef::choice("empty", "Empty regions", &["Nothing", "A blank line"], "Nothing").hint("A blank line gives every empty region (and every region in the student copy) a line to write on"),
        ParamDef::bool("totals", "Work out the totals underneath", true),
        ParamDef::text("title", "Title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off keeps the circles and the shading and leaves the region values and totals empty for students"),
    ]
}

/// A saved figure from before the arrangements, the universal set's label and the shading colours (its spec has no
/// `arrangement`): each region value keeps the 14 characters that were drawn (longer values were cut there), so it opens
/// as it was saved. (Its shading keeps its strong blue through `fill`'s plain default.)
pub fn migrate(spec: &crate::model::FigureSpec, p: &mut Params) {
    if spec.params.contains_key("arrangement") {
        return;
    }
    if let Some(cut) = cut_old_values(p.s("counts")) {
        p.set_s("counts", &cut);
    }
}

/// A share link made before links v7 (they carry only the keys that differ from the opening state, so the missing
/// `arrangement` cannot mark them): its region values keep the 14 characters that were drawn, as `migrate` does for a
/// saved figure. Called by the workspace's `apply_link` on the link's params (opening state plus the link's changes).
pub fn migrate_link(id: &str, version: u32, params: &mut std::collections::BTreeMap<String, serde_json::Value>) {
    if id != ID || version >= 7 {
        return;
    }
    if let Some(cut) = params.get("counts").and_then(|v| v.as_str()).and_then(cut_old_values) {
        params.insert("counts".into(), serde_json::Value::String(cut));
    }
}

/// Region values cut to the 14 characters the figure drew before November 2026, when any was longer.
fn cut_old_values(counts: &str) -> Option<String> {
    counts.split(',').any(|v| v.trim().chars().count() > 14).then(|| counts.split(',').map(|v| v.trim().chars().take(14).collect::<String>()).collect::<Vec<_>>().join(", "))
}

// ------------------------------------------------------------------------------------------------ layout

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Arr {
    One,
    Over2,
    Separate,
    Inside,
    Over3,
}

/// An axis-aligned box (x0, y0, x1, y1).
type Bx = (f64, f64, f64, f64);

fn overlaps(a: &Bx, b: &Bx) -> bool { a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3 }

/// Advance width of `s` at font size 1, from Noto Sans' metrics (near enough for layout; the tests measure real ink).
fn em_width(s: &str) -> f64 {
    s.chars()
        .map(|c| match c {
            '0'..='9' => 0.572,
            ' ' => 0.26,
            'i' | 'j' | 'l' | '.' | ',' | ':' | ';' | '\'' | '|' | '!' => 0.27,
            'f' | 't' | 'r' | '(' | ')' | '[' | ']' | '-' | '/' | 'I' | 'J' => 0.37,
            'm' | 'w' => 0.88,
            'M' | 'W' => 0.92,
            'a'..='z' => 0.57,
            'A'..='Z' => 0.68,
            _ => 0.62,
        })
        .sum()
}

/// Ink box of one line of text at baseline `y`, anchored `anchor` at `x`.
fn line_box(x: f64, y: f64, size: f64, anchor: &str, s: &str) -> Bx {
    let w = em_width(s) * size;
    let x0 = match anchor {
        "start" => x,
        "end" => x - w,
        _ => x - w / 2.0,
    };
    (x0, y - 0.76 * size, x0 + w, y + descent(s) * size)
}

/// How far below the baseline `s` reaches, at font size 1 (descenders, commas and brackets go lower).
fn descent(s: &str) -> f64 {
    if s.chars().any(|c| matches!(c, 'g' | 'j' | 'p' | 'q' | 'y' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '_' | 'Q' | 'J' | '/')) { 0.26 } else { 0.06 }
}

/// Everything the drawing and the handles share.
struct Geo {
    arr: Arr,
    n: usize,
    fs: f64,
    top: f64,
    bottom: f64,
    w: f64,
    rect: Bx,
    rect_xywh: (f64, f64, f64, f64),
    circles: Vec<(f64, f64, f64)>,
    /// Region masks (A = 1, B = 2, C = 4) in the order their values are typed, each with where its text may go (the first is
    /// where it went before the arrangements existed).
    regions: Vec<(usize, Vec<Pt>)>,
    names: Vec<String>,
    /// Each name: baseline position, anchor and size.
    name_at: Vec<(f64, f64, &'static str, f64)>,
    /// The universal set's label: text, size, baseline position and anchor (None when none is drawn).
    ulabel: Option<(String, f64, Pt, &'static str)>,
}

impl Geo {
    fn new(p: &Params) -> Geo {
        let n: usize = match p.s("sets") { "1" => 1, "3" => 3, _ => 2 };
        let arr = match (n, p.s("arrangement")) {
            (1, _) => Arr::One,
            (3, _) => Arr::Over3,
            (_, "Separate") => Arr::Separate,
            (_, "B inside A") => Arr::Inside,
            _ => Arr::Over2,
        };
        let k = svg::label_factor(p.s("label_size"));
        let fs = 15.0 * k;
        let title = p.s("title").trim();
        let top = if title.is_empty() { 0.0 } else { 14.0 + fs * 1.5 };
        let clip = |s: &str, m: usize| -> String { s.trim().chars().take(m).collect() };
        let names: Vec<String> =
            [p.s("a_name"), p.s("b_name"), p.s("c_name")].iter().enumerate().map(|(i, s)| { let c = clip(s, 12); if c.is_empty() { ["A", "B", "C"][i].to_string() } else { c } }).collect();
        type L = (Bx, Vec<(f64, f64, f64)>, Vec<(usize, Vec<Pt>)>, Vec<(f64, f64, &'static str)>, f64);
        let (rect, circles, regions, name_base, bottom): L = match arr {
            Arr::Over2 => (
                (20.0, 20.0, 480.0, 280.0),
                vec![(205.0, 160.0, 105.0), (315.0, 160.0, 105.0)],
                vec![
                    (1, vec![(150.0, 165.0), (140.0, 130.0), (140.0, 200.0)]),
                    (2, vec![(370.0, 165.0), (380.0, 130.0), (380.0, 200.0)]),
                    (3, vec![(260.0, 165.0)]),
                    (0, vec![(48.0, 284.0), (472.0, 284.0), (60.0, 268.0), (460.0, 268.0), (260.0, 288.0), (472.0, 44.0)]),
                ],
                vec![(150.0, 44.0, "middle"), (370.0, 44.0, "middle")],
                300.0,
            ),
            Arr::Over3 => (
                (20.0, 20.0, 480.0, 360.0),
                vec![(205.0, 150.0, 95.0), (315.0, 150.0, 95.0), (260.0, 245.0, 95.0)],
                vec![
                    (1, vec![(160.0, 130.0), (150.0, 160.0)]),
                    (2, vec![(360.0, 130.0), (370.0, 160.0)]),
                    (4, vec![(260.0, 300.0), (260.0, 285.0)]),
                    (3, vec![(260.0, 112.0), (260.0, 100.0)]),
                    (5, vec![(212.0, 210.0)]),
                    (6, vec![(308.0, 210.0)]),
                    (7, vec![(260.0, 178.0)]),
                    (0, vec![(48.0, 364.0), (460.0, 300.0), (460.0, 340.0), (60.0, 300.0), (60.0, 340.0), (460.0, 250.0)]),
                ],
                vec![(140.0, 44.0, "middle"), (380.0, 44.0, "middle"), (352.0, 338.0, "start")],
                380.0,
            ),
            Arr::One => (
                (20.0, 20.0, 480.0, 280.0),
                vec![(260.0, 168.0, 110.0)],
                vec![(1, vec![(260.0, 168.0)]), (0, vec![(48.0, 284.0), (472.0, 284.0), (64.0, 262.0), (456.0, 262.0), (456.0, 80.0)])],
                vec![(150.0, 44.0, "middle")],
                300.0,
            ),
            Arr::Separate => (
                (20.0, 20.0, 480.0, 280.0),
                vec![(140.0, 168.0, 100.0), (380.0, 168.0, 100.0)],
                vec![(1, vec![(140.0, 168.0)]), (2, vec![(380.0, 168.0)]), (0, vec![(48.0, 284.0), (472.0, 284.0), (260.0, 64.0), (260.0, 282.0), (260.0, 168.0)])],
                vec![(140.0, 50.0, "middle"), (380.0, 50.0, "middle")],
                300.0,
            ),
            Arr::Inside => (
                (20.0, 20.0, 480.0, 280.0),
                vec![(260.0, 172.0, 118.0), (300.0, 192.0, 62.0)],
                vec![(1, vec![(195.0, 172.0), (215.0, 230.0), (200.0, 120.0)]), (3, vec![(300.0, 192.0)]), (0, vec![(48.0, 284.0), (472.0, 284.0), (64.0, 262.0), (456.0, 262.0), (450.0, 80.0)])],
                vec![(150.0, 44.0, "middle"), (300.0, 116.0, "middle")],
                300.0,
            ),
        };
        let rect_xywh = (rect.0, rect.1 + top, rect.2, rect.3);
        let circles: Vec<(f64, f64, f64)> = circles.into_iter().map(|(x, y, r)| (x, y + top, r)).collect();
        let regions: Vec<(usize, Vec<Pt>)> = regions.into_iter().map(|(m, v)| (m, v.into_iter().map(|(x, y)| (x, y + top)).collect())).collect();
        let mut g = Geo {
            arr,
            n,
            fs,
            top,
            bottom,
            w: 520.0,
            rect: (rect_xywh.0, rect_xywh.1, rect_xywh.0 + rect_xywh.2, rect_xywh.1 + rect_xywh.3),
            rect_xywh,
            circles,
            regions,
            names,
            name_at: vec![],
            ulabel: None,
        };
        // a name keeps its place and size unless its text would cross a circle or leave the box: then it shrinks to fit
        for (i, (x, y, a)) in name_base.iter().enumerate() {
            let (x, y) = (*x, y + top);
            let s0 = fs * 1.1;
            let mut size = s0;
            while size > 9.0 && !g.clear(&line_box(x, y, size, a, &g.names[i]), 3.0) {
                size = (size * 0.92).max(9.0);
            }
            g.name_at.push((x, y, a, size));
        }
        let word: String = p.s("u_word").trim().chars().take(24).collect();
        let label = match p.s("universal") {
            "None" => None,
            "Word" if word.is_empty() => None,
            "Word" => Some(word),
            "" => Some("U".to_string()),
            u => Some(u.to_string()),
        };
        // top left (where U always went); a word too long for there goes in the bottom right corner, else shrinks top left
        // top left (where U always went) at the largest size that fits, else the bottom right corner; failing both, cut short
        g.ulabel = label.map(|t| {
            let names = g.name_boxes();
            let ok = |t: &str, at: Pt, size: f64, a: &str| {
                let b = line_box(at.0, at.1, size, a, t);
                !names.iter().any(|n| overlaps(n, &b)) && g.clear(&b, 3.0)
            };
            let tl = (g.rect.0 + 12.0, g.rect.1 + fs * 1.3);
            let br = (g.rect.2 - 12.0, g.rect.3 - fs * 0.6);
            let mut size = fs;
            loop {
                if ok(&t, tl, size, "start") {
                    return (t, size, tl, "start");
                }
                if ok(&t, br, size, "end") {
                    return (t, size, br, "end");
                }
                if size <= 9.0 {
                    break;
                }
                size = (size * 0.92).max(9.0);
            }
            let mut cut: Vec<char> = t.chars().collect();
            while cut.len() > 1 {
                cut.pop();
                let c = format!("{}\u{2026}", cut.iter().collect::<String>().trim_end());
                if ok(&c, tl, 9.0, "start") {
                    return (c, 9.0, tl, "start");
                }
            }
            (t.chars().take(1).collect(), 9.0, tl, "start")
        });
        g
    }

    fn name_boxes(&self) -> Vec<Bx> {
        self.name_at.iter().enumerate().map(|(i, (x, y, a, s))| line_box(*x, *y, *s, a, &self.names[i])).collect()
    }

    fn ulabel_box(&self) -> Option<Bx> {
        self.ulabel.as_ref().map(|(t, s, at, a)| line_box(at.0, at.1, *s, a, t))
    }

    fn h(&self, rows: usize, totals: bool, notes: usize) -> f64 {
        let foot = if totals { 24.0 + rows as f64 * self.fs * 1.7 } else { 10.0 };
        self.top + self.bottom + foot + notes as f64 * self.fs * 1.5
    }

    /// The membership mask (A = 1, B = 2, C = 4) of a point inside the universal box, None outside it.
    fn mask_at(&self, q: Pt) -> Option<usize> {
        if !(q.0.is_finite() && q.1.is_finite()) || q.0 < self.rect.0 || q.0 > self.rect.2 || q.1 < self.rect.1 || q.1 > self.rect.3 {
            return None;
        }
        Some(self.circles.iter().enumerate().filter(|(_, c)| interact::dist((c.0, c.1), q) <= c.2).map(|(i, _)| 1 << i).sum())
    }

    /// The masks drawn as regions.
    fn masks(&self) -> Vec<usize> { self.regions.iter().map(|r| r.0).collect() }

    /// The regions as a bit set over masks (bit m for mask m).
    fn all(&self) -> u8 { self.masks().iter().fold(0, |a, m| a | 1 << m) }

    /// Point `q` lies in region `mask`, at least `m` from every line.
    fn inside(&self, mask: usize, q: Pt, m: f64) -> bool {
        if q.0 < self.rect.0 + m || q.0 > self.rect.2 - m || q.1 < self.rect.1 + m || q.1 > self.rect.3 - m {
            return false;
        }
        self.circles.iter().enumerate().all(|(i, c)| {
            let d = interact::dist((c.0, c.1), q);
            if mask & (1 << i) != 0 { d <= c.2 - m } else { d >= c.2 + m }
        })
    }

    /// Box `b` is inside the universal box and crosses no circle, each by at least `m`.
    fn clear(&self, b: &Bx, m: f64) -> bool {
        if b.0 < self.rect.0 + m || b.2 > self.rect.2 - m || b.1 < self.rect.1 + m || b.3 > self.rect.3 - m {
            return false;
        }
        samples(b).all(|q| self.circles.iter().all(|c| (interact::dist((c.0, c.1), q) - c.2).abs() >= m))
    }

    /// Box `b` lies in region `mask` with a margin and clear of `avoid`.
    fn fits(&self, mask: usize, b: &Bx, avoid: &[Bx]) -> bool {
        avoid.iter().all(|a| !overlaps(a, b)) && samples(b).all(|q| self.inside(mask, q, 3.0))
    }

    /// What a region is called on the figure, with the sets' names.
    fn region_name(&self, mask: usize) -> String {
        let nm = |i: usize| self.names[i].clone();
        match (self.arr, mask) {
            (_, 0) => "outside the circles".to_string(),
            (Arr::One | Arr::Separate, m) => nm(m.trailing_zeros() as usize),
            (Arr::Inside, 3) => nm(1),
            (Arr::Over2, 3) => format!("{} and {}", nm(0), nm(1)),
            (Arr::Over3, 7) => format!("{}, {} and {}", nm(0), nm(1), nm(2)),
            (_, m) if m.count_ones() == 1 => format!("{} only", nm(m.trailing_zeros() as usize)),
            (_, m) => {
                let v: Vec<String> = (0..3).filter(|i| m & (1 << i) != 0).map(nm).collect();
                format!("{} only", v.join(" and "))
            }
        }
    }

    /// A region's name in the Shaded regions box (sets as letters, so it reads back whatever the sets are called).
    fn region_code(&self, mask: usize) -> &'static str {
        match (self.arr, mask) {
            (_, 0) => "Outside",
            (Arr::One | Arr::Separate, 1) => "A",
            (Arr::Separate, 2) => "B",
            (Arr::Inside, 3) => "B",
            (Arr::Over2, 3) => "A and B",
            (_, 1) => "A only",
            (_, 2) => "B only",
            (_, 4) => "C only",
            (_, 3) => "A and B only",
            (_, 5) => "A and C only",
            (_, 6) => "B and C only",
            _ => "A and B and C",
        }
    }

    /// The outline of region `mask` (for the hover): the universal box for outside the circles, else the arcs that bound it
    /// joined in order (an inner circle's loop is left out of a ring).
    fn outline(&self, mask: usize) -> Vec<Pt> {
        let (x0, y0, x1, y1) = self.rect;
        if mask == 0 {
            return vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        }
        const N: usize = 360;
        let mut runs: Vec<Vec<Pt>> = vec![];
        for (i, c) in self.circles.iter().enumerate() {
            let on: Vec<(bool, Pt)> = (0..N)
                .map(|k| {
                    let q = interact::polar((c.0, c.1), k as f64, c.2);
                    // the side of this circle the region lies on matches, and the other circles agree with the mask here
                    let ok = self.circles.iter().enumerate().all(|(j, o)| j == i || (interact::dist((o.0, o.1), q) <= o.2) == (mask & (1 << j) != 0));
                    (ok, q)
                })
                .collect();
            if on.iter().all(|x| x.0) {
                runs.push(on.iter().map(|x| x.1).collect());
                continue;
            }
            let start = on.iter().position(|x| !x.0).unwrap_or(0);
            let mut cur: Vec<Pt> = vec![];
            for k in 1..=N {
                let (ok, q) = on[(start + k) % N];
                if ok {
                    cur.push(q);
                } else if !cur.is_empty() {
                    runs.push(std::mem::take(&mut cur));
                }
            }
            if !cur.is_empty() {
                runs.push(cur);
            }
        }
        // chain the runs end to end, starting from the longest, while the next one starts close by
        runs.sort_by_key(|r| std::cmp::Reverse(r.len()));
        let mut out = if runs.is_empty() { return vec![] } else { runs.remove(0) };
        while !runs.is_empty() {
            let end = *out.last().unwrap();
            let (bi, rev, d) = runs
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let (a, b) = (interact::dist(end, r[0]), interact::dist(end, *r.last().unwrap()));
                    if a <= b { (i, false, a) } else { (i, true, b) }
                })
                .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap();
            if d > 8.0 {
                break;
            }
            let mut r = runs.remove(bi);
            if rev {
                r.reverse();
            }
            out.extend(r);
        }
        out
    }
}

/// Points over box `b`, about every 4 units, edges included.
fn samples(b: &Bx) -> impl Iterator<Item = Pt> + '_ {
    let nx = ((b.2 - b.0) / 4.0).ceil().max(1.0) as usize;
    let ny = ((b.3 - b.1) / 4.0).ceil().max(1.0) as usize;
    (0..=nx).flat_map(move |i| (0..=ny).map(move |j| (b.0 + (b.2 - b.0) * i as f64 / nx as f64, b.1 + (b.3 - b.1) * j as f64 / ny as f64)))
}

// ------------------------------------------------------------------------------------------------ region text

/// A region's text laid out: anchor x, each line's baseline, size and text.
struct Placed {
    x: f64,
    lines: Vec<(f64, String)>,
    size: f64,
}

/// Greedy wrap of each written line's words to `maxw` at `size`.
fn wrap(src: &[Vec<String>], maxw: f64, size: f64) -> Vec<String> {
    let mut out = vec![];
    for words in src {
        let mut cur = String::new();
        for w in words {
            let next = if cur.is_empty() { w.clone() } else { format!("{cur} {w}") };
            if !cur.is_empty() && em_width(&next) * size > maxw {
                out.push(std::mem::replace(&mut cur, w.clone()));
            } else {
                cur = next;
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

/// The box of `lines` centred on `at` at `size`, and each line's baseline.
fn block(at: Pt, lines: &[String], size: f64) -> (Bx, Vec<f64>) {
    let lh = size * 1.2;
    let k = lines.len() as f64;
    let ys: Vec<f64> = (0..lines.len()).map(|i| at.1 + size / 3.0 - (k - 1.0) * lh / 2.0 + i as f64 * lh).collect();
    let w = lines.iter().map(|l| em_width(l) * size).fold(0.0, f64::max);
    ((at.0 - w / 2.0, ys[0] - 0.76 * size, at.0 + w / 2.0, ys[ys.len() - 1] + descent(&lines[lines.len() - 1]) * size), ys)
}

/// Lays out a region's text (`src`: its written lines, each a list of words): at the largest size from `size0` down to 9
/// that fits, at the first of the region's places it fits, on as few lines as fit; failing that, as many lines as fit at
/// size 9 and an ellipsis.
fn place(g: &Geo, mask: usize, anchors: &[Pt], src: &[Vec<String>], size0: f64, avoid: &[Bx]) -> Option<Placed> {
    if src.iter().all(|l| l.is_empty()) {
        return None;
    }
    // each place as given, then nudged a little each way (a small region may have room just beside its middle)
    let nudges = [(0.0, 0.0), (0.0, -8.0), (0.0, 8.0), (-8.0, 0.0), (8.0, 0.0), (0.0, -16.0), (0.0, 16.0), (-14.0, 0.0), (14.0, 0.0)];
    let spots: Vec<Pt> = nudges.iter().flat_map(|d| anchors.iter().map(move |a| (a.0 + d.0, a.1 + d.1))).collect();
    let mut sizes = vec![size0];
    while *sizes.last().unwrap() > 9.0 {
        sizes.push((sizes.last().unwrap() * 0.92).max(9.0));
    }
    let widths = |size: f64| -> Vec<f64> {
        let natural = src.iter().map(|l| em_width(&l.join(" ")) * size).fold(0.0, f64::max);
        let widest_word = src.iter().flatten().map(|w| em_width(w) * size).fold(0.0, f64::max);
        std::iter::once(natural).chain((1..=14).map(|i| natural * (1.0 - i as f64 / 16.0))).filter(|w| *w >= widest_word - 1e-9).collect()
    };
    for &size in &sizes {
        let ws = widths(size);
        for at in &spots {
            for &maxw in &ws {
                let lines = wrap(src, maxw + 1e-6, size);
                let (b, ys) = block(*at, &lines, size);
                if g.fits(mask, &b, avoid) {
                    return Some(Placed { x: at.0, lines: ys.into_iter().zip(lines).collect(), size });
                }
            }
        }
    }
    // too much to fit: at the smallest size, the most of it that fits anywhere, cut short with an ellipsis
    let size = 9.0;
    let mut best: Option<(usize, Placed)> = None;
    for at in &spots {
        for maxw in widths(size).into_iter().chain((2..=30).rev().map(|i| i as f64 * 8.0)) {
            let all = wrap(src, maxw + 1e-6, size);
            for k in (1..=all.len()).rev() {
                let mut v: Vec<String> = all[..k].to_vec();
                let mut last: Vec<char> = v[k - 1].chars().collect();
                let cut = k < all.len();
                loop {
                    let tail: String = last.iter().collect();
                    v[k - 1] = if cut || last.len() < all[k - 1].chars().count() { format!("{}\u{2026}", tail.trim_end_matches([',', ' '])) } else { tail };
                    let (b, ys) = block(*at, &v, size);
                    if g.fits(mask, &b, avoid) {
                        let got: usize = v.iter().map(|l| l.chars().count()).sum();
                        if best.as_ref().is_none_or(|x| got > x.0) {
                            best = Some((got, Placed { x: at.0, lines: ys.into_iter().zip(v.clone()).collect(), size }));
                        }
                        break;
                    }
                    if last.is_empty() {
                        break;
                    }
                    last.pop();
                }
            }
        }
    }
    best.map(|b| b.1)
}

/// The members typed for a set: commas between, ranges such as 1-20 written out, empty entries left out.
fn members(text: &str) -> Vec<String> {
    let mut out = vec![];
    for t in text.split(',') {
        let t = t.trim();
        if t.is_empty() {
            continue;
        }
        let range = t.split_once('-').and_then(|(a, b)| Some((a.trim().parse::<i64>().ok()?, b.trim().parse::<i64>().ok()?))).filter(|(a, b)| b >= a && b - a < MAX_MEMBERS as i64);
        match range {
            Some((a, b)) if !t.starts_with('-') => out.extend((a..=b).map(|v| v.to_string())),
            _ => out.push(t.chars().take(24).collect()),
        }
        if out.len() >= MAX_MEMBERS {
            out.truncate(MAX_MEMBERS);
            break;
        }
    }
    out
}

/// Sets mode: each member in the region of the sets that list it (a member listed twice in a set is two members, the
/// second copy in the sets that list it at least twice), members of the universal set in no set outside the circles;
/// numbers in order, other members in the order first typed. Second: members that land in a region the arrangement does not draw.
fn sorted(p: &Params, g: &Geo) -> ([Vec<String>; 8], Vec<String>) {
    let lists: Vec<Vec<String>> = [p.s("a_set"), p.s("b_set"), p.s("c_set")].iter().take(g.n).map(|t| members(t)).collect();
    let u = members(p.s("u_set"));
    let mut seen: Vec<String> = vec![];
    for v in lists.iter().flatten().chain(u.iter()) {
        if !seen.contains(v) {
            seen.push(v.clone());
        }
    }
    let mut out: [Vec<String>; 8] = Default::default();
    let mut lost = vec![];
    let drawn = g.masks();
    for v in &seen {
        let counts: Vec<usize> = lists.iter().map(|l| l.iter().filter(|x| *x == v).count()).collect();
        let cu = u.iter().filter(|x| *x == v).count();
        let most = counts.iter().copied().max().unwrap_or(0).max(cu);
        for j in 1..=most {
            let mask: usize = counts.iter().enumerate().filter(|(_, c)| **c >= j).map(|(i, _)| 1 << i).sum();
            if drawn.contains(&mask) {
                out[mask].push(v.clone());
            } else {
                lost.push(v.clone());
            }
        }
    }
    for r in out.iter_mut() {
        if r.iter().all(|v| v.parse::<f64>().is_ok_and(f64::is_finite)) {
            r.sort_by(|a, b| a.parse::<f64>().unwrap().partial_cmp(&b.parse::<f64>().unwrap()).unwrap_or(std::cmp::Ordering::Equal));
        }
    }
    (out, lost)
}

/// The value typed for each drawn region (by mask) in Text mode, each the slot in `counts` its place in the typing order
/// gives; None when there are more values than a two- or three-set overlapping diagram has regions (the message it showed).
fn typed(p: &Params, g: &Geo) -> Option<[String; 8]> {
    let given: Vec<String> = p.s("counts").split(',').map(|s| s.trim().chars().take(MAX_TEXT).collect()).collect();
    if matches!(g.arr, Arr::Over2 | Arr::Over3) && given.len() > g.regions.len() {
        return None;
    }
    let mut vals: [String; 8] = Default::default();
    for (v, (m, _)) in given.into_iter().zip(&g.regions) {
        vals[*m] = v;
    }
    Some(vals)
}

/// Writes region `slot`'s text into `counts`; commas in it start new lines (stored as semicolons).
fn set_slot(p: &mut Params, slot: usize, text: &str) {
    let mut v: Vec<String> = p.s("counts").split(',').map(|s| s.trim().to_string()).collect();
    if v.len() == 1 && v[0].is_empty() {
        v.clear();
    }
    while v.len() <= slot {
        v.push(String::new());
    }
    v[slot] = text.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("; ");
    while v.last().is_some_and(|s| s.is_empty()) {
        v.pop();
    }
    p.set_s("counts", &v.join(", "));
}

// ------------------------------------------------------------------------------------------------ shading

/// The regions a typed shading names, as a bit set over masks, within `all` (the drawn regions). Commas join parts;
/// a part is a set expression of A, B, C (or the sets' names), and, or, not, only, minus, outside, everything, none and
/// brackets (also the symbols ∩ ∪ ' and \). A part that does not read is left out.
pub(crate) fn parse_shade(text: &str, names: &[String], all: u8) -> u8 {
    text.split(',').map(|part| Expr::new(part, names, all).read().unwrap_or(0)).fold(0, |a, b| a | b) & all
}

struct Expr {
    toks: Vec<String>,
    i: usize,
    names: Vec<String>,
    all: u8,
}

impl Expr {
    fn new(s: &str, names: &[String], all: u8) -> Expr {
        let mut toks = vec![];
        let mut cur = String::new();
        for c in s.chars() {
            if c.is_alphanumeric() {
                cur.extend(c.to_lowercase());
                continue;
            }
            if !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
            match c {
                '(' | ')' | '\'' | '\u{2019}' | '\u{2032}' | '\u{2229}' | '\u{222a}' | '\\' | '\u{2216}' | '&' | '|' | '-' | '+' => toks.push(c.to_string()),
                _ => {}
            }
        }
        if !cur.is_empty() {
            toks.push(cur);
        }
        Expr { toks, i: 0, names: names.iter().map(|n| n.to_lowercase()).collect(), all }
    }

    fn peek(&self) -> Option<&str> { self.toks.get(self.i).map(|s| s.as_str()) }

    /// The whole part, or None when it does not read to its end.
    fn read(mut self) -> Option<u8> {
        let v = self.union()?;
        (self.i == self.toks.len()).then_some(v)
    }

    fn union(&mut self) -> Option<u8> {
        let mut v = self.inter()?;
        loop {
            match self.peek() {
                Some("or" | "\u{222a}" | "|" | "+" | "union") => {
                    self.i += 1;
                    v |= self.inter()?;
                }
                Some("minus" | "\\" | "\u{2216}" | "-" | "without" | "but") => {
                    self.i += 1;
                    if self.peek() == Some("not") {
                        self.i += 1;
                    }
                    v &= !self.inter()? & self.all;
                }
                _ => return Some(v),
            }
        }
    }

    /// Sets joined by and; a trailing `only` keeps the regions of no other set than those named (`A and B only`).
    fn inter(&mut self) -> Option<u8> {
        let (mut v, mut sets) = self.unary()?;
        while matches!(self.peek(), Some("and" | "\u{2229}" | "&" | "intersect")) {
            self.i += 1;
            let (w, s) = self.unary()?;
            v &= w;
            sets |= s;
        }
        if self.peek() == Some("only") {
            self.i += 1;
            v &= (0..8u8).filter(|m| (*m as usize) & !sets == 0).fold(0, |a, m| a | 1 << m);
        }
        Some(v)
    }

    /// A set, its complement (`not A`, `A'`), with the sets it names.
    fn unary(&mut self) -> Option<(u8, usize)> {
        if matches!(self.peek(), Some("not")) {
            self.i += 1;
            let (v, sets) = self.unary()?;
            return Some((!v & self.all, sets));
        }
        let (mut v, sets) = self.atom()?;
        while matches!(self.peek(), Some("'" | "\u{2019}" | "\u{2032}")) {
            self.i += 1;
            v = !v & self.all;
        }
        Some((v, sets))
    }

    /// One set, a bracket, or a word for a part of the diagram; with the sets it names (for `only`).
    fn atom(&mut self) -> Option<(u8, usize)> {
        let t = self.peek()?.to_string();
        self.i += 1;
        let set = |i: usize| (0..8u8).filter(|m| m & (1 << i) != 0).fold(0u8, |a, m| a | 1 << m);
        let named = self.names.iter().position(|n| *n == t);
        Some(match t.as_str() {
            "(" => {
                let start = self.i;
                let v = self.union()?;
                (self.peek() == Some(")")).then_some(())?;
                let sets = self.toks[start..self.i].iter().map(|x| match x.as_str() { "a" => 1, "b" => 2, "c" => 4, _ => 0 }).fold(0, |a, b| a | b);
                self.i += 1;
                (v, if sets == 0 { 0b111 } else { sets })
            }
            "a" | "b" | "c" => {
                let i = (t.as_bytes()[0] - b'a') as usize;
                (set(i) & self.all, 1 << i)
            }
            _ if named.is_some() => {
                let i = named.unwrap();
                (set(i) & self.all, 1 << i)
            }
            "outside" | "neither" | "nothing" if t != "nothing" => (1, 0),
            "both" => (1 << 3, 0b011),
            "all" if self.peek() == Some("three") => {
                self.i += 1;
                (1 << 7, 0b111)
            }
            "everything" | "all" | "u" | "universe" | "\u{3be}" | "\u{3b5}" => (self.all, 0b111),
            "none" | "nothing" | "empty" => (0, 0b111),
            _ => return None,
        })
    }
}

/// The shading after a click on region `mask` (on if it was off, off if on), written as one of the familiar shadings
/// when it is one, else as the regions in typing order.
fn toggled(g: &Geo, cur: &str, mask: usize) -> String {
    let all = g.all();
    let now = parse_shade(cur, &g.names, all) ^ (1 << mask);
    if now == 0 {
        return "None".to_string();
    }
    if let Some(name) = PRESETS.iter().find(|s| parse_shade(s, &g.names, all) == now) {
        return name.to_string();
    }
    g.masks().into_iter().filter(|m| now & (1 << m) != 0).map(|m| g.region_code(m)).collect::<Vec<_>>().join(", ")
}

/// The paint for shaded regions, the pattern defs it needs, and whether text on it needs a white halo.
fn paint(p: &Params, fills: &Fills) -> (String, &'static str, bool) {
    let pattern = |i: usize| (format!("url(#vn-pat{i})"), PATTERNS, true);
    match p.s("fill") {
        "Hatching" => pattern(0),
        "Cross-hatching" => pattern(1),
        "Dots" => pattern(2),
        _ if fills.is_print() => (fills.series(0), "", false),
        "Strong blue" => (fills.series(0), "", false),
        // 30 % of the series colours over white: black text on any of them is over 10:1
        "Orange" => ("#f6d9c5".into(), "", false),
        "Green" => ("#c9e4d3".into(), "", false),
        "Red" => ("#efc8ca".into(), "", false),
        "Purple" => ("#dcd0ed".into(), "", false),
        "Grey" => ("#d9d9d9".into(), "", false),
        _ => ("#cad8f2".into(), "", false),
    }
}

/// Black-and-white patterns (grey ink, so they print in Print style too).
const PATTERNS: &str = r##"<defs><pattern id="vn-pat0" width="9" height="9" patternUnits="userSpaceOnUse"><rect width="9" height="9" fill="#fff"/><path d="M-2,2 l4,-4 M0,9 l9,-9 M7,11 l4,-4" stroke="#444" stroke-width="1.3" fill="none"/></pattern><pattern id="vn-pat1" width="9" height="9" patternUnits="userSpaceOnUse"><rect width="9" height="9" fill="#fff"/><path d="M-2,2 l4,-4 M0,9 l9,-9 M7,11 l4,-4 M-2,7 l4,4 M0,0 l9,9 M7,-2 l4,4" stroke="#444" stroke-width="1.1" fill="none"/></pattern><pattern id="vn-pat2" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="#fff"/><circle cx="2" cy="2" r="1.3" fill="#444"/><circle cx="6" cy="6" r="1.3" fill="#444"/></pattern></defs>"##;

// ------------------------------------------------------------------------------------------------ drawing

fn clip_defs(c: &[(f64, f64, f64)], rect: (f64, f64, f64, f64)) -> String {
    let mut s = String::new();
    for (i, (cx, cy, r)) in c.iter().enumerate() {
        let (cx, cy, r) = (*cx, *cy, *r);
        let ring = format!("M{:.2},{:.2} a{r},{r} 0 1,0 {:.2},0 a{r},{r} 0 1,0 {:.2},0 Z", cx - r, cy, 2.0 * r, -2.0 * r);
        s.push_str(&format!(r##"<clipPath id="vn-in{i}"><path d="{ring}"/></clipPath>"##));
        s.push_str(&format!(
            r##"<clipPath id="vn-out{i}"><path clip-rule="evenodd" d="M{:.2},{:.2} h{:.2} v{:.2} h{:.2} Z {ring}"/></clipPath>"##,
            rect.0, rect.1, rect.2, rect.3, -rect.2
        ));
    }
    s
}

/// Text with a white halo (on a patterned fill).
fn halo_text(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}" stroke="#fff" stroke-width="4.5" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        esc(s)
    ));
}

fn render(p: &Params) -> String {
    let g = Geo::new(p);
    let fills = Fills::new(p);
    let answers = p.b(SHOW_ANSWERS);
    let fs = g.fs;
    let title = p.s("title").trim();
    let sets_mode = p.s("mode") == "Sets";
    let n = g.n;

    // what goes in each region (by mask), and the counts the totals add
    let (vals, nums, lost): ([String; 8], Option<Vec<f64>>, Vec<String>) = if sets_mode {
        let (items, lost) = sorted(p, &g);
        let nums = items.iter().map(|v| v.len() as f64).collect();
        let vals = items.map(|v| {
            let sep = if v.iter().any(|x| x.contains(' ')) { "; " } else { " " };
            v.join(sep)
        });
        (vals, Some(nums), lost)
    } else {
        let Some(vals) = typed(p, &g) else {
            return chart::message(&format!("{n} sets have {} regions; there are {} values", g.regions.len(), p.s("counts").split(',').count()));
        };
        let nums = (0..8).map(|m| if m >= (1 << n) || vals[m].is_empty() { Some(0.0) } else { vals[m].parse::<f64>().ok().filter(|v| v.is_finite()) }).collect();
        (vals, nums, vec![])
    };

    let totals = p.b("totals");
    let rows = totals_rows(&g, &nums);
    let notes = usize::from(!lost.is_empty());
    let (w, h) = (g.w, g.h(rows.len(), totals, notes));
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.2, fs * 1.3, "middle", title);
    }
    let rect = g.rect_xywh;
    d.raw(&clip_defs(&g.circles, rect));
    // shading: each shaded region is the universe clipped to the circles it is in and out of
    let shaded = parse_shade(p.s("shade"), &g.names, g.all());
    let (fill, pats, halo) = paint(p, &fills);
    if shaded != 0 && !pats.is_empty() {
        d.raw(pats);
    }
    for mask in 0..(1usize << n) {
        if shaded & (1 << mask) == 0 {
            continue;
        }
        let (mut open, mut close) = (String::new(), String::new());
        for i in 0..n {
            open.push_str(&format!(r##"<g clip-path="url(#vn-{}{i})">"##, if mask & (1 << i) != 0 { "in" } else { "out" }));
            close.push_str("</g>");
        }
        d.raw(&format!(r##"{open}<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>{close}"##, rect.0, rect.1, rect.2, rect.3, fill));
    }
    d.rect(rect.0, rect.1, rect.2, rect.3, "none", INK, 2.0);
    for (cx, cy, r) in &g.circles {
        d.circle(*cx, *cy, *r, "none", INK, 2.4);
    }
    // on a patterned fill every label gets a white halo
    let pat = halo && shaded != 0;
    if let Some((t, size, at, a)) = &g.ulabel {
        if pat { halo_text(&mut d, at.0, at.1, *size, a, t) } else { chart::text(&mut d, at.0, at.1, *size, a, t) }
    }
    for (i, (x, y, a, size)) in g.name_at.iter().enumerate() {
        if pat { halo_text(&mut d, *x, *y, *size, a, &g.names[i]) } else { chart::text(&mut d, *x, *y, *size, a, &g.names[i]) }
    }
    let mut avoid = g.name_boxes();
    avoid.extend(g.ulabel_box());
    let size0 = fs * 1.05;
    let blank = p.s("empty") == "A blank line";
    for (m, anchors) in &g.regions {
        let text = if answers { vals[*m].as_str() } else { "" };
        let src: Vec<Vec<String>> = text.split(';').map(|l| l.split_whitespace().map(str::to_string).collect::<Vec<_>>()).filter(|l| !l.is_empty()).collect();
        let on_pattern = halo && shaded & (1 << m) != 0;
        match place(&g, *m, anchors, &src, size0, &avoid) {
            // one line at full size where it always went: drawn exactly as before
            Some(pl) if pl.lines.len() == 1 && pl.size == size0 && pl.x == anchors[0].0 && !on_pattern && (pl.lines[0].0 - (anchors[0].1 + size0 / 3.0)).abs() < 1e-9 => {
                chart::text(&mut d, anchors[0].0, anchors[0].1 + fs * 0.35, size0, "middle", &pl.lines[0].1);
            }
            Some(pl) => {
                for (y, l) in &pl.lines {
                    if on_pattern {
                        halo_text(&mut d, pl.x, *y, pl.size, "middle", l);
                    } else {
                        d.raw(&format!(
                            r##"<text x="{:.2}" y="{y:.2}" font-size="{:.2}" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
                            pl.x,
                            pl.size,
                            esc(l)
                        ));
                    }
                }
            }
            None if blank => {
                // a line to write on, as long as fits the region (at most 40 units at medium labels)
                for at in anchors {
                    let y = at.1 + fs * 0.35;
                    let fit = (4..=10).rev().map(|i| i as f64 * 4.0 * fs / 15.0).find(|l| g.fits(*m, &(at.0 - l / 2.0, y - 4.0, at.0 + l / 2.0, y + 4.0), &avoid));
                    if let Some(l) = fit {
                        let (stroke, sw) = if on_pattern { ("#fff", 4.0) } else { ("none", 0.0) };
                        if sw > 0.0 {
                            d.line(at.0 - l / 2.0, y, at.0 + l / 2.0, y, stroke, sw);
                        }
                        d.line(at.0 - l / 2.0, y, at.0 + l / 2.0, y, INK, 1.2);
                        break;
                    }
                }
            }
            None => {}
        }
    }
    let base = g.top + g.bottom;
    if totals {
        // (as before, unless a long name's label would run into its value)
        let old_w = rows.iter().map(|(l, _)| l.chars().count()).max().unwrap_or(8) as f64 * fs * 0.5 + 6.0;
        let ink_w = rows.iter().map(|(l, _)| em_width(l) * fs).fold(0.0, f64::max);
        let lab_w = if ink_w + 3.0 > old_w { ink_w + 8.0 } else { old_w };
        for (i, (l, v)) in rows.iter().enumerate() {
            let y = base + 14.0 + fs * 1.7 * (i as f64 + 0.8);
            chart::text(&mut d, 28.0, y, fs, "start", l);
            match (answers, v) {
                (true, Some(v)) => chart::text(&mut d, 28.0 + lab_w, y, fs, "start", v),
                (true, None) => {}
                _ => {
                    d.line(28.0 + lab_w, y + 2.0, 28.0 + lab_w + 90.0, y + 2.0, INK, 1.0);
                }
            }
        }
    }
    if notes > 0 && answers {
        let what = match g.arr {
            Arr::Separate => "the sets are drawn apart",
            _ => "B is drawn inside A",
        };
        let mut l: Vec<String> = lost.clone();
        l.dedup();
        let shown: String = l.join(", ").chars().take(40).collect();
        let y = h - fs * 0.8;
        chart::text(&mut d, 28.0, y, fs * 0.85, "start", &format!("Not drawn ({what}): {shown}"));
    }
    d.finish()
}

/// The totals underneath: label and value (None when a region's value is not a number).
fn totals_rows(g: &Geo, nums: &Option<Vec<f64>>) -> Vec<(String, Option<String>)> {
    let n = g.n;
    let names = &g.names;
    let tot = |f: &dyn Fn(usize) -> bool| -> Option<String> { nums.as_ref().map(|v| chart::fmt(((0..(1usize << n)).filter(|m| f(*m)).map(|m| v[m]).sum::<f64>() * 1000.0).round() / 1000.0)) };
    match n {
        1 => vec![(format!("n({}) =", names[0]), tot(&|m| m & 1 != 0)), ("n(all) =".to_string(), tot(&|_| true))],
        2 => vec![
            (format!("n({}) =", names[0]), tot(&|m| m & 1 != 0)),
            (format!("n({}) =", names[1]), tot(&|m| m & 2 != 0)),
            (format!("n({} and {}) =", names[0], names[1]), tot(&|m| m & 3 == 3)),
            (format!("n({} or {}) =", names[0], names[1]), tot(&|m| m & 3 != 0)),
            ("n(all) =".to_string(), tot(&|_| true)),
        ],
        _ => vec![
            (format!("n({}) =", names[0]), tot(&|m| m & 1 != 0)),
            (format!("n({}) =", names[1]), tot(&|m| m & 2 != 0)),
            (format!("n({}) =", names[2]), tot(&|m| m & 4 != 0)),
            ("n(all three) =".to_string(), tot(&|m| m == 7)),
            ("n(all) =".to_string(), tot(&|_| true)),
        ],
    }
}

// ------------------------------------------------------------------------------------------------ on the figure

/// One click area over the universal box: a click in a region shades it or clears it, a double-click types what goes in it
/// (Text mode), and the hover outlines the region under the pointer and says so. Each set's name is renamed in place by a
/// double-click.
pub fn handles(p: &Params) -> Vec<Handle> {
    let g = Arc::new(Geo::new(p));
    let (x0, y0, x1, y1) = g.rect;
    let sets_mode = p.s("mode") == "Sets";
    let (g1, g2, g3) = (g.clone(), g.clone(), g.clone());
    let area = Handle::area(((x0 + x1) / 2.0, (y0 + y1) / 2.0), (x1 - x0, y1 - y0), "Click a region to shade it (again to clear it); double-click a region to type what goes in it", move |p, q| {
        if let Some(m) = g1.mask_at(q).filter(|m| g1.masks().contains(m)) {
            let s = toggled(&g1, p.s("shade"), m);
            p.set_s("shade", &s);
        }
    })
    .with_hover(move |q| {
        let m = g2.mask_at(q).filter(|m| g2.masks().contains(m))?;
        let type_it = if sets_mode { "" } else { ", double-click to type what goes in it" };
        Some(Hover { outline: g2.outline(m), closed: true, tip: format!("{}: click to shade it (again to clear it){type_it}", capital(&g2.region_name(m))) })
    })
    .with_edit_at("counts", move |p, q| {
        if p.s("mode") == "Sets" {
            return None;
        }
        let m = g3.mask_at(q)?;
        let slot = g3.regions.iter().position(|r| r.0 == m)?;
        let text = p.s("counts").split(',').nth(slot).map(|s| s.trim().replace(';', ",").split(',').map(str::trim).collect::<Vec<_>>().join(", ")).unwrap_or_default();
        Some(EditAt { text, hint: format!("{}: commas start new lines", capital(&g3.region_name(m))), set: Arc::new(move |p: &mut Params, s: &str| set_slot(p, slot, s)) })
    });
    let mut v = vec![area];
    for (i, b) in g.name_boxes().iter().enumerate() {
        let key = ["a_name", "b_name", "c_name"][i];
        v.push(Handle::label(((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0), (b.2 - b.0 + 4.0, b.3 - b.1 + 2.0), key, &format!("Double-click to rename set {}", ["A", "B", "C"][i])));
    }
    v
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

#[cfg(test)]
pub(crate) mod tests;
