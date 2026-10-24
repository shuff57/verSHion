# Critic 47b: circles phase B (rounds 37-39), security / privacy / abuse lens

Fresh reviewer, no earlier round. Scope: uncommitted phase B on `shared-pool-phase1`. Ran `cd server/worker && npm test`:
592 pass, 0 fail. `npm run typecheck`: clean. Scripts (all run, `node --experimental-strip-types --no-warnings --test <file>`):
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c47/{mig,plan,plans,fuzz,list,rec}.test.ts`
and the c45/c46 scripts. No repo file was changed except this report.

## Verdict

**Blockers (new class): none.** I attacked the migration, the listing logic, the merge token, authz, replay, erasure, DoS
and the public path and found no new mechanism that exposes student data, crosses a circle boundary, or breaks integrity.
Two limits are worth a line in the README (V1 is new in phase B; V2 is a measured cost).

## Step 1: earlier blockers, re-run (outputs read, not only "pass")

| Earlier finding | Result now |
|---|---|
| 45b B1 / 45a (dirty v2, clean v3, member reads `versions/2`) | a1: `E reads v2 after hold lifted: 404 version_withheld`; history `[3 kept, 2 kept:false, 1 kept]`; flag still open. Fixed. |
| 46b N1 (title/subject/course/grade of a withheld version) | w2 (rollback to v1): Vic's circle page `["Unit 2","Math"]`, head read `Unit 2`, activity `Unit 2` on every line. w1 (bank-wide release, v2 stays withheld): list shows v1's `Unit 2 clean`, head read 404. Fixed. My own list.test: first version withheld gives a blank listing, a clean v2 sets its own, dirty v3 keeps v2's, rollback restores the target's, erasing the committer and a trim change nothing. |
| 46b N2 (lost commit + failed R2 delete) | r1: both keys now present in `r2_orphans`. Fixed (lost CAS path `circlesend.ts:873-877`, abandoned retry `:304-309`). |
| 46a M1 / 45 s5 (held bank at the byte cap) | m: dirty send, then at the cap the author's clean fix is a plain 413, rollback 200, trim 200, rollback to the oldest kept 200. Fixed. |
| trim vs evidence/pins/flags | w3/n: report pins its version through `keep=1` (`pinned_versions:[9]`), idempotent twice, race 200/200/409, held trim 409 (author) / 404 (owner), bad `keep` 400. A withheld version with an open flag survives a trim (statement at `circleversions.ts:~140`). Fixed. |
| origins (45b v1, 46b V1) | o1: editor's graft of a duplicate qid with other content gets no credit (`origins [[1,"Dee"]]` is the exact copy only); editor's false origin on a question the head holds is ignored (`license_author_only` 422 on the licence mismatch, author relicense 200). a3/s3: omitted-qid and exact duplicate keep the origin, relicense `422 license_incompatible`. Fixed. |
| licence PATCH compare-and-set (46b V7) | `circles.ts:1308` now `AND head_version_id IS ?6`; authz demoted author 403. Fixed. |
| merge token (45b a2) | a2 plus my fuzz: nine tampered tokens (truncated, extra char, doubled dot, case change, 401-char, last-char flip) all `422 merge_token_invalid` at the right head; MAC compared as the encoded string in constant time before JSON parse. Clean. |

## Step 2: hunt

### Blockers (new class)

None.

### Variants and limits

**V1. An editor can burn the circle's record budget on the AUTHOR's bank, and neither rollback nor trim gives it back (rec.test.ts, ran).**
`circle_qkeys` / `circles.qkeys` (cap `circle_records`, default 50,000) counts every distinct key-free question ever sent to the
circle and is only released when the bank is purged (`circles.ts purgeBank`). Phase A let only a bank's author add records; phase
B lets any editor send to any readable bank (`circlesend.ts:212`, `:786`). Repro with `circle_records=40`: Ann sends 3 questions;
Eli (editor) sends four versions of 8 distinct junk questions to Ann's bank (`qkeys` 3 -> 35); Ann rolls back and trims
(`qkeys` still 35); Ann's new bank of 10 new questions is `413 circle_records_full {used:35, needed:10, limit:40}` and every
edit of hers that adds a new question fails the same way. At the real cap it is 51 sends of ~990 questions (inside 100 sends a day).
Recovery needs the bank purged (the author loses the bank and its history) or the editor's junk to sit in the editor's own bank
(owner purges that one). Availability only, by a trusted role, recoverable; I do not call it a new class (phase A already had
it through an editor's own banks). Suggest: count a rollback/trim's released versions, or release `qkeys` for hashes no kept
version holds, or at least say in the README that the owner's only remedy is to purge the bank.

**V2. `LISTING_REFRESH_SQL` walks the withheld streak four times per commit.** Each of the four column subselects is
`ORDER BY lv.version DESC` over the bank's versions by the `(bank_id, version)` unique index (EXPLAIN: `SEARCH lv USING INDEX
sqlite_autoindex_circle_versions_3`), stopping at the first readable version with a listing. With a clean head that is one row.
While the head and its recent predecessors are withheld it reads `4 x streak` rows, and history rows are kept forever (V4 of
critic 46b). An author who can produce student text sends dirty versions every send (100 a day, each flags and mails an
admin within the 5/hour budget): after 30 days about 12,000 reads per further send. Cost amplification only, small in D1 terms.
Cheap fix: one subselect returning the json, then `json_extract` four times.

**V3. Rollback reads two index objects from R2 before the heavy slot and reservation** (`circlesend.ts:1013-1015`). Each index is
about 170 bytes a question, 168 KB at 990 questions, plus its parse; with many accounts rolling back max-size banks at once in one
isolate that memory is unreserved (account lock is per account, 20 rollbacks a day). Low; reading them after `acquireHeavy` and
`reserveBytes` as `sendOnce` does would remove it.

**V4. A stale or replayed send still costs a body read, a SHA-256 and three lookups before the cheap 409** (`circlesend.ts:218-225`,
`REFUNDED` includes 409). 46b V6 is much reduced (no heavy slot, no reservation) but the day's counter is given back, so the
only bound is the general 300 requests / 5 minutes. Acceptable; say so.

**V5. The commit's own prune drops a withheld version past the 20 even while its flag is open; a trim keeps it** (`circlesend.ts:806-812`
vs `circleversions.ts` trim). The README documents it. It means 20 clean sends after a dirty one remove the admin's evidence body
(the flag, positions and counts remain). Privacy-favourable; the inconsistency is the only reason I list it.

**V6. A rollback to a version sent before migration 0110 (NULL `listing`) records the bank's current listing, so the title does not revert**
(`circlesend.ts:1092`). Only banks that already exist on staging; no leak (the bank's listing never comes from a withheld
version). Cosmetic.

**V7. Unverifiable here: D1 bound-parameter size for the commit's `json_each(?)` of block keys.** Miniflare accepts 6.7 MB
(d1big.mjs). A Multipart-heavy bank (the share-core limits I found do not cap parts) yields about 3 keys a part at 66 bytes; roughly
10,000 parts would exceed a 2 MB string. The public publish uses the same pattern (`blockKeys`), so this is parity, not a phase B
regression. If production D1 rejects it, a circle send would 500 after its R2 puts (cron collects the staging row). Worth one real-D1 probe.

**V8. Known, unchanged:** the send answer's `held`/`withheld` is a membership oracle for the confirmed-block list for any editor
(46a v6, documented); a confirmed block is not retroactive (46b V3); a member's student-data report hold freezes pruning and trim
(`BLOCKING_HOLD_SQL`) and blocks the cap repair until an admin acts (phase A report design).

### Checked and clean

- **Migration 0110 on a populated database** (mig.test.ts, ran on a real 0001-0109 schema, 20,000 banks x 6 versions = 120,000 rows):
  66 ms; only `status='current'` rows get a listing (20,000 of 20,000), previous/pruned/staging stay NULL; quotes, backslashes,
  accents, emoji and an empty title round-trip through `json_object`; removed banks fill too; `withheld_at` all NULL. Plan:
  `SCAN circle_versions` + a PK probe per current row, so cost is one pass over the table (120,000 row reads here), once.
  Re-running the file fails at `ALTER TABLE` (not idempotent, as any migration; D1 tracks the name). **Operational note:** the file
  was edited in place, so any environment that already applied an earlier 0110 (only round 38's `withheld_at`) will skip it and
  every send then 500s on "no such column: listing". The task says none did; run `wrangler d1 migrations list` on staging before the
  first apply. **No repo test exercises the backfill** (round16/20_migrate only apply the file); mig.test.ts is a ready template.
- **Circular import circleversions <-> circlesend.** esbuild bundles `src/index.ts` (the bundler wrangler uses) with no error or
  warning (854 KB, externals for wasm only). In the bundle `circleversions` is emitted before `circlesend` (lines 12169 / 12297) and
  the only cross uses (`verifyMergeToken`, `BLOCKING_HOLD_SQL`, `PRUNED_NOW_SQL`) are inside function bodies; neither module touches
  the other's binding at top level, so there is no TDZ at load, in the bundle or in Node's ESM order. I did not run the real
  `wrangler deploy --dry-run` (its `[build]` step compiles the share-core wasm and writes into the repo). Nitpick: move the two SQL
  constants to a leaf module and the cycle is gone.
- **Query plans.** I recorded every statement a send, history, merge, version read, rollback and trim executed (440) and ran
  `EXPLAIN QUERY PLAN` on each: no table SCAN on any `circle_*`, `admin_flags` or `privacy_*` table (plans.test.ts).
- **Input fuzz.** 19 hostile values (null, floats, 2^53, 1e99, 600-char, NUL, emoji, arrays, objects) through merge `base_version`,
  trim `keep`, rollback fields, history `before`, version `:n`, and a send's `merge_token` / `merged_from` / `confirm_large_change` /
  `message`, as owner, editor and viewer: no 5xx (fuzz.test.ts).
- **Authz / IDOR.** Non-member, foreign circle id, foreign bank id, viewer, non-author editor, demoted, removed, held-for-non-author:
  uniform 404 / 403 as in 46b (a2, w3). Rollback target and version reads are keyed `bank_id = ? AND version = ?`; R2 keys are built from
  route-validated hex ids and DB values; the commit statements interpolate only `newId()` / regex-checked ids.
- **Withheld on every read path I could find** (grep of every `full_key` / `circle_versions` use): head and `versions/:n` (`circles.ts:1136`),
  blob route (`:1257`), history line text (`circleversions.ts lineOf`), merge `between` / `base_kept`, rollback (`:1012`), `keptInfo`,
  activity titles, `heads`, export (own message only). The admin route reads any kept version only on an audited basis.
- **Commit batch.** `changes()` adjacency for `circle_qkeys` -> `circles.qkeys` still holds after the added listing statement;
  `withheld_at` is computed from the privacy holds that exist after this commit's own hold statements and nothing later clears it;
  a lost CAS leaves every guarded statement inert.
- **Public publish path.** `privacynear.ts` is equivalent to the removed block (same band query and limit, same `cut` fail-closed, exact
  set fetched only when a band row matched, same keyed bands and token sets); `acquireHeavy`'s default slot object is the same module
  state, publish passes `c.deps.heavySlots` (undefined) so behaviour is unchanged; the unused `keyedOne` import was removed and
  typecheck is clean; rounds 12-22 pass.
- **Mail.** Circle student-data notices use the `circle_privacy_match` channel (5/hour), body is ids and fixed text only; the
  public channel cannot be starved from circles.
- **Replay / erasure.** Replay runs after membership, role and readability; cross-user commit_id 409 without detail; abandoned staging
  rows are recorded as orphans in the same batch; staging insert, CAS, trim, PATCH and pending-blob upserts carry `ALIVE_SQL`.

## Nitpicks

- README "Round 39" says a version release refreshes the listing and a bank-wide release does not; both are right, but
  "the update check never shows a withheld version's title" should name `heads` (it never returns a title at all).
- `circle-release {version}` still writes its audit row for a version that is not withheld (409 follows). Harmless.
- `activity()` shows `bank_id`, `version` and now `rollback_of` for a held bank to every member (title is null). Existing
  behaviour; the ids are not secret.
- No test for the one-time backfill (see Migration above).
