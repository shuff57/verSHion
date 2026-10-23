//! Round 12 (critic23a, critic23b): another part's or question's key in a served stem, word
//! banks judged by order, labelled numbers read without their labels, pasted review pages, and the
//! scanners' cost on every field.
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
fn low(r: &Value) -> bool {
    r["ok"] == false && r["errors"].as_array().is_some_and(|e| e.iter().all(|x| x["ack_allowed"] == true))
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
fn pmc(stem: &str, texts: &[&str], correct: usize) -> Value {
    json!({"stem": stem, "kind": mc(texts, correct)})
}
const HK: &str = r#"{"hide_key":true}"#;
const ACK: &str = r#"{"hide_key":true,"ack_formatting":[0]}"#;

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


/// The ten critic23a Multipart bases: main stem, part (a) stem, choices, key, part (b) question.
const MPB: [(&str, &str, &[&str], usize, &str); 10] = [
    ("A car goes from rest to 24 m/s in 6.0 s.", "(a) Its acceleration is:", &["2.0 m/s\u{B2}", "4.0 m/s\u{B2}", "6.0 m/s\u{B2}", "8.0 m/s\u{B2}"], 1, "How far does it travel?"),
    ("Consider 2Mg + O\u{2082} \u{2192} 2MgO.", "(a) Which substance is oxidised?", &["Mg", "O\u{2082}", "MgO"], 0, "Write the half-equation."),
    ("f(x) = x\u{B2} \u{2212} 4x + 3.", "(a) The vertex is at x =", &["1", "2", "3", "4"], 1, "Find the minimum value of f."),
    ("Une voiture freine.", "(a) Quelle force s'oppose au mouvement ?", &["Le poids", "Les frottements", "La pouss\u{E9}e"], 1, "Expliquez."),
    ("Pea plants: Tt \u{D7} Tt.", "(a) The ratio tall : short is", &["1:1", "3:1", "1:2:1"], 1, "How many of 400 offspring are short?"),
    ("Cells and energy.", "(a) Which organelle makes most ATP?", &["Ribosome", "Mitochondrion", "Golgi apparatus", "Nucleus"], 1, "Describe how it uses oxygen."),
    ("Un circuito tiene R = 10 \u{3A9} e I = 2 A.", "(a) \u{BF}Cu\u{E1}l es el voltaje?", &["5 V", "12 V", "20 V", "0,2 V"], 2, "\u{BF}Qu\u{E9} potencia disipa?"),
    ("Read the passage about the Nile.", "(a) Into which sea does the Nile flow?", &["Red Sea", "Mediterranean Sea", "Dead Sea", "Arabian Sea"], 1, "Name one city on its delta."),
    ("Ein Ball wird senkrecht hochgeworfen.", "(a) Am h\u{F6}chsten Punkt ist die Geschwindigkeit:", &["null", "maximal", "negativ"], 0, "Wie gro\u{DF} ist dort die Beschleunigung?"),
    ("A bond pays 5 % on $1000.", "(a) The annual coupon is:", &["$5", "$50", "$500", "$105"], 1, "What is the coupon over 3 years?"),
];

fn multi(main: &str, parts: Vec<Value>) -> Value {
    qv(main, json!({"Multipart": {"parts": parts}}))
}

/// B1: a part stem stating another part's key. Answer lines are high risk; the bare value is
/// low risk; a statement that happens to use the word, and an instruction, publish.
#[test]
fn b1_parts_state_other_parts_keys() {
    for (main, a, ch, k, b) in MPB {
        let key = ch[k];
        for form in [format!("Answer to (a): {key}.\n(b) "), format!("ANSWER: {key}\n(b) "), format!("Respuesta (a): {key}\n(b) ")] {
            let q = multi(main, vec![pmc(a, ch, k), json!({"stem": format!("{form}{b}"), "kind": {"Short": {"sample": "x"}}})]);
            assert!(hard(&pp(&one(q), HK)), "{form}{b}");
        }
        for form in [format!("(b) Using your answer from (a) ({key}), "), format!("(b) If you could not do (a), use {key}. ")] {
            let q = multi(main, vec![pmc(a, ch, k), json!({"stem": format!("{form}{b}"), "kind": {"Short": {"sample": "x"}}})]);
            let r = pp(&one(q.clone()), HK);
            assert!(low(&r), "{form}{b}: {r}");
            assert_eq!(pp(&one(q), ACK)["ok"], true, "{form}{b}");
        }
        // Honest: the instruction without the value.
        let q = multi(main, vec![pmc(a, ch, k), json!({"stem": format!("(b) If you could not answer (a), use the value you found. {b}"), "kind": {"Short": {"sample": "x"}}})]);
        assert_eq!(pp(&one(q), HK)["ok"], true, "{main}");
    }
    for (s, v) in [("(a) The Moon produces its own light.", false), ("(a) Copper conducts electricity.", true), ("(a) Spiders are insects.", false), ("(a) La Tierra gira alrededor del Sol.", true)] {
        let w = if v { "True" } else { "False" };
        for b in [format!("Answer to (a): {w}.\n(b) Explain."), format!("(b) (a) is {w}: explain why."), format!("ANSWER: {w}\n(b) Explain.")] {
            let q = multi("Read each statement.", vec![json!({"stem": s, "kind": {"Tf": {"correct": v}}}), json!({"stem": b, "kind": {"Short": {"sample": "x"}}})]);
            assert!(hard(&pp(&one(q), HK)), "{b}");
        }
    }
    // A statement that uses another part's key word, and the main stem's own data, publish.
    let q = multi("A ball is thrown straight up.", vec![pmc("At the top, its velocity is:", &["negative", "zero", "maximum"], 1), json!({"stem": "Its acceleration at the top is zero.", "kind": {"Tf": {"correct": false}}})]);
    assert_eq!(pp(&one(q), HK)["ok"], true);
    let q = multi("A car goes from 0 to 24 m/s in 6.0 s; then it brakes at 4.0 m/s\u{B2}.", vec![pmc("(a) Its acceleration is:", &["2.0 m/s\u{B2}", "4.0 m/s\u{B2}", "6.0 m/s\u{B2}"], 1), json!({"stem": "(b) Braking at 4.0 m/s\u{B2}, how long does it take to stop?", "kind": {"Short": {"sample": "x"}}})]);
    assert_eq!(pp(&one(q), HK)["ok"], true);
    // A number named whole: "4.05" is not "4.0".
    let q = multi("Data.", vec![pmc("(a) x =", &["2", "3"], 0), json!({"stem": "(b) Find f(2.5).", "kind": {"Short": {"sample": "x"}}})]);
    assert_eq!(pp(&one(q), HK)["ok"], true);
}

/// B1, bank level: a question's stem opening with another question's answer line.
#[test]
fn b1_bank_answer_line_of_another_question() {
    for i in 0..MPB.len() {
        let (_, a, ch, k, _) = MPB[i];
        let (_, a2, ch2, k2, _) = MPB[(i + 1) % MPB.len()];
        let q1 = qv(a.trim_start_matches("(a) "), mc(ch, k));
        let q2 = qv(&format!("ANSWER: {}\n\n{}", ch[k], a2.trim_start_matches("(a) ")), mc(ch2, k2));
        let r = pp(&bank(vec![q1.clone(), q2]), HK);
        assert!(hard(&r), "{a}: {r}");
        // Honest: the same bank without the line.
        let q2 = qv(a2.trim_start_matches("(a) "), mc(ch2, k2));
        assert_eq!(pp(&bank(vec![q1, q2]), HK)["ok"], true, "{a}");
    }
    // A number is named whole: question 1's key "2" is not named by "Answer: 2.5 m".
    let q1 = qv("The vertex is at x =", mc(&["1", "2", "3", "4"], 1));
    let q2 = qv("Write your result like this. Answer: 2.5 m", json!({"Short": {"sample": "a length"}}));
    assert_eq!(pp(&bank(vec![q1.clone(), q2]), HK)["ok"], true);
    let q2 = qv("Write your result like this. Answer: 2 m", json!({"Short": {"sample": "a length"}}));
    assert!(hard(&pp(&bank(vec![q1, q2]), HK)));
}

/// B2: a word bank in the order of the blanks is the key; in another order it is allowed; the one
/// blank's answer first in the bank is low risk.
#[test]
fn b2_word_banks_by_order() {
    let blank = |stem: &str, a: &[&str]| one(qv(stem, json!({"Blank": {"accept": a.iter().map(|x| vec![x]).collect::<Vec<_>>()}})));
    let s = "Plants take in ___ through their leaves and release ___.";
    let k = ["carbon dioxide", "oxygen"];
    for stem in [format!("Word bank: carbon dioxide, oxygen, glucose, nitrogen\n\n{s}"), format!("(carbon dioxide / oxygen / glucose / nitrogen)\n{s}"), format!("{s}\nWord bank: carbon dioxide, glucose, oxygen, nitrogen"), format!("{s}\n\nWord bank:\n\u{2022} glucose\n\u{2022} carbon dioxide\n\u{2022} nitrogen\n\u{2022} oxygen"), format!("{s} (glucose, carbon dioxide, oxygen)")] {
        // (Round 13: two answers in blank order are low risk, a chance one time in two.)
        let r = pp(&blank(&stem, &k), HK);
        assert!(low(&r), "{stem}: {r}");
        assert!(r["errors"][0]["hint"].as_str().unwrap().contains("order"), "{r}");
    }
    // Three or more in blank order: high risk.
    let three = "The ___ pumps blood; the ___ filter it; the ___ exchange gases.";
    for stem in [format!("Word bank: heart, liver, kidneys, lungs\n{three}"), format!("{three}\n\u{2022} heart\n\u{2022} kidneys\n\u{2022} liver\n\u{2022} lungs")] {
        let r = pp(&blank(&stem, &["heart", "kidneys", "lungs"]), HK);
        assert!(hard(&r), "{stem}: {r}");
    }
    assert_eq!(pp(&blank(&format!("Word bank: lungs, heart, liver, kidneys\n{three}"), &["heart", "kidneys", "lungs"]), HK)["ok"], true);
    for stem in [format!("Word bank: oxygen, glucose, nitrogen, carbon dioxide\n\n{s}"), format!("{s}\n\nWord bank:\n\u{2022} nitrogen\n\u{2022} oxygen\n\u{2022} glucose\n\u{2022} carbon dioxide"), format!("{s} (glucose, oxygen, nitrogen, carbon dioxide)"), "Plants take in ___ and release ___. Word bank: glucose, oxygen, nitrogen, carbon dioxide".to_string()] {
        let a: &[&str] = &k;
        assert_eq!(pp(&blank(&stem, a), HK)["ok"], true, "{stem}");
    }
    let r = pp(&blank("Word bank: Mars, Venus, Jupiter.\nThe red planet is ____.", &["Mars"]), HK);
    assert!(low(&r), "{r}");
    // Matching with a fixed order: a stem list of the rights in pair order.
    let m = |stem: &str, ko: bool| {
        let mut q = qv(stem, json!({"Matching": {"pairs": [{"left": "Hastings", "right": "1066"}, {"left": "Magna Carta", "right": "1215"}, {"left": "Columbus", "right": "1492"}, {"left": "Bastille", "right": "1789"}], "distractors": []}}));
        q["keep_order"] = json!(ko);
        one(q)
    };
    assert!(hard(&pp(&m("Match each event to its year. Choose from: 1066, 1215, 1492, 1789", true), HK)));
    assert_eq!(pp(&m("Match each event to its year. Choose from: 1492, 1066, 1789, 1215", true), HK)["ok"], true);
}

/// B3: labelled numbers read without their labels: every critic23a base in every numbering style
/// freezes no more often than unlabelled.
#[test]
fn b3_labelled_bases_freeze_no_more_than_unlabelled() {
    let bases = corpus("critic23a_bases.json");
    let roman = ["i", "ii", "iii", "iv", "v", "vi"];
    type Style = fn(&str, usize, &[&str]) -> String;
    let styles: [(&str, Style); 7] = [
        ("none", |t, _, _| t.to_string()),
        ("abc", |t, i, _| format!("{}. {t}", (b'a' + i as u8) as char)),
        ("ABCD", |t, i, _| format!("{}. {t}", (b'A' + i as u8) as char)),
        ("123", |t, i, _| format!("{}. {t}", i + 1)),
        ("iii", |t, i, r| format!("{}. {t}", r[i])),
        ("IIII", |t, i, r| format!("{}. {t}", r[i].to_uppercase())),
        ("aiken", |t, i, _| format!("{}) {t}", (b'A' + i as u8) as char)),
    ];
    let mut rates = Vec::new();
    for (name, f) in styles {
        let mut frozen = Vec::new();
        for (n, b) in bases.iter().enumerate() {
            let ch: Vec<String> = b["choices"].as_array().unwrap().iter().enumerate().map(|(i, t)| f(t.as_str().unwrap(), i, &roman)).collect();
            let cor: Vec<u64> = b["correct"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
            let q = json!({"stem": b["stem"], "kind": {"Mc": {"answers": ch.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": cor.contains(&(i as u64)), "locked": false})).collect::<Vec<_>>()}}});
            let (_, r) = drive(&q, n);
            if r["stripped"]["order_reasons"].as_array().is_some_and(|a| !a.is_empty()) || r["stripped"]["keep_order"] == true {
                frozen.push(ch.join(" | "));
            }
        }
        rates.push((name, frozen));
    }
    let base = rates[0].1.len();
    for (name, f) in &rates {
        assert!(f.len() <= base, "{name}: {} > {base}: {:?}", f.len(), f);
    }
}

/// 614 honest Moodle-style items (critic23a): no hard refusal; acknowledgements on 16 pasted
/// "Select one: True / False" previews (low risk by the owner's decision) and at most 2% of the
/// rest.
#[test]
fn critic23a_honest_corpus() {
    let items = corpus("critic23a_honest.json");
    let (mut refused, mut acks, mut pasted) = (Vec::new(), 0, 0);
    for (i, it) in items.iter().enumerate() {
        let (out, _) = drive(&it["q"], i);
        match out {
            Out::Refused(e) => refused.push(format!("{} {e:?}", it["id"])),
            Out::Acked if it["q"]["stem"].as_str().is_some_and(|s| s.contains("Select one:")) => pasted += 1,
            Out::Acked => acks += 1,
            _ => {}
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
    assert!(pasted <= 16 && acks * 50 <= items.len(), "{acks} + {pasted} of {}", items.len());
}

/// critic23a's 264 marked items: every template refused but two documented limits (a hint
/// giving the key's initial; a Kahoot row's bare index).
#[test]
fn critic23a_markers() {
    let mut served = Vec::new();
    for (i, it) in corpus("critic23a_markers.json").iter().enumerate() {
        if matches!(it["template"].as_str(), Some("hint_initial" | "kahoot_index_bare")) {
            continue;
        }
        let (out, _) = drive(&it["q"], i);
        if out == Out::Served {
            served.push(format!("{} {}", it["template"], it["q"]["stem"]));
        }
    }
    assert!(served.is_empty(), "{}", served.join("\n"));
}

/// The cheap variants: ExamView rows, review-page verdict lines, compact keys, arrows, Blackboard
/// pools, bank answer keys, ordinals with a fixed order, the pasted True/False preview, a locked
/// key that names the others.
#[test]
fn cheap_variants() {
    let ch = ["Butt joint", "Dovetail joint", "Lap joint", "Mitre joint"];
    let s = "Which joint is traditionally used for drawer fronts?";
    for stem in [format!("{s}\nANS: B\tPTS: 1\tDIF: Easy"), format!("{s}\n\nANS:  B  PTS:  1"), format!("{s}  [Key: 4B]"), format!("{s}\n\u{2192} Dovetail joint"), format!("{s}\nCorrect answer(s): 2"), format!("{s}\nCorrect answer(s): B")] {
        assert!(hard(&pp(&one(qv(&stem, mc(&ch, 1))), HK)), "{stem}");
    }
    for k in ["Correct!\nDovetail joint", "Correct Answer\nDovetail joint", "Correcta\nDovetail joint"] {
        let r = pp(&one(qv(s, mc(&["Butt joint", k, "Lap joint", "Mitre joint"], 1))), HK);
        assert!(hard(&r), "{k}: {r}");
    }
    assert!(hard(&pp(&one(qv("Which aperture lets in the most light?\nCorrect Answer: f/1.8", mc(&["f/16", "f/8", "f/4", "f/1.8"], 3))), HK)));
    for d in ["Answer key: 1B 2T", "Key: 1-B, 2-True"] {
        let b = bank(vec![qv(s, mc(&ch, 1)), qv("Wood is a polymer.", json!({"Tf": {"correct": true}}))]);
        let mut v: Value = serde_json::from_str(&b).unwrap();
        v["bank"]["details"]["description"] = json!(d);
        assert!(hard(&pp(&v.to_string(), HK)), "{d}");
    }
    let mut q = qv(&format!("{s}\nAnswer: the second option"), mc(&ch, 1));
    q["keep_order"] = json!(true);
    assert!(hard(&pp(&one(q), r#"{"hide_key":true,"order_acks":[{"question":0}]}"#)));
    // Honest neighbours.
    for stem in [format!("{s}\nANS: (show your work)"), format!("{s} Answer: 2 marks each.")] {
        assert_eq!(pp(&one(qv(&stem, mc(&ch, 1))), HK)["ok"], true, "{stem}");
    }
    // Pasted preview: low risk, not hard.
    let r = pp(&one(qv("Water is a compound.\n\nSelect one:\nTrue\nFalse", json!({"Tf": {"correct": true}}))), HK);
    assert!(low(&r), "{r}");
    // A locked key naming two others is a reference, not a locked answer.
    let lock = json!({"Mc": {"answers": [{"text": "Mitochondria", "correct": false, "locked": false}, {"text": "Chloroplasts", "correct": false, "locked": false}, {"text": "Ribosomes", "correct": false, "locked": false}, {"text": "Both mitochondria and chloroplasts", "correct": true, "locked": true}]}});
    assert_eq!(pp(&one(qv("Which organelles contain their own DNA?", lock)), HK)["ok"], true);
    // A choice that is only a number with a full stop is content.
    let r = pp(&one(qv("How many senators does each US state have?", mc(&["1.", "2.", "4.", "Depends on population."], 1))), HK);
    assert_eq!(r["ok"], true, "{r}");
}

/// critic23a's bullets_bank: a long run of bulleted lines is read once (restarting at each of its
/// lines was quadratic). Cost relative to a plain stem of the same size, fastest of 5.
#[test]
fn bullet_runs_are_linear() {
    let stem = |line: &str| format!("{}\nWord bank:\n{}", "___ ".repeat(50), line.repeat(7600 / line.len()));
    let body = |s: &str| {
        let qs: Vec<Value> = (0..60).map(|i| { let mut q = qv(s, json!({"Blank": {"accept": vec![vec!["oxygen"]; 50]}})); q["id"] = json!(format!("q{:08}", i + 1)); q }).collect();
        json!({"schema_version": 2, "bank": {"id": "b00000001", "name": "Bank", "created": 0, "modified": 0, "details": {"description": ""}, "questions": qs}}).to_string()
    };
    let (bul, plain) = (body(&stem("\u{2022} oxygen\n")), body(&stem("xx oxygen yy\n")));
    let time = |p: &str| (0..5).map(|_| { let t = std::time::Instant::now(); publish_prepare_json(p.as_bytes(), HK); t.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min);
    let (a, b) = (time(&bul), time(&plain));
    assert!(a < 4.0 * b + 0.02, "bulleted {a:.4} s, plain {b:.4} s");
}

/// Round 12 cost work keeps its readings: a stem listing a correct and a wrong choice on lines of
/// their own names nothing; a right item holding its left item is still found in wide scripts.
#[test]
fn shortcuts_keep_readings() {
    assert_eq!(pp(&one(qv("Which planet is red?\nVenus\nMars", mc(&["Venus", "Mars", "Earth"], 1))), HK)["ok"], true);
    assert!(hard(&pp(&one(qv("Which planet is red?\nMars", mc(&["Venus", "Mars", "Earth"], 1))), HK)));
    let r = share_core::format::matching_names_left(&[("France", "Paris (France)"), ("Japan", "Tokyo"), ("Peru", "Lima")]);
    assert!(r.iter().any(|x| x.0 == 0), "{r:?}");
}
