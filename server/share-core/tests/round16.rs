//! Round 16 (critic28b): the lead scan's work budget, lists in order with shared keys and
//! repeated values, word banks without caps, topics past the key cap, a served form that
//! validates to itself, and the scanners' remaining limits.
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

fn mcl(texts: &[&str], correct: usize) -> Value {
    mc(texts, correct)
}
fn with_desc(qs: Vec<Value>, desc: &str) -> String {
    let mut v: Value = serde_json::from_str(&bank(qs)).unwrap();
    v["bank"]["details"]["description"] = json!(desc);
    v.to_string()
}
fn has(r: &Value, reason: &str) -> bool {
    r["errors"].as_array().is_some_and(|e| e.iter().any(|x| x["reason"] == reason))
}
fn tf(v: bool) -> Value {
    json!({"Tf": {"correct": v}})
}
const KEYS: [&str; 5] = ["mitosis", "osmosis", "diffusion", "meiosis", "glycolysis"];
fn five() -> Vec<Value> {
    KEYS.iter().enumerate().map(|(i, k)| qv(&format!("Question {i}?"), mcl(&[k, "anaphase", "prophase"], 0))).collect()
}

/// B2: lists in order where questions or parts share a key, or True/False values repeat.
#[test]
fn b2_shared_keys_and_repeated_values() {
    let parts = |kinds: Vec<Value>| kinds.into_iter().enumerate().map(|(i, k)| json!({"stem": format!("Part {i}."), "kind": k})).collect::<Vec<_>>();
    let mp = |main: &str, kinds: Vec<Value>| one(qv(main, json!({"Multipart": {"parts": parts(kinds)}})));
    assert!(hard(&pp(&mp("Results: True, False, False, True", vec![tf(true), tf(false), tf(false), tf(true)]), HK)));
    assert!(hard(&pp(&mp("Results: mitosis, True, osmosis, True", vec![mcl(&["mitosis", "x"], 0), tf(true), mcl(&["osmosis", "y"], 0), tf(true)]), HK)));
    assert_eq!(pp(&mp("Results: True, False", vec![tf(true), tf(true), tf(false)]), HK)["ok"], true);
    let mut qs = five();
    qs.insert(1, qv("Again?", mcl(&["mitosis", "z1", "z2"], 0)));
    assert!(has(&pp(&with_desc(qs, "Word bank: mitosis, osmosis, diffusion, meiosis, glycolysis"), HK), "bank_word_bank_order"));
    assert!(has(&pp(&with_desc(five(), "Word bank: glycolysis, mitosis, osmosis, diffusion, meiosis"), HK), "bank_word_bank_order"));
    // Two questions sharing a key are one list item for both: four in order over three items.
    let shared: Vec<Value> = ["mitosis", "mitosis", "osmosis", "diffusion"].iter().enumerate().map(|(i, k)| qv(&format!("Question {i}?"), mcl(&[k, "anaphase", "prophase"], 0))).collect();
    assert!(has(&pp(&with_desc(shared, "Word bank: mitosis, osmosis, diffusion"), HK), "bank_word_bank_order"));
    assert_eq!(pp(&with_desc(five(), "Word bank: meiosis, glycolysis, osmosis, mitosis, diffusion"), HK)["ok"], true);
    let blank = one(qv("Word bank: mitosis, osmosis, diffusion.\nCells copy by ___. Later ___ again. Water by ___. Gas by ___.", json!({"Blank": {"accept": [["mitosis"], ["mitosis"], ["osmosis"], ["diffusion"]]}})));
    assert!(hard(&pp(&blank, HK)));
}

/// B3-B6: no count of banks, sentences or line length hides a list, and one odd item does not
/// drop it.
#[test]
fn b3_b6_word_banks_without_caps() {
    let list = "Word bank: mitosis, osmosis, diffusion, meiosis, glycolysis";
    for d in [format!("{}\n{list}", "(a,b,c)".repeat(260)), format!("{}{list}", "Hi. ".repeat(100)), format!("{list},"), format!("{list}, ..."), format!("{list}, and one more that is not used")] {
        assert!(has(&pp(&with_desc(five(), &d), HK), "bank_word_bank_order"), "{}", &d[..d.len().min(60)]);
    }
    // A line over 4 KB in a stem.
    let mut qs = five();
    qs.push(qv(&format!("{list}. {}", "lorem ipsum dolor sit amet ".repeat(160)), json!({"Short": {"sample": "x"}})));
    assert!(has(&pp(&bank(qs), HK), "bank_word_bank_order"));
}

/// B7: a topic that is a key past 65,536 bank keys; a description answer line then fails closed.
#[test]
fn b7_topics_past_the_key_cap() {
    let mut qs: Vec<Value> = (0..66).map(|b| qv(&"___ ".repeat(50), json!({"Blank": {"accept": (0..50).map(|g| (0..20).map(|k| format!("k{b}y{g}z{k}")).collect::<Vec<_>>()).collect::<Vec<_>>()}}))).collect();
    qs.push(qv("Which process makes glucose?", mcl(&["Photosynthesis", "Respiration"], 0)));
    let mut q = qv("Which gas do plants release?", mcl(&["Oxygen", "Nitrogen"], 0));
    q["topics"] = json!(["Photosynthesis"]);
    qs.push(q);
    let v = pp(&bank(qs.clone()), HK);
    assert!(v["errors"].as_array().unwrap().iter().any(|e| e["reason"] == "topic_is_key"), "{}", v.to_string().chars().take(300).collect::<String>());
    qs.pop();
    // (Round 17: no key cap; the description is read against every key.)
    assert_eq!(pp(&with_desc(qs.clone(), "Answer: something"), HK)["ok"], true);
    let v = pp(&with_desc(qs, "Answer: k65y49z19"), HK);
    assert!(v["errors"].as_array().unwrap().iter().any(|e| e["reason"] == "answer_line"));
}

/// B1: 99 key lengths and stems of answer leads: the scan is budgeted and fails closed, in time
/// linear in the payload (two sizes) and under a bound.
#[test]
fn b1_lead_scan_budget() {
    let shape = |stems: usize| {
        let mut qs: Vec<Value> = (3..200).step_by(2).map(|l| qv("Explain.", json!({"Short": {"sample": format!("{}b", "a.".repeat((l - 1) / 2))}}))).collect();
        let stem = format!("Statement. {}{}", format!("answer:{}", "a.".repeat(27)).repeat(64), "a.".repeat(100));
        qs.extend((0..stems).map(|_| qv(&stem, tf(true))));
        bank(qs)
    };
    let time = |p: &str| (0..3).map(|_| { let t = std::time::Instant::now(); std::hint::black_box(publish_prepare_json(p.as_bytes(), HK)); t.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min);
    let (a, b) = (time(&shape(220)), time(&shape(440)));
    assert!(b / a < 3.0 || b < 0.05, "{a:.3} -> {b:.3}");
    assert!(b < 1.5, "{b:.3} s");
    let v = pp(&shape(440), HK);
    assert!(v["errors"].as_array().unwrap().iter().any(|e| e["reason"] == "scan_budget"), "{}", v.to_string().chars().take(300).collect::<String>());
    // An honest bank is far below the budget.
    assert_eq!(pp(&bank((0..200).map(|i| qv(&format!("Question {i}: which is right? Answer the question with care."), mcl(&["alpha", "beta", "gamma"], i % 3))).collect()), HK)["ok"], true);
}

/// The scanners' remaining limits: 30 pasted lines, 128 stem-list items, twelve words after a
/// lead, parts past "z".
#[test]
fn remaining_limits_are_gone() {
    let mut stem = String::from("Which gas?\n");
    for k in 0..40 {
        let _ = k;
        stem.push_str("a. decoy word\n");
    }
    stem.push_str("b. Nitrogen \u{2713}\nc. Argon");
    assert!(hard(&pp(&one(qv(&stem, mcl(&["Oxygen", "Nitrogen", "Argon"], 1))), HK)));
    let list: String = (0..200).map(|k| format!("{}. item{k}\n", k + 1)).collect();
    let pairs: Vec<Value> = (0..3).map(|k| json!({"left": format!("Left {k} ({})", 151 + k), "right": format!("item{}", 150 + k)})).collect();
    assert!(hard(&pp(&one(qv(&format!("Match.\n{list}"), json!({"Matching": {"pairs": pairs, "distractors": []}}))), HK)));
    assert!(hard(&pp(&one(qv("Which planet is red?\nAnswer: as we said in class many many times this term it is of course Mars", mcl(&["Venus", "Mars"], 1))), HK)));
    let parts: Vec<Value> = (0..30).map(|i| json!({"stem": if i == 29 { "Note: (27) is True.".to_string() } else { format!("Statement {i}.") }, "kind": tf(i == 26)})).collect();
    assert!(hard(&pp(&one(qv("Read.", json!({"Multipart": {"parts": parts}}))), HK)));
}

/// Words every question accepts say nothing of the order (critic24a's wordbank_lines_ok).
#[test]
fn shared_words_are_no_key_list() {
    let names = ["alpha", "beta", "gamma", "delta"];
    let unit = "Use: delta, gamma, beta, alpha\nChoose from: gamma; alpha; beta\n";
    let stem = format!("{}\n{}", unit.repeat(10), "___ ".repeat(50));
    let qs: Vec<Value> = (0..12).map(|_| qv(&stem, json!({"Blank": {"accept": (0..50).map(|b| vec![names[b % 4]]).collect::<Vec<_>>()}}))).collect();
    let r = pp(&bank(qs), HK);
    assert!(!has(&r, "bank_key_list") && !has(&r, "bank_word_bank_order") && !has(&r, "bank_word_bank_three"), "{}", r.to_string().chars().take(300).collect::<String>());
}
