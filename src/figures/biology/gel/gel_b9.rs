//! Gel electrophoresis, batch 9: a sample lane left blank for students. Each sample lane's card has "Leave blank for
//! students"; the lanes ticked are kept as their numbers in `blank_lanes` (rewritten whenever the cards move, so a
//! flag stays with its lane). They follow the gel's Answer key switch, the one Student + key already turns off for
//! the student copy and on for the key (`answer_rule`): with it on a blank lane draws its bands inside a dashed outline
//! that marks it as the students' lane; with it off the lane is an empty well and track. Old figures have no blank
//! lanes, so they and their Student + key exports draw as before.
use super::*;

pub(super) fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("blank_lanes", "Sample lanes left blank for students (lane numbers)", ""),
    ]
}

/// The lane numbers (1 = the first lane drawn) ticked blank; anything else in the text is ignored.
pub(crate) fn blank_set(p: &Params) -> Vec<usize> {
    let mut v: Vec<usize> = p.s("blank_lanes").split([',', ' ', ';']).filter_map(|t| t.trim().parse::<usize>().ok()).filter(|n| (1..=MAX_LANES).contains(n)).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// "2, 4" from the cards' flags (sample lanes only).
pub(super) fn blank_text(lanes: &[LaneCard]) -> String {
    lanes.iter().enumerate().filter(|(_, l)| l.blank && matches!(l.kind, LaneKind::Sample(_))).map(|(i, _)| (i + 1).to_string()).collect::<Vec<_>>().join(", ")
}

/// Is drawn lane `i` (0-based) a sample lane left blank? `ladder` lanes never are.
pub(super) fn is_blank(set: &[usize], i: usize, ladder: bool) -> bool { !ladder && set.contains(&(i + 1)) }

#[cfg(test)]
mod tests {
    use crate::figures::{registry, start_params, FigureDef, Params, Value};
    use super::super::{lane_cards, set_lane_cards, LaneCard, LaneKind};

    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "gel-electrophoresis").unwrap() }
    fn params(over: &[(&str, Value)]) -> Params {
        let d = def();
        let mut p = start_params(&d);
        for (k, v) in over { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    fn s(v: &str) -> Value { Value::S(v.into()) }
    fn b(v: bool) -> Value { Value::B(v) }

    /// The band rectangles drawn (34 wide), as (centre x, top y).
    fn bands(svg: &str) -> Vec<(f64, f64)> {
        svg.split("<rect").skip(1).filter(|e| e.contains("width=\"34\"")).map(|e| {
            let f = |k: &str| -> f64 { e.split(&format!(" {k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
            (f("x") + 17.0, f("y"))
        }).collect()
    }

    /// The Student + key export, as the toolbar makes it (`answer_pair`, through the gel's answer rule).
    fn pair(p: &Params) -> (String, String) { crate::figures::answer_pair(&def(), p).unwrap() }
    /// The params each copy is drawn from (the rule turns the Answer key switch off, then on).
    fn copies(p: &Params) -> (Params, Params) {
        let (mut st, mut k) = (p.clone(), p.clone());
        st.set_b("answer", false);
        k.set_b("answer", true);
        (st, k)
    }

    #[test]
    fn gel_no_blank_lane_is_the_old_drawing() {
        let d = def();
        let a = d.svg(&params(&[]));
        assert_eq!(d.svg(&params(&[("blank_lanes", s(""))])), a);
        // a ladder or a lane past the last cannot be blanked, and junk reads as nothing
        for t in ["1", "6", "16", "x, 0, -2, 99", ""] { assert_eq!(d.svg(&params(&[("blank_lanes", s(t))])), a, "{t}"); }
        // no Show answers switch: Student + key goes through the gel's answer rule, as before batch 9
        assert!(!crate::figures::hides_answers(&(d.params)()) && crate::figures::has_answer_pair(&d));
    }

    #[test]
    fn gel_blank_lane_is_empty_on_the_student_copy_and_kept_on_the_key() {
        let lay = |p: &Params| crate::figures::biology::gel::layout_of(p);
        for setup in ["Crime scene", "Paternity test", "Restriction digest", "PCR products"] {
            for look in ["Printable", "Blue stain", "Glowing"] {
                for kind in ["Matching lanes", "Band sizes", "Both"] {
                    for lane in [2usize, 3, 4, 5] {
                        let base = params(&[("setup", s(setup)), ("look", s(look)), ("answer_kind", s(kind))]);
                        let (_, key_plain) = pair(&base);
                        let mut p = base.clone();
                        p.set_s("blank_lanes", &lane.to_string());
                        let (student, key) = pair(&p);
                        let (ps, pk) = copies(&p);
                        let in_lane = |v: &Vec<(f64, f64)>, cx: f64| v.iter().filter(|q| (q.0 - cx).abs() < 1.0).count();
                        let (cs, ck) = (crate::figures::biology::gel::lane_x(&lay(&ps), lane - 1), crate::figures::biology::gel::lane_x(&lay(&pk), lane - 1));
                        // the key: every band and the printed answer, as before, and a dashed outline round the students' lane
                        assert_eq!(bands(&key), bands(&key_plain), "{setup} {kind} lane {lane}: the key keeps its bands");
                        assert!(key.contains("Answer key") && key.contains("data-blank-lane"), "{setup} lane {lane}: the key marks the lane");
                        assert!(in_lane(&bands(&key), ck) > 0, "{setup} {look} lane {lane}");
                        // the student copy: the lane is empty, every other lane's bands drawn (same rows as on the key)
                        assert_eq!(in_lane(&bands(&student), cs), 0, "{setup} lane {lane}: empty for students");
                        let others = |v: Vec<(f64, f64)>, cx: f64| v.into_iter().filter(|q| (q.0 - cx).abs() >= 1.0).map(|q| q.1).collect::<Vec<_>>();
                        assert_eq!(others(bands(&student), cs), others(bands(&key), ck), "{setup} lane {lane}: the other lanes unchanged");
                        // nothing of the answer in the student SVG (the printed key, its title and description)
                        assert!(!student.contains("Answer key") && !student.contains("matches") && !student.contains("data-blank-lane"));
                        // no drag handle on a band the student copy hides
                        assert!(lay(&ps).lanes[lane - 1].bands.is_empty());
                    }
                }
            }
        }
    }

    #[test]
    fn gel_blank_flag_moves_with_its_lane() {
        let mut p = params(&[]);
        let mut v = lane_cards(&p);
        assert!(v.iter().all(|l| !l.blank));
        v[2].blank = true; // Suspect 1
        set_lane_cards(&mut p, &v);
        assert_eq!(p.s("blank_lanes"), "3");
        // moved left, its flag goes with it
        let mut v = lane_cards(&p);
        assert!(v[2].blank && v[2].label == "Suspect 1");
        v.swap(1, 2);
        set_lane_cards(&mut p, &v);
        assert_eq!(p.s("blank_lanes"), "2");
        // a lane before it removed
        let mut v = lane_cards(&p);
        v.remove(0);
        set_lane_cards(&mut p, &v);
        assert_eq!(p.s("blank_lanes"), "1");
        assert_eq!(lane_cards(&p)[0].label, "Suspect 1");
        // a ladder is never blank, and a new lane starts filled in
        let mut v = lane_cards(&p);
        v.push(LaneCard { blank: true, ..LaneCard::ladder() });
        v.push(LaneCard::sample());
        set_lane_cards(&mut p, &v);
        assert_eq!(p.s("blank_lanes"), "1");
        assert!(matches!(lane_cards(&p).last().unwrap().kind, LaneKind::Sample(_)));
    }

    /// Through the workspace's hit test: on the student copy no band handle answers anywhere in the blank lane's
    /// track (nothing to drag, nothing to give the bands away); on the key the lane's bands are draggable as before.
    #[test]
    fn gel_blank_lane_has_no_band_handles_on_the_student_copy() {
        use crate::figures::interact;
        let d = def();
        for lane in [2usize, 4] {
            let (student, key) = copies(&params(&[("blank_lanes", s(&lane.to_string()))]));
            let hits = |p: &Params| {
                let lay = crate::figures::biology::gel::layout_of(p);
                let cx = crate::figures::biology::gel::lane_x(&lay, lane - 1);
                let hs = d.handles(p, (lay.w, 900.0));
                let mut n = 0;
                let mut y = lay.well_y;
                while y < lay.well_y + lay.run + 10.0 {
                    for dx in [-15.0, -8.0, 0.0, 8.0, 15.0] {
                        if let Some(i) = interact::pick(&hs, (cx + dx, y), 1.0) { if hs[i].tip.contains("fragment size") { n += 1; } }
                    }
                    y += 1.0;
                }
                n
            };
            assert!(hits(&key) > 0, "lane {lane}: the key's bands drag");
            assert_eq!(hits(&student), 0, "lane {lane}: nothing under the pointer on the student copy");
        }
    }

    fn fnv(s: &str) -> String { let mut h: u64 = 0xcbf29ce484222325; for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); } format!("{h:016x}") }

    /// The Student + key export of old gels is exactly what it was at e2c68d7 (before batch 9): the opening state,
    /// every example and every classic setup, with the printed key off and on in each of its forms. The pinned hashes
    /// (tests/fixtures/versions_b9_gel_pairs_e2c68d7.txt) were written by that commit's own code, the cases built
    /// exactly as here.
    #[test]
    fn gel_student_and_key_pairs_are_the_old_pairs() {
        use crate::figures::{answer_pair, classic_setups, examples_for};
        let _ = classic_setups;
        let d = def();
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("opening".into(), vec![])];
        for e in examples_for(d.id) { cases.push((format!("example: {}", e.name), e.params)); }
        for st in super::super::setup_names() { cases.push((format!("setup: {st}"), vec![("setup", Value::S(st.to_string()))])); }
        let mut out = String::new();
        for (name, over) in cases {
            for (ak, kind) in [(false, "Matching lanes"), (true, "Matching lanes"), (true, "Band sizes"), (true, "Both")] {
                let mut p: Params = start_params(&d);
                for (k, v) in &over { p.0.insert(k.to_string(), v.clone()); }
                p.set_b("answer", ak);
                if !over.iter().any(|(k, _)| *k == "answer_kind") { p.set_s("answer_kind", kind); }
                p.normalize(&(d.params)(), false);
                let (st, k) = answer_pair(&d, &p).unwrap();
                // (the opening state's student copy without the printed answer, its key with it)
                if name == "opening" && kind != "Band sizes" { assert!(!st.contains("matches") && k.contains("Crime scene matches Suspect 2")); }
                out.push_str(&format!("{name}\t{ak}\t{kind}\t{}\t{}\n", fnv(&st), fnv(&k)));
            }
        }
        let pinned = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/versions_b9_gel_pairs_e2c68d7.txt")).unwrap();
        for (now, was) in out.lines().zip(pinned.lines()) { assert_eq!(now, was, "the Student + key pair changed"); }
        assert_eq!(out.lines().count(), pinned.lines().count());
        assert_eq!(pinned.lines().count(), 40);
    }

    /// Blank lanes also go through Student + key: the student copy empties them, the key outlines them, and the rest of
    /// each copy is the old pair's.
    #[test]
    fn gel_student_and_key_pair_honours_blank_lanes() {
        let base = params(&[]);
        let (st0, k0) = pair(&base);
        let mut p = base.clone();
        p.set_s("blank_lanes", "3");
        let (st, k) = pair(&p);
        assert_ne!(st, st0, "the student copy empties lane 3");
        assert_ne!(k, k0, "the key outlines lane 3");
        assert!(k.contains("Crime scene matches Suspect 2") && !st.contains("matches"));
        assert_eq!(bands(&st).len() + 4, bands(&st0).len(), "Suspect 1's four bands are gone");
        assert_eq!(bands(&k), bands(&k0));
    }
}
