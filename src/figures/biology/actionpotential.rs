//! The action potential of a neuron: membrane potential (mV) against time (ms), the standard exam diagram.
//!
//! The trace is built from a few analytic pieces, never a hand-placed polyline, and every piece is monotone, so the
//! curve cannot wiggle: it rests, rises along a smooth S through the threshold to the peak (zero slope at the top),
//! falls along a mirror S to the lowest point of the undershoot (zero slope again), then creeps back to rest along a
//! critically damped exponential. The resting level, the peak and the lowest point are hit exactly, and the
//! threshold is crossed once on the way up. A "failed initiation" is a stimulus that only makes a small bump
//! (a fixed fraction of the way to threshold) which returns to rest: the all-or-none lesson.
//!
//! What is the exercise: the NAMES. Under Show answers they are listed in a key under the figure; in a student copy
//! the same key has a blank line to write each one on, so both copies lay out identically. Numbered discs point at
//! the stimulus, the resting level, the threshold line and the phases of the curve; lettered discs sit under the
//! brackets that mark when the channels are open and the refractory periods. The typed numbers (resting, threshold,
//! peak, undershoot) are never printed as text in either copy: they are read off the axes, which is the point.
//! The curve is the same in both copies.
//!
//! Layout is by search: each numbered disc is tried at several places along the curve and in several directions and
//! takes the first that is clear of the curve, the guide lines, the other discs and the plot edge. If none is clear
//! the figure is refused with that reason rather than drawn crowded.

use super::cladogram::text_w;
use super::foodweb::{text_el, wrap};
use super::*;
use crate::figures::axes::nice_step;
use crate::figures::chart::fmt;

pub const ID: &str = "action-potential";

// ---- the curve (ms and mV) ----
/// Rest to peak: the time of the whole upstroke, threshold included.
const T_RISE: f64 = 0.8;
/// Peak to the lowest point of the undershoot.
const T_FALL: f64 = 1.4;
/// Rate of the return to rest after the lowest point (per ms): (1 + k t) e^(-k t).
const K_BACK: f64 = 2.6;
/// How long after the lowest point the trace needs to have settled to within a fraction of a millivolt.
const T_SETTLE: f64 = 2.5;
/// Everything after the stimulus: the shortest trace holds this much after it.
pub const NEED: f64 = T_RISE + T_FALL + T_SETTLE;
/// The rise is `smoothstep(u^SKEW)`: slow at first, then fast through the threshold. The exponent also sets how sharp
/// the tip is (the curvature at the peak grows with its square), so it stays modest.
const SKEW: f64 = 1.5;
/// Channels stay open this long after the lowest point of the undershoot.
const T_LINGER: f64 = 1.0;
/// Failed initiation: how long the bump takes to peak, and how far up to threshold it climbs.
const T_BUMP: f64 = 0.7;
const BUMP_FRAC: f64 = 0.6;

// ---- what is accepted ----
pub const MV_MIN: f64 = -100.0;
pub const MV_MAX: f64 = 70.0;
/// The resting potential is negative; this keeps the stimulus arrow, which sits under the resting level, off the 0 mV line.
pub const REST_MAX: f64 = -10.0;
pub const MAX_DUR: f64 = 12.0;
pub const MIN_LEAD: f64 = 0.5;
const MIN_REST_TO_THR: f64 = 10.0;
const MIN_THR_TO_PEAK: f64 = 20.0;
const MIN_DIP: f64 = 3.0;
const MIN_THR_TO_ZERO: f64 = 4.0;

// ---- the drawing (px at Medium; text-related sizes scale with the label size) ----
const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const INK: &str = "#111111";
const ERR: &str = "#c0392b";
const PLOT_H: f64 = 320.0;
const PLOT_W_MIN: f64 = 520.0;
const MARGIN: f64 = 16.0;
const MIN_W: f64 = 420.0;
const SEG_PX: f64 = 0.9;
const COARSE_PX: f64 = 5.0;
const THEN_LINE: &str = "Nothing is drawn until every value is a number in its range.";

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Action potential",
        blurb: "A neuron's membrane potential against time: rest, stimulus, threshold, depolarisation, peak, repolarisation, undershoot and recovery, with ion-channel and refractory bands and a failed-initiation mode. The phase names are the exercise.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("resting", "Resting potential (mV)", "-70").hint("Type the number, with or without mV: -70, \u{2212}70 and -70 mV all work"),
        ParamDef::text("threshold", "Threshold (mV)", "-55").hint("At least 10 mV above the resting potential"),
        ParamDef::text("peak", "Peak (mV)", "40").hint("At least 20 mV above the threshold"),
        ParamDef::text("undershoot", "Lowest point of the undershoot (mV)", "-80").hint("Type the resting potential again for no undershoot; otherwise at least 3 mV below it"),
        ParamDef::text("stimulus", "Stimulus at (ms)", "1").hint("0.5 ms or later; a decimal comma is fine"),
        ParamDef::text("duration", "Total time (ms)", "8").hint("Room for the whole spike after the stimulus (about 4.7 ms), at most 12 ms"),
        ParamDef::bool("fail", "Failed initiation (stimulus too weak)", false).hint("A small bump that returns to rest: no spike, no channel or refractory bands"),
        ParamDef::bool("show_threshold", "Threshold line", true),
        ParamDef::bool("show_stimulus", "Stimulus marker", true),
        ParamDef::bool("show_phases", "Phase pointers", true).hint("Numbered discs on the figure and their key; off leaves the curve bare"),
        ParamDef::bool("show_channels", "Ion-channel bands", true),
        ParamDef::bool("show_refractory", "Refractory-period bands", false),
        answers_param().hint("Off keeps the pointers and gives each a blank line in the key for the student to name"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p) }

// ================================================================== reading the controls

/// Why the figure was refused, and the second line to show under the reason, chosen where it is raised.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub why: String,
    pub then: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{} {}", self.why, self.then) }
}

fn refuse(why: String, then: &str) -> Refusal { Refusal { why, then: then.to_string() } }

fn clip(s: &str) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= 20 { t } else { format!("{}…", t.chars().take(20).collect::<String>()) }
}

/// A number as shown in a message: up to three decimals, no trailing zeros, a true minus.
fn show(v: f64) -> String { fmt((v * 1000.0).round() / 1000.0) }

/// Reads one typed number: optional sign, digits with one decimal point or comma, and optionally the unit that
/// belongs to the control (`mV` or `ms`, any case). A comma followed by exactly three digits is ambiguous
/// (1,000: one thousand or one?) and is refused rather than guessed.
fn number(raw: &str, what: &str, units: &[&str], example: &str) -> Result<f64, Refusal> {
    let shown = clip(raw);
    let bad = |why: String| refuse(why, THEN_LINE);
    let not_a_number = || bad(format!("{what}: \"{shown}\" is not a number. Type it as {example}, with or without its unit."));
    let t = raw.trim_start_matches('\u{feff}').trim();
    if t.is_empty() { return Err(bad(format!("{what} is empty. Type a number such as {example}."))); }
    let lower = t.to_lowercase();
    let mut body = t;
    for u in units {
        if lower.ends_with(u) {
            // `get`, not a slice: a lower-cased string can be a different length from the typed one
            body = t.get(..t.len().saturating_sub(u.len())).map_or(t, str::trim_end);
            break;
        }
    }
    let mut chars = body.chars().peekable();
    let mut neg = false;
    if let Some(&c) = chars.peek() {
        if matches!(c, '-' | '\u{2212}' | '\u{2013}' | '\u{2014}' | '\u{2010}' | '\u{2011}') { neg = true; chars.next(); }
        else if c == '+' { chars.next(); }
    }
    let rest: String = chars.collect::<String>().trim_start().to_string();
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') { return Err(not_a_number()); }
    if !rest.chars().any(|c| c.is_ascii_digit()) { return Err(not_a_number()); }
    let digits = if rest.contains(',') {
        if rest.contains('.') || rest.matches(',').count() > 1 { return Err(not_a_number()); }
        let (a, b) = rest.split_once(',').unwrap();
        if b.len() == 3 && !a.is_empty() {
            return Err(bad(format!("{what}: \"{shown}\" is ambiguous: a comma is read as a decimal point, so it could mean {a}.{b} or {a}{b}. Type {a}.{b} or {a}{b} to be clear.")));
        }
        format!("{a}.{b}")
    } else {
        rest
    };
    if digits.matches('.').count() > 1 { return Err(not_a_number()); }
    let v: f64 = digits.parse().map_err(|_| not_a_number())?;
    if !v.is_finite() { return Err(bad(format!("{what}: \"{shown}\" is too large to read."))); }
    Ok(if neg { -v } else { v } + 0.0)
}

/// The typed controls, read and checked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spike {
    pub rest: f64,
    pub thr: f64,
    pub peak: f64,
    pub under: f64,
    pub ts: f64,
    pub dur: f64,
    pub fail: bool,
}

pub fn read(p: &Params) -> Result<Spike, Refusal> {
    let mv = |key: &str, what: &str| number(p.s(key), what, &["millivolts", "millivolt", "mv"], "-70");
    let ms = |key: &str, what: &str| number(p.s(key), what, &["milliseconds", "millisecond", "msec", "ms"], "1.5");
    let rest = mv("resting", "Resting potential")?;
    let thr = mv("threshold", "Threshold")?;
    let peak = mv("peak", "Peak")?;
    let under = mv("undershoot", "Undershoot")?;
    let ts = ms("stimulus", "Stimulus time")?;
    let dur = ms("duration", "Total time")?;
    if rest > REST_MAX {
        return Err(refuse(
            format!("The resting potential is {} mV, but a neuron's inside is negative at rest: this figure draws it from {} to {} mV.", show(rest), show(MV_MIN), show(REST_MAX)),
            "Type a negative resting potential.",
        ));
    }
    for (v, what) in [(rest, "The resting potential"), (thr, "The threshold"), (peak, "The peak"), (under, "The undershoot")] {
        if !(MV_MIN..=MV_MAX).contains(&v) {
            return Err(refuse(
                format!("{what} is {} mV, outside the {} to +{} mV this figure draws.", show(v), show(MV_MIN), show(MV_MAX)),
                "Bring it inside that range; every neuron potential in a textbook is.",
            ));
        }
    }
    if thr < rest + MIN_REST_TO_THR {
        return Err(refuse(
            format!("The threshold ({} mV) must be at least {} mV above the resting potential ({} mV): a neuron fires only when it is depolarised up to threshold, and closer than that the dashed line would run into the resting level.", show(thr), show(MIN_REST_TO_THR), show(rest)),
            "Raise the threshold or lower the resting potential.",
        ));
    }
    if thr.abs() < MIN_THR_TO_ZERO {
        return Err(refuse(
            format!("The threshold ({} mV) is within {} mV of 0 mV, so the dashed threshold line would lie on the 0 mV line.", show(thr), show(MIN_THR_TO_ZERO)),
            "Move the threshold to at least 4 mV from zero.",
        ));
    }
    if peak < thr + MIN_THR_TO_PEAK {
        return Err(refuse(
            format!("The peak ({} mV) must be at least {} mV above the threshold ({} mV): a rise shorter than that is too small to label.", show(peak), show(MIN_THR_TO_PEAK), show(thr)),
            "Raise the peak or lower the threshold.",
        ));
    }
    if under > rest {
        return Err(refuse(
            format!("The undershoot ({} mV) is above the resting potential ({} mV), but the undershoot is the dip below rest.", show(under), show(rest)),
            "Type a lower value, or the resting potential itself for no undershoot.",
        ));
    }
    if under < rest && rest - under < MIN_DIP {
        return Err(refuse(
            format!("An undershoot only {} mV below rest ({} mV) is too shallow to see.", show(rest - under), show(rest)),
            "Type a value at least 3 mV below the resting potential, or the resting potential itself for no undershoot.",
        ));
    }
    if ts < MIN_LEAD {
        return Err(refuse(
            format!("The stimulus time ({} ms) is before {} ms, so the stimulus arrow would lie on the y axis and there would be no resting stretch before it.", show(ts), show(MIN_LEAD)),
            "Type 0.5 ms or later.",
        ));
    }
    if dur > MAX_DUR {
        return Err(refuse(
            format!("A total time of {} ms is over the {} ms the figure draws: the 0.8 ms upstroke would be under 35 px wide, too narrow to label.", show(dur), show(MAX_DUR)),
            "Type a shorter time.",
        ));
    }
    if ts + NEED > dur + 1e-9 {
        return Err(refuse(
            format!("The spike needs about {} ms after the stimulus to settle back to rest, but a stimulus at {} ms leaves only {} ms of a {} ms trace.", show(NEED), show(ts), show((dur - ts).max(0.0)), show(dur)),
            &format!("Make the total time at least {} ms, or move the stimulus earlier.", show(((ts + NEED) * 10.0).ceil() / 10.0)),
        ));
    }
    Ok(Spike { rest, thr, peak, under, ts, dur, fail: p.b("fail") })
}

// ================================================================== the curve

fn smooth(w: f64) -> f64 { let w = w.clamp(0.0, 1.0); w * w * (3.0 - 2.0 * w) }

impl Spike {
    pub fn t_peak(&self) -> f64 { self.ts + T_RISE }
    pub fn t_low(&self) -> f64 { self.ts + T_RISE + T_FALL }

    /// The membrane potential at time `t` (ms).
    pub fn v(&self, t: f64) -> f64 {
        if t <= self.ts { return self.rest; }
        if self.fail {
            let x = (t - self.ts) / T_BUMP;
            return self.rest + BUMP_FRAC * (self.thr - self.rest) * x * x * (2.0 * (1.0 - x)).exp();
        }
        let tp = self.t_peak();
        if t <= tp {
            let u = (t - self.ts) / T_RISE;
            return self.rest + (self.peak - self.rest) * smooth(u.powf(SKEW));
        }
        let tl = self.t_low();
        if t <= tl {
            let u = (t - tp) / T_FALL;
            return self.peak - (self.peak - self.under) * smooth(u);
        }
        let x = K_BACK * (t - tl);
        self.rest + (self.under - self.rest) * (1.0 + x) * (-x).exp()
    }

    /// Where a monotone stretch of the curve reaches `level`, by bisection.
    fn cross(&self, mut a: f64, mut b: f64, level: f64) -> f64 {
        let up = self.v(b) > self.v(a);
        for _ in 0..80 {
            let m = (a + b) / 2.0;
            if (self.v(m) < level) == up { a = m } else { b = m }
        }
        (a + b) / 2.0
    }

    /// The upstroke passes the threshold (a successful spike only).
    pub fn t_threshold(&self) -> f64 { self.cross(self.ts, self.t_peak(), self.thr) }
    /// The falling phase returns to the resting level (the end of the fall when there is no undershoot).
    pub fn t_back_to_rest(&self) -> f64 {
        if self.under < self.rest { self.cross(self.t_peak(), self.t_low(), self.rest) } else { self.t_low() }
    }
    fn dips(&self) -> bool { self.under < self.rest }
    fn t_linger_end(&self) -> f64 { self.t_low() + T_LINGER }
}

// ================================================================== the plan

#[derive(Clone, Copy, Debug)]
struct Frame {
    l: f64,
    t: f64,
    w: f64,
    h: f64,
    lo: f64,
    hi: f64,
    dur: f64,
}

impl Frame {
    fn px(&self, t: f64) -> f64 { self.l + t / self.dur * self.w }
    fn py(&self, v: f64) -> f64 { self.t + self.h - (v - self.lo) / (self.hi - self.lo) * self.h }
}

type P2 = (f64, f64);

fn dist_seg(p: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}

fn dist_poly(p: P2, pts: &[P2]) -> f64 {
    pts.windows(2).map(|s| dist_seg(p, s[0], s[1])).fold(f64::INFINITY, f64::min)
}

fn seg_cross(a: P2, b: P2, c: P2, d: P2) -> bool {
    let o = |p: P2, q: P2, r: P2| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) && d1 != 0.0 && d2 != 0.0 && d3 != 0.0 && d4 != 0.0
}

/// The trace as pixel points, every segment at most `limit` long (the key times are always sampled).
fn trace(sp: &Spike, f: &Frame, limit: f64) -> Vec<P2> {
    let mut ts: Vec<f64> = (0..=96).map(|i| sp.dur * i as f64 / 96.0).collect();
    let keys = [sp.ts, sp.t_peak(), sp.t_low(), sp.ts + T_BUMP, sp.t_threshold_safe(), sp.t_back_to_rest()];
    ts.extend(keys.iter().copied().filter(|t| *t > 0.0 && *t < sp.dur));
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ts.dedup();
    let pt = |t: f64| (f.px(t), f.py(sp.v(t)));
    for _ in 0..24 {
        let mut next = Vec::with_capacity(ts.len() * 2);
        let mut grew = false;
        for w in ts.windows(2) {
            next.push(w[0]);
            let (a, b) = (pt(w[0]), pt(w[1]));
            if ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() > limit && next.len() < 60_000 {
                next.push((w[0] + w[1]) / 2.0);
                grew = true;
            }
        }
        next.push(*ts.last().unwrap());
        ts = next;
        if !grew { break; }
    }
    ts.iter().map(|&t| pt(t)).collect()
}

impl Spike {
    /// The threshold time when there is a spike; the stimulus time when there is none (only used as a sample point).
    fn t_threshold_safe(&self) -> f64 { if self.fail { self.ts } else { self.t_threshold() } }
}

/// One numbered or lettered pointer.
#[derive(Clone, Debug)]
struct Pointer {
    mark: String,
    name: &'static str,
    /// The disc centre and, when it points at the curve, the dot on the curve it points to.
    disc: P2,
    dot: Option<P2>,
}

#[derive(Clone, Debug)]
struct Band {
    from: f64,
    to: f64,
    lane: usize,
    /// Index into `Plan::pointers` of its lettered disc.
    key: &'static str,
    colour_slot: usize,
}

#[derive(Clone, Debug)]
struct Plan {
    sp: Spike,
    f: Frame,
    w: f64,
    h: f64,
    lab: f64,
    r: f64,
    pointers: Vec<Pointer>,
    bands: Vec<Band>,
    lane_top: f64,
    lane_h: f64,
    yticks: Vec<(f64, String)>,
    xticks: Vec<(f64, String)>,
    show_thr: bool,
    thr_disc: bool,
    stim: Option<(f64, f64, f64)>,
    key_top: f64,
    key_rows: usize,
    key_col_w: f64,
    key_row_h: f64,
    cap: Vec<String>,
    cap_top: f64,
    notes: Vec<String>,
    title_y: f64,
}

const NAMES_COMMON: [&str; 8] = [
    "Stimulus",
    "Resting potential",
    "Threshold",
    "Depolarisation",
    "Peak",
    "Repolarisation",
    "Hyperpolarisation (undershoot)",
    "Return to resting potential",
];
const NAME_LOCAL: &str = "Local (graded) response";
const NAME_NA: &str = "Na+ channels open";
const NAME_K: &str = "K+ channels open";
const NAME_ABS: &str = "Absolute refractory period";
const NAME_REL: &str = "Relative refractory period";

/// The sentence the picture and the editor note both give, with no phase name in it (a student copy carries it).
const NOTE_NO_DIP: &str = "The lowest point is the resting level, so the trace has no dip below rest and the items that belong to a dip are left out.";
const NOTE_FAILED: &str = "This input is too weak to start a spike, so the bands that belong to a spike are not drawn.";

fn notes_for_spike(sp: &Spike, p: &Params) -> Vec<String> {
    let mut v = Vec::new();
    if sp.fail {
        if p.b("show_channels") || p.b("show_refractory") { v.push(NOTE_FAILED.to_string()); }
    } else if !sp.dips() && (p.b("show_refractory") || p.b("show_phases")) {
        v.push(NOTE_NO_DIP.to_string());
    }
    v
}

/// Shape of the text for a label: `Na+` is drawn with a raised plus, but measured and read as plain text.
fn name_text(x: f64, y: f64, size: f64, name: &str) -> String {
    let (head, tail) = match name.split_once('+') {
        Some((a, b)) if a.ends_with("Na") || a.ends_with('K') => (a, Some(b)),
        _ => (name, None),
    };
    let mut out = format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="start" font-family="{FONT}" fill="{INK}">{}"##, svg::esc(head));
    if let Some(tail) = tail {
        out.push_str(&format!(r#"<tspan dy="{:.2}" font-size="{:.2}">+</tspan>"#, -0.42 * size, 0.7 * size));
        out.push_str(&format!(r#"<tspan dy="{:.2}" font-size="{size}">{}</tspan>"#, 0.42 * size, svg::esc(tail)));
    }
    out.push_str("</text>");
    out
}

/// The padded range and its tick step; ticks are at least `min_gap` px apart.
fn y_range(sp: &Spike, need_lo_rest: f64, need_lo_low: f64, need_hi: f64, min_gap: f64) -> (f64, f64, f64) {
    let low = sp.under.min(sp.rest);
    let mut pxmv = PLOT_H / (sp.peak - low + 40.0);
    let mut best = (0.0, 0.0, 1.0);
    for _ in 0..40 {
        let lo_raw = (sp.rest - need_lo_rest / pxmv).min(low - need_lo_low / pxmv);
        let hi_raw = sp.peak + need_hi / pxmv;
        let mut target = 11.0;
        let (mut lo, mut hi, mut step);
        loop {
            step = nice_step(hi_raw - lo_raw, target);
            lo = (lo_raw / step + 1e-9).floor() * step;
            hi = (hi_raw / step - 1e-9).ceil() * step;
            if PLOT_H / (hi - lo) * step >= min_gap || target <= 2.0 { break; }
            target -= 1.0;
        }
        best = (lo, hi, step);
        let now = PLOT_H / (hi - lo);
        if now >= pxmv * (1.0 - 1e-9) { break; }
        pxmv = now;
    }
    best
}

fn plan(p: &Params) -> Result<Plan, Refusal> {
    let sp = read(p)?;
    let lab = svg::label_factor(p.s("label_size"));
    let r = 9.5 * lab;
    let fs = 12.0 * lab;
    let tfs = 13.0 * lab;
    let kfs = 13.0 * lab;
    let show_thr = p.b("show_threshold");
    let show_stim = p.b("show_stimulus");
    let phases = p.b("show_phases");
    let channels = p.b("show_channels") && !sp.fail;
    let refr = p.b("show_refractory") && !sp.fail;
    let rel = refr && sp.dips();
    let dips = sp.dips();

    // ---- which items exist, in order
    let mut numbered: Vec<&'static str> = Vec::new();
    if phases {
        if show_stim { numbered.push(NAMES_COMMON[0]); }
        numbered.push(NAMES_COMMON[1]);
        if show_thr { numbered.push(NAMES_COMMON[2]); }
        if sp.fail {
            numbered.push(NAME_LOCAL);
            numbered.push(NAMES_COMMON[7]);
        } else {
            numbered.push(NAMES_COMMON[3]);
            numbered.push(NAMES_COMMON[4]);
            numbered.push(NAMES_COMMON[5]);
            if dips { numbered.push(NAMES_COMMON[6]); }
            numbered.push(NAMES_COMMON[7]);
        }
    }
    let mut lettered: Vec<&'static str> = Vec::new();
    if channels { lettered.push(NAME_NA); lettered.push(NAME_K); }
    if refr { lettered.push(NAME_ABS); if rel { lettered.push(NAME_REL); } }
    let n_items = numbered.len() + lettered.len();

    // ---- key geometry (independent of the plot)
    let name_w = |s: &str| text_w(s, kfs);
    // 6 % of slack: the table behind `text_w` is an estimate of the face, and a name must never reach the edge
    let blank_w = numbered.iter().chain(lettered.iter()).map(|s| name_w(s)).fold(0.0, f64::max).max(90.0 * lab) * 1.06;
    let key_cols = if n_items >= 4 { 2 } else { 1 };
    let key_rows = if n_items == 0 { 0 } else { n_items.div_ceil(key_cols) };
    let key_col_w = 2.0 * r + 10.0 + blank_w;
    let key_w = key_cols as f64 * key_col_w + (key_cols as f64 - 1.0) * 28.0;

    // ---- vertical range
    let arrow_l = 24.0 * lab;
    let need_lo_rest = if show_stim { arrow_l + 2.0 * r + 12.0 } else { 6.0 };
    let need_lo_low = if phases { 2.0 * r + 18.0 } else { 6.0 };
    let need_hi = if phases { 2.0 * r + 18.0 } else { 8.0 };
    let (lo, hi, ystep) = y_range(&sp, need_lo_rest, need_lo_low, need_hi, 2.6 * fs);
    let yticks: Vec<(f64, String)> = {
        let k0 = (lo / ystep - 1e-9).ceil() as i64;
        let k1 = (hi / ystep + 1e-9).floor() as i64;
        (k0..=k1).map(|k| (k as f64 * ystep, fmt(k as f64 * ystep))).collect()
    };
    let tick_w = yticks.iter().map(|t| text_w(&t.1, fs)).fold(0.0, f64::max);
    let ml = 8.0 + 1.15 * tfs + 8.0 + tick_w + 12.0;
    let thr_disc = show_thr && phases;
    let mr_disc = if thr_disc { 14.0 + 2.0 * r + 10.0 } else { 14.0 };
    // the last tick label is centred on the right end of the axis
    let dur_label = fmt(sp.dur);
    let mr = mr_disc.max(text_w(&dur_label, fs) / 2.0 + 6.0);
    let plot_w = PLOT_W_MIN.max(key_w + 2.0 * MARGIN - ml - mr);
    let w = ml + plot_w + mr;

    // ---- horizontal ticks
    let mut target = 8.0;
    let (xstep, xticks) = loop {
        let step = nice_step(sp.dur, target);
        let k1 = (sp.dur / step + 1e-9).floor() as i64;
        let ticks: Vec<(f64, String)> = (0..=k1).map(|k| (k as f64 * step, fmt(k as f64 * step))).collect();
        let wmax = ticks.iter().map(|t| text_w(&t.1, fs)).fold(0.0, f64::max);
        if step / sp.dur * plot_w >= wmax + 12.0 || target <= 2.0 { break (step, ticks); }
        target -= 1.0;
    };
    let _ = xstep;

    // ---- vertical layout
    let mt = 0.8 * fs + 6.0;
    let f = Frame { l: ml, t: mt, w: plot_w, h: PLOT_H, lo, hi, dur: sp.dur };
    let bottom = mt + PLOT_H;
    let title_y = bottom + 8.0 + 1.3 * fs + 6.0 + 0.8 * tfs;
    let lane_top = title_y + 0.25 * tfs + 10.0;
    let lane_h = 2.0 * r + 22.0;
    let lanes = usize::from(channels) + usize::from(refr);
    let lanes_end = lane_top + lanes as f64 * lane_h;
    let key_row_h = 2.0 * r + 8.0;
    let key_top = lanes_end + if n_items > 0 { 14.0 } else { 0.0 };
    let key_end = key_top + key_rows as f64 * key_row_h;
    let notes = notes_for_spike(&sp, p);
    let cap: Vec<String> = notes.iter().flat_map(|n| wrap(n, w - 2.0 * MARGIN, 12.0 * lab)).collect();
    let cap_top = key_end + if cap.is_empty() { 0.0 } else { 8.0 };
    let h = cap_top + cap.len() as f64 * 16.0 * lab + 12.0;

    // ---- bands
    let mut bands = Vec::new();
    if channels {
        bands.push(Band { from: sp.t_threshold(), to: sp.t_peak(), lane: 0, key: NAME_NA, colour_slot: 1 });
        bands.push(Band { from: sp.t_peak(), to: if dips { sp.t_linger_end() } else { sp.t_low() }, lane: 0, key: NAME_K, colour_slot: 2 });
    }
    if refr {
        let lane = usize::from(channels);
        bands.push(Band { from: sp.t_threshold(), to: sp.t_back_to_rest(), lane, key: NAME_ABS, colour_slot: 3 });
        if rel { bands.push(Band { from: sp.t_back_to_rest(), to: sp.t_linger_end(), lane, key: NAME_REL, colour_slot: 4 }); }
    }

    // ---- obstacles for the discs
    let coarse = trace(&sp, &f, COARSE_PX);
    let (x0, x1) = (f.l, f.l + f.w);
    let mut hlines: Vec<(P2, P2)> = Vec::new();
    if show_thr { hlines.push(((x0, f.py(sp.thr)), (x1 + 12.0, f.py(sp.thr)))); }
    if lo < 0.0 && 0.0 < hi { hlines.push(((x0, f.py(0.0)), (x1, f.py(0.0)))); }
    let mut arrow: Option<(f64, f64, f64)> = None;
    let mut segs: Vec<(P2, P2)> = Vec::new();
    if show_stim {
        let xs = f.px(sp.ts);
        let y_top = f.py(sp.rest) + 4.0;
        let y_tail = f.py(sp.rest) + arrow_l;
        arrow = Some((xs, y_top, y_tail));
        segs.push(((xs, y_top - 4.0), (xs, y_tail)));
    }
    let mut discs: Vec<(P2, f64)> = Vec::new();
    let mut leaders: Vec<(P2, P2)> = Vec::new();
    let mut pointers: Vec<Pointer> = Vec::new();

    // numbered ones, in order
    let mut counter = 0usize;
    for &name in &numbered {
        counter += 1;
        let mark = counter.to_string();
        if name == NAMES_COMMON[0] {
            let (xs, _, y_tail) = arrow.unwrap();
            let c = (xs, y_tail + 3.0 + r);
            if dist_poly(c, &coarse) < r + 4.0 || c.1 + r > f.t + f.h - 3.0 {
                return Err(refuse(
                    "The stimulus pointer would touch the curve: the dip after the spike passes too close to the stimulus arrow.".to_string(),
                    "Move the stimulus later, shorten the total time, or make the undershoot shallower.",
                ));
            }
            discs.push((c, r));
            pointers.push(Pointer { mark, name, disc: c, dot: None });
            continue;
        }
        if name == NAMES_COMMON[2] {
            let c = (x1 + 14.0 + r, f.py(sp.thr));
            discs.push((c, r));
            pointers.push(Pointer { mark, name, disc: c, dot: None });
            continue;
        }
        let anchors = anchors_for(&sp, name);
        let dirs = dirs_for(name);
        let lens = [r + 14.0 * lab, r + 24.0 * lab, r + 36.0 * lab, r + 50.0 * lab];
        let mut placed: Option<(P2, P2)> = None;
        'search: for strict in [true, false] {
            for &t in &anchors {
                let dot = (f.px(t), f.py(sp.v(t)));
                // a dot on a guide line is only allowed once nothing better exists (a peak at exactly 0 mV sits on the zero line)
                if strict && hlines.iter().any(|(a, b)| dist_seg(dot, *a, *b) < 6.0) { continue; }
                for &(dx, dy) in &dirs {
                    let n = (dx * dx + dy * dy).sqrt();
                    let (ux, uy) = (dx / n, dy / n);
                    for &len in &lens {
                        let c = (dot.0 + ux * len, dot.1 + uy * len);
                        if c.0 < f.l + r + 4.0 || c.0 > x1 - r - 4.0 || c.1 < f.t + r + 4.0 || c.1 > f.t + f.h - r - 4.0 { continue; }
                        if dist_poly(c, &coarse) < r + 4.0 + 0.5 { continue; }
                        if hlines.iter().any(|(a, b)| dist_seg(c, *a, *b) < r + 3.0) { continue; }
                        if segs.iter().any(|(a, b)| dist_seg(c, *a, *b) < r + 4.0) { continue; }
                        if discs.iter().any(|(d, dr)| ((c.0 - d.0).powi(2) + (c.1 - d.1).powi(2)).sqrt() < r + dr + 4.0) { continue; }
                        let edge = (dot.0 + ux * (len - r), dot.1 + uy * (len - r));
                        if discs.iter().any(|(d, dr)| dist_seg(*d, dot, edge) < dr + 2.0) { continue; }
                        // the leader must leave the curve, not run along it
                        let mut ok = true;
                        let steps = ((len - r) / 2.0).ceil().max(1.0) as usize;
                        for s in 1..=steps {
                            let q = (dot.0 + ux * (len - r) * s as f64 / steps as f64, dot.1 + uy * (len - r) * s as f64 / steps as f64);
                            let d = q.0 - dot.0;
                            let e = q.1 - dot.1;
                            if (d * d + e * e).sqrt() > 7.0 && dist_poly(q, &coarse) < 2.5 { ok = false; break; }
                        }
                        if !ok { continue; }
                        if strict {
                            if hlines.iter().any(|(a, b)| seg_cross(dot, edge, *a, *b)) { continue; }
                            if leaders.iter().any(|(a, b)| seg_cross(dot, edge, *a, *b)) { continue; }
                        }
                        placed = Some((c, dot));
                        break 'search;
                    }
                }
            }
        }
        let Some((c, dot)) = placed else {
            return Err(refuse(
                format!("There is no clear place for the pointer to \"{name}\" with these values: it would touch the curve, a guide line or another pointer."),
                "Change the potentials or the times a little, or turn the phase pointers off.",
            ));
        };
        let edge = (dot.0 + (c.0 - dot.0) * (1.0 - r / ((c.0 - dot.0).powi(2) + (c.1 - dot.1).powi(2)).sqrt()), dot.1 + (c.1 - dot.1) * (1.0 - r / ((c.0 - dot.0).powi(2) + (c.1 - dot.1).powi(2)).sqrt()));
        leaders.push((dot, edge));
        discs.push((c, r));
        pointers.push(Pointer { mark, name, disc: c, dot: Some(dot) });
    }
    // lettered ones, under their brackets
    for (i, &name) in lettered.iter().enumerate() {
        let b = bands.iter().find(|b| b.key == name).unwrap();
        let cx = (f.px(b.from) + f.px(b.to)) / 2.0;
        let cy = lane_top + b.lane as f64 * lane_h + 12.0 + 4.0 + r;
        let mark = ((b'A' + i as u8) as char).to_string();
        pointers.push(Pointer { mark, name, disc: (cx, cy), dot: None });
    }
    // two lettered discs in one lane must not touch: a narrow band's letter is moved clear of its wide neighbour's
    for lane in 0..lanes {
        let names: Vec<&str> = bands.iter().filter(|b| b.lane == lane).map(|b| b.key).collect();
        if names.len() == 2 {
            let at = |n: &str| pointers.iter().position(|p| p.name == n).unwrap();
            let (i, j) = (at(names[0]), at(names[1]));
            let (a, c) = if pointers[i].disc.0 <= pointers[j].disc.0 { (i, j) } else { (j, i) };
            let gap = pointers[c].disc.0 - pointers[a].disc.0;
            if gap < 2.0 * r + 6.0 {
                let push = (2.0 * r + 6.0 - gap) / 2.0;
                pointers[a].disc.0 -= push;
                pointers[c].disc.0 += push;
            }
            if pointers[a].disc.0 < f.l + r + 2.0 || pointers[c].disc.0 > x1 - r - 2.0 {
                return Err(refuse(
                    "The two bands in one row are too close together, and too near the edge, for their letters.".to_string(),
                    "Move the stimulus later, or turn one of the band switches off.",
                ));
            }
        }
    }

    Ok(Plan {
        sp, f, w, h, lab, r, pointers, bands, lane_top, lane_h, yticks, xticks, show_thr,
        thr_disc, stim: arrow, key_top, key_rows, key_col_w, key_row_h, cap, cap_top, notes, title_y,
    })
}

/// Where along the curve (times, in order of preference) the pointer to this phase may point.
fn anchors_for(sp: &Spike, name: &str) -> Vec<f64> {
    let (ts, tp, tl) = (sp.ts, sp.t_peak(), sp.t_low());
    let frac = |a: f64, b: f64, fs: &[f64]| -> Vec<f64> { fs.iter().map(|k| a + (b - a) * k).collect() };
    if sp.fail {
        let tb = ts + T_BUMP;
        return match name {
            n if n == NAMES_COMMON[1] => rest_anchors(sp),
            n if n == NAME_LOCAL => vec![tb, tb - 0.15, tb + 0.2, tb + 0.45],
            _ => vec![tb + 1.0, tb + 1.4, tb + 0.8, tb + 1.9, tb + 2.4],
        };
    }
    match name {
        n if n == NAMES_COMMON[1] => rest_anchors(sp),
        n if n == NAMES_COMMON[3] => {
            let mut v = Vec::new();
            for f in [0.5, 0.35, 0.65, 0.8, 0.2] { v.push(sp.cross(sp.t_threshold(), tp, sp.thr + f * (sp.peak - sp.thr))); }
            v
        }
        n if n == NAMES_COMMON[4] => vec![tp, tp - 0.04, tp + 0.04, tp - 0.08, tp + 0.08],
        n if n == NAMES_COMMON[5] => {
            let low = sp.rest;
            let mut v = Vec::new();
            for f in [0.5, 0.35, 0.65, 0.8, 0.2] { v.push(sp.cross(tp, sp.t_back_to_rest().max(tp + 1e-6), sp.peak - f * (sp.peak - low))); }
            v
        }
        n if n == NAMES_COMMON[6] => vec![tl, tl - 0.2, tl + 0.2, tl - 0.4, tl + 0.4],
        _ => {
            let a = tl + T_LINGER;
            let mut v = frac(a, ts + NEED, &[0.0, 0.25, 0.5, 0.75, 1.0]);
            v.extend(frac(ts + NEED, sp.dur, &[0.2, 0.6]));
            v
        }
    }
}

fn rest_anchors(sp: &Spike) -> Vec<f64> {
    let mut v = vec![sp.ts * 0.5, sp.ts * 0.3, sp.ts * 0.75];
    let end = sp.ts + NEED;
    v.extend([end + (sp.dur - end) * 0.5, end + (sp.dur - end) * 0.85, end + (sp.dur - end) * 0.15]);
    v
}

fn dirs_for(name: &str) -> Vec<P2> {
    // screen directions: y grows downwards
    let (u, ul, ur, l, rr, dl, dr, d) = ((0.0, -1.0), (-1.0, -1.0), (1.0, -1.0), (-1.0, 0.0), (1.0, 0.0), (-1.0, 1.0), (1.0, 1.0), (0.0, 1.0));
    match name {
        n if n == NAMES_COMMON[3] => vec![l, ul, dl, u, d, ur, dr, rr],
        n if n == NAMES_COMMON[4] => vec![u, ul, ur, l, rr, dl, dr, d],
        n if n == NAMES_COMMON[5] => vec![rr, ur, dr, d, u, dl, ul, l],
        n if n == NAMES_COMMON[6] => vec![d, dl, dr, l, rr, ul, ur, u],
        n if n == NAMES_COMMON[1] => vec![u, ul, ur, d, dl, dr, l, rr],
        _ => vec![u, ur, ul, d, dr, dl, rr, l],
    }
}

// ================================================================== the picture

pub fn warnings(p: &Params) -> Vec<String> {
    match plan(p) {
        Err(e) => vec![format!("The action potential could not be drawn: {e}")],
        Ok(pl) => pl.notes,
    }
}

fn message(why: &str, then: &str, lab: f64) -> String {
    let max = MIN_W - 48.0;
    let head = wrap("The action potential could not be drawn:", max, 13.0 * lab);
    let first = wrap(why, max, 13.0 * lab);
    let last = wrap(then, max, 12.0 * lab);
    let mut y = 22.0 + 18.0 * lab;
    let mut lines: Vec<(f64, f64, &str, &str)> = Vec::new();
    for l in &head { lines.push((y, 13.0 * lab, ERR, l)); y += 18.0 * lab; }
    y += 6.0 * lab;
    for l in &first { lines.push((y, 13.0 * lab, INK, l)); y += 18.0 * lab; }
    y += 6.0 * lab;
    for l in &last { lines.push((y, 12.0 * lab, INK, l)); y += 16.0 * lab; }
    let last_baseline = lines.last().map_or(y, |l| l.0);
    let mut d = Doc::new(MIN_W, last_baseline + 4.0 * lab + 20.0);
    d.describe("Action potential", "An action potential could not be drawn from the typed values");
    for (y, size, colour, text) in lines { d.raw(&text_el(24.0, y, size, "start", colour, text)); }
    d.finish()
}

pub fn build(p: &Params) -> String {
    match plan(p) {
        Err(e) => message(&e.why, &e.then, svg::label_factor(p.s("label_size"))),
        Ok(pl) => draw(p, &pl),
    }
}

fn disc(d: &mut Doc, c: P2, r: f64, mark: &str, dfs: f64) {
    d.circle(c.0, c.1, r, "#ffffff", INK, 1.4);
    d.raw(&text_el(c.0, c.1 + 0.35 * dfs, dfs, "middle", INK, mark));
}

fn draw(p: &Params, pl: &Plan) -> String {
    let fills = svg::Fills::new(p);
    let print = fills.is_print();
    let key = p.b(SHOW_ANSWERS);
    let (lab, r, f, sp) = (pl.lab, pl.r, pl.f, pl.sp);
    let fs = 12.0 * lab;
    let tfs = 13.0 * lab;
    let kfs = 13.0 * lab;
    let dfs = 12.0 * lab;
    let curve_col = fills.line(0);
    let thr_col = if print { "#444444" } else { "#c0392b" };
    let zero_col = if print { "#666666" } else { "#8a8f98" };
    let mut d = Doc::new(pl.w, pl.h);
    let names: Vec<&str> = pl.pointers.iter().map(|q| q.name).collect();
    let desc = if key && !names.is_empty() {
        format!("Membrane potential in mV against time in ms for a neuron{}. Pointers: {}.", if sp.fail { ", a trace that fails to start a spike" } else { "" }, names.join("; "))
    } else {
        format!("Membrane potential in mV against time in ms for a neuron{}. {} pointers to name.", if sp.fail { ", a trace that fails to start a spike" } else { "" }, pl.pointers.len())
    };
    d.describe("Action potential", &desc);
    d.defs(&fills.defs());

    // axes
    let (x0, x1, yb, yt) = (f.l, f.l + f.w, f.t + f.h, f.t);
    if f.lo < 0.0 && 0.0 < f.hi { d.line(x0, f.py(0.0), x1, f.py(0.0), zero_col, 1.3); }
    d.line(x0, yb, x1, yb, INK, 1.8);
    d.line(x0, yt, x0, yb, INK, 1.8);
    for (v, label) in &pl.yticks {
        let y = f.py(*v);
        d.line(x0 - 6.0, y, x0, y, INK, 1.4);
        d.raw(&text_el(x0 - 10.0, y + fs * 0.35, fs, "end", INK, label));
    }
    for (v, label) in &pl.xticks {
        let x = f.px(*v);
        d.line(x, yb, x, yb + 6.0, INK, 1.4);
        d.raw(&text_el(x, yb + 8.0 + 1.05 * fs, fs, "middle", INK, label));
    }
    d.raw(&text_el(x0 + f.w / 2.0, pl.title_y, tfs, "middle", INK, "Time (ms)"));
    let yx = 8.0 + 0.85 * tfs;
    d.raw(&format!(
        r##"<text transform="translate({yx:.2} {:.2}) rotate(-90)" font-size="{tfs}" text-anchor="middle" font-family="{FONT}" fill="{INK}">Membrane potential (mV)</text>"##,
        f.t + f.h / 2.0
    ));

    // threshold line
    if pl.show_thr {
        let y = f.py(sp.thr);
        let end = if pl.thr_disc { x1 + 12.0 } else { x1 };
        d.raw(&format!(r#"<line x1="{x0:.2}" y1="{y:.2}" x2="{end:.2}" y2="{y:.2}" stroke="{thr_col}" stroke-width="1.5" stroke-dasharray="7 5"/>"#));
    }
    // stimulus arrow
    if let Some((xs, y_top, y_tail)) = pl.stim { d.arrow(xs, y_tail, xs, y_top, INK, 1.8); }
    // the trace
    let pts = trace(&sp, &f, SEG_PX);
    let mut path = String::with_capacity(pts.len() * 14);
    for (i, q) in pts.iter().enumerate() { path.push_str(&format!("{}{:.2},{:.2}", if i == 0 { "M" } else { "L" }, q.0, q.1)); }
    d.path(&path, "none", curve_col, 2.6);

    // bands
    for b in &pl.bands {
        let y = pl.lane_top + b.lane as f64 * pl.lane_h + 12.0;
        let (a, c) = (f.px(b.from) + 1.5, f.px(b.to) - 1.5);
        let col = fills.line(b.colour_slot);
        d.path(&format!("M{a:.2},{:.2}V{y:.2}H{c:.2}V{:.2}", y - 6.0, y - 6.0), "none", col, 2.2);
    }
    // pointers
    for q in &pl.pointers {
        if let Some(dot) = q.dot {
            let dist = ((q.disc.0 - dot.0).powi(2) + (q.disc.1 - dot.1).powi(2)).sqrt();
            let k = 1.0 - r / dist;
            d.line(dot.0, dot.1, dot.0 + (q.disc.0 - dot.0) * k, dot.1 + (q.disc.1 - dot.1) * k, INK, 1.2);
            d.circle(dot.0, dot.1, 2.6, INK, INK, 0.5);
        }
        disc(&mut d, q.disc, r, &q.mark, dfs);
    }
    if pl.thr_disc && pl.show_thr {
        // the dashed line stops at the disc; nothing more to draw
    }

    // the key: both copies lay out identically
    let blank_w = pl.key_col_w - 2.0 * r - 10.0;
    let rows = pl.key_rows.max(1);
    for (i, q) in pl.pointers.iter().enumerate() {
        let (col, row) = (i / rows, i % rows);
        let cx = MARGIN + col as f64 * (pl.key_col_w + 28.0) + r;
        let cy = pl.key_top + row as f64 * pl.key_row_h + r + 2.0;
        disc(&mut d, (cx, cy), r, &q.mark, dfs);
        let x = cx + r + 10.0;
        if key {
            d.raw(&name_text(x, cy + 0.35 * kfs, kfs, q.name));
        } else {
            d.line(x, cy + 0.35 * kfs + 3.0, x + blank_w, cy + 0.35 * kfs + 3.0, INK, 1.0);
        }
    }
    for (i, l) in pl.cap.iter().enumerate() {
        d.raw(&text_el(MARGIN, pl.cap_top + 12.0 * lab + i as f64 * 16.0 * lab, 12.0 * lab, "start", INK, l));
    }
    d.finish()
}

// ================================================================== examples

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "The textbook spike with the channels", params: vec![] },
        Example { name: "Label the phases (student copy)", params: vec![b(SHOW_ANSWERS, false), b("show_channels", false)] },
        Example { name: "Refractory periods", params: vec![b("show_channels", false), b("show_refractory", true), b("show_phases", false)] },
        Example { name: "A stimulus that is too weak", params: vec![b("fail", true), s("stimulus", "1.5")] },
        Example { name: "Cardiac muscle: resting -85, threshold -65, peak +30", params: vec![s("resting", "-85"), s("threshold", "-65"), s("peak", "30"), s("undershoot", "-90"), s("duration", "9")] },
        Example { name: "No undershoot", params: vec![s("undershoot", "-70"), b("show_channels", true), b("show_refractory", true)] },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
    fn with(list: &[(&'static str, Value)]) -> Params {
        let d = def();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }
    fn sp(rest: f64, thr: f64, peak: f64, under: f64, ts: f64, dur: f64, fail: bool) -> Spike { Spike { rest, thr, peak, under, ts, dur, fail } }

    // ------------------------------------------------------------------ a tiny SVG reader of my own
    type P = (f64, f64);
    struct Tx { x: f64, y: f64, size: f64, anchor: String, s: String, rot: bool }
    fn attr(el: &str, k: &str) -> Option<String> {
        let el = format!(" {el}");
        let at = el.find(&format!(" {k}=\""))? + k.len() + 3;
        el[at..].split('"').next().map(str::to_string)
    }
    fn num(el: &str, k: &str) -> f64 { attr(el, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
    fn unesc(s: &str) -> String { s.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&") }
    fn strip_tags(body: &str) -> String {
        let mut out = String::new();
        let mut rest = body;
        while let Some(i) = rest.find('<') {
            out.push_str(&rest[..i]);
            rest = &rest[rest[i..].find('>').map_or(rest.len(), |j| i + j + 1)..];
        }
        out.push_str(rest);
        out
    }
    fn texts(svg: &str) -> Vec<Tx> {
        svg.split("<text ").skip(1).map(|part| {
            let head = part.split('>').next().unwrap();
            let body = &part[head.len() + 1..];
            let rot = head.contains("rotate(-90)");
            let (x, y) = if rot {
                let t = attr(head, "transform").unwrap();
                let inner = t.split("translate(").nth(1).unwrap().split(')').next().unwrap().to_string();
                let mut it = inner.split_whitespace().map(|v| v.parse::<f64>().unwrap());
                (it.next().unwrap(), it.next().unwrap())
            } else { (num(head, "x"), num(head, "y")) };
            Tx { x, y, size: num(head, "font-size"), anchor: attr(head, "text-anchor").unwrap(), s: unesc(&strip_tags(body.split("</text>").next().unwrap())), rot }
        }).collect()
    }
    fn joined(svg: &str) -> String { texts(svg).iter().map(|t| t.s.clone()).collect::<Vec<_>>().join("\n") }
    fn circles(svg: &str) -> Vec<(f64, f64, f64)> {
        svg.split("<circle ").skip(1).map(|c| { let c = c.split('>').next().unwrap(); (num(c, "cx"), num(c, "cy"), num(c, "r")) }).collect()
    }
    fn lines(svg: &str) -> Vec<(P, P)> {
        svg.split("<line ").skip(1).map(|l| { let l = l.split('>').next().unwrap(); ((num(l, "x1"), num(l, "y1")), (num(l, "x2"), num(l, "y2"))) }).collect()
    }
    fn polys(svg: &str) -> Vec<Vec<P>> {
        svg.split("<polygon ").skip(1).map(|g| {
            attr(g.split('>').next().unwrap(), "points").unwrap().split_whitespace().map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect()
        }).collect()
    }
    /// The trace (the path with straight segments) and the bracket segments (the paths with V and H).
    fn paths(svg: &str) -> (Vec<P>, Vec<(P, P)>) {
        let mut curve = Vec::new();
        let mut br = Vec::new();
        for part in svg.split("<path ").skip(1) {
            let head = part.split('>').next().unwrap();
            let d = attr(head, "d").unwrap();
            if d.contains('L') {
                curve = d.split(|c| c == 'M' || c == 'L').filter(|t| !t.is_empty()).map(|t| { let (x, y) = t.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect();
            } else if d.contains('V') {
                let a: Vec<&str> = d.trim_start_matches('M').split(['V', 'H']).collect();
                let (x0, y0) = a[0].split_once(',').map(|(x, y)| (x.parse::<f64>().unwrap(), y.parse::<f64>().unwrap())).unwrap();
                let y1: f64 = a[1].parse().unwrap();
                let x1: f64 = a[2].parse().unwrap();
                let y2: f64 = a[3].parse().unwrap();
                br.push(((x0, y0), (x0, y1)));
                br.push(((x0, y1), (x1, y1)));
                br.push(((x1, y1), (x1, y2)));
            }
        }
        (curve, br)
    }
    fn canvas(svg: &str) -> P { (num(svg, "width"), num(svg, "height")) }

    /// My own text width, from a table the module does not use.
    fn tw(st: &str, size: f64) -> f64 {
        // Arial's advance widths (a table of its own, in `plot`), 6 % wider because Noto Sans is
        st.chars().map(|c| size * 1.06 * if c == '\u{2212}' { 0.584 } else if c.is_ascii() { super::super::plot::arial_em(c, false) } else { 0.6 }).sum()
    }
    /// Text box (x0, y0, x1, y1). A rotated text is a tall box.
    fn bbox(t: &Tx) -> (f64, f64, f64, f64) {
        let w = tw(&t.s, t.size);
        if t.rot { return (t.x - 0.85 * t.size, t.y - w / 2.0, t.x + 0.3 * t.size, t.y + w / 2.0); }
        let x0 = match t.anchor.as_str() { "middle" => t.x - w / 2.0, "end" => t.x - w, _ => t.x };
        (x0, t.y - 0.82 * t.size, x0 + w, t.y + 0.25 * t.size)
    }
    fn dseg(p: P, a: P, b: P) -> f64 {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l2 = dx * dx + dy * dy;
        let t = if l2 < 1e-12 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
        ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
    }
    fn dpoly(p: P, pts: &[P]) -> f64 { pts.windows(2).map(|w| dseg(p, w[0], w[1])).fold(f64::INFINITY, f64::min) }
    fn seg_in_box(a: P, b: P, bx: (f64, f64, f64, f64), pad: f64) -> bool {
        let n = (((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt() / 0.5).ceil().max(1.0) as usize;
        (0..=n).any(|i| {
            let q = (a.0 + (b.0 - a.0) * i as f64 / n as f64, a.1 + (b.1 - a.1) * i as f64 / n as f64);
            q.0 > bx.0 - pad && q.0 < bx.2 + pad && q.1 > bx.1 - pad && q.1 < bx.3 + pad
        })
    }
    fn box_hit(a: (f64, f64, f64, f64), c: (f64, f64, f64, f64), pad: f64) -> bool {
        a.0 < c.2 + pad && c.0 < a.2 + pad && a.1 < c.3 + pad && c.1 < a.3 + pad
    }
    fn circle_box(c: (f64, f64, f64), bx: (f64, f64, f64, f64), pad: f64) -> bool {
        let nx = c.0.clamp(bx.0, bx.2);
        let ny = c.1.clamp(bx.1, bx.3);
        ((c.0 - nx).powi(2) + (c.1 - ny).powi(2)).sqrt() < c.2 + pad
    }

    /// Everything that must stay apart, measured with literal gaps (1 px between texts, 2 px between a disc and the
    /// curve or another disc), none of them read from the module. Returns the problems found.
    fn audit(svg: &str) -> Vec<String> {
        let mut bad = Vec::new();
        let (w, h) = canvas(svg);
        let tx = texts(svg);
        let discs: Vec<(f64, f64, f64)> = circles(svg).into_iter().filter(|c| c.2 >= 6.0).collect();
        let (curve, brackets) = paths(svg);
        let ls = lines(svg);
        let heads = polys(svg);
        // a disc's own mark sits inside it
        let is_mark = |t: &Tx| discs.iter().any(|d| ((t.x - d.0).powi(2) + (t.y - d.1).powi(2)).sqrt() < d.2);
        let boxes: Vec<((f64, f64, f64, f64), &Tx)> = tx.iter().filter(|t| !is_mark(t)).map(|t| (bbox(t), t)).collect();
        for (bx, t) in &boxes {
            if bx.0 < 0.0 || bx.1 < 0.0 || bx.2 > w || bx.3 > h { bad.push(format!("text leaves the canvas: {:?} {bx:?} in {w}x{h}", t.s)); }
        }
        for i in 0..boxes.len() {
            for j in i + 1..boxes.len() {
                if box_hit(boxes[i].0, boxes[j].0, 1.0) { bad.push(format!("texts touch: {:?} / {:?}", boxes[i].1.s, boxes[j].1.s)); }
            }
        }
        for (bx, t) in &boxes {
            if curve.windows(2).any(|q| seg_in_box(q[0], q[1], *bx, 2.0)) { bad.push(format!("text touches the curve: {:?}", t.s)); }
            for l in ls.iter().chain(brackets.iter()) {
                if seg_in_box(l.0, l.1, *bx, 1.0) { bad.push(format!("a line crosses text {:?}: {l:?}", t.s)); }
            }
            for g in &heads {
                if g.iter().any(|q| q.0 > bx.0 - 1.0 && q.0 < bx.2 + 1.0 && q.1 > bx.1 - 1.0 && q.1 < bx.3 + 1.0) { bad.push(format!("an arrowhead touches text {:?}", t.s)); }
            }
            for d in &discs { if circle_box(*d, *bx, 1.0) { bad.push(format!("a disc touches text {:?}", t.s)); } }
        }
        for (i, d) in discs.iter().enumerate() {
            if d.0 - d.2 < 0.0 || d.1 - d.2 < 0.0 || d.0 + d.2 > w || d.1 + d.2 > h { bad.push(format!("disc leaves the canvas {d:?}")); }
            for e in discs.iter().skip(i + 1) {
                if ((d.0 - e.0).powi(2) + (d.1 - e.1).powi(2)).sqrt() < d.2 + e.2 + 2.0 { bad.push(format!("discs touch {d:?} {e:?}")); }
            }
            if dpoly((d.0, d.1), &curve) < d.2 + 2.0 { bad.push(format!("disc touches the curve {d:?}")); }
            for l in ls.iter().chain(brackets.iter()) {
                if dseg((d.0, d.1), l.0, l.1) < d.2 - 1.0 { bad.push(format!("a line runs through disc {d:?}: {l:?}")); }
            }
            for g in &heads { if g.iter().any(|q| ((q.0 - d.0).powi(2) + (q.1 - d.1).powi(2)).sqrt() < d.2 + 1.0) { bad.push(format!("an arrowhead touches disc {d:?}")); } }
        }
        for q in &curve { if q.0 < 0.0 || q.0 > w || q.1 < 0.0 || q.1 > h { bad.push(format!("curve leaves the canvas at {q:?}")); break; } }
        for l in &ls { for q in [l.0, l.1] { if q.0 < -0.01 || q.0 > w + 0.01 || q.1 < -0.01 || q.1 > h + 0.01 { bad.push(format!("line leaves the canvas {l:?}")); } } }
        bad
    }

    /// value -> pixel maps read back from the picture's own ticks, so a test never uses the module's frame.
    struct Axes { y0: f64, ky: f64, x0: f64, kx: f64 }
    impl Axes {
        fn y(&self, v: f64) -> f64 { self.y0 + self.ky * v }
        fn x(&self, t: f64) -> f64 { self.x0 + self.kx * t }
    }
    fn axes_of(svg: &str) -> Axes {
        let ls = lines(svg);
        let tx = texts(svg);
        let mut ys: Vec<(f64, f64)> = Vec::new(); // (value, y)
        let mut xs: Vec<(f64, f64)> = Vec::new();
        for l in &ls {
            let val = |t: &Tx| t.s.replace('\u{2212}', "-").parse::<f64>().ok();
            if l.0 .1 == l.1 .1 && ((l.1 .0 - l.0 .0).abs() - 6.0).abs() < 0.01 {
                if let Some(v) = tx.iter().filter(|t| t.anchor == "end" && !t.rot && (t.y - 0.35 * t.size - l.0 .1).abs() < 0.01).find_map(val) { ys.push((v, l.0 .1)); }
            }
            if l.0 .0 == l.1 .0 && ((l.1 .1 - l.0 .1).abs() - 6.0).abs() < 0.01 {
                if let Some(v) = tx.iter().filter(|t| t.anchor == "middle" && (t.x - l.0 .0).abs() < 0.01 && t.y > l.0 .1).find_map(val) { xs.push((v, l.0 .0)); }
            }
        }
        assert!(ys.len() >= 3 && xs.len() >= 3, "ticks found: {} y, {} x", ys.len(), xs.len());
        let fit = |pts: &[(f64, f64)]| {
            let (a, c) = (pts[0], pts[pts.len() - 1]);
            let k = (c.1 - a.1) / (c.0 - a.0);
            (a.1 - k * a.0, k)
        };
        let (y0, ky) = fit(&ys);
        let (x0, kx) = fit(&xs);
        Axes { y0, ky, x0, kx }
    }

    // ------------------------------------------------------------------ the number reader
    #[test]
    fn numbers_are_read_forgivingly_and_refused_with_a_reason() {
        let mv = |t: &str| number(t, "Peak", &["millivolts", "millivolt", "mv"], "-70");
        for (t, v) in [
            ("-70", -70.0), ("\u{2212}70", -70.0), ("\u{feff}-70", -70.0), (" \t-70 mV\r\n", -70.0), ("-70mv", -70.0), ("- 70", -70.0),
            ("+40", 40.0), ("40.5", 40.5), ("40,5", 40.5), ("-0,5 MV", -0.5), ("\u{2013}55 millivolts", -55.0), ("007", 7.0), (".5", 0.5), ("0", 0.0), ("-0", 0.0),
        ] {
            assert_eq!(mv(t), Ok(v), "{t:?}");
        }
        assert!(mv("-0").unwrap().is_sign_positive());
        for t in ["", " ", "mV", "-", "abc", "1e3", "NaN", "inf", "-inf", "infinity", "1.2.3", "1,2,3", "1,000", "-1,000", "--5", "5 5", "7 V", "\u{0663}", "1.5,5", "9".repeat(400).as_str()] {
            let e = mv(t).expect_err(t);
            assert!(!e.why.is_empty() && !e.then.is_empty(), "{t:?}");
        }
        // the ambiguous comma says why, and the unit of the other control is not accepted
        assert!(mv("1,000").unwrap_err().why.contains("ambiguous"));
        assert!(number("6 mV", "Total time", &["ms"], "1.5").is_err());
        assert_eq!(number("6 MS", "Total time", &["milliseconds", "ms"], "1.5"), Ok(6.0));
    }

    fn refusal(list: &[(&'static str, Value)]) -> Refusal { read(&with(list)).expect_err("expected a refusal") }

    #[test]
    fn each_refusal_is_true_and_says_what_to_do() {
        let cases: Vec<(Vec<(&'static str, Value)>, &str, &str)> = vec![
            (vec![s("resting", "x")], "is not a number", "Nothing is drawn until"),
            (vec![s("peak", "")], "is empty", "Nothing is drawn until"),
            (vec![s("resting", "-5")], "negative at rest", "negative resting"),
            (vec![s("peak", "150")], "outside the", "inside that range"),
            (vec![s("undershoot", "-120")], "outside the", "inside that range"),
            (vec![s("threshold", "-65")], "at least 10 mV above the resting potential", "Raise the threshold"),
            (vec![s("threshold", "-3")], "within 4 mV of 0 mV", "at least 4 mV from zero"),
            (vec![s("peak", "-40")], "at least 20 mV above the threshold", "Raise the peak"),
            (vec![s("undershoot", "-60")], "is above the resting potential", "Type a lower value"),
            (vec![s("undershoot", "-72")], "too shallow", "at least 3 mV below"),
            (vec![s("stimulus", "0")], "before 0.5 ms", "Type 0.5 ms or later"),
            (vec![s("duration", "13")], "over the 12 ms", "shorter time"),
            (vec![s("duration", "5")], "needs about 4.7 ms", "at least 5.7 ms"),
        ];
        for (list, why, then) in cases {
            let e = refusal(&list);
            assert!(e.why.contains(why), "{list:?}: {} / {}", e.why, e.then);
            assert!(e.then.contains(then), "{list:?}: {} / {}", e.why, e.then);
            // a refusal is a canvas of words: 420 wide, with both lines and nothing of a figure
            let svg = (def().render)(&with(&list));
            assert_eq!(canvas(&svg).0, 420.0);
            assert!(!svg.contains("<path"), "{list:?}");
            let j = joined(&svg).replace('\n', " ");
            assert!(j.contains("could not be drawn") && j.contains(&e.then[..12]), "{j}");
        }
        // the boundaries are exactly as stated
        assert!(read(&with(&[s("threshold", "-60")])).is_ok());
        assert!(read(&with(&[s("peak", "-35")])).is_ok());
        assert!(read(&with(&[s("undershoot", "-73")])).is_ok());
        assert!(read(&with(&[s("undershoot", "-70")])).is_ok());
        assert!(read(&with(&[s("stimulus", "0.5"), s("duration", "5.2")])).is_ok());
        assert!(read(&with(&[s("stimulus", "0.5"), s("duration", "5.1")])).is_err());
        assert!(read(&with(&[s("duration", "12"), s("stimulus", "7.3")])).is_ok());
        assert!(read(&with(&[s("duration", "12"), s("stimulus", "7.4")])).is_err());
        assert!(read(&with(&[s("duration", "12.01")])).is_err());
        // the first problem in field order is the one reported, so the message never mixes two
        assert!(refusal(&[s("resting", "x"), s("peak", "y")]).why.contains("Resting"));
    }

    #[test]
    fn a_refusal_header_scales_and_the_canvas_stays_420_wide() {
        for size in ["Small", "Medium", "Large"] {
            let svg = render_of(&[s("peak", "x"), s("label_size", size)]);
            assert_eq!(canvas(&svg).0, 420.0);
            let t = texts(&svg);
            let lab = svg::label_factor(size);
            assert_eq!(t[0].size, 13.0 * lab);
            assert!(audit(&svg).is_empty(), "{size}: {:?}", audit(&svg));
        }
    }

    // ------------------------------------------------------------------ the curve
    fn family() -> Vec<Spike> {
        let mut v = Vec::new();
        for (rest, thr, peak, under) in [
            (-70.0, -55.0, 40.0, -80.0), (-70.0, -55.0, -35.0, -80.0), (-70.0, -60.0, 40.0, -80.0), (-70.0, -55.0, 40.0, -70.0),
            (-90.0, -60.0, 60.0, -100.0), (-85.0, -65.0, 30.0, -90.0), (-20.0, -5.0, 40.0, -30.0), (-70.0, -55.0, 40.0, -73.0),
            (-40.0, -30.0, 70.0, -50.0), (-100.0, -90.0, 70.0, -100.0), (-100.0, 10.0, 70.0, -100.0),
        ] {
            for (ts, dur) in [(1.0, 8.0), (0.5, 5.2), (0.5, 12.0), (7.3, 12.0), (5.0, 12.0), (2.0, 7.0)] {
                for fail in [false, true] { v.push(sp(rest, thr, peak, under, ts, dur, fail)); }
            }
        }
        v
    }

    #[test]
    fn the_curve_hits_the_typed_extremes_exactly() {
        for s in family().into_iter().filter(|s| !s.fail) {
            assert_eq!(s.v(0.0), s.rest);
            assert_eq!(s.v(s.ts), s.rest);
            assert!((s.v(s.t_peak()) - s.peak).abs() < 1e-9, "{s:?}");
            assert!((s.v(s.t_low()) - s.under).abs() < 1e-9, "{s:?}");
            let n = 40_000;
            let (mut hi, mut lo) = (f64::MIN, f64::MAX);
            for i in 0..=n { let v = s.v(s.dur * i as f64 / n as f64); hi = hi.max(v); lo = lo.min(v); }
            assert!(hi <= s.peak + 1e-9 && hi >= s.peak - 0.01, "{s:?}: highest {hi}");
            assert!(lo >= s.under - 1e-9 && lo <= s.under + 0.01, "{s:?}: lowest {lo}");
            assert!(s.v(s.dur) - s.rest <= 0.5 && s.v(s.dur) <= s.rest + 1e-9, "settled by the end: {s:?}");
        }
    }

    #[test]
    fn the_threshold_is_crossed_once_up_and_once_down_and_never_by_a_failure() {
        for s in family() {
            let n = 60_000;
            let (mut up, mut down) = (0, 0);
            let mut prev = s.v(0.0) > s.thr;
            for i in 1..=n {
                let now = s.v(s.dur * i as f64 / n as f64) > s.thr;
                if now && !prev { up += 1 }
                if !now && prev { down += 1 }
                prev = now;
            }
            if s.fail {
                assert_eq!((up, down), (0, 0), "a failed initiation must stay below threshold: {s:?}");
                let top = (0..=n).map(|i| s.v(s.dur * i as f64 / n as f64)).fold(f64::MIN, f64::max);
                assert!((top - (s.rest + BUMP_FRAC * (s.thr - s.rest))).abs() < 0.01, "{top}");
                assert!(top < s.thr - 0.3 * (s.thr - s.rest));
            } else {
                assert_eq!((up, down), (1, 1), "{s:?}");
                let t = s.t_threshold();
                assert!((s.v(t) - s.thr).abs() < 1e-9 && t > s.ts && t < s.t_peak());
            }
        }
    }

    /// The slope's sign over the whole trace: rises, falls, and (with an undershoot) rises again; no wiggle.
    #[test]
    fn the_derivative_changes_sign_only_where_the_physiology_says() {
        for s in family() {
            let n = 80_000;
            let mut signs: Vec<i8> = Vec::new();
            let mut prev = s.v(0.0);
            for i in 1..=n {
                let v = s.v(s.dur * i as f64 / n as f64);
                let dv = v - prev;
                prev = v;
                if dv.abs() < 1e-12 { continue; }
                let sg = if dv > 0.0 { 1 } else { -1 };
                if signs.last() != Some(&sg) { signs.push(sg); }
            }
            let want: &[i8] = if s.fail || s.under >= s.rest { &[1, -1] } else { &[1, -1, 1] };
            assert_eq!(signs, want, "{s:?}");
        }
    }

    #[test]
    fn the_path_is_dense_smooth_and_reaches_the_extremes_on_the_axes_of_the_picture() {
        for size in ["Small", "Medium", "Large"] {
            for dur in ["5.2", "8", "12"] {
                let svg = render_of(&[s("label_size", size), s("duration", dur), s("stimulus", "0.5")]);
                let (curve, _) = paths(&svg);
                let ax = axes_of(&svg);
                let mut worst_len: f64 = 0.0;
                let mut worst_sag: f64 = 0.0;
                // the true curve, read back through the picture's own axes, against each chord of the path
                let truth = sp(-70.0, -55.0, 40.0, -80.0, 0.5, num_of(dur), false);
                for w in curve.windows(2) {
                    worst_len = worst_len.max(((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt());
                    let t = ((w[0].0 + w[1].0) / 2.0 - ax.x(0.0)) / ax.kx;
                    let on_curve = (ax.x(t), ax.y(truth.v(t)));
                    worst_sag = worst_sag.max(dseg(on_curve, w[0], w[1]));
                }
                assert!(worst_len <= 1.0, "{size} {dur}: a {worst_len:.2} px segment");
                assert!(worst_sag < 0.25, "{size} {dur}: the path leaves the true curve by {worst_sag:.3} px");
                let top = curve.iter().map(|p| p.1).fold(f64::MAX, f64::min);
                let bot = curve.iter().map(|p| p.1).fold(f64::MIN, f64::max);
                assert!((top - ax.y(40.0)).abs() < 0.06, "{size} {dur}: peak row {top} vs {}", ax.y(40.0));
                assert!((bot - ax.y(-80.0)).abs() < 0.06, "{size} {dur}: lowest row {bot} vs {}", ax.y(-80.0));
                assert!((curve[0].1 - ax.y(-70.0)).abs() < 0.06, "rest row");
                assert!((curve[0].0 - ax.x(0.0)).abs() < 0.06 && (curve.last().unwrap().0 - ax.x(num_of(dur))).abs() < 0.06, "spans 0 to the total time");
                // the peak is at the stimulus time + 0.8 ms on the time axis
                let peak_pt = curve.iter().min_by(|a, c| a.1.partial_cmp(&c.1).unwrap()).unwrap();
                assert!((peak_pt.0 - ax.x(0.5 + 0.8)).abs() < 0.8, "{size} {dur}: peak at x {} vs {}", peak_pt.0, ax.x(1.3));
            }
        }
    }
    fn b2f(a: P, b: P) -> P { (a.0 - b.0, a.1 - b.1) }
    fn num_of(s: &str) -> f64 { s.parse().unwrap() }

    #[test]
    fn the_threshold_line_is_drawn_at_the_typed_level() {
        let svg = render_of(&[s("threshold", "-58.5")]);
        let ax = axes_of(&svg);
        let dashed: Vec<&str> = svg.split("<line ").filter(|l| l.contains("stroke-dasharray")).collect();
        assert_eq!(dashed.len(), 1);
        assert!((num(dashed[0], "y1") - ax.y(-58.5)).abs() < 0.02 && num(dashed[0], "y1") == num(dashed[0], "y2"));
        assert!(!render_of(&[b("show_threshold", false)]).contains("stroke-dasharray"));
    }

    // ------------------------------------------------------------------ the family sweep
    fn sets() -> Vec<Vec<(&'static str, Value)>> {
        let mk = |r: &str, t: &str, p: &str, u: &str, ts: &str, d: &str| vec![s("resting", r), s("threshold", t), s("peak", p), s("undershoot", u), s("stimulus", ts), s("duration", d)];
        vec![
            mk("-70", "-55", "40", "-80", "1", "8"),
            mk("-70", "-55", "-35", "-80", "1", "8"),
            mk("-70", "-60", "40", "-80", "1", "8"),
            mk("-70", "-55", "40", "-70", "1", "8"),
            mk("-90", "-60", "60", "-100", "2", "12"),
            mk("-70", "-55", "40", "-80", "0.5", "5.2"),
            mk("-70", "-55", "40", "-80", "1", "12"),
            mk("-70", "-55", "40", "-80", "7.3", "12"),
            mk("-85", "-65", "30", "-90", "1", "9"),
            mk("-20", "-5", "40", "-30", "1", "8"),
            mk("-70", "-55", "40", "-73", "3", "10"),
            mk("-100", "10", "70", "-100", "0.5", "12"),
            mk("-45", "-33", "70", "-60", "4", "12"),
        ]
    }
    fn switches(mask: u32, fail: bool, key: bool, size: &str) -> Vec<(&'static str, Value)> {
        vec![
            b("show_threshold", mask & 1 != 0), b("show_stimulus", mask & 2 != 0), b("show_phases", mask & 4 != 0),
            b("show_channels", mask & 8 != 0), b("show_refractory", mask & 16 != 0), b("fail", fail), b(SHOW_ANSWERS, key), s("label_size", size),
        ]
    }

    /// How many pointers of each kind a valid figure draws, worked out here from the switches alone.
    fn expected_pointers(mask: u32, fail: bool, dips: bool) -> (usize, usize) {
        let (thr, stim, ph, ch, rf) = (mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0, mask & 16 != 0);
        let numbered = if !ph { 0 } else if fail { 3 + usize::from(stim) + usize::from(thr) } else { 1 + usize::from(stim) + usize::from(thr) + 3 + usize::from(dips) + 1 };
        let lettered = if fail { 0 } else { 2 * usize::from(ch) + usize::from(rf) * (1 + usize::from(dips)) };
        (numbered, lettered)
    }

    #[test]
    fn the_family_never_overlaps_and_never_leaves_the_canvas() {
        let mut checked = 0;
        let mut refused = 0;
        for set in sets() {
            for size in ["Small", "Medium", "Large"] {
                for mask in 0..32u32 {
                    for fail in [false, true] {
                        for key in [false, true] {
                            let mut list = set.clone();
                            list.extend(switches(mask, fail, key, size));
                            let p = with(&list);
                            let svg = (def().render)(&p);
                            let sp = read(&p);
                            if sp.is_err() { refused += 1; continue; }
                            let sp = sp.unwrap();
                            assert!(!joined(&svg).contains("could not be drawn"), "a valid trace was refused: {set:?} {mask} {fail} {size}: {}", joined(&svg));
                            let bad = audit(&svg);
                            assert!(bad.is_empty(), "{set:?} mask {mask} fail {fail} key {key} {size}: {:?}", &bad[..bad.len().min(4)]);
                            // nothing is silently dropped: the discs are the pointers, drawn once on the figure and once in the key
                            let (n, l) = expected_pointers(mask, fail, sp.dips());
                            let big = circles(&svg).iter().filter(|c| c.2 >= 6.0).count();
                            assert_eq!(big, 2 * (n + l), "{set:?} mask {mask} fail {fail}: discs");
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked >= 4992, "{checked} checked, {refused} refused");
        assert_eq!(refused, 0, "every set in the sweep is a valid trace");
    }

    #[test]
    fn placement_never_fails_for_any_valid_trace() {
        // a wide grid of valid traces: if the values read, the figure is drawn
        let mut n = 0;
        for rest in [-100.0f64, -80.0, -70.0, -50.0, -30.0, -10.0] {
            for dthr in [10.0, 15.0, 30.0] {
                for dpk in [20.0, 40.0, 60.0] {
                    for dip in [0.0, 3.0, 10.0, 25.0] {
                        let (thr, peak, under) = (rest + dthr, rest + dthr + dpk, rest - dip);
                        if thr.abs() < 4.0 || peak > 70.0 || under < -100.0 { continue; }
                        for (ts, dur) in [(0.5, 5.2), (1.0, 8.0), (4.0, 12.0)] {
                            for size in ["Small", "Large"] {
                                for mask in [31u32, 0b10111, 0b01110, 0b11111] {
                                    let list = vec![s("resting", &rest.to_string()), s("threshold", &thr.to_string()), s("peak", &peak.to_string()), s("undershoot", &under.to_string()), s("stimulus", &ts.to_string()), s("duration", &dur.to_string())];
                                    let mut l = list.clone();
                                    l.extend(switches(mask, false, true, size));
                                    let p = with(&l);
                                    if read(&p).is_err() { continue; }
                                    if let Err(e) = plan(&p) { panic!("{l:?}: {e}"); }
                                    let bad = audit(&(def().render)(&p));
                                    assert!(bad.is_empty(), "{l:?}: {:?}", &bad[..bad.len().min(3)]);
                                    n += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(n > 1500, "{n}");
    }

    // ------------------------------------------------------------------ the exercise
    const NAMES: [&str; 14] = [
        "resting potential", "threshold", "depolarisation", "peak", "repolarisation", "hyperpolarisation", "undershoot", "stimulus",
        "channels open", "refractory", "na+", "k+", "local", "return to",
    ];

    #[test]
    fn a_student_copy_carries_no_name_and_no_typed_number() {
        let typed = [s("resting", "-73.5"), s("threshold", "-58.2"), s("peak", "37.5"), s("undershoot", "-81.3")];
        for set in [typed.to_vec(), vec![]] {
            for size in ["Small", "Large"] {
                for mask in 0..32u32 {
                    for fail in [false, true] {
                        let mut list = set.clone();
                        list.extend(switches(mask, fail, false, size));
                        let svg = (def().render)(&with(&list));
                        let low = svg.to_lowercase();
                        for n in NAMES { assert!(!low.contains(n), "student copy mentions {n:?}: {set:?} {mask} {fail}"); }
                        if !set.is_empty() {
                            let said = joined(&svg) + svg.split("<desc>").nth(1).unwrap_or("").split("</desc>").next().unwrap();
                            for n in ["73.5", "58.2", "37.5", "81.3", "73,5", "58,2"] { assert!(!said.contains(n), "student copy prints {n}: {said}"); }
                        }
                        // the key has a blank line for every pointer and not one name
                        let mut with_key = list.clone();
                        with_key.push(b(SHOW_ANSWERS, true));
                        let keyed = (def().render)(&with(&with_key));
                        let (n, l) = expected_pointers(mask, fail, true);
                        if n + l > 0 {
                            let key_text = joined(&keyed).to_lowercase();
                            assert!(key_text.contains("stimulus") || mask & 2 == 0 || mask & 4 == 0, "{mask}");
                            assert!(lines(&svg).len() > lines(&keyed).len(), "a student key adds blank lines: {} vs {}", lines(&svg).len(), lines(&keyed).len());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_key_names_every_pointer_and_the_two_copies_differ_only_in_the_key() {
        let on = render_of(&[b(SHOW_ANSWERS, true), b("show_refractory", true)]);
        let off = render_of(&[b(SHOW_ANSWERS, false), b("show_refractory", true)]);
        let j = joined(&on);
        for n in ["Stimulus", "Resting potential", "Threshold", "Depolarisation", "Peak", "Repolarisation", "Hyperpolarisation (undershoot)", "Return to resting potential", "Na+ channels open", "K+ channels open", "Absolute refractory period", "Relative refractory period"] {
            assert!(j.contains(n), "{n} missing from {j}");
        }
        assert_eq!(canvas(&on), canvas(&off), "same size with and without the key");
        assert_eq!(paths(&on), paths(&off), "the curve and the brackets are the same");
        assert_eq!(circles(&on), circles(&off));
        let nums = |svg: &str| texts(svg).into_iter().filter(|t| t.s.len() <= 2 || t.s.chars().all(|c| c.is_ascii_digit() || c == '\u{2212}' || c == '.')).map(|t| (t.s, t.x.to_bits(), t.y.to_bits())).collect::<Vec<_>>();
        assert_eq!(nums(&on), nums(&off), "numbers, marks and ticks are identical");
        // the discs are numbered 1.. in order and lettered A..
        let ds = circles(&on);
        let marks: Vec<String> = texts(&on).into_iter().filter(|t| ds.iter().any(|d| d.2 > 6.0 && ((t.x - d.0).powi(2) + (t.y - d.1).powi(2)).sqrt() < d.2)).map(|t| t.s).collect();
        assert!(marks.iter().any(|m| m == "1") && marks.iter().any(|m| m == "D"), "{marks:?}");
    }

    #[test]
    fn no_typed_number_is_ever_printed_with_the_answers_on_either() {
        let svg = render_of(&[s("resting", "-73.5"), s("threshold", "-58.2"), s("peak", "37.5"), s("undershoot", "-81.3"), b(SHOW_ANSWERS, true)]);
        let said = joined(&svg) + svg.split("<desc>").nth(1).unwrap_or("").split("</desc>").next().unwrap();
        for n in ["73.5", "58.2", "37.5", "81.3"] { assert!(!said.contains(n), "{n}"); }
    }

    // ------------------------------------------------------------------ switches
    #[test]
    fn each_switch_draws_or_removes_exactly_its_own_part() {
        let base = render_of(&[]);
        let j = |svg: &str| joined(svg);
        // the default has the dashed line, the stimulus arrow with its disc, phases and the channel brackets
        assert!(base.contains("stroke-dasharray"));
        assert_eq!(base.matches("<polygon").count(), 1, "one arrowhead");
        assert_eq!(paths(&base).1.len(), 6, "two channel brackets of three strokes each");
        let no_thr = render_of(&[b("show_threshold", false)]);
        assert!(!no_thr.contains("stroke-dasharray") && j(&no_thr).len() < j(&base).len());
        assert_eq!(no_thr.matches("<polygon").count(), 1);
        let no_stim = render_of(&[b("show_stimulus", false)]);
        assert_eq!(no_stim.matches("<polygon").count(), 0);
        let no_ph = render_of(&[b("show_phases", false)]);
        assert_eq!(no_ph.matches("<circle").count(), 2 * 2, "only the two lettered discs, on the figure and in the key");
        let no_ch = render_of(&[b("show_channels", false)]);
        assert!(paths(&no_ch).1.is_empty());
        let rf = render_of(&[b("show_refractory", true)]);
        assert_eq!(paths(&rf).1.len(), 12);
        // a lane is added for the refractory bands, and the figure grows to hold it
        assert!(canvas(&rf).1 > canvas(&base).1);
        let bare = render_of(&[b("show_phases", false), b("show_channels", false), b("show_stimulus", false), b("show_threshold", false)]);
        assert_eq!(bare.matches("<circle").count(), 0);
        assert!(!joined(&bare).contains("Hyperpolarisation"));
    }

    #[test]
    fn failed_initiation_draws_a_bump_and_no_spike_bands_and_says_so_only_when_it_drops_something() {
        let f = |extra: &[(&'static str, Value)]| { let mut l = vec![b("fail", true)]; l.extend(extra.iter().cloned()); l };
        let svg = render_of(&f(&[]));
        assert!(paths(&svg).1.is_empty(), "no channel brackets in a failure");
        // the default switches have the channel bands on, so the note applies and is on the canvas
        assert!(joined(&svg).contains("too weak to start a spike"));
        // with both band switches off nothing was dropped, so nothing is said
        let quiet = render_of(&f(&[b("show_channels", false)]));
        assert!(!joined(&quiet).contains("too weak"));
        // and the ordinary spike never carries the note
        assert!(!joined(&render_of(&[])).contains("too weak"));
        // the bump is below the threshold line and returns to rest
        let ax = axes_of(&svg);
        let (curve, _) = paths(&svg);
        let top = curve.iter().map(|p| p.1).fold(f64::MAX, f64::min);
        assert!(top > ax.y(-55.0) + 5.0 && top < ax.y(-70.0) - 5.0, "{top}");
        assert!((curve.last().unwrap().1 - ax.y(-70.0)).abs() < 0.2);
        // the key lists the failure's own three phases
        let k = joined(&render_of(&f(&[b(SHOW_ANSWERS, true)])));
        assert!(k.contains("Local (graded) response") && !k.contains("Depolarisation") && !k.contains("Peak"));
    }

    #[test]
    fn no_undershoot_is_said_once_and_only_when_something_is_left_out() {
        let none = |extra: &[(&'static str, Value)]| { let mut l = vec![s("undershoot", "-70")]; l.extend(extra.iter().cloned()); l };
        // phase pointers on: the hyperpolarisation pointer is dropped, so the note applies
        let svg = render_of(&none(&[]));
        assert!(joined(&svg).contains("no dip below rest"));
        let t = texts(&svg).into_iter().filter(|t| t.s.contains("no dip below rest")).count();
        assert_eq!(t, 1);
        let k = joined(&render_of(&none(&[b(SHOW_ANSWERS, true)])));
        assert!(!k.contains("Hyperpolarisation") && !k.contains("Relative"));
        // nothing that depends on a dip is on: nothing to say
        let calm = render_of(&none(&[b("show_phases", false), b("show_refractory", false)]));
        assert!(!joined(&calm).contains("no dip"));
        // with a dip it is never said
        assert!(!joined(&render_of(&[b("show_refractory", true)])).contains("no dip"));
        // K+ ends at the end of the fall when there is no dip, and the one refractory period reaches the same time
        let rf = render_of(&none(&[b("show_refractory", true)]));
        assert_eq!(paths(&rf).1.len(), 6 + 3, "Na, K and the absolute period only");
    }

    #[test]
    fn the_channel_bands_sit_at_the_times_the_curve_says() {
        let svg = render_of(&[s("stimulus", "2"), s("duration", "10"), b("show_refractory", true)]);
        let ax = axes_of(&svg);
        let sp = sp(-70.0, -55.0, 40.0, -80.0, 2.0, 10.0, false);
        let (_, br) = paths(&svg);
        // each bracket is three strokes: left tick, bar, right tick
        let spans: Vec<(f64, f64)> = br.chunks(3).map(|c| (c[0].0 .0, c[2].0 .0)).collect();
        let want = [
            (sp.t_threshold(), sp.t_peak()),
            (sp.t_peak(), sp.t_low() + 1.0),
            (sp.t_threshold(), sp.t_back_to_rest()),
            (sp.t_back_to_rest(), sp.t_low() + 1.0),
        ];
        assert_eq!(spans.len(), 4);
        for ((a, c), (t0, t1)) in spans.iter().zip(want) {
            assert!((a - (ax.x(t0) + 1.5)).abs() < 0.02 && (c - (ax.x(t1) - 1.5)).abs() < 0.02, "{a} {c} vs {t0} {t1}");
            assert!(*a < *c);
        }
        // Na+ opens as the curve passes the threshold: the bracket starts where the curve is at the threshold level
        let (curve, _) = paths(&svg);
        let at = curve.iter().min_by(|p, q| (p.0 - spans[0].0 + 1.5).abs().partial_cmp(&(q.0 - spans[0].0 + 1.5).abs()).unwrap()).unwrap();
        assert!((at.1 - ax.y(-55.0)).abs() < 1.0, "{} vs {}", at.1, ax.y(-55.0));
        // and K+ starts at the peak
        let pk = curve.iter().min_by(|p, q| p.1.partial_cmp(&q.1).unwrap()).unwrap();
        assert!((pk.0 - (spans[1].0 - 1.5)).abs() < 0.8);
    }

    // ------------------------------------------------------------------ labels, sizes, print
    #[test]
    fn label_size_scales_the_text_and_the_discs() {
        let h = |size: &str| { let svg = render_of(&[s("label_size", size)]); let t = texts(&svg); (t.iter().map(|x| x.size).fold(0.0, f64::max), circles(&svg).iter().map(|c| c.2).fold(0.0, f64::max), canvas(&svg)) };
        let (sm, md, lg) = (h("Small"), h("Medium"), h("Large"));
        assert!(sm.0 < md.0 && md.0 < lg.0 && sm.1 < md.1 && md.1 < lg.1);
        assert!((lg.0 / md.0 - 1.4).abs() < 0.01 && (sm.0 / md.0 - 0.8).abs() < 0.01);
        // none of it ever shrinks below a readable 9 px at Small
        let svg = render_of(&[s("label_size", "Small")]);
        assert!(texts(&svg).iter().all(|t| t.size >= 9.0), "{:?}", texts(&svg).iter().map(|t| t.size).fold(99.0, f64::min));
    }

    #[test]
    fn print_is_grey_only_in_every_state() {
        for set in sets().into_iter().take(4) {
            for mask in [0u32, 31, 0b10101, 0b01010] {
                for fail in [false, true] {
                    let mut l = set.clone();
                    l.extend(switches(mask, fail, mask & 1 == 0, "Medium"));
                    l.push(s("style", "Print"));
                    let svg = (def().render)(&with(&l));
                    assert!(crate::figures::census::non_grey_paints_within(&svg, 0).is_empty(), "{:?}", crate::figures::census::non_grey_paints_within(&svg, 0));
                    assert!(!svg.contains("<path") || svg.contains("#000000") || svg.contains("#000\""), "the curve is black in Print");
                }
            }
        }
        // and Colour is not grey (the difference is real)
        assert!(!crate::figures::census::non_grey_paints_within(&render_of(&[]), 0).is_empty());
    }

    // ------------------------------------------------------------------ editor note and canvas agree
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
        fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T { &v[(self.next() % v.len() as u64) as usize] }
        fn f(&mut self, lo: f64, hi: f64) -> f64 { lo + (self.next() % 1_000_000) as f64 / 1_000_000.0 * (hi - lo) }
    }
    fn junk(r: &mut Rng, lo: f64, hi: f64, unit: &str) -> String {
        let v = r.f(lo, hi);
        let one_decimal = format!("{:.1}", v);
        let n = match r.next() % 24 {
            0 => String::new(), 1 => "NaN".into(), 2 => "inf".into(), 3 => "-inf".into(), 4 => "9e999".into(), 5 => "1e3".into(),
            6 => "abc".into(), 7 => "1,000".into(), 8 => "-0".into(), 9 => format!("{}", v * 1e12), 10 => format!("{}", -v * 1e-12),
            11 => one_decimal.replace('.', ","), 12 => one_decimal.replace('-', "\u{2212}"), 13 => format!("\u{feff}{one_decimal}"),
            14 => format!(" \t{one_decimal} {unit}\r\n"), 15 => format!("{one_decimal}{unit}"), 16 => format!("{:.0}", v), 17 => format!("{:.3}", v),
            18 => "- 5".into(), 19 => format!("{}{}", one_decimal, " X"), 20 => "٣".into(), 21 => format!("{one_decimal}\r\n{one_decimal}"), _ => one_decimal,
        };
        n
    }

    /// What a canvas says in words: its text, with line breaks and wrapping flattened.
    fn flat(svg: &str) -> String { joined(svg).split_whitespace().collect::<Vec<_>>().join(" ") }

    #[test]
    fn the_editor_note_says_exactly_what_the_canvas_shows_over_ten_thousand_inputs() {
        let mut r = Rng(0x9e3779b97f4a7c15);
        let (mut refused, mut figures, mut noted) = (0, 0, 0);
        for i in 0..10_000 {
            let mut l = vec![
                s("resting", &junk(&mut r, -105.0, -5.0, "mV")),
                s("threshold", &junk(&mut r, -80.0, 20.0, "mV")),
                s("peak", &junk(&mut r, -20.0, 80.0, "mV")),
                s("undershoot", &junk(&mut r, -110.0, -30.0, "mV")),
                s("stimulus", &junk(&mut r, 0.0, 12.0, "ms")),
                s("duration", &junk(&mut r, 3.0, 24.0, "ms")),
                b("fail", r.next() % 4 == 0), b("show_threshold", r.next() % 2 == 0), b("show_stimulus", r.next() % 2 == 0),
                b("show_phases", r.next() % 2 == 0), b("show_channels", r.next() % 2 == 0), b("show_refractory", r.next() % 2 == 0),
                b(SHOW_ANSWERS, r.next() % 2 == 0), s("label_size", r.pick(&["Small", "Medium", "Large"])), s("style", r.pick(&["Colour", "Print"])),
            ];
            // a third of the inputs are made valid so the figure side is exercised as hard as the refusal side
            if i % 3 == 0 {
                l[0] = s("resting", &format!("{}", r.f(-95.0, -40.0).round()));
                l[1] = s("threshold", &format!("{}", (r.f(-35.0, 10.0)).round()));
                l[2] = s("peak", &format!("{}", (r.f(35.0, 65.0)).round()));
                l[3] = s("undershoot", &format!("{}", (r.f(-100.0, -75.0)).round()));
                l[4] = s("stimulus", &format!("{:.1}", r.f(0.5, 5.0)));
                l[5] = s("duration", &format!("{:.1}", r.f(10.0, 12.0)));
            }
            let p = with(&l);
            let svg = (def().render)(&p);
            let notes = notes_for(ID, &p);
            let words = flat(&svg);
            let is_refusal = canvas(&svg).0 == 420.0 && words.starts_with("The action potential could not be drawn:");
            match read(&p) {
                Err(e) => {
                    refused += 1;
                    assert!(is_refusal, "not a refusal canvas: {l:?} {words}");
                    assert_eq!(notes.len(), 1, "notes: {l:?} {notes:?}");
                    assert_eq!(notes[0].split_whitespace().collect::<Vec<_>>().join(" "), format!("The action potential could not be drawn: {} {}", e.why, e.then).split_whitespace().collect::<Vec<_>>().join(" "), "note text {l:?}");
                    // the canvas is the note, word for word
                    assert_eq!(words, notes[0].split_whitespace().collect::<Vec<_>>().join(" "), "canvas vs note: {l:?}");
                }
                Ok(_) => match plan(&p) {
                    Err(e) => {
                        refused += 1;
                        assert!(is_refusal, "{l:?}");
                        assert_eq!(words, notes[0].split_whitespace().collect::<Vec<_>>().join(" "), "{l:?}");
                        assert!(e.why.contains("no clear place") || e.why.contains("too close") || e.why.contains("would touch"), "{}", e.why);
                    }
                    Ok(pl) => {
                        figures += 1;
                        assert!(!is_refusal, "{l:?}");
                        assert_eq!(notes, pl.notes, "{l:?}");
                        // the caption on the canvas is the note: the last texts, in order
                        let t = texts(&svg);
                        let n = pl.cap.len();
                        let tail: Vec<String> = t[t.len() - n..].iter().map(|x| x.s.clone()).collect();
                        assert_eq!(tail.join(" "), notes.join(" ").split_whitespace().collect::<Vec<_>>().join(" "), "{l:?}");
                        if n == 0 { for known in [NOTE_NO_DIP, NOTE_FAILED] { assert!(!words.contains(known), "{l:?}"); } } else { noted += 1; }
                        // exactly the cases the module says: a dip-less spike with a dependent item, or a failure with band switches on
                        let sp = read(&p).unwrap();
                        let should = if sp.fail { p.b("show_channels") || p.b("show_refractory") } else { !sp.dips() && (p.b("show_refractory") || p.b("show_phases")) };
                        assert_eq!(!notes.is_empty(), should, "{l:?}");
                    }
                },
            }
            // nothing in a render is not a number
            if !is_refusal { assert!(!svg.contains("NaN") && !svg.contains("inf\"") && !svg.contains("-inf"), "{l:?}"); }
        }
        assert!(refused > 2000 && figures > 2000 && noted > 100, "{refused} refused, {figures} drawn, {noted} with a note");
    }

    #[test]
    fn hostile_text_never_panics_and_is_either_drawn_or_refused_with_one_note() {
        let long = "9".repeat(600);
        let wild = [
            "\u{0}", "\u{202e}-70", "-70\u{0}", "<script>", "&amp;", "\"'", long.as_str(), "\u{2212}", "--", "+-5", "5+", "5 ,5", "\u{0663}\u{0663}", "\u{ff11}\u{ff12}",
            "70 volts", "\u{130}mv", "5 \u{130}ms", "\u{212a}", "5\u{212a}mv", "\u{1f600}", "-70 m\u{0307}V", "ms", "mV mV", "5 mvmv",
        ];
        for a in wild {
            for fld in ["resting", "threshold", "peak", "undershoot", "stimulus", "duration"] {
                let p = with(&[s(fld, a)]);
                let svg = (def().render)(&p);
                assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && !svg.contains("NaN"), "{fld}={a:?}");
                let notes = notes_for(ID, &p);
                match read(&p) {
                    Err(_) => assert_eq!(notes.len(), 1, "{fld}={a:?}"),
                    Ok(_) => assert_eq!(notes, plan(&p).map(|x| x.notes).unwrap_or_default(), "{fld}={a:?}"),
                }
            }
        }
    }

    // ------------------------------------------------------------------ the rest of the contract
    #[test]
    fn the_defaults_draw_and_every_example_draws_without_a_refusal() {
        let p = with(&[]);
        assert!(warnings(&p).is_empty());
        let svg = (def().render)(&p);
        assert!(!joined(&svg).contains("could not be drawn"));
        assert!(audit(&svg).is_empty());
        let ex = examples(ID).unwrap();
        assert!(ex.len() >= 3);
        for e in ex {
            let svg = render_of(&e.params);
            assert!(!joined(&svg).contains("could not be drawn"), "{}", e.name);
            let bad = audit(&svg);
            assert!(bad.is_empty(), "{}: {bad:?}", e.name);
        }
        assert!(examples("food-web").is_none());
    }

    #[test]
    fn the_description_names_nothing_in_a_student_copy() {
        let desc = |key: bool| { let svg = def().svg(&with(&[b(SHOW_ANSWERS, key)])); svg.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap().to_string() };
        assert!(desc(true).contains("Depolarisation"));
        assert!(!desc(false).to_lowercase().contains("depolar") && desc(false).contains("pointers to name"));
        assert!(def().svg(&with(&[])).contains("role=\"img\""));
    }

    #[test]
    fn reading_the_same_values_typed_differently_gives_the_same_picture() {
        let a = render_of(&[s("resting", "-70"), s("threshold", "-55"), s("stimulus", "1.5")]);
        let b2 = render_of(&[s("resting", "\u{2212}70 mV"), s("threshold", "\u{feff} -55,0mv"), s("stimulus", "1,5 ms")]);
        assert_eq!(a, b2);
    }

    /// Writes the cases, and a copy reduced to 66% grey (the print check), to $VERSHION_PNG_DIR.
    #[test]
    fn action_potential_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]),
            ("student".into(), vec![b(SHOW_ANSWERS, false)]),
            ("student-all".into(), vec![b(SHOW_ANSWERS, false), b("show_refractory", true)]),
            ("all".into(), vec![b("show_refractory", true)]),
            ("failed".into(), vec![b("fail", true)]),
            ("failed-student".into(), vec![b("fail", true), b(SHOW_ANSWERS, false)]),
            ("print".into(), vec![s("style", "Print"), b("show_refractory", true)]),
            ("print-student".into(), vec![s("style", "Print"), b("show_refractory", true), b(SHOW_ANSWERS, false)]),
            ("nothing".into(), vec![b("show_phases", false), b("show_channels", false), b("show_stimulus", false), b("show_threshold", false)]),
            ("bare-curve-phases".into(), vec![b("show_channels", false)]),
            ("no-dip".into(), vec![s("undershoot", "-70"), b("show_refractory", true)]),
            ("tight-peak".into(), vec![s("peak", "-35"), b("show_refractory", true)]),
            ("tight-threshold".into(), vec![s("threshold", "-60"), b("show_refractory", true)]),
            ("short".into(), vec![s("stimulus", "0.5"), s("duration", "5.2"), b("show_refractory", true)]),
            ("long".into(), vec![s("duration", "12"), b("show_refractory", true)]),
            ("late".into(), vec![s("stimulus", "7.3"), s("duration", "12"), b("show_refractory", true)]),
            ("deep".into(), vec![s("resting", "-90"), s("threshold", "-60"), s("peak", "60"), s("undershoot", "-100"), b("show_refractory", true)]),
            ("refused-short".into(), vec![s("duration", "4")]),
            ("refused-number".into(), vec![s("peak", "forty"), s("label_size", "Large")]),
            ("refused-threshold".into(), vec![s("threshold", "-66"), s("label_size", "Small")]),
        ];
        for size in ["Small", "Medium", "Large"] {
            cases.push((format!("size-{size}"), vec![s("label_size", size), b("show_refractory", true)]));
            cases.push((format!("size-{size}-student"), vec![s("label_size", size), b("show_refractory", true), b(SHOW_ANSWERS, false)]));
            cases.push((format!("size-{size}-failed"), vec![s("label_size", size), b("fail", true)]));
        }
        for (idx, e) in examples(ID).unwrap().into_iter().enumerate() { cases.push((format!("ex{idx}"), e.params)); }
        for (tag, list) in cases {
            let svg = render_of(&list);
            std::fs::write(format!("{dir}/ap-{tag}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
            for (scale, name) in [(1.0f32, "1x"), (1.3, "1.3x")] {
                let (w, h, px) = to_rgba(&svg, scale).unwrap();
                let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/ap-{tag}-{name}-bw66.png")).unwrap();
            }
        }
    }
}
