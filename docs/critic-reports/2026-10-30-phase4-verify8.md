# Phase 4 biology: eighth verification critic, build dfcbd79

Source: the frozen `git archive` of dfcbd79 at
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/0d1ac786-d5e8-40ac-b5ff-5513b2ee7026/scratchpad/r8-src`
(biology files diffed against `git show dfcbd79`: identical). Binary: `.../scratchpad/r8-target/debug/vershion`.
Below, `P/` stands for `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/0d1ac786-d5e8-40ac-b5ff-5513b2ee7026/scratchpad/r8c-pngs/`.

For exact text I built a scratch crate (`.../scratchpad/r8c-svgdump`, path dependency on the frozen copy, no edits).
Its PNG for `hardy-weinberg {"p":0.5,"n":999,"dec":1,"counts":true}` is byte-identical to the binary's
(`P/chk_svgdump.png` vs `P/chk_bin.png`). Sweep scripts are in `.../scratchpad/r8c-tools/`. Label geometry was
checked with real Noto Sans advance widths (PIL), not the code's 0.55 em estimate. About 73,000 parameter sets were
dumped and checked by script: HW 37,584, enzyme 17,720, chi-square 3,400, cladogram 2,733 random trees plus probes.
There are 217 PNGs in `P/`, including 8 contact sheets. Every PNG cited below was opened and looked at.

Scope: `hardy-weinberg`, `chi-square-fit`, `enzyme-kinetics`, `cladogram`. `food-web` was not reviewed.

Repro: `VERSHION_RENDER_FIG=<id> VERSHION_FIG_PARAMS='<json>' VERSHION_OUT=<png> <binary>`

## VERDICT: FAIL. 1 blocker, 2 majors, 8 minors

---

## BLOCKER 1: Hardy-Weinberg rewrites a given p that has 5 or 6 decimals, beside a p² and a p list computed from the typed p

- File: `src/figures/biology/hardyweinberg.rs:330`, `:332` (legend) and `:382-383` (givens). Both call `val(p0, dp)`,
  which goes through `num`, then `math::fmt`, and `math::fmt` clamps to 4 decimals (`src/figures/math.rs:17-18`). The
  round-8 floor lets `own_decimals` raise `dp` to 5 or 6, so the rows print at 6 decimals but the givens are cut to 4.
- Repro: `{"p":0.123456,"n":1000,"dec":2,"counts":true,"sel":"against the recessive phenotype","gens":4}` →
  `P/hw_p123456_n1000_d2_counts.png`. I saw `A p = 0.1235`, `a q = 0.8765` and givens `p = 0.1235, q = 0.8765, N = 1000`.
  Beside them: `AA p² = 0.015241` (that is 0.123456²; 0.1235² = 0.015252), and a p list that starts
  `0.123456, 0.532895, …`. The page gives p two different values.
- Same with 5 decimals, no selection: `{"p":0.12345,"dec":1}` → `P/hw_p12345_d1.png` (givens 0.1235, AA 0.01524).
- Scale: every 5- or 6-decimal p in my sweep (288 of 288 renders at p = 0.123456).
- Reachability: the slider steps by 0.01, so this needs a share link or JSON params. That is the same input channel as
  the 0.333 and 0.1234 cases the brief and the round-8 test use. The commit claims "the given p and q print exactly
  as typed". The test `the_page_is_never_coarser_than_typed_and_never_rewrites_the_given_p` only goes up to 4 decimals.

## MAJOR 1: Enzyme, the new Km candidates and line obstacles move "Km′ = …" up to 560 px from its marker, next to the point

- File: `src/figures/biology/enzyme.rs:303-318`. Round 8 added the asymptote and half-rate dashed lines to
  `km_obstacles` (`:317-318`). It tests them against a full-em box (`box_around` top = baseline − size, `:211-213`),
  so a dashed line that only passes through the empty space above the cap height rejects the base position. The next
  candidates that clear are `(px ± 8, km_y)`, which sit beside the marked point, wherever that point is.
- Repro A: `{"mode":"noncompetitive","vmax":10,"km":3.3,"smax":40,"at":40,"inh":1,"ki":0.2}` →
  `P/enz_nc_km33_s40_at40_M.png`. The dotted Km′ marker is at [S] = 3.3, at the far left. `Km′ = 3.3` is printed at
  the far right under the point at [S] = 40, next to the `40` tick, so it reads as Km′ ≈ 38-40 or as a label on the
  point.
- Repro B, Large: `{"mode":"noncompetitive","inh":4,"ki":1,"vmax":10,"km":1,"smax":40,"at":40,"label_size":"Large"}`
  → `P/enz_nc_km1_s40_at40_L.png`. The marker is at [S] = 1, and `Km′ = 1` is at [S] ≈ 38. The same params at Medium
  keep the label at its marker (`P/enz_nc_km1_s40_at40_M_print.png`), so the size alone flips it.
- Also `{"mode":"noncompetitive","inh":4,"ki":1,"vmax":1,"km":0.2,"smax":2,"at":1,"label_size":"Large"}` (266 px
  away).
- Scale (17,720 grid and random cases, real widths): 563 Km labels are more than 14 px from their marker, and about 496
  of those are more than 100 px away. All are noncompetitive, at every size. The round-8 test checks only overlap, and
  "the least overlap" here costs the label its meaning. In my sweep no label touched another label, the point, a tick,
  the canvas edge or the plot frame (0 cases).

## MAJOR 2: Cladogram, a slanted labelled ancestor is still struck through, now by a sibling's branch

- File: `src/figures/biology/cladogram.rs:535-556`. The fix keeps the label off its own incoming diagonal
  (`from_above`). But for a middle child, whose incoming branch is horizontal, or an upper child, the parent's other
  diagonals pass through the above-left spot. The candidate cost is only compared, never required to be 0, so the
  least-bad spot is still struck. The halo cuts the line, but the stroke still visibly crosses the word.
- Repro A: `{"tree":"(Gorilla, (Human, Chimp)Hominini, Orangutan)","shape":"slanted"}` →
  `P/clado_slant_mid3_M.png`, crop `P/crop_clado_slant_mid3_M.png`. The root-to-Gorilla diagonal runs through the `Ho`
  of `Hominini`, and the label sits on its own horizontal branch.
- Repro B: `{"tree":"((Homo, X)Ruminantia, (Ovis, Homo)Primates and others, Homo)A","shape":"slanted","label_size":"Large"}`
  → `P/clado_slant_two_adj.png`. The diagonal to `Ruminantia` strikes the `P` of `Primates and others`.
- Also `{"tree":"(Pongo, (Pongo, X)Ruminantia, Pan)Clade 7","shape":"slanted","label_size":"Large"}` →
  `P/clado_slant_mid3_L.png` (34 px of branch inside the label box).
- Scale: in 2,733 random trees, 432 slanted renders have more than 0.5 px of branch inside an ancestor label's ink box.
  Rectangular renders: 0. The round-7 repro `((Human, Chimp)Hominini, (Gorilla, Orangutan)Others)` is fixed
  (`P/clado_slant_r7repro.png`). Deep chains of either handedness were clean (`P/clado_slant_deep.png`,
  `P/clado_slant_deep_rev.png`).

---

## MINOR 1: Cladogram, `latin_class` admits whole symbol blocks Noto Sans lacks, and the entire order line changes face

- File: `src/figures/biology/cladogram.rs:262-266` (`u < 0x0590 || 0x1e00..=0x1fff || 0x2000..=0x2bff`).
- No tofu: the rasteriser picks a fallback font for the whole `<text>`, so "Left to right: Homo sapiens, …, Pan"
  prints in a bold monospace face, or a serif face for dingbats. It is the only text on the page in that face. Affected:
  Armenian (`Հայերեն`), arrows (`→⇒↔`), misc symbols (`☀ ♥ ♀♂`), dingbats (`✈ ✓`, serif, and the leaf too), Braille,
  box drawing, geometric shapes, misc technical (`⌘⌚`), `⭐ ⬆` and enclosed alphanumerics (`①②`).
  Clean: Greek, Cyrillic, IPA, Vietnamese, Latin Extended-B, combining marks, math (`∑∫√∞≠≤`), letterlike, subscripts,
  currency, superscripts.
- `{"tree":"((Homo sapiens, ⠃⠗⠇), Pan)"}` and the others → `P/sheet_clado_latinclass_a.png`,
  `P/sheet_clado_latinclass_b.png` (per-case PNGs `P/clado_lc_*.png`).

## MINOR 2: Hardy-Weinberg, remainder ties are broken by array order or float noise, so mirror populations print differently

- Files: `hardyweinberg.rs:99-119` (`printed_parts`) and `:199-215` (`counts`). Both sort on f64 remainders.
- `{"p":0.07,"dec":1}` → `P/hw_p07_d1.png` prints at 2 decimals, `0.01 / 0.13 / 0.86`. Its mirror `{"p":0.93,"dec":1}`
  → `P/hw_p93_d1.png` prints at 3 decimals, `0.865 / 0.130 / 0.005`. The exact remainders tie at .49. The noise
  (0.49000000000000005 vs 0.4899999999999949 for p = 0.07, the reverse for 0.93) decides which genotype gets the unit,
  and so whether a zero part forces the precision up. 29 of 297 mirror pairs (dec 1-3, counts off) are not mirror
  images.
- Same mechanism, by index order: `{"p":0.5,"n":10,"dec":1,"counts":true,"sel":"against the recessive phenotype","gens":6}`
  → `P/hw_p50_n10_d1_sel6_counts.png` prints `AA p² = 0.3`, `aa q² = 0.2` beside `p = 0.5, q = 0.5` and two
  equal-width bars. It is the round-7 blocker's output at a typed 1 decimal: allowed by the new floor, but still two
  equal quantities printed unequal. Raising the precision on an exact half tie would print 0.25. Judgement call: by
  the "wrong-looking render" rule this could be filed as a blocker.

## MINOR 3: Hardy-Weinberg, the selection note is now finer than the page it describes

- File: `hardyweinberg.rs:228-230`. The note's precision is raised until it "rises", but the page's p list (`:420`)
  and bar labels stay at `dp`.
- `{"p":0.5,"n":10,"dec":1,"counts":true,"sel":"against the recessive phenotype","gens":6}` →
  `P/hw_p50_n10_d1_sel6_counts.png`. The page reads `p in each generation 0.5, 0.7, 0.8, 0.8, 0.8, 0.9`, with
  generations 4-6 all labelled `AA 0.7 Aa 0.3`. The note says `from 0.50 to 0.86 … and to 0.88`. That is the round-7
  minor 5 (note and page carry different precisions) moved to the neighbouring case, and the flat page list is the
  "rise that prints as no rise" the fix was written for. 480 of 22,032 sized renders disagree.

## MINOR 4: Chi-square, a fractional observed count makes the Total and the givens contradict the cells

- File: `chisquarefit.rs:305`, `:321`, `:430`. The round-8 "total is the sum of the printed cells" only holds when
  every cell is whole (`obs_full`).
- `{"obs":"5.5, 10.25","ratio":"1, 1"}` → `P/chi_frac_total.png`: Total `15.75`, givens `out of 16 observations`
  (`val(total, 0)`). The input is accepted with no warning.
- `{"obs":"12345678, 0.5","ratio":"1, 1"}` → `P/chi_frac_big.png` (student `P/chi_frac_big_student.png`): cells
  `12,345,678` and `0.5`, Total `1.23e7`, givens `out of 1.23e7`. This is the exact round-7 minor 6 shape, one
  neighbour over. 73 of 3,000 random tables hit this.

## MINOR 5: Chi-square, the Expected total goes scientific while its cells are plain

- File: `chisquarefit.rs:436`. `{"obs":"5000000, 5000001","ratio":"1, 1"}` → `P/chi_5e6_exp_total.png`: Expected
  cells `5,000,000.50` twice, Expected total `1.00e7`, beside an Observed total of `10,000,001`. This is answer key
  only, and lossy rather than wrong.

## MINOR 6: Enzyme, the half-rate label is placed on the Vmax′ dashed line and the curve

- File: `enzyme.rs:374-383`. `half_cands` obstacles exclude the dashed lines and the curve, and only the halo protects
  the label. `{"mode":"noncompetitive","inh":4,"ki":1,"vmax":10,"km":1,"smax":40,"at":40,"label_size":"Large"}` →
  crop `P/crop_enz_nc_km1_L_half.png`: the dashes and the 2.4 px curve run through `Vmax′/2 = 1` at x-height. It is
  readable but struck through.

## MINOR 7: Enzyme, at Large the x-axis title and the givens line nearly merge

- `enzyme.rs:238` (title at `bottom + 40·lab`) and `:438` (givens at `H + 18·lab`). The gap is about 20 px at Medium
  and about 5 px at Large. See crop `P/crop_enz_nc_km1_L_title_givens.png`. Cosmetic.

## MINOR 8: Cladogram, script runs inside one name are still placed by estimated width

- `cladogram.rs:404-416`. `Mixed 日本한국` as an ancestor shows a double-width gap before the Han run
  (`P/clado_mix_leaf_anc.png`). Cosmetic. Leaf and row runs render correctly with no jamo
  (`P/clado_mix_leaf_anc_rect_L.png`).

---

## Verified correct (rendered and looked at, values recomputed independently)

Hardy-Weinberg
- 37,584 renders (99 two-decimal p plus 0.333, 0.1234, 0.6667, 0.505, 0.995 and others; N 1-10000 including 1, 2, 3,
  37, 99, 999, 9999, 10000; dec 1-3; counts on/off; selection off and gens 2/4/6; all sizes; Print):
  - Never coarser than the typed dec or p's own decimals (up to 4).
  - The fractions sum to exactly 1, each within one unit of the exact value. Every count set is a valid exact
    largest-remainder allocation summing to N, and every row is within 1 of fraction × N.
  - One precision per readout, bar label and p list. No bare 0 and no 1 anywhere.
  - Precision never goes above 6, and the canvas never overflows.
  - Bar labels always fit their segment with real widths, including Small (`P/hw_p35_Small.png` keeps `AA 0.12`).
- The round-7 repros are fixed: p 0.5/N 10/dec 2 prints `0.25 / 0.50 / 0.25` (`P/hw_p50_n10_d2_counts.png`).
  p 0.21/N 10 prints `p = 0.21`, `AA 0.04 (1 of 10)` (`P/hw_p21_n10_d2_counts.png`). p 0.6/N 10 prints `0.36 / 0.48 / 0.16`
  (`P/hw_p60_n10_d2_counts.png`).
- The selection trajectory was recomputed exactly as p′ = 1/(2 − p) for every printed list entry. Student copy blanks
  everything. NaN, negative and huge inputs clamp (`P/hw_nan.png`).

Enzyme
- 17,720 cases, real widths: 0 label-label, 0 label-point, 0 label-tick, 0 label-canvas and 0 label-outside-plot.
  The round-7 fallback repros are clean.
- Recomputed: Km′ = Km(1 + [I]/Ki) (1,020 for competitive [I] 10 / Ki 0.2), Vmax′ = Vmax/factor, and v at the marked
  [S] (7.5, 0.909, 1.951, 3.774, 99.502). Givens, readout and plot label agree.
- Student copy blanks the factor and both pairs (`P/enz_student_nc.png`). Haloes are present.

Chi-square
- 3,000 random tables plus 360 magnitude cases (1 to 1.2e16 in each column, every size, key and student): whole counts
  below 1e15 print in full and grouped, the Total is the sum of the printed cells, and the givens name the same total.
  No overlap or canvas overflow with real widths. A 19-digit cell with a long name truncates the name cleanly
  (`P/chi_6x1e15_S_names.png`).
- The p-value against an independent incomplete gamma, and the decision at α boundaries, all agree: 531/469 is
  borderline 0.050 and 532/468 rejects at 0.043.
- The givens gap is about 6 lab px, including when the givens wrap (`P/chi_wrap_givens_L.png`).
- Zero observations refuse, a single category has no df, mismatched lists use the first pairs with a note, and
  negative, NaN and inf counts are ignored with a note. 1e160 prints ∞ "too large". The student copy blanks E, the
  Expected total and the stats.

Cladogram
- Numbered rows: alignment holds at 10+ rows (`P/clado_hangul12_L.png`). The 40-row cap prints 39 rows and
  "… and 6 more" (`P/crop_clado_hangul45_tail.png`). A 500-character all-Hangul tree gives the too-long message
  (`P/clado_hangul500.png`). A 125-character Arabic name widens the canvas and is not clipped
  (`P/clado_arabic_long.png`). Han+Hangul names render as runs with no jamo.
- Latin trees against the round-7 binary: defaults, the slanted example and the labelled ancestor are byte-identical.
  Long lists differ only by the restored trailing comma or a width change of 2-36 px. No absurd blank canvas.
  Rectangular ancestors were never struck (0 of about 900).

Global
- All four defaults rendered twice are byte-identical: `P/hw_default_a/b`, `P/enz_default_a/b`, `P/chi_default_a/b`
  and `P/clado_base_r7_1` vs `P/clado_base_r8_1`.

## Not checked

- The live egui editor, dragging handles, and whether a typed slider value can carry more than 2 decimals.
- The wasm and web renderer. All font findings are on the resvg PNG path with this machine's system fonts loaded
  (`load_system_fonts`), so the fallback faces in Minor 1 may differ on another box.
- PDF and DOCX export, share-link round trips, and `food-web`.

## Tests (frozen copy, `systemd-run` 8 GB cap, target `.../scratchpad/r8-target`)

- `cargo test -j2 --lib figures`: **1418 passed, 0 failed, 4 ignored**, 519 filtered out (509.6 s).
- `cargo test -j2 --lib figuresui`: **82 passed, 0 failed, 0 ignored**, 1859 filtered out (59.8 s).
- Measured on the committed tree, the figures count equals the commit message's 1418, despite the message's caveat.
  None of the findings above is covered by a test.
