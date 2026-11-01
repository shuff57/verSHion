//! share-core: the one implementation of the shared pool's payload rules, linked natively by the
//! app and compiled to wasm for the Worker (docs/shared-pool.md, "One share-core crate").
//!
//! * [`validate`]: strict parsing, the schema, the Limits table, character rules, figure params.
//! * [`strip`]: the stripped (keyless) form, shuffled once with a seed drawn inside the crate;
//!   [`labels`]: what the author must decide about choice labels first.
//! * [`import`]: the importer's conversion of a stripped question, and re-keying.
//! * [`match_hash`], [`full_hash`] / [`served_hash`], [`canonical_json`]: see [`hash`] for exactly
//!   what each hash covers.
//! * [`wasm`]: the same, JSON in and JSON out, for the Worker.
//! * [`argon2id`]: password hashing (phase 1).
pub mod figures;
pub mod format;
pub mod hash;
pub mod import;
pub mod json;
pub mod labels;
pub mod limits;
pub mod strip;
pub mod text;
pub mod validate;
pub mod wasm;
pub mod wire;

pub use figures::{Declared, FigureRegistry, ParamKind, ParamSpec, Structural};
pub use hash::{canonical_json, full_hash, match_hash, match_hash_with, match_skeleton, served_hash, write_payload, write_question};
pub use limits::Limits;
pub use labels::LabelChoice;
pub use strip::{LabelDecision, LabelFinding, OrderNote, OrderReason, Pos, QuestionLabels, ShuffleRng, StripOptions, Stripped, label_findings, question_labels, strip, strip_question, strip_with_seed, strip_without_shuffle};
pub use validate::{Dropped, Meta, PublishOptions, Validated, ValidationError, publish_errors, validate, validate_bytes, validate_for_publish, validate_meta};
pub use wire::*;

use argon2::{Algorithm, Argon2, Params, Version};
use wasm_bindgen::prelude::*;

/// Hash `password` with Argon2id; returns raw 32-byte tag. Salt is supplied by the caller.
#[wasm_bindgen]
pub fn argon2id(password: &[u8], salt: &[u8], m_kib: u32, t: u32, p: u32) -> Result<Vec<u8>, JsError> {
    wasm::clear_author_data();
    let params = Params::new(m_kib, t, p, Some(32)).map_err(|e| JsError::new(&e.to_string()))?;
    let a = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = vec![0u8; 32];
    a.hash_password_into(password, salt, &mut out).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(out)
}

/// A cheap self-check of the instance: validates a one-question payload and hashes a short
/// password with tiny Argon2id parameters, returning 1. A trap (a Rust panic: `RuntimeError:
/// unreachable`) leaves a wasm instance's memory in an unknown state, so after ANY exception from
/// ANY export the caller must call `reinit()` (appended to the generated `share_core.js` by
/// `scripts/build-wasm.sh`) before the next call; `health()` throwing or returning anything but 1
/// means the same (round 14, critic26b).
#[wasm_bindgen]
pub fn health() -> u32 {
    let ok_validate = wasm::validate_json(br#"{"schema_version":2,"question":{"id":"q00000001","stem":"s","kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"created":0,"modified":0}}"#, "").contains("\"ok\":true");
    let params = Params::new(8, 1, 1, Some(32)).ok();
    let ok_argon = params.is_some_and(|p| {
        let mut out = [0u8; 32];
        Argon2::new(Algorithm::Argon2id, Version::V0x13, p).hash_password_into(b"health", b"saltsalt", &mut out).is_ok() && out != [0u8; 32]
    });
    wasm::clear_author_data();
    u32::from(ok_validate && ok_argon)
}

/// Current wasm linear-memory size in bytes (peak is what matters; wasm memory never shrinks).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn memory_bytes() -> usize {
    wasm::clear_author_data();
    core::arch::wasm32::memory_size(0) * 65536
}
