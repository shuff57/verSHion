//! Tiny SVG builder shared by all generators.
use std::fmt::Write;

pub struct Doc { w: f64, h: f64, body: String }

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

impl Doc {
    pub fn new(w: f64, h: f64) -> Self { Doc { w, h, body: String::new() } }
    pub fn raw(&mut self, s: &str) -> &mut Self { self.body.push_str(s); self }
    /// Shared `<defs>` (the print patterns of [`Fills`]); drawn before everything that uses them.
    pub fn defs(&mut self, s: &str) -> &mut Self { if !s.is_empty() { let _ = write!(self.body, "<defs>{s}</defs>"); } self }
    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, stroke: &str, sw: f64) -> &mut Self {
        let _ = write!(self.body, r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{stroke}" stroke-width="{sw}" stroke-linecap="round"/>"#);
        self
    }
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, fill: &str, stroke: &str, sw: f64) -> &mut Self {
        let _ = write!(self.body, r#"<rect x="{x:.2}" y="{y:.2}" width="{w:.2}" height="{h:.2}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>"#);
        self
    }
    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, fill: &str, stroke: &str, sw: f64) -> &mut Self {
        let _ = write!(self.body, r#"<circle cx="{cx:.2}" cy="{cy:.2}" r="{r:.2}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>"#);
        self
    }
    pub fn poly(&mut self, pts: &[(f64, f64)], fill: &str, stroke: &str, sw: f64) -> &mut Self {
        let p: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
        let _ = write!(self.body, r#"<polygon points="{}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round"/>"#, p.join(" "));
        self
    }
    pub fn polyline(&mut self, pts: &[(f64, f64)], stroke: &str, sw: f64) -> &mut Self {
        let p: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
        let _ = write!(self.body, r#"<polyline points="{}" fill="none" stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round" stroke-linecap="round"/>"#, p.join(" "));
        self
    }
    pub fn path(&mut self, d: &str, fill: &str, stroke: &str, sw: f64) -> &mut Self {
        let _ = write!(self.body, r#"<path d="{d}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round" stroke-linecap="round"/>"#);
        self
    }
    /// anchor: "start" | "middle" | "end"
    pub fn text(&mut self, x: f64, y: f64, size: f64, anchor: &str, s: &str) -> &mut Self {
        let _ = write!(self.body, r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="#000">{}</text>"##, esc(s));
        self
    }
    /// Arrow from (x1,y1) to (x2,y2) with a filled head.
    pub fn arrow(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, color: &str, sw: f64) -> &mut Self {
        let (dx, dy) = (x2 - x1, y2 - y1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (ux, uy) = (dx / len, dy / len);
        let hl = (sw * 4.0).max(9.0);
        let (bx, by) = (x2 - ux * hl, y2 - uy * hl);
        self.line(x1, y1, bx, by, color, sw);
        let pts = [(x2, y2), (bx - uy * hl * 0.4, by + ux * hl * 0.4), (bx + uy * hl * 0.4, by - ux * hl * 0.4)];
        self.poly(&pts, color, color, 0.5)
    }
    /// Arrow like [`Doc::arrow`] with options: a dash array for the shaft (empty = solid) and a
    /// second head at the start (`both`). With `""` and `false` the output equals `arrow`.
    /// Draws nothing for a non-finite coordinate.
    pub fn arrow_with(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, color: &str, sw: f64, dash: &str, both: bool) -> &mut Self {
        if ![x1, y1, x2, y2, sw].iter().all(|v| v.is_finite()) { return self; }
        if dash.is_empty() && !both { return self.arrow(x1, y1, x2, y2, color, sw); }
        let (dx, dy) = (x2 - x1, y2 - y1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (ux, uy) = (dx / len, dy / len);
        let hl = (sw * 4.0).max(9.0);
        let (bx, by) = (x2 - ux * hl, y2 - uy * hl);
        let (ax, ay) = if both { (x1 + ux * hl, y1 + uy * hl) } else { (x1, y1) };
        if dash.is_empty() {
            self.line(ax, ay, bx, by, color, sw);
        } else {
            let _ = write!(self.body, r#"<line x1="{ax:.2}" y1="{ay:.2}" x2="{bx:.2}" y2="{by:.2}" stroke="{color}" stroke-width="{sw}" stroke-dasharray="{dash}" stroke-linecap="round"/>"#);
        }
        self.poly(&[(x2, y2), (bx - uy * hl * 0.4, by + ux * hl * 0.4), (bx + uy * hl * 0.4, by - ux * hl * 0.4)], color, color, 0.5);
        if both {
            self.poly(&[(x1, y1), (ax + uy * hl * 0.4, ay - ux * hl * 0.4), (ax - uy * hl * 0.4, ay + ux * hl * 0.4)], color, color, 0.5);
        }
        self
    }
    /// The minor arc of radius `r` about `v` between the unit directions `u` and `w` (screen
    /// coordinates), stroked with no fill. Nothing for a bad radius, a non-finite or zero
    /// direction, or equal directions.
    pub fn arc_dirs(&mut self, v: (f64, f64), u: (f64, f64), w: (f64, f64), r: f64, stroke: &str, sw: f64) -> &mut Self {
        let ok = [v.0, v.1, u.0, u.1, w.0, w.1, r].iter().all(|x| x.is_finite()) && r > 0.0;
        let cross = u.0 * w.1 - u.1 * w.0;
        if !ok || (u.0 == w.0 && u.1 == w.1) || (cross == 0.0 && u.0 * w.0 + u.1 * w.1 > 0.0) { return self; }
        let p1 = (v.0 + u.0 * r, v.1 + u.1 * r);
        let p2 = (v.0 + w.0 * r, v.1 + w.1 * r);
        let sweep = if cross > 0.0 { 1 } else { 0 };
        self.path(&format!("M{:.2},{:.2} A{r:.2},{r:.2} 0 0 {sweep} {:.2},{:.2}", p1.0, p1.1, p2.0, p2.1), "none", stroke, sw)
    }
    /// Stroked arc about (`cx`,`cy`) of radius `r` from `a0` to `a1` degrees (y-up, counter-clockwise
    /// on screen when `a1 > a0`, clockwise otherwise). Spans of 360 or more are clamped to a
    /// nearly full turn. Nothing for equal angles, a non-positive radius or non-finite input.
    pub fn arc(&mut self, cx: f64, cy: f64, r: f64, a0: f64, a1: f64, stroke: &str, sw: f64) -> &mut Self {
        match arc_geom(cx, cy, r, a0, a1) {
            Some((p0, p1, large, sweep)) => self.path(&format!("M{:.2},{:.2} A{r:.2},{r:.2} 0 {large} {sweep} {:.2},{:.2}", p0.0, p0.1, p1.0, p1.1), "none", stroke, sw),
            None => self,
        }
    }
    /// Filled wedge (pie slice) about (`cx`,`cy`): centre, out to `a0`, round to `a1`, back.
    /// Angles as in [`Doc::arc`]. Nothing for degenerate input.
    pub fn sector(&mut self, cx: f64, cy: f64, r: f64, a0: f64, a1: f64, fill: &str, stroke: &str, sw: f64) -> &mut Self {
        match arc_geom(cx, cy, r, a0, a1) {
            Some((p0, p1, large, sweep)) => self.path(&format!("M{cx:.2},{cy:.2} L{:.2},{:.2} A{r:.2},{r:.2} 0 {large} {sweep} {:.2},{:.2}Z", p0.0, p0.1, p1.0, p1.1), fill, stroke, sw),
            None => self,
        }
    }
    /// Angle mark at vertex `v` between rays at `a0` and `a1` degrees (y-up): an arc of radius
    /// `r`, `ticks` extra concentric arcs (0..=3, 4 px apart) and an optional label on the
    /// bisector beyond the arcs. Nothing for degenerate input.
    pub fn angle_mark(&mut self, v: (f64, f64), a0: f64, a1: f64, r: f64, label: Option<&str>, ticks: u8, stroke: &str, sw: f64) -> &mut Self {
        if arc_geom(v.0, v.1, r, a0, a1).is_none() { return self; }
        let n = ticks.min(3) as usize;
        for k in 0..=n { self.arc(v.0, v.1, r + 4.0 * k as f64, a0, a1, stroke, sw); }
        if let Some(t) = label.filter(|t| !t.is_empty()) {
            let mid = ((a0 + a1) / 2.0).to_radians();
            let lr = r + 4.0 * n as f64 + 10.0;
            self.text(v.0 + lr * mid.cos(), v.1 - lr * mid.sin() + 4.0, 12.0, "middle", t);
        }
        self
    }
    /// Square corner mark at `v` with arms of `size` px along the unit directions `u` and `w`.
    /// Nothing for non-finite input or a non-positive size.
    pub fn right_angle(&mut self, v: (f64, f64), u: (f64, f64), w: (f64, f64), size: f64, stroke: &str, sw: f64) -> &mut Self {
        if ![v.0, v.1, u.0, u.1, w.0, w.1, size].iter().all(|x| x.is_finite()) || size <= 0.0 { return self; }
        let p1 = (v.0 + u.0 * size, v.1 + u.1 * size);
        let p3 = (v.0 + w.0 * size, v.1 + w.1 * size);
        let p2 = (p1.0 + w.0 * size, p1.1 + w.1 * size);
        self.polyline(&[p1, p2, p3], stroke, sw)
    }
    /// Dimension line from (x1,y1) to (x2,y2): the line, a perpendicular tick (4 px each side)
    /// at both ends and `label` centred above its middle (size `size`). Nothing for non-finite
    /// input or equal end points.
    pub fn dim_line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, label: &str, size: f64, stroke: &str, sw: f64) -> &mut Self {
        if ![x1, y1, x2, y2, size, sw].iter().all(|v| v.is_finite()) { return self; }
        let (dx, dy) = (x2 - x1, y2 - y1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-9 { return self; }
        let (nx, ny) = (-dy / len, dx / len);
        self.line(x1, y1, x2, y2, stroke, sw);
        for (x, y) in [(x1, y1), (x2, y2)] { self.line(x - nx * 4.0, y - ny * 4.0, x + nx * 4.0, y + ny * 4.0, stroke, sw); }
        if !label.is_empty() {
            let s = if ny <= 0.0 { 1.0 } else { -1.0 };
            let off = size * 0.6 + 4.0;
            self.text((x1 + x2) / 2.0 + nx * s * off, (y1 + y2) / 2.0 + ny * s * off + size * 0.35, size, "middle", label);
        }
        self
    }
    pub fn finish(&self) -> String {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="100%" height="100%" fill="#fff"/>{b}</svg>"##,
            w = self.w, h = self.h, b = self.body
        )
    }
}

/// Endpoints and flags of the arc about (`cx`,`cy`) from `a0` to `a1` degrees (y-up), as
/// (start, end, large-arc flag, sweep flag); None for non-finite input, `r <= 0` or equal angles.
fn arc_geom(cx: f64, cy: f64, r: f64, a0: f64, a1: f64) -> Option<((f64, f64), (f64, f64), u8, u8)> {
    if ![cx, cy, r, a0, a1].iter().all(|v| v.is_finite()) || r <= 0.0 { return None; }
    let mut span = a1 - a0;
    if span == 0.0 { return None; }
    if span.abs() >= 360.0 { span = 359.99 * span.signum(); }
    let pt = |a: f64| (cx + r * a.to_radians().cos(), cy - r * a.to_radians().sin());
    Some((pt(a0), pt(a0 + span), (span.abs() > 180.0) as u8, (span < 0.0) as u8))
}

/// The reference's L/M/S label-size toggle: text size factor for a `label_size` choice
/// (Large is 1.4x and Small 0.8x Medium, as on the reference generators).
pub fn label_factor(choice: &str) -> f64 {
    match choice {
        "Large" => 1.4,
        "Small" => 0.8,
        _ => 1.0,
    }
}

/// The `label_size` param shared by the figures that get the toggle by scaling their output.
pub fn label_size_param() -> super::ParamDef {
    super::ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium")
}

/// Applies the `label_size` param to a finished SVG (`scale_text`); Medium returns it unchanged.
pub fn with_label_size(svg: String, p: &super::Params) -> String {
    let f = label_factor(p.s("label_size"));
    if f == 1.0 { svg } else { scale_text(&svg, f) }
}

/// Multiplies every numeric `font-size="…"` and `dy="…"` (superscript offsets, which generators
/// work out from the font size) by `f`, for generators that draw text at fixed sizes. Values in
/// other units (`0.35em`) are left alone, so they keep scaling with their font.
pub fn scale_text(svg: &str, f: f64) -> String {
    let mut out = String::with_capacity(svg.len() + 64);
    let mut rest = svg;
    loop {
        let next = ["font-size=\"", " dy=\""].iter().filter_map(|pat| rest.find(pat).map(|i| (i, *pat))).min_by_key(|(i, _)| *i);
        let Some((i, pat)) = next else {
            out.push_str(rest);
            break;
        };
        let start = i + pat.len();
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest.find('"').unwrap_or(rest.len());
        let val = &rest[..end];
        let num = val.trim_end_matches("px");
        match num.parse::<f64>() {
            Ok(v) => {
                let n = ((v * f) * 100.0).round() / 100.0;
                out.push_str(&format!("{n}"));
                if num.len() != val.len() {
                    out.push_str("px");
                }
            }
            Err(_) => out.push_str(val),
        }
        rest = &rest[end..];
    }
    out
}

/// The `style` param of the figures that draw filled series (bars, regions, categories): Colour
/// for the screen, Print for black-and-white exams (hatch patterns instead of colour fills).
pub fn style_param() -> super::ParamDef {
    super::ParamDef::choice("style", "Style", &["Colour", "Print"], "Colour").hint("Print swaps colour fills for hatching that survives a black-and-white copy")
}

const SERIES_COLOURS: [&str; 6] = ["#4f7cd4", "#e0803c", "#4aa36b", "#c9494f", "#8a62c4", "#d6b23a"];

/// Fills for series 0, 1, 2 ... in the figure's chosen style. Call [`Fills::defs`] once (through
/// [`Doc::defs`]) before using [`Fills::series`] in Print style.
#[derive(Clone, Copy, Debug)]
pub struct Fills { print: bool }

impl Fills {
    pub fn new(p: &super::Params) -> Self { Fills { print: p.s("style") == "Print" } }
    pub fn is_print(&self) -> bool { self.print }
    /// The `fill` for series `i` (wraps after six): a colour, or a pattern reference in Print.
    pub fn series(&self, i: usize) -> String {
        if self.print { format!("url(#vs-hatch{})", i % 6) } else { SERIES_COLOURS[i % 6].to_string() }
    }
    /// The same series as a stroke colour for lines: black in Print (the dash style tells them
    /// apart, see [`Fills::dash`]), the series colour otherwise.
    pub fn line(&self, i: usize) -> &'static str {
        if self.print { "#000" } else { SERIES_COLOURS[i % 6] }
    }
    /// `stroke-dasharray` value that tells line series apart in Print; empty in Colour.
    pub fn dash(&self, i: usize) -> &'static str {
        if !self.print { return ""; }
        ["", "6 3", "2 3", "8 3 2 3", "1 4", "10 4"][i % 6]
    }
    /// The pattern definitions for Print style; empty in Colour.
    pub fn defs(&self) -> String {
        if !self.print { return String::new(); }
        let line = |a: &str| format!(r##"<path d="{a}" stroke="#000" stroke-width="1" fill="none"/>"##);
        let pats: [(&str, String); 6] = [
            ("<rect width=\"8\" height=\"8\" fill=\"#d9d9d9\"/>", String::new()),
            ("", line("M-2,2 l4,-4 M0,8 l8,-8 M6,10 l4,-4")),
            ("", format!("{}{}", line("M-2,2 l4,-4 M0,8 l8,-8 M6,10 l4,-4"), line("M-2,6 l4,4 M0,0 l8,8 M6,-2 l4,4"))),
            ("", r##"<circle cx="2" cy="2" r="1.1" fill="#000"/><circle cx="6" cy="6" r="1.1" fill="#000"/>"##.to_string()),
            ("", line("M0,2 h8 M0,6 h8")),
            ("", line("M2,0 v8 M6,0 v8")),
        ];
        let mut out = String::new();
        for (i, (bg, fg)) in pats.iter().enumerate() {
            out.push_str(&format!(r##"<pattern id="vs-hatch{i}" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="#fff"/>{bg}{fg}</pattern>"##));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_text_scales_sizes_and_offsets_only() {
        let svg = r#"<text font-size="14" x="3"><tspan font-size="10.0" dy="-5.0">2</tspan><tspan dy="0.35em">x</tspan></text><text font-size="20px">y</text><line stroke-width="2"/>"#;
        let big = scale_text(svg, 1.4);
        assert!(big.contains(r#"font-size="19.6""#), "{big}");
        assert!(big.contains(r#"font-size="14" dy="-7""#), "{big}");
        assert!(big.contains(r#"dy="-7""#), "{big}");
        assert!(big.contains(r#"dy="0.35em""#), "{big}");
        assert!(big.contains(r#"font-size="28px""#), "{big}");
        assert!(big.contains(r#"x="3""#) && big.contains(r#"stroke-width="2""#), "{big}");
        assert_eq!(scale_text(svg, 1.0).replace(".0\"", "\""), svg.replace(".0\"", "\""));
    }

    #[test]
    fn label_factors_match_reference() {
        assert_eq!(label_factor("Large"), 1.4);
        assert_eq!(label_factor("Medium"), 1.0);
        assert_eq!(label_factor("Small"), 0.8);
    }
    #[test]
    fn fills_are_colour_or_pattern_by_style() {
        let defs = [style_param()];
        let mut p = super::super::Params::defaults(&defs);
        let c = Fills::new(&p);
        assert!(!c.is_print() && c.series(1).starts_with('#') && c.defs().is_empty() && c.dash(2).is_empty());
        p.set_s("style", "Print");
        let pr = Fills::new(&p);
        assert!(pr.is_print() && pr.series(7) == "url(#vs-hatch1)" && pr.line(3) == "#000");
        assert_eq!(pr.defs().matches("<pattern").count(), 6);
        let dashes: std::collections::BTreeSet<_> = (0..6).map(|i| pr.dash(i)).collect();
        assert_eq!(dashes.len(), 6, "every line series has its own dash");
    }

    #[test]
    fn print_style_renders_black_and_white_only() {
        let mut p = super::super::Params::defaults(&[style_param()]);
        p.set_s("style", "Print");
        let f = Fills::new(&p);
        let mut d = Doc::new(60.0, 20.0);
        d.defs(&f.defs());
        for i in 0..6 { d.rect(i as f64 * 10.0, 0.0, 10.0, 20.0, &f.series(i), "#000", 1.0); }
        let png = super::super::to_png(&d.finish(), 1.0).expect("patterns rasterize");
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let mut dark = 0;
        for px in img.pixels() {
            assert!(px[0] == px[1] && px[1] == px[2], "grey only, got {px:?}");
            if px[0] < 128 { dark += 1; }
        }
        assert!(dark > 50, "hatching actually drew ({dark} dark pixels)");
    }
}

#[cfg(test)]
mod geometry_tests {
    use super::*;
    fn body(d: &Doc) -> String { d.finish() }

    #[test]
    fn arc_endpoints_and_flags() {
        let mut d = Doc::new(100.0, 100.0);
        d.arc(50.0, 50.0, 10.0, 0.0, 90.0, "#000", 1.0);
        assert!(body(&d).contains(r#"d="M60.00,50.00 A10.00,10.00 0 0 0 50.00,40.00""#), "{}", body(&d));
        let mut d = Doc::new(100.0, 100.0);
        d.arc(50.0, 50.0, 10.0, 0.0, 270.0, "#000", 1.0);
        assert!(body(&d).contains(" 0 1 0 50.00,60.00"));
        let mut d = Doc::new(100.0, 100.0);
        d.arc(50.0, 50.0, 10.0, 90.0, 0.0, "#000", 1.0);
        assert!(body(&d).contains("M50.00,40.00 A10.00,10.00 0 0 1 60.00,50.00"));
    }

    #[test]
    fn arc_dirs_matches_the_cross_sign_rule() {
        let mut d = Doc::new(10.0, 10.0);
        d.arc_dirs((0.0, 0.0), (1.0, 0.0), (0.0, 1.0), 5.0, "#000", 1.0);
        assert!(body(&d).contains("M5.00,0.00 A5.00,5.00 0 0 1 0.00,5.00"));
    }

    #[test]
    fn sector_is_closed_with_centre() {
        let mut d = Doc::new(100.0, 100.0);
        d.sector(50.0, 50.0, 10.0, 0.0, 90.0, "#ddd", "#000", 1.0);
        assert!(body(&d).contains("M50.00,50.00 L60.00,50.00 A10.00,10.00 0 0 0 50.00,40.00Z"));
    }

    #[test]
    fn right_angle_corner() {
        let mut d = Doc::new(100.0, 100.0);
        d.right_angle((10.0, 10.0), (1.0, 0.0), (0.0, 1.0), 5.0, "#000", 1.0);
        assert!(body(&d).contains(r#"points="15.00,10.00 15.00,15.00 10.00,15.00""#));
    }

    #[test]
    fn angle_mark_ticks_and_label() {
        let mut d = Doc::new(100.0, 100.0);
        d.angle_mark((50.0, 50.0), 0.0, 60.0, 20.0, Some("60"), 2, "#000", 1.0);
        let s = body(&d);
        assert_eq!(s.matches("<path").count(), 3);
        assert!(s.contains(">60</text>"));
    }

    #[test]
    fn arrow_variants() {
        let mut a = Doc::new(10.0, 10.0);
        a.arrow(0.0, 0.0, 50.0, 0.0, "#000", 2.0);
        let mut b = Doc::new(10.0, 10.0);
        b.arrow_with(0.0, 0.0, 50.0, 0.0, "#000", 2.0, "", false);
        assert_eq!(a.finish(), b.finish());
        let mut c = Doc::new(10.0, 10.0);
        c.arrow_with(0.0, 0.0, 50.0, 0.0, "#000", 2.0, "4 3", true);
        let s = c.finish();
        assert!(s.contains("stroke-dasharray=\"4 3\"") && s.matches("<polygon").count() == 2);
    }

    #[test]
    fn dim_line_has_ticks_and_label() {
        let mut d = Doc::new(100.0, 100.0);
        d.dim_line(10.0, 50.0, 90.0, 50.0, "8 cm", 12.0, "#000", 1.0);
        let s = body(&d);
        assert_eq!(s.matches("<line").count(), 3);
        assert!(s.contains(">8 cm</text>") && s.contains(r#"x="50.00""#));
    }

    #[test]
    fn degenerate_input_draws_nothing() {
        let empty = Doc::new(10.0, 10.0).finish();
        let mut d = Doc::new(10.0, 10.0);
        for r in [0.0, -3.0, f64::NAN, f64::INFINITY] {
            d.arc(5.0, 5.0, r, 0.0, 90.0, "#000", 1.0);
            d.sector(5.0, 5.0, r, 0.0, 90.0, "#ccc", "#000", 1.0);
            d.angle_mark((5.0, 5.0), 0.0, 90.0, r, Some("x"), 1, "#000", 1.0);
            d.arc_dirs((5.0, 5.0), (1.0, 0.0), (0.0, 1.0), r, "#000", 1.0);
        }
        d.arc(5.0, 5.0, 3.0, 30.0, 30.0, "#000", 1.0);
        d.arc(5.0, 5.0, 3.0, f64::NAN, 30.0, "#000", 1.0);
        d.sector(5.0, 5.0, 3.0, 45.0, 45.0, "#ccc", "#000", 1.0);
        d.arc_dirs((5.0, 5.0), (1.0, 0.0), (1.0, 0.0), 3.0, "#000", 1.0);
        d.right_angle((1.0, 1.0), (f64::NAN, 0.0), (0.0, 1.0), 4.0, "#000", 1.0);
        d.right_angle((1.0, 1.0), (1.0, 0.0), (0.0, 1.0), -4.0, "#000", 1.0);
        d.dim_line(1.0, 1.0, 1.0, 1.0, "a", 12.0, "#000", 1.0);
        d.dim_line(f64::NAN, 1.0, 4.0, 1.0, "a", 12.0, "#000", 1.0);
        d.arrow_with(f64::NAN, 0.0, 1.0, 1.0, "#000", 1.0, "3 3", true);
        assert_eq!(d.finish(), empty);
    }

    #[test]
    fn full_turn_is_clamped_not_degenerate() {
        let mut d = Doc::new(10.0, 10.0);
        d.arc(5.0, 5.0, 3.0, 0.0, 720.0, "#000", 1.0);
        assert!(d.finish().contains(" 0 1 0 "));
    }
}
