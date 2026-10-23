//! The JSON surface the Worker calls.
//!
//! The functions here take UTF-8 **bytes** and are what native tests call. The wasm exports (in
//! `js`, wasm32 only) wrap them and take a `Uint8Array`: the Worker hands over the raw request
//! body (`new Uint8Array(await request.arrayBuffer())`), so nothing in JavaScript has parsed it
//! first (a `JSON.parse` / `JSON.stringify` round trip silently resolves duplicate keys, last one
//! winning) and no lone surrogate is turned into U+FFFD on the way in. Anything that is not a
//! `Uint8Array` (a JS string included) is refused with `expected_bytes`; invalid UTF-8 with
//! `invalid_utf8`. U+FFFD itself is an ordinary symbol and is accepted when it was really sent.
//!
//! Every function returns one JSON object and never throws for bad input:
//! `{"ok": true, ...}` or `{"ok": false, "errors": [{"code", "path", "limit"?, "offset"?, "offset16"?, "cp"?}, ...]}`
//! (plus `label_findings` when the problem is an undecided label). Errors never contain input text.
//!
//! `options` is `""` or a JSON object with any of: `limits` (a partial [`Limits`], clamped by
//! `Limits::sane`), `figures` (a [`Declared`] registry; absent = [`Structural`]), `image_ok`
//! (`strip`: question positions whose image stays; out of range is an error), `hide_key`
//! (`validate_for_publish`) and `labels` (`[{"question", "part"?, "choice": "strip"|"keep"}]`, the
//! author's label decisions).
//!
//! **The Worker never chooses the shuffle seed**: `strip` draws it inside the crate.
use crate::figures::{Declared, FigureRegistry, Structural};
use crate::hash::{canonical_json, full_hash, match_hash, write_payload};
use crate::limits::Limits;
use crate::strip::{LabelDecision, Pos, StripOptions, Stripped, label_findings};
use std::collections::BTreeSet;
use crate::validate::{Dropped, MAX_ERRORS, PublishOptions, Validated, ValidationError, validate_bytes, validate_for_publish, validate_meta};
use crate::wire::Payload;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Options {
    limits: Limits,
    figures: Option<Declared>,
    image_ok: Vec<usize>,
    hide_key: bool,
    labels: Vec<LabelDecision>,
    /// The author's acknowledgement of every preserved order (`strip`; see `needs_order_ack`).
    order_acks: Vec<Pos>,
    /// Questions whose low-risk formatting findings the author acknowledged as content.
    ack_formatting: Vec<usize>,
    /// `strip`: remove client-sent `origin` (the server writes its own).
    drop_client_provenance: bool,
    /// `full_hash`: the server's secret key (hex, at least 32 bytes).
    hash_key: Option<String>,
}

fn fail(errors: &[ValidationError]) -> String {
    let e = &errors[..errors.len().min(MAX_ERRORS)];
    if author_only(errors) {
        return canonical_json(&json!({ "ok": false, "errors": e, "author_only_refusal": true }));
    }
    canonical_json(&json!({ "ok": false, "errors": e }))
}

/// Every error is for the author only (the Worker filters those from what others see): the
/// refusal carries `author_only_refusal: true`, so a filtered list is not shown as an empty one
/// (round 13, critic24b N4).
fn author_only(errors: &[ValidationError]) -> bool {
    !errors.is_empty() && errors.iter().all(|e| e.author_only)
}

fn fail_code(code: &'static str) -> String {
    fail(&[ValidationError::new(code, "")])
}

fn options(s: &str) -> Result<Options, String> {
    if s.trim().is_empty() {
        return Ok(Options::default());
    }
    if s.len() > 1024 * 1024 {
        return Err(fail_code("bad_options"));
    }
    serde_json::from_str(s).map_err(|_| fail_code("bad_options"))
}

fn registry(o: &Options) -> &dyn FigureRegistry {
    match &o.figures {
        Some(d) => d,
        None => &Structural,
    }
}

/// One JSON object from pre-serialised fields (keys sorted), `payload` written in place: no
/// `serde_json::Value` tree of large lists is ever built.
fn object(payload: Option<&Payload>, mut fields: Vec<(&str, String)>) -> String {
    fields.sort_by(|a, b| a.0.cmp(b.0));
    let mut out = String::with_capacity(payload.map_or(0, |p| p.json_len()) + fields.iter().map(|f| f.1.len() + f.0.len() + 4).sum::<usize>() + 16);
    out.push('{');
    let mut first = true;
    let mut wrote_payload = payload.is_none();
    let put = |out: &mut String, first: &mut bool| {
        if let Some(p) = payload {
            if !*first {
                out.push(',');
            }
            *first = false;
            out.push_str("\"payload\":");
            write_payload(p, out);
        }
    };
    for (k, v) in &fields {
        if !wrote_payload && *k > "payload" {
            put(&mut out, &mut first);
            wrote_payload = true;
        }
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&Value::String(k.to_string()).to_string());
        out.push(':');
        out.push_str(v);
    }
    if !wrote_payload {
        put(&mut out, &mut first);
    }
    out.push('}');
    out
}

fn js<T: serde::Serialize + ?Sized>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

/// At most this many label findings are written into one response (`label_findings_total` says
/// how many there are): the app computes the full list locally with this crate.
pub const MAX_FINDINGS_OUT: usize = 2000;

fn findings_fields(f: &[crate::strip::LabelFinding]) -> Vec<(&'static str, String)> {
    let n = f.len().min(MAX_FINDINGS_OUT);
    vec![("label_findings", js(&f[..n])), ("label_findings_total", f.len().to_string())]
}

fn validated_output(v: &Validated) -> String {
    object(Some(&v.payload), vec![("dropped", js(&v.dropped)), ("ok", "true".into())])
}

/// Errors, plus the label findings for the author's preview when a decision is missing.
fn fail_with_findings(errors: &[ValidationError], p: Option<&Payload>) -> String {
    let e = &errors[..errors.len().min(MAX_ERRORS)];
    match p {
        Some(p) if errors.iter().any(|e| e.code.starts_with("labels_")) => {
            let mut f = findings_fields(&label_findings(p));
            f.push(("errors", js(e)));
            f.push(("ok", "false".into()));
            object(None, f)
        }
        _ => fail(errors),
    }
}

/// For payloads the client **received** → `{"ok": true, "payload", "dropped", "label_findings"}`.
pub fn validate_json(payload: &[u8], options_json: &str) -> String {
    let o = match options(options_json) {
        Ok(o) => o,
        Err(e) => return e,
    };
    match validate_bytes(payload, &o.limits, registry(&o)) {
        Ok(v) => validated_output(&v),
        Err(e) => fail(&e),
    }
}

/// For **uploads** (`options.hide_key`, `options.labels`).
pub fn validate_for_publish_json(payload: &[u8], options_json: &str) -> String {
    let o = match options(options_json) {
        Ok(o) => o,
        Err(e) => return e,
    };
    let opts = PublishOptions { hide_key: o.hide_key, labels: o.labels.clone(), ack_formatting: o.ack_formatting.clone() };
    match validate_bytes(payload, &o.limits, registry(&o)) {
        Ok(v) => {
            let errs = crate::validate::publish_errors(&v.payload, &opts);
            if errs.is_empty() { validated_output(&v) } else { fail_with_findings(&errs, Some(&v.payload)) }
        }
        Err(e) => fail(&e),
    }
}

fn stripped_output(dropped: Vec<Dropped>, s: Stripped, acks: &[Pos]) -> String {
    match stripped_result(dropped, s, acks) {
        Ok(o) | Err(o) => o,
    }
}

/// `strip`'s response: `Ok` with the served form, or `Err` (`order_ack_required`).
fn stripped_result(dropped: Vec<Dropped>, s: Stripped, acks: &[Pos]) -> Result<String, String> {
    // Order that is preserved must be acknowledged by the author, or nothing is published: the
    // Worker cannot "forget" to look at `needs_order_ack`.
    let acked: BTreeSet<Pos> = acks.iter().copied().collect();
    let missing: Vec<&Pos> = s.needs_order_ack.iter().filter(|p| !acked.contains(p)).collect();
    if !missing.is_empty() {
        let errs: Vec<ValidationError> = missing
            .iter()
            .take(MAX_ERRORS)
            .map(|p| {
                let path = match p.part {
                    Some(pp) => format!("order_acks[question={},part={pp}]", p.question),
                    None => format!("order_acks[question={}]", p.question),
                };
                ValidationError::new("order_ack_required", &path)
            })
            .collect();
        return Err(object(None, vec![("errors", js(&errs)), ("needs_order_ack", js(&s.needs_order_ack)), ("ok", "false".into()), ("order_reasons", js(&s.order_reasons))]));
    }
    // The author-only part never travels in this response: it waits for `strip_author_data`.
    AUTHOR_DATA.with(|a| *a.borrow_mut() = Some(s.author_only.author_json()));
    let qs = s.payload.questions();
    let fields = vec![
        ("needs_order_ack", js(&s.needs_order_ack)),
        ("order_reasons", js(&s.order_reasons)),
        ("labels_removed", js(&s.labels_removed)),
        // The decisions to store with the publish (no text: the previews were the author's).
        ("label_findings", js(&s.label_findings)),
        ("images_kept", js(&s.images_kept)),
        ("images_removed", js(&s.images_removed)),
        ("figures_removed", js(&s.figures_removed)),
        ("dropped", js(&dropped)),
        ("match_hashes", js(&qs.iter().map(match_hash).collect::<Vec<_>>())),
        ("stripped_hashes", js(&qs.iter().map(full_hash).collect::<Vec<_>>())),
        ("ok", "true".into()),
    ];
    Ok(object(Some(&s.payload), fields))
}

thread_local! {
    /// The last successful strip's author-only data, until `strip_author_data` takes it.
    static AUTHOR_DATA: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The author-only data of the last successful `strip` in this instance (`{"ok": true,
/// "served_orders": [...]}`), handed out once and then cleared; `no_author_data` otherwise. A
/// separate call, so a caller placing `strip`'s response in a public place cannot leak it. Put it
/// in the publishing author's response only.
pub fn strip_author_data_json() -> String {
    match AUTHOR_DATA.with(|a| a.borrow_mut().take()) {
        Some(j) => {
            let mut v: Value = serde_json::from_str(&j).unwrap_or(Value::Null);
            v["ok"] = Value::Bool(true);
            canonical_json(&v)
        }
        None => fail_code("no_author_data"),
    }
}

/// Forgets the last strip's author data.
pub fn clear_author_data() {
    AUTHOR_DATA.with(|a| *a.borrow_mut() = None);
}

fn strip_inner(payload: &[u8], options_json: &str, seed: Option<[u8; 32]>) -> String {
    clear_author_data();
    let o = match options(options_json) {
        Ok(o) => o,
        Err(e) => return e,
    };
    let opts = PublishOptions { hide_key: true, labels: o.labels.clone(), ack_formatting: o.ack_formatting.clone() };
    let v = match validate_for_publish(payload, &o.limits, registry(&o), &opts) {
        Ok(v) => v,
        Err(e) => {
            // Re-read only to attach the preview (cheap next to a refused publish).
            let p = e.iter().any(|x| x.code.starts_with("labels_")).then(|| validate_bytes(payload, &o.limits, registry(&o)).ok()).flatten();
            return fail_with_findings(&e, p.as_ref().map(|v| &v.payload));
        }
    };
    let Validated { payload, dropped } = v;
    let so = StripOptions { image_ok: o.image_ok, labels: o.labels, ack_formatting: o.ack_formatting, drop_client_provenance: o.drop_client_provenance, formatting_checked: true };
    let s = match seed {
        Some(seed) => crate::strip::strip_with_seed(payload, &so, seed),
        None => crate::strip::strip(payload, &so),
    };
    match s {
        Ok(s) => stripped_output(dropped, s, &o.order_acks),
        Err(e) => fail(&e),
    }
}

/// Validates for a `hide_key` publish (labels decided), then strips with a seed drawn inside the
/// crate from `crypto.getRandomValues` → `{"ok": true, "payload", "needs_order_ack", "order_reasons",
/// "labels_removed", "label_findings", "images_kept", "images_removed", "figures_removed",
/// "dropped", "match_hashes", "stripped_hashes"}`.
pub fn strip_json(payload: &[u8], options_json: &str) -> String {
    strip_inner(payload, options_json, None)
}

/// The Worker's whole publish in one call, reading and validating the payload once:
/// `validate_for_publish` (with `options.hide_key`), then, when `hide_key`, `strip` with the same
/// options. `{"ok": true, "payload", "dropped", "match_hashes", "full_hashes"?, "stripped"?}`:
/// `payload`/`dropped` are the validated full form, `match_hashes` its match hashes,
/// `full_hashes` its keyed hashes when `options.hash_key` is given, and `stripped` exactly what
/// `strip` returns on success (its `payload`, `needs_order_ack`, `order_reasons`, ...,
/// `match_hashes`, `stripped_hashes`). Any refusal is returned as by `validate_for_publish` or
/// `strip` (`ok: false`). The author-only data follows from `strip_author_data`.
pub fn publish_prepare_json(payload: &[u8], options_json: &str) -> String {
    clear_author_data();
    let o = match options(options_json) {
        Ok(o) => o,
        Err(e) => return e,
    };
    let key = match o.hash_key.as_deref() {
        None => None,
        Some(h) => match parse_hash_key(h) {
            Some(k) => Some(k),
            None => return fail_code("hash_key_required"),
        },
    };
    let opts = PublishOptions { hide_key: o.hide_key, labels: o.labels.clone(), ack_formatting: o.ack_formatting.clone() };
    // (`validate_for_publish` in two steps: a refusal reuses the parsed payload.)
    let v = match validate_bytes(payload, &o.limits, registry(&o)) {
        Ok(v) => v,
        Err(e) => return fail(&e),
    };
    let errors = crate::validate::publish_errors(&v.payload, &opts);
    if !errors.is_empty() {
        return refusal(&errors, &v.payload, &o);
    }
    let qs = v.payload.questions();
    let mut fields = vec![("dropped", js(&v.dropped)), ("match_hashes", js(&qs.iter().map(match_hash).collect::<Vec<_>>())), ("ok", "true".into())];
    if let Some(k) = &key {
        fields.push(("full_hashes", js(&qs.iter().map(|q| crate::hash::full_hash_keyed(q, k)).collect::<Vec<_>>())));
    }
    if o.hide_key {
        let so = StripOptions { image_ok: o.image_ok.clone(), labels: o.labels.clone(), ack_formatting: o.ack_formatting.clone(), drop_client_provenance: o.drop_client_provenance, formatting_checked: true };
        let st = match crate::strip::strip(v.payload.clone(), &so) {
            Ok(s) => stripped_result(v.dropped.clone(), s, &o.order_acks),
            Err(e) => return fail(&e),
        };
        match st {
            Ok(st) => fields.push(("stripped", st)),
            Err(e) => {
                clear_author_data();
                return e;
            }
        }
    }
    object(Some(&v.payload), fields)
}

/// A `publish_prepare` refusal of a payload that parsed: `{"ok": false, "errors", "label_findings"
/// (when a label decision is involved), "formatting_findings", "formatting_findings_total",
/// "order_preview"}`, so the Worker needs no second call.
fn refusal(errors: &[ValidationError], p: &crate::wire::Payload, o: &Options) -> String {
    // (Round 20, critic36a B36a-1: the findings and the order preview read each choice's labels
    // again and again; inside a work scope the readings are kept for the length of the call.)
    let _work = crate::format::WorkScope::new();
    let e = &errors[..errors.len().min(MAX_ERRORS)];
    let mut f = if errors.iter().any(|e| e.code.starts_with("labels_")) { findings_fields(&label_findings(p)) } else { Vec::new() };
    let ff = crate::strip::findings_from_errors(p, &o.labels, e);
    let total = ff.len();
    if author_only(errors) {
        f.push(("author_only_refusal", "true".into()));
    }
    f.push(("errors", js(e)));
    f.push(("formatting_findings", js(&ff[..total.min(MAX_FINDINGS_OUT)])));
    f.push(("formatting_findings_total", total.to_string()));
    f.push(("ok", "false".into()));
    f.push(("order_preview", js(&crate::strip::order_preview(p, &o.labels))));
    object(None, f)
}

/// `strip` with an explicit 32-byte seed: tests and fixtures only. Exported to wasm only when the
/// crate is built with the `test-seed` feature.
pub fn strip_with_seed_json(payload: &[u8], seed: &[u8], options_json: &str) -> String {
    let Ok(seed) = <[u8; 32]>::try_from(seed) else { return fail_code("bad_seed") };
    strip_inner(payload, options_json, Some(seed))
}

/// The label findings of a payload, for the author's preview before publishing.
pub fn label_findings_json(payload: &[u8], options_json: &str) -> String {
    // (A work scope for the call: the sibling-reference answers are kept for its length. Round 19.)
    let _work = crate::format::WorkScope::new();
    let o = match options(options_json) {
        Ok(o) => o,
        Err(e) => return e,
    };
    match validate_bytes(payload, &o.limits, registry(&o)) {
        Ok(v) => {
            let mut f = findings_fields(&label_findings(&v.payload));
            // What will move and what will not, before anything is published.
            f.push(("order_preview", js(&crate::strip::order_preview(&v.payload, &o.labels))));
            f.push(("ok", "true".into()));
            object(None, f)
        }
        Err(e) => fail(&e),
    }
}

/// `{"ok": true, "hashes": [<hex per question>]}`.
pub fn match_hash_json(payload: &[u8], options_json: &str) -> String {
    hashes(payload, options_json, match_hash)
}

/// `{"ok": true, "hashes": [<hex per question>]}`: `full_hash` keyed with `options.hash_key` (the
/// server's secret, hex, at least 32 bytes; `hash_key_required` otherwise). HMAC-SHA256.
pub fn full_hash_json(payload: &[u8], options_json: &str) -> String {
    let o = match options(options_json) {
        Ok(o) => o,
        Err(e) => return e,
    };
    let Some(key) = o.hash_key.as_deref().and_then(parse_hash_key) else { return fail_code("hash_key_required") };
    match validate_bytes(payload, &o.limits, registry(&o)) {
        Ok(v) => canonical_json(&json!({ "ok": true, "hashes": v.payload.questions().iter().map(|q| crate::hash::full_hash_keyed(q, &key)).collect::<Vec<_>>() })),
        Err(e) => fail(&e),
    }
}

/// A server secret given as hex: hex digits only (either case; no sign, no other characters, so
/// nothing is sliced inside a multibyte character), at least 32 bytes, not all zero.
pub fn parse_hash_key(h: &str) -> Option<Vec<u8>> {
    if h.len() < 64 || !h.len().is_multiple_of(2) || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v: Vec<u8> = h.as_bytes().chunks(2).map(|p| (hex_val(p[0]) << 4) | hex_val(p[1])).collect();
    (!v.iter().all(|b| *b == 0)).then_some(v)
}

fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        _ => b - b'A' + 10,
    }
}

/// The formatting findings of a payload, for the author's preview before publishing (labels as in
/// `options.labels`, Keep where undecided): every flagged choice with its feature type, risk,
/// whether `ack_formatting` may accept it, and the text as served; plus `order_preview`. One
/// question's findings never hide another's.
pub fn formatting_findings_json(payload: &[u8], options_json: &str) -> String {
    // (A work scope for the call: the sibling-reference answers are kept for its length. Round 19.)
    let _work = crate::format::WorkScope::new();
    let o = match options(options_json) {
        Ok(o) => o,
        Err(e) => return e,
    };
    match validate_bytes(payload, &o.limits, registry(&o)) {
        Ok(v) => {
            let f = crate::strip::formatting_findings(&v.payload, &o.labels);
            let total = f.len();
            object(None, vec![
                ("formatting_findings", js(&f[..total.min(MAX_FINDINGS_OUT)])),
                ("formatting_findings_total", total.to_string()),
                ("order_preview", js(&crate::strip::order_preview(&v.payload, &o.labels))),
                ("ok", "true".into()),
            ])
        }
        Err(e) => fail(&e),
    }
}

fn hashes(payload: &[u8], options_json: &str, f: fn(&crate::wire::Question) -> String) -> String {
    let o = match options(options_json) {
        Ok(o) => o,
        Err(e) => return e,
    };
    match validate_bytes(payload, &o.limits, registry(&o)) {
        Ok(v) => canonical_json(&json!({ "ok": true, "hashes": v.payload.questions().iter().map(f).collect::<Vec<_>>() })),
        Err(e) => fail(&e),
    }
}

/// `{"ok": true, "meta": {title, subject, course, grade, tags, standards}}`.
pub fn validate_meta_json(meta: &[u8], options_json: &str) -> String {
    let o = match options(options_json) {
        Ok(o) => o,
        Err(e) => return e,
    };
    let Ok(s) = std::str::from_utf8(meta) else { return fail_code("invalid_utf8") };
    match validate_meta(s, &o.limits) {
        Ok(m) => canonical_json(&json!({ "ok": true, "meta": m })),
        Err(e) => fail(&e),
    }
}

/// The crate's schema range and Unicode version.
pub fn schema_versions() -> String {
    canonical_json(&json!({
        "min": crate::wire::SCHEMA_MIN, "max": crate::wire::SCHEMA_MAX, "stripped": crate::wire::SCHEMA_STRIPPED,
        "unicode": format!("{}.{}.{}", crate::text::UNICODE_VERSION.0, crate::text::UNICODE_VERSION.1, crate::text::UNICODE_VERSION.2),
    }))
}

/// The `expected_bytes` refusal (what the wasm exports return for a non-`Uint8Array` payload).
pub fn expected_bytes() -> String {
    fail_code("expected_bytes")
}

/// The wasm exports: `Uint8Array` payloads, string options.
#[cfg(target_arch = "wasm32")]
mod js {
    use super::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;

    fn bytes(v: &JsValue) -> Option<Vec<u8>> {
        v.dyn_ref::<js_sys::Uint8Array>().map(|a| a.to_vec())
    }
    fn opts(v: &JsValue) -> Option<String> {
        if v.is_undefined() || v.is_null() {
            return Some(String::new());
        }
        v.as_string()
    }
    macro_rules! export {
        ($js:literal, $name:ident, $inner:path) => {
            #[wasm_bindgen(js_name = $js)]
            pub fn $name(payload: JsValue, options: JsValue) -> String {
                // Every entry forgets the last strip's author data: take it right after strip.
                super::clear_author_data();
                let Some(o) = opts(&options) else { return fail_code("bad_options") };
                match bytes(&payload) {
                    Some(b) => $inner(&b, &o),
                    None => expected_bytes(),
                }
            }
        };
    }
    export!("validate", validate, super::validate_json);
    export!("validate_for_publish", validate_for_publish, super::validate_for_publish_json);
    // `strip` clears the author data first, whatever then fails (options, bytes).
    #[wasm_bindgen(js_name = strip)]
    pub fn strip(payload: JsValue, options: JsValue) -> String {
        super::clear_author_data();
        let Some(o) = opts(&options) else { return fail_code("bad_options") };
        match bytes(&payload) {
            Some(b) => super::strip_json(&b, &o),
            None => expected_bytes(),
        }
    }
    #[wasm_bindgen(js_name = publish_prepare)]
    pub fn publish_prepare(payload: JsValue, options: JsValue) -> String {
        super::clear_author_data();
        let Some(o) = opts(&options) else { return fail_code("bad_options") };
        match bytes(&payload) {
            Some(b) => super::publish_prepare_json(&b, &o),
            None => expected_bytes(),
        }
    }
    export!("label_findings", label_findings, super::label_findings_json);
    export!("match_hash", match_hash, super::match_hash_json);
    export!("full_hash", full_hash, super::full_hash_json);
    export!("formatting_findings", formatting_findings, super::formatting_findings_json);

    #[wasm_bindgen(js_name = strip_author_data)]
    pub fn strip_author_data() -> String {
        super::strip_author_data_json()
    }
    export!("validate_meta", validate_meta, super::validate_meta_json);

    #[cfg(feature = "test-seed")]
    #[wasm_bindgen(js_name = strip_with_seed)]
    pub fn strip_with_seed(payload: JsValue, seed: &[u8], options: JsValue) -> String {
        let Some(o) = opts(&options) else { return fail_code("bad_options") };
        match bytes(&payload) {
            Some(b) => super::strip_with_seed_json(&b, seed, &o),
            None => expected_bytes(),
        }
    }

    #[wasm_bindgen(js_name = schema_versions)]
    pub fn schema_versions() -> String {
        super::clear_author_data();
        super::schema_versions()
    }
}
