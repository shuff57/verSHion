# Circles phase C: staging run (2026-10-25)

Owner approval: a staging-only deploy and a short load test of circles phase C, by the procedure of the phase B run
(`2026-10-24-circles-phaseB-staging-run.md`). Deploy the current server tree to the staging Worker
`vershion-pool-staging`, apply `0111`-`0115` to the staging D1, turn `workers_dev` on for the test window, test with two
throwaway accounts, turn `workers_dev` off and redeploy, and erase both accounts and all circle data. Production was
not touched. Nothing was pushed. Local port 8799 was not used. Nothing was committed.

## Summary

- **Code deployed:** the server tree of HEAD `b228485` (round 52). `git diff` showed no change under `server/`, and the
  typecheck was clean before the deploy. A dry-run build showed the bindings first: `MAIL_MODE "log"` and the
  localhost `APP_URL`.
- **Test version:** `a97d0066-cceb-460a-872d-ed50341703e3`, deployed 16:10:17Z with `workers_dev` on.
- **Final version:** `ed43235c-4a7a-4e1a-8087-b6d244f219d6`, deployed 16:13:25Z with `workers_dev = false`.
  `https://vershion-pool-staging.shuff57.workers.dev` answers `error code: 1042` again, and `git diff` shows no change to
  `wrangler.staging.toml`.
- **Migrations:** `0111` to `0115` were applied in 24 ms in total (6, 3, 6, 2 and 3 commands). `d1 migrations list`
  had shown exactly these five as pending, and none came after `0115`. The checks on real rows are below.
- **Run result:** 53 of 55 checks passed. The two failures were wrong expectations in the test, not server faults (see
  Anomalies, items 1 and 2). There was no 500, no 503, no `D1_ERROR`, no 1102 and no `exceededCpu`/`exceededMemory`.
  All 118 tail events had outcome `ok` and none had an exception.
- **Mail:** no real mail could go out. Staging runs `MAIL_MODE = "log"`, which prints a mail to the console and
  returns before any network call. Its `APP_URL` is localhost, and staging has no `RESEND_API_KEY` secret (only
  `AUTH_SECRET` and `POOL_HASH_KEY`). So the email-invite path ran end to end to a throwaway `@example.com` address: the
  "mail" was a line in `wrangler tail`, and B joined with the code read from that line.
- **Cleanup:** both accounts were erased (204, 204). All 5 test banks were purged and the circle was closed. 0 circle
  versions, qkeys, holds, version blobs or flags remain, and the test block is gone. All 78 recorded R2 keys (76 circle
  objects and 2 images) return "The specified key does not exist". The bucket is back to 49 objects and 196 MB, the
  same as before the run.
- **Not measured: the CPU of the one 3.7 MiB send.** `wrangler tail` missed that request's event (31 of 149 requests
  had none). See "Worker CPU".

## Timeline (UTC)

| Time | Step |
|---|---|
| 15:58:03 | `workers_dev = true` in the toml; `wrangler triggers deploy` turned workers.dev on for the deployed phase B version `c0cfb93d`, without uploading code |
| 16:08:55-16:09:18 | Run script on the **phase B code**: tail attached; A and B signed up, verified and logged in; approved and redeemed by one scoped D1 write (exactly 2+2 rows); circle "Memtest circle C" created; one plain code made and left open; A1 v1 (3.7 MiB) and S v1 seeded |
| 16:09:23-16:09:26 | `wrangler d1 migrations apply --remote`: `0111`-`0115` applied; checked in D1 (below) |
| 16:09:41-16:10:17 | `wrangler deploy` of phase C (`a97d0066`), workers_dev still on |
| 16:10:20-16:12:48 | GO; tail restarted; phase C load test (16:10:36-16:12:22); cleanup through the API and the post-delete D1 check |
| 16:12:53-16:13:25 | `git checkout -- wrangler.staging.toml` (workers_dev false); `wrangler deploy` of `ed43235c`; 1042 confirmed |
| 16:17 | Hourly cron (phase C code): blob GC removed the two test images; R2 back to 49 objects |

workers.dev was publicly reachable for 15 min 22 s. About 11 minutes of that, from 15:58 to 16:09, came before the run
script made its first request. The script started late on my side, not because of the server. The test itself took
under 4 minutes.

## Migrations 0111-0115 on a populated database

At apply time the database held one open plain code made on phase B code, the 2 older (revoked) invites, 9 circle
banks (7 old purged tombstones plus A1 and S) and 2 circle versions. After the apply:

- `circle_invites`: all 3 rows are `kind = 'code'` with `email_label` and `email_key` NULL. The phase C list served the
  pre-migration code as `{kind: "code", address_label: ""}`.
- `circle_banks.hide_key`: `INTEGER DEFAULT 0`, 0 on all 9 banks. `circle_versions.strip_opts`: TEXT, NULL on both
  versions.
- `circle_email_sends` and its `_day` index, `circle_invites_label`, `circle_invites_email_key` and
  `circle_versions_withheld` all exist. The withheld index is `(circle_id, status) WHERE withheld_at IS NOT NULL AND
  full_key <> ''`, the round 48 form.
- `circle_holds` was rebuilt with the CHECK list `('reports', 'student_data', 'admin', 'copyright')`, its
  `circle_holds_at` index and no `circle_holds_r44` left behind. Staging had 0 holds, so the row copy in `0113` ran on
  an empty table. Two accounts on phase B code cannot create a hold.

## What the load test did (phase C code unless noted)

Banks:

- **A1** (A): 990 Matching questions, 3.7 MiB.
- **S** (A): 5 questions, small.
- **H** (A, the answer-key bank): a labelled MC question ("A. Mitochondrion" ...), an MC question with picture 1, and a
  short-answer question with picture 2.
- **U** (A): H's question 0 without its labels.
- **HB** (B): a small labelled bank.

| Step | Result |
|---|---|
| Invites list after 0111 | the phase B code is `kind: "code"`, no label |
| Email invite to `...@example.c0m` | `422 invite_not_sent {reason: "not_deliverable", emailed: false}` |
| Email invite with a `note` field | `400 invalid_request {field: "note"}` |
| Email invite to B's address (editor) | `202 {emailed: true, address_label: "v***@example.com"}`; the answer holds neither the code nor the address. The log-mode mail was seen in the tail with a code and a `#code=` link and no circle name |
| B: preview and join with the mailed code | 200, 200, role editor |
| Second mail, to B's `+again` address (first code used) | `202`, a new invite, label `v***@example.com #2` |
| Third mail, to B's address | `202 {resent: true}`: the open invite was rotated (same id, role editor) |
| Fourth mail, upper case | `422 invite_not_sent {reason: "address_limit"}`. One mailbox, so `+tag` and case were folded |
| Owner's list | 2 email invites (`used_up`, `active`) and the plain code |
| Two images uploaded; H v1, U v1, HB v1 (B) | 201 x 5 |
| H: hide on without a label decision | `422 labels_need_decision` with `label_findings` |
| H: hide on with `base_version: 2` (head 1) | `409 conflict {head_version: 1}`, before any work |
| H: hide on, labels strip q0, `image_ok: [1]`, base 1 | 200, `images_kept: [1]`, `images_removed: [2]`, `labels_removed: [{question: 0}]` |
| D1 (D13) | H's viewers' question 0 hash = U's hash (1 shared `circle_qkeys` row; H has 4 rows); decisions recorded on v1 |
| B demoted to viewer; B reads H | 200, `keys_hidden: true`. Of 8 MC answers none was `correct: true`; the sample was gone; no "A. " labels; picture 1 kept and picture 2 dropped; every question `keyless: true` |
| B, viewer image route | picture 1 200, picture 2 404 |
| `POST /circles/heads` (B) | `hide_key` per bank: H true, HB false (round 52) |
| Change picture choices (`image_ok: [1,2]`) | 200, both kept; the same decisions again: `200 {unchanged: true}`; the viewer then gets picture 2 (200) and both in the read |
| B promoted to editor; sends back the viewer's copy | `422 keyless_question {positions: [0,1,2], kinds: ["Mc","Mc","Short"]}` |
| B sends H v2 with decisions | 200 `keys_hidden`; then a send with no label decision: `422 labels_need_decision`, nothing stored |
| **Burst x4** hide-on PATCH on HB (A, A, B, B) | 4 x 200 (wall 0.8-1.4 s) |
| **Burst x4** hide-off PATCH on HB | 4 x 200: exactly one changed it, three `unchanged` |
| **Burst x4 mixed** on HB from v1: A PATCH on, B PATCH on, A PUT, B PUT | both PATCHes 200; A's PUT 200 (`keys_hidden`); B's PUT `409 hide_key_changed {hide_key: true, head_version: 1}` |
| **Burst x4** PUT on S (A, B, A, B), twice | each burst: exactly one 200; the others `409 publish_in_progress` (each account's own second send; the account lock) or `409 conflict` |
| 28 sequential small sends to S (A and B alternately) | 28 x 200, head v31 |
| Scoped D1: a copyright re-upload block on U's hash | 1 row (see "Scoped D1 writes") |
| H: hide off (base 2), then hide on with the same decisions | off 200 (nothing restored); on `200 {withheld: true, held: true}` |
| Owner reads H and `versions/2` | `404 version_withheld {hide_off_releases: true}` x 2 |
| B (editor, not the author) reads H | `404 not_found`, no `hide_off_releases` field |
| **4 parallel owner reads** of the withheld head | 2 x `404 version_withheld` (`hide_off_releases` true, false), 2 x `429 too_many_open_reads` (Anomalies, item 2) |
| **The 3.7 MiB PUT** (A1 v2, A) with 4 owner reads of H started 1.5-3.6 s into it | PUT 200 (client 5.0 s). Reads all `404 version_withheld`: the two that overlapped the send said `hide_off_releases: false` (the slot was busy: nothing promised, no wait, no 503); the two after it said true |
| H: hide off | `200 {restored: [2]}`; B reads H again (200, v2) |
| H: hide on again | `200 {withheld: true, held: true}` |
| D1 | 2 `blocklist` flags `via: hide_key` (the first resolved `hide_key_off`, the second open); 1 hold `copyright/blocklist` |
| Trim S `keep: 3` | `200 {pruned: 17, oldest_kept: 29, evidence_kept: 0}`; history 31 lines, 3 kept |
| Activity, every page by its cursor | 2 pages, 58 lines, none twice; `email_invited` 3, `hide_key_changed` 12, `version_sent` 33, `bank_shared` 5, `role_changed` 2, `joined` 1, `created` 1, `versions_trimmed` 1. Malformed `before`: `400 {field: "before"}` |
| Remove H, then purge-now | `200 {purged_partial: true, pending_review: true, evidence_kept: 1}`; v1 pruned, the withheld v2 kept; undelete `409 bank_removed` |
| Reads: circle detail, heads, list | 3 x 200 |
| **Extra:** hide on A1 (3.7 MiB; strips and checks the head and v1), B as viewer reads A1, hide off | 200; 200 (4.22 MB stripped body); 200 |

## HTTP status counts

All 149 requests:

| Status | Count |
|---|---|
| 200 | 100 |
| 201 | 9 |
| 202 | 5 (2 sign-ups, 3 email invites) |
| 204 | 2 (account erasures) |
| 404 not_found | 6 (4 tail probes, the viewer's dropped picture, the other editor's read of the held bank) |
| 404 version_withheld | 8 |
| 409 publish_in_progress | 5 (4 in the S bursts, 1 in the seed) |
| 409 conflict | 3 |
| 409 hide_key_changed | 1 |
| 409 bank_removed | 1 |
| 422 invite_not_sent | 2 |
| 422 labels_need_decision | 2 |
| 422 keyless_question | 1 |
| 400 invalid_request | 2 |
| 429 too_many_open_reads | 2 |

On phase C code: 132 requests, the same mix less the phase B seed. Every refusal was one the test asked for.

None of the following occurred: 500, 503 (`busy` or other), `cf_1102`, `D1_ERROR`, `exceededCpu`/`exceededMemory`, a
tail exception, or a `cron_failure` flag from the 16:17 cron.

## Worker CPU and wall time (wrangler tail, per request)

| Route | n | CPU ms | Worker wall ms | client ms |
|---|---|---|---|---|
| **3.7 MiB PUT, A1 v2 (phase C)** | 1 | **not captured** (no tail event) | - | 4,996 |
| 3.7 MiB POST, A1 v1 new bank (phase B code, seed) | 1 | 1,459 | 3,316 | 4,274 |
| **Hide on A1 (strip and check of head and v1, 2 x 4 MB)** | 1 | **4,146** | 5,340 | 5,380 |
| **Viewer read of hidden A1 (4.22 MB stripped)** | 1 | **3,429** | 4,121 | 4,349 |
| Hide on H/HB (small; including the one that withheld the head) | 6 | 15-23 | 732-1,131 | 774-1,168 |
| Hide on refused `labels_need_decision` / `409 conflict` (base) | 1 / 1 | 39 / 4 | 614 / 240 | 659 / 295 |
| Hide off (the restore itself had no event; client 694 ms) | 3 | 6-9 | 469-493 | 513-531 |
| Viewer read of H (small) | 1 | 12 | 487 | 526 |
| Owner read of a withheld head (`hide_off_releases` scan) | 6 | 7-39 | 631-2,166 | 674-2,211 |
| Small send (S), min / p50 / max | 28 | 17 / 18 / 28 | 1,467 / 1,756 (p50 / max) | 1,511 / 1,801 |
| Small sends H/U/HB/mixed (200, 201) | 5 | 15-46 | 1,372-2,802 | 1,415-2,850 |
| Refused sends (`keyless_question`: no event; `labels_need_decision`; `hide_key_changed`; `conflict`) | 3 | 9-18 | 837-1,921 | 880-1,965 |
| Email invite 202 / 422 `address_limit` / 422 `not_deliverable` | 2 / 1 / 1 | 10-13 / 5 / 7 | 792-891 / 1,141 / 278 | 833-932 / 1,175 / 312 |
| Purge-now partial / full purge | 1 / 5 | 6 / 6-8 | 367 / 558-867 | 403 / 604-901 |
| Account erasure (204) | 2 | 51-58 | 362-377 | 398-412 |

Notes:

- **The one 3.7 MiB send.** Its CPU is unknown because the tail missed it. The request succeeded, with a wall time of
  5.0 s at the client. Phase B measured 1.2-1.7 s CPU for the same send, and the phase B code's new-bank POST here took
  1.46 s. Phase C adds one extra share-core pass to every send: the round 47 keyless guard, a `validate_for_publish`
  without `hide_key`. In node it costs about 220 ms against about 390 ms for the send's own `publish_prepare`, on the
  same 3.7 MiB payload. My estimate for a phase C send of this bank is therefore about 1.4-2.0 s CPU. This is an
  estimate, not a measurement.
- **The new cost to note: a big bank with hidden keys.** The owner's PATCH that hides the keys of the 3.7 MiB Matching
  bank took 4.1 s CPU, because it strips and checks the head and the older version. Each viewer read of that bank costs
  3.4 s CPU, because every read strips again. In node, share-core's `strip` of this payload is 2.3-2.5 s, so the
  Worker's numbers are share-core's strip, not D1. The caps hold it to `circle_hidden_reads` 100 a day per viewer and
  20 answer-key changes a day per account, and the work runs under the isolate's heavy slot. A circle of viewers
  reading a large hidden bank is therefore the heaviest new path. It is well within the 30 s CPU limit and caused no
  1102. This bank is an extreme case: 990 questions with 50 Matching pairs each.
- **Owner reads during the big send** took 1.9-2.2 s of wall time but only 7 ms CPU, and answered
  `hide_off_releases: false`. The isolate was busy with the send's synchronous share-core work, and the heavy slot was
  taken, so the scan was skipped. Round 51 intends exactly this: the read did not queue for the slot and no publish was
  refused.

## D1

**Insights** (`wrangler d1 insights --time-period 1h`, sorted by avg reads, count and time; the dumps are in the
scratchpad). The listing again stopped at 60 statements. Most of the new phase C statements are missing: the email
invite statements, `hideOffCandidates`, `STRIPPED_QKEYS_SQL`, the evidence bound, the activity page and the erasure
DELETEs. The run counts also look inflated: the `hide_key = 1` UPDATE shows 56 runs for 11 calls. This matches
phase B's Anomalies item 1. What it did report:

| Statement | avg rows read | avg ms |
|---|---|---|
| `INSERT OR IGNORE INTO circle_holds ... 'student_data', 'privacy' ... PRIVACY_SQL` (2 runs: the 990-question version's ~4k block keys through `json_each`) | 3,963 | 3.26 |
| Copyright block lookup `blocked_hashes ck WHERE kind = 'match' AND hash IN (json_each)` | 26 | 0.59 |
| Copyright hold INSERT in the hide-on batch | 23 | 0.48 |
| Hide-on final SELECT (won, held, withheld, flagged) | 19 | 0.13 |
| `UPDATE circle_banks SET hide_key = 1 ...` | 5 | 0.37 |
| Email hour slot `INSERT INTO pool_alert_budget ... RETURNING sent` | 3 | 0.31 |
| `ALTER TABLE circle_invites/circle_versions ADD COLUMN` (the migrations) | 288-291 | 1.9-2.1 |

No other statement averaged more than 48 rows. The circle statements captured in the window total 16,580 rows read,
1,725 rows written and 378 ms of D1 time.

**Query plans** (`EXPLAIN QUERY PLAN` on staging, for the new statements insights did not report; `eqp.out` in the
scratchpad). All of the following use an index; none scans a table:

- Email: the open-invite lookup (`circle_invites_email_key`, plus a temp B-tree for `ORDER BY ... LIMIT 1` over that
  circle's rows for one mailbox) and the mailbox count (primary key). The " #n" label and open email invites both use
  `circle_invites_circle`.
- Hide keys: `hideOffCandidates` and the hide-on older-versions SELECT (`circle_versions_bank` (bank_id, status), with
  `admin_flags_publish` and the holds primary key for the subqueries). `STRIPPED_QKEYS_SQL` probes the qkeys primary key
  once per `json_each` pair.
- Flags and blocks: the evidence bound's inner SELECT (`circle_versions_withheld` (circle_id, status), with the holds
  primary key, `circle_reports_open` and `admin_flags_publish`), the purge-now partial prune (`circle_versions_bank`),
  the copyright block lookup (`blocked_hashes` primary key per key, `dmca_exempt` covering index) and the flag room
  (`admin_flags_user_at`).
- Activity, erasure and cron: the activity page with a cursor (`pool_audit_item` (item_id, at<?) plus the cursor row
  by primary key), the erasure's invite label clean-up (`circle_invites_creator`), the cron's label clean-up
  (`circle_invites_label`) and the `circle_email_sends` day clean-up (`circle_email_sends_day`).

**Two statements scan a table:** both erasure DELETEs on `pool_alert_budget` (`SCAN pool_alert_budget`). These are the
round 51 domain-share DELETE (substr form) and the round 46 inviter DELETE (`channel = 'circle_email_inviter:' ||
?1`). The table's primary key leads with `hour`, so neither can use it. The table holds only 48 hours of hourly counters
(4 rows now) and erasure is rare, so this is cheap today. It grows with the number of inviters, and each inviter's
domains, in two days.

**The LIKE limit fix on real D1.** Before erasure, A had the `circle_email_inviter:<uid>` and
`circle_email_domain:<hmac>:<uid>` rows (3 each). `DELETE /account` answered 204 for both accounts, and afterwards 0
`pool_alert_budget` rows name either account. The check itself used `instr`, because a LIKE pattern over 50 bytes
would fail on D1.

## Scoped D1 writes (ids and hashes only, never text)

Two throwaway accounts cannot raise a copyright block, because a takedown is an administrator's action. So the hold,
`hide_off_releases` and purge-now-partial steps were set up with one test block, removed afterwards:

1. Approve and redeem A and B: the same statement as phase B, exactly 2+2 rows.
2. Insert one `blocked_hashes` row (`kind 'match'`). Its hash was copied inside the SQL from U's `circle_qkeys` row,
   which is the key-free match hash of this run's own invented question. Its action id is `memtest-<run>-<random>`, and
   there is no `dmca_actions` row. It matched nothing but this run's question.
3. At cleanup, mark H's open flag `evidence_captured` (1 row): what `POST /admin/pool/circle-evidence-captured` records.
   This let the owner's purge-now finish (200 `{ok}`, a full purge).
4. Delete the test block (1 row).
5. After the purges, delete this run's 2 `blocklist` flags (2 rows). They existed only because of the test block.

The two copyright-match admin alerts this raised went to the log only (`[dev mail] to=<admin>`), like every mail in
this run.

## Anomalies and observations

1. **Failed check: the tail saw 2 of the 3 invitation mails.** The tail delivered no event for the second invite
   (`email-B-tag-viewer`). The server sent all 3: `circle_email_sends` shows one mailbox with n = 3, and the inviter
   and domain hour counters show 3. This was tail coverage, not the server: 31 of 149 requests had no tail event (21%,
   phase B 9%), among them the 3.7 MiB send.
2. **Failed check: 4 parallel reads by one account give 2 x `429 too_many_open_reads`.** This is by design (`read.ts`:
   2 open streamed reads per account per address per isolate; `Retry-After: 2`). The test expected 4 x 404. A read of
   a withheld version holds a read slot while its `hide_off_releases` scan runs. Because the cap is per isolate, at
   least three of the four requests landed on the same isolate. Neither result is a 503.
3. **Concurrent hide-on PATCHes each write an activity line.** All 4 hide-on PATCHes of the HB burst, and both
   PATCHes of the mixed burst, read `hide_key = 0` at their start. Each stripped the head, recorded its decisions,
   counted toward `circle_hide_changes` and wrote `circle.bank_hide_key` ("hide_key_changed") within 113 ms. So
   the activity list shows 6 "keys hidden" lines for 2 real changes. Only one of the 4 concurrent hide-off PATCHes
   wrote a line, because hide-off's UPDATE requires `hide_key = 1`. This is harmless (no wrong state, no 5xx) but is a
   small activity and cap nit for the owner. The hide-on UPDATE has no `hide_key = 0` guard, which allows "record
   decisions again".
4. **The account lock under bursts.** In each 4-send burst, each account's second concurrent send got `409
   publish_in_progress` (Retry-After 5), and one send of the four won. The seed's A1 POST also hit A's own concurrent S
   POST and went through on retry. This is by design (one publish or send per account at a time).
5. **The 0113 holds copy ran on an empty table.** The rebuild, CHECK list and index are verified; copying existing
   rows is not, because staging had none.
6. **The insights listing is again incomplete and its counts inflated** (see D1). Query plans stand in for the missing
   statements.
7. **Residue after the purge and erasure, by design:**
   - `attestations` +38 and `pool_audit` +80 (ids, no bank text).
   - 2 user tombstones and `erasure_done` +2.
   - The circle "Memtest circle C" `81697b25…`, closed `owner_closed` and purged by the cron in 13 days. Its name
     remains until then, as with phase A's and phase B's circles. Its 5 bank tombstones have blank text, and its 3
     invites are revoked with labels and email keys nulled.
   - `circle_email_sends`: 1 row, an HMAC of the test mailbox, n = 3. The daily cron deletes it after 2 days.
   - `pool_alert_budget`: 3 hourly counters (the domain HMAC, `circle_email_invite`, `circle_copyright_match`). They
     name no account, and the cron deletes them after 48 hours.

No 500s, D1 errors, 1102s or 503s occurred, real mail could not go out, every cleanup step succeeded, and no step
needed a stop.

## Cleanup verification

**Through the API:**

- A removed and purged A1, S, H, U and HB: 5 x (200, 200), all full purges. H's second purge came after its flag was
  marked captured.
- A closed the circle: 200.
- B was erased (204), then A was erased (204).

**Test accounts** `41bf714b-…` and `e1a163b0-…`:

- `deleted` 1, `email_gone` 1, `pw_gone` 1.
- Sessions 0, redemptions 0, pool locks 0, pending blobs 0.
- `pool_alert_budget` rows naming them 0, `erasure_done` 2.

**Circle `81697b2559fde263a09c39c138445fe6`:**

- Closed (`owner_closed`, purge set, 13 days), members 0.
- Invites 3, all revoked, with 0 labels and 0 email keys.
- Banks 5, all purged, 0 with text.
- Versions 0, qkeys 0, holds 0, version blobs 0, flags 0.
- Test blocks 0, `r2_orphans` 0.

**Whole database against the pre-run baseline:**

- users 15 (13 + 2 tombstones), live 1 (unchanged); `vershion-memtest-%` emails 0.
- circles 3 (all closed), open 0; circle banks 12 (live 0).
- circle versions 0, members 0, qkeys 0, holds 0, reports 0, version blobs 0.
- items 19, `r2_orphans` 0, `pool_locks` 0.
- `admin_flags` 13, all from before the run (1 `cron_failure` of 10-23 and 12 `evidence_overflow` of 10-24).
- `blocked_hashes` 0, `privacy_blocks` 0, blobs 0, pending blobs 0.
- D1 size 180.7 MB.

**R2:**

- Every one of the 78 keys recorded before the cleanup (76 circle body and index objects, pruned ones included, plus
  the 2 images) returns "The specified key does not exist". The images went at the 16:17 hourly blob GC, after the
  erasure released their pending upload rows.
- `r2 bucket info`: 49 objects and 196 MB, the same as before the run.

## State left on staging

- `vershion-pool-staging` runs `ed43235c-4a7a-4e1a-8087-b6d244f219d6`, the phase C code (HEAD `b228485`) with
  `workers_dev = false`. workers.dev answers 1042. The cron (`17 * * * *`) is unchanged.
- D1 `vershion-pool-staging` is at migration `0115`; `d1 migrations list` has nothing to apply.
- Both throwaway accounts are erased. Their passwords, verification tokens, circle codes (the mailed ones included,
  read from the log-mode mail line in memory), access tokens and merge tokens existed only in the run process. None
  was ever printed, saved, or passed in argv or env. The saved tail logs have addresses and codes scrubbed.
- One closed test circle is waiting for the cron purge (day 14), plus the residue listed above.
- `server/worker/wrangler.staging.toml` is unchanged (`git diff` empty). No repo file changed except this report,
  which is not committed.

## Files (scratchpad, not in the repo)

All under `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/phaseC/`:

- `run-c.mjs`: the run (seed, wait for GO, load test, cleanup).
- `genc.mjs` and `gen.mjs`: payloads and PNGs.
- `valc.mjs`, `mh.mjs` and `cost.mjs`: the local share-core checks and timings.
- `r2check.mjs`: the R2 key check.
- `insightsc.mjs`, `insights-*.json` and `eqp.sql`/`eqp.out`: the insights summary and the query plans.
- `baseline-d1.json`: the pre-run counts.
- `run.log` and `results-c.json`: the run's output and every request with tag, status, CPU and wall.
