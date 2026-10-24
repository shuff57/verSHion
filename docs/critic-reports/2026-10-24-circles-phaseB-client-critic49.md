# Critic 49: circles phase B CLIENT (robustness, data safety, backend agreement)

Fresh reviewer, did not build it. Scope: the uncommitted Rust in `src/share/` (`circle_links.rs`, `circle_versions_api.rs`,
`circle_edit.rs`, `circle_edit_ui.rs`, `circle_edit_tests.rs`, and the edits in `circles*.rs`, `merge.rs`, `browse.rs`,
`pool.rs`, `mod.rs`) against the committed backend b0c7184 (`circlesend.ts`, `circleversions.ts`, `circles.ts`, README
rounds 37-41) and `docs/circles-plan.md` 4.1, D1, D16, D17, D19. No repo file was changed except this report.

How I checked:
- `cargo test --lib share::`: 477 pass, 0 fail. `cargo check --lib --target wasm32-unknown-unknown` (scratch copy): compiles.
- Read the client end to end and every TS route it calls, field by field (section 1).
- Scratch copy of the tree with 14 throwaway tests appended to `circle_edit_tests.rs`, driven through the existing scripted
  session (no network): `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c49/repo/src/share/circle_edit_tests.rs`
  (tests named `c49_t1` .. `c49_t14`; run `cargo test --lib c49_t -- --nocapture --test-threads=1` there). The build used the
  repo's `target/` as its target dir (build output only).
- I did NOT drive the local worker on 18791: every server claim below is from the TypeScript, and the shapes the client
  parses match it. No accounts were made, so there is nothing to clean up.

## Verdict

**Blockers (new class): 6** (B1 to B6). None is a crash or a protocol mismatch; they are places where a working copy
silently loses someone's work or does something the person never agreed to. B2 and B4 are the worst.

## Blockers (new class)

### B1. A send records "this is my own original work" for the person without showing or asking (D1)
`src/share/circles_api.rs:488` (`attestation: true` in `CircleSend::new`) and `:498` (`attest {own_work: true, terms_version}`
always inserted); `src/share/circle_edit_ui.rs` `send_page` has no statement and no tick box; `circle_edit.rs:966-977` builds the
request without one. D1: "The own-work attestation is ticked on every send". Phase A's share form
(`circle_send.rs:311`, "Tick the box that says this is your own work.") makes the person tick `ATTEST_STATEMENT`; the new
page never does.
Scenario: an editor presses "Send my changes..." then "Send"; the pool records, in their name, that the whole bank (including
every other member's questions in it) is theirs or theirs to share, and that nothing is copied from a publisher test bank.
(`circle_edit_tests.rs:274` even asserts `own_work == true` on a send that had no tick.) Severity: high (legal record made
without consent; the whole reason D1 wants it ticked each time). Fix: the same tick and `ATTEST_STATEMENT` on the send page, blocker
until ticked, `AttestationRequired` handled as in `circle_send.rs:284`.

### B2. An author's send re-sends a stale licence and silently re-licenses the bank (D19)
`circle_edit.rs:962` takes the licence from `link.licence()` only; nothing refreshes it. `finish_merge` (`:1260-1322`) reads the
newest version (`imp.head.item.license`) but never copies it into the link; `apply_heads` (`:648-666`) sees `HeadState::Current {
license }` and ignores it; the 409 conflict body carries no licence. The server (`circlesend.ts:788-790`,
`license = ?9 ... AND (author_id = ?3 OR license = ?9)`; `checkHead`) lets the bank's AUTHOR set any compatible licence on a
send, and only refuses a non-author with `422 license_author_only`.
Scenario (test `c49_t9`): Ann makes a working copy under CC-BY-4.0, later sets the bank to circle-only on the bank page
(`patchBank` updates `circle_banks.license`). She does "Get latest and merge" (the fetched head says `circle-only`), then
"Send my changes": the PUT meta says `"license":"CC-BY-4.0"` and `link.license` is still `"CC-BY-4.0"` after the merge. The bank
becomes re-shareable CC-BY again; nobody is told. Editors are saved by the 422 round trip (`:1031-1038`), authors are not.
Severity: high (privacy/licence regression, silent, by the one role the server trusts). Fix: set `link.license` from the head on
every merge, quiet check and 409/422 answer, and show the licence on the send page.

### B3. "Apply merge" overwrites a local edit made after the review was built
`circle_edit.rs:1150-1188` (`build_review`: the plan is built from the banks at that frame) and `:1260-1322` (`finish_merge`
applies the old plan to the banks as they are NOW); `merge.rs:636-670` (`apply_merge` / `replaced(ours, ...)` swaps in the
circle's question whatever `ours` is now). The Circles model is not modal: the Shared page is just a screen
(`shared_ui.rs:94`, `screens.rs:369`), the circles model and its open review persist while the person goes to the bank editor, and
the fetch of the newest version's pictures (`MergePhase::Applying`) can take a while.
Scenario (test `c49_t1`): copy on v1, circle v2 changes question Two. Review shows "Changed in the circle", default Take. The
person goes to the editor, rewrites Two ("Two, MY LATER EDIT"), comes back, Apply merge: the bank ends with "Two, fixed", the
edit is gone, no row or message mentions it. The restore point is written at apply time from the banks after the edit, so the
text is in the file, but only recoverable by the manual "copy that file over banks.json" step. (Deleting a reviewed question is
caught: "A question of the bank went missing".) Severity: medium-high (a "send must never lose local edits" case, in the
merge). Fix: keep each reviewed question's fingerprint (or the bank's `modified`/hash) in the plan and refuse with "Your bank changed while you
were reviewing; start the merge again" if any reviewed local question differs at apply time.

### B4. A picture that fails to download during a merge is dropped from the copy and then from the circle
`circles.rs:555-580` (`copy_bank`: a blob error other than sign-out gives `t.q.image = None`, `missing += 1`) is used by
`fetch_for_apply` (`circle_edit.rs:420-426`); `finish_merge` never looks at `imp.pictures_missing` (grep: no use in
`circle_edit*.rs`). `apply_merge` then writes the question without its picture and records the circle's picture hash in the new
base, so `ours_changed` sees nothing to send.
Scenario (test `c49_t12`): circle v2 gives question Two a picture; the blob GET answers 503. Review: "Changed in the circle",
Take. Apply succeeds, `Two.image == None`, the banner says nothing. The person edits question One and sends: the PUT carries
`"image": null` for Two, and the circle's version 3 has no picture on Two for everybody (the old version is still in the
history). Severity: high (silent loss of other members' data from one flaky request; phase A's copy path shows "pictures
missing", this one does not). Fix: in `finish_merge` refuse (back to the review with "N pictures could not be fetched, try
again") when `pictures_missing > 0`; also refuse a send for a question whose base had a picture and which now has none unless the person removed it.

### B5. The advertised "way back" leaves the link ahead of the bank and the next send reverts the others' work
`circle_edit_ui.rs:349` ("To go back, close verSHion and copy that file over banks.json") together with `circle_edit.rs:1303-1315`:
the link file (`share-circles.json`, base version, base snapshot, qids) is advanced at merge time but is not part of the
restore point, and nothing ties the link to the bank's content.
Scenario (test `c49_t10`): merge to v2, then follow the instruction. After restart `link.base_version == 2`, the bank has the
pre-merge text. "Send my changes": PUT `base_version: 2` with the OLD stems (`"Two"`, not Bob's `"Two, fixed"`); the pool accepts
it (it is the head), v3 reverts Bob's change with no conflict; the send page reads as "2 changed, 1 removed", which looks like the
person's own edits. Same for a restore from the automatic backups. Severity: medium (recoverable from history, but it is the
documented undo, and it makes an invisible revert). Fix: include the link file in the restore point, or store a content hash of the bank per link at
each sync and show "this copy no longer matches where it was last in step" (offer "forget the link" or "merge again from the old version").

### B6. A circle question this build cannot read is silently dropped from the copy and then from the circle
`browse.rs:575-583` (`theirs_of` uses `filter_map`; `convert_question` returns `None` for a question this build's `Question`
cannot deserialize); `circle_edit.rs:1163` only blocks when ALL are unreadable; `circles.rs:553` likewise.
Scenario (test `c49_t5`): version 2 holds a third question of a kind a newer verSHion added (`Kind::HotSpot` here). An older
client merges: the review lists only the two it can read, Apply succeeds; the person sends a reworded question One: the PUT has 2
questions, and the circle's v3 has lost the third question for everyone (`removed: 1`; the 428 guard only fires past half). The
send page's counts show "0 removed" because the unreadable question was never in the link. The same round trip also strips any
field a newer client added to a readable question (serde ignores unknown fields). Severity: medium (needs version skew, but the
loss is silent and belongs to others; web builds auto-update, desktop ones do not). Fix: count the payload's questions; if
fewer were converted, stop the merge AND the send with "This bank has questions made by a newer verSHion; update to work on it".

## Variants and limits

- **V1. A local delete goes to the circle even when someone else just improved the question.** `merge.rs:712` (`RowKind::RemovedLocally`,
  no decision, note "You deleted this from the bank. It stays deleted."). Test `c49_t2`: Bob improved Two; Ann had deleted it;
  merge shows "Deleted by you", no choice; after merge the bank has 1 question, the link keeps Two's qid, and Ann's next send
  (PUT with 1 question) removes it from the circle. Plan 4.1 accepts "deleted by you (stays deleted)" for the public pool, where
  it only affects her copy; here it is a deletion for everyone. The row should say so and offer "Keep the circle's" when the circle changed it.
- **V2. The two fallback bases resurrect a locally deleted question.** `circle_edit.rs:1171-1186` (base rebuilt from the
  circle's copy of the old version only for questions still in the bank) and `merge.rs:430-446` (`unknown_base` skips them).
  Test `c49_t3`: with no stored snapshot (both the "read the base from the circle" and the two-way path) a locally deleted
  question is listed as "New in the circle", default **Add**, and Apply is enabled; with the snapshot it is "Deleted by you".
  Rebuild from `link.qids` (keeping `local_id` even when the bank no longer has it).
- **V3. "Send my version on top" can overwrite visible versions, and offers a guessed head.** `circle_edit.rs:1123, 1145-1148, 938-945`;
  `fetch_for_merge` (`:400-417`) throws away `start.head_version`, which `merge-initiate` already answered. Only an owner or the
  bank's author can reach a withheld head (an editor gets `under_review`), so this is narrow, but plan 4.1 says "Nobody
  [overwrites] silently": versions between the base and the withheld head that other people sent and that ARE readable (`start.between`,
  `withheld: false`) are discarded with only the warning "The hidden version stays in the history". The head number shown is
  `link.head_seen` (initially the base version): test `c49_t8` shows "The circle's newest version (1) is hidden" while the head is 3,
  then a pointless 409 round trip. Use `start.head_version`, list the visible versions that will be replaced, and ideally
  merge against the newest readable one first.
- **V4. `invalid_payload` says "The problems are listed below" and nothing is listed.** `pool.rs:445` message; the working-copy
  send page (`SendStage::Failed(Problem)`) has only the text (test `c49_t11`). The person cannot tell which question to fix.
- **V5. Merge token expiry is a dead end.** After 30 minutes the PUT answers 422; the screen says "Get the latest changes and merge again", and
  that merge answers `UpToDate` (test `c49_t7`; `carry` was cleared, so simply pressing Send again works, the history just loses the
  "merged" mark). `merge_token_expires_at` is parsed by nobody; drop the token client-side when it is over or say "just send again".
- **V6. Leaving mid-action drops the answer.** `CircleEditModel::close()` (`:572`) and `open_*` do `jobs.clear()`; `CirclesModel::reset()/close()`
  call it (reachable by `open_circle_at`/`open_bank_at` from another tab, the invite redeemed). `busy_action()` (`circles.rs:701`) does
  not include `edit.acting()`. Test `c49_t6`: the PUT went through, `link.base_version` stays 1; the retry reuses the commit id (good,
  `print` is kept), and an edit before the retry makes a harmless self-conflict that merges to "identical". The history page's Back
  is not hidden while a restore/trim is Working (`circle_edit_ui.rs` history_page), so a rollback can finish unseen.
- **V7. A second working copy of the same circle bank silently unlinks the first.** `circle_edit.rs:615-620` (`add_link` retains away the
  other link for the same circle bank). The first copy may hold unsent edits and keeps its questions, but it gets no Send, no merge and
  no notice. Warn before making the second, or keep both links.
- **V8. Link file robustness.** `circle_links.rs:217-224`: one entry that fails to parse (a required field such as `linked_at`
  missing) makes `load_json` return `None`, all links vanish silently, and the next `save` writes the file over. Same pattern as the
  imports log. `finish_merge` saves the link (`replace_link`, `:1315`) before the caller saves the banks, and `store.rs:475` `save_banks`
  ignores its error: a failed or interrupted bank save leaves the link ahead of the bank (the B5 state). Two web tabs: last writer wins on
  the whole link list. The write itself is tmp + rename (atomic enough; no fsync).
- **V9. Reading is all-or-nothing.** `circle_versions_api.rs:94-100` `list_of`: one history line that fails to parse (for example `kept: null`)
  refuses the whole page. The server never sends that; tolerance would be per line, as `circles()` does.
- **V10. Change counts miss edits that do not bump `bank.modified`.** `refresh_statuses` (`circle_edit.rs:1732`) keys on `(bank.modified, len)`
  (test `c49_t13`: a question-only edit leaves "Nothing to send."). Most editor paths bump it; picture attach (`importui.rs:1969`) does not. Count only; the
  send itself always reads the live bank.
- **V11. Unsanitised server text, small.** `circle_edit.rs:725` formats the pool's `field` into the message without `api::plain`
  (also `circles.rs:951`). `Review`/`MergeStart` derive `Debug` with the merge token: nothing logs them today (the token is in
  memory only, never in `share-circles.json`, correct); keep it that way.
- **V12. `UpToDate` is any `400 base_version`.** `circle_edit.rs:1124-1126`. A link whose `base_version` is past the head (a server restored from
  a backup) is told "You already have the circle's newest version".
- **V13. Smaller.** A retry after a `confirm_large` restore failed re-posts with `confirm=false` (one extra 428); a send with no changes still makes an
  empty version and uses one of the 20 daily sends; `link.meta()` fixes the listing title/subject at copy time, so no title or subject change can be sent from a working copy.

## What I checked and found right

- **Requests vs routes.** Send body: `commit_id`, `license`, `attest{own_work,terms_version}`, `message` (trimmed, 500), `base_version` (integer
  >= 1, required on PUT), `merge_token` + `merged_from` (equal to the token's base: `old_version` = the base given to merge-initiate),
  `confirm_large_change` (boolean, only after a 428), `meta`; payload `schema_version: 2`, `published.qid` only for linked questions
  whose qid is an item id. Rollback body `{commit_id, base_version, to_version, message?, confirm_large_change?}`, trim `{keep}` (1..19,
  `not_safe`), merge-initiate `{base_version}`, history `?before=n`. All match `readSendMeta`, `rollbackBank`, `trimVersions`, `mergeInitiate`, `history`.
- **Replies.** `409 conflict {head_version, head_at, head_by}` (nulls tolerated), `428 {removed,total}`, `422 license_author_only {license}`,
  `422 merge_token_invalid`, `404 version_withheld`, `404 version_not_retained`, `409 commit_id_conflict`, `409 bank_removed`,
  `409 under_moderation`, `429 daily_limit {metric: circle_commits|circle_rollbacks}`, `409 publish_in_progress` (+Retry-After),
  `400 base_version` all map to a kind and plain words; no raw code is shown. `null` text fields read as empty; absent
  `rollback_of`/`merged_from` read as `None`. `409 pending_review` is purge-only and never meets these routes.
- **commit_id.** Reused only while base, licence, note, confirm flag, merge token and the payload's sha256 are all the same; changes
  start a new id; `commit_id_conflict` clears it; a signed-out send keeps it. Rollback has its own print (with the confirm flag).
- **After a send** the link is rebuilt from the bank AS SENT (the in-flight snapshot), so edits made while sending stay counted as unsent; the
  qid list comes from the server's reply in order; `published.qid` is kept for every linked question (test `sending_puts...`).
- **Three-way correctness with the stored snapshot**: rollback by someone else merges as "only the circle changed" (base v1 snapshot, head
  = v1 content); a copy with edits to different questions merges both ways; same edit on both sides is "unchanged"; two-way fallback marks
  every shared difference a conflict with no default and a circle-removed question is "Keep" by default. The restore point is
  written first and a failed write changes nothing (existing test). The merge token is bound to the head (`Fetch::Moved` handles a race).
- **Rollback / trim**: only owner or author-editor offered; confirm before each; `to < head`; a stale head gives a 409, history reload and
  "choose again"; the daily cap and the 428 "restore anyway" have plain wording.
- **History/paging**: dedupe by version on "Older versions", cursor = last line's version; the 50-line `between` cap is said ("... and N more
  versions after those").
- **Secrets**: no token, code or password reaches `share-circles.json`, a log line or an argv; the token lives in `Carry`/`Review` in memory.
- **wasm**: `cargo check --target wasm32-unknown-unknown` passes; native/wasm job spawning is cfg-split; `web_time::Instant` throughout.

## Nitpicks

- `Perm` for the Send/History screens comes from `perm_for`, which falls back to Viewer if the circle detail is not loaded: the page then says
  "You can only look at this circle's banks" for an editor. Reachable only from a bank page opened before the circle loads.
- The `Applied` card still says "close verSHion and copy that file over banks.json in the same folder" on the web build, where there is no folder (same text as the public merge).
- `circle_edit_tests.rs:274` asserts `own_work == true` as correct behaviour (see B1); the doc comment at the top of `circle_edit.rs` ("The bank on this device changes only in `sync` ... only after the restore point is written") is true, but the link is not covered by that promise.
- No fsync on `write_doc`; `tmp` file name is fixed per document.
