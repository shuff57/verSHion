# D2 client, critic round 9 — security / ASVS

**Build reviewed:** `aac85dd` (r10). Read-only; probes and frames in a frozen `git archive` copy.

**VERDICT: FINDINGS — 0 blockers, 2 important, 2 minor.**

No full-form content reaches a viewer of a hiding bank; no withheld-version text is served or compared; restore
mechanics hold; account switch and sign-out drop the caches. Both importants are precedence/currency defects that
misreport state in reachable states (no confidentiality boundary crossed, no work lost).

## Important

### I1 — a merge carries the old base figure into a record marked as the new base version's
**Where:** `src/share/merge.rs:1028-1044` (the `OursChanged` arm; `let mut nb = ob.clone()` keeps
`figure`/`figure_recorded`, only the `Unchanged` half refreshes them).

`theirs_delta`/`ours_delta` do not compare `figure`, so a circle change of only the figure arrives as
"only you changed this". The merge keeps the copy and advances `base_version`, but the record keeps the old base's
figure and `figure_recorded = true`. Restoring the old version's question then passes the r8-T1 guard — the copy
ends in step with version 10 carrying a figure version 10 never had.

**Fix:** in the `OursChanged` path with `theirs_index.is_some()`, record the fetched version's figure and set
`figure_recorded = true`. Preserve: pre-r10 records (default false) and two-way fallback records stay false; the
guard's known/unknown branches and messages are unchanged; no `figure` field is added to `Snap`.

**Probes:** `zz_sec_r10_a_merge_records_the_old_figure_as_the_new_bases` (fails before the fix),
`zz_sec_r10_a_merge_lets_the_old_figure_back_through_the_restore_guard` (demonstration; a fix must make its restore
be refused).

### I2 — with a dropped base, the per-question picture retain keeps stale names that override the sent file's own
**Where:** `src/share/circle_links.rs:772-787` (`picture_files_kept`), called from `after_send` (`:760`) and
`after_partial_send` (`:841`); lookup precedence at `src/share/question_history.rs:653-670`.

`after_send` builds the new base with `merge::bounded`, which returns an empty vector above 500 questions or 1.5 MB,
or when the answer's qids do not align. `picture_files_kept` then matches nothing and keeps every record whose file a
bank question still has — including questions the send carried. `picture_sha_on_circle` prefers that stale name (the
old circle name, e.g. a PNG hash) over the file's own name (the re-encoded WebP hash the circle now stores), so
Compare calls the just-sent picture "a different picture". `after_partial_send` has the same edge on a large bank.

**Fix:** never guess from an empty base: a whole-bank send clears the map (every question sent as itself); a partial
send judges from the composed questions, keeping only names for questions sent as the base has them (held back).
Never let a retained name override the name derivable from a file the circle now holds under its own hash.

**Probe:** `zz_sec_r10_an_empty_base_keeps_stale_pictures_that_override_the_sent_files_own_name`.

## Minor

### M1 — `role_seen` and `viewers_form` are read strictly: one unreadable value drops the whole link
**Where:** `src/share/circle_links.rs:84-90`, skip in `parse_links` (`:983-998`).

A corrupted or hand-edited `share-circles.json` makes one entry fail to parse; the link is skipped and the next
save drops it (only `.bad` keeps the raw file), silently unlinking the bank. The r9 M3 rule was applied to `figure`
and `figure_recorded` but not to these D1 fields; `head_seen`, `role_seen_at`, `qids` and `seen.hide_key` share the
class.

**Fix:** lenient readers. `viewers_form` is the safety flag: an unreadable value reads `true` (never sendable),
never serde's `false`. `role_seen` unreadable keeps the link with `None`; an unknown `mode` stays `Follow` (already
true).

### M2 — `picture_files_kept` is O(records x questions x base) on every send/merge
About 127 M pair checks for a 990-question bank with a full base, on the UI thread. Not a security boundary; listed so
the larger fixes do not leave it. A file-to-question map plus one pass removes it.

## Verified correct (summary)

Authorization and hidden keys; withheld/pruned versions; cache scoping and account switches; read caps and 304s;
the 256-entry picture cap (reader and writer agree); lenient reads for `figure`, `figure_recorded`, `mode`,
`held_back`, `picture_files`; r10 precedence; restore integrity (restore point first, one question, Undo, draft
guard); partial-send composition and after-send state; no panic on malformed D2 replies; disk carries no account
tokens.

## Not checked

The `/changes` backend and worker; real network/staging; the 200/day `circle_change_reads` number; the `qedit.rs`
`opened_at` guard (needs the owner's yes); protected files; wasm32/share_core.

**Suite counts:** share 1553/0/3; frames 58 PNGs (18/19 inspected); probes 3 passed, 5 failed on the reviewed build.
