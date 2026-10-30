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

/// Reads the typed links. Blank lines and `#` notes are skipped; `->` and `→` point from prey to predator, `<-`
/// the other way; a chain may be written on one line. Everything else is a name, spaces included.
pub fn parse(text: &str) -> Result<Web, String> {
    let mut names: Vec<String> = Vec::new();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let at = n + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let line = line.replace('→', "->").replace('←', "<-");
        let (fwd, back) = (line.matches("->").count(), line.matches("<-").count());
        if fwd > 0 && back > 0 {
            return Err(format!("Line {at} mixes -> and <-; use one direction on a line."));
        }
        if fwd == 0 && back == 0 {
            return Err(format!("Line {at} has no arrow: write Prey -> Predator."));
        }
        let mut parts: Vec<String> = if fwd > 0 {
            line.split("->").map(tidy).collect()
        } else {
            let mut v: Vec<String> = line.split("<-").map(tidy).collect();
            v.reverse();
            v
        };
        if parts.iter().any(String::is_empty) {
            return Err(format!("Line {at} has an empty name: put a name on both sides of every arrow."));
        }
        if parts.len() == 1 {
            return Err(format!("Line {at} has no arrow: write Prey -> Predator."));
        }
        for pair in parts.windows(2) {
            let prey = index_of(&mut names, &pair[0], at)?;
            let predator = index_of(&mut names, &pair[1], at)?;
            if prey == predator {
                return Err(format!("Line {at}: {} cannot eat itself.", pair[0]));
            }
            if !edges.contains(&(prey, predator)) {
                edges.push((prey, predator));
            }
        }
    }
    if let Some(cycle) = cycle(&names.len(), &edges) {
        let path = cycle.iter().map(|&i| names[i].as_str()).collect::<Vec<_>>().join(" -> ");
        return Err(format!("A food web cannot run in a circle: {path}. Remove one of those links."));
    }
    Ok(Web { names, edges })
}

/// A name as typed, tidied: one space for any run of whitespace, none at the ends. So `Arctic  fox` is the same
/// organism as `Arctic fox`, while a double space can not make a second one.
fn tidy(s: &str) -> String { s.split_whitespace().collect::<Vec<_>>().join(" ") }

fn index_of(names: &mut Vec<String>, name: &str, line: usize) -> Result<usize, String> {
    if let Some(i) = names.iter().position(|n| n == name) { return Ok(i); }
    if names.len() >= MAX_NAMES {
        return Err(format!("More than {MAX_NAMES} names (line {line}): split the web into two pictures."));
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

pub fn warnings(p: &Params) -> Vec<String> {
    match parse(p.s("links")) {
        Ok(w) if w.is_empty() => Vec::new(),
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
            let mut cut = rest.len();
            for (i, _) in rest.char_indices() {
                if text_w(&rest[..i], size) > max {
                    cut = i;
                    break;
                }
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

pub fn build(p: &Params) -> String {
    let lab = svg::label_factor(p.s("label_size"));
    let (ink, edge_col, hint_col) = (INK, EDGE, ERR);
    let fs = NODE_FS * lab;
    let level_fs = LEVEL_FS * lab;
    let show_levels = p.b("levels");
    let box_h = fs + 2.0 * PAD_Y * lab;
    let min_w: f64 = 420.0;

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
        Err(e) => {
            let reason = "The food web could not be read:";
            let max = min_w - 48.0;
            let first = wrap(&e, max, 13.0 * lab);
            // (critic minor 11) every line IS a link in a cycle: say what is actually missing
            let then = wrap("Nothing is drawn until the links form a one-way web.", max, 12.0 * lab);
            let h = 34.0 + (first.len() as f64 + 1.0 + then.len() as f64) * 18.0 * lab + 20.0;
            let mut d = Doc::new(min_w, h);
            d.describe("Food web", "A food web from typed links");
            d.raw(&text_el(24.0, 40.0, 13.0 * lab, "start", hint_col, reason));
            let mut y = 64.0 * lab;
            for line in &first {
                d.raw(&text_el(24.0, y, 13.0 * lab, "start", ink, line));
                y += 18.0 * lab;
            }
            y += 6.0 * lab;
            for line in &then {
                d.raw(&text_el(24.0, y, 12.0 * lab, "start", ink, line));
                y += 16.0 * lab;
            }
            return d.finish();
        }
        Ok(web) => {
            let placed = place(&web);
            let widths: Vec<f64> = web.names.iter().map(|n| text_w(n, fs) + 2.0 * PAD_X * lab).collect();
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
            for (l, row) in placed.rows.iter().enumerate() {
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
                for (k, &e) in outs.iter().enumerate() { top_frac[e] = (k as f64 + 1.0) / (outs.len() as f64 + 1.0); }
                let ins: Vec<usize> = (0..web.edges.len()).filter(|&e| web.edges[e].1 == node).collect();
                for (k, &e) in ins.iter().enumerate() { bot_frac[e] = (k as f64 + 1.0) / (ins.len() as f64 + 1.0); }
            }
            let head = |d: &mut Doc, x: f64, y: f64| {
                let hl = (EDGE_W * lab * 4.0).max(8.0 * lab);
                d.poly(&[(x, y), (x - hl * 0.42, y + hl), (x + hl * 0.42, y + hl)], edge_col, edge_col, 0.5);
            };
            // the links first, then the boxes
            for (e, &(prey, predator)) in web.edges.iter().enumerate() {
                let px = box_x[prey] + top_frac[e] * widths[prey];
                let qx = box_x[predator] + bot_frac[e] * widths[predator];
                let py = cy(placed.level[prey]) - box_h / 2.0;
                let qy = cy(placed.level[predator]) + box_h / 2.0;
                if span((prey, predator)) <= 1 {
                    d.arrow(px, py, qx, qy, edge_col, EDGE_W * lab);
                } else {
                    // around the rows: out above the prey's row, up a lane right of every box, in below the predator's
                    // row — never straight through an unrelated box, which would hide the shaft (the blocker)
                    let k = long.iter().position(|&x| x == e).unwrap_or(0);
                    let lane = left + content_w + 14.0 + k as f64 * 12.0 * lab + 6.0 * lab;
                    let y0 = py - 8.0 * lab;
                    let y1 = qy + 8.0 * lab;
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
        assert!(e.contains("circle") && e.contains("A -> B -> A"), "{e}");
        assert!(parse("A -> B <- C").unwrap_err().contains("mixes"), "a line with both arrows is refused");
        let line = parse("Grass -> Rabbit\nnonsense").unwrap_err();
        assert!(line.contains("Line 2") && line.contains("no arrow"), "{line}");
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
    fn print_draws_no_hue() {
        let svg = render_of(&[s("style", "Print")]);
        for part in svg.split("fill=\"").skip(1) {
            let c = part.split('"').next().unwrap_or("");
            let hex = c.trim_start_matches('#');
            let byte = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
            let (r, g, b) = match hex.len() {
                3 => (byte(&hex[0..1]) * 17, byte(&hex[1..2]) * 17, byte(&hex[2..3]) * 17),
                6 => (byte(&hex[0..2]), byte(&hex[2..4]), byte(&hex[4..6])),
                _ => continue,
            };
            assert!(r == g && g == b, "Print draws a hue: {c}");
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
    fn the_left_arrow_and_tidied_spacing_are_read() {
        assert_eq!(parse("Fox ← Rabbit").unwrap().edges, vec![(0, 1)]);
        // runs of whitespace inside a name are one space, so a double space does not make a second organism
        assert_eq!(parse("Arctic  fox -> Wolf").unwrap().names, vec!["Arctic fox", "Wolf"]);
    }

    #[test]
    fn an_unbroken_long_name_stays_inside_the_error_canvas() {
        let long = "x".repeat(300);
        let svg = render_of(&[("links", Value::S(format!("{long} -> {long}")))]);
        let w: f64 = {
            let at = svg.find("width=\"").expect("width") + 7;
            svg[at..].split('"').next().unwrap().parse().unwrap()
        };
        for el in svg.split("<text").skip(1) {
            let at = el.find("x=\"") .unwrap_or(0) + 3;
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

    /// Writes every case (Colour and Print, Small and Large) to $VERSHION_PNG_DIR.
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
            ("empty".into(), vec![s("links", "")]),
            ("cycle".into(), vec![s("links", "A -> B\nB -> A")]),
        ];
        for (idx, e) in examples(ID).unwrap().into_iter().enumerate() { cases.push((format!("ex{idx}"), e.params)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/foodweb-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/foodweb-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
