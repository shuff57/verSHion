//! A supplied direction field, represented by editable segments or observed directions.
//! Formula sampling is opt-in for a printed derivative. No solution curve is computed.
use super::{
    FigureDef, ParamDef, Params, Subject,
    chart::{self, Frame},
    data,
    svg::Doc,
};

mod diagnostics;

pub fn def() -> FigureDef {
    FigureDef {
        id: "direction-field",
        subject: Subject::Math,
        title: "Direction field",
        blurb: "Recreate a supplied slope/direction field using explicit segment endpoints or observed column directions on a lattice. Optional printed-slope-formula sampling; never draws a solution curve or infers the equation of a given field.",
        params,
        render,
    }
}
fn params() -> Vec<ParamDef> {
    vec![
    ParamDef::choice("mode","Supplied field representation",&["Endpoints","Column directions","Printed slope formula"],"Endpoints"),
    ParamDef::data("segments","Segment endpoints x1,y1,x2,y2",4,"").hint("One visible source segment per row, in graph coordinates. JSON arrays of rows are accepted. Keep empty for blank source axes; no equation inference is needed."),
    ParamDef::data("columns","Observed directions x,dx,dy",3,"").hint("Column directions mode: one source x-column and its observed direction vector dx,dy per row. That direction repeats at the visible y lattice positions. This copies given directions without deriving an equation."),
    ParamDef::long("slope","Printed derivative expression in x,y","").hint("Used only in Printed slope formula mode. Enter the visibly printed dy/dx expression, such as x+y. Do not infer a formula when the task asks students to identify a given field. No integral or solution curve is drawn."),
    ParamDef::float("xmin","x-axis from",-1000.0,1000.0,1.0,-4.0),ParamDef::float("xmax","x-axis to",-1000.0,1000.0,1.0,4.0),
    ParamDef::float("ymin","y-axis from",-1000.0,1000.0,1.0,-4.0),ParamDef::float("ymax","y-axis to",-1000.0,1000.0,1.0,4.0),
    ParamDef::float("xstep","x tick spacing",0.01,1000.0,0.5,1.0),ParamDef::float("ystep","y tick spacing",0.01,1000.0,0.5,1.0),
    ParamDef::choice("x_number_format","x tick format",&["Numbers","Fractions","Multiples of π"],"Numbers").hint("Display only: coordinates, ranges, columns and steps stay in actual graph units (radians for a radian source). π labels recognize decimal approximations within 0.0001."),
    ParamDef::choice("y_number_format","y tick format",&["Numbers","Fractions","Multiples of π"],"Numbers"),
    ParamDef::long("x_tick_labels","Explicit x ticks and labels","").hint("Optional one position = literal label per semicolon, e.g. -pi = −π; -pi/2 = −π/2; pi/2 = π/2; pi = π. Positions accept numbers, fractions and pi. Overrides automatic x ticks without changing the supplied field or grid."),
    ParamDef::long("y_tick_labels","Explicit y ticks and labels","").hint("Optional position = literal label rows, separated by semicolons or newlines. Empty uses the y tick spacing and format."),
    ParamDef::choice("number_font","Tick label font",&["Sans","Serif"],"Sans"),
    ParamDef::bool("stacked_fractions","Stack fractional tick labels",false),
    ParamDef::float("number_size","Tick label size",8.0,22.0,1.0,12.0),
    ParamDef::float("sample_xstep","Formula sample column spacing",0.01,1000.0,0.1,0.5),ParamDef::float("sample_ystep","Sample row spacing",0.01,1000.0,0.1,0.5),
    ParamDef::float("segment_length","Sample segment length (graph units)",0.01,10.0,0.01,0.3).hint("Match the source's short-stroke length AND visible gaps. A length equal to row/column spacing can make steep neighboring strokes touch; use supplied endpoints for exact observed lengths. Do not turn a disconnected field into continuous lines."),
    ParamDef::float("field_clip_padding","Field clip padding (SVG px)",0.0,20.0,0.5,0.0).hint("Explicit extra room for whole observed outer marks, in SVG pixels. Expands only the field's clipping rectangle; never expands the axes/frame or adds sample rows/columns. Keep zero when the source intentionally clips its marks; inspect the original before changing it. Never applied automatically."),
    ParamDef::color("color","Segment colour",&["#0000ff","#000000","#ff0000"],"#0000ff"),ParamDef::float("line_width","Segment thickness",0.5,4.0,0.1,1.6),
    ParamDef::bool("grid","Draw gridlines",false),ParamDef::bool("numbers","Number the axes",true),ParamDef::bool("origin_zero","Print origin zero",false),
    ParamDef::text("xlabel","Horizontal axis letter","x"),ParamDef::text("ylabel","Vertical axis letter","y"),ParamDef::bool("arrows","Arrowheads on axes",true),
]
}
fn frame(p: &Params) -> Frame {
    Frame {
        w: 480.0,
        h: 480.0,
        l: 45.0,
        r: 35.0,
        t: 30.0,
        b: 50.0,
        x0: p.f("xmin"),
        x1: p.f("xmax"),
        y0: p.f("ymin"),
        y1: p.f("ymax"),
    }
}
pub fn validate(p: &Params) -> Result<(), String> {
    let padding = match p.0.get("field_clip_padding") {
        None => 0.0,
        Some(super::Value::F(v)) => *v,
        Some(super::Value::I(v)) => *v as f64,
        _ => return Err("Direction field clip padding must be a finite number from 0 to 20 SVG pixels".into()),
    };
    if !padding.is_finite() || !(0.0..=20.0).contains(&padding) {
        return Err("Direction field clip padding must be a finite number from 0 to 20 SVG pixels".into());
    }
    for (lo, hi, step) in [
        (p.f("xmin"), p.f("xmax"), p.f("xstep")),
        (p.f("ymin"), p.f("ymax"), p.f("ystep")),
    ] {
        if !(lo.is_finite() && hi.is_finite() && hi > lo && step > 0.0) || (hi - lo) / step > 400.0
        {
            return Err(
                "Direction field needs increasing axis ranges and at most 400 tick intervals"
                    .into(),
            );
        }
    }
    explicit_ticks(p, "x")?;
    explicit_ticks(p, "y")?;
    let mode = p.s("mode");
    let key = if mode == "Column directions" {
        "columns"
    } else {
        "segments"
    };
    let n = if mode == "Column directions" { 3 } else { 4 };
    let read = data::read(p.s(key), n);
    if read.skipped > 0 || read.truncated || read.rows.len() > 2500 {
        return Err(format!(
            "Invalid {key}: provide at most 2500 rows of {n} finite numbers"
        ));
    }
    if mode == "Column directions" && read.rows.iter().any(|r| r[1].hypot(r[2]) < 1e-12) {
        return Err("Observed direction vector must be nonzero".into());
    }
    if mode != "Endpoints" {
        let nx = if mode == "Column directions" {
            read.rows.len() as f64
        } else {
            ((p.f("xmax") - p.f("xmin")) / p.f("sample_xstep")).floor() + 1.0
        };
        let ny = ((p.f("ymax") - p.f("ymin")) / p.f("sample_ystep")).floor() + 1.0;
        if !nx.is_finite() || !ny.is_finite() || nx * ny > 2500.0 {
            return Err("Direction field lattice exceeds 2500 segments".into());
        }
    }
    if mode == "Printed slope formula"
        && (p.s("slope").trim().is_empty()
            || !(0..16).any(|i| {
                let x = p.f("xmin") + (p.f("xmax") - p.f("xmin")) * i as f64 / 15.0;
                super::math::eval_xy(p.s("slope"), x, 0.0).is_some_and(|v| !v.is_nan())
            }))
    {
        return Err("Cannot read the printed derivative expression".into());
    }
    Ok(())
}
fn explicit_ticks(p: &Params, axis: &str) -> Result<Option<Vec<chart::Tick>>, String> {
    let key = format!("{axis}_tick_labels");
    let text = p.s(&key).trim();
    if text.is_empty() {
        return Ok(None);
    }
    let (lo, hi) = (p.f(&format!("{axis}min")), p.f(&format!("{axis}max")));
    let mut out = vec![];
    for row in text
        .split([';', '\n'])
        .map(str::trim)
        .filter(|r| !r.is_empty())
    {
        let (a, b) = row
            .split_once('=')
            .ok_or_else(|| format!("{key}: expected position = label"))?;
        let at = super::math::eval_constant(a)
            .filter(|v| v.is_finite() && *v >= lo && *v <= hi)
            .ok_or_else(|| format!("{key}: tick position must be within the axis range"))?;
        if b.trim().is_empty()
            || b.chars().count() > 80
            || out
                .iter()
                .any(|(v, _): &(f64, String)| (v - at).abs() < 1e-9)
        {
            return Err(format!(
                "{key}: labels must be nonempty and tick positions unique"
            ));
        }
        out.push((at, b.trim().into()));
        if out.len() > 400 {
            return Err(format!("{key}: at most 400 ticks"));
        }
    }
    Ok(Some(out))
}
fn formatted_tick(v: f64, kind: &str) -> String {
    if kind == "Numbers" {
        return chart::fmt(v);
    }
    let pi = kind == "Multiples of π";
    let value = if pi { v / std::f64::consts::PI } else { v };
    for den in 1..=64 {
        let n = (value * den as f64).round();
        if (value - n / den as f64).abs() <= if pi { 1e-4 } else { 1e-9 } {
            let sign = if n < 0.0 { "−" } else { "" };
            let n = n.abs() as i64;
            let top = if pi && n != 0 {
                if n == 1 {
                    "π".into()
                } else {
                    format!("{n}π")
                }
            } else {
                n.to_string()
            };
            return if den == 1 || n == 0 {
                format!("{sign}{top}")
            } else {
                format!("{sign}{top}/{den}")
            };
        }
    }
    chart::fmt(v)
}
fn ticks(p: &Params, axis: &str) -> Vec<chart::Tick> {
    if let Ok(Some(v)) = explicit_ticks(p, axis) {
        return v;
    }
    chart::ticks(
        p.f(&format!("{axis}min")),
        p.f(&format!("{axis}max")),
        p.f(&format!("{axis}step")),
    )
    .into_iter()
    .map(|(v, _)| (v, formatted_tick(v, p.s(&format!("{axis}_number_format")))))
    .collect()
}
fn number(d: &mut Doc, p: &Params, x: f64, y: f64, anchor: &str, t: &str) {
    let fs = p.f("number_size");
    if p.s("number_font") == "Sans" && !p.b("stacked_fractions") && fs == 12.0 {
        d.text(x, y, fs, anchor, t);
        return;
    }
    let font = if p.s("number_font") == "Serif" {
        "Liberation Serif, DejaVu Serif, serif"
    } else {
        "Liberation Sans, DejaVu Sans, sans-serif"
    };
    let mut put = |x: f64, y: f64, an: &str, t: &str| {
        d.raw(&format!(r#"<text x="{x:.2}" y="{y:.2}" text-anchor="{an}" font-family="{font}" font-size="{fs}">{}</text>"#,super::svg::esc(t)));
    };
    if p.b("stacked_fractions") && t.contains('/') {
        let (a, b) = t.split_once('/').unwrap();
        let w = a.chars().count().max(b.chars().count()) as f64 * fs * 0.55;
        let cx = match anchor {
            "end" => x - w / 2.0,
            "start" => x + w / 2.0,
            _ => x,
        };
        put(cx, y - fs * 0.4, "middle", a);
        put(cx, y + fs * 0.7, "middle", b);
        d.line(
            cx - w / 2.0,
            y - fs * 0.15,
            cx + w / 2.0,
            y - fs * 0.15,
            "#000000",
            0.7,
        );
    } else {
        put(x, y, anchor, t);
    }
}
pub fn warnings(p: &Params) -> Vec<String> {
    validate(p).err().into_iter().collect()
}
fn sampled_segment(x: f64, y: f64, dx: f64, dy: f64, len: f64) -> [f64; 4] {
    let norm = dx.hypot(dy);
    let (ux, uy) = if norm.is_infinite() {
        (0.0, dy.signum())
    } else {
        (dx / norm, dy / norm)
    };
    [
        x - ux * len / 2.0,
        y - uy * len / 2.0,
        x + ux * len / 2.0,
        y + uy * len / 2.0,
    ]
}
fn segments(p: &Params) -> Vec<[f64; 4]> {
    if p.s("mode") == "Endpoints" {
        return data::read(p.s("segments"), 4)
            .rows
            .into_iter()
            .map(|r| [r[0], r[1], r[2], r[3]])
            .collect();
    }
    let rows = chart::ticks(p.f("ymin"), p.f("ymax"), p.f("sample_ystep"));
    let mut out = vec![];
    if p.s("mode") == "Column directions" {
        for r in data::read(p.s("columns"), 3).rows {
            for (y, _) in &rows {
                out.push(sampled_segment(r[0], *y, r[1], r[2], p.f("segment_length")));
            }
        }
    } else {
        for (x, _) in chart::ticks(p.f("xmin"), p.f("xmax"), p.f("sample_xstep")) {
            for (y, _) in &rows {
                if let Some(m) = super::math::eval_xy(p.s("slope"), x, *y).filter(|m| !m.is_nan()) {
                    out.push(sampled_segment(x, *y, 1.0, m, p.f("segment_length")));
                }
            }
        }
    }
    out
}

/// Describe only the emitted candidate field and its actual clip window, including
/// explicitly requested field-only padding. Reporting never changes the drawing.
/// A source can intentionally clip its marks: this is advisory, never a fidelity verdict.
pub(crate) fn render_with_diagnostics(p: &Params) -> (String, serde_json::Value) {
    let svg = render(p);
    let f = frame(p);
    let report = diagnostics::report(
        &svg, p.s("mode"), [f.x0, f.y0, f.x1, f.y1],
        [f.left(), f.top(), f.right(), f.bottom()], p.f("field_clip_padding"),
    );
    (svg, report)
}

fn render(p: &Params) -> String {
    if let Err(e) = validate(p) {
        return chart::message(&e);
    }
    let f = frame(p);
    let mut d = Doc::new(f.w, f.h);
    let xt = ticks(p, "x");
    let yt = ticks(p, "y");
    if p.b("grid") {
        for (v, _) in &chart::ticks(f.x0, f.x1, p.f("xstep")) {
            d.line(f.px(*v), f.top(), f.px(*v), f.bottom(), "#cccccc", 0.7);
        }
        for (v, _) in &chart::ticks(f.y0, f.y1, p.f("ystep")) {
            d.line(f.left(), f.py(*v), f.right(), f.py(*v), "#cccccc", 0.7);
        }
    }
    // Leave the world-to-SVG mapping and every supplied endpoint untouched. At
    // zero (including an omitted legacy parameter), emit the historical rectangle.
    let padding = p.f("field_clip_padding");
    d.raw(&format!(r#"<defs><clipPath id="field-window"><rect x="{}" y="{}" width="{}" height="{}"/></clipPath></defs><g clip-path="url(#field-window)" data-supplied-field="1">"#,f.left()-padding,f.top()-padding,f.right()-f.left()+2.0*padding,f.bottom()-f.top()+2.0*padding));
    for q in segments(p) {
        d.line(
            f.px(q[0]),
            f.py(q[1]),
            f.px(q[2]),
            f.py(q[3]),
            p.s("color"),
            p.f("line_width"),
        );
    }
    d.raw("</g>");
    let (ax, ay) = (
        f.px(0.0f64.clamp(f.x0, f.x1)),
        f.py(0.0f64.clamp(f.y0, f.y1)),
    );
    if p.b("arrows") {
        d.arrow_with(
            f.left() - 10.0,
            ay,
            f.right() + 10.0,
            ay,
            "#000000",
            1.2,
            "",
            true,
        );
        d.arrow_with(
            ax,
            f.bottom() + 10.0,
            ax,
            f.top() - 10.0,
            "#000000",
            1.2,
            "",
            true,
        );
    } else {
        d.line(f.left(), ay, f.right(), ay, "#000000", 1.2);
        d.line(ax, f.bottom(), ax, f.top(), "#000000", 1.2);
    }
    if p.b("numbers") {
        for (v, t) in &xt {
            if v.abs() < 1e-9 && !p.b("origin_zero") {
                continue;
            }
            d.line(f.px(*v), ay - 3.0, f.px(*v), ay + 3.0, "#000000", 0.8);
            number(&mut d, p, f.px(*v), ay + 16.0, "middle", t);
        }
        for (v, t) in &yt {
            if v.abs() < 1e-9 {
                continue;
            }
            d.line(ax - 3.0, f.py(*v), ax + 3.0, f.py(*v), "#000000", 0.8);
            number(&mut d, p, ax - 7.0, f.py(*v) + 4.0, "end", t);
        }
    }
    d.text(f.right() + 20.0, ay + 10.0, 14.0, "middle", p.s("xlabel"));
    d.text(ax + 10.0, f.top() - 15.0, 14.0, "middle", p.s("ylabel"));
    d.finish()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{self, Value, spec};

    fn padding_fixture(mode: &str) -> Params {
        let mut p = figures::start_params(&def());
        p.set_s("mode", mode);
        p.set_b("grid", true);
        match mode {
            "Endpoints" => p.set_s("segments", "-4,-4.15,-4,-3.85\n0,0,0.15,0.1\n4,3.85,4,4.15"),
            "Column directions" => p.set_s("columns", &(-8..=8)
                .map(|i| format!("{},1,2", i as f64 / 2.0)).collect::<Vec<_>>().join("\n")),
            "Printed slope formula" => p.set_s("slope", "x-y/2"),
            _ => unreachable!(),
        }
        p
    }

    fn without_field_clip(svg: &str) -> String {
        let start = svg.find("<defs><clipPath id=\"field-window\">").unwrap();
        let end = start + svg[start..].find("</clipPath></defs>").unwrap()
            + "</clipPath></defs>".len();
        format!("{}{}", &svg[..start], &svg[end..])
    }

    #[test]
    fn field_clip_padding_zero_and_omitted_preserve_legacy_svg_and_native_png() {
        use sha2::{Digest, Sha256};
        // Captured from the unchanged R13 native test executable, using these
        // synthetic fixtures before the padding control was implemented.
        for (mode, legacy_svg_sha256) in [
            ("Endpoints", "652fdf6f017de3c4801baebb29b2b6d5acde4e6fff16c0fe5bea732258ddb18b"),
            ("Column directions", "9db8c1e06abf85774ae63097acd9bf427fcdd2f775ec25efa9ff81ba751fbc84"),
            ("Printed slope formula", "efae9ada7860102a289cc5c06144790cc65045da3a628cfe48bb4503201a9c5f"),
        ] {
            let mut p = padding_fixture(mode);
            assert_eq!(p.f("field_clip_padding"), 0.0);
            let zero = def().svg(&p);
            assert_eq!(format!("{:x}", Sha256::digest(zero.as_bytes())), legacy_svg_sha256, "{mode}");
            p.0.remove("field_clip_padding");
            assert!(validate(&p).is_ok());
            let omitted = def().svg(&p);
            assert_eq!(zero, omitted, "{mode}");
            assert_eq!(figures::to_png(&zero, 1.0).unwrap(), figures::to_png(&omitted, 1.0).unwrap(), "{mode}");
        }
    }

    #[test]
    fn field_clip_padding_retains_outer_marks_without_resampling_or_moving_axes() {
        for (mode, emitted) in [("Endpoints", 3), ("Column directions", 289), ("Printed slope formula", 289)] {
            let mut p = padding_fixture(mode);
            let before = segments(&p);
            let xt = ticks(&p, "x");
            let yt = ticks(&p, "y");
            let (ordinary, clipped) = render_with_diagnostics(&p);
            assert_eq!(before.len(), emitted);
            assert_eq!(clipped["counts"]["emitted_segments"], emitted);
            assert!(clipped["counts"]["clipped_centerlines"].as_u64().unwrap() > 0, "{mode}: {clipped}");
            assert!(clipped["centerline_lengths_px"]["removed_by_window"].as_f64().unwrap() > 0.0);
            p.set_f("field_clip_padding", 10.0);
            let (padded, whole) = render_with_diagnostics(&p);
            assert_eq!(segments(&p), before, "supplied endpoints changed in {mode}");
            assert_eq!(ticks(&p, "x"), xt);
            assert_eq!(ticks(&p, "y"), yt);
            assert_eq!(without_field_clip(&ordinary), without_field_clip(&padded), "only the clipping rectangle may change in {mode}");
            assert_eq!(whole["graph_window_xyxy"], serde_json::json!([-4.0, -4.0, 4.0, 4.0]));
            assert_eq!(whole["clip_rectangle_svg_xywh_px"], serde_json::json!([35.0, 20.0, 420.0, 420.0]));
            assert_eq!(whole["counts"]["emitted_segments"], emitted);
            assert_eq!(whole["counts"]["whole_centerlines"], emitted);
            assert_eq!(whole["counts"]["clipped_centerlines"], 0);
            assert_eq!(whole["counts"]["outside_centerlines"], 0);
            assert_eq!(whole["counts"]["endpoints_outside_window"], 0);
            assert_eq!(whole["centerline_lengths_px"]["removed_by_window"], 0.0);
            assert_eq!(whole["centerline_lengths_px"]["emitted"], whole["centerline_lengths_px"]["retained"]);
            assert_eq!(whole["stroke_envelope_advisory"]["round_cap_envelope_bounds_cross_window"], 0);
            assert_eq!(whole["stroke_envelope_advisory"]["round_cap_envelope_bounds_wholly_outside_window"], 0);
            assert_eq!(whole["source_match"], "not_assessed");
            assert_eq!(whole["advisory_only"], true);
        }
    }

    #[test]
    fn field_clip_padding_is_bounded_explicit_and_survives_saved_specs() {
        let mut p = padding_fixture("Column directions");
        for invalid in [-0.1, 20.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            p.set_f("field_clip_padding", invalid);
            assert!(validate(&p).unwrap_err().contains("clip padding"), "{invalid}");
        }
        p.set_s("field_clip_padding", "10");
        assert!(validate(&p).is_err());
        for padding in [0.0, 10.0, 20.0] {
            p.set_f("field_clip_padding", padding);
            assert!(validate(&p).is_ok());
            let saved = spec::spec_for("direction-field", &p);
            let restored = spec::params_from_spec(&def(), &saved);
            assert_eq!(restored.0, p.0);
            assert_eq!(render(&restored), render(&p));
            let (svg, report) = render_with_diagnostics(&p);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let rect = xml.descendants().find(|n| n.has_tag_name("rect")
                && n.parent_element().is_some_and(|n| n.attribute("id") == Some("field-window"))).unwrap();
            let value = |key| rect.attribute(key).unwrap().parse::<f64>().unwrap();
            let (x, y, w, h) = (value("x"), value("y"), value("width"), value("height"));
            assert!(x >= 0.0 && y >= 0.0 && x + w <= 480.0 && y + h <= 480.0);
            assert_eq!(report["clip_rectangle_svg_xywh_px"], serde_json::json!([x, y, w, h]));
        }
    }

    #[test]
    fn pi_fraction_tick_labels_preserve_the_supplied_field_and_world_positions() {
        let mut p = figures::start_params(&def());
        p.set_s("segments", "-1,-0.1,-1,0.1\n0,0,0.2,0.3");
        p.set_f("xstep", std::f64::consts::PI / 2.0);
        let before = segments(&p);
        let original = ticks(&p, "x");
        p.set_s("x_number_format", "Multiples of π");
        p.set_s("number_font", "Serif");
        p.set_b("stacked_fractions", true);
        let xt = ticks(&p, "x");
        assert_eq!(before, segments(&p));
        assert_eq!(
            original.iter().map(|x| x.0).collect::<Vec<_>>(),
            xt.iter().map(|x| x.0).collect::<Vec<_>>()
        );
        assert_eq!(
            xt.iter().map(|x| x.1.as_str()).collect::<Vec<_>>(),
            ["−π", "−π/2", "0", "π/2", "π"]
        );
        let s = render(&p);
        assert!(!s.contains("1.570796"));
        assert!(s.contains("Liberation Serif"));
        assert!(crate::figures::to_png(&s, 1.0).is_ok());
        assert_eq!(formatted_tick(1.5708, "Multiples of π"), "π/2");
        assert_eq!(formatted_tick(-1.5, "Fractions"), "−3/2");
    }
    #[test]
    fn explicit_ticks_keep_literal_labels_and_cannot_change_the_grid_or_field() {
        let mut p = figures::start_params(&def());
        p.set_b("grid", true);
        let s = render(&p);
        let count = |s: &str| s.matches("stroke=\"#cccccc\"").count();
        p.set_s(
            "x_tick_labels",
            "-pi = −π; -pi/2 = −π/2; pi/2 = π/2; pi = π",
        );
        let xt = ticks(&p, "x");
        assert_eq!(xt.len(), 4);
        assert!((xt[0].0 + std::f64::consts::PI).abs() < 1e-12);
        assert_eq!(count(&s), count(&render(&p)));
        assert!(segments(&p).is_empty());
        p.set_s("x_tick_labels", "0 = a; 0 = b");
        assert!(validate(&p).is_err());
        p.set_s("x_tick_labels", "99 = outside");
        assert!(validate(&p).is_err());
    }
    #[test]
    fn source_endpoints_survive_exactly_without_a_formula_or_solution() {
        let mut p = figures::start_params(&def());
        p.set_s("segments", "-1,-0.1,-1,0.1\n0,0,0.2,0.3\n1,1,1.3,0.8");
        let s = render(&p);
        let field = s
            .split("data-supplied-field=\"1\"")
            .nth(1)
            .unwrap()
            .split("</g>")
            .next()
            .unwrap();
        assert_eq!(field.matches("<line ").count(), 3);
        assert!(p.s("slope").is_empty());
        assert!(!s.contains("<polyline") && !s.contains("<path"));
        assert_eq!(
            spec::params_from_spec(&def(), &spec::spec_for("direction-field", &p)).0,
            p.0
        );
        assert!(figures::to_png(&s, 1.0).is_ok());
    }
    #[test]
    fn observed_column_vectors_repeat_only_the_supplied_directions() {
        let mut p = figures::start_params(&def());
        p.set_s("mode", "Column directions");
        p.set_s("columns", "-1,0,1\n0,1,0\n1,1,-1");
        p.set_f("sample_ystep", 1.0);
        let v = segments(&p);
        assert_eq!(v.len(), 27);
        for s in &v[..9] {
            assert_eq!(s[0], s[2]);
        }
        for s in &v[9..18] {
            assert_eq!(s[1], s[3]);
        }
        assert!(p.s("slope").is_empty());
    }
    #[test]
    fn formula_mode_is_explicit_and_cannot_draw_a_solution_curve() {
        let mut p = figures::start_params(&def());
        assert!(segments(&p).is_empty());
        p.set_s("slope", "x+y");
        assert!(segments(&p).is_empty());
        p.set_s("mode", "Printed slope formula");
        assert_eq!(segments(&p).len(), 289);
        p.0.insert("sample_xstep".into(), Value::F(0.001));
        assert!(validate(&p).is_err());
    }
}
