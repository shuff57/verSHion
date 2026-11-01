# ASVS critic — verSHion shared pool, ROUND 26 (security)

Read-only. No repo file was edited except this report. Scratch probes are in
`/tmp/critic34d.7HDGtT/` (round-24 repros copied there first; `lib`-free standalone node:test files,
run against the in-process harness `test/pool/harness.ts`, share-core `pkg` from 13:05, not rebuilt).

Every command below is run from `/home/shuff57/Documents/GitHub/verSHion/server/worker`:

```sh
node --experimental-strip-types --no-warnings --test /tmp/critic34d.7HDGtT/<file>.test.ts
```

## Baseline (run alone, this tree)

- `cd server/worker && npm run typecheck` → exit 0.
- `cd server/worker && npm test` → **464/464 pass, 0 fail** (exit 0).
- `cd server/auth && npm run typecheck` → exit 0.
- `cd server/auth && npm test` → **224/224 pass, 18 files** (exit 0).
- `POOL_PROPERTY_SEEDS=40 node --experimental-strip-types --no-warnings --test --test-name-pattern="round 26 differential" test/pool/round26.test.ts` → 40 seeds pass (25.9 s). I widened the builder's 8-seed differential; no failure.

No flakes seen.

## (a) Previous (round-24 security) findings, re-run

### B1 — the author's own original held on its second ordinary republish

Repro: `/tmp/critic34d.7HDGtT/upstream.test.ts` (`critic-asvs-24/upstream.test.ts`, unchanged).

```
# blocked hashes: [{"kind":"match","n":2}]
# after republish 1: []
# after republish 2: []
ok 1 - round 24 upstream exemption lapses after one republish: the author's own original is held
```

**FIXED.** The derivation-based exemption no longer lapses; `upstreamCarried` (served.ts:110-115) is
true for every republish of the upstream item and the take-down's blocked hashes are
`[{"kind":"match","n":2}]`, so the test is not vacuous.

### B2 — upstream-by-import/read forgeable with one GET or import

Repros: `shield.test.ts`, `shield-read.test.ts`, `shield-read2.test.ts`, `shield-read3.test.ts`,
`shield-ctrl0.test.ts` (unchanged). Every variant:

```
# hold-copies: 200 {"ok":true,"held":["<Bob's copy>"],"more":false}
# held after hold-copies: ["<Bob's copy>"]
ok 1 - upstream-by-import lets an accomplice's copy escape hold-copies
# CONTROL held: ["<Bob's copy>"] ...
ok 2 - CONTROL: without the infringer's import, the accomplice's copy IS held
```

**FIXED.** Branch 2 is gone (`upstreamOf`, served.ts:89-101); the accomplice's copy is held after
an import and after a plain GET, identical to the control.

### B3 — migration 0105 loses a window whose content is no longer retained

The round-24 script as committed (`critic-asvs-24/mig.test.ts`) is **not a valid repro**: its
statement filter

```ts
mig.split(/;\s*\n/).map(s => s.trim()).filter(s => s && !s.startsWith("--") && !/^ALTER TABLE/ …)
```

drops any chunk that begins with a `--` comment. On the current 0105 that leaves exactly **two**
executable statements, both DML, in the wrong order:

```
original-filter statements: 2
[ 'DELETE FROM content_licences WHERE rowid IN ( SE', 'INSERT OR IGNORE INTO item_offer (item_id, ckey) S' ]
```

i.e. it runs the `DELETE` of the old exact-keyed rows but **not** the `INSERT` that re-keys them, so
its `422` is produced by the script deleting the history, not by 0105.

Corrected runner (comments stripped, then `INSERT`/`DELETE` statements executed — the same
extraction the round-25 suite uses in `rekeyStatements()`):
`/tmp/critic34d.7HDGtT/mig2.test.ts`.

```
# 0104 (pre-upgrade) grant: [ { licenses_json: '["CC-BY-NC-4.0","CC0-1.0"]' } ]
# statements executed: 4 [ 'INSERT INTO content_licences', 'DELETE FROM content_licences',
#                           'INSERT OR IGNORE INTO item_o', 'DELETE FROM item_offer WHERE' ]
# licences the view grants: [ 'CC-BY-NC-4.0', 'CC0-1.0' ]
# copy status: 201
ok
```

**FIXED.** The re-key maps through the ghost (`0105:41-58`, `:120-137`) and the earlier reader keeps
the CC0 window; the merge is `MIN(from_at)` / `MAX(until_at)` with open-wins, so no window is lost.
(The hourly `licenceRekey` covers rows whose near hashes were NULL at migration time,
publish.ts:308-368; the batch selects mappable rows only, so an unmappable row cannot starve it.)

### Non-blocker 1 — held-publish charge omitted the upstream exemption

Repro: `cap.test.ts` (unchanged), 22 matching republishes of the exempt original:

```
# blocks: [{"kind":"match","n":2}]
# holds after takedown: [{"item_id":"<Bob>", ...}]
# \#1 charge= [] holds= [] … \#22 charge= [] holds= []
# no hold after 22 republishes
```

**FIXED.** The charge now subtracts `upstreamCarried` (publish.ts:2740) and the daily counter stays
empty; `probe_cap` never fires for the exempt original. I also checked the other exemption branch
(`rightAuthor` pre-existing material): the charge query keeps
`rightfulExempts(...)` (publish.ts:2741), and a rightful-author republish is not held or charged
(`/tmp/critic34d.7HDGtT/rightful.test.ts`: `after republish 1: []`, `after republish 2: []`) because
`rightfulExempts` (served.ts:61-73) exempts it outright. The `baseOf.committed_at <= bh.at` clause
at publish.ts:2652-2654 is therefore effectively redundant for the rightful author, not a live bug.

## (b) New-class blockers

### B-1 (ran): an erasure that lands during the stem backfill's R2 read is followed by the backfill's INSERT — the erased account's durable authorship records come back

**Mechanism.** Round 26's B-3 fix is a **check at SELECT time**:
`stemBackfillAuthor` (publish.ts:394-404) excludes `users.deleted_at IS NOT NULL`, then, for a
hidden publish, awaits `c.r2.get(p.full_key)` (:426) and `fieldKeys` (HMACs), and only then runs the
`INSERT INTO author_stems …` (:446-457). That INSERT re-checks nothing. An account deletion that
commits in that window deletes the rows, and the INSERT re-creates them. The daily `erasure_resweep`
(cron.ts:173-187) is the only thing that removes them again; it runs only in the 04:xx daily pass
(index.ts:188-192, cron `17 * * * *`), after `stem_backfill`. Nothing an erased account does can
read these rows (they are keyed sketches/HMACs, and no query reads `author_stems` without
`s.author_id = <the publisher>`), so this is an erasure-fidelity defect, not a cross-user leak — but
it is the exact resurrection class round 25 reported and round 26 claims closed ("none may: each
skips erased ones", cron.ts:174).

**Inputs** (`/tmp/critic34d.7HDGtT/toc.test.ts`; the harness's `MemR2.onGet` is documented as
"simulates a concurrent request landing mid-flight"):

1. Ann (teacher) publishes one **hidden-key** Mc question with an explanation (so the backfill must
   read the hidden full body from R2).
2. `UPDATE author_stems SET bk_expl_full = NULL` — the pre-round-24 state the backfill exists for.
3. `w.r2.onGet` is armed to fire on that publish's `full_key`: inside it, the auth package's exact
   erasure is run (`UPDATE users SET … deleted_at = …` and
   `db.batch(deleteContentStatements(db, Ann, now))`), i.e. the tombstone and the pool cleanup
   commit while the cron is between its SELECT and its INSERT.
4. One `w.pool.hourly()`.

**Outputs.**

```
# erasure landed inside the backfill window; stems right after erasure: 0
# fired: true author_stems after the hourly cron: 8
# author_stems after the NEXT daily cron: 0
ok 1 - erasure that lands during the backfill's R2 read is followed by the INSERT (records resurrected)
```

**Control** (same erasure, committed one moment earlier — before the cron):

```
# control author_stems after the hourly cron: 0
ok 2 - CONTROL: the same erasure one moment earlier (before the cron) leaves no records
```

So the guard works when the deletion precedes the SELECT, and fails when it lands after it. The
window is the R2 GET plus the HMAC work (tens of ms) for a hidden publish; even a shown publish has
the same two-statement gap (the candidate SELECT and the per-publish `item_questions` SELECT are
separate awaits before the INSERT). The rows survive until the next 04:17 UTC run (≤ ~24 h), and on
a deployment whose daily pass does not run they survive indefinitely. The round-26 `backfillItem`
(the other writer of account records) does **not** have this problem, because its deleted-author
guard is inside the `INSERT … SELECT` itself (cron.ts:567-592) and is re-evaluated when the INSERT
runs.

**Why it counts.** "Data resurrection (erasure)" is in the convergence list; the code added in
round 26 to fix the round-25 B-3 resurrection does not hold under a concurrent erasure, and the
README's claim that nothing re-creates an erased account's records is false. Caveat for the loop:
the harm is bounded to <24 h by `erasure_resweep`, so this is Medium, not High; if the convention is
that an incomplete fix of a prior round's blocker is only a "variant", downgrade it — the mechanism
is new (concurrency), the symptom is the old one.

**Fix direction.** Put the guard in the write, not the read: add
`AND EXISTS (SELECT 1 FROM users u WHERE u.id = ?1 AND u.deleted_at IS NULL)` to the `INSERT …
SELECT` at publish.ts:446-457 (the shape `backfillItem` already uses), and re-check after the R2
read before the INSERT. A second, broader instance of the same shape is the publish-time writer
(publish.ts:2544-2563): a publish already authenticated can commit `author_stems` after a concurrent
`DELETE /account`; the same in-statement guard would close it, or the erasure batch can be re-run by
`erasure_finish`/`erasure_resweep` (it is, daily).

## (c) Non-blockers / variants (terse)

1. **`lacksAdded` treats "a different explanation/answer" as "lacks it"** (provenance.ts:919-920).
   A source whose explanation merely differs from the publisher's is judged against her earlier own
   version. Repro `/tmp/critic34d.7HDGtT/lacks.test.ts`: with Bea's *different* explanation (or a
   different Short answer) published between Ann's v1 (no explanation) and v2 (her own), Bea is not
   credited and Zoe's verbatim CC0 copy of Ann's item is accepted `201`; with a source that lacks the
   explanation the same `[]`/`201` is intended. Control `/tmp/critic34d.7HDGtT/lacks_ctrl.test.ts`
   (no earlier own version): the same Bea *is* credited and Zoe is refused `422
   license_incompatible ["CC-BY-NC-4.0","CC-BY-NC-SA-4.0"]`, so the credit machinery is intact and
   the difference is only the loose first. I did **not** count it: "Earlier" is defined as before the
   publisher's own first publish of the question including "own near-rewordings"
   (README round 16 B2), and Ann's v1 is such a near-reword with the same body, so the empty chain is
   defensible. Worth renaming/deciding, not a blocker.
2. **`UPSTREAM_SNAPSHOT_SQL` is a cross product** `json_each(derivation) × json_each(selected keys)`
   (served.ts:122-132). A bank of 1,000 questions each with its own origin and a scope with ~5 keys
   can make millions of candidate pairs in one admin batch. Admin-only, and a failure aborts the
   atomic takedown (fail-closed), so not a blocker; a bound on the pair count would be prudent.
3. **`dmca_upstream_keys` is never pruned** (not on restore/withdraw/expiry). Rows are item ids and
   block-key HMACs only; growth is bounded by admin takedowns. Not a blocker.
4. **`stem_bf_skip:<publish_id>` is permanent.** Three unreadable-body attempts stop that publish's
   stem backfill for good (`bk_expl_full` stays "unknown" = never a false credit). Only the server
   controls the R2 key, so an author cannot trigger it. Not a blocker.
5. **`licenceRekey` can widen a merged window across a gap** between two offers of the same
   canonical content (publish.ts:340-343). Documented (README round 25: the old exact-keyed rows
   could not express the gap; CC grants are irrevocable). Not new.
6. **`export`'s `retained` query has no `purged_at` filter** (export.ts:157-165), unlike `items`.
   A publish row left behind by a partially failed purge would be exported with its own body. It is
   the account's own data and self-export only; I did not reproduce a leftover row.
7. **Round-24 B3's repro was invalid as committed** (see (a)); the suite's own version is the valid
   one. This is why the round-24 report's B3 verdict should not be relied on without the corrected
   runner.

## (d) What I did NOT run

- Real workerd / D1 / R2, staging, or any deployed cron: everything is the in-process harness. In
  particular the B-1 window was measured as an injected interleaving (the harness's documented
  mid-flight hook), not real network latency; the ordering is real, the duration is not measured.
- Auth attack simulations beyond `server/auth`'s 224-test suite (enumeration, login refunds,
  lockdown, refresh reuse, admin sudo guard) — re-run green, not re-derived.
- The 20/day `held_publishes` cap for the `rightAuthor` pre-existing path beyond 2 republishes (the
  hold and charge are both exempt via `rightfulExempts`, so I stopped).
- Performance/limits of the snapshot cross product on a 1,000-question bank (code-read only).
- A real pre-round-24 database; the 0105 run is the documented exact-keyed representation, as before.

BLOCKERS: 1
