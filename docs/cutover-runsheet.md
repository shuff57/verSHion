# Cutover run sheet (draft, 2026-11-03, NOT executed)

An ordered version of the cutover checklist in `docs/hosting-port.md`, with today's staging results folded in. Who: **You** = the owner, **Me** = Claude. Every step has a check and a way back. Secrets are copied by hand, never through chat or git.

## Before cutover day

| # | Who | Step | Check | Way back |
|---|---|---|---|---|
| 1 | You | Decide the production From address and the profile-through-confirmation question (see "Open decisions") | Written into the checklist | n/a |
| 2 | You | Pick a quiet hour; tell teachers about the downtime | Date set | Move the date |
| 3 | Me | Merge the figures work (branch `merge-figures` already merges cleanly into `main`; wait until the figures session stops) | `cargo test` green on the merged main; census passes | Do not push; reset the local branch |
| 4 | Me | Rebuild the web bundle (`build-web.sh`, then `deploy-web.sh`; check the wasm hash changed) and the API binary from `main` | `/health` ok; smoke test passes | Redeploy the previous binary and bundle |
| 5 | You | Fill the production `api.env`: same `AUTH_SECRET` and `POOL_HASH_KEY` as the Worker, `AI_RELAY_KEY` (new, random), SMTP, `APP_URL`, `ADMIN_EMAIL`, Stripe live keys | `GET /admin/pool/key` shows `match: true` later | Secrets stay in the Worker too |
| 6 | You | Lower the API DNS TTL to 60 s a day ahead | `dig` shows 60 | Raise it again |
| 7 | You | Add the new Stripe webhook endpoint and put its secret in `STRIPE_WEBHOOK_SECRET` | A dashboard test event is accepted; replay changes nothing | Leave the old endpoint enabled |
| 8 | Me | Restore test: restore last night's backup copy to a scratch dir, second instance on another port | `/health` ok and `match: true` | Delete the scratch dir |
| 9 | You | In D1 run `SELECT COUNT(*) FROM items WHERE fold_rev = 0 AND purged_at IS NULL`; if not 0 let the Worker's hourly cron run until it is | Count is 0 | n/a |

## Cutover day

| # | Who | Step | Check | Way back |
|---|---|---|---|---|
| 10 | You | Put the Worker route on a maintenance page (writes stop) | Writes fail, reads show the page | Remove the maintenance route |
| 11 | You | `wrangler d1 export`, then `rclone copy` of R2 to the VPS | Files present | Nothing changed yet; the Worker's D1 and R2 are untouched |
| 12 | Me | Load into SQLite, set ownership, compare row counts for the nine tables, object counts and bytes, `integrity_check`, `foreign_key_check` | All equal / `ok` / empty | Delete the VPS copy |
| 13 | Me | Start the service; check `/health`, `/admin/pool/key` (`match: true`, else STOP), sign in, open a shared item and a circle bank, an import job, a credits balance | All pass | Stop the service |
| 14 | Me | First backup before opening traffic; confirm it ships off the machine | Backup file exists off-box | n/a |
| 15 | You | Repoint DNS or the tunnel to the VPS | `/health` over the public name; TLS valid; log shows real client addresses (last `X-Forwarded-For` entry) | Repoint to the Worker (60 s TTL) |

## First hour and after

| # | Who | Step | Check |
|---|---|---|---|
| 16 | Both | Browser run on the live origin: sign up a throwaway address, open the mailed link, choose a password, sign in, reload (cookie kept), browse the pool, make a circle, share a bank | Mail reaches the inbox (not Spam); no CORS or CSP errors in the console |
| 17 | Me | `GET /admin/pool/flags` has no `cron_failure`; journal shows hourly `pool cron` lines; an admin alert reaches `ADMIN_EMAIL` | Clean |
| 18 | You | One real payment through the new Stripe endpoint, then reconcile the day's Stripe payments with `credit_ledger` | Equal |
| 19 | Me | Anonymous chat answers 401, a signed-in import costs credits, rate limit answers 429 after 5 wrong passwords | Matches staging results |
| 20 | Me | Next day: the 04:xx UTC daily run happened (`daily=true`); `pool_outbox` has no old unsent rows; disk use as planned | Clean |
| 21 | You | After a week: disable the Worker route, then the Worker, then the old Stripe endpoint (disable, not delete). Keep the D1 export and R2 bucket 30 days | n/a |

## Already proven on staging (2026-11-01 to 11-03)

Sign-up, mailed confirm, one-step password, sign-in, reload with cookie, forgot-password; approval by hand; rate limit (5 wrong passwords, then 429, other addresses unaffected); admin approval with the sudo step and its guards; AI import, answer check and edit panel on the merged build; native hosted-AI path (service session, credits charged); share gate; circles (create, code, join, add banks); archive, restore and history. See the phase log in `docs/hosting-port.md` for each result.

## Open decisions and not yet done

- Production sender address and the profile-through-confirmation choice (listed in "Open decisions" below).
- Not yet proven: a test-mode Stripe purchase; the email sign-up flow end to end on production mail; Clear history (unit-tested only); the desktop app GUI against the hosted AI (the request path is proven).
- Production needs `AI_RELAY_KEY` set (staging done), or anonymous chat stays open.
- Applicants waiting in the admin queue must be decided by an admin; nothing in the cutover approves them.
