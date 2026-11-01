//! Round 7: the outlier net and the closed list on what students are served, ambiguous cues
//! acknowledgeable, code and maths left alone, Matching pairing tokens and named lefts, hashes of
//! the author's and the served form, linear costs, the single-call `publish_prepare`.
mod common;
use serde_json::{Value, json};
use share_core::wasm::*;

fn j(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}
fn mc(texts: &[&str], correct: &[usize], locked: &[usize]) -> String {
    let a: Vec<Value> = texts.iter().enumerate().map(|(i, t)| json!({"text": t, "correct": correct.contains(&i), "locked": locked.contains(&i)})).collect();
    json!({"schema_version": 2, "question": {"id":"q1","stem":"Which?","kind":{"Mc":{"answers": a}},"points":1.0,"difficulty":null,"created":0,"modified":0}}).to_string()
}
fn matching(pairs: &[(&str, &str)], d: &[&str]) -> String {
    json!({"schema_version": 2, "question": {"id":"q","stem":"Match","kind":{"Matching":{"pairs": pairs.iter().map(|(l, r)| json!({"left": l, "right": r})).collect::<Vec<_>>(), "distractors": d}},"points":null,"difficulty":null,"created":0,"modified":0}}).to_string()
}

#[derive(Debug, PartialEq)]
enum Out {
    Served,
    Acked,
    Refused(Vec<String>),
}

/// An honest author: Strip where allowed, acks every frozen order, and acknowledges low-risk
/// formatting (as the app does after showing the preview).
fn publish(p: &str) -> Out {
    let f = j(&label_findings_json(p.as_bytes(), ""));
    assert_eq!(f["ok"], true, "{f}");
    let labels: Vec<Value> = f["label_findings"].as_array().unwrap().iter().map(|x| {
        let c = if x["strip_allowed"] == true { "strip" } else { "keep" };
        let mut d = json!({"question": x["question"], "choice": c});
        if !x["part"].is_null() { d["part"] = x["part"].clone(); }
        d
    }).collect();
    let mut o = json!({"labels": labels});
    let mut acked = false;
    for _ in 0..4 {
        let r = j(&strip_json(p.as_bytes(), &o.to_string()));
        if r["ok"] == true {
            return if acked { Out::Acked } else { Out::Served };
        }
        let e = r["errors"].as_array().unwrap();
        if e[0]["code"] == "order_ack_required" {
            o["order_acks"] = r["needs_order_ack"].clone();
            continue;
        }
        if !acked && e.iter().all(|x| x["ack_allowed"] == true) {
            o["ack_formatting"] = json!([0]);
            acked = true;
            continue;
        }
        return Out::Refused(e.iter().map(|x| format!("{}/{}", x["code"].as_str().unwrap(), x["reason"].as_str().unwrap_or(""))).collect());
    }
    Out::Refused(vec!["loop".into()])
}

// ---- the critics' corpora ----------------------------------------------------------------------

/// The Moodle critic's 463 honest questions, the correct choice at every position: no un-ackable
/// refusal, acknowledgements on at most 1% of the questions.
#[test]
fn clean_corpus_has_no_hard_refusals() {
    let items: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(common::fixtures_dir().join("../tests/data/clean_mc.json")).unwrap()).unwrap();
    let (mut refused, mut acked_items) = (vec![], 0);
    for it in &items {
        let ch: Vec<String> = it["choices"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
        let corr: Vec<usize> = it["correct"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect();
        let locked: Vec<usize> = it["locked"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect();
        let rots = if it["rotate"] == true { ch.len() } else { 1 };
        let mut acked = false;
        for k in 0..rots {
            let mut v = ch.clone();
            let mut c = corr.clone();
            if k > 0 {
                v.swap(corr[0], k);
                c = vec![k];
            }
            let refs: Vec<&str> = v.iter().map(String::as_str).collect();
            match publish(&mc(&refs, &c, &locked)) {
                Out::Served => {}
                Out::Acked => {
                    if !acked && std::env::var("SHOW_ACKS").is_ok() {
                        eprintln!("ACK {:?} correct {c:?}: {:?}", v, format_reasons(&refs, &c));
                    }
                    acked = true
                }
                Out::Refused(e) => refused.push(format!("{:?} correct {c:?}: {e:?}", v)),
            }
        }
        acked_items += usize::from(acked);
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
    assert!(acked_items * 100 <= items.len(), "{acked_items} of {} needed an acknowledgement", items.len());
}

/// The Moodle critic's 147 marker cases (the correct choice first, the wrong ones plain unless
/// given), published without any acknowledgement: none is served, except the control and one
/// stated limit.
#[test]
fn marker_cases_are_refused() {
    let lines = std::fs::read_to_string(common::fixtures_dir().join("../tests/data/marker_cases.txt")).unwrap();
    let mut accepted = vec![];
    for l in lines.lines().filter(|l| !l.is_empty()) {
        let (c, w): (String, Vec<String>) = if l.contains(" || ") {
            let mut it = l.split(" || ").map(String::from);
            let a = it.next().unwrap();
            (a, it.collect())
        } else {
            (l.to_string(), ["Mars", "Earth", "Jupiter"].map(String::from).to_vec())
        };
        let mut ch = vec![c.as_str()];
        ch.extend(w.iter().map(String::as_str));
        let p = mc(&ch, &[0], &[]);
        let f = j(&label_findings_json(p.as_bytes(), ""));
        let labels: Vec<Value> = f["label_findings"].as_array().unwrap().iter().map(|x| json!({"question": 0, "choice": if x["strip_allowed"] == true {"strip"} else {"keep"}})).collect();
        let mut o = json!({"labels": labels});
        let mut r = j(&strip_json(p.as_bytes(), &o.to_string()));
        if r["ok"] != true && r["errors"][0]["code"] == "order_ack_required" {
            o["order_acks"] = r["needs_order_ack"].clone();
            r = j(&strip_json(p.as_bytes(), &o.to_string()));
        }
        if r["ok"] == true {
            accepted.push(l.to_string());
        }
    }
    // "Venus" (the control: nothing marked). Round 11: "Venus (hottest planet)" is now a
    // low-risk outlier (refused unless acknowledged).
    assert_eq!(accepted, ["Venus"], "{accepted:#?}");
}

/// The ASVS critic's p4 markers: refused, or (invisible joiners, NBSP, en spaces) not served.
#[test]
fn asvs_markers_are_refused_or_not_served() {
    let bold = |s: &str| s.chars().map(|c| match c { 'A'..='Z' => char::from_u32(0x1D400 + c as u32 - 65).unwrap(), 'a'..='z' => char::from_u32(0x1D41A + c as u32 - 97).unwrap(), c => c }).collect::<String>();
    let comb = |s: &str, m: char| s.chars().flat_map(|c| [c, m]).collect::<String>();
    let base = ["Paris", "London", "Rome", "Berlin"];
    type Wrong = Option<fn(&str) -> String>;
    let cases: Vec<(String, Wrong)> = vec![
        (bold("Paris"), None), (comb("Paris", '\u{332}'), None), (comb("Paris", '\u{305}'), None), ("Paris \u{221A}".into(), None), ("\u{221A} Paris".into(), None),
        ("\u{25CB} Paris".into(), Some(|t| format!("\u{D7} {t}"))), ("\u{25CE} Paris".into(), None), ("O Paris".into(), Some(|t| format!("X {t}"))),
        ("Paris \u{25C0}".into(), None), ("Paris \u{25C4}".into(), None), ("Paris \u{261A}".into(), None), ("Paris \u{2665}".into(), None), ("Paris \u{2730}".into(), None),
        ("Paris \u{272A}".into(), None), ("Paris\u{2217}".into(), None), ("\u{FF0A}Paris".into(), None), ("Paris\u{204E}".into(), None), ("\\textbf{Paris}".into(), None),
        ("$\\mathbf{Paris}$".into(), None), ("[b]Paris[/b]".into(), None), ("==Paris==".into(), None), ("Paris".into(), Some(|t| format!("~~{t}~~"))),
        ("Paris".into(), Some(|t| t.chars().flat_map(|c| [c, '\u{336}']).collect())), ("P\u{3B1}ris".into(), None), ("\u{1D18}\u{1D00}\u{280}\u{26A}\u{A731}".into(), None),
        ("\u{24C5}\u{24D0}\u{24E1}\u{24D8}\u{24E2}".into(), None), ("Paris (c)".into(), None), ("Paris - yes".into(), None), ("Paris x".into(), None), ("X Paris".into(), None),
        ("Paris \u{24B8}".into(), None), ("Paris \u{1F132}".into(), None), ("Paris\u{1D43}".into(), None), ("Paris\u{207D}\u{1D9C}\u{207E}".into(), None),
    ];
    let mut accepted = vec![];
    for (c, w) in &cases {
        let texts: Vec<String> = base.iter().enumerate().map(|(i, t)| if i == 0 { c.clone() } else { w.map_or(t.to_string(), |f| f(t)) }).collect();
        let r: Vec<&str> = texts.iter().map(String::as_str).collect();
        let res = j(&strip_json(mc(&r, &[0], &[]).as_bytes(), ""));
        if res["ok"] == true || res["errors"][0]["code"] == "labels_need_decision" && j(&strip_json(mc(&r, &[0], &[]).as_bytes(), r#"{"labels":[{"question":0,"choice":"keep"}],"order_acks":[{"question":0}]}"#))["ok"] == true {
            accepted.push(c.clone());
        }
    }
    assert!(accepted.is_empty(), "{accepted:?}");
    // Not served: the invisible ones never reach students.
    for t in ["Paris\u{200D}", "Paris\u{200C}", "Paris\u{34F}", "Paris\u{A0}", "Par\u{2002}is"] {
        let s = share_core::format::served_text(t);
        assert!(s == "Paris" || s == "Par is", "{t:?} -> {s:?}");
    }
    // Joiners that a script needs stay (Persian, emoji sequences).
    assert_eq!(share_core::format::served_text("\u{645}\u{6CC}\u{200C}\u{62E}\u{648}\u{627}\u{647}\u{645}"), "\u{645}\u{6CC}\u{200C}\u{62E}\u{648}\u{627}\u{647}\u{645}");
    assert_eq!(share_core::format::served_text("\u{1F468}\u{200D}\u{1F469}"), "\u{1F468}\u{200D}\u{1F469}");
}

#[test]
fn marking_every_wrong_choice_is_refused() {
    for w in ["{} (wrong)", "{} (incorrect)", "{} - wrong", "{} (falsch)", "{} (no)", "{} \u{D7}", "{} X", "{} \u{2717}", "{} (x)", "{} 0", "{} (F)", "~~{}~~"] {
        let ws: Vec<String> = ["Mars", "Earth", "Jupiter"].iter().map(|m| w.replace("{}", m)).collect();
        let p = mc(&["Venus", &ws[0], &ws[1], &ws[2]], &[0], &[]);
        assert!(matches!(publish(&p), Out::Refused(_)), "{w}");
        // In a Multipart part too.
        let parts = json!({"schema_version": 2, "question": {"id":"q","stem":"s","kind":{"Multipart":{"parts":[{"stem":"p","kind":{"Mc":{"answers":[{"text":"Venus","correct":true},{"text":ws[0]},{"text":ws[1]},{"text":ws[2]}]}}}]}},"points":null,"difficulty":null,"created":0,"modified":0}}).to_string();
        assert!(matches!(publish(&parts), Out::Refused(_)), "part {w}");
    }
}

#[test]
fn code_and_maths_are_not_markers() {
    for set in [
        &["<b>", "<i>", "<u>", "<p>"][..], &["<em>", "<div>", "<span>", "<br>"], &["`code`", "**code**", "_code_", "> code"], &["# comment", "// comment", "/* comment */", "<!-- comment -->"],
        &["int *p;", "int &p;", "int p;", "pointer int p;"], &["*p", "&p", "p&", "p->"], &["*args", "args[]", "&args", "...args"], &["#menu", ".menu", "menu", "@menu"],
        &["(x) => x * 2", "function(x) -> x * 2", "x -> x * 2", "lambda x: x * 2"], &["_count", "2count", "count-1", "class"], &["List<String>", "List(String)", "List[String]", "String[]"],
        &["#include <stdio.h>", "import stdio", "using stdio;", "require 'stdio'"], &["__name__", "name", "_name", "Name"], &["key: value", "key = value", "key -> value", "key, value"],
        &["K_{a}", "K_{b}", "K_{w}", "K_{sp}"], &["v_{0}", "v_{f}", "a", "t"], &["V = IR", "V = I/R", "V = R/I", "V = I + R"], &["PV = nRT", "PV = RT/n", "P = nRTV", "PV = nR/T"],
    ] {
        for k in 0..set.len() {
            let o = publish(&mc(set, &[k], &[]));
            assert!(!matches!(o, Out::Refused(_)), "{set:?} correct {k}: {o:?}");
        }
    }
    // and are not frozen as references
    use share_core::labels::refers_to_siblings;
    for set in [&["V = IR", "V = I/R", "V = R/I", "V = I + R"][..], &["x\u{B2} + C", "2x\u{B2} + C", "x + C", "2 + C"], &["log a + log b", "log a \u{B7} log b", "log a - log b", "(log a)/(log b)"],
        &["C\u{2261}C", "C=C", "C-C", "C\u{2026}H"], &["Me and him went.", "He and I went.", "Him and I went.", "Me and he went."], &["'cba'", "'abc'", "'a'", "'c'"], &["V_{max}", "K_{m}", "k_{cat}", "E"],
        &["I came; I saw.", "I came; and I saw.", "I; came I saw.", "I came I; saw."], &["\u{3042}\u{308A}\u{304C}\u{3068}\u{3046}\u{3054}\u{3056}\u{3044}\u{307E}\u{3059}", "\u{3042}\u{308A}\u{304C}\u{3068}\u{3046}", "\u{3069}\u{3046}\u{3082}", "\u{30B5}\u{30F3}\u{30AD}\u{30E5}\u{30FC}"]] {
        assert!(!refers_to_siblings(&set.iter().map(|t| (*t, false)).collect::<Vec<_>>()), "{set:?}");
    }
}

#[test]
fn matching_pairing_tokens_and_named_lefts() {
    let refused = |p: &str| matches!(publish(p), Out::Refused(_));
    for pairs in [
        vec![("France\u{B9}", "Paris\u{B9}"), ("Spain\u{B2}", "Madrid\u{B2}"), ("Italy\u{B3}", "Rome\u{B3}")], vec![("France\u{2460}", "Paris\u{2460}"), ("Spain\u{2461}", "Madrid\u{2461}"), ("Italy\u{2462}", "Rome\u{2462}")],
        vec![("France-1", "Paris-1"), ("Spain-2", "Madrid-2"), ("Italy-3", "Rome-3")],
        vec![("\u{1F534} France", "\u{1F534} Paris"), ("\u{1F535} Italy", "\u{1F535} Rome"), ("\u{1F7E2} Spain", "\u{1F7E2} Madrid")],
    ] {
        assert!(refused(&matching(&pairs, &["Berlin"])), "{pairs:?}");
    }
    // Round 13: a right that holds its own left is low risk (acknowledged): "Cotton" / "Seed hairs
    // of the cotton plant" is content as often as "France" / "Paris (France)" is a cue.
    for pairs in [vec![("France", "Paris (France)"), ("Italy", "Rome (Italy)"), ("Spain", "Madrid (Spain)")], vec![("Capital of France", "Paris, capital of France"), ("Capital of Italy", "Rome, capital of Italy"), ("Capital of Spain", "Madrid, capital of Spain")]] {
        assert_eq!(publish(&matching(&pairs, &["Berlin"])), Out::Acked, "{pairs:?}");
    }
    assert!(refused(&matching(&[("France", "Paris \u{221A}"), ("Spain", "Madrid \u{221A}"), ("Italy", "Rome \u{221A}")], &["Berlin", "Lisbon"])));
    assert!(refused(&matching(&[("France", "Paris"), ("Italy", "Rome"), ("Spain", "Madrid")], &["Lisbon \u{D7}", "Berlin \u{D7}"])));
    // Maths shares numbers by design: acknowledgeable, not refused; plain sets are served.
    assert_eq!(publish(&matching(&[("y = 2x + 5", "5"), ("y = -x + 3", "3"), ("y = 4x - 7", "-7")], &["0"])), Out::Acked);
    for (pairs, d) in [
        (vec![("Sodium", "Na"), ("Potassium", "K"), ("Iron", "Fe")], vec!["Ag"]), (vec![("Mg", "Mg\u{B2}\u{207A}"), ("Na", "Na\u{207A}"), ("Cl", "Cl\u{207B}")], vec!["Al\u{B3}\u{207A}"]),
        (vec![("End of WWII", "1945"), ("Moon landing", "1969")], vec!["1914"]), (vec![("L1", "R1"), ("L2", "R2"), ("L3", "R3")], vec![]),
        (vec![("der Hund", "the dog"), ("die Katze", "the cat")], vec!["the mouse"]), (vec![("Spreadsheet cell A1", "column A, row 1"), ("B3", "column B, row 3")], vec![]),
    ] {
        let o = publish(&matching(&pairs, &d));
        assert_eq!(o, Out::Served, "{pairs:?}");
    }
}

#[test]
fn locked_fifty_fifty_is_refused() {
    let codes = |p: &str| j(&strip_json(p.as_bytes(), r#"{"order_acks":[{"question":0}]}"#))["errors"].as_array().map(|e| e.iter().map(|x| x["code"].as_str().unwrap().to_string()).collect::<Vec<_>>()).unwrap_or_default();
    for (texts, locked) in [(&["Venus", "Mars", "Earth", "I don't know"][..], &[0, 3][..]), (&["Venus", "Mars", "Earth", "Other", "None of these"], &[0, 3, 4]), (&["Venus", "Mars", "Earth", "Jupiter"], &[0, 1])] {
        assert!(codes(&mc(texts, &[0], locked)).contains(&"locked_key".to_string()), "{texts:?} {locked:?}");
    }
}

// ---- hashes -------------------------------------------------------------------------------------

/// Property: whatever the hidden characters, match_hash(full) == match_hash_with(full, {}) ==
/// strip's match_hashes, for unlabelled questions.
#[test]
fn author_and_served_hashes_agree() {
    let atoms = ["Wash", "ing", "ton", "\u{AD}", "\u{200B}", "\u{2060}", "\u{200D}", "\u{200C}", "\u{34F}", "\u{A0}", "\u{2002}", " ", "\t", "e\u{301}", "\u{E9}", "x"];
    let mut seed = 0x9E3779B97F4A7C15u64;
    let mut next = |k: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % k as u64) as usize
    };
    let mut checked = 0;
    for it in 0..2000 {
        let texts: Vec<String> = (0..3).map(|i| format!("w{i}{}", (0..1 + next(5)).map(|_| atoms[next(atoms.len())]).collect::<String>())).collect();
        let r: Vec<&str> = texts.iter().map(String::as_str).collect();
        let p = mc(&r, &[next(3)], &[]);
        let Ok(v) = share_core::validate(&p, &share_core::Limits::default(), &share_core::Structural) else { continue };
        let q = &v.payload.questions()[0];
        let s = j(&strip_json(p.as_bytes(), r#"{"order_acks":[{"question":0}]}"#));
        if s["ok"] != true {
            continue;
        }
        let full = share_core::match_hash(q);
        assert_eq!(full, share_core::match_hash_with(q, &Default::default()), "{it}: {texts:?}");
        assert_eq!(json!(full), s["match_hashes"][0], "{it}: {texts:?}");
        checked += 1;
    }
    assert!(checked > 1500, "{checked}");
}

// ---- API ----------------------------------------------------------------------------------------

#[test]
fn hash_keys_are_parsed_strictly() {
    let p = common::fixture_payload("question_tf");
    let h = |k: &str| j(&full_hash_json(p.as_bytes(), &json!({"hash_key": k}).to_string()));
    assert_eq!(h(&"0f".repeat(32))["ok"], true);
    assert_eq!(h(&"0f".repeat(32))["hashes"], h(&"0F".repeat(32))["hashes"], "either case");
    for bad in ["+f".repeat(32), "0f".repeat(31), "00".repeat(32), "\u{20AC}".repeat(22), "0f".repeat(32) + "0", "g0".repeat(32)] {
        assert_eq!(h(&bad)["errors"][0]["code"], "hash_key_required", "{bad:?}");
    }
}

#[test]
fn publish_prepare_is_the_whole_publish_in_one_call() {
    let p = mc(&["Venus", "Mars", "Earth", "Jupiter"], &[0], &[]);
    let key = "ab".repeat(32);
    let r = j(&publish_prepare_json(p.as_bytes(), &json!({"hide_key": true, "hash_key": key}).to_string()));
    assert_eq!(r["ok"], true, "{r}");
    let v = j(&validate_for_publish_json(p.as_bytes(), r#"{"hide_key":true}"#));
    assert_eq!(r["payload"], v["payload"]);
    assert_eq!(r["match_hashes"], j(&match_hash_json(p.as_bytes(), ""))["hashes"]);
    assert_eq!(r["full_hashes"], j(&full_hash_json(p.as_bytes(), &json!({"hash_key": key}).to_string()))["hashes"]);
    let st = &r["stripped"];
    assert_eq!(st["ok"], true);
    assert_eq!(st["payload"]["question"]["keyless"], true);
    assert_eq!(st["match_hashes"], r["match_hashes"], "the served form hashes as the full one");
    assert_eq!(j(&strip_author_data_json())["served_orders"][0]["order"].as_array().unwrap().len(), 4);
    // Without hide_key: the validated form only; refusals as validate_for_publish / strip.
    let r = j(&publish_prepare_json(p.as_bytes(), ""));
    assert!(r.get("stripped").is_none() && r["ok"] == true);
    let bad = mc(&["Venus \u{2713}", "Mars", "Earth"], &[0], &[]);
    assert_eq!(j(&publish_prepare_json(bad.as_bytes(), r#"{"hide_key":true}"#))["errors"][0]["code"], "key_in_formatting");
    let frozen = mc(&["Venus", "Mars", "Both A and B"], &[0], &[]);
    let r = j(&publish_prepare_json(frozen.as_bytes(), r#"{"hide_key":true}"#));
    assert_eq!(r["errors"][0]["code"], "order_ack_required");
    assert_eq!(j(&strip_author_data_json())["errors"][0]["code"], "no_author_data");
    // A placeholder error is the author's.
    let tf = json!({"schema_version":2,"question":{"id":"q","stem":"s","kind":{"Tf":{"correct":false}},"points":null,"difficulty":null,"created":0,"modified":0,"origin":{"item_id":"i","qid":"q","publish_id":"p","version":1,"author_name":"a","license":"CC-BY-4.0","form":"stripped"}}}).to_string();
    let e = j(&validate_for_publish_json(tf.as_bytes(), ""));
    assert_eq!((e["errors"][0]["code"].as_str(), e["errors"][0]["author_only"].as_bool()), (Some("placeholder_key"), Some(true)));
}

#[test]
fn a_failed_strip_clears_author_data() {
    let p = mc(&["Venus", "Mars", "Earth", "Jupiter"], &[0], &[]);
    assert_eq!(j(&strip_json(p.as_bytes(), ""))["ok"], true);
    let _ = strip_json(p.as_bytes(), "{bad");
    assert_eq!(j(&strip_author_data_json())["errors"][0]["code"], "no_author_data");
    assert_eq!(j(&strip_json(p.as_bytes(), ""))["ok"], true);
    clear_author_data();
    assert_eq!(j(&strip_author_data_json())["errors"][0]["code"], "no_author_data");
}

// ---- costs --------------------------------------------------------------------------------------

/// The critics' adversarial shapes through `publish_prepare` (the Worker's one call): bounded,
/// and doubling a shape's size costs at most ~2.6x.
#[test]
fn adversarial_shapes_are_cheap() {
    let fill = |make: &dyn Fn(usize) -> String| {
        let mut qs = vec![];
        let mut len = 120;
        for i in 0..1000 {
            let s = make(i);
            if len + s.len() + 1 > 4 * 1048576 - 300 { break; }
            len += s.len() + 1;
            qs.push(s);
        }
        format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, qs.join(","))
    };
    let qh = |i: usize, kind: String| format!(r#"{{"id":"q{i}","stem":"s","kind":{kind},"points":null,"difficulty":null,"created":1,"modified":1}}"#);
    let az = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let bang = |len: usize| fill(&|i| qh(i, format!(r#"{{"Mc":{{"answers":[{}]}}}}"#, (0..26).map(|k| format!(r#"{{"text":{},"correct":{}}}"#, json!(format!("{}. w{}{}", &az[k..k + 1], &az[k..k + 1].to_lowercase(), "!".repeat(len))), k == 0)).collect::<Vec<_>>().join(","))));
    let tokens = |len: usize| fill(&|i| qh(i, format!(r#"{{"Matching":{{"pairs":[{}],"distractors":[]}}}}"#, (0..50).map(|k| format!(r#"{{"left":{},"right":{}}}"#, json!(format!("{}L{k}", "0 ".repeat(len))), json!(format!("{}R{k}", "1 ".repeat(len))))).collect::<Vec<_>>().join(","))));
    let boxes = |len: usize| fill(&|i| qh(i, format!(r#"{{"Mc":{{"answers":[{}]}}}}"#, (0..26).map(|k| format!(r#"{{"text":{},"correct":{}}}"#, json!(format!("w{k} {}", (0..len).map(|n| format!("({n:03})")).collect::<Vec<_>>().join(" "))), k == 0)).collect::<Vec<_>>().join(","))));
    let bound = if cfg!(debug_assertions) || std::env::var("SHARE_CORE_MUTATION").is_ok() { 120.0 } else { 2.5 };
    for (name, small, big) in [("bang", bang(1018), bang(2036)), ("tokens", tokens(510), tokens(1020)), ("boxes", boxes(170), boxes(340))] {
        let opts = r#"{"hide_key":true}"#;
        let t = |p: &str| {
            let s = std::time::Instant::now();
            let _ = publish_prepare_json(p.as_bytes(), opts);
            s.elapsed().as_secs_f64()
        };
        let (a, b) = (t(&small), t(&big));
        assert!(b < bound, "{name}: {b:.2} s");
        let _ = a;
    }
}

/// The closed list names what it found (the outlier net is the fallback), and high risk survives
/// an acknowledgement.
#[test]
fn closed_list_reasons_and_risks() {
    let reasons = |texts: &[&str]| -> Vec<(String, bool)> {
        let f = j(&formatting_findings_json(mc(texts, &[0], &[]).as_bytes(), ""));
        f["formatting_findings"].as_array().unwrap().iter().map(|x| (x["reason"].as_str().unwrap().to_string(), x["ack_allowed"] == true)).collect()
    };
    let has = |r: &[(String, bool)], name: &str, ack: bool| r.iter().any(|(n, a)| n == name && *a == ack);
    let r = reasons(&["\u{24C5}\u{24D0}\u{24E1}\u{24D8}\u{24E2}", "London", "Rome", "Berlin"]);
    assert!(has(&r, "enclosed", true), "{r:?}");
    let strike = |t: &str| t.chars().flat_map(|c| [c, '\u{336}']).collect::<String>();
    let (a, b, c) = (strike("London"), strike("Rome"), strike("Berlin"));
    let r = reasons(&["Paris", &a, &b, &c]);
    assert!(has(&r, "line_mark", false), "{r:?}");
    let u: String = "Paris".chars().flat_map(|c| [c, '\u{332}']).collect();
    let r = reasons(&[&u, "London", "Rome", "Berlin"]);
    assert!(has(&r, "line_mark", false), "{r:?}");
    // An acknowledgement does not let high risk through.
    let p = mc(&[&u, "London", "Rome", "Berlin"], &[0], &[]);
    assert_eq!(j(&strip_json(p.as_bytes(), r#"{"ack_formatting":[0]}"#))["ok"], false);
}

#[test]
fn shared_scores_in_maths_and_too_many_features() {
    // A "1" every wrong choice ends with is maths, not a "wrong" mark: served without a word.
    let p = mc(&["f\u{207B}\u{B9}(x) = (x - 1)/2", "f\u{207B}\u{B9}(x) = 2x - 1", "f\u{207B}\u{B9}(x) = x/2 + 1", "f\u{207B}\u{B9}(x) = 1/(2x + 1)"], &[0], &[]);
    assert_eq!(publish(&p), Out::Served);
    // More distinct marker features than the cap: refused as too complex, whatever the author says.
    let many: String = (0..40).map(|n| format!("({n:02})")).collect::<Vec<_>>().join(" ");
    let t = format!("w {many}");
    let r = j(&strip_json(mc(&[&t, "Mars", "Earth"], &[0], &[]).as_bytes(), r#"{"ack_formatting":[0]}"#));
    assert!(r["errors"].as_array().unwrap().iter().any(|e| e["reason"] == "too_complex"), "{r}");
}

#[test]
fn more_non_references_and_tagged_distractors() {
    use share_core::labels::sibling_ref;
    for t in ["A = B and C", "Vitamin C and iron", "Iron and vitamin C"] {
        assert_eq!(sibling_ref(t), None, "{t}");
    }
    // Distractors tagged "(extra)" while the paired items are not: they name the unused rights,
    // refused (round 15, critic28a; acknowledging it published the leak).
    let p = matching(&[("France", "Paris"), ("Italy", "Rome"), ("Spain", "Madrid")], &["Lisbon (extra)", "Berlin (extra)"]);
    assert!(matches!(publish(&p), Out::Refused(_)));
}

fn format_reasons(v: &[&str], c: &[usize]) -> Vec<String> {
    let ch: Vec<(&str, bool)> = v.iter().enumerate().map(|(i, t)| (*t, c.contains(&i))).collect();
    share_core::format::check(&ch).iter().map(|f| format!("{}:{}{}", f.index, f.reason, if f.risk == share_core::format::Risk::High { "!" } else { "" })).collect()
}
