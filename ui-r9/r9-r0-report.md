# Round 9 baseline (r0) — 2026-11-07

Build: `cargo build` at commit `38d64e8` (debug). Captured into `ui-r9/shots` (60 frames).
Panel: five critics — `deepseek-v4.1-flash`, `glm-5.3-flash`, `kimi-k3`, `qwen3.8-max`, `gpt-6-luna`.
Blind set: `ui-r9/blind-r0`, 50 items (20 pairs incl. 2 controls, 4 mirrors, 28 singles); `verify_blind.py` OK.

## Honesty first

- All five judges parsed 50 of 50 items.
- Controls: 10 items, **0 split, 0.00 error on every judge** — the panel is not measuring itself.
- Mirrors: 20 groups, **0 flips**; every judge agreed with itself under a side swap (4/4 each).
- The number is usable. It is also a *different screen set* from round 8 (that round graded grade/admin/figures), so it does not replace 7.31; it is the baseline for the core screens.

## The numbers

| set | mean | bar |
|---|---|---|
| desktop light, solo screens | **6.74** | 8.0 |
| desktop light, all graded screens | **6.87** (17 screens) | 8.0 |
| dark | 6.49 | 8.0 |
| narrow | 6.43 | 7.5 |
| pairs vs Test Parrot | **64–16, 80% won** | ≥80% |

- **No screen clears 8.0. Every one fails.** Best: imports 7.50, paper-condensed 7.25, home-empty 7.17, question-editor-bank 7.17.
- **Spacing is the weak dimension almost everywhere** (desktop light 6.0–7.0, narrow 5.8). Round 8 had the same shape at 6.74.
- **Dark parity fails on 9 of 14 screens** (over 0.3): home-empty 1.08, imports 0.67, account/banks/question-editor 0.58, bank/settings/welcome 0.42.
- Pairs at exactly 80% means we beat a weak reference while still being mediocre in absolute terms — the bar is the absolute one, not the comparison.
- Judge spread is wide, as expected: gpt 8–8 on pairs, deepseek/glm 13–3, kimi/qwen 15–1. Aggregates are the mean of all five; never compare a changed panel's means.

## What the critics actually name (recurring, located)

1. **Dead vertical bands / empty floors — the single biggest defect.** Welcome (150–200 px band between the CTA and "How it works", named by all five), settings and account (content stops two-thirds down, lower-left empty grid), figure-editor (preview's bottom third empty), home-empty (bottom third empty under the dialog). This one pattern explains most of the spacing score.
2. **A disabled Save with no reason.** Settings and account: a pale ghost pill is the only action on the page, and nothing says it waits for a server address.
3. **Controls that misread.** figure-editor: "Save preset" is a dashed ghost that looks disabled next to solid pills; "Save to question…" is clipped by its own ellipsis; five equal-weight chips around the primary action drown it. Account: feature cards sit at the same elevation as the real form, so they read as settings entries.
4. **Text under overlays.** home-empty: the alpha dialog slices the onboarding card mid-sentence and readable fragments poke out on both sides.
5. **Unstyled result blocks.** figure-editor: computed momentum/energy values are a raw block of plain lines under a rule, reading like a debug dump.

## Task track

| task | state |
|---|---|
| T8 empty states | **pass** — Banks, Exams, Grade, Import, Figures each name themselves and offer a next action; friction: Grade says "make an exam first" but gives no button to get there |
| T1 first bank | partial — bank created and question editor open in 3 actions, not completed |
| T2, T3, T4, T5, T6, T7 | not measured this round |

Six unmeasured tasks alone keep the round at NOT YET.

## Verdict

**NOT YET.** Overall 6.87 against an 8.0 bar; spacing floor broken on nearly every screen; dark parity broken on nine; six tasks unmeasured.

What would move it most, in order: kill the dead bands and anchor short pages (welcome, settings, account, figure-editor), give disabled controls a stated reason, and fix figure-editor's action hierarchy. That is a visual round; the task track then needs T2–T7 recorded before any round can claim the bar.

## Frame caveats

- `shared`, `circles`, `circle-open` were captured in their "Turn on sharing" state (the capture build has no pool server), so they are excluded rather than graded as a different screen.
- `export` and `question-editor-exam` are excluded: the round-8 click that opens them no longer lands after the sidebar change.
