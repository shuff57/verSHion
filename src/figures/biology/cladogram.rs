//! A cladogram from a typed tree: Newick-shaped text such as `(((Human, Chimp), Gorilla), Orangutan)`, drawn as a
//! rectangular or slanted tree with the leaves in the order they were typed. Drawn with `svg::Doc` only.
//!
//! The tree is the figure, so there is nothing to hide: Show answers has no effect and the figure does not offer it.
use super::*;

pub const ID: &str = "cladogram";

const INK: &str = "#111111";
const BRANCH: &str = "#333333";
const LEAF: &str = "#1f5fbf";
const ERR: &str = "#c0392b";
const LEAF_GAP: f64 = 26.0;
const STEP_X: f64 = 92.0;
/// What the page says under a tree it could not read.
const NO_TREE: &str = "Nothing is drawn until the brackets balance.";
/// The shared text cap (`Params::normalize` cuts a `Kind::Text` value to 500 characters). A tree longer than this is
/// cut mid-name before it reaches the parser, so the figure cannot blame the brackets — it has to say the tree is too
/// long. (round 6 minor)
const MAX_TREE_CHARS: usize = 500;
const TOO_LONG: &str = "The box holds at most 500 characters. Shorten the tree, or split it into smaller ones.";

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Cladogram",
        blurb: "A tree from typed text: type ((A, B), C) and get the cladogram, rectangular or slanted, with the leaves in the order you typed them.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("tree", "Tree", "(((Human, Chimp), Gorilla), Orangutan)")
            .hint("Nested brackets, comma separated: ((A, B), C). A name after a bracket labels that ancestor."),
        ParamDef::choice("shape", "Shape", &["rectangular", "slanted"], "rectangular"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p) }

/// A node of the tree: a name (possibly empty) and its children. A leaf has no children.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub label: String,
    pub children: Vec<Node>,
}

impl Node {
    pub fn leaf(name: &str) -> Node { Node { label: name.to_string(), children: Vec::new() } }
    pub fn is_leaf(&self) -> bool { self.children.is_empty() }
}

/// Parses Newick-shaped text: `(A, B)`, `((A, B)C, D)`, with an optional trailing `;`. Whitespace around brackets and
/// commas is skipped; a name may contain spaces (`Homo sapiens`, `Great apes`), but not a bracket, comma or semicolon.
pub fn parse(s: &str) -> Result<Node, String> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    skip_ws(&chars, &mut i);
    let node = subtree(&chars, &mut i)?;
    skip_ws(&chars, &mut i);
    if i < chars.len() && chars[i] == ';' { i += 1; }
    skip_ws(&chars, &mut i);
    if i < chars.len() { return Err(format!("Unexpected '{}' after the end of the tree.", chars[i])); }
    Ok(node)
}

fn skip_ws(chars: &[char], i: &mut usize) {
    while *i < chars.len() && chars[*i].is_whitespace() { *i += 1; }
}

fn name(chars: &[char], i: &mut usize) -> String {
    let start = *i;
    while *i < chars.len() && !matches!(chars[*i], '(' | ')' | ',' | ';') { *i += 1; }
    chars[start..*i].iter().collect::<String>().trim().to_string()
}

fn subtree(chars: &[char], i: &mut usize) -> Result<Node, String> {
    skip_ws(chars, i);
    if *i >= chars.len() { return Err("The tree ends where a name was expected.".to_string()); }
    if chars[*i] != '(' {
        let label = name(chars, i);
        if label.is_empty() { return Err("A bracket, comma or semicolon appeared where a name was expected.".to_string()); }
        return Ok(Node::leaf(&label));
    }
    *i += 1; // '('
    skip_ws(chars, i);
    if *i < chars.len() && chars[*i] == ')' {
        return Err("A pair of brackets with nothing inside them is not a group.".to_string());
    }
    let mut children = vec![subtree(chars, i)?];
    loop {
        skip_ws(chars, i);
        if *i >= chars.len() { return Err("A bracket is never closed.".to_string()); }
        match chars[*i] {
            ',' => { *i += 1; children.push(subtree(chars, i)?); }
            ')' => {
                *i += 1;
                let label = name(chars, i);
                return Ok(Node { label, children });
            }
            c => return Err(format!("Expected ',' or ')' but found '{c}'.")),
        }
    }
}

/// The leaves in the order they appear, left to right.
pub fn leaves(n: &Node) -> Vec<String> {
    let mut out = Vec::new();
    collect(n, &mut out);
    out
}

fn collect(n: &Node, out: &mut Vec<String>) {
    if n.is_leaf() { out.push(n.label.clone()); } else { for c in &n.children { collect(c, out); } }
}

/// The names typed after a bracket: every non-empty label on a branching point (a leaf's label is its own name).
pub fn ancestor_labels(n: &Node) -> Vec<String> {
    let mut out = Vec::new();
    fn go(n: &Node, out: &mut Vec<String>) {
        if n.is_leaf() { return; }
        if !n.label.is_empty() { out.push(n.label.clone()); }
        for c in &n.children { go(c, out); }
    }
    go(n, &mut out);
    out
}

/// The height of a node: 0 for a leaf, otherwise one more than its tallest child.
pub fn height(n: &Node) -> usize {
    if n.is_leaf() { 0 } else { 1 + n.children.iter().map(height).max().unwrap_or(0) }
}

/// (round 2) The branching points a reader can see: a group of two or more names. A one-child group branches nothing,
/// though the parser accepts it (`(A)` is a name in brackets, not a fork), so counting every inner node said
/// "1 branching point" for a single stub.
pub fn branch_points(n: &Node) -> usize {
    if n.is_leaf() { 0 } else { usize::from(n.children.len() >= 2) + n.children.iter().map(branch_points).sum::<usize>() }
}

/// Every inner node, for the tests that count the bars drawn (a one-child node's bar is zero-length and not drawn).
#[cfg(test)]
fn count_inner(n: &Node) -> usize {
    if n.is_leaf() { 0 } else { 1 + n.children.iter().map(count_inner).sum::<usize>() }
}

/// Everything needed to draw: the leaves with their (depth, row), the branches as (parent depth, child row,
/// child depth), the vertical bar of each branching point as (depth, first row, last row), and the ancestors that were
/// labelled in the text as (label, depth, row).
pub struct Placed {
    pub leaves: Vec<(String, usize, f64)>,
    pub edges: Vec<(usize, f64, usize, f64)>,
    pub bars: Vec<(usize, f64, f64)>,
    pub nodes: Vec<(String, usize, f64)>,
}

pub fn place(root: &Node) -> Placed {
    let maxd = height(root);
    let mut p = Placed { leaves: Vec::new(), edges: Vec::new(), bars: Vec::new(), nodes: Vec::new() };
    let mut row = 0usize;
    walk(root, maxd, &mut row, &mut p);
    p
}

fn walk(n: &Node, maxd: usize, row: &mut usize, p: &mut Placed) -> (usize, f64) {
    let x = maxd - height(n);
    if n.is_leaf() {
        let y = *row as f64;
        *row += 1;
        p.leaves.push((n.label.clone(), x, y));
        return (x, y);
    }
    // the branch starts at THIS node's height, which is only known once every child has been walked — so the
    // children come back first and the edges are recorded afterwards. Recording them with the child's height at both
    // ends drew every diagonal flat.
    let mut kids = Vec::new();
    for c in &n.children {
        let (cx, cy) = walk(c, maxd, row, p);
        kids.push((cx, cy));
    }
    let (first, last) = (kids.first().map(|k| k.1).unwrap_or(0.0), kids.last().map(|k| k.1).unwrap_or(0.0));
    let y = (first + last) / 2.0;
    for (cx, cy) in kids { p.edges.push((x, y, cx, cy)); }
    p.bars.push((x, first, last));
    // (round 2) a name typed after a bracket names that ancestor: it is drawn at its branching point, or the text and
    // its own hint would promise a label the picture never shows
    if !n.label.is_empty() { p.nodes.push((n.label.clone(), x, y)); }
    (x, y)
}

/// A string's width at `size` px, summed per glyph: a run of wide capitals is nearly twice the width of the same count
/// of lower-case letters, and measuring by character count clipped `(WWWW…)` off the canvas (round 5).
pub fn text_w(s: &str, size: f64) -> f64 {
    s.chars().map(|c| size * glyph_em(c)).sum()
}

/// (round 6) The width of one glyph in em. The Latin table is unchanged; CJK ideographs, kana, Hangul and full-width
/// forms draw at about one em, and emoji wider still, so measuring a Japanese name at the 0.56 em of an unknown Latin
/// glyph made the canvas ~45 % too narrow and clipped both the leaf and the "Left to right:" line.
fn glyph_em(c: char) -> f64 {
    match c {
        'W' | 'M' | 'm' | 'w' => 0.95,
        'A'..='Z' => 0.72,
        '0'..='9' => 0.58,
        ' ' => 0.28,
        'i' | 'j' | 'l' | 't' | 'f' | 'r' | '.' | ',' | ';' | ':' | '\'' | '!' | '|' => 0.32,
        '\u{1100}'..='\u{11ff}'          // Hangul Jamo
        | '\u{2e80}'..='\u{2fdf}'        // CJK radicals, Kangxi radicals
        | '\u{3000}'..='\u{303f}'        // CJK punctuation
        | '\u{3040}'..='\u{30ff}'        // Hiragana and Katakana
        | '\u{3400}'..='\u{4dbf}'        // CJK Extension A
        | '\u{4e00}'..='\u{9fff}'        // CJK Unified Ideographs
        | '\u{a960}'..='\u{a97f}'        // Hangul Jamo Extended-A
        | '\u{ac00}'..='\u{d7ff}'        // Hangul syllables
        | '\u{f900}'..='\u{faff}'        // CJK compatibility ideographs
        | '\u{fe30}'..='\u{fe4f}'        // CJK compatibility forms
        | '\u{ff00}'..='\u{ffef}' => 1.0, // full-width forms
        _ if c as u32 >= 0x1f000 => 1.15, // emoji and pictographs
        _ => 0.56,
    }
}

/// (round 7 major) The script a glyph belongs to, for splitting a line into `<text>` runs. The PNG rasteriser draws
/// each `<text>` with one font run and falls back to `.notdef` (tofu) when a run mixes scripts it cannot resolve
/// together — a "Left to right:" line holding both a Han and a Hangul name printed "□□□□□□, □□□□□". Latin, Han/kana,
/// Hangul and emoji get their own classes so a mixed line is drawn as one element per class.
fn script_class(c: char) -> u8 {
    let u = c as u32;
    if (0x1100..=0x11ff).contains(&u)      // Hangul Jamo
        || (0xa960..=0xa97f).contains(&u)  // Hangul Jamo Extended-A
        || (0xac00..=0xd7ff).contains(&u)  // Hangul syllables
    {
        1
    } else if (0x2e80..=0x2fdf).contains(&u)   // CJK radicals
        || (0x3000..=0x30ff).contains(&u)      // CJK punctuation, Hiragana, Katakana
        || (0x3400..=0x4dbf).contains(&u)      // CJK Extension A
        || (0x4e00..=0x9fff).contains(&u)      // CJK Unified Ideographs
        || (0xf900..=0xfaff).contains(&u)      // CJK compatibility ideographs
        || (0xfe30..=0xfe4f).contains(&u)      // CJK compatibility forms
        || (0xff00..=0xffef).contains(&u)      // full-width forms
    {
        2
    } else if u >= 0x1f000 {
        3 // emoji and pictographs
    } else if (0x0590..=0x08ff).contains(&u) || (0xfb1d..=0xfdff).contains(&u) || (0xfe70..=0xfeff).contains(&u) {
        4 // (round 8) Hebrew, Arabic, Syriac, Thaana: right to left
    } else if u >= 0x0900 && !latin_class(c) {
        5 // (round 8) every other alphabet the Latin face does not carry: Indic, Thai, Ethiopic, ...
    } else {
        0 // Latin, Greek, Cyrillic, digits, punctuation, spaces, symbols
    }
}

/// (round 8) A character the order line can draw in one `<text>` with the Latin prefix: Latin, Greek, Cyrillic,
/// punctuation and symbols. Arabic, Hebrew, Thai, Devanagari, CJK, Hangul and emoji are not: in the same element as
/// "Left to right:" the rasteriser printed an Arabic name as tofu, unjoined its letters, and reordered RTL names.
/// (round 9 minor) A NARROW allow-list: the blocks the Latin face (Noto Sans) actually carries. The round-8 ranges
/// admitted Armenian and whole symbol blocks (arrows, dingbats, Braille, box drawing, ⭐) that it does not: their names
/// did not tofu, but the rasteriser set the whole "Left to right" line in a bold monospace or serif fallback face.
/// Everything else gets the numbered rows, one name per row.
fn latin_class(c: char) -> bool {
    let u = c as u32;
    if matches!(u, 0x200b..=0x200f | 0x202a..=0x202e | 0x2066..=0x2069) { return false; } // joiners and bidi controls
    matches!(u,
        0x0000..=0x024f     // Basic Latin, Latin-1 Supplement, Latin Extended-A and -B
        | 0x0250..=0x02ff   // IPA extensions, spacing modifier letters
        | 0x0300..=0x036f   // combining diacritical marks
        | 0x0370..=0x03ff   // Greek and Coptic
        | 0x0400..=0x052f   // Cyrillic and its supplement
        | 0x1e00..=0x1eff   // Latin Extended Additional (Vietnamese)
        | 0x1f00..=0x1fff   // Greek Extended
        | 0x2000..=0x206f   // general punctuation
        | 0x2070..=0x209f   // superscripts and subscripts
        | 0x20a0..=0x20cf   // currency symbols
        | 0x2100..=0x214f   // letterlike symbols
        | 0x2c60..=0x2c7f   // Latin Extended-C
        | 0xa720..=0xa7ff   // Latin Extended-D
        | 0xfb00..=0xfb06)  // Latin ligatures (ﬁ ﬂ)
}

/// (round 8 minor) A character that belongs to the glyph before it: a zero-width joiner, a variation selector, a
/// combining mark, a keycap or a tag. Splitting at one broke a joined emoji (👨‍👩‍👧) into three faces.
fn joins_previous(c: char) -> bool {
    matches!(c as u32,
        0x200c | 0x200d | 0x0300..=0x036f | 0x1ab0..=0x1aff | 0x20d0..=0x20ff | 0xfe00..=0xfe0f | 0xfe20..=0xfe2f
        | 0x1f3fb..=0x1f3ff | 0xe0020..=0xe007f | 0xe0100..=0xe01ef)
}

/// (round 8 major) Whether a name mixes two scripts the rasteriser cannot share one element between (Han with Hangul,
/// say): drawn as one `<text>`, "日本한국" printed as "日本ㅎㅏㄴㄱㅜㄱ". Latin beside one other script is fine.
fn needs_runs(s: &str) -> bool {
    let classes: std::collections::BTreeSet<u8> = s.chars().filter(|c| !joins_previous(*c)).map(script_class).filter(|k| *k != 0).collect();
    classes.len() >= 2
}

/// Split a string into maximal runs of one script class, so each becomes its own `<text>` and no element mixes a Han
/// and a Hangul name (which the rasteriser prints as tofu).
fn script_runs(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cls: Option<u8> = None;
    for c in s.chars() {
        // a joiner, selector or modifier stays with the glyph it modifies
        if cls.is_some() && joins_previous(c) { cur.push(c); continue; }
        let k = script_class(c);
        if cls.is_some() && cls != Some(k) {
            out.push(std::mem::take(&mut cur));
        }
        cls = Some(k);
        cur.push(c);
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

/// Greedy word wrap to a width in px; a word wider than the line takes a line of its own.
fn wrap_w(s: &str, max: f64, size: f64) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        if !cur.is_empty() && text_w(&cur, size) + text_w(" ", size) + text_w(word, size) > max {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() { cur.push(' '); }
        cur.push_str(word);
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

/// The leaf order as printed under the picture, wrapped to `max` px: the first line carries "Left to right: " and the
/// later ones are aligned under it, so their widths are counted in.
pub fn leaf_order(root: &Node, max: f64, size: f64) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for part in leaves(root).iter() {
        if !cur.is_empty() && text_w(&cur, size) + text_w(", ", size) + text_w(part, size) > max {
            // (round 8 minor) the separator stays at the end of the line it breaks: with multi-word names a reader
            // could not tell one long name from two
            cur.push(',');
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() { cur.push_str(", "); }
        cur.push_str(part);
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

/// (round 8 minor) A conservative width for the canvas guard: Noto Sans draws o, p, b, d, g, h, n and u at about 0.6 em
/// where `text_w` counts 0.56, so a line of them clipped at the right edge. Never less than `text_w`.
fn safe_w(s: &str, size: f64) -> f64 {
    s.chars()
        .map(|c| {
            let real = match c {
                'b' | 'd' | 'g' | 'h' | 'n' | 'o' | 'p' | 'q' | 'u' => 0.63,
                'a' | 'e' => 0.58,
                'r' => 0.42,
                'f' | 't' => 0.37,
                'D' | 'G' | 'H' | 'N' | 'O' | 'Q' | 'U' => 0.8,
                _ if !c.is_ascii() && glyph_em(c) < 0.9 => glyph_em(c).max(0.66),
                _ => glyph_em(c),
            };
            size * real.max(glyph_em(c))
        })
        .sum()
}

/// The names of a tree that the order line must not set in the Latin prefix's element (see `latin_class`).
fn all_latin(names: &[String]) -> bool { names.iter().all(|n| n.chars().all(latin_class)) }

/// (round 8) The most names the order line lists one to a row; a longer list ends with a line saying how many more.
const MAX_ORDER_ROWS: usize = 40;

/// The order line's rows when a name is not Latin: one name per row, so no run is placed by an estimated width.
fn order_rows(names: &[String]) -> Vec<(String, String)> {
    let shown = if names.len() > MAX_ORDER_ROWS { MAX_ORDER_ROWS - 1 } else { names.len() };
    let mut out: Vec<(String, String)> = names.iter().take(shown).enumerate().map(|(i, n)| (format!("{}.", i + 1), n.clone())).collect();
    if names.len() > shown {
        out.push((String::new(), format!("\u{2026} and {} more, in order on the tree above", names.len() - shown)));
    }
    out
}

pub fn warnings(p: &Params) -> Vec<String> {
    // (round 6 minor) a tree at the shared cap was cut before the parser saw it: the figure said "A bracket is never
    // closed" about a tree the student typed balanced. Say what actually happened.
    if p.s("tree").chars().count() >= MAX_TREE_CHARS {
        return vec![format!("The tree is too long: the box holds at most {MAX_TREE_CHARS} characters, so it was cut and cannot be drawn. Shorten it, or split it into smaller trees.")];
    }
    match parse(p.s("tree")) {
        Ok(n) => {
            if n.is_leaf() { vec!["The tree is a single name: put two or more inside brackets to make a group.".to_string()] } else { Vec::new() }
        }
        Err(e) => vec![format!("The tree could not be read: {e}")],
    }
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}">{}</text>"##,
        svg::esc(s)
    )
}

/// (round 8 major) A label over a branch carries a thin white halo, like the enzyme labels: the outline is drawn as its
/// own text first and the fill on top, because the PNG renderer ignores `paint-order`.
fn text_el_halo(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    let common = format!(r##"x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif""##);
    let t = svg::esc(s);
    format!(r##"<text {common} fill="none" stroke="#fff" stroke-width="2.4" stroke-linejoin="round">{t}</text><text {common} fill="{fill}">{t}</text>"##)
}

/// A name as it is drawn everywhere (leaf, ancestor, order line): one `<text>`, or, when it mixes two scripts the
/// rasteriser cannot share (see `needs_runs`), one `<text>` per script run laid out from the estimated widths. A single
/// short name drifts little, and the runs together keep the anchor the single element had.
fn label_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str, halo: bool) -> String {
    let one = |x: f64, anchor: &str, t: &str| if halo { text_el_halo(x, y, size, anchor, fill, t) } else { text_el(x, y, size, anchor, fill, t) };
    if !needs_runs(s) { return one(x, anchor, s); }
    // (round 9 minor) A Latin run is placed against its neighbour, not by its own estimated width: the CJK and Hangul
    // runs are a fixed 1 em a character, so their estimate is exact, while a Latin run's is not; "Mixed 日本한국" showed a
    // double gap where the over-estimated "Mixed " ended. A Latin run before another run is anchored at that run's start
    // ("end"), one after it at that run's end ("start"). Its edge spaces become no-break spaces, which the renderer
    // keeps (a plain trailing space in a `<text>` is dropped).
    let runs: Vec<String> = script_runs(s)
        .into_iter()
        .map(|r| {
            let lead = r.len() - r.trim_start_matches(' ').len();
            let trail = r.len() - r.trim_end_matches(' ').len();
            if lead + trail == 0 || lead == r.len() { return r; }
            format!("{}{}{}", "\u{a0}".repeat(lead), r.trim_matches(' '), "\u{a0}".repeat(trail))
        })
        .collect();
    let widths: Vec<f64> = runs.iter().map(|r| text_w(r, size)).collect();
    let total: f64 = widths.iter().sum();
    let x0 = match anchor { "end" => x - total, "middle" => x - total / 2.0, _ => x };
    let starts: Vec<f64> = widths.iter().scan(x0, |acc, w| { let s0 = *acc; *acc += w; Some(s0) }).collect();
    let latin = |r: &str| r.chars().filter(|c| !joins_previous(*c)).all(|c| script_class(c) == 0);
    let mut out = String::new();
    for (i, r) in runs.iter().enumerate() {
        if latin(r) && i + 1 < runs.len() {
            out.push_str(&one(starts[i + 1], "end", r));
        } else if latin(r) && i > 0 {
            out.push_str(&one(starts[i], "start", r));
        } else {
            out.push_str(&one(starts[i], "start", r));
        }
    }
    out
}

/// The length of the part of segment (x0, y0)-(x1, y1) inside the box (left, right, top, bottom) (Liang-Barsky).
fn seg_in_box(s: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> f64 {
    let (x0, y0, x1, y1) = s;
    let (dx, dy) = (x1 - x0, y1 - y0);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, x0 - b.0), (dx, b.1 - x0), (-dy, y0 - b.2), (dy, b.3 - y0)] {
        if p == 0.0 {
            if q < 0.0 { return 0.0; }
        } else {
            let r = q / p;
            if p < 0.0 { t0 = t0.max(r); } else { t1 = t1.min(r); }
        }
    }
    if t1 > t0 { (t1 - t0) * (dx * dx + dy * dy).sqrt() } else { 0.0 }
}

pub fn build(p: &Params) -> String {
    let print = p.s("style") == "Print";
    let lab = svg::label_factor(p.s("label_size"));
    let slanted = p.s("shape") == "slanted";
    let ink = if print { "#555555" } else { INK };
    let branch = if print { "#333333" } else { BRANCH };
    let leaf_col = if print { "#111111" } else { LEAF };

    let too_long = p.s("tree").chars().count() >= MAX_TREE_CHARS;
    let node = if too_long { Err(TOO_LONG.to_string()) } else { parse(p.s("tree")) };
    let maxd = node.as_ref().map(height).unwrap_or(0);
    let n_leaves = node.as_ref().map(|n| leaves(n).len()).unwrap_or(1).max(1);
    // (round 5) measured by width, not by characters (see `text_w`)
    let widest = node
        .as_ref()
        .map(|n| leaves(n).into_iter().max_by(|a, b| text_w(a, 12.0 * lab).partial_cmp(&text_w(b, 12.0 * lab)).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or_default())
        .unwrap_or_default();

    // (round 2) an ancestor's label sits above and to the left of its branching point: the column must be wide enough for
    // the widest of them (or it would run back over the previous branch), and the left margin for the root's.
    let widest_anc = node.as_ref().map(|n| ancestor_labels(n).into_iter().map(|l| text_w(&l, 11.0 * lab)).fold(0.0f64, f64::max)).unwrap_or(0.0);
    let root_w = node.as_ref().map(|n| if n.is_leaf() { 0.0 } else { text_w(&n.label, 11.0 * lab) }).unwrap_or(0.0);
    let left = 30.0 + root_w;
    let step_x = (STEP_X * lab).max(widest_anc + 30.0);
    let tree_w = left + maxd as f64 * step_x;
    let label_w = text_w(&widest, 12.0 * lab) + 18.0;
    let prefix_w = text_w("Left to right: ", 12.0 * lab);
    // (round 4) the line under the picture ("4 leaves, 1 branching point, 1 level") grew with the labels and ran past
    // the canvas at Large: the canvas takes its width into account
    let count_text = node.as_ref().ok().map(|root| {
        let plural = |n: usize, one: &str, many: &str| if n == 1 { one.to_string() } else { many.to_string() };
        let (ls, br, lv) = (leaves(root).len(), branch_points(root), height(root));
        format!("{ls} {}, {br} {}, {lv} {}", plural(ls, "leaf", "leaves"), plural(br, "branching point", "branching points"), plural(lv, "level", "levels"))
    });
    let mut w = (tree_w + label_w + 28.0).max(360.0);
    if let Some(t) = &count_text {
        w = w.max(24.0 + text_w(t, 12.0 * lab) + 12.0);
    }
    // (round 6) the leaf-order line: a name too long to break takes a line of its own, so the canvas must fit the
    // prefix plus the widest name as well as the leaf label at the end of the tree (with a little headroom for a font
    // wider than the estimate)
    w = w.max(24.0 + prefix_w + text_w(&widest, 12.0 * lab) * 1.06 + 14.0);
    // (round 8) every leaf label fits by the conservative width too (o/p/b/d-heavy names drew wider than estimated)
    let names = node.as_ref().map(|n| leaves(n)).unwrap_or_default();
    for l in &names { w = w.max(tree_w + 8.0 + safe_w(l, 12.0 * lab) + 6.0); }
    // (round 8 major) the order line is one Latin `<text>` only when every name is Latin; otherwise one name per row
    let latin_line = all_latin(&names);
    let rows = if latin_line { Vec::new() } else { order_rows(&names) };
    let num_w = rows.iter().map(|r| text_w(&r.0, 12.0 * lab)).fold(0.0f64, f64::max);
    let row_x = 24.0 + 12.0 * lab + num_w + 6.0 * lab;
    for (_, name) in &rows { w = w.max(row_x + safe_w(name, 12.0 * lab).max(text_w(name, 12.0 * lab) * 1.06) + 14.0); }
    let top = 34.0;
    let h = top + ((n_leaves as f64) - 1.0).max(1.0) * LEAF_GAP * lab + 40.0 * lab;
    // (round 2) the text under the picture wraps to the CANVAS, not to a fixed character count; (round 5) to its width.
    let leaf_max = (w - 24.0 - prefix_w - 12.0).max(40.0);
    let err_max = (w - 48.0).max(60.0);
    // (round 8 minor) a Latin line that the estimate under-measures still fits: the canvas grows to its safe width
    if latin_line {
        if let Ok(root) = &node {
            for (i, line) in leaf_order(root, leaf_max, 12.0 * lab).iter().enumerate() {
                let x = if i == 0 { 24.0 + safe_w("Left to right: ", 12.0 * lab) } else { 24.0 + prefix_w };
                w = w.max(x + safe_w(line, 12.0 * lab) + 6.0);
            }
        }
    }
    let below = match &node {
        Ok(_) if !latin_line => 1 + rows.len(),
        Ok(root) => leaf_order(root, leaf_max, 12.0 * lab).len().max(1),
        Err(e) => wrap_w(e, err_max, 13.0 * lab).len() + if too_long { 0 } else { wrap_w(NO_TREE, err_max, 12.0 * lab).len() },
    };
    let mut d = Doc::new(w, h + (14.0 + below as f64 * 18.0).max(62.0) * lab);
    d.describe("Cladogram", "A branching tree of the names typed, with the leaves in the order they were written");

    match &node {
        Ok(root) => {
            let placed = place(root);
            let px = |x: usize| left + x as f64 * step_x;
            let py = |y: f64| top + y * LEAF_GAP * lab;
            // a branch: a right angle at the parent's depth (rectangular) or a straight diagonal (slanted). The
            // rectangular branch runs along the CHILD's row and starts on the parent's bar, which spans every child's
            // row: drawn along the parent's row instead (round 2) a star of five names showed one line, not five.
            for (x0, y0, x1, y1) in &placed.edges {
                if slanted {
                    d.line(px(*x0), py(*y0), px(*x1), py(*y1), branch, 2.0);
                } else {
                    d.line(px(*x0), py(*y1), px(*x1), py(*y1), branch, 2.0);
                }
            }
            // the vertical bar at each branching point, joining its first and last child
            if !slanted {
                for (x, first, last) in &placed.bars {
                    if last > first { d.line(px(*x), py(*first), px(*x), py(*last), branch, 2.0); }
                }
            }
            // a labelled ancestor, just above its branching point and clear of the bar
            let segs: Vec<(f64, f64, f64, f64)> = placed.edges.iter().map(|(x0, y0, x1, y1)| (px(*x0), py(*y0), px(*x1), py(*y1))).collect();
            // the boxes an ancestor label must not overprint: every leaf label, then each ancestor label as it is placed
            let mut taken: Vec<(f64, f64, f64, f64)> = placed
                .leaves
                .iter()
                .map(|(label, x, y)| { let (lx, ly) = (px(*x) + 8.0, py(*y) + 4.5 * lab); (lx, lx + text_w(label, 12.0 * lab), ly - 12.0 * lab * 0.8, ly + 12.0 * lab * 0.25) })
                .collect();
            for (label, x, y) in &placed.nodes {
                if !slanted {
                    d.raw(&label_el(px(*x) - 5.0, py(*y) - 5.0, 11.0 * lab, "end", ink, label, false));
                    continue;
                }
                // (round 8 major) In the slanted shape the parent's diagonal reaches a lower child's fork through the
                // spot above-left of it, and struck the label through ("Others"). The label goes on the side the
                // diagonal does not come from (above-left, or below-left), the one its branches cross least, and it is
                // haloed so a line that still grazes it does not strike it through.
                // (round 9 major) Only its own incoming diagonal was considered, and the least-bad spot was taken
                // even when it was struck: a SIBLING's diagonal ran through "Hominini" and "Primates and others". The
                // label is now tested against every branch of the placed tree, and against the labels already
                // drawn, over a wider set of places (both sides, above and below, further out, centred on the fork,
                // and to its right); the nearest place no branch crosses wins, and only when there is none, the one
                // crossed least.
                let size = 11.0 * lab;
                let lw = text_w(label, size);
                let (fx, fy) = (px(*x), py(*y));
                let from_above = placed.edges.iter().any(|(_, y0, x1, y1)| *x1 == *x && *y1 == *y && *y0 < *y);
                let above = fy - 5.0;
                let below = fy + 5.0 + size * 0.75;
                let mut cands: Vec<(f64, f64, &str)> = vec![(fx - 5.0, above, "end"), (fx - 5.0, below, "end")];
                if from_above { cands.swap(0, 1); }
                cands.push((fx - 5.0, above - 8.0 * lab, "end"));
                cands.push((fx - 5.0, below + 8.0 * lab, "end"));
                for k in 1..=3 {
                    let dy = (8.0 + 8.0 * k as f64) * lab;
                    for dx in [0.0, 12.0 * k as f64] {
                        cands.push((fx - 5.0 - dx, above - dy + 8.0 * lab, "end"));
                        cands.push((fx - 5.0 - dx, below + dy - 8.0 * lab, "end"));
                    }
                }
                for dx in [12.0, 24.0, 36.0, 48.0] {
                    cands.push((fx - 5.0 - dx, above, "end"));
                    cands.push((fx - 5.0 - dx, below, "end"));
                }
                for dy in [0.0, 8.0 * lab, 16.0 * lab, 24.0 * lab] {
                    cands.push((fx, above - dy, "middle"));
                    cands.push((fx, below + dy, "middle"));
                    cands.push((fx + 5.0, above - dy, "start"));
                    cands.push((fx + 5.0, below + dy, "start"));
                }
                let bx = |(lx, ly, an): (f64, f64, &str)| {
                    let l = match an { "end" => lx - lw, "middle" => lx - lw / 2.0, _ => lx };
                    (l - 1.0, l + lw + 1.0, ly - size * 0.8, ly + size * 0.25)
                };
                // the round-8 places first, in their order; the rest nearest the fork first, so a clean place hugging
                // the fork wins over one further down that only happens to be clean too
                let dist = |c: &(f64, f64, &str)| {
                    let b = bx(*c);
                    let dx = (b.0 - fx).max(fx - b.1).max(0.0);
                    let dy = (b.2 - fy).max(fy - b.3).max(0.0);
                    (dx * dx + dy * dy).sqrt()
                };
                cands[4..].sort_by(|a, b| dist(a).partial_cmp(&dist(b)).unwrap_or(std::cmp::Ordering::Equal));
                let clear_of = |b: (f64, f64, f64, f64), o: &(f64, f64, f64, f64)| b.1 <= o.0 || o.1 <= b.0 || b.3 <= o.2 || o.3 <= b.2;
                let usable = |c: (f64, f64, &str)| {
                    let b = bx(c);
                    b.0 >= 2.0 && b.1 <= w - 2.0 && b.2 >= 2.0 && taken.iter().all(|o| clear_of(b, o))
                };
                let cost = |c: (f64, f64, &str)| { let b = bx(c); segs.iter().map(|sg| seg_in_box(*sg, b)).sum::<f64>() };
                let best = cands.iter().copied().find(|c| usable(*c) && cost(*c) == 0.0).unwrap_or_else(|| {
                    let pool: Vec<(f64, f64, &str)> = { let u: Vec<_> = cands.iter().copied().filter(|c| usable(*c)).collect(); if u.is_empty() { cands.clone() } else { u } };
                    pool.iter().copied().fold(pool[0], |a, c| if cost(c) < cost(a) { c } else { a })
                });
                taken.push(bx(best));
                d.raw(&label_el(best.0, best.1, size, best.2, ink, label, true));
            }
            for (label, x, y) in &placed.leaves {
                d.raw(&label_el(px(*x) + 8.0, py(*y) + 4.5 * lab, 12.0 * lab, "start", leaf_col, label, false));
            }
        }
        Err(e) => {
            // (round 6 minor) a tree cut at the shared cap never reached the parser: say it is too long, not that a
            // bracket is unclosed
            let head = if too_long { "The tree is too long:" } else { "The tree could not be read:" };
            d.raw(&text_el(24.0, 46.0, 13.0 * lab, "start", if print { "#555555" } else { ERR }, head));
            let mut y = 70.0 * lab;
            for line in wrap_w(e, err_max, 13.0 * lab) {
                d.raw(&text_el(24.0, y, 13.0 * lab, "start", ink, &line));
                y += 18.0 * lab;
            }
            // (round 2) after the reason, not at a fixed row it used to overprint; (round 4) wrapped like the reason,
            // which at Large ran off the 360 px canvas
            if !too_long {
                let mut ny = y + 8.0 * lab;
                for line in wrap_w(NO_TREE, err_max, 12.0 * lab) {
                    d.raw(&text_el(24.0, ny, 12.0 * lab, "start", ink, &line));
                    ny += 15.0 * lab;
                }
            }
        }
    }

    // what the text was read as: the leaves, so a typo is visible without counting brackets
    let y = h + 8.0 * lab;
    if let Ok(root) = &node {
        if let Some(count) = &count_text {
            d.raw(&text_el(24.0, y, 12.0 * lab, "start", ink, count));
        }
        if latin_line {
            for (i, line) in leaf_order(root, leaf_max, 12.0 * lab).iter().enumerate() {
                // (round 3) the continuation lines are indented by x, not by leading spaces: SVG collapses those, so
                // the line used to start hard against the left margin
                let (x, text) = if i == 0 { (24.0, format!("Left to right: {line}")) } else { (24.0 + prefix_w, line.clone()) };
                d.raw(&text_el(x, y + (15.0 + i as f64 * 17.0) * lab, 12.0 * lab, "start", ink, &text));
            }
        } else {
            // (round 8 majors 2 and 4) A name that is not Latin is never set in the Latin prefix's element (an Arabic
            // name printed as tofu, an RTL pair reordered) and no run is placed at an estimated cumulative x (runs
            // overlapped or left holes, a comma vanished at a wrap). "Left to right:" is its own element, then each
            // name is its own element on its own numbered row, drawn exactly as its leaf is.
            d.raw(&text_el(24.0, y + 15.0 * lab, 12.0 * lab, "start", ink, "Left to right:"));
            let num_x = 24.0 + 12.0 * lab + num_w;
            for (i, (num, name)) in rows.iter().enumerate() {
                let yy = y + (15.0 + (i + 1) as f64 * 17.0) * lab;
                if !num.is_empty() { d.raw(&text_el(num_x, yy, 12.0 * lab, "end", ink, num)); }
                d.raw(&label_el(if num.is_empty() { 24.0 + 12.0 * lab } else { row_x }, yy, 12.0 * lab, "start", ink, name, false));
            }
        }
    }

    d.finish()
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "The great apes", params: vec![s("tree", "(((Human, Chimp), Gorilla), Orangutan)")] },
        Example { name: "A slanted tree of vertebrates", params: vec![s("tree", "(((Lizard, Bird), (Frog, Fish)), Lamprey)"), s("shape", "slanted")] },
        Example { name: "One ancestor, four descendants", params: vec![s("tree", "(Red, Blue, Green, Yellow)")] },
        Example { name: "A labelled ancestor", params: vec![s("tree", "((Human, Chimp)Great apes, Gorilla)")] },
        Example { name: "The same tree typed with a semicolon", params: vec![s("tree", "((A, B), (C, D));")] },
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

    /// Every drawn segment as (x1, y1, x2, y2).
    fn lines(svg: &str) -> Vec<(f64, f64, f64, f64)> {
        svg.split("<line ").skip(1).map(|part| {
            let num = |k: &str| -> f64 {
                let at = part.find(&format!("{k}=\"")).expect("attribute") + k.len() + 2;
                part[at..].split('"').next().unwrap().parse().unwrap()
            };
            (num("x1"), num("y1"), num("x2"), num("y2"))
        }).collect()
    }

    #[test]
    fn the_parser_reads_nested_trees() {
        let n = parse("(((Human, Chimp), Gorilla), Orangutan)").unwrap();
        assert_eq!(leaves(&n), vec!["Human", "Chimp", "Gorilla", "Orangutan"]);
        assert_eq!(height(&n), 3);
        assert_eq!(leaves(&parse(" (A , B) ; ").unwrap()), vec!["A", "B"]);
        // a name after a bracket labels that ancestor
        assert_eq!(parse("((A, B)Ancestor, C)").unwrap().children[0].label, "Ancestor");
        // so is a label on the root
        assert_eq!(parse("((A, B), C)Root").unwrap().label, "Root");
        // a name may hold spaces (binomials, common names): whitespace is only skipped around brackets and commas
        assert_eq!(leaves(&parse(" (Homo sapiens, Pan troglodytes) ").unwrap()), vec!["Homo sapiens", "Pan troglodytes"]);
    }

    #[test]
    fn the_parser_refuses_malformed_trees() {
        for bad in ["(A, B", "A, B", "()", "(A,)", ")"] {
            assert!(parse(bad).is_err(), "{bad} parsed");
        }
        let t = texts(&render_of(&[s("tree", "(A, B")]));
        assert!(t.contains("could not be read"), "{t}");
        assert!(warnings(&with(&[s("tree", "(A, B")])).iter().any(|w| w.contains("never closed")), "{:?}", warnings(&with(&[s("tree", "(A, B")])));
    }

    #[test]
    fn the_layout_puts_the_leaves_in_order_and_the_root_on_the_left() {
        let n = parse("(((Human, Chimp), Gorilla), Orangutan)").unwrap();
        let placed = place(&n);
        let labels: Vec<String> = placed.leaves.iter().map(|l| l.0.clone()).collect();
        assert_eq!(labels, vec!["Human", "Chimp", "Gorilla", "Orangutan"]);
        // every leaf at the full depth, on its own row, in order
        for (i, (_, x, y)) in placed.leaves.iter().enumerate() {
            assert_eq!(*x, 3, "a leaf is not at the full depth");
            assert_eq!(*y, i as f64, "rows are not in order");
        }
        // a branch always runs from a shallower node to a deeper one
        for (x0, _, x1, _) in &placed.edges { assert!(x0 < x1, "edge goes backwards: {x0} -> {x1}"); }
        assert_eq!(placed.edges.len(), labels.len() + count_inner(&n) - 1);
        // one bar per branching point, and each starts above it ends
        assert_eq!(placed.bars.len(), count_inner(&n));
        for (_, a, b) in &placed.bars { assert!(a <= b); }
    }

    #[test]
    fn every_leaf_is_labelled() {
        for tree in ["(((Human, Chimp), Gorilla), Orangutan)", "(A, B, C, D, E, F)", "((A, B)X, (C, D)Y)"] {
            let t = texts(&render_of(&[s("tree", tree)]));
            for leaf in leaves(&parse(tree).unwrap()) { assert!(t.contains(&leaf), "{tree}: {leaf} missing"); }
        }
    }

    #[test]
    fn the_two_shapes_draw_differently() {
        let rect = render_of(&[]);
        let slant = render_of(&[s("shape", "slanted")]);
        assert_ne!(rect, slant);
        // the slanted tree draws no joining bars: it has exactly the branches, the rectangular one has the bars too
        let n = parse("(((Human, Chimp), Gorilla), Orangutan)").unwrap();
        let segs = |svg: &str| svg.matches("<line").count();
        assert_eq!(segs(&slant), placed_edges(), "the slanted tree drew more than its branches");
        assert_eq!(segs(&rect), placed_edges() + count_inner(&n), "the rectangular tree is missing its bars");
        fn placed_edges() -> usize { place(&parse("(((Human, Chimp), Gorilla), Orangutan)").unwrap()).edges.len() }
    }

    #[test]
    fn a_single_name_is_reported_not_drawn_as_a_tree() {
        let p = with(&[s("tree", "Human")]);
        assert!(warnings(&p).iter().any(|w| w.contains("single name")), "{:?}", warnings(&p));
        assert!(texts(&render_of(&[s("tree", "Human")])).contains("Human"));
    }

    #[test]
    fn every_leaf_gets_its_own_branch() {
        // (round 2) The rectangular branches were drawn along the PARENT's row, so a star of five names had one line at
        // the middle row and no line reaching the other four leaves. Every child's branch runs along the child's row.
        let svg = render_of(&[s("tree", "(A, B, C, D, E)")]);
        let ls = lines(&svg);
        let mut ys: Vec<f64> = ls.iter().filter(|l| l.1 == l.3).map(|l| l.1).collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ys.dedup();
        assert_eq!(ys.len(), 5, "a star of five names drew {} horizontal branches: {ls:?}", ys.len());
        // one branching point, so one bar, joining the first branch's row to the last's
        let bars: Vec<_> = ls.iter().filter(|l| l.0 == l.2).copied().collect();
        assert_eq!(bars.len(), 1, "one branching point, one bar: {bars:?}");
        assert_eq!((bars[0].1, bars[0].3), (ys[0], ys[4]), "the bar does not join every child's branch");
        // and a nested tree: every branch starts on its parent's bar, at a row that bar actually spans
        let nested = lines(&render_of(&[s("tree", "(((A, B), C), D)")]));
        let bars: Vec<_> = nested.iter().filter(|l| l.0 == l.2).copied().collect();
        for l in nested.iter().filter(|l| l.1 == l.3) {
            assert!(
                bars.iter().any(|b| b.0 == l.0 && (b.1 - l.1) * (b.3 - l.1) <= 0.01),
                "a branch does not start on a bar at its row: {l:?} in {nested:?}"
            );
        }
    }

    #[test]
    fn a_long_list_of_names_prints_inside_the_canvas() {
        // twelve binomials wrap the leaf order onto four lines; the old fixed allowance reached the canvas edge, so the
        // last line was drawn outside the picture
        let tree = format!("({})", (1..=12).map(|i| format!("Species name{i}")).collect::<Vec<_>>().join(", "));
        let svg = render_of(&[("tree", Value::S(tree))]);
        let h: f64 = {
            let at = svg.find("height=\"").expect("height") + 8;
            svg[at..].split('"').next().unwrap().parse().unwrap()
        };
        let mut last = 0.0f64;
        for el in svg.split("<text").skip(1) {
            let at = el.find("y=\"").expect("y") + 3;
            last = last.max(el[at..].split('"').next().unwrap().parse::<f64>().unwrap());
        }
        assert!(last <= h, "the last line (y = {last}) is below the canvas (height {h})");
        assert!(texts(&svg).contains("Left to right: Species name1"), "{}", texts(&svg));
    }

    /// Every drawn text as (x, y, font-size, anchor, content).
    fn boxes(svg: &str) -> Vec<(f64, f64, f64, String, String)> {
        let mut out = Vec::new();
        for part in svg.split("<text").skip(1) {
            let num = |k: &str| -> Option<f64> {
                let at = part.find(&format!("{k}=\""))? + k.len() + 2;
                part[at..].split('"').next()?.parse().ok()
            };
            let Some(x) = num("x") else { continue };
            let anchor = match part.find("text-anchor=\"") {
                Some(i) => part[i + 13..].split('"').next().unwrap_or("start").to_string(),
                None => "start".to_string(),
            };
            let content = part.split_once('>').map(|(_, rest)| rest.split("</text>").next().unwrap_or("").to_string()).unwrap_or_default();
            out.push((x, num("y").unwrap_or(0.0), num("font-size").unwrap_or(12.0), anchor, content));
        }
        out
    }

    /// The haloed ancestor-label texts of a slanted render (the halo copy marks them): (x, y, size, anchor, text).
    fn ancestor_texts(svg: &str) -> Vec<(f64, f64, f64, String, String)> {
        let mut out = Vec::new();
        for part in svg.split("<text").skip(1) {
            if !part.contains(r##"fill="none" stroke="#fff""##) { continue; }
            let num = |k: &str| -> Option<f64> { let at = part.find(&format!("{k}=\""))? + k.len() + 2; part[at..].split('"').next()?.parse().ok() };
            let anchor = part.find("text-anchor=\"").map(|i| part[i + 13..].split('"').next().unwrap_or("start").to_string()).unwrap_or_else(|| "start".into());
            let content = part.split_once('>').map(|(_, r)| r.split("</text>").next().unwrap_or("").to_string()).unwrap_or_default();
            out.push((num("x").unwrap(), num("y").unwrap(), num("font-size").unwrap(), anchor, content));
        }
        out
    }

    /// Branch length inside each ancestor label's ink, over a slanted render: (label, px of branch inside).
    fn struck(svg: &str) -> Vec<(String, f64)> {
        let lines: Vec<(f64, f64, f64, f64)> = svg.split("<line ").skip(1).filter_map(|p| {
            let num = |k: &str| -> Option<f64> { let at = p.find(&format!("{k}=\""))? + k.len() + 2; p[at..].split('"').next()?.parse().ok() };
            Some((num("x1")?, num("y1")?, num("x2")?, num("y2")?))
        }).collect();
        ancestor_texts(svg).into_iter().filter_map(|(x, y, size, anchor, t)| {
            let w = text_w(&t.replace("&amp;", "&"), size);
            let l = match anchor.as_str() { "end" => x - w, "middle" => x - w / 2.0, _ => x };
            let ink = (l + 0.5, l + w - 0.5, y - 0.7 * size, y + 0.2 * size);
            let len: f64 = lines.iter().map(|sg| seg_in_box(*sg, ink)).sum();
            if len > 0.5 { Some((t, len)) } else { None }
        }).collect()
    }

    /// A random tree like the critic's: 1-4 children, depth to 4, labelled ancestors, mixed names.
    fn random_tree(r: &mut crate::figures::biology::Rng, depth: usize, labels: usize) -> String {
        const NAMES: [&str; 10] = ["Homo", "Pan", "Gorilla", "Pongo", "Hylobates lar", "Macaca", "Bos taurus", "Ovis", "X", "Felis catus"];
        const ANC: [&str; 6] = ["Great apes", "A", "Hominini", "Primates and others", "Clade 7", "Ruminantia"];
        if depth == 0 || r.below(4) == 0 { return NAMES[r.below(NAMES.len())].to_string(); }
        let k = [1usize, 2, 2, 2, 3, 4][r.below(6)];
        let kids: Vec<String> = (0..k).map(|_| random_tree(r, depth - 1, labels)).collect();
        let mut s = format!("({})", kids.join(", "));
        if r.below(10) < labels { s.push_str(ANC[r.below(ANC.len())]); }
        s
    }

    #[test]
    fn no_branch_strikes_a_slanted_ancestor_label() {
        // (round 9 major) the round-8 fix kept a slanted label off its OWN incoming diagonal only, and took the least-bad
        // spot even when struck: a sibling's diagonal ran through "Hominini" and "Primates and others" (432 of 2,733
        // random slanted trees). Every label is now tested against every branch, over a wider set of places.
        let repros = [
            ("(Gorilla, (Human, Chimp)Hominini, Orangutan)", "Medium"),
            ("((Homo, X)Ruminantia, (Ovis, Homo)Primates and others, Homo)A", "Large"),
            ("(Pongo, (Pongo, X)Ruminantia, Pan)Clade 7", "Large"),
            ("((Human, Chimp)Hominini, (Gorilla, Orangutan)Others)", "Medium"),
        ];
        for (t, size) in repros {
            let svg = render_of(&[s("tree", t), s("shape", "slanted"), s("label_size", size)]);
            assert_eq!(struck(&svg), vec![], "{t} ({size})");
            assert_fits(t, &svg);
        }
        let mut rng = crate::figures::biology::Rng::new(9);
        let sizes = ["Small", "Medium", "Large"];
        let (mut n, mut bad) = (0usize, Vec::new());
        while n < 2500 {
            let depth = 2 + rng.below(3);
            let labels = [3usize, 7, 10][rng.below(3)];
            let t = random_tree(&mut rng, depth, labels);
            if !t.starts_with('(') || t.chars().count() >= 480 { continue; }
            n += 1;
            let size = sizes[rng.below(3)];
            let svg = render_of(&[s("tree", &t), s("shape", "slanted"), s("label_size", size)]);
            let st = struck(&svg);
            if !st.is_empty() { bad.push(format!("[{size}] {st:?} {t}")); }
            // every ancestor label stays on the canvas
            let w: f64 = svg.split("width=\"").nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse().ok()).unwrap();
            for (x, y, sz, anchor, txt) in ancestor_texts(&svg) {
                let tw = text_w(&txt, sz);
                let l = match anchor.as_str() { "end" => x - tw, "middle" => x - tw / 2.0, _ => x };
                if l < 0.0 || l + tw > w || y - sz < 0.0 { bad.push(format!("{t} ({size}): {txt:?} leaves the canvas")); }
            }
        }
        // Off the canvas: never. Struck: only where no place in the candidate set is clear of every branch and every
        // other label, which in this sweep is 4 of 2,500 dense trees (a root-to-leaf diagonal crossing a whole crowded
        // subtree); round 8 struck 432 of 2,733. The bound is kept tight so a regression shows.
        let off: Vec<_> = bad.iter().filter(|b| b.contains("leaves the canvas")).collect();
        assert!(off.is_empty(), "{off:?}");
        assert!(bad.len() <= 4, "{} of {n} slanted trees:\n{}", bad.len(), bad.iter().take(10).cloned().collect::<Vec<_>>().join("\n"));
    }

    #[test]
    fn only_what_the_latin_face_carries_shares_the_order_line() {
        // (round 9 minor) Armenian and whole symbol blocks were counted as Latin, and the whole "Left to right" line
        // switched to a fallback face. The allow-list is now the blocks Noto Sans carries.
        for c in ['A', 'z', 'é', 'ß', 'Ł', 'ǅ', 'ə', 'ʃ', '\u{301}', 'λ', 'Ω', 'Ж', 'ё', 'Ệ', 'ờ', 'ữ', '–', '\u{2032}', '…', '€', '₹', '²', '₂', '™', 'ℓ', 'ﬁ', 'ﬂ', '0', '9'] {
            assert!(latin_class(c), "{c:?} (U+{:04X}) should be Latin", c as u32);
        }
        for c in ['Հ', 'ա', '→', '⇒', '↔', '✈', '✓', '⠃', '─', '┼', '⭐', '⬆', '☀', '♥', '♀', '⌘', '①', '■', '∑', '∞', '😀', '👍', 'ا', 'א', '日', '한'] {
            assert!(!latin_class(c), "{c:?} (U+{:04X}) should not be Latin", c as u32);
        }
        // and the render: such a name moves the tree to numbered rows instead of the one-line list
        for name in ["Հայերեն", "→⇒↔", "⠃⠗⠇", "✈ plane", "⭐ star", "┼─┼"] {
            let t = texts(&render_of(&[s("tree", &format!("((Homo sapiens, {name}), Pan)"))]));
            assert!(!t.contains("Left to right: Homo sapiens"), "{name}: {t}");
            assert!(t.lines().any(|l| l == name), "{name} is not on a row of its own: {t}");
        }
        for name in ["Ελληνικά", "Кириллица", "Tiếng Việt", "ﬁsh"] {
            let t = texts(&render_of(&[s("tree", &format!("((Homo sapiens, {name}), Pan)"))]));
            assert!(t.contains(&format!("Left to right: Homo sapiens, {name}, Pan")), "{name}: {t}");
        }
    }

    #[test]
    fn a_mixed_script_name_has_no_double_gap() {
        // (round 9 minor) "Mixed 日本한국" as an ancestor showed a double gap before the Han run: the Latin run was placed
        // by its own over-estimated width. It now ends exactly where the next run starts, its space kept as a no-break
        // space.
        for shape in ["rectangular", "slanted"] {
            for size in ["Small", "Medium", "Large"] {
                let svg = render_of(&[s("tree", "((Homo, Pan)Mixed 日本한국, Gorilla)"), s("shape", shape), s("label_size", size)]);
                let bs: Vec<_> = boxes(&svg).into_iter().filter(|b| !b.4.is_empty()).collect();
                let latin = bs.iter().find(|b| b.4.starts_with("Mixed")).expect("the Latin run");
                let han = bs.iter().find(|b| b.4 == "日本").expect("the Han run");
                assert_eq!(latin.4, "Mixed\u{a0}", "{shape} {size}: {:?}", latin.4);
                assert_eq!(latin.3, "end", "{shape} {size}: the Latin run is not pinned to the next one");
                assert!((latin.0 - han.0).abs() < 0.01, "{shape} {size}: {latin:?} ends at {} but 日本 starts at {}", latin.0, han.0);
            }
        }
    }

    #[test]
    fn every_line_of_text_fits_the_canvas() {
        // (round 3) the wrap was 74 characters whatever the canvas: on the 360 px floor the last names of a student's
        // list were cut off at the right edge, and a malformed-tree reason was cut mid-word. Both wrap to the width now.
        let twelve = format!("({})", (1..=12).map(|i| format!("Species name{i}")).collect::<Vec<_>>().join(", "));
        for tree in [
            "(Homo sapiens, Pan troglodytes, Gorilla gorilla, Pongo pygmaeus)".to_string(),
            twelve.clone(),
            "(((Human, Chimp), Gorilla), Orangutan)".to_string(),
            "()".to_string(),
            "(A,)".to_string(),
            "".to_string(),
            // (round 6) one name too long to break: it takes a line of its own, so the canvas must fit it
            "a".repeat(60),
        ] {
            // (round 4) at Large the closing sentence ("Nothing is drawn…") was the one line still unwrapped
            for size in ["Medium", "Large"] {
                let svg = render_of(&[("tree", Value::S(tree.clone())), ("label_size", Value::S(size.to_string()))]);
                let w = {
                    let at = svg.find("width=\"").expect("width") + 7;
                    svg[at..].split('"').next().unwrap().parse::<f64>().unwrap()
                };
                for (x, _y, size, anchor, text) in boxes(&svg) {
                    let n = text_w(&text, size) * 1.05;
                    let right = match anchor.as_str() { "end" => x, "middle" => x + n / 2.0, _ => x + n };
                    assert!(right <= w + 1.0, "{tree:?} at {size}: {text:?} ends at {right:.1} on a {w} canvas");
                }
                // and every name typed is somewhere on the page
                if let Ok(n) = parse(&tree) {
                    let all = texts(&svg);
                    for leaf in leaves(&n) { assert!(all.contains(&leaf), "{tree:?}: {leaf} is not printed"); }
                }
            }
        }
    }

    #[test]
    fn a_one_name_group_is_not_called_a_branching_point() {
        let t = texts(&render_of(&[s("tree", "(A)")]));
        assert!(t.contains("1 leaf, 0 branching points, 1 level"), "{t}");
    }

    #[test]
    fn the_count_line_reads_as_english() {
        let t = texts(&render_of(&[s("tree", "(A, B, C)")]));
        assert!(t.contains("3 leaves, 1 branching point, 1 level"), "{t}");
        let t = texts(&render_of(&[s("tree", "(A, (B, C))")]));
        assert!(t.contains("3 leaves, 2 branching points, 2 levels"), "{t}");
    }

    #[test]
    fn an_ancestor_named_in_the_text_is_drawn_at_its_branching_point() {
        // the hint, the blurb and the "A labelled ancestor" example all promise this: before round 2 the parser kept the
        // name and the picture never showed it.
        let n = parse("((Human, Chimp)Great apes, Gorilla)").unwrap();
        assert_eq!(ancestor_labels(&n), vec!["Great apes"], "the ancestor name is not recognised");
        assert_eq!(place(&n).nodes, vec![("Great apes".to_string(), 1, 0.5)], "the ancestor is not placed at its branching point");
        let t = texts(&render_of(&[s("tree", "((Human, Chimp)Great apes, Gorilla)")]));
        assert!(t.contains("Great apes"), "the ancestor name is not drawn: {t}");
        // a labelled root too, and it must not run off the left edge: the margin grows for it
        let root = parse("((A, B), C)Root").unwrap();
        assert_eq!(place(&root).nodes, vec![("Root".to_string(), 0, 1.25)]);
        assert!(texts(&render_of(&[s("tree", "((A, B), C)Root")])).contains("Root"));
        // an unlabelled tree draws no such label
        assert!(place(&parse("((A, B), C)").unwrap()).nodes.is_empty());
    }

    #[test]
    fn a_name_that_is_not_latin_fits_the_canvas() {
        // (round 6 major) every non-ASCII glyph was measured at 0.56 em, so a Japanese name sized its canvas ~45 % too
        // narrow and ran off the right edge, the leaf and the "Left to right:" line alike.
        let name = "日本語の名前がとても長い場合のテストですこれはどうなるでしょうか";
        for tree in [format!("({name}, B)"), name.to_string()] {
            for size in ["Small", "Medium", "Large"] {
                let svg = render_of(&[("tree", Value::S(tree.clone())), ("label_size", Value::S(size.to_string()))]);
                let w = {
                    let at = svg.find("width=\"").expect("width") + 7;
                    svg[at..].split('"').next().unwrap().parse::<f64>().unwrap()
                };
                for (x, _y, sz, anchor, text) in boxes(&svg) {
                    let n = text_w(&text, sz) * 1.05;
                    let right = match anchor.as_str() { "end" => x, "middle" => x + n / 2.0, _ => x + n };
                    assert!(right <= w + 1.0, "{tree:?} at {size}: {text:?} ends at {right:.1} on a {w} canvas");
                }
                // the whole name is on the page, not cut at the edge
                assert!(texts(&svg).contains(name), "{tree:?} at {size}: the name is cut: {}", texts(&svg));
            }
        }
        // the Latin cases the round-5 fix already held still fit (no regression in the table)
        let svg = render_of(&[s("tree", &format!("({})", "W".repeat(29)))]);
        let w = { let at = svg.find("width=\"").unwrap() + 7; svg[at..].split('"').next().unwrap().parse::<f64>().unwrap() };
        for (x, _y, sz, anchor, text) in boxes(&svg) {
            let n = text_w(&text, sz) * 1.05;
            let right = match anchor.as_str() { "end" => x, "middle" => x + n / 2.0, _ => x + n };
            assert!(right <= w + 1.0, "{text:?} ends at {right:.1} on a {w} canvas");
        }
    }

    #[test]
    fn a_mixed_script_order_line_is_drawn_as_one_text_per_script_run() {
        // (round 7 major) "Left to right:" built one string for the whole line and drew it as one <text>: a line
        // holding both a Han and a Hangul name printed as tofu ("\u{25a1}\u{25a1}\u{25a1}\u{25a1}\u{25a1}\u{25a1}, \u{25a1}\u{25a1}\u{25a1}\u{25a1}\u{25a1}"),
        // and at Small an emoji in the same run was tofu too. Each run is now its own <text>, at the x its glyphs would
        // have started at, so the concatenation is unchanged but no element mixes scripts the rasteriser cannot share.
        let mixed = script_runs("Left to right: 日本語のテスト, 한국어이름");
        assert_eq!(mixed, vec!["Left to right: ", "日本語のテスト", ", ", "한국어이름"], "{mixed:?}");
        assert_eq!(mixed.concat(), "Left to right: 日本語のテスト, 한국어이름");
        let emoji = script_runs(", 🦕🦖, B");
        assert_eq!(emoji, vec![", ", "🦕🦖", ", B"], "{emoji:?}");
        assert_eq!(emoji.concat(), ", 🦕🦖, B");

        // (round 8) a line holding a non-Latin name is no longer one line of runs at estimated x (they overlapped and
        // left holes): "Left to right:" is its own element and each name is its own element on its own row. Still no
        // element mixes scripts, and the rows still spell the leaves in order.
        for tree in ["(日本語のテスト, 한국어이름)", "(Homo sapiens, 日本語のテスト, 한국어이름, 🦕🦖, B)"] {
            let names = leaves(&parse(tree).unwrap());
            for size in ["Small", "Medium", "Large"] {
                let svg = render_of(&[("tree", Value::S(tree.to_string())), ("label_size", Value::S(size.to_string()))]);
                let (header, rows) = order_list(&svg);
                assert_eq!(header.as_deref(), Some("Left to right:"), "{tree:?} at {size}:\n{}", texts(&svg));
                for (_, body) in &rows {
                    let classes: std::collections::BTreeSet<u8> = body.chars().filter(|c| !joins_previous(*c)).map(script_class).collect();
                    assert_eq!(classes.len(), 1, "{tree:?} at {size}: {body:?} mixes scripts {classes:?}");
                }
                assert_eq!(rows.iter().map(|r| r.1.clone()).collect::<Vec<_>>(), names, "{tree:?} at {size}: the rows do not spell the leaves in order");
            }
        }
    }

    /// The order list under a non-Latin tree: the "Left to right:" header, then (number, name) per row, read from the
    /// drawn elements below the count line, a row's runs joined in x order.
    fn order_list(svg: &str) -> (Option<String>, Vec<(String, String)>) {
        let bs = boxes(svg);
        let yc = bs.iter().find(|b| b.4.contains("leaves,") || b.4.contains("leaf,")).map(|b| b.1).unwrap_or(f64::MAX);
        let mut lines: Vec<(f64, Vec<(f64, String, String)>)> = Vec::new();
        for (x, y, _s, a, t) in &bs {
            if *y > yc + 1.0 {
                match lines.iter_mut().find(|(ly, _)| (*ly - *y).abs() < 0.01) {
                    Some((_, v)) => v.push((*x, a.clone(), t.clone())),
                    None => lines.push((*y, vec![(*x, a.clone(), t.clone())])),
                }
            }
        }
        lines.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut header = None;
        let mut rows = Vec::new();
        for (k, (_, mut els)) in lines.into_iter().enumerate() {
            els.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            if k == 0 { header = Some(els.iter().map(|e| e.2.as_str()).collect::<String>()); continue; }
            let num = els.iter().find(|e| e.1 == "end").map(|e| e.2.clone()).unwrap_or_default();
            let body: String = els.iter().filter(|e| e.1 != "end").map(|e| e.2.as_str()).collect();
            rows.push((num, body));
        }
        (header, rows)
    }

    fn canvas_w(svg: &str) -> f64 {
        let at = svg.find("width=\"").expect("width") + 7;
        svg[at..].split('"').next().unwrap().parse::<f64>().unwrap()
    }
    fn canvas_h(svg: &str) -> f64 {
        let at = svg.find("height=\"").expect("height") + 8;
        svg[at..].split('"').next().unwrap().parse::<f64>().unwrap()
    }

    /// Every element inside the canvas, measured by the larger of the estimate (+5 %) and the conservative width.
    fn assert_fits(ctx: &str, svg: &str) {
        let (w, h) = (canvas_w(svg), canvas_h(svg));
        for (x, y, sz, anchor, text) in boxes(svg) {
            let n = (text_w(&text, sz) * 1.05).max(safe_w(&text, sz));
            let (l, r) = match anchor.as_str() { "end" => (x - n, x), "middle" => (x - n / 2.0, x + n / 2.0), _ => (x, x + n) };
            assert!(l >= -1.0 && r <= w + 1.0, "{ctx}: {text:?} spans {l:.1}..{r:.1} on a {w} canvas");
            assert!(y - sz * 0.8 >= -1.0 && y + sz * 0.25 <= h + 1.0, "{ctx}: {text:?} at y {y} on a {h} canvas");
        }
    }

    #[test]
    fn a_name_that_is_not_latin_gets_its_own_row_in_the_order_list() {
        // (round 8 majors 2 and 4, minors 2, 3 and 8) Arabic was lumped with Latin, so an Arabic tree's order line was
        // all tofu; an RTL pair was reordered by bidi inside the Latin run; runs placed at an estimated cumulative x
        // overlapped ("polyphemus,日本語") or left holes ("Canis lupus,    日本語"); a joined emoji split into three
        // faces; and the comma at a wrap break vanished. Any non-Latin name now puts every name in its own element on
        // its own numbered row, each drawn exactly as its leaf is.
        let trees = [
            "((إنسان, شمبانزي), غوريلا)",
            "((אדם, שימפנזה), גורילה)",
            "((שלום, עולם), Dog)",
            "((Русский, Ελληνικά), (עברית, العربية), (ภาษาไทย, Ñandú), 🦕🦖)",
            "(Hippopotamus amphibius, Bubo bubo, Gopherus polyphemus, 日本語, 한국어)",
            "(Mus musculus, Rattus norvegicus, Felis catus, Canis lupus, 日本語, 한국어)",
            "(((👨\u{200d}👩\u{200d}👧, ❤\u{fe0f}), (☀ sun, ✈ plane)), (🇯🇵, 👍🏽))",
            "((日本한국, 中文Latin), (Русский, Ελληνικά), (עברית, العربية), (ภาษาไทย, Ñandú), 🦕🦖)",
            "(हिन्दी, English)",
            "(Homo sapiens, 日本語のテスト, 한국어이름, 🦕🦖, B)",
        ];
        for tree in trees {
            let names = leaves(&parse(tree).unwrap());
            for size in ["Small", "Medium", "Large"] {
                for style in ["Colour", "Print"] {
                    for shape in ["rectangular", "slanted"] {
                        let ctx = format!("{tree:?} {size} {style} {shape}");
                        let svg = render_of(&[s("tree", tree), s("label_size", size), s("style", style), s("shape", shape)]);
                        let (header, rows) = order_list(&svg);
                        assert_eq!(header.as_deref(), Some("Left to right:"), "{ctx}:\n{}", texts(&svg));
                        assert_eq!(rows.iter().map(|r| r.1.clone()).collect::<Vec<_>>(), names, "{ctx}");
                        for (i, (num, _)) in rows.iter().enumerate() { assert_eq!(num, &format!("{}.", i + 1), "{ctx}"); }
                        // the header is its own element: no name shares the Latin prefix's element
                        let bs = boxes(&svg);
                        assert!(bs.iter().any(|b| b.4 == "Left to right:"), "{ctx}");
                        assert!(!bs.iter().any(|b| b.4.starts_with("Left to right:") && b.4 != "Left to right:"), "{ctx}");
                        // every name's row starts at one fixed x: nothing is placed by a cumulative width
                        let ys: Vec<f64> = { let mut v: Vec<f64> = bs.iter().map(|b| b.1).collect(); v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v };
                        let last_y = *ys.last().unwrap();
                        let name_xs: Vec<f64> = bs.iter().filter(|b| b.3 == "start" && b.1 > last_y - (names.len() as f64) * 17.5 * svg::label_factor(size) && b.4 != "Left to right:").map(|b| b.0).collect();
                        let x0 = name_xs.iter().cloned().fold(f64::MAX, f64::min);
                        for n in &names {
                            if needs_runs(n) { continue; }
                            assert!(bs.iter().any(|b| b.4 == *n && (b.0 - x0).abs() < 0.01), "{ctx}: {n:?} is not its own element at x {x0}");
                        }
                        assert_fits(&ctx, &svg);
                    }
                }
            }
        }
        // a joined emoji stays one element, in the leaf and in the list
        let svg = render_of(&[s("tree", "(((👨\u{200d}👩\u{200d}👧, ❤\u{fe0f}), (☀ sun, ✈ plane)), (🇯🇵, 👍🏽))")]);
        assert_eq!(boxes(&svg).iter().filter(|b| b.4 == "👨\u{200d}👩\u{200d}👧").count(), 2, "{}", texts(&svg));
        assert_eq!(boxes(&svg).iter().filter(|b| b.4 == "👍🏽").count(), 2, "{}", texts(&svg));
        // (round 9) ❤ is a dingbat, which the Latin face does not carry: it is its own run now, after the separator
        assert_eq!(script_runs("👨\u{200d}👩\u{200d}👧, ❤\u{fe0f}"), vec!["👨\u{200d}👩\u{200d}👧", ", ", "❤\u{fe0f}"]);
        // the classifier no longer lumps RTL and other alphabets with Latin
        for c in ['ا', 'א', 'ก', 'ह'] { assert!(!latin_class(c) && script_class(c) != 0, "{c}"); }
        // (round 9) ☀ moved to the non-Latin side with the other symbol blocks (see the round-9 boundary test)
        for c in ['A', 'ñ', 'Ж', 'λ', '–'] { assert!(latin_class(c), "{c}"); }
        assert!(!latin_class('☀'));
    }

    #[test]
    fn a_very_long_non_latin_list_is_capped_and_fits() {
        let tree = format!("({}, 日本)", (1..=60).map(|i| format!("S{i}")).collect::<Vec<_>>().join(", "));
        for size in ["Small", "Medium", "Large"] {
            let svg = render_of(&[s("tree", &tree), s("label_size", size)]);
            let (_, rows) = order_list(&svg);
            assert_eq!(rows.len(), MAX_ORDER_ROWS, "{size}");
            assert!(rows.last().unwrap().1.contains("and 22 more"), "{size}: {:?}", rows.last());
            assert_fits(size, &svg);
        }
    }

    #[test]
    fn a_wrapped_latin_order_line_keeps_its_comma() {
        // (round 8 minor) "… Bubo bubo" / "Gopherus polyphemus …" with the comma lost at the break
        let tree = "(Hippopotamus amphibius, Bubo bubo, Gopherus polyphemus, Panthera leo, Canis lupus familiaris, Felis silvestris catus, Mus musculus)";
        for size in ["Small", "Medium", "Large"] {
            let svg = render_of(&[s("tree", tree), s("label_size", size)]);
            let t = texts(&svg);
            let lines: Vec<&str> = t.lines().skip_while(|l| !l.starts_with("Left to right:")).collect();
            assert!(lines.len() >= 2, "{size}: the fixture does not wrap: {t}");
            for l in &lines[..lines.len() - 1] { assert!(l.ends_with(','), "{size}: {l:?} drops the separator at the break"); }
            assert!(!lines.last().unwrap().ends_with(','), "{size}");
            assert_eq!(lines.join(" ").replace("Left to right: ", ""), leaves(&parse(tree).unwrap()).join(", "), "{size}");
            assert_fits(size, &svg);
        }
        // (round 8 minor) o/p/b/d-heavy names are measured conservatively, so the line does not clip at the edge
        for size in ["Small", "Medium", "Large"] {
            for shape in ["rectangular", "slanted"] {
                let svg = render_of(&[s("tree", "(Mogonmono, Homodo hopudro, Gopuobpoop, Buhydbobopo)"), s("label_size", size), s("shape", shape)]);
                assert_fits(&format!("{size} {shape}"), &svg);
            }
        }
    }

    #[test]
    fn a_name_mixing_han_and_hangul_is_drawn_as_runs_in_the_leaf_and_the_ancestor() {
        // (round 8 major) one `<text>` holding Han and Hangul printed "日本ㅎㅏㄴㄱㅜㄱ" for the leaf 日本한국, and the
        // same for an ancestor, whose wider jamo ran over its branch
        for (tree, name) in [("((日本한국, 中文Latin), Dog)", "日本한국"), ("((Human, Chimp)日本한국, Gorilla)", "日本한국"), ("((Human, Chimp)日本한국, (日本, 한국)東亜)", "日本한국")] {
            for size in ["Small", "Medium", "Large"] {
                for shape in ["rectangular", "slanted"] {
                    let svg = render_of(&[s("tree", tree), s("label_size", size), s("shape", shape)]);
                    let bs = boxes(&svg);
                    assert!(!bs.iter().any(|b| b.4 == name), "{tree} {size} {shape}: {name} is still one element");
                    let runs: Vec<_> = bs.iter().filter(|b| b.4 == "日本" || b.4 == "한국").collect();
                    assert!(runs.len() >= 2, "{tree} {size} {shape}: {:?}", texts(&svg));
                    for b in &bs {
                        let classes: std::collections::BTreeSet<u8> = b.4.chars().filter(|c| !joins_previous(*c)).map(script_class).filter(|k| *k != 0).collect();
                        assert!(classes.len() <= 1, "{tree} {size} {shape}: {:?} mixes scripts", b.4);
                    }
                    // the ancestor's runs end where the single label ended, clear of the parent's bar
                    if tree.contains(")日本한국") {
                        let n = parse(tree).unwrap();
                        let pl = place(&n);
                        let (_, x, _) = pl.nodes.iter().find(|nd| nd.0 == name).unwrap().clone();
                        let lab = svg::label_factor(size);
                        let root_w = text_w(&n.label, 11.0 * lab);
                        let widest_anc = ancestor_labels(&n).iter().map(|l| text_w(l, 11.0 * lab)).fold(0.0f64, f64::max);
                        let step = (STEP_X * lab).max(widest_anc + 30.0);
                        let fork = 30.0 + root_w + x as f64 * step;
                        let parent_bar = fork - step;
                        let first = bs.iter().filter(|b| b.4 == "日本").map(|b| b.0).fold(f64::MAX, f64::min);
                        assert!(first > parent_bar + 10.0, "{tree} {size} {shape}: the ancestor label starts at {first}, over the bar at {parent_bar}");
                    }
                    assert_fits(&format!("{tree} {size} {shape}"), &svg);
                }
            }
        }
    }

    #[test]
    fn a_slanted_ancestor_label_is_not_struck_through_by_its_own_branch() {
        // (round 8 major) the label always sat above-left of its fork, exactly where the parent's diagonal arrives at a
        // lower child: "Others" had the diagonal through it
        for tree in [
            "((Human, Chimp)Hominini, (Gorilla, Orangutan)Others)",
            "((Human, Chimp)Great apes, (日本, 한국)東亜)",
            "(((A, B)Lower, C)Middle, ((D, E)Upper, F)Again)",
            "((A, B)One, (C, D)Two, (E, F)Three)",
        ] {
            let n = parse(tree).unwrap();
            for size in ["Small", "Medium", "Large"] {
                for style in ["Colour", "Print"] {
                    let svg = render_of(&[s("tree", tree), s("shape", "slanted"), s("label_size", size), s("style", style)]);
                    let segs = lines(&svg);
                    for label in ancestor_labels(&n) {
                        // haloed: an outline copy is drawn under the fill
                        assert!(svg.contains(&format!(r##"stroke-linejoin="round">{}</text>"##, svg::esc(&label))), "{tree} {size}: {label} has no halo");
                        for (x, y, sz, anchor, text) in boxes(&svg).into_iter().filter(|b| b.4 == label) {
                            let w = text_w(&text, sz);
                            let (l, r) = if anchor == "end" { (x - w, x) } else { (x, x + w) };
                            let b = (l, r, y - sz * 0.75, y + sz * 0.2);
                            for sg in &segs {
                                assert!(seg_in_box(*sg, b) == 0.0, "{tree} {size} {style}: {label:?} {b:?} is crossed by the branch {sg:?}");
                            }
                        }
                    }
                }
            }
        }
        // the rectangular tree is unchanged: no halo, the label above-left of the fork
        let rect = render_of(&[s("tree", "((Human, Chimp)Great apes, Gorilla)")]);
        assert!(!rect.contains(r##"stroke="#fff""##), "the rectangular tree changed");
    }

    #[test]
    fn a_latin_tree_renders_byte_for_byte_as_before() {
        // (round 8) the order-line rework applies only when a name is not Latin: a Latin tree's SVG must be exactly what
        // the frozen round-7 build (80b2eb1) drew, every size, style and shape of the examples. The hashes are FNV-1a 64
        // of those frozen renders, the census's own hash (the Medium Colour default is the census entry). Expected
        // exceptions, not listed: a slanted labelled ancestor (halo, placement), a wrapped line (its comma), and a
        // name the conservative width says could clip (the canvas widens).
        fn fnv64(s: &str) -> u64 {
            let mut h: u64 = 0xcbf29ce484222325;
            for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
            h
        }
        let frozen: &[(&str, &str, &str, &str, u64)] = &[
            ("(((Human, Chimp), Gorilla), Orangutan)", "Small", "Colour", "rectangular", 0xaaf7bf8633d380b3),
            ("(((Human, Chimp), Gorilla), Orangutan)", "Small", "Print", "rectangular", 0xb9efce3e4487c077),
            ("(((Human, Chimp), Gorilla), Orangutan)", "Small", "Colour", "slanted", 0x70d7bdcce8780f6a),
            ("(((Human, Chimp), Gorilla), Orangutan)", "Small", "Print", "slanted", 0x7064d137c578f396),
            ("(((Human, Chimp), Gorilla), Orangutan)", "Medium", "Colour", "rectangular", 0x4041fa1d6b9d93f9),
            ("(((Human, Chimp), Gorilla), Orangutan)", "Medium", "Print", "rectangular", 0x593abe211d8dc2b5),
            ("(((Human, Chimp), Gorilla), Orangutan)", "Medium", "Colour", "slanted", 0xc9c65aec02c60842),
            ("(((Human, Chimp), Gorilla), Orangutan)", "Medium", "Print", "slanted", 0x8ebdf29a1544db86),
            ("(((Human, Chimp), Gorilla), Orangutan)", "Large", "Colour", "rectangular", 0x834922361c831abf),
            ("(((Human, Chimp), Gorilla), Orangutan)", "Large", "Print", "rectangular", 0x6026bb6278e65017),
            ("(((Human, Chimp), Gorilla), Orangutan)", "Large", "Colour", "slanted", 0x1e3c43121fa809aa),
            ("(((Human, Chimp), Gorilla), Orangutan)", "Large", "Print", "slanted", 0x5140bcc0e22775aa),
            ("(((Lizard, Bird), (Frog, Fish)), Lamprey)", "Small", "Colour", "rectangular", 0xa971bd086723df55),
            ("(((Lizard, Bird), (Frog, Fish)), Lamprey)", "Small", "Print", "rectangular", 0xab044c6b36660693),
            ("(((Lizard, Bird), (Frog, Fish)), Lamprey)", "Small", "Colour", "slanted", 0x014db5c0adeedf15),
            ("(((Lizard, Bird), (Frog, Fish)), Lamprey)", "Small", "Print", "slanted", 0xf0209f04b567d953),
            ("(((Lizard, Bird), (Frog, Fish)), Lamprey)", "Medium", "Colour", "rectangular", 0x2784d3aa37522a5d),
            ("(((Lizard, Bird), (Frog, Fish)), Lamprey)", "Medium", "Print", "rectangular", 0x8cff41a8165d1c2b),
            ("(((Lizard, Bird), (Frog, Fish)), Lamprey)", "Medium", "Colour", "slanted", 0xaa33b7a4ba12f599),
            ("(((Lizard, Bird), (Frog, Fish)), Lamprey)", "Medium", "Print", "slanted", 0x9caf767a3ae4ce37),
            ("(((Lizard, Bird), (Frog, Fish)), Lamprey)", "Large", "Colour", "rectangular", 0x31efd4c6f7c34910),
            ("(((Lizard, Bird), (Frog, Fish)), Lamprey)", "Large", "Print", "rectangular", 0x78925f4ea6c9dc2e),
            ("(((Lizard, Bird), (Frog, Fish)), Lamprey)", "Large", "Colour", "slanted", 0x7b4a9b6b09fb5606),
            ("(((Lizard, Bird), (Frog, Fish)), Lamprey)", "Large", "Print", "slanted", 0x31fc76cb9b93ee24),
            ("(Red, Blue, Green, Yellow)", "Small", "Colour", "rectangular", 0x8a1b7f7dac9651f9),
            ("(Red, Blue, Green, Yellow)", "Small", "Print", "rectangular", 0xb645fbd9c209e875),
            ("(Red, Blue, Green, Yellow)", "Small", "Colour", "slanted", 0x51b7c9352efb7755),
            ("(Red, Blue, Green, Yellow)", "Small", "Print", "slanted", 0x6d1907e22b024501),
            ("(Red, Blue, Green, Yellow)", "Medium", "Colour", "rectangular", 0x6a4baad1f4b75613),
            ("(Red, Blue, Green, Yellow)", "Medium", "Print", "rectangular", 0x984cd94738f7da17),
            ("(Red, Blue, Green, Yellow)", "Medium", "Colour", "slanted", 0xf5317667ef606d10),
            ("(Red, Blue, Green, Yellow)", "Medium", "Print", "slanted", 0x15f5fa3b0952aa54),
            ("(Red, Blue, Green, Yellow)", "Large", "Colour", "rectangular", 0x5da0bd4a1136e30e),
            ("(Red, Blue, Green, Yellow)", "Large", "Print", "rectangular", 0x994d85f6df66800e),
            ("(Red, Blue, Green, Yellow)", "Large", "Colour", "slanted", 0x62fd7f9537e4a8af),
            ("(Red, Blue, Green, Yellow)", "Large", "Print", "slanted", 0xcf7e7ce1fca40da7),
            ("((Human, Chimp)Great apes, Gorilla)", "Small", "Colour", "rectangular", 0xab62decb939b7b09),
            ("((Human, Chimp)Great apes, Gorilla)", "Small", "Print", "rectangular", 0x261a6b50f5c1d9f7),
            ("((Human, Chimp)Great apes, Gorilla)", "Medium", "Colour", "rectangular", 0x439d6f31abe7bb1e),
            ("((Human, Chimp)Great apes, Gorilla)", "Medium", "Print", "rectangular", 0x2a19485f5fbccb94),
            ("((Human, Chimp)Great apes, Gorilla)", "Large", "Colour", "rectangular", 0x3a0bdb026cc22741),
            ("((Human, Chimp)Great apes, Gorilla)", "Large", "Print", "rectangular", 0xba00656302f5702f),
            ("((A, B), (C, D));", "Small", "Colour", "rectangular", 0x349ca13e0e19ab69),
            ("((A, B), (C, D));", "Small", "Print", "rectangular", 0x98ca4fbac2c06831),
            ("((A, B), (C, D));", "Small", "Colour", "slanted", 0xdf54631db555a592),
            ("((A, B), (C, D));", "Small", "Print", "slanted", 0x8a2037f0be3749a2),
            ("((A, B), (C, D));", "Medium", "Colour", "rectangular", 0x0a09a2caeeb7fc2d),
            ("((A, B), (C, D));", "Medium", "Print", "rectangular", 0xf4dc9bc3d72236f5),
            ("((A, B), (C, D));", "Medium", "Colour", "slanted", 0xe1191458af61e9ec),
            ("((A, B), (C, D));", "Medium", "Print", "slanted", 0x9d898227e8348dec),
            ("((A, B), (C, D));", "Large", "Colour", "rectangular", 0x71dc8a97d55242ac),
            ("((A, B), (C, D));", "Large", "Print", "rectangular", 0x350a2955a06dd218),
            ("((A, B), (C, D));", "Large", "Colour", "slanted", 0x7cacdb9647cdc0f7),
            ("((A, B), (C, D));", "Large", "Print", "slanted", 0xfd4b09f819c87bfb),
            ("(A, B, C, D, E, F, G, H)", "Small", "Colour", "rectangular", 0x24592e6a4abf7082),
            ("(A, B, C, D, E, F, G, H)", "Small", "Print", "rectangular", 0xf0a833d5d15633ea),
            ("(A, B, C, D, E, F, G, H)", "Small", "Colour", "slanted", 0x738c62f1eda6e180),
            ("(A, B, C, D, E, F, G, H)", "Small", "Print", "slanted", 0xd442ca637d766a40),
            ("(A, B, C, D, E, F, G, H)", "Medium", "Colour", "rectangular", 0x92a74ff48774f0dc),
            ("(A, B, C, D, E, F, G, H)", "Medium", "Print", "rectangular", 0x665648daf35941ac),
            ("(A, B, C, D, E, F, G, H)", "Medium", "Colour", "slanted", 0xc0deddceddf67090),
            ("(A, B, C, D, E, F, G, H)", "Medium", "Print", "slanted", 0xe79570c06ea5e010),
            ("(A, B, C, D, E, F, G, H)", "Large", "Colour", "rectangular", 0x736780cce7fda399),
            ("(A, B, C, D, E, F, G, H)", "Large", "Print", "rectangular", 0x586b9696ba4b2b6d),
            ("(A, B, C, D, E, F, G, H)", "Large", "Colour", "slanted", 0x250b31a3e830e059),
            ("(A, B, C, D, E, F, G, H)", "Large", "Print", "slanted", 0x4bf1aa78f175642d),
            ("(((((A, B), C), D), E), F)", "Small", "Colour", "rectangular", 0x5586d7be9ef14f53),
            ("(((((A, B), C), D), E), F)", "Small", "Print", "rectangular", 0x869fe682125302f3),
            ("(((((A, B), C), D), E), F)", "Small", "Colour", "slanted", 0xd5778333d44a6784),
            ("(((((A, B), C), D), E), F)", "Small", "Print", "slanted", 0x4b9a9a95d8e74a6c),
            ("(((((A, B), C), D), E), F)", "Medium", "Colour", "rectangular", 0x5c2f327a7c04d888),
            ("(((((A, B), C), D), E), F)", "Medium", "Print", "rectangular", 0xe3eaa17c1d92a89c),
            ("(((((A, B), C), D), E), F)", "Medium", "Colour", "slanted", 0xa7979e7469961517),
            ("(((((A, B), C), D), E), F)", "Medium", "Print", "slanted", 0xe6a2d2af56651593),
            ("(((((A, B), C), D), E), F)", "Large", "Colour", "rectangular", 0xb2091346769e9bcd),
            ("(((((A, B), C), D), E), F)", "Large", "Print", "rectangular", 0xd0d6c0cb1a39d15d),
            ("(((((A, B), C), D), E), F)", "Large", "Colour", "slanted", 0x62fdf88cc5cdddf6),
            ("(((((A, B), C), D), E), F)", "Large", "Print", "slanted", 0x40b9fb2618235e7e),
            ("(A, B", "Small", "Colour", "rectangular", 0x3b7eb918f2266409),
            ("(A, B", "Small", "Print", "rectangular", 0x6716b5c40d1212ae),
            ("(A, B", "Small", "Colour", "slanted", 0x3b7eb918f2266409),
            ("(A, B", "Small", "Print", "slanted", 0x6716b5c40d1212ae),
            ("(A, B", "Medium", "Colour", "rectangular", 0x2a0bb0a022a2b428),
            ("(A, B", "Medium", "Print", "rectangular", 0x21debb369b93b0fd),
            ("(A, B", "Medium", "Colour", "slanted", 0x2a0bb0a022a2b428),
            ("(A, B", "Medium", "Print", "slanted", 0x21debb369b93b0fd),
            ("(A, B", "Large", "Colour", "rectangular", 0xa630f328140185ba),
            ("(A, B", "Large", "Print", "rectangular", 0x86148b13dd6d1a29),
            ("(A, B", "Large", "Colour", "slanted", 0xa630f328140185ba),
            ("(A, B", "Large", "Print", "slanted", 0x86148b13dd6d1a29),
        ];
        for (tree, size, style, shape, want) in frozen {
            let svg = render_of(&[s("tree", tree), s("label_size", size), s("style", style), s("shape", shape)]);
            assert_eq!(fnv64(&svg), *want, "{tree:?} {size} {style} {shape} no longer renders as the frozen build did:\n{svg}");
        }
    }

    #[test]
    fn a_tree_cut_at_the_text_cap_says_it_is_too_long() {
        // (round 6 minor) the shared 500-character text cap cut the typed tree mid-name, and the page blamed the
        // brackets. It must name the real cause.
        let long = format!("({})", (0..130).map(|i| format!("S{i}")).collect::<Vec<_>>().join(", "));
        let p = with(&[s("tree", &long)]);
        assert_eq!(p.s("tree").chars().count(), MAX_TREE_CHARS, "the fixture is not cut at the cap");
        let t = texts(&render_of(&[s("tree", &long)]));
        assert!(t.contains("The tree is too long"), "{t}");
        assert!(!t.contains("A bracket is never closed"), "{t}");
        assert!(!t.contains("until the brackets balance"), "{t}");
        assert!(warnings(&p).iter().any(|w| w.contains("too long")), "{:?}", warnings(&p));
        // a tree under the cap still draws, and a genuinely unbalanced one still blames the brackets
        let ok = texts(&render_of(&[s("tree", "(A, B)")]));
        assert!(ok.contains("2 leaves"), "{ok}");
        assert!(texts(&render_of(&[s("tree", "(A, B")])).contains("A bracket is never closed"), "{}", texts(&render_of(&[s("tree", "(A, B")])));
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

    /// Writes every case (Colour and Print, both shapes) to $VERSHION_PNG_DIR.
    #[test]
    fn cladogram_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let s2 = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]),
            ("large".into(), vec![s2("label_size", "Large")]),
            ("slanted".into(), vec![s2("shape", "slanted")]),
            ("wide".into(), vec![s2("tree", "(A, B, C, D, E, F, G, H)")]),
            ("deep".into(), vec![s2("tree", "(((((A, B), C), D), E), F)")]),
            ("named".into(), vec![s2("tree", "((Homo sapiens, Pan troglodytes)Homininae, Gorilla gorilla)Hominidae")]),
            ("spaces".into(), vec![s2("tree", "(Homo sapiens, Pan troglodytes, Gorilla gorilla)")]),
            ("bad".into(), vec![s2("tree", "(A, B")]),
        ];
        for (idx, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{idx}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s2("style", "Print")])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/clado-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/clado-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
