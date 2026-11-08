# UI hill-climb criteria (round 9 onward)

Two tracks, one verdict. The visual track is the blind A/B panel that produced round 8's 7.31; its
four dimensions are **unchanged on purpose**, so a new round's numbers stay comparable with that
one. The task track is new: screenshots cannot show whether a teacher can actually finish a job, so
eight real jobs are measured alongside the panel.

Everything here is a criterion *to meet*, not a description of the app. `criteria_check.py` turns
`results.json` (+ an optional `tasks.json`) into PASS/FAIL against these numbers.

## Track 1 — visual (blind panel, unchanged rubric)

Each graded frame is scored 0–10 on four dimensions by five independent critics, blind, with the
same prompt and the same controls/mirrors as before (`critic.md`, `make_blind.py`,
`verify_blind.py`, `analyze.py`).

| dimension | question |
|---|---|
| hierarchy | can you tell at once what matters most, what comes next, and what to do? |
| spacing | alignment, rhythm, consistency, density, clipping, overlap, wasted space |
| type | size scale, weights, readability, line length, truncation |
| state | are selected, active, disabled, empty, saved and error states legible? is it clear what is interactive? |

Graded frames: every screen in `capture-vershion.py` — welcome, home (empty and seeded), banks,
bank, question editor (bank and exam), exams, exam, the four paper styles, export, imports,
figures, figure editor, settings, shared, circles, circle open, account — in light and dark, plus
the narrow (760 px) pass. Grade screen and admin screens come in through `VS_EXTRA` once they have
fixtures (see `tools/ui-grading/fixtures`).

## Track 2 — task (new)

Eight jobs a teacher actually does. Each is executed the same way every round (procedure below),
recorded as screenshots at every step plus an action log, and ruled pass/fail against a stated
condition. A task is not "passed" because it looked fine: it passes when the condition holds in the
recording.

| id | job | passes when |
|---|---|---|
| T1 | first bank, cold start | a bank exists with 3 questions in ≤ 12 actions and no screen that has to be guessed |
| T2 | import → exam → PDF | a text file becomes questions, an exam is laid out and a PDF exported, on the first attempt, with no detour through settings |
| T3 | sign up | email → mailed link → password + name + school + staff page → a pending screen that says what happens next, in ≤ 6 screens and with the staff-page link accepted |
| T4 | wrong input | a bad password, a bad link and an unreadable file each say what is wrong, and typed work survives the error |
| T5 | destructive action | deleting a bank or exam asks first, the safe choice is the default, and nothing is lost without an undo the person can find |
| T6 | keyboard only | every control on the screen is reachable by Tab, focus is visible at each step, Enter submits and Esc cancels |
| T7 | quit mid-edit | work in progress survives a reload/relaunch, and the app says so rather than silently restoring |
| T8 | empty states | every empty list names itself, states why it is empty, and offers the next action |

Every task records: pass/fail, actions taken, screens visited, wall-clock seconds, and the one
biggest friction (a specific step, one sentence). "Critical friction" = a step where a teacher
would stop and have to ask someone; any task with one fails even if it eventually completes.

T1, T3, T4 (link/password half), T6, T7 and T8 are walked on the web build from a fixed script of
clicks and keystrokes, with a screenshot after every step. T2, T4 (file half) and T5's undo half are
walked on the desktop build, because a native file dialog cannot be driven headlessly. The recording
and its verdict are attached to the round; automation of the stable web half is the next step, and
until it exists no round may claim a task from memory, only from its recording.

## The bar

Both tracks must hold for a round to count as met:

1. **Every graded screen ≥ 8.0** on its trimmed mean (round 8 bar was 7.5; the app sat at 7.31).
2. **No dimension below 7.0** on any screen. A screen cannot pass on the strength of hierarchy while
   spacing sits at 6.1 — that was round 8's shape.
3. **≥ 80% of reference pairs won**, and no screen loses more pairs than it wins.
4. **Narrow ≥ 7.5**, and no screen passes desktop while failing narrow.
5. **Dark within 0.3 of light** on the same screen.
6. **All eight tasks pass**, none with critical friction.

The first round that satisfies all six is the claim. Until then a round reports the numbers and the
named gaps, exactly as before — the bar is the target, not a formality.

## Honesty rules (unchanged)

- The builder never grades its own work.
- Controls first: a round with a split control is void.
- Same panel across rounds; a changed panel resets the scale and is reported as such.
- One build per round; every frame from the same binary.
- Fix what the critics name, not what the builder likes.
- Never trade a passing test for a look: build, run the suite, then shoot.
