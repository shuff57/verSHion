# Photosynthesis, critic 1 (accuracy, teacher and regression), commit 49809e6

Verdict: FAIL. 0 blocker, 0 major, 6 MED, 5 LOW, 4 minor.
(The critic could not write its report file; condensed from its final answer. PNGs vis/, edge renders edge/, mutant results mutresults.txt, scripts audit.py and lead.py: scratchpad/photo-crit1/.)

Sources: OpenStax Biology 2e 8.1 and 8.3 (Campbell and Raven not reachable).

## MED 1: reading the rate at P cannot be done to the precision the key prints (default and start state)
Start state (graph on), any size, student copy. With Pmax 10 `y_axis` uses step 5, so the only gridlines are 0, 5, 10, 15. P sits at 20 % with a rate of 6.667 and the key prints "6.7" because `dec` defaults to 1. The FAQ says to accept a reading "within half of one gridline", i.e. anything from 4.2 to 9.2, so the question tests nothing. Temperature (8.9) and CO2 have the same problem. In Print the #d0d0d0 gridlines disappear at 66 % (vis/full_L_stu_Small_Print_bin66.png), leaving ticks 5 units apart.
Fix: minor gridlines or a finer step; Print gridlines dark enough to survive 66 %; `dec` default 0 or a tolerance stated in units.

## MED 2: switching the axis to CO2 leaves P at 20 ppm, jammed against the origin
Start state, then Graph against = CO2 concentration. P is 1.7 % along the axis, its horizontal guide is about 14 px long, the key answer is 1.4. 20 ppm is also below the C3 CO2 compensation point (about 40-50 ppm), the least defensible point to ask about. The builder's own PNG list used mark 300 for CO2, so this was never looked at.
Fix: a default mark per axis (e.g. 20 %, 400 ppm, 20 degC).

## MED 3: the temperature key is wrong biology
Temperature, optimum 15 (allowed), key: the caption reads "falls steeply to zero at 21 degC (1.4 times the optimum), when enzymes have lost their shape". No plant enzyme denatures at 21 degC, and a ratio of Celsius temperatures has no physical meaning; optimum 40 gives zero at 56 degC.
Fix: put the zero a fixed number of degrees above the optimum (about 12-15 degC) and reword the caption without the ratio.

## MED 4: the equation blank has a second defensible answer and the student sheet omits the FAQ rule
The student copy says "balanced equation ... one molecule of glucose, with whole-number coefficients". Campbell's form 6CO2 + 12H2O + light -> C6H12O6 + 6O2 + 6H2O meets that wording. Only the teacher FAQ says "smallest" and allows "light over the arrow"; the key lists no accepted variants for this blank though it does for every name blank.
Fix: add "smallest" (or "net") to the student sentence and an accepted-forms line for the equation to the key.

## MED 5: refusal advice for an ambiguous comma names two values that are both refused when applied
Optimum `1,500` (Temperature) gets "Type 1.500 or 1500 to be clear": both are outside 15-40. Same for Light half-saturation `1,000` and CO2 half-saturation `1,500`; for Light P `1,000` the 1000 option is refused (beyond 100). The commit claims "an example that draws" but the module test skips this branch.
Fix: when a reading is out of range say so in the same message and give a "such as" value as the other refusals do.

## MED 6: the tests lock in much less than the commit claims ("26 mutants, all killed")
21 critic mutants run on the product code in 3 parallel clones: 8 killed, 13 survived. Survivors that matter: M03 ADP + Pi and NADP+ clues swapped onto the wrong rows (wrong biology), M06 the student copy prints the rate at P as a number (the leak test only looks at alphabetic words), M04 the rate-at-P disc reuses the equation's number (two blanks share a number), M11 and M12 wrong numbers in the key captions (temperature zero point; half-saturation quoting Pmax), M13 the O2 arrow rerouted across the H2O arrow (the arrow audit never checks arrows against arrows), M18 an empty half-saturation draws at 20 % not the documented 10 %, M05/M15/M21 the student instructions (G3P "makes directly", "balanced", the ADP + Pi / NADP+ clue sentence) can each be deleted. Lesser: M01 rounding epsilon removed, M07 stroma ring within 4.7 px of the inner membrane, M17 Calvin-loop arrowheads moved to the start of each arc. Killed: M02, M08, M09, M10, M14, M16, M19, M20.

## LOW
1. The "Plateau" line is never reached by the curve: K 20 and Pmax 7.5 ends at 6.25 under a "Plateau: 7.5" line while the caption says "on the plateau ... no longer limiting"; at the default the curve ends at 91 % of the line.
2. The FAQ says "degrees C" is accepted; it is refused (`30 degrees C`, `30 deg C`, `45 degrees C` get "is not a number"; the code strips only a trailing "c"; `30 degC`, `30C`, `30 deg` work).
3. The half-up rounding is not exact: `round_half_up` works on a double with a 1e-9 fudge while its comment says "by counting in integers". Light, Pmax 10, K 10, P 3.333333333, dec 0: the key prints 3, the exact value 2.4999999998 rounds to 2 (113 such inputs).
4. Print at Small, 66 %: the outer- and inner-membrane rings both straddle the hatched band between the membranes (vis/z2.png).
5. The letter P sits on a gridline in every temperature student copy (96 of 768 sweep renders, by 0.8-4.6 px); the "Plateau" and "Peak" labels sit on a gridline the same way.

## minor
`let _ = canvas_w_equal(...)` in `the_key_and_the_student_copy_share_one_layout` asserts nothing and `pointer_problems` builds a `cfg` and discards it; at Large the Stroma ring is 21 px above the ATP disc 13; some common wordings not accepted (photons, NADPH + H+, GA3P, glyceraldehyde-3-phosphate hyphenated, Calvin-Benson-Bassham cycle); the equation's rule has no "Equation:" prompt beside it.

## Biggest gap and teacher verdict
The chloroplast half is accurate, clean and usable; the graph half is not ready for a test (MED 1-3). Biggest gap vs a textbook: no thylakoid lumen, no inside of the Calvin cycle (RuBisCO, RuBP, 3-PGA, regeneration), no photosystems or electron transport.

## Verified correct
Biology vs OpenStax 8.1 and 8.3: light and water end on the thylakoids and O2 leaves from them; Calvin cycle in the stroma, CO2 enters the loop and G3P (not glucose) leaves; ATP and NADPH go to the cycle and ADP + Pi and NADP+ come back; double envelope, granum, stroma, thylakoids correct; "dark reactions" correctly refused; equation balances. Graph: K is the half-saturation, 5/6 claim holds at the limits, temperature curve has no jump. Sweep (768 renders, own parser): no text/text, text/ink, text/disc, disc/ink or off-canvas collision; leaders touch only the membranes and their own target; each ring touches only its own structure. Student copies: no answer words anywhere in the SVG source; numbering has no gaps. Edge inputs (NaN, 1e400, empty, whitespace, Arabic digits, minus sign, 10-decimal values): no panic, true refusals. Print grey only; hatches, arrows and loop survive 66 %. Module 31 passed 2 ignored; census 4 passed (fixture gains only photosynthesis__defaults/__start); faq 13, start 19, handle 22 passed; NO_HANDLES lists photosynthesis; `notes_for` routes to `warnings`; no tests or assertions deleted; full figures:: 1805 passed, 1 failed (known absurd_windows_cannot_run_away under load 30-40).

## Not checked
The egui panel, dark theme, real printed output; Campbell's equation form is from the critic's own knowledge; `added_in` does not apply (new figure).
