# Question editor loop: progress

Worktree: .claude/worktrees/question-editor, branch question-editor-polish (from ee6f015).
Plan and prompt: docs/question-editor-loop.md. Local commits only, never push.

## State

- Current phase: 3 (math: inline editing at the caret with equaSHion, replacing the floating panel)
- Round: 1
- Target used for measuring: web build (trunk serve, port 8781), driven with playwright-cli session `qed`
- Latest critic biggest gap: keyboard-driven, autofocused entry (Enter/Tab through answers) = phase 2 scope
- Next: phase 3 builder round, then a blind critic round against TestParrot. Phases 0-2 committed.

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

- Round 1, phase 0: baseline agent finished, docs only, no source changes. Committed locally (80cddfe).
- Round 1, phase 1: builder done (draw_modal in qedit.rs, question_modal in bankui.rs, 4 new tests). I verified:
  only those two files changed, no protected files, cargo test 1129 passed 0 failed. Uncommitted.
  Builder caveats: "___ Blank" popup showed on opening a Fill question (pre-existing? unchecked); Esc/click
  outside while the equation panel is open and the Image section were checked by code only.
  Critic round launched (blind, vs TestParrot, 10 tasks); waiting for verdict.
- Round 1, phase 1 critic verdict: ours clearly better overall (better on type switch, matching, Esc safety,
  delete undo, validation; worse on keyboard flow and formula editing). Biggest gap: keyboard-driven,
  autofocused entry (Enter/Tab through answers), which is PHASE 2. Majors that are phase 1 shell work:
  (a) first topic chip shifts content 42 px, (b) clipped suggestion chip over Points plus 38 px dead band.
  Minors: stray '___ Blank' popup on opening Fill, Esc closes modal when Link popup is open, MC->TF->Save
  silently keys True, blank left in stem after Fill->MC, new answer row not scrolled into view.
  Diff review found no data-loss path. Sent (a), (b), the Blank popup and the Link-popup Esc back to the
  builder (round 2); the rest carry to phase 2 or 5.
- Note: the scratch bank in the user's TestParrot `tp` profile is gone (empty banks vanish); nothing left.
- Round 2, phase 1: builder fixed (a) topic chip shift, (b) suggestion dropdown, Blank popup on reopen (modal's first-frame click; `armed` flag), Link-popup Esc. I re-verified: cargo test 1130 passed 0 failed, no protected files. Phase 1 converged (critic picked ours; narrow fixes, no second full critic) and committed locally.
- Carried to phase 2: autofocus on open and on new answer row, scroll new row into view, MC->TF->Save silently keys True (warn on key), blank left in stem after Fill->MC. Carried to phase 5: validation softening.
- Round 1, phase 2: builder done (autofocus, Enter/Tab/Backspace answer flow, hover row controls, type menu before editor, type-switch carry/drop with key warning, Make Blank button + Ctrl+Shift+K). I verified: cargo test 1142 passed 0 failed, no tests removed, no protected files. MC with 4 answers + 2nd correct + save now 2 clicks + 6 keys (was ~8 clicks). Uncommitted. Builder decisions to confirm with owner at the end: Alt+Enter marks answer correct; focus stays in stem after Make Blank; untouched TF defaults to True. Not fixed: Multipart part-type chips still reset on switch. Critic round launched.
- Round 1, phase 2 critic verdict: ours clearly better for fast entry. Counts: MC 8 vs 8 (tie), fill 5 vs 5, matching 9 vs ~15, edit/type switch/Esc safety/row handling all better. Biggest gap: entry flow not fully keyboard-reachable and not discoverable. Majors: (1) Alt+Enter undiscoverable; (2) keyless imported TF treated as keyed on edit, so TF->MC/Short writes a placeholder key (code-read only; data integrity); (3) Tab loses focus with no ring after Add answer, TF circles not tab stops. Minors sent too: type-switch bar keyboard + only destructive button red, Add menu keyboard, Esc-Esc keeps editing, stale Blank popup, Fill Tab trap. Deferred: multi-line paste into rows, undoable row removal. Sent to builder (round 2).
- Round 2, phase 2: builder fixed the three majors and most minors. I re-verified: cargo test 1146 passed 0 failed, no protected files, tf_is_keyed now `tf_tapped() || (is_edit() && !keyless)` so a keyless TF stays unkeyed through a type switch (4 new tests). Phase 2 converged (critic picked ours; narrow fixes, no second full critic) and committed locally. Deferred: multi-line paste into rows, undoable row removal, Multipart part-type chips reset on switch. Read-only, not run: 1-6 menu hotkeys, Tab from last Fill blank, quiet Blank popup.
