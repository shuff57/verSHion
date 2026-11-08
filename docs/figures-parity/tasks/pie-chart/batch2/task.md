# Interaction task: pie-chart (new figure), batch 2

Task: "Make a complete-the-pie-chart worksheet: favourite sports of 60 students, Soccer 20 and Basketball 15 given, two sectors empty for students to draw, then give me the student copy and the key."

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | about 4 (Start from an example, Edit "Complete the pie chart", retype rows, copy image) | n/a (no figure) | 5 (Start from an example, type 4 rows in one box, type total 60, label mode, Student + key) |
| finished? | yes | no | yes, by renders; the scripted Xvfb click-through was not run (alpha dialog and egui coordinates), so the UI steps are counted, not run |
| dead ends | none seen | n/a | none seen |

How: the reference edits a per-row table (name box, value box) and shows a derived value/angle/percent/fraction table in the panel; ours takes one text box (paste friendly) and shows the numbers on the chart itself. The reference picks a fill per sector with a dropdown per row; ours takes a comma list. Student copy: the reference blanks the empty sector in the same chart; ours uses Show answers off (empty sector white, number labels dropped) and Student + key saves both.

Evidence: ref/01-example-school.png, ref/02-examples-menu.png; ours-after/ (start, fruit, sam, angle, sports_key, sports_stu, blank, print, ui-opening).

Verdict: finished, a few more typed fields than the reference's example picker but no per-row table.
