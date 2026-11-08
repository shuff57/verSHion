//! Pie chart (circle graph): one sector per category, running clockwise from 12 o'clock, from a
//! typed or pasted table of counts, percents or degrees. A row with no value is an empty sector:
//! it takes whatever share of the circle the others leave (counts need the total typed), and the
//! student copy draws it blank for students to fill in. Sector labels (name, value, percent,
//! angle, fraction, or a name with one number), a key on the right or below, a title or a blank
//! title line, colour, black-and-white or white sectors with a fill picked per sector.
//!
//! The data is typed text, one `name, value` per line (a spreadsheet paste is tab separated and
//! works), read with the shared `figures::data` splitter. It is not `Kind::Data`: that control
//! tidies a paste down to numbers and would wipe the names.
//!
//! Original design on the shared pieces (`Fills`, `show_answers`, `label_size`).
use crate::figures::data;
use crate::figures::stats::bar::tw;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "pie-chart";
/// Most sectors drawn.
pub const MAX_CATS: usize = 20;
/// Longest category name or label, in characters.
pub const MAX_NAME: usize = 40;
pub const MAX_TITLE: usize = 120;

pub const VALUES: [&str; 3] = ["Counts", "Percents", "Degrees"];
pub const ORDERS: [&str; 2] = ["As typed", "Largest first"];
pub const LABELS: [&str; 11] = [
    "None", "Name", "Value", "Percent", "Angle", "Fraction", "Name and value", "Name and percent", "Name and angle", "Name and fraction", "Value and percent",
];
pub const LOOKS: [&str; 3] = ["Color", "Black and white", "White"];
pub const KEYS: [&str; 3] = ["None", "Right", "Below"];
pub const TITLES: [&str; 3] = ["None", "Chart title", "Blank line"];

const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const INK: &str = "#111827";
const R: f64 = 150.0;
/// Black-and-white fills, in turn: a light wash, hatching, dots, cross-hatching, back hatching, a
/// mid grey. Hatch and dots are patterns (`pie-p1`..`pie-p4`); the two greys are plain fills.
const BW_WASH: &str = "#d4d4d4";
const BW_MID: &str = "#9a9a9a";

pub fn def() -> FigureDef {
    FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Pie Chart",
        blurb: "A pie chart of counts, percents or degrees with labelled sectors and a key: empty sectors for students to draw, black-and-white fills that photocopy, a title or a blank line for one.",
        params,
        render,
    }
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("data", "Categories and values", "Walk, 14\nCar, 12\nBus, 10\nBike, 4")
            .hint("One category per line: its name, a comma (or a tab, from a spreadsheet paste), then its value. Leave the value out and that sector is empty: it takes what the others leave, and the student copy draws it blank. Leave every value out for a blank circle. Sectors run clockwise from 12 o'clock"),
        ParamDef::choice("values", "Values are", &VALUES, "Counts").hint("Counts are shares of the total; percents add to 100; degrees add to 360"),
        ParamDef::text("total", "Total (counts)", "").hint("Blank uses the sum of the counts. Type a bigger total to leave room for the empty sectors"),
        ParamDef::choice("order", "Order", &ORDERS, "As typed"),
        ParamDef::bool("dot", "Centre dot", false),
        ParamDef::choice("labels", "Sector labels", &LABELS, "Name"),
        ParamDef::int("decimals", "Decimals in percents and angles", 0, 3, 1),
        ParamDef::long("own", "A sector's own label", "")
            .hint("One line per sector, in the order typed; blank keeps the chart's label. Type your own text, a letter (x°), _ for a blank line to write on, or - for no label"),
        ParamDef::choice("look", "Fills", &LOOKS, "Color").hint("Black and white uses hatching and greys that photocopy; White leaves every sector for students to shade"),
        ParamDef::list("fill_list", "A sector's own fill", "")
            .hint("Fills in the order typed, separated by commas, blank for the automatic one: Blue, Red, Green, Orange, Purple, Gray, White, Hatching, Dots, Cross-hatching, Back hatching, Black"),
        ParamDef::choice("key", "Key", &KEYS, "None"),
        ParamDef::choice("title_kind", "Title", &TITLES, "None"),
        ParamDef::text("title", "Chart title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off makes the student copy: empty sectors are drawn white and unlabelled for students to fill in; their number labels are left out. Student + key saves both"),
    ]
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        (ID, "total") => p.s("values") == VALUES[0],
        (ID, "title") => p.s("title_kind") == TITLES[1],
        (ID, "fill_list") => p.s("look") != LOOKS[2],
        (ID, "decimals") => matches!(p.s("labels"), "Percent" | "Angle" | "Name and percent" | "Name and angle" | "Value and percent") || p.s("values") != VALUES[0],
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        ID => vec![
            Example { name: "Getting to school", params: vec![] },
            Example {
                name: "Favourite fruit, percents",
                params: vec![
                    ("data", s("Apples, 40\nBananas, 26.7\nCherries, 20\nGrapes, 13.3")),
                    ("values", s("Percents")),
                    ("labels", s("Name and percent")),
                    ("title_kind", s("Chart title")),
                    ("title", s("Favourite fruit")),
                ],
            },
            Example {
                name: "Fractions: how Sam spends a day",
                params: vec![
                    ("data", s("Sleep, 9\nSchool, 6\nHomework, 2\nPlay, 3\nMeals, 2\nOther, 2")),
                    ("labels", s("Name and fraction")),
                    ("total", s("24")),
                    ("look", s("Black and white")),
                    ("title_kind", s("Chart title")),
                    ("title", s("How Sam spends a day")),
                ],
            },
            Example {
                name: "Find the missing angle x",
                params: vec![
                    ("data", s("Red, 110\nBlue, 80\nGreen, 100\nYellow")),
                    ("values", s("Degrees")),
                    ("labels", s("Name and angle")),
                    ("own", s("\n\n\nx°")),
                    ("look", s("Black and white")),
                    ("dot", Value::B(true)),
                ],
            },
            Example {
                name: "Complete the pie chart",
                params: vec![
                    ("data", s("Soccer, 20\nBasketball, 15\nTennis\nOther")),
                    ("total", s("60")),
                    ("labels", s("Name and value")),
                    ("key", s("None")),
                    ("title_kind", s("Chart title")),
                    ("title", s("Favourite sports of 60 students")),
                ],
            },
            Example {
                name: "Blank circle to fill in",
                params: vec![
                    ("data", s("Dog\nCat\nFish\nRabbit")),
                    ("look", s("White")),
                    ("labels", s("None")),
                    ("key", s("Right")),
                    ("title_kind", s("Blank line")),
                    ("dot", Value::B(true)),
                ],
            },
        ],
        _ => return None,
    })
}

// ---- reading the table ----

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub value: Option<f64>,
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn number(c: &str) -> Option<f64> {
    let c = c.trim().trim_end_matches(['%', '\u{b0}']).trim();
    if c.is_empty() {
        return None;
    }
    c.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0 && *v <= 1e12)
}

/// The rows of `text`, and a note for each cell that was not a number (left as an empty sector).
pub fn parse(text: &str) -> (Vec<Row>, Vec<String>) {
    let (mut rows, mut notes) = (Vec::new(), Vec::new());
    for (n, line) in text.lines().map(str::trim).enumerate().filter(|(_, l)| !l.is_empty()) {
        if rows.len() >= MAX_CATS {
            notes.push(format!("Only the first {MAX_CATS} categories are drawn."));
            break;
        }
        let sep = line.contains(['\t', ',', ';']);
        let (name, cell) = if sep {
            let cells = data::parse_table(line).into_iter().next().unwrap_or_default();
            (cells.first().cloned().unwrap_or_default(), cells.get(1).cloned().unwrap_or_default())
        } else if let Some((a, b)) = line.rsplit_once(char::is_whitespace).filter(|(_, b)| number(b).is_some()) {
            (a.trim().to_string(), b.to_string())
        } else {
            (line.to_string(), String::new())
        };
        let value = number(&cell);
        if value.is_none() && !cell.trim().is_empty() {
            notes.push(format!("Line {}: \"{}\" is not a number, so {} is an empty sector.", n + 1, cell.trim(), clip(&name, MAX_NAME)));
        }
        rows.push(Row { name: clip(&name, MAX_NAME), value });
    }
    (rows, notes)
}

/// Sizes worked out from the table: each sector's share of the circle (0 to 1), in typed order.
#[derive(Clone, Debug)]
pub struct Model {
    pub rows: Vec<Row>,
    pub share: Vec<f64>,
    pub blank: Vec<bool>,
    /// What the whole circle stands for in the table's unit (the total, 100 or 360).
    pub whole: f64,
    pub unit: usize,
    pub notes: Vec<String>,
}

pub fn model(p: &Params) -> Model {
    let (rows, mut notes) = parse(p.s("data"));
    let unit = VALUES.iter().position(|v| *v == p.s("values")).unwrap_or(0);
    let blank: Vec<bool> = rows.iter().map(|r| r.value.is_none()).collect();
    let sum: f64 = rows.iter().filter_map(|r| r.value).sum();
    let nb = blank.iter().filter(|b| **b).count();
    let typed_total = number(p.s("total")).filter(|t| *t > 0.0);
    let whole = match unit {
        1 => 100.0,
        2 => 360.0,
        _ => typed_total.unwrap_or(sum),
    };
    let mut share = vec![0.0; rows.len()];
    if whole > 0.0 && !rows.is_empty() {
        let used = if sum > whole * (1.0 + 1e-9) {
            notes.push(format!("The values add to {}, more than the whole ({}), so the sectors are scaled down to fit and none is left empty.", data::fmt_num(sum), data::fmt_num(whole)));
            sum
        } else {
            whole
        };
        for (i, r) in rows.iter().enumerate() {
            if let Some(v) = r.value {
                share[i] = v / used;
            }
        }
        let left = (1.0 - share.iter().sum::<f64>()).max(0.0);
        if nb > 0 && left > 1e-9 && used == whole {
            for i in 0..rows.len() {
                if blank[i] {
                    share[i] = left / nb as f64;
                }
            }
        } else if nb > 0 && used == whole && unit == 0 && typed_total.is_none() {
            notes.push("Type a total, larger than the counts, to leave room for the empty sectors.".into());
        }
    }
    Model { rows, share, blank, whole, unit, notes }
}

pub fn warnings(p: &Params) -> Vec<String> {
    let m = model(p);
    let mut w = m.notes.clone();
    if m.unit != 0 && m.rows.iter().filter_map(|r| r.value).sum::<f64>() > 0.0 && m.blank.iter().all(|b| !*b) {
        let sum: f64 = m.rows.iter().filter_map(|r| r.value).sum();
        if (sum - m.whole).abs() > 1e-6 {
            w.push(format!("The values add to {}, not {}.", data::fmt_num(sum), data::fmt_num(m.whole)));
        }
    }
    w
}

// ---- label text ----

fn num(v: f64, dec: usize) -> String {
    let f = 10f64.powi(dec as i32);
    let r = (v * f).round() / f;
    let mut s = format!("{:.*}", dec, if r == 0.0 { 0.0 } else { r });
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    s
}

/// A share of the circle as a reduced fraction (denominator up to 720), or `None`.
pub fn fraction(share: f64) -> Option<String> {
    if !share.is_finite() || share < 0.0 {
        return None;
    }
    for d in 1..=720u32 {
        let n = (share * d as f64).round();
        if (share * d as f64 - n).abs() < 1e-6 {
            return Some(match (n as u32, d) {
                (0, _) => "0".into(),
                (n, 1) => n.to_string(),
                (n, d) => format!("{n}/{d}"),
            });
        }
    }
    None
}

/// The chart's text for one sector: the lines of its label under `mode`. `computed` is false for
/// an empty sector on a student copy, which keeps only its name.
fn lines_for(m: &Model, i: usize, mode: &str, dec: usize, computed: bool) -> Vec<String> {
    let share = m.share[i];
    let name = m.rows[i].name.clone();
    let value = || -> String {
        let v = m.rows[i].value.unwrap_or(share * m.whole);
        match m.unit {
            1 => format!("{}%", num(v, dec)),
            2 => format!("{}\u{b0}", num(v, dec)),
            _ => num(v, dec),
        }
    };
    let percent = || format!("{}%", num(share * 100.0, dec));
    let angle = || format!("{}\u{b0}", num(share * 360.0, dec));
    let frac = || fraction(share).unwrap_or_else(|| percent());
    let named = |x: String| if computed { vec![name.clone(), x] } else { vec![name.clone()] };
    let only = |x: String| if computed { vec![x] } else { vec![] };
    let mut out = match mode {
        "Name" => vec![name.clone()],
        "Value" => only(value()),
        "Percent" => only(percent()),
        "Angle" => only(angle()),
        "Fraction" => only(frac()),
        "Name and value" => named(value()),
        "Name and percent" => named(percent()),
        "Name and angle" => named(angle()),
        "Name and fraction" => named(frac()),
        "Value and percent" => if computed { vec![value(), percent()] } else { vec![] },
        _ => vec![],
    };
    out.retain(|l| !l.is_empty());
    out
}

// ---- fills ----

#[derive(Clone, Debug)]
struct Paint {
    css: String,
    /// Dark enough for white type, plain light, or a pattern (type sits on a white plate).
    kind: u8,
}
const DARK: u8 = 0;
const LIGHT: u8 = 1;
const PATTERN: u8 = 2;

fn hex_dark(hex: &str) -> bool {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return false;
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0) as f64 / 255.0;
    0.299 * c(0) + 0.587 * c(2) + 0.114 * c(4) < 0.56
}

fn bw(i: usize) -> Paint {
    match i % 6 {
        0 => Paint { css: BW_WASH.into(), kind: LIGHT },
        1 => Paint { css: "url(#pie-p1)".into(), kind: PATTERN },
        2 => Paint { css: "url(#pie-p2)".into(), kind: PATTERN },
        3 => Paint { css: "url(#pie-p3)".into(), kind: PATTERN },
        4 => Paint { css: "url(#pie-p4)".into(), kind: PATTERN },
        _ => Paint { css: BW_MID.into(), kind: LIGHT },
    }
}

fn solid(hex: &str) -> Paint {
    Paint { css: hex.into(), kind: if hex_dark(hex) { DARK } else { LIGHT } }
}

/// The fill named by `tok` (a Fills token typed by the teacher), or `None` when blank/unknown.
fn named(tok: &str, print: bool) -> Option<Paint> {
    let t = tok.trim().to_ascii_lowercase().replace(['-', ' '], "");
    let colour = |hex: &str, i: usize| Some(if print { bw(i) } else { solid(hex) });
    match t.as_str() {
        "blue" => colour("#4f7cd4", 0),
        "red" => colour("#c9494f", 1),
        "green" => colour("#4aa36b", 2),
        "orange" => colour("#e0803c", 3),
        "purple" => colour("#8a62c4", 4),
        "gray" | "grey" => Some(solid(if print { BW_WASH } else { "#9aa0a6" })).map(|p| Paint { kind: LIGHT, ..p }),
        "white" => Some(Paint { css: "#ffffff".into(), kind: LIGHT }),
        "hatching" => Some(bw(1)),
        "dots" => Some(bw(2)),
        "crosshatching" => Some(bw(3)),
        "backhatching" => Some(bw(4)),
        "black" => Some(Paint { css: "#000000".into(), kind: DARK }),
        _ => None,
    }
}

fn patterns() -> String {
    let line = |d: &str| format!(r##"<path d="{d}" stroke="#444" stroke-width="1.3" fill="none"/>"##);
    let fwd = "M-2,2 l4,-4 M0,8 l8,-8 M6,10 l4,-4";
    let back = "M-2,6 l4,4 M0,0 l8,8 M6,-2 l4,4";
    let pats = [
        line(fwd),
        r##"<circle cx="2" cy="2" r="1.3" fill="#444"/><circle cx="6" cy="6" r="1.3" fill="#444"/>"##.to_string(),
        format!("{}{}", line(fwd), line(back)),
        line(back),
    ];
    let mut out = String::new();
    for (i, fg) in pats.iter().enumerate() {
        out.push_str(&format!(r##"<pattern id="pie-p{}" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="#fff"/>{fg}</pattern>"##, i + 1));
    }
    out
}

// ---- drawing ----

struct Placed {
    lines: Vec<String>,
    /// Blank line to write on instead of text.
    blank: bool,
    x: f64,
    y: f64,
    anchor: &'static str,
    /// From the rim, for an outside label.
    leader: Option<((f64, f64), (f64, f64))>,
    plate: bool,
    white: bool,
    w: f64,
}

fn text(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str, ink: &str, bold: bool) {
    let weight = if bold { r#" font-weight="600""# } else { "" };
    d.raw(&format!(r#"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{ink}"{weight}>{}</text>"#, esc(s)));
}

fn render(p: &Params) -> String {
    let m = model(p);
    let key_on = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let (fl, ft, fk) = (14.0 * k, 22.0 * k, 15.0 * k);
    let ink = if print { "#000000" } else { INK };
    let n = m.rows.len();
    let look = p.s("look");
    let dec = p.i("decimals").clamp(0, 3) as usize;
    let mode = p.s("labels");

    // drawn order: typed, or largest first with the empty sectors last
    let mut idx: Vec<usize> = (0..n).collect();
    if p.s("order") == ORDERS[1] {
        idx.sort_by(|&a, &b| m.blank[a].cmp(&m.blank[b]).then(m.share[b].partial_cmp(&m.share[a]).unwrap_or(std::cmp::Ordering::Equal)));
    }
    let own: Vec<String> = p.s("own").lines().map(|l| l.trim().to_string()).collect();
    let picks: Vec<String> = p.s("fill_list").split(',').map(|s| s.trim().to_string()).collect();

    // a paint for every sector (typed index): the auto fill follows the drawn position
    let mut paint: Vec<Paint> = vec![Paint { css: "#ffffff".into(), kind: LIGHT }; n];
    for (pos, &i) in idx.iter().enumerate() {
        let auto = match look {
            "White" => Paint { css: "#ffffff".into(), kind: LIGHT },
            "Black and white" => bw(pos),
            _ if print => bw(pos),
            _ => solid(&fills.series(pos)),
        };
        paint[i] = if look == "White" { auto } else { picks.get(i).and_then(|t| named(t, print || look == "Black and white")).unwrap_or(auto) };
    }
    // an empty sector on the student copy is white, for students to shade
    let shown = |i: usize| -> Paint {
        if m.blank[i] && !key_on { Paint { css: "#ffffff".into(), kind: LIGHT } } else { paint[i].clone() }
    };

    let all_blank = n == 0 || m.blank.iter().all(|b| *b);
    let single = n > 0 && !all_blank && m.share.iter().any(|s| *s > 0.9999);
    let mut starts = vec![0.0; n];
    let mut acc = 0.0;
    for &i in &idx {
        starts[i] = acc;
        acc += m.share[i] * 360.0;
    }

    // labels: inside the sector when they fit, otherwise outside with a short leader
    let mut placed: Vec<Placed> = Vec::new();
    let (cx0, cy0) = (0.0, 0.0);
    let line_h = fl * 1.25;
    if !all_blank {
        for &i in &idx {
            let sh = m.share[i];
            if sh <= 1e-9 {
                continue;
            }
            let mut blank_line = false;
            let lines: Vec<String> = match own.get(i).map(String::as_str) {
                Some("_") => {
                    blank_line = true;
                    vec![]
                }
                Some("-") => vec![],
                Some(t) if !t.is_empty() => vec![clip(t, MAX_NAME)],
                _ => lines_for(&m, i, mode, dec, !m.blank[i] || key_on),
            };
            if lines.is_empty() && !blank_line {
                continue;
            }
            let mid = starts[i] + sh * 180.0; // degrees clockwise from 12
            let (sx, sy) = ((mid.to_radians()).sin(), -(mid.to_radians()).cos());
            let w = if blank_line { 56.0 * k } else { lines.iter().map(|l| tw(l, fl)).fold(0.0, f64::max) };
            let h = if blank_line { line_h } else { lines.len() as f64 * line_h };
            let th = sh * std::f64::consts::TAU;
            let rin = if sh > 0.9999 { 0.0 } else { 0.62 * R };
            let chord = if sh > 0.9999 { 2.0 * R } else { 2.0 * rin * (th / 2.0).sin().abs() };
            let fits = chord >= w + 8.0 && (sh > 0.45 || h + 6.0 <= 0.55 * R) && !(sh > 0.5 && sh < 0.9999 && w > 1.4 * R);
            let sector = shown(i);
            if fits {
                let (x, y) = (cx0 + rin * sx, cy0 + rin * sy);
                let white = sector.kind == DARK;
                placed.push(Placed { lines, blank: blank_line, x, y: y - h / 2.0 + fl, anchor: "middle", leader: None, plate: sector.kind == PATTERN, white, w });
            } else {
                let right = sx >= 0.0;
                let (x0, y0) = (cx0 + R * sx, cy0 + R * sy);
                let (x1, y1) = (cx0 + (R + 12.0) * sx, cy0 + (R + 12.0) * sy);
                let joined = if lines.len() > 1 { vec![lines.join(" ")] } else { lines };
                let w = if blank_line { w } else { tw(&joined[0.min(joined.len().saturating_sub(1))], fl) };
                let hx = x1 + if right { 6.0 } else { -6.0 };
                placed.push(Placed { lines: joined, blank: blank_line, x: hx, y: y1 + fl * 0.35, anchor: if right { "start" } else { "end" }, leader: Some(((x0, y0), (x1, y1))), plate: false, white: false, w });
            }
        }
    }
    // push apart outside labels that would sit on each other, one side at a time
    for right in [true, false] {
        let mut side: Vec<usize> = (0..placed.len()).filter(|&j| placed[j].leader.is_some() && (placed[j].anchor == "start") == right).collect();
        side.sort_by(|&a, &b| placed[a].y.partial_cmp(&placed[b].y).unwrap_or(std::cmp::Ordering::Equal));
        for w in 1..side.len() {
            let min = placed[side[w - 1]].y + line_h;
            if placed[side[w]].y < min {
                placed[side[w]].y = min;
            }
        }
    }

    // canvas: the pie centred at the origin, then the labels, title and key around it
    let (mut lo_x, mut hi_x, mut lo_y, mut hi_y) = (-R, R, -R, R);
    for pl in &placed {
        if pl.leader.is_some() {
            let (a, b) = if pl.anchor == "start" { (pl.x, pl.x + pl.w) } else { (pl.x - pl.w, pl.x) };
            lo_x = f64::min(lo_x, a);
            hi_x = f64::max(hi_x, b);
            lo_y = f64::min(lo_y, pl.y - fl);
            hi_y = f64::max(hi_y, pl.y + fl * 0.4);
        }
    }
    let names: Vec<(usize, String)> = idx.iter().map(|&i| (i, m.rows[i].name.clone())).filter(|(_, s)| !s.is_empty()).collect();
    let key = p.s("key");
    let swatch = 16.0 * k;
    let key_w = names.iter().map(|(_, s)| tw(s, fk)).fold(0.0, f64::max) + swatch + 10.0;
    let pad = 20.0;
    let title_text = if p.s("title_kind") == TITLES[1] { clip(p.s("title").trim(), MAX_TITLE) } else { String::new() };
    let title_blank = p.s("title_kind") == TITLES[2];
    let title_h = if title_text.is_empty() && !title_blank { 0.0 } else { ft + 18.0 };
    let title_w = if title_blank { 280.0 * k } else { tw(&title_text, ft) };

    let mut content_w = hi_x - lo_x;
    let mut key_below_rows: Vec<Vec<(usize, String, f64)>> = Vec::new();
    let key_gap = 28.0;
    if key == KEYS[1] && !names.is_empty() {
        content_w += key_gap + key_w;
    }
    let mut w_total = (content_w + 2.0 * pad).max(title_w + 2.0 * pad);
    if key == KEYS[2] && !names.is_empty() {
        let avail = (w_total - 2.0 * pad).max(300.0);
        let mut row: Vec<(usize, String, f64)> = Vec::new();
        let mut used = 0.0;
        for (i, s) in &names {
            let iw = swatch + 8.0 + tw(s, fk) + 22.0;
            if !row.is_empty() && used + iw > avail {
                key_below_rows.push(std::mem::take(&mut row));
                used = 0.0;
            }
            used += iw;
            row.push((*i, s.clone(), iw));
        }
        if !row.is_empty() {
            key_below_rows.push(row);
        }
        let widest = key_below_rows.iter().map(|r| r.iter().map(|x| x.2).sum::<f64>() - 22.0).fold(0.0, f64::max);
        w_total = w_total.max(widest + 2.0 * pad);
    }
    let key_below_h = if key_below_rows.is_empty() { 0.0 } else { key_below_rows.len() as f64 * (swatch + 10.0) + 14.0 };
    let top = pad + title_h + (-lo_y - R).max(0.0);
    let h_total = top + R + hi_y.max(R) + key_below_h + pad;
    // the pie (and its labels) sit at the left of the content, which is centred on the canvas
    let ox = (w_total - content_w) / 2.0 - lo_x;
    let cx = ox;
    let cy = top + R;

    let mut d = Doc::new(w_total.round(), h_total.ceil());
    let sw_print = 1.6;
    d.defs(&patterns());
    if title_blank {
        let y = pad + ft * 0.9;
        d.line(w_total / 2.0 - title_w / 2.0, y, w_total / 2.0 + title_w / 2.0, y, ink, 1.4);
    } else if !title_text.is_empty() {
        text(&mut d, w_total / 2.0, pad + ft * 0.9, ft, "middle", &title_text, ink, true);
    }

    let outline = if print || look != "Color" { ink } else { "#ffffff" };
    let outline_w = if print || look != "Color" { sw_print } else { 2.0 };
    if all_blank {
        d.circle(cx, cy, R, "#ffffff", ink, sw_print);
        d.line(cx, cy, cx, cy - R, ink, sw_print);
    } else if single {
        let i = (0..n).find(|&i| m.share[i] > 0.9999).unwrap_or(0);
        d.circle(cx, cy, R, &shown(i).css, ink, sw_print);
    } else {
        for &i in &idx {
            let sh = m.share[i];
            if sh <= 1e-9 {
                continue;
            }
            let (a0, a1) = (90.0 - starts[i], 90.0 - starts[i] - sh * 360.0);
            let blank_here = m.blank[i];
            let (s, w) = if blank_here { (ink, sw_print) } else { (outline, outline_w) };
            d.sector(cx, cy, R, a0, a1, &shown(i).css, s, w);
        }
        // empty sectors last, so their outline is not covered by a neighbour
        if m.blank.iter().any(|b| *b) && m.share.iter().zip(&m.blank).any(|(s, b)| *b && *s > 1e-9) {
            d.circle(cx, cy, R, "none", ink, sw_print);
        } else if print || look != "Color" {
            d.circle(cx, cy, R, "none", ink, sw_print);
        }
    }
    if p.b("dot") {
        d.circle(cx, cy, 3.2, ink, ink, 0.0);
    }

    for pl in &placed {
        let (x, y) = (cx + pl.x, cy + pl.y);
        let txt_ink = if pl.white { "#ffffff" } else { ink };
        if let Some(((x0, y0), (x1, y1))) = pl.leader {
            d.line(cx + x0, cy + y0, cx + x1, cy + y1, ink, 1.0);
        }
        if pl.blank {
            let yy = y;
            let (a, b) = match pl.anchor {
                "start" => (x, x + pl.w),
                "end" => (x - pl.w, x),
                _ => (x - pl.w / 2.0, x + pl.w / 2.0),
            };
            d.line(a, yy, b, yy, ink, 1.2);
            continue;
        }
        if pl.plate {
            let w = pl.w + 8.0;
            let hh = pl.lines.len() as f64 * line_h + 2.0;
            d.raw(&format!(r##"<rect x="{:.2}" y="{:.2}" width="{w:.2}" height="{hh:.2}" rx="3" fill="#ffffff" fill-opacity="0.9"/>"##, x - w / 2.0, y - fl - 1.0));
        }
        for (j, l) in pl.lines.iter().enumerate() {
            text(&mut d, x, y + j as f64 * line_h, fl, pl.anchor, l, txt_ink, j == 0 && pl.lines.len() > 1);
        }
    }

    // the key
    let swatch_rect = |d: &mut Doc, x: f64, y: f64, i: usize| {
        d.rect(x, y, swatch, swatch, &shown(i).css, ink, 1.2);
    };
    if key == KEYS[1] && !names.is_empty() {
        let kx = ox + hi_x + key_gap;
        let step = swatch + 12.0;
        let ky = cy - names.len() as f64 * step / 2.0 + 6.0;
        for (j, (i, s)) in names.iter().enumerate() {
            let y = ky + j as f64 * step;
            swatch_rect(&mut d, kx, y, *i);
            text(&mut d, kx + swatch + 10.0, y + swatch * 0.82, fk, "start", s, ink, false);
        }
    } else if !key_below_rows.is_empty() {
        let mut y = cy + hi_y.max(R) + 18.0;
        for row in &key_below_rows {
            let rw = row.iter().map(|x| x.2).sum::<f64>() - 22.0;
            let mut x = (w_total - rw) / 2.0;
            for (i, s, iw) in row {
                swatch_rect(&mut d, x, y, *i);
                text(&mut d, x + swatch + 8.0, y + swatch * 0.82, fk, "start", s, ink, false);
                x += iw;
            }
            y += swatch + 10.0;
        }
    }

    let desc = if all_blank {
        "A blank circle for a pie chart".to_string()
    } else {
        let parts: Vec<String> = idx.iter().map(|&i| format!("{} {}", m.rows[i].name, num(m.share[i] * 100.0, dec))).collect();
        format!("Pie chart, sectors in percent: {}", parts.join(", "))
    };
    d.describe("Pie chart", &desc);
    d.finish()
}

#[cfg(test)]
mod tests;
