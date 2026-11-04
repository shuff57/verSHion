# periodic-table — blind critic 1 (f31433c)

Verdict: **FAIL — 0 blocker, 3 major, 2 med, 4 low, 3 minor.** 22/22 module tests pass; census passes; nothing deleted.

## MAJOR
1. **Wrong advice for an f-block highlight.** `read()` (periodic.rs:296-302) says "choose {With transition metals} or {Full with f-block} to see it" for any hidden element, but `block_shows(1,57)==false`. With Block=Main group, Highlight=La (or Ac, Ce…Lr) the note names a setting that still hides it; with Block=With transition metals it names the current setting. Repro: block=Main group, highlight=La — `Report.notes` says "…choose With transition metals or Full with f-block…". No test pins La/Ac — only Pd (tests.rs:822). Fix: when `is_f(z)`, name only BLOCKS[2].
2. **The Print key is not reliably legible, and the test that "proves" it is a false pass.** Swatches are 26.40×16.32 (Medium) and 21.12×13.06 (Small): 2.2×1.36 and 1.76×1.09 of the 12 px tile, so the Small key shows barely one period of the repeat and the eye has no second repeat to read. At 66 % (1.5× resvg PNG, downscaled, threshold 150) metalloid (cat 4, dots) has 5 % ink at Medium (7 dark px of 144) and 9 % at Large — effectively blank; lanthanide (cat 8, 3 px zigzag) prints as two rows of horizontal dashes, a glance-match for noble gas (cat 7, dashed horizontals); alkali/alkaline earth (0/1) differ only in diagonal direction. Fix: ≥24×24 swatches, a one-letter code in each swatch/cell (A, AE, T, P, M, N, H, G, L, An), merge lanthanide+actinide and alkali+alkaline-earth in Print, denser metalloid mark. **Proof the test is invalid:** replacing pattern 1 with pattern 0's path in svg.rs:350 (two categories byte-identical) leaves all 22 tests green (mutant M8). The test compares thresholded *crops* of swatches at different positions (phase 7.76 vs 9.66 mod 12), so it measures phase, not pattern identity; it must compare the `<pattern>` strings.
3. **The names contradict the authority the figure cites.** Header/FAQ/key say it follows periodic-table.rsc.org, which spells **Aluminium** and **Caesium**; data.rs:18/60 print "Aluminum"/"Cesium". The "independent" NAMES list (tests.rs:20-24) copies the same spellings, so no test can catch it. Fix: use the cited spellings (watch other chemistry figures and the census), or stop claiming the RSC's spelling.

## MED
1. **The trend caveat is not in the SVG.** The electronegativity setting draws "increases across a period / decreases down a group" with no printed exception; He/Ne/Ar have no Pauling value and d/f are irregular — only in the param hint and FAQ, which do not export. A handout asserts a rule the header itself calls a simplification.
2. **The metalloid source claim is overstated.** The RSC's own polonium page says "It is classed as a metal" and the RSC table colours the p-block by group (B/Al/Ga/In/Tl share a class; Si/C/Ge/Sn/Pb another), not by the ten categories. La/Ac with group 3 empty *does* match the RSC's table (verified from its element classes), but "the RSC's metalloid key" for B…Po is contested: say "the common teaching convention; the RSC's own Po page calls it a metal".

## LOW
- Colour: `#4e79a7` (transition metal) vs `#111` text = 4.15:1; the 10.56 px atomic number fails WCAG AA (the bold symbol passes).
- The default main-group view's key still lists transition metal / lanthanide / actinide, which have no cell there; mark them "not in this view".
- No on-canvas title or "main-group excerpt" caption; an exported PNG shows group numbers jumping 2→13 with nothing explaining it.
- `suggest()`'s doc says "A" suggests Ag; it returns Al (index 12 beats Ag 46).
- The student copy's `<desc>` says the panel "reads out the highlighted element" while it is blank.

## minor
- `CategoryFills::fill` wraps after ten silently; `defs()` says 2 px marks but widths are 1.6/1.8/2.2.
- FAQ: "the later periods have eighteen" — periods 6-7 have 32 elements; only the grid has 18 places.
- The echo line prints the typed atomic number, so a student given "79" can copy it into the Z blank.

## Verified (recomputed independently)
All 118 symbols/names/Z; group+period against my own 18-column grid (H 1/1, He 18/1, 57-71/89-103 f-block, group 3 empty in periods 6/7, Lu/Lr in the strips) — 0 mismatches; counts 6/6/38/11/7/7/6/7/15/15, one category each. Valence 1/2/g−10, He=2, H=1, Ne/Ar/Og=8, none for d/f. One cell per element (118 full, 50 main, 88 d-block); strips 15 wide under group 3; labels 1,2,13-18; highlight any case/079/typo advice/refusals; student and key same size, no answer text; hostile and 10 k-fuzz pure and in budget; Print grey-only; census baseline passes; NO_HANDLES, FAQ arm, registry present; `CategoryFills` is used only here and no other figure's census hash moved.

## Mutants (one line each, module tests)
M2 B→group 14 FAILED (3); M3 Po→post-transition FAILED (1); M4 f starts 58 FAILED (10); M5 valence g−9 FAILED (2); M6 lookup off-by-one FAILED (6); M7 student shows answers FAILED (2); M9 group label 12 FAILED (1); M10 radius trend reversed FAILED (1); M11 advice text FAILED (1); M12 quote cut 10 FAILED (1). **Survivor: M8 cat1 = cat0, all 22 pass** — see major 2.

## Not checked
No image input: I did not look at any PNG; visual claims are pixel probes/ASCII of the raster plus SVG geometry. 66 % is simulated (1.5× resvg → 66 % Lanczos → threshold 150), not a real copier. No release sweep, no full lib suite (per instructions), no browser/app UI. RSC read as HTML (its JS table not rendered). Atomic masses/states/configs are not printed, so not verified.

## Biggest gap vs a textbook table
No relative atomic mass (nor state/electron configuration/oxidation states) in any cell, so the figure cannot serve the mole/mass/isotope/config questions a textbook table is used for; the Print key is a ten-hatch scheme no textbook uses.
