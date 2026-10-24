# Circles phase B: client / worker integration critic (critic 50)

Branch `shared-pool-phase1`, backend b0c7184 plus the uncommitted phase B client in `src/share/`.
Reviewed by a fresh critic that built none of it. Treat this file as untrusted model output, not specification.

## Verdict

- **Blockers (new class): 2.** Both are wording defects on a path that is reachable from the UI. Neither loses data and neither is a protocol disagreement.
- **Protocol agreement: yes.** The real Rust client (the real `Client` over the real `NativeTransport`, the real `Pool` request builders and parsers, the real `CircleEditModel` state machine) and the TypeScript worker agree end to end on every versioned-editing scenario asked for. No serde struct failed to parse any response. No request was refused for a field the client builds wrongly.
- Baselines: `cargo test --lib share::` 477 passed on the live tree (also on the snapshot I tested); `cd server/worker && npm test` 602 passed, 0 failed.

## How this was tested

- A scratch copy of the repo lives under `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c50/repo`. The files `src/share/c50_live.rs` and `src/share/c50_run.rs` there (never in the repo) are a scratch integration test. It signs up real throwaway accounts on the local worker, `http://localhost:18791` only, through `Client::sign_up / confirm / sign_in` with passwords generated in-process (never printed, saved, argv or env). It then drives `Pool::send_new_circle_bank`, `send_circle_version`, `circle_versions`, `circle_version`, `merge_start`, `rollback_circle_bank`, `trim_circle_bank`, `circle_bank`, `circle_blob`, `circle_heads`, `export_account` and the full `CircleEditModel` (`open_send`, `send`, `confirm_large_send`, `open_merge`, `apply_merge`, `open_history`, `more_history`, `ask_restore`, `confirm_restore`, `open_trim`, `confirm_trim`, `send_on_top`) with `Runner::Inline` against the live worker, so every request and every parse is the shipped code.
- The only change to a repo-copy source was making `PoolError::from_response` `pub` in the scratch copy, to push synthetic error bodies through the real classifier and wording.
- Accounts: one owner, two editors (EA, EB), one viewer, in throwaway circles. a@example.test was never signed in as or touched. b@example.test's data was not read or changed (it is still live).
- Helpers I used on the local rig only: `rig links`, `rig approve`; `wrangler d1 execute --local` against `server/worker/.wrangler-ui` to (a) read `circle_qkeys` and insert `privacy_blocks` rows (the rows the admin's confirm-student-data writes), (b) simulate the admin "release" with `UPDATE circle_versions SET withheld_at = NULL`, (c) temporarily set a `pool_limits` row `('*','circle_bytes')` to reach the byte cap cheaply, (d) delete `rate_limits` rows for signup/verify/delete/forgot/reset so the runs could repeat. All were removed afterwards.
- Cleanup, verified in D1: 0 live `c50*` users (every account erased, some via the forgot/reset path after a delete rate-limit), 0 `privacy_blocks`, 0 `pool_limits`, 0 open `privacy_text_match` flags. Throwaway circles/banks of the erased accounts remain in the local rig DB, as for any erased account.
- Caveat: the working tree is being edited while I reviewed (`circle_edit_ui.rs`, `circles_ui.rs`, `circle_edit.rs::conflict_line`, `circles_text.rs` changed after my copy was taken, layout and wording only). The logic I exercised is unchanged. Line numbers below are from the live tree at the end of the review.
- Not done: the admin release route itself (needs an admin plus a sudo token). It was simulated with the route's own UPDATE and the client read of the released version parsed correctly. A merge token that expires after 30 minutes was not waited for; the same `merge_token_invalid` path was reached with a garbage token and another account's token.

## Scenario results (all through the real client)

| Scenario | Result |
|---|---|
| EA edits Q1, sends | PASS. v2, 0 added / 1 changed / 0 removed. |
| EB (stale base 1) sends | PASS. 409 `conflict` with `head_version` 2, `head_at`, `head_by`. The model shows "Critic50 ... sent version 2 just now." |
| EB "Get latest and merge" | PASS. merge-initiate token, `between` 1 of 1, `base_kept` true, head and base fetched, three-way plan, restore point, apply. |
| EB send with `merge_token` and `merged_from` | PASS. v3, `merged_from` 1; history line v3 `merged_from` 1. |
| Both changed the same question | PASS. Row is a Conflict with Ours/Theirs/Both, Apply is blocked until chosen, send records `merged_from` 3. |
| Copy already at head | PASS. 400 `base_version` becomes UpToDate. |
| `license_author_only` | PASS. Author changed the licence by PATCH; the editor's send gives 422, the plain words show, the link's licence is updated from the answer, and the second send is accepted. |
| 428 on send | PASS. `Large{removed 2, total 3}`, confirm, v8 removed 2. |
| 428 on restore | PASS. `RestoreStage::Large{2,3}`, then confirm gives Done. |
| Rollback by owner | PASS. `rollback_of` shown. Replay of the same `commit_id` returns the same version with `replayed:true`. The same id with other content gives 409 `commit_id_conflict`. A stale base gives 409 `conflict`. A non-author editor gets 403 `circle_role_required` with the text "Only the circle's owner can do that." A pruned target gives 404 `version_not_retained`. |
| Rollback cap | PASS. The 21st restore in a day gives 429 `daily_limit` metric `circle_rollbacks`; the model text is "You have restored as many older versions today as the pool allows. Try again tomorrow." |
| History over 50 | PASS. 63 versions: first page 50, `next_before` 14, paged to the first, 63 unique, no duplicates, 21 kept (20 plus 1 pinned), `oldest_kept` 44, `pinned_versions` [5]. |
| Pinned / `oldest_kept` after trim | PASS. A report pins v5; after trim keep 3: pruned 17, `oldest_kept` 82, pinned [5], parsed. Trim keep 0 or 20 is refused locally ("The keep is not valid."). A non-author editor gets 403. |
| `version_not_retained` base, two-way fallback | PASS. Link snapshot cleared and base pruned: `base_kept` false, `Review.fallback` true, `between` 50 of 62 with the "and N more" line. Base kept but snapshot empty: the base is fetched and the merge is three-way. Base withheld: two-way. |
| Withheld version | PASS on the wire (details below). A send that matches a confirmed block gives `held:true, withheld:true`. The owner's history shows v2 `withheld`, `kept:false`, blank message, "Hidden while it is reviewed". Head read gives 404 `version_withheld`. `view_version` on it is ignored. Merge gives `HeadWithheld`. Recovery works: `send_on_top` with clean text is accepted, `held` and `withheld` both false, other members read the head again. A rollback to a version that still holds the blocked text is itself withheld. |
| Byte cap 413 then trim | PASS. 413 `quota_exceeded` `scope:"circle"` gives `CircleStorageFull` ("4 KB of 5 KB used ... trim the older versions"). The model shows it for owner and editor alike. Trim keep 1 pruned 3; the next send is accepted. |
| Pending-review purge | **FAIL, see B1.** |
| Erasure of an editor with versions and of an author | PASS. History `by`, conflict `head_by`, merge `head_by` and `between[].by`, `BankHead.author_name`, `committed_by`, list entry and activity all say "deleted teacher". Parsed, no null failure. Another editor and the owner keep working on the erased author's bank (send, rollback). |
| Export | PASS. `/me/export` has a `circles` section (`versions_sent`, `banks_authored`, `memberships`, `owned`, `invites_created`, `reports`, `transfer_offers`); `versions_sent` rows carry a string `message` (never null). The client treats the export as opaque parts. |
| Pictures | PASS. A picture bank is sent, copied (`circle_blob`), re-sent by an editor, restored and fetched by a third member. |
| Merge token misuse | PASS. A garbage token or another account's token gives 422 `merge_token_invalid` and the model's "merge again" refusal. A `merged_from` that disagrees with the token gives 400 `invalid_request` field `merged_from`, with the model's out-of-date text. |
| Viewer | PASS. Send and merge give 403 `needed:"editor"` with the "can only look" text; rollback and trim give "Only the circle's owner can do that."; history is allowed. |
| Add / remove / rollback merges | PASS. `NewInTheirs` (Add/Skip), `RemovedInTheirs` (Keep/Remove) and a merge across another editor's send and an owner rollback all plan and apply. |

### Response structs against the server

Every JSON field the worker sends for these routes (history line, history page, merge-initiate, trim, send/rollback answer, head and version reads, conflict body, circle detail with `held`) is read by a matching client field. Nulls are accepted for text (`null_text`, `null_num`). I found no field the client reads under a name the server does not send, and no enum value the server sends that the client cannot take. Two harmless gaps: the client ignores `merge_token_expires_at` and the merge answer's `pinned_versions`, and `BankHead` has no `head_version`, `message`, `rollback_of` or `merged_from` (not needed by any screen). Structs use `#[serde(default)]`, so a missing field quietly becomes 0/false/empty rather than failing; the worker always sends them, so this is a robustness note, not a defect.

### Error codes

I ran 80 (status, code) pairs through the real classifier and wording (`RefusalKind::classify` and `message`), covering every `PoolError(...)` code in `circles.ts`, `circlesend.ts`, `circleversions.ts`, `shared.ts`, `publish.ts`, `blobs.ts` and the gateway codes. Every code that the circle routes can return has plain wording **except `pending_review`** (B1). The remaining "RAW" hits in my run were codes I invented to probe (`not_withheld` is admin-only) and `busy` (a false match on the word).

## Blockers (new class)

### B1. `pending_review` shows a raw code (wording coverage gap)

- Route: `POST /circles/:cid/banks/:bid/purge`, the owner's "Delete for good" on a removed bank.
- Server: `server/worker/src/pool/circles.ts:1426` throws `pendingReview()` (`circles.ts:1667`): `409 {"error":"pending_review","message":"An administrator is reviewing a version of this bank for student data; it can be purged once that review is done."}`. This is when the bank has an open student-data flag and no report or admin hold.
- Client: `src/share/pool.rs` has no arm for it. `RefusalKind::classify` (around :389) and `other_message` (:539, `"member_unavailable"` at :541 is its neighbour) do not name it, and `problem_of` in `src/share/circles.rs:955` (`RefusalKind::Other => match ...`) only special-cases `conflict` and `pool_migrating`. The server's `message` is dropped for `Other`.
- Observed (live, owner client): `remove_circle_bank` OK, then `purge_circle_bank` gives status 409, code `pending_review`, message **"The pool refused the request (pending_review)."**
- Repro: a bank whose newest send matched a confirmed student-data block (held and withheld), owner recovers with a clean version, owner removes the bank, then purge. The flag stays open until an administrator resolves it.
- Severity: low-medium (a raw code in the UI, and it tells the owner nothing about why or what to do). Fix is one line in `other_message` or a `RefusalKind`: "An administrator is still reviewing this bank for student information. It can be deleted for good once that review is done. You can still leave it removed."

### B2. Phase B send shows "The problems are listed below." and lists nothing (`invalid_payload`)

- Route: `PUT /circles/:cid/banks/:bid` (send my changes). Any payload share-core refuses (a stem over 8192 characters, too many answers, a bad picture reference).
- Observed: a 60,000-character stem gives `422 invalid_payload` with `errors:[{code:"too_long",path:"bank.questions[0].stem",limit:8192}]`. The client parses the findings correctly (`PayloadRefusal` shows `Issue{code:"too_long", path:"bank.questions[0].stem", limit:Some(8192)}`), but the send page ends in `SendStage::Failed(Problem{message:"The pool can not take this as it is. The problems are listed below."})`.
- Where: `src/share/circle_edit.rs` `on_sent` error arm (about :1010-1075): `InvalidPayload`, `LabelsNeedDecision` and `OrderAckRequired` are not matched, so they fall to `problem_of` and the default `e.message()` (`src/share/pool.rs:~452`). The page draws that through `src/share/circle_edit_ui.rs:174` (`SendStage::Failed(p) => problem(ui, p)`), which shows only the message. The phase A first send does render the findings (`src/share/circle_send.rs:276`, `Refused::Payload`). The restore and merge-send paths share the same sentence.
- Impact: the person is told to read a list that does not exist and cannot tell which question to fix; `retry:false`, so only "Back". Severity: medium. Realistic triggers are an over-long stem or explanation, or an image problem, in a working copy that was edited on this device.

## Variants and limits

1. **A restore that comes back withheld or held says "Done".** `src/share/circle_edit.rs:1549` keeps only `RestoreStage::Done(sent.version)`, dropping `SentBank.held` and `.withheld`. `src/share/circle_edit_ui.rs:684` then says "Done. Version N is now the circle's newest, with the content of version M. Everyone with a working copy sees it as a new version." Observed: restore to v1, whose text now matches a confirmed block, gives `held:true, withheld:true` (the model returned `Done(4)`, and the worker had hidden v3/v4 and held the bank). The send page does word it (`sent_lines`, `circle_edit.rs:1776`); the restore page should too.
2. **The withheld/held advice is for the author, but an editor is the one who sees it.** After an editor's send gives `held:true, withheld:true`, `sent_lines` (`circle_edit.rs:1776-1781`) tells them to "take the student information out of the questions and send again" and that "a clean version sent later lifts this". Observed: the next call by that editor (history, merge, send) gives 404 for the held bank, shown as "That circle or bank is not available to you any more. It may have been closed, removed or hidden, or you may have left the circle." (`circle_edit.rs:712`). Only the bank's author or the owner can recover. The editor's link was also advanced to the withheld v2, so the merge base is a version the circle never showed. Suggested: for a non-author, say the bank's author or the owner has to fix it.
3. **Refused attempts spend the daily allowance.** `circlesend.ts` refunds only 409, 428 and 503 (`REFUNDED`). A 404 `version_not_retained` on rollback (a prune racing the dialog) costs one of the 20 daily `circle_rollbacks`; any 4xx other than those costs a `circle_commits` of 100. Not reachable by normal clicking, but a script or a retry loop can burn the cap.
4. **After the author changes the licence, every other editor's first send is refused once.** `finish_merge` (`circle_edit.rs`, link update) does not refresh `link.license` from the merge. The refusal is plain and self-healing, the second send is accepted. A merge could carry the licence.
5. **Pinned versions are labelled "Kept for a review".** `version_flags` (`circle_edit.rs:1798`) says so for anything in `pinned_versions`. The worker also pins ordinary kept versions that a withheld version cut off from the head's run: after the recovery in my run, v1 shows "Kept for a review" with no report or hold on it (`oldest_kept` 5, `pinned` [1]).
6. **History shows only the first 50 lines until "Older versions" is pressed**, so a pinned version beyond the first page is in `pinned` but not on screen. Behaviour is consistent with the worker.
7. **Merge shows the first 50 versions after the base**; `between_total` is shown as "and N more versions after those" (fine). With a pruned base and no snapshot the merge is two-way and every difference is a decision (the screen says so).
8. **Large banks lose the snapshot** (`merge::bounded` empties it): the base is then fetched from the circle, three-way if still kept, two-way otherwise. Verified both.

## Nitpicks

- A non-author editor who tries to restore or trim gets "Only the circle's owner can do that." The worker also lets the bank's author do it while an editor (`rollbackBank`, `trimVersions`); the text drops that.
- The byte-cap text ("Ask the owner (or the bank's author) to trim ...") is shown to the owner and the author themselves, without a pointer to the Trim button in History.
- A payload over the limit reads "4 MB of 4 MB allowed" (`LocalProblem::PayloadTooLarge`, rounding in `size_text`).
- Server `message` text is discarded for `Other` refusals even when it is already plain English (`pending_review`, `under_moderation` use it; `Other` does not).
- The history `by` for a version whose committer was erased is "deleted teacher"; the activity feed prints "deleted teacher left." Consistent, just worth knowing.

## What was not a problem (checked)

- The send/merge/rollback request shapes (`commit_id`, `base_version`, `merge_token`, `merged_from`, `confirm_large_change`, `to_version`, `keep`, `before`) match what the worker validates; the `commit_id` is kept across a retry and replays return the first answer.
- The merge token is bound to account, bank, base and head on the worker and the client carries it only for the head it was issued for; a stale one gets the plain "merge again" refusal.
- A held bank is invisible to non-authors in the list (`GET /circles/:cid`) and the author's list entry carries `held:true`; the client reads it.
- Erasure leaves versions in place with a neutral name and never breaks a parse.
- No path crashed the model, left it busy, or applied a merge without a restore point.
