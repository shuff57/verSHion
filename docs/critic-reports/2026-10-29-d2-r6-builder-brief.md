# D2 client round 6 — builder brief (2026-10-29)

This is the round-6 build brief for the circles D2 **client** loop (per-question history, compare,
restore). It supersedes the "tree is clean" line in `HANDOFF.md`: see **Dirty tree** below.

## Where things are

- Repo: `/home/shuff57/Documents/GitHub/verSHion`, branch `shared-pool-phase1`.
- HEAD `61afbb9` is docs only; the code base to build on is r5 `80dd3d1`.
- Baseline: share suite **1504 passing** at `80dd3d1`; full `cargo test --lib` 2108 pass / 0 fail / 5 ignored.
- Round-4 critic findings are summarized in `HANDOFF.md` under "Phase D2 client" (N1–N5 + should-fix).
  The probe tests that demonstrate them survived in the previous session's scratchpad (paths below).

## Hard rules (unchanged from the loop)

- **Local commits only. Never push.**
- Do not touch `server/`.
- Protected files: `src/screens.rs`, `src/theme.rs`, `src/figures*`, `src/fonts.rs`, `src/main.rs`,
  `examples/`. Stop and report if a fix genuinely needs one.
- Stop and report on a new class of critic finding, a plan decision that proves wrong, staging/deploy,
  or privacy/terms wording.
- **Commit after each finding (WIP commits are fine).** The reason r6 was lost is that the builder hit
  the 5-hour limit with nothing committed. A limit kill must never cost more than one finding.

## Dirty tree — do not sweep other workstreams into commits

The working tree is **not** clean at `80dd3d1`. Three workstreams are uncommitted. Stage paths
explicitly; never `git add -A` / `git commit -a`.

**Leave alone (other workstreams):**

| Files | Workstream |
|---|---|
| `src/gradeui.rs`, `src/model.rs`, `src/store.rs`, `src/ai.rs`, `src/app.rs`, `src/lib.rs`, `src/screens.rs`, `Cargo.toml`, `Cargo.lock`, `prompts/handwriting.md`, `parity/`, `docs/grade-*.md`, `docs/critic-reports/grade-*`, `tenshion-diagnostic-*.zip` | Grade / handwriting / viSHion |
| `src/figures/mod.rs` (a params clamp) | Figures loop |
| `docs/question-editor-loop.md`, `docs/figures-loop.md`, `docs/handoff-ai-worker-harness.md` | Other loops |

**D2 files you may stage:** `src/share/**`, and `src/bankui.rs`.

`src/bankui.rs` already carries an **uncommitted D2 layout hunk** (strip scoped to the page width,
long breadcrumb names truncated, rename-pencil width). It is not Grade work. Verify it builds and
belongs to r6, keep it, and include it in an r6 commit. Staging `src/bankui.rs` stages only that hunk.

## Test first

The round-4 probes are complete source copies in the previous session scratchpad:

```
SP=/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/63f2b0d4-500a-4288-83db-19bdb8133e9a/scratchpad
```

| Probe | File:line | Finding |
|---|---|---|
| `zz_critic_restore_base_over_stem_and_figure_edit_is_falsely_refused` | `$SP/copy6/src/share/question_history_tests.rs:1088` | N1 |
| `zz_critic_not_countable_bypass_via_a_deleted_question` | `copy6/.../question_history_tests.rs:1140` | N1 |
| `zz_critic_figure_only_restore_bypass_when_copy_figure_already_differs` | `copy6/.../question_history_tests.rs:1104` | N1 |
| `zz_critic_topics_order_only_restore` | `copy6/.../question_history_tests.rs:1123` | N1 (minor) |
| `zcrit_a_circle_picture_the_store_keeps_as_webp_reads_as_a_different_picture` | `$SP/copy7/src/share/question_history_tests.rs:1090` | N2 |
| `zz_critic_restore_lands_in_the_first_frame_after_leaving_and_its_result_and_undo_are_lost` | `copy6/.../question_history_ui_tests.rs:2185` | N3 |
| `zcrit_a_restore_lands_in_the_first_frame_on_another_linked_bank_and_its_result_is_never_shown` | `copy7/.../question_history_ui_tests.rs:2187` | N3 |
| `zz_critic_restore_lands_while_another_linked_banks_page_is_drawn_with_its_draft_flag` | `copy6/.../question_history_ui_tests.rs:2215` | N3 |
| `zcrit_undo_vanishes_on_the_page_when_the_bank_turns_on_hiding` | `copy7/.../question_history_ui_tests.rs:2223` | N4 |
| (no probe exists) | — | N5 — write one |

Paste each probe into the matching real test file, **run it and watch it fail**, then fix the code
until it passes. Keep the probes as regression tests (rename `zz_`/`zcrit_` to the repo's naming
if that is the local convention; keep the assertions verbatim).

## Findings to fix

### N1 — `NotCountable` is bypassed / falsely raised
- Restoring the **base version itself** over a stem **and** figure-settings edit is refused as
  `NotCountable`, although the result is exactly that version ("in step").
- A locally **DELETED** question lets a figure-only difference slip in through `NotCountable`.
- Figure-only refusal must still hold; a topics-order-only difference should not be miscounted.
- Root cause is in the counting in `src/share/question_history.rs` (`NotCountable`, `changes_since`,
  the in-step comparison). Count against the base version and the copy consistently; do not special-case
  "deleted" by skipping the comparison.

### N2 — picture re-encode reads as a different picture
A circle picture the local store re-encodes (PNG/JPEG → WebP, new file name) compares as "a different
picture" on a teammate's copy, so compare, the header and "already the same" are wrong, while the strip
count is right. Compare picture identity by the content the file decodes to / the circle's hash — the
mechanism `changes_since` already uses — not by the local file name. Probe:
`zcrit_a_circle_picture_the_store_keeps_as_webp_reads_as_a_different_picture`.

### N3 — a confirmed restore can be applied unseen in the first frame after leaving the page
`CircleEditModel::sync` also runs from the Shared page and other banks' strips. If a restore is
confirmed and the user leaves, the restore lands in the first frame on another page and its
Done/Undo result is never shown. Gate the sync's restore application on the page that owns the ask
(or surface the result globally). Probes: the three `.../question_history_ui_tests.rs` first-frame
tests above.

### N4 — Undo is dropped on a role/hide change while still on the page
Keep the pending Undo (and its message) across a role or hide-state change while the page stays open;
only the cache may be scoped/dropped. Probe:
`zcrit_undo_vanishes_on_the_page_when_the_bank_turns_on_hiding`.

### N5 — the difficulty pass can overwrite a restored question's difficulty
A background difficulty pass must not write over a question a restore just set (the restore's value
wins until the user changes it). Write a test first; no probe exists.

### Should fix (round-4 "should fix" list)
1. `Show` disabled while a restore is being asked.
2. Cap `by` / `message` length at parse (untrusted `/changes` and version replies).
3. No same side on both compare pickers.
4. Dedupe restore points.
5. Report a keyboard route for hook H3 (the `question_menu` line in `src/bankui.rs`).

## Acceptance

- All probes above pass; new tests for N5 and each should-fix item where behaviour is testable.
- `cargo test --lib share` ≥ **1504 + new tests**, 0 failures.
- Full `cargo test --lib`: 0 failures (5 known-ignored only).
- Commits: one per finding or coherent group, all local, message prefix
  `circles D2 client r6: ...`.
- Report back: what changed, test counts, anything skipped and why, and whether the `bankui.rs` layout
  hunk was kept or dropped.

## Then

A fifth critic pair (product vs Moodle, security vs ASVS) runs on the resulting build. Loop exit:
both clean on the same build.
