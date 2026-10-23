//! Round 13 (critic24a, critic24b): LaTeX quiz keys, Matching lefts pointing at a stem list, the
//! bank's own text against every key, numbers read like letters, stems' own labels, and the
//! bank-level and cross-part scans' cost.
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



fn mcl(texts: &[&str], correct: usize) -> Value {
    mc(texts, correct)
}
fn matching(stem: &str, pairs: &[(&str, &str)], d: &[&str]) -> String {
    one(qv(stem, json!({"Matching": {"pairs": pairs.iter().map(|(l, r)| json!({"left": l, "right": r})).collect::<Vec<_>>(), "distractors": d}})))
}
fn frozen(r: &Value) -> bool {
    r["stripped"]["order_reasons"].as_array().is_some_and(|a| !a.is_empty())
}

/// B1: LaTeX quiz packages' key commands, on the choices and pasted into stems.
#[test]
fn b1_latex_key_commands() {
    for ch in [
        ["\\CorrectChoice E7018", "\\choice E6010", "\\choice E6013", "\\choice E6011"],
        ["\\item* E7018", "\\item E6010", "\\item E6013", "\\item E6011"],
        ["\\bonne{E7018}", "\\mauvaise{E6010}", "\\mauvaise{E6013}", "\\mauvaise{E6011}"],
        ["\\correctchoice{E7018}", "\\wrongchoice{E6010}", "\\wrongchoice{E6013}", "\\wrongchoice{E6011}"],
    ] {
        assert!(hard(&pp(&one(qv("Which electrode is low-hydrogen?", mcl(&ch, 0))), HK)), "{ch:?}");
    }
    for stem in ["Arc eye is caused by ultraviolet light.\n\\begin{oneparchoices}\\CorrectChoice True \\choice False\\end{oneparchoices}", "Arc eye is caused by UV.\n\\begin{choices}\n\\item* True\n\\item False\n\\end{choices}"] {
        assert!(hard(&pp(&one(qv(stem, json!({"Tf": {"correct": true}}))), HK)), "{stem}");
    }
    // Mathematics is no key command.
    assert_eq!(pp(&one(qv("Which equals 0.5?", mcl(&["\\frac{1}{2}", "\\sqrt{2}", "\\pi", "\\log 2"], 0))), ACK)["ok"], true);
    // Export attributes and a spreadsheet's flag column.
    assert!(hard(&pp(&one(qv("Which?", mcl(&["<answer fraction=\"100\">E7018</answer>", "<answer fraction=\"0\">E6010</answer>", "<answer fraction=\"0\">E6013</answer>"], 0))), HK)));
    assert!(hard(&pp(&one(qv("Which?", mcl(&["E7018,1", "E6010,0", "E6013,0", "E6011,0"], 0))), HK)));
    assert_eq!(pp(&one(qv("Which is the mass in kg?", mcl(&["2,1", "3,0", "4,0", "5,0"], 0))), HK)["ok"], true);
}

/// B2: a Matching left item pointing at its right through a list in the stem, or holding it in
/// its blank.
#[test]
fn b2_matching_lefts_point_to_rights() {
    let ores = [("Bauxite", "Aluminium"), ("Hematite", "Iron"), ("Galena", "Lead"), ("Cinnabar", "Mercury")];
    let list = "Write the letter of the metal next to each ore.\na. Tin\nb. Mercury\nc. Lead\nd. Iron\ne. Aluminium";
    let letter = |m: &str| ["Tin", "Mercury", "Lead", "Iron", "Aluminium"].iter().position(|x| *x == m).map(|p| (b'a' + p as u8) as char).unwrap();
    for form in ["{l} ___{x}___", "{l} \u{2014} {X}", "{l} ({X})", "{l} (Answer: {X})"] {
        let pairs: Vec<(String, String)> = ores.iter().map(|(l, r)| (form.replace("{l}", l).replace("{x}", &letter(r).to_string()).replace("{X}", &letter(r).to_ascii_uppercase().to_string()), r.to_string())).collect();
        let pr: Vec<(&str, &str)> = pairs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        assert!(hard(&pp(&matching(list, &pr, &["Tin"]), HK)), "{form}");
        // One line, one space apart.
        assert!(hard(&pp(&matching("Match each ore.\nA. Tin B. Mercury C. Lead D. Iron E. Aluminium", &pr, &["Tin"]), HK)), "{form}");
    }
    let http = [("HTTP (5)", "80"), ("HTTPS (4)", "443"), ("SMTP (1)", "25"), ("DNS (2)", "53")];
    assert!(hard(&pp(&matching("Match each protocol to its port.\nOptions: 1. 25  2. 53  3. 22  4. 443  5. 80", &http, &["22"]), HK)));
    let filled: Vec<(String, &str)> = ores.iter().map(|(l, r)| (format!("{l} ___{r}___"), *r)).collect();
    let fr: Vec<(&str, &str)> = filled.iter().map(|(a, b)| (a.as_str(), *b)).collect();
    assert!(hard(&pp(&matching("Match each ore to its metal.", &fr, &["Tin"]), HK)));
    // Honest: pointers that name another item, and no list.
    assert_eq!(pp(&matching(list, &[("Bauxite (a)", "Aluminium"), ("Hematite (b)", "Iron"), ("Galena (e)", "Lead")], &["Tin"]), HK)["ok"], true);
    assert_eq!(pp(&matching("Match each ore to its metal.", &ores, &["Tin"]), HK)["ok"], true);
}

/// B3: the bank's description and name against every question's key.
#[test]
fn b3_bank_text_against_all_keys() {
    let qs = || vec![qv("Which electrode is low-hydrogen?", mcl(&["E7018", "E6010", "E6013"], 0)), qv("What is the capital of Lithuania?", json!({"Short": {"sample": "Vilnius"}})), qv("Arc eye is caused by UV light.", json!({"Tf": {"correct": true}}))];
    let with = |field: &str, text: &str| {
        let mut v: Value = serde_json::from_str(&bank(qs())).unwrap();
        if field == "name" {
            v["bank"]["name"] = json!(text);
        } else {
            v["bank"]["details"]["description"] = json!(text);
        }
        v.to_string()
    };
    for d in ["Answers: 1. E7018 2. Vilnius 3. True", "Answer key: E7018, Vilnius, True", "Key:\n1. E7018\n2. Vilnius\n3. True", "ANSWER: Vilnius", "The correct answer is: E7018.", "Answer key: A, B, True", "Key: ABT", "Answers: A B T", "Teacher notes: answers are E7018, Vilnius, true."] {
        assert!(hard(&pp(&with("description", d), HK)), "{d}");
    }
    assert!(hard(&pp(&with("name", "Quiz 3 (Answers: 1A 2B 3T)"), HK)));
    for d in ["Welding unit 3 review.", "Key terms: electrode, arc, flux.", "Answer every question; true or false items count once."] {
        assert_eq!(pp(&with("description", d), HK)["ok"], true, "{d}");
    }
}

/// B4: numbers read like letters: out of range, or with a unit or noun, they are content.
#[test]
fn b4_number_lists_are_content_out_of_range() {
    for (ch, k) in [
        (vec!["1 and 3", "7 and 9", "11 and 13", "7 only"], 0), (vec!["1 and 2 only", "3 and 4 only", "5 and 6 only", "7 and 8 only"], 0),
        (vec!["400 to 700 nm", "10 to 100 nm", "1 to 10 mm", "Both 10 to 100 nm and 1 to 10 mm"], 0), (vec!["pH 7 only", "pH 2", "pH 12", "pH 4"], 0),
    ] {
        let (_, r) = drive(&json!({"stem": "Which?", "kind": mcl(&ch, k)}), 0);
        assert!(r["ok"] == true && !frozen(&r), "{ch:?}: {r}");
    }
    // In range with labels: still a reference that freezes the order.
    let (_, r) = drive(&json!({"stem": "Which?", "kind": mcl(&["1. Mitochondria", "2. Ribosome", "3. Both 1 and 2", "4. Nucleus"], 2)}), 0);
    assert!(frozen(&r), "{r}");
}

/// The part's own label and labels stripped from shuffled choices name nothing (critic24a: 213
/// of 400 exam pastes were refused).
#[test]
fn own_part_labels_name_nothing() {
    let part = |stem: &str, key: usize| json!({"stem": stem, "kind": mcl(&["a. 2.0 s", "b. 4.0 s", "c. 1.0 s", "d. 1.4 s"], key)});
    for (stem, key) in [("(a) How long does it take to land?", 0), ("a) How long does it take to land?", 0), ("[a] How long does it take?", 0), ("(b) How long does it take?", 1)] {
        let q = qv("A ball is dropped from 20 m.", json!({"Multipart": {"parts": [part(stem, key), part("(z) Its speed on landing?", 2)]}}));
        let (out, r) = drive(&q, 0);
        assert!(r["ok"] == true, "{stem}: {out:?} {r}");
    }
    let (_, r) = drive(&qv("a) Which time is right?", mcl(&["a. 2.0 s", "b. 4.0 s", "c. 1.0 s"], 0)), 0);
    assert_eq!(r["ok"], true, "{r}");
    // Stripped labels on shuffled choices name nothing, wherever the letter stands.
    let stem = one(qv("How long does it take to land?\n\u{2192} a", mcl(&["a. 2.0 s", "b. 4.0 s", "c. 1.0 s", "d. 1.4 s"], 0)));
    assert_eq!(pp(&stem, r#"{"hide_key":true,"labels":[{"question":0,"choice":"strip"}]}"#)["ok"], true);
    // Kept, the label travels with its choice: "→ a" names it.
    assert!(hard(&pp(&stem, r#"{"hide_key":true,"labels":[{"question":0,"choice":"keep"}],"order_acks":[{"question":0}]}"#)));
    // The stem's own label with an order students see is still its numbering.
    let mut q = qv("(a) How long does it take to land?", mcl(&["2.0 s", "4.0 s", "1.0 s"], 0));
    q["keep_order"] = json!(true);
    assert_eq!(pp(&one(q), r#"{"hide_key":true,"order_acks":[{"question":0}]}"#)["ok"], true);
    // A marked letter still names the key where students see the order.
    let mut q = qv("Which time is right? [A]", mcl(&["2.0 s", "4.0 s", "1.0 s"], 0));
    q["keep_order"] = json!(true);
    assert!(hard(&pp(&one(q), r#"{"hide_key":true,"order_acks":[{"question":0}]}"#)));
}

/// Honest Matching and lists from critic24a: names_left is low risk, list punctuation and
/// script marks are no outliers, a fraction before words is a quantity.
#[test]
fn honest_critic24a_shapes() {
    for pairs in [vec![("Wool", "Sheep"), ("Silk", "Silkworm cocoons"), ("Linen", "Flax"), ("Cotton", "Seed hairs of the cotton plant")], vec![("Madra", "Dog"), ("Cat", "Cat"), ("Teach", "House")]] {
        let r = pp(&matching("Match.", &pairs, &["River"]), HK);
        assert!(low(&r), "{pairs:?}: {r}");
    }
    for (ch, k) in [
        (vec!["Nitrogen;", "Chlorine;", "Oxygen;", "Xenon."], 3), (vec!["1/4 inch per foot;", "1 inch per foot;", "1/16 inch per foot;", "No slope."], 0),
        (vec!["\u{179F}\u{17C0}\u{1798}\u{179A}\u{17B6}\u{1794};", "\u{1794}\u{17B6}\u{178F}\u{17CB}\u{178A}\u{17C6}\u{1794}\u{1784};", "\u{1780}\u{17C6}\u{1796}\u{178F};", "\u{1797}\u{17D2}\u{1793}\u{17C6}\u{1796}\u{17C1}\u{1789}."], 0),
    ] {
        assert_eq!(pp(&one(qv("Which?", mcl(&ch, k))), HK)["ok"], true, "{ch:?}");
    }
    let si = matching("Match.", &[("\u{DC1}\u{DCA}\u{200D}\u{DBB}\u{DD3} \u{DBD}\u{D82}\u{D9A}\u{DCF}\u{DC0}", "\u{DC1}\u{DCA}\u{200D}\u{DBB}\u{DD3} \u{DA2}\u{DBA}\u{DC0}\u{DBB}\u{DCA}\u{DB0}\u{DB1}\u{DB4}\u{DD4}\u{DBB}"), ("\u{DA2}\u{DB4}\u{DCF}\u{DB1}\u{DBA}", "\u{DA7}\u{DDD}\u{D9A}\u{DD2}\u{DBA}\u{DDD}"), ("\u{DB4}\u{DCA}\u{200D}\u{DBB}\u{D82}\u{DC1}\u{DBA}", "\u{DB4}\u{DD0}\u{DBB}\u{DD2}\u{DC3}\u{DD2}\u{DBA}")], &["\u{DBB}\u{DDD}\u{DB8}\u{DBA}"]);
    assert_eq!(pp(&si, HK)["ok"], true);
}

/// critic24b: a topic of nothing visible is dropped; a refusal for the author only says so.
#[test]
fn invisible_topics_and_author_only_refusals() {
    let mut q = qv("The Sun is a star.", json!({"Tf": {"correct": true}}));
    q["topics"] = json!(["\u{200D}", "\u{200E}", "\u{200C}\u{200D}", "astronomy"]);
    let r = j(&validate_json(one(q).as_bytes(), ""));
    assert_eq!(r["payload"]["question"]["topics"], json!(["astronomy"]), "{r}");
    let r = pp(&one(qv("Which planet is red? Answer: Mars", mcl(&["Venus", "Mars"], 1))), HK);
    assert_eq!(r["author_only_refusal"], true, "{r}");
}

/// 727 honest Moodle-style items (critic24a): no hard refusal but the critic's own inconsistent
/// item (rights labelled a, b, a, d); acknowledgements on at most 2%.
#[test]
fn critic24a_honest_corpus() {
    let items = corpus("critic24a_honest.json");
    let (mut refused, mut acks) = (Vec::new(), 0);
    for (i, it) in items.iter().enumerate() {
        let (out, _) = drive(&it["q"], i);
        match out {
            Out::Refused(e) if it["q"]["stem"] != "Classify the animals." => refused.push(format!("{} {e:?}", it["id"])),
            Out::Acked => acks += 1,
            _ => {}
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
    assert!(acks * 50 <= items.len(), "{acks} of {}", items.len());
}

/// critic24b's cost shapes, at two sizes: the bank-level scan with 4,096 keys sharing a first
/// character against stems of answer lines, and Multipart parts holding the other parts' keys.
/// Linear in size (ratio of the fastest of three under 3 for twice the size) and bounded.
#[test]
fn critic24b_scans_are_linear() {
    let tag = |mut i: usize| (0..3).map(|_| { let c = (b'a' + (i % 26) as u8) as char; i /= 26; c }).collect::<String>();
    let bank_shape = |stems: usize| {
        let mut qs = Vec::new();
        let mut ki = 0;
        for b in 0..64 {
            let accept: Vec<Vec<String>> = (0..4).map(|_| (0..16).map(|_| { ki += 1; format!("q-q-q-q-q-q-q-q-q-{}", tag(ki - 1)) }).collect()).collect();
            let mut q = qv("Fill: ___ and ___ and ___ and ___.", json!({"Blank": {"accept": accept}}));
            q["id"] = json!(format!("k{b:07}"));
            qs.push(q);
        }
        let stem = format!("Explain.\n{}", format!("answer: {}\n", "q-".repeat(30)).repeat(33));
        for _ in 0..stems {
            let mut q = qv(&stem, json!({"Short": {"sample": ""}}));
            q["keyless"] = json!(true);
            qs.push(q);
        }
        bank(qs)
    };
    let keys: Vec<Vec<String>> = (0..16).map(|j| (0..26).map(|k| format!("w{}", tag(j * 26 + k))).collect()).collect();
    let mp_shape = |nq: usize| {
        let parts: Vec<Value> = (0..16).map(|j| {
            let others: Vec<&str> = keys.iter().enumerate().filter(|(jj, _)| *jj != j).flat_map(|(_, ks)| ks[..16].iter().map(String::as_str)).collect();
            let mut stem = format!("Given {} ", others.join(" "));
            stem.push_str(&".".repeat(8000usize.saturating_sub(stem.len())));
            json!({"stem": stem, "kind": {"Mc": {"answers": keys[j].iter().enumerate().map(|(k, t)| json!({"text": t, "correct": k < 16, "locked": false})).collect::<Vec<_>>()}}})
        }).collect();
        bank((0..nq).map(|_| qv("A problem.", json!({"Multipart": {"parts": parts.clone()}}))).collect())
    };
    let time = |p: &str| (0..3).map(|_| { let t = std::time::Instant::now(); std::hint::black_box(publish_prepare_json(p.as_bytes(), HK)); t.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min);
    for (name, small, big) in [("bank", bank_shape(200), bank_shape(400)), ("multipart", mp_shape(4), mp_shape(8))] {
        let (a, b) = (time(&small), time(&big));
        assert!(b / a < 3.0 || b < 0.05, "{name}: {a:.3} s -> {b:.3} s");
        assert!(b < 2.0, "{name}: {b:.3} s for {} bytes", big.len());
    }
}
