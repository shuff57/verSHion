//! Round 15 (critic28a): Anki cloze in every served field, instructions that only hold a
//! verdict word, Kahoot shapes, the bank's author and licence, answer keys of any layout, tags
//! that alone differ, and other variants.
use serde_json::{Value, json};
use share_core::wasm::*;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}
fn pp(p: &str, o: &str) -> Value {
    j(&publish_prepare_json(p.as_bytes(), o))
}
fn hard(r: &Value) -> bool {
    r["ok"] == false && r["errors"].as_array().is_some_and(|e| e.iter().any(|x| x["ack_allowed"] == false))
}
fn qv(stem: &str, kind: Value) -> Value {
    json!({"id":"q10000001","stem": stem,"kind": kind,"points":1.0,"difficulty":null,"topics":[],"created":0,"modified":0})
}
fn one(q: Value) -> String {
    json!({"schema_version": 2, "question": q}).to_string()
}
fn bank(qs: Vec<Value>) -> String {
    let qs: Vec<Value> = qs.into_iter().enumerate().map(|(i, mut q)| { q["id"] = json!(format!("q{:08}", i + 1)); q }).collect();
    json!({"schema_version": 2, "bank": {"id": "b00000001", "name": "Bank", "created": 0, "modified": 0, "details": {"description": ""}, "questions": qs}}).to_string()
}
fn mc(texts: &[&str], correct: usize) -> Value {
    json!({"Mc": {"answers": texts.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": i == correct, "locked": false})).collect::<Vec<_>>()}})
}
const HK: &str = r#"{"hide_key":true}"#;

#[derive(Debug, PartialEq)]
enum Out {
    Served,
    Acked,
    Refused(Vec<String>),
}

/// The critic's honest author through `publish_prepare`: label decisions, order acks, low-risk
/// formatting acks.
fn drive(qv: &Value, i: usize) -> (Out, Value) {
    let mut q = json!({"id": format!("q{i:010}"), "stem": "", "points": 1.0, "difficulty": null, "topics": [], "image": null, "explanation": null, "figure": null, "created": 1760000000000i64, "modified": 1760000100000i64});
    for (k, v) in qv.as_object().unwrap() {
        q[k] = v.clone();
    }
    let p = json!({"schema_version": 2, "question": q}).to_string();
    let mut o = json!({"hide_key": true, "labels": [], "order_acks": [], "ack_formatting": []});
    let mut acked = false;
    for _ in 0..8 {
        let r = j(&publish_prepare_json(p.as_bytes(), &o.to_string()));
        if r["ok"] == true {
            let _ = strip_author_data_json();
            return (if acked { Out::Acked } else { Out::Served }, r);
        }
        let mut changed = false;
        if let Some(f) = r["label_findings"].as_array()
            && o["labels"].as_array().unwrap().is_empty()
        {
            o["labels"] = json!(f.iter().map(|x| { let mut d = json!({"question": x["question"], "choice": if x["strip_allowed"] == true { "strip" } else { "keep" }}); if !x["part"].is_null() { d["part"] = x["part"].clone(); } d }).collect::<Vec<_>>());
            changed = true;
        }
        let e = r["errors"].as_array().unwrap();
        if e.iter().any(|x| x["code"] == "labels_keep_refused") {
            for l in o["labels"].as_array_mut().unwrap() {
                l["choice"] = json!("strip");
            }
            changed = true;
        }
        if let Some(n) = r["needs_order_ack"].as_array()
            && o["order_acks"].as_array().unwrap().is_empty()
        {
            o["order_acks"] = json!(n);
            changed = true;
        }
        // (Any low-risk refusal: key_in_stem has low-risk reasons too.)
        if !acked && e.iter().any(|x| x["ack_allowed"] == true) {
            o["ack_formatting"] = json!([0]);
            acked = true;
            changed = true;
        }
        if !changed {
            return (Out::Refused(e.iter().map(|x| format!("{}/{}", x["code"].as_str().unwrap(), x["reason"].as_str().unwrap_or(""))).collect()), r);
        }
    }
    (Out::Refused(vec!["loop".into()]), Value::Null)
}

fn corpus(name: &str) -> Vec<Value> {
    serde_json::from_str(&std::fs::read_to_string(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap()
}





fn mcl(texts: &[&str], correct: usize) -> Value {
    mc(texts, correct)
}
fn with_details(qs: Vec<Value>, name: &str, details: Value) -> String {
    let mut v: Value = serde_json::from_str(&bank(qs)).unwrap();
    v["bank"]["name"] = json!(name);
    v["bank"]["details"] = details;
    v.to_string()
}

/// B1: Anki cloze is answer syntax in every served field, keyless too.
#[test]
fn b1_anki_cloze_everywhere() {
    let mars = || mcl(&["Venus", "Mars", "Earth"], 1);
    for q in [
        qv("{{c1::Mitochondria}} are the site of aerobic respiration.", json!({"Short": {"sample": "Inner membrane folds into cristae"}})),
        qv("Which planet is the Red Planet? {{c1::Mars}}", mars()), qv("Mars is the Red Planet. {{c1::True}}", json!({"Tf": {"correct": true}})),
        qv("{{c1::Paris::capital}} is in France.", json!({"Short": {"sample": "x"}})), qv("Type: {{type:Back}}", json!({"Short": {"sample": "x"}})),
        qv("Which?", mcl(&["Venus", "{{c1::Mars}}", "Earth"], 1)),
        qv("Match.", json!({"Matching": {"pairs": [{"left": "{{c1::Paris}} is the capital of France", "right": "France"}, {"left": "Rome", "right": "Italy"}], "distractors": []}})),
        qv("Read.", json!({"Multipart": {"parts": [{"stem": "{{c1::Watson}} and {{c2::Crick}} found the helix.", "kind": {"Short": {"sample": "x"}}}]}})),
    ] {
        assert!(hard(&pp(&one(q.clone()), HK)), "{q}");
    }
    let mut k = qv("{{c1::Mitochondria}} are the site of aerobic respiration.", json!({"Short": {"sample": ""}}));
    k["keyless"] = json!(true);
    assert!(hard(&pp(&one(k), HK)), "keyless");
    let mut q = qv("Which planet is red?", mars());
    q["topics"] = json!(["{{c1::Mars}}"]);
    assert!(hard(&pp(&one(q), HK)));
    assert!(hard(&pp(&with_details(vec![qv("Which?", mars()), qv("Which?", mcl(&["Mars", "Venus"], 0))], "Cards", json!({"description": "Cards: {{c1::Mars}} is red"})), HK)));
    // Not cloze: braces in content, a template word.
    for stem in ["The set {{1, 2}} has one element.", "Write {{name}} in the template."] {
        assert_eq!(pp(&one(qv(stem, mars())), HK)["ok"], true, "{stem}");
    }
}

/// Instructions that contain a verdict word are no answer lines; a value after a lead and at the
/// line's end still is.
#[test]
fn instructions_with_verdict_words() {
    let qs = || vec![qv("Water is wet.", json!({"Tf": {"correct": true}})), qv("Fire is cold.", json!({"Tf": {"correct": false}})), qv("Which planet is red?", mcl(&["Venus", "Mars"], 1))];
    for d in ["Ch\u{1ECD}n \u{111}\u{E1}p \u{E1}n \u{111}\u{FA}ng.", "Pick the answer that is true.", "Elige la respuesta verdadera.", "Kreuze die richtige Antwort an."] {
        assert_eq!(pp(&with_details(qs(), "Review", json!({"description": d})), HK)["ok"], true, "{d}");
        assert_eq!(pp(&with_details(qs(), d, json!({})), HK)["ok"], true, "name {d}");
    }
    assert_eq!(pp(&one(qv("Elige la respuesta verdadera.", json!({"Multipart": {"parts": [{"stem": "El agua moja.", "kind": {"Tf": {"correct": true}}}]}}))), HK)["ok"], true);
    assert!(hard(&pp(&one(qv("Water is wet.\nAnswer: True", json!({"Tf": {"correct": true}}))), HK)));
    assert!(hard(&pp(&one(qv("Water is wet. Answer True", json!({"Tf": {"correct": true}}))), HK)));
}

/// Kahoot: the filled quartet is a label scheme; a filled shape among outlined ones is a mark.
#[test]
fn kahoot_shapes() {
    for k in 0..4 {
        let ch = ["\u{25B2} Venus", "\u{25C6} Mars", "\u{25CF} Jupiter", "\u{25A0} Saturn"];
        assert_eq!(pp(&one(qv("Which?", mcl(&ch, k))), HK)["ok"], true, "{k}");
    }
    assert!(hard(&pp(&one(qv("Which?", mcl(&["\u{25B3} Venus", "\u{25B2} Mars", "\u{25C7} Jupiter", "\u{25A1} Saturn"], 1))), HK)));
}

/// The bank's author and licence name are served and read.
#[test]
fn author_and_licence_are_read() {
    let qs = || vec![qv("Which planet is red?", mcl(&["Venus", "Mars"], 1)), qv("Which is hottest?", mcl(&["Venus", "Earth"], 0))];
    assert!(hard(&pp(&with_details(qs(), "B", json!({"author": "Ms Lee (Answer: Mars)"})), HK)));
    assert!(hard(&pp(&with_details(qs(), "B", json!({"license_name": "CC BY 4.0 Answer: Mars"})), HK)));
    assert_eq!(pp(&with_details(qs(), "B", json!({"author": "Ms Lee", "license_name": "CC BY 4.0"})), HK)["ok"], true);
}

/// Answer keys of any layout in the bank's text or a stem: three values in question order within
/// 200 characters, or two next to their own question numbers.
#[test]
fn bank_answer_keys_any_layout() {
    let qs = || vec![
        qv("Where is ATP made?", mcl(&["Ribosome", "Mitochondrion", "Nucleus"], 1)), qv("Cells divide by mitosis.", json!({"Tf": {"correct": true}})),
        qv("The basic unit of life is the ___.", json!({"Blank": {"accept": [["cell"]]}})), qv("Plants take in which gas?", mcl(&["Oxygen", "Carbon dioxide", "Nitrogen"], 1)),
        qv("Fish are mammals.", json!({"Tf": {"correct": false}})),
    ];
    for d in ["1 2 3 4 5\nMitochondrion T cell Carbon dioxide F", "| 1 | Mitochondrion |\n| 2 | T |\n| 3 | cell |", "2 T, 5 F", "Key \u{2014} 1 Mitochondrion; 2 True", "Q1 (Mitochondrion) Q2 (True)", "Mock paper \u{2014} Key: Mitochondrion, True, cell, Carbon dioxide"] {
        assert!(hard(&pp(&with_details(qs(), "Biology", json!({"description": d})), HK)), "{d}");
    }
    for d in ["This mock covers cells, mitochondria and gas exchange.", "Answer true or false where asked.", "Sections: cell biology, respiration."] {
        assert_eq!(pp(&with_details(qs(), "Biology", json!({"description": d})), HK)["ok"], true, "{d}");
    }
}

/// HTML whose tags alone differ: the key's tags unlike every wrong choice's.
#[test]
fn tags_alone_differ() {
    for (k, w) in [("<p><strong>Mars</strong></p>", "<p>{}</p>"), ("<p style=\"font-weight:700\">Mars</p>", "<p style=\"font-weight:400\">{}</p>"), ("<span style=\"color:red\">Mars</span>", "<span style=\"color:black\">{}</span>")] {
        let a = w.replace("{}", "Venus");
        let b = w.replace("{}", "Earth");
        assert!(hard(&pp(&one(qv("Which planet is red?", mcl(&[&a, k, &b], 1))), HK)), "{k}");
    }
    // Notation tags and code are content.
    assert_eq!(pp(&one(qv("Which is water?", mcl(&["H<sub>2</sub>O", "NaCl", "KBr"], 0))), HK)["ok"], true);
    assert_eq!(pp(&one(qv("Which header?", mcl(&["#include <stdio.h>", "import stdio", "using stdio;"], 0))), HK)["ok"], true);
}

/// Variants: hint and definition leads, a decorated key ending the stem, verdicts and praise on the
/// key choice, Anki rows, marked distractors, a True/False symbol ending the statement.
#[test]
fn critic28a_variants() {
    let mars = || mcl(&["Venus", "Mars", "Earth"], 1);
    for stem in ["Which planet is red?\nHint: Mars", "Which planet is red?\nRationale: Mars has iron oxide", "Which planet is red?\nDefinition: Mars", "Which planet is red? **Mars**", "Which planet is red? {Mars}", "Which planet is red? \u{2018}Mars\u{2019}", "Which planet is red? \u{2192} Mars", "Which planet is red? = Mars"] {
        assert!(hard(&pp(&one(qv(stem, mars())), HK)), "{stem}");
    }
    assert_eq!(pp(&one(qv("Compute f(2)", mcl(&["1", "2", "4"], 1))), HK)["ok"], true);
    for k in ["Correct Answer Mars", "Correct! Mars", "Mars OK", "Mars Well done!"] {
        assert!(hard(&pp(&one(qv("Which planet is red?", mcl(&["Venus", k, "Earth"], 1))), HK)), "{k}");
    }
    assert!(hard(&pp(&one(qv("What is the capital of Peru?\tLima", json!({"Short": {"sample": "Lima"}}))), HK)));
    let mp = |d: &[&str]| one(qv("Match each river to its country.", json!({"Matching": {"pairs": [{"left": "Danube", "right": "Hungary"}, {"left": "Vistula", "right": "Poland"}, {"left": "Tagus", "right": "Portugal"}], "distractors": d}})));
    for d in [&["Norway (not used)", "Chile (not used)"][..], &["Norway \u{2013} extra", "Chile \u{2013} extra"], &["Norway (distractor)"]] {
        assert!(hard(&pp(&mp(d), HK)), "{d:?}");
    }
    assert!(hard(&pp(&one(qv("Mars is the Red Planet \u{1F44D}", json!({"Tf": {"correct": true}}))), HK)));
}

/// critic28a's honest corpora: 777 single items and 32 banks, no hard refusal, acknowledgements on
/// at most 2%.
#[test]
fn critic28a_honest_corpora() {
    let items = corpus("critic28a_honest.json");
    let (mut refused, mut acks) = (Vec::new(), 0);
    for (i, it) in items.iter().enumerate() {
        match drive(&it["q"], i).0 {
            Out::Refused(e) => refused.push(format!("{} {e:?}", it["id"])),
            Out::Acked => acks += 1,
            _ => {}
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
    assert!(acks * 50 <= items.len(), "{acks} of {}", items.len());
}

/// The answer-key rule's guards: one word that several questions accept is no list, and a lone
/// "true"/"false" counts only next to its question's number.
#[test]
fn bank_answer_key_guards() {
    let blanks: Vec<Value> = (0..3).map(|i| qv(&format!("Gas {i}: plants release ___."), json!({"Blank": {"accept": [["oxygen"]]}}))).collect();
    assert_eq!(pp(&with_details(blanks, "Gases", json!({"description": "Each answer here is oxygen."})), HK)["ok"], true);
    let tfs = vec![qv("Water is wet.", json!({"Tf": {"correct": true}})), qv("Fire is cold.", json!({"Tf": {"correct": false}})), qv("Ice is cold.", json!({"Tf": {"correct": true}}))];
    assert_eq!(pp(&with_details(tfs, "Review", json!({"description": "Mark each one true or false, then say why it is true."})), HK)["ok"], true);
}
