# Boltzmann distribution, critic 1, commit 8b01f3d

Verdict: **FAIL. 0 blocker, 0 major, 1 MED, 4 LOW, 3 minor.** The render is numerically correct: 22/22 boltzmann tests and `census_baseline` pass at this commit. The MED is test coverage, not a wrong number. Independent recompute: my own f(v), Simpson/adaptive speed-space integrals, libm erfc, and SVGs parsed from a probe and checked against those, not against `Report`.

## MED
1. **No test pins the typed givens to a drawn or printed answer; three one-line wiring mutants survive the figure's tests.** M9 (`m_g/100`, a 10x wrong molar mass) passes the named pinning test `the_curve_points...` because it compares the SVG to `Report`, which is wrong the same way; only a blank-count assert and a pixel-rounding assert kill it later. M6 (legend `fmt(c.t+1.0)`) and M10 (every curve printing the hottest curve's fraction) pass all 21 non-PNG tests. The blessed census catches any render change as a hash, but a hash cannot say which number is wrong and `VERSHION_CENSUS_BLESS=1` re-blesses it. Repro: change `self.m_g / 1000.0` to `/ 100.0`, then `cargo test --lib figures::chemistry::boltzmann::tests::the_curve_points_lie_on_the_maxwell_boltzmann_formula -- --exact` -> green. Fix: one hand-value test (N2 28.014 g/mol, 300 K -> 421.993 m/s; 300/600/900 K at Ea 20 kJ -> 0.111/4.564/14.818 %, legend 300/600/900 K).

## LOW
- **The shaded tail is the area under the clipped curve, not F to the printed decimals.** Default (Ea=10, hottest 600 K) shades 26.0450 % against the printed 26.049 % (3.6e-5 fraction); Ea=0 shades 99.9949 % against the printed 100.000 % (x_top=3.35 vp, missing tail 5.2e-5). The printed F is the correct one, and the gap is invisible.
- **Far-tail false precision.** Below 0.1 % `res_fmt` prints 4 significant figures, but the A&S erfc's relative error reaches 1.5e-3: Ea=431.5 kJ/mol, T=70 K (x=741) prints 3.192 x 10^-31 % against the exact 3.187 x 10^-31 %; 46 % of the 2.35M scientific-branch (Ea,T) pairs probed differ in the last digit (fixed branch 0.9 %, worst Ea=5.0 kJ T=2335 K: 91.556 vs 91.557). No material error: erfc is evaluated directly, so the tail keeps ~3.5 figures, and 1.4e-7 in F is ~70x below a dec=3 percentage unit.
- **Coarse drawn curve when a high Ea stretches the axis.** M=1 g/mol, T=50/3000 K, Ea=500 kJ -> x_top=40000 with vp(50 K)=912, so the cold curve is ~18 segments and its polygon encloses 1.00125 (default 1.0000015), against the note "each curve encloses area 1".
- **Brittle axes test.** `the_axes_...` recomputes the data step from 2-decimal pixel coordinates and demands 1e-9 niceness; a correct layout with 6 tick intervals (109.33 px) fails "step 999.9695 is not a nice step". That, not a science assert, is what caught M9.

## minor
- Ea=0: the legend shows a dashed "Ea = 0 kJ/mol" swatch though no Ea line is drawn (the y axis is the line), and the tail shades the whole area.
- Default dec=3 prints 421.993 m/s (6 s.f.) from a 3-s.f. temperature; a chemist will want dec=1 or "3 s.f.".
- The student copy keeps the mark dot/guide; its `cy` encodes f(mark) to ~1e-4 of y_top, but the curve encodes it too, so it is a read-off, not a leaked number.

## verified (independent)
- f(v) vs 4*pi*(a/pi)^1.5*v^2*exp(-a v^2), 5 T (50-3000 K), 7 M (0.001-1 kg/mol), 400 v each: worst relative 4.2e-14. Peak exactly at vp (slope) and height 4/(sqrt(pi e) vp): 2.3e-16.
- Area 0-12 vp = 1.000000000000 (Simpson, all 9 T/M pairs); drawn 0-3.2 vp = 0.999865 (missing 1.35e-4); 0-x_top from the SVG polygon = 1.0000015 (default).
- Speeds: N2 300 K vp 421.993, mean 476.168, rms 516.834; mean/vp = 2/sqrt(pi) = 1.128379167, rms/vp = sqrt(3/2) = 1.224744871; O2 300 K vp 394.849 (OpenStax 9.32: "a little less than 400").
- F(Ea) vs my own adaptive speed-space integral: worst 4.4e-10 over T=50/300/3000, M=0.002016/0.028014/1, Ea=0.5-100 kJ (x<18). A&S erfc vs libm: worst absolute 1.39e-7 at x=0.26. Printed: 4.564/26.049 %, 0.111/4.564/14.818 %, 84.903 %, 100 % all right at dec=3 except the LOW cases.
- Ea line/tail: va=sqrt(2Ea/M)=844.943 m/s default; the line x and the tail's first point sit at px(va) within 0.06 px; `data-f="c1"` (hottest); each curve's printed F matches its own T; mark 2000 m/s on 600 K -> 5.630e-7 s/m with the dot on that curve.
- Units: 28.014 g/mol -> 0.028014 kg/mol, Ea kJ -> J, y axis "Fraction ... (s/m)", legend 300/600/900 K tied to each stroke/dash in Colour and Print; table in m/s.
- Extremes: 3 equal T -> 1 curve + 2 notes; 1 curve; M=1 and 1000; T=50 and 3000; Ea=0 and 500 (500 at 50 K/N2: line omitted with a note, F still printed); mark 0 / 2000 (axis end, drawn) / 2000.1 (note, no guide); dec 0-3. Refusals name the box, the reason and the fixed value; the rest draws.
- Student: 0 `data-o="answer"`, 0 answer texts, 8-13 blanks, no tail. Print: 0 non-grey hex, 3 distinct dashes + 3 hatches, min font 13 px -> 8.58 px at 66 %. Pure; 36 draws, worst 0.756 ms (debug).
- Registration: chemistry.rs:429, faq.rs:61 with 5 Q&A, interact.rs:326 NO_HANDLES; `census_baseline` passes here.

## mutants (single pinning test; survivors re-run over the module)
M1 vp killed; M2 rms killed; M3 fraction killed; M4 erfc branches killed; M5 tail start killed; M6 legend T+1 **survived, 21/21**; M7 table answers killed; M8 Ea *100 J killed; M9 m_g/100 **survived the named test** (module kills it via 2 layout/text tests); M10 all curves hottest F **survived, 21/21**; M11 peak doubled killed; M12 Ea line at 0 killed. 9 killed, 3 survived.

## not checked
No image input: I did not look at a PNG at 100 % or 66 %, so hatch/dash legibility, the bitonal copy and the visual collision result are unverified. The resvg ink-box audit is theirs (passes, 216 renders) and skips grid/tail strokes. No full-lib run, no UI/start-state inspection.

## biggest gap versus the textbook
OpenStax 9.5 (Figs 9.32-9.33) marks vp and u_rms on the speed axis; this figure puts all three speeds only in a table and has no on-curve markers (the optional guide marks an arbitrary speed and is off by default), so a student cannot see the vp shift without reading numbers. The Ea treatment uses the equivalent speed tail but offers no energy axis, which is how 12.5 presents it.
