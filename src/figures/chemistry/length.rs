//! The length reading's caliper and micrometer (the ruler is drawn in `measure.rs`): a 150 mm
//! vernier caliper read to 0.1, 0.05 or 0.02 mm, and a 0-25 mm micrometer read to its 0.01 mm
//! marks or to an estimated 0.001 mm, each with an object drawn to size in its jaws, a zero error
//! (and the instrument drawn closed to show it), and a close-up of its scales drawn at a larger
//! scale rather than magnified, so every mark stays sharp in print.
//!
//! The physics, as the textbooks give it:
//! * Vernier caliper: the sliding scale has n divisions over (n - 1) mm (0.1 mm: 10 over 9 mm,
//!   0.02 mm: 50 over 49 mm) or over (2n - 1) mm against 2 mm main divisions (0.05 mm: 20 over
//!   39 mm). Least count = one main division less one sliding division. A reading R = M + j x LC
//!   puts sliding mark j exactly on a main-scale mark, so exactly one mark lines up (and mark n
//!   too when j = 0). Source: Mitutoyo, "Quick Guide to Precision Measuring Instruments" (vernier
//!   and micrometer sections), and any school physics practical text.
//! * Micrometer: 0.5 mm pitch, 50 thimble divisions, so 0.01 mm a division; whole millimetres
//!   above the sleeve's datum line, half millimetres below it. The thimble is a cylinder seen
//!   from the side, so its marks sit at y = r sin(phi) and bunch toward its edges. Turning it to
//!   open the jaws brings higher numbers down to the datum line (a right-hand screw), so the
//!   numbers above the line are the larger ones.
//! * Zero error: the reading with the jaws closed. The scale is offset by it (the slider's zero,
//!   or the sleeve's scale, sits that far from where the jaws close), and the length is the
//!   reading minus the zero error.
use super::*;
use crate::figures::interact::{Cursor, Handle};
use crate::figures::{ParamDef, Params};

pub const INSTRUMENTS: [&str; 3] = ["Ruler", "Caliper", "Micrometer"];
pub const LEAST: [&str; 3] = ["0.1 mm", "0.05 mm", "0.02 mm"];
pub const MIC_READ: [&str; 2] = ["Marks (0.01 mm)", "One estimated digit (0.001 mm)"];
pub const HELD: [&str; 5] = ["Nothing", "Block", "Cylinder", "Ball", "Wire"];
pub const VIEW: [&str; 3] = ["Instrument and close-up", "Close-up only", "Instrument only"];

/// The instrument choice; "Ruler" is the figure as it was.
pub fn instrument_param() -> ParamDef { ParamDef::choice("instrument", "Instrument", &INSTRUMENTS, "Ruler") }

/// The caliper's and micrometer's controls. Every one acts only off the ruler, so a ruler
/// (and every ruler saved or shared before them) draws as it did.
pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("least_count", "Least count", &LEAST, "0.05 mm")
            .hint("0.1 mm: 10 sliding-scale divisions over 9 mm. 0.05 mm: 20 over 39 mm. 0.02 mm: 50 over 49 mm."),
        ParamDef::bool("lc_print", "Least count on the slider", true).hint("Print the least count on the sliding scale, or leave it for students to work out."),
        ParamDef::choice("mic_read_to", "Read to", &MIC_READ, MIC_READ[0])
            .hint("To the thimble's marks, or one estimated digit between them."),
        ParamDef::float("cal_reading", "Reading", 0.0, 150.0, 0.01, 23.45),
        ParamDef::float("mic_reading", "Reading", 0.0, 25.0, 0.001, 7.38),
        ParamDef::float("zero_error", "Zero error", -1.0, 1.0, 0.001, 0.0)
            .hint("What the instrument reads with its jaws closed: positive past zero, negative short of it. The length is the reading minus this."),
        ParamDef::bool("show_closed", "Also show it closed", false).hint("Draw the closed instrument above the measuring one, to read the zero error from."),
        ParamDef::choice("held", "In the jaws", &HELD, "Block"),
        ParamDef::text("held_label", "Label", "").hint("A short name beside the object, such as Coin or Wire A."),
        ParamDef::choice("close_up", "Show", &VIEW, VIEW[0]),
    ]
}

pub fn is_caliper(p: &Params) -> bool { p.s("instrument") == "Caliper" }
pub fn is_micrometer(p: &Params) -> bool { p.s("instrument") == "Micrometer" }
pub fn is_ruler(p: &Params) -> bool { !is_caliper(p) && !is_micrometer(p) }

/// Only the chosen instrument's controls are shown (hidden ones keep their values).
pub fn relevant(key: &str, p: &Params) -> bool {
    let (cal, mic) = (is_caliper(p), is_micrometer(p));
    let ruler = !cal && !mic;
    match key {
        "units" | "ruler" | "marks" | "read_to" | "length" | "offset" | "object" | "dashes" | "mag_show" | "mag_marks" => ruler,
        "least_count" | "lc_print" | "cal_reading" => cal,
        "mic_read_to" | "mic_reading" => mic,
        "zero_error" | "show_closed" | "held" | "close_up" => !ruler,
        "held_label" => !ruler && p.s("held") != "Nothing",
        _ => true,
    }
}

// ---------------------------------------------------------------------------------------------
// Readings
// ---------------------------------------------------------------------------------------------

/// `v` to the nearest multiple of `q`, without float dust.
fn snap(v: f64, q: f64) -> f64 {
    let s = (v / q).round() * q;
    (s * 1e6).round() / 1e6
}

/// What the instrument shows: the reading `r` (on its scale), the zero error `z`, both in
/// multiples of the resolution `q` (mm), written to `dp` places.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Read { pub r: f64, pub z: f64, pub q: f64, pub dp: usize }

impl Read {
    /// The true gap between the jaws: the reading less the zero error.
    pub fn gap(&self) -> f64 { (self.r - self.z).max(0.0) }
    /// The same instrument with its jaws closed: it reads its zero error.
    fn closed(&self) -> Read { Read { r: self.z, ..*self } }
}

/// The caliper's sliding scale: `n` divisions over `span` mm.
#[derive(Clone, Copy, Debug)]
pub struct Vern { pub n: usize, pub span: f64, pub lc: f64 }
impl Vern {
    pub fn div(&self) -> f64 { self.span / self.n as f64 }
    /// Divisions between numbered marks: the numbers count tenths of a millimetre.
    fn lab_every(&self) -> usize { self.n / 10 }
}

pub fn vern(p: &Params) -> Vern {
    match p.s("least_count") {
        "0.1 mm" => Vern { n: 10, span: 9.0, lc: 0.1 },
        "0.02 mm" => Vern { n: 50, span: 49.0, lc: 0.02 },
        _ => Vern { n: 20, span: 39.0, lc: 0.05 },
    }
}

pub fn cal_read(p: &Params) -> Read {
    let lc = vern(p).lc;
    let z = snap(p.f("zero_error").clamp(-1.0, 1.0), lc);
    let r = snap(p.f("cal_reading").clamp(0.0, 150.0), lc).max(z);
    Read { r, z, q: lc, dp: if lc >= 0.1 { 1 } else { 2 } }
}

fn mic_est(p: &Params) -> bool { p.s("mic_read_to") == MIC_READ[1] }

pub fn mic_read(p: &Params) -> Read {
    let q = if mic_est(p) { 0.001 } else { 0.01 };
    let z = snap(p.f("zero_error").clamp(-0.5, 0.5), q);
    let r = snap(p.f("mic_reading").clamp(0.0, 25.0), q).max(z);
    Read { r, z, q, dp: if mic_est(p) { 3 } else { 2 } }
}

pub fn read_of(p: &Params) -> Read { if is_caliper(p) { cal_read(p) } else { mic_read(p) } }

/// "+0.10" / "−0.08" (a real minus sign).
fn signed(v: f64, dp: usize) -> String {
    if v < 0.0 { format!("\u{2212}{}", num(-v, dp)) } else { format!("+{}", num(v, dp)) }
}

/// The answer key: the reading, and with a zero error the correction and the length.
pub fn key_lines(p: &Params) -> Vec<String> {
    if !p.b("answer") { return vec![]; }
    let rd = read_of(p);
    let mut v = vec![format!("Reading: {} mm", num(rd.r, rd.dp))];
    if rd.z.abs() > 1e-9 {
        let (op, mag) = if rd.z > 0.0 { ("\u{2212}", rd.z) } else { ("+", -rd.z) };
        v.push(format!("Zero error: {} mm, so the length is {} {op} {} = {} mm", signed(rd.z, rd.dp), num(rd.r, rd.dp), num(mag, rd.dp), num(rd.gap(), rd.dp)));
    }
    v
}

/// A random reading on the chosen instrument, at its resolution.
pub fn random(p: &mut Params, seed: u64) -> bool {
    let u = crate::figures::unit_random(seed);
    if is_caliper(p) {
        let lc = vern(p).lc;
        p.set_f("cal_reading", snap(2.0 + u * 138.0, lc));
    } else if is_micrometer(p) {
        let q = if mic_est(p) { 0.001 } else { 0.01 };
        p.set_f("mic_reading", snap(0.5 + u * 24.0, q));
    } else {
        return false;
    }
    true
}

/// One-line summaries under the caliper's and micrometer's section headers (and the instrument's).
pub fn summary(title: &str, p: &Params) -> Option<String> {
    if title == "Instrument" { return Some(p.s("instrument").to_string()); }
    if is_ruler(p) { return None; }
    let rd = read_of(p);
    Some(match title {
        "Caliper" => format!("{} least count{}", p.s("least_count"), if p.b("lc_print") { ", printed" } else { "" }),
        "Micrometer" => if mic_est(p) { "To 0.001 mm, estimated".to_string() } else { "To 0.01 mm".to_string() },
        "Reading" => {
            let mut t = format!("{} mm", num(rd.r, rd.dp));
            if rd.z.abs() > 1e-9 { t.push_str(&format!(", zero error {} mm", signed(rd.z, rd.dp))); }
            if p.b("show_closed") { t.push_str(", closed too"); }
            t
        }
        "In the jaws" => {
            let h = p.s("held");
            let l = p.s("held_label").trim();
            if h == "Nothing" || l.is_empty() { h.to_string() } else { format!("{h}, \u{201c}{l}\u{201d}") }
        }
        "Close-up" => p.s("close_up").to_string(),
        _ => return None,
    })
}

/// The Reading section's range line.
pub fn range_note(p: &Params) -> Option<String> {
    let base = range_text(p)?;
    // a typed reading the scale cannot show is drawn at the nearest one: say so, since the box still shows what was typed
    let (typed, rd) = if is_caliper(p) { (p.f("cal_reading"), cal_read(p)) } else { (p.f("mic_reading"), mic_read(p)) };
    if (typed - rd.r).abs() > 1e-9 {
        return Some(format!("{base} {} mm is drawn as {} mm, the nearest reading this scale shows.", fmt_typed(typed), num(rd.r, rd.dp)));
    }
    Some(base)
}

/// A typed number as the teacher typed it, without trailing zeros.
fn fmt_typed(v: f64) -> String {
    let t = format!("{v:.6}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    t.replace('-', "\u{2212}")
}

fn range_text(p: &Params) -> Option<String> {
    if is_caliper(p) {
        let v = vern(p);
        let lc = num(v.lc, if v.lc >= 0.1 { 1 } else { 2 });
        Some(format!("From 0 to 150 mm, to {lc} mm, and the zero error to {lc} mm too. You can also drag the slider in the figure."))
    } else if is_micrometer(p) {
        let rd = mic_read(p);
        Some(format!("From 0 to 25 mm, to {q} mm, and the zero error from \u{2212}0.5 to 0.5 mm to {q} mm. You can also drag the thimble in the figure.", q = num(rd.q, rd.dp)))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------------------------

const STEEL: &str = "#cfd4d9";
const STEEL_DK: &str = "#9ea6ae";
const FRAME: &str = "#b7bdc3";
const BEAM: &str = "#eceef0";
const PLATE: &str = "#dfe3e7";
const SLEEVE: &str = "#e9ecef";
const THIMBLE: &str = "#d3d8dd";

/// Caliper: canvas width, x of the main scale's 0 (the fixed jaw's face), px per mm.
const CW: f64 = 780.0;
const CX0: f64 = 70.0;
const CK: f64 = 3.2;
/// Caliper close-up: height, widest it gets, most px per mm.
const CPH: f64 = 150.0;
const CPW: f64 = 720.0;
/// Micrometer: canvas width, anvil face x, px per mm, the frame's hub, the sleeve's start, where the
/// sleeve's 0 mark sits with no zero error, the thimble's radius.
const MW: f64 = 720.0;
const MXA: f64 = 80.0;
const MK: f64 = 8.0;
const MXH: f64 = MXA + 25.0 * MK + 22.0;
const MXS: f64 = MXH + 46.0;
const MXZ0: f64 = MXS + 10.0;
const MRT: f64 = 22.0;
/// Micrometer close-up: height, px per mm, thimble radius, how much of the thimble shows.
const MPH: f64 = 250.0;
const MPK: f64 = 36.0;
const MPRT: f64 = 110.0;
const MPTV: f64 = 170.0;
const PANEL_GAP: f64 = 22.0;
const CAPTION_H: f64 = 24.0;
/// One thimble division as an angle (50 divisions a turn).
const THETA: f64 = std::f64::consts::TAU / 50.0;

fn view_instr(p: &Params) -> bool { p.s("close_up") != VIEW[1] }
fn view_close(p: &Params) -> bool { p.s("close_up") != VIEW[2] }

/// One instrument drawn: a caption when the closed copy is shown, its close-up (above) and itself.
#[derive(Clone, Debug)]
struct Block { rd: Read, closed: bool, caption: Option<&'static str>, panel_y: Option<f64>, instr_y: Option<f64> }

struct Layout { w: f64, h: f64, blocks: Vec<Block> }

/// What is in the jaws: nothing on the closed copy, or when they are shut.
fn held<'a>(p: &'a Params, rd: &Read, closed: bool) -> &'a str {
    if closed || rd.gap() <= 1e-9 { "Nothing" } else { p.s("held") }
}

fn label_of<'a>(p: &'a Params, rd: &Read, closed: bool) -> Option<&'a str> {
    let t = p.s("held_label").trim();
    (held(p, rd, closed) != "Nothing" && !t.is_empty()).then_some(t)
}

fn layout(p: &Params) -> Layout {
    let cal = is_caliper(p);
    let rd = read_of(p);
    let both = p.b("show_closed");
    let mut list = vec![];
    if both { list.push((rd.closed(), true)); }
    list.push((rd, false));
    let mut y = 8.0;
    let mut blocks = vec![];
    for (r, closed) in list {
        let caption = both.then_some(if closed { "Closed" } else { "Measuring" });
        let mut b = Block { rd: r, closed, caption, panel_y: None, instr_y: None };
        if caption.is_some() { y += CAPTION_H; }
        if view_close(p) {
            b.panel_y = Some(y);
            y += if cal { CPH } else { MPH };
            if view_instr(p) { y += PANEL_GAP; }
        }
        if view_instr(p) {
            b.instr_y = Some(y);
            y += if cal { cal_h(p, &r, closed) } else { mic_h(p, &r, closed) };
        }
        y += 10.0;
        blocks.push(b);
    }
    Layout { w: canvas_w(p), h: y, blocks }
}

/// The canvas width: the instrument's, or with the close-ups alone the widest of them and a margin.
fn canvas_w(p: &Params) -> f64 {
    let cal = is_caliper(p);
    if view_instr(p) { return if cal { CW } else { MW }; }
    let rd = read_of(p);
    let mut reads = vec![rd];
    if p.b("show_closed") { reads.push(rd.closed()); }
    let v = vern(p);
    reads.iter().map(|r| if cal { cal_pw(r, &v).0 } else { mic_pw(r) }).fold(0.0, f64::max) + 20.0
}

/// A thimble: filled, outlined on its top, bottom and right only. Its left edge is where its fill
/// begins, so a sleeve mark a hair before it (5.51 mm shows the 5.5 mark) is not lost under a border.
fn open_rect(d: &mut Doc, x: f64, y: f64, w: f64, h: f64, fill: &str, sw: f64) {
    d.rect(x, y, w, h, fill, "none", 0.0);
    d.raw(&format!(r#"<polyline points="{x:.3},{y:.2} {:.2},{y:.2} {:.2},{:.2} {x:.3},{:.2}" fill="none" stroke="{INK}" stroke-width="{sw}"/>"#, x + w, x + w, y + h, y + h));
}

/// The sleeve from `x0` to the thimble's edge `xe`: outlined top and bottom only (its ends are under
/// the hub and the thimble).
fn sleeve(d: &mut Doc, x0: f64, xe: f64, ym: f64, hh: f64, sw: f64) {
    d.rect(x0, ym - hh, xe - x0, 2.0 * hh, SLEEVE, "none", 0.0);
    d.line(x0, ym - hh, xe, ym - hh, INK, sw);
    d.line(x0, ym + hh, xe, ym + hh, INK, sw);
}

fn tick(d: &mut Doc, class: &str, x1: f64, y1: f64, x2: f64, y2: f64, sw: f64) {
    d.raw(&format!(r#"<line class="{class}" x1="{x1:.3}" y1="{y1:.3}" x2="{x2:.3}" y2="{y2:.3}" stroke="{INK}" stroke-width="{sw}"/>"#));
}

fn dashed_rect(d: &mut Doc, x: f64, y: f64, w: f64, h: f64) {
    d.raw(&format!(r#"<rect x="{x:.2}" y="{y:.2}" width="{w:.2}" height="{h:.2}" fill="none" stroke="{INK}" stroke-width="1.1" stroke-dasharray="5 4"/>"#));
}

/// Estimated width of `s` in Arial at `size` px.
fn text_w(s: &str, size: f64) -> f64 {
    s.chars().map(|c| match c {
        'i' | 'l' | 'j' | '.' | ',' | ':' | ';' | '!' | '|' | '\'' | 'I' => 0.28,
        'f' | 't' | 'r' | '(' | ')' | ' ' | '-' => 0.36,
        'm' | 'w' | 'M' | 'W' => 0.86,
        c if c.is_ascii_uppercase() => 0.68,
        c if c.is_ascii_digit() => 0.56,
        _ => 0.56,
    }).sum::<f64>() * size
}

/// The object's label at `size`, shrunk (to 9 px at least) and then cut to fit `maxw`.
fn fit(s: &str, size: f64, maxw: f64) -> (String, f64) {
    let w = text_w(s, size);
    if w <= maxw { return (s.to_string(), size); }
    let sz = (size * maxw / w).max(9.0);
    if text_w(s, sz) <= maxw { return (s.to_string(), sz); }
    let mut t: String = String::new();
    for c in s.chars() {
        if text_w(&format!("{t}{c}\u{2026}"), sz) > maxw { break; }
        t.push(c);
    }
    (format!("{}\u{2026}", t.trim_end()), sz)
}

/// The object's label centred on `x`, kept inside the canvas.
fn object_label(d: &mut Doc, w: f64, x: f64, y: f64, s: &str) {
    let (t, sz) = fit(s, 12.0, w - 24.0);
    let tw = text_w(&t, sz);
    let x = x.clamp(12.0 + tw / 2.0, w - 12.0 - tw / 2.0);
    tx(d, x, y, sz, "middle", INK, false, &t);
}

/// (fill, edge) of an object in the jaws.
fn obj_colors(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "Block" => ("#d9c08a", "#5f4b1f"),
        "Cylinder" => ("#b4b4b4", "#3a3a3a"),
        "Ball" => ("#8fb4d6", "#2e4f6e"),
        _ => ("#c8834a", "#6b3d17"),
    }
}

/// An object `w` px across between x0 and x0 + w, centred on `cy`, `hh` px above and below it
/// (a ball's radius is its own), drawn as a block, a cylinder standing on end, a ball or a wire.
fn draw_object(d: &mut Doc, kind: &str, x0: f64, w: f64, cy: f64, hh: f64) {
    let (fill, edge) = obj_colors(kind);
    match kind {
        "Block" => { d.rect(x0, cy - hh, w, 2.0 * hh, fill, edge, 1.4); }
        "Cylinder" => {
            let ry = (w * 0.18).min(6.0);
            d.raw(&format!(r#"<path d="M{:.2} {:.2} L{:.2} {:.2} A{:.2} {ry:.2} 0 0 0 {:.2} {:.2} L{:.2} {:.2} A{:.2} {ry:.2} 0 0 0 {:.2} {:.2} Z" fill="{fill}" stroke="{edge}" stroke-width="1.4"/>"#,
                x0, cy - hh, x0, cy + hh, w / 2.0, x0 + w, cy + hh, x0 + w, cy - hh, w / 2.0, x0, cy - hh));
            d.raw(&format!(r##"<ellipse cx="{:.2}" cy="{:.2}" rx="{:.2}" ry="{ry:.2}" fill="#e3e3e3" stroke="{edge}" stroke-width="1.2"/>"##, x0 + w / 2.0, cy - hh, w / 2.0));
        }
        "Ball" => {
            let r = w / 2.0;
            d.circle(x0 + r, cy, r, fill, edge, 1.4);
            if r > 6.0 { d.circle(x0 + r * 0.65, cy - r * 0.38, r * 0.2, "#d9e9f6", "none", 0.0); }
        }
        _ => { d.rect(x0, cy - hh, w, 2.0 * hh, fill, edge, if w < 3.0 { 0.6 } else { 1.2 }); }
    }
}

// ---------------------------------------------------------------------------------------------
// Caliper
// ---------------------------------------------------------------------------------------------

fn x_cal(m: f64) -> f64 { CX0 + m * CK }

/// The jaws' object: (half height, centre y) below the slider, as drawn from the beam's top `yb`.
fn cal_obj(kind: &str, w: f64, yb: f64) -> (f64, f64) {
    match kind {
        "Block" => (25.0, yb + 99.0),
        "Cylinder" => (28.0, yb + 98.0),
        "Ball" => { let r = w / 2.0; (r, (yb + 97.0).max(yb + 65.0 + r)) }
        "Wire" => (42.0, yb + 108.0),
        _ => (0.0, yb + 97.0),
    }
}

/// Height of a drawn caliper (its nibs to the lowest of jaws, object and label).
fn cal_h(p: &Params, rd: &Read, closed: bool) -> f64 {
    let yb = 40.0;
    let kind = held(p, rd, closed);
    let (hh, cy) = cal_obj(kind, rd.gap() * CK, yb);
    let mut bottom = (yb + 134.0).max(if kind == "Nothing" { 0.0 } else { cy + hh + 4.0 });
    if label_of(p, rd, closed).is_some() { bottom = bottom.max(yb + 130.0).max(cy + hh) + 22.0; }
    bottom + 4.0
}

/// The part of the main scale the close-up shows: from the numbered mark before the reading (and
/// two millimetres back at least) to past the sliding scale's end.
fn cal_window(rd: &Read, v: &Vern) -> (f64, f64) {
    // (near zero the 0 mark itself is the numbered one, and nothing lies before -2 mm)
    let a = (rd.r.floor() - 2.0).min((rd.r / 10.0).floor() * 10.0 - 1.0).max(rd.r.min(0.0) - 2.5);
    (a, rd.r + v.span + 2.5)
}

fn caliper(d: &mut Doc, p: &Params, rd: &Read, oy: f64, closed: bool, region: bool) {
    let v = vern(p);
    let yb = oy + 40.0;
    let (xv, xj) = (x_cal(rd.r), x_cal(rd.gap()));
    let beam_end = x_cal(215.0);
    // the slider's left and right sides (outside its window onto the main scale)
    let pl = xj.min(xv - 6.0);
    let lc_txt = format!("{} mm", num(v.lc, if v.lc >= 0.1 { 1 } else { 2 }));
    let pr = x_cal(rd.r + v.span) + if p.b("lc_print") { 48.0 } else { 12.0 };
    // a main-scale number half under a side would read as another number: the side hides it whole
    let hidden = |x0: f64, x1: f64| [(pl - 5.0, pl + 1.0), (pr - 1.0, pr + 5.0)].iter().any(|&(a, b)| x1 > a && x0 < b);
    // the fixed jaws and the beam
    d.poly(&[(CX0 - 14.0, yb), (CX0, yb), (CX0, yb - 30.0), (CX0 - 3.0, yb - 34.0), (CX0 - 14.0, yb - 18.0)], STEEL, INK, 1.4);
    d.poly(&[(CX0 - 46.0, yb), (CX0, yb), (CX0, yb + 128.0), (CX0 - 4.0, yb + 132.0), (CX0 - 12.0, yb + 132.0), (CX0 - 46.0, yb + 64.0)], STEEL, INK, 1.6);
    d.raw(&format!(r#"<rect x="{:.2}" y="{yb:.2}" width="{:.2}" height="34" rx="2" fill="{BEAM}" stroke="{INK}" stroke-width="1.6"/>"#, CX0, beam_end - CX0));
    for m in 0..=200 {
        let x = x_cal(m as f64);
        let len = if m % 10 == 0 { 11.0 } else if m % 5 == 0 { 8.0 } else { 5.0 };
        tick(d, "ms", x, yb + 34.0, x, yb + 34.0 - len, 0.8);
        if m % 10 == 0 {
            // the 0 sits just right of its mark, clear of the fixed jaw's edge
            let t = m.to_string();
            let w = text_w(&t, 9.0);
            let x0 = if m == 0 { x + 2.5 } else { x - w / 2.0 };
            if !hidden(x0, x0 + w) {
                if m == 0 { tx(d, x0, yb + 19.0, 9.0, "start", INK, false, &t) } else { tx(d, x, yb + 19.0, 9.0, "middle", INK, false, &t) }
            }
        }
    }
    tx(d, x_cal(208.0), yb + 19.0, 9.0, "middle", INK, false, "mm");
    // the slider: its outside jaw, upper nib, top bar and the plate carrying the sliding scale
    d.poly(&[(xj, yb + 58.0), (xj + 46.0, yb + 58.0), (xj + 46.0, yb + 64.0), (xj + 12.0, yb + 132.0), (xj + 4.0, yb + 132.0), (xj, yb + 128.0)], STEEL, INK, 1.6);
    d.poly(&[(xj, yb - 6.0), (xj + 14.0, yb - 6.0), (xj + 14.0, yb - 18.0), (xj + 3.0, yb - 34.0), (xj, yb - 30.0)], STEEL, INK, 1.4);
    d.rect(pl, yb - 7.0, pr - pl, 7.0, PLATE, INK, 1.4);
    d.rect(xj + 22.0, yb - 13.0, 14.0, 6.0, STEEL_DK, INK, 1.0);
    // the slider's sides, outside its window onto the main scale
    d.rect(pl - 4.0, yb - 7.0, 4.0, 41.0, PLATE, INK, 1.2);
    d.rect(pr, yb - 7.0, 4.0, 41.0, PLATE, INK, 1.2);
    d.rect(pl, yb + 34.0, pr - pl, 28.0, PLATE, INK, 1.4);
    d.circle(xj + 60.0, yb + 68.0, 6.0, STEEL_DK, INK, 1.2);
    let every = if v.n == 20 { 4 } else { 5 };
    for j in 0..=v.n {
        let x = x_cal(rd.r + j as f64 * v.div());
        let len = if j % v.lab_every() == 0 { 8.0 } else { 5.0 };
        tick(d, "vs", x, yb + 34.0, x, yb + 34.0 + len, 0.8);
        if j % every == 0 { tx(d, x, yb + 52.0, 8.5, "middle", INK, false, &(j / v.lab_every()).to_string()); }
    }
    if p.b("lc_print") { tx(d, x_cal(rd.r + v.span) + 26.0, yb + 52.0, 8.5, "middle", INK, false, &lc_txt); }
    // the object, drawn to size between the jaws, and its label
    let kind = held(p, rd, closed);
    if kind != "Nothing" {
        let w = rd.gap() * CK;
        let (hh, cy) = cal_obj(kind, w, yb);
        draw_object(d, kind, CX0, w, cy, hh);
        if let Some(t) = label_of(p, rd, closed) {
            let y = (yb + 130.0).max(cy + hh) + 18.0;
            object_label(d, CW, CX0 + w / 2.0, y, t);
        }
    }
    if region {
        let (a, b) = cal_window(rd, &v);
        dashed_rect(d, x_cal(a), yb - 9.0, (b - a) * CK, 73.0);
    }
}

/// The close-up: (left x, width, px per mm) of the panel for this reading.
/// The close-up's (width, px per mm).
fn cal_pw(rd: &Read, v: &Vern) -> (f64, f64) {
    let (a, b) = cal_window(rd, v);
    let k = (CPW / (b - a)).min(40.0);
    ((b - a) * k, k)
}

/// The close-up's (left x, width, px per mm): over the part of the instrument it shows, or
/// centred on a canvas of close-ups alone.
fn cal_panel_geo(p: &Params, rd: &Read, v: &Vern) -> (f64, f64, f64) {
    let (a, b) = cal_window(rd, v);
    let (pw, k) = cal_pw(rd, v);
    let x = if view_instr(p) { centred(x_cal((a + b) / 2.0), pw, CW) } else { (canvas_w(p) - pw) / 2.0 };
    (x, pw, k)
}

/// The left x of a panel `pw` wide centred over `cx`, kept 10 px inside a canvas `w` wide.
fn centred(cx: f64, pw: f64, w: f64) -> f64 { (cx - pw / 2.0).clamp(10.0, (w - 10.0 - pw).max(10.0)) }

fn caliper_panel(d: &mut Doc, p: &Params, rd: &Read, py: f64, id: usize) {
    let v = vern(p);
    let (a, b) = cal_window(rd, &v);
    let (px0, pw, k) = cal_panel_geo(p, rd, &v);
    let x = |m: f64| px0 + (m - a) * k;
    let bo = py + 80.0;
    d.raw(&format!(r##"<defs><clipPath id="lrcu{id}"><rect x="{px0:.2}" y="{py:.2}" width="{pw:.2}" height="{CPH}" rx="6"/></clipPath></defs><g clip-path="url(#lrcu{id})"><rect x="{px0:.2}" y="{py:.2}" width="{pw:.2}" height="{CPH}" fill="#fff"/>"##));
    d.rect(px0 - 2.0, py + 6.0, pw + 4.0, bo - py - 6.0, BEAM, INK, 1.2);
    let m0 = a.ceil().max(0.0) as i64;
    let m1 = b.floor().min(200.0) as i64;
    for m in m0..=m1 {
        let xm = x(m as f64);
        let len = if m % 10 == 0 { 32.0 } else if m % 5 == 0 { 24.0 } else { 16.0 };
        tick(d, "cms", xm, bo, xm, bo - len, 1.3);
        if m % 10 == 0 { tx(d, xm, bo - 40.0, 15.0, "middle", INK, false, &m.to_string()); }
    }
    let (pl, pr) = (x(rd.r) - 14.0, x(rd.r + v.span) + 14.0);
    d.rect(pl, bo, pr - pl, 68.0, PLATE, INK, 1.4);
    for j in 0..=v.n {
        let xj = x(rd.r + j as f64 * v.div());
        let len = if j % (5 * v.lab_every()) == 0 { 30.0 } else if j % v.lab_every() == 0 { 24.0 } else { 15.0 };
        tick(d, "cvs", xj, bo, xj, bo + len, 1.1);
        if j % v.lab_every() == 0 { tx(d, xj, bo + 44.0, 13.0, "middle", INK, false, &(j / v.lab_every()).to_string()); }
    }
    if p.b("lc_print") {
        tx(d, x(rd.r + v.span / 2.0), bo + 62.0, 12.0, "middle", INK, false, &format!("{} mm", num(v.lc, if v.lc >= 0.1 { 1 } else { 2 })));
    }
    d.raw("</g>");
    d.raw(&format!(r#"<rect x="{px0:.2}" y="{py:.2}" width="{pw:.2}" height="{CPH}" rx="6" fill="none" stroke="{INK}" stroke-width="2"/>"#));
}

/// Dashed lines from the instrument's marked region up to its close-up's bottom corners.
fn joins(d: &mut Doc, region: (f64, f64, f64), panel: (f64, f64, f64)) {
    let (rx0, rx1, ry) = region;
    let (px0, px1, py) = panel;
    for (a, b) in [(rx0, px0), (rx1, px1)] {
        d.raw(&format!(r#"<line x1="{a:.2}" y1="{ry:.2}" x2="{b:.2}" y2="{py:.2}" stroke="{INK}" stroke-width="1.1" stroke-dasharray="5 4"/>"#));
    }
}

// ---------------------------------------------------------------------------------------------
// Micrometer
// ---------------------------------------------------------------------------------------------

/// (x of the sleeve's 0 mark, x of the thimble's edge, x of the spindle's face). The sleeve's scale
/// is offset by the zero error; the thimble's edge and the spindle move together.
fn mic_x(rd: &Read) -> (f64, f64, f64) {
    (MXZ0 - rd.z * MK, MXZ0 + rd.gap() * MK, MXA + rd.gap() * MK)
}

/// The thimble division on the datum line (fractional when estimated).
pub fn thimble_t(rd: &Read) -> f64 { ((rd.r.rem_euclid(0.5) / 0.01) * 1000.0).round() / 1000.0 }

/// The angle of thimble mark `j` from the datum line, in (-pi, pi].
fn mark_phi(j: usize, t: f64) -> f64 {
    let mut u = (j as f64 - t).rem_euclid(50.0);
    if u > 25.0 { u -= 50.0; }
    u * THETA
}

/// Half height of the object in the jaws (a ball's is its radius).
fn mic_obj(kind: &str, w: f64) -> f64 {
    match kind { "Block" => 22.0, "Cylinder" => 30.0, "Ball" => w / 2.0, "Wire" => 44.0, _ => 0.0 }
}

/// The axis's y below the top of a drawn micrometer: room for the lock lever, the object and its label.
fn mic_top(p: &Params, rd: &Read, closed: bool) -> f64 {
    let hh = mic_obj(held(p, rd, closed), rd.gap() * MK);
    if label_of(p, rd, closed).is_some() { 40f64.max(hh.max(34.0) + 30.0) } else { 40f64.max(hh + 8.0) }
}

fn mic_h(p: &Params, rd: &Read, closed: bool) -> f64 {
    let hh = mic_obj(held(p, rd, closed), rd.gap() * MK);
    mic_top(p, rd, closed) + 100f64.max(hh + 6.0) + 4.0
}

/// Where a sleeve number centred on its mark at `x` goes: moved left just enough to clear the
/// thimble's edge `xe`, so a reading on a multiple of 5 mm does not hide half its number (10.000 mm
/// showed only "1", which a student could read as 1).
fn num_x(x: f64, xe: f64, size: f64, m: i64) -> f64 {
    let half = 0.3 * size * m.to_string().len() as f64;
    x.min(xe - 1.5 - half)
}

/// Where the sleeve begins, in mm on its scale (the frame's hub is before it).
fn sleeve_start(rd: &Read) -> f64 { -(MXZ0 - MXS) / MK - rd.z }

/// The first millimetre the close-up shows: a numbered mark (or 2.5 mm) before the reading, and
/// no more than a little of the frame's hub.
fn mic_window(rd: &Read) -> f64 { ((rd.r / 5.0).floor() * 5.0 - 1.0).min(rd.r - 2.5).max(sleeve_start(rd) - 0.8) }

fn micrometer(d: &mut Doc, p: &Params, rd: &Read, oy: f64, closed: bool, region: bool) {
    let ya = oy + mic_top(p, rd, closed);
    let (xz, xe, xs) = mic_x(rd);
    // the frame, anvil and spindle
    d.poly(&[(MXA - 64.0, ya - 22.0), (MXA - 30.0, ya - 22.0), (MXA - 30.0, ya + 62.0), (MXH, ya + 62.0), (MXH, ya - 26.0), (MXS, ya - 26.0), (MXS, ya + 98.0), (MXA - 64.0, ya + 98.0)], FRAME, INK, 1.6);
    d.rect(MXA - 30.0, ya - 11.0, 30.0, 22.0, STEEL, INK, 1.4);
    if xs < MXH { d.rect(xs, ya - 10.0, MXH - xs, 20.0, STEEL, INK, 1.4); }
    d.rect(MXH + 12.0, ya - 34.0, 22.0, 8.0, STEEL_DK, INK, 1.2);
    // the sleeve: whole millimetres above the datum line, half millimetres below
    sleeve(d, MXS, xe, ya, 15.0, 1.4);
    tick(d, "dl", MXS, ya, xe, ya, 1.1);
    for m in 0..=25 {
        let mm = m as f64;
        if mm <= rd.r + 1e-9 {
            let x = xz + mm * MK;
            tick(d, "sl", x, ya, x, ya - if m % 5 == 0 { 7.0 } else { 5.0 }, 0.9);
            if m % 5 == 0 { tx(d, num_x(x, xe, 8.5, m), ya - 8.0, 8.5, "middle", INK, false, &m.to_string()); }
        }
        if m < 25 && mm + 0.5 <= rd.r + 1e-9 {
            let x = xz + (mm + 0.5) * MK;
            tick(d, "sh", x, ya, x, ya + 5.0, 0.9);
        }
    }
    // the thimble, its marks bunching toward its edges, then the ratchet
    open_rect(d, xe, ya - MRT, 110.0, 2.0 * MRT, THIMBLE, 1.4);
    d.line(xe + 2.5, ya - MRT, xe + 2.5, ya + MRT, STEEL_DK, 1.0);
    for i in 0..12 {
        let x = xe + 62.0 + i as f64 * 4.0;
        d.line(x, ya - MRT + 2.0, x, ya + MRT - 2.0, STEEL_DK, 0.8);
    }
    d.rect(xe + 110.0, ya - 12.0, 30.0, 24.0, STEEL, INK, 1.4);
    let t = thimble_t(rd);
    for j in 0..50 {
        let phi = mark_phi(j, t);
        if phi.abs() < 1.35 {
            let y = ya - MRT * phi.sin();
            tick(d, "tm", xe, y, xe + if j % 5 == 0 { 8.0 } else { 5.0 }, y, 0.8);
            if j % 5 == 0 && phi.abs() < 0.75 { tx(d, xe + 10.5, y + 3.0, 8.5, "start", INK, false, &j.to_string()); }
        }
    }
    // the object, drawn to size between the anvil and the spindle, and its label above it
    let kind = held(p, rd, closed);
    if kind != "Nothing" {
        let w = rd.gap() * MK;
        let hh = mic_obj(kind, w);
        draw_object(d, kind, MXA, w, ya, hh);
        // above the object and clear of the lock lever and the sleeve's numbers
        if let Some(lab) = label_of(p, rd, closed) { object_label(d, MW, MXA + w / 2.0, ya - hh.max(34.0) - 9.0, lab); }
    }
    if region {
        let a = mic_window(rd);
        dashed_rect(d, xz + a * MK, ya - 27.0, xe + 34.0 - (xz + a * MK), 54.0);
    }
}

fn mic_pw(rd: &Read) -> f64 { (rd.r - mic_window(rd)) * MPK + MPTV }

fn mic_panel_geo(p: &Params, rd: &Read) -> (f64, f64) {
    let pw = mic_pw(rd);
    let (xz, xe, _) = mic_x(rd);
    (if view_instr(p) { centred((xz + mic_window(rd) * MK + xe + 34.0) / 2.0, pw, MW) } else { (canvas_w(p) - pw) / 2.0 }, pw)
}

fn micrometer_panel(d: &mut Doc, p: &Params, rd: &Read, py: f64, id: usize) {
    let a = mic_window(rd);
    let (px0, pw) = mic_panel_geo(p, rd);
    let x = |m: f64| px0 + (m - a) * MPK;
    let ym = py + MPH / 2.0;
    let xe = x(rd.r);
    d.raw(&format!(r##"<defs><clipPath id="lrcu{id}"><rect x="{px0:.2}" y="{py:.2}" width="{pw:.2}" height="{MPH}" rx="6"/></clipPath></defs><g clip-path="url(#lrcu{id})"><rect x="{px0:.2}" y="{py:.2}" width="{pw:.2}" height="{MPH}" fill="#fff"/>"##));
    // the frame's hub where the sleeve begins (1.25 mm before its 0 mark, shifted by the zero error)
    let start = sleeve_start(rd);
    if start > a { d.rect(px0 - 2.0, ym - 96.0, x(start) - px0 + 2.0, 192.0, FRAME, INK, 1.4); }
    let s0 = x(start.max(a)) - if start > a { 0.0 } else { 2.0 };
    sleeve(d, s0, xe, ym, 62.0, 1.4);
    tick(d, "cdl", s0, ym, xe, ym, 1.6);
    let m0 = a.ceil().max(0.0) as i64;
    for m in m0..=25 {
        let mm = m as f64;
        if mm <= rd.r + 1e-9 {
            tick(d, "csl", x(mm), ym, x(mm), ym - if m % 5 == 0 { 34.0 } else { 26.0 }, 1.4);
            if m % 5 == 0 { tx(d, num_x(x(mm), xe, 17.0, m), ym - 42.0, 17.0, "middle", INK, false, &m.to_string()); }
        }
        if m < 25 && mm + 0.5 >= a && mm + 0.5 <= rd.r + 1e-9 {
            tick(d, "csh", x(mm + 0.5), ym, x(mm + 0.5), ym + 26.0, 1.4);
        }
    }
    open_rect(d, xe, ym - MPRT, MPTV + 20.0, 2.0 * MPRT, THIMBLE, 1.6);
    d.line(xe + 5.0, ym - MPRT, xe + 5.0, ym + MPRT, STEEL_DK, 1.2);
    let t = thimble_t(rd);
    for j in 0..50 {
        let phi = mark_phi(j, t);
        if phi.abs() < 1.4 {
            let y = ym - MPRT * phi.sin();
            tick(d, "ctm", xe, y, xe + if j % 5 == 0 { 34.0 } else { 20.0 }, y, 1.4);
            if j % 5 == 0 && phi.abs() < 1.05 { tx(d, xe + 42.0, y + 6.0, 16.0, "start", INK, false, &j.to_string()); }
        }
    }
    d.raw("</g>");
    d.raw(&format!(r#"<rect x="{px0:.2}" y="{py:.2}" width="{pw:.2}" height="{MPH}" rx="6" fill="none" stroke="{INK}" stroke-width="2"/>"#));
}

// ---------------------------------------------------------------------------------------------
// Render and handles
// ---------------------------------------------------------------------------------------------

pub fn render(p: &Params) -> String {
    let cal = is_caliper(p);
    let lay = layout(p);
    let mut d = Doc::new(lay.w, lay.h);
    let v = vern(p);
    for (bi, b) in lay.blocks.iter().enumerate() {
        if let Some(c) = b.caption {
            let y = b.panel_y.or(b.instr_y).unwrap_or(0.0) - CAPTION_H + 16.0;
            tx(&mut d, 12.0, y, 13.0, "start", INK, true, c);
        }
        let both = b.panel_y.is_some() && b.instr_y.is_some();
        if let Some(py) = b.panel_y {
            if cal { caliper_panel(&mut d, p, &b.rd, py, bi) } else { micrometer_panel(&mut d, p, &b.rd, py, bi) }
        }
        if let Some(oy) = b.instr_y {
            if cal { caliper(&mut d, p, &b.rd, oy, b.closed, both) } else { micrometer(&mut d, p, &b.rd, oy, b.closed, both) }
            if let (true, Some(py)) = (both, b.panel_y) {
                if cal {
                    let (a, bb) = cal_window(&b.rd, &v);
                    let (px0, pw, _) = cal_panel_geo(p, &b.rd, &v);
                    joins(&mut d, (x_cal(a), x_cal(bb), oy + 31.0), (px0, px0 + pw, py + CPH));
                } else {
                    let ya = oy + mic_top(p, &b.rd, b.closed);
                    let (xz, xe, _) = mic_x(&b.rd);
                    let (px0, pw) = mic_panel_geo(p, &b.rd);
                    joins(&mut d, (xz + mic_window(&b.rd) * MK, xe + 34.0, ya - 27.0), (px0, px0 + pw, py + MPH));
                }
            }
        }
    }
    wrap(p, d.finish(), &key_lines(p))
}

/// The measuring instrument's handle: the caliper's moving jaw, or the micrometer's thimble.
pub fn handles(p: &Params) -> Vec<Handle> {
    let lay = layout(p);
    let Some(b) = lay.blocks.last() else { return vec![] };
    let Some(oy) = b.instr_y else { return vec![] };
    if is_caliper(p) {
        let xj = x_cal(b.rd.gap());
        vec![Handle::square((xj + 22.0, oy + 40.0 + 92.0), "Drag the slider to open or close the jaws", |q, t| {
            let rd = cal_read(q);
            let lc = vern(q).lc;
            let nr = snap(((t.0 - 22.0 - CX0) / CK + rd.z).clamp(0.0, 150.0), lc).max(rd.z);
            if (nr - rd.r).abs() > 1e-9 { q.set_f("cal_reading", nr); }
        }).with_cursor(Cursor::ResizeH)]
    } else {
        let ya = oy + mic_top(p, &b.rd, false);
        let (_, xe, _) = mic_x(&b.rd);
        vec![Handle::square((xe + 84.0, ya), "Drag the thimble to open or close the micrometer", |q, t| {
            let rd = mic_read(q);
            let nr = snap(((t.0 - 84.0 - MXZ0) / MK + rd.z).clamp(0.0, 25.0), rd.q).max(rd.z);
            if (nr - rd.r).abs() > 1e-9 { q.set_f("mic_reading", nr); }
        }).with_cursor(Cursor::ResizeH)]
    }
}

#[cfg(test)]
mod tests {
    use crate::figures::{registry, FigureDef, Params, Value};

    const ID: &str = "length-reading";
    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("length-reading is registered") }
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.into())) }
    fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
    /// The opening state with `list` set on top.
    fn with(list: &[(&'static str, Value)]) -> Params {
        let d = def();
        let mut p = crate::figures::start_params(&d);
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }

    fn fnv(s: &str) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
        format!("{h:016x}")
    }

    /// The rulers saved and shared before batch 5 (every old choice and switch, alone and in a few
    /// combinations): their saved spec and share link with the hash of what they drew, written once
    /// from the build before the caliper and micrometer with VERSHION_LEN_OLD_BLESS=1.
    fn old_cases() -> Vec<Vec<(&'static str, Value)>> {
        let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
        for e in super::super::measure::examples(ID).unwrap().into_iter().take(5) { cases.push(e.params); }
        let opts: Vec<(&'static str, Value)> = vec![
            s("units", "Imperial (in)"), s("ruler", "20 cm"), s("ruler", "30 cm"), s("ruler", "50 cm"), s("ruler", "100 cm"),
            s("marks", "10 cm"), s("marks", "5 cm"), s("marks", "1 cm"), s("marks", "0.5 cm"), f("length", 0.05), f("length", 12.34), f("length", 100.0),
            f("offset", 3.21), f("offset", 100.0), s("object", "Marbles"), s("object", "Rock"), s("object", "Cube"), b("dashes", true),
            b("answer", true), s("title", "Measure the cylinder"), s("read_to", "Nearest mark"), s("mag_show", "Ruler and magnifiers"),
            s("mag_show", "Ruler only"), i("mag_marks", 1), i("mag_marks", 6), s("title_style", "None"), s("title_style", "Blank line"),
        ];
        for o in &opts {
            cases.push(vec![o.clone()]);
            cases.push(vec![s("units", "Imperial (in)"), b("answer", true), o.clone()]);
            cases.push(vec![s("mag_show", "Ruler and magnifiers"), f("offset", 2.0), b("answer", true), s("read_to", "Nearest mark"), o.clone()]);
        }
        cases.push(vec![s("ruler", "100 cm"), s("marks", "1 cm"), f("length", 62.4), f("offset", 10.0), s("object", "Marbles"), b("dashes", true), b("answer", true), s("title", "Meter stick")]);
        cases
    }

    fn old_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/length_reading_old.txt") }

    #[test]
    fn old_saved_figures_and_links_render_as_before() {
        use crate::figures::{share, spec, start_params};
        let d = def();
        if std::env::var("VERSHION_LEN_OLD_BLESS").is_ok_and(|v| v == "1") {
            let mut out = String::new();
            for c in old_cases() {
                let p = with(&c);
                let saved = serde_json::to_string(&spec::spec_for(ID, &p)).unwrap();
                let link = share::encode(ID, &p, &start_params(&d));
                out.push_str(&format!("{}\t{saved}\t{link}\n", fnv(&d.svg(&p))));
            }
            std::fs::write(old_fixture(), out).unwrap();
            return;
        }
        let text = std::fs::read_to_string(old_fixture()).expect("tests/fixtures/length_reading_old.txt");
        let mut n = 0;
        for line in text.lines().filter(|l| !l.is_empty()) {
            let mut it = line.split('\t');
            let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
            let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
            let p = spec::params_from_spec(&d, &sp);
            assert_eq!(fnv(&d.svg(&p)), hash, "an old saved figure draws differently: {saved}");
            let (id, changed) = share::decode(link).unwrap();
            assert_eq!(id, ID);
            let p = share::link_params(&d, changed, share::version(link));
            assert_eq!(fnv(&d.svg(&p)), hash, "an old share link draws differently: {link}");
            n += 1;
        }
        assert!(n >= 80, "only {n} old cases");
    }

    // ---------------------------------------------------------------------------------------------
    // Batch 5: the caliper and the micrometer
    // ---------------------------------------------------------------------------------------------

    use super::*;

    fn render_of(list: &[(&'static str, Value)]) -> String { def().svg(&with(list)) }
    fn cal(extra: &[(&'static str, Value)]) -> Vec<(&'static str, Value)> { let mut v = vec![s("instrument", "Caliper")]; v.extend(extra.iter().cloned()); v }
    fn mic(extra: &[(&'static str, Value)]) -> Vec<(&'static str, Value)> { let mut v = vec![s("instrument", "Micrometer")]; v.extend(extra.iter().cloned()); v }

    fn attr(tag: &str, name: &str) -> Option<f64> {
        let k = format!(" {name}=\"");
        let i = tag.find(&k)? + k.len();
        tag[i..].split('"').next()?.parse().ok()
    }
    /// (x1, y1, x2, y2) of the lines of `class` in `svg`.
    fn lines(svg: &str, class: &str) -> Vec<(f64, f64, f64, f64)> {
        let k = format!("<line class=\"{class}\"");
        svg.match_indices(&k).map(|(i, _)| {
            let tag = &svg[i..i + svg[i..].find("/>").unwrap()];
            (attr(tag, "x1").unwrap(), attr(tag, "y1").unwrap(), attr(tag, "x2").unwrap(), attr(tag, "y2").unwrap())
        }).collect()
    }
    /// (x, y, font size, text, anchor) of every text in `svg`.
    fn texts(svg: &str) -> Vec<(f64, f64, f64, String, String)> {
        let mut out = vec![];
        for (i, _) in svg.match_indices("<text ") {
            let end = i + svg[i..].find('>').unwrap();
            let tag = &svg[i..end];
            let body = &svg[end + 1..end + 1 + svg[end + 1..].find("</text>").unwrap()];
            let anchor = tag.split("text-anchor=\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("start").to_string();
            out.push((attr(tag, "x").unwrap_or(0.0), attr(tag, "y").unwrap_or(0.0), attr(tag, "font-size").unwrap_or(0.0), body.replace("&amp;", "&"), anchor));
        }
        out
    }
    /// The body of close-up `n` (its clipped group).
    fn panel(svg: &str, n: usize) -> String {
        let k = format!("<g clip-path=\"url(#lrcu{n})\">");
        let i = svg.find(&k).unwrap_or_else(|| panic!("no close-up {n}"));
        let rest = &svg[i..];
        rest[..rest.find("</g>").unwrap()].to_string()
    }
    fn key_value(svg: &str, head: &str) -> f64 {
        let i = svg.find(head).unwrap_or_else(|| panic!("no {head:?}")) + head.len();
        svg[i..].split(" mm").next().unwrap().trim().parse().unwrap()
    }

    /// A student's reading of a caliper's scales as drawn: whole millimetres from the main-scale mark
    /// at or before the sliding scale's 0, then the sliding mark lying closest to a main mark, times the
    /// least count. The main marks' values come from their printed numbers and spacing. Returns
    /// (reading, the miss of the best mark in px, the miss of the next best in px).
    fn read_caliper(body: &str, main: &str, slide: &str, lc: f64) -> (f64, f64, f64) {
        let fs = if main == "ms" { 9.0 } else { 15.0 };
        let ms: Vec<f64> = { let mut v: Vec<f64> = lines(body, main).iter().map(|l| l.0).collect(); v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v };
        let vs: Vec<f64> = lines(body, slide).iter().map(|l| l.0).collect();
        let k = ms[1] - ms[0];
        // a printed main-scale number and its mark
        let (lx, lv) = texts(body).iter().filter(|t| t.2 == fs && t.4 == "middle").filter_map(|t| t.3.parse::<f64>().ok().map(|v| (t.0, v)))
            .find(|(x, v)| (*v as i64) % 10 == 0 && ms.iter().any(|m| (m - x).abs() < 0.01)).unwrap_or_else(|| panic!("a numbered main mark in {main}"));
        let lx = ms.iter().copied().min_by(|a, b| (a - lx).abs().partial_cmp(&(b - lx).abs()).unwrap()).unwrap();
        let mm = |x: f64| lv + ((x - lx) / k).round();
        let zero = vs[0];
        let m_before = ms.iter().copied().filter(|&x| x <= zero + 1e-6).fold(f64::NEG_INFINITY, f64::max);
        let whole = if m_before.is_finite() { mm(m_before) } else { -1.0 };
        let miss: Vec<f64> = vs.iter().map(|&x| ms.iter().map(|m| (m - x).abs()).fold(f64::INFINITY, f64::min)).collect();
        let n = vs.len() - 1;
        let mut order: Vec<usize> = (0..=n).collect();
        order.sort_by(|a, b| miss[*a].partial_cmp(&miss[*b]).unwrap());
        // a whole-millimetre reading lines up 0 and the last mark: the last is the same reading
        let j = if order[0] == n { 0 } else { order[0] };
        let second = order.iter().copied().filter(|&i| i != j && !(j == 0 && i == n)).map(|i| miss[i]).next().unwrap();
        (((whole + j as f64 * lc) * 1e6).round() / 1e6, miss[j], second)
    }

    #[test]
    fn the_caliper_reads_back_off_its_drawn_scales() {
        for (lc_name, lc) in [("0.1 mm", 0.1), ("0.05 mm", 0.05), ("0.02 mm", 0.02)] {
            for r in [0.0, 0.1, 0.35, 0.9, 3.4, 9.95, 10.0, 23.45, 34.7, 41.38, 62.35, 99.98, 120.06, 149.9, 150.0] {
                let p = with(&cal(&[s("least_count", lc_name), f("cal_reading", r), b("answer", true)]));
                let svg = def().svg(&p);
                let want = (r / lc).round() * lc;
                let key = key_value(&svg, "Reading: ");
                assert!((key - want).abs() < 1e-9, "{lc_name} {r}: key {key}, typed {want}");
                // the close-up, read as a student would
                let (got, best, next) = read_caliper(&panel(&svg, 0), "cms", "cvs", lc);
                assert!((got - key).abs() < 1e-9, "{lc_name} {r}: close-up reads {got}, key says {key}");
                assert!(best < 0.01, "{lc_name} {r}: the lined-up mark is {best} px off");
                let k = (super::CPW / (cal_window(&cal_read(&p), &vern(&p)).1 - cal_window(&cal_read(&p), &vern(&p)).0)).min(40.0);
                assert!((next - lc * k).abs() < 0.01, "{lc_name} {r}: the next best mark is {next} px off, one least count is {} px", lc * k);
                // and the instrument itself
                let (got, best, _) = read_caliper(&svg, "ms", "vs", lc);
                assert!((got - key).abs() < 1e-9 && best < 0.01, "{lc_name} {r}: the instrument reads {got}, key {key}");
                // the jaws are open by the reading (no zero error): face to face, in mm
                let obj = svg.find("fill=\"#d9c08a\"").map(|i| &svg[svg[..i].rfind('<').unwrap()..i]);
                if want > 0.0 { let t = obj.expect("the block"); assert!((attr(t, "width").unwrap() / CK - want).abs() < 0.01, "{t}"); } else { assert!(obj.is_none(), "closed jaws hold nothing"); }
            }
        }
        // the textbook case by hand: 23 mm and the 9th mark of a 0.05 mm scale (23 + 9 x 0.05)
        let p = with(&cal(&[f("cal_reading", 23.45)]));
        let rd = cal_read(&p);
        assert_eq!((rd.r - rd.r.floor(), ((rd.r - 23.0) / 0.05).round()), (rd.r - 23.0, 9.0));
        assert!((rd.r + 9.0 * vern(&p).div() - 41.0).abs() < 1e-9, "mark 9 sits on the 41 mm mark");
        assert_eq!((vern(&p).div(), 2.0 - vern(&p).div()), (1.95, 0.050000000000000044), "least count = 2 mm less 1.95 mm");
    }

    /// A student's reading of a micrometer close-up: whole millimetres from the last sleeve mark
    /// showing, 0.5 if a half mark below the line shows past it, then the thimble mark on the line
    /// (or between two, estimated). The thimble's numbers name its marks.
    fn read_micrometer(body: &str, est: bool) -> f64 {
        let dl = lines(body, "cdl")[0];
        let (edge, ym) = (dl.2, dl.1);
        let sl: Vec<f64> = { let mut v: Vec<f64> = lines(body, "csl").iter().map(|l| l.0).collect(); v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v };
        let sh: Vec<f64> = lines(body, "csh").iter().map(|l| l.0).collect();
        let tx_: Vec<(f64, f64, f64, String, String)> = texts(body);
        // sleeve numbers sit over their marks above the line
        let last = sl.iter().copied().filter(|&x| x < edge + 1e-6).fold(f64::NEG_INFINITY, f64::max);
        // no sleeve mark showing: the thimble stands short of the 0 mark (a negative zero error)
        let whole = if last.is_finite() {
            let (lx, lv) = tx_.iter().filter(|t| t.2 > 16.5 && t.0 <= edge + 1.0).filter_map(|t| t.3.parse::<f64>().ok().map(|v| (t.0, v))).last().expect("a sleeve number");
            lv + ((last - lx) / MPK).round()
        } else { -1.0 };
        let half = sh.iter().any(|&x| x > last && x < edge + 1e-6);
        // thimble: marks top to bottom, their numbers from the printed labels
        let mut tm: Vec<f64> = lines(body, "ctm").iter().map(|l| l.1).collect();
        tm.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let (ly, lj) = tx_.iter().filter(|t| t.0 > edge).filter_map(|t| t.3.parse::<i64>().ok().map(|v| (t.1 - 6.0, v))).next().expect("a thimble number");
        let il = tm.iter().position(|y| (y - ly).abs() < 0.01).unwrap() as i64;
        let j_of = |i: usize| (lj - (i as i64 - il)).rem_euclid(50);
        // the mark at or just below the line, and the one above it
        let ib = tm.iter().position(|&y| y >= ym - 1e-6).unwrap();
        let below = tm[ib];
        let frac = if (below - ym).abs() < 1e-6 { 0.0 } else { (below - ym) / (below - tm[ib - 1]) };
        if !est { assert!(frac.abs() < 1e-9, "read to the marks: the line lies on a mark"); }
        let mut th = j_of(ib) as f64 + (frac * 10.0).round() / 10.0;
        if j_of(ib) == 49 && frac > 0.95 { th = 50.0; }
        let r = if whole < 0.0 { -0.5 + th * 0.01 } else { whole + if half { 0.5 } else { 0.0 } + th * 0.01 };
        (r * 1e6).round() / 1e6
    }

    #[test]
    fn a_reading_the_scale_cannot_show_is_said_to_be_rounded() {
        // the batch 5 checker: 23.45 on the 0.02 mm scale drew and keyed 23.46 with no word of it
        let p = with(&[s("instrument", "Caliper"), s("least_count", "0.02 mm"), f("cal_reading", 23.45)]);
        let n = range_note(&p).unwrap();
        assert!(n.contains("23.45 mm is drawn as 23.46 mm"), "{n}");
        let p = with(&[s("instrument", "Caliper"), f("cal_reading", 23.45)]);
        assert!(!range_note(&p).unwrap().contains("drawn as"));
        let p = with(&mic(&[f("mic_reading", 7.384)]));
        assert!(range_note(&p).unwrap().contains("7.384 mm is drawn as 7.38 mm"));
    }

    #[test]
    fn a_sleeve_number_is_never_under_the_thimble() {
        // the batch 5 checker: at 10.000 mm the close-up showed "1", at 25.00 "2"
        for r in [5.0, 10.0, 15.0, 20.0, 25.0, 10.02, 4.99] {
            let svg = def().svg(&with(&mic(&[f("mic_reading", r)])));
            let body = panel(&svg, 0);
            let edge = lines(&body, "cdl")[0].2;
            for t in texts(&body).iter().filter(|t| t.2 > 16.5 && t.0 <= edge) {
                if let Ok(v) = t.3.parse::<i64>() {
                    if v % 5 != 0 { continue; }
                    let half = 0.3 * t.2 * t.3.len() as f64;
                    assert!(t.0 + half <= edge + 1e-6, "{r} mm: sleeve number {v} runs under the thimble (ends {:.1}, edge {edge:.1})", t.0 + half);
                }
            }
            // and it still reads right
            assert!((read_micrometer(&body, false) - r).abs() < 1e-9, "{r}");
        }
    }

    #[test]
    fn the_micrometer_reads_back_off_its_drawn_scales() {
        for est in [false, true] {
            let rs: &[f64] = if est { &[0.004, 2.003, 5.507, 6.847, 7.384, 9.999, 12.5, 18.261, 24.996] } else { &[0.0, 0.01, 0.49, 0.5, 4.77, 5.01, 5.51, 7.38, 7.88, 12.0, 19.99, 24.5, 25.0] };
            for &r in rs {
                let p = with(&mic(&[s("mic_read_to", MIC_READ[est as usize]), f("mic_reading", r), b("answer", true)]));
                let svg = def().svg(&p);
                let key = key_value(&svg, "Reading: ");
                assert!((key - r).abs() < 1e-9, "{r}: key {key}");
                let got = read_micrometer(&panel(&svg, 0), est);
                assert!((got - key).abs() < 1e-9, "est {est}, {r}: the close-up reads {got}, the key says {key}");
                // the key gives the decimals the instrument reads to
                let dec = svg[svg.find("Reading: ").unwrap()..].split(" mm").next().unwrap().split('.').nth(1).map(|d| d.len());
                assert_eq!(dec, Some(if est { 3 } else { 2 }));
            }
        }
        // by hand: 7.38 is 7 mm on the sleeve, no half mark, thimble 38; 7.88 shows the 7.5 mark
        assert_eq!(thimble_t(&mic_read(&with(&mic(&[f("mic_reading", 7.38)])))), 38.0);
        assert_eq!(thimble_t(&mic_read(&with(&mic(&[f("mic_reading", 7.88)])))), 38.0);
        let half = |r: f64| lines(&render_of(&mic(&[f("mic_reading", r)])), "csh").len();
        assert_eq!(half(7.88), half(7.38) + 1, "the 7.5 mark shows only past 7.5 mm");
        // 0.4 of the way from 38 to 39 reads 7.384
        assert_eq!(thimble_t(&mic_read(&with(&mic(&[s("mic_read_to", MIC_READ[1]), f("mic_reading", 7.384)])))), 38.4);
        // the higher numbers are above the line (a right-hand screw opening)
        let body = panel(&render_of(&mic(&[f("mic_reading", 7.38)])), 0);
        let y_of = |n: &str| texts(&body).into_iter().find(|t| t.3 == n && t.2 < 16.5).unwrap().1;
        assert!(y_of("40") < y_of("35"));
        // its marks bunch toward the thimble's edges: spacing near the line is the widest
        let mut ys: Vec<f64> = lines(&body, "ctm").iter().map(|l| l.1).collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let gaps: Vec<f64> = ys.windows(2).map(|w| w[1] - w[0]).collect();
        let mid = gaps.len() / 2;
        assert!(gaps[0] < gaps[mid] * 0.4 && gaps[gaps.len() - 1] < gaps[mid] * 0.4, "{gaps:?}");
    }

    #[test]
    fn a_zero_error_reads_off_the_closed_instrument_and_corrects_the_key() {
        // caliper: +0.10 and -0.10 mm (0.05 mm scale), micrometer: +0.03 and -0.03 mm
        for (inst, z, r) in [("Caliper", 0.1, 23.45), ("Caliper", -0.1, 23.45), ("Caliper", 0.4, 7.0), ("Micrometer", 0.03, 7.38), ("Micrometer", -0.03, 5.51), ("Micrometer", -0.27, 3.62)] {
            let key = if inst == "Caliper" { "cal_reading" } else { "mic_reading" };
            let p = with(&[s("instrument", inst), f(key, r), f("zero_error", z), b("show_closed", true), b("answer", true)]);
            let svg = def().svg(&p);
            let closed = if inst == "Caliper" { read_caliper(&panel(&svg, 0), "cms", "cvs", 0.05).0 } else { read_micrometer(&panel(&svg, 0), false) };
            let open = if inst == "Caliper" { read_caliper(&panel(&svg, 1), "cms", "cvs", 0.05).0 } else { read_micrometer(&panel(&svg, 1), false) };
            assert!((closed - z).abs() < 1e-9, "{inst} {z}: the closed instrument reads {closed}");
            assert!((open - r).abs() < 1e-9, "{inst}: the measuring one reads {open}");
            let line = svg[svg.find("Zero error: ").unwrap()..].split('<').next().unwrap().to_string();
            let sign = if z > 0.0 { format!("+{:.2}", z) } else { format!("\u{2212}{:.2}", -z) };
            assert!(line.starts_with(&format!("Zero error: {sign} mm")), "{line}");
            let length: f64 = line.rsplit("= ").next().unwrap().trim_end_matches(" mm").parse().unwrap();
            assert!((length - (r - z)).abs() < 1e-9, "{line}: the length is the reading less the zero error");
            // the object is drawn to the true length, not the reading
            let i = svg.find("fill=\"#d9c08a\"").unwrap();
            let tag = &svg[svg[..i].rfind('<').unwrap()..i];
            let k = if inst == "Caliper" { CK } else { MK };
            assert!((attr(tag, "width").unwrap() / k - (r - z)).abs() < 0.01, "{inst} {z}: {tag}");
        }
        // the closed copy's captions, and nothing in its jaws
        let svg = render_of(&cal(&[f("zero_error", 0.1), b("show_closed", true)]));
        assert!(svg.contains(">Closed<") && svg.contains(">Measuring<"));
        assert_eq!(svg.matches("fill=\"#d9c08a\"").count(), 1);
        // no zero error, no second line; the key without the closed copy is the same
        assert!(!render_of(&cal(&[b("answer", true)])).contains("Zero error"));
        // a typed zero error goes to the least count (the 0.05 mm scale cannot show 0.08)
        assert_eq!(cal_read(&with(&cal(&[f("zero_error", -0.08)]))).z, -0.1);
    }

    #[test]
    fn a_reading_below_the_zero_error_closes_the_jaws() {
        let p = with(&cal(&[f("cal_reading", 0.0), f("zero_error", 0.3), b("answer", true)]));
        let rd = cal_read(&p);
        assert_eq!((rd.r, rd.gap()), (0.3, 0.0));
        let svg = def().svg(&p);
        assert!(!svg.contains("#d9c08a"), "nothing fits in closed jaws");
        assert!(svg.contains("so the length is 0.30 \u{2212} 0.30 = 0.00 mm"), "{}", key_lines(&p).join(" | "));
    }

    #[test]
    fn no_answer_on_the_student_copy() {
        for list in [cal(&[]), cal(&[f("zero_error", 0.1), b("show_closed", true)]), mic(&[]), mic(&[s("mic_read_to", MIC_READ[1]), f("mic_reading", 6.847), f("zero_error", -0.03), b("show_closed", true)])] {
            let mut l = list.clone();
            l.push(b("answer", false));
            let svg = render_of(&l);
            for bad in ["Reading:", "Zero error", "length is", "23.45", "6.847", "+0.10", "0.03"] {
                assert!(!svg.contains(bad), "{bad:?} on the student copy of {list:?}");
            }
            let p = with(&l);
            assert!(key_lines(&p).is_empty());
            // the alt text is the figure's blurb only
            let desc = svg.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
            assert_eq!(desc, def().blurb);
            l.pop();
            l.push(b("answer", true));
            assert!(render_of(&l).contains("Reading: "));
        }
    }

    /// Text boxes as drawn (x0, y0, x1, y1): estimated Arial widths, cap height above the baseline.
    fn boxes(svg: &str) -> Vec<(f64, f64, f64, f64, String)> {
        // the wrapped figure's content sits in a translated group: add its offset
        let pat = "<g transform=\"translate(0 ";
        let dy = svg.find(pat).map(|i| svg[i + pat.len()..].split(')').next().unwrap().parse::<f64>().unwrap()).unwrap_or(0.0);
        let body_start = svg.find("<g transform=\"translate(0 ").unwrap_or(usize::MAX);
        let body_end = svg.rfind("</g>").unwrap_or(0);
        let mut out = vec![];
        for (i, _) in svg.match_indices("<text ") {
            let end = i + svg[i..].find('>').unwrap();
            let tag = &svg[i..end];
            let body = &svg[end + 1..end + 1 + svg[end + 1..].find("</text>").unwrap()];
            let (x, y, fs) = (attr(tag, "x").unwrap(), attr(tag, "y").unwrap() + if i > body_start && i < body_end { dy } else { 0.0 }, attr(tag, "font-size").unwrap());
            let w = text_w(body, fs);
            let x0 = match tag.split("text-anchor=\"").nth(1).and_then(|r| r.split('"').next()) { Some("middle") => x - w / 2.0, Some("end") => x - w, _ => x };
            out.push((x0, y - fs * 0.72, x0 + w, y + fs * 0.05, body.to_string()));
        }
        out
    }

    fn audit(list: &[(&'static str, Value)]) {
        let svg = render_of(list);
        let (w, h) = crate::figures::svg_size(&svg);
        let bx = boxes(&svg);
        for t in &bx {
            assert!(t.0 >= -0.5 && t.2 <= w + 0.5 && t.1 >= -0.5 && t.3 <= h + 0.5, "{list:?}: {:?} leaves the {w}x{h} canvas", t.4);
        }
        for (i, a) in bx.iter().enumerate() {
            for b_ in &bx[i + 1..] {
                let ov = a.0 < b_.2 - 0.5 && b_.0 < a.2 - 0.5 && a.1 < b_.3 - 0.5 && b_.1 < a.3 - 0.5;
                assert!(!ov, "{list:?}: {:?} overlaps {:?}", a.4, b_.4);
            }
        }
        for (_, _, fs, t, _) in texts(&svg) { assert!(fs >= 8.0, "{t:?} at {fs} px"); }
    }

    #[test]
    fn text_stays_readable_inside_the_page_and_apart() {
        let long = "A very long label for the steel ball bearing held between the jaws of the instrument";
        for held in HELD {
            for lab in ["", "Coin", long] {
                for r in [0.05, 1.0, 23.45, 75.0, 120.0, 150.0] {
                    for lc in LEAST {
                        audit(&cal(&[s("held", held), s("held_label", lab), f("cal_reading", r), s("least_count", lc), b("answer", true), f("zero_error", 0.2), b("show_closed", r > 100.0)]));
                    }
                }
                for r in [0.0, 0.5, 7.384, 12.0, 24.5, 25.0] {
                    for est in MIC_READ {
                        audit(&mic(&[s("held", held), s("held_label", lab), f("mic_reading", r), s("mic_read_to", est), b("answer", true), f("zero_error", -0.03), b("show_closed", r > 10.0)]));
                    }
                }
            }
        }
        for v in VIEW {
            audit(&cal(&[s("close_up", v), b("show_closed", true), f("zero_error", -0.5), s("title", "Measure the block to the least count of the caliper"), b("answer", true)]));
            audit(&mic(&[s("close_up", v), b("show_closed", true), f("zero_error", 0.5), s("title", "Measure the wire"), b("answer", true)]));
        }
    }

    /// The ends of every range, and the biggest case the controls allow together.
    #[test]
    fn the_ends_of_every_range_draw_whole() {
        for (key, lo, hi) in [("cal_reading", 0.0, 150.0), ("zero_error", -1.0, 1.0)] {
            for v in [lo, hi] {
                for lc in LEAST {
                    let list = cal(&[f(key, v), s("least_count", lc), s("held", "Ball"), b("show_closed", true), b("answer", true), s("held_label", "Ball")]);
                    let svg = render_of(&list);
                    let (w, _) = crate::figures::svg_size(&svg);
                    // the slider and its scale stay on the beam and inside the canvas
                    for l in lines(&svg, "vs") { assert!(l.0 <= x_cal(200.0) + 1e-6 && l.0 <= w, "{list:?}: a sliding mark at {}", l.0); }
                    audit(&list);
                    let rd = cal_read(&with(&list));
                    assert!(rd.r >= rd.z && rd.r <= 151.0, "{rd:?}");
                }
            }
        }
        for (key, lo, hi) in [("mic_reading", 0.0, 25.0), ("zero_error", -1.0, 1.0)] {
            for v in [lo, hi] {
                let list = mic(&[f(key, v), f("mic_reading", if key == "mic_reading" { v } else { 25.0 }), s("held", "Ball"), b("show_closed", true), b("answer", true), s("held_label", "Ball")]);
                let p = with(&list);
                let rd = mic_read(&p);
                assert!(rd.z.abs() <= 0.5 + 1e-9, "the micrometer's zero error is held to half a millimetre");
                let (_, _, xs) = mic_x(&rd);
                assert!(xs <= MXH, "the spindle stays in the frame: {rd:?}");
                audit(&list);
            }
        }
    }

    /// Each of the caliper's and micrometer's controls changes the drawing, the ruler's are hidden for
    /// them (and theirs for the ruler), and the hooks leave other figures alone.
    #[test]
    fn only_the_instruments_controls_are_shown_and_each_changes_the_drawing() {
        let d = def();
        let defs = (d.params)();
        for inst in INSTRUMENTS {
            let base = with(&[s("instrument", inst), s("held_label", "Coin"), f("mic_reading", 6.847), s("title", "Measure it")]);
            let svg0 = d.svg(&base);
            for pd in &defs {
                if !crate::figures::param_relevant(ID, pd.key, &base) || pd.key == "instrument" { continue; }
                let mut changed = false;
                let tries: Vec<Value> = match &pd.kind {
                    crate::figures::Kind::Choice(o) => o.iter().map(|v| Value::S(v.to_string())).collect(),
                    crate::figures::Kind::Bool => vec![Value::B(!base.b(pd.key))],
                    crate::figures::Kind::Float { min, max, .. } => vec![Value::F(*min), Value::F(*max), Value::F((min + max) / 3.0)],
                    crate::figures::Kind::Int { min, max } => vec![Value::I(*min), Value::I(*max)],
                    _ => vec![Value::S("Another title".into())],
                };
                for v in tries {
                    let mut p = base.clone();
                    p.0.insert(pd.key.to_string(), v);
                    if pd.key == "read_to" { p.set_b("answer", true); }
                    if d.svg(&p) != svg0 { changed = true; }
                }
                assert!(changed, "{inst}: {} changes nothing", pd.key);
            }
            let ruler_only = ["units", "ruler", "marks", "read_to", "length", "offset", "object", "dashes", "mag_show", "mag_marks"];
            for k in ruler_only { assert_eq!(crate::figures::param_relevant(ID, k, &base), inst == "Ruler", "{inst} {k}"); }
            for k in ["least_count", "lc_print", "cal_reading"] { assert_eq!(crate::figures::param_relevant(ID, k, &base), inst == "Caliper", "{inst} {k}"); }
            for k in ["mic_read_to", "mic_reading"] { assert_eq!(crate::figures::param_relevant(ID, k, &base), inst == "Micrometer", "{inst} {k}"); }
            for k in ["zero_error", "show_closed", "held", "close_up", "held_label"] { assert_eq!(crate::figures::param_relevant(ID, k, &base), inst != "Ruler", "{inst} {k}"); }
        }
        // nothing to label with empty jaws
        assert!(!crate::figures::param_relevant(ID, "held_label", &with(&cal(&[s("held", "Nothing")]))));
        // other figures' controls are untouched by the hook
        let vol = registry().into_iter().find(|d| d.id == "volume-reading").unwrap();
        let pv = crate::figures::start_params(&vol);
        for pd in (vol.params)() { if pd.key == "instrument" { assert!(crate::figures::param_relevant("volume-reading", pd.key, &pv)); } }
    }

    #[test]
    fn the_jaws_hold_each_object_drawn_to_size() {
        for (kind, fill) in [("Block", "#d9c08a"), ("Cylinder", "#b4b4b4"), ("Ball", "#8fb4d6"), ("Wire", "#c8834a")] {
            for (inst, k, r) in [("Caliper", CK, 31.4), ("Micrometer", MK, 4.86)] {
                let rk = if inst == "Caliper" { "cal_reading" } else { "mic_reading" };
                let svg = render_of(&[s("instrument", inst), f(rk, r), s("held", kind), s("held_label", "Part A")]);
                let i = svg.find(&format!("fill=\"{fill}\"")).unwrap_or_else(|| panic!("{inst} {kind}"));
                let tag = &svg[svg[..i].rfind('<').unwrap()..i];
                let across = attr(tag, "width").or_else(|| attr(tag, "r").map(|r| 2.0 * r)).or_else(|| {
                    // the cylinder's outline path: its two sides
                    let d_ = tag.split("d=\"M").nth(1)?;
                    let x0: f64 = d_.split(' ').next()?.parse().ok()?;
                    let x1: f64 = d_.split(" L").nth(2)?.split(' ').next()?.parse().ok()?;
                    Some(x1 - x0)
                }).unwrap();
                assert!((across / k - r).abs() < 0.02, "{inst} {kind}: {across} px is {} mm, not {r}", across / k);
                assert!(svg.contains(">Part A<"), "{inst} {kind}: the label");
            }
        }
        assert!(!render_of(&cal(&[s("held", "Nothing"), s("held_label", "Part A")])).contains("Part A"), "no label with nothing held");
    }

    #[test]
    fn the_least_count_prints_on_the_slider_or_not() {
        for (lc, txt) in [("0.1 mm", "0.1 mm"), ("0.05 mm", "0.05 mm"), ("0.02 mm", "0.02 mm")] {
            let on = render_of(&cal(&[s("least_count", lc)]));
            assert_eq!(on.matches(&format!(">{txt}<")).count(), 2, "{lc}: on the slider and in its close-up");
            let off = render_of(&cal(&[s("least_count", lc), b("lc_print", false)]));
            assert!(!off.contains(&format!(">{txt}<")), "{lc}");
            // 10, 20 or 50 sliding divisions, numbered 0 to 10 in tenths of a millimetre
            let n = lines(&on, "vs").len() - 1;
            assert_eq!(n, match lc { "0.1 mm" => 10, "0.02 mm" => 50, _ => 20 });
        }
    }

    #[test]
    fn the_close_up_alone_or_the_instrument_alone() {
        for list in [cal(&[]), mic(&[])] {
            let mut a = list.clone(); a.push(s("close_up", VIEW[1]));
            let mut b_ = list.clone(); b_.push(s("close_up", VIEW[2]));
            let (both, only, inst) = (render_of(&list), render_of(&a), render_of(&b_));
            assert!(both.contains("lrcu0") && only.contains("lrcu0") && !inst.contains("lrcu0"));
            let cls = if list[0].1 == Value::S("Caliper".into()) { "\"vs\"" } else { "\"tm\"" };
            assert!(both.contains(cls) && !only.contains(cls) && inst.contains(cls));
            // the close-up alone is centred in its own page
            let (w, _) = crate::figures::svg_size(&only);
            let i = only.find("<rect x=\"").unwrap();
            let x0 = attr(&only[i..], "x").unwrap();
            let pw = attr(&only[i..], "width").unwrap();
            assert!((x0 - (w - pw) / 2.0).abs() < 0.5, "{x0} {pw} {w}");
        }
    }

    #[test]
    fn random_readings_land_on_the_instruments_resolution() {
        for seed in 0..200u64 {
            for (list, key, q, hi) in [(cal(&[s("least_count", "0.02 mm")]), "cal_reading", 0.02, 150.0), (cal(&[s("least_count", "0.1 mm")]), "cal_reading", 0.1, 150.0), (mic(&[]), "mic_reading", 0.01, 25.0), (mic(&[s("mic_read_to", MIC_READ[1])]), "mic_reading", 0.001, 25.0)] {
                let mut p = with(&list);
                assert!(crate::figures::random_reading(ID, &mut p, seed));
                let v = p.f(key);
                assert!(v > 0.0 && v < hi, "{v}");
                assert!(((v / q) - (v / q).round()).abs() < 1e-6, "{key} {v} is not a multiple of {q}");
            }
        }
        // the ruler's own reading is left alone
        let mut p = with(&[]);
        assert!(!crate::figures::random_reading(ID, &mut p, 3));
        assert!(crate::figures::is_random_key(ID, "cal_reading") && crate::figures::is_random_key(ID, "mic_reading") && !crate::figures::is_random_key(ID, "length"));
        assert!(crate::figures::is_random_key("volume-reading", "reading"));
    }

    /// The slider and the thimble are dragged through the workspace's own hit test.
    #[test]
    fn the_slider_and_thimble_drag_through_the_hit_test() {
        use crate::figures::interact::{handles_for, pick};
        let cases: Vec<(Vec<(&'static str, Value)>, &str, &str, f64)> = vec![
            (cal(&[]), "Drag the slider", "cal_reading", CK),
            (cal(&[b("show_closed", true), f("zero_error", 0.1), s("title", "Calipers")]), "Drag the slider", "cal_reading", CK),
            (cal(&[s("close_up", VIEW[2]), s("least_count", "0.1 mm"), s("held", "Ball"), f("cal_reading", 60.0)]), "Drag the slider", "cal_reading", CK),
            (mic(&[]), "Drag the thimble", "mic_reading", MK),
            (mic(&[b("show_closed", true), f("zero_error", -0.03), s("title", "Micrometer"), s("held", "Ball"), s("held_label", "Bearing"), f("mic_reading", 20.0)]), "Drag the thimble", "mic_reading", MK),
            (mic(&[s("close_up", VIEW[2]), s("mic_read_to", MIC_READ[1])]), "Drag the thimble", "mic_reading", MK),
        ];
        for (list, tip, key, k) in cases {
            let p = with(&list);
            let svg = def().svg(&p);
            let size = crate::figures::svg_size(&svg);
            let hs = handles_for(ID, &p, size);
            assert_eq!(hs.len(), 1, "{list:?}");
            let h = &hs[0];
            assert!(h.tip.starts_with(tip), "{}", h.tip);
            // the handle sits on the drawn jaw or thimble: sweep the pixels around it
            let mut hits = 0;
            for dx in -6..=6 { for dy in -6..=6 {
                if pick(&hs, (h.pos.0 + dx as f64, h.pos.1 + dy as f64), 1.0) == Some(0) { hits += 1; }
            } }
            assert!(hits > 20, "{list:?}: {hits} hits");
            assert_eq!(pick(&hs, (h.pos.0 + 60.0, h.pos.1 + 60.0), 1.0), None);
            // dragging it to where it is changes nothing; 5 mm to the right reads 5 mm more
            let drag = h.drag.clone().unwrap();
            let mut q = p.clone();
            drag(&mut q, h.pos);
            assert_eq!(q.f(key), p.f(key));
            let mut q = p.clone();
            drag(&mut q, (h.pos.0 + 5.0 * k, h.pos.1));
            let before = read_of(&p).r;
            assert!((read_of(&q).r - (before + 5.0)).abs() < 1e-6, "{list:?}: {} -> {}", before, read_of(&q).r);
            // and the handle follows the jaw or thimble it moved
            let svg2 = def().svg(&q);
            let h2 = &handles_for(ID, &q, crate::figures::svg_size(&svg2))[0];
            assert!((h2.pos.0 - h.pos.0 - 5.0 * k).abs() < 1e-6);
            // dragging past the ends stops at them
            let mut q = p.clone();
            drag(&mut q, (-500.0, h.pos.1));
            assert!((read_of(&q).r - read_of(&q).z.max(0.0)).abs() < 1e-9, "{:?}", read_of(&q));
        }
        // the close-up alone has nothing to drag
        assert!(handles_for(ID, &with(&cal(&[s("close_up", VIEW[1])])), (800.0, 400.0)).is_empty());
    }

    #[test]
    fn the_panel_notes_follow_the_instrument() {
        let p = with(&cal(&[]));
        assert_eq!(range_note(&p).unwrap(), "From 0 to 150 mm, to 0.05 mm, and the zero error to 0.05 mm too. You can also drag the slider in the figure.");
        assert!(range_note(&with(&mic(&[s("mic_read_to", MIC_READ[1])]))).unwrap().contains("to 0.001 mm"));
        assert!(range_note(&with(&[])).is_none());
        assert_eq!(summary("Reading", &with(&cal(&[f("zero_error", 0.1), b("show_closed", true)]))).unwrap(), "23.45 mm, zero error +0.10 mm, closed too");
        assert_eq!(summary("Instrument", &with(&mic(&[]))).unwrap(), "Micrometer");
        let mut q = with(&cal(&[f("zero_error", 0.1)]));
        q.set_b("answer", false);
        assert_eq!(crate::figures::sections::note_after(ID, "answer", &q).unwrap(), "Print \u{201c}Reading: 23.45 mm\u{201d} under the figure.");
        // the ruler's note is as it was
        assert_eq!(crate::figures::sections::note_after(ID, "answer", &with(&[])).unwrap(), "Print \u{201c}Length: 4.37 cm\u{201d} under the figure.");
    }

    #[test]
    fn the_new_examples_draw_what_they_say() {
        let ex = super::super::measure::examples(ID).unwrap();
        assert!(ex.len() >= 14);
        for e in &ex[5..] {
            let p = with(&e.params);
            let rd = read_of(&p);
            let shown = num(rd.r, rd.dp);
            let zero = num(rd.z.abs(), rd.dp);
            assert!(e.name.contains(&shown) || e.name.contains(&zero), "{}: reads {shown}", e.name);
            let svg = def().svg(&p);
            assert!(svg.len() > 2000, "{}", e.name);
        }
    }
}
