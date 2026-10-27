# Circles phase D1, product/UX critic 80 (as a non-technical teacher), round 8

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 8. Round 8 changed:

- the invite wording on circle screens ("Your verSHion account needs an invite code from an administrator before it
  can join circles. This is not the circle's code. ...") and the email-invite line ("...that has redeemed its account
  invite code. Until then the circle's code in the mail can not be used.");
- "Circles are not available on this server yet.";
- what the strip says once the check after a send with no clear answer comes back ("The check found a newer version
  in the circle, version N. It may be the one you sent: press Get latest to see." / "Your send did not go through:
  the circle still has version N. ...");
- the merge review matching a question new in the circle to the same unlinked question in the bank ("You and the
  circle added the same question, so it is linked to the circle's, not added twice.");
- the Send page line after an earlier send whose answer could not be read ("Your earlier send went through, so these
  counts still include its changes until you get the latest. ...") with Get latest on the page;
- the conflict box and strip line when the newer version may be the teacher's own earlier send ("It may be your
  earlier send").

Checked here: is the wording clear to a teacher; do the strip, the badge, the Send page and the Get latest page
contradict each other; do the buttons match the text; is any "pool" left on a circle screen. Known exceptions, not
re-reported: the Shared page's account guide and the Account window (they name the shared pool on purpose). Only
blockers and important findings are reported. A fresh critic who built none of this wrote this report.

## Evidence

- **Fresh copy and rig.**
  - `scratchpad/c80`: the repo's `src/share/` and `src/bankui.rs` as they are now, on the compiling base of the other
    files (the other session's `ai.rs`, `app.rs`, `lib.rs`, `model.rs`, `screens.rs`, `store.rs`, `figures/mod.rs`
    are the earlier versions there; `gradeui.rs` is left out). Nothing is changed in `src/share/` except one added
    module, `qa_c80.rs` (my own rig, written for this round), and its `mod` line.
  - The rig draws the real `bank_links_ui::strip_with` (and so the real Send, Get latest and History pages), with a
    Banks-list card above it drawn by the real `BankLinks::badge` and `badge_line`, so the badge and the strip are in
    the same frame. The circle screens are the real `circles_ui::body_of`. Every state is reached through the real
    models with a scripted session: a send that times out after it left, the real check that follows (heads answering
    version 3 or 4, by the teacher or by "Bob Teacher"), an answer with no readable version (version 0), 409s naming
    the teacher or a colleague, the real merge review and Apply merge, invite and not-configured refusals on join,
    list and create, and the email form before and after sending.
  - 32 modes: `moved-*`, `still-*`, `moved-change-merge`, `moved-edited-*`, `moved-colleague*`, `earlier-*`,
    `maybe-conflict*`, `plain-conflict`, `join-*`, `list-*`, `create-invite`, `email-*`.
- **Frames.** On my own Xvfb `:80` (stopped when done): 29 modes at 1280, 760 and 420 px and 3 follow-up modes at
  1280 and 420 px, light and dark, 186 frames. All exit 0; none blank (`c80tools/crop.py` checks). Frames in
  `scratchpad/c80shots/` (cropped in `c80crop/`); contact sheets `c80_420a.png`, `c80_420b.png`, `c80_done.png`,
  `c80_editsend.png`.
- **Suite.** `cargo test --lib share::` on the copy: 956 passed, 0 failed.
- **Rules kept.** The only repo file changed is this report. Nothing was committed. Port 8799 was not used. The rig on
  18791 (workerd pid 478730) was left running.

## Blockers

None. No path loses unsent work: every "not sent", "may have gone through" and "did not go through" line keeps the
changes on this computer and counted, with Send or Get latest beside it, and every merge saves a restore point first.

## Important

### I1. A question added by a send whose answer was lost, then corrected before Get latest, comes back as "New in the circle" and ends up twice in the bank and twice in the circle

Frames `moved-edited-merge-*`, `moved-edited-merged-*`, `moved-edited-done-*`, `moved-edited-send-*`,
`earlier-add-merge-*`.

The teacher adds "Solve 5x = 20" and sends. The send times out after it left, and it did go through as version 4. The
strip checks and says, correctly:

> Shared with Algebra team. Version 4 is the latest. You have version 3 and 1 unsent change. Get the latest first,
> then send.
> The check found a newer version in the circle, version 4. It may be the one you sent: press Get latest to see.
> Your changes are still on this computer.

Before pressing Get latest the teacher fixes the new question to "Solve 5x = 25" (the note just told them their changes
are safe here; fixing a typo in a question just added is the most ordinary edit there is). Get latest then shows:

> What was sent since: Version 4, by Olga Teacher, 1 added ...
> 1 new in the circle - 1 unchanged
> 1 question in this bank is yours alone (not in the circle's bank yet), so the merge leaves it as it is.
> **New in the circle: Solve 5x = 20** (•) Add it to my bank ( ) Do not add it

Round 8's match needs the same content, so it misses the corrected question. Apply merge with the default leaves the
bank with both "Solve 5x = 25" and "Solve 5x = 20". The strip says "1 unsent change" with Send as the main button, and
the Send page says "1 added". Sending puts the corrected question into the circle next to the uncorrected one, where
colleagues see both. The same happens after a send whose answer could not be read (round 6's "Press Get latest to
catch up", `earlier-add-merge`).

The changed-question case shows the same root (`earlier-merge`). The teacher's sent change (13) and later change (15)
come up as **"CONFLICT: both changed ... In version 4 (the circle's newest): 13"**, with no decision made. The strip
just said "Your earlier changes were sent. Changes you made since are not sent yet." The page never says that version
4 is the teacher's own send. "Use the circle's version" would throw away the later edit (a restore point exists).

**Why it matters.** This is the path the strip recommends on flaky school wifi. The merge page presents the teacher's
own earlier send as someone else's work, so they cannot tell that "Solve 5x = 20" is their own question in its old
wording. The default answer duplicates it. Nothing is lost, but the bank and then the shared circle bank get stale
duplicates.

**Fix.**
- (main) When a send gets no clear answer, or an answer with no version, keep a small record with the link: for each
  question it sent without a circle id, the local id and the content key that `same_result` compares.
- In `match_new_to_unlinked`, also match a circle question to the unlinked local question whose sent content equals
  it. When that question changed since, the row becomes "changed only by you" (kept, now linked to the circle's
  question), never "New in the circle".
- For a question the send changed, a circle version equal to what this bank sent is the base, not a conflict, so the
  later edit is "changed only by you".
- Drop the record on a normal send, on a merge, and on unlink.
- Alternative: critic 78's "better still". Keep the commit id and body of that send so that Send replays it. The server
  gives back the first answer with its qids, and the later edits are then ordinary unsent changes.

**Test:**
- A send that times out but made version 4 with one added question, then an edit to that question, then the check,
  then Get latest: no "New in the circle" row, the question appears once, and it counts as 1 changed (not added).
- The same after a version-0 answer.
- A question changed twice around such a send is never a conflict.

### I2. "It may be your earlier send" printed above "Bob Teacher sent version 4", after the strip already said the send did not go through

Frames `maybe-conflict-*`, `maybe-conflict-strip-*`, `still-strip-*`.

The send timed out. The check found the circle still at version 3, and the strip said so, plainly:

> Your send did not go through: the circle still has version 3. Your changes are still on this computer; press Send
> my changes to send them.

Meanwhile a colleague sends version 4. The teacher presses Send my changes and meets the 409. The Send page then
reads:

> **The circle has a newer version than your copy, so nothing was sent and nothing was overwritten. It may be your
> earlier send.**
> Bob Teacher sent version 4, 1 minute ago.

After Back, the strip reads "Your changes were not sent: the circle has version 4, which may be your earlier send."

The teacher is Olga. One screen says it may be her send and, in the next line, that Bob sent it. The strip earlier
said her send did not go through. The cause is that `checked()` clears `unclear_noted` but not `maybe_sent`, so "may
be your earlier send" outlives the app's own "did not go through", and the conflict wording never looks at `head_by`.
The same contradiction appears without the check (the teacher offline until after Bob's send), because the 409 names
Bob either way.

**Why it matters.** This is the wording round 8 introduced to stop "three screens telling three stories" (critic 78
I2), and on this path it tells two stories on one screen. The buttons are right (Get latest), so this is
confusion, not loss.

**Fix (cheap).**
- In `checked()`, when the check found the version the send was based on (the "did not go through" note), also remove
  the bank from `maybe_sent`. The app has told the teacher the send did not go through, so a later conflict is someone
  else's.
- When the 409, or the check, names a sender (`head_by`, or the heads `by`) who is not the teacher, use the colleague
  wording ("Someone else sent changes first ...", "someone sent version N first"). To tell who the teacher is, use the
  circle's member marked `is_me`, or the account's display name. Keep "may be your earlier send" only when the sender
  is the teacher or is not named.
- The same `by` can firm up the moved note: "Bob Teacher sent version 4; your send did not go through." instead of
  "It may be the one you sent."

**Test:**
- An unclear send, a check answering the same version, then a 409 naming another member: neither the page nor the
  strip says "your earlier send".
- An unclear send with no check, then a 409 naming another member: the same.
- A 409 naming the teacher keeps the own-send wording.

## Brief checklist

| Item | Verdict |
|---|---|
| Invite wording (join) | **Holds.** Under "Found a code: C-7KQ2-...": "Your verSHion account needs an invite code from an administrator before it can join circles. This is not the circle's code. Open the Account window, find "Invite code" and enter it. Everything else in verSHion keeps working." [Open account...] (`join-invite-*`, every width, both themes). A teacher now knows the circle's code is fine and which other code is meant. Not reported (polish): the same sentence on the Circles list and on Create a circle (`list-invite`, `create-invite`) says "join circles" and "This is not the circle's code" where no circle code was entered. "...before it can use circles. The code it needs is not a circle's code." would fit all three. |
| Email-invite text | **Holds.** On the form and after "Invitation sent to p***@school.org": "To join, they need a verSHion account that an administrator has approved and that has redeemed its account invite code. Until then the circle's code in the mail can not be used." No loop now: the two codes are named apart. ("redeemed" is the only jargon left.) |
| "Circles are not available on this server yet." | **Holds** (`join-notconf`, `list-notconf`): "Circles are not available on this server yet. Nothing you can change; everything else works." |
| After-check notes | **Hold when the send did go and the bank was not touched since** (`moved-strip`, `moved-merge`, `moved-merged-done`: Get latest links the question, the strip ends "Your copy is up to date.", the note is gone) and **when nothing moved** (`still-strip`: "Your send did not go through ... press Send my changes" over a primary Send my changes; `still-send`: the note for the history is kept, 1 added). A colleague's version 4 (`moved-colleague*`) reads honestly: the review names "Version 4, by Bob Teacher" and the strip then offers Send. **Not met** for a bank edited before Get latest (I1) and for a later conflict (I2). Not reported: with the Send page left open through the check, the page keeps "Press Try again", which is the right button anyway. |
| Merge note "You and the circle added the same question..." | **Holds** where it applies (`moved-merge`): it explains why the "new" question is not offered. It does not apply after an edit (I1). Polish only: the review prints "2 unchanged" twice (summary and footer), and the result box then reads "0 taken from the circle, 0 kept as yours, 0 added, 0 removed" (a "1 linked" would read better). |
| Send page "counts still include its changes" | **Holds** (`earlier-send-*`): the line sits in the warning box under the counts, with Get latest right under it, and the strip above says the same in its own words. The page's Send stays available; if pressed, the 409 says "Your earlier send went through, so the circle is ahead of your copy ... Olga Teacher sent version 4" (`earlier-conflict`), which matches. |
| Conflict wording "It may be your earlier send" | **Holds** when the sender is the teacher or not named. **Contradicts itself** when the 409 names a colleague (I2). The plain colleague conflict (`plain-conflict`) still says "Someone else sent changes first". |
| Strip vs badge vs Send page vs Get latest page | **Agree** in every mode for counts and next step: "newer version · 1 unsent" / "Get the latest first, then send"; "newer version · unsent changes" / "Your earlier changes were sent. Changes you made since are not sent yet."; "1 unsent change" / Send; nothing / "up to date". **The Get latest page breaks the story** in I1: it calls the teacher's own earlier send "new in the circle" or a conflict. Not reported, as before: the unread note says "the server's answer" where the page says "the circle's answer". |
| Buttons match the text | **Hold.** Every note that names a button has it on the strip or page next to it (Get latest, Send my changes..., Try again, Open account...). |
| Layout | Every new line wraps cleanly at 420 px in both themes. The badge keeps its state and cuts the circle name first. Dismiss sits after the note, and no button is pushed off-screen. |
| Remaining "pool" | **None on circle screens.** A grep of the string literals in `circles_ui.rs`, `circle_edit_ui.rs`, `bank_links*.rs`, `circle_edit.rs`, `circles.rs`, `circles_text.rs`, `circle_hide*.rs`, `circle_send.rs`, `circle_links.rs` and `act_results.rs` finds only the true "items from the public pool" credit line and the `circle_words` / `merge_words` replacement tables. None of the frames shows "pool". |

## What works

- After a send with no clear answer, the strip now says what the check found. The usual case ends at "Your copy is up
  to date" after one Get latest, with the added question once.
- "Your send did not go through ... press Send my changes" sits over a primary Send my changes, and the note for the
  history is kept for the retry.
- The two codes, the circle's and the account's, are named apart on the join refusal and in the email form.
- The Send page no longer counts an earlier send's changes as new without saying so, and the conflict that follows
  no longer blames "someone else" for the teacher's own version.
