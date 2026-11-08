# Interaction task: `transformations`, batch 2

## The task

"Draw triangle A(1, 1) B(4, 1) C(1, 3), move C to (2, 4), rotate it 90° about (0, 0), give me the student copy without the image."

Caps it exercises: cap-01a (click the vertices onto the grid), cap-i01 (drag a vertex), cap-02e/02g (turn, about a centre), cap-04a (student copy). cap-i02 (click to set a translation) was run separately as `ours-after/04-click-translation.png`.

## Reference (mathfigures.com/transformations, playwright-cli, headless)

The page opens on a triangle reflected over the y-axis.

| step | action | shot |
|---|---|---|
| 1 | "Start the preimage over" | |
| 2-5 | click grid points (1, 1), (4, 1), (1, 3), then (1, 1) again to close | `ref/01-click-vertices.png` |
| 6 | drag C to (2, 4): it snaps to the grid point, the image follows | `ref/02-drag-c.png` |
| 7 | remove the opening reflection step | |
| 8 | "Add a transformation" | `ref/03-add-transformation.png` |
| 9 | "Rotation" ("click the center") | `ref/04-rotation.png` |
| 10 | click the origin: three candidate turns are previewed ("click the turn you want") | `ref/05-click-centre.png` |
| 11 | click the 90° anticlockwise preview | `ref/06-pick-turn.png` |
| 12 | "Student" | `ref/07-student.png` |

The centre stays as a small ring on the student copy. It cannot be dragged afterwards: a press on it selected vertex A. To move it you use "Pick the center and turn on the grid" or type it.

## Ours before (main at 4337404)

| step | action | shot |
|---|---|---|
| - | the triangle is fixed at A(1, 1) B(4, 1) C(1, 3) (it happens to match the task) | `ours-before/00-open.png` |
| - | move C: **impossible** (no vertex control, no handles) | |
| 1 | "Rotate" (90° anticlockwise about the origin is the default) | `ours-before/01-rotate.png` |

Not finished: C cannot be moved, and no other triangle can be drawn.

## Ours after (this batch)

| step | action | shot |
|---|---|---|
| - | opens on A(1, 1) B(4, 1) C(1, 3) on a -10..10 grid, with vertex knobs | `ours-after/00-open.png` |
| 1 | drag C to (2, 4): it snaps, and the image and coordinates follow live | `ours-after/01-drag-c.png` |
| 2 | "Rotate" (the turn and the origin centre are defaults) | `ours-after/02-rotate.png` |
| 3 | Show answers off (Show and hide section), or 1 click "Student + key" to export both | `ours-after/03-student.png` (seeded `show_answers=false`: the QA script hook cannot scroll the panel to the switch) |

Drawing a different triangle by clicking, the reference's way: clear the vertex list (Data section, 1 action), click 3 grid points, then click A to close it. That is 5 actions against the reference's 5 (start over plus 4 clicks). Covered in-app by unit test `clicking_grid_points_builds_the_pre_image_and_clicking_a_closes_it`. It was not screenshotted, because clearing the data box needs panel navigation the script hook can't scroll to.

Translation by click: from the opening state, one click on grid point (-3, 4) moves A' there. The rule becomes (x - 4, y + 3). Shot: `ours-after/04-click-translation.png`.

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 12 (from its opening state) | n/a | 3 (triangle already there; 8 if it has to be clicked out) |
| finished? | yes | no (C cannot move) | yes |
| dead ends | 0 (the opening reflection had to be removed: 1 extra action) | 1 (no way to move C) | 0 |

How the two differ:
- Input: both build the preimage by clicking grid points and closing on the first. Both drag a vertex to move it, with the image following live and snapping to the grid.
- Rotation: the reference is a multi-step on-grid flow (choose Rotation, click the centre, click one of three previewed turns). Ours is one panel choice, with the angle and centre as defaults. The centre is a draggable diamond on the figure (the reference's centre is not draggable), or it can be typed.
- Student copy: the reference has a one-click Student view. Ours is the Show answers switch (further down the panel), or one click on Student + key to export both copies. Both keep the centre ring on the student copy and drop the image.

Verdict: `fewer-or-equal actions and finished`.
