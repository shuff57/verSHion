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
//! takes the first that is clear of the curve (on a 2 px copy of it), the guide lines, the other discs, the stimulus
//! arrow and the plot edge, with its leader kept off the curve, the guide lines and the arrow. "Resting potential" points at
//! the flat line (before the stimulus if there is room, otherwise after the spike has settled); "return to resting
//! potential" points at the recovering limb, inside the K+ and relative-refractory brackets and a literal 4 px off the
//! rest line (in a failed initiation, at the tail of the bump). If none is clear the figure is refused with that
//! reason rather than drawn crowded. Anything left out of the picture, and why, is said only in the editor note.

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
/// The membrane counts as back at rest when it is within this many mV of the resting level after the undershoot.
const REST_TOL: f64 = 1.0;
/// Failed initiation: how long the bump takes to peak, and how far up to threshold it climbs.
const T_BUMP: f64 = 0.7;
const BUMP_FRAC: f64 = 0.6;

// ---- what is accepted ----
pub const MV_MIN: f64 = -100.0;
pub const MV_MAX: f64 = 70.0;
/// The threshold is below 0 mV, at least `MIN_THR_TO_ZERO` of it, and at least `MIN_REST_TO_THR` above rest: so the
/// resting potential cannot be above their difference.
pub const REST_MAX: f64 = -(MIN_REST_TO_THR + MIN_THR_TO_ZERO);
pub const MAX_DUR: f64 = 12.0;
pub const MIN_LEAD: f64 = 0.5;
const MIN_REST_TO_THR: f64 = 10.0;
const MIN_THR_TO_PEAK: f64 = 20.0;
const MIN_DIP: f64 = 3.0;
/// The threshold must be this far below 0 mV (a neuron's threshold is a negative potential, and the dashed line must
/// not lie on the 0 mV line).
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
/// Radius of the dot a leader starts on, and the clear gaps (px between centre lines, so a stroke's own half-width is
/// already counted) kept around the leaders.
const DOT_R: f64 = 3.2;
const LEADER_FREE: f64 = 6.0;
const LEADER_CURVE_GAP: f64 = 3.4;
const LEADER_ARROW_GAP: f64 = 8.0;
const LEADER_DOT_GAP: f64 = 6.0;
/// The pointer to the resting potential is ALWAYS on the flat line before the stimulus, never after the spike, and the
/// plot is never widened for it. So the stretch before the stimulus must hold its dot: at least `DOT_FROM_AXIS` px from
/// the y axis (or a disc radius and 4 px, whichever is more, so the disc above it is clear of the axis) and, when the
/// stimulus arrow is drawn, `LEADER_ARROW_GAP` px from the arrow (4 px from the rise when it is not). That is a lead of
/// 20.1, 21.6 or 25.4 px at Small, Medium or Large; a stimulus that leaves less is refused with that reason.
const LEAD_NO_ARROW_GAP: f64 = 4.0;
const LEAD_SLACK: f64 = 0.05;
/// A pointer's dot is at least this far (px) from the y axis.
const DOT_FROM_AXIS: f64 = 12.0;
/// "Return to resting potential" points at the recovering limb, HALFWAY up the recovery: its dot is `RETURN_SHARE` of the
/// dip's depth above the lowest point (so half the depth below the rest line). A third of the way up would leave it
/// beside the lowest point on a shallow dip; two thirds would leave it hugging the rest line. Half also puts it about
/// 0.65 ms after the lowest point, so at least 28 px right of it on any plot. The dot is at least `RETURN_SEP_V` px from
/// the rest line and `RETURN_FROM_DOT` from every other dot.
const RETURN_SHARE: f64 = 0.5;
const RETURN_SEP_V: f64 = 4.0;
const RETURN_FROM_DOT: f64 = 24.0;
/// In a failed initiation the return dot is at least this far above the rest line (so it is not on the flat) and at
/// most this fraction of the bump's height (so it is not beside the top, "Local (graded) response").
const BUMP_RETURN_MIN_PX: f64 = 2.5;
const BUMP_RETURN_MAX_FRAC: f64 = 0.45;
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
        ParamDef::text("resting", "Resting potential (mV)", "-70").hint("-100 to -14 mV. Type the number, with or without mV: -70, \u{2212}70 and -70 mV all work"),
        ParamDef::text("threshold", "Threshold (mV)", "-55").hint("At least 10 mV above the resting potential, and -4 mV or lower: a threshold is below zero"),
        ParamDef::text("peak", "Peak (mV)", "40").hint("At least 20 mV above the threshold"),
        ParamDef::text("undershoot", "Lowest point of the undershoot (mV)", "-80").hint("Type the resting potential again for no undershoot; otherwise at least 3 mV below it"),
        ParamDef::text("stimulus", "Stimulus at (ms)", "1").hint("0.5 ms or later; a decimal comma is fine"),
        ParamDef::text("duration", "Total time (ms)", "8").hint("Room for the whole spike after the stimulus (4.7 ms), at most 12 ms"),
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

/// One unit of the exact scale below: a typed number is kept to a billionth.
const SCALE: i128 = 1_000_000_000;

/// A typed number, kept twice: as the nearest double (for drawing) and exactly, in billionths (for every comparison
/// and every message). A value typed exactly on a boundary, such as a threshold 10 mV above rest, is therefore on the
/// boundary, not a rounding error either side of it, and a message quotes the digits that were typed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Num {
    pub v: f64,
    d: i128,
}

/// A constant of the module (a limit written as a decimal) on the exact scale.
fn exact(x: f64) -> i128 { (x * SCALE as f64).round() as i128 }

/// An exact value as typed: a true minus, up to nine decimals, no trailing zeros.
fn show_d(d: i128) -> String {
    let (neg, a) = (d < 0, d.abs());
    let (int, frac) = (a / SCALE, a % SCALE);
    let mut t = int.to_string();
    if frac != 0 { t.push('.'); t.push_str(format!("{frac:09}").trim_end_matches('0')); }
    if neg && a != 0 { format!("\u{2212}{t}") } else { t }
}

/// A module constant as shown in a message.
fn show(x: f64) -> String { show_d(exact(x)) }

/// Reads one typed number: optional sign, digits with one decimal point or comma, and optionally the unit that
/// belongs to the control (`mV` or `ms`, any case). A comma followed by exactly three digits is ambiguous
/// (1,000: one thousand or one?) and is refused rather than guessed.
fn number(raw: &str, what: &str, units: &[&str], example: &str) -> Result<Num, Refusal> {
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
        if a.is_empty() || b.is_empty() { return Err(not_a_number()); }
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
    // the same digits, exactly: up to 12 whole digits and 9 decimals; more is refused, never rounded or clamped
    let (whole, frac) = digits.split_once('.').unwrap_or((digits.as_str(), ""));
    let (whole, frac) = (whole.trim_start_matches('0'), frac.trim_end_matches('0'));
    if whole.len() > 12 { return Err(bad(format!("{what}: \"{shown}\" is too large to read."))); }
    if frac.len() > 9 { return Err(bad(format!("{what}: \"{shown}\" has more than 9 decimal places. Type it with fewer."))); }
    let whole: i128 = whole.parse().unwrap_or(0);
    let nine = frac.chars().chain(std::iter::repeat('0')).take(9).fold(0i128, |acc, c| acc * 10 + c.to_digit(10).unwrap_or(0) as i128);
    let d = whole * SCALE + nine;
    Ok(Num { v: if neg { -v } else { v } + 0.0, d: if neg { -d } else { d } })
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

/// How to make a stimulus time and a total time agree: the one thing that is true at every boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fix {
    /// Only a longer total time helps (the stimulus is already as early as it may be): at least this long.
    Total(i128),
    /// Only an earlier stimulus helps (the total is as long as it may be, or the stimulus is too late for any total).
    Stimulus(i128),
    /// Either helps: a total at least the first, or a stimulus no later than the second.
    Either(i128, i128),
    /// Neither alone helps: both are out of reach.
    Both,
}

/// The advice for a stimulus at `ts` in a trace of `dur` (exact units), which is too short for the spike.
fn timing_fix(ts: i128, dur: i128) -> Fix {
    let (need, lead, max) = (exact(NEED), exact(MIN_LEAD), exact(MAX_DUR));
    let longer_works = ts + need <= max;
    let earlier_works = dur - need >= lead;
    match (longer_works, earlier_works) {
        (true, true) => Fix::Either(ts + need, dur - need),
        (true, false) => Fix::Total(ts + need),
        (false, true) => Fix::Stimulus(dur - need),
        (false, false) => Fix::Both,
    }
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
    let (lo, hi, rmax, tmax) = (exact(MV_MIN), exact(MV_MAX), exact(REST_MAX), exact(-MIN_THR_TO_ZERO));
    let range = |what: &str, x: Num, top: f64| {
        refuse(
            format!("{what} is {} mV, outside the {} to {} mV this figure draws.", show_d(x.d), show(MV_MIN), signed(top)),
            "Bring it inside that range; every neuron potential in a textbook is.",
        )
    };
    // each value inside its own range, in field order
    if rest.d >= 0 {
        return Err(refuse(
            format!("The resting potential is {} mV, but a neuron's inside is negative at rest: this figure draws it from {} to {} mV.", show_d(rest.d), show(MV_MIN), show(REST_MAX)),
            "Type a negative resting potential.",
        ));
    }
    if rest.d > rmax {
        return Err(refuse(
            format!("The resting potential is {} mV, above the {} mV limit: the threshold has to be at least {} mV above it and still {} mV below 0 mV, which leaves no room.", show_d(rest.d), show(REST_MAX), show(MIN_REST_TO_THR), show(MIN_THR_TO_ZERO)),
            &format!("Type a resting potential of {} mV or lower.", show(REST_MAX)),
        ));
    }
    if rest.d < lo { return Err(range("The resting potential", rest, REST_MAX)); }
    if thr.d >= 0 {
        return Err(refuse(
            format!("The threshold is {} mV, at or above 0 mV, but a neuron fires from a threshold below zero: the inside is still negative when the spike starts.", show_d(thr.d)),
            &format!("Type a threshold below zero, at most {} mV.", show(-MIN_THR_TO_ZERO)),
        ));
    }
    if thr.d > tmax {
        return Err(refuse(
            format!("The threshold ({} mV) is within {} mV of 0 mV, so the dashed threshold line would lie on the 0 mV line.", show_d(thr.d), show(MIN_THR_TO_ZERO)),
            &format!("Move the threshold to {} mV or lower.", show(-MIN_THR_TO_ZERO)),
        ));
    }
    if thr.d < lo { return Err(range("The threshold", thr, -MIN_THR_TO_ZERO)); }
    if peak.d < lo || peak.d > hi { return Err(range("The peak", peak, MV_MAX)); }
    if under.d < lo || under.d > hi { return Err(range("The undershoot", under, MV_MAX)); }
    // and then against each other
    if thr.d < rest.d + exact(MIN_REST_TO_THR) {
        return Err(refuse(
            format!("The threshold ({} mV) must be at least {} mV above the resting potential ({} mV): a neuron fires only when it is depolarised up to threshold, and closer than that the dashed line would run into the resting level.", show_d(thr.d), show(MIN_REST_TO_THR), show_d(rest.d)),
            "Raise the threshold or lower the resting potential.",
        ));
    }
    if peak.d < thr.d + exact(MIN_THR_TO_PEAK) {
        return Err(refuse(
            format!("The peak ({} mV) must be at least {} mV above the threshold ({} mV): a rise shorter than that is too small to label.", show_d(peak.d), show(MIN_THR_TO_PEAK), show_d(thr.d)),
            "Raise the peak or lower the threshold.",
        ));
    }
    if under.d > rest.d {
        return Err(refuse(
            format!("The undershoot ({} mV) is above the resting potential ({} mV), but the undershoot is the dip below rest.", show_d(under.d), show_d(rest.d)),
            "Type a lower value, or the resting potential itself for no undershoot.",
        ));
    }
    if under.d < rest.d && rest.d - under.d < exact(MIN_DIP) {
        return Err(refuse(
            format!("An undershoot only {} mV below rest ({} mV) is too shallow to see.", show_d(rest.d - under.d), show_d(rest.d)),
            "Type a value at least 3 mV below the resting potential, or the resting potential itself for no undershoot.",
        ));
    }
    if ts.d < exact(MIN_LEAD) {
        return Err(refuse(
            format!("The stimulus time ({} ms) is before {} ms, so the stimulus arrow would lie on the y axis and there would be no resting stretch before it.", show_d(ts.d), show(MIN_LEAD)),
            "Type 0.5 ms or later.",
        ));
    }
    if dur.d <= 0 {
        return Err(refuse(
            format!("A total time of {} ms is not a time: the trace has to run for more than 0 ms.", show_d(dur.d)),
            // the least total depends on the stimulus: it is the stimulus time plus the spike, which must still fit
            &if ts.d + exact(NEED) == exact(MAX_DUR) {
                format!("Type a total time of {} ms.", show(MAX_DUR))
            } else if ts.d + exact(NEED) < exact(MAX_DUR) {
                format!("Type a total time of {} to {} ms.", show_d(ts.d + exact(NEED)), show(MAX_DUR))
            } else {
                format!("Type a stimulus time of {} to {} ms first, then a total time of at least {} ms after it.", show(MIN_LEAD), show(MAX_DUR - NEED), show(NEED))
            },
        ));
    }
    if dur.d > exact(MAX_DUR) {
        return Err(refuse(
            format!("A total time of {} ms is over the {} ms the figure draws: the 0.8 ms upstroke would be under 35 px wide, too narrow to label.", show_d(dur.d), show(MAX_DUR)),
            "Type a shorter time.",
        ));
    }
    if ts.d + exact(NEED) > dur.d {
        let left = dur.d - ts.d;
        let leaves = if left > 0 { format!("leaves only {} ms of the {} ms trace", show_d(left), show_d(dur.d)) } else { format!("is not before the end of the {} ms trace", show_d(dur.d)) };
        let then = match timing_fix(ts.d, dur.d) {
            Fix::Total(t) => format!("Make the total time at least {} ms.", show_d(t)),
            Fix::Stimulus(s) => format!("Move the stimulus to {} ms or earlier; no total time up to {} ms is long enough for a stimulus this late.", show_d(s), show(MAX_DUR)),
            Fix::Either(t, s) => format!("Make the total time at least {} ms, or move the stimulus to {} ms or earlier.", show_d(t), show_d(s)),
            Fix::Both => format!("Type a stimulus time of {} to {} ms and a total time of at least {} ms after it.", show(MIN_LEAD), show(MAX_DUR - NEED), show(NEED)),
        };
        return Err(refuse(
            format!("The spike needs about {} ms after the stimulus to settle back to rest, but a stimulus at {} ms {leaves}.", show(NEED), show_d(ts.d)),
            &then,
        ));
    }
    Ok(Spike { rest: rest.v, thr: thr.v, peak: peak.v, under: under.v, ts: ts.v, dur: dur.v, fail: p.b("fail") })
}

/// A limit with its sign, `+70` or `−14`.
fn signed(x: f64) -> String { if x > 0.0 { format!("+{}", show(x)) } else { show(x) } }

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
    /// The time on the way back from the lowest point when the dip has shrunk to `remaining` (0..1) of its depth.
    pub fn t_recovered(&self, remaining: f64) -> f64 {
        let (mut a, mut b) = (0.0f64, 20.0f64);
        for _ in 0..80 {
            let m = (a + b) / 2.0;
            if (1.0 + m) * (-m).exp() > remaining { a = m } else { b = m }
        }
        self.t_low() + (a + b) / 2.0 / K_BACK
    }
    /// The recovery is over: the membrane is back within `REST_TOL` mV of the resting level (the end of the fall when
    /// there is no undershoot). The potassium channels have closed again and the relative refractory period has ended.
    pub fn t_settled(&self) -> f64 {
        if !self.dips() { return self.t_low(); }
        let (mut a, mut b) = (self.t_low(), self.t_low() + 20.0);
        for _ in 0..80 {
            let m = (a + b) / 2.0;
            if (self.v(m) - self.rest).abs() > REST_TOL { a = m } else { b = m }
        }
        (a + b) / 2.0
    }
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


use crate::figures::composite::seg_dist as dist_seg;

fn dist(a: P2, b: P2) -> f64 { ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() }

fn dist_poly(p: P2, pts: &[P2]) -> f64 {
    pts.windows(2).map(|s| dist_seg(p, s[0], s[1])).fold(f64::INFINITY, f64::min)
}

/// The least distance between two segments (zero when they cross).
fn segs_dist(a: P2, b: P2, c: P2, d: P2) -> f64 {
    // two segments that cross are 0 apart, whatever their ends do
    if seg_cross(a, b, c, d) { return 0.0; }
    dist_seg(a, c, d).min(dist_seg(b, c, d)).min(dist_seg(c, a, b)).min(dist_seg(d, a, b))
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
    /// Which of the shared dash styles tells it apart from the others in Print (solid, long dashes, dots, dash-dot).
    dash_slot: usize,
    /// The name of its lettered pointer.
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
    /// Unlabelled tick levels between the labelled ones (mV), and the same for time (ms).
    yminor: Vec<f64>,
    xminor: Vec<f64>,
    /// One short label per bracket row, set to the left of the plot.
    lane_labels: Vec<Vec<&'static str>>,
    xticks: Vec<(f64, String)>,
    show_thr: bool,
    thr_disc: bool,
    stim: Option<(f64, f64, f64)>,
    key_top: f64,
    key_rows: usize,
    key_col_w: f64,
    key_row_h: f64,
    /// What was left out and why. The editor shows it; the picture never does.
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

/// What the editor note says when something switched on is left out. Only the note says it: the picture, its
/// description and a student copy stay silent, because the sentence names the answer to a pointer.
const NOTE_NO_DIP: &str = "No undershoot (the lowest point is the resting level), so the items that belong to a dip are left out: the hyperpolarisation and return-to-rest pointers and the relative refractory period.";
const NOTE_FAILED: &str = "The stimulus is too weak to start a spike (failed initiation), so the bands that belong to a spike are not drawn: the channel and refractory bands.";

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
        // a raised plus, big and bold enough that a 66 % print copy cannot turn it into a minus or a dot
        out.push_str(&format!(r#"<tspan dy="{:.2}" font-size="{:.2}" font-weight="700">+</tspan>"#, -0.34 * size, 0.95 * size));
        out.push_str(&format!(r#"<tspan dy="{:.2}" font-size="{size}">{}</tspan>"#, 0.34 * size, svg::esc(tail)));
    }
    out.push_str("</text>");
    out
}

/// The minor step that divides a 1-2-5 major step into 5 (for 1 and 5) or 4 (for 2) parts.
fn minor_of(step: f64) -> f64 {
    let mag = 10f64.powf(step.log10().floor());
    if ((step / mag).round() - 2.0).abs() < 1e-9 { step / 4.0 } else { step / 5.0 }
}

/// The padded range, its labelled step and its minor step. The range ends on a minor tick, not a labelled one, so a
/// coarse labelled step (50 mV) cannot add up to 50 mV of empty axis at either end. Labelled ticks are at least
/// `min_gap` px apart.
fn y_range(sp: &Spike, need_lo_rest: f64, need_lo_low: f64, need_hi: f64, min_gap: f64) -> (f64, f64, f64, f64) {
    let low = sp.under.min(sp.rest);
    let mut pxmv = PLOT_H / (sp.peak - low + 40.0);
    let mut best = (0.0, 0.0, 1.0, 0.2);
    for _ in 0..40 {
        let lo_raw = (sp.rest - need_lo_rest / pxmv).min(low - need_lo_low / pxmv);
        let hi_raw = sp.peak + need_hi / pxmv;
        let mut target = 11.0;
        let (mut lo, mut hi, mut step, mut minor);
        loop {
            step = nice_step(hi_raw - lo_raw, target);
            minor = minor_of(step);
            lo = (lo_raw / minor + 1e-9).floor() * minor;
            hi = (hi_raw / minor - 1e-9).ceil() * minor;
            if PLOT_H / (hi - lo) * step >= min_gap || target <= 2.0 { break; }
            target -= 1.0;
        }
        best = (lo, hi, step, minor);
        let now = PLOT_H / (hi - lo);
        if now >= pxmv * (1.0 - 1e-9) { break; }
        pxmv = now;
    }
    best
}

/// Labelled ticks (value, label) and the unlabelled ones between them, for `[lo, hi]` with labelled step `step`.
fn ticks_in(lo: f64, hi: f64, step: f64, minor: f64) -> (Vec<(f64, String)>, Vec<f64>) {
    let per = (step / minor).round() as i64;
    let (j0, j1) = ((lo / minor - 1e-9).ceil() as i64, (hi / minor + 1e-9).floor() as i64);
    let (mut major, mut small) = (Vec::new(), Vec::new());
    for j in j0..=j1 {
        let v = j as f64 * minor;
        if j.rem_euclid(per) == 0 { major.push((j as f64 / per as f64 * step, fmt(j as f64 / per as f64 * step))); } else { small.push(v); }
    }
    (major, small)
}

fn plan(p: &Params) -> Result<Plan, Refusal> { plan_inner(p, true) }

/// `advise`: whether a refusal may search for advice by planning variations of the figure (and never recurses).
fn plan_inner(p: &Params, advise: bool) -> Result<Plan, Refusal> {
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
            // without a dip the curve never leaves the resting line after the fall, so a "return" would point at the same
            // flat stretch as "resting potential": the pair is dropped to the one name that has its own place on the curve
            if dips { numbered.push(NAMES_COMMON[6]); numbered.push(NAMES_COMMON[7]); }
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
    let (lo, hi, ystep, yminor_step) = y_range(&sp, need_lo_rest, need_lo_low, need_hi, (1.6 * fs).max(26.0));
    let (yticks, yminor) = ticks_in(lo, hi, ystep, yminor_step);
    let tick_w = yticks.iter().map(|t| text_w(&t.1, fs)).fold(0.0, f64::max);
    // the row labels of the bracket lanes sit left of the plot, right-aligned to it
    let mut lane_labels: Vec<Vec<&'static str>> = Vec::new();
    if channels { lane_labels.push(vec!["Channels"]); }
    // neutral for both brackets: it names neither of the two periods
    if refr { lane_labels.push(vec!["Periods"]); }
    let label_w = lane_labels.iter().flatten().map(|l| text_w(l, fs)).fold(0.0, f64::max);
    let ml = (8.0 + 1.15 * tfs + 8.0 + tick_w + 12.0).max(label_w + 6.0 + 8.0);
    let thr_disc = show_thr && phases;
    let mr_disc = if thr_disc { 14.0 + 2.0 * r + 10.0 } else { 14.0 };
    // the last tick label is centred on the right end of the axis
    let dur_label = fmt(sp.dur);
    let mr = mr_disc.max(text_w(&dur_label, fs) / 2.0 + 6.0);
    let plot_w = PLOT_W_MIN.max(key_w + 2.0 * MARGIN - ml - mr);
    // the stretch before the stimulus must hold the resting pointer (it never goes after the spike, and the plot is
    // never widened for it)
    let lead_px = sp.ts / sp.dur * plot_w;
    let lead_from_axis = DOT_FROM_AXIS.max(r + 4.0);
    let lead_gap = if show_stim { LEADER_ARROW_GAP } else { LEAD_NO_ARROW_GAP };
    let lead_need = lead_from_axis + lead_gap + LEAD_SLACK;
    if phases && lead_px < lead_need {
        return Err(lead_refusal(p, &sp, lead_px, lead_need, show_stim, advise));
    }
    let w = ml + plot_w + mr;

    // ---- horizontal ticks
    let mut target = 8.0;
    let (xticks, xminor) = loop {
        let step = nice_step(sp.dur, target);
        let (major, small) = ticks_in(0.0, sp.dur, step, minor_of(step));
        let wmax = major.iter().map(|t| text_w(&t.1, fs)).fold(0.0, f64::max);
        if step / sp.dur * plot_w >= wmax + 12.0 || target <= 2.0 { break (major, small); }
        target -= 1.0;
    };

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
    let h = key_end + 12.0;

    // ---- bands
    let mut bands = Vec::new();
    if channels {
        bands.push(Band { from: sp.t_threshold(), to: sp.t_peak(), lane: 0, dash_slot: 0, key: NAME_NA, colour_slot: 1 });
        // potassium channels close again as the membrane returns to rest
        bands.push(Band { from: sp.t_peak(), to: sp.t_settled(), lane: 0, dash_slot: 1, key: NAME_K, colour_slot: 2 });
    }
    if refr {
        let lane = usize::from(channels);
        bands.push(Band { from: sp.t_threshold(), to: sp.t_back_to_rest(), lane, dash_slot: 3, key: NAME_ABS, colour_slot: 3 });
        if rel { bands.push(Band { from: sp.t_back_to_rest(), to: sp.t_settled(), lane, dash_slot: 2, key: NAME_REL, colour_slot: 4 }); }
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
    let mut dots: Vec<P2> = Vec::new();
    let mut pointers: Vec<Pointer> = Vec::new();
    // a finer copy of the curve for the leaders, which are thin and may graze it
    let fine = trace(&sp, &f, 2.0);

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
        // the resting pointer's candidate places: the flat line before the stimulus, between the least distance from the
        // y axis and the arrow, from the middle outwards
        let (rest_lo, rest_hi) = (f.l + lead_from_axis, f.px(sp.ts) - lead_gap);
        let rest_times: Vec<f64> = [0.5, 0.3, 0.7, 0.15, 0.85, 0.0, 1.0].iter().map(|k| (rest_lo + (rest_hi - rest_lo) * k - f.l) / f.w * sp.dur).collect();
        let anchors = anchors_for(&sp, name, &rest_times);
        let dirs = dirs_for(name, sp.fail);
        let bump_px = (f.py(sp.rest) - f.py(sp.rest + BUMP_FRAC * (sp.thr - sp.rest))).abs();
        let lens = [r + 14.0 * lab, r + 24.0 * lab, r + 36.0 * lab, r + 50.0 * lab];
        let mut placed: Option<(P2, P2, P2)> = None;
        // did any point of the recovering limb have room for the return dot at all (4 px off the rest line, clear of the others)?
        let mut return_dev: Option<f64> = None;
        let mut dot_ok = false;
        'search: for strict in [true, false] {
            for &t in &anchors {
                let dot = (f.px(t), f.py(sp.v(t)));
                if dot.0 < f.l + DOT_FROM_AXIS || t >= sp.dur { continue; }
                if name == NAMES_COMMON[1] && dot.0 > f.px(sp.ts) { continue; }
                // where a pointer may point, measured in px: the resting pointer on the flat line; the return pointer on
                // the recovering limb, off the rest line and away from every other dot
                let dev = (dot.1 - f.py(sp.rest)).abs();
                if name == NAMES_COMMON[1] && dev > 1.0 { continue; }
                if name == NAMES_COMMON[7] {
                    return_dev = Some(dev);
                    let off = if sp.fail { dev >= BUMP_RETURN_MIN_PX && dev <= BUMP_RETURN_MAX_FRAC * bump_px } else { dev >= RETURN_SEP_V };
                    if !off || dots.iter().any(|q| dist(dot, *q) < RETURN_FROM_DOT) { continue; }
                }
                dot_ok = true;
                // a dot on a guide line is only allowed once nothing better exists (a peak at exactly 0 mV sits on the zero line)
                if strict && hlines.iter().any(|(a, b)| dist_seg(dot, *a, *b) < 6.0) { continue; }
                // the dot is a mark of its own: clear of every earlier leader and dot
                if leaders.iter().any(|(a, b)| dist_seg(dot, *a, *b) < LEADER_DOT_GAP) || dots.iter().any(|q| dist(dot, *q) < 2.0 * DOT_R + 3.0) { continue; }
                for &(dx, dy) in &dirs {
                    let n = (dx * dx + dy * dy).sqrt();
                    let (ux, uy) = (dx / n, dy / n);
                    for &len in &lens {
                        let c = (dot.0 + ux * len, dot.1 + uy * len);
                        if c.0 < f.l + r + 4.0 || c.0 > x1 - r - 4.0 || c.1 < f.t + r + 4.0 || c.1 > f.t + f.h - r - 4.0 { continue; }
                        if dist_poly(c, &fine) < r + 4.0 + 0.5 { continue; }
                        if hlines.iter().any(|(a, b)| dist_seg(c, *a, *b) < r + 3.0) { continue; }
                        if segs.iter().any(|(a, b)| dist_seg(c, *a, *b) < r + 4.0) { continue; }
                        if discs.iter().any(|(d, dr)| dist(c, *d) < r + dr + 4.0) { continue; }
                        // the disc keeps clear of the leaders and dots already placed
                        if leaders.iter().any(|(a, b)| dist_seg(c, *a, *b) < r + 3.0) || dots.iter().any(|q| dist(c, *q) < r + LEADER_DOT_GAP) { continue; }
                        let edge = (dot.0 + ux * (len - r), dot.1 + uy * (len - r));
                        if discs.iter().any(|(d, dr)| dist_seg(*d, dot, edge) < dr + 2.0) { continue; }
                        // the leader: not along the curve, not across the stimulus arrow, another leader or another dot
                        let mut ok = true;
                        let steps = ((len - r) / 2.0).ceil().max(1.0) as usize;
                        for st in 1..=steps {
                            let q = (dot.0 + ux * (len - r) * st as f64 / steps as f64, dot.1 + uy * (len - r) * st as f64 / steps as f64);
                            if dist(q, dot) > LEADER_FREE && dist_poly(q, &fine) < LEADER_CURVE_GAP { ok = false; break; }
                        }
                        if !ok { continue; }
                        if let Some((xs, y_top, y_tail)) = arrow {
                            if segs_dist(dot, edge, (xs, y_top), (xs, y_tail)) < LEADER_ARROW_GAP { continue; }
                        }
                        if strict && hlines.iter().any(|(a, b)| seg_cross(dot, edge, *a, *b)) { continue; }
                        placed = Some((c, dot, edge));
                        break 'search;
                    }
                }
            }
        }
        let Some((c, dot, edge)) = placed else {
            // the reason is the one that is true: no spot for the dot itself (a dip too shallow for the return dot to be
            // 4 px off the rest line halfway up the recovery), or a dot but no room for its disc and leader
            if name == NAMES_COMMON[1] {
                return Err(crowded_refusal(p, &sp, lead_px, advise));
            }
            if name == NAMES_COMMON[7] && !sp.fail && !dot_ok {
                if let Some(dev) = return_dev.filter(|d| *d < RETURN_SEP_V) {
                    return Err(shallow_refusal(p, &sp, dev, advise));
                }
            }
            return Err(refuse(
                format!("There is no clear place for the pointer to \"{name}\" with these values: it would touch the curve, a guide line or another pointer."),
                "Change the potentials or the times a little, or turn the phase pointers off.",
            ));
        };
        leaders.push((dot, edge));
        dots.push(dot);
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
        thr_disc, stim: arrow, key_top, key_rows, key_col_w, key_row_h, notes, title_y, yminor, xminor, lane_labels,
    })
}

/// The stimulus leaves too little plot before it for the resting pointer (`why` says which way: no dot fits, or the dot fits
/// and its disc and leader do not). The advice is found by trying later stimuli and shorter totals, to the tenth, so it is
/// the least change that draws the figure.
fn early_refusal(p: &Params, sp: &Spike, why: String, advise: bool) -> Refusal {
    let mut options: Vec<String> = Vec::new();
    if advise {
        let works = |ts: i128, dur: i128| {
            let mut q = p.clone();
            q.0.insert("stimulus".into(), Value::S(show_d(ts)));
            q.0.insert("duration".into(), Value::S(show_d(dur)));
            plan_inner(&q, false).is_ok()
        };
        let (ts0, dur0) = (exact(sp.ts), exact(sp.dur));
        let tenth = SCALE / 10;
        // a later stimulus: the least tenth of a millisecond, at the same total time, that draws
        let mut ts = (ts0 / tenth + 1) * tenth;
        while ts + exact(NEED) <= dur0 {
            if works(ts, dur0) { options.push(format!("move the stimulus to {} ms or later", show_d(ts))); break; }
            ts += tenth;
        }
        // a shorter total: the longest tenth below the one typed, with the same stimulus, that draws
        let mut dur = ((dur0 - 1) / tenth) * tenth;
        while dur >= ts0 + exact(NEED) {
            if works(ts0, dur) { options.push(format!("shorten the total time to {} ms or less", show_d(dur))); break; }
            dur -= tenth;
        }
    }
    options.push("turn the phase pointers off".to_string());
    let then = options.join(", or ");
    refuse(why, &format!("{}{}.", then[..1].to_uppercase(), &then[1..]))
}

/// The stimulus leaves too little plot before it for the resting pointer's dot.
fn lead_refusal(p: &Params, sp: &Spike, lead_px: f64, need: f64, show_stim: bool, advise: bool) -> Refusal {
    let floor1 = |x: f64| (x * 10.0).floor() / 10.0;
    let ceil1 = |x: f64| (x * 10.0 - 1e-9).ceil() / 10.0;
    let why = format!(
        "The stimulus at {} ms leaves only {} px of the plot before it in a {} ms trace, and the pointer to the resting potential, which always points at the flat line before the stimulus, needs at least {} px there ({}).",
        show(sp.ts), show(floor1(lead_px)), show(sp.dur), show(ceil1(need)),
        if show_stim { "its disc clear of the y axis and of the stimulus arrow" } else { "its disc clear of the y axis and of the rise" },
    );
    early_refusal(p, sp, why, advise)
}

/// The dot fits before the stimulus but its disc and leader do not, between the y axis, the arrow and the guide lines.
fn crowded_refusal(p: &Params, sp: &Spike, lead_px: f64, advise: bool) -> Refusal {
    let floor1 = |x: f64| (x * 10.0).floor() / 10.0;
    let why = format!(
        "The stimulus at {} ms leaves only {} px of the plot before it in a {} ms trace: room for the dot of the pointer to the resting potential, which always points at the flat line before the stimulus, but not for its disc and leader between the y axis, the stimulus arrow and the guide lines.",
        show(sp.ts), show(floor1(lead_px)), show(sp.dur),
    );
    early_refusal(p, sp, why, advise)
}

/// The dip is too shallow for the return pointer halfway up the recovery to be `RETURN_SEP_V` px off the rest line.
fn shallow_refusal(p: &Params, sp: &Spike, dev: f64, advise: bool) -> Refusal {
    let floor1 = |x: f64| (x * 10.0).floor() / 10.0;
    let why = format!(
        "The undershoot ({} mV below rest) is too shallow at this scale: the pointer to \"{}\" sits halfway up the recovery, which would be only {} px from the resting line, and it needs at least {} px.",
        show(sp.rest - sp.under), NAMES_COMMON[7], show(floor1(dev)), show(RETURN_SEP_V),
    );
    let mut options: Vec<String> = Vec::new();
    if advise {
        let ok = |q: &Params| plan_inner(q, false).is_ok();
        // a deeper undershoot: the least (to the tenth) that works, scanning down from the dip typed to the floor
        let mut d = ((sp.rest - sp.under) * 10.0).round() as i64 + 1;
        let mut found = None;
        while sp.rest - d as f64 / 10.0 >= MV_MIN - 1e-9 {
            let mut q = p.clone();
            q.0.insert("undershoot".into(), Value::S(show(sp.rest - d as f64 / 10.0)));
            if ok(&q) { found = Some(sp.rest - d as f64 / 10.0); break; }
            d += 1;
        }
        if let Some(u) = found { options.push(format!("type a deeper undershoot, {} mV", show(u))); }
        // a lower peak: the highest whole millivolt below the one typed that works, down to 20 mV above the threshold
        let mut pk = sp.peak.floor() - if sp.peak.fract() == 0.0 { 1.0 } else { 0.0 };
        while pk >= sp.thr + MIN_THR_TO_PEAK - 1e-9 {
            let mut q = p.clone();
            q.0.insert("peak".into(), Value::S(show(pk)));
            if ok(&q) { options.push(format!("lower the peak to {} mV", show(pk))); break; }
            pk -= 1.0;
        }
    }
    options.push("turn the phase pointers off".to_string());
    let then = options.join(", or ");
    refuse(why, &format!("{}{}.", then[..1].to_uppercase(), &then[1..]))
}

/// Where along the curve (times, in order of preference) the pointer to this phase may point. `rest_times` are the
/// places on the flat line before the stimulus where the resting pointer may point.
fn anchors_for(sp: &Spike, name: &str, rest_times: &[f64]) -> Vec<f64> {
    let (tp, tl) = (sp.t_peak(), sp.t_low());
    if sp.fail {
        let tb = sp.ts + T_BUMP;
        return match name {
            n if n == NAMES_COMMON[1] => rest_times.to_vec(),
            // the top of the bump, or its rising side when the top is crowded: never the falling side, which is where
            // the return pointer is
            n if n == NAME_LOCAL => vec![tb, tb - 0.12, tb - 0.25, tb - 0.35],
            // the tail of the bump, where it is back down near rest but not on the flat line: x is the time since the
            // stimulus in bump units, and the bump is then 44 % to 10 % of its height
            _ => [2.6, 2.4, 2.8, 3.0, 3.2, 2.2].iter().map(|x| sp.ts + T_BUMP * x).collect(),
        };
    }
    match name {
        n if n == NAMES_COMMON[1] => rest_times.to_vec(),
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
        // the recovering limb, where the dip has shrunk to half its depth: RETURN_SHARE of the way up from the lowest
        // point. One place, not a list: "visibly above the lowest point" is the rule.
        _ => vec![sp.t_recovered(1.0 - RETURN_SHARE)],
    }
}

fn dirs_for(name: &str, fail: bool) -> Vec<P2> {
    // screen directions: y grows downwards
    let (u, ul, ur, l, rr, dl, dr, d) = ((0.0, -1.0), (-1.0, -1.0), (1.0, -1.0), (-1.0, 0.0), (1.0, 0.0), (-1.0, 1.0), (1.0, 1.0), (0.0, 1.0));
    match name {
        n if n == NAMES_COMMON[3] => vec![l, ul, dl, u, d, ur, dr, rr],
        n if n == NAMES_COMMON[4] => vec![u, ul, ur, l, rr, dl, dr, d],
        n if n == NAMES_COMMON[5] => vec![rr, ur, dr, d, u, dl, ul, l],
        n if n == NAMES_COMMON[6] => vec![d, dl, dr, l, rr, ul, ur, u],
        n if n == NAMES_COMMON[1] => vec![u, ul, ur, d, dl, dr, l, rr],
        // below the recovering limb, toward the rest of the plot's lower margin
        n if n == NAMES_COMMON[7] && !fail => vec![dr, d, rr, ur, dl, l, ul, u],
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
    // nothing in the description says what was left out or why: that is the editor note's job
    let desc = if key && !names.is_empty() {
        format!("Membrane potential in mV against time in ms for a neuron. Pointers: {}.", names.join("; "))
    } else {
        format!("Membrane potential in mV against time in ms for a neuron. {} pointers to name.", pl.pointers.len())
    };
    d.describe("Action potential", &desc);
    d.defs(&fills.defs());

    // axes
    let (x0, x1, yb, yt) = (f.l, f.l + f.w, f.t + f.h, f.t);
    // light grid lines at every tick level, behind everything (graph paper): a level can be read across the plot
    {
        let (g_major, g_minor) = if print { ("#cfcfcf", "#e6e6e6") } else { ("#d3d7de", "#e9ebf0") };
        let mut g = String::from(r#"<g id="grid">"#);
        let mut put = |y: f64, col: &str, sw: f64| g.push_str(&format!(r#"<line x1="{x0:.2}" y1="{y:.2}" x2="{x1:.2}" y2="{y:.2}" stroke="{col}" stroke-width="{sw}"/>"#));
        for v in &pl.yminor { put(f.py(*v), g_minor, 0.8); }
        for (v, _) in &pl.yticks { put(f.py(*v), g_major, 1.0); }
        g.push_str("</g>");
        d.raw(&g);
    }
    if f.lo < 0.0 && 0.0 < f.hi { d.line(x0, f.py(0.0), x1, f.py(0.0), zero_col, 1.3); }
    d.line(x0, yb, x1, yb, INK, 1.8);
    d.line(x0, yt, x0, yb, INK, 1.8);
    for (v, label) in &pl.yticks {
        let y = f.py(*v);
        d.line(x0 - 6.0, y, x0, y, INK, 1.4);
        d.raw(&text_el(x0 - 10.0, y + fs * 0.35, fs, "end", INK, label));
    }
    for v in &pl.yminor { let y = f.py(*v); d.line(x0 - 3.5, y, x0, y, INK, 1.0); }
    for v in &pl.xminor { let x = f.px(*v); d.line(x, yb, x, yb + 3.5, INK, 1.0); }
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

    // bands: in Print the four brackets differ by their dash, as well as by their letter
    for b in &pl.bands {
        let y = pl.lane_top + b.lane as f64 * pl.lane_h + 12.0;
        let (a, c) = (f.px(b.from) + 3.0, f.px(b.to) - 3.0);
        let col = fills.line(b.colour_slot);
        let dash = fills.dash(b.dash_slot);
        let dash = if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) };
        d.raw(&format!(r#"<path d="M{a:.2},{:.2}V{y:.2}H{c:.2}V{:.2}" fill="none" stroke="{col}" stroke-width="2.2"{dash}/>"#, y - 6.0, y - 6.0));
    }
    // a short label for each row of brackets, so two rows read as two different questions
    for (i, lines) in pl.lane_labels.iter().enumerate() {
        let cy = pl.lane_top + i as f64 * pl.lane_h + 16.0 + r;
        let lh = 1.2 * fs;
        for (j, l) in lines.iter().enumerate() {
            let y = cy - (lines.len() as f64 - 1.0) * lh / 2.0 + j as f64 * lh + 0.35 * fs;
            d.raw(&text_el(x0 - 6.0, y, fs, "end", INK, l));
        }
    }
    // pointers
    for q in &pl.pointers {
        if let Some(dot) = q.dot {
            let dist = ((q.disc.0 - dot.0).powi(2) + (q.disc.1 - dot.1).powi(2)).sqrt();
            let k = 1.0 - r / dist;
            d.line(dot.0, dot.1, dot.0 + (q.disc.0 - dot.0) * k, dot.1 + (q.disc.1 - dot.1) * k, INK, 1.2);
            // a white ring round the dot, so it stands out on a black curve in Print
            d.circle(dot.0, dot.1, DOT_R, INK, "#ffffff", 1.4);
        }
        disc(&mut d, q.disc, r, &q.mark, dfs);
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
    /// The marks written inside the discs (1.. and A..), figure and key.
    fn disc_marks(svg: &str) -> Vec<String> {
        let ds: Vec<_> = circles(svg).into_iter().filter(|c| c.2 >= 6.0).collect();
        texts(svg).into_iter().filter(|t| ds.iter().any(|d| ((t.x - d.0).powi(2) + (t.y - d.1).powi(2)).sqrt() < d.2)).map(|t| t.s).collect()
    }
    fn joined(svg: &str) -> String { texts(svg).iter().map(|t| t.s.clone()).collect::<Vec<_>>().join("\n") }
    fn circles(svg: &str) -> Vec<(f64, f64, f64)> {
        svg.split("<circle ").skip(1).map(|c| { let c = c.split('>').next().unwrap(); (num(c, "cx"), num(c, "cy"), num(c, "r")) }).collect()
    }
    /// The picture without its grid group (light graph-paper lines under everything, which no collision rule covers).
    fn strip_grid(svg: &str) -> String {
        match svg.split_once("<g id=\"grid\">") {
            Some((a, rest)) => format!("{a}{}", rest.split_once("</g>").unwrap().1),
            None => svg.to_string(),
        }
    }
    fn grid_lines(svg: &str) -> Vec<(P, f64, String)> {
        let Some((_, rest)) = svg.split_once("<g id=\"grid\">") else { return vec![] };
        rest.split_once("</g>").unwrap().0.split("<line ").skip(1).map(|l| {
            let l = l.split('>').next().unwrap();
            ((num(l, "x1"), num(l, "x2")), num(l, "y1"), attr(l, "stroke").unwrap())
        }).collect()
    }
    /// Every line with its stroke width and colour.
    fn lines_w(svg: &str) -> Vec<((P, P), f64, String)> {
        strip_grid(svg).split("<line ").skip(1).map(|l| { let l = l.split('>').next().unwrap(); (((num(l, "x1"), num(l, "y1")), (num(l, "x2"), num(l, "y2"))), num(l, "stroke-width"), attr(l, "stroke").unwrap_or_default()) }).collect()
    }
    fn lines(svg: &str) -> Vec<(P, P)> { lines_w(svg).into_iter().map(|l| l.0).collect() }
    fn polys(svg: &str) -> Vec<Vec<P>> {
        strip_grid(svg).split("<polygon ").skip(1).map(|g| {
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
    fn dseg(p: P, a: P, b: P) -> f64 { crate::figures::composite::seg_dist(p, a, b) }
    /// Least distance between two segments, from my own crossing test.
    fn sseg(a: P, b: P, c: P, d: P) -> f64 {
        let o = |p: P, q: P, r: P| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
        let crossing = (o(c, d, a) > 0.0) != (o(c, d, b) > 0.0) && (o(a, b, c) > 0.0) != (o(a, b, d) > 0.0);
        if crossing { return 0.0; }
        dseg(a, c, d).min(dseg(b, c, d)).min(dseg(c, a, b)).min(dseg(d, a, b))
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
        bad.extend(leader_problems(svg));
        for q in &curve { if q.0 < 0.0 || q.0 > w || q.1 < 0.0 || q.1 > h { bad.push(format!("curve leaves the canvas at {q:?}")); break; } }
        for l in &ls { for q in [l.0, l.1] { if q.0 < -0.01 || q.0 > w + 0.01 || q.1 < -0.01 || q.1 > h + 0.01 { bad.push(format!("line leaves the canvas {l:?}")); } } }
        bad
    }

    /// The leaders (the 1.2 px lines from a dot on the curve to a disc), each measured against everything it could
    /// cross, with literal gaps: 1 px between strokes' edges. The module's own constants are not used.
    fn leader_problems(svg: &str) -> Vec<String> {
        let mut bad = Vec::new();
        let ls = lines_w(svg);
        let leaders: Vec<(P, P)> = ls.iter().filter(|l| l.1 == 1.2).map(|l| l.0).collect();
        let (curve, _) = paths(svg);
        let all = circles(svg);
        let discs: Vec<_> = all.iter().filter(|c| c.2 >= 6.0).collect();
        let dots: Vec<_> = all.iter().filter(|c| c.2 < 6.0).collect();
        let heads = polys(svg);
        for (i, &(a, b)) in leaders.iter().enumerate() {
            // along the curve: the curve is 2.6 wide, the leader 1.2, and they may meet only at the leader's own dot
            let n = (dist2(a, b) / 0.5).ceil().max(1.0) as usize;
            for k in 0..=n {
                let q = (a.0 + (b.0 - a.0) * k as f64 / n as f64, a.1 + (b.1 - a.1) * k as f64 / n as f64);
                if dist2(q, a) > 7.0 && dpoly(q, &curve) < 1.3 + 0.6 + 1.0 { bad.push(format!("a leader touches the curve away from its dot: {a:?} -> {b:?} at {q:?}")); break; }
            }
            for &&(cx, cy, r) in &discs {
                let own = (dist2((cx, cy), b) - r).abs() < 0.6;
                if !own && dseg((cx, cy), a, b) < r + 0.6 + 1.0 { bad.push(format!("a leader {a:?} -> {b:?} touches another disc {cx},{cy}")); }
            }
            for &&(cx, cy, r) in &dots {
                if dist2((cx, cy), a) > 0.05 && dseg((cx, cy), a, b) < r + 0.7 + 0.6 + 1.0 { bad.push(format!("a leader {a:?} -> {b:?} touches another leader's dot {cx},{cy}")); }
            }
            for &(c, d) in leaders.iter().skip(i + 1) {
                if sseg(a, b, c, d) < 1.2 + 1.0 { bad.push(format!("two leaders touch: {a:?}->{b:?} / {c:?}->{d:?}")); }
            }
            // the stimulus arrow: its shaft is the vertical 1.8 line under the head's tip, the head is the polygon
            for g in &heads {
                let tip = g[0];
                let shaft: Vec<&(P, P)> = ls.iter().filter(|l| l.1 == 1.8 && l.0 .0 .0 == l.0 .1 .0 && (l.0 .0 .0 - tip.0).abs() < 0.02).map(|l| &l.0).collect();
                for sh in shaft { if sseg(a, b, sh.0, sh.1) < 0.9 + 0.6 + 1.0 { bad.push(format!("a leader {a:?} -> {b:?} crosses or touches the stimulus arrow")); } }
                for k in 0..3 {
                    if sseg(a, b, g[k], g[(k + 1) % 3]) < 0.6 + 1.0 { bad.push(format!("a leader {a:?} -> {b:?} touches the arrowhead")); }
                }
            }
        }
        bad
    }
    fn dist2(a: P, b: P) -> f64 { ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() }

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
        let mv = |t: &str| number(t, "Peak", &["millivolts", "millivolt", "mv"], "-70").map(|n| n.v);
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
        assert_eq!(number("6 MS", "Total time", &["milliseconds", "ms"], "1.5").map(|n| n.v), Ok(6.0));
    }

    #[test]
    fn a_number_is_kept_exactly_as_typed_and_quoted_as_typed() {
        let ex = |t: &str| number(t, "Peak", &["mv"], "-70").unwrap().d;
        assert_eq!(ex("-63.6"), -63_600_000_000);
        assert_eq!(ex("-73.6") + 10 * SCALE, ex("-63.6"), "ten more than -73.6 is exactly -63.6");
        assert_eq!(ex("0.4999"), 499_900_000);
        assert_eq!(ex("12,0001"), 12_000_100_000);
        assert_eq!(ex("-0"), 0);
        for t in ["-63.6", "0.4999", "12.0001", "-72.9999", "5", "-100.0001", "0.123456789"] {
            assert_eq!(show_d(ex(t)), t.replace('-', "\u{2212}"), "{t}: a message quotes what was typed");
        }
        assert_eq!(show_d(ex("4.50")), "4.5");
    }

    fn refusal(list: &[(&'static str, Value)]) -> Refusal { read(&with(list)).expect_err("expected a refusal") }

    #[test]
    fn each_refusal_is_true_and_says_what_to_do() {
        let cases: Vec<(Vec<(&'static str, Value)>, &str, &str)> = vec![
            (vec![s("resting", "x")], "is not a number", "Nothing is drawn until"),
            (vec![s("peak", "")], "is empty", "Nothing is drawn until"),
            (vec![s("resting", "5")], "negative at rest", "negative resting"),
            (vec![s("resting", "-5")], "above the \u{2212}14 mV limit", "\u{2212}14 mV or lower"),
            (vec![s("resting", "-150")], "outside the \u{2212}100 to \u{2212}14 mV", "inside that range"),
            (vec![s("peak", "150")], "outside the", "inside that range"),
            (vec![s("undershoot", "-120")], "outside the", "inside that range"),
            (vec![s("threshold", "-65")], "at least 10 mV above the resting potential", "Raise the threshold"),
            (vec![s("threshold", "-3")], "within 4 mV of 0 mV", "\u{2212}4 mV or lower"),
            (vec![s("threshold", "0")], "at or above 0 mV", "below zero"),
            (vec![s("threshold", "12"), s("peak", "60")], "at or above 0 mV", "below zero"),
            (vec![s("threshold", "-120")], "outside the \u{2212}100 to \u{2212}4 mV", "inside that range"),
            (vec![s("peak", "-40")], "at least 20 mV above the threshold", "Raise the peak"),
            (vec![s("undershoot", "-60")], "is above the resting potential", "Type a lower value"),
            (vec![s("undershoot", "-72")], "too shallow", "at least 3 mV below"),
            (vec![s("stimulus", "0")], "before 0.5 ms", "Type 0.5 ms or later"),
            (vec![s("duration", "13")], "over the 12 ms", "shorter time"),
            (vec![s("duration", "5")], "needs about 4.7 ms", "Make the total time at least 5.7 ms."),
            (vec![s("stimulus", "7"), s("duration", "8")], "leaves only 1 ms of the 8 ms trace", "at least 11.7 ms, or move the stimulus to 3.3 ms or earlier"),
            (vec![s("stimulus", "8"), s("duration", "12")], "leaves only 4 ms of the 12 ms trace", "Move the stimulus to 7.3 ms or earlier"),
            (vec![s("stimulus", "100"), s("duration", "8")], "is not before the end of the 8 ms trace", "Move the stimulus to 3.3 ms or earlier"),
            (vec![s("stimulus", "9"), s("duration", "5")], "is not before the end of the 5 ms trace", "stimulus time of 0.5 to 7.3 ms"),
            (vec![s("duration", "-5")], "is not a time", "Type a total time of 5.7 to 12 ms."),
            (vec![s("duration", "0")], "is not a time", "Type a total time of 5.7 to 12 ms."),
            (vec![s("stimulus", "7"), s("duration", "0")], "is not a time", "Type a total time of 11.7 to 12 ms."),
            (vec![s("stimulus", "7.3"), s("duration", "0")], "is not a time", "Type a total time of 12 ms."),
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
        assert!(read(&with(&[s("threshold", "-4"), s("peak", "40")])).is_ok(), "-4 is the highest threshold");
        assert!(read(&with(&[s("threshold", "-3.9")])).is_err());
        assert!(read(&with(&[s("resting", "-14"), s("threshold", "-4"), s("undershoot", "-30")])).is_ok(), "-14 is the highest resting potential");
        assert!(read(&with(&[s("resting", "-13.9"), s("threshold", "-4")])).is_err());
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
            (-40.0, -30.0, 70.0, -50.0), (-100.0, -90.0, 70.0, -100.0), (-100.0, -4.0, 70.0, -100.0), (-14.0, -4.0, 40.0, -100.0),
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
                let ts = if dur == "5.2" { "0.5" } else { "0.6" }; // 0.5 ms in a 12 ms trace leaves too little room before it at Large
                let svg = render_of(&[s("label_size", size), s("duration", dur), s("stimulus", ts)]);
                let (curve, _) = paths(&svg);
                let ax = axes_of(&svg);
                let mut worst_len: f64 = 0.0;
                let mut worst_sag: f64 = 0.0;
                // the true curve, read back through the picture's own axes, against each chord of the path
                let truth = sp(-70.0, -55.0, 40.0, -80.0, num_of(ts), num_of(dur), false);
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
                assert!((peak_pt.0 - ax.x(num_of(ts) + 0.8)).abs() < 0.8, "{size} {dur}: peak at x {} vs {}", peak_pt.0, ax.x(num_of(ts) + 0.8));
            }
        }
    }
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
            mk("-70", "-55", "40", "-75", "3", "10"),
            mk("-100", "-90", "70", "-100", "0.5", "5.2"),
            mk("-100", "-90", "70", "-100", "1", "8"),
            mk("-100", "-4", "70", "-100", "0.6", "12"),
            mk("-14", "-4", "40", "-100", "2", "12"),
            mk("-45", "-33", "70", "-60", "4", "12"),
            mk("-62.3", "-50.5", "37.5", "-77.9", "1.7", "9.3"),
            // the late return on a deep dip, and the shallowest dip with the earliest stimulus in the longest trace
            mk("-14", "-4", "70", "-100", "7.3", "12"),
            mk("-70", "-55", "40", "-75", "0.6", "12"),
            mk("-14", "-4", "70", "-100", "0.8", "12"),
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
        let numbered = if !ph { 0 } else if fail { 3 + usize::from(stim) + usize::from(thr) } else { 1 + usize::from(stim) + usize::from(thr) + 3 + 2 * usize::from(dips) };
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
        assert!(checked >= 6528, "{checked} checked, {refused} refused");
        assert_eq!(refused, 0, "every set in the sweep is a valid trace");
    }

    #[test]
    fn placement_never_fails_for_any_valid_trace() {
        // a wide grid of valid traces: if the values read, the figure is drawn
        let mut n = 0;
        for rest in [-100.0f64, -80.0, -70.0, -50.0, -30.0, -20.0, -14.0] {
            for dthr in [10.0, 15.0, 30.0] {
                for dpk in [20.0, 40.0, 60.0] {
                    for dip in [0.0, 3.0, 10.0, 25.0] {
                        let (thr, peak, under) = (rest + dthr, rest + dthr + dpk, rest - dip);
                        if thr > -4.0 || peak > 70.0 || under < -100.0 { continue; }
                        for (ts, dur) in [(0.5, 5.2), (1.0, 8.0), (4.0, 12.0)] {
                            for size in ["Small", "Large"] {
                                for mask in [31u32, 0b10111, 0b01110, 0b11111] {
                                    let list = vec![s("resting", &rest.to_string()), s("threshold", &thr.to_string()), s("peak", &peak.to_string()), s("undershoot", &under.to_string()), s("stimulus", &ts.to_string()), s("duration", &dur.to_string())];
                                    let mut l = list.clone();
                                    l.extend(switches(mask, false, true, size));
                                    let p = with(&l);
                                    if read(&p).is_err() { continue; }
                                    if let Err(e) = plan(&p) {
                                        // the only refusals are the two true ones: a dip too shallow for the return dot (never a
                                        // dip of 6 mV or more: even the widest scale draws that 4 px deep halfway up) and a
                                        // stimulus with no room before it
                                        assert!(e.why.contains("too shallow at this scale") && dip < 6.0 || e.why.contains("leaves only"), "{l:?}: {e}");
                                        continue;
                                    }
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
        assert!(n > 1200, "{n}");
    }

    // ------------------------------------------------------------------ the exercise
    const NAMES: [&str; 15] = [
        "resting potential", "threshold", "depolarisation", "peak", "repolarisation", "hyperpolarisation", "undershoot", "stimulus",
        "channels open", "absolute", "relative", "na+", "k+", "local", "return to",
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
    fn failed_initiation_draws_a_bump_and_no_spike_bands_and_only_the_note_says_so() {
        let f = |extra: &[(&'static str, Value)]| { let mut l = vec![b("fail", true)]; l.extend(extra.iter().cloned()); l };
        let svg = render_of(&f(&[]));
        assert!(paths(&svg).1.is_empty(), "no channel brackets in a failure");
        // the default switches have the channel bands on, so something was dropped: the NOTE says so, the canvas does not
        let notes = notes_for(ID, &with(&f(&[])));
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("too weak to start a spike"), "{notes:?}");
        assert!(!svg.to_lowercase().contains("weak"));
        // with both band switches off nothing was dropped, so there is nothing to say
        assert!(notes_for(ID, &with(&f(&[b("show_channels", false)]))).is_empty());
        // and the ordinary spike never carries the note
        assert!(notes_for(ID, &with(&[])).is_empty());
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
    fn no_undershoot_drops_the_pair_that_would_point_at_one_flat_line_and_only_the_note_says_so() {
        let none = |extra: &[(&'static str, Value)]| { let mut l = vec![s("undershoot", "-70")]; l.extend(extra.iter().cloned()); l };
        let k = joined(&render_of(&none(&[b(SHOW_ANSWERS, true)])));
        for gone in ["Hyperpolarisation", "Return to resting", "Relative"] { assert!(!k.contains(gone), "{gone}"); }
        assert!(k.contains("Resting potential") && k.contains("Repolarisation"));
        // the key and the figure hold the same 1..N with no gap: stimulus, resting, threshold, depolarisation, peak, repolarisation
        let marks = disc_marks(&render_of(&none(&[b(SHOW_ANSWERS, false)])));
        assert_eq!(marks.iter().filter(|m| *m == "6").count(), 2, "{marks:?}");
        assert!(!marks.iter().any(|m| m == "7"), "{marks:?}");
        // the note says why, in the editor only
        let notes = notes_for(ID, &with(&none(&[])));
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("No undershoot") && notes[0].contains("left out"), "{notes:?}");
        // nothing that depends on a dip is on: nothing to say
        assert!(notes_for(ID, &with(&none(&[b("show_phases", false), b("show_refractory", false)]))).is_empty());
        // with a dip it is never said
        assert!(notes_for(ID, &with(&[b("show_refractory", true)])).is_empty());
        // K+ ends at the end of the fall when there is no dip, and the one refractory period reaches the same time
        let rf = render_of(&none(&[b("show_refractory", true)]));
        assert_eq!(paths(&rf).1.len(), 6 + 3, "Na, K and the absolute period only");
        // the two pointers the exercise needs to tell apart are not on the same stretch: with a dip the
        // resting pointer's dot is on the flat line before the stimulus and the return pointer's dot is on the recovering limb
        let svg = render_of(&[]);
        let ax = axes_of(&svg);
        let dots: Vec<_> = circles(&svg).into_iter().filter(|c| c.2 < 6.0).collect();
        let sp = sp(-70.0, -55.0, 40.0, -80.0, 1.0, 8.0, false);
        let before = dots.iter().filter(|d| d.0 < ax.x(sp.ts) && (d.1 - ax.y(-70.0)).abs() < 0.3).count();
        let limb = dots.iter().filter(|d| d.0 > ax.x(sp.t_low()) + 2.0 && d.0 < ax.x(sp.t_settled()) && (d.1 - ax.y(-70.0)).abs() >= 4.0).count();
        assert_eq!((before, limb), (1, 1), "{dots:?}");
    }

    #[test]
    fn the_channel_bands_sit_at_the_times_the_curve_says() {
        let svg = render_of(&[s("stimulus", "2"), s("duration", "10"), b("show_refractory", true)]);
        let ax = axes_of(&svg);
        let sp = sp(-70.0, -55.0, 40.0, -80.0, 2.0, 10.0, false);
        let (curve, br) = paths(&svg);
        // each bracket is three strokes: left tick, bar, right tick; drawn 3 px in from its times
        let spans: Vec<(f64, f64)> = br.chunks(3).map(|c| (c[0].0 .0 - 3.0, c[2].0 .0 + 3.0)).collect();
        assert_eq!(spans.len(), 4);
        // what the curve itself says, read off the picture: the height of the curve at a pixel column
        let mv_at = |x: f64| {
            let q = curve.windows(2).find(|w| w[0].0 <= x && x <= w[1].0).unwrap();
            let k = if q[1].0 > q[0].0 { (x - q[0].0) / (q[1].0 - q[0].0) } else { 0.0 };
            (q[0].1 + k * (q[1].1 - q[0].1) - ax.y(0.0)) / ax.ky
        };
        let near = |v: f64, want: f64, tol: f64| (v - want).abs() <= tol;
        let (na, k, abs, rel) = (spans[0], spans[1], spans[2], spans[3]);
        // Na+ channels open from the threshold to the peak (Campbell 48.3, OpenStax 12.4: activation at threshold, inactivation at the peak)
        assert!(near(mv_at(na.0), -55.0, 0.8) && near(mv_at(na.1), 40.0, 0.8), "Na+ {na:?}");
        // K+ channels open from the peak until the membrane is back at rest, within a millivolt: after the undershoot
        assert!(near(mv_at(k.0), 40.0, 0.8) && near(mv_at(k.1), -70.0, 1.05), "K+ {k:?} ends at {}", mv_at(k.1));
        assert!(k.1 > ax.x(sp.t_low()), "K+ outlasts the lowest point");
        // absolute refractory: from the threshold until the falling curve is back at rest; relative: until it has settled
        assert!(near(mv_at(abs.0), -55.0, 0.8) && near(mv_at(abs.1), -70.0, 0.8), "absolute {abs:?}");
        assert!(abs.1 < ax.x(sp.t_low()) && abs.1 > ax.x(sp.t_peak()), "absolute ends on the falling limb, before the lowest point");
        assert!((rel.0 - abs.1).abs() < 1e-6 && (rel.1 - k.1).abs() < 1e-6, "relative runs from the end of the absolute period to the end of K+");
        assert!(near(mv_at(rel.1), -70.0, 1.05));
        // and with a coarser eye: the same four times as the module's curve gives
        let want = [(sp.t_threshold(), sp.t_peak()), (sp.t_peak(), sp.t_settled()), (sp.t_threshold(), sp.t_back_to_rest()), (sp.t_back_to_rest(), sp.t_settled())];
        for ((a, c), (t0, t1)) in spans.iter().zip(want) {
            assert!((a - ax.x(t0)).abs() < 0.02 && (c - ax.x(t1)).abs() < 0.02, "{a} {c} vs {t0} {t1}");
            assert!(*a < *c);
        }
        // the return pointer's dot is on the recovering limb: after the lowest point, inside both the K+ and the relative
        // refractory brackets, and at least 4 px off the rest line
        let dots: Vec<_> = circles(&svg).into_iter().filter(|c| c.2 < 6.0).collect();
        let after_low: Vec<_> = dots.iter().filter(|d| d.0 > ax.x(sp.t_low()) + 2.0).collect();
        assert_eq!(after_low.len(), 1, "{dots:?}");
        let ret = after_low[0];
        assert!(ret.0 > k.0 && ret.0 < k.1 - 1.0 && ret.0 > rel.0 && ret.0 < rel.1 - 1.0, "return dot at {} outside K+ {k:?} / relative {rel:?}", ret.0);
        assert!((ret.1 - ax.y(sp.rest)).abs() >= 4.0, "return dot {} px from the rest line", (ret.1 - ax.y(sp.rest)).abs());
        // the lowest point is inside the K+ and relative brackets
        let low = curve.iter().map(|p| p.1).fold(f64::MIN, f64::max);
        let xl = curve.iter().find(|p| p.1 == low).unwrap().0;
        assert!(xl > k.0 && xl < k.1 && xl > rel.0 && xl < rel.1);
    }

    #[test]
    fn the_recovery_ends_within_a_millivolt_of_rest_for_every_dip() {
        for dip in [3.0, 5.0, 10.0, 25.0, 50.0, 86.0] {
            let sp = sp(-14.0 - 0.0, -4.0, 40.0, -14.0 - dip, 1.0, 12.0, false);
            let e = sp.t_settled();
            assert!((sp.v(e) - sp.rest).abs() <= REST_TOL + 1e-6 && (sp.v(e - 0.01) - sp.rest).abs() > REST_TOL - 0.2, "{dip}: {}", sp.v(e) - sp.rest);
            assert!(e > sp.t_low() && e < sp.ts + NEED, "{dip}: settled by {e}, the trace is kept {} long after the stimulus", NEED);
        }
        assert_eq!(sp(-70.0, -55.0, 40.0, -70.0, 1.0, 8.0, false).t_settled(), 1.0 + T_RISE + T_FALL);
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

    /// 0.299 R + 0.587 G + 0.114 B of a `#rgb` or `#rrggbb` colour, 0..255.
    fn luma(c: &str) -> f64 {
        let h = c.trim_start_matches('#');
        let h: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
        let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap() as f64;
        0.299 * v(0) + 0.587 * v(2) + 0.114 * v(4)
    }
    /// The stroke of the curve itself: the one path made of straight segments.
    fn curve_stroke(svg: &str) -> String {
        svg.split("<path ").skip(1).map(|p| p.split('>').next().unwrap()).find(|h| attr(h, "d").is_some_and(|d| d.contains('L'))).and_then(|h| attr(h, "stroke")).unwrap()
    }

    #[test]
    fn print_is_grey_only_and_every_mark_survives_a_66_percent_threshold() {
        for set in sets().into_iter().take(4) {
            for mask in [0u32, 31, 0b10101, 0b01010] {
                for fail in [false, true] {
                    let mut l = set.clone();
                    l.extend(switches(mask, fail, mask & 1 == 0, "Medium"));
                    l.push(s("style", "Print"));
                    let svg = (def().render)(&with(&l));
                    assert!(crate::figures::census::non_grey_paints_within(&svg, 0).is_empty(), "{:?}", crate::figures::census::non_grey_paints_within(&svg, 0));
                    // the CURVE element is black, not just something somewhere in the picture
                    assert_eq!(curve_stroke(&svg), "#000", "the curve itself is black in Print");
                    // everything drawn as ink (all but the light grid) is dark enough to survive the 66 % copy
                    let bw = 0.66 * 255.0;
                    for (_, _, col) in lines_w(&svg) { assert!(luma(&col) < bw, "a line of {col} fades in the 66 % copy"); }
                    for part in svg.split("<path ").skip(1) {
                        let head = part.split('>').next().unwrap();
                        let col = attr(head, "stroke").unwrap();
                        assert!(luma(&col) < bw, "a path of {col} fades in the 66 % copy");
                    }
                }
            }
        }
        // and Colour is not grey (the difference is real)
        assert!(!crate::figures::census::non_grey_paints_within(&render_of(&[]), 0).is_empty());
    }

    #[test]
    fn a_pale_curve_or_threshold_line_in_print_would_be_caught() {
        // the test above must be able to fail: a stroke of #bbbbbb is lighter than the 66 % cut
        assert!(luma("#bbbbbb") > 0.66 * 255.0 && luma("#000") < 1.0 && luma("#444444") < 0.66 * 255.0);
        let svg = render_of(&[s("style", "Print")]);
        // only the curve element is paled: black brackets, axes and pattern fills stay in the picture
        let mut pale = String::new();
        for (i, part) in svg.split("<path ").enumerate() {
            if i == 0 { pale.push_str(part); continue; }
            pale.push_str("<path ");
            let is_curve = attr(part.split('>').next().unwrap(), "d").is_some_and(|d| d.contains('L'));
            if is_curve { pale.push_str(&part.replacen("stroke=\"#000\"", "stroke=\"#bbbbbb\"", 1)); } else { pale.push_str(part); }
        }
        assert!(pale.contains("stroke=\"#000\" stroke-width=\"2.2\""), "something else is still black");
        assert_eq!(curve_stroke(&pale), "#bbbbbb", "the curve is found by its element, not by any black stroke");
    }

    #[test]
    fn in_print_the_four_brackets_differ_by_dash_and_the_plus_and_dots_stand_out() {
        let svg = render_of(&[s("style", "Print"), b("show_refractory", true)]);
        let dashes: Vec<String> = svg.split("<path ").skip(1).map(|p| p.split('>').next().unwrap()).filter(|h| attr(h, "d").is_some_and(|d| d.contains('V'))).map(|h| attr(h, "stroke-dasharray").unwrap_or_default()).collect();
        assert_eq!(dashes.len(), 4);
        let mut u = dashes.clone();
        u.sort();
        u.dedup();
        assert_eq!(u.len(), 4, "four different dash styles: {dashes:?}");
        // in Colour they are told apart by colour and stay solid
        let col = render_of(&[b("show_refractory", true)]);
        assert!(!col.contains("stroke-dasharray=\"6 3\"") && !col.contains("stroke-dasharray=\"2 3\"") && !col.contains("stroke-dasharray=\"8 3 2 3\""));
        // each dot has a white ring so it shows on the black curve
        let rings = svg.split("<circle ").skip(1).map(|c| c.split('>').next().unwrap()).filter(|c| num(c, "r") > 2.0 && num(c, "r") < 6.0).collect::<Vec<_>>();
        assert!(rings.len() >= 5);
        for c in rings { assert_eq!(attr(c, "stroke").unwrap(), "#ffffff", "{c}"); assert!(num(c, "stroke-width") >= 1.2); }
        // the plus of Na+ and K+ is a bold glyph at nearly full size
        let key = render_of(&[b(SHOW_ANSWERS, true), s("style", "Print"), s("label_size", "Small")]);
        let plus = key.split("<tspan ").filter(|t| t.contains(">+</tspan>")).collect::<Vec<_>>();
        assert_eq!(plus.len(), 2);
        for t in plus { assert!(num(t, "font-size") >= 0.9 * 13.0 * 0.8 - 0.01 && t.contains("font-weight=\"700\""), "{t}"); }
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

    /// Every phrase that gives away why something is missing (the failed initiation, the absent dip, anything left
    /// out). Written here, not read from the module, and searched for in the whole of the SVG source: text, title,
    /// description, attributes.
    const DISCLOSURES: [&str; 14] = [
        "too weak", "weak", "start a spike", "fails to start", "failed", "no dip", "no undershoot", "lowest point", "resting level",
        "left out", "not drawn", "belong to", "dropped", "omitted",
    ];

    fn disclosure_in(svg: &str) -> Option<&'static str> {
        let low = svg.to_lowercase();
        DISCLOSURES.iter().copied().find(|p| low.contains(p))
    }

    #[test]
    fn the_editor_note_carries_what_was_left_out_and_the_canvas_never_does_over_ten_thousand_inputs() {
        let mut r = Rng(0x9e3779b97f4a7c15);
        let (mut refused, mut figures, mut noted, mut silent) = (0, 0, 0, 0);
        for i in 0..10_000 {
            let mut l = vec![
                s("resting", &junk(&mut r, -105.0, -5.0, "mV")),
                s("threshold", &junk(&mut r, -80.0, 10.0, "mV")),
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
                let rest = format!("{}", r.f(-95.0, -40.0).round());
                l[0] = s("resting", &rest);
                l[1] = s("threshold", &format!("{}", (r.f(-35.0, -4.0)).round()));
                l[2] = s("peak", &format!("{}", (r.f(35.0, 65.0)).round()));
                l[3] = s("undershoot", &if r.next() % 5 == 0 { rest.clone() } else { format!("{}", (r.f(-100.0, -75.0)).round()) });
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
                    // a refusal's canvas is the note, word for word
                    assert_eq!(words, notes[0].split_whitespace().collect::<Vec<_>>().join(" "), "canvas vs note: {l:?}");
                }
                Ok(sp) => match plan(&p) {
                    Err(e) => {
                        refused += 1;
                        assert!(is_refusal, "{l:?}");
                        assert_eq!(words, notes[0].split_whitespace().collect::<Vec<_>>().join(" "), "{l:?}");
                        assert!(e.why.contains("no clear place") || e.why.contains("too close") || e.why.contains("would touch") || e.why.contains("too shallow at this scale") || e.why.contains("leaves only"), "{}", e.why);
                    }
                    Ok(pl) => {
                        figures += 1;
                        assert!(!is_refusal, "{l:?}");
                        assert_eq!(notes, pl.notes, "{l:?}");
                        // the picture is silent: no sentence of the note, and none of the phrases that give the reason away,
                        // anywhere in the SVG source (so not in a caption, the description, the title or an attribute)
                        assert_eq!(disclosure_in(&svg), None, "the canvas discloses {:?}: {l:?}", disclosure_in(&svg));
                        for known in [NOTE_NO_DIP, NOTE_FAILED] { assert!(!words.contains(known), "{l:?}"); }
                        // the note says something exactly when the switches ask for something this trace cannot have
                        let should = if sp.fail { p.b("show_channels") || p.b("show_refractory") } else { !sp.dips() && (p.b("show_refractory") || p.b("show_phases")) };
                        assert_eq!(!notes.is_empty(), should, "{l:?}");
                        // and what it says is true of the picture: those items really are absent, the rest really present
                        let (n, lt) = expected_pointers(
                            u32::from(p.b("show_threshold")) | u32::from(p.b("show_stimulus")) << 1 | u32::from(p.b("show_phases")) << 2 | u32::from(p.b("show_channels")) << 3 | u32::from(p.b("show_refractory")) << 4,
                            sp.fail, sp.dips(),
                        );
                        assert_eq!(circles(&svg).iter().filter(|c| c.2 >= 6.0).count(), 2 * (n + lt), "{l:?}");
                        if should {
                            noted += 1;
                            let said = notes.join(" ").to_lowercase();
                            if sp.fail { assert!(said.contains("too weak to start a spike") && said.contains("channel"), "{notes:?}"); } else { assert!(said.contains("no undershoot") && said.contains("left out"), "{notes:?}"); }
                        } else {
                            silent += 1;
                        }
                    }
                },
            }
            // nothing in a render is not a number
            if !is_refusal { assert!(!svg.contains("NaN") && !svg.contains("inf\"") && !svg.contains("-inf"), "{l:?}"); }
        }
        assert!(refused > 2000 && figures > 2000 && noted > 100 && silent > 500, "{refused} refused, {figures} drawn, {noted} with a note, {silent} silent");
    }

    /// Everything the canvas says that is not a tick number, a mark inside a disc, an axis title or a row label, and a
    /// description that is not the fixed sentence. A student copy must have none: whatever else is on it names or hints.
    fn strays(svg: &str) -> Vec<String> {
        let tx = texts(svg);
        let ds: Vec<_> = circles(svg).into_iter().filter(|c| c.2 >= 6.0).collect();
        let mut out = Vec::new();
        for t in &tx {
            let in_disc = ds.iter().any(|d| ((t.x - d.0).powi(2) + (t.y - d.1).powi(2)).sqrt() < d.2);
            let number = t.s.replace('\u{2212}', "-").parse::<f64>().is_ok();
            let fixed = ["Time (ms)", "Membrane potential (mV)", "Channels", "Periods"].contains(&t.s.as_str());
            if !(number || fixed || (in_disc && t.s.chars().count() <= 2)) { out.push(t.s.clone()); }
        }
        let desc = svg.split("<desc>").nth(1).and_then(|x| x.split("</desc>").next()).unwrap_or("");
        let fixed_desc = desc.strip_prefix("Membrane potential in mV against time in ms for a neuron.").map(|r| r.trim()).is_some_and(|r| r.is_empty() || (r.ends_with(" pointers to name.") && r.trim_end_matches(" pointers to name.").trim().parse::<usize>().is_ok()));
        if !fixed_desc { out.push(format!("<desc>{desc}")); }
        let title = svg.split("<title>").nth(1).and_then(|x| x.split("</title>").next()).unwrap_or("");
        if title != "Action potential" { out.push(format!("<title>{title}")); }
        out
    }

    #[test]
    fn a_student_copy_says_nothing_at_all_beyond_numbers_marks_axis_titles_and_row_labels() {
        let mut checked = 0;
        for set in sets() {
            for fail in [false, true] {
                for size in ["Small", "Large"] {
                    for style in ["Colour", "Print"] {
                        for mask in [0u32, 7, 21, 31] {
                            let mut l = set.clone();
                            l.extend(switches(mask, fail, false, size));
                            l.push(s("style", style));
                            let p = with(&l);
                            if plan(&p).is_err() { continue; }
                            let svg = (def().render)(&p);
                            assert_eq!(strays(&svg), Vec::<String>::new(), "{l:?}");
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked > 400, "{checked}");
        // the check is real: a caption on a failed canvas, in any words, is seen
        let svg = render_of(&[b(SHOW_ANSWERS, false), b("fail", true)]);
        for words in ["No action potential: all-or-none", "Subthreshold", "Stimulus too weak", "Nothing happens"] {
            let t = svg.replace("</svg>", &format!("<text x=\"20\" y=\"14\" font-size=\"12\" text-anchor=\"start\">{words}</text></svg>"));
            assert_eq!(strays(&t), vec![words.to_string()]);
        }
        let d = svg.replacen("</desc>", " It fails.</desc>", 1);
        assert_eq!(strays(&d).len(), 1);
        assert_eq!(strays(&svg.replacen("<title>Action potential", "<title>Failed initiation", 1)).len(), 1);
    }

    #[test]
    fn the_whole_svg_of_every_variant_is_free_of_every_disclosure_phrase() {
        // answers off and on, every state a disclosure could belong to, every size, both styles
        let mut checked = 0;
        for set in [vec![], vec![s("undershoot", "-70")]] {
            for fail in [false, true] {
                for key in [false, true] {
                    for size in ["Small", "Medium", "Large"] {
                        for style in ["Colour", "Print"] {
                            for mask in 0..32u32 {
                                let mut l = set.clone();
                                l.extend(switches(mask, fail, key, size));
                                l.push(s("style", style));
                                let svg = render_of(&l);
                                assert_eq!(disclosure_in(&svg), None, "{l:?}");
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 2 * 2 * 2 * 3 * 2 * 32);
        // the phrase search is real: it sees the phrases in the places they used to be
        let svg = render_of(&[]);
        let caption = svg.replace("</svg>", "<text x=\"1\" y=\"1\">This input is too weak to start a spike</text></svg>");
        assert_eq!(disclosure_in(&caption), Some("too weak"));
        let desc = svg.replacen("</desc>", " a trace that fails to start a spike</desc>", 1);
        assert_eq!(disclosure_in(&desc), Some("start a spike"));
        let title = svg.replacen("<title>", "<title>Failed ", 1);
        assert_eq!(disclosure_in(&title), Some("failed"));
    }

    // ------------------------------------------------------------------ exact boundaries
    /// A value in tenths as a typed one-decimal string.
    fn tenth(t: i64) -> String { format!("{}{}.{}", if t < 0 { "-" } else { "" }, t.abs() / 10, t.abs() % 10) }

    #[test]
    fn a_one_decimal_value_typed_exactly_on_a_boundary_is_never_refused() {
        let ok = |r: i64, t: i64, p: i64, u: i64, ts: i64, d: i64| read(&with(&[s("resting", &tenth(r)), s("threshold", &tenth(t)), s("peak", &tenth(p)), s("undershoot", &tenth(u)), s("stimulus", &tenth(ts)), s("duration", &tenth(d))])).is_ok();
        let mut n = 0;
        // every resting potential from -100.0 to -14.0, the threshold exactly 10 above, the peak exactly 20 above that
        for r in -1000..=-140i64 {
            let (t, p) = (r + 100, r + 300);
            let u_ok = r - 30 >= -1000;
            assert!(ok(r, t, p, r, 10, 80), "rest {r}");
            if u_ok { assert!(ok(r, t, p, r - 30, 10, 80), "dip of exactly 3 at rest {r}"); }
            assert!(!ok(r, t - 1, p, r, 10, 80), "a threshold 9.9 above rest {r}");
            assert!(!ok(r, t, p - 1, r, 10, 80), "a peak 19.9 above the threshold {t}");
            assert!(!ok(r, t, p, r - 29, 10, 80), "a dip of 2.9 at rest {r}");
            assert!(!ok(r, t, p, r + 1, 10, 80), "an undershoot above rest {r}");
            n += 6;
        }
        assert!(!ok(-1001, -901, -701, -1001, 10, 80), "rest below -100");
        assert!(!ok(-139, -39, 161, -139, 10, 80), "rest -13.9 leaves no room for a threshold under zero");
        // the same for the times, to the tenth
        for ts in 5..=73i64 {
            assert!(ok(-700, -550, 400, -800, ts, ts + 47), "ts {ts}");
            assert!(!ok(-700, -550, 400, -800, ts, ts + 46), "ts {ts}: 0.1 ms short");
            n += 2;
        }
        assert!(!ok(-700, -550, 400, -800, 4, 80) && ok(-700, -550, 400, -800, 5, 52) && !ok(-700, -550, 400, -800, 5, 51));
        assert!(ok(-700, -550, 400, -800, 73, 120) && !ok(-700, -550, 400, -800, 74, 120) && !ok(-700, -550, 400, -800, 10, 121));
        // other spellings of a boundary value: a comma, a true minus, a unit
        let l = [s("resting", "\u{2212}73,6 mV"), s("threshold", "-63.6"), s("peak", "-43,6mV"), s("undershoot", "-76.6")];
        assert!(read(&with(&l)).is_ok());
        assert!(n > 5000);
    }

    #[test]
    fn a_message_never_contradicts_the_number_it_quotes() {
        let why = |l: &[(&'static str, Value)]| refusal(l).why;
        assert!(why(&[s("stimulus", "0.4999")]).contains("(0.4999 ms) is before 0.5 ms"));
        assert!(why(&[s("duration", "12.0001")]).contains("12.0001 ms is over the 12 ms"));
        assert!(why(&[s("undershoot", "-70.0001")]).contains("only 0.0001 mV below rest"));
        assert!(why(&[s("undershoot", "-72.9999")]).contains("only 2.9999 mV below rest"));
        assert!(why(&[s("resting", "-100.0001")]).contains("\u{2212}100.0001 mV, outside the \u{2212}100 to"));
        assert!(why(&[s("resting", "-9.99")]).contains("above the \u{2212}14 mV limit"));
        assert!(why(&[s("duration", "-5")]).contains("is not a time") && !why(&[s("duration", "-5")]).contains("\u{2212}5 ms trace"));
        assert!(why(&[s("threshold", "-65.0")]).contains("(\u{2212}65 mV)"));
    }

    /// The width of the canvas and the smallest text on it, from the picture alone.
    fn width_and_smallest_text(svg: &str) -> (f64, f64) {
        (canvas(svg).0, texts(svg).iter().map(|t| t.size).fold(f64::MAX, f64::min))
    }

    #[test]
    fn the_canvas_never_widens_for_an_early_stimulus_so_the_smallest_text_prints_legibly_in_a_column() {
        // A 6.5 inch column is 624 px at 96 dpi, and the canvas is scaled to fit it. Text smaller than 9 px on paper is
        // not read, so the smallest text of the picture, times 624 over the canvas width, must stay at 9 px or more.
        // That is a ceiling on the canvas width for each size, worked out here from the text actually drawn
        let (column, floor) = (624.0, 9.0);
        let mut widest = [0.0f64; 3];
        let mut worst_print = f64::MAX;
        for set in sets() {
            for (i, size) in ["Small", "Medium", "Large"].iter().enumerate() {
                for mask in [0u32, 31] {
                    for fail in [false, true] {
                        for key in [false, true] {
                            let mut l = set.clone();
                            l.extend(switches(mask, fail, key, size));
                            let p = with(&l);
                            if plan(&p).is_err() { continue; }
                            let (w, small) = width_and_smallest_text(&(def().render)(&p));
                            widest[i] = widest[i].max(w);
                            worst_print = worst_print.min(small * column / w.max(column));
                            assert!(small * column / w.max(column) >= floor - 1e-6, "{set:?} {size}: {w} px wide, smallest text {small} px prints at {}", small * column / w);
                        }
                    }
                }
            }
        }
        // the canvas is the same width for every stimulus time and every total time (the plot has a fixed width, the
        // key and the margins decide the rest), so the widest one is no wider than the default plus a few px
        for (w, cap) in widest.iter().zip([640.0, 650.0, 720.0]) { assert!(*w <= cap, "widest canvas {w} px over the {cap} px ceiling"); }
        // and the two cases that used to widen the plot: stimulus 0.5 ms in a 12 ms trace, and 0.5 in 5.2
        for (ts, dur) in [("0.6", "12"), ("0.5", "5.2"), ("0.5", "8")] {
            for size in ["Small", "Medium", "Large"] {
                let a = render_of(&[s("stimulus", ts), s("duration", dur), s("label_size", size)]);
                let c = render_of(&[s("stimulus", "4"), s("duration", "12"), s("label_size", size)]);
                assert_eq!(canvas(&a).0, canvas(&c).0, "{ts}/{dur} {size}: the width must not depend on when the stimulus is");
                // a dot is never put on the y axis
                let ax = axes_of(&a);
                for d in circles(&a).into_iter().filter(|c| c.2 < 6.0) { assert!(d.0 - ax.x(0.0) >= 12.0, "{ts}/{dur} {size}: a dot {} px from the y axis", d.0 - ax.x(0.0)); }
            }
        }
    }

    #[test]
    fn every_disc_keeps_clear_of_the_true_curve_and_of_every_other_disc() {
        // the curve is walked every 0.01 ms from the module's own function, not read back from the drawn chords, which
        // cut inside a steep limb; the gaps are literal pixels
        let (mut closest_curve, mut closest_disc) = (f64::MAX, f64::MAX);
        let mut bad = Vec::new();
        for set in sets() {
            for size in ["Small", "Medium", "Large"] {
                for fail in [false, true] {
                    for mask in [31u32, 7, 15] {
                        let mut l = set.clone();
                        l.extend(switches(mask, fail, true, size));
                        let p = with(&l);
                        let Ok(sp) = read(&p) else { continue };
                        if plan(&p).is_err() { continue; }
                        let svg = (def().render)(&p);
                        let ax = axes_of(&svg);
                        let n = (sp.dur / 0.01) as usize;
                        let walk: Vec<P> = (0..=n).map(|i| { let t = i as f64 * 0.01; (ax.x(t), ax.y(sp.v(t))) }).collect();
                        let ds: Vec<_> = circles(&svg).into_iter().filter(|c| c.2 >= 6.0).collect();
                        for (i, d) in ds.iter().enumerate() {
                            let gap = walk.iter().map(|q| dist2(*q, (d.0, d.1))).fold(f64::MAX, f64::min) - d.2;
                            if gap < 2.0 { bad.push(format!("{set:?} {size} fail {fail} mask {mask}: a disc is {gap:.2} px from the curve")); }
                            closest_curve = closest_curve.min(gap);
                            for e in &ds[i + 1..] {
                                let g = dist2((d.0, d.1), (e.0, e.1)) - d.2 - e.2;
                                closest_disc = closest_disc.min(g);
                                if g < 3.0 { bad.push(format!("{set:?} {size} fail {fail} mask {mask}: two discs {g:.2} px apart")); }
                            }
                        }
                    }
                }
            }
        }
        bad.sort(); bad.dedup();
        assert!(bad.is_empty(), "{} problems, e.g. {:?} (closest to curve {closest_curve:.2}, closest discs {closest_disc:.2})", bad.len(), &bad[..bad.len().min(4)]);
    }

    #[test]
    fn no_leader_comes_near_another_leader_or_another_pointers_dot_in_the_whole_family() {
        // There is no leader-against-leader or leader-against-dot rule in the planner: each one was once there and a sweep
        // with it switched off showed it never changed a drawing, because a pointer's dot and its disc are already kept
        // clear of every earlier leader and dot, and a leader is kept off the curve and the arrow. This is the check that
        // that stays true, with literal gaps: leaders 3.6 px apart, a leader 6 px from every other dot
        let (mut checked, mut nearest_pair, mut nearest_dot) = (0, f64::MAX, f64::MAX);
        for set in sets() {
            for size in ["Small", "Medium", "Large"] {
                for fail in [false, true] {
                    for mask in 0..32u32 {
                        let mut l = set.clone();
                        l.extend(switches(mask, fail, true, size));
                        let p = with(&l);
                        if plan(&p).is_err() { continue; }
                        let svg = (def().render)(&p);
                        let ls: Vec<(P, P)> = lines_w(&svg).into_iter().filter(|q| q.1 == 1.2).map(|q| q.0).collect();
                        let dots: Vec<P> = circles(&svg).into_iter().filter(|c| c.2 < 6.0).map(|c| (c.0, c.1)).collect();
                        for (i, a) in ls.iter().enumerate() {
                            for c in &ls[i + 1..] { nearest_pair = nearest_pair.min(sseg(a.0, a.1, c.0, c.1)); }
                            for q in &dots { if dist2(*q, a.0) > 0.05 { nearest_dot = nearest_dot.min(dseg(*q, a.0, a.1)); } }
                        }
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 1500, "{checked}");
        assert!(nearest_pair >= 3.6, "two leaders {nearest_pair} px apart");
        assert!(nearest_dot >= 6.0, "a leader passes {nearest_dot} px from another pointer's dot");
    }

    #[test]
    fn two_segments_that_cross_are_zero_apart_and_parallel_ones_are_their_gap_apart() {
        // a leader crossing the arrow's shaft with both its ends far from it must still count as touching it
        assert_eq!(segs_dist((0.0, 0.0), (20.0, 0.0), (10.0, -30.0), (10.0, 30.0)), 0.0);
        assert!((segs_dist((0.0, 0.0), (20.0, 0.0), (0.0, 7.0), (20.0, 7.0)) - 7.0).abs() < 1e-9);
        assert!((segs_dist((0.0, 0.0), (10.0, 0.0), (13.0, 4.0), (30.0, 40.0)) - 5.0).abs() < 1e-9);
    }

    /// The number after a phrase in a refusal's advice, as typed.
    fn number_after(t: &str, key: &str) -> Option<String> {
        let rest = t.split(key).nth(1)?;
        let w = rest.split_whitespace().next()?;
        Some(w.trim_end_matches([',', '.', ';']).replace('\u{2212}', "-"))
    }
    /// A figure with its stimulus, total time, undershoot, peak or switches changed.
    fn changed(list: &[(&'static str, Value)], key: &'static str, v: Value) -> Params {
        let mut p = with(list);
        p.0.insert(key.into(), v);
        p
    }

    #[test]
    fn a_dip_too_shallow_for_the_return_pointer_is_refused_only_when_no_dot_fits_and_the_advice_works() {
        // The rule: the return dot sits halfway up the recovery, so at least 4 px off the rest line only if the dip is at
        // least 8 px deep. Walk the dip from 3 mV in 0.1 mV steps on a grid of scales: every refusal before the first dip
        // that is drawn says "too shallow" (and nothing else is refused), and that first dip is drawn with its return dot
        // only just 4 px off the rest line (within what one more 0.1 mV step adds), so nothing was refused that fits
        let mut refusals = 0;
        for (rest, peak) in [(-70.0f64, 40.0f64), (-70.0, 70.0), (-50.0, 30.0), (-95.0, 70.0), (-60.0, 50.0)] {
            for size in ["Small", "Large"] {
                let mut first_ok: Option<f64> = None;
                let mut deeper_said: Vec<f64> = Vec::new();
                let mut d = 30i64;
                while d <= 400 && first_ok.is_none() {
                    let under = rest - d as f64 / 10.0;
                    if under < -100.0 { break; }
                    let list = vec![s("resting", &rest.to_string()), s("threshold", &(rest + 15.0).to_string()), s("peak", &peak.to_string()), s("undershoot", &show(under)), s("label_size", size), b("show_refractory", true)];
                    match plan(&with(&list)) {
                        Ok(_) => first_ok = Some(d as f64 / 10.0),
                        Err(e) => {
                            refusals += 1;
                            assert!(e.why.contains("is too shallow at this scale") && e.why.contains("halfway up the recovery"), "{list:?}: {}", e.why);
                            let only: f64 = number_after(&e.why, "would be only ").unwrap().parse().unwrap();
                            assert!(only < 4.0, "{}", e.why);
                            // every piece of advice, applied, draws the figure
                            let opts: Vec<&str> = e.then.trim_end_matches('.').split(", or ").collect();
                            assert!(opts.last().unwrap().starts_with("turn the phase pointers off") || opts.last().unwrap().starts_with("Turn the phase pointers off"), "{}", e.then);
                            for o in &opts {
                                let o = o.to_lowercase();
                                if let Some(u) = o.strip_prefix("type a deeper undershoot, ").and_then(|t| t.strip_suffix(" mv")) { deeper_said.push(u.replace('\u{2212}', "-").parse().unwrap()); }
                                let q = if let Some(u) = o.strip_prefix("type a deeper undershoot, ").and_then(|t| t.strip_suffix(" mv")) { changed(&list, "undershoot", Value::S(u.replace('\u{2212}', "-"))) }
                                    else if let Some(pk) = o.strip_prefix("lower the peak to ").and_then(|t| t.strip_suffix(" mv")) { changed(&list, "peak", Value::S(pk.replace('\u{2212}', "-"))) }
                                    else if o.starts_with("turn the phase pointers off") { changed(&list, "show_phases", Value::B(false)) }
                                    else { panic!("advice not understood: {o}") };
                                assert!(plan(&q).is_ok(), "{list:?}: advice {o:?} does not work");
                            }
                        }
                    }
                    d += 1;
                }
                // "deeper" is the least undershoot that is drawn, to the tenth: the first dip the walk drew
                if let Some(d) = first_ok { for u in &deeper_said { assert!((*u - (rest - d)).abs() < 1e-6, "{rest}/{peak} {size}: advice says {u}, the first undershoot drawn is {}", rest - d); } }
                if let Some(d) = first_ok {
                    if d > 3.0 {
                        let under = rest - d;
                        let list = vec![s("resting", &rest.to_string()), s("threshold", &(rest + 15.0).to_string()), s("peak", &peak.to_string()), s("undershoot", &show(under)), s("label_size", size), b("show_refractory", true), b(SHOW_ANSWERS, true)];
                        let svg = render_of(&list);
                        let ax = axes_of(&svg);
                        let dots = pointer_dots(&svg);
                        let ret = dot_named(&dots, "Return to resting potential").unwrap();
                        let half = (ret.1 - ax.y(rest)).abs();
                        // one 0.1 mV step of the dip moves the halfway point by 0.05 mV: 0.05 * px per mV
                        assert!(half >= 4.0 - 1e-6 && half <= 4.0 + 0.05 * ax.ky.abs() * 1.3 + 0.1, "{rest}/{peak} {size}: the first dip drawn ({d} mV) has its return dot {half} px off rest, so the dip before it was refused wrongly");
                    }
                } else {
                    // no dip down to the floor fits at this scale: then nothing offered is a deeper undershoot
                    let list = vec![s("resting", &rest.to_string()), s("threshold", &(rest + 15.0).to_string()), s("peak", &peak.to_string()), s("undershoot", &show((rest - 3.0).max(-100.0))), s("label_size", size)];
                    if let Err(e) = plan(&with(&list)) { assert!(!e.then.contains("deeper undershoot"), "{}", e.then); }
                }
            }
        }
        assert!(refusals > 40, "{refusals}");
        // the dip is quoted as typed, not rounded
        let e = plan(&with(&[s("resting", "-95"), s("threshold", "-85"), s("peak", "70"), s("undershoot", "-98.0004")])).err().expect("refused");
        assert!(e.why.contains("(3.0004 mV below rest)"), "{}", e.why);
        // a dip that has room is drawn, whatever the scale of the others
        assert!(plan(&with(&[s("undershoot", "-75")])).is_ok());
        // rest -97: the floor is -100, so a deeper undershoot is not offered unless it works, and the other options do
        let near_floor = [s("resting", "-97"), s("threshold", "-87"), s("peak", "70"), s("undershoot", "-100")];
        let e = plan(&with(&near_floor)).err().expect("refused");
        assert!(e.why.contains("too shallow") && !e.then.contains("deeper undershoot"), "{} / {}", e.why, e.then);
        let pk = number_after(&e.then, "lower the peak to ");
        if let Some(pk) = pk { assert!(plan(&changed(&near_floor, "peak", Value::S(pk))).is_ok()); }
        assert!(plan(&changed(&near_floor, "show_phases", Value::B(false))).is_ok());
    }

    /// The plot width in px for these switches and size, read off a figure that is certainly drawn (the width does not depend
    /// on the stimulus or the total time).
    fn plot_width_px(size: &str, stim_on: bool) -> f64 {
        let svg = render_of(&[s("stimulus", "1"), s("duration", "8"), s("label_size", size), b("show_stimulus", stim_on), b(SHOW_ANSWERS, true)]);
        let ax = axes_of(&svg);
        ax.x(8.0) - ax.x(0.0)
    }
    /// The least lead (px of plot before the stimulus) the resting pointer needs, worked out here from literals: its disc
    /// radius (7.6 / 9.5 / 13.3 px), 12 px from the axis or the radius plus 4, and 8 px from the arrow (4 without it).
    fn lead_needed_px(size: &str, stim_on: bool) -> f64 {
        let r = 9.5 * match size { "Small" => 0.8, "Large" => 1.4, _ => 1.0 };
        (12.0f64).max(r + 4.0) + if stim_on { 8.0 } else { 4.0 }
    }

    #[test]
    fn the_resting_pointer_is_always_before_the_stimulus_on_the_resting_level_and_a_stimulus_with_no_room_is_refused() {
        // the typical-values grid (rest -70/-65/-60, threshold -55/-50, peak 30/40/50, undershoot -75/-80/-90), every
        // stimulus time from 0.5 ms to the last allowed, totals 5.2 to 12, all sizes, the stimulus arrow on and off
        let (mut drawn, mut refused) = (0, 0);
        let mut pre = [0usize; 3];
        for (rest, thr) in [(-70, -55), (-70, -50), (-65, -55), (-65, -50), (-60, -50)] {
            for peak in [30, 50] {
                for under in [-75, -90] {
                    for (si, size) in ["Small", "Medium", "Large"].iter().enumerate() {
                        for stim_on in [true, false] {
                            let w = plot_width_px(size, stim_on);
                            for ts in ["0.5", "0.6", "0.8", "1", "2", "4", "7.3"] {
                                for dur in ["5.2", "8", "10", "12"] {
                                    let tsv: f64 = ts.parse().unwrap();
                                    let durv: f64 = dur.parse().unwrap();
                                    if tsv + 4.7 > durv + 1e-9 { continue; }
                                    for fail in [false, true] {
                                        let list = vec![
                                            s("resting", &rest.to_string()), s("threshold", &thr.to_string()), s("peak", &peak.to_string()), s("undershoot", &under.to_string()),
                                            s("stimulus", ts), s("duration", dur), s("label_size", size), b("show_stimulus", stim_on), b("fail", fail), b(SHOW_ANSWERS, true),
                                        ];
                                        let p = with(&list);
                                        let lead = tsv / durv * w;
                                        let need = lead_needed_px(size, stim_on);
                                        match plan(&p) {
                                            Err(e) => {
                                                refused += 1;
                                                // either no dot fits, or the dot fits and its disc and leader do not (guide lines close
                                                // above the resting level and the arrow below), which needs far less lead than three discs
                                                assert!(e.why.contains("leaves only") && (lead < need + 0.3 || e.why.contains("not for its disc and leader") && lead < 3.0 * need), "{list:?}: {}", e.why);
                                            }
                                            Ok(_) => {
                                                drawn += 1;
                                                assert!(lead >= need - 0.3, "{list:?}: lead {lead} px is under the {need} px the pointer needs, and it was drawn");
                                                let svg = (def().render)(&p);
                                                let ax = axes_of(&svg);
                                                let dots = pointer_dots(&svg);
                                                let rd = dot_named(&dots, "Resting potential").expect("a resting dot");
                                                // on the resting level, left of the stimulus (and its arrow), clear of the y axis, and not on the spike
                                                assert!((rd.1 - ax.y(rest as f64)).abs() < 0.5, "{list:?}: resting dot {rd:?} off the resting level");
                                                assert!(rd.0 < ax.x(tsv) - if stim_on { 8.0 } else { 3.0 } && rd.0 - ax.x(0.0) >= 12.0, "{list:?}: resting dot at {} px, stimulus at {}", rd.0, ax.x(tsv));
                                                pre[si] += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(drawn > 2500 && refused >= 20, "{drawn} drawn, {refused} refused");
        assert!(pre.iter().all(|n| *n > 800), "{pre:?}");
        // the cases the critic named: 0.5 ms in 8 ms is drawn at every size; 0.5 in 12 is refused exactly where the lead
        // is under what the pointer needs (Large), and is drawn, with its dot before the arrow, where it is not
        for size in ["Small", "Medium", "Large"] {
            assert!(plan(&with(&[s("stimulus", "0.5"), s("duration", "8"), s("label_size", size)])).is_ok(), "0.5/8 {size}");
            let r = plan(&with(&[s("stimulus", "0.5"), s("duration", "12"), s("label_size", size)]));
            if 0.5 / 12.0 * plot_width_px(size, true) < lead_needed_px(size, true) - 0.3 {
                let e = r.err().expect("0.5/12 refused");
                assert!(e.why.contains("leaves only") && e.why.contains("flat line before the stimulus"), "{}", e.why);
            } else { assert!(r.is_ok(), "0.5/12 {size}"); }
            // with the phase pointers off there is no resting pointer to place, so nothing is refused
            assert!(plan(&with(&[s("stimulus", "0.5"), s("duration", "12"), s("label_size", size), b("show_phases", false)])).is_ok());
        }
        assert!(plan(&with(&[s("stimulus", "0.5"), s("duration", "12"), s("label_size", "Large")])).is_err(), "Large needs 25.4 px and has 21.7");
    }

    #[test]
    fn the_advice_for_a_stimulus_with_no_room_is_the_least_that_works_and_is_applied() {
        // the default scale (a dot that does not fit) and a crowded one (rest -30: the 0 mV line is close above the threshold,
        // so the disc has no room above the dot, and the arrow leaves none below)
        let bases: [Vec<(&'static str, Value)>; 2] = [vec![], vec![s("resting", "-30"), s("threshold", "-15"), s("peak", "40"), s("undershoot", "-60")]];
        let (mut checked, mut crowded) = (0, 0);
        for (bi, base) in bases.iter().enumerate() {
            let rd = |ts: i64, d: i64, size: &str| {
                let mut l = base.clone();
                l.extend([s("stimulus", &tenth(ts)), s("duration", &tenth(d)), s("label_size", size)]);
                plan(&with(&l))
            };
            for size in ["Small", "Medium", "Large"] {
                for ts in 5..=73i64 {
                    for d in (ts + 47)..=120 {
                        let Err(e) = rd(ts, d, size) else { continue };
                        if !e.why.contains("leaves only") { continue; }
                        if e.why.contains("not for its disc and leader") { crowded += 1; }
                        let opts: Vec<String> = e.then.trim_end_matches('.').split(", or ").map(|o| o.to_lowercase()).collect();
                        assert!(opts.last().unwrap().starts_with("turn the phase pointers off"), "{}", e.then);
                        // a later stimulus: offered exactly when some later stimulus works, and it is the least one
                        let later = (ts + 1..=(d - 47)).find(|t| rd(*t, d, size).is_ok());
                        let said_later = opts.iter().find_map(|o| o.strip_prefix("move the stimulus to ").and_then(|t| t.strip_suffix(" ms or later")).map(|t| (t.parse::<f64>().unwrap() * 10.0).round() as i64));
                        assert_eq!(said_later, later, "base {bi} {size} {ts}/{d}: {}", e.then);
                        // a shorter total: offered exactly when some shorter total works, and it is the longest one
                        let shorter = ((ts + 47)..d).rev().find(|d2| rd(ts, *d2, size).is_ok());
                        let said_shorter = opts.iter().find_map(|o| o.strip_prefix("shorten the total time to ").and_then(|t| t.strip_suffix(" ms or less")).map(|t| (t.parse::<f64>().unwrap() * 10.0).round() as i64));
                        assert_eq!(said_shorter, shorter, "base {bi} {size} {ts}/{d}: {}", e.then);
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked >= 8 && crowded >= 3, "{checked} checked, {crowded} crowded");
    }

    #[test]
    fn no_two_dots_of_a_failed_initiation_are_close_and_the_local_response_is_never_on_the_falling_side() {
        let mut checked = 0;
        for set in sets() {
            for size in ["Small", "Medium", "Large"] {
                for mask in [31u32, 7, 15, 5] {
                    let mut l = set.clone();
                    l.extend(switches(mask, true, true, size));
                    let p = with(&l);
                    if plan(&p).is_err() { continue; }
                    let svg = (def().render)(&p);
                    let dots = pointer_dots(&svg);
                    if dots.len() < 3 { continue; }
                    for (i, (n, a)) in dots.iter().enumerate() {
                        for (m, c) in &dots[i + 1..] { assert!(dist2(*a, *c) >= 24.0, "{set:?} {size} mask {mask}: dots for {n} and {m} are {} px apart", dist2(*a, *c)); }
                    }
                    if let Some(local) = dot_named(&dots, NAME_LOCAL) {
                        // the top of the bump: the curve's highest point; the local dot is there or on the rising side
                        let top = paths(&svg).0.into_iter().min_by(|a, c| a.1.partial_cmp(&c.1).unwrap()).unwrap();
                        assert!(local.0 <= top.0 + 3.0, "{set:?} {size}: the local-response dot at {} is past the top of the bump at {}", local.0, top.0);
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked > 150, "{checked}");
    }

    /// Everything in a whole SVG that could carry a word: every attribute name and value, and every comment. With the answers
    /// off these may hold only drawing vocabulary: numbers, path data, colours, and the few keywords below.
    fn markup_leaks(svg: &str) -> Vec<String> {
        let mut out = Vec::new();
        if svg.contains("<!--") { out.push("a comment".to_string()); }
        const NAMES: [&str; 31] = [
            "xmlns", "width", "height", "viewBox", "role", "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "d", "fill", "stroke", "stroke-width", "stroke-dasharray",
            "stroke-linecap", "stroke-linejoin", "font-size", "font-family", "font-weight", "text-anchor", "transform", "points", "id", "dy", "patternUnits", "patternTransform",
        ];
        const WORDS: [&str; 15] = ["img", "grid", "none", "middle", "start", "end", "round", "butt", "700", "translate", "rotate", "http://www.w3.org/2000/svg", "100%", "square", "userSpaceOnUse"];
        // every `name="value"` inside a tag
        let mut rest = svg;
        while let Some(i) = rest.find('<') {
            let tag = &rest[i + 1..];
            let end = tag.find('>').unwrap_or(tag.len());
            let inner = &tag[..end];
            let mut it = inner;
            while let Some(eq) = it.find("=\"") {
                let name = it[..eq].rsplit(|c: char| c.is_whitespace()).next().unwrap_or("");
                let after = &it[eq + 2..];
                let close = after.find('"').unwrap_or(after.len());
                let value = &after[..close];
                if !NAMES.contains(&name) { out.push(format!("attribute {name}")); }
                else if name == "id" { if value != "grid" && !(value.starts_with("vs-hatch") && value[8..].chars().all(|c| c.is_ascii_digit())) { out.push(format!("id {value}")); } }
                else if name == "font-family" { if !value.chars().all(|c| c.is_ascii_alphabetic() || " ,-'".contains(c)) { out.push(format!("font-family {value}")); } }
                else if value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit()) {}
                else {
                    for w in value.split(|c: char| !(c.is_ascii_alphabetic() || c == '/' || c == ':' || c == '.' || c == '%' || c.is_ascii_digit())).filter(|w| w.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 3) {
                        // hex colours and path letters have no three-letter word besides the keywords
                        if !WORDS.contains(&w) && !w.chars().all(|c| "MLHVZmlhvz".contains(c) || c.is_ascii_digit() || c == '.') { out.push(format!("{name}={value}")); break; }
                    }
                }
                it = &after[(close + 1).min(after.len())..];
            }
            rest = &tag[end..];
        }
        out
    }

    #[test]
    fn a_student_copy_has_no_word_in_any_attribute_id_class_or_comment() {
        let mut checked = 0;
        for set in sets() {
            for fail in [false, true] {
                for size in ["Small", "Large"] {
                    for style in ["Colour", "Print"] {
                        for mask in [0u32, 21, 31] {
                            let mut l = set.clone();
                            l.extend(switches(mask, fail, false, size));
                            l.push(s("style", style));
                            let p = with(&l);
                            if plan(&p).is_err() { continue; }
                            assert_eq!(markup_leaks(&(def().render)(&p)), Vec::<String>::new(), "{l:?}");
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked > 300, "{checked}");
        // the check is real: each of these, on a failed canvas with the answers off, is seen
        let svg = render_of(&[b(SHOW_ANSWERS, false), b("fail", true)]);
        assert_eq!(markup_leaks(&svg.replacen("role=\"img\"", "role=\"img\" aria-label=\"graded potential only\"", 1)), vec!["attribute aria-label".to_string()]);
        assert_eq!(markup_leaks(&svg.replacen("role=\"img\"", "role=\"img\" data-kind=\"subthreshold\"", 1)), vec!["attribute data-kind".to_string()]);
        assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect class=\"no-spike\" ", 1)), vec!["attribute class".to_string()]);
        assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect id=\"failed\" ", 1)), vec!["id failed".to_string()]);
        assert_eq!(markup_leaks(&svg.replacen("<rect ", "<!-- no spike here --><rect ", 1)), vec!["a comment".to_string()]);
        assert_eq!(markup_leaks(&svg.replacen("fill=\"#fff\"", "fill=\"subthreshold\"", 1)), vec!["fill=subthreshold".to_string()]);
    }

    #[test]
    fn a_comma_needs_digits_on_both_sides_and_a_number_too_long_to_quote_is_refused_not_clipped() {
        // a comma with nothing on one side is not a number, with or without a sign, a unit or spaces
        for bad in ["-,5", ",5", "-70,", "70,", "-70, mV", "+,5", "-,", ",", "-70,,5", "-7,0,5"] {
            let e = refusal(&[s("resting", bad)]);
            assert!(e.why.contains("is not a number") || e.why.contains("is ambiguous"), "{bad}: {}", e.why);
        }
        // ...and the ordinary ones still read
        for good in ["-70,5", "-70.5", "\u{2212}70,5 mV", "-70", "-70 mV"] {
            let r = read(&with(&[s("resting", good), s("threshold", "-55"), s("peak", "40"), s("undershoot", "-80")]));
            assert!(r.is_ok(), "{good}: {:?}", r.err().map(|e| e.why));
        }
        // more digits than the exact reading holds (12 whole, 9 decimal) are refused and say so; nothing is rounded
        // or saturated into a number the message then misquotes
        for (bad, says) in [
            ("1234567890123", "too large to read"),
            ("-1234567890123456789012345", "too large to read"),
            ("-70.1234567891", "more than 9 decimal places"),
            ("0.0000000001", "more than 9 decimal places"),
        ] {
            let e = refusal(&[s("stimulus", bad)]);
            assert!(e.why.contains(says), "{bad}: {}", e.why);
            assert!(!e.why.contains("999999999") && !e.why.contains("1234567890124"), "{bad}: {}", e.why);
        }
        // exactly at the limit is read exactly: 12 whole digits and 9 decimals are quoted back as typed
        let e = refusal(&[s("stimulus", "100000000000.123456789")]);
        assert!(e.why.contains("100000000000.123456789"), "{}", e.why);
        // trailing zeros past the 9th decimal are the same number, not too many digits
        assert!(read(&with(&[s("stimulus", "1.0000000000000")])).is_ok());
    }

    #[test]
    fn the_timing_advice_is_possible_at_every_boundary_and_names_only_what_helps() {
        let base = with(&[]);
        let rd = |ts: i64, d: i64| { let mut p = base.clone(); p.0.insert("stimulus".into(), Value::S(tenth(ts))); p.0.insert("duration".into(), Value::S(tenth(d))); read(&p) };
        // the number after a phrase, to the tenth
        let after = |t: &str, key: &str| -> Option<i64> {
            let rest = t.split(key).nth(1)?;
            let w = rest.split_whitespace().next()?;
            let v: f64 = w.trim_end_matches(['.', ',', ';']).parse().ok()?;
            Some((v * 10.0).round() as i64)
        };
        let mut checked = 0;
        for ts in (5..=150).step_by(2) {
            for d in (1..=130).step_by(2) {
                let Err(e) = rd(ts, d) else { continue };
                if !e.why.contains("needs about") { continue; }
                let fix_total = (52..=120).any(|d2| rd(ts, d2).is_ok());
                let fix_stim = (5..=73).any(|t2| rd(t2, d).is_ok());
                let says_total = e.then.contains("total time at least");
                let says_stim = e.then.contains("stimulus to");
                if !fix_total && !fix_stim {
                    assert!(e.then.starts_with("Type a stimulus time of 0.5 to 7.3 ms"), "{ts} {d}: {}", e.then);
                } else {
                    assert_eq!((says_total, says_stim), (fix_total, fix_stim), "{} / {}: {ts} {d}", e.why, e.then);
                    // and the number it gives works
                    if says_total { let t = after(&e.then, "total time at least ").unwrap(); assert!(rd(ts, t).is_ok(), "total {t} for {ts}: {}", e.then); assert!(t <= 120, "{}", e.then); }
                    if says_stim { let t = after(&e.then, "stimulus to ").unwrap(); assert!(rd(t, d).is_ok(), "stimulus {t} for {d}: {}", e.then); assert!(t >= 5, "{}", e.then); }
                }
                checked += 1;
            }
        }
        assert!(checked > 2000, "{checked}");
        // the "not a time" refusal suggests a total that the same stimulus accepts, at every stimulus allowed
        let mut said = 0;
        for ts in 5..=73 {
            for bad in ["0", "-5", "-0.1", "-100"] {
                let mut p = base.clone();
                p.0.insert("stimulus".into(), Value::S(tenth(ts)));
                p.0.insert("duration".into(), Value::S(bad.into()));
                let e = read(&p).expect_err("not a time");
                assert!(e.why.contains("is not a time"), "{}", e.why);
                let t = after(&e.then, "total time of ").unwrap_or_else(|| panic!("{ts} {bad}: {}", e.then));
                assert!(t <= 120 && rd(ts, t).is_ok(), "stimulus {ts}: it suggests {t}, which is refused: {}", e.then);
                // and it is the least that works
                assert!(rd(ts, t - 1).is_err(), "stimulus {ts}: {t} is not the least: {}", e.then);
                said += 1;
            }
        }
        assert_eq!(said, 69 * 4);
    }

    // ------------------------------------------------------------------ the axes: ticks, grid, range
    #[test]
    fn minor_ticks_and_a_light_grid_make_every_level_readable_at_every_label_size() {
        let mut worst_span: f64 = 0.0;
        for set in sets() {
            for size in ["Small", "Medium", "Large"] {
                let mut l = set.clone();
                l.push(s("label_size", size));
                let svg = render_of(&l);
                let ax = axes_of(&svg);
                let ls = lines(&svg);
                let x0 = ls.iter().filter(|q| q.0 .0 == q.1 .0).map(|q| q.0 .0).fold(f64::MAX, f64::min); // the y axis
                // labelled ticks: 6 px; unlabelled: 3.5 px, on the same axis, between them
                let major: Vec<f64> = ls.iter().filter(|q| q.0 .1 == q.1 .1 && q.1 .0 == x0 && (q.1 .0 - q.0 .0 - 6.0).abs() < 0.01).map(|q| q.0 .1).collect();
                let minor: Vec<f64> = ls.iter().filter(|q| q.0 .1 == q.1 .1 && q.1 .0 == x0 && (q.1 .0 - q.0 .0 - 3.5).abs() < 0.01).map(|q| q.0 .1).collect();
                assert!(major.len() >= 3 && minor.len() >= 2 * major.len(), "{set:?} {size}: {} labelled, {} unlabelled", major.len(), minor.len());
                // between two labelled ticks there are 3 or 4 unlabelled ones: 4 or 5 divisions
                let gap = (major[1] - major[0]).abs();
                let between = minor.iter().filter(|y| **y > major[0].min(major[1]) && **y < major[0].max(major[1])).count();
                assert!(between == 3 || between == 4, "{set:?} {size}: {between} between labelled ticks");
                // a division is at most 10 mV and at least 4 px: a level is read to within about half a division
                let division_px = gap / (between + 1) as f64;
                let division_mv = division_px / ax.ky.abs();
                assert!(division_mv <= 10.01 && division_px >= 3.9, "{set:?} {size}: division {division_mv} mV = {division_px} px");
                // the grid has a line at every tick level, inside the plot, drawn first
                let g = grid_lines(&svg);
                assert_eq!(g.len(), major.len() + minor.len(), "{set:?} {size}");
                for (xs, y, _) in &g {
                    assert!(xs.0 == x0 && (major.iter().chain(minor.iter()).any(|t| (t - y).abs() < 0.01)), "grid line off a tick: {y}");
                }
                assert!(svg.find("<g id=\"grid\">").unwrap() < svg.find("<path").unwrap(), "grid under the curve");
                // the axis does not run on past what the picture needs: never twice the span of the trace
                let span = 320.0 / ax.ky.abs();
                let low = sp_of(&set).under.min(sp_of(&set).rest);
                worst_span = worst_span.max(span / (sp_of(&set).peak - low));
                assert!(span <= 2.0 * (sp_of(&set).peak - low) + 1e-6, "{set:?} {size}: axis spans {span} mV for a trace of {}", sp_of(&set).peak - low);
            }
        }
        assert!(worst_span < 2.0, "{worst_span}");
    }

    fn sp_of(set: &[(&'static str, Value)]) -> Spike { read(&with(set)).unwrap() }

    #[test]
    fn a_large_label_size_does_not_double_the_range() {
        // the critic's case: -100 / -90 / 70, no dip, 0.5 ms into a 5.2 ms trace, Large. It used to draw -200 to 150.
        let l = [s("resting", "-100"), s("threshold", "-90"), s("peak", "70"), s("undershoot", "-100"), s("stimulus", "0.5"), s("duration", "5.2"), s("label_size", "Large")];
        let svg = render_of(&l);
        let ax = axes_of(&svg);
        let span = 320.0 / ax.ky.abs();
        let lowest_label = texts(&svg).iter().filter(|t| t.anchor == "end" && !t.rot).filter_map(|t| t.s.replace('\u{2212}', "-").parse::<f64>().ok()).fold(f64::MAX, f64::min);
        assert!(span < 290.0 && lowest_label >= -150.0, "span {span}, lowest label {lowest_label}");
        // the labelled step stays a 1-2-5 step that is not wider than the unlabelled ones need
        assert!(audit(&svg).is_empty());
    }

    #[test]
    fn each_bracket_row_has_a_short_label_that_names_no_answer() {
        let svg = render_of(&[b(SHOW_ANSWERS, false), b("show_refractory", true)]);
        let ax = axes_of(&svg);
        let tx = texts(&svg);
        let find = |w: &str| tx.iter().find(|t| t.s == w).unwrap_or_else(|| panic!("{w} missing"));
        let (ch, rf) = (find("Channels"), find("Periods"));
        let x_axis = ax.x(0.0);
        for t in [ch, rf] { assert_eq!(t.anchor, "end"); assert!(t.x < x_axis - 5.0 && t.x - tw(&t.s, t.size) > 0.0, "{} sits left of the plot, inside the canvas", t.s); }
        assert!(ch.y < rf.y, "rows in order");
        // alone, the row labels name nothing: no ion, no period, no phase. Each is true of both brackets in its row
        // (Na+ and K+ are both channels; the absolute and the relative are both periods), so neither bracket has two
        // defensible names, and nothing says "refractory", "absolute" or "relative"
        for t in [ch, rf] { for n in NAMES { assert!(!t.s.to_lowercase().contains(n)); } }
        for w in ["refractory", "absolute", "relative", "na+", "k+", "sodium", "potassium"] { assert!(!joined(&svg).to_lowercase().contains(w), "{w}"); }
        // a lane that is off has no label
        let one = joined(&render_of(&[b("show_refractory", false)]));
        assert!(one.contains("Channels") && !one.contains("Periods"));
        let only_rf = joined(&render_of(&[b("show_channels", false), b("show_refractory", true)]));
        assert!(!only_rf.contains("Channels") && only_rf.contains("Periods"));
        let none = joined(&render_of(&[b("show_channels", false)]));
        assert!(!none.contains("Channels") && !none.contains("Periods"));
        // the label of the first row is level with the discs of its row
        let ds: Vec<_> = circles(&svg).into_iter().filter(|c| c.2 >= 6.0).collect();
        let row_disc = ds.iter().filter(|d| d.1 > ch.y - 6.0 && d.1 < ch.y + 6.0).count();
        assert!(row_disc >= 2, "the discs of the channel row share its line");
    }

    /// The dot each named pointer starts from, found through the key row's mark and the figure's disc with the same mark
    /// and its leader (answers on, so the key names every pointer).
    fn pointer_dots(svg: &str) -> Vec<(String, P)> {
        let tx = texts(svg);
        let ds: Vec<_> = circles(svg).into_iter().filter(|c| c.2 >= 6.0).collect();
        let leaders: Vec<(P, P)> = lines_w(svg).into_iter().filter(|q| q.1 == 1.2).map(|q| q.0).collect();
        let marked: Vec<((f64, f64, f64), String)> = ds.iter().filter_map(|d| tx.iter().find(|t| dist2((t.x, t.y), (d.0, d.1)) < d.2).map(|t| (*d, t.s.clone()))).collect();
        let mut out = Vec::new();
        for name in ["Stimulus", "Resting potential", "Threshold", "Depolarisation", "Peak", "Repolarisation", "Hyperpolarisation (undershoot)", "Return to resting potential", "Local (graded) response"] {
            let Some(row) = tx.iter().find(|t| t.s == name) else { continue };
            let Some((kd, mark)) = marked.iter().filter(|(d, _)| (d.1 - row.y).abs() < 9.0 && d.0 < row.x).max_by(|a, c| a.0 .0.partial_cmp(&c.0 .0).unwrap()) else { continue };
            let Some((fd, _)) = marked.iter().find(|(d, m)| m == mark && d.1 < kd.1 - 20.0) else { continue };
            if let Some(q) = leaders.iter().find(|q| (dist2(q.1, (fd.0, fd.1)) - fd.2).abs() < 0.8) { out.push((name.to_string(), q.0)); }
        }
        out
    }
    fn dot_named(dots: &[(String, P)], name: &str) -> Option<P> { dots.iter().find(|d| d.0 == name).map(|d| d.1) }

    #[test]
    fn resting_and_return_point_at_different_places_in_every_trace() {
        // with the key on, find the dot each named pointer starts from, through its leader
        for set in sets() {
            for size in ["Small", "Medium", "Large"] {
                for fail in [false, true] {
                    let mut l = set.clone();
                    l.extend(switches(31, fail, true, size));
                    let p = with(&l);
                    let sp = read(&p).unwrap();
                    let svg = (def().render)(&p);
                    let ax = axes_of(&svg);
                    let dots = pointer_dots(&svg);
                    let rest = dot_named(&dots, "Resting potential").expect("a dot for the resting pointer");
                    // the resting pointer is ALWAYS on the resting level before the stimulus: at least 12 px from the y axis
                    assert!((rest.1 - ax.y(sp.rest)).abs() < 0.5, "{set:?} {size} fail {fail}: resting dot {rest:?} is off the rest line");
                    assert!(rest.0 < ax.x(sp.ts) && rest.0 - ax.x(0.0) >= 12.0, "{set:?} {size} fail {fail}: resting dot {rest:?} is not before the stimulus at {}", ax.x(sp.ts));
                    if let Some(ret) = dot_named(&dots, "Return to resting potential") {
                        // literal minimum separations, in px, from the rest line and from every other dot
                        let up = (ret.1 - ax.y(sp.rest)).abs();
                        for (n, d) in &dots { if *d != ret { assert!(dist2(ret, *d) >= 24.0, "{set:?} {size} fail {fail}: the return dot is {} px from the dot for {n}", dist2(ret, *d)); } }
                        // it is on the curve itself
                        let t = (ret.0 - ax.x(0.0)) / (ax.x(1.0) - ax.x(0.0));
                        assert!((ret.1 - ax.y(sp.v(t))).abs() < 0.5, "{set:?}: return dot is off the curve at {t} ms");
                        if fail {
                            let top = dot_named(&dots, NAME_LOCAL).expect("a dot for the local response");
                            // the height of the bump, read off the drawn curve itself
                            let bump = ax.y(sp.rest) - paths(&svg).0.iter().map(|p| p.1).fold(f64::MAX, f64::min);
                            assert!(up >= 2.5 && up <= 0.45 * bump, "{set:?} {size}: the return dot is {up} px above rest on a bump {bump} px high");
                            assert!(ret.0 > top.0 + 10.0 && ret.1 > top.1 + 0.1 * bump && up <= 0.45 * bump, "{set:?} {size}: the return dot {ret:?} is beside the top of the bump {top:?}");
                        } else {
                            assert!(sp.dips());
                            assert!(up >= 4.0, "{set:?} {size}: the return dot is {up} px from the rest line");
                            // halfway up the recovery: the dot is half of the dip's depth above the lowest point
                            let trough = paths(&svg).0.iter().map(|p| p.1).fold(f64::MIN, f64::max);
                            let share = (trough - ret.1) / (trough - ax.y(sp.rest));
                            assert!((share - 0.5).abs() < 0.02, "{set:?} {size}: the return dot is {share} of the dip above the lowest point");
                            // inside the K+ bracket and the relative refractory bracket, after the lowest point
                            assert!(ret.0 > ax.x(sp.t_low()) && ret.0 < ax.x(sp.t_settled()), "{set:?} {size}: return dot at {} outside the recovery {}..{}", ret.0, ax.x(sp.t_low()), ax.x(sp.t_settled()));
                            assert!(ret.0 < ax.x(sp.dur) - 1.0, "{set:?}");
                        }
                    } else {
                        assert!(!fail && !sp.dips(), "{set:?}: a trace with a dip, or a failure, keeps its return pointer");
                    }
                }
            }
        }
    }

    #[test]
    fn the_faq_states_the_limits_the_code_enforces() {
        let all: String = crate::figures::faq::faq_for(ID).iter().map(|q| q.a).collect::<Vec<_>>().join(" ");
        let ascii = |x: f64| show(x).replace('\u{2212}', "-");
        for want in [
            format!("from {} to +{} mV", ascii(MV_MIN), ascii(MV_MAX)),
            format!("from {} to {} mV and the threshold from {} to {} mV", ascii(MV_MIN), ascii(REST_MAX), ascii(MV_MIN), ascii(-MIN_THR_TO_ZERO)),
            format!("at least {} mV above the resting potential", ascii(MIN_REST_TO_THR)),
            format!("at least {} mV above the threshold", ascii(MIN_THR_TO_PEAK)),
            format!("at least {} mV below it", ascii(MIN_DIP)),
            format!("plus {} ms, and at most {} ms", ascii(NEED), ascii(MAX_DUR)),
            format!("within {} mV of the resting level", ascii(REST_TOL)),
            "divided into 4 or 5 parts".to_string(),
            // round 3: the row labels, where the two pointers sit, and the one refusal a very shallow dip can meet
            "(Channels, Periods)".to_string(),
            format!("at least {} px off the resting line", ascii(RETURN_SEP_V)),
            "never after the spike, and the plot is never widened for it".to_string(),
            "halfway up the recovery (half of the dip's depth above the lowest point)".to_string(),
            "at least 24 px from every other dot".to_string(),
            "a stimulus at 0.5 ms in a 12 ms trace is refused at the Large label size".to_string(),
            "very shallow undershoot on a wide scale".to_string(),
        ] {
            assert!(all.contains(&want), "the FAQ does not say {want:?}");
        }
        // what it says about reading precision is what the picture draws: 4 or 5 divisions, none wider than 10 mV
        // (checked against the ticks in `minor_ticks_and_a_light_grid_make_every_level_readable_at_every_label_size`)
        assert!(all.contains("2 to 5 mV"));
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
        // the description of a failed initiation says nothing of the failure, with or without the key
        for key in [false, true] {
            let svg = def().svg(&with(&[b(SHOW_ANSWERS, key), b("fail", true)]));
            assert_eq!(disclosure_in(&svg), None);
        }
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
            ("refused-above-zero".into(), vec![s("threshold", "12"), s("peak", "60")]),
            ("refused-timing".into(), vec![s("stimulus", "8"), s("duration", "12")]),
            ("large-extreme".into(), vec![s("resting", "-100"), s("threshold", "-90"), s("peak", "70"), s("undershoot", "-100"), s("stimulus", "0.5"), s("duration", "5.2"), s("label_size", "Large"), b("show_refractory", true)]),
            ("large-extreme-student".into(), vec![s("resting", "-100"), s("threshold", "-90"), s("peak", "70"), s("undershoot", "-100"), s("stimulus", "0.5"), s("duration", "5.2"), s("label_size", "Large"), b("show_refractory", true), b(SHOW_ANSWERS, false)]),
            ("early-long".into(), vec![s("stimulus", "0.5"), s("duration", "12"), b("show_refractory", true)]),
            ("no-dip-print".into(), vec![s("undershoot", "-70"), s("style", "Print"), b("show_refractory", true)]),
            ("deep-dip".into(), vec![s("resting", "-14"), s("threshold", "-4"), s("peak", "40"), s("undershoot", "-100"), b("show_refractory", true)]),
        ];
        for (tag, list) in [
            ("m-0.5-8-student", vec![s("stimulus", "0.5"), s("duration", "8"), b(SHOW_ANSWERS, false), b("show_refractory", true)]),
            ("m-0.5-8", vec![s("stimulus", "0.5"), s("duration", "8"), b("show_refractory", true)]),
            ("m-0.5-7", vec![s("stimulus", "0.5"), s("duration", "7"), b("show_refractory", true)]),
            ("l-0.5-12-refused", vec![s("stimulus", "0.5"), s("duration", "12"), s("label_size", "Large")]),
            ("l-0.5-8", vec![s("stimulus", "0.5"), s("duration", "8"), s("label_size", "Large"), b("show_refractory", true)]),
            ("shallow94-large", vec![s("resting", "-90"), s("threshold", "-80"), s("peak", "70"), s("undershoot", "-94"), s("label_size", "Large"), b("show_refractory", true)]),
            ("shallow-refused", vec![s("resting", "-95"), s("threshold", "-85"), s("peak", "70"), s("undershoot", "-98")]),
            ("crowded-refused", vec![s("resting", "-30"), s("threshold", "-15"), s("peak", "40"), s("undershoot", "-60"), s("stimulus", "0.5"), s("duration", "12")]),
            ("failed-wide-large", vec![b("fail", true), s("resting", "-14"), s("threshold", "-4"), s("peak", "70"), s("undershoot", "-100"), s("stimulus", "0.8"), s("duration", "12"), s("label_size", "Large")]),
            ("print-m-0.5-8-student", vec![s("stimulus", "0.5"), s("duration", "8"), s("style", "Print"), b(SHOW_ANSWERS, false), b("show_refractory", true)]),
        ] { cases.push((tag.to_string(), list)); }
        for size in ["Small", "Medium", "Large"] {
            cases.push((format!("early12-{size}"), vec![s("stimulus", "0.5"), s("duration", "12"), s("label_size", size), b("show_refractory", true)]));
            cases.push((format!("early12-{size}-student"), vec![s("stimulus", "0.5"), s("duration", "12"), s("label_size", size), b("show_refractory", true), b(SHOW_ANSWERS, false)]));
            cases.push((format!("early5-{size}"), vec![s("stimulus", "0.5"), s("duration", "5.2"), s("label_size", size), b("show_refractory", true)]));
            cases.push((format!("shallow-{size}"), vec![s("undershoot", "-73"), s("label_size", size), b("show_refractory", true)]));
            cases.push((format!("lateret-{size}"), vec![s("resting", "-14"), s("threshold", "-4"), s("peak", "70"), s("undershoot", "-100"), s("stimulus", "7.3"), s("duration", "12"), s("label_size", size), b("show_refractory", true)]));
            cases.push((format!("failed-late-{size}"), vec![b("fail", true), s("stimulus", "7.3"), s("duration", "12"), s("label_size", size)]));
            cases.push((format!("failed-tight-{size}"), vec![b("fail", true), s("resting", "-100"), s("threshold", "-90"), s("peak", "70"), s("undershoot", "-100"), s("label_size", size)]));
        }
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
