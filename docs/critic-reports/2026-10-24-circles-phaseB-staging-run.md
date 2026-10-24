# Circles phase B: staging run (2026-10-24)

Owner approval: phase B only. Deploy the phase B backend to the staging Worker `vershion-pool-staging`, apply
`0110_pool_round38.sql`, turn `workers_dev` on briefly, load-test with two throwaway accounts, turn `workers_dev` off and
redeploy, erase the test accounts and circle data. Production was not touched. Local port 8799 was not used. Nothing was
committed.

## Summary

- **Code deployed:** the server tree of `b0c7184`. HEAD `129a30c` only changes rig scripts under `server/worker/scripts/`.
  The typecheck was clean before the deploy.
- **Test version:** `cef91c30-cd9b-409b-9058-1ed13c9e6df2`, deployed 20:31:17Z with `workers_dev` on.
- **Final version:** `c0cfb93d-b21f-4b3f-99b8-a5867774e575`, deployed 20:35:15Z with `workers_dev = false`.
  `https://vershion-pool-staging.shuff57.workers.dev` answers `error code: 1042` again.
  `git diff` shows no change to `wrangler.staging.toml`.
- **Migration:** `0110_pool_round38.sql` applied (5 commands, 5.4 ms). `d1 migrations list` now says "No migrations to
  apply". The backfill is correct on real rows (see below).
- **Run result:** 46 of 46 checks passed. No 500, no `D1_ERROR`, no 1102 and no 503 busy. All 125 tail events had
  outcome `ok`, with no exceptions.
- **Cleanup:** both accounts erased (204). All 3 test banks were purged through the owner route and the circle was
  closed. 0 circle versions, qkeys, holds, reports or orphans remain. All 152 recorded R2 keys are gone, and the
  bucket is back to 49 objects.
- **Not measured:** D1 insights did not report three of the new statements: the listing refresh, merge-initiate
  `between` and the trim UPDATE. The cause is under Anomalies (item 1). Query plans were checked for these statements
  instead.

## Timeline (UTC)

| Time | Step |
|---|---|
| 20:29:58 | `workers_dev = true` in the toml; `wrangler triggers deploy` turned workers.dev on for the deployed phase A version `33100b12`, without uploading code |
| 20:30:05-20:30:24 | Run script: tail attached, accounts A and B signed up, verified and logged in; approved and given an invite redemption by one scoped D1 write (exactly 2+2 rows); circle created; B joined as editor; seed versions sent **on the phase A code** |
| ~20:30:50 | `wrangler d1 migrations apply --remote`: 0110 applied |
| ~20:31:00 | Backfill checked in D1 (below) |
| 20:31:17 | `wrangler deploy` of phase B (`cef91c30`), still with workers_dev on |
| 20:31:25-20:34:34 | GO signal; tail restarted; phase B load test; cleanup through the API; post-delete D1 check |
| 20:35:15 | `git checkout -- wrangler.staging.toml` (workers_dev false); `wrangler deploy` of `c0cfb93d` |

workers.dev was publicly reachable for about 5.3 minutes.

## Migration 0110 on a populated database

Staging had no circle versions before this run: phase A's data had been purged. To test the backfill on real rows, the
script seeded data first with the phase A code still deployed:

- A1 v1: 990 questions, 4,156,444 bytes stored, `current`
- B1 v1: 5 questions, `previous`
- B1 v2: `current`

After `0110`:

| bank (title) | version | status | listing | withheld_at |
|---|---|---|---|---|
| memtest B1 v2 title | 1 | previous | NULL | NULL |
| memtest B1 v2 title | 2 | current | `{"title":"memtest B1 v2 title","subject":"Science","course":"","grade":""}` | NULL |
| memtest A1 | 1 | current | `{"title":"memtest A1","subject":"Science","course":"","grade":""}` | NULL |

Across the whole database:

- 3 versions, 2 of them current.
- `current_without_listing` 0, `noncurrent_with_listing` 0, withheld 0.
- `current_listing_mismatch` 0: the JSON matches the bank's title, subject, course and grade.
- No live bank was blanked by the third statement. None qualified: no head was withheld.
- `pragma_table_info` shows `withheld_at INTEGER` and `listing TEXT`.

After the deploy, the phase B history route served pre-0110 A1 v1 as `kept: true, withheld: false`.

## What the load test did (all on phase B code unless noted)

Bank A1 is ~3.7 MiB: each request is 3.94-4.07 MB, and each head read returns 4.06 MB. The script tracked qids through
`published.qid`.

| Step | Result |
|---|---|
| Seed (phase A code): A POST A1 v1 3.7 MiB; B POST B1 v1, PUT B1 v2 (small) | 201, 201, 200 |
| 4 concurrent bursts: A and B PUT from the same head at the same instant | every burst exactly one 200 and one `409 conflict` with the true `head_version` (A won all four) |
| Loser (B) per burst: `POST .../merge {base_version}`; GET head (4.06 MB); PUT with `merge_token` + `merged_from` | merge 200 (`base_kept: true`, 1 line between, token present); head read showed the new head; merged send 200 with `merged_from` = the base (v3, v5, v7, v9) |
| 5 sequential alternating sends A/B/A/B/A | 5 x 200 (v10-v14) |
| Large change: B sends 400 of 990 questions, new title | `428 confirm_large_required {removed: 590, total: 990}`; resent with a new commit id and `confirm_large_change` (as the app does): 200, v15 `-590`; listing title became "memtest A1 short" |
| Rollback by the owner to v14 | 200, v16 `rollback_of: 14`, `+590`, 990 qids back; listing title back to "memtest A1" (target's listing) |
| 2 more sequential sends | 200, 200 (v17, v18) |
| Stale send (base = head - 2) | `409 conflict` |
| Trim `keep: 5` by the owner | 200 `{pruned: 13, oldest_kept: 14, pinned_versions: []}`; history: 18 lines, kept 14-18, 4 merged lines, 1 rollback line |
| Kept version v17 read / pruned v1 read | 200 (4.06 MB) / `404 version_not_retained {oldest_kept: 14}` |
| Merge-initiate from pruned v1 | 200, `base_kept: false`, 17 between, total 17 |
| B (author, editor) rolls B1 back to pre-0110 v1, which has no recorded listing | 200, v3 `rollback_of: 1`; bank title = v1's body name "memtest B1 body v1" (47a V3 behaviour) |
| Small bank S: 55 versions, A and B alternately, sequential | 1 x 201 + 54 x 200, no failure; D16 pruned beyond 20 |
| S history paging | page 1: 50 lines, `next_before: 6`, 20 kept, `oldest_kept: 36`; page 2 (`?before=6`): 5 lines (v5..v1), `next_before: null` |
| S merge-initiate from v1 | 200: 50 between (v2..v51, oldest first), `between_total: 54`, `base_kept: false` |
| Reads: circle detail, `POST /circles/heads`, activity, list | 4 x 200 |

Counts for the 3.7 MiB bank on phase B:

- 20 requests over 3 MB: 15 accepted (11 plain, 4 merged), 4 burst losers, and the stale send.
- 2 large-change requests of 1.6 MB.
- 1 rollback.

Across all banks, A1 reached 18 versions, B1 3 and S 55. The optional byte-cap check was skipped: the cap is 200 MB, so
reaching it would take about 54 more 3.7 MiB versions.

## HTTP status counts

All 138 requests:

| Status | Count |
|---|---|
| 200 | 118 |
| 201 | 5 |
| 202 | 2 (sign-ups) |
| 204 | 2 (account erasures) |
| 404 not_found | 4 (all `/memtest-probe`, the tail-ready probes) |
| 409 conflict | 5 |
| 428 confirm_large_required | 1 |
| 404 version_not_retained | 1 |

The 121 requests on phase B code: 200 x109, 201 x1, 204 x2, 404 probe x2, 409 x5, 428 x1, 404 version_not_retained x1.

None of the following occurred: 500, 503 (`busy` or other), 429, `cf_1102`, `D1_ERROR`, `exceededCpu` or
`exceededMemory`, or a tail exception.

## Worker CPU and wall time (wrangler tail, per request)

| Route (phase B code unless noted) | n | CPU ms (min / p50 / max) | Worker wall ms (p50 / max) | client ms (p50 / max) |
|---|---|---|---|---|
| PUT 3.7 MiB, accepted (7 sequential, 4 burst winners) | 11 | 1,261 / 1,388 / 1,687 | 3,190 / 3,689 | 4,367 / 5,140 |
| PUT 3.7 MiB merged send (with `merge_token`) | 4 | 1,211 / 1,328 / 1,437 | 3,130 / 3,170 | 4,125 / 4,138 |
| PUT 400-question large change (confirmed) | 1 | 517 | 2,010 | 2,524 |
| PUT refused 428 (large change) | 1 | 68 | 799 | 1,251 |
| PUT refused 409 (burst losers) | 4 | 115 / 126 / 149 | ~2,900-3,500 | ~4,700-5,300 |
| PUT refused 409 (stale, no contention) | 1 | 61 | 673 | 1,563 |
| POST rollback A1 (990 questions) | 1 | 489 | 2,316 | 2,360 |
| POST rollback B1 (5 questions) | 1 | 20 | 1,709 | 1,749 |
| GET head / kept version (4.06 MB body) | 5 | 77 / 89 / 92 | 696 / 726 | 753 / 768 |
| GET pruned version (404) | 1 | 6 | 351 | 386 |
| POST merge (merge-initiate) | 6 | 6 / 7 / 16 | 382 / 470 | 421 / 507 |
| GET versions (history) | 7 | 5 / 6 / 8 | ~285-290 | ~325-345 |
| POST trim | 1 | 7 | 338 | 452 |
| PUT small bank S | 54 | 15 / 17 / 41 | 1,378 / 1,693 | 1,425 / 1,729 |
| POST /circles/heads | 1 | 16 | 313 | 476 |
| Phase A code: POST 3.7 MiB new bank (seed A1 v1) | 1 | 1,040 | 2,591 | 3,885 |

The burst losers took about 3 s of wall time but only 0.12-0.15 s of CPU. They waited for the heavy slot, read the head
again, and answered 409 without running share-core (46b V6 as designed). A send's daily count is given back on a 409
or 428.

## Memory budget

- No `503 busy` and no 1102 or `exceededMemory`. All outcomes were `ok`.
- The two simultaneous 3.7 MiB sends never shed. Each burst was serialised: one ran, the other waited and was refused
  cheaply.
- Five head and kept-version reads of 4.06 MB streamed at under 0.1 s CPU each.
- Busy shedding with more than two concurrent senders is still untested on staging. The test had only two accounts.

## D1: rows read per statement

Source: `wrangler d1 insights --time-period 2h`, sorted by avg reads, sum reads, writes and count. Raw dumps are in the
scratchpad (paths at the end).

| Statement | avg rows read | avg ms | Note |
|---|---|---|---|
| Commit CAS `UPDATE circle_banks SET head_version = ...`, which contains `BYTES_SQL` and **`PRUNABLE_SQL`** | 49 | 0.56 | the new net-of-prune cap check is inside it |
| Commit prune `UPDATE circle_versions SET status = 'pruned' ...`, S past 20 / A1 | 35 / 13 (others 2) | 0.14-0.54 | |
| History page `... v.version < ?2 ORDER BY v.version DESC LIMIT ?3` | 3 (6 runs captured) | 1.48 | the S pages (about 51 rows) were not among the captured runs |
| `keptInfo` (`INDEXED BY circle_versions_bank`) | 9 | 0.22 | |
| Version INSERT | 2 | 0.32 | |
| `circle_holds` student-data INSERTs in the commit (privacy re-check) | 23 | 0.11-0.26 | |
| Records check `COUNT(*) FROM json_each(?1) ... circle_qkeys` | 1,146 | 0.66 | |
| Student-data keys `SELECT hash FROM privacy_blocks WHERE kind='match' AND hash IN (json_each)` | 2,084 | 2.16 | the largest per-call cost in a send |
| Near-rewording bands `SELECT DISTINCT band, qhash, tokens FROM privacy_bands ...` | 4,926 | 1.65 | the highest avg of any statement; shared with the public publish path |
| Listing refresh (`LISTING_REFRESH_SQL`), merge-initiate `between`, trim UPDATE, `circle_qkeys` INSERT | **not reported by insights** | - | see Anomalies, item 1 |

No statement averaged more than 4,926 rows read. Every circle statement was at most 1,146 rows and under 2.2 ms. The
total for circle statements captured in the window was 104,964 rows read, 2,925 written and 470 ms of D1 time.

For the statements insights did not report, `EXPLAIN QUERY PLAN` on staging gave these plans:

- **Listing refresh subquery:** `SEARCH lv USING INDEX sqlite_autoindex_circle_versions_3 (bank_id=? AND version>?)`,
  ordered by version DESC with LIMIT 1. It stops at the newest non-withheld version that has a listing, which is
  normally the head.
- **Merge-initiate between:** `SEARCH v USING INDEX sqlite_autoindex_circle_versions_3 (bank_id=? AND version>? AND
  version<?)`, plus a users primary-key lookup, with LIMIT 50.
- **History page:** `SEARCH v USING INDEX ... (bank_id=? AND version<?)`, plus a users primary-key lookup, with LIMIT 51.
- **`PRUNABLE_SQL`:** `SEARCH pv USING INDEX ... (bank_id=? AND version<?)`, with the holds primary key and
  `circle_reports_open` for the subqueries.
- **`BYTES_SQL`:** `SEARCH sv USING INDEX circle_versions_circle (circle_id=? AND status=?)`.
- **Trim UPDATE:** `SEARCH circle_versions USING INDEX ... (bank_id=? AND version<?)`.

Every one is an index range bounded by one bank, or one circle for the byte sum. None scans a table.

## Anomalies and observations

1. **D1 insights is incomplete for this run.** Several statements that run on every commit are missing from the
   output: the listing refresh, the `circle_qkeys` INSERT and the `status = 'previous'` UPDATE. The merge-initiate
   `between` SELECT (6 calls) and the trim UPDATE (1 call) are missing too. The run counts also look sampled or
   inflated: the CAS UPDATE shows 176 runs for about 80 commits, and the version INSERT moved from 67 to 131 between two
   queries a few minutes apart. Per-statement rows_read for those statements therefore comes from the query plans
   above, not from a measurement. The README's workerd numbers (round 37) remain the measured reference: history page
   131 rows, merge-initiate 37-130 rows.
2. **The run script's own pre-cleanup D1 snapshot failed** (exit 1, a script problem, not the server). There is no
   per-version D1 dump of `listing` and `withheld_at` after the test. The API checks covered the listing behaviour
   (large change, rollback, pre-0110 rollback), and no version answered `withheld`.
3. **Tail coverage:** 13 of 138 requests have no tail event. These include the first probe, join and preview, seed B1
   v1, S v3, the final reads, two purges, the circle close and B's erasure. Their CPU is unknown. Their HTTP statuses
   were all as expected.
4. **CPU of a phase B send.** A 3.7 MiB next version costs 1.2-1.7 s CPU. The phase A run reported 0.6-0.9 s for a
   circle send, and the phase A seed here (a new bank) took 1.04 s. The two are not directly comparable: a next version
   with qids adds the head index diff, origins, and the student-data and near checks. The cost is well within limits.
   It is roughly a public publish of the same size, and it occupies the isolate's one heavy slot for about 3 s of wall
   time.
5. **Residue after the purge and erasure, by design:**
   - 74 `attestations` rows. Circle sends record one per version, with ids, time and terms version, and no text. They
     reference the purged versions and the erased accounts. The cron calls attestations "the documented residue" of an
     erasure, but no circle purge path deletes them, so they stay indefinitely.
   - 91 append-only `pool_audit` rows.
   - The user tombstones and `erasure_done`.

   Flagged only for the owner's data-minimisation review. Nothing here is text from the banks.
6. **Circle names remain until the day-14 purge.** Both test circles keep their row and name: phase A's "Memtest
   circle" `b7215ba9…` and this run's "Memtest circle B" `f9fc87c0…`. Each is closed `owner_closed` with
   `purge_in_days` 13. The hourly cron purges them.

   All 7 circle-bank tombstones in staging have blank title, subject, course and grade, phase A's 4 included
   (`banks_with_text` 0). So the tombstone-title concern in the phase A notes does not show on staging.

No 500s, D1 errors, 1102s or unexpected 503s occurred, and no step needed a stop.

## Cleanup verification (D1 after the erasures)

**Through the API:**

- A removed and purged A1, B1 and S: 3 x (200, 200).
- A closed the circle: 200.
- B was erased (204), then A was erased (204).

**Test accounts** `ae7b839c-…` and `ce202926-…`:

- `deleted` 1, `email_gone` 1, `pw_gone` 1.
- Sessions 0, invite redemptions 0, pool locks 0, `usage_daily` 0, `pool_limits` 0.
- `erasure_done` 2, items 0.

**Circle `f9fc87c0d90e8c9ecf33293c7fd061e4`:**

- Closed (`owner_closed`, purge set, 13 days), members 0.
- 1 invite (revoked, no note), transfers 0.
- Banks: 3, all purged, 0 with text.
- Versions 0, qkeys 0, holds 0, reports 0.
- `r2_orphans` under `circles/<cid>/` 0, admin flags 0.

**Whole database:**

- users 13 (live 1, the same as before; +2 tombstones), `vershion-memtest-%` emails 0.
- circles 2 (both closed, awaiting purge), open circles 0.
- circle_banks 7 (live 0), circle_versions 0, members 0, qkeys 0, holds 0, reports 0, version_blobs 0.
- items 19 (unchanged), r2_orphans 0, pool_locks 0.

**R2:**

- Every one of the 152 keys recorded before the cleanup (76 versions x body and index, pruned ones included) returns
  "The specified key does not exist".
- `r2 bucket info`: 49 objects and 196 MB, the same as before the run.

## State left on staging

- `vershion-pool-staging` runs `c0cfb93d-b21f-4b3f-99b8-a5867774e575`, phase B code with `workers_dev = false`.
  workers.dev answers 1042. The cron (`17 * * * *`) is unchanged.
- D1 `vershion-pool-staging` is at migration 0110.
- Both throwaway accounts are erased. Their passwords, verification tokens, invite code, access tokens and merge tokens
  existed only in the run process and were never printed, saved, or passed in argv or env.
- Two closed test circles are waiting for the cron purge (day 14), plus the residue rows listed above.
- `server/worker/wrangler.staging.toml` is unchanged (`git diff` empty). No repo file changed except this report.

## Files (scratchpad, not in the repo)

All under `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/phaseB/`:

- `run-b.mjs`: the run (seed, wait for GO, load test, cleanup).
- `gen.mjs`: bank payloads.
- `r2check.mjs`: the R2 key check.
- `insights.mjs`: the insights summary.
- `run.log`: the run's output.
- `results-b.json`: every request with tag, status, CPU and wall.
- `insights-*.json`: the D1 insights dumps.
