# Critic 4 on c1e47bb: action potential round 4 (decisions C and D)

I checked out `c1e47bb` into my own clone (`crit4`) and made two `git archive` copies: `crit4h` for a harness (a child module of the
module's `tests`, so it reuses the SVG readers) and `crit4m` for mutants. Each copy had its own target dir. The repo was not
edited. I did not re-raise anything critics 1-3 list as fixed.

**Verdict: PASS (0 blocker, 0 major, 0 MED, 3 LOW, 5 minor).**

Decisions C and D hold everywhere I measured, which covered about 230,000 plans. This is the first round where the fix was
not wrong one step over. What is left is advice text that predates round 4 and was not updated for the new lead refusal,
a failed-initiation "every pair ≥ 24 px" claim that the code does not enforce (it holds only by luck), and gaps in the tests.

## What I ran
- **Module suite** (`cargo test --lib -j2 actionpotential`): **49 passed, 0 failed**, 359 s. The build showed **0 warnings from
  `actionpotential.rs`**; the lib-test build has 22 warnings, all in other files.
- **Scope.** `git show --stat c1e47bb` touches only `actionpotential.rs`, `faq.rs`, `figures-progress.md` and the census.
  - The census diff moves only `action-potential__defaults/__start` (0de0a055… → d01e1646…).
  - `mod.rs` (`warnings` arm) and `interact.rs` are untouched.
  - I did not re-run `census_baseline` or the full 2,100-test suite.
- **Harness sweeps** (debug build, threaded). Every drawn figure was measured from its own SVG, with axes read back from the
  tick marks and dots found through key row, disc mark and leader:
  - **typical**: 9,648 plans. Six textbook value sets x every stimulus 0.5-3 x totals 5.2-12 x S/M/L x 4 switch masks x
    failed on/off.
  - **grid**: 142,560 plans. Rest -100..-14 (11 values) x threshold +10/+15/+25 x peaks thr+20/30/50/70 x dips
    0/3/3.5/4/5/6/8/10/15/30 x 9 timings x S/M/L x masks 31/5/13/30 x failed on mask 31.
  - **shallow**: 78,975 plans. Dips 3.00-7.00 mV in 0.05 steps x 8 rests x 2 thresholds x 3 peaks x 3 timings x S/M/L x 3
    masks.
  - **pairs**: 64,395 plans, every dot pair under 26 px logged.
  - **leaks**: 7,680 student copies. These are the 20 family sets x 32 masks x fail x S/M/L x Colour/Print. I collected
    every alphabetic word in the whole SVG source and ran the module's `markup_leaks`.
  - **edge probe**: 49 inputs, plus a probe of the read-level timing advice.
- **Geometry, two layers.**
  - The builder's `audit()` ran on every drawn figure.
  - My own extra checks: discs vs the y-axis line, disc and dot vs the filled arrowhead triangle (edges, not just vertices),
    dot vs the arrow shaft, any non-leader line through a disc ring, and dot vs the y axis.
- **Refusals.**
  - Every refusal was classified.
  - Every piece of advice was applied and planned. For "or later" and "or less" I tried 5 values across the claimed range.
  - The "least" claim was checked one tenth before the value offered.
  - The quoted lead was checked against `ts/dur x` the plot width measured from a drawn sibling.
- **Mutants.** 18 were run on `crit4m` against 8 targeted tests.
- **PNGs.** 1.5x and 66 % bitonal renders, looked at with the image reader.

## Claim table
| Claim | Verdict | Evidence |
|---|---|---|
| C: pointer 2 is always on the flat line before the stimulus, ≥ 12 px from the axis, ≥ 8 px from the arrow (4 px with no arrow) | **CONFIRMED** | Over 230k plans, the resting dot was never after the stimulus. It was always within 0.5 px of the resting level (from the SVG). Least gap to the y axis: 12.000 px. Least gap to the stimulus with the arrow: 9.46 px (with no arrow: 4.18 px). 0 audit and 0 own-geometry problems. M15b (post-spike anchor plus guard removed) is killed by 2 tests. |
| C: fixed lead 20.05/21.55/25.35 px; canvas capped | **CONFIRMED** | Widest canvases are 621.24 / 633.55 / 702.70 px, the same for every stimulus and total. The smallest text in a 624 px column is 9.60 px. The quoted lead matches my measurement within 0.15 px in every lead and crowded refusal (0 LEAD-QUOTE, 0 LEAD-FALSE). |
| C: no-room refusal is true, and its advice draws | **CONFIRMED for the planner's own advice; LOW 1 for the older read-level advice** | Lead refusals (3,168 in the grid, all Large at 0.5 ms with a total of 11.2 ms or more, or crowded rows) and crowded refusals (367): 0 advice failures. "or later" and "or less" held at all 5 sampled values, and the least/longest claim held. The crowded refusal for -30/-15/40/-60 0.5/12 Medium draws once the threshold line is off, so "guide lines" in its reason is true. |
| D: return dot at 0.5 of the dip, on the recovering limb, ≥ 4 px off rest, ≥ 24 px from every dot | **CONFIRMED** | Share error at most 0.0010 (SVG rounding). Least offset from rest: 3.994 px (the 4.0 rule at 2-decimal SVG precision). Least distance to dot 7: 28.26 px. Dot 7 was never off the trough (share 0.000) and never after it. In every spike render, dot 8 is right of the trough. |
| D: shallow refusal only when the halfway dot is < 4 px; the message is true; the advice works | **CONFIRMED (minor 1)** | 20,618 shallow refusals in the shallow sweep and 16,323 in the grid. Every one has its advice applied, with **0 failures**. There were no "no clear place" refusals at all in any sweep, so no return failure is blamed on the wrong cause in the valid space I covered. Dips are quoted as typed ("3.0004"). Not least: minor 1. |
| Failed initiation: no two dots within 24 px; local never on the falling side | **PARTLY (LOW 2)** | The local dot was never right of the bump's top (all 11k "LOCAL-FALLING" hits were my own flat-top artefact, re-checked: 0 real). The pair claim fails by 0.02 px in one case and is not enforced for rest vs local (LOW 2). |
| Student copy: no answer word in any attribute, id, class or comment | **CONFIRMED** | Over 7,680 student copies the only alphabetic words anywhere in the source are drawing vocabulary plus "action, potential, membrane, neuron, time, ms, mv, channels, periods, pointers, name, to, for, in, against". There are no comments, no aria, no class, and no `data-`. M11 (comment) and M12 (aria-label) are both killed by the new test. |
| FAQ says what the code does | **CONFIRMED, with minor 3** | The 20/22/25 px lead, Large 0.5/12, the halfway rule, 24 px and the 4 px rule all match the code. Stale text is in the module's doc comment, not the FAQ (minor 3). |
| Typed digits quoted | **CONFIRMED** | `-98.0004` gives "(3.0004 mV below rest)". M14 (rounded) is killed. `-72.999999999`, `-70.000000001`, `0.499999999`, `12.000000001` and `5.699999999` are all exact. |

## Defects

### LOW 1: the read-level timing advice offers a stimulus or total that the new lead refusal then rejects
- **Reproduce.** Label size Large, stimulus marker and phase pointers on (the defaults), with:
  - stimulus `0` (or `0.3`, or `-1` with total 12) and total `12`. The figure says "Type 0.5 ms or later." 0.5 ms with 12 ms
    at Large is refused next ("leaves only 23.5 px … needs at least 25.4 px").
  - stimulus `0.5` and total `0`, `-5` or `5`. It says "Type a total time of 5.2 to 12 ms" or "Make the total time at least
    5.2 ms". **Nine** of the totals in that range are refused: 11.2 through 12.
  - stimulus `0.5` and total `13`. It says "Type a shorter time"; 12, the obvious next value, is refused.
  - stimulus `8` or `100` and total `12`. It says "Move the stimulus to 7.3 ms or earlier", which includes 0.5, and 0.5 is
    refused.
- **Small and Medium:** 0 failures in the same probe.
- **Why it matters.** Decision C promises "advice that actually draws". These messages come from `read()`/`timing_fix`, which
  are one step upstream and were not updated for the planner's new refusal. The second refusal is true and its advice works,
  so the teacher gets there in two steps.
- **Fix.** Pass label size and switches to the timing advice, or phrase the range from the planner, as `early_refusal`
  already does. Alternatively, start the stimulus hint and FAQ "0.5 ms or later" range at the least that draws for the size.

### LOW 2: "in a failed initiation every dot pair is at least 24 px apart" is not enforced; it holds by margin, and missed once by 0.02 px
- **Reproduce.** Values `-100 / -90 / 70 / -100`, failed on, stimulus 0.6, total 12, Large, all switches.
  - The resting dot and the local-response dot are **23.98 px** apart.
  - Moving the stimulus to 0.65 gives 25.04 px.
  - Render: `c4png2/c4-fp.png`. Dot 4 is on the rising side of a 7 px bump, about 0.35 ms after the arrow. The arrow tip sits
    between dots 2 and 4.
- **Cause.** `RETURN_FROM_DOT` (24) is applied only when placing the return dot. The local dot is kept only `2*DOT_R+3` =
  9.4 px from the resting dot. When the top of the bump is blocked, the local anchor falls back to `tb - 0.35`, which is
  ≥ 8 + 15 px from the resting dot at 12 ms.
- **Tests.** Mutant M04 (drop the 24 px rule for the failed return dot) **survives**. The new test
  `no_two_dots_of_a_failed_initiation_are_close…` uses `sets()`, which contains 0.6/12 only with `-100/-4`, not `-100/-90`,
  so it never meets the case.
- **Fix.** Apply the 24 px distance to every dot in a failed initiation, or drop the claim from the progress note. The FAQ
  only says local vs return, which is true.

### LOW 3: the fixed minimum lead is pinned only to ±0.3 px by the tests
- Mutant **M09**, `LEAD_SLACK` 0.05 → -0.6, **survives** all 8 targeted tests.
  - The 0.3 px tolerance in `the_resting_pointer_is_always_before…` and its coarse stimulus/total grid never put a lead inside
    the 0.65 px window.
  - So the exact need (20.05 / 21.55 / 25.35 px) is untested. `the_advice_for_a_stimulus…` walks tenths, but on only two
    value sets.
- **Rendering impact** (my sweep): the arrow-gap minimum is 9.46 px, so nothing visible hangs on this today.
- **Fix.** Assert `lead >= need` exactly (no 0.3 slack) on a dense tenths walk, or assert the resting dot is at least 8 px
  from the arrow with no tolerance.

### minor
1. **The "deeper undershoot" advice is not always the least, to the tenth.**
   - Repro: `-90 / -80 / 40 / -94.45`, any timing, Small. The figure says "Type a deeper undershoot, −94.6 mV", but −94.5
     draws (201 cases in the shallow sweep).
   - Cause: `round(dip*10)+1` skips the tenth when the typed dip ends in .05-.09.
   - Fix: use `floor(dip*10)+1`.
2. **The shallow refusal's "lower the peak" advice can be a large change.**
   - Repro: `-95 / -85 / 70 / -98` offers only "Lower the peak to −15 mV" (85 mV lower); `-97/-87/70/-100` is the same.
   - It is true, but it rewrites the exercise. A teacher would rather be told "the dip must be at least about 5 mV at this
     scale".
3. **The module doc comment (lines 18-22) is stale.**
   - It still says the resting pointer is "before the stimulus if there is room, otherwise after the spike has settled". It
     also describes the return pointer only as "4 px off".
   - It is a Rust `//!` comment, not SVG, so nothing leaks, but it contradicts decision C.
4. **The new tests moved their own fixtures out of the refusal zones.** `sets()` changed `-73` → `-75` and `0.5` → `0.6`.
   The family sweep therefore no longer renders a 3 mV dip or a 0.5/12 trace at any size. It is legitimate given the new
   refusals, but the Small and Medium 0.5/12 figures, which still draw, lost their audit coverage in that test. My sweep
   covered them: 0 problems.
5. **Advice searches are heavy.**
   - Debug timings: lead refusal 46 ms, crowded 380 ms, shallow 320-390 ms, against 2-4 ms with no advice.
   - `plan()` runs twice per refused state (`build` and `warnings`).
   - In release this is probably 20-40 ms per keystroke. I did not drive the editor.

## Things looked at that are not defects of this round
- **Depolarisation (4) and peak (5) only 15.7 px apart** at `-30 / -20 / 0 / -60`, 0.55-0.65/12, Large (peak exactly on the
  zero line). This is the existing 9.4 px dot rule, not round 4, and the dots are on clearly different features
  (`c4png2/c4-dp.png`).
- **Failed wide Large (`-14/-4/70/-100` fail 0.8/12):** leaders 2 and 4 drop about 130 px straight down, past disc 1. They are
  readable but ugly. This is the extreme critic 3 already listed (LOW 1 there).

## Mutants (18; targeted tests: the 6 new ones, plus the FAQ test and `placement_never_fails`)
| # | Mutant | Result |
|---|---|---|
| M01 | `RETURN_SHARE` 0.5 → 0.42 | killed (`resting_and_return…`) |
| M02 | `RETURN_FROM_DOT` 24 → 14 | **survived** (disclosed as equivalent at 0.5; my sweep's least 7-8 distance is 28.26 px, so it is equivalent for spikes) |
| M03 | local-response anchor on the falling side first | killed (`no_two_dots…failed…`) |
| M04 | 24 px rule dropped for the failed return dot | **survived** (LOW 2) |
| M05 | deeper advice starts 0.3 deeper | killed (`a_dip_too_shallow…`) |
| M06 | deeper advice offered unchecked | killed (`a_dip_too_shallow…`) |
| M07 | later-stimulus advice unchecked | killed (`the_advice_for_a_stimulus…`) |
| M08 | shorter-total advice not the longest | killed (`the_advice_for_a_stimulus…`) |
| M09 | `LEAD_SLACK` 0.05 → -0.6 | **survived** (LOW 3) |
| M10 | arrow gap 8 → 6 for the resting dot | killed (`the_resting_pointer_is_always…`) |
| M11 | `<!-- … -->` in the student copy | killed (`a_student_copy_has_no_word…`) |
| M12 | `aria-label` in the student copy | killed (`a_student_copy_has_no_word…`) |
| M13 | crowded refusal folded into the generic one | killed (`the_advice_for_a_stimulus…`) |
| M14 | shallow dip quoted rounded | killed (`a_dip_too_shallow…`) |
| M15 | post-spike resting anchor added (guard kept) | survived: an equivalent mutant (the guard skips it) |
| M15b | post-spike anchor added and the guard removed | killed (2 tests) |
| M16 | every return failure blamed on a shallow dip | **survived** (disclosed; my sweeps found 0 "no clear place" refusals in the valid space, so there is no visible effect) |
| M17 | shallow message quotes 2x the halfway px | killed (`a_dip_too_shallow…`) |

13 of 18 were killed. M02 and M15 are equivalent mutants. The real survivors are M04, M09 and M16; M16 was disclosed.

## Teacher view (PNGs)
- **Default student, Medium** (`c4-default-student.png`): clean.
  - 2 sits on the flat line before the arrow and 7 is at the trough.
  - 8 is visibly halfway up the recovery, well right of 7.
  - All 8 blanks have one answer. I would hand it out.
- **Print at 66 %** (`c4-print-student-bw66.png`): curve, dashes, dots and bracket dashes all survive.
- **Large 0.6/12 student** (`c4-large-early-student.png`):
  - The leader of 2 goes straight up and crosses the red threshold line, so disc 2 sits just above that line.
  - Disc 3 is in the right margin and 2's dot is plainly on the rest line, so I read it as unambiguous. It is still the
    weakest layout I saw.
- **Small 0.5/12 student** (`c4-small-early-student.png`): dot 2 is wedged between the axis and the arrow tip, about 20 px of
  flat line. Tight, but readable.
- **Failed key and student:** fine at the defaults.
- **Failed extreme:** see above.
- **Refusals:**
  - The lead, crowded and shallow canvases are 420 px wide with a true reason and working advice.
  - The 5 mV dip at `-90/-80/70/-95` Large is now refused. It drew in round 3; this is the recorded decision D trade-off.

## Edge inputs (all as expected, no panic)
- **Refused:** NaN, inf, `1e3`, `1e2`, `1e-1` and `1e9` (not numbers); empty and blank (empty); 10 decimals; 12 nines (out of
  range); `12 s`.
- **Read:** `0,5` and `12 ms`.
- **Exact at the boundaries:** -14 / -13.999999999, -4 / -3.999999999, -35 / -35.000000001, 70.000000001, -73 / -72.999999999,
  -100.000000001 and 0.499999999.
- **Saved format:** no param changed in this round. An old save with a numeric `F` value in a text field reads as "empty",
  which predates this round. A save missing keys depends on the load path filling defaults; I did not trace that path.

## Not checked
- The live egui editor and release-build timings.
- The full lib suite and `census_baseline` (I used the census diff and the scope only).
- Screen readers, real printers, PDF/DOCX export.
- Text extents: I used the builder's Arial table plus my own disc and line geometry, not rasterised text boxes.
- Leader-to-curve clearance beyond the builder's audit.
- Values between my grid points: tenths for stimulus 0.5-1.2 and dips at 0.05, otherwise coarse.
