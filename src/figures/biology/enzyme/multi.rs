//! Rate against temperature or pH, and rate against substrate with more than one curve: up to three curves on one
//! graph, each a named enzyme (or an unknown one with its own optimum and width) or, against substrate, an enzyme with
//! no inhibitor, a competitive one or a noncompetitive one; named in a key, with the optimum and the denaturation
//! marked by dots, or Vmax and Km by dashed lines, each label written, left as a blank line or left off.
//!
//! The single Michaelis-Menten curve with its marked point stays in the parent module, untouched: every saved figure
//! from before these controls existed is drawn there, byte for byte as before.
use super::*;
use std::f64::consts::FRAC_PI_2;

/// The height every temperature and pH curve peaks at, in arbitrary units.
pub const PEAK: f64 = 10.0;
/// A label left for the student to write: a line to write on.
pub const BLANK_LINE: &str = "__________";
const GREEN: &str = "#1e7a46";
const LETTERS: [char; 3] = ['A', 'B', 'C'];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Against {
    Substrate,
    Temperature,
    Ph,
}

pub fn against(p: &Params) -> Against {
    match p.s("against") {
        "Temperature" => Against::Temperature,
        "pH" => Against::Ph,
        _ => Against::Substrate,
    }
}

/// Whether this module draws the figure: anything but the single Michaelis-Menten curve.
pub fn is_multi(p: &Params) -> bool { against(p) != Against::Substrate || p.b("curve2") || p.b("curve3") }

/// A named enzyme's activity profile: the temperature it works best at, the temperature by which it has denatured
/// (the rate is zero from there on), how fast the rate falls away below the optimum, and its pH optimum and the pH
/// distance either side at which it has stopped working.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    pub t_opt: f64,
    pub t_zero: f64,
    pub t_rise: f64,
    pub ph_opt: f64,
    pub ph_half: f64,
}

/// Human enzymes work best at body temperature, about 37 °C, and are denatured some 10 °C above it (the usual school
/// figure; the rise width puts the rate at 0 °C near 7 % of the peak, roughly a Q10 of 2 over 37 °C).
const BODY: (f64, f64, f64) = (37.0, 47.0, 19.0);

/// The profile of a named enzyme; None for the unknown enzyme (its numbers are typed).
pub fn profile(name: &str) -> Option<Profile> {
    let (t_opt, t_zero, t_rise) = BODY;
    let body = |ph_opt: f64, ph_half: f64| Profile { t_opt, t_zero, t_rise, ph_opt, ph_half };
    Some(match name {
        // a typical human enzyme of the blood and cytoplasm: pH about 7
        "Human enzyme" => body(7.0, 3.0),
        // Taq DNA polymerase from Thermus aquaticus: optimum 75-80 °C (Chien, Edgar and Trela, J. Bacteriol. 127,
        // 1976; Lawyer et al., PCR Methods Appl. 2, 1993). It survives the 95 °C step of PCR (half-life about 40 min,
        // Lawyer et al.), so its rate is still about a tenth of the peak at 95 °C and reaches zero only at 100 °C.
        // Its PCR buffer is Tris-HCl at pH 8.3.
        "Taq polymerase" => Profile { t_opt: 75.0, t_zero: 100.0, t_rise: 18.0, ph_opt: 8.3, ph_half: 2.5 },
        // pepsin in the stomach: optimum pH about 1.5-2 (Piper and Fenton, Gut 6, 1965), inactive above about pH 5
        "Pepsin" => body(2.0, 2.5),
        // salivary amylase: optimum pH 6.7-7.0, inactivated by stomach acid below about pH 4
        "Salivary amylase" => body(6.8, 3.0),
        // trypsin in the small intestine: optimum pH about 8 (7.8-8.7)
        "Trypsin" => body(8.0, 3.0),
        // catalase: a broad optimum near pH 7, active from about pH 4 to 10
        "Catalase" => body(7.0, 4.0),
        _ => return None,
    })
}

pub const ENZYMES: [&str; 7] = ["Human enzyme", "Taq polymerase", "Pepsin", "Salivary amylase", "Trypsin", "Catalase", "Unknown enzyme"];
pub const MODES: [&str; 3] = ["none", "competitive", "noncompetitive"];
pub const LABEL_MODES: [&str; 4] = ["Name and value", "Name only", "Blank line", "None"];
pub const TITLE_MODES: [&str; 3] = ["Text", "Blank line", "None"];

/// The rate against temperature, as a fraction of the peak: a rounded rise to the optimum (exp(-(d/w)^1.5), flat at
/// the top and falling away like an exponential far below it), then a steep fall to zero where the enzyme has
/// denatured (cos², so the curve leaves the peak smoothly and meets the axis without a corner).
pub fn temp_rate(t: f64, opt: f64, zero: f64, rise: f64) -> f64 {
    if t <= opt {
        (-((opt - t) / rise.max(1e-6)).powf(1.5)).exp()
    } else if t >= zero {
        0.0
    } else {
        let u = (t - opt) / (zero - opt).max(1e-6);
        (FRAC_PI_2 * u).cos().powi(2)
    }
}

/// The rate against pH, as a fraction of the peak: a symmetric bell (cos²) that is zero `half` pH units either side
/// of the optimum, where the enzyme's active site has been changed too far to work.
pub fn ph_rate(x: f64, opt: f64, half: f64) -> f64 {
    let u = (x - opt) / half.max(1e-6);
    if u.abs() >= 1.0 { 0.0 } else { (FRAC_PI_2 * u).cos().powi(2) }
}

/// One curve on the graph.
#[derive(Clone, Debug)]
pub struct Curve {
    /// Its slot: 0, 1 or 2 (curve A, B or C in the panel).
    pub slot: usize,
    pub name: String,
    /// Temperature or pH: the optimum, the denaturation point(s) (pH has one below and one above) and the rise.
    pub opt: f64,
    pub lo: f64,
    pub hi: f64,
    pub rise: f64,
    /// Substrate: the constants in force with this curve's inhibitor.
    pub vmax: f64,
    pub km: f64,
    pub mode: &'static str,
}

impl Curve {
    pub fn letter(&self) -> char { LETTERS[self.slot] }
    /// The rate at `x` (°C, pH or [S]), in the units of the rate axis.
    pub fn rate(&self, a: Against, x: f64) -> f64 {
        match a {
            Against::Temperature => PEAK * temp_rate(x, self.opt, self.hi, self.rise),
            Against::Ph => PEAK * ph_rate(x, self.opt, (self.hi - self.opt).max(1e-6)),
            Against::Substrate => rate(self.vmax, self.km, x),
        }
    }
}

fn mode_of(s: &str) -> &'static str {
    match s {
        "competitive" => "competitive",
        "noncompetitive" => "noncompetitive",
        _ => "none",
    }
}

/// Typed text with its runs of spaces and line breaks made single spaces.
pub fn tidy_text(s: &str) -> String { s.split_whitespace().collect::<Vec<_>>().join(" ") }

fn clean_name(s: &str) -> String {
    let t = tidy_text(s);
    if t.chars().count() <= 40 { t } else { format!("{}\u{2026}", t.chars().take(39).collect::<String>().trim_end()) }
}

/// The curves drawn, in slot order: curve A always, B and C when switched on.
pub fn curves(p: &Params) -> Vec<Curve> {
    let a = against(p);
    let (vmax, km, _, _, _, _) = given(p);
    // every inhibited curve shares the one inhibitor strength 1 + [I]/Ki (given() reports 1 when curve A has none)
    let factor = 1.0 + fin(p.f("inh"), 0.0, 10.0, 3.0) / fin(p.f("ki"), 0.2, 10.0, 1.0);
    let keys: [(&str, &str, &str, &str, &str, &str, &str, &str); 3] = [
        ("", "enz1", "mode", "name1", "uopt_t1", "uwid_t1", "uopt_ph1", "uwid_ph1"),
        ("curve2", "enz2", "mode2", "name2", "uopt_t2", "uwid_t2", "uopt_ph2", "uwid_ph2"),
        ("curve3", "enz3", "mode3", "name3", "uopt_t3", "uwid_t3", "uopt_ph3", "uwid_ph3"),
    ];
    let mut out = Vec::new();
    for (slot, (on, enz, mode_k, name_k, ot, wt, op, wp)) in keys.iter().enumerate() {
        if !on.is_empty() && !p.b(on) { continue; }
        let typed = clean_name(p.s(name_k));
        let mut c = Curve { slot, name: String::new(), opt: 0.0, lo: f64::NEG_INFINITY, hi: 0.0, rise: 1.0, vmax, km, mode: "none" };
        match a {
            Against::Substrate => {
                let mode = mode_of(p.s(mode_k));
                c.mode = mode;
                match mode {
                    "competitive" => c.km = km * factor,
                    "noncompetitive" => c.vmax = vmax / factor,
                    _ => {}
                }
                c.name = match mode {
                    "competitive" => "Competitive inhibitor",
                    "noncompetitive" => "Noncompetitive inhibitor",
                    _ => "No inhibitor",
                }
                .to_string();
            }
            Against::Temperature | Against::Ph => {
                let e = p.s(enz);
                let prof = profile(e).unwrap_or_else(|| {
                    let t_opt = fin(p.f(ot), 0.0, 100.0, 45.0);
                    let t_w = fin(p.f(wt), 2.0, 40.0, 10.0);
                    Profile { t_opt, t_zero: t_opt + t_w, t_rise: 1.9 * t_w, ph_opt: fin(p.f(op), 0.0, 14.0, 7.0), ph_half: fin(p.f(wp), 0.5, 7.0, 3.0) }
                });
                c.name = if ENZYMES.contains(&e) { e.to_string() } else { ENZYMES[0].to_string() };
                if a == Against::Temperature {
                    c.opt = prof.t_opt;
                    c.hi = prof.t_zero;
                    c.rise = prof.t_rise;
                } else {
                    c.opt = prof.ph_opt;
                    c.lo = prof.ph_opt - prof.ph_half;
                    c.hi = prof.ph_opt + prof.ph_half;
                }
            }
        }
        if !typed.is_empty() { c.name = typed; }
        out.push(c);
    }
    out
}

/// The x range drawn: temperature from 0 °C to a little past the last denaturation, pH 0 to 14, substrate to the
/// typed highest [S].
pub fn x_range(p: &Params, cs: &[Curve]) -> (f64, f64) {
    match against(p) {
        Against::Temperature => {
            let hi = cs.iter().map(|c| c.hi).fold(0.0, f64::max);
            (0.0, ((hi + 8.0) / 10.0).ceil() * 10.0)
        }
        Against::Ph => (0.0, 14.0),
        Against::Substrate => (0.0, given(p).2),
    }
}

/// A temperature or pH value as the axis shows it: "37 °C", "pH 2".
pub fn x_value(a: Against, v: f64) -> String {
    match a {
        Against::Temperature => format!("{} \u{b0}C", num(v, 1)),
        Against::Ph => format!("pH {}", num(v, 1)),
        Against::Substrate => num(v, 2),
    }
}

/// What a mark's label shows: a written text, a blank line, or nothing.
#[derive(Clone, Debug, PartialEq)]
pub enum Label {
    Text(String),
    Blank,
    Off,
}

/// The label of a mark called `name` whose value is `value`, for label mode `mode`. On the student copy (answers off)
/// a value is never printed: "Name and value" keeps the name and blanks the value, and "Name only" becomes a blank line
/// for the student to name the mark.
pub fn mark_label(mode: &str, name: &str, value: &str, sep: &str, ans: bool) -> Label {
    match mode {
        "None" => Label::Off,
        "Blank line" => Label::Blank,
        "Name only" => if ans { Label::Text(name.to_string()) } else { Label::Blank },
        _ => {
            if ans { Label::Text(format!("{name}{sep}{value}")) } else {
                // "pH ____" and "____ °C": the unit stays where it is printed
                let blank = if let Some(rest) = value.strip_prefix("pH ") { let _ = rest; format!("pH {BLANK}") }
                    else if value.ends_with(" \u{b0}C") { format!("{BLANK} \u{b0}C") }
                    else { BLANK.to_string() };
                Label::Text(format!("{name}{sep}{blank}"))
            }
        }
    }
}

fn axis_title(p: &Params, key_style: &str, key_text: &str, auto: &str) -> Label {
    match p.s(key_style) {
        "None" => Label::Off,
        "Blank line" => Label::Blank,
        _ => {
            let t = tidy_text(p.s(key_text));
            Label::Text(if t.is_empty() { auto.to_string() } else { t.chars().take(80).collect() })
        }
    }
}

/// The x and y axis titles and the chart title, as the panel sets them (shared with the single-curve figure).
pub fn titles(p: &Params) -> (Label, Label, Label) {
    let a = against(p);
    let numbered = p.s("axes") != "Shape only";
    let x_auto = match a {
        Against::Substrate => "Substrate concentration [S]",
        Against::Temperature => "Temperature (\u{b0}C)",
        Against::Ph => "pH",
    };
    let y_auto = match a {
        Against::Substrate => "Reaction rate v",
        _ if numbered => "Rate of reaction (arbitrary units)",
        _ => "Rate of reaction",
    };
    let chart = match p.s("title_style") {
        "None" => Label::Off,
        "Blank line" => Label::Blank,
        _ => {
            let t = tidy_text(p.s("title"));
            if t.is_empty() { Label::Off } else { Label::Text(t.chars().take(80).collect()) }
        }
    };
    (axis_title(p, "xtitle_style", "xtitle", x_auto), axis_title(p, "ytitle_style", "ytitle", y_auto), chart)
}

/// Colour and dash of curve `slot`: solid, dashed, dotted (Print: the same dashes in dark grey).
pub fn curve_style(slot: usize, print: bool) -> (&'static str, &'static str) {
    let col = if print { CURVE_PRINT } else { [CURVE, MARK, GREEN][slot % 3] };
    (col, ["", "9 6", "1 5"][slot % 3])
}

fn poly_el(pts: &[(f64, f64)], col: &str, dash: &str, w: f64) -> String {
    let pts: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
    let dash = if dash.is_empty() { String::new() } else { format!(r##" stroke-dasharray="{dash}""##) };
    format!(r##"<polyline points="{}" fill="none" stroke="{col}" stroke-width="{w}"{dash} stroke-linejoin="round" stroke-linecap="round"/>"##, pts.join(" "))
}

/// Arrowheads at the x axis's right end and the y axis's top end.
pub fn axis_arrows(d: &mut Doc, f: &Frame, ink: &str) {
    let (r, b, l, t) = (f.right(), f.bottom(), f.left(), f.top());
    d.line(r, b, r + 6.0, b, ink, 1.6);
    d.poly(&[(r + 14.0, b), (r + 4.0, b - 5.0), (r + 4.0, b + 5.0)], ink, ink, 1.0);
    d.line(l, t, l, t - 6.0, ink, 1.6);
    d.poly(&[(l, t - 14.0), (l - 5.0, t - 4.0), (l + 5.0, t - 4.0)], ink, ink, 1.0);
}

/// Splits typed text into lines of at most `per` characters, at spaces where it can and through a word only when
/// the word alone is longer than a line.
pub fn split_lines(s: &str, per: usize) -> Vec<String> {
    let per = per.max(4);
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let mut w: Vec<char> = word.chars().collect();
        while w.len() > per {
            if !cur.is_empty() { out.push(std::mem::take(&mut cur)); }
            out.push(w[..per].iter().collect());
            w.drain(..per);
        }
        let w: String = w.into_iter().collect();
        if w.is_empty() { continue; }
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > per { out.push(std::mem::take(&mut cur)); }
        if !cur.is_empty() { cur.push(' '); }
        cur.push_str(&w);
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

/// The lines a title takes in `room` px at font size `size` (`em`: the width of a character in ems, 0.55 regular,
/// 0.6 bold). A title that fits is one line, unchanged.
pub fn title_lines(t: &Label, room: f64, size: f64, em: f64) -> Vec<String> {
    match t {
        Label::Text(s) => {
            let per = (room / (size * em)).floor().max(4.0) as usize;
            if s.chars().count() <= per { vec![s.clone()] } else { split_lines(s, per) }
        }
        _ => Vec::new(),
    }
}

/// Line spacing of a wrapped chart title, x title and (across) y title, before the label size.
const CHART_LH: f64 = 19.0;
const X_LH: f64 = 16.0;
const Y_LH: f64 = 17.0;

/// The width a chart title centred on `cx` has before either edge of a canvas `w` wide (20 px kept at each side).
pub fn chart_room(w: f64, cx: f64) -> f64 { 2.0 * (cx - 20.0).min(w - 20.0 - cx) }

/// The band the chart title takes over the plot: none, one line (34 px, scaled) and 19 px more for each further line.
pub fn chart_band(t: &Label, room: f64, lab: f64) -> f64 {
    match t {
        Label::Off => 0.0,
        Label::Blank => 34.0 * lab,
        Label::Text(_) => 34.0 * lab + (title_lines(t, room, 15.0 * lab, 0.6).len().max(1) - 1) as f64 * CHART_LH * lab,
    }
}

/// How much further right the plot starts so a y title wrapped onto more lines (to fit the plot's height `plot_h`)
/// stays clear of the tick numbers.
pub fn y_band(t: &Label, plot_h: f64, lab: f64) -> f64 {
    (title_lines(t, plot_h - 4.0, 13.0 * lab, 0.55).len().max(1) - 1) as f64 * Y_LH * lab
}

/// How much lower everything under the x title goes when it wraps onto more lines (to fit the plot's width).
pub fn x_band(t: &Label, plot_w: f64, lab: f64) -> f64 {
    (title_lines(t, plot_w, 13.0 * lab, 0.55).len().max(1) - 1) as f64 * X_LH * lab
}

/// The chart title (or its blank line) centred over the plot at `cx`, its first baseline at `y`, wrapped to `room`.
pub fn draw_chart_title(d: &mut Doc, t: &Label, cx: f64, y: f64, room: f64, lab: f64, ink: &str) {
    match t {
        Label::Text(_) => {
            for (i, line) in title_lines(t, room, 15.0 * lab, 0.6).iter().enumerate() {
                d.raw(&format!(
                    r##"<text x="{cx:.2}" y="{:.2}" font-size="{}" text-anchor="middle" font-weight="bold" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{ink}">{}</text>"##,
                    y + i as f64 * CHART_LH * lab,
                    15.0 * lab,
                    svg::esc(line)
                ));
            }
        }
        Label::Blank => { d.line(cx - 120.0 * lab, y, cx + 120.0 * lab, y, ink, 1.3); }
        Label::Off => {}
    }
}

/// The x title at baseline `y` and the y title rotated at x = 18 (or the blank lines to write them on). A title too
/// long for the plot's width (x) or height (y) wraps: the x title's further lines go below, the y title's to the
/// right of the first, nearer the axis (the plot was moved right by [`y_band`] to make room).
pub fn draw_axis_titles(d: &mut Doc, f: &Frame, xt: &Label, yt: &Label, y: f64, lab: f64, ink: &str) {
    let cx = (f.left() + f.right()) / 2.0;
    match xt {
        Label::Text(_) => {
            for (i, line) in title_lines(xt, f.right() - f.left(), 13.0 * lab, 0.55).iter().enumerate() {
                d.raw(&text_el(cx, y + i as f64 * X_LH * lab, 13.0 * lab, "middle", ink, line));
            }
        }
        Label::Blank => { d.line(cx - 90.0 * lab, y, cx + 90.0 * lab, y, ink, 1.3); }
        Label::Off => {}
    }
    let cy = (f.top() + f.bottom()) / 2.0;
    match yt {
        Label::Text(_) => {
            for (i, line) in title_lines(yt, f.bottom() - f.top() - 4.0, 13.0 * lab, 0.55).iter().enumerate() {
                let x = 18.0 + i as f64 * Y_LH * lab;
                let tx = if i == 0 { "18.00".to_string() } else { format!("{x:.2}") };
                d.raw(&format!(
                    r##"<text transform="translate({tx} {cy:.2}) rotate(-90)" font-size="{}" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{ink}">{}</text>"##,
                    13.0 * lab,
                    svg::esc(line)
                ));
            }
        }
        Label::Blank => {
            let half = ((f.bottom() - f.top()) * 0.3).min(90.0 * lab);
            d.line(18.0, cy - half, 18.0, cy + half, ink, 1.3);
        }
        Label::Off => {}
    }
}

/// The handles' report: where each draggable optimum is, and the frame to invert a drag with.
pub struct Multi {
    pub frame: (f64, f64, f64, f64, f64, f64),
    /// (slot, dot position) of every optimum drawn for an unknown enzyme.
    pub unknown_opts: Vec<(usize, (f64, f64))>,
    /// Every label box drawn on the plot, with its text (tests check none overprints another).
    pub labels: Vec<((f64, f64, f64, f64), String)>,
    /// Every mark dot drawn.
    pub dots: Vec<(f64, f64)>,
}

/// Text width at font size `size` (the same 0.55 em estimate the single-curve figure uses).
fn tw(s: &str, size: f64) -> f64 { s.chars().count() as f64 * size * 0.55 }

pub fn build(p: &Params) -> (String, Multi) {
    let a = against(p);
    let cs = curves(p);
    let dec = p.i("dec").clamp(1, 3);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let lab = svg::label_factor(p.s("label_size"));
    let ink = if print { "#555555" } else { INK };
    let grid = if print { "#787878" } else { "#e2e2e2" };
    let numbered = p.s("axes") != "Shape only";
    let grid_on = numbered && p.b("gridlines");
    let (xt, yt, chart) = titles(p);
    let fs = 12.0 * lab;

    let (x0, x1) = x_range(p, &cs);
    let y_fit_top = match a {
        Against::Substrate => cs.iter().map(|c| c.vmax).fold(0.0, f64::max) * 1.15,
        _ => PEAK * 1.15,
    };
    let y_top = if p.b("y_fit") { y_fit_top } else { fin(p.f("y_to"), 1.0, 200.0, 12.0) };

    let w = (W * (1.0 + 0.45 * (lab - 1.0))).max(W);
    // the plot grows a little with the labels so the rotated rate title still fits beside it at Large
    let plot_h = (H - 22.0 - 52.0) * (1.0 + 0.3 * (lab - 1.0)).max(1.0);
    let left = 66.0 + y_band(&yt, plot_h, lab);
    let title_cx = (left + w - 20.0) / 2.0;
    let title_w = chart_room(w, title_cx);
    let title_room = chart_band(&chart, title_w, lab);
    let top = 22.0 + title_room;
    let h = top + plot_h + 52.0;
    let f = Frame { w, h, l: left, r: 20.0, t: top, b: 52.0, x0, x1, y0: 0.0, y1: y_top };
    let x_more = x_band(&xt, f.right() - f.left(), lab);

    // under the plot: the x title, then the key, then (against substrate) the givens
    let title_y = f.bottom() + 40.0 * lab;
    let tagged = cs.len() > 1;
    let key_items: Vec<String> = if cs.len() > 1 {
        cs.iter().map(|c| if tagged { format!("{}: {}", c.letter(), c.name) } else { c.name.clone() }).collect()
    } else {
        Vec::new()
    };
    // the key in rows that fit the canvas: a 28 px sample, 6 px, the name, 22 px to the next item
    let item_w = |s: &str| (28.0 + 6.0) * lab + tw(s, fs) + 22.0 * lab;
    let mut key_rows: Vec<Vec<usize>> = Vec::new();
    let mut row_w = 0.0;
    for (i, it) in key_items.iter().enumerate() {
        let iw = item_w(it);
        if key_rows.is_empty() || row_w + iw - 22.0 * lab > w - 32.0 {
            key_rows.push(vec![i]);
            row_w = iw;
        } else {
            key_rows.last_mut().unwrap().push(i);
            row_w += iw;
        }
    }
    let after_title = if xt == Label::Off { f.bottom() + 24.0 * lab } else { title_y + 6.0 * lab + x_more };
    let key_y0 = after_title + 20.0 * lab;
    let mut next_y = if key_rows.is_empty() { after_title } else { key_y0 + (key_rows.len() - 1) as f64 * 20.0 * lab + 6.0 * lab };

    let inhibited = cs.iter().any(|c| c.mode != "none");
    let givens_lines: Vec<String> = if a == Against::Substrate {
        let (vmax, km, smax, _, _, _) = given(p);
        let factor = 1.0 + fin(p.f("inh"), 0.0, 10.0, 3.0) / fin(p.f("ki"), 0.2, 10.0, 1.0);
        let g = if inhibited {
            let factor_text = if ans { val(factor, 2) } else { BLANK.to_string() };
            format!(
                "Vmax = {}, Km = {}, [I] = {}, Ki = {} (1 + [I]/Ki = {})",
                val(vmax, dec),
                val(km, dec),
                val(fin(p.f("inh"), 0.0, 10.0, 3.0), 1),
                val(fin(p.f("ki"), 0.2, 10.0, 1.0), 1),
                factor_text
            )
        } else {
            format!("Vmax = {}, Km = {}, curve drawn to [S] = {}", val(vmax, dec), val(km, dec), val(smax, dec))
        };
        let chars_per_line = ((w - 48.0) / (fs * 0.55)).floor().max(20.0) as usize;
        super::super::wrap(&g, chars_per_line)
    } else {
        Vec::new()
    };
    let givens_y0 = next_y + 22.0 * lab;
    if !givens_lines.is_empty() { next_y = givens_y0 + (givens_lines.len() - 1) as f64 * 15.0 * lab; }
    let total_h = next_y + 16.0 * lab;

    let mut d = Doc::new(w, total_h);
    let names: Vec<&str> = cs.iter().map(|c| c.name.as_str()).collect();
    let what = match a {
        Against::Substrate => "substrate concentration",
        Against::Temperature => "temperature",
        Against::Ph => "pH",
    };
    let marks = match a {
        Against::Substrate => if p.s("vk_label") == "None" { "" } else { ", with Vmax and Km marked" },
        _ => match (p.b("mark_opt"), p.b("mark_den")) {
            (true, true) => ", with the optimum and denaturation marked",
            (true, false) => ", with the optimum marked",
            (false, true) => ", with denaturation marked",
            _ => "",
        },
    };
    // no value in the description: it is exported with the student copy
    d.describe("Enzyme activity", &format!("Rate of reaction against {what} for {}{marks}", names.join(", ")));

    draw_chart_title(&mut d, &chart, title_cx, 22.0 + 20.0 * lab, title_w, lab, ink);

    // axes, gridlines and numbers
    let mut tick_boxes: Vec<(f64, f64, f64, f64)> = Vec::new();
    if numbered {
        let xs = if a == Against::Ph { 2.0 } else { nice_step(x1 - x0, 6.0) };
        let ys = nice_step(y_top, 5.0);
        for (v, label) in ticks(x0, x1, xs) {
            let x = f.px(v);
            if grid_on { d.line(x, f.top(), x, f.bottom(), grid, 1.0); }
            d.raw(&text_el(x, f.bottom() + 18.0 * lab, fs, "middle", ink, &label));
            tick_boxes.push(box_around(x, tw(&label, fs), "middle", f.bottom() + 18.0 * lab, fs));
        }
        for (v, label) in ticks(0.0, y_top, ys) {
            let y = f.py(v);
            if grid_on { d.line(f.left(), y, f.right(), y, grid, 1.0); }
            d.raw(&text_el(f.left() - 8.0, y + 4.0 * lab, fs, "end", ink, &label));
            tick_boxes.push(box_around(f.left() - 8.0, tw(&label, fs), "end", y + 4.0 * lab, fs));
        }
    }
    d.line(f.left(), f.bottom(), f.right(), f.bottom(), ink, 1.6);
    d.line(f.left(), f.top(), f.left(), f.bottom(), ink, 1.6);
    if p.b("arrows") { axis_arrows(&mut d, &f, ink); }
    draw_axis_titles(&mut d, &f, &xt, &yt, title_y, lab, ink);

    // the curves, clipped to the plot (a rate axis set by hand may stop below the peak)
    d.raw(&format!(
        r##"<clipPath id="enz-plot"><rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}"/></clipPath><g clip-path="url(#enz-plot)">"##,
        f.left() - 2.0,
        f.top() - 2.0,
        f.right() - f.left() + 4.0,
        f.bottom() - f.top() + 4.0
    ));
    let mut paths: Vec<Vec<(f64, f64)>> = Vec::new();
    for c in &cs {
        let n = 320;
        // the optimum and the denaturation points are samples too, so the peak and the corners are drawn exactly
        let mut xs: Vec<f64> = (0..=n).map(|k| x0 + (x1 - x0) * k as f64 / n as f64).collect();
        if a != Against::Substrate { xs.extend([c.opt, c.lo, c.hi].into_iter().filter(|v| v.is_finite() && *v > x0 && *v < x1)); }
        xs.sort_by(|u, v| u.total_cmp(v));
        let pts: Vec<(f64, f64)> = xs.iter().map(|&x| (f.px(x), f.py(c.rate(a, x)))).collect();
        let (col, dash) = curve_style(c.slot, print);
        d.raw(&poly_el(&pts, col, dash, 2.4));
        paths.push(pts);
    }
    d.raw("</g>");

    // the marks
    let mut placed: Vec<((f64, f64, f64, f64), String)> = Vec::new();
    let mut dots: Vec<(f64, f64)> = Vec::new();
    let mut unknown_opts = Vec::new();
    const DOT_R: f64 = 4.5;
    let dot_box = |x: f64, y: f64| (x - DOT_R - 2.0, x + DOT_R + 2.0, y - DOT_R - 2.0, y + DOT_R + 2.0);
    let in_x = |v: f64| v >= x0 - 1e-9 && v <= x1 + 1e-9;
    let plot_ok = |b: (f64, f64, f64, f64)| b.0 >= 2.0 && b.1 <= w - 2.0 && b.2 >= f.top() - 18.0 * lab && b.3 <= f.bottom() - 2.0;
    // text drawn after every dot and line so the halo is on top
    let mut texts: Vec<(f64, &'static str, f64, String, &'static str, Option<(f64, f64)>)> = Vec::new();

    // each label is placed against every dot and label before it, and the tick numbers
    // (`lines`: thin dashed lines a label should not sit on, as boxes)
    let place = |cands: &[(f64, &'static str, f64)], s: &str, placed: &mut Vec<((f64, f64, f64, f64), String)>, dots: &[(f64, f64)], lines: &[(f64, f64, f64, f64)], ok: &dyn Fn((f64, f64, f64, f64)) -> bool| -> (f64, &'static str, f64) {
        let width = tw(s, fs);
        let mut obstacles: Vec<(f64, f64, f64, f64)> = placed.iter().map(|(b, _)| *b).collect();
        obstacles.extend(lines.iter().copied());
        obstacles.extend(dots.iter().map(|(x, y)| dot_box(*x, *y)));
        obstacles.extend(tick_boxes.iter().copied());
        let c = pick(cands, width, fs, &obstacles, ok);
        placed.push((box_around(c.0, width, c.1, c.2, fs), s.to_string()));
        c
    };

    let mut line_boxes: Vec<(f64, f64, f64, f64)> = Vec::new();
    match a {
        Against::Temperature | Against::Ph => {
            let opt_name = { let t = tidy_text(p.s("opt_text")); if t.is_empty() { "Optimum".to_string() } else { t.chars().take(30).collect() } };
            let den_name = { let t = tidy_text(p.s("den_text")); if t.is_empty() { "Denatured".to_string() } else { t.chars().take(30).collect() } };
            // every dot first, so each label can clear all of them
            let mut opt_dots: Vec<(usize, (f64, f64))> = Vec::new();
            let mut den_dots: Vec<(usize, f64, (f64, f64))> = Vec::new();
            for (i, c) in cs.iter().enumerate() {
                // a rate axis set by hand below the peak has no optimum to mark
                if p.b("mark_opt") && in_x(c.opt) && PEAK <= y_top + 1e-9 {
                    let pt = (f.px(c.opt), f.py(PEAK));
                    opt_dots.push((i, pt));
                    dots.push(pt);
                    if p.s(["enz1", "enz2", "enz3"][c.slot]) == "Unknown enzyme" { unknown_opts.push((c.slot, pt)); }
                }
                if p.b("mark_den") {
                    for v in [c.lo, c.hi] {
                        if v.is_finite() && in_x(v) {
                            let pt = (f.px(v), f.bottom());
                            den_dots.push((i, v, pt));
                            dots.push(pt);
                        }
                    }
                }
            }
            for (i, (x, y)) in &opt_dots {
                let c = &cs[*i];
                d.raw(&format!(r##"<line x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1.2" stroke-dasharray="4 4"/>"##, y + DOT_R, f.bottom()));
                let (col, _) = curve_style(c.slot, print);
                d.circle(*x, *y, DOT_R, "#fff", col, 2.0);
            }
            for (i, _, (x, y)) in &den_dots {
                let (col, _) = curve_style(cs[*i].slot, print);
                d.circle(*x, *y, DOT_R, "#fff", col, 2.0);
            }
            // curves with the same optimum share one dot and one label naming them all ("Optimum A, B")
            let same_at = |pts: &[(f64, f64)], q: (f64, f64)| -> Vec<usize> { pts.iter().enumerate().filter(|(_, r)| (r.0 - q.0).abs() < 0.5 && (r.1 - q.1).abs() < 0.5).map(|(k, _)| k).collect() };
            let group_tag = |n: &str, ks: &[usize], idx: &dyn Fn(usize) -> usize| -> String {
                if !tagged { return n.to_string(); }
                let ls: Vec<String> = ks.iter().map(|&k| cs[idx(k)].letter().to_string()).collect();
                format!("{n} {}", ls.join(", "))
            };
            let opt_pts: Vec<(f64, f64)> = opt_dots.iter().map(|(_, q)| *q).collect();
            for (k_this, (i, (x, y))) in opt_dots.iter().enumerate() {
                let c = &cs[*i];
                let same = same_at(&opt_pts, (*x, *y));
                if same[0] != k_this { continue; }
                let lbl = mark_label(p.s("opt_label"), &group_tag(&opt_name, &same, &|k| opt_dots[k].0), &x_value(a, c.opt), ": ", ans);
                let s = match &lbl { Label::Text(s) => s.clone(), Label::Blank => BLANK_LINE.to_string(), Label::Off => continue };
                let (x, y) = (*x, *y);
                let cands = [
                    (x, "middle", y - 11.0 * lab),
                    (x + 9.0, "start", y - 8.0 * lab),
                    (x - 9.0, "end", y - 8.0 * lab),
                    (x + 9.0, "start", y + 10.0 * lab),
                    (x - 9.0, "end", y + 10.0 * lab),
                    (x + 9.0, "start", y + 26.0 * lab),
                    (x - 9.0, "end", y + 26.0 * lab),
                    (x + 9.0, "start", y + 42.0 * lab),
                    (x - 9.0, "end", y + 42.0 * lab),
                    (x + 9.0, "start", y + 58.0 * lab),
                    (x - 9.0, "end", y + 58.0 * lab),
                ];
                let (lx, an, ly) = place(&cands, &s, &mut placed, &dots, &line_boxes, &plot_ok);
                texts.push((lx, an, ly, s, curve_style(c.slot, print).0, Some((x, y))));
            }
            let den_pts: Vec<(f64, f64)> = den_dots.iter().map(|(_, _, q)| *q).collect();
            for (k_this, (i, v, (x, y))) in den_dots.iter().enumerate() {
                let c = &cs[*i];
                let same = same_at(&den_pts, (*x, *y));
                if same[0] != k_this { continue; }
                let lbl = mark_label(p.s("den_label"), &group_tag(&den_name, &same, &|k| den_dots[k].0), &x_value(a, *v), ": ", ans);
                let s = match &lbl { Label::Text(s) => s.clone(), Label::Blank => BLANK_LINE.to_string(), Label::Off => continue };
                let (x, y) = (*x, *y);
                // the side away from the optimum first: the curve comes down to the dot from the other side
                let right_first = *v >= c.opt;
                let side = |dx: f64, dy: f64| -> [(f64, &'static str, f64); 2] {
                    let r = (x + dx, "start", y - dy);
                    let l = (x - dx, "end", y - dy);
                    if right_first { [r, l] } else { [l, r] }
                };
                let mut cands: Vec<(f64, &'static str, f64)> = Vec::new();
                for dy in [10.0, 26.0, 42.0, 58.0, 74.0] { cands.extend(side(6.0, dy * lab)); }
                cands.push((x, "middle", y - 12.0 * lab));
                let (lx, an, ly) = place(&cands, &s, &mut placed, &dots, &line_boxes, &plot_ok);
                texts.push((lx, an, ly, s, curve_style(c.slot, print).0, Some((x, y))));
            }
        }
        Against::Substrate => {
            let mode = p.s("vk_label");
            // Vmax: one dashed line per distinct value, named for every curve that shares it
            let mut groups: Vec<(f64, Vec<usize>)> = Vec::new();
            for (i, c) in cs.iter().enumerate() {
                match groups.iter_mut().find(|g| (g.0 - c.vmax).abs() < 1e-9) {
                    Some(g) => g.1.push(i),
                    None => groups.push((c.vmax, vec![i])),
                }
            }
            for (v, members) in &groups {
                let vy = f.py(*v);
                if *v > y_top { continue; }
                let (col, _) = curve_style(cs[members[0]].slot, print);
                d.raw(&format!(r##"<line x1="{:.2}" y1="{vy:.2}" x2="{:.2}" y2="{vy:.2}" stroke="{col}" stroke-width="1.4" stroke-dasharray="7 5"/>"##, f.left(), f.right()));
            }
            for c in &cs {
                if c.km > x1 { continue; }
                let (col, _) = curve_style(c.slot, print);
                let (kx, half) = (f.px(c.km), f.py(c.vmax / 2.0));
                d.raw(&format!(r##"<line x1="{kx:.2}" y1="{:.2}" x2="{kx:.2}" y2="{half:.2}" stroke="{col}" stroke-width="1.2" stroke-dasharray="3 4"/>"##, f.bottom()));
                d.raw(&format!(r##"<line x1="{:.2}" y1="{half:.2}" x2="{kx:.2}" y2="{half:.2}" stroke="{col}" stroke-width="1.2" stroke-dasharray="3 4"/>"##, f.left()));
                line_boxes.push((f.left(), kx, half - 1.0, half + 1.0));
            }
            for (v, _) in &groups {
                if *v <= y_top { let vy = f.py(*v); line_boxes.push((f.left(), f.right(), vy - 1.0, vy + 1.0)); }
            }
            for (v, members) in &groups {
                if *v > y_top { continue; }
                let letters: Vec<String> = members.iter().map(|&i| cs[i].letter().to_string()).collect();
                let name = format!("Vmax {}", letters.join(", "));
                let s = match mark_label(mode, &name, &val(*v, dec), " = ", ans) { Label::Text(s) => s, Label::Blank => BLANK_LINE.to_string(), Label::Off => continue };
                let vy = f.py(*v);
                let mut cands = Vec::new();
                for dy in [6.0, 22.0, 38.0] { cands.push((f.right() - 4.0, "end", vy - dy * lab)); }
                for dy in [6.0, 22.0, 38.0] { cands.push((f.left() + 6.0, "start", vy - dy * lab)); }
                for dy in [16.0, 32.0] { cands.push((f.right() - 4.0, "end", vy + dy * lab)); }
                let (lx, an, ly) = place(&cands, &s, &mut placed, &dots, &line_boxes, &plot_ok);
                texts.push((lx, an, ly, s, curve_style(cs[members[0]].slot, print).0, None));
            }
            // half of each Vmax: the dotted line from the rate axis to the curve, named like its Vmax (one label per value)
            for (v, members) in &groups {
                let inside: Vec<usize> = members.iter().copied().filter(|&i| cs[i].km <= x1).collect();
                if inside.is_empty() { continue; }
                let letters: Vec<String> = inside.iter().map(|&i| cs[i].letter().to_string()).collect();
                let s = match mark_label(mode, &format!("\u{bd}Vmax {}", letters.join(", ")), &val(*v / 2.0, dec), " = ", ans) { Label::Text(s) => s, Label::Blank => BLANK_LINE.to_string(), Label::Off => continue };
                let half = f.py(*v / 2.0);
                let end = inside.iter().map(|&i| f.px(cs[i].km)).fold(f.left(), f64::max);
                let mut cands = Vec::new();
                for dy in [-6.0, 15.0, -22.0, 31.0] {
                    for dx in [6.0, 36.0, 66.0, 96.0, 126.0] {
                        let x = f.left() + dx * lab;
                        if x < end + 40.0 * lab { cands.push((x, "start", half + dy * lab)); }
                    }
                }
                if cands.is_empty() { cands.push((f.left() + 6.0, "start", half - 6.0 * lab)); }
                let (lx, an, ly) = place(&cands, &s, &mut placed, &dots, &line_boxes, &plot_ok);
                texts.push((lx, an, ly, s, curve_style(cs[inside[0]].slot, print).0, None));
            }
            // Km: one label per distinct value, named for every curve that shares it (a noncompetitive inhibitor keeps Km)
            let mut km_groups: Vec<(f64, Vec<usize>)> = Vec::new();
            for (i, c) in cs.iter().enumerate() {
                if c.km > x1 { continue; }
                match km_groups.iter_mut().find(|g| (g.0 - c.km).abs() < 1e-9) {
                    Some(g) => g.1.push(i),
                    None => km_groups.push((c.km, vec![i])),
                }
            }
            for (km_v, members) in &km_groups {
                let c = &cs[members[0]];
                let letters: Vec<String> = members.iter().map(|&i| cs[i].letter().to_string()).collect();
                let s = match mark_label(mode, &format!("Km {}", letters.join(", ")), &val(*km_v, dec), " = ", ans) { Label::Text(s) => s, Label::Blank => BLANK_LINE.to_string(), Label::Off => continue };
                let kx = f.px(c.km);
                let base = f.bottom() - 6.0 * lab;
                let mut cands = Vec::new();
                for row in [0.0, 16.0, 32.0, 48.0] {
                    let y = base - row * lab;
                    cands.push((kx, "middle", y));
                    for dx in [4.0, 12.0, 24.0] { cands.push((kx + dx, "start", y)); cands.push((kx - dx, "end", y)); }
                }
                let (lx, an, ly) = place(&cands, &s, &mut placed, &dots, &line_boxes, &plot_ok);
                texts.push((lx, an, ly, s, curve_style(c.slot, print).0, Some((kx, f.bottom()))));
            }
        }
    }
    // a label that had to move away from its dot is tied back to it by a thin leader, drawn under every label
    for (x, an, y, s, col, dot) in &texts {
        let Some((dx, dy)) = dot else { continue };
        let b = box_around(*x, tw(s, fs), an, *y, fs);
        let ink_b = (b.0, b.1, *y - 0.74 * fs, *y + 0.2 * fs);
        let (nx, ny) = (dx.clamp(ink_b.0, ink_b.1), dy.clamp(ink_b.2, ink_b.3));
        let dist = ((nx - dx).powi(2) + (ny - dy).powi(2)).sqrt();
        if dist > 16.0 * lab {
            let (ux, uy) = ((nx - dx) / dist, (ny - dy) / dist);
            let (ax, ay) = (dx + ux * (DOT_R + 2.0), dy + uy * (DOT_R + 2.0));
            let (bx, by) = (nx - ux * 3.0, ny - uy * 3.0);
            d.raw(&format!(r##"<line x1="{ax:.2}" y1="{ay:.2}" x2="{bx:.2}" y2="{by:.2}" stroke="{col}" stroke-width="1"/>"##));
        }
    }
    for (x, an, y, s, col, _) in &texts {
        d.raw(&mark_text(*x, *y, fs, an, col, s));
    }

    // the key
    for (r, row) in key_rows.iter().enumerate() {
        let total: f64 = row.iter().map(|&i| item_w(&key_items[i])).sum::<f64>() - 22.0 * lab;
        let mut x = (w - total) / 2.0;
        let y = key_y0 + r as f64 * 20.0 * lab;
        for &i in row {
            let c = &cs[i];
            let (col, dash) = curve_style(c.slot, print);
            d.raw(&poly_el(&[(x, y - 4.0 * lab), (x + 28.0 * lab, y - 4.0 * lab)], col, dash, 2.4));
            d.raw(&text_el(x + 34.0 * lab, y, fs, "start", ink, &key_items[i]));
            x += item_w(&key_items[i]);
        }
    }
    for (i, line) in givens_lines.iter().enumerate() {
        d.raw(&text_el(24.0, givens_y0 + i as f64 * 15.0 * lab, fs, "start", ink, line));
    }

    let rep = Multi { frame: (f.l, f.w - f.l - f.r, f.x0, f.x1, f.t, f.h - f.t - f.b), unknown_opts, labels: placed, dots };
    (d.finish(), rep)
}

/// An unknown enzyme's optimum dot is draggable along the axis: it sets that curve's optimum temperature or pH.
pub fn handles(p: &Params) -> Vec<Handle> {
    let a = against(p);
    let rep = build(p).1;
    let (l, span, x0, x1, _, _) = rep.frame;
    let mut out = Vec::new();
    for (slot, pt) in rep.unknown_opts {
        let key: &'static str = match (a, slot) {
            (Against::Temperature, 0) => "uopt_t1",
            (Against::Temperature, 1) => "uopt_t2",
            (Against::Temperature, _) => "uopt_t3",
            (_, 0) => "uopt_ph1",
            (_, 1) => "uopt_ph2",
            _ => "uopt_ph3",
        };
        let (lo, hi, stepk): (f64, f64, f64) = if a == Against::Temperature { (0.0, 100.0, 1.0) } else { (0.0, 14.0, 10.0) };
        let tip = match slot {
            0 => "Drag the optimum of curve A along the axis",
            1 => "Drag the optimum of curve B along the axis",
            _ => "Drag the optimum of curve C along the axis",
        };
        out.push(Handle::knob(pt, tip, move |p, q| {
            if !q.0.is_finite() { return; }
            let v = x0 + (q.0 - l) / span.max(1e-9) * (x1 - x0);
            let v = (v * stepk).round() / stepk;
            p.set_f(key, v.clamp(lo, hi.min(x1)));
        }));
    }
    out
}

/// The plain information under the curves in the panel: each enzyme's optimum and where it has denatured, and which
/// curves lie exactly on top of each other (human digestive enzymes all work best at body temperature). None
/// against substrate.
pub fn info(p: &Params) -> Option<String> {
    let a = against(p);
    if a == Against::Substrate { return None; }
    let cs = curves(p);
    let mut lines: Vec<String> = cs
        .iter()
        .map(|c| match a {
            Against::Temperature => format!("{}: works best at {} and has denatured by {}", c.name, x_value(a, c.opt), x_value(a, c.hi)),
            _ => {
                if c.lo > 0.0 {
                    format!("{}: works best at {}, and only between {} and {}", c.name, x_value(a, c.opt), x_value(a, c.lo), x_value(a, c.hi))
                } else {
                    format!("{}: works best at {}, and has stopped working by {}", c.name, x_value(a, c.opt), x_value(a, c.hi))
                }
            }
        })
        .collect();
    for (i, c) in cs.iter().enumerate() {
        for d in &cs[i + 1..] {
            // the same numbers give the same curve (lo is -inf for both against temperature, and -inf == -inf)
            if c.opt == d.opt && c.lo == d.lo && c.hi == d.hi && (a == Against::Ph || c.rise == d.rise) {
                lines.push(format!("{} and {} have the same curve here, so curve {} is drawn exactly over curve {}.", c.name, d.name, d.letter(), c.letter()));
            }
        }
    }
    Some(lines.join("\n"))
}

/// The panel's introduction, following what the figure draws.
pub fn intro(p: &Params) -> Option<String> {
    Some(match against(p) {
        Against::Temperature => "Rate of reaction against temperature: each enzyme rises to its optimum and falls to zero where it has denatured. Up to three named enzymes on one graph.".to_string(),
        Against::Ph => "Rate of reaction against pH: each enzyme works best at its optimum pH and stops working too far either side. Up to three named enzymes on one graph.".to_string(),
        Against::Substrate if is_multi(p) => "Rate against substrate concentration for up to three curves, each with no inhibitor, a competitive one (Km rises) or a noncompetitive one (Vmax falls), with Vmax and Km marked.".to_string(),
        Against::Substrate => return None,
    })
}

/// Editor warnings: only what could not be drawn as asked.
pub fn notes(p: &Params) -> Vec<String> { axis_note(p).into_iter().collect() }

/// The note under the controls when a rate axis set by hand stops below the peaks.
pub fn axis_note(p: &Params) -> Option<String> {
    if against(p) == Against::Substrate || p.b("y_fit") { return None; }
    let top = fin(p.f("y_to"), 1.0, 200.0, 12.0);
    (top < PEAK).then(|| format!("The rate axis stops at {} but every curve peaks at {}, so the tops are cut off and no optimum is marked.", num(top, 1), num(PEAK, 0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::interact;

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

    /// Every `<text>` body and the `<desc>` of an SVG.
    fn texts(svg: &str) -> Vec<String> {
        let mut out = Vec::new();
        for tag in ["<text", "<desc", "<title"] {
            let mut rest = svg;
            while let Some(i) = rest.find(tag) {
                rest = &rest[i..];
                let Some(j) = rest.find('>') else { break };
                let body = &rest[j + 1..];
                let Some(k) = body.find("</") else { break };
                out.push(body[..k].to_string());
                rest = &body[k..];
            }
        }
        out
    }

    #[test]
    fn the_temperature_curve_rises_to_the_optimum_and_falls_to_zero_where_it_denatures() {
        let (opt, zero, rise) = BODY;
        assert_eq!((opt, zero), (37.0, 47.0));
        assert!((temp_rate(opt, opt, zero, rise) - 1.0).abs() < 1e-12);
        assert_eq!(temp_rate(zero, opt, zero, rise), 0.0);
        assert_eq!(temp_rate(60.0, opt, zero, rise), 0.0);
        // by hand: at 0 °C, exp(-(37/19)^1.5) = exp(-2.717) = 0.066, about the Q10-of-2 figure 2^-3.7 = 0.077
        let cold = temp_rate(0.0, opt, zero, rise);
        assert!((cold - (-(37.0f64 / 19.0).powf(1.5)).exp()).abs() < 1e-12 && cold > 0.05 && cold < 0.08, "{cold}");
        // halfway down the fall, cos²(π/4) = 0.5
        assert!((temp_rate(42.0, opt, zero, rise) - 0.5).abs() < 1e-12);
        // rises all the way to the optimum, falls all the way after it, and the fall is the steeper
        let mut last = -1.0;
        for t in 0..=37 { let r = temp_rate(t as f64, opt, zero, rise); assert!(r > last, "{t}"); last = r; }
        for t in 38..=47 { let r = temp_rate(t as f64, opt, zero, rise); assert!(r < last, "{t}"); last = r; }
        assert!(1.0 - temp_rate(42.0, opt, zero, rise) > 1.0 - temp_rate(32.0, opt, zero, rise));
        // Taq: still about a tenth of its peak at 95 °C (it survives PCR's denaturing step), zero at 100 °C
        let taq = profile("Taq polymerase").unwrap();
        let at95 = temp_rate(95.0, taq.t_opt, taq.t_zero, taq.t_rise);
        assert!((at95 - (FRAC_PI_2 * 0.8).cos().powi(2)).abs() < 1e-12 && at95 > 0.08 && at95 < 0.11, "{at95}");
        assert!(temp_rate(37.0, taq.t_opt, taq.t_zero, taq.t_rise) < 0.06, "Taq is nearly idle at body temperature");
    }

    #[test]
    fn the_ph_curve_is_a_hill_zero_either_side() {
        assert!((ph_rate(2.0, 2.0, 2.5) - 1.0).abs() < 1e-12);
        assert_eq!(ph_rate(4.5, 2.0, 2.5), 0.0);
        assert_eq!(ph_rate(-0.5, 2.0, 2.5), 0.0);
        for d in [0.3, 1.0, 1.7, 2.4] { assert!((ph_rate(2.0 + d, 2.0, 2.5) - ph_rate(2.0 - d, 2.0, 2.5)).abs() < 1e-12); }
        // the named enzymes' optima, from the sources in `profile`
        let opt = |n: &str| profile(n).unwrap();
        assert_eq!(opt("Pepsin").ph_opt, 2.0);
        assert_eq!(opt("Salivary amylase").ph_opt, 6.8);
        assert_eq!(opt("Trypsin").ph_opt, 8.0);
        assert_eq!(opt("Catalase").ph_opt, 7.0);
        assert_eq!(opt("Human enzyme").ph_opt, 7.0);
        assert_eq!(opt("Taq polymerase").ph_opt, 8.3);
        assert_eq!(opt("Taq polymerase").t_opt, 75.0);
        for n in ["Human enzyme", "Pepsin", "Salivary amylase", "Trypsin", "Catalase"] { assert_eq!(opt(n).t_opt, 37.0, "{n}"); }
        assert!(profile("Unknown enzyme").is_none());
    }

    /// The highest point of each drawn curve (its polyline) is at the enzyme's optimum, in the plot's pixels.
    #[test]
    fn each_drawn_curve_peaks_at_its_enzymes_optimum() {
        for (a, e) in [("Temperature", "Human enzyme"), ("Temperature", "Taq polymerase"), ("pH", "Pepsin"), ("pH", "Trypsin"), ("pH", "Salivary amylase"), ("pH", "Catalase")] {
            let p = with(&[s("against", a), s("enz1", e)]);
            let (svg, rep) = build(&p);
            let (l, span, x0, x1, t, hgt) = rep.frame;
            let poly = svg.split("<polyline points=\"").nth(1).unwrap();
            let pts: Vec<(f64, f64)> = poly[..poly.find('"').unwrap()].split(' ').map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect();
            let top = pts.iter().cloned().fold((0.0, f64::INFINITY), |acc, q| if q.1 < acc.1 { q } else { acc });
            let c = &curves(&p)[0];
            let want = l + (c.opt - x0) / (x1 - x0) * span;
            assert!((top.0 - want).abs() <= span / 320.0 + 1e-6, "{a} {e}: peak at {} want {want}", top.0);
            // and it peaks at 10 on a fitted axis to 11.5
            assert!((top.1 - (t + hgt - PEAK / 11.5 * hgt)).abs() < 0.02, "{a} {e}");
        }
    }

    #[test]
    fn inhibitor_curves_move_km_or_vmax_and_name_it() {
        // [I] = 2, Ki = 1: factor 3. Competitive: Km 2 -> 6, Vmax 10 kept; noncompetitive: Vmax 10 -> 3.33, Km 2 kept.
        let l = [b("curve2", true), b("curve3", true), f("inh", 2.0), f("ki", 1.0)];
        let cs = curves(&with(&l));
        assert_eq!(cs.iter().map(|c| c.mode).collect::<Vec<_>>(), ["none", "competitive", "noncompetitive"]);
        assert_eq!((cs[0].vmax, cs[0].km), (10.0, 2.0));
        assert_eq!((cs[1].vmax, cs[1].km), (10.0, 6.0));
        assert!((cs[2].vmax - 10.0 / 3.0).abs() < 1e-12 && cs[2].km == 2.0);
        // the rate at Km is half of each curve's own Vmax
        for c in &cs { assert!((c.rate(Against::Substrate, c.km) - c.vmax / 2.0).abs() < 1e-12); }
        let t = texts(&build(&with(&l)).0).join("\n");
        for want in ["Vmax A, B = 10", "Vmax C = 3.33", "Km A, C = 2", "Km B = 6", "A: No inhibitor", "B: Competitive inhibitor", "C: Noncompetitive inhibitor", "(1 + [I]/Ki = 3)"] {
            assert!(t.contains(want), "{want} missing from {t}");
        }
        // even when curve A has no inhibitor of its own, B and C use the typed strength
        assert!(!t.contains("(1 + [I]/Ki = 1)"));
    }

    /// No value that is an answer reaches the student copy: not the drawing, not the description.
    #[test]
    fn the_student_copy_carries_no_value_and_names_become_blank_lines() {
        let temp = [s("against", "Temperature"), b("curve2", true), b("mark_den", true)];
        for mode in LABEL_MODES {
            let mut l = temp.to_vec();
            l.extend([s("opt_label", mode), s("den_label", mode), b(SHOW_ANSWERS, false)]);
            let svg = build(&with(&l)).0;
            let t = texts(&svg).join("\n");
            for v in ["37", "47", "75", "100 \u{b0}C"] {
                // the axis numbers are givens (0 20 40 ... 100); only a value on a label would be an answer
                assert!(!t.lines().any(|x| x.contains(&format!("{v} \u{b0}C")) || x.contains(&format!(": {v}"))), "{mode}: {v} leaked in {t}");
            }
            assert!(!t.contains("Optimum A") || mode == "Name and value", "{mode}: a name only label is the student's to write: {t}");
            let desc = texts(&svg).into_iter().find(|x| x.starts_with("Rate of reaction against")).unwrap();
            assert!(!desc.chars().any(|c| c.is_ascii_digit()), "{desc}");
        }
        // Name and value: the name stays and the value is a blank in the unit shown
        let t = texts(&build(&with(&[s("against", "pH"), s("opt_label", "Name and value"), b(SHOW_ANSWERS, false)])).0).join("\n");
        assert!(t.contains(&format!("Optimum: pH {BLANK}")), "{t}");
        let t = texts(&build(&with(&[s("against", "Temperature"), s("opt_label", "Name and value"), b(SHOW_ANSWERS, false)])).0).join("\n");
        assert!(t.contains(&format!("Optimum: {BLANK} \u{b0}C")), "{t}");
        // the key: the same label in degrees and in pH
        let t = texts(&build(&with(&[s("against", "Temperature"), s("opt_label", "Name and value")])).0).join("\n");
        assert!(t.contains("Optimum: 37 \u{b0}C"), "{t}");
        let t = texts(&build(&with(&[s("against", "pH"), s("enz1", "Pepsin"), s("opt_label", "Name and value")])).0).join("\n");
        assert!(t.contains("Optimum: pH 2"), "{t}");
        // against substrate with several curves, every Vmax and Km value is blanked and the inhibition factor too
        let svg = build(&with(&[b("curve2", true), b("curve3", true), f("inh", 2.0), b(SHOW_ANSWERS, false)])).0;
        let t = texts(&svg).join("\n");
        assert!(!t.contains("= 6") && !t.contains("3.33") && !t.contains("Ki = 3)") && t.contains(&format!("Km B = {BLANK}")), "{t}");
    }

    /// A label left blank is drawn as a line, never as the name.
    #[test]
    fn a_blank_label_is_a_line_and_none_leaves_the_mark_unlabelled() {
        let svg = build(&with(&[s("against", "Temperature"), s("opt_label", "Blank line")])).0;
        assert!(!texts(&svg).iter().any(|t| t.contains("Optimum")), "{svg}");
        assert!(svg.contains(&format!(r##"stroke="{INK}" stroke-width="1.4""##)));
        let none = build(&with(&[s("against", "Temperature"), s("opt_label", "None")]));
        assert!(none.1.labels.is_empty() && none.1.dots.len() == 1, "the dot and its dashed line stay");
    }

    /// Every combination sweeps the layout: labels never overprint each other, a dot or a tick number, and all text
    /// stays on the canvas at every label size, with long names.
    #[test]
    fn no_label_overprints_another_or_a_dot_and_all_text_stays_on_the_canvas() {
        let mut n = 0;
        let sets: Vec<Vec<(&'static str, Value)>> = vec![
            vec![s("against", "Temperature")],
            vec![s("against", "Temperature"), b("curve2", true), b("curve3", true), s("enz3", "Unknown enzyme"), f("uopt_t3", 70.0)],
            vec![s("against", "Temperature"), b("curve2", true), s("enz2", "Pepsin"), b("curve3", true), s("enz3", "Trypsin")],
            vec![s("against", "pH"), s("enz1", "Pepsin"), b("curve2", true), s("enz2", "Salivary amylase"), b("curve3", true), s("enz3", "Trypsin")],
            vec![s("against", "pH"), s("enz1", "Catalase"), b("curve2", true), s("enz2", "Human enzyme"), b("curve3", true), s("enz3", "Salivary amylase")],
            vec![s("against", "pH"), s("enz1", "Unknown enzyme"), f("uopt_ph1", 13.5), f("uwid_ph1", 0.5), b("curve2", true), s("enz2", "Unknown enzyme"), f("uopt_ph2", 0.5)],
            vec![b("curve2", true), b("curve3", true)],
            vec![b("curve2", true), s("mode2", "noncompetitive"), f("inh", 10.0), f("ki", 0.2), f("km", 0.2)],
            vec![b("curve3", true), f("km", 20.0), f("smax", 40.0)],
        ];
        for set in &sets {
            for size in ["Small", "Medium", "Large"] {
                for mode in LABEL_MODES {
                    for names in [false, true] {
                        let mut l = set.clone();
                        l.extend([s("label_size", size), s("opt_label", mode), s("den_label", mode), s("vk_label", mode), b("mark_den", true)]);
                        if names {
                            l.extend([s("name1", "A rather long enzyme name"), s("name2", "Another quite long one"), s("name3", "And a third long name"), s("opt_text", "Works best here")]);
                        }
                        let p = with(&l);
                        let (svg, rep) = build(&p);
                        let (w, h) = crate::figures::svg_size(&svg);
                        for (i, (a, ta)) in rep.labels.iter().enumerate() {
                            assert!(a.0 >= 0.0 && a.1 <= w && a.2 >= 0.0 && a.3 <= h, "{l:?}: {ta} off the canvas");
                            for (bb, tb) in &rep.labels[i + 1..] {
                                assert!(!super::super::hits(*a, *bb), "{l:?}: {ta} overprints {tb}");
                            }
                            for d in &rep.dots {
                                let db = (d.0 - 4.5, d.0 + 4.5, d.1 - 4.5, d.1 + 4.5);
                                assert!(!super::super::hits(*a, db), "{l:?}: {ta} over a dot at {d:?}");
                            }
                        }
                        // every text's estimated box is on the canvas (the key, the titles, the givens)
                        let fs = 12.0 * svg::label_factor(size);
                        assert!(fs >= 8.0);
                        for t in texts(&svg).iter().filter(|t| !t.starts_with("Rate of reaction against") && !t.starts_with("Reaction rate against")) {
                            assert!(t.chars().count() as f64 * fs * 0.55 <= w, "{t} wider than the canvas");
                        }
                        n += 1;
                    }
                }
            }
        }
        assert!(n > 200);
    }

    #[test]
    fn every_new_control_changes_the_drawing() {
        let base = [s("against", "Temperature"), b("curve2", true)];
        let r0 = build(&with(&base)).0;
        let changes: Vec<(&'static str, Value)> = vec![
            s("against", "pH"), s("enz1", "Pepsin"), s("name1", "Mine"), b("curve2", false), s("enz2", "Trypsin"), b("curve3", true),
            b("mark_opt", false), s("opt_label", "Blank line"), s("opt_text", "Peak"), b("mark_den", true), b("gridlines", false),
            s("title", "Enzymes"), s("xtitle", "T"), s("xtitle_style", "None"), s("ytitle_style", "Blank line"), s("ytitle", "v"),
            s("axes", "Shape only"), b("y_fit", false), b("arrows", true), s("style", "Print"), s("label_size", "Large"),
        ];
        for c in changes {
            let mut l = base.to_vec();
            l.push(c.clone());
            assert_ne!(build(&with(&l)).0, r0, "{c:?} changed nothing");
        }
        // an unknown enzyme's numbers, and the inhibitor of curves B and C against substrate
        let u = [s("against", "Temperature"), s("enz1", "Unknown enzyme")];
        let r = build(&with(&u)).0;
        for c in [f("uopt_t1", 60.0), f("uwid_t1", 20.0)] { let mut l = u.to_vec(); l.push(c.clone()); assert_ne!(build(&with(&l)).0, r, "{c:?}"); }
        let u = [s("against", "pH"), s("enz1", "Unknown enzyme")];
        let r = build(&with(&u)).0;
        for c in [f("uopt_ph1", 4.0), f("uwid_ph1", 1.0)] { let mut l = u.to_vec(); l.push(c.clone()); assert_ne!(build(&with(&l)).0, r, "{c:?}"); }
        let m = [b("curve2", true), b("curve3", true)];
        let r = build(&with(&m)).0;
        for c in [s("mode2", "none"), s("mode3", "competitive"), s("vk_label", "Name only"), s("name3", "X")] { let mut l = m.to_vec(); l.push(c.clone()); assert_ne!(build(&with(&l)).0, r, "{c:?}"); }
        // and on the single Michaelis-Menten figure
        let r = (def().render)(&with(&[]));
        for c in [s("vk_label", "Blank line"), s("vk_label", "None"), s("title", "T"), s("axes", "Shape only"), b("gridlines", false), b("arrows", true), b("y_fit", false), s("xtitle_style", "Blank line"), s("ytitle", "Rate")] {
            assert_ne!((def().render)(&with(&[c.clone()])), r, "{c:?}");
        }
    }

    /// Dragging an unknown enzyme's optimum, through the hit test the workspace uses, over several layouts.
    #[test]
    fn an_unknown_enzymes_optimum_drags_through_the_real_hit_test() {
        for (a, key, set) in [
            ("Temperature", "uopt_t1", vec![s("enz1", "Unknown enzyme")]),
            ("Temperature", "uopt_t2", vec![b("curve2", true), s("enz2", "Unknown enzyme"), s("label_size", "Large")]),
            ("pH", "uopt_ph1", vec![s("enz1", "Unknown enzyme"), b("curve3", true)]),
            ("pH", "uopt_ph3", vec![b("curve3", true), s("enz3", "Unknown enzyme"), s("title", "Title")]),
        ] {
            let mut l = vec![s("against", a)];
            l.extend(set);
            let p = with(&l);
            let svg = (def().render)(&p);
            let size = crate::figures::svg_size(&svg);
            let hs = interact::handles_for(ID, &p, size);
            assert_eq!(hs.len(), 1, "{l:?}");
            let h = &hs[0];
            assert!(h.tip.contains("optimum"));
            // a sweep of presses round the dot reaches the handle; far away does not
            let mut hit = 0;
            for dx in -8..=8 {
                for dy in -8..=8 {
                    if interact::pick(&hs, (h.pos.0 + dx as f64, h.pos.1 + dy as f64), 1.0) == Some(0) { hit += 1; }
                }
            }
            assert_eq!(hit, 17 * 17, "{l:?}");
            assert_eq!(interact::pick(&hs, (h.pos.0 + 40.0, h.pos.1 + 40.0), 1.0), None);
            // drag to the pixel of a value and the optimum lands on it
            let rep = build(&p).1;
            let (lft, span, x0, x1, _, _) = rep.frame;
            let target = if a == "Temperature" { 52.0 } else { 3.4 };
            let mut q = p.clone();
            (h.drag.as_ref().unwrap())(&mut q, (lft + (target - x0) / (x1 - x0) * span, h.pos.1));
            assert!((q.f(key) - target).abs() < 1e-9, "{key}: {}", q.f(key));
            // and the curve now peaks there
            let slot = key.chars().last().unwrap().to_digit(10).unwrap() as usize - 1;
            assert!((curves(&q).iter().find(|c| c.slot == slot).unwrap().opt - target).abs() < 1e-9);
        }
        // named enzymes are not draggable: their optimum is a fact
        let p = with(&[s("against", "Temperature")]);
        assert!(interact::handles_for(ID, &p, crate::figures::svg_size(&(def().render)(&p))).is_empty());
    }

    #[test]
    fn print_draws_no_hue() {
        for l in [
            vec![s("against", "pH"), b("curve2", true), b("curve3", true), b("mark_den", true), s("style", "Print"), b("arrows", true), s("opt_label", "Blank line")],
            vec![b("curve2", true), b("curve3", true), s("style", "Print"), s("vk_label", "Blank line")],
        ] {
            let svg = build(&with(&l)).0;
            for part in svg.split('"').filter(|x| x.starts_with('#') && x.len() == 7 && x[1..].chars().all(|c| c.is_ascii_hexdigit())) {
                assert!(part[1..3] == part[3..5] && part[3..5] == part[5..7], "Print draws {part}");
            }
        }
    }

    #[test]
    fn the_editor_notes_name_each_enzymes_optimum_and_a_cut_off_axis() {
        let p = with(&[s("against", "Temperature"), b("curve2", true)]);
        assert_eq!(info(&p).unwrap(), "Human enzyme: works best at 37 \u{b0}C and has denatured by 47 \u{b0}C\nTaq polymerase: works best at 75 \u{b0}C and has denatured by 100 \u{b0}C");
        assert!(notes(&p).is_empty(), "information is not a warning");
        let p = with(&[s("against", "pH"), s("enz1", "Trypsin")]);
        assert_eq!(info(&p).unwrap(), "Trypsin: works best at pH 8, and only between pH 5 and pH 11");
        // pepsin and the human enzyme share a curve against temperature: said, not hidden
        let p = with(&[s("against", "Temperature"), s("enz1", "Pepsin"), b("curve2", true), s("enz2", "Human enzyme")]);
        assert!(info(&p).unwrap().contains("Pepsin and Human enzyme have the same curve here, so curve B is drawn exactly over curve A."), "{:?}", info(&p));
        assert!(!info(&with(&[s("against", "Temperature"), b("curve2", true)])).unwrap().contains("same curve"));
        // and on the figure the shared dot carries one label naming both curves
        let labels: Vec<String> = build(&p).1.labels.into_iter().map(|(_, t)| t).collect();
        assert_eq!(labels, ["Optimum A, B"]);
        assert!(info(&with(&[])).is_none() && intro(&with(&[])).is_none());
        assert!(intro(&with(&[s("against", "pH")])).unwrap().starts_with("Rate of reaction against pH"));
        let p = with(&[s("against", "pH"), b("y_fit", false), f("y_to", 8.0)]);
        assert!(axis_note(&p).is_some() && build(&p).1.dots.is_empty());
        assert!(axis_note(&with(&[s("against", "pH")])).is_none());
    }

    #[test]
    fn the_examples_and_every_choice_render() {
        for e in examples(ID).unwrap() {
            let svg = (def().render)(&with(&e.params));
            assert!(svg.len() > 2000, "{}", e.name);
        }
        for a in ["Temperature", "pH", "Substrate concentration"] {
            for e in ENZYMES {
                for n in 1..=3 {
                    let k: &'static str = ["enz1", "enz2", "enz3"][n - 1];
                    let svg = (def().render)(&with(&[s("against", a), b("curve2", true), b("curve3", true), s(k, e)]));
                    assert!(svg.contains("<polyline"), "{a} {e}");
                }
            }
        }
    }

    /// Every text of an SVG as (x, y, size, anchor, bold, rotated, text).
    fn text_items(svg: &str) -> Vec<(f64, f64, f64, String, bool, bool, String)> {
        let attr = |tag: &str, k: &str| -> Option<String> { let i = tag.find(&format!(" {k}=\""))? + k.len() + 3; Some(tag[i..i + tag[i..].find('"')?].to_string()) };
        let mut out = Vec::new();
        let mut rest = svg;
        while let Some(i) = rest.find("<text") {
            rest = &rest[i..];
            let Some(j) = rest.find('>') else { break };
            let tag = &rest[..j];
            let body = &rest[j + 1..];
            let Some(k) = body.find("</text>") else { break };
            let text = body[..k].to_string();
            rest = &body[k..];
            if tag.contains(r##"fill="none""##) { continue; } // a halo, drawn again with its fill
            let size: f64 = attr(tag, "font-size").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let anchor = attr(tag, "text-anchor").unwrap_or_default();
            let bold = tag.contains("font-weight=\"bold\"");
            if let Some(t) = attr(tag, "transform") {
                let nums: Vec<f64> = t.trim_start_matches("translate(").split(|c| c == ' ' || c == ')').filter_map(|v| v.parse().ok()).collect();
                out.push((nums[0], nums[1], size, anchor, bold, true, text));
            } else {
                let x: f64 = attr(tag, "x").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let y: f64 = attr(tag, "y").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                out.push((x, y, size, anchor, bold, false, text));
            }
        }
        out
    }

    /// The y axis line (x, top, bottom): the longest vertical 1.6 px ink line.
    fn y_axis(svg: &str) -> (f64, f64, f64) {
        let mut best = (0.0, 0.0, 0.0);
        for part in svg.split("<line ").skip(1) {
            let g = |k: &str| -> f64 { let i = part.find(&format!("{k}=\"")).unwrap() + k.len() + 2; part[i..i + part[i..].find('"').unwrap()].parse().unwrap() };
            if !part.contains("stroke-width=\"1.6\"") { continue; }
            let (x1, y1, x2, y2) = (g("x1"), g("y1"), g("x2"), g("y2"));
            if (x1 - x2).abs() < 1e-6 && (y2 - y1).abs() > best.2 - best.1 { best = (x1, y1.min(y2), y1.max(y2)); }
        }
        best
    }

    /// The longest titles the boxes take (80 characters), at every label size, on every kind of figure: every line
    /// stays on the canvas, the y title stays within the plot's height and clear of the tick numbers, no line under
    /// the plot runs into another, and no text is drawn under 8 px.
    #[test]
    fn the_longest_titles_wrap_and_stay_on_the_canvas_at_every_size() {
        let words = "The effect of temperature on the rate of an enzyme-controlled reaction in humans and bacteria";
        let long: String = words.chars().take(80).collect();
        let yt: String = "Volume of oxygen released per minute (cm\u{b3}/min) by catalase from potato and liver cells".chars().take(80).collect();
        let unbroken: String = "Pneumonoultramicroscopicsilicovolcanoconiosis".repeat(2).chars().take(80).collect();
        let mut n = 0;
        for base in [
            vec![],
            vec![s("mode", "competitive")],
            vec![s("against", "Temperature"), b("curve2", true), b("curve3", true)],
            vec![s("against", "pH"), b("curve2", true), b("mark_den", true)],
            vec![b("curve2", true), b("curve3", true)],
        ] {
            for size in ["Small", "Medium", "Large"] {
                for (title, x, y) in [(long.as_str(), long.as_str(), yt.as_str()), (unbroken.as_str(), unbroken.as_str(), unbroken.as_str()), ("Rate", "pH", "v")] {
                    let mut l = base.clone();
                    l.extend([s("label_size", size), s("title", title), s("xtitle", x), s("ytitle", y)]);
                    let svg = (def().render)(&with(&l));
                    let (w, h) = crate::figures::svg_size(&svg);
                    let (ax, top, bottom) = y_axis(&svg);
                    let items = text_items(&svg);
                    let mut under: Vec<((f64, f64, f64, f64), String)> = Vec::new();
                    for (tx, ty, fs, anchor, bold, rot, t) in &items {
                        assert!(*fs >= 8.0, "{l:?}: {t} at {fs} px");
                        let len = t.chars().count() as f64 * fs * if *bold { 0.6 } else { 0.55 };
                        if *rot {
                            assert!(ty - len / 2.0 >= top - 2.0 && ty + len / 2.0 <= bottom + 2.0, "{l:?}: y title line {t} runs past the plot ({top}..{bottom})");
                            assert!(tx - 0.8 * fs >= 0.0, "{l:?}: y title off the left edge");
                            // clear of the widest tick number (3 characters, 12 px scaled) ending 8 px left of the axis
                            let tick_left = ax - 8.0 - 3.0 * 12.0 * svg::label_factor(size) * 0.55;
                            assert!(tx + 0.3 * fs <= tick_left, "{l:?}: y title line {t} at {tx} runs into the tick numbers at {tick_left}");
                        } else {
                            let (x0, x1) = match anchor.as_str() { "middle" => (tx - len / 2.0, tx + len / 2.0), "end" => (tx - len, *tx), _ => (*tx, tx + len) };
                            assert!(x0 >= -0.5 && x1 <= w + 0.5, "{l:?}: {t} off the canvas ({x0:.1}..{x1:.1} of {w})");
                            assert!(*ty - fs * 0.8 >= 0.0 && *ty + fs * 0.25 <= h, "{l:?}: {t} off the canvas vertically");
                            if *ty > bottom + 4.0 { under.push(((x0, x1, ty - fs * 0.74, ty + fs * 0.22), t.clone())); }
                            if *bold { assert!(*ty < top, "{l:?}: the chart title {t} runs into the plot"); }
                        }
                    }
                    for (i, (a, ta)) in under.iter().enumerate() {
                        for (bb, tb) in &under[i + 1..] {
                            assert!(!super::super::hits(*a, *bb), "{l:?}: {ta} runs into {tb}");
                        }
                    }
                    n += 1;
                }
            }
        }
        assert_eq!(n, 45);
        // the splitter: at spaces when it can, through a word only when the word alone is longer than a line
        assert_eq!(split_lines("one two three", 7), ["one two", "three"]);
        assert_eq!(split_lines("abcdefghij k", 4), ["abcd", "efgh", "ij k"]);
    }

    /// A title that fits is drawn exactly as before the wrapping existed: one line, same place.
    #[test]
    fn a_title_that_fits_keeps_its_place() {
        let one = |l: &[(&'static str, Value)]| (def().render)(&with(l));
        let svg = one(&[s("title", "Michaelis-Menten"), s("ytitle", "Rate"), s("xtitle", "[S] (mmol/L)")]);
        assert!(svg.contains(r##"<text transform="translate(18.00 "##) && svg.matches("rotate(-90)").count() == 1);
        assert_eq!(svg.matches("Michaelis-Menten</text>").count(), 1);
        assert_eq!(y_axis(&svg).0, 66.0);
    }
}
