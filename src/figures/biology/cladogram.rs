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
    s.chars()
        .map(|c| {
            size * match c {
                'W' | 'M' | 'm' | 'w' => 0.95,
                'A'..='Z' => 0.72,
                '0'..='9' => 0.58,
                ' ' => 0.28,
                'i' | 'j' | 'l' | 't' | 'f' | 'r' | '.' | ',' | ';' | ':' | '\'' | '!' | '|' => 0.32,
                _ => 0.56,
            }
        })
        .sum()
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
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() { cur.push_str(", "); }
        cur.push_str(part);
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

pub fn warnings(p: &Params) -> Vec<String> {
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

pub fn build(p: &Params) -> String {
    let print = p.s("style") == "Print";
    let lab = svg::label_factor(p.s("label_size"));
    let slanted = p.s("shape") == "slanted";
    let ink = if print { "#555555" } else { INK };
    let branch = if print { "#333333" } else { BRANCH };
    let leaf_col = if print { "#111111" } else { LEAF };

    let node = parse(p.s("tree"));
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
    let top = 34.0;
    let h = top + ((n_leaves as f64) - 1.0).max(1.0) * LEAF_GAP * lab + 40.0 * lab;
    // (round 2) the text under the picture wraps to the CANVAS, not to a fixed character count; (round 5) to its width.
    let leaf_max = (w - 24.0 - prefix_w - 12.0).max(40.0);
    let err_max = (w - 48.0).max(60.0);
    let below = match &node {
        Ok(root) => leaf_order(root, leaf_max, 12.0 * lab).len().max(1),
        Err(e) => wrap_w(e, err_max, 13.0 * lab).len() + wrap_w(NO_TREE, err_max, 12.0 * lab).len(),
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
            for (label, x, y) in &placed.nodes {
                d.raw(&text_el(px(*x) - 5.0, py(*y) - 5.0, 11.0 * lab, "end", ink, label));
            }
            for (label, x, y) in &placed.leaves {
                d.raw(&text_el(px(*x) + 8.0, py(*y) + 4.5 * lab, 12.0 * lab, "start", leaf_col, label));
            }
        }
        Err(e) => {
            d.raw(&text_el(24.0, 46.0, 13.0 * lab, "start", if print { "#555555" } else { ERR }, "The tree could not be read:"));
            let mut y = 70.0 * lab;
            for line in wrap_w(e, err_max, 13.0 * lab) {
                d.raw(&text_el(24.0, y, 13.0 * lab, "start", ink, &line));
                y += 18.0 * lab;
            }
            // (round 2) after the reason, not at a fixed row it used to overprint; (round 4) wrapped like the reason,
            // which at Large ran off the 360 px canvas
            let mut ny = y + 8.0 * lab;
            for line in wrap_w(NO_TREE, err_max, 12.0 * lab) {
                d.raw(&text_el(24.0, ny, 12.0 * lab, "start", ink, &line));
                ny += 15.0 * lab;
            }
        }
    }

    // what the text was read as: the leaves, so a typo is visible without counting brackets
    let y = h + 8.0 * lab;
    if let Ok(root) = &node {
        let ls = leaves(root);
        if let Some(count) = &count_text {
            d.raw(&text_el(24.0, y, 12.0 * lab, "start", ink, count));
        }
        for (i, line) in leaf_order(root, leaf_max, 12.0 * lab).iter().enumerate() {
            // (round 3) the continuation lines are indented by x, not by leading spaces: SVG collapses those, so the
            // line used to start hard against the left margin
            let (x, text) = if i == 0 { (24.0, format!("Left to right: {line}")) } else { (24.0 + prefix_w, line.clone()) };
            d.raw(&text_el(x, y + (15.0 + i as f64 * 17.0) * lab, 12.0 * lab, "start", ink, &text));
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
