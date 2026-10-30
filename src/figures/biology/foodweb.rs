//! A food web from typed links: one line per link, `Prey -> Predator`, drawn with the producers at the bottom and
//! energy flowing upward. A link may be written as a whole chain (`Grass -> Rabbit -> Fox`), with `<-` to write it
//! from the predator's end, and a `#` line is a note. Spaces in names are fine.
//!
//! The web is the figure, so there is nothing to hide: Show answers has no effect and the figure does not offer it.
//! A food web cannot run in a circle; a typed loop is refused with the names in the loop.

use super::cladogram::text_w;
use super::*;

pub const ID: &str = "food-web";

/// More names than one picture can hold; refused with a reason rather than drawn unreadably.
const MAX_NAMES: usize = 24;
const INK: &str = "#111111";
const EDGE: &str = "#333333";
const ERR: &str = "#c0392b";
/// The trophic level names, bottom up; past the fifth a numbered level is used.
const LEVELS: [&str; 5] = ["Producers", "Primary consumers", "Secondary consumers", "Tertiary consumers", "Quaternary consumers"];
const NODE_FS: f64 = 13.0;
const LEVEL_FS: f64 = 12.0;
const PAD_X: f64 = 10.0;
const PAD_Y: f64 = 8.0;
const NODE_GAP: f64 = 18.0;
const LEVEL_GAP: f64 = 76.0;
const EDGE_W: f64 = 1.5;
/// The narrowest page these figures use; a refusal never widens it.
const MIN_W: f64 = 420.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Food web",
        blurb: "Type the links of a food web, one per line, and get the web: producers at the bottom, arrows to whatever eats them, and the trophic levels named.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("links", "Food chains", "Grass -> Rabbit\nGrass -> Grasshopper\nGrasshopper -> Frog\nRabbit -> Fox\nFrog -> Fox")
            .hint("Prey -> Predator, one per line; a whole chain may go on one line (Grass -> Rabbit -> Fox). Use <- to write it from the predator's end. A # line is a note."),
        ParamDef::bool("levels", "Name the trophic levels", true),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p) }

/// A food web: names in the order they first appear, and (prey, predator) index pairs, deduplicated.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Web {
    pub names: Vec<String>,
    pub edges: Vec<(usize, usize)>,
}

impl Web {
    pub fn is_empty(&self) -> bool { self.names.is_empty() }
}

/// Why a typed web was refused, and the line to show under the reason. The kind is known where the refusal is
/// raised, so the second line never guesses from the message text (verification round 2: a name such as `mixes`
/// made the sentence under a self-eat line talk about arrow directions).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub why: String,
    pub then: &'static str,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.why) }
}

fn refuse(why: String, then: &'static str) -> Refusal { Refusal { why, then } }

/// Reads the typed links. Blank lines and `#` notes are skipped; `->` and `→` point from prey to predator, `<-`
/// the other way; a chain may be written on one line. Everything else is a name, spaces included.
pub fn parse(text: &str) -> Result<Web, Refusal> {
    let mut names: Vec<String> = Vec::new();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let at = n + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let line = line.replace('→', "->").replace('←', "<-");
        let (fwd, back) = (line.matches("->").count(), line.matches("<-").count());
        if fwd > 0 && back > 0 {
            return Err(refuse(format!("Line {at} mixes -> and <-; use one direction on a line."), "Nothing is drawn until every line uses one direction."));
        }
        if fwd == 0 && back == 0 {
            return Err(refuse(format!("Line {at} has no arrow: write Prey -> Predator."), "Nothing is drawn until every line is a link."));
        }
        let parts: Vec<String> = if fwd > 0 {
            line.split("->").map(tidy).collect()
        } else {
            let mut v: Vec<String> = line.split("<-").map(tidy).collect();
            v.reverse();
            v
        };
        if parts.iter().any(String::is_empty) {
            return Err(refuse(format!("Line {at} has an empty name: put a name on both sides of every arrow."), "Nothing is drawn: an unnamed organism cannot be placed."));
        }
        if parts.len() == 1 {
            return Err(refuse(format!("Line {at} has no arrow: write Prey -> Predator."), "Nothing is drawn until every line is a link."));
        }
        for pair in parts.windows(2) {
            let prey = index_of(&mut names, &pair[0], at)?;
            let predator = index_of(&mut names, &pair[1], at)?;
            if prey == predator {
                return Err(refuse(format!("Line {at}: {} cannot eat itself.", pair[0]), "Nothing is drawn until no name eats itself."));
            }
            if !edges.contains(&(prey, predator)) {
                edges.push((prey, predator));
            }
        }
    }
    if let Some(cycle) = cycle(&names.len(), &edges) {
        let path = cycle.iter().map(|&i| names[i].as_str()).collect::<Vec<_>>().join(" -> ");
        return Err(refuse(format!("A food web cannot run in a circle: {path}. Remove one of those links."), "Nothing is drawn until the links form a one-way web."));
    }
    Ok(Web { names, edges })
}

/// A name as typed, tidied: one space for any run of whitespace, none at the ends. So `Arctic  fox` is the same
/// organism as `Arctic fox`, while a double space can not make a second one.
fn tidy(s: &str) -> String { s.split_whitespace().collect::<Vec<_>>().join(" ") }

fn index_of(names: &mut Vec<String>, name: &str, line: usize) -> Result<usize, Refusal> {
    if let Some(i) = names.iter().position(|n| n == name) { return Ok(i); }
    if names.len() >= MAX_NAMES {
        return Err(refuse(format!("More than {MAX_NAMES} names (line {line}): split the web into two pictures."), "Nothing is drawn: a web this large cannot be read."));
    }
    names.push(name.to_string());
    Ok(names.len() - 1)
}

/// The trophic level of each name: 0 for a producer (nothing eats), otherwise one more than the highest level it
/// eats. `None` when the links run in a circle (there is no such level).
pub fn level_of(web: &Web) -> Option<Vec<usize>> {
    let n = web.names.len();
    let mut adj = vec![Vec::new(); n];
    let mut indeg = vec![0usize; n];
    for &(prey, predator) in &web.edges {
        adj[prey].push(predator);
        indeg[predator] += 1;
    }
    let mut q: std::collections::VecDeque<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut level = vec![0usize; n];
    let mut seen = 0usize;
    while let Some(prey) = q.pop_front() {
        seen += 1;
        for &predator in &adj[prey] {
            level[predator] = level[predator].max(level[prey] + 1);
            indeg[predator] -= 1;
            if indeg[predator] == 0 { q.push_back(predator); }
        }
    }
    (seen == n).then_some(level)
}

/// One circle in the links, as name indices, for the refusal's reason. Only called when [`level_of`] found one.
fn cycle(n: &usize, edges: &[(usize, usize)]) -> Option<Vec<usize>> {
    let mut adj = vec![Vec::new(); *n];
    for &(a, b) in edges { adj[a].push(b); }
    let mut state = vec![0u8; *n];
    let mut stack: Vec<usize> = Vec::new();
    fn go(v: usize, adj: &[Vec<usize>], state: &mut [u8], stack: &mut Vec<usize>) -> Option<Vec<usize>> {
        state[v] = 1;
        stack.push(v);
        for &w in &adj[v] {
            if state[w] == 0 {
                if let Some(c) = go(w, adj, state, stack) { return Some(c); }
            } else if state[w] == 1 {
                let at = stack.iter().position(|&x| x == w).unwrap_or(0);
                let mut c = stack[at..].to_vec();
                c.push(w);
                return Some(c);
            }
        }
        stack.pop();
        state[v] = 2;
        None
    }
    for v in 0..*n {
        if state[v] == 0 && let Some(c) = go(v, &adj, &mut state, &mut stack) { return Some(c); }
    }
    None
}

/// The names placed on their trophic level, level 0 (producers) first.
pub struct Placed {
    pub rows: Vec<Vec<usize>>,
    pub level: Vec<usize>,
}

pub fn place(web: &Web) -> Placed {
    let level = level_of(web).unwrap_or_default();
    let mut rows: Vec<Vec<usize>> = Vec::new();
    for (i, &l) in level.iter().enumerate() {
        if rows.len() <= l { rows.resize(l + 1, Vec::new()); }
        rows[l].push(i);
    }
    if rows.is_empty() && !web.names.is_empty() { rows.push((0..web.names.len()).collect()); }
    Placed { rows, level }
}

/// The level's name: the familiar one for the first five, then "Level n".
pub fn level_name(l: usize) -> String {
    LEVELS.get(l).map(|s| s.to_string()).unwrap_or_else(|| format!("Level {l}"))
}

/// The room one row gap gives the routed runs, as (clear, arrival margin, departure margin, usable band). Only an
/// arrival has to clear an arrowhead; a departure only needs a small margin off its box.
fn band(lab: f64) -> (f64, f64, f64, f64) {
    let box_h = NODE_FS * lab + 2.0 * PAD_Y * lab;
    let normal_hl = (EDGE_W * lab * 4.0).max(9.0);
    let clear = (LEVEL_GAP * lab - box_h).max(0.0);
    let (arrival, departure) = (normal_hl + 2.0, 4.0 * lab);
    (clear, arrival, departure, (clear - arrival - departure).max(0.0))
}

/// Runs through each row gap: a link that skips a level leaves its prey's gap and enters its predator's.
fn gap_runs(web: &Web, level: &[usize]) -> Vec<usize> {
    let mut n = vec![0usize; level.iter().max().map_or(0, |m| m + 1)];
    for &(prey, predator) in &web.edges {
        if level[predator].saturating_sub(level[prey]) > 1 {
            n[level[prey]] += 1;
            n[level[predator] - 1] += 1;
        }
    }
    n
}

/// A web too dense to draw: some gap holds so many runs that they could not stay a stroke plus one visible pixel apart,
/// and the picture would read as links that do not exist. It is refused rather than drawn (owner decision, 2026-10-29).
/// The limit moves with the label size, because the gap does: 10 runs at Small, 12 at Medium, 15 at Large.
pub fn crowded(web: &Web, lab: f64) -> bool {
    let Some(level) = level_of(web) else { return false };
    let height = band(lab).3;
    gap_runs(web, &level).iter().any(|&n| n > 0 && height / (n as f64 + 1.0) < EDGE_W * lab + 1.0)
}

pub fn warnings(p: &Params) -> Vec<String> {
    match parse(p.s("links")) {
        Ok(w) if w.is_empty() => Vec::new(),
        Ok(w) if crowded(&w, svg::label_factor(p.s("label_size"))) => vec![
            "The food web could not be drawn: too many links skip a trophic level to keep their runs apart. Remove some, or choose a larger label size, which leaves more room.".to_string(),
        ],
        Ok(_) => Vec::new(),
        Err(e) => vec![format!("The food web could not be read: {e}")],
    }
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}">{}</text>"##,
        svg::esc(s)
    )
}

fn wrap(s: &str, max: f64, size: f64) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        if !cur.is_empty() {
            if text_w(&cur, size) + text_w(" ", size) + text_w(word, size) <= max {
                cur.push(' ');
                cur.push_str(word);
                continue;
            }
            out.push(std::mem::take(&mut cur));
        }
        // (critic minor 7) a word wider than the line is broken, or a long unbroken name runs off the canvas
        let mut rest = word;
        loop {
            if text_w(rest, size) <= max || rest.chars().count() <= 1 {
                cur.push_str(rest);
                break;
            }
            // the longest prefix that still fits; a single glyph wider than the line is kept whole
            let mut cut = 0usize;
            for (i, _) in rest.char_indices() {
                if i == 0 { continue; }
                if text_w(&rest[..i], size) <= max { cut = i; } else { break; }
            }
            if cut == 0 { cut = rest.char_indices().nth(1).map(|(i, _)| i).unwrap_or(rest.len()); }
            out.push(rest[..cut].to_string());
            rest = rest[cut..].trim_start();
        }
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

fn plural(n: usize, one: &str, many: &str) -> String { if n == 1 { one.to_string() } else { many.to_string() } }

/// The canvas for a web that is not drawn: a header naming what failed, the reason, and what to do about it. Used
/// both for a typed web that cannot be read and for one that is too dense to draw without its runs merging.
fn refusal(header: &str, why: &str, then: &str, lab: f64) -> String {
    let max = MIN_W - 48.0;
    let first = wrap(why, max, 13.0 * lab);
    let last = wrap(then, max, 12.0 * lab);
    let h = 34.0 + (first.len() as f64 + 1.0 + last.len() as f64) * 18.0 * lab + 20.0;
    let mut d = Doc::new(MIN_W, h);
    d.describe("Food web", "A food web from typed links");
    d.raw(&text_el(24.0, 40.0 * lab, 13.0 * lab, "start", ERR, header));
    let mut y = 64.0 * lab;
    for line in &first {
        d.raw(&text_el(24.0, y, 13.0 * lab, "start", INK, line));
        y += 18.0 * lab;
    }
    y += 6.0 * lab;
    for line in &last {
        d.raw(&text_el(24.0, y, 12.0 * lab, "start", INK, line));
        y += 16.0 * lab;
    }
    d.finish()
}

pub fn build(p: &Params) -> String {
    let lab = svg::label_factor(p.s("label_size"));
    let (ink, edge_col) = (INK, EDGE);
    let fs = NODE_FS * lab;
    let level_fs = LEVEL_FS * lab;
    let show_levels = p.b("levels");
    let box_h = fs + 2.0 * PAD_Y * lab;
    let min_w = MIN_W;

    match parse(p.s("links")) {
        Ok(web) if web.is_empty() => {
            let reason = "Type one link per line, for example Grass -> Rabbit. A whole chain may go on one line: Grass -> Rabbit -> Fox.";
            let max = min_w - 48.0;
            let lines = wrap(reason, max, 13.0 * lab);
            let h = 34.0 + lines.len() as f64 * 18.0 * lab + 20.0;
            let mut d = Doc::new(min_w, h);
            d.describe("Food web", "A food web from typed links");
            for (i, line) in lines.iter().enumerate() {
                d.raw(&text_el(24.0, 34.0 + i as f64 * 18.0 * lab, 13.0 * lab, "start", ink, line));
            }
            return d.finish();
        }
        Err(e) => refusal("The food web could not be read:", &e.why, e.then, lab),
        Ok(web) => {
            let placed = place(&web);
            let normal_hl = (EDGE_W * lab * 4.0).max(9.0);
            let slot_need = normal_hl * 0.84 + 2.0 * lab;
            let mut widths: Vec<f64> = web.names.iter().map(|n| text_w(n, fs) + 2.0 * PAD_X * lab).collect();
            // (verification critic, MED) on a box narrower than its own slots a row of heads merges into a band; widen
            // the box rather than shrink the heads, so every head is the same size and every attachment stays apart.
            for node in 0..web.names.len() {
                let n = web.edges.iter().filter(|e| e.0 == node).count().max(web.edges.iter().filter(|e| e.1 == node).count());
                if n > 1 { widths[node] = widths[node].max(n as f64 * slot_need); }
            }
            let rows = placed.rows.len();
            let row_w = |row: &[usize]| row.iter().map(|i| widths[*i]).sum::<f64>() + row.len().saturating_sub(1) as f64 * NODE_GAP * lab;
            let content_w = placed.rows.iter().map(|r| row_w(r)).fold(0.0f64, f64::max);
            let level_names: Vec<String> = (0..rows).map(level_name).collect();
            let level_w = if show_levels { level_names.iter().map(|s| text_w(s, level_fs)).fold(0.0f64, f64::max) } else { 0.0 };
            // (critic blocker and majors) A link that skips a level is routed outside the rows; a lane is reserved for
            // each of them, so every typed link is visible and the count line matches the picture.
            let span = |e: (usize, usize)| placed.level[e.1].saturating_sub(placed.level[e.0]);
            let long: Vec<usize> = (0..web.edges.len()).filter(|&e| span(web.edges[e]) > 1).collect();
            let lane_w = if long.is_empty() { 0.0 } else { 14.0 + long.len() as f64 * 12.0 * lab };
            let base_left = 20.0 + if show_levels { level_w + 16.0 } else { 0.0 };
            let w = (base_left + content_w + 20.0 + lane_w).max(min_w);
            // (critic minor 5) a narrow web is centred in the 420 px minimum, not left-aligned with a blank margin
            let slack = (w - base_left - content_w - 20.0 - lane_w).max(0.0);
            let left = base_left + slack / 2.0;
            let top: f64 = 28.0;
            let max_level = rows.saturating_sub(1);
            let cy = |l: usize| top + box_h / 2.0 + (max_level - l) as f64 * LEVEL_GAP * lab;
            let bottom = cy(0) + box_h / 2.0 + 18.0;
            let count = format!(
                "{} {}, {} {}, {} {}",
                web.names.len(),
                plural(web.names.len(), "name", "names"),
                web.edges.len(),
                plural(web.edges.len(), "link", "links"),
                rows,
                plural(rows, "trophic level", "trophic levels"),
            );
            let count_lines = wrap(&count, w - 48.0, 12.0 * lab);
            let h = bottom + (count_lines.len().max(1) as f64 * 17.0 + 8.0) * lab;
            let mut d = Doc::new(w, h);
            d.describe("Food web", "A food web from typed links: producers at the bottom, arrows to whatever eats them, trophic levels named");

            // each box's left edge
            let mut box_x = vec![0.0; web.names.len()];
            for row in placed.rows.iter() {
                let rw = row_w(row);
                let mut x = left + (content_w - rw) / 2.0;
                for &i in row {
                    box_x[i] = x;
                    x += widths[i] + NODE_GAP * lab;
                }
            }
            // (critic major 2) every link gets its own attachment on the box edge: outgoing slots spread along the top,
            // incoming slots along the bottom, so a predator with many prey does not become one black cluster.
            let mut top_frac = vec![0.5; web.edges.len()];
            let mut bot_frac = vec![0.5; web.edges.len()];
            for node in 0..web.names.len() {
                let outs: Vec<usize> = (0..web.edges.len()).filter(|&e| web.edges[e].0 == node).collect();
                for (k, &e) in outs.iter().enumerate() { top_frac[e] = (k as f64 + 0.5) / outs.len() as f64; }
                let ins: Vec<usize> = (0..web.edges.len()).filter(|&e| web.edges[e].1 == node).collect();
                for (k, &e) in ins.iter().enumerate() { bot_frac[e] = (k as f64 + 0.5) / ins.len() as f64; }
            }
            let head = |d: &mut Doc, x: f64, y: f64| {
                d.poly(&[(x, y), (x - normal_hl * 0.42, y + normal_hl), (x + normal_hl * 0.42, y + normal_hl)], edge_col, edge_col, 0.5);
            };
            // (verification round 4, LOW) reserving the head's clearance at BOTH ends of the gap wasted a third of
            // the packing space: only an arrival must clear an arrowhead, a departure only needs to clear its box.
            // Arrivals take the slots nearest the predator (top), departures the ones nearest the prey (bottom).
            let (clear, arrival_drop, _, height) = band(lab);
            let mut run_y = vec![(0.0, 0.0); web.edges.len()];
            let mut gaps: Vec<Vec<(usize, bool)>> = vec![Vec::new(); rows];
            for &e in &long {
                gaps[placed.level[web.edges[e].0]].push((e, true));
                gaps[placed.level[web.edges[e].1] - 1].push((e, false));
            }
            // (owner decision, 2026-10-29) a dense web must not ship with runs closer than a stroke and a visible
            // gap: it would read as links that do not exist. Refused like the 24-name cap, never drawn ambiguous.
            // The same test feeds the editor's note (`warnings`), so the canvas and the note area always agree.
            if crowded(&web, lab) {
                return refusal(
                    "The food web could not be drawn:",
                    "Too many links skip a trophic level to keep their runs apart.",
                    "Nothing is drawn until fewer links skip a trophic level.",
                    lab,
                );
            }
            for (g, list) in gaps.iter_mut().enumerate() {
                let n = list.len();
                if n == 0 { continue; }
                list.sort_by_key(|&(_, dep)| dep); // arrivals first, so they take the top slots
                let top = cy(g) - box_h / 2.0 - clear + arrival_drop;
                for (s, &(e, dep)) in list.iter().enumerate() {
                    let y = top + (s as f64 + 1.0) * height / (n as f64 + 1.0);
                    if dep { run_y[e].0 = y } else { run_y[e].1 = y }
                }
            }
            // the links first, then the boxes
            for (e, &(prey, predator)) in web.edges.iter().enumerate() {
                let px = box_x[prey] + top_frac[e] * widths[prey];
                let qx = box_x[predator] + bot_frac[e] * widths[predator];
                let py = cy(placed.level[prey]) - box_h / 2.0;
                let qy = cy(placed.level[predator]) + box_h / 2.0;
                if span((prey, predator)) <= 1 {
                    d.line(px, py, qx, qy, edge_col, EDGE_W * lab);
                    head(&mut d, qx, qy);
                } else {
                    // around the rows: out above the prey's row, up a lane right of every box, in below the predator's
                    // row — never straight through an unrelated box, which would hide the shaft (the blocker)
                    let k = long.iter().position(|&x| x == e).unwrap_or(0);
                    let lane = left + content_w + 14.0 + k as f64 * 12.0 * lab + 6.0 * lab;
                    let y0 = run_y[e].0;
                    let y1 = run_y[e].1;
                    d.polyline(&[(px, py), (px, y0), (lane, y0), (lane, y1), (qx, y1), (qx, qy)], edge_col, EDGE_W * lab);
                    head(&mut d, qx, qy);
                }
            }
            // the boxes, filled white so a link running behind one is not drawn through its name
            for (l, row) in placed.rows.iter().enumerate() {
                let rw = row_w(row);
                let mut x = left + (content_w - rw) / 2.0;
                for &i in row {
                    let (bw, bh) = (widths[i], box_h);
                    d.rect(x, cy(l) - bh / 2.0, bw, bh, "#ffffff", ink, 1.2);
                    d.text(x + bw / 2.0, cy(l) + fs * 0.35, fs, "middle", &web.names[i]);
                    x += bw + NODE_GAP * lab;
                }
            }
            if show_levels {
                for (l, name) in level_names.iter().enumerate() {
                    d.raw(&text_el(left - 10.0, cy(l) + level_fs * 0.35, level_fs, "end", ink, name));
                }
            }
            for (i, line) in count_lines.iter().enumerate() {
                d.raw(&text_el(24.0, bottom + (8.0 + i as f64 * 17.0) * lab, 12.0 * lab, "start", ink, line));
            }
            d.finish()
        }
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "A meadow", params: vec![s("links", "Grass -> Rabbit\nGrass -> Grasshopper\nGrasshopper -> Frog\nRabbit -> Fox\nFrog -> Fox")] },
        Example { name: "An arctic web", params: vec![s("links", "Lichens -> Lemming\nLichens -> Caribou\nLemming -> Arctic fox\nCaribou -> Wolf\nArctic fox -> Wolf")] },
        Example { name: "An omnivore", params: vec![s("links", "Grass -> Rabbit\nGrass -> Fox\nRabbit -> Fox")] },
        Example { name: "A chain on one line", params: vec![s("links", "Grass -> Grasshopper -> Frog -> Heron")] },
        Example { name: "Written from the predator's end", params: vec![s("links", "Fox <- Rabbit <- Grass\nGrass -> Grasshopper")] },
        Example { name: "With a note", params: vec![s("links", "# the pond\nAlgae -> Water flea\nWater flea -> Minnow\nMinnow -> Heron")] },
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
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
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
    fn the_parser_reads_links_chains_and_notes() {
        let w = parse("Grass -> Rabbit\nGrass -> Grasshopper\n# a note\n\nGrasshopper -> Frog").unwrap();
        assert_eq!(w.names, vec!["Grass", "Rabbit", "Grasshopper", "Frog"]);
        assert_eq!(w.edges, vec![(0, 1), (0, 2), (2, 3)]);
        // one line may hold a chain
        let chain = parse("Grass -> Grasshopper -> Frog").unwrap();
        assert_eq!(chain.edges, vec![(0, 1), (1, 2)]);
        assert_eq!(level_of(&chain), Some(vec![0, 1, 2]));
        // the same chain from the predator's end, and the unicode arrow
        assert_eq!(parse("Frog <- Grasshopper <- Grass").unwrap().edges, vec![(0, 1), (1, 2)]);
        assert_eq!(parse("Grass → Rabbit").unwrap().edges, vec![(0, 1)]);
        // a name may hold spaces
        assert_eq!(parse("Arctic fox -> Wolf").unwrap().names, vec!["Arctic fox", "Wolf"]);
        // a repeated link is one link
        assert_eq!(parse("A -> B\nA -> B").unwrap().edges, vec![(0, 1)]);
    }

    #[test]
    fn the_parser_refuses_what_is_not_a_food_web() {
        for bad in ["Grass", "Grass ->", "-> Rabbit", "Rabbit -> Rabbit", "A -> B\nB -> A"] {
            assert!(parse(bad).is_err(), "{bad:?} parsed");
        }
        let e = parse("A -> B\nB -> A").unwrap_err();
        assert!(e.why.contains("circle") && e.why.contains("A -> B -> A"), "{e}");
        assert!(parse("A -> B <- C").unwrap_err().why.contains("mixes"), "a line with both arrows is refused");
        let line = parse("Grass -> Rabbit\nnonsense").unwrap_err();
        assert!(line.why.contains("Line 2") && line.why.contains("no arrow"), "{line}");
    }

    #[test]
    fn the_levels_follow_the_longest_path() {
        let w = parse("Grass -> Rabbit\nGrass -> Fox\nRabbit -> Fox").unwrap();
        // the fox eats a producer and a primary consumer: it is a secondary consumer
        assert_eq!(level_of(&w), Some(vec![0, 1, 2]));
        let two = parse("Grass -> Rabbit\nGrass -> Grasshopper").unwrap();
        assert_eq!(level_of(&two), Some(vec![0, 1, 1]));
        let placed = place(&two);
        assert_eq!(placed.rows.len(), 2);
        assert_eq!(placed.rows[0], vec![0]);
    }

    #[test]
    fn every_name_and_link_is_drawn() {
        let svg = render_of(&[]);
        let t = texts(&svg);
        for name in ["Grass", "Rabbit", "Grasshopper", "Frog", "Fox"] { assert!(t.contains(name), "{name} missing"); }
        let default = with(&[]);
        let edges = parse(default.s("links")).map(|w| w.edges.len()).unwrap_or(0);
        assert_eq!(svg.matches("<polygon").count(), edges, "one arrowhead per link");
        assert_eq!(svg.matches("<rect").count(), 5 + 1, "one box per name, and the page's background");
        for level in ["Producers", "Primary consumers", "Secondary consumers"] { assert!(t.contains(level), "{level} missing"); }
    }

    #[test]
    fn the_level_names_can_be_turned_off() {
        assert!(texts(&render_of(&[])).contains("Producers"));
        assert!(!texts(&render_of(&[("levels", Value::B(false))])).contains("Producers"));
    }

    #[test]
    fn an_empty_web_says_what_to_type_and_a_bad_one_says_why() {
        let empty = texts(&render_of(&[s("links", "")]));
        assert!(empty.contains("Type one link per line"), "{empty}");
        assert!(empty.contains("Grass -&gt; Rabbit"), "the example is drawn escaped in the SVG: {empty}");
        let bad = texts(&render_of(&[s("links", "A -> B\nB -> A")]));
        assert!(bad.contains("could not be read") && bad.contains("circle"), "{bad}");
    }

    #[test]
    fn a_long_web_stays_inside_the_canvas() {
        let links = (1..=8).map(|i| format!("Grass -> Species name{i}")).collect::<Vec<_>>().join("\n");
        let svg = render_of(&[("links", Value::S(links))]);
        let h: f64 = {
            let at = svg.find("height=\"").expect("height") + 8;
            svg[at..].split('"').next().unwrap().parse().unwrap()
        };
        let w: f64 = {
            let at = svg.find("width=\"").expect("width") + 7;
            svg[at..].split('"').next().unwrap().parse().unwrap()
        };
        for el in svg.split("<text").skip(1) {
            let num = |k: &str| -> f64 {
                let at = el.find(&format!("{k}=\"")) .unwrap_or(0) + k.len() + 2;
                el.get(at..).and_then(|r| r.split('"').next()).and_then(|v| v.parse().ok()).unwrap_or(0.0)
            };
            let (x, y, size) = (num("x"), num("y"), num("font-size"));
            assert!(y <= h, "text below the canvas: {el}");
            if el.contains("text-anchor=\"middle\"") { assert!(x <= w, "a box is off the canvas: {el}"); }
            let _ = size;
        }
    }

    #[test]
    fn a_valid_web_draws_no_hue() {
        // the Style control is gone: this figure is monochrome in its one output, so it prints as it stands
        let svg = render_of(&[]);
        for attr in ["fill=\"", "stroke=\""] {
            for part in svg.split(attr).skip(1) {
                let c = part.split('"').next().unwrap_or("");
                let hex = c.trim_start_matches('#');
                let byte = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
                let (r, g, b) = match hex.len() {
                    3 => (byte(&hex[0..1]) * 17, byte(&hex[1..2]) * 17, byte(&hex[2..3]) * 17),
                    6 => (byte(&hex[0..2]), byte(&hex[2..4]), byte(&hex[4..6])),
                    _ => continue,
                };
                assert!(r == g && g == b, "a valid web draws a hue: {c}");
            }
        }
    }

    /// Every drawn shaft as (x1, y1, x2, y2).
    fn segs(svg: &str) -> Vec<(f64, f64, f64, f64)> {
        svg.split("<line ").skip(1).map(|part| {
            let num = |k: &str| -> f64 {
                let at = part.find(&format!("{k}=\"")).expect("attribute") + k.len() + 2;
                part[at..].split('"').next().unwrap().parse().unwrap()
            };
            (num("x1"), num("y1"), num("x2"), num("y2"))
        }).collect()
    }

    /// Every routed shaft as its points.
    fn routes(svg: &str) -> Vec<Vec<(f64, f64)>> {
        svg.split("<polyline ").skip(1).map(|part| {
            let at = part.find("points=\"").expect("points") + 8;
            part[at..].split('"').next().unwrap().split_whitespace().map(|p| {
                let (x, y) = p.split_once(',').expect("x,y");
                (x.parse().unwrap(), y.parse().unwrap())
            }).collect()
        }).collect()
    }

    #[test]
    fn a_link_that_skips_a_level_is_routed_and_never_hidden() {
        // (critic blocker) The omnivore's Grass -> Fox runs straight through Rabbit's box and is hidden; the picture
        // then shows a plain chain while the count line says three links.
        let svg = render_of(&[s("links", "Grass -> Rabbit\nGrass -> Fox\nRabbit -> Fox")]);
        let rs = routes(&svg);
        assert_eq!(rs.len(), 1, "the link that skips a level must be routed around the rows, not drawn straight");
        let ls = segs(&svg);
        let mut starts: Vec<(i64, i64)> = ls.iter().map(|l| (l.0 as i64, l.1 as i64)).chain(rs.iter().map(|p| (p[0].0 as i64, p[0].1 as i64))).collect();
        starts.sort();
        starts.dedup();
        assert_eq!(starts.len(), 3, "two links leave the same point: {starts:?}");
        let mut ends: Vec<(i64, i64)> = ls.iter().map(|l| (l.2 as i64, l.3 as i64)).chain(rs.iter().map(|p| (p[p.len() - 1].0 as i64, p[p.len() - 1].1 as i64))).collect();
        ends.sort();
        ends.dedup();
        assert_eq!(ends.len(), 3, "two links meet the same point: {ends:?}");
    }

    #[test]
    fn a_predator_with_many_prey_gets_distinct_arrivals() {
        // (critic major) Eight arrowheads stacked on one point under the apex.
        let links = (1..=8).map(|i| format!("Grass -> Prey{i}")).collect::<Vec<_>>().join("\n") + "\n" + &(1..=8).map(|i| format!("Prey{i} -> Apex")).collect::<Vec<_>>().join("\n");
        let svg = render_of(&[("links", Value::S(links))]);
        let ls = segs(&svg);
        let arrivals: Vec<i64> = ls.iter().filter(|l| l.3 < l.1).map(|(_x1, _y1, x2, _y2)| *x2 as i64).collect();
        let mut unique = arrivals.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), arrivals.len(), "arrowheads share an arrival point: {arrivals:?}");
    }

    #[test]
    fn a_departure_and_an_arrival_never_share_a_run_height() {
        // (verification round 3, MED) one global stagger let X1->P2's arrival and P1->P3's departure sit 1 px apart
        // in the same gap and overlap; every run in a gap now takes its own slot. (round 4, minor: sweep every label
        // size and hold to the real stroke, not a looser bound.)
        let links = "P0 -> P1\nP1 -> P2\nP2 -> P3\nX0 -> P2\nX1 -> P2\nX2 -> P3\nP1 -> P3";
        for (size, stroke) in [("Small", 1.2), ("Medium", 1.5), ("Large", 2.1)] {
            let svg = render_of(&[s("links", links), s("label_size", size)]);
            let mut runs: Vec<(f64, f64, f64)> = Vec::new(); // y, x0, x1
            for p in routes(&svg) {
                runs.push((p[1].1, p[0].0.min(p[2].0), p[0].0.max(p[2].0)));
                runs.push((p[3].1, p[2].0.min(p[4].0), p[2].0.max(p[4].0)));
            }
            assert_eq!(runs.len(), 8, "{size}: four long links have eight runs");
            for i in 0..runs.len() {
                for j in (i + 1)..runs.len() {
                    let (y1, a0, a1) = runs[i];
                    let (y2, b0, b1) = runs[j];
                    let overlap = a1.min(b1) - a0.max(b0);
                    // the figure itself refuses below a stroke plus one visible pixel, so a drawn web holds to that
                    assert!(!((y1 - y2).abs() < stroke + 1.0 - 0.01 && overlap > 0.5), "{size}: two runs too close: y {y1} and {y2}, x overlap {overlap}");
                }
            }
        }
    }

    #[test]
    fn a_web_too_dense_to_draw_is_refused_not_banded() {
        // (owner decision 2026-10-29) twelve long links through one gap would leave their runs closer than a stroke
        // plus a visible gap; the figure refuses with a reason, like the 24-name cap, rather than draw a picture
        // that can be misread as links that do not exist
        let mut links = String::new();
        for i in 0..12 { links.push_str(&format!("A{i} -> B\nA{i} -> C\n")); }
        links.push_str("B -> C\n");
        let svg = render_of(&[s("links", &links)]);
        let t = texts(&svg);
        assert!(t.contains("could not be drawn"), "{t}");
        assert!(t.contains("fewer links skip a trophic level"), "{t}");
        assert_eq!(svg.matches("<rect").count(), 1, "only the page background is drawn");
        let ok = (0..8).map(|i| format!("A{i} -> B\nA{i} -> C")).collect::<Vec<_>>().join("\n") + "\nB -> C\n";
        let dr = render_of(&[("links", Value::S(ok))]);
        assert!(dr.matches("<rect").count() > 1, "eight long links fit in a gap and must draw");
    }

    #[test]
    fn the_density_limit_sits_where_the_notes_say() {
        // n long links A_i -> C put n departures in one gap and n arrivals in the next. Refused from 10 runs at
        // Small, 12 at Medium and 15 at Large (band / (n + 1) < stroke + 1), drawn one run below that.
        let web = |n: usize| (0..n).map(|i| format!("A{i} -> B\nA{i} -> C")).collect::<Vec<_>>().join("\n") + "\nB -> C\n";
        for (size, first_refused) in [("Small", 10), ("Medium", 12), ("Large", 15)] {
            let drawn = render_of(&[("links", Value::S(web(first_refused - 1))), s("label_size", size)]);
            assert!(drawn.matches("<rect").count() > 1, "{size}: {} long links must draw", first_refused - 1);
            let refused = render_of(&[("links", Value::S(web(first_refused))), s("label_size", size)]);
            assert_eq!(refused.matches("<rect").count(), 1, "{size}: {first_refused} long links must be refused");
            // (verification round 5, MED) the editor's note area must agree with the canvas, both ways
            let note = |n: usize| crate::figures::notes_for(ID, &with(&[("links", Value::S(web(n))), s("label_size", size)]));
            assert!(note(first_refused).iter().any(|w| w.contains("could not be drawn")), "{size}: no editor note for a refused web");
            assert!(note(first_refused - 1).is_empty(), "{size}: a note for a web that draws");
        }
    }

    #[test]
    fn a_narrow_predator_does_not_merge_its_heads() {
        // (verification critic, MED) `X` as typed is only 29 px wide, so its eight arrivals were ~3.3 px apart while a
        // head base is ~7 px: the heads merged into a band. The box now widens to hold its slots, full-size.
        let links = (1..=8).map(|i| format!("Grass -> Prey{i}")).collect::<Vec<_>>().join("\n") + "\n" + &(1..=8).map(|i| format!("Prey{i} -> X")).collect::<Vec<_>>().join("\n");
        let svg = render_of(&[("links", Value::S(links))]);
        let heads: Vec<(f64, f64, f64)> = svg.split("<polygon ").skip(1).filter_map(|part| {
            let at = part.find("points=\"")? + 8;
            let pts: Vec<(f64, f64)> = part[at..].split('"').next()?.split_whitespace().map(|p| {
                let (x, y) = p.split_once(',')?;
                Some((x.parse().ok()?, y.parse().ok()?))
            }).collect::<Option<Vec<_>>>()?;
            let (minx, maxx) = pts.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
            Some((pts[0].0, pts[0].1, maxx - minx))
        }).collect();
        assert_eq!(heads.len(), 16, "one head per link");
        let mut rows: std::collections::BTreeMap<i64, Vec<(f64, f64)>> = std::collections::BTreeMap::new();
        for (cx, y, w) in heads { rows.entry((y * 100.0).round() as i64).or_default().push((cx, w)); }
        for (_, mut v) in rows {
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            for pair in v.windows(2) {
                let gap = pair[1].0 - pair[0].0;
                let need = (pair[0].1 + pair[1].1) / 2.0;
                assert!(gap >= need - 0.01, "two heads overlap: centres {gap:.2} px apart, bases need {need:.2}");
            }
        }
    }

    #[test]
    fn the_second_error_line_matches_the_refusal() {
        // (verification round 2) one generic sentence was false for the name cap and misleading for mixed arrows
        let many = (1..=25).map(|i| format!("N{i} -> M{i}")).collect::<Vec<_>>().join("\n");
        let over = texts(&render_of(&[("links", Value::S(many))]));
        assert!(over.contains("cannot be read"), "{over}");
        assert!(!over.contains("one-way web"), "the 24-name cap is not a cycle: {over}");
        let cycle = texts(&render_of(&[s("links", "A -> B\nB -> A")]));
        assert!(cycle.contains("one-way web"), "{cycle}");
        let mixed = texts(&render_of(&[s("links", "A -> B <- C")]));
        assert!(mixed.contains("one direction"), "{mixed}");
        let empty = texts(&render_of(&[s("links", "A ->")]));
        assert!(empty.contains("unnamed organism cannot be placed"), "{empty}");
    }

    #[test]
    fn a_name_that_looks_like_an_error_keeps_the_right_second_line() {
        // (verification round 2, LOW) the sentence must come from the refusal kind, not from words in the name
        let t = texts(&render_of(&[s("links", "mixes -> mixes")]));
        assert!(t.contains("no name eats itself"), "{t}");
        assert!(!t.contains("one direction"), "{t}");
        let c = texts(&render_of(&[s("links", "run in a circle -> run in a circle")]));
        assert!(!c.contains("one-way web"), "a self-eat is not a cycle: {c}");
    }

    #[test]
    fn two_disjoint_routed_links_do_not_share_a_run() {
        // (verification round 2, MED) both links ran their horizontal segments along one y and overlapped, so the
        // picture read as {A1,A2}x{C1,C2}; each long link must get its own run height.
        let svg = render_of(&[s("links", "A1 -> B1\nB1 -> C1\nA1 -> C1\nA2 -> B2\nB2 -> C2\nA2 -> C2")]);
        let rs = routes(&svg);
        assert_eq!(rs.len(), 2, "two links skip a level");
        let mut y0s: Vec<i64> = rs.iter().map(|p| (p[1].1 * 100.0).round() as i64).collect();
        let mut y1s: Vec<i64> = rs.iter().map(|p| (p[3].1 * 100.0).round() as i64).collect();
        y0s.sort();
        y0s.dedup();
        y1s.sort();
        y1s.dedup();
        assert_eq!(y0s.len(), 2, "the departures run along one y: {y0s:?}");
        assert_eq!(y1s.len(), 2, "the arrivals run along one y: {y1s:?}");
    }

    #[test]
    fn the_left_arrow_and_tidied_spacing_are_read() {
        assert_eq!(parse("Fox ← Rabbit").unwrap().edges, vec![(0, 1)]);
        // runs of whitespace inside a name are one space, so a double space does not make a second organism
        assert_eq!(parse("Arctic  fox -> Wolf").unwrap().names, vec!["Arctic fox", "Wolf"]);
    }

    #[test]
    fn an_unbroken_long_name_stays_inside_the_error_canvas() {
        // (verification critic, LOW-5) the old test only checked each line's start x, which is always 24; assert the
        // wrapped line's measured width instead. (CJK is still under-measured by text_w — a recorded leftover.)
        let long = "x".repeat(300);
        let max = 420.0 - 48.0;
        for line in wrap(&long, max, 13.0) {
            assert!(text_w(&line, 13.0) <= max + 0.01, "a wrapped line is wider than its reserve: {line:?}");
        }
        let svg = render_of(&[("links", Value::S(format!("{long} -> {long}")))]);
        let w: f64 = {
            let at = svg.find("width=\"").expect("width") + 7;
            svg[at..].split('"').next().unwrap().parse().unwrap()
        };
        for el in svg.split("<text").skip(1) {
            let at = el.find("x=\"").unwrap_or(0) + 3;
            let x: f64 = el.get(at..).and_then(|r| r.split('"').next()).and_then(|v| v.parse().ok()).unwrap_or(0.0);
            assert!(x < w, "a text starts outside the canvas: {el}");
        }
    }

    #[test]
    fn the_editor_reports_a_parse_error() {
        let p = with(&[s("links", "Grass -> Rabbit\nnonsense")]);
        assert!(warnings(&p).iter().any(|w| w.contains("no arrow")), "{:?}", warnings(&p));
        assert!(crate::figures::notes_for(ID, &p).iter().any(|w| w.contains("no arrow")), "the Figures tab's note area never shows it: {:?}", crate::figures::notes_for(ID, &p));
    }

    /// Writes every case, and a copy reduced to 66% grey (the print check), to $VERSHION_PNG_DIR.
    #[test]
    fn food_web_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]),
            ("large".into(), vec![s("label_size", "Large")]),
            ("levels-off".into(), vec![("levels", Value::B(false))]),
            ("chain".into(), vec![s("links", "Grass -> Grasshopper -> Frog -> Heron")]),
            ("reversed".into(), vec![s("links", "Fox <- Rabbit <- Grass\nGrass -> Grasshopper")]),
            ("fanin".into(), vec![s("links", "Grass -> Prey1\nGrass -> Prey2\nGrass -> Prey3\nGrass -> Prey4\nGrass -> Prey5\nGrass -> Prey6\nGrass -> Prey7\nGrass -> Prey8\nPrey1 -> Apex\nPrey2 -> Apex\nPrey3 -> Apex\nPrey4 -> Apex\nPrey5 -> Apex\nPrey6 -> Apex\nPrey7 -> Apex\nPrey8 -> Apex")]),
            ("fanin-narrow".into(), vec![s("links", "Grass -> Prey1\nGrass -> Prey2\nGrass -> Prey3\nGrass -> Prey4\nGrass -> Prey5\nGrass -> Prey6\nGrass -> Prey7\nGrass -> Prey8\nPrey1 -> X\nPrey2 -> X\nPrey3 -> X\nPrey4 -> X\nPrey5 -> X\nPrey6 -> X\nPrey7 -> X\nPrey8 -> X")]),
            ("two-omni".into(), vec![s("links", "A1 -> B1\nB1 -> C1\nA1 -> C1\nA2 -> B2\nB2 -> C2\nA2 -> C2")]),
            ("gap-clash".into(), vec![s("links", "P0 -> P1\nP1 -> P2\nP2 -> P3\nX0 -> P2\nX1 -> P2\nX2 -> P3\nP1 -> P3")]),
            ("dense-refused".into(), { let mut l = String::new(); for i in 0..12 { l.push_str(&format!("A{i} -> B\nA{i} -> C\n")); } l.push_str("B -> C\n"); vec![("links", Value::S(l))] }),
            ("dense-refused-small".into(), { let mut l = String::new(); for i in 0..10 { l.push_str(&format!("A{i} -> B\nA{i} -> C\n")); } l.push_str("B -> C\n"); vec![("links", Value::S(l)), s("label_size", "Small")] }),
            ("dense-refused-large".into(), { let mut l = String::new(); for i in 0..15 { l.push_str(&format!("A{i} -> B\nA{i} -> C\n")); } l.push_str("B -> C\n"); vec![("links", Value::S(l)), s("label_size", "Large")] }),
            ("empty".into(), vec![s("links", "")]),
            ("cycle".into(), vec![s("links", "A -> B\nB -> A")]),
        ];
        for (idx, e) in examples(ID).unwrap().into_iter().enumerate() { cases.push((format!("ex{idx}"), e.params)); }
        for (tag, list) in cases {
            let svg = render_of(&list);
            std::fs::write(format!("{dir}/foodweb-{tag}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
            let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
            let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
            image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/foodweb-{tag}-bw66.png")).unwrap();
        }
    }
}
