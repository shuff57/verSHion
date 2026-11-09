# Interaction task: `circle`, batch 2

## The task

"Put points A, B, C, D on the circle, draw chord BD, the tangent at C, shade the minor segment cut off by BD, and label arc BC 70°."

Capabilities: cap-01b, cap-01g, cap-i01 (points), cap-02a (line between chosen points), cap-03c, cap-i03 (tangent at any point, by click),
cap-i09, cap-06b (shade an enclosed area, a segment), cap-i07 (label an arc).

## Reference (mathfigures.com/circle, headless playwright, 1280x800)

The reference opens with A (210°), B (330°), C (100°), radii OA, OB and chords CA, CB.

| # | action | shot |
|---|---|---|
| 1 | click the circle between A and B (opens the arc's pop-up) | ref/01-click-arc.png |
| 2 | Add point here (D at 270°, the arc's middle) | ref/02-add-point-here.png |
| 3 | click point B (pop-up: Name, Join to, Tangent here) | ref/03-click-B.png |
| 4 | Join to: D | ref/04-join-BD.png |
| 5 | Escape | |
| 6 | click point C | |
| 7 | Tangent here: Tangent | ref/05-tangent-C.png |
| 8 | Escape | |
| 9 | click inside the segment (dead end: the click landed on arc DB, its pop-up opened) | ref/06-click-segment.png |
| 10 | Escape, click again deeper inside the segment (Area BD pop-up) | |
| 11 | Shading: Blue | ref/07-shade.png |
| 12 | Escape | |
| 13 | click arc BC (pop-up) | |
| 14 | Label: Text | |
| 15 | click the text box, type 70° | |
| 16 | click away | ref/08-label-arc.png |

## Ours before (main at 4337404)

Opening state: the circle, O and radius OA, A at 20°. Clicking the circle does nothing (ours-before/01-click-circle-nothing.png). There is no
fourth point, no tangent except at A, no shading but the sector AB, and no label on an arc. Not finished.

## Ours after (re-run on the one click area, debug build under Xvfb, 1280x800)

Re-run after the click targets moved into one area that acts on the part under the pointer (the label zones are gone): a point's knob
first, then within 5 px of a drawn line (away from its ends) a label, within 6 px of the circle a new point, an angle's wedge a label,
the ring just outside an arc a label (unless that spot is in an enclosed area outside the circle), and anywhere else in an enclosed
area, inside the circle or outside it, shading. Hovering outlines the part and its tip says what a click does.

| # | action | shot |
|---|---|---|
| 1 | click the circle at 330°: B | ours-after/01-click-circle-B.png |
| 2 | click the circle at 100°: C | ours-after/02-click-circle-C.png |
| 3 | click the circle at 270°: D | |
| 4 | click point C: its tangent | ours-after/03-click-circle-D-then-click-C-tangent.png |
| 5-7 | Lines between points: + Add, click the row, type BD | (seeded) |
| 8 | click inside the minor segment, 10 px past chord BD: shaded (outlined under the pointer) | ours-after/04-click-segment-shade.png |
| 9-11 | Arc and line labels: + Add, click the row, type arc BC 70° | (seeded) |

ours-after/05-hover-line-outlined.png: with chord BD drawn, the pointer 30% along BD outlines the chord and its tip reads "Click to label
line BD with its length"; a click there labels it (a click anywhere along a drawn line does, not only at its middle).

Seeded: the two typed rows (`joins` = BD, `marks` = arc BC 70°) went in through VERSHION_FIG_PARAMS because the QA hook cannot scroll the
side panel to the two list controls; they are counted above as the three panel actions each would take. Every other step is a real click
in the running app. Waits between clicks were needed in the script: the figure's canvas grows when a new label needs room, and a click in
the very next frames is mapped with the old canvas scale (a workspace-wide behaviour, not new here).

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 16 (incl. 3 pop-up closes) | - | 11 |
| finished? | yes | no | yes |
| dead ends | 1 (a click inside a thin segment hit the arc) | 1 (clicking the circle did nothing) | 0 |

How it differs:
- Points: the reference's click on an arc opens a pop-up with Add point here (the arc's middle); ours puts the point where the circle is
  clicked (within 6 px of it), in one click, snapped to whole degrees and the familiar angles.
- Tangent: the reference clicks the point, then Tangent here; ours is one click on the point (again to take it off).
- Line between points: the reference picks Join to in the point's pop-up; ours is a typed row (BD) in Lines between points. No click on
  the figure joins two points yet (cap-i02).
- Shading: both click inside the area; the reference then picks a colour, ours shades in one click (one colour, a hatch in Print). A
  click more than 5 px from any line and 6 px from the circle shades, so a thin segment stays clickable.
- Labels: the reference clicks a line, angle or arc and picks from a pop-up; ours labels by a click on the line (anywhere along it), in
  the angle's wedge or just outside the arc, and typed text is a row (arc BC 70°). Here A (at 20°) lies on arc BC, so arc BC is offered
  from the second ring out (a click there gives its degrees, 130°); the text 70° was typed.

Verdict: fewer actions and finished.
