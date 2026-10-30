# Critic 1 on ae36501: action potential (new figure)

A first critic pass over `ae36501`. The whole change is `actionpotential.rs` (1,820 lines), `faq.rs`, the registry and
`notes_for` lines, `interact.rs`'s NO_HANDLES list, the progress doc and two census lines. I ran everything on my own
`git archive ae36501` copies with their own target dirs, and ignored the worktree's uncommitted HANDOFF.md.

My harness (a `critic` child module appended to my copy only) adds:
- a **pixel audit** that does not use the module's constants or text tables. It splits each SVG into elements, rasterizes
  each group alone with resvg at 2x (real Noto Sans), and measures literal pixel gaps: text vs text, text vs every mark,
  disc vs curve (2 px), disc vs guides/axes/brackets (1 px), leader vs curve away from its own dot, leader vs guides/arrow,
  leader vs leader, leader vs other discs, and ink outside the canvas. It ran on 1,428 renders: 17 trace sets (peak =
  threshold + 20, threshold = rest + 10, threshold at -4 and +4 mV, rest -100 and -10, undershoot = rest and rest - 3,
  stimulus 0.5 and its last allowed time, totals 5.2/6.4/8/12, values like -62.3/37.5/-77.9) x S/M/L x 7 switch sets x
  failed x answers;
- a 66-input parse/boundary probe and a 12,000-input agreement sweep with my own generator;
- 14 mutants on a pristine copy, plus a render-hash comparison over 6,912 renders to see whether a surviving mutant
  changes the output;
- PNGs at 1.5x and the 66 % print copy at 1x and 1.3x, which I looked at with the image reader.

**Verdict: FAIL (0 blocker, 3 major, 4 MED, 5 LOW, 6 minor).**

What holds:
- The curve is right. It is analytic, hits the typed extremes exactly, and has no wiggle.
- Note and canvas agree: 0 disagreements in 12,000 inputs.
- The family never overlaps text: 0 text collisions and 0 disc/curve contacts in 1,428 pixel-audited renders.
- The suite and the census are clean.

What fails is the exercise itself:
1. The axis cannot be read to the precision the figure says it is testing. Ticks are 20 mV at Small/Medium and **50 mV**
   at Large, with no minor ticks or grid.
2. In the bundled "No undershoot" example, "Resting potential" and "Return to resting potential" point at the same flat line.
3. The student copy's caption gives away the failed-initiation answer ("too weak to start a spike") and the word
   "resting".

One biology error also matters to a teacher: the relative refractory and K+ brackets end while the curve is still
visibly hyperpolarised.

## Claim table

| # | Claim | Verdict | Evidence |
|---|---|---|---|
| 1 | Analytic curve exact at rest/peak/lowest point, monotone (+,-,+), threshold crossed once up and once down | **HOLDS** | I re-derived it independently: default samples 1.0 → -70, 1.8 → 40.000, 3.2 → -80.000; t_thr = 1.3019 ms, back-to-rest 2.9515 ms. Every join is C1: the derivative is 0 at the stimulus, peak and trough, and `(1+x)e^-x` has slope 0 at x=0. The curvature jumps at the tip (left 2,320 vs right 367 mV/ms² at default), so the peak is visibly sharp-left/round-right. That is physiological, not a kink: in a 1.5x render (`all.png`) there is no visible corner at the joins. After settling it never dips below rest a second time; the pieces are monotone. |
| 2 | Failed initiation peaks 60 % of the way to threshold | **HOLDS** | Mutant BUMP_FRAC=1.05 is killed by 2 tests. `failed.png` peaks at -61 mV for -70/-55. |
| 3 | Path dense, no chord > 0.25 px | **HOLDS** | The builder's test is real (mutant "peak 1 px off" killed). Default path: 18.7 KB, 1,353 points. That is reasonable. |
| 4 | 8 numbered pointers + lettered brackets, two-column key, blank rules in student copy | **HOLDS, but two pointers are ambiguous (MAJOR 2, MED 5)** | `all.png`, `student-all.png`. The blank rule is ~305 px at Medium, which is ample for "Hyperpolarisation (undershoot)" handwritten at every size. |
| 5 | Typed resting/threshold/peak/undershoot never printed as text | **HOLDS** | Typed values are never text, desc or title. (Tick labels can coincide with a typed value, e.g. -70 at a 10 mV step; that is the axis.) |
| 6 | Refusals true, with a second line; stated ranges | **PARTLY** | All 13 builder cases and every integer boundary behave as stated (probe table below). But the second line can ask for an impossible total (LOW 1); one-decimal boundary inputs are refused about 10 % of the time (LOW 2); and rounding makes several messages contradict themselves (minor 1-3). |
| 7 | Disclosures name-free and only when something was left out | **PARTLY** | They are only shown when something was dropped (my 12,000 sweep: 0 mismatches). They are free of key names, but not answer-free: see MAJOR 3. |
| 8 | `notes_for` == canvas caption over 10,000 inputs | **HOLDS** | My own generator, 12,000 inputs (6,830 drawn, mixed `−`, `mV`, decimal comma, Print, all switches): **0** disagreements, and **0** plan refusals of a read-valid input. The two "never-firing" refusal branches never fired. |
| 9 | 4,992-render sweep: no overlaps | **PARTLY** | 0 text/text, text/mark, disc/curve, disc/guide or off-canvas problems in my 1,428 pixel-audited renders. But the builder's audit never measures **leaders**: a leader crosses the stimulus arrow in 16 renders (MED 1), and two guards on leaders are untested (LOW 4). |
| 10 | Print grey-only | **HOLDS literally** | It is grey-only. But the four brackets are identical black strokes (`fills.dash` unused), and in the 66 % copy the raised `+` turns into a minus or an apostrophe (MED 3). |
| 11 | 17/17 mutants killed | **PARTLY** | 10 of my 14 are killed and 4 survive (LOW 4). |
| 12 | Suite 2007/0; census moved only the two new ids | **CONFIRMED** | See Regression. |

## Defects (ranked)

### MAJOR 1: the values the exercise says are "read off the axes" cannot be read off these axes
- **Ticks.** The y step is chosen so that tick labels are 2.6·font apart.
  - Default at Small and Medium: ticks every **20 mV** (-100, -80 … 60). There are no minor ticks and no grid, so the
    -55 threshold is a dashed line a quarter of the way between -60 and -40.
  - Default at **Large**: ticks every **50 mV**, -150 to 100 (`large-all.png`). Rest -70, threshold -55 and undershoot
    -80 all sit between the -100 and -50 ticks. A student cannot tell -70 from -75 or -65.
- **Wasted range.** At Large with rest -100, threshold -90, peak 70, no dip, stimulus 0.5 and total 5.2, the axis runs
  **-200 to 150** (`s9all.png`). That is a -200 tick on a figure that refuses anything below -100. It is worse than the
  disclosed "-150 tick with white space", and a third of the plot is empty.
- **Why it matters.** FAQ 3 says: "they are read off the axes, which is the skill being tested". With ±5 mV (Medium) or
  ±12 mV (Large) reading error, a question like "what is the threshold?" has no single right answer.
- **Fix.** Minor ticks every 5 or 10 mV (unlabelled), or a light grid, at every size. Also stop the Large range from
  growing in 50 mV steps.

### MAJOR 2: "Resting potential" and "Return to resting potential" point at the same flat line (no undershoot)
- **Input.** Undershoot = resting (`-70`). This is the bundled example "No undershoot" (`nodip.png`, key on).
- **What is drawn.**
  - Pointer 2 ("Resting potential") points at the flat line at 0.5 ms.
  - Pointer 7 ("Return to resting potential") points at the flat line at **4.2 ms**.
  - The curve has been flat at -70 since 3.2 ms (the end of the fall).
- **Why it matters.** Both dots are on an identical horizontal stretch at the same level, so "resting potential" is a
  correct answer for 7 and the blank has two defensible answers. Without a dip there is no separate return phase: the
  return is the last part of repolarisation.
- **Fix.** Either drop pointer "Return to resting potential" when there is no dip (and say so in the existing
  disclosure), or anchor it on the end of the fall, before the curve is flat.

### MAJOR 3: the student copy's caption and description give away answers
- **Failed initiation.** Input: Failed initiation on, default switches (channel bands on, the default), Show answers off
  (`failed-student.png`).
  - The canvas prints "**This input is too weak to start a spike**, so the bands that belong to a spike are not drawn."
    The failed-initiation figure exists for the all-or-none question ("why is there no action potential?"), and this
    caption is the answer.
  - The `<desc>` of the same student copy says "…for a neuron, **a trace that fails to start a spike**. 5 pointers to name."
  - The caption appears only because the channel switch is on by default, so a teacher who just ticks "Failed
    initiation" ships it.
- **No undershoot.** In a student copy the caption says "The lowest point is **the resting level**, so the trace has no
  dip below rest…". That names the answer to pointer 2 (and 7).
- **Why the leak test missed it.** Its list (`NAMES`) has "resting potential", not "resting level", and has neither
  "too weak" nor "fails to start".
- **Fix.**
  - Show these disclosures only in the editor note, not on a student canvas.
  - Or word them without the concept: "Some bands were switched on but are not drawn for this trace."
  - Drop the fail clause from the student desc.

### MED 1: a leader crosses the stimulus arrow, and two discs bunch at the arrow
- **Input.** Resting -100, threshold -90, peak 70, undershoot -100, stimulus 0.5, total 5.2, Large, any switch set
  with stimulus + phases on: 16 of my renders (`s9all.png`).
- **What is drawn.** Pointer 2's leader goes from the rest line at 0.25 ms diagonally down-right, **through the
  stimulus arrow's shaft**. Its disc then sits beside disc 1 under the arrow. Students will read 2 as pointing at the
  arrow.
- **Measured.** Leader from (112.6, 248.0) meets the arrow at (136.0, 270.5): 24 overlapping px at 2x.
- **Why it happens.** The search tests only the disc centre against the arrow (`segs`), never the leader. The builder's
  audit has no leader checks at all.
- **Fix.** Add the arrow (and the other leaders) to the leader `seg_cross` test in the strict pass, and add a
  leader-vs-arrow check to `audit`.

### MED 2: the relative refractory (D) and K+ (B) brackets end while the membrane is still hyperpolarised
- **What is drawn.** Both brackets end at trough + 1 ms (`T_LINGER`).
  - Default: they end at 4.2 ms, where the curve is at **-72.7 mV**. 27 % of the 10 mV dip remains, about 5 px below
    the rest line at Medium.
  - The curve is within 0.5 mV of rest only at about 5.0 ms (`all.png`: D clearly stops before the curve flattens).
- **What textbooks say** (OpenStax A&P 12.4; Campbell 48.3): the relative refractory period lasts through the
  hyperpolarisation until the membrane is back at rest, and K+ channels close as it returns. A biology teacher would
  mark D as too short.
- **Consequence.** Pointer 8 ("Return to resting potential") is placed at the same 4.2 ms, on a stretch that is still
  below rest. A student who writes "hyperpolarisation" for 8 is defensible (a second two-answer blank).
- **No dip.** The relative refractory period is dropped entirely when there is no dip (FAQ 4 says so). The relative
  period is about Na+ inactivation recovery and residual K+ conductance, not about a visible dip; I rank that part LOW.
- **Fix.** End D (and B) where the curve is back within about 1 % of the dip, i.e. ts + NEED, and put pointer 8 at the
  end of the recovery.

### MED 3: in the 66 % print copy the ion charges read as negative, and the brackets are told apart by letter only
- **Charges.** The raised `+` is drawn at 0.7× the key font: 9.1 px at Medium and 7.3 px at Small.
  - Print, Medium, 1x, 66 % (`print-all-1x-bw66.png`, zoom `zoom-na-med.png`): "**K⁻** channels open". The vertical
    stroke drops out.
  - Small, 1x: "**Na⁻** channels open" (`zoom-na-small.png`).
  - Small, 1.3x: "K' channels open" (`zoom-na-small13.png`).
  - On the teacher's answer key that is a wrong ion.
- **Brackets.** In Print all four are the same solid black (`fills.line()` is `#000`; `fills.dash()` exists but is not
  used). Na+ and K+ share a lane with a 3 px gap at the peak, so A+B read as one long bracket with a notch
  (`print-student-1x-bw66.png`).
- **Dots.** The pointer dots (black, r 2.6) vanish into the black curve.
- **Fix.**
  - Draw the `+` at no less than about 0.85× (or as a full-size "Na+").
  - Use `fills.dash(slot)` on the brackets in Print.
  - Give dots a white halo.

### MED 4: the K+ / Na+ / refractory lanes are unlabelled, so a student cannot tell which row means what
- **What is drawn.** In a student copy (`student-all.png`) the two lanes have no row label.
  - A (threshold → peak) can be named "Na+ channels open", "depolarisation" or "absolute refractory (start)".
  - C (threshold → back at rest) can be named "absolute refractory" or "Na+ channels inactivated/open".
  - Each is defensible, and the key will accept only one.
- **Fix.** A row label ("Ion channels" / "Refractory periods") in both copies is not an answer and removes the ambiguity.

### LOW 1: the second line of a timing refusal can ask for an impossible total
- `stimulus 8, total 12`: "Make the total time at least **12.7 ms**, or move the stimulus earlier". The maximum is 12.
- `stimulus 7.4, total 12`: "at least 12.2 ms".
- `stimulus 100, total 8`: "at least **104.7 ms**".
- When ts > 7.3 the only fix is moving the stimulus, so the first suggestion is false.

### LOW 2: exact-boundary decimals are refused (float comparison without tolerance)
- `resting -73.6, threshold -63.6` gives "The threshold (−63.6 mV) must be at least 10 mV above the resting potential
  (−73.6 mV)".
- `threshold -83.6, peak -63.6` gives "must be at least 20 mV above the threshold (−83.6 mV)".
- One-decimal inputs placed exactly on a boundary are refused for 84 of 901 (rest + 10), 152 of 1,321 (threshold + 20)
  and 18 of 871 (rest - 3). My agreement sweep hit one (`-41,08 / -14.12 / 5.88`).
- The `ts + NEED` check already has `1e-9`; the other three checks need the same tolerance.

### LOW 3: a failed initiation can climb above 0 mV
- Input: resting -10, threshold 50, peak 70.
- The "Local (graded) response" peaks at **+26 mV** (`pngm` pristine render, set 7), above the 0 mV line, under a
  threshold of +50 mV.
- Thresholds above 0 mV (anything up to +50) are accepted. No textbook neuron has one.
- Owner decision: cap the threshold at about -20 mV, or say so.

### LOW 4: test quality — four mutants survive, and leaders are never audited
I ran 14 mutants on a pristine copy, each against the module's 25 tests (+2 env-gated).

| Mutant | Result |
|---|---|
| curve 1 px off at the peak (draw only) | killed (path test) |
| failure bump crosses threshold (BUMP_FRAC 1.05) | killed (2) |
| phase names in the student `<desc>` | killed (2) |
| caption dropped in student copy (note kept) | killed (agreement test) |
| Na+ bracket starts 0.1 ms early | killed (band test) |
| Na+ bracket ends 0.25 ms after the peak | killed (band test) |
| disc clearance shrunk to r-3 | killed (2) |
| relative bracket ends at the trough | killed (band test) |
| Return pointer anchored on the flat rest | killed (2) |
| red threshold line in Print | killed (print test) |
| **"leader must leave the curve" guard removed** | **SURVIVED**. It changes 32 of 6,912 renders; in `pngm/mutant.png` (resting -10, threshold 50, failed, Large) pointer 4's leader runs **along the bump**, and my audit flags 39 px of contact |
| **leader-crosses-leader guard removed** | **SURVIVED** (0 of 6,912 renders change: equivalent in the sweep, but untested) |
| **curve `#bbbbbb` in Print** | **SURVIVED**. The "curve is black" assertion is `svg.contains("#000000") \|\| svg.contains("#000\"")`, and the brackets' `#000` satisfies it. A pale curve vanishes in the 66 % copy |
| **threshold line `#bbbbbb` in Print** | **SURVIVED** (same reason; grey-only is all the test checks) |

- The builder's `audit()` measures text, discs, lines-through-discs and arrowheads, but **never a leader** against the
  curve, the arrow or another leader. MED 1 is the result.
- The Print test should rasterize at 66 % and check for ink on the curve and the dashed line, as the energy-pyramid
  round 3 test does.
- The curve-truth tests share `Spike::v` / `t_peak` / `t_threshold` with the code. They are fine for the band times,
  because the expected values come from the curve, but the band test would pass with any biologically wrong rule that
  uses those same functions (MED 2 is not catchable by it).

### LOW 5: the editor uses six free-text boxes where the other biology figures use sliders
- Enzyme kinetics and Hardy-Weinberg use `ParamDef::float/int` with ranges; this figure uses `ParamDef::text`.
- That is the source of every parse refusal and of LOW 1 and LOW 2. The builder disclosed it.
- For the four potentials and two times, a slider with the stated range (and a cross-check refusal) would remove most
  refusal paths. Owner decision.

### minor 1-6
1. 0.4999 ms gives "The stimulus time (**0.5 ms**) is before 0.5 ms". 12.0001 gives "A total time of **12 ms** is over
   the 12 ms". Display rounding contradicts the rule.
2. Undershoot -70.0001 gives "An undershoot only **0 mV** below rest … is too shallow". -72.9999 gives "only **3 mV**
   below rest … too shallow".
3. Resting -100.0001 gives "The resting potential is **−100 mV**, outside the −100 to +70 mV".
4. Resting -9.99 gives "…but a neuron's inside is **negative** at rest". -9.99 is negative; the true reason is the
   -10 limit. Resting -150 is told "outside the −100 to +70 mV", but the resting range is -100 to -10.
5. Total -5 or 0 gives "…leaves only 0 ms of a **−5 ms** trace" instead of "a time must be positive".
6. Code:
   - dead test helper `b2f` (the module's only compiler warning);
   - `let _ = xstep;`;
   - an empty `if pl.thr_disc && pl.show_thr { }` block;
   - the `Band.key` doc comment says "Index into `Plan::pointers`" but it is a name;
   - `dist_seg` duplicates the public `composite::seg_dist`, and `seg_cross` duplicates `math.rs`'s;
   - a leader can graze the curve 7-10 px from its dot (1-3 px contact, 4 renders at Large, sets 9/15, mask 11100).
     That is cosmetic.

## Probe table (selected; all 66 in my scratchpad `probe.txt`)
- **Read correctly:** `-70 mV`, `-70mV`, `−70`, `-70,0`, BOM+CRLF, tabs, NBSP before `mV`, `-70 millivolts`, `8ms`,
  `1,5`.
- **Refused as "not a number":** `-70 m V`, `-7O`, `NaN`, `inf`, `1e999`, `1e1`, `8 s`, `8 mV` (in a time field).
- **Refused as ambiguous:** `1,500` and `8,500`.
- **Refused as empty:** empty and whitespace-only input.
- **Integer boundaries exactly as stated:** threshold -60 ok / -60.0001 no; ±4 ok / ±3.99 no; peak -35 ok / -35.01 no;
  70 ok / 70.01 no; undershoot -70 ok (no dip) / -73 ok / -72.9999 no; -100 ok / -100.5 no; stimulus 0.5 ok / 0.4999
  no; total 5.7 ok / 5.69 no; 12 ok / 12.0001 no.
- **Panics:** none. **Silent drops:** none. **A different number than typed:** none.

## Teacher view (PNGs looked at)
- **Default** (`all.png`, `default.png`): clean, conventional and correctly ordered.
  - Depolarisation (4) is on the rising limb above threshold, Peak (5) is at the top, and Repolarisation (6) is on the
    falling limb.
  - Na+ runs from threshold to peak and K+ from peak onward, as textbooks draw them.
  - Weak points: the 20 mV ticks (MAJOR 1) and the short D/B brackets (MED 2).
- **Student** (`student-all.png`): good blanks, but there are no lane labels (MED 4).
- **Failed initiation** (`failed.png`, `failed-student.png`): the pointers (stimulus, rest, threshold, local response,
  return) are right. The caption gives the answer away (MAJOR 3).
- **Large** (`large-all.png`, `large-short.png`): 50 mV ticks and an empty -100 to -150 band. Not usable for reading
  values.
- **Print at 66 %** (`print-student-1x-bw66.png`, `small-all-1x-bw66.png`):
  - The curve, the dashed threshold, the 0 mV line and every disc survive.
  - The brackets survive but are identical (MED 3), and "K⁻" appears on the key.
  - The blue/red/orange/green/red/purple Colour copy also survives the 66 % threshold, because every colour's luminance
    is under 168.
- **5.2 ms trace** (`short.png`, `large-short.png`): it does not crowd. At Large, disc 2 and disc 1 sit next to each
  other below the rest line but are distinct.

Would a biology teacher drop this into an exam now? For a "name the numbered parts" question at Small/Medium with the
default values, yes. For anything that asks for a value (threshold, resting potential, the amplitude of the
undershoot), or at Large, no. For the failed-initiation copy, not without hand-deleting the caption.

**Single biggest remaining gap:** the axes have no fine scale (MAJOR 1). The figure's own FAQ says reading those values
is the skill being tested.

**Missing features a teacher would want:**
- minor ticks or a light grid;
- vertical guide lines (or shading on the plot) at the bracket ends, so a student sees which part of the curve each
  bracket covers;
- row labels for the bracket lanes;
- optionally a second stimulus inside the refractory period (the classic follow-up question).

## Owner decisions
1. Should the student canvas carry any disclosure (MAJOR 3)? Recommend: editor note only.
2. Where the relative refractory period and the K+ bracket end (MED 2). Recommend: at the return to rest.
3. Whether to cap the threshold below 0 mV (LOW 3).
4. Text boxes vs sliders (LOW 5).

## Regression and census
- **Suite on my pristine copy of ae36501:** lib **2007 passed, 0 failed, 5 ignored** (980 s), matching the builder.
  `absurd_windows_cannot_run_away` passed on the first run. `every_figure_panel_has_a_faq`, `census_baseline` and
  `print_style_is_grey_only` all passed.
- **Census:** `census_baseline` passes, so the recomputed hashes equal the fixture. The fixture diff adds only
  `action-potential__defaults/__start 8423a3e79404d8d0`, and no existing line changes. Every other figure therefore
  renders byte-identically.
- **Scope:** `git diff --stat 4be4850 ae36501` touches no protected file (screens.rs, theme.rs, fonts.rs, main.rs,
  examples/) and neither Cargo file. No dependency, no saved-format change, no test deleted or loosened elsewhere (the
  `interact.rs` change only adds the id to NO_HANDLES).
- **Compiler:** the module's only warning is the dead test helper `b2f` (minor 6).

## Not checked
- The egui editor live: how the six text boxes and refusals feel, and whether the note area shows the note.
- A real screen reader.
- Print at real printer DPI (only 1x and 1.3x rasters).
- RTL locales and CJK.
- My pixel audit's sweep was 1,428 renders over 7 of the 32 switch masks, not the full 32. The 6,912-render hash sweep
  covered all 32 but checks only whether output changed.
- I did not run `census_dump` per file.
