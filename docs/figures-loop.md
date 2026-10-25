# Figures gap-fill gauntlet loop

Paste the block below into a fresh Claude Code session opened in this repo. It adds the figures the
2026-10-25 review found missing, plus the shared engine work that makes them cheap, and loops each
phase until a blind critic accepts it.

Source of the list: the five-agent figure review of 2026-10-25 (math/stats, physics, biology/chemistry,
engine). 58 figures existed: 11 physics, 6 biology, 16 chemistry, 19 math, 6 stats. Decisions already
made: build the engine groundwork first, then the named gaps (circles, trig), then the rest by
teacher demand. Figures are original work; reference sites are for scope and controls, never code,
assets or copy.

```
Add the missing figures to this repo (src/figures/, src/figuresui/) in the phases below, in order.
The working tree on shared-pool-phase1 holds uncommitted circles work. Do not touch it: make a git
worktree off figures-circuits (44 unmerged commits of figure work, ahead of this review) on a new branch
`figures-gapfill` and work only there.

Follow docs/new-figure.md for every figure: module with defs(), register in registry(), FAQ arm in
faq.rs, start.rs opening state, examples, handles in interact.rs where the figure has draggable
parts, answers_param() whenever it prints a computed value, svg::style_param() Print style (grey-only,
via Fills), label_size_param(), svg::Doc output only. Render must be pure and never panic on an odd
value. Original code and drawing only.

Phases:
0. Engine groundwork, no new figures. Before touching anything, dump a baseline census:
   VERSHION_CENSUS_DIR=<scratchpad>/census-before cargo test --release census_dump -- --ignored
   a. One shared axes/ticks module (start from chart::Frame/axes/nice_step) and delete the six
      nice_step copies (chart.rs, biology.rs, chemistry/curves.rs, chemistry/frame.rs,
      physics/common.rs, math.rs). Port curves, popdyn, graphs and math_plot to it.
   b. Doc geometry helpers: arc, sector, angle_mark, right_angle, dashed/double arrow, dim_line;
      remove the private copies in math.rs, physics/common.rs, composite.rs.
   c. Doc::finish emits <title>, <desc> and role="img" (figure title plus a one-line summary).
      Add a colour-blind-safe series palette beside Print.
   d. Generic answer keys: computed_for beyond math, and answers_param on every figure that prints
      a computed value (titration, mass spectrum, motion graphs, projectile, spring-scale hide-reading).
   e. QA gates: handle_coverage_report asserts against an explicit NO_HANDLES list; census_dump gets a
      baseline-diff test; a test that Print renders are grey-only; agent-eval cases for every subject.
   Acceptance: re-run the census; every EXISTING figure's SVG is byte-identical to the baseline except
   the added title/desc/role (diff -r, and say which files differ and why). cargo test passes.
1. Math core: conics and implicit curves on coordinate-grid (x^2+y^2=25, ellipse, hyperbola, parabola),
   circle figure (radius, diameter, chord, tangent, secant, central and inscribed angle, arc, sector,
   arc length and area readout, Measures card), right-triangle trig labels as modes on triangle
   (opposite/adjacent/hypotenuse, SOH-CAH-TOA), sine/cosine markers (amplitude, period, midline,
   phase presets), unit circle (degrees or radians, coordinate triple).
2. Math rest: transformations on a grid (translate, reflect, rotate, dilate, pre-image and image),
   bar/pie/line charts, Venn (2 and 3 sets) and probability tree, fraction and number models (bars,
   circles, tape diagram, area model, base-ten blocks, ten-frame), similar/congruent triangles,
   triangle centers, slope triangle, protractor, solids surface-area/volume readout and nets.
3. Physics: energy bar charts, collisions and momentum, circular motion, electric field lines and
   equipotentials, Snell/refraction/total internal reflection, magnetic field and right-hand rule,
   standing waves and interference, PV diagrams. Plus handles for ray-diagram and motion-map,
   hide-reading on spring-scale, series-parallel and capacitor on circuit-diagram.
4. Biology: Hardy-Weinberg, chi-square / expected-vs-observed table, enzyme kinetics, cladogram from a
   typed tree string, food web and energy pyramid, action potential, cell cycle, cell diagrams.
5. Chemistry: reaction energy diagram (Ea, catalyst, delta H), rate and kinetics graphs, phase
   diagram, solubility curves, gas-law graphs, equilibrium/ICE graphs, buffer curves, Boltzmann
   distribution, periodic-table excerpt, VSEPR 3-D geometry.
6. Stats: bar chart for categories, frequency table, cumulative/ogive, residual plot and other
   regression models, t and chi-square distributions, back-to-back stem-and-leaf, handles for the
   six stats figures.

Within a phase, take the figures one at a time (a figure is a unit; do not batch a whole phase into
one builder). For each figure fan out a builder and, separately, fresh critics:
  - Accuracy critic: checks the maths/science. Recomputes every printed value and geometric
    relationship independently, tries edge parameters (zero, negative, huge, empty, unparsable) and
    states a wrong-looking render as a blocker.
  - Teacher critic: rasterises the figure (to_png) in default, Print and answer-key states and LOOKS
    at it with the image reader. Judges whether a teacher could drop it straight into an exam:
    label collisions, clipped text, unreadable sizes, missing controls a teacher would expect, the
    right defaults. Compares scope against the subject's reference site when one exists (mathfigures.com
    for math; for the other tabs the reference named in the module header or chem_shell.rs). Names
    the single biggest remaining gap.
  - Regression critic (phases 0 and 1 especially): census diff against the baseline, test count
    change, anything deleted to go green.
Critics are harsh; praise is not useful. Then back to the builder. cargo test --lib figures and
cargo test --lib figuresui must pass, and the whole suite at the end of each phase. Existing tests
are updated or replaced, never deleted to go green.

/loop on each figure until all critics pass it. Do not stop before that. Commit each converged figure
locally on figures-gapfill; write a one-paragraph summary per phase to docs/figures-progress.md and
keep critic reports in docs/critic-reports/ in the existing naming. Never push.

Stop and ask me for: a new class of critic problem, a decision in this plan that proves wrong, any
edit to src/screens.rs, theme.rs, fonts.rs, main.rs or examples/ (a missing glyph in the bundled
fonts counts), a change to any saved figure/spec/share-link format (old links must still load: use
start::added_in), any new dependency, or privacy/terms wording.

Keep a live progress page updating as the work evolves so I can watch it.

Fan out subagents and ultracode.
```

## Notes

- 2026-10-25: base changed from shared-pool-phase1 to `figures-circuits`, which already adds tabs
  (economics, accounting, computing, earth, art, ...), a chi-square table, sampling distribution,
  rejection region, a circuit solver and a tree editor. Before building any figure in a later phase,
  check it is not already there; the review's counts (58 figures) predate that branch.

- Earlier loops treated `figures*` as protected. This loop is the exception: `src/figures/` and
  `src/figuresui/` are in scope here, and the rest of the protected list still stands.
- Phase 0 is the risky one: it rewrites shared drawing code every figure uses. The census byte-diff
  is the safety net, so do not skip the baseline dump.
- Saved figures and share links must keep loading (`vershion://figure/<id>?p=...&v=3`). New params
  need defaults that reproduce the old render; changing a default needs `start::added_in`.
- Lower priority, not in the phases: pendulum/SHM, torque, orbit, Doppler, photoelectric, nuclear
  decay, DNA/translation, mitosis/meiosis, osmosis, electrochemical cell, calorimetry, Born-Haber,
  skeletal structures, lab apparatus, IR/NMR, polar/parametric graphs, vectors on the grid.
- Standing permissions from the circles loop do not carry over. This loop has its own: local commits
  only, no push, no deploy.
