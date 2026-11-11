//! Batch 8: a line's label dragged across its line stays on the side it was dropped. The side is a row of `label_sides` (empty by
//! default, so every old figure draws as it did): the line, then `left` or `right` as seen looking along the line from its first
//! end to its second. The circle's own lines are named by a word (`radius` OA, `chord` AB, `diameter` from A through the centre,
//! `om` OM, `am` AM, `secant` AB), a labelled line by its two letters (`BD left`, read with the figure's own reader, so `B D` and
//! `D B right` name the same side), a tangent's word by `tangent C`. The label is then placed only where its whole box is on that
//! side, by the same search as before, so it stays clear of the other labels, lines and points and inside the page.
use super::edit::{self, End, V};
use super::*;

/// Which line a label belongs to.
#[derive(Clone, Debug, PartialEq)]
pub enum LKey { Word(&'static str), Line(V, V), Tan(f64) }

pub const WORDS: [&str; 6] = ["radius", "chord", "diameter", "om", "am", "secant"];

/// The tip of a label's knob.
pub const TIP: &str = "Drag the label across its line to put it on the other side";

fn near(a: V, b: V) -> bool { (a.0 - b.0).hypot(a.1 - b.1) < 1e-6 }

/// A row's line and side (true: left), read with the figure's own readers; None for a row that names no line on the figure.
fn read(p: &Params, row: &str) -> Option<(LKey, bool)> {
    let toks: Vec<&str> = row.split_whitespace().collect();
    let (last, rest) = toks.split_last()?;
    let left = match last.to_lowercase().as_str() { "left" => true, "right" => false, _ => return None };
    let pts = edit::circle_points(p);
    if let [w] = rest && let Some(k) = WORDS.iter().find(|k| k.eq_ignore_ascii_case(w)) { return Some((LKey::Word(k), left)); }
    if rest.len() == 2 && rest[0].eq_ignore_ascii_case("tangent") {
        let End::At(d) = edit::find(p, &pts, rest[1])? else { return None };
        return Some((LKey::Tan(d), left));
    }
    let known = edit::known_names(p, &pts);
    let (a, b) = edit::pair_names(&rest.join(" "), &known)?;
    let (x, y) = (edit::find(p, &pts, &a)?, edit::find(p, &pts, &b)?);
    if edit::same_place(p, x, y) { return None; }
    Some((LKey::Line(edit::end_pos(p, x), edit::end_pos(p, y)), left))
}

/// Whether `row` sets the side of `key`, and the side it sets for `key` as it is oriented (a line written the other way round
/// has its sides swapped).
fn sets(p: &Params, row: &str, key: &LKey) -> Option<bool> {
    let (k, left) = read(p, row)?;
    match (&k, key) {
        (LKey::Word(a), LKey::Word(b)) if a == b => Some(left),
        (LKey::Tan(a), LKey::Tan(b)) if angdiff(*a, *b) < 1e-6 => Some(left),
        (LKey::Line(u, w), LKey::Line(x, y)) if near(*u, *x) && near(*w, *y) => Some(left),
        (LKey::Line(u, w), LKey::Line(x, y)) if near(*u, *y) && near(*w, *x) => Some(!left),
        _ => None,
    }
}

/// The side a row puts the label of `key` on (true: left), the last row that names it; None leaves it where the figure puts it.
pub fn side(p: &Params, key: &LKey) -> Option<bool> {
    let rows = edit::rows(p.s("label_sides"));
    if rows.is_empty() { return None; }
    rows.iter().rev().find_map(|r| sets(p, r, key))
}

/// The letter of the place `u` on the figure (the centre, a point, M, a point off the circle).
fn letter_at(p: &Params, u: V) -> Option<String> {
    let n = names(p);
    if near(u, (0.0, 0.0)) { return Some(n.o); }
    if let Some((nm, _)) = edit::circle_points(p).into_iter().find(|(_, d)| near(u, (d.to_radians().cos(), d.to_radians().sin()))) { return Some(nm); }
    if edit::m_ok(p) && near(u, edit::end_pos(p, End::M)) { return Some(n.m); }
    pts::free_points(p).into_iter().find(|f| near(f.1, u)).map(|f| f.0)
}

/// The row that puts the label of `key` on the left (or right) side.
fn row_for(p: &Params, key: &LKey, left: bool) -> Option<String> {
    let s = if left { "left" } else { "right" };
    match key {
        LKey::Word(w) => Some(format!("{w} {s}")),
        LKey::Tan(d) => edit::circle_points(p).into_iter().find(|x| angdiff(x.1, *d) < 1e-6).map(|x| format!("tangent {} {s}", x.0)),
        LKey::Line(u, w) => {
            let (a, b) = (letter_at(p, *u)?, letter_at(p, *w)?);
            Some(if a.chars().count() > 1 || b.chars().count() > 1 { format!("{a} {b} {s}") } else { format!("{a}{b} {s}") })
        }
    }
}

/// Puts the label of `key` on the left (or right) of its line: the rows that named it go, one row says the side.
pub fn set_side(p: &mut Params, key: &LKey, left: bool) {
    let Some(row) = row_for(p, key, left) else { return };
    let q = p.clone();
    let mut r: Vec<String> = edit::rows(q.s("label_sides")).into_iter().filter(|x| sets(&q, x, key).is_none()).collect();
    r.push(row);
    edit::set_rows(p, "label_sides", &r);
}

/// How far `q` is to the left of the line from `a` to `b` (SVG units, y down): positive on the left, negative on the right.
pub fn left_of(a: Pt, b: Pt, q: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx.hypot(dy).max(1e-9);
    -(dx * (q.1 - a.1) - dy * (q.0 - a.0)) / l
}

/// Whether the whole box `r` is on the `sign` side (1 left, -1 right) of the line through `a` and `b`, by at least 2 px.
pub fn box_on_side(r: &Rect, a: Pt, b: Pt, sign: f64) -> bool {
    [(r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)].iter().all(|c| left_of(a, b, *c) * sign >= 2.0)
}

/// The knob of a line's label (its text box, drawn on top of the other handles): dragged across the line, the label goes to the
/// other side and stays there. A click or double-click on it does what it did on the part under it before the label had a knob.
pub fn knob(key: LKey, rect: Rect, a: Pt, b: Pt, centre: Pt, hit: std::sync::Arc<edit::Hit>) -> Handle {
    let c = ((rect.x0 + rect.x1) / 2.0, (rect.y0 + rect.y1) / 2.0);
    let cur = left_of(a, b, c) > 0.0;
    let mut h = Handle::knob(c, TIP, move |p, t| {
        if super::unmoved(c, t) || !(t.0.is_finite() && t.1.is_finite()) { return; }
        let l = left_of(a, b, t);
        // past the line by a little, so a label that only wobbles on its own side stays where it is
        if l.abs() < 2.0 || (l > 0.0) == cur { return; }
        set_side(p, &key, l > 0.0);
    })
    .with_edit_at("styles", move |p, q| clicks::edit_at(p, centre, q));
    let h2 = hit.clone();
    h.click_at = Some(std::sync::Arc::new(move |p: &mut Params, q: Pt| h2.act(p, q)));
    h.shape = interact::Shape::Zone;
    h.cursor = interact::Cursor::Move;
    h.half = ((rect.x1 - rect.x0) / 2.0, (rect.y1 - rect.y0) / 2.0);
    h.label_drag = true;
    h
}

// ------------------------------------------------------------------------------------------------ placing a label on its side

/// Whether the leader from `f` to `q` (on the edge of its own box `own`) is one a reader can follow: it does not run into its own
/// text, nor through any other label, point or leader, nor along a drawn line (checker round 1). `segs` are the drawn lines.
pub fn leader_ok(f: Pt, q: Pt, own: &Rect, rects: &[Rect], dots: &[Rect], leaders: &[(Pt, Pt)], segs: &[(Pt, Pt)]) -> bool {
    let len = (q.0 - f.0).hypot(q.1 - f.1);
    if len < 1e-6 { return true; }
    // not into its own text: it ends on the edge of the box, coming from outside
    if seg_hits(f, q, &own.grow(-1.0)) { return false; }
    if rects.iter().any(|r| seg_hits(f, q, &r.grow(1.0))) { return false; }
    // (a dot at the leader's own start is the mark it points from)
    if dots.iter().any(|d| { let c = ((d.x0 + d.x1) / 2.0, (d.y0 + d.y1) / 2.0); (c.0 - f.0).hypot(c.1 - f.1) > 6.0 && seg_hits(f, q, &d.grow(1.0)) }) { return false; }
    if leaders.iter().any(|l| super::leaders_clash((f, q), *l)) { return false; }
    // along a drawn line: nearly parallel to it and close to it over some length
    let u = ((q.0 - f.0) / len, (q.1 - f.1) / len);
    for (a, b) in segs {
        let sl = (b.0 - a.0).hypot(b.1 - a.1);
        if sl < 1e-6 { continue; }
        let w = ((b.0 - a.0) / sl, (b.1 - a.1) / sl);
        if (u.0 * w.1 - u.1 * w.0).abs() > 0.14 { continue; }
        let near = (1..=7).map(|k| k as f64 / 8.0).filter(|t| super::pt_seg_dist((f.0 + (q.0 - f.0) * t, f.1 + (q.1 - f.1) * t), *a, *b) < 3.0).count();
        if near >= 2 { return false; }
    }
    true
}

/// The best leader for box `r` from the starts `froms`, by the rules of `leader_ok`: (start, end on the box, lines it crosses).
fn route(froms: &[Pt], r: &Rect, rects: &[Rect], dots: &[Rect], leaders: &[(Pt, Pt)], segs: &[(Pt, Pt)]) -> Option<(Pt, Pt, u32)> {
    let mut best: Option<(Pt, Pt, u32, f64)> = None;
    let (mx, my) = ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0);
    for f in froms {
        let near = super::nearest_on_rect(r, *f);
        for q in [near, (mx, r.y0), (mx, r.y1), (r.x0, my), (r.x1, my), (r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)] {
            if !leader_ok(*f, q, r, rects, dots, leaders, segs) { continue; }
            let len = (q.0 - f.0).hypot(q.1 - f.1);
            let cross = segs.iter().skip(super::CIRCLE_SEGS).filter(|sg| super::seg_cross(*f, q, sg.0, sg.1).is_some_and(|t| t * len > 3.0 && (1.0 - t) * len > 0.5)).count() as u32;
            if best.is_none_or(|b| (cross, len) < (b.2, b.3)) { best = Some((*f, q, cross, len)); }
        }
    }
    best.map(|b| (b.0, b.1, b.2))
}

/// Places the labels a teacher put on one side of their line (`labs`), every other label already placed (`pre`, where the figure
/// puts them with no side chosen, so choosing a side moves that one label only). A sided label goes only where its whole box is on
/// its side and touches nothing (no text, point, line or edge of the page), its leader by `leader_ok`; when no such spot is on that
/// side it goes on the other side by the same rules, and its id is returned in the third list (the notes say so); when neither side
/// has one, it is placed as any label is (`super::place`).
pub(super) fn place_sided(labs: Vec<Lab>, segs: &[(Pt, Pt)], solid: &[(Pt, Pt)], dots: &[Rect], bounds: &Rect, hatch: &[Vec<Pt>], sides: &[(usize, Pt, Pt, f64)], pre: Vec<Placed>) -> (Vec<Placed>, Vec<(usize, String, Option<K>)>, Vec<usize>) {
    let mut placed = pre;
    let mut leaders: Vec<(Pt, Pt)> = placed.iter().filter_map(|p| p.line).collect();
    let mut severe = vec![];
    let mut fell = vec![];
    let mut labs = labs;
    labs.sort_by_key(|l| l.prio);
    for l in labs {
        let Some(&(_, a, b, sg)) = sides.iter().find(|x| x.0 == l.id) else { continue };
        let mt = l.wide.as_ref().unwrap_or(&l.text);
        let (w, h) = (tw(mt, l.size, l.bold) + 2.0, l.size * 1.15);
        let from0 = l.leader_from.unwrap_or(l.anchor);
        let froms: Vec<Pt> = if l.froms.is_empty() { vec![from0] } else { l.froms.clone() };
        let mut cands = l.cands.clone();
        let own = cands.len();
        for extra in [12.0, 24.0, 44.0, 70.0, 98.0, 130.0, 170.0] {
            for i in 0..24 {
                let ang = i as f64 * PI / 12.0;
                let d = (ang.cos(), ang.sin());
                let sup = d.0.abs() * w / 2.0 + d.1.abs() * h / 2.0;
                for f in &froms { cands.push(((f.0 + d.0 * (sup + extra), f.1 + d.1 * (sup + extra)), true)); }
            }
        }
        let rects: Vec<Rect> = placed.iter().map(|p| p.rect).collect();
        let try_side = |sign: f64| -> Option<(Rect, Option<(Pt, Pt)>, bool, Pt)> {
            let mut best: Option<((u32, u32, u32, f64), Rect, Option<(Pt, Pt)>, bool, Pt)> = None;
            for (i, (c, far)) in cands.iter().enumerate() {
                let r = Rect::around(*c, w, h);
                if !box_on_side(&r, a, b, sign) || super::conflicts(&r, &rects, segs, &leaders, dots, bounds) != 0 { continue; }
                if let Some((o, rad)) = l.keep_out { let q = super::nearest_on_rect(&r, o); if (q.0 - o.0).hypot(q.1 - o.1) < rad + 2.0 { continue; } }
                let f = super::pick_from(&l.froms, from0, *c);
                let q = super::nearest_on_rect(&r, f);
                let dist = (q.0 - f.0).hypot(q.1 - f.1);
                let needs = l.force || (*far && dist > 6.0) || dist > 18.0;
                let (line, cross, len) = if needs && dist > 2.0 {
                    match route(&froms, &r, &rects, dots, &leaders, segs) { Some((f2, q2, x)) => (Some((f2, q2)), x, (q2.0 - f2.0).hypot(q2.1 - f2.1)), None => continue }
                } else { (None, 0, 0.0) };
                let on_hatch = hatch.iter().any(|hh| super::rect_hits_poly(&r.grow(-2.0), hh));
                let key = (on_hatch as u32, cross, (i >= own) as u32, len);
                if best.as_ref().is_none_or(|bk| key < bk.0) { best = Some((key, r, line, on_hatch, line.map(|x| x.0).unwrap_or(f))); }
            }
            best.map(|b| (b.1, b.2, b.3, b.4))
        };
        let got = try_side(sg).or_else(|| { let o = try_side(-sg); if o.is_some() { fell.push(l.id); } o });
        match got {
            Some((rect, line, on_hatch, anchor)) => {
                if let Some(ld) = line { leaders.push(ld); }
                let plate = on_hatch && !segs.iter().any(|s| seg_hits(s.0, s.1, &rect));
                placed.push(Placed { id: l.id, kind: l.kind, text: l.text, rect, clean: true, leader: line.is_some(), plate, anchor, line, size: l.size, bold: l.bold, c: ((rect.x0 + rect.x1) / 2.0, (rect.y0 + rect.y1) / 2.0) });
            }
            None => {
                // nowhere clean on either side: as any label is placed, among the others
                let pre2 = std::mem::take(&mut placed);
                let (mut out, sev) = super::place(vec![l], segs, solid, dots, bounds, hatch, &[], pre2);
                severe.extend(sev);
                leaders = out.iter().filter_map(|p| p.line).collect();
                placed.append(&mut out);
            }
        }
    }
    (placed, severe, fell)
}

/// What a line is called in a note.
fn line_name(p: &Params, key: &LKey) -> String {
    let n = names(p);
    match key {
        LKey::Word("radius") => format!("radius {}{}", n.o, n.a),
        LKey::Word("chord") => format!("chord {}{}", n.a, n.b),
        LKey::Word("diameter") => format!("the diameter through {}", n.a),
        LKey::Word("om") => format!("{}{}", n.o, n.m),
        LKey::Word("am") => format!("{}{}", n.a, n.m),
        LKey::Word("secant") => "the secant".into(),
        LKey::Word(w) => w.to_string(),
        LKey::Tan(d) => edit::circle_points(p).into_iter().find(|x| angdiff(x.1, *d) < 1e-6).map(|x| format!("the tangent at {}", x.0)).unwrap_or_else(|| "a tangent".into()),
        LKey::Line(u, w) => match (letter_at(p, *u), letter_at(p, *w)) { (Some(a), Some(b)) => format!("line {a}{b}"), _ => "a line".into() },
    }
}

/// Notes for labels that could not go on the side chosen for them.
pub fn warnings(p: &Params) -> Vec<String> {
    if edit::rows(p.s("label_sides")).is_empty() { return vec![]; }
    build(p).1.side_fell.iter().map(|k| format!("The label on {} has no room on the side chosen without touching other text or lines, so it is on the other side.", line_name(p, k))).collect()
}

