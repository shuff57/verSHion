# Interaction task: ogive (cumulative frequency), batch 4

Task: "Draw the median and both quartiles as reading lines, with a blank under each for students to write the value on."

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 6 (Start from an example, Edit the first example, open Readings, Blank for median, lower and upper quartile) | not possible | 6 (Reading: open the menu, choose None; Mark: Median and quartiles; Blank line for the median, lower and upper quartile), plus 3 scrolls of the panel |
| finished? | yes | no: a reading was either drawn with its value (key) or not drawn at all (student copy) | yes, run under Xvfb with the app's QA hooks |
| dead ends | none | n/a | none |

The reference opens on empty axes and gets its data from the example; ours opens on the test-score table already reading the 40th percentile, so its first two actions turn that reading off. The alpha notice is dismissed once before step 1 and is not counted.

How it differs:
- Input: both are panel controls; the reference shows each reading as Off / Lines only / Value / Blank, ours as Value / Lines only / Blank line under Mark (which reading exists is still chosen with Mark, so no Off per quartile).
- The blank: both put it under the horizontal axis below the foot of the guide. The reference draws bare lines; ours puts the reading's name before each line ("Q1 =", "M =", "Q3 ="), centred on its guide, and moves a blank to a second row when two would touch.
- Student copy: on the reference the blanks are the student version; ours draws the blanks in both copies, and Show answers off still hides the cumulative column and the worked answers (ours-after/05 is the student copy with a smooth curve and crosses).
- Nothing is done on the figure itself on either side: the reference graph takes no clicks for this task.

Evidence: ref/00-open.png, ref/01-start-from-an-example.png, ref/02-edit-first-example.png, ref/03-three-readings-blank.png; ours-after/00-open.png (opening, alpha notice), 01-reading-none.png, 02-median-and-quartiles.png, 03-median-and-q1-blank.png, 04-all-three-blank.png, 05-render-student-copy-smooth-crosses.png. No ours-before shots: the build before batch 4 had no per-reading choice to click.

Verdict: fewer-or-equal actions and finished.
