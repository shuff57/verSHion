# Handoff: figures gap-fill loop, figure 1.3 round 3 committed

Written 2026-10-27 ~19:40 PT by the circles/D1 session, which the owner asked to take over and "finish both".

## Where things are
- Worktree `/home/shuff57/Documents/GitHub/verSHion-figures`, branch `figures-gapfill` (based on `figures-circuits`). HEAD `b706974` (the four uncritiqued figures' blocker and major fixes). **Phase 2 is complete.** **Math core (1.1-1.5) is complete; phase 2 has started.**
- Do not touch the main repo `/home/shuff57/Documents/GitHub/verSHion` (branch `shared-pool-phase1`): it holds the circles D1 work, committed locally, waiting on critics 101/102.
- Loop spec: `docs/figures-loop.md`. Rules: `docs/figures-conventions.md`. Progress and critic minors: `docs/figures-progress.md`. Memory: `figures-gapfill-loop.md`.
- Done and committed: phase 0, 1.1 conics (`b009d82`), 1.2 circle (`4f5715d`, closed), 1.3 rounds 1-3 (`c3eb729`, `0f81c30`, `2cc0a58`).
- Standing rules: local commits only, never push, no deploy. Protected: `src/screens.rs`, `theme.rs`, `fonts.rs`, `main.rs`, `examples/`. Stop and ask for saved-format changes, new dependencies, or privacy wording.

## Figure 1.3 right-triangle-trig, status: built, awaiting a verification critic
- Round 1 critic FAILED (`c3eb729`): fraction leaked an Ask side, 10-degree arc unreadable.
- Round 2 (`0f81c30`) fixed both. Round-2 combined critic (`docs/critic-reports/2026-10-27-figures-1.3-critic-r2.md`) found ONE major: at 0 decimal places the printed sides were no longer a right triangle (5-9-10) and `cos theta = 9/10 = 0.87` did not hold. It accepted a round-3 fix.
- Round 3 (`2cc0a58`) is that fix: lengths and fraction operands print at >= 2 dp always, the printed decimal is computed from the printed lengths, theta keeps `dec`; a sweep (5 define modes x whole/non-integer x dec 0-4 x key/student) asserts every printed `x/y = z`, every printed right triangle, every printed length. FAQ + parameter hint now say what Hidden does; progress doc has a Round 3 paragraph, chord 6.6 px, 52 census files, six examples.
- Suite on `2cc0a58`: **1635 passed, 0 failed, 5 ignored** (lib), all integration targets ok, exit 0. Census unchanged (`census_baseline` ok, no fixture diff).
- Open minors from the r2 critic (not failing, in `docs/figures-progress.md`): theta label 2 px below the base at 10-20 degrees; drag can trip the 0.1 snap and raise a spurious clamp note; visible gap before the degree sign in the blank; Large labels push `adjacent` far below the base.

## Next steps
1. **Verification critic on `2cc0a58`** — blocked by the weekly agent limit until **2026-10-28 16:00 PT**. Run ONE combined critic in a `git archive 2cc0a58` copy with its own `CARGO_TARGET_DIR`; if only minors remain, log them, write the report to `docs/critic-reports/`, close 1.3.
2. **1.4 sine/cosine markers** (amplitude, period, midline, phase presets) and **1.5 unit circle** (degrees or radians, coordinate triple) are the next figures — both still unbuilt (checked against `figures-circuits`). Then phases 2-6 in `docs/figures-loop.md`.
3. Re-read `docs/figures-conventions.md` before a new figure: labels, Print neutrality, student/key parity, PNG inspection in Colour/Print/student/66% bitonal (`VERSHION_PNG_DIR=<dir>`).

## Figure 1.4 sine-cosine, status: built, awaiting its first critic
- New module `src/figures/sincos.rs`, id `sine-cosine`: y = A sin(B(x - C)) + D on a degree axis (B = 360/period), a
  start preset that picks sin/cos/negatives, a folded phase shift, and amplitude/period/midline markers.
- Registered in `mod.rs` (defs, notes_for, examples_for), added to `NO_HANDLES` (no handles), FAQ arm in `faq.rs`,
  five examples, nine tests and a PNG export.
- Census: only `sine-cosine__defaults` and `sine-cosine__start` added. The hashes were blessed twice (once before
  the layout fix); re-bless after any render change with
  `VERSHION_CENSUS_BLESS=1 cargo test --lib -j2 census_baseline` (debug is fine: the diff must show only the two
  new ids, never a moved existing line).
- Three defects were found only by looking at the PNGs, not by tests: the blanked period label lost its degree
  sign, `1,080` disagreed with the axis's `1000` (use `chart::fmt`, not `num`, for a period), and the readout rows
  overlapped at Large labels because row spacing did not scale. Canvas height is now
  `ROW_Y0 + 4 * ROW_H * label_factor + 4`.

## Figure 1.5 unit-circle, status: built, awaiting its first critic
- `src/figures/unitcircle.rs`, id `unit-circle`: radius-1 circle, point at theta (typed or dragged), radius, dashed
  drops to both axes, centre angle mark, readout of the coordinate triple (angle, cos x, sin y, P) with
  Given/Ask/Hidden. Radians as a pi fraction when the denominator is at most 12, otherwise a decimal.
- It has a real drag handle, so it is NOT in `NO_HANDLES`; the dispatch is in `math.rs` `handles`.
- Labels use a candidate-placement search (`put` with a candidate list) that keeps them on the canvas and off every
  label already placed, axis numbers included. `labels_never_overlap_or_leave_the_canvas` sweeps every second angle;
  it caught the x value riding on the -1 tick and the P label leaving the left edge.
- Tests: 10 (defaults, pi fractions, radians row, student/key, point position, drag, Hidden, label sweep, Print
  neutrality, PNG export). Census: only `unit-circle__defaults` / `__start`.

## What is left in the figures loop
- Critic runs for 1.3 round 3, 1.4 and 1.5 (all blocked until 2026-10-28 16:00 PT by the agent quota).
- Phase 2 (math rest). Done: `transformations` (`e7eac7c`), `fraction-models` (`ed8f6b2`), `triangle-centres` (`4786301`), `slope-triangle` (`0f8a07a`). Phase-2 gaps still missing, checked against the 141
  registered ids: fraction and number models (bars, circles, tape diagram, area model, base-ten blocks, ten-frame),
  triangle centres, slope triangle, protractor, nets, and explicit bar/pie/line chart figures. Venn, probability
  tree, histogram, scatter, dot plot, stem-and-leaf and the solids already exist.
- Then phases 3-6 (physics, biology, chemistry, stats) in `docs/figures-loop.md`.

## Pitfalls
- Builders sometimes stop without reporting: ping them with SendMessage.
- The scratchpad can be wiped mid-session. Rebuild the baseline census from `git archive 0c1a8a8` if needed.
- **The agent limit gate is lifted** (the owner lifted it on 2026-10-27 evening; the circles critics ran that night).
  Agent builders and critics can be spawned now.
- **Critic round 1 is done** (2026-10-28): two combined critics over all seven new figures, both FAIL; every
  blocker and major fixed in `e72a4bc`; full disposition in `docs/critic-reports/2026-10-28-figures-critics-2.md`.
  Suite on the fix commit: 1685 passed, 0 failed, 5 ignored. **The fixes themselves have not been re-critiqued.**
- Phase 2 is complete (all committed): transformations, fraction-models (bar, circle, tape, ten-frame, area model),
  triangle-centres, slope-triangle, protractor, base-ten-blocks, solid-net. Next is phase 3 (physics).
- **Phase 3 inventory (2026-10-28, checked against the 141 registered ids):** almost nothing exists. Present:
  `waves` (generic), `circuit-diagram`, `motion-graphs`, `projectile-motion`, `free-body`, `inclined-plane`,
  `pulley`, `spring-scale`, `vector-diagram`, `logic-circuit`, `kirchhoff-loops`, `rc-curve`. Missing from the
  phase-3 list: energy bar charts, collisions and momentum, circular motion, electric field lines and
  equipotentials, refraction/Snell/total internal reflection, magnetic field and right-hand rule, standing waves and
  interference, PV diagrams, a ray diagram with handles, a capacitor, series-parallel circuits, and the handles /
  hide-reading work on the existing physics figures.
- Suite on `2f9924a`: 1716 passed, 0 failed, 5 ignored.
- **All eleven figures have now been critiqued.** The last four (protractor, area model, base-ten blocks, solid-net)
  were failed on 2 blockers and 8 majors, fixed in `b706974`, and re-checked by a verification critic: the blockers
  and all but one major hold; the leftovers were fixed in the commit after it. See
  `docs/critic-reports/2026-10-28-figures-critics-3.md`.
- The five newest figures (1.4, 1.5, transformations, fraction models, triangle centres, slope) have had two
  critic rounds; the protractor has had none yet.
- `HANDOFF.md` here is untracked; do not commit it with the figure work.
- Round 2's handoff claimed round 2 was uncommitted and HEAD was `c3eb729`; it was stale. Check `git log` before trusting a handoff.
