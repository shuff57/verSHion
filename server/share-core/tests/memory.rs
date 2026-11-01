//! Peak heap use of `validate` and `strip` on hostile and on large legitimate payloads, measured
//! with a counting global allocator (the critics' inputs from scratchpad critic11b/node/mem.mjs).
//! The wasm build's linear memory is checked the same way in `tests/node/check.mjs`.
//!
//! Run alone (`cargo test --test memory -- --test-threads=1`): the counter is process-wide.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;
static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), Ordering::Relaxed) + l.size();
            PEAK.fetch_max(c, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        CUR.fetch_sub(l.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            if n > l.size() {
                let c = CUR.fetch_add(n - l.size(), Ordering::Relaxed) + (n - l.size());
                PEAK.fetch_max(c, Ordering::Relaxed);
            } else {
                CUR.fetch_sub(l.size() - n, Ordering::Relaxed);
            }
        }
        q
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Peak bytes allocated above the starting level while `f` runs.
fn peak<F: FnOnce() -> String>(f: F) -> (usize, String) {
    let base = CUR.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let out = f();
    (PEAK.load(Ordering::Relaxed) - base, out)
}

const MB: usize = 1024 * 1024;
const CEILING: usize = 40 * MB;

fn hostile() -> Vec<(&'static str, String)> {
    let objs = format!(r#"{{"schema_version":1,"question":{{"x":[{}{{"":0}}]}}}}"#, r#"{"":0},"#.repeat(248_999));
    let unit = r#"{"":{"":{"":{"":{"":{"":{"":{"":{"":{"":{"":{"":0}}}}}}}}}}}},"#;
    let objs2 = format!(r#"{{"schema_version":1,"question":{{"x":[{}0]}}}}"#, unit.repeat(499_000 / 13));
    let strs = format!(r#"{{"schema_version":1,"question":{{"x":[{}""]}}}}"#, "\"\",".repeat(498_999));
    // Known positions: tiny answers (kept, so they cost real memory) up to the caps.
    let ans = (0..26).map(|_| r#"{"text":"a"}"#).collect::<Vec<_>>().join(",");
    let q = |i: usize| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Mc":{{"answers":[{ans}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#);
    let tiny = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, (0..1000).map(q).collect::<Vec<_>>().join(","));
    let blanks = {
        let acc = (0..50).map(|_| format!("[{}]", [r#""v""#; 20].join(","))).collect::<Vec<_>>().join(",");
        let q = |i: usize| format!(r#"{{"id":"q{i}","stem":"s{}","kind":{{"Blank":{{"accept":[{acc}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#, " ___".repeat(50));
        format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, (0..1000).map(q).collect::<Vec<_>>().join(","))
    };
    let lab = "A. ".repeat(640);
    let lab2 = "B. ".repeat(640);
    let q = |i: usize| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Mc":{{"answers":[{{"text":"{lab}x","correct":true}},{{"text":"{lab2}y"}}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#);
    let labels = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, (0..1000).map(q).collect::<Vec<_>>().join(","));
    // Kept objects up to the object limit: 1,000 multipart questions of 50 Tf parts.
    let part = r#"{"stem":"p","kind":{"Tf":{"correct":true}}}"#;
    let parts = [part; 50].join(",");
    let q = |i: usize| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Multipart":{{"parts":[{parts}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#);
    let kept = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, (0..1000).map(q).collect::<Vec<_>>().join(","));
    // Fewer values than the limit, but long strings in known positions: the allocation budget.
    let long = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#,
        (0..1000).map(|i| format!(r#"{{"id":"q{i}","stem":"{}","kind":{{"Tf":{{"correct":true}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#, "s".repeat(4000))).collect::<Vec<_>>().join(","));
    // The critics' shapes: 256 short figure params per question; and U+1D160, which NFC triples.
    // 180 params x 1,000 questions: accepted, just under the value limit (256 would be refused).
    let ps: String = (0..180).map(|k| format!(r#""p{k}":"xxxxxx""#)).collect::<Vec<_>>().join(",");
    let params = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#,
        (0..1000).map(|i| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Tf":{{"correct":true}}}},"points":null,"difficulty":null,"created":1,"modified":1,"figure":{{"id":"f","params":{{{ps}}}}}}}"#)).collect::<Vec<_>>().join(","));
    let n = "\u{1D160}";
    let mut qs = vec![];
    let mut len = 200;
    for i in 0..1000 {
        let ans: Vec<String> = (0..26).map(|j| format!(r#"{{"text":"{}{}","correct":{}}}"#, n.repeat(169), (b'a' + j as u8) as char, j == 0)).collect();
        let q = format!(r#"{{"id":"q{i}","stem":"{}","explanation":"{}","kind":{{"Mc":{{"answers":[{}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#, n.repeat(682), n.repeat(682), ans.join(","));
        if len + q.len() > 4_190_000 {
            break;
        }
        len += q.len() + 1;
        qs.push(q);
    }
    let nfc = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, qs.join(","));
    // Long figure-param keys: the key bytes are charged.
    let kp = "k".repeat(200);
    let lk: String = (0..256).map(|k| format!(r#""{kp}{k}":0"#)).collect::<Vec<_>>().join(",");
    let mut qs = vec![];
    let mut len = 200;
    for i in 0..1000 {
        let q = format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Tf":{{"correct":true}}}},"points":null,"difficulty":null,"created":1,"modified":1,"figure":{{"id":"f","params":{{{lk}}}}}}}"#);
        if len + q.len() > 4_190_000 {
            break;
        }
        len += q.len() + 1;
        qs.push(q);
    }
    let long_keys = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, qs.join(","));
    // Many labelled choices (critic 3a's many_choices): 41 questions x 50 parts x 26 answers, all
    // findings undecided, so every preview path runs.
    let l = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let part = |k: usize| format!(r#"{{"stem":"p","kind":{{"Mc":{{"answers":[{}]}}}}}}"#, (0..26).map(|i| format!(r#"{{"text":"{}. w{k}{i}{}"{}}}"#, l[i] as char, "x".repeat(25), if i == 0 { r#","correct":true"# } else { "" })).collect::<Vec<_>>().join(","));
    let many = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#,
        (0..41).map(|i| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Multipart":{{"parts":[{}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#, (0..50).map(part).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(","));
    vec![("many_choices", many), ("params", params), ("nfc", nfc), ("long_keys", long_keys), ("objs", objs), ("objs2", objs2), ("strs", strs), ("tiny_answers", tiny), ("blanks", blanks), ("labels", labels), ("kept_objects", kept), ("long_strings", long)]
}

/// A legitimate bank just under 4 MB: 1,000 questions of 26 choices.
fn legit() -> String {
    let q = |i: usize| {
        let answers: Vec<serde_json::Value> = (0..26).map(|j| serde_json::json!({"text": format!("planet {j} {}", "x".repeat(60)), "correct": j == 0, "locked": false})).collect();
        serde_json::json!({"id": format!("q{i}"), "stem": "S".repeat(600), "points": 1, "difficulty": null, "created": 0, "modified": 0, "explanation": "E".repeat(400), "kind": {"Mc": {"answers": answers}}})
    };
    let qs: Vec<serde_json::Value> = (0..1000).map(q).collect();
    serde_json::json!({"schema_version": 1, "bank": {"id": "b", "name": "Big", "created": 0, "modified": 0, "questions": qs}}).to_string()
}

#[test]
fn hostile_inputs_stay_under_the_ceiling() {
    let _g = ONE_AT_A_TIME.lock().unwrap();
    for (name, p) in hostile() {
        let (v, out) = peak(|| share_core::wasm::validate_json(p.as_bytes(), ""));
        let (s, out2) = peak(|| share_core::wasm::strip_json(p.as_bytes(), ""));
        let (f, _) = peak(|| share_core::wasm::label_findings_json(p.as_bytes(), ""));
        let (vp, _) = peak(|| share_core::wasm::validate_for_publish_json(p.as_bytes(), r#"{"hide_key":true}"#));
        eprintln!(
            "{name:>13}: {:>8} bytes in; peak validate {:>5.1} MB, strip {:>5.1} MB, label_findings {:>5.1} MB, validate_for_publish {:>5.1} MB; {}",
            p.len(), v as f64 / MB as f64, s as f64 / MB as f64, f as f64 / MB as f64, vp as f64 / MB as f64, &out[..out.len().min(70)]
        );
        assert!(v < CEILING && s < CEILING && f < CEILING && vp < CEILING, "{name}: {v} / {s} / {f} / {vp} bytes");
        let _ = out2;
    }
}

/// The reader's allocation estimate is an upper bound of what the parsed tree really allocates,
/// for every shape (so the budget bounds memory, whatever the payload).
#[test]
fn allocation_estimate_bounds_the_real_tree() {
    let _g = ONE_AT_A_TIME.lock().unwrap();
    let lim = share_core::Limits { parse_budget_bytes: 256 * MB, max_values: 2_000_000, max_objects: 400_000, ..Default::default() };
    let mut shapes = hostile();
    shapes.push(("legit", legit()));
    for (name, p) in shapes {
        let base = CUR.load(Ordering::Relaxed);
        PEAK.store(base, Ordering::Relaxed);
        let r = share_core::json::parse_stats(&p, &share_core::json::ENVELOPE, lim.budget(), &lim);
        let real = PEAK.load(Ordering::Relaxed) - base;
        if let Ok((tree, estimate)) = r {
            eprintln!("{name:>13}: tree {:>5.1} MB, estimate {:>5.1} MB", real as f64 / MB as f64, estimate as f64 / MB as f64);
            assert!(real <= estimate + 256 * 1024, "{name}: real {real} > estimate {estimate}");
            drop(tree);
        }
    }
}

#[test]
fn legit_bank_peak() {
    let _g = ONE_AT_A_TIME.lock().unwrap();
    let p = legit();
    assert!(p.len() < 4 * MB);
    let (v, out) = peak(|| share_core::wasm::validate_json(p.as_bytes(), ""));
    assert!(out.contains("\"ok\":true"), "{}", &out[..200]);
    let (s, out) = peak(|| share_core::wasm::strip_json(p.as_bytes(), ""));
    assert!(out.contains("\"ok\":true"));
    eprintln!("legit bank: {} bytes in; peak validate {:.1} MB, strip {:.1} MB", p.len(), v as f64 / MB as f64, s as f64 / MB as f64);
    assert!(v < CEILING && s < CEILING, "{v} / {s}");
}

// ---- per-function cost bound over field shapes (round 5) ------------------------------------------

/// The worst field shapes, each filling a 4 MB body: one huge field (refused before any character
/// work), many fields at their limit with a refused character first / in the middle / last, CR- and
/// BOM-heavy fields, NFC-expanding fields, combining-mark-heavy fields, and many small fields. The
/// same shapes are in `tests/node/mem.mjs` (modes `f_*`).
fn field_shapes() -> Vec<(&'static str, String)> {
    let one = |stem: &str, kind: &str, extra: &str| format!(r#"{{"schema_version":1,"question":{{"id":"q","stem":{stem},"kind":{kind},"points":null,"difficulty":null,"created":1,"modified":1{extra}}}}}"#);
    let tf = r#"{"Tf":{"correct":true}}"#;
    let big = 4 * MB - 400;
    let js = |s: &str| serde_json::to_string(s).unwrap();
    let bank = |stems: &dyn Fn(usize) -> String| {
        let mut qs = Vec::new();
        let mut len = 200;
        for i in 0.. {
            let q = format!(r#"{{"id":"q{i}","stem":{},"kind":{tf},"points":null,"difficulty":null,"created":1,"modified":1}}"#, stems(i));
            if len + q.len() + 1 > 4 * MB - 100 || i >= 1000 {
                break;
            }
            len += q.len() + 1;
            qs.push(q);
        }
        format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, qs.join(","))
    };
    let at_limit = |pos: usize| {
        move |_| {
            let mut s = "a".repeat(8 * 1024 - 3);
            let k = [0, s.len() / 2, s.len()][pos];
            s.insert(k, '\u{202E}');
            js(&s)
        }
    };
    let ans = |t: &str| format!(r#"{{"Mc":{{"answers":[{{"text":{},"correct":true}},{{"text":"b"}}]}}}}"#, js(t));
    vec![
        ("f_long_stem", one(&js(&"a".repeat(big)), tf, "")),
        ("f_cr_refused", one(&js(&format!("\r{}\u{202E}", "a".repeat(big - 8))), tf, "")),
        ("f_bom_refused", one(&js(&format!("\u{FEFF}{}\u{202E}", "a".repeat(big - 8))), tf, "")),
        ("f_long_choice", one(r#""s""#, &ans(&"a".repeat(big)), "")),
        ("f_long_expl", one(r#""s""#, tf, &format!(r#","explanation":{}"#, js(&"a".repeat(big))))),
        ("f_refused_first", bank(&at_limit(0))),
        ("f_refused_middle", bank(&at_limit(1))),
        ("f_refused_last", bank(&at_limit(2))),
        ("f_crlf", bank(&|_| js(&"\r\n".repeat(4 * 1024 - 2)))),
        ("f_bom", bank(&|_| js(&format!("{}x", "\u{FEFF}".repeat(2700))))),
        ("f_nfc_expand", bank(&|_| js(&"\u{1D160}".repeat(2000)))),
        ("f_marks", bank(&|_| js(&format!("a{}", "\u{0301}".repeat(16)).repeat(240)))),
        ("f_many_small", {
            let a = (0..26).map(|j| format!(r#"{{"text":"choice {j} {}","correct":{}}}"#, "x".repeat(80), j == 0)).collect::<Vec<_>>().join(",");
            let q = |i: usize| format!(r#"{{"id":"q{i}","stem":"s","kind":{{"Mc":{{"answers":[{a}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#);
            format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, (0..1000).map(q).collect::<Vec<_>>().join(","))
        }),
    ]
}

/// Peak heap per input byte, per exported function, for every field shape: at most
/// `COST_PER_BYTE` x the input plus 1 MB, and under the 40 MB ceiling.
const COST_PER_BYTE: f64 = 4.0;

#[test]
fn field_shapes_have_a_per_byte_cost_bound() {
    use share_core::wasm::*;
    let _g = ONE_AT_A_TIME.lock().unwrap();
    type F = fn(&[u8], &str) -> String;
    let fns: [(&str, F, &str); 6] = [
        ("validate", validate_json, ""),
        ("validate_for_publish", validate_for_publish_json, r#"{"hide_key":true}"#),
        ("strip", strip_json, ""),
        ("label_findings", label_findings_json, ""),
        ("match_hash", match_hash_json, ""),
        ("full_hash", full_hash_json, ""),
    ];
    for (name, p) in field_shapes() {
        assert!(p.len() <= 4 * MB, "{name}: {} bytes", p.len());
        let mut row = format!("{name:>17} {:>8} B:", p.len());
        let mut first = String::new();
        for (f, call, opts) in fns {
            let (m, out) = peak(|| call(p.as_bytes(), opts));
            if first.is_empty() {
                first = out[..out.len().min(60)].to_string();
            }
            let per = m as f64 / p.len() as f64;
            row += &format!(" {f} {:.2}", per);
            assert!(m as f64 <= COST_PER_BYTE * p.len() as f64 + MB as f64 && m < CEILING, "{name}/{f}: peak {m} for {} bytes", p.len());
        }
        eprintln!("{row}  [{first}]");
    }
}

/// Round 17 (critic31a B6): the Blank bank with 170,000 distinct variants (170 questions x 50
/// blanks x 20), the critic's 4 MB shape whose trie took 166 MB in wasm. The sorted-arena `Lex`
/// keeps it under the 40 MB ceiling; its per-byte cost is about 6x, above the 4x bound of the
/// field shapes (README, Costs).
#[test]
fn blank_bank_with_170000_variants_stays_under_the_ceiling() {
    let _g = ONE_AT_A_TIME.lock().unwrap();
    let acc = (0..50)
        .map(|b| format!("[{}]", (0..20).map(|k| format!("\"ka{}{} lele ka lele\"", b, k)).collect::<Vec<_>>().join(",")))
        .collect::<Vec<_>>()
        .join(",");
    let q = |i: usize| format!(r#"{{"id":"q{i}","stem":"{}","kind":{{"Blank":{{"accept":[{acc}]}}}},"points":null,"difficulty":null,"created":1,"modified":1}}"#, "___ ".repeat(50));
    let p = format!(r#"{{"schema_version":1,"bank":{{"id":"b","name":"n","created":1,"modified":2,"questions":[{}]}}}}"#, (0..170).map(q).collect::<Vec<_>>().join(","));
    assert!(p.len() > 3 * MB && p.len() <= 4 * MB, "{}", p.len());
    let (v, _) = peak(|| share_core::wasm::validate_for_publish_json(p.as_bytes(), r#"{"hide_key":true}"#));
    let (s, _) = peak(|| share_core::wasm::strip_json(p.as_bytes(), ""));
    eprintln!("blank170: {} bytes in; peak validate_for_publish {:.1} MB ({:.2}x), strip {:.1} MB", p.len(), v as f64 / MB as f64, v as f64 / p.len() as f64, s as f64 / MB as f64);
    assert!(v < CEILING && s < CEILING, "{v} / {s}");
}
