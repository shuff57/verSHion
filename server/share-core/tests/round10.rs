//! Round 10: repetition amplification. Every token a scanner starts on (the word tables of
//! `format` and `labels`, and the syntax marks) is repeated to fill every field kind at its byte
//! limit; `publish_prepare` (validation, the publish checks, the refusal's findings and order
//! preview, or the strip) must stay linear: the time of one such bank, scaled to a 4 MB body, is
//! bounded. A scanner that re-reads the field from every occurrence of its token is quadratic in
//! the field and fails this bound by orders of magnitude.
use serde_json::{Value, json};
use share_core::wasm::publish_prepare_json;

/// `unit` repeated to at most `bytes` bytes (whole characters).
fn fill(unit: &str, bytes: usize) -> String {
    let mut s = String::with_capacity(bytes);
    'outer: loop {
        for c in unit.chars() {
            if s.len() + c.len_utf8() > bytes {
                break 'outer;
            }
            s.push(c);
        }
        if unit.is_empty() {
            break;
        }
    }
    s
}

/// One bank per question kind (each valid on its own, so the publish checks run), every text
/// field filled to `scale` of its byte limit; the bank's name and description too.
fn banks(unit: &str, scale: f64) -> Vec<String> {
    let b = |n: usize| (n as f64 * scale) as usize;
    let stem = fill(unit, b(8 * 1024 - 8));
    let choice = |k: usize| format!("{} z{k}", fill(unit, b(2 * 1024 - 8)));
    let mc = |correct: usize| json!({"Mc": {"answers": (0..26).map(|k| json!({"text": choice(k), "correct": k == correct, "locked": false})).collect::<Vec<_>>()}});
    let key = if unit.trim().is_empty() { "a" } else { unit.trim() };
    let q0 = |kind: Value, stem: String| json!({"id": "q0", "stem": stem, "kind": kind, "points": null, "difficulty": null, "created": 1, "modified": 1, "topics": (0..20).map(|t| format!("{} {t}", fill(unit, b(56)))).collect::<Vec<_>>()});
    // (Blank stems carry their blanks first.)
    let q = |mut kind: Value| {
        let blanks = kind["Blank"]["accept"].as_array().map_or(0, |a| a.len());
        let lines = kind["Mc"]["_lines"] == true;
        if lines {
            kind["Mc"].as_object_mut().unwrap().remove("_lines");
        }
        let bullets = kind["Blank"]["_bullets"] == true;
        if bullets {
            kind["Blank"].as_object_mut().unwrap().remove("_bullets");
        }
        let stem = if bullets {
            format!("{}\nWord bank:\n{}", "___ ".repeat(50), fill(&format!("\u{2022} {unit}\n"), b(7600)))
        } else if blanks > 0 {
            format!("{}{}", "___ ".repeat(blanks), fill(unit, b(8 * 1024 - 8) - 4 * blanks))
        } else if lines {
            ["A. ", "B. "].iter().map(|l| format!("{l}{}", fill(unit, b(4000)))).collect::<Vec<_>>().join("\n")
        } else {
            stem.clone()
        };
        q0(kind, stem)
    };
    let kinds = [
        mc(0),
        json!({"Matching": {"pairs": (0..12).map(|k| json!({"left": choice(k), "right": format!("r{}", choice(k))})).collect::<Vec<_>>(), "distractors": (0..6).map(|k| format!("d{}", choice(k))).collect::<Vec<_>>()}}),
        json!({"Tf": {"correct": true}}),
        json!({"Multipart": {"parts": (0..2).map(|p| json!({"stem": stem, "kind": mc(p)})).collect::<Vec<_>>()}}),
        json!({"Short": {"sample": fill(unit, b(2000))}}),
        // Rights that are the token itself, found many times in their own lefts.
        json!({"Matching": {"pairs": (0..12).map(|k| json!({"left": format!("{} {k}", fill(unit, b(2040))), "right": unit.trim()})).collect::<Vec<_>>(), "distractors": []}}),
        // Keys that are the repeated token (critic22b): a Blank's accepted answer (one blank; fifty
        // blanks), a correct choice, a Short sample.
        json!({"Blank": {"accept": [[key]]}}),
        json!({"Blank": {"accept": (0..50).map(|_| vec![key]).collect::<Vec<_>>()}}),
        json!({"Mc": {"answers": [{"text": key, "correct": true, "locked": false}, {"text": "z1", "correct": false, "locked": false}, {"text": "z2", "correct": false, "locked": false}]}}),
        json!({"Short": {"sample": key}}),
        // (critic23b) A stem of label-led lines, each at a choice's length, every one full of the
        // token: the pasted-list check reads each line.
        json!({"Mc": {"answers": [{"text": key, "correct": true, "locked": false}, {"text": "z1", "correct": false, "locked": false}, {"text": "z2", "correct": false, "locked": false}], "_lines": true}}),
        // (critic23a) Fifty blanks over a bulleted word bank of the token: a long run of lines.
        json!({"Blank": {"accept": vec![vec![key]; 50], "_bullets": true}}),
    ];
    kinds.into_iter().map(|k| json!({"schema_version": 2, "bank": {"id": "b", "name": format!("{} n", fill(unit, b(190))), "created": 1, "modified": 2, "details": {"description": fill(unit, b(2000))}, "questions": [q(k)]}}).to_string()).collect()
}

/// Whether the publish checks ran: the body validated (a refusal by the publish checks is fine).
fn validated(p: &str) -> bool {
    share_core::wasm::validate_json(p.as_bytes(), "").contains("\"ok\":true")
}

/// Seconds for one `publish_prepare` of `p` (the fastest of `runs`).
fn secs(p: &str, runs: usize) -> f64 {
    (0..runs)
        .map(|_| {
            let a = std::time::Instant::now();
            std::hint::black_box(publish_prepare_json(p.as_bytes(), r#"{"hide_key":true}"#));
            a.elapsed().as_secs_f64()
        })
        .fold(f64::MAX, f64::min)
}

/// (the cost growth from half to full field size: ~2 linear, ~4 quadratic; seconds per 4 MB at
/// full size) for kind `k` of `unit`.
fn growth(unit: &str, k: usize, runs: usize) -> (f64, f64) {
    let (half, full) = (&banks(unit, 0.5)[k], &banks(unit, 1.0)[k]);
    let (th, tf) = (secs(half, runs), secs(full, runs));
    (tf / th.max(1e-6), tf * (4.0 * 1024.0 * 1024.0) / full.len() as f64)
}

/// Every token, alone, followed by a space, and (non-ASCII tokens) by "?". `every`: every n-th
/// unit only, plus every token of a script without spaces (the CJK leads were the round-10 find).
fn units(every: usize) -> Vec<String> {
    let mut units: Vec<String> = Vec::new();
    for (i, t) in share_core::format::scanner_tokens().into_iter().chain(share_core::labels::scanner_tokens()).enumerate() {
        let unspaced = t.chars().any(|c| matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0x0E00..=0x0E7F));
        // (Syntax marks and tokens of scripts without spaces always: the round-10 finds.)
        let syntax = !t.chars().any(char::is_alphabetic);
        if i % every != 0 && !unspaced && !syntax {
            continue;
        }
        let mut forms = vec![t.to_string(), format!("{t} ")];
        if !t.is_ascii() {
            forms.push(format!("{t}?"));
        }
        for u in forms {
            if !units.contains(&u) {
                units.push(u);
            }
        }
    }
    // Composite shapes that start a scanner's inner loop: bracket groups of annotation words,
    // delimiters before them, answer lines, braces, entities, letter lists.
    // (critic26a: answer leads of four scripts with colons, Georgian and Latin keys.)
    for u in ["\u{10DE}\u{10D0}\u{10E1}\u{10E3}\u{10EE}\u{10D8}: \u{10D1} \u{627}\u{644}\u{625}\u{62C}\u{627}\u{628}\u{629}: \u{628} \u{41E}\u{442}\u{432}\u{435}\u{442}: \u{411} \u{C815}\u{B2F5}: \u{3134} ", "Answer: x \u{41E}\u{442}\u{432}\u{435}\u{442}: y \u{C815}\u{B2F5}: z ", "\u{FF3F}", "\u{FF3F} ", "\u{FE4D}", "___", "x \u{2713} ", "**x** ", "*x ", "(correct) ", "- correct ", "A and B ", "Both A and B ", "Both A and B + ", "a ", "a. ", "a\n", "a, ", "a; ", "Mars ", "Mars, ", "\u{7B54}", "\u{7B54}\u{3002}", "___ ", "(a) ", "a/", "(yes x) ", "(this one) ", "(yes) ", "- yes x ", "{x ", "{=", "answer: zz ", "answer: b ", "(right) ", "(right-hand) ", "\u{B2F5} ", "\u{7B54}\u{6848}\u{662F}", "&amp;", "&#10007; ", "(a) and (c) ", "A, B and C ", "both a and b ", "Key: x ", "(100%) ", "# well done ", "~%50%x ", "\u{3010}\u{7B54}\u{6848}\u{3011}"] {
        units.push(u.to_string());
    }
    units
}

/// A sample of the tokens (every 7th, and all of scripts without spaces): the regular suite.
#[test]
fn repeated_tokens_stay_linear() {
    sweep(&units(7));
}

/// Every token: `cargo test --release --test round10 -- --ignored` (several minutes).
#[test]
#[ignore]
fn repeated_tokens_stay_linear_full() {
    sweep(&units(1));
}

fn sweep(units: &[String]) {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let jobs: Vec<(String, usize)> = units.iter().flat_map(|u| (0..12).map(move |k| (u.clone(), k))).collect();
    let chunks: Vec<Vec<(String, usize)>> = jobs.chunks(jobs.len().div_ceil(threads)).map(|c| c.to_vec()).collect();
    // First pass in parallel: quadratic growth (or a slow body) is measured again alone.
    let timed: Vec<(String, usize, f64, f64)> = std::thread::scope(|s| {
        let hs: Vec<_> = chunks.iter().map(|c| s.spawn(move || c.iter().map(|(u, k)| { let (g, t) = growth(u, *k, 1); (u.clone(), *k, g, t) }).collect::<Vec<_>>())).collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let worst = timed.iter().cloned().fold((String::new(), 0, 0.0, 0.0), |a, b| if b.3 > a.3 { b } else { a });
    eprintln!("{} bodies; slowest first pass {:?} kind {}: growth {:.2}, {:.3} s per 4 MB", timed.len(), worst.0, worst.1, worst.2, worst.3);
    // Bounds that do not depend on the machine's load: the growth from half to full size (linear
    // ~2, quadratic ~4), and the cost against plain text ("x ") of the same kind measured at the
    // same moment (at most 6x: round 12 made plain ASCII text about twice as fast to validate and
    // check, so the slowest scripts measured, Korean, Thai, Malayalam, Amharic, now stand at ~5x
    // of it at the same absolute cost; in wasm a 4 MB body of them takes about 2.3 s, under the
    // 3.5 s limit).
    let bases: Vec<f64> = (0..12).map(|k| growth("x ", k, 3).1).collect();
    let mut slow = Vec::new();
    for (u, k, g, t) in timed.iter().filter(|(_, k, g, t)| (*g > 3.0 && *t > 0.3) || *t > 3.0 * bases[*k]) {
        // (Interleaved with plain text, fastest of each: a burst of load hits both.)
        let (mut g2, mut t2, mut base) = (f64::MAX, f64::MAX, f64::MAX);
        for _ in 0..3 {
            let (g, t) = growth(u, *k, 3);
            g2 = g2.min(g);
            t2 = t2.min(t);
            base = base.min(growth("x ", *k, 3).1);
        }
        let relative = t2 / base.max(1e-6);
        // (Quadratic growth also shows as a cost well above plain text at full size.)
        // (Growth alone at under a second per 4 MB is a text that ends its scan early at half
        // size and reads to a fail-closed budget at full size: bounded, round 14.)
        if ((g2 > 3.3 && relative > 2.0 && t2 > 0.8) || relative > 6.0) && std::env::var("SHARE_CORE_MUTATION").is_err() {
            slow.push(format!("{u:?} kind {k}: growth {g:.2}/{g2:.2}, {t:.2}/{t2:.2} s per 4 MB, {relative:.1}x plain text"));
        }
    }
    assert!(slow.is_empty(), "{} of {} bodies:\n{}", slow.len(), timed.len(), slow.join("\n"));
    // The harness measures the checks only if the bodies validate: most of them must.
    let ok = units.iter().filter(|u| validated(&banks(u, 1.0)[0])).count();
    assert!(ok * 10 >= units.len() * 8, "only {ok} of {} Mc bodies validate", units.len());
}
