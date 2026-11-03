//! Sine and cosine as a graph: one to two cycles drawn on a degree axis, with the amplitude, the period and the
//! midline marked, and the equation printed. Drawn with `svg::Doc` only.
//!
//! The wave is y = A sin(B(x - C)) + D (or cos), with the x axis in degrees, so B = 360 / period. Where it starts
//! (at the midline going up, at a peak, and so on) is a preset, and the phase shift C is a typed number of degrees.
//! Every measured value (amplitude, period, midline) and the equation itself has a state: Given (printed on both
//! copies), Ask (a blank on the student copy, the value on the key) or Hidden.
use super::chart::{fmt, nice_step, ticks, Frame};
use super::circle::{num, tw, Rect};
use super::svg::{self, Doc};
use super::*;

type Pt = (f64, f64);

pub const ID: &str = "sine-cosine";
const INK: &str = "#111111";
/// Print is neutral grey only; the screen uses the same accent as the other Math figures.
const CURVE_COLOUR: &str = "#1f5fbf";
const CURVE_PRINT: &str = "#111111";
const GRID_COLOUR: &str = "#e2e6ee";
const GRID_PRINT: &str = "#787878"; // dark enough to survive a 66% bitonal copy at the 1.3x raster the tests use
const MARK_COLOUR: &str = "#7a4fc4";
const MARK_PRINT: &str = "#555555";
const BLANK: &str = "_____";
/// Canvas and plot area (px).
const W: f64 = 520.0;
const L: f64 = 50.0;
const R: f64 = 26.0;
const T: f64 = 18.0;
const PLOT_B: f64 = 236.0;
const ROW_Y0: f64 = 252.0;
const ROW_H: f64 = 17.0;
const ROW_FS: f64 = 15.0;

const STARTS: [&str; 4] = ["Midline, going up", "Peak", "Midline, going down", "Trough"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum V { Amp, Period, Mid, Eq }

/// (value, state key, label, default state).
const VALS: [(V, &str, &str, &str); 4] = [
    (V::Amp, "g_amp", "Amplitude", "Given"),
    (V::Period, "g_per", "Period", "Ask"),
    (V::Mid, "g_mid", "Midline", "Given"),
    (V::Eq, "g_eq", "Equation", "Ask"),
];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Sine and cosine",
        blurb: "A sine or cosine wave on a degree axis, with the amplitude, the period and the midline marked and the equation written from them.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("start", "The wave starts at the", &STARTS, STARTS[0])
            .hint("This sets the function: midline going up is sin, a peak is cos, and the other two are their negatives. The phase shift moves where that start sits"),
        ParamDef::float("amp", "Amplitude A", 0.1, 10.0, 0.1, 1.0),
        ParamDef::float("period", "Period (degrees per cycle)", 36.0, 1080.0, 1.0, 360.0),
        ParamDef::float("mid", "Midline y =", -5.0, 5.0, 0.1, 0.0),
        ParamDef::float("shift", "Phase shift (degrees)", 0.0, 360.0, 15.0, 0.0)
            .hint("How far the start is moved to the right. A shift of a whole cycle or more is folded into one cycle"),
        ParamDef::int("dec", "Decimal places", 0, 2, 1),
    ];
    for e in VALS {
        v.push(ParamDef::choice(e.1, e.2, &["Given", "Ask", "Hidden"], e.3).hint("Given is printed on the student copy; Ask is blank there and filled on the key; Hidden shows no value"));
    }
    v.push(answers_param());
    v.push(svg::style_param());
    v.push(svg::label_size_param());
    v
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(x: f64, lo: f64, hi: f64, d: f64) -> f64 {
    if x.is_finite() { x.clamp(lo, hi) } else { d }
}

#[derive(Clone, Copy, Debug)]
pub struct Wave {
    /// Amplitude, never negative (the sign lives in `start`).
    pub a: f64,
    /// Degrees for one full cycle.
    pub period: f64,
    pub mid: f64,
    /// Phase shift in degrees, folded into `[0, period)`.
    pub shift: f64,
    /// Index into `STARTS`.
    pub start: usize,
    /// The typed shift was a whole cycle or more and has been folded.
    pub wrapped: bool,
}

impl Wave {
    /// B, the number of cycles across 360 degrees.
    pub fn b(&self) -> f64 { 360.0 / self.period }
    /// The wave's value at `x` (degrees).
    pub fn y(&self, x: f64) -> f64 {
        let u = (self.b() * (x - self.shift)).to_radians();
        let s = match self.start { 0 => u.sin(), 1 => u.cos(), 2 => -u.sin(), _ => -u.cos() };
        self.mid + self.a * s
    }
    /// Where the first maximum at or after `x0` sits (degrees).
    pub fn first_peak(&self, x0: f64) -> f64 {
        let off = match self.start { 0 => 0.25, 1 => 0.0, 2 => 0.75, _ => 0.5 } * self.period;
        let base = self.shift + off;
        base + ((x0 - base) / self.period).ceil() * self.period
    }
    /// The equation as printed. A and D carry at least one decimal (their controls step by 0.1), and B is printed
    /// at four places so that 360 / B round-trips to the exact period: printed at the readout precision, B
    /// disagreed with the Period row and with the curve as drawn (360/1080 printed as 0.3 implies 1200).
    pub fn equation(&self, dec: i64) -> String {
        let (ad, bd) = (dec.max(1), 4);
        let (sign, func) = match self.start { 0 => ("", "sin"), 1 => ("", "cos"), 2 => ("\u{2212}", "sin"), _ => ("\u{2212}", "cos") };
        let mut s = format!("y = {sign}{} {func}({}(x \u{2212} {}))", num(self.a, ad), num(self.b(), bd), num(self.shift, ad));
        if self.mid.abs() > 1e-9 {
            s.push_str(&format!(" {} {}", if self.mid > 0.0 { "+" } else { "\u{2212}" }, num(self.mid.abs(), ad)));
        }
        s
    }
}

pub fn solve(p: &Params) -> Wave {
    let a = fin(p.f("amp"), 0.1, 10.0, 1.0);
    let period = fin(p.f("period"), 36.0, 1080.0, 360.0);
    let mid = fin(p.f("mid"), -5.0, 5.0, 0.0);
    let raw = fin(p.f("shift"), 0.0, 360.0, 0.0);
    let shift = raw.rem_euclid(period);
    let start = STARTS.iter().position(|s| *s == p.s("start")).unwrap_or(0);
    Wave { a, period, mid, shift, start, wrapped: (shift - raw).abs() > 1e-9 }
}

fn mode(p: &Params, v: V) -> &str {
    VALS.iter().find(|e| e.0 == v).map(|e| p.s(e.1)).unwrap_or("Given")
}

pub fn warnings(p: &Params) -> Vec<String> {
    let w = solve(p);
    let mut out = Vec::new();
    if w.wrapped {
        out.push(format!("The phase shift is a whole cycle or more (one cycle is {} degrees), so it is drawn shifted by {}.", fmt(w.period), fmt(w.shift)));
    }
    out
}

/// The readout rows: (value, key text, student text).
fn rows(w: &Wave, dec: i64) -> [(V, String, String); 4] {
    let ad = dec.max(1);
    let (pa, pp, pm, pe) = (num(w.a, ad), format!("{}\u{b0}", fmt(w.period)), format!("y = {}", num(w.mid, ad)), w.equation(dec));
    [
        (V::Amp, format!("Amplitude  A = {pa}"), format!("Amplitude  A = {BLANK}")),
        (V::Period, format!("Period  P = {pp}"), format!("Period  P = {BLANK}\u{b0}")),
        (V::Mid, format!("Midline  {pm}"), format!("Midline  y = {BLANK}")),
        (V::Eq, format!("Equation  {pe}"), format!("Equation  y = {BLANK}")),
    ]
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, halo: bool, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}"{}>{}</text>"##,
        if halo { r##" stroke="#fff" stroke-width="2" paint-order="stroke" stroke-linejoin="round""## } else { "" },
        svg::esc(s)
    )
}

fn text_rect(x: f64, y: f64, size: f64, anchor: &str, s: &str) -> Rect {
    let width = tw(s, size, false);
    let (x0, x1) = match anchor { "end" => (x - width, x), "middle" => (x - width / 2.0, x + width / 2.0), _ => (x, x + width) };
    Rect { x0, y0: y - size, x1, y1: y + size * 0.3 }
}

/// Geometry a test can check without reading the SVG.
pub struct Report {
    /// The first peak, in data coordinates.
    pub peak: Pt,
    /// The last point the curve is drawn to (degrees).
    pub x1: f64,
    /// The boxes of the marker labels, in pixels.
    pub labels: Vec<Rect>,
}

pub fn build(p: &Params) -> (String, Report) {
    let w = solve(p);
    let dec = p.i("dec").clamp(0, 2);
    // the amplitude and the midline step by 0.1, so they never print fewer than one decimal
    let ad = dec.max(1);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let f = svg::label_factor(p.s("label_size"));

    let x1 = (w.shift + w.first_peak(0.0) + 2.0 * w.period).max(w.period);
    let pad = 0.18 * w.a;
    let (y0, y1) = (w.mid - w.a - pad, w.mid + w.a + pad);
    let frame = Frame { w: W, h: PLOT_B, l: L, r: R, t: T, b: 40.0, x0: 0.0, x1, y0, y1 };

    // the readout grows with the label size, so the rows never ride on each other
    let row_h = ROW_H * f;
    let mut d = Doc::new(W, ROW_Y0 + 4.0 * row_h + 4.0);
    d.describe("Sine and cosine", "A sine or cosine wave on a degree axis with its amplitude, period and midline marked");

    // grid and axes
    let sx = nice_step(x1, 7.0);
    let sy = nice_step(y1 - y0, 4.0);
    let (xt, yt) = (ticks(0.0, x1, sx), ticks(y0, y1, sy));
    let (grid, ink) = (if print { GRID_PRINT } else { GRID_COLOUR }, if print { "#555555" } else { INK });
    for (v, _) in &xt { let x = frame.px(*v); d.line(x, frame.top(), x, frame.bottom(), grid, 1.0); }
    for (v, _) in &yt { let y = frame.py(*v); d.line(frame.left(), y, frame.right(), y, grid, 1.0); }
    let x_axis_y = if y0 <= 0.0 && 0.0 <= y1 { frame.py(0.0) } else { frame.bottom() };
    d.line(frame.left(), x_axis_y, frame.right(), x_axis_y, ink, 1.8);
    d.line(frame.left(), frame.top(), frame.left(), frame.bottom(), ink, 1.8);
    for (v, label) in &xt {
        let x = frame.px(*v);
        d.line(x, frame.bottom(), x, frame.bottom() + 5.0, ink, 1.3);
        if !label.is_empty() { super::chart::text(&mut d, x, frame.bottom() + 21.0, 13.0, "middle", label); }
    }
    for (v, label) in &yt {
        let y = frame.py(*v);
        d.line(frame.left() - 5.0, y, frame.left(), y, ink, 1.3);
        if !label.is_empty() { super::chart::text(&mut d, frame.left() - 9.0, y + 4.5, 13.0, "end", label); }
    }
    super::chart::text(&mut d, (frame.left() + frame.right()) / 2.0, frame.bottom() + 34.0, 14.0, "middle", "x (degrees)");

    // the wave
    let n = 600;
    let pts: Vec<Pt> = (0..=n).map(|i| { let x = frame.x0 + (frame.x1 - frame.x0) * i as f64 / n as f64; (frame.px(x), frame.py(w.y(x))) }).collect();
    d.polyline(&pts, if print { CURVE_PRINT } else { CURVE_COLOUR }, if print { 2.6 } else { 3.0 });

    // the markers: midline, amplitude at the first peak, period between the first two peaks. A cosine that starts
    // at a peak puts that peak on the y-axis, where a marker drawn over the axis reads as part of it, so the next
    // peak is used when it leaves room for both arrows.
    let mcol = if print { MARK_PRINT } else { MARK_COLOUR };
    let mut xp = w.first_peak(0.0);
    // the axis covers two periods past the first peak, so the next peak is always inside the frame
    if frame.px(xp) < frame.left() + 12.0 && xp + 2.0 * w.period <= frame.x1 + 1e-9 {
        xp += w.period;
    }
    let xp2 = xp + w.period;
    let peak_y = frame.py(w.mid + w.a);
    let mid_y = frame.py(w.mid);
    d.raw(&format!(
        r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{mcol}" stroke-width="1.6" stroke-dasharray="7 5"/>"##,
        frame.left(), mid_y, frame.right(), mid_y
    ));
    let ax = frame.px(xp);
    d.arrow_with(ax, mid_y, ax, peak_y, mcol, 1.6, "", true);
    let (px1, px2) = (frame.px(xp), frame.px(xp2));
    d.arrow_with(px1, peak_y, px2, peak_y, mcol, 1.6, "", true);

    let label = |v: V, text: String, blank: String| -> String {
        match mode(p, v) {
            "Hidden" => String::new(),
            "Ask" if !ans => blank,
            _ => text,
        }
    };
    let amp_label = label(V::Amp, format!("amplitude = {}", num(w.a, ad)), format!("amplitude = {BLANK}"));
    let per_label = label(V::Period, format!("period = {}\u{b0}", fmt(w.period)), format!("period = {BLANK}\u{b0}"));
    let mid_label = label(V::Mid, format!("midline  y = {}", num(w.mid, ad)), format!("midline  y = {BLANK}"));

    let mut placed: Vec<Rect> = Vec::new();
    let mut put = |x: f64, y: f64, anchor: &str, s: &str| {
        if s.is_empty() { return; }
        let size = 13.0 * f;
        d.raw(&text_el(x, y, size, anchor, true, mcol, s));
        placed.push(text_rect(x, y, size, anchor, s));
    };
    // amplitude: beside its arrow, half way up (to the left when the peak is near the right edge)
    let amp_x = if ax + 8.0 + 110.0 < frame.right() { ax + 8.0 } else { ax - 8.0 };
    put(amp_x, (mid_y + peak_y) / 2.0 + 4.0, if amp_x < ax { "end" } else { "start" }, &amp_label);
    // period: above the middle of its arrow
    put((px1 + px2) / 2.0, peak_y - 8.0, "middle", &per_label);
    // midline: at the right end, above the line
    put(frame.right() - 2.0, mid_y - 6.0, "end", &mid_label);

    // the readout. The equation is a composite of A, B and D: printing it while any of those is Ask or Hidden
    // would spell out the answer on the student copy, so it is blanked unless its parts are all Given.
    let parts_given = mode(p, V::Amp) == "Given" && mode(p, V::Period) == "Given" && mode(p, V::Mid) == "Given";
    for (i, (v, key, stu)) in rows(&w, dec).into_iter().enumerate() {
        let line = match mode(p, v) {
            "Hidden" => continue,
            _ if v == V::Eq && !ans && !parts_given => stu,
            "Ask" if !ans => stu,
            _ => key,
        };
        super::chart::text(&mut d, L, ROW_Y0 + i as f64 * row_h, ROW_FS * f, "start", &line);
    }

    (d.finish(), Report { peak: (xp, w.mid + w.a), x1, labels: placed })
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let st = |g: [&str; 4]| -> Vec<(&'static str, Value)> { VALS.iter().zip(g).map(|(e, m)| (e.1, Value::S(m.to_string()))).collect() };
    let ex = |name: &'static str, mut v: Vec<(&'static str, Value)>, g: [&str; 4]| { v.extend(st(g)); Example { name, params: v } };
    // order of the states: amplitude, period, midline, equation
    Some(vec![
        ex("y = sin x: read the amplitude, the period and the midline", vec![s("start", STARTS[0]), f("amp", 1.0), f("period", 360.0)], ["Given", "Ask", "Given", "Ask"]),
        ex("y = 3 sin 2x: a taller wave, twice as fast", vec![s("start", STARTS[0]), f("amp", 3.0), f("period", 180.0)], ["Given", "Ask", "Given", "Ask"]),
        ex("y = cos x: it starts at a peak", vec![s("start", STARTS[1]), f("amp", 1.0), f("period", 360.0)], ["Given", "Given", "Given", "Ask"]),
        ex("y = 2 cos x - 1: a midline below the axis", vec![s("start", STARTS[1]), f("amp", 2.0), f("period", 360.0), f("mid", -1.0)], ["Given", "Given", "Ask", "Ask"]),
        ex("Write the equation: shifted 90 degrees", vec![s("start", STARTS[0]), f("amp", 1.5), f("period", 360.0), f("shift", 90.0)], ["Given", "Given", "Given", "Ask"]),
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
    fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }

    fn texts(svg: &str) -> String {
        let mut out = String::new();
        let mut rest = svg;
        while let Some(i) = rest.find("<text") {
            rest = &rest[i..];
            let Some(j) = rest.find('>') else { break };
            let body = &rest[j + 1..];
            let Some(k) = body.find("</text>") else { break };
            if !out.is_empty() { out.push('\n'); }
            out.push_str(&body[..k]);
            rest = &body[k..];
        }
        out
    }

    #[test]
    fn defaults_are_a_sine_with_its_three_markers() {
        let p = with(&[]);
        let w = solve(&p);
        assert_eq!((w.a, w.period, w.mid, w.start), (1.0, 360.0, 0.0, 0));
        let t = texts(&render_of(&[]));
        assert!(t.contains("Amplitude  A = 1"), "{t}");
        assert!(t.contains("Period  P = 360\u{b0}"), "{t}");
        assert!(t.contains("Midline  y = 0"), "{t}");
        assert!(t.contains("Equation  y = 1 sin(1(x \u{2212} 0))"), "{t}");
        assert!(t.contains("amplitude = 1") && t.contains("period = 360\u{b0}") && t.contains("midline  y = 0"), "{t}");
    }

    #[test]
    fn the_wave_is_the_equation_it_prints() {
        for start in STARTS {
            for (amp, period, mid, shift) in [(1.0, 360.0, 0.0, 0.0), (3.0, 180.0, -1.0, 45.0), (0.5, 720.0, 2.0, 90.0)] {
                let w = solve(&with(&[s("start", start), f("amp", amp), f("period", period), f("mid", mid), f("shift", shift)]));
                let xp = w.first_peak(0.0);
                assert!((w.y(xp) - (mid + amp)).abs() < 1e-6, "{start} {amp} {period} {mid} {shift}: peak {}", w.y(xp));
                assert!((w.y(xp + period) - (mid + amp)).abs() < 1e-6, "{start}: the next peak");
                assert!((w.b() * period - 360.0).abs() < 1e-9);
                // the printed equation holds at its own shift, folded
                assert!((w.y(w.shift) - mid).abs() < 1e-6 || w.start % 2 == 1, "{start}: the start point");
            }
        }
    }

    #[test]
    fn the_axis_covers_two_peaks_so_the_period_arrow_fits() {
        for start in STARTS {
            for (period, shift) in [(360.0, 0.0), (36.0, 0.0), (1080.0, 0.0), (180.0, 150.0), (90.0, 85.0)] {
                let w = solve(&with(&[s("start", start), f("period", period), f("shift", shift)]));
                let r = build(&with(&[s("start", start), f("period", period), f("shift", shift)])).1;
                assert!(!r.labels.is_empty(), "{start} {period} {shift}: the marker labels");
                let xp = w.first_peak(0.0);
                assert!(xp + w.period <= r.x1 + 1e-9, "{start} {period} {shift}: the second peak is past the axis");
            }
        }
    }

    #[test]
    fn an_asked_value_is_blank_on_the_student_copy_and_shown_on_the_key() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Period  P = _____"), "{stu}");
        assert!(stu.contains("Equation  y = _____"), "{stu}");
        assert!(stu.contains("Amplitude  A = 1"), "a Given value stays: {stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Period  P = 360\u{b0}"), "{key}");
        assert!(key.contains("Equation  y = 1 sin(1(x \u{2212} 0))"), "{key}");
    }

    #[test]
    fn an_asked_marker_label_is_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[s("g_per", "Ask"), b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("period = _____\u{b0}"), "the marker label is blank too: {stu}");
        assert!(!stu.contains("period = 360"), "{stu}");
    }

    #[test]
    fn hidden_values_are_not_drawn_at_all() {
        let t = texts(&render_of(&[s("g_per", "Hidden"), s("g_mid", "Hidden"), s("g_amp", "Hidden")]));
        assert!(!t.to_lowercase().contains("period"), "{t}");
        assert!(!t.to_lowercase().contains("amplitude"), "{t}");
        assert!(!t.to_lowercase().contains("midline"), "{t}");
    }

    #[test]
    fn a_shift_of_a_whole_cycle_is_folded_and_noted() {
        let w = solve(&with(&[f("period", 180.0), f("shift", 180.0)]));
        assert_eq!(w.shift, 0.0);
        assert!(w.wrapped);
        assert!(warnings(&with(&[f("period", 180.0), f("shift", 180.0)])).iter().any(|s| s.contains("whole cycle")));
        assert!(warnings(&with(&[f("period", 360.0), f("shift", 45.0)])).is_empty());
    }

    /// Writes every example (Colour, Print, student, key, student Print, Print at a 66% bitonal threshold) to $VERSHION_PNG_DIR.
    #[test]
    fn sincos_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        cases.push(("tall".into(), vec![f("amp", 9.5), f("mid", 4.0), f("period", 1080.0)]));
        cases.push(("fast".into(), vec![f("period", 36.0), s("label_size", "Large")]));
        cases.push(("shift".into(), vec![f("shift", 270.0), s("start", STARTS[3]), f("mid", -2.0)]));
        cases.push(("narrow".into(), vec![f("amp", 0.1), ("dec", Value::I(2))]));
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)]), ("student-print", vec![s("style", "Print"), b(SHOW_ANSWERS, false)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/sincos-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/sincos-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }

    /// A Given equation is a composite of A, B and D: when any of those is Ask, the student copy must not print it.
    #[test]
    fn a_given_equation_never_spells_out_an_asked_part() {
        let l = [s("g_amp", "Ask"), s("g_per", "Ask"), s("g_mid", "Ask"), s("g_eq", "Given"), b(SHOW_ANSWERS, false), f("amp", 3.0), f("mid", 2.0)];
        let t = texts(&render_of(&l));
        assert!(!t.contains("3 sin") && !t.contains("+ 2)"), "the equation spells out an asked amplitude or midline: {t}");
        assert!(t.contains("Equation  y = _____"), "{t}");
        // the key still prints it
        let k = texts(&render_of(&[s("g_amp", "Ask"), s("g_eq", "Given"), b(SHOW_ANSWERS, true)]));
        assert!(k.contains("Equation  y = "), "{k}");
    }

    /// The printed B must round-trip to the period printed beside it, or the equation describes another wave.
    #[test]
    fn the_printed_equation_reproduces_its_own_period() {
        for period in [36.0, 180.0, 270.0, 360.0, 541.0, 1080.0] {
            for dec in 0..=2i64 {
                let w = solve(&with(&[f("period", period), ("dec", Value::I(dec))]));
                let b: f64 = num(360.0 / w.period, 4).replace('\u{2212}', "-").parse().unwrap();
                assert!((360.0 / b - period).abs() < 0.5, "period {period} at dec {dec}: B prints as {b}, which means {}", 360.0 / b);
            }
        }
    }

    #[test]
    fn print_is_neutral_grey_only() {
        let svg = render_of(&[s("style", "Print")]);
        for line in svg.lines().filter(|l| l.contains("stroke=\"#")) {
            for part in line.split("stroke=\"#").skip(1) {
                let hex = &part[..6.min(part.len())];
                if hex.len() < 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) { continue; }
                let (r, g, b) = (u8::from_str_radix(&hex[0..2], 16).unwrap(), u8::from_str_radix(&hex[2..4], 16).unwrap(), u8::from_str_radix(&hex[4..6], 16).unwrap());
                assert!(r == g && g == b, "Print draws a hue: {hex} in {line}");
            }
        }
        assert!(render_of(&[]).contains(CURVE_COLOUR), "the screen copy keeps its colour");
    }
}
