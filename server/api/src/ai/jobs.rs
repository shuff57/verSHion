//! Import jobs: a document's pages converted to Ditto text in the background (port of worker/src/ai/jobs.ts).
//!
//! The server returns each page's raw model text, not a parsed document: parsing and picture cropping need
//! the page raster and live in the app's `ditto::import_text`, so there is one parser, not two. Pages run one
//! at a time in a spawned task; a job whose process was restarted mid-run shows as failed once it has not
//! moved for `STALE_MS`.
use super::credits::{DAY_MS, utc_day_start};
use super::policy::Outcome;
use super::request::{Content, Message, Part, data_uri_bytes};
use super::routes::AiState;
use super::store::{RESULT_D1_MAX, Usage};
use super::upstream::{ModelOut, Transport, call_model};
use serde_json::{Map, Value, json};
use std::sync::Arc;

pub const MAX_HINT_CHARS: usize = 500;
pub const STALE_MS: i64 = 10 * 60_000;
const MIMES: [&str; 3] = ["image/png", "image/jpeg", "image/webp"];

/// The prompt the app embeds too (`prompts/ditto-convert.md`): one file, two readers.
pub const DITTO_PROMPT: &str = include_str!("../../../../prompts/ditto-convert.md");

/// The prompt for one page, the user's note included.
pub fn page_prompt(hint: &str) -> String {
    let h = hint.trim();
    if h.is_empty() { DITTO_PROMPT.to_string() } else { format!("{DITTO_PROMPT}\n\nUser note about this source:\n{h}") }
}

/// A valid job: the note and each page as a data URI.
pub fn validate(body: &Map<String, Value>, max_pages: i64, max_image_bytes: usize) -> Result<(String, Vec<String>), String> {
    let hint = body.get("hint").and_then(Value::as_str).map(|h| h.trim().to_string()).unwrap_or_default();
    if hint.chars().count() > MAX_HINT_CHARS {
        return Err(format!("hint is longer than {MAX_HINT_CHARS} characters"));
    }
    let pages = match body.get("pages").and_then(Value::as_array) {
        Some(p) if !p.is_empty() => p,
        _ => return Err("pages must be a non-empty array".into()),
    };
    if pages.len() as i64 > max_pages {
        return Err(format!("more than {max_pages} pages"));
    }
    let mut images = Vec::with_capacity(pages.len());
    for (i, p) in pages.iter().enumerate() {
        let mime = p.get("mime").and_then(Value::as_str).filter(|m| MIMES.contains(m));
        let Some(mime) = mime else { return Err(format!("pages[{i}].mime must be png, jpeg or webp")) };
        let Some(b64) = p.get("b64").and_then(Value::as_str) else { return Err(format!("pages[{i}].b64 must be a base64 string")) };
        let url = format!("data:{mime};base64,{b64}");
        let Some(bytes) = data_uri_bytes(&url) else { return Err(format!("pages[{i}].b64 is not base64")) };
        if bytes > max_image_bytes {
            return Err(format!("pages[{i}] image is too large"));
        }
        images.push(url);
    }
    Ok((hint, images))
}

fn retry_after_midnight(now: i64) -> String {
    ((utc_day_start(now) + DAY_MS - now + 999) / 1000).to_string()
}

/// `POST /ai/import`: validates, applies the quotas, records the job, starts it, answers 202.
pub fn start_job<T: Transport + Send + Sync + 'static>(st: &Arc<AiState<T>>, body: &Map<String, Value>, principal: &str) -> Outcome {
    let (hint, images) = match validate(body, st.limits.max_pages, st.limits.max_image_bytes) {
        Ok(v) => v,
        Err(e) => return Outcome::err(400, "bad_request", &e),
    };
    let now = st.now();
    let id = st.store(|s| -> rusqlite::Result<Result<String, Outcome>> {
        if s.jobs_since(principal, now - DAY_MS)? >= st.limits.import_per_day {
            let mut o = Outcome::err(429, "rate_limited", "Too many imports today. Try again tomorrow.");
            o.headers.push(("Retry-After", retry_after_midnight(now)));
            return Ok(Err(o));
        }
        if s.cost_since(utc_day_start(now))? >= st.limits.daily_usd {
            let mut o = Outcome::err(429, "quota_exhausted", "The AI allowance for today is used up.");
            o.headers.push(("Retry-After", retry_after_midnight(now)));
            return Ok(Err(o));
        }
        let id = s.new_id()?;
        s.create_job(&id, principal, images.len() as i64, &hint, now)?;
        Ok(Ok(id))
    });
    let id = match id {
        Ok(Ok(id)) => id,
        Ok(Err(o)) => return o,
        Err(_) => {
            tracing::error!("ai: could not create a job");
            return Outcome::err(503, "quota_unavailable", "The AI service is not ready. Try again later.");
        }
    };
    (st.spawn)(Box::pin(run_job(st.clone(), id.clone(), principal.to_string(), hint, images)));
    Outcome::json(202, json!({ "job_id": id }))
}

fn pages_json(pages: &[(i64, Result<String, &'static str>)]) -> Value {
    json!({ "pages": pages.iter().map(|(n, r)| match r { Ok(t) => json!({ "page": n, "text": t }), Err(e) => json!({ "page": n, "error": e }) }).collect::<Vec<_>>() })
}

/// Rewrites the job row with the pages so far; a result above the row limit goes to the blob store instead.
fn save<T: Transport>(st: &AiState<T>, id: &str, status: &str, pages: &[(i64, Result<String, &'static str>)], error: Option<&str>) -> Result<(), ()> {
    let text = pages_json(pages).to_string();
    let stored = if text.len() > RESULT_D1_MAX {
        st.blobs.as_ref().ok_or(())?.put(&format!("ai/jobs/{id}/result.json"), text.as_bytes()).map_err(|_| ())?;
        None
    } else {
        Some(text)
    };
    st.store(|s| s.progress(id, status, pages.len() as i64, stored.as_deref(), error, st.now())).map_err(|_| ())
}

async fn run_job<T: Transport + Send + Sync + 'static>(st: Arc<AiState<T>>, id: String, principal: String, hint: String, images: Vec<String>) {
    let done = run_pages(&st, &id, &principal, &hint, images).await;
    if done.is_err() {
        // A store or blob failure: say so plainly, never the error's own text.
        let _ = st.store(|s| s.progress(&id, "failed", 0, None, Some("The job stopped unexpectedly."), st.now()));
    }
}

async fn run_pages<T: Transport + Send + Sync + 'static>(st: &Arc<AiState<T>>, id: &str, principal: &str, hint: &str, images: Vec<String>) -> Result<(), ()> {
    let Some(or) = st.openrouter.as_ref() else { return Err(()) };
    let mut out: Vec<(i64, Result<String, &'static str>)> = Vec::new();
    let mut ok = 0;
    let prompt = page_prompt(hint);
    st.store(|s| s.progress(id, "running", 0, None, None, st.now())).map_err(|_| ())?;
    for (i, url) in images.into_iter().enumerate() {
        let n = i as i64 + 1;
        let over = st.store(|s| s.cost_since(utc_day_start(st.now()))).map_err(|_| ())? >= st.limits.daily_usd;
        if over {
            out.push((n, Err("quota_exhausted")));
        } else {
            let messages = [Message { role: "user".into(), content: Content::Parts(vec![Part::Text(prompt.clone()), Part::Image(url)]) }];
            match call_model(&st.transport, or, &messages, st.limits.max_out_tokens, 0.0).await {
                ModelOut::Ok(r) => {
                    ok += 1;
                    st.store(|s| s.record(principal, &Usage { kind: "import_page", in_tokens: r.in_tokens, out_tokens: r.out_tokens, cost_usd: r.cost_usd, session: None }, st.now())).map_err(|_| ())?;
                    out.push((n, Ok(r.text)));
                }
                _ => out.push((n, Err("model_failed"))),
            }
        }
        save(st, id, "running", &out, None)?;
    }
    if ok > 0 { save(st, id, "done", &out, None) } else { save(st, id, "failed", &out, Some("No page could be read.")) }
}

fn is_job_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// `GET /ai/import/:id`: the job's state and the pages converted so far. Only its owner may read it.
pub fn read_job<T: Transport>(st: &AiState<T>, id: &str, principal: &str) -> Outcome {
    let row = if is_job_id(id) { st.store(|s| s.job(id, principal)) } else { Ok(None) };
    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => return Outcome::err(404, "not_found", "No such job."),
        Err(_) => return Outcome::err(503, "quota_unavailable", "The AI service is not ready. Try again later."),
    };
    let now = st.now();
    let (mut status, mut error) = (row.status.clone(), row.error.clone());
    if (status == "queued" || status == "running") && now - row.updated_at > STALE_MS {
        status = "failed".into();
        error = Some("The job was interrupted. Start it again.".into());
        let _ = st.store(|s| s.progress(id, "failed", row.pages_done, row.result.as_deref(), error.as_deref(), now));
    }
    let result = match &row.result {
        Some(r) => serde_json::from_str(r).unwrap_or(json!({ "pages": [] })),
        None if row.pages_done == 0 => json!({ "pages": [] }),
        None => st.blobs.as_ref().and_then(|b| b.get(&format!("ai/jobs/{}/result.json", row.id)).ok().flatten()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({ "pages": [] })),
    };
    Outcome::json(200, json!({ "status": status, "pages_done": row.pages_done, "pages_total": row.pages_total, "error": error, "result": result }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    fn body(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn the_page_prompt_is_the_apps_file_with_the_users_note() {
        assert_eq!(page_prompt("  "), DITTO_PROMPT);
        assert_eq!(page_prompt(" Quiz 3 "), format!("{DITTO_PROMPT}\n\nUser note about this source:\nQuiz 3"));
        assert!(DITTO_PROMPT.len() > 1000, "the real prompt file is embedded");
    }

    #[test]
    fn job_input_is_validated_with_the_workers_sentences() {
        let page = json!({ "mime": "image/png", "b64": PNG });
        let err = |v: Value| validate(&body(v), 2, 100).unwrap_err();
        assert_eq!(err(json!({ "pages": [] })), "pages must be a non-empty array");
        assert_eq!(err(json!({})), "pages must be a non-empty array");
        assert_eq!(err(json!({ "pages": [page, page, page] })), "more than 2 pages");
        assert_eq!(err(json!({ "pages": [{ "mime": "application/pdf", "b64": PNG }] })), "pages[0].mime must be png, jpeg or webp");
        assert_eq!(err(json!({ "pages": [{ "mime": "image/png" }] })), "pages[0].b64 must be a base64 string");
        assert_eq!(err(json!({ "pages": [{ "mime": "image/png", "b64": "not base64 !!" }] })), "pages[0].b64 is not base64");
        assert_eq!(err(json!({ "pages": [null] })), "pages[0].mime must be png, jpeg or webp");
        assert_eq!(err(json!({ "pages": [{ "mime": "image/png", "b64": "A".repeat(400) }] })), "pages[0] image is too large");
        assert_eq!(err(json!({ "hint": "x".repeat(501), "pages": [page] })), "hint is longer than 500 characters");
        let (hint, images) = validate(&body(json!({ "hint": "  Quiz 3 ", "pages": [page] })), 2, 1000).unwrap();
        assert_eq!((hint.as_str(), images), ("Quiz 3", vec![format!("data:image/png;base64,{PNG}")]));
    }
}
