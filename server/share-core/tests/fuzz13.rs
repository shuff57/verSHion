//! Round 13 (critic24b B3): no input panics. Random texts from an alphabet of digits,
//! punctuation, labels, answer and verdict words, marks and scripts through every scanner and the
//! whole publish path (a panic is a wasm trap). Deterministic seeds.
use serde_json::{Value, json};
use share_core::format::{self, KeyInfo};
use share_core::labels;
use share_core::wasm::*;

const TOKENS: &[&str] = &[
    "0", "1", "2", "7", "10", "100", "%", "(", ")", "[", "]", "{", "}", "-", "--", ":", ";", ",", ".", "!", "?", "=", "~", "#", "*", "**", "_", "___", "/", "\\", "|", "<", ">",
    "Answer", "answer", "ANS", "Key", "correct", "Correct!", "incorrect", "right", "wrong", "True", "False", "T", "F", "yes", "no", "A", "B", "c", "a)", "A.", "(a)", "I.",
    "II", "iv", "Both", "and", "only", "of the above", "None", "1.", "2)", "Option", "choice", "Word bank:", "is", "the", "✓", "✗", "→", "•", "é", "\u{301}", "\u{200D}", "\u{200C}",
    "\u{FE0F}", "답", "정답", "答案", "是", "ශ්‍රී", "ক", "\u{0E33}", "&amp;", "&#10003;", "$", "^", "\\CorrectChoice", "\\choice", "\\item*", " ", " ", " ", "\n", "\t", "＿", "（", "）",
    "Venus", "Mars", "x", "pH", "nm", "1 and 3", "a. Tin", "(Answer: E)", "___e___",
    // Round 14 (critic26b): characters whose case forms change length or count.
    "\u{130}", "\u{1E9E}", "\u{1C5}", "\u{1C8}", "\u{1CB}", "\u{1F2}", "\u{149}", "\u{DF}", "\u{17F}", "\u{212A}", "\u{212B}", "\u{390}", "\u{FB03}", "\u{FB00}",
    "\u{1C90}", "\u{10D0}", "\u{131}", "\u{3C2}", "\u{3A3}", "e\u{301}\u{323}", "\u{1F80}", "\u{1FB3}", "\u{130}stanbul est", "\u{1E9E} ist", " est ", " is ", " es ", " \u{E8} ",
    "\u{130}zmir ist \u{E4}lter", "\u{130}stanbul is \u{FC}lke\u{2019}s", "\u{FB01}", "\u{1E9E}\u{1E9E}", "\u{130}\u{130}",
];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn text(&mut self, max: u64) -> String {
        let n = 1 + self.next() % max;
        let mut s = String::new();
        for _ in 0..n {
            s.push_str(TOKENS[(self.next() % TOKENS.len() as u64) as usize]);
            if self.next().is_multiple_of(3) {
                s.push(' ');
            }
        }
        s
    }
}

fn kinds(r: &mut Rng) -> Value {
    let t = |r: &mut Rng| r.text(8);
    match r.next() % 6 {
        0 | 1 => {
            let n = 2 + r.next() % 4;
            let k = r.next() % n;
            json!({"Mc": {"answers": (0..n).map(|i| json!({"text": t(r), "correct": i == k, "locked": r.next().is_multiple_of(7)})).collect::<Vec<_>>()}})
        }
        2 => json!({"Tf": {"correct": r.next().is_multiple_of(2)}}),
        3 => json!({"Blank": {"accept": (0..1 + r.next() % 3).map(|_| vec![t(r)]).collect::<Vec<_>>()}}),
        4 => json!({"Matching": {"pairs": (0..2 + r.next() % 3).map(|_| json!({"left": t(r), "right": t(r)})).collect::<Vec<_>>(), "distractors": [t(r)]}}),
        _ => json!({"Multipart": {"parts": (0..2).map(|_| json!({"stem": t(r), "kind": {"Mc": {"answers": [{"text": t(r), "correct": true, "locked": false}, {"text": t(r), "correct": false, "locked": false}]}}})).collect::<Vec<_>>()}}),
    }
}

#[test]
fn scanners_never_panic() {
    let scale: u64 = std::env::var("FUZZ_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let mut r = Rng(0x9E37_79B9_7F4A_7C15 ^ scale);
    for case in 0..60_000 * scale {
        let a = r.text(10);
        let b = r.text(6);
        let c = r.text(6);
        let res = std::panic::catch_unwind(|| {
            let _ = format::features(&a);
            let _ = format::check(&[(&a, true), (&b, false), (&c, false)]);
            let _ = format::check(&[(&b, false), (&a, true), (&c, false)]);
            let _ = labels::detect(&a);
            let _ = labels::sibling_ref(&a);
            let _ = labels::analyze(&[&a, &b, &c]);
            let _ = labels::refers_to_siblings(&[(&a, false), (&b, true), (&c, false)]);
            let k = KeyInfo { texts: vec![b.clone()], wrong: vec![c.clone()], choices: true, tf: true, blank: true, letters: vec!['a'], positional: true, order_groups: vec![vec![b.clone()], vec![c.clone()]], ..Default::default() }.folded();
            let _ = format::stem_key(&a, &k);
            let k2 = KeyInfo { texts: vec![b.clone(), c.clone()], lines_only: true, ..Default::default() }.with_maps();
            let _ = format::stem_key(&a, &k2);
            let _ = format::left_names_right(&a.to_lowercase(), &b.to_lowercase());
            let _ = format::matching_names_left(&[(&a, &b), (&b, &c)]);
            let _ = format::holds_token(&a, &b);
            let _ = format::tf_of_part(&a, 'a', true);
        });
        assert!(res.is_ok(), "case {case}: {a:?} / {b:?} / {c:?}");
    }
}

#[test]
fn publish_never_panics() {
    let scale: u64 = std::env::var("FUZZ_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let mut r = Rng(0xD1B5_4A32_D192_ED03 ^ scale);
    for case in 0..8_000 * scale {
        let q = |r: &mut Rng, i: usize| {
            let mut q = json!({"id": format!("q{i:08}"), "stem": r.text(12), "kind": kinds(r), "points": 1.0, "difficulty": null, "topics": [r.text(3)], "created": 0, "modified": 0});
            if r.next().is_multiple_of(4) {
                q["keep_order"] = json!(true);
            }
            q
        };
        let body = if r.next().is_multiple_of(3) {
            json!({"schema_version": 2, "bank": {"id": "b00000001", "name": r.text(4), "created": 0, "modified": 0, "details": {"description": r.text(10)}, "questions": (0..3).map(|i| q(&mut r, i)).collect::<Vec<_>>()}}).to_string()
        } else {
            json!({"schema_version": 2, "question": q(&mut r, 0)}).to_string()
        };
        let opts = [r#"{"hide_key":true}"#, r#"{"hide_key":true,"ack_formatting":[0,1,2],"order_acks":[{"question":0},{"question":1},{"question":2}]}"#];
        let res = std::panic::catch_unwind(|| {
            // (Round 16, critic28b B8: what is served validates to itself, the full payload and
            // the stripped one.)
            for o in opts {
                let r: Value = serde_json::from_str(&publish_prepare_json(body.as_bytes(), o)).unwrap();
                if r["ok"] == true {
                    for (name, inner) in [("payload", &r["payload"]), ("stripped", &r["stripped"]["payload"])] {
                        if inner.is_null() {
                            continue;
                        }
                        let again: Value = serde_json::from_str(&validate_json(inner.to_string().as_bytes(), "")).unwrap();
                        assert_eq!(&again["payload"], inner, "{name} revalidates to itself: {body}");
                    }
                }
                let _ = validate_for_publish_json(body.as_bytes(), o);
                let _ = formatting_findings_json(body.as_bytes(), "");
                let _ = label_findings_json(body.as_bytes(), "");
                let _ = strip_json(body.as_bytes(), "");
            }
            // Every label finding decided both ways.
            let f: Value = serde_json::from_str(&label_findings_json(body.as_bytes(), "")).unwrap_or(Value::Null);
            if let Some(list) = f["label_findings"].as_array() {
                for choice in ["strip", "keep"] {
                    let labels: Vec<Value> = list.iter().map(|x| { let mut d = json!({"question": x["question"], "choice": choice}); if !x["part"].is_null() { d["part"] = x["part"].clone(); } d }).collect();
                    let _ = publish_prepare_json(body.as_bytes(), &json!({"hide_key": true, "labels": labels, "ack_formatting": [0, 1, 2]}).to_string());
                }
            }
        });
        assert!(res.is_ok(), "case {case}: {body}");
    }
}

#[test]
fn served_topics_are_a_fixed_point() {
    let q = json!({"id": "q00000001", "stem": "Which planet is red?", "kind": {"Mc": {"answers": [{"text": "Mars", "correct": true, "locked": false}, {"text": "Venus", "correct": false, "locked": false}]}}, "points": 1.0, "difficulty": null, "topics": ["Planets", "\u{200D} Planets", "\u{200C}Planets"], "created": 0, "modified": 0});
    let r: Value = serde_json::from_str(&publish_prepare_json(json!({"schema_version": 2, "question": q}).to_string().as_bytes(), r#"{"hide_key":true}"#)).unwrap();
    assert_eq!(r["stripped"]["payload"]["question"]["topics"], json!(["Planets"]), "{r}");
    let again: Value = serde_json::from_str(&validate_json(r["stripped"]["payload"].to_string().as_bytes(), "")).unwrap();
    assert_eq!(again["payload"], r["stripped"]["payload"]);
}

#[test]
fn critic24b_trap_inputs() {
    for t in ["1Answer", "100% correct", "1 True", "2 False", "(1) Wrong", "3 incorrect", "#1 Answer", "1 Answer", "2Correct", "A. 2.5Correct!\n"] {
        let _ = format::features(t);
    }
    for t in ["\u{130}stanbul est \u{E0} l\u{2019}ouest", "\u{130}zmir ist \u{E4}lter als Ankara", "Das \u{1E9E} ist der Gro\u{DF}buchstabe zu \u{DF}", "\u{130}stanbul es \u{FA}nica: est\u{E1} en dos continentes", "\u{130}stanbul is \u{FC}lke\u{2019}s capital"] {
        let _ = format::features(t);
        let p = json!({"schema_version": 2, "question": {"id": "q00000001", "stem": "Which is true?", "kind": {"Mc": {"answers": [{"text": t, "correct": true, "locked": false}, {"text": "Ankara est au centre", "correct": false, "locked": false}, {"text": "\u{130}zmir est au sud", "correct": false, "locked": false}]}}, "points": 1.0, "difficulty": null, "created": 0, "modified": 0}}).to_string();
        let _ = publish_prepare_json(p.as_bytes(), &format!(r#"{{"hide_key":true,"hash_key":"{}"}}"#, "ab".repeat(32)));
    }
    let p = json!({"schema_version": 2, "question": {"id": "q00000001", "stem": "Pick one.", "kind": {"Mc": {"answers": [{"text": "100% correct", "correct": true, "locked": false}, {"text": "50% correct", "correct": false, "locked": false}, {"text": "0% correct", "correct": false, "locked": false}]}}, "points": 1.0, "difficulty": null, "created": 0, "modified": 0}}).to_string();
    let _ = publish_prepare_json(p.as_bytes(), r#"{"hide_key":true}"#);
}
