# Phase 4 biology: seventh verification critic, build 68a5ed7

Source: the frozen `git archive` of 68a5ed7 at
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/0d1ac786-d5e8-40ac-b5ff-5513b2ee7026/scratchpad/r7-src`
(read-only). Binary: `.../scratchpad/r7-target/debug/vershion`. Renders are in `.../scratchpad/r7-pngs/`. Below, `P/`
stands for `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/0d1ac786-d5e8-40ac-b5ff-5513b2ee7026/scratchpad/r7-pngs/`.
Every path in this report is that absolute prefix plus the file name.

For exact text and geometry I built a scratch crate, `.../scratchpad/svgdump`, with a path dependency on the frozen copy
and no edits to it. It dumps the SVG, the editor notes (`notes_for`) and optionally the PNG. Its PNG for
`hardy-weinberg {"p":0.5,"n":999,"dec":1,"counts":true}` is byte-identical to the binary's (`cmp`), so the dumped
text is what the binary draws. About 26,000 parameter sets were dumped and checked by script, and 199 PNGs sit in
`P/`, including 92 per-case sweep PNGs and 9 contact sheets. Every PNG cited below was opened and looked at.

Scope: `hardy-weinberg`, `chi-square-fit`, `enzyme-kinetics`, `cladogram`. `food-web` was not reviewed.

Repro: `VERSHION_RENDER_FIG=<id> VERSHION_FIG_PARAMS='<json>' VERSHION_OUT=<png> <binary>`

## VERDICT: FAIL. 1 blocker, 5 majors, 10 minors

---

## BLOCKER 1: Hardy-Weinberg drops below the typed precision to agree with a tie-broken count, then prints a wrong p² and changes the given p

- Files: `src/figures/biology/hardyweinberg.rs:162-166`. The strict pass's `order()` (`:163`) is
  `(dec..=6).chain((1..dec).rev())`, so a COARSER precision that strictly "agrees" is returned before the round-7
  minimum-error fallback (`:173-186`) ever runs. The page then prints everything at that precision: the readout
  (`:389`), the bar labels (`:343`) and the givens and legend (`:321-323`, `:370`).
- The fault: `counts` (`:195-211`) breaks remainder ties on floating-point noise. For p = 0.6, N = 10 the exact
  counts are 3.6 / 4.8 / 1.6 and the result is 3 / 5 / 2, because 0.6² = 3.5999… loses to 1.6000…3. At the typed
  2 decimals, 0.36 × 10 = 3.6 → 4 ≠ 3. The loop keeps going, finds that 1 decimal makes the largest-remainder
  rounding 0.3 / 0.5 / 0.2 "agree", and returns it. The page that results contradicts its own formula and its own
  givens. Round 7's fallback would have accepted the typed 0.36 / 0.48 / 0.16, which is within 1 of every count, but
  the coarser branch pre-empts it.
- Repro A, the classic 1 : 2 : 1 at a class-sized N: `{"p":0.5,"n":10,"dec":2,"counts":true}` →
  `P/hw_p50n10d2.png`. I saw `AA p² = 0.3 (3 of 10)`, `Aa 2pq = 0.5`, `aa q² = 0.2`, and bar labels
  `AA 0.3 | Aa 0.5 | aa 0.2` beside givens `p = 0.5, q = 0.5`. p² = q² = 0.25, and the page prints them unequal.
  This is the exact asymmetric 1 : 2 : 1 that round 5 called a defect. The user asked for 2 decimals, which print
  0.25 / 0.50 / 0.25 exactly.
- Repro B, the given itself is rewritten: `{"p":0.21,"n":10,"dec":2,"counts":true}` → `P/hw_p21n10d2.png`. I saw
  the legend `A p = 0.2`, the givens `p = 0.2, q = 0.8, N = 10` (typed p = 0.21), and `AA p² = 0.1 (1 of 10)`. The
  true p² is 0.0441, and 0.2² = 0.04, so the printed p² is wrong both for the typed p and for the p the page prints.
  The AA bar is ~4 % of the width with a readout of 0.1.
- Repro C: `{"p":0.6,"n":10,"dec":2,"counts":true}` → `P/hw_p60n10d2.png` shows `AA p² = 0.3` for p = 0.6
  (p² = 0.36), with the identical readout to p = 0.5.
- Scale, from the real source over p = 0.01..0.99, N = 10..40 plus 50, 60, 75, 99, 101, 150, 200, 250 and 500, at
  dec 2 and 3 (7,920 renders with counts on): **287 print coarser than the typed precision**, and in **96** of them a
  printed fraction is not even the nearest rounding of its exact value. At dec = 2 over N = 10..40, 50, 100 and 250,
  **36 of 3,366 rewrite the given p** (for example p 0.25/N 10, p 0.26/N 12, p 0.28/N 18 and N 34). The round-7
  test grid does not reach small N.
- A correct rule never goes below the typed precision while the typed one stays within 1 of every count, or else
  breaks the count tie on the exact value rather than on f64 noise.

---

## MAJOR 1: Enzyme, the rate label's "nothing is clear" fallback still prints through the Km label and the beyond-axis note

- File: `src/figures/biology/enzyme.rs:353-360`. When no candidate clears every box, the label takes the first
  candidate that is merely `inside` (`:357`), with no test against `under_box`, `half_box` or `vmax_box`. The brief
  asked for exactly this path. It fires whenever an inhibitor brings the half-rate or Vmax′ label down near the point.
  Every candidate then misses by a fraction of a pixel (`py − 52` hit the half box by 0.3 px below), and the label
  falls back onto the Km label it was meant to avoid.
- Repro A, a slider minimum (drag the point to the origin) with a strong noncompetitive inhibitor:
  `{"mode":"noncompetitive","vmax":10,"km":1,"smax":20,"inh":10,"ki":1,"at":0}` → `P/enz_nc_at0_med.png`, crop
  `P/crop_enz_nc_at0_med.png`. `Km′ = 1` (moved to x = 74 by the round-7 candidates) is entirely overprinted by
  `v = 0 at [S] = 0` at the same x and 3 px higher, and only blue fragments of `Km′ = 1` show under the red text. The
  Km constant, the answer, is illegible. Small: `{…,"label_size":"Small"}` → `P/enz_nc_at0_small.png`.
- Repro B: `{"mode":"noncompetitive","vmax":1,"km":20,"smax":12,"at":12,"inh":4,"ki":1}` →
  `P/enz_nc_km20_s12.png`. `Km′ = 20, beyond this axis` is struck through by `v = 0.08 at [S] = 12`. SVG: the note at
  y 248.74 and the rate label at y 252.30 = `py − 9`, which is cands[0] via the fallback.
- Scale: 52 of a 3,240-case grid and 43 of 6,000 random cases (all three label sizes, Print and Colour) have the
  rate label's box crossing the Km label or note, measured with real Noto Sans widths.

## MAJOR 2: Cladogram, an Arabic tree's order line is all tofu (the round-6 defect, one script over)

- Files: `src/figures/biology/cladogram.rs:232-253` (`script_class` puts Arabic, Hebrew, Thai, Cyrillic and Greek
  in class 0 with "Left to right: ") and `:445-448`.
- Repro: `{"tree":"((إنسان, شمبانزي), غوريلا)"}` at Small, Medium and Large (`P/clado_ar_only_w.png`,
  `P/sheet_clado_1.png` #12-#14). The leaves render in Arabic, and the line reads `Left to right: □□□□□□ ,□□□□□□□ ,□□□□□`
  at all three sizes. This is the same failure the sixth critic's Major 2 reported, for the script the brief named
  ("a script the classifier lumps into Latin").
- The neighbouring case also fails. `((Русский, Ελληνικά), (עברית, العربية), (ภาษาไทย, Ñandú), …)` →
  `P/clado_mixname_Medium_w.png`, crop `P/crop_clado_mixname_line2.png`: in the order line the Arabic name prints
  with its letters unjoined (`ال ع ر ب ي ة`-like isolated forms), and the same name renders correctly as a leaf.

## MAJOR 3: Cladogram, a leaf or ancestor name that is itself Han+Hangul renders as broken jamo

- Files: `src/figures/biology/cladogram.rs:402` (ancestor label) and `:405` (leaf label). Each is still one
  `<text>`, and only the order line was split into runs.
- Repro, a leaf: `{"tree":"((日本한국, 中文Latin), (Русский, Ελληνικά), (עברית, العربية), (ภาษาไทย, Ñandú), 🦕🦖)"}` →
  `P/clado_mixname_Medium_w.png`, crop `P/crop_clado_mixname_leaves.png`, plus the Small and Large `_w` versions. The
  leaf reads `日本ㅎㅏㄴㄱㅜㄱ`, decomposed jamo instead of 한국. The order line, which is split, shows `日本한국`
  correctly, so the two disagree.
- Repro, an ancestor: `{"tree":"((Human, Chimp)日本한국, Gorilla)"}` → `P/clado_anc_mixed_w.png`. The ancestor
  label is the same jamo soup, and because it is wider than `text_w` estimates it runs over the vertical bar at the
  fork.

## MAJOR 4: Cladogram, run x positions come from the width estimate, so runs overlap or leave visible holes

- File: `src/figures/biology/cladogram.rs:447` (`cx += text_w(&run, …)`), with `glyph_em` at `:205-226`. The table
  is 0.56 em for every unlisted lowercase letter, Ñ, Cyrillic and Greek, while Noto Sans draws o/p/b/d/g/h/n/u at
  about 0.6 em and i/l/t/r narrower. Measured drift on real lines at 12 px: −7.1 px for `Hippopotamus amphibius,
  Bubo bubo, Gopherus polyphemus, ` and +13.9 px for `Mus musculus, Rattus norvegicus, Felis catus, Canis lupus, `.
  Hangul is estimated at 1 em but drawn narrower.
- Overlap: `{"tree":"(Hippopotamus amphibius, Bubo bubo, Gopherus polyphemus, 日本語, 한국어)"}` →
  `P/clado_overlap_Medium.png`, crop `P/crop_clado_overlap_Medium.png`. The line reads `polyphemus,日本語`: the space
  is gone and the Han run sits on the comma.
- Gap: `{"tree":"(Mus musculus, Rattus norvegicus, Felis catus, Canis lupus, 日本語, 한국어)"}` →
  `P/clado_gap_Medium.png`, which reads `Canis lupus,    日本語` with a hole of about 4 spaces. The same shows in
  `P/sheet_clado_1.png` #18/#19 (`Mus musculus,   日本語`, `Felis catus,   中文`).
- Before a comma: `한국어 , Felis` and `日本한국 , 中文Latin` both have a gap before the comma after a Hangul run
  (`P/crop_clado_mixname_line2.png`, `P/sheet_clado_0.png` #3-#5).
- Collision: `Ñandú, 🦕🦖` prints the ú and the comma into the dinosaur (`P/crop_clado_mixname_line2.png`).

## MAJOR 5: Cladogram, a labelled ancestor in the slanted shape is struck through by its own incoming branch

- File: `src/figures/biology/cladogram.rs:401-403`. The label is always placed at `(px − 5, py − 5)`, above-left of
  the fork. That is clear of a rectangular tree's horizontal, but in the slanted shape the diagonal from the parent
  arrives through exactly that spot whenever the fork is a lower child.
- Repro: `{"tree":"((Human, Chimp)Hominini, (Gorilla, Orangutan)Others)","shape":"slanted"}` →
  `P/clado_slant_anc_Medium_w.png`. The diagonal runs through `Others`. The same happens with `東亜` in
  `P/sheet_clado_1.png` #15 and #17 (`((Human, Chimp)Great apes, (日本, 한국)東亜)`, slanted, Small/Large).
  This is not a round-7 regression, but rounds 1-6 never reported it and both options are offered in the editor.

---

## MINOR 1: Enzyme, the Vmax label and the beyond-axis note still fall back to an unchecked spot that touches the point

- Files: `enzyme.rs:283-289` (Vmax: when all four rows fail, `vmax_y = vy − 8`, which is the row its own box test
  rejected for hitting `point_box`) and `:304-311` (note: falls back to `half − 7`).
- `{"km":0.2,"smax":40,"at":40}` → `P/enz_km02_s40_at40.png`, crop `P/crop_enz_km02_vmax.png`: the `0` of
  `Vmax = 10` touches the marked point. Large: `P/crop_enz_km02_vmax_L.png`.
- `{"mode":"noncompetitive","vmax":2,"km":20,"smax":12,"inh":10,"ki":0.2,"at":20}` →
  `P/enz_nc_note_point_med.png`, crop `P/crop_enz_nc_note_point_med.png`: the `s` of `axis` touches the point. My
  checker flagged 4 of 6,000 random cases like this.

## MINOR 2: Cladogram, an emoji ZWJ sequence is split into separate runs

- `script_class` (`cladogram.rs:248-252`) puts U+200D (ZWJ) and U+FE0F in class 0, so `👨‍👩‍👧` becomes three
  `<text>` runs. `{"tree":"(((👨‍👩‍👧, ❤️), (☀ sun, ✈ plane)), (🇯🇵, 👍🏽))"}` → `P/clado_emoji_Medium_w.png`: the order
  line shows three separate faces with wide gaps, while the leaf shows the joined sequence.

## MINOR 3: Cladogram, RTL names are reordered by bidi inside the class-0 run

- `{"tree":"((שלום, עולם), Dog)"}` → `P/sheet_clado_0.png` #9-#11: the line reads `Left to right: עולם ,שלום, Dog`,
  with the second leaf visually first. All-Hebrew `((אדם, שימפנזה), גורילה)` → `P/clado_he_only_w.png` prints the
  list reversed and moves the colon. In the Hebrew+Arabic case, `العربية` is shown left of `עברית`, the opposite of
  the tree.

## MINOR 4: Hardy-Weinberg, a segment labelled at Medium loses its label at Small (contradicts round 6's "verified")

- `hardyweinberg.rs:344-346`. The bar shrinks to 452 × 0.8 but the margin stays a fixed 8 px, so a borderline
  segment fails at Small. `{"p":0.35,"label_size":"Small"}` → `P/hw_p35_Small_w.png` has no `AA 0.12`, which
  `P/hw_p35_Medium_w.png` has. The same failure appears for 78 (p, N, style, sel, counts) groups, for example p 0.35
  and p 0.65. Small also leaves the 640 px canvas with a ~200 px empty right band.

## MINOR 5: Hardy-Weinberg, the editor note and the p list do not carry the page's one precision

- `hardyweinberg.rs:222-228` uses `val`, which trims, while the page uses `fixed`.
  `{"p":0.41,"dec":2,"gens":6,"sel":"against the recessive phenotype","counts":true,"n":10}` gives the note
  `rises from 0.41 to 0.851 … and to 0.87 in the next` beside the page's `0.410, 0.629, …, 0.851`.
  `{"p":0.41,"dec":1,"gens":6,"sel":…}` gives `rises from 0.4 to 0.9 … and to 0.9 in the next`, a "rise" to the same
  number. `{"p":0.333,"n":9999,"dec":1,"counts":true,"sel":…}` → `P/hw_p333n9999_Large_Print.png`: givens `p = 0.333`
  beside a p list starting `0.33300`.

## MINOR 6: Chi-square, per-value notation is lossy on the givens and inconsistent at the 1e7 boundary

- `chisquarefit.rs:343-346`, `:398` and `:429`. An observed count is a given, but at or above 1e7 it prints with 3
  significant figures. `{"obs":"12345678, 87654321","ratio":"1, 1","show_answers":false}` →
  `P/chi_obs_lossy_student_w.png` shows the student `1.23e7`, `8.77e7`, `1.00e8` and cannot recover the counts.
  `{"obs":"5000000, 5000001","ratio":"1, 1"}` (`P/chi_total_lossy_w.png`) has plain rows `5,000,000` and
  `5,000,001` with an Observed total of `1.00e7`, which is not their sum as printed. `{"obs":"9999999, 10000000",…}`
  prints `9,999,999` beside `1.00e7` in the same column.

## MINOR 7: Chi-square, the givens line is jammed against the χ² line

- `chisquarefit.rs:435` (`stats_y0 − 14·lab`, a 12 px line above a 15 px line). In the default render
  `P/chi_default_Medium_w.png` (crop `P/crop_chi_default_givens_Medium.png`) and at Large (`P/chi_mixcol_L3_w.png`)
  the `p` descender of "Expected" sits about 1 px from the `C`/`h` of "Chi-square". This is pre-existing.

## MINOR 8: Cladogram, a wrapped order line drops the separator at the break

- `cladogram.rs:290-302` and `:440`. `(Hippopotamus amphibius, Bubo bubo, Gopherus polyphemus, …)` at Medium
  (`P/clado_overlap_Medium.png`) ends line 1 at `Bubo bubo` and starts line 2 with `Gopherus polyphemus`, with no
  comma. With multi-word names the reader cannot tell one long name from two.

## MINOR 9: Cladogram, Hangul is drawn in a faint, thin face

- Every Hangul leaf and run is visibly lighter than the Han and Latin text in Colour and in Print
  (`P/clado_r6repro_print_w.png`, `P/sheet_clado_0.png` #3-#5, `P/clado_overlap_Medium.png`). In a bitonal copy it
  is the weakest text on the page.

## MINOR 10: Cladogram, the order line can clip at the right edge for o/p/b/d/h-heavy words

- Same estimate as Major 4. `{"tree":"(Mogonmono, Homodo hopudro, Gopuobpoop, Buhydbobopo)","label_size":"Small",
  "shape":"slanted"}` → `P/clado_clip_case55_Small_white.png`: `Buhydbobopo` is cut by the canvas edge. 8 of 300
  synthetic trees had ink in the last pixel column. No real species list (0 of 400) clipped, so this stays a minor.

---

## Verified correct (rendered and looked at, values recomputed independently)

Hardy-Weinberg
- Round-7 fix as specified: over 9,792 count-on renders (102 p × 16 N including 7, 10, 11, 13, 37, 99-101, 500,
  999-1001, 3333, 9999 and 10000 × dec 1-3 × selection on/off), every row has |fraction × N − count| ≤ 0.8 (never
  > 1), the fractions sum to 1 at the printed precision, there is one precision per readout, there is no bare 0, and
  there is no column of 1s in the p list or the bar labels. The sixth critic's repros now print `0.25 / 0.50 / 0.25`
  (250/499/250) for p 0.5/N 999 at dec 1, and `0.1369…`-style four-decimal sets for p 0.37/N 1001.
- Selection trajectory recomputed (p′ = p/(1 − q²)): p 0.333 → 0.33300, 0.59988, 0.71422, 0.77774, 0.81816, 0.84614.
  Generation-2 bars (0.35986 / 0.48005 / 0.16009) match p₂², 2p₂q₂, q₂². Later generations' bar labels use the page
  precision with no 0 and no 1 (2,496 selection renders, counts on/off). The editor note never collapses to 1.
- Student copy blanks genotypes, counts, Check and the p list. Clamps hold (p ∈ [0.01, 0.99], N ∈ [10, 10000],
  gens ≥ 2), and NaN, huge and negative inputs do not panic.

Enzyme
- 1,500 random parameter sets: the readout Vmax′/Km′, the factor 1 + [I]/Ki, the marked [S] and v, and the plot
  label `v = … at [S] = …` all match the recomputed values to the printed decimal (0 mismatches).
- The point is now in every label's box test. In 9,240 checked cases no Km or half-rate label touches the point
  except through the fallbacks above. The displaced Km label (`px ± 8` or the upper rows) sits next to its dotted
  marker (`P/sweep_enz/00032.png`, `P/enz_nc_at0_med.png`).
- Student copy blanks the factor (`1 + [I]/Ki = _____`), both pairs and v. Haloes are present on Km, Vmax, half,
  rate and note.

Cladogram
- The round-6 repro `(日本語のテスト, 한국어이름)` is fixed: the order line renders in Han and Hangul in Colour and
  Print (`P/clado_r6repro_print_w.png`). Separate Han and Hangul names, Cyrillic, Greek, Thai and accented Latin all
  render in the order line.
- The too-long message, the unbalanced-tree message, the empty tree, `(A)` and `Solo` are unchanged and correct.
  The order follows typed order and the topology counts are right.

Chi-square
- 3,000 random tables around 1e7 in every column: every printed O, E and term matches its recomputed value to its
  printed precision. Nothing ≥ 1e7 prints plain (except Minor 6's boundary cases) and nothing < 1e7 prints scientific,
  apart from never-zero tiny terms. The Total equals the sum of the printed terms, the χ² line equals the Total cell,
  and the p-value matches an independent incomplete-gamma computation. Mixed plain and scientific in one column
  (`P/chi_mixcol_L3_w.png`: 1.50e7 / 1,666,666.667 / 3,333,333.333, total 2.00e7) is right-aligned and sums.
- The sixth critic's minor is fixed: `1 : 1.00e20` with counts `1, 1` prints `1`, `1`, `2` (`P/chi_huge_r6_w.png`).
- Regressions none: `0, 0` refuses (`χ² = —`, `p = —`, no decision), a single category says no degrees of
  freedom, 531/469 is borderline at 0.050, 532/468 rejects, 1e160 gives ∞ "too large", and the student copy blanks
  E, terms, the Expected Total and the whole stats block.

Global
- All four defaults rendered twice are byte-identical.

## Not checked

- The live egui editor, dragging handles, and how the notes panel displays.
- The wasm and web text renderer. All script findings are on the resvg PNG path, which is the export path. Another
  rasteriser may shape Arabic and Hangul differently.
- The PDF and DOCX export, and share-link round trips.
- `food-web`.

## Tests (frozen copy, own target dir, `systemd-run` 8 GB cap)

- `cargo test -j2 --lib figures`: **1404 passed, 0 failed, 4 ignored**, 519 filtered out (656.67 s).
- `cargo test -j2 --lib figuresui`: **82 passed, 0 failed, 0 ignored**, 1845 filtered out (65.82 s).
- The commit message claims figures 1405/0/4. That count does not come from the committed tree. The builder's log
  (`.../scratchpad/r7-figures.log`) contains `figures::biology::foodweb::tests::a_web_too_dense_to_draw_is_refused_not_banded`,
  which is not in 68a5ed7 (uncommitted food-web work), and lacks `figures::implicit::tests::tails_reach_the_window_edge_in_wide_flat_windows`,
  which is. The claimed count was measured on the worktree, not on the commit. None of the failures above is covered
  by a test.
