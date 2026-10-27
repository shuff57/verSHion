# Circles phase D1, data-safety and integration critic 95, round 16

Scope: HEAD 0d17a83 on `shared-pool-phase1` plus the uncommitted rounds 15 and 16 in `src/share/` and `src/bankui.rs`.
The round-16 diff was taken against critic 93's round-15 snapshot (`scratchpad/c93/src/share`).

Round 16 changed these things:

- a restore point is saved before the read's ratings replace hers, and the strip says so once;
- a rating she already pressed Send for is kept when her send record carries a different one;
- save or retry on failure: Send is blocked (`Learning::Held`) and it is tried again after 60 s;
- `banks.json` is written before the links file says the read is done;
- "Use my bank as it is" settles a pending read, and `c87_b1_p3` was rewritten;
- Undo of a deleted question takes the circle's rating;
- the daily read limit is waited out;
- the "not kept" wording agrees across screens, with a new "Use my bank as it is..." button;
- the first read failure still says Checking;
- the close and remove panels name unsent changes;
- `bankui.rs` changed layout only.

I hunted for data loss and wrong data. A fresh critic who built none of this wrote this report.

## Evidence

- **Scratch copy.** `scratchpad/c95/src_tree` is `git archive HEAD` with the live `src/share/.` and `src/bankui.rs`
  copied over it. Its own `CARGO_TARGET_DIR` is `scratchpad/c95/target`. The only additions are
  `src/share/c95_probe_tests.rs` (cfg(test)) and its `mod` line. The live repo was not touched apart from this report.
  Nothing was committed.
- **Baseline.** `cargo test --lib` gives 1885 passed, 0 failed, 3 ignored. That was rerun after the mutation run,
  skipping the probes: 1885 again.
- **Probes.** 14 in `c95_probe_tests.rs`. Run them with `cargo test --lib c95_ -- --test-threads=1`: 14 passed. Each
  one asserts the behaviour as found, so a fix will turn it red. p95_4b, p95_7 and p95_10 are controls that hold.
- **Mutation.** 64 hand-written mutants of the round-16 lines, driven by `scratchpad/c95/mut/mutants.py`, with results
  in `mut/results.jsonl`. Each mutant runs `bank_links_fix16` first. A survivor then runs the whole `share::` suite, with
  the probes skipped. At the end, `diff -r` against a copy taken before the run is identical, so every mutant was
  restored.

## Blockers

None. I found no lost or duplicated question, and no truncated `banks.json`. Both the strip and the Shared page pass the
full `store.banks` to `sync`, and `LinkLog::Store` writes the same backend document as `Store::save_banks`, atomically
(tmp file, then rename). Every merge still takes a restore point first.

## Important

### I1. A rating she sets while the ratings are held is overwritten by the retry, in the same session or after a restart

Probes p95_2 and p95_2b.

1. Phase C link. One is Low (hers) and the circle's version 3 has High.
2. The read succeeds, but the restore point (or `banks.json`) can't be written, so the bank is `Held`.
3. She then deliberately rates One Medium in the card editor, and the app saves it.
4. 61 s later the retry replaces Medium with High. The strip says "Ratings were updated from the circle's version 3".

The same happens on the first pass after a restart, because `inherit` stays true in the links file (p95_2b). The brief
names the restart variant as a known risk. It also happens in-session, with no quit at all.

This breaks the second half of the owner's rule ("a later deliberate change supersedes"). Two things soften it: a
restore point is taken before the overwrite (it holds Medium), and the line appears. But putting that copy back means
replacing `banks.json` by hand, which also undoes everything else done since.

**Cause.** `inherit_pending` re-runs `inherit_difficulty` over the bank as it is now. Nothing records what the bank held
when the read was learnt.

**Fix.** When the read is learnt (or first held), record each base question's difficulty in the bank on the link,
alongside `inherit` so that a restart honours it. On every later attempt, skip any question whose difficulty changed
since then.

**Test.** p95_2 and p95_2b turned around: Medium stays, and only the untouched questions take the circle's rating.

### I2. "Use my bank as it is" on a link the round-12 build wrote turns the bank-filled base into version 3's ratings

Probes p95_1 (stale) and p95_1b (not kept, using round 16's new strip button).

The round-12 build filled a link's base difficulties from the bank (critic 88's scope note; `needs_difficulty_read`'s
doc). Critic 87 B1 and critic 89 I1 require those values to be "learnt over" by the read, never taken as the circle's.

Round 16's `do_rebaseline` sets `difficulty_read = Some(base_version)` and leaves those values in place, so they become
version 3's ratings for good. With One rated Low by her (unsent) and none in version 3:

- **Strip after "Use my bank as it is".** "Version 3 is the latest. Your copy is up to date." The count is
  `known, 0 changed`, and Send is not offered. That contradicts the line just above it: "the next send sends it as it
  is".
- **Get latest (version 4, One still unrated).** "Changed in the circle · One · Difficulty · In version 3: Easy · Yours
  now: Easy · In version 4: none". The version 3 rating shown is false, and Take is preselected. After Apply, her Low is
  gone.
- **The phase C variant.** The rewritten `c87_b1_p3` pins only this variant (base `None`). There the count is partial,
  Send is offered, and Get latest is a Conflict that waits for her choice.

**Reach.** Only links written by the round-12 D1 build, so it depends on whether such links exist on the owner's machine.

**Fix.** In `do_rebaseline`, when `difficulty_read` is `None`, also set every base `snap.difficulty` to `None` before
recording the read. That is what the phase C link already gets.

**Test.** Add the round-12 variant of `c87_b1_p3`: base One `Some(Low)` from the bank, `difficulty_read: None`. After
"Use my bank as it is", the count is partial, Send is offered and carries Low, and Get latest gives a Conflict.

## Minor

1. **A review built while the ratings are held calls the circle's rating hers** (p95_3; p95_3b by the reachable route).
   - The route: the page opens behind, and the first read fails, so the strip shows "Checking" with Get latest. She
     presses Get latest. The merge's own base read lands, but `banks.json` can't be written (Held).
   - `build_review` runs anyway. It shows "Changed by you · One · Only you changed this. It stays as it is · In version
     3: Hard · Yours now: none", with no choice.
   - After Apply, `finish_merge` clears `inherit`. One counts as 1 unsent change, and her next send removes the
     colleague's High. That is critic 92 I1's harm again.
   - The page's own line, "verSHion tries again in a minute", is then false: nothing is tried again.
   - Fix: hold the review while the bank is `Held` (as `merge_waits_for_base` does), or apply the read's ratings to the
     ours side before the plan is built.
2. **Signed out after a restart, her own send record is not recognised** (p95_4; control p95_4b).
   - With `inherit` still pending and no Ready snapshot, `self.account` is `None`, so `record_mine` ignores a record
     whose `by` is her tag.
   - The Low she pressed Send for is replaced. Signed in, it is kept.
   - Fix: while no account is known, treat any send record as protective, or defer the pass when one exists.
3. **A full restore point every minute while `banks.json` can't be written** (p95_5: 11 copies in 10 minutes).
   - Each retry saves a new `banks-before-merge-*.json`, with no cap. A lock on `banks.json` (sync tool, antivirus) can
     fill the disk over a day.
   - Fix: keep the first restore point's path in the hold and reuse it.
4. **Tidy's Undo after the read restores the pre-read ratings** (p95_6).
   - `tidy::undo` writes whole `Question`s back from its snapshot. If Tidy ran while the read could not reach the
     circle, Undo after the read puts back her old ratings. The strip then says "2 unsent changes", and the next send
     removes the colleague's ratings.
   - Same class as critic 93 minor 2, which round 16 fixed only for the delete Undo.
   - Fix: Tidy's Undo restores only the stem fields it touched.
5. **A huge `Retry-After` on the daily-limit read panics** (p95_8).
   - It fails with "overflow when adding duration to instant" (`circle_edit.rs:2157`), inside `frame()` on the UI thread.
   - The same `now + Duration::from_secs(s)` exists at 6 older sites: `mine.rs:778`, `circle_send.rs:237`,
     `publish.rs:1593`, `browse.rs:1042`, `invite.rs:170`, `circles.rs:1368`.
   - Fix: use `checked_add` with a cap.
6. **The fix16 daily-limit test does not pin "waits the server's time"** (p95_9, survivor m25).
   - `c93_m4` puts `retry_after` in the JSON body, which `ErrorDetail` never reads (it takes the header only). The test
     therefore passes on the 30-minute `BASE_UNREADABLE_RETRY` fallback. A real header is honoured.
   - Fix the test: use `s.on_wait`.

## Checked and holding

| Area | Verdict and evidence |
|---|---|
| Restore point contents | Holds. It holds the pre-change banks (fix16 test, p95_7). It is saved before any write (m35 killed). |
| Putting it back | Holds (p95_7). The ratings are not taken again (`inherit` is false), hers come back as 2 unsent changes, and Send carries Low. The link is not "put back", since inherit never re-marks the link. Restoring means replacing `banks.json` by hand; there is no in-app restore of `backups/*.json` (the same as for merges). |
| Write order | Holds: restore point, then `banks.json`, then the links file (fix16 m1 test; m36 killed). On the web, writes are queued in order and never fail, so Held does not occur there. |
| Sent rule | Holds for her own record and for one with an empty `by` (m01, m02, m32 killed). Another account's record is not kept, as pinned. Gap: minor 2. |
| `c87_b1_p3` rewrite | Holds for phase C links. Fails the c87 B1 intent for round-12 links (I2). |
| Use as is against a late read | Holds (m28 killed). |
| Undo of a delete | Holds, and touches only the question put back (p95_10 passes; see m03). |
| Send gating | Holds. `open_send` refuses while `inherit` is true (m41 killed), and the Held strip has no Send (m12, m15 killed). |
| Cross-account sends, unclear-send replay | Not touched by round 16. The baseline tests pass. Not re-probed beyond minor 2. |
| `bankui.rs` | Layout only: the breadcrumb is truncated, the name is 30 px narrower, and the strip is drawn in a scope of the page's width. `strip` is called once, and `hosted` returns as before. |

## Mutation check of `bank_links_fix16_tests.rs`

64 mutants. fix16 alone kills 51. The whole suite kills 52: m07 is caught by fix15
`s15_not_kept_and_behind_offers_get_latest_first`. 12 survive:

| Mutant | What it does | Verdict |
|---|---|---|
| m03 | `inherit_difficulty` ignores `only` | **Real, harmful gap.** At an Undo the put-back pass would overwrite her later change to another question. p95_10 kills it and passes on the real code. |
| m05 | `LinkLog::save_banks` writes nothing | **Real gap.** No test runs the production `banks.json` write: every test uses `bank_saver` or `Memory`. A `LinkLog::Dir` test should read `banks.json` back. |
| m25 | Daily limit ignores `retry_after` | **Real gap** (minor 6). |
| m42 | Get latest reads the base during the limit | Gap. Nothing tests Get latest while Limited. |
| m43 | `Fetch::UpToDate` branch ignores "not kept" | Gap. `c94_m1` covers only the 400 `base_version` branch (m44 killed). |
| m53 | A lost Base job always counts as the first failure | Gap. It would stay "Checking" and be asked again every 5 s. |
| m10 | Signed out: a zero `role_seen_at` gives a date | Gap, wording only. |
| m38 | Hold kept after success | Weak gap. It delays an Undo pass by up to 60 s. |
| m47 | Use as is keeps held and missing | Weak gap. An Undo after "Use my bank as it is" would take the circle's rating. |
| m29, m33 | Missing ids kept across base moves; recorded for a put-back bank | Probably equivalent in reachable states. |
| m46 | Use as is keeps `inherit` | Equivalent in practice. Held never coexists with a stale or not-kept copy, so the button is not reachable then. |

Real gaps: m03, m05, m25, m42, m43 and m53. Weak gaps: m10, m38 and m47. Equivalent: m29, m33 and m46.
