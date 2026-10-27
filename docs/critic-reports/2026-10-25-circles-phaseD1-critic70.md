# Circles phase D1, product/UX critic 70 (as a non-technical teacher), round 3

Scope: the round-3 changes in the uncommitted phase D1 on `shared-pool-phase1` (HEAD 142e6d6). These are:

- the Banks-list rows for banks being added, adds waiting for sign-in, and failed adds;
- the "Wait for the send from X to finish" refusals, on the strip and on Bank settings;
- the Unlink confirm that names the unsent changes;
- the reworded send, share and Imported text;
- the claim that "pool" no longer appears anywhere a teacher can see it in circle flows.

Previous findings are in critic 68. Its minors 4 and 8-13 were skipped on purpose and are not re-reported here.

## Evidence

- **Fresh rig.** `src/share/bank_links.rs` was newer than the QA rig binary, so I rebuilt the rig instead of reusing it. I copied
  the current `src/` to `scratchpad/c70src`. `diff -rq` against the repo shows one file differs, `circle_edit.rs`. In that copy only,
  `wait_note()` returns the refusal sentence when `QA_WAIT` is set. That lets the real strip and Bank-settings drawing code render
  the refused state. The rig (`scratchpad/c70rig`) is critic 69's rig plus these modes:
  - `d-banks-results-x`: list view, plus a 5xx failure and a 403 failure;
  - `d-unlink-removed` and `d-unlink-gone`;
  - `d-send-issues`, which has four issue codes, one of them not mapped;
  - `d-send-failed500`;
  - `d-settings-hideask` and `d-settings-hidefail`.
- **Frames.** On my own Xvfb `:70`: 20 modes at 1280, 760 and 420 px, in light and dark, 120 frames in total, all exit 0. They are in
  `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c70shots/`. Contact
  sheets are next to them: `c70sheet_results_x.png`, `c70sheet_signin.png`, `c70_pair_results420.png`, `c70_pair_signin420.png`,
  `c70_strips_unlink.png`, `c70_send.png`, `c70_420trio.png`, `c70_hide.png` and `c70_misc.png`.
- **Not drawn.** The share window and the Imported tab need a signed-in Shared page, so both are judged from the code. Which
  presses trigger the wait refusal is also judged from the code (`circle_edit.rs:848-861`, `:1350`, `:1904`, `:2297`), with the
  i3 tests in `bank_links_fix3_tests.rs`.
- **Rules kept.** The only repo file changed is this report. Nothing was committed. Ports 8799 and 18791 were not touched.

## Blockers

None. Nothing I found loses unsent work or hides it. The Unlink confirm now names the count, for example "Your 3 unsent changes
can then never be sent to the circle", and it sits right under "Send my changes...". A refused screen never takes over another
bank's send in progress.

## Important

**I1. "Pool" still reaches teachers on circle screens, and the new blanket swap puts the server's faults on "the circle".** The
round-3 claim of no "pool" anywhere a teacher can see it in circle flows does not hold. This is the same harm critic 66 rated I7: a
teacher reads "pool" and thinks their private circle bank is going public.

1. **The most common case is in the frame `d-settings-hideask-*`.** An owner who ticks "Hide answer keys from viewers" on Bank
   settings is told "The pool first checks the viewers' copy..." and "...also when the pool asks for decisions..."
   (`circles_text.rs:58`, `:136`). `HIDE_TRIES` also appears on the send page's decisions step
   (`circle_hide_ui.rs:166` via `circle_edit_ui.rs:221`), and round 3 called the send page pool-free.
2. **Other constants on circle screens still say "pool":**
   - `HELD_BANK` (`:40`), "the pool's reviewers", on the circle row and in Bank settings;
   - `PICTURES_CHANGE_DECIDE` (`:79`) and `circles_ui.rs:1886`;
   - the held-back result (`circles_text.rs:364`);
   - `REPORT_THANKS` (`:34`);
   - `EMAIL_NEEDS_ACCOUNT` (`:44`);
   - `EMPTY_BANKS_HELP_EDITOR` (`:15`).
3. **Problems on the Shared page's circle screens are shown unchanged.** `circles_ui::problem_box`/`warn_box` print
   `p.message` as it comes, so these texts appear as written:
   - `circles.rs:1322`, `:1352-1355`, `:1889`, `:1891`, `:1936`;
   - every generic text in `pool.rs`. Frame `d-settings-hidefail-*` shows "The pool had a problem of its own.", and there are
     daily-limit texts such as "...answer-key settings as many times as the pool allows today" and "...emailed as many circle
     invitations today as the pool allows".
4. **Where round 3 did swap the word, `circle_words` turns "the pool" into "the circle".** For server faults and account limits
   that tells a teacher something false about their group of colleagues:
   - frame `d-banks-results-x-*`: "The circle had a problem of its own." and "The circle does not allow that for this account."
     The second reads as if the owner blocked them;
   - frame `d-send-failed500-*`: "The circle had a problem of its own.";
   - frame `d-send-issues-*`: "this question is in a form the circle does not take", directly under "verSHion checked these
     questions for the circle".
5. **One server error now has three wordings.** A 5xx reads "The pool..." on Bank settings and "The circle..." on the send page
   and in the Banks rows. `circle_edit.rs` already says "verSHion's server is being upgraded" and "...as verSHion allows".

Fix, all cheap:
- Change `circle_words` to map "The pool" to "verSHion's server", "the pool's" to "verSHion's", and "the pool" to "verSHion".
- Run it inside `circles_ui::warn_box` (or `problem_box`) for circle problems.
- Reword the six constants listed in item 2.
- Leave the share window's destination radio "The public pool: ..." as it is: it names the real alternative. Hiding that radio
  when the window is opened from a circle's "Share one of my banks here..." would be safer, but that is the owner's call.

## Brief checklist

| Item | Verdict |
|---|---|
| Banks-list rows: adding / sign-in / failed / not connected | Done and readable at every width and in both themes. Retryable failures have "Open the circle"; others say no "try again". Not-connected rows have "Open it". Sign-in rows have "Sign in..." and no Dismiss. The other-account failure is plain. Their wording carries I1.4. |
| "Wait for the send from X" on the strip | Shown under the strip's sentence, above the still-live buttons, legible in dark mode at 420 px. It clears itself when the send ends. Acceptable. |
| "Wait for the send from X" on Bank settings | Shown under the History card. It reads as loose text between two cards, but it is next to the button that was refused. Acceptable. |
| Unlink confirm with unsent count | Clear. The count matches the strip ("3"), with a follow-copy variant and an uncountable variant. |
| Send / share / Imported rewording | Empty-issues case: one sentence, fixed. The share chooser says "share with the circle". Share result: "add it to My Banks or make their own copy". Licence line: "Bank settings on the circle's page". Imported linked rows: "Open in My Banks" now matches the line. All fixed. Issue rows carry I1.4. |
| No "pool" in circle flows (grep) | Not met: see I1. |

## What works

- The Banks page now reports adds that ran in the background honestly, and every row's button does what its sentence says.
- The wait refusal never silently drops another bank's send in progress, and it always says why nothing opened.
- Unlink can no longer strand changes without saying how many, and "Send my changes..." is one row above the confirm.
- No regressions: the removed, closed, circle-page and viewer frames match critic 68's.
