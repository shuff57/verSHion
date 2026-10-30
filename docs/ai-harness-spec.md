# AI harness spec — doc → test bank, figures by spec

Status: first slice specified 2026-10-29, after the live probe (`docs/handoff-ai-worker-harness.md`).
Owner decisions: account `bd92…9055` (Shuff57), storage **D1 + R2**, figures **`FigureSpec` only**.

---

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
{ "status": "queued|running|done|failed", "pages_done": 3, "pages_total": 10,
  "result": { "doc": { … `ditto::import_text` wire shape … }, "warnings": ["…"] } }
```

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
| `ai/jobs/<job_id>/page-000.png` | the page the app sent (kept for re-runs; expires with the job) |
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

## 8. Acceptance

1. `POST /ai/chat/completions` with the figure agent's system prompt returns one JSON object per
   turn, 10/10 runs (the probe's `response_format` reliability, re-checked in CI).
2. A 5-question page image returns `ditto::import_text`-shaped JSON with all questions, via the job
   path, in ≤ 60 s warm.
3. `reasoning_content` never appears in a response; no key or prompt text appears in D1, R2 or logs
   (`npm test` asserts the sanitiser on a forced upstream error).
4. Over-cap requests answer 429 and do not call `env.AI.run` (asserted with a stub binding).
5. Local `wrangler dev --local` answers 503 `ai_not_configured` rather than crashing.
