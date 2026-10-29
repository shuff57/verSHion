//! A motion map: one dot per equal time interval (a strobe), with the velocity at each dot drawn as an arrow. Equal
//! gaps mean constant velocity; gaps that grow mean speeding up; gaps that shrink mean slowing down. Drawn with
//! `svg::Doc` only.
//!
//! The velocities, the gaps and the acceleration are the answers, so they blank while Show answers is off; the dots,
//! the arrows' directions, the track and the given interval stay.
use super::circle::num;
use super::interact::Handle;
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "motion-map";
const INK: &str = "#111111";
const DOT: &str = "#1f5fbf";
const VEL: &str = "#c0392b";
const TRACK: &str = "#8a8f99";
const DOT_PRINT: &str = "#111111";
const VEL_PRINT: &str = "#333333";
const TRACK_PRINT: &str = "#999999";
const BLANK: &str = "_____";
const W: f64 = 620.0;
const TRACK_Y: f64 = 230.0;
const X0: f64 = 60.0;
const ROWS_Y0: f64 = 330.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Physics,
        title: "Motion map",
        blurb: "A stroboscopic motion map: one dot per equal time interval with its velocity arrow, for constant velocity, speeding up or slowing down.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("motion", "The motion is", &["constant velocity", "speeding up", "slowing down"], "constant velocity"),
        ParamDef::float("v0", "Velocity at the first flash (m/s)", 0.5, 10.0, 0.5, 2.0),
        ParamDef::float("a", "Acceleration (m/s\u{b2})", 0.5, 5.0, 0.5, 2.0),
        ParamDef::float("dt", "Time between flashes (s)", 0.1, 1.0, 0.1, 0.5),
        ParamDef::int("n", "Flashes", 2, 8, 5),
        ParamDef::int("dec", "Decimal places", 1, 2, 1),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(v: f64, lo: f64, hi: f64, d: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { d } }

/// (signed acceleration, first velocity, flash interval, number of flashes).
pub fn given(p: &Params) -> (f64, f64, f64, usize) {
    let mag = fin(p.f("a"), 0.5, 5.0, 2.0);
    let a = match p.s("motion") { "speeding up" => mag, "slowing down" => -mag, _ => 0.0 };
    (a, fin(p.f("v0"), 0.5, 10.0, 2.0), fin(p.f("dt"), 0.1, 1.0, 0.5), p.i("n").clamp(2, 8) as usize)
}

/// Where each flash happens (metres from the first) and the velocity at it. A slowing object stops rather than
/// running backwards, so the later flashes may sit on the stopping point.
pub fn flashes(p: &Params) -> (Vec<f64>, Vec<f64>) {
    let (a, v0, dt, n) = given(p);
    let stop = if a < 0.0 { v0 / -a } else { f64::INFINITY };
    let mut xs = Vec::new();
    let mut vs = Vec::new();
    for k in 0..n {
        let t = k as f64 * dt;
        let tc = t.min(stop);
        xs.push(v0 * tc + 0.5 * a * tc * tc);
        vs.push((v0 + a * tc).max(0.0));
    }
    (xs, vs)
}

/// The gaps between consecutive flashes, in metres. Computed from the velocities where that is exact — subtracting two
/// positions carried float noise that made equal gaps print as "0.1, 0.1, 0.1, 0.05, 0.05" — and from the positions
/// where the stop clamps them.
pub fn gaps(p: &Params) -> Vec<f64> {
    let (_, _, dt, _) = given(p);
    let (xs, vs) = flashes(p);
    xs.windows(2).zip(vs.windows(2)).map(|(x, v)| {
        let exact = (v[0] + v[1]) / 2.0 * dt;
        let drawn = x[1] - x[0];
        if (drawn - exact).abs() < 1e-9 { exact } else { drawn }
    }).collect()
}

/// A number that never rounds a real value to "0": two more decimals first, then exponential form. A gap of 0.05 m
/// printed as "0.0" (and then as 5.00e-2) read as a missing value beside its neighbours' "0.1".
fn q_num(v: f64, dec: i64) -> String {
    let zeros = |s: &str| s.trim_start_matches('\u{2212}').chars().filter(|c| c.is_ascii_digit()).all(|c| c == '0');
    let s = num(v, dec);
    if v != 0.0 && zeros(&s) {
        let more = num(v, dec + 2);
        if !zeros(&more) { return more; }
        return format!("{v:.2e}").replace('-', "\u{2212}");
    }
    s
}

pub fn warnings(p: &Params) -> Vec<String> {
    let (a, v0, dt, n) = given(p);
    let mut out = Vec::new();
    if a < 0.0 {
        let stop = v0 / -a;
        if (n - 1) as f64 * dt > stop + 1e-9 {
            out.push(format!("It stops after {:.2} s ({:.2} m), so the later flashes sit on the stopping point.", stop, v0 * stop / 2.0));
        }
    }
    out
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}" stroke="#fff" stroke-width="2.2" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        svg::esc(s)
    )
}

pub struct Report {
    /// Pixel position of the last dot (the drag handle).
    pub last: (f64, f64),
    pub scale: f64,
    pub track_y: f64,
}

pub fn build(p: &Params) -> (String, Report) {
    let (xs, vs) = flashes(p);
    let (a, v0, dt, n) = given(p);
    let dec = p.i("dec").clamp(1, 2);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let lab = svg::label_factor(p.s("label_size"));
    let (ink, dot, vel, track) = (
        if print { "#555555" } else { INK },
        if print { DOT_PRINT } else { DOT },
        if print { VEL_PRINT } else { VEL },
        if print { TRACK_PRINT } else { TRACK },
    );

    let rows_y0 = ROWS_Y0 + (lab - 1.0) * 30.0;
    let w = (W * (1.0 + 0.8 * (lab - 1.0))).max(W);
    let mut d = Doc::new(w, rows_y0 + 3.0 * ROW_H * lab + 10.0);
    d.describe("Motion map", "Dots at equal time intervals with a velocity arrow at each, for a moving object");

    // the drawing scale: every dot has to fit, however far the object travels
    let x_max = xs.last().copied().unwrap_or(1.0).max(0.001);
    let scale = ((w - X0 - 110.0) / x_max).clamp(1.0, 90.0);
    let px = |x: f64| X0 + x * scale;
    d.line(30.0, TRACK_Y, w - 30.0, TRACK_Y, track, 1.6);

    // the arrows are scaled so the fastest is a fixed length: they show the change, not the absolute speed
    let v_max = vs.iter().cloned().fold(0.0f64, f64::max).max(1e-9);
    let arrow = |v: f64| (v / v_max * 74.0).max(if v > 0.0 { 10.0 } else { 0.0 });
    let (mut high, mut low): (Option<f64>, Option<f64>) = (None, None);
    for (k, (x, v)) in xs.iter().zip(vs.iter()).enumerate() {
        let (cx, cy) = (px(*x), TRACK_Y);
        d.circle(cx, cy, 6.0, dot, ink, 1.4);
        if *v > 0.0 {
            d.arrow(cx, cy - 12.0, cx, cy - 12.0 - arrow(*v), vel, 2.0);
            if k == 0 || k + 1 == n {
                d.raw(&text_el(cx + 8.0, cy - 18.0, 11.0 * lab, "start", vel, "v"));
            }
        }
        // the flash number under each dot, on two rows, and only when it will not sit on the one before it: a
        // slowing object's last dots, or a slow strobe, can put several dots at almost the same place
        let row = if k % 2 == 1 { 18.0 * lab } else { 0.0 };
        let taken = if row > 0.0 { &mut low } else { &mut high };
        if taken.map_or(true, |last| (cx - last).abs() >= 20.0) {
            d.raw(&text_el(cx, cy + 22.0 + row, 11.0 * lab, "middle", ink, &format!("{}", k + 1)));
            *taken = Some(cx);
        }
    }

    // the givens: the strobe and the motion type. The starting velocity and the acceleration are what the student is
    // asked to read off the arrows, so they are printed only on the key.
    let kind = match p.s("motion") { "speeding up" => "speeding up", "slowing down" => "slowing down", _ => "steady" };
    let givens = if ans && a != 0.0 {
        format!("one flash every {} s,  {} flashes,  {},  v0 = {} m/s,  a = {} m/s\u{b2}", q_num(dt, 2), n, kind, q_num(v0, dec), q_num(a, dec))
    } else if ans {
        format!("one flash every {} s,  {} flashes,  {},  v = {} m/s throughout", q_num(dt, 2), n, kind, q_num(v0, dec))
    } else {
        format!("one flash every {} s,  {} flashes,  {}", q_num(dt, 2), n, kind)
    };
    d.raw(&text_el(24.0, rows_y0 - 18.0 * lab, 12.0 * lab, "start", ink, &givens));

    // the readout
    let gap_list = gaps(p).iter().map(|g| q_num(*g, dec)).collect::<Vec<_>>().join(", ");
    let rows = [
        if ans {
            format!("Velocity at each flash  {} m/s", vs.iter().map(|v| q_num(*v, dec)).collect::<Vec<_>>().join(", "))
        } else {
            format!("Velocity at each flash  {BLANK} m/s")
        },
        if ans { format!("Gap between flashes  {} m", gap_list) } else { format!("Gap between flashes  {BLANK} m") },
        if ans {
            if a == 0.0 { "Equal gaps: constant velocity".to_string() } else { format!("Acceleration  {} m/s\u{b2}", q_num(a, dec)) }
        } else if a == 0.0 { String::new() } else { format!("Acceleration  {BLANK} m/s\u{b2}") },
    ];
    for (i, line) in rows.iter().enumerate() {
        if line.is_empty() { continue; }
        d.raw(&format!(
            r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            rows_y0 + i as f64 * ROW_H * lab,
            ROW_FS * lab,
            ink,
            svg::esc(line)
        ));
    }

    (d.finish(), Report { last: (px(xs[xs.len() - 1]), TRACK_Y), scale, track_y: TRACK_Y })
}

/// The last flash is draggable sideways: it sets the first velocity, so the whole map stretches or compresses. When the
/// last flashes are clamped at the stopping point the drag cannot be inverted, so it is not offered at all.
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let (a, v0, dt, n) = given(p);
    if a < 0.0 && (n - 1) as f64 * dt > v0 / -a + 1e-9 { return Some(Vec::new()); }
    let rep = build(p).1;
    let scale = rep.scale;
    Some(vec![Handle::knob(rep.last, "Drag the last flash to set the first velocity", move |p, pt| {
        if !pt.0.is_finite() { return; }
        let x = (pt.0 - X0) / scale;
        let t = (n - 1) as f64 * dt;
        // solve x = v0 t + a t^2 / 2 for v0
        let v0 = (x - 0.5 * a * t * t) / t.max(1e-9);
        if v0.is_finite() { p.set_f("v0", (v0 * 4.0).round() / 4.0); }
    })])
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let i = |k: &'static str, v: i64| (k, Value::I(v));
    Some(vec![
        Example { name: "Constant velocity: the dots are evenly spaced", params: vec![f("v0", 2.0), f("dt", 0.5), i("n", 5)] },
        Example { name: "Speeding up from 0.5 m/s", params: vec![s("motion", "speeding up"), f("v0", 0.5), f("a", 2.0), f("dt", 0.5), i("n", 6)] },
        Example { name: "Speeding up from 1 m/s", params: vec![s("motion", "speeding up"), f("v0", 1.0), f("a", 1.5), f("dt", 0.4), i("n", 5)] },
        Example { name: "Slowing down until it stops", params: vec![s("motion", "slowing down"), f("v0", 4.0), f("a", 2.0), f("dt", 0.5), i("n", 6)] },
        Example { name: "The same motion sampled twice as often", params: vec![f("v0", 3.0), f("dt", 0.25), i("n", 8)] },
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
    fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
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
    fn constant_velocity_gives_equal_gaps() {
        let g = gaps(&with(&[]));
        for w in g.windows(2) { assert!((w[0] - w[1]).abs() < 1e-12, "{g:?}"); }
        // 2 m/s for 0.5 s is 1 m between flashes
        assert!((g[0] - 1.0).abs() < 1e-12, "{g:?}");
    }

    #[test]
    fn speeding_up_grows_the_gaps_by_a_dt_squared() {
        // the gap from flash k to k+1 is v0 dt + a dt^2 (k + 1/2), so consecutive gaps differ by a dt^2
        let p = with(&[s("motion", "speeding up"), f("v0", 1.0), f("a", 2.0), f("dt", 0.5)]);
        let g = gaps(&p);
        for w in g.windows(2) {
            assert!((w[1] - w[0] - 2.0 * 0.25).abs() < 1e-12, "{g:?}");
        }
        // and the velocities rise by a dt
        let (_, vs) = flashes(&p);
        for w in vs.windows(2) { assert!((w[1] - w[0] - 1.0).abs() < 1e-12, "{vs:?}"); }
    }

    #[test]
    fn slowing_down_stops_rather_than_reversing() {
        // 4 m/s with a = -2 stops at 2 s, 4 m; flashes past that sit on the stopping point with v = 0
        let p = with(&[s("motion", "slowing down"), f("v0", 4.0), f("a", 2.0), f("dt", 1.0), i("n", 5)]);
        let (xs, vs) = flashes(&p);
        assert!((xs[4] - 4.0).abs() < 1e-9, "{xs:?}");
        assert_eq!(vs[4], 0.0);
        assert!((xs[2] - 4.0).abs() < 1e-9, "the flash at the stop is at 4 m: {xs:?}");
        assert!(warnings(&p).iter().any(|w| w.contains("stops after")));
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Velocity at each flash  _____ m/s"), "{stu}");
        assert!(stu.contains("Gap between flashes  _____ m"), "{stu}");
        assert!(!stu.contains("1.0, 1.0"), "{stu}");
        // the givens stay
        assert!(stu.contains("one flash every 0.5 s"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Gap between flashes  1"), "{key}");
    }

    #[test]
    fn a_small_gap_never_prints_as_zero_or_in_scientific_style() {
        // 0.5 m/s sampled every 0.1 s is a 0.05 m gap: float noise in the position differences used to make some of
        // them round to "0.0" and then to "5.00e-2" beside their neighbours' "0.1"
        let t = texts(&render_of(&[f("v0", 0.5), f("dt", 0.1), i("n", 6), b(SHOW_ANSWERS, true)]));
        let line = t.lines().find(|x| x.starts_with("Gap between flashes")).unwrap_or("");
        let values = line.split("  ").nth(1).unwrap_or("");
        assert!(!values.contains('e'), "scientific style in the gaps: {values}");
        assert!(!values.contains("0.0,") && !values.contains("0.0 m"), "{values}");
        // and constant velocity prints the same gap every time
        let gaps: Vec<&str> = values.trim_end_matches(" m").split(", ").collect();
        assert!(gaps.windows(2).all(|w| w[0] == w[1]), "equal gaps printed unequally: {values}");
    }

    #[test]
    fn every_dot_fits_the_canvas_even_at_the_fastest_settings() {
        for list in [vec![s("motion", "speeding up"), f("v0", 10.0), f("a", 5.0), f("dt", 1.0), i("n", 8)],
                     vec![f("v0", 10.0), f("dt", 1.0), i("n", 8)]] {
            let p = with(&list);
            let rep = build(&p).1;
            let (xs, _) = flashes(&p);
            let last = X0 + xs.last().copied().unwrap_or(0.0) * rep.scale;
            assert!(last <= W - 20.0, "{list:?}: the last dot is at {last}");
        }
    }

    #[test]
    fn the_givens_do_not_give_the_answers_away() {
        // with answers off the starting velocity and the acceleration are what is being asked for
        let stu = texts(&render_of(&[s("motion", "speeding up"), b(SHOW_ANSWERS, false)]));
        assert!(!stu.contains("v0"), "{stu}");
        assert!(!stu.contains("a = "), "{stu}");
        assert!(stu.contains("one flash every"), "the strobe interval stays: {stu}");
        let key = texts(&render_of(&[s("motion", "speeding up"), b(SHOW_ANSWERS, true)]));
        assert!(key.contains("v0 = 2 m/s"), "{key}");
        assert!(key.contains("a = 2 m/s"), "{key}");
    }

    #[test]
    fn the_handle_is_offered_only_where_it_can_be_inverted() {
        // a slowing object whose later flashes sit at the stop cannot be dragged: the model clamps there
        let clamped = with(&[s("motion", "slowing down"), f("v0", 5.0), f("a", 2.0), f("dt", 1.0), i("n", 8)]);
        assert_eq!(handles(ID, &clamped, (W, 430.0)).map(|h| h.len()), Some(0), "no drag at the stop");
        let fine = with(&[]);
        assert_eq!(handles(ID, &fine, (W, 430.0)).map(|h| h.len()), Some(1), "the default is draggable");
    }

    #[test]
    fn print_is_neutral_grey_only() {
        let svg = render_of(&[s("style", "Print"), b(SHOW_ANSWERS, true)]);
        for attr in ["stroke=\"#", "fill=\"#"] {
            for line in svg.lines().filter(|l| l.contains(attr)) {
                for part in line.split(attr).skip(1) {
                    let hex = &part[..6.min(part.len())];
                    if hex.len() < 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) { continue; }
                    let (r, g, b) = (u8::from_str_radix(&hex[0..2], 16).unwrap(), u8::from_str_radix(&hex[2..4], 16).unwrap(), u8::from_str_radix(&hex[4..6], 16).unwrap());
                    assert!(r == g && g == b, "Print draws a hue: {hex} in {line}");
                }
            }
        }
    }

    /// Writes every example (Colour, Print, student, key, Large) to $VERSHION_PNG_DIR.
    #[test]
    fn motion_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]),
            ("large".into(), vec![s("label_size", "Large")]),
            // the extremes: the fastest map there is, and one that stops with later flashes on the stop
            ("fast".into(), vec![s("motion", "speeding up"), f("v0", 10.0), f("a", 5.0), f("dt", 1.0), i("n", 8)]),
            ("stop".into(), vec![s("motion", "slowing down"), f("v0", 5.0), f("a", 2.0), f("dt", 1.0), i("n", 8)]),
        ];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/motion-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/motion-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
