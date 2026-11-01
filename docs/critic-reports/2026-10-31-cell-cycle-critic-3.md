# Cell cycle, critic 3 (verification), commit c3c58d9

**Verdict: FAIL**: 0 blockers, 0 major, 1 MED, 2 LOW, 5 minor.

How I tested
- Own clone `ccrit3` at c3c58d9 with its own target, plus a release target for timing. My probes are in
  `scratchpad/critic3.rs`, added as a child module of the module's `tests` in my clone only. Mutants ran in a second clone,
  `ccrit3m`. Nothing was committed or pushed.
- Text boxes are resvg ink boxes. Band, end marks, ticks, dots, leaders, polygons and rects are read back from the SVG.
- I proved the audits can fail:
  - critic 2's audit is reused unchanged.
  - My end-mark audit flags an end mark moved onto the G1 label (`CAP/text "G1 (first gap)"`).
  - My finer label search is sound: it finds a place for 400 of 400 figures the module draws.
- Baseline: 0 compiler warnings in cellcycle.rs (the one warning in the build was in my probe file).

## Findings

### MED 1: the key's "(the phase rounded up furthest)" is false in common cycles, and the student rule can be read two ways there
- Repro (defaults otherwise): `g1=2, s=4, g2=4, m=1`.
  - Key shares: 18.1, 36.4, 36.4, 9.1.
  - Key caption: "Percents are rounded half up to 0.1 and made to add to exactly 100.0 by changing G1 by -0.1 (the phase rounded up
    furthest)."
  - Exact shares: G1 18.18 (rounded up 0.018), S and G2 36.36 (rounded up 0.036 each), M 9.09 (rounded up 0.009).
  - S and G2 were rounded up twice as far as G1. The parenthesis is false.
  - G1 is only the furthest of the phases whose hours no other phase shares.
- More repros:
  - `2/2/2/1`: M is changed "(the phase rounded up furthest)". M was rounded up the least: 0.014 against 0.029.
  - `1/2/3/3` has the same fault.
- How often, measured with exact rationals:
  - In my 1,090,625-set sample the parenthesis is false in 43,047 cycles: 12 % of the roughly 347,000 cycles whose shares
    are changed.
  - On a realistic half-hour grid (G1 10–30, S 4–12, G2 1–8, M 0.5–3; 90,270 cycles) it is false in 2,263. That is 2.5 % of
    all cycles and 7.3 % of the 30,907 whose shares are changed.
  - Among integer cycles of 1 to 24 h each it is false in 15,768.
- The student rule reads: "change by 0.1 **the one that was rounded furthest the wrong way** (... the earlier wedge on a tie),
  and repeat ... Only a wedge whose hours differ from every other wedge's may be changed ...; if no wedge can be changed,
  leave the sum as it is."
  - Read in that order, for 2/4/4/1 "the one" is S (S and G2 tie, and S is earlier).
  - S may not be changed, so "leave the sum as it is" (18.2, 36.4, 36.4, 9.1 = 100.1) is a defensible answer.
  - The key says 18.1.
  - The builder's test `student_rule` filters first, then picks. That is one of the two readings, and the only one tested.
- This is the round-2 defect class again: the fix is right for the named case (1/1/1/13, where every phase ties at +0.05),
  but one step over it is wrong.
- Fix direction:
  - Key: "(of the phases with hours of their own, the one rounded up furthest)".
  - Student: put the filter first: "Of the wedges whose hours differ from every other wedge's, change by 0.1 the one rounded
    furthest the wrong way..."
  - Test the parenthesis against all four phases, and pin 2/4/4/1 and 2/2/2/1.

### LOW 1: the arc's end marks are not obstacles in the layout, so a leader can cross or graze one
- Repro A: `g1=25.195 s=0.14 g2=0.07 m=2`, G0 on, Print, Large, key.
  - M's leader runs right across the top of the ring and crosses the start mark at 0.4°: the distance is 0.00 px, a visible T.
  - See `ccrit3-png/cap-leader-a.png`.
- Repro B: `g1=2.51 s=0.63 g2=5.0 m=115`, checkpoints off, marker Metaphase, student, Large.
  - G2's leader passes 0.88 px from the G2 end mark.
  - See `ccrit3-png/cap-leader-b.png`.
- Rate: 2 of 6,060 random drawn figures with the arc. None of the 22,400 drawn grid figures.
- Fix: add the two end-mark segments to `clear()` as obstacles, the same way ticks are (≥ 3 px).

### LOW 2: advice can be one step longer than needed and turn off the arc for nothing
- When the quick search finds a 3-step plan, only one-step fixes are re-checked with the full search. A 2-step plan that
  only the full layout search can draw is missed.
- My full-solve BFS: 3 of 304 three-step plans (4,529 refusals) have a 2-step plan that draws.
- Repro: `g1=648.702 s=0.0001 g2=0.75 m=0.95`, Large, Print, stages of M off, marker Prophase.
  - The module advises: "To draw it: Type 8 hours for S instead of "0.0001". Turn on Show the stages of M. Turn off Show
    interphase."
  - The first two steps alone draw, with the arc. The third step needlessly removes an answer from the figure.
  - Same with `g1=739.30 s=7.684 g2=-2 m=1`, Letters, student, Large, marker Anaphase, strip off.
- Fix: when the quick plan has k ≥ 3 steps, run the full search to depth k−1. Or run the full search at depth 2, as is
  already done for depth 1. Both are cheap: these states are rare.

### Minor
1. **The "interphase dot ≥ 10° from every other leader start" claim does not hold outside the builder's cases.**
   - Grid: 1,722 of 22,400 drawn figures have the dot under 10° from another start, 224 under 5°, minimum 2.5°.
     Example: 1/1/1/300, Large, checkpoints off.
   - Random: 92 of 6,060 under 10°.
   - In pixels the nearest start is still ≥ 15 px away, because the dot is on the band and the wedge dots are mid-ring. So
     this is a wrong claim, not a collision.
   - The quarter-degree grid's "roomiest first" ordering, which decides this, is untested (mutant `inter_grid_by_angle`
     survives).
2. **With checkpoints on, the G2 end mark sits under the G2/M tick and is painted over by the tick's white halo.**
   - The mark is 1.11 px from the tick in all 15,944 drawn grid figures with checkpoints.
   - Raster check at 4x of the mark's part outside the band: Medium 8/10 px dark, Large 0/12, thin M 5/10.
   - Harmless, because the tick marks the end. But "a mark across each end, with or without checkpoints" is true only in
     the SVG source. The test checks the source, not the pixels.
3. **Drawn figures are not cached; only refusals are.**
   - A drawn figure whose label needs the full quarter-degree search is solved in full twice per keystroke (note, then
     canvas), and again by the preview after the debounce.
   - `0.08 / 19.85 / 0.08 / 0.07`, Large, marker Telophase (draws): 430–470 ms per keystroke in debug, about 90 ms in release.
   - Fix: keep the last Ok layout in the same one-entry cache.
4. **Two of my mutants on new code survive.**
   - `caption_plural_off`: the two-change caption ("the phases") is never rendered in a test. 396 of 1.09 M cycles need it.
   - `arc_reason_always`: the generic Layout reason with the arc on was reached 0 times in 40,000 random cases plus the grid.
     It is unreachable as far as I can tell, so this mutant is effectively equivalent.
5. **The FAQ says an arc refusal "tells you to turn it off".**
   - True in every arc refusal I saw (79 of 79 random, 208 of 208 grid).
   - But advice takes the first one-switch fix in fallback order, and "Set the marker to None" comes before "Turn off Show
     interphase". So a marker fix would win if it ever drew. I found no such case.

## Performance (item 4)
- **Release**, single thread, 2,235 refusals including Large, G0 and stages-off markers:
  - median 0.13 ms, p95 0.54, p99 6.0, worst 110 ms;
  - 1 over 100 ms, 0 over 300 ms.
- **Debug**, 2,786 refusals:
  - median 0.31 ms, p99 43 ms, worst 320 ms;
  - 13 over 100 ms, 2 over 300 ms.
  - My debug figures were taken while other jobs loaded the CPU, so they are pessimistic. I did not reproduce the builder's
    1.18 s case.
- **Typing 300.5 into M over 1/1/1, Large, G0, marker Telophase:** ≤ 7 ms per keystroke in debug, 1 ms in release.
- **Judgement: acceptable, not a MED.**
  - The note box recomputes on each keystroke without a debounce (`notes_box` → `notes_for`).
  - Even so, the release worst case (110 ms refusal, about 90 ms for the slow drawn case) is a single frame hitch below the
    150 ms preview debounce, not a visible typing freeze.
  - The smallest improvement is minor 3 (cache the Ok layout too).

## Round-2/3 claims verified (my own measurement)
- **Percent logic** (my rule, exact rationals, nothing shared with `shares`):
  - Sample: 1,090,625 hour sets:
    - 400,000 random thousandths up to 1000 h;
    - 300,000 tie families [a,a,b,c], [a,a,a,b], [a,a,b,b] and all-equal, with rotations and swaps;
    - all 390,625 integer sets of 1 to 25 h.
  - 0 differences in shares or in the list of changes.
  - 0 shares 0.1 or more from exact.
  - 0 cases of equal hours with unequal percents.
  - 4 equal always gives 25.0 each.
- **Read back from the SVG**: 6,016 rendered keys and student copies (ties, Equal wedges with times, Letters, arc on and off):
  - printed shares equal my rule in every one;
  - the key's sentence is exactly the one for what happened: changed to 100: 1,220; changed and still off: 10; printed as
    rounded: 293; plain: 3,770;
  - "exactly 100.0" never appears with a sum other than 100.0;
  - the student rule appears exactly when the key's sentence does;
  - no share or sum leaks into a student copy;
  - 1/1/1/13 now reads "then M by −0.1 ... is changed; ... add to 100.1 rather than 100.0" (but see MED 1 for the
    parenthesis).
- **Comma**:
  - only 1,000 is refused ("1,000", "+1,000", "1,000 h", "1,000 hours");
  - 0,125, 5,500, 12,345 and 01,000 are read as decimals;
  - "1,0000" is refused for having more than 3 decimal places.
- **Fuzz**:
  - 4,000 renders with random strings of 0–5 pieces (Kelvin, ẞ, İ, ﬀ, ǅ, combining marks, full-width, Arabic and
    mathematical digits, NUL, BOM, ZWSP, U+2028, U+10FFFF, RLO, emoji, quotes, <, &) in 1 to 4 random fields at once, every
    field including style and palette;
  - 0 panics through build, warnings, notes_for and read;
  - every SVG parses.
  - Chromosome edges: 2, 1000, " 46" and "00046" draw. 1002, 0, +46, 46.0, −46, 4294967296 and 2^64 are refused, each with
    advice that draws.
- **Interphase arc**:
  - Grid: 27,744 cases (34 hour families, including both critic thin families and the builder's two fine-search cases) × 3
    sizes × key/student × G0 × checkpoints × all 10 marker positions × names × wedges × times, with Colour/Print mixed in.
    - 22,400 drawn: 0 failures in critic 2's audit, and 0 in my arc audit (end marks against text, discs, start dots,
      leaders and polygons; the dot against ticks and end marks).
    - 5,344 refused: 0 "Nothing is drawn", 0 reason mismatches (the arc is named exactly when the figure without it draws),
      208 arc refusals.
  - Random: 8,000 drawn, with only the 2 end-mark contacts in LOW 1.
- **Arc refusals are true**:
  - All 79 arc refusals in 40,000 random cases (10,000 of them forced to Large with the arc on) are unlayoutable on a 0.05°
    grid with the module's 2° room, and also with 1° room.
  - Every one's advice ("Turn off Show interphase.") draws.
- **Advice**: 6,501 refusals over 25 class mixes (junk and non-ASCII hours, chromosome junk, stage markers with the strip
  off, G0 thin, Ratio):
  - 0 with no advice;
  - 0 whose advice, applied by my own parser, fails to draw;
  - 0 advised figures that draw with collisions;
  - 0 multi-step plans where one of 9 single switches draws;
  - plan lengths: 4,480 one step, 1,833 two, 188 three.
- **One search per settings**: mutant `cache_off` is killed by the builder's test.
- **Key and biology**:
  - The key prints "Spindle (M) checkpoint". The tick sits between the Metaphase and Anaphase boxes, which is correct.
  - Interphase = G1+S+G2, M excluded, G0 off the cycle: correct.
- **Leaks**: 1,307 student copies, whole `def().svg()` source. No phase name, "interphase", "spindle", "(m)", share, total,
  "rounded" or "rather than" anywhere. The only ids are `vs-hatch*`; "chromatid" appears only in the allowed caption.

## PNGs looked at
- Cases: default, student, key and student at Large Print with G0, Large, thin M with and without checkpoints, G0 student,
  arc off, 1/1/1/13 key and student, pairs, the fine-search case (key and student), all 9 marker positions, the arc refusal,
  the generic and three-step refusal canvases, and both end-mark contacts.
- The figure is clean and usable in an exam at typical inputs. The end marks read as the arc's ends, and none is mistaken
  for a checkpoint: they are short and thin, and the ticks are long and bold.
- **Biggest remaining gap for a teacher**: the percent rule. The key's justification is false on common small-integer
  cycles (2/4/4/1), and the student sentence can be read to give a second answer. The second gap is that on a tiny arc the
  Interphase number sits on a seam among four other leaders (`cap-leader-b`). It is legible, but it points at a spot rather
  than spanning the arc.

## Mutation (22 real mutants of my own on the round-2/3 code; 19 killed, 3 survive)
- **Killed**:
  - comma: `comma_any_000`, `comma_none`;
  - shares and caption: `bound_strict`, `tiebreak_later`, `caption_ignores_miss`, `caption_dir_swapped`, `student_no_limit`;
  - label places: `inter_margin_only2`, `inter_leftmost_first`, `inter_step_1`, `inter_no_fine`, `inter_edge_zero`;
  - arc reason: `arc_reason_never`;
  - end marks: `caps_one`, `caps_short`, `caps_at_bounds`;
  - refusal cache: `cache_off`;
  - advice: `one_step_check_off`;
  - key text: `spindle_old_name`.
- **Survived**: `caption_plural_off`, `inter_grid_by_angle`, `arc_reason_always` (see minor 1 and 4).
- The builder's own four survivors (`max_steps_6`, `fallback_letters_off`, `label_clear_band`, `quick_none_stays_none`) are
  consistent with what I measured. 0 of my 6,501 refusals needed more than 3 steps.

## Regression
- c3c58d9 touches exactly cellcycle.rs, faq.rs, figures-progress.md and the two census lines `cell-cycle__defaults` and
  `cell-cycle__start`.
- 0 warnings in cellcycle.rs.
- `cargo test --lib -j2 figures::` (my probes skipped): **1648 passed, 0 failed, 4 ignored** (2,721 s). The known flake did not fire.

## Not checked
- The live egui editor and real typing latency (I timed `notes_for` plus `build` in the test binary, debug and release).
- Browser font metrics.
- PDF/DOCX export.
- Whether students actually read the student rule in the second way in MED 1. That is an argument from the wording, not an
  observation.
- The builder's 1.18 s debug case itself; I measured my own worst cases instead.
