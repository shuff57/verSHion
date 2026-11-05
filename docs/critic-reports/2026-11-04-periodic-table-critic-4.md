# periodic-table — blind critic 4 (6c138e0)

Verdict: **FAIL — 0 blocker, 0 major, 2 med, 4 low, 3 minor.** 37/37 module tests pass; `census_baseline` and `handle_coverage_report` pass; 17 mutants: 14 killed, 2 survived, 1 invalid; plus one test-enforced leak.

## BLOCKER
None.

## MAJOR
None.

## MED
1. **The student key note still names Al and Cs.** The note reads "IUPAC and the RSC print the British spellings (aluminium, caesium)." It is drawn on the student copy (`data-o="key-note"`, or `data-o="foot"` with the key off) and the panel still asks `Name ______`. The round-4 sweep scans `data::name(z)` — the US names "aluminum"/"cesium" — so "aluminium"/"caesium" pass; `the_names_follow...` line 467 actually **requires** them. I applied the US variant and the sweep fails ("Main group/Al: a student note names it"); I removed the British names and that test fails, so the test enforces the leak. Repro: `build(block=Main group, labels=Add names at Large, label_size=Large, highlight=Al, answers=false)`; grep "aluminium" (and highlight=Cs for "caesium"). Fix: suppress/reword the note for Al/Cs (use the symbols) or keep it off the student sheet, and scan the aliases the figure prints.
2. **Small+Print at 66 % is still not reliably readable.** Cell number "13" is 5x4 px (14 px <100, 4 <70), code "pm" 7x3 (11/3), symbol 6x4; the key legend code is 3 px <100 and the key label 5. The test gate is 7 px <100 and 2.5 px high — presence, not legibility. The default and Medium/Large are fixed (Medium Colour 29/39 px <100; Small at 100 % 42/71), so this is the 66 %-of-Small corner. Repro: `build(block=Full, label_size=Small, style=Print)`, Al crop from a 1.5x resvg render Lanczos-resampled to 0.66 (`/tmp/figures-crit/probe-out/Small-Print-plain.png`). Fix: raise the Small code/number/key, or note Small as a 100 % print mode.

## LOW
- **The soft-grey notes fade out at 66 %.** Medium `foot` min 160 (0 px <160), `key-note` 147 (2), `panel` prompt 136 (19); Large `foot` 112 (0 <100). The metalloid/spelling caveats and valence rule are the figure's honesty; the mass rule refuses Medium at "about four points" while these print at 3.6-4.5 pt at 66 %.
- **The halo width is untested.** M7 (`2.0`→`0.0`, tag still present) survives `print_halos...`; removing the halo entirely is killed, but contrast is not guarded.
- **The key legend is smaller than the cell code it explains.** key-code 0.22×cell, key label 0.21, cell code 0.26; at Small Print 66 % the legend is the weaker copy.
- **The lowercase-code note overstates.** `am`/`pm`/`tm` are Am/Pm/Tm case-insensitively; the test only rejects exact-case symbols.

## minor
- The student panel asks Z/group/period/category, but the cell prints Z and the code, and position gives group/period; only the name is truly withheld. The header's "printed only under Show answers" is false for Z/category.
- `suggest("gld")` returns Gd (gadolinium), not Au; a name misspelling gets a misleading nearest symbol.
- `data.rs`'s shared comment still says "mass number of the longest-lived isotope"; Hs [269] is not the longest-lived (277mHs); the figure's wording is fixed.

## Verified
- 118 symbols/names/Z; 50/88/118 cells; categories 6/6/38/11/7/7/6/7/15/15; f strips 15 wide under group 3; all 118 valence values; H/He; every cell code/swatch; 10 byte-distinct hatches; Print grey-only; WCAG AA; swatches ≥24; 452-render collision audit; 10k hostile; purity/CPU; FAQ 6 arms; registration; NO_HANDLES; blessed census changed only periodic's two hashes; nothing deleted.
- Masses: all 118 match the independent list; 40+ checked against IUPAC/CIAAW incl. He 4.003, Fe 55.845, Ag 107.87, Te 127.6, Cs 132.91, Au 196.97, Pb 207.2, Th 232.04, U 238.03; brackets Tc [98], Pm [145], Po [209], At [210], Rn [222], Fr [223], Ra [226], Ac [227], Np [237], Hs [269], Og [294]; standard/bracket set 43,61,84-89,93+ correct; desc max 374 ≤420.
- Leak scan (US names): 12,744 block×size×labels×mass×key×highlight cases × all 118, whole-SVG lowercase word scan of foot/key-note/note/panel/blank/answer/trend-note/desc: 0; key panel names all 118×3; 118/118 outlines on the cell; round-4 foot-He/Po/lowercase/mass-rule/bracket/block-bounds/f-prompt/guard/halo-tag/outline/desc fixes verified.
- The exception is the British aliases above.

## Mutants
17 run: killed M1 foot He; M2 polonium; M4 untied key code; M5 code 0.19; M6 number 0.20; M8 halo removed; M9 outline shifted; M10 bracket truncation; M11 mass rule on show_mass; M12 masses at Medium; M13 off-table block; M14 names at Medium; M15 typed Z blank; M16 not-in-view dropped. Survived: M7 halo width 0; M17 desc "masses left out" removal. Invalid: M3 (changed the header, not the source note). Separate M18 (remove the British names) is killed by `the_names_follow...` — the test enforces the MED.

## Not checked
No image input; I never viewed a PNG. Visual claims are SVG geometry plus resvg/PIL pixel probes (1x direct and 1.5x→0.66 Lanczos at 63 dpi). No full lib suite (per instructions), no app/browser/share-link load. Masses checked against memory plus the independent list, not a live CIAAW page for all 118; OpenStax/RSC read as prior extracts. 66 % is a simulated copier, not a real one. Mutation testing was in a detached worktree at 6c138e0; the frozen tree is untouched except this report and scratchpad.

## Biggest gap
Still an excerpt: no physical state, electron configuration, oxidation states, atomic radius/electronegativity values or isotopes, and masses only at Large. The shipping limits are the Al/Cs note leak and the 4-5 pt Small+Print 66 % copy.
