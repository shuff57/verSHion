# Cell cycle, critic 2 (verification), commit d624a25

**Verdict: FAIL**: 0 blockers, 0 major, 1 MED, 5 LOW, 4 minor.

How I tested
- Own clone `ccrit2` at d624a25 with its own target. My probes are in `scratchpad/critic2.rs`, which I added as a child module of the
  module's `tests` for the runs only. Mutants ran in a second clone, `ccrit2m`. Nothing was pushed or committed.
- Text boxes come from resvg ink boxes. The band, ticks, dots, leaders, polygons and rects are read back from the SVG.
- I proved the audit can fail. A G2 label moved onto the band is flagged four ways (BAND/text, leader/text, text on ring, text/tick).
- Baseline at d624a25: 43 module tests pass, and cellcycle.rs has 0 compiler warnings.

## Findings

### MED 1: in the three-equal-phases exact-tie family, the key caption contradicts itself and the student rule gives a different answer
- Repro (defaults otherwise): `g1=1, s=1, g2=1, m=13`. Also 2/2/2/26, 13/1/1/1 and 1/13/1/1 (any permutation).
- Key shares: 6.3, 6.3, 6.3, 81.2. They add to 100.1.
- The key caption contains both of these sentences:
  - "...made to add to exactly 100.0 by changing M by -0.1 (the phase rounded up furthest)."
  - "Phases with equal hours show equal percents, so the shares are printed as rounded and add to 100.1 rather than 100.0."
- The first sentence is false: the shares do not add to 100.0. The second is also false: one share was changed, so they are not
  "printed as rounded".
- The student caption has no "stay within 0.1 of the exact share" limit. It says to change the wedge rounded furthest the wrong way
  and "repeat until they do", and to leave the sum only when the remaining wedges are the equal ones.
  - M is a wedge with its own hours, and it "was rounded" up by 0.05.
  - A student who follows the printed rule changes M twice: 81.1, sum 100.0. The key says 81.2.
  - So the blank has two defensible answers.
- Measured:
  - From the SVG, 160+ distinct inputs print "made to add to exactly 100.0" with a sum of 100.1.
  - A direct search of `shares()` over [a,a,b,c]-type hour sets (384,000) found 1,000 where a change is made and the sum still
    misses 100.0.
  - It needs three equal phases whose shares end exactly in 5. Small integers hit it: 1/1/1/13.
- Fix direction:
  - When a change is made and the sum still misses, say what happened: "changed M by -0.1; the equal phases keep equal percents,
    so they add to 100.1". Do not say "exactly 100.0".
  - Add "never more than 0.1 from its exact share" (or "change each wedge at most once") to the student rule.
  - Add 1/1/1/13 to the tests. No current test or mutant pins this case.

### LOW 1: the comma check refuses unambiguous decimal commas, with a false reason, and the advice discards the value
- These are all refused as "ambiguous: ... could be a thousands separator": `0,125`, `0,001`, `5,500`, `12,345`, `2,375`,
  `1000,000`, `-1,000`.
- None of them can be a thousands separator inside the 1000 h limit:
  - "0,125" has a leading 0 group.
  - "5,500" read as thousands is 5500 h, which is out of range anyway.
- Only `1,000` is really ambiguous (1000 h or 1 h).
- The hint advertises decimal commas ("5,5") "to the thousandth". A decimal-comma user typing thousandths is refused every time.
- The advice is "Type 11 hours for G1 instead of "0,125"", which throws the number away.
- In de1fedd these inputs drew.
- My mutants `comma_any_digits` (`f.len() >= 3`) and `comma_dot_ignored` both survive.
- Fix: refuse only when both readings are in range, i.e. a is 1 to 3 digits without a leading 0 and a·1000+f ≤ 1000, which is
  just "1,000". Otherwise read it as a decimal comma.

### LOW 2: band refusals are not limited to "thin G1+S+G2 arcs", and some say "cannot be placed" when a placement exists
- With defaults and only the hours changed, the band refuses a figure that draws without it:
  - `300/1/1/1` at Large (arc ≈ 345°);
  - `1/300/1/1` at Medium and Large (arc ≈ 355°);
  - `1/1/1/300` and `1/1/1/77` at every size.
- `0.258/138.52/0.269/0.18` Large is also refused, and its arc covers almost the whole ring. The cause is three thin wedges plus two
  ticks crowding one spot, not a thin arc. The builder's note and the FAQ ("if G1, S and G2 are so narrow together...")
  describe this wrongly.
- Rates (band on vs off; the advice is always "Turn off Show interphase."):

  | Inputs | Drew without the band | Refused with it |
  |---|---|---|
  | Realistic hours (G1 1-30, S 4-12, G2 1-8, M 0.5-3) | 4,938 | 0 |
  | Log-uniform 0.05-1000 h, random settings | — | 12 |
  | Log-uniform, only the hours typed | — | about 127 |
  | Both log-uniform sets combined | 5,513 | 139 (2.5 %) |
  | My family grid | — | 144 |

- I re-ran `arrange()` with the label angle on a 0.25° grid over the whole arc, keeping the same 2° clearance. 3 of 12
  band-only refusals (log-uniform set) lay out (all at 270.25°, Large). Example: `25.170/0.115/0.054/0.396` Large.
  - So "cannot be placed round this ring" is false there. The integer grid plus `cands.truncate(30)` misses the slot.
  - `inter_one_try` (`truncate(1)`) and `inter_margin_only10` both survive, so the candidate search is untested.
- The reason sentence never names the band. Only the advice reveals it. The specific "The interphase arc covers only X degrees"
  reason fires only when no candidate exists at all, which is rare.
- Fix direction:
  - Search a finer grid without the 30 cap (the cost is small next to the BFS).
  - Name the band in the reason ("The Interphase label cannot be placed...").
  - Correct the FAQ and progress note.

### LOW 3: the spindle blank still has more than one textbook answer
- The single rule plus "checkpoint" fixes the shape.
- But OpenStax Biology 2e and Campbell both call it the **M checkpoint** ("also known as the spindle checkpoint"), and "metaphase
  checkpoint" is common too.
- The key prints only "Spindle checkpoint" ("Spindle" in Letters). An OpenStax class will write "M", correctly.
- Fix: key "Spindle (M) checkpoint", or add a caption line in the key.

### LOW 4: the interphase blank is attached to one point of the band, beside another answer
- The label leaves from a dot on the band at the angle nearest the arc's middle:
  - default: 175°, 10° past the G1/S tick, over S;
  - Equal with G0: 125°, 10° from S's own leader at 135°;
  - pairs 1/1/199/199: 87°, 10° from G2's leader at 97.5°.
- In the student copy, numbers 8 and 2 (or 7 and 3) sit stacked next to each other and point at the same region.
- The band has no end marks:
  - At 0° it simply starts.
  - With a thin M (thinM PNG) the gap above M is about 6°, so the band reads almost as a full outer ring.
- A student can label 8 as "S" or "G1/S". This is not a strict two-answer blank, but it is weak as an exam item.
- Fix: short radial end caps at both ends of the band. Attach the label at the middle of the largest clear stretch, not the
  nearest angle to the arc's centre.

### LOW 5: refusal renders are slow because the BFS runs twice (build and notes)
- Debug build:
  - build+notes up to 735 ms (`0.08/19.85/0.08/0.07`, Large, Telophase marker);
  - 8 of 1,795 refusals over 300 ms;
  - build alone up to 402 ms, 28 of 4,258 over 200 ms.
- Release will be several times faster. I did not measure release.
- Fix: cache the refusal per params hash, or compute it once for both callers.

### Minor
1. The hint for Show interphase says "An arc **inside** the ring". The band is drawn outside (r+6), as the FAQ says.
2. Old specs (no `interphase` key) load with the band on.
   - 294 of 298 drawable old specs renumber the strip and spindle discs (+1).
   - 4 of 298 are now refused.
   - Harmless only because the figure has not shipped. Say so in the progress note, or default the key off for specs saved
     before d624a25 (`migrate`).
3. Mutation survivors on new code (see below):
   - `bfs_two`: the 3-step cap is never needed by any test (the builder's own MAX_STEPS 3->6 survivor is the same gap).
   - `fallback_letters_off`.
   - `inter_no_gap_rule`, `inter_margin_only10`, `inter_one_try`.
   - `label_clear_band`: the 11k label clearance from the band is never binding in the tests.
4. With checkpoints off, the band's G2 end has nothing marking it. Same root as LOW 4.

## Critic-1 fixes verified (my own measurement)
- **MAJOR panic: fixed.**
  - 2,516 renders with odd strings in **every** field (g1, s, g2, m, chromosomes, pointer, readout, names, wedges, label_size,
    style, palette) × 4 setting sets, plus wrong value types in five fields: 0 panics through build, warnings and notes_for.
  - Strings included: Kelvin, ẞ, İ, ﬀ ligature, ǅ, combining marks, full-width Ｈ, BOM, ZWSP, U+2028, U+10FFFF, NUL, Cyrillic о.
  - "1 Hour" reads 1 h; "1hHOURS" and "1 hr." are refused as not a number.
- **Advice: fixed.**
  - 4,258 random refusals across 25 class mixes, with every advice sentence parsed and applied by my own parser:
    - 0 with "Nothing is drawn";
    - 0 whose advice fails to draw;
    - 0 advised draws with collisions;
    - 0 longer than one step when one of 9 single switches draws.
  - The critic-1 repro sets give one step (marker to None; Equal), as claimed.
  - The Ratio reason lists every phase under 0.1 %.
  - The share check is gated on show_times.
- **Percents: fixed for the named case.**
  - 7,000 SVG read-backs, both band settings: 0 mismatches against my exact-rational implementation of the stated rule.
  - 0 equal-hours/unequal-percent cases.
  - The "add to X rather than 100.0" claim appears exactly when the sum ≠ 100.0, with the right X.
  - Exception: the contradiction in MED 1.
- **Spindle blank**: one rule plus "checkpoint" (mutant `spindle_row_slash` is killed).
- **Marker**: the triangle stays inside its wedge on raised 5° wedges, and the caption explains it.
- **"1,000"** is refused. This over-reaches (LOW 1).

## The interphase band (new)
- **Biology**
  - Interphase = G1+S+G2, and M is excluded: correct.
  - G0 sits off the arc as a box reached by an arrow that crosses the band. That is honest: G0 is outside the cycle, and neither
    the key nor the caption claims otherwise.
- **Collisions** (resvg ink boxes): 0 in both sweeps.
  - Family grid: 17,400 drawn renders = 27 hour families × 3 sizes × key/student × G0 × checkpoints × 6 markers × names × wedges
    × show_times.
  - Random: 10,000 drawn renders, random settings including Print.
  - Checks include: text, disc, polygon and rect on the band; leaders running along the band; slanted legs crossing it; leaders
    within 1.5 px of another leader's start dot; plus every critic-1 check.
- **Leaks**: the student copy and `def.svg()` (title/desc) were scanned in 780 student copies.
  - No phase name, "interphase", share or total anywhere in the source.
  - The only ids are `vs-hatch0..5`. "chromatid" appears only in the allowed caption.
- **Student copy**
  - The band is drawn with a numbered disc and one rule.
  - "Interphase" is the one answer, with the weakness in LOW 4.
  - Numbering: wedges, G0, ticks, interphase, strip, spindle.

## PNGs looked at (1.3x plus 0.66 bitonal "print66")
- Cases: default, student, student+G0, key/student at Large Print, Small, thin M (Telophase), band-thin at Large and with G0,
  equal key with G0, equal student, pairs 100.2, G1-thin Large, Letters student, all 9 marker stages, band off, and the
  1/1/1/300 and "0,125" refusal canvases.
- Clean and exam-usable, apart from LOW 4 and the MED 1 caption.
- **Biggest remaining gap for a biology teacher:**
  - The percent rule still has an unstated limb (MED 1).
  - The interphase label points at a spot rather than spanning the arc (no bracket or end caps).
  - Synonym names for the checkpoints (M / G1 / G2 checkpoint, per OpenStax) are not in the key.

## Mutation (25 real mutants of my own, on the new code; 17 killed, 8 survive)
- **Killed** (14 on new code):
  - `strip_unit_no_trim`, `alone_only_earlier`, `tie_later_wins`, `bound_loose`, `residual_sign`, `residual_only_short`
  - `ratio_grammar`, `ratio_on_proportional`, `fallbacks_first`, `ratio_lower_off`
  - `inter_from_s`, `inter_name_not_bold`, `inter_dot_on_ring`, `inter_numbered_last`
- **Killed** (3 on round-1 fixes): `marker_base_wider`, `marker_caption_g0_wrong`, `spindle_row_slash`.
- **Survived:**
  - `comma_any_digits`, `comma_dot_ignored`
  - `bfs_two` (BFS depth 2)
  - `fallback_letters_off`
  - `inter_margin_only10`, `inter_no_gap_rule`, `inter_one_try`
  - `label_clear_band`

## Regression
- d624a25 itself touches exactly cellcycle.rs, faq.rs, figures-progress.md and the two census lines (`cell-cycle__defaults`,
  `__start`).
- The range de1fedd..d624a25 also contains f6bc4e1 (solubility curves round 2, its own census lines). That is a different figure's
  commit, not this one.
- 0 warnings in cellcycle.rs.
- Old specs load (`params_from_spec`) and render (see minor 2).
- Full `cargo test --lib -j2 figures::` (critic2 probes skipped): **1624 passed, 0 failed, 4 ignored**, including census_baseline and the FAQ tests.

## Not checked
- The live egui editor, typing latency in release builds, and browser font metrics.
- PDF/DOCX and pair export.
- Whether band-refusal rates matter for real classroom inputs: 0 of 4,938 realistic.
- Release-build timing.
