//! Shared AI entry points for native and browser builds: Ditto conversion, figure tools,
//! handwriting and answer checks. Platform HTTP sits behind the common async engine.
//!
//! One conversion job at a time: `send_image` spawns a `std::thread` (egui never blocks)
//! and keeps the result channel in [`Ai`]; the Imports screen calls `poll` each frame and
//! the thread calls `ctx.request_repaint()` when it finishes, so the parsed questions show
//! up on the next frame. API keys come from, in order: a key saved in the app (`ai-keys.json`,
//! mode 0600, never in settings.json or backups), the provider's environment variable, and
//! (Ollama Cloud) the ollama-cloud tool's credentials file. They are read at request time
//! and never logged, echoed, put in errors or Debug output.

pub use crate::Settings;

use serde_json::{Value, json};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

mod engine;
mod operation;
mod source_scope;
mod directions;
mod figure_failure;
pub(crate) mod figure_observation;
pub use figure_failure::FigureFailure;
pub use directions::DirectionReviewTrace;
pub use source_scope::SourceBoxReviewTrace;
use source_scope::review_source_figure_boxes;
pub use operation::{OperationContext, Started};
pub use vershion_ai_core::{OutputMode, OutputSchema};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use pollster::block_on;

const OLLAMA_DEFAULT_BASE: &str = "http://127.0.0.1:11434";
const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";
/// Small local vision model that a laptop GPU can carry through a slow image round trip.
const OLLAMA_DEFAULT_MODEL: &str = "gemma4:31b";
/// OpenRouter default mirrors the local Ollama model on the free tier; set `ai_model` to override.
const OPENROUTER_DEFAULT_MODEL: &str = "google/gemma-4-26b-a4b-it:free";
/// Custom OpenAI-compatible endpoints default to the same model as local Ollama unless
/// `ai_model` says otherwise (the base URL decides what that model id actually means).
const CUSTOM_DEFAULT_MODEL: &str = "gemma4:31b";
/// Display model for legacy private Worker relays. Account AI is server-managed and
/// negotiates the current Rust relay protocol at service creation.
const WORKERS_DEFAULT_MODEL: &str = "@cf/google/gemma-4-26b-a4b-it";
const WORKERS_KEY_ENV: &str = "VERSHION_RELAY_KEY";
/// Images are slow, on small local GPUs especially.
const TIMEOUT: Duration = Duration::from_secs(300);

const OPENROUTER_KEY_ENV: &str = "OPENROUTER_API_KEY";
const CUSTOM_KEY_ENV: &str = "VERSHION_AI_KEY";
/// Ollama's hosted service: same OpenAI-compatible surface (`/v1`), keyed.
#[cfg(not(target_arch = "wasm32"))]
const OLLAMA_CLOUD_BASE: &str = "https://ollama.com/v1";
/// Browsers cannot call ollama.com directly (it sends no CORS headers). The web build talks to
/// the bundled relay instead, which forwards `/u/ollama.com/...` to `https://ollama.com/...`.
#[cfg(target_arch = "wasm32")]
const OLLAMA_CLOUD_RELAY_BASE: &str = "http://localhost:8189/u/ollama.com/v1";
const OLLAMA_CLOUD_DEFAULT_MODEL: &str = "gemma4:31b";
/// Ollama Cloud's key: `OLLAMA_API_KEY`, else the ollama-cloud tool's own credentials file.
const CLOUD_KEY_ENV: &str = "OLLAMA_API_KEY";
const CLOUD_KEY_FILE: &str = ".config/ollama-cloud/credentials.json";

/// The conversion prompt: one page in, one verSHion Ditto package (`docs/ditto-format.md`) out.
const TRANSCRIBE_PROMPT: &str = include_str!("../prompts/ditto-convert.md");

/// The instruction plus the user's typed note about the source, if any.
/// Public so the headless `VERSHION_CONVERT` path shares the UI's prompt.
pub fn prompt_for(hint: &str) -> String {
    transcription_prompt(hint)
}

fn transcription_prompt(hint: &str) -> String {
    let hint = hint.trim();
    if hint.is_empty() {
        return TRANSCRIBE_PROMPT.to_string();
    }
    format!("{TRANSCRIBE_PROMPT}\n\nUser note about this source:\n{hint}")
}

/// The conversion prompt for one page, with the directions earlier pages of the same document printed.
fn page_prompt(hint: &str, context: Option<&crate::ditto::DocContext>, page_no: usize) -> String {
    let base = transcription_prompt(hint);
    match context.map(|c| c.prompt_block(page_no)).filter(|b| !b.is_empty()) {
        Some(block) => format!("{base}\n\n{block}"),
        None => base,
    }
}

/// Ollama Cloud's base for this target: direct on the desktop, through the local relay in the
/// browser.
fn cloud_base() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        OLLAMA_CLOUD_RELAY_BASE.to_string()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        OLLAMA_CLOUD_BASE.to_string()
    }
}

/// The Ollama Cloud key note: the browser goes through the relay; the desktop can also read the
/// environment and the ollama-cloud credentials file.
fn cloud_key_help() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        "Ollama Cloud needs an API key. Requests go through the local relay: run \
         `node tools/relay.js` once and keep it running. The first request asks for local-network \
         access in your browser; allow it. The key stays in this browser."
            .to_string()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        format!(
            "Ollama Cloud needs an API key. A key saved here is used first, then the {CLOUD_KEY_ENV} environment variable, then ~/{CLOUD_KEY_FILE}."
        )
    }
}

#[derive(Clone, Debug)]
pub struct AiCfg {
    /// Normalized provider: "ollama" (the default), "openrouter", or "custom".
    pub provider: String,
    pub base: String,
    pub model: String,
    pub output_mode: OutputMode,
    pub output: Option<OutputSchema>,
    pub native_tools: bool,
    pub operation: Option<std::sync::Arc<OperationContext>>,
}

impl Default for AiCfg {
    fn default() -> Self {
        Self { provider: "ollama".into(), base: OLLAMA_DEFAULT_BASE.into(), model: OLLAMA_DEFAULT_MODEL.into(), output_mode: OutputMode::Json, output: None, native_tools: false, operation: None }
    }
}

impl AiCfg {
    pub fn with_output(&self, output: OutputSchema) -> Self {
        Self { output: Some(output), ..self.clone() }
    }
    pub fn from(s: &Settings) -> AiCfg {
        let provider = match s.ai_provider.trim().to_ascii_lowercase().as_str() {
            "openrouter" => "openrouter",
            "custom" => "custom",
            "workers" | "workers-ai" | "relay" => "workers",
            "account" => "account",
            "ollama-cloud" | "cloud" | "ollama cloud" => "cloud",
            // Empty or unknown settings fall back to local Ollama.
            _ => "ollama",
        };
        let model: String = if s.ai_model.trim().is_empty() {
            match provider {
                "openrouter" => OPENROUTER_DEFAULT_MODEL.to_string(),
                "custom" => CUSTOM_DEFAULT_MODEL.to_string(),
                "workers" => WORKERS_DEFAULT_MODEL.to_string(),
                "account" => "server-managed".to_string(),
                "cloud" => OLLAMA_CLOUD_DEFAULT_MODEL.to_string(),
                _ => OLLAMA_DEFAULT_MODEL.to_string(),
            }
        } else {
            s.ai_model.trim().to_string()
        };
        let base: String = match provider {
            "openrouter" => OPENROUTER_BASE.to_string(),
            "cloud" => cloud_base(),
            // Both the custom endpoint and the relay are whatever URL the user set.
            "custom" | "workers" => s.ai_base.trim().trim_end_matches('/').to_string(),
            // The hosted AI is the account server's own relay.
            "account" => account_base(),
            _ => {
                let own = s.ollama_base.trim().trim_end_matches('/');
                if own.starts_with("http://") || own.starts_with("https://") { own.to_string() } else { OLLAMA_DEFAULT_BASE.to_string() }
            }
        };
        AiCfg {
            output_mode: match s.ai_output_mode.as_str() { "schema" => OutputMode::Schema, "tool" => OutputMode::Tool, _ => OutputMode::Json },
            native_tools: s.ai_native_tools,
            provider: provider.to_string(), base, model, output: None, operation: None,
        }
    }

    /// The built-in relay, whether it is the one a person typed in or the account server's own.
    pub fn is_relay(&self) -> bool {
        self.provider == "workers" || self.provider == "account"
    }

    /// One human sentence about where the API key for this provider comes from.
    /// Order: a key saved in the app wins, then the environment variable, then (Ollama
    /// Cloud only) the ollama-cloud credentials file.
    pub fn key_source_help(&self) -> String {
        match self.provider.as_str() {
            "openrouter" => format!(
                "OpenRouter needs an API key. A key saved here is used first, otherwise the {OPENROUTER_KEY_ENV} environment variable."
            ),
            "cloud" => cloud_key_help(),
            "custom" => format!(
                "A custom endpoint may need an API key. A key saved here is used first, otherwise the {CUSTOM_KEY_ENV} environment variable."
            ),
            "account" => "The hosted AI runs on the verSHion server you signed in to. Each conversion costs credits from your account: 40 are free every week, and you can buy more on the Account screen.".to_string(),
            "workers" => format!(
                "A private relay keeps its AI provider key on the server. Use the relay key issued by its administrator (saved here first, otherwise {WORKERS_KEY_ENV}). The relay chooses the provider and model."
            ),
            _ => "Ollama runs on the local machine and needs no API key.".to_string(),
        }
    }

    /// True for providers that take an API key (everything but local Ollama).
    pub fn uses_key(&self) -> bool {
        self.key_env().is_some()
    }

    /// Where the key would come from right now, for the Settings status line.
    pub fn key_status(&self) -> KeySource {
        resolve_key(self, &load_keys(), &|v| std::env::var(v).ok(), home_dir().as_deref()).map(|(_, s)| s).unwrap_or(KeySource::None)
    }

    /// True when a request would carry a key (or the provider needs none). The relay's key is
    /// optional: a relay that is not publicly reachable (local `wrangler dev --remote`) needs none.
    pub fn has_key_or_needs_none(&self) -> bool {
        !self.uses_key() || self.provider == "custom" || self.provider == "workers" || self.key_status() != KeySource::None
    }

    fn key_env(&self) -> Option<&'static str> {
        match self.provider.as_str() {
            "openrouter" => Some(OPENROUTER_KEY_ENV),
            "custom" => Some(CUSTOM_KEY_ENV),
            "cloud" => Some(CLOUD_KEY_ENV),
            "workers" => Some(WORKERS_KEY_ENV),
            _ => None,
        }
    }
}

/// Where a resolved key came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySource {
    Saved,
    Env(&'static str),
    CredentialsFile,
    None,
}

impl KeySource {
    pub fn describe(self) -> String {
        match self {
            KeySource::Saved => "Key saved on this computer".to_string(),
            KeySource::Env(v) => format!("Using {v} from the environment"),
            KeySource::CredentialsFile => format!("Using ~/{CLOUD_KEY_FILE}"),
            KeySource::None => "No key".to_string(),
        }
    }
}

/// Provider -> key, as stored in `ai-keys.json`. `Debug` is deliberately redacted.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Keys(std::collections::BTreeMap<String, String>);

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Keys({} saved, redacted)", self.0.len())
    }
}

impl Keys {
    pub fn get(&self, provider: &str) -> Option<&str> {
        self.0.get(provider).map(String::as_str).filter(|k| !k.is_empty())
    }
}

const KEYS_FILE: &str = "ai-keys.json";

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

/// Browser keys live in memory for the page's lifetime: a page cannot write a protected file,
/// and a key must never land in IndexedDB/localStorage.
#[cfg(target_arch = "wasm32")]
thread_local! {
    static WEB_KEYS: std::cell::RefCell<Keys> = std::cell::RefCell::new(Keys::default());
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_keys() -> Keys {
    load_keys_in(&crate::store::store_dir())
}

#[cfg(target_arch = "wasm32")]
pub fn load_keys() -> Keys {
    WEB_KEYS.with(|k| k.borrow().clone())
}
pub fn load_keys_in(dir: &std::path::Path) -> Keys {
    std::fs::read(dir.join(KEYS_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Saves (`Some`) or removes (`None`) one provider's key.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_key(provider: &str, key: Option<&str>) -> Result<(), String> {
    set_key_in(&crate::store::store_dir(), provider, key)
}

#[cfg(target_arch = "wasm32")]
pub fn set_key(provider: &str, key: Option<&str>) -> Result<(), String> {
    WEB_KEYS.with(|keys| {
        let mut keys = keys.borrow_mut();
        match key.map(str::trim).filter(|k| !k.is_empty()) {
            Some(k) => {
                keys.0.insert(provider.to_string(), k.to_string());
            }
            None => {
                keys.0.remove(provider);
            }
        }
    });
    Ok(())
}

/// Atomic write (temp + rename) of the whole map, mode 0600 on unix from creation.
pub fn set_key_in(dir: &std::path::Path, provider: &str, key: Option<&str>) -> Result<(), String> {
    use std::io::Write as _;
    let mut keys = load_keys_in(dir);
    match key.map(str::trim).filter(|k| !k.is_empty()) {
        Some(k) => {
            keys.0.insert(provider.to_string(), k.to_string());
        }
        None => {
            keys.0.remove(provider);
        }
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't save the key: {}", e.kind()))?;
    let tmp = dir.join(format!("{KEYS_FILE}.tmp"));
    let path = dir.join(KEYS_FILE);
    let body = serde_json::to_vec(&keys).map_err(|_| "Couldn't save the key.".to_string())?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    let res = (|| -> std::io::Result<()> {
        let mut f = opts.open(&tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        f.write_all(&body)?;
        f.sync_all()?;
        std::fs::rename(&tmp, &path)
    })();
    res.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Couldn't save the key: {}", e.kind())
    })
}

/// Pure resolution: saved key, then env var, then (cloud) the credentials file under `home`.
fn resolve_key(
    cfg: &AiCfg,
    saved: &Keys,
    env: &dyn Fn(&str) -> Option<String>,
    home: Option<&std::path::Path>,
) -> Option<(String, KeySource)> {
    let var = cfg.key_env()?;
    if let Some(k) = saved.get(&cfg.provider) {
        return Some((k.to_string(), KeySource::Saved));
    }
    if let Some(k) = env(var).filter(|k| !k.is_empty()) {
        return Some((k, KeySource::Env(var)));
    }
    if cfg.provider == "cloud"
        && let Some(bytes) = home.and_then(|h| std::fs::read(h.join(CLOUD_KEY_FILE)).ok())
        && let Ok(v) = serde_json::from_slice::<Value>(&bytes)
        && let Some(k) = v.get("apiKey").and_then(Value::as_str)
        && !k.is_empty()
    {
        return Some((k.to_string(), KeySource::CredentialsFile));
    }
    None
}

/// The signed-in account the hosted AI is charged to: the server's relay address and a way to get a fresh access token.
/// The app keeps this current each frame; an account that is not signed in leaves it empty.
type TokenSource = std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>;
static ACCOUNT_AI: std::sync::Mutex<Option<(String, TokenSource)>> = std::sync::Mutex::new(None);

pub fn set_account_ai(account: Option<(String, TokenSource)>) {
    if let Ok(mut a) = ACCOUNT_AI.lock() {
        *a = account;
    }
}

/// True when a signed-in account is there to pay for the hosted AI.
pub fn account_ai_ready() -> bool {
    ACCOUNT_AI.lock().is_ok_and(|a| a.is_some())
}

fn account_base() -> String {
    ACCOUNT_AI.lock().ok().and_then(|a| a.as_ref().map(|(b, _)| format!("{}/ai", b.trim_end_matches('/')))).unwrap_or_default()
}

fn account_token() -> Option<String> {
    let src = ACCOUNT_AI.lock().ok().and_then(|a| a.as_ref().map(|(_, t)| t.clone()))?;
    src()
}

/// The API key for a request, resolved at request time. Never logged or echoed.
fn request_key(cfg: &AiCfg) -> Option<String> {
    if let Some(op) = &cfg.operation { return op.key(); }
    // The hosted AI is charged to the signed-in account, so it carries that account's access token.
    if cfg.provider == "account" {
        return account_token();
    }
    resolve_key(cfg, &load_keys(), &|v| std::env::var(v).ok(), home_dir().as_deref()).map(|(k, _)| k)
}

/// The listing/probe URL for a provider: Ollama's tags, the relay's health route, else `/models`.
fn list_url(cfg: &AiCfg, base: &str) -> String {
    match cfg.provider.as_str() {
        "ollama" => format!("{base}/api/tags"),
        // The relay serves one model and has no /models route; its health route proves it is up.
        "workers" | "account" => format!("{base}/health"),
        _ => format!("{base}/models"),
    }
}

/// Cheap authenticated probe for the Settings "Test connection" button: GET {base}/models.
/// Blocking; call from a thread. The reply never contains the key.
#[cfg(not(target_arch = "wasm32"))]
pub fn test_connection(cfg: &AiCfg) -> Result<(), String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = list_url(cfg, base);
    let mut req = agent().get(&url);
    if let Some(k) = request_key(cfg) {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let resp = req.call().map_err(|_| format!("Can't reach {base}."))?;
    match resp.status().as_u16() {
        200..=299 => Ok(()),
        401 | 403 => Err("Key rejected (401).".to_string()),
        404 => Err("Reached the server, but it has no /models endpoint (404). The key may still work.".to_string()),
        n => Err(format!("The server answered {n}.")),
    }
}

/// Browser probe: the same GET, awaited through `fetch`.
#[cfg(target_arch = "wasm32")]
pub async fn test_connection_async(cfg: &AiCfg) -> Result<(), String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = list_url(cfg, base);
    let status = fetch_status_async(&url, request_key(cfg).as_deref())
        .await
        .map_err(|e| cloud_error_hint(cfg, e))?;
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err("Key rejected (401).".to_string()),
        404 => Err("Reached the server, but it has no /models endpoint (404). The key may still work.".to_string()),
        n => Err(format!("The server answered {n}.")),
    }
}

/// Adds the relay instruction when a browser cannot reach Ollama Cloud.
fn cloud_error_hint(cfg: &AiCfg, err: String) -> String {
    if cfg!(target_arch = "wasm32") && cfg.provider == "cloud" {
        format!(
            "{err} If the relay is not running, start it once with `node tools/relay.js`; if the \
             browser asked for local-network access, choose Allow."
        )
    } else {
        err
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_status_async(url: &str, key: Option<&str>) -> Result<u16, String> {
    use wasm_bindgen::JsCast as _;
    let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let headers = web_sys::Headers::new().map_err(js)?;
    if let Some(k) = key {
        headers.set("Authorization", &format!("Bearer {k}")).map_err(js)?;
    }
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_mode(web_sys::RequestMode::Cors);
    init.set_headers(&headers);
    let signal = web_sys::AbortSignal::timeout_with_u32(30_000);
    init.set_signal(Some(&signal));
    let window = web_sys::window().ok_or("no window")?;
    let promise = window.fetch_with_str_and_init(url, &init);
    let resp = wasm_bindgen_futures::JsFuture::from(promise).await.map_err(|_| {
        format!("Can't reach {url}. The server may be off, or it does not allow requests from this page (CORS).")
    })?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "not a response".to_string())?;
    Ok(resp.status())
}

/// Model ids from a `/models` (OpenAI shape) or `/api/tags` (Ollama) reply.
pub fn parse_models(provider: &str, v: &Value) -> Vec<String> {
    let (rows, key) = if provider == "ollama" {
        (v["models"].as_array(), "name")
    } else {
        (v["data"].as_array(), "id")
    };
    let mut ids: Vec<String> = rows
        .into_iter()
        .flatten()
        .filter_map(|m| m.get(key).and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// The models the server lists. Blocking; call from a thread.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_models(cfg: &AiCfg) -> Result<Vec<String>, String> {
    // The relay serves exactly one model: nothing to list.
    if cfg.is_relay() {
        return Ok(vec![cfg.model.clone()]);
    }
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = list_url(cfg, base);
    let mut req = agent().get(&url);
    if let Some(k) = request_key(cfg) {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let mut resp = req.call().map_err(|_| format!("Can't reach {base}."))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().map_err(|e| format!("Bad reply from {base}: {e}"))?;
    if !(200..300).contains(&status) {
        return Err(format!("The server answered {status}."));
    }
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("{base} did not answer with JSON ({e})"))?;
    Ok(parse_models(&cfg.provider, &v))
}

/// Browser model list: the same GET, awaited through `fetch`.
#[cfg(target_arch = "wasm32")]
pub async fn list_models_async(cfg: &AiCfg) -> Result<Vec<String>, String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = list_url(cfg, base);
    let v = get_json_async(&url, request_key(cfg).as_deref())
        .await
        .map_err(|e| cloud_error_hint(cfg, e))?;
    Ok(parse_models(&cfg.provider, &v))
}

#[cfg(target_arch = "wasm32")]
async fn get_json_async(url: &str, key: Option<&str>) -> Result<Value, String> {
    use wasm_bindgen::JsCast as _;
    let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let headers = web_sys::Headers::new().map_err(js)?;
    if let Some(k) = key {
        headers.set("Authorization", &format!("Bearer {k}")).map_err(js)?;
    }
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_mode(web_sys::RequestMode::Cors);
    init.set_headers(&headers);
    let signal = web_sys::AbortSignal::timeout_with_u32(30_000);
    init.set_signal(Some(&signal));
    let window = web_sys::window().ok_or("no window")?;
    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str_and_init(url, &init))
        .await
        .map_err(|_| {
            format!("Can't reach {url}. The server may be off, or it does not allow requests from this page (CORS).")
        })?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "not a response".to_string())?;
    let status = resp.status();
    let text = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(js)?)
        .await
        .map_err(js)?
        .as_string()
        .unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(format!("{url} answered {status}."));
    }
    serde_json::from_str(&text).map_err(|e| format!("{url} did not answer with JSON ({e})"))
}

/// Pending work and a single batch session. Neither is persisted with the app's settings.
#[derive(Default)]
pub struct Ai {
    job: Option<Receiver<(AiCfg, Result<Converted,String>)>>,
    batch: Option<AiCfg>,
    batch_pages: usize,
    cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// The source document the next page belongs to, and what its earlier pages printed (document key, last page read).
    document: Option<String>,
    context: Option<(String, usize, crate::ditto::DocContext)>,
    reading: Option<(String, usize)>,
}
impl Ai {
    /// Names the source document of the next page (a PDF's pages share one), or `None` for a stand-alone photo.
    pub fn set_document(&mut self, document: Option<String>) { self.document = document; }
    pub fn start_batch(&mut self, pages: usize) {
        self.batch=None;
        self.batch_pages=pages;
    }
    pub fn end_batch(&mut self) { self.batch=None; self.batch_pages=0; self.context=None; self.reading=None; }
    pub fn cancel(&mut self) {
        if let Some(c)=self.cancel.take() { c.store(true,std::sync::atomic::Ordering::Relaxed); }
        if let Some(op)=self.batch.as_ref().and_then(|c|c.operation.as_ref()) { op.cancel(); }
        self.job=None;
        self.end_batch();
    }
    pub fn send_image(&mut self, bytes: Vec<u8>, mime: &str, hint: String, page_no: usize, cfg: &AiCfg, ctx: &egui::Context) {
        let (tx,rx)=channel(); self.job=Some(rx);
        let cfg=self.batch.clone().unwrap_or_else(||cfg.clone());
        let pages=self.batch_pages.max(1);
        let control=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.cancel=Some(control.clone());
        let mime=mime.to_string(); let ctx=ctx.clone();
        // Directions carry only from the immediately preceding page of the SAME document, never across documents.
        let document=self.document.clone();
        let context=document.as_ref().map(|key| match &self.context {
            Some((k,last,c)) if k==key && last+1==page_no => c.clone(),
            _ => crate::ditto::DocContext { document: key.clone(), directions: Vec::new() },
        });
        self.reading=document.map(|key|(key,page_no));
        let work=async move {
            let cfg=if cfg.operation.is_none() {
                match operation::start_controlled(&cfg,"import",pages,control).await {
                    Ok(s)=>s.cfg,
                    Err(e)=> { let _=tx.send((cfg,Err(e))); ctx.request_repaint(); return; }
                }
            } else { cfg };
            let result=convert_page_in_async(&bytes,&mime,&hint,&cfg,page_no,context.as_ref()).await;
            let _=tx.send((cfg,result)); ctx.request_repaint();
        };
        #[cfg(not(target_arch="wasm32"))]
        std::thread::spawn(move || block_on(work));
        #[cfg(target_arch="wasm32")]
        wasm_bindgen_futures::spawn_local(work);
    }
    pub fn poll(&mut self) -> Option<Result<Converted,String>> {
        let rx=self.job.as_ref()?;
        match rx.try_recv() {
            Ok((cfg,result))=> {
                self.job=None; self.cancel=None;
                match (&result,self.reading.take()) {
                    (Ok(c),Some((key,page)))=> {
                        let before=match &self.context { Some((k,last,c0)) if *k==key && last+1==page => c0.clone(), _=>crate::ditto::DocContext{document:key.clone(),directions:Vec::new()} };
                        self.context=Some((key,page,before.after_page(page,&c.items)));
                    },
                    // A page that failed leaves the document's earlier context where it was.
                    _=> {},
                }
                if self.batch_pages>0 && cfg.operation.is_some() { self.batch=Some(cfg); }
                Some(result)
            },
            Err(TryRecvError::Empty)=>None,
            Err(TryRecvError::Disconnected)=> {self.job=None; Some(Err("The AI job ended without a result.".into()))},
        }
    }
}

/// One round trip: Ollama's native `/api/chat` or an OpenAI-compatible `/chat/completions`.
/// Also the entry point for the headless `VERSHION_CONVERT` path.
pub fn transcribe(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str) -> Result<String, String> {
    transcribe_at(bytes, mime, cfg, prompt, 0.0)
}

fn transcribe_at(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str, temp: f64) -> Result<String, String> {
    match cfg.provider.as_str() {
        "ollama" => ollama_chat(cfg, prompt, &[bytes], temp),
        _ => openai_chat(cfg, prompt, mime, &[bytes], temp),
    }
}

/// A question to the model about one or more PNG images, answered with JSON.
pub fn ask_images(images: &[&[u8]], cfg: &AiCfg, prompt: &str, temp: f64) -> Result<String, String> {
    match cfg.provider.as_str() {
        "ollama" => ollama_chat(cfg, prompt, images, temp),
        _ => openai_chat(cfg, prompt, "image/png", images, temp),
    }
}

// ---------- Find models ----------
//
// "Find models" keeps what this account can use to read a picture. Ollama Cloud and custom
// endpoints: each listed model gets a tiny image and a one-word reply limit, and only the ones that
// answer stay (a free account is refused with 402 on paid models, text-only models refuse the
// image). Local Ollama is not pinged, since that would load every installed model; its own
// `capabilities` say who can see. OpenRouter lists its input modalities, so it is not pinged either.

/// One model a server lists. `vision` is `Some` when the listing itself says.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub vision: Option<bool>,
}

/// What Find models learned.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Checked {
    /// Models to offer, in the order listed.
    pub ok: Vec<String>,
    /// Models left out and why.
    pub left_out: Vec<(String, String)>,
    /// How many the server listed.
    pub total: usize,
}

/// Models past this many are not pinged one by one (a long public list would be slow and rude).
const PROBE_LIMIT: usize = 40;

/// Ids from a listing, with OpenRouter's `architecture.input_modalities` read as the vision flag.
pub fn parse_model_info(provider: &str, v: &Value) -> Vec<ModelInfo> {
    let modal = |id: &str| -> Option<bool> {
        let row = v["data"].as_array()?.iter().find(|m| m.get("id").and_then(Value::as_str) == Some(id))?;
        let mods = row.pointer("/architecture/input_modalities")?.as_array()?;
        Some(mods.iter().any(|m| m.as_str() == Some("image")))
    };
    parse_models(provider, v)
        .into_iter()
        .map(|id| {
            let vision = if provider == "openrouter" { modal(&id) } else { None };
            ModelInfo { id, vision }
        })
        .collect()
}

/// A small picture with something in it, for the ping.
fn probe_png() -> &'static [u8] {
    static P: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let mut img = image::RgbImage::from_pixel(64, 64, image::Rgb([255, 255, 255]));
        for y in 16..48 {
            for x in 16..48 {
                img.put_pixel(x, y, image::Rgb([40, 40, 40]));
            }
        }
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .expect("encode the probe picture");
        out
    })
}

/// The ping: the model's chat endpoint, one picture, a reply of a few tokens at most.
fn probe_request(cfg: &AiCfg, model: &str) -> (String, Value) {
    let base = cfg.base.trim_end_matches('/');
    let png = base64_encode(probe_png());
    let prompt = "Reply with the single word OK.";
    if cfg.provider == "ollama" {
        let body = json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt, "images": [png]}],
            "stream": false,
            "options": {"num_predict": 8},
        });
        (format!("{base}/api/chat"), body)
    } else {
        let body = json!({
            "model": model,
            "stream": false,
            "max_tokens": 8,
            "messages": [{"role": "user", "content": [
                {"type": "text", "text": prompt},
                {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{png}")}},
            ]}],
        });
        (format!("{base}/chat/completions"), body)
    }
}

/// A few words for why a ping failed, from `post_json`'s "<url> answered <status>: <message>".
pub fn why_not(err: &str) -> String {
    let (status, msg) = match err.split_once(" answered ") {
        Some((_, rest)) => match rest.split_once(": ") {
            Some((s, m)) => (s.trim().parse::<u16>().unwrap_or(0), m),
            None => (rest.trim().parse::<u16>().unwrap_or(0), ""),
        },
        None => (0, err),
    };
    let low = msg.to_ascii_lowercase();
    match status {
        401 => "key refused".to_string(),
        402 => "not on your plan".to_string(),
        403 if low.contains("rolled out") || low.contains("not yet available") => "not available yet".to_string(),
        403 => "not available to you".to_string(),
        400 if low.contains("image") => "text only".to_string(),
        400 => "not a chat model".to_string(),
        404 => "not found".to_string(),
        429 => "busy, try again".to_string(),
        0 => "no answer".to_string(),
        s => format!("error {s}"),
    }
}

/// Folds ping results into what to offer. `results` is in listing order.
fn assemble_checked(total: usize, results: Vec<(String, Result<(), String>)>) -> Checked {
    let mut c = Checked { total, ..Default::default() };
    for (id, r) in results {
        match r {
            Ok(()) => c.ok.push(id),
            Err(e) => c.left_out.push((id, why_not(&e))),
        }
    }
    c
}

/// The line under the picker: how many answered and what was left out.
pub fn checked_note(c: &Checked) -> String {
    let mut note = format!(
        "{} of {} models can read images with your account{}",
        c.ok.len(),
        c.total,
        if c.ok.len() == c.total { ". All are listed." } else { ". Only those are listed." }
    );
    if !c.left_out.is_empty() {
        let shown: Vec<String> = c.left_out.iter().take(4).map(|(id, why)| format!("{id} ({why})")).collect();
        note.push_str(&format!(" Left out: {}", shown.join(", ")));
        if c.left_out.len() > 4 {
            note.push_str(&format!(" and {} more", c.left_out.len() - 4));
        }
        note.push('.');
    }
    note
}

/// The answer to "Find models" when nothing could be kept (usually a wrong key or no connection):
/// the picker keeps its old list rather than going empty.
fn nothing_answered(total: usize, left_out: &[(String, String)]) -> String {
    let refused = left_out.iter().any(|(_, w)| w == "key refused");
    let tail = if refused { ", and the server refused the key. Check the API key." } else { ". Check the key and the connection." };
    format!("None of the {total} models answered{tail}")
}

#[cfg(not(target_arch = "wasm32"))]
fn list_models_json(cfg: &AiCfg) -> Result<Value, String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = list_url(cfg, base);
    let mut req = agent().get(&url);
    if let Some(k) = request_key(cfg) {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let mut resp = req.call().map_err(|_| format!("Can't reach {base}."))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().map_err(|e| format!("Bad reply from {base}: {e}"))?;
    if !(200..300).contains(&status) {
        return Err(format!("The server answered {status}."));
    }
    serde_json::from_str(&text).map_err(|e| format!("{base} did not answer with JSON ({e})"))
}

/// Local Ollama: does `/api/show` list `vision` among the model's capabilities? `None` when the
/// server is too old to say.
fn show_vision(shown: &Value) -> Option<bool> {
    let caps = shown.get("capabilities")?.as_array()?;
    Some(caps.iter().any(|c| c.as_str() == Some("vision")))
}

/// Find models: the listed models this account can use to read a picture. `progress(done, total)` is
/// called as the pings go. Blocking, so call it off the UI thread.
#[cfg(not(target_arch = "wasm32"))]
pub fn check_models(cfg: &AiCfg, progress: &dyn Fn(usize, usize)) -> Result<Checked, String> {
    // The relay serves one model chosen by the worker; there is nothing to list or ping.
    if cfg.is_relay() {
        return Ok(Checked { ok: vec![cfg.model.clone()], total: 1, ..Default::default() });
    }
    let listing = list_models_json(cfg)?;
    let infos = parse_model_info(&cfg.provider, &listing);
    let total = infos.len();
    if total == 0 {
        return Err("The server listed no models.".to_string());
    }
    match cfg.provider.as_str() {
        "openrouter" => Ok(checked_from_flags(total, infos.iter().map(|m| (m.id.clone(), m.vision)).collect())),
        "ollama" => {
            let base = cfg.base.trim_end_matches('/');
            let flags = infos
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    progress(i, total);
                    let shown = post_json(&format!("{base}/api/show"), json!({"model": m.id}), None).ok();
                    (m.id.clone(), shown.as_ref().and_then(show_vision))
                })
                .collect();
            Ok(checked_from_flags(total, flags))
        }
        _ if total > PROBE_LIMIT => Ok(checked_from_flags(total, infos.iter().map(|m| (m.id.clone(), None)).collect())),
        _ => {
            let key = request_key(cfg);
            let mut results = Vec::new();
            for (i, m) in infos.iter().enumerate() {
                progress(i, total);
                let (url, body) = probe_request(cfg, &m.id);
                results.push((m.id.clone(), post_json(&url, body, key.as_deref()).map(|_| ())));
            }
            let c = assemble_checked(total, results);
            if c.ok.is_empty() { Err(nothing_answered(total, &c.left_out)) } else { Ok(c) }
        }
    }
}

/// Listing flags into a [`Checked`]: a model the listing says cannot see is left out; one it does not
/// say anything about stays.
fn checked_from_flags(total: usize, flags: Vec<(String, Option<bool>)>) -> Checked {
    let mut c = Checked { total, ..Default::default() };
    for (id, v) in flags {
        match v {
            Some(false) => c.left_out.push((id, "text only".to_string())),
            _ => c.ok.push(id),
        }
    }
    c
}

#[cfg(target_arch = "wasm32")]
pub async fn check_models_async(cfg: &AiCfg, progress: &dyn Fn(usize, usize)) -> Result<Checked, String> {
    // The relay serves one model chosen by the worker; there is nothing to list or ping.
    if cfg.is_relay() {
        return Ok(Checked { ok: vec![cfg.model.clone()], total: 1, ..Default::default() });
    }
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set a base URL first.".to_string());
    }
    let url = list_url(cfg, base);
    let listing = get_json_async(&url, request_key(cfg).as_deref()).await.map_err(|e| cloud_error_hint(cfg, e))?;
    let infos = parse_model_info(&cfg.provider, &listing);
    let total = infos.len();
    if total == 0 {
        return Err("The server listed no models.".to_string());
    }
    match cfg.provider.as_str() {
        "openrouter" => Ok(checked_from_flags(total, infos.iter().map(|m| (m.id.clone(), m.vision)).collect())),
        "ollama" => {
            let mut flags = Vec::new();
            for (i, m) in infos.iter().enumerate() {
                progress(i, total);
                let shown = post_json_async(&format!("{base}/api/show"), &json!({"model": m.id}), None).await.ok();
                flags.push((m.id.clone(), shown.as_ref().and_then(show_vision)));
            }
            Ok(checked_from_flags(total, flags))
        }
        _ if total > PROBE_LIMIT => Ok(checked_from_flags(total, infos.iter().map(|m| (m.id.clone(), None)).collect())),
        _ => {
            let key = request_key(cfg);
            let mut results = Vec::new();
            for (i, m) in infos.iter().enumerate() {
                progress(i, total);
                let (url, body) = probe_request(cfg, &m.id);
                results.push((m.id.clone(), post_json_async(&url, &body, key.as_deref()).await.map(|_| ())));
            }
            let c = assemble_checked(total, results);
            if c.ok.is_empty() { Err(nothing_answered(total, &c.left_out)) } else { Ok(c) }
        }
    }
}

/// One message of a multi-turn chat (the figure agent's loop). `role` is "system", "user" or
/// "assistant"; `images` are PNG bytes shown to the model alongside `text`.
#[derive(Clone, Debug)]
pub struct Turn {
    pub role: &'static str,
    pub text: String,
    pub images: Vec<Vec<u8>>,
    pub mime: String,
}

impl Turn {
    pub fn new(role: &'static str, text: impl Into<String>) -> Turn {
        Turn { role, text: text.into(), images: Vec::new(), mime: "image/png".into() }
    }
}

/// A whole conversation in, the model's JSON reply out. Same providers and keys as [`transcribe`].
pub fn chat_turns(cfg: &AiCfg, turns: &[Turn], temp: f64) -> Result<String, String> {
    #[cfg(not(target_arch = "wasm32"))]
    { block_on(chat_turns_async(cfg, turns, temp)) }
    #[cfg(target_arch = "wasm32")]
    { let _ = (cfg, turns, temp); Err("Use the async AI API in the browser.".into()) }
}

pub(crate) async fn figure_turn_async(cfg: &AiCfg, turns: &[Turn], saved: &mut std::collections::BTreeMap<usize,Value>) -> Result<String,String> {
    engine::figure(cfg,turns,saved).await
}

pub(crate) async fn figure_turn_traced_async(cfg: &AiCfg, turns: &[Turn], saved: &mut std::collections::BTreeMap<usize,Value>, provider_reply: &mut Option<Value>) -> Result<String,String> {
    engine::figure_traced(cfg,turns,saved,provider_reply).await
}

/// The selected model reply is captured before output-envelope or schema validation.
/// This trace excludes HTTP headers, credentials and image data.
pub(crate) async fn chat_message_traced_async(cfg: &AiCfg, turns: &[Turn], temp: f64, provider_reply: &mut Option<Value>) -> Result<Value,String> {
    engine::chat_message_traced(cfg,turns,temp,provider_reply).await
}

pub(crate) async fn chat_turns_traced_async(cfg: &AiCfg, turns: &[Turn], temp: f64, provider_reply: &mut Option<Value>) -> Result<String,String> {
    engine::chat_traced(cfg,turns,temp,provider_reply).await
}

pub async fn chat_turns_async(cfg: &AiCfg, turns: &[Turn], temp: f64) -> Result<String, String> {
    engine::chat(cfg, turns, temp).await
}

/// What one converted page yields.
#[derive(Debug)]
pub struct Converted {
    pub doc: crate::formats::ImportDoc,
    /// Skipped questions, dropped figures, figures kept as plain images.
    pub warnings: Vec<String>,
    /// Pictures cropped from the page, and how many of those became editable Figures-tab figures.
    pub cropped: usize,
    pub redrawn: usize,
    /// Source reply and figure execution evidence for isolated import evaluations.
    pub raw_reply: String,
    pub figure_runs: Vec<FigureImportTrace>,
    /// One entry per raw item of the reply (dropped ones included), keyed to this page's `doc.questions`.
    pub items: Vec<crate::ditto::SourceItem>,
    /// The earlier-page directions this page was read with; none when it was read on its own.
    pub context: Option<crate::ditto::DocContext>,
    /// The full-page/crop scope check, separate from a figure model judging its own redraw.
    pub source_box_review: Option<SourceBoxReviewTrace>,
    pub direction_review: Option<DirectionReviewTrace>,
}

fn source_review_contact_sheet(previews: &[(usize, String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    if previews.is_empty() { return Err("No readable source crops for a contact sheet.".into()); }
    let columns = previews.len().min(3);
    let rows = previews.len().div_ceil(columns);
    let (cell_w, cell_h) = (420.0, 360.0);
    let mut doc = crate::figures::svg::Doc::new(columns as f64 * cell_w, rows as f64 * cell_h);
    doc.rect(0.0, 0.0, columns as f64 * cell_w, rows as f64 * cell_h, "#fff", "none", 0.0);
    let mut tiles = Vec::new();
    for (tile, (_, label, png)) in previews.iter().enumerate() {
        let image = image::load_from_memory(png).map_err(|e| format!("Source crop could not be decoded: {e}"))?;
        let scale = (400.0 / image.width().max(1) as f64).min(310.0 / image.height().max(1) as f64);
        let (w, h) = (image.width() as f64 * scale, image.height() as f64 * scale);
        let (left, top) = ((tile % columns) as f64 * cell_w, (tile / columns) as f64 * cell_h);
        doc.text(left + 10.0, top + 25.0, 18.0, "start", &format!("{}. Source question {label}", tile + 1));
        tiles.push((image, w.round().max(1.0) as u32, h.round().max(1.0) as u32,
            (left + (cell_w - w) / 2.0).round() as i64, (top + 40.0 + (310.0 - h) / 2.0).round() as i64));
    }
    let captions = crate::figures::to_png(&doc.finish(), 1.0).map_err(|e| format!("Source crop captions could not be rendered: {e}"))?;
    let mut canvas = image::load_from_memory(&captions).map_err(|e| format!("Source crop canvas could not be decoded: {e}"))?.to_rgba8();
    // The SVG rasterizer intentionally does not resolve embedded raster hrefs. Compose the
    // actual source pixels directly, so the model never receives an apparently valid blank sheet.
    for (image, w, h, x, y) in tiles {
        let tile = image.resize_exact(w, h, image::imageops::FilterType::Lanczos3).to_rgba8();
        image::imageops::overlay(&mut canvas, &tile, x, y);
    }
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(canvas).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("Source crop contact sheet could not be encoded: {e}"))?;
    Ok(png)
}

#[derive(Debug, serde::Serialize)]
pub struct FigureImportTrace {
    pub executions:Vec<crate::figure_agent::ExecutionTrace>,
    pub question_index: usize,
    /// Set when the run redrew one of the question's answer choices rather than its own picture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choice: Option<ChoiceRef>,
    pub sees: Option<String>,
    pub steps: Vec<crate::figure_agent::StepLog>,
    pub spec: Option<crate::model::FigureSpec>,
    pub review: Option<crate::figure_agent::Verdict>,
    pub accepted: bool,
    pub note: String,
    pub error: Option<String>,
    pub failure: Option<FigureFailure>,
}

fn figure_import_verified(run: &crate::figure_agent::AgentRun) -> bool {
    run.error.is_none()
        && run.declined.is_none()
        && run.spec.is_some()
        && run.png.is_some()
        && run.review.as_ref().is_some_and(|v| v.matches)
        && run.steps.iter().any(|s| s.tool == "render_figure")
}

/// Blocking wrapper around the common page workflow; the UI calls it on a worker thread.
pub fn convert_page(bytes: &[u8], mime: &str, hint: &str, cfg: &AiCfg, page_no: usize) -> Result<Converted, String> {
    #[cfg(not(target_arch = "wasm32"))]
    { block_on(convert_page_async(bytes, mime, hint, cfg, page_no)) }
    #[cfg(target_arch = "wasm32")]
    { let _ = (bytes,mime,hint,cfg,page_no); Err("Use the async conversion API in the browser.".into()) }
}

pub fn convert_page_chat(bytes: &[u8], mime: &str, hint: &str, cfg: &AiCfg, page_no: usize) -> Result<Converted, String> {
    #[cfg(not(target_arch = "wasm32"))]
    { block_on(convert_page_chat_async(bytes, mime, hint, cfg, page_no)) }
    #[cfg(target_arch = "wasm32")]
    { let _ = (bytes,mime,hint,cfg,page_no); Err("Use the async conversion API in the browser.".into()) }
}

/// Pages read at the same time. Each page is its own chat request (about 20 s on the relay), so a
/// document takes roughly the time of its slowest page plus one more per `PAGE_PARALLEL` pages.
/// `VERSHION_AI_PARALLEL` overrides it (used to measure the best value).
#[cfg(not(target_arch = "wasm32"))]
fn page_parallel() -> usize {
    crate::env_var("VERSHION_AI_PARALLEL").ok().and_then(|v| v.trim().parse::<usize>().ok()).filter(|n| *n > 0).unwrap_or(PAGE_PARALLEL)
}
#[cfg(not(target_arch = "wasm32"))]
const PAGE_PARALLEL: usize = 12;

/// Every page of a document, a few at a time. Each page is one chat request, not an import job: the
/// relay's background job was cut off after about a page (the platform cancels work that outlives
/// the request that started it), and one request per page needs no polling. `progress(done, total)`
/// reports pages as they finish; results come back in page order. A page's figure cropping and
/// fitting run on the same worker thread right after its read.
#[cfg(not(target_arch = "wasm32"))]
pub fn convert_pages(pages: &[Vec<u8>], hint: &str, cfg: &AiCfg, progress: &(dyn Fn(usize, usize) + Sync)) -> Vec<Result<Converted, String>> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let total = pages.len();
    let started = match begin_service(cfg, "import", total) {
        Ok(s) => s,
        Err(e) => return (0..total).map(|_| Err(e.clone())).collect(),
    };
    let cfg = &started.cfg;
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<Result<Converted, String>>>> = (0..total).map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|sc| {
        for _ in 0..page_parallel().min(total) {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(png) = pages.get(i) else { break };
                    let r = convert_page_chat(png, "image/png", hint, cfg, i + 1);
                    *slots[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
                    progress(done.fetch_add(1, Ordering::SeqCst) + 1, total);
                }
            });
        }
    });
    slots.into_iter().map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner()).unwrap_or_else(|| Err("page was not read".into()))).collect()
}

/// The request that asks the figure agent to redraw one cropped picture. The question text is context only.
fn figure_request(stem: &str, rejected: &str) -> String {
    let context = if stem.trim().is_empty() || stem == crate::ditto::UNRESOLVED_TASK { "(the task is not visible on the page)".to_string() } else { stem.to_string() };
    let mut s = format!("Recreate only the printed figure in this worksheet crop. The question text below is context only and is NOT part of the picture: never use any of it as a title, label or caption unless that exact text is visibly printed inside the crop. Question context: {context}\n\nPreserve every given label, value, curve, point and scale exactly as printed, and keep blank graph axes blank. Do not solve the question or add answers, solution shading, highlights or a title the crop does not show; leave any answer, solution or title option off unless the crop shows it. Use a registered figure generator and render it before finishing; decline if it cannot faithfully represent the source.");
    if !rejected.is_empty() {
        s.push_str(&format!("\n\nYour previous figure was rejected: {rejected}. Redraw it from what is printed in the crop."));
    }
    s
}

fn figure_request_for_item(stem: &str, rejected: &str, printed_label: Option<&str>) -> String {
    let mut request = figure_request(stem, rejected);
    if let Some(marker) = source_item_marker(printed_label) {
        request.push_str(&format!("\nApplication-owned source identification: {marker} is the external worksheet item number, not a diagram label. A padded crop or unverified source observation may include it; do not print that item marker in the recreated figure. Preserve actual dimensions, variables and point names."));
    }
    request.push_str("\nFor supplied scatter plots preserve marker size relative to grid spacing and visible mark-to-mark and mark-to-axis touching/overlap, as well as plotted positions. Do not move data points to compensate for marker sizes or frame proportions.");
    request.push_str("\nCount the tick marks on each axis of the crop, including unlabelled minor ticks between numbered ones, and make the recreated axis show the same number of ticks at the same spacing; numbered labels alone do not fix the tick count.");
    request.push_str("\nWhen source scale numbers are printed directly over gridlines without white backing, coordinate-grid's number_halo=false can preserve that grid/label contact. Read each sign literally; a nearby grid stub is not an extra minus sign. Use the option only to match the source, not to change supplied tick values.");
    request
}

fn source_item_marker(label: Option<&str>) -> Option<String> {
    let number = label?.trim().trim_matches(|c: char| c == '(' || c == ')' || c == '.');
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) { return None; }
    Some(format!("{})", number.parse::<u64>().ok()?))
}

/// A whole external item token in displayed text, not an internal node ID or dimension.
fn copied_source_item_marker(spec: &crate::model::FigureSpec, printed_label: Option<&str>) -> Option<String> {
    fn visit(value: &Value, marker: &str) -> bool {
        match value {
            Value::Array(values) => values.iter().any(|v| visit(v, marker)),
            Value::Object(fields) => fields.iter().any(|(key, value)| {
                let visible = !fields.get(&format!("{key}_mode")).is_some_and(|m| m == "None");
                let text_field = matches!(key.as_str(), "text" | "label" | "title" | "caption")
                    || key.starts_with("name_") || key.ends_with("label");
                if visible && text_field && value.as_str().is_some_and(|s| plain(s) == marker) { return true; }
                if key == "diagram" && let Some(s) = value.as_str()
                    && let Ok(diagram) = serde_json::from_str::<Value>(s) { return visit(&diagram, marker); }
                (value.is_object() || value.is_array()) && visit(value, marker)
            }),
            _ => false,
        }
    }
    let marker = source_item_marker(printed_label)?;
    visit(&json!(spec.params), &marker).then(|| format!("a printed label {marker:?} copies the external worksheet item number rather than a figure given"))
}

fn plain(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace() && *c != '$').map(|c| if c == '\u{2212}' || c == '\u{2013}' { '-' } else { c }).collect::<String>().to_lowercase()
}

/// A visible figure title or caption that repeats a line of the question's own text, which a crop that
/// reached into the question produces. A legitimate printed title is unlikely to equal a stem line.
pub(crate) fn copied_question_text(spec: &crate::model::FigureSpec, stem: &str) -> Option<String> {
    let lines: Vec<String> = stem.lines().map(plain).filter(|l| l.chars().count() >= 3).collect();
    for (key, value) in &spec.params {
        let lower = key.to_lowercase();
        if !(lower.contains("title") || lower.contains("caption")) || lower.ends_with("_mode") {
            continue;
        }
        if spec.params.get(&format!("{key}_mode")).and_then(Value::as_str).is_some_and(|m| m == "None") {
            continue;
        }
        let Some(text) = value.as_str().map(str::trim).filter(|t| !t.is_empty()) else { continue };
        if lines.contains(&plain(text)) {
            return Some(format!("its {key} \"{text}\" copies a line of the question text, which is not printed in the picture"));
        }
    }
    None
}

fn record_figure_execution(run: &mut crate::figure_agent::AgentRun, stem: &str, printed_label: Option<&str>, executions: &mut Vec<crate::figure_agent::ExecutionTrace>) -> Option<String> {
    let rejection = run.spec.as_ref().and_then(|spec| copied_question_text(spec, stem).or_else(|| copied_source_item_marker(spec, printed_label)));
    let mut execution = run.take_execution_trace();
    execution.rejection_reason = rejection.clone();
    executions.push(execution);
    rejection
}

/// Parses one page's model reply into questions, crops its pictures and fits them to generators.
async fn finish_page_async(raw: &str, bytes: &[u8], cfg: &AiCfg, page_no: usize, context: Option<&crate::ditto::DocContext>, source_box_review: Option<SourceBoxReviewTrace>) -> Result<Converted, String> {
    let text = extract_import_text(raw);
    if text.is_empty() {
        return Err("The model returned no text.".to_string());
    }
    if !crate::ditto::is_ditto(&text) {
        let doc = crate::formats::detect_and_parse(text.as_bytes(), "converted")?;
        return Ok(Converted { doc, warnings: Vec::new(), cropped: 0, redrawn: 0, raw_reply: raw.into(), figure_runs: Vec::new(), items: Vec::new(), context: context.cloned(), source_box_review, direction_review:None });
    }
    let mut r = crate::ditto::import_text_in(&text, Some(bytes), page_no, "converted", context)?;
    if let Some(review) = &source_box_review { source_scope::mark_unconfirmed(&mut r, review); }
    let mut redrawn = 0;
    let mut figure_runs = Vec::new();
    // Every picture the figure agent redraws: question pictures first, then answer pictures. Each
    // slot keeps its place, so traces and warnings keep the page's order however the jobs run.
    enum Slot { Done(FigureImportTrace), Job(usize) }
    let mut slots: Vec<Slot> = Vec::new();
    let mut jobs: Vec<RedrawJob> = Vec::new();
    for (i, q) in r.doc.questions.iter_mut().enumerate() {
        if q.image.is_none() || q.figure.is_some() {
            continue;
        }
        let printed_label = r.items.iter().find(|item| item.question_index == Some(i))
            .and_then(|item| item.label.clone());
        let label = printed_label.clone().unwrap_or_else(|| (i + 1).to_string());
        if let Some(item) = r.items.iter().find(|item| item.question_index == Some(i) && item.status == "saved_figure_box_unresolved") {
            // Retain visible source evidence for review, but do not spend figure-tool calls fitting
            // a picture whose scope may include the next problem. A plausible redraw cannot certify it.
            slots.push(Slot::Done(FigureImportTrace { executions:Vec::new(), question_index: i, choice: None, sees: None, steps: Vec::new(), spec: None, review: None, accepted: false,
                note: "Source figure box remains unresolved; figure recreation was not attempted.".into(), error: item.reason.clone(),
                failure: Some(FigureFailure::new("source_scope_unconfirmed", item.reason.as_deref().unwrap_or("The source crop has not been confirmed complete; drawing was not attempted."), None)) }));
            continue;
        }
        let crop = crate::store::read_media(&q.image.as_ref().expect("checked").file).unwrap_or_default();
        if cfg.native_tools {
            slots.push(Slot::Job(jobs.len()));
            jobs.push(RedrawJob { question: i, choice: None, crop, context: q.stem.clone(), printed_label, label });
            continue;
        }
        // Up to 3 attempts: a picture the first generator misses gets another generator.
        match crate::figfit::fit_async_with_retries(&crop, &q.stem, cfg, 3).await {
            Ok(Some(f)) => match crate::store::put_image(&f.png) {
                Ok(img) => {
                    q.image = Some(img);
                    q.figure = Some(f.spec);
                    redrawn += 1;
                }
                Err(e) => r.warnings.push(format!("Question {}: couldn't save the redrawn figure ({e}).", i + 1)),
            },
            Ok(None) => r.warnings.push(format!("Question {}: no Figures-tab generator matched its picture, kept the cropped image.", i + 1)),
            Err(e) => r.warnings.push(format!("Question {}: figure fitting failed ({e}), kept the cropped image.", i + 1)),
        }
    }
    // A choice that is a picture (four pie charts as options A-D) is redrawn like a question's
    // picture; its run is recorded with the choice it belongs to.
    if cfg.native_tools {
        for (i, q) in r.doc.questions.iter().enumerate() {
            let printed_label = r.items.iter().find(|item| item.question_index == Some(i)).and_then(|item| item.label.clone());
            let label = printed_label.clone().unwrap_or_else(|| (i + 1).to_string());
            for (part, ci, a) in choice_slots(&q.kind) {
                let Some(image) = a.image.as_ref().filter(|_| a.figure.is_none()) else { continue };
                let letter = char::from(b'A' + (ci as u8).min(25));
                let context = format!("{}\n[This picture is answer choice {letter}{}.]", q.stem, part.map(|p| format!(" of part {}", char::from(b'a' + (p as u8).min(25)))).unwrap_or_default());
                slots.push(Slot::Job(jobs.len()));
                jobs.push(RedrawJob { question: i, choice: Some(ChoiceRef { part, choice: ci }), crop: crate::store::read_media(&image.file).unwrap_or_default(), context, printed_label: printed_label.clone(), label: format!("{label} choice {letter}") });
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let started = std::time::Instant::now();
    let mut first = run_redraws(&jobs, cfg).await;
    #[cfg(not(target_arch = "wasm32"))]
    retry_failed_siblings(&jobs, &mut first, started.elapsed().as_secs_f64(), cfg).await;
    let mut results: Vec<Option<_>> = first.into_iter().map(|(out, _secs)| Some(out)).collect();
    for slot in slots {
        let trace = match slot {
            Slot::Done(trace) => trace,
            Slot::Job(j) => {
                let (redraw, trace) = results[j].take().expect("each job runs once");
                let job = &jobs[j];
                match redraw {
                    Some((img, spec)) => {
                        let q = &mut r.doc.questions[job.question];
                        match job.choice {
                            None => { q.image = Some(img); q.figure = Some(spec); }
                            Some(c) => if let Some(a) = choice_mut(&mut q.kind, c) { a.image = Some(img); a.figure = Some(spec); },
                        }
                        redrawn += 1;
                    }
                    None => if let Some(reason) = &trace.failure { r.warnings.push(reason.warning(&job.label)) },
                }
                trace
            }
        };
        figure_runs.push(trace);
    }
    Ok(Converted { doc: r.doc, warnings: r.warnings, cropped: r.figures, redrawn, raw_reply: raw.into(), figure_runs, items: r.items, context: context.cloned(), source_box_review, direction_review:None })
}

type RedrawOutcome = (Option<(crate::model::ImageRef, crate::model::FigureSpec)>, FigureImportTrace);

/// Redraw phases at most this long may spend one more wave on retries (seconds).
#[cfg(not(target_arch = "wasm32"))]
const RETRY_PHASE_SECS: f64 = 240.0;
/// A failed redraw is retried only when its own first run took at most this long, which bounds the
/// extra wait (a document runs under one 20-minute budget in the harness).
#[cfg(not(target_arch = "wasm32"))]
const RETRY_JOB_SECS: f64 = 180.0;

/// Which printed item a picture belongs to: a question split into one saved question per
/// sub-figure keeps its printed label, so its sub-figures are siblings.
fn sibling_key(job: &RedrawJob) -> String {
    job.printed_label.clone().unwrap_or_else(|| format!("#{}", job.question))
}

/// Failed redraws whose siblings (other pictures of the same printed item) were redrawn: the
/// figure type is drawable, so a failure there is often a one-off reading or judging slip. One
/// wave of them is asked once more when the page's redraws were quick enough (holdout v11: five
/// items kept one crop among redrawn siblings).
fn sibling_retries(jobs: &[RedrawJob], results: &[(RedrawOutcome, f64)], phase_secs: f64, phase_limit: f64, job_limit: f64) -> Vec<usize> {
    if phase_secs > phase_limit { return Vec::new(); }
    let drawn: std::collections::HashSet<String> = jobs.iter().zip(results).filter(|(_, r)| r.0.0.is_some()).map(|(j, _)| sibling_key(j)).collect();
    (0..jobs.len())
        .filter(|&i| results[i].0.0.is_none() && results[i].1 <= job_limit && drawn.contains(&sibling_key(&jobs[i])))
        .take(3)
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
async fn retry_failed_siblings(jobs: &[RedrawJob], results: &mut [(RedrawOutcome, f64)], phase_secs: f64, cfg: &AiCfg) {
    let retry = sibling_retries(jobs, results, phase_secs, RETRY_PHASE_SECS, RETRY_JOB_SECS);
    if retry.is_empty() { return; }
    let again: Vec<RedrawJob> = retry.iter().map(|&i| jobs[i].clone()).collect();
    for (&i, (out, secs)) in retry.iter().zip(run_redraws(&again, cfg).await) {
        if out.0.is_none() { continue; }
        let (_, first) = &results[i].0;
        let why = first.failure.as_ref().map(|f| f.detail.clone()).or_else(|| first.error.clone()).unwrap_or_else(|| first.note.clone());
        let (redraw, mut trace) = out;
        trace.note = format!("Second attempt, made because the item's other pictures were redrawn; the first failed: {why}. {}", trace.note).trim_end().to_string();
        results[i] = ((redraw, trace), results[i].1 + secs);
    }
}

/// One picture for the figure agent to redraw: a question's own, or one of its answer choices'.
#[derive(Clone)]
struct RedrawJob {
    question: usize,
    choice: Option<ChoiceRef>,
    crop: Vec<u8>,
    /// The question text given as context (with the choice named for an answer picture).
    context: String,
    printed_label: Option<String>,
    /// How warnings name the picture ("8", "8 choice B").
    label: String,
}

/// Figure redraws running at once on one page. Each runs its own model conversation; a page of
/// several figures otherwise took one redraw after another (holdout v10: three figures, past
/// the 20-minute page budget).
#[cfg(not(target_arch = "wasm32"))]
const FIGURE_PARALLEL: usize = 3;

/// Runs a page's redraws, a few at a time on native (each model call blocks its thread), in
/// order in the browser. Results come back in job order.
async fn run_redraws(jobs: &[RedrawJob], cfg: &AiCfg) -> Vec<(RedrawOutcome, f64)> {
    // A panic inside one redraw (holdout v14: the renderer overflowed on far-off coordinates) fails
    // that picture only; the page's other redraws and its questions are kept.
    #[cfg(not(target_arch = "wasm32"))]
    let timed = |job: &RedrawJob| {
        let t = std::time::Instant::now();
        let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            block_on(redraw_crop_async(&job.crop, &job.context, job.printed_label.as_deref(), job.question, job.choice, cfg))
        }))
        .unwrap_or_else(|_| (None, crashed_redraw(job)));
        (out, t.elapsed().as_secs_f64())
    };
    #[cfg(not(target_arch = "wasm32"))]
    if jobs.len() > 1 {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let next = AtomicUsize::new(0);
        let slots: Vec<std::sync::Mutex<Option<_>>> = (0..jobs.len()).map(|_| std::sync::Mutex::new(None)).collect();
        std::thread::scope(|sc| {
            for _ in 0..FIGURE_PARALLEL.min(jobs.len()) {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(job) = jobs.get(i) else { break };
                    *slots[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(timed(job));
                });
            }
        });
        return slots.into_iter().map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner()).expect("every job ran")).collect();
    }
    let mut out = Vec::with_capacity(jobs.len());
    for job in jobs {
        #[cfg(not(target_arch = "wasm32"))]
        let t = std::time::Instant::now();
        let result = redraw_crop_async(&job.crop, &job.context, job.printed_label.as_deref(), job.question, job.choice, cfg).await;
        #[cfg(not(target_arch = "wasm32"))]
        let secs = t.elapsed().as_secs_f64();
        #[cfg(target_arch = "wasm32")]
        let secs = 0.0;
        out.push((result, secs));
    }
    out
}

/// The trace of a redraw that panicked: no figure, the source crop stays.
fn crashed_redraw(job: &RedrawJob) -> FigureImportTrace {
    let detail = "The figure drawing crashed before it finished; the source crop was kept.";
    FigureImportTrace { executions: Vec::new(), question_index: job.question, choice: job.choice, sees: None, steps: Vec::new(), spec: None, review: None, accepted: false, note: String::new(), error: Some(detail.into()), failure: Some(figure_failure::FigureFailure::new("generation_error", detail, None)) }
}

/// Every answer choice of a question kind: (part, choice index, answer).
fn choice_slots(kind: &crate::model::Kind) -> Vec<(Option<usize>, usize, &crate::model::Answer)> {
    match kind {
        crate::model::Kind::Mc { answers } => answers.iter().enumerate().map(|(ci, a)| (None, ci, a)).collect(),
        crate::model::Kind::Multipart { parts } => parts.iter().enumerate().flat_map(|(pi, p)| match &p.kind {
            crate::model::PartKind::Mc { answers } => answers.iter().enumerate().map(|(ci, a)| (Some(pi), ci, a)).collect::<Vec<_>>(),
            _ => Vec::new(),
        }).collect(),
        _ => Vec::new(),
    }
}

fn choice_mut(kind: &mut crate::model::Kind, c: ChoiceRef) -> Option<&mut crate::model::Answer> {
    match (kind, c.part) {
        (crate::model::Kind::Mc { answers }, None) => answers.get_mut(c.choice),
        (crate::model::Kind::Multipart { parts }, Some(pi)) => match &mut parts.get_mut(pi)?.kind {
            crate::model::PartKind::Mc { answers } => answers.get_mut(c.choice),
            _ => None,
        },
        _ => None,
    }
}

/// The answer choice a figure run redrew: its part (for a multipart) and its index.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct ChoiceRef {
    pub part: Option<usize>,
    pub choice: usize,
}

/// One cropped picture through the figure agent, asking once more when the drawing copies the
/// question text. Some((image, spec)) only for a verified redraw that was saved; otherwise the
/// trace's failure says why the source crop stays. The trace is recorded either way.
async fn redraw_crop_async(crop: &[u8], stem: &str, printed_label: Option<&str>, question_index: usize, choice: Option<ChoiceRef>, cfg: &AiCfg) -> (Option<(crate::model::ImageRef, crate::model::FigureSpec)>, FigureImportTrace) {
    let request = figure_request_for_item(stem, "", printed_label);
    let mut run = crate::figure_agent::run_model_async(&request, Some(crop), cfg).await;
    let mut executions=Vec::new();
    let mut earlier_steps = Vec::new();
    // A title that copies a line of the question text is how a crop that reached into the question
    // used to look: ask once for the figure as printed, and do not accept it if the text stays.
    let mut leak = record_figure_execution(&mut run, stem, printed_label, &mut executions);
    if let Some(problem) = leak.clone() {
        earlier_steps = std::mem::take(&mut run.steps);
        run = crate::figure_agent::run_model_async(&figure_request_for_item(stem, &problem, printed_label), Some(crop), cfg).await;
        leak = record_figure_execution(&mut run, stem, printed_label, &mut executions);
    }
    earlier_steps.append(&mut run.steps);
    run.steps = earlier_steps;
    // A known mismatch must never replace the source crop. An unavailable review
    // also stays unresolved rather than becoming an apparently verified redraw.
    let mut saved = None;
    let mut failure = None;
    if leak.is_none() && figure_import_verified(&run) && let (Some(spec), Some(png)) = (&run.spec, &run.png) {
        match crate::store::put_image(png) {
            Ok(img) => saved = Some((img, spec.clone())),
            Err(e) => failure = Some(FigureFailure::new("storage_error", format!("The rendered figure could not be saved: {e}"), Some(&spec.id))),
        }
    }
    let accepted = saved.is_some();
    if !accepted && failure.is_none() {
        failure = Some(figure_failure::from_run(&run, &executions, leak.as_deref()));
    }
    let trace = FigureImportTrace { executions, question_index, choice, sees: run.sees, steps: run.steps, spec: run.spec, review: run.review, accepted, note: run.note, error: run.error.or(leak), failure };
    (saved, trace)
}

/// `transcribe`, then up to two retries when the reply isn't valid JSON: the error is quoted
/// back to the model and the temperature nudged up so it doesn't repeat the same slip.
pub fn transcribe_checked(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str) -> Result<String, String> {
    #[cfg(not(target_arch = "wasm32"))]
    { block_on(transcribe_checked_async(bytes, mime, cfg, prompt)) }
    #[cfg(target_arch = "wasm32")]
    { let _ = (bytes,mime,cfg,prompt); Err("Use the async transcription API in the browser.".into()) }
}

#[cfg(not(target_arch = "wasm32"))]
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).http_status_as_error(false).build().into()
}

fn single_turn(prompt: &str, mime: &str, images: &[&[u8]]) -> Turn {
    Turn { role: "user", text: prompt.into(), images: images.iter().map(|b| b.to_vec()).collect(), mime: mime.into() }
}

fn ollama_chat(cfg: &AiCfg, prompt: &str, images: &[&[u8]], temp: f64) -> Result<String, String> {
    chat_turns(cfg, &[single_turn(prompt, "image/png", images)], temp)
}
fn openai_chat(cfg: &AiCfg, prompt: &str, mime: &str, images: &[&[u8]], temp: f64) -> Result<String, String> {
    openai_chat_once(cfg, prompt, mime, images, temp)
}

/// Compatibility targets that rejected JSON mode, for the life of the process.
fn refused_json_mode() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static SET: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = std::sync::OnceLock::new();
    SET.get_or_init(Default::default)
}

fn json_mode_refused(base: &str) -> bool {
    refused_json_mode().lock().map(|s| s.contains(base)).unwrap_or(false)
}

fn note_json_mode_refused(base: &str) {
    if let Ok(mut s) = refused_json_mode().lock() {
        s.insert(base.to_string());
    }
}

/// Opens an isolated operation. Callers must use `started.cfg` for every subsequent model call.
#[cfg(not(target_arch = "wasm32"))]
pub fn begin_service(cfg: &AiCfg, service: &str, pages: usize) -> Result<Started, String> {
    block_on(begin_service_async(cfg, service, pages))
}

pub async fn begin_service_async(cfg: &AiCfg, service: &str, pages: usize) -> Result<Started, String> {
    operation::start(cfg, service, pages).await
}

/// The hosted AI has no address until the account is signed in: say so, instead of posting to a relative `/service`.
fn account_signed_in_check(cfg: &AiCfg) -> Result<(), String> {
    if cfg.provider == "account" && (cfg.base.trim().is_empty() || !account_ai_ready()) {
        return Err("Sign in on the Account screen first (or wait a moment if you just opened the page), then convert again.".to_string());
    }
    Ok(())
}

/// The relay's sentence for a refused service ("This needs 2 credits and you have 0 left this week.").
fn credit_message(err: &str) -> String {
    if let Some(i) = err.find("answered 402: ") {
        return err[i + "answered 402: ".len()..].trim().to_string();
    }
    err.to_string()
}

fn openai_chat_once(cfg: &AiCfg, prompt: &str, mime: &str, images: &[&[u8]], temp: f64) -> Result<String, String> {
    chat_turns(cfg, &[single_turn(prompt, mime, images)], temp)
}

/// How long a job may run, polled every JOB_POLL, before the app stops waiting.
#[allow(dead_code)] // the app reads pages over chat; kept for when the relay's jobs run on Workflows
#[cfg(not(target_arch = "wasm32"))]
const JOB_WAIT: Duration = Duration::from_secs(25 * 60);
#[allow(dead_code)] // the app reads pages over chat; kept for when the relay's jobs run on Workflows
#[cfg(not(target_arch = "wasm32"))]
const JOB_POLL: Duration = Duration::from_millis(1500);

/// Sends pages to the relay's import job (`POST {base}/import`, then `GET {base}/import/<id>`) and
/// waits for it. One entry per page: the model's reply text, or why that page has none. `Err`
/// means the job could not be started or followed at all (an older relay, a refused request, a job
/// that failed outright), and the caller falls back to the chat path.
#[allow(dead_code)] // the app reads pages over chat; kept for when the relay's jobs run on Workflows
#[cfg(not(target_arch = "wasm32"))]
fn job_texts(pages: &[(&[u8], &str)], hint: &str, cfg: &AiCfg, progress: &dyn Fn(usize, usize)) -> Result<Vec<Result<String, String>>, String> {
    let base = cfg.base.trim_end_matches('/');
    if base.is_empty() {
        return Err("Set the relay URL in Settings.".to_string());
    }
    let key = request_key(cfg);
    let body = json!({
        "hint": hint.trim().chars().take(500).collect::<String>(),
        "pages": pages.iter().map(|(b, m)| json!({"mime": m, "b64": base64_encode(b)})).collect::<Vec<_>>(),
    });
    let id = post_json(&format!("{base}/import"), body, key.as_deref())?
        .get("job_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "The relay did not start a job.".to_string())?;
    let url = format!("{base}/import/{id}");
    let started = std::time::Instant::now();
    loop {
        let job = get_json(&url, key.as_deref())?;
        progress(job.get("pages_done").and_then(Value::as_u64).unwrap_or(0) as usize, pages.len());
        match job.get("status").and_then(Value::as_str) {
            Some("done") => return Ok(job_pages(&job, pages.len())),
            Some("failed") => return Err(job.get("error").and_then(Value::as_str).unwrap_or("The import job failed.").to_string()),
            _ if started.elapsed() > JOB_WAIT => return Err("The import job took too long.".to_string()),
            _ => std::thread::sleep(JOB_POLL),
        }
    }
}

/// The per-page texts of a finished job, in page order; a missing page counts as unread.
#[allow(dead_code)] // the app reads pages over chat; kept for when the relay's jobs run on Workflows
#[cfg(not(target_arch = "wasm32"))]
fn job_pages(job: &Value, total: usize) -> Vec<Result<String, String>> {
    let mut out: Vec<Result<String, String>> = (0..total).map(|_| Err("no reply".to_string())).collect();
    for p in job.pointer("/result/pages").and_then(Value::as_array).into_iter().flatten() {
        let Some(i) = p.get("page").and_then(Value::as_u64).and_then(|n| (n as usize).checked_sub(1)).filter(|i| *i < total) else { continue };
        out[i] = match p.get("text").and_then(Value::as_str) {
            Some(t) => Ok(t.to_string()),
            None => Err(p.get("error").and_then(Value::as_str).unwrap_or("no reply").to_string()),
        };
    }
    out
}

/// One JSON GET, answered like [`post_json`].
#[cfg(not(target_arch = "wasm32"))]
fn get_json(url: &str, key: Option<&str>) -> Result<Value, String> {
    let mut req = agent().get(url);
    if let Some(k) = key {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let mut resp = req.call().map_err(|e| format!("Can't reach {url}: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().map_err(|e| format!("Bad reply from {url}: {e}"))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("{url} did not answer with JSON ({e})."))?;
    if !(200..300).contains(&status) {
        return Err(format!("{url} answered {status}: {}", v.pointer("/error/message").and_then(Value::as_str).unwrap_or("request failed")));
    }
    Ok(v)
}

/// Pause before the single retry of a refused connection.
#[cfg(not(target_arch = "wasm32"))]
const REFUSED_RETRY_MS: u64 = 2000;

/// One JSON POST, with the same error shapes for both providers; answers with the body.
#[cfg(not(target_arch = "wasm32"))]
fn post_json(url: &str, body: Value, key: Option<&str>) -> Result<Value, String> {
    let send = || {
        let mut req = agent().post(url).header("Content-Type", "application/json");
        if let Some(k) = key {
            req = req.header("Authorization", &format!("Bearer {k}"));
        }
        req.send(body.to_string())
    };
    // A refused connection never reached the server, so one retry after a short pause cannot
    // repeat or double-bill work. Every other failure is reported at once.
    let refused = |e: &ureq::Error| matches!(e, ureq::Error::Io(io) if io.kind() == std::io::ErrorKind::ConnectionRefused);
    let first = send();
    let sent = match first {
        Err(e) if refused(&e) => {
            std::thread::sleep(std::time::Duration::from_millis(REFUSED_RETRY_MS));
            send()
        }
        other => other,
    };
    let mut resp = sent.map_err(|e| format!("Can't reach {url}: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().map_err(|e| format!("Bad reply from {url}: {e}"))?;
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{url} did not answer with JSON ({e})."))?;
    if !(200..300).contains(&status) {
        let msg = v.pointer("/error/message").and_then(Value::as_str).or_else(|| v.get("error").and_then(Value::as_str)).unwrap_or("request failed");
        return Err(format!("{url} answered {status}: {msg}"));
    }
    Ok(v)
}

/// The blocking path is not used in the browser; async callers use [`post_json_async`].
#[cfg(target_arch = "wasm32")]
fn post_json(url: &str, _body: Value, _key: Option<&str>) -> Result<Value, String> {
    Err(format!("This build uses the async browser transport (cannot reach {url})"))
}

#[cfg(not(target_arch = "wasm32"))]
async fn post_json_async(url: &str, body: &Value, key: Option<&str>) -> Result<Value, String> {
    post_json(url, body.clone(), key)
}

/// Browser transport: one JSON POST with CORS and a 300 s timeout.
#[cfg(target_arch = "wasm32")]
async fn post_json_async(url: &str, body: &Value, key: Option<&str>) -> Result<Value, String> {
    use wasm_bindgen::JsCast as _;
    let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let headers = web_sys::Headers::new().map_err(js)?;
    headers.set("Content-Type", "application/json").map_err(js)?;
    if let Some(k) = key {
        headers.set("Authorization", &format!("Bearer {k}")).map_err(js)?;
    }
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_mode(web_sys::RequestMode::Cors);
    init.set_body(&wasm_bindgen::JsValue::from_str(&body.to_string()));
    init.set_headers(&headers);
    let signal = web_sys::AbortSignal::timeout_with_u32(300_000);
    init.set_signal(Some(&signal));
    let window = web_sys::window().ok_or("no window")?;
    let promise = window.fetch_with_str_and_init(url, &init);
    let resp = wasm_bindgen_futures::JsFuture::from(promise).await.map_err(|_| {
        // A page cannot tell a CORS refusal from a server that is not there.
        format!("Can't reach {url}. The server may be off, or it does not allow requests from this page (CORS).")
    })?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "not a response".to_string())?;
    let status = resp.status();
    let text = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(js)?).await.map_err(js)?.as_string().unwrap_or_default();
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{url} did not answer with JSON ({e})."))?;
    if !(200..300).contains(&status) {
        let msg = v.pointer("/error/message").and_then(Value::as_str).or_else(|| v.get("error").and_then(Value::as_str)).unwrap_or("request failed");
        return Err(format!("{url} answered {status}: {msg}"));
    }
    Ok(v)
}

pub async fn ask_images_async(images: &[&[u8]], cfg: &AiCfg, prompt: &str, temp: f64) -> Result<String, String> {
    chat_turns_async(cfg, &[single_turn(prompt, "image/png", images)], temp).await
}

async fn transcribe_at_async(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str, temp: f64) -> Result<String, String> {
    chat_turns_async(cfg, &[single_turn(prompt, mime, &[bytes])], temp).await
}

pub async fn transcribe_checked_async(bytes: &[u8], mime: &str, cfg: &AiCfg, prompt: &str) -> Result<String, String> {
    let mut last = Err("The AI returned no usable answer.".to_string());
    let mut instruction = prompt.to_string();
    for attempt in 0..=2 {
        match transcribe_at_async(bytes, mime, cfg, &instruction, 0.2 * attempt as f64).await {
            Ok(reply) => match crate::ditto::json_error(&reply) {
                None if !reply.trim().is_empty() => return Ok(reply),
                problem => last = Err(problem.unwrap_or_else(|| "The reply was empty.".into())),
            },
            Err(e) => {
                // Authentication, credit, connectivity and unsupported capability errors need user action.
                if !e.starts_with("Invalid AI result:") { return Err(e); }
                last = Err(e);
            }
        }
        instruction = format!("{prompt}\n\nYour previous reply could not be used. Return the complete valid JSON result, without commentary.");
    }
    last
}

/// Shared conversion workflow; desktop calls this on a worker, browser awaits it.
pub async fn convert_page_async(bytes: &[u8], mime: &str, hint: &str, cfg: &AiCfg, page_no: usize) -> Result<Converted, String> {
    let started;
    let cfg = if cfg.operation.is_none() {
        started = begin_service_async(cfg, "import", 1).await?;
        &started.cfg
    } else { cfg };
    convert_page_chat_async(bytes, mime, hint, cfg, page_no).await
}

pub async fn convert_page_chat_async(bytes: &[u8], mime: &str, hint: &str, cfg: &AiCfg, page_no: usize) -> Result<Converted, String> {
    convert_page_in_async(bytes, mime, hint, cfg, page_no, None).await
}

/// One page read together with the directions earlier pages of the same document printed. `None` reads the
/// page on its own (a single photo, or the page-only ablation).
pub async fn convert_page_in_async(bytes: &[u8], mime: &str, hint: &str, cfg: &AiCfg, page_no: usize, context: Option<&crate::ditto::DocContext>) -> Result<Converted, String> {
    let prompt = page_prompt(hint, context, page_no);
    let raw = crate::ditto::split_part_figures(&transcribe_checked_async(bytes, mime, cfg, &prompt).await?);
    let (raw, direction_review) = directions::review(raw,bytes,mime,cfg,page_no,context).await;
    let (raw, review) = review_source_figure_boxes(raw, bytes, mime, cfg, page_no).await;
    let mut converted=finish_page_async(&raw, bytes, cfg, page_no, context, review).await?;
    if let Some(task)=direction_review.as_ref().and_then(|r|r.page_task_recovery.as_ref()).filter(|r|!r.blocked_indices.is_empty()) {
        let source=crate::ditto::json_object(&raw);
        let labels=task.blocked_indices.iter().map(|index|{
            source.as_ref().and_then(|v|v["bank"]["questions"].get(*index))
                .and_then(|q|q.get("label")).and_then(Value::as_str)
                .map(str::to_string).unwrap_or_else(||(index+1).to_string())
        }).collect::<Vec<_>>().join(", ");
        let reason=task.error.as_deref().unwrap_or("The current page's task wording or applicability is uncertain.");
        converted.warnings.push(format!("Task directions need review for questions {labels} on source page {page_no}: {reason}"));
    }
    converted.direction_review=direction_review;
    Ok(converted)
}

/// Every page of ONE source document, in page order, each read with the directions the earlier pages of
/// that document printed. The context starts empty and exists only inside this call, so nothing carries
/// into another document. A page that fails leaves the context as it was.
pub async fn convert_document_async(pages: &[Vec<u8>], hint: &str, cfg: &AiCfg, document: &str, progress: &(dyn Fn(usize, usize) + Sync)) -> Vec<Result<Converted, String>> {
    let mut context = crate::ditto::DocContext { document: document.to_string(), directions: Vec::new() };
    let mut out = Vec::with_capacity(pages.len());
    for (i, png) in pages.iter().enumerate() {
        let r = convert_page_in_async(png, "image/png", hint, cfg, i + 1, Some(&context)).await;
        if let Ok(c) = &r {
            context = context.after_page(i + 1, &c.items);
        }
        out.push(r);
        progress(i + 1, pages.len());
    }
    for k in 1..out.len() {
        let (before, after) = out.split_at_mut(k);
        if let (Ok(prev), Ok(next)) = (&mut before[k - 1], &mut after[0]) {
            join_page_break(prev, next, k);
        }
    }
    out
}

/// Link content that continues a question begun on the previous page (the reply's `continued`
/// items, saved first on `next`) to that page's last question. One continuation is joined onto it,
/// so a problem split by a page break is one bank item with its number, data and task. When a join
/// would drop a picture or answers, or the continuation was split into one question per figure,
/// each continuation question instead takes the host's number and lead text, like a split part.
/// Returns whether anything was linked.
fn join_page_break(prev: &mut Converted, next: &mut Converted, prev_page: usize) -> bool {
    // A continuation is known by its review topic: a later check (the figure scope review) may
    // have replaced its saved_continued status with its own review state (holdout v13, SEC JC).
    let mut conts: Vec<(usize, usize)> = next.items.iter().enumerate()
        .filter_map(|(slot, i)| i.question_index.map(|qi| (qi, slot)))
        .filter(|&(qi, slot)| next.items[slot].status == "saved_continued"
            || next.doc.questions.get(qi).is_some_and(|q| q.topics.iter().any(|t| t == crate::ditto::CONTINUED_TOPIC)))
        .collect();
    conts.sort();
    // Only content leading the page continues the previous page's last question.
    let leading = conts.iter().enumerate().take_while(|(k, (qi, _))| k == qi).count();
    conts.truncate(leading);
    if conts.is_empty() || prev.doc.questions.is_empty() { return false; }
    // Only a continuation with no other review state is joined; one with an unresolved figure box
    // keeps that state on its own question.
    if conts.len() == 1 && next.items[conts[0].1].status == "saved_continued" && merge_continuation(prev, next, conts[0].1, prev_page) { return true; }
    adopt_continuations(prev, next, &conts, prev_page)
}

/// Each continuation question keeps its own picture and takes the host's printed number and its
/// lead text (the shared start of a split group, else the host's stem).
fn adopt_continuations(prev: &mut Converted, next: &mut Converted, conts: &[(usize, usize)], prev_page: usize) -> bool {
    let target = prev.doc.questions.len() - 1;
    let Some(number) = prev.items.iter().find(|i| i.question_index == Some(target)).and_then(|i| i.number) else { return false };
    let group: Vec<&str> = prev.items.iter()
        .filter(|i| i.number == Some(number))
        .filter_map(|i| i.question_index.and_then(|qi| prev.doc.questions.get(qi)).map(|q| q.stem.as_str()))
        .collect();
    let lead = if group.len() > 1 { shared_lead(&group) } else { prev.doc.questions[target].stem.trim().to_string() };
    for &(qi, slot) in conts {
        let item = &mut next.items[slot];
        let suffix: String = item.label.as_deref().unwrap_or("").trim().trim_matches(['(', ')', '[', ']']).trim()
            .get("continued".len()..).unwrap_or("").trim().trim_matches(['(', ')', '.', ' ']).to_string();
        let label = if suffix.is_empty() { format!("{number} (continued)") } else { format!("{number}{suffix}") };
        item.number = Some(number);
        item.label = Some(label.clone());
        // Another review state (an unresolved figure box) stays; only the continued flag is settled.
        if item.status == "saved_continued" { item.status = "saved".into(); }
        item.reason = Some(format!("continues question {number} from page {prev_page}; saved as its own question with that number and its lead text"));
        let q = &mut next.doc.questions[qi];
        // The host's lead text carries the task the continuation lacked.
        if !lead.is_empty() { settle_unresolved_task(q); }
        if !lead.is_empty() && !q.stem.trim_start().starts_with(&lead) { q.stem = format!("{lead}\n\n{}", q.stem.trim_start()).trim_end().to_string(); }
        q.topics.retain(|t| t != crate::ditto::CONTINUED_TOPIC);
        next.warnings.push(format!("Question {label}: continues question {number} from page {prev_page}; saved with that number and its lead text."));
    }
    next.warnings.retain(|w| !w.contains("continues a question begun on an earlier page"));
    true
}

/// Drop the "task not visible" placeholder and its review topic from a continuation linked to the
/// question whose task it continues (holdout v14, RMIT sine rule 1c).
fn settle_unresolved_task(q: &mut crate::model::Question) {
    if q.stem.contains(crate::ditto::UNRESOLVED_TASK) {
        q.stem = q.stem.replace(crate::ditto::UNRESOLVED_TASK, "").trim().to_string();
        while q.stem.contains("\n\n\n") { q.stem = q.stem.replace("\n\n\n", "\n\n"); }
    }
    q.topics.retain(|t| t != crate::ditto::UNRESOLVED_TOPIC);
}

/// The text a split group's questions share before their own part, cut back to a line or sentence end.
fn shared_lead(stems: &[&str]) -> String {
    let first = stems[0];
    let mut end = first.len();
    for s in &stems[1..] {
        end = end.min(first.char_indices().zip(s.chars()).take_while(|((_, a), b)| a == b).last().map(|((i, a), _)| i + a.len_utf8()).unwrap_or(0));
    }
    let common = &first[..end];
    let cut = common.rfind('\n').or_else(|| common.rfind(". ").map(|i| i + 1)).unwrap_or(0);
    common[..cut].trim().to_string()
}

/// [`join_page_break`]'s join: the continuation at `slot` appended to the previous page's last question.
fn merge_continuation(prev: &mut Converted, next: &mut Converted, slot: usize, prev_page: usize) -> bool {
    use crate::model::{Kind, PartKind};
    let target = prev.doc.questions.len() - 1;
    let cont = &next.doc.questions[0];
    let host = &prev.doc.questions[target];
    let choice_pictures = |q: &crate::model::Question| match &q.kind {
        Kind::Mc { answers } => answers.iter().any(|a| a.image.is_some() || a.figure.is_some()),
        Kind::Multipart { parts } => parts.iter().any(|p| matches!(&p.kind, PartKind::Mc { answers } if answers.iter().any(|a| a.image.is_some() || a.figure.is_some()))),
        _ => false,
    };
    let cont_picture = cont.image.is_some() || cont.figure.is_some();
    if choice_pictures(cont) || (cont_picture && (host.image.is_some() || host.figure.is_some())) { return false; }
    let open_host = matches!(&host.kind, Kind::Short { sample } if sample.trim().is_empty());
    let kind = match (&host.kind, &cont.kind) {
        (Kind::Multipart { parts: a }, Kind::Multipart { parts: b }) => {
            Kind::Multipart { parts: a.iter().chain(b).cloned().collect() }
        }
        (Kind::Multipart { parts }, Kind::Short { sample }) if sample.trim().is_empty() && !parts.is_empty() => {
            let mut parts = parts.clone();
            let last = parts.last_mut().expect("nonempty parts");
            last.stem = join_text(&last.stem, &cont.stem);
            Kind::Multipart { parts }
        }
        (Kind::Short { sample: a }, Kind::Short { sample: b }) => {
            Kind::Short { sample: if a.trim().is_empty() { b.clone() } else { a.clone() } }
        }
        (_, other) if open_host && !matches!(other, Kind::Short { .. }) => other.clone(),
        _ => return false,
    };
    let into_part = matches!((&host.kind, &cont.kind), (Kind::Multipart { .. }, Kind::Short { .. }));
    let mut cont = next.doc.questions.remove(0);
    settle_unresolved_task(&mut cont);
    let label = prev.items.iter().find(|i| i.question_index == Some(target)).and_then(|i| i.label.clone()).unwrap_or_else(|| format!("{}", target + 1));
    let host = &mut prev.doc.questions[target];
    if !into_part { host.stem = join_text(&host.stem, &cont.stem); }
    host.kind = kind;
    if host.points.is_none() { host.points = cont.points; }
    for topic in cont.topics {
        if topic != crate::ditto::CONTINUED_TOPIC && !host.topics.contains(&topic) { host.topics.push(topic); }
    }
    if host.explanation.is_none() { host.explanation = cont.explanation; }
    if cont_picture {
        host.image = cont.image;
        host.figure = cont.figure;
    }
    let mut kept = Vec::with_capacity(next.figure_runs.len());
    for mut run in next.figure_runs.drain(..) {
        if run.question_index == 0 {
            run.question_index = target;
            prev.figure_runs.push(run);
        } else {
            run.question_index -= 1;
            kept.push(run);
        }
    }
    next.figure_runs = kept;
    for item in &mut next.items {
        if let Some(qi) = item.question_index.as_mut() && *qi > 0 { *qi -= 1; }
    }
    let item = &mut next.items[slot];
    item.question_index = None;
    item.merged_into = Some(target);
    item.status = "merged_into_previous_page".into();
    item.reason = Some(format!("continues question {label} from page {prev_page}; joined to that question"));
    prev.warnings.push(format!("Question {label}: joined with its continuation printed at the top of page {}.", prev_page + 1));
    next.warnings.retain(|w| !w.contains("continues a question begun on an earlier page"));
    next.warnings.push(format!("The continuation at the top of this page was joined to question {label} on page {prev_page}."));
    true
}

/// Two halves of text split by a page break, joined with one space (a word hyphenated at the break is rejoined).
fn join_text(first: &str, second: &str) -> String {
    let (a, b) = (first.trim_end(), second.trim_start());
    if a.is_empty() { return b.to_string(); }
    if b.is_empty() { return a.to_string(); }
    let mut chars = a.chars().rev();
    if chars.next() == Some('-') && chars.next().is_some_and(char::is_alphabetic) && b.starts_with(|c: char| c.is_lowercase()) {
        return format!("{}{b}", &a[..a.len() - 1]);
    }
    format!("{a} {b}")
}

/// Several documents, each read in page order, up to `VERSHION_AI_PARALLEL` documents at the same time.
/// Results come back per document, in the order given.
#[cfg(not(target_arch = "wasm32"))]
pub fn convert_documents(documents: &[(String, Vec<Vec<u8>>)], hint: &str, cfg: &AiCfg, progress: &(dyn Fn(usize, usize) + Sync)) -> Vec<Vec<Result<Converted, String>>> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let total: usize = documents.iter().map(|d| d.1.len()).sum();
    let started = match begin_service(cfg, "import", total) {
        Ok(s) => s,
        Err(e) => return documents.iter().map(|d| d.1.iter().map(|_| Err(e.clone())).collect()).collect(),
    };
    let cfg = &started.cfg;
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<Vec<Result<Converted, String>>>>> = documents.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|sc| {
        for _ in 0..page_parallel().min(documents.len()) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some((name, pages)) = documents.get(i) else { break };
                let results = block_on(convert_document_async(pages, hint, cfg, name, &|_, _| {
                    progress(done.fetch_add(1, Ordering::SeqCst) + 1, total);
                }));
                *slots[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(results);
            });
        }
    });
    slots.into_iter().zip(documents).map(|(m, d)| m.into_inner().unwrap_or_else(|e| e.into_inner()).unwrap_or_else(|| d.1.iter().map(|_| Err("page was not read".to_string())).collect())).collect()
}

/// Strips an accidental markdown fence (with its language tag) and outer whitespace; the
/// `formats` parsers tolerate everything else a model might add around the import text.
pub fn extract_import_text(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(rest) = s.strip_prefix("```")
        && let Some((_, body)) = rest.split_once('\n')
    {
        let body = body.trim_end();
        let body = body.strip_suffix("```").map_or(body, str::trim_end);
        s = body;
    }
    s.trim().to_string()
}

/// Standard base64 (RFC 4648, with padding). Small local encoder; base64 is not a crate here.
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let a = u32::from(chunk[0]);
        let b = chunk.get(1).copied().map_or(0, u32::from);
        let c = chunk.get(2).copied().map_or(0, u32::from);
        let n = (a << 16) | (b << 8) | c;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn page_question(stem: &str, kind: crate::model::Kind) -> crate::model::Question {
        crate::model::Question { id: crate::model::new_id(), stem: stem.into(), kind, points: None, difficulty: None, topics: Vec::new(), image: None, explanation: None, figure: None, created: 0, modified: 0 }
    }
    fn page_of(questions: Vec<crate::model::Question>, items: Vec<crate::ditto::SourceItem>) -> Converted {
        Converted { doc: crate::formats::ImportDoc { name: String::new(), questions, exams: Vec::new() }, warnings: Vec::new(), cropped: 0, redrawn: 0, raw_reply: String::new(), figure_runs: Vec::new(), items, context: None, source_box_review: None, direction_review: None }
    }
    fn saved(index: usize, label: &str, number: Option<u32>, status: &str, qi: usize) -> crate::ditto::SourceItem {
        crate::ditto::SourceItem { index, label: Some(label.into()), number, status: status.into(), question_index: Some(qi), figure: "none".into(), ..Default::default() }
    }
    fn short() -> crate::model::Kind { crate::model::Kind::Short { sample: String::new() } }
    fn figure_run(question_index: usize) -> FigureImportTrace {
        FigureImportTrace { executions: Vec::new(), question_index, choice: None, sees: None, steps: Vec::new(), spec: None, review: None, accepted: true, note: String::new(), error: None, failure: None }
    }

    #[test]
    fn only_a_quick_failed_picture_with_a_redrawn_sibling_is_retried() {
        let job = |question: usize, label: Option<&str>| RedrawJob { question, choice: None, crop: Vec::new(), context: String::new(), printed_label: label.map(str::to_string), label: String::new() };
        let ok = |q: usize, secs: f64| ((Some((crate::model::ImageRef::default(), crate::model::FigureSpec { id: "x".into(), params: Default::default() })), figure_run(q)), secs);
        let bad = |q: usize, secs: f64| ((None, figure_run(q)), secs);
        // Question 1 split into three saved questions (1a, 1b, 1c), one failed; question 2 alone failed;
        // question 3's own picture redrew but its slow choice picture failed.
        let jobs = vec![job(0, Some("1")), job(1, Some("1")), job(2, Some("1")), job(3, Some("2")), job(4, None), job(4, None)];
        let results = vec![ok(0, 40.0), bad(1, 60.0), ok(2, 50.0), bad(3, 30.0), ok(4, 20.0), bad(4, 200.0)];
        assert_eq!(sibling_retries(&jobs, &results, 120.0, 240.0, 180.0), [1]);
        assert!(sibling_retries(&jobs, &results, 300.0, 240.0, 180.0).is_empty(), "a slow page spends no more time");
        assert_eq!(sibling_retries(&jobs, &results, 120.0, 240.0, 600.0), [1, 5]);
    }

    #[test]
    fn a_problem_split_by_a_page_break_is_joined_into_one_question() {
        let mut prev = page_of(vec![page_question("First.", short()), page_question("Assume the sons of professional men ... Set up", short())],
            vec![saved(0, "10", Some(10), "saved", 0), saved(1, "11", Some(11), "saved", 1)]);
        let mut cont = page_question("the matrix of transition probabilities. Find the probability.", short());
        cont.topics.push(crate::ditto::CONTINUED_TOPIC.into());
        let mut next = page_of(vec![cont, page_question("Twelve.", short()), page_question("Thirteen.", short())],
            vec![saved(0, "continued", None, "saved_continued", 0), saved(1, "12", Some(12), "saved", 1), saved(2, "13", Some(13), "saved", 2)]);
        next.warnings.push("Question continued: continues a question begun on an earlier page; its number and task are not printed here, so review it with that page.".into());
        next.figure_runs.push(figure_run(2));
        assert!(join_page_break(&mut prev, &mut next, 1));
        assert_eq!(prev.doc.questions[1].stem, "Assume the sons of professional men ... Set up the matrix of transition probabilities. Find the probability.");
        assert!(prev.doc.questions[1].topics.is_empty());
        assert_eq!(next.doc.questions.iter().map(|q| q.stem.as_str()).collect::<Vec<_>>(), ["Twelve.", "Thirteen."]);
        assert_eq!(next.items.iter().map(|i| i.question_index).collect::<Vec<_>>(), [None, Some(0), Some(1)]);
        assert_eq!((next.items[0].status.as_str(), next.items[0].merged_into), ("merged_into_previous_page", Some(1)));
        assert_eq!(next.figure_runs[0].question_index, 1);
        assert!(next.warnings.iter().all(|w| !w.contains("continues a question begun")));
        assert!(prev.warnings.iter().any(|w| w.contains("Question 11: joined")));
        let json = serde_json::to_value(&next.items).unwrap();
        assert_eq!(json[0]["merged_into"], 1);
        assert!(json[1].get("merged_into").is_none());
    }

    #[test]
    fn continued_parts_and_a_moved_picture_join_but_a_second_picture_does_not() {
        use crate::model::{Kind, Part, PartKind};
        let part = |s: &str| Part { stem: s.into(), kind: PartKind::Short { sample: String::new() } };
        let mut prev = page_of(vec![page_question("Use the table.", Kind::Multipart { parts: vec![part("a"), part("b")] })], vec![saved(0, "4", Some(4), "saved", 0)]);
        let mut cont = page_question("", Kind::Multipart { parts: vec![part("c")] });
        cont.image = Some(crate::model::ImageRef { file: "media/x.png".into(), width: 1, height: 1 });
        let mut next = page_of(vec![cont], vec![saved(0, "continued", None, "saved_continued", 0)]);
        next.figure_runs.push(figure_run(0));
        assert!(join_page_break(&mut prev, &mut next, 1));
        let Kind::Multipart { parts } = &prev.doc.questions[0].kind else { panic!() };
        assert_eq!(parts.iter().map(|p| p.stem.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(prev.doc.questions[0].stem, "Use the table.");
        assert!(prev.doc.questions[0].image.is_some());
        assert_eq!((prev.figure_runs.len(), prev.figure_runs[0].question_index, next.figure_runs.len()), (1, 0, 0));
        // Text continuing the last part goes to that part.
        let mut prev = page_of(vec![page_question("Lead.", Kind::Multipart { parts: vec![part("a) Find the"), part("b) Show that the prob-")] })], vec![saved(0, "4", Some(4), "saved", 0)]);
        let mut next = page_of(vec![page_question("ability is one half.", short())], vec![saved(0, "continued", None, "saved_continued", 0)]);
        assert!(join_page_break(&mut prev, &mut next, 1));
        let Kind::Multipart { parts } = &prev.doc.questions[0].kind else { panic!() };
        assert_eq!(parts[1].stem, "b) Show that the probability is one half.");
        // Both halves carry a picture (holdout v12, SCSA #6): no join; the continuation keeps its
        // picture and takes the host's number and lead text.
        let mut host = page_question("The graph is shown below.", short());
        host.image = Some(crate::model::ImageRef::default());
        let mut cont = page_question(&format!("(b) Use the second graph.\n\n{}", crate::ditto::UNRESOLVED_TASK), short());
        cont.image = Some(crate::model::ImageRef::default());
        cont.topics.push(crate::ditto::CONTINUED_TOPIC.into());
        cont.topics.push(crate::ditto::UNRESOLVED_TOPIC.into());
        let mut prev = page_of(vec![host], vec![saved(0, "6", Some(6), "saved", 0)]);
        let mut next = page_of(vec![cont], vec![saved(0, "continued", None, "saved_continued", 0)]);
        assert!(join_page_break(&mut prev, &mut next, 1));
        assert_eq!(prev.doc.questions[0].stem, "The graph is shown below.");
        assert_eq!(next.doc.questions[0].stem, "The graph is shown below.\n\n(b) Use the second graph.");
        assert!(next.doc.questions[0].image.is_some() && next.doc.questions[0].topics.is_empty());
        let item = &next.items[0];
        assert_eq!((item.number, item.label.as_deref(), item.status.as_str(), item.question_index), (Some(6), Some("6 (continued)"), "saved", Some(0)));
        // No continuation on the next page: nothing changes.
        let mut next = page_of(vec![page_question("Three.", short())], vec![saved(0, "3", Some(3), "saved", 0)]);
        assert!(!join_page_break(&mut prev, &mut next, 1));
        // A host with no printed number: the continuation stays flagged.
        let mut prev = page_of(vec![page_question("Intro.", short())], vec![saved(0, "", None, "saved", 0)]);
        let mut next = page_of(vec![page_question("more", Kind::Mc { answers: vec![crate::model::Answer::new("x", true)] })], vec![saved(0, "continued", None, "saved_continued", 0)]);
        next.doc.questions[0].image = Some(crate::model::ImageRef::default());
        prev.doc.questions[0].image = Some(crate::model::ImageRef::default());
        assert!(!join_page_break(&mut prev, &mut next, 1));
        assert_eq!(next.items[0].status, "saved_continued");
    }

    #[test]
    fn a_continuation_whose_status_a_later_review_replaced_is_still_linked_and_keeps_that_state() {
        // Holdout v13, SEC JC #6: the scope review set saved_figure_box_unresolved on the continuation.
        use crate::model::{Kind, Part, PartKind};
        let part = |s: &str| Part { stem: s.into(), kind: PartKind::Short { sample: String::new() } };
        let mut host = page_question("John looked up the figures.", Kind::Multipart { parts: vec![part("(a)"), part("(b)")] });
        host.image = Some(crate::model::ImageRef::default());
        let mut prev = page_of(vec![host], vec![saved(0, "6", Some(6), "saved", 0)]);
        let mut cont = page_question("", Kind::Multipart { parts: vec![part("(c) Draw two bars."), part("(d)")] });
        cont.topics.push(crate::ditto::CONTINUED_TOPIC.into());
        let mut next = page_of(vec![cont], vec![saved(0, "continued", None, "saved_figure_box_unresolved", 0)]);
        assert!(join_page_break(&mut prev, &mut next, 1));
        let item = &next.items[0];
        assert_eq!((item.number, item.label.as_deref(), item.status.as_str()), (Some(6), Some("6 (continued)"), "saved_figure_box_unresolved"));
        assert_eq!(next.doc.questions[0].stem, "John looked up the figures.");
        assert!(next.doc.questions[0].topics.is_empty());
        // Without the continued topic, an unresolved first item is not taken for a continuation.
        let mut next = page_of(vec![page_question("Seven.", short())], vec![saved(0, "7", Some(7), "saved_figure_box_unresolved", 0)]);
        assert!(!join_page_break(&mut prev, &mut next, 1));
    }

    #[test]
    fn a_continued_problem_split_per_figure_takes_the_host_number_and_shared_lead() {
        // Holdout v12, Utah grade 8: 1a-1d (one grid each) on page 1, parts e-h on page 2.
        let lead = "Solve the system of linear equations graphically.\nIf there is one solution, verify it.";
        let pictured = |stem: String| { let mut q = page_question(&stem, short()); q.image = Some(crate::model::ImageRef::default()); q };
        let mut prev = page_of(["a", "b", "c", "d"].iter().map(|p| pictured(format!("{lead}\n{p}. y = x"))).collect(),
            ["1a", "1b", "1c", "1d"].iter().enumerate().map(|(i, l)| saved(i, l, Some(1), "saved", i)).collect());
        let mut next = page_of(["e", "f", "g", "h"].iter().map(|p| pictured(format!("{p}. y = 2x"))).collect(),
            ["e", "f", "g", "h"].iter().enumerate().map(|(i, p)| saved(i, &format!("continued{p}"), None, "saved_continued", i)).collect());
        next.items.push(saved(4, "2", Some(2), "saved", 4));
        next.doc.questions.push(page_question("Two.", short()));
        assert!(join_page_break(&mut prev, &mut next, 1));
        assert_eq!(next.items.iter().map(|i| i.label.clone().unwrap()).collect::<Vec<_>>(), ["1e", "1f", "1g", "1h", "2"]);
        assert!(next.items[..4].iter().all(|i| i.number == Some(1) && i.status == "saved"));
        assert_eq!(next.doc.questions[0].stem, format!("{lead}\n\ne. y = 2x"));
        assert_eq!(next.doc.questions[4].stem, "Two.");
        assert_eq!(shared_lead(&["Find it.\na) one", "Find it.\nb) two"]), "Find it.");
        assert_eq!(shared_lead(&["a) one", "b) two"]), "");
    }

    fn settings(provider: &str, model: &str, base: &str) -> Settings {
        Settings { ai_provider: provider.into(), ai_model: model.into(), ai_base: base.into(), dark: bool::default(), ..Default::default() }
    }

    #[test]
    fn legacy_settings_keep_provider_model_and_compatible_output() {
        let settings:Settings=serde_json::from_value(json!({"ai_provider":"custom","ai_base":"https://my.example/v1","ai_model":"own-model"})).unwrap();
        let cfg=AiCfg::from(&settings);
        assert_eq!((cfg.provider.as_str(),cfg.base.as_str(),cfg.model.as_str()),("custom","https://my.example/v1","own-model"));
        assert_eq!(cfg.output_mode,OutputMode::Json); assert!(!cfg.native_tools); assert!(cfg.operation.is_none());
        assert_eq!(AiCfg::from(&Settings::default()).provider,"ollama");
    }

    #[test]
    fn prompt_names_every_type_and_the_figure_rules() {
        let p = transcription_prompt("");
        for ty in ["multiple-choice", "true-false", "matching", "fill-in-the-blank", "short-answer", "multipart"] {
            assert!(p.contains(ty), "prompt must name {ty}");
        }
        for must in ["___", "\"box\"", "queshion/ditto", "No markdown fences", "[ymin, xmin, ymax, xmax]", "0 to 1000 grid"] {
            assert!(p.contains(must), "prompt must mention {must}");
        }
    }

    #[test]
    fn prompt_appends_the_user_hint() {
        let p = transcription_prompt("page 2 is multiple choice");
        assert!(p.contains("User note about this source:\npage 2 is multiple choice"), "{p}");
        // Empty hint changes nothing.
        assert_eq!(transcription_prompt("  "), TRANSCRIBE_PROMPT);
    }

    #[test]
    fn extract_strips_fences_and_whitespace() {
        assert_eq!(extract_import_text("  \n```text\nMC\nQ?\n*a. 1\n```\n  "), "MC\nQ?\n*a. 1");
        assert_eq!(extract_import_text("```\nMC\nQ?\n*a. 1\n```"), "MC\nQ?\n*a. 1");
        assert_eq!(extract_import_text(" MC\nQ? "), "MC\nQ?");
        // No content line after the fence: left as-is for the parsers to report.
        assert_eq!(extract_import_text("```junk"), "```junk");
    }

    #[test]
    #[ignore = "requires VERSHION_SOURCE_SCOPE_FIXTURES with pinned local source replies and output paths"]
    fn write_source_scope_contact_sheets() {
        let path=std::env::var("VERSHION_SOURCE_SCOPE_FIXTURES").unwrap();
        let fixtures:Vec<Value>=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for fixture in fixtures {
            let page=std::fs::read(fixture["source_png"].as_str().unwrap()).unwrap();
            let previews=crate::ditto::source_figure_previews(fixture["raw_reply"].as_str().unwrap(),&page,fixture["page_no"].as_u64().unwrap() as usize);
            assert_eq!(previews.len(),fixture["expected_crops"].as_u64().unwrap() as usize);
            let sheet=source_review_contact_sheet(&previews).unwrap();
            let output=std::path::Path::new(fixture["output"].as_str().unwrap());
            std::fs::create_dir_all(output.parent().unwrap()).unwrap();std::fs::write(output,sheet).unwrap();
        }
    }

    #[test]
    fn many_source_crops_stay_in_one_readable_sheet_with_their_original_pixels() {
        let previews:Vec<_>=(0..12).map(|i|{
            let color=if i%2==0 {image::Rgb([255,0,0])} else {image::Rgb([0,0,255])};
            let image=image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(80,60,color));
            let mut png=Vec::new();image.write_to(&mut std::io::Cursor::new(&mut png),image::ImageFormat::Png).unwrap();
            (i,(i+1).to_string(),png)
        }).collect();
        let png=source_review_contact_sheet(&previews).unwrap();
        let image=image::load_from_memory(&png).unwrap().to_rgb8();
        assert_eq!(image.dimensions(),(1260,1440));
        for i in 0..12 {
            let color=image.get_pixel((i%3)*420+210,(i/3)*360+190).0;
            assert_eq!(color,if i%2==0 {[255,0,0]} else {[0,0,255]},"source crop {i} must remain visible in its own labeled tile");
        }
    }

    /// A one-connection-per-request stub relay: answers `replies` in order, records each request line
    /// and body, and stops when they run out.
    #[test]
    fn a_refused_connection_is_retried_once_and_then_reaches_the_server() {
        use std::io::{Read, Write};
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        // Nothing listens for the first attempt; a server starts during the retry pause.
        let server = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let listener = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = sock.read(&mut buf);
            let body = r#"{"ok":true}"#;
            write!(sock, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        let got = super::post_json(&format!("http://127.0.0.1:{port}/v1/chat/completions"), serde_json::json!({}), None);
        server.join().unwrap();
        assert_eq!(got.unwrap(), serde_json::json!({"ok":true}));
        // Still refused after the retry: one honest error, not a loop.
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let started = std::time::Instant::now();
        let err = super::post_json(&format!("http://127.0.0.1:{closed}/x"), serde_json::json!({}), None).unwrap_err();
        assert!(err.contains("Can't reach") && started.elapsed() < std::time::Duration::from_secs(10), "{err}");
    }
    pub(crate) fn stub_relay(replies: Vec<(u16, String)>) -> (String, std::thread::JoinHandle<Vec<(String, String)>>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/ai", listener.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for (status, body) in replies {
                let (mut sock, _) = listener.accept().unwrap();
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let (head_end, want) = loop {
                    let n = sock.read(&mut chunk).unwrap();
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..p]).to_ascii_lowercase();
                        let len = head.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
                        break (p + 4, len);
                    }
                };
                while buf.len() < head_end + want {
                    let n = sock.read(&mut chunk).unwrap();
                    buf.extend_from_slice(&chunk[..n]);
                }
                let line = String::from_utf8_lossy(&buf).lines().next().unwrap_or("").to_string();
                seen.push((line, String::from_utf8_lossy(&buf[head_end..]).to_string()));
                let reply = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                sock.write_all(reply.as_bytes()).unwrap();
            }
            seen
        });
        (base, h)
    }

    fn relay_cfg(base: &str) -> AiCfg {
        AiCfg { provider: "workers".into(), base: base.into(), model: String::new(), ..AiCfg::default() }
    }

    #[test]
    fn the_hosted_ai_goes_to_the_account_server_with_the_accounts_token() {
        let mut st = Settings { ai_provider: "account".into(), ..Default::default() };
        st.ai_base = "https://typed.example/ignored".into();
        set_account_ai(Some(("https://pool.example/".into(), std::sync::Arc::new(|| Some("tok-1".to_string())))));
        let cfg = AiCfg::from(&st);
        assert_eq!((cfg.provider.as_str(), cfg.base.as_str()), ("account", "https://pool.example/ai"));
        assert!(cfg.is_relay() && !cfg.uses_key() && cfg.has_key_or_needs_none());
        assert_eq!(request_key(&cfg).as_deref(), Some("tok-1"));
        assert!(account_ai_ready());
        // signed out: no address and no token, so nothing is sent for anybody else's account
        set_account_ai(None);
        let cfg = AiCfg::from(&st);
        assert!(cfg.base.is_empty() && request_key(&cfg).is_none() && !account_ai_ready());
    }

    #[test]
    fn a_job_is_started_followed_and_read_in_page_order() {
        let done = r#"{"status":"done","pages_done":3,"pages_total":3,"error":null,"result":{"pages":[{"page":2,"error":"model_failed"},{"page":1,"text":"ONE"},{"page":3,"text":"THREE"}]}}"#;
        let (base, h) = stub_relay(vec![
            (202, r#"{"job_id":"abc"}"#.into()),
            (200, r#"{"status":"running","pages_done":1,"pages_total":3,"result":{"pages":[]}}"#.into()),
            (200, done.into()),
        ]);
        let seen_progress = std::sync::Mutex::new(Vec::new());
        let pages: [(&[u8], &str); 3] = [(b"p1", "image/png"), (b"p2", "image/png"), (b"p3", "image/png")];
        let out = job_texts(&pages, "  Quiz 4  ", &relay_cfg(&base), &|d, t| seen_progress.lock().unwrap().push((d, t))).unwrap();
        // Page order, not the order the worker listed them in; a failed page says why.
        assert_eq!(out, vec![Ok("ONE".to_string()), Err("model_failed".to_string()), Ok("THREE".to_string())]);
        assert_eq!(*seen_progress.lock().unwrap(), vec![(1, 3), (3, 3)]);
        let seen = h.join().unwrap();
        assert_eq!(seen[0].0, "POST /ai/import HTTP/1.1");
        assert_eq!(seen[1].0, "GET /ai/import/abc HTTP/1.1");
        let sent: Value = serde_json::from_str(&seen[0].1).unwrap();
        assert_eq!(sent["hint"], "Quiz 4");
        assert_eq!(sent["pages"].as_array().unwrap().len(), 3);
        assert_eq!(sent["pages"][0]["mime"], "image/png");
        assert_eq!(sent["pages"][0]["b64"], base64_encode(b"p1"));
    }

    #[test]
    fn a_job_the_relay_refuses_or_fails_is_an_error_so_the_caller_can_fall_back() {
        let pages: [(&[u8], &str); 1] = [(b"p", "image/png")];
        // An older relay without the job routes.
        let (base, h) = stub_relay(vec![(404, r#"{"error":{"message":"not found"}}"#.into())]);
        assert!(job_texts(&pages, "", &relay_cfg(&base), &|_, _| {}).unwrap_err().contains("404"));
        h.join().unwrap();
        // A quota refusal names itself.
        let (base, h) = stub_relay(vec![(429, r#"{"error":{"code":"quota_exhausted","message":"The AI allowance for today is used up."}}"#.into())]);
        assert!(job_texts(&pages, "", &relay_cfg(&base), &|_, _| {}).unwrap_err().contains("used up"));
        h.join().unwrap();
        // A job that failed outright.
        let (base, h) = stub_relay(vec![(202, r#"{"job_id":"j"}"#.into()), (200, r#"{"status":"failed","error":"No page could be read.","result":{"pages":[]}}"#.into())]);
        assert_eq!(job_texts(&pages, "", &relay_cfg(&base), &|_, _| {}).unwrap_err(), "No page could be read.");
        h.join().unwrap();
        // No URL set.
        assert!(job_texts(&pages, "", &relay_cfg(""), &|_, _| {}).unwrap_err().contains("relay URL"));
    }

    #[test]
    fn a_page_the_job_did_not_return_counts_as_unread() {
        let job: Value = serde_json::from_str(r#"{"result":{"pages":[{"page":1,"text":"A"},{"page":9,"text":"stray"},{"page":0,"text":"zero"}]}}"#).unwrap();
        assert_eq!(job_pages(&job, 2), vec![Ok("A".to_string()), Err("no reply".to_string())]);
    }

    #[test]
    fn cfg_maps_the_relay_provider() {
        let c = AiCfg::from(&settings("workers", "", "https://relay.example/ai/"));
        assert_eq!(c.provider, "workers");
        assert_eq!(c.base, "https://relay.example/ai");
        assert_eq!(c.model, WORKERS_DEFAULT_MODEL);
        assert!(c.uses_key(), "the relay needs its relay key");
        assert!(c.key_source_help().contains("relay"));
        // The aliases map to the same provider; an empty base stays empty so chat_turns says to set it.
        assert_eq!(AiCfg::from(&settings("relay", "", "")).provider, "workers");
        assert_eq!(AiCfg::from(&settings("workers-ai", "", "")).base, "");
    }

    #[test]
    fn a_self_hosted_ollama_url_is_used_and_a_bad_one_falls_back() {
        let mut st = settings("ollama", "", "https://not-for-ollama.example/v1");
        assert_eq!(AiCfg::from(&st).base, OLLAMA_DEFAULT_BASE, "the Custom URL must not leak into Ollama");
        st.ollama_base = " http://gpu-box.lan:11434/ ".into();
        assert_eq!(AiCfg::from(&st).base, "http://gpu-box.lan:11434");
        st.ollama_base = "gpu-box.lan".into();
        assert_eq!(AiCfg::from(&st).base, OLLAMA_DEFAULT_BASE, "a URL without a scheme is ignored");
    }

    #[test]
    fn a_server_that_refuses_json_mode_is_asked_again_without_it_and_remembered() {
        let refusal = r#"{"error":"'response_format.type' must be 'json_schema' or 'text'"}"#;
        let ok = r#"{"choices":[{"message":{"content":"{\"ok\":true}"}}]}"#;
        let (base, h) = stub_relay(vec![(400, refusal.into()), (200, ok.into()), (200, ok.into())]);
        let cfg = AiCfg { provider: "custom".into(), base: base.clone(), model: "m".into(), ..AiCfg::default() };
        assert_eq!(openai_chat_once(&cfg, "p", "image/png", &[], 0.0).unwrap(), "{\"ok\":true}");
        assert_eq!(openai_chat_once(&cfg, "p", "image/png", &[], 0.0).unwrap(), "{\"ok\":true}");
        let seen = h.join().unwrap();
        assert_eq!(seen.len(), 3);
        assert!(seen[0].1.contains("response_format"));
        assert!(!seen[1].1.contains("response_format"), "the retry drops JSON mode");
        assert!(!seen[2].1.contains("response_format"), "and so does the next call");
    }

    #[test]
    fn json_mode_is_only_dropped_for_a_400_that_names_it() {
        let (base, h) = stub_relay(vec![(400, r#"{"error":"model not found"}"#.into())]);
        let cfg = AiCfg { provider: "custom".into(), base, model: "m".into(), ..AiCfg::default() };
        assert!(openai_chat_once(&cfg, "p", "image/png", &[], 0.0).is_err());
        assert_eq!(h.join().unwrap().len(), 1, "no retry for an unrelated 400");
    }

    #[test]
    fn openrouter_is_asked_not_to_reason_and_to_report_cost() {
        let ok = r#"{"choices":[{"message":{"content":"{}"}}]}"#;
        let (base, h) = stub_relay(vec![(200, ok.into())]);
        let cfg = AiCfg { provider: "openrouter".into(), base, model: "m".into(), ..AiCfg::default() };
        openai_chat_once(&cfg, "p", "image/png", &[], 0.0).unwrap();
        let body = &h.join().unwrap()[0].1;
        assert!(body.contains(r#""reasoning":{"enabled":false}"#) && body.contains(r#""usage":{"include":true}"#));
    }

    #[test]
    fn the_hosted_ai_with_nobody_signed_in_says_so_and_sends_nothing() {
        // The address comes from the signed-in account; without it the old code posted to a relative "/service".
        let cfg = AiCfg { provider: "account".into(), base: String::new(), model: "m".into(), ..AiCfg::default() };
        let err = begin_service(&cfg, "import", 1).unwrap_err();
        assert!(err.starts_with("Sign in on the Account screen"), "{err}");
    }

    #[test]
    fn a_service_session_is_started_and_rides_on_later_calls() {
        let ok = r#"{"choices":[{"message":{"content":"{}"}}]}"#;
        let started = r#"{"session":"abc123","credits":2,"left":38}"#;
        let (base, h) = stub_relay(vec![(200, started.into()), (200, ok.into())]);
        let cfg = AiCfg { provider: "workers".into(), base, model: String::new(), ..AiCfg::default() };
        let c = begin_service(&cfg, "import", 4).unwrap();
        assert_eq!((c.credits,c.left), (2,Some(38)));
        chat_turns(&c.cfg, &[Turn::new("user","p")],0.0).unwrap();
        let seen = h.join().unwrap();
        assert!(seen[0].0.contains("/ai/service"));
        assert!(seen[0].1.contains(r#""service":"import""#) && seen[0].1.contains(r#""pages":4"#));
        assert!(seen[1].1.contains(r#""session":"abc123""#));
        let (base, h) = stub_relay(vec![(200, r#"{"session":null,"credits":0,"unlimited":true}"#.into()), (200, ok.into())]);
        let cfg = AiCfg { provider: "workers".into(), base, model: String::new(), ..AiCfg::default() };
        let started=begin_service(&cfg, "import", 1).unwrap();
        openai_chat_once(&started.cfg, "p", "image/png", &[], 0.0).unwrap();
        assert!(!h.join().unwrap()[1].1.contains("session"));
    }

    #[test]
    fn a_refused_service_says_what_the_relay_said() {
        let body = r#"{"error":{"code":"insufficient_credits","message":"This needs 2 credits and you have 0 left this week."}}"#;
        let (base, h) = stub_relay(vec![(402, body.into())]);
        let cfg = AiCfg { provider: "workers".into(), base, model: String::new(), ..AiCfg::default() };
        assert_eq!(begin_service(&cfg, "import", 1).unwrap_err(), "This needs 2 credits and you have 0 left this week.");
        h.join().unwrap();
        assert_eq!(begin_service(&AiCfg { provider: "ollama".into(), base: String::new(), model: String::new(), ..AiCfg::default() }, "import", 1).unwrap().credits, 0);
    }

    fn chat_reply(text: &str) -> (u16, String) {
        (200, json!({"choices":[{"message":{"content":text},"finish_reason":"stop"}]}).to_string())
    }

    fn ditto_page(first: &str, instruction: &str, inherited: Option<u64>) -> String {
        let mut section = json!({"title":"","instructions":instruction});
        if let Some(p) = inherited { section["inheritedFromPage"] = json!(p); }
        json!({"format":"queshion/ditto","formatVersion":"0.1.0","bank":{"name":"Fractions","questions":[{"id":"q1","label":first,"type":"short-answer","stem":format!("$({first})/(5)$")}]},
            "exams":[{"name":"Fractions","sections":[section],"positions":[{"question":"q1","section":0}]}]}).to_string()
    }

    #[test]
    fn a_document_reads_each_page_with_the_earlier_directions_and_the_next_document_starts_clean() {
        let (base, h) = stub_relay(vec![
            chat_reply(&ditto_page("1", "Evaluate each expression.", None)),
            chat_reply(&ditto_page("2", "Evaluate each expression.", Some(1))),
            chat_reply(&ditto_page("1", "Find the volume of each figure.", None)),
        ]);
        let cfg = AiCfg { provider: "custom".into(), base, ..AiCfg::default() };
        let a = block_on(convert_document_async(&[vec![1], vec![2]], "", &cfg, "doc-a", &|_, _| {}));
        let b = block_on(convert_document_async(&[vec![3]], "", &cfg, "doc-b", &|_, _| {}));
        let sent = h.join().unwrap();
        assert!(!sent[0].1.contains("Earlier pages of THIS document"), "the first page of a document is read on its own");
        assert!(sent[1].1.contains("Earlier pages of THIS document") && sent[1].1.contains("printed on page 1") && sent[1].1.contains("Evaluate each expression."));
        assert!(!sent[2].1.contains("Earlier pages of THIS document") && !sent[2].1.contains("Evaluate each expression."), "nothing crosses into another document");
        let second = a[1].as_ref().unwrap();
        assert_eq!(second.doc.questions[0].stem, "Evaluate each expression.\n\n$(2)/(5)$");
        let d = second.items[0].directions.as_ref().unwrap();
        assert_eq!((d.origin.as_str(), d.from_page, d.document.as_deref()), ("inherited_from_document_context", Some(1), Some("doc-a")));
        assert!(second.context.as_ref().is_some_and(|c| c.document == "doc-a"));
        assert!(b[0].as_ref().unwrap().context.as_ref().is_some_and(|c| c.is_empty()));
    }

    #[test]
    fn a_title_that_copies_the_question_text_is_a_leaked_title_but_a_real_one_is_not() {
        let spec = |title: &str, mode: &str| crate::model::FigureSpec { id: "coordinate-grid".into(), params: [("title".to_string(), json!(title)), ("title_mode".to_string(), json!(mode)), ("xtitle".to_string(), json!("Time (s)"))].into() };
        let stem = "$y > 4x - 3$\n$y \u{2265} \u{2212}2x + 3$";
        assert!(copied_question_text(&spec("y \u{2265} \u{2212}2x + 3", "Text"), stem).is_some());
        assert!(copied_question_text(&spec("y>4x-3", "Text"), stem).is_some(), "spacing and the minus sign do not hide a copy");
        assert!(copied_question_text(&spec("y \u{2265} \u{2212}2x + 3", "None"), stem).is_none(), "a title that is switched off is not drawn");
        assert!(copied_question_text(&spec("Projectile height", "Text"), stem).is_none(), "an unrelated printed title is kept");
        assert!(copied_question_text(&spec("", "Text"), stem).is_none());
    }

    #[test]
    fn external_item_number_is_rejected_even_after_a_positive_model_review() {
        let fixture: Value = serde_json::from_str(include_str!("../tests/worksheet_banks/fixtures/figure_item_marker.json")).unwrap();
        let spec: crate::model::FigureSpec = serde_json::from_value(fixture["spec"].clone()).unwrap();
        let label = fixture["source_printed_label"].as_str();
        let mut run = crate::figure_agent::AgentRun { spec:Some(spec), png:Some(vec![1]),
            review:Some(crate::figure_agent::Verdict {matches:true,problems:Vec::new()}),..Default::default() };
        let mut executions = Vec::new();
        let rejection = record_figure_execution(&mut run,"Find the volume.",label,&mut executions).unwrap();
        assert!(rejection.contains("5)"));
        assert_eq!(executions[0].rejection_reason.as_deref(),Some(rejection.as_str()));
        let failure = figure_failure::from_run(&run,&executions,Some(&rejection));
        assert!(failure.warning("5").contains("external worksheet item number"));
        assert!(figure_request_for_item("Find the volume.",&rejection,label).contains("do not print that item marker"));
    }

    #[test]
    fn item_guard_preserves_dimensions_variables_hidden_titles_and_internal_ids() {
        let diagram=json!({"nodes":{"5)":[20,20],"B":[80,20]},"segments":[{"from":"5)","to":"B","label":"5"}],"labels":[{"at":[40,40],"text":"5 cm"},{"at":[40,60],"text":"(5)"},{"at":[40,80],"text":"x + 5"}]});
        let spec=crate::model::FigureSpec {id:"geometry-diagram".into(),params:[("diagram".into(),json!(diagram.to_string())),("title".into(),json!("5)")),("title_mode".into(),json!("None"))].into()};
        assert!(copied_source_item_marker(&spec,Some("5")).is_none());
        assert!(copied_source_item_marker(&spec,None).is_none(),"an unknown source label is never guessed from a bank offset");
        assert!(copied_source_item_marker(&spec,Some("Puzzle #5")).is_none());
    }

    #[test]
    fn title_retry_keeps_the_discarded_execution_when_the_next_run_fails_before_making() {
        struct Maker(Vec<String>);
        impl crate::figure_agent::Chat for Maker {
            fn chat(&mut self,_:&[Turn])->Result<String,String>{Ok(self.0.remove(0))}
        }
        let stem="Read the printed figure.";
        let params=json!({"title_mode":"Text","title":stem});
        let mut maker=Maker(vec![
            json!({"tool":"get_figure","args":{"id":"coordinate-grid"}}).to_string(),
            json!({"tool":"render_figure","args":{"id":"coordinate-grid","params":params}}).to_string(),
            json!({"final":{"id":"coordinate-grid","params":params,"note":"Copied a figure."}}).to_string(),
        ]);
        let mut run=crate::figure_agent::run_with_retries(&figure_request(stem,""),Some(b"source"),&mut maker,None,3);
        assert!(run.spec.is_some());
        let mut executions=Vec::new();
        let rejection=record_figure_execution(&mut run,stem,None,&mut executions).expect("copied title rejected");
        let first_turns=executions[0].attempts[0].turns.len();
        let mut failed=crate::figure_agent::AgentRun{error:Some("service start failed before source observation".into()),..Default::default()};
        assert!(record_figure_execution(&mut failed,stem,None,&mut executions).is_none());
        assert_eq!(executions.len(),2);
        assert_eq!(executions[0].rejection_reason.as_deref(),Some(rejection.as_str()));
        assert_eq!((executions[0].selected_attempt,first_turns),(1,3));
        assert!(executions[0].attempts[0].spec.is_some());
        assert_eq!(executions[1].selected_attempt,0);
        assert!(executions[1].attempts.is_empty());
        assert_eq!(executions[1].error,failed.error);
        if let Ok(dir)=std::env::var("VERSHION_TRACE_FIXTURES_OUT") {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(std::path::Path::new(&dir).join("title-retry-startup-failure.json"),serde_json::to_vec_pretty(&executions).unwrap()).unwrap();
        }
    }

    #[test]
    fn the_figure_request_marks_the_question_text_as_context_and_forbids_added_answers() {
        let r = figure_request("$y > 4x - 3$", "");
        assert!(r.contains("NOT part of the picture") && r.contains("keep blank graph axes blank") && r.contains("solution shading"));
        assert!(figure_request(crate::ditto::UNRESOLVED_TASK, "").contains("not visible on the page"));
        assert!(figure_request("x", "its title copies the stem").contains("previous figure was rejected: its title copies the stem"));
    }

    #[test]
    fn import_figures_need_render_execution_and_a_positive_review() {
        let mut run = crate::figure_agent::AgentRun::default();
        run.spec = Some(crate::model::FigureSpec { id: "coordinate-grid".into(), params: Default::default() });
        run.png = Some(vec![1]);
        assert!(!figure_import_verified(&run));
        run.review = Some(crate::figure_agent::Verdict { matches: true, problems: Vec::new() });
        assert!(!figure_import_verified(&run));
        run.steps.push(crate::figure_agent::StepLog { tool: "render_figure".into(), summary: "rendered".into() });
        assert!(figure_import_verified(&run));
        run.review.as_mut().unwrap().matches = false;
        assert!(!figure_import_verified(&run));
        run.review.as_mut().unwrap().matches = true;
        run.error = Some("renderer failed".into());
        assert!(!figure_import_verified(&run));
    }

    #[test]
    fn cfg_maps_the_three_providers() {
        let c = AiCfg::from(&settings("ollama", "", ""));
        assert_eq!(c.provider, "ollama");
        assert_eq!(c.base, OLLAMA_DEFAULT_BASE);
        assert_eq!(c.model, OLLAMA_DEFAULT_MODEL);

        let c = AiCfg::from(&settings("openrouter", "vendor/my-model", ""));
        assert_eq!(c.provider, "openrouter");
        assert_eq!(c.base, OPENROUTER_BASE);
        assert_eq!(c.model, "vendor/my-model");

        let c = AiCfg::from(&settings("custom", "", "https://api.example.com/v1/"));
        assert_eq!(c.provider, "custom");
        assert_eq!(c.base, "https://api.example.com/v1");
        assert_eq!(c.model, CUSTOM_DEFAULT_MODEL);

        // Empty or unknown provider settings fall back to local Ollama.
        let c = AiCfg::from(&Settings::default());
        assert_eq!(c.provider, "ollama");
        assert_eq!(c.model, OLLAMA_DEFAULT_MODEL);
    }

    #[test]
    fn key_help_names_the_env_var() {
        assert!(AiCfg::from(&settings("openrouter", "", "")).key_source_help().contains(OPENROUTER_KEY_ENV));
        assert!(AiCfg::from(&settings("custom", "", "")).key_source_help().contains(CUSTOM_KEY_ENV));
        assert!(AiCfg::from(&settings("ollama", "", "")).key_source_help().contains("no API key"));
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn poll_reports_a_job_that_ended_without_a_result() {
        let (tx, rx) = channel::<(AiCfg,Result<Converted, String>)>();
        drop(tx);
        let mut ai = Ai { job: Some(rx), ..Ai::default() };
        assert!(matches!(ai.poll(), Some(Err(_))), "a dropped sender is an error, not silence");
        assert!(ai.poll().is_none(), "the ended job must not stick");
    }

    #[test]
    fn poll_silently_waits_for_a_running_job() {
        let (tx, rx) = channel::<(AiCfg,Result<Converted, String>)>();
        let mut ai = Ai { job: Some(rx), ..Ai::default() };
        assert!(ai.poll().is_none());
        assert!(ai.job.is_some());
        let doc = crate::formats::detect_and_parse(b"MC\nQ?\n*a. 1\nb. 2\n", "x").unwrap();
        tx.send((AiCfg::default(),Ok(Converted { doc, warnings: Vec::new(), cropped: 0, redrawn: 0, raw_reply: String::new(), figure_runs: Vec::new(), items: Vec::new(), context: None, source_box_review:None, direction_review:None }))).unwrap();
        let _ = tx; // keep the channel alive until the reply is read
        assert_eq!(ai.poll().unwrap().unwrap().doc.questions.len(), 1);
        assert!(ai.poll().is_none(), "the job is consumed once read");
    }

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vershion-keytest-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn key_store_roundtrip_remove_and_perms() {
        let d = tmpdir("rt");
        assert!(load_keys_in(&d).get("cloud").is_none());
        set_key_in(&d, "cloud", Some("  sk-dummy-1 ")).unwrap();
        set_key_in(&d, "openrouter", Some("sk-dummy-2")).unwrap();
        let k = load_keys_in(&d);
        assert_eq!(k.get("cloud"), Some("sk-dummy-1"));
        assert_eq!(k.get("openrouter"), Some("sk-dummy-2"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let m = std::fs::metadata(d.join(KEYS_FILE)).unwrap().permissions().mode();
            assert_eq!(m & 0o777, 0o600);
        }
        assert!(!d.join("ai-keys.json.tmp").exists());
        set_key_in(&d, "cloud", None).unwrap();
        assert!(load_keys_in(&d).get("cloud").is_none());
        assert_eq!(load_keys_in(&d).get("openrouter"), Some("sk-dummy-2"));
        assert!(!format!("{:?}", load_keys_in(&d)).contains("sk-dummy"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn key_resolution_order_saved_env_file() {
        let home = tmpdir("home");
        std::fs::create_dir_all(home.join(".config/ollama-cloud")).unwrap();
        std::fs::write(home.join(CLOUD_KEY_FILE), r#"{"apiKey":"from-file"}"#).unwrap();
        let cfg = AiCfg::from(&settings("cloud", "", ""));
        let env_set = |v: &str| (v == CLOUD_KEY_ENV).then(|| "from-env".to_string());
        let env_none = |_: &str| None;
        let mut saved = Keys::default();
        // file only
        let r = resolve_key(&cfg, &saved, &env_none, Some(&home)).unwrap();
        assert_eq!((r.0.as_str(), r.1), ("from-file", KeySource::CredentialsFile));
        // env beats file
        let r = resolve_key(&cfg, &saved, &env_set, Some(&home)).unwrap();
        assert_eq!((r.0.as_str(), r.1), ("from-env", KeySource::Env(CLOUD_KEY_ENV)));
        // saved beats env
        saved.0.insert("cloud".into(), "from-app".into());
        let r = resolve_key(&cfg, &saved, &env_set, Some(&home)).unwrap();
        assert_eq!((r.0.as_str(), r.1), ("from-app", KeySource::Saved));
        // openrouter ignores the credentials file; local ollama never has a key
        let or = AiCfg::from(&settings("openrouter", "", ""));
        assert!(resolve_key(&or, &Keys::default(), &env_none, Some(&home)).is_none());
        let local = AiCfg::from(&settings("ollama", "", ""));
        assert!(resolve_key(&local, &saved, &env_set, Some(&home)).is_none());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn parses_model_lists() {
        let openai = json!({"data":[{"id":"b"},{"id":"a"},{"id":"a"}]});
        assert_eq!(parse_models("cloud", &openai), vec!["a".to_string(), "b".to_string()]);
        let ollama = json!({"models":[{"name":"z"},{"name":"y"}]});
        assert_eq!(parse_models("ollama", &ollama), vec!["y".to_string(), "z".to_string()]);
        assert!(parse_models("openrouter", &json!({})).is_empty());
    }

    #[test]
    fn why_not_names_the_reason_in_a_few_words() {
        let e = |s: u16, m: &str| format!("https://ollama.com/v1/chat/completions answered {s}: {m}");
        assert_eq!(why_not(&e(402, "This model is not in the Free plan. Pro is $20/month")), "not on your plan");
        assert_eq!(why_not(&e(403, "This model is currently being rolled out and is not yet available to you.")), "not available yet");
        assert_eq!(why_not(&e(403, "forbidden")), "not available to you");
        assert_eq!(why_not(&e(400, "this model does not support image input (ref: x)")), "text only");
        assert_eq!(why_not(&e(400, "clef:27b is a decision model and only supports /v1/systemone")), "not a chat model");
        assert_eq!(why_not(&e(401, "bad key")), "key refused");
        assert_eq!(why_not(&e(429, "slow down")), "busy, try again");
        assert_eq!(why_not("Can't reach https://x/v1/chat/completions: timed out"), "no answer");
        assert_eq!(why_not(&e(500, "oops")), "error 500");
    }

    #[test]
    fn find_models_keeps_only_the_models_that_answered() {
        let refused = |s: u16, m: &str| Err(format!("u answered {s}: {m}"));
        let c = assemble_checked(
            5,
            vec![
                ("gemma4:31b".into(), Ok(())),
                ("kimi-k3".into(), refused(402, "not in the Free plan")),
                ("beam".into(), refused(400, "does not support image input")),
                ("minimax-m3.1-flash".into(), Ok(())),
                ("clef:27b".into(), refused(400, "decision model")),
            ],
        );
        assert_eq!(c.ok, ["gemma4:31b", "minimax-m3.1-flash"]);
        assert_eq!(c.left_out[0], ("kimi-k3".to_string(), "not on your plan".to_string()));
        assert_eq!(c.left_out.len(), 3);
        let note = checked_note(&c);
        assert!(note.starts_with("2 of 5 models can read images with your account. Only those are listed."), "{note}");
        assert!(note.contains("kimi-k3 (not on your plan)") && note.contains("beam (text only)"), "{note}");
    }

    #[test]
    fn the_note_says_when_everything_answered_and_caps_the_left_out_list() {
        let all = Checked { ok: vec!["a".into(), "b".into()], left_out: vec![], total: 2 };
        assert_eq!(checked_note(&all), "2 of 2 models can read images with your account. All are listed.");
        let many = Checked {
            ok: vec!["a".into()],
            left_out: (0..7).map(|i| (format!("m{i}"), "not on your plan".to_string())).collect(),
            total: 8,
        };
        let note = checked_note(&many);
        assert!(note.contains("m3 (not on your plan)") && !note.contains("m4 (") && note.ends_with("and 3 more."), "{note}");
    }

    #[test]
    fn nothing_answering_is_an_error_that_names_a_refused_key() {
        let refused = vec![("a".to_string(), "key refused".to_string())];
        assert!(nothing_answered(3, &refused).contains("refused the key"));
        assert!(nothing_answered(3, &[("a".to_string(), "no answer".to_string())]).contains("connection"));
    }

    #[test]
    fn openrouter_modalities_and_local_capabilities_decide_vision_without_pinging() {
        let v = json!({"data": [
            {"id": "a/vision", "architecture": {"input_modalities": ["text", "image"]}},
            {"id": "b/text", "architecture": {"input_modalities": ["text"]}},
            {"id": "c/unknown"},
        ]});
        let infos = parse_model_info("openrouter", &v);
        assert_eq!(infos.iter().map(|m| m.vision).collect::<Vec<_>>(), [Some(true), Some(false), None]);
        // Only OpenRouter's listing carries modalities; other servers' flags stay unknown.
        assert!(parse_model_info("cloud", &v).iter().all(|m| m.vision.is_none()));
        let c = checked_from_flags(3, infos.into_iter().map(|m| (m.id, m.vision)).collect());
        assert_eq!(c.ok, ["a/vision", "c/unknown"]);
        assert_eq!(c.left_out, [("b/text".to_string(), "text only".to_string())]);
        assert_eq!(show_vision(&json!({"capabilities": ["completion", "vision"]})), Some(true));
        assert_eq!(show_vision(&json!({"capabilities": ["completion"]})), Some(false));
        assert_eq!(show_vision(&json!({"details": {}})), None);
    }

    #[test]
    fn the_ping_is_one_tiny_picture_and_a_short_reply() {
        let cloud = AiCfg { provider: "cloud".into(), base: "https://ollama.com/v1/".into(), model: "x".into(), ..AiCfg::default() };
        let (url, body) = probe_request(&cloud, "gemma4:31b");
        assert_eq!(url, "https://ollama.com/v1/chat/completions");
        assert_eq!(body["model"], "gemma4:31b");
        assert_eq!(body["max_tokens"], 8);
        assert!(body.pointer("/messages/0/content/1/image_url/url").and_then(Value::as_str).is_some_and(|u| u.starts_with("data:image/png;base64,")));
        let local = AiCfg { provider: "ollama".into(), base: "http://127.0.0.1:11434".into(), model: "x".into(), ..AiCfg::default() };
        let (url, body) = probe_request(&local, "m");
        assert_eq!(url, "http://127.0.0.1:11434/api/chat");
        assert_eq!(body["options"]["num_predict"], 8);
        assert_eq!(body.pointer("/messages/0/images").and_then(Value::as_array).map(Vec::len), Some(1));
        assert!(probe_png().starts_with(b"\x89PNG"));
    }
}
