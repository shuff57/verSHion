# Question editor gauntlet loop

Paste the block below into a fresh Claude Code session opened in this repo, on a new branch off
`shared-pool-phase1` (for example `question-editor-polish`). It runs the six phases of the question
editor rework and loops each one until a blind critic prefers our editor to TestParrot's.

Decisions already made (2026-10-24): edit in a modal, single-column layout, drafts may be saved,
deep restructure allowed (the typing engine in `src/wysiwyg.rs` is in scope), and all four pain
points count: layout, typing feel, click count, math entry.

```
Rework question editing in this repo (src/qedit.rs, src/wysiwyg.rs, the editor call sites in
src/bankui.rs and src/bankops.rs) in six phases, in order. Work on a new branch, never on
shared-pool-phase1 itself, because it holds uncommitted circles work. Leave those files alone.

The bar is the TestParrot question editor. Drive the real site, not a description of it:
  playwright-cli -s=tp open https://www.testparrot.com/question-banks --profile=$HOME/.browser-profiles/tp
No login is needed, data is local to the browser profile. Use the existing empty "Untitled Question
Bank", delete every scratch question afterwards (Delete asks to confirm), and close the session.
What it does: one centered modal; a compact Type / Difficulty / Topics / Points strip above one rich
document of stem and answers; floating toolbar on selection (bold, italic, strike, sub, sup, align,
code, sigma, link); "$x^2$" renders as math as you type; answer rows show radio, up, down, lock;
matching is items beside a word bank; fill-in-the-blank is "select a word, make it a Blank"; Esc
closes without a prompt; Delete is bottom-left with a confirm.

Phases:
0. Baseline, no code changes. Run our app (find out whether web via Trunk or native is the main
   target, and say which you used), edit one question of each of the six types, and write the
   measured defect list to docs/question-editor-baseline.md: caret and selection bugs, IME, click
   counts for common tasks (add an answer, mark correct, switch type, add a topic, insert a
   formula), layout jumps. Decide from it whether wysiwyg.rs is fixed or its model replaced.
1. New shell. Modal editor replaces the expanding card. Single column: metadata strip, stem,
   answers, collapsible Explanation and Image. Footer: Delete left, Cancel and Save right.
   Unsaved-changes prompt on Esc, confirmed delete, Cmd/Ctrl+Enter shown in the footer.
2. Fewer clicks. Enter on the last answer adds one, Tab and Shift+Tab walk the answers, row
   controls appear on hover or focus, type is chosen before the editor opens, switching type keeps
   the stem and compatible answers and warns before discarding, select a word to make a blank.
3. Math. "$...$" converts to rendered math as you type. The sigma button edits the formula inline
   at the caret and replaces the floating equation panel. Use our own equaSHion crates (pinned in
   Cargo.toml, local checkout ~/Documents/GitHub/equaSHion): EquationEditor from equashion-egui,
   its caret-hosting support (edge exit, caret entry, placeholder; see the repo HEAD commit), and
   src/equation.rs and src/equation_text.rs here. Do not write a second math editor. If it needs a
   change in equaSHion itself, stop and ask: that is a separate repo and a new pinned rev.
   Design (checked against both repos, not yet run): equaSHion already has EquationEditor
   .inline(true), .overlay(true) (host draws the equation, widget draws only caret, selection and
   active place), content_size(), inline_baseline(), inline_pad(), display_style(false),
   exit_at_edges(true) with EditorResponse.exit, caret_to(Edge), placeholder(), set_linear() and
   linear(). verSHion uses none of these today, only the dialog form in qedit.rs equation_panel.
   wysiwyg.rs already treats each $..$ as an AtomKind::Math atom, typeset when it parses, and a
   click on one goes to the panel through Out.eq. The work is all on the verSHion side:
   a. Confirm in phase 0 that "$..$" already typesets as you type; count it as work only if not.
   b. Active-atom mode in wysiwyg.rs: one inline, overlay EquationEditor for the clicked or
      entered atom, drawn over atom_rect. Each frame write its linear() back into the text and
      re-lay-out the line.
   c. Caret handoff: Left or Right at an edge (EditorResponse.exit) moves the text caret past the
      atom; arrowing into an atom from the text calls caret_to.
   d. Undo and focus: reconcile the equation editor's undo with wysiwyg.rs's so Cmd/Ctrl+Z is
      unambiguous; map Enter to committed and Esc to cancelled.
   Unverified, possible reasons to ask about an equaSHion change: Backspace or Delete at the edge
   deleting the whole atom (exit covers only arrows), and Up or Down at the edge handing off.
4. Typing engine. Fix or replace the model in wysiwyg.rs per the phase 0 findings so stem and
   answers behave like one document. Highest risk, so it carries the strictest critic.
5. Drafts. Save works at any time. Missing correct answer, empty answers and unbalanced blanks are
   inline warnings on the field. Only adding an incomplete question to an exam is blocked.

For each phase, fan out a builder and a separate critic with fresh context. The critic runs our app
and TestParrot side by side for the same task, blind, says which is easier and clearer to use,
counts clicks and keystrokes for the same task on both, and names the single biggest remaining gap.
Then it goes back to the builder. The critic is harsh; praise is not useful. cargo test must pass
and the existing qedit and wysiwyg tests are updated or replaced, never deleted to go green.
Imported questions without a key, locked answers, pictures and blanks must round-trip unchanged.

/loop on each phase until the critic picks ours blind. Do not stop before that. Commit each
converged phase locally on the branch with a one-paragraph summary. Never push.

Stop and ask me for: a new class of critic problem, a decision in this plan that proves wrong, any
change to the equaSHion repo or its pinned rev, any
edit to protected files (src/screens.rs, theme.rs, figures*, fonts.rs, main.rs, examples/), or a
change to any saved question or bank data format.

Keep a live progress page updating as the work evolves so I can watch it.

Fan out subagents and ultracode.
```

## Notes

- TestParrot is the bar because it is the product this app clones and the one the owner compared against.
  Phase 0 exists so the "deep restructure" of `wysiwyg.rs` is chosen from measured defects.
- `src/figuresui.rs` and `draw_image_row` touch the figures area, which is protected. If the Image
  section move in phase 1 needs edits there, the loop stops and asks.
- Saved data format must not change: banks already on disk and shared-pool circle banks read the same
  question shape. Phase 5 changes when warnings appear, not what is stored.
- Native and web may behave differently in egui (focus, IME). The phase 0 note says which was used;
  the critic uses the same one.
- Standing permissions from the circles loop do not carry over. This loop has its own: local commits
  only, no push, no deploy.
