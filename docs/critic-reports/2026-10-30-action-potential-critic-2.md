# Critic 2 on 322d3f3: action potential round 2 (editor-note disclosures, brackets to return-to-rest, threshold below 0, minor ticks and grid, leader audit, Print fixes, row labels, exact limits)

A verification pass over `322d3f3`. Its own change is `ae36501..322d3f3` restricted to `actionpotential.rs`, `faq.rs`,
`figures-progress.md` and the census (`db37b61`/`599b9ce` in between are chi-square and docs, not attributed here; the
diff `599b9ce..322d3f3` touches only those four files). Everything ran on my own `git archive 322d3f3` copies with their
own target dirs; the worktree's uncommitted HANDOFF.md was ignored.

My harness (a `critic` module appended to my copy only; scratchpad `critic_ap2.rs`) adds:
- an independent **pixel audit**: each element is rasterized alone with resvg at 2x (real Noto Sans) on a cropped canvas,
  and literal pixel gaps are measured: text vs text (1 px), text vs every mark, disc vs curve (2 px), disc vs
  guide/axis/bracket/arrow (1 px), disc vs disc and disc vs dot, dot vs dot, leader vs curve away from its own dot, leader
  vs stimulus arrow, leader vs axis/ticks, leader vs leader, leader vs other dots and other discs, leader across the
  threshold/zero line (reported separately), and ink outside the canvas. **10,752 renders**: 28 valid trace sets (rest
  -14/-100, threshold -4 and rest+10, peak = threshold+20 and 70, no undershoot, dip 3 and 86 mV, stimulus 0.5 and its last
  allowed time, totals 5.2/6.4/8/9.15/11.9/12, values like -62.3/-48.7/37.5/-85.5/-77.7) x S/M/L x all 32 switch masks x
  failed x answers. Two further sets I wrote were invalid and were correctly refused;
- a 115-line parse/boundary probe (`probe2.txt`), a 12,000-input agreement/leak sweep with my own generator, an 18,048-plan
  axis sweep, a 3,072-render pointer census, a width check;
- 22 mutants on a pristine copy (module tests), plus a pixel sweep of a build with the three admitted-survivor guards
  removed;
- PNGs at 1.5x and the 66 % copy at 1x and 1.3x, looked at with the image reader.

**Verdict: FAIL (0 blocker, 1 major, 2 MED, 4 LOW, 6 minor).**

What holds:
- The axis claim is true at every size and range: 18,048 plans, every labelled step divided into 4 or 5, worst half
  division 5 mV, smallest division 5.7 px.
- The canvas, `<title>`, `<desc>` and aria are silent on every disclosure; the note carries it (12,000 inputs, 0 problems).
- Numbering is contiguous everywhere (3,072 renders, no gap, no duplicate name).
- Every one-decimal boundary is exact. The brackets now end where the curve is back within 1 mV.
- My pixel audit finds 0 text collisions, 0 leader defects and 0 off-canvas ink in 10,752 renders.
- Suite and census are clean, and the module has zero warnings.

What fails is again the exercise one step over the fix:
- With an undershoot, pointer 8 "Return to resting potential" now always sits on the flat resting line, 0.4-1.4 px from
  the level that pointer 2 "Resting potential" names. That is round 1's MAJOR 2 moved from the no-dip example into
  the default.
- In a failed initiation, "return" points two thirds of the way up the bump.
- The early-stimulus widening makes a 1,295 px figure and still parks the resting pointer on the y axis.

## Claim table

| # | Claim | Verdict | Evidence |
|---|---|---|---|
| 1 | MAJOR 1: minor ticks + light grid divide each labelled step into 4 or 5; Large -100/-90/70 no longer -200..150 (span < 290); FAQ precision (half a division, 2-5 mV) pinned | **FIXED, with a leftover** | Axis sweep, 18,048 plans (rest -100..-14 step 2, six threshold gaps, six peak gaps, five dips, S/M/L, phases on/off): steps 5/10/20/50 with 5/5/4/5 divisions in every plan, worst half division **5.0 mV**, smallest division 5.71 px, worst span/trace 1.94. The default at Large is now 20 mV with 5 mV minors (`large-all.png`), not 50. **True at every size and range.** It is finer than the FAQ says in 1,559 plans (5 mV steps with 1 mV minors at rest near -14: 0.5 mV half division), which is harmless. `s9.png` (-100/-90/70, 0.5/5.2, Large): span **280** mV, 50 mV step, 10 mV minors. But the axis still runs **-170..110** with a "−150" label, so 70 mV of empty plot lies below a floor the figure refuses to go under (LOW 1). Grid: light, behind everything, and never read as data in any PNG I looked at. In the 66 % copy it vanishes as disclosed, and the 3.5 px minor ticks remain (`print-student-small-1x-bw66.png`). |
| 2 | MAJOR 2: no-undershoot drops return + hyperpolarisation, key renumbers; no two blanks share an answer; failed "return" on the falling limb; early stimulus widens the plot | **PARTLY** | Renumbering is right: the pointer census (8 traces x 32 masks x fail x S/M/L x key) found **0** gaps, 0 duplicate names and every mark drawn twice; `nodip-student.png` shows 1-6 and A-C. But "no two blanks share an answer" is **false with a dip** (MAJOR 1 below), and in a failed initiation (MED 1). The widening exists, but it does not give the resting pointer room and it makes the figure unprintable at Large (MED 2). |
| 3 | MAJOR 3: whole-SVG phrase search (2x2x2x3x2x32) and a 10,000-input test | **FIXED (test weaker than it looks)** | My 12,000-input sweep (own generator: 0-3 decimals, true minus, comma, BOM+CRLF, mV suffix, Print, all switches) drew 10,383 figures. Every student copy had **no** phrase from my own list in the SVG source. That list covers weak, spike, fail, dip, left out, not drawn, lowest point, resting level and belong, plus every key name including "return", "graded" and "local". 3,832 notes carried the disclosure exactly when the switches asked for it, 0 mismatches. `notes_for` reaches only `figuresui.rs` (the editor's note area), so no other route takes it to a copy. The test's phrase list is closed, though: a mutant that prints "No action potential: all-or-none" on a failed canvas **survives** (LOW 3). |
| 4 | MED 1: placement and audit check leader vs curve/arrow/leaders/discs/dots | **FIXED** | My pixel audit over 10,752 renders found **0** leader contacts of any kind: curve, arrow, axis, other leaders, other dots, other discs. 0 leaders cross the threshold or zero line. The critic-1 case (`s9.png`) is clean. |
| 5 | MED 3: Print dashes solid/long/dash-dot/dotted, bold plus, white-ringed dots, curve read by element, every stroke vs the 66 % cut | **FIXED** | `print-student-small-1x-bw66.png`: the four brackets are told apart at Small 1x, and dots show on the black curve. `zp-print-key-small-1x-bw66.png` (8x zoom): "Na⁺" and "K⁺" are unmistakable at Small 1x 66 %. Mutants: pale curve, pale threshold, plus at 0.7 and plus not bold are all **killed**. |
| 6 | MED 4: row labels "Channels", "Refractory period" | **FIXED** | Labels sit level with each disc row, left of the axis, inside the canvas at all sizes (`large-student.png`, `small-student.png`). minor 5 covers what "Refractory period" gives away. |
| 7 | MED 2 / owner 2: K+ and relative-refractory end within 1 mV of rest, pointer 8 after them | **FIXED as specified** | Default: K+ 1.800..4.696 ms and D 2.952..4.696 ms, the curve at **-71.000** mV at both ends. The no-dip K+ ends at the end of the fall (-70.000). The bracket-end mutants (trough+1, REST_TOL 3, relative ends at the trough) are all killed. Textbook check: see Biology. The consequence one step over is MAJOR 1. |
| 8 | Owner 3: threshold ≥ 0 refused, and within 4 mV; rest tops out at -14 | **FIXED** | `-4`/`-4.0` accepted; `-3.9` refused "within 4 mV"; `-0`, `-0.0`, `0.0`, `+0` and `0.01` refused "at or above 0 mV". Rest `-13.9` refused "above the −14 mV limit"; `-14` accepted with threshold -4. Mutant killed. |
| 9 | Exact billionths; one-decimal boundaries never refused; messages quote digits typed; negative total "is not a time"; timing advice brute-forced; dead code gone | **PARTLY** | One-decimal boundaries are exact: every probe in the table below is right, and the float mutant is killed. Two gaps remain. A **"not a time" refusal advises 5.2 ms**, which fails for the default stimulus (LOW 2). **Quotes are wrong** past 12 integer digits or 9 decimals (minor 3). `b2f`, the empty block and `let _ = xstep` are gone. `dist_seg` is now `use composite::seg_dist as dist_seg`, while the duplicate `seg_cross` is kept. |
| 10 | 46 mutants, 43 killed; survivors change no render | **PARTLY** | My 22: **18 killed, 4 survived** (table below). The disc-clearance guard, which the builder lists as a survivor, is **killed** in my run by `the_family_never_overlaps…`. A pixel sweep with the leader-leader and leader-dot guards removed: see the guards note. |
| 11 | Suite 2024/0/5; census moved only the two AP ids | **CONFIRMED** | See Regression. |

## Defects (ranked)

### MAJOR 1: with an undershoot, "Return to resting potential" (8) points at the same flat line as "Resting potential" (2)
- **Input:** the defaults (`-70 / -55 / 40 / -80`, stimulus 1, total 8), any size, any copy (`student-all.png`, `default`).
- **What is drawn.**
  - Dot 2 is at 0.5 ms on the rest line (-70.000 mV).
  - Dot 8 is at **5.446 ms**, where the curve is at **-70.199 mV**, 0.4 px below dot 2's level.
  - The curve has been visibly flat since about 4.7 ms.
  - In `student-all.png` dot 8 sits on a horizontal stretch identical to dot 2's.
- **Measured across inputs.**
  - Pointer census: return dot 0.20-0.98 mV below rest and 0.4-1.4 px from the rest level, in every spike trace with a
    dip, at every size.
  - Agreement sweep: in **2,408** of the drawn figures with both pointers, dot 8 is within 1 px of dot 2's row.
- **Why it matters.** "Resting potential" is a correct answer for blank 8, so the key's "Return to resting potential"
  is one of two defensible names. This is round 1's MAJOR 2, moved from the bundled "No undershoot" example into the
  default figure.
- **Why it happens.**
  - The owner's decision ends the brackets within 1 mV of rest and puts 8 after them.
  - At this scale 1 mV is under 2 px, so anything after the brackets is on the flat line.
- **Tests.** `resting_and_return_point_at_different_places_in_every_trace` asserts only that the two dots are 20 px apart
  in x, which a flat line satisfies.
- **Owner decision needed.**
  - (a) Anchor 8 on the recovering limb (for example halfway back from the trough, inside D/B) and accept that it
    precedes the bracket ends.
  - (b) Keep the decision and accept "resting potential" for 8 in the key.
  - (c) Drop 8 whenever there is a dip, too.

### MED 1: in a failed initiation, "Return to resting potential" (5) points two thirds of the way up the bump, beside "Local (graded) response" (4)
- **Input:** Failed initiation on, defaults (`failed-student.png`, `late-failed-large.png`).
- **Measured.**
  - Dot 4 is at the top of the bump (-61 mV).
  - Dot 5 is 0.55 ms later at **-64.04 mV**: 66 % of the bump height and 11-12 px above the rest line (census: 0.54-0.66
    of the bump in every trace).
- **Why it matters.**
  - The whole bump is the local (graded) response, so "local response" is defensible for 5.
  - "Return to resting potential" names a place that is visibly not at rest.
- **Crowding at Large.** With stimulus 7.3 / total 12 (`late-failed-large.png`), discs 4 and 5 sit side by side under the
  bump with a 2.8 px gap.
- **Tests.** A mutant that anchors 5 at the top of the bump (`tb + 0.05`) **survives**: nothing checks that 5 is
  distinct from 4.
- **Fix.** Point 5 at the tail (≤ 20 % of the bump, still above the rest line by > 1 px), or drop "return" in a failed
  initiation.

### MED 2: the early-stimulus widening makes the figure 1,295 px wide and still parks the resting pointer on the y axis
- **Input:** stimulus 0.5, total 12, Large, phases on (`early-long-large.png`).
- **What is drawn.**
  - Canvas **1,295 x 735 px**, with 6-12 ms empty.
  - Dot 2 is at **t = 0.05 ms** (the last anchor, 0.1·ts), 7 px from the y axis. Its disc is squeezed between the axis,
    the arrow and the rising limb.
  - Disc 4 is 1-2 px from the rising limb (`zoom-disc4.png`; 24 sweep renders).
- **Widening does not happen at all for a short trace.** `s9.png` (0.5 / 5.2, Large) gets no widening and has the same
  dot 2 at 0.05 ms on the axis.
- **On a page.**
  - Fitted to a 6.5 in (624 px) column, the Large early-long figure's 16.8 px text prints at **8.1 px**, below the native
    Small size.
  - Small early-long is 840 px wide, so its 9.6 px text prints at **7.1 px**, under the module's own 9 px floor.
  - Medium is 991 px wide, giving 7.6 px.
  - Widening starts at Large as soon as ts = 0.5 with total 8 (912 px).
- **Why it happens.** `LEAD_DISCS * r + LEAD_EXTRA` (48 px at Large) is too little room before the arrow for a disc plus
  its leader and the arrow clearance. The search then falls through to the 0.1·ts anchor.
- **Fix.**
  - Give the lead stretch a fixed minimum px width without scaling the whole time axis (for example a minimum stimulus x
    of about 3r + arrow gap + leader), or place the resting disc above the line.
  - Cap the canvas width.

### LOW 1: the Large extreme still draws 70 mV of empty axis under a -100 floor
`s9.png`: -100/-90/70, Large. The axis runs **-170 to 110** with "−150" labelled, and the curve never goes below -100.
The claim (span < 290, nothing at -200) holds. The empty band is about a quarter of the plot (`need_lo_rest` for the
arrow and disc 1 sets it).

### LOW 2: the "not a time" refusal advises a total that is refused next
`stimulus 1, total -5` gives "Type a total time of **5.2** to 12 ms". Typing 5.2 then gives "a stimulus at 1 ms leaves
only 4.2 ms… at least 5.7 ms". With stimulus 7.3 the advised range would have to start at 12. The brute-forced timing
advice covers only the "needs about" path; this path quotes `MIN_LEAD + NEED` regardless of the stimulus. Use
`ts + NEED` when the stimulus is valid.

### LOW 3: the leak test is a closed phrase list, and a synonym gets through
Mutant: on a failed canvas, add `<text>No action potential: all-or-none</text>`. **Survives** all 38 module tests.
- "action potential" is not in `NAMES` or `DISCLOSURES`.
- My `desc` mutant ("subthreshold stimulus") was caught only because "subthreshold" contains "threshold".

A structural assertion would close it: the set of `<text>` strings of a failed student copy equals that of a successful
student copy minus the tick labels, and the `<desc>` is identical between fail on and off.

### LOW 4: deep dip with the last allowed stimulus — "return" points at the last pixel of the trace
Input: `-14 / -4 / 70 / -100`, stimulus 7.3, total 12 (`deep-late.png`).
- K+ and D end at **11.986 ms**, and dot 8 is at **11.993 ms**, at the plot's right edge, with the curve still visibly
  climbing (it is 2.9 mV below rest 0.5 ms earlier).
- "Back at rest" is never shown: the figure satisfies "8 after the brackets" by 0.007 ms.
- `NEED` (4.7 ms) is only just enough for an 86 mV dip (settles in 2.49 ms after the trough). A dip-dependent minimum
  total (or 1 ms of settled line) would show the return.

### minor 1-6
1. Disc-to-curve clearance is 1-2 px in two families:
   - Large early-long: disc 4 against the rising limb, 24 renders (`zoom-disc4.png`).
   - Small failed `-100/-4/70`, 0.5/12: 16 renders.
   The search measures against the 5 px coarse polyline, whose chords cut inside a steep limb.
2. Failed Large late (MED 1): discs 4/5 are 2.8 px apart. My 2 px Chebyshev check flagged 16 renders; visually they do not
   touch.
3. Quoting:
   - More than 12 integer digits saturate: `-123456789012345678` is quoted as "−1000000000000 mV", and total
     `99999999999999999999` as "1000000000000 ms".
   - A 10th decimal rounds: `-60.0000000005` is quoted "−60.000000001". `-60.0000000004` and `-3.9999999999` are accepted
     as -60 and -4.
   - Harmless numerically, but "quotes the digits typed" is not exact.
4. Lenient reads: `-,5` reads as -0.5 and `-70,` as -70.
5. The row label "Refractory period" supplies two of the three words of C and D. In the no-dip figure the only bracket in
   that row is C, so "refractory period" alone is a defensible answer for it (`nodip-student.png`). Owner may prefer
   "Periods" or no label in that row.
6. Code:
   - `seg_cross` still duplicates `math.rs`.
   - `dist_seg` survives as an alias.
   - The module doc still says a disc is "clear of the curve, the guide lines, the other discs", without leaders.
   - No stale caption, `T_LINGER` or "1 ms after the lowest point" text remains in code, FAQ or progress doc.

## Probe table (selected; all in scratchpad `probe2.txt`)
- **Read:** `-70 mV`, `-70mV`, `−70`, `-70,0`, BOM+CRLF, tabs, NBSP before mV, `- 70 mV`, `-70 MV`, `1,5`, `1.5ms\r\n`,
  `8ms`.
- **Refused as not a number:** `-70 m V`, `-7O`, `NaN`, `inf`, `1e999`, `1e1`, `8 s`, `8 mV` (time), `-70mv` + ZWSP.
- **Refused as too large:** 30 and 400 nines.
- **Refused as ambiguous:** `1,500`, `8,500`.
- **Exact at the boundary** (accepted / refused):
  - rest -14.0 / -13.9; -100.0 / -100.0001;
  - threshold -4.0 / -3.9; threshold -0, -0.0, 0.0, +0, 0.01 all refused "at or above 0";
  - threshold -60 / -60.1 (rest -70); peak -35 / -35.1, 70.0 / 70.1;
  - undershoot -70.0 (no dip) / -70.1 and -72.9 (too shallow) / -73.0; -100 / -100.1;
  - stimulus 0.5 / 0.4; total 12.0 / 12.1; 0.5+5.2 / 5.1; 7.3+12 / 7.4+12; 2.35+7.05 / 7.0499.
- **Applied each timing suggestion**:
  - "at least 5.7 ms": renders.
  - "move the stimulus to 0.9": renders.
  - "3.3 ms or earlier": renders.
  - "0.5 to 7.3 and at least 4.7 after": renders.
  - The exception is LOW 2.
- **Panics: none.**

## Mutants (22, pristine copy, module tests)
| Mutant | Result |
|---|---|
| K+ and relative end at trough + 1 ms | killed (band test) |
| REST_TOL 3 mV | killed (3 tests) |
| relative ends at the trough | killed |
| minor division 2 for every step | killed (2) |
| minor division 10 for 1/5 steps | killed |
| no-dip keeps "return" | killed (4) |
| `<desc>` adds "subthreshold stimulus, no action potential" | killed, only via "threshold" ⊂ "subthreshold" |
| **canvas adds "No action potential: all-or-none" when failed** | **SURVIVED** (LOW 3) |
| Print curve `#bbbbbb` / threshold `#bbbbbb` | killed / killed |
| plus 0.7 not bold / plus 0.7 | killed / killed |
| threshold ≥ 0 refusal off | killed |
| one-decimal comparisons in floats | killed |
| timing `<=` → `<` / Either says total only | killed / killed (2) |
| **leader-crosses-leader guard off** | **SURVIVED** |
| **leader-touches-dot guard off** | **SURVIVED** |
| disc-clearance guard off | **killed** (`the_family_never_overlaps…`; the builder lists it as a survivor) |
| widen-plot off | killed (4) |
| **failed "return" at the bump top** | **SURVIVED** (MED 1) |
| failed "return" on the flat line | killed |

**Guards note.** I ran my 10,752-render pixel sweep on a build with all three of the builder's admitted-survivor guards
removed.
- Leader-crosses-leader and leader-touches-dot never matter: **0** "leaders touch" and **0** "leader touches another
  dot" findings. They are redundant, as the builder says, but untested.
- Removing the disc-clearance guard produces **32** renders where a leader runs into another disc:
  - they come from set 7 (`-14/-4/70/-100`, 7.3/12) Large failed and set 10 (`-100/-90/70/-100`, 0.5/5.2)
    Medium/Large failed;
  - example: a leader from (464.6,174.0) into the disc at (453.7,205.3).
  - That guard therefore matters, and a module test does catch it.
- Every other finding equals the pristine sweep's 64.

## Biology (OpenStax A&P 12.4, Campbell 48.3, from memory)
- Na+ from threshold to the peak: correct.
- K+ from the peak to back at rest, after the undershoot: correct. Voltage-gated K+ channels are slow and close as the
  membrane returns.
- Absolute refractory from threshold to where the falling limb crosses rest: an accepted simplification. Some texts end it
  earlier on the falling limb.
- Relative refractory from there through the hyperpolarisation to back at rest: correct now (`student-all.png`: D spans
  the trough and ends where the curve is flat).
- A beside C reads sensibly. A and C now carry different row labels, so the round 1 ambiguity (A = "depolarisation" or
  "absolute") is gone.
- The no-dip omission of the relative period stays as round 1 recorded.

## Teacher view (PNGs looked at)
- **Default and student, Medium** (`student-all.png`):
  - A clean, conventional figure with a 5 mV grid that makes -55 and -80 readable.
  - Blank 8 (MAJOR 1) would cost a teacher an argument.
- **Large and Small student** (`large-student.png`, `small-student.png`): the 20 mV step and 5 mV minors at Large are a
  real fix; nothing crowds.
- **Failed initiation** (`failed-student.png`):
  - The canvas no longer gives the answer.
  - Pointer 5 is the weak spot (MED 1).
  - The axis keeps the spike's -100..60, so the bump is 27 px tall. That is consistent and arguably intended.
- **No undershoot** (`nodip-student.png`): 1-6 and A-C, and the return and hyperpolarisation pointers are gone. Good.
- **5.2 ms trace** (`short`): fine at Medium. At Large, dot 2 sits on the y axis (`s9.png`, MED 2).
- **Print at 66 %** (`print-student-small-1x-bw66.png`, `zp-print-key-small-1x-bw66.png`):
  - The black curve, the dashed threshold, the white-ringed dots, four distinct bracket dashes and "Na⁺/K⁺" all survive.
  - The grid vanishes as disclosed, and the minor ticks remain.
- **Would a teacher drop it into an exam?**
  - For "name the parts" at the defaults: yes, if the key accepts "resting potential" for 8.
  - For reading values: yes now, to about ±2.5 mV at Medium.
  - Not the early-long Large figure, which prints its text at 8 px.
- **Biggest remaining gap:** blank 8 has two right answers in every figure with a dip.

## Owner decisions
1. MAJOR 1: where "Return to resting potential" points, given decision 2 (recovering limb, accept "resting potential",
   or drop it).
2. minor 5: whether the "Refractory period" row label should name less.

## Regression and census
- **Suite** on my pristine copy of 322d3f3: lib **2024 passed, 0 failed, 5 ignored** (900 s).
  - `absurd_windows_cannot_run_away` passed first time.
  - `census_baseline`, `print_style_is_grey_only` and `every_figure_panel_has_a_faq` all passed.
- **Census:** `census_baseline` passes, so every other figure is byte-identical to the fixture.
  - `git diff 599b9ce 322d3f3 -- tests/fixtures/figure_census.txt` changes only `action-potential__defaults/__start` (now
    `f5a9272a232c8038`).
- **Scope:**
  - `git diff ae36501 322d3f3 --stat` touches no protected file (screens.rs, theme.rs, fonts.rs, main.rs, examples/) and
    no Cargo file.
  - `action-potential` is still in NO_HANDLES, and the FAQ is present.
- **Compiler:** **0** warnings from `actionpotential.rs`; the lib-test build's 22 warnings are all in other files.
- **Tests:** 25 → 38 in the module.
  - None deleted; four were renamed with new assertions.
  - The old band test's two loose checks (±1 px at threshold, ±0.8 at the peak) were replaced by stricter mV-at-pixel
    checks.
  - The FAQ precision test pins strings, and the tick test pins the numbers. Together they are real (both minor-division
    mutants are killed).

## Not checked
- The live egui editor (text boxes, note area).
- A screen reader.
- Real printer DPI.
- PDF/DOCX export.
- RTL/CJK.
- My pixel sweep's Chebyshev 2 px metric over-flags diagonal near-misses (minor 2); I confirmed those by eye.
- I did not recompute census hashes per file beyond `census_baseline`.
