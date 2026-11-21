//! Scatterplot of (x, y) pairs with an optional least-squares line, its equation and r. The
//! points are what a student is given; the line, equation and r are the answers, so the student
//! copy (Show answers off) keeps the points and hides them.
use crate::figures::chart::{self, AxesOpts, Frame, INK};
use crate::figures::data;
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

mod diagnostics;

pub fn def() -> FigureDef {
    FigureDef {
        id: "scatterplot",
        subject: Subject::Math,
        title: "Scatterplot",
        blurb: "A scatterplot of x, y pairs with automatic or explicit axes, independently bounded grids, major/minor grid colours, plain or outlined points with preset or custom radius, and an optional best-fit line, equation and r. Manual axes also support blank graph paper.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        ("scatterplot", "eq") => p.b("fit"),
        ("scatterplot", "grid_xmin" | "grid_xmax" | "grid_ymin" | "grid_ymax") => p.s("grid_bounds_mode") == "Explicit",
        ("scatterplot", "minor_grid_color") => p.i("x_major_every") > 1 || p.i("y_major_every") > 1,
        ("scatterplot", "axis_x_extension" | "axis_y_extension" | "axis_arrows" | "x_end_label" | "y_end_label") => p.s("axis_style") != "None",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "scatterplot" => vec![
            Example { name: "Hours studied and score", params: vec![] },
            Example {
                name: "Falling trend",
                params: vec![
                    ("pts", s(&data::pairs_text(&data::generate_pairs(30, 8, 40.0, 12.0, -1.5, 160.0, 9.0, 0)))),
                    ("x_title", s("Age of car (months)")),
                    ("y_title", s("Value (hundreds of dollars)")),
                ],
            },
            Example {
                name: "No relationship",
                params: vec![("pts", s(&data::pairs_text(&data::generate_pairs(30, 3, 50.0, 10.0, 0.0, 40.0, 8.0, 0))))],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::data("pts", "Pairs (x, y)", 2, "1, 52\n2, 55\n2.5, 61\n3, 60\n4, 68\n4.5, 71\n5, 70\n6, 79\n6.5, 77\n7, 85\n8, 88\n9, 93")
            .hint("Leave pairs empty with Manual axes for blank graph paper. Axis extension controls lengthen only the axes; grid bounds remain independent."),
        ParamDef::bool("fit", "Line of best fit", true),
        ParamDef::bool("eq", "Write its equation", false),
        ParamDef::bool("rval", "Write the correlation r", false),
        ParamDef::choice("point_size", "Point size", &["Small", "Medium", "Large"], "Medium"),
        ParamDef::float("point_radius", "Custom point radius", 0.0, 64.0, 0.1, 0.0)
            .hint("Zero uses the Point size preset. A positive radius is in drawing units, preserving source marker size relative to grid spacing, touching and overlaps without moving the supplied data. Use Square plot area when the source grid is square."),
        ParamDef::bool("grid", "Gridlines", true),
        ParamDef::choice("axes_mode", "Axis ranges", &["Automatic", "Manual"], "Automatic"),
        ParamDef::float("xmin", "x-axis from", -1e15, 1e15, 1.0, 0.0),
        ParamDef::float("xmax", "x-axis to", -1e15, 1e15, 1.0, 10.0),
        ParamDef::float("xstep", "Vertical grid spacing", 1e-9, 1e15, 1.0, 1.0),
        ParamDef::float("ymin", "y-axis from", -1e15, 1e15, 1.0, 0.0),
        ParamDef::float("ymax", "y-axis to", -1e15, 1e15, 1.0, 10.0),
        ParamDef::float("ystep", "Horizontal grid spacing", 1e-9, 1e15, 1.0, 1.0),
        ParamDef::choice("grid_bounds_mode", "Grid extent", &["Follow axes", "Explicit"], "Follow axes")
            .hint("Explicit keeps the grid rectangle independent of the axis ranges. Axes and data can extend beyond the last gridline without adding more gridlines."),
        ParamDef::float("grid_xmin", "Grid left edge", -1e15, 1e15, 1.0, 0.0),
        ParamDef::float("grid_xmax", "Grid right edge", -1e15, 1e15, 1.0, 10.0),
        ParamDef::float("grid_ymin", "Grid bottom edge", -1e15, 1e15, 1.0, 0.0),
        ParamDef::float("grid_ymax", "Grid top edge", -1e15, 1e15, 1.0, 10.0),
        ParamDef::long("x_ticks", "Explicit x tick labels", "").hint("Optional comma-separated tick positions to label; e.g. 200,400,600,800. Empty uses the grid step. This changes labels without changing grid spacing."),
        ParamDef::long("y_ticks", "Explicit y tick labels", "").hint("Optional comma-separated tick positions to label; empty uses the grid step."),
        ParamDef::bool("zero_labels", "Print zero tick labels", true),
        ParamDef::choice("grid_directions", "Grid directions", &["Horizontal", "Both"], "Horizontal"),
        ParamDef::color("grid_color", "Grid colour", &["#dfe3ea", "#777777", "#000000", "#b0b0b0"], "#dfe3ea"),
        ParamDef::int("x_major_every", "Darker vertical gridline every", 1, 100, 1)
            .hint("Counts existing x grid steps from zero; 1 keeps every line in Grid colour. Does not add lines or change tick labels."),
        ParamDef::int("y_major_every", "Darker horizontal gridline every", 1, 100, 1)
            .hint("Counts existing y grid steps from zero; intervening lines use Minor grid colour."),
        ParamDef::color("minor_grid_color", "Minor grid colour", &["#dfe3ea", "#c7c7c7", "#b0b0b0", "#777777"], "#dfe3ea"),
        ParamDef::choice("axis_style", "Axes", &["Left and bottom", "Box", "None"], "Left and bottom"),
        ParamDef::float("axis_x_extension", "x-axis extra length (grid steps)", 0.0, 4.0, 0.05, 0.0)
            .hint("Extends only the right axis stroke into extra canvas space. Gridlines, tick positions and supplied data coordinates stay fixed. With Box axes the bottom axis runs past the box."),
        ParamDef::float("axis_y_extension", "y-axis extra length (grid steps)", 0.0, 4.0, 0.05, 0.0)
            .hint("Extends only the upper axis stroke into extra canvas space; does not add gridlines, numbers, points or a top border."),
        ParamDef::float("axis_width", "Axis thickness", 0.0, 4.0, 0.1, 2.0),
        ParamDef::bool("axis_arrows", "Arrowheads on the axis ends", false)
            .hint("Filled arrowheads at the right end of the x-axis and the top of the y-axis, as printed on many test graphs. Pair with the axis extensions when the source arrow runs past the grid or box."),
        ParamDef::text("x_end_label", "Letter at the x-axis end", "")
            .hint("A variable printed just past the arrow end of the x-axis, such as x. Separate from the centred axis title."),
        ParamDef::text("y_end_label", "Letter at the y-axis end", "")
            .hint("A variable printed just above the top of the y-axis, such as y. Separate from the rotated axis title."),
        ParamDef::bool("tick_marks", "Tick marks outside the grid", true),
        ParamDef::bool("square_plot", "Square plot area", false),
        ParamDef::choice("point_color", "Point colour", &["Automatic", "#0000ff", "#000000", "#ff0000"], "Automatic"),
        ParamDef::bool("point_outline", "Outline the points", true),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("x_title", "Horizontal axis title", "Hours studied"),
        ParamDef::text("y_title", "Vertical axis title", "Score"),
        ParamDef::choice("y_title_at", "Vertical axis title sits", &["Turned, left of the axis", "Across, above the axis"], "Turned, left of the axis")
            .hint("Across prints the title level, just above the top of the y-axis, as some exam graphs do"),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the line, equation and r, leaving just the points for students to work from"),
    ]
}

/// Least-squares line through a set of points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub slope: f64,
    pub intercept: f64,
    /// Correlation; None when y does not vary at all.
    pub r: Option<f64>,
}

/// The line of best fit, or None with fewer than two points, all the same x, or overflow.
pub fn fit(pts: &[(f64, f64)]) -> Option<Fit> {
    if pts.len() < 2 {
        return None;
    }
    let n = pts.len() as f64;
    let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for (x, y) in pts {
        sxx += (x - mx) * (x - mx);
        sxy += (x - mx) * (y - my);
        syy += (y - my) * (y - my);
    }
    if !(sxx > 0.0 && sxx.is_finite() && sxy.is_finite() && syy.is_finite()) {
        return None;
    }
    let slope = sxy / sxx;
    let intercept = my - slope * mx;
    if !(slope.is_finite() && intercept.is_finite()) {
        return None;
    }
    let r = (syy > 0.0).then(|| (sxy / (sxx * syy).sqrt()).clamp(-1.0, 1.0));
    Some(Fit { slope, intercept, r })
}

/// "y = 2.5x + 4" (the sign and spacing written out, coefficients to three decimals).
pub fn equation(f: &Fit) -> String {
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    let m = round(f.slope);
    let b = round(f.intercept);
    let mut s = String::from("y = ");
    if m == 0.0 {
        return s + &chart::fmt(b);
    }
    s += &match m {
        1.0 => "x".to_string(),
        -1.0 => "\u{2212}x".to_string(),
        _ => format!("{}x", chart::fmt(m)),
    };
    if b > 0.0 {
        s += &format!(" + {}", chart::fmt(b));
    } else if b < 0.0 {
        s += &format!(" \u{2212} {}", chart::fmt(-b));
    }
    s
}

/// The scatterplot's axes: about six intervals, padded (see `chart::axis_range`).
pub fn axis_range(lo: f64, hi: f64) -> (f64, f64, f64) {
    chart::axis_range(lo, hi, 6.0)
}

/// The part of the line y = m x + b inside the box, if any (Liang-Barsky on y).
pub fn clip(m: f64, b: f64, x0: f64, x1: f64, y0: f64, y1: f64) -> Option<((f64, f64), (f64, f64))> {
    let (ya, yb) = (m * x0 + b, m * x1 + b);
    let dy = yb - ya;
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    if dy == 0.0 {
        if ya < y0 || ya > y1 {
            return None;
        }
    } else {
        let (ta, tb) = ((y0 - ya) / dy, (y1 - ya) / dy);
        t0 = t0.max(ta.min(tb));
        t1 = t1.min(ta.max(tb));
        if t0 > t1 {
            return None;
        }
    }
    Some(((x0 + t0 * (x1 - x0), ya + t0 * dy), (x0 + t1 * (x1 - x0), ya + t1 * dy)))
}

fn render(p: &Params) -> String {
    render_impl(p, false).0
}

/// Render normally while describing only this candidate's emitted SVG geometry.
/// No source/reference inputs, coordinate repair, raster-region claims or grading gate.
pub(crate) fn render_with_diagnostics(p: &Params) -> (String, serde_json::Value) {
    render_impl(p, true)
}

fn render_impl(p: &Params, capture: bool) -> (String, serde_json::Value) {
    let message = |reason, text| {
        let svg = chart::message(text);
        let report = if capture { diagnostics::unavailable(&svg, reason) } else { serde_json::Value::Null };
        (svg, report)
    };
    let pts = data::pairs(p.s("pts"));
    let manual=p.s("axes_mode")=="Manual";
    if pts.is_empty() && !manual {
        return message("empty_automatic_data", "Enter some (x, y) pairs");
    }
    let (xlo, xhi) = pts.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.0), b.max(q.0)));
    let (ylo, yhi) = pts.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.1), b.max(q.1)));
    let (ax0, ax1, xs) = if manual {(p.f("xmin"),p.f("xmax"),p.f("xstep"))}else{axis_range(xlo, xhi)};
    let (ay0, ay1, ys) = if manual {(p.f("ymin"),p.f("ymax"),p.f("ystep"))}else{axis_range(ylo, yhi)};
    if manual && (!(ax1>ax0 && ay1>ay0 && xs>0.0 && ys>0.0) || (ax1-ax0)/xs>400.0 || (ay1-ay0)/ys>400.0) {return message("invalid_manual_axes", "Manual axes need increasing ranges and at most 400 grid intervals");}
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let (xt, yt, title) = (p.s("x_title").trim(), p.s("y_title").trim(), p.s("title").trim());
    let explicit_grid = p.s("grid_bounds_mode") == "Explicit";
    let (gx0, gx1, gy0, gy1) = if explicit_grid {
        (p.f("grid_xmin").max(ax0), p.f("grid_xmax").min(ax1), p.f("grid_ymin").max(ay0), p.f("grid_ymax").min(ay1))
    } else { (ax0, ax1, ay0, ay1) };
    if explicit_grid && !(gx0 < gx1 && gy0 < gy1) {
        return message("invalid_explicit_grid", "The explicit grid rectangle must have increasing bounds inside the axes");
    }
    let xgrid = chart::ticks(gx0, gx1, xs);
    let ygrid = chart::ticks(gy0, gy1, ys);
    let ticks=|key:&str,grid:&Vec<chart::Tick>,lo:f64,hi:f64|->Vec<chart::Tick>{let values=if p.s(key).trim().is_empty(){grid.clone()}else{data::numbers(p.s(key)).into_iter().filter(|v|*v>=lo&&*v<=hi).take(400).map(|v|(v,chart::fmt(v))).collect()};values.into_iter().filter(|(v,_)|p.b("zero_labels")||v.abs()>1e-12).collect()};
    let xticks=ticks("x_ticks",&xgrid,ax0,ax1);
    let yticks=ticks("y_ticks",&ygrid,ay0,ay1);
    let label_chars = yticks.iter().map(|t| t.1.chars().count()).max().unwrap_or(1) as f64;
    let (w, h) = (640.0, 420.0 + (k - 1.0) * 60.0);
    let mut frame = Frame {
        w,
        h,
        l: 34.0 + fs * 1.4 + label_chars * fs * 0.62,
        r: 28.0,
        t: if title.is_empty() { 24.0 } else { 34.0 + fs * 1.4 },
        b: 22.0 + fs * 1.9 + if xt.is_empty() { 0.0 } else { fs * 1.7 },
        x0: ax0,
        x1: ax1,
        y0: ay0,
        y1: ay1,
    };
    if p.b("square_plot"){frame.h=frame.w-frame.l-frame.r+frame.t+frame.b;}
    let (xext,yext)=if p.s("axis_style")!="None" {
        (p.f("axis_x_extension").clamp(0.0,4.0)*xs/(ax1-ax0)*(frame.right()-frame.left()),p.f("axis_y_extension").clamp(0.0,4.0)*ys/(ay1-ay0)*(frame.bottom()-frame.top()))
    } else {(0.0,0.0)};
    let ends=p.s("axis_style")!="None";
    let (x_end,y_end)=if ends {(p.s("x_end_label").trim().to_string(),p.s("y_end_label").trim().to_string())} else {(String::new(),String::new())};
    let arrows=ends&&p.b("axis_arrows");
    // Room past the axis ends for an end letter.
    let y_above=p.s("y_title_at")=="Across, above the axis";
    let (xroom,yroom)=(if x_end.is_empty(){0.0}else{fs*1.6},if y_end.is_empty()&&!y_above{0.0}else{fs*1.5});
    // Preserve the plot's size and mapping. Only the new upper/right margin grows.
    frame.t+=yext+yroom;frame.h+=yext+yroom;
    let mut d = Doc::new(frame.w+xext+xroom, frame.h);
    let mut captured = capture.then(|| diagnostics::Capture::new(pts.len(), p.s("axis_style"), p.b("grid")));
    d.defs(&fills.defs());
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.4, fs * 1.3, "middle", title);
    }
    let custom=manual||explicit_grid||xext>0.0||yext>0.0||p.i("x_major_every")>1||p.i("y_major_every")>1||p.s("grid_directions")=="Both"||p.s("grid_color")!="#dfe3ea"||p.s("axis_style")!="Left and bottom"||p.f("axis_width")!=2.0||!p.b("tick_marks")||!p.b("zero_labels")||!p.s("x_ticks").trim().is_empty()||!p.s("y_ticks").trim().is_empty()||arrows||!x_end.is_empty()||!y_end.is_empty()||y_above;
    if !custom {
        let from = d.len();
        chart::axes(&mut d, &frame, &AxesOpts { xticks: &xticks, yticks: &yticks, grid: p.b("grid"), x_title: xt, y_title: yt, fs });
        if let Some(c) = &mut captured { c.legacy_axes(d.fragment(from), if p.b("grid") { yticks.len() } else { 0 }); }
    }
    else {
        let grid_colour = |v: f64, step: f64, key: &str| {
            if ((v / step).round() as i64).rem_euclid(p.i(key).max(1)) == 0 { p.s("grid_color") } else { p.s("minor_grid_color") }
        };
        let from = d.len();
        if p.b("grid") {for(v,_)in &ygrid{d.line(frame.px(gx0),frame.py(*v),frame.px(gx1),frame.py(*v),grid_colour(*v,ys,"y_major_every"),1.0);}if p.s("grid_directions")=="Both"{for(v,_)in &xgrid{d.line(frame.px(*v),frame.py(gy1),frame.px(*v),frame.py(gy0),grid_colour(*v,xs,"x_major_every"),1.0);}}}
        if let Some(c) = &mut captured { c.grid(d.fragment(from)); }
        let from = d.len();
        if p.s("axis_style")!="None" {
            if arrows {d.arrow(frame.left(),frame.bottom(),frame.right()+xext,frame.bottom(),INK,p.f("axis_width").max(0.5));d.arrow(frame.left(),frame.bottom(),frame.left(),frame.top()-yext,INK,p.f("axis_width").max(0.5));}
            else {d.line(frame.left(),frame.bottom(),frame.right()+xext,frame.bottom(),INK,p.f("axis_width"));d.line(frame.left(),frame.top()-yext,frame.left(),frame.bottom(),INK,p.f("axis_width"));}
            if p.s("axis_style")=="Box"{d.line(frame.left(),frame.top(),frame.right(),frame.top(),INK,p.f("axis_width"));d.line(frame.right(),frame.top(),frame.right(),frame.bottom(),INK,p.f("axis_width"));}
        }
        if let Some(c) = &mut captured {
            let roles: &[&str] = match p.s("axis_style") {
                "None" => &[], "Box" => &["bottom", "left", "top", "right"], _ => &["bottom", "left"],
            };
            c.axes(d.fragment(from), roles);
            if arrows { c.arrows(); }
        }
        for(v,t)in &xticks{let x=frame.px(*v);if p.b("tick_marks"){d.line(x,frame.bottom(),x,frame.bottom()+6.0,INK,1.6);}chart::text(&mut d,x,frame.bottom()+8.0+fs*1.05,fs,"middle",t);}
        for(v,t)in &yticks{let y=frame.py(*v);if p.b("tick_marks"){d.line(frame.left()-6.0,y,frame.left(),y,INK,1.6);}chart::text(&mut d,frame.left()-10.0,y+fs*0.35,fs,"end",t);}
        if !x_end.is_empty(){chart::text(&mut d,frame.right()+xext+5.0,frame.bottom()+fs*0.35,fs*1.1,"start",&x_end);}
        if !y_end.is_empty(){chart::text(&mut d,frame.left(),frame.top()-yext-6.0,fs*1.1,"middle",&y_end);}
        if !xt.is_empty(){chart::text(&mut d,(frame.left()+frame.right())/2.0,frame.h-10.0,fs*1.1,"middle",xt);}if !yt.is_empty(){if y_above{chart::text(&mut d,frame.left()+if y_end.is_empty(){0.0}else{fs*1.2},frame.top()-yext-6.0,fs*1.1,"start",yt);}else{chart::vtext(&mut d,18.0+fs*0.2,(frame.top()+frame.bottom())/2.0,fs*1.1,yt);}}
    }

    let answers = p.b(SHOW_ANSWERS);
    let f = fit(&pts);
    if answers && p.b("fit") {
        if let Some(f) = &f {
            if let Some(((xa, ya), (xb, yb))) = clip(f.slope, f.intercept, ax0, ax1, ay0, ay1) {
                let dash = fills.dash(1);
                d.raw(&format!(
                    r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="2.6" stroke-linecap="round"{}/>"#,
                    frame.px(xa),
                    frame.py(ya),
                    frame.px(xb),
                    frame.py(yb),
                    fills.line(1),
                    if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) }
                ));
            }
        }
    }
    let radius = if p.f("point_radius") > 0.0 { p.f("point_radius").min(64.0) } else { match p.s("point_size") {
        "Small" => 3.2,
        "Large" => 6.4,
        _ => 4.6,
    }};
    for (index, (x, y)) in pts.iter().enumerate() {
        if manual && (*x<ax0||*x>ax1||*y<ay0||*y>ay1){
            if let Some(c) = &mut captured { c.skipped(index + 1); }
            continue;
        }
        let color=if p.s("point_color")=="Automatic"||fills.is_print(){fills.series(0)}else{p.s("point_color").to_string()};
        let from = d.len();
        d.circle(frame.px(*x), frame.py(*y), radius, &color, if p.b("point_outline"){INK}else{"none"}, if p.b("point_outline"){1.5}else{0.0});
        if let Some(c) = &mut captured { c.marker(index + 1, d.fragment(from)); }
    }
    if answers {
        let mut notes: Vec<String> = Vec::new();
        if p.b("fit") && p.b("eq") {
            if let Some(f) = &f {
                notes.push(equation(f));
            }
        }
        if p.b("rval") {
            if let Some(r) = f.as_ref().and_then(|f| f.r) {
                notes.push(format!("r = {}", chart::fmt((r * 1000.0).round() / 1000.0)));
            }
        }
        // the notes sit in the corner the line stays away from: top-left for a rising line, top-right for a falling one
        let right = f.is_some_and(|f| f.slope < 0.0) && p.b("fit");
        for (i, n) in notes.iter().enumerate() {
            let w = n.chars().count() as f64 * fs * 0.56 + 14.0;
            let y = frame.top() + 10.0 + i as f64 * (fs * 1.5);
            let (plate_x, text_x, anchor) = if right { (frame.right() - 14.0 - w, frame.right() - 14.0 - 6.0, "end") } else { (frame.left() + 14.0 - 6.0, frame.left() + 14.0, "start") };
            // a white plate keeps the note readable over gridlines and points
            d.rect(plate_x, y, w, fs * 1.4, "#fff", chart::GRID, 1.0);
            chart::text(&mut d, text_x, y + fs * 1.05, fs, anchor, n);
        }
    }
    let svg = d.finish();
    let report = captured.map(|c| c.finish(&svg)).unwrap_or(serde_json::Value::Null);
    (svg, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }

    #[test]
    fn custom_markers_can_touch_an_axis_and_overlap_without_moving_data() {
        let mut p=defaults();p.set_s("axes_mode","Manual");p.set_f("xmin",0.0);p.set_f("xmax",10.0);p.set_f("xstep",1.0);p.set_f("ymin",0.0);p.set_f("ymax",10.0);p.set_f("ystep",1.0);
        p.set_s("pts","0.18,5\n0.52,5");p.set_s("point_size","Large");p.set_b("point_outline",false);p.set_b("fit",false);p.set_b("square_plot",true);
        let before=render(&p);p.set_f("point_radius",12.0);let after=render(&p);
        let old=roxmltree::Document::parse(&before).unwrap();let new=roxmltree::Document::parse(&after).unwrap();
        let circles=|doc:&roxmltree::Document|doc.descendants().filter(|n|n.has_tag_name("circle")).map(|n|["cx","cy","r"].map(|k|n.attribute(k).unwrap().parse::<f64>().unwrap())).collect::<Vec<_>>();
        let a=circles(&old);let b=circles(&new);assert_eq!(a.len(),2);assert_eq!(b.len(),2);
        assert_eq!(a.iter().map(|c|[c[0],c[1]]).collect::<Vec<_>>(),b.iter().map(|c|[c[0],c[1]]).collect::<Vec<_>>(),"changing marker radius cannot move supplied observations");
        let axis=new.descendants().filter(|n|n.has_tag_name("line")&&n.attribute("stroke")==Some(INK)&&n.attribute("x1")==n.attribute("x2")).map(|n|n.attribute("x1").unwrap().parse::<f64>().unwrap()).next().unwrap();
        assert!(a[0][0]-a[0][2]>axis,"the old largest preset cannot reach this axis");
        assert!(b[0][0]-b[0][2]<axis,"an explicit radius can preserve a visible marker/axis overlap");
        assert!(a[1][0]-a[0][0]>a[0][2]+a[1][2]);assert!(b[1][0]-b[0][0]<b[0][2]+b[1][2],"marker overlap changes without altering the data or inventing observations");
        assert_eq!(old.descendants().filter(|n|n.has_tag_name("line")).count(),new.descendants().filter(|n|n.has_tag_name("line")).count());
        let defs=params();let sections=crate::figures::layout::sections("scatterplot",&defs,None);
        assert!(sections.iter().any(|s|s.items.iter().any(|i|defs[*i].key=="point_radius")),"custom radius is in the existing Figures tab");
    }

    #[test]
    fn zero_custom_radius_preserves_all_legacy_size_presets() {
        for (size,radius) in [("Small",3.2),("Medium",4.6),("Large",6.4)] {
            let mut p=defaults();p.set_s("point_size",size);let before=render(&p);p.set_f("point_radius",0.0);assert_eq!(before,render(&p));
            let xml=roxmltree::Document::parse(&before).unwrap();assert!(xml.descendants().filter(|n|n.has_tag_name("circle")).all(|n|n.attribute("r").unwrap().parse::<f64>().unwrap()==radius));
        }
    }

    #[test]
    fn explicit_grid_can_stop_before_axes_and_given_points() {
        let mut p=defaults();
        p.set_s("axes_mode","Manual");p.set_f("xmin",-2.0);p.set_f("xmax",14.0);p.set_f("xstep",2.0);
        p.set_f("ymin",-1.0);p.set_f("ymax",9.0);p.set_f("ystep",1.0);
        p.set_s("grid_bounds_mode","Explicit");p.set_f("grid_xmin",0.0);p.set_f("grid_xmax",12.0);p.set_f("grid_ymin",0.0);p.set_f("grid_ymax",8.0);
        p.set_s("grid_directions","Both");p.set_s("grid_color","#777777");p.set_s("pts","13,8.5");p.set_b("fit",false);
        let svg=render(&p);let xml=roxmltree::Document::parse(&svg).unwrap();
        let val=|n:roxmltree::Node,k:&str|n.attribute(k).unwrap().parse::<f64>().unwrap();
        let grid:Vec<_>=xml.descendants().filter(|n|n.has_tag_name("line")&&n.attribute("stroke")==Some("#777777")).collect();
        assert_eq!(grid.len(),16,"seven vertical and nine horizontal lines, no added outer cell");
        let right=grid.iter().map(|n|val(*n,"x2")).fold(f64::NEG_INFINITY,f64::max);
        let top=grid.iter().map(|n|val(*n,"y1")).fold(f64::INFINITY,f64::min);
        let point=xml.descendants().find(|n|n.has_tag_name("circle")).unwrap();
        assert!(val(point,"cx")>right && val(point,"cy")<top,"a supplied point beyond the grid stays visible inside the axes");
        assert!(xml.descendants().filter(|n|n.has_tag_name("line")&&n.attribute("stroke")==Some(INK)).any(|n|val(n,"x2")>val(point,"cx")),"axis extends beyond the last gridline and point");
        assert!(!svg.contains("y = ")&&!svg.contains("r = "));
    }

    #[test]
    fn major_minor_grid_colours_preserve_lines_labels_and_data() {
        let mut p=defaults();p.set_s("axes_mode","Manual");p.set_f("xmin",-3.0);p.set_f("xmax",3.0);p.set_f("xstep",1.0);p.set_f("ymin",-0.3);p.set_f("ymax",0.3);p.set_f("ystep",0.1);
        p.set_s("grid_directions","Both");p.set_s("grid_color","#777777");p.set_s("minor_grid_color","#c7c7c7");p.set_b("fit",false);p.set_s("pts","-1,0.1\n2,-0.2");
        let before=render(&p);p.set_i("x_major_every",3);p.set_i("y_major_every",2);let after=render(&p);
        let bx=roxmltree::Document::parse(&before).unwrap();let ax=roxmltree::Document::parse(&after).unwrap();
        let texts=|x:&roxmltree::Document|x.descendants().filter(|n|n.has_tag_name("text")).map(|n|n.text().unwrap_or("").to_string()).collect::<Vec<_>>();
        assert_eq!(texts(&bx),texts(&ax),"shading does not change tick cadence or labels");
        let grids:Vec<_>=ax.descendants().filter(|n|n.has_tag_name("line")&&matches!(n.attribute("stroke"),Some("#777777"|"#c7c7c7"))).collect();
        assert_eq!(grids.len(),14);assert_eq!(grids.iter().filter(|n|n.attribute("stroke")==Some("#777777")).count(),6,"three vertical and three horizontal major lines, anchored at zero across negative/fractional values");
        let points=|x:&roxmltree::Document|x.descendants().filter(|n|n.has_tag_name("circle")).map(|n|(n.attribute("cx").unwrap().to_string(),n.attribute("cy").unwrap().to_string())).collect::<Vec<_>>();
        assert_eq!(points(&bx),points(&ax));
    }

    #[test]
    fn inactive_grid_overrides_leave_legacy_svg_unchanged() {
        let mut p=defaults();let before=render(&p);
        p.set_f("grid_xmin",123.0);p.set_f("grid_xmax",456.0);p.set_f("grid_ymin",10.0);p.set_f("grid_ymax",20.0);p.set_s("minor_grid_color","#000000");
        assert_eq!(render(&p),before,"Follow axes and every1 retain the exact previous render path");
        p.set_s("grid_bounds_mode","Explicit");p.set_f("grid_xmin",20.0);p.set_f("grid_xmax",10.0);
        assert!(render(&p).contains("explicit grid rectangle"),"reversed new controls are reported, not silently substituted");
    }

    #[test]
    fn axis_arrows_and_end_letters_are_drawn_only_when_asked() {
        // Holdout v5 (STAAR hotel costs): arrowed axes with x and y at their ends, plus centred titles.
        let mut p=defaults();
        let plain=render(&p);
        p.set_b("axis_arrows",false);p.set_s("x_end_label","");p.set_s("y_end_label","");
        assert_eq!(render(&p),plain,"the defaults keep the previous render");
        p.set_s("axes_mode","Manual");p.set_f("xmin",0.0);p.set_f("xmax",7.0);p.set_f("xstep",1.0);p.set_f("ymin",0.0);p.set_f("ymax",700.0);p.set_f("ystep",100.0);
        let before=render(&p);
        p.set_b("axis_arrows",true);p.set_s("x_end_label","x");p.set_s("y_end_label","y");
        let svg=render(&p);
        let xml=roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(xml.descendants().filter(|n|n.has_tag_name("polygon")&&n.attribute("fill")==Some(INK)).count(),2,"one head per axis");
        let texts:Vec<_>=xml.descendants().filter(|n|n.has_tag_name("text")).filter_map(|n|n.text()).collect();
        assert!(texts.contains(&"x")&&texts.contains(&"y"),"{texts:?}");
        assert_ne!(svg,before);
        // Boxed axes keep their ends too (the STAAR graph is boxed); no axes have none.
        p.set_s("axis_style","Box");
        let boxed=render(&p);
        assert!(boxed.contains(">x<")&&roxmltree::Document::parse(&boxed).unwrap().descendants().filter(|n|n.has_tag_name("polygon")&&n.attribute("fill")==Some(INK)).count()==2);
        p.set_s("axis_style","None");
        let none=render(&p);
        assert!(!none.contains(">x<")&&roxmltree::Document::parse(&none).unwrap().descendants().filter(|n|n.has_tag_name("polygon")&&n.attribute("fill")==Some(INK)).count()==0);
    }

    #[test]
    fn axis_stroke_extensions_do_not_expand_the_grid_or_move_data_relative_to_it() {
        let mut p=defaults();p.set_s("axes_mode","Manual");p.set_f("xmin",0.0);p.set_f("xmax",24.0);p.set_f("xstep",3.0);p.set_f("ymin",0.0);p.set_f("ymax",8.0);p.set_f("ystep",2.0);p.set_s("grid_directions","Both");p.set_s("grid_color","#777777");p.set_s("pts","21,6");p.set_b("fit",false);p.set_b("tick_marks",false);
        let old=render(&p);p.set_f("axis_x_extension",0.75);p.set_f("axis_y_extension",0.5);let new=render(&p);
        let oldx=roxmltree::Document::parse(&old).unwrap();let newx=roxmltree::Document::parse(&new).unwrap();
        let grid=|x:&roxmltree::Document|x.descendants().filter(|n|n.has_tag_name("line")&&n.attribute("stroke")==Some("#777777")).count();
        assert_eq!(grid(&oldx),14);assert_eq!(grid(&newx),14,"axis extensions do not create another cell or gridline");
        let num=|n:roxmltree::Node,k:&str|n.attribute(k).unwrap().parse::<f64>().unwrap();
        let point=|x:&roxmltree::Document|{let n=x.descendants().find(|n|n.has_tag_name("circle")).unwrap();(num(n,"cx"),num(n,"cy"))};
        let a=point(&oldx);let b=point(&newx);assert_eq!(a.0,b.0);
        let lower=|x:&roxmltree::Document|x.descendants().filter(|n|n.has_tag_name("line")&&n.attribute("stroke")==Some("#777777")).map(|n|num(n,"y2")).fold(f64::NEG_INFINITY,f64::max);
        assert!(((lower(&newx)-b.1)-(lower(&oldx)-a.1)).abs()<0.011,"data mapping relative to the grid is unchanged");
        let right=newx.descendants().filter(|n|n.has_tag_name("line")&&n.attribute("stroke")==Some("#777777")).map(|n|num(n,"x2")).fold(f64::NEG_INFINITY,f64::max);
        assert!(newx.descendants().filter(|n|n.has_tag_name("line")&&n.attribute("stroke")==Some(INK)).any(|n|num(n,"x2")>right));
        let defs=params();let sections=crate::figures::layout::sections("scatterplot",&defs,None);
        for key in ["axis_x_extension","axis_y_extension","grid_bounds_mode","grid_xmin","grid_xmax","grid_ymin","grid_ymax","x_major_every","y_major_every","minor_grid_color"] {assert!(sections.iter().any(|s|s.items.iter().any(|i|defs[*i].key==key)),"Figures tab exposes {key}");}
    }

    #[test]
    fn manual_million_axes_keep_exact_tick_labels_full_grid_and_plain_points() {
        let mut p=defaults();p.set_s("axes_mode","Manual");p.set_f("xmin",0.0);p.set_f("xmax",100.0);p.set_f("xstep",10.0);p.set_f("ymin",0.0);p.set_f("ymax",5_000_000.0);p.set_f("ystep",500_000.0);p.set_s("x_ticks","20,40,60,80");p.set_s("y_ticks","1000000,2000000,3000000,4000000");p.set_s("pts","20,1000000\n80,4000000");p.set_s("x_title","");p.set_s("y_title","");p.set_b("fit",false);p.set_b("zero_labels",false);p.set_s("grid_directions","Both");p.set_s("grid_color","#777777");p.set_b("point_outline",false);p.set_s("point_color","#0000ff");p.set_b("tick_marks",false);p.set_b("square_plot",true);
        let s=render(&p);let xml=roxmltree::Document::parse(&s).unwrap();let ts:Vec<_>=xml.descendants().filter(|n|n.has_tag_name("text")).map(|n|n.text().unwrap()).collect();
        assert_eq!(ts,vec!["20","40","60","80","1000000","2000000","3000000","4000000"]);
        let lines:Vec<_>=xml.descendants().filter(|n|n.has_tag_name("line")&&n.attribute("stroke")==Some("#777777")).collect();assert_eq!(lines.len(),22);
        assert!(lines.iter().any(|n|n.attribute("x1")==n.attribute("x2")));assert!(lines.iter().any(|n|n.attribute("y1")==n.attribute("y2")));
        let pts:Vec<_>=xml.descendants().filter(|n|n.has_tag_name("circle")).collect();assert_eq!(pts.len(),2);for p in pts{assert_eq!(p.attribute("fill"),Some("#0000ff"));assert_eq!(p.attribute("stroke"),Some("none"));}
        assert!(!s.contains("y = ")&&!s.contains("r = "));
    }

    #[test]
    fn manual_blank_grid_has_no_synthetic_points_or_answers() {
        let mut p=defaults();p.set_s("axes_mode","Manual");p.set_s("pts","");p.set_s("x_title","");p.set_s("y_title","");p.set_b("zero_labels",false);let s=render(&p);assert!(!s.contains("<circle")&&!s.contains("Enter some"));p.set_f("xmax",p.f("xmin"));assert!(render(&p).contains("Manual axes need"));
    }

    #[test]
    fn fit_recovers_an_exact_line_and_its_correlation() {
        let f = fit(&[(0.0, 1.0), (1.0, 3.0), (2.0, 5.0), (3.0, 7.0)]).unwrap();
        assert!((f.slope - 2.0).abs() < 1e-12 && (f.intercept - 1.0).abs() < 1e-12);
        assert_eq!(f.r, Some(1.0));
        let neg = fit(&[(0.0, 9.0), (1.0, 6.0), (2.0, 3.0)]).unwrap();
        assert_eq!(neg.r, Some(-1.0));
    }

    #[test]
    fn fit_refuses_what_has_no_line() {
        assert!(fit(&[]).is_none() && fit(&[(1.0, 1.0)]).is_none());
        assert!(fit(&[(2.0, 1.0), (2.0, 5.0)]).is_none(), "a vertical set has no slope");
        let flat = fit(&[(0.0, 4.0), (1.0, 4.0), (2.0, 4.0)]).unwrap();
        assert_eq!((flat.slope, flat.r), (0.0, None), "a flat set has a line but no r");
        assert!(fit(&[(0.0, 0.0), (1e300, 1e300), (2e300, -1e300)]).is_none(), "overflow is not a line");
    }

    #[test]
    fn fit_matches_the_generator() {
        let pts = data::generate_pairs(500, 5, 10.0, 3.0, 2.5, 4.0, 1.0, 3);
        let f = fit(&pts).unwrap();
        assert!((f.slope - 2.5).abs() < 0.1 && f.r.unwrap() > 0.95, "{f:?}");
    }

    #[test]
    fn equations_read_naturally() {
        let eq = |m, b| equation(&Fit { slope: m, intercept: b, r: None });
        assert_eq!(eq(2.0, 1.0), "y = 2x + 1");
        assert_eq!(eq(1.0, -3.5), "y = x \u{2212} 3.5");
        assert_eq!(eq(-1.0, 0.0), "y = \u{2212}x");
        assert_eq!(eq(0.0, 4.25), "y = 4.25");
        assert_eq!(eq(0.12345, 10.0), "y = 0.123x + 10");
    }

    #[test]
    fn axis_ranges_have_air_around_the_data() {
        let (a, b, step) = axis_range(1.0, 9.0);
        assert!(a < 1.0 && b > 9.0 && step > 0.0);
        let (a, b, _) = axis_range(5.0, 5.0);
        assert!(a < 5.0 && b > 5.0, "no spread still has a range");
        let (a, b, _) = axis_range(0.0, 10.0);
        assert!(a < 0.0 && b > 10.0, "points on a tick are not on the frame");
    }

    #[test]
    fn clipping_keeps_the_line_inside_the_box() {
        let ((xa, ya), (xb, yb)) = clip(1.0, 0.0, 0.0, 10.0, 2.0, 8.0).unwrap();
        assert_eq!((xa, ya, xb, yb), (2.0, 2.0, 8.0, 8.0));
        assert!(clip(0.0, 20.0, 0.0, 10.0, 0.0, 10.0).is_none(), "a flat line above the box");
        assert!(clip(1.0, 50.0, 0.0, 10.0, 0.0, 10.0).is_none(), "a line that misses the box");
        assert_eq!(clip(0.0, 5.0, 0.0, 10.0, 0.0, 10.0).unwrap(), ((0.0, 5.0), (10.0, 5.0)));
    }

    #[test]
    fn default_draws_every_point() {
        let p = defaults();
        let svg = render(&p);
        assert_eq!(svg.matches("<circle").count(), data::pairs(p.s("pts")).len());
        assert!(svg.contains("stroke-width=\"2.6\""), "the line of best fit");
    }

    #[test]
    fn student_copy_keeps_the_points_and_hides_the_answers() {
        let def = def();
        let mut p = defaults();
        p.set_b("eq", true);
        p.set_b("rval", true);
        let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
        let points = data::pairs(p.s("pts")).len();
        assert_eq!(student.matches("<circle").count(), points);
        assert!(!student.contains("stroke-width=\"2.6\"") && !student.contains("r = ") && !student.contains("y = "));
        assert!(key.contains("stroke-width=\"2.6\"") && key.contains("r = 0.") && key.contains("y = "));
    }

    #[test]
    fn print_style_is_grey_only_with_a_dashed_line() {
        let mut p = defaults();
        p.set_s("style", "Print");
        let svg = render(&p);
        assert!(!svg.contains("#4f7cd4") && !svg.contains("#e0803c"), "no series colours");
        assert!(svg.contains("stroke-dasharray"), "the line is told apart by its dashes");
        let png = crate::figures::to_png(&svg, 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        // the ink is a near-black with a slight tint; a leaked series colour is far further from grey
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24), "no colour, grey and ink only");
    }

    #[test]
    fn odd_input_never_panics() {
        let mut p = defaults();
        for text in ["", "abc", "1", "1, 2", "5, 5\n5, 5", "1, 2\n1, 9", "-1e308, 1e308\n1e308, -1e308", "1e300, 1\n2e300, 2", "0, 0\n0.0001, 1000000"] {
            p.set_s("pts", text);
            for answers in [true, false] {
                p.set_b(SHOW_ANSWERS, answers);
                p.set_b("eq", true);
                p.set_b("rval", true);
                let svg = render(&p);
                assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{text:?}");
                assert!(!svg.contains("NaN") && !svg.contains("inf"), "{text:?}");
            }
        }
        p.set_s("pts", "");
        assert!(render(&p).contains("Enter some"));
    }

    #[test]
    fn notes_move_away_from_a_falling_line() {
        let mut p = defaults();
        p.set_b("eq", true);
        let plate_x = |svg: &str| -> f64 {
            // the plate is the white rect with the grid-coloured stroke
            let i = svg.find(r##"fill="#fff" stroke="#dfe3ea""##).expect("a plate");
            let head = &svg[..i];
            let x = &head[head.rfind("<rect x=\"").unwrap() + 9..];
            x[..x.find('"').unwrap()].parse().unwrap()
        };
        let rising = plate_x(&render(&p));
        p.set_s("pts", "1, 9\n2, 7\n3, 6\n4, 3\n5, 2");
        let falling = plate_x(&render(&p));
        assert!(rising < 200.0 && falling > 320.0, "rising {rising}, falling {falling}");
    }

    #[test]
    fn eq_only_matters_with_the_line() {
        let mut p = defaults();
        assert!(relevant("scatterplot", "eq", &p));
        p.set_b("fit", false);
        assert!(!relevant("scatterplot", "eq", &p) && relevant("scatterplot", "rval", &p));
    }

    #[test]
    fn examples_build_and_render() {
        for e in examples("scatterplot").unwrap() {
            let mut p = defaults();
            for (k, v) in e.params {
                p.0.insert(k.to_string(), v);
            }
            assert!(render(&p).contains("<circle"), "{}", e.name);
        }
    }
}
