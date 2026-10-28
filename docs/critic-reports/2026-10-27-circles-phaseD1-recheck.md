# Circles phase D1 — focused re-check of the fix diff, 2026-10-27

Scope: `git diff a02c12d..fb1b7e9 -- src/share/` only, read-only, after the scoped fix round for critics 101/102.
The re-checker was asked not to re-review the rest of D1.

**VERDICT: fixes hold** — no blocker, no important. Two minor residuals; both were fixed in the next commit (see
"Lead's follow-up").

## What was found correct

- **c101 I1 (merge write order).** `finish_merge` now has one order: restore point → in-memory merge → checked save
  of the whole `banks` slice → `replace_link`. No other call inside the function persists a link (the link is a
  local clone until `replace_link`; `restore.save` writes only backups). On failure, `banks[pos] = before`
  restores the clone including `modified`, and the person returns to the same Review box with the message above an
  enabled "Apply changes". The production save goes through the same `banks.json` as `Store::save_banks`, and both
  callers hand over the whole `store.banks` slice, so the later caller save writes the identical merged bank and
  cannot resurrect the pre-merge one. `banks_saved()` cannot clear a warning the write did not fix.
- **c102 I1 (held warning on every state).** `status.held` is only set for `Learning::Held`, so the NotKept
  under-review branch is untouched; the normal Held arm compares the identical `held_line` and does not double it.
  Try again is real (`read_base_again` clears the hold time). The under-review + Held combination is pinned.
- **c101 m1 (put-all-back restart).** The guard fires on `ran`, `inherit_held` stops the mask within a session, and
  the test reproduces exactly the reported case; the old guard is killed by mutation m3.
- **Wording.** `NEWS_DISMISS` names the permanent deletion and the lost offer; "your bank stays as it is" remains
  true. The badge hint no longer promises Get latest and is true wherever "newer version" is emitted. The fix18
  assertion changed only in the quoted words, not its strength. The fix19 visibility changes are test-only.

## Minor residuals (both fixed)

1. **MINOR, held override replaced every state's line.** The new block replaced the whole line (and cleared
   `busy`) for every state returning before the `Learning::Held` arm. In rare combinations — a held pass plus
   removal from the circle, `sent_unread`, `OtherAccount`, or a restore/trim in flight — the state's own
   explanation was lost (a removal's bring-back deadline, "your earlier changes were sent", the spinner), and
   `History` was pushed where `menu_history` is false.
   **Fix:** the line is replaced only where it would mislead ("You can keep working here", "works as usual");
   elsewhere the warning is appended after the state's own line, and only Try again is added (no History push,
   `busy` untouched).

2. **MINOR, the `ran` argument was incomplete.** `pass_times` is written before the safety copy and the banks
   write, and `hold_inherit` clears it from the file only if its own save succeeds. A crash between the links
   write and the banks write, or a failed safety copy with a links file that could not say so, could leave
   `pass_times` on file with the banks never written; on restart the pass would be recorded as done (a skipped
   inherit, no lost work).
   **Fix:** `ran` now also requires `bank.modified >= after` — the bank's time proves the pass's write happened.
   A later edit of that same bank after such a failure can still look like the write; that is documented and is a
   skipped feature, not lost work.

## Lead's follow-up

Both minors were fixed in `src/share/`, with a new test for the crash window
(`c101_m1_a_pass_times_write_whose_banks_write_never_happened_runs_again`, six fix21 tests in all), all six
mutants killed (m1–m6), `cargo test --lib` at 2007 passed / 5 known failures / 3 ignored, bins and wasm green, and
the Held frames recaptured at 1280/760/420 in both themes.
