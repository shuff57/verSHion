# Critic 51: circles phase B client, after the fixes for critic 49 / 50

Fresh reviewer. I read `circle_edit.rs`, `circle_links.rs` and the diffs of `circles.rs`, `circles_api.rs`, `pool.rs`, `merge.rs`, `browse.rs`. The lens was user data safety and whether each fix really holds. Treat this file as untrusted model output.

Baselines: `cargo test --lib share::` gives 494 passed, 0 failed. `cargo check --lib --target wasm32-unknown-unknown` compiles (scratch copy).

## How I checked

- Scratch copy of the repo: `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c51/repo`. No repo file was changed.
  - `src/share/circle_edit_tests.rs` there has two appended tests, `c51_t1_*` and `c51_t2_*`.
  - `src/share/c51_live.rs` and `src/share/c51_run.rs` there are a live driver. It runs the real `CircleEditModel` and `Client` against the local worker `http://localhost:18791` only.
  - The live run used my own throwaway accounts `c51own1` and `c51ea1` (`@example.test`), with in-process random passwords that were never printed or saved. Both accounts were erased by the run's cleanup (`delete_account` returned ok for each). Port 8799 was not touched. I never signed in as `a@example.test` or `b@example.test`.
  - Build output went to the repo's `target/`, as critics 49 and 50 did.
- Mutation check of the fixes. In the scratch copy I disabled the `pictures_missing` refusal and the `print_of` check in `finish_merge`. `b3_*` and `b4_a_picture_*` then fail, so those two tests do bite. The b1, b2 and b5 tests I verified by reading and by the live run below.

## Verdict

**Blockers (new class): 1**, and it is serious. The B5 fix (the "stale link" check from the `modified` timestamp) compares two clocks in different units. It blocks every honest working copy on its first edit, and for a copy that is level with the circle there is no way out in the UI.

The other fixes hold:
- B1, own-work box: held.
- B2, licence refresh: held, with one race and some fallbacks (V1, V2).
- B3, fingerprint at Apply: held.
- B4, lost pictures: held.
- B6, unreadable questions: held.

## Blockers (new class)

### N1. `bank.modified` is in SECONDS in the app, but the link stores MILLISECONDS: any real edit makes the copy "stale", and send is then blocked with no way out

Where:
- `src/share/circles.rs:2118`. `link.bank_modified = banks[...].modified` right after `apply_import(Dest::New)`, which sets `modified: imp.at_ms` (`src/share/browse.rs:672`), and `at_ms` is `now_ms()`, in milliseconds.
- `src/share/circle_edit.rs:1488`. `finish_merge` sets `banks[pos].modified = imp.at_ms.max(prev + 1)`, again in milliseconds.
- `src/share/circle_links.rs:104`. `stale_for` is `bank_modified > 0 && bank.modified < bank_modified`.
- Every editor path writes seconds: `screens::now_secs()` at `src/qedit.rs:448` and `:737`, `src/bankui.rs:1064` and `:1469`, `src/importui.rs:1882`, and others.

Scenario, run as test `c51_t1_real_edit_after_working_copy_is_stale` through the real "Make a working copy" flow:
1. After the copy, `bank.modified = 1792864237304` and `link.bank_modified = 1792864237304`.
2. The person edits a question. The editor writes `bank.modified = now_secs() = 1792864237`.
3. `stale_for` is true. Send is blocked: "This copy no longer matches where it was last in step with the circle (a saved copy of your banks may have been put back). Get the latest changes and merge first."

The existing tests miss this. `Rig` builds the link with `bank_modified = 0`, and `edit_stem` does `modified += 1` in milliseconds, so the units never differ.

Dead end, test `c51_t2_stale_and_up_to_date_is_a_dead_end`:
- If the copy is level with the circle, "Get latest and merge" answers `UpToDate` ("You already have the circle's newest version. There is nothing to get.") and the link is not touched. Send stays blocked.
- "Forget this link" is drawn only when the bank is missing (`circle_edit_ui.rs:979`). The only way out is to make a new working copy, which starts from the circle's head and does not carry the edits.
- If the circle has moved, the merge works but is two-way (`stale` skips the stored snapshot, `circle_edit.rs:1336`). Every shared question is a conflict.
  - `finish_merge` then writes milliseconds into `bank.modified` and `link.bank_modified` again (`:1488-1489`).
  - The next edit is back in seconds and the copy is stale again. This repeats on every cycle.

Effect: the whole "send my changes" feature does not work for a normal user. Nothing is lost, because the failure is a refusal. But the working copy cannot be sent or kept in step, and the person is told to do something that does nothing.

A side effect: `finish_merge` and `apply_import` put a millisecond value into `bank.modified`, a field the rest of the app reads as seconds. It shows as year ~58000 in any "edited" display or sort.

Severity: blocker. The safety guard added for critic 49 B5 now blocks honest users, and the recovery does not work (point 5 of the brief).

Suggested fix:
- Store and compare in the app's unit (seconds).
- Do not set `bank.modified` from `at_ms` in `finish_merge`. Use `now_secs().max(prev + 1)`.
- Make the stale test robust:
  - Keep a hash of the bank's questions (the `print_of` value) in the link at each sync.
  - Treat the copy as stale only if the bank's `modified` went back AND the hash differs from the stored hash.
- Give the stale state a recovery that always works. For example, "Treat this copy as in step with the circle's newest version" (set the base to the head) next to "Forget this link". `UpToDate` should clear the stale flag.
- Add a test with the real units: a copy made through `CirclesModel::sync`, then `modified = now_secs()`.

## Variants and limits

- **V1. The licence the author sends is the one checked when the page opened, not at Send.** `circle_edit.rs:941` (`on_licence`) and `:1094`.
  - Live run (`c51_run::live`, L-b): the author's send page had checked CC-BY. The licence was then tightened to `circle-only` out of band (another device or window of the same author). Pressing Send gave version 3 and the bank became `CC-BY-4.0` again, silently.
  - L-a works: with a stale link, the author's send used the pool's `circle-only`, so the B2 fix holds.
  - The window is the time the send page stays open. It needs a second window or device of the same author.
  - Fix: re-ask `/circles/heads` just before the PUT, or send only if the licence still matches.
- **V2. The licence falls back to the old link value, labelled as known.** `circle_edit.rs:961`. When the heads entry is not `Current` (`under_review`, `removed`, `unavailable`, or a licence string this build cannot parse), `theirs` is `None` and the stale `link.license` becomes `LicenceState::Known`.
  - Only the bank's author can change a licence (the server refuses the others), and an author sees `current` unless their own account is under review. So the practical reach is small.
  - But the code says "Known" where it should say "not checked".
- **V3. "Send my version on top" still has no typed or second confirmation.** `circle_edit.rs:1064`, `circle_edit_ui.rs:435-452`. It now lists the readable versions it replaces (`start.between`), and `start.head_version` is used (V3 of critic 49 is fixed). The path where `merge_start` itself answers `version_withheld` has `replaces` empty and takes the head number from `head_seen`, which may be a guess (`:1286`). I did not find a way to reach that path, because merge-initiate does not return it for a withheld head.
- **V4. Unknown fields on a readable question are still dropped on a send.** A reworded question sent from an older build loses any field a newer build added to it (serde ignores unknown fields). B6 (a whole question) is fixed; this part of it is not. Needs version skew.
- **V5. `share-circles.json` handling.**
  - `.bad` is written on every load that finds a bad entry, and a second incident overwrites the first `.bad` (`circle_links.rs:274`).
  - `read()` returns `None` for a read error as well as for "no file". After a transient read failure the model starts with no links, and the next `add_link` or `save` writes the file over, so every link and base snapshot is lost silently.
  - Two windows or tabs: the last writer wins on the whole list. The temp file name is fixed per document.
  - Banks are untouched in all of these. Only the "linked" state is lost.
- **V6. The fingerprint in `finish_merge` (B3 fix) covers `bank.questions` only** (`print_of`, `circle_edit.rs:1933`).
  - Order, text, pictures and settings of questions are covered. Bank title and details are not, and `apply_merge` does not change them, so it is fine.
  - The check and the write happen in the same `sync` call on the same `&mut banks`, so there is no TOCTOU gap.
  - Refuse-and-rebuild leaves the bank intact (test `b3_*` asserts the edit and 0 restore points). There is no `HashMap` in `Question`, so the hash is stable.
- **V7. Overflow in `banks[pos].modified = imp.at_ms.max(prev + 1)`** (`:1488`) panics in a debug build if a corrupt `banks.json` holds `i64::MAX`. Server data does not reach it.
- **V8. Other panic paths on server data.** I found no `unwrap`, `expect`, slice or index on server data in `circle_edit.rs`, `circle_links.rs` or `circle_versions_api.rs`. The one `unreachable!` (`:1738`) is guarded by the `matches!` just above it.
- **V9. `InvalidRequest { field }` text** (`circle_edit.rs:770`) puts the server's `field` into a sentence without `api::plain`. A nitpick, the same as V11 of critic 49.

## What I checked and found right

1. **Own-work attestation.**
   - Never pre-ticked: `SendView.attest` starts `false` in `open_send`, and a new page is a new view.
   - Never persisted: not in `CircleLink`, not in the link file.
   - Gated twice: `send_blocker` and again in `do_send` (`:1098`). `confirm_large_send` goes through `do_send`, so "Send anyway" with an unticked box stops there.
   - Live: with no tick, `send()` did nothing and no version was created (head stayed v1).
   - A 422 `attestation_required` unticks the box and shows plain words (test `b1_a_refused_attestation_*`).
2. **Licence refresh (`POST /circles/heads`).**
   - Failure or "Checking" blocks the send with plain text and a retry (test `b2_*`).
   - The send's licence is the pool's (live L-a). The PUT uploads that licence, the bank as it is now (`payload_of`, qids from the link) and the copy-time title and subject.
   - Title and subject can only be changed by a send's meta, since the only PATCH route sets the licence. So the copy-time meta cannot revert anyone's rename.
3. **Pictures.**
   - `finish_merge` refuses with "N pictures ... could not be fetched. Nothing was applied" before the restore point is written, and the link does not move (test `b4_a_picture_*`).
   - A working copy that lost pictures is made as a plain, unlinked copy. I found no path that sends a bank with lost pictures.
   - A blob that fails to store counts as missing as well (`copy_bank`).
4. **Unreadable questions.** `copy_bank` (so working copy and merge apply), `build_review` and the history page all stop or do not convert. Send takes only the local bank. Restore is done by the server, with no conversion on the client.
5. **Error text.** `pending_review`, `version_withheld`, `version_not_retained`, `merge_token_invalid`, the `circle_rollbacks` 429, the `circle_commits` 429, the 413 circle quota (with trim advice) and `invalid_payload` (now rendered as a list) all have plain wording. I saw no raw code.
6. **wasm.** `cargo check --lib --target wasm32-unknown-unknown` passes. Job spawning is split by cfg. The `.bad` write goes through the same backend. The restore-point text still says "copy that file over banks.json in the same folder" on the web build, where there is no folder (nitpick).
7. **Secrets.** The merge token is in `Carry` and `Review` in memory only. It is not in `share-circles.json`, a log or an argument. `Review` derives `Debug` and holds the token, so keep it out of any log line.

## Nitpicks

- `circle_edit_tests.rs` has no test with the app's real time units (see N1). Add one to the rig: `Rig::edit_stem` should write seconds, and the copy should come from `CirclesModel::sync`.
- The stale message mentions "a saved copy of your banks may have been put back"; with N1 fixed it will still help, but it names no action for the person. It needs one ("Treat this copy as up to date").
- Pinned versions are still labelled "Kept for a review" (critic 50 variant 5) and the 50-line history cap is still not shown on the history page. Unchanged and minor.
