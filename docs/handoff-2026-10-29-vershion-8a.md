# Handoff 2026-10-29: session vershion-8a (circles D2 r10, figures phase 4 biology)

Written instead of overwriting `HANDOFF.md`, which is tracked and holds the circles loop's 519-line log. Nothing here is committed. Local commits only, never push, no deploy.

## Two repos, two owners

| | Main repo `verSHion` | Figures worktree `verSHion-figures` |
|---|---|---|
| Branch | `shared-pool-phase1` | `figures-gapfill` |
| Work | circles D2 client | figures gap-fill, phase 4 (biology) |
| Never touch from the other | yes | yes |

The main repo tree carries uncommitted **grade work from another session** (`src/gradeui.rs`, `src/app.rs`, `src/screens.rs`, `parity/`, `docs/critic-reports/grade-*`, and others). Leave it alone. Stage by name, never `git add -A`.

## Circles D2 client (main repo)

- r10 is `aac85dd`. It fixed the three product-critic findings from r9:
  - A merge no longer wipes the picture records. `picture_files_kept` in `src/share/circle_links.rs` keeps a name while some bank question still has the file and its base record does not name it. It is judged per question, so two questions sharing one file are handled.
  - `BaseQuestion.figure_recorded` is a three-state record, so "base had no figure" is no longer confused with "not recorded". The `restore_question` guard in `src/share/question_history.rs` reads it.
  - Three `d2p_` probes in `src/share/question_history_tests.rs` replace `d2o_a_merge_clears_the_picture_records`.
- Lib suite at r10: 2248 passed.
- Other sessions have since gone on to r15 and r16 (`13ac7b9` is HEAD). Read `HANDOFF.md` in the main repo for that state.
- **Open:** the r9 security critic's findings never reached this session. Check whether r11 onward already folded them in before acting.

## Figures gap-fill (worktree `/home/shuff57/Documents/GitHub/verSHion-figures`)

Loop spec: `docs/figures-loop.md`. Live state: `docs/figures-progress.md` and `HANDOFF.md` there (`HANDOFF.md` is never committed). Memory: `figures-gapfill-loop.md`.

- Phases 0-3 done and verified. Phase 4 has these figures, all committed:
  - hardy-weinberg, chi-square-fit, enzyme-kinetics, cladogram (my work, through `57a8fe0`).
  - food-web (`39ae4f6`, another session's work).
- Four verification passes on my batch found 6, 5, 4, then 2 majors, with no blocker in the last two. A fifth pass on `57a8fe0` was offered and not run.
- Suite at `57a8fe0`: 1897 passed, 0 failed, 5 ignored. `absurd_windows_cannot_run_away` and `dragging_a_label_moves_it_as_one_undo_step` are timing flakes under CPU contention. Re-run them alone before calling them failures.
- **The worktree is shared.** Right now it has uncommitted changes from the other session: `foodweb.rs`, `faq.rs`, `mod.rs`, the census fixture, `docs/figures-progress.md`, `HANDOFF.md`. Run `git status` first. Do not commit those, and do not re-bless the census over their lines.
- Protected there: `src/screens.rs`, `theme.rs`, `fonts.rs`, `main.rs`, `examples/`.

## How to run a round

- Tests: `systemd-run --user --scope -p MemoryMax=8G -p MemorySwapMax=0 cargo test -j2`. Default jobs run out of memory.
- Re-bless the census with `VERSHION_CENSUS_BLESS=1 cargo test --lib -j2 census_baseline`. The fixture diff must show only the intended ids.
- Render a figure headless: `VERSHION_RENDER_FIG=<id> VERSHION_FIG_PARAMS='{...}' VERSHION_OUT=<png> ./target/debug/vershion`.
- Critics run on a frozen `git archive` copy with their own `CARGO_TARGET_DIR`.
- Do not pipe a backgrounded command through `tail`. Redirect to a log file and write an `exit=$?` line at the end.
- `pkill -f "cargo test"` kills the shell (exit 144).

## Lessons that earned their keep

- Passing tests say nothing about legibility. Render the PNGs. Rendering found the cladogram's rows being drawn along the parent, a parser that ate spaces, and a promised ancestor label the renderer never drew.
- A fix right for the reported case is usually wrong for the neighbouring one. Test the whole family (every pair of labels, every edge), then render it.

## Open product question

The enzyme drag handle still sits at the answer point on screen while Show answers is off. The exported student copy is clean. Decide whether the on-screen handle should hide too.

## Next steps, in order

1. `git status` in the figures worktree. Find out whether the food-web session is finished.
2. Optionally run a fifth verification pass on `57a8fe0`.
3. Phase 4 remaining: action potential, cell cycle, energy pyramid, cell diagrams. Then phase 5 chemistry (10 figures) and phase 6 stats (7 figures).
4. Circles: confirm the r9 security findings were handled.

## Standing stop rules

Stop and ask for: a saved-format change, a new dependency, privacy or terms wording, staging or deploy, or any file listed as protected. End commit messages with the attribution line the session gives.
