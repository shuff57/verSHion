//! Leaf cross-section: a transverse section through the blade of a dicot leaf, with its layers labelled.
//!
//! Layers, top to bottom: waxy cuticle, upper epidermis (clear cells, no chloroplasts), palisade mesophyll (columnar
//! cells packed with chloroplasts), spongy mesophyll (irregular cells with fewer chloroplasts and large air spaces),
//! lower epidermis with stomata (each pore between two guard cells, the only epidermal cells with chloroplasts, an air
//! chamber above it), and a thinner lower cuticle. A vein (vascular bundle) sits in the mesophyll, ringed by a bundle
//! sheath, with xylem on the upper side and phloem (sieve tubes and companion cells) on the lower side. Stomata are
//! drawn only in the lower epidermis, where most of them are in a typical dicot leaf.
//!
//! Source checked for names and order: OpenStax, Biology 2e (2018), section 30.4 "Leaves", figures 30.24, 30.26 and
//! 30.27 (cuticle on aerial surfaces; upper and lower epidermis; "palisade parenchyma (also called the palisade
//! mesophyll) has column-shaped, tightly packed cells"; "loosely arranged cells of an irregular shape" with air spaces
//! below them; "Guard cells are the only epidermal cells to contain chloroplasts"; stomata on the underside; xylem at
//! the top of the bundle and phloem at the bottom). Palisade cells holding most of the chloroplasts: Campbell Biology
//! (Urry et al.), leaf anatomy figure in the plant structure chapter. The drawing and the code are original.
//!
//! Pointers: a bracket beside a layer spans exactly that layer (and one beside the vein spans the whole bundle); a
//! plain leader line ends inside the one part it names (no dot or ring, so nothing small is hidden); an arrow carries
//! light or a gas and is named by what it carries. With Show answers off the names become numbered discs in the same
//! places and a table under the figure gives each a rule to write on.
//!
//! Stomata are a choice, Open or Closed: open guard cells are swollen and bowed apart round a pore, closed ones meet.
//! The gas-exchange arrows go through an open stoma only; with the stomata closed they are left out and the editor
//! note says why. A small surface view (the pore and the two kidney-shaped guard cells seen from below) is optional.

use super::cladogram::text_w;
use super::*;

pub const ID: &str = "leaf-cross-section";

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Leaf Cross-Section",
        blurb: "A section through a leaf: cuticle, epidermis, palisade and spongy mesophyll, a vein with xylem and phloem, and stomata with guard cells drawn open or closed. Optional gas-exchange and light arrows; a student copy with numbered blanks.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("stomata", "Stomata", &["Open", "Closed"], "Open").hint("Open: the guard cells are swollen and bowed apart round a pore. Closed: they are limp and meet, so the pore is shut"),
        ParamDef::bool("show_layers", "Label the layers", true).hint("Waxy cuticle, upper epidermis, palisade mesophyll, spongy mesophyll, lower epidermis"),
        ParamDef::bool("show_vein", "Label the vein", true).hint("Vascular bundle, xylem and phloem"),
        ParamDef::bool("show_stoma", "Label the stoma", true).hint("Stoma, guard cell and an air space. With the stomata closed there is no open pore, so the stoma itself is not labelled"),
        ParamDef::bool("show_chloroplast", "Label a chloroplast", true).hint("One chloroplast at the top of a palisade cell"),
        ParamDef::bool("show_gas", "Gas exchange arrows", true).hint("Carbon dioxide in, oxygen and water vapour out, through an open stoma. Not drawn when the stomata are closed"),
        ParamDef::bool("show_light", "Light arrow", true),
        ParamDef::bool("show_surface", "Stoma seen from below", true).hint("A small surface view beside the section: the pore between two kidney-shaped guard cells"),
        answers_param().hint("Off keeps numbered blanks in place of the names, with a table to write them in"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { draw(p) }

// ================================================================== text with subscripts

const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const INK: &str = "#111111";
const MARGIN: f64 = 18.0;

pub type P = (f64, f64);
pub type Bx = (f64, f64, f64, f64);

/// A name with a little markup: `~` then digits is a subscript run (`CO~2`). Nothing else uses the character.
pub fn plain(s: &str) -> String { s.replace('~', "") }

/// (text, size factor, baseline shift in em) runs of a marked-up string.
fn runs(s: &str) -> Vec<(String, f64, f64)> {
    let mut out: Vec<(String, f64, f64)> = Vec::new();
    let mut it = s.chars().peekable();
    let mut cur = String::new();
    while let Some(c) = it.next() {
        if c == '~' {
            if !cur.is_empty() { out.push((std::mem::take(&mut cur), 1.0, 0.0)); }
            let mut d = String::new();
            while let Some(x) = it.peek().copied().filter(|x| x.is_ascii_digit()) { d.push(x); it.next(); }
            if !d.is_empty() { out.push((d, 0.72, 0.22)); }
        } else { cur.push(c); }
    }
    if !cur.is_empty() { out.push((cur, 1.0, 0.0)); }
    out
}

/// Width in px of a marked-up string (a little generous: a label must never be wider than its box).
fn rw(s: &str, size: f64) -> f64 { runs(s).iter().map(|(t, f, _)| text_w(t, size * f)).sum::<f64>() * 1.04 + 1.0 }

/// A `<text>` for a marked-up string. Subscripts are tspans that shift and return.
fn rich(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    let mut o = format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{fill}">"##);
    let mut shifted = 0.0;
    for (t, f, sh) in runs(s) {
        if f == 1.0 {
            if shifted != 0.0 { o.push_str(&format!(r#"<tspan dy="{:.2}">{}</tspan>"#, -shifted * size, svg::esc(&t))); shifted = 0.0; } else { o.push_str(&svg::esc(&t)); }
        } else {
            o.push_str(&format!(r#"<tspan font-size="{:.2}" dy="{:.2}">{}</tspan>"#, size * f, (sh - shifted) * size, svg::esc(&t)));
            shifted = sh;
        }
    }
    o.push_str("</text>");
    o
}

/// Greedy wrap of a marked-up string to `max` px.
fn wrap_rich(s: &str, max: f64, size: f64) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in s.split_whitespace() {
        let t = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
        if !cur.is_empty() && rw(&t, size) > max { lines.push(std::mem::take(&mut cur)); cur = w.to_string(); } else { cur = t; }
    }
    if !cur.is_empty() { lines.push(cur); }
    lines
}

// ================================================================== what can be labelled

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum It { Cuticle, Upper, Palisade, Chloroplast, Spongy, Lower, Bundle, Xylem, Phloem, AirSpace, Guard, Stoma, Light, Co2, O2, Water }

pub const ORDER: [It; 16] = [It::Cuticle, It::Upper, It::Palisade, It::Chloroplast, It::Spongy, It::Lower, It::Bundle, It::Xylem, It::Phloem, It::AirSpace, It::Guard, It::Stoma, It::Light, It::Co2, It::O2, It::Water];

pub fn name(i: It) -> &'static str {
    match i {
        It::Cuticle => "Waxy cuticle", It::Upper => "Upper epidermis", It::Palisade => "Palisade mesophyll", It::Chloroplast => "Chloroplast", It::Spongy => "Spongy mesophyll", It::Lower => "Lower epidermis",
        It::Bundle => "Vascular bundle", It::Xylem => "Xylem", It::Phloem => "Phloem", It::AirSpace => "Air space", It::Guard => "Guard cell", It::Stoma => "Stoma",
        It::Light => "Light", It::Co2 => "CO~2", It::O2 => "O~2", It::Water => "Water vapour",
    }
}

/// Other wordings the key accepts for the same blank.
pub fn alts(i: It) -> &'static str {
    match i {
        It::Cuticle => "cuticle, wax layer",
        It::Upper => "epidermis, adaxial epidermis",
        It::Palisade => "palisade layer, palisade cells, palisade parenchyma",
        It::Chloroplast => "chloroplasts (plural)",
        It::Spongy => "spongy layer, spongy cells, spongy parenchyma",
        It::Lower => "epidermis, abaxial epidermis",
        It::Bundle => "vein, leaf vein",
        It::Xylem => "xylem vessel",
        It::Phloem => "sieve tube, phloem sieve tube",
        It::AirSpace => "intercellular air space, intercellular space",
        It::Guard => "guard cells (plural)",
        It::Stoma => "stomata (plural), stomatal pore, pore",
        It::Light => "sunlight, light energy",
        It::Co2 => "carbon dioxide",
        It::O2 => "oxygen",
        It::Water => "water, H~2O, water vapor",
    }
}

/// The other wordings the key accepts for a blank in this state: `alts`, and with the stomata closed (no Stoma blank to
/// ask for it) "stoma" as well for the Guard cell, the answer to the pair of cells that make up the shut stoma.
pub fn accepted(i: It, open: bool) -> String {
    if i == It::Guard && !open { format!("{}, stoma", alts(i)) } else { alts(i).to_string() }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cfg { pub open: bool, pub chloro: bool, pub layers: bool, pub vein: bool, pub stoma: bool, pub gas: bool, pub light: bool, pub surface: bool, pub key: bool, pub lab: f64 }

pub fn cfg(p: &Params) -> Cfg {
    Cfg {
        open: p.s("stomata") != "Closed", chloro: p.b("show_chloroplast"),
        layers: p.b("show_layers"), vein: p.b("show_vein"), stoma: p.b("show_stoma"), gas: p.b("show_gas"), light: p.b("show_light"), surface: p.b("show_surface"),
        key: p.b(SHOW_ANSWERS), lab: svg::label_factor(p.s("label_size")),
    }
}

impl Cfg {
    /// The gas arrows are drawn only through an open stoma.
    pub fn gas_drawn(&self) -> bool { self.gas && self.open }
    pub fn wants(&self, i: It) -> bool {
        match i {
            It::Cuticle | It::Upper | It::Palisade | It::Spongy | It::Lower => self.layers,
            It::Bundle | It::Xylem | It::Phloem => self.vein,
            It::Chloroplast => self.chloro,
            It::AirSpace | It::Guard => self.stoma,
            // a closed stoma has no open pore to point at: the pointer would end on the guard cells, the Guard cell blank's answer
            It::Stoma => self.stoma && self.open,
            It::Light => self.light,
            It::Co2 | It::O2 | It::Water => self.gas_drawn(),
        }
    }
}

// ================================================================== the section, as shapes (section coordinates)

pub const SW: f64 = 600.0;
pub const Y_UE0: f64 = 6.0;
pub const Y_PAL0: f64 = 30.0;
pub const Y_SP0: f64 = 110.0;
pub const Y_LE0: f64 = 196.0;
pub const Y_LC0: f64 = 216.0;
pub const Y_LC1: f64 = 219.5;
/// The x of each stoma; the first carries the gas arrows, the second the stoma and guard-cell labels.
pub const STOMATA: [f64; 2] = [160.0, 330.0];
pub const GAS_STOMA: usize = 0;
pub const LABEL_STOMA: usize = 1;
/// Half the width of the gap left in the lower epidermis for a stoma: the guard cells and 1 px. The neighbouring
/// epidermal cells widen when the guard cells shrink, as subsidiary cells do.
pub fn stoma_zone(open: bool) -> f64 { let (rx, _, off) = guard_geom(open); off + rx + 1.0 }
pub const GUARD_Y: f64 = 206.0;
/// The vein: the centre, the ellipse of xylem and phloem, the ring of bundle-sheath cells.
pub const V: P = (474.0, 152.0);
pub const VEIN_RX: f64 = 37.0;
pub const VEIN_RY: f64 = 28.0;
pub const SHEATH_RX: f64 = 44.0;
pub const SHEATH_RY: f64 = 34.5;
pub const SHEATH_R: f64 = 6.5;
pub const SHEATH_N: usize = 16;
/// The air space the "Air space" leader ends in (kept free of cells), and the radius kept free.
pub const AIR: P = (570.0, 178.0);
pub const AIR_FREE: f64 = 10.0;
/// Xylem vessels (dx, dy, r) from the vein centre; the first is the one the leader ends in.
pub const XYLEM: [(f64, f64, f64); 7] = [(0.0, -17.0, 5.5), (-15.0, -16.0, 4.0), (15.0, -16.0, 4.0), (-24.0, -6.0, 4.3), (-9.0, -5.0, 5.8), (7.0, -5.0, 5.8), (22.0, -6.0, 4.6)];
/// Phloem sieve tubes (dx, dy, r); the leader ends in index 6.
pub const PHLOEM: [(f64, f64, f64); 9] = [(-22.0, 8.0, 3.6), (-11.0, 9.0, 4.0), (0.0, 10.0, 4.0), (11.0, 9.0, 4.0), (22.0, 8.0, 3.6), (-14.0, 20.0, 3.4), (0.0, 20.5, 3.8), (10.0, 19.5, 3.4), (18.0, 16.5, 3.0)];
pub const PHLOEM_TARGET: usize = 6;
/// Companion cells (dx, dy, r): small cells beside the sieve tubes.
pub const COMPANION: [(f64, f64, f64); 3] = [(-16.5, 13.5, 1.8), (-5.5, 14.0, 1.8), (5.5, 14.0, 1.8)];
pub const PAL_PITCH: f64 = 22.0;
pub const PAL_W: f64 = 18.0;
pub const PAL_X0: f64 = -8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum K { Cuticle, Upper, Palisade, Spongy, VeinBg, Sheath, Xylem, Phloem, Companion, Lower, Guard }

#[derive(Clone, Debug)]
pub struct Shape { pub k: K, pub pts: Vec<P> }

/// A chloroplast: a small filled ellipse inside a cell of kind `k`.
#[derive(Clone, Copy, Debug)]
pub struct Chl { pub c: P, pub rx: f64, pub ry: f64, pub k: K }

pub struct Scene { pub shapes: Vec<Shape>, pub chl: Vec<Chl> }

/// (half width, half height, centre offset from the pore) of a guard cell in the section. Open: swollen with water
/// (turgid), larger and round, bowed apart round the pore. Closed: limp (flaccid), smaller and flatter, meeting.
pub fn guard_geom(open: bool) -> (f64, f64, f64) { if open { (9.5, 9.5, 15.5) } else { (7.0, 8.0, 7.2) } }

/// Half the width of the pore between the guard cells (the gap between their outlines).
pub fn pore_half(open: bool) -> f64 { let (rx, _, off) = guard_geom(open); off - rx }

/// The guard cell centres of the stoma at `xs`: (left, right).
pub fn guard_centres(xs: f64, open: bool) -> (P, P) { let (_, _, off) = guard_geom(open); ((xs - off, GUARD_Y), (xs + off, GUARD_Y)) }

fn ellp(c: P, rx: f64, ry: f64, n: usize) -> Vec<P> {
    (0..n).map(|i| { let a = std::f64::consts::TAU * i as f64 / n as f64; (c.0 + rx * a.cos(), c.1 + ry * a.sin()) }).collect()
}

fn rrect(x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> Vec<P> {
    let r = r.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
    let mut v = Vec::new();
    let corners = [(x1 - r, y0 + r, -90.0), (x1 - r, y1 - r, 0.0), (x0 + r, y1 - r, 90.0), (x0 + r, y0 + r, 180.0)];
    for (cx, cy, a0) in corners {
        for i in 0..=4 { let a = (a0 + 22.5 * i as f64).to_radians(); v.push((cx + r * a.cos(), cy + r * a.sin())); }
    }
    v
}

/// A rounded, slightly lobed cell outline.
fn blob(c: P, rx: f64, ry: f64, ph: f64) -> Vec<P> {
    (0..28).map(|i| {
        let a = std::f64::consts::TAU * i as f64 / 28.0;
        let k = 1.0 + 0.08 * (3.0 * a + ph).sin() + 0.04 * (5.0 * a + 2.0 * ph).sin();
        (c.0 + rx * k * a.cos(), c.1 + ry * k * a.sin())
    }).collect()
}

/// Cuts a polygon to `lo <= x <= hi` (the cut edges of the section).
fn clip_x(pts: &[P], lo: f64, hi: f64) -> Vec<P> {
    let cut = |pts: &[P], keep: &dyn Fn(P) -> bool, edge: f64| -> Vec<P> {
        let mut out = Vec::new();
        let n = pts.len();
        for i in 0..n {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let (ia, ib) = (keep(a), keep(b));
            if ia { out.push(a); }
            if ia != ib { let t = (edge - a.0) / (b.0 - a.0); out.push((edge, a.1 + t * (b.1 - a.1))); }
        }
        out
    };
    let a = cut(pts, &|q: P| q.0 >= lo, lo);
    if a.len() < 3 { return Vec::new(); }
    let b = cut(&a, &|q: P| q.0 <= hi, hi);
    if b.len() < 3 { Vec::new() } else { b }
}

fn seg_d(q: P, a: P, b: P) -> f64 { crate::figures::composite::seg_dist(q, a, b) }

/// Smallest distance between two outlines (vertex to edge, both ways).
fn outline_gap(a: &[P], b: &[P]) -> f64 {
    let one = |a: &[P], b: &[P]| a.iter().map(|q| (0..b.len()).map(|i| seg_d(*q, b[i], b[(i + 1) % b.len()])).fold(f64::INFINITY, f64::min)).fold(f64::INFINITY, f64::min);
    one(a, b).min(one(b, a))
}

/// A number in [-0.5, 0.5] from a cell's grid position (deterministic jitter).
fn jit(i: usize, j: usize, s: usize) -> f64 { ((i * 73 + j * 151 + s * 29 + 11) % 97) as f64 / 96.0 - 0.5 }

/// Fixed lines inside the spongy layer that spongy cells keep clear of: the vein's bracket, its leader and the
/// air-space leader (so those lines run through air, not over a cell).
pub fn spongy_keepout() -> Vec<(P, P)> {
    vec![((BRACKET_VX, Y_SP0), (BRACKET_VX, Y_LE0)), ((BRACKET_VX, V.1), (SW, V.1)), ((RX - 4.0, AIR_LY), AIR)]
}

/// The spongy mesophyll cells: centre, radii and outline. Rows of lobed cells, each kept at least 3 px from every
/// other, out of the vein, the air chambers over the stomata, the reserved air space and the keep-out lines.
pub fn spongy_cells() -> Vec<(P, f64, f64, f64, Vec<P>)> {
    static CELLS: std::sync::OnceLock<Vec<(P, f64, f64, f64, Vec<P>)>> = std::sync::OnceLock::new();
    CELLS.get_or_init(make_spongy).clone()
}

fn make_spongy() -> Vec<(P, f64, f64, f64, Vec<P>)> {
    let mut out: Vec<(P, f64, f64, f64, Vec<P>)> = Vec::new();
    let keep = spongy_keepout();
    let try_cell = |out: &mut Vec<(P, f64, f64, f64, Vec<P>)>, c: P, rx: f64, ry: f64, ph: f64, gap: f64| {
        let pts = blob(c, rx, ry, ph);
        let ok_y = pts.iter().all(|q| q.1 >= Y_SP0 + 2.5 && q.1 <= Y_LE0 - 2.5);
        // a cell cut by the edge of the section keeps at least 6 px of itself
        let ok_x = pts.iter().any(|q| q.0 > 6.0) && pts.iter().any(|q| q.0 < SW - 6.0);
        let out_of = |e: P, ex: f64, ey: f64| pts.iter().all(|q| ((q.0 - e.0) / ex).powi(2) + ((q.1 - e.1) / ey).powi(2) > 1.0);
        let vein_ok = out_of(V, SHEATH_RX + SHEATH_R + 4.0, SHEATH_RY + SHEATH_R + 4.0);
        let chambers_ok = STOMATA.iter().all(|xs| out_of((*xs, 185.0), 22.0, 12.0));
        let air_ok = pts.iter().all(|q| (q.0 - AIR.0).hypot(q.1 - AIR.1) > AIR_FREE + 2.0);
        let lines_ok = keep.iter().all(|(a, b)| pts.iter().all(|q| seg_d(*q, *a, *b) > 3.0));
        let near: Vec<&(P, f64, f64, f64, Vec<P>)> = out.iter().filter(|o| (o.0 .0 - c.0).abs() < 40.0 && (o.0 .1 - c.1).abs() < 40.0).collect();
        let gap_ok = near.iter().all(|o| outline_gap(&o.4, &pts) > gap && !inside_poly(&o.4, c));
        if ok_y && ok_x && vein_ok && chambers_ok && air_ok && lines_ok && gap_ok { out.push((c, rx, ry, ph, pts)); }
    };
    // three staggered rows of large lobed cells ...
    for j in 0..3usize {
        for i in 0..24usize {
            let c = (-4.0 + i as f64 * 27.0 + if j % 2 == 1 { 13.5 } else { 0.0 } + 5.0 * jit(i, j, 1), 125.5 + j as f64 * 27.5 + 2.0 * jit(i, j, 2));
            try_cell(&mut out, c, 9.5 + ((i * 3 + j * 2) % 4) as f64 * 1.15, 9.0 + ((i + j * 2) % 3) as f64 * 0.7, (i * 7 + j * 3) as f64 * 0.9, 2.0);
        }
    }
    // ... then smaller ones where a large one did not fit, leaving the rest as air spaces
    for j in 0..9usize {
        for i in 0..70usize {
            let c = (i as f64 * 9.0 + 4.0 * jit(i, j, 3), 120.0 + j as f64 * 8.5 + 2.0 * jit(i, j, 4));
            try_cell(&mut out, c, 8.0 + 1.5 * (jit(i, j, 5) + 0.5), 7.0 + 1.2 * (jit(i, j, 6) + 0.5), (i * 5 + j * 11) as f64 * 0.7, 4.0);
        }
    }
    out
}

fn inside_poly(pts: &[P], q: P) -> bool {
    let mut c = false;
    let n = pts.len();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        if (a.1 > q.1) != (b.1 > q.1) && q.0 < (b.0 - a.0) * (q.1 - a.1) / (b.1 - a.1) + a.0 { c = !c; }
    }
    c
}

/// Centres of the bundle-sheath cells: equal steps along an ellipse, with a gap (not a cell) at the top.
pub fn sheath_centres() -> Vec<P> {
    let n = 1440;
    let pt = |t: f64| (V.0 + SHEATH_RX * t.cos(), V.1 + SHEATH_RY * t.sin());
    let t0 = -std::f64::consts::FRAC_PI_2;
    let mut cum = vec![0.0];
    for i in 1..=n {
        let (a, b) = (pt(t0 + std::f64::consts::TAU * (i - 1) as f64 / n as f64), pt(t0 + std::f64::consts::TAU * i as f64 / n as f64));
        cum.push(cum[i - 1] + (b.0 - a.0).hypot(b.1 - a.1));
    }
    let total = cum[n];
    (0..SHEATH_N).map(|k| {
        let s = (k as f64 + 0.5) * total / SHEATH_N as f64;
        let i = cum.iter().position(|c| *c >= s).unwrap_or(n);
        pt(t0 + std::f64::consts::TAU * i as f64 / n as f64)
    }).collect()
}

/// Cells of one epidermal row between `a` and `b`, as equal widths near `pitch`.
fn epi_row(a: f64, b: f64, pitch: f64, y0: f64, y1: f64, out: &mut Vec<Vec<P>>) {
    let n = ((b - a) / pitch).round().max(1.0) as usize;
    let w = (b - a) / n as f64;
    for i in 0..n {
        let x = a + i as f64 * w;
        let c = clip_x(&rrect(x + 0.7, y0, x + w - 0.7, y1, 3.0), 0.0, SW);
        if !c.is_empty() { out.push(c); }
    }
}

/// The bottom of palisade cell `k` (they differ a little, as in a real section).
fn pal_bottom(k: usize) -> f64 { 107.0 - [0.0, 2.5, 1.0, 3.5, 0.5, 2.0][k % 6] }

/// Everything in the section, in drawing order; the chloroplasts are drawn after the cells.
pub fn scene(open: bool) -> Scene {
    let mut s: Vec<Shape> = Vec::new();
    let mut chl: Vec<Chl> = Vec::new();
    let mut add = |k: K, pts: Vec<P>| if pts.len() >= 3 { s.push(Shape { k, pts }); };
    // upper cuticle
    add(K::Cuticle, vec![(0.0, 0.0), (SW, 0.0), (SW, Y_UE0), (0.0, Y_UE0)]);
    // upper epidermis: clear cells of mixed widths
    let widths = [34.0, 29.0, 37.0, 31.0, 35.0, 28.0, 33.0];
    let mut x = -14.0;
    let mut i = 0;
    while x < SW {
        let w = widths[i % widths.len()];
        add(K::Upper, clip_x(&rrect(x + 0.7, Y_UE0 + 0.8, x + w - 0.7, Y_PAL0 - 0.8, 3.0), 0.0, SW));
        x += w;
        i += 1;
    }
    // palisade: tall columns with chloroplasts along their side walls
    let mut k = 0;
    loop {
        let x0 = PAL_X0 + k as f64 * PAL_PITCH;
        if x0 >= SW { break; }
        let bot = pal_bottom(k);
        add(K::Palisade, clip_x(&rrect(x0, Y_PAL0 + 1.5, x0 + PAL_W, bot, 6.0), 0.0, SW));
        let mut y = Y_PAL0 + 8.0;
        while y + 2.4 <= bot - 4.0 {
            for cx in [x0 + 4.0, x0 + PAL_W - 4.0] { if cx > 3.0 && cx < SW - 3.0 { chl.push(Chl { c: (cx, y), rx: 1.5, ry: 2.4, k: K::Palisade }); } }
            y += 5.2;
        }
        k += 1;
    }
    // spongy: lobed cells with a few chloroplasts
    for (c, rx, ry, ph, pts) in spongy_cells() {
        add(K::Spongy, clip_x(&pts, 0.0, SW));
        // three in a large cell, two in a small one: fewer than the palisade, as in a real leaf
        let n = if rx < 9.4 { 2 } else { 3 };
        for q in 0..n {
            let a = ph + 0.4 + q as f64 * std::f64::consts::TAU / n as f64;
            let p = (c.0 + 0.5 * rx * a.cos(), c.1 + 0.5 * ry * a.sin());
            if p.0 > 4.0 && p.0 < SW - 4.0 { chl.push(Chl { c: p, rx: 2.2, ry: 1.5, k: K::Spongy }); }
        }
    }
    // the vein
    add(K::VeinBg, ellp(V, VEIN_RX, VEIN_RY, 48));
    for c in sheath_centres() { add(K::Sheath, ellp(c, SHEATH_R, SHEATH_R, 18)); }
    for (dx, dy, r) in XYLEM { add(K::Xylem, ellp((V.0 + dx, V.1 + dy), r, r, 20)); }
    for (dx, dy, r) in PHLOEM { add(K::Phloem, ellp((V.0 + dx, V.1 + dy), r, r, 16)); }
    for (dx, dy, r) in COMPANION { add(K::Companion, ellp((V.0 + dx, V.1 + dy), r, r, 10)); }
    // lower epidermis, with a gap for each stoma
    let mut cells = Vec::new();
    let mut a = -12.0;
    let zone = stoma_zone(open);
    for xs in STOMATA { epi_row(a, xs - zone, 27.0, Y_LE0 + 0.8, Y_LC0 - 0.8, &mut cells); a = xs + zone; }
    epi_row(a, SW + 12.0, 27.0, Y_LE0 + 0.8, Y_LC0 - 0.8, &mut cells);
    for c in cells { add(K::Lower, c); }
    // guard cells, each with three chloroplasts kept off its middle and its outer lower side (where a leader may come in)
    let (grx, gry, _) = guard_geom(open);
    for xs in STOMATA {
        let (l, r) = guard_centres(xs, open);
        add(K::Guard, ellp(l, grx, gry, 28));
        add(K::Guard, ellp(r, grx, gry, 28));
        for (c, side) in [(l, -1.0), (r, 1.0)] {
            for (dx, dy) in [(-0.42, -0.5), (0.42, -0.5), (-0.42 * side, 0.5)] { chl.push(Chl { c: (c.0 + dx * grx, c.1 + dy * gry), rx: 1.7, ry: 1.2, k: K::Guard }); }
        }
    }
    // lower cuticle, open at each open pore
    let ph = pore_half(open);
    let mut x0 = 0.0;
    if open { for xs in STOMATA { add(K::Cuticle, vec![(x0, Y_LC0), (xs - ph, Y_LC0), (xs - ph, Y_LC1), (x0, Y_LC1)]); x0 = xs + ph; } }
    add(K::Cuticle, vec![(x0, Y_LC0), (SW, Y_LC0), (SW, Y_LC1), (x0, Y_LC1)]);
    Scene { shapes: s, chl }
}

// ---------------------------------------------------------------- the surface view (its own coordinates)

pub const INSET_W: f64 = 120.0;
pub const INSET_H: f64 = 80.0;

/// The surface view: two kidney-shaped guard cells round the pore (crescents), the walls of the cells round them,
/// and the guard cells' chloroplasts.
pub fn inset(open: bool) -> (Vec<Vec<P>>, Vec<(P, P)>, Vec<P>) {
    let (cx, cy) = (INSET_W / 2.0, INSET_H / 2.0);
    // open: swollen kidney-shaped cells bowed round a wide pore; closed: smaller limp cells meeting at a slit
    let (orx, ory) = if open { (23.0, 29.0) } else { (15.0, 24.0) };
    let (prx, pry) = if open { (7.5, 17.0) } else { (0.6, 15.0) };
    // the left guard cell: the outer left half-ellipse from bottom to top, then the pore's left half back down
    let mut left: Vec<P> = Vec::new();
    for i in 0..=24 { let a = (90.0 + 180.0 * i as f64 / 24.0f64).to_radians(); left.push((cx + orx * a.cos(), cy + ory * a.sin())); }
    for i in 0..=16 { let a = (270.0 - 180.0 * i as f64 / 16.0f64).to_radians(); left.push((cx + prx * a.cos(), cy + pry * a.sin())); }
    let right: Vec<P> = left.iter().map(|q| (2.0 * cx - q.0, q.1)).collect();
    let walls = vec![
        ((cx, cy - ory), (cx, 0.0)), ((cx, cy + ory), (cx, INSET_H)),
        ((cx - orx * 0.87, cy - ory * 0.5), (0.0, 12.0)), ((cx - orx * 0.87, cy + ory * 0.5), (0.0, 66.0)),
        ((cx + orx * 0.87, cy - ory * 0.5), (INSET_W, 14.0)), ((cx + orx * 0.87, cy + ory * 0.5), (INSET_W, 64.0)),
    ];
    let mut chl = Vec::new();
    for a in [150.0f64, 180.0, 210.0] {
        let a = a.to_radians();
        let q = (cx + ((orx + prx) / 2.0) * a.cos(), cy + ((ory + pry) / 2.0) * a.sin());
        chl.push(q);
        chl.push((2.0 * cx - q.0, q.1));
    }
    (vec![left, right], walls, chl)
}

// ================================================================== labels

/// How a label is joined to what it names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Join {
    /// A bracket at `x` from `y0` to `y1`, its end ticks pointing `tick` px in x; `lead` joins it to the label.
    Bracket { x: f64, y0: f64, y1: f64, tick: f64, lead: Option<(P, P)> },
    /// A plain line from the label to a point inside the part.
    Leader { from: P, to: P },
    /// The label names an arrow.
    Arrow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anc { Start, Mid, End }

#[derive(Clone, Debug)]
pub struct Lab { pub it: It, pub num: usize, pub bx: Bx, pub anc: Anc, pub join: Join }

pub struct Sizes { pub fs: f64, pub r: f64 }

pub fn sizes(lab: f64) -> Sizes { Sizes { fs: 13.0 * lab, r: 10.5 * lab } }

pub fn slot_h(z: &Sizes) -> f64 { (1.2 * z.fs).max(2.0 * z.r) }

fn name_w(i: It, z: &Sizes) -> f64 { rw(name(i), z.fs).max(2.0 * z.r) }

/// x of the left brackets and of the right edge of their labels.
pub const BX_L: f64 = -7.0;
pub const LX_END: f64 = -15.0;
/// Bottom of the top label row; top of the first bottom row.
pub const TOP_B: f64 = -16.0;
pub const BOT1: f64 = 236.0;
/// Left edge of the right label column, and the label y of the air space.
pub const RX: f64 = SW + 30.0;
pub const AIR_LY: f64 = 204.0;
pub const BRACKET_VX: f64 = V.0 + SHEATH_RX + SHEATH_R + 3.5;
pub const CUTICLE_X: f64 = 70.0;
/// The chloroplast the Chloroplast leader ends on: the top one of the right-hand column of palisade cell 16.
pub const CHLORO_CELL: usize = 16;
pub fn chloro_target() -> P { (PAL_X0 + CHLORO_CELL as f64 * PAL_PITCH + PAL_W - 4.0, Y_PAL0 + 8.0) }
pub const LIGHT_X: f64 = 250.0;
pub const LIGHT_Y0: f64 = -62.0;
pub const LIGHT_Y1: f64 = -4.0;
pub const INSET_Y: f64 = 252.0;

/// Where the leader of each part ends.
pub fn target(i: It, open: bool) -> Option<P> {
    Some(match i {
        It::Cuticle => (CUTICLE_X, Y_UE0 / 2.0),
        // the leader stops 0.4 px above the chloroplast (half height 2.4), so it touches it without covering it
        It::Chloroplast => { let c = chloro_target(); (c.0, c.1 - 2.8) }
        It::Xylem => (V.0 + XYLEM[0].0, V.1 + XYLEM[0].1),
        It::Phloem => (V.0 + PHLOEM[PHLOEM_TARGET].0, V.1 + PHLOEM[PHLOEM_TARGET].1),
        It::AirSpace => AIR,
        It::Guard => guard_centres(STOMATA[LABEL_STOMA], open).0,
        It::Stoma => (STOMATA[LABEL_STOMA], GUARD_Y),
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct Flow { pub it: It, pub a: P, pub b: P, pub sw: f64 }

/// The gas-exchange arrows through the first stoma (fanned out to the left) and the light arrow.
pub fn flows(c: &Cfg) -> Vec<Flow> {
    let mut v = Vec::new();
    if c.light { v.push(Flow { it: It::Light, a: (LIGHT_X, LIGHT_Y0), b: (LIGHT_X, LIGHT_Y1), sw: 3.4 }); }
    if c.gas_drawn() {
        let m = (STOMATA[GAS_STOMA], Y_LC1);
        for (it, d) in [(It::Water, (-0.88, 0.47)), (It::Co2, (-0.5, 0.866)), (It::O2, (-0.05, 1.0))] {
            let n: f64 = f64::hypot(d.0, d.1);
            let (ux, uy) = (d.0 / n, d.1 / n);
            let inner = (m.0 + 7.0 * ux, m.1 + 7.0 * uy);
            let outer = (m.0 + 80.0 * ux, m.1 + 80.0 * uy);
            let (a, b) = if it == It::Co2 { (outer, inner) } else { (inner, outer) };
            v.push(Flow { it, a, b, sw: 2.6 });
        }
    }
    v
}

pub fn labels(c: &Cfg) -> Vec<Lab> {
    let z = sizes(c.lab);
    let h = slot_h(&z);
    let mut out: Vec<Lab> = Vec::new();
    // the layers: brackets on the left
    for (it, y0, y1) in [(It::Upper, Y_UE0, Y_PAL0), (It::Palisade, Y_PAL0, Y_SP0), (It::Spongy, Y_SP0, Y_LE0), (It::Lower, Y_LE0, Y_LC0)] {
        if !c.wants(it) { continue; }
        let w = name_w(it, &z);
        let cy = (y0 + y1) / 2.0;
        out.push(Lab { it, num: 0, bx: (LX_END - w, cy - h / 2.0, LX_END, cy + h / 2.0), anc: Anc::End, join: Join::Bracket { x: BX_L, y0: y0 + 1.8, y1: y1 - 1.8, tick: 5.0, lead: None } });
    }
    // the top row: the cuticle and the xylem, leaders straight down
    for it in [It::Cuticle, It::Chloroplast, It::Xylem] {
        if !c.wants(it) { continue; }
        let t = target(it, c.open).unwrap();
        let w = name_w(it, &z);
        out.push(Lab { it, num: 0, bx: (t.0 - w / 2.0, TOP_B - h, t.0 + w / 2.0, TOP_B), anc: Anc::Mid, join: Join::Leader { from: (t.0, TOP_B + 2.0), to: t } });
    }
    // the right column: the whole vein (a bracket) and an air space
    if c.wants(It::Bundle) {
        let w = name_w(It::Bundle, &z);
        let (y0, y1) = (V.1 - SHEATH_RY - SHEATH_R, V.1 + SHEATH_RY + SHEATH_R);
        out.push(Lab { it: It::Bundle, num: 0, bx: (RX, V.1 - h / 2.0, RX + w, V.1 + h / 2.0), anc: Anc::Start, join: Join::Bracket { x: BRACKET_VX, y0, y1, tick: -5.0, lead: Some(((BRACKET_VX, V.1), (RX - 4.0, V.1))) } });
    }
    if c.wants(It::AirSpace) {
        let w = name_w(It::AirSpace, &z);
        out.push(Lab { it: It::AirSpace, num: 0, bx: (RX, AIR_LY - h / 2.0, RX + w, AIR_LY + h / 2.0), anc: Anc::Start, join: Join::Leader { from: (RX - 4.0, AIR_LY), to: AIR } });
    }
    // the bottom: guard cell (left of the stoma), stoma (under it, a row lower), phloem
    let xs = STOMATA[LABEL_STOMA];
    if c.wants(It::Guard) {
        let w = name_w(It::Guard, &z);
        out.push(Lab { it: It::Guard, num: 0, bx: (xs - 22.0 - w, BOT1, xs - 22.0, BOT1 + h), anc: Anc::End, join: Join::Leader { from: (xs - 26.0, BOT1 - 2.0), to: target(It::Guard, c.open).unwrap() } });
    }
    if c.wants(It::Stoma) {
        let w = name_w(It::Stoma, &z);
        let top = BOT1 + h + 6.0;
        out.push(Lab { it: It::Stoma, num: 0, bx: (xs - w / 2.0, top, xs + w / 2.0, top + h), anc: Anc::Mid, join: Join::Leader { from: (xs, top - 2.0), to: target(It::Stoma, c.open).unwrap() } });
    }
    if c.wants(It::Phloem) {
        let w = name_w(It::Phloem, &z);
        let t = target(It::Phloem, c.open).unwrap();
        let cx = t.0.max(xs + 10.0 + w / 2.0);
        out.push(Lab { it: It::Phloem, num: 0, bx: (cx - w / 2.0, BOT1, cx + w / 2.0, BOT1 + h), anc: Anc::Mid, join: Join::Leader { from: (cx, BOT1 - 2.0), to: t } });
    }
    // names at the arrows
    for f in flows(c) {
        let w = name_w(f.it, &z);
        let (bx, anc) = match f.it {
            It::Light => { let cy = LIGHT_Y0 + 6.0; ((LIGHT_X - 8.0 - w, cy - h / 2.0, LIGHT_X - 8.0, cy + h / 2.0), Anc::End) }
            It::Water => { let e = f.b; ((e.0 - 8.0 - w, e.1 - h / 2.0, e.0 - 8.0, e.1 + h / 2.0), Anc::End) }
            It::Co2 => { let e = f.a; ((e.0 - 8.0 - w, e.1 + 3.0 - h / 2.0, e.0 - 8.0, e.1 + 3.0 + h / 2.0), Anc::End) }
            _ => { let e = f.b; ((e.0 - w / 2.0, e.1 + 5.0, e.0 + w / 2.0, e.1 + 5.0 + h), Anc::Mid) }
        };
        out.push(Lab { it: f.it, num: 0, bx, anc, join: Join::Arrow });
    }
    out.sort_by_key(|l| ORDER.iter().position(|o| *o == l.it).unwrap());
    for (n, l) in out.iter_mut().enumerate() { l.num = n + 1; }
    out
}

// ================================================================== paint

#[derive(Clone, Copy)]
struct Tone { fill: &'static str, stroke: &'static str, sw: f64 }

fn tone(k: K, pr: bool) -> Tone {
    let t = |fill, stroke, sw| Tone { fill, stroke, sw };
    if pr {
        return match k {
            K::Cuticle => t("#808080", "#111111", 0.8),
            K::VeinBg => t("#ffffff", "none", 0.0),
            K::Xylem => t("#ffffff", "#111111", 2.2),
            K::Phloem => t("url(#vs-hatch4)", "#111111", 0.9),
            K::Companion => t("#ffffff", "#111111", 0.8),
            K::Guard => t("#ffffff", "#111111", 1.4),
            _ => t("#ffffff", "#111111", 1.0),
        };
    }
    match k {
        K::Cuticle => t("#f2d36b", "#b58f1a", 0.8),
        K::Upper | K::Lower => t("#f4f8ee", "#6b8f47", 1.0),
        K::Palisade => t("#d4ebb8", "#4e7d2c", 1.0),
        K::Spongy => t("#e2f1cf", "#5c8a3a", 1.0),
        K::VeinBg => t("#f8f1df", "none", 0.0),
        K::Sheath => t("#eef3dc", "#7a8a4a", 1.0),
        K::Xylem => t("#fffaf0", "#8a4b16", 2.0),
        K::Phloem => t("#f2d7e6", "#9a4f7a", 0.9),
        K::Companion => t("#e7b9d2", "#9a4f7a", 0.8),
        K::Guard => t("#a8d888", "#2f5f1a", 1.4),
    }
}

fn chl_fill(pr: bool) -> &'static str { if pr { "#4d4d4d" } else { "#3d9a3a" } }

fn arrow_col(i: It, pr: bool) -> &'static str {
    if pr { return INK; }
    match i { It::Light => "#c98a00", It::Co2 => "#555555", It::O2 => "#1f8f84", _ => "#2b7bbd" }
}

// ================================================================== the whole figure

pub fn warnings(p: &Params) -> Vec<String> {
    let c = cfg(p);
    let mut out = Vec::new();
    if !c.open && (c.gas || c.stoma) {
        let (what, them) = match (c.gas, c.stoma) {
            (true, true) => ("the gas-exchange arrows and the Stoma label are left out", "them"),
            (true, false) => ("the gas-exchange arrows are left out", "them"),
            _ => ("the Stoma label is left out (Guard cell and Air space are still labelled)", "it"),
        };
        out.push(format!("The stomata are closed, so {what}: a closed stoma has no open pore to point at, and lets almost no carbon dioxide in or water vapour out. Set Stomata to Open to show {them}."));
    }
    if !ORDER.iter().any(|i| c.wants(*i)) {
        // only switches that label something in the current state are named
        let mut names: Vec<&str> = vec!["Label the layers", "Label a chloroplast", "Label the vein", "Label the stoma"];
        if c.open { names.push("Gas exchange arrows"); }
        names.push("Light arrow");
        let last = names.pop().unwrap_or_default();
        out.push(format!("Nothing is labelled, so the leaf is drawn bare. Switch on {} or {last}.", names.join(", ")));
    }
    out
}

/// The x extent and y extent of everything drawn around the section (section coordinates).
#[derive(Clone, Copy, Debug)]
struct Ext { x0: f64, y0: f64, x1: f64, y1: f64 }

impl Ext {
    fn add(&mut self, b: Bx) { self.x0 = self.x0.min(b.0); self.y0 = self.y0.min(b.1); self.x1 = self.x1.max(b.2); self.y1 = self.y1.max(b.3); }
}

/// The box of the surface view's caption and frame (section coordinates).
pub fn inset_boxes(lab: f64) -> (Bx, Bx) {
    let fs = 12.0 * lab;
    let cap = (RX, INSET_Y - 5.0 - 1.25 * fs, RX + rw(INSET_CAPTION, fs), INSET_Y - 5.0);
    (cap, (RX, INSET_Y, RX + INSET_W, INSET_Y + INSET_H))
}

pub const INSET_CAPTION: &str = "Seen from below";

fn draw(p: &Params) -> String {
    let c = cfg(p);
    let z = sizes(c.lab);
    let fills = svg::Fills::new(p);
    let pr = fills.is_print();
    let labs = labels(&c);
    let fl = flows(&c);
    // ---------- extents of the section and its labels
    let mut ext = Ext { x0: 0.0, y0: 0.0, x1: SW, y1: Y_LC1 };
    for l in &labs { ext.add(l.bx); }
    if c.layers { ext.add((BX_L - 1.0, 0.0, 0.0, 0.0)); }
    for f in &fl { ext.add((f.a.0.min(f.b.0) - 6.0, f.a.1.min(f.b.1) - 6.0, f.a.0.max(f.b.0) + 6.0, f.a.1.max(f.b.1) + 6.0)); }
    if c.surface { let (cap, fr) = inset_boxes(c.lab); ext.add(cap); ext.add(fr); }
    // ---------- the table of blanks (student copy)
    let rule_w = 150.0 * c.lab;
    let cell_w = 2.0 * z.r + 8.0 + rule_w;
    let n = labs.len();
    let cols = if c.key || n == 0 { 1 } else if n > 10 { 3 } else if n > 4 { 2 } else { 1 };
    let table_w = if c.key || n == 0 { 0.0 } else { cols as f64 * cell_w + (cols - 1) as f64 * 28.0 };
    let row_h = 2.0 * z.r + 10.0;
    let table_rows = if c.key { 0 } else { n.div_ceil(cols) };
    // ---------- captions
    let mut cap: Vec<String> = vec!["Not to scale: a simplified section through a leaf blade.".to_string()];
    if c.key {
        let acc: Vec<String> = labs.iter().map(|l| format!("{}: {}", name(l.it), accepted(l.it, c.open))).collect();
        if !acc.is_empty() { cap.push(format!("Also accepted: {}.", acc.join("; "))); }
        cap.push(if c.open { "The stomata are open: the guard cells have taken up water and swollen (turgid), so they bow apart and leave a pore." } else { "The stomata are closed: the guard cells have lost water and gone limp (flaccid), so they meet and the pore is shut." }.to_string());
        if c.gas_drawn() {
            cap.push("The arrows show gas exchange in daylight, when photosynthesis is faster than respiration: carbon dioxide diffuses in through an open stoma; oxygen and water vapour diffuse out.".to_string());
            cap.push("The two arrows out of the leaf, O~2 and water vapour, may be named in either order on a student copy.".to_string());
        }
        cap.push(format!("Chloroplasts ({}) are most crowded in the palisade mesophyll; the guard cells are the only epidermal cells with chloroplasts. Most stomata are on the lower surface.", if pr { "dark dots" } else { "green dots" }));
    } else if n > 0 {
        let mut how: Vec<&str> = Vec::new();
        if labs.iter().any(|l| matches!(l.join, Join::Bracket { .. })) { how.push("a bracket marks a layer or a whole structure"); }
        if labs.iter().any(|l| matches!(l.join, Join::Leader { .. })) { how.push("a line ends on a part"); }
        if labs.iter().any(|l| matches!(l.join, Join::Arrow)) { how.push("an arrow shows what moves in or out"); }
        let joined = match how.len() { 1 => how[0].to_string(), 2 => format!("{} and {}", how[0], how[1]), _ => format!("{}, {} and {}", how[0], how[1], how[2]) };
        let mut first = joined.chars();
        let cap0 = first.next().map(|f| f.to_uppercase().collect::<String>() + first.as_str()).unwrap_or_default();
        cap.push(format!("{cap0}. Write the name of each numbered part."));
        if c.gas_drawn() { cap.push(format!("The two arrows that point out of the leaf ({} and {}) may be named in either order.", num_of(&labs, It::O2), num_of(&labs, It::Water))); }
    }
    let sec_w = ext.x1 - ext.x0;
    let w = (sec_w + 2.0 * MARGIN).max(table_w + 2.0 * MARGIN).max(480.0);
    let ox = (w - sec_w) / 2.0 - ext.x0;
    let oy = MARGIN - ext.y0;
    let cap_w = w - 2.0 * MARGIN;
    let cap_lines: Vec<String> = cap.iter().flat_map(|s| wrap_rich(s, cap_w, 12.0 * c.lab)).collect();
    let mut y = oy + ext.y1 + 14.0;
    let table_y = y;
    if table_rows > 0 { y += table_rows as f64 * row_h + 10.0; }
    let cap_y = y;
    let total_h = cap_y + cap_lines.len() as f64 * 16.0 * c.lab + 8.0;
    let mut d = Doc::new(w, total_h);
    d.defs(&fills.defs());
    if c.key {
        let names: Vec<String> = labs.iter().map(|l| plain(name(l.it))).collect();
        d.describe("Leaf cross-section", &format!("A section through a leaf with {} stomata{}.", if c.open { "open" } else { "closed" }, if names.is_empty() { String::new() } else { format!(", labelled: {}", names.join(", ")) }));
    } else {
        d.describe("Leaf cross-section diagram", &format!("A leaf cross-section with {n} numbered blanks; the table under it has a rule for each."));
    }
    let o = |q: P| (q.0 + ox, q.1 + oy);
    let pts_s = |pts: &[P], dx: f64, dy: f64| pts.iter().map(|q| format!("{:.2},{:.2}", q.0 + dx, q.1 + dy)).collect::<Vec<_>>().join(" ");
    // ---------- the section
    let sc = scene(c.open);
    for sh in &sc.shapes {
        let t = tone(sh.k, pr);
        d.raw(&format!(r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="{}" stroke-linejoin="round"/>"#, pts_s(&sh.pts, ox, oy), t.fill, t.stroke, t.sw));
    }
    for ch in &sc.chl {
        let q = o(ch.c);
        d.raw(&format!(r#"<ellipse cx="{:.2}" cy="{:.2}" rx="{}" ry="{}" fill="{}"/>"#, q.0, q.1, ch.rx, ch.ry, chl_fill(pr)));
    }
    // ---------- the surface view
    if c.surface {
        let (capb, fr) = inset_boxes(c.lab);
        let (gx, gy) = (fr.0 + ox, fr.1 + oy);
        d.raw(&format!(r#"<rect x="{gx:.2}" y="{gy:.2}" width="{INSET_W}" height="{INSET_H}" fill="{}" stroke="{}" stroke-width="1"/>"#, if pr { "#ffffff" } else { "#f4f8ee" }, if pr { "#111111" } else { "#6b8f47" }));
        let (cres, walls, ich) = inset(c.open);
        let wt = tone(K::Lower, pr);
        for (a, b) in walls { d.line(a.0 + gx, a.1 + gy, b.0 + gx, b.1 + gy, wt.stroke, 0.9); }
        let gt = tone(K::Guard, pr);
        for cr in &cres { d.raw(&format!(r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="{}" stroke-linejoin="round"/>"#, pts_s(cr, gx, gy), gt.fill, gt.stroke, gt.sw)); }
        for q in ich { d.raw(&format!(r#"<ellipse cx="{:.2}" cy="{:.2}" rx="1.8" ry="1.3" fill="{}"/>"#, q.0 + gx, q.1 + gy, chl_fill(pr))); }
        d.raw(&rich(capb.0 + ox, capb.3 + oy - 0.3 * 12.0 * c.lab, 12.0 * c.lab, "start", "#222222", INSET_CAPTION));
    }
    // ---------- arrows
    for f in &fl { let (a, b) = (o(f.a), o(f.b)); d.arrow(a.0, a.1, b.0, b.1, arrow_col(f.it, pr), f.sw); }
    // ---------- brackets and leaders (plain lines: no white halo, which would erase the tissue under them)
    for l in &labs {
        match l.join {
            Join::Leader { from, to } => { let (a, b) = (o(from), o(to)); d.line(a.0, a.1, b.0, b.1, INK, 1.0); }
            Join::Bracket { x, y0, y1, tick, lead } => {
                let pts = [(x + tick, y0), (x, y0), (x, y1), (x + tick, y1)];
                d.raw(&format!(r#"<polyline points="{}" fill="none" stroke="{INK}" stroke-width="1.4" stroke-linejoin="round" stroke-linecap="round"/>"#, pts_s(&pts, ox, oy)));
                if let Some((a, b)) = lead { let (a, b) = (o(a), o(b)); d.line(a.0, a.1, b.0, b.1, INK, 1.0); }
            }
            Join::Arrow => {}
        }
    }
    // ---------- names or numbered discs
    for l in &labs {
        let b = (l.bx.0 + ox, l.bx.1 + oy, l.bx.2 + ox, l.bx.3 + oy);
        let cy = (b.1 + b.3) / 2.0;
        if c.key {
            let (x, anc) = match l.anc { Anc::Start => (b.0, "start"), Anc::End => (b.2, "end"), Anc::Mid => ((b.0 + b.2) / 2.0, "middle") };
            d.raw(&rich(x, cy + 0.35 * z.fs, z.fs, anc, INK, name(l.it)));
        } else {
            let x = match l.anc { Anc::Start => b.0 + z.r, Anc::End => b.2 - z.r, Anc::Mid => (b.0 + b.2) / 2.0 };
            disc(&mut d, x, cy, z.r, l.num, 0.9 * z.fs);
        }
    }
    // ---------- the table of blanks
    if table_rows > 0 {
        let x0 = (w - table_w) / 2.0;
        for (q, l) in labs.iter().enumerate() {
            let (col, row) = (q / table_rows, q % table_rows);
            let rx = x0 + col as f64 * (cell_w + 28.0);
            let ry = table_y + row as f64 * row_h + row_h / 2.0;
            disc(&mut d, rx + z.r, ry, z.r, l.num, 0.9 * z.fs);
            let xr = rx + 2.0 * z.r + 8.0;
            d.line(xr, ry + z.r * 0.6, xr + rule_w, ry + z.r * 0.6, INK, 1.1);
        }
    }
    for (i, line) in cap_lines.iter().enumerate() {
        d.raw(&rich(MARGIN, cap_y + 12.0 * c.lab + i as f64 * 16.0 * c.lab, 12.0 * c.lab, "start", "#222222", line));
    }
    d.finish()
}

/// The number of the blank for `it` (0 when it has none).
fn num_of(labs: &[Lab], it: It) -> usize { labs.iter().find(|l| l.it == it).map_or(0, |l| l.num) }

fn disc(d: &mut Doc, x: f64, y: f64, r: f64, n: usize, fs: f64) {
    d.circle(x, y, r, "#ffffff", INK, 1.2);
    d.raw(&format!(r##"<text x="{x:.2}" y="{:.2}" font-size="{fs:.2}" text-anchor="middle" font-family="{FONT}" font-weight="bold" fill="#000">{n}</text>"##, y + 0.34 * fs));
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "Leaf section, everything labelled", params: vec![] },
        Example { name: "The layers, student copy", params: vec![b("show_vein", false), b("show_stoma", false), b("show_gas", false), b("show_light", false), b("show_chloroplast", false), b(SHOW_ANSWERS, false)] },
        Example { name: "Gas exchange through a stoma, student copy", params: vec![b("show_layers", false), b("show_vein", false), b("show_chloroplast", false), b(SHOW_ANSWERS, false)] },
        Example { name: "Stomata closed", params: vec![s("stomata", "Closed"), b("show_gas", false), b("show_stoma", false)] },
        Example { name: "The vein: xylem and phloem", params: vec![b("show_layers", false), b("show_stoma", false), b("show_gas", false), b("show_light", false), b("show_chloroplast", false), b("show_surface", false)] },
    ])
}

#[cfg(test)]
mod tests;
