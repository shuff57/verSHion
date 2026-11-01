# Figures gap-fill: what is left after we stop (written 2026-11-01)

Branch `figures-gapfill` in the worktree `/home/shuff57/Documents/GitHub/verSHion-figures`. Spec `docs/figures-loop.md`, live state `docs/figures-progress.md`, critic reports `docs/critic-reports/`. Nothing here has been pushed or deployed.

## 0. Done (all local commits on `figures-gapfill`, worktree HEAD `db599c3` at the time of stopping)

- Phases 0-3 complete and verified (engine groundwork, math core, math rest, physics).
- Phase 4 biology converged, each passed a fresh verification critic: hardy-weinberg, chi-square-fit, enzyme-kinetics, cladogram, food-web, energy-pyramid, action-potential (PASS critic 4, `c1e47bb`), cell-cycle (PASS critic 4, `ad36770`).
- Phase 5 chemistry, by the other session: reaction energy diagram, reaction rate graph, phase diagram, solubility curves, gas-law graphs, equilibrium graphs, buffer curves are committed (some with critic rounds still open; see `docs/figures-progress.md`).
- Phase 4b started: photosynthesis (`49809e6`) and soil texture (`db599c3`) are in the worktree but NOT yet passed.

## 1. Figures in flight when we stopped (the agents were stopped; nothing is lost on disk)

| Figure | State | Next step |
|---|---|---|
| `cell-diagram` (phase 4, last biology figure) | In the worktree as `b587bcb`. Critic 1 FAILED (1 major, 4 MED, 4 LOW, 9 minor; report `2026-11-01-cell-diagram-critic-1.md`, `aa9562e`). The round-2 builder was stopped with UNCOMMITTED edits to `celldiagram.rs` and `faq.rs` in its clone `scratchpad/cd1` (it had 44 mutants running). | Start a builder (or resume agent `a3df602f366195da4` if still available) from that clone: finish round 2, commit, cherry-pick, fresh critic, repeat until PASS. The edits may be incomplete; check `git diff` in the clone first. |
| `photosynthesis` (phase 4b) | In the worktree as `49809e6` (builder's original `7de962f`, merge with soil-texture resolved in five shared files, compile checked). Critic 1 was running and was STOPPED before reporting: no verdict. The builder's full suite had not been re-run after its FAQ fix. | Launch a fresh opus critic on the current commit (brief template: see the soil-texture critic prompts in the session transcript; it must verify the biology against sources, pointer rings, leaks, Print 66 % grey, refusals, and run `figures::`). |
| `soil-texture` (phase 4b) | Critic 1 FAILED (1 blocker, 1 major, 3 MED; `49e4e66`). Round 2 builder commit `2342a34` is in the worktree as `db599c3` (Manual cascade classifier, USDA orientation, label/guide placement, Print guides, sum tolerance 99.5-100.5, show_worked option). Critic 2 was running and was STOPPED before reporting: no verdict on round 2. | Fresh verification critic on `db599c3`; it must compare the drawn regions with the Manual's fig 3-7 (tools from critic 1 are in `scratchpad/scrit1-tools/`). Open: 17 label-on-guide cases at Large, first draw 308 ms debug, survivor n22 (Print guide `#999999`). |

Stray untracked file in the worktree: `src/figures/faq.rs.orig` (not mine on purpose; a merge-tool leftover; look before committing or delete). `HANDOFF.md` is another session's uncommitted file: never overwrite.

Method that works (see memory `figures-gapfill-loop`): one sonnet builder in its own `git clone` under the scratchpad; I inspect its commit and cherry-pick it into the worktree (when the other session has uncommitted census/figure edits use `git format-patch`, `git apply --index --exclude=<census>`, `git apply --cached` for the census patch, sed the two hashes in the working file); a fresh opus critic on the exact commit; findings go back to the SAME builder; the critic's report is committed as a docs commit. Shared one-line files (`mod.rs` `notes_for`, `faq.rs`, `biology.rs`, `start.rs`, the census) conflict whenever two figures land: keep both sides (a union script; the FAQ needs the missing `];` added). Always `cd X && ...`, verify `git rev-parse --show-toplevel` before any reset, never run anything in the main checkout. Critics should keep their scratch files in their own subdirectory (another agent overwrote a shared `mutate.py` twice) and may not be allowed to write a report file: take the report from their final answer.

## 2. Figures not started

- Phase 4b plant science: **leaf cross-section** (labelled layers, stomata and guard cells open or closed, student copy with blanks), **xylem and phloem** (stem and root cross-section, direction of water and sugar transport).
- Phase 5 chemistry (the other session is doing these in the same worktree; 7 of 10 committed: reaction energy, reaction rate graph, phase diagram, solubility curves, gas-law graphs, equilibrium graphs, buffer curves): still to come **Boltzmann distribution, periodic-table excerpt, VSEPR 3-D geometry**, plus whatever critic rounds are still open on the committed ones. Check `git log` and `docs/figures-progress.md` before starting anything there; do not build the same figure twice.
- Phase 6 stats (7): bar chart for categories, frequency table, cumulative/ogive, residual plot and other regression models, t and chi-square distributions, back-to-back stem-and-leaf, handles for the six stats figures.
- Lower priority, not in the phases: pendulum/SHM, torque, orbit, Doppler, photoelectric, nuclear decay, DNA/translation, mitosis/meiosis, osmosis, electrochemical cell, calorimetry, Born-Haber, skeletal structures, lab apparatus, IR/NMR, polar/parametric graphs, vectors on the grid.

## 3. Recorded limits worth a later look (details in `docs/figures-progress.md`)

- Action potential (PASS, critic 4): timing advice at Large offers a value the lead check refuses; "every dot pair 24 px" is enforced only for the return dot in a failed initiation; minimum lead pinned only to about 0.3 px; Large -100/-90/70 draws -170..110 with a "-150" label; `seg_cross` local copy.
- Cell cycle (PASS, critic 4): arc-reason check still carries the end marks; one surviving mutant (`caps_exempt_all_ticks`); student word allowlist still holds old words; the student copy does not say shares may not add to 100.0; no prometaphase.
- Energy pyramid (PASS): long caption on the default figure; two unequal slivers can look equal; negative `-1e-400` message; spaces between digits joined.
- Latent bug seen, not fixed (`mod.rs` belongs to a protected area for this loop's other session): `figures::svg_size` slices the first 400 bytes of the SVG, so a multi-byte character straddling byte 400 panics.
- Not checked for any figure: the live egui editor and handle dragging, browser font metrics, PDF/DOCX export, release-build timing.

## 4. Merge, rebuild, redeploy (requested by the owner 2026-11-01, NOT done, needs decisions)

The owner asked to finish the running figures, then commit, merge to `main`, rebuild and redeploy. The standing rule until then was "local commits only, never push or deploy"; this request is the owner's explicit instruction, but the details below are open:

- `figures-gapfill` is 162 commits ahead of `main` (merge base `0c1a8a89`, main is at `fd3b8d0`, 2026-10-25) and sits on top of `figures-circuits` (44 commits of figure work never merged to main). A merge to main brings all of it.
- The main checkout `/home/shuff57/Documents/GitHub/verSHion` is used by other sessions (branch `shared-pool-phase1`, uncommitted work). Do not check out or merge there; do the merge from a separate clone or worktree, or let the owner do it.
- Which commits go: the other session's phase 5 chemistry commits are interleaved on this branch, so they go too unless split out.
- What "redeploy" means is not recorded anywhere I could find: `README.md` and `NOTES.md` have no deploy section. The repo has `Trunk.toml` and `index.html` (a web build) and `server/worker/wrangler*.toml` (Cloudflare worker, `server/` is off-limits for the figures loop). Ask which target and which command.
- Before any merge: the full lib suite on the merged tree (`systemd-run --user --scope -p MemoryMax=8G -p MemorySwapMax=0 cargo test --lib -j2`, known flake `absurd_windows_cannot_run_away`), then a release build.
- Never type or store credentials; deploy auth must come from the owner.
