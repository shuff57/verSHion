//! Coordinate grid, batch 7: each row's line, weight and arrows, each axis end, fraction ranges and the
//! inverses of sec, csc and cot. The grid itself is drawn by `coordinate_grid_build` in math.rs; this file
//! holds what batch 7 added beside it, and every batch 7 test.
#![allow(dead_code)]

#[cfg(test)]
pub(crate) mod frozen {
    //! Old coordinate grids frozen before batch 7: every saved spec and share link here is stored with the
    //! hash of what main 47e760a drew, and must draw byte for byte the same.
    use crate::figures::{registry, FigureDef, Params, Value};

    pub(crate) const ID: &str = "coordinate-grid";
    pub(crate) fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("coordinate-grid is registered") }
    pub(crate) fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.into())) }
    pub(crate) fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
    pub(crate) fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    pub(crate) fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
    /// The opening state with `list` set on top.
    pub(crate) fn with(list: &[(&'static str, Value)]) -> Params {
        let d = def();
        let mut p = crate::figures::start_params(&d);
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    /// FNV-1a, so the hashes in the fixture are the same in any build.
    pub(crate) fn fnv(s: &str) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
        format!("{h:016x}")
    }
    fn fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/coordinate_grid_old.txt") }

    /// Every row form the Equations box read before batch 7 (lines in each form, curves with every function,
    /// pieces with open and closed ends, inequalities and bands, conics, systems, typed style words, many rows).
    pub(crate) const ROWS: [&str; 46] = [
        "y = 2x + 1",
        "2x + 3y = 6",
        "x = 4",
        "y = 9 - x; y = 2x + 1",
        "y = x^2 - 4",
        "y = -(x - 2)^2 + 3",
        "y = sqrt(x)",
        "y = |x - 5|",
        "y = 3sin(x) + 5",
        "y = sec(x); y = csc(x); y = cot(x)",
        "y = arctan(x); y = arcsin(x); y = arccos(x); y = asin(x)",
        "y = ln(x); y = log(x); y = log_2(x)",
        "y = e^x; y = exp(-x)",
        "y = tan(x)",
        "y = 1/(x - 5)",
        "y = 1/x",
        "y = (x^2 - 1)/(x - 1)",
        "y = x + 2, x < 1; y = 5 - x, x >= 1",
        "y = 3x, -5 <= x < 7",
        "x = 2, -1 < y <= 3",
        "y > x - 2",
        "y <= x^2",
        "-2 < x <= 3",
        "x > 0",
        "y > 0",
        "y > x - 2; y < 6 - x",
        "x^2 + y^2 = 25",
        "(x-2)^2 + (y+1)^2 = 9",
        "x^2/16 + y^2/9 = 1",
        "x^2/9 - y^2/4 = 1",
        "x = y^2",
        "x^2 + y^2 < 25",
        "x^2 + y^2 < 25; y > x",
        "4 < x^2 + y^2 <= 25",
        "x^2 + y^2 = 25, y > 0",
        "xy = 4",
        "y = 2x/3 dashed; y = x dotted; y = 1 - x solid; y = 2 dash-dot",
        "y = 2x/3 asymptote",
        "x^2 + y^2 = 25 dashed",
        "y = x; y = 2x; y = 3x; y = 4x; y = 5x; y = 6x",
        "y = 2e3; y = 2e",
        "y = sin(x) / x",
        "y = foo(x)",
        "x^2 + y^2 = -4",
        "y = x^3 - 3x",
        "y = 2^x; y = (1/2)^x",
    ];

    /// Old figures: the opening state, every old control alone and with rows, ranges and examples.
    pub(crate) fn old_cases() -> Vec<Vec<(&'static str, Value)>> {
        let q4 = |v: Vec<(&'static str, Value)>| { let mut w = vec![f("xmin", -10.0), f("xmax", 10.0), f("ymin", -10.0), f("ymax", 10.0)]; w.extend(v); w };
        let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
        // every row form, in the first quadrant and round the origin, with labels and in Print
        for r in ROWS {
            cases.push(vec![s("equations", r)]);
            cases.push(q4(vec![s("equations", r)]));
            cases.push(q4(vec![s("equations", r), b("eq_labels", true), s("style", "Print")]));
            cases.push(q4(vec![s("equations", r), s("line_width", "Thick"), b("asymptotes", true), b("show_answers", false)]));
        }
        // every old control alone, on three figures
        let mut opts: Vec<Vec<(&'static str, Value)>> = vec![];
        for g in ["Light", "Standard", "Bold", "None"] { opts.push(vec![s("grid", g)]); }
        for m in [0, 1, 2, 4, 5, 10] { opts.push(vec![i("minor", m)]); }
        for n in ["Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"] { opts.push(vec![s("xnum", n)]); opts.push(vec![s("ynum", n)]); }
        for u in ["Numbers", "Multiples of \u{3c0}", "Degrees"] { opts.push(vec![s("xunits", u)]); }
        for w in ["Thin", "Medium", "Thick"] { opts.push(vec![s("line_width", w)]); }
        for t in ["Tint", "Hatching"] { opts.push(vec![s("shading", t)]); }
        for ps in ["Dot", "Cross"] { opts.push(vec![s("pt_style", ps)]); }
        for pl in ["Letter", "Letter and coordinates", "None"] { opts.push(vec![s("pt_label", pl)]); }
        for ls in ["Large", "Medium", "Small"] { opts.push(vec![s("label_size", ls)]); }
        for tm in ["Text", "Blank line", "None"] {
            opts.push(vec![s("title_mode", tm), s("title", "Graph of a line")]);
            opts.push(vec![s("xtitle_mode", tm)]);
            opts.push(vec![s("ytitle_mode", tm), s("ytitle", "Height (m)")]);
        }
        for k in ["numbers", "arrows", "square", "eq_labels", "asymptotes", "endpoints", "show_answers", "student_shading", "show_notes"] {
            opts.push(vec![b(k, false)]);
            opts.push(vec![b(k, true)]);
        }
        opts.push(vec![s("style", "Print")]);
        opts.push(vec![s("xlabel", "t"), s("ylabel", "d")]);
        opts.push(vec![s("xlabel", ""), s("ylabel", "")]);
        opts.push(vec![s("eq1_color", "#2a8a3e"), s("eq2_color", "#d68910"), s("eq3_color", "#111111"), s("eq4_color", "#c0392b")]);
        opts.push(vec![s("pt_color", "#c0392b")]);
        opts.push(vec![s("points", "A(1, 2), B(-3, 4)")]);
        opts.push(vec![s("points", "(2,3) A; (-1,4) B; (0, 0)")]);
        opts.push(vec![b("arrows", false), s("style", "Print"), s("label_size", "Large")]);
        let bases: [Vec<(&'static str, Value)>; 3] = [
            vec![],
            q4(vec![s("equations", "y = 2x + 1; y > x - 2; x^2 + y^2 = 25; y = 1/(x - 3); y = x + 2, x < 1"), s("points", "A(1, 2), B(-3, 4)"), b("eq_labels", true)]),
            vec![f("xmin", -2.0), f("xmax", 2.0), f("xstep", 0.5), f("ymin", -3.0), f("ymax", 3.0), s("equations", "y = sin(x); y = tan(x) dashed; y < cos(x); y = sec(x)"), s("xunits", "Multiples of \u{3c0}")],
        ];
        for o in &opts {
            for base in &bases {
                let mut c = base.clone();
                c.extend(o.iter().cloned());
                cases.push(c);
            }
        }
        // ranges: negative, decimal, wide, tall, reversed, zero-width, huge, tiny steps, degrees
        let ranges: Vec<Vec<(&'static str, Value)>> = vec![
            vec![f("xmin", -6.0), f("xmax", 6.0), f("ymin", -5.0), f("ymax", 7.0)],
            vec![f("xmin", -3.5), f("xmax", 3.5), f("xstep", 0.5), f("ymin", -2.5), f("ymax", 2.5), f("ystep", 0.5)],
            vec![f("xmin", -100.0), f("xmax", 100.0), f("xstep", 10.0), f("ymin", -50.0), f("ymax", 50.0), f("ystep", 5.0)],
            vec![f("xmin", 0.0), f("xmax", 40.0), f("ymin", 0.0), f("ymax", 4.0)],
            vec![f("xmin", 0.0), f("xmax", 4.0), f("ymin", 0.0), f("ymax", 40.0), b("square", false)],
            vec![f("xmin", 5.0), f("xmax", -5.0)],
            vec![f("ymin", 3.0), f("ymax", 3.0)],
            vec![f("xmin", -1000.0), f("xmax", 1000.0), f("xstep", 0.05)],
            vec![f("xmin", -1000.0), f("xmax", 1000.0), f("ymin", -1000.0), f("ymax", 1000.0), f("xstep", 180.0), f("ystep", 180.0)],
            vec![f("xmin", 0.0), f("xmax", 360.0), f("xstep", 30.0), f("ymin", -2.0), f("ymax", 2.0), s("xunits", "Degrees"), s("equations", "y = sin(x); y = arctan(x)")],
            vec![f("xmin", 0.0), f("xmax", 2.0), f("xstep", 0.25), f("ymin", -2.0), f("ymax", 2.0), s("xunits", "Multiples of \u{3c0}"), s("equations", "y = sin(x); y = cos(x)")],
            vec![f("xmin", 0.1), f("xmax", 0.9), f("xstep", 0.05), f("ymin", 0.0), f("ymax", 1.0), f("ystep", 0.05)],
        ];
        for r in &ranges {
            for extra in [vec![], vec![s("equations", "y = x^2 - 4; y > 1 - x; x^2 + y^2 = 4"), s("points", "A(1, 2)")], vec![s("style", "Print"), b("arrows", false), i("minor", 4)]] {
                let mut c = r.clone();
                c.extend(extra);
                cases.push(c);
            }
        }
        // the shipped examples, as they open
        if let Some(exs) = super::super::examples_b_plots(ID) {
            for e in exs { cases.push(e.params.clone()); }
        }
        cases
    }

    /// Expressions the evaluator read before batch 7 (and some it could not read): every function, implicit products,
    /// scientific notation, |..|, degrees, runs of letters, near-misses of the names batch 7 adds.
    pub(crate) fn old_exprs() -> Vec<String> {
        let mut v: Vec<String> = [
            "2x + 1", "2*x+1", "x^2 - 4", "-(x - 2)^2 + 3", "sqrt(x)", "sqrt x", "|x - 5|", "3sin(x)+5", "sin(x)", "cos(x)", "tan(x)",
            "sec(x)", "csc(x)", "cot(x)", "arcsin(x)", "arccos(x)", "arctan(x)", "asin(x)", "acos(x)", "atan(x)", "ln(x)", "log(x)",
            "log_2(x)", "log_10(x)", "log_(1/2)(x)", "exp(x)", "e^x", "2e", "2ex", "xe", "2e3", "2.5E-3", "x^2e3", "pi", "2pi", "pix",
            "\u{3c0}x", "xy", "x y", "yx", "y", "2y + x", "x/y", "1/x", "1/(x-5)", "(x^2-1)/(x-1)", "x^3 - 3x", "2^x", "(1/2)^x", "x^-1",
            "x^(-1)", "sin^-1(x)", "sin^(-1)(x)", "sin^2(x)", "sin(x)^2", "cos^-1(x)", "tan^-1(x)", "sec^-1(x)", "csc^-1(x)", "cot^-1(x)",
            "arcsec(x)", "arccsc(x)", "arccot(x)", "asec(x)", "acsc(x)", "acot(x)", "arcsecx", "arcsec x", "arcsec", "xarcsec(x)",
            "2arcsec(x)", "arcsec(2x)", "asec(1/x)", "arccot(0)", "sinx", "xsin(x)", "x sin(x)", "abs(x)", "abs(-x)", "foo(x)", "x x",
            "", " ", "(", ")", "x)", "((x)", "1e", "1e+", "1e-2x", "3.", ".5x", "1..2", "x^^2", "--x", "+-x", "-x^2", "2^-x", "x^0.5",
            "sec(x)cos(x)", "1/sec(x)", "e^(-x^2)", "ln(e)", "log(100)", "log_2(8)", "acosh(x)", "arcsinh(x)", "sinh(x)", "a", "b(x)",
            "x\u{2212}1", "2\u{b7}x", "2\u{d7}x", "|x| + |y|", "x^2 + y^2 - 25", "(x)-(y^2)", "((x)-(1))*((3)-(x))", "arctan(1/x)", "1/tan(x)",
            "sec", "csc", "cot", "secant(x)", "ARCSEC(x)", "Arcsec(x)", "SIN(x)", "arc sec(x)", "arc-sec(x)", "a sec(x)", "asec (x)",
            "arccot(-x)", "arccot(x) + 1", "pi/2 - arctan(x)", "acot", "acsc", "asec", "arcs", "arc", "sec^-1 (x)", "sin^ -1(x)",
            "sin ^-1(x)", "sin^-1 x", "sin^-2(x)", "sin^-1", "sin^{-1}(x)", "sin\u{207b}\u{b9}(x)", "sec\u{207b}\u{b9}(x)",
        ].iter().map(|s| s.to_string()).collect();
        // every pair of a function and an argument shape
        for f in ["sin", "cos", "tan", "sec", "csc", "cot", "arcsin", "arccos", "arctan", "asin", "acos", "atan", "ln", "log", "exp", "sqrt", "abs"] {
            for a in ["x", "2x - 1", "1/x", "x^2", "-x"] { v.push(format!("{f}({a})")); v.push(format!("2{f}({a})")); v.push(format!("x{f}({a})")); }
        }
        v
    }
    pub(crate) const XS: [f64; 10] = [-3.0, -2.5, -1.0, -0.5, 0.0, 0.3, 1.0, 2.0, 7.5, 45.0];
    /// One line per expression: the expression, then its value (bits in hex, or "-") at every x, with and without
    /// y = 1.5 and in degrees.
    pub(crate) fn eval_lines() -> Vec<String> {
        old_exprs().iter().map(|e| {
            let mut l = format!("{e:?}");
            for &x in &XS {
                for (y, deg) in [(None, false), (Some(1.5), false), (None, true)] {
                    let v = super::super::eval_full(e, x, y, deg);
                    l.push('\t');
                    l.push_str(&v.map_or("-".to_string(), |v| format!("{:016x}", v.to_bits())));
                }
            }
            l
        }).collect()
    }
    fn eval_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/coordinate_grid_eval_old.txt") }

    /// Expressions batch 7 teaches the evaluator to read (the inverses of sec, csc and cot, and the f^-1 spellings of the
    /// six inverses); before it, every one of them read nothing at all.
    pub(crate) fn newly_read(e: &str) -> bool {
        let t = e.to_lowercase().replace(' ', "");
        ["arcsec(", "arccsc(", "arccot(", "asec(", "acsc(", "acot(", "^-1(", "^(-1)(", "\u{207b}\u{b9}("].iter().any(|k| t.contains(k))
    }

    /// Every expression the evaluator read before batch 7 reads exactly the same value now (to the bit, at ten x values,
    /// with y and in degrees), and every one it could not read still cannot be read, except those batch 7 adds.
    #[test]
    fn old_expressions_read_as_before() {
        let text = std::fs::read_to_string(eval_fixture()).expect("tests/fixtures/coordinate_grid_eval_old.txt");
        let now = eval_lines();
        let old: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(old.len(), now.len());
        let mut changed = 0;
        for (o, n) in old.iter().zip(&now) {
            if o == n { continue; }
            let e = o.split('\t').next().unwrap();
            assert!(newly_read(e), "{e} reads differently: {o} / {n}");
            assert!(o.split('\t').skip(1).all(|v| v == "-"), "{e} was read before batch 7 and must not change");
            changed += 1;
        }
        // (the frozen list holds the near-misses too: "arcsecx", "arc sec(x)", "sin^-2(x)", "sin^2(x)" still read nothing)
        let _ = changed;
    }

    /// Writes the fixture (only with VERSHION_GRID_OLD_BLESS=1, once, on the code before batch 7), else checks it.
    #[test]
    fn old_saved_figures_and_links_render_as_before() {
        use crate::figures::{share, spec, start_params};
        let d = def();
        if std::env::var("VERSHION_GRID_OLD_BLESS").is_ok_and(|v| v == "1") {
            let mut out = String::new();
            for c in old_cases() {
                let p = with(&c);
                let saved = serde_json::to_string(&spec::spec_for(ID, &p)).unwrap();
                let link = share::encode(ID, &p, &start_params(&d));
                out.push_str(&format!("{}\t{saved}\t{link}\n", fnv(&d.svg(&p))));
            }
            std::fs::write(fixture(), out).unwrap();
            std::fs::write(eval_fixture(), eval_lines().join("\n") + "\n").unwrap();
            return;
        }
        let text = std::fs::read_to_string(fixture()).expect("tests/fixtures/coordinate_grid_old.txt");
        let mut n = 0;
        for line in text.lines().filter(|l| !l.is_empty()) {
            let mut it = line.split('\t');
            let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
            let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
            let p = spec::params_from_spec(&d, &sp);
            assert_eq!(fnv(&d.svg(&p)), hash, "an old saved figure draws differently: {saved}");
            let (id, changed) = share::decode(link).unwrap();
            assert_eq!(id, ID);
            let p = share::link_params(&d, changed, share::version(link));
            assert_eq!(fnv(&d.svg(&p)), hash, "an old share link draws differently: {link}");
            n += 1;
        }
        assert!(n >= 400, "only {n} old cases");
    }
}
