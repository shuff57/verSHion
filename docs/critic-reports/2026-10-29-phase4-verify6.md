# Phase 4 biology — sixth verification critic, build bab13f8

Source: frozen `git archive` of bab13f8 at `/tmp/fig-verify-r6` (read-only). Binary:
`/tmp/fig-verify-r6-target/debug/vershion`; the four module files in the archive are byte-identical
to the worktree at bab13f8 (`md5sum`). Renders in `/tmp/fig-verify-r6-pngs/`. For exact text and
geometry I also built an external scratch crate (`/tmp/fig-svgdump`, a path dependency on the frozen
copy, no edit to it) that dumps the SVG for arbitrary params; 3889 + 238 cases were dumped. One
cross-check (hw p = 0.06, n = 13) confirmed the scratch SVG text and the binary's PNG agree, so the
binary matches the source.

Scope: `hardy-weinberg`, `chi-square-fit`, `enzyme-kinetics`, `cladogram` only. `food-web` was not
touched.

## VERDICT: FAIL — 1 blocker, 2 majors, 1 minor

Reproduce each with
`VERSHION_RENDER_FIG=<id> VERSHION_FIG_PARAMS='<json>' VERSHION_OUT=<png> /tmp/fig-verify-r6-target/debug/vershion`.

---

## BLOCKER 1 — Hardy-Weinberg still prints a fraction that contradicts the count beside it

- Files: `src/figures/biology/hardyweinberg.rs:126-165` (`shown_dec`; the fallback is `:158-164`),
  `:143-147` (`agrees`), `:169-185` (`counts`), `:357-371` (the readout).
- What is wrong: the round-6 fix accepts a precision `d` only while `agrees` holds —
  `round(printed part[i] * N) == count[i]`. But `counts` allocates N by largest remainder, which can
  put a row a whole unit away from the nearest integer of its own exact frequency × N (p = 0.34,
  N = 9999 gives Aa = 4487 against 2pq·N = 4487.55). For many (p, N) **no** d in 1..6 can satisfy
  `agrees` (most often when N is not a power of ten, or a part sits on an exact half); the fallback
  at `:161-164` then ignores `agrees` entirely and returns the typed `dec` (only raised for a
  never-zero/collapse), so the page prints the coarsest, worst fraction set. This is exactly the
  failure the fifth critic's blocker named, one case over. The fix's own test grid (`:584-587`)
  happens to avoid every fallback case, so the tests stay green.
- Scale measured with the real source: in a grid of 102 allele frequencies (the 99 at 0.01 steps
  plus 0.333, 0.1234, 0.6667) × 12 populations × 3 decimal settings (3672 count-on renders),
  **651 (17.7 %) contradict in at least one row** — 222 of 1224 at the default `dec = 2`; 227
  distinct (p, N) pairs. p = 0.34 is fixed at N = 1000 but not at N = 9999 — the exact neighbouring
  case.

Repro A — the classic 1 : 2 : 1, a plausible N, one decimal (the brief's own grid):
`{"p":0.5,"n":999,"dec":1,"counts":true}` → `/tmp/fig-verify-r6-pngs/hw_bad_p50n999d1.png`
- Saw: `AA p² = 0.3 (250 of 999)`. 0.3 × 999 = 299.7 ≈ 300, not 250.
- `Aa 2pq = 0.5 (499 of 999)`. 0.5 × 999 = 499.5; the nearest is 500, not 499.
- `aa q² = 0.2 (250 of 999)`. 0.2 × 999 = 199.8 ≈ 200, not 250.
- The AA bar is plainly a quarter of the width while its label says 0.3.
- `dec = 2` at the same p, N prints `0.25 / 0.50 / 0.25` (only the 499.5 tie is left), so the
  precision the page chooses changes the answer by ~50 per row.

Repro B — brief grid, default decimal places:
`{"p":0.34,"n":9999,"dec":2,"counts":true}` → `/tmp/fig-verify-r6-pngs/hw_bad_p34n9999.png`
- Saw: `AA p² = 0.12 (1156 of 9999)` (= 1199.88), `Aa 2pq = 0.45 (4487 of 9999)` (= 4499.55),
  `aa q² = 0.43 (4356 of 9999)` (= 4299.57). All three rows contradict.

Repro C — brief grid, one decimal: `{"p":0.5,"n":9999,"dec":1,"counts":true}` →
`/tmp/fig-verify-r6-pngs/hw_bad_p50n9999d1.png` — `0.3 (2500)`, `0.5 (4999)`, `0.2 (2500)`.

Repro D — a classroom-plausible N: `{"p":0.87,"n":500,"dec":2,"counts":true}` →
`/tmp/fig-verify-r6-pngs/hw_bad_p87n500.png` — `0.76 (378 of 500)` (= 380),
`0.22 (113 of 500)` (= 110), `0.02 (9 of 500)` (= 10).

Repro E — the same with selection on:
`{"p":0.5,"n":999,"dec":1,"counts":true,"gens":6,"sel":"against the recessive phenotype"}` prints the
same `0.3 / 0.5 / 0.2` beside `250 / 499 / 250` (generation 1's readout is the same as the
no-selection one). Also `{"p":0.94,"n":999,"dec":2,"counts":true}` →
`/tmp/fig-verify-r6-pngs/hw_bad_p94n999.png` (`0.88 (883)` = 879.12; `0.01 (3)` = 9.99).

A correct fallback would take the best/finest precision that agrees (for p = 0.5, N = 999 that is
`dec = 2`, `0.25 / 0.50 / 0.25`) or print the count that the printed fraction implies, instead of
the typed coarse precision. As it stands the reported failure is alive for a large reachable family.

---

## MAJOR 1 — Enzyme-kinetics: the marked point is drawn on top of the half-rate / Km labels

- Files: `src/figures/biology/enzyme.rs:310` (`d.circle(px, py, 4.5, mark, ink, 1.2)` is drawn after
  the half-rate label at `:305`), `:322-333` (the rate label's candidate rows test `half_box`,
  `under_box` and `vmax_box`, never the point), `:295-305` (the half-rate label's candidate rows
  likewise). The marked point is in no collision box at all.
- Repro A — an ordinary textbook noncompetitive case:
  `{"mode":"noncompetitive","inh":3,"ki":1,"vmax":10,"km":2,"smax":40,"at":6}` →
  `/tmp/fig-verify-r6-pngs/enz_bad_g56.png`, crop `/tmp/fig-verify-r6-pngs/crop_g56_half.png`
  - Saw: the red dot sits in the middle of `Vmax′/2 = 1.25`, covering the `/2`; the label reads
    `Vmax′●2 = 1.25`. The dot also sits on the curve.
- Repro B — the fifth critic's own s6 case:
  `{"mode":"noncompetitive","inh":3,"ki":1,"vmax":50,"km":0.2,"smax":2,"at":0.5,"label_size":"Large","dec":1}`
  → `/tmp/fig-verify-r6-pngs/enz2_enz_s6.png`, crop `/tmp/fig-verify-r6-pngs/crop_s6c.png` — the dot
  covers the final digit of `Vmax′/2 = 6.2`.
- Repro C: `{"mode":"noncompetitive","inh":3,"ki":1,"vmax":10,"km":2,"smax":12,"at":2,"label_size":"Large"}`
  → `/tmp/fig-verify-r6-pngs/enz_bad_g193.png`, crop `crop_g193_km.png` — the dot clips the top of
  `Km′ = 2`.
- Scale: a 216-case sweep (mode × vmax × km × smax × [S] × [I] × Ki × label size) found 5
  point-over-label overlaps, all this class.
- The round-6 halo work itself is present and works: Km/Vmax/half/rate/note labels all carry the
  `fill="none" stroke="#fff"` halo and the red dashed comparison curve no longer cuts the blue
  glyph strokes (`enz_s4m`, crop `crop_s4mb.png`). The marker is the one drawn element with no
  collision test.

---

## MAJOR 2 — Cladogram: the "Left to right:" line renders as tofu when one line mixes Japanese and Hangul

- Files: `src/figures/biology/cladogram.rs:245-257` (`leaf_order` builds ONE string for the whole
  line) and `:392-397` (drawn as ONE `<text>`). A line holding both a Han and a Hangul name is a
  single mixed-script run, and the PNG rasteriser emits `.notdef` boxes for it. Each leaf label is
  its own `<text>` and renders correctly; the order line is the only place the runs are concatenated.
- Repro (all label sizes): `{"tree":"(日本語のテスト, 한국어이름)"}` →
  `/tmp/fig-verify-r6-pngs/mixrun_Small.png`, `mixrun_Medium.png`, `mixrun_Large.png`
  - Saw: the two leaves read `日本語のテスト` and `한국어이름`; the line under the picture reads
    `Left to right: □□□□□□, □□□□□` at every size.
- Brief's mixed case: `{"tree":"(Homo sapiens, 日本語のテスト, 한국어이름, 🦕🦖, B)"}` →
  `mixed2_Small.png`, `mixed2_Medium.png`, `mixed2_Large.png` — the Han+Hangul segment is tofu at all
  three sizes; at Small the emoji in the same run is tofu too.
- Latin+Han (`Left to right: 日本語…, B`) and Latin+Hangul render fine (`cjk2_Large.png`,
  `mixrun_latin_hangul.png`); the trigger is Han+Hangul in one run.
- The round-6 canvas-width fix itself holds: CJK-only names now fit at every size (`cjk2_Large.png`,
  `clado_cjk_Small.png`), the 500-char tree says "The tree is too long" and does not blame the
  brackets (`clado_cap.png`), the short unbalanced tree and the single name render, and every text
  extent checked (CJK, Hangul, emoji, 29 Ws, 60-char single name, twelve binomials, comb, ancestor,
  root, slanted, at Small/Medium/Large) stays inside the canvas. Splitting the order line into one
  `<text>` per name (or per script run) would avoid the rasteriser's per-run fallback failure.

---

## MINOR 1 — Chi-square huge mode still prints its observed counts and Total as `1.00e0` / `2.00e0`

- Files: `src/figures/biology/chisquarefit.rs:340` (the `huge` predicate) and `:343`
  (`fmt_cnt = sci` for every count), used at `:348` and `:393`.
- Repro: `{"names":"A, B","obs":"1, 1","ratio":"1, 100000000000000000000"}` →
  `/tmp/fig-verify-r6-pngs/chi_huge.png`
  - Saw: the ratio now prints honestly (`1 : 1.00e20`; no i64 saturation — the round-6 fix holds),
    but the Observed cells print `1.00e0` and the total `2.00e0`. The fifth critic's Minor 5 had two
    halves, saturation and this; only the saturation half was fixed. Cosmetic, not wrong.

---

## Verified correct (rendered and looked; values independently recomputed)

Hardy-Weinberg
- The fifth critic's two reported cases are fixed: p = 0.34, N = 1000 prints `0.116 (116)`,
  `0.449 (449)`, `0.435 (435)`; p = 0.06, N = 9999 prints `0.0036 (36)`, `0.1128 (1128)`,
  `0.8836 (8835)`. Each sums to 1 and each fraction × N rounds to its count.
- Selection independently recomputed (p′ = p/(1 − q²)) and the trajectory matches: p = 0.4 →
  `0.40, 0.62, 0.73, 0.79, 0.82, 0.85`; p = 0.01 → `0.0100, 0.5025, 0.6678, 0.7506, 0.8004, 0.8336`;
  p = 0.99 at dec 1 → `0.9900 … 0.9905`. Generation genotype bars match the printed precision.
- Rows and the generation list carry one fixed precision (no `AA 0.53` beside `Aa 0.4`; no
  `0.99, 0.9901`). All bar labels in the 84 label-size cases carry the page precision; there is no
  precision mixture in any bar row.
- No Medium-labelled segment loses its label at Large or at Small (14 p values × sel on/off × 3
  sizes); no label overflows its bar (the bar grows with the label and the 0.55 em estimate is
  conservative for these glyphs).
- The bar width scales with the label size and the drag inverse uses the drawn width.
- Student copy blanks the genotypes and counts and keeps the pool/legend/givens.
- p clamps to [0.01, 0.99], N to [10, 10000]; NaN/inf/`dec:"x"` do not panic and clamp sensibly.

Chi-square
- Zero observations: `obs:"0, 0"` and `"0, 0, 0"` print `Chi-square χ² = — (no observations to
  test)`, `p = —`, `The test cannot be run`, and **no decision**; the editor warning fires. A single
  category prints `χ² = — (no degrees of freedom)`. Student copies show blanks, not a decision.
- Mendel: E = 312.75/104.25/104.25/34.75, terms 0.0162/0.1013/0.1349/0.2176, χ² = 0.47, df = 3,
  p = 0.93, do not reject (independent incomplete-gamma check).
- Decimal ratio `1.5 : 2.5 : 1.5` prints as typed; E = 109.09/181.82/109.09, terms 0.76/1.82/0.76,
  total 3.34 (sum of printed), p = 0.19.
- α boundary: 531/469 → χ² 3.84, printed p 0.050, "level with α = 0.05 at this precision"; 532/468
  rejects.
- 1 : 0 ratio with a count in the empty part: row ∞, Total ∞, χ² ∞ with its reason, p = —, refuses;
  an empty zero part with nothing counted carries no degree of freedom.
- 1e160 counts → ∞ with "the counts are too large", not "inf".
- 19 normal tables checked cell-by-cell against recomputed E and (O−E)²/E; the Total row equals the
  sum of the printed contributions (0.0133 for 301/99; 1.42 for 2,3,4,5; 3.34; 50.00 for six
  categories).
- Student copy blanks the Expected cells and the Expected Total (556 no longer leaks); the observed
  total stays, as it is the question.

Enzyme
- v = Vmax·[S]/(Km+[S]); competitive Km′ = Km(1 + [I]/Ki) with Vmax fixed; noncompetitive
  Vmax′ = Vmax/(1 + [I]/Ki) with Km fixed. 25 parameter cases recomputed: givens, both readout
  pairs, the factor, the marked rate, the plotted Vmax′/Km′, the half label (Vmax′/2 when inhibited)
  and the beyond-the-axis note all match to the printed decimal.
- Student copy blanks the curve, the constants, the rate **and** the derived factor
  (`1 + [I]/Ki = _____`), keeping the axes, ticks and input givens; the fifth critic's leak is gone.
- Km/Vmax/half/rate/note labels all carry the halo; the asymptote and dotted markers no longer cut
  the glyphs (default, strong noncompetitive, s6, s4m, beyond, Large, Small).
- Clamps Vmax→[1,100], Km→[0.2,20], [S]max→[2,40], Ki→[0.2,10], [I]→[0,10]; NaN/inf/`dec:"y"` do not
  panic.

Cladogram
- Parser refusals unchanged and correct; `(A)` is "1 leaf, 0 branching points, 1 level"; leaves in
  typed order; each ancestor label drawn once at its fork; labelled root inside the grown margin;
  rectangular and slanted shapes differ.
- CJK-only names fit at Small/Medium/Large (round-6 width fix holds); 29 Ws, the 60-char single
  name, the twelve-binomial wrap, and comb/ancestor/root/slanted at all sizes all fit the canvas.
- 500-char tree: `The tree is too long: The box holds at most 500 characters…`, no bracket blame;
  a genuinely unbalanced short tree still says "A bracket is never closed."; a single name renders.

Global
- Render purity: the four defaults repeated byte-identical.
- Print copies are grey-only; the enzyme Print grid is `#787878`.
- Census: the commit's fixture diff touches only `enzyme-kinetics__defaults/__start`, which is the
  baseline's only pair for these four (the baseline hashes defaults/start with show_answers on, so
  the student-copy changes are not in it).

---

## Not checked

- The live egui preview and dragging the two handles with a real pointer (the module tests pin the
  inverse arithmetic; the drawn geometry was checked).
- The wasm/web renderer and the PDF/DOCX export path. The mixed-script tofu was measured on the PNG
  (resvg) path, which is the export path.
- Share-link/saved-figure round trip and a full `census_dump` diff for every case (the committed
  defaults/start fixture diff was read; the rest is covered by the test suite).
- The rest of the biology tab (`food-web` is another session's work).

---

## Tests (frozen copy, own target dir, `systemd-run` 8 GB cap)

- `cargo test -j2 --lib figures`: **1400 passed, 0 failed, 4 ignored, 519 filtered out** (589.69 s).
- `cargo test -j2 --lib figuresui`: **82 passed, 0 failed, 0 ignored, 1841 filtered out** (62.34 s).
- Both counts match the builder's claim (1400/0/4 and 82/0/0); no test was deleted or loosened. The
  failing family above has no test: the round-6 test grid at `hardyweinberg.rs:584-587` avoids every
  fallback case.
