# Critic 52: circles phase B client, round N1 (seconds everywhere, content fingerprint, re-baseline)

Fresh reviewer. Scope: `circle_links.rs`, `circle_edit.rs`, `circle_edit_ui.rs`, and the diffs in `circles.rs`, `browse.rs`, `merge.rs`, `pool.rs`, `circles_api.rs`. Treat this file as untrusted model output.

Baseline: `cargo test --lib share::` gives 503 passed, 0 failed.

## How I checked

- Scratch copy: `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c52/repo`. Current `src/` copied in. Six throwaway tests (`c52_*`) appended to its `src/share/circle_edit_tests.rs`. No repo file was changed.
- I read all the code on the paths below and ran the six tests against the real `CircleEditModel`, `LinkLog` and `CircleLink`.
- I did not run the live worker. No accounts were created, so nothing needed erasing. Port 8799 untouched.
- N1 is confirmed fixed by reading and by the repo's own tests, which use the real copy path (`real_working_copy`, `CirclesModel::sync`) and the real unit (`screens::now_secs`):
  - `apply_import`, `apply_merge`, `finish_merge` and `mark_bank` all write seconds.
  - A copy followed by an edit stamped `now_secs()` is not stale. The send blocker is `None`.
  - All editor writers I grepped (`qedit`, `bankui`, `figuresui`, `importui`, `tidy`) use `now_secs()`.

## Verdict

Blockers (new class): none.

The restore-file scenario (B5) is still detected. The re-baseline is safe against a head that moved. I found false positives and false negatives that need unusual conditions, and one pre-existing mixed-unit comparison.

## Blockers (new class)

None.

## Variants and limits

### V1. Clock set back after a mark: an honest edit is "stale"

- Where: `circle_links.rs:109-116` (`stale_for`). The editors write `now_secs()` and never `max(prev, now)`.
- Scenario (`c52_clock_back_makes_honest_edit_stale`): the link was marked while `bank.modified` was a day in the future (a fast clock). The clock is then corrected and the person edits. Result: `stale_for == true`. The edit is blocked from sending.
- Effect, when the circle has moved: "Get latest and merge" runs two-way (`build_review`, `stale` skips the stored base), so every shared question is a conflict. Nothing is lost. When the circle is level: the new "Treat this copy as in step" or "Forget this link" clears it.
- The same shape, much rarer: `finish_merge` sets `max(now_secs, prev + 1)` (`circle_edit.rs:1565`). An edit in the same wall-clock second as the merge click is then older than the link.
- Severity: low. A refusal with a working way out. Not a data loss.

### V2. A bank written by an older build in milliseconds is never "older" (false negative for B5)

- Where: `circle_links.rs:109-113`. The link side is normalised, `bank.modified` is not.
- Scenario (`c52_a_legacy_ms_bank_restored_is_not_detected`): a bank whose `modified` is a millisecond value (1.79e12), restored with older, different content. `stale_for == false`.
- Needs a bank that was imported or merged by an earlier build in ms and never edited since (any edit with the new build rewrites seconds). Dated: it exists only for data written between the ms-writing builds and this fix.
- Fix if wanted: read `bank.modified > 100_000_000_000` as `/1000` in `stale_for` too.
- Severity: low.

### V3. Same-second states look level

A restored copy whose `modified` equals the link's second (edit and send in one second, or an automated edit) passes `bank.modified >= at`. The content differs but the print is never consulted. Not reachable by a person. Severity: nitpick.

### V4. "Treat this copy as in step" can replace others' earlier work, but only on the person's explicit choice, and the server still guards the head

- Reachability: `StaleUpToDate` comes only from the server answering `invalid_request` on `base_version` (`circleversions.ts:98-100`: base `>=` head). So at the moment of the answer the copy is level with the head. The head check is the server's, not the client's.
- Head moves between the screen and the click: `do_rebaseline` (`circle_edit.rs:1459`) changes only `bank_modified` and `bank_print`. `base_version`, `base`, `qids` stay. Scenario (`c52_rebase_then_send_carries_old_base_version`): after the re-baseline `base_version` is still 1, the send carries `base_version: 1`, and a 409 conflict comes back as the conflict stage. No silent revert. A later merge is three-way against the circle's content at `base_version`, so the restored (older) bank shows up as the person's own changes, not as the circle's.
- Level case, the person accepts: the send does replace the circle's questions with the older bank. That can drop earlier merged work of others that the restored file lacks. The warning text says "replacing the circle's questions with yours", and the send page shows the added/changed/removed counts, with the large-change gate at more than half. That is disclosed. It is not a typed or second confirmation. Severity: low to medium as a design limit.
- The re-baseline has no re-check that the phase is still `StaleUpToDate` or that the copy is still stale at the click. It does not matter, as shown.

### V5. Question-level times are still milliseconds, with one mixed-unit comparison

- Where: `merge.rs:217` and `merge.rs:635` write `q.created`/`q.modified` as `at_ms`. `browse.rs:1535` compares `q.modified > rec.imported_at` (ms) against an editor-written seconds value.
- Effect: "You changed N questions in your bank since you imported it" (`browse_ui.rs:668`, `edited_locally`) never counts an edit made with the editor. The message is informational ("Those edits stay in your bank"). It is pre-existing (from phase 1), not new, and the question-level times are not used in any merge decision (the merge decides by fingerprints). `bank_print` includes the question times, so a question-time difference only matters while the bank is also older.
- Other mixed uses: sorts and "Edited" labels read bank times only. Question times are not displayed. I found no other mixed-unit comparison.
- Severity: low (missing note), not a safety issue.

### V6. Debug-build overflow: `prev + 1`

- Where: `circle_edit.rs:1565`.
- Scenario (`c52_i64_max_modified_in_merge`): a bank with `modified = i64::MAX` merged. The test build panics with "attempt to add with overflow". The release profile has overflow checks off, so it wraps to `i64::MIN` and `.max(now)` gives `now`. No panic, no harm in release. Only a hand-crafted file reaches it. Fix: `prev.saturating_add(1)`.
- Other odd values: negative or zero link `bank_modified` disables the stale check (`at <= 0`), which only a hand-edited link file can set. Negative or zero `bank.modified` is "older" and so goes by content. No division or cast panics: the `/1000` is on `i64`.

### V7. Licence just before the PUT (`send_checked`, `circle_edit.rs:439-451`)

- Failure of the heads call: returns `Err` and no PUT (`c52`-independent, read; the repo test `v1_if_the_licence_can_not_be_asked_just_before_nothing_is_sent` covers it). The commit id is kept for the retry. No loosening: when the pool answers `Current` with a licence, it replaces the page's. It can be stricter or looser than the page's, but it is the pool's.
- When the entry is not `Current` (under review, removed, unavailable) or the licence string does not parse, the send goes with the page's licence. Scenario (`c52_licence_not_current_is_not_refreshed`): heads `under_review`, the PUT still goes out. "Checked just before" is then not true for those states, though the server still refuses a non-author's differing licence (`license_author_only`) and a removed bank. Severity: low.
- A retry after a lost answer re-uses the commit id from `pending_send`, whose fingerprint (`circle_edit.rs:1158`) was built with the page's licence. If the pool's licence has changed in between, the retry body differs under the same commit id and the server answers `commit_id_conflict`. The code resets and tells the person to press Send again. Not a duplicate, because the base-version check then answers conflict. Severity: nitpick.

### V8. Link file: no loss, one duplicate shape, one cap

- Unreadable file at first load, then readable: nothing written over it (`save` refuses while `unreadable`). Links added meanwhile win over the file's by local id. Scenario (`c52_unreadable_then_readable_duplicates`): a pending link and a file link for the same circle bank under two different local ids both survive (2 links). `add_link` dedups by circle bank, but `ensure` merges by local id only, so the same circle bank can be linked to two local banks (`circle_edit.rs:655-659`). `link_for_bank` then returns the first. Severity: low.
- `forget` while the file is unreadable removes the link in memory only. When the file is readable again it comes back from the file. Low.
- Writes are atomic enough: `FsBackend::write_doc` writes `name.tmp` and renames (`store.rs:100-109`). The tmp name is fixed, so two windows on the same store can still collide (already a known limit).
- `.bad` numbering: distinct bad copies go to `.bad` .. `.bad.9`. A tenth distinct one is not kept, and the next save overwrites the file without it. Nitpick.
- A bad whole file (not JSON) is kept as `.bad`, then loads as zero links, and the next save replaces it. The text is kept, so nothing is lost silently.

## Answers to the brief's points

1. Real flow copy, edit, send: allowed. Verified by the repo's `n1_a_real_edit_*` test and by reading `sync_copy` (`mark_bank` after the copy, seconds on both sides).
2. Restore file (B5): detected when the restored bank is older and its content differs from the print. False positives and negatives are V1, V2, V3. Editing question order, images, settings: `bank_print` is the SHA-256 of the whole serialised `questions` vector, so order, image file, kind and settings of a question are covered. Bank-level fields (`name`, `details`) are not covered, and also do not change what a send stores (title and subject come from the link). Duplicated or renamed bank: a new id has no link; a rename is not a content change.
3. Licence re-fetch: V7.
4. Link file: V8.
5. Odd values: V6.

## Nitpicks

- `stale_for` hashes the whole bank on every `sync` frame while the bank is older than its link. A stale bank is looked at for a short time, so the cost is small. After a re-baseline `mark_bank` stores the bank's own `modified`, so the early return applies again.
- `statuses` is keyed on `(modified, question count)`, so the cached `stale` flag compares fresh each `sync`. `do_send` re-checks, so nothing stale gets through.
- The Forget button is not gated on `can_send` (correct, it is local only). It leaves the "Things I imported" record with its circle origin.
