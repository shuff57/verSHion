# Phase 4 biology — fifth verification critic, build 57a8fe0

Source: frozen `git archive` of 57a8fe0 at `/tmp/fig-verify-57a8fe0` (read-only). Binary:
`/tmp/fig-verify-57a8fe0-target/debug/vershion`. Renders in `/tmp/fig-verify-pngs/`. The binary was
confirmed to contain the round-6 fixes (a decimal ratio prints "1.5 : 2.5 : 1.5"; a 95-char single
name sizes its canvas), so it matches the source.

Scope: `hardy-weinberg`, `chi-square-fit`, `enzyme-kinetics`, `cladogram` only. `food-web` not touched.

## VERDICT: FAIL — 2 blockers, 3 majors, 5 minors

Reproduce each with
`VERSHION_RENDER_FIG=<id> VERSHION_FIG_PARAMS='<json>' VERSHION_OUT=<png> /tmp/fig-verify-57a8fe0-target/debug/vershion`.

---

## BLOCKER 1 — Hardy-Weinberg prints a frequency and a count that contradict each other

- Files: `src/figures/biology/hardyweinberg.rs:321-335` (the readout prints `printed_parts`),
  `:99-119` (`printed_parts`, largest remainder at the page precision), `:144-160` (`counts`, largest
  remainder of the **exact** frequencies × N), `:124-140` (`shown_dec`, which raises the precision only
  when a part would print as 0).
- Repro A: `{"p":0.34,"counts":true}` → `/tmp/fig-verify-pngs/hardy-weinberg__p34.png`
  - Saw: `AA p² = 0.12 (116 of 1000)` — 0.12 × 1000 = **120**, not 116.
  - Saw: `aa q² = 0.43 (435 of 1000)` — 435/1000 = 0.435, which rounds to **0.44**, not 0.43.
  - The counts sum to 1000 and the fractions sum to 1.00, but they come from two different roundings.
    The count is not the printed fraction × N in any row (116 vs 120, 449 vs 450, 435 vs 430), and
    the aa row visibly contradicts itself: 435 of 1000 is 0.435, which rounds to 0.44, not the 0.43
    printed. This is the same failure the ray-diagram round called a blocker ("Magnification −0.7,
    image height −2.7" beside h = 4).
- Repro B (worse): `{"p":0.06,"n":9999,"counts":true}` →
  `/tmp/fig-verify-pngs/hardy-weinberg__p06.png`
  - Saw: `AA p² = 0.01 (36 of 9999)`. True p² = 0.0036; 36/9999 = 0.0036; 0.01 × 9999 = 100. The
    printed **fraction is 2.8× the true value**. Largest-remainder hit an exact tie (AA .36, aa .36)
    and bumped AA from 0.00 to 0.01; because 0.01 is nonzero, `needs_more` never fired, so the
    precision was not raised to 4 and 0.0036 was never printed. Bumping the other tied part would
    have produced 0.00, fired the never-zero rule and printed 0.004/0.113/0.883.
  - `{"p":0.07,"counts":true}` → `hardy-weinberg__p07.png`: same, `AA p² = 0.01 (5 of 1000)` for a
    true 0.0049.
- The module's own doc (`:95-98`) says each printed part is "the nearest it can be to its true value";
  at p = 0.06 the printed AA is not the nearest, and the count on the same line says so.

## BLOCKER 2 — Chi-square runs a test on zero observations and reports a confident pass

- Files: `src/figures/biology/chisquarefit.rs:92-135` (with `total_obs = 0` every expected count is 0;
  `kept` still counts the ratio parts, so `df = 1`; `chi2 = 0`; `p = Some(1 - chisq_cdf(0,1)) = 1`),
  `:137-178` (`warnings` says nothing for this input).
- Repro: `{"names":"A, B","obs":"0, 0","ratio":"1, 1"}` →
  `/tmp/fig-verify-pngs/chi-square-fit__allzero.png`
  - Saw: `Expected from the ratio 1 : 1 out of 0 observations`, rows `E = 0.00`, `χ² = 0.00`,
    `df = 1`, `p = 1.00`, and `p = 1.00 is above α = 0.05, so do not reject the hypothesis: the
    counts are consistent with the ratio.`
  - Also `{"obs":"0, 0, 0","ratio":"0, 1, 1"}` → `chi-square-fit__zeroratio.png`, same block.
  - (O−E)²/E is 0/0 for every row; the page prints it as 0.00 and draws a conclusion from it. This is
    the round-1 blocker B1 pattern ("a test that never ran reported df = 1 … do not reject") one case
    over: the ratio parts are nonzero, the *data* are empty.
- Related, same file: the single-category case `{"names":"Only","obs":"10","ratio":"1"}` →
  `chi-square-fit__single.png` prints `χ² = 0.00` for a test whose decision line correctly says it
  cannot be run (`df = 0`); the statistic row could blank instead.

## MAJOR 1 — Hardy-Weinberg drops genotype labels at Large that it draws at Medium

- Files: `src/figures/biology/hardyweinberg.rs:281-283` (`if tw <= wseg - 8.0`), `:22` (`BAR_W`
  fixed at 452), `:280` (the label width scales with `12.0 * f * 0.55`).
- Repro: `{"label_size":"Large"}` → `/tmp/fig-verify-pngs/hardy-weinberg__large.png`;
  `{"style":"Print","label_size":"Large"}` → `hardy-weinberg__print-large.png`.
  - Saw: in every generation bar the orange segment is bare — `aa 0.16` is drawn at Medium
    (`hardy-weinberg__print-medium.png`) and gone at Large. 0.16 × 452 = 72.32; the label at
    f = 1.4 needs 7 × 12 × 1.4 × 0.55 = 64.68 against an allowance of 64.32.
  - `{"p":0.4,"sel":"against the recessive phenotype","gens":6,"style":"Print","label_size":"Large"}`
    → `hardy-weinberg__selprint-large.png`: Gen 1 loses `AA 0.16`, which Medium draws
    (`hardy-weinberg__selection.png`).
  - The canvas widens by 55 % at Large (`:222`) but the bars do not, so the size control removes
    annotations instead of enlarging them. (At Medium the rule also drops the AA label for p = 0.3 and
    0.34, e.g. `hardy-weinberg__p34.png`; there the segment really is narrow and the readout covers
    it, but at Large the segment is the same width as Medium's while the label grew.)

## MAJOR 2 — Enzyme-kinetics: the Km and Vmax constants are the only plot labels with no halo, and lines are drawn through them

- Files: `src/figures/biology/enzyme.rs:263-269` (both use `text_el`, not `text_el_halo` at `:122`),
  `:267-268` (the dotted Km marker line is drawn at the same x the label is centred on),
  `:302`, `:332` (the half-rate and rate labels do get halos).
- Repros / what is in the PNG:
  - `{}` → `/tmp/fig-verify-pngs/enzyme-kinetics__default.png`: the vertical dotted Km line runs
    straight through `Km = 2` (between "Km" and "="). Same at `{"km":10,"smax":40,"at":20}` →
    `enzyme-kinetics__highkm.png` and in every inhibited render.
  - `{"mode":"noncompetitive","inh":10.0,"ki":0.2}` → `enzyme-kinetics__strongnon.png`
    (crop `crop_enz_strong_labels.png`): the near-flat inhibited curve runs through the bottom half of
    `Km′ = 2`, which is the same blue as the curve.
  - `{"mode":"noncompetitive","inh":3,"ki":1,"vmax":50,"km":0.2,"smax":2,"at":0.5,"label_size":"Large","dec":1}`
    → `enzyme-kinetics__s6.png` (crop `crop_s6.png`): the curve and the dotted line both strike
    `Km′ = 0.2`.
  - `{"mode":"noncompetitive","inh":0.5,"ki":10,"vmax":100,"km":0.2,"smax":40,"at":1,"dec":3}` →
    `enzyme-kinetics__s4m.png` (crop `crop_s4m.png`): the uninhibited dashed comparison curve runs
    through the lower half of `Vmax′ = 95.238`.
  - Conventions (`docs/figures-conventions.md:9`) require a thin halo for a label over a curve; the
    half-rate and rate labels got theirs in rounds 3-4, these two never did, and no collision box
    tests a label against a drawn curve.

## MAJOR 3 — Cladogram clips names that are not Latin

- Files: `src/figures/biology/cladogram.rs:193-206` (`text_w`: every glyph not in the Latin table is
  0.56 em, including CJK and emoji), `:288-295` (canvas width, leaf-order wrap and count line all use
  `text_w`).
- Repro: `{"tree":"(日本語の名前がとても長い場合のテストですこれはどうなるでしょうか, B)"}` →
  `/tmp/fig-verify-pngs/cladogram__cjk.png`
  - Saw: the leaf label is cut off at the right canvas edge (…テストですこ), and the
    `Left to right: …` line under it is cut at the same edge (…これはど). The glyphs render (font
    fallback), at roughly 1 em each, so the canvas is ~45 % too narrow.
  - `{"tree":"日本語の名前がとても長い場合のテストですこれはどうなるでしょうか"}` →
    `cladogram__cjk-single.png`: both the leaf label and the order line clipped.
  - The round-5 per-glyph fix handles `W`/`M` (29 Ws, `cladogram__wide-W.png`, fits) and the Latin
    twelve-name case (`cladogram__twelve.png`, fits); CJK is the class it does not know.

## MINOR 1 — A tree longer than 500 characters is silently truncated, then refused with a false reason

- Files: `src/figures/mod.rs:260` (every `Kind::Text` value is cut to 500 chars by `normalize`),
  `src/figures/biology/cladogram.rs:55-104` (parses only what it is given, so it cannot say why).
- Repro: the balanced 670-char tree (130 names, full JSON also at
  `/tmp/fig-verify-pngs/cap-json.txt`) → `/tmp/fig-verify-pngs/cladogram__cap.png`

```json
{"tree": "(S0, S1, S2, S3, S4, S5, S6, S7, S8, S9, S10, S11, S12, S13, S14, S15, S16, S17, S18, S19, S20, S21, S22, S23, S24, S25, S26, S27, S28, S29, S30, S31, S32, S33, S34, S35, S36, S37, S38, S39, S40, S41, S42, S43, S44, S45, S46, S47, S48, S49, S50, S51, S52, S53, S54, S55, S56, S57, S58, S59, S60, S61, S62, S63, S64, S65, S66, S67, S68, S69, S70, S71, S72, S73, S74, S75, S76, S77, S78, S79, S80, S81, S82, S83, S84, S85, S86, S87, S88, S89, S90, S91, S92, S93, S94, S95, S96, S97, S98, S99, S100, S101, S102, S103, S104, S105, S106, S107, S108, S109, S110, S111, S112, S113, S114, S115, S116, S117, S118, S119, S120, S121, S122, S123, S124, S125, S126, S127, S128, S129)"}
```

  - Saw: `The tree could not be read: A bracket is never closed.` although the typed tree is balanced
    (the shared 500-char cap cut it mid-name). A 150-name tree (`stress-c150.png`) does the same.
    Through a share link the stored tree is cut and the page blames the brackets; no note says the
    text was shortened.

## MINOR 2 — The enzyme student copy prints the derived inhibition factor

- Files: `src/figures/biology/enzyme.rs:337-350` (the givens keep `(1 + [I]/Ki = 5)` when inhibited).
- Repro: `{"show_answers":false,"mode":"competitive"}` →
  `/tmp/fig-verify-pngs/enzyme-kinetics__compstudent.png` (and `student-large` with an inhibitor):
  - Saw: `With the inhibitor: Vmax = _____, Km = _____` is blanked, but the givens line hands the
    student `1 + [I]/Ki = 5`, the multiplier the blanks are computed from. `Without it: Vmax = ___,
    Km = ___` is also answerable from `Vmax = 10, Km = 2` on the same line.

## MINOR 3 — Hardy-Weinberg mixes precisions on one page

- Files: `src/figures/biology/hardyweinberg.rs:280` (`val(shown[i], dp)` trims trailing zeros),
  `:344` (the "p in each generation" list).
- Repro: `{"p":0.4,"sel":"against the recessive phenotype","gens":6,"style":"Print","label_size":"Large"}`
  → `hardy-weinberg__selprint-large.png`: Gen 3 prints `AA 0.53` beside `Aa 0.4`.
  `{"p":0.99,"dec":1,"sel":"against the recessive phenotype","gens":6}` →
  `hardy-weinberg__sel-onedec.png`: `AA 0.981` beside `AA 0.9809` on the previous row, and
  `p in each generation 0.99, 0.9901, …`. The values are right; the rows just do not line up at a
  fixed number of decimals.

## MINOR 4 — The chi-square student copy prints the expected Total

- File: `src/figures/biology/chisquarefit.rs:380` (prints `fmt_cnt(total_obs)` unconditionally in the
  Expected column).
- Repro: `{"show_answers":false}` → `/tmp/fig-verify-pngs/chi-square-fit__student.png`
  - Saw: the Expected cells are `_____` but the Expected Total row shows `556`. It equals the observed
    total by construction, so it is not a real answer, but it is an unlabelled number in a column the
    student is asked to fill.

## MINOR 5 — Chi-square huge mode saturates a ratio part and prints counts as "1.00e0"

- File: `src/figures/biology/chisquarefit.rs:394` (`format!("{}", v.round() as i64)` saturates at
  i64::MAX; the `huge` branch at `:329` sends every count through the 2-dp scientific formatter).
- Repro: `{"names":"A, B","obs":"1, 1","ratio":"1, 100000000000000000000"}` →
  `/tmp/fig-verify-pngs/chi-square-fit__tinythenhuge.png`
  - Saw: `Expected from the ratio 1 : 9223372036854775807` for a typed `1 : 1e20` (the expected
    counts are computed from 1e20, so the givens line contradicts the column). The observed counts
    print as `1.00e0`, and the Total as `2.00e0`.

---

## Verified correct (recomputed by hand / with an independent chi-square implementation)

Hardy-Weinberg
- p², 2pq, q² and their sum at p = 0.6 (0.36/0.48/0.16), 0.34 (0.1156/0.4488/0.4356), 0.06, 0.07,
  0.99, 0.01; printed largest-remainder sets at every dp 1-4 reachable (0.3/0.5/0.2 at one decimal,
  0.12/0.45/0.43, 0.9801/0.0198/0.0001), each summing to 1 at the printed precision.
- Counts by largest remainder sum to N exactly at N = 10, 37, 1000, 9999, 10000 (360/480/160 of 1000;
  1/4/5 of 10; 36/1128/8835 of 9999).
- Selection: p′ = p/(1 − q²) checked per generation (0.4 → 0.625 → 0.727273 → 0.785714 → 0.823529
  → 0.85, printed 0.4, 0.62, 0.73, 0.79, 0.82, 0.85); the "p in each generation" column prints at
  the raised page precision (p = 0.99 at dec = 1 prints 0.99, 0.9901, …); genotype sums = 1 in
  every bar.
- Never-zero: p = 0.01 prints AA 0.0001, not 0; p = 0.99 at dec = 1 prints p = 0.99, q = 0.01.
- Clamps p→[0.01,0.99], N→[10,10000], gens→[2,6] behave (`pzero`, `pneg`, `pabove`, `nmin`, `nmax`);
  the drag inverse matches the drawn boundary (test + `Report.split`).
- Print copies are grey-only and survive a 66 % bitonal copy with three distinguishable hatch
  directions (`hardy-weinberg__print-medium-bw66.png`); labels sit on white plates.

Chi-square
- Mendel: E = 312.75/104.25/104.25/34.75; terms 0.0162/0.1013/0.1349/0.2176; χ² = 0.47002, df = 3,
  p = 0.92543 (independent incomplete-gamma check) → prints 0.47 and p = 0.93, "do not reject".
- Printed total = sum of printed terms: 0.0133 (301/99), 1.42 (2,3,4,5), 0.34 (1,2), 47.00 (big),
  11.25, 5.00, 3.34; the table's Total row always carries the same string as the χ² line.
- p at 531/469 = 0.049924 → prints `p = 0.050`, equal to α = 0.05, and the decision says "level with
  α = 0.05 at this precision" (the round-3 fix holds). 532/468 → χ² = 4.096, p = 0.043, reject.
- Six categories 10-60 vs 1:1:1:1:1:1 → χ² = 50, df = 5, p = 1.4e-9, p < 0.001.
- 1:0 ratio with a count in the zero part → row ∞, Total ∞, χ² ∞ with its reason, p = —, and the test
  refuses; the empty zero part with nothing counted carries no degree of freedom; single category →
  df = 0, refuses.
- 1e160 counts → ∞, not "inf"; thousands grouping on both Observed and Expected (31,500 /
  31,275.00); contributions grouped (10,229.77); decimal ratios print as typed (1.5 : 2.5 : 1.5);
  over-long names shortened with the observed column's width reserved; all lines fit the canvas at
  Medium, Large and Small.
- Alpha clamped to [0.01, 0.10]; decision is judged on the printed p (no "p = 0.050 is below α = 0.05").

Enzyme kinetics
- v = Vmax·[S]/(Km+[S]): default v(6) = 7.5; competitive [I] = 4, Ki = 1 → factor 5, Km′ = 10,
  Vmax = 10, v(6) = 3.75; noncompetitive → Vmax′ = 2, Km = 2, v(6) = 1.5; strong noncompetitive
  [I] = 10, Ki = 0.2 → factor 51, Vmax′ = 0.196, v(6) = 0.15; competitive Km = 20 → Km′ = 100,
  named "beyond this axis", v(6) = 0.566 → 0.57; Vmax = 100, Km = 20, [S] = 40 → 66.667; [S] = 0 →
  v = 0.
- Primes match: Vmax′/Km′ on the plot when inhibited, plain names when not; the half-rate label is
  Vmax′/2 when inhibited (round-4 fix holds); the "Without it" line is the typed pair (round-3 fix
  holds); both curves are drawn, the comparison dashed (round-2 fix holds).
- Clamps Vmax→[1,100], Km→[0.2,20], [S]max→[2,40], Ki→[0.2,10]; at > smax is held at the edge and
  reported; Km past the axis prints its note; the rate label flips left at the right edge and stays
  inside the plot (round-5/6 fixes hold).
- Print grid is #787878 and survives 66 % bitonal (`enzyme-kinetics__print-medium-bw66.png`);
  student copy blanks the curve, the constants and the rate while keeping the axes, tick numbers and
  givens.

Cladogram
- Parser: nested, labelled (ancestor and root), trailing semicolon, whitespace, names with spaces
  (Homo sapiens), one-child group `(A)`, and every refusal I tried — `(A, B` "never closed", `()`
  "nothing inside", `A, B` "Unexpected ',' after the end", `(A, B))` "Unexpected ')'", `""` "ends
  where a name was expected". No line numbers exist in Newick, so none are expected.
- Layout: leaves in typed order, every leaf on its own row at full depth, branches from parent depth
  to child depth along the child's row (star of five draws five distinct branch rows, nested trees
  start on a bar spanning their row), one bar per branching point, slanted edges are true diagonals,
  equal step length.
- Ancestor labels drawn once at their fork (labelled root inside the grown left margin); every leaf
  name drawn once; counts grammatical and correct ("4 leaves, 3 branching points, 3 levels",
  "(A)" → "1 leaf, 0 branching points", 12 leaves wrap inside the canvas, 95-char single name sizes
  the canvas to fit).
- No show_answers on this figure (correct); Print grey-only.

Global
- Determinism: the four default renders repeated byte-identical
  (`hardy-weinberg__def2.png`, `chi-square-fit__def2.png`, `enzyme-kinetics__def2.png`,
  `cladogram__def2.png`).
- Purity: no panic and < 1 s on 2000-deep trees, 500-leaf trees, 100 categories, 1e9 inputs, empty
  text, non-numeric tokens, NaN/inf strings, and out-of-range numbers.
- 151 PNGs produced across Colour/Print × Medium/Large/Small, student, key, the data cases named
  above and the stress sweep; every distinct render was inspected (byte-identical repeats and the
  stress duplicates were checked in a montage).

## Not checked

- The editor Notes box itself (the `warnings()` strings were verified through the tests and the code,
  not rendered in the running app — no GUI session).
- Dragging the two handles in the UI (the inverse arithmetic and handle positions are verified by the
  module tests and the `Report` geometry, not by a real drag).
- The wasm/web renderer (font fallback for CJK there), and the PDF/DOCX export path.
- Share-link / saved-figure round trip and the census byte-identity for these four (tests assert them;
  I did not re-dump the census).
- The rest of the biology tab (food-web is another session's work; gel, pedigree, punnett, popdyn,
  pipette are out of scope).

## Tests (frozen copy, own target dir, `systemd-run` 8 GB cap)

- `cargo test -j2 --lib figures`: **1371 passed, 0 failed, 4 ignored, 519 filtered out** (478.67 s).
- `cargo test -j2 --lib figuresui`: **82 passed, 0 failed, 0 ignored, 1812 filtered out** (53.25 s).
- No test was deleted or loosened.
