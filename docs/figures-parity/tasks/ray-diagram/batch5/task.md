# Interaction task: `ray-diagram`, batch 5

## The task

"From the opening state, switch to a concave mirror, put the object between F and C, mark the image
distance on the figure and print the answer key line under it."

## Reference (physicsfigures.com/ray-diagram, headless, no login)

1. Open Lens or mirror, click Concave mirror (2 actions). The mirror opens hatched, with C in
   place of 2F and the ray through C already on (`ref/01-concave-mirror.png`).
2. Open Object, click "Between C and F" (2): 15 cm, image 30 cm away (`ref/02-between-c-and-f.png`).
3. Open Marks, tick Image distance (2): a d_i dimension line under the drawing (`ref/03-image-distance.png`).
4. Open Title and answer key, tick Answer key (2): "Image distance: 30 cm, magnification: −2 /
   Real, inverted, magnified, in front of the mirror" (`ref/04-answer-key.png`).

## Ours before (c3f69f1, the code at main 303d812)

1. Open Lens or mirror, open the Type list, pick Concave mirror (3): a plain arc labelled 2F, no
   ray through C (`ours-before/01-concave-mirror.png`).
2. Open Object, click Distance, type 15 (3; no quick places).
3. Not finished: there is no Marks section and no answer key line; "answer key" there only means
   the image is drawn (`ours-before/02-typed-15-no-marks-no-key.png`).

## Ours after

Under Xvfb with the app's QA hooks (`VERSHION_SCRIPT`), 1280x800. The alpha notice's "Got it"
click is the environment, and scrolling the settings panel is not counted.

1. Open Lens or mirror, open the Type list, pick Concave mirror (3). As on the reference, a switch
   from a lens to a curved mirror turns on the hatch, C in place of 2F and the ray through C
   (`ours-after/01-concave-mirror.png`, summary "Complete · 4 of 4 rays").
2. Open Object, open Place it, pick "Between F and 2F" (3): 15 cm (`ours-after/02-place-it.png`,
   `03-between-f-and-2f.png`).
3. Open Marks, tick Image distance (2): the d_i dimension line, label as text, blank line or none
   (`ours-after/04-image-distance.png`).
4. Open Title and answer key, tick Answer key line (2): "Image distance: 30 cm, magnification: −2 /
   Real, inverted, magnified, in front of the mirror / Real is positive: 1/f = 1/do + 1/di and
   m = −di/do" (`ours-after/05-answer-key.png`; the whole figure rendered: `06-whole-figure-render.png`).

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 8 | stopped after 6 | 10 |
| finished? | yes | no: no image-distance mark, no key line | yes |
| dead ends | 0 | 0 | 0 |

How it differs:
- The two extra actions are our panel's drop-down lists: Type and Place it each take two clicks
  (open, pick) where the reference shows the choices as buttons.
- The object's distance and height can also be set by dragging its tip on the figure (below the
  axis turns it upside down), and the focal length by dragging F; the reference has no handles.
- Our quick places keep the 2F names for a mirror too (the panel's help says 2F is C there); the
  reference renames them "Beyond C" etc. for a mirror.
- Student copy: on ours the image, its label, the image's marks, the key line and the dashed
  virtual extensions are left off, their room kept.
- At 1280x800 the physics page's preview box crops the drawing's left side on ours, before and
  after alike (the shared box in `figuresui.rs::phys_vb`), so the image at 30 cm is off the
  preview in steps 2 to 4; the exported figure is whole (`06-whole-figure-render.png`).

Verdict: finished, more actions: 10 against 8, because two of the choices are drop-down lists.
