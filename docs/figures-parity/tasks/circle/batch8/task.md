# Interaction task: `circle`, batch 8

## The task

"Pull point A off the circle to 1.5 radii, put it back on the circle, then move the radius's length label to the other side
of the radius."

Uses cap-01f (off the circle and back on), cap-01g (drag a point on the figure) and cap-i10 (a line's label across its line).

## Reference (mathfigures.com/circle, headless, no login)

The reference opens with A, B and C, radii OA and OB and chords CA and CB, and no line labels.

| # | action | shot |
|---|---|---|
| 0 | open | ref/00-open.png |
| 1 | drag A outward: it leaves the circle at 1.5 r, keeps its direction, radius OA becomes "Line OA", and chord CA follows | ref/01-drag-a-off.png |
| 2 | drag A back near the circle: it snaps on at 1 r | ref/02-drag-a-back-on.png |
| 3 | click the line to open its card | |
| 4 | Label: Measure | ref/05-label-the-line.png (shown on chord CB) |
| 5 | close the card (Escape) | |
| 6 | drag the label across the line: it goes where it is dropped, on the other side | ref/06-drag-label-across.png |

6 actions. In shot 02 the reference shaded the sector because its release landed inside an area. This is an artefact of
playwright's press and release, not a step. The label was tried on chord CB (the radius has no label until steps 3 to 5,
which are the same for any line). On the reference the dropped label lands exactly at the pointer, even on top of O and the
central angle's text (shot 06).

## Ours before (binary built from main before batch 8, under Xvfb, 1280 x 800, nothing seeded)

| # | action | shot |
|---|---|---|
| 0 | open (radius OA labelled r = 5 cm) | ours-before/00-open.png |
| 1 | drag A outward: A stays on the circle at its direction (the knob only turned it round) | ours-before/01-drag-a-out.png |
| 2 | drag the label r = 5 cm downward across OA: nothing moves | ours-before/02-drag-label.png |

Not finished: no point could leave the circle except a More point, and no label could be moved.

## Ours after (this branch, debug build under Xvfb, 1280 x 800, nothing seeded)

The alpha notice is closed with Escape first. This is part of the environment and is not counted.

| # | action | shot |
|---|---|---|
| 0 | open | ours-after/00-open.png |
| 1 | drag A outward: off the circle at 1.5 r; OA follows; the r = 5 cm label is left off and a note says why | ours-after/01-drag-a-off.png |
| 2 | drag A back near the circle: it snaps on, and the radius label comes back | ours-after/02-drag-a-back-on.png |
| 3 | drag the label r = 5 cm across OA: it moves to the other side and stays there | ours-after/03-drag-label-across.png |

3 actions; the figure is a circle with A on it and the radius's label on the other side.

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 6 | - | 3 |
| finished? | yes | no | yes |
| dead ends | 0 | 2 (A would not leave the circle; the label would not move) | 0 |

How the two differ:
- **Input.** Both drag the point and the label on the figure. Ours also has a "from the centre (radii)" slider for A, B
  and C, which matches the reference's From O box.
- **Radius label.** Ours labels the radius already, so the reference's three clicks to add a label are not needed. While
  A is off the circle, ours does not draw the label "r = 5 cm" on OA, because OA is no longer a radius; the notes say so.
  The reference keeps calling the line OA, and its length label would show OA's own length.
- **Where the label lands.** Ours keeps the dropped label on the chosen side, and the label placer picks the spot, clear
  of other text and lines. The reference puts it exactly at the pointer.
- **Student copy.** Unchanged by these drags. Rows that would state a value the figure no longer draws (chord, arc,
  sector, inscribed angle) read "A is off the circle" on both copies.

Verdict: fewer-or-equal actions and finished.
