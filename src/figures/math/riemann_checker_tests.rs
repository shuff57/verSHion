//! The first independent checker's own measure of the labels and its 100 classroom cases (ported from its round-1
//! checker, riemann_check.rs, unchanged but for the file writing): every label on no stroke, dot, text or other label,
//! each strip label nearer its own strip than any other, each a or b nearer its own line, each xᵢ nearest its own tick,
//! and nothing left off without a note.
use super::*;
use crate::figures::{registry, FigureDef};


fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
fn from_json(j: &str) -> Params {
    let d = def();
    let v: serde_json::Value = serde_json::from_str(j).unwrap_or_else(|e| panic!("{j}: {e}"));
    let mut p = crate::figures::start_params(&d);
    for (k, x) in v.as_object().unwrap() {
        let isint = (d.params)().into_iter().any(|q| q.key == k.as_str() && matches!(q.kind, crate::figures::Kind::Int { .. }));
        let val = match x {
            serde_json::Value::Bool(b) => Value::B(*b),
            serde_json::Value::String(s) => Value::S(s.clone()),
            serde_json::Value::Number(n) => if isint { Value::I(n.as_f64().unwrap() as i64) } else { Value::F(n.as_f64().unwrap()) },
            _ => continue,
        };
        p.0.insert(k.clone(), val);
    }
    p
}
fn plain(inner: &str) -> String {
    let mut t = String::new();
    let mut tag = false;
    for c in inner.chars() { match c { '<' => tag = true, '>' => tag = false, _ if !tag => t.push(c), _ => {} } }
    t.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
}
/// (kind, text, box) of every figure text and blank.
fn labels(svg: &str) -> Vec<(String, String, [f64; 4])> {
    let mut out = vec![];
    let mut rest = svg;
    while let Some(k) = rest.find("data-rs=\"") {
        rest = &rest[k..];
        let kind: String = rest[9..].split('"').next().unwrap().to_string();
        let st = rest.rfind('<').unwrap_or(0);
        let _ = st;
        let e = rest.find('>').unwrap();
        let tag = &rest[..e];
        if let Some(bx) = tag.find("data-box=\"") {
            let nums: Vec<f64> = tag[bx + 10..].split('"').next().unwrap().split(' ').filter_map(|t| t.parse().ok()).collect();
            let txt = if kind == "blank" { "_".to_string() } else { let close = rest.find("</text>").unwrap_or(e); plain(&rest[e + 1..close]) };
            out.push((kind, txt, [nums[0], nums[1], nums[2], nums[3]]));
        }
        rest = &rest[e..];
    }
    out
}
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> { let pat = format!(" {name}=\""); let i = tag.find(&pat)? + pat.len(); tag[i..].split('"').next() }
fn tags<'a>(svg: &'a str, open: &str) -> Vec<&'a str> {
    let mut out = vec![];
    let mut rest = svg;
    while let Some(k) = rest.find(open) { rest = &rest[k..]; let e = rest.find('>').unwrap(); out.push(&rest[..e]); rest = &rest[e..]; }
    out
}
fn nums(s: &str) -> Vec<f64> { s.split([' ', ',']).filter_map(|t| t.parse().ok()).collect() }
type Seg = ((f64, f64), (f64, f64));
fn seg_hits(b: &[f64; 4], s: &Seg) -> bool {
    // shrink box by 0.5 px so touching is not a hit
    let b = [b[0] + 0.5, b[1] + 0.5, b[2] - 0.5, b[3] - 0.5];
    let ((x0, y0), (x1, y1)) = *s;
    let n = 40;
    for i in 0..=n { let t = i as f64 / n as f64; let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t); if x > b[0] && x < b[2] && y > b[1] && y < b[3] { return true; } }
    false
}
fn hit(a: &[f64; 4], b: &[f64; 4]) -> bool { a[0] < b[2] - 0.5 && b[0] < a[2] - 0.5 && a[1] < b[3] - 0.5 && b[1] < a[3] - 0.5 }
/// grid texts (numbers, axis letters, title): my own box estimate, 0.55 em a char, generous.
fn grid_texts(svg: &str) -> Vec<(String, [f64; 4])> {
    let mut out = vec![];
    let mut rest = svg;
    while let Some(i) = rest.find("<text ") {
        rest = &rest[i..];
        let e = rest.find('>').unwrap();
        let tag = &rest[..e];
        let close = rest.find("</text>").unwrap();
        let inner = plain(&rest[e + 1..close]);
        rest = &rest[close..];
        if tag.contains("data-rs=") || inner.trim().is_empty() { continue; }
        let (Some(x), Some(y), Some(fs)) = (attr(tag, "x").and_then(|v| v.parse::<f64>().ok()), attr(tag, "y").and_then(|v| v.parse::<f64>().ok()), attr(tag, "font-size").and_then(|v| v.parse::<f64>().ok())) else { continue };
        let w = inner.chars().count() as f64 * 0.5 * fs;
        let x0 = match attr(tag, "text-anchor") { Some("middle") => x - w / 2.0, Some("end") => x - w, _ => x };
        out.push((inner, [x0, y - 0.7 * fs, x0 + w, y + 0.2 * fs]));
    }
    out
}

pub struct Report { pub faults: Vec<String>, pub drawn: usize, pub expected: usize, pub missing_noted: usize, pub missing_silent: usize }

/// My own measurement of one render.
fn measure(p: &Params) -> Report {
    let (svg, notes) = build(p);
    let (w, h) = crate::figures::svg_size(&svg);
    let labs = labels(&svg);
    let mut faults = vec![];
    let mut segs: Vec<(String, Seg)> = vec![];
    for t in tags(&svg, "<polygon data-rs=\"strip\"") {
        let v = nums(attr(t, "points").unwrap());
        let pts: Vec<(f64, f64)> = v.chunks(2).map(|c| (c[0], c[1])).collect();
        for i in 0..pts.len() { segs.push(("strip-edge".into(), (pts[i], pts[(i + 1) % pts.len()]))); }
    }
    for k in ["bound", "tick"] {
        for t in tags(&svg, &format!("<line data-rs=\"{k}\"")) {
            let q: Vec<f64> = ["x1", "y1", "x2", "y2"].iter().map(|a| attr(t, a).unwrap().parse().unwrap()).collect();
            segs.push((k.into(), ((q[0], q[1]), (q[2], q[3]))));
        }
    }
    let g = grid_layout(&grid_params(p));
    let sx = |x: f64| g.ml + (x - g.x0) * g.ux;
    let sy = |y: f64| g.mt + g.ph - (y - g.y0) * g.uy;
    let axy = sy(if g.y0 <= 0.0 && g.y1 >= 0.0 { 0.0 } else { g.y0 });
    let axx = sx(if g.x0 <= 0.0 && g.x1 >= 0.0 { 0.0 } else { g.x0 });
    segs.push(("x-axis".into(), ((g.ml - 20.0, axy), (g.ml + g.pw + 20.0, axy))));
    segs.push(("y-axis".into(), ((axx, g.mt - 20.0), (axx, g.mt + g.ph + 20.0))));
    let table = p.s("region") == "From a table of values";
    if !table && p.b("curve") {
        let mut fs = vec![p.s("f").to_string()];
        if p.s("region") == "Between f(x) and g(x)" { fs.push(p.s("g").to_string()); }
        for f in fs {
            let Ok(src) = read_fn(&f, "f") else { continue };
            let mut prev: Option<(f64, f64)> = None;
            for i in 0..=1500 {
                let x = g.x0 + (g.x1 - g.x0) * i as f64 / 1500.0;
                let q = ev(&src, x).filter(|y| *y >= g.y0 - 0.5 && *y <= g.y1 + 0.5).map(|y| (sx(x), sy(y).clamp(g.mt, g.mt + g.ph)));
                if let (Some(a), Some(b)) = (prev, q) && (a.1 - b.1).abs() < g.ph { segs.push(("curve".into(), (a, b))); }
                prev = q;
            }
        }
    }
    let dots: Vec<[f64; 4]> = tags(&svg, "<circle data-rs=\"dot\"").iter().map(|t| { let (x, y): (f64, f64) = (attr(t, "cx").unwrap().parse().unwrap(), attr(t, "cy").unwrap().parse().unwrap()); [x - 3.5, y - 3.5, x + 3.5, y + 3.5] }).collect();
    let gt = grid_texts(&svg);
    for (i, (k, t, b)) in labs.iter().enumerate() {
        if b[0] < 0.0 || b[1] < 0.0 || b[2] > w || b[3] > h { faults.push(format!("off-page {k} {t:?}")); }
        if k == "line" { continue; }
        for (sk, s) in &segs {
            // strip labels and blanks may sit on their own strip's tint but not on an edge; x-marks may sit on their own tick? no
            if seg_hits(b, s) { faults.push(format!("{k} {t:?} on {sk}")); break; }
        }
        for d in &dots { if hit(b, d) { faults.push(format!("{k} {t:?} on a dot")); break; } }
        for (gtxt, gb) in &gt { if hit(b, gb) { faults.push(format!("{k} {t:?} on grid text {gtxt:?}")); break; } }
        for (j, (k2, t2, b2)) in labs.iter().enumerate() { if j > i && k2 != "line" && hit(b, b2) { faults.push(format!("{k} {t:?} on {k2} {t2:?}")); } }
    }
    // ownership: strip labels within their strip's x span; bound labels nearer their own line; x-marks nearest their tick
    let m = model(p).ok();
    let mut expected = 0;
    let mut drawn = 0;
    if let Some(m) = &m {
        let student = !p.b("show_answers");
        let no_strips = student && p.s("student") == "Curve and bounds only";
        let kd = kind(p);
        let sl: Vec<&(String, String, [f64; 4])> = labs.iter().filter(|l| l.0 == "strip-label" || (l.0 == "blank" && l.2[1] < g.mt + g.ph + 40.0 && l.2[3] - l.2[1] < 7.0 && l.2[0] > 15.0 && (l.2[2] - l.2[0]) < 60.0)).collect();
        let want_sl = kd == Kind::V && !no_strips && (p.s("strip_labels") != "None" || (student && p.s("student") == "Strip heights blank"));
        if want_sl {
            expected += m.strips.len(); drawn += sl.len();
            for (idx, l) in sl.iter().enumerate() {
                if let Some(s) = m.strips.get(idx) {
                    let cx = 0.5 * (l.2[0] + l.2[2]);
                    let (a, b) = (sx(s.x0), sx(s.x1));
                    // nearest strip centre must be its own
                    let near = m.strips.iter().enumerate().min_by(|x, y| ((0.5 * (sx(x.1.x0) + sx(x.1.x1)) - cx).abs()).total_cmp(&(0.5 * (sx(y.1.x0) + sx(y.1.x1)) - cx).abs())).map(|q| q.0).unwrap();
                    if near != idx || cx < a - 1.0 || cx > b + 1.0 { faults.push(format!("strip label {:?} (#{idx}) nearer strip #{near} than its own", l.1)); }
                }
            }
        }
        if p.b("mark_bounds") {
            let bl: Vec<&(String, String, [f64; 4])> = labs.iter().filter(|l| l.0 == "bound").collect();
            let inwin = [m.xa, m.xb].iter().filter(|x| sx(**x) >= g.ml - 0.5 && sx(**x) <= g.ml + g.pw + 0.5).count();
            expected += inwin; drawn += bl.len();
            for l in &bl {
                let own = if l.1.starts_with('a') { m.xa } else { m.xb };
                let other = if l.1.starts_with('a') { m.xb } else { m.xa };
                let d = |x: f64| { let px = sx(x); (l.2[0] - px).max(px - l.2[2]).max(0.0) };
                if d(own) > d(other) + 0.01 || d(own) > 40.0 { faults.push(format!("bound label {:?} nearer the other line or far ({:.1} px)", l.1, d(own))); }
            }
        }
        if p.b("x_marks") && kd == Kind::V && !no_strips {
            let xl: Vec<&(String, String, [f64; 4])> = labs.iter().filter(|l| l.0 == "xmark").collect();
            let n = m.nodes.len();
            let inwin = m.nodes.iter().filter(|x| sx(**x) >= g.ml - 0.5 && sx(**x) <= g.ml + g.pw + 0.5).count();
            let want = if inwin == n { n } else { inwin };
            expected += want.min(if xl.iter().any(|l| l.1.contains('\u{2026}')) { 4 } else { want });
            drawn += xl.len();
            for l in &xl {
                if let Some(i) = l.1.strip_prefix('x').and_then(|r| r.parse::<usize>().ok()) {
                    let cx = 0.5 * (l.2[0] + l.2[2]);
                    let near = m.nodes.iter().enumerate().min_by(|a, b| (sx(*a.1) - cx).abs().total_cmp(&(sx(*b.1) - cx).abs())).unwrap().0;
                    if near != i { faults.push(format!("xmark {:?} nearer x_{near}", l.1)); }
                }
            }
        }
        if p.b("sample_pts") && kd == Kind::V && !no_strips && !table {
            let ndots = tags(&svg, "<circle data-rs=\"dot\"").len();
            let inwin = m.samples.iter().filter(|q| { let (a, b) = (sx(q.0), sy(q.1)); a >= g.ml - 1.0 && a <= g.ml + g.pw + 1.0 && b >= g.mt - 1.0 && b <= g.mt + g.ph + 1.0 }).count();
            expected += inwin; drawn += ndots;
        }
    }
    let missing = expected.saturating_sub(drawn);
    let (mut noted, mut silent) = (0, 0);
    if missing > 0 { if notes.iter().any(|n| n.contains("no room") || n.contains("do not fit")) { noted = missing; } else { silent = missing; } }
    Report { faults, drawn, expected, missing_noted: noted, missing_silent: silent }
}

fn nice(span: f64) -> f64 { let raw = span / 8.0; let p = 10f64.powf(raw.log10().floor()); for k in [1.0, 2.0, 2.5, 5.0, 10.0] { if raw <= k * p { return k * p; } } 10.0 * p }

/// (name, overrides) of ordinary classroom cases, each with a window a teacher would choose.
fn classroom() -> Vec<(String, String)> {
    let mut out = vec![];
    let fns: Vec<(&str, &str, &str)> = vec![
        ("x^2", "0", "2"), ("x^2 + 1", "0", "2"), ("x^2", "1", "3"), ("x^3", "0", "2"), ("4 - x^2", "0", "2"), ("4 - x^2", "-2", "2"),
        ("2x + 1", "0", "3"), ("x", "0", "4"), ("sqrt(x)", "0", "4"), ("sqrt(x)", "1", "9"), ("1/x", "1", "4"), ("1/x", "1", "2"),
        ("e^x", "0", "1"), ("e^x", "0", "2"), ("e^(-x)", "0", "3"), ("ln(x)", "1", "4"), ("sin(x)", "0", "pi"), ("cos(x)", "0", "pi/2"),
        ("sin(x)", "0", "2pi"), ("x^2 - 2x", "0", "3"), ("3x - x^2", "0", "3"), ("x^3 - 3x", "-2", "2"), ("1/(1 + x^2)", "0", "1"),
        ("x^2 - 4", "-1", "3"), ("6 - 2x", "0", "3"), ("0.5x^2", "0", "4"), ("x e^(-x)", "0", "3"), ("sin(x)", "1", "3"),
        ("10 - x^2", "0", "3"), ("(x - 2)^2 + 1", "0", "4"), ("2^x", "0", "3"), ("x^2 + 2x + 1", "-1", "1"), ("16 - x^2", "0", "4"),
        ("x/2 + 1", "0", "6"), ("sqrt(9 - x^2)", "0", "3"), ("x^3 - x", "-1", "1"), ("cos(x)", "0", "2pi"), ("e^(x/2)", "0", "2"),
        ("1/x^2", "1", "3"), ("5", "0", "4"), ("x^4", "0", "1"), ("-x^2 + 4x", "0", "4"), ("ln(x)", "1/2", "3"),
    ];
    for (f, a, b) in &fns {
        let av = eval_typed(a).unwrap(); let bv = eval_typed(b).unwrap();
        let (mut lo, mut hi) = (0.0f64, 0.0f64);
        for i in 0..=400 { let x = av + (bv - av) * i as f64 / 400.0; if let Some(y) = ev(f, x) { lo = lo.min(y); hi = hi.max(y); } }
        let ys = nice((hi - lo).max(1.0) * 1.15);
        let ymin = ((lo / ys).floor() - 1.0) * ys; let ymax = ((hi / ys).ceil() + 1.0) * ys;
        let pi = b.contains("pi");
        let (xmin, xmax, xstep, xu) = if pi { ((av / PI - 0.5).min(-0.5), (bv / PI + 0.5), 0.5, "Multiples of \u{3c0}") } else { let s = nice((bv - av + 2.0).max(2.0) * 1.1); ((((av - 1.0) / s).floor() * s).min(-s), ((bv + 1.0) / s).ceil() * s, s, "Numbers") };
        for n in [4, 6] {
            out.push((format!("{f} [{a},{b}] n={n}"), format!(r#"{{"f":"{f}","a":"{a}","b":"{b}","n":{n},"xmin":{xmin:.4},"xmax":{xmax:.4},"xstep":{xstep:.4},"ymin":{ymin:.4},"ymax":{ymax:.4},"ystep":{ys:.4},"xunits":"{xu}"}}"#)));
        }
    }
    out.push(("table AP".into(), r#"{"region":"From a table of values","xmin":-1.0,"xmax":8.0,"ymin":-1.0,"ymax":7.0}"#.into()));
    out.push(("table 2".into(), r#"{"region":"From a table of values","table":"0, 3; 2, 5; 5, 9; 7, 10; 10, 8","xmin":-1.0,"xmax":11.0,"ymin":-1.0,"ymax":11.0}"#.into()));
    out.push(("table 3".into(), r#"{"region":"From a table of values","table":"0, 20; 10, 26; 20, 30; 30, 28; 40, 25; 50, 22; 60, 21","xmin":-10.0,"xmax":70.0,"xstep":10.0,"ymin":-5.0,"ymax":35.0,"ystep":5.0}"#.into()));
    out.push(("between x,x^2".into(), r#"{"region":"Between f(x) and g(x)","f":"x","g":"x^2","bounds":"Where f and g cross","xmin":-0.5,"xmax":1.5,"xstep":0.5,"ymin":-0.5,"ymax":1.5,"ystep":0.5}"#.into()));
    out.push(("between x+2,x^2".into(), r#"{"region":"Between f(x) and g(x)","f":"x + 2","g":"x^2","bounds":"Where f and g cross","xmin":-2.0,"xmax":3.0,"ymin":-1.0,"ymax":5.0}"#.into()));
    out.push(("between 4-x^2, x^2-4".into(), r#"{"region":"Between f(x) and g(x)","f":"4 - x^2","g":"x^2 - 4","bounds":"Where f and g cross","xmin":-3.0,"xmax":3.0,"ymin":-5.0,"ymax":5.0}"#.into()));
    out.push(("between sin cos".into(), r#"{"region":"Between f(x) and g(x)","f":"sin(x)","g":"cos(x)","a":"0","b":"pi/2","xunits":"Multiples of π","xmin":-0.5,"xmax":1.0,"xstep":0.25,"ymin":-0.5,"ymax":1.5,"ystep":0.5}"#.into()));
    out.push(("between e^x, x a..b".into(), r#"{"region":"Between f(x) and g(x)","f":"e^x","g":"x","a":"0","b":"1","xmin":-1.0,"xmax":2.0,"ymin":-1.0,"ymax":3.0}"#.into()));
    out.push(("horiz sqrt x/2".into(), r#"{"region":"Between f(x) and g(x)","f":"sqrt(x)","g":"x/2","bounds":"Where f and g cross","strips":"Horizontal","xmin":-1.0,"xmax":5.0,"ymin":-1.0,"ymax":3.0}"#.into()));
    out.push(("horiz x, x^2".into(), r#"{"region":"Between f(x) and g(x)","f":"x","g":"x^2","bounds":"Where f and g cross","strips":"Horizontal","rule":"Midpoint","xmin":-0.5,"xmax":1.5,"xstep":0.5,"ymin":-0.5,"ymax":1.5,"ystep":0.5}"#.into()));
    out.push(("signed x^2-1".into(), r#"{"f":"x^2 - 1","signed":true,"ymin":-2.0,"ymax":4.0}"#.into()));
    out.push(("signed sin 2pi strips".into(), r#"{"f":"sin(x)","a":"0","b":"2pi","signed":true,"n":8,"rule":"Midpoint","xunits":"Multiples of π","xmin":-0.5,"xmax":2.5,"xstep":0.5,"ymin":-1.5,"ymax":1.5,"ystep":0.5}"#.into()));
    out.push(("signed x^3-x".into(), r#"{"f":"x^3 - 4x","a":"-2","b":"3","signed":true,"n":5,"rule":"Right","xmin":-3.0,"xmax":4.0,"ymin":-4.0,"ymax":16.0,"ystep":2.0}"#.into()));
    out.push(("signed 2-x".into(), r#"{"f":"2 - x","a":"0","b":"4","signed":true,"rule":"Trapezoid","xmin":-1.0,"xmax":5.0,"ymin":-3.0,"ymax":3.0}"#.into()));
    out
}

fn optsets() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        ("plain", vec![]),
        ("sum", vec![("sum_line", "true")]),
        ("exact", vec![("exact_line", "true")]),
        ("integral", vec![("integral_line", "true")]),
        ("error", vec![("error_line", "true")]),
        ("dx", vec![("show_dx", "true")]),
        ("heights", vec![("strip_labels", "\"Heights\"")]),
        ("areas", vec![("strip_labels", "\"Areas\"")]),
        ("xmarks", vec![("x_marks", "true")]),
        ("samples", vec![("sample_pts", "true")]),
        ("key", vec![("key_line", "true")]),
        ("nobounds", vec![("mark_bounds", "false")]),
        ("all", vec![("sum_line", "true"), ("exact_line", "true"), ("integral_line", "true"), ("error_line", "true"), ("show_dx", "true"), ("strip_labels", "\"Heights\""), ("x_marks", "true"), ("sample_pts", "true"), ("key_line", "true")]),
        ("all-areas-dec", vec![("sum_line", "true"), ("exact_line", "true"), ("integral_line", "true"), ("error_line", "true"), ("show_dx", "true"), ("strip_labels", "\"Areas\""), ("x_marks", "true"), ("sample_pts", "true"), ("values", "\"Decimals\"")]),
        ("student-blank", vec![("show_answers", "false"), ("sum_line", "true"), ("exact_line", "true"), ("integral_line", "true"), ("error_line", "true"), ("show_dx", "true"), ("x_marks", "true"), ("sample_pts", "true")]),
        ("student-curve", vec![("show_answers", "false"), ("student", "\"Curve and bounds only\""), ("sum_line", "true"), ("exact_line", "true"), ("x_marks", "true")]),
        ("student-heights", vec![("show_answers", "false"), ("student", "\"Strip heights blank\""), ("strip_labels", "\"Heights\""), ("sum_line", "true"), ("x_marks", "true")]),
        ("print-all", vec![("style", "\"Print\""), ("shading", "\"Hatching\""), ("sum_line", "true"), ("exact_line", "true"), ("strip_labels", "\"Heights\""), ("x_marks", "true"), ("sample_pts", "true")]),
    ]
}

#[test]
fn the_checkers_sweep_is_clean() {
    let cases = classroom();
    assert_eq!(cases.len(), 100);
    let mut tot = std::collections::BTreeMap::<String, [usize; 6]>::new();
    let mut faults = vec![];
    let mut noted = vec![];
    for (name, base) in &cases {
        for (on, opts) in optsets() {
            for size in ["Small", "Medium", "Large"] {
                let mut j = base.trim_end_matches('}').to_string();
                for (k, v) in &opts { j.push_str(&format!(",\"{k}\":{v}")); }
                j.push_str(&format!(",\"label_size\":\"{size}\"}}"));
                let p = from_json(&j);
                let r = measure(&p);
                let e = tot.entry(size.to_string()).or_default();
                e[0] += 1; e[1] += r.faults.len().min(1); e[2] += r.expected; e[3] += r.drawn; e[4] += r.missing_noted; e[5] += r.missing_silent;
                for f in &r.faults { faults.push(format!("{size} {name} {on}: {f}")); }
                if r.missing_silent > 0 { faults.push(format!("{size} {name} {on}: {} left off silently", r.missing_silent)); }
                if r.missing_noted > 0 { noted.push(format!("{size} {name} {on}: {}", r.missing_noted)); }
            }
        }
    }
    println!("checker sweep [figures, with a fault, labels expected, drawn, left off with a note, left off silently]: {tot:?}");
    for n in &noted { println!("  left off with a note: {n}"); }
    assert!(faults.is_empty(), "{} faults:\n{}", faults.len(), faults.iter().take(60).cloned().collect::<Vec<_>>().join("\n"));
}
