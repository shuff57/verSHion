//! Source pixel check for hidden edges in a geometry diagram.
//!
//! Printed solids draw hidden (back) edges dashed, often in faint gray. A model reading the
//! source can take faint dashes for a continuous line and author a SOLID gray segment, and a
//! model judge can then agree. This check does not trust either: it registers the candidate's
//! solid dark edges onto the reference image's ink, then samples the reference along each solid
//! gray segment and asks whether the ink there comes in separate runs with gaps between them.
//! Where it measures dashes under a solid gray edge (a segment, a polyline path or a line), the
//! agent sets that edge dashed itself and records why: the measurement decides, not the model.
//! An unclear reading (no registration, no stroke under the edge) changes and reports nothing.

use std::collections::BTreeMap;

use serde_json::Value as Json;

/// One drawn straight stroke in canvas coordinates, and the diagram element it belongs to
/// (`segments`, `paths` or `lines`, with its index): a polyline path is one stroke per leg.
struct Seg {
    a: (f64, f64),
    b: (f64, f64),
    from: String,
    to: String,
    solid: bool,
    color: String,
    owner: (&'static str, usize),
}

fn channels(c: &str) -> Option<[u8; 3]> {
    let h = c.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v: Vec<u8> = (0..3).filter_map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).ok()).collect();
    (v.len() == 3).then(|| [v[0], v[1], v[2]])
}

/// Black or near black: a visible edge to register on.
fn dark(c: &str) -> bool {
    channels(c).is_some_and(|v| v.iter().all(|x| *x <= 0x40))
}

/// A neutral mid gray, the colour models give a hidden edge (matches the semantic report's rule).
fn mid_gray(c: &str) -> bool {
    channels(c).is_some_and(|v| v.iter().max().unwrap() - v.iter().min().unwrap() <= 8 && (0x60..=0xb0).contains(&v[0]))
}

fn diagram_of(params: &BTreeMap<String, Json>) -> Option<Json> {
    match params.get("diagram")? {
        Json::String(s) => serde_json::from_str(s).ok(),
        other => Some(other.clone()),
    }
}

fn segments(params: &BTreeMap<String, Json>) -> Option<((f64, f64), Vec<Seg>)> {
    let size = (params.get("width")?.as_f64()?, params.get("height")?.as_f64()?);
    let diagram = diagram_of(params)?;
    let nodes = diagram.get("nodes")?.as_object()?;
    let at = |name: &str| -> Option<(f64, f64)> {
        let v = nodes.get(name)?.as_array()?;
        Some((v.first()?.as_f64()?, v.get(1)?.as_f64()?))
    };
    let style_of = |e: &Json| e.get("style").and_then(Json::as_str).unwrap_or("solid") == "solid";
    let color_of = |e: &Json| e.get("color").and_then(Json::as_str).unwrap_or("#000000").to_string();
    let mut out = Vec::new();
    for (i, s) in diagram.get("segments").and_then(Json::as_array).into_iter().flatten().enumerate() {
        let (Some(from), Some(to)) = (s.get("from").and_then(Json::as_str), s.get("to").and_then(Json::as_str)) else { continue };
        // Ports such as rim:top are derived positions; only literal nodes are sampled.
        let (Some(a), Some(b)) = (at(from), at(to)) else { continue };
        out.push(Seg { a, b, from: from.into(), to: to.into(), solid: style_of(s), color: color_of(s), owner: ("segments", i) });
    }
    // A polyline is straight legs between its nodes; curved paths are not sampled.
    for (i, p) in diagram.get("paths").and_then(Json::as_array).into_iter().flatten().enumerate() {
        if p.get("kind").and_then(Json::as_str).unwrap_or("polyline") != "polyline" {
            continue;
        }
        let names: Vec<&str> = p.get("nodes").and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).collect();
        for w in names.windows(2) {
            let (Some(a), Some(b)) = (at(w[0]), at(w[1])) else { continue };
            out.push(Seg { a, b, from: w[0].into(), to: w[1].into(), solid: style_of(p), color: color_of(p), owner: ("paths", i) });
        }
    }
    for (i, l) in diagram.get("lines").and_then(Json::as_array).into_iter().flatten().enumerate() {
        let (Some(through), Some(toward)) = (l.get("through").and_then(Json::as_str), l.get("toward").and_then(Json::as_str)) else { continue };
        let (Some(o), Some(t)) = (at(through), at(toward)) else { continue };
        let len = ((t.0 - o.0).powi(2) + (t.1 - o.1).powi(2)).sqrt();
        if len < 1e-9 {
            continue;
        }
        let (ux, uy) = ((t.0 - o.0) / len, (t.1 - o.1) / len);
        let back = l.get("back").and_then(Json::as_f64).unwrap_or(0.0);
        let forward = l.get("forward").and_then(Json::as_f64).unwrap_or(0.0);
        let id = l.get("id").and_then(Json::as_str).unwrap_or("line");
        out.push(Seg { a: (o.0 - ux * back, o.1 - uy * back), b: (o.0 + ux * forward, o.1 + uy * forward), from: format!("{id}:back"), to: format!("{id}:forward"), solid: style_of(l), color: color_of(l), owner: ("lines", i) });
    }
    Some((size, out))
}

/// The reference as luma, and the brightness of its paper.
struct Page {
    luma: image::GrayImage,
    paper: u8,
}

impl Page {
    fn new(png: &[u8]) -> Option<Page> {
        let luma = image::load_from_memory(png).ok()?.to_luma8();
        let mut values: Vec<u8> = luma.pixels().map(|p| p.0[0]).collect();
        if values.len() < 100 {
            return None;
        }
        values.sort_unstable();
        // The paper is the bright majority.
        let paper = values[values.len() * 3 / 4];
        Some(Page { luma, paper })
    }

    /// The darkest pixel across the line at (x, y): a thin or antialiased stroke a pixel off the
    /// sampled centreline still counts, while a gap along the line stays a gap.
    fn across(&self, x: f64, y: f64, normal: (f64, f64)) -> u8 {
        let (w, h) = (self.luma.width() as i64, self.luma.height() as i64);
        [-1.5, -0.75, 0.0, 0.75, 1.5]
            .iter()
            .filter_map(|o| {
                let (px, py) = ((x + normal.0 * o).round() as i64, (y + normal.1 * o).round() as i64);
                (px >= 0 && py >= 0 && px < w && py < h).then(|| self.luma.get_pixel(px as u32, py as u32).0[0])
            })
            .min()
            .unwrap_or(255)
    }
}

/// Canvas to reference: x' = sx·x + tx, y' = sy·y + ty.
#[derive(Clone, Copy, Debug)]
struct Fit {
    sx: f64,
    sy: f64,
    tx: f64,
    ty: f64,
}

impl Fit {
    fn map(&self, p: (f64, f64)) -> (f64, f64) {
        (self.sx * p.0 + self.tx, self.sy * p.1 + self.ty)
    }
}

/// Points along a segment between fractions `lo` and `hi` of its length, about one per pixel,
/// with the segment's unit normal.
fn samples(fit: &Fit, s: &Seg, lo: f64, hi: f64) -> (Vec<(f64, f64)>, (f64, f64)) {
    let (a, b) = (fit.map(s.a), fit.map(s.b));
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(1e-9);
    let n = (len * (hi - lo)).round().max(2.0) as usize;
    let pts = (0..=n).map(|i| lo + (hi - lo) * i as f64 / n as f64).map(|t| (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))).collect();
    (pts, (-(b.1 - a.1) / len, (b.0 - a.0) / len))
}

/// Share of the anchors' length that lands on clear ink.
fn coverage(page: &Page, fit: &Fit, anchors: &[&Seg]) -> f64 {
    let ink = page.paper.saturating_sub(55);
    let (mut hit, mut all) = (0usize, 0usize);
    for s in anchors {
        let (pts, normal) = samples(fit, s, 0.1, 0.9);
        for p in pts {
            all += 1;
            hit += usize::from(page.across(p.0, p.1, normal) < ink);
        }
    }
    if all == 0 { 0.0 } else { hit as f64 / all as f64 }
}

/// The scale and offset that best put the solid dark edges on the reference's ink, near the
/// canvas-to-image scale. None unless the edges really land on ink.
fn register(page: &Page, size: (f64, f64), anchors: &[&Seg]) -> Option<Fit> {
    let base = Fit { sx: page.luma.width() as f64 / size.0, sy: page.luma.height() as f64 / size.1, tx: 0.0, ty: 0.0 };
    let mut best = (coverage(page, &base, anchors), base);
    let (w, h) = (page.luma.width() as f64, page.luma.height() as f64);
    for scale in [0.94, 0.97, 1.0, 1.03, 1.06] {
        for ox in (-6..=6).map(|i| i as f64 * w * 0.01) {
            for oy in (-6..=6).map(|i| i as f64 * h * 0.01) {
                let fit = Fit { sx: base.sx * scale, sy: base.sy * scale, tx: ox + w * (1.0 - scale) / 2.0, ty: oy + h * (1.0 - scale) / 2.0 };
                let c = coverage(page, &fit, anchors);
                if c > best.0 {
                    best = (c, fit);
                }
            }
        }
    }
    // Without a convincing fit the source position of an edge is unknown: say nothing.
    (best.0 >= 0.85).then_some(best.1)
}

/// The stroke along the middle of a segment: (ink runs, share of samples on ink), with ink meaning
/// darker than halfway between the stroke's own darkest point and the paper. None when there is
/// no stroke there at all (the edge is not where the drawing put it).
fn profile(page: &Page, fit: &Fit, s: &Seg) -> Option<(usize, f64)> {
    let (pts, normal) = samples(fit, s, 0.15, 0.85);
    let luma: Vec<u8> = pts.iter().map(|p| page.across(p.0, p.1, normal)).collect();
    let darkest = *luma.iter().min()?;
    if darkest > page.paper.saturating_sub(40) {
        return None;
    }
    let half = ((darkest as u16 + page.paper as u16) / 2) as u8;
    let on: Vec<bool> = luma.iter().map(|v| *v < half).collect();
    let runs = on.iter().enumerate().filter(|(i, x)| **x && (*i == 0 || !on[i - 1])).count();
    let share = on.iter().filter(|x| **x).count() as f64 / on.len().max(1) as f64;
    Some((runs, share))
}

/// What the reference shows under one solid gray stroke.
struct Finding {
    owner: (&'static str, usize),
    from: String,
    to: String,
    color: String,
    runs: usize,
    share: f64,
}

impl Finding {
    /// A dashed stroke breaks into several separate runs with ink on part of the line.
    fn dashed(&self) -> bool {
        self.runs >= 4 && (0.15..=0.85).contains(&self.share)
    }
    /// A continuous stroke, even faint or antialiased, stays one run over nearly all of it.
    fn continuous(&self) -> bool {
        self.runs <= 2 && self.share >= 0.9
    }
}

fn measure(params: &BTreeMap<String, Json>, reference: &[u8]) -> Vec<Finding> {
    let Some((size, segs)) = segments(params) else { return Vec::new() };
    let gray: Vec<&Seg> = segs.iter().filter(|s| s.solid && mid_gray(&s.color)).collect();
    // Many gray lines are a grid, not hidden edges.
    if gray.is_empty() || gray.len() > 6 {
        return Vec::new();
    }
    let anchors: Vec<&Seg> = segs.iter().filter(|s| s.solid && dark(&s.color)).collect();
    if anchors.len() < 2 {
        return Vec::new();
    }
    let Some(page) = Page::new(reference) else { return Vec::new() };
    let Some(fit) = register(&page, size, &anchors) else { return Vec::new() };
    gray.iter()
        .filter_map(|s| {
            let (runs, share) = profile(&page, &fit, s)?;
            Some(Finding { owner: s.owner, from: s.from.clone(), to: s.to.clone(), color: s.color.clone(), runs, share })
        })
        .collect()
}

/// What the check measured, for the trace: why an edge was or was not corrected. None when the
/// drawing has no solid gray stroke to check.
pub fn diagnosis(params: &BTreeMap<String, Json>, reference: &[u8]) -> Option<String> {
    let (size, segs) = segments(params)?;
    let gray = segs.iter().filter(|s| s.solid && mid_gray(&s.color)).count();
    if gray == 0 {
        return None;
    }
    let anchors: Vec<&Seg> = segs.iter().filter(|s| s.solid && dark(&s.color)).collect();
    let Some(page) = Page::new(reference) else { return Some(format!("dash check: {gray} solid gray strokes; reference image unreadable")) };
    if gray > 6 || anchors.len() < 2 {
        return Some(format!("dash check: {gray} solid gray strokes, {} dark anchors; not measured (grid or too few anchors)", anchors.len()));
    }
    let base = Fit { sx: page.luma.width() as f64 / size.0, sy: page.luma.height() as f64 / size.1, tx: 0.0, ty: 0.0 };
    let Some(fit) = register(&page, size, &anchors) else {
        return Some(format!("dash check: {gray} solid gray strokes; registration failed (anchor ink coverage {:.0}% at the canvas scale, 85% needed)", coverage(&page, &base, &anchors) * 100.0));
    };
    let found: Vec<String> = segs.iter().filter(|s| s.solid && mid_gray(&s.color)).map(|s| match profile(&page, &fit, s) {
        Some((runs, share)) => format!("{}→{} {runs} runs {:.0}% ink", s.from, s.to, share * 100.0),
        None => format!("{}→{} no stroke under it", s.from, s.to),
    }).collect();
    Some(format!("dash check: registered (anchor coverage {:.0}%); {}", coverage(&page, &fit, &anchors) * 100.0, found.join("; ")))
}

fn describe(f: &Finding) -> String {
    let kind = match f.owner.0 { "paths" => "path", "lines" => "line", _ => "segment" };
    format!("{kind} {:?}→{:?} (solid {}: the reference's stroke there breaks into {} separate runs, ink on {:.0}% of its middle)", f.from, f.to, f.color, f.runs, f.share * 100.0)
}

/// Solid gray strokes the reference draws dashed, described; empty means no contradiction was
/// measured (not a source match).
pub fn solid_gray_over_dashes(params: &BTreeMap<String, Json>, reference: &[u8]) -> Vec<String> {
    measure(params, reference).iter().filter(|f| f.dashed()).map(|f| format!("Source dash check: {}.", describe(f))).collect()
}

/// The same parameters with every solid gray element measured dashed set to style "dashed", and
/// one note per element; None when nothing was measured dashed. A polyline path is changed only
/// when one of its legs measures dashed and none measures continuous.
pub fn dashed_corrections(params: &BTreeMap<String, Json>, reference: &[u8]) -> Option<(BTreeMap<String, Json>, Vec<String>)> {
    let found = measure(params, reference);
    let mut owners: Vec<(&'static str, usize)> = found.iter().filter(|f| f.dashed()).map(|f| f.owner).collect();
    owners.retain(|o| !found.iter().any(|f| f.owner == *o && f.continuous()));
    owners.sort();
    owners.dedup();
    if owners.is_empty() {
        return None;
    }
    let mut diagram = diagram_of(params)?;
    let mut notes = Vec::new();
    for (kind, i) in &owners {
        let element = diagram.get_mut(*kind)?.as_array_mut()?.get_mut(*i)?;
        element["style"] = Json::String("dashed".into());
        if let Some(f) = found.iter().find(|f| f.owner == (*kind, *i) && f.dashed()) {
            notes.push(format!("Set dashed by the source dash check: {}.", describe(f)));
        }
    }
    let mut out = params.clone();
    out.insert("diagram".into(), match params.get("diagram")? {
        Json::String(_) => Json::String(diagram.to_string()),
        _ => diagram,
    });
    Some((out, notes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reference with two black edges and one gray edge, drawn dashed or solid.
    fn reference(dashed: bool) -> Vec<u8> {
        let mut img = image::GrayImage::from_pixel(400, 300, image::Luma([255]));
        let mut line = |a: (f64, f64), b: (f64, f64), v: u8, dash: bool| {
            let n = 400;
            for i in 0..=n {
                let t = i as f64 / n as f64;
                if dash && (i / 20) % 2 == 1 {
                    continue;
                }
                let (x, y) = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
                for d in -1..=1 {
                    img.put_pixel(x.round() as u32, (y.round() as i64 + d) as u32, image::Luma([v]));
                }
            }
        };
        line((40.0, 40.0), (360.0, 40.0), 0, false);
        line((40.0, 40.0), (40.0, 260.0), 0, false);
        line((40.0, 260.0), (360.0, 260.0), 140, dashed);
        let mut out = Vec::new();
        image::DynamicImage::ImageLuma8(img).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    /// The candidate on a half-size canvas, its back edge solid gray.
    fn params() -> BTreeMap<String, Json> {
        let diagram = serde_json::json!({"nodes":{"A":[20,20],"B":[180,20],"C":[20,130],"D":[180,130]},
            "segments":[{"from":"A","to":"B","color":"#000000"},{"from":"A","to":"C","color":"#000000"},{"from":"C","to":"D","color":"#888888"}]});
        BTreeMap::from([("width".into(), serde_json::json!(200.0)), ("height".into(), serde_json::json!(150.0)), ("diagram".into(), Json::String(diagram.to_string()))])
    }

    #[test]
    fn a_solid_gray_edge_over_source_dashes_is_reported() {
        let found = solid_gray_over_dashes(&params(), &reference(true));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("\"C\"→\"D\"") && found[0].contains("separate runs"));
    }

    #[test]
    fn a_measured_dashed_edge_is_set_dashed_in_segments_and_in_paths() {
        let (fixed, notes) = dashed_corrections(&params(), &reference(true)).expect("corrected");
        assert_eq!(notes.len(), 1, "{notes:?}");
        let d = diagram_of(&fixed).unwrap();
        assert_eq!(d["segments"][2]["style"], "dashed");
        assert!(dashed_corrections(&fixed, &reference(true)).is_none(), "nothing left to correct");
        assert!(dashed_corrections(&params(), &reference(false)).is_none(), "a continuous source line stays solid");
        // Round 25: the same edge moved into a polyline path is measured too.
        let mut moved = params();
        moved.insert("diagram".into(), Json::String(serde_json::json!({"nodes":{"A":[20,20],"B":[180,20],"C":[20,130],"D":[180,130]},
            "segments":[{"from":"A","to":"B","color":"#000000"},{"from":"A","to":"C","color":"#000000"}],
            "paths":[{"nodes":["C","D"],"kind":"polyline","color":"#888888","style":"solid"}]}).to_string()));
        let (fixed, _) = dashed_corrections(&moved, &reference(true)).expect("path corrected");
        assert_eq!(diagram_of(&fixed).unwrap()["paths"][0]["style"], "dashed");
    }

    #[test]
    fn a_hand_drawn_coordinate_graph_is_recognised() {
        let mut nodes = serde_json::Map::new();
        let mut segs = Vec::new();
        for i in 0..6 {
            let v = 20.0 + 30.0 * i as f64;
            for (k, p) in [(format!("h{i}a"), [20.0, v]), (format!("h{i}b"), [180.0, v]), (format!("v{i}a"), [v, 20.0]), (format!("v{i}b"), [v, 180.0])] {
                nodes.insert(k, serde_json::json!(p));
            }
            segs.push(serde_json::json!({"from":format!("h{i}a"),"to":format!("h{i}b"),"color":"#cccccc"}));
            segs.push(serde_json::json!({"from":format!("v{i}a"),"to":format!("v{i}b"),"color":"#cccccc"}));
        }
        let labels: Vec<Json> = ["-2", "\u{2212}1", "1", "2"].iter().map(|t| serde_json::json!({"text":t,"at":[10,10]})).collect();
        let diagram = serde_json::json!({"nodes":nodes,"segments":segs,"labels":labels});
        let p = BTreeMap::from([("width".into(), serde_json::json!(200.0)), ("height".into(), serde_json::json!(200.0)), ("diagram".into(), Json::String(diagram.to_string()))]);
        assert!(hand_drawn_grid(&p).is_some_and(|m| m.contains("coordinate-grid")));
        assert!(hand_drawn_grid(&params()).is_none(), "a prism is not a graph");
    }

    #[test]
    fn a_continuous_source_line_and_unclear_cases_report_nothing() {
        assert!(solid_gray_over_dashes(&params(), &reference(false)).is_empty(), "continuous source line");
        // Already dashed in the candidate: nothing to check.
        let mut p = params();
        p.insert("diagram".into(), Json::String(p["diagram"].as_str().unwrap().replace("\"color\":\"#888888\"", "\"color\":\"#888888\",\"style\":\"dashed\"")));
        assert!(solid_gray_over_dashes(&p, &reference(true)).is_empty());
        // A drawing that does not land on the reference's ink cannot be read.
        let mut far = params();
        far.insert("diagram".into(), Json::String(far["diagram"].as_str().unwrap().replace("[20,20]", "[150,140]")));
        assert!(solid_gray_over_dashes(&far, &[]).is_empty(), "no reference image");
        assert!(solid_gray_over_dashes(&far, &reference(true)).is_empty(), "black edges off the ink: no registration, no claim");
    }
}

/// Diagnostic over saved runs: DASH_CASES lists `spec.json<TAB>reference image` lines.
#[cfg(test)]
#[test]
#[ignore]
fn dash_check_over_saved_cases() {
    let list = std::env::var("DASH_CASES").expect("DASH_CASES");
    for line in std::fs::read_to_string(list).unwrap().lines() {
        let (spec, image) = line.split_once('\t').unwrap();
        let params: BTreeMap<String, Json> = serde_json::from_str(&std::fs::read_to_string(spec).unwrap()).unwrap();
        let p = crate::figures::params_from_json(&crate::figures::geometry_diagram::def(), &serde_json::to_value(&params).unwrap());
        println!("{spec}: dash={:?} minus={:?} grid={}", solid_gray_over_dashes(&params, &std::fs::read(image).unwrap()).len(),
            crate::figures::geometry_diagram::hidden_minus_signs(&p).unwrap_or_default().len(), hand_drawn_grid(&params).is_some());
    }
}

/// A coordinate graph built by hand from geometry-diagram strokes: at least five long horizontal
/// and five long vertical lines and four number labels. coordinate-grid draws axes, grids, tick
/// labels and curves exactly, so the maker is asked once to use it.
pub fn hand_drawn_grid(params: &BTreeMap<String, Json>) -> Option<String> {
    let ((w, h), segs) = segments(params)?;
    let (mut across, mut down) = (0, 0);
    for s in &segs {
        let (dx, dy) = ((s.b.0 - s.a.0).abs(), (s.b.1 - s.a.1).abs());
        if dy <= 0.02 * dx && dx >= 0.3 * w {
            across += 1;
        } else if dx <= 0.02 * dy && dy >= 0.3 * h {
            down += 1;
        }
    }
    let diagram: Json = match params.get("diagram")? {
        Json::String(s) => serde_json::from_str(s).ok()?,
        other => other.clone(),
    };
    let numbers = diagram.get("labels").and_then(Json::as_array).into_iter().flatten()
        .filter_map(|l| l.get("text").and_then(Json::as_str))
        .filter(|t| t.trim().replace('\u{2212}', "-").parse::<f64>().is_ok())
        .count();
    (across >= 5 && down >= 5 && numbers >= 4).then(|| format!(
        "This geometry-diagram is a coordinate graph drawn by hand ({across} horizontal and {down} vertical grid lines, {numbers} number labels). coordinate-grid draws a graph's axes, grid, tick labels and curves exactly; inspect it with get_figure and use it, unless the source needs something it cannot draw, in which case send this final again and say what in the note."
    ))
}
