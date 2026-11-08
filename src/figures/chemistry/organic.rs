//! Organic structure generator: a straight-chain alkane, alkene, alkyne, alcohol or carboxylic
//! acid, or a benzene/cyclohexane/cyclopentane ring, drawn skeletal (vertices and heteroatoms
//! only), condensed (formula text) or full (every carbon and hydrogen). Original drawing; the
//! shapes follow the usual textbook conventions rather than any site's code.
//!
//! Molecular and condensed formulas are answers, so they are printed only under Show answers; the
//! condensed mode is itself the formula, so it always draws it (it is the figure the teacher chose
//! to show) and the answer key adds the molecular formula and relative mass.

use super::*;
use crate::figures::data::fmt_num;
use crate::figures::{answers_param, svg, FigureDef, ParamDef, Params, Subject, SHOW_ANSWERS};

pub const ID: &str = "organic-structure";

const W: f64 = 560.0;
const H: f64 = 320.0;
const INK: &str = "#111827";
const SKELETONS: [&str; 4] = ["Straight chain", "Benzene ring", "Cyclohexane ring", "Cyclopentane ring"];
const GROUPS: [&str; 5] = ["None (alkane)", "\u{2013}OH (alcohol)", "\u{2013}COOH (carboxylic acid)", "C=C double bond (alkene)", "C\u{2261}C triple bond (alkyne)"];
const MODES: [&str; 3] = ["Skeletal", "Condensed", "Full"];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Organic Structure",
        blurb: "A straight-chain or ring organic molecule drawn skeletal, condensed or with every carbon and hydrogen, with its formula for the key.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("skeleton", "Skeleton", &SKELETONS, "Straight chain"),
        ParamDef::int("carbons", "Carbon atoms in the chain", 1, 12, 4),
        ParamDef::choice("group", "Functional group", &GROUPS, "None (alkane)").hint("For a straight chain; rings keep their own formula"),
        ParamDef::choice("mode", "Draw", &MODES, "Skeletal"),
        ParamDef::bool("ends", "Label the CH\u{2083} ends", true).hint("Skeletal mode only"),
        ParamDef::bool("numbers", "Number the carbons", false),
        ParamDef::text("caption", "Caption", ""),
        answers_param().hint("Adds the molecular formula and relative mass under the structure"),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { svg::with_label_size(build(p), p) }

fn fin(v: f64, lo: f64, hi: f64, d: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { d } }

fn atom_count(p: &Params) -> (usize, usize, usize) {
    match p.s("skeleton") {
        "Benzene ring" => (6, 6, 0),
        "Cyclohexane ring" => (6, 12, 0),
        "Cyclopentane ring" => (5, 10, 0),
        _ => {
            let n = p.i("carbons").clamp(1, 12) as usize;
            match p.s("group") {
                "\u{2013}OH (alcohol)" => (n, if n == 1 { 4 } else { 2 * n + 2 }, 1),
                "\u{2013}COOH (carboxylic acid)" => (n, if n == 1 { 2 } else { 2 * n }, 2),
                "C=C double bond (alkene)" if n >= 2 => (n, 2 * n, 0),
                "C\u{2261}C triple bond (alkyne)" if n >= 2 => (n, 2 * n - 2, 0),
                _ => (n, 2 * n + 2, 0),
            }
        }
    }
}

fn formula(c: usize, h: usize, o: usize) -> String {
    let part = |sym: &str, n: usize| if n == 0 { String::new() } else if n == 1 { sym.into() } else { format!("{sym}{}", sub(n)) };
    format!("{}{}{}", part("C", c), part("H", h), part("O", o))
}

fn sub(n: usize) -> String {
    n.to_string().chars().map(|c| ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'][c.to_digit(10).unwrap_or(0) as usize]).collect()
}

/// Repeating CH₂ units written without a stray "1" or empty brackets.
fn mid(k: usize) -> String {
    match k {
        0 => String::new(),
        1 => "CH₂".into(),
        n => format!("(CH₂){}", sub(n)),
    }
}

/// The condensed formula of a chain.
fn condensed(p: &Params, c: usize, h: usize, o: usize) -> String {
    let n = c.max(1);
    match p.s("skeleton") {
        "Benzene ring" => "C₆H₆".into(),
        "Cyclohexane ring" => "C₆H₁₂".into(),
        "Cyclopentane ring" => "C₅H₁₀".into(),
        _ => match p.s("group") {
            "\u{2013}OH (alcohol)" => if n == 1 { "CH₃OH".into() } else { format!("CH₃{}CH₂OH", mid(n - 2)) },
            "\u{2013}COOH (carboxylic acid)" => if n == 1 { "HCOOH".into() } else { format!("CH₃{}COOH", mid(n - 2)) },
            "C=C double bond (alkene)" if n >= 2 => if n == 2 { "CH₂=CH₂".into() } else { format!("CH₂=CH{}CH₃", mid(n - 3)) },
            "C\u{2261}C triple bond (alkyne)" if n >= 2 => if n == 2 { "CH≡CH".into() } else { format!("CH≡C{}CH₃", mid(n - 3)) },
            _ => if n == 1 { "CH₄".into() } else if n == 2 { "CH₃CH₃".into() } else { format!("CH₃{}CH₃", mid(n - 2)) },
        },
    }
}

/// A note when a chosen group cannot fit the chain.
pub fn warnings(p: &Params) -> Vec<String> {
    let mut out = Vec::new();
    let n = p.i("carbons").clamp(1, 12);
    if p.s("skeleton") == "Straight chain" && n < 2 && matches!(p.s("group"), "C=C double bond (alkene)" | "C\u{2261}C triple bond (alkyne)") {
        out.push("A double or triple bond needs two carbons, so the single bond is drawn.".into());
    }
    if p.s("skeleton") == "Straight chain" && n < 2 && p.s("group") == "\u{2013}COOH (carboxylic acid)" {
        out.push("A one-carbon carboxylic acid is methanoic acid; its carbon has no chain to sit at the end of.".into());
    }
    out
}

#[derive(Clone, Copy, Debug)]
struct V { x: f64, y: f64 }

fn chain_vertices(n: usize) -> Vec<V> {
    let n = n.max(1);
    let step = if n <= 1 { 0.0 } else { (W - 160.0) / (n - 1) as f64 };
    let mid = H / 2.0 - 10.0;
    (0..n).map(|i| V { x: 90.0 + i as f64 * step, y: mid + if i % 2 == 0 { 16.0 } else { -16.0 } }).collect()
}

fn ring_vertices(n: usize) -> Vec<V> {
    let (cx, cy, r) = (W / 2.0, H / 2.0 - 14.0, 82.0);
    (0..n).map(|i| {
        let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * i as f64 / n as f64;
        V { x: cx + r * a.cos(), y: cy + r * a.sin() }
    }).collect()
}

fn bond(d: &mut Doc, a: V, b: V, order: usize) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = (dx * dx + dy * dy).sqrt().max(1e-9);
    let (nx, ny) = (-dy / len, dx / len);
    let off = 4.0;
    let line = |d: &mut Doc, o: f64| { d.line(a.x + nx * o, a.y + ny * o, b.x + nx * o, b.y + ny * o, INK, 2.2); };
    match order {
        2 => { line(d, -off); line(d, off); }
        3 => { line(d, -off); line(d, 0.0); line(d, off); }
        _ => line(d, 0.0),
    }
}

fn label(d: &mut Doc, x: f64, y: f64, s: &str, size: f64) {
    d.raw(&format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}" font-weight="bold">{}</text>"##, crate::figures::svg::esc(s)));
}

/// A direction in degrees, away from the bonds already at this atom.
fn free_dirs(occupied: &[f64], want: usize) -> Vec<f64> {
    let mut out = Vec::new();
    let mut best: Vec<(f64, f64)> = (0..8).map(|k| {
        let a = k as f64 * 45.0;
        let clearance = occupied.iter().map(|o| {
            let mut dd = (a - o).rem_euclid(360.0);
            if dd > 180.0 { dd = 360.0 - dd; }
            dd
        }).fold(360.0, f64::min);
        (clearance, a)
    }).collect();
    best.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)));
    for (clear, a) in best {
        if out.len() >= want { break; }
        if clear > 30.0 { out.push(a); }
    }
    out
}

fn dir_deg(a: V, b: V) -> f64 { (b.y - a.y).atan2(b.x - a.x).to_degrees() }

fn draw_chain(p: &Params, d: &mut Doc, v: &[V], full: bool) {
    let group = p.s("group");
    let n = v.len();
    // carbon-carbon bonds; the chosen group changes the first bond
    for i in 0..n.saturating_sub(1) {
        let order = if i == 0 {
            match group { "C=C double bond (alkene)" => 2, "C\u{2261}C triple bond (alkyne)" => 3, _ => 1 }
        } else { 1 };
        bond(d, v[i], v[i + 1], order);
    }
    let last = v[n - 1];
    let mut occupied_last = vec![];
    if n >= 2 { occupied_last.push(dir_deg(last, v[n - 2])); }
    match group {
        "\u{2013}OH (alcohol)" => {
            let dir = if n >= 2 { dir_deg(last, v[n - 2]) + 120.0 } else { -60.0 };
            let r = 48.0;
            let (ox, oy) = (last.x + r * dir.to_radians().cos(), last.y + r * dir.to_radians().sin());
            d.line(last.x, last.y, ox, oy, INK, 2.2);
            label(d, ox + 14.0, oy + 5.0, "OH", 18.0);
        }
        "\u{2013}COOH (carboxylic acid)" => {
            let up = V { x: last.x, y: last.y - 52.0 };
            let right = V { x: last.x + 52.0, y: last.y + 4.0 };
            bond(d, last, up, 2);
            label(d, up.x + 14.0, up.y + 5.0, "O", 18.0);
            d.line(last.x, last.y, right.x, right.y, INK, 2.2);
            label(d, right.x + 16.0, right.y + 5.0, "OH", 18.0);
            occupied_last.push(-90.0);
            occupied_last.push(0.0);
        }
        _ => {}
    }

    if full {
        for (i, a) in v.iter().enumerate() {
            let mut occ = Vec::new();
            if i > 0 { occ.push(dir_deg(*a, v[i - 1])); }
            if i + 1 < n { occ.push(dir_deg(*a, v[i + 1])); }
            let mut h = if i == 0 || i + 1 == n { 3 } else { 2 };
            // the group's attachments take hydrogens from the last carbon
            if i + 1 == n {
                match group {
                    "\u{2013}OH (alcohol)" => { h = if n == 1 { 3 } else { 2 }; occ.push(-60.0); }
                    "\u{2013}COOH (carboxylic acid)" => { h = if n == 1 { 1 } else { 0 }; occ.push(-90.0); occ.push(0.0); }
                    "C=C double bond (alkene)" if n >= 2 && i == 0 => h = 2,
                    "C=C double bond (alkene)" if n >= 2 && i == 1 => h = 1,
                    "C\u{2261}C triple bond (alkyne)" if n >= 2 && i == 0 => h = 1,
                    "C\u{2261}C triple bond (alkyne)" if n >= 2 && i == 1 => h = 0,
                    _ => {}
                }
            }
            if i == 0 {
                match group {
                    "C=C double bond (alkene)" if n >= 2 => h = 2,
                    "C\u{2261}C triple bond (alkyne)" if n >= 2 => h = 1,
                    _ => {}
                }
            }
            label(d, a.x, a.y + 7.0, "C", 18.0);
            for dir in free_dirs(&occ, h) {
                let r = 27.0;
                let (hx, hy) = (a.x + r * dir.to_radians().cos(), a.y + r * dir.to_radians().sin());
                d.line(a.x, a.y, hx, hy, INK, 1.6);
                label(d, hx + 8.0 * dir.to_radians().cos(), hy + 8.0 * dir.to_radians().sin() + 5.0, "H", 15.0);
            }
        }
    } else {
        if p.b("ends") && n > 1 {
            let left = v[0];
            label(d, left.x - 24.0, left.y + 5.0, "CH₃", 15.0);
            if group == "None (alkane)" || group.is_empty() {
                label(d, last.x + 24.0, last.y + 5.0, "CH₃", 15.0);
            }
        }
        if p.b("numbers") {
            for (i, a) in v.iter().enumerate() {
                label(d, a.x, a.y + 26.0, &(i + 1).to_string(), 12.0);
            }
        }
        // a one-carbon alkane has no vertices to show without a label
        if n == 1 && group == "None (alkane)" { label(d, v[0].x, v[0].y + 5.0, "CH₄", 22.0); }
        if n == 1 && group == "\u{2013}OH (alcohol)" { label(d, v[0].x - 22.0, v[0].y + 5.0, "CH₃", 17.0); }
    }
}

fn draw_ring(p: &Params, d: &mut Doc, v: &[V], double_bonds: bool, full: bool) {
    let n = v.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let order = if double_bonds && i % 2 == 0 { 2 } else { 1 };
        bond(d, v[i], v[j], order);
    }
    if full {
        let c = V { x: W / 2.0, y: H / 2.0 - 14.0 };
        for (i, a) in v.iter().enumerate() {
            let ang = dir_deg(c, *a);
            label(d, a.x, a.y + 7.0, "C", 18.0);
            let r = 26.0;
            let (hx, hy) = (a.x + r * ang.to_radians().cos(), a.y + r * ang.to_radians().sin());
            d.line(a.x, a.y, hx, hy, INK, 1.6);
            label(d, hx + 10.0 * ang.to_radians().cos(), hy + 6.0 + 10.0 * ang.to_radians().sin(), "H", 15.0);
            if p.b("numbers") { label(d, a.x, a.y + 26.0, &(i + 1).to_string(), 12.0); }
        }
    } else if p.b("numbers") {
        for (i, a) in v.iter().enumerate() { label(d, a.x, a.y + 26.0, &(i + 1).to_string(), 12.0); }
    }
}

fn build(p: &Params) -> String {
    let mode = p.s("mode");
    let (c, h, o) = atom_count(p);
    let mut d = Doc::new(W, H);
    let caption = p.s("caption").trim();
    if !caption.is_empty() { label(&mut d, W / 2.0, 30.0, caption, 17.0); }

    if mode == "Condensed" {
        let text = condensed(p, c, h, o);
        label(&mut d, W / 2.0, H / 2.0 - 2.0, &text, 46.0);
        super::super::chart::text(&mut d, W / 2.0, H / 2.0 + 30.0, 14.0, "middle", "Condensed formula");
    } else {
        let full = mode == "Full";
        match p.s("skeleton") {
            "Benzene ring" => draw_ring(p, &mut d, &ring_vertices(6), true, full),
            "Cyclohexane ring" => draw_ring(p, &mut d, &ring_vertices(6), false, full),
            "Cyclopentane ring" => draw_ring(p, &mut d, &ring_vertices(5), false, full),
            _ => draw_chain(p, &mut d, &chain_vertices(p.i("carbons").clamp(1, 12) as usize), full),
        }
    }

    if p.b(SHOW_ANSWERS) && mode != "Condensed" {
        let mass = c as f64 * 12.011 + h as f64 * 1.008 + o as f64 * 15.999;
        let f = formula(c, h, o);
        super::super::chart::text(&mut d, 18.0, H - 22.0, 14.0, "start", &format!("Molecular formula {} · Mr = {} · {}", f, fmt_num((mass * 100.0).round() / 100.0), condensed(p, c, h, o)));
    } else if p.b(SHOW_ANSWERS) {
        let mass = c as f64 * 12.011 + h as f64 * 1.008 + o as f64 * 15.999;
        super::super::chart::text(&mut d, 18.0, H - 22.0, 14.0, "start", &format!("Molecular formula {} · Mr = {}", formula(c, h, o), fmt_num((mass * 100.0).round() / 100.0)));
    }
    d.finish()
}
