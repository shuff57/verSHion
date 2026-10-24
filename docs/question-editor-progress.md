# Question editor loop: progress

Worktree: .claude/worktrees/question-editor, branch question-editor-polish (from ee6f015).
Plan and prompt: docs/question-editor-loop.md. Local commits only, never push.

## State

- Current phase: 1 (new shell: modal, single column, metadata strip, footer, unsaved prompt)
- Round: 1
- Target used for measuring: web build (trunk serve, port 8781), driven with playwright-cli session `qed`
- Latest critic biggest gap: n/a (phase 0 has no critic)
- Next: phase 1 builder round, then a blind critic round against TestParrot

## Phase 0 result (done, see docs/question-editor-baseline.md)

- Worst defects: math atoms are raw characters (arrows enter `$..$`); Esc, Cancel or click-outside on the
  equation panel silently discards; type switch deletes all answers with no warning; validation errors
  shift layout 24-28 px under the cursor; equation panel ~650x800 px covers the form, floating toolbar
  duplicates the fixed one and covers Points/Difficulty.
- Phase 4 recommendation: replace the model (one TextEdit per paragraph over a raw string). Keep the pure
  string functions and their tests.
- Unmeasured: double-click word select (harness), IME, and first keystrokes sometimes lost after a click
  or Tab (unconfirmed, needs a native check).
- `$x^2$` already typesets when the closing `$` is typed, so phase 3 item (a) is not work.

## Log

- Round 1, phase 0: baseline agent finished, docs only, no source changes. Committed locally.
