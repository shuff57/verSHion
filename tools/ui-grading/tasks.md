# Task track: how a round records it

`criteria.md` says which jobs count and what makes one pass. This file is the procedure: what the
runner does, what evidence is kept, and where the verdict goes. It is deliberately separate from the
capture harness, because these are interactions, not frames.

## Evidence per task

A task is recorded as a folder `ui-rN/tasks/<id>/` holding:

- `step-01.png`, `step-02.png`, … one screenshot after every action that changes the screen;
- `log.txt` — `<n>. <action> <target> -> <observed screen/notice>`, plus actions, screens visited
  and wall-clock seconds at the end;
- `verdict.json` — `{"id":"T1","pass":true,"critical":false,"actions":9,"seconds":74,
  "screens":["welcome","home","bank"],"friction":"…"}`.

The verdict is ruled against the pass condition in `criteria.md`, from the recording, by someone who
did not build the screen. The screenshots and log are the mechanical half; the friction
sentence and the pass/fail are filled in after looking at them.

## What is driven, and what is walked

| task | where | how |
|---|---|---|
| T1 first bank | web build | walked with the fixed click script, throwaway browser profile |
| T3 sign up | web build | driven to the "check your email" screen; the mailed-link step is recorded separately (it needs an inbox the runner does not have) |
| T4 wrong input (password, link) | web build | driven; the server's answers are the evidence |
| T6 keyboard | web build | driven: Tab through the screen under test, capturing focus at each stop |
| T7 quit mid-edit | web build | driven: edit, reload, screenshot |
| T8 empty states | web build | driven: every empty list in turn |
| T2 import → exam → PDF | desktop build | walked by hand: a native file dialog cannot be driven headlessly; screenshots via the app's own `VERSHION_SHOT` |
| T4 unreadable file | desktop build | as T2 |
| T5 destructive action | desktop build | as T2, including finding the undo |

The web build is the same UI code as the desktop build, so T1/T3/T6/T7/T8 are not a different app;
they are the same screens without a file dialog.

## Running a round

```bash
# visual frames (one build, every screen; no test suite running at the same time)
UC_SEED=/tmp/vs-design-store python3 tools/ui-grading/capture-vershion.py ui-r9/shots

# tasks: walk the script below on the web build, saving one screenshot per step into
# ui-rN/tasks/<id>/step-NN.png and the action log beside it (see "Evidence per task")

# the round's gate
python3 tools/ui-grading/criteria_check.py ui-r9/r0-results.json ui-r9/tasks/tasks.json
```

A task's action count and seconds come from the log; the pass/fail is ruled from the screenshots, so
the count alone cannot pass a task.

## Known limits

- Canvas UI: the walk clicks coordinates, so a layout change can mis-target a step. A mis-click is
  recorded as a failed step and the task does not count as passed; nothing is assumed from the click
  alone, only from the screenshot after it.
- T3 cannot finish without the mailbox; the round records the steps up to the mailed link and marks
  the tail as not measured, rather than passing it by default.
- The desktop half (T2, T4-file, T5) is a walked recording, so it is only as repeatable as its
  written procedure. That is a known weakness; automating a file dialog is out of scope for now.
