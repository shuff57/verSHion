# Boltzmann distribution, critic 2, commit 2292be3

Verdict: **FAIL. 0 blocker, 0 major, 2 MED, 5 LOW, 3 minor.** 27/27 boltzmann tests and `census_baseline` pass here. Independent check: Python `math.erfc` + Simpson in speed space, my own rounding, and 25 key/student SVGs rendered from arbitrary params in a throwaway copy, parsed and compared to the formulas, not to `Report`.

## MED
1. **The student copy answers two of the three blanked speeds.** `show_answers=false` still draws, by default, the vp dot + axis tick at every curve and the dash-dot guide at the hottest rms (boltzmann.rs:542-545, 664-682; header 32-33 calls all three speeds answers). Default N2 300/600 K: the blank cells are 422.0/596.8 (vp) and 516.8/730.9 (rms); on the 0-2000 axis with 500 ticks the guide lies 75.7 px (46 %) past the 500 tick and the dot 52 px (32 %) past it, so both read to about 1 %. The readout admits it ("symbols ... kept on the student copy") but `answers_param().hint` does not. Gate the marks on `answers` (the key keeps them) or stop asking for vp/rms on the student sheet.
2. **No opening-state migration for `speeds` (new, default true) or `dec` (default 3 -> 1).** `start::older_values("boltzmann-distribution", 4)` is `[]`, `share::VERSION` stays 4, and `apply_link` (figuresui.rs:709-715) rebuilds from `start_params`, so every older link or saved spec now gains two legend rows + marks and prints 4.6 % / 422.0 where it printed 4.564 % / 421.993. The `decimals()` "missing -> 3" fallback (boltzmann.rs:265-269) never fires on that path. docs/figures-loop.md:112-113 requires `added_in`.

## LOW
3. "Larger only by the invisible tail" (boltzmann.rs:599) is not exact: shaded polygon vs the integral over [va, x_top] is 26.0450 % vs 26.049 % (defaults) and 26.0515 % vs 26.0485 % (H2 300 K, Ea 5), so sampling moves the shade about +/-0.004 pt in either direction; the shade can exceed the printed fraction. Invisible at dec 1, not at dec 3.
4. The 0.1 % crossing is untested. Probe: Ea 40.579 kJ at 600 K -> "1.0 x 10-1 %"; 40.521 -> "0.101 %"; 41.13 -> "9.0 x 10-2 %" (rule holds), but a threshold mutant 0.1 -> 0.05 passes the suite (M7 only tests 0.5).
5. `mark == x_top` is untested: 2000.0 draws, 2000.1 gets the note (verified in my probe); no suite test or audit scenario puts a mark at the axis end, so M14 (half the axis) survives 27/27.
6. The dec default 1 is unpinned by the module: M12 (default 3) survives 27/27 (only the census hash would catch it). At dec=1 the table prints 422.0/730.9 (4 s.f.) while the hint says the speeds are good to 3 s.f.
7. Header, readout and FAQ call the rms guide "dotted"; it is dash-dot `6 3 1 3`, the dotted one is Ea (`1.5 4`).

## minor
8. `describe()` says "with a vertical line at the activation energy" when Ea=0, where no line is drawn (the y axis is the line).
9. `speeds` is missing from the registration test's key list (tests.rs:90); only census and the audit see it.
10. At dec=0 a far-tail percentage prints one s.f. ("9 x 10-2") though the FAQ says "two significant figures"; the code is "at most two".

## verified (independent)
- 27/27 module tests; `figures::census::census_baseline` ok at 2292be3; registration chemistry.rs:429, faq.rs:61 (5 Q&A), NO_HANDLES interact.rs:326.
- 25 probe cases (defaults; dec 0-3; 1/2/3 curves; equal/duplicate T; M=1/1000; T=50/3000; Ea 0/45/431.5/500; mark 0/2000/2000.1; Print; speeds off; H2/CO2/custom): every curve vertex on my f(v) to <5e-8 s/m (0.02 px), peak exactly at vp and 4/(sqrt(pi e) vp), area 0-12 vp = 1.000000000, per-curve T and M; table, marked f and fraction strings agree with my rounding at the chosen dec; vp dots, vp axis ticks and rms guide at my px(vp/rms) to <0.01 px; Ea line at sqrt(2Ea/M); shade vs integral <2e-4.
- F: printed 4.564/26.049 %, 0.111/4.564/14.818 %, 84.903 %, Ea=0 100.000 % (shade 99.9973 %), far tail 2.6e-31 % (45 kJ/70 K) and 6.2e-30 % (431.5/700) at 2 s.f., 431.5/70 -> "less than 0.001 %"; all against my speed integral and libm erfc.
- Student: 0 answer texts, no tail, 6-11 blanks, marks at the same pixel as the key. Print: 3 distinct dashes + 3 hatches, no non-grey hex. Refusals name box, reason and fixed value; the rest draws.

## mutants (14 run, 12 killed, 2 survived)
Killed: M1 m_g/100 (8 tests), M2 legend T+1, M3 hottest fraction for all, M4 vp dot at mean, M5 rms at mean, M6 switch ignored, M7 pct threshold 0.5, M8 no 2-sf cap, M9 tail from 0, M10 mark on coldest, M11 one vp, M13 EA_SPAN 3 vp. Survived 27/27: M12 dec default 3 (census would catch), M14 mark range halved. M16 (drop the vp/rms legend on the student copy) also survives by inspection: tests.rs:585 reads the key's legend only.

## not checked
No image input: I did not look at any PNG (Colour, Print, student, 66 % bitonal), so dot/hatch/dash legibility and the visual collision result are the resvg ink-box audit's claim, not mine. No full-lib suite, no UI, no browser/PDF export. The collision audit has no `speeds=false` scenario.

## biggest gap
The student copy's default geometry answers vp and rms; the marks are defensible only if the student table stops asking for them. (Still no energy axis: this teaches the speed tail, not OpenStax 12.5's E/RT view.)
