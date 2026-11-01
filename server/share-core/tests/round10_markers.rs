//! Round 10 (critic21a): annotations on numeric choices, GIFT feedback glued with "#", answer
//! leads opening a choice, words with combining marks, scores after a delimiter, True/False value
//! markers in stems, answer-line gaps checked against this question's real key (instructions
//! publish), locked localized "All of the above" behind labels, unit symbols, word banks, content
//! that starts with "Both", Moodle's long blanks.
use serde_json::{Value, json};
use share_core::wasm::*;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}
fn q(stem: &str, kind: Value) -> Value {
    json!({"id":"q1","stem": stem,"kind": kind,"points":1.0,"difficulty":null,"created":0,"modified":0})
}
fn payload(q: Value) -> String {
    json!({"schema_version": 2, "question": q}).to_string()
}
fn mc(texts: &[&str], correct: usize) -> Value {
    json!({"Mc": {"answers": texts.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": i == correct, "locked": false})).collect::<Vec<_>>()}})
}
fn pp(p: &str, o: &str) -> Value {
    j(&publish_prepare_json(p.as_bytes(), o))
}
/// Refused with an error that cannot be acknowledged.
fn hard(r: &Value) -> bool {
    r["ok"] == false && r["errors"].as_array().is_some_and(|e| e.iter().any(|x| x["ack_allowed"] == false))
}
/// Published, possibly after label decisions, order acks and low-risk acknowledgements.
fn publishes(p: &str) -> (bool, Value) {
    let mut o = json!({"hide_key": true, "labels": [], "order_acks": [], "ack_formatting": []});
    for _ in 0..6 {
        let r = pp(p, &o.to_string());
        if r["ok"] == true {
            let _ = strip_author_data_json();
            return (true, r);
        }
        let mut changed = false;
        if let Some(f) = r["label_findings"].as_array()
            && o["labels"].as_array().unwrap().is_empty()
        {
            o["labels"] = json!(f.iter().map(|x| json!({"question": x["question"], "choice": if x["strip_allowed"] == true { "strip" } else { "keep" }})).collect::<Vec<_>>());
            changed = true;
        }
        if let Some(n) = r["needs_order_ack"].as_array()
            && o["order_acks"].as_array().unwrap().is_empty()
        {
            o["order_acks"] = json!(n);
            changed = true;
        }
        let e = r["errors"].as_array().unwrap();
        if o["ack_formatting"].as_array().unwrap().is_empty() && e.iter().all(|x| x["ack_allowed"] != false) && e.iter().any(|x| x["ack_allowed"] == true) {
            o["ack_formatting"] = json!([0]);
            changed = true;
        }
        if !changed {
            return (false, r);
        }
    }
    (false, Value::Null)
}

#[test]
fn h1_annotations_on_numeric_choices() {
    for t in [
        ["6 (correct)", "5", "7", "8"],
        ["6 - correct", "5", "7", "8"],
        ["6 [KEY]", "5", "7", "8"],
        ["6 (1 pt)", "5", "7", "8"],
        ["6 +1", "5", "7", "8"],
        ["1945 (correct)", "1944", "1946", "1950"],
        ["$40 (correct)", "$30", "$50", "$60"],
        ["1945", "1944 (wrong)", "1946 (wrong)", "1950 (wrong)"],
    ] {
        let r = pp(&payload(q("Which?", mc(&t, 0))), r#"{"hide_key":true,"ack_formatting":[0]}"#);
        assert!(hard(&r), "{t:?}: {r}");
    }
}

#[test]
fn h2_gift_feedback_and_pairs_in_text() {
    for t in [vec!["French#Yes, well done!", "English#Sorry, try again.", "German#No."], vec!["Mars#Good job", "Venus", "Earth"], vec!["%100%blue hair#yes, that is right", "%0%red hair#no", "%0%green hair#no"]] {
        let r = pp(&payload(q("Which?", mc(&t, 0))), r#"{"hide_key":true,"ack_formatting":[0]}"#);
        assert!(hard(&r), "{t:?}: {r}");
    }
    let m = |stem: &str, pairs: &[(&str, &str)]| payload(q(stem, json!({"Matching": {"pairs": pairs.iter().map(|(l, r)| json!({"left": l, "right": r})).collect::<Vec<_>>(), "distractors": ["Oslo"]}})));
    assert!(hard(&pp(&m("Match.", &[("Peru -> Lima", "Lima"), ("Japan", "Tokyo")]), r#"{"hide_key":true}"#)));
    assert!(hard(&pp(&m("Match the capitals. Key: France\u{2013}Paris, Japan\u{2013}Tokyo", &[("France", "Paris"), ("Japan", "Tokyo")]), r#"{"hide_key":true}"#)));
}

#[test]
fn h3_h4_answer_leads_and_words_with_marks_in_choices() {
    for lead in [
        "Correct answer: Mars", "Right answer - Mars", "resp. correcta: Mars", "bonne r\u{E9}ponse: Mars", "\u{43F}\u{440}\u{430}\u{432}\u{438}\u{43B}\u{44C}\u{43D}\u{44B}\u{439} \u{43E}\u{442}\u{432}\u{435}\u{442}: Mars",
        "do\u{11F}ru cevap: Mars", "jawaban benar: Mars", "risposta esatta: Mars", "\u{6B63}\u{786E}\u{7B54}\u{6848}: Mars", "\u{7B54}\u{6848}: Mars", "\u{6B63}\u{89E3}: Mars", "\u{7B54}\u{3048}: Mars",
        "\u{C815}\u{B2F5}: Mars", "\u{627}\u{644}\u{625}\u{62C}\u{627}\u{628}\u{629}: Mars", "\u{909}\u{924}\u{94D}\u{924}\u{930}: Mars",
        "\u{92E}\u{902}\u{917}\u{932} (\u{909}\u{924}\u{94D}\u{924}\u{930})", "\u{9AE}\u{999}\u{9CD}\u{997}\u{9B2} (\u{989}\u{9A4}\u{9CD}\u{9A4}\u{9B0})", "Mars (\u{111}\u{E1}p \u{E1}n)", "Marte - esatta", "Marte - gabarito", "\u{D654}\u{C131} - \u{C633}\u{C74C}", "Mars - kunci",
    ] {
        let r = pp(&payload(q("Which planet is red?", mc(&[lead, "Venus", "Earth", "Jupiter"], 0))), r#"{"hide_key":true,"ack_formatting":[0]}"#);
        assert!(hard(&r), "{lead}: {r}");
    }
    let r = pp(&payload(q("Which planet is red?", mc(&["Mars", "V\u{EA}nus - incorreta", "Terra - incorreta", "J\u{FA}piter - incorreta"], 0))), r#"{"hide_key":true,"ack_formatting":[0]}"#);
    assert!(hard(&r), "{r}");
    // Honest choices that open with a lead-like word.
    for t in [["Solution: a homogeneous mixture", "Suspension", "Colloid", "Emulsion"], ["key: value", "key = value", "key -> value", "key, value"]] {
        assert!(publishes(&payload(q("Which?", mc(&t, 0)))).0, "{t:?}");
    }
}

#[test]
fn h5_scores_after_a_delimiter() {
    for k in ["Mars = 1 point", "Mars \u{2014} 5 pts", "Mars: 1 point", "Mars (worth 1 point)"] {
        let r = pp(&payload(q("Which planet is red?", mc(&[k, "Venus", "Earth", "Jupiter"], 0))), r#"{"hide_key":true,"ack_formatting":[0]}"#);
        assert!(hard(&r), "{k}: {r}");
    }
    // An index is no score.
    assert!(publishes(&payload(q("Last element?", mc(&["xs[-1]", "xs[0]", "xs.last()", "xs[len(xs)]"], 0)))).0);
}

#[test]
fn h6_true_false_value_markers() {
    let tf = |stem: &str, v: bool| payload(q(stem, json!({"Tf": {"correct": v}})));
    for (stem, v) in [
        ("The Sun is a star. TRUE", true), ("The Sun is a star. [T]", true), ("The Sun is a star. *T", true), ("T The Sun is a star.", true),
        ("The Moon makes its own light \u{2192} F", false), ("The Moon makes its own light = F", false), ("The Moon makes its own light. (F)", false),
        ("The Moon makes its own light (FALSE \u{2013} it reflects light)", false), ("The Moon makes its own light. Feedback: False, it reflects sunlight.", false),
    ] {
        let r = pp(&tf(stem, v), r#"{"hide_key":true}"#);
        assert!(hard(&r), "{stem}: {r}");
    }
    let part = payload(q("Astronomy.", json!({"Multipart": {"parts": [{"stem": "The Sun is a star. TRUE", "kind": {"Tf": {"correct": true}}}]}})));
    assert!(hard(&pp(&part, r#"{"hide_key":true}"#)));
    // Instructions (H9): publish.
    for stem in ["Answer true or false. The Sun is a star.", "Directions: Answer true or false.\nThe Sun is a star."] {
        assert_eq!(pp(&tf(stem, true), r#"{"hide_key":true}"#)["ok"], true, "{stem}");
    }
    // "O Sol é uma estrela": an article, not a value.
    assert_eq!(pp(&tf("O Sol \u{E9} uma estrela.", true), r#"{"hide_key":true}"#)["ok"], true);
}

#[test]
fn h7_h9_answer_lines_against_the_real_key() {
    let planets = ["Venus", "Mars", "Earth", "Jupiter"];
    for stem in ["Which planet is red?\nClave: B", "Which planet is red?\nRpta: B", "Which planet is red?\nResp.: B", "Which planet is red?\nGabarito: B", "Which planet is red?\nCorrig\u{E9} : B", "Which planet is red?\nTr\u{1EA3} l\u{1EDD}i: B", "Which planet is red? Answer B", "Which planet is red? (B)"] {
        let r = pp(&payload(q(stem, mc(&planets, 1))), r#"{"hide_key":true}"#);
        assert!(hard(&r), "{stem}: {r}");
    }
    // The same lines naming a wrong letter, and instructions, publish (H9).
    for stem in ["Answer A, B, C or D. Which planet is red?", "Which planet is red? (C)", "Choose one: A, B, C or D.", "Which planet is red?\nAnswer: C", "Name the red planet. Answer A, C, D or B."] {
        assert_eq!(pp(&payload(q(stem, mc(&planets, 1))), r#"{"hide_key":true}"#)["ok"], true, "{stem}");
    }
    // Topics.
    for topic in ["Key B", "answer B", "Mars (correct)", "ans Mars"] {
        let mut v = q("Which planet is red?", mc(&planets, 1));
        v["topics"] = json!([topic]);
        assert!(hard(&pp(&payload(v), r#"{"hide_key":true}"#)), "{topic}");
    }
    let mut v = q("The Sun is a star.", json!({"Tf": {"correct": true}}));
    v["topics"] = json!(["True"]);
    assert!(hard(&pp(&payload(v), r#"{"hide_key":true}"#)));
    assert!(hard(&pp(&payload(q("What forms when iron rusts? Answer: iron oxide", json!({"Short": {"sample": "iron oxide"}}))), r#"{"hide_key":true}"#)));
    // A page reference is no answer line.
    assert_eq!(pp(&payload(q("According to the textbook (p. 214), which planet is red?", mc(&planets, 1))), r#"{"hide_key":true}"#)["ok"], true);
}

#[test]
fn h10_locked_localized_all_of_the_above_behind_labels() {
    for (all, labels) in [("d) All of the above", true), ("\u{2022} All of the above", false), ("d) Toate cele de mai sus", true)] {
        let texts: Vec<String> = if labels { vec!["a) Dog".into(), "b) Cat".into(), "c) Whale".into(), all.into()] } else { vec!["\u{2022} Dog".into(), "\u{2022} Cat".into(), "\u{2022} Whale".into(), all.into()] };
        let answers: Vec<Value> = texts.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": i == 3, "locked": i == 3})).collect();
        let (ok, r) = publishes(&payload(q("Which are mammals?", json!({"Mc": {"answers": answers}}))));
        assert!(ok, "{all}: {r}");
    }
}

#[test]
fn h11_h12_units_and_blood_groups_are_not_hard() {
    for (t, k) in [(vec!["ohm (\u{3A9})", "volt (V)", "watt (W)", "ampere (A)"], 0), (vec!["degree (\u{B0})", "radian", "gradian", "turn"], 0), (vec!["pi (\u{3C0})", "e", "phi", "tau"], 0), (vec!["O negative", "A positive", "B negative", "AB positive"], 0), (vec!["neutron (no charge)", "proton", "electron", "positron"], 0), (vec!["hydrogen - no neutrons", "helium", "lithium", "beryllium"], 0)] {
        let r = pp(&payload(q("Which?", mc(&t, k))), r#"{"hide_key":true,"ack_formatting":[0],"labels":[{"question":0,"choice":"keep"}],"order_acks":[{"question":0}]}"#);
        assert!(!hard(&r), "{t:?}: {r}");
    }
}

#[test]
fn h13_word_banks_and_blanks() {
    let blank = |stem: &str, a: &[&str]| payload(q(stem, json!({"Blank": {"accept": a.iter().map(|x| vec![x]).collect::<Vec<_>>()}})));
    // (Round 12: the one answer first in the bank is low risk; elsewhere it publishes.)
    for stem in ["Word bank: Venus, Mars, Jupiter.\nThe red planet is ____.", "Use these words: Venus, Jupiter, Mars. The red planet is ____.", "Banco de palabras: Venus, Marte, J\u{FA}piter. El planeta rojo es ____."] {
        let key = if stem.starts_with("Banco") { "Marte" } else { "Mars" };
        assert_eq!(pp(&blank(stem, &[key]), r#"{"hide_key":true}"#)["ok"], true, "{stem}");
    }
    for stem in ["Word bank: Mars, Venus, Jupiter.\nThe red planet is ____.", "Use these words: Mars, Venus, Jupiter. The red planet is ____."] {
        let r = pp(&blank(stem, &["Mars"]), r#"{"hide_key":true}"#);
        assert!(r["ok"] == false && !hard(&r), "{stem}: {r}");
        assert_eq!(pp(&blank(stem, &["Mars"]), r#"{"hide_key":true,"ack_formatting":[0]}"#)["ok"], true, "{stem}");
    }
    // Moodle's long blank is one blank.
    assert_eq!(pp(&blank("The red planet is __________.", &["Mars"]), r#"{"hide_key":true}"#)["ok"], true);
    // The accepted word in brackets after its blank: acknowledgeable (an ESL form hint).
    let r = pp(&blank("I ___ (eat) breakfast every day.", &["eat"]), r#"{"hide_key":true}"#);
    assert!(r["ok"] == false && !hard(&r), "{r}");
    assert_eq!(pp(&blank("I ___ (eat) breakfast every day.", &["eat"]), r#"{"hide_key":true,"ack_formatting":[0]}"#)["ok"], true);
}

#[test]
fn h14_content_that_starts_with_both_is_no_reference() {
    use share_core::labels::refers_to_siblings;
    let r = |t: &[&str]| refers_to_siblings(&t.iter().map(|x| (*x, false)).collect::<Vec<_>>());
    assert!(!r(&["Both involve DNA replication", "Both make gametes", "Neither involves crossing over", "Only mitosis does"]));
    assert!(!r(&["Both are amphibians", "Both are reptiles", "One is a mammal", "Neither is a vertebrate"]));
    assert!(!r(&["Shade 3", "Shade 5", "Shade 10", "Shade 14 only for all currents"]));
    assert!(!r(&["o I only", "o I and II only", "o II only", "o I, II and III"]));
    assert!(r(&["Mitochondria", "Ribosome", "Both", "Neither"]));
    assert!(r(&["Mitochondria", "Ribosome", "Both of them", "Nucleus"]));
}
