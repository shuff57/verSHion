//! Round 6: formatting judged on what students are served, the closed set of marker types, false
//! refusals, the author-only data, keyed full_hash, locked leaks, sibling references in ~25
//! languages, linear costs.
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

/// Publishes as a permissive author: `choice` for every label finding ("auto": Strip where
/// allowed), every frozen order acknowledged. Ok(served texts) or Err(error codes).
fn publish(p: &str, choice: &str, extra: Value) -> Result<Vec<String>, Vec<String>> {
    let f = j(&label_findings_json(p.as_bytes(), ""));
    assert_eq!(f["ok"], true, "{f}");
    let labels: Vec<Value> = f["label_findings"].as_array().unwrap().iter().map(|x| {
        let c = match choice { "auto" => if x["strip_allowed"] == true { "strip" } else { "keep" }, c => c };
        let mut d = json!({"question": x["question"], "choice": c});
        if !x["part"].is_null() { d["part"] = x["part"].clone(); }
        d
    }).collect();
    let mut o = json!({"labels": labels});
    for (k, v) in extra.as_object().unwrap() { o[k] = v.clone(); }
    let mut r = j(&strip_json(p.as_bytes(), &o.to_string()));
    if r["ok"] != true && r["errors"][0]["code"] == "order_ack_required" {
        o["order_acks"] = r["needs_order_ack"].clone();
        r = j(&strip_json(p.as_bytes(), &o.to_string()));
    }
    if r["ok"] != true {
        return Err(r["errors"].as_array().unwrap().iter().map(|e| e["code"].as_str().unwrap().to_string()).collect());
    }
    Ok(r["payload"]["question"]["kind"]["Mc"]["answers"].as_array().map(|a| a.iter().map(|x| x["text"].as_str().unwrap().to_string()).collect()).unwrap_or_default())
}

// ---- N1: the served form is what counts ---------------------------------------------------------

#[test]
fn decoration_behind_a_label_is_refused_under_strip_and_keep() {
    for ch in [
        ["A. *Venus", "B. Mars", "C. Earth", "D. Jupiter"],
        ["A. \u{2713} Venus", "B. Mars", "C. Earth", "D. Jupiter"],
        ["a) [x] Venus", "b) [ ] Mars", "c) [ ] Earth", "d) [ ] Jupiter"],
        ["A. =Venus", "B. ~Mars", "C. ~Earth", "D. ~Jupiter"],
        ["A. VENUS", "B. Mars", "C. Earth", "D. Jupiter"],
        ["A. \u{2714} Venus", "B. Mars", "C. Earth", "D. Jupiter"],
        ["a) **Venus**", "b) Mars", "c) Earth", "d) Jupiter"],
        ["\u{24B6} **Venus**", "\u{24B7} Mars", "\u{24B8} Earth", "\u{24B9} Jupiter"],
        ["Venus (A) \u{2714}", "Mars (B)", "Earth (C)", "Jupiter (D)"],
    ] {
        for choice in ["keep", "strip"] {
            let r = publish(&mc(&ch, &[0], &[]), choice, json!({}));
            assert!(r.as_ref().is_err_and(|e| e.iter().any(|c| c == "key_in_formatting")), "{ch:?} {choice}: {r:?}");
        }
    }
    // The label itself is fine, and the spacing after it is not served.
    assert_eq!(publish(&mc(&["A.  Venus", "B. Mars", "C. Earth"], &[0], &[]), "keep", json!({})).unwrap(), ["A. Venus", "B. Mars", "C. Earth"]);
}

/// Differential fuzzer: full label sets in every style, a marker on exactly the correct choice,
/// every decision. Zero false accepts.
#[test]
fn differential_marker_fuzzer_has_no_false_accepts() {
    let styles: [fn(usize) -> String; 6] = [
        |i| format!("{}. ", (b'A' + i as u8) as char),
        |i| format!("{}) ", (b'a' + i as u8) as char),
        |i| format!("{}. ", i + 1),
        |i| format!("({}) ", (b'a' + i as u8) as char),
        |i| format!("{}. ", ["i", "ii", "iii", "iv", "v"][i]),
        |i| format!("Option {}: ", (b'A' + i as u8) as char),
    ];
    let marks: [fn(&str) -> String; 24] = [
        |w| format!("*{w}"), |w| format!("{w}*"), |w| format!("**{w}**"), |w| format!("_{w}_"), |w| format!("`{w}`"),
        |w| format!("\u{2713} {w}"), |w| format!("{w} \u{2714}\u{FE0F}"), |w| format!("\u{2705} {w}"), |w| format!("{w} \u{2B50}"),
        |w| format!("{w} \u{1F44D}"), |w| format!("{w} (correct)"), |w| format!("{w} [answer]"), |w| format!("{w} - correct"),
        |w| format!("{w}: correct"), |w| format!("{w} <--"), |w| format!("-> {w}"), |w| format!("{w} \u{2190}"), |w| format!("={w}"),
        |w| format!("{} (*)", w), |w| format!("{w} correct"), |w| format!("<b>{w}</b>"), |w| format!("{w} #"), |w| w.to_uppercase(),
        |w| format!("{w} (\u{2713})"),
    ];
    let words = ["Venus", "Saturn", "Mercury", "Neptune", "Jupiter"];
    let mut cases = 0;
    let mut accepted = vec![];
    for st in &styles {
        for (mi, mk) in marks.iter().enumerate() {
            for n in [3usize, 4, 5] {
                for correct in [0, n - 1] {
                    let texts: Vec<String> = (0..n).map(|i| {
                        let body = if i == correct { mk(words[i]) } else { words[i].to_string() };
                        format!("{}{}", st(i), body)
                    }).collect();
                    let r: Vec<&str> = texts.iter().map(String::as_str).collect();
                    for choice in ["keep", "strip", "auto"] {
                        cases += 1;
                        if let Ok(served) = publish(&mc(&r, &[correct], &[]), choice, json!({})) {
                            accepted.push(format!("mark {mi} {choice}: {texts:?} -> {served:?}"));
                        }
                    }
                }
            }
        }
    }
    assert!(accepted.is_empty(), "{} of {cases} accepted:\n{}", accepted.len(), accepted.join("\n"));
}

/// Property: whatever strip serves, the same detector over the served choices (correct flags
/// carried through the author-only served order) finds nothing.
#[test]
fn served_forms_never_isolate_the_correct_choice() {
    let atoms = ["Venus", "Mars", "Earth", "A. ", "B) ", "(c) ", "1. ", "x", "2", "-", "+", "*", "(z)", " - ", "correct", "Na+", "\u{2713}", "..", "?", "**", "\u{1F600}", "[x]", "~", "=", "VENUS", "e\u{301}", " "];
    let mut seed = 0x2545F4914F6CDD1Du64;
    let mut next = |k: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % k as u64) as usize
    };
    let mut published = 0;
    for _ in 0..3000 {
        let n = 2 + next(4);
        let texts: Vec<String> = (0..n).map(|i| (0..1 + next(3)).map(|_| atoms[next(atoms.len())]).collect::<String>() + &format!(" w{i}")).collect();
        let c = next(n);
        let r: Vec<&str> = texts.iter().map(String::as_str).collect();
        let p = mc(&r, &[c], &[]);
        for choice in ["keep", "auto"] {
            let Ok(served) = publish(&p, choice, json!({})) else { continue };
            published += 1;
            let ad = j(&strip_author_data_json());
            let order: Vec<usize> = ad["served_orders"].get(0).map(|o| o["order"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect()).unwrap_or((0..n).collect());
            let ch: Vec<(&str, bool)> = served.iter().zip(&order).map(|(t, &k)| (t.as_str(), k == c)).collect();
            let flags = share_core::format::check(&ch);
            assert!(flags.iter().all(|f| f.risk == share_core::format::Risk::Low && false), "{texts:?} served {served:?}: {flags:?}");
        }
    }
    assert!(published > 500, "{published}");
}

// ---- N2 / N5: marker types, content that is not a marker, acknowledgement ------------------------

#[test]
fn accidental_markers_are_refused() {
    for ch in [
        ["Venus [x]", "Mars [ ]", "Earth [ ]", "Jupiter [ ]"], ["Venus (x)", "Mars ( )", "Earth ( )", "Jupiter ( )"],
        ["Venus (\u{2713})", "Mars (\u{2717})", "Earth (\u{2717})", "Jupiter (\u{2717})"], ["Venus (T)", "Mars (F)", "Earth (F)", "Jupiter (F)"],
        ["Venus (1)", "Mars (0)", "Earth (0)", "Jupiter (0)"], ["Venus (*)", "Mars (red)", "Earth (blue)", "Jupiter (big)"],
        ["=4", "~3", "~5", "~6"], ["=a dog", "~a cat", "~a cow", "~a pig"], ["Venus correct", "Mars", "Earth", "Jupiter"],
        ["Venus <- correct", "Mars", "Earth", "Jupiter"], ["Venus: correct", "Mars", "Earth", "Jupiter"], ["Venus = correct", "Mars", "Earth", "Jupiter"],
        ["Venus -- correct", "Mars", "Earth", "Jupiter"], ["Venus \u{2014} the answer", "Mars", "Earth", "Jupiter"], ["Venus <== answer", "Mars", "Earth", "Jupiter"],
        ["The answer is **Venus** here", "The answer is Mars here", "The answer is Earth here", "The answer is Jupiter here"],
        ["Venus \u{2705} planet", "Mars planet", "Earth planet", "Jupiter planet"], ["<b>Venus</b>", "<i>Mars</i>", "<i>Earth</i>", "<i>Jupiter</i>"],
        ["Venus \u{2714}\u{FE0F}", "Mars \u{2714}\u{FE0E}", "Earth \u{2714}\u{FE0E}", "Jupiter \u{2714}\u{FE0E}"], ["V\u{435}nus", "Mars", "Earth", "Jupiter"],
        ["Answer: Venus", "Mars", "Earth", "Jupiter"], ["Venus(correct)", "Mars", "Earth", "Jupiter"], ["Venus (planet) (correct)", "Mars (planet)", "Earth (planet)", "Jupiter (planet)"],
        ["1 Venus", "0 Mars", "0 Earth", "0 Jupiter"], ["+Venus", "-Mars", "-Earth", "-Jupiter"], ["Venus #", "Mars", "Earth", "Jupiter"],
        ["Venus\u{20DD}", "Mars", "Earth", "Jupiter"], ["\u{2248}4", "3", "5", "6"], ["Venus -- right", "Mars", "Earth", "Jupiter"],
        ["Venus: key", "Mars", "Earth", "Jupiter"], ["Venus (yes)", "Mars", "Earth", "Jupiter"], ["Venus (this one)", "Mars", "Earth", "Jupiter"],
    ] {
        let r = publish(&mc(&ch, &[0], &[]), "auto", json!({}));
        assert!(r.is_err(), "{ch:?}: {r:?}");
    }
    // Several correct choices sharing a mark.
    assert!(publish(&mc(&["Venus*", "Mars*", "Earth", "Jupiter"], &[0, 1], &[]), "auto", json!({})).is_err());
}

/// The critics' content corpus (ions, maths, programming, punctuation, acronyms, CJK): published.
#[test]
fn content_is_not_a_marker() {
    let sets: &[&[&str]] = &[
        &["(a + b)\u{B2}", "a\u{B2} + b\u{B2}", "a\u{B2} + 2ab", "2ab"], &["\u{221E}", "0", "1", "-1"], &["-\u{221E}", "0", "1", "\u{221E}"], &["\u{2205}", "{1}", "{2}", "{1, 2}"],
        &["|x|", "x", "-x", "x\u{B2}"], &["n!", "n", "2n", "n\u{B2}"], &["len()", "size", "count", "length"], &["a[0]", "a.first", "head a", "car a"],
        &["[Cu(NH\u{2083})\u{2084}]\u{B2}\u{207A}", "CuSO\u{2084}", "CuCl\u{2082}", "Cu\u{2082}O"], &["\u{201C}Hamlet\u{201D}", "Macbeth", "Othello", "Lear"], &["Wait...", "Stop", "Go", "Run"],
        &["Why?", "Because", "So", "Then"], &["90\u{B0}", "45", "30", "60"], &["1:2", "2", "3", "4"], &["50 %", "50", "25", "75"], &["\u{B1}2", "2", "-2", "0"],
        &["\u{2192}", "\u{2190}", "\u{2191}", "\u{2193}"], &["C\u{266F}", "C", "D", "E"], &["F#", "F", "G", "A"], &["C++", "Java", "Python", "Rust"], &["C#", "Java", "Python", "Rust"],
        &[".NET", "Java", "Python", "Rust"], &["[0, 1]", "(0, 1)", "(0, 1]", "[0, 1)"], &["9.8 (m/s\u{B2})", "9.8", "10", "8"], &["Mitosis (cell division)", "Meiosis", "Osmosis", "Diffusion"],
        &["NASA", "Esa", "Jaxa", "Isro"], &["The correct one", "The wrong one", "Another one", "None one"], &["'a'", "a", "b", "c"], &["-x", "x", "2x", "x/2"],
        &["-ing", "ed", "s", "ly"], &["#1", "1", "2", "3"], &["&", "and", "or", "but"], &["\u{1F600}", "smile", "frown", "cry"], &["Na\u{207A}", "Na", "Cl", "K"],
        &["Venus", "Mars.", "Earth.", "Jupiter."], &["Na+", "K+", "Cl-", "Ca2+"], &["ATP", "NADPH", "Glucose", "Oxygen"], &["NAFTA", "GATT", "the WTO", "the EU"],
        &["Where are you going?", "Close the door.", "What a day!", "I am going home."], &["!", "?", ".", ","], &["\u{BF}D\u{F3}nde est\u{E1} el ba\u{F1}o?", "Est\u{E1} aqu\u{ED}.", "Vamos.", "Hace fr\u{ED}o."],
        &["Temperature (in \u{B0}C or \u{B0}F)", "Pressure", "Humidity", "Mass"], &["pascal (Pa)", "newton (N)", "joule (J)", "watt (W)"], &["(-\u{221E}, 0)", "(0, \u{221E})", "(-\u{221E}, \u{221E})", "[0, 1]"],
        &["DNA\u{30DD}\u{30EA}\u{30E1}\u{30E9}\u{30FC}\u{30BC}", "\u{30D8}\u{30EA}\u{30AB}\u{30FC}\u{30BC}", "\u{30EA}\u{30AC}\u{30FC}\u{30BC}"], &["Iron (II) oxide", "Iron (III) oxide", "Iron (IV) oxide"],
        &["5 \u{3A9}", "5 V", "5 A"], &["MARS", "Venus", "Earth", "Pluto"], &["1 apple", "2 apples", "3 apples"], &["x > 2", "x < 2", "x = 2"], &["Turn right", "Turn left", "Go back"],
    ];
    // Round 7: nothing is refused outright; the ambiguous ones need the author's acknowledgement.
    let mut refused = vec![];
    let mut acked = 0;
    for set in sets {
        for k in 0..set.len() {
            let p = mc(set, &[k], &[]);
            if publish(&p, "auto", json!({})).is_err() {
                acked += 1;
                if let Err(e) = publish(&p, "auto", json!({"ack_formatting": [0]})) {
                    refused.push(format!("{set:?} correct {k}: {e:?}"));
                }
            }
        }
    }
    assert!(acked <= 12, "{acked} needed an acknowledgement");
    assert!(refused.is_empty(), "{}", refused.join("\n"));
    // A mark on the one wrong choice of two is allowed (the absence rule needs two wrong choices).
    assert!(publish(&mc(&["Venus", "Mars*"], &[0], &[]), "auto", json!({})).is_ok());
    // Stated limits: an all-capital word of 5+ letters among mixed-case words is refused (low risk,
    // the author can acknowledge it); so is a mapped acronym like UNESCO there.
    let p = mc(&["UNESCO", "Unicef", "Who", "Nato"], &[0], &[]);
    assert!(publish(&p, "auto", json!({})).is_err());
    assert!(publish(&p, "auto", json!({"ack_formatting": [0]})).is_ok(), "low risk: acknowledged");
    // High risk cannot be acknowledged.
    assert!(publish(&mc(&["**Venus**", "Mars", "Earth"], &[0], &[]), "auto", json!({"ack_formatting": [0]})).is_err());
}

#[test]
fn findings_preview_and_reasons() {
    let p = json!({"schema_version": 2, "bank": {"id":"b","name":"n","created":0,"modified":0,"questions": [
        {"id":"q0","stem":"s","kind":{"Mc":{"answers":[{"text":"VENUS","correct":true},{"text":"Mars"},{"text":"Earth"}]}},"points":null,"difficulty":null,"created":0,"modified":0},
        {"id":"q1","stem":"s","kind":{"Mc":{"answers":[{"text":"Venus \u{2713}","correct":true},{"text":"Mars"},{"text":"Earth"}]}},"points":null,"difficulty":null,"created":0,"modified":0},
        {"id":"q2","stem":"s","kind":{"Mc":{"answers":[{"text":"Venus","correct":true},{"text":"Mars"},{"text":"Earth"},{"text":"Both A and B"}]}},"points":null,"difficulty":null,"created":0,"modified":0}
    ]}}).to_string();
    let f = j(&formatting_findings_json(p.as_bytes(), ""));
    let ff = f["formatting_findings"].as_array().unwrap();
    let find = |q: u64, r: &str| ff.iter().find(|x| x["question"] == q && x["reason"] == r).cloned();
    let c = find(0, "capitals").unwrap_or_else(|| panic!("{f}"));
    assert_eq!((c["ack_allowed"].as_bool(), c["served"].as_str()), (Some(true), Some("VENUS")));
    assert_eq!(find(1, "check").unwrap()["ack_allowed"], false);
    assert!(ff.iter().all(|x| x["question"] != 2), "{f}");
    // The order preview: what will move, before publishing.
    let op = f["order_preview"].as_array().unwrap();
    assert_eq!(op[0]["movable"], json!([0, 1, 2]));
    assert_eq!(op[2]["frozen"], "sibling_refs");
    assert_eq!(op[2]["movable"], json!([]));
    assert_eq!(j(&label_findings_json(p.as_bytes(), ""))["order_preview"], f["order_preview"]);
    // Errors that point at the key are marked for the author only.
    let r = j(&validate_for_publish_json(p.as_bytes(), r#"{"hide_key":true}"#));
    let e = r["errors"].as_array().unwrap();
    assert!(e.iter().all(|x| x["author_only"] == true && x["reason"].is_string()), "{r}");
}

#[test]
fn matching_pairs_sharing_a_token_are_refused() {
    let m = |pairs: &[(&str, &str)], d: &[&str]| json!({"schema_version": 2, "question": {"id":"q","stem":"Match","kind":{"Matching":{"pairs": pairs.iter().map(|(l, r)| json!({"left": l, "right": r})).collect::<Vec<_>>(), "distractors": d}},"points":null,"difficulty":null,"created":0,"modified":0}}).to_string();
    for pairs in [
        [("France \u{2460}", "Paris \u{2460}"), ("Spain \u{2461}", "Madrid \u{2461}"), ("Italy \u{2462}", "Rome \u{2462}")],
        [("France \u{1F534}", "Paris \u{1F534}"), ("Spain \u{1F535}", "Madrid \u{1F535}"), ("Italy \u{1F7E2}", "Rome \u{1F7E2}")],
        [("France - 1", "Paris - 1"), ("Spain - 2", "Madrid - 2"), ("Italy - 3", "Rome - 3")],
        [("France #1", "Paris #1"), ("Spain #2", "Madrid #2"), ("Italy #3", "Rome #3")],
    ] {
        let r = j(&strip_json(m(&pairs, &[]).as_bytes(), r#"{"labels":[{"question":0,"choice":"strip"}]}"#));
        let r = if r["errors"][0]["code"] == "labels_unused" { j(&strip_json(m(&pairs, &[]).as_bytes(), "")) } else { r };
        assert!(r["errors"].as_array().is_some_and(|e| e.iter().any(|x| x["reason"] == "pairing_token")), "{pairs:?}: {r}");
    }
    // Labels stripped from the rights, the shared token stays: still refused.
    let p = m(&[("France \u{2460}", "a. Paris \u{2460}"), ("Spain \u{2461}", "b. Madrid \u{2461}"), ("Italy \u{2462}", "c. Rome \u{2462}")], &[]);
    let r = j(&strip_json(p.as_bytes(), r#"{"labels":[{"question":0,"choice":"strip"}]}"#));
    assert!(r["errors"].as_array().is_some_and(|e| e.iter().any(|x| x["reason"] == "pairing_token")), "{r}");
    // Digits inside words are not tokens ("L12" / "R12"), and years are content.
    let ok = j(&strip_json(m(&[("L1", "R1"), ("L2", "R2")], &[]).as_bytes(), ""));
    assert_eq!(ok["ok"], true, "{ok}");
    let ok = j(&strip_json(m(&[("WWI ends", "1918"), ("WWII ends", "1945")], &["1939"]).as_bytes(), ""));
    assert_eq!(ok["ok"], true, "{ok}");
}

// ---- N3 / N4: costs ---------------------------------------------------------------------------------

/// Timing bounds are relaxed when the mutation runner shares the machine (it sets this), so a
/// slow neighbour never counts as a caught mutation.
fn loaded() -> bool {
    std::env::var("SHARE_CORE_MUTATION").is_ok()
}

/// The fastest of seven runs: other test threads running at the same time only slow some runs.
fn time<F: FnMut()>(mut f: F) -> f64 {
    (0..7).map(|_| { let t = std::time::Instant::now(); f(); t.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min)
}

/// Doubling the count (keyless word bank items, choices) or the length (sibling-reference text)
/// costs at most ~2.6x; the critics' Worker-pattern payloads stay under a hard bound.
#[test]
fn costs_are_linear() {
    let bank = |d: usize| {
        let q = |i: usize| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Matching":{{"pairs":[{}],"distractors":[{}]}}}},"points":null,"difficulty":null,"created":1,"modified":1,"keyless":true}}"#,
            (0..50).map(|k| format!(r#"{{"left":"l{k}","right":""}}"#)).collect::<Vec<_>>().join(","),
            (0..d).map(|k| format!("\"{}{k}\"", "x".repeat(1000))).collect::<Vec<_>>().join(","));
        format!(r#"{{"schema_version":2,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, (0..20).map(q).collect::<Vec<_>>().join(","))
    };
    let (a, b) = (bank(38), bank(76));
    // (Interleaved and the fastest kept: parallel test threads slow some runs.)
    let (mut ta, mut tb) = (f64::MAX, f64::MAX);
    for _ in 0..3 {
        ta = ta.min(time(|| { validate_json(a.as_bytes(), ""); }));
        tb = tb.min(time(|| { validate_json(b.as_bytes(), ""); }));
    }
    assert!(tb / ta < 2.6 || tb < 0.05 || loaded(), "keyless bank: {ta:.3} -> {tb:.3}");
    // sibling_ref on long choices ending in a parenthetical: bounded per choice.
    let long = |n: usize| format!("w {} (z)", "a ".repeat(n));
    let (s1, s2) = (long(1000), long(4000));
    let (t1, t2) = (time(|| for _ in 0..200 { std::hint::black_box(share_core::labels::sibling_ref(&s1)); }), time(|| for _ in 0..200 { std::hint::black_box(share_core::labels::sibling_ref(&s2)); }));
    assert!(t2 / t1 < 2.0 || t2 < 0.01 || loaded(), "sibling_ref is length-bounded: {t1:.4} -> {t2:.4}");
    // The critic's ps.rs payload, Worker pattern (vfp + strip).
    let q = |i: usize| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Mc":{{"answers":[{}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#,
        (0..26).map(|k| format!(r#"{{"text":"w{k} {} (z)","correct":{}}}"#, "a ".repeat(995), k == 0)).collect::<Vec<_>>().join(","));
    let mut qs = vec![];
    let mut n = 0;
    loop { let s = q(qs.len()); n += s.len() + 1; if n >= 3_850_000 { break; } qs.push(s); }
    let p = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, qs.join(","));
    // The "(z)" suffixes are a (non-revealing) label finding: the author keeps them.
    let lf = j(&label_findings_json(p.as_bytes(), ""));
    let labels: Vec<Value> = lf["label_findings"].as_array().unwrap().iter().map(|f| json!({"question": f["question"], "choice": "keep"})).collect();
    let vo = json!({"hide_key": true, "labels": labels}).to_string();
    let so = json!({"labels": labels}).to_string();
    // (The fastest of five runs: other work on the machine only slows some runs, and the bound
    // is the CPU cost the Worker's three calls pay, not the queueing around them. A real
    // regression is at least that slow in every run.)
    let el = (0..5)
        .map(|_| {
            let t = std::time::Instant::now();
            assert!(validate_for_publish_json(p.as_bytes(), &vo).contains("\"ok\":true"));
            assert!(validate_for_publish_json(p.as_bytes(), &vo).contains("\"ok\":true"));
            assert!(strip_json(p.as_bytes(), &so).contains("\"ok\":true"));
            t.elapsed().as_secs_f64()
        })
        .fold(f64::MAX, f64::min);
    let bound = if cfg!(debug_assertions) || loaded() { 120.0 } else { 3.0 };
    assert!(el < bound, "ps.rs Worker pattern: {el:.3} s");
}

// ---- N6: re-keying ---------------------------------------------------------------------------------

#[test]
fn rekeying_a_true_false_false_publishes() {
    use share_core::import::{Key, PartKey, localize, rekey};
    let origin = share_core::Origin { item_id: "i".into(), qid: "q1".into(), publish_id: "p".into(), version: 1, author_name: "T".into(), license: "CC-BY-4.0".into(), form: "stripped".into(), served_sha256: String::new(), match_hash: String::new(), chain: vec![] };
    let served = |kind: share_core::Kind| {
        let mut q = common::question("The Sun is a planet.", kind);
        q.keyless = true;
        q.origin = Some(origin.clone());
        localize(&q)
    };
    let republish = |q: &share_core::Question| j(&validate_for_publish_json(common::q_payload(q).as_bytes(), ""));
    let k = rekey(&served(share_core::Kind::Tf { correct: false }), &Key::Tf(false)).unwrap();
    assert!(k.rekeyed && !k.keyless);
    assert_eq!(republish(&k)["ok"], true, "{}", republish(&k));
    let parts = vec![share_core::Part { stem: "a".into(), kind: share_core::PartKind::Tf { correct: false } }, share_core::Part { stem: "b".into(), kind: share_core::PartKind::Short { sample: String::new() } }];
    let k = rekey(&served(share_core::Kind::Multipart { parts }), &Key::Multipart(vec![PartKey::Tf(false), PartKey::Short("y".into())])).unwrap();
    assert_eq!(republish(&k)["ok"], true, "{}", republish(&k));
}

// ---- N7: sibling references ---------------------------------------------------------------------------

#[test]
fn sibling_references_in_many_languages() {
    use share_core::labels::{SiblingRef, refers_to_siblings, sibling_ref};
    for t in [
        "A as well as B", "A through C", "A\u{2013}C", "A-C", "A along with B", "A together with B", "Weder A noch B", "Sia A che B", "N\u{E9} A n\u{E9} B",
        "A ainsi que B", "A\u{4E0E}B", "A en B", "A ve B", "A och B", "A og B", "A dan B", "A v\u{E0} B", "A\u{C640} B", "A\u{304A}\u{3088}\u{3073}B", "Zowel A als B",
        "A i B", "A oraz B", "\u{41D}\u{438} \u{410}, \u{43D}\u{438} \u{411}",
    ] {
        assert_eq!(sibling_ref(t), Some(SiblingRef::Labels), "{t}");
    }
    for t in [
        "Alle oben genannten", "\u{4EE5}\u{4E0A}\u{90FD}\u{5BF9}", "\u{4E0A}\u{8A18}\u{306E}\u{3059}\u{3079}\u{3066}", "Alle bovenstaande", "Wszystkie powy\u{17C}sze", "Hepsi",
        "\u{BAA8}\u{B450} \u{C815}\u{B2F5}", "Toutes les r\u{E9}ponses pr\u{E9}c\u{E9}dentes", "Aucune de ces r\u{E9}ponses", "Keine der oben genannten", "Tutte le precedenti",
        "Nessuna delle precedenti", "Todas as anteriores", "Nenhuma das anteriores", "\u{412}\u{441}\u{435} \u{43E}\u{442}\u{432}\u{435}\u{442}\u{44B} \u{432}\u{435}\u{440}\u{43D}\u{44B}",
        "Geen van bovenstaande", "\u{17B}adne z powy\u{17C}szych", "Hi\u{E7}biri", "Allt ovanst\u{E5}ende", "Semua benar", "All the above", "Any of the above", "Each of the above",
        "None of the others", "\u{412}\u{441}\u{435} \u{43F}\u{435}\u{440}\u{435}\u{447}\u{438}\u{441}\u{43B}\u{435}\u{43D}\u{43D}\u{43E}\u{435}",
    ] {
        assert_eq!(sibling_ref(t), Some(SiblingRef::Above), "{t}");
    }
    // Ordinary sentences and bare words are not references.
    for t in ["Above the troposphere", "All of them are warm-blooded", "All are good conductors", "none", "5 A", "5 V", "5 \u{3A9}"] {
        assert_eq!(sibling_ref(t), None, "{t}");
    }
    let shuffles = |c: &[&str]| !refers_to_siblings(&c.iter().map(|t| (*t, false)).collect::<Vec<_>>());
    assert!(shuffles(&["I only", "II only", "I and II", "Neither I nor II"]), "Roman numerals name the stem's statements");
    assert!(shuffles(&["5 \u{3A9}", "5 V", "5 A"]));
    assert!(shuffles(&["y", "x", "2", "none"]));
    assert!(shuffles(&["Na+", "K+", "Cl-", "Ca2+"]), "a trailing sign is a charge, not a joiner");
}

// ---- N8: author-only data -------------------------------------------------------------------------

#[test]
fn author_data_travels_separately_and_never_prints() {
    let p = mc(&["Venus", "Mars", "Earth", "Jupiter"], &[0], &[]);
    let r = j(&strip_json(p.as_bytes(), ""));
    assert_eq!(r["ok"], true);
    assert!(r.get("author_only").is_none() && r.get("served_orders").is_none(), "{r}");
    let a = j(&strip_author_data_json());
    assert_eq!(a["ok"], true);
    assert_eq!(a["served_orders"][0]["order"].as_array().unwrap().len(), 4);
    // Handed out once.
    assert_eq!(j(&strip_author_data_json())["errors"][0]["code"], "no_author_data");
    // A failed strip leaves none behind.
    let _ = j(&strip_json(p.as_bytes(), ""));
    let _ = j(&strip_json(b"{}", ""));
    assert_eq!(j(&strip_author_data_json())["errors"][0]["code"], "no_author_data");
    // Natively: not Serialize, and {:?} redacts.
    let v = share_core::validate(&p, &share_core::Limits::default(), &share_core::Structural).unwrap();
    let s = share_core::strip_with_seed(v.payload, &Default::default(), [1; 32]).unwrap();
    let dbg = format!("{s:?}");
    assert!(dbg.contains("redacted") && !dbg.contains("order: ["), "{dbg}");
    assert!(s.author_only.author_json().contains("served_orders"));
}

// ---- N11: locked flags ---------------------------------------------------------------------------------

#[test]
fn locking_only_the_correct_choice_is_refused() {
    let codes = |r: Result<Vec<String>, Vec<String>>| r.err().unwrap_or_default();
    assert!(codes(publish(&mc(&["Venus", "Mars", "Earth", "Jupiter"], &[2], &[2]), "auto", json!({}))).contains(&"locked_key".to_string()));
    // An "all of the above" locked at the end is fine, correct or not; so is any lock with others.
    assert!(publish(&mc(&["Venus", "Mars", "Earth", "All of the above"], &[3], &[3]), "auto", json!({})).is_ok());
    // Round 7: the correct choice locked with one other is a 50/50: refused.
    assert!(codes(publish(&mc(&["Venus", "Mars", "Earth", "Jupiter"], &[2], &[1, 2]), "auto", json!({}))).contains(&"locked_key".to_string()));
    assert!(codes(publish(&mc(&["Venus", "Mars", "Earth", "I don't know"], &[0], &[0, 3]), "auto", json!({}))).contains(&"locked_key".to_string()));
    // Round 8: locking the correct choice with up to two others narrows the key too.
    assert!(codes(publish(&mc(&["Venus", "Mars", "Earth", "Jupiter", "Pluto"], &[2], &[1, 2, 4]), "auto", json!({}))).contains(&"locked_key".to_string()));
    assert!(publish(&mc(&["Venus", "Mars", "Earth", "Jupiter", "Pluto", "Neptune"], &[2], &[1, 2, 4, 5]), "auto", json!({})).is_ok());
    // A frozen order is served all locked: no leak.
    assert!(publish(&mc(&["Venus", "Mars", "Both A and B", "Jupiter"], &[2], &[2]), "auto", json!({})).is_ok());
}

// ---- N12: limits and provenance --------------------------------------------------------------------

#[test]
fn provenance_can_be_dropped_and_limits_clamped() {
    let d = share_core::Limits::default();
    let up = share_core::Limits { description_bytes: 1 << 30, figure_text_bytes: 1 << 30, chain_links: 1 << 20, ..d.clone() }.sane();
    assert_eq!((up.description_bytes, up.figure_text_bytes, up.chain_links), (d.description_bytes, d.figure_text_bytes, d.chain_links));
    let mut q: Value = j(&mc(&["Venus", "Mars"], &[0], &[]));
    q["question"]["origin"] = json!({"item_id":"forged","qid":"q","publish_id":"p","version":1,"author_name":"Someone","license":"CC-BY-4.0","form":"full"});
    let p = q.to_string();
    let kept = j(&strip_json(p.as_bytes(), ""));
    assert_eq!(kept["payload"]["question"]["origin"]["item_id"], "forged");
    let dropped = j(&strip_json(p.as_bytes(), r#"{"drop_client_provenance":true}"#));
    assert!(dropped["payload"]["question"].get("origin").is_none_or(Value::is_null), "{dropped}");
}

// ---- non-blocking: labels that are not labels ---------------------------------------------------------

#[test]
fn numbers_and_initials_are_not_stripped_into_nonsense() {
    use share_core::labels::analyze;
    for t in [vec!["1 : 2", "2 : 3", "3 : 4", "4 : 5"], vec!["1 - 2 hours", "2 - 3 hours", "3 - 4 hours"]] {
        assert!(analyze(&t).is_none(), "{t:?}");
    }
    let a = analyze(&["A. A. Einstein", "B. B. Franklin", "C. C. Darwin"]).unwrap();
    assert!(!a.strip_allowed, "the initial is part of the name");
}
