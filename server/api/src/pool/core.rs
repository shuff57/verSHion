//! The ONE adapter between the pool and share-core (worker/src/pool/core.ts). The Worker called share-core as wasm; here the
//! crate is linked natively and its `wasm::*_json` functions (bytes + a JSON options string in, one JSON object out) are called
//! directly, so the contract, the error shape and the "payload bytes are share-core's own" rule are unchanged.
//!
//! Payload JSON is passed on as the exact canonical bytes share-core wrote (sliced out of its result by `raw_field`, never
//! re-serialised). A panic inside share-core is the native form of a wasm trap: it fails THAT request as a plain 500, is
//! counted (flushed per hour, the hourly cron raises `wasm_trap` past `WASM_TRAPS_PER_HOUR`), and nothing of it is passed on.
use super::image::{ImageInfo, check_image};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use share_core::wasm as sc;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};

/// Traps an hour past which the admin is flagged (`wasm_trap`).
pub const WASM_TRAPS_PER_HOUR: u64 = 5;
static TRAPS: AtomicU64 = AtomicU64::new(0);
/// Traps counted since the last flush (the pool flushes them to the database).
pub fn take_wasm_traps() -> u64 {
    TRAPS.swap(0, Ordering::SeqCst)
}

/// Why a core call did not produce a value.
#[derive(Debug)]
pub enum CoreErr {
    /// share-core refused the input (an `ok: false` result).
    Fail(Box<Fail>),
    /// share-core panicked: answered as a plain 500 `server_error`.
    Trap,
    /// The server is not configured for this call (no pool hash key).
    Config(&'static str),
}
impl From<Fail> for CoreErr {
    fn from(f: Fail) -> Self {
        CoreErr::Fail(Box::new(f))
    }
}
pub type CoreRes<T> = Result<T, CoreErr>;

/// A share-core error, reduced to the fields that carry no input text.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CoreError {
    pub code: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset16: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack_allowed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// Author-facing messages for share-core codes and notes.
pub fn core_message(code: &str) -> Option<&'static str> {
    Some(match code {
        "labels_keep_refused" => "These answer labels give the answer away (the right column counts off one by one). Strip them instead of keeping them.",
        "placeholder_key" => "This copy was imported without its key. Set the True/False answer yourself: the placeholder value is not a key.",
        "symbol_font_char" => "This text contains a symbol-font character that cannot be converted. Retype it with a standard character.",
        "object_replacement" => "This text contains a placeholder for a pasted object (U+FFFC). Remove it.",
        "keyless_key_material" => "A question shared without its key cannot contain key material (a correct answer, a sample, an explanation or the Matching answers).",
        "symbol_font_mapped" => "Symbol-font characters were converted to standard Unicode.",
        "locked_key" => "Only the correct choice is locked, which would give the answer away. Lock none, or lock every choice whose place matters.",
        "key_in_stem" => "The question text (or a left item) gives the answer away (an answer line, GIFT or Cloze answer syntax, a True/False value, or a left item naming its right item). Remove it from the question.",
        "key_in_formatting" => "The correct choice looks different from the others (spacing, decoration, capitals or a note), which would give the answer away. Make the choices look alike.",
        "hash_key_required" => "The server is not configured to accept publishes yet.",
        "v2_field_in_v1" => "This file uses fields of a newer format. Update verSHion and export it again.",
        "keyless_needs_hide_key" => "This copy came from a version shared without its key. Publish it with the key hidden, or set the correct answers yourself before publishing it with the key shown.",
        _ => return None,
    })
}

/// A question (and optionally a Multipart part) by position in the payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pos {
    pub question: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelDecision {
    pub question: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<usize>,
    pub choice: String,
}

/// A share-core refusal.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fail {
    pub errors: Vec<CoreError>,
    /// Present when the problem is an undecided label: the author's preview (their own text).
    pub label_findings: Option<Vec<Value>>,
    pub needs_order_ack: Option<Vec<Pos>>,
    pub order_reasons: Option<Vec<Value>>,
    pub order_preview: Option<Vec<Value>>,
    pub formatting_findings: Option<Vec<Value>>,
    pub formatting_findings_total: Option<f64>,
}

/// A validated payload: the canonical bytes share-core wrote, and the same parsed (read-only use).
#[derive(Debug, Clone)]
pub struct Validated {
    pub bytes: Vec<u8>,
    pub payload: Value,
    pub dropped: Vec<Value>,
}
impl Validated {
    pub fn json(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

#[derive(Debug, Clone)]
pub struct Stripped {
    pub v: Validated,
    pub match_hashes: Vec<String>,
    pub stripped_hashes: Vec<String>,
    pub needs_order_ack: Vec<Pos>,
    pub order_reasons: Vec<Value>,
    pub labels_removed: Vec<Pos>,
    pub label_findings: Vec<Value>,
    pub served_orders: Vec<Value>,
    pub images_kept: Vec<usize>,
    pub images_removed: Vec<usize>,
    pub figures_removed: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct Prepared {
    pub full: Validated,
    pub match_hashes: Vec<String>,
    pub full_hashes: Vec<String>,
    pub stripped: Option<Stripped>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Meta {
    pub title: String,
    pub subject: String,
    pub course: String,
    pub grade: String,
    pub tags: Vec<String>,
    pub standards: Vec<String>,
}

/// What the author chose for a publish.
#[derive(Debug, Clone, Default)]
pub struct PublishOpts {
    pub hide_key: bool,
    pub labels: Vec<LabelDecision>,
    pub order_acks: Vec<Pos>,
    pub image_ok: Vec<usize>,
    pub ack_formatting: Vec<usize>,
}

#[derive(Debug)]
pub enum ImageCheck {
    Ok(ImageInfo),
    Refused(&'static str),
}

pub struct Core {
    /// The derived full-hash key (hex), or None (every hash is then refused).
    hash_key: Option<String>,
}

/// The raw JSON text of top-level `key` in a JSON object `json`, or None. A small scanner (strings and nesting only), so the
/// payload share-core wrote is reused byte for byte.
pub fn raw_field<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let want = serde_json::to_string(key).ok()?;
    let b = json.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    fn ws(b: &[u8], i: &mut usize) {
        while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\r' | b'\n') {
            *i += 1;
        }
    }
    fn skip_string(b: &[u8], i: &mut usize) -> Option<()> {
        *i += 1;
        while *i < b.len() {
            match b[*i] {
                b'\\' => *i += 2,
                b'"' => {
                    *i += 1;
                    return Some(());
                }
                _ => *i += 1,
            }
        }
        None
    }
    fn skip_value(b: &[u8], i: &mut usize) -> Option<()> {
        ws(b, i);
        match b.get(*i)? {
            b'"' => skip_string(b, i),
            b'{' | b'[' => {
                let mut depth = 0;
                while *i < b.len() {
                    match b[*i] {
                        b'"' => {
                            skip_string(b, i)?;
                            continue;
                        }
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth -= 1;
                            if depth == 0 {
                                *i += 1;
                                return Some(());
                            }
                        }
                        _ => {}
                    }
                    *i += 1;
                }
                None
            }
            _ => {
                while *i < b.len() && !matches!(b[*i], b',' | b'}' | b']') {
                    *i += 1;
                }
                Some(())
            }
        }
    }
    ws(b, &mut i);
    if b.get(i) != Some(&b'{') {
        return None;
    }
    i += 1;
    loop {
        ws(b, &mut i);
        if i >= n || b[i] == b'}' {
            return None;
        }
        let ks = i;
        skip_string(b, &mut i)?;
        let k = &json[ks..i];
        ws(b, &mut i);
        if b.get(i) != Some(&b':') {
            return None;
        }
        i += 1;
        ws(b, &mut i);
        let vs = i;
        skip_value(b, &mut i)?;
        if k == want {
            return Some(&json[vs..i]);
        }
        ws(b, &mut i);
        if b.get(i) == Some(&b',') {
            i += 1;
        } else {
            return None;
        }
    }
}

fn clean_errors(raw: Option<&Value>) -> Vec<CoreError> {
    let Some(arr) = raw.and_then(Value::as_array) else {
        return vec![CoreError { code: "share_core_error".into(), ..Default::default() }];
    };
    arr.iter()
        .take(100)
        .map(|e| {
            let x = e.as_object();
            let get = |k: &str| x.and_then(|m| m.get(k));
            let s = |k: &str| get(k).and_then(Value::as_str);
            let code = s("code").unwrap_or("invalid").to_string();
            let mut out = CoreError { path: s("path").unwrap_or("").to_string(), message: core_message(&code), code, ..Default::default() };
            out.limit = get("limit").and_then(Value::as_f64);
            out.offset16 = get("offset16").and_then(Value::as_f64);
            // `^U\+[0-9A-F]{4,6}$`
            out.cp = s("cp").filter(|c| c.strip_prefix("U+").is_some_and(|h| (4..=6).contains(&h.len()) && h.bytes().all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)))).map(String::from);
            out.reason = s("reason").filter(|r| (1..=64).contains(&r.len()) && r.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')).map(String::from);
            out.ack_allowed = get("ack_allowed").and_then(Value::as_bool);
            if get("author_only") == Some(&Value::Bool(true)) {
                out.author_only = Some(true);
            }
            // A short fixed English hint; anything that looks like more than that is not passed on.
            out.hint = s("hint").filter(|h| h.encode_utf16().count() <= 500).map(String::from);
            out
        })
        .collect()
}

fn fail(r: &Map<String, Value>) -> Fail {
    let arr = |k: &str, cap: usize| r.get(k).and_then(Value::as_array).map(|a| a.iter().take(cap).cloned().collect::<Vec<_>>());
    Fail {
        errors: clean_errors(r.get("errors")),
        label_findings: arr("label_findings", usize::MAX),
        needs_order_ack: r.get("needs_order_ack").and_then(|v| serde_json::from_value(v.clone()).ok()),
        order_reasons: arr("order_reasons", usize::MAX),
        // Refusals carry the author's previews inline (author only; bounded here).
        order_preview: arr("order_preview", 2000),
        formatting_findings: arr("formatting_findings", 200),
        formatting_findings_total: r.get("formatting_findings_total").and_then(Value::as_f64),
    }
}

fn labels_out(l: &[LabelDecision]) -> Value {
    serde_json::to_value(l).unwrap()
}

fn is_hash(v: &Value) -> bool {
    v.as_str().is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}
fn hash_list(r: &Map<String, Value>, k: &str) -> Option<Vec<String>> {
    let a = r.get(k)?.as_array()?;
    a.iter().all(is_hash).then(|| a.iter().map(|x| x.as_str().unwrap().to_string()).collect())
}

/// Runs one share-core export under the trap contract.
fn guard<R>(f: impl FnOnce() -> R) -> CoreRes<R> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => Ok(r),
        Err(_) => {
            sc::clear_author_data();
            TRAPS.fetch_add(1, Ordering::SeqCst);
            Err(CoreErr::Trap)
        }
    }
}

fn call(out: &str) -> Map<String, Value> {
    match serde_json::from_str::<Value>(out) {
        Ok(Value::Object(m)) => m,
        _ => panic!("share-core returned a non-object"),
    }
}

fn validated(out: &str, r: &Map<String, Value>) -> Validated {
    let json = raw_field(out, "payload").expect("share-core result has no payload");
    let dropped = r
        .get("dropped")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|d| {
                    // (share-core names a note by `reason`; older packages by `code`.)
                    let k = d.get("reason").and_then(Value::as_str).or_else(|| d.get("code").and_then(Value::as_str));
                    match (d.as_object(), k.and_then(core_message)) {
                        (Some(o), Some(m)) => {
                            let mut o = o.clone();
                            o.insert("message".into(), json!(m));
                            Value::Object(o)
                        }
                        _ => d.clone(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Validated { bytes: json.as_bytes().to_vec(), payload: r.get("payload").cloned().unwrap_or(Value::Null), dropped }
}

fn n_questions(p: &Value) -> usize {
    p.get("bank").and_then(|b| b.get("questions")).and_then(Value::as_array).map(Vec::len).unwrap_or(usize::from(p.get("question").is_some_and(|q| !q.is_null())))
}

fn typed<T: for<'a> Deserialize<'a>>(r: &Map<String, Value>, k: &str) -> Vec<T> {
    r.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| serde_json::from_value(x.clone()).ok()).collect()).unwrap_or_default()
}
fn arr(r: &Map<String, Value>, k: &str) -> Vec<Value> {
    r.get(k).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn stripped_of(out: &str, r: &Map<String, Value>, author_data: Option<Map<String, Value>>) -> Stripped {
    let v = validated(out, r);
    let served_orders = match author_data {
        Some(a) if a.get("ok") == Some(&Value::Bool(true)) && a.get("served_orders").is_some_and(Value::is_array) => arr(&a, "served_orders"),
        _ => match r.get("author_only").and_then(|a| a.get("served_orders")).and_then(Value::as_array) {
            Some(a) => a.clone(),
            None => arr(r, "served_orders"),
        },
    };
    let usize_list = |k: &str| typed::<usize>(r, k);
    let s = Stripped {
        match_hashes: typed(r, "match_hashes"),
        stripped_hashes: typed(r, "stripped_hashes"),
        needs_order_ack: typed(r, "needs_order_ack"),
        order_reasons: arr(r, "order_reasons"),
        labels_removed: typed(r, "labels_removed"),
        label_findings: arr(r, "label_findings"),
        served_orders,
        images_kept: usize_list("images_kept"),
        images_removed: usize_list("images_removed"),
        figures_removed: usize_list("figures_removed"),
        v,
    };
    let n = n_questions(&s.v.payload);
    assert!(s.match_hashes.len() == n && s.stripped_hashes.len() == n, "share-core strip hashes do not match the question count");
    s
}

fn opts_json(hide_key: Option<bool>, o: &PublishOpts, hash_key: Option<&str>) -> String {
    let mut m = Map::new();
    if let Some(h) = hide_key {
        m.insert("hide_key".into(), json!(h));
    }
    m.insert("labels".into(), labels_out(&o.labels));
    m.insert("ack_formatting".into(), json!(o.ack_formatting));
    if hide_key != Some(false) || hash_key.is_some() {
        m.insert("order_acks".into(), serde_json::to_value(&o.order_acks).unwrap());
        m.insert("image_ok".into(), json!(o.image_ok));
        // The client's `origin` is never served (the Worker owns provenance).
        m.insert("drop_client_provenance".into(), json!(true));
    }
    if let Some(k) = hash_key {
        m.insert("hash_key".into(), json!(k));
    }
    Value::Object(m).to_string()
}

impl Core {
    pub fn new(hash_key: Option<String>) -> Core {
        Core { hash_key }
    }

    /// (min, max, stripped) schema versions share-core accepts.
    pub fn schema(&self) -> CoreRes<(i64, i64, i64)> {
        let r = call(&guard(sc::schema_versions)?);
        let g = |k: &str| r.get(k).and_then(Value::as_i64).unwrap_or(0);
        Ok((g("min"), g("max"), g("stripped")))
    }

    /// For uploads: strict parse, limits, characters, key completeness, label decisions (hide_key).
    pub fn validate_for_publish(&self, bytes: &[u8], hide_key: bool, labels: &[LabelDecision], ack_formatting: &[usize]) -> CoreRes<Validated> {
        let o = json!({ "hide_key": hide_key, "labels": labels_out(labels), "ack_formatting": ack_formatting }).to_string();
        let out = guard(|| sc::validate_for_publish_json(bytes, &o))?;
        let r = call(&out);
        if r.get("ok") == Some(&Value::Bool(true)) { Ok(validated(&out, &r)) } else { Err(fail(&r).into()) }
    }

    /// Validates for a hide_key publish and strips (share-core draws the shuffle seed; client provenance dropped). The
    /// author-only data of the strip is taken in this same call, so two publishes never swap it.
    pub fn strip(&self, bytes: &[u8], o: &PublishOpts) -> CoreRes<Stripped> {
        let opts = opts_json(None, o, None);
        let (out, author) = guard(|| (sc::strip_json(bytes, &opts), sc::strip_author_data_json()))?;
        let r = call(&out);
        if r.get("ok") != Some(&Value::Bool(true)) {
            return Err(fail(&r).into());
        }
        Ok(stripped_of(&out, &r, serde_json::from_str::<Map<String, Value>>(&author).ok()))
    }

    /// The whole publish in one read and validation (validated full form, its match and keyed full hashes, and the stripped
    /// form when the key is hidden). No key, no hashing.
    pub fn publish_prepare(&self, bytes: &[u8], o: &PublishOpts) -> CoreRes<Prepared> {
        let key = self.hash_key.as_deref().ok_or(CoreErr::Config("pool hash key not configured (POOL_HASH_KEY)"))?;
        let opts = opts_json(Some(o.hide_key), o, Some(key));
        let hide = o.hide_key;
        let (out, author) = guard(|| (sc::publish_prepare_json(bytes, &opts), hide.then(sc::strip_author_data_json)))?;
        let r = call(&out);
        if r.get("ok") != Some(&Value::Bool(true)) {
            return Err(fail(&r).into());
        }
        let full = validated(&out, &r);
        let n = n_questions(&full.payload);
        let (match_hashes, full_hashes) = (hash_list(&r, "match_hashes"), hash_list(&r, "full_hashes"));
        let (Some(match_hashes), Some(full_hashes)) = (match_hashes, full_hashes) else { panic!("share-core publish_prepare hashes malformed") };
        assert!(match_hashes.len() == n && full_hashes.len() == n, "share-core publish_prepare hashes malformed");
        let mut stripped = None;
        if hide {
            let s_out = raw_field(&out, "stripped").expect("share-core publish_prepare has no stripped form");
            let s = r.get("stripped").and_then(Value::as_object).expect("share-core publish_prepare has no stripped form");
            let ad = author.and_then(|a| serde_json::from_str::<Map<String, Value>>(&a).ok());
            stripped = Some(stripped_of(s_out, s, ad));
        }
        Ok(Prepared { full, match_hashes, full_hashes, stripped })
    }

    /// match_hash per question (key-free; safe to compare across items).
    pub fn match_hashes(&self, bytes: &[u8]) -> CoreRes<Vec<String>> {
        let r = call(&guard(|| sc::match_hash_json(bytes, ""))?);
        hashes(&r)
    }

    /// full_hash per question (a key oracle for small answer spaces: author-only, never compared). No key, no hashing.
    pub fn full_hashes(&self, bytes: &[u8]) -> CoreRes<Vec<String>> {
        let key = self.hash_key.as_deref().ok_or(CoreErr::Config("pool hash key not configured (POOL_HASH_KEY)"))?;
        let o = json!({ "hash_key": key }).to_string();
        let r = call(&guard(|| sc::full_hash_json(bytes, &o))?);
        hashes(&r)
    }

    pub fn validate_meta(&self, meta: &Value) -> CoreRes<Meta> {
        let body = if meta.is_null() { "{}".to_string() } else { meta.to_string() };
        let r = call(&guard(|| sc::validate_meta_json(body.as_bytes(), ""))?);
        if r.get("ok") != Some(&Value::Bool(true)) {
            return Err(fail(&r).into());
        }
        let m = r.get("meta").cloned().unwrap_or(Value::Null);
        let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let l = |k: &str| m.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        Ok(Meta { title: s("title"), subject: s("subject"), course: s("course"), grade: s("grade"), tags: l("tags"), standards: l("standards") })
    }

    /// Keyed content hashes of questions, ids, dates and provenance aside (what "the same content" compares): HMAC-SHA256
    /// with the pool hash key over a canonical JSON.
    pub fn content_hashes(&self, qs: &[Value]) -> CoreRes<Vec<String>> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let key = self.hash_key.as_deref().ok_or(CoreErr::Config("pool hash key not configured (POOL_HASH_KEY)"))?;
        let raw: Vec<u8> = (0..key.len() / 2).filter_map(|i| u8::from_str_radix(&key[2 * i..2 * i + 2], 16).ok()).collect();
        Ok(qs
            .iter()
            .map(|q| {
                let mut m = Hmac::<Sha256>::new_from_slice(&raw).expect("HMAC takes any key length");
                m.update(format!("vershion/content/v2\n{}", canonical(&content_of(q))).as_bytes());
                super::sqlfrag::hex(&m.finalize().into_bytes())
            })
            .collect())
    }

    /// Structural image check (format sniffed from the bytes).
    pub fn image_info(&self, bytes: &[u8]) -> ImageCheck {
        match check_image(bytes) {
            Ok(i) => ImageCheck::Ok(i),
            Err(e) => ImageCheck::Refused(e),
        }
    }
}

fn hashes(r: &Map<String, Value>) -> CoreRes<Vec<String>> {
    if r.get("ok") != Some(&Value::Bool(true)) {
        return Err(fail(r).into());
    }
    Ok(hash_list(r, "hashes").expect("share-core hashes malformed"))
}

/// True when a core result is a share-core refusal.
pub fn is_fail<T>(r: &CoreRes<T>) -> bool {
    matches!(r, Err(CoreErr::Fail(_)))
}

/// The questions of a payload, in order.
pub fn questions_of(p: &Value) -> Vec<&Value> {
    if let Some(q) = p.get("bank").and_then(|b| b.get("questions")).and_then(Value::as_array) {
        return q.iter().collect();
    }
    match p.get("question") {
        Some(q) if !q.is_null() => vec![q],
        _ => vec![],
    }
}

/// The kind tag of a question (`"Mc"`, `"Tf"`, ...).
pub fn kind_tag(q: &Value) -> String {
    q.get("kind").and_then(Value::as_object).and_then(|k| k.keys().next()).cloned().unwrap_or_default()
}

/// A question with what is not content dropped (ids, times, provenance).
fn content_of(q: &Value) -> Value {
    let mut o = q.as_object().cloned().unwrap_or_default();
    for k in ["id", "created", "modified", "origin", "published"] {
        o.remove(k);
    }
    Value::Object(o)
}

/// JSON with object keys sorted by UTF-16 code units (the Worker's stable encoding for hashing).
pub fn canonical(v: &Value) -> String {
    match v {
        Value::Array(a) => format!("[{}]", a.iter().map(canonical).collect::<Vec<_>>().join(",")),
        Value::Object(o) => {
            let mut keys: Vec<String> = o.keys().cloned().collect();
            super::qjson::sort_utf16(&mut keys);
            format!("{{{}}}", keys.iter().map(|k| format!("{}:{}", serde_json::to_string(k).unwrap(), canonical(&o[k]))).collect::<Vec<_>>().join(","))
        }
        // (JSON.stringify prints numbers the JS way.)
        Value::Number(n) => n.as_f64().map(|f| if f.fract() == 0.0 && f.abs() < 1e21 { super::qjson::js_number(f) } else { n.to_string() }).unwrap_or_else(|| n.to_string()),
        other => other.to_string(),
    }
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
