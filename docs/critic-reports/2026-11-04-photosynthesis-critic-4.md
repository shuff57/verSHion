# Photosynthesis, critic 4, commit 8b01f3d (branch figures-gapfill)

Verdict: **PASS**. 0 blocker, 0 major, 0 MED, 3 LOW, 5 minor. Probes: `scratchpad/band_sweep.py`, `scratchpad/probe/` (talks to the real `read_graph`/`accept`).

## BLOCKER / MAJOR / MED
None. The round-3 MAJOR is fixed: `census_baseline` passes at 8b01f3d (3.5 s). Module tests: 53 passed, 0 failed, 1 ignored (1120 s, debug).

## LOW
1. **"Decimal places for P" = 0 is silently exceeded by one decimal, and the control has no hint.** Light, plateau 5, K 10, P 20, dec 0 → key prints "Rate at P (x = 20 %): **3.3** arbitrary units" beside "accept any value from 3.2 to 3.4" (the 0 dp rounding, 4, is outside the band). Frequency over 160k typed states: 26 % at pmax 3-10, 72-93 % at pmax 1-3, 0 % above 10; the shipped default is dec 1, so only a teacher who chose 0 is affected. The raise is bounded at exactly one decimal (fine spacing is 0.05-2, so a 0.1 step always lands in a one-spacing band), the printed number is inside the band, and the P arm of the FAQ says "or to more" — but the control's label is where a teacher looks. `.hint("Shows one more decimal when rounding to this many would print a rate outside the range the key accepts")` is one line and, I think, required.
2. **The student sheet's grid is now 2.2x darker in ink (not the key's).** Fine #7a7a7a is 4.29:1 on white (was #a4a4a4, 1.47:1), 1.4 px, 18 lines plus 6 labelled; the curve is #111 at 2.8 px. The Colour key is *lighter* than Print (5.50/4.25 vs 6.90/4.29), so "key too dark next to Print" is answered no; the risk is that the student's 24 ruled lines read as ruled paper (critic 3's minor, now worse) and that the white pads (P letter, "Plateau: 10") punch brighter notches. I cannot see it; the visual pass must. If it objects, 1.2 px or #808080 is the lever, not a return to #a4a4a4 (that fails the 66 % test).
3. **Doubled units name a unit the box accepts.** `5 %%` → `ends with "%", which is not one of the units this box takes (% or percent)`; same shape for `500 ppm ppm` ("ppm" not one of "ppm or parts per million") and `30 cc` ("c" not one of the C spellings). The advice is right and types back, but the sentence is false — it is the *second* unit that is wrong. Say "two units" instead.

## minor
- `1 000` and `1 000` (NBSP) still say `is not a number`; the comma fix did not reach the space thousands separator (critic 3 LOW 4's second half).
- A trailing BOM quotes invisibly: `"5" is not a number` reads as a false message.
- The band-widening fallback (`lo.min(p)`, `hi.max(p)`) is unreachable in 160k states and has no test (mutant 12 survived); "widens only if that still fails" is unexercised.
- `docs/figures-progress.md` still gives the round-2 rule ("half of the fine spacing either side") and has no round-3 entry — stale on exactly the rule that changed.
- The key caption "(0.5 apart)" describes the spacing while the printed band is 1.0 wide when the rate is on a gridline; "one spacing apart" would read better.

## verified (independent method)
- Claims 1-2: I rewrote nice_step, minor_divisions, rate_frac, band, ratio_text and accept in Python from the prose (no module import) and swept 160,000 typed states with 9-decimal values on all three axes × dec 0-3 (plus 60,000 earlier): printed rate inside the band, band exactly the gridlines either side (two spacings when on one), every neighbour inside, hi ≤ axis top, never wider. Named cases through the real code: 9.5-10.5, 70-80, 4.5-5.5. The sweep also proves the dec raise never changes the band.
- Claim 3: strokes actually drawn are #5f6a78 (1.6 px) + #717c89 (1.4 px) in the Colour key, #5a5a5a/#7a7a7a in Print and in every student copy; contrast on white 5.50/4.25 vs Print 6.90/4.29. My own 1x raster, thresholded at 168 with label-pad columns excluded: all 12 states (opening included) plus pmax 1/100 and Large/Small keep 99.1-100 % of every gridline's columns.
- Claim 4: the key renders "… any multiple of the answer, such as 12CO2 + 12H2O gives 2C6H12O6 + 12O2, because one molecule of glucose is asked for."; `1,500 ppm` on Light gives both faults and "such as 20" (types back), on CO2 "Only 1.5 … Type 1.5".
- Round 2: the water-on-both-sides form is balanced (6 C, 24 H, 24 O each side) with gcd 1, so it can only be refused as "water on both sides", and the key/FAQ say that; the student prompt says "for making one molecule of glucose, with no substance on both sides". Biology (locations, O2 from water, ATP/NADPH to the Calvin cycle and ADP+Pi/NADP+ back, G3P as the direct product, net equation 6/12/18) matches OpenStax 8.1 as the header summarises.
- Hostile: 87 inputs × 3 boxes × 3 axes = 783 cases, 654 refusals; every "such as X" / "Type X to be clear" / "Only X" value typed back and accepted — 0 advice loops, 0 messages with no applicable fix.
- Print is grey-only and the student copy leaks no rate (source and 10 render states checked).

## mutants (12 run, 11 killed, 1 survivor)
Killed: on_line forced false; dec-raise loop removed (pinned by `the_printed_accepted_range…`'s dec-0 block — note `the_rate_at_p_is_rounded_half_up…` alone passed this mutant); FINE_COLOUR→#9aa4b2; FINE_SW 1.4→0.7; minor_divisions 2→5; comma ambiguity 3→4 digits; both-faults unit dropped; ratio_text truncating instead of half-up; student Colour keeps Colour greys; FALL_SPAN 12→11; `key_rate` reverted to `rate_text` (the round-3 bug). Survived: removing the band-widening fallback (unreachable; no test).

## not checked
No image input: I did not look at any PNG, so collisions beyond the module's resvg measurement, the P/label white pads, the ruled-paper judgement, dark theme, a real 150/300 dpi print and browser fonts are unjudged — the visual pass is separate. Also not run: the rest of the lib suite, release timing, the live egui editor.

## biggest gap vs a textbook figure
Still "where", not "how": no lumen, photosystems, electron transport or ATP synthase, nothing inside the Calvin loop (no RuBP, fixation/reduction/regeneration), and no light or CO2 compensation point — so the rate is gross photosynthesis and the figure never says so (only the CO2 compensation point is mentioned, in the FAQ).
