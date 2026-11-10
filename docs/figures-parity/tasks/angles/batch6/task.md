# Interaction task: `angles`, batch 6

## The task

"From the opening 60° angle: add a ray and delete it again on the figure, run ray 2 and the baseline through the vertex as lines,
make the baseline dashed, mark the two 60° angles with two congruence arcs each, shade one 120° angle and name the vertex O."

Capabilities: cap-i02 (delete a ray on the figure), cap-i01 (run a ray through the vertex on the figure), cap-06b (style a ray on
the figure), cap-04b and cap-04g (congruence arcs on a chosen angle, by a click), cap-04c (shade a chosen angle), cap-04h and
cap-04e (a named point at the vertex, by a click, with its name typed). cap-01f (more than six rays) is not part of this task:
Add ray now goes to 12.

## Reference (mathfigures.com/angles, headless playwright, 1280x800)

Opens with the baseline and ray 2 at 60° (ref/00-open.png).

| # | action | shot |
|---|---|---|
| 1 | Add ray (120°) | ref/01-add-ray.png |
| 2 | click the new ray (pop-up: Through the vertex Ray/Line, End points, Line style, Delete this ray) | ref/02-click-ray-popup.png |
| 3 | Delete this ray | ref/03-delete-this-ray.png |
| 4 | click ray 2 | |
| 5 | Through the vertex: Line | ref/04-ray2-through-vertex.png |
| 6 | Escape | |
| 7 | click the baseline | |
| 8 | Line | |
| 9 | Dashed | ref/05-baseline-line-dashed.png |
| 10 | Escape | |
| 11 | click the 60° angle (pop-up: Label, Congruence arcs, Shading) | |
| 12 | Two arcs | ref/06-angle-popup-two-arcs.png |
| 13 | Escape | |
| 14 | click the other 60° angle | |
| 15 | Two arcs | |
| 16 | Escape | |
| 17 | click a 120° angle | |
| 18 | Shading: Blue | |
| 19 | Escape | ref/07-arcs-and-shading.png |
| 20 | click the vertex (pop-up: Point) | |
| 21 | Show | |
| 22 | type O in Label | |
| 23 | Escape | ref/08-vertex-point-O.png |

## Ours before (main at e0d5cd4)

Same opening state (ours-before/00-open.png). Add ray worked and "Remove the last" took the last row away, but on the figure a ray
only had its direction knob: no click styled it, ran it through the vertex or deleted it (the Ray / Line and style buttons are in the
Rays card). No congruence arcs or per-angle shading anywhere ("Shade the angles" shades every angle at once). No named points: "Point
letters on the rays" letters the vertex and every ray at once, with fixed letters. Not finished: stopped at the arcs.

## Ours after (figtest build under Xvfb, 1280x800, nothing seeded)

The alpha notice is closed with Escape first (the environment, not counted).

| # | action | shot |
|---|---|---|
| 1 | Add ray (120°) | ours-after/01-add-ray.png |
| 2 | click ray 3 near its end (hover outlines that part: "Click to delete ray 3") | ours-after/02-hover-ray3-end.png, 03-delete-ray3.png |
| 3 | click ray 2 near the vertex (outlined, "Click to run ray 2 on through the vertex as a line") | ours-after/04-ray2-through-vertex.png |
| 4 | click the baseline near the vertex | ours-after/05-baseline-through-vertex.png |
| 5 | click the baseline along its middle: dashed | |
| 6-7 | click the 60° angle near its corner twice: one, then two arcs | |
| 8-9 | the same on the other 60° angle | ours-after/06-dashed-and-two-arcs.png |
| 10-13 | click a 120° angle near its corner four times: one, two, three arcs, then shaded | ours-after/07-shade-120.png |
| 14 | double-click the vertex: its name box opens with A | ours-after/08-dclick-vertex-edit.png |
| 15 | Ctrl+A | |
| 16 | type O | |
| 17 | Enter | ours-after/09-vertex-point-O.png |

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 23 (17 without the 6 pop-up closes) | - | 17 |
| finished? | yes | no: no arcs, no per-angle shading, no named point | yes |
| dead ends | 0 | 1 (a click on a ray did nothing) | 0 |

How it differs:
- Rays: the reference opens a pop-up on a click (Through the vertex, End points, Line, Delete this ray); ours acts where the ray is
  clicked: near the vertex it runs the ray through as a line (on a line, cuts it back to a ray), along its middle it steps the style,
  near its end it deletes it (once there are two or more rays; the baseline is never deleted). The hover outlines the part and names
  what the click does. The ray's end keeps its drag knob; a click on the knob puts a named point there. All parts are on the ray
  itself: the workspace zooms to the drawing, so the empty side behind the vertex is often off the screen.
- Angles: the reference's pop-up has Label, Congruence arcs (0-3) and Shading (four colours); ours steps the marks with each click
  near the corner: one, two, three arcs, then shaded (one tint), then plain. So shading takes 4 clicks against 3 there. A click on the
  label spot, further out, still steps degrees / x / blank, and a double-click near the corner types the label.
- Points: the reference's vertex pop-up has Show and a Label box; ours puts the next free letter there in one click and names it on a
  double-click (4 actions with Ctrl+A, as our label boxes open with the old text unselected). Named points can also be typed in the
  panel's Named points box (A: vertex; B: ray 2; C: ray 2 back).
- Labels: the reference labels only the first angle by default; ours labels every angle (as it always has), so the task's drawing
  shows all four measures.
- More than six rays (cap-01f): ours now has 12 rows; rows 7-12 added with Add ray are placed in the widest gap round the whole circle,
  and on such a figure each angle label is placed where it clears every ray and every other label.

Verdict: fewer actions and finished.
