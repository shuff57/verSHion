//! The length reading's caliper and micrometer (the ruler is drawn in `measure.rs`).

#[cfg(test)]
mod tests {
    use crate::figures::{registry, FigureDef, Params, Value};

    const ID: &str = "length-reading";
    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("length-reading is registered") }
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.into())) }
    fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
    /// The opening state with `list` set on top.
    fn with(list: &[(&'static str, Value)]) -> Params {
        let d = def();
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

    /// The rulers saved and shared before batch 5 (every old choice and switch, alone and in a few
    /// combinations): their saved spec and share link with the hash of what they drew, written once
    /// from the build before the caliper and micrometer with VERSHION_LEN_OLD_BLESS=1.
    fn old_cases() -> Vec<Vec<(&'static str, Value)>> {
        let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
        for e in super::super::measure::examples(ID).unwrap().into_iter().take(5) { cases.push(e.params); }
        let opts: Vec<(&'static str, Value)> = vec![
            s("units", "Imperial (in)"), s("ruler", "20 cm"), s("ruler", "30 cm"), s("ruler", "50 cm"), s("ruler", "100 cm"),
            s("marks", "10 cm"), s("marks", "5 cm"), s("marks", "1 cm"), s("marks", "0.5 cm"), f("length", 0.05), f("length", 12.34), f("length", 100.0),
            f("offset", 3.21), f("offset", 100.0), s("object", "Marbles"), s("object", "Rock"), s("object", "Cube"), b("dashes", true),
            b("answer", true), s("title", "Measure the cylinder"), s("read_to", "Nearest mark"), s("mag_show", "Ruler and magnifiers"),
            s("mag_show", "Ruler only"), i("mag_marks", 1), i("mag_marks", 6), s("title_style", "None"), s("title_style", "Blank line"),
        ];
        for o in &opts {
            cases.push(vec![o.clone()]);
            cases.push(vec![s("units", "Imperial (in)"), b("answer", true), o.clone()]);
            cases.push(vec![s("mag_show", "Ruler and magnifiers"), f("offset", 2.0), b("answer", true), s("read_to", "Nearest mark"), o.clone()]);
        }
        cases.push(vec![s("ruler", "100 cm"), s("marks", "1 cm"), f("length", 62.4), f("offset", 10.0), s("object", "Marbles"), b("dashes", true), b("answer", true), s("title", "Meter stick")]);
        cases
    }

    fn old_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/length_reading_old.txt") }

    #[test]
    fn old_saved_figures_and_links_render_as_before() {
        use crate::figures::{share, spec, start_params};
        let d = def();
        if std::env::var("VERSHION_LEN_OLD_BLESS").is_ok_and(|v| v == "1") {
            let mut out = String::new();
            for c in old_cases() {
                let p = with(&c);
                let saved = serde_json::to_string(&spec::spec_for(ID, &p)).unwrap();
                let link = share::encode(ID, &p, &start_params(&d));
                out.push_str(&format!("{}\t{saved}\t{link}\n", fnv(&d.svg(&p))));
            }
            std::fs::write(old_fixture(), out).unwrap();
            return;
        }
        let text = std::fs::read_to_string(old_fixture()).expect("tests/fixtures/length_reading_old.txt");
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
        assert!(n >= 80, "only {n} old cases");
    }
}
