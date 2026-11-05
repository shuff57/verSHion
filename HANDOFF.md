# Handoff: figures gap-fill loop, phase 3 critic round 8 fixed

Written 2026-10-27 ~19:40 PT by the circles/D1 session, which the owner asked to take over and "finish both".
Updated 2026-10-28 late: the five-figure phase-3 critic round is fixed and committed.

## Where things are
- Worktree `/home/shuff57/Documents/GitHub/verSHion-figures`, branch `figures-gapfill` (based on `figures-circuits`). HEAD `21e5896` (phase 3: the five-figure critic's minors). **Math core, phase 2 and eight phase-3 figures are built; the five newest physics figures have had a combined critic round and a fix, and a verification critic is running on the fix commit.**
- Do not touch the main repo `/home/shuff57/Documents/GitHub/verSHion` (branch `shared-pool-phase1`): another session owns `src/share/` there. **The circles D1 critics 101/102 gate is closed** — their findings were fixed in `fb1b7e9` and re-checked in `cc49eed`, and that repo has since moved through D3 and D2 (`dce14de`). Nothing on the circles side is waiting on this worktree.
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

## Phase-3 critic round 8 (2026-10-28, the five newest physics figures)
- Combined critic over `electric-field`, `magnetic-field`, `standing-wave`, `pv-diagram` and `refraction` at `cfd89f0`:
  **FAIL — 2 blockers, 6 majors, 9 minors.** Both blockers were student-copy leaks through drawn geometry: PV printed
  the two asked pressures on the y axis and shaded the work; standing drew the node/antinode markers beside the blanked
  counts. Neither was visible to any test.
- Fixed in `8f40aff` (blockers and majors) and `21e5896` (the minors worth doing). Census re-blessed twice, each time
  only the intended ids: `electric-field`, `magnetic-field`, `pv-diagram` (six lines), then `magnetic-field` and
  `standing-wave` (four lines, from the micro-sign text and the antinode-ring side).
- Full disposition and the list left open: `docs/critic-reports/2026-10-28-figures-critics-8.md`.
- Suite on `8f40aff`: **1775 passed, 0 failed, 5 ignored**. The minors commit has its module tests green (36) and the
  census checked, but **the full suite has not been run on `21e5896`** — it is deliberately deferred until the
  verification critic finishes, because `figuresui::tests::dragging_a_label_moves_it_as_one_undo_step` fails under the
  CPU contention of two concurrent full runs. Run it once the critic is done.
- **A verification critic on `8f40aff` came back: 7 of 8 FIXED, 1 PARTLY** (the PV title was still crossed by a
  seven-digit pressure label at every label size, from the reachable V1 = 1 L), plus 3 defects the fixes themselves
  introduced: the PV endpoint number could sit against the top edge; the electric 40 px distance floor made 0.1 m and
  1.2 m draw identically; the magnetic inner arrowhead was longer than its chord. All four fixed in the commit the
  verification produced — see `docs/critic-reports/2026-10-28-figures-verify-8f40aff.md`. **The lesson worth keeping:
  the first fix aimed at the symptom (make the margin grow with the label size) instead of at the cause (measure the
  widest label the axis will print).**

## Circuit diagram critic round (2026-10-29)
- Combined critic on `9addc16`: **FAIL — 2 blockers, 7 majors, 11 minors**, all arithmetic verified correct. Fixed in
  the commit after; disposition in `docs/critic-reports/2026-10-29-circuit-diagram-critic.md`.
- The two blockers: a combination voltmeter drawn at y = −2 (the "above the wire" placement is series-only), and a
  regression I introduced — adding `F` to the shared parser let resistor mode read `470 µF` as 470 µΩ. The parser is
  now unit-parameterised (`parse_ohm` / `parse_cap` / `parse_qty`).
- **Lesson worth keeping: a parser shared by two quantities needs its units passed in, not widened.** Widening
  `parse_qty` to accept farads looked additive and silently changed what a resistor value meant.

## What is left in the figures loop
- **Spring-scale hide-reading: done** (`82846df`) — a new `answer_text` caption, so the worksheet copy can read
  `Force: ________ N` with the reading nowhere on the figure. Census unchanged.
- Two phase-3 items left, and **both are bigger than they look** (sized 2026-10-28 late):
  1. **Series-parallel and capacitor modes on `circuit-diagram`.** The figure is a mature subsystem
     (`src/figures/physics/instr.rs` ~800 lines plus `circuit_solve.rs`): a `CLay` geometry shared with the drag
     handles, per-component kinds and label modes, an ammeter/voltmeter with their own readings, a `solve` mode with a
     results table, US/IEC symbol styles and Print.
     - **(a) Capacitor mode: DONE.** A new first-choice `components` control (default `Resistors and bulbs`, so saved
       figures are untouched) makes every slot a capacitor, draws two plates, names them C/C_1..C_4, and switches the
       worked-out block to `C_eq`, `Q`, and each part's `V_k`/`Q_k` (`solve_caps` in `circuit_solve.rs`). Series adds
       reciprocals, parallel adds capacitances. Values are farads (`10 µF`, `470 nF`); `parse_qty` gained `F` and the
       n/p prefixes. A value still in ohms is refused (prints `?`) rather than read as farads, and
       `circuit_warnings` tells the editor why. The ammeter and voltmeter are forced out of the **layout**
       (`circuit_layout`) and out of the drawing (including "Across the battery"), so the two meter controls render an
       identical figure; an open switch leaves the capacitors uncharged (zero charge, zero volts) while the equivalent
       capacitance is still known. The parallel layout also now starts its branches beyond the battery's value label
       and the first branch's, measured from the texts — a fixed 240 px already collided for "4.7 kΩ" beside "12 V". Two examples and a FAQ entry added; census moved only for
       `circuit-diagram__defaults`/`__start`, from the blurb.
     - **(b) A combination (series-parallel) layout: DONE.** `In series and parallel` puts the first part on the top
       wire and the rest as branches, with `solve_combo`/`solve_caps_combo`, handles that follow the drawing, and the
       voltmeter placed per part. Census unchanged (the default layout is still series).
  2. **Ray diagram and motion map: BOTH BUILT.** `ray-diagram` (thin lens, three principal rays, lens equation, drag
     handle on the object) and `motion-map` (strobe dots, velocity arrows, constant/speeding/slowing, drag handle on
     the last flash). Both have five examples, four FAQ entries, tests, PNG exports and handles, so neither is in
     `NO_HANDLES`. **Phase 3 is complete.**
- Then phase 4 (biology, 7 figures), phase 5 (chemistry, 10), phase 6 (stats, 7).
- Then phase 4 (biology, 7 figures), phase 5 (chemistry, 10), phase 6 (stats, 7).
- Phase 2 is complete and committed. Phase 3 built: energy bar charts, collision/momentum, circular motion, electric
  field, refraction, magnetic field, standing waves, PV diagrams.


## Notes box — fixed and confirmed in the app (2026-10-28)
The editor notes used to be built inside `rows_card`, which only four figures get, so `figures::notes_for` returned
strings that were never drawn for `collision-momentum`, `energy-bar-chart`, `circle`, `sine-cosine` or
`slope-triangle`. They are now a `notes_box` called from `controls` for every figure (`src/figuresui.rs`).

Confirmed by running the app (`VERSHION_SCREEN=figures VERSHION_FIG=collision-momentum`, Xvfb :88):
- with `VERSHION_FIG_PARAMS='{"u1":0,"u2":5}'` the red framed **Notes** box appears above Values with "A is not
  closing on B (A must start faster than B for the carts to meet), so there is no collision to work out."
- with `{"u1":4,"u2":0}` the box is absent, so it is not always-on.

## Pitfalls
- **`until ! pgrep -f "cargo test …"` deadlocks.** The guard shell's own command line contains the pattern, so it
  matches itself and never starts the command it is waiting to run — and two such guards match each other forever.
  Wait on a real pid (`pgrep -x cargo`) or just run the suite directly; the background task notifies on exit anyway.
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
- Suite on `d19c08b`: **1775 passed, 0 failed, 5 ignored**.
  Note: `figuresui::tests::dragging_a_label_moves_it_as_one_undo_step` fails under CPU contention (two full
  parallel runs) and passes alone and in both clean full runs; it is timing-sensitive, not a regression. The real
  fix is for that test to drive enough frames to settle.
- **All eleven figures have now been critiqued.** The last four (protractor, area model, base-ten blocks, solid-net)
  were failed on 2 blockers and 8 majors, fixed in `b706974`, and re-checked by a verification critic: the blockers
  and all but one major hold; the leftovers were fixed in the commit after it. See
  `docs/critic-reports/2026-10-28-figures-critics-3.md`.
- The five newest figures (1.4, 1.5, transformations, fraction models, triangle centres, slope) have had two
  critic rounds; the protractor has had none yet.
- `HANDOFF.md` here is untracked; do not commit it with the figure work.
- Round 2's handoff claimed round 2 was uncommitted and HEAD was `c3eb729`; it was stale. Check `git log` before trusting a handoff.

## 2026-10-29, late: phase 3 closed, phase 4 at three figures

- HEAD `4f89e17` on `figures-gapfill`; tree clean apart from this note. Local commits only.
- Phase 3 complete and verified. Phase 4 figures done:
  - `hardy-weinberg` — `bd58e90`
  - `chi-square-fit` — `d30ece5`
  - `enzyme-kinetics` — `4f89e17`
- Full suite on `4f89e17`: **1851 passed, 0 failed, 5 ignored** (run alone; ~10-19 min depending on load).
- **In flight:** one combined accuracy + teacher + regression critic over the three, on a frozen `git archive
  4f89e17`. When it reports, fix blockers and majors, commit, then run a verification critic on the fix commit.
- **Then**: phase 4 figures 4-7 (cladogram from a typed tree, food web / energy pyramid, action potential, cell
  cycle, cell diagrams), phase 5 chemistry (10), phase 6 stats (7).
- Pitfalls confirmed this session:
  - `gen` is a reserved keyword in this edition.
  - A test helper named `f` is shadowed by any `for f in [...]`; use `fl`/`uu`.
  - `paint-order="stroke"` is ignored by the PNG renderer; use a white plate or draw the text twice.
  - A numeric param set with `Value::S` in an example passes the figure tests and fails the two share round-trip
    tests; always use `Value::F`.
  - `git add -A` commits this file; stage figure files by name.
  - Timing-sensitive tests (`dragging_a_label_moves_it_as_one_undo_step`, `absurd_windows_cannot_run_away`) fail
    under CPU contention — never run a second cargo while the suite runs, and check them alone before calling a
    regression.

## 2026-10-29, later: biology critic fixed + cladogram, round 2

- The combined biology critic (`docs/critic-reports/2026-10-29-biology-critics-1.md`) came back **FAIL — 2 blockers,
  9 majors, 13 minors**, all on awkward input; the arithmetic was verified correct. Its fixes were staged but not
  committed when this session picked the work up.
- Before committing them I read the *rendered* cladogram PNGs and found three defects the critic and every test had
  missed, one a blocker: (1) the rectangular shape drew each branch along the PARENT's row, so `(A, B, C, D, E)` showed
  one bar and a single stub — four leaves had no branch; (2) the parser stripped all whitespace, so `Homo sapiens`
  printed as `Homosapiens`; (3) an ancestor name typed after a bracket was parsed and never drawn, although the hint,
  the blurb, the FAQ and an example all promised it. All three fixed, with tests; the FAQ corrected; the leaf-order
  text under the picture now grows the canvas instead of running off it; the count line is pluralised properly.
- Also fixed a contradiction the biology critic's M7 fix had introduced: the chi-square table's Total row still printed
  the exact statistic beside a line that now prints the sum of the printed contributions (1.43 vs 1.42).
- Census re-blessed; only `cladogram`'s two ids moved.
- **Next:** the verification critic on this fix commit (the fixes themselves have not been re-critiqued), then phase 4
  figures 4-7: food web / energy pyramid, action potential, cell cycle, cell diagrams; then phase 5 chemistry (10),
  phase 6 stats (7).

## 2026-10-29, round 3: the verification critic's six majors fixed

- Verification critic on `9a5ae46` (frozen copy): **FAIL** — both blockers and most majors genuinely fixed, but six
  majors remained, three of them in classes the first round claimed. Fixed: cladogram text wrapping to the canvas
  (last names were cut off) and indentation of continuation lines; error reasons wrapping and "Nothing is drawn"
  placement; the enzyme rate label landing on the Km label at [S] = 0; the rate label having no halo over the curve;
  chi-square's total precision when a contribution prints below 0.005 (0.0033 + 0.01 now totals 0.0133); Hardy-Weinberg
  printing p = 0.99 as "1" at one decimal place. Six minors fixed too (see docs/figures-progress.md).
- Census: only `chi-square-fit` and `enzyme-kinetics` re-hashed. Full suite re-run on the fix.
- **Next:** a second verification critic on this fix commit (the loop's rule: fixes themselves get re-checked), then
  phase-4 figures 5-8: food web / energy pyramid, action potential, cell cycle, cell diagrams; then phase 5, phase 6.
- The critic noted (not counted) that the enzyme's drag handle still sits at the answer point on SCREEN while Show
  answers is off — the exported student copy is clean; deciding whether on-screen handles should follow Show answers is
  an open product question.

## 2026-10-29, round 4: the second verification critic's five majors fixed

- Second verification critic on `a073352`: **FAIL** — five majors, three of them incomplete versions of round 3's own
  fixes. Fixed: enzyme rate label over "Vmax/2" at [S] = Km (the lift now checks both labels); inhibited half-rate label
  now "Vmax′/2"; Hardy-Weinberg's "p in each generation" line now uses the page precision (p = 0.99 no longer prints as
  a column of 1s), and so does the editor note; the cladogram's closing sentence wraps (it ran off the canvas at Large)
  and the canvas takes the count line's width at Large; a chi-square contribution for a category expected to be empty
  but counted now prints ∞ instead of 0.00. Minors: half-rate label haloed; enzyme Print grid `#787878` (it vanished in
  the 66% copy). Census: enzyme only.
- **Next:** a third verification critic on this commit, or accept the loop as converged if it comes back with minors
  only; then phase-4 figures: food web / energy pyramid, action potential, cell cycle, cell diagrams.

## 2026-10-29, round 5: the third verification critic's four majors fixed

- Third verification critic on `8d64904`: **FAIL**, no blocker, 4 majors + 6 minors — and again three majors were one
  case over from the previous fix. Fixed: the half-rate label vs the Km label (strong noncompetitive inhibitor printed
  them on one line); the "beyond this axis" note outside the collision set (a point at the axis end printed through it);
  name truncation not reserving the observed number's width; Hardy-Weinberg rounding moved the whole difference into the
  largest part (1:2:1 printed 0.3/0.4/0.3) — now largest remainder. Minors: givens wraps, ∞ instead of "inf" with a
  consistent Total and χ² line, a huge contribution in one compact form, grouped contributions, the decision line wraps,
  and the cladogram measures text by width per glyph so a run of Ws is not clipped.
- Census: cladogram only.
- **Loop status:** three verification passes, each finding real defects, the last two mostly "the fix did not cover the
  case next door". A fourth pass would be the test of convergence; otherwise move to phase-4 figure 5 (food web /
  energy pyramid).
