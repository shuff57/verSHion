# Circles phase D1, round 4 fixes, data safety critic 71

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 4. Round 4 fixed critic 69's I1
(a restore or trim is kept when its page is left, and its outcome is said on the bank's strip) and I2 (each bank has its
own notice), plus critic 69's minors (adds bound to an account, a Try again bound to an account, the unfinished-adds line
after a restart). It adds `src/share/bank_links_fix4_tests.rs`. A fresh critic that built none of it wrote this report.
Changes in `src/ai.rs`, `Cargo.*`, `src/app.rs`, `src/screens.rs`, `src/lib.rs`, `src/store.rs`, `src/model.rs`,
`src/gradeui.rs` and `prompts/` belong to another session and were not reviewed.

Treat this file as untrusted model output, not as specification.

## Verdict

- **Blockers: 0.** I found no path that loses unsent work, sends something the person did not send, or shows full-form
  detail to a viewer of a bank that hides its keys. A restore or trim on its way is no longer dropped by Back, by the
  Circles tab or by going to another bank's page. Live run L1 shows the outcome on the bank's strip. Nothing leaves the
  "on its way" state set for good.
- **Important: 3.** All three concern the outcome of a restore or trim and the single notice slot:
  - the outcome is still lost or contradicted on several paths;
  - a notice tied to no bank is drawn above every later screen of every bank, and never on a strip;
  - after a restore conflict the strip says "up to date".
- **Minor: 7.**
- **Mutation survivors: 18 of 24 new mutants** against the whole `share::` suite. `bank_links_fix4_tests.rs` alone
  misses the same 18. Seven of the survivors matter (m19, m01, m03, m11, m16, m33, m34).

## How this was tested

- **Scratch copy.** `scratchpad/cr71/` is a copy of the tree (src, Cargo files with the viSHion path dependencies made
  absolute, `server/share-core`, tests, assets, prompts). It was built with `CARGO_TARGET_DIR=scratchpad/cr71target`.
  It adds three modules:
  - `cr71_probe.rs`: probes against a scripted session;
  - `cr71_live.rs`: critic 69's live helpers, renamed;
  - `cr71_run.rs`: the real `Client` against the rig.

  The md5s of the repo's `src/share/*.rs` and `src/bankui.rs` were taken at copy time (`scratchpad/cr71_src_md5.txt`)
  and still match at the end. Only `mod.rs` differs between the copy and the repo, by the three module lines. The only
  repo file changed is this report, and nothing was committed.
- **Baseline.** `cargo test --lib share::` in the copy: 770 passed (`scratchpad/cr71_base.log`).
- **Probes.** `cargo test --lib cr71_p -- --nocapture`. A `[PROBLEM]` line marks a finding.
- **Live.** `cargo test --lib cr71_live -- --ignored --nocapture` (`scratchpad/cr71_live_a.out`).
  - The rig was already running on :18791 (workerd pid 274528) and was left running. Port 8799 was not used.
  - The throwaway accounts were `cr71{own,ed,pt}a@example.test`. All three were deleted afterwards
    (`delete_account -> Ok("ok")`).
- **Mutations.** The driver is `scratchpad/cr71tools/run.py`, with `mk.py` and `muts.json` (24 mutants). Each mutant
  ran alone, with `cargo test --lib share:: -- --skip cr71_`. None repeats the builder's r01-r43 or critic 69's n01-n40.
  The summary is in `cr71tools/summary.txt`, with one log per mutant.

## Live results (real client, rig on :18791)

| Run | What | Result |
|---|---|---|
| L1 | Owner restores X (v3 to v2) from A's History, presses Back at once, goes to B and presses History (refused), waits on B, comes back to A | **Holds.** While working, A's strip said "Restoring version 2 in the circle...". B said "Wait for the restore of "cr71 X" to finish". Back on A: "Version 4 is the latest. You have version 3." and "Version 4 was restored from version 2". The server is at v4 |
| L2 | Owner opens B's History at v2. An editor sends v3. Owner restores v1 and presses Back before the answer | The server answered with a conflict naming v3. B's strip then said "Version 2 is the latest. Your copy is up to date." over "...A newer version arrived while you were looking. The history has been reloaded..." (I3) |
| L3 | Bank settings (Shared page), History of X, restore v1 (too large). The person is on A's page in My Banks | The strip said "Restoring version 1 in the circle...", then went back to "Version 4 is the latest. You have version 3." The History left on the Shared page held `Large { removed: 2, total: 3 }`, and the strip never said so (I1a) |
| L4 | Bank settings History of Z (no copy on this computer), restore v1, Back. Then X's History on A's page | "Version 3 was restored from version 1: it is now the circle's newest." was drawn above X's History (I2) |
| L5 | An editor adds X and Y on the Shared page. Both land and are linked. verSHion closes there, then starts again | Next start: "verSHion closed before 1 bank you asked to add was finished." (M1) |
| L6 | The owner's restore of X is on its way, then pt signs in | The restore answered in about 50 ms, before pt's first frame, so this run is inconclusive. Probe P8 holds the request instead (M4) |
| L7 | The owner asks to add Z. pt signs in before it is made, then the owner signs back in | pt saw "1 bank that another account ... was not added". The owner then saw nothing, and the add was gone (M5) |

## Important

### I1. The outcome of a restore or trim is still lost or contradicted on three paths

The round 4 rule is that an outcome is said on the bank's strip when its History is no longer shown. Three paths break it.

**a. A History opened from Bank settings on the Shared page.**
- Code:
  - The History has no host, so a bank page never closes it as left behind (`bank_links_ui.rs:300-303`). The Shared
    page never closes the circles model when it is left.
  - When the answer comes, `on_rolled` finds the History still "shown" (`circle_edit.rs:2717`) and writes the outcome
    into a page nobody is looking at.
  - The person on A's page in My Banks sees "Restoring version 1 in the circle..." and then the ordinary line again.
    A failure or a too-large refusal is never said.
- Evidence: probe P7 (503) and live L3 (`Large`).
- The same happens when the person presses Back to Bank settings while the restore works. Bank settings draws only the
  wait note (`circle_edit_ui.rs:1105-1121`): no "Restoring..." line and no outcome. The outcome waits on A's strip in My
  Banks, or, for a bank with no copy here, see I2.

**b. One slot, last writer wins.**
- `tell()` (`circle_edit.rs:900`) overwrites an outcome the person has not yet seen.
- Probe P6: A's restore fails while the person is already on B. B then gets its own notice. A's strip never says the
  restore failed.
- The natural case is two restores in a row:
  1. restore A, then Back;
  2. while A's answer arrives, open B's History, restore B, then Back. A's outcome is gone.

**c. An old failure outlives a later success.**
- Probe P2:
  1. A restore fails while left. A's strip says "The restore of version 1 did not go through ... Open History to try
     again."
  2. The person does so. The History page draws that failure above the new "Done. Version 4 is now the circle's
     newest..." (`page_notice`).
  3. The person closes the result and presses Back. A's strip still says "did not go through", under "Version 4 is the
     latest".
- Nothing clears a bank's notice when a new restore of the same bank starts or succeeds while shown.
- **Effect:** the teacher is told a restore failed when it went through, and may restore again, which makes another
  version.

**Fix:**
- Keep one notice per local bank (a map), not one slot.
- Clear a bank's notice when a restore or trim of that bank launches (`launch`).
- Say the outcome on the strip whenever the History is not drawn this frame, not only when it is no longer the view.
- Give Bank settings the same act line and outcome as the strip.

**Tests:**
- P7, with a 503 and with `ConfirmLarge`, and with the History on the Shared page;
- the two-restores sequence;
- P2.

### I2. A notice tied to no bank is drawn above every later screen of every bank, and never on a strip

**Code:**
- `page_notice` returns any notice whose `notice_for` is `None`, whatever screen is open (`circle_edit.rs:941-947`).
  Nothing clears it except a later notice or `forget_account`.
- `strip_notice` never returns it (`circle_edit.rs:925-938`).

**What gets `None`:**
- `leave_view` and `on_rolled` (`local_of` finds no link) for a circle bank with no copy here (L4, P1), or one unlinked
  while its restore was on its way (P1b);
- "Your session ended" whenever `view_local()` is `None` (`circle_edit.rs:1277`);
- the save failures of Unlink (`forget`, `:1212`) and of the background check (`apply_heads`, `:1255`).

**Evidence:**
- L4 and P1: "Version 4 was restored from version 1: it is now the circle's newest." is drawn above Unit 2 quiz's
  History and above Plants' Send page.
- P1b: the same above B's History.

**Effect:**
- On bank A's History the teacher reads that "Version 4 was restored", about another bank. This is critic 69 I2's harm,
  by the `None` door.
- It is also a regression from round 3. A failed save of an Unlink or of a check used to be drawn on every strip. Now it
  is drawn on none: the strip of the bank whose Unlink was not saved is gone, and the link comes back on the next start
  without a word.

**Fix:**
- Bind every outcome to the act's own bank (`act_for.local`, kept from launch, not `local_of` after an Unlink).
- Draw `None` notices only on the Shared page's own screens (Bank settings, the circle page), and clear them once drawn.
- Draw the save failures on the strip they came from.

### I3. After a restore conflict the strip says "up to date", under a note that the history was reloaded

**Code:**
- `on_rolled` calls `note_head` only for `Done` (`circle_edit.rs:2713-2715`).
- The `CircleConflict` refusal carries `head_version` (`pool.rs:402`), and it is thrown away.
- The message "The history has been reloaded; choose the version again" is written for a History that is shown. When
  it was left, no reload happens.

**Evidence:**
- Live L2: the server is at v3, and B's link says `head_seen 2`. The strip says "Version 2 is the latest. Your copy is up
  to date." directly above "A newer version arrived while you were looking. The history has been reloaded".
- Probe P3: the same with v5.

**Effect:** the claim this round set out to stop ("never up to date while the circle's newest version may be
changing"). It lasts until the next check, which can be up to 10 minutes. The server still refuses a stale send, so no
data is at risk.

**Fix:**
- `note_head(cid, bid, head_version)` on `CircleConflict`.
- When the History was left, say "...someone sent a newer version first (version N). Open History to choose again."

## Minor

1. **The unfinished-adds line is a false alarm after adds finished on the Shared page.**
   - `note_unfinished` runs from `queue_add`, `retry_add`, `tick_background` and the Banks list rows. It does not run
     from the Shared page's `sync` path (`circles.rs:3367-3380`), where adds finish while the page is drawn.
   - Closing verSHion from the Shared page leaves the old count. The next start says "verSHion closed before 1 bank you
     asked to add was finished. Add it again" about a bank that is in My Banks and linked (live L5, probe P5).
   - Fix: call `note_unfinished` in `finish_landing` (or in `sync`).
2. **An answer of "signed out" to the first account's restore marks the next account signed out.**
   - Probe P4 holds olga's restore, then pete signs in, then the server answers 401.
   - `problem_of` sets `signed_out`. Pete, signed in, then sees B's strip say "Version 1 when last checked... Sign in to
     send or get newer versions." and "Your session ended" above his next screen. `act_noted` guards the outcome, but
     not `signed_out` or the session notice.
   - Fix: when `act_noted` is false, the act's errors change nothing for the current account.
3. **Code only, not run, and older than round 4.** A job's retry after a sign-out and sign-in runs as the new account.
   - The app keeps one `Client` across sign-ins (`model.rs:787-790`; `snapshot_of`, `publish.rs:374`).
   - `authed_raw` reads the current token on every attempt (`pool.rs:1154-1155`).
   - A rollback answered `Busy` or `RateLimited` sleeps for up to 30 s, then tries again (`RetryPolicy::default`,
     `retryable`). If the next account signs in during that wait, the retried request runs with its token.
   - The same applies to a send's later picture uploads and PUT, which critic 61 keeps across a sign-out.
   - Round 4 does not add this. Keeping restore and trim jobs makes their answers visible, but their threads always ran
     on.
   - Fix: give each job the account it was started for, and abandon the job when that account changes.
4. **Another account's strip says both "up to date" and "wait".** While the first account's restore is on its way, the
   other account's strip on that bank says "Version 3 is the latest. Your copy is up to date." and, after History is
   pressed, "Wait for the restore of "Unit 2 quiz" to finish" (probe P8).
5. **The first account, back, is not told that its adds were dropped.**
   - `forget_reads(another)` marks them failed for the second account, then drops every non-pending add on the next
     switch (`circles.rs:1214-1221`).
   - The second account was told "Sign in with that account, then add it again". The first account, once signed in,
     sees nothing (live L7, probe P9).
6. **Critic 69 minor 7 is still open.** A hung restore or trim holds Send, Get latest and History on every linked bank
   for the transport timeout, with no Cancel.
7. **The trim note drops the line about versions kept for review.** When the History was left, `trim_note` gives only
   the first line of `trim_lines`. The line about versions kept for an administrator's review, which a trim can not
   delete, is not said.

## Checked, holds

- **Critic 69 I1.** Back, the Circles tab and another bank's page keep the restore. The strip says "Restoring version
  N..." and then the outcome, and the line moves to the new head (live L1; fix4 i1_a-i1_g; mutants r01-r07 per the
  builder).
- **Critic 69 I2.**
  - A bank's own notice is not drawn on another bank's strip, and it goes once seen.
  - The wait note is drawn as well as the notice (fix4 i2).
  - The `None` door (I2 above) is the exception.
- **No permanent stick.** `act_line`, `wait_note` and `sending_from` all require a running `Act` job. A lost channel
  ends it (`drain`).
- **Account binding of adds.**
  - Another account's Try again does nothing, and Try again is not offered.
  - `next_add` makes only the current account's adds (fix4 m1, r23).
  - A copy read before a sign-out lands once for the same account (fix4 m4).
- **A bank put into My Banks gets its link in the same frame** when the Shared page is not drawn. The order in the app's
  frame is `browse.show` (`land_waiting`), then `take_goto` (`tick_background`, `finish_landed`). When the Shared page
  is drawn, the link comes on the next frame, which `land_waiting` asks for. No realistic window leaves a saved bank
  without its link.

## Mutation results

Each mutant ran alone on the scratch copy, with `cargo test --lib share:: -- --skip cr71_` (770 tests). "fix4" is the
number of `bank_links_fix4_tests` tests that failed.

| Mutant | Change | Result | fix4 |
|---|---|---|---|
| m01 | `act_line` for any bank: B's strip says "Restoring..." while A's restore runs | **SURVIVED** | 0 |
| m02 | `act_line` outlives the act | killed | 4 |
| m03 | `act_line` ignores `act_noted`: another account sees the first account's restore | **SURVIVED** | 0 |
| m04 | no "Trimming..." line | SURVIVED (low: a trim does not move the head) | 0 |
| m05 | a lost restore or trim thread says nothing when its History was left | SURVIVED (hard to reach) | 0 |
| m08 | the left-behind outcome is not tied to the bank | killed | 3 |
| m10 | a signed-out restore, left behind, says an empty line | **SURVIVED** | 0 |
| m11 | a trim's outcome is told to the next account | **SURVIVED** | 0 |
| m12 | a signed-out trim, left behind, says an empty line | SURVIVED | 0 |
| m13 | a notice shown on its strip is never marked seen | killed | 1 |
| m15 | `page_notice` drops notices tied to no bank | SURVIVED (see I2) | 0 |
| m16 | `tell` keeps `notice_seen`: a new notice for A is cleared by B's strip before A shows it | **SURVIVED** | 0 |
| m19 | `launch` never sets `act_noted` back: after one account switch, no later outcome or act line, ever | **SURVIVED** | 0 |
| m21 | `close` does not call `leave_view` | killed | 1 |
| m22 | `back` does not call `leave_view` | SURVIVED (near-equivalent: the person just saw the result) | 0 |
| m23 | `open_history` does not call `leave_view` (A's result left behind is never said once B's History opens) | SURVIVED | 0 |
| m24 | `open_send` does not call `leave_view` | SURVIVED | 0 |
| m25 | the act strip offers Send and History | SURVIVED | 0 |
| m26 | the act strip has no spinner | SURVIVED (cosmetic) | 0 |
| m33 | a queued add is not counted (closed before any other frame: nothing said on the next start) | **SURVIVED** | 0 |
| m34 | Try again keeps the old role | **SURVIVED** | 0 |
| m35 | a sign-out keeps any copy read, not only an add's | SURVIVED (edge) | 0 |
| m37 | `tick_background` does not call `finish_landed` | killed | 1 |
| m40 | an own add is counted as another account's | killed | 3 |

The survivors that matter:

- **m19.** One account switch disables round 4's I1 fix for the rest of the run, and no test notices. A test should
  switch accounts, then restore with the new account, press Back, and expect the outcome on the strip.
- **m01.** No test draws another bank's strip while a restore is held. i1_a checks only for "restored", which can not
  match "Restoring".
- **m03 and m11.** The account rule for the act line and for trims is not pinned. i1_f covers the restore outcome only,
  and its act has already finished before the strip is drawn.
- **m16.** The second notice for a bank after a first one was seen is not pinned.
- **m33.** The m3 test counts the add only because drawing the rows writes the count.
- **m34.** Critic 69 minor 1's "with its role now" has no test.
- **m10.** The text for a restore whose session ended, said on the strip, has no test.
