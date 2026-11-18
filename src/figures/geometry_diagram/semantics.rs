//! A report of what the authored primitives actually say. It is not a source-match verdict.
//! Ambiguous near-incidences are conditional warnings, never snapped or filled automatically.
use super::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AngleAssociation {
    pub vertex: String,
    pub from: String,
    pub to: String,
    pub arcs: u8,
    pub right: bool,
    pub reflex: bool,
    pub label: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct ThroughAssociation {
    pub id: String,
    pub through: String,
    pub back_port: String,
    pub forward_port: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct EllipseAssociation {
    pub id: String,
    pub center: String,
    pub ports: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct SegmentAnnotation {
    pub segment_index: usize,
    pub from: String,
    pub to: String,
    pub ticks: u8,
    pub tick_color: String,
    pub parallel_marks: u8,
    pub parallel_color: String,
    pub label: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct PointAssociation {
    pub node: String,
    pub position: (f64, f64),
    pub color: String,
    pub radius: f64,
    pub connected_edges: Vec<[String; 2]>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ArrowTarget {
    pub node: String,
    pub position: (f64, f64),
    /// Named/declared alias connections only; no nearest-edge attachment is inferred.
    pub connected_edges: Vec<[String; 2]>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ArrowedStroke {
    /// May be a leader, ray or vector: this inventory does not infer the source role.
    pub primitive: &'static str,
    pub item_index: usize,
    pub nodes: Vec<String>,
    pub arrow_start: bool,
    pub arrow_end: bool,
    pub targets: Vec<ArrowTarget>,
}
#[derive(Debug, Clone, Serialize)]
pub struct StrokeDescription {
    pub primitive: &'static str,
    pub item_index: usize,
    pub id: Option<String>,
    /// Authored names, in drawing order; path control nodes are included.
    pub nodes: Vec<String>,
    pub endpoints: Option<[String; 2]>,
    pub style: &'static str,
    pub color: String,
    pub width: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct EndpointNearHost {
    pub branch_edge: String,
    pub branch_node: String,
    pub host_edge: String,
    pub host_nodes: [String; 2],
    pub projection_fraction: f64,
    pub centerline_distance: f64,
    pub combined_half_stroke_allowance: f64,
    /// Signed centerline distance minus the allowance, not a measured pixel gap.
    pub clearance_after_allowance: f64,
    pub stroke_envelopes: &'static str,
    pub distance_limit: f64,
    pub declared_incidence: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct EndpointNearHostCheck {
    pub scope: &'static str,
    pub total_straight_edges: usize,
    pub analyzed_straight_edges: usize,
    pub edge_limit: usize,
    pub candidate_limit: usize,
    pub analysis_truncated: bool,
    pub candidates_truncated: bool,
    pub candidates: Vec<EndpointNearHost>,
}
#[derive(Debug, Clone, Serialize)]
pub struct SemanticReport {
    pub scope: &'static str,
    pub edge_pairs: Vec<[String; 2]>,
    pub incidences: BTreeMap<String, Vec<String>>,
    pub through_lines: Vec<ThroughAssociation>,
    pub ellipse_attachments: Vec<EllipseAssociation>,
    pub segment_annotations: Vec<SegmentAnnotation>,
    pub point_markers: Vec<PointAssociation>,
    pub arrowed_strokes: Vec<ArrowedStroke>,
    pub stroke_inventory_scope: &'static str,
    pub stroke_inventory_total: usize,
    pub stroke_inventory_truncated: bool,
    pub stroke_inventory: Vec<StrokeDescription>,
    pub endpoint_near_host_check: EndpointNearHostCheck,
    pub angle_associations: Vec<AngleAssociation>,
    pub unbound_labels: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EdgeOwner { Segment(usize), Line(usize), Path(usize) }
#[derive(Clone)]
pub(super) struct Edge {
    pub owner: EdgeOwner,
    pub name: String,
    pub from: String,
    pub to: String,
    pub a: (f64, f64),
    pub b: (f64, f64),
    pub width: f64,
    pub both: bool,
}
pub(super) fn edges(g: &Diagram) -> Vec<Edge> {
    let mut v: Vec<_> = g
        .segments
        .iter()
        .enumerate()
        .map(|(i,s)| Edge {
            owner: EdgeOwner::Segment(i),
            name: format!("{}:{}", s.from, s.to),
            from: s.from.clone(),
            to: s.to.clone(),
            a: g.node(&s.from).unwrap(),
            b: g.node(&s.to).unwrap(),
            width: s.width,
            both: matches!(s.arrows, Arrows::Both),
        })
        .collect();
    for (i,l) in g.lines.iter().enumerate() {
        let (_, back, forward) = g.line_points(l).unwrap();
        v.push(Edge {
            owner: EdgeOwner::Line(i),
            name: l.id.clone(),
            from: format!("{}:back", l.id),
            to: format!("{}:forward", l.id),
            a: back,
            b: forward,
            width: l.width,
            both: matches!(l.arrows, Arrows::Both),
        });
    }
    for (i, path) in g
        .paths
        .iter()
        .enumerate()
        .filter(|(_, p)| matches!(p.kind, PathKind::Polyline))
    {
        for (k, pair) in path.nodes.windows(2).enumerate() {
            v.push(Edge {
                owner: EdgeOwner::Path(i),
                name: format!("path {i} segment {k}"),
                from: pair[0].clone(),
                to: pair[1].clone(),
                a: g.node(&pair[0]).unwrap(),
                b: g.node(&pair[1]).unwrap(),
                width: path.width,
                both: false,
            });
        }
    }
    v
}
fn connected_edges(g: &Diagram, edges: &[Edge], node: &str, exclude: Option<EdgeOwner>) -> Vec<[String; 2]> {
    let key = g.node_key(node);
    edges.iter().filter(|e| {
        Some(e.owner) != exclude && (g.node_key(&e.from) == key || g.node_key(&e.to) == key
            || matches!(e.owner,EdgeOwner::Line(i) if g.node_key(&g.lines[i].through)==key))
    }).map(|e|[e.from.clone(),e.to.clone()]).collect()
}
fn arrowed_strokes(g: &Diagram, edges: &[Edge]) -> Vec<ArrowedStroke> {
    let mut out = Vec::new();
    let mut add = |primitive, item_index, owner, nodes: Vec<String>, arrows| {
        let arrow_start = matches!(arrows,Arrows::Start | Arrows::Both);
        let arrow_end = matches!(arrows,Arrows::End | Arrows::Both);
        if !arrow_start && !arrow_end { return; }
        let mut targets = Vec::new();
        for (shown,node) in [(arrow_start,&nodes[0]),(arrow_end,nodes.last().unwrap())] {
            if shown {
                targets.push(ArrowTarget {node:node.clone(),position:g.node(node).unwrap(),connected_edges:connected_edges(g,edges,node,Some(owner))});
            }
        }
        out.push(ArrowedStroke {primitive,item_index,nodes,arrow_start,arrow_end,targets});
    };
    for (i,s) in g.segments.iter().enumerate() {
        add("segment",i,EdgeOwner::Segment(i),vec![s.from.clone(),s.to.clone()],s.arrows);
    }
    for (i,l) in g.lines.iter().enumerate() {
        add("through-line",i,EdgeOwner::Line(i),vec![format!("{}:back",l.id),format!("{}:forward",l.id)],l.arrows);
    }
    for (i,p) in g.paths.iter().enumerate() {
        add(match p.kind {PathKind::Polyline=>"polyline",PathKind::Quadratic=>"quadratic",PathKind::Cubic=>"cubic"},i,EdgeOwner::Path(i),p.nodes.clone(),p.arrows);
    }
    out
}
pub(super) fn text_box(at: (f64, f64), text: &str, size: f64) -> [f64; 4] {
    let hw = crate::figures::circle::tw(text, size, false) / 2.0 + 0.7;
    [at.0 - hw, at.1 - size * 0.85, at.0 + hw, at.1 + size * 0.22]
}
pub(super) fn segment_meets_box(a: (f64, f64), b: (f64, f64), r: [f64; 4], margin: f64) -> bool {
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for (p, d, mn, mx) in [
        (a.0, b.0 - a.0, r[0] - margin, r[2] + margin),
        (a.1, b.1 - a.1, r[1] - margin, r[3] + margin),
    ] {
        if d.abs() < 1e-12 {
            if p < mn || p > mx {
                return false;
            }
        } else {
            let (mut t0, mut t1) = ((mn - p) / d, (mx - p) / d);
            if t0 > t1 {
                std::mem::swap(&mut t0, &mut t1);
            }
            lo = lo.max(t0);
            hi = hi.min(t1);
            if lo > hi {
                return false;
            }
        }
    }
    true
}
fn cross(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}
fn sub(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
}
fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}
fn intersection(a: &Edge, b: &Edge) -> Option<(f64, f64)> {
    let (u, v, w) = (sub(a.b, a.a), sub(b.b, b.a), sub(b.a, a.a));
    let den = cross(u, v);
    if den.abs() < 1e-9 {
        return None;
    }
    let (t, s) = (cross(w, v) / den, cross(w, u) / den);
    if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&s) {
        return None;
    }
    Some((a.a.0 + t * u.0, a.a.1 + t * u.1))
}
fn quadrant(a: (f64, f64), b: (f64, f64)) -> &'static str {
    let u = unit(a, b);
    match (u.0.abs() < 0.025, u.1.abs() < 0.025, u.0 > 0.0, u.1 > 0.0) {
        (true, _, _, true) => "down",
        (true, _, _, false) => "up",
        (_, true, true, _) => "right",
        (_, true, false, _) => "left",
        (_, _, true, true) => "down-right",
        (_, _, true, false) => "up-right",
        (_, _, false, true) => "down-left",
        _ => "up-left",
    }
}
fn warn(out: &mut Vec<String>, s: String) {
    if out.len() < 12 && !out.contains(&s) {
        out.push(s);
    }
}

const FEEDBACK_EDGE_LIMIT: usize = 128;
const NEAR_ENDPOINT_LIMIT: usize = 32;
const STROKE_INVENTORY_LIMIT: usize = 128;

fn style_name(style: LineStyle) -> &'static str {
    match style {
        LineStyle::Solid => "solid",
        LineStyle::Dashed => "dashed",
        LineStyle::Dotted => "dotted",
    }
}

fn stroke_inventory(g: &Diagram) -> Vec<StrokeDescription> {
    let mut out = Vec::new();
    let mut add = |primitive, item_index, id, nodes: Vec<String>, endpoints, style, color: &str, width| {
        if out.len() < STROKE_INVENTORY_LIMIT {
            out.push(StrokeDescription { primitive, item_index, id, nodes, endpoints,
                style: style_name(style), color: color.into(), width });
        }
    };
    for (i, s) in g.segments.iter().enumerate() {
        add("segment", i, None, vec![s.from.clone(), s.to.clone()],
            Some([s.from.clone(), s.to.clone()]), s.style, &s.color, s.width);
    }
    for (i, l) in g.lines.iter().enumerate() {
        let ends = [format!("{}:back", l.id), format!("{}:forward", l.id)];
        add("through-line", i, Some(l.id.clone()),
            vec![ends[0].clone(), l.through.clone(), ends[1].clone()],
            Some(ends), l.style, &l.color, l.width);
    }
    for (i, p) in g.paths.iter().enumerate() {
        add(match p.kind { PathKind::Polyline => "polyline", PathKind::Quadratic => "quadratic", PathKind::Cubic => "cubic" },
            i, None, p.nodes.clone(), Some([p.nodes[0].clone(), p.nodes.last().unwrap().clone()]),
            p.style, &p.color, p.width);
    }
    for (i, e) in g.ellipses.iter().enumerate() {
        add("ellipse", i, e.id.clone(), vec![e.center.clone()],
            e.id.as_ref().map(|id| [format!("{id}:start"), format!("{id}:end")]),
            e.style, &e.color, e.width);
    }
    out
}

fn endpoint_near_host_check(g: &Diagram, edges: &[Edge]) -> EndpointNearHostCheck {
    // Degrees use the complete authored inventory, even when the pair search is
    // truncated. A node shared outside the search window must not become "free".
    let mut degrees: BTreeMap<String, usize> = BTreeMap::new();
    for edge in edges {
        *degrees.entry(g.node_key(&edge.from)).or_default() += 1;
        *degrees.entry(g.node_key(&edge.to)).or_default() += 1;
    }
    for line in &g.lines {
        *degrees.entry(g.node_key(&line.through)).or_default() += 2;
    }
    let analyzed = &edges[..edges.len().min(FEEDBACK_EDGE_LIMIT)];
    let mut candidates = Vec::new();
    let mut found = 0;
    for (i, branch) in analyzed.iter().enumerate() {
        for (node, at) in [(&branch.from, branch.a), (&branch.to, branch.b)] {
            let key = g.node_key(node);
            if degrees.get(&key) != Some(&1) { continue; }
            for (j, host) in analyzed.iter().enumerate() {
                if i == j || host.owner == branch.owner { continue; }
                let u = sub(host.b, host.a);
                let len2 = u.0 * u.0 + u.1 * u.1;
                if len2 <= 1e-12 { continue; }
                let v = sub(at, host.a);
                let t = (v.0 * u.0 + v.1 * u.1) / len2;
                // Host endpoints are not an unsplit T-junction candidate.
                if !(0.0 < t && t < 1.0) { continue; }
                let projected = (host.a.0 + t * u.0, host.a.1 + t * u.1);
                let d = distance(at, projected);
                let allowance = (branch.width + host.width) / 2.0;
                let local_extent = distance(branch.a, branch.b).min(len2.sqrt());
                let limit = allowance + allowance.max(0.01 * local_extent).max(1.0);
                if d > limit { continue; }
                found += 1;
                if candidates.len() >= NEAR_ENDPOINT_LIMIT { continue; }
                let clearance = d - allowance;
                candidates.push(EndpointNearHost {
                    branch_edge: branch.name.clone(), branch_node: node.clone(),
                    host_edge: host.name.clone(), host_nodes: [host.from.clone(), host.to.clone()],
                    projection_fraction: t, centerline_distance: d,
                    combined_half_stroke_allowance: allowance,
                    clearance_after_allowance: clearance,
                    stroke_envelopes: if clearance > 0.0 { "separated" } else { "overlap_or_touch" },
                    distance_limit: limit, declared_incidence: false,
                });
            }
        }
    }
    EndpointNearHostCheck {
        scope: "Candidate straight-edge centerlines only; free named endpoints near finite host interiors. Stroke envelopes are conservative half-width allowances, not pixel contact, cap, arrowhead or dash-phase measurements. No source intent, join or mismatch is inferred; coordinates are never changed.",
        total_straight_edges: edges.len(), analyzed_straight_edges: analyzed.len(),
        edge_limit: FEEDBACK_EDGE_LIMIT, candidate_limit: NEAR_ENDPOINT_LIMIT,
        analysis_truncated: edges.len() > analyzed.len(),
        candidates_truncated: found > candidates.len(), candidates,
    }
}

pub(super) fn report(g: &Diagram, p: &Params) -> SemanticReport {
    let edges = edges(g);
    let endpoint_near_host_check = endpoint_near_host_check(g, &edges);
    let mut incidences: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in &edges {
        if g.lines.iter().any(|l| l.id == e.name) {
            continue;
        }
        for (a, b, qa, qb) in [(&e.from, &e.to, e.a, e.b), (&e.to, &e.from, e.b, e.a)] {
            incidences
                .entry(a.clone())
                .or_default()
                .push(format!("{b} ({})", quadrant(qa, qb)));
        }
    }
    for l in &g.lines {
        let (o, back, forward) = g.line_points(l).unwrap();
        incidences.entry(l.through.clone()).or_default().extend([
            format!("{}:back ({})", l.id, quadrant(o, back)),
            format!("{}:forward ({})", l.id, quadrant(o, forward)),
        ]);
        incidences
            .entry(format!("{}:back", l.id))
            .or_default()
            .push(format!("{} ({})", l.through, quadrant(back, o)));
        incidences
            .entry(format!("{}:forward", l.id))
            .or_default()
            .push(format!("{} ({})", l.through, quadrant(forward, o)));
    }
    let mut warnings = vec![];
    for c in &endpoint_near_host_check.candidates {
        warn(&mut warnings, format!(
            "Candidate endpoint {:?} of edge {:?} is near the interior of edge {:?} ({:?}→{:?}): centerline distance {:.4} px, combined half-stroke allowance {:.4} px, signed clearance {:.4} px ({}); no declared incidence. This reports authored geometry only, not source intent, mismatch or actual ink contact. If the source visibly joins here, use one shared node and collinear split host edges; preserve an intended nonjoin. No snapping is performed.",
            c.branch_node, c.branch_edge, c.host_edge, c.host_nodes[0], c.host_nodes[1],
            c.centerline_distance, c.combined_half_stroke_allowance,
            c.clearance_after_allowance, c.stroke_envelopes));
    }
    // Do not decide that a real small triangle is a concurrency. Only report the ambiguity.
    let both: Vec<_> = edges.iter().filter(|e| e.both).take(40).collect();
    'triples: for i in 0..both.len() {
        for j in i + 1..both.len() {
            for k in j + 1..both.len() {
                if let (Some(a), Some(b), Some(c)) = (
                    intersection(both[i], both[j]),
                    intersection(both[j], both[k]),
                    intersection(both[k], both[i]),
                ) {
                    let spread = distance(a, b).max(distance(b, c)).max(distance(c, a));
                    // A relative distance keeps the diagnostic meaningful when a source
                    // drawing is scaled; it never changes that drawing or asserts intent.
                    let extent = distance(both[i].a, both[i].b)
                        .min(distance(both[j].a, both[j].b))
                        .min(distance(both[k].a, both[k].b));
                    if spread > 1e-6 && spread <= (0.03 * extent).max(2.0) {
                        warn(
                            &mut warnings,
                            format!(
                                "Double-ended edges {}, {}, {} meet at different nearby points, forming a small triangle rather than one shared vertex. If the source is concurrent, use through-lines with the same named node; do not snap a real source triangle.",
                                both[i].name, both[j].name, both[k].name
                            ),
                        );
                        if warnings.len() >= 12 {
                            break 'triples;
                        }
                    }
                }
            }
        }
    }
    // Near-opposite spokes remain separate unless the author explicitly chooses a line.
    let mut spokes: BTreeMap<String, Vec<(String, (f64, f64))>> = BTreeMap::new();
    for s in &g.segments {
        let (a, b) = (g.node(&s.from).unwrap(), g.node(&s.to).unwrap());
        spokes
            .entry(s.from.clone())
            .or_default()
            .push((s.to.clone(), unit(a, b)));
        spokes
            .entry(s.to.clone())
            .or_default()
            .push((s.from.clone(), unit(b, a)));
    }
    for (node, sp) in &spokes {
        for i in 0..sp.len() {
            for j in i + 1..sp.len() {
                let (a, b) = (sp[i].1, sp[j].1);
                if a.0 * b.0 + a.1 * b.1 < -0.996 && cross(a, b).abs() > 1e-7 {
                    warn(
                        &mut warnings,
                        format!(
                            "Spokes {node}→{} and {node}→{} are nearly opposite but kink. If the source is one straight line, use a through-line; preserve distinct directions when the source is not straight.",
                            sp[i].0, sp[j].0
                        ),
                    );
                }
            }
        }
    }
    for l in &g.labels {
        let q = l
            .node
            .as_ref()
            .map(|n| g.node(n).unwrap())
            .unwrap_or_else(|| {
                let q = l.at.unwrap();
                (q[0], q[1])
            });
        let at = (q.0 + l.offset[0], q.1 + l.offset[1]);
        let b = text_box(at, &l.text, l.size);
        if let Some(e) = edges
            .iter()
            .find(|e| segment_meets_box(e.a, e.b, b, e.width / 2.0))
        {
            warn(
                &mut warnings,
                format!(
                    "Free text {:?} has conservative text bounds overlapping edge {}. Check the actual glyphs and source placement: dimension, point or annotation text may intentionally sit on an edge. This does not identify the text's role or prove a source mismatch. Preserve intentional source placement; do not move it automatically.",
                    l.text, e.name
                ),
            );
        }
        if b[0] < 0.0 || b[1] < 0.0 || b[2] > p.f("width") || b[3] > p.f("height") {
            warn(
                &mut warnings,
                format!("Free text {:?} has conservative text bounds outside the canvas; check the actual text and source crop.", l.text),
            );
        }
    }
    incidences.retain(|_, v| v.len() > 1);
    SemanticReport {
        scope: "Authored/rendered geometry only; named incidences, no inferred crossings or source-match verdict",
        edge_pairs: edges
            .iter()
            .map(|e| [e.from.clone(), e.to.clone()])
            .collect(),
        incidences,
        through_lines: g
            .lines
            .iter()
            .map(|l| ThroughAssociation {
                id: l.id.clone(),
                through: l.through.clone(),
                back_port: format!("{}:back", l.id),
                forward_port: format!("{}:forward", l.id),
            })
            .collect(),
        ellipse_attachments: g.ellipses.iter().filter_map(|e| e.id.as_ref().map(|id| EllipseAssociation {
            id: id.clone(),
            center: e.center.clone(),
            ports: ELLIPSE_PORTS.iter().copied().chain(e.ports.keys().map(String::as_str)).map(|port|format!("{id}:{port}")).collect(),
        })).collect(),
        segment_annotations: g.segments.iter().enumerate().filter(|(_,s)|s.ticks>0 || s.parallel_marks>0 || !s.label.is_empty()).map(|(i,s)|SegmentAnnotation {
            segment_index:i,from:s.from.clone(),to:s.to.clone(),ticks:s.ticks,
            tick_color:s.tick_color.as_ref().unwrap_or(&s.color).clone(),parallel_marks:s.parallel_marks,
            parallel_color:s.parallel_color.as_ref().unwrap_or(&s.color).clone(),label:s.label.clone(),
        }).collect(),
        point_markers: g.points.iter().map(|p|PointAssociation {
            node:p.node.clone(),position:g.node(&p.node).unwrap(),color:p.color.clone(),radius:p.radius,
            connected_edges:connected_edges(g,&edges,&p.node,None),
        }).collect(),
        arrowed_strokes: arrowed_strokes(g,&edges),
        stroke_inventory_scope: "Candidate segment/through-line/path/ellipse primitives only; dimension, mark and text strokes are not inventoried. Authored styles/colors/widths and named nodes; style does not imply a source role such as hidden. Curved paths and named/unnamed ellipses are included; no source-match verdict or dash pixel contact is inferred.",
        stroke_inventory_total: g.segments.len() + g.lines.len() + g.paths.len() + g.ellipses.len(),
        stroke_inventory_truncated: g.segments.len() + g.lines.len() + g.paths.len() + g.ellipses.len() > STROKE_INVENTORY_LIMIT,
        stroke_inventory: stroke_inventory(g),
        endpoint_near_host_check,
        angle_associations: g
            .angles
            .iter()
            .map(|a| AngleAssociation {
                vertex: a.vertex.clone(),
                from: a.from.clone(),
                to: a.to.clone(),
                arcs: a.arcs,
                right: a.right,
                reflex: a.reflex,
                label: a.label.clone(),
            })
            .collect(),
        unbound_labels: g.labels.iter().map(|l| l.text.clone()).collect(),
        warnings,
    }
}
