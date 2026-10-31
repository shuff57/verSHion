# Critic 3 on 59bef50: action potential round 3 (return pointer on the recovering limb, no plot widening, neutral row label, structural leak test)

This is a verification pass over `59bef50`. Its own change is `322d3f3..59bef50` restricted to `actionpotential.rs`,
`faq.rs`, `figures-progress.md` and the census. `f4175bb` in between is another session's reaction-energy commit and is
not attributed here: `git diff f4175bb 59bef50 --stat` touches only those four files. Everything ran on my own
`git archive 59bef50` copies with their own target dirs. I ignored the worktree's uncommitted HANDOFF.md.

My harness (`c3h.rs`, a child of the module's `tests`, so it reuses the builder's SVG readers) adds:
- **A pointer census.** 885 renders (30 trace sets x S/M/L x fail x 5 masks). It logs where the resting, return,
  hyperpolarisation and local-response dots sit, in px from the rest line and from each other.
- **A typical-values grid.** 12,150 plans: rest -70/-65/-60, threshold -55/-50, peak 30/40/50, undershoot -75/-80/-90,
  nine timings, S/M/L, five masks, fail.
- **A resting-pointer census on typical values.** 11,016 figures.
- **A refusal grid.** 192,420 valid plans: rest -100..-14 step 5, threshold rest+10, five peaks, dips 3.0..9.0 mV in
  0.1 mV steps plus 10/12/15/20/30, six timings, S/M/L, two masks. Each "too shallow" refusal was checked twice:
  - against a brute-force walk of the recovering limb (2,000 steps) for a dot at least 4 px off rest and 14 px from
    every other dot;
  - against the module re-run with a finer anchor walk (k = 0.30..0.97 in 0.01 steps) and the same rules.
  Each piece of advice was also applied.
- **A parse probe** (79 inputs) and a **12,000-input agreement and leak sweep**. The leak sweep covered visible text,
  title, desc, aria-label, ids, classes, `data-*` and comments.
- **A pixel geometry audit** with literal ink gaps and text measured by rasterizing each text element with resvg at 4x.
  It covers text vs text, text vs every other ink (1 px), disc vs the drawn curve, discs, lines and arrowheads, leaders vs the curve away from their dot, other leaders, other dots, other discs, the arrow and text, and ink off the canvas. **6,756 renders**: 30 trace sets (extremes, non-integers, no dip, 3-5 mV dips, 0.5/12, 7.3/12, 5.2 traces) x S/M/L x 16 of the 32 switch masks x fail x answers, plus Print.
- **23 mutants** on a pristine copy, scored by the module tests.
- **PNGs** at 1.5x, the 66 % bitonal copy at 1x and 1.3x, and a 624 px column copy, looked at with the image reader.

**Verdict: FAIL (0 blocker, 1 major, 2 MED, 2 LOW, 4 minor).**

What holds:
- Owner decision A is met to the letter: pointer 8 is on the recovering limb, inside the K+ and D brackets, at least
  4.0 px off the rest line in every spike render. The late-return trace is fixed.
- The canvas is no longer widened: 621 / 634 / 703 px at S/M/L, and the smallest text prints at 9.6 px or more.
- "Periods" and "Channels" are in place.
- The parser rules, the "not a time" advice and the structural leak test work.
- Suite and census are clean.

What fails is again one step over the fix:
- **MAJOR 1.** The plot is no longer widened. Instead, in **27 %** of figures drawn from typical textbook values, the
  "Resting potential" pointer now sits on the flat line **after** the spike, to the right of 7 and 8. A student then has
  two defensible readings of 2 and 8.
- **MED 1.** The new refusal is **false in 972 of its 2,122 cases**: the module itself draws 863 of them once its anchor
  walk is finer.
- **MED 2.** On a 3-5 mV dip, dots 7 and 8 sit side by side on the same few-pixel trough.

## Claim table

| # | Claim | Verdict | Evidence |
|---|---|---|---|
| 1 | Pointer 8 ≥ 4 px off rest, 14 px from other dots, on the curve, inside the brackets; late return fixed; failed: on the tail ≥ 2.5 px, ≤ 45 % of the bump, ≥ 10 px right of and lower than dot 4; shallow dip on a wide scale refused with a true reason and working advice | **PARTLY** | **Spike.** Census: every spike render has the return dot 4.0-88.8 px off rest, before `t_settled` and inside B and D. The default is at 3.846 ms, 10 px above the trough's level 20 px (`default-key`). **Late return.** `-14/-4/70/-100`, 7.3/12: dot 8 at 10.146 ms, 62.6 px off rest, the brackets end at 11.986. Fixed. **Failed.** frac 0.28-0.44 of the bump, dev 2.69-5.12 px, dx 23-112 px. The literal claim holds. **The refusal is not "true"** (MED 1), and the 14 px rule does not make 7 and 8 distinguishable on a shallow dip (MED 2). |
| 2 | No widening; widest 621/634/703; smallest text ≥ 9 px in 624; resting pointer after the spike when the lead is short; no dot within 12 px of the y axis | **FIXED as built; consequence is MAJOR 1** | Canvas widths are identical for 0.5/12, 0.5/5.2, 0.5/8 and 4/12 at each size (621.24 / 633.55 / 702.70). The smallest text is 9.6 px (Small, 9.6 x 624/621). Sweep: widest 621.24 / 633.55 / 702.70, worst print 9.6 px (Small). Closest dot to the y axis: 16.2 (census, 0.5/8 Small; the 12 px floor holds) px. The early-long Large figure fits a page (`print-early-long-large-student-1x-bw66.png`). The empty 5-12 ms stretch is wasteful but sane. |
| 3 | "Not a time" quotes stimulus + 4.7; brute-force applied at every stimulus | **FIXED** | Probe: ts 0.5 → "5.2 to 12", 1 → "5.7", 3.3 → "8", 7 → "11.7", 7.29 → "11.99", 7.3 → "Type a total time of 12 ms." Each is the least the same stimulus accepts. Mutant M05 (old 5.2) is killed by 2 tests. |
| 4 | Structural leak check | **FIXED (narrower than described)** | The synonym caption (M06) is killed by the new test, and so are a name in `<title>` and a sentence in `<desc>` (built into the test). It is not the whole net, though. `strays()` accepts **any** text that parses as a number, so a typed value as a label (M07) passes it; an older test kills that mutant. It never reads attributes or comments: the aria mutant M09 is **survives**, and the comment-synonym M09b is **survives**. My 12,000-input sweep over text, title, desc, aria, ids, classes, `data-*` and comments found **0** leaks and 0 numbers outside the tick set. The note carries the disclosure exactly when it should (3,002 noted, 0 mismatches) and every refusal (2,108). |
| 5 | `-,5`, `-70,` refused; > 12 integer digits or > 9 decimals refused, never rounded | **FIXED** | `-,5`, `-70,`, `-70.5,` and `,` are "not a number". `-123456789012` is quoted exactly; `-1234567890123` is "too large to read". `-70.123456789` is read exactly; `-70.1234567891` gives "more than 9 decimal places". `-70.1234567890` and `-70.0000000000` are accepted, because trailing zeros are the same number. Boundaries `-3.999999999` / `-4.000000001`, `70.000000001`, `0.499999999` and `12.000000001` are all exact. BOM, CRLF, tab, NBSP, en dash, true minus and `-70 mV` read. NaN, inf, 1e999, 0x46, Arabic digits, ZWSP, `-7 0` and `+-70` are refused. No panic. M12-M14 are all killed by the new parser test. |
| 6 | Disc vs the 2 px curve copy, literal 2/3 px gaps; dead guards deleted; literal 3.6/6 px gap test on every render | **CONFIRMED (see geometry)** | My audit: **0 problem renders in 6,756**. Least ink gaps: text-text 7.26 px, disc-curve 2.67 px, disc-disc 2.98 px, leader-curve away from its dot 1.25 px (Large), leader-leader 13.24, leader-dot 9.94, leader-disc 8.36, leader-arrow 15.97, 0 text pixels within 1 px of other ink, nothing off the canvas. The fine-polyline guard matters as built: M11 (gap r+1.5) is killed, but M11b (back to the coarse polyline) survives as disclosed, and with it my sweep was not re-run, so I cannot say it never matters. |
| 7 | Suite 2050/0/5, zero module warnings, census moved only the two AP ids, 26 of 27 mutants killed | **CONFIRMED** | Suite: **2050 passed, 0 failed, 5 ignored** (998 s), `absurd_windows_cannot_run_away` passed first time. `census_baseline` and `every_figure_panel_has_a_faq` pass. 0 warnings from `actionpotential.rs`. The census diff `f4175bb..59bef50` changes only `action-potential__defaults/__start` (f5a9272a232c8038 → 0de0a055bf82fa73). Test count 38 → 45, none deleted. |

## Defects (ranked)

### MAJOR 1: in a quarter of typical figures "Resting potential" (2) points at the flat line after the spike, where "Return to resting potential" is the textbook name
- **Input:** the defaults with stimulus 0.5 and total 8, Medium (`post-0.5-8-med-student.png`). Also stimulus 1,
  total 12, Large (`post-1-12-large-student.png`), and failed initiation 0.5/8.
- **What is drawn.**
  - Dot 2 is at 6.3 ms on the settled line, right of dot 8 (3.35 ms, -75 mV, still 10 px below rest) and dot 7.
  - The left-to-right order on the curve is 4, 5, 6, 7, 8, **2**.
- **Why it matters.**
  - The flat line after the recovery is exactly what most texts label "return to resting potential" (or "resting
    potential restored").
  - Dot 8 is still hyperpolarised: it is 5 mV below rest in the default and the K+ bracket is still open over it.
  - So a student can defensibly write "Return to resting potential" for 2 and "Hyperpolarisation / undershoot" for 8.
  - Owner decision A placed 8. Moving 2 after the spike was the builder's choice, and it reopens the very ambiguity
    critics 1 and 2 raised.
  - In a failed initiation the same move puts "Resting potential" on the line after the bump, where "returned to rest"
    is equally natural (`failed-wide-large-key.png`).
- **How often.** My census of 11,016 figures from typical values (rest -70/-65/-60, peak 30/40/50, undershoot
  -75/-80/-90, stimulus 0.5-2, total 6-12, S/M/L) puts the resting dot after the spike in **2,964 (27 %)**:
  - every figure with stimulus 0.5 and total ≥ 8 at Medium, and with total ≥ 6 at Large;
  - stimulus 0.6/0.8 with total ≥ 9 at Medium/Large;
  - stimulus 1 with total 12 at Large.
  - 260 of my 885 census renders do it too.
- **It also happens where the lead has room.** At 0.5/7 Medium the lead is 37.1 px (≥ the 36.5 px `lead_ok` needs), yet
  21/54 figures go after the spike. The strict pass walks every anchor, post-spike ones included, before the relaxed
  pass reconsiders the pre-stimulus ones. The FAQ's "when the stimulus is too early to leave room" is therefore not the
  whole rule.
- **Tests.**
  - `resting_and_return_point_at_different_places_in_every_trace` accepts a resting dot after the settled time.
  - Mutant M18 (resting dot **always** after the spike) is killed (2 tests that pin the default resting dot before the stimulus), but nothing asserts *which* figures may move it.
- **Fix / owner.** Either keep 2 before the stimulus (a disc above-left of the arrow, or an anchor at ts·0.5 with a
  longer leader up over the threshold line), or say in the key what each means. Otherwise the owner must accept
  "Return to resting potential" for 2 in those figures.

### MED 1: the new "too shallow" refusal is false in 46 % of its cases, and the planner itself draws most of them
- **Input (one of many):** `-70 / -60 / 70 / -73`, stimulus 0.5, total 12, Small, all switches. It is refused: "The
  undershoot (3 mV below rest) is too shallow at this scale … at least 4 px from the resting line."
- **Grid.** 192,420 valid plans; **2,122** are refused for "too shallow" (dips 3.0-4.2 mV, peak 30-70, every size).
  - In **972** of them my brute-force walk finds a point on the recovering limb at least 4.0 px off rest and 14 px from
    every other dot. In the example: t = 3.022 ms, 4.02 px off rest.
  - In **863** of those 972, the module's own planner draws the figure once its seven fixed fractions
    (0.5/0.4/0.6/0.7/0.3/0.8/0.9) become a 0.01 walk of the same range, with every other rule unchanged.
  - The other 109 still fail. The dot fits, but the disc or leader does not, so the reason given ("too shallow …
    4 px") is the wrong one. This is the builder's disclosed survivor "every return failure blamed on a shallow dip",
    and here it matters in real renders.
- **Typical values are never refused.** 0 of 12,150 plans with undershoot -75/-80/-90 are refused. The refusal bites
  3-4.2 mV dips: -73 or -74 with rest -70 and a +50/+70 peak, and the wide-scale -95/-98 kind.
- **Advice.**
  - Applying "deeper" (1 mV deeper, or to -100), "lower peak" (threshold + 20) or "phase pointers off" rendered in every
    refused case. **0 advice failures.**
  - At rest -97..-98 "deeper" is not available (the floor is -100). The other two options remain.
- **Fix.** Walk the limb finely (or bisect for the 4 px level), and name the limiting rule.

### MED 2: on a shallow dip, "Hyperpolarisation" (7) and "Return to resting potential" (8) sit side by side at the bottom of the dip
- **Input:** `-90 / -80 / 70 / -94`, 1/8, Large, refractory on (`shallow94-large-student.png`). Census row: dot 7 is
  4.92 px below rest and dot 8 4.43 px below, **14.4 px** apart. Their discs nearly touch.
- **Other cases.**
  - `-70/-55/40/-73`, 0.5/12, Large: 5.49 px / 4.39 px, 14.9 px apart.
  - The bundled-style `-70/-55/40/-73`, 1/8: 27.5 px apart, but still 6.0 vs 4.2 px off rest.
- **Why it matters.**
  - Both dots are at the trough as far as the eye can tell (1 px apart vertically on a 5 px dip).
  - 8 is not visibly "returning". Only the order 7 → 8 separates them, so "undershoot" is a defensible name for 8.
- **What the limits test.** The 4 px / 14 px floors are met, and they guard against the wrong thing: the return dot
  should be visibly higher than the trough dot, not just off the rest line.
- **Fix.** Require dot 8 to be at most about half of the dip's pixel depth and at least one disc diameter right of
  dot 7, or refuse (with a true reason).
- **Tests.** Mutant M19 (return anchor tried nearest the trough first) **survives**, and so does M16 (`RETURN_FROM_DOT` 14 → 6), so the 14 px rule between 8 and the other dots is not tested by anything.

### LOW 1: in a failed initiation on a wide scale, "Local (graded) response" (4) is not at the top of the bump and sits 3 px above "return" (5)
- **Input:** `-14 / -4 / 70 / -100`, fail, 0.5/12, Large (`failed-wide-large-key.png`).
- **Measured.** The bump is 7.7 px tall. Dot 4 is on the falling side at 5.7 px (the `tb + 0.45` fallback, because the
  top anchor is blocked). Dot 5 is 2.69 px above rest, 23 px right.
- **Why it matters.** The two discs share one 8 px wiggle. The test's "≥ 10 px right and lower by 10 % of the bump"
  passes with a 3 px difference.
- **Scope.** This is an extreme scale, and the axis keeps the spike's -140..100 range.

### LOW 2: the structural leak test is not structural for numbers, attributes or comments
- `strays()` passes any text that parses as `f64`, so a typed value printed as a label passes it. This includes
  "NaN" and "inf", which Rust parses.
- It never reads `aria-label`, ids, classes or comments. M09 (aria-label "graded potential only" on a failed canvas):
  **survives**. M09b (comment "no spike here"): **survives**.
- My sweep found no such leak in the actual code. This is a test-quality gap, not a drawing defect.

### minor 1-4
1. The FAQ says the resting pointer goes after the spike "when the stimulus is too early to leave room". It also does
   so with room (see MAJOR 1, 0.5/7 Medium).
2. The FAQ says the failed return is "well away from the pointer to the local response". At LOW 1 it is 3 px lower
   and 23 px right.
3. A test was loosened: the 2-to-8 distance went from `> 20` px to `>= 14` px, and the extra `across >= 14 || up >= 14`
   is implied by the 14 px distance check above it.
4. The "too shallow" message rounds the dip to 3 decimals (`show(((rest-under)*1000).round()/1000)`), so a 3.0004 mV
   dip reads "3 mV". This is harmless.

## Answerability, all blanks (default, no dip, shallow, deep, failed, early-long, late-return)
- **1 Stimulus, 3 Threshold, 4 Depolarisation, 5 Peak, 6 Repolarisation:** one answer each in every case I looked at.
  - Failed initiation: 4 is the bump's top (local response). There is no "peak"/"depolarisation" pair, so no clash on a
    short bump.
- **2 Resting / 8 Return:** two readings in the post-spike layouts (MAJOR 1). One reading when 2 is before the
  stimulus.
- **7 / 8:** separable only by order on 3-5 mV dips (MED 2). Clear on the default (8 is 10 px above 7's 20 px).
- **A-D:** Channels A/B and Periods C/D. Each label is true of both brackets, and the letters are told apart by
  position (threshold-to-peak vs peak-to-rest; threshold-to-rest vs rest-to-settled). No second defensible answer.
  - The no-dip figure has only C under "Periods". "Refractory period" as an answer for C is acceptable (it is the
    absolute one).

## Geometry (my own audit)
6,756 renders, 0 problem renders (literal 1 px text gaps, 1 px disc-curve ink gap, 1.5 px disc-disc, 1 px leader gaps). Closest: leader to the curve 1.25 px at Large, disc to the curve 2.67 px, two discs 2.98 px (Large, shallow dip: discs 7 and 8, MED 2). 84 refusals, all "too shallow" for `-97/-87/70/-100` (rest 3 mV above the floor, so "make the undershoot deeper" cannot be followed; the other two options work). Widest canvases 621.24 / 633.55 / 702.70; smallest printed text 9.6 px. The deleted leader-leader and leader-dot guards: the nearest pair of leaders is 13.2 px and the nearest leader-to-dot 9.9 px, far above 3.6 / 6, so their removal does not show in any render I made.

## Mutants
| Mutant | Result |
|---|---|
| M01 pointer 8 back on the rest line (old anchors, 0 px) | killed (5 tests) |
| M02 failed return at the bump top (frac 1.0) | killed (7) |
| M02b failed return at 60 % of the bump | killed (3) |
| M03 widening restored | killed (1: the width test) |
| M04 shallow-dip refusal removed | killed (1) |
| M05 old 5.2 advice | killed (2) |
| M06 synonym caption on a failed canvas | killed (2, incl. the structural test) |
| M07 typed threshold printed as a label | killed (3, **not** by the structural test) |
| M08 comment "failed initiation: subthreshold" | killed (4, by the phrase lists only) |
| M09 aria-label "graded potential only" on a failed canvas | **SURVIVED** (LOW 2) |
| M09b comment "no spike here" | **SURVIVED** (LOW 2) |
| M10 row label back to "Refractory period" | killed (2) |
| M11 disc-to-curve gap r+1.5 | killed (1, `placement_never_fails…`) |
| M11b disc guard on the coarse polyline | **SURVIVED** (disclosed) |
| M12 comma rule off | killed |
| M13 > 12 whole digits saturate | killed |
| M14 10th decimal silently dropped | killed |
| M15 dots allowed on the y axis, pre-stimulus first | killed (width test) |
| M16 `RETURN_FROM_DOT` 14 → 6 | **SURVIVED** (untested rule; MED 2 risk) |
| M17 every return failure blamed on a shallow dip | **SURVIVED** (disclosed; MED 1 shows it matters) |
| M18 resting dot always after the spike | killed (2) |
| M19 return anchor tried nearest the trough first | **SURVIVED** (MED 2) |
| M20 `RETURN_SEP_V` 4 → 2 | killed (3) |

17 of 23 killed. Of the six survivors, two are disclosed (M11b, M17), and M17 is a real risk: it produces the false reason in 109 grid cases (MED 1). M16 and M19 leave dot 8's distance from dot 7 untested (MED 2). M09/M09b show the "structural" test reads only text, title and desc.

## Teacher view (PNGs looked at)
- **Default, student, Medium and Small** (`default-student.png`, `student-small-1.3x-bw66.png`):
  - Clean.
  - 8 is visibly on the way up from 7.
  - "Periods" reads neutral.
  - I would use it.
- **Large student** (`student-large`): fine, 703 px wide.
- **Failed initiation** (`failed-student.png`):
  - 4 is at the top of the bump and 5 is on the tail at 28 %. Good at the defaults.
  - Weak on a wide scale (LOW 1).
- **No undershoot** (`nodip-student`): 1-6 and A-C, as in round 2.
- **5.2 ms trace** (`short52-student`): resting pointer before the stimulus. Fine.
- **Early-long** (`print-early-long-large-student-1x-bw66.png`, `post-0.5-8-med-student.png`):
  - Prints legibly now.
  - The resting pointer is after the spike (MAJOR 1). This is the figure I would not hand out without changing the key.
- **Print at 66 %** (Small 1x, Large, failed, no-dip, shallow): black curve, dashed threshold, four bracket dashes,
  white-ringed dots, including dot 8 and dot 2 on the flat line (zoomed 4x).
- **Refractory on:** C/D under "Periods" with distinct dashes in Print.
- **Biggest remaining gap:** MAJOR 1. Blank 2 has two defensible names whenever the stimulus is early in a long trace,
  and teachers commonly choose an early stimulus to show a long recovery.

## Owner decisions
1. MAJOR 1: keep "Resting potential" before the stimulus always (the planner must make room there without widening),
   or accept "Return to resting potential" for blank 2 in those figures.
2. MED 2: whether dot 8 must sit visibly above the trough (a fraction of the dip), at the price of more shallow-dip
   refusals.

## Regression and census
- **Suite** on my pristine copy: lib **2050 passed, 0 failed, 5 ignored** (998 s).
  - `absurd_windows_cannot_run_away`, `census_baseline` and `every_figure_panel_has_a_faq` all passed.
- **Scope.** `git diff f4175bb 59bef50 --stat` touches only the four files. No protected file, no Cargo file, no param
  or saved-format change.
- **Census.** `census_baseline` passes, so every other figure is byte-identical to the fixture. The fixture diff vs
  `f4175bb` moves only `action-potential__defaults/__start` → `0de0a055bf82fa73`. I did not recompute hashes file by
  file.
- **Compiler:** 0 warnings from `actionpotential.rs`.
- **Tests:** 38 → 45, none deleted. One check loosened (minor 3), one assertion widened to accept the new refusal's
  wording (legitimate).
- **FAQ.** True of the code for "Periods", the 4 px rule, "never widened" and the shallow-dip refusal. It is not quite
  true on when the resting pointer moves (minor 1) or on "well away" (minor 2).

## Not checked
- The live egui editor; a screen reader; real printer DPI; PDF/DOCX export.
- My scratchpad was wiped (tmpfs cleared) midway. The numbers above for the census, typical grid, refusal grid,
  agreement sweep, probe and PNGs come from the first run, recorded before the wipe. The geometry sweep and mutants
  M09-M20 ran after it; the geometry sweep used 16 of the 32 switch masks.
- I did not prove that a deleted leader-vs-leader or leader-vs-dot guard is needed; the sweep's closest leader gaps
  (above) are the evidence.
