# Phase 4 biology: ninth verification critic, build df9c7c2

Source: the frozen `git archive` of df9c7c2 at
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/0d1ac786-d5e8-40ac-b5ff-5513b2ee7026/scratchpad/r9-src`.
Binary: `.../scratchpad/r9-target/debug/vershion`. Below, `P/` stands for
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/0d1ac786-d5e8-40ac-b5ff-5513b2ee7026/scratchpad/r9c-pngs/`.

Method:
- A scratch dump crate (`.../scratchpad/r9c-svgdump`, a path dependency on r9-src, no edits) gives exact SVG text.
- For the baseline I built a second dump crate (`.../scratchpad/r9c-basedump`) against a fresh `git archive dfcbd79`
  (`.../scratchpad/r9c-base-src`). `.../scratchpad/r8-src` and `r8-target` had been edited and rebuilt after the eighth
  critic ran, so they are not dfcbd79 any more and I did not use them as a baseline.
- Sweep scripts are in `.../scratchpad/r9c-tools/`. Label geometry uses real Noto Sans advance widths (PIL).
- Volume: HW 203,904 renders (3 sweeps) plus 12,474 mirror and 2,520 size cases. Enzyme 2 × 17,720, the same grid on
  dfcbd79, and a strike comparison. Chi-square 3,324 per build. Cladogram 2,733 random trees per build, 2,148
  mixed-script random trees, 402 Latin rectangular trees diffed against dfcbd79, and 24 `latin_class` probes.
- 199 PNGs are in `P/`, including contact sheets, crops and `base_*` renders from dfcbd79. I opened every PNG I cite.

Scope: `hardy-weinberg`, `chi-square-fit`, `enzyme-kinetics`, `cladogram`. I did not review `food-web` or `energy-pyramid`.

Repro: `VERSHION_RENDER_FIG=<id> VERSHION_FIG_PARAMS='<json>' VERSHION_OUT=<png> <binary>`

## VERDICT: FAIL. 0 blockers, 3 majors, 6 minors

---

## MAJOR 1 (regression): Enzyme, the half-rate label is now placed on or below the x-axis line and piles into the Km and rate labels

- File: `src/figures/biology/enzyme.rs:466-471` (the round-9 `near` candidates at `half + 14·lab`) and `:485` (the
  `ok` test `b.3 > vy + 1.0` only requires the label to be BELOW the asymptote, and nothing keeps it above the x-axis).
  With a strong noncompetitive inhibitor the half-rate line is within a few px of the axis. The only candidates "under
  the asymptote" then sit across or below the axis line, and `pick_near` takes one because thin lines are a tie-break
  and the axis is not an obstacle.
- Repro (Medium):
  `{"mode":"noncompetitive","inh":10,"ki":1,"vmax":7.5,"km":1.0,"smax":2,"at":0,"label_size":"Medium"}`
  → `P/enz_half_axis_M.png`, zoom `P/zoom_enz_half_axis_M.png`.
  - `Vmax′/2 = 0.34` sits on the x-axis line and is struck by it and by the dotted half-rate line.
  - `Km′ = 1` is struck by the 2.4 px curve and the dotted line.
  - `v = 0 at [S] = 0` is struck by the dotted half-rate line.
  - All three are crammed into one 15 px band. On dfcbd79 the same params give three clear labels
    (`P/base_enz_half_axis_M.png`, zoom `P/zoom_base_enz_half_axis_M.png`).
- Large: `{"mode":"noncompetitive","inh":10,"ki":1,"vmax":1,"km":0.2,"smax":7,"at":0,"label_size":"Large"}`
  → `P/enz_half_below_axis_L.png`, crop `P/crop_enz_half_below_axis_L.png`. `Vmax′/2 = 0.05` hangs below the axis,
  between the tick row and the axis, and the axis line runs through its cap height.
- Small, Print: `{"mode":"noncompetitive","vmax":5,"km":0.3,"smax":8,"at":4.0,"inh":10,"ki":0.5,"dec":3,"label_size":"Small","style":"Print"}`
  → `P/enz_half_axis_S_print.png`. Also `P/enz_ext_19.png` (vmax 1, km 20, smax 2, inh 10, ki 0.2). In that one the
  "Km′ = 20, beyond this axis" note, `Vmax′/2 = 0.01` and the point label share the bottom band.
- Scale: the same 17,720-case grid plus random set the eighth critic ran shows 767 labels outside the plot frame, all
  half-rate labels, at all three sizes (Small 256, Medium 240, Large 271), all noncompetitive. dfcbd79 has 0 on the
  identical cases. A line-crossing count on the same cases puts `Vmax′/2` on the x-axis line 172 times in df9c7c2 and
  0 times in dfcbd79.
- This is a different case from the accepted flat-curve residual (vmax 10, km 1, smax 20, Large): it occurs at Medium
  and Small, puts the label on the axis, and is worse than the previous build.
- Fair note: across the grid the round-9 thin-line model did cut half-rate strikes by the curve sharply. Curve strikes
  fell from 3,138 to 396 and dashed-curve strikes from 3,254 to 70. The defect is the neighbour case, where the line
  being labelled sits on the axis.

## MAJOR 2: Cladogram, a Latin run before a CJK/Hangul run is end-anchored by an estimated width and overprints the branch and the list number

- File: `src/figures/biology/cladogram.rs:439-452` (`label_el`). The round-9 change anchors a leading Latin run at the
  next run's start with `text-anchor="end"` (`:447`). It is placed at `x0 + estimated width`, so the full estimation
  error now lands on its LEFT end. `glyph_em` (`:205-225`) gives lowercase 0.56 em, but Noto Sans b/d/n/o/p/u are about
  0.61, so ordinary Latin is underestimated. The name then runs left past its anchor, into the branch tip in the tree
  and into the number column in the list.
- Repro: `{"tree":"((Gorilla, Bubo bubo bubo bubo bubo bubo bubo bubo 日本한국), Pan)"}` → `P/clado_long_mixed_M.png`.
  The leaf's `B` is drawn on the end of its branch line. List row 2 reads `2Bubo…`: the `B` overprints the `2.`.
  - On dfcbd79 the same tree (`P/base_clado_long_mixed_M.png`) overprinted at the inner junction (`bubo日本`).
    The fix moved the overprint from the middle of the name to its start; it did not remove it.
  - Shorter name at Large: `{"tree":"((Gorilla, Bubo bubo bubo bubo 日本한국), Pan)","label_size":"Large"}`
    → `P/clado_bubo_mixed_L.png`. The leaf touches its branch tip and the list reads `2.Bubo` with no gap.
  - Wide glyphs: `{"tree":"((Gorilla, @@@@@@@@——————%%%% 日本한국), Pan)"}` → `P/clado_wideglyph_mixed.png`. The leaf
    runs back across its branch to the internal node. List row 2 overprints `2.` and is cut off at the left canvas
    edge.
- The other side of the same estimate (overestimate) leaves a detached leading gap of 10-35 px. See
  `{"tree":"((Gorilla, iiiiiiiiiiii lllll 日本한국), Pan)"}` → `P/clado_narrow_mixed_M.png`, `P/clado_wide_mixed_M.png`
  and `P/clado_greek_anc_struck_L.png` ("Mixed 日本한국", leaf and list row 6). That half is cosmetic. It is the
  eighth critic's minor 8 moved from the middle of the name to its front.
- Applies only to names that need runs (two non-Latin classes, such as Han + Hangul), which is exactly the case this
  round targeted.

## MAJOR 3 (pre-existing, not in any earlier report): Enzyme, at Large a wrapped givens line overprints the first readout row

- File: `src/figures/biology/enzyme.rs:539-541` (givens wrapped by the 0.55 em estimate at `15·lab` spacing) against
  `:287` (`rows_y0 = H + 46·lab + shift`). A second givens line puts its baseline at `H + 33·lab + shift`. At
  lab = 1.4 its descenders reach about 1.5 px into the cap height of the 21 px row below.
- Repro: `{"mode":"noncompetitive","inh":0.5,"ki":10,"vmax":1,"km":0.2,"smax":2,"at":0,"label_size":"Large"}`
  → `P/enz_givenswrap_L.png`, crop `P/crop_enz_givenswrap_L.png`. The `)` of the wrapped `1.05)` sits on the `h` of
  `With the inhibitor`.
- The wrap is also needless: the unwrapped line measures about 570 px with real widths on a 732 px canvas, but the
  0.55 em estimate breaks it at 73 characters.
- Scale: 452 of 5,868 Large cases (every inhibitor case whose givens exceed 73 estimated characters), including the
  student copy (`_____)` over `With the inhibitor: Vmax = _____`). 0 at Small and Medium. The same 452 occur on
  dfcbd79. The round-9 `givens_y0` work fixed the title/givens gap but not the givens/readout gap under it.
- The overlap is small (1.5 px of ink), but it is glyph on glyph. If the loop treats a 1-2 px descender touch as
  cosmetic, downgrade this to a minor; I have not counted it as one.

---

## MINOR 1: Cladogram, `latin_class` admits Coptic and the newest Latin Extended-D letters, which Noto Sans lacks

- File: `cladogram.rs:273` (`0x0370..=0x03ff` includes Coptic U+03E2-03EF) and `:282` (`0xa720..=0xa7ff` includes
  U+A7CB-A7CD and U+A7DA-A7DC). Coverage was checked glyph by glyph against `NotoSans-Regular.ttf`. These are the only
  allow-listed characters it lacks.
- `{"tree":"((Homo sapiens, Ꟛꟛ Ɤ), (Pan, Gorilla)Ꟛꟛ Ɤ anc)"}` → `P/clado_lc_latd.png`, crop
  `P/crop_clado_lc_latd.png`. The whole order line falls to a bold monospace face and runs off the right edge
  (`Gorill|a`).
- `{"tree":"((Homo sapiens, Ϣϣ Ϭ), (Pan, Gorilla)Ϣϣ Ϭ anc)"}` → `P/clado_lc_coptic.png`. The order line, leaf and
  ancestor print in a serif face.
- The extreme input makes this a minor.
- Every other boundary class checked behaves as the commit says (`P/sheet_clado_lc_a.png`, `P/sheet_clado_lc_b.png`):
  - Greek, Cyrillic, Vietnamese, IPA, modifiers, super/subscripts, letterlike symbols, ligatures, combining marks and
    currency (€ ₹ ₿) stay on one Latin line.
  - Math, arrows, dingbats, box drawing, Braille, number forms, Armenian, geometric shapes, ZWSP, RLM and private use
    get the numbered list.

## MINOR 2: Enzyme, in the noncompetitive default at Large the half-rate label leaves its line

- `{"mode":"noncompetitive","label_size":"Large"}` → `P/cmp_enz_def_nc_L.png` (left dfcbd79, right df9c7c2).
  `Vmax′/2 = 1.25` now sits on the Km row, right of the `Km′ = 2` label, about 20 px below the dotted line it names.
  It reads as a second label of the Km marker. dfcbd79 had it on its line, though struck by the curve.
- The plain default is better than dfcbd79 at every size and in Print: the half-rate label is off the curve
  (`P/cmp_enz_def_Small_Colour.png`, `P/cmp_enz_def_Medium_Colour.png`, `P/cmp_enz_def_Large_Print.png`).

## MINOR 3: Chi-square, a column below 1e15 still goes scientific when a fractional count raises its scale

- File: `chisquarefit.rs:320` (`(r.observed * scale).abs() < 9.0e15`).
- `{"obs":"1000000000000, 0.123456","ratio":"1, 1"}` → `P/chi_1e12_frac6.png`. The cells print `1.00e12` and `0.12`,
  so the typed 0.123456 is not printed as typed and 1e12 is not plain. The Total is `1.00e12`.
- Also `{"obs":"999999999999999, 1000001.5286","ratio":"1, 1"}` (cell `1,000,001.53`).
- Nothing contradicts at 3 significant figures, but this is the neighbour of the claimed "plain until 1e15" rule.

## MINOR 4: Chi-square, a count with 7+ decimals is silently rounded, to 0 if small

- File: `chisquarefit.rs:197-204` (`own_decimals` caps at 6). There is no editor note, unlike Hardy-Weinberg's p.
- `{"obs":"0.0000001, 5","ratio":"1, 1"}` → `P/chi_7dec.png`. Observed prints `0` and the Total `5`.
- `1.123456789` prints `1.123457`.

## MINOR 5: Chi-square, the Expected column's cells do not add up to its printed total, unlike the other two columns

- `{"obs":"10, 10, 11","ratio":"1, 1, 1","dec":1}` → `P/chi_E_sum_dec1.png`. The Expected cells are 10.3 × 3 = 30.9,
  but the Expected total is 31.
- `{"obs":"5.5, 10.25","ratio":"1, 1"}` → `P/chi_frac_total.png`. The Expected cells are 7.88 + 7.88 = 15.76, against
  15.75.
- The contribution column IS forced to equal the sum of its printed cells, and so is the Observed column. The brief's
  rule ("every column's total is the sum of its printed cells") fails here. The values themselves are honest roundings.

## MINOR 6: Hardy-Weinberg, at six decimals p² never prints as the square of the printed p

- File: `hardyweinberg.rs:139-159` (`allocate`). For a p with 6 decimals, p²·10⁶ and q²·10⁶ always have equal
  remainders, because q² = 1 − 2p + p² and 2p·10⁶ is an integer. The tie goes to the smaller value.
- `{"p":0.123456,"n":1000,"dec":2,"counts":true,"sel":"against the recessive phenotype","gens":6,"label_size":"Large"}`
  → `P/hw_p123456_d2_sel6_L.png`. It prints `AA p² = 0.015242` beside `p = 0.123456`, whose square is 0.0152414.
  This is within one unit and is the documented sum-to-1 rule, but a student squaring the given p is marked wrong.

---

## Verified correct (rendered, looked at, recomputed independently)

Hardy-Weinberg (`.../scratchpad/r9c-tools/hw9.py`, `hw_mirror.py`, `hw_sizes.py`)
- Sweep: 203,904 renders.
  - p: every 0.01 step plus 0.001, 0.0001, 0.123456, 0.9999, 0.5, 0.333333, 0.1234567, 0.12345678, 0.876544,
    0.666667, 0.0101, 0.989999, 0.010001, 0.25, 0.75, 0.505, 0.495, 0.123455 and 0.999999.
  - N: 1, 2, 3, 10, 37, 99, 100, 500, 999, 1000, 9999 and 10000. dec 1-3, counts on/off.
  - Selection off and gens 2-6. Small/Medium/Large, and Large in Print.
- Every check gave 0 failures, against exact `Fraction` arithmetic:
  - p and q print as typed after the 6-decimal rounding: legend, givens and note agree.
  - Fractions sum to exactly 1, and each is within one unit of the exact value.
  - Counts are a valid exact largest-remainder allocation summing to N. Every row is within 1 of fraction × N.
  - One precision per row, bar and list. Never coarser than the typed dec or p's own decimals, never 7+ decimals, and
    no bare 0 in a fraction.
  - Bar labels fit their segments with real widths. Every generation's bar label is within one unit of that
    generation's exact genotype frequency.
  - The p list rises strictly and is never 1. The note uses the list's own first and last entries, and its "next"
    value is the exact next generation.
  - Canvas never overflowed, including Large with 6 generations at 6 decimals (`P/hw_p123456_d2_sel6_L.png`).
- The recurrence was derived from first principles. Survivors are AA (p²) and Aa (2pq), so
  p′ = (p² + pq) / (p² + 2pq) = 1/(1 + q) = 1/(2 − p) = p/(1 − q²). Recomputed for every printed list entry. Spot
  checks: 0.123456 → 0.532895 → 0.681614, and 0.99 → 0.990099.
- Mirror p vs 1 − p: 12,474 cases, every one an exact mirror apart from p = 0.5, N = 10 (accepted 3/2).
- Equal true values print equal: p = 0.5 at dec 1 gives 0.25/0.50/0.25 (`P/hw_p50_n10_d1_counts.png`). Generation 2
  at p = 2/3 gives AA = Aa = 0.44 (`P/hw_p50_n10_d1_sel6.png`).
- Over 2,520 size comparisons, no segment labelled at Medium is lost at Small or Large.
- Inputs:
  - The student copy blanks bars, counts and the list (`P/hw_student_sel6_L.png`).
  - NaN, negative, huge and string inputs clamp (`P/sheet_hw_a.png`).
  - p 0.1234567 prints 0.123457 with the rounding note (`P/hw_p1234567_d1.png`).

Enzyme (`enz.py`, `enz_strike.py`)
- Km label: 0 cases more than 45 px from its marker and 0 above the second row, in 2 × 17,720 cases. dfcbd79 had 496
  and 166.
- 0 label-label, label-point, label-tick or label-canvas collisions with real widths.
- Values recomputed for every haloed label and readout line: Km′ = Km(1 + [I]/Ki), Vmax′ = Vmax/(1 + [I]/Ki),
  Vmax′/2, v = Vmax′[S]/(Km′ + [S]), and the factor at 2 decimals. All agree within the printed precision, including
  `Km′ = 1,020` (competitive, [I] 10, Ki 0.2, Km 20). Earlier "failures" were my own parser on grouped or trimmed
  numbers.
- The student copy blanks the factor and both pairs (`P/sheet_enz_ext_0.png`).
- The default is byte-deterministic (`P/enz_default_a.png` = `P/enz_default_b.png`). The competitive Medium default is
  byte-identical to dfcbd79.

Chi-square (`chi.py`)
- Observed column: in the plain range, cells print as typed (0-6 decimals, grouped).
  - The Observed total is the exact decimal sum of the printed cells: `2,000,000,000.123455` for 1e9 + 0.123456 +
    999,999,999.999999 (`P/chi_1e9_frac6_L.png`) and 21.984375 (`P/chi_mixed_dec.png`).
  - The Expected total and the givens print the same string.
- At 1e14, 999,999,999,999,999 and a 2.5e15 total of cells below 1e15, the column stays plain and exact
  (`P/chi_6x1e14_L.png`). At 1e15 and above, the whole column is scientific (`P/chi_6x1e15_M.png`). χ², df, p and the
  decision were recomputed, including the 531/469 borderline (p = 0.050, "level", `P/chi_alpha_border.png`) and
  532/468 reject.
- No overlap or canvas overflow with real widths in 3,324 tables. The eighth critic's minors 4 and 5 are gone: 0
  `obs-total!=sum` and 0 `E-total` cases against 160 and 124 on dfcbd79.
- Zero observations, a single category, NaN/inf/negative counts and an empty box all refuse or note correctly
  (`P/sheet_chi_b.png`). The student copy blanks E and the stats (`P/chi_1e9_frac6_student.png`).
- The default is deterministic (`P/chi_default_a.png` = `P/chi_default_b.png`).

Cladogram (`clado.py`, `clado2.py`)
- Slanted ancestor strikes: 2 of 2,733 random trees (dfcbd79: 432), and 0 text overlaps (dfcbd79: 16). That is within
  the accepted ~4 per 2,500. The worst case is `Primates and others` at 11.4 px in a 40-leaf tree.
- Mixed-script random trees: 8 grazes of 0.7-1.4 px on accented Greek ancestors (`Ἄλφα`, `P/clado_greek_anc_struck.png`),
  which is the leading diacritic and is cosmetic. No canvas overflow.
- Earlier repros are clean: `P/clado_adj_slant.png` and `P/clado_adj2_slant_L.png`. One-child chains are clean
  (`P/clado_chain.png`).
- Latin rectangular trees: 402 of 402 are byte-identical to dfcbd79. The census baseline test passes with cladogram
  `4041fa1d6b9d93f9`. The default is deterministic (`P/clado_default_a.png` = `P/clado_default_b.png`).
- The 45-leaf Hangul list prints 39 rows plus "… and 6 more", with the number column end-aligned at 10+ rows.
- The 630-character tree gives the too-long message (`P/clado_500.png`). Unbalanced and empty trees give their
  messages.

## Not checked

- The live egui editor, slider steps and drags. The wasm/web renderer.
- PDF/DOCX export and share-link round trips.
- Fonts on other machines: every font finding is on this box's resvg path with system fonts.
- `food-web` and `energy-pyramid` (out of scope).

## Tests (frozen copy, `systemd-run` 8 GB cap, `CARGO_TARGET_DIR=.../scratchpad/r9-target`)

- `cargo test -j2 --lib figures`: **1451 passed, 0 failed, 4 ignored**, 519 filtered out (576.9 s).
- `cargo test -j2 --lib figuresui`: **82 passed, 0 failed, 0 ignored**, 1892 filtered out (72.7 s).
- These match the commit message. No test covers Major 1 (the half-rate label against the x-axis), Major 2 (an
  underestimated Latin run before a CJK/Hangul run) or Major 3 (wrapped givens against the readout).
