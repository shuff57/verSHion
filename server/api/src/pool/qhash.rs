//! The pure question-identity functions of worker/src/pool/provenance.ts: text normalisation, the licence-sense hash, the
//! key-free hash, block keys, unit keys, stem tokens and MinHash bands. Every hash here is stored, so each function builds
//! the exact string the Worker fed to its HMAC (see `qjson`), and the tests compare against values the Worker's own
//! functions produced (qhash_vectors.json).
use super::fingerprint::Fingerprinter;
use super::qjson::{Json, sort_utf16};
use std::collections::HashSet;
use unicode_general_category::{GeneralCategory as G, get_general_category};
use unicode_normalization::UnicodeNormalization;

/// ECMAScript `\s` or Unicode `Zs`.
fn is_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
        || get_general_category(c) == G::SpaceSeparator
}
fn is_punct(c: char) -> bool {
    matches!(
        get_general_category(c),
        G::ConnectorPunctuation | G::DashPunctuation | G::OpenPunctuation | G::ClosePunctuation | G::InitialPunctuation | G::FinalPunctuation | G::OtherPunctuation
    )
}
fn js_trim(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// Replaces each run of characters satisfying `f` with one space.
fn collapse(s: &str, f: impl Fn(char) -> bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_run = false;
    for c in s.chars() {
        if f(c) {
            if !in_run {
                out.push(' ');
            }
            in_run = true;
        } else {
            out.push(c);
            in_run = false;
        }
    }
    out
}

/// Text as the licence-sense comparison sees it: compatibility forms folded, invisible format characters (\p{Cf}, the
/// combining grapheme joiner) removed, case folded, punctuation and runs of whitespace (NBSP included) as one space,
/// trimmed. So a trailing period, a doubled space or a changed case keeps a copy a copy. Rewording is not caught here.
pub fn norm_text(s: &str) -> String {
    let nfkc: String = s.nfkc().filter(|c| *c != '\u{34f}' && get_general_category(*c) != G::Format).collect();
    let lower = nfkc.to_lowercase();
    let no_punct = collapse(&lower, is_punct);
    let spaced = collapse(&no_punct, is_space);
    js_trim(&spaced).to_string()
}

/// The stem's loose hash input: letters and digits only, case-folded; None when too short to mean anything.
pub fn stem_hash(fp: &Fingerprinter, stem: &Json) -> Option<String> {
    let s = stem.as_str()?;
    let lower = s.nfkc().collect::<String>().to_lowercase();
    let is_ln = |c: char| matches!(get_general_category(c), G::UppercaseLetter | G::LowercaseLetter | G::TitlecaseLetter | G::ModifierLetter | G::OtherLetter | G::DecimalNumber | G::LetterNumber | G::OtherNumber);
    let norm = collapse(&lower, |c| !is_ln(c));
    let norm = js_trim(&norm);
    if norm.encode_utf16().count() < 12 {
        return None; // too short to mean anything
    }
    Some(fp.fingerprint("stem", &format!("vershion/stem/v1\n{norm}")))
}

fn norm_deep(v: &Json) -> Json {
    match v {
        Json::Str(s) => Json::Str(norm_text(s)),
        Json::Arr(a) => Json::Arr(a.iter().map(norm_deep).collect()),
        Json::Obj(o) => Json::Obj(o.iter().map(|(k, x)| (k.clone(), norm_deep(x))).collect()),
        other => other.clone(),
    }
}

fn set_key(o: &mut Vec<(String, Json)>, k: &str, v: Json) {
    match o.iter_mut().find(|(e, _)| e == k) {
        Some(slot) => slot.1 = v,
        None => o.push((k.to_string(), v)),
    }
}
fn obj_of(v: &Json) -> Vec<(String, Json)> {
    if let Json::Obj(o) = v { o.clone() } else { vec![] }
}
fn sorted_strings(items: impl Iterator<Item = String>) -> Vec<String> {
    let mut v: Vec<String> = items.collect();
    sort_utf16(&mut v);
    v
}

/// The identity of a question does not depend on an order students never rely on: Mc choices (except locked positions),
/// Matching pairs and distractors, and Multipart parts are compared as sorted multisets, unless the author keeps the order
/// (`keep_order: true`), where order is content.
pub fn order_free(v: &Json) -> Json {
    let Json::Obj(q) = v else { return v.clone() };
    let kind = match v.get("kind") {
        Some(k @ Json::Obj(_)) if v.get("keep_order") != Some(&Json::Bool(true)) => k,
        _ => return v.clone(),
    };
    // Locked choices keep their places; the free ones are a multiset.
    let mc = |answers: &Json| -> Json {
        let Json::Arr(a) = answers else { return answers.clone() };
        let locked = |x: &Json| x.get("locked").is_some_and(Json::truthy);
        let free = sorted_strings(a.iter().filter(|x| !locked(x)).map(Json::stringify));
        let mut out: Vec<Json> = a.iter().enumerate().filter(|(_, x)| locked(x)).map(|(i, x)| Json::obj(vec![("at", Json::Num(i as f64)), ("a", x.clone())])).collect();
        out.extend(free.into_iter().map(|f| Json::obj(vec![("free", Json::Str(f))])));
        Json::Arr(out)
    };
    let mut kind = obj_of(kind);
    let truthy = |kind: &[(String, Json)], k: &str| kind.iter().find(|(e, _)| e == k).map(|(_, x)| x.clone()).filter(Json::truthy);
    if let Some(m) = truthy(&kind, "Mc") {
        let mut o = obj_of(&m);
        if let Some(ans) = m.get("answers") {
            set_key(&mut o, "answers", mc(ans));
        }
        set_key(&mut kind, "Mc", Json::Obj(o));
    }
    if let Some(m) = truthy(&kind, "Matching") {
        let mut o = obj_of(&m);
        let strs = |k: &str| Json::Arr(sorted_strings(m.get(k).map(Json::as_arr).unwrap_or(&[]).iter().map(Json::stringify)).into_iter().map(Json::Str).collect());
        set_key(&mut o, "pairs", strs("pairs"));
        set_key(&mut o, "distractors", strs("distractors"));
        set_key(&mut kind, "Matching", Json::Obj(o));
    }
    if let Some(m) = truthy(&kind, "Multipart") {
        let mut o = obj_of(&m);
        let parts = m.get("parts").map(Json::as_arr).unwrap_or(&[]);
        let keyed = sorted_strings(parts.iter().map(|p| {
            match p.get("kind").and_then(|k| k.get("Mc")).filter(|x| x.truthy()) {
                Some(mcv) if p.get("kind").is_some_and(Json::truthy) => {
                    let mut mo = obj_of(mcv);
                    if let Some(ans) = mcv.get("answers") {
                        set_key(&mut mo, "answers", mc(ans));
                    }
                    let mut po = obj_of(p);
                    set_key(&mut po, "kind", Json::obj(vec![("Mc", Json::Obj(mo))]));
                    Json::Obj(po).stringify()
                }
                _ => p.stringify(),
            }
        }));
        set_key(&mut o, "parts", Json::Arr(keyed.into_iter().map(Json::Str).collect()));
        set_key(&mut kind, "Multipart", Json::Obj(o));
    }
    let mut out = q.clone();
    set_key(&mut out, "kind", Json::Obj(kind));
    Json::Obj(out)
}

/// The licence-sense hash of a question as served: its canonical content with what is not content and what changing does
/// not adapt it left out (ids, times, origin, published, points, difficulty, topics). `bare` also leaves the explanation
/// out. Computed for both forms; a hidden subject never uses the full one.
pub fn near_hash(fp: &Fingerprinter, q: &Json, bare: bool) -> String {
    const DROP: [&str; 9] = ["id", "created", "modified", "origin", "published", "points", "difficulty", "topics", "explanation"];
    let Json::Obj(all) = q else { return fp.fingerprint("near", &Json::Arr(vec![Json::s("vershion/near/v3"), order_free(&norm_deep(q))]).stringify()) };
    let mut body: Vec<(String, Json)> = all.iter().filter(|(k, _)| !DROP.contains(&k.as_str())).cloned().collect();
    if !bare {
        if let Some(e) = q.get("explanation") {
            body.push(("explanation".into(), e.clone()));
        }
    }
    fp.fingerprint("near", &Json::Arr(vec![Json::s("vershion/near/v3"), order_free(&norm_deep(&Json::Obj(body)))]).stringify())
}

fn opt(v: Option<&Json>) -> Json {
    v.cloned().filter(|x| !matches!(x, Json::Null)).unwrap_or(Json::Null)
}

/// Hash of the key-free facts of a question as served to non-authors: its match_hash (key-free by construction in
/// share-core), points, difficulty, topics, and the image if it is served. Never the key, the explanation, a figure or an
/// unticked image.
pub fn keyfree_hash(fp: &Fingerprinter, match_hash: &str, served: &Json, img_of: &dyn Fn(&Json) -> Option<String>) -> String {
    let topics = sorted_strings(served.get("topics").map(Json::as_arr).unwrap_or(&[]).iter().map(|t| match t {
        Json::Str(s) => s.clone(),
        other => other.stringify(),
    }));
    let facts = Json::Arr(vec![
        Json::s("vershion/keyfree/v1"),
        Json::s(match_hash),
        opt(served.get("points")),
        opt(served.get("difficulty")),
        Json::Arr(topics.into_iter().map(Json::Str).collect()),
        img_of(served).map_or(Json::Null, Json::Str),
    ]);
    fp.fingerprint("keyfree", &facts.stringify())
}

/// ... and its key-free part: the match_hash and the image as served.
pub fn near_kf_hash(fp: &Fingerprinter, match_hash: &str, served: &Json, img_of: &dyn Fn(&Json) -> Option<String>) -> String {
    fp.fingerprint("near", &Json::Arr(vec![Json::s("vershion/nearkf/v1"), Json::s(match_hash), img_of(served).map_or(Json::Null, Json::Str)]).stringify())
}

fn kind_of(q: &Json) -> (String, Json) {
    let k = q.get("kind").and_then(Json::first_key).unwrap_or("").to_string();
    let body = q.get("kind").and_then(|x| x.get(&k)).cloned().filter(|b| !matches!(b, Json::Null)).unwrap_or(Json::Obj(vec![]));
    (k, body)
}
/// `String(v)` for the primitives a stored answer can be.
fn js_string(v: &Json) -> String {
    match v {
        Json::Str(s) => s.clone(),
        Json::Null => "null".into(),
        Json::Bool(b) => b.to_string(),
        Json::Num(n) => super::qjson::js_number(*n).replace("null", "NaN"),
        Json::Arr(a) => a.iter().map(|x| if matches!(x, Json::Null) { String::new() } else { js_string(x) }).collect::<Vec<_>>().join(","),
        Json::Obj(_) => "[object Object]".into(),
    }
}

/// The KEY-FREE words a question adds to its stem as served (choices, Matching columns, part stems and choices; never an
/// answer, a key or an explanation, which a hidden-key publish does not serve), with its kind as a token: a near stem with a
/// different body is a different question. Distinct, at most 64 (plus the kind).
pub fn body_tokens(q: &Json) -> Vec<String> {
    let (k, b) = kind_of(q);
    let mut texts: Vec<Json> = Vec::new();
    let text_of = |x: &Json| x.get("text").cloned().unwrap_or(Json::Null);
    match k.as_str() {
        "Mc" => texts.extend(b.get("answers").map(Json::as_arr).unwrap_or(&[]).iter().map(text_of)),
        "Matching" => {
            for p in b.get("pairs").map(Json::as_arr).unwrap_or(&[]) {
                texts.push(p.get("left").cloned().unwrap_or(Json::Null));
                texts.push(p.get("right").cloned().unwrap_or(Json::Null));
            }
            texts.extend(b.get("distractors").map(Json::as_arr).unwrap_or(&[]).iter().cloned());
        }
        "Multipart" => {
            for p in b.get("parts").map(Json::as_arr).unwrap_or(&[]) {
                texts.push(p.get("stem").cloned().unwrap_or(Json::Null));
                let sub = Json::obj(vec![("kind", p.get("kind").cloned().filter(|x| !matches!(x, Json::Null)).unwrap_or(Json::Obj(vec![])))]);
                texts.extend(body_tokens(&sub).into_iter().skip(1).map(Json::Str));
            }
        }
        _ => {}
    }
    let mut seen = HashSet::new();
    let mut words: Vec<String> = Vec::new();
    for t in texts.iter().filter_map(Json::as_str) {
        for w in norm_text(t).split(' ').filter(|w| !w.is_empty()) {
            if seen.insert(w.to_string()) {
                words.push(w.to_string());
            }
        }
    }
    words.truncate(64);
    let mut out = vec![format!("kind:{k}")];
    out.extend(words);
    out
}

/// Bodies close enough for a near stem to be the same question.
pub const BODY_JACCARD: f64 = 0.5;
pub fn same_body(a: Option<&[String]>, b: Option<&[String]>) -> bool {
    let (Some(a), Some(b)) = (a, b) else { return false };
    if a.first() != b.first() {
        return false;
    }
    if a.len() == 1 && b.len() == 1 {
        return true; // a Tf, a bare Short: the stem is the question
    }
    jaccard(a, b) >= BODY_JACCARD
}

fn strs(items: Vec<String>) -> Json {
    Json::Arr(items.into_iter().map(Json::Str).collect())
}
fn t(v: Option<&Json>) -> String {
    v.and_then(Json::as_str).map(norm_text).unwrap_or_default()
}

/// The block key of a question's student-visible skeleton in the licence-sense normalisation: kind, stem, Mc choices,
/// Matching columns, blank count, Multipart parts, every text through `norm_text`. A privacy or copyright block holds it
/// alongside the exact match_hash, so a one-character edit does not evade the block.
pub fn norm_skeleton_key(fp: &Fingerprinter, q: &Json) -> String {
    let (k, body) = kind_of(q);
    let mut sk: Vec<(String, Json)> = vec![("kind".into(), Json::s(&k)), ("stem".into(), Json::Str(t(q.get("stem"))))];
    let arr = |key: &str| body.get(key).map(Json::as_arr).unwrap_or(&[]).to_vec();
    if k == "Mc" {
        sk.push(("choices".into(), strs(sorted_strings(arr("answers").iter().map(|a| t(a.get("text")))))));
    }
    if k == "Matching" {
        sk.push(("left".into(), strs(sorted_strings(arr("pairs").iter().map(|p| t(p.get("left")))))));
        let mut seen = HashSet::new();
        let bank: Vec<String> = arr("pairs").iter().map(|p| t(p.get("right"))).chain(arr("distractors").iter().map(|d| t(Some(d)))).filter(|s| !s.is_empty() && seen.insert(s.clone())).collect();
        sk.push(("bank".into(), strs(sorted_strings(bank.into_iter()))));
    }
    if k == "Blank" {
        sk.push(("blanks".into(), Json::Num(body.get("accept").map_or(0, |a| a.as_arr().len()) as f64)));
    }
    if k == "Multipart" {
        let parts: Vec<Json> = arr("parts")
            .iter()
            .map(|p| {
                let pk = p.get("kind").and_then(Json::first_key).unwrap_or("").to_string();
                let mut o = vec![("stem".to_string(), Json::Str(t(p.get("stem")))), ("kind".to_string(), Json::s(&pk))];
                if pk == "Mc" {
                    let ans = p.get("kind").and_then(|x| x.get("Mc")).and_then(|m| m.get("answers")).map(Json::as_arr).unwrap_or(&[]);
                    o.push(("choices".into(), strs(sorted_strings(ans.iter().map(|a| t(a.get("text")))))));
                }
                Json::Obj(o)
            })
            .collect();
        // Parts as a multiset unless the order is kept.
        let v = if q.get("keep_order") == Some(&Json::Bool(true)) { Json::Arr(parts) } else { strs(sorted_strings(parts.iter().map(Json::stringify))) };
        sk.push(("parts".into(), v));
    }
    let h = fp.fingerprint("block", &Json::Arr(vec![Json::s("vershion/block-norm/v1"), Json::Obj(sk)]).stringify());
    format!("n:{h}")
}

/// The block key of a question's STEM alone, normalised: student data in the stem is held whatever the answer.
pub fn stem_key(fp: &Fingerprinter, q: &Json) -> String {
    let stem = q.get("stem").and_then(Json::as_str).map(norm_text).unwrap_or_default();
    format!("s:{}", fp.fingerprint("block", &format!("vershion/block-stem/v1\n{stem}")))
}

fn field_hash(fp: &Fingerprinter, tag: &str, s: &str) -> String {
    format!("{tag}:{}", fp.fingerprint("block", &format!("vershion/block-field/v1\n{tag}\n{s}")))
}

/// Field block keys: the normalised explanation, and the normalised model answer / accepted answers.
pub fn field_keys(fp: &Fingerprinter, q: &Json) -> (Option<String>, Option<String>) {
    let e = q.get("explanation").and_then(Json::as_str).map(norm_text).unwrap_or_default();
    let (k, body) = kind_of(q);
    let a = match (k.as_str(), body.get("accept")) {
        ("Short", _) => norm_text(&body.get("sample").filter(|x| !matches!(x, Json::Null)).map(js_string).unwrap_or_default()),
        ("Blank", Some(Json::Arr(acc))) => {
            // `accept.flat().map(String).join(" | ")`: one level flattened.
            let flat: Vec<String> = acc.iter().flat_map(|x| if let Json::Arr(inner) = x { inner.iter().map(js_string).collect::<Vec<_>>() } else { vec![js_string(x)] }).collect();
            norm_text(&flat.join(" | "))
        }
        _ => String::new(),
    };
    (if e.is_empty() { None } else { Some(field_hash(fp, "e", &e)) }, if a.is_empty() { None } else { Some(field_hash(fp, "a", &a)) })
}

/// The block keys of each Multipart part: its stem (`s:`, as a standalone question's stem key), explanation (`e:`) and
/// model / accepted answers (`a:`).
pub fn part_keys_of(fp: &Fingerprinter, q: &Json) -> Vec<(usize, String)> {
    let parts = q.get("kind").and_then(|k| k.get("Multipart")).and_then(|m| m.get("parts")).map(Json::as_arr).unwrap_or(&[]);
    let mut out = Vec::new();
    for (k, p) in parts.iter().enumerate() {
        if !matches!(p, Json::Obj(_)) {
            continue;
        }
        if p.get("stem").and_then(Json::as_str).is_some_and(|s| !js_trim(s).is_empty()) {
            out.push((k, stem_key(fp, p)));
        }
        let (e, a) = field_keys(fp, p);
        out.extend(e.map(|x| (k, x)));
        out.extend(a.map(|x| (k, x)));
    }
    out
}

/// The explanation and answer block keys a text WOULD have (a lifted explanation may be pasted as a model answer, and the
/// other way round).
pub fn text_field_keys(fp: &Fingerprinter, text: &str) -> Vec<String> {
    let t = norm_text(text);
    if t.is_empty() { vec![] } else { vec![field_hash(fp, "e", &t), field_hash(fp, "a", &t)] }
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnitKey {
    /// -1: the question itself; otherwise the Multipart part's index.
    pub part: i64,
    pub key: String,
    pub stem_only: bool,
}

/// A question's key-free identity as a UNIT, the same whether it stands alone or is a part of a Multipart: its kind,
/// normalised stem and choices (as a set) or Matching columns. For the question itself and each Multipart part; none for a
/// short Tf / Short / Blank statement.
pub fn unit_keys_of(fp: &Fingerprinter, q: &Json) -> Vec<UnitKey> {
    let one = |stem: Option<&Json>, kind: Option<&Json>| -> Option<String> {
        let k = kind.and_then(Json::first_key).unwrap_or("");
        let stem = stem.and_then(Json::as_str)?;
        if k.is_empty() || k == "Multipart" {
            return None;
        }
        let b = kind.and_then(|x| x.get(k)).cloned().filter(|x| !matches!(x, Json::Null)).unwrap_or(Json::Obj(vec![]));
        let st = norm_text(stem);
        if ["Tf", "Short", "Blank"].contains(&k) && st.split(' ').filter(|w| !w.is_empty()).count() <= 8 {
            return None;
        }
        let set = |xs: Vec<&Json>| strs(sorted_strings(xs.into_iter().filter_map(Json::as_str).map(norm_text)));
        let arr = |key: &str| b.get(key).map(Json::as_arr).unwrap_or(&[]).to_vec();
        let body = match k {
            "Mc" => set(arr("answers").iter().map(|a| a.get("text").unwrap_or(&Json::Null).clone()).collect::<Vec<_>>().iter().collect()),
            "Matching" => {
                let lefts: Vec<Json> = arr("pairs").iter().map(|p| p.get("left").cloned().unwrap_or(Json::Null)).collect();
                let rights: Vec<Json> = arr("pairs").iter().map(|p| p.get("right").cloned().unwrap_or(Json::Null)).chain(arr("distractors")).filter(|x| *x != Json::s("")).collect();
                Json::Arr(vec![set(lefts.iter().collect()), set(rights.iter().collect())])
            }
            _ => Json::Arr(vec![]),
        };
        Some(format!("u:{}", fp.fingerprint("block", &Json::Arr(vec![Json::s("vershion/unit/v1"), Json::s(k), Json::Str(st), body]).stringify())))
    };
    let stem_only = |kind: Option<&Json>| ["Tf", "Short", "Blank"].contains(&kind.and_then(Json::first_key).unwrap_or(""));
    let mut out = Vec::new();
    if let Some(key) = one(q.get("stem"), q.get("kind")) {
        out.push(UnitKey { part: -1, key, stem_only: stem_only(q.get("kind")) });
    }
    let parts = q.get("kind").and_then(|k| k.get("Multipart")).and_then(|m| m.get("parts")).map(Json::as_arr).unwrap_or(&[]);
    for (k, p) in parts.iter().enumerate() {
        if let Some(key) = one(p.get("stem"), p.get("kind")) {
            out.push(UnitKey { part: k as i64, key, stem_only: stem_only(p.get("kind")) });
        }
    }
    out
}

// ---- fuzzy stems ----

/// The stem's normalised words (`norm_text`), distinct, at most 64.
pub fn stem_tokens(stem: &Json) -> Vec<String> {
    let Some(s) = stem.as_str() else { return vec![] };
    let mut seen = HashSet::new();
    let mut out: Vec<String> = norm_text(s).split(' ').filter(|w| !w.is_empty()).filter(|w| seen.insert(w.to_string())).map(String::from).collect();
    out.truncate(64);
    out
}

/// FNV-1a over UTF-16 code units, seeded the way `Math.imul(seed, 0x9e3779b1)` seeds it.
fn fnv(s: &str, seed: u32) -> u32 {
    let mut h = 0x811c9dc5u32 ^ seed.wrapping_mul(0x9e3779b1);
    for u in s.encode_utf16() {
        h ^= u as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}
fn base36(mut n: u32) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut d = Vec::new();
    while n > 0 {
        d.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(n % 36) as usize]);
        n /= 36;
    }
    d.reverse();
    String::from_utf8(d).unwrap()
}
pub const STEM_BANDS: usize = 8;
/// MinHash over the tokens, 16 hashes in 8 bands of 2: two stems with Jaccard 0.8 share a band with probability > 0.999.
pub fn stem_bands(tokens: &[String]) -> Vec<String> {
    if tokens.len() < 4 {
        return vec![];
    }
    let mins: Vec<u32> = (0..STEM_BANDS * 2).map(|k| tokens.iter().map(|t| fnv(t, k as u32 + 1)).min().unwrap()).collect();
    (0..STEM_BANDS).map(|b| format!("{b}:{}:{}", base36(mins[2 * b]), base36(mins[2 * b + 1]))).collect()
}
pub fn jaccard(a: &[String], b: &[String]) -> f64 {
    let (sa, sb): (HashSet<&String>, HashSet<&String>) = (a.iter().collect(), b.iter().collect());
    let inter = sa.intersection(&sb).count();
    let uni = sa.len() + sb.len() - inter;
    if uni == 0 { 0.0 } else { inter as f64 / uni as f64 }
}

#[cfg(test)]
#[path = "qhash_tests.rs"]
mod tests;
