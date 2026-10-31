# Hosting port plan: off Cloudflare, SQLite file on a server

Status: draft 2026-10-31, nothing built. Decided: Rust server on a VPS (see Decisions).

## What the Worker depends on today

| Cloudflare thing | Where | Replacement |
|---|---|---|
| D1 (SQLite) | every store; 118 `.batch([...])` calls rely on atomic multi-statement writes | One SQLite file, WAL mode. A batch becomes one transaction (`BEGIN IMMEDIATE ... COMMIT`). One writer process, so the "two requests racing" notes in `ai/index.ts` get tighter, not looser. |
| D1 migrations (`migrations/0001-0120`) | `wrangler d1 migrations` | Same `.sql` files, applied by a small runner that records applied names in a table. |
| R2 (`env.POOL`) | pool bodies `items/`, `quarantine/`, `blobs/`, AI job results `ai/jobs/` (put, get, delete only) | A private directory on disk, same key layout. Only three operations are used, so this is small. Back it up with the DB. |
| Hourly cron (`scheduled`) | notice backlog, `pool.hourly()`, `pruneAi`; at 04:xx UTC also `purgeExpired`, `reviewReminders`, `pool.daily()` | An in-process timer, or systemd timer calling an internal route. Keep the same two stages. |
| `ctx.waitUntil` (9 uses) | import jobs run after answering 202 | Detached task in the same process. A restart no longer kills them silently, but the stale-job check stays. |
| Secrets (`wrangler secret put`) | `AUTH_SECRET`, `POOL_HASH_KEY`, `AI_RELAY_KEY`, `OPENROUTER_API_KEY`, `STRIPE_*`, SMTP | Environment variables from a root-only file read by the service unit. |
| `[ai]` Workers AI binding | fallback upstream | Gone off Cloudflare. The relay already works with OpenRouter only (commit `f0c911d`); keeping the fallback means a second OpenRouter model or provider instead. |
| Pages `_headers` | CSP and cache rules for `web/account` | Set the same headers in the server, or in the reverse proxy. |
| Custom domain, TLS, `workers_dev = false` | edge | Reverse proxy (Caddy) with automatic TLS. The "unreachable keyless deployment" rule in the spec needs a firewall rule or loopback binding instead. |
| Rate limiting (`D1RateLimitStore`) | auth and billing | Unchanged: it is already SQLite. |
| `share-core` as wasm | argon2 hashing, validation, canonicalisation, hashing of shared questions | See below. |

Size of the port: about 21k lines of worker TypeScript plus 3.7k in `auth`, with 786 passing tests.

## Rust or TypeScript

`share-core` is already a Rust crate, built native for the app and as wasm for the Worker, with the Unicode tables pinned so both sides agree. A Rust server calls it directly, with no wasm layer, no `share-core-guard` build check, and the app and server provably running the same code. That is the real argument for Rust.

The cost is the rewrite. Everything else (auth, pool, circles, moderation, billing, AI relay, mail) is TypeScript that does not exist in Rust. Rewriting 25k lines means re-earning the tests, and the Stripe webhook and ledger code just written would be redone.

| | TypeScript server (Node/Bun) | Rust server (axum + rusqlite) |
|---|---|---|
| Rewrite | Small: swap the entry point, D1 shim, R2 shim, timer. Handlers and 786 tests mostly stay. | Large: all handlers and tests. |
| `share-core` | Keep loading the wasm (works under Node/Bun), or use napi bindings. | Direct native call. |
| Ops | Runtime plus `node_modules` to patch. | One static binary. |
| Risk | Low, incremental. | High; a long freeze on features. |

Recommendation: port to a TypeScript server first. It is the smallest change that gets you off Cloudflare, and the D1/R2 shims are the only new code. Move pieces to Rust later, one route group at a time (the AI relay and billing are the cleanest to move first), behind the same HTTP contract, if you want the single binary. That path never has a period where nothing works.

## Decisions (2026-10-31)

- **Straight to Rust** (axum + rusqlite), calling `share-core` natively. Overrides the recommendation above to port TypeScript first.
- **Runs on the user's VPS**; the Cloudflare paid account can stay for DNS and TLS in front of it if wanted.
- **Fallback:** OpenRouter first, a second OpenRouter model if it fails. The neuron conversion goes away off Cloudflare.

## Rust plan: strangler order, one HTTP contract

The Worker keeps serving until each route group has a Rust twin passing the same cases, so there is never a period with nothing working. Each phase ends with its tests green and is committed on its own.

0. **Skeleton.** New crate `server/api` in the workspace: axum, rusqlite (WAL), migration runner over `worker/migrations/*.sql`, config from environment, blob store on disk (put/get/delete), hourly and daily timers, `share-core` as a native dependency. A `/health` route. Contract tests that replay recorded request/response pairs from the Worker so both servers can be checked against the same cases.
1. **AI relay and credits** (cleanest, newest, fewest dependencies): `/ai/chat/completions`, `/ai/service`, `/ai/credits`, `/ai/import` jobs, session pricing, the ledger.
2. **Billing:** Stripe PaymentIntents, the signed webhook, refund and dispute reversal, `/billing/packs`.
3. **Auth** (3.7k lines): sign-up, sessions, refresh cookies, argon2 through `share-core`, mail, rate limits.
4. **Pool:** items, blobs, circles, moderation, invites, export, cron stages.
5. **Cut over:** copy D1 and R2 data once, repoint DNS and the Stripe webhook, retire the Worker.

Open for phase 0: which mail path to keep (SMTP or Resend), and whether the web pages are served by this binary or by Caddy.

## Steps (TypeScript route, not chosen; kept for reference)

1. Add a `D1Database`-shaped adapter over `better-sqlite3` (`prepare/bind/first/all/run/batch`). The test harness already has `d1()` in `test/pool/harness.ts`; promote it to production code. Batches become transactions.
2. Add an `R2Bucket`-shaped adapter over a directory (`put/get/delete`, with the same keys). The harness has `MemR2`; mirror it.
3. Add `server.ts`: an HTTP listener that builds the `Env` from environment variables, calls the existing `fetch` handler, and runs the two cron stages on a timer.
4. Migration runner and a first-boot check that `POOL_HASH_KEY` and `AUTH_SECRET` are set.
5. Serve `web/` statically with the `_headers` rules.
6. Deploy behind Caddy; copy the D1 data once (`wrangler d1 export`) and the R2 objects (`rclone`), then switch DNS.
7. Backups: nightly `sqlite3 .backup` plus the blob directory, off the machine.
8. Fill in the real origins (`api-base` in `credits.html`, CSP) and repoint the Stripe webhook.

## Decisions for you

- TypeScript server first, or go straight to Rust?
- Where does it run (a VPS, a home server)? That sets backup and uptime needs.
- Is the Workers AI fallback still wanted off Cloudflare? It cannot be kept as is.

## Progress log

- 2026-10-31 phase 0: skeleton (db, blobs, /health).
- 2026-10-31 phase 1 slice 1: `ai/credits.rs` (service prices, call budgets, UTC day and week). 
- 2026-10-31 phase 1 slice 2: `ai/store.rs` (usage and cost, credit ledger, sessions, refunds; start_session is one immediate transaction). 
- 2026-10-31 phase 1 slice 3: job rows and prune in `ai/store.rs` (owner-only reads, status constraint, retention). 
- 2026-10-31 phase 1 slice 4: `ai/request.rs` (data-URI checks, message parsing with the Worker's sentences, image and message caps, max_tokens and temperature clamps, error envelope). 
- 2026-10-31 phase 1 slice 5: `ai/upstream.rs` (OpenRouter call logic behind a `Transport` trait, reply shaping, retry: primary once, then the fallback model twice, or the primary again with none). 
- 2026-10-31 phase 1 slice 6: `HttpTransport` (reqwest + rustls) in `ai/upstream.rs`: key in the Authorization header only, non-2xx bodies never read, errors reduced to a class (Timeout, Connect, Decode, Request, TooLarge), tested against a local axum server. Next: the relay handler (`POST /ai/chat/completions`) with caller checks, quotas and sessions.
- 2026-10-31 phase 1 slice 7: `ai/policy.rs`, the relay's rules without HTTP: caller resolution (origin rule, relay key in constant time, keyless native app), daily-dollar and hourly quotas that fail closed, the balance, `start_service` (allowance pays first, purchased balance the shortfall, one retry then `busy`), and the session gate. Replays the Worker's credits scenarios (burn the week, shortfall, overdrawn balance, unused-session refund). Next: the axum routes (`/ai/chat/completions`, `/ai/service`, `/ai/credits`, `/ai/health`) wiring policy, request and upstream together; user authentication (pool session token) stays a stub until phase 3.
- 2026-10-31 phase 1 slice 8: `ai/routes.rs`: axum routes `/ai/health`, `/ai/credits`, `/ai/service`, `/ai/chat/completions` in the Worker's order (OPTIONS, method, caller, body, validation, session gate, quota, model call, usage booking). `Authenticator` trait is a stub (`NoAuth`) until phase 3. 57 tests. Next: `/ai/import` job routes and the background runner, then mount the AI router in `main.rs` and add the hourly and daily timers.
- 2026-10-31 phase 1 slice 9: `ai/jobs.rs` and the `/ai/import` (POST) and `/ai/import/{id}` (GET) routes: validation with the Worker's sentences, daily import and dollar caps, a spawned page-by-page runner, owner-only reads, stale detection, large results to the blob store, signed-in accounts refused (`use_service_session`). `AiState` gained `blobs` and `spawn`. 68 tests. Next: mount the AI router in `main.rs` from environment variables and run the Worker's hourly prune on a timer.
- 2026-10-31 phase 1 slice 10 (phase 1 done): `config.rs` (the Worker's var and secret names from the environment: `OPENROUTER_API_KEY`, `OPENROUTER_MODEL`, new `OPENROUTER_FALLBACK_MODEL`, `OPENROUTER_PRIVACY`, `AI_RELAY_KEY`, `ALLOWED_ORIGINS`, the `AI_*` limits incl. `AI_DAILY_USD`, `AI_JOB_TTL_HOURS`), `ai/cron.rs` (hourly prune of jobs, blob results and old usage) on a one-hour timer, and `main.rs` serving health plus the AI relay over a migrated file. 74 tests. Not yet on timers: the notice backlog, `pool.hourly`, `purgeExpired`, `reviewReminders`, `pool.daily` (phases 3 and 4). Next: phase 2, billing (`worker/src/billing/index.ts`): Stripe signature check, packs, intent creation.
- 2026-10-31 phase 2 (billing done): `billing.rs` (HMAC-SHA256 signature check with the 300 s tolerance, `/billing/packs`, `/billing/intent` priced server-side, the webhook that credits once, and reverses refunds and disputes pro rata, idempotently; DB failure is a 500 so Stripe redelivers; Stripe sits behind a `StripeTransport` trait, key only in the Authorization header), `ratelimit.rs` (the sliding-window limiter over `rate_limits`, shared with auth later, pruned hourly), `body.rs` (the shared body reader: 411/413/400/408). `BILLING_ENABLED`, `STRIPE_SECRET_KEY`, `STRIPE_WEBHOOK_SECRET`, `STRIPE_PUBLISHABLE_KEY` read from the environment in `config.rs`; billing mounted in `main.rs`. No live keys touched; tests use a scripted Stripe. 102 tests. Billing authenticates through the same stub as the AI relay until phase 3. Next: phase 3, auth (`server/auth/src`, 3.7k lines): start with the pure pieces (password and token hashing via share-core, validation).
- 2026-10-31 phase 3 slice 1: `auth/crypto.rs` (base64 and base64url codecs, random tokens, SHA-256, constant-time compare, the six HKDF-derived keys with the Worker's salt and info strings), `auth/jwt.rs` (HS256 access tokens, fixed byte-compared header, the same claim checks and 60 s skew), `auth/validate.rs` (clean_text, profile_is_safe), `auth/password.rs` (policy with the generated common-password lists, PHC encode and parse, Argon2id through `share_core::argon2id` with bounded parameters, dummy verify), 127 tests. Existing accounts' PHC hashes verify unchanged. Next: `auth/types.rs` config defaults and the store layer (users, sessions, refresh tokens) from `auth.ts`.
