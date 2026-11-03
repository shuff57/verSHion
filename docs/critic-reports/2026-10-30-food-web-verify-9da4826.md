# Verification critic on 9da4826: the density refusal reaches the editor note

A read-only verification pass over `9da4826` ("figures phase 4: food web verification round 5 (the density refusal
reaches the editor note)"). The scope is `e76d968..9da4826` only. I ran it on `git archive` copies of both commits, each
with its own target dir. I used my own SVG comparison, my own run-spacing parser, two mutants, and PNG renders at
1.3x, which I looked at with the image reader.

**Verdict: PASS (0 blocker, 0 major, 0 MED, 1 LOW, 2 minor, 1 nit).** All six claims hold. Every valid web renders
byte-identical to `e76d968` across 3,144 renders. `crowded()` agrees with the old inline test on every web I generated.
The canvas, `warnings()` and `notes_for` agree on every one of them, and the new note test catches a `warnings()` that
stops calling `crowded()`. One new weakness came in with the refactor: the refusal now counts runs in `gap_runs()`,
apart from the `gaps` vectors the layout uses, and no test pins a gap that holds both arrivals and departures. A
mutant that drops the arrival count passes all 21 food-web tests.

## Claim table

| # | Claim | Verdict | Evidence |
|---|---|---|---|
| 1 | The density refusal reaches the note; `crowded()` is the single test, called by `build()` and `warnings()` | **FIXED** | The note text is exactly as claimed. Harness `critic_agree`: 524 link texts × Small/Medium/Large = 1,572 cases. **1,392** draw, **150** are density-refused and **24** are parse errors (plus the empty web). For every one, the canvas's "could not be drawn", `warnings()` and `notes_for("food-web")` agree, with **0** mismatches. A parse error still gives "could not be read" on both and is never shadowed by the density branch, because `Err` cannot match the `Ok(w) if crowded` arm. The empty web is still silent. No web that draws gets a note. |
| 2 | `band()` = (clear, normal_hl+2, 4lab, usable); the dead `6lab` term is removed; drawn webs are byte-identical | **FIXED** | `6lab` is at most 8.4 (Large), and `normal_hl + 2` is at least 11, so the term was dead at every size. The band measures (clear/arr/dep/usable) 37.6/11/3.2/23.4 (Small), 47/11/4/32 (Medium) and 65.8/11/5.6/49.2 (Large). Equivalence: 524 webs × 3 sizes × levels on/off = **3,144** SVGs from each commit. This covers the default, all six examples, the omnivore, fanin, fanin-narrow, two-omni, gap-clash, span-3/span-4 skips, n = 1..24 long links (with and without extra mid links), 20 three-level-skip webs, 44 mixed departure/arrival webs and 400 random DAGs of up to 24 names. **232** files differ, and all 232 are refusal canvases at Small or Large. The only differing element is the red header's `y`. That is every Small/Large refusal canvas there is (232 of 232). There are 0 differences at Medium and 0 in any drawn web. |
| 3 | `gap_runs` counts one departure in the prey's gap and one arrival in gap `level[pred]-1` | **FIXED** | It is per-gap identical to the old `gaps` vector lengths on all 524 webs. For a 3-level skip (`A_i -> B -> C -> D`, `A_i -> D`), counts land in gaps 0 and 2 and nothing lands in gap 1. Index safety: `level_of` gives a predator a level of at least prey+1, so `level[pred]-1` cannot underflow. `parse()` refuses cycles, so `crowded()`'s `None` arm is unreachable from `warnings()` and `build()`. An empty web gives an empty vector. |
| 4 | Header baseline is `40*lab`; `min_w = MIN_W`; run-height test holds to stroke+1; comment reworded | **FIXED** | Header/first-line baselines: Small 32/51.2, Medium 40/64, Large 56/89.6. The gap is 24·lab, so it is proportional (it was 11.2 and 49.6). The canvas is 420 wide at all sizes. The run-height test asserts 8 runs at each size, and its bar is now 2.19/2.49/3.09. It is **not vacuous** (8 runs), but on gap-clash the minimum same-gap spacing is 5.85/8.0/12.3, so the bar is far from binding. See minor 2. |
| 5 | Note pinned both ways at 10/12/15 and n-1 | **FIXED** | Measured first refusals: long links **10/12/15**, three-level skips **10/12/15**. Mutant `Ok(w) if false && crowded(..)`: `the_density_limit_sits_where_the_notes_say` **fails** at line 751 ("no editor note for a refused web"). |
| 6 | 1924 passed / 0 failed / 5 ignored | **CONFIRMED** | A clean `git archive 9da4826` gave **1924 passed, 0 failed, 5 ignored** in 581 s. The known flake did not fire. `census_baseline` passes. |

Diff scope: `foodweb.rs`, `figures-progress.md` and the round-5 critic report only. No protected file is touched, the
census fixture is untouched, and no test is deleted. One assertion was **tightened** (from `stroke` to `stroke+1`). The
food-web tests stay at 21, and `food_web_pngs` gains two cases. `foodweb.rs` compiles with no warnings.

## New defects (ranked)

1. **LOW: the refusal and the layout now count runs separately, and the mixed-gap case is unpinned.** `build()`
   still fills `gaps` (the layout) at `foodweb.rs` ~417–420, while the refusal reads `gap_runs()` (~225–234). The two
   agree today (checked on 524 webs). Before this commit the refusal read the very vectors it laid out, so it could not
   drift from them. Now it can, and the suite does not notice. Mutant: delete `n[level[predator] - 1] += 1;` from
   `gap_runs`. **All 21 food-web tests pass.** The density tests use `A_i -> B, A_i -> C`, which puts n departures in
   gap 0 and n arrivals in gap 1, so dropping either count leaves the maximum at n. With that mutant, a gap holding 6
   arrivals and 6 departures at Medium would be counted as 6 and drawn with 2.46 px spacing, under the 2.5 bar.
   Fix: either derive `gaps` from the same pass or assert `gap_runs == gaps.len()` in `build()` under
   `debug_assert`. Also add a mixed-gap case, `P -> Q -> R -> S`, k × (`X_i -> Q`, `X_i -> R`), m × (`P -> Y_i`,
   `Y_i -> S`), to the density test: k+m = 11 must draw and k+m = 12 must be refused at Medium.
2. **minor: the run-spacing bar is only enforced by the refusal itself.** The tightened run-height test runs on
   gap-clash, whose closest runs are 2.7–4x the bar. The webs that sit right at the limit (n = 9/11/14 long links:
   closest spacing **2.34/2.66/3.28** against bars of 2.2/2.5/3.1) are checked only by `<rect>` counts, so no test
   measures that a drawn web keeps stroke+1. A test that parses the polylines of `web(first_refused-1)` would do it.
3. **minor: the refusal canvas's bottom margin runs the opposite way to the new top.** The canvas height is still
   `34 + lines·18·lab + 20`, fixed in its constants, so the space under the last baseline is **26.8 px at Small, 20 at
   Medium and 9.2 at Large**. Meanwhile the top above the header now grows to 56. In the Large PNG,
   "trophic level." sits close to the bottom edge under a large empty top. It was 9.2 before too, so this is not a
   regression, but the "proportionate spacing" fix covers only the top. Fix: `h = 40·lab + …`, or scale the 34/20 by
   `lab`.
4. **nit:** the canvas's last line ("Nothing is drawn until fewer links skip a trophic level.") does not mention the
   label size, which the note does. `band().2` (departure) is returned but never read by any caller (`_` in
   `build()`, `.3` in `crowded()`). Neither causes a compiler warning.

## What I verified

- SVG byte-equivalence of the two commits over 3,144 renders, classifying every difference.
- `crowded` against a pasted copy of `e76d968`'s inline condition (including the `max(…, 6lab)` term) and
  `gap_runs` against the old `gaps` lengths, on all 524 webs at all sizes. 0 disagreements.
- Canvas, `warnings` and `notes_for` agreement on 1,572 cases, including parse errors (no arrow, self-eat, cycle) and
  the empty web.
- The progress-doc numbers: 10/12/15, band 23.4/32/49.2 = 47lab − 11 − 4lab, and the header gaps 11/50 px before the fix.
- PNGs looked at: dense-refused at Small/Medium/Large, cycle and gap-clash. All are legible, and the text fits 420 px.
- Two mutants (the `warnings` bypass was killed, the dropped arrival count survived) and the full suite on a clean copy.

## What I did not check

- The app's note area in the running egui editor (I verified only `notes_for`).
- Webs over 24 names, which parse refuses before density can apply. The 20–24-name random webs were included.
- Other label factors than 0.8/1.0/1.4 (`label_factor` has no others).
- The bw66 print PNGs, beyond confirming they were written.
