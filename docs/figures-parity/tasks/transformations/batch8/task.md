# Interaction task: `transformations`, batch 8

## The task

"Reflect the triangle in y = x by picking that line on the grid. Then make it a quarter turn about (3, -4), putting the centre there with a click."

Checklist entry: cap-02j ("Place a transformation's line or center on the grid with a preview"). The probe reads handles only at the opening state, so this cannot be scored; it was built at the owner's request.

## Reference (mathfigures.com/transformations, playwright-cli, headless, no login, 2026-11-11)

The page opens on its own triangle A(-7, 2) B(-2, 3) C(-6, 7), reflected over the y-axis (`ref/00-open.png`).

| step | action | shot |
|---|---|---|
| 1 | "Pick the line on the grid" (the button beside the line's select). It shows "Placing… (Esc to stop)", and faint y = x and y = -x appear | `ref/01-pick-the-line.png` |
| - | hover: the line a click would choose is drawn dashed and named, with its image previewed | `ref/02-hover-near-y-eq-x.png` |
| 2 | click: the line is chosen, the select changes, and placing ends | `ref/03-click-picks-the-x-axis.png` (a click near the x-axis) |
| 3 | "Add a transformation" | |
| 4 | "Rotation": "click the center", with a ring under the pointer on the nearest grid point | `ref/06-rotation-hover-ring.png` |
| 5 | click (3, -4): the centre is placed, then "click the turn you want" (three ghost turns) | `ref/07-centre-placed-then-pick-turn.png` |
| 6 | click a ghost turn | |

A finished rotation step has the same pick button, which places its centre again (`ref/08-repick-centre-hover.png`).

What the reference picks among, checked by hovering: only the x-axis, the y-axis, y = x and y = -x. A hover at (8, -3) offered the x-axis, not x = 8 (`ref/04-…`). Beside a chosen x = 2, at (2.3, -5), it offered y = -x, not x = 2 (`ref/05-…`). The independent checker's note that numbered lines are offered too did not hold on the page. A numbered line is still chosen in the select and moved by its number.

Reflection, 2 actions (button, click). Centre, 2 actions (add a rotation, click), plus a third to pick the turn.

## Ours before (main at 7f86b33)

No PNGs: with Pick off the figure draws and acts exactly as on main (`pick_off_draws_and_acts_as_the_figure_always_did`). How a teacher did the task before:

- **Reflection**: Transformation: Reflect (1), then open "Reflect in" and choose "the line y = x" (2). That is 3 actions, all in the panel; nothing on the grid chooses a line.
- **Centre**: Transformation: Rotate (1), then drag the diamond from (0, 0) to (3, -4) (1), or type x and y (2). Placing it with a click was not possible.

## Ours after (this batch, under Xvfb, driven through the real panel and grid)

The alpha notice is dismissed with one click in every run and not counted. The panel is scrolled to the Transformations section and the section is opened (one wheel turn and one click, the same before and after).

| step | action | shot |
|---|---|---|
| 1 | Transformation: Reflect | `ours-after/01-reflect-open-section.png` |
| 2 | Pick a line or centre on the grid: First (right under "Reflect in") | `ours-after/02-pick-first.png` |
| - | hover near y = x: the line is outlined across the grid, with the tip "Click to reflect in y = x" | `ours-after/03-hover-near-y-eq-x.png` |
| 3 | click: "Reflect in" now reads the line y = x, the image and rule follow, and Pick is back on Off | `ours-after/04-click-picks-y-eq-x.png` |
| 4 | Transformation: Rotate | |
| 5 | Pick: First | |
| - | hover near (3, -4): a ring on that grid point, with the tip "Click to turn about (3, −4)" | `ours-after/05-rotate-pick-hover-centre.png` |
| 6 | click: the centre is at (3, -4) (the x and y boxes read 3 and -4), the image A′(-2, -6) B′(-2, -3) C′(-4, -6) is drawn, and Pick is back on Off | `ours-after/06-click-places-centre.png` |

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish (reflection) | 2 | 3 (panel only) | 2 after choosing Reflect (switch, click) |
| actions to finish (centre) | 2 + turn | 2 (drag) | 2 after choosing Rotate (switch, click) |
| finished? | yes | yes, but not by a pick on the grid | yes |
| dead ends | 0 | 0 | 0 |

How it differs:

- The reference has a pick button per step. Ours has one switch naming the transformation (First to Fourth), so later transformations typed under Then are picked the same way, into their own Then line.
- The reference previews the image under the hover as well as the line. Ours outlines the line, or the ring at the centre, and names it in the tip, but does not preview the image. The workspace's area hover draws one outline, and drawing an image too would need shared workspace code.
- Esc does not leave the mode in ours. Choosing Off does, as does the click itself.
- After the centre, the reference asks for the turn on the grid. Ours keeps the turn in the panel's "Turn by".

Verdict: fewer-or-equal actions and finished.
