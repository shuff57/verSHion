# Circuit diagram (capacitors and combination): combined critic, 2026-10-29

One combined accuracy + teacher + regression critic over `fbef675` and `9addc16` at `9addc16`, read-only, in its own
`git archive` copy. Verdict: **FAIL — 2 blockers, 7 majors, 11 minors.** Every defect is reachable from ordinary editor
controls; none needs a hand-edited link. All numbers were recomputed independently and every one was right — the
arithmetic was never the problem.

Fixed in the commit after. Full critic report: `/tmp/crit-l-report.md` (its own working copy; not kept in the repo).

## Blockers

| # | Finding | State |
|---|---------|-------|
| B1 | **Combination + voltmeter across component 1 drew the meter above the canvas.** The above-the-wire placement is only valid for the series layout, whose top wire is at y = 150; a combination's is at y = 80, so the meter landed at y = −2 with half of it clipped. | **fixed**: a horizontal component in a combination puts its meter *below* the wire (y = top_y + 82), which is empty space there. Test: no circle in any combination + voltmeter render has a negative `cy`. |
| B2 | **REGRESSION I introduced: resistor mode read a capacitance as a resistance.** Adding `F` to the shared quantity parser, and using that parser for resistor values, meant `470 µF` became 470 µΩ — "R_eq 4 Ω, V₁ = 1.41 mV" on a zigzag labelled 470 µF. The base build printed `?`. Also `2 N` became 2 nΩ. | **fixed**: the parser now takes the allowed units as a parameter. `parse_ohm` accepts ohms or a bare number; `parse_cap` accepts farads; the battery keeps `parse_qty`. Uppercase `N`/`P` prefixes removed (`N` is newtons). Test asserts each cross-unit case is `None`. |

## Majors

| # | Finding | State |
|---|---------|-------|
| M1 | Combination kept the narrow branch pitch (175) with a solved voltmeter, so a reading printed on top of the next branch's value. | **fixed**: 230 px when solved, as parallel already did. |
| M2 | Voltmeter on the last component ran 6 px off the right edge and clipped the final "V". | **fixed**: the right margin for a meter is 160 px (was 140). Test asserts every circle centre + 18 px is inside the canvas width. |
| M3 | Branch name and value collided for long values ("R₂4.7 kΩ", "C₁470 µF"); the pitch was a fixed 125. | **fixed**: the branch pitch is measured from the widest value and name (`56 + value + name`), in both parallel and combination. |
| M4 | Series with four parts and a switch dropped the slot to 58 px, merging `4.7 kΩ4.7 kΩ`. | **fixed**: the series canvas grows when the labels need it; the standard canvas and its right-hand wire are untouched, so the default render does not move. |
| M5 | Capacitor mode read a voltage as a capacitance: "12 V" → 12 F. | **fixed** by the same unit-parameterised parser (`parse_cap` refuses volts and amps). |
| M6 | Parallel with a 0 Ω branch printed V₁ = V₂ = 12 V beside R_eq = 0 — a short circuit with a full voltage across every branch. | **fixed**: a 0 Ω branch leaves 0 V everywhere and no current stated (`solve_combo` already had the right semantics). Test added. |
| M7 | Capacitor mode left the ohms value printed on the figure ("2 Ω" on a capacitor symbol), so a student copies a resistance onto a capacitor. | **fixed**: a value that is not a capacitance draws a blank line instead of the text; the teacher's typed text is still in the editor field, and the Notes box explains. |

## Minors, and what was done

- **A hidden battery still produced answers** (capacitor mode turned Q and V into `?`, resistor mode kept solving). The
  resistor path now respects `Battery value shows: None` as well — the worked values would have given the hidden
  voltage away. Test added.
- **`fmt_qty` stopped at nano**, so 100 pF + 100 pF printed `5.00e-11 F`. Pico added.
- **The resistor/bulb toggle did nothing in capacitor mode** (the kind is ignored there). Its click zone is left off
  in capacitor mode rather than silently flipping a hidden parameter.
- **A one-part combination fell back to series silently** — fixed before the critique landed (`3c394b2`): the Notes
  box says so.

Left open (recorded, not fixed):
- `fmt_qty`'s three significant figures can disagree with the printed inputs at a .5 rounding (4 × 470 µF prints
  C_eq 117 µF, exact 117.5, so Q/C_eq reads 12.05 V against a 12 V battery). The printed numbers are each right to
  3 s.f.; making them agree needs a different rounding rule.
- Mirror mode mirrors the results table's text ("3 A  I ="). Pre-existing, outside this round.
- "Fill meter readings" does nothing in capacitor mode (there are no meters); a 0 µF branch reads `?` though it is an
  open circuit; an unknown part in parallel blanks V_k even though the branch voltage is the battery's.
- This figure has no label-size control, so the Large/Small sweeps the brief asks for are unreachable; simulated 1.4×
  text shows little headroom even after M3/M4.

## Verified correct (so silence is not approval)

- **Every number, recomputed independently**: 12 Ω → 1 A; 12/11 Ω → 11 A; combination 2 + (4∥4) = 4 Ω, 3 A, 6 V + 6 V,
  1.5 A each; 4.7 kΩ combination 6.27 kΩ, 1.91 mA, 9 V + 3 V; bulbs 4.4 Ω; capacitors 5 µF/60 µC/6 V; 4 × 470 µF in
  series 117.5 µF/1.41 mC; parallel 1.88 mF; combination 20/3 µF, 80 µC, 8 V + 4 V; 1 F + 1 F → 500 mF/2.5 C.
- **Q = CV and the charge/voltage splits**, including the combination's shared charge; **open switch** → 0 A, 0 V,
  0 C with the equivalent still printed.
- **No leak through drawn geometry** with answers off: no position, canvas size, dot or plate depends on a solved value.
- **66 % bitonal** at 1×, 1.3× and 2×: capacitor plates keep a 6–7/8–9/13–14 px white gap and never merge; junction
  dots and the results table survive.
- **Regression**: resistor series renders are byte-identical to the base; parallel is a pure recentring (the intended
  label-gap fix — the base default had "12 V" overlapping "2 Ω" by 5 px) with identical readouts.
