# ASVS critic: verSHion shared pool, ROUND 27 (security, critic 36d)

Read-only. The only repo file written is this report. Scratch probes live in `/tmp/c36.eKXNa4/` (`S`
below; `lib.ts` is `test/pool/round27.test.ts` with its helpers exported, the rest are the probes). Every
probe is a standalone `node:test` file against the in-process harness `test/pool/harness.ts` (share-core
pkg not rebuilt). All commands run from `/home/shuff57/Documents/GitHub/verSHion/server/worker`:

```sh
node --experimental-strip-types --no-warnings --test $S/<file>.test.ts
```

## Baseline (this tree)

- `cd server/worker && npm run typecheck`: exit 0.
- `cd server/worker && npm test`: **477/477 pass, 0 fail** (190 s; it ran while two of my probes were
  running, so it was under load, and still green).
- `cd server/auth && npm test`: **224/224 pass, 18 files**; `npx tsc --noEmit -p .` exit 0.

## (a) Previous findings re-run (35d B-1: a request authenticated before an erasure writes after it)

Method as 35d: for each route, count the statement boundaries (a lone statement or the start of an atomic
D1 batch) of one request, erase the account (`UPDATE users ... deleted_at` then
`deleteContentStatements`, as the auth package does) immediately before boundary k, settle with 20 to 100
days of hourly+daily crons, scan EVERY table for the erased id AND the name/phone/address text. A control
erases first and then sends the request.

**35d's own script, unchanged** (`S/race35.test.ts`, a copy of `c35d.6bKG/race.test.ts`; 13 scenarios):

```
publish-new   statements=58 RACE-ONLY residue tables: []
republish     57  []        get 10 []         import 23 []       prefs 6 []        report 6 []
blob          15  []        delete-own 2 []   patch 3 []         appeal 5 []       export 2 []
cron-backfill 64  []
counter-notice 4  [["retired_publish_ids",3,[0,1,2]]]
```

The one entry (`retired_publish_ids`, opaque publish ids, boundaries 0 to 2) is the setup difference
between control (item taken down, kept as evidence) and race (erased before the takedown, so the item is
removed and retired): a documented kept table, no id of the account, no text. 35d's six residues
(`imports`, `pool_prefs`, `reports`, `pool_audit`, `dmca_counter_notices`, open appeal + attestations)
are all gone.

**The suite's own race tests** (`test/pool/round27.test.ts`, 9 race scenarios + resweep + guards) are in
the green 477.

| 35d finding | Verdict |
|---|---|
| `GET /items/:id` and `import` write `imports`/`served` for the erased id | FIXED (guarded; race 0 residue) |
| `PUT /items/prefs` writes `pool_prefs` | FIXED |
| `report` writes `reports` row with note + audit | FIXED for the row, audit, holds; **see (c) 1 for one follow-up UPDATE** |
| `counter-notice` writes name/address/phone/signature | FIXED (`PII` scan `{}` in all 4 boundaries) |
| open `appeal` of an erased author blocks the purge | FIXED (no open appeal; item purged on schedule) |

Routes I added beyond 35d's list, same method (`S/sc1.test.ts` through round27's `race()`, which also
asserts "no more rows of the id than the reference, no PII text, R2 not larger, control = 401"):
`POST /items/:id/undelete`, `PATCH /items/:id` on a KEPT item, `DELETE /items/:id`,
`GET /items` + `/items/mine` + `HEAD /items/:id` + `POST /items/versions` in one request, `GET /me/export`.
All pass: no residue beyond the reference erasure, no PII text. Undelete is discussed in (c) 2.

### Completeness of the guard (focus a)

I listed every `INSERT`/`UPSERT`/`ON CONFLICT`/`UPDATE` in `src/pool/*.ts` and `src/*.ts` (283 hits) and
classified those keyed to a user id:

- **Guarded in the write** (ALIVE_SQL or `changes()` chained in the same atomic batch): `imports`, `served`,
  `pending_blobs` (3 writers), `blob_uploads`, `usage_daily` (`takeDaily`, `countMiss`, held_publishes),
  `pool_locks`, `pool_prefs`, `reports`, report holds/flags/audit, `appeal` flag + audit, counter-notice
  (action UPDATE then notice, audit, flag by `changes()`), `author_stems` backfill, `pool_limits`.
- **Not guarded, keyed to the caller, covered by the resweep or by design** (each proven with the race
  scan above or below): `publish` commit batch (`items`, `item_publishes`, `attestations`,
  `author_contents/firsts/stems`, `stem_bands`, flags with `user_id`), `deleteItem` (`carried_keys`,
  audit without note), `undeleteItem`, `patchItem` (items row, audit without note, `item_offer`,
  `content_licences`), `capAlert` (flag `cap` with the id, audit), `giveDaily`/undo (UPDATE of existing
  rows only, cannot insert).
- **Keyed to an item or an admin, not an account**: all DMCA/privacy/hold/appeal-decision admin writes,
  `pool_outbox` (payload = item/action/author ids, no email: `poolmail.ts` resolves the address at send
  time with `deleted_at IS NULL`, so no mail can reach an erased address; I confirmed the outbox row of a
  takedown of an erased author's kept item is marked sent with no recipient), `pool_kv`
  (`pool_key_id`, `key_mismatch_alerted:<kid>`, `stem_bf_*:<publish_id>`: no account), `r2_orphans`
  (`quarantine/upload/<id>`: swept in a day), `held_publishes` (the `usage_daily` row), export (stateless,
  HMAC tokens, no write).
- **Auth side** (`server/auth`): tombstone batch deletes `sessions`, `email_tokens`, `device_enrolments`
  and nulls the free text; every later writer (`login` session insert, refresh rotation insert-select
  from the deleted row, verify, reset consume, device enrolment, approval request) carries
  `deleted_at IS NULL` in its own statement (read at auth.ts 504, 675, 701, 768, 841, 918, 1015, 1120,
  1253, 1313); `authenticate` re-reads the session and user row on every request, so an access token
  does not outlive an erasure. Password reset after erasure: token deleted, consume guarded.

**Admin-mediated writes about an erased account** (`S/adm.test.ts`; Z erased, kept item, then admin
actions, 100 days of crons):

```
admin limits on erased: 200 {"ok":true}  (nothing written: pool_limits has no row of Z)
takedown of kept item: 200 ; remove after takedown: 409
tables holding erased id after admin actions: users, items, item_publishes, imports, dmca_actions, strikes,
  pool_audit(4), deletion_rewrites, erasure_done, pool_outbox(1), attestations
PII {}        outbox: takedown_author ... sent=1 (no mail: address is gone)
```
Id-only rows (strike, action, audit, outbox), no text. See (c) 4.

## (b) New-class blockers

**None found.** How hard I hunted: roughly 600 statement-boundary interleavings (13 + 9 + 6 route
scenarios, 56 boundaries on the kept-republish alone, 10 on key_leak), all four admin write families, the
whole resweep statement list, the retention/purge state machine for erased authors' items, and the three
questions of focus (b). The candidates that looked like blockers and why they are not:

### Candidate: republish of a KEPT item commits after the erasure (not a blocker)

The commit batch of `publish` has no ALIVE guard (`UPDATE items ... WHERE id AND author_id AND removed_at
IS NULL`): the kept (anonymised) item stays `removed_at IS NULL`, so a republish that took its lock before
the erasure commits after it. `S/kept.test.ts`, 56 boundaries:

```
control request status 200, boundaries 56
k=0..42  status=401 erased=true item.version=1 title="Kept"
k=43..55 status=200 erased=true item.version=2 title="Revised title Zorgon Quixote" attestations(Z)=2
LATE-COMMIT-AFTER-ERASURE interleavings: 13 of 56
```

This is a real post-erasure write, so I checked whether it is distinguishable from the lawful order
(republish, then erase). `S/kept2.test.ts`, 48+ days of crons, both orders:

```
# ctl (request, then erasure): item {"title":"Revised title Zorgon Quixote","version":2,"anonymized":1}
#   id in: users, items, item_publishes(2), imports(2), pool_audit(3), deletion_rewrites, erasure_done, attestations(2)
#   name in: items(1), item_publishes(1)         R2 keys: 2
# 50  (erasure before the commit): identical lines, byte for byte.
```

Identical end state, and the table list is exactly the documented residue (the name in a kept item's
title is the documented limit from 35c/35d; `author_contents/firsts/stems` and `stem_bands` of the late
publish are removed by the resweep, as `tables holding erased id` shows none). The hold/launder/key
analysis is the same as for the ordinary republish. So it is a serialisable order, not data resurrection.
It is not a fix of 35d (35d's republish was a non-kept item, removed by the erasure, so the commit
affected 0 rows). Non-blocker; (c) 3 suggests a one-line guard.

### Candidate: undelete / admin unremove / counter-noticed restore revive an erased author's item (not a blocker)

`undeleteItem` (`removed_reason = 'author_deleted' AND purge_after > now`) has no ALIVE guard, and the
erasure leaves `removed_reason = 'author_deleted'` on already-deleted items. `S/undel.test.ts`:

```
# control (erase, then undelete): undelete=401 item live=0 ... readerA GET=404 ... settled: purged=1 R2=0
# race   : undelete=200 item live=1 removed_reason=null anonymized=0 purge_after=null
#          readerA GET=404 inListing=false     (+1h hourly: unchanged)
#          +1d daily: live=0 removed_reason=account_deleted purge_after=+14d    settled: purged=1 R2=0
```

The row is live for up to one day, but every non-author read joins the author's standing, so no reader sees
it (404, not listed), and the daily `account_deleted` stage removes it again with a fresh 14-day purge.
Same shape for the counter-noticed takedown: `S/td.test.ts` (two taken-down items, counter-notices,
then erasure): day 14 both `account_deleted`, day 30 both purged (not resurrected). The state machine
converges; the only effect is a purge up to about 1 day later than the 14-day promise. Non-blocker.

### (b) answers to the three questions

1. **Does the resweep scrub something an erasure intentionally keeps?** No. Statement by statement it
   matches the erasure batch's own scrubs; kept anonymised items (`UPDATE` never touches `items` except
   through the existing `account_deleted` stage, which skips `anonymized = 1`), the tombstone,
   `erasure_done`, audit rows without notes, `retired_publish_ids`, attestations are untouched (the suite
   test checks them and idempotence; my kept-republish run shows `attestations(Z)=2` kept after 100 days).
   One consequence worth knowing: for an erased ADMIN account, `pool_audit.note` of every note it wrote is
   nulled (the erasure batch already does that; not new).
2. **Starvation / unbounded?** `LIMIT 50` applies only to the `imports` re-key and makes progress every run
   (the re-keyed rows leave the `IN erasure_done` set). `S/starve.test.ts`:
   ```
   N=40 : accounts with an un-rekeyed straggler after each daily run: 40 -> 0 -> 0 -> 0 -> 0 ; deleted: keys=40
   N=120: 120 -> 70 -> 20 -> 0 -> 0 ; deleted: keys=120
   ```
   No account is left behind (ceil(N/50) days). Every other statement is unbounded but indexed:
   `EXPLAIN QUERY PLAN` of all 22 statements (`S/plan.test.ts`) shows an index lookup everywhere except
   `SCAN dmca_actions` (tiny table) and `SCAN pending_blobs` (rows live 1 day); the `IN (SELECT user_id FROM
   erasure_done)` driver is the PK. Cost grows with `|erasure_done|` (forever) per day, not with request
   load; fine for D1 at this scale, worth a number in a future capacity note. All 22 are one atomic batch,
   so a single failing statement (none can fail: every JSON function is behind `json_valid`) would stop the
   whole resweep, which is isolated by `stage()` and alerts (`cron_failure`).
3. **Does re-keying `imports` change counts, licences or provenance for OTHER users?** It only
   `UPDATE ... WHERE user_id = <erased id>`; `licenses_json`, `license`, `first_at`, `author_id` are
   untouched, so no licence grant or provenance of another account changes. One cosmetic effect:
   a straggler for an item the same erased person already imported gets a SECOND anonymous key, so
   `import_count` (read.ts:684, `COUNT(DISTINCT user_id)`) is +1 for that item. Only reachable by the
   race the guard now closes (straggler rows come only from a request past its guard); non-blocker.
4. **Does auto-closing an appeal wrongly resolve another user's appeal or leave an item stuck?** The
   predicate is `kind = 'appeal' AND resolved_at IS NULL AND (user_id IN done OR item_id IN items of done)`.
   Only the author can file an appeal (`appeal()` requires `author_id = me`), so no flag of another
   account can match. It does not touch `appeal_overdue` / `appeal_escalated` flags: those stay open
   (admin-queue clutter only; they are not part of `OPEN_APPEAL_SQL`, so they block nothing). The purge
   then proceeds (`S/sc1` appeal scenario check passes: `purged_at` set, R2 empty, no open appeal).

## (c) Non-blockers (terse)

1. **key_leak `$.served` quote can survive an erasure** (incomplete fix of 27b E1, low). `report` writes the
   flag's `$.served` text AFTER its batch, as a separate unguarded `UPDATE admin_flags ... json_set(...)`
   (moderation.ts:319). If the erasure lands between the batch and that UPDATE, the report row has
   already lost its `reporter_id`, so neither the resweep (`reporter_id IN done`) nor anything else finds
   the flag. `S/kl.test.ts`, 10 boundaries, 48 days: `residue interleavings: 3 of 10` (k=7,8,9:
   `served_in_flag=true reports=[{"reporter_id":null,"note":null}]`). What stays is up to 4 KB of the
   reported item's own SERVED text (stems and choices every importer can read), unlinked to the account
   (report row anonymous). Not PII of the reporter, so not a blocker; fix: do the `$.served` write inside
   the same statement chain, or have the resweep also strip `$.served` of key_leak flags older than a day
   whose report has `reporter_id IS NULL`.
2. Undelete / admin unremove of an erased author's item makes the row live for up to a day before the
   `account_deleted` stage re-removes it (above); readers get 404 throughout. A guard `ALIVE_SQL` on
   `undeleteItem` (and on the republish commit, candidate 1) would close it for free.
3. `PUT /admin/pool/limits` for an erased user answers `200 {"ok":true}` while writing nothing: a silent
   no-op for the admin. Return 404/409 when `changes() = 0` and the user is erased.
4. Erased accounts still collect id-only rows from admin actions on their kept items: strikes,
   `dmca_actions`, `pool_outbox` (takedown_author), and `repeat_infringer` flags/admin mail ("suspend
   account X") for an account that no longer exists. No text, no address.
5. A takedown of an erased author's KEPT item that was counter-noticed before the erasure ends as
   `account_deleted` then purged at day 30, not kept anonymised (the takedown made `removed_at` non-null
   at the erasure). Consistent with the stated rule; noted for the docs.
6. A late straggler import re-keyed to a second `deleted:` key is +1 in `import_count` (b.3).
7. Racing erasure at a publish commit: a NEW item published post-erasure is `anonymized = 0`, removed by
   the next daily `account_deleted` stage and purged 14 days after that: up to 15 days, not 14 (35d saw the
   same; fine).
8. 35d's non-blockers (snapshot cross-product size, `dmca_upstream_keys` never pruned, a name typed into a
   kept item's title or tags) unchanged and not re-measured.

## (d) Not run

- Real workerd, D1 or R2, staging, any deployed cron; race windows are injected at statement and batch
  boundaries (D1 batches are atomic, so I fire before a batch, never inside one); real width not measured.
  D1 CPU/row-read cost of the resweep on a large production table (only query plans, not timings).
- A real pre-0105 database (suite's `rekeyStatements()` form only).
- Auth attack simulations beyond the package's 224 tests; I read the erasure and post-erasure writers
  (login, refresh, verify, reset, enrolment) and relied on those tests rather than re-racing them.
- Statistical timing oracle on publish refusals; rate-limit bypass by header; concurrent R2 `onPut` races
  against takedown (D1-boundary sweeps only).
- The admin routes `court-action`, `privacy-restore`, `appeal-decision` after an erasure were read, not
  raced; each acts on an item row and resolves to `account_deleted` through the daily stage as shown above.

BLOCKERS: 0
