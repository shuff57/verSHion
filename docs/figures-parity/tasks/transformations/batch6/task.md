# Interaction task: `transformations`, batch 6

## The task

"Reflect triangle ABC over the y-axis, then rotate that image 90° clockwise about the origin. Then give me the question copy, where students name the transformations."

Caps it exercises: cap-02a (one transformation after another, each acting on the last image), cap-04b (question version), and cap-03b/cap-04e by the way (the in-between image is drawn dotted on the key and left off the question copy).

## Reference (mathfigures.com/transformations, playwright-cli, headless, no login)

The page opens on its own triangle A(-7, 2) B(-2, 3) C(-6, 7), already reflected over the y-axis (`ref/00-open.png`).

| step | action | shot |
|---|---|---|
| 1 | "Add a transformation" | `ref/01-add-transformation.png` |
| 2 | "Rotation" ("click the center") | `ref/02-rotation.png` |
| 3 | click the origin: three candidate turns are previewed | `ref/03-click-centre.png` |
| 4 | click the 90° clockwise preview: A″B″C″ is drawn dotted, labelled with two primes | `ref/04-pick-turn.png` |
| 5 | "Question" | `ref/05-question.png` |

On the question copy the reference draws the pre-image and the last image only: no in-between image, no mirror line, no centre.

## Ours before (main at e0d5cd4)

`ours-before/00-open.png` (the opening state, unchanged by this batch). There is one transformation only: no way to rotate the reflected image, and no question version (Show answers off removes the image). Not finished.

## Ours after (this batch)

Opens on A(1, 1) B(4, 1) C(1, 3) translated by (3, -2) (`ours-after/00-open.png`; the alpha notice is dismissed with one click in every run and not counted).

| step | action | shot |
|---|---|---|
| 1-2 | Transformation: Reflect (open the list, pick) | |
| 3-4 | Reflect in: the y-axis (open the list, pick) | |
| 5-6 | Then: click the box, type `rotate 90 clockwise` | `ours-after/01-reflect-then-rotate.png` (steps 1-6 seeded as `op`, `axis`, `then`: the QA hook drives clicks and the wheel, but not egui's dropdowns reliably) |
| 7 | Show answers off (Show section, top of the panel) | |
| 8 | Question version on (the switch appears once Show answers is off, one wheel scroll down) | `ours-after/02-answers-off-scrolled.png`, `ours-after/03-question.png` |

Steps 7 and 8 were driven through the real panel under Xvfb (clicks at the switches, one mouse-wheel scroll). The key draws A′B′C′ dotted in blue and A″(1, 1) B″(1, 4) C″(3, 1) solid in purple, with both rules listed. The question copy draws the pre-image and A″B″C″ only, with the rules as blanks ("Rule ____", "Then ____") and A″'s coordinates listed.

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 5 (its opening state already had the reflection) | n/a | 8 (from our opening translation) |
| finished? | yes | no (one transformation only) | yes |
| dead ends | 0 | 1 (nothing chains) | 0 |

How the two differ:
- Input: the reference adds a step from a menu and places the centre by clicking the grid, then picks one of three previewed turns. Ours types the later step as one line under Then (`rotate 90 clockwise about (1, 2)`, `reflect in y = x + 2`); its centre or mirror line can then be dragged on the grid, with the images following live (unit test `a_later_centre_or_mirror_line_is_dragged_on_the_grid_and_written_back`, through `interact::pick`).
- First step: the reference opened on a reflection, ours on a translation, so ours spent 4 actions picking Reflect and the y-axis that the reference did not need. From equal starting points both take 3-4 actions for the second step.
- Versions: the reference has a three-way Student / Question / Answer key switch. Ours is Show answers plus a Question version switch that appears when Show answers is off: 2 actions against 1. The Student + key button exports the question copy and its key together.
- Both question copies draw the last image only, with no mirror line or centre; ours also blanks the rules under the grid.

Verdict: `finished, more actions: the opening state differs (ours starts on a translation, 4 actions to make it the reflection) and the question copy is two switches, not one`.
