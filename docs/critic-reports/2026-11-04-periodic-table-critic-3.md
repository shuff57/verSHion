# periodic-table — blind critic 3 (5c6b899)

Verdict: **FAIL — 0 blocker, 0 major, 2 med, 4 low, 4 minor.** 32/32 module tests pass in the frozen tree (34/34 with two probe tests in a detached worktree); `census_baseline` and `handle_coverage_report` pass; the blessed census commit changed only periodic-table's two hashes; nothing deleted.

## BLOCKER
None.

## MAJOR
None.

## MED
1. **The student copy still names the highlighted element for He, Al, Cs and Po.** The highlighted cell's `name` is correctly omitted, but fixed notes spell it: He → the foot "…helium has 2 in group 18."; Al/Cs → the spelling key/foot note "(aluminum, cesium)"; Po → the metalloid note "…polonium…". The panel still asks `Name ______`. Repro: `build(block=Main group, highlight=He, answers=false)` (or Al/Cs/Po): no `data-o="name"` for Z, but the text is in `data-o="foot"`/`key-note`; a case-insensitive whole-SVG scan of `data::name(z)` finds it. The round-3 test loops only Au/La/H/79/Og/Xx/"" and the other scan checks "Gold", so neither covers these four. Fix: suppress or rephrase those notes when the highlighted element is the one named (use Z/symbol or "the group-18 exception"), or keep the citation off the student sheet.
2. **The cell category code and the atomic number are not legible at 66 % on the default and Small Print copies.** `fs_code = 0.19*cell` = 9.12 px (Medium), 7.30 (Small); after a real 1.5x→0.66 Lanczos the Al "pm" crop is 6×3 px with 8 dark pixels at Medium and 2 at Small; the Small "13" is 5×3 px with 4 dark pixels. The bold symbol survives (12×8) and the key code is readable, so the cell code/number are the failure. The tests assert only `fs >= 7` at 1x and ten 66 % hatch signatures; they never rasterise cell text. Repro: `periodic-full-print.svg`, `periodic-full-mass-print-small.svg` from `periodic_pngs`, crop the Al cell, resample to 0.66 (PIL). Fix: raise `fs_code`/`fs_num` or thin the 2.5–3 px white halo at these sizes.

## LOW
- The mass-definition foot line ("Atomic masses are the IUPAC 2021…") is printed even when the masses are refused (Medium/Small, Symbols only), below a note saying they are not drawn; the sheet defines data it omits. Repro: block full, `show_mass=true`, Medium. Mutant M17 (a conditional fix) survives.
- The highlight-outline position is untested: the test checks only `data-o="highlight" data-z="46"`, while the rect is drawn from `c.x0/c.y0`; a mutant that draws the outline on Z+1 (M14) survives. Current output is correct.
- The lowercase-code safeguard is nominal: `am`, `pm`, `tm` still read as Am, Pm, Tm if case is ignored, and at 66 % the case is invisible; nothing ties the key-note example "pm" to `CAT_CODES`, so changing it to a non-code (M19) survives.
- Print halos/contrast are untested: removing the atomic number's white halo (M24) survives; no test checks text over the hatch, so the 66 % faintness in MED 2 has no guard.

## minor
- The maximal `<desc>` is 367 characters (full + names + masses + trend + highlight) but the test's 320-character cap is checked only on four smaller cases.
- `data.rs`'s table comment still says "mass number of the longest-lived isotope"; Hs [269] is not (277mHs is longer), though the figure's wording is fixed. The figure's bracket set is the IUPAC one.
- The student panel asks `group ______` for an f-block element where the key answers "group - (f-block)"; the header says the f-block has no group.
- `block_shows(1, z)` returns true for z outside 1..=118 (`!is_f(z)`); no current caller passes one. The name-fit guard and bracket `.round()` are dead (M16/M18 survive).

## Verified
- 50/88/118 cells; 118 symbols/names/Z; categories 6/6/38/11/7/7/6/7/15/15; f strips 15 wide under group 3; all 118 valence values; H/He; every cell code matches `category(z)` and every swatch; 10 byte-distinct patterns; Print grey-only; WCAG AA on all ten colours; swatches ≥24×24.
- Masses: all 118 match the independent `MASSES`; recomputed ≥20 against IUPAC/CIAAW incl. He 4.003, C 12.011, Fe 55.845, Rh 102.91, Te 127.6, I 126.9, Cs 132.91, La 138.91, Eu 151.96, W 183.84, Au 196.97, Hg 200.59, Pb 207.2, Th 232.04, Pa 231.04, U 238.03 and brackets Tc [98], Pm [145], Po [209], Ac [227], Np [237], Hs [269], Og [294]; standard/bracket set 43,61,84-89,93+ correct.
- Mass placement: 0 masses at Medium/Small and Symbols only; the SVG note "Atomic masses are drawn only at the Large label size; choose Large to show them." and the Symbols-only note; the shipped example is Large; 1x ink ≥7 px, 66 % ≥4.6; the mass crop reads at 66 %.
- Names: every block×labels×size×highlight case omits exactly the highlighted cell's `name` and keeps the rest; panel asks Name; no `data-o="answer"` on the student. The note leak is MED 1.
- Desc/trend/advice/print: desc names/masses/kinds match what is drawn; refusal and hidden-element sentences; trend caveat in the SVG, key and student; f advice La/Ac/Ce/Pd; 10 patterns and codes; code ≥7 px at 1x; panel reserves the longer variant (Small+Al key wraps more); 452-render audit clean; 10k hostile inputs pure and <120 s.

## Mutants (25 valid, 19 killed, 6 survived)
Killed: mass at Medium/Small; mass with Symbols only; both mass notes; student cell name always/never; panel student-only; code 0.13 cell; code "Ml"; desc mass claim; mass +1; Th bracket; blank trend caveat; f advice d-block; typed-number Z blank; mass 0.12 cell; example Medium; desc names; Symbols-only note; label moved onto strip.
Survived: M14 outline on Z+1 (position untested); M16 name-fit guard removed (dead at Large); M17 mass rule only when drawn (real low); M18 `.round()`→`as i64` (values integral); M19 key-note "pm"→"zz" (untied); M24 number halo removed (untested).

## Not checked
No image input: I never viewed a PNG; visual claims are SVG geometry plus resvg/PIL pixel probes of 1.5x renders and Lanczos 0.66 crops. No full lib suite (per instructions), no app/browser/share-link load. Masses checked against memory plus the independent list, not a live CIAAW page for all 118; OpenStax/RSC read as prior extracts, not re-fetched. 66 % is simulated, not a real copier. Mutation testing was in a detached worktree at 5c6b899 (source restored); the frozen tree is untouched except the report and scratchpad.

## Biggest gap
Still an excerpt with only Z/symbol/name/mass (mass only at Large): no physical state, electron configuration or oxidation states, so it cannot serve the textbook reference lookups; and the Print cell code/number do not survive 66 % at Medium/Small, so the one print-specific disambiguation is not dependable.
