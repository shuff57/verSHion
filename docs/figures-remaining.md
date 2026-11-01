# Figures gap-fill: what is left after we stop (written 2026-11-01)

Branch `figures-gapfill` in the worktree `/home/shuff57/Documents/GitHub/verSHion-figures`. Spec `docs/figures-loop.md`, live state `docs/figures-progress.md`, critic reports `docs/critic-reports/`. Nothing here has been pushed or deployed.

## 1. Figures in flight when we stopped (finish these first)

| Figure | State | Next step |
|---|---|---|
| `cell-diagram` (phase 4, last biology figure) | Critic 1 FAILED (1 major, 4 MED, 4 LOW, 9 minor; report `2026-11-01-cell-diagram-critic-1.md`). Round-2 builder was running its mutation tests, not yet committed. | Cherry-pick the builder's round-2 commit, fresh verification critic, repeat until PASS. |
| `photosynthesis` (phase 4b) | Built (`49809e6` in the worktree), critic 1 running. | Commit the critic's report, send findings to the same builder, verify, repeat until PASS. |
| `soil-texture` (phase 4b) | Critic 1 FAILED (1 blocker, 1 major, 3 MED; report `2026-11-01-soil-texture-critic-1.md`): on-line points classified against the Soil Survey Manual, triangle mirrored against the USDA one, labels over guides, Print guides lost. Round-2 builder running. | Same loop. Check every polygon vertex against the Manual again in the next critic. |

Method that works (see memory `figures-gapfill-loop`): one sonnet builder in its own `git clone` under the scratchpad; I inspect its commit and cherry-pick it into the worktree (when the other session has uncommitted census/figure edits use `git format-patch`, `git apply --index --exclude=<census>`, `git apply --cached` for the census patch, sed the two hashes in the working file); a fresh opus critic on the exact commit; findings go back to the SAME builder; the critic's report is committed as a docs commit. Shared one-line files (`mod.rs` `notes_for`, `faq.rs`, `biology.rs`, `start.rs`, the census) conflict whenever two figures land: keep both sides.

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
