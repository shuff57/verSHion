# Circles phase D1, data safety re-check, critic 67

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6), after the fixes for critic 63 and critic 65.
Files: `src/share/bank_links*.rs` (with `bank_links_fix_tests.rs` and `bank_links_fix2_tests.rs`), and the D1 edits in
`circle_edit.rs`, `circle_links.rs`, `circles.rs`, `circles_ui.rs`, `browse*.rs`, `shared_ui.rs`, `circle_send.rs`. Spec:
`docs/circles-phaseD-plan.md` sections 4-6, 10-12 and 15. A fresh critic that built none of it wrote this.

Treat this file as untrusted model output, not as specification.

## Verdict

- **Blockers: 0.** No path found that loses unsent work, replaces the link of a copy that has unsent work, or puts the
  viewers' form into a copy that has its keys. Every critic 63 and critic 65 data-safety fix held in live runs.
- **Important: 4.** Three are about background adds and the one edit model that all bank pages share; the fourth is the
  test gaps. None loses a question, but each leaves the person with a wrong idea of what happened.
- **Minor: 6.**
- **Mutation survivors: 17 of 32 new mutants.** The tests in `bank_links_fix2_tests.rs` kill only 9 of the 32 by
  themselves; the whole `share::` suite kills 15. The survivors that matter are listed under I4.

## How this was tested

- **Scratch copy.** `scratchpad/cr67/` is a copy of the live tree (src, Cargo files, `server/share-core`, `tests/`). It adds
  three modules: `cr67_probe.rs` and `cr67_probe2.rs` (scripted-session probes) and `cr67_live.rs` + `cr67_run.rs` (the
  real `Client` against the rig). It was built with `CARGO_TARGET_DIR=<repo>/target`. No repo file was edited except this
  report. Nothing was committed.
- **Baseline.** `cargo test --lib share::` in the copy: 710 passed (`scratchpad/cr67_base.log`).
- **Probes.** `cargo test --lib cr67_ -- --nocapture` (`scratchpad/cr67_probe.log`, `cr67_probe2.log`). A `PROBLEM` line marks a
  finding.
- **Live.** `cargo test --lib cr67_live -- --ignored --nocapture` (`scratchpad/cr67_live_a.out`).
  - Rig: `server/worker/scripts/local-ui-rig.sh`, already running on :18791 (pid 273219), left running. Port 8799 was not used.
  - Throwaway accounts `cr67{own,ed,vw,xx}a@example.test`, all deleted afterwards (`delete_account -> Ok`).
- **Mutations.** Driver `scratchpad/cr67tools/run.py` with `muts.json` (32 new mutants, none from the builder's 47 or from
  critic 65's list). Results in `cr67tools/summary.txt`, one log per mutant.

## Live results (real client, rig on :18791)

| Run | What | Result |
|---|---|---|
| S1 | Links file unreadable at start (a directory in its place). Add, join again, share a linked bank into the circle, then the file comes back and Try again | Add refused, join offer adds nothing and says why, share outcome `NotSaved`. After Try again the copy keeps its link to X, its unsent change is still counted, the file still has it and not the share's bank, and the change then sends (v2) |
| S2 | Owner's shared bank (`link_from_share`), then an editor account, an outsider, and the owner again on the same computer | Owner keeps Restore/Trim; editor is not the author (and the circle page does not change that); outsider changes nothing ("by another account" line); owner back: author again |
| S3 | Viewer leaves, restarts (link says "no longer in", rechecked once), joins again | The offer shows the bank as already in My Banks, unticked; the strip is checked again and is back to "(you can view) ... up to date" |
| S4 | Promoted viewer presses Add a full copy, is made a viewer again before the read | No keyless "full copy", the link stays, the row says why. Control: promoted for real, the full copy is an edit link with keys and the old copy is unlinked with a notice |
| S5 | Editor's full copy of a hiding bank, editor demoted, owner sends v2; a viewer's viewers'-form copy of the same bank | Editor: no Get latest on the strip, a forced one is `Blocked`, keys and the unsent wording stay. Viewer: 1 of 2 rows changed, applied |
| S6a | Join offer of 3 banks, Shared page left, another account signs in after the first bank landed | 1 bank, linked and tagged for the first account; the other two silently gone (see I2) |
| S6b | Join offer of 3 banks, then the session ends while the Account window still says signed in | Endless relaunch (see I1) |
| S6c | Add pressed twice, then Make my own copy while it waits, then Add again after it landed | One bank, linked |
| S7 | A working copy as phase B/C left it: link without the new fields, plus a circle import record; owner sends v3 | Strip works, no "Update from pool", Imported tab counts no update; Get latest applies and the record follows to v3. After Unlink the record stays (a plain copy, as plan 11 says) |

## Important

### I1. A "signed out" answer during an add relaunches it on every app frame, forever, with "Adding..." spinning

**Where**
- `circles.rs` `on_added`: a `SignedOut` error (`problem_of` -> `None`) puts the add back to `Waiting`, with the comment
  "(the sign-in ended: it is made once signed in again)".
- `tick_background` takes `session` from the snapshot, which stays `Ready` until the Account window looks the account up
  again (`snapshot_of` keys on the cached `me`). `next_add` then launches the add again at once.
- `shared_ui::take_goto` asks for a repaint every 250 ms while `tick_background` says it has work.

**Evidence**
- Probe P1 (scripted): 20 app frames give 20 bank reads; the state stays `Working`; the Banks list row still says
  `Adding "Cells" from Biology team...`, with no word about signing in. On the Shared page: 10 reads in 20 frames.
- Live S6b: after `sign_out` on the real client, 40 of 40 background ticks were busy, with the add back in `Working` each
  time. The worker saw 0 requests (the client fails locally). In the app, that is a new background job 4 times a second
  and an app that never goes idle, until the Account window looks the account up again. Meanwhile `problem_of` writes
  "Your session ended" into a notice that nothing in the background shows.

**Fix:** treat a signed-out add like `BankLinks` does (`out`): stop launching adds until the account changes or the person
signs in again, and say "Sign in to finish adding ..." on the Banks list row. Test: a scripted session that answers
`SignedOut`, 20 `tick_background` calls, at most one read.

### I2. Signing out, or another account signing in, drops the waiting adds without a word

**Where:** `forget_reads` clears `adds` and the landing slot for both a sign-out and an account change.

**Evidence**
- Probe P2: an add waiting, the Account window says signed out, then the same account signs in. Nothing is added; the
  "Adding..." row vanished; there is no notice and no failed row.
- Live S6a: 3 ticked banks from a join offer, another account signs in after the first landed. 1 bank, `adds []`, and the
  Banks list says nothing. The join offer is gone too, so the only way back is adding each bank from the circle's page.
- This contradicts the `on_added` comment above, which promises the add is made after signing in again.

**Fix:** keep the adds through a sign-out of the same account (as the comment says), and on another account turn each into
a dismissible failed row ("... was not added: you signed out before it finished. Add it again from the circle's page.").
Do not keep running them for the new account.

### I3. A send (or restore, or trim) under way on one bank is dropped when another bank's strip opens a page

**Where:** `open_send`, `open_merge` and `open_history` in `circle_edit.rs` start with `self.jobs.clear()`. There is one
edit model for all banks. In D1 each linked bank's page has its own strip, so while bank A's send uploads (up to 4 MB),
the person can go to bank B and press Get latest, Send or History.

**Evidence:** probe P8 (`cr67_probe2.rs`). Two linked banks, A with one unsent change. A's send is launched, and the PUT
reaches the server, which makes v2. Before its answer is read, B's strip opens Get latest (`open_merge`). Afterwards A's
link is still at base 1, A still shows 1 unsent change, `acting()` is false, and there is no notice.

**What the person sees:** the server has the version, but A's link stays at the old base and A's strip still counts the
changes as unsent. The next Send meets a 409 and leads to Get latest, which then merges the identical content. No question
is lost, but the person is told their changes were not sent. Before D1 this needed "< Back" and another card on the same
Shared page; now it is two clicks across the sidebar.

**Fix:** keep the `Act` job (and `inflight`) when another bank's page opens, as `close()` already does, or refuse to open
another bank's Send/Get latest/History while a send is on its way ("Wait for the send from "Unit 2 quiz" to finish").

### I4. Test gaps in the new tests (see "Mutation results")

The fix2 tests pin each fix in one shape. The variants next to them are free. The survivors that matter:

- **The viewers' form normaliser can hide and revert a real change (k25, k26, k28).** `settle_shuffle` replaces the whole
  `kind` with the base's kind whenever `order_free` says equal. So any field that `order_free` drops is silently put back.
  Today it drops none, so this is a gap in the tests, not a bug in the code. A later edit to `order_free` would go
  unnoticed:
  - k25: a Multipart part's stem.
  - k26: the text of a locked choice.
  - k28: running the settle on the apply side for full-form reads too. The review then says Take, but an author's real
    reorder is never applied.

  Nothing pins these. The m03/m04/m05 test covers only a moved locked choice and a reworded part choice, and m02 checks
  only the review.
- **Rejoin, as a real restart meets it (k08).** `rearm` forgetting `rechecked` survives. The i1 test's link only becomes
  "gone" during the test, so its one recheck was never used. A link that was already "gone" when the app started (live
  S3, probe P4) is the case that needs it.
- **The share link's author rules from critic 65 I2 (k12, k13, k15).**
  - k12: a share link (`role_seen_by` empty, `is_author` true) keeps `is_author` for another member.
  - k13: the author's own first check no longer rereads authorship.
  - k15: the circle page teaches links whose account is not known.

  All three tests use `author_link()`, whose `role_seen_by` is set.
- **The account rule on background adds (k34, k35).** Keeping the adds across an account change survives, and so does a
  `tick_background` that ignores the account. That is the critic 60/61 rule on a new surface, and nothing pins it.
- **The signed-out add (k36)** survives, which matches I1: no test covers that state at all.
- **The duplicate-add guard (k06).** It works (live S6c), but nothing pins it.
- **Try again on an unreadable links file (k32), and the re-read when the Shared page opens (k33).** If the button did
  nothing, the person would be stuck until a restart. The b1 test calls `read_links_again` directly.

## Minor

- **M1. An add stuck as "Working".** `close()` (called when an invite code is redeemed) drops the jobs but leaves
  `Working` adds as they are; `next_add` only starts `Waiting` ones. Probe P3: still `Working` after 10 frames, spinner on the
  Banks list, until the Circles tab is opened again (`reset` turns them back to `Waiting`).
- **M2. Wrong words, still (critic 65 minor).** A viewers'-form copy promoted to editor whose apply read comes back stripped is
  told KEYS_KEPT: "Your copy has its answer keys" (probe P7). It has none. Safe; nothing applied.
- **M3. Unlink with unsent changes.** The confirm ("Unlink? The bank stays here as it is ...") does not say that the N unsent
  changes can then never reach the circle. Name the count when it is above 0.
- **M4. "pool" on the Banks list.** The failed-add row composes `CirclesModel::problem_of` messages, several of which still say
  "pool" (probe P6: "The pool had a problem of its own. Nothing was changed; try again in a minute.").
- **M5. A shared bank's author is unlearnt on its first check.** `link_from_share` leaves `role_seen_by` empty, so the first
  check resets `is_author` and relearns it from the circle page. If that one read fails, the author has no Restore or
  Trim on the bank page for the rest of the sign-in (`author_asked`), unless they press Check now or visit the circle page.
  Setting `role_seen_by` from the publish window's account would avoid the round trip.
- **M6. Every check rewrites `share-circles.json`.** `apply_check_as` always sets `role_seen_at`, so each check (every 10
  minutes per open linked bank) saves the whole file. With two app windows open, the older list overwrites links the other
  made (the class is older than D1, and `banks.json` has it too, but D1 makes the writes far more frequent).

## Checked, holds

- **Links file unreadable** (critic 65 B1): `add_link`, `add_link_replacing`, `take_shared_link`, `queue_add` and the join offer
  all refuse while it cannot be read; nothing goes into memory; a failed save restores the list. Try again keeps the file's
  links. Live S1 and the code agree. The pending-merge loop in `ensure()` is now unreachable defence (k01 is equivalent in
  practice).
- **Get latest, viewers' form vs full copy:** the review refuses a stripped read for any copy not made from the viewers' form,
  and the apply refuses a stripped re-read of a full review (`ApplyRead::KeysHidden`). The viewers' form applies the body
  it reviewed. Live S5, and critic 65's L1/L2 shapes.
- **Per-account `is_author`:** live S2.
- **Rejoin re-arm:** live S3 (after a restart) and probe P4 (a gone link already rechecked this sign-in).
- **"Add a full copy" keyless refusal:** live S4, both directions.
- **Background adds, leaving the Shared page:** the copy lands from the browse window's frame and is linked by the next
  `tick_background`; nothing else is read in the background. An account switch between landing and linking links it for
  the account that asked (probe P5b). Duplicates are refused (S6c).
- **Added banks not recorded as imports:** no older path breaks. Old circle records still feed `circle_bank_ids` and the
  heads; `update_for_bank` never offers a circle bank; a linked old copy is not counted as an update; `set_circle_version`
  still follows merges (live S7). After Unlink, a new-style bank has no record and so no update notices, while an old one
  keeps its record; that matches Q7 for own copies.

## Mutation results

Each mutant ran alone on the scratch copy with `cargo test --lib share:: -- --skip cr67_` (710 tests). "fix2" is the
number of `bank_links_fix2_tests` tests that failed.

| Mutant | Change | Result | fix2 |
|---|---|---|---|
| k01 | `ensure()`: a pending link wins over the file's again | SURVIVED (equivalent in practice) | 0 |
| k02 | a failed save leaves `LINKED_TO` naming the unsaved link | SURVIVED | 0 |
| k04 | a link whose bank is gone still blocks Add | killed | 0 |
| k05 | Make my own copy refused while the file can't be read | killed | 1 |
| k06 | duplicate-add guard removed | **SURVIVED** | 0 |
| k07 | join offer ticks a bank already in My Banks | killed | 0 |
| k08 | `rearm` keeps `rechecked` | **SURVIVED** | 0 |
| k09 | `rearm` keeps the 10-minute throttle | killed | 1 |
| k10 | `rearm` keeps the last check's state | SURVIVED (low weight) | 0 |
| k12 | share link: `is_author` kept for another member | **SURVIVED** | 0 |
| k13 | share link: the author's first check does not reread authorship | **SURVIVED** | 0 |
| k15 | circle page teaches links of an unknown account | **SURVIVED** | 0 |
| k17 | keyless full copy refused but still lands | killed | 1 |
| k20 | "by another account" said for links of no known account | SURVIVED (wording) | 0 |
| k22 | free choices compared in order | killed | 2 |
| k23 | Matching left column compared in order | killed | 1 |
| k24 | Matching word bank compared in order | killed | 1 |
| k25 | `order_free` ignores a Multipart part's stem | **SURVIVED** | 0 |
| k26 | `order_free` ignores a locked choice's text | **SURVIVED** | 0 |
| k28 | settle on apply for full-form reads too | **SURVIVED** | 0 |
| k29 | apply the reviewed body for full-form reads (no re-read) | killed | 0 |
| k30 | a stripped read clears `viewers_form` | killed | 0 |
| k31 | the keys guard only for edit links | killed | 0 |
| k32 | Try again on the unreadable strip does nothing | **SURVIVED** | 0 |
| k33 | opening the Shared page does not re-read the file | **SURVIVED** | 0 |
| k34 | adds kept across an account change | **SURVIVED** | 0 |
| k35 | `tick_background` ignores the account | **SURVIVED** | 0 |
| k36 | a signed-out add fails instead of waiting | **SURVIVED** | 0 |
| k37 | a full copy never takes the link | killed | 0 |
| k41 | Check now keeps the signed-out stop | killed | 1 |
| k48 | a full copy blocked by the copy it replaces | killed | 1 |
| k53 | `save()` does not note the linked banks | killed | 1 |

The scratch sources were restored after each mutant; `diff -rq` against the repo shows only the added scratch modules and
their lines in `mod.rs`.
