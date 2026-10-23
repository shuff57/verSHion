//! Round 18 (critic34b robustness, critic34a Moodle):
//! * B1: the bank's cheap answer-line prefilter folds whitespace like the scan it gates, so a
//!   tab or a run of spaces inside a multi-word lead does not turn the whole cross-question scan
//!   off.
//! * B2: a dichotomous key's couplet labels ("1a", "2b") are part labels, not question 1's
//!   choice A: the canonical worksheet publishes whatever the correct choice's position. Real
//!   numbered key lists still match.
//! * B3: a Multipart part's answers (Mc key texts, Short samples, True/False values) and its
//!   correct positions are in the bank key-list scans, both the value set and the order slots.
//! * B4: a Matching question's pairings and rights are read in bank texts: a listed pair block and
//!   a fixed-order rights list. The same strings in the Matching stem are still refused.
use serde_json::{Value, json};
use share_core::wasm::*;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}
fn pp(p: &str, o: &str) -> Value {
    j(&publish_prepare_json(p.as_bytes(), o))
}
fn qv(stem: &str, kind: Value) -> Value {
    json!({"id":"q10000001","stem":stem,"kind":kind,"points":1.0,"difficulty":null,"topics":[],"created":0,"modified":0})
}
fn one(q: Value) -> String {
    json!({"schema_version": 2, "question": q}).to_string()
}
fn bank(qs: Vec<Value>, name: &str, desc: &str) -> String {
    let qs: Vec<Value> = qs.into_iter().enumerate().map(|(i, mut q)| { q["id"] = json!(format!("q{:08}", i + 1)); q }).collect();
    json!({"schema_version":2,"bank":{"id":"b00000001","name":name,"created":0,"modified":0,"details":{"author":"T","description":desc,"license_name":"CC BY 4.0","license_url":"https://example.com/licence"},"questions":qs}}).to_string()
}
fn mc(texts: &[&str], correct: usize) -> Value {
    json!({"Mc": {"answers": texts.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": i == correct, "locked": false})).collect::<Vec<_>>()}})
}
fn matching(lefts: &[&str], rights: &[&str], distractors: &[&str]) -> Value {
    json!({"Matching": {"pairs": lefts.iter().zip(rights).map(|(l, r)| json!({"left": l, "right": r})).collect::<Vec<_>>(), "distractors": distractors}})
}
fn multipart(parts: Vec<Value>) -> Value {
    qv("Read the data.", json!({"Multipart": {"parts": parts}}))
}
fn pmc(stem: &str, texts: &[&str], correct: usize) -> Value {
    json!({"stem": stem, "kind": mc(texts, correct)})
}
fn psh(stem: &str, sample: &str) -> Value {
    json!({"stem": stem, "kind": {"Short": {"sample": sample}}})
}
fn has(r: &Value, reason: &str) -> bool {
    r["errors"].as_array().is_some_and(|e| e.iter().any(|x| x["reason"] == reason))
}
fn reasons(r: &Value) -> Vec<String> {
    r["errors"].as_array().map(|e| e.iter().map(|x| x["reason"].as_str().unwrap_or(x["code"].as_str().unwrap_or("?")).to_string()).collect()).unwrap_or_default()
}
const HK: &str = r#"{"hide_key":true}"#;

/// critic34b B1: the prefilter (`format::may_have_answer_line`) must not turn off the
/// cross-question bank scan for a lead typed with a tab or a run of spaces. Everything the scan
/// reads (`stem_key` on the served, folded text) collapses those runs, so all of these leak
/// before the fix.
#[test]
fn b1_whitespace_in_a_lead_does_not_disable_the_bank_scan() {
    // (The two multi-word leads that no single-word lead covers: "model answer" and
    // "acceptable answer" contain "answer", so they were read even before.)
    let leads = ["mark scheme", "indicative content"];
    // (Separators the scanner folds into one space. `\n` and `\r` stay line breaks: the scan does
    // not join a lead across them, so they are not leaks.)
    let seps = ["  ", "   ", "\t", " \t"];
    // (The served fields the bank reads: stem, Mc choice, Matching left, Matching right,
    // distractor, part stem, part choice.)
    let fields = ["stem", "choice", "left", "right", "distractor", "part stem", "part choice"];
    let mut checked = 0;
    for lead in leads {
        for sep in seps {
            let leak = format!("Notes.\n{lead}{sep}: Zebra");
            for field in fields {
                let q0 = qv("Name it.", mc(&["Zebra", "Lion"], 0));
                let q1 = match field {
                    "stem" => qv(&leak, json!({"Tf": {"correct": true}})),
                    "choice" => qv("Pick one.", mc(&["Lion", &leak], 0)),
                    "left" => qv("Match.", matching(&[&leak, "Quartz"], &["Aluminium", "Soft"], &["Granite"])),
                    "right" => qv("Match.", matching(&["Bauxite", "Chalk"], &[&leak, "Soft"], &["Granite"])),
                    "distractor" => qv("Match.", matching(&["Bauxite", "Chalk"], &["Aluminium", "Soft"], &[&leak])),
                    "part stem" => multipart(vec![pmc(&leak, &["Lion", "Tiger"], 0), psh("Why?", "because")]),
                    "part choice" => multipart(vec![pmc("Pick one.", &["Lion", &leak], 0), psh("Why?", "because")]),
                    _ => unreachable!(),
                };
                let r = pp(&bank(vec![q0, q1], "B", ""), HK);
                assert!(has(&r, "answer_line"), "{lead:?} {sep:?} {field}: {}", r.to_string().chars().take(300).collect::<String>());
                checked += 1;
            }
        }
    }
    assert_eq!(checked, leads.len() * seps.len() * fields.len());

    // Controls. One space was always read.
    let r = pp(&bank(vec![qv("Name it.", mc(&["Zebra", "Lion"], 0)), qv("Notes.\nmark scheme: Zebra", json!({"Tf": {"correct": true}}))], "B", ""), HK);
    assert!(has(&r, "answer_line"), "one space");
    // A folded lead with no key after it is not a false positive.
    let r = pp(&bank(vec![qv("Name it.", mc(&["Zebra", "Lion"], 0)), qv("Notes.\nmark\tscheme: Walrus", json!({"Tf": {"correct": true}}))], "B", ""), HK);
    assert_eq!(r["ok"], true, "no key after the folded lead: {}", r.to_string().chars().take(300).collect::<String>());
    // The same text as a lone question whose own key is Zebra was always read (it does not use
    // the prefilter).
    let r = pp(&one(qv("Notes.\nmark  scheme: Zebra", mc(&["Zebra", "Lion"], 0))), HK);
    assert!(has(&r, "answer_line"), "lone question");
}

/// critic34b B2: a dichotomous key's `1a`/`1b` couplets under a `Key` heading are content, not a
/// numbered answer key. The canonical worksheet publishes for every position of the correct
/// choice; real numbered key lists ("1 A 2 B", "1. A", "1) A", "1 - A", "1B 2T", "4B") still match.
#[test]
fn b2_dichotomous_couplet_labels_are_not_a_numbered_key() {
    let dk = "Use the key to identify the invertebrate.\n\nKey\n1a  Has six legs ................ go to 2\n1b  Has more than six legs ...... go to 3\n2a  Has wings ................... Fly\n2b  No wings .................... Springtail\n3a  Eight legs .................. Spider\n3b  Many legs ................... Centipede\n\nAn animal has eight legs. What is it?";
    let choices = ["Spider", "Fly", "Springtail", "Centipede"];
    // Every correct position (the bug depended on A being the correct choice).
    for (i, ch) in choices.iter().enumerate() {
        let r = pp(&one(qv(dk, mc(&choices, i))), HK);
        assert_eq!(r["ok"], true, "correct={ch}: {}", r.to_string().chars().take(400).collect::<String>());
    }
    // Every couplet label, with the matching correct position: `a` labels the first leaf (Fly),
    // `b` the second (Ant). The letter attached to its number is a part label, never a choice.
    for (n, a, b) in [("1", "Fly", "Ant"), ("2", "Wasp", "Beetle"), ("3", "Moth", "Louse")] {
        let key = format!("Key\n{n}a  Has wings ........ {a}\n{n}b  No wings ........ {b}\nWhat is it?");
        for (c, correct) in [(a, 0), (b, 1)] {
            let r = pp(&one(qv(&key, mc(&[a, b], correct))), HK);
            assert_eq!(r["ok"], true, "{n}a/{n}b, {c}: {}", r.to_string().chars().take(300).collect::<String>());
        }
    }
    // A two-space separator after the couplet label is the same form.
    let simple = "Key\n1a  Has wings ........ Fly\n1b  No wings ........ Ant\nWhat is it?";
    assert_eq!(pp(&one(qv(simple, mc(&["Fly", "Ant"], 0))), HK)["ok"], true, "simple key");

    // Real numbered key lists still match (the forms the code read before this round). A
    // separated value is a choice letter; the couplet rule only looks at a letter glued to its
    // number and followed by a word.
    let ch = ["Butt joint", "Dovetail joint", "Lap joint", "Mitre joint"];
    assert!(has(&pp(&one(qv("Which joint? Answers: 1C 2B 3D", mc(&ch, 1))), HK), "answer_line"), "1C 2B 3D");
    for d in ["Key: 1. A 2. B", "Key: 1) A 2) B", "Key: 1 - A 2 - B"] {
        let b = bank(vec![qv("Which joint?", mc(&ch, 1)), qv("Wood is a polymer.", json!({"Tf": {"correct": true}}))], "B", d);
        assert!(has(&pp(&b, HK), "answer_line"), "{d}");
    }
    // A glued key list in a run ("1B 2T", "4B") is still a key: the value is not followed by a
    // description word.
    for d in ["Answer key: 1B 2T", "Key: 1-B, 2-True"] {
        let b = bank(vec![qv("Which joint?", mc(&ch, 1)), qv("Wood is a polymer.", json!({"Tf": {"correct": true}}))], "B", d);
        let r = pp(&b, HK);
        assert!(has(&r, "bank_key_list") || has(&r, "answer_line"), "{d}: {}", r.to_string().chars().take(300).collect::<String>());
    }
    assert!(has(&pp(&one(qv("Which joint?  [Key: 4B]", mc(&ch, 1))), HK), "answer_line"), "[Key: 4B]");
    // The controls the critic ran: no `Key` line, no lead at all, a `Step 1a` label.
    for stem in [dk.replace("\n\nKey\n", "\n\n"), "1a  Has wings ........ Fly\n1b  No wings ........ Ant\nWhat is it?".to_string(), dk.replace("1a  ", "Step 1a  ")] {
        let r = pp(&one(qv(&stem, mc(&choices, 0))), HK);
        assert_eq!(r["ok"], true, "{}", r.to_string().chars().take(300).collect::<String>());
    }
}

/// critic34a B1: a Multipart question's part answers are key material like any other kind's.
/// The key-list scan reads the part's correct texts, samples and True/False values, and its
/// correct positions when students see the order; `bank_order_errors` has a slot per part.
#[test]
fn b3_multipart_part_answers_are_bank_key_material() {
    // Three Multipart questions, part (a) keyed Quartz / Basalt / Mohs.
    let mp = |key: &str| multipart(vec![pmc("Which mineral?", &[key, "Talc", "Gypsum"], 0), psh("Why?", "because")]);
    let qs = || vec![mp("Quartz"), mp("Basalt"), mp("Mohs")];
    for d in ["1 Quartz 2 Basalt 3 Mohs", "Check your work. 1 Quartz 2 Basalt 3 Mohs"] {
        let mut v = qs();
        if d.starts_with("Check") {
            v.push(qv(d, json!({"Short": {"sample": "x"}})));
            assert!(has(&pp(&bank(v, "B", ""), HK), "bank_key_list"), "stem: {d}");
        } else {
            assert!(has(&pp(&bank(v, "B", d), HK), "bank_key_list"), "description: {d}");
        }
    }
    // The same description with a part's Short sample.
    let short_mp = |s: &str| multipart(vec![psh("Name it.", s), pmc("Pick.", &["a", "b"], 0)]);
    let r = pp(&bank(vec![short_mp("Quartz"), short_mp("Basalt"), short_mp("Mohs")], "B", "1 Quartz 2 Basalt 3 Mohs"), HK);
    assert!(has(&r, "bank_key_list"), "part samples: {}", r.to_string().chars().take(300).collect::<String>());
    // True/False parts ("1 T 2 F 3 T").
    let tf_mp = |v: bool| multipart(vec![json!({"stem": "True or false?", "kind": {"Tf": {"correct": v}}}), psh("Why?", "because")]);
    let r = pp(&bank(vec![tf_mp(true), tf_mp(false), tf_mp(true)], "B", "1 T 2 F 3 T"), HK);
    assert!(has(&r, "bank_key_list"), "part Tf: {}", r.to_string().chars().take(300).collect::<String>());

    // Correct positions of a part: a fixed order, part (a) is C / B / D.
    let letter_mp = |c: usize| {
        let mut q = multipart(vec![pmc("Part one.", &["alpha", "beta", "gamma", "delta"], c), psh("Part two.", "x")]);
        q["keep_order"] = json!(true);
        q
    };
    let b = bank(vec![letter_mp(2), letter_mp(1), letter_mp(3)], "B", "1 C 2 B 3 D");
    let r = pp(&b, HK);
    assert!(has(&r, "bank_key_list"), "part letters before ack: {}", r.to_string().chars().take(300).collect::<String>());
    // (After the order acknowledgement too: the check is not an order finding.)
    let acked = json!({"hide_key": true, "order_acks": [{"question": 0}, {"question": 1}, {"question": 2}]}).to_string();
    assert!(has(&pp(&b, &acked), "bank_key_list"), "part letters after ack");

    // Controls: the same description with top-level Short / Mc / Blank keys is still refused.
    let sh = |s: &str| qv("Name it.", json!({"Short": {"sample": s}}));
    assert!(has(&pp(&bank(vec![sh("Quartz"), sh("Basalt"), sh("Mohs")], "B", "1 Quartz 2 Basalt 3 Mohs"), HK), "bank_key_list"), "Short control");
    let mq = |k: &str| qv("Pick.", mc(&[k, "Talc"], 0));
    assert!(has(&pp(&bank(vec![mq("Quartz"), mq("Basalt"), mq("Mohs")], "B", "1 Quartz 2 Basalt 3 Mohs"), HK), "bank_key_list"), "Mc control");
}

/// critic34a B2: a Matching question's pairing is key material in bank texts, not only in its own
/// stem. The key-list form (three questions' rights in question order), the pair block (dash
/// pairs, the GIFT "=Bauxite -> Aluminium" lines) and the fixed-order rights list are all refused;
/// a shuffled question's rights list names nothing.
#[test]
fn b4_matching_pairs_and_rights_are_read_in_bank_texts() {
    let pairs = (["Bauxite", "Chalk", "Slate"], ["Aluminium", "Soft", "Roof"]);
    let m = |keep: bool| {
        let mut q = qv("Match each material.", matching(&pairs.0, &pairs.1, &["Granite"]));
        q["keep_order"] = json!(keep);
        q
    };
    let short = qv("Name a material.", json!({"Short": {"sample": "Granite"}}));
    // The key-list form: three questions' rights in question order. (A Matching question needs
    // two pairs; the second pair of each is unrelated filler.)
    let rights_qs = |r: &str, l: &str| qv("Match.", matching(&[l, "Filler"], &[r, "Other"], &[]));
    let r = pp(&bank(vec![rights_qs("Mammal", "Lion"), rights_qs("Fish", "Trout"), rights_qs("Amphibian", "Frog")], "B", "Key: Mammal, Fish, Amphibian"), HK);
    assert!(has(&r, "bank_key_list"), "rights in question order: {}", r.to_string().chars().take(300).collect::<String>());
    // One question's listed pairs, dash and comma forms, and the GIFT block.
    for d in ["Bauxite - Aluminium, Chalk - Soft, Slate - Roof", "=Bauxite -> Aluminium\n=Chalk -> Soft\n=Slate -> Roof"] {
        let r = pp(&bank(vec![m(false), short.clone()], "B", d), HK);
        assert!(reasons(&r).iter().any(|x| x == "pairs_listed" || x == "answer_line" || x == "bank_key_list"), "{d:?}: {}", r.to_string().chars().take(300).collect::<String>());
        assert_eq!(r["ok"], false, "{d:?}");
    }
    // The fixed-order rights list: the served lefts are in pair order, so the list is the pairing.
    let r = pp(&bank(vec![m(true), short.clone()], "B", "Aluminium, Soft, Roof"), HK);
    assert!(has(&r, "word_bank_order"), "rights in pair order: {}", r.to_string().chars().take(300).collect::<String>());
    // (Before and after the order acknowledgement.)
    let acked = json!({"hide_key": true, "order_acks": [{"question": 0}, {"question": 1}]}).to_string();
    assert!(has(&pp(&bank(vec![m(true), short.clone()], "B", "Aluminium, Soft, Roof"), HK), "word_bank_order"), "before ack");
    assert!(has(&pp(&bank(vec![m(true), short.clone()], "B", "Aluminium, Soft, Roof"), &acked), "word_bank_order"), "after ack");
    // A shuffled question's rights list is allowed: without a fixed left column it names nothing.
    let r = pp(&bank(vec![m(false), short.clone()], "B", "Aluminium, Soft, Roof"), HK);
    assert_eq!(r["ok"], true, "shuffled rights list: {}", r.to_string().chars().take(300).collect::<String>());

    // Controls: the same strings in the Matching question's own stem are still refused.
    let in_stem = |stem: &str, keep: bool| {
        let mut q = qv(stem, matching(&pairs.0, &pairs.1, &["Granite"]));
        q["keep_order"] = json!(keep);
        q
    };
    assert!(has(&pp(&one(in_stem("Match.\nBauxite - Aluminium, Chalk - Soft", false)), HK), "pairs_listed"), "same pairs in the stem");
    assert!(has(&pp(&one(in_stem("Match.\nAluminium, Soft, Roof", true)), HK), "word_bank_order"), "rights list in the stem");
}
