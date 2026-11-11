//! Batch 7 additions to the mass and pH readings (the rest of these figures is in `measure.rs`).

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
        let mut n = [0usize; 2];
        for line in text.lines().filter(|l| !l.is_empty()) {
            let mut it = line.split('\t');
            let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
            let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
            let k = ids.iter().position(|x| *x == sp.id).expect("one of the two ids");
            let d = def(ids[k]);
            let p = spec::params_from_spec(&d, &sp);
            assert_eq!(fnv(&d.svg(&p)), hash, "an old saved figure draws differently: {saved}");
            let (id, changed) = share::decode(link).unwrap();
            assert_eq!(id, ids[k]);
            let p = share::link_params(&d, changed, share::version(link));
            assert_eq!(fnv(&d.svg(&p)), hash, "an old share link draws differently: {link}");
            n[k] += 1;
        }
        assert!(n[0] >= 100 && n[1] >= 60, "only {n:?} old cases");
    }
}
