# Circles phase D1 — critic 101 (data safety), 2026-10-27

Run: 2026-10-27 evening, read-only on `a02c12d` (diff base `0d17a83`), per
`docs/critic-reports/2026-10-28-round101-102-brief.md`. Scope: everything under `src/share/` in that diff; the rest of
the dirty tree belongs to other sessions.

**VERDICT: important problem(s)** — one important data-safety path and one minor residual. No blocker.

## I1 (important) — a merge whose banks write fails loses her merge and leaves the link advanced

`src/share/circle_edit.rs:4320-4369` (`finish_merge`), called from `src/share/bank_links_ui.rs:275-277`;
`src/store.rs:477` (`Store::save_banks`).

`finish_merge` is the one model path that writes the links file before the banks file. It applies the merge to the
in-memory bank (`banks[pos] = bank`), advances and **persists** the link (`self.replace_link(link)` → `save()` →
`share-circles.json`), then returns `true`. The merged bank reaches disk only through the caller
`bank_links_ui::strip` → `store.save_banks()`, which discards the error:

```rust
if out.save { store.save_banks(); }          // bank_links_ui.rs:275-277
pub fn save_banks(&self) { let _ = save_json("banks.json", &self.banks); }   // store.rs:477
```

Failure scenario (the app's own accepted failure mode — the round-19 tests model it by making a `.tmp` path a
directory): `banks.json.tmp` cannot be written. The teacher opens Get latest, reviews, presses Apply changes. The
restore point in `backups/` succeeds; the merge is applied in memory; `share-circles.json` is written with
`base_version` = the circle's newest, `base` = the merged questions and `bank_print` of the merged bank. The page says
the change was applied. She quits before any other bank save reaches disk. Next start: `banks.json` is the pre-merge
bank while the link sits at the new base, so the bank is stale ("This bank was put back from a safety copy, so
verSHion can't tell what you changed since version N"). Her per-question merge decisions are gone; a later Get latest
finds `link.base` non-empty, so `need_base` is false and it falls to `merge::unknown_base`, turning every shared
question into an undecided Conflict row.

Evidence: the function's comment says "restore point first, then the bank, then the link", but the disk order is
restore point → link → (UI) banks. `replace_link(save_link_now/save)` writes the list immediately
(`circle_edit.rs:2089-2096`). No test covers a merge whose banks write fails: the merge tests call `strip_with` and
inspect `banks`, never the store write. On wasm the writes are queued and do not fail, so this is native-only.

Scope: this code predates `0d17a83` (critics 63-90), so it was not introduced by rounds 19-20. It is reported because
it is exactly the round-19 I2 class ("a save that fails while edits are in memory"), which this diff fixed in
`put_back_these` and `inherit_pending` but not here.

Suggested fix (not applied): write the merged bank with the model's checked save **before** `replace_link`; on
failure `back(...)` with "nothing was changed", as the restore-point branch at 4321-4327 already does.

## m1 (minor) — putting back every question the difficulty pass moved lets a restart re-run it

`src/share/circle_edit.rs:2392-2415` (guard), `src/share/circle_links.rs:617-628` (`inherited_before`); test gap in
`src/share/bank_links_fix19_tests.rs:335`.

The round-19 m2 guard recognises "the pass already ran" only through `inherited_before`, which returns the questions
that still carry the circle's difficulty now. If she puts back *every* question the pass moved — one press with
round 20's "Put all of mine back" — `already` is empty, the `only.is_none() && !already.is_empty()` guard is skipped,
and after a restart (the pass's links write failed, so `inherit`/`at_read`/`pass_times` are still on file)
`inherit_difficulty` sees `at_read[id] == q.difficulty` (her restored value) and imposes the circle's difficulty
again. Her deliberate put-back is undone and the news offers it again. Limited to difficulty values, recoverable by
pressing the re-offered button; no question text at risk. The pinning test puts back one of two moved questions, so
the complete case is never exercised.

## Coverage

Checked clean: links-file load/save (`load_checked`, an unreadable file never written over; per-entry lenient
readers; `.bad` copies); the inherit/held write order (links with `at_read`/`pass_times`, then one safety copy per
state of all banks, then the checked banks write, then the links that say it was done) and the m1 retry that never
writes `banks.json` while the links file is behind; `hold_inherit`; put-back writing banks first with the warning
strip, badge warning, Try again and `banks_again`; `settle_all_news`; send: follow links and viewers'-form links can
never send, `after_send`/`finish_send` use the as-sent snapshot, conflict/`sent_unread`/`sent_record` bookkeeping is
per account and per turn; share-to-circle `link_from_share` (version 1, qids by position, `mark_bank` on the as-sent
snapshot, AlreadyLinked guard); account switch/sign-out; restore/trim are server-side and write no local bank.
Spot-checked the round-19/20 mutation claims against the code; they would fail on the mutants in `HANDOFF.md`.

Not checked: cargo build/test (read-only), a live/staging circle read, the mutation harness itself.

## Lead's note

I verified I1 against the live tree before recording it: the disk order above is real, `save_banks` does discard the
error, and no test covers the failing-banks-write merge. Per the brief, an important finding stops this round: no fix
was started and no another-round was opened; the owner is told.
