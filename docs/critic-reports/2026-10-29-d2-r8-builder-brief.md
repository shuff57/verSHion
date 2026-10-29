# D2 client round 8 — builder brief (2026-10-29)

r7 is at `e977032` (8 commits; share suite 1536/0/3). The sixth critic pair (product vs Moodle,
security vs ASVS) **both returned FINDINGS**. I re-ran every probe below against the green tree; all
fail as described. Both critics independently found the same R2 root cause.

## Important

### T1 (product + security, one root cause) — the figure guard is still a guess; it allows a figure the base does not have
**Location:** `src/share/question_history.rs:1108-1114`
`undoable = existing.is_some_and(|q| q.figure.is_some() && merge::ours_changed(b, q))`.
`ours_changed` sees text only, never figures, so any counted edit satisfies it; the version's figure is
never checked against the base's. r6 refused these; r7 newly allows them and the strip then says
"Your copy is up to date" over a figure that can never be sent.

- Security: stem edit, base figure a=3, version figure a=9 → `Ok(6)`, strip `changed: 0`.
- Product: copy figure a=3, version figure a=5, base a=4 → `Ok(6)`, strip "up to date".
- Mirror: product also shows the same guard still refuses a restore that lands the copy exactly on the
  base when the copy has **no** figure, with a message that falsely says "differs only in figure
  settings".

**Root cause:** `BaseQuestion.snap` holds no figure, so the build cannot tell "undoing my figure edit"
from "introducing a figure the base lacks". Do not infer it from `q.figure.is_some()` again.

**Required fix — record the base's figure in the link:**
1. Add `figure: Option<FigureSpec>` to `BaseQuestion` (`src/share/circle_links.rs`), `#[serde(default)]`
   so old links still parse.
2. Populate it at **every** base-creation site: landing (`base_of`), merge (`apply_merge`), send
   (`base_for`), partial send. The source question already carries the figure.
3. In the guard, compare the restored question's figure with the **recorded base figure**:
   - equal → allowed; the restore may undo counted edits (count drops to 0);
   - different, or the record is `None` while the version has a figure → `NotCountable` unless the
     restore counts as a change (unknown record is treated as unsafe).
4. Correct the refusal message (`question_history_ui.rs:873`) so it never claims "differs only in
   figure settings" when the restore also undoes a counted edit.

**Probes — all four must pass:**
- security `/tmp/d2critic2-security/.../question_history_tests.rs`
  `zz_critic_a_text_edit_lets_an_uncounted_figure_move_pass_the_guard` → now `Err(NotCountable)`
- product `/tmp/d2critic2-product/.../question_history_tests.rs`
  `zz_prod_r2_allows_a_figure_not_in_the_base_and_the_strip_says_up_to_date` → `Err(NotCountable)`
- product `zz_prod_r2_still_refuses_a_restore_that_lands_in_step_when_the_copy_has_no_figure` → `Ok`
- r7's own `d2m_a_restore_that_undoes_an_unsent_edit_is_not_refused_as_figure_only` → must still `Ok`

### T2 (product) — R3 is incomplete on the unpaired restore path
**Location:** `question_history.rs:1159-1170` (writes `picture_now` into the base entry), `:1180-1188`
(the `unpair` branch does `l.base.retain(...)`, discarding it), `picture_sha_on_circle:651-662`.
A `removed_in_circle` restore of a picture the store kept as WebP loses the record with the base entry,
so Compare says "a different picture" against the very version just restored.
**Fix:** keep the record outside the base entry — a link-level `file → circle-sha` map (or equivalent)
consulted by `picture_sha_on_circle`, surviving unpair. Keep the paired path working.
**Probe:** product `zz_prod_r3_an_unpaired_restore_still_compares_as_a_different_picture` → must pass.

## Minor

- **S — `/changes` strings kept whole for the session.** `circle_versions_api.rs:212-214` (qids in
  added/changed/removed), `:216-231` (`change_entry`), `:235` (`reason`), `:247` (`bank_id`). Cap at
  parse like the r7 `VersionLine` caps. Probe: security
  `zz_critic_a_changes_reply_strings_are_kept_whole`.
- **S — `MergeStart.between` is not truncated.** `circle_versions_api.rs:394`; cap at `HISTORY_PAGE`.
- **S+P — the M4 documentation does not exist.** Commit `e977032` claims `picture_sha_on_circle`
  documents the second-re-encode limit; no such text is in the source. Add the real comment at
  `question_history.rs:646-663` (or correct the record).
- **P — the disabled-Show tooltip says untrue things.** `question_history_ui.rs:1149,1154-1157,1486,1170`:
  during Asking it says "reading its version" while the panel shows Yes/Cancel; during Reading it says
  "press Cancel" but there is no Cancel button and Escape does nothing. Fix text/behaviour and update
  `d2m_a_disabled_show_says_wait_for_the_restore`.
- **P — the card hint promises "History in the circle" on unlinked banks.** `bankui.rs:2500` vs
  `question_menu_of`'s early return in `question_history_ui.rs:1686`. Make the hint context-aware.
- **P — "older versions are not shown" uses the bank's `next_before` on a question page.**
  `question_history_ui.rs:1398-1401,417`. Show the line only when this question's own history could
  have older rows, or reword it.

## Rules (unchanged)

Local commits only, never push; no `server/`, no protected files (`src/screens.rs`, `theme.rs`,
`figures*`, `fonts.rs`, `main.rs`, `examples/`); the tree still has three dirty workstreams, so stage
explicitly and never `git add -A`/`commit -a`; commit after each finding; test first (paste the probe,
watch it fail, then fix) and keep it as a regression test; verify `cargo test --lib share` ≥ 1536 + new
and full `cargo test --lib` 0 failures. Report per finding, test counts, skips, commit hashes.
