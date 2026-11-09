//! Direct-manipulation layer. A figure may expose `Handle`s (positions in SVG canvas units) that
//! the workspace draws over the preview; dragging/clicking/scrolling/double-clicking maps the
//! pointer back into `Params`. Dispatch is by figure id so `FigureDef` literals stay untouched.
//!
//! Contract for handle authors (see `handles_for`):
//! * positions use the SAME geometry as the renderer (reuse its layout helpers, or capture the
//!   scale/origin in the closure) and are recomputed from `Params` every frame;
//! * drag closures receive the pointer position in SVG units and write ONLY the params the
//!   handle drives; the workspace then snaps + clamps via `Params::normalize`, so closures
//!   need not clamp (but should avoid NaN);
//! * a handle must round-trip: dragging it to its own `pos` must not change the params
//!   (checked for every figure by `figures::interact::tests`).
use std::sync::Arc;

use super::Params;

pub type Pt = (f64, f64);
pub type DragFn = Arc<dyn Fn(&mut Params, Pt) + Send + Sync>;
pub type ClickFn = Arc<dyn Fn(&mut Params) + Send + Sync>;
pub type ScrollFn = Arc<dyn Fn(&mut Params, f64) + Send + Sync>;
/// A click that needs to know where it landed (in SVG units): an editor's "click the circle to add a point here".
pub type ClickAtFn = Arc<dyn Fn(&mut Params, Pt) + Send + Sync>;
/// What an area's click would act on under the pointer (SVG units), for the hover: None when nothing.
pub type HoverFn = Arc<dyn Fn(Pt) -> Option<Hover> + Send + Sync>;
/// Writes text typed in an inline edit box back into the params.
pub type SetFn = Arc<dyn Fn(&mut Params, &str) + Send + Sync>;
/// The text a double-click at a point (SVG units) edits in place: None where there is nothing to type.
pub type EditAtFn = Arc<dyn Fn(&Params, Pt) -> Option<EditAt> + Send + Sync>;

/// An inline edit a double-click opens on one part of an area (a Venn region's text): what the box starts with, its
/// placeholder, and how the typed text is written back.
#[derive(Clone)]
pub struct EditAt {
    pub text: String,
    pub hint: String,
    pub set: SetFn,
}

/// The part of an area a click there would act on: its outline (a line, an arc, an area's edge) and what the click does.
#[derive(Clone, Debug, PartialEq)]
pub struct Hover {
    pub outline: Vec<Pt>,
    pub closed: bool,
    pub tip: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Circle,
    Diamond,
    Square,
    /// Invisible hit-box that only outlines on hover (cells, labels, bonds).
    Zone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Grab,
    Move,
    ResizeH,
    ResizeV,
    Pointer,
    Crosshair,
}

#[derive(Clone)]
pub struct Handle {
    /// Centre in SVG units.
    pub pos: Pt,
    /// Half extent of the hit area in SVG units (min visual size is enforced on screen).
    pub half: Pt,
    pub shape: Shape,
    pub cursor: Cursor,
    pub tip: String,
    pub drag: Option<DragFn>,
    pub click: Option<ClickFn>,
    pub scroll: Option<ScrollFn>,
    /// Click that receives the pointer position (SVG units); for an area whose parts act differently
    /// (a ring that adds a point where it is clicked, a region that is shaded where it is clicked).
    pub click_at: Option<ClickAtFn>,
    /// With `click_at`: the part under the pointer, outlined on hover with its own tip (the area's `tip` is its summary).
    pub hover_at: Option<HoverFn>,
    /// Text param edited inline on double-click.
    pub edit: Option<&'static str>,
    /// With `click_at`: a double-click edits the text of the part under the pointer in place (the param it writes, for
    /// listings, and the edit there). The click the double-click began with is taken back first.
    pub edit_at: Option<(&'static str, EditAtFn)>,
    /// A movable figure label (`labels::handles`): hit only over its own text box and drawn on
    /// top of the other handles, so a drag that starts on the label moves the label.
    pub label_drag: bool,
}

impl Handle {
    fn base(pos: Pt, shape: Shape, cursor: Cursor, tip: &str) -> Self {
        Handle { pos, half: (7.0, 7.0), shape, cursor, tip: tip.to_string(), drag: None, click: None, scroll: None, click_at: None, hover_at: None, edit: None, edit_at: None, label_drag: false }
    }
    /// Round knob that drives params when dragged.
    pub fn knob(pos: Pt, tip: &str, f: impl Fn(&mut Params, Pt) + Send + Sync + 'static) -> Self {
        Handle { drag: Some(Arc::new(f)), ..Handle::base(pos, Shape::Circle, Cursor::Grab, tip) }
    }
    pub fn diamond(pos: Pt, tip: &str, f: impl Fn(&mut Params, Pt) + Send + Sync + 'static) -> Self {
        Handle { drag: Some(Arc::new(f)), ..Handle::base(pos, Shape::Diamond, Cursor::Grab, tip) }
    }
    pub fn square(pos: Pt, tip: &str, f: impl Fn(&mut Params, Pt) + Send + Sync + 'static) -> Self {
        Handle { drag: Some(Arc::new(f)), ..Handle::base(pos, Shape::Square, Cursor::Grab, tip) }
    }
    /// Bar-like handle constrained to one axis (resize bars, levels).
    pub fn bar(pos: Pt, vertical: bool, tip: &str, f: impl Fn(&mut Params, Pt) + Send + Sync + 'static) -> Self {
        let c = if vertical { Cursor::ResizeV } else { Cursor::ResizeH };
        Handle { drag: Some(Arc::new(f)), ..Handle::base(pos, Shape::Square, c, tip) }
    }
    /// Clickable rectangle (width/height in SVG units) such as a cell, bond or toggle.
    pub fn zone(centre: Pt, size: Pt, tip: &str, f: impl Fn(&mut Params) + Send + Sync + 'static) -> Self {
        Handle { half: (size.0 / 2.0, size.1 / 2.0), click: Some(Arc::new(f)), ..Handle::base(centre, Shape::Zone, Cursor::Pointer, tip) }
    }
    /// Clickable area (width/height in SVG units) whose click is told where it landed. Drawn under
    /// the knobs like any zone, so a knob inside it still wins its own clicks and drags.
    pub fn area(centre: Pt, size: Pt, tip: &str, f: impl Fn(&mut Params, Pt) + Send + Sync + 'static) -> Self {
        Handle { half: (size.0 / 2.0, size.1 / 2.0), click_at: Some(Arc::new(f)), ..Handle::base(centre, Shape::Zone, Cursor::Crosshair, tip) }
    }
    /// Double-click a label to edit text param `key` in place.
    pub fn label(centre: Pt, size: Pt, key: &'static str, tip: &str) -> Self {
        Handle { half: (size.0 / 2.0, size.1 / 2.0), edit: Some(key), ..Handle::base(centre, Shape::Zone, Cursor::Pointer, tip) }
    }
    pub fn with_hover(mut self, f: impl Fn(Pt) -> Option<Hover> + Send + Sync + 'static) -> Self {
        self.hover_at = Some(Arc::new(f));
        self
    }
    /// Double-click a part of this area to type its text in place (see `edit_at`); `key` is the param it writes.
    pub fn with_edit_at(mut self, key: &'static str, f: impl Fn(&Params, Pt) -> Option<EditAt> + Send + Sync + 'static) -> Self {
        self.edit_at = Some((key, Arc::new(f)));
        self
    }
    pub fn with_scroll(mut self, f: impl Fn(&mut Params, f64) + Send + Sync + 'static) -> Self {
        self.scroll = Some(Arc::new(f));
        self
    }
    pub fn with_click(mut self, f: impl Fn(&mut Params) + Send + Sync + 'static) -> Self {
        self.click = Some(Arc::new(f));
        self
    }
    pub fn with_cursor(mut self, c: Cursor) -> Self {
        self.cursor = c;
        self
    }
    pub fn with_half(mut self, hx: f64, hy: f64) -> Self {
        self.half = (hx, hy);
        self
    }
}

// ---------- geometry helpers for handle authors ----------

/// Angle in degrees (0 = +x, counter-clockwise, y-down screen aware) of `p` seen from `c`, 0..360.
pub fn angle_deg(c: Pt, p: Pt) -> f64 {
    let a = (-(p.1 - c.1)).atan2(p.0 - c.0).to_degrees();
    if a < 0.0 { a + 360.0 } else { a }
}

pub fn dist(a: Pt, b: Pt) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Point at `deg` (math convention) and `r` from `c`, in SVG coordinates.
pub fn polar(c: Pt, deg: f64, r: f64) -> Pt {
    (c.0 + r * deg.to_radians().cos(), c.1 - r * deg.to_radians().sin())
}

/// Cycle the choice string `key` to the next option of `opts`.
pub fn cycle(p: &mut Params, key: &str, opts: &[&str]) {
    let cur = p.s(key).to_string();
    let i = opts.iter().position(|o| *o == cur).map(|i| i + 1).unwrap_or(0) % opts.len().max(1);
    p.set_s(key, opts[i]);
}

/// Linear map of `v` from [a0,a1] to [b0,b1] (unclamped).
pub fn lerp_map(v: f64, a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    if (a1 - a0).abs() < 1e-12 { b0 } else { b0 + (v - a0) / (a1 - a0) * (b1 - b0) }
}

// ---------- hit testing ----------

/// The order the workspace hit-tests handles in: areas and zones first, then knobs, then movable labels; where two overlap, the
/// later one takes the pointer (the workspace registers them in this order, and egui gives the pointer to the last).
pub fn hit_order(hs: &[Handle]) -> Vec<usize> {
    let mut v: Vec<usize> = (0..hs.len()).collect();
    v.sort_by_key(|&i| (hs[i].label_drag, hs[i].shape != Shape::Zone));
    v
}

/// A handle's hit box half extent in SVG units at `k` screen px per SVG unit: its own, but never under 9 px on screen (5 for a
/// movable label, whose box is its text).
pub fn hit_half(h: &Handle, k: f64) -> Pt {
    let min = if h.label_drag { 5.0 } else { 9.0 };
    let k = if k > 0.0 { k } else { 1.0 };
    ((h.half.0 * k).max(min) / k, (h.half.1 * k).max(min) / k)
}

/// The handle a press at `q` (SVG units) goes to, as the workspace decides it.
pub fn pick(hs: &[Handle], q: Pt, k: f64) -> Option<usize> {
    hit_order(hs).into_iter().filter(|&i| { let (hx, hy) = hit_half(&hs[i], k); (q.0 - hs[i].pos.0).abs() <= hx && (q.1 - hs[i].pos.1).abs() <= hy }).last()
}

// ---------- dispatch ----------

/// Handles for figure `id` given its current params and canvas size (w, h) in SVG units.
/// Each subject module contributes via its own `handles(id, p, size) -> Option<Vec<Handle>>`.
pub fn handles_for(id: &str, p: &Params, size: Pt) -> Vec<Handle> {
    let mut hs = super::physics::handles(id, p, size)
        .or_else(|| super::biology::handles(id, p, size))
        .or_else(|| super::chemistry::handles(id, p, size))
        .or_else(|| super::math::handles(id, p, size))
        .unwrap_or_default();
    if super::labels::draggable(id)
        && let Some(def) = super::registry().into_iter().find(|d| d.id == id)
    {
        hs.extend(super::labels::handles(def.render, p));
    }
    hs
}

#[cfg(test)]
mod tests {
    use super::super::*;

    /// Parameter sets to probe: defaults, then each bool flipped and each choice option tried alone.
    fn variants(defs: &[ParamDef]) -> Vec<Params> {
        let base = Params::defaults(defs);
        let mut v = vec![base.clone()];
        for d in defs {
            match &d.kind {
                Kind::Bool => {
                    let mut p = base.clone();
                    p.set_b(d.key, !base.b(d.key));
                    v.push(p);
                }
                Kind::Choice(opts) => {
                    for o in opts {
                        let mut p = base.clone();
                        p.set_s(d.key, o);
                        v.push(p);
                    }
                }
                Kind::Int { min, max } if d.key == "n" => {
                    for n in *min..=*max {
                        let mut p = base.clone();
                        p.set_i(d.key, n);
                        v.push(p);
                    }
                }
                _ => {}
            }
        }
        v
    }

    fn in_range(defs: &[ParamDef], p: &Params) -> Result<(), String> {
        for d in defs {
            match (&d.kind, p.0.get(d.key)) {
                (Kind::Float { min, max, .. }, Some(Value::F(v))) if !(v.is_finite() && v >= min && v <= max) => {
                    return Err(format!("{} = {v} outside [{min},{max}]", d.key));
                }
                (Kind::Int { min, max }, Some(Value::I(v))) if v < min || v > max => {
                    return Err(format!("{} = {v} outside [{min},{max}]", d.key));
                }
                (Kind::Choice(o), Some(Value::S(v))) if !o.contains(&v.as_str()) => {
                    return Err(format!("{} = {v:?} not an option", d.key));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn close(defs: &[ParamDef], a: &Params, b: &Params) -> Result<(), String> {
        for d in defs {
            let (x, y) = (a.0.get(d.key), b.0.get(d.key));
            let ok = match (&d.kind, x, y) {
                (Kind::Float { min, max, step }, Some(Value::F(x)), Some(Value::F(y))) => {
                    let tol = if *step > 0.0 { *step * 1.01 } else { (max - min) * 0.01 }.max(1e-6);
                    (x - y).abs() <= tol
                }
                (_, x, y) => x == y,
            };
            if !ok {
                return Err(format!("{}: {:?} -> {:?}", d.key, x, y));
            }
        }
        Ok(())
    }

    #[test]
    fn figure_ids_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for d in registry() {
            assert!(seen.insert(d.id), "duplicate figure id {}", d.id);
        }
    }

    #[test]
    fn every_figure_defaults_render_nonempty_svg() {
        for d in registry() {
            let p = Params::defaults(&(d.params)());
            let svg = (d.render)(&p);
            assert!(svg.len() > 300 && svg.contains("<svg"), "{}: svg too small", d.id);
            let (w, h) = svg_size(&svg);
            assert!(w > 50.0 && h > 50.0, "{}: bad canvas {w}x{h}", d.id);
            let png = to_png(&svg, 0.5).unwrap_or_else(|e| panic!("{}: {e}", d.id));
            assert!(png.len() > 500, "{}: empty raster", d.id);
        }
    }

    #[test]
    fn params_clamp_and_snap() {
        for d in registry() {
            let defs = (d.params)();
            let mut p = Params::defaults(&defs);
            for pd in &defs {
                match &pd.kind {
                    Kind::Float { .. } => p.set_f(pd.key, f64::NAN),
                    Kind::Int { .. } => p.set_i(pd.key, i64::MAX),
                    Kind::Choice(_) => p.set_s(pd.key, "<<bogus>>"),
                    _ => {}
                }
            }
            p.normalize(&defs, true);
            in_range(&defs, &p).unwrap_or_else(|e| panic!("{}: {e}", d.id));
            for pd in &defs {
                if let Kind::Float { max, .. } = &pd.kind {
                    p.set_f(pd.key, f64::INFINITY);
                    p.normalize(&defs, false);
                    assert_eq!(p.f(pd.key), *max, "{}.{}", d.id, pd.key);
                }
            }
            // After clamping, the figure must still render.
            to_png(&(d.render)(&p), 0.3).unwrap_or_else(|e| panic!("{}: clamped render: {e}", d.id));
        }
    }

    #[test]
    fn every_handle_round_trips_and_stays_in_range() {
        let mut total = 0;
        for d in registry() {
            let defs = (d.params)();
            for p in variants(&defs) {
                let size = svg_size(&(d.render)(&p));
                let hs = d.handles(&p, size);
                for (i, h) in hs.iter().enumerate() {
                    assert!(h.pos.0.is_finite() && h.pos.1.is_finite(), "{} handle {i} ({}) non-finite", d.id, h.tip);
                    if let Some(f) = &h.drag {
                        total += 1;
                        // 1. dragging a handle onto its own position changes nothing.
                        let mut q = p.clone();
                        f(&mut q, h.pos);
                        q.normalize(&defs, true);
                        let mut want = p.clone();
                        want.normalize(&defs, true);
                        close(&defs, &want, &q).unwrap_or_else(|e| panic!("{} handle {i} ({}) round trip: {e}", d.id, h.tip));
                        // 2. wild targets stay in range and the figure still renders.
                        for t in [(0.0, 0.0), (size.0, size.1), (-5000.0, 9000.0), (size.0 / 2.0, -300.0), (1e9, -1e9)] {
                            let mut q = p.clone();
                            f(&mut q, t);
                            q.normalize(&defs, true);
                            in_range(&defs, &q).unwrap_or_else(|e| panic!("{} handle {i} ({}) -> {t:?}: {e}", d.id, h.tip));
                            let svg = (d.render)(&q);
                            assert!(svg.contains("<svg"), "{} handle {i}", d.id);
                        }
                    }
                    if let Some(f) = &h.click {
                        let mut q = p.clone();
                        f(&mut q);
                        q.normalize(&defs, true);
                        in_range(&defs, &q).unwrap_or_else(|e| panic!("{} click {i} ({}): {e}", d.id, h.tip));
                        assert!(!(d.render)(&q).is_empty());
                    }
                    if let Some(f) = &h.hover_at {
                        let (c, hh) = (h.pos, h.half);
                        for t in [c, (c.0 - hh.0, c.1 - hh.1), (c.0 + hh.0 * 0.3, c.1 - hh.1 * 0.7), (f64::NAN, 3.0)] {
                            if let Some(hv) = f(t) { assert!(hv.outline.iter().all(|q| q.0.is_finite() && q.1.is_finite()), "{} hover {i}", d.id); }
                        }
                    }
                    if let Some(f) = &h.click_at {
                        // at its centre, at its corners and far outside: in range, and the figure still renders
                        let (c, hh) = (h.pos, h.half);
                        for t in [c, (c.0 - hh.0, c.1 - hh.1), (c.0 + hh.0, c.1 + hh.1), (c.0 + hh.0 * 0.5, c.1), (-5000.0, 9000.0), (f64::NAN, 1e9)] {
                            let mut q = p.clone();
                            f(&mut q, t);
                            q.normalize(&defs, true);
                            in_range(&defs, &q).unwrap_or_else(|e| panic!("{} click-at {i} ({}) -> {t:?}: {e}", d.id, h.tip));
                            assert!((d.render)(&q).contains("<svg"), "{} click-at {i}", d.id);
                        }
                    }
                    if let Some(f) = &h.scroll {
                        for dy in [-3.0, 3.0] {
                            let mut q = p.clone();
                            f(&mut q, dy);
                            q.normalize(&defs, true);
                            in_range(&defs, &q).unwrap_or_else(|e| panic!("{} scroll {i} ({}): {e}", d.id, h.tip));
                        }
                    }
                    if let Some(k) = h.edit {
                        assert!(defs.iter().any(|x| x.key == k), "{}: edit key {k} unknown", d.id);
                    }
                    if let Some((k, f)) = &h.edit_at {
                        assert!(defs.iter().any(|x| x.key == *k), "{}: edit-at key {k} unknown", d.id);
                        // typing at its centre and corners keeps the params in range and the figure drawing
                        let (c, hh) = (h.pos, h.half);
                        for t in [c, (c.0 - hh.0, c.1 - hh.1), (c.0 + hh.0, c.1 + hh.1), (f64::NAN, 1e9)] {
                            if let Some(e) = f(&p, t) {
                                for typed in ["", "x, 2x + 1", &e.text, "a, b; c,,"] {
                                    let mut q = p.clone();
                                    (e.set)(&mut q, typed);
                                    q.normalize(&defs, true);
                                    in_range(&defs, &q).unwrap_or_else(|er| panic!("{} edit-at {i} -> {typed:?}: {er}", d.id));
                                    assert!((d.render)(&q).contains("<svg"), "{} edit-at {i}", d.id);
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("drag handles probed: {total}");
    }

    /// Figures with nothing sensible to drag (sorted). A new figure must expose handles or be added
    /// here deliberately; a figure that gains handles must be removed (the test checks both ways).
    const NO_HANDLES: &[&str] = &[
        "accounting-equation", "action-potential", "ad-as", "bank-reconciliation", "bar-chart", "base-ten-blocks", "block-diagram", "body-map", "boltzmann-distribution", "boolean-table",
        "brain-regions", "break-even", "cash-flow-statement", "cause-effect", "cell-cycle", "cell-diagram", "chi-square-fit", "chi-square-table",
        "circle-of-fifths", "circular-flow", "circular-motion", "cladogram", "climate-graph", "collision-momentum", "colour-wheel", "confidence-interval",
        "cost-curves", "court-layout", "data-structure", "depreciation-schedule", "discrete-distribution", "dna-protein-synthesis",
        "dot-and-cross", "dot-plot", "earth-cycles", "earth-layers", "elasticity", "electric-field", "energy-bar-chart", "energy-pyramid", "er-diagram", "experiment-flow",
        "family-tree", "financial-statement", "flowchart", "food-plate", "food-web", "fraction-models", "free-body", "frequency-table", "gear-train",
        "graph-diagram", "grid-reference", "guitar-chords", "heart-zones", "histogram", "hr-diagram",
        "journal-entry", "kirchhoff-loops", "leaf-cross-section", "loan-amortisation", "logic-circuit", "lorenz", "magnetic-field", "map-scale",
        "marketing-funnel", "maslow", "monopoly", "moon-seasons", "motion-map", "music-staff", "network-topology",
        "normal-curve", "nuclide-notation", "number-base", "ogive", "org-chart", "organic-structure", "organiser", "orthographic", "osmosis", "periodic-table", "perspective-grid",
        "photosynthesis", "piano-keyboard", "pie-chart", "plate-boundaries", "plot-arc", "population-pyramid", "ppf", "probability-tree", "protractor", "pv-diagram",
        "rc-curve", "recursion-tree", "refraction", "rejection-region", "residual-plot", "rhythm-counting",
        "sampling-distribution", "scatterplot", "sentence-diagram", "sine-cosine", "slope-triangle", "soil-texture", "solid-net", "sorting-passes", "standing-wave", "state-machine",
        "stem-and-leaf", "strata", "structure-chart", "supply-demand", "swot", "t-account", "time-value",
        "timeline", "topo-contours", "trace-table", "tree-diagram", "trial-balance", "triangle-centres",
        "vsepr-shapes", "xylem-phloem",
    ];

    #[test]
    fn handle_coverage_report() {
        let mut missing = Vec::new();
        for d in registry() {
            let p = Params::defaults(&(d.params)());
            let size = svg_size(&(d.render)(&p));
            let n = d.handles(&p, size).len();
            println!("HANDLES {} {}", d.id, n);
            if n == 0 {
                missing.push(d.id);
            }
        }
        missing.sort();
        println!("without handles: {missing:?}");
        let mut listed = NO_HANDLES.to_vec();
        listed.sort();
        assert_eq!(NO_HANDLES, &listed[..], "NO_HANDLES must be sorted");
        let new: Vec<&&str> = missing.iter().filter(|m| !NO_HANDLES.contains(m)).collect();
        let stale: Vec<&&str> = NO_HANDLES.iter().filter(|m| !missing.contains(m)).collect();
        assert!(new.is_empty(), "figures without drag handles not in NO_HANDLES: {new:?}");
        assert!(stale.is_empty(), "NO_HANDLES lists figures that now have handles (remove them): {stale:?}");
    }
}
