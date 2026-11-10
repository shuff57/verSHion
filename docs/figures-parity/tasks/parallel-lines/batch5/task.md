# Interaction task: `parallel-lines`, batch 5

## The task

"Three transversals across the two parallel lines; mark the corresponding angles above-right of t at m and at n congruent with one
arc each, and put a point A where t crosses m."

Capabilities: cap-01d (three or more transversals), cap-i01 (congruence arcs by a click on the angle), cap-05a and cap-05b (a named
point where lines cross, by a click there).

## Reference (mathfigures.com/parallel-lines, headless playwright, 1280x800)

Opens with m, n and t at 65°.

| # | action | shot |
|---|---|---|
| 1 | Add transversal (s, 120°) | |
| 2 | Add transversal (u, 45°) | ref/01-add-transversal-twice.png |
| 3 | click the angle above m right of t (pop-up: Label, Congruence arcs, Shading) | ref/02-click-angle-popup.png |
| 4 | Congruence arcs: one | |
| 5 | Escape | |
| 6 | click the angle above n right of t | ref/03-second-angle-popup.png |
| 7 | Congruence arcs: one | |
| 8 | Escape | |
| 9 | click where m and t cross (pop-up: Point) | ref/04-click-crossing-popup.png |
| 10 | Show | |
| 11 | click the Label box | |
| 12 | type the name | ref/05-show-point-name-P.png |

## Ours before (main at 303d812)

Same opening state (ours-before/00-open.png). Add transversal set the count to 2, so a third transversal could not be had (cap-01d).
A click on an angle only labelled it with its measure: no congruence arcs or shading anywhere in the figure. No param or click put a
point on the figure. Not finished.

## Ours after (debug build under Xvfb, 1280x800, nothing seeded)

| # | action | shot |
|---|---|---|
| 1 | Add transversal (u, 115°) | |
| 2 | Add transversal (v, 45°) | ours-after/01-add-transversal-twice.png |
| 3 | click near the corner of the angle above m right of t: one arc (the hover outlines the wedge, tip "Angle 2: click for two congruence arcs (arcs, then shading)") | ours-after/02-click-angle-2-one-arc.png |
| 4 | click near the corner of the angle above n right of t: one arc | ours-after/03-click-angle-6-one-arc.png |
| 5 | click where m and t cross: point A | ours-after/04-click-crossing-point-A.png |

ours-after/05-hover-line-m-outlined.png: the pointer on line m outlines it; a click there makes it dashed, a double-click renames it.

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 12 (incl. 2 pop-up closes and 2 for the name) | - | 5 |
| finished? | yes | no | yes |
| dead ends | 0 | 2 (no third transversal; the angle click labels instead) | 0 |

How it differs:
- Angles: the reference opens a pop-up with Label, Congruence arcs (0-3) and Shading (four colours); ours steps the marks with each click
  near the angle's corner: one, two, three arcs, then shaded (one tint), then plain. A click on the label spot, further out, still
  labels it; a double-click near the corner types its label.
- Points: the reference's crossing pop-up has Show and a Label box; ours puts the next free letter there in one click (a second click
  takes it away), and a double-click names it. The ends of lines take points the same way (a transversal's top end through its angle
  knob); an end with a point loses its arrowhead.
- Lines: the reference's line pop-up has Name, End points, Parallel arrows, Line and Delete this line; ours: a click steps the line's
  style, a double-click (on the line or its name) renames it, and a click on the spot just past its start deletes it. Arrows and end
  styles stay in the panel.
- Layout: with three transversals both spread them out and run the lines past every crossing; ours widens the gap between transversals
  until no two meet within 50 of each other on a line or at their ends.

Verdict: fewer actions and finished.
