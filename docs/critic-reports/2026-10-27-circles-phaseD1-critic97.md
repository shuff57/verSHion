# Circles phase D1, data-safety and integration critic 97, round 17

Scope: HEAD 0d17a83 on `shared-pool-phase1` plus the uncommitted rounds 15, 16 and 17 in `src/share/` and `src/bankui.rs`.
The round-17 diff was taken against critic 95's round-16 snapshot (`scratchpad/c95/src_tree/src/share`).

Round 17 changed these things:

- each question's difficulty at the read is stored on the link (`at_read`), and the inherit pass skips a question she changed
  since then (also after a restart);
- what the circle's difficulty replaced is kept on the link (`difficulty_news`) until she dismisses it, with "Put mine back"
  per question and a Dismiss;
- a failed save keeps its reason, its tries and its safety copy, and makes no new safety copy while the bank is unchanged;
- signed out after a restart, any send record still protects the difficulty she sent;
- "Use my bank as it is" has its own page and an `as_is` flag, and it clears the base difficulties of round-12 links;
- Get latest waits while the difficulty can't be saved;
- the daily limit uses a capped `Retry-After` (`api::later`) at 7 sites;
- the safety copy is named `banks-before-difficulty-*`.

Known and left alone, as briefed: Tidy's Undo brings back the pre-read difficulty (`src/tidy.rs`), and the `expires_in`
overflow in `client.rs` `set_access`.

I hunted for data loss and wrong data. A fresh critic who built none of this wrote this report.

## Evidence

- **Scratch copy.** `scratchpad/c97/` is `git archive HEAD` with the live `src/share/.` and `src/bankui.rs` copied over it.
  Its own `CARGO_TARGET_DIR` is `scratchpad/c97-target`. The only addition is `src/share/c97_probe_tests.rs` (cfg(test)) and
  its `mod` line. The live repo was not touched apart from this report. Nothing was committed.
- **Baseline.** `cargo test --lib` gives 1920 passed, 0 failed, 3 ignored. With 12 of the probes it gave 1932 passed. The
  13th probe was added after that run, and only the `c97_` filter was run again.
- **Probes.** 13 in `c97_probe_tests.rs`. Run them with `cargo test --lib c97_ -- --test-threads=1`: 13 passed. Each of the
  nine `p` probes (p1 to p9) asserts the behaviour as found, so a fix will turn it red (p7 is latent: see the table below). The four `ctl_` probes are controls
  that hold.
- **Mutation.** 66 hand-written mutants of the round-17 lines, driven by `scratchpad/c97mut/mutants.py`, with results in
  `c97mut/results.jsonl`. They ran in a separate tree (`scratchpad/c97m`, its own target). Each mutant runs
  `bank_links_fix17` first. A survivor then runs the whole `share::` suite, with the c97 probes skipped. At the end,
  `diff -r` against `c97mut/share_before` is identical, so every mutant was restored.

## Blockers

None. I found no lost, duplicated or reverted question, and no write of `banks.json` that drops a bank. The success path
still writes in order: safety copy, then `banks.json`, then the links file. `banks.json` from `LinkLog::save_banks` has the
same format as `Store::save_banks`: both are `serde_json::to_vec(&banks)` into the same document.

## Important

None. Each finding below either needs a rare combination (a save failure plus a later step, two accounts on one computer, a
hand-edited file), or the app still tells her what happened and offers a way back. m1 comes closest: it loses a colleague's
difficulty with no word, but it needs a delete before the read, an Undo after it, and `banks.json` failing at that moment.

## Minor

### m1. A held Undo pass is dropped: the question keeps its pre-read difficulty, Send is offered, and the send removes the colleague's difficulty

Probe p97_5.

1. One (her Low) is deleted before the read, and the read (version 3: One High) settles the link.
2. `banks.json` can't be written. She presses Undo, and One comes back with Low.
3. The put-back pass (`only = [One]`) can't be saved, so it is held.

What happens then:

- The `Some(back)` branch of `inherit_pending` (circle_edit.rs ~2282) removes One from `inherit_missing` before the save is
  tried. The pass is therefore never tried again.
- `learning_of` sees `inherit == false`, so the strip is not Held: it says "Send my changes", and `open_send` succeeds.
- Once saving works, One still has Low. It counts as her change, and her next send carries `"Low"`, which removes the
  colleague's High. That is critic 93 minor 2's harm, back by another way.
- The `only` pass also never records `at_read`. If it were retried, a change she made during the hold would be overwritten.

**Fix.** Prune `inherit_missing` only after a pass that worked. Treat a held `only` pass as Held (Send waits), as the full
pass is.

### m2. "Put mine back" says "yours is back, not sent yet" forever, also after it was sent

Probe p97_1. After Put mine back and a send that went through (`after_send` to version 4), the count is (0, 0, 0). The news
row still says "Question 2, \"Two\": yours (Easy) is back, not sent yet", until she dismisses it. That is a false statement
about her data, and it may lead her to send again or to doubt the send.

**Fix.** Say "not sent yet" only while the bank differs from the base (`changes_since` counts that question), or drop
put-back rows once the base version moves on.

### m3. "Put mine back" writes the links file (put_back) before the banks file

Probe p97_2.

- `put_mine_back` saves the link with `put_back = true` through `update_link`. Only afterwards does the caller run
  `store.save_banks()`, which ignores its error.
- If `banks.json` can't be written and the app is quit (or crashes in between), the next start has the circle's None in the
  bank. The strip still says "yours (Easy) is back, not sent yet", and that row's "Put mine back" button is gone, because
  `news_view` trusts the flag.
- This is the save order critic 93 minor 1 fixed for the pass, reversed here.

**Fix.** Derive "back" from the bank alone (`q.difficulty == before`). If the flag is kept, write it only after the bank has
been saved.

### m4. The one safety copy kept during a long hold misses work done on other banks during it

Probe p97_3.

- `saved_before` (circle_edit.rs ~2302) compares the print of the inherited bank only, but the copy holds every bank.
- While `banks.json` can't be written, bank B is edited. The final successful pass writes `banks.json` with B's work, and the
  only `banks-before-difficulty-*` copy is still from the first try, with B as it was then.
- Restoring that copy to undo the difficulty would also silently undo B's work from the hold.
- Round 16 made a copy per try (critic 95 minor 3). Round 17 traded that for a copy that is stale for the other banks.

**Fix.** Key `restored_for` on the print of all banks (or the bytes written), not on this bank's print.

### m5. Signed out after a restart, another account's send record is kept as hers

Probe p97_4.

- The round-17 rule `if self.account.is_none() { link.sent_record.clone() }` treats any record as protective. That
  includes one whose `by` is another account.
- The pass then settles the link for good (`inherit` false), and that account's Low stays in her copy as her own unsent
  change. Signed in, the same restart takes the circle's (the control in the same probe).
- Narrow: two accounts on one computer, plus a held read across a restart, plus that account's unclear send.

**Fix.** While no account is known, defer the pass for a link whose record has a non-empty `by` (or keep only records whose
`by` is empty or matches the last account seen on the link).

### m6. A links-file write that fails after a successful pass: after a restart the news is lost and a later change is overwritten

Probe p97_8 (`LinkLog::Dir`, the links file made unwritable after the read is learnt).

- The pass writes `banks.json` (the circle's High on One, None on Two), but the links file keeps `inherit = true` and no
  news. The strip says "could not be saved".
- She then rates One Medium. After a restart the first pass replaces Medium with High again.
- Two's "Low -> none" is no longer in the news, so it can't be put back.
- A crash between the two writes loses the news in the same way (with no edit to lose).

**Fix.** When the links-file write after a successful pass fails, keep the `at_read` and news in memory and retry the links
save at the next frame. Or record the read-time difficulties on the link before `banks.json` is written, so that a re-run
pass after a restart skips what she changed.

### m7. Strictness of the new fields: a non-bool `as_is` drops the whole link, and one bad `at_read` entry drops all of them

Probe p97_6.

- `as_is` (like `inherit`) is a plain `bool`. `"as_is": "yes"` makes the whole link unreadable: it is skipped and the file
  is kept as `.bad`.
- `at_read` with one entry of an unknown difficulty drops every entry. With `inherit` pending, the next pass then has no
  protection.
- Only a future or hand-edited file does this. The new fields are not "dropped alone" in every case.

**Fix.** Use a lenient bool, as for `mode`, and read `at_read` per entry.

### m8. The news badge hides "newer version" on the Banks list until dismissed

Probe p97_9. `badge_of_sent` checks `difficulty_news` before `behind()`. A bank with news and a newer circle version says
"difficulty updated" on the Banks list, and never "newer version", until she dismisses the news. The strip still offers Get
latest. Since the news has no end besides Dismiss, the list's "newer version" signal can be gone for good.

**Fix.** Show both ("newer version · difficulty updated"), or let behind take precedence.

## Checked and holding

| Area | Verdict and evidence |
|---|---|
| `at_read` skip, held in-session and across a restart | Holds (fix17 c95_i1 tests; k01 killed). Gaps: the Undo pass (m1), a links-file write that fails (m6), and a second failed try (survivor k11). |
| Put mine back | Holds as an ordinary unsent change. It is counted (0,1,0), the next send carries hers, the bank's time moves on, the open editor's draft is told, and Get latest to v4 keeps hers (`ctl_put_back_carried_and_kept_through_get_latest`). Wording and order issues: m2, m3. |
| Dismiss | Holds. It is saved in the links file and survives a restart (`ctl_dismiss_is_saved_and_survives_restart`). |
| Old links file | Holds. A round-16 file with no `at_read`/`difficulty_news`/`as_is` and an unknown extra field loads with defaults and no `.bad` (`ctl_round16_links_file_loads`). Unknown fields are not kept on the next save (as before). |
| Save order on success | Holds: safety copy, then `banks.json`, then the links file. The gaps are when the links-file write fails or a crash comes between (m6). |
| Get latest while held | Holds. No review is built while held, and it is built once saved (fix17 c95_m1; k40, k41). |
| Use my bank as it is | Holds. A late read is not learnt (`learn_base` needs `needs_difficulty_read`). Round-12 bases are cleared to unknown (k50). A send or merge ends `as_is`. The direct page skips the fetch only for a copy that can't be read and is not stale. |
| Cross-account, unclear-send replay | Not changed by round 17 except the signed-out record rule (m5). `after_send` and `finish_send` use the bank as sent, so a Put mine back during a send stays unsent. |
| Latent, p97_7: read learnt with no pass | `CirclesModel::tick_background` calls `edit.frame` while an act runs, so a quiet read can be learnt (`inherit` set) with no `sync` and no pass. At the model level, a difficulty changed in that gap is overwritten by the first pass (the probe edits the bank directly). I found no UI route to it: the question editor is drawn only on the bank page (`qedit::draw_inline` in `bankui.rs`), and the exam builder's Edit switches to that page, whose strip runs the pass before the editor. So it does not count. If a later screen edits difficulties off the bank page, record the read-time state at learn time. |
| Daily limit | Holds. `api::later` caps at 2 days at all 7 sites. No other `Instant + Retry-After` is left in `src/share` (only `client.rs`, as briefed). |
| `banks-before-difficulty-*` | The pass passes the name "difficulty" (fix17 sink test; k96 killed). Nothing pins the real `StoreRestore` file name (k95 survives). |

## Mutation check of `bank_links_fix17_tests.rs`

66 mutants. fix17 alone kills 52. The whole suite kills 54:

- k72 (the news badge drops the unsent count) is caught by fix15 `c91_p4_nothing_is_sent_before_the_read`;
- k94 (the circle row's held arm removed) is caught by fix16 `c93_i1_no_restore_point_no_change_and_no_send_until_it_works`.

0 compile errors. 12 survive:

| Mutant | What it does | Verdict |
|---|---|---|
| k11 | `at_read` taken again at every held pass, not only the first | **Real, harmful gap.** With two failed tries around her change, the second try records her Medium as the read-time state, and the third replaces it with High. `c97_ctl_her_change_survives_two_failed_tries` passes on the real code and fails under k11 (checked). Add it to fix17. |
| k63 | `api::later` without the 2-day cap | **Real gap.** `c95_m5` only tests the base read, which caps it a second time itself. At the 6 other sites a `u64::MAX` `Retry-After` overflows, `unwrap_or(now)` then gives "try again now", and a daily limit is retried at once. Test `later(now, u64::MAX)` directly, or one other site. |
| k95 | `StoreRestore::save_named` ignores `what` (writes `banks-before-merge-*`) | **Real gap.** The real file name is untested (the tests use fake sinks). Test `StoreRestore` with a `FsBackend` in a temp dir. |
| k81 | The news row ignores the `put_back` flag (bank compare only) | **Gap, and supports m3.** Nothing needs the flag. Without it, p97_2 would show the honest "Easy -> none" with Put mine back after the restart. |
| k56 | `open_merge_to_rebase` (stale path) does not note `as_is` | Gap, UI only. On the stale path "No, go back" returns to the Get latest question instead of the bank, and the title says Get latest while fetching. The fix17 test covers only the not-kept direct page. |
| k14, k51 | `at_read` left on the link after a pass that moved nothing, or after "Use my bank as it is" | Weak. A leftover `at_read` would only steer a later Undo pass, and those ids are not in it. |
| k03, k06 | `learn_difficulty` / `after_send` keep `at_read` | Equivalent. `at_read` is set only while `inherit` waits, and then neither a learn nor a send can happen. |
| k16 | Re-taking a news item keeps `put_back` | Equivalent. A question can't be moved twice by the pass (the Undo pass covers only questions the first pass did not have). |
| k22 | The hold does not drop the old status | Equivalent. `refresh_one` recomputes `learning` and `held` every pass. |
| k23 | The old Ratings line is not dropped | Equivalent. Nothing writes `ResultKind::Ratings` any more (dead code). |

Real gaps: k11, k63 and k95, with k81 and k56 as smaller ones. Weak: k14 and k51. Equivalent: k03, k06, k16, k22 and k23.

## Counts

Blockers 0, important 0, minor 8. Probes: 8 reproducing (p1 to p6, p8, p9), 1 latent (p7), 4 controls holding. Mutants: 66; 54 killed (52 by fix17); 12
survivors, of which 3 are real gaps, 2 smaller gaps, 2 weak and 5 equivalent.
