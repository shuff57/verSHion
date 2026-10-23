//! Round 14 (critic26a, critic26b): every Multipart part read, scans that fail closed, a bank's
//! lists in question order, topics against every key, worksheet answer blanks and other honest
//! forms.
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

/// The honest author through a whole bank: label decisions, order acks, low-risk acks by the
/// questions the refusals name.
fn drive_bank(p: &str) -> (Out, Value) {
    let mut o = json!({"hide_key": true, "labels": [], "order_acks": [], "ack_formatting": []});
    let mut acked = false;
    for _ in 0..8 {
        let r = j(&publish_prepare_json(p.as_bytes(), &o.to_string()));
        if r["ok"] == true {
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
        let low: Vec<u64> = e.iter().filter(|x| x["ack_allowed"] == true).filter_map(|x| x["path"].as_str().and_then(|p| p.strip_prefix("bank.questions[")).and_then(|p| p.split(']').next()).and_then(|n| n.parse().ok())).collect();
        if !low.is_empty() && !acked {
            o["ack_formatting"] = json!(low);
            acked = true;
            changed = true;
        }
        if !changed {
            return (Out::Refused(e.iter().map(|x| format!("{}/{}@{}", x["code"].as_str().unwrap(), x["reason"].as_str().unwrap_or(""), x["path"].as_str().unwrap_or(""))).collect()), r);
        }
    }
    (Out::Refused(vec!["loop".into()]), Value::Null)
}

const WORDS: [&str; 22] = ["Photosynthesis", "Mitochondria", "Chlorophyll", "Glucose", "Stomata", "Xylem", "Phloem", "Ribosome", "Nucleus", "Osmosis", "Diffusion", "Enzyme", "Protein", "Lipid", "Cellulose", "Vacuole", "Cytoplasm", "Membrane", "Chloroplast", "Respiration", "Transpiration", "Pollination"];

fn parts20(edits: &[(usize, &str)], main: &str) -> String {
    let mut parts: Vec<Value> = (0..20).map(|i| json!({"stem": format!("Question {}: which term fits description number {}?", i + 1, i + 1), "kind": {"Mc": {"answers": [{"text": WORDS[i], "correct": true, "locked": false}, {"text": format!("Distractor {i}a"), "correct": false, "locked": false}, {"text": format!("Distractor {i}b"), "correct": false, "locked": false}]}}})).collect();
    for (i, s) in edits {
        parts[*i]["stem"] = json!(s);
    }
    one(qv(main, json!({"Multipart": {"parts": parts}})))
}

/// critic26b B2: every part of a Multipart is read against every other part's key (parts 16 to
/// 49 were not).
#[test]
fn b2_every_part_is_read() {
    for (edits, main) in [
        (vec![(17, "ANSWER: Photosynthesis")], "Read the passage."), (vec![(0, "ANSWER: Respiration")], "Read the passage."), (vec![], "Read the passage. ANSWER: Respiration"),
        (vec![(18, "Answer to (a): Photosynthesis")], "Read the passage."),
    ] {
        assert!(hard(&pp(&parts20(&edits, main), HK)), "{edits:?} {main}");
    }
    assert_eq!(pp(&parts20(&[], "Read the passage about plant cells."), HK)["ok"], true);
    let tf: Vec<Value> = (0..20).map(|i| json!({"stem": if i == 18 { "Note: (a) is True.".to_string() } else { format!("Statement {i}.") }, "kind": {"Tf": {"correct": i % 2 == 0}}})).collect();
    assert!(hard(&pp(&one(qv("Read each statement.", json!({"Multipart": {"parts": tf}}))), HK)));
}

/// critic26b: what was capped now fails closed, or is read whole.
#[test]
fn caps_fail_closed() {
    let r = |p: &str| pp(p, HK);
    let reason = |v: &Value| v["errors"].as_array().and_then(|e| e.first()).and_then(|e| e["reason"].as_str()).unwrap_or("").to_string();
    // Many answer lines naming no key publish (round 17: the count of leads is no budget);
    // one more naming the key is still read.
    let stem = format!("Which planet is red?\n{}", "Answer: maybe\n".repeat(70));
    assert_eq!(r(&one(qv(&stem, mcl(&["Venus", "Mars"], 1))))["ok"], true);
    let v = r(&one(qv(&format!("{stem}Answer: Mars"), mcl(&["Venus", "Mars"], 1))));
    assert!(hard(&v) && reason(&v) == "answer_line", "{v}");
    // A Blank answer among 1,000 accepted ones, named by an answer line.
    let accept: Vec<Vec<String>> = (0..50).map(|b| (0..20).map(|k| format!("w{b}x{k}")).collect()).collect();
    let v = r(&one(qv(&format!("{}\nAnswer: w49x19", "___ ".repeat(50)), json!({"Blank": {"accept": accept}}))));
    assert!(hard(&v), "{v}");
    // A key longer than the window, named by its start.
    let long = format!("The model answer explains {}", "the water cycle in many careful words, ".repeat(8));
    let q1 = qv("Explain the water cycle.", json!({"Short": {"sample": long}}));
    let q2 = qv(&format!("Describe evaporation. Answer: {}", &long[..long.len() - 20]), json!({"Short": {"sample": "heat"}}));
    assert!(hard(&r(&bank(vec![q1, q2]))));
    // An answer line at the end of a long Matching left item.
    let left = format!("{} Answer: Paris. It borders Spain and Italy.", "France is a country in western Europe. ".repeat(20));
    let v = r(&one(qv("Match.", json!({"Matching": {"pairs": [{"left": left, "right": "Paris"}, {"left": "Japan", "right": "Tokyo"}], "distractors": ["Rome"]}}))));
    assert!(hard(&v), "{v}");
    // A pasted choice list line longer than a choice, marked.
    let stem = format!("Which gas?\nA. Oxygen {}\nB. Nitrogen \u{2713} {}\nC. Argon", "x".repeat(2100), "y".repeat(2100));
    assert!(hard(&r(&one(qv(&stem, mcl(&["Oxygen", "Nitrogen", "Argon"], 1))))));
    // A word bank of 70 items with the answers past the 64th, in blank order.
    let mut items: Vec<String> = (0..66).map(|k| format!("filler{k}")).collect();
    items.extend(["alpha", "beta", "gamma", "delta"].iter().map(|s| s.to_string()));
    let v = r(&one(qv(&format!("Word bank: {}\nThe ___, the ___, the ___ and the ___.", items.join(", ")), json!({"Blank": {"accept": [["alpha"], ["beta"], ["gamma"], ["delta"]]}}))));
    assert!(hard(&v), "{v}");
}

/// critic26b: more keys than are looked up across a bank: a stem with an answer line fails closed.
#[test]
fn bank_keys_over_the_budget_fail_closed() {
    let mut qs: Vec<Value> = (0..66).map(|b| qv("___ x", json!({"Blank": {"accept": [(0..1000).map(|k| format!("k{b}y{k}")).collect::<Vec<_>>().chunks(20).next().unwrap().to_vec()]}}))).collect();
    // (Each question: 50 blanks x 20 answers.)
    for (b, q) in qs.iter_mut().enumerate() {
        q["stem"] = json!("___ ".repeat(50));
        q["kind"] = json!({"Blank": {"accept": (0..50).map(|g| (0..20).map(|k| format!("k{b}y{g}z{k}")).collect::<Vec<_>>()).collect::<Vec<_>>()}});
    }
    // (Round 17: no key cap any more: every key is read, so a line naming none publishes and one
    // naming the 66,000th key is refused.)
    let mut honest = qs.clone();
    honest.push(qv("Which planet is red? Answer: something else", mcl(&["Venus", "Mars"], 1)));
    assert_eq!(pp(&bank(honest), HK)["ok"], true);
    qs.push(qv("Which planet is red? Answer: k65y49z19", mcl(&["Venus", "Mars"], 1)));
    let v = pp(&bank(qs), HK);
    let reasons: Vec<&str> = v["errors"].as_array().unwrap().iter().filter_map(|e| e["reason"].as_str()).collect();
    assert!(reasons.contains(&"answer_line"), "{reasons:?}");
}

/// critic26a B1: a list in the bank's name, description or a stem holding the questions' keys in
/// question order.
#[test]
fn b1_bank_lists_in_question_order() {
    let keys = ["osmosis", "diffusion", "phagocytosis", "pinocytosis"];
    let stems = ["Water crossing a membrane toward more solute is ___.", "Particles spreading from high to low concentration is ___.", "A cell engulfing a solid particle is ___.", "A cell taking in droplets of fluid is ___."];
    let qs = |first: &str| -> Vec<Value> { (0..4).map(|i| qv(&if i == 0 { format!("{first}{}", stems[i]) } else { stems[i].to_string() }, json!({"Blank": {"accept": [[keys[i]]]}}))).collect() };
    let with = |name: &str, desc: &str, first: &str| {
        let mut v: Value = serde_json::from_str(&bank(qs(first))).unwrap();
        v["bank"]["name"] = json!(name);
        v["bank"]["details"]["description"] = json!(desc);
        v.to_string()
    };
    for (name, desc, first) in [
        ("Transport", "Word bank: osmosis, diffusion, phagocytosis, pinocytosis", ""), ("Transport", "Words to use:\n\u{2022} osmosis\n\u{2022} diffusion\n\u{2022} phagocytosis\n\u{2022} pinocytosis", ""),
        ("osmosis, diffusion, phagocytosis, pinocytosis", "", ""), ("Transport", "", "Word bank for questions 1\u{2013}4: osmosis, diffusion, phagocytosis, pinocytosis\n\n"),
    ] {
        let (out, r) = drive_bank(&with(name, desc, first));
        assert!(matches!(out, Out::Refused(_)), "{name} / {desc} / {first}: {r}");
    }
    for (name, desc) in [("Transport", "Word bank: pinocytosis, osmosis, phagocytosis, diffusion"), ("Transport", "Key vocabulary: diffusion, osmosis, pinocytosis, phagocytosis.")] {
        assert_eq!(drive_bank(&with(name, desc, "")).0, Out::Served, "{desc}");
    }
    // Mc keys too.
    let planets = [("Which planet is largest?", ["Jupiter", "Mars", "Venus"], 0), ("Which is hottest?", ["Mars", "Venus", "Earth"], 1), ("Which is red?", ["Venus", "Mars", "Earth"], 1), ("Which has rings?", ["Saturn", "Mars", "Venus"], 0)];
    let mut v: Value = serde_json::from_str(&bank(planets.iter().map(|(s, c, k)| qv(s, mcl(c, *k))).collect())).unwrap();
    v["bank"]["details"]["description"] = json!("Word bank: Jupiter, Venus, Mars, Saturn");
    assert!(matches!(drive_bank(&v.to_string()).0, Out::Refused(_)));
    // A Multipart main stem listing its parts' keys in part order.
    let mp = qv("Results: 3 m/s\u{B2}, True, Newton's second law, 12 N.", json!({"Multipart": {"parts": [
        {"stem": "(a) The acceleration?", "kind": {"Mc": {"answers": [{"text": "3 m/s\u{B2}", "correct": true, "locked": false}, {"text": "2 m/s\u{B2}", "correct": false, "locked": false}]}}},
        {"stem": "(b) Is the net force 6 N?", "kind": {"Tf": {"correct": true}}},
        {"stem": "(c) Which law?", "kind": {"Short": {"sample": "Newton's second law"}}},
        {"stem": "(d) The force on 4 kg?", "kind": {"Mc": {"answers": [{"text": "12 N", "correct": true, "locked": false}, {"text": "8 N", "correct": false, "locked": false}]}}}]}}));
    assert!(hard(&pp(&one(mp), HK)));
}

/// critic26a: topics against every key: Multipart parts, a key standing in the topic, another
/// question's key.
#[test]
fn topics_against_every_key() {
    let mp = |topic: &str| {
        let mut q = qv("A 2 kg cart is pushed with 6 N.", json!({"Multipart": {"parts": [{"stem": "Its acceleration?", "kind": {"Mc": {"answers": [{"text": "3 m/s\u{B2}", "correct": true, "locked": false}, {"text": "12 m/s\u{B2}", "correct": false, "locked": false}]}}}, {"stem": "Is it accelerating?", "kind": {"Tf": {"correct": true}}}, {"stem": "Which law?", "kind": {"Short": {"sample": "Newton's second law"}}}]}}));
        q["topics"] = json!([topic]);
        one(q)
    };
    for t in ["3 m/s\u{B2}", "True", "Newton's second law", "Acceleration = 3 m/s\u{B2}"] {
        assert!(hard(&pp(&mp(t), HK)), "{t}");
    }
    assert_eq!(pp(&mp("Dynamics"), HK)["ok"], true);
    let mut q = qv("Which planet is red?", mcl(&["Venus", "Mars"], 1));
    q["topics"] = json!(["Unit 4: Mars"]);
    assert!(hard(&pp(&one(q), HK)));
    let q1 = qv("Which planet is red?", mcl(&["Venus", "Mars"], 1));
    let mut q2 = qv("Which planet is hottest?", mcl(&["Venus", "Earth"], 0));
    q2["topics"] = json!(["Mars"]);
    assert!(hard(&pp(&bank(vec![q1, q2]), HK)));
}

/// critic26a: honest forms that were refused hard with a misleading hint.
#[test]
fn honest_critic26a_forms() {
    for stem in ["T   F   Larches are conifers that drop their needles.", "T F Larches are conifers.", "V  F  Los alerces son con\u{ED}feras.", "True / False Larches are conifers."] {
        for v in [true, false] {
            assert_eq!(pp(&one(qv(stem, json!({"Tf": {"correct": v}}))), HK)["ok"], true, "{stem} {v}");
        }
    }
    assert!(hard(&pp(&one(qv("Larches are conifers. True", json!({"Tf": {"correct": true}}))), HK)));
    let pairs = [("\u{BF}Qu\u{E9} hora es?", "What time is it?"), ("Comment \u{E7}a va ?", "How are you?"), ("Buenos d\u{ED}as", "Good morning")];
    let m = one(qv("Match each phrase to its translation.", json!({"Matching": {"pairs": pairs.iter().map(|(l, r)| json!({"left": l, "right": r})).collect::<Vec<_>>(), "distractors": ["Good night"]}})));
    assert_eq!(pp(&m, HK)["ok"], true);
    assert_eq!(pp(&one(qv("What is one stroke under par called?", mcl(&["Bogey (+1)", "Birdie (\u{2212}1)", "Eagle (\u{2212}2)", "Par (0)"], 1))), HK)["ok"], true);
    assert!(hard(&pp(&one(qv("Which is right?", mcl(&["Mars (100%)", "Venus (0%)", "Earth (0%)"], 0))), HK)));
    let (_, r) = drive(&json!({"stem": "Which tree?", "kind": mcl(&["- [ ] Oak (Q)", "- [ ] Elm (U)", "- [ ] Ash (F)", "- [ ] Yew (T)"], 0)}), 0);
    assert_eq!(r["ok"], true, "{r}");
    assert!(hard(&pp(&one(qv("Which tree?", mcl(&["- [x] Oak", "- [ ] Elm", "- [ ] Ash"], 0))), HK)));
    let p = |t: &str| format!("<p dir=\"ltr\" style=\"text-align: left;\">{t}</p>");
    assert_eq!(pp(&one(qv("Which word means 'correct' in German?", mcl(&["<p dir=\"ltr\" style=\"text-align: left;\">falsch<br></p>", &p("richtig"), &p("vielleicht")], 1))), HK)["ok"], true);
    assert!(hard(&pp(&one(qv("Which?", mcl(&["<p class=\"correct\">Mars</p>", "<p>Venus</p>", "<p>Earth</p>"], 0))), HK)));
    assert_eq!(pp(&one(qv("The variance of a Poisson(\u{3BB}) variable is ___. (one word)", json!({"Blank": {"accept": [["\u{3BB}", "lambda"]]}}))), HK)["ok"], true);
    let tr = one(qv("B\u{F6}lgeleri iklim tipleriyle e\u{15F}le\u{15F}tirin.", json!({"Matching": {"pairs": [{"left": "Karadeniz", "right": "Her mevsim ya\u{11F}\u{131}\u{15F}l\u{131}"}, {"left": "Akdeniz", "right": "Yazlar\u{131} s\u{131}cak ve kurak"}, {"left": "\u{130}\u{E7} Anadolu", "right": "Karasal"}], "distractors": ["Tundra"]}})));
    assert_eq!(pp(&tr, HK)["ok"], true);
}

/// critic26a variants: LaTeX ticks and crosses, mark-scheme leads, exam.cls solutions.
#[test]
fn critic26a_variants() {
    for k in ["Mars \\checkmark", "Mars $\\checkmark$", "Mars \\ding{51}", "Mars \\cmark"] {
        assert!(hard(&pp(&one(qv("Which planet is red?", mcl(&["Venus", k, "Earth"], 1))), HK)), "{k}");
    }
    assert!(hard(&pp(&one(qv("Which planet is red?", mcl(&["Venus \\xmark", "Mars", "Earth \\xmark"], 1))), HK)));
    for stem in ["Which planet is red?\nMark scheme: Mars", "Which planet is red?\nAccept: Mars", "Which planet is red?\nIndicative content: Mars, the fourth planet", "Which planet is red?\n\\begin{solution}Mars\\end{solution}"] {
        assert!(hard(&pp(&one(qv(stem, mcl(&["Venus", "Mars", "Earth"], 1))), HK)), "{stem}");
    }
}

/// 634 honest items and 122 honest banks (critic26a): no hard refusal but the poet named in his own
/// book's title (the stem shows the answer); acknowledgements on at most 2%.
#[test]
fn critic26a_honest_corpora() {
    let items = corpus("critic26a_honest.json");
    let (mut refused, mut acks) = (Vec::new(), 0);
    for (i, it) in items.iter().enumerate() {
        match drive(&it["q"], i).0 {
            Out::Refused(e) if !it["q"]["stem"].as_str().unwrap_or("").contains("\u{AB}\u{646}\u{627}\u{644}\u{6CC}\u{BB}") => refused.push(format!("{} {e:?}", it["id"])),
            Out::Acked => acks += 1,
            _ => {}
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
    assert!(acks * 50 <= items.len(), "{acks} of {}", items.len());
    let banks = corpus("critic26a_banks.json");
    let mut refused = Vec::new();
    for b in &banks {
        let qs: Vec<Value> = b["questions"].as_array().unwrap().iter().enumerate().map(|(i, q)| { let mut q = q.clone(); q["id"] = json!(format!("q{:09}", i + 1)); q }).collect();
        let p = json!({"schema_version": 2, "bank": {"id": "b00000001", "name": b["name"], "created": 0, "modified": 0, "details": {"description": b["description"]}, "questions": qs}}).to_string();
        if let (Out::Refused(e), _) = drive_bank(&p)
            && !e.iter().all(|x| x.contains("questions[2].stem") && b["lang"] == "ckb")
        {
            refused.push(format!("{} {e:?}", b["name"]));
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
}
