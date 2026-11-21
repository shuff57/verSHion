//! An explicit, non-solving geometry drawing. Named nodes keep shared vertices and interior
//! segments connected; every label, mark and dimension is supplied by the author. Nothing is
//! inferred from the lengths or angles used to place the drawing.
use std::collections::BTreeMap;

use serde::Deserialize;

use super::{
    FigureDef, ParamDef, Params, Subject,
    svg::{self, Doc},
};
mod semantics;
pub use semantics::SemanticReport;

type Pt = [f64; 2];
fn black() -> String {
    "#000000".into()
}
fn one() -> f64 {
    1.5
}
fn radius() -> f64 {
    18.0
}
fn font_size() -> f64 {
    18.0
}
fn label_offset() -> f64 {
    -12.0
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Arrows {
    #[default]
    None,
    Start,
    End,
    Both,
}
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}
impl LineStyle {
    fn dash(self) -> &'static str {
        match self {
            Self::Solid => "",
            Self::Dashed => "7 5",
            Self::Dotted => "2 4",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Segment {
    from: String,
    to: String,
    #[serde(default)]
    arrows: Arrows,
    #[serde(default)]
    style: LineStyle,
    #[serde(default = "black")]
    color: String,
    #[serde(default = "one")]
    width: f64,
    #[serde(default)]
    ticks: u8,
    #[serde(default)]
    tick_color: Option<String>,
    /// Explicit middle-of-segment chevrons, separate from endpoint arrows and ticks.
    #[serde(default)]
    parallel_marks: u8,
    #[serde(default)]
    parallel_color: Option<String>,
    #[serde(default)]
    label: String,
    #[serde(default = "label_offset")]
    label_offset: f64,
}

fn line_length() -> f64 {
    100.0
}
/// Constructive incidence, not a fitted/snapped version of two independent endpoints.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThroughLine {
    id: String,
    through: String,
    toward: String,
    #[serde(default = "line_length")]
    back: f64,
    #[serde(default = "line_length")]
    forward: f64,
    #[serde(default)]
    arrows: Arrows,
    #[serde(default)]
    style: LineStyle,
    #[serde(default = "black")]
    color: String,
    #[serde(default = "one")]
    width: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Angle {
    vertex: String,
    from: String,
    to: String,
    /// Zero draws no arc. A right-angle square is separately explicit.
    #[serde(default)]
    arcs: u8,
    #[serde(default)]
    right: bool,
    #[serde(default)]
    reflex: bool,
    #[serde(default = "radius")]
    radius: f64,
    #[serde(default = "black")]
    color: String,
    #[serde(default)]
    label: String,
    #[serde(default = "black")]
    label_color: String,
    #[serde(default)]
    label_radius: f64,
    #[serde(default = "font_size")]
    label_size: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    text: String,
    #[serde(default)]
    node: Option<String>,
    #[serde(default)]
    at: Option<Pt>,
    #[serde(default)]
    offset: Pt,
    #[serde(default = "black")]
    color: String,
    #[serde(default = "font_size")]
    size: f64,
    /// Degrees clockwise about the anchor; -90 reads bottom to top, as a printed y-axis title.
    #[serde(default)]
    rotate: f64,
    #[serde(default)]
    weight: FontWeight,
    #[serde(default)]
    style: FontStyle,
    /// A white backing behind the glyphs, so a line under the label does not hide it.
    #[serde(default)]
    halo: bool,
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum FontWeight {
    #[default]
    Normal,
    Bold,
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum FontStyle {
    #[default]
    Normal,
    Italic,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DimensionEnds {
    #[default]
    Arrows,
    Ticks,
    ArrowsAndTicks,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dimension {
    from: String,
    to: String,
    label: String,
    /// Signed perpendicular distance from the named side, in canvas pixels.
    #[serde(default)]
    offset: f64,
    #[serde(default)]
    ends: DimensionEnds,
    #[serde(default = "black")]
    color: String,
    #[serde(default = "black")]
    label_color: String,
    #[serde(default = "label_offset")]
    label_offset: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PointMarker {
    node: String,
    #[serde(default = "black")]
    color: String,
    #[serde(default = "point_radius")]
    radius: f64,
}
fn point_radius() -> f64 {
    3.0
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PathKind {
    #[default]
    Polyline,
    Quadratic,
    Cubic,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DrawingPath {
    nodes: Vec<String>,
    #[serde(default)]
    kind: PathKind,
    #[serde(default)]
    arrows: Arrows,
    #[serde(default)]
    style: LineStyle,
    #[serde(default = "black")]
    color: String,
    #[serde(default = "one")]
    width: f64,
}
fn full_turn() -> f64 {
    360.0
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ellipse {
    /// Optional constructive attachment, never a request to infer or draw a radius.
    #[serde(default)]
    id: Option<String>,
    center: String,
    rx: f64,
    ry: f64,
    #[serde(default)]
    start: f64,
    #[serde(default = "full_turn")]
    end: f64,
    #[serde(default)]
    style: LineStyle,
    #[serde(default = "black")]
    color: String,
    #[serde(default = "one")]
    width: f64,
    /// Named points at explicitly authored parameter angles of this projected ellipse.
    #[serde(default)]
    ports: BTreeMap<String, f64>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Diagram {
    #[serde(default)]
    nodes: BTreeMap<String, Pt>,
    #[serde(default)]
    segments: Vec<Segment>,
    #[serde(default)]
    lines: Vec<ThroughLine>,
    #[serde(default)]
    angles: Vec<Angle>,
    #[serde(default)]
    labels: Vec<Label>,
    #[serde(default)]
    dimensions: Vec<Dimension>,
    #[serde(default)]
    points: Vec<PointMarker>,
    #[serde(default)]
    paths: Vec<DrawingPath>,
    #[serde(default)]
    ellipses: Vec<Ellipse>,
}

const EXAMPLE: &str = r##"{"nodes":{"A":[65,235],"B":[185,45],"C":[325,235]},"segments":[{"from":"A","to":"B"},{"from":"B","to":"C"},{"from":"C","to":"A"}],"labels":[{"node":"A","offset":[-12,18],"text":"A"},{"node":"B","offset":[0,-14],"text":"B"},{"node":"C","offset":[12,18],"text":"C"}]}"##;

pub fn def() -> FigureDef {
    FigureDef {
        id: "geometry-diagram",
        subject: Subject::Math,
        title: "Geometry and projected solids",
        blurb: "Explicit geometry and projected solid diagrams: exact through-node lines, sector-bound literal labels, joined triangles, hidden segments, rays, ellipses, leaders, dimensions and individual congruence marks. Use for source diagrams a specialized solver cannot match; no solving or automatic measurements.",
        params,
        render,
    }
}
fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("width", "Canvas width", 100.0, 1600.0, 1.0, 390.0),
        ParamDef::float("height", "Canvas height", 100.0, 1600.0, 1.0, 285.0),
        ParamDef::long("diagram", "Diagram JSON", EXAMPLE).hint("JSON object, or its JSON text. Coordinates are canvas pixels, x right, y down. nodes: {A:[x,y],B:[x,y]}. segments: [{from:'A',to:'B',arrows:'none|start|end|both',style:'solid|dashed|dotted',color:'#rrggbb',width:1.5,ticks:0,tick_color:'#rrggbb',parallel_marks:0,parallel_color:'#rrggbb',label:'',label_offset:-12}]. ticks and parallel_marks are independently the exact authored count 0..3; their optional colors default to the edge color. parallel_marks draws middle chevrons pointing from from toward to; it does not add endpoint arrows or infer that any other edge is parallel. Use arrows only for visibly arrowed endpoints. lines: [{id:'axis',through:'O',toward:'T',back:100,forward:100,arrows:'none|start|end|both',style:'solid|dashed|dotted',color:'#rrggbb',width:1.5}]. through/toward must be literal base nodes. back/forward are explicit pixel lengths in opposite directions; ports axis:back and axis:forward may be used as node names in angles/labels/segments. Lines sharing through:'O' are exactly concurrent. Use this primitive for source straight lines/opposite rays; never approximate a shared vertex with independent crossing endpoints. angles: [{vertex:'B',from:'A',to:'C',arcs:0,right:false,reflex:false,radius:18,color:'#rrggbb',label:'',label_color:'#rrggbb',label_radius:0,label_size:18}]; the minor sector is used unless reflex:true; arcs is independently the exact count 0..3 (0 draws no arc). Put plain angle text in label with arcs:0 and the actual two named boundary legs, rather than unbound absolute text. label_radius is a preferred minimum; the entire label is placed within that sector clear of its rays/canvas, or the tool returns an error. Right squares use the explicit from/to legs; no angle measure is inferred. Count source marks at each named corner individually; no polygon regularity fills omitted marks. labels: [{text:'x + 3',node:'A',offset:[dx,dy]}] or {text:'...',at:[x,y],color:'#rrggbb',size:18,rotate:0,weight:'normal|bold',style:'normal|italic'}. rotate turns the text that many degrees clockwise about its anchor: a printed y-axis title read bottom to top is rotate:-90, one continuous label, never letters stacked one per line. Match printed bold titles with weight:'bold' and italic variable letters with style:'italic'. halo:true draws a white backing behind a label that sits on a line. dimensions: [{from:'A',to:'B',label:'?',offset:25,ends:'arrows|ticks|arrows_and_ticks',color:'#0000ff',label_color:'#000000',label_offset:-12}]. points: [{node:'A',radius:3,color:'#rrggbb'}]. ellipses: [{center:'O',rx:80,ry:20,start:0,end:360,style:'solid|dashed|dotted',color:'#rrggbb',width:1.5}]; start/end are degrees y-up, full or partial ellipse. Optional id:'rim' exposes rim:center/right/top/left/bottom/start/end as exact node names. Optional ports:{tip:25} exposes rim:tip at that explicitly supplied projected-ellipse parameter angle (not a physical angle or a solved measure). Named ellipses need a literal base center node and ids distinct from line ids. Use these ports in segments only when the source visibly connects center/rim; no radius, dot, mark or attachment is drawn automatically, and existing independent endpoints are never snapped. paths: [{nodes:['A','control','B'],kind:'polyline|quadratic|cubic',arrows:'none|start|end|both',style:'solid|dashed|dotted',color:'#rrggbb',width:1.5}]; quadratic has 3 nodes, cubic 4. Projected solids use explicit visible/hidden edges, ellipses and colored source annotations; do not require a 3D solver. Draw only visible source marks and labels; nodes alone have no dot or name. Omit optional arrays. JSON requires double quotes. Unmarked angle-text example: {\"nodes\":{\"O\":[80,180],\"A\":[80,30],\"B\":[250,180]},\"segments\":[{\"from\":\"O\",\"to\":\"A\"},{\"from\":\"O\",\"to\":\"B\"}],\"angles\":[{\"vertex\":\"O\",\"from\":\"A\",\"to\":\"B\",\"label\":\"b\",\"arcs\":0}]}. This draws b in its named sector and NO arc; the lack of a source arc is not a reason to use free text. Source-first annotation check: enumerate every visible corner/edge mark and its count, each point joined to a guide, each leader target, and visible/hidden edge roles. Do not fill missing marks from a regular-looking shape or treat a nearby point as joined. A black edge with two red ticks is explicit: {\"from\":\"A\",\"to\":\"B\",\"color\":\"#000000\",\"ticks\":2,\"tick_color\":\"#ff0000\"}. A middle red parallel chevron is {\"from\":\"A\",\"to\":\"B\",\"parallel_marks\":1,\"parallel_color\":\"#ff0000\",\"arrows\":\"none\"}. Keep these distinct from endpoint arrows. Exact leader-target example: {\"nodes\":{\"A\":[60,70],\"M\":[140,110],\"B\":[220,150],\"S\":[240,230],\"C\":[120,210]},\"segments\":[{\"from\":\"A\",\"to\":\"M\"},{\"from\":\"M\",\"to\":\"B\"}],\"paths\":[{\"nodes\":[\"S\",\"C\",\"M\"],\"kind\":\"quadratic\",\"arrows\":\"end\"}]}. The explicit guide and leader share M; M alone draws no dot. Use the actual source target, not a nearby independent endpoint; no snapping or missing target is inferred. For a visibly joined T-junction, explicitly split the host at one shared node: nodes K:[20,80], B:[120,80], L:[220,80], C:[120,180], with segments K-to-B, B-to-L and B-to-C. Keep K/B/L collinear and reuse B; an independent B near an unsplit K-to-L segment does not declare a join. Apply this only when the source visibly joins; preserve intentional nonjoins. No node or coordinate is autogenerated."),
        ParamDef::choice("font", "Label font", &["Serif", "Sans"], "Serif"),
    ]
}

fn color_ok(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|c| c.is_ascii_hexdigit())
}
fn finite(p: Pt) -> bool {
    p.iter().all(|v| v.is_finite() && v.abs() <= 10000.0)
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
const ELLIPSE_PORTS: [&str; 7] = ["center", "right", "top", "left", "bottom", "start", "end"];
impl Diagram {
    /// Only declared aliases are equivalent; equal/near coordinates never create a join.
    fn node_key(&self, name: &str) -> String {
        if !self.nodes.contains_key(name) {
            if let Some((id, "center")) = name.rsplit_once(':') {
                if let Some(e) = self.ellipses.iter().find(|e| e.id.as_deref() == Some(id)) {
                    return e.center.clone();
                }
            }
        }
        name.to_string()
    }
    fn node(&self, name: &str) -> Result<(f64, f64), String> {
        if let Some(p) = self.nodes.get(name) {
            return Ok((p[0], p[1]));
        }
        if let Some((id, port)) = name.rsplit_once(':') {
            if let Some(line) = self.lines.iter().find(|l| l.id == id) {
                let (_, back, forward) = self.line_points(line)?;
                return match port {
                    "back" => Ok(back),
                    "forward" => Ok(forward),
                    _ => Err(format!("Unknown line port {name:?}; use back or forward")),
                };
            }
            if let Some(e) = self.ellipses.iter().find(|e| e.id.as_deref() == Some(id)) {
                return self.ellipse_port(e, port);
            }
        }
        Err(format!("Unknown node {name:?}"))
    }
    fn ellipse_port(&self, e: &Ellipse, port: &str) -> Result<(f64, f64), String> {
        // Named ellipses intentionally use a literal center: this prevents recursive port
        // definitions. Legacy ellipses without ids retain their old node resolution.
        let c = self.nodes.get(&e.center).ok_or_else(|| format!("Named ellipse {:?} needs a literal base center node", e.id))?;
        if port == "center" { return Ok((c[0], c[1])); }
        let angle = match port {
            "right" => 0.0,
            "top" => 90.0,
            "left" => 180.0,
            "bottom" => 270.0,
            "start" => e.start,
            "end" => e.start + (e.end - e.start).clamp(-360.0, 360.0),
            _ => *e.ports.get(port).ok_or_else(|| format!("Unknown ellipse port {:?}:{port}; use a cardinal/start/end port or an explicitly named angle in ports", e.id))?,
        }.to_radians();
        Ok((c[0] + e.rx * angle.cos(), c[1] - e.ry * angle.sin()))
    }
    fn line_points(
        &self,
        line: &ThroughLine,
    ) -> Result<((f64, f64), (f64, f64), (f64, f64)), String> {
        let raw = |n: &str| {
            self.nodes
                .get(n)
                .map(|p| (p[0], p[1]))
                .ok_or_else(|| format!("Line {:?} needs literal base node {n:?}", line.id))
        };
        let (o, t) = (raw(&line.through)?, raw(&line.toward)?);
        let u = unit(o, t);
        Ok((
            o,
            (o.0 - u.0 * line.back, o.1 - u.1 * line.back),
            (o.0 + u.0 * line.forward, o.1 + u.1 * line.forward),
        ))
    }
    fn validate(&self) -> Result<(), String> {
        if self.nodes.len() > 200
            || self.segments.len()
                + self.angles.len()
                + self.labels.len()
                + self.dimensions.len()
                + self.points.len()
                + self.paths.len()
                + self.ellipses.len()
                + self.lines.len()
                > 500
        {
            return Err("Diagram exceeds 200 nodes or 500 drawing items".into());
        }
        if self.nodes.values().any(|p| !finite(*p)) {
            return Err("Node coordinates must be finite canvas coordinates".into());
        }
        let color = |c: &str| {
            if color_ok(c) {
                Ok(())
            } else {
                Err(format!("Color {c:?} must be #rrggbb"))
            }
        };
        let side = |a: &str, b: &str| -> Result<(), String> {
            let (a, b) = (self.node(a)?, self.node(b)?);
            if (a.0 - b.0).hypot(a.1 - b.1) <= 1e-6 {
                return Err("A segment or dimension has coincident ends".into());
            }
            Ok(())
        };
        let mut ids = std::collections::BTreeSet::new();
        for l in &self.lines {
            if !valid_id(&l.id)
                || !ids.insert(&l.id)
            {
                return Err(
                    "Constructive lines need unique alphanumeric ids (underscore/dash allowed)"
                        .into(),
                );
            }
            if self.nodes.contains_key(&format!("{}:back", l.id))
                || self.nodes.contains_key(&format!("{}:forward", l.id))
            {
                return Err("A base node cannot override a constructed line port".into());
            }
            let (o, _, _) = self.line_points(l)?;
            let t = self.nodes.get(&l.toward).unwrap();
            if (o.0 - t[0]).hypot(o.1 - t[1]) < 1e-6 {
                return Err("A constructive line needs distinct through/toward nodes".into());
            }
            color(&l.color)?;
            if !(0.2..=3000.0).contains(&l.back)
                || !(0.2..=3000.0).contains(&l.forward)
                || !(0.2..=12.0).contains(&l.width)
            {
                return Err(
                    "Constructive line lengths must be 0.2..3000 pixels and width 0.2..12".into(),
                );
            }
        }
        for e in &self.ellipses {
            if let Some(id) = &e.id {
                if !valid_id(id) || !ids.insert(id) {
                    return Err("Named ellipses and constructive lines need unique alphanumeric ids (underscore/dash allowed)".into());
                }
                if !self.nodes.contains_key(&e.center) {
                    return Err("A named ellipse needs a literal base center node; constructed ports cannot define its center".into());
                }
                for port in ELLIPSE_PORTS.iter().copied().chain(e.ports.keys().map(String::as_str)) {
                    if self.nodes.contains_key(&format!("{id}:{port}")) {
                        return Err("A base node cannot override a constructed ellipse port".into());
                    }
                }
            } else if !e.ports.is_empty() {
                return Err("Ellipse ports require an explicit ellipse id".into());
            }
            if e.ports.len() > 40 || e.ports.iter().any(|(k,a)| !valid_id(k) || ELLIPSE_PORTS.contains(&k.as_str()) || !a.is_finite() || a.abs() > 10000.0) {
                return Err("Ellipse ports need at most 40 unique non-reserved names and finite parameter angles within ±10000 degrees".into());
            }
        }
        for s in &self.segments {
            side(&s.from, &s.to)?;
            color(&s.color)?;
            if let Some(c) = &s.tick_color { color(c)?; }
            if let Some(c) = &s.parallel_color { color(c)?; }
            if !(0.2..=12.0).contains(&s.width) || s.ticks > 3 || s.parallel_marks > 3 || !s.label_offset.is_finite() {
                return Err("Segment width must be 0.2..12; ticks and parallel_marks must each be 0..3".into());
            }
        }
        for a in &self.angles {
            side(&a.vertex, &a.from)?;
            side(&a.vertex, &a.to)?;
            color(&a.color)?;
            color(&a.label_color)?;
            if a.arcs > 3 || !(2.0..=150.0).contains(&a.radius) {
                return Err("Angle arcs must be 0..3 and radius 2..150".into());
            }
            if !(0.0..=1500.0).contains(&a.label_radius) || !(4.0..=80.0).contains(&a.label_size) {
                return Err("Angle label radius must be 0..1500 pixels and size 4..80".into());
            }
            if a.label.chars().count() > 160 {
                return Err("A bound angle label may have at most 160 characters".into());
            }
            let (v, f, t) = (
                self.node(&a.vertex)?,
                self.node(&a.from)?,
                self.node(&a.to)?,
            );
            let (u, w) = (unit(v, f), unit(v, t));
            if a.right && ((u.0 * w.1 - u.1 * w.0).abs() < 1e-6 || a.arcs != 0) {
                return Err(format!(
                    "Right-angle marker at {:?} needs two noncollinear directions and arcs=0 (projected squares may be skewed)",
                    a.vertex
                ));
            }
        }
        for l in &self.labels {
            if l.node.is_some() == l.at.is_some() {
                return Err("A label needs exactly one of node or at".into());
            }
            if let Some(n) = &l.node {
                self.node(n)?;
            }
            if l.at.is_some_and(|p| !finite(p))
                || !finite(l.offset)
                || !(4.0..=80.0).contains(&l.size)
                || !(-360.0..=360.0).contains(&l.rotate)
            {
                return Err("Invalid label position or size".into());
            }
            color(&l.color)?;
        }
        for m in &self.dimensions {
            side(&m.from, &m.to)?;
            color(&m.color)?;
            color(&m.label_color)?;
            if !m.offset.is_finite() || m.offset.abs() > 1000.0 || !m.label_offset.is_finite() {
                return Err("Invalid dimension offset".into());
            }
        }
        for m in &self.points {
            self.node(&m.node)?;
            color(&m.color)?;
            if !(0.2..=20.0).contains(&m.radius) {
                return Err("Point radius must be 0.2..20".into());
            }
        }
        for path in &self.paths {
            let ok = match path.kind {
                PathKind::Polyline => (2..=200).contains(&path.nodes.len()),
                PathKind::Quadratic => path.nodes.len() == 3,
                PathKind::Cubic => path.nodes.len() == 4,
            };
            if !ok || !(0.2..=12.0).contains(&path.width) {
                return Err("Paths need 2..200 nodes for polyline, 3 for quadratic, 4 for cubic; width 0.2..12".into());
            }
            for n in &path.nodes {
                self.node(n)?;
            }
            color(&path.color)?;
        }
        for e in &self.ellipses {
            self.node(&e.center)?;
            color(&e.color)?;
            if !(0.2..=1500.0).contains(&e.rx)
                || !(0.2..=1500.0).contains(&e.ry)
                || !(0.2..=12.0).contains(&e.width)
                || !e.start.is_finite()
                || !e.end.is_finite()
                || e.start == e.end
                || e.start.abs() > 10000.0
                || e.end.abs() > 10000.0
            {
                return Err("Ellipse needs positive bounded radii/width and distinct finite start/end angles".into());
            }
        }
        Ok(())
    }
}

fn parse(p: &Params) -> Result<Diagram, String> {
    if p.s("diagram").len() > 100_000 {
        return Err("Diagram JSON exceeds 100000 bytes".into());
    }
    let g: Diagram =
        serde_json::from_str(p.s("diagram")).map_err(|e| format!("Invalid diagram JSON: {e}"))?;
    g.validate()?;
    let edges = semantics::edges(&g);
    for a in &g.angles {
        if !a.label.is_empty() && angle_label_position(&g, a, p, &edges).is_none() {
            return Err(format!(
                "Angle label {:?} at {:?} cannot fit inside its declared sector and canvas without crossing a ray; adjust its radius/size/canvas or use an explicit source leader",
                a.label, a.vertex
            ));
        }
    }
    Ok(g)
}
pub fn validate(p: &Params) -> Result<(), String> {
    parse(p).map(|_| ())
}
pub fn warnings(p: &Params) -> Vec<String> {
    match semantic_report(p) {
        Ok(r) => r.warnings,
        Err(e) => vec![e],
    }
}
pub fn semantic_report(p: &Params) -> Result<SemanticReport, String> {
    let g = parse(p)?;
    Ok(semantics::report(&g, p))
}

/// Negative labels whose minus sign is drawn on top of a horizontal stroke, where it cannot be
/// seen ("-3" on a grid line reads as "3"). Each message names the label and the stroke. These
/// are defects of the drawing itself, whatever the source shows.
pub fn hidden_minus_signs(p: &Params) -> Result<Vec<String>, String> {
    Ok(hidden_minus_labels(p)?.into_iter().map(|(_, m)| m).collect())
}

/// [`hidden_minus_signs`] with the index of each label in `labels`. A label with a halo is clear.
pub fn hidden_minus_labels(p: &Params) -> Result<Vec<(usize, String)>, String> {
    let g = parse(p)?;
    let mut out = Vec::new();
    for (index, l) in g.labels.iter().enumerate() {
        let Some(first) = l.text.trim_start().chars().next().filter(|c| *c == '-' || *c == '\u{2212}') else { continue };
        if l.rotate != 0.0 || l.halo { continue; }
        let base = match &l.node {
            Some(n) => g.node(n)?,
            None => l.at.map(|q| (q[0], q[1])).unwrap_or_default(),
        };
        let at = semantics::keep_on_canvas((base.0 + l.offset[0], base.1 + l.offset[1]), &l.text, l.size, p.f("width"), p.f("height"));
        let b = semantics::text_box(at, &l.text, l.size);
        // The minus is the first glyph. Its bar sits about 0.34 em above the baseline for U+2212 and
        // 0.27 em for a hyphen (measured on rendered labels: a 2 px grid line 0.34 em up hid "−1",
        // one 0.24 em up left it readable).
        let glyph = (b[2] - b[0]) / l.text.chars().count().max(1) as f64;
        let height = if first == '-' { 0.27 } else { 0.34 };
        let (x0, x1, y) = (b[0] + glyph * 0.15, b[0] + glyph * 0.85, at.1 - height * l.size);
        for s in &g.segments {
            let (Ok(a), Ok(c)) = (g.node(&s.from), g.node(&s.to)) else { continue };
            let (dx, dy) = (c.0 - a.0, c.1 - a.1);
            if dx.abs() < 1e-9 || dy.abs() > 0.05 * dx.abs() || a.0.min(c.0) > x1 || a.0.max(c.0) < x0 {
                continue;
            }
            let at_x = a.1 + dy * (((x0 + x1) / 2.0 - a.0) / dx);
            // Hidden means the line covers the bar, not merely touches it.
            if (at_x - y).abs() <= 0.05 * l.size + s.width / 2.0 {
                out.push((index, format!(
                    "Label {:?} at ({:.0}, {:.0}): its minus sign {first} is drawn on segment {:?}→{:?}, so it cannot be seen and the value reads as positive. Place the label clear of that line, as printed labels sit beside their grid line rather than on it.",
                    l.text, at.0, at.1, s.from, s.to
                )));
                break;
            }
        }
    }
    Ok(out)
}

fn unit(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let l = (b.0 - a.0).hypot(b.1 - a.1).max(1e-9);
    ((b.0 - a.0) / l, (b.1 - a.1) / l)
}
fn angle_span(v: (f64, f64), f: (f64, f64), t: (f64, f64), reflex: bool) -> (f64, f64) {
    let start = (-(f.1 - v.1)).atan2(f.0 - v.0).to_degrees();
    let end = (-(t.1 - v.1)).atan2(t.0 - v.0).to_degrees();
    let mut delta = (end - start).rem_euclid(360.0);
    if delta > 180.0 {
        delta -= 360.0;
    }
    if reflex {
        delta = if delta > 0.0 {
            delta - 360.0
        } else {
            delta + 360.0
        };
    }
    (start, delta)
}
fn inside_sector(v: (f64, f64), start: f64, delta: f64, q: (f64, f64)) -> bool {
    let a = (-(q.1 - v.1)).atan2(q.0 - v.0).to_degrees();
    let d = if delta >= 0.0 {
        (a - start).rem_euclid(360.0)
    } else {
        (start - a).rem_euclid(360.0)
    };
    d > 1e-8 && d < delta.abs() - 1e-8
}
fn angle_label_position(
    g: &Diagram,
    a: &Angle,
    p: &Params,
    edges: &[semantics::Edge],
) -> Option<(f64, f64)> {
    if a.label.is_empty() {
        return None;
    }
    let (v, f, t) = (
        g.node(&a.vertex).ok()?,
        g.node(&a.from).ok()?,
        g.node(&a.to).ok()?,
    );
    let (start, delta) = angle_span(v, f, t, a.reflex);
    let mid = (start + delta / 2.0).to_radians();
    let initial = if a.label_radius > 0.0 {
        a.label_radius
    } else {
        a.radius + 4.0 * a.arcs.saturating_sub(1) as f64 + 15.0
    };
    for i in 0..=1600 {
        let r = initial + i as f64 * 0.75;
        let at = (v.0 + r * mid.cos(), v.1 - r * mid.sin() + 5.0);
        let b = semantics::text_box(at, &a.label, a.label_size);
        if b[0] < 2.0 || b[1] < 2.0 || b[2] > p.f("width") - 2.0 || b[3] > p.f("height") - 2.0 {
            continue;
        }
        let corners = [(b[0], b[1]), (b[0], b[3]), (b[2], b[1]), (b[2], b[3])];
        if corners.iter().all(|q| inside_sector(v, start, delta, *q))
            && edges
                .iter()
                .all(|e| !semantics::segment_meets_box(e.a, e.b, b, e.width / 2.0 + 1.0))
        {
            return Some(at);
        }
    }
    None
}
fn label(d: &mut Doc, p: &Params, at: (f64, f64), size: f64, t: &str, c: &str) {
    if t.is_empty() {
        return;
    }
    let font = if p.s("font") == "Sans" {
        "Liberation Sans, DejaVu Sans, sans-serif"
    } else {
        "Liberation Serif, DejaVu Serif, serif"
    };
    d.raw(&format!(r#"<text x="{:.2}" y="{:.2}" font-family="{font}" font-size="{size}" text-anchor="middle" fill="{c}">{}</text>"#,at.0,at.1,svg::esc(t)));
}
/// A free label with a turn, bold or italic; plain labels keep [`label`]'s exact output.
fn styled_label(d: &mut Doc, p: &Params, at: (f64, f64), l: &Label) {
    if l.text.is_empty() {
        return;
    }
    let font = if p.s("font") == "Sans" { "Liberation Sans, DejaVu Sans, sans-serif" } else { "Liberation Serif, DejaVu Serif, serif" };
    let mut extra = String::new();
    if l.weight == FontWeight::Bold { extra.push_str(r#" font-weight="bold""#); }
    if l.style == FontStyle::Italic { extra.push_str(r#" font-style="italic""#); }
    if l.rotate != 0.0 { extra.push_str(&format!(r#" transform="rotate({:.2} {:.2} {:.2})""#, l.rotate, at.0, at.1)); }
    if l.halo { extra.push_str(&format!(r##" stroke="#fff" stroke-width="{:.2}" stroke-linejoin="round" paint-order="stroke""##, l.size * 0.22)); }
    d.raw(&format!(r#"<text x="{:.2}" y="{:.2}" font-family="{font}" font-size="{}" text-anchor="middle" fill="{}"{extra}>{}</text>"#, at.0, at.1, l.size, l.color, svg::esc(&l.text)));
}
fn segment(
    d: &mut Doc,
    a: (f64, f64),
    b: (f64, f64),
    c: &str,
    w: f64,
    style: LineStyle,
    arrows: Arrows,
) {
    match arrows {
        Arrows::End | Arrows::Both => {
            d.arrow_with(
                a.0,
                a.1,
                b.0,
                b.1,
                c,
                w,
                style.dash(),
                matches!(arrows, Arrows::Both),
            );
        }
        Arrows::Start => {
            d.arrow_with(b.0, b.1, a.0, a.1, c, w, style.dash(), false);
        }
        Arrows::None => {
            d.raw(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{c}" stroke-width="{w}" stroke-dasharray="{}"/>"#,a.0,a.1,b.0,b.1,style.dash()));
        }
    }
}
fn arrow_tip(d: &mut Doc, tip: (f64, f64), back: (f64, f64), color: &str, width: f64) {
    if (tip.0 - back.0).hypot(tip.1 - back.1) < 1e-6 {
        return;
    }
    let u = unit(back, tip);
    let n = (-u.1, u.0);
    let len = (width * 4.0).max(7.0);
    let b = (tip.0 - u.0 * len, tip.1 - u.1 * len);
    d.poly(
        &[
            tip,
            (b.0 + n.0 * len * 0.35, b.1 + n.1 * len * 0.35),
            (b.0 - n.0 * len * 0.35, b.1 - n.1 * len * 0.35),
        ],
        color,
        color,
        0.5,
    );
}
fn attr(s: &str) -> String {
    svg::esc(s).replace('"', "&quot;")
}
fn render(p: &Params) -> String {
    let g = match parse(p) {
        Ok(g) => g,
        Err(e) => return super::chart::message(&e),
    };
    let mut d = Doc::new(p.f("width"), p.f("height"));
    let edges = semantics::edges(&g);
    for l in &g.lines {
        let (o, back, forward) = g.line_points(l).unwrap();
        d.raw(&format!(
            r#"<g data-through-line="{}" data-through-node="{}">"#,
            attr(&l.id),
            attr(&l.through)
        ));
        segment(
            &mut d,
            o,
            back,
            &l.color,
            l.width,
            l.style,
            if matches!(l.arrows, Arrows::Start | Arrows::Both) {
                Arrows::End
            } else {
                Arrows::None
            },
        );
        segment(
            &mut d,
            o,
            forward,
            &l.color,
            l.width,
            l.style,
            if matches!(l.arrows, Arrows::End | Arrows::Both) {
                Arrows::End
            } else {
                Arrows::None
            },
        );
        d.raw("</g>");
    }
    for e in &g.ellipses {
        let c = g.node(&e.center).unwrap();
        let span = (e.end - e.start).clamp(-360.0, 360.0);
        d.raw(&format!(
            r#"<g data-ellipse="{}" stroke-dasharray="{}">"#,
            attr(&e.center),
            e.style.dash()
        ));
        if span.abs() > 359.999 {
            d.raw(&format!(r#"<ellipse cx="{:.2}" cy="{:.2}" rx="{:.2}" ry="{:.2}" fill="none" stroke="{}" stroke-width="{}"/>"#,c.0,c.1,e.rx,e.ry,e.color,e.width));
        } else {
            let q = |a: f64| {
                let a = a.to_radians();
                (c.0 + e.rx * a.cos(), c.1 - e.ry * a.sin())
            };
            let (a, b) = (q(e.start), q(e.start + span));
            d.path(
                &format!(
                    "M{:.2},{:.2} A{:.2},{:.2} 0 {} {} {:.2},{:.2}",
                    a.0,
                    a.1,
                    e.rx,
                    e.ry,
                    if span.abs() > 180.0 { 1 } else { 0 },
                    if span > 0.0 { 0 } else { 1 },
                    b.0,
                    b.1
                ),
                "none",
                &e.color,
                e.width,
            );
        }
        d.raw("</g>");
    }
    for path in &g.paths {
        let pts: Vec<_> = path.nodes.iter().map(|n| g.node(n).unwrap()).collect();
        let mut cmd = format!("M{:.2},{:.2}", pts[0].0, pts[0].1);
        match path.kind {
            PathKind::Polyline => {
                for q in &pts[1..] {
                    cmd.push_str(&format!(" L{:.2},{:.2}", q.0, q.1));
                }
            }
            PathKind::Quadratic => cmd.push_str(&format!(
                " Q{:.2},{:.2} {:.2},{:.2}",
                pts[1].0, pts[1].1, pts[2].0, pts[2].1
            )),
            PathKind::Cubic => cmd.push_str(&format!(
                " C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}",
                pts[1].0, pts[1].1, pts[2].0, pts[2].1, pts[3].0, pts[3].1
            )),
        }
        d.raw(&format!(
            r#"<g data-explicit-path="1" stroke-dasharray="{}">"#,
            path.style.dash()
        ));
        d.path(&cmd, "none", &path.color, path.width);
        if matches!(path.arrows, Arrows::Start | Arrows::Both) {
            arrow_tip(&mut d, pts[0], pts[1], &path.color, path.width);
        }
        if matches!(path.arrows, Arrows::End | Arrows::Both) {
            let n = pts.len();
            arrow_tip(&mut d, pts[n - 1], pts[n - 2], &path.color, path.width);
        }
        d.raw("</g>");
    }
    for s in &g.segments {
        let (a, b) = (g.node(&s.from).unwrap(), g.node(&s.to).unwrap());
        let u = unit(a, b);
        let n = (-u.1, u.0);
        d.raw(&format!(
            r#"<g data-segment="{}:{}">"#,
            attr(&s.from),
            attr(&s.to)
        ));
        segment(&mut d, a, b, &s.color, s.width, s.style, s.arrows);
        for i in 0..s.ticks {
            let k = (i as f64 - (s.ticks as f64 - 1.0) / 2.0) * 5.0;
            let c = ((a.0 + b.0) / 2.0 + u.0 * k, (a.1 + b.1) / 2.0 + u.1 * k);
            d.line(
                c.0 - n.0 * 5.0,
                c.1 - n.1 * 5.0,
                c.0 + n.0 * 5.0,
                c.1 + n.1 * 5.0,
                s.tick_color.as_deref().unwrap_or(&s.color),
                1.4,
            );
        }
        if s.parallel_marks > 0 {
            d.raw(&format!(r#"<g data-parallel-from="{}" data-parallel-to="{}" data-parallel-count="{}">"#,attr(&s.from),attr(&s.to),s.parallel_marks));
            for i in 0..s.parallel_marks {
                let k = (i as f64 - (s.parallel_marks as f64 - 1.0) / 2.0) * 8.0;
                let c = ((a.0 + b.0) / 2.0 + u.0 * k, (a.1 + b.1) / 2.0 + u.1 * k);
                d.polyline(&[
                    (c.0 - u.0 * 3.5 + n.0 * 4.0, c.1 - u.1 * 3.5 + n.1 * 4.0),
                    (c.0 + u.0 * 3.5, c.1 + u.1 * 3.5),
                    (c.0 - u.0 * 3.5 - n.0 * 4.0, c.1 - u.1 * 3.5 - n.1 * 4.0),
                ],s.parallel_color.as_deref().unwrap_or(&s.color),1.4);
            }
            d.raw("</g>");
        }
        label(
            &mut d,
            p,
            (
                (a.0 + b.0) / 2.0 + n.0 * s.label_offset,
                (a.1 + b.1) / 2.0 + n.1 * s.label_offset + 5.0,
            ),
            18.0,
            &s.label,
            &s.color,
        );
        d.raw("</g>");
    }
    for a in &g.angles {
        let (v, f, t) = (
            g.node(&a.vertex).unwrap(),
            g.node(&a.from).unwrap(),
            g.node(&a.to).unwrap(),
        );
        let (u, w) = (unit(v, f), unit(v, t));
        let (start, delta) = angle_span(v, f, t, a.reflex);
        d.raw(&format!(
            r#"<g data-angle-vertex="{}" data-arc-count="{}">"#,
            attr(&a.vertex),
            a.arcs
        ));
        if a.right {
            d.right_angle(v, u, w, a.radius, &a.color, 1.4);
        } else {
            for i in 0..a.arcs {
                d.arc(
                    v.0,
                    v.1,
                    a.radius + i as f64 * 4.0,
                    start,
                    start + delta,
                    &a.color,
                    1.4,
                );
            }
        }
        if let Some(at) = angle_label_position(&g, a, p, &edges) {
            label(&mut d, p, at, a.label_size, &a.label, &a.label_color);
        }
        d.raw("</g>");
    }
    for m in &g.dimensions {
        let (a, b) = (g.node(&m.from).unwrap(), g.node(&m.to).unwrap());
        let u = unit(a, b);
        let n = (-u.1, u.0);
        let (a, b) = (
            (a.0 + n.0 * m.offset, a.1 + n.1 * m.offset),
            (b.0 + n.0 * m.offset, b.1 + n.1 * m.offset),
        );
        d.raw(&format!(
            r#"<g data-dimension="{}:{}">"#,
            attr(&m.from),
            attr(&m.to)
        ));
        segment(
            &mut d,
            a,
            b,
            &m.color,
            1.2,
            LineStyle::Solid,
            if matches!(m.ends, DimensionEnds::Ticks) {
                Arrows::None
            } else {
                Arrows::Both
            },
        );
        if !matches!(m.ends, DimensionEnds::Arrows) {
            for q in [a, b] {
                d.line(
                    q.0 - n.0 * 6.0,
                    q.1 - n.1 * 6.0,
                    q.0 + n.0 * 6.0,
                    q.1 + n.1 * 6.0,
                    &m.color,
                    1.2,
                );
            }
        }
        label(
            &mut d,
            p,
            (
                (a.0 + b.0) / 2.0 + n.0 * m.label_offset,
                (a.1 + b.1) / 2.0 + n.1 * m.label_offset + 5.0,
            ),
            18.0,
            &m.label,
            &m.label_color,
        );
        d.raw("</g>");
    }
    for m in &g.points {
        let q = g.node(&m.node).unwrap();
        d.circle(q.0, q.1, m.radius, &m.color, &m.color, 0.0);
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
        let at = semantics::keep_turned_on_canvas((q.0 + l.offset[0], q.1 + l.offset[1]), &l.text, l.size, l.rotate, p.f("width"), p.f("height"));
        if l.rotate == 0.0 && l.weight == FontWeight::Normal && l.style == FontStyle::Normal && !l.halo {
            label(&mut d, p, at, l.size, &l.text, &l.color);
        } else {
            styled_label(&mut d, p, at, l);
        }
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{self, Value, spec};
    fn draw(j: serde_json::Value) -> (Params, String) {
        let d = def();
        let mut p = figures::start_params(&d);
        p.0.insert("diagram".into(), Value::S(j.to_string()));
        let s = d.svg(&p);
        (p, s)
    }
    #[test]
    fn a_haloed_label_keeps_its_minus_sign_visible_on_a_line() {
        let label = |halo: bool| serde_json::json!({"nodes":{"L":[0,200],"R":[380,200]},"segments":[{"from":"L","to":"R","color":"#999999"}],
            "labels":[{"text":"\u{2212}3","at":[40.0, 200.0 + 0.3 * 18.0],"size":18,"halo":halo}]});
        let (p, plain) = draw(label(false));
        assert_eq!(hidden_minus_labels(&p).unwrap().iter().map(|(i, _)| *i).collect::<Vec<_>>(), [0]);
        assert!(!plain.contains("paint-order"));
        let (p, s) = draw(label(true));
        assert!(hidden_minus_labels(&p).unwrap().is_empty());
        assert!(s.contains(r##"stroke="#fff""##) && s.contains(r#"paint-order="stroke""#), "{s}");
    }
    #[test]
    fn a_minus_sign_drawn_on_a_grid_line_is_found_and_one_beside_it_is_not() {
        // Round 24, trig graphs: "-3" centred on its grid line reads as "3".
        let on = |dy: f64| {
            let (p, _) = draw(serde_json::json!({"nodes":{"L":[0,200],"R":[380,200]},"segments":[{"from":"L","to":"R","color":"#999999"}],
                "labels":[{"text":"\u{2212}3","at":[40.0, 200.0 + 0.3 * 18.0 + dy],"size":18}]}));
            hidden_minus_signs(&p).unwrap()
        };
        assert_eq!(on(0.0).len(), 1, "{:?}", on(0.0));
        assert!(on(0.0)[0].contains("cannot be seen"));
        assert!(on(-12.0).is_empty(), "a label beside its line keeps its sign");
        assert!(on(-2.5).is_empty(), "a bar just above its line stays readable (round 22)");
        let (p, _) = draw(serde_json::json!({"nodes":{"L":[0,200],"R":[380,200]},"segments":[{"from":"L","to":"R"}],"labels":[{"text":"3","at":[40.0,205.4]}]}));
        assert!(hidden_minus_signs(&p).unwrap().is_empty(), "no minus, nothing to hide");
    }
    #[test]
    fn separate_and_shared_triangles_keep_edges_and_literal_labels_without_solving() {
        let j = serde_json::json!({"nodes":{"A":[30,30],"B":[80,180],"C":[140,30],"D":[210,30],"E":[250,180],"F":[330,30],"P":[55,105],"Q":[110,105]},"segments":[{"from":"A","to":"B","label":"11x − 4"},{"from":"B","to":"C"},{"from":"C","to":"A"},{"from":"D","to":"E"},{"from":"E","to":"F"},{"from":"F","to":"D"},{"from":"P","to":"Q","label":"?"}]});
        let (p, s) = draw(j);
        assert_eq!(s.matches("data-segment=").count(), 7);
        assert!(s.contains("11x − 4"));
        assert!(s.contains(">?</text>"));
        assert!(!s.contains("<circle"));
        assert!(!s.contains("°"));
        assert_eq!(
            spec::params_from_spec(&def(), &spec::spec_for("geometry-diagram", &p)).0,
            p.0
        );
    }
    #[test]
    fn exact_congruence_arc_counts_stay_with_their_named_vertices() {
        let (_, s) = draw(
            serde_json::json!({"nodes":{"U":[40,50],"T":[180,50],"X":[100,180],"J":[240,50],"K":[360,50],"Y":[300,180]},"angles":[{"vertex":"U","from":"T","to":"X","arcs":2},{"vertex":"T","from":"U","to":"X","arcs":1},{"vertex":"J","from":"K","to":"Y","arcs":1},{"vertex":"K","from":"J","to":"Y","arcs":2}]}),
        );
        for (v, n) in [("U", 2), ("T", 1), ("J", 1), ("K", 2)] {
            let g = s
                .split(&format!("<g data-angle-vertex=\"{v}\""))
                .nth(1)
                .unwrap()
                .split("</g>")
                .next()
                .unwrap();
            assert_eq!(g.matches("<path ").count(), n, "{v}");
        }
    }
    #[test]
    fn colored_right_square_and_parallel_double_arrow_dimension_are_explicit() {
        let (_, s) = draw(
            serde_json::json!({"nodes":{"A":[40,40],"B":[40,180],"C":[230,180]},"angles":[{"vertex":"B","from":"A","to":"C","right":true,"color":"#ff0000"}],"dimensions":[{"from":"B","to":"C","offset":30,"label":"x + 2","color":"#0000ff","ends":"arrows_and_ticks"}]}),
        );
        assert_eq!(s.matches("<polygon ").count(), 2);
        assert!(s.contains("stroke=\"#ff0000\""));
        let dim = s
            .split("data-dimension=\"B:C\"")
            .nth(1)
            .unwrap()
            .split("</g>")
            .next()
            .unwrap();
        assert!(dim.contains("y1=\"210.00\" y2=\"210.00\"") || dim.contains("y1=\"210.00\""));
        assert!(dim.contains("x + 2"));
    }

    #[test]
    fn ticks_have_independent_color_without_recoloring_the_authored_edge() {
        let (p,s)=draw(serde_json::json!({"nodes":{"A":[50,80],"B":[270,80]},"segments":[{"from":"A","to":"B","color":"#000000","ticks":2,"tick_color":"#ff0000"}]}));
        let xml=roxmltree::Document::parse(&s).unwrap();
        let lines:Vec<_>=xml.descendants().filter(|n|n.has_tag_name("line")).collect();
        assert_eq!(lines.len(),3);
        assert_eq!(lines[0].attribute("stroke"),Some("#000000"));
        assert!(lines[1..].iter().all(|n|n.attribute("stroke")==Some("#ff0000")));
        let r=semantic_report(&p).unwrap();
        assert_eq!(r.segment_annotations.len(),1);
        assert_eq!(r.segment_annotations[0].ticks,2);
        assert_eq!(r.segment_annotations[0].tick_color,"#ff0000");
        assert!(r.arrowed_strokes.is_empty());
        let j=serde_json::json!({"nodes":{"A":[50,80],"B":[270,80]},"segments":[{"from":"A","to":"B","color":"#0000ff","ticks":2}]});
        let (_,old_default)=draw(j.clone());let mut explicit=j;explicit["segments"][0]["tick_color"]=serde_json::json!("#0000ff");
        assert_eq!(old_default,draw(explicit).1,"omitted tick color keeps the edge color exactly");
    }

    #[test]
    fn parallel_chevrons_are_explicit_middle_marks_not_endpoint_arrows_or_inferred_pairs() {
        let (p,s)=draw(serde_json::json!({"nodes":{"A":[50,80],"B":[270,80],"C":[50,150],"D":[270,150]},"segments":[{"from":"A","to":"B","parallel_marks":2,"parallel_color":"#ff0000"},{"from":"C","to":"D"}]}));
        let xml=roxmltree::Document::parse(&s).unwrap();
        let marks:Vec<_>=xml.descendants().filter(|n|n.has_tag_name("polyline")).collect();
        assert_eq!(marks.len(),2);
        assert!(marks.iter().all(|n|n.attribute("stroke")==Some("#ff0000")));
        assert_eq!(s.matches("data-parallel-count=").count(),1,"unmarked edge gets no inferred parallel mark");
        assert!(!s.contains("<polygon"),"no endpoint arrowheads are added");
        let r=semantic_report(&p).unwrap();
        assert_eq!(r.segment_annotations.len(),1);
        assert_eq!(r.segment_annotations[0].parallel_marks,2);
        assert_eq!(r.segment_annotations[0].parallel_color,"#ff0000");
        assert!(r.arrowed_strokes.is_empty());
        assert!(crate::figures::to_png(&s,1.0).is_ok());
    }

    #[test]
    fn point_and_leader_inventory_uses_declared_shared_nodes_and_aliases_only() {
        let (p,s)=draw(serde_json::json!({"nodes":{"O":[80,100],"M":[140,100],"B":[220,100],"free":[140,100],"S":[230,230],"C":[100,210]},"ellipses":[{"id":"rim","center":"O","rx":50,"ry":20}],"segments":[{"from":"rim:center","to":"M"},{"from":"M","to":"B"}],"points":[{"node":"O","color":"#ff0000"},{"node":"free"}],"paths":[{"nodes":["S","C","M"],"kind":"quadratic","arrows":"end"},{"nodes":["S","C","free"],"kind":"quadratic","arrows":"end"}]}));
        let original=p.s("diagram").to_string();let r=semantic_report(&p).unwrap();
        assert_eq!(r.point_markers.len(),2);
        assert_eq!(r.point_markers[0].connected_edges,[["rim:center".to_string(),"M".to_string()]],"ellipse:center is an explicitly declared alias");
        assert!(r.point_markers[1].connected_edges.is_empty(),"equal coordinates do not create a named join");
        assert_eq!(r.arrowed_strokes.len(),2);
        assert_eq!(r.arrowed_strokes[0].primitive,"quadratic");
        assert_eq!(r.arrowed_strokes[0].targets[0].node,"M");
        assert_eq!(r.arrowed_strokes[0].targets[0].connected_edges.len(),2);
        assert!(r.arrowed_strokes[1].targets[0].connected_edges.is_empty());
        assert_eq!(p.s("diagram"),original);
        assert_eq!(def().svg(&p),s,"inventory never changes geometry or fills a point/guide connection");
    }

    #[test]
    fn malformed_independent_mark_colors_and_counts_are_tool_errors() {
        for fields in [
            serde_json::json!({"ticks":4}),serde_json::json!({"parallel_marks":4}),
            serde_json::json!({"tick_color":"red"}),serde_json::json!({"parallel_color":"#zz0000"}),
        ] {
            let mut segment=serde_json::json!({"from":"A","to":"B"});for(k,v)in fields.as_object().unwrap(){segment[k]=v.clone();}
            let(p,_)=draw(serde_json::json!({"nodes":{"A":[50,80],"B":[270,80]},"segments":[segment]}));
            assert!(validate(&p).is_err());
        }
    }
    #[test]
    fn invalid_topology_unknown_fields_and_false_right_angles_are_rejected() {
        for j in [
            serde_json::json!({"nodes":{"A":[0,0]},"segments":[{"from":"A","to":"missing"}]}),
            serde_json::json!({"nodes":{},"segements":[]}),
            serde_json::json!({"nodes":{"A":[0,0],"B":[5,0],"C":[10,0]},"angles":[{"vertex":"A","from":"B","to":"C","right":true}]}),
        ] {
            let (p, s) = draw(j);
            assert!(validate(&p).is_err());
            assert!(!s.contains("data-segment="));
        }
    }
    #[test]
    fn a_label_can_turn_and_be_bold_or_italic_and_plain_labels_are_unchanged() {
        // Holdout v12: printed y-axis titles read bottom to top, bold titles, italic axis letters.
        let (_, s) = draw(serde_json::json!({"labels":[{"at":[20,150],"text":"Number of Pictures Taken","rotate":-90,"weight":"bold"},{"at":[200,20],"text":"x","style":"italic"}]}));
        assert!(s.contains(r#"font-weight="bold" transform="rotate(-90.00 "#), "{s}");
        assert!(s.contains(r#"font-style="italic""#));
        assert_eq!(s.matches("Number of Pictures Taken").count(), 1, "one continuous label");
        // A long turned title near the left edge stays where it was put: its turned box is narrow.
        assert!(s.contains(r#"x="20.00" y="150.00""#), "{s}");
        let (_, plain) = draw(serde_json::json!({"labels":[{"at":[50,50],"text":"A"}]}));
        assert!(!plain.contains("transform=\"rotate") && !plain.contains("font-weight"));
        let d = def();
        let mut p = figures::start_params(&d);
        p.0.insert("diagram".into(), Value::S(serde_json::json!({"labels":[{"at":[5,5],"text":"A","rotate":720}]}).to_string()));
        assert!(validate(&p).unwrap_err().contains("Invalid label position or size"));
    }
    #[test]
    fn text_is_escaped_and_default_is_rasterizable() {
        let (_, s) = draw(serde_json::json!({"labels":[{"at":[50,50],"text":"<x & y>"}]}));
        assert!(s.contains("&lt;x &amp; y&gt;"));
        let d = def();
        assert!(figures::to_png(&d.svg(&figures::start_params(&d)), 1.0).is_ok());
    }

    #[test]
    fn projected_solids_keep_bottom_ellipse_dashed_radius_and_curved_leader() {
        let (_, s) = draw(
            serde_json::json!({"nodes":{"top":[180,65],"bottom":[180,205],"tl":[80,65],"tr":[280,65],"bl":[80,205],"br":[280,205],"L":[245,270],"control":[220,250],"target":[230,205]},"ellipses":[{"center":"top","rx":100,"ry":25},{"center":"bottom","rx":100,"ry":25}],"segments":[{"from":"tl","to":"bl"},{"from":"tr","to":"br"},{"from":"bottom","to":"br","style":"dotted","color":"#ff0000"}],"paths":[{"nodes":["L","control","target"],"kind":"quadratic","arrows":"end"}],"labels":[{"node":"L","offset":[0,15],"text":"r cm"}]}),
        );
        let xml = roxmltree::Document::parse(&s).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|n| n.has_tag_name("ellipse"))
                .count(),
            2
        );
        assert!(xml.descendants().any(|n| n.has_tag_name("line")
            && n.attribute("stroke") == Some("#ff0000")
            && n.attribute("stroke-dasharray") == Some("2 4")));
        assert!(s.contains("Q220.00,250.00 230.00,205.00"));
        assert_eq!(s.matches("<polygon ").count(), 1);
        assert!(!s.contains("<circle"), "no inferred center dot");
        assert!(crate::figures::to_png(&s, 1.0).is_ok());
    }

    #[test]
    fn projected_right_marker_keeps_a_source_square_on_skewed_face_edges() {
        let (p, s) = draw(
            serde_json::json!({"nodes":{"A":[50,50],"B":[170,70],"C":[90,200]},"angles":[{"vertex":"A","from":"B","to":"C","right":true,"color":"#ff0000"}]}),
        );
        assert!(validate(&p).is_ok());
        assert!(s.contains("stroke=\"#ff0000\""));
    }

    #[test]
    fn constructive_lines_share_one_exact_vertex_and_opposite_named_ports() {
        let (p, s) = draw(
            serde_json::json!({"nodes":{"O":[180,140],"A":[260,140],"B":[215,50],"C":[260,65]},"lines":[{"id":"axis","through":"O","toward":"A","back":120,"forward":120,"arrows":"both"},{"id":"steep","through":"O","toward":"B","back":105,"forward":105,"arrows":"both"},{"id":"diagonal","through":"O","toward":"C","back":100,"forward":100,"arrows":"both"}]}),
        );
        let g = parse(&p).unwrap();
        for l in &g.lines {
            let (o, back, forward) = g.line_points(l).unwrap();
            assert_eq!(o, (180.0, 140.0));
            let (u, v) = (unit(o, back), unit(o, forward));
            assert!((u.0 * v.1 - u.1 * v.0).abs() < 1e-12);
            assert!((u.0 * v.0 + u.1 * v.1 + 1.0).abs() < 1e-12);
            assert_eq!(g.node(&format!("{}:back", l.id)).unwrap(), back);
        }
        let r = semantic_report(&p).unwrap();
        assert_eq!(r.incidences["O"].len(), 6);
        assert_eq!(r.through_lines.len(), 3);
        assert!(
            !r.warnings
                .iter()
                .any(|w| w.contains("small triangle") || w.contains("kink"))
        );
        let xml = roxmltree::Document::parse(&s).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|n| n.attribute("data-through-node") == Some("O"))
                .count(),
            3
        );
        assert_eq!(s.matches("x1=\"180.00\" y1=\"140.00\"").count(), 6);
        assert!(crate::figures::to_png(&s, 1.0).is_ok());
    }

    #[test]
    fn conditional_incidence_warnings_never_snap_a_real_small_triangle_or_kink() {
        let (p, s) = draw(
            serde_json::json!({"nodes":{"A":[20,100],"B":[180,100],"C":[100,20],"D":[103,180],"E":[20,179],"F":[180,21]},"segments":[{"from":"A","to":"B","arrows":"both"},{"from":"C","to":"D","arrows":"both"},{"from":"E","to":"F","arrows":"both"}]}),
        );
        let original = p.s("diagram").to_string();
        let r = semantic_report(&p).unwrap();
        assert!(r.warnings.iter().any(|w| w.contains("small triangle")));
        assert!(r.through_lines.is_empty());
        assert_eq!(p.s("diagram"), original);
        assert_eq!(def().svg(&p), s);
        let (p, _) = draw(
            serde_json::json!({"nodes":{"O":[120,120],"U":[125,20],"D":[108,220]},"segments":[{"from":"O","to":"U","arrows":"end"},{"from":"O","to":"D","arrows":"end"}]}),
        );
        let r = semantic_report(&p).unwrap();
        assert!(r.warnings.iter().any(|w| w.contains("kink")));
        assert!(r.through_lines.is_empty());
        assert!(
            validate(&p).is_ok(),
            "a genuine nonstraight source is not rejected or converted"
        );
    }

    #[test]
    fn free_branch_near_host_reports_distance_and_stroke_allowance_without_snapping() {
        let (p, svg) = draw(serde_json::json!({
            "nodes":{"K":[20,20],"L":[180,140],"B":[97,84],"C":[97,200]},
            "segments":[{"from":"K","to":"L","width":3},{"from":"B","to":"C","width":3}]
        }));
        let original = p.s("diagram").to_string();
        let png = figures::to_png(&svg, 1.0).unwrap();
        let r = semantic_report(&p).unwrap();
        let c = &r.endpoint_near_host_check.candidates[0];
        assert_eq!((c.branch_node.as_str(), c.branch_edge.as_str(), c.host_edge.as_str()),
            ("B", "B:C", "K:L"));
        assert!((c.projection_fraction - 0.5).abs() < 1e-12);
        assert!((c.centerline_distance - 5.0).abs() < 1e-12);
        assert!((c.combined_half_stroke_allowance - 3.0).abs() < 1e-12);
        assert!((c.clearance_after_allowance - 2.0).abs() < 1e-12);
        assert_eq!(c.stroke_envelopes, "separated");
        assert!(!c.declared_incidence);
        assert!(r.warnings.iter().any(|w| w.contains("no declared incidence")));
        assert_eq!(p.s("diagram"), original);
        assert_eq!(def().svg(&p), svg, "an intended nonjoin remains untouched");
        assert_eq!(figures::to_png(&def().svg(&p), 1.0).unwrap(), png);
        assert!(validate(&p).is_ok(), "a nearby endpoint is not a validation failure");
    }

    #[test]
    fn explicit_join_is_not_free_and_touching_envelopes_do_not_declare_a_join() {
        let (p, _) = draw(serde_json::json!({
            "nodes":{"K":[20,80],"L":[220,80],"B":[120,80],"C":[120,180]},
            "segments":[{"from":"K","to":"B"},{"from":"B","to":"L"},{"from":"B","to":"C"}]
        }));
        assert!(semantic_report(&p).unwrap().endpoint_near_host_check.candidates.is_empty());
        let (p, _) = draw(serde_json::json!({
            "nodes":{"O":[120,80],"T":[220,80],"C":[120,180]},
            "lines":[{"id":"host","through":"O","toward":"T","back":100,"forward":100}],
            "segments":[{"from":"O","to":"C"}]
        }));
        assert!(semantic_report(&p).unwrap().endpoint_near_host_check.candidates.is_empty(),
            "an explicit through-line node is already a declared join");
        let (p, _) = draw(serde_json::json!({
            "nodes":{"K":[20,80],"L":[220,80],"B":[120,81],"C":[120,180]},
            "segments":[{"from":"K","to":"L","width":2,"style":"dashed"},{"from":"B","to":"C","width":2}]
        }));
        let r = semantic_report(&p).unwrap();
        let c = &r.endpoint_near_host_check.candidates[0];
        assert_eq!(c.stroke_envelopes, "overlap_or_touch");
        assert_eq!(c.clearance_after_allowance, -1.0);
        assert!(!c.declared_incidence);
        assert!(r.endpoint_near_host_check.scope.contains("not pixel contact"));
        assert_eq!(r.stroke_inventory[0].style, "dashed");
    }

    #[test]
    fn candidate_stroke_inventory_keeps_authored_pattern_color_and_named_ends() {
        let (p, _) = draw(serde_json::json!({
            "nodes":{"A":[50,100],"B":[180,100],"C":[100,180],"D":[200,180]},
            "segments":[{"from":"A","to":"B","color":"#777777","width":2}],
            "lines":[{"id":"host","through":"A","toward":"B","style":"dashed","width":3}],
            "paths":[{"nodes":["A","C","B"],"kind":"quadratic","style":"dotted","color":"#ff0000","width":4}],
            "ellipses":[{"center":"C","rx":30,"ry":15},{"id":"rim","center":"D","rx":25,"ry":12,"style":"dashed"}]
        }));
        let r = semantic_report(&p).unwrap();
        let s = &r.stroke_inventory;
        assert_eq!(r.stroke_inventory_total, 5);
        assert!(!r.stroke_inventory_truncated);
        assert_eq!((s[0].primitive,s[0].style,s[0].color.as_str(),s[0].width),
            ("segment","solid","#777777",2.0));
        assert_eq!(s[1].endpoints, Some(["host:back".into(),"host:forward".into()]));
        assert_eq!((s[2].primitive,s[2].style,s[2].color.as_str(),s[2].width),
            ("quadratic","dotted","#ff0000",4.0));
        assert_eq!(s[2].endpoints, Some(["A".into(),"B".into()]));
        assert_eq!((s[3].primitive,s[3].id.as_ref(),s[3].endpoints.as_ref()), ("ellipse",None,None));
        assert_eq!(s[4].endpoints, Some(["rim:start".into(),"rim:end".into()]));
        assert!(r.stroke_inventory_scope.contains("style does not imply a source role"));
    }

    #[test]
    fn endpoint_feedback_has_explicit_search_and_output_limits() {
        let mut nodes = serde_json::Map::new();
        nodes.insert("K".into(), serde_json::json!([0,80]));
        nodes.insert("L".into(), serde_json::json!([1000,80]));
        let mut segments = vec![serde_json::json!({"from":"K","to":"L"})];
        for i in 0..90 {
            nodes.insert(format!("B{i}"), serde_json::json!([10+i*10,82]));
            nodes.insert(format!("C{i}"), serde_json::json!([10+i*10,180]));
            segments.push(serde_json::json!({"from":format!("B{i}"),"to":format!("C{i}")}));
        }
        for _ in 0..50 { segments.push(serde_json::json!({"from":"K","to":"L"})); }
        let (p, _) = draw(serde_json::json!({"nodes":nodes,"segments":segments}));
        let r = semantic_report(&p).unwrap();
        let check = &r.endpoint_near_host_check;
        assert_eq!(check.analyzed_straight_edges, 128);
        assert_eq!(check.edge_limit, 128);
        assert!(check.analysis_truncated && check.candidates_truncated);
        assert_eq!(check.candidates.len(), check.candidate_limit);
        assert_eq!(check.candidate_limit, 32);
        assert!(r.warnings.len() <= 12);
        assert_eq!(r.stroke_inventory.len(), 128);
        assert!(r.stroke_inventory_truncated);
    }

    #[test]
    fn semantic_feedback_leaves_default_svg_png_and_params_unchanged() {
        let p = figures::start_params(&def());
        let before = def().svg(&p);
        let params_before = spec::spec_for("geometry-diagram", &p);
        let png = figures::to_png(&before, 1.0).unwrap();
        let _ = semantic_report(&p).unwrap();
        assert_eq!(spec::spec_for("geometry-diagram", &p), params_before);
        assert_eq!(def().svg(&p), before);
        assert_eq!(figures::to_png(&def().svg(&p), 1.0).unwrap(), png);
    }

    #[test]
    fn literal_angle_label_stays_inside_its_declared_sector_without_adding_an_arc() {
        let (p, s) = draw(
            serde_json::json!({"nodes":{"O":[180,230],"A":[140,30],"B":[195,20]},"segments":[{"from":"O","to":"A","arrows":"end"},{"from":"O","to":"B","arrows":"end"}],"angles":[{"vertex":"O","from":"A","to":"B","label":"b","arcs":0,"label_size":14,"label_radius":10}]}),
        );
        let g = parse(&p).unwrap();
        let a = &g.angles[0];
        let e = semantics::edges(&g);
        let at = angle_label_position(&g, a, &p, &e).unwrap();
        let b = semantics::text_box(at, &a.label, a.label_size);
        let (v, f, t) = (
            g.node("O").unwrap(),
            g.node("A").unwrap(),
            g.node("B").unwrap(),
        );
        let (start, delta) = angle_span(v, f, t, false);
        for q in [(b[0], b[1]), (b[0], b[3]), (b[2], b[1]), (b[2], b[3])] {
            assert!(inside_sector(v, start, delta, q));
        }
        for e in &e {
            assert!(!semantics::segment_meets_box(
                e.a,
                e.b,
                b,
                e.width / 2.0 + 1.0
            ));
        }
        assert!(!s.contains("<path"));
        assert!(s.contains(">b</text>"));
        assert_eq!(semantic_report(&p).unwrap().angle_associations[0].arcs, 0);
    }

    #[test]
    fn free_text_crossing_a_ray_is_reported_without_moving_it() {
        let (p, s) = draw(
            serde_json::json!({"nodes":{"O":[180,230],"A":[140,30]},"segments":[{"from":"O","to":"A"}],"labels":[{"at":[172,195],"text":"b","size":14}]}),
        );
        let r = semantic_report(&p).unwrap();
        assert!(
            r.warnings
                .iter()
                .any(|w| w.contains("conservative text bounds overlapping edge"))
        );
        assert_eq!(def().svg(&p), s);
        assert_eq!(r.unbound_labels, ["b"]);
    }

    #[test]
    fn dimension_text_collision_is_role_neutral_and_preserves_source_placement() {
        let (p,s)=draw(serde_json::json!({"nodes":{"A":[30,80],"B":[250,80]},"segments":[{"from":"A","to":"B"}],"labels":[{"at":[135,80],"text":"12 cm"}]}));
        let r=semantic_report(&p).unwrap();
        assert_eq!(r.warnings.len(),1);
        assert!(r.warnings[0].contains("conservative text bounds"));
        assert!(!r.warnings[0].contains("angle") && !r.warnings[0].contains("arcs"));
        assert!(r.warnings[0].contains("may intentionally sit on an edge"));
        assert_eq!(def().svg(&p),s);
    }

    #[test]
    fn explicit_ellipse_ports_bind_rim_endpoints_without_solving_or_moving_legacy_nodes() {
        let (p,s)=draw(serde_json::json!({"nodes":{"C":[180,140],"independent":[245,140]},"ellipses":[{"id":"rim","center":"C","rx":90,"ry":30,"ports":{"tip":30}}],"segments":[{"from":"rim:center","to":"rim:right","style":"dashed"},{"from":"rim:center","to":"rim:tip"},{"from":"C","to":"independent"}]}));
        let g=parse(&p).unwrap();
        assert_eq!(g.node("rim:center").unwrap(),g.node("C").unwrap());
        assert_eq!(g.node("rim:right").unwrap(),(270.0,140.0));
        for name in ["rim:right","rim:top","rim:left","rim:bottom","rim:tip"] {
            let (x,y)=g.node(name).unwrap();
            assert!((((x-180.0)/90.0).powi(2)+((y-140.0)/30.0).powi(2)-1.0).abs()<1e-12);
        }
        assert_eq!(g.node("independent").unwrap(),(245.0,140.0));
        assert_eq!(s.matches("data-segment=").count(),3);
        assert!(!s.contains("<circle") && !s.contains("data-angle-vertex"));
        let r=semantic_report(&p).unwrap();
        assert_eq!(r.ellipse_attachments.len(),1);
        assert!(r.ellipse_attachments[0].ports.contains(&"rim:tip".into()));
        assert!(crate::figures::to_png(&s,1.0).is_ok());
    }

    #[test]
    fn ellipse_ports_follow_explicit_partial_arc_endpoints_and_add_no_drawn_geometry() {
        let old=serde_json::json!({"nodes":{"C":[180,140]},"ellipses":[{"center":"C","rx":90,"ry":30,"start":20,"end":180,"style":"dashed"}]});
        let (_,before)=draw(old.clone());
        let mut named=old;named["ellipses"][0]["id"]=serde_json::json!("rim");
        let (p,after)=draw(named);let g=parse(&p).unwrap();
        assert_eq!(before,after,"an id alone draws no additional geometry and preserves old SVG");
        let start=g.node("rim:start").unwrap();let end=g.node("rim:end").unwrap();
        assert!((start.0-(180.0+90.0*20f64.to_radians().cos())).abs()<1e-12);
        assert!((start.1-(140.0-30.0*20f64.to_radians().sin())).abs()<1e-12);
        assert!((end.0-90.0).abs()<1e-12 && (end.1-140.0).abs()<1e-12);
        assert!(semantic_report(&p).unwrap().edge_pairs.is_empty());
    }

    #[test]
    fn ellipse_ports_reject_collisions_unknown_names_and_recursive_centers() {
        for ellipse in [
            serde_json::json!({"id":"rim","center":"rim:center","rx":90,"ry":30}),
            serde_json::json!({"id":"rim","center":"C","rx":90,"ry":30,"ports":{"right":30}}),
            serde_json::json!({"center":"C","rx":90,"ry":30,"ports":{"tip":30}}),
        ] {
            let (p,_)=draw(serde_json::json!({"nodes":{"C":[180,140]},"ellipses":[ellipse]}));assert!(validate(&p).is_err());
        }
        for j in [
            serde_json::json!({"nodes":{"C":[180,140],"rim:right":[200,140]},"ellipses":[{"id":"rim","center":"C","rx":90,"ry":30}]}),
            serde_json::json!({"nodes":{"C":[180,140],"T":[200,140]},"lines":[{"id":"rim","through":"C","toward":"T"}],"ellipses":[{"id":"rim","center":"C","rx":90,"ry":30}]}),
            serde_json::json!({"nodes":{"C":[180,140]},"ellipses":[{"id":"rim","center":"C","rx":90,"ry":30}],"segments":[{"from":"C","to":"rim:missing"}]}),
        ] {let(p,_)=draw(j);assert!(validate(&p).is_err());}
    }

    #[test]
    fn marker_inventory_keeps_exact_declared_corners_and_legs_without_filling_marks() {
        let (p, s) = draw(
            serde_json::json!({"nodes":{"A":[50,50],"B":[160,50],"C":[200,150],"D":[100,220],"E":[30,150]},"segments":[{"from":"A","to":"B"},{"from":"B","to":"C"},{"from":"C","to":"D"},{"from":"D","to":"E"},{"from":"E","to":"A"}],"angles":[{"vertex":"A","from":"E","to":"B","arcs":1},{"vertex":"C","from":"B","to":"D","right":true,"color":"#ff0000"}]}),
        );
        let r = semantic_report(&p).unwrap();
        assert_eq!(r.angle_associations.len(), 2);
        assert_eq!(r.angle_associations[0].vertex, "A");
        assert_eq!(r.angle_associations[0].arcs, 1);
        assert_eq!(
            (&*r.angle_associations[1].from, &*r.angle_associations[1].to),
            ("B", "D")
        );
        assert!(r.angle_associations[1].right);
        assert_eq!(
            s.matches("data-angle-vertex=").count(),
            2,
            "no regularity inference adds the unrequested corners"
        );
    }

    #[test]
    fn malformed_constructive_lines_and_unplaceable_bound_labels_are_tool_errors() {
        for j in [
            serde_json::json!({"nodes":{"O":[50,50],"T":[100,50],"l:back":[0,0]},"lines":[{"id":"l","through":"O","toward":"T"}]}),
            serde_json::json!({"nodes":{"O":[50,50],"T":[100,50]},"lines":[{"id":"l","through":"l:back","toward":"T"}]}),
            serde_json::json!({"nodes":{"O":[50,50],"T":[100,50]},"lines":[{"id":"l","through":"O","toward":"O"}]}),
            serde_json::json!({"nodes":{"O":[50,50],"A":[70,10],"B":[80,10]},"angles":[{"vertex":"O","from":"A","to":"B","label":"very long label that does not fit","label_radius":1500}]}),
        ] {
            let (p, _) = draw(j);
            assert!(validate(&p).is_err());
        }
    }
}
#[cfg(test)]
mod hidden_edge_tests {
    fn hidden(diagram: serde_json::Value) -> Vec<String> {
        let d = crate::figures::registry().into_iter().find(|d| d.id == "geometry-diagram").unwrap();
        let p = crate::figures::params_from_json(&d, &serde_json::json!({"width":400,"height":400,"diagram":diagram.to_string()}));
        super::semantic_report(&p).unwrap().warnings.into_iter().filter(|w| w.starts_with("Hidden-edge end")).collect()
    }
    #[test]
    fn a_hidden_edge_stopping_in_empty_space_is_reported_and_closed_or_anchored_ones_are_not() {
        // Hidden back corner H joins three dashed edges: closed, nothing reported.
        let mut d = serde_json::json!({"nodes":{"A":[100,300],"B":[300,300],"C":[300,100],"H":[150,250],"G":[350,250],"K":[150,50]},
            "segments":[{"from":"A","to":"B"},{"from":"B","to":"C"},
                {"from":"A","to":"H","style":"dashed"},{"from":"H","to":"G","style":"dashed"},{"from":"H","to":"K","style":"dashed"},
                {"from":"G","to":"B"},{"from":"K","to":"C"}]});
        assert!(hidden(d.clone()).is_empty());
        // The hidden edge H->G stops at M, short of G: reported once, at M.
        d["nodes"]["M"] = serde_json::json!([250, 250]);
        d["segments"][3] = serde_json::json!({"from":"H","to":"M","style":"dashed"});
        let w = hidden(d);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("\"M\"") && w[0].contains("Nothing is joined automatically"));
    }
    #[test]
    fn a_few_solid_gray_edges_are_flagged_and_grid_lines_and_dashed_edges_are_not() {
        let d = crate::figures::registry().into_iter().find(|d| d.id == "geometry-diagram").unwrap();
        let warns = |diagram: serde_json::Value| -> Vec<String> {
            let p = crate::figures::params_from_json(&d, &serde_json::json!({"width":400,"height":400,"diagram":diagram.to_string()}));
            super::semantic_report(&p).unwrap().warnings.into_iter().filter(|w| w.contains("SOLID gray line")).collect()
        };
        let prism = |style: &str| serde_json::json!({"nodes":{"A":[100,300],"B":[300,300],"C":[200,100],"H":[180,260]},
            "segments":[{"from":"A","to":"B"},{"from":"B","to":"C"},{"from":"C","to":"A"},
                {"from":"A","to":"H","color":"#777777","style":style},{"from":"H","to":"C","color":"#888888","style":style}]});
        assert_eq!(warns(prism("solid")).len(), 2, "two solid gray back edges are flagged");
        assert!(warns(prism("dashed")).is_empty(), "dashed gray hidden edges are fine");
        // A grid of many gray lines is not a solid's hidden edges.
        let mut nodes = serde_json::Map::new();
        let mut segs = Vec::new();
        for i in 0..10 {
            nodes.insert(format!("a{i}"), serde_json::json!([20 + 30 * i, 20]));
            nodes.insert(format!("b{i}"), serde_json::json!([20 + 30 * i, 300]));
            segs.push(serde_json::json!({"from":format!("a{i}"),"to":format!("b{i}"),"color":"#777777"}));
        }
        assert!(warns(serde_json::json!({"nodes":nodes,"segments":segs})).is_empty());
        // Blue or red solid strokes are colour annotations, not gray edges.
        assert!(warns(serde_json::json!({"nodes":{"A":[10,10],"B":[90,90]},"segments":[{"from":"A","to":"B","color":"#0000ff"}]})).is_empty());
    }
    #[test]
    fn marks_radii_rays_and_t_junctions_are_not_hidden_edge_stubs() {
        let d = serde_json::json!({"nodes":{"O":[200,300],"R":[300,300],"T1":[50,50],"T2":[60,60],"P":[120,200],"Q":[120,120],"S":[200,100],"U":[200,200]},
            "ellipses":[{"center":"O","rx":100,"ry":30}],
            "segments":[
                {"from":"O","to":"R","style":"dashed","color":"#ff0000"},
                {"from":"T1","to":"T2","style":"dashed"},
                {"from":"P","to":"Q","style":"dashed","arrows":"end"},
                {"from":"S","to":"U"},{"from":"Q","to":"U","style":"dashed"}],
            "labels":[{"text":"5 in","at":[125,105]}]});
        assert!(hidden(d).is_empty(), "radius from the centre, an isolated mark, an arrowed stroke, a label and a T-junction are not stubs");
    }
}
