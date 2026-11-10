# Interaction task: `circle`, batch 5

## The task

"Put a point outside the circle and draw both tangents from it."

Capabilities: cap-01c (a point off the circle by its distance in radii), cap-01f (drag a point off the circle), cap-03b (both
tangents from a point outside the circle), cap-i04 (click a point outside the circle for the tangents from it).

## Reference (mathfigures.com/circle, headless playwright, 1280x800)

The reference opens with A (210°), B (330°) and C (100°) on the circle, radii OA and OB, chords CA and CB.

| # | action | shot |
|---|---|---|
| 0 | (opening state) | ref/00-open.png |
| 1 | type 2 in C's "From O" box (C goes 2 radii out) | ref/01-type-distance.png |
| 2 | click point C on the figure (its pop-up: Name, Join to, Tangents from here) | ref/02-click-point.png |
| 3 | Tangents from here: Both | ref/03-both-tangents.png |

## Ours before (main at 303d812)

Not run as a separate build. Points sat on the circle only: a More points row took a letter and a direction, the knobs moved round
the circle, and a Tangents row named a point on the circle. There was no point outside the circle, so no tangents from one. Not finished.

## Ours after (figtest build under Xvfb, 1280x800)

Our opening state has A only (B and C are not drawn), and A, B and C stay on the circle (the radius, chord and angles are built on
them), so the point that goes outside is a More point. Nothing seeded; the alpha notice was dismissed first (not counted).

| # | action | shot |
|---|---|---|
| 1 | click the circle at 100°: B | |
| 2 | click the circle at 200°: C | |
| 3 | click the circle at 300°: D | ours-after/03-click-circle-D.png |
| 4 | drag D well away from the circle (it leaves it: a row like `D 330 1.8r`) | ours-after/04-drag-D-off.png |
| 5 | click D: both tangents from it (Tangents row `D`) | ours-after/05-click-D-tangents.png |

Typed instead: More points `D 330 1.8r` and Tangents `D` (two rows; `D T U` also letters the touch points), or the More points row
and then a click on D.

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 3 | - | 5 (2 typed rows, or 1 row + 1 click) |
| finished? | yes | no | yes |
| dead ends | 0 | - | 0 |

How it differs: the reference sets the distance in a box beside the point and chooses Both in the point's pop-up; ours drags the point
off the circle on the figure (or types the distance as `1.8r`) and a single click on the point draws both tangents, with no pop-up.
Ours needs two more clicks only because our opening state has one point, not three. The touch points are lettered by typing the row
(`D T U`); the reference names them from the line pop-ups.

Verdict: `finished, more actions: the opening state has one point where the reference has three; the tangent step itself is one click
against the reference's two`.
