# AI harness spec — doc → test bank, figures by spec

## Shared desktop/web engine (2026-11-15)

The current app uses one async conversion/fitting, handwriting and answer-checking workflow on both
native and WASM. Native workers drive it with `pollster`; the browser awaits `fetch`. Shared wire types,
output schemas and the bounded schema validator live in `server/ai-core`. Figure generation has the same
async local tool loop plus blocking wrappers for existing headless tools.

Each `begin_service[_async]` returns `Started`; **all later calls must use `started.cfg`**. It owns the
operation's endpoint, credential source, session and cancellation state. There is no global service session.
A Convert action opens one import session for its selected pages (up to 20 on the hosted service), reused
by transcription, repairs and figure fitting. Independent answer checks and handwriting reads own separate
sessions. Cancelling stops subsequent requests and discards late results; an HTTP request already accepted
by an upstream may still finish and incur provider cost. Speculative duplicate model calls are disabled.

`POST /ai/service` accepts an optional random 32-lowercase-hex `operation` id. The Rust server derives an
account-scoped session id and reuses it on retries. Reusing it with a different service/page count or after
expiry returns 409; it cannot charge again. The mapping uses existing session rows, so account erasure and
90-day pruning need no new tables. Legacy requests and their response shapes remain supported.

A client sending `protocol_version: 1` receives `protocol_version` and service `capabilities` containing
`structured_outputs`, `tools`, and `idempotent_service`. An older Worker returns no capabilities, causing
the client to use its existing JSON protocol. It remains deployable during the Rust-server cutover.

Settings offer verSHion AI (account credits), own-key providers, and local Ollama. Existing provider/model
settings are retained. Empty settings consistently resolve to local Ollama in UI and headless paths.
`ai_output_mode` selects compatible JSON (default), `schema`, or a forced result `tool`; `ai_native_tools`
enables native figure function calls. Browser keys remain in memory for the tab lifetime, and BYOK keys
are never automatically forwarded to the hosted relay.

Schema mode sends strict `response_format: json_schema` (or Ollama's native schema format), and result-tool
mode forces `submit_result`. The app validates strict results before importing. OpenRouter requests require
parameter support and retain the existing no-data-collection preference. JSON compatibility mode retains
the established tolerant importers. Unsupported strict options fail instead of silently downgrading on a
direct provider; older hosted deployments explicitly negotiate compatibility before the operation begins.

The Rust relay enables the new formats only with `AI_STRUCTURED_OUTPUTS=1` / `AI_NATIVE_TOOLS=1`; both are
off by default and must match the configured model's actual support. It forwards bounded schema options,
allowlisted function definitions, matching tool results and opaque `reasoning_details` for tool turns.
It still owns model selection, privacy, token limits, quotas and prices. Schema and tool turns cannot
silently fall back to a different model. Tool-only assistant replies count as answered calls and record cost.
Local figure tools validate arguments and run only registry lookups and rendering; they cannot run shell code.

Roll out the backward-compatible Rust API first, then native/web clients. Exercise the selected model in
staging before enabling either feature flag. Reset the flags and select compatible JSON to roll back without
changing banks or credentials. Durable import jobs and retiring the Worker remain part of deployment cutover.

The sections below describe the historical Worker harness and its original decisions.

Status: first slice specified 2026-10-29, after the live probe (`docs/handoff-ai-worker-harness.md`).
Owner decisions: account `bd92…9055` (Shuff57), storage **D1 + R2**, figures **`FigureSpec` only**.

---

> **2026-11-16 note: which server this describes.** The block below is the **Cloudflare Worker** (`server/worker`).
> The Rust API (`server/api`), which replaces it at cutover, has no Workers AI: its default upstream is
> CheaperInference (`CHEAPERINFERENCE_API_KEY`, model `deepseek-v4.1-flash-1017`, reasoning off, set
> `AI_UPSTREAM=openrouter` for OpenRouter), and its fallback is a second model on the same provider
> (`*_FALLBACK_MODEL`; one try on the primary, then up to two on the fallback). `/ai/health` names the provider
> actually serving. A key for the provider `AI_UPSTREAM` does not select is reported at startup, and the relay
> answers 503 `ai_not_configured`. Before the shared engine described above, the relay forwarded only
> `messages`, a clamped `max_tokens` and `temperature`; no tool or reasoning metadata crossed it.
>
> **2026-10-30 update: OpenRouter first, Workers AI as the fallback (Worker).** Secret `OPENROUTER_API_KEY`, optional
> `OPENROUTER_MODEL` (default `google/gemma-4-26b-a4b-it`), reasoning off, `provider.data_collection: "deny"`
> unless `OPENROUTER_PRIVACY=0`. A failed, empty or timed-out OpenRouter call goes to the `AI` binding
> (`AI_MODEL`, `AI_THINKING` as before); with no binding it is tried once more, then 502. Each turn's cost is
> stored in `ai_usage.cost_usd` with its session id: OpenRouter's `usage.cost` in dollars, or a fallback turn's
> neurons at $0.000011 each. The account-wide guard `AI_DAILY_USD` (default 5) replaces `AI_DAILY_NEURONS`.
> `/ai/health` names the first upstream; 503 `ai_not_configured` needs neither a key nor a binding.
> Paid credits: `docs/billing.md`.

## 1. Shape, and why it is this shape

```
desktop app (Rust, native, ureq)          worker (TypeScript, existing pool worker)      Workers AI
  figure_agent loop ── Chat ──┐            POST /ai/chat/completions  ── env.AI.run ──▶  gemma-4-26b
  import screen ──────────────┘            POST /ai/import  (job)   ── D1 + R2
                                           GET  /ai/import/:id
```

The model call is made **in the Worker**, but the **tool loop stays in the app**. The figure agent's
`Chat` trait is the only model seam, so `WorkersChat` is a transport change and nothing in
`src/figure_agent.rs` moves.

What this deliberately avoids:

| Blocker in the handoff | Why it drops out |
|---|---|
| wasm size 19.9 MiB | No Rust is compiled to wasm. The relay is TypeScript in the existing worker. |
| sync `Chat` / native-gated `ai.rs` | The app stays native; `Chat` stays sync over blocking `ureq`. |
| `to_png` needs system/asset fonts | Figures are returned as `FigureSpec`; the app renders. |
| `../viSHion` path deps | No new crate. The app and worker build as they do today. |

What does **not** drop out: the file-claim discipline (see §7) and the fact that a job — not a single
`fetch` — is the right unit for import (§3), because a cold start alone can take 36 s.

## 2. Contract A — model relay

`POST /ai/chat/completions` — OpenAI-compatible, so the existing Rust parser
(`chat_turns` → `choices/0/message/content`) and the existing `custom` provider keep working.

Request (the app sends exactly what `chat_turns` already builds):

```json
{ "model": "ignored", "messages": [{ "role": "user", "content": "…" }],
  "temperature": 0.2, "response_format": { "type": "json_object" }, "max_tokens": 1200 }
```

- `messages[].content` is a string or the OpenAI content array (`{type:"text"}` /
  `{type:"image_url", image_url:{url:"data:image/png;base64,…"}}`) — the probe confirmed the worker
  accepts the data-URI form directly, so the app's existing image encoding is unchanged.
- **`model` is ignored.** The worker always uses the model named in `env.AI_MODEL` (default
  `@cf/google/gemma-4-26b-a4b-it`). A client cannot pick a more expensive model.
- `max_tokens` is clamped to `MAX_OUT_TOKENS` (default **8192**). 2048 looked generous against the
  probe's ~750-token page replies, but with the real 7.6 KB Ditto prompt the model spends
  1600–3400 tokens per page, most of it hidden reasoning; at 2048 some pages came back truncated
  (`finish_reason: "length"`) and some came back **empty**, which surfaced as a 502. Temperature is
  clamped to [0, 1].
- **Auth has three cases.** A signed-in pool user's bearer token; the relay key
  (`wrangler secret put AI_RELAY_KEY`) that the desktop app stores under the provider `workers`;
  or — **when no relay key is set at all — a native client with no `Origin` header**. The third
  case exists because Workers AI needs no key from the caller: the binding is the credential, and
  the relay only has to stop strangers. A key is therefore needed only for a *public* relay. The
  rule that keeps case 3 safe: **a deployment without `AI_RELAY_KEY` must not be reachable**, which
  both wrangler configs enforce (`workers_dev = false`, no route). Browsers always send `Origin`,
  so a web page cannot use an unkeyed relay even if it learns the URL. `GET /ai/health` is
  unauthenticated and data-free (model name, binding present).
- The base URL the app sets is the worker's `/ai` path, e.g. `https://relay.example/ai`, so the
  app's existing `{base}/chat/completions` lands on this route.

Response — the worker returns the upstream body with two changes:

```json
{ "choices": [ { "index": 0, "message": { "role": "assistant", "content": "…" }, "finish_reason": "stop" } ],
  "usage": { "prompt_tokens": 323, "completion_tokens": 726, "total_tokens": 1049 } }
```

- **`reasoning_content` is stripped.** It is large (the probe saw 95–136 hidden tokens on trivial
  answers), it is not part of the contract, and forwarding it would double every response body.
- `usage.neurons` is not forwarded; the worker records it in its own log row (§4) for quota accounting.

Errors — OpenAI error envelope, never a raw upstream string:

| Status | Body `error.code` | When |
|---|---|---|
| 400 | `bad_request` | unparseable body, empty `messages`, oversized image (> 4 MiB decoded) |
| 401 | `unauthenticated` | no valid principal |
| 429 | `rate_limited` | per-principal bucket empty |
| 502 | `model_failed` | `env.AI.run` threw; message sanitised (no keys, no prompt echo) |
| 503 | `ai_not_configured` | no `AI` binding (local `wrangler dev --local`, or a deploy without `[ai]`) |

Limits (from the probe, plus headroom):

- Text turn: ~2 s warm. Vision turn: ~1.7 s warm. **Cold start: up to 40 s.**
- The app's ureq client must allow ≥ 60 s for one turn. One figure run is ≤ 8 turns —
  the app already budgets per-step, so the relay never sees a request longer than one turn.

## 3. Contract B — import jobs

`POST /ai/import` → `202 { "job_id": "…" }`

```json
{ "hint": "Blackboard quiz, 10 questions", "pages": [ { "mime": "image/png", "b64": "…" } ] }
```

- Pages are **rasterised by the app** (as the Import screen already does) and sent as PNG bytes.
  This sidesteps the known `VERSHION_CONVERT=<file.pdf>` defect (raw PDF bytes sent as `image_url`).
- Bounds: ≤ `MAX_PAGES` (default 20), ≤ 4 MiB decoded per page, ≤ 12 MiB per request.

`GET /ai/import/:id` →

```json
{ "status": "queued|running|done|failed", "pages_done": 3, "pages_total": 10, "error": null,
  "result": { "pages": [ { "page": 1, "text": "<the model's Ditto JSON text>" }, { "page": 2, "error": "model_failed" } ] } }
```

- The worker returns each page's **raw Ditto text**, not a parsed document: parsing and picture
  cropping need the page raster, so the app feeds `text` to its own `ditto::import_text`. One parser.
- Only the job's owner can read it; anyone else gets the same 404 as for a missing id.
- Work runs in `ctx.waitUntil`, one page at a time (the model is the bottleneck, not the worker).
- **Why a job:** 12–15 s/page warm, so 10 pages is 2+ minutes — beyond a single `fetch`, and the
  first call may add 36 s of cold start.
- Results are stored in D1 (`ai_jobs.result` as JSON text) up to 256 KB, and **in R2** above that
  (`ai/jobs/<id>/result.json`). The response always returns the full `result`.

## 4. Storage

D1, in the existing `vershion-pool` database. Worker-owned migrations currently end at `0115`, so the
next two are `0116` and `0117`:

```sql
CREATE TABLE ai_jobs (            -- migration 0116
  id          TEXT PRIMARY KEY,   -- 16 random bytes, hex
  principal   TEXT NOT NULL,      -- auth principal id
  created_at  INTEGER NOT NULL,   -- unix ms
  updated_at  INTEGER NOT NULL,
  status      TEXT NOT NULL,      -- queued | running | done | failed
  pages_total INTEGER NOT NULL,
  pages_done  INTEGER NOT NULL DEFAULT 0,
  hint        TEXT,
  error       TEXT,               -- sanitised, never a key or a prompt echo
  result      TEXT                -- JSON, or NULL when the result lives in R2
);

CREATE TABLE ai_usage (           -- migration 0117: one row per relayed turn
  id         TEXT PRIMARY KEY,
  principal  TEXT NOT NULL,
  at         INTEGER NOT NULL,
  kind       TEXT NOT NULL,       -- chat | import_page
  in_tokens  INTEGER NOT NULL,
  out_tokens INTEGER NOT NULL,
  neurons    REAL NOT NULL
);
CREATE INDEX ai_usage_principal_at ON ai_usage(principal, at);
```

R2 (binding `POOL`, existing private bucket, served only through the Worker):

| Key | Contents |
|---|---|
| `ai/jobs/<job_id>/result.json` | result when the D1 row would exceed 256 KB |

Retention: jobs deleted after `AI_JOB_TTL_HOURS` (default 72) by the existing hourly cron.
`ai_usage` is kept 90 days for quota, then pruned.

## 5. Quota and abuse

Chosen model billing is the account's, so an open endpoint is a drain risk. Per principal:

- `AI_CHAT_PER_HOUR` (default 120 turns), `AI_IMPORT_PER_DAY` (default 20 jobs).
- Rate Limiting binding where available, `ai_usage` counting as the durable fallback.
- Hard cap `AI_DAILY_NEURONS` (default 50 000 ≈ 2 000 pages at the measured 23 neurons/page); above
  it the relay answers 429 `quota_exhausted` until UTC midnight.
- No BYOK: the UI must say inference is billed to the account owning the binding and that a pasted
  Cloudflare token is **not** used. Keys are never written to D1, R2, logs or error strings.

## 6. Figures

The relay is the whole figure story on the server: it returns text, and the app's
`figure_agent::run` asks for one JSON command per turn (`list_figures` / `get_figure` /
`render_figure`). `render()` produces `(FigureSpec, PNG, notes)` **client-side**.

- The worker stores nothing about figures in v1.
- If a headless batch is ever needed, that is a second phase and brings back the wasm/font blockers.

## 7. Rollout and file claims

The D1 session's claim list (`HANDOFF.md`) has been quiet since 2026-10-25/28 — no file mtime in this
group is newer than 2026-10-28 20:11. This slice touches only:

- `server/worker/` — `wrangler.toml`, `wrangler.staging.toml`, `src/index.ts`, new `src/ai/`, tests.
  Last worker edit was 2026-10-23; the grade session never touched it.
- **No Rust source, no `Cargo.toml`, no `src/store.rs`.** The app-side `WorkersChat` is a later
  slice and is a new module plus one `mod` line in `src/lib.rs`.

Order: relay route + tests (this slice) → app `WorkersChat` + provider row → import job route →
app Import screen wired to jobs → figure agent pointed at the relay.

### Slice 1 — done 2026-10-29

- `server/worker/src/ai/index.ts` — `POST /ai/chat/completions`, authenticated, model pinned by
  `AI_MODEL`, `reasoning_content` and `usage.neurons` stripped, upstream errors never echoed.
- `[ai] binding = "AI"` in `wrangler.toml` and `wrangler.staging.toml`; `wrangler deploy --dry-run`
  lists `env.AI` alongside `env.DB` and `env.POOL`.
- 13 tests in `test/ai.test.ts`; full worker suite **717/717**, `npm run typecheck` clean.
- The body is read by the pool's guarded `readJson`: the I4/I5 lint refuses any direct body read in
  `src/`, and the first draft of this route failed exactly that lint. Tests must send an exact
  `Content-Length`, as every real client does.
- Still open: per-principal quota and `ai_usage` (§5) and the import job endpoints (§3). Until §5
  lands, the route is authentication-gated but not rate-capped.

### Slice 2 — app side, done 2026-10-29

- `AiCfg` gained the `workers` provider (aliases `workers-ai`, `relay`): base is the URL the user
  sets (`https://relay.example/ai`), model is display-only because the worker pins it, and the
  key is the relay key (`VERSHION_RELAY_KEY`, or the one saved in Settings). Listing/ping go to
  `{base}/health`, never `/models`.
- Settings: a "verSHion relay" chip, a Base URL field, and the key field labelled "Relay key".
- Import: `convert_page` now calls `figfit::fit_with_retries(…, 3)`. A picture the first generator
  misses is retried up to 3 times, each attempt told what the last one got wrong (no generator
  named, generator that cannot draw the parameters, or a redraw that did not match after three
  parameter repairs). The wasm twin `fit_async_with_retries` matches it.
- Tests: `cargo test --lib` **2273 passed, 0 failed, 6 ignored**; wasm32 check clean; worker
  **720/720**. New tests cover provider mapping, the retry loop (early stop, tries honoured,
  misses vs. errors) and the relay key.
- **Verified live, 2026-10-29:** `wrangler dev --remote --config wrangler.staging.toml` on the
  real account, then `POST /ai/chat/completions` from this machine with no key and no `Origin`:
  **200**, `{"tool":"list_figures","args":{}}`, no `reasoning_content`, 3.8 s. The same call with a
  browser `Origin` answered **403**. `/ai/health` answered `{"ok":true,"ai":true}`.
- **Not deployed.** Staging's live version (`05465e46`, 2026-10-28) predates `/ai/*`, and the
  worker has no public URL. Production `vershion-pool` does not exist.

### Slice 3 — the AI loop test, and the two defects it found (2026-10-29)

`tests/ai_loop/make_pages.py` writes one page per question kind (mc, tf, blank, short, matching,
multipart, plus a labelled-triangle figure page) and `quiz_all_types.pdf`. `tests/ai_loop.rs` is
opt-in (`VERSHION_RELAY_BASE=… cargo test --test ai_loop -- --ignored --nocapture`) and runs the
binary's `VERSHION_IMPORT_PDF` hook against the relay, in its own store, asserting one question per
page, all six kinds present, and an editable figure from the diagram page.

The first real runs found two defects that unit tests could not:

1. **`preReadBody` capped every non-pool route at 8 KB**, so the first page image died at
   `413 body_too_large` before the relay saw it. Fixed by adding `ai` to the paths that read their
   own body, like `/items`, `/blobs`, `/circles`.
2. **`MAX_OUT_TOKENS` at 2048** truncated pages (`finish_reason: "length"`) and produced empty
   completions on some, which the app reported as "The model returned no text". Raised to 8192;
   the relay now also retries once on an empty completion, and the app retries an empty reply the
   same way it retries invalid JSON.
3. The relay reads its body under a per-request key: a PDF import converts pages in parallel, and
   the pool's per-account cap (four bodies at once) refused the fifth page with 429.

Result of the sequential per-page run through the app's real path (`VERSHION_CONVERT`, own store,
live relay), 2026-10-29:

| Page | Parsed as | Time |
|---|---|---|
| mc | multiple-choice | 32 s |
| tf | true-false | 54 s |
| blank | fill-in-the-blank ×2 | 222 s |
| short | short-answer | 41 s |
| matching | matching | 72 s |
| multipart | multipart | 42 s |
| figure | short-answer | 77 s |

All seven pages produced exactly the expected kind(s). Times are wall-clock including hidden
reasoning and vary with upstream load (a parallel burst pushed single calls to 127–263 s), so the
import path is throughput-bound, not stuck.

The figure page went further: the import **cropped one picture** from it, then the fitting chain
reported `figure fitting failed (… Network connection lost)` and, correctly, kept the cropped image
with that note. The three attempts all lost the connection: `wrangler dev --remote` drops requests
that run for minutes, which the model now does under load. The crop half and the fallback are proven
live; the generator-fitting half is covered by `retry_fit` unit tests but has **not** completed
against a live model yet. A real deployment (rather than the dev tunnel) is the way to finish it.
- The app has not yet been driven through the GUI against the relay; the proof above is the
  endpoint the app calls, not a click-through.

### Slice 4 — quota and import jobs, done 2026-10-30

- **Quota** (`ai_usage`, migration `0117`): per principal `AI_CHAT_PER_HOUR` (120) answered chat turns,
  per principal `AI_IMPORT_PER_DAY` (20) jobs, and the account-wide `AI_DAILY_NEURONS` (50 000) from UTC
  midnight. Over a limit: 429 `rate_limited` or `quota_exhausted` with `Retry-After`, before any model
  call. Counted from usage rows, so two requests racing at the line can both pass: a budget guard, not
  an exact meter. If the quota cannot be read (migrations not applied) the relay **fails closed**: 503
  `quota_unavailable`.
- **Jobs** (`ai_jobs`, migration `0116`): `POST /ai/import`, `GET /ai/import/:id`, as in section 3. Pages
  run one at a time in `ctx.waitUntil`; a failing page is reported on that page and the job still
  finishes; a job that has not moved for 10 minutes reads as failed ("interrupted"). The quota is
  re-checked before every page.
- **Deviation from the plan:** the uploaded pages are **not** kept in R2 (they live in memory for the
  run); only a result above 256 KB goes to R2. Re-running a job means uploading it again.
- The worker's prompt is generated from `prompts/ditto-convert.md` (`node scripts/gen-ai-prompt.mjs`);
  a test fails if they differ.
- Hourly cron prunes jobs past `AI_JOB_TTL_HOURS` (72) with their R2 results, and usage past 90 days.
- Not yet: the app does not call the job endpoints (it still converts page by page through the relay),
  and migrations `0116`/`0117` are not applied to any remote database.

## 8. Acceptance

### Worksheet figure failure reasons

New worksheet inference runs and independent source audits target **98%**. Earlier
frozen rounds keep their recorded 95% gates; missing or uncertain items remain in
the denominator at either threshold.

An import that keeps a source bitmap must explain why it did not attach an editable redraw.
The import preview retains every conversion note in the source details; a transient toast
is not the only place to read the reasons. A failed comparison shows the reviewer's actual
problem list, rather than the maker's description of its drawing or compromise.

`FigureImportTrace.failure` records a reason kind, detail, candidate generator ID, whether
that ID is registered in Figures, and the reviewer's concrete problems. Source-reading or
scope holds, generation/tool/service errors, unavailable comparisons, rejected comparisons,
storage errors and unverified outputs are distinct outcomes. Printed item labels remain in
the displayed notes; document page offsets must not renumber a failure.

The harness reports `figure_failures` by source question number, including retained crops
and errors from every recorded attempt. It can recover comparison reasons from older
traces without new inference. These diagnostic fields do not change the acceptance gates.
A declined request or a rejected render does **not** prove that Figures lacks a type or
feature. Confirm a catalogue or parameter limitation against the registry/schema, and
distinguish it from a supported option the model did not use. Independent source-fidelity
failures of model-approved redraws remain in the separate council audit.

The source-description reader reports format/schema/structural-contract errors separately
from unresolved visual evidence, bad image input and failed requests. Text-length failures
name the field, actual character count and bound without echoing the source text. Explanatory
topology reasons allow up to 512 characters; required status and unknown-join checks remain.

Source cropping follows dark ink connected to the proposed box perimeter, including
single-pixel axis shafts and diagonal strokes, within the existing 2.5% page margin.
It adds only a few clean paper pixels and does not jump a paper gap into nearby text.
This is a crop repair, not independent certification that the proposed source scope is complete.

Eligible questions without task mappings can receive one separate current-page source-only
task read, including bare expressions and prose. It may add only literal printed directions
and application-key mappings; existing mappings and question/answer/figure payloads stay
protected. Explicit absent tasks can proceed to same-document continuation review, while
uncertain or failed current-page readings block inheritance for their targeted items and
produce a review warning. Exact qualifiers remain part of the task. This one request is
separate from the existing bounded continuation and figure-observer calls.

Source-scope evidence distinguishes a known own external question marker from diagram
givens. A marker fragment alone is not clipped diagram ink; unknown ownership and actual
clipped labels/arrows remain strict. A clipped-given proposal that repeats current bounds
can reconsider them in the existing second proposal slot, shared with format recovery.
Changed crops must still be shown again and confirmed. Negative model decisions are
preserved, not filtered or automatically promoted.

An unresolved source reading can use the existing second-call allowance with a fresh
instruction that excludes the first reading's claims and inventory IDs. The full source
stays first; optional upper/lower panels contain overlapping pixel-preserving corner
views. Their rectangles, integer scales, order, hashes and bounded fallback are traced.
No extra model call, fabricated ink or default topology is supplied, and genuine
uncertainty still retains the source crop for review.

Native grid/scatter previews can report their own emitted vector geometry to the maker:
grid/axis extents and endpoint choices, or finite-axis clearances and authored marker groups.
The report is bound to the preview image and is candidate-only feedback. It does not
certify raster visibility, infer source data or enter the independent image judge's prompt.
Source fidelity, the acceptance threshold and independent audit requirements are unchanged.

Source observation v4 records nullable Cartesian plot assertions and an optional attention
box explicitly (required nullable fields, no parser-inserted defaults). Node position prose
allows 256 characters. A valid attention box can give the existing second read one bounded,
integer-repeated focus image beside the unchanged full source; invalid/null attention retains
the quadrant/full-only fallback. It never supplies a join decision or adds model calls.
Plain finite axis ends, including a first-quadrant plot's shared origin corner, are
recorded as lines. The ambiguous legacy source-end value "none" is not accepted by the
new observation schema; old assertions using it are unsupported by the application
comparison, never silently converted into a source fact.

Native figure sessions receive native function-call instructions rather than conflicting
JSON-command examples. A malformed native reply can receive at most two repair requests
inside the existing turn limit; the reply and error stay in the trace. No content-only
JSON is executed as a substitute for a native tool call, and transport/auth errors are
not protocol repairs. A failed attempt's retained preview diagnostic without a final
spec/PNG is reported as unavailable binding evidence, not as a proven hash mismatch.

For native plot previews, applicable source assertions can be compared conservatively with
the emitted candidate geometry. Known contradictions are shown to the maker and block a
conflicting final, including a turn-limit fallback, before any image judge runs. The check is
bound to the final PNG and normalized spec; all attempts retain it without fabricating a
judge verdict. Source assertions remain unverified. Missing, unsupported, subpixel or
raster-ambiguous evidence is reported explicitly and cannot certify a match. Independent
source review and the unchanged 98% benchmark remain necessary.

The coordinate-grid tool has independent axis extensions, periodic emphasized gridlines,
and dot radius/outline controls. Defaults preserve previous drawings. These controls change
only requested strokes/markers; they do not alter data, add lattice lines or solve a question.
Unsafe extension canvases return a render error rather than a silently fitted drawing.

For source copies, coordinate-grid's effective answer-toggle default is off when
omitted, null or invalid. Its source-specific schema describes that default; an
explicit boolean remains the caller's choice. Preview, final and fallback use the
same normalized parameters. Other generation contexts and the Figures tab retain
their existing defaults.

Direction-field preview feedback can describe clipping of the candidate's actual
emitted marks against its own viewport. Counts and bounded samples are advisory
vector geometry, separate from stroke-envelope uncertainty and source fidelity.
They do not enlarge a window or reject a source by themselves. Maker and judge
instructions preserve whole supplied marks at all four borders; partial field
segments are not treated as cosmetic axis-tail styling.

An explicit direction-field stroke clipping margin can retain complete outer marks
without changing the axes or sampling rows. Its default is zero and preserves old
drawings. The maker must choose it from visible source evidence; it is never fitted
automatically. Widening an axis range still changes the implicit sampling lattice.
Clipping feedback also gives a conservative total/additional SVG-pixel margin for
the candidate's own emitted centerlines and round-cap envelopes. This is actionable
candidate geometry, not a source measurement or automatic parameter correction.

Scope review can append a separate application coordinate guide with normalized
page rulers. The clean original and crop sheet remain unchanged; the guide adds
no suggested bounds or source facts and is never passed to a figure maker,
observer or judge. It does not add review calls or waive negative confirmations.
The trace distinguishes this guide's page rectangle from nominal crop bounds and
records derivation limits rather than inventing effective extraction rectangles.

Polar-grid accepts explicit cubic source paths as well as sampled polylines.
Controls and endpoints stay authored in graph units; no curve is inferred,
automatically fitted or closed. Empty cubic paths preserve existing drawings.
Pure polar grid guides are excluded from structural-solid inventories; genuinely
unreadable supplied geometry and non-guide rays still require source review.

Geometry feedback inventories authored stroke patterns and conditional endpoint
proximities. A thin gray solid line does not stand in for a dashed source edge, and
a branch near a side is not automatically joined to it. These reports do not snap
nodes, infer hidden edges or certify a source match. Shared source junctions must
be represented with shared nodes or explicit line ports.

Native render/final calls require an object-valued `params`, with a precise type
error for stringified outer objects. Nested drawing objects avoid repeated JSON
escaping. Invalid calls remain unexecuted and retain the same bounded repairs.
Failed imports retain attempt errors alongside the selected comparison rejection,
so later provider cut-offs or protocol errors do not disappear from the reason.

Figure comparison keeps the complete candidate and source as images 1 and 2. When bounded
detail images can be made, images 3 and 4 show only integer-repeated original pixels from
each image's left and bottom edges. The trace records original dimensions, source/output
rectangles, scale and hashes. Optional panels fall back to the full pair for invalid,
unsupported or oversized input; no observer text is given to the reviewer. This adds image
inputs, not another review request. Strict source givens remain the gate; small font,
arrowhead and unlabelled axis-tail styling alone does not change a numeric range.

1. `POST /ai/chat/completions` with the figure agent's system prompt returns one JSON object per
   turn, 10/10 runs (the probe's `response_format` reliability, re-checked in CI).
2. A 5-question page image returns `ditto::import_text`-shaped JSON with all questions, via the job
   path, in ≤ 60 s warm.
3. `reasoning_content` never appears in a response; no key or prompt text appears in D1, R2 or logs
   (`npm test` asserts the sanitiser on a forced upstream error).
4. Over-cap requests answer 429 and do not call `env.AI.run` (asserted with a stub binding).
5. Local `wrangler dev --local` answers 503 `ai_not_configured` rather than crashing.
