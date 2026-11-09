# Interaction task: enzyme-kinetics (reference: biology, enzyme-activity), batch 3

Task: "Draw rate against temperature for a human enzyme and Taq polymerase on one graph, and leave the optimum labels as blank lines for the students."

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 3 (tick Curve 2, open Marked, Blank line) | not possible | 4 (Rate against: Temperature, tick Curve B, open Marked, Blank line) |
| finished? | yes | no: the figure drew rate against substrate only | yes, clicked through under Xvfb |
| dead ends | none | n/a | none |

How it differs:
- The reference opens on temperature; ours keeps its Michaelis-Menten opening state (old saved figures and the census are unchanged), so choosing Temperature is the one extra action.
- In both, Curve 2 (ours: Curve B) defaults to Taq polymerase, so ticking it is enough. The key under the plot names the curves. Ours prefixes A: and B: so that the marks ("Optimum A", "Km B") can name their curve when there is more than one.
- Label modes: the reference has Text, Blank line and None. Ours has Name and value, Name only, Blank line and None. With Show answers off (or the Student + key export), values are blanked in the unit shown ("Optimum A: ____ °C", "Optimum: pH ____") and names become blank lines.
- Direct manipulation: an Unknown enzyme's optimum dot can be dragged along the axis. The reference has no on-figure actions here.
- Panel scrolling (two mouse-wheel moves) is not counted on either side.

Evidence: ref/00-open, 01-curve2-taq, 02-optimum-blank; ours-before/00-open (alpha notice over the substrate-only figure); ours-after/00-open, 01-rate-against-temperature, 02-tick-curve-b, 03-optimum-blank-line, and fig-* renders (temperature with denaturation, pH with three enzymes, the student copy, Large with long names, shape-only axes with arrows and titles, three inhibitor curves).

Verdict: finished, one more action than the reference, because our opening state stays on substrate.
