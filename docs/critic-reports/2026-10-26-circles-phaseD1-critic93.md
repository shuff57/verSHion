# Circles phase D1, data-safety and integration critic 93, round 15

Scope: HEAD 0d17a83 on `shared-pool-phase1` plus the uncommitted round-15 changes in `src/share/` (15 files, +360/-117).
Round 15 changes:

- after one successful read of the circle's base version, each matched question takes the circle's difficulty (the
  owner's rule);
- Send is not offered until that read succeeds, and the strip says Checking / Can't check;
- the badge says "not counted" and the copy is never called "up to date" until then;
- the open question editor's draft is updated;
- a 404 on the base read means "Not kept";
- the leave panel's wording changed;
- undecided rows now read "Keep mine" first.

I hunted for data loss and wrong data. A fresh critic who built none of this wrote this report.

## Evidence

- **Scratch copies.** `scratchpad/c93/` is `git archive HEAD` with the live `src/share/.` copied over it. Its
  `CARGO_TARGET_DIR` is `scratchpad/c93-target`, and the mutation run used it. `scratchpad/c93b/` is the same thing, with
  its own target, and was used for the probes while the mutation run held c93. It holds the final probe files (adds
  p93_3b, and runs q93_2 through the strip). At the end, `diff -rq` against the live `src/share/` shows only the probe
  files and their `mod` lines, so every mutant was restored. The only additions are the cfg(test) probe modules
  `src/share/c93_probe_tests.rs` and `c93_probe2_tests.rs`, plus their `mod` lines. The live repo was not touched,
  apart from this report. Nothing was committed.
- **Baseline.** `cargo test --lib` on c93 gives 1854 passed, 0 failed, 3 ignored, the expected baseline.
- **Probes.**
  - `c93_probe_tests.rs`: p93_1, 1b, 2, 3, 3b, 4, 5 and 7 reproduce the findings below. p93_6 is refuted and marked
    ignored.
  - `c93_probe2_tests.rs`: q93_1 to q93_4 check the races. They hold, except for the wording mismatch in q93_4.

  Run with `cargo test --lib c93_ -- --nocapture --test-threads=1` in c93b: 12 passed, 1 ignored. Each probe asserts the
  behaviour as found, so a fix will turn it red.
- **Mutation.** 50 hand-written mutants of the round-15 lines, driven by `scratchpad/c93-mut/mutants.py`, with results
  in `results.jsonl`. Each mutant runs `bank_links_fix15` first. A survivor then runs the whole `share::` suite, with my
  probes skipped.

## Blockers

None. I found no lost or duplicated question, no cross-account use of a read, no double read, and no send that goes out
before the read. Every merge still takes a restore point first.

## Important

### I1. The read erases ratings the teacher entered herself, with no restore point and no word on any screen

Probes p93_1 and p93_1b.

The owner's rule is applied literally: "for each question the version has, whatever the copy had". The read therefore
erases three kinds of rating the teacher set herself:

- her own unsent rating, where the circle's version 3 has none (Two: Low becomes none);
- her own unsent rating, where the circle has a different one (One: Low becomes High);
- **a rating she already pressed Send for, whose send did not go through** (p93_1b, with `sent_record.not_through =
  true`). The link still records what she sent (`sent_record.questions[1].q.difficulty == Low`), yet the bank becomes
  none and her next send carries `null`.

In all three cases:

- `inherit_pending` saves no restore point. The counting `RestoreSink` stays at 0; the merge that follows in the same
  probe takes 1.
- Nothing is said: `results_of(LOCAL)` is empty.
- The strip says "Your copy is up to date." and never mentions difficulty.

The teacher's work is gone, and she has no copy to go back to.

Plan section 10 lists "unsent work lost" as a blocker class. Only the owner's rule keeps this from being one. The owner
was asked about the trade-off "a colleague's dropped rating against hers". I do not think the owner was asked about:

- the cases where the circle has no rating at all;
- a rating she already tried to send;
- the absence of any restore point.

**Fix.**

- Save a restore point (`self.restore.save(banks)`) before the first overwrite, as `finish_merge` does, and say one line
  on the strip, for example: "verSHion took the circle's difficulty for 2 questions (version 3). A copy of your banks
  was saved first."
- Do not overwrite a question whose `sent_record` (`by` == this account) carries a different rating.
- Ask the owner whether "inherit" was meant to include "inherit none".

**Test.**

- A phase C link with Two rated Low by her and none in the circle: after the read, one restore point and one result
  line.
- A link with `sent_record` carrying Low on Two: Two stays Low.

### I2. "Use my bank as it is" promises the bank is sent as it is; a read that lands afterwards changes it

Probe p93_3.

1. A phase C link whose bank was put back from a saved copy (stale). One is rated Low; the circle's version 3 has High.
2. The read meets a 503 and lands in `CantReach`.
3. She presses "Use my bank as it is..." and confirms. The strip says: "Your bank is now used as it is, as matching the
   circle's newest version. It was not changed; the next send sends it as it is."
4. A minute later the read is retried, succeeds, and the pass is no longer stopped by `stale_for`, since `mark_bank`
   just cleared it.
5. One becomes High, and the next send carries "High".

If the read lands before her click, `inherit_difficulty` returns nothing (the bank is still stale), and her Low stands
and is sent (probe p93_3b). The outcome of a deliberate choice depends on which network answer comes first, and the app's
own message ends up false.

**Cause.**

- `do_rebaseline` (circle_edit.rs ~3638) marks the bank but leaves `difficulty_read: None`.
- The later `learn_difficulty` sets `inherit = true`.
- `inherit_difficulty` then sees a bank that is no longer stale.

**Fix.** Choose one:

- (a) In `do_rebaseline`, when the link still awaits its read, learn nothing into the bank. Set `difficulty_read = Some(0)`
  and set the base snaps' difficulties from the bank, since she declared the bank to be the base.
- (b) Grey out "Use my bank as it is" while the copy awaits its read.

**Test.** p93_3 turned around: after rebaseline, a read that lands later leaves One at Low, and the next send carries
Low.

## Minor

1. **The link is written as done before the bank is saved** (p93_2).
   - Round 15 takes the `inherit` flag in the first pass. `update_link` saves the links file with it at once, and the
     caller saves the banks afterwards. `save_banks` ignores errors.
   - If the app dies between the two writes, or the bank file write fails, then after a restart the circle's rating is
     counted as "1 unsent change" and her next send reverts it (One: "Low").
   - Round 14 wrote the bank first and cleared the flag one frame later, so a crash was safe. Its comment claimed this
     guarantee, and round 15 dropped it. Mutant m08 (keep the flag until a pass finds nothing) survives every test.
   - Fix: restore that order, or clear the flag only in a pass after the save.
2. **Undo of a delete brings back the old rating as her change** (p93_4).
   - She deletes One before the read. The read skips it, because One is not in the bank, and clears the flag. She then
     presses Undo.
   - One returns at its old, empty rating. The strip says "You have 1 unsent change", and the send removes the
     colleague's "High".
   - Fix: when a base question reappears with the same id, the next pass should apply what was learnt, or Undo should be
     dropped across a pass.
3. **The leave panel during the read** (p93_5).
   - A copy that awaits its read and has a reworded question shows "has 1 unsent change. Send it first if the circle
     should have it." But Send is neither offered nor opened (`open_send` returns false).
   - A copy in the same state with nothing else counted is not named at all, although its difficulty changes can't be
     counted. `unsent_in` gives `(true, 0)` → `None`, because `partial()` is true while it waits.
   - Fix: while it waits, use the "may have changes that are not sent" line with "once the circle has been checked".
4. **The daily read limit looks like a network fault** (p93_7).
   - A 429 `daily_limit` on the read (`retry_after` 10 h) maps to `NotReached`.
   - The strip says "Can't check the circle right now: try again". The read is asked again every minute until the limit
     resets, and Send stays off all day.
   - Fix: use the refusal's `retry_at` and say the limit plainly.
5. **The strip, the badge and the circle page row disagree in "Not kept"** (q93_4).
   - The strip says "You have 1 unsent change ... Sending is turned off until the circle can be checked."
   - The badge says "changes not counted".
   - The circle page row says "Not sent yet: 0 added, 1 changed, 0 removed. A change to a difficulty can't be
     counted..." and nothing about sending being off.

## Checked and holding

| Area | Verdict and evidence |
|---|---|
| Send before the read | Holds. `open_send` refuses (mutant m12 killed), the strip strips Send in every branch (m28 killed), and `do_send` can't be reached. |
| Get latest racing the quiet read | Holds. The review waits for the read on its way (c89_m2 tests). `read_bases` skips the bank being merged. `finish_merge` rebuilds the review if the bank changed (`ours_print`). |
| The open editor | Holds. `refresh_draft` runs after the strip's pass and after the Shared page's pass. The editor reads `draft.difficulty` directly (no cached widget state). Difficulty is edited only in the bank page's card editor, after the strip's pass. q93_3 shows that a pass left pending across a restart would overwrite an edit made in between, but no screen can make that edit first. |
| Try again / Check now during a read | Holds (q93_1). No second read, and one inheritance. |
| Account switch during a read | Holds through the real path (q93_2, strip, `note_account` → `forget_account`): the first account's answer is dropped and the read is asked again for the new account. Calling `edit.frame` alone with a new account does use the old answer, but no app path does that. |
| Re-inheritance after her later change | Not reachable. `learn_difficulty` runs only while `needs_difficulty_read`. `after_send` and `finish_merge` write `Some(0)` with fully known bases. `apply_merge` gives every new-base question a known difficulty. |
| Restart | `inherit` survives in the file (c90 restart test). Read states are memory only and are asked again once after a restart. |
| 15 s wait | Holds (m17 killed). It measures from the quiet read's launch, so a Get latest opened long after a stuck read says "can't be reached" at once (wording only). |
| Refuted | p93_6: an unreachable account server does not spin "Checking" for ever. The strip shows the Offline line. |
| Unicode / `text::clean` | Round 15 matches by qid only, with no text involved. The share-core oracle tests (fix10) pass in the baseline. |
| Option order | Every `options = vec![...]` now lists Keep first. Nothing reads `options[0]` as a default. |

## Mutation check of `bank_links_fix15_tests.rs`

50 hand-written mutants of the round-15 lines. Each one is run against `bank_links_fix15` first. A survivor is then
run against the whole `share::` suite, with my probes skipped.

- **The round-15 file alone** kills 37 of 50.
- **The whole suite** kills 40 of 50. m01 (no stale guard) is killed by fix14 `c90_i1_nothing_is_inherited_into_a_bank_put_back`.
  m14 (`awaits_read` ignores `gone`) and m49 (ignores `removed`) are killed by fix13
  `c87_b1_a_link_that_can_no_longer_reach_its_bank_is_not_read_for`.

Killed by fix15:

- m02-m07 and m09: the inheritance rules, the time bump, and the `sent_unread` / `sent_record` moves;
- m10-m11: the editor draft;
- m12-m13 and m15: Send gating and the account filter for reads;
- m16-m17: the 15 s wait;
- m19-m22: Try again and the learning states;
- m26-m32 and m34-m36: badge, strip and Check now;
- m39-m42 and m44-m47: the circle page row, option order, labels, the leave panel, and the save flag;
- m50.

Survivors of every test (10):

| Mutant | What it does | Verdict |
|---|---|---|
| m08 | Keep `inherit` until a pass finds nothing left (round 14's order) | Behaviourally equivalent in-process. It differs only across a crash or a failed bank save: see minor 1. Probe p93_2 tells the two apart. |
| m18 | `merge_wait_note` in any Merge phase (Fetching too) | Gap, wording only: no test of the Fetching phase while a quiet read runs. |
| m23 | `on_base_read` clears `base_asked` again (round 15 claims it fixed survivor b_asked_kept) | Still a survivor. Probably equivalent: `quiet_read_of` also needs a running Base job. |
| m24 | `close()` drops `inflight` again (round 15 claims it fixed survivor x_close_inflight) | Still a survivor. Probably equivalent: the next send replaces it, and the answer takes it. |
| m25 | `unsent_in` skips copies whose changes can't be counted | **Gap.** The "may have changes that are not sent" line of the leave panel has no test. |
| m33 | Checking while behind: no Get latest | **Gap.** No test of the Checking state with a newer version. |
| m37 | "Check now" in the menu even when the strip shows the button | Gap, a duplicated control only. |
| m38 | `bank_links_ui::strip` no longer calls `refresh_draft` | **Gap.** The production wiring of critic 91 I1 is not pinned. The tests call `refresh_draft` by hand, and the Shared page's call in `draw_shared` is unpinned too. |
| m43 | key-only row options back to Take, Keep | **Gap.** Critic 92 I3's order is not pinned for the "you entered its answer key" row. |
| m48 | `learn_base` no longer calls `recount` | Gap. In the tests a recount still happens through the `unknown` flag or the time bump. A partly learnt link whose `unknown` flag stays the same would keep a stale count until the bank changes. |

Equivalent or harmless: m08, m23, m24, m37. Real test gaps: m25, m33, m38 and m43, plus m18 and m48 as weak gaps.
