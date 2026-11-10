# Interaction task: `composite-shape`, batch 5

## The task

"Start from the house, shade the roof yellow, and draw a dashed line from the top of the roof down to the middle of the floor."

## Reference side (mathfigures.com, headless, no profile)

1. House (Start with): the house is drawn.
2. Click the triangle: its pop-up opens (angles, sides, Shading, Delete).
3. Shading: Yellow.
4. Lines mode.
5. Drag from the roof's top corner to the middle of the bottom side: a dashed line is drawn (`ref/05-drag-line.png`).

A sixth click (Close on the pop-up) is not needed. During automation one extra drag ended on the wrong marker and was undone. That was the script's mistake, not the page's, so it is not counted.

## Our side (the app under Xvfb, 1280 x 800)

The composite shape is edited on its canvas editor (`figuresui::composite_canvas`). This batch did not change it, so ours-before and ours-after are the same app and the same five actions. Only the after shots are kept.

1. House (the Start with tile): `ours-after/01-house.png`.
2. Click the roof: the Triangle panel opens with its corners, Shading, Label inside and Delete (`ours-after/02-select-roof.png`).
3. Shading: the yellow swatch.
4. Line tool.
5. Drag from the roof's top corner to the diamond in the middle of the bottom side: the dashed line is drawn, and the roof shows yellow under the Line tool's greying (`ours-after/05-drag-line.png`).

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 5 | 5 | 5 |
| finished? | yes | yes | yes |
| dead ends | 0 | 0 | 0 |

What differs in how:
- Both sides select a piece and then use a panel with Shading and Delete. The reference's triangle panel also takes angles and side lengths with locks. Ours takes the corners' coordinates (a rectangle's panel takes Width and Height).
- Lines: both use a separate mode with snap markers on corners and midpoints. The reference also offers where a height meets a side as a marker. Ours snaps to the half-unit grid there, which lands on the foot when it is on the grid.
- Labels: both have a Label mode with pop-ups for a side (label, ticks, arrows, line, thickness), an angle (label, right-angle mark or arcs, shading) and a corner (point and name).

The canvas tests that drive each of these actions are `figuresui::tests::composite_canvas_*`.

Verdict: fewer-or-equal actions and finished.
