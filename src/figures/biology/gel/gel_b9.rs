//! Gel electrophoresis, batch 9: a sample lane left blank for students. Each sample lane's card has "Leave blank for
//! students"; the lanes ticked are kept as their numbers in `blank_lanes` (rewritten whenever the cards move, so a
//! flag stays with its lane). With Show answers on (the key) a blank lane draws its bands inside a dashed outline that
//! marks it as the students' lane; with it off (the student copy) the lane is an empty well and track, and the
//! printed answer key is left off. Old figures have no blank lanes and Show answers on, so they draw as before.
use super::*;

pub(super) fn params() -> Vec<ParamDef> {
    vec![
        crate::figures::answers_param(),
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
    use crate::figures::{registry, start_params, FigureDef, Params, Value, SHOW_ANSWERS};
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

    #[test]
    fn gel_no_blank_lane_is_the_old_drawing() {
        let d = def();
        let a = d.svg(&params(&[]));
        assert_eq!(d.svg(&params(&[("blank_lanes", s("")), (SHOW_ANSWERS, b(true))])), a);
        // a ladder or a lane past the last cannot be blanked, and junk reads as nothing
        for t in ["1", "6", "16", "x, 0, -2, 99", ""] { assert_eq!(d.svg(&params(&[("blank_lanes", s(t))])), a, "{t}"); }
    }

    #[test]
    fn gel_blank_lane_is_empty_on_the_student_copy_and_kept_on_the_key() {
        let d = def();
        let lay = |p: &Params| crate::figures::biology::gel::layout_of(p);
        for setup in ["Crime scene", "Paternity test", "Restriction digest", "PCR products"] {
            for look in ["Printable", "Blue stain", "Glowing"] {
                for lane in [2usize, 3, 4, 5] {
                    let base = vec![("setup", s(setup)), ("look", s(look)), ("answer", b(true)), ("answer_kind", s("Both"))];
                    let key_plain = d.svg(&params(&base));
                    let mut o = base.clone();
                    o.push(("blank_lanes", s(&lane.to_string())));
                    let key = d.svg(&params(&o));
                    o.push((SHOW_ANSWERS, b(false)));
                    let student = d.svg(&params(&o));
                    let l = lay(&params(&o));
                    // (the two copies share one page, so every lane is at the same place on both)
                    let cx = crate::figures::biology::gel::lane_x(&l, lane - 1);
                    assert_eq!(crate::figures::biology::gel::lane_x(&lay(&params(&base)), lane - 1), cx);
                    assert_eq!(student.split('>').next(), key.split('>').next(), "{setup} lane {lane}: the same page");
                    let in_lane = |v: &Vec<(f64, f64)>| v.iter().filter(|q| (q.0 - cx).abs() < 1.0).count();
                    // the key: every band, as before, and a dashed outline round the students' lane
                    assert_eq!(bands(&key), bands(&key_plain), "{setup} lane {lane}: the key keeps its bands");
                    assert!(key.contains("data-blank-lane"), "{setup} lane {lane}: the key marks the lane");
                    assert!(in_lane(&bands(&key)) > 0, "{setup} {look} lane {lane}: {:?} at {cx}", bands(&key));
                    // the student copy: the lane is empty, every other band where it was
                    assert_eq!(in_lane(&bands(&student)), 0, "{setup} lane {lane}: empty for students");
                    let others: Vec<_> = bands(&key).into_iter().filter(|q| (q.0 - cx).abs() >= 1.0).collect();
                    assert_eq!(bands(&student), others, "{setup} lane {lane}: the other lanes unchanged");
                    // nothing of the answer anywhere in the student SVG (the printed key, its title and description)
                    assert!(!student.contains("Answer key") && !student.contains("matches") && !student.contains("data-blank-lane"));
                    // (the key's band-size line for the lane, "Name: 5200, 3100 bp", is not printed)
                    let name = crate::figures::biology::gel::lane_cards(&params(&base))[lane - 1].label.clone();
                    assert!(key.contains(&format!("{name}: ")) && !student.contains(&format!("{name}: ")), "{setup} lane {lane}: its sizes only on the key");
                    // no drag handle on a band the student copy hides
                    assert!(l.lanes[lane - 1].bands.is_empty());
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

    #[test]
    fn gel_student_copy_hides_the_printed_answer_key_only_when_asked() {
        let d = def();
        // Show answers on is every old figure: the printed key as before
        let key = d.svg(&params(&[("answer", b(true)), ("answer_kind", s("Band sizes"))]));
        assert!(key.contains("Answer key"));
        let student = d.svg(&params(&[("answer", b(true)), ("answer_kind", s("Band sizes")), (SHOW_ANSWERS, b(false))]));
        assert!(!student.contains("Answer key") && !student.contains("5200, 3100"));
        // without blank lanes and without a printed key, the student copy is the key
        assert_eq!(d.svg(&params(&[(SHOW_ANSWERS, b(false))])), d.svg(&params(&[])));
    }

    /// Through the workspace's hit test: on the student copy no band handle answers anywhere in the blank lane's
    /// track (nothing to drag, nothing to give the bands away); on the key the lane's bands are draggable as before.
    #[test]
    fn gel_blank_lane_has_no_band_handles_on_the_student_copy() {
        use crate::figures::interact;
        let d = def();
        for lane in [2usize, 4] {
            let key = params(&[("blank_lanes", s(&lane.to_string()))]);
            let mut student = key.clone();
            student.set_b(SHOW_ANSWERS, false);
            let lay = crate::figures::biology::gel::layout_of(&key);
            let cx = crate::figures::biology::gel::lane_x(&lay, lane - 1);
            let hits = |p: &Params| {
                let hs = d.handles(p, (lay.w, 600.0));
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
}
