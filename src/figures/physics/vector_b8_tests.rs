//! Batch 8 vector diagram: a vector's own angle mark, marks from the vertical and a vector's own components. The
//! label checks read the drawn SVG (text boxes by `G::tex`'s own widths, every line, arc and arrowhead) over crowded
//! figures: four vectors, each with its mark and components, the resultant's too, at the bold (Large) size, in
//! black and white (Print), mirrored or not, head to tail with tails shared (vector 1 and the resultant) and turning
//! back on themselves.
use super::vector_b8::{self, sweep, tex_extent, VX};
use crate::figures::{interact, registry, start_params, FigureDef, Params, SHOW_ANSWERS};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "vector-diagram").unwrap() }

fn open(over: &[(&str, &str)]) -> Params {
    let d = def();
    let mut p = start_params(&d);
    for (k, v) in over {
        let pd = (d.params)().into_iter().find(|x| x.key == *k).unwrap_or_else(|| panic!("no param {k}"));
        match pd.default {
            crate::figures::Value::B(_) => p.set_b(k, *v == "true"),
            crate::figures::Value::I(_) => p.set_i(k, v.parse().unwrap()),
            crate::figures::Value::F(_) => p.set_f(k, v.parse().unwrap()),
            crate::figures::Value::S(_) => p.set_s(k, v),
        }
    }
    p.normalize(&(d.params)(), false);
    p
}

// ------------------------------------------------------------------ reading the drawn SVG

#[derive(Debug, Clone)]
struct Text { content: String, x0: f64, x1: f64, y0: f64, y1: f64, fill: String }
#[derive(Debug, Clone)]
struct Line { a: (f64, f64), b: (f64, f64), sw: f64, dashed: bool, stroke: String }

fn attr(tag: &str, name: &str) -> Option<String> {
    let k = format!(" {name}=\"");
    let i = tag.find(&k)? + k.len();
    Some(tag[i..i + tag[i..].find('"')?].to_string())
}
fn num(tag: &str, name: &str) -> f64 { attr(tag, name).and_then(|v| v.parse().ok()).unwrap_or(0.0) }
fn unesc(s: &str) -> String { s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&") }

/// Text boxes as drawn (`G::tex`: base characters 0.55 em, sub/superscripts 0.62 of their own size; ascent 0.74 em,
/// descent 0.24 em, a subscript reaching 0.46 em below the baseline), lines, arcs and arrowheads.
fn read(svg: &str) -> (Vec<Text>, Vec<Line>, Vec<[(f64, f64); 3]>) {
    let (mut texts, mut lines, mut tris) = (vec![], vec![], vec![]);
    let mut rest = svg;
    while let Some(i) = rest.find('<') {
        rest = &rest[i..];
        let end = rest.find('>').unwrap();
        let tag = &rest[..=end];
        if tag.starts_with("<text ") {
            let close = rest.find("</text>").unwrap();
            let body = &rest[end + 1..close];
            let (x, y, fs) = (num(tag, "x"), num(tag, "y"), num(tag, "font-size"));
            let anchor = attr(tag, "text-anchor").unwrap_or_default();
            let (mut w, mut content, mut sub, mut sup, mut lvl) = (0.0, String::new(), false, false, 0.0f64);
            let mut b = body;
            while let Some(j) = b.find("<tspan") {
                let te = b[j..].find('>').unwrap() + j;
                let tt = &b[j..=te];
                let ce = b[te..].find("</tspan>").unwrap() + te;
                let c = unesc(&b[te + 1..ce]);
                let tfs = num(tt, "font-size");
                lvl -= num(tt, "dy");
                let small = (tfs - fs).abs() > 0.01;
                if small && lvl < 0.0 { sub = true; }
                if small && lvl > 0.0 { sup = true; }
                w += c.chars().count() as f64 * tfs * if small { 0.62 } else { 0.55 };
                content.push_str(&c);
                b = &b[ce + 8..];
            }
            if !body.contains("<tspan") {
                content = unesc(body);
                w = content.chars().count() as f64 * fs * 0.55;
            }
            let (x0, x1) = match anchor.as_str() { "start" => (x, x + w), "end" => (x - w, x), _ => (x - w / 2.0, x + w / 2.0) };
            let top = if sup { 0.92 } else { 0.74 } * fs;
            let bot = if sub { 0.46 } else { 0.24 } * fs;
            texts.push(Text { content, x0, x1, y0: y - top, y1: y + bot, fill: attr(tag, "fill").unwrap_or_default() });
            rest = &rest[close + 7..];
            continue;
        }
        let stroke = attr(tag, "stroke").unwrap_or_default();
        let sw = num(tag, "stroke-width");
        let dashed = tag.contains("stroke-dasharray");
        if tag.starts_with("<line ") {
            lines.push(Line { a: (num(tag, "x1"), num(tag, "y1")), b: (num(tag, "x2"), num(tag, "y2")), sw, dashed, stroke });
        } else if tag.starts_with("<polyline ") || tag.starts_with("<polygon ") {
            let pts: Vec<(f64, f64)> = attr(tag, "points").unwrap().split_whitespace().map(|q| {
                let (a, b) = q.split_once(',').unwrap();
                (a.parse().unwrap(), b.parse().unwrap())
            }).collect();
            if tag.starts_with("<polygon ") && pts.len() == 3 {
                tris.push([pts[0], pts[1], pts[2]]);
            } else {
                for w in pts.windows(2) {
                    lines.push(Line { a: w[0], b: w[1], sw, dashed, stroke: format!("poly:{stroke}") });
                }
            }
        }
        rest = &rest[end + 1..];
    }
    (texts, lines, tris)
}

fn seg_box(a: (f64, f64), b: (f64, f64), t: &Text, m: f64) -> bool {
    let bx = vector_b8::Bx { c: ((t.x0 + t.x1) / 2.0, (t.y0 + t.y1) / 2.0), hw: (t.x1 - t.x0) / 2.0, hh: (t.y1 - t.y0) / 2.0 };
    // a segment entering the box by more than `m`... reuse the placer's clip with a negative margin
    let (x0, x1, y0, y1) = (bx.c.0 - bx.hw + m, bx.c.0 + bx.hw - m, bx.c.1 - bx.hh + m, bx.c.1 + bx.hh - m);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (pp, q) in [(-dx, a.0 - x0), (dx, x1 - a.0), (-dy, a.1 - y0), (dy, y1 - a.1)] {
        if pp.abs() < 1e-12 {
            if q < 0.0 { return false; }
        } else {
            let r = q / pp;
            if pp < 0.0 { t0 = t0.max(r); } else { t1 = t1.min(r); }
            if t0 > t1 { return false; }
        }
    }
    true
}

fn crosses(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let o = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}

/// The start of the leader line ending at text `t` (the one ending nearest its box, within 6 px), if any.
fn leader_to(lines: &[Line], t: &Text) -> Option<(f64, f64)> {
    let gap = |q: (f64, f64)| (t.x0 - q.0).max(q.0 - t.x1).max(0.0).hypot((t.y0 - q.1).max(q.1 - t.y1).max(0.0));
    // a leader points at the centre of its own text
    let c = centre(t);
    let aims = |l: &Line| {
        let (dx, dy) = (l.b.0 - l.a.0, l.b.1 - l.a.1);
        let len = dx.hypot(dy).max(1e-9);
        ((c.0 - l.a.0) * dy - (c.1 - l.a.1) * dx).abs() / len < 1.0
    };
    lines.iter().filter(|l| (l.sw - 0.9).abs() < 1e-9 && gap(l.b) < 6.0 && aims(l)).min_by(|a, b| gap(a.b).total_cmp(&gap(b.b))).map(|l| l.a)
}

fn in_tri(q: (f64, f64), t: &[(f64, f64); 3]) -> bool {
    let s = |a: (f64, f64), b: (f64, f64)| (b.0 - a.0) * (q.1 - a.1) - (b.1 - a.1) * (q.0 - a.0);
    let (d1, d2, d3) = (s(t[0], t[1]), s(t[1], t[2]), s(t[2], t[0]));
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

/// Blank answer lines (26 px, 1.5 wide, level) count as text.
fn is_blank(l: &Line) -> bool { !l.dashed && (l.sw - 1.5).abs() < 1e-9 && (l.a.1 - l.b.1).abs() < 1e-9 && ((l.b.0 - l.a.0).abs() - 26.0).abs() < 1e-6 }
fn is_grid(l: &Line) -> bool { l.stroke == "#cdd1d8" }

/// Every rule a checker applies to the labels of a drawn figure: no text on text, on a line (not the grid, which the
/// labels' white halo sits over), on an arc or an arrowhead, and nothing off the page (480 x 360, or the larger page
/// the drawing grew to).
fn label_faults(svg: &str) -> Vec<String> {
    let (mut texts, lines, tris) = read(svg);
    for l in lines.iter().filter(|l| is_blank(l)) {
        texts.push(Text { content: "__".into(), x0: l.a.0.min(l.b.0), x1: l.a.0.max(l.b.0), y0: l.a.1 - 1.5, y1: l.a.1 + 1.5, fill: String::new() });
    }
    let (pw, ph) = crate::figures::svg_size(svg);
    let mut out = vec![];
    // leaders (0.9 px): each from a mark to the edge of one text, through no other text, crossing no other leader,
    // arrow, arc, axis or arrowhead
    let leaders: Vec<&Line> = lines.iter().filter(|l| (l.sw - 0.9).abs() < 1e-9).collect();
    let near_box = |q: (f64, f64), t: &Text| q.0 > t.x0 - 7.0 && q.0 < t.x1 + 7.0 && q.1 > t.y0 - 7.0 && q.1 < t.y1 + 7.0;
    for (k, l) in leaders.iter().enumerate() {
        if texts.iter().filter(|t| near_box(l.b, t)).count() == 0 {
            out.push(format!("a leader ending at no text: {l:?}"));
        }
        for m in &leaders[k + 1..] {
            if crosses(l.a, l.b, m.a, m.b) {
                out.push("leaders cross".into());
            }
        }
        for o in lines.iter().filter(|o| !o.dashed && !is_grid(o) && !is_blank(o) && (o.sw - 0.9).abs() > 1e-9) {
            // its own arrow or arc where it starts
            if dist_seg(l.a, o.a, o.b) < 1.5 {
                continue;
            }
            if crosses(l.a, l.b, o.a, o.b) {
                out.push(format!("a leader crosses a line {o:?}"));
            }
        }
        for tr in &tris {
            if (0..3).any(|j| crosses(l.a, l.b, tr[j], tr[(j + 1) % 3])) {
                out.push("a leader crosses an arrowhead".into());
            }
        }
    }
    for (i, t) in texts.iter().enumerate() {
        if t.x0 < 0.0 || t.x1 > pw || t.y0 < 0.0 || t.y1 > ph {
            out.push(format!("{:?} off the page", t.content));
        }
        for u in &texts[i + 1..] {
            if t.x0 < u.x1 - 0.5 && u.x0 < t.x1 - 0.5 && t.y0 < u.y1 - 0.5 && u.y0 < t.y1 - 0.5 {
                out.push(format!("{:?} on {:?}", t.content, u.content));
            }
        }
        if t.content == "__" {
            continue;
        }
        for l in lines.iter().filter(|l| !is_grid(l) && !is_blank(l) && !((l.sw - 0.9).abs() < 1e-9 && near_box(l.b, t))) {
            if seg_box(l.a, l.b, t, -(l.sw / 2.0) + 0.25) {
                out.push(format!("{:?} on a line {:?}", t.content, l));
            }
        }
        for tr in &tris {
            let c = ((t.x0 + t.x1) / 2.0, (t.y0 + t.y1) / 2.0);
            if (0..3).any(|k| seg_box(tr[k], tr[(k + 1) % 3], t, 0.25)) || in_tri(c, tr) {
                out.push(format!("{:?} on an arrowhead", t.content));
            }
        }
    }
    out
}

fn dist_seg(q: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 { vector_b8::seg_dist(q, a, b) }

/// The drawn arcs (1.6 px polylines), each as its list of points.
fn arcs(svg: &str) -> Vec<Vec<(f64, f64)>> {
    let mut out = vec![];
    for tag in svg.split('<').filter(|t| t.starts_with("polyline ") && t.contains("stroke-width=\"1.6\"")) {
        let pts = attr(&format!(" {tag}"), "points").unwrap();
        out.push(pts.split_whitespace().map(|q| { let (a, b) = q.split_once(',').unwrap(); (a.parse().unwrap(), b.parse().unwrap()) }).collect());
    }
    out
}

fn centre(t: &Text) -> (f64, f64) { ((t.x0 + t.x1) / 2.0, (t.y0 + t.y1) / 2.0) }

/// Seeded crowded figures: four vectors, each marked and with components, the resultant's mark and components, bold.
fn crowded(seed: u64, k: usize) -> Vec<Params> {
    let mut s = seed;
    let mut next = |n: u64| { s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (s >> 33) % n };
    let mut out = vec![];
    for c in 0..k {
        let mut over: Vec<(String, String)> = vec![("n".into(), "4".into()), ("mark_angle".into(), "true".into()), ("components".into(), "true".into())];
        for i in 1..=4 {
            // at an angle, never along the grid
            let mut a = next(360) as f64;
            if a % 90.0 == 0.0 { a += 17.0; }
            over.push((format!("v{i}_ang"), a.to_string()));
            over.push((format!("v{i}_mag"), (1.0 + next(23) as f64 * 0.5).to_string()));
            over.push((format!("v{i}_mark"), "true".into()));
            over.push((format!("v{i}_comps"), "true".into()));
            over.push((format!("v{i}_from"), ["Horizontal", "Vertical"][next(2) as usize].into()));
            over.push((format!("v{i}_alab_mode"), ["Text", "Degrees", "Text", "Blank line"][next(4) as usize].into()));
        }
        over.push(("ang_from".into(), ["Horizontal", "Vertical"][next(2) as usize].into()));
        over.push(("ang_lab_mode".into(), ["Text", "Degrees"][next(2) as usize].into()));
        over.push(("mirror".into(), (c % 2 == 1).to_string()));
        over.push(("axes".into(), (next(3) == 0).to_string()));
        let o: Vec<(&str, &str)> = over.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        out.push(open(&o));
    }
    out
}

/// Hand-picked crowded figures: tails shared, arrows turning back, the longest labels, the biggest vectors.
fn hard() -> Vec<Params> {
    let all = |angs: [&str; 4], mags: [&str; 4], extra: &[(&str, &str)]| {
        let mut o: Vec<(&str, &str)> = vec![("n", "4"), ("mark_angle", "true"), ("components", "true")];
        let keys = [("v1_ang", "v1_mag"), ("v2_ang", "v2_mag"), ("v3_ang", "v3_mag"), ("v4_ang", "v4_mag")];
        for (i, (ka, km)) in keys.iter().enumerate() {
            o.push((ka, angs[i]));
            o.push((km, mags[i]));
        }
        for x in VX {
            o.push((x[0], "true"));
            o.push((x[4], "true"));
        }
        o.extend_from_slice(extra);
        open(&o)
    };
    vec![
        all(["30", "120", "210", "300"], ["4", "4", "4", "4"], &[]),
        all(["30", "120", "210", "300"], ["4", "4", "4", "4"], &[("v1_from", "Vertical"), ("v2_from", "Vertical"), ("v3_from", "Vertical"), ("v4_from", "Vertical"), ("ang_from", "Vertical")]),
        // folding back over itself, and all four the same way
        all(["20", "200", "25", "205"], ["5", "4", "3", "2"], &[]),
        all(["45", "45", "45", "45"], ["3", "3", "3", "3"], &[]),
        all(["10", "80", "100", "170"], ["12", "12", "12", "12"], &[("v1_alab_mode", "Degrees"), ("v2_alab_mode", "Degrees"), ("v3_alab_mode", "Degrees"), ("v4_alab_mode", "Degrees"), ("ang_lab_mode", "Degrees")]),
        all(["1", "89", "181", "269"], ["0.5", "0.5", "0.5", "0.5"], &[]),
        all(["37", "143", "323", "217"], ["6", "6", "6", "6"], &[("v1_lab", "F_{app}"), ("v2_lab", "Displacement"), ("v3_lab", "v_{net}"), ("v4_lab", "\\vec{F}"), ("res_lab", "R_{total}"), ("axes", "true")]),
        all(["60", "150", "240", "330"], ["8", "2", "8", "2"], &[("mirror", "true"), ("bold", "false")]),
    ]
}

#[test]
fn crowded_labels_clear_of_everything() {
    let d = def();
    let mut cases = hard();
    cases.extend(crowded(0x5eed_b8, 80));
    let mut bad = vec![];
    let (mut left_off, mut four, mut four_off) = (0, 0, 0);
    for (k, p) in cases.iter().enumerate() {
        for (color, answers) in [(false, true), (false, false), (true, true)] {
            let mut q = p.clone();
            q.set_b("color", color);
            q.set_b(SHOW_ANSWERS, answers);
            let svg = (d.render)(&q);
            let mut f = label_faults(&svg);
            // a label left off for lack of room is said so: in the panel always, in the figure where the note fits
            let sc = vector_b8::scene(&q);
            if q.i("n") == 4 && answers && !color {
                four += 1;
                four_off += (!sc.misses.is_empty()) as usize;
            }
            if !sc.misses.is_empty() || !sc.dropped.is_empty() {
                left_off += 1;
                if crate::figures::notes_for("vector-diagram", &q).is_empty() {
                    f.push("labels left off without a panel note".into());
                }
                if svg.contains("left off") || svg.contains("room") {
                    f.push("a note drawn in the figure".into());
                }
            }
            if !f.is_empty() {
                bad.push(format!("case {k} color={color} answers={answers}: {}", f.join("; ")));
            }
        }
    }
    assert!(bad.is_empty(), "{} faults:\n{}", bad.len(), bad.join("\n"));
    eprintln!("crowded: {left_off} of {} drawings left a label or component off; four vectors with everything on: a label left off in {four_off} of {four}", cases.len() * 3);
    // a ceiling at this commit's count (about half, as before: the goal is well under)
    assert!(four_off * 100 <= four * 50, "four-vector figures leaving a label off: {four_off} of {four}");
}

/// Each mark's label is nearest its own arc, and that arc is about its own vector's tail, reaching that vector.
#[test]
fn each_mark_and_component_belongs_to_its_own_vector() {
    let d = def();
    let mut cases = hard();
    cases.extend(crowded(0x0b1e_c7, 60));
    let mut bad = vec![];
    for (k, p) in cases.into_iter().enumerate() {
        let mut q = p.clone();
        for (i, x) in VX.iter().enumerate() {
            q.set_s(x[2], "Text");
            q.set_s(x[3], ["alpha", "beta", "gamma", "phi"][i]);
            q.set_s(&format!("v{}_lab", i + 1), ["A", "B", "C", "D"][i]);
            q.set_s(&format!("v{}_lab_mode", i + 1), "Text");
        }
        q.set_s("ang_lab_mode", "Text");
        q.set_s("ang_lab", "theta");
        q.set_s("res_lab", "R");
        let svg = (d.render)(&q);
        let (texts, lines, _) = read(&svg);
        let drawn_arcs = arcs(&svg);
        let left_off = vector_b8::scene(&q).misses.clone();
        let l = vector_b8::scene(&q).lay.clone();
        let pw = page(&svg).0;
        let mir = |x: f64| if q.b("mirror") { pw - x } else { x };
        let px = |g: (f64, f64)| (mir(l.ox + g.0 * l.u), l.oy - g.1 * l.u);
        let near_arc = |c: (f64, f64)| drawn_arcs.iter().min_by(|a, b| {
            let da = a.windows(2).map(|w| dist_seg(c, w[0], w[1])).fold(f64::MAX, f64::min);
            let db = b.windows(2).map(|w| dist_seg(c, w[0], w[1])).fold(f64::MAX, f64::min);
            da.total_cmp(&db)
        }).cloned();
        for (i, greek) in ["\u{3b1}", "\u{3b2}", "\u{3b3}", "\u{3c6}", "\u{3b8}"].iter().enumerate() {
            let Some(t) = texts.iter().find(|t| t.content == *greek) else {
                if i < 4 && !left_off.iter().any(|m| m == ["alpha", "beta", "gamma", "phi"][i]) { bad.push(format!("case {k}: no label {greek}")); }
                continue;
            };
            // a leader line ending at the label: the arc it starts on is the one named
            let c = leader_to(&lines, t).unwrap_or(centre(t));
            let arc = near_arc(c).unwrap();
            let (tail, tip) = if i < 4 { (px(l.pts[i]), px(l.pts[i + 1])) } else { (px((0.0, 0.0)), px(l.res)) };
            // inside its angle, read off the drawing: on the arc's side of the lines through the tail along the arc's two
            // ends (its arrow and its reference line), unless a leader joins it to its arc
            let led = c != centre(t);
            let side = |e: (f64, f64), q: (f64, f64)| (q.0 - tail.0) * (e.1 - tail.1) - (q.1 - tail.1) * (e.0 - tail.0);
            let (e0, e1, mid) = (arc[0], *arc.last().unwrap(), arc[arc.len() / 2]);
            // which end is the arrow's: the one on the arrow's line
            let on_arrow = |e: (f64, f64)| dist_seg(e, tail, (tail.0 + (tip.0 - tail.0) * 50.0, tail.1 + (tip.1 - tail.1) * 50.0)) < 0.6;
            let (ea, er) = if on_arrow(e0) { (e0, e1) } else { (e1, e0) };
            let span = {
                let a0 = (tail.1 - ea.1).atan2(ea.0 - tail.0);
                let a1 = (tail.1 - er.1).atan2(er.0 - tail.0);
                let mut d = (a0 - a1).to_degrees().abs();
                if d > 180.0 { d = 360.0 - d; }
                d
            };
            // inside its own angle's sector widened by 10 degrees each side (the checker's rule)
            let _ = (side, mid, led);
            let ang = |q: (f64, f64)| (tail.1 - q.1).atan2(q.0 - tail.0).to_degrees();
            let (a_ref, a_arr) = (ang(er), ang(ea));
            let mut sweep_ = (a_arr - a_ref).rem_euclid(360.0);
            let dir = if sweep_ > 180.0 { sweep_ = 360.0 - sweep_; -1.0 } else { 1.0 };
            let mut tt = (ang(centre(t)) - a_ref).rem_euclid(360.0);
            if dir < 0.0 { tt = (360.0 - tt).rem_euclid(360.0); }
            if tt > 180.0 { tt -= 360.0; }
            if span >= 2.0 && (tt < -10.5 || tt > sweep_ + 10.5) {
                bad.push(format!("case {k}: {greek} is outside its angle"));
            }
            let r0 = dist_seg(tail, arc[0], arc[0]);
            if arc.iter().any(|&a| (dist_seg(tail, a, a) - r0).abs() > 0.3) {
                bad.push(format!("case {k}: {greek}'s nearest arc is not about its own tail"));
                continue;
            }
            // one end of the arc on the arrow's own line
            let on = |a: (f64, f64)| dist_seg(a, tail, (tail.0 + (tip.0 - tail.0) * 50.0, tail.1 + (tip.1 - tail.1) * 50.0)) < 0.6;
            if !on(arc[0]) && !on(*arc.last().unwrap()) {
                bad.push(format!("case {k}: {greek}'s nearest arc does not reach its own arrow"));
            }
        }
        // components: the label's nearest dashed component (2.2 px) is its own, along x or y between tail and tip
        let comp_lines: Vec<&Line> = lines.iter().filter(|l| l.dashed && (l.sw - 2.2).abs() < 1e-9).collect();
        for (i, name) in ["A", "B", "C", "D", "R"].iter().enumerate() {
            let (tail, tip) = if i < 4 { (px(l.pts[i]), px(l.pts[i + 1])) } else { (px((0.0, 0.0)), px(l.res)) };
            for axis in ["x", "y"] {
                let Some(t) = texts.iter().find(|t| t.content == format!("{name}{axis}")) else { continue };
                let c = centre(t);
                // a leader line ending at the label: the component it starts on is the one named
                let c = leader_to(&lines, t).unwrap_or(c);
                let (lo_x, hi_x) = (tail.0.min(tip.0) - 0.6, tail.0.max(tip.0) + 0.6);
                let (lo_y, hi_y) = (tail.1.min(tip.1) - 0.6, tail.1.max(tip.1) + 0.6);
                let inside = |l: &&&Line| { let i = |q: (f64, f64)| q.0 >= lo_x && q.0 <= hi_x && q.1 >= lo_y && q.1 <= hi_y; i(l.a) && i(l.b) };
                let level = |l: &&&Line| (l.a.1 - l.b.1).abs() < 0.01;
                // the nearest component is one of its own arrow's (between its tail and tip), and its own (level for x,
                // upright for y) is close by
                let lnear = comp_lines.iter().min_by(|a, b| dist_seg(c, a.a, a.b).total_cmp(&dist_seg(c, b.a, b.b))).unwrap();
                let own = comp_lines.iter().filter(|l| inside(l) && level(l) == (axis == "x")).map(|l| dist_seg(c, l.a, l.b)).fold(f64::MAX, f64::min);
                if !inside(&lnear) || own > (t.x1 - t.x0).max(t.y1 - t.y0) / 2.0 + 72.0 {
                    bad.push(format!("case {k}: {name}{axis}'s nearest component is not its own: {lnear:?}, own at {own}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The angle a "Degrees" label states is the arc as drawn, from the reference line chosen.
#[test]
fn degrees_label_reads_the_drawn_arc() {
    let d = def();
    for (ang, from, want) in [(30.0, "Horizontal", "30\u{b0}"), (30.0, "Vertical", "60\u{b0}"), (150.0, "Horizontal", "30\u{b0}"), (200.0, "Horizontal", "20\u{b0}"), (300.0, "Vertical", "30\u{b0}"), (225.0, "Vertical", "45\u{b0}"), (100.0, "Vertical", "10\u{b0}"), (359.0, "Horizontal", "1\u{b0}"), (37.5, "Horizontal", "37.5\u{b0}")] {
        let p = open(&[("n", "2"), ("v2_ang", &ang.to_string()), ("v2_mark", "true"), ("v2_from", from), ("v2_alab_mode", "Degrees")]);
        let svg = (d.render)(&p);
        let (texts, _, _) = read(&svg);
        let t = texts.iter().find(|t| t.content.ends_with('\u{b0}')).unwrap_or_else(|| panic!("{ang} {from}: no degrees label"));
        assert_eq!(t.content, want, "{ang} from the {from}");
        // the drawn arc spans the stated angle about vector 2's tail
        let arc = arcs(&svg).into_iter().next().unwrap();
        let l = vector_b8::scene(&p).lay.clone();
        let tail = (l.ox + l.pts[1].0 * l.u, l.oy - l.pts[1].1 * l.u);
        let a0 = (tail.1 - arc[0].1).atan2(arc[0].0 - tail.0).to_degrees();
        let a1 = (tail.1 - arc.last().unwrap().1).atan2(arc.last().unwrap().0 - tail.0).to_degrees();
        let mut span = (a1 - a0).abs();
        if span > 180.0 { span = 360.0 - span; }
        assert!((vector_b8::degrees(span) == want), "{ang} {from}: arc spans {span}, label {want}");
        // the arc starts on the horizontal or the vertical through the tail
        let r = (a0.rem_euclid(90.0)).min(90.0 - a0.rem_euclid(90.0));
        assert!(r < 0.01, "{ang} {from}: arc starts at {a0}");
        let vertical = (a0.rem_euclid(180.0) - 90.0).abs() < 0.01;
        assert_eq!(vertical, from == "Vertical", "{ang}: measured from {from}, starts at {a0}");
    }
    // the resultant from the vertical: R at 36.9 degrees above the horizontal is 53.1 from the vertical
    let p = open(&[("mark_angle", "true"), ("ang_from", "Vertical"), ("ang_lab_mode", "Degrees")]);
    let (texts, _, _) = read(&(d.render)(&p));
    assert!(texts.iter().any(|t| t.content == "53.1\u{b0}"), "{:?}", texts.iter().map(|t| &t.content).collect::<Vec<_>>());
    // a whole number of degrees without its ".0"
    assert_eq!(vector_b8::degrees(-60.0), "60\u{b0}");
    assert_eq!(vector_b8::degrees(36.87), "36.9\u{b0}");
    let p = open(&[("mark_angle", "true"), ("ang_lab_mode", "Degrees")]);
    let (texts, _, _) = read(&(d.render)(&p));
    assert!(texts.iter().any(|t| t.content == "36.9\u{b0}"));
}

#[test]
fn sweeps_by_hand() {
    // (direction, from) -> (reference line, signed sweep), the short way from the nearer side
    for (a, f, want) in [
        (30.0, "Horizontal", (0.0, 30.0)), (30.0, "Vertical", (90.0, -60.0)), (150.0, "Horizontal", (180.0, -30.0)),
        (150.0, "Vertical", (90.0, 60.0)), (200.0, "Horizontal", (180.0, 20.0)), (300.0, "Vertical", (270.0, 30.0)),
        (225.0, "Vertical", (270.0, -45.0)), (330.0, "Horizontal", (0.0, -30.0)), (360.0, "Horizontal", (0.0, 0.0)),
    ] {
        let (r, d) = sweep(a, f);
        assert!((r - want.0).abs() < 1e-9 && (d - want.1).abs() < 1e-9, "{a} {f}: {r} {d}");
    }
    assert_eq!(vector_b8::comp_name("A", "x"), "A_x");
    assert_eq!(vector_b8::comp_name("F_1", "y"), "F_{1y}");
    assert_eq!(vector_b8::comp_name("v_{net}", "x"), "v_{netx}");
    let (w, top, bot) = tex_extent("A_x", 20.0, super::common::ITALIC);
    assert!((w - (11.0 + 20.0 * 0.7 * 0.62)).abs() < 1e-9 && top < 0.0 && bot > 0.0);
}

/// Defaults draw today's figure; each new control draws something of its own, and every option differs.
#[test]
fn every_new_control_changes_the_drawing() {
    let d = def();
    let base = start_params(&d);
    assert!(!vector_b8::active(&base));
    let r = |o: &[(&str, &str)]| (d.render)(&open(o));
    let opening = r(&[]);
    // the evidence: from the opening state
    for o in [
        vec![("v2_ang", "60"), ("v2_mark", "true")],
        vec![("mark_angle", "true"), ("ang_from", "Vertical")],
        vec![("v2_ang", "60"), ("v2_comps", "true")],
    ] {
        assert_ne!(r(&o), opening, "{o:?}");
    }
    // vertical differs from horizontal, on the resultant and on a vector
    assert_ne!(r(&[("mark_angle", "true"), ("ang_from", "Vertical")]), r(&[("mark_angle", "true")]));
    assert_ne!(r(&[("v2_ang", "60"), ("v2_mark", "true"), ("v2_from", "Vertical")]), r(&[("v2_ang", "60"), ("v2_mark", "true")]));
    // each label mode, and a vector's mark and components each add something of their own
    let modes: Vec<String> = ["Text", "Blank line", "Degrees"].iter().map(|m| r(&[("v2_ang", "60"), ("v2_mark", "true"), ("v2_alab_mode", m)])).collect();
    assert!(modes[0] != modes[1] && modes[1] != modes[2] && modes[0] != modes[2]);
    let mark = r(&[("v2_ang", "60"), ("v2_mark", "true")]);
    let comps = r(&[("v2_ang", "60"), ("v2_comps", "true")]);
    let both = r(&[("v2_ang", "60"), ("v2_mark", "true"), ("v2_comps", "true")]);
    assert!(mark != comps && both != mark && both != comps);
    assert_eq!(arcs(&mark).len(), 1);
    assert!(mark.contains("\u{3b2}"), "vector 2's own letter, beta");
    assert!(comps.contains(">B</tspan><tspan") && comps.contains(">x</tspan>") && comps.contains(">y</tspan>"));
    // settings of a vector not shown change nothing
    assert_eq!(r(&[("v3_mark", "true"), ("v3_comps", "true"), ("v3_ang", "30")]), r(&[("v3_ang", "30")]));
}

#[test]
fn switches_offered_only_where_they_draw_something() {
    let p = open(&[]);
    let rel = |p: &Params, k: &str| crate::figures::param_relevant("vector-diagram", k, p);
    // the opening vectors lie along the grid (0 and 90 degrees): nothing of their own to mark
    assert!(!rel(&p, "v1_mark") && !rel(&p, "v1_comps") && !rel(&p, "v2_mark"));
    let q = open(&[("v2_ang", "60")]);
    assert!(rel(&q, "v2_mark") && rel(&q, "v2_comps") && !rel(&q, "v1_mark"));
    assert!(!rel(&q, "v2_from") && !rel(&q, "v2_alab_mode"));
    let q = open(&[("v2_ang", "60"), ("v2_mark", "true")]);
    assert!(rel(&q, "v2_from") && rel(&q, "v2_alab_mode") && rel(&q, "v2_alab"));
    let q = open(&[("v2_ang", "60"), ("v2_mark", "true"), ("v2_alab_mode", "Degrees")]);
    assert!(!rel(&q, "v2_alab"));
    // a switch already on stays offered (to turn it off) when the vector is turned along the grid
    let q = open(&[("v2_mark", "true")]);
    assert!(rel(&q, "v2_mark"));
    // vectors past the number shown, and a zero vector
    let q = open(&[("v3_ang", "60")]);
    assert!(!rel(&q, "v3_mark"));
    let q = open(&[("v2_ang", "60"), ("v2_mag", "0")]);
    assert!(!rel(&q, "v2_mark"));
    // the resultant's "Measured from" once its angle is marked; other figures untouched
    assert!(!rel(&open(&[]), "ang_from") && rel(&open(&[("mark_angle", "true")]), "ang_from"));
    let fbd = start_params(&registry().into_iter().find(|d| d.id == "free-body-diagram").unwrap());
    assert!(crate::figures::param_relevant("free-body-diagram", "f1_mark", &fbd));
}

/// The student copy hides what it hid (the resultant, its mark and components) and keeps what a teacher gives.
#[test]
fn student_copy_hides_only_the_answers() {
    let d = def();
    let mut p = open(&[("n", "3"), ("v1_ang", "30"), ("v2_ang", "100"), ("v3_ang", "200"), ("mark_angle", "true"), ("components", "true"), ("ang_from", "Vertical"), ("ang_lab_mode", "Degrees")]);
    for x in VX.iter().take(3) {
        p.set_b(x[0], true);
        p.set_b(x[4], true);
    }
    let key = (d.render)(&p);
    let ans = super::vector_b8::scene(&p);
    let res_deg = ans.marks.iter().find(|m| m.arrow == 3).map(|m| super::vector_b8::degrees(m.d)).unwrap();
    assert!(key.contains(&res_deg));
    p.set_b(SHOW_ANSWERS, false);
    let student = (d.render)(&p);
    let full = d.svg(&p);
    for leak in [res_deg.as_str(), ">R</tspan>", ">Rx", "R</tspan><tspan font-size"] {
        assert!(!student.contains(leak), "student copy shows {leak}");
        assert!(!full.contains(leak), "student copy's SVG (with its title and description) shows {leak}");
    }
    // the given vectors keep their marks and components
    assert_eq!(arcs(&student).len(), 3);
    assert!(student.contains("\u{3b1}") && student.contains("\u{3b2}") && student.contains("\u{3b3}"));
    assert!(student.matches("stroke-width=\"2.2\"").count() >= 6);
    // a hidden vector name stays hidden in its components' names
    p.set_s("v1_lab_mode", "Blank line");
    let s = (d.render)(&p);
    assert!(!s.contains(">A</tspan>"), "vector 1's name shows in its components");
}

/// The label handles sit on the labels as drawn in the batch 8 drawing (the workspace's own hit test).
#[test]
fn label_handles_follow_the_drawn_labels() {
    let d = def();
    for p in hard().into_iter().take(4).chain(crowded(77, 10)) {
        let mut q = p.clone();
        for i in 1..=4 {
            q.set_s(&format!("v{i}_lab_mode"), "Text");
            q.set_s(&format!("v{i}_lab"), ["A", "B", "C", "D"][i - 1]);
        }
        let svg = (d.render)(&q);
        let (texts, _, _) = read(&svg);
        let hs = interact::handles_for("vector-diagram", &q, page(&svg));
        for (i, name) in ["A", "B", "C", "D"].iter().enumerate() {
            let Some(t) = texts.iter().find(|t| t.content == *name) else {
                // left off for lack of room (and said so)
                assert!(vector_b8::scene(&q).misses.iter().any(|m| m == name), "label {name} missing");
                continue;
            };
            let hit = interact::pick(&hs, centre(t), 1.0).map(|k| hs[k].edit);
            assert_eq!(hit, Some(Some(["v1_lab", "v2_lab", "v3_lab", "v4_lab"][i])), "label {name}");
        }
    }
}

/// The ends of every range together: no stray coordinates, a bounded drawing, clean numbers.
#[test]
fn extremes_draw_cleanly() {
    let d = def();
    for mag in ["0", "0.5", "12"] {
        for a in ["0", "0.4", "45", "89.6", "359.9", "360"] {
            let mut o: Vec<(&str, &str)> = vec![("n", "4"), ("mark_angle", "true"), ("components", "true"), ("ang_lab_mode", "Degrees"), ("ang_from", "Vertical")];
            for (i, x) in VX.iter().enumerate() {
                o.push(([("v1_mag"), "v2_mag", "v3_mag", "v4_mag"][i], mag));
                o.push((["v1_ang", "v2_ang", "v3_ang", "v4_ang"][i], a));
                o.extend([(x[0], "true"), (x[4], "true"), (x[2], "Degrees"), (x[1], "Vertical")]);
            }
            let svg = (d.render)(&open(&o));
            assert!(!svg.contains("NaN") && !svg.contains("inf"), "{mag} {a}");
            assert!(svg.len() < 60_000, "{mag} {a}: {}", svg.len());
            let (texts, _, _) = read(&svg);
            for t in texts.iter().filter(|t| t.content.ends_with('\u{b0}')) {
                let v: f64 = t.content.trim_end_matches('\u{b0}').parse().unwrap();
                assert!((0.0..=180.0).contains(&v) && vector_b8::degrees(v) == t.content, "{}", t.content);
            }
        }
    }
}

#[test]
fn removing_and_swapping_vectors_carry_their_marks() {
    let d = def();
    let defs = (d.params)();
    let mut p = open(&[("n", "3"), ("v3_ang", "40"), ("v3_mark", "true"), ("v3_from", "Vertical"), ("v3_comps", "true")]);
    crate::figures::items::swap_items(&mut p, &defs, "v", 2, 3);
    assert!(p.b("v2_mark") && p.b("v2_comps") && p.s("v2_from") == "Vertical" && !p.b("v3_mark"));
    assert!(crate::figures::items::remove_item(&mut p, &defs, "v", "n", 1));
    assert!(p.b("v1_mark") && !p.b("v3_mark") && !p.b("v3_comps"));
}


/// Textbook figures, every arrow marked and resolved: nothing is left off, in either copy, mirrored or not, from
/// either reference line, bold or not. (Random crowded figures may leave a label off, with a note: see above.)
#[test]
fn textbook_figures_leave_nothing_off() {
    let d = def();
    let all = |n: usize| -> Vec<(&'static str, &'static str)> {
        let mut o = vec![("mark_angle", "true"), ("components", "true")];
        for x in VX.iter().take(n) {
            o.push((x[0], "true"));
            o.push((x[4], "true"));
        }
        o
    };
    let figs: Vec<Vec<(&str, &str)>> = vec![
        [vec![("n", "1"), ("v1_mag", "5"), ("v1_ang", "37"), ("axes", "true")], all(1)].concat(),
        [vec![("n", "2"), ("v1_ang", "30"), ("v2_ang", "120")], all(2)].concat(),
        [vec![("n", "2"), ("v1_mag", "6"), ("v1_ang", "20"), ("v2_mag", "4"), ("v2_ang", "75")], all(2)].concat(),
        [vec![("n", "3"), ("v1_mag", "5"), ("v1_ang", "30"), ("v2_mag", "3.5"), ("v2_ang", "110"), ("v3_mag", "3"), ("v3_ang", "200")], all(3)].concat(),
        [vec![("n", "4"), ("v1_ang", "30"), ("v2_mag", "4"), ("v2_ang", "120"), ("v3_mag", "4"), ("v3_ang", "210"), ("v4_mag", "3"), ("v4_ang", "300")], all(4)].concat(),
    ];
    let mut bad = vec![];
    for f in &figs {
        for (from, mirror, bold, answers) in [("Horizontal", "false", "true", "true"), ("Vertical", "true", "true", "true"), ("Horizontal", "true", "false", "true"), ("Vertical", "false", "true", "false")] {
            let mut o = f.clone();
            o.extend([("ang_from", from), ("v1_from", from), ("v2_from", from), ("v3_from", from), ("v4_from", from), ("mirror", mirror), ("bold", bold), ("show_answers", answers)]);
            let p = open(&o);
            let sc = vector_b8::scene(&p);
            let svg = (d.render)(&p);
            if !sc.misses.is_empty() || !sc.dropped.is_empty() || !label_faults(&svg).is_empty() {
                bad.push(format!("{:?}: {:?} {:?} {:?} {:?} marks {:?}", &o[..8], sc.misses, sc.why, sc.dropped, label_faults(&svg), sc.marks));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}


// ------------------------------------------------------------------ batch 8 round 2: the realistic worksheet family

/// Figures of the realistic family still leaving an angle value / a component name off at this commit (of 9,267).
const ANG_OFF: usize = 33;
const COMP_OFF: usize = 25;

const FAM_ANGS: [f64; 11] = [0.0, 30.0, 45.0, 60.0, 90.0, 120.0, 135.0, 180.0, 210.0, 270.0, 300.0];
const FAM_LENS: [f64; 3] = [2.0, 4.0, 6.0];

/// The checker's realistic worksheet figures: every pair of those angles and lengths 2, 4, 6 for two vectors, and a
/// seeded 2,000 three-vector figures (lengths 2 to 6), each with one mark (vector 2's, in degrees), with every mark in
/// degrees (the resultant's too), and with every mark and component; at Medium (thin) and Large (bold).
fn family() -> Vec<(String, Params)> {
    // the checker's sweep, exactly: every ordered pair of the angles with lengths 2, 4, 6 for two vectors (in its
    // order), its 1,500 seeded three-vector sets (tests/fixtures/vector_b8_sweep3.txt, read from its result list),
    // each with vector 2 marked in degrees ("one"), every vector marked in degrees ("allmarks"), every mark and every
    // component on, the resultant's too ("all"); bold off (Medium) and on (Large)
    let mut sets: Vec<Vec<(f64, f64)>> = vec![];
    for &a1 in &FAM_ANGS { for &l1 in &FAM_LENS { for &a2 in &FAM_ANGS { for &l2 in &FAM_LENS { sets.push(vec![(l1, a1), (l2, a2)]); } } } }
    sets.sort_by(|a, b| (a[0].1, a[0].0, a[1].1, a[1].0).partial_cmp(&(b[0].1, b[0].0, b[1].1, b[1].0)).unwrap());
    let fix = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vector_b8_sweep3.txt")).unwrap();
    for line in fix.lines() {
        let v: Vec<f64> = line.split_whitespace().map(|x| x.parse().unwrap()).collect();
        sets.push((0..3).map(|i| (v[2 * i + 1], v[2 * i])).collect());
    }
    let mut out = vec![];
    for bold in [false, true] {
        for set in &sets {
            for (variant, setting) in ["one", "allmarks", "all"].iter().enumerate() {
                let mut p = open(&[]);
                p.set_i("n", set.len() as i64);
                p.set_b("bold", bold);
                for (i, &(m, a)) in set.iter().enumerate() {
                    p.set_f(&format!("v{}_mag", i + 1), m);
                    p.set_f(&format!("v{}_ang", i + 1), a);
                }
                match variant {
                    0 => { p.set_b("v2_mark", true); p.set_s("v2_alab_mode", "Degrees"); }
                    _ => {
                        for x in VX.iter().take(set.len()) { p.set_b(x[0], true); p.set_s(x[2], "Degrees"); if variant == 2 { p.set_b(x[4], true); } }
                        if variant == 2 { p.set_b("components", true); p.set_b("mark_angle", true); p.set_s("ang_lab_mode", "Degrees"); }
                    }
                }
                out.push((format!("{set:?} {setting} variant {variant} bold {bold}"), p));
            }
        }
    }
    out
}

/// The drawn sweep's problems for one figure, by the checker's rules: a vector's name not drawn; an arc with no value;
/// a component with no name; a value outside its own angle's sector widened by 10 degrees, further from its arc's
/// centre than the radius and 1.5 text heights, or not plainly nearer its own arc (the one whose span it states) than
/// any other. Returns (problems, leaders drawn).
fn sweep_problems(p: &Params, svg: &str) -> (Vec<String>, usize) {
    let (texts, lines, _) = read(svg);
    let mut out = vec![];
    let n = p.i("n") as usize;
    for i in 0..n {
        let name = ["A", "B", "C", "D"][i];
        if !texts.iter().any(|t| t.content == name) { out.push(format!("NAME {name}")); }
    }
    let drawn_arcs = arcs(svg);
    let degs: Vec<&Text> = texts.iter().filter(|t| t.content.ends_with('\u{b0}')).collect();
    if degs.len() < drawn_arcs.len() { out.push(format!("MISSING_DEG {}", drawn_arcs.len() - degs.len())); }
    let comp_lines = lines.iter().filter(|l| l.dashed && (l.sw - 2.2).abs() < 1e-9).count();
    let names = texts.iter().filter(|t| t.fill == "#6b7280").count();
    if names < comp_lines { out.push(format!("MISSING_COMP {}", comp_lines - names)); }
    // each arc's centre (the start of its own reference line) and span
    let refs: Vec<(f64, f64)> = lines.iter().filter(|l| l.dashed && (l.sw - 1.3).abs() < 1e-9).map(|l| l.a).collect();
    let geo: Vec<((f64, f64), f64, f64, f64, f64)> = drawn_arcs.iter().filter_map(|arc| {
        let (p1, p3) = (arc[0], *arc.last().unwrap());
        let o = refs.iter().copied().find(|&o| { let r = dist_seg(o, p1, p1); arc.iter().all(|&a| (dist_seg(o, a, a) - r).abs() < 0.3) })?;
        let ang = |a: (f64, f64)| (o.1 - a.1).atan2(a.0 - o.0).to_degrees();
        let (a0, a1) = (ang(p1), ang(p3));
        let mut span = (a1 - a0).rem_euclid(360.0);
        let dir = if span > 180.0 { span = 360.0 - span; -1.0 } else { 1.0 };
        Some((o, dist_seg(o, p1, p1), a0, dir * span, span))
    }).collect();
    let arc_dist = |c: (f64, f64), k: usize| drawn_arcs[k].windows(2).map(|w| dist_seg(c, w[0], w[1])).fold(f64::MAX, f64::min);
    let leaders = lines.iter().filter(|l| (l.sw - 0.9).abs() < 1e-9).count();
    for t in &degs {
        let said: f64 = t.content.trim_end_matches('\u{b0}').parse().unwrap();
        let c = leader_to(&lines, t).unwrap_or(centre(t));
        let own: Vec<usize> = (0..geo.len()).filter(|&k| (geo[k].4 - said).abs() < 0.15).collect();
        let Some(&k) = own.iter().min_by(|&&a, &&b| arc_dist(c, a).total_cmp(&arc_dist(c, b))) else { out.push(format!("NO_ARC {said}")); continue };
        let (o, r, a0, sweep_, _) = geo[k];
        let mine = arc_dist(c, k);
        if (0..geo.len()).any(|j| j != k && !own.contains(&j) && arc_dist(c, j) < mine + 1.5) { out.push(format!("NEARER_OTHER {said}")); }
        let th = (t.y1 - t.y0) - 0.0;
        let cc = centre(t);
        if (cc.0 - o.0).hypot(cc.1 - o.1) > r + 1.5 * th + 0.5 { out.push(format!("FAR {said}")); }
        let a = (o.1 - cc.1).atan2(cc.0 - o.0).to_degrees();
        let mut tt = (a - a0).rem_euclid(360.0);
        if sweep_ < 0.0 { tt = (360.0 - tt).rem_euclid(360.0); }
        if tt > 180.0 { tt -= 360.0; }
        if tt < -10.0 - 0.5 || tt > sweep_.abs() + 10.0 + 0.5 { out.push(format!("OUTSIDE {said}")); }
    }
    for f in label_faults(svg) { out.push(format!("FAULT {f}")); }
    (out, leaders)
}

/// Items 1 to 4: the checker's sweep of 15,534 figures, read from the drawn SVG: no name dropped, no value or
/// component name missing, every value by its own arc (inside its sector, near the arc, nearer it than any other), no
/// text on text or lines; and no leader at all on the two-vector figures with one mark.
#[test]
fn checker_sweep_is_clean() {
    let d = def();
    let fam = family();
    assert_eq!(fam.len(), 15_534);
    let mut bad = vec![];
    let mut led: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for (name, p) in &fam {
        let svg = (d.render)(p);
        let (probs, leaders) = sweep_problems(p, &svg);
        let setting = format!("{} vectors, {}", p.i("n"), name.split(' ').rev().nth(4).unwrap());
        let e = led.entry(setting).or_default();
        e.0 += 1;
        e.1 += (leaders > 0) as usize;
        if p.i("n") == 2 && name.contains(" one ") && leaders > 0 {
            bad.push(format!("{name}: a leader on a two-vector figure with one mark"));
        }
        if !probs.is_empty() {
            bad.push(format!("{name}: {}", probs.join("; ")));
        }
    }
    for (k, (all, l)) in &led {
        eprintln!("{k}: leaders in {l} of {all} ({:.1}%)", 100.0 * *l as f64 / *all as f64);
    }
    let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
    for b in &bad { for p in b.split(": ").nth(1).unwrap_or("").split("; ") { *kinds.entry(p.split(' ').next().unwrap().to_string()).or_default() += 1; } }
    eprintln!("sweep problems by kind: {kinds:?}");
    assert!(bad.is_empty(), "{} figures with problems:\n{}", bad.len(), bad[..bad.len().min(25)].join("\n"));
}

/// The page drawn (the batch 8 drawing may grow it for room).
fn page(svg: &str) -> (f64, f64) { crate::figures::svg_size(svg) }

/// Item 1: nothing addressed to the teacher is drawn, on the key or the student copy: notes go to the panel only.
#[test]
fn no_teacher_note_is_drawn() {
    let d = def();
    let mut cases: Vec<Params> = vec![open(&[("v2_ang", "90"), ("v2_comps", "true"), ("show_answers", "false")]), open(&[("v2_ang", "90"), ("v2_comps", "true")])];
    cases.extend(hard());
    cases.extend(crowded(0x0de_b8, 40));
    cases.extend(family().into_iter().step_by(23).map(|f| f.1));
    let mut noted = 0;
    for p in cases {
        for ans in [true, false] {
            let mut q = p.clone();
            q.set_b(SHOW_ANSWERS, ans);
            let svg = d.svg(&q);
            let (texts, _, _) = read(&svg);
            for t in &texts {
                let low = t.content.to_lowercase();
                assert!(!["room", "left off", "note", "editor", "lies along", "zero length"].iter().any(|w| low.contains(w)), "drawn: {:?}", t.content);
            }
            if !crate::figures::notes_for("vector-diagram", &q).is_empty() { noted += 1; }
        }
    }
    assert!(noted > 0, "the notes still reach the panel");
}

/// Item 2: a vector along the grid has no separate components; the note says so, quietly, and never "no room".
#[test]
fn an_arrow_along_the_grid_has_no_separate_components() {
    let d = def();
    for (ang, axis) in [("90", "y"), ("270", "y"), ("0", "x"), ("180", "x")] {
        let p = open(&[("v2_ang", ang), ("v2_comps", "true")]);
        let notes = crate::figures::notes_for("vector-diagram", &p);
        assert!(notes.iter().any(|n| n == &format!("B lies along the {axis} direction, so it has no separate components.")), "{ang}: {notes:?}");
        assert!(notes.iter().all(|n| !n.contains("room")), "{ang}: {notes:?}");
        let svg = (d.render)(&p);
        let (texts, lines, _) = read(&svg);
        assert!(!lines.iter().any(|l| l.dashed && (l.sw - 2.2).abs() < 1e-9), "{ang}: a component drawn on the vector itself");
        assert!(!texts.iter().any(|t| t.content == "Bx" || t.content == "By"), "{ang}");
    }
    // at an angle: drawn, and no note
    let p = open(&[("v2_ang", "60"), ("v2_comps", "true")]);
    assert!(crate::figures::notes_for("vector-diagram", &p).is_empty());
    // the resultant along the grid (A + B straight up and back): its components say the same
    let p = open(&[("v1_ang", "90"), ("components", "true"), ("v2_ang", "90"), ("v2_mark", "true"), ("v2_from", "Vertical")]);
    assert!(crate::figures::notes_for("vector-diagram", &p).iter().any(|n| n.starts_with("R lies along the y direction")));
}

/// Items 3 and 4 over the checker's realistic worksheet family, read from the drawn SVG: every arc drawn has its
/// value drawn, nearest that arc (or at the end of a leader from it) and stating its angle; every component drawn has
/// its name; no text on text, lines or heads; leaders short and slanted; a figure with one mark uses no leader. The
/// rare figure where a value or name still finds no clean spot even on the larger page is counted and capped (see the
/// report: the goal is none).
#[test]
fn realistic_family_keeps_angle_values_and_component_names() {
    let d = def();
    let (mut ang_off, mut comp_off, mut led, mut bad) = (0, 0, 0, vec![]);
    let fam = family();
    for (name, p) in &fam {
        let sc = vector_b8::scene(p);
        let svg = (d.render)(p);
        let (texts, lines, _) = read(&svg);
        let drawn_arcs = arcs(&svg);
        let degs: Vec<&Text> = texts.iter().filter(|t| t.content.ends_with('\u{b0}')).collect();
        let marks_off = sc.missed.iter().filter(|o| matches!(o, vector_b8::Own::Mark(_))).count();
        let comps_off = sc.missed.iter().filter(|o| matches!(o, vector_b8::Own::Comp(..))).count();
        ang_off += (marks_off > 0) as usize;
        comp_off += (comps_off > 0) as usize;
        if degs.len() + marks_off != drawn_arcs.len() {
            bad.push(format!("{name}: {} arcs, {} values drawn, {marks_off} left off", drawn_arcs.len(), degs.len()));
        }
        let comp_lines = lines.iter().filter(|l| l.dashed && (l.sw - 2.2).abs() < 1e-9).count();
        let names = texts.iter().filter(|t| svg.contains(&format!("fill=\"#6b7280\"")) && t.content.ends_with('x') || t.content.ends_with('y')).count();
        if names + comps_off < comp_lines {
            bad.push(format!("{name}: {comp_lines} components, {names} names drawn, {comps_off} left off"));
        }
        let leaders: Vec<&Line> = lines.iter().filter(|l| (l.sw - 0.9).abs() < 1e-9).collect();
        if !leaders.is_empty() { led += 1; }
        if name.contains("variant 0") && p.i("n") == 2 && !leaders.is_empty() {
            bad.push(format!("{name}: a leader on a figure with one mark"));
        }
        for l in &leaders {
            let (dx, dy) = (l.b.0 - l.a.0, l.b.1 - l.a.1);
            let deg = dy.atan2(dx).to_degrees().rem_euclid(90.0);
            if dx.hypot(dy) > 81.0 || !(15.0..=75.0).contains(&deg) {
                bad.push(format!("{name}: leader {l:?} long, level or upright"));
            }
        }
        // each value nearest its own arc (or at the end of a leader from it), and stating that arc's angle as drawn
        for t in &degs {
            let c = leader_to(&lines, t).unwrap_or(centre(t));
            let arc = drawn_arcs.iter().min_by(|a, b| {
                let da = a.windows(2).map(|w| dist_seg(c, w[0], w[1])).fold(f64::MAX, f64::min);
                let db = b.windows(2).map(|w| dist_seg(c, w[0], w[1])).fold(f64::MAX, f64::min);
                da.total_cmp(&db)
            }).unwrap();
            // the arc's centre: the start of its own dashed reference line (every point of the arc equally far from it)
            let (p1, p3) = (arc[0], *arc.last().unwrap());
            let Some(o) = lines.iter().filter(|l| l.dashed && (l.sw - 1.3).abs() < 1e-9).map(|l| l.a).find(|&o| { let r = dist_seg(o, p1, p1); arc.iter().all(|&a| (dist_seg(o, a, a) - r).abs() < 0.3) }) else {
                bad.push(format!("{name}: an arc with no reference line"));
                continue;
            };
            let ang = |a: (f64, f64)| (o.1 - a.1).atan2(a.0 - o.0).to_degrees();
            let mut span = (ang(p3) - ang(p1)).abs();
            if span > 180.0 { span = 360.0 - span; }
            let said: f64 = t.content.trim_end_matches('\u{b0}').parse().unwrap();
            if (said - span).abs() > 0.3 {
                bad.push(format!("{name}: {:?} sits by an arc of {span:.1}", t.content));
            }
        }
        let f = label_faults(&svg);
        if !f.is_empty() {
            bad.push(format!("{name}: {}", f.join("; ")));
        }
    }
    eprintln!("realistic family: {} figures; an angle value left off in {ang_off}, a component name in {comp_off}; leaders in {led}", fam.len());
    assert!(bad.is_empty(), "{} faults:\n{}", bad.len(), bad[..bad.len().min(30)].join("\n"));
    // ceilings measured at this commit: the goal is none
    assert!(ang_off <= ANG_OFF && comp_off <= COMP_OFF, "angle values left off in {ang_off}, component names in {comp_off}");
    assert!(led * 100 <= fam.len() * 15, "leaders in {led} of {}", fam.len());
}

/// Item 4: the plain textbook figures use no leader: the examples, and two vectors with one mark at any of the
/// worksheet angles.
#[test]
fn plain_figures_use_no_leader() {
    let d = def();
    for e in crate::figures::examples_for("vector-diagram") {
        let mut p = open(&[]);
        for (k, v) in e.params { p.0.insert(k.to_string(), v); }
        assert!(!(d.render)(&p).contains("stroke-width=\"0.9\""), "{}", e.name);
    }
    for (a, b, mark) in [("0", "60", "v2_mark"), ("30", "120", "v1_mark"), ("0", "90", "mark_angle"), ("45", "300", "v2_mark")] {
        let mut o = vec![("v1_ang", a), ("v2_ang", b), (mark, "true"), ("v2_comps", "true"), ("components", "true")];
        if mark == "mark_angle" { o.push(("ang_from", "Vertical")); }
        let svg = (d.render)(&open(&o));
        assert!(!svg.contains("stroke-width=\"0.9\""), "{o:?}");
    }
    // the checker's textbook figure: beta inside its own angle (above the horizontal, below B)
    let p = open(&[("v2_ang", "60"), ("components", "true"), ("v2_mark", "true"), ("v2_comps", "true")]);
    let sc = vector_b8::scene(&p);
    let (c, _, _) = sc.labels.iter().find(|l| matches!(l.2, vector_b8::Own::Mark(1))).unwrap();
    assert!(vector_b8::inside_angle(sc.marks.iter().find(|m| m.arrow == 1).unwrap(), *c));
    assert!(sc.leaders.is_empty());
}

/// Item 5: the layout is worked out once per figure (drawing, notes, handles and both copies share it) and the work
/// is bounded, over the realistic family and the crowded four-vector figures.
#[test]
fn layout_is_shared_and_bounded() {
    let d = def();
    let p = open(&[("n", "3"), ("v1_ang", "37"), ("v2_ang", "151"), ("v3_ang", "263"), ("v1_mark", "true"), ("v2_comps", "true"), ("mark_angle", "true")]);
    let before = vector_b8::layouts_built();
    let key = (d.render)(&p);
    let mut s = p.clone();
    s.set_b(SHOW_ANSWERS, false);
    let _ = (d.render)(&s);
    let _ = crate::figures::notes_for("vector-diagram", &p);
    let _ = crate::figures::notes_for("vector-diagram", &s);
    let _ = interact::handles_for("vector-diagram", &p, page(&key));
    assert_eq!(vector_b8::layouts_built() - before, 1, "the drawing, both copies, the notes and the handles share one layout");
    let mut most = 0;
    for p in family().into_iter().map(|f| f.1).chain(crowded(0x5eed_b8, 80)).chain(hard()) {
        most = most.max(vector_b8::scene(&p).work);
    }
    eprintln!("most candidate spots tried for one figure: {most}");
    // (no retry starts past the budget; the one under way may finish past it)
    assert!(most <= vector_b8::TOTAL_CAP + vector_b8::WORK_CAP, "{most}");
}

/// Item 6: a zero-length vector has no angle and no components: neither is drawn, every other label stays, and the
/// panel says why.
#[test]
fn zero_length_vector_marks_nothing_and_keeps_the_labels() {
    let d = def();
    let p = open(&[("v2_mag", "0"), ("v2_ang", "60"), ("v2_mark", "true"), ("v2_comps", "true"), ("v2_alab_mode", "Degrees")]);
    let svg = (d.render)(&p);
    let (texts, lines, _) = read(&svg);
    assert!(arcs(&svg).is_empty(), "an arc on a zero vector");
    assert!(!lines.iter().any(|l| l.dashed && (l.sw - 2.2).abs() < 1e-9));
    for name in ["A", "B", "R"] {
        assert!(texts.iter().any(|t| t.content == name), "{name} lost: {:?}", texts.iter().map(|t| &t.content).collect::<Vec<_>>());
    }
    assert!(!texts.iter().any(|t| t.content.ends_with('\u{b0}')));
    assert!(crate::figures::notes_for("vector-diagram", &p).iter().any(|n| n == "B has zero length, so it has no direction to mark and no components."));
    assert!(label_faults(&svg).is_empty());
}

/// Item 7: the student copy is the key with the answer parts left out: every element it draws, the key draws the same.
#[test]
fn student_copy_is_the_key_without_the_answers() {
    let d = def();
    let mut cases = hard();
    cases.extend(crowded(0x7e_57, 30));
    cases.push(open(&[("n", "4"), ("v1_ang", "30"), ("v2_ang", "120"), ("v3_ang", "210"), ("v4_ang", "300"), ("v1_comps", "true"), ("v2_comps", "true"), ("v3_comps", "true"), ("v4_comps", "true"), ("components", "true"), ("bold", "false")]));
    for p in cases {
        let mut k = p.clone();
        k.set_b(SHOW_ANSWERS, true);
        let mut s = p.clone();
        s.set_b(SHOW_ANSWERS, false);
        let (key, student) = ((d.render)(&k), (d.render)(&s));
        let elems = |svg: &str| svg.split('<').filter(|e| !e.starts_with('/') && !e.starts_with("svg") && !e.starts_with('?')).map(|e| e.to_string()).collect::<std::collections::BTreeSet<_>>();
        let (ke, se) = (elems(&key), elems(&student));
        let extra: Vec<&String> = se.difference(&ke).collect();
        assert!(extra.is_empty(), "the student copy draws what the key does not: {:?}", &extra[..extra.len().min(3)]);
    }
}

/// Item 8: each component's own label: Text (its typed text, else the name from the vector's), Blank line or None.
#[test]
fn each_component_has_its_own_label_choice() {
    let d = def();
    let base = [("v2_ang", "60"), ("v2_comps", "true")];
    let r = |o: &[(&str, &str)]| { let mut v = base.to_vec(); v.extend_from_slice(o); (d.render)(&open(&v)) };
    let texts = |svg: &str| read(svg).0.into_iter().map(|t| t.content).collect::<Vec<_>>();
    let auto = r(&[]);
    assert!(texts(&auto).contains(&"Bx".to_string()) && texts(&auto).contains(&"By".to_string()), "defaults keep today's names");
    let typed = r(&[("v2_cx_lab", "F_h")]);
    assert!(texts(&typed).contains(&"Fh".to_string()) && texts(&typed).contains(&"By".to_string()));
    let none = r(&[("v2_cx_mode", "None")]);
    assert!(!texts(&none).contains(&"Bx".to_string()) && texts(&none).contains(&"By".to_string()));
    let blank = r(&[("v2_cy_mode", "Blank line")]);
    assert!(!texts(&blank).contains(&"By".to_string()));
    assert!(read(&blank).1.iter().any(|l| is_blank(l)), "a blank answer line");
    let all = [&auto, &typed, &none, &blank];
    for i in 0..4 { for j in i + 1..4 { assert_ne!(all[i], all[j]); } }
    // the choices sit with the vector, and only once its components are on
    let rel = |p: &Params, k: &str| crate::figures::param_relevant("vector-diagram", k, p);
    assert!(rel(&open(&base), "v2_cx_mode") && !rel(&open(&[("v2_ang", "60")]), "v2_cx_mode"));
    assert!(!rel(&open(&[("v2_ang", "60"), ("v2_comps", "true"), ("v2_cx_mode", "None")]), "v2_cx_lab"));
}



/// Item 5: a drag step starts from the last layout (its page, its arcs' radii, its labels' places and their order):
/// over steps of a degree from solved crowded figures (every mark and component on, the same labels drawn before and
/// after), the steps cost under a fifth of
/// the cold layouts at the median and in two steps of three, and every step still draws clean by the sweep's rules.
#[test]
fn a_drag_step_starts_from_the_last_layout() {
    let d = def();
    let (mut cold_all, mut warm_all, mut ratios, mut worst) = (0usize, 0usize, vec![], (0.0f64, String::new()));
    for (name, p) in family().into_iter().filter(|f| f.0.contains(" all ")).step_by(19) {
        vector_b8::forget_last();
        let first = vector_b8::scene(&p);
        let cold = first.work;
        // (a crowded figure: a trivial one costs next to nothing cold)
        if cold < 2000 {
            continue;
        }
        let mut q = p.clone();
        q.set_f("v2_ang", q.f("v2_ang") + 1.0);
        let next = vector_b8::scene(&q);
        let warm = next.work;
        let (probs, _) = sweep_problems(&q, &(d.render)(&q));
        // (a step that changes what is drawn, a vector turning off the grid and gaining its components, is no small
        // change: not counted)
        let owns = |s: &vector_b8::Scene| { let mut v: Vec<String> = s.labels.iter().map(|l| format!("{:?}", l.2)).collect(); v.sort(); v };
        if owns(&first) != owns(&next) {
            continue;
        }
        assert!(probs.is_empty(), "{name} one step on: {probs:?}");
        cold_all += cold;
        warm_all += warm;
        let r = warm as f64 / cold as f64;
        if r > worst.0 { worst = (r, name.clone()); }
        ratios.push(r);
    }
    ratios.sort_by(|a, b| a.total_cmp(b));
    let median = ratios[ratios.len() / 2];
    eprintln!("drag steps: {} from crowded figures, in all {warm_all} spots against {cold_all} cold ({:.1}%), median {:.1}%, worst {:.0}% ({})", ratios.len(), 100.0 * warm_all as f64 / cold_all as f64, 100.0 * median, 100.0 * worst.0, worst.1);
    let under = ratios.iter().filter(|r| **r < 0.2).count();
    eprintln!("drag steps under a fifth of a cold layout: {under} of {}", ratios.len());
    assert!(ratios.len() >= 40);
    // the typical step settles in under a fifth of a cold layout (measured at this commit: median 7.8%, 47 of 70 under a
    // fifth); a step whose new place is itself a hard layout searches afresh
    assert!(median < 0.2 && under * 3 >= ratios.len() * 2, "median {median}, {under} of {} under a fifth", ratios.len());
}



