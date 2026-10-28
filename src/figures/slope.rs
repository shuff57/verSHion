//! The slope of a straight line, shown as a right-angled triangle: a run of `run` units across, a rise of `run * m`
//! up, and the gradient read off as rise over run. The line is y = mx + c.
//!
//! The gradient, the rise and the equation are the answer, so they are printed only while Show answers is on.
//! Drawn with `svg::Doc` only, on a light grid with ink axes.
use super::circle::{num, tw, Rect};
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "slope-triangle";
const INK: &str = "#111111";
const LINE: &str = "#1f5fbf";
const LINE_PRINT: &str = "#111111";
const TRI: &str = "#a0704c";
const TRI_PRINT: &str = "#777777";
const GRID_COLOUR: &str = "#e6ebf1";
const GRID_PRINT: &str = "#dedede";
const BLANK: &str = "_____";
const W: f64 = 460.0;
const PLOT: f64 = 330.0;
const L: f64 = 50.0;
const T: f64 = 16.0;
const ROWS_Y0: f64 = T + PLOT + 26.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;
/// The window the line is drawn in.
const X0: f64 = -6.0;
const X1: f64 = 6.0;
const Y0: f64 = -6.0;
const Y1: f64 = 6.0;
/// Where the triangle's left corner sits.
const ANCHOR: f64 = -2.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Slope triangle",
        blurb: "A straight line with a right-angled triangle showing its rise over its run, and the gradient and equation worked out from it.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("m", "Gradient m", -5.0, 5.0, 0.25, 2.0),
        ParamDef::float("c", "y-intercept c", -5.0, 5.0, 0.5, 1.0),
        ParamDef::int("run", "Triangle run", 1, 4, 1),
        ParamDef::int("dec", "Decimal places", 0, 2, 0),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

/// The line's numbers: gradient, intercept, the triangle's run and its rise.
pub fn values(p: &Params) -> (f64, f64, i64, f64) {
    let clamp = |v: f64, lo: f64, hi: f64, d: f64| if v.is_finite() { v.clamp(lo, hi) } else { d };
    let m = clamp(p.f("m"), -5.0, 5.0, 1.0);
    let c = clamp(p.f("c"), -5.0, 5.0, 0.0);
    let run = p.i("run").clamp(1, 4);
    (m, c, run, m * run as f64)
}

pub fn warnings(_p: &Params) -> Vec<String> { Vec::new() }

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}" stroke="#fff" stroke-width="2" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        svg::esc(s)
    )
}

pub struct Report {
    pub m: f64,
    pub c: f64,
    pub run: i64,
    pub rise: f64,
}

pub fn build(p: &Params) -> (String, Report) {
    let (m, c, run, rise) = values(p);
    let dec = p.i("dec").clamp(0, 2);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let f = svg::label_factor(p.s("label_size"));
    let (ink, line, tri, grid) = (if print { "#555555" } else { INK }, if print { LINE_PRINT } else { LINE }, if print { TRI_PRINT } else { TRI }, if print { GRID_PRINT } else { GRID_COLOUR });

    let u = PLOT / (X1 - X0);
    let ox = L;
    let oy = T;
    let px = |x: f64| ox + (x - X0) * u;
    let py = |y: f64| oy + (Y1 - y) * u;

    let mut d = Doc::new(W, ROWS_Y0 + 3.0 * ROW_H * f + 8.0);
    d.describe("Slope triangle", "A straight line with a right-angled triangle showing its rise over its run");

    // grid, then the axes
    let mut k = X0.ceil();
    while k <= X1 + 1e-9 { d.line(px(k), oy, px(k), oy + PLOT, grid, 1.0); k += 1.0; }
    let mut k = Y0.ceil();
    while k <= Y1 + 1e-9 { d.line(ox, py(k), ox + PLOT, py(k), grid, 1.0); k += 1.0; }
    d.line(ox, py(0.0), ox + PLOT, py(0.0), ink, 1.6);
    d.line(px(0.0), oy, px(0.0), oy + PLOT, ink, 1.6);

    // the line, from edge to edge of the window
    let (ya, yb) = (m * X0 + c, m * X1 + c);
    d.line(px(X0), py(ya), px(X1), py(yb), line, if print { 2.6 } else { 3.0 });

    // the slope triangle: across `run`, up `rise`
    let (ax, ay) = (ANCHOR, m * ANCHOR + c);
    let (bx, by) = (ax + run as f64, ay + rise);
    let c1 = (px(ax), py(ay));
    let c2 = (px(bx), py(ay));
    let c3 = (px(bx), py(by));
    d.polyline(&[c1, c2, c3], tri, 2.0);
    // the right angle at the corner
    d.right_angle(c2, ((c1.0 - c2.0).signum(), 0.0), (0.0, (c3.1 - c2.1).signum()), 11.0, tri, 1.4);

    // the run and the rise written along their legs
    let size = 13.0 * f;
    let run_text = if ans { format!("run {}", num(run as f64, 0)) } else { format!("run {BLANK}") };
    let rise_text = if ans { format!("rise {}", num(rise, dec)) } else { format!("rise {BLANK}") };
    let mid_x = (c1.0 + c2.0) / 2.0;
    let mid_y = (c2.1 + c3.1) / 2.0;
    // both labels sit outside the triangle: the run below the flat leg (above it when the gradient is negative),
    // and the rise to the right of the upright leg
    let run_y = if m >= 0.0 { c2.1 + 16.0 } else { c2.1 - 8.0 };
    d.raw(&text_el(mid_x, run_y, size, "middle", tri, &run_text));
    d.raw(&text_el(c3.0 + 8.0, mid_y + 4.0, size, "start", tri, &rise_text));

    // the axis numbers, except where a number would sit on one of the triangle's labels
    let (rw, rw2) = (tw(&run_text, size, false), tw(&rise_text, size, false));
    let boxes = [
        Rect { x0: mid_x - rw / 2.0, y0: run_y - size, x1: mid_x + rw / 2.0, y1: run_y + size * 0.3 },
        Rect { x0: c3.0 + 8.0, y0: mid_y + 4.0 - size, x1: c3.0 + 8.0 + rw2, y1: mid_y + 4.0 + size * 0.3 },
    ];
    let clear = |x: f64, y: f64, w: f64| {
        let r = Rect { x0: x - w / 2.0, y0: y - 12.0, x1: x + w / 2.0, y1: y + 4.0 };
        !boxes.iter().any(|b| r.x0 < b.x1 && b.x0 < r.x1 && r.y0 < b.y1 && b.y0 < r.y1)
    };
    for (k, x) in [(-5, px(-5.0)), (-4, px(-4.0)), (-2, px(-2.0)), (2, px(2.0)), (4, px(4.0)), (5, px(5.0))] {
        if clear(x, py(0.0) + 15.0, tw(&k.to_string(), 12.0, false)) {
            d.raw(&text_el(x, py(0.0) + 15.0, 12.0, "middle", ink, &k.to_string()));
        }
    }
    for (k, y) in [(-5, py(-5.0)), (-4, py(-4.0)), (-2, py(-2.0)), (2, py(2.0)), (4, py(4.0)), (5, py(5.0))] {
        if clear(px(0.0) - 6.0, y + 4.0, tw(&k.to_string(), 12.0, false)) {
            d.raw(&text_el(px(0.0) - 6.0, y + 4.0, 12.0, "end", ink, &k.to_string()));
        }
    }

    // the readout
    let mut rows = vec![];
    rows.push(if ans { format!("Gradient  m = {}", num(m, dec)) } else { format!("Gradient  m = {BLANK}") });
    rows.push(if ans { format!("Rise {B} for a run of {R}", B = num(rise, dec), R = num(run as f64, 0)) } else { format!("Rise {BLANK} for a run of {BLANK}") });
    let eq = if c.abs() < 1e-9 { format!("y = {}x", num(m, dec)) } else if c > 0.0 { format!("y = {}x + {}", num(m, dec), num(c, dec)) } else { format!("y = {}x \u{2212} {}", num(m, dec), num(-c, dec)) };
    rows.push(if ans { format!("Equation  {eq}") } else { format!("Equation  y = {BLANK}") });
    for (i, line) in rows.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            ROWS_Y0 + i as f64 * ROW_H * f,
            ROW_FS * f,
            ink,
            svg::esc(line)
        ));
    }

    (d.finish(), Report { m, c, run, rise })
}

/// A point on the line (kept for tests that want to check the drawing against the equation).
pub fn at(p: &Params, x: f64) -> f64 { let (m, c, _, _) = values(p); m * x + c }

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let i = |k: &'static str, v: i64| (k, Value::I(v));
    Some(vec![
        Example { name: "Gradient 2: up 2 for every 1 across", params: vec![f("m", 2.0), f("c", 1.0), i("run", 1)] },
        Example { name: "Gradient 1/2: up 1 for every 2 across", params: vec![f("m", 0.5), f("c", -1.0), i("run", 2)] },
        Example { name: "A negative gradient: down as it goes across", params: vec![f("m", -1.5), f("c", 2.0), i("run", 2)] },
        Example { name: "Through the origin", params: vec![f("m", 3.0), f("c", 0.0), i("run", 1)] },
        Example { name: "A gentle gradient", params: vec![f("m", 0.25), f("c", 3.0), i("run", 4)] },
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
    fn the_defaults_are_a_gradient_of_two() {
        let rep = build(&with(&[])).1;
        assert_eq!((rep.m, rep.c, rep.run, rep.rise), (2.0, 1.0, 1, 2.0));
        let t = texts(&render_of(&[]));
        assert!(t.contains("Gradient  m = 2"), "{t}");
        assert!(t.contains("Rise 2 for a run of 1"), "{t}");
        assert!(t.contains("Equation  y = 2x + 1"), "{t}");
    }

    #[test]
    fn the_rise_is_the_gradient_times_the_run() {
        for (m, run) in [(2.0, 1), (0.5, 2), (-1.5, 2), (3.0, 4), (-0.25, 3)] {
            let (_, _, r, rise) = values(&with(&[f("m", m), i("run", run)]));
            assert_eq!(r, run);
            assert!((rise - m * run as f64).abs() < 1e-9, "{m} {run}: {rise}");
            // and the triangle's two ends lie on the line
            let p = with(&[f("m", m), i("run", run)]);
            assert!((at(&p, ANCHOR) - (m * ANCHOR + p.f("c"))).abs() < 1e-9);
        }
    }

    #[test]
    fn the_equation_is_written_with_the_right_signs() {
        assert!(texts(&render_of(&[f("m", 2.0), f("c", 0.0)])).contains("Equation  y = 2x"));
        assert!(texts(&render_of(&[f("m", 2.0), f("c", 3.0)])).contains("Equation  y = 2x + 3"));
        assert!(texts(&render_of(&[f("m", -1.0), f("c", -4.0)])).contains("Equation  y = \u{2212}1x \u{2212} 4"));
    }

    #[test]
    fn the_answer_is_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Gradient  m = _____"), "{stu}");
        assert!(stu.contains("Rise _____ for a run of _____"), "{stu}");
        assert!(stu.contains("Equation  y = _____"), "{stu}");
        // the legs of the triangle are blanked too
        assert!(stu.contains("run _____") && stu.contains("rise _____"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("run 1") && key.contains("rise 2"), "{key}");
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
    }

    /// Writes every example (Colour, Print, student, key, Print at a 66% bitonal threshold) to $VERSHION_PNG_DIR.
    #[test]
    fn slope_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/slope-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/slope-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
