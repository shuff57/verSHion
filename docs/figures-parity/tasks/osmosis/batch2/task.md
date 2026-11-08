# Interaction task: `osmosis` batch 2 (new figure)

Task: "Make a student worksheet: a plant cell in three solutions (half, same, double), results hidden, tonicity and result as blanks, empty boxes for the water arrows, then give me the key with the explanation."

## Reference (biologyfigures.com/osmosis, playwright-cli, headless)
Steps (PNGs in ref/): 01 open (red blood cell three-up already shown), 02 Scene = Plant cell, 03 U-tube, 04 Dialysis bag, 05 Split beaker.
Reference actions for the task: scene select (1), Show stays Three-up, Each cell shows = Starts (1), Order = Shuffled (1), Labels group: tonicity Blank, result Blank (2 + opening the group), Arrows group: water Blank boxes (1 + opening), Chart title and answer key group: answer key (1 + opening). About 10 actions, three collapsed groups opened.

## Ours
Before: no osmosis figure (not built), so the task could not be done.
After: the figure opens on the three-up with the key printed. Teacher: Scene = Plant cell (1), Each cell shows = Starts (1), Order = Shuffled (1), Tonicity label = Blank (1), Result label = Blank (1), Water arrows = Blank boxes (1), Show answers off for the student copy (1); "Student + key" exports both. 7 actions plus the export button. Example "Red blood cell: predict the result (student copy)" in Start from an example sets all of it in 1 action.
ours-after/01-open.png: the Figures tab opened on osmosis through the QA hook (first frame, alpha dialog still up). Renders of each scene, the student copy and Print are in /tmp/claude-1000/batch2-osmo/ (not committed). Scripted clicks through the egui panel were not run; the counts above are by construction of the controls.

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | ~10 | n/a | 7 (1 with the example) |
| finished? | yes | no (no figure) | yes |
| dead ends | 0 | n/a | not measured |

How: the reference groups its controls in collapsed sections; ours are one flat list and label modes are a choice each. The student copy hides the explanation and leaves blanks empty; the key fills blanks, draws the arrows in the boxes and prints the explanation.

Verdict: finished, comparable action count; clicks not scripted.
