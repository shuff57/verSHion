# Boltzmann distribution, critic 3, commit 7de31d2

Verdict: **PASS. 0 blocker, 0 major, 0 MED, 6 LOW, 4 minor.** 30/30 boltzmann tests pass; `census_baseline`, `print_style_is_grey_only` and `handle_coverage_report` pass here. I re-derived f, vp/mean/rms, F(Ea), the tail and every printed string in Python, and rendered 38 key/student/Small/Large/Print/extreme states with a throwaway probe linked to this commit, parsing the SVG (not `Report`).

## BLOCKER / MAJOR / MED
None. **MED 1 is fixed**: every student state (default, Small, Large, Print, Print+Small, `speeds` on) has no `data-o="tail"`, no vp dot/`vpaxis` tick, no rms guide, no legend entry naming vp/rms, 0 `data-o="answer"` and 6 blanks. The key/student default frames are identical (t=37). The typed mark dot/guide stays and its cy is on the drawn hottest curve to 0.004 px (a read-off, not a leaked number).

**MED 2 is acceptable, but the note overclaims.** A v4 link made from round 1/2 of this branch does reopen differently: `encode` omits `dec` at the old default 3, so it now opens at 1 (422.0 vs 421.993). Saved specs are not affected (`spec.rs:30` stores every param, so an old spec keeps dec=3). No release, saved spec or link fixture for this figure exists, so only dev-made links are hit; the test pins the decision. Header 40-43 should say "no *released*" link, not "no saved specification or link".

## LOW
1. **Print at 66 % is tested for hue, not darkness.** The Print grid is #cccccc (204/255) and a one-curve tail is `url(#vs-hatch0)`, a plain #d9d9d9 wash with no foreground (`svg.rs:310`); at a 66 % bitonal threshold (~168) both go white, so a one-curve key loses its whole shaded tail. `print_is_grey_only_and_holds_up_at_66_percent` (tests.rs:793) checks hue and font/stroke sizes only; `hatches.len()==3` counts the plain wash as a hatch. Repro: `key-print-1curve-ea0.svg`.
2. **Small Print at 66 %:** 10.4 px -> 6.86 px, below that same test's 8.4 px floor (only Medium is checked). Repro: `student-print-small.svg`.
3. **Key and student are not the same drawing when a mark is set.** The key's vp/rms legend entries add a row (t=58, canvas 706) where the student's legend fits one (t=37, canvas 631); the student plot is 21 px taller and every curve is stretched ~7 %. Repro: `key-mark.svg` vs `student-mark.svg`.
4. **vp is still readable off the drawn peak** (the curve maximum is vp), to a few percent. The exact marks are gone, but "the blank speed cells cannot be read off the axis" (header 13, hint 401, FAQ 3900) is too strong for vp; mean/rms are not visible.
5. **The erfc error text is about the wrong function.** Header 29-31 and FAQ 3885 say the far-tail relative error reaches "about 1.5e-3"; for erfc it is 2.8e-3 at x=4, 6.7e-3 at x=5, 4.4e-2 at x=10, 3.3e-2 at x=77. F's error is <=2.2e-4 (the (2/sqrt(pi))sqrt(x)e^-x term dominates), so the 2-s.f. rule is sound; say "of F".
6. **`MARK_HI` 20000 is below x_top=40000** for M=1, T=3000, Ea=500 (probe); the mark control cannot reach that axis end, and the end/past test covers only x_top=2000.

## minor
- `decimals()` still falls back to 3 (boltzmann.rs:275); a missing `dec` renders 421.993. M12 (3->1) survives 30/30.
- The Ea line is drawn `1.5 4` (dotted) but the hint and FAQ still call it "the dashed line"; the rms wording was fixed.
- `odd_inputs` does not fuzz `speeds`; a -inf temperature uses the default 300 K while -5 uses the 50 K floor.
- The mark FAQ "whose density is the blank value" reads as a leak; safer "marks where to read the curve".

## verified (independent)
- **Physics:** default table 422.0/476.2/516.8 and 596.8/673.4/730.9; peak = 4/(sqrt(pi e) vp); area 0-12 vp = 1. F against my speed-space Simpson: worst 2.1e-4 relative (x~77); 45 kJ/70 K prints 2.6e-31 % at 2 s.f., 431.5/70 K "less than 0.001 %"; dec 0-3.
- **Drawing/tail:** Ea line x = tail first x = 361.14; shade polygon vs plotted integral max 3.0e-5 fraction (0.003 pt) over 160 gas/T/Ea cases, so "about 0.01 percentage point" is true. vp dots at the peaks; rms guide on the hot curve; mark 2000 at right=740, 2000.1 no guide; Ea=0 no line/legend; Ea 45/70 K omitted with a note.
- **Extremes/refusals:** 1/2/3 curves, equal T, M 1/1000, T 50/3000, Print 3 distinct dashes + hatches 0/1/2, greys only; refusals name box, reason and fixed value and the rest draws.
- **Student:** as in MED 1 above, incl. Print/Small; key answers match the formulas.

## mutants (15 runs; 13 killed, 1 survived, 1 equivalent)
Killed: marks_on gate, student legend, vp points+ticks, rms guide, dec default 3, MARK_HI 10k, pct threshold 0.05, mark < x_top, Ea=0 describe, shade wording, hot index 0, mark hidden on student, legend T+1. Survived 30/30: decimals fallback 3->1. Equivalent: gating only the vp draw loops on `speeds` (vp_points is still empty on the student).

## not checked
No image input: I did not look at any PNG or a real 66 % bitonal copy; hatch/dash legibility and the resvg collision result are the test's claim (it passes), and my 66 % finding is luminance arithmetic. No full lib suite, UI/egui editor, PDF/DOCX, browser fonts or release timing.

## biggest gap
No energy axis / E/RT view: Ea is drawn at sqrt(2Ea/M) on a speed axis, so the Boltzmann factor e^-Ea/RT is never shown as an energy distribution, and the mean speed is table-only. Same scope gap critic 2 named.
