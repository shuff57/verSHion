//! Round 5: field-size cost bounds, linear-time formatting checks, the general formatting-key
//! rule, Matching labels, sibling references, bare tokens, and the import order lock.
mod common;
use share_core::labels::key_in_formatting;
use share_core::format::features as look;

/// Correct choice first; everything else wrong.
fn kif(c: &[&str]) -> Vec<usize> {
    key_in_formatting(&c.iter().enumerate().map(|(i, t)| (*t, i == 0)).collect::<Vec<_>>())
}

#[test]
fn decoration_on_the_correct_choice_only_is_refused() {
    for first in [
        "Venus \u{2714}\u{FE0F}", "\u{2611}\u{FE0F} Venus", "Venus \u{2714}", "*Venus", "*a. Venus", "**Venus**", "Venus (Correct!)", "Venus (ans)",
        "Venus - correct", "Venus <--", "Venus <-", "Venus \u{2190}", "Venus \u{2B50}", "Venus \u{1F44D}", "`Venus`", "VENUS", "Venus.",
        "Venus\u{200E}",
    ] {
        assert_eq!(kif(&[first, "Mars", "Earth"]), vec![0], "{first:?}");
    }
    // Round 6: padding and invisible spacing are not served at all (served_text), and an ordinary
    // parenthetical is content.
    for t in ["Venus ", "Venus\u{A0}", "Venus\u{200B}", " Venus", "\tVenus", "Ve\u{AD}nus", "Venus\u{2060}"] {
        assert_eq!(share_core::format::served_text(t), "Venus", "{t:?}");
    }
    // Removing an invisible character can bring a mark next to its base: served NFC again.
    assert_eq!(share_core::format::served_text("e\u{200B}\u{301}"), "\u{E9}");
    // Round 11: a parenthetical on the correct choice alone is a low-risk outlier (the net).
    assert!(share_core::format::check(&[("Venus (Mitosis)", true), ("Mars", false), ("Earth", false)]).iter().all(|f| f.risk == share_core::format::Risk::Low));
    // Pairs that mark both sides: "[x] Venus" / "[ ] Mars", "=Venus" / "~Mars", "+Venus" / "-Mars".
    for (c, w) in [("[x] Venus", "[ ] Mars"), ("=Venus", "~Mars"), ("+Venus", "-Mars"), ("(*) Venus", "( ) Mars")] {
        assert_eq!(kif(&[c, w, &w.replace("Mars", "Earth")]), vec![0], "{c:?}");
    }
    // Several correct choices sharing a decoration no wrong choice has.
    let v = key_in_formatting(&[("*Venus", true), ("*Mars", true), ("Earth", false), ("Pluto", false)]);
    assert_eq!(v, vec![0, 1]);
    // The correct choice lacking a marker every wrong choice has.
    assert_eq!(kif(&["Venus", "~Mars", "~Earth"]), vec![0]);
    // Round 7: punctuation on every wrong choice and not the correct one is an outlier (low risk,
    // the author may acknowledge it).
    let f = share_core::format::check(&[("Venus", true), ("Mars.", false), ("Earth.", false)]);
    assert!(!f.is_empty() && f.iter().all(|x| x.risk == share_core::format::Risk::Low), "{f:?}");
}

#[test]
fn shared_or_uniform_formatting_is_accepted() {
    for set in [
        &["**Venus**", "**Mars**", "**Earth**"][..],
        &["Venus.", "Mars.", "Earth."],
        &["Venus", "**Mars**", "Earth"],
        &["Venus (planet)", "Mars (planet)", "Moon"],
        &["Venus (planet)", "Mars (red)", "Earth (home)"],
        &["*Venus", "*Mars", "Earth"],
        &["VENUS", "MARS", "EARTH"],
        &["-2", "0", "2", "4"],
        &["$5", "$10", "$15"],
        &["5%", "10%", "15%"],
        &["30\u{B0}", "45\u{B0}", "60\u{B0}"],
        &["-x", "x", "2x"],
        &["DNA", "RNA", "ATP"],
        &["x > 2", "x < 2", "x = 2"],
        &["a. Venus", "b. Mars", "c. Earth"],
        &["(a) Venus", "(b) Mars", "(c) Earth"],
        &["Venus - the hot one", "Mars - red", "Earth"],
        &["The cell divides.", "The cell grows.", "The cell dies."],
        &["H2O", "CO2", "NaCl", "O2"],
        &["1/2", "0.5", "2/4", "50%"],
        &["f(x) = 2x", "f(x) = x^2", "f(x) = 1/x"],
        &["O(n log n)", "O(n)", "O(1)", "O(n^2)"],
        &["1945", "1939", "1918", "1914"],
        &["George Washington", "Abraham Lincoln", "Thomas Jefferson"],
        &["print(x)", "echo x", "console.log(x)"],
        &["3 m/s", "9.8 m/s\u{B2}", "0 m/s"],
        &["\u{3C0}/2", "\u{3C0}", "2\u{3C0}"],
        &["NaOH", "HCl", "H2SO4"],
        &["mitosis", "meiosis", "binary fission"],
    ] {
        let mut s = set.to_vec();
        // every position may be the correct one
        for k in 0..s.len() {
            s.rotate_left(1);
            let v = key_in_formatting(&s.iter().enumerate().map(|(i, t)| (*t, i == k)).collect::<Vec<_>>());
            let lone = |f: &dyn Fn(&str) -> bool| s.iter().enumerate().filter(|(_, t)| f(t)).map(|(i, _)| i).collect::<Vec<_>>() == vec![k];
            // Only a feature that isolates the correct one may be refused.
            if lone(&|t| t.starts_with('*') || t.ends_with("**")) || lone(&|t| t.ends_with(')')) || lone(&|t| t.contains(" - ")) || lone(&|t| !t.contains(" - ")) || lone(&|t| !t.ends_with(')')) || lone(&|t| !t.starts_with('*')) {
                continue;
            }
            assert!(v.is_empty(), "{s:?} correct {k}: {v:?}");
        }
    }
}

#[test]
fn look_is_linear_on_hostile_shapes() {
    for n in [1_000usize, 8_000] {
        let t = std::time::Instant::now();
        let s = "*_".repeat(n) + "*";
        let p = "(".repeat(n) + "x" + &")".repeat(n);
        let d = " - x".repeat(n);
        for _ in 0..26 {
            look(&s);
            look(&p);
            look(&d);
        }
        assert!(t.elapsed().as_millis() < 2000, "{n}: {:?}", t.elapsed());
    }
}

use share_core::labels::{SiblingRef, refers_to_siblings, sibling_ref};

fn unlocked(c: &[&str]) -> bool {
    refers_to_siblings(&c.iter().map(|t| (*t, false)).collect::<Vec<_>>())
}

#[test]
fn more_sibling_references_are_found() {
    for t in [
        "Ni A ni B", "А и Б", "А или В", "Sowohl A als auch B", "Tanto A como B", "AとB", "A和B", "A、B", "A, C", "A y C",
        "A et B", "B und C", "Оба", "Both", "A+C",
    ] {
        assert_eq!(sibling_ref(t), Some(SiblingRef::Labels), "{t}");
    }
    for t in ["I, III", "II, III"] {
        assert_eq!(sibling_ref(t), Some(SiblingRef::Weak), "{t}");
    }
    for t in ["Все перечисленные", "Все вышеперечисленные", "Alle Antworten sind richtig", "Todas las anteriores", "Ни один из перечисленных"] {
        assert_eq!(sibling_ref(t), Some(SiblingRef::Above), "{t}");
    }
    for t in ["Venus", "Mars and Venus", "I think so", "x > 0", "Vitamin C"] {
        assert_eq!(sibling_ref(t), None, "{t}");
    }
}

#[test]
fn bare_answers_shuffle_but_labelled_sets_with_references_freeze() {
    for set in [
        &["4", "5", "6", "7"][..],
        &["12", "15", "20", "25"],
        &["-2", "0", "2"],
        &["1/2", "1/3", "1/4"],
        &["x", "y", "z"],
        &["e", "\u{3C0}", "i"],
        &["C", "H", "O", "N"],
        &["II", "IV", "VI"],
        &["True only if x>0", "y"],
        &["2, 3", "1, 2", "-1, 3", "no real roots"],
        &["x and y", "x - y", "x + y", "2x"],
        &["1 and 2", "2 and 3", "1 and 3", "no solution"],
        &["a, b", "b, c", "a, c"],
    ] {
        assert!(!unlocked(set), "{set:?} should shuffle");
    }
    for set in [
        &["A", "B", "C", "D", "Both A and B"][..],
        &["A. Venus", "B. Mars", "C. Earth", "A and C"],
        &["a) 4", "b) 5", "c) 6", "a and b"],
        &["a) 4", "b) 5", "c) 6", "2, 3"],
        &["a) 4", "b) 5", "c) 6", "b"],
        &["I only", "II only", "I and II", "Neither"],
        &["Venus", "Mars", "Earth", "None of the above"],
    ] {
        assert!(unlocked(set), "{set:?} should freeze");
    }
    // A locked "All of the above" keeps its slot; the rest shuffle.
    assert!(!refers_to_siblings(&[("Venus", false), ("Mars", false), ("All of the above", true)]));
}

// ---- G4: Matching right columns -------------------------------------------------------------------

use serde_json::{Value, json};
use share_core::wasm::*;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}

fn matching(pairs: &[(&str, &str)], distractors: &[&str]) -> String {
    json!({"schema_version": 1, "question": {"id":"q1","stem":"Match","kind":{"Matching":{"pairs": pairs.iter().map(|(l,r)| json!({"left":l,"right":r})).collect::<Vec<_>>(),"distractors":distractors}},"points":1.0,"difficulty":null,"created":0,"modified":0}}).to_string()
}

#[test]
fn matching_rights_with_position_labels_need_strip() {
    let p = matching(&[("1. Paris", "a. France"), ("2. Madrid", "b. Spain"), ("3. Rome", "c. Italy")], &["d. Peru"]);
    let f = j(&label_findings_json(p.as_bytes(), ""));
    assert_eq!(f["label_findings"][0]["matching"], true, "{f}");
    assert_eq!(f["label_findings"][0]["order_revealing"], true);
    assert_eq!(f["label_findings"][0]["choices"][0]["after"], "France");
    // No decision, Keep: refused (the left column never moves, so Keep would publish the key).
    let r = j(&strip_json(p.as_bytes(), ""));
    assert_eq!(r["errors"][0]["code"], "labels_need_decision", "{r}");
    let keep = r#"{"labels":[{"question":0,"choice":"keep"}]}"#;
    assert_eq!(j(&strip_json(p.as_bytes(), keep))["errors"][0]["code"], "labels_keep_refused");
    assert_eq!(j(&validate_for_publish_json(p.as_bytes(), &format!(r#"{{"hide_key":true,{}"#, &keep[1..])))["errors"][0]["code"], "labels_keep_refused");
    // Strip: the bank carries no labels, and the hash of the author's form with the decision is
    // the hash of what was served.
    let strip = r#"{"labels":[{"question":0,"choice":"strip"}]}"#;
    let v = j(&validate_for_publish_json(p.as_bytes(), &format!(r#"{{"hide_key":true,{}"#, &strip[1..])));
    assert_eq!(v["ok"], true, "{v}");
    let o = j(&common::strip_ok(p.as_bytes(), strip));
    let mut bank: Vec<String> = o["payload"]["question"]["kind"]["Matching"]["distractors"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
    bank.sort();
    assert_eq!(bank, ["France", "Italy", "Peru", "Spain"]);
    let full = share_core::validate(&p, &share_core::Limits::default(), &share_core::Structural).unwrap();
    let served = share_core::validate(&o["payload"].to_string(), &share_core::Limits::default(), &share_core::Structural).unwrap();
    let labels: share_core::strip::QuestionLabels = [(None, share_core::labels::LabelChoice::Strip)].into_iter().collect();
    assert_eq!(share_core::hash::match_hash_with(&full.payload.questions()[0], &labels), share_core::hash::match_hash(&served.payload.questions()[0]));
    // Labels that are part of the text and not in position order: Keep is fine.
    let p = matching(&[("Bacterium", "E. coli"), ("Yeast", "S. cerevisiae")], &[]);
    let f = j(&label_findings_json(p.as_bytes(), ""));
    assert_eq!(f["label_findings"][0]["order_revealing"], false, "{f}");
    assert_eq!(j(&strip_json(p.as_bytes(), r#"{"labels":[{"question":0,"choice":"keep"}]}"#))["ok"], true);
}

#[test]
fn matching_rights_naming_their_left_or_decorated_are_refused() {
    let p = matching(&[("1. Paris", "France (1)"), ("2. Madrid", "Spain (2)"), ("3. Rome", "Italy (3)")], &[]);
    // Order-revealing suffix labels: Keep refused, Strip allowed.
    assert_eq!(j(&strip_json(p.as_bytes(), r#"{"labels":[{"question":0,"choice":"keep"}]}"#))["errors"][0]["code"], "labels_keep_refused");
    assert_eq!(j(&common::strip_ok(p.as_bytes(), r#"{"labels":[{"question":0,"choice":"strip"}]}"#))["ok"], true);
    // Scrambled labels that name the left item: the label pairs them whatever the order.
    let p = matching(&[("c. Paris", "France (c)"), ("a. Madrid", "Spain (a)"), ("b. Rome", "Italy (b)")], &[]);
    let r = j(&strip_json(p.as_bytes(), r#"{"labels":[{"question":0,"choice":"keep"}]}"#));
    assert!(r["errors"].as_array().unwrap().iter().any(|e| e["code"] == "key_in_formatting" && e["path"].as_str().unwrap().ends_with("Matching.pairs[0].right")), "{r}");
    let v = j(&validate_for_publish_json(p.as_bytes(), r#"{"hide_key":true,"labels":[{"question":0,"choice":"keep"}]}"#));
    assert!(v["errors"].as_array().unwrap().iter().any(|e| e["code"] == "key_in_formatting"), "{v}");
    // Paired rights decorated, distractors not.
    let p = matching(&[("Paris", "*France"), ("Madrid", "*Spain")], &["Italy", "Peru"]);
    let r = j(&strip_json(p.as_bytes(), ""));
    assert_eq!(r["errors"][0]["code"], "key_in_formatting", "{r}");
    assert!(r["errors"][0]["path"].as_str().unwrap().ends_with("Matching.pairs[0].right"), "{r}");
    // Look-alike rights count once in the column and the bank.
    let p = matching(&[("Paris", "France"), ("Lyon", "France ")], &["Peru"]);
    let o = j(&common::strip_ok(p.as_bytes(), ""));
    assert_eq!(o["payload"]["question"]["kind"]["Matching"]["distractors"].as_array().unwrap().len(), 2, "{o}");
}

// ---- G7: imports keep a kept order; a lost keyless flag cannot publish a placeholder -------------

fn mcq(texts: &[&str], correct: usize) -> String {
    json!({"schema_version": 2, "question": {"id":"q1","stem":"Pick","kind":{"Mc":{"answers": texts.iter().enumerate().map(|(i,t)| json!({"text":t,"correct":i == correct})).collect::<Vec<_>>()}},"points":1.0,"difficulty":null,"created":0,"modified":0}}).to_string()
}

#[test]
fn a_kept_order_is_served_locked_and_imported_locked() {
    for (p, why) in [
        (mcq(&["Mitochondria", "Ribosome", "Both A and B", "Nucleus"], 2), "sibling_refs"),
        (mcq(&["A) Venus", "B) Mars", "C) Earth"], 0), "labels_kept"),
    ] {
        let opts = r#"{"labels":[{"question":0,"choice":"keep"}]}"#;
        let opts = if why == "labels_kept" { opts } else { "" };
        let o = j(&common::strip_ok(p.as_bytes(), opts));
        assert_eq!(o["order_reasons"][0]["reason"], why, "{o}");
        let served = share_core::validate(&o["payload"].to_string(), &share_core::Limits::default(), &share_core::Structural).unwrap();
        let q = share_core::import::localize(&served.payload.questions()[0]);
        let share_core::Kind::Mc { answers } = &q.kind else { panic!() };
        assert!(answers.iter().all(|a| a.locked), "{why}: every choice locked: {answers:?}");
    }
    // A shuffled question is served as it was (nothing locked that the author did not lock).
    let o = j(&common::strip_ok(mcq(&["Venus", "Mars", "Earth"], 0).as_bytes(), ""));
    assert!(o["payload"]["question"]["kind"]["Mc"]["answers"].as_array().unwrap().iter().all(|a| a.get("locked").is_none_or(|l| l == false)), "{o}");
    // keep_order on the question: localize locks every choice, parts included.
    let mut q = common::question("Order", share_core::Kind::Multipart { parts: vec![share_core::Part { stem: "p".into(), kind: share_core::PartKind::Mc { answers: vec![common::answer("x", false), common::answer("y", false)] } }] });
    q.keep_order = true;
    let l = share_core::import::localize(&q);
    let share_core::Kind::Multipart { parts } = &l.kind else { panic!() };
    let share_core::PartKind::Mc { answers } = &parts[0].kind else { panic!() };
    assert!(answers.iter().all(|a| a.locked));
}

#[test]
fn a_true_false_placeholder_from_a_stripped_copy_never_publishes() {
    let origin = json!({"item_id":"i","qid":"q","publish_id":"p","version":1,"author_name":"a","license":"CC-BY-4.0","form":"stripped"});
    let tf = |correct: bool, extra: Value| {
        let mut q = json!({"id":"q1","stem":"The Sun is a star.","kind":{"Tf":{"correct":correct}},"points":1.0,"difficulty":null,"created":0,"modified":0,"origin":origin});
        for (k, v) in extra.as_object().unwrap() {
            q[k] = v.clone();
        }
        json!({"schema_version": 2, "question": q}).to_string()
    };
    // keyless flag and sentinel lost, false still the placeholder: refused, with or without hide_key.
    for o in ["", r#"{"hide_key":true}"#] {
        let r = j(&validate_for_publish_json(tf(false, json!({})).as_bytes(), o));
        assert_eq!(r["errors"][0]["code"], "placeholder_key", "{r}");
        assert_eq!(r["errors"][0]["path"], "question.kind.Tf");
    }
    // Re-keyed (the editor sets it when the teacher picks an answer): fine, False included.
    assert_eq!(j(&validate_for_publish_json(tf(false, json!({"rekeyed": true})).as_bytes(), ""))["ok"], true);
    // True cannot be the placeholder.
    assert_eq!(j(&validate_for_publish_json(tf(true, json!({})).as_bytes(), ""))["ok"], true);
    // v1 cannot carry the flag.
    let v1 = json!({"schema_version": 1, "question": {"id":"q1","stem":"s","kind":{"Tf":{"correct":true}},"points":1.0,"difficulty":null,"created":0,"modified":0,"rekeyed":true}}).to_string();
    assert_eq!(j(&validate_json(v1.as_bytes(), ""))["errors"][0]["code"], "v2_field_in_v1");
    // Strip clears it.
    let o = j(&common::strip_ok(tf(true, json!({"rekeyed": true})).as_bytes(), ""));
    assert!(o["payload"]["question"].get("rekeyed").is_none(), "{o}");
}

// ---- non-blocking: label forms, text rules ---------------------------------------------------------

#[test]
fn more_label_forms_are_found_and_stripped() {
    for set in [
        ["A.) Venus", "B.) Mars", "C.) Earth"],
        ["1.) Venus", "2.) Mars", "3.) Earth"],
        ["A\tVenus", "B\tMars", "C\tEarth"],
        ["\u{2022} A. Venus", "\u{2022} B. Mars", "\u{2022} C. Earth"],
        ["- A. Venus", "- B. Mars", "- C. Earth"],
    ] {
        let a = share_core::labels::analyze(&set).unwrap_or_else(|| panic!("{set:?}"));
        assert!(a.order_revealing && a.strip_allowed, "{set:?}");
        let after: Vec<String> = a.previews(&set).into_iter().map(|p| p.after).collect();
        assert_eq!(after, ["Venus", "Mars", "Earth"], "{set:?}");
    }
    for set in [["- 2", "- 3", "4"], ["* Venus", "* Mars", "Earth"]] {
        assert!(share_core::labels::analyze(&set).is_none(), "{set:?}");
    }
}

fn tf_stem(stem: &str) -> Value {
    j(&validate_json(json!({"schema_version": 1, "question": {"id":"q","stem":stem,"kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"created":1,"modified":1}}).to_string().as_bytes(), ""))
}

#[test]
fn text_rules_round_5() {
    let run = "\u{301}".repeat(16);
    // Format characters neither count as marks nor end a run.
    for sep in ["\u{200D}", "\u{200E}", "\u{200F}", "\u{200B}", "\u{2060}", "\u{AD}", "\u{2062}"] {
        let r = tf_stem(&format!("a{}?", (run.clone() + sep).repeat(3)));
        assert_eq!(r["errors"][0]["code"], "mark_run", "{sep:?}: {r}");
    }
    // Variation selectors: one, after a base that can take it.
    for ok in ["\u{2714}\u{FE0F} yes", "1\u{FE0F}\u{20E3}", "\u{2764}\u{FE0E}", "\u{8FBB}\u{E0100}", "\u{2229}\u{FE00}"] {
        assert_eq!(tf_stem(ok)["ok"], true, "{ok:?}");
    }
    let hidden: String = "KEY=B".bytes().map(|b| char::from_u32(if b < 16 { 0xFE00 + b as u32 } else { 0xE0100 + b as u32 - 16 }).unwrap()).collect();
    for bad in [format!("Pick one{hidden}"), "Venus\u{FE0F}".into(), "\u{2714}\u{FE0F}\u{FE0F}".into(), "a\u{E0100}".into(), "a\u{301}\u{FE0F}".into(), "\u{1D160}\u{FE0F}".into()] {
        assert_eq!(tf_stem(&bad)["errors"][0]["code"], "forbidden_char", "{bad:?}");
    }
    // Word Symbol-font characters: mapped (and reported), or refused with a precise code.
    let r = tf_stem("x \u{F0B1} 2, \u{F0A3} 3, \u{F070}r\u{F0B2}");
    assert_eq!(r["payload"]["question"]["stem"], "x \u{B1} 2, \u{2264} 3, \u{3C0}r\u{2033}", "{r}");
    assert_eq!(r["dropped"][0]["reason"], "symbol_font_mapped");
    assert_eq!(tf_stem("x \u{F0FF}")["errors"][0]["code"], "symbol_font_char");
    // An object Word could not export.
    let r = tf_stem("Solve \u{FFFC} for x");
    assert_eq!(r["errors"][0]["code"], "object_replacement");
    assert_eq!(r["errors"][0]["offset"], 6);
    // A field over 4x its limit is refused before any character is read.
    assert_eq!(tf_stem(&"a".repeat(4 * 8192 + 65))["errors"][0]["code"], "too_long");
    assert_eq!(tf_stem(&format!("{}\u{202E}", "a".repeat(4 * 8192 + 65)))["errors"][0]["code"], "too_long");
    // Offsets are still the input's after CRLF and BOMs (no copy is made to compute them).
    let r = tf_stem("\u{FEFF}a\r\nb\u{202E}");
    assert_eq!((r["errors"][0]["offset"].as_u64(), r["errors"][0]["offset16"].as_u64()), (Some(5), Some(5)), "{r}");
}

#[test]
fn work_sizing_limits_can_only_be_lowered() {
    let d = share_core::Limits::default();
    let up = share_core::Limits { mc_answers: 10_000, matching_pairs: 10_000, matching_distractors: 10_000, parts: 10_000, blanks: 10_000, accept_variants: 10_000, questions_per_bank: 100_000, topics: 10_000, stem_bytes: 1 << 30, explanation_bytes: 1 << 30, answer_bytes: 1 << 30, ..d.clone() }.sane();
    assert_eq!((up.mc_answers, up.matching_pairs, up.matching_distractors, up.parts, up.blanks, up.accept_variants), (d.mc_answers, d.matching_pairs, d.matching_distractors, d.parts, d.blanks, d.accept_variants));
    assert_eq!((up.questions_per_bank, up.topics, up.stem_bytes, up.explanation_bytes, up.answer_bytes), (d.questions_per_bank, d.topics, d.stem_bytes, d.explanation_bytes, d.answer_bytes));
    let down = share_core::Limits { mc_answers: 4, ..d.clone() }.sane();
    assert_eq!(down.mc_answers, 4);
    // Through the wasm options too.
    let p = json!({"schema_version":1,"question":{"id":"q","stem":"s","kind":{"Mc":{"answers":(0..27).map(|i| json!({"text": format!("c{i}"), "correct": i == 0})).collect::<Vec<_>>()}},"points":null,"difficulty":null,"created":1,"modified":1}}).to_string();
    assert_eq!(j(&validate_json(p.as_bytes(), r#"{"limits":{"mc_answers":100}}"#))["errors"][0]["code"], "too_many");
}

/// The round-4 decoration shape (36 s through the Worker): every exported call is linear in the
/// choice length (doubling it at most ~2.6x the time, quadratic would be 4x) and fast.
#[test]
fn formatting_checks_are_linear() {
    let deco = |n: usize| {
        let qs: Vec<String> = (0..77)
            .map(|i| {
                let a: Vec<Value> = (0..26).map(|k| json!({"text": format!("{}w{k}{}*", "*_".repeat(n), "*_".repeat(n)), "correct": k == 0})).collect();
                json!({"id": format!("q{i}"), "stem":"s","kind":{"Mc":{"answers":a}},"points":null,"difficulty":null,"created":1,"modified":1}).to_string()
            })
            .collect();
        format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, qs.join(","))
    };
    let time = |p: &str| {
        (0..3)
            .map(|_| {
                let t = std::time::Instant::now();
                let _ = validate_for_publish_json(p.as_bytes(), r#"{"hide_key":true}"#);
                let _ = strip_json(p.as_bytes(), "");
                let _ = label_findings_json(p.as_bytes(), "");
                t.elapsed().as_secs_f64()
            })
            .fold(f64::MAX, f64::min)
    };
    let (small, big) = (deco(127), deco(254));
    assert!(big.len() < 4 * 1024 * 1024);
    let (a, b) = (time(&small), time(&big));
    let bound = if cfg!(debug_assertions) { 60.0 } else { 3.0 };
    assert!(b < bound, "{b:.2} s");
    assert!(b / a < 2.6 || b < 0.05, "doubling the choice length: {a:.3} s -> {b:.3} s");
}

// ---- mutation-table gaps (round 5) ------------------------------------------------------------------

/// Native strip, which does not go through `validate_for_publish`: its own checks must hold.
fn native_strip(p: &str, labels: Vec<share_core::strip::LabelDecision>) -> Result<share_core::strip::Stripped, Vec<share_core::ValidationError>> {
    let v = share_core::validate(p, &share_core::Limits::default(), &share_core::Structural).unwrap();
    share_core::strip::strip_with_seed(v.payload, &share_core::strip::StripOptions { image_ok: vec![], labels, ..Default::default() }, [3; 32])
}

fn decision(choice: share_core::labels::LabelChoice) -> Vec<share_core::strip::LabelDecision> {
    vec![share_core::strip::LabelDecision { question: 0, part: None, choice }]
}

#[test]
fn native_strip_keeps_its_own_checks() {
    use share_core::labels::LabelChoice::{Keep, Strip};
    let codes = |r: Result<share_core::strip::Stripped, Vec<share_core::ValidationError>>| r.err().unwrap_or_default().into_iter().map(|e| e.code).collect::<Vec<_>>();
    // Strip where not allowed (labels not exactly 1, 2, 3).
    let p = mcq(&["A. Venus", "C. Mars", "D. Earth"], 0);
    assert!(codes(native_strip(&p, decision(Strip))).contains(&"labels_strip_refused"));
    // Matching: Keep of revealing labels, and rights naming their left.
    let p = matching(&[("1. Paris", "a. France"), ("2. Madrid", "b. Spain")], &[]);
    assert!(codes(native_strip(&p, decision(Keep))).contains(&"labels_keep_refused"));
    let p = matching(&[("c. Paris", "France (c)"), ("a. Madrid", "Spain (a)"), ("b. Rome", "Italy (b)")], &[]);
    assert!(codes(native_strip(&p, decision(Keep))).contains(&"key_in_formatting"));
}

#[test]
fn formatting_rule_details() {
    // A key word in a parenthetical when every choice has one.
    assert_eq!(kif(&["Venus (correct)", "Mars (red)", "Earth (blue)"]), vec![0]);
    // One wrong choice: its decoration is allowed (the correct one merely lacks it).
    assert!(kif(&["Venus", "Mars."]).is_empty());
    // Qualified lists are references by themselves.
    assert_eq!(sibling_ref("both a and b"), Some(SiblingRef::Labels));
    assert_eq!(sibling_ref("only 2"), Some(SiblingRef::Labels));
}

#[test]
fn rekeyed_round_trips_and_selector_offsets_are_the_inputs() {
    let p = json!({"schema_version": 2, "question": {"id":"q1","stem":"s","kind":{"Tf":{"correct":true}},"points":1.0,"difficulty":null,"created":0,"modified":0,"rekeyed":true}}).to_string();
    assert_eq!(j(&validate_json(p.as_bytes(), ""))["payload"]["question"]["rekeyed"], true);
    // U+1D160 is a note plus marks after NFC: the selector after it is refused at its own offset.
    let r = tf_stem("\u{1D160}\u{FE0F}");
    assert_eq!((r["errors"][0]["code"].as_str(), r["errors"][0]["offset"].as_u64()), (Some("forbidden_char"), Some(1)), "{r}");
}
