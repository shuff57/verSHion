# HANDOFF 2026-11-01: the AI worker harness (relay, credits, OpenRouter)

Repo `verSHion`, branch `shared-pool-phase1`, pushed to `origin/shared-pool-phase1`.
Supersedes the review-only `docs/handoff-ai-worker-harness.md` (2026-10-28), which is now history.

**Nothing is deployed.** No staging deploy, no secret set, no migration applied outside local/in-memory
SQLite. Everything below is in git; the only live calls made were direct and relay-code-level calls to
OpenRouter from this machine.

---

## What exists

A Cloudflare Worker relay (`server/worker/src/ai/`) in front of a model, plus app-side plumbing in Rust.
It turns uploaded PDFs/images into Ditto JSON test banks and rebuilds figures from the app's own registry
as editable `FigureSpec`s.

### Upstreams (order matters)
1. **OpenRouter, paid `google/gemma-4-26b-a4b-it`** (not `:free`; the free variants were unusable: 700 s and 11/12
   failures). Reasoning off, `usage.include`, `provider.data_collection: "deny"` by default.
   `OPENROUTER_PRIVACY="0"` relaxes the privacy filter. The model and provider options are server-side; a client
   cannot choose them.
2. **Workers AI** `@cf/google/gemma-4-26b-a4b-it` as the fallback when OpenRouter fails, returns empty or times out.
   With no binding the call is tried a second time on OpenRouter. With neither configured: 503 `ai_not_configured`.

A fallback turn's neurons are booked as dollars at `0.000011` each, so one dollar cap covers both upstreams.

### Routes (`server/worker/src/ai/index.ts`)
| Route | Purpose |
|---|---|
| `POST /ai/chat/completions` | One model turn. Signed-in accounts must send `session`. |
| `POST /ai/service` | `{service, pages}`: server prices it, charges credits, opens a session with a call budget. |
| `GET /ai/credits` | Weekly balance, purchased balance, price list, reset time. |
| `POST /ai/import`, `GET /ai/import/:id` | Background import jobs. 403 for signed-in accounts (unpriced). Currently unused by the app. |
| `GET /ai/health` | `{ok, model, ai, upstream}`. |

### Credits (the AI layer; billing sits underneath)
- **40 free credits per account per UTC week**, resetting Monday 00:00 (`AI_WEEKLY_CREDITS`), then the purchased balance.
- Prices live in `server/worker/src/ai/credits.ts`: `import` 2 flat (max 20 pages, to be revisited), `figure` 2,
  `handwriting` 1 per page (each side of a sheet is a page), `alternatives` 1, `answer_key` 1, `grade` 3 (last three reserved,
  not in the app yet). Each has a call budget and a lifetime. A client names only the service and the page count.
- A session that expires with zero answered calls is refunded (not counted against the week).
- Not on credits: the relay key (`AI_RELAY_KEY`), a keyless native client, and bring-your-own-key users.
- Prices and the allowance are expected to be rebalanced after real usage. They are constants plus one env var.

### Billing (written by the `credits-billing` session, merged whole; I have not reviewed it)
Stripe PaymentIntents, a signed webhook, a buy-credits page, 25 credits per dollar, a ledger (`credit_ledger`, migration 0119).
Off unless `BILLING_ENABLED=1`. Purchased credits never expire except on account deletion; refunds and chargebacks debit the
ledger. See `docs/billing.md`. **Review this before turning it on or deploying with the routes reachable.**

### Migrations
`0116_ai_jobs`, `0117_ai_usage`, `0118_ai_sessions`, `0119_credit_ledger`, `0120_ai_openrouter_cost` (adds `cost_usd`, `session` to `ai_usage`).
Staging D1 has 0116 and 0117 only.

### Limits and env (all optional; defaults in `ai/index.ts`)
`AI_CHAT_PER_HOUR` 120 (callers without a session), `AI_IMPORT_PER_DAY` 20, `AI_DAILY_USD` 5 (account-wide), `AI_MAX_PAGES` 20,
`AI_JOB_TTL_HOURS`, `AI_WEEKLY_CREDITS` 40, `AI_MODEL`, `AI_THINKING`, `OPENROUTER_API_KEY`, `OPENROUTER_MODEL`, `OPENROUTER_PRIVACY`,
`AI_RELAY_KEY`, `BILLING_ENABLED`. Workers AI's own limit: 300 requests/min per account for text generation.

## App side (Rust)
- Providers: `cloud` (Ollama Cloud), `workers` (this relay), `ollama`, `openrouter`, `custom` (any OpenAI-compatible URL).
- `convert_pages` reads each page as its own chat request, 12 in parallel (`VERSHION_AI_PARALLEL`), with a hedge: a call still
  unanswered after 25 s is re-sent and the first usable reply wins (`VERSHION_AI_HEDGE_SECS`). The prompt asks for JSON with no
  indentation, one question per line. Result: the 4-page Ch4 quiz imports in 13–15 s, down from about 8 minutes through the job route.
- `begin_service` (src/ai.rs) starts a credit session before an import and attaches its id to later relay calls. A refused service shows
  the relay's own sentence. **Today the app authenticates with the relay key, which is not on credits, so it gets no session and nothing
  changes. Credits only bite once the app sends the signed-in user's token.**
- Settings: an editable Ollama URL (`ollama_base`), and Custom presets for LM Studio (`:1234/v1`), llama.cpp (`:8080/v1`), vLLM (`:8000/v1`),
  Ollama `/v1`. A Custom endpoint that answers 400 to `response_format` is asked again without it and remembered per process.
  Tested live: Ollama accepts `json_object`; LM Studio rejects it with 400 (fallback covers it). llama.cpp and vLLM were not installed, so untested.

## Measured (direct, 12 parallel unless noted)
| Model | Median/page | Cost/page | Battery |
|---|---|---|---|
| OpenRouter gemma-4-26b (paid) | 8.0 s (8.6 via app) | ~$0.00005 | 141/141, 106/109 |
| OpenRouter gemma-4-31b | 12.7 s | ~$0.00054 | not run |
| deepseek-v4.1-flash | 3.8 s | ~$0.00029 | 143/144, 109/109 |
| deepseek-v4.1-flash-1017 | 5.6 s | ~$0.00028 | 144/144, 109/109 |
| Workers Gemma | slower, rate-limited | neurons | 135/137, 103/107 |

Prompt was tuned on Gemma. DeepSeek is served by third-party providers: check the data terms before using it for student work.
Real model cost is tiny, so credits are about fairness and abuse control, not cost recovery.

Live relay check (this machine, in-memory DB, real OpenRouter): a picture of "4821" read correctly through the relay in 3.5 s;
no-session chat 402; start 200 (2 credits); balance correct; third import refused 402 `insufficient_credits`.

## Tests (at merge, 7743e1b)
- Worker: 786/786 (`cd server/worker && npm test`; `ai-credits.test.ts` is in the list).
- App: 2282 pass (`cargo test`), native and `wasm32` check clean. One pool test (32-12b, statement counts) failed once under load and
  passed on rerun; `pool` code is unchanged.
- Not verified live: refund of an unused session, the weekly reset, and the cost-to-dollars readback from OpenRouter (unit-tested only).

## Not done / next
1. **Staging deploy** (needs the user's go-ahead): apply 0118–0120 to staging D1, `wrangler secret put OPENROUTER_API_KEY`, deploy, then
   rerun the checks and time the deployed relay. Real-world deployed speed is unmeasured.
2. **App sends the user's token**, shows the weekly + purchased balance, and a "not enough credits" screen.
3. **Figure agent and handwriting** do not call `begin_service` yet. Only the test import does. The session id is a process-wide static, so two
   services at once would clash.
4. Alternatives, answer key and grading are priced but not built.
5. Relay hardening not done: in-flight cap with 429 backoff, a result cache by image hash, a per-user page quota beyond credits.
6. The web build lacks the parallel reader, redo and hedge. Occasional unexplained 400s were seen in early parallel runs and stopped.
7. Review the Stripe billing code before enabling it.

## Hazards
- **Do not run `cd X; git reset --hard`.** A figures builder did exactly that in this checkout (a failed `cd`), wiping all uncommitted tracked
  edits from several sessions; some were unrecoverable (the `Cargo.toml`, `model.rs`, handwriting hunk and the `mod gradeui;` line in `lib.rs`).
  `src/gradeui.rs` is committed but not declared in `lib.rs`, so it does not compile into the app. See the `feedback-guard-cd-before-reset` memory.
- **Keys.** The OpenRouter key lives only in `~/.local/share/vershion/ai-keys.json` (chmod 600). One key was pasted into chat earlier and is exposed:
  revoke it, do not use it. Never put a key on a command line or in a file in the repo.
- The edge blocks the `python-urllib` User-Agent (403); use `curl/8.0`.
- Repo lint: request bodies are read only through the pool's `readJson`/`readBody`; a `// r2-object` marker exempts non-request reads.

## Where things are
- Worker: `server/worker/src/ai/{index,store,credits,jobs,util,prompt}.ts`, billing in `server/worker/src/billing/`, tests in `server/worker/test/ai*.test.ts`.
- App: `src/ai.rs`, `src/figfit.rs`, `src/figure_agent.rs`, `src/ditto.rs`, `src/screens.rs` (Settings), `prompts/ditto-convert.md`
  (regenerate `ai/prompt.ts` with `node scripts/gen-ai-prompt.mjs`).
- Scorer: `tests/import_battery/battery.py`. Spec: `docs/ai-harness-spec.md`.
- Other sessions: `vershion-e4` owns `credits-billing` (worktree `.claude/worktrees/credits-billing`) and `startSession`/`startService`/`balance`.
