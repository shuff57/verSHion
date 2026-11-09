# Interaction task: motion-map (reference: physics, motion-map), batch 4

Task: "Make a motion map of the cart that speeds up, cruises and slows to a stop, then have it go back at 4 m/s instead of resting, with the times under the dots and an acceleration arrow over each segment."

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 6 (open Motion, segment 4: Constant velocity back, open Dots, On the dots: Times, open Arrows, tick Acceleration) | not possible | 6 (open Motion, segment 4: Constant velocity back, open Dots, On the dots: Times, open Arrows, tick Acceleration for each segment) |
| finished? | yes | no: four fixed stages in one direction, no times, no acceleration arrows | yes, clicked through under Xvfb (1280 x 800) |
| dead ends | none | n/a | none |

How it differs:
- Both edit the motion as Segment cards (what it does, speed, time, which way from rest, a worked line, move up and down, delete, Add a segment). Ours reuses the motion graphs' card. Both keep a 4 m/s speed ready on a rest segment, so switching it to Constant velocity back moves straight away.
- Both draw the way back a row above, with the 12 s turning dot on the new row pointing back. Both draw a dashed acceleration arrow, along the motion while it speeds up and against it while it slows down, and a = 0 over the constant velocity and the way back. Both drop crowded time labels a level lower (1 s under t = 0 s).
- Velocity arrows: ours keeps its first-version scale (2 dot widths per m/s), so its arrows are shorter than the reference's. Old figures and links still draw byte for byte.
- On ours, a dropdown pick is a click on the box and a click on the option. That is counted as one action, as a select is on the reference. Panel scrolling (two mouse-wheel moves) is not counted on either side. The alpha notice's "Got it" is not counted.
- Neither side can be changed by acting on the figure itself; the reference has no on-figure actions here.
- Fixed on the way: at 1280 x 800 our preview put the map in the reference's 930 px box inside a 730 px frame, which cut off both ends of the line (ours-before/01). The map now fits the frame (ours-after/00).

Evidence: ref/00-open, 01-segment4-back, 02-times-and-acceleration. ours-before/00-open, 01-open-motion (the old four-stage controls). ours-after/00-open, 01-open-motion, 02-segment4-back, 03-times, 04-acceleration. fig-* renders: the ramp example (slows, turns, speeds back), the walk there and back (segment-end letters, origin, displacement blank), every option at Large with a long object name, and a start below zero with two turns.

Verdict: same number of actions as the reference, and finished.
