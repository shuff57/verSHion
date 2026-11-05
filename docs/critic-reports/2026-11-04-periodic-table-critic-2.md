# periodic-table — blind critic 2 (a5afd6e)

Verdict: **FAIL — 0 blocker, 0 major, 2 med, 5 low, 4 minor.** 28/28 module tests pass; `census_baseline` passes and HEAD blessed only periodic-table's two hashes; nothing deleted. Tree clean except untracked `scratchpad/`.

## BLOCKER
None.

## MAJOR
None. The three round-1 majors do reproduce fixed: f-block advice (La/Ac/Ce/Pd), Print letters/swatches/ten hatches, and the US-spelling + OpenStax/RSC story.

## MED
1. **The new mass line is unreadable at the default size.** `fs_mass = 0.115*cell` = 5.52 px at Medium = 4.1 pt at 96 dpi (3.6 px on a 66 % copy). `read()` warns only for Small, and `examples()` ships "Full table with atomic masses" at Medium; in the 1.5x resvg raster "1.008" and "[98]" merge into blobs. Repro: `build(&with(&[s("block",BLOCKS[2]), b(SHOW_MASS,true)]))`; only Large (7.73 px = 5.8 pt) is marginal. Raise `fs_mass` to >=0.16 cell, or treat Medium like Small in the hint/note.
2. **The student copy prints the highlighted element's name when names are on.** `labels="Add names at Large"`, `label_size="Large"`, `highlight="Au"`, `SHOW_ANSWERS=false` draws `data-o="name" data-z="79">Gold` in Au's cell while the panel asks `Name ______`. `the_student_copy_blanks` builds only the default labels, so it passes; the header's "printed only under Show answers" is false in this mode. Repro: `scratchpad/critic2/png/periodic-probe-student-names.svg` (`data-o="answer"` count 0, `Gold` present). Fix: omit names on the student copy (the convention already says the student copy omits labels) or blank that one cell's name.

## LOW
- **`<desc>` overstates the drawing.** Small + `show_mass` says "with relative atomic masses" but 0 masses are drawn; Medium + `labels=2` says "with element names at the large label size" but 0 names (`periodic-table__labels_2.svg`). A screen reader is told about data not on the sheet.
- **The cell code is 5.76 px (4.3 pt) at Small** (`0.15*cell`); the "every cell has a code" assertion only builds Print at Medium, so dropping the code at Small survives (mutant C2). The key code is 8.45 px at Small.
- **Codes P and N are also element symbols** (phosphorus, nitrogen); the distinctness test compares only the ten codes with each other. A corner "P" in Al/Ga/… can be read as phosphorus.
- **Symbols only + masses** leaves the mass alone in the top-left, where Z normally sits: Tc shows "[98]" and no "43", which reads as Z=98. The audit only checks fit. Omit masses in Symbols-only or put the mass under the symbol.
- **Hs [269]** is the conventional bracketed value but not the longest-lived isotope under current data (271Hs ~46 s; 277mHs ~110 s), while the header/foot say "longest-lived". The value matches the IUPAC poster.

## minor
- `nlines = key_wrapped.len().max(student_wrapped.len())` is never exercised: both variants always wrap to the same count in every tested case, so mutant C5 (student length only) survives and the "both copies same size" guarantee is untested for a divergent case.
- `describe()` never says a highlight was refused or that the element is hidden; only the visible note does.
- The metalloid/spelling and "Not in this view" key notes exist only when the key is on; key=off exports no metalloid/spelling caveat (the trend caveat is independent of the key).
- The key note (the only part that exports) cites OpenStax for the six but not the RSC polonium page; only the header/FAQ do.

## Verified
- Registration/FAQ/NO_HANDLES/census: 28 tests; `census_baseline` green; HEAD diff is the two periodic hashes only; `CategoryFills` used only here.
- Advice: La, Ac, Ce, Pd and visible H/Pd/La/Og; f names only Full, d only With transition; applying the advice draws the cell and clears the note; printed in the SVG.
- Codes/swatches: 10 distinct codes; every cell's code matches `category(z)`; every key code matches its swatch; swatches 24.96x24 (S), 31.2x26.4 (M), 43.68x36.96 (L); 10 byte-distinct pattern bodies; metalloid four-dot; Ln zigzag; >=8 % ink at 66 % on the synthetic sheet.
- Spelling/citation: data Aluminum/Cesium/Sulfur; header, FAQ, key note and NAMES all US plus "IUPAC/RSC print aluminium and caesium"; OpenStax 2e §18.3 does name exactly B,Si,Ge,As,Sb,Te and §2.5 does state the bracket rule (checked online); the RSC Po page says "classed as a metal"; "RSC's metalloid key" is gone.
- Trend caveat: printed (SVG, key and student) only when trends != 0; absent when off; d/f/He/Ne/Ar named.
- Masses: all 118 compared to the independent `MASSES`; >=25 recomputed (Tc [98], Pm [145], Po-Ac [209..227], Np-Og, Th 232.04, Pa 231.04, U 238.03); bracket set 43,61,84-89,93+ correct; omitted at Small with the in-figure note.
- Data/layout: 118 symbols/names/Z; categories 6/6/38/11/7/7/6/7/15/15; group/period against an 18-column grid; f strips 15 wide under group 3; H/He; valence all 118; 440+ collision renders; Print grey-only; caption; highlight symbol/Z/lowercase/unknown/f-block.

## Mutants (16 run)
Killed: cell code = next category; key code off-by-one; swatch floor removed; mass at Small; Th loses standard weight; mass 2 dp; caveat dropped; student answers; numeric asks Z; Po not metalloid; f advice d-block; student desc "reads out"; no not-in-view note.
**Survived: C2 cell code dropped at Small (test never builds Print Small); C5 panel reserves student length only (variants never differ); C10 caveat computed when trends off (dead, semantically equivalent).**

## Not checked
No image input: I never viewed a PNG; visual claims are SVG geometry plus greyscale pixel/ASCII probes of resvg output (1.5x, and a Lanczos 66 % for cell crops). 66 % is simulated, not a real copier. No full lib suite (per instructions), no browser/app UI, no share-link load. OpenStax/RSC read as search extracts, not the rendered RSC JS table. Masses checked against memory plus the independent list, not a live CIAAW page for all 118; font metrics are the module's own resvg test.

## Biggest gap
No physical state (s/l/g at STP), electron configuration or oxidation states, and the one data line it now carries (mass) is 4 pt at the default Medium — so it still cannot serve the reference lookups a textbook table is used for.
