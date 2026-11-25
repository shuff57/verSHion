//! Limit questions on the coordinate grid's own graphs (the new-figures checklist `coordinate-grid-limits`).
//!
//! The coordinate grid draws its rows exactly as it always has. When a limit control is on, the rows that make f (y = rows,
//! pieces with their domains) and the grid's points are handed to the limits figure's own reader and analysis
//! (`Model`), and this module adds what that figure adds: the horizontal asymptotes found from f's end behaviour (dashed,
//! with their equations), each discontinuity's kind on the graph, and the questions with their key under the graph.
//! Every control here is off by default and the grid's drawing is returned untouched while they are all off, so every
//! existing grid drawing, saved file and share link opens as before.
use super::*;
use crate::figures::ParamDef;
use crate::figures::math::{coordinate_grid_build_with, GridHooks};

pub const INF_CHOICES: [&str; 4] = ["None", "x \u{2192} \u{221e}", "x \u{2192} \u{2212}\u{221e}", "Both"];

/// The grid's limit controls (appended to its params; every default leaves the drawing as it was).
pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("lim_ask_at", "Ask the limits at x =", "").hint("One or more values of a, such as 1 or -2, 1, 4: the left and right limits, the limit and f(a) for each, with the answers in the key. Empty = none."),
        ParamDef::choice("lim_inf", "Ask the limits at infinity", &INF_CHOICES, "None"),
        ParamDef::bool("lim_disc", "Ask where f is not continuous", false),
        ParamDef::bool("lim_hasym", "Draw f's horizontal asymptotes (dashed, with their equations)", false),
        ParamDef::bool("lim_kinds", "Label each discontinuity's kind (removable, jump, infinite)", false),
        ParamDef::text("lim_rows", "f is the rows", "").hint("Row numbers of the Equations box, such as 1, or 1, 2, 3 for a function typed in pieces (y = x + 2, x < 1; y = 4 - 2x, x >= 1). Empty: every y = row."),
        ParamDef::bool("lim_points", "The points give f's values (a point at a hole is f(a))", true),
        ParamDef::choice("lim_inf_answer", "An infinite limit is written", &["\u{221e} or \u{2212}\u{221e}", "does not exist"], "\u{221e} or \u{2212}\u{221e}"),
        ParamDef::choice("lim_dne", "Does not exist is written", &["does not exist", "DNE"], "does not exist"),
        ParamDef::bool("lim_stu_kinds", "Student copy: leave off the kind labels", true),
        ParamDef::bool("lim_stu_asym", "Student copy: leave off the asymptotes' equations", true),
    ]
}

/// Keys of the limit controls, for the panel.
pub const KEYS: [&str; 11] = ["lim_ask_at", "lim_inf", "lim_disc", "lim_hasym", "lim_kinds", "lim_rows", "lim_points", "lim_inf_answer", "lim_dne", "lim_stu_kinds", "lim_stu_asym"];

/// Any limit control on: only then is anything added to the grid.
pub fn active(p: &Params) -> bool {
    let s = |k: &str| match p.0.get(k) { Some(Value::S(t)) => t.clone(), _ => String::new() };
    let b = |k: &str| matches!(p.0.get(k), Some(Value::B(true)));
    !s("lim_ask_at").trim().is_empty() || (!s("lim_inf").is_empty() && s("lim_inf") != "None") || b("lim_disc") || b("lim_hasym") || b("lim_kinds")
}

/// Shown in the panel only where they change the drawing.
pub fn relevant(key: &str, p: &Params) -> bool {
    match key {
        "lim_ask_at" | "lim_inf" | "lim_disc" | "lim_hasym" | "lim_kinds" => true,
        "lim_rows" | "lim_points" => active(p),
        "lim_inf_answer" | "lim_dne" => !p.s("lim_ask_at").trim().is_empty() || p.s("lim_inf") != "None",
        "lim_stu_kinds" => p.b("lim_kinds"),
        "lim_stu_asym" => p.b("lim_hasym"),
        _ => true,
    }
}

/// Whether the grid draws a row as the limit questions read it: across the window (and at its domain's ends and its holes)
/// the grid's own reader and the limits reader give the row a value at the same x's, and the same value. A row the grid
/// draws only for x ≥ 0 (x^(1/3)) or cannot draw at all (floor x) gives no answers about a curve that is not on the page.
fn drawn_as_read(t: &str, x0: f64, x1: f64) -> bool {
    let mut tmp = vec![];
    // (a row the limits reader refuses is said by the model's own note)
    let Some(pc) = read_line(1, 0, t, &mut tmp) else { return true };
    let Src::Formula { x: ref fx } = pc.src else { return true };
    let (main, dom) = split_domain(t);
    let Some(Row::Func(e)) = parse_row(main) else { return false };
    let ivs = dom.map(|d| crate::figures::math::parse_ineq(&d.to_lowercase().replace('y', "x"))).unwrap_or_default();
    let in_dom = |v: f64| ivs.is_empty() || ivs.iter().any(|iv| (v > iv.lo || (iv.lo_c && v >= iv.lo)) && (v < iv.hi || (iv.hi_c && v <= iv.hi)));
    let grid_v = |x: f64| -> Option<f64> { if !in_dom(x) { return None; } crate::figures::math::eval_full(&e, x, None, false).filter(|v| v.is_finite()) };
    let lim_v = |x: f64| -> Option<f64> { if !pc.includes(x) { return None; } cas::ev(fx, x).filter(|v| v.is_finite()) };
    let mut xs: Vec<f64> = (0..=400).map(|i| x0 + (x1 - x0) * (i as f64 + 0.37) / 401.0).collect();
    for e in pc.lo.iter().chain(pc.hi.iter()) { xs.push(e.x.v); }
    for h in &pc.holes { xs.push(h.0.v); }
    xs.into_iter().filter(|x| *x >= x0 && *x <= x1).all(|x| match (grid_v(x), lim_v(x)) {
        (None, None) => true,
        (Some(a), Some(b)) => a.abs() > 1e6 || b.abs() > 1e6 || (a - b).abs() <= 1e-6 * (1.0 + a.abs()),
        // (beside a pole one reader may overflow where the other does not)
        (Some(a), None) | (None, Some(a)) => a.abs() > 1e6,
    })
}

/// The row numbers (1-based) of the rows that make f, with their texts (style words taken off).
fn f_rows(p: &Params, notes: &mut Vec<String>) -> Vec<(usize, String)> {
    let g = grid_layout(p);
    let rows = f_rows_typed(p, notes);
    rows.into_iter().filter(|(n, t)| {
        let same = drawn_as_read(t, g.x0, g.x1);
        if !same { notes.push(format!("Row {n} is drawn differently from how the limit questions would read it (part of it is not drawn, or it is not drawn at all), so no question is asked about it: {}", t.trim())); }
        same
    }).collect()
}
fn f_rows_typed(p: &Params, notes: &mut Vec<String>) -> Vec<(usize, String)> {
    let rows: Vec<&str> = p.s("equations").split(';').collect();
    let is_func = |t: &str| { let (main, _) = split_domain(t); matches!(parse_row(main), Some(Row::Func(e)) if !e.trim().is_empty()) };
    let pick = p.s("lim_rows").trim().to_string();
    let mut out = vec![];
    if pick.is_empty() {
        for (i, r) in rows.iter().enumerate() {
            let (t, _) = crate::figures::math::unstyle(r);
            if !t.trim().is_empty() && is_func(&t) { out.push((i + 1, t.trim().to_string())); }
        }
        if out.is_empty() { notes.push("The limit questions need f: type it in Equations as y = (a formula in x), with a domain for each piece, such as y = x + 2, x < 1; y = 4 - 2x, x >= 1".into()); }
        return out;
    }
    for tok in pick.split([',', ';', ' ']).filter(|t| !t.trim().is_empty()) {
        let Ok(n) = tok.trim().parse::<usize>() else { notes.push(format!("\u{201c}{}\u{201d} in f is the rows is not a row number: type numbers such as 1, 2", tok.trim())); continue };
        let Some(r) = rows.get(n.wrapping_sub(1)).filter(|_| n >= 1) else { notes.push(format!("f is the rows: there is no row {n} in Equations")); continue };
        let (t, _) = crate::figures::math::unstyle(r);
        if t.trim().is_empty() { notes.push(format!("f is the rows: row {n} is empty")); continue; }
        if !is_func(&t) { notes.push(format!("Row {n} is not a function of x (y = a formula in x), so it is not part of f: {}", t.trim())); continue; }
        if !out.iter().any(|(m, _)| *m == n) { out.push((n, t.trim().to_string())); }
    }
    out
}

/// The grid's points as single points "(x, y)", in the words they were typed.
fn f_dots(p: &Params) -> String {
    let mut out = vec![];
    for seg in p.s("points").split(';') {
        let mut at = 0;
        while let Some(a) = seg[at..].find('(') {
            let a = at + a;
            let Some(b) = seg[a..].find(')').map(|b| a + b) else { break };
            let inner = &seg[a + 1..b];
            if inner.split(',').count() == 2 { out.push(format!("({inner})")); }
            at = b + 1;
        }
    }
    out.join(", ")
}

/// The limits figure's params for the grid's f and questions.
fn limit_params(p: &Params, rows: &[(usize, String)]) -> Params {
    let defs = super::params();
    let mut q = Params::defaults(&defs);
    q.set_s("pieces", &rows.iter().map(|r| r.1.clone()).collect::<Vec<_>>().join("\n"));
    q.set_s("dots", &if p.b("lim_points") { f_dots(p) } else { String::new() });
    q.set_s("ask_at", p.s("lim_ask_at"));
    q.set_s("ask_inf", p.s("lim_inf"));
    q.set_b("ask_disc", p.b("lim_disc"));
    q.set_b("kind_labels", p.b("lim_kinds"));
    q.set_s("inf_answer", p.s("lim_inf_answer"));
    q.set_s("dne_word", p.s("lim_dne"));
    q.set_b("stu_hide_kinds", p.b("lim_stu_kinds"));
    q.set_b("show_answers", p.b("show_answers"));
    q.set_s("style", p.s("style"));
    q.set_s("label_size", p.s("label_size"));
    q
}

/// The limits model's notes in the grid's words: its piece k is the k-th row of f.
fn in_grid_words(n: &str, rows: &[(usize, String)]) -> String {
    let mut s = n.to_string();
    // (longest numbers first, so "Piece 12" is not read as "Piece 1")
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by_key(|&k| std::cmp::Reverse(k));
    for k in order {
        let r = rows[k].0;
        s = s.replace(&format!("Pieces {} and ", k + 1), &format!("Rows \u{1}{r} and "));
        s = s.replace(&format!(" and {} both", k + 1), &format!(" and \u{1}{r} both"));
        s = s.replace(&format!("Piece {} ", k + 1), &format!("Row \u{1}{r} "));
        s = s.replace(&format!("Piece {}:", k + 1), &format!("Row \u{1}{r}:"));
        s = s.replace(&format!("piece {} ", k + 1), &format!("row \u{1}{r} "));
    }
    s.replace('\u{1}', "")
        .replace("Type the function in Function (one piece a line), such as y = x^2 or y = x + 1, x < 2", "Type f in Equations, such as y = x^2 or y = x + 1, x < 2")
        .replace("is not drawn: f already has the value", "is not taken as a value of f: f already has the value")
}

/// The grid's drawing with the limit work added (only while a limit control is on), and its notes.
pub fn render(p0: &Params, notes: &mut Vec<String>) -> String {
    if !active(p0) { return coordinate_grid_build(p0, notes); }
    let mut pn = p0.clone();
    pn.normalize(&coordinate_grid_params(), false);
    let p = &pn;
    if p.s("xunits") == "Degrees" {
        notes.push("The limit questions work in radians: set the x-axis units to Numbers or Multiples of \u{3c0} to ask them".into());
        return coordinate_grid_build(p0, notes);
    }
    let print = p.s("style") == "Print";
    let ink = inks(print);
    let stu = !p.b("show_answers");
    let ls = match p.s("label_size") { "Large" => 1.4, "Small" => 0.8, _ => 1.0 };
    let size = 15.0 * ls;
    let mut lnotes: Vec<String> = vec![];
    let rows = f_rows(p, &mut lnotes);
    let lp = limit_params(p, &rows);
    // (other curves drawn beside f: the questions name f, so the graph has to say which curve that is)
    let others: Vec<usize> = p.s("equations").split(';').enumerate().filter(|(_, r)| !crate::figures::math::unstyle(r).0.trim().is_empty()).map(|(i, _)| i + 1).filter(|i| !rows.iter().any(|r| r.0 == *i)).collect();
    if !rows.is_empty() && !others.is_empty() && !p.b("eq_labels") {
        lnotes.push(format!("The questions are about f, rows {}, and the grid draws other rows too ({}): write each equation on its line, or say in the title which curve is f", rows.iter().map(|r| r.0.to_string()).collect::<Vec<_>>().join(", "), others.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", ")));
    }
    let g = grid_layout(p);
    let m = Model::new(&lp, &g);
    // (two rows of f over the same x give f two values there: no limit is asked about a function that is not one)
    if m.notes.iter().any(|n| n.starts_with("Pieces ") && n.contains("both cover")) {
        for n in m.notes.iter().filter(|n| n.starts_with("Pieces ") && n.contains("both cover")) {
            let t = format!("{}; no limit question is asked until f is one function: name its rows in f is the rows", in_grid_words(n, &rows).trim_end_matches(", so change one of the domains").replace(": a function has one value for each x, so change one of the domains", ""));
            if !notes.contains(&t) { notes.push(t); }
        }
        for n in &lnotes { let t = in_grid_words(n, &rows); if !notes.contains(&t) { notes.push(t); } }
        return coordinate_grid_build(p0, notes);
    }
    let _ = m.piles();
    lnotes.extend(m.notes.iter().cloned());
    let mut dr = Drawn::default();
    let sx = |x: f64| g.ml + (x - g.x0) * g.ux;
    let sy = |y: f64| g.mt + g.ph - (y - g.y0) * g.uy;
    let rect: Bx = [g.ml, g.mt, g.ml + g.pw, g.mt + g.ph];
    let inside_y = |y: f64| y >= g.y0 - 1e-9 && y <= g.y1 + 1e-9;
    // horizontal asymptotes, found from the end behaviour
    let mut asym_svg = String::new();
    let mut asym_lines: Vec<(String, Pt, Pt)> = vec![];
    let (adash, aw) = if print { ("7 5", 1.6) } else { ("6 4", 1.5) };
    if p.b("lim_hasym") && !(m.pieces.is_empty() && m.dots.is_empty()) {
        let horiz = m.horizontals();
        if horiz.is_empty() { lnotes.push("f has no horizontal asymptote (no finite limit as x \u{2192} \u{221e} or x \u{2192} \u{2212}\u{221e}), so none is drawn".into()); }
        for (y, t) in &horiz {
            if !inside_y(*y) { lnotes.push(format!("The horizontal asymptote y = {} is outside the window: change the y-axis to show it", t.replace('-', "\u{2212}"))); continue; }
            let (a, b) = ((rect[0], sy(*y)), (rect[2], sy(*y)));
            // (over a white stroke that takes the gridline out from under it, so the dashes read even on a gridline; under the curves)
            asym_svg.push_str(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#fff" stroke-width="{:.1}"/>"##, a.0, a.1, b.0, b.1, aw + 2.5));
            asym_svg.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{aw}" stroke-dasharray="{adash}" data-lim="hasym"/>"#, a.0, a.1, b.0, b.1, ink.asym));
            asym_lines.push((format!("y = {t}"), a, b));
        }
    }

    // the grid's own drawing, the asymptotes between its gridlines and its curves
    let mut gnotes = vec![];
    let svg = coordinate_grid_build_with(p0, &mut gnotes, &GridHooks { late: false, over_grid: format!(r#"<g data-lim="lines">{asym_svg}</g>"#) });
    notes.extend(gnotes);
    let (gw, gh) = crate::figures::svg_size(&svg);

    // what a label must keep clear of: the grid's curves, axes, circles, points and texts
    let mut board = Board { bounds: [rect[0] + 2.0, rect[1] + 2.0, rect[2] - 2.0, rect[3] - 2.0], ..Default::default() };
    let ax_y = if g.y0 <= 0.0 && g.y1 >= 0.0 { 0.0 } else { g.y0 };
    let ax_x = if g.x0 <= 0.0 && g.x1 >= 0.0 { 0.0 } else { g.x0 };
    for (a, b) in [((g.ml - 20.0, sy(ax_y)), (g.ml + g.pw + 20.0, sy(ax_y))), ((sx(ax_x), g.mt - 20.0), (sx(ax_x), g.mt + g.ph + 20.0))] { board.segs.push((a, b)); }
    let body = grid_body(&svg);
    for pl in polylines(&body) {
        let clipped: Vec<Pt> = pl.iter().map(|q| (q.0.clamp(rect[0] - 1.0, rect[2] + 1.0), q.1.clamp(rect[1] - 1.0, rect[3] + 1.0))).collect();
        for w in clipped.windows(2) { board.segs.push((w[0], w[1])); }
    }
    for part in body.split("<circle").skip(1) {
        if let (Some(cx), Some(cy)) = (attr(part, "cx"), attr(part, "cy")) { board.dots.push(((cx, cy), attr(part, "r").unwrap_or(5.0) + 1.0)); }
    }
    let (_, gtexts) = grid_texts(&body);
    for t in &gtexts { board.boxes.push(grow(t.bx, 1.0)); }
    // (the gridlines are drawn as lines, not polylines: a label may sit over one on its white plate)

    for (_, a, b) in &asym_lines { board.dashed.push((*a, *b)); }

    // the labels on the plot: each discontinuity's kind, and each asymptote's equation
    let (discs, discs_complete) = m.discontinuities();
    let mut reqs: Vec<(char, Req)> = vec![];
    let cr = 5.0;
    let show_kinds = p.b("lim_kinds") && !(stu && p.b("lim_stu_kinds"));
    let many_kinds = discs.iter().filter(|d| d.kind != UNSURE).count() > 12;
    let mut unlabelled: Vec<String> = vec![];
    if show_kinds && many_kinds { lnotes.push(format!("Too many discontinuities to label each one on the graph: {}", if p.b("lim_disc") { "the key lists them" } else { "ask where f is not continuous to list them in the key" })); }
    if show_kinds && !many_kinds {
        if discs.is_empty() && discs_complete && !(m.pieces.is_empty() && m.dots.is_empty()) { lnotes.push("f has no discontinuities in the window, so there is no kind to label".into()); }
        for d in &discs {
            let x = sx(d.x);
            let ys: Vec<f64> = [d.left.num(), d.right.num(), d.value.num()].into_iter().flatten().collect();
            let on_plot = d.kind == "infinite" || ys.iter().any(|y| inside_y(*y));
            if d.kind == UNSURE { continue; }
            if (d.x - g.x0).abs() < 1e-9 * (1.0 + d.x.abs()) || (d.x - g.x1).abs() < 1e-9 * (1.0 + d.x.abs()) || !on_plot { unlabelled.push(eq_text("x", &d.text)); continue; }
            let clampy = |y: f64| y.clamp(g.y0, g.y1);
            let anchors: Vec<Pt> = match (d.left.num(), d.right.num()) {
                _ if d.kind == "infinite" => vec![(x, rect[1] + 0.25 * g.ph), (x, rect[1] + 0.5 * g.ph), (x, rect[1] + 0.75 * g.ph)],
                (Some(l), Some(r)) if d.kind == "jump" => { let mut v = vec![(x, sy(clampy(0.5 * (l + r))))]; for y in [l, r] { if inside_y(y) { v.push((x, sy(y))); } } v }
                (Some(l), _) => vec![(x, sy(clampy(l)))],
                _ => vec![(x, sy(0.5 * (g.y0 + g.y1)))],
            };
            let own: Vec<(Pt, Pt)> = match (d.left.num(), d.right.num()) {
                (Some(l), Some(r)) => vec![((x, sy(clampy(l))), (x, sy(clampy(r))))],
                _ if d.kind == "infinite" => vec![((x, rect[1]), (x, rect[3]))],
                _ => anchors.iter().map(|q| (*q, *q)).collect(),
            };
            let w = tw(d.kind, size);
            let mut tries = vec![];
            for an in &anchors {
                for gap in [3.0, 6.0, 10.0, 16.0, 24.0, 34.0] {
                    let (hx, hy) = (w / 2.0 + cr + gap, 0.5 * size + cr + gap * 0.6);
                    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (1.0, -1.0), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (0.0, -1.0), (0.0, 1.0)] { tries.push((an.0 + dx * hx, an.1 + dy * hy)); }
                    for (dx, dy) in [(1.0, -1.0), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] { tries.push((an.0 + dx * (w / 2.0 - 6.0), an.1 + dy * hy)); }
                }
            }
            // (and then a ring around each of the break's marks: every 15°, from 2 px to 60 px clear of it, so the open side of a
            // hole is reached where the spots above are all refused)
            let hh = 0.5 * size;
            for gap in [2.0, 5.0, 9.0, 14.0, 20.0, 28.0, 38.0, 48.0, 60.0] {
                for an in &anchors {
                    for k in 0..24 {
                        let t = k as f64 * std::f64::consts::PI / 12.0;
                        tries.push((an.0 + t.cos() * (w / 2.0 + cr + gap), an.1 - t.sin() * (hh + cr + gap)));
                    }
                }
            }
            // (which break a label is read as belonging to is judged on its box against every break's marks, below)
            let gap = if d.kind == "removable" { cr + 2.0 } else { 1.5 };
            reqs.push(('k', Req { text: d.kind.to_string(), own, tries, anchor: anchors[0], rivals: vec![], gap }));
        }
    }
    for (t, a, b) in asym_lines.iter().filter(|_| !(stu && p.b("lim_stu_asym"))) {
        let w = tw(t, size);
        let n = 24;
        let mut ts: Vec<f64> = (0..n).map(|j| j as f64 / (n - 1) as f64).collect();
        ts.sort_by(|u, v| (u.min(1.0 - u), *u).partial_cmp(&(v.min(1.0 - v), *v)).unwrap());
        let mut tries = vec![];
        for t in ts { for side in [1.0, -1.0] { tries.push((rect[2] - 4.0 - w / 2.0 - (rect[2] - rect[0] - 8.0 - w) * t, a.1 - side * (size * 0.5 + 4.0))); } }
        reqs.push(('a', Req { text: t.clone(), own: vec![(*a, *b)], tries, anchor: (rect[2] - 40.0, a.1), rivals: asym_lines.iter().filter(|o| o.1 != *a || o.2 != *b).map(|o| (o.1, o.2)).collect(), gap: 1.5 }));
    }
    let kind_owns: Vec<Vec<(Pt, Pt)>> = reqs.iter().filter(|r| r.0 == 'k').map(|r| r.1.own.clone()).collect();
    for (i, r) in reqs.iter_mut().filter(|r| r.0 == 'k').enumerate() { r.1.rivals = kind_owns.iter().enumerate().filter(|(j, _)| *j != i).flat_map(|(_, o)| o.clone()).collect(); }
    if !unlabelled.is_empty() { lnotes.push(format!("Not labelled on the graph (on the window's edge or off the plot; the key names them when you ask where f is not continuous): {}", unlabelled.join(", ").replace('-', "\u{2212}"))); }
    let mut labels_svg = String::new();
    let mut beside: Vec<String> = vec![];
    // Each break's marks, in px: its one-sided limits' heights and its value at x (the circles and dots drawn there), or its
    // whole upright for an infinite one. A kind label is accepted only where its box is nearer its own break's marks than any
    // other break's, by a margin, and, on a leader, only where the leader slants clearly (never along a gridline). Otherwise
    // the placer, shared with the limits figure, is asked again with the refused spot blocked.
    let marks_of = |d: &Disc| -> Vec<(Pt, Pt)> {
        let x = sx(d.x);
        if d.kind == "infinite" { return vec![((x, rect[1]), (x, rect[3]))]; }
        [d.left.num(), d.right.num(), d.value.num()].into_iter().flatten().filter(|y| inside_y(*y)).map(|y| ((x, sy(y)), (x, sy(y)))).collect()
    };
    let disc_marks: Vec<(String, Vec<(Pt, Pt)>)> = discs.iter().filter(|d| d.kind != UNSURE).map(|d| (d.kind.to_string(), marks_of(d))).collect();
    let mark_d = |ms: &[(Pt, Pt)], b: Bx| ms.iter().map(|(a, c)| seg_box_dist(*a, *c, b)).fold(f64::MAX, f64::min);
    let owner = |r: &Req| -> Option<usize> { discs.iter().filter(|d| d.kind != UNSURE).position(|d| (sx(d.x) - r.anchor.0).abs() < 0.5) };
    let acceptable = |kind: char, r: &Req, spot: &Spot| -> bool {
        // (a leader ties the label to its own mark, wherever the label sits, so long as it can be seen: clearly slanting, at
        // least about 14° off upright and level, never along a gridline)
        // (and its text, at the leader's end, is held to the same rule as a label beside its break: a reader matches a word to
        // the mark it sits by, whatever line runs to it)
        let c = match spot { Spot::Beside(c) => *c, Spot::Leader(c, s, e) => { let (dx, dy) = ((s.0 - e.0).abs(), (s.1 - e.1).abs()); if !(dx >= 0.25 * dy && dy >= 0.25 * dx) { return false; } *c } Spot::Off => return true };
        if kind != 'k' { return true; }
        let Some(i) = owner(r) else { return true };
        let b = label_box_for(c, &r.text, size);
        let own = mark_d(&disc_marks[i].1, b);
        // (beside its break, it is also centred nearer that break's x than any other's: a label is read by its column; one
        // on a leader is read by its leader)
        let own_x = disc_marks[i].1.first().map_or(c.0, |m| m.0 .0);
        let centred = matches!(spot, Spot::Leader(..)) || !disc_marks.iter().enumerate().filter(|(j, _)| *j != i).any(|(_, (_, ms))| ms.first().is_some_and(|m| (c.0 - m.0 .0).abs() < (c.0 - own_x).abs() + 4.0));
        // (or, wider than the gap between two breaks, it hugs its own mark: within 12 px of it, every other break's marks at
        // least three times as far)
        let hugs = own <= 12.0 && disc_marks.iter().enumerate().filter(|(j, _)| *j != i).all(|(_, (_, ms))| mark_d(ms, b) >= 3.0 * own.max(4.0));
        (centred || hugs) && disc_marks.iter().enumerate().filter(|(j, _)| *j != i).all(|(_, (_, ms))| mark_d(ms, b) >= own + 6.0)
    };
    for (kind, r0) in &reqs {
        // (the spots beside the break that the nearest-mark rule would refuse are never offered: the placer then returns
        // an acceptable one if there is any, and only leaders can still be refused and retried)
        let tries: Vec<Pt> = r0.tries.iter().copied().filter(|c| acceptable(*kind, r0, &Spot::Beside(*c))).collect();
        let r = &Req { text: r0.text.clone(), own: r0.own.clone(), tries, anchor: r0.anchor, rivals: r0.rivals.clone(), gap: r0.gap };
        // (first, the nearest offered spot beside the break that is clear of everything drawn and within 40 px of the break's
        // marks: the shared placer also wants a label nearer its own mark than any curve, which beside a hole, where the
        // curve runs through the mark, refuses the open side; then the placer, for a leader)
        // (and is read as its own break's: it crosses no other break's column within the height of that break's marks
        // (an infinite break's column is the plot's whole height), it is nearer its own marks than any other point or
        // circle on the plot by 6 px, and it keeps 4 px from tick numbers and curves)
        let own_i = owner(r);
        let clear_with = |pad: f64| -> Option<Pt> { if *kind != 'k' { return None; } r.tries.iter().copied().find(|c| {
            let bx = label_box_for(*c, &r.text, size);
            let Some(i) = own_i else { return false };
            let own = mark_d(&disc_marks[i].1, bx);
            if own > 40.0 || !board.free(grow(bx, pad.max(if size >= 20.0 { 0.15 * size } else { 1.5 }))) { return false; }
            // (centred nearer its own break's x than any other break's, by 4 px: a label is read by the column it is in)
            let own_x = disc_marks[i].1.first().map_or(c.0, |m| m.0 .0);
            let off_centre = disc_marks.iter().enumerate().filter(|(j, _)| *j != i).any(|(_, (_, ms))| ms.first().is_some_and(|m| (c.0 - m.0 .0).abs() < (c.0 - own_x).abs() + 4.0));
            let hugs = own <= 12.0 && disc_marks.iter().enumerate().filter(|(j, _)| *j != i).all(|(_, (_, ms))| mark_d(ms, bx) >= 3.0 * own.max(4.0));
            if off_centre && !hugs { return false; }
            let crosses = disc_marks.iter().enumerate().filter(|(j, _)| *j != i).any(|(_, (_, ms))| {
                let Some(cx) = ms.first().map(|m| m.0 .0) else { return false };
                if cx < bx[0] - 4.0 || cx > bx[2] + 4.0 { return false; }
                let (lo, hi) = ms.iter().flat_map(|(a, c)| [a.1, c.1]).fold((f64::MAX, f64::MIN), |(l, h), y| (l.min(y), h.max(y)));
                bx[3] >= lo - 8.0 && bx[1] <= hi + 8.0
            });
            if crosses { return false; }
            let own_pts: Vec<Pt> = disc_marks[i].1.iter().flat_map(|(a, c)| [*a, *c]).collect();
            board.dots.iter().filter(|(q, _)| !own_pts.iter().any(|o| (o.0 - q.0).abs() < 1.0 && (o.1 - q.1).abs() < 1.0)).all(|(q, _)| box_pt(bx, *q) >= own + 6.0)
        }) };
        // (4 px clear of tick numbers and curves where it can be; else as close as the placer itself allows)
        let clear = clear_with(4.0).or_else(|| clear_with(0.0));
        if let Some(c) = clear {
            board.boxes.push(grow(label_box_for(c, &r.text, size), 1.0));
            labels_svg.push_str(&text_el(c.0 - tw(&r.text, size) / 2.0 + 0.5, c.1 + 0.32 * size, size, &r.text, ink.text, true));
            continue;
        }
        let mut blocked: Vec<Bx> = vec![];
        let spot = loop {
            let (nb, ns) = (board.boxes.len(), board.segs.len());
            let sp = place(&mut board, r, size);
            if acceptable(*kind, r, &sp) || blocked.len() >= 200 { break if acceptable(*kind, r, &sp) { sp } else { board.boxes.truncate(nb); board.segs.truncate(ns); Spot::Off }; }
            board.boxes.truncate(nb);
            board.segs.truncate(ns);
            let c = match &sp { Spot::Beside(c) | Spot::Leader(c, _, _) => *c, Spot::Off => unreachable!() };
            let b = label_box_for(c, &r.text, size);
            blocked.push(b);
            board.boxes.push(b);
        };
        // (the refused spots are no obstacle to the next label)
        for b in &blocked { if let Some(k) = board.boxes.iter().position(|o| o == b) { board.boxes.remove(k); } }
        match spot {
            Spot::Beside(c) => labels_svg.push_str(&text_el(c.0 - tw(&r.text, size) / 2.0 + 0.5, c.1 + 0.32 * size, size, &r.text, ink.text, true)),
            Spot::Leader(c, s, e) => {
                // (on a white casing, so a leader along a gridline still reads as a leader)
                labels_svg.push_str(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#fff" stroke-width="4"/>"##, s.0, s.1, e.0, e.1));
                labels_svg.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.2" data-lim="leader"/>"#, s.0, s.1, e.0, e.1, ink.text));
                labels_svg.push_str(&text_el(c.0 - tw(&r.text, size) / 2.0 + 0.5, c.1 + 0.32 * size, size, &r.text, ink.text, true));
            }
            Spot::Off => beside.push(if *kind == 'k' { format!("{} ({})", eq_text("x", &discs.iter().find(|d| sx(d.x) == r.anchor.0).map(|d| d.text.clone()).unwrap_or_default()), r.text) } else { format!("the asymptote {}", r.text) }),
        }
    }

    // the questions and the key, under the graph
    let mut brows = block(&lp, &m, &discs, discs_complete, stu, &mut dr, &mut lnotes);
    if !beside.is_empty() { brows.push(vec![Item::Text(format!("Not labelled on the graph (no clear room): {}", beside.join("; ")))]); }
    let pad = 16.0;
    let avail = gw.max(420.0) - 2.0 * pad;
    let (block_svg, block_h, block_w) = lay_rows(&brows, pad, gh + 6.0, avail, size, ink.text);
    let w = gw.max(block_w + 2.0 * pad);
    let h = gh + if brows.is_empty() { 0.0 } else { block_h + 14.0 };

    for n in lnotes { let t = in_grid_words(&n, &rows); if !notes.contains(&t) { notes.push(t); } }

    // the grid's drawing as it is, on a page grown to hold the key
    let body_end = svg.rfind("</svg>").unwrap_or(svg.len());
    let head_end = svg.find('>').map(|i| i + 1).unwrap_or(0);
    let mut out = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.0} {h:.0}"><rect width="100%" height="100%" fill="#fff"/>"##);
    out.push_str(&svg[head_end..body_end]);
    let mut add = String::new();
    add.push_str(&format!(r#"<g data-lim="labels">{labels_svg}</g>"#));
    add.push_str(&format!(r#"<g data-lim="block">{block_svg}</g>"#));
    out.push_str(&if print { grey(&add) } else { add });
    out.push_str("</svg>");
    out
}

/// The grid's classic limit setups (each one a starting point the teacher then edits).
pub fn classic_setups() -> Vec<Example> {
    let s = |v: &str| Value::S(v.to_string());
    let f = Value::F;
    let b = Value::B;
    let win = |x0: f64, x1: f64, y0: f64, y1: f64| vec![("xmin", f(x0)), ("xmax", f(x1)), ("ymin", f(y0)), ("ymax", f(y1)), ("xstep", f(1.0)), ("ystep", f(1.0))];
    let with = |mut v: Vec<(&'static str, Value)>, more: Vec<(&'static str, Value)>| { v.extend(more); v };
    vec![
        Example { name: "Hole with the point moved", params: with(win(-4.0, 6.0, -2.0, 7.0), vec![("equations", s("y = (x^2 - 4)/(x - 2)")), ("points", s("(2, 1)")), ("pt_label", s("None")), ("lim_ask_at", s("2")), ("lim_kinds", b(true)), ("title", s("A hole with the point moved"))]) },
        Example { name: "Jump with one-sided limits", params: with(win(-5.0, 5.0, -3.0, 6.0), vec![("equations", s("y = x + 2, x < 1; y = 4 - 2x, x >= 1")), ("points", s("")), ("lim_ask_at", s("1")), ("lim_kinds", b(true)), ("title", s("A jump discontinuity"))]) },
        Example { name: "Vertical asymptote with limits of \u{b1}\u{221e}", params: with(win(-4.0, 8.0, -7.0, 7.0), vec![("equations", s("y = 2/(x - 3)")), ("points", s("")), ("asymptotes", b(true)), ("lim_ask_at", s("3")), ("lim_kinds", b(true)), ("title", s("A vertical asymptote"))]) },
        Example { name: "Limit at infinity with a horizontal asymptote", params: with(win(-8.0, 8.0, -3.0, 6.0), vec![("equations", s("y = (2x^2 + 1)/(x^2 + 1)")), ("points", s("")), ("lim_inf", s("Both")), ("lim_hasym", b(true)), ("title", s("A limit at infinity"))]) },
    ]
}

/// The limits model the grid's params give (for the tests).
#[cfg(test)]
pub(crate) fn model_for(p0: &Params) -> Model {
    let mut pn = p0.clone();
    pn.normalize(&coordinate_grid_params(), false);
    let rows = f_rows(&pn, &mut vec![]);
    Model::new(&limit_params(&pn, &rows), &grid_layout(&pn))
}
