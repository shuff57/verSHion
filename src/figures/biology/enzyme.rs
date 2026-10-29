//! Enzyme kinetics: the Michaelis-Menten curve of reaction rate against substrate concentration, with Vmax as the
//! asymptote it approaches and Km as the concentration that reaches half of it — unchanged, or moved by a competitive
//! inhibitor (Km rises, Vmax does not) or a noncompetitive one (Vmax falls, Km does not). Drawn with `svg::Doc` only.
//!
//! The curve, Vmax, Km and the rate at the marked concentration are the answers, so none of them is drawn while Show
//! answers is off; the axes, their numbers and the givens stay, so the student can sketch the curve and mark the two
//! constants themselves.
use super::super::circle::num;
use super::super::chart::{fmt, nice_step, ticks, Frame};
use super::*;
use crate::figures::interact::Handle;

pub const ID: &str = "enzyme-kinetics";

const INK: &str = "#111111";
const CURVE: &str = "#1f5fbf";
const CURVE_PRINT: &str = "#333333";
const MARK: &str = "#c0392b";
const MARK_PRINT: &str = "#555555";
const BLANK: &str = "_____";
const W: f64 = 620.0;
const H: f64 = 330.0;
const ROW_FS: f64 = 15.0;
const ROW_H: f64 = 20.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Enzyme Kinetics",
        blurb: "The Michaelis-Menten curve of rate against substrate, with Vmax and Km marked, and what a competitive or noncompetitive inhibitor does to each.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("vmax", "Vmax (rate units)", 1.0, 100.0, 1.0, 10.0),
        ParamDef::float("km", "Km (concentration units)", 0.2, 20.0, 0.1, 2.0),
        ParamDef::float("smax", "Highest [S] shown", 2.0, 40.0, 1.0, 12.0),
        ParamDef::float("at", "Mark the rate at [S] =", 0.0, 40.0, 0.5, 6.0),
        ParamDef::choice("mode", "Inhibitor", &["none", "competitive", "noncompetitive"], "none"),
        ParamDef::float("inh", "Inhibitor concentration [I]", 0.0, 10.0, 0.5, 3.0),
        ParamDef::float("ki", "Inhibition constant Ki", 0.2, 10.0, 0.1, 1.0),
        ParamDef::int("dec", "Decimal places", 1, 3, 2),
        answers_param().hint("Off keeps the axes and blanks the curve, Vmax, Km and the marked rate"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(v: f64, lo: f64, hi: f64, d: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { d } }

/// (Vmax, Km, highest [S] drawn, concentration to mark, inhibition factor 1 + [I]/Ki, inhibitor on).
pub fn given(p: &Params) -> (f64, f64, f64, f64, f64, bool) {
    let mode = p.s("mode");
    let inhibited = mode != "none";
    let factor = if inhibited {
        let i = fin(p.f("inh"), 0.0, 10.0, 3.0);
        let ki = fin(p.f("ki"), 0.2, 10.0, 1.0);
        1.0 + i / ki
    } else {
        1.0
    };
    (
        fin(p.f("vmax"), 1.0, 100.0, 10.0),
        fin(p.f("km"), 0.2, 20.0, 2.0),
        fin(p.f("smax"), 2.0, 40.0, 12.0),
        p.f("at"),
        factor,
        inhibited,
    )
}

/// The curve in force: Vmax and Km as the inhibitor leaves them. A competitive inhibitor binds the free enzyme, so
/// it takes more substrate to reach half the rate (Km' = Km (1 + [I]/Ki)) and Vmax is unchanged; a noncompetitive one
/// takes some enzyme out of play, so Vmax' = Vmax / (1 + [I]/Ki) and Km is unchanged.
pub fn kinetics(p: &Params) -> (f64, f64) {
    let (vmax, km, _, _, factor, on) = given(p);
    if !on { return (vmax, km); }
    if p.s("mode") == "competitive" { (vmax, km * factor) } else { (vmax / factor, km) }
}

/// The Michaelis-Menten rate at concentration `s`.
pub fn rate(vmax: f64, km: f64, s: f64) -> f64 { vmax * s / (km + s).max(1e-9) }

/// The concentration marked in the figure, held inside the axis.
pub fn marked(p: &Params) -> f64 {
    let (_, _, smax, at, _, _) = given(p);
    if at.is_finite() { at.clamp(0.0, smax) } else { smax / 2.0 }
}

pub fn warnings(p: &Params) -> Vec<String> {
    let (_, _, smax, at, _, _) = given(p);
    let (_, km_a) = kinetics(p);
    let mut out = Vec::new();
    if at.is_finite() && at > smax + 1e-9 {
        out.push(format!("The marked concentration is beyond the axis, so it is drawn at [S] = {}.", num(smax, 1)));
    }
    if km_a > smax + 1e-9 {
        out.push(format!("Km is {} but the axis stops at [S] = {}, so the half-rate line runs to the edge and Km is named there.", num(km_a, 2), num(smax, 1)));
    }
    out
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}">{}</text>"##,
        svg::esc(s)
    )
}

fn val(v: f64, dec: i64) -> String {
    let zeros = |s: &str| s.trim_start_matches('\u{2212}').chars().filter(|c| c.is_ascii_digit()).all(|c| c == '0');
    let s = num(v, dec);
    if v != 0.0 && zeros(&s) {
        let more = num(v, dec + 2);
        if !zeros(&more) { return more; }
        let six = format!("{:.6}", v);
        let six = six.trim_end_matches('0').trim_end_matches('.');
        if !zeros(six) { return six.to_string(); }
        return format!("{v:.2e}").replace('-', "\u{2212}");
    }
    s
}

pub struct Report {
    /// The frame's mapping, so the drag inverts exactly.
    pub frame: (f64, f64, f64, f64, f64, f64),
    /// Where the marked point is drawn.
    pub point: (f64, f64),
}

pub fn build(p: &Params) -> (String, Report) {
    let (vmax, km, smax, _, _, inhibited) = given(p);
    let (vmax_a, km_a) = kinetics(p);
    let s_at = marked(p);
    let v_at = rate(vmax_a, km_a, s_at);
    let dec = p.i("dec").clamp(1, 3);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let lab = svg::label_factor(p.s("label_size"));
    let ink = if print { "#555555" } else { INK };
    let curve = if print { CURVE_PRINT } else { CURVE };
    let mark = if print { MARK_PRINT } else { MARK };

    // the y axis runs a little above Vmax so the asymptote is visibly an asymptote
    let y_top = (if inhibited { vmax.max(vmax_a) } else { vmax }) * 1.15;
    // the canvas grows with the labels: at Large the givens line with an inhibitor ran off the right edge
    let w = (W * (1.0 + 0.45 * (lab - 1.0))).max(W);
    let f = Frame { w, h: H, l: 66.0, r: 20.0, t: 22.0, b: 52.0, x0: 0.0, x1: smax, y0: 0.0, y1: y_top };

    let rows = if inhibited { 3.0 } else { 1.0 };
    let rows_y0 = H + 46.0 * lab;
    let mut d = Doc::new(w, rows_y0 + (rows - 1.0) * ROW_H * lab + 16.0);
    d.describe("Enzyme kinetics", "Reaction rate against substrate concentration with Vmax and Km marked");

    // axes and gridlines
    let xs = nice_step(smax, 6.0);
    let ys = nice_step(y_top, 5.0);
    for (v, label) in ticks(f.x0, f.x1, xs) {
        let x = f.px(v);
        d.line(x, f.top(), x, f.bottom(), "#e2e2e2", 1.0);
        d.raw(&text_el(x, f.bottom() + 18.0 * lab, 12.0 * lab, "middle", ink, &label));
    }
    for (v, label) in ticks(f.y0, f.y1, ys) {
        let y = f.py(v);
        d.line(f.left(), y, f.right(), y, "#e2e2e2", 1.0);
        d.raw(&text_el(f.left() - 8.0, y + 4.0 * lab, 12.0 * lab, "end", ink, &label));
    }
    d.line(f.left(), f.bottom(), f.right(), f.bottom(), ink, 1.6);
    d.line(f.left(), f.top(), f.left(), f.bottom(), ink, 1.6);
    d.raw(&text_el((f.left() + f.right()) / 2.0, f.bottom() + 40.0 * lab, 13.0 * lab, "middle", ink, "Substrate concentration [S]"));
    d.raw(&format!(
        r##"<text transform="translate(18.00 {:.2}) rotate(-90)" font-size="{}" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">Reaction rate v</text>"##,
        (f.top() + f.bottom()) / 2.0,
        13.0 * lab,
        ink
    ));

    // the curve and its two constants — the answer, so absent from the student copy
    if ans {
        let mut pts = Vec::new();
        for k in 0..=160 {
            let s = smax * k as f64 / 160.0;
            pts.push((f.px(s), f.py(rate(vmax_a, km_a, s))));
        }
        let poly: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
        d.raw(&format!(
            r##"<polyline points="{}" fill="none" stroke="{curve}" stroke-width="2.4" stroke-linejoin="round" stroke-linecap="round"/>"##,
            poly.join(" ")
        ));
        // the curve without the inhibitor, dashed underneath: competitive raises Km (stretched to the right),
        // noncompetitive lowers Vmax (squashed down) — a claim the picture should show, not only the caption
        if inhibited {
            let mut pts0 = Vec::new();
            for k in 0..=160 {
                let s = smax * k as f64 / 160.0;
                pts0.push((f.px(s), f.py(rate(vmax, km, s))));
            }
            let poly0: Vec<String> = pts0.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
            d.raw(&format!(
                r##"<polyline points="{}" fill="none" stroke="{mark}" stroke-width="1.6" stroke-dasharray="6 5" stroke-linejoin="round" stroke-linecap="round"/>"##,
                poly0.join(" ")
            ));
        }
        // Vmax as the dashed asymptote
        let vy = f.py(vmax_a);
        d.raw(&format!(r##"<line x1="{:.2}" y1="{vy:.2}" x2="{:.2}" y2="{vy:.2}" stroke="{curve}" stroke-width="1.4" stroke-dasharray="7 5"/>"##, f.left(), f.right()));
        d.raw(&text_el(f.right() - 4.0, vy - 8.0 * lab, 12.0 * lab, "end", curve, &format!("Vmax = {}", val(vmax_a, dec))));
        // Km: the concentration that reaches half of it — or, past the right-hand edge, named there, because an
        // inhibitor can push Km far beyond the axis the curve is drawn on
        let half = f.py(vmax_a / 2.0);
        if km_a <= smax {
            let kx = f.px(km_a);
            d.raw(&format!(r##"<line x1="{kx:.2}" y1="{:.2}" x2="{kx:.2}" y2="{half:.2}" stroke="{curve}" stroke-width="1.2" stroke-dasharray="3 4"/>"##, f.bottom()));
            d.raw(&format!(r##"<line x1="{:.2}" y1="{half:.2}" x2="{kx:.2}" y2="{half:.2}" stroke="{curve}" stroke-width="1.2" stroke-dasharray="3 4"/>"##, f.left()));
            d.raw(&text_el(kx, f.bottom() - 6.0 * lab, 12.0 * lab, "middle", curve, &format!("Km = {}", val(km_a, dec))));
        } else {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{half:.2}" x2="{:.2}" y2="{half:.2}" stroke="{curve}" stroke-width="1.2" stroke-dasharray="3 4"/>"##, f.left(), f.right()));
            let note = format!("Km = {}, beyond this axis", val(km_a, dec));
            d.raw(&text_el(f.right() - 4.0, half - 7.0 * lab, 12.0 * lab, "end", curve, &note));
        }
        let half_label = format!("Vmax/2 = {}", val(vmax_a / 2.0, dec));
        // the Km marker and this label both sit low and near the left when Km is small: put the label right of the
        // marker rather than on top of it
        let hx = if km_a <= smax && f.px(km_a) < f.left() + 118.0 { f.px(km_a) + 8.0 } else { f.left() + 6.0 };
        d.raw(&text_el(hx, half - 7.0 * lab, 11.0 * lab, "start", curve, &half_label));
        // the marked point
        let px = f.px(s_at);
        let py = f.py(v_at);
        d.circle(px, py, 4.5, mark, ink, 1.2);
        let label = format!("v = {} at [S] = {}", val(v_at, dec), val(s_at, dec));
        // the label reads to the right of the point; it only flips to the left when that would run off the canvas
        let text_w = label.chars().count() as f64 * 12.0 * lab * 0.55;
        let (lx, anchor) = if px + 8.0 + text_w <= f.right() { (px + 8.0, "start") } else { (px - 8.0, "end") };
        // close under the Vmax line there is no room above the point, and the label would print into the dashed line
        let above = py - 9.0 * lab;
        let ly = if (above - vy).abs() < 14.0 * lab { py + 18.0 * lab } else { above };
        d.raw(&text_el(lx, ly, 12.0 * lab, anchor, mark, &label));
    }

    // the givens, always printed: they are what the curve is drawn from
    let mode = p.s("mode");
    let givens = if inhibited {
        let (_, _, _, _, factor, _) = given(p);
        format!(
            "Vmax = {}, Km = {}, {}, [I] = {}, Ki = {} (1 + [I]/Ki = {})",
            val(vmax, dec),
            val(km, dec),
            mode,
            val(fin(p.f("inh"), 0.0, 10.0, 3.0), 1),
            val(fin(p.f("ki"), 0.2, 10.0, 1.0), 1),
            val(factor, 2)
        )
    } else {
        format!("Vmax = {}, Km = {}, curve drawn to [S] = {}", val(vmax, dec), val(km, dec), val(smax, dec))
    };
    let chars_per_line = ((w - 48.0) / (12.0 * lab * 0.55)).floor().max(20.0) as usize;
    for (i, line) in super::wrap(&givens, chars_per_line).iter().enumerate() {
        d.raw(&text_el(24.0, H + (18.0 + i as f64 * 15.0) * lab, 12.0 * lab, "start", ink, line));
    }

    // the readout. Without an inhibitor Vmax and Km are already given, so only the marked rate is repeated here;
    // with one, the apparent pair is the whole point and both pairs are shown
    let (vmax_0, km_0) = (vmax, km);
    let mut lines: Vec<String> = Vec::new();
    if inhibited {
        lines.push(if ans {
            format!("With the inhibitor: Vmax = {}, Km = {}", val(vmax_a, dec), val(km_a, dec))
        } else {
            format!("With the inhibitor: Vmax = {BLANK}, Km = {BLANK}")
        });
        lines.push(if ans {
            format!("Without it: Vmax = {}, Km = {}", val(vmax_0, dec), val(km_0, dec))
        } else {
            format!("Without it: Vmax = {BLANK}, Km = {BLANK}")
        });
    }
    lines.push(if ans {
        format!("Rate at [S] = {}  v = {}", val(s_at, dec), val(v_at, dec))
    } else {
        format!("Rate at [S] = {}  v = {BLANK}", val(s_at, dec))
    });
    for (i, line) in lines.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            rows_y0 + i as f64 * ROW_H * lab,
            ROW_FS * lab,
            ink,
            svg::esc(line)
        ));
    }

    let point = (f.px(s_at), f.py(v_at));
    (d.finish(), Report { frame: (f.l, f.w - f.l - f.r, f.x0, f.x1, f.t, f.h - f.t - f.b), point })
}

/// The marked point is draggable along the x axis: it sets the concentration whose rate is read off.
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let rep = build(p).1;
    let (l, span, x0, x1, _, _) = rep.frame;
    Some(vec![Handle::knob(rep.point, "Drag the point to read the rate at another substrate concentration", move |p, pt| {
        if !pt.0.is_finite() { return; }
        let v = x0 + (pt.0 - l) / span.max(1e-9) * (x1 - x0);
        let v = (v * 10.0).round() / 10.0;
        p.set_f("at", v.clamp(x0, x1));
    })])
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "A typical enzyme: Vmax 10, Km 2", params: vec![] },
        Example { name: "A low-Km enzyme: saturated early", params: vec![f("km", 0.5), f("at", 2.0)] },
        Example { name: "A high-Km enzyme: needs a lot of substrate", params: vec![f("km", 10.0), f("smax", 40.0), f("at", 20.0)] },
        Example { name: "Competitive inhibition raises Km", params: vec![s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0)] },
        Example { name: "Noncompetitive inhibition lowers Vmax", params: vec![s("mode", "noncompetitive"), f("inh", 4.0), f("ki", 1.0)] },
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
    fn the_curve_is_michaelis_menten() {
        let p = with(&[]);
        let (vmax, km) = kinetics(&p);
        assert_eq!((vmax, km), (10.0, 2.0));
        // v = Vmax [S] / (Km + [S])
        for s in [0.0, 0.5, 2.0, 6.0, 12.0, 100.0] {
            let want = 10.0 * s / (2.0 + s);
            assert!((rate(vmax, km, s) - want).abs() < 1e-12, "s = {s}");
        }
        // at Km the rate is exactly half of Vmax
        assert!((rate(vmax, km, km) - vmax / 2.0).abs() < 1e-12);
        // and the curve approaches Vmax from below without reaching it
        assert!(rate(vmax, km, 1e6) < vmax && rate(vmax, km, 1e6) > vmax * 0.999);
    }

    #[test]
    fn a_competitive_inhibitor_raises_km_and_leaves_vmax() {
        let p = with(&[s("mode", "competitive"), f("inh", 3.0), f("ki", 1.0)]);
        let (vmax, km) = kinetics(&p);
        assert!((vmax - 10.0).abs() < 1e-12, "{vmax}");
        assert!((km - 2.0 * 4.0).abs() < 1e-12, "{km}");
        // the rate at a huge concentration is still Vmax
        assert!((rate(vmax, km, 1e7) - 10.0).abs() < 1e-3);
    }

    #[test]
    fn a_noncompetitive_inhibitor_lowers_vmax_and_leaves_km() {
        let p = with(&[s("mode", "noncompetitive"), f("inh", 3.0), f("ki", 1.0)]);
        let (vmax, km) = kinetics(&p);
        assert!((vmax - 2.5).abs() < 1e-12, "{vmax}");
        assert!((km - 2.0).abs() < 1e-12, "{km}");
        assert!((rate(vmax, km, km) - vmax / 2.0).abs() < 1e-12);
    }

    #[test]
    fn the_pair_without_the_inhibitor_is_always_what_was_typed() {
        // the noncompetitive case once printed "Without it: Vmax = 2, Km = 10" — the shifted pair, not the typed one
        for mode in ["competitive", "noncompetitive"] {
            let p = with(&[s("mode", mode), f("inh", 4.0), f("ki", 1.0)]);
            let (vmax, km) = kinetics(&p);
            let t = texts(&render_of(&[s("mode", mode), f("inh", 4.0), f("ki", 1.0)]));
            assert!(t.contains("Without it: Vmax = 10, Km = 2"), "{mode}: {t}");
            assert!(t.contains(&format!("With the inhibitor: Vmax = {}, Km = {}", num(vmax, 2), num(km, 2))), "{mode}: {t}");
        }
        // and each inhibitor moves exactly one of the two
        let (vc, kc) = kinetics(&with(&[s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0)]));
        assert_eq!(vc, 10.0);
        assert_ne!(kc, 2.0);
        let (vn, kn) = kinetics(&with(&[s("mode", "noncompetitive"), f("inh", 4.0), f("ki", 1.0)]));
        assert_eq!(kn, 2.0);
        assert_ne!(vn, 10.0);
    }

    #[test]
    fn the_marked_point_follows_the_curve_and_the_axis() {
        let p = with(&[f("at", 6.0)]);
        let (vmax, km) = kinetics(&p);
        let rep = build(&p).1;
        let fr = Frame { w: W, h: H, l: 66.0, r: 20.0, t: 22.0, b: 52.0, x0: 0.0, x1: 12.0, y0: 0.0, y1: 10.0 * 1.15 };
        assert!((rep.point.0 - fr.px(6.0)).abs() < 1e-9, "{:?}", rep.point);
        assert!((rep.point.1 - fr.py(rate(vmax, km, 6.0))).abs() < 1e-9, "{:?}", rep.point);
        // beyond the axis the mark is held at the edge, and the editor is told
        let far = with(&[f("at", 30.0)]);
        assert!((marked(&far) - 12.0).abs() < 1e-12);
        assert!(warnings(&far).iter().any(|w| w.contains("beyond the axis")), "{:?}", warnings(&far));
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Vmax = 10") && stu.contains("Km = 2"), "the givens must stay: {stu}");
        assert!(stu.contains("Rate at [S] = 6  v = _____"), "{stu}");
        assert!(!stu.contains("v = 7.5"), "the rate leaked: {stu}");
        // with an inhibitor both pairs of constants are the answer
        let inh = texts(&render_of(&[s("mode", "competitive"), b(SHOW_ANSWERS, false)]));
        assert!(inh.contains("With the inhibitor: Vmax = _____") && inh.contains("Without it: Vmax = _____"), "{inh}");
        let svg = render_of(&[b(SHOW_ANSWERS, false)]);
        assert!(!svg.contains(&format!("stroke=\"{CURVE}\"")), "the curve is the answer too");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Rate at [S] = 6  v = 7.5"), "{key}");
    }

    #[test]
    fn a_rate_never_prints_as_zero() {
        // Km 20 with [S] 0.5 is a rate of 0.244: at one decimal place it is 0.2, never 0
        let t = texts(&render_of(&[f("km", 20.0), f("at", 0.5), ("dec", Value::I(1))]));
        assert!(t.contains("v = 0.2"), "{t}");
    }

    #[test]
    fn a_km_past_the_axis_is_named_at_the_edge() {
        // a strong competitive inhibitor at a high Km: Km_app = 100 while the axis stops at 12
        let p = with(&[s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0), f("km", 20.0)]);
        assert!((kinetics(&p).1 - 100.0).abs() < 1e-9);
        let t = texts(&render_of(&[s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0), f("km", 20.0)]));
        assert!(t.contains("Km = 100, beyond this axis"), "{t}");
        assert!(warnings(&p).iter().any(|w| w.contains("the axis stops")), "{:?}", warnings(&p));
        // and inside the axis it is still drawn at its dotted line
        let inside = with(&[]);
        assert!(texts(&render_of(&[])).contains("Km = 2"), "{}", texts(&render_of(&[])));
        assert!(warnings(&inside).is_empty());
    }

    #[test]
    fn the_drag_inverts_exactly() {
        let p = with(&[]);
        let rep = build(&p).1;
        let hs = handles(ID, &p, (W, H)).unwrap();
        assert_eq!(hs.len(), 1);
        assert!((hs[0].pos.0 - rep.point.0).abs() < 1e-9);
        // the closure maps a pointer x back to the concentration on the axis
        let mut q = p.clone();
        let (l, span, x0, x1, _, _) = rep.frame;
        let mid = l + span * 0.75;
        (hs[0].drag.as_ref().unwrap())(&mut q, (mid, 0.0));
        let want = ((x0 + (mid - l) / span * (x1 - x0)) * 10.0).round() / 10.0;
        assert!((q.f("at") - want).abs() < 1e-9, "{} vs {want}", q.f("at"));
    }

    #[test]
    fn the_inhibited_figure_draws_both_curves() {
        // the comparison the figure promises has to be on the page: the uninhibited curve dashed under the other
        let svg = render_of(&[s("mode", "competitive"), f("inh", 4.0)]);
        assert_eq!(svg.matches("<polyline").count(), 2, "only one curve is drawn");
        assert!(svg.contains(&format!("stroke-dasharray=\"6 5\"")), "the comparison curve is not dashed");
        // and a plain figure draws exactly one
        assert_eq!(render_of(&[]).matches("<polyline").count(), 1);
    }

    #[test]
    fn labels_stay_inside_the_canvas_at_the_marked_zero_and_at_large() {
        // [S] = 0.5 sits near the left edge: the rate label used to be end-anchored there and ran off the canvas
        for at in [0.0, 0.5, 12.0] {
            let svg = render_of(&[f("at", at)]);
            for line in svg.lines().filter(|l| l.contains("<text")) {
                if let Some(x) = line.split("x=\"").nth(1).and_then(|v| v.split('"').next()).and_then(|v| v.parse::<f64>().ok()) {
                    assert!((0.0..=640.0).contains(&x), "[S] = {at}: a label at x = {x}");
                }
            }
        }
        // Large with an inhibitor: the givens line must fit the wider canvas
        let wide = render_of(&[s("mode", "competitive"), f("inh", 4.0), s("label_size", "Large")]);
        assert!(texts(&wide).contains("(1 + [I]/Ki = 5)"), "the givens were cut: {}", texts(&wide));
    }

    #[test]
    fn print_is_neutral_grey_only() {
        let svg = render_of(&[s("style", "Print")]);
        for line in svg.lines().filter(|x| x.contains("stroke=\"#") || x.contains("fill=\"#")) {
            for part in line.split('"').filter(|x| x.starts_with('#') && x.len() == 7 && x[1..].chars().all(|c| c.is_ascii_hexdigit())) {
                let (r, g, bl) = (
                    u8::from_str_radix(&part[1..3], 16).unwrap(),
                    u8::from_str_radix(&part[3..5], 16).unwrap(),
                    u8::from_str_radix(&part[5..7], 16).unwrap(),
                );
                assert!(r == g && g == bl, "Print draws a hue: {part} in {line}");
            }
        }
    }

    /// Writes every case (Colour, Print, student, key) to $VERSHION_PNG_DIR.
    #[test]
    fn enzyme_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let s2 = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]),
            ("large".into(), vec![s2("label_size", "Large")]),
            ("competitive".into(), vec![s2("mode", "competitive"), f("inh", 4.0)]),
            ("noncompetitive".into(), vec![s2("mode", "noncompetitive"), f("inh", 4.0)]),
            ("edge".into(), vec![f("at", 12.0), f("km", 20.0), f("smax", 12.0)]),
        ];
        for (idx, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{idx}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s2("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/enz-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/enz-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
