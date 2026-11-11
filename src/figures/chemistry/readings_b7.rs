//! Batch 7 additions to the mass and pH readings (the rest of these figures is in `measure.rs`):
//! a 4-place analytical balance, a weigh boat, a beaker or 1 to 5 marbles on the pan, a random
//! mass or pH, and a magnifier beside, or in place of, the analog pH meter's dial.
use super::magnify::{self, Focus};
use super::*;
use crate::figures::interact::Handle;
use crate::figures::{ParamDef, Params, Value};

/// The digital balance's decimal places: 1, 2 (to 400 g), 3 or 4 (analytical, to 200 g).
pub const MASS_PLACES: [&str; 4] = ["0.1", "0.01", "0.001", "0.0001"];
/// What can sit on the pan.
pub const PANS: [&str; 7] = ["Nothing", "Weigh boat", "Beaker", "Marbles", "Rock", "Cube", "Metal cylinder"];

pub fn marbles_param() -> ParamDef {
    ParamDef::int("marbles", "Number of marbles", 1, 5, 3).hint("How many marbles sit on the pan. The reading stays the mass you type.")
}

/// Decimal places the mass is shown and read to.
pub fn mass_dp(p: &Params) -> usize {
    if p.s("balance").starts_with("Triple") { return 2; }
    match p.s("places") { "0.1" => 1, "0.001" => 3, "0.0001" => 4, _ => 2 }
}

/// Marbles on the pan: 3 for a figure saved before the count existed.
pub fn marble_count(p: &Params) -> usize {
    match p.0.get("marbles") { Some(Value::I(n)) => (*n).clamp(1, 5) as usize, _ => 3 }
}

/// Draws what is on the pan, resting on `base` and centred on `cx` (`size` is the object scale the
/// balance gives), and returns the y of its top (`base` when the pan is empty).
pub fn pan_draw(d: &mut Doc, p: &Params, cx: f64, base: f64, size: f64, seed: u64) -> f64 {
    let s = size;
    match p.s("pan") {
        "" | "Nothing" => base,
        "Weigh boat" => {
            let h = 0.24 * s;
            d.poly(&[(cx - 0.62 * s, base - h), (cx + 0.62 * s, base - h), (cx + 0.42 * s, base), (cx - 0.42 * s, base)], "#f4f4f4", INK, 1.5);
            d.line(cx - 0.47 * s, base - 0.07 * s, cx + 0.47 * s, base - 0.07 * s, "#9a9a9a", 1.0);
            base - h
        }
        "Beaker" => {
            let (w, h) = (0.62 * s, 0.8 * s);
            let (xl, xr) = (cx - w / 2.0, cx + w / 2.0);
            d.poly(&[(xl, base - h + 5.0), (xl - 5.0, base - h), (xr, base - h), (xr, base), (xl, base)], GLASS, GLASS_EDGE, 1.8);
            for t in [0.3, 0.5, 0.7] { d.line(xl + 3.0, base - t * h, xl + 3.0 + 0.25 * w, base - t * h, GLASS_EDGE, 1.0); }
            base - h
        }
        "Marbles" => {
            let n = marble_count(p);
            let r = s * 0.3;
            if n == 3 {
                object(d, "Marbles", cx, base, s, seed);
            } else {
                let (lo, hi) = (base - r, base - r - r * 1.74);
                let at: Vec<(f64, f64)> = match n {
                    1 => vec![(cx, lo)],
                    2 => vec![(cx - r, lo), (cx + r, lo)],
                    4 => vec![(cx - 2.0 * r, lo), (cx, lo), (cx + 2.0 * r, lo), (cx - r, hi)],
                    _ => vec![(cx - 2.0 * r, lo), (cx, lo), (cx + 2.0 * r, lo), (cx - r, hi), (cx + r, hi)],
                };
                for (x, y) in at {
                    d.circle(x, y, r, "#8fb4d6", "#2e4f6e", 1.5);
                    d.circle(x - r * 0.3, y - r * 0.3, r * 0.25, "#d9e9f6", "none", 0.0);
                }
            }
            if n <= 2 { base - 2.0 * r } else { base - r - 1.74 * r - r }
        }
        "Rock" => {
            // the rock's outline is irregular and stood up to 4 px above the pan before batch 7: it
            // is lowered until its lowest point rests on the pan
            let mut t = Doc::new(10.0, 10.0);
            object(&mut t, "Rock", cx, base, s, seed);
            let low = t.finish().split("points=\"").nth(1).map(|pts| pts.split('"').next().unwrap_or("").split_whitespace().filter_map(|q| q.split_once(',')?.1.parse::<f64>().ok()).fold(f64::MIN, f64::max)).unwrap_or(base);
            let drop = (base - low).max(0.0);
            object(d, "Rock", cx, base + drop, s, seed);
            base + drop - 0.36 * s - 0.36 * s
        }
        other => {
            object(d, other, cx, base, s, seed);
            match other {
                "Cube" => base - 0.8 * s - 0.224 * s,
                _ => base - 0.85 * s - 0.55 * s * 0.18,
            }
        }
    }
}

/// Rows the triple beam balance drops from its canvas's top when no lens is drawn: the empty band
/// the lens would use, but never so many that what is on the pan is cut off (before batch 7 a
/// fixed 120 cut the top off the marbles, the cube and the cylinder).
pub fn triple_crop(top: f64) -> f64 { 120f64.min((top - 8.0).floor()).max(0.0) }

/// The pan of the triple beam balance: (centre x, base y, object scale).
pub const TRIPLE_PAN: (f64, f64, f64) = (16.0 + 98.0, 20.0 + 140.0, 62.0);
/// The pan of the digital balance.
pub const DIGITAL_PAN: (f64, f64, f64) = (320.0, 218.0, 90.0);

/// Top of what is on the triple beam's pan (for the crop and the handles).
pub fn triple_top(p: &Params) -> f64 {
    let mut d = Doc::new(10.0, 10.0);
    pan_draw(&mut d, p, TRIPLE_PAN.0, TRIPLE_PAN.1, TRIPLE_PAN.2, 3)
}

/// How far the triple beam's handles move up when its canvas is cropped.
pub fn triple_handle_shift(p: &Params) -> f64 {
    if magnify::wants_magnifier(p) || p.s("mag_show") == "Beams only" { 0.0 } else { triple_crop(triple_top(p)) }
}

/// A random reading the instrument can show: on its decimal places and inside its range (the
/// triple beam's 610 g; the digital balance's 610 g at 1 place, 400 g at 2, 200 g at 3 or 4; pH 0
/// to 14, whole numbers on pH paper). The value is stored like a typed one, so a saved figure or a
/// link keeps it.
pub fn random(id: &str, p: &mut Params, seed: u64) -> bool {
    let (key, cap, dp) = match id {
        "mass-reading" => {
            let dp = mass_dp(p);
            let cap = if p.s("balance").starts_with("Triple") { 610.0 } else { match dp { 1 => 610.0, 2 => 400.0, _ => 200.0 } };
            ("mass", cap, dp)
        }
        "ph-reading" => ("ph", 14.0, ph_dp(p)),
        _ => return false,
    };
    let q = 10f64.powi(dp as i32);
    let steps = (cap * q).round();
    let k = (crate::figures::unit_random(seed) * (steps + 1.0)).floor().clamp(0.0, steps);
    p.set_f(key, k / q);
    true
}

/// Decimal places the pH is shown and keyed to.
pub fn ph_dp(p: &Params) -> usize {
    match p.s("instrument") { "pH paper" => 0, "Analog meter" => 2, _ => if p.s("places") == "0.0" { 1 } else { 2 } }
}

// ---------------------------------------------------------------------------------------------
// The analog pH meter's magnifier
// ---------------------------------------------------------------------------------------------

pub const PH_MAG: [&str; 3] = ["Meter only", "Meter and magnifier", "Magnifier only"];

pub fn ph_mag_param() -> ParamDef {
    ParamDef::choice("mag_show", "Magnifier", &PH_MAG, PH_MAG[0]).hint("For the analog meter: an enlarged view of the needle and the marks round it.")
}

/// The analog dial: centre and radius of its arc (as `measure::ph_render` draws it).
pub const DIAL: (f64, f64, f64) = (280.0, 270.0, 170.0);
/// Lens radius and the width of dial it shows (about 3 pH either side would be too small to read;
/// this shows the needle's numbered mark and its neighbours).
pub const LENS_R: f64 = 150.0;
pub const WINDOW: f64 = 112.0;
/// The lens beside the meter, and the canvas that holds both.
pub const LENS_C: (f64, f64) = (745.0, 195.0);
pub const SIDE_W: f64 = 912.0;
/// The lens alone.
pub const ONLY_C: (f64, f64) = (174.0, 174.0);

fn ph_lens(p: &Params) -> Option<bool> {
    if p.s("instrument") != "Analog meter" { return None; }
    match p.s("mag_show") { "Meter and magnifier" => Some(false), "Magnifier only" => Some(true), _ => None }
}

/// Where the lens looks: on the needle, between the ends of the marks and their numbers.
pub fn ph_focus(ph: f64) -> Focus {
    let (cx, cy, r) = DIAL;
    let a = std::f64::consts::PI * (1.0 - ph.clamp(0.0, 14.0) / 14.0);
    Focus { x: cx + (r - 20.0) * a.cos(), y: cy - (r - 20.0) * a.sin(), label_px: WINDOW }
}

/// Adds the lens to the analog meter's drawing (unchanged when the magnifier is off).
pub fn ph_compose(p: &Params, svg: String, ph: f64) -> String {
    let Some(only) = ph_lens(p) else { return svg };
    let (_, h, body) = magnify::inner(&svg);
    let f = ph_focus(ph);
    let k = 2.0 * LENS_R / WINDOW;
    if only {
        let mut d = Doc::new(2.0 * ONLY_C.0, 2.0 * ONLY_C.1);
        d.raw(&magnify::zoom_group(&body, &f, ONLY_C, LENS_R, k, 0));
        return d.finish();
    }
    let mut d = Doc::new(SIDE_W, h);
    // the joins go under the meter, so they never cross its marks or numbers; they show from the
    // meter's edge to the lens
    for (a, b) in magnify::tangents((f.x, f.y), WINDOW / 2.0, LENS_C, LENS_R) {
        d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1.2" stroke-dasharray="5 4"/>"##, a.0, a.1, b.0, b.1));
    }
    d.raw(&body);
    d.circle(f.x, f.y, WINDOW / 2.0, "none", INK, 1.4);
    d.raw(&magnify::zoom_group(&body, &f, LENS_C, LENS_R, k, 0));
    d.finish()
}

/// Handles of the analog meter with its lens: scroll on the lens to turn the needle (and, beside
/// the meter, the needle's own drag handle as before). `None` when no lens is drawn.
pub fn ph_lens_handles(p: &Params) -> Option<Vec<Handle>> {
    let only = ph_lens(p)?;
    let c = if only { ONLY_C } else { LENS_C };
    Some(vec![Handle::zone(c, (1.4 * LENS_R, 1.4 * LENS_R), "Scroll on the magnifier to turn the needle", |_| {}).with_scroll(|q, dy| {
        let v = (q.f("ph").clamp(0.0, 14.0) + dy * 0.05).clamp(0.0, 14.0);
        q.set_f("ph", (v * 100.0).round() / 100.0);
    })])
}

#[cfg(test)]
mod tests {
    use crate::figures::{registry, FigureDef, Params, Value};

    fn def(id: &str) -> FigureDef { registry().into_iter().find(|d| d.id == id).expect("registered") }
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.into())) }
    fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
    /// The opening state of `id` with `list` set on top.
    fn with(id: &str, list: &[(&'static str, Value)]) -> Params {
        let d = def(id);
        let mut p = crate::figures::start_params(&d);
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }

    fn fnv(s: &str) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
        format!("{h:016x}")
    }

    /// The mass and pH readings saved and shared before batch 7: every old choice and switch alone
    /// and in a few combinations, with the hash of what they drew, written once from the build
    /// before batch 7 with VERSHION_READINGS_OLD_BLESS=1.
    fn old_cases(id: &str) -> Vec<Vec<(&'static str, Value)>> {
        let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
        for e in super::super::measure::examples(id).unwrap() { cases.push(e.params); }
        let (opts, combos): (Vec<(&'static str, Value)>, Vec<Vec<(&'static str, Value)>>) = if id == "mass-reading" {
            (vec![
                s("balance", "Digital balance"), s("balance", "Triple beam balance"),
                s("pan", "Marbles"), s("pan", "Rock"), s("pan", "Cube"), s("pan", "Metal cylinder"), s("pan", "Nothing"),
                f("mass", 0.0), f("mass", 0.01), f("mass", 99.99), f("mass", 100.0), f("mass", 347.85), f("mass", 609.99), f("mass", 610.0), f("mass", 0.2537),
                s("places", "0.1"), s("places", "0.001"), s("title", "Mass of a rock"), s("title", "A very long chart title for a balance figure on a test"),
                s("mag_show", "Instrument only"), s("mag_show", "Beams only"), s("mag_show", "Instrument and magnifier"),
                i("mag_marks", 1), i("mag_marks", 6), b("answer", true), s("title_style", "None"), s("title_style", "Blank line"),
            ], vec![
                vec![s("balance", "Digital balance")],
                vec![s("balance", "Digital balance"), b("answer", true), s("places", "0.001")],
                vec![s("pan", "Marbles"), b("answer", true), s("mag_show", "Instrument only")],
                vec![s("pan", "Marbles"), s("balance", "Digital balance"), s("places", "0.1")],
            ])
        } else {
            (vec![
                s("instrument", "Digital meter"), s("instrument", "Analog meter"), s("instrument", "pH paper"),
                s("places", "0.0"), s("meter", "Probe hanging"), s("meter", "Probe in a beaker"),
                f("ph", 0.0), f("ph", 0.01), f("ph", 6.99), f("ph", 7.0), f("ph", 13.99), f("ph", 14.0),
                s("title", "Lemon juice"), b("answer", true), s("title_style", "None"), s("title_style", "Blank line"),
                            ], vec![
                vec![s("instrument", "Analog meter")],
                vec![s("instrument", "Analog meter"), b("answer", true), s("places", "0.0")],
                vec![s("instrument", "pH paper"), b("answer", true)],
                vec![s("meter", "Probe hanging"), b("answer", true)],
            ])
        };
        for o in &opts {
            cases.push(vec![o.clone()]);
            for c in &combos {
                let mut v = c.clone();
                v.push(o.clone());
                cases.push(v);
            }
        }
        cases
    }

    fn old_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/readings_b7_old.txt") }

    /// Old saved specs and share links of both figures draw byte for byte as before batch 7.
    #[test]
    fn old_saved_mass_and_ph_readings_and_links_render_as_before() {
        use crate::figures::{share, spec, start_params};
        let ids = ["mass-reading", "ph-reading"];
        if std::env::var("VERSHION_READINGS_OLD_BLESS").is_ok_and(|v| v == "1") {
            let mut out = String::new();
            for id in ids {
                let d = def(id);
                for c in old_cases(id) {
                    let p = with(id, &c);
                    let saved = serde_json::to_string(&spec::spec_for(id, &p)).unwrap();
                    let link = share::encode(id, &p, &start_params(&d));
                    out.push_str(&format!("{}\t{saved}\t{link}\n", fnv(&d.svg(&p))));
                }
            }
            std::fs::write(old_fixture(), out).unwrap();
            return;
        }
        let text = std::fs::read_to_string(old_fixture()).expect("tests/fixtures/readings_b7_old.txt");
        let fixed_path = old_fixture().with_file_name("readings_b7_corrected.txt");
        let bless_fixed = std::env::var("VERSHION_READINGS_FIX_BLESS").is_ok_and(|v| v == "1");
        let fixed_text = std::fs::read_to_string(&fixed_path).unwrap_or_default();
        let fixed: std::collections::BTreeMap<&str, &str> = fixed_text.lines().filter_map(|l| l.split_once('\t')).map(|(h, s)| (s, h)).collect();
        let mut renewed = String::new();
        let mut n = [0usize; 2];
        let mut corrected_n = 0;
        for line in text.lines().filter(|l| !l.is_empty()) {
            let mut it = line.split('\t');
            let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
            let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
            let k = ids.iter().position(|x| *x == sp.id).expect("one of the two ids");
            let d = def(ids[k]);
            let p = spec::params_from_spec(&d, &sp);
            let now = fnv(&d.svg(&p));
            let (id, changed) = share::decode(link).unwrap();
            assert_eq!(id, ids[k]);
            let lp = share::link_params(&d, changed, share::version(link));
            assert_eq!(fnv(&d.svg(&lp)), now, "an old share link draws differently from its saved spec: {link}");
            if corrected(ids[k], &p) {
                // drawn whole now: pinned to the corrected drawing, which must differ from the old cut-off one
                assert_ne!(now, hash, "a corrected case still draws as before: {saved}");
                renewed.push_str(&format!("{now}\t{saved}\n"));
                if !bless_fixed { assert_eq!(fixed.get(saved).copied(), Some(now.as_str()), "a corrected old figure draws differently from its pin: {saved}"); }
                corrected_n += 1;
            } else {
                assert_eq!(now, hash, "an old saved figure draws differently: {saved}");
            }
            n[k] += 1;
        }
        if bless_fixed { std::fs::write(&fixed_path, renewed).unwrap(); }
        assert!(n[0] >= 100 && n[1] >= 60, "only {n:?} old cases");
        assert!((10..=60).contains(&corrected_n), "{corrected_n} corrected cases");
    }

    /// The only old figures batch 7 redraws: a triple beam balance with something on its pan and no
    /// lens ("Instrument only"), whose top rows were cropped through the marbles, cube, cylinder and
    /// rock (the crop now stops above what is on the pan); and a rock on either balance, which stood
    /// 2 to 4 px above the pan and now rests on it.
    fn corrected(id: &str, p: &Params) -> bool {
        id == "mass-reading" && (p.s("pan") == "Rock" || (p.s("balance").starts_with("Triple") && p.s("pan") != "Nothing" && p.s("mag_show") == "Instrument only"))
    }

    // ---------------------------------------------------------------------------------------------
    // Reading what is drawn
    // ---------------------------------------------------------------------------------------------

    fn attr(tag: &str, name: &str) -> Option<f64> {
        let k = format!(" {name}=\"");
        let i = tag.find(&k)? + k.len();
        tag[i..].split('"').next()?.parse().ok()
    }
    fn sattr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
        let k = format!(" {name}=\"");
        let i = tag.find(&k)? + k.len();
        tag[i..].split('"').next()
    }
    /// Every `<name ...>` tag in `svg`.
    fn tags<'a>(svg: &'a str, name: &str) -> Vec<&'a str> {
        let open = format!("<{name} ");
        svg.match_indices(&open).map(|(i, _)| { let e = svg[i..].find('>').unwrap(); &svg[i..i + e] }).collect()
    }
    /// (x, y, font size, anchor, text) of every text element.
    fn texts(svg: &str) -> Vec<(f64, f64, f64, String, String)> {
        svg.match_indices("<text ").map(|(i, _)| {
            let e = svg[i..].find('>').unwrap();
            let tag = &svg[i..i + e];
            let close = svg[i + e..].find("</text>").unwrap();
            let body = svg[i + e + 1..i + e + close].to_string();
            (attr(tag, "x").unwrap_or(0.0), attr(tag, "y").unwrap_or(0.0), attr(tag, "font-size").unwrap_or(0.0), sattr(tag, "text-anchor").unwrap_or("start").to_string(), body)
        }).collect()
    }
    /// Bounding box of the shapes drawn (circle, ellipse, rect, polygon, polyline, line).
    fn bbox(svg: &str) -> (f64, f64, f64, f64) {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let mut add = |x: f64, y: f64| { b.0 = b.0.min(x); b.1 = b.1.min(y); b.2 = b.2.max(x); b.3 = b.3.max(y); };
        for t in tags(svg, "circle") { let (x, y, r) = (attr(t, "cx").unwrap(), attr(t, "cy").unwrap(), attr(t, "r").unwrap()); add(x - r, y - r); add(x + r, y + r); }
        for t in tags(svg, "ellipse") { let (x, y, a, c) = (attr(t, "cx").unwrap(), attr(t, "cy").unwrap(), attr(t, "rx").unwrap(), attr(t, "ry").unwrap()); add(x - a, y - c); add(x + a, y + c); }
        for t in tags(svg, "rect") { let (x, y, w, h) = (attr(t, "x").unwrap(), attr(t, "y").unwrap(), attr(t, "width").unwrap(), attr(t, "height").unwrap()); add(x, y); add(x + w, y + h); }
        for t in tags(svg, "line") { add(attr(t, "x1").unwrap(), attr(t, "y1").unwrap()); add(attr(t, "x2").unwrap(), attr(t, "y2").unwrap()); }
        for name in ["polygon", "polyline"] {
            for t in tags(svg, name) {
                for pt in sattr(t, "points").unwrap().split_whitespace() {
                    let (x, y) = pt.split_once(',').unwrap();
                    add(x.parse().unwrap(), y.parse().unwrap());
                }
            }
        }
        b
    }
    fn view(svg: &str) -> (f64, f64) { crate::figures::svg_size(svg) }

    const PAN_OPTS: [&str; 7] = super::PANS;

    /// Every pan content on both balances (and 1 to 5 marbles): what is drawn rests on the pan, inside
    /// its width, above the display, and wholly on the page in every view.
    #[test]
    fn what_is_on_the_pan_sits_on_the_pan_and_on_the_page() {
        for balance in ["Triple beam balance", "Digital balance"] {
            let (cx, base, size) = if balance.starts_with("Triple") { super::TRIPLE_PAN } else { super::DIGITAL_PAN };
            // the pan's top surface: the triple beam's dish 34..194 at y 162, the digital's plate 210..430 at y 218
            let (pl, pr) = if balance.starts_with("Triple") { (34.0, 194.0) } else { (210.0, 430.0) };
            for pan in PAN_OPTS.iter().skip(1) {
                for n in if *pan == "Marbles" { 1..=5 } else { 3..=3 } {
                    let p = with("mass-reading", &[s("balance", balance), s("pan", pan), i("marbles", n)]);
                    let mut d = crate::figures::svg::Doc::new(10.0, 10.0);
                    let top = super::pan_draw(&mut d, &p, cx, base, size, if balance.starts_with("Triple") { 3 } else { 5 });
                    let svg = d.finish();
                    let body = &svg[svg.find("fill=\"#fff\"/>").unwrap()..];
                    let (x0, y0, x1, y1) = bbox(body);
                    let what = format!("{balance}, {pan} x{n}");
                    assert!((y1 - base).abs() <= 1.0, "{what}: bottom {y1} is not on the pan at {base}");
                    assert!(x0 >= pl && x1 <= pr, "{what}: {x0}..{x1} is off the pan {pl}..{pr}");
                    assert!((y0 - top).abs() <= 6.0, "{what}: reported top {top} but drawn to {y0}");
                    if *pan == "Marbles" {
                        let r = size * 0.3;
                        let balls: Vec<(f64, f64)> = tags(body, "circle").into_iter().filter(|t| (attr(t, "r").unwrap() - r).abs() < 0.02).map(|t| (attr(t, "cx").unwrap(), attr(t, "cy").unwrap())).collect();
                        assert_eq!(balls.len(), n as usize, "{what}: marbles drawn");
                        for a in 0..balls.len() { for b in a + 1..balls.len() {
                            let dd = (balls[a].0 - balls[b].0).hypot(balls[a].1 - balls[b].1);
                            assert!(dd >= 2.0 * r - 0.05, "{what}: marbles overlap");
                        } }
                        // each upper marble rests on two below it
                        for m in balls.iter().filter(|m| m.1 < base - r - 1.0) {
                            assert_eq!(balls.iter().filter(|o| o.1 > m.1 + 1.0 && (o.0 - m.0).hypot(o.1 - m.1) < 2.0 * r + 1.0).count(), 2, "{what}: a marble floats");
                        }
                    }
                    for show in ["Instrument only", "Instrument and magnifier"] {
                        let p = with("mass-reading", &[s("balance", balance), s("pan", pan), i("marbles", n), s("mag_show", show)]);
                        let full = def("mass-reading").svg(&p);
                        let (_, h) = view(&full);
                        // the crop moves the drawing up: what was at y is drawn at y - crop
                        let shift = if balance.starts_with("Triple") && show == "Instrument only" { super::triple_crop(top) } else { 0.0 };
                        assert!(y0 - shift >= 2.0 && y1 - shift <= h, "{what}, {show}: top {} is off the page", y0 - shift);
                        if !balance.starts_with("Triple") { assert!(y1 <= 218.5, "{what}: over the display"); }
                    }
                }
            }
        }
    }

    /// The 4-place analytical balance: the display shows exactly the mass to 4 places, its digits fit
    /// the display (clear of its "g") at every mass up to 610 g, and the key says the same.
    #[test]
    fn four_places_fit_the_display_and_the_key_gives_four() {
        for m in [0.0, 0.0001, 0.2537, 9.9999, 24.73, 199.9999, 200.0, 609.9999, 610.0] {
            for extra in [vec![], vec![s("label_size", "Large")], vec![s("pan", "Beaker")]] {
                let mut l = vec![s("balance", "Digital balance"), s("places", "0.0001"), f("mass", m), b("answer", true), s("mag_show", "Instrument only")];
                l.extend(extra);
                let svg = def("mass-reading").svg(&with("mass-reading", &l));
                let want = format!("{m:.4}");
                let shown: Vec<_> = texts(&svg).into_iter().filter(|t| t.2 == 36.0).collect();
                assert_eq!(shown.len(), 1);
                let (x, _, fs, anchor, t) = &shown[0];
                assert_eq!(t, &want, "display");
                assert_eq!(anchor, "end");
                // bold Arial digits are 0.556 em wide and the point 0.278 em: allow 0.62 and 0.32
                let w = t.chars().map(|c| if c == '.' { 0.32 } else { 0.62 }).sum::<f64>() * fs;
                assert!(x - w >= 142.0 && *x <= 350.0, "{t}: {:.1}..{x} overruns the display 120..350 or its g", x - w);
                assert!(svg.contains(&format!("Mass: {want} g")), "key for {m}");
            }
        }
        // the range note and the panel follow the places
        let p = with("mass-reading", &[s("balance", "Digital balance"), s("places", "0.0001")]);
        assert_eq!(super::super::measure::range_note("mass-reading", &p).unwrap(), "From 0 to 610 g, to 4 decimal places.");
        // and each of the four places draws differently
        let r: Vec<String> = super::MASS_PLACES.iter().map(|pl| def("mass-reading").svg(&with("mass-reading", &[s("balance", "Digital balance"), s("places", pl), f("mass", 12.3456)]))).collect();
        for a in 0..4 { for c in a + 1..4 { assert_ne!(r[a], r[c], "places {a} and {c}"); } }
    }

    /// Every new choice draws differently from every other choice of the same control.
    #[test]
    fn every_option_draws_differently() {
        for balance in ["Triple beam balance", "Digital balance"] {
            let mut seen: Vec<(String, String)> = Vec::new();
            for pan in PAN_OPTS {
                for n in if pan == "Marbles" { 1..=5 } else { 3..=3 } {
                    for show in ["Instrument only", "Instrument and magnifier"] {
                        let svg = def("mass-reading").svg(&with("mass-reading", &[s("balance", balance), s("pan", pan), i("marbles", n), s("mag_show", show)]));
                        let name = format!("{balance} {pan} {n} {show}");
                        for (other, o) in &seen { if o == &svg { panic!("{name} draws as {other}"); } }
                        seen.push((name, svg));
                    }
                }
            }
        }
        let r: Vec<String> = super::PH_MAG.iter().map(|m| def("ph-reading").svg(&with("ph-reading", &[s("instrument", "Analog meter"), s("mag_show", m)]))).collect();
        for a in 0..3 { for c in a + 1..3 { assert_ne!(r[a], r[c], "{} and {}", super::PH_MAG[a], super::PH_MAG[c]); } }
    }

    /// The marble count and the magnifier are offered only where they act, and these hooks leave every
    /// other figure's controls alone.
    #[test]
    fn new_controls_show_only_where_they_act() {
        use crate::figures::cards::relevant;
        let p = with("mass-reading", &[s("pan", "Rock")]);
        assert!(!relevant("mass-reading", "marbles", &p));
        assert!(relevant("mass-reading", "marbles", &with("mass-reading", &[s("pan", "Marbles")])));
        assert!(!relevant("ph-reading", "mag_show", &with("ph-reading", &[])));
        assert!(relevant("ph-reading", "mag_show", &with("ph-reading", &[s("instrument", "Analog meter")])));
        for id in ["volume-reading", "temperature-reading", "gas-syringe", "length-reading", "volume-by-displacement"] {
            let d = def(id);
            let q = crate::figures::start_params(&d);
            for k in ["marbles", "mag_show", "places", "pan"] { assert!(relevant(id, k, &q), "{id} {k}"); }
        }
        // the marble count only matters with marbles: elsewhere it draws nothing different
        for pan in ["Rock", "Beaker", "Nothing"] {
            assert_eq!(def("mass-reading").svg(&with("mass-reading", &[s("pan", pan), i("marbles", 1)])), def("mass-reading").svg(&with("mass-reading", &[s("pan", pan), i("marbles", 5)])));
        }
    }

    // ---------------------------------------------------------------------------------------------
    // Random readings
    // ---------------------------------------------------------------------------------------------

    /// A random mass or pH lands on what the instrument shows, stays in its range, varies, is stored
    /// (so a saved figure and a share link redraw it exactly), and the key states what is drawn.
    #[test]
    fn random_readings_are_on_the_instrument_and_kept() {
        use crate::figures::{random_key, random_reading, share, spec, start_params};
        assert_eq!(random_key("mass-reading"), Some("mass"));
        assert_eq!(random_key("ph-reading"), Some("ph"));
        let setups: Vec<(&str, Vec<(&'static str, Value)>, f64, usize)> = vec![
            ("mass-reading", vec![], 610.0, 2),
            ("mass-reading", vec![s("balance", "Digital balance"), s("places", "0.1")], 610.0, 1),
            ("mass-reading", vec![s("balance", "Digital balance")], 400.0, 2),
            ("mass-reading", vec![s("balance", "Digital balance"), s("places", "0.001")], 200.0, 3),
            ("mass-reading", vec![s("balance", "Digital balance"), s("places", "0.0001")], 200.0, 4),
            ("ph-reading", vec![], 14.0, 2),
            ("ph-reading", vec![s("places", "0.0")], 14.0, 1),
            ("ph-reading", vec![s("instrument", "Analog meter")], 14.0, 2),
            ("ph-reading", vec![s("instrument", "pH paper")], 14.0, 0),
        ];
        for (id, base, cap, dp) in setups {
            let d = def(id);
            let key = random_key(id).unwrap();
            let mut seen = std::collections::BTreeSet::new();
            for seed in 0..300u64 {
                let mut p = with(id, &base);
                p.set_b("answer", true);
                assert!(random_reading(id, &mut p, seed));
                let v = p.f(key);
                let q = 10f64.powi(dp as i32);
                assert!((0.0..=cap).contains(&v), "{id} {base:?}: {v}");
                assert!(((v * q).round() - v * q).abs() < 1e-6, "{id}: {v} is not on {dp} places");
                assert_eq!(format!("{v}").split('.').nth(1).map_or(0, |f| f.len()) <= dp, true, "{id}: {v} prints with float noise");
                seen.insert((v * q).round() as i64);
                let svg = d.svg(&p);
                let shown = format!("{v:.dp$}");
                let key_line = if id == "mass-reading" { format!("Mass: {shown} g") } else { format!("pH: {shown}") };
                assert!(svg.contains(&key_line), "{id}: key {key_line}");
                // saved and shared, it draws the same reading (it is not rolled again)
                if seed % 50 == 0 {
                    let sp = spec::spec_for(id, &p);
                    let sp: crate::model::FigureSpec = serde_json::from_str(&serde_json::to_string(&sp).unwrap()).unwrap();
                    assert_eq!(d.svg(&spec::params_from_spec(&d, &sp)), svg, "{id}: saved");
                    let link = share::encode(id, &p, &start_params(&d));
                    let (_, changed) = share::decode(&link).unwrap();
                    assert_eq!(d.svg(&share::link_params(&d, changed, share::version(&link))), svg, "{id}: link");
                }
            }
            assert!(seen.len() as f64 > (0.8 * cap * 10f64.powi(dp as i32)).min(150.0), "{id} {base:?}: only {} values", seen.len());
        }
        // a mass picked for the triple beam is what its riders add up to, as drawn
        for seed in 0..60u64 {
            let mut p = with("mass-reading", &[s("mag_show", "Instrument only")]);
            random_reading("mass-reading", &mut p, seed);
            let m = p.f("mass");
            let svg = def("mass-reading").svg(&p);
            assert!((riders_read(&svg, &p) - m).abs() < 0.005, "riders read {} for {m}", riders_read(&svg, &p));
        }
        let mut q = with("ph-reading", &[]);
        assert!(!super::random("volume-reading", &mut q, 1));
    }

    /// The triple beam's mass read off its drawn riders (each rider's tip against its beam's scale).
    fn riders_read(svg: &str, p: &Params) -> f64 {
        let _ = p;
        let x0 = 16.0 + 248.0;
        let mut total = 0.0;
        // riders are the grey paths; their tip x is the middle of the path's two x extremes
        for t in tags(svg, "path").into_iter().filter(|t| t.contains("fill=\"#d4d4d4\"")) {
            let d = sattr(t, "d").unwrap();
            let nums: Vec<f64> = d.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter_map(|v| v.parse().ok()).collect();
            let (xa, ya) = (nums[0], nums[1]);
            let xb = nums[2];
            let x = (xa + xb) / 2.0;
            let y = ya;
            let range = if (y - 137.0).abs() < 3.0 { 100.0 } else if (y - 185.0).abs() < 3.0 { 500.0 } else { 10.0 };
            total += (x - x0) / 336.0 * range;
        }
        total
    }

    // ---------------------------------------------------------------------------------------------
    // The analog pH meter's magnifier
    // ---------------------------------------------------------------------------------------------

    /// The dial read the way a student reads it, from what is drawn: the marks and the needle (lines),
    /// the numbers beside the long marks. Inside the lens, `lens` gives its group's transform and
    /// clip circle; only what lies inside the circle counts. Returns (reading, numbered marks seen).
    fn read_dial(svg: &str, lens: bool) -> (f64, usize) {
        let (body, tf, clip) = if lens {
            let g = svg.find("<g clip-path=\"url(#qsmag0)\">").expect("a lens");
            let c = &svg[svg.find("<clipPath id=\"qsmag0\">").unwrap()..];
            let ct = tags(c, "circle")[0];
            let clip = (attr(ct, "cx").unwrap(), attr(ct, "cy").unwrap(), attr(ct, "r").unwrap());
            let tg = &svg[g..];
            let tr = tg.find("transform=\"").unwrap() + 11;
            let spec = &tg[tr..tr + tg[tr..].find('"').unwrap()];
            let nums: Vec<f64> = spec.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter_map(|v| v.parse().ok()).collect();
            // translate(a b) scale(k) translate(c d)
            let (a, bb, k, c2, d2) = (nums[0], nums[1], nums[2], nums[3], nums[4]);
            let end = tg.find("</g></g>").unwrap();
            (tg[..end].to_string(), (k, a + k * c2, bb + k * d2), Some(clip))
        } else {
            let cut = svg.find("<defs><clipPath id=\"qsmag0\"").unwrap_or(svg.len());
            (svg[..cut].to_string(), (1.0, 0.0, 0.0), None)
        };
        let t = |x: f64, y: f64| (tf.1 + tf.0 * x, tf.2 + tf.0 * y);
        let inside = |q: (f64, f64)| clip.is_none_or(|c| (q.0 - c.0).hypot(q.1 - c.1) <= c.2 - 1.0);
        let lines: Vec<((f64, f64), (f64, f64), String)> = tags(&body, "line").into_iter().filter(|l| !l.contains("stroke-dasharray")).map(|l| (t(attr(l, "x1").unwrap(), attr(l, "y1").unwrap()), t(attr(l, "x2").unwrap(), attr(l, "y2").unwrap()), sattr(l, "stroke").unwrap().to_string())).collect();
        let needle = lines.iter().find(|l| l.2 == "#b02020").expect("the needle");
        let ticks: Vec<&((f64, f64), (f64, f64), String)> = lines.iter().filter(|l| l.2 == "#111").collect();
        // the pivot: where the marks' lines meet (two marks far apart)
        let meet = |a: &((f64, f64), (f64, f64), String), b: &((f64, f64), (f64, f64), String)| {
            let (p, r) = (a.0, (a.1.0 - a.0.0, a.1.1 - a.0.1));
            let (q, s2) = (b.0, (b.1.0 - b.0.0, b.1.1 - b.0.1));
            let den = r.0 * s2.1 - r.1 * s2.0;
            let u = ((q.0 - p.0) * s2.1 - (q.1 - p.1) * s2.0) / den;
            (p.0 + u * r.0, p.1 + u * r.1)
        };
        // the marks a student sees: their outer end inside the lens
        let seen: Vec<_> = ticks.iter().filter(|l| inside(l.0)).collect();
        assert!(seen.len() >= 4, "only {} marks in view", seen.len());
        let piv = meet(seen[0], seen[(seen.len() - 1).min(seen.len() / 2 + 2)]);
        let ang = |q: (f64, f64)| { let a = (piv.1 - q.1).atan2(q.0 - piv.0); if a < -std::f64::consts::FRAC_PI_2 { a + 2.0 * std::f64::consts::PI } else { a } };
        let len = |l: &((f64, f64), (f64, f64), String)| (l.1.0 - l.0.0).hypot(l.1.1 - l.0.1);
        let long = seen.iter().map(|l| len(l)).fold(0.0, f64::max);
        let mut marks: Vec<(f64, bool, (f64, f64))> = seen.iter().map(|l| (ang(l.0), len(l) > 0.75 * long, l.1)).collect();
        marks.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap()); // from the left (pH 0) round to the right
        // numbers: digits whose centre is inside the lens, each beside its nearest long mark
        let nums: Vec<(f64, (f64, f64))> = texts(&body).into_iter().filter_map(|(x, y, fs, _, s2)| {
            let v: f64 = s2.parse().ok()?;
            let c = t(x, y - 5.0 + 0.0 * fs);
            inside(c).then_some((v, c))
        }).collect();
        let mut named: Vec<(usize, f64)> = Vec::new();
        for (v, c) in &nums {
            let Some((i, dd)) = marks.iter().enumerate().filter(|m| m.1.1).map(|(i, m)| (i, (m.2.0 - c.0).hypot(m.2.1 - c.1))).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()) else { continue };
            // a number whose own mark is outside the lens is not used
            if dd > 30.0 * tf.0 { continue; }
            named.push((i, *v));
        }
        assert!(named.len() >= 2, "only {} numbered marks in view", named.len());
        let step = (named[1].1 - named[0].1) / (named[1].0 as f64 - named[0].0 as f64);
        let val = |i: usize| named[0].1 + (i as f64 - named[0].0 as f64) * step;
        let a = ang(needle.1);
        for i in 0..marks.len() - 1 {
            let (a0, a1) = (marks[i].0, marks[i + 1].0);
            if a <= a0 + 1e-9 && a >= a1 - 1e-9 {
                return (val(i) + (a0 - a) / (a0 - a1) * (val(i + 1) - val(i)), named.len());
            }
        }
        if (a - marks[0].0).abs() < 1e-6 { return (val(0), named.len()); }
        panic!("the needle at {a} is outside the marks in view");
    }

    /// The lens beside the dial and in place of it shows the needle against the marks enlarged, and a
    /// student reads the same pH from it as from the dial itself (and as the key) for every pH.
    #[test]
    fn the_magnifier_reads_the_same_as_the_dial() {
        for k in 0..=280 {
            let ph = k as f64 * 0.05;
            for show in ["Meter and magnifier", "Magnifier only"] {
                let p = with("ph-reading", &[s("instrument", "Analog meter"), f("ph", ph), s("mag_show", show)]);
                let svg = def("ph-reading").svg(&p);
                let (lens, n) = read_dial(&svg, true);
                assert!((lens - ph).abs() < 0.005, "{show} at pH {ph}: the lens reads {lens}");
                assert!(n >= 2);
                if show == "Meter and magnifier" {
                    let (dial, _) = read_dial(&svg, false);
                    assert!((dial - lens).abs() < 0.005, "pH {ph}: dial {dial}, lens {lens}");
                }
            }
        }
        // the lens sits beside the meter, on the page, and in place of it the canvas is the lens
        let svg = def("ph-reading").svg(&with("ph-reading", &[s("instrument", "Analog meter"), s("mag_show", "Meter and magnifier")]));
        let (w, h) = view(&svg);
        assert!(super::LENS_C.0 - super::LENS_R > 500.0 + 10.0 && super::LENS_C.0 + super::LENS_R <= w - 8.0, "beside the meter");
        assert!(super::LENS_C.1 - super::LENS_R >= 8.0 && super::LENS_C.1 + super::LENS_R <= h - 8.0);
        let only = def("ph-reading").svg(&with("ph-reading", &[s("instrument", "Analog meter"), s("mag_show", "Magnifier only")]));
        assert_eq!(view(&only), (348.0, 348.0));
        // the magnifier is the analog meter's: the digital meter and the paper draw as before
        for inst in ["Digital meter", "pH paper"] {
            assert_eq!(def("ph-reading").svg(&with("ph-reading", &[s("instrument", inst), s("mag_show", "Magnifier only")])), def("ph-reading").svg(&with("ph-reading", &[s("instrument", inst)])), "{inst}");
        }
    }

    /// No reading on the student copy: not in the drawing's text, not in its title or description,
    /// with the lens on or off; the key line appears only with the answer key.
    #[test]
    fn the_student_copy_leaks_no_reading() {
        for (id, list, shown) in [
            ("ph-reading", vec![s("instrument", "Analog meter"), f("ph", 8.46), s("mag_show", "Meter and magnifier")], "8.46"),
            ("ph-reading", vec![s("instrument", "Analog meter"), f("ph", 3.27), s("mag_show", "Magnifier only")], "3.27"),
            ("mass-reading", vec![f("mass", 347.85), s("pan", "Marbles"), i("marbles", 5)], "347.85"),
            ("mass-reading", vec![f("mass", 72.35), s("pan", "Beaker"), s("mag_show", "Instrument only")], "72.35"),
        ] {
            let mut l = list.clone();
            l.push(b("answer", false));
            l.push(b("show_answers", false));
            let svg = def(id).svg(&with(id, &l));
            assert!(texts(&svg).iter().all(|t| !t.4.contains(shown)), "{id} shows {shown}");
            for t in ["title", "desc"] {
                for chunk in svg.split(&format!("<{t}>")).skip(1) {
                    let txt = chunk.split(&format!("</{t}>")).next().unwrap();
                    assert!(!txt.chars().any(|c| c.is_ascii_digit()), "{id} <{t}> {txt}");
                }
            }
            let mut l = list;
            l.push(b("answer", true));
            assert!(texts(&def(id).svg(&with(id, &l))).iter().any(|t| t.4.contains(shown)), "{id} key");
        }
    }

    /// Through the workspace's hit test: the lens turns the needle on scroll, the needle's own handle
    /// is still where the needle is, and the triple beam's riders are under the pointer in the
    /// cropped view too.
    #[test]
    fn handles_answer_through_the_hit_test() {
        use crate::figures::interact::pick;
        for show in ["Meter and magnifier", "Magnifier only"] {
            let p = with("ph-reading", &[s("instrument", "Analog meter"), f("ph", 6.4), s("mag_show", show)]);
            let svg = def("ph-reading").svg(&p);
            let hs = def("ph-reading").handles(&p, view(&svg));
            let c = if show == "Magnifier only" { super::ONLY_C } else { super::LENS_C };
            let mut hits = 0;
            for dx in (-100..=100).step_by(25) { for dy in (-100..=100).step_by(25) {
                let q = (c.0 + dx as f64, c.1 + dy as f64);
                let Some(ix) = pick(&hs, q, 1.0) else { continue };
                assert_eq!(hs[ix].tip, "Scroll on the magnifier to turn the needle");
                let mut p2 = p.clone();
                (hs[ix].scroll.as_ref().unwrap())(&mut p2, 2.0);
                assert!((p2.f("ph") - 6.5).abs() < 1e-9, "{}", p2.f("ph"));
                hits += 1;
            } }
            assert!(hits >= 60, "{show}: {hits}");
            let needle = hs.iter().position(|h| h.tip == "Drag the needle to set the pH");
            assert_eq!(needle.is_some(), show != "Magnifier only", "{show}");
            if let Some(n) = needle {
                assert_eq!(pick(&hs, hs[n].pos, 1.0), Some(n));
                let mut p2 = p.clone();
                (hs[n].drag.as_ref().unwrap())(&mut p2, (280.0 + 100.0, 270.0 - 100.0));
                assert!((p2.f("ph") - 10.5).abs() < 0.01, "{}", p2.f("ph"));
            }
        }
        // the triple beam riders, with and without the crop: each handle sits on its drawn rider
        for (pan, show) in [("Nothing", "Instrument only"), ("Marbles", "Instrument only"), ("Beaker", "Instrument only"), ("Marbles", "Instrument and magnifier")] {
            let p = with("mass-reading", &[s("pan", pan), i("marbles", 5), s("mag_show", show), f("mass", 347.85)]);
            let svg = def("mass-reading").svg(&p);
            let hs = def("mass-reading").handles(&p, view(&svg));
            let riders: Vec<(f64, f64)> = tags(&svg, "path").into_iter().filter(|t| t.contains("fill=\"#d4d4d4\"")).map(|t| {
                let d = sattr(t, "d").unwrap();
                let nums: Vec<f64> = d.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter_map(|v| v.parse().ok()).collect();
                // drawn inside the crop's translate, so on screen it is up by the crop
                ((nums[0] + nums[2]) / 2.0, nums[1] + 6.0 - super::triple_handle_shift(&p))
            }).collect();
            assert!(riders.len() >= 3);
            for r in riders.iter().take(3) {
                let ix = pick(&hs, *r, 1.0).unwrap_or_else(|| panic!("{pan} {show}: no handle on the rider at {r:?}"));
                assert!(hs[ix].tip.contains("rider"), "{}", hs[ix].tip);
            }
        }
    }

    /// The ends of every range draw whole and print cleanly.
    #[test]
    fn the_ends_of_every_range_draw_whole() {
        for list in [
            vec![s("balance", "Digital balance"), s("places", "0.0001"), f("mass", 610.0), s("pan", "Beaker"), b("answer", true)],
            vec![s("balance", "Digital balance"), s("places", "0.0001"), f("mass", 0.0), s("pan", "Weigh boat"), b("answer", true)],
            vec![f("mass", 610.0), s("pan", "Marbles"), i("marbles", 5), b("answer", true), s("mag_show", "Instrument only")],
            vec![f("mass", 0.0), s("pan", "Marbles"), i("marbles", 1), b("answer", true), s("title", "A long title for the balance on the test, with marbles")],
            vec![f("mass", 1e9), i("marbles", 99), s("pan", "Marbles")],
        ] {
            let p = with("mass-reading", &list);
            assert!((0.0..=610.0).contains(&p.f("mass")) && (1..=5).contains(&p.i("marbles")));
            let svg = def("mass-reading").svg(&p);
            assert!(svg.len() < 400_000 && !svg.contains("NaN") && !svg.contains("inf"), "{list:?}");
        }
        for ph in [-5.0, 0.0, 14.0, 99.0] {
            let p = with("ph-reading", &[s("instrument", "Analog meter"), f("ph", ph), s("mag_show", "Magnifier only"), b("answer", true)]);
            let svg = def("ph-reading").svg(&p);
            assert!(!svg.contains("NaN"));
            assert!(svg.contains(&format!("pH: {:.2}", ph.clamp(0.0, 14.0))), "{ph}");
        }
    }
}
