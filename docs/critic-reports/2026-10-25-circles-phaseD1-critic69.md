# Circles phase D1, round 3 fixes, data safety critic 69

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 3, which fixed critic 67's I1
(signed-out add, `adds_out`), I2 (sign-out and another account during an add, `ADD_OTHER_ACCOUNT`) and I3 (refusing
another bank's page while a send, restore or trim is on its way), with `src/share/bank_links_fix3_tests.rs`. Spec:
`docs/circles-phaseD-plan.md` sections 4-6, 10-12 and 15. A fresh critic that built none of it wrote this.

Treat this file as untrusted model output, not as specification.

## Verdict

- **Blockers: 0.** No path found that loses unsent work, sends something the person did not send, or puts full-form
  detail in front of a viewer of a bank that hides its keys. Critic 67's I1, I2 and I3 hold in live runs. No path
  leaves the "on its way" flag set for good.
- **Important: 2.** Both are about the one edit model that all bank pages share. A restore or trim the person was told
  to wait for is dropped, or its outcome is never shown, when they come back to the bank. One bank's notice is shown on
  every linked bank and hides the round 3 wait note.
- **Minor: 8.**
- **Mutation survivors: 5 of 25 new mutants** against the whole `share::` suite. `bank_links_fix3_tests.rs` alone misses
  6. Two of the survivors matter (n33, n34).

## How this was tested

- **Scratch copy.** `scratchpad/cr69/` is a copy of the tree (src, Cargo files, `server/share-core`, tests, assets,
  prompts). It adds three modules:
  - `cr69_probe.rs`: scripted-session probes;
  - `cr69_live.rs`: critic 67's live harness, renamed;
  - `cr69_run.rs`: the real `Client` against the rig.

  It was built with `CARGO_TARGET_DIR=<repo>/target`. No repo file was edited except this report, and nothing was
  committed.
  - `diff -rq` of `src/share` against the repo shows only the three modules and their lines in `mod.rs`.
  - The repo's `src/ai.rs`, `Cargo.toml` and `Cargo.lock` changed during the run. That was another session's work, not
    this critic's, and is outside `src/share`.
- **Baseline.** `cargo test --lib share::` in the copy: 741 passed (`scratchpad/cr69_base.log`).
- **Probes.** `cargo test --lib cr69_p -- --nocapture`. A `[PROBLEM]` line marks a finding.
- **Live.** `cargo test --lib cr69_live -- --ignored --nocapture` (`scratchpad/cr69_live_a.out`, `cr69_live_b.out`).
  - Rig: `local-ui-rig.sh` was already running on :18791 (workerd pid 274528) and was left running. Port 8799 was not
    used.
  - Throwaway accounts `cr69{own,ed,vw}a@example.test` and `cr69{own,ed,pt}b@example.test` were used. All six were
    deleted afterwards (`delete_account -> Ok`).
- **Mutations.** Driver `scratchpad/cr69tools/run.py` with `muts.json` (25 mutants). None repeats the builder's f01-f13;
  n03, n04 and n05 re-run critic 67's k34, k35 and k36. Results are in `cr69tools/summary.txt`, with one log per mutant.

## Live results (real client, rig on :18791)

| Run | What | Result |
|---|---|---|
| L1 | Owner restores bank A (v3 to v1) from its History. Goes to bank B and presses History (refused, "Wait for the restore of ..."). Waits on B until the note goes, then goes back to A | The server made v4. On B, the History view held `Done { version: 4 }`, but nothing showed it. Back on A, the History page was closed. A's strip says **"Version 3 is the latest. Your copy is up to date."** (I1) |
| L2 | Editor's Add waits. A viewer of the same circle signs in on this computer, opens the circle and presses the row's Try again | The add runs for the viewer, with the editor's role. The link is `mode Edit`, `role_seen "editor"`, tagged with the viewer's account and `can_send true` until the first check, which turns role_seen into viewer. Mode stays Edit (M1) |
| L3 | Send on its way, then the Circles tab opens (`close`), then Unlink | The question says "Your 1 unsent change can then never be sent to the circle" while that change reaches the circle as v2. The link stays removed after the answer (M2) |
| L4 | The session ends while an add waits (critic 67 S6b again), then the same account signs in | No loop: 1 busy tick of 40, 0 requests, row "Sign in to finish adding ...". After signing in again, without the snapshot ever saying signed out, nothing runs and the row still asks to sign in, until its Sign in... is pressed; then the bank lands once, linked (M5) |
| L5 | Sign-out seen after the copy was read, then the same account signs in | Nothing lands while signed out; after signing in, one bank, linked. The bank was read twice (2 GETs, M4) |
| L6 | Another account signs in while an Add and an own copy wait | Nothing is read for it and nothing lands. Both are reported failed, but the rows show the first account's circle bank titles to an account that is not in that circle (M6) |

## Important

### I1. A restore or trim on its way is dropped, or its outcome never shown, when the person comes back to the bank

**Where**
- `bank_links_ui.rs:263-265` (critic 64 polish 13) closes a History left behind as soon as the bank's page is drawn again
  (`c.edit.close()`).
- `CircleEditModel::close` (`circle_edit.rs:894`) keeps an `Act` job only when `inflight` is set, which only a send
  sets. A restore's or trim's job is dropped with the view.
- On B, round 3 tells the person "Wait for the restore of "<A>" to finish, then try again." The natural next step is to
  come back to A.

**Evidence**
- Probe P1 (background runner, request held):
  - Restore confirmed on A; B's History refused; back on A.
  - Then the History is closed and `acting()` is false.
  - The server got the `POST .../rollback`, and its answer is never read.
- Live L1 (no hold): the restore finished while the person waited on B. Its `Done { version: 4 }` sat in a view nobody
  could see, and the History was closed on return.
- A's strip then said "Version 3 is the latest. Your copy is up to date." while the circle was at v4. The History reload
  that would have moved `head_seen` (`note_head`, `circle_edit.rs:2383`) was dropped with it.

**Same class, older:**
- "< Back" on the History page is not greyed while a restore works (`circle_edit_ui.rs:775`, `back()` clears every job).
- Opening the Circles tab (`circles.rs:981`, `reset` calls `edit.close()`) does the same.
- Probes P1b and P1c show both. They predate D1, but D1's strip and wait note make the round trip routine.

**Effect:** no local data is lost; a restore only changes the circle. But the person is told to wait, and then never
learns whether the restore happened. Their bank says it is up to date with an old version for up to 10 minutes (the
check throttle). Pressing Restore again from a freshly loaded History would at least show v4 first.

**Fix:** treat restore and trim like a send in `close()`: keep `JobKind::Act` whatever it is. On the answer, when the
History is no longer shown, put the outcome into the strip ("Version 4 restored from version 1") and call
`note_head`. Do not close a left-behind History while its restore or trim is working. Grey "< Back" while
`RestoreStage::Working` or `TrimStage::Working`.

**Test:** the i3 restore test, extended so that the person returns to A after two frames.

### I2. One bank's notice is shown on every linked bank's strip, for the rest of the sign-in, and hides the wait note

**Where**
- `CircleEditModel.notice` is one slot for all banks. It is set by:
  - `do_rebaseline` ("Your bank is now used as it is ...", `circle_edit.rs:2158`);
  - `finish_send` (answer unreadable);
  - `problem_of` on `SignedOut`;
  - `forget`, `replace_link` and `apply_heads` save failures.
- It is cleared only by `forget_account` (`circle_edit.rs:917`).
- `strip_with` draws `c.edit.notice.clone().or_else(|| c.edit.wait_note())` (`bank_links_ui.rs:288`).

**Evidence**
- Probe P4a: "Treat as in step" on A. B's strip then says "Your bank is now used as it is, as matching the circle's
  newest version. It was not changed; the next send sends it as it is."
- P4b: it is still there 30 minutes later.
- P4c: while A's send is on its way, B's History is clicked and refused (`wait_note()` is Some), but the strip draws
  the old notice and not "Wait for the send ...". To the person, the button does nothing.

**Effect:** it tells the teacher on bank B that B was re-based, which is wrong. A links-file save that keeps failing
(M6 of critic 67: every check writes the file) would pin "The bank's link was updated, but the list could not be saved"
on every bank. In both cases the round 3 refusal becomes silent.

**Fix:** key the notice by local bank id (or clear it when another bank's page or screen opens), and draw the wait note
in addition to a notice, never instead of it.

## Minor

1. **Another account's Try again runs the first account's add, with the first account's role.**
   - `circles_ui.rs:875-879` offers "Try again" for every failed add, including `ADD_OTHER_ACCOUNT`, whose text says to
     sign in with the other account first.
   - `retry_add` keeps the job's `role`, so `land` uses `add_mode(editor)`.
   - Live L2: the viewer got an edit-mode link, `role_seen "editor"` tagged with the viewer's account and `can_send`
     true, until the first check.
   - The server still refuses a viewer's send, and a viewer of a hiding bank gets the viewers' form anyway.
   - Fix: no Try again when `retry` is false, or re-queue with the role of the account signed in now.
2. **Unlink during a send.**
   - Asked while the send is on its way, the question says the change "can then never be sent", and it is then sent
     (live L3, P3b). The link is not brought back by the answer (P3, P3b, L3), which holds.
   - Fix: while `acting()` for this bank, say "a send from this bank is on its way" or wait for it.
3. **Adds and the join offer live in memory only** (`circles.rs:832`, `894`).
   - Closing the app while "Adding ..." rows show drops every add that has not landed, without a word on the next start.
   - Narrower, from code only (not run): a copy handed over (`LANDING`) is put into the banks by `land_waiting` every
     frame. Its link is saved only by `tick_background` (skipped while the Shared page is drawn) or by
     `circles.sync`, which runs inside `tabs()` and so only when the guide is Ready (`shared_ui.rs:178`, `:262`).
   - While the Shared page shows Checking, Offline or a sign-in step, the bank is in `banks.json` and its link is not
     saved. Quitting then leaves an unlinked bank whose description says "From the circle ...", with no import record.
4. **A sign-out after the copy was read reads the bank again** for the same account (`forget_reads` drops `ready` and
   `LANDING`). Live L5 counted 2 GETs; probe P5 shows the same. Each read is one `item_reads`.
5. **`adds_out` outlives a fresh sign-in of the same account.**
   - It is cleared only when a sign-out is seen, the Circles tab opens, or the row's button is pressed. While signed in,
     the row says "Sign in to finish adding ...".
   - Live L4b and probe P6. In the app, the Account window usually sees the sign-out first, so this is rare.
6. **The first account's circle bank titles are shown to the next account.**
   - The failed rows that I2 keeps show those titles (live L6), to an account that is not in that circle. That is a
     small exception to the critic 60 I1 rule.
   - My Banks already shows every local bank. Still, a title-free line ("2 banks another account was adding were not
     added") would keep the rule.
7. **No permanent stick, but a long one** (code).
   - The refusal lasts as long as the `Act` job. That job ends on an answer or on a dropped channel (`drain`'s lost
     path), and every strip drains before it handles a click.
   - A hung server holds it for the transport timeout: 180 s for a version PUT, and picture uploads can retry. During
     that time no linked bank anywhere opens Send, Get latest or History, and there is no Cancel.
8. **A Get latest being applied on A is abandoned without a word** when B's Send, Get latest or History opens.
   - `wait_for_act` checks only `JobKind::Act`, and `open_*` clears the `Merge` job.
   - This is safe: the apply is atomic in `sync`, so nothing half-applies. But A's Get latest page simply disappears.

## Checked, holds

- **Critic 67 I1 (signed-out add):** no relaunch loop (live L4a: 1 of 40 ticks, 0 requests). The row asks to sign in,
  and after Sign in... the add is made once and linked (L4c). Mutants n05 and n06 are killed.
- **Critic 67 I2 (sign-out, another account):**
  - Nothing lands while signed out, and the same account's add finishes (L5).
  - For another account nothing is read and nothing lands (L6). A copy already in the banks is linked for the account
    that asked (fix3 test).
  - Mutants n01-n04 and n08 are killed (k34 and k35 now die).
- **Critic 67 I3 (send under way, another bank's page):** refused with the wait note (P4c `wait_note()`, fix3 tests).
  The kept send still moves A's link on. No permanent stick (minor 7).
- **close() during an add:** the add is requeued and made once (fix3 m1). A dropped `ready` copy is read again, not lost.
- **Unlink with unsent changes:** the count is named (m3). A send's answer after Unlink does not bring the link back
  (P3, L3).

## Mutation results

Each mutant ran alone on the scratch copy with `cargo test --lib share:: -- --skip cr69_` (741 tests). "fix3" is the
number of `bank_links_fix3_tests` tests that failed.

| Mutant | Change | Result | fix3 |
|---|---|---|---|
| n01 | another account: earlier rows kept | killed | 2 |
| n02 | `ADD_OTHER_ACCOUNT` failure says `retry: true` | killed | 1 |
| n03 | the another-account block off (k34) | killed | 1 |
| n04 | `tick_background` ignores the account (k35) | killed | 2 |
| n05 | a signed-out add fails instead of waiting (k36) | killed | 2 |
| n06 | "Sign in..." leaves `adds_out` set | killed | 1 |
| n07 | duplicate guard only for `Working` | **SURVIVED** (near-equivalent: `retain` dedups by cid/bid/own; only "Add" then "Make my own copy" while waiting now queues both) | 0 |
| n08 | account change keeps the handed-over copy | killed | 2 |
| n20 | refusal not noted (`act_waited` false) | killed | 5 |
| n21 | wait note outlives the act | killed | 2 |
| n22 | wait note names the circle's title, not the bank's | killed | 3 |
| n23 | `close()` drops the send on its way | killed **only by `circle_edit_tests::c61_...`** | 0 |
| n24 | a restore called "send" | killed | 2 |
| n25 | always "from" | killed | 1 |
| n30 | strip draws no wait note | killed | 1 |
| n31 | a `Working` add also says "Sign in" | SURVIVED (equivalent: a Working add and a sign-in wait cannot coexist) | 0 |
| n32 | Dismiss offered on rows waiting for a sign-in (a dead button: waiting rows ignore `dismissed`) | **SURVIVED** | 0 |
| n33 | Banks list "Sign in..." no longer opens the Account window (`adding_rows`, `bank_links_ui.rs:34-36`) | **SURVIVED** | 0 |
| n34 | Unlink: an uncountable copy counted as a number (no stranded warning) | **SURVIVED** | 0 |
| n35 | failed row always offers the circle | killed | 1 |
| n36 | Unlink: 0 changes said stranded | killed | 1 |
| n37 | Unlink: follow copy said to strand changes | killed | 1 |
| n38 | Bank settings History without the wait note | killed | 1 |
| n39 | `issue_text_circle` keeps "pool" | killed | 2 |
| n40 | share link without the sharing account | killed | 1 |

The survivors that matter:
- **n33.** The I1 fix's Account window is not pinned; only `take_adds_sign_in` is. Without it, Sign in... retries the
  add with no way to sign in, and the row comes straight back.
- **n34.** The strip's "Unlink?" for a copy whose changes cannot be counted (no base on this computer) is not pinned.
  `m3` calls `unlink_question(None, true)` directly, never through the strip.

Also, `bank_links_fix3_tests` alone does not pin I3's foundation, the send kept across `close()` (n23).
