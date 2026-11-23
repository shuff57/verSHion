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

/// The second checker's 120 more cases (its round-2 harness, riemann_r2.rs), ported unchanged.
fn extra_cases() -> Vec<(String, String)> {
    let mut out = vec![];
    let fns: Vec<(&str, &str, &str, f64, f64, f64, f64, f64, f64)> = vec![
        // f, a, b, xmin, xmax, xstep, ymin, ymax, ystep
        ("x^2", "0", "3", -1.0, 4.0, 1.0, -1.0, 10.0, 1.0),
        ("x^2 + 2", "-1", "2", -2.0, 3.0, 1.0, -1.0, 7.0, 1.0),
        ("9 - x^2", "-3", "3", -4.0, 4.0, 1.0, -1.0, 10.0, 1.0),
        ("x^3", "1", "2", -1.0, 3.0, 1.0, -1.0, 9.0, 1.0),
        ("2x", "1", "5", -1.0, 6.0, 1.0, -1.0, 11.0, 1.0),
        ("x + 1", "0", "4", -1.0, 5.0, 1.0, -1.0, 6.0, 1.0),
        ("1/x", "1", "e", -1.0, 4.0, 1.0, -0.5, 1.5, 0.5),
        ("1/(1 + x^2)", "-1", "1", -2.0, 2.0, 0.5, -0.5, 1.5, 0.5),
        ("e^(-x^2)", "0", "2", -1.0, 3.0, 0.5, -0.5, 1.5, 0.5),
        ("sqrt(x + 1)", "0", "3", -2.0, 4.0, 1.0, -1.0, 3.0, 0.5),
        ("cos(x) + 1", "0", "pi", -1.0, 4.0, 1.0, -1.0, 3.0, 0.5),
        ("x^2 - 4x + 5", "0", "4", -1.0, 5.0, 1.0, -1.0, 6.0, 1.0),
        ("8 - 2x", "0", "4", -1.0, 5.0, 1.0, -1.0, 9.0, 1.0),
        ("ln(x)", "1", "e^2", -1.0, 8.0, 1.0, -1.0, 3.0, 0.5),
        ("x^2/4", "0", "4", -1.0, 5.0, 1.0, -1.0, 5.0, 1.0),
        ("20 - x^2", "0", "4", -1.0, 5.0, 1.0, -2.0, 22.0, 2.0),
        ("100 - x^2", "0", "10", -2.0, 12.0, 2.0, -10.0, 110.0, 10.0),
        ("0.01x^3", "0", "10", -2.0, 12.0, 2.0, -1.0, 11.0, 1.0),
        ("x sin(x)", "0", "pi", -1.0, 4.0, 1.0, -0.5, 2.5, 0.5),
        ("3", "1", "4", -1.0, 5.0, 1.0, -1.0, 4.0, 1.0),
    ];
    for (f, a, b, x0, x1, xs, y0, y1, ys) in fns {
        for (n, rule) in [(4, "Left"), (5, "Right"), (8, "Midpoint"), (10, "Trapezoid"), (3, "Upper"), (6, "Lower")] {
            out.push((format!("{f} [{a},{b}] n={n} {rule}"), format!(r#"{{"f":"{f}","a":"{a}","b":"{b}","n":{n},"rule":"{rule}","xmin":{x0},"xmax":{x1},"xstep":{xs},"ymin":{y0},"ymax":{y1},"ystep":{ys}}}"#)));
        }
    }
    out
}

/// Label sets left off at Small or Medium with only one label option on, each with a note (named, and asserted to be all).
/// (Strips too short for a label near a curve's low point (x⁴ near 0, ln x near 1, x² − 2x near 0), or 12 to 15 px
/// wide (8 or 10 strips on a window 4 units wide); each left off with a note whose advice works, as
/// `refusal_advice_works` checks.)
const SMALL_MEDIUM_REFUSED: &[&str] = &[
    "Small x^4 [0,1] n=6 heights: strip labels",
    "Medium x^4 [0,1] n=6 heights: strip labels",
    "Small x^4 [0,1] n=6 areas: strip labels",
    "Medium x^4 [0,1] n=6 areas: strip labels",
    "Small ln(x) [1/2,3] n=4 areas: strip labels",
    "Medium ln(x) [1/2,3] n=4 areas: strip labels",
    "Medium x^2 + 2 [-1,2] n=8 Midpoint areas: strip labels",
    "Small x^3 [1,2] n=10 Trapezoid heights: strip labels",
    "Medium x^3 [1,2] n=10 Trapezoid heights: strip labels",
    "Small x^3 [1,2] n=10 Trapezoid areas: strip labels",
    "Medium x^3 [1,2] n=10 Trapezoid areas: strip labels",
];

#[test]
fn the_checkers_sweep_is_clean() {
    let mut cases = classroom();
    assert_eq!(cases.len(), 100);
    cases.extend(extra_cases());
    let mut sets: std::collections::BTreeMap<String, usize> = Default::default();
    let mut one_option: Vec<String> = vec![];
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
                // (a whole set left off: not the partial x₀, x₁, …, xₙ forms, which are drawn with their own note)
                for n in warnings(&p).iter().filter(|n| (n.contains("no room") || n.contains("do not fit")) && !n.contains("are marked")) {
                    let kind = if n.contains("strip labels") { "strip labels" } else if n.contains("x\u{2080}") { "x marks" } else { "a and b" };
                    *sets.entry(format!("{size} {kind}")).or_default() += 1;
                    if size != "Large" && matches!(on, "heights" | "areas" | "xmarks") { one_option.push(format!("{size} {name} {on}: {kind}")); }
                }
                let e = tot.entry(size.to_string()).or_default();
                e[0] += 1; e[1] += r.faults.len().min(1); e[2] += r.expected; e[3] += r.drawn; e[4] += r.missing_noted; e[5] += r.missing_silent;
                for f in &r.faults { faults.push(format!("{size} {name} {on}: {f}")); }
                if r.missing_silent > 0 { faults.push(format!("{size} {name} {on}: {} left off silently", r.missing_silent)); }
                if r.missing_noted > 0 { noted.push(format!("{size} {name} {on}: {}", r.missing_noted)); }
            }
        }
    }
    println!("checker sweep [figures, with a fault, labels expected, drawn, left off with a note, left off silently]: {tot:?}");
    println!("label sets left off (with a note) by size and kind: {sets:?}");
    for n in &one_option { println!("  one option on, left off at Small/Medium: {n}"); }
    let unnamed: Vec<&String> = one_option.iter().filter(|n| !SMALL_MEDIUM_REFUSED.contains(&n.as_str())).collect();
    assert!(unnamed.is_empty() && one_option.len() == SMALL_MEDIUM_REFUSED.len(), "unnamed: {unnamed:#?}");
    assert!(faults.is_empty(), "{} faults:\n{}", faults.len(), faults.iter().take(60).cloned().collect::<Vec<_>>().join("\n"));
}

/// The third checker's 223 cases (its round-3 harness, riemann_r3.rs `mine`, ported unchanged).
fn detach_cases() -> Vec<(String, String)> {
    let fns: Vec<(&str, &str, &str)> = vec![
        ("x^2", "0", "4"), ("x^2", "-2", "2"), ("x^2 + 3", "0", "3"), ("2x^2", "0", "2"), ("x^3", "0", "3"), ("x^3 + 1", "-1", "2"),
        ("9 - x^2", "0", "3"), ("4 - x^2", "-1", "2"), ("3x + 2", "0", "4"), ("5 - x", "0", "5"), ("x/3", "0", "6"), ("sqrt(x)", "0", "9"),
        ("sqrt(x) + 1", "0", "4"), ("2sqrt(x)", "1", "4"), ("1/x", "1", "5"), ("2/x", "1", "3"), ("1/(x + 1)", "0", "3"), ("e^x", "-1", "1"),
        ("e^(-x)", "0", "2"), ("3e^(-x/2)", "0", "4"), ("ln(x)", "1", "5"), ("ln(x + 1)", "0", "3"), ("sin(x)", "0", "pi/2"), ("cos(x)", "-pi/2", "pi/2"),
        ("sin(x) + 1", "0", "2pi"), ("2cos(x)", "0", "pi"), ("x^2 - 6x + 10", "1", "5"), ("(x - 1)^2", "0", "3"), ("12 - 3x^2", "0", "2"),
        ("x^4 - 2x^2 + 2", "-1", "2"), ("x^(1/3)", "0", "8"), ("abs(x)", "-2", "3"), ("x^2/2 + 1", "0", "4"), ("1 + 1/x^2", "1", "3"),
    ];
    let combos = [(4, "Left"), (4, "Right"), (5, "Midpoint"), (6, "Trapezoid"), (8, "Upper"), (3, "Lower"), (10, "Midpoint"), (5, "Left")];
    let mut out = vec![];
    for (fi, (f, a, b)) in fns.iter().enumerate() {
        let av = eval_typed(a).unwrap(); let bv = eval_typed(b).unwrap();
        let (mut lo, mut hi) = (0.0f64, 0.0f64);
        for i in 0..=400 { let x = av + (bv - av) * i as f64 / 400.0; if let Some(y) = ev(f, x) { lo = lo.min(y); hi = hi.max(y); } }
        let ys = nice((hi - lo).max(1.0) * 1.15);
        let ymin = ((lo / ys).floor() - 1.0) * ys; let ymax = ((hi / ys).ceil() + 1.0) * ys;
        let pi = b.contains("pi");
        for k in 0..6 {
            let (n, rule) = combos[(fi + k) % combos.len()];
            let loose = k % 2 == 1;
            let (xmin, xmax, xstep, xu) = if pi {
                let (l, h) = (av / PI, bv / PI);
                if loose { ((l - 1.0).floor().min(-1.0), h.ceil() + 1.0, 0.5, "Multiples of \u{3c0}") } else { ((l - 0.25).min(-0.25), h + 0.25, 0.25, "Multiples of \u{3c0}") }
            } else if loose {
                let s = nice((bv - av) * 2.5);
                ((((av - (bv - av)) / s).floor() * s).min(-s), ((bv + (bv - av) / 2.0) / s).ceil() * s, s, "Numbers")
            } else {
                let s = nice((bv - av + 2.0).max(2.0) * 1.1);
                ((((av - 1.0) / s).floor() * s).min(-s), ((bv + 1.0) / s).ceil() * s, s, "Numbers")
            };
            out.push((format!("{f} [{a},{b}] n={n} {rule} {}", if loose { "loose" } else { "tight" }), format!(r#"{{"f":"{f}","a":"{a}","b":"{b}","n":{n},"rule":"{rule}","xmin":{xmin:.4},"xmax":{xmax:.4},"xstep":{xstep:.4},"ymin":{ymin:.4},"ymax":{ymax:.4},"ystep":{ys:.4},"xunits":"{xu}"}}"#)));
        }
    }
    for (t, x1, y1, xs, ys) in [
        ("0, 4; 2, 7; 3, 9; 6, 8; 8, 5", 9.0, 10.0, 1.0, 1.0), ("0, 12; 4, 15; 9, 18; 12, 14; 15, 10", 16.0, 20.0, 1.0, 2.0),
        ("1, 3; 1.5, 4; 2.5, 6; 3, 5; 4.5, 2", 5.0, 7.0, 0.5, 1.0), ("0, 0; 5, 20; 10, 35; 20, 50; 30, 55", 32.0, 60.0, 5.0, 10.0),
        ("0, 100; 2, 80; 4, 65; 6, 50; 8, 42; 10, 35", 11.0, 110.0, 1.0, 10.0), ("0, 1.2; 0.5, 1.8; 1, 2.1; 2, 2.6; 3, 2.2", 3.5, 3.0, 0.5, 0.5),
    ] {
        for rule in ["Left", "Trapezoid"] {
            out.push((format!("table {t} {rule}"), format!(r#"{{"region":"From a table of values","table":"{t}","rule":"{rule}","xmin":-{xs},"xmax":{x1},"xstep":{xs},"ymin":-{ys},"ymax":{y1},"ystep":{ys}}}"#)));
        }
    }
    for (f, a, b, n, x0, x1, y0, y1) in [("x^2 - 4", "0", "3", 6, -1.0, 4.0, -5.0, 6.0), ("x - 2", "0", "5", 5, -1.0, 6.0, -3.0, 4.0), ("cos(x)", "0", "pi", 6, -1.0, 4.0, -1.5, 1.5), ("x^3 - 3x^2 + 2x", "0", "3", 6, -1.0, 4.0, -2.0, 7.0)] {
        out.push((format!("signed {f}"), format!(r#"{{"f":"{f}","a":"{a}","b":"{b}","n":{n},"signed":true,"rule":"Midpoint","xmin":{x0},"xmax":{x1},"ymin":{y0},"ymax":{y1},"ystep":0.5}}"#)));
    }
    for (f, g, n) in [("x", "x^2", 4), ("x + 2", "x^2", 6), ("2x", "x^2", 5)] {
        out.push((format!("between {f} {g}"), format!(r#"{{"region":"Between f(x) and g(x)","f":"{f}","g":"{g}","bounds":"Where f and g cross","n":{n},"rule":"Midpoint","xmin":-2.0,"xmax":3.0,"ymin":-1.0,"ymax":5.0}}"#)));
    }
    out
}


/// The third checker's measures, ported: no strip label under the row of axis numbers, none more than 22 px from its
/// own strip, and (the fourth checker's) no a or b label in the row of x-axis numbers, on its 223 cases with heights or
/// areas on, at every size.
#[test]
fn strip_labels_sit_on_their_strips() {
    let cases = detach_cases();
    assert!(cases.len() >= 223, "{}", cases.len());
    let mut tot: std::collections::BTreeMap<String, [usize; 5]> = Default::default();
    let mut bad = vec![];
    let mut refused: std::collections::BTreeMap<String, usize> = Default::default();
    for (name, base) in &cases {
        for on in ["\"Heights\"", "\"Areas\""] {
            for size in ["Small", "Medium", "Large"] {
                let j = format!("{},\"strip_labels\":{on},\"label_size\":\"{size}\"}}", base.trim_end_matches('}'));
                let p = from_json(&j);
                let (svg, notes) = build(&p);
                if notes.iter().any(|n| n.contains("strip labels do not fit")) { *refused.entry(format!("{size} {on}")).or_default() += 1; }
                let Ok(m) = model(&p) else { continue };
                let g = grid_layout(&grid_params(&p));
                let sx = |x: f64| g.ml + (x - g.x0) * g.ux;
                let sy = |y: f64| g.mt + g.ph - (y - g.y0) * g.uy;
                let axy = sy(if g.y0 <= 0.0 && g.y1 >= 0.0 { 0.0 } else { g.y0 });
                // (and no a or b label in the row of x-axis numbers)
                let row = tags(&svg, "<text ").into_iter().filter(|t| t.contains(r#"data-axis="0""#)).filter_map(|t| { let (y, fs) = (attr(t, "y")?.parse::<f64>().ok()?, attr(t, "font-size")?.parse::<f64>().ok()?); Some((y - 0.8 * fs, y + 0.25 * fs)) }).fold(None, |acc: Option<(f64, f64)>, (a, b)| Some(acc.map_or((a, b), |(x, y)| (x.min(a), y.max(b)))));
                if let Some((r0, r1)) = row { for l in labels(&svg).into_iter().filter(|l| l.0 == "bound") { if l.2[1] < r1 && l.2[3] > r0 { bad.push(format!("{size} {on} {name}: {:?} in the number row", l.1)); } } }
                let labs: Vec<(String, String, [f64; 4])> = labels(&svg).into_iter().filter(|l| l.0 == "strip-label").collect();
                if labs.is_empty() { continue; }
                let inwin: Vec<usize> = (0..m.strips.len()).filter(|&i| { let c = 0.5 * (sx(m.strips[i].x0) + sx(m.strips[i].x1)); c >= g.ml && c <= g.ml + g.pw }).collect();
                let (mut under, mut far) = (0, 0);
                for (k, l) in labs.iter().enumerate() {
                    let Some(&i) = inwin.get(k) else { continue };
                    let st = &m.strips[i];
                    let (y0, y1) = { let (a, b) = (sy(st.top).clamp(g.mt, g.mt + g.ph), sy(st.base).clamp(g.mt, g.mt + g.ph)); (a.min(b), a.max(b)) };
                    let gap = (y0 - l.2[3]).max(l.2[1] - y1).max(0.0);
                    if l.2[1] > axy + 12.0 && y1 <= axy + 0.5 { under += 1; }
                    if gap > 22.0 { far += 1; }
                }
                let e = tot.entry(format!("{size} {on}")).or_default();
                e[0] += 1; e[1] += (under > 0) as usize; e[2] += (far > 0) as usize; e[3] += under; e[4] += far;
                if under + far > 0 { bad.push(format!("{size} {on} {name}: under {under}, far {far}")); }
            }
        }
    }
    println!("[figures with labels, with a label under the numbers, with one far, labels under, labels far]: {tot:?}");
    println!("label sets left off with a note: {refused:?}");
    assert!(bad.is_empty(), "{} figures:\n{}", bad.len(), bad.iter().take(30).cloned().collect::<Vec<_>>().join("\n"));
}
