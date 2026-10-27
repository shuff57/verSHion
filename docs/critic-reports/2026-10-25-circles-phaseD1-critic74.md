# Circles phase D1, product/UX critic 74 (as a non-technical teacher), round 5

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 5. Round 5 added a list of
results per bank, each with Dismiss (`act_results.rs`). It also changed four pieces of wording:

- the merge notes now say the circle removed a question (`merge_words`), not "verSHion's server";
- `NEW_BANK_NOTE` has new wording;
- the strip no longer says "up to date" when that is false;
- jobs are pinned to the account that started them.

Checked here:

- the result lines and Dismiss on strips, Bank settings, History and the Banks list;
- the wording of the Get latest (merge) page;
- the Banks list rows;
- the share page text;
- any "pool" left.

Known exceptions, not re-reported: `EMAIL_NEEDS_ACCOUNT`, the "shared pool" invite-code errors, and the Shared page's
account guide (critic 72 put it with these). A fresh critic who built none of this wrote this report.

## Evidence

- **Fresh copy and rig.**
  - `scratchpad/c74src` is a copy of the current tree. It has two changes:
    - `gradeui.rs` is replaced by the last version that compiled. Another session is part-way through editing it.
    - The viSHion path dependencies are made absolute.
  - `diff -rq` of `src/share` and `src/bankui.rs` against the repo shows three differences only:
    - `circle_edit.rs` has QA hooks added at its end (`qa_act`, `qa_rolled`, `qa_trimmed`, `qa_review`, and a new
      `qa_send_unread`). They call the real handlers, or `say()` with the code's own sentence;
    - `mod.rs` registers the probe file;
    - the probe file, `c74_probe_tests.rs`.
  - The rig is `scratchpad/c74rig`: round 5's rig plus these modes:
    - `y-send-unread`;
    - `y-merge-applyerr` (the merge's own error, through `merge_words`);
    - `y-merge-uptodate` and `y-merge-withheld`;
    - `x-banks-r5-list`.
- **Frames.** On my own Xvfb `:74` (now stopped): 34 modes at 1280, 760 and 420 px, light and dark, 204 frames in total.
  All exit 0, and all are readable and not blank. They are in `scratchpad/c74shots/`. Contact sheets are in the
  scratchpad: `c74_strips1280.png`, `c74_strips1280b.png`, `c74_420strips.png`, `c74_banks420.png`, `c74_bankslist.png`,
  `c74_merge2.png`, `c74_merge3.png`, `c74_pages.png` and `c74_420pages.png`.
- **Probes** (`c74_probe_tests.rs`, scripted sessions; both pass):
  - p1: a send whose answer has no readable version;
  - p2: real pointer clicks on Dismiss, on a strip and on a Banks-list row.
- **Not drawn.** The share window needs a signed-in Shared page, so it was judged from the code.
- **Rules kept.** The only repo file changed is this report. Nothing was committed. Port 8799 was not used. The rig on
  18791 (workerd pid 274528) was left running.

## Blockers

None. Nothing loses unsent work. No strip says "up to date" while a restore or trim is on its way, after one lands, or
after one fails.

## Important

### I1 (rare path, cheap). After a send whose answer could not be read, the screens say nothing was sent, and point to a button that is not there

Probe p1 runs `two_at_three`, then one change, then a Send that the server answers with 201 but no readable version
(the case round 5 worded in `finish_send`).

**The Send page, still open, reads:**

> The version was sent, but the server's answer could not be read. Press Get latest to catch up. [Dismiss]
>
> Sent
>
> Version 0 is now the circle's newest. 0 added, 0 changed, 0 removed. [Done]

`sent_lines` (`circle_edit.rs:3254`) has no case for `version == 0`. A teacher reads "0 added, 0 changed" right under
"was sent" and concludes that nothing went.

**The strip, for up to 10 minutes** (frame `y-send-unread-*` and p1 at +0 and +1 min):

> Shared with Algebra team. Version 3 is the latest. You have 1 unsent change.
>
> The version was sent, but the server's answer could not be read. Press Get latest to catch up. [Dismiss]
>
> [Send my changes...] [History] [...]

Three things are wrong here:

- There is no Get latest button: the not-behind state offers Send and History only.
- The version just sent is not mentioned.
- The change that was sent is counted as unsent. The Banks-list badge says the same.

The obvious next press is the primary "Send my changes...". That meets a 409 and then the merge. No work is lost,
but the teacher has to recover without knowing why.

Only the 10-minute check (p1 at +11 min) changes the line to "Version 4 is the latest. You have version 3 and 1 unsent
change. Get the latest first" and shows a Get latest button. "Check now" in "..." does the same, but nothing says to
use it.

**Fix (all cheap):**

- In `sent_lines`, when the version is 0, write "Your changes were sent, but the circle's answer could not be read.
  Press Get latest to catch up." in place of the "Version 0 ..." line.
- When this result is said, arm a check for the bank (`links.check_now(bank)`), so that the strip learns the new head
  and offers Get latest at once.
- Or show Get latest on the strip while a Send result of this kind is listed for the bank.

**Test:** p1. Right after the send, the strip has a Get latest button, and neither screen says "Version 0".

## Brief checklist

| Item | Verdict |
|---|---|
| Result lines + Dismiss on the strip | **Hold.** Each result sits under the strip's line with its own Dismiss, above the buttons, at every width and in both themes. Two results on one bank stack (`r-two-results`). Another bank's wait note stays separate and has no Dismiss (`x-notes-both`). p2: a real click on Dismiss removes the result (1 -> 0) and the strip redraws without it. The restore-failure line says "try again" twice ("...try again in a minute. Open History to try again."). That is clumsy but clear. |
| Results on History, Bank settings, Send | **Hold.** The screen's own bank only, with Dismiss. Bank settings shows the restore failure under "Trimming the bank's old versions in the circle..." without either hiding the other. One small oddity: on History itself, the line still says "Open History to try again". |
| Banks list rows | **Hold.** Adds being made, sign-in, failed, "not connected" and other-account rows are as critic 72 found them. The new bankless result row has Dismiss, and p2's click removes it. The rig frame `x-banks-r5` shows the row without a bank name, but that comes from the rig's hook (no History was open when it started). The real path, in p2, says `"Inequalities exit tickets": Version 3 was restored from version 1.`. Rows wrap at 420 px. |
| No false "up to date" | **Holds** for restore, trim, held restore and failures. After a restore lands, the line is "Version 8 is the latest. You have version 7." with Get latest as the main button. A failure keeps "up to date", which is true because nothing changed. The false claim left is I1's "unsent". |
| Get latest / merge page wording | **Fixed.** `x-merge-notes-*` shows "Removed from the circle" rows reading "The circle removed this question. It is kept unless you remove it. Kept questions are no longer linked to the circle." and "...You changed it, so it is kept unless you remove it." The apply error now reads "The circle's version changed while you were reviewing. Nothing was applied; start the merge again." A 500 reads "verSHion's server had a problem of its own. Nothing was changed; try again in a minute." The up-to-date, withheld and stale pages are plain. A Keep-both copy's "From the pool:" topic is rewritten to "From the circle:" (`circle_edit.rs:2635`). |
| Share page text | **Fixed (from the code).** `NEW_BANK_NOTE` now says "This adds a new bank to the circle and links this bank to it: you send later changes from its page in My Banks." That matches the done page's "...is now linked to it". |
| Remaining "pool" | **One new spot (minor, cheap).** In `publish_ui::title`, `Step::Gate` is always "Share to the pool", even when the window was opened from a circle's "Share one of my banks here..." (`start_for_circle`). The window passes through the gate quickly when the person is signed in. If they are signed out or not approved, or the session drops part-way, a teacher sharing to a private circle sees "Share to the pool" as the heading. Fix: `Step::Gate if model.circle.active => "Share to a circle"`. Every other literal on a circle screen is either mapped by `circle_words` / `merge_words` or names the real public pool ("built on items from the public pool", the destination radio). The bank page's "From pool..." and "Update from pool..." are the owner's public-import items. |

## What works

- A result is said on its own bank, stays until dismissed, and Dismiss really works.
- The merge review blames neither the server nor the colleagues wrongly.
- No screen claims "up to date" while the circle may be changing.
