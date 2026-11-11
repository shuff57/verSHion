//! Tests of the number line's batch 7 controls (`number_line_b7.rs`): sequences and their n range,
//! values above the line, dot or cross, point labels, the range in π. The crowded sweeps read the drawn
//! SVG (text boxes measured as drawn, the dots, crosses and lines), not the plan.
use super::number_line_b7::*;
use crate::figures::{interact, registry, start_params, FigureDef, Params, Value};
use std::f64::consts::PI;

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "number-line").unwrap() }
fn with(kv: &[(&str, Value)]) -> Params {
    let d = def();
    let mut p = start_params(&d);
    for (k, v) in kv { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn s(t: &str) -> Value { Value::S(t.into()) }
fn f(v: f64) -> Value { Value::F(v) }
fn i(v: i64) -> Value { Value::I(v) }
fn svg(p: &Params) -> String { def().svg(p) }

/// An old row the figure now reads as a sequence.
pub(super) fn seq_row(t: &str) -> bool { matches!(read_row(&with(&[("inequality", s(t))]), 0), Row::Seq(_) | Row::Bad(_)) }

// ───────────── reading the drawn SVG ─────────────

fn attr(tag: &str, k: &str) -> Option<String> {
    let pat = format!(" {k}=\"");
    let i = tag.find(&pat)? + pat.len();
    Some(tag[i..].split('"').next()?.to_string())
}
fn num(tag: &str, k: &str) -> f64 { attr(tag, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
/// A text element as drawn: its box (ascent 0.74, descent 0.22 of each run's size) and its words.
#[derive(Debug, Clone)]
struct T { l: f64, t: f64, r: f64, b: f64, s: String }
fn texts(svg: &str) -> Vec<T> {
    let mut out = vec![];
    let mut rest = svg;
    while let Some(i) = rest.find("<text ") {
        let tail = &rest[i..];
        let end = tail.find("</text>").unwrap();
        let open_end = tail.find('>').unwrap();
        let tag = &tail[..open_end];
        let inner = &tail[open_end + 1..end];
        let (x, y, size) = (num(tag, "x"), num(tag, "y"), num(tag, "font-size"));
        let anchor = attr(tag, "text-anchor").unwrap_or_else(|| "start".into());
        let serif = tag.contains("Serif");
        let bold = tag.contains("font-weight=\"bold\"");
        // runs: (text, size, serif)
        let mut runs: Vec<(String, f64, bool)> = vec![];
        if inner.contains("<tspan") {
            for part in inner.split("<tspan").skip(1) {
                let (ttag, body) = part.split_once('>').unwrap();
                let body = body.split("</tspan>").next().unwrap();
                let sz = if ttag.contains("font-size") { num(ttag, "font-size") } else { size };
                runs.push((body.to_string(), sz, serif && !ttag.contains("DejaVu")));
            }
        } else { runs.push((inner.to_string(), size, serif)); }
        let unesc = |t: &str| t.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
        let w: f64 = runs.iter().map(|(t, sz, se)| if *se { t.chars().count() as f64 * sz * 0.7 } else { sans_w(&unesc(t), *sz, bold) }).sum();
        let big = runs.iter().map(|r| r.1).fold(0.0, f64::max);
        let l = match anchor.as_str() { "middle" => x - w / 2.0, "end" => x - w, _ => x };
        out.push(T { l, t: y - 0.74 * big, r: l + w, b: y + 0.22 * big, s: runs.iter().map(|r| unesc(&r.0)).collect() });
        rest = &tail[end..];
    }
    out
}
/// Dots (cx, cy, r) and crosses (centre, half size) as drawn.
fn dots(svg: &str) -> Vec<(f64, f64, f64)> {
    svg.split("<circle").skip(1).map(|c| (num(c, "cx"), num(c, "cy"), num(c, "r") + num(c, "stroke-width") / 2.0)).collect()
}
fn lines(svg: &str) -> Vec<(f64, f64, f64, f64, f64)> {
    svg.split("<line").skip(1).map(|c| (num(c, "x1"), num(c, "y1"), num(c, "x2"), num(c, "y2"), num(c, "stroke-width"))).collect()
}
fn crosses(svg: &str) -> Vec<(f64, f64)> {
    lines(svg).into_iter().filter(|l| l.4 == 2.6 && l.1 < l.3).map(|l| ((l.0 + l.2) / 2.0, (l.1 + l.3) / 2.0)).collect()
}
fn size(svg: &str) -> (f64, f64) { crate::figures::svg_size(svg) }
fn overlap(a: &T, b: &T, pad: f64) -> bool { a.l < b.r + pad && b.l < a.r + pad && a.t < b.b + pad && b.t < a.b + pad }
fn hits_box(t: &T, l: f64, top: f64, r: f64, b: f64) -> bool { t.l < r && l < t.r && t.t < b && top < t.b }

/// Every rule a checker reads off the drawing: no text on text (a stacked fraction's own parts and
/// the old tick numbers, which may crowd at Large as they always have, apart), none on a dot, cross
/// or line, all of it inside the page.
fn check_drawn(svg: &str, what: &str) {
    let (w, h) = size(svg);
    let ts = texts(svg);
    for t in &ts {
        assert!(t.l >= -0.5 && t.r <= w + 0.5 && t.t >= -0.5 && t.b <= h + 0.5, "{what}: text {:?} leaves the {w} x {h} page", t);
    }
    let ys: Vec<f64> = dots(svg).iter().map(|d| d.1).chain(crosses(svg).iter().map(|c| c.1)).collect();
    let axis = lines(svg).iter().find(|l| l.4 == 2.4 && (l.1 - l.3).abs() < 1e-9).map(|l| l.1).unwrap();
    // the new labels are the texts above the axis that are not old point names (serif, 17 high)
    let above: Vec<&T> = ts.iter().filter(|t| t.b < axis).collect();
    for (a, x) in above.iter().enumerate() {
        for y in above.iter().skip(a + 1) {
            // a fraction's numerator, bar and denominator touch nothing but each other
            let same_stack = (x.l + x.r - y.l - y.r).abs() < 1.0 && (x.t - y.t).abs() > 4.0 && x.s.chars().all(|c| c.is_ascii_digit() || c == '\u{3c0}') && y.s.chars().all(|c| c.is_ascii_digit() || c == '\u{3c0}');
            let sign_of_stack = (x.s == "\u{2212}" || y.s == "\u{2212}") && ((x.r - y.l).abs() < 4.0 || (y.r - x.l).abs() < 4.0);
            if same_stack || sign_of_stack { continue; }
            assert!(!overlap(x, y, 0.5), "{what}: {:?} on {:?}", x, y);
        }
        for d in dots(svg) {
            assert!(!hits_box(x, d.0 - d.2, d.1 - d.2, d.0 + d.2, d.1 + d.2), "{what}: text {:?} on a dot at {:?}", x, d);
        }
        for c in crosses(svg) {
            assert!(!hits_box(x, c.0 - 6.8, c.1 - 6.8, c.0 + 6.8, c.1 + 6.8), "{what}: text {:?} on a cross at {:?}", x, c);
        }
        for l in lines(svg).iter().filter(|l| l.4 >= 2.0) {
            let (hw, l0, r0, t0, b0) = (l.4 / 2.0, l.0.min(l.2), l.0.max(l.2), l.1.min(l.3), l.1.max(l.3));
            assert!(!hits_box(x, l0 - hw, t0 - hw, r0 + hw, b0 + hw), "{what}: text {:?} on a line {:?}", x, l);
        }
    }
    let _ = ys;
}

// ───────────── reading rows: old rows mean what they meant (rule 6e) ─────────────

#[test]
fn number_line_old_rows_read_as_before() {
    let old = |t: &str| super::nl_intervals(t);
    let mut became_seq = vec![];
    for r in super::number_line_frozen::OLD_ROWS {
        let p = with(&[("inequality", s(r))]);
        match read_row(&p, 0) {
            Row::Ineq(ivs) => {
                let o = old(r);
                assert_eq!(ivs.len(), o.len(), "{r}");
                for (a, b) in ivs.iter().zip(o.iter()) {
                    assert!(a.lo.to_bits() == b.lo.to_bits() && a.hi.to_bits() == b.hi.to_bits() && a.lo_c == b.lo_c && a.hi_c == b.hi_c, "{r}: read differently");
                }
            }
            Row::Seq(_) | Row::Bad(_) => { assert!(old(r).is_empty(), "{r}: an inequality the old figure drew is now a sequence"); became_seq.push(r); }
        }
    }
    // only the sequence forms, which the old figure drew nothing for
    assert_eq!(became_seq, vec!["a_n = 1/n", "a\u{2099} = 1/n", "x_n = n^2"]);
}

// ───────────── sequences (cap-01f, cap-05) ─────────────

fn terms(t: &str, a: i64, b: i64) -> Vec<(i64, f64)> {
    match read_row(&with(&[("inequality", s(t)), ("n_from", i(a)), ("n_to", i(b))]), 0) { Row::Seq(q) => q.terms, _ => panic!("{t}: not a sequence") }
}

#[test]
fn number_line_sequence_terms_are_right_and_drawn_where_they_are() {
    // checked by hand: 1/n, (-1)^n/n, n^2 - 3, 2^-n, (n+1)/n
    let close = |got: Vec<(i64, f64)>, want: &[(i64, f64)]| {
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(want) { assert!(g.0 == w.0 && (g.1 - w.1).abs() < 1e-12, "{g:?} vs {w:?}"); }
    };
    close(terms("a_n = 1/n", 1, 5), &[(1, 1.0), (2, 0.5), (3, 1.0 / 3.0), (4, 0.25), (5, 0.2)]);
    close(terms("a\u{2099} = (-1)^n/n", 1, 4), &[(1, -1.0), (2, 0.5), (3, -1.0 / 3.0), (4, 0.25)]);
    close(terms("b(n) = n^2 \u{2212} 3", 0, 3), &[(0, -3.0), (1, -2.0), (2, 1.0), (3, 6.0)]);
    close(terms("c_{n} = 2^(-n)", 1, 3), &[(1, 0.5), (2, 0.25), (3, 0.125)]);
    close(terms("a_n = (n+1)/n", 1, 2), &[(1, 2.0), (2, 1.5)]);
    close(terms("a_n = sin(n pi/2)", 1, 2), &[(1, 1.0), (2, (PI).sin())]);
    // last n before first n: the same terms
    close(terms("a_n = 1/n", 5, 3), &[(3, 1.0 / 3.0), (4, 0.25), (5, 0.2)]);
    // drawn: one dot per term on the line, at the term's place
    let p = with(&[("min", f(0.0)), ("max", f(1.0)), ("step", f(0.1)), ("inequality", s("a_n = 1/n")), ("n_from", i(1)), ("n_to", i(5))]);
    let out = svg(&p);
    let (_, _, x0, x1, y) = super::nl_geom(&p);
    let ds = dots(&out);
    assert_eq!(ds.len(), 5);
    for (n, d) in (1..=5).zip(&ds) {
        let x = x0 + (1.0 / n as f64) * (x1 - x0);
        assert!((d.0 - x).abs() < 0.01 && (d.1 - y).abs() < 0.01, "a{n} drawn at {:?}, not at x = {x}", d);
    }
}

#[test]
fn number_line_sequence_problems_give_notes_not_wrong_points() {
    let warn = |kv: &[(&str, Value)]| crate::figures::notes_for("number-line", &with(kv)).join(" | ");
    // divides by zero at n = 0: no point there, a note
    let p = with(&[("min", f(0.0)), ("max", f(1.0)), ("inequality", s("a_n = 1/n")), ("n_from", i(0)), ("n_to", i(3))]);
    let out = svg(&p);
    assert_eq!(dots(&out).len(), 3);
    assert!(!out.contains("NaN") && !out.contains("inf"));
    assert!(warn(&[("min", f(0.0)), ("max", f(1.0)), ("inequality", s("a_n = 1/n")), ("n_from", i(0)), ("n_to", i(3))]).contains("no value at n = 0"));
    // leaves the range: the terms on the line are drawn, a note says how many are off
    let kv = [("min", f(-10.0)), ("max", f(10.0)), ("inequality", s("a_n = 2^n")), ("n_from", i(1)), ("n_to", i(6))];
    assert_eq!(dots(&svg(&with(&kv))).len(), 3);
    assert!(warn(&kv).contains("3 of the 6 terms are off the line"), "{}", warn(&kv));
    // typed wrongly: forty sequences a teacher might type; never a crash, never a non-number in the drawing,
    // a note in the panel and in the figure for one the figure cannot read
    let tries = [
        "a_n = 1/k", "a_n = x + 1", "a_n =", "a_n = 1/n)", "a_n = ((n)", "a_n = sqrt(n - 3)", "a_n = ln(n)", "a_n = 10^n", "a_n = n!", "a_n = 1e400",
        "a_n = 1/0", "a_n = 0/0", "a_n = n/(n-2)", "a_n = tan(n)", "a_n = (-1)^n", "a_n = (-2)^(1/n)", "a_n = n < 3", "a_n = 1/n = 2", "a_n = y", "a_n = nn",
        "a_n = 2n + 1", "a_n = -n", "a_n = n^n", "a_n = log(n)", "a_n = |n - 3|", "a_n = pi n", "a_n = e^(-n)", "a_n = 1/n^2", "a_n = n/2", "a_n = abc",
        "a_n = 1,2", "a_n = n%2", "a_n = 3", "ab_n = 1/n", "_n = 1/n", "a_n 1/n", "aₙ = 1 / n", "A(n) = n", "a_n = 1 - 1/n", "a_n = sqrt(-n)",
    ];
    assert_eq!(tries.len(), 40);
    for t in tries {
        for (a, b) in [(0, 5), (1, 1000), (1000, 1000)] {
            let kv = [("inequality", s(t)), ("n_from", i(a)), ("n_to", i(b)), ("ineq_values", s("Write"))];
            let p = with(&kv);
            let out = svg(&p);
            assert!(!out.contains("NaN") && !out.contains("inf"), "{t}: a coordinate is not a number");
            for d in dots(&out) { assert!(d.0.is_finite() && d.1.is_finite()); }
            assert!(out.len() < 400_000, "{t}: {} bytes", out.len());
            let (w, h) = size(&out);
            for x in texts(&out) { assert!(x.l >= 0.0 && x.r <= w && x.t >= 0.0 && x.b <= h, "{t}: {:?} leaves the page", x.s); }
            match read_row(&p, 0) {
                Row::Bad(m) => { assert!(warn(&kv).contains(&m) && out.contains("graph:"), "{t}: no note"); }
                Row::Seq(q) => {
                    // a defined term is the formula's own value: never a point for an undefined one
                    for (n, v) in &q.terms { assert!(v.is_finite(), "{t}: a{n}"); }
                    if !q.undefined.is_empty() { assert!(warn(&kv).contains("no value"), "{t}"); }
                }
                Row::Ineq(ivs) => assert!(ivs.is_empty(), "{t}: read as an inequality"),
            }
        }
    }
    // which of them cannot be read at all (a letter other than n, a broken formula)
    for t in ["a_n = 1/k", "a_n = x + 1", "a_n =", "a_n = 1/n)", "a_n = y", "a_n = abc", "a_n = n < 3", "a_n = 1,2", "a_n = n!"] {
        assert!(matches!(read_row(&with(&[("inequality", s(t))]), 0), Row::Bad(_)), "{t}");
    }
}

#[test]
fn number_line_n_range_is_clamped_and_bounded() {
    let d = def();
    let pd = (d.params)();
    for (k, lo, hi) in [("n_from", 0, 1000), ("n_to", 0, 1000), ("n2_from", 0, 1000), ("n2_to", 0, 1000)] {
        let p = with(&[(k, i(-5_000_000))]);
        assert_eq!(p.i(k), lo);
        let p = with(&[(k, i(5_000_000))]);
        assert_eq!(p.i(k), hi);
        assert!(pd.iter().any(|q| q.key == k));
    }
    // the most terms the boxes allow, both rows, values on, Large: drawn, bounded, clean numbers
    let p = with(&[("min", f(0.0)), ("max", f(1.0)), ("step", f(0.05)), ("inequality", s("a_n = 1/n")), ("n_from", i(0)), ("n_to", i(1000)), ("ineq_values", s("Write")),
        ("inequality2", s("b_n = 1 - 1/n")), ("n2_from", i(0)), ("n2_to", i(1000)), ("ineq2_values", s("Write")), ("label_size", s("Large"))]);
    let out = svg(&p);
    assert!(out.len() < 600_000, "{} bytes", out.len());
    assert!(dots(&out).len() <= 2002);
    check_drawn(&out, "1000 terms of 1/n and 1 - 1/n");
    for t in texts(&out) { assert!(!t.s.contains("0000") && !t.s.contains("9999"), "float noise in {:?}", t.s); }
}

// ───────────── values above the line (cap-02c) ─────────────

#[test]
fn number_line_endpoint_values_are_written_off_the_numbered_ticks() {
    let txt = |kv: &[(&str, Value)]| texts(&svg(&with(kv))).into_iter().map(|t| t.s).collect::<Vec<_>>();
    let on = txt(&[("inequality", s("-2.5 < x <= 5.3")), ("ineq_values", s("Write"))]);
    assert!(on.contains(&"\u{2212}2.5".to_string()) && on.contains(&"5.3".to_string()), "{on:?}");
    // on numbered ticks nothing extra (the tick numbers say it)
    let base = txt(&[("inequality", s("-2 < x <= 5"))]);
    assert_eq!(txt(&[("inequality", s("-2 < x <= 5")), ("ineq_values", s("Write"))]), base);
    // None (the default, as old figures drew) writes nothing
    assert_eq!(svg(&with(&[("inequality", s("-2.5 < x <= 5.3"))])), svg(&with(&[("inequality", s("-2.5 < x <= 5.3")), ("ineq_values", s("None"))])));
    // the value is the one drawn: its text sits right over its circle
    let p = with(&[("inequality", s("x > 1/3")), ("ineq_values", s("Write"))]);
    let out = svg(&p);
    let c = dots(&out)[0];
    let (_, _, x0, x1, _) = super::nl_geom(&p);
    assert!((c.0 - (x0 + (1.0 / 3.0 + 10.0) / 20.0 * (x1 - x0))).abs() < 0.01);
    let ts = texts(&out);
    let num = ts.iter().find(|t| t.s == "1" && t.b < c.1).unwrap();
    let den = ts.iter().find(|t| t.s == "3" && t.b < c.1).unwrap();
    assert!(((num.l + num.r) / 2.0 - c.0).abs() < 2.0 && ((den.l + den.r) / 2.0 - c.0).abs() < 2.0 && den.b < c.1 - c.2, "1/3 over its circle: {num:?} {den:?} {c:?}");
    // the second row's values too, over the second row
    let p = with(&[("inequality", s("x > 3")), ("inequality2", s("x < -7.25")), ("ineq2_values", s("Write"))]);
    let out = svg(&p);
    let t = texts(&out).into_iter().find(|t| t.s == "\u{2212}7.25").expect("second row's value");
    let ring = dots(&out).into_iter().find(|d| d.1 < super::nl_geom(&p).4 - 1.0).unwrap();
    assert!(t.b < ring.1 - ring.2 && ((t.l + t.r) / 2.0 - ring.0).abs() < 1.0);
}

// ───────────── dot or cross (cap-02d), point labels (cap-02e) ─────────────

#[test]
fn number_line_every_new_option_draws_differently() {
    let cases: Vec<(Vec<(&str, Value)>, &str, &[&str])> = vec![
        (vec![("points", s("3, -4, P(2.5)"))], "pt_mark", &MARKS),
        (vec![("points", s("P(3/4), Q(-2)"))], "pt_labels", &LABELS),
        (vec![("inequality", s("-2.5 < x <= 5.3"))], "ineq_values", &VALUES),
        (vec![("inequality", s("x > 3")), ("inequality2", s("-1.5 < x < 0.5"))], "ineq2_values", &VALUES),
        (vec![("inequality", s("a_n = 1/n")), ("min", f(0.0)), ("max", f(1.0)), ("step", f(0.1))], "ineq_mark", &MARKS),
        (vec![("inequality", s("x > 3")), ("inequality2", s("b_n = n"))], "ineq2_mark", &MARKS),
        (vec![("inequality", s("a_n = 1/n")), ("min", f(0.0)), ("max", f(1.0)), ("step", f(0.1))], "ineq_values", &VALUES),
    ];
    for (base, key, opts) in cases {
        let outs: Vec<String> = opts.iter().map(|o| { let mut kv = base.clone(); kv.push((key, s(o))); svg(&with(&kv)) }).collect();
        for a in 0..outs.len() { for b in a + 1..outs.len() { assert_ne!(outs[a], outs[b], "{key}: {} draws as {}", opts[a], opts[b]); } }
    }
    for (key, vals) in [("n_from", [1, 3]), ("n_to", [5, 8]), ("n2_from", [1, 3]), ("n2_to", [5, 8])] {
        let row = if key.starts_with("n2") { "inequality2" } else { "inequality" };
        let a = svg(&with(&[(row, s("a_n = n")), (key, i(vals[0]))]));
        let b = svg(&with(&[(row, s("a_n = n")), (key, i(vals[1]))]));
        assert_ne!(a, b, "{key}");
    }
    for (key, v) in [("min_text", "-2pi"), ("max_text", "3/2")] { assert_ne!(svg(&with(&[(key, s(v))])), svg(&with(&[]))); }
    // a cross is a cross, and an open point stays an open dot
    let out = svg(&with(&[("points", s("3, -4, 5o")), ("pt_mark", s("Cross"))]));
    assert_eq!(crosses(&out).len(), 2);
    assert_eq!(dots(&out).len(), 1);
    // Letter and value: the letter in italics and the value as typed
    let out = svg(&with(&[("points", s("P(3/4), Q(-2), R(pi/2)")), ("pt_labels", s(LABELS[1]))]));
    let ts: Vec<String> = texts(&out).into_iter().map(|t| t.s).collect();
    for want in ["P(3/4)", "Q(\u{2212}2)", "R(\u{3c0}/2)"] { assert!(ts.contains(&want.to_string()), "{want} in {ts:?}"); }
    assert!(out.contains(r#"<tspan font-style="italic">P</tspan>"#));
}

// ───────────── π (cap-03d) ─────────────

#[test]
fn number_line_pi_numbers_are_fractions_in_lowest_terms() {
    let words = |kv: &[(&str, Value)]| {
        let p = with(kv);
        let pl = plan(&p);
        let (a, b) = pl.pi.expect("counted in π");
        (pl.k0..=pl.k1).filter(|k| (k - pl.k0) % pl.every == 0).map(|k| pi_txt(k * a, b).plain()).collect::<Vec<_>>()
    };
    let m = "\u{2212}";
    assert_eq!(words(&[("min_text", s("-2pi")), ("max_text", s("2pi")), ("step_text", s("pi/2"))]),
        [format!("{m}2\u{3c0}"), format!("{m}3\u{3c0}/2"), format!("{m}\u{3c0}"), format!("{m}\u{3c0}/2"), "0".into(), "\u{3c0}/2".into(), "\u{3c0}".into(), "3\u{3c0}/2".into(), "2\u{3c0}".into()]);
    // 2pi/4 is π/2; 2π/3 steps reduce (4π/3, 2π)
    assert_eq!(words(&[("min", f(0.0)), ("max_text", s("pi")), ("step_text", s("2pi/4"))]), ["0", "\u{3c0}/2", "\u{3c0}"]);
    assert_eq!(words(&[("min", f(0.0)), ("max_text", s("2\u{3c0}")), ("step_text", s("2pi/3"))]), ["0", "2\u{3c0}/3", "4\u{3c0}/3", "2\u{3c0}"]);
    assert_eq!(words(&[("min_text", s("-pi")), ("max_text", s("pi")), ("step_text", s("pi/6")), ("label_every", i(2))]),
        [format!("{m}\u{3c0}"), format!("{m}2\u{3c0}/3"), format!("{m}\u{3c0}/3"), "0".into(), "\u{3c0}/3".into(), "2\u{3c0}/3".into(), "\u{3c0}".into()]);
    // the drawing writes them, stacked: −3π/2 is a minus, 3π over 2
    let out = svg(&with(&[("min_text", s("-2pi")), ("max_text", s("2pi")), ("step_text", s("pi/2"))]));
    let ts: Vec<String> = texts(&out).into_iter().map(|t| t.s).collect();
    for want in ["\u{2212}2\u{3c0}", "3\u{3c0}", "\u{3c0}", "2\u{3c0}", "0"] { assert!(ts.contains(&want.to_string()), "{want} in {ts:?}"); }
    assert!(!ts.iter().any(|t| t.contains('.')), "no decimals: {ts:?}");
    // the ticks stand at the multiples of π/2: the drawn tick under 3π/2 is 3π/2 along
    let p = with(&[("min_text", s("-2pi")), ("max_text", s("2pi")), ("step_text", s("pi/2"))]);
    let (_, _, x0, x1, y) = super::nl_geom(&p);
    let ticks: Vec<f64> = lines(&out).iter().filter(|l| l.4 == 2.0 && (l.1 - (y - 9.0)).abs() < 0.01).map(|l| l.0).collect();
    assert_eq!(ticks.len(), 9);
    assert!((ticks[7] - (x0 + (1.5 * PI + 2.0 * PI) / (4.0 * PI) * (x1 - x0))).abs() < 0.01);
    // thinned on a long line, the numbers step by whole turns of π, not 7π/6
    let long = words(&[("min_text", s("-20pi")), ("max_text", s("20pi")), ("step_text", s("pi/12")), ("label_size", s("Large"))]);
    assert_eq!(long.first().unwrap(), &format!("{m}20\u{3c0}"));
    assert!(long.iter().all(|w| w == "0" || w.ends_with('\u{3c0}') && w.trim_start_matches('\u{2212}').trim_end_matches('\u{3c0}').parse::<i64>().is_ok_and(|n| n % 5 == 0)), "{long:?}");
    // not a simple multiple of π: decimals as before, and a note
    let kv = [("step_text", s("pi/2 + 1"))];
    assert!(plan(&with(&kv)).pi.is_none());
    assert!(crate::figures::notes_for("number-line", &with(&kv)).iter().any(|n| n.contains("multiple of")));
}

#[test]
fn number_line_pi_numbers_never_overlap_at_any_range_and_size() {
    let mut n = 0;
    for (lo, hi) in [("-2pi", "2pi"), ("0", "pi/2"), ("-1000", "1000"), ("-20pi", "20pi"), ("-pi/4", "pi/4"), ("0", "100"), ("-3pi", "pi"), ("-7", "7")] {
        for st in ["pi/12", "pi/6", "pi/4", "pi/3", "pi/2", "2pi/3", "3pi/4", "pi", "2pi", "5pi/6"] {
            for size in ["Small", "Medium", "Large"] {
                for every in [1, 2] {
                    let p = with(&[("min_text", s(lo)), ("max_text", s(hi)), ("step_text", s(st)), ("label_size", s(size)), ("label_every", i(every))]);
                    let out = svg(&p);
                    assert!(out.len() < 3_000_000, "{lo}..{hi} by {st}: {} bytes", out.len());
                    let (w, h) = size_of(&out);
                    let ts = texts(&out);
                    let axis = super::nl_geom(&p).4;
                    let below: Vec<&T> = ts.iter().filter(|t| t.t > axis).collect();
                    // each number's parts belong together; the whole numbers apart are what must not touch
                    let groups = group_labels(&below);
                    for (a, x) in groups.iter().enumerate() {
                        assert!(x.0 >= 0.0 && x.1 <= w && x.3 <= h, "{lo}..{hi} by {st} at {size}: a number leaves the page");
                        for y in groups.iter().skip(a + 1) {
                            assert!(x.1 + 3.0 <= y.0 || y.1 + 3.0 <= x.0, "{lo}..{hi} by {st} at {size}: numbers {x:?} and {y:?} touch");
                        }
                    }
                    if plan(&p).pi.is_some() { assert!(!below.iter().any(|t| t.s.contains('.')), "{lo}..{hi} by {st}"); }
                    n += 1;
                }
            }
        }
    }
    assert_eq!(n, 480);
}
fn size_of(svg: &str) -> (f64, f64) { size(svg) }
/// The tick numbers below the line, each as one box (a stacked fraction's sign, numerator and
/// denominator together): (left, right, top, bottom).
fn group_labels(ts: &[&T]) -> Vec<(f64, f64, f64, f64)> {
    let mut v: Vec<(f64, f64, f64, f64)> = ts.iter().map(|t| (t.l, t.r, t.t, t.b)).collect();
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut out: Vec<(f64, f64, f64, f64)> = vec![];
    for b in v {
        // overlapping in x with the last group: the same number (a numerator over its denominator)
        // or a sign right beside its fraction
        if let Some(l) = out.last_mut() && (b.0 < l.1 - 0.5 && b.0 >= l.0 - 0.5 || b.0 - l.1 < 1.5 && (b.2 - l.2).abs() > 3.0) {
            *l = (l.0.min(b.0), l.1.max(b.1), l.2.min(b.2), l.3.max(b.3));
            continue;
        }
        out.push(b);
    }
    out
}

#[test]
fn number_line_typed_range_is_clamped_and_read() {
    assert_eq!(range(&with(&[("min_text", s("-2pi")), ("max_text", s("2\u{3c0}"))])), (-2.0 * PI, 2.0 * PI));
    assert_eq!(range(&with(&[("min_text", s("-3/2")), ("max_text", s("1 1/2"))])), (-1.5, 1.5));
    assert_eq!(range(&with(&[("min_text", s("-1e9")), ("max_text", s("1e30"))])), (-1000.0, 1000.0));
    // unreadable: the slider value, and a note
    let p = with(&[("min_text", s("abc")), ("min", f(-4.0))]);
    assert_eq!(range(&p).0, -4.0);
    assert!(crate::figures::notes_for("number-line", &p).iter().any(|n| n.contains("Type From")));
    // a tiny Count by over the longest range is refused with a note, not drawn without limit
    let p = with(&[("min", f(-1000.0)), ("max", f(1000.0)), ("step_text", s("1/100000"))]);
    let out = svg(&p);
    assert!(out.len() < 50_000 && out.contains("Too many ticks"), "{} bytes", out.len());
}

// ───────────── crowded figures: every label as drawn (rule 6m) ─────────────

#[test]
fn number_line_new_labels_never_overlap_text_lines_or_marks() {
    let cases: Vec<Vec<(&str, Value)>> = vec![
        vec![("min", f(0.0)), ("max", f(1.0)), ("step", f(0.1)), ("inequality", s("a_n = 1/n")), ("n_to", i(50)), ("ineq_values", s("Write"))],
        vec![("min", f(-1.0)), ("max", f(1.0)), ("step", f(0.5)), ("inequality", s("a_n = (-1)^n/n")), ("n_to", i(30)), ("ineq_values", s("Write")), ("ineq_mark", s("Cross"))],
        vec![("min", f(0.0)), ("max", f(1.0)), ("step", f(0.25)), ("inequality", s("a_n = 1/n")), ("n_to", i(12)), ("ineq_values", s("Write")),
            ("inequality2", s("b_n = 1 - 1/n")), ("n2_to", i(12)), ("ineq2_values", s("Write")), ("points", s("P(0.3), Q(0.35), R(0.4), S(0.45)")), ("pt_labels", s(LABELS[1]))],
        vec![("inequality", s("-2.5 < x <= -2.4 or x > 2.6")), ("ineq_values", s("Write")), ("inequality2", s("x < -7.25")), ("ineq2_values", s("Write")),
            ("points", s("P(1), Q(1.4), R(1.8), T(2.2)")), ("pt_labels", s(LABELS[1]))],
        vec![("min_text", s("-2pi")), ("max_text", s("2pi")), ("step_text", s("pi/2")), ("inequality", s("-pi/3 < x < 5pi/4")), ("ineq_values", s("Write")),
            ("points", s("A(pi/6), B(pi/4), C(pi/3)")), ("pt_labels", s(LABELS[1])), ("pt_mark", s("Cross"))],
        vec![("min", f(0.0)), ("max", f(2.0)), ("step_text", s("1/4")), ("frac_style", s("Mixed (1 1/4)")), ("inequality", s("1/3 < x < 5/3")), ("ineq_values", s("Write")),
            ("inequality2", s("a_n = 2 - 1/n")), ("ineq2_values", s("Write")), ("n2_to", i(9))],
        // at the page's ends, and a long point name
        vec![("inequality", s("-9.9 < x < 9.95")), ("ineq_values", s("Write")), ("points", s("Alpha(-9.8), Omega(9.9)")), ("pt_labels", s(LABELS[1]))],
    ];
    let mut n = 0;
    for kv in &cases {
        for size in ["Small", "Medium", "Large"] {
            let mut kv = kv.clone();
            kv.push(("label_size", s(size)));
            let p = with(&kv);
            let out = svg(&p);
            check_drawn(&out, &format!("{kv:?}"));
            // each placed label stands over its own mark, with no other mark under a value
            let pl = plan(&p);
            assert!(pl.labels.len() >= 2, "{kv:?}: the sweep places labels");
            for l in &pl.labels {
                let cx = (l.bx.l + l.bx.r) / 2.0;
                assert!(l.mark.0 >= l.bx.l && l.mark.0 <= l.bx.r, "{:?} not over its mark", l.txt);
                if !matches!(l.txt, Txt::Named(..)) { assert!((cx - l.mark.0).abs() < l.bx.r - l.bx.l, "{:?}", l.txt); }
            }
            n += 1;
        }
    }
    assert_eq!(n, 21);
}

// ───────────── handles: still under what is drawn when the line moves down ─────────────

#[test]
fn number_line_handles_follow_the_line_through_the_real_hit_test() {
    let d = def();
    for kv in [
        vec![("inequality", s("-2.5 < x <= 5.3")), ("ineq_values", s("Write")), ("inequality2", s("x < -7.25")), ("points", s("3, -4o")), ("pt_labels", s(LABELS[1]))],
        vec![("min_text", s("-2pi")), ("max_text", s("2pi")), ("step_text", s("pi/2")), ("inequality", s("x > 1")), ("points", s("2.5")), ("pt_mark", s("Cross"))],
        vec![("inequality", s("x >= -2")), ("points", s("P(3), 4")), ("pt_labels", s(LABELS[1])), ("inequality2", s("a_n = n")), ("ineq2_values", s("Write")), ("label_size", s("Large"))],
    ] {
        let p = with(&kv);
        let out = d.svg(&p);
        let hs = interact::handles_for("number-line", &p, size(&out));
        assert!(!hs.is_empty());
        let rings: Vec<(f64, f64, f64)> = dots(&out);
        let marks: Vec<(f64, f64)> = rings.iter().map(|r| (r.0, r.1)).chain(crosses(&out)).collect();
        // every handle sits on a drawn mark on the axis, and a sweep of pixels round it picks that handle
        for (k, h) in hs.iter().enumerate().filter(|(_, h)| h.tip.starts_with("Drag")) {
            assert!(marks.iter().any(|m| (m.0 - h.pos.0).abs() < 0.02 && (m.1 - h.pos.1).abs() < 0.02), "{}: no mark under it at {:?}", h.tip, h.pos);
            let mut hit = 0;
            for dx in -3..=3 { for dy in -3..=3 { if interact::pick(&hs, (h.pos.0 + dx as f64, h.pos.1 + dy as f64), 1.0) == Some(k) { hit += 1; } } }
            assert!(hit >= 25, "{}: picked {hit} of 49", h.tip);
        }
        // dragging the first handle through the real path moves the drawn thing
        let k = hs.iter().position(|h| h.tip.starts_with("Drag")).unwrap();
        let mut q = p.clone();
        (hs[k].drag.as_ref().unwrap())(&mut q, (hs[k].pos.0 + 60.0, hs[k].pos.1));
        assert_ne!(d.svg(&q), out);
    }
}

// ───────────── no student copy to leak ─────────────

#[test]
fn number_line_has_no_hidden_answers_to_leak() {
    // the figure has no Show answers switch (nothing on it is hidden for a student copy), and its SVG
    // carries no title or description text that could say more than the drawing
    let d = def();
    assert!(!(d.params)().iter().any(|q| q.key == crate::figures::SHOW_ANSWERS));
    let out = svg(&with(&[("inequality", s("a_n = 1/n")), ("ineq_values", s("Write")), ("points", s("P(3/4)")), ("pt_labels", s(LABELS[0]))]));
    // the alt text is the figure's own title and blurb, the same for every number line
    let title = out.split("<title>").nth(1).unwrap().split("</title>").next().unwrap();
    let desc = out.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
    assert_eq!((title, desc), (d.title, d.blurb));
    assert!(!out.contains("a_n") && !out.contains("1/n"));
    // with letters only, a lettered point's value is nowhere in the drawing
    assert!(!out.contains("3/4") && !out.contains("0.75"));
}

#[test]
fn number_line_new_controls_hide_where_they_do_nothing() {
    let r = |key: &str, kv: &[(&str, Value)]| crate::figures::param_relevant("number-line", key, &with(kv));
    assert!(!r("n_from", &[("inequality", s("x > 3"))]) && r("n_from", &[("inequality", s("a_n = 1/n"))]));
    assert!(!r("ineq2_mark", &[]) && r("ineq2_mark", &[("inequality2", s("b_n = n"))]));
    assert!(!r("ineq_values", &[("inequality", s(""))]) && r("ineq_values", &[("inequality", s("x > 3"))]));
    assert!(!r("pt_labels", &[("points", s("3, 4"))]) && r("pt_labels", &[("points", s("P(3)"))]));
    assert!(!r("pt_mark", &[]) && r("pt_mark", &[("points", s("3"))]));
    // other figures' controls are untouched by this hook
    assert!(crate::figures::param_relevant("coordinate-grid", "n_from", &with(&[])));
}
