# periodic-table — blind critic 5 (db9dc11)

Verdict: **FAIL — 0 blocker, 0 major, 2 med, 6 low, 4 minor.** 38/38 module tests pass; `census_baseline` and `handle_coverage_report` pass; default/start hash `f118cda5d266d974` matches the blessed fixture; 16 mutants inspected (no recompiles): 9 killed, 6 predicted survivors, 1 wrong-kill.

## BLOCKER
None.

## MAJOR
None.

## MED
1. **The cell number and symbol still merge at 66 %.** At 66 %, the Colour crops (no hatch) give Small `Z38` “38” = 1 component and `Z20` “Ca” = 1; Medium `Z12` “12” = 1 and `Z71` “Lu” = 1; Large `Z38` “38” = 1. The test measures only Al and never counts components; its Print crops are the `#666` hatch, not the glyph (Small symbol (43,13) vs Colour (22,6); Medium (46,16) vs (33,7)), so the Print gate passes even with the symbol gone. Repro: `block=Full, label_size=Small/Medium/Large, style=Colour` and the 66 % crops from `periodic_pngs` (`/tmp/figures-crit/periodic5/scratchpad/out`). Fix: make Small/Medium 100 %-only with a note, or enlarge until two digits/letters separate; gate on components and a hatch-free crop.
2. **The student panel prints the f-block group answer.** `src/figures/chemistry/periodic.rs:504-505` gives the student copy `Name ______; Z ______; group: none (f-block); period ______; category ______.` for every f-block element; the key answer is `group - (f-block)`. The header says the read-out is printed only under Show answers and the student copy has blank lines. `the_student_copy_never_names...` builds La but checks only the name; `the_student_copy_blanks...` uses Au. Repro: `block=Full with f-block, labels=Add names at Large, label_size=Large, highlight=La, show_answers=false`. Fix: blank the group and put the “strips have no group number” fact in the foot; add La/Ac to the blanking test.

## LOW
- **Argon is 39.948, the 1979 value; CIAAW 2021 abridged is 39.95** (the full 2021 value is the interval [39.792,39.963]). 117/118 match; all brackets match. The independent `MASSES` list also has 39.948, so changing the data to the CIAAW value fails `the_optional_masses...`.
- **The 66 % notes are still not text at Small/Medium.** Actual component ratios: Small foot 0.18 (10 comps/57 chars), key-note 0.10-0.25, ink 3-4 px; Medium 0.64-0.86, ink 5-6; Large 1.04-1.36, ink 8-9. The test’s 87/108/136 counts and “>= 2 px” gate are presence. The metalloid, spelling and valence caveats vanish at Small 66 %.
- **The Small code note contradicts the key.** The note says “Category codes are not drawn at the Small label size” while the key still draws 10 `key-code`s at 8.62 px; `the_cell_number...` and `the_category_codes...` assert both. Reword “Cell category codes…” or drop the note.
- **The leak sweep is 708 cases, not every note state.** It is 118 × 3 blocks × key/student at Large+names, key on, trends off, masses off, valid symbols. `trend-note` is never produced and refusals are never scanned. `highlight=auu, show_answers=false` prints `Did you mean Au (Gold)?` on the student copy.
- **The Small key and Large names are 4-5 px at 66 %.** Key labels 6.84 px Small (ink 4-5), 8.55 Medium; key code 8.62 Small is one blob; names 5.68 px Large (ink 4). The key code is larger than the cell code, but the Small Print key is not readable.
- **The cell code at Medium 66 % is one blob.** “am”/“ae”/“nm”/“an” are one component at 8.24 px; the key code is 10.77 and readable, so this is secondary, but the header says the cell code distinguishes the category.

## minor
- The key note says “two of these names” but no longer says which; the header still says the figure says so under the key.
- `suggest("gld")` returns Gd, not Au.
- An f-block highlight typed as a number still asks `Z ______` (the typed number is given); the main/d-block branch omits it.
- The f-block placeholder “f-block” is 3.55 px at Small 66 % (“57-71” 4.05); secondary.

## Verified
38 tests, including the 452-render collision audit, 10 000 hostile inputs, Print grey-only, WCAG, ten byte-distinct hatches and codes; `census_baseline`; `handle_coverage_report`; NO_HANDLES; 118 symbols/names/Z; 50/88/118 cells; categories 6/6/38/11/7/7/6/7/15/15; 15-wide f strips; all valence values; 117/118 masses incl. all brackets; caption fits (Small Main right edge 204/233); key off; exact halo widths; outline; desc; student name and British alias absent in valid states (gold-student whole-source scan); highlight in all blocks; not-in-view; bracket rounding.

## Mutants
16 inspected, not recompiled: M1 key note names killed; M2 Small code killed; M3 Small number 0.26 killed; M4 Small symbol 0.30 **survives**; M5 key code 0.20 killed; M6 key label 0.21 killed; M7 note fs 0.19 **survives**; M8 note soft **survives**; M9 halo 0 killed; M10 masses Medium killed; M11 f-block advice killed; M12 student name killed; M13 Ar 39.95 **killed by the wrong test**; M14 f-block group blank **survives**; M15 trend caveat full names **survives**; M16 refusal name removed **survives**.

## Not checked
No image input; I did not view a PNG. Measurements are SVG tags, the actual 1.5x→0.66 Lanczos crops, and PIL at the same 66 % font sizes; the Print crops include the hatch, so the Colour crops are the glyph evidence. No full lib suite (only the 38 periodic tests, `census_baseline`, `handle_coverage_report`); no app/browser/share-link load; no release census; no mutation recompiles (the frozen tree forbids source edits); 66 % is simulated; the f-block student render was not dumped (the MED is the exact source string); the CIAAW argon page was fetched, the other masses are from the fetched abridged table.

## Biggest gap
Still an excerpt: no electron configurations, oxidation states, atomic radii/electronegativity values, isotopes or physical states; masses only at Large; and the 66 % cell-number/symbol merge plus the f-block group answer are the remaining ship blockers.
