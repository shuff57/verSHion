# ASVS critic: verSHion shared pool, ROUND 26b (security, critic 35d)

Read-only. The only repo file written is this report. Scratch probes are in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c35d.6bKG/`
(`S` below). The earlier critics' scratch dirs no longer existed, so every earlier repro was re-derived
from the report text and from the suite's own tests (standalone `node:test` files against the in-process
harness `test/pool/harness.ts`, share-core `pkg` not rebuilt). Every command is run from
`/home/shuff57/Documents/GitHub/verSHion/server/worker` as:

```sh
node --experimental-strip-types --no-warnings --test $S/<file>.test.ts
```

## Baseline (run alone, this tree)

- `cd server/worker && npm run typecheck` exit 0.
- `cd server/worker && npm test`: **466/466 pass, 0 fail** (270 s).
- `cd server/auth && npm test`: **224/224 pass, 18 files**; `npx tsc --noEmit -p .` exit 0.

## (a) Previous findings re-run

| Finding | Verdict | Evidence |
|---|---|---|
| 34d B-1: erasure lands during the stem backfill's R2 read, then the backfill INSERTs | **FIXED** | `$S/toc.test.ts`: `# during-r2-read: stems before=8 fired=true after hourly= 0` and the control `# before-cron: stems before=8 fired=false after hourly= 0`. The hook fired on the hidden body's `full_key` (the 34d interleaving), and no `author_stems` row came back. |
| 34d B-1, wider form: the cron racing an erasure at EVERY statement boundary | **FIXED** | `$S/race.test.ts`, scenario `cron-backfill` (hourly, then a day later hourly+daily, 63 boundaries): `RACE-ONLY residue tables: []`. |
| 34d B-1, "broader instance": a publish already authenticated, racing an erasure | **FIXED for publish** | `$S/race.test.ts` `publish-new` (58 boundaries) and `republish` (57): no race-only residue. `$S/del.test.ts` `OPS=erase`: 56 interleavings, the racing republish answers 409 or 200 and the item is hidden in all of them (0 violations). **But the same shape on the other user routes is not fixed: see B-1 below.** |
| 24 B1: author's own original held on its 2nd/3rd ordinary republish | **FIXED** | `$S/prev.test.ts`: 2 block hashes recorded; `republish 1: holds []`, `2: []`, `3: []`. |
| 24 B2: author reads/imports an accomplice's copy, shields it from hold-copies | **FIXED** | `$S/prev2.test.ts`: `control`, `get` and `import` all give `hold-copies 200 {"ok":true,"held":["<Bob's copy>"],...}` (identical). |
| 24 B3: migration 0105 loses a window | **FIXED** | `node ... --test --test-name-pattern="licen|0105|rekey|re-key" test/pool/round25.test.ts`: 25-3, 25-4, 25-8 pass; round-25 tests 25-1 and 25-2 (the same two mechanisms as my re-derivations) are in the green 466. I did not re-derive the pre-0105 database (see (d)). |

## (b) New-class blockers

### B-1: a user request already authenticated when the account is erased commits its write after the erasure; the erasure sweep and the cron never remove it (data resurrection, three PII variants)

**Class.** Data resurrection (erasure). It is NEW as a class of writer, not an incomplete fix of the
34d cron: 34d's repair (the guard inside the `INSERT ... SELECT`) closed the cron writers and the
`publish` routes, but the user-facing routes that write rows keyed to the caller (`GET /items/:id`,
`POST /items/:id/import`, `PUT /items/prefs`, `POST /items/:id/report`, `POST /items/:id/appeal`,
`POST /items/:id/counter-notice`) check nothing about `users.deleted_at` in their write, and the
`erasure_resweep` stage (cron.ts:173-187) covers only 7 tables (`author_contents`, `author_firsts`,
`author_stems`, `stem_bands`, `carried_keys`, `blob_uploads`, `served`). Erasure in production is two
separate batches (auth tombstone, then `onDelete` pool batch), and any request authenticated before the
tombstone keeps running, so a teacher who taps "delete my account" while the app has a report, an
appeal, an import or a counter-notice in flight is the honest case. No attacker is needed.

**Method.** `$S/race.test.ts`: for each route, count the boundaries (a standalone statement, or the
start of an atomic D1 batch) of one request; for each boundary k, run the account erasure
(`UPDATE users ... deleted_at` + `db.batch(deleteContentStatements(...))`, exactly as the auth package
does) immediately before boundary k; then settle (20 days of hourly+daily crons) and scan EVERY table
for the erased id and the account's name. A control erases first and sends the request after (401,
nothing written). Residue present only in the race is reported.

```
# get:            race-only residue: imports          (10 of 10 boundaries)
# import:         race-only residue: imports          (11 of 23 boundaries)
# prefs:          race-only residue: pool_prefs        (6 of 6)
# report:         race-only residue: reports, pool_audit (6 of 6)
# counter-notice: race-only residue: dmca_counter_notices (3 of 4)
# appeal:         race-only residue: item_publishes, attestations (5 of 5)
# control for every route: users, pool_audit(1), deletion_rewrites(2), erasure_done(1) only
```

Routes with no race-only residue (so the sweep is not vacuous): publish-new, republish, blob upload,
DELETE item, PATCH item, `/me/export`, cron backfill.

**Exact outputs after the erasure, 40 days of crons** (`$S/b2.test.ts`, one statement-level
interleaving per route, with the control):

```
# counter control (request after erasure): status 401   -> counter: []
# counter race: status 202 -> after 40 days:
  {"counter":[{"name":"Zorgon Quixote","address":"12 Elm St Chico CA","phone":"5305551212","signature":"Zorgon Quixote"}]}
# report  control: 401 -> reports: []
# report  race: 202 -> {"reports":[{"is_erased_id":1,"note":"my note Zorgon Quixote"}]}
# get     control: 401 -> imports_with_erased_id: 0
# get     race: 200 -> imports_with_erased_id: 1
```

`deleteContentStatements` scrubs `dmca_counter_notices` and `reports.note` only for rows that exist when
it runs, and re-keys `imports` once, so none of these rows is ever touched again. The counter-notice row
is a real name, postal address, phone number and signature of a person who asked to be erased.

**The appeal variant is worse than residue** (`$S/b1.test.ts`): the appeal inserted after erasure is an
OPEN `appeal` flag. The purge query skips an item with an open appeal (`OPEN_APPEAL_SQL`), so the erased
account's removed item, with its full hidden-key body in R2, is not purged on the promised 14-day
schedule:

```
# control (erasure, then appeal -> 401):
#   day 20  item purged=1  publishes 0  R2 objs 0
# race (appeal authenticated, erasure, appeal commits -> 202):
#   day 1   purged=0 publishes 1 R2 objs 2 appeal flag open, note "please restore, call me Zorgon Quixote 5305551212"
#   day 20  purged=0 publishes 1 R2 objs 2 (appeal still open)
#   day 70  removed_reason=account_deleted purged=0 (flag auto-resolved at the 60-day appeal rule)
#   day 400 purged=1; the flag's detail still holds the note text and the phone number
```

So the erased author's items and answer keys outlive the 14-day promise by roughly 60 days, and the
free-text note (name and phone number) is never removed (`deleteContentStatements` strips `$.note` from
flags that exist at erasure only).

**Why it counts.** "Data loss or resurrection (erasure)" is on the convergence list; the README's
claim after 34d (nothing re-creates an erased account's records) is false for these six routes, and
the mechanism is concurrency, the same family as 34d. Severity: Medium (milliseconds-wide window, honest
user, but the residue is PII and permanent, and the appeal variant also delays a purge of hidden keys).

**Fix direction.** (1) Put the guard in the write: add
`WHERE EXISTS (SELECT 1 FROM users WHERE id = ?me AND deleted_at IS NULL)` (or `AND changes() = 1`
chained) to the `INSERT`s in `recordServed` (imports/served), `putPrefs`, `report`, `appeal`,
`counterNotice` (both the counter-notice INSERT and its `admin_flags` INSERT), and the usage counters.
(2) Extend `erasure_resweep` to `imports` (`UPDATE ... user_id = 'deleted:...'`), `pool_prefs`,
`reports` (note NULL, reporter_id NULL), `dmca_counter_notices` (scrub where the action's author is in
`erasure_done`), `admin_flags` (strip `$.note`/`$.statement`, close open appeals of an erased user),
`usage_daily`, `pool_limits`, `pending_blobs`. Both together, as 34d recommended for the cron.

**Control for the method.** Erasure at every boundary of `publish` leaves nothing extra (publish IS
guarded), and the post-erasure request is refused (401) in every control, so the residue is produced
only by the straddling request, not by the sweep's bookkeeping.

## Hunted and clean (how hard)

I ran roughly 600 interleavings plus direct probes. None of the following produced a blocker.

- **Takedown racing publish/import** (`$S/td.test.ts`, 3 scenarios, 51/51/74 boundaries, the takedown run as a
  concurrent promise started at each boundary): every outcome is one of (publish committed first, so it
  is covered by the admin's separate hold-copies step by design), (takedown committed first, the copy
  is held: 11/11/34), or (`428`, the takedown's preview token was stale, so the admin retries; 14 each,
  fail closed). Audit-row order confirms every `view=200 holds=0` case had its publish commit before
  the takedown (`order: ["item.publish:T","item.publish:A","dmca.takedown:T"]`). No copy was published
  after the takedown and left visible.
- **Delete / unlist / suspend racing republish** (`$S/del.test.ts`, 56 boundaries each): the item is hidden from a
  third reader and from `GET /items` in every interleaving that ends removed, unlisted or suspended
  (0 violations).
- **Hidden vs absent oracle** (`$S/enum.test.ts`): for unlisted, self-deleted, admin-removed and non-existent ids,
  the response is byte-identical (status, body, headers except date/etag) on GET, HEAD, import, report,
  blob GET, appeal, counter-notice, PATCH and DELETE (1 distinct response per route). The publish
  preview refusal answers the same 400 for all four.
- **Export** (`$S/exp.test.ts`): an importing teacher's export contains none of another author's hidden
  explanation or answer, an unlisted item's text, another reporter's note, an admin note, another
  account's id or name, or a tag. Ann's export contains no other account's id, name or note.
- **Request size, depth, duplicate keys** (`$S/limits.test.ts`): on `PUT /items/prefs`, `POST /items/versions`,
  report, appeal, counter-notice, PATCH, import, undelete: a 20,000-deep array answers 400, a 20,000-deep
  object 413, a 70 KB string 413, a duplicate key 400, a NUL 400 (counter-notice 404 on the item), and a
  2.6 MB chunked body without Content-Length 411. `POST /items` with a 5,000-deep meta answers
  `400 too_deep`; a 3 MB body with no newline answers 400. No 500, none slow.
- **SQL interpolation**: all `'${...}'` sites reviewed. Values are a regex-validated id
  (`ID_RE`, `PUBLISH_ID_RE`), a `crypto.randomUUID()` nonce, a hex-sanitised `source` (blocking.ts:392),
  a constant (`FLAG_SEVERITY_SQL`), or a JWT `sub` (a server-issued UUID, auth.ts:1411). None takes
  request text.
- **Admin surface**: `guard()` requires a live, unsuspended, undeleted admin on a known device plus a
  sudo token for every `write: true` route; the 6 pool admin routes with `write: false` are all `GET`s.
  The pool admin bodies are read after the guard (the pre-read applies the 256 KB bound only to an
  authenticated admin, 8 KB for everyone else).
- **Mail**: pool mail carries ids only; the provider call is a JSON body (no header assembly), so a
  teacher-controlled title or name cannot inject headers or HTML; rejection and suspension reasons are
  admin-written plain text.
- **Normal (non-racing) erasure** (`$S/erasure.test.ts`, a rich account: kept, unlisted, imported, reported,
  uploaded, 45 days of crons): only the documented residue remains (tombstone, `erasure_done`, audit
  rows without notes, anonymised kept item, `retired_publish_ids`, `attestations`). R2 holds only the
  kept item's bodies.

## (c) Non-blockers (terse)

1. A kept (anonymised) item keeps the author-typed title/tags verbatim (e.g. `"Zorgon Quixote's unit"`
   stays after erasure). By design (the author chose to keep it; the title is content) but the README
   should say a name typed into a title is not scrubbed.
2. 34d non-blockers 2 and 3 (snapshot cross-product size; `dmca_upstream_keys` never pruned) are
   unchanged and not re-measured.
3. The auth tombstone batch and the pool `onDelete` batch are separate calls (auth.ts:1393 vs 1415);
   `erasure_finish` covers a failed second batch daily. This is the root of B-1's window; a single
   batch would not remove the in-flight request but would shorten the "no pool cleanup" gap.
4. `pool_prefs` for an erased id (B-1, prefs) is a one-bit row; I count it inside B-1, not separately.

## (d) Not run

- Real workerd, D1 or R2, staging, any deployed cron. The race windows are injected interleavings at
  statement/batch boundaries (D1 batches are atomic, so I fire before a batch, never inside one); their
  real duration is not measured. A first attempt that fired inside a batch was an artefact of the
  harness's nested savepoint and was discarded.
- A real pre-0105 database (the 0105 re-key is covered by the suite's `rekeyStatements()` form only).
- Auth attack simulations beyond the package's 224 tests (lockdown, refresh reuse, enumeration, sudo
  replay): re-run green, not re-derived. No change-password route exists (reset only); reset revokes
  all sessions and bumps the device epoch (read, not run).
- Timing differentials on publish refusals (a statistical oracle for a hidden key): the status/body
  differential above is exact, but I did not measure CPU time on the harness.
- Rate-limit bypass by header: `CF-Connecting-IP` is trusted as Cloudflare sets it; not testable
  in-process.
- Concurrent R2 `onPut` races for import/read against takedown (covered by D1-boundary sweeps only).

BLOCKERS: 1
