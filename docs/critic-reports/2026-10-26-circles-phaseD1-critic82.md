# Circles phase D1, product/UX critic 82 (as a non-technical teacher), round 9

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 9. Round 9 changed:

- after a send with no clear answer, the strip decides "did not go through" by the version the send was based on (kept
  with the link as `sent_record`, also across a restart), not by whether the check moved the newest version;
- "The check found version N in the circle, sent by you: your send went through. Press Get latest to catch up.";
- "<Colleague> sent version N first, so your send did not go through. ..." and the colleague wording on a later 409;
- the merge review: "Version N is your own send from this computer ...", questions that send carried matched to the
  bank even when corrected since ("You sent this in version N and changed it since ..."), compared as "Yours now" vs
  "In version N (the circle's newest)", never under the old version's number;
- "N unchanged" said once;
- the invite sentence "... before it can use circles. The code it needs is not a circle's code. ...";
- a retried send keeps its note and viewers'-copy decisions.

Checked here: is the wording clear to a teacher; do the strip, the badge, the Send page, the Get latest page and the
conflict box contradict each other; do the buttons match the text; is any "pool" left on a circle screen. Known
exceptions, not re-reported: the Shared page's account guide and the Account window. Only blockers and important
findings are reported. A fresh critic who built none of this wrote this report.

## Evidence

- **Fresh copy and rig.**
  - `scratchpad/c82`: the repo's `src/share/` and `src/bankui.rs` as they are now, on the compiling base of the other
    files that critic 80 used (the other session's `ai.rs`, `app.rs`, `lib.rs`, `model.rs`, `screens.rs`, `store.rs`,
    `figures/mod.rs` are the earlier versions; `gradeui.rs` left out). The only change in `src/share/` is one added
    module, `qa_c82.rs`, and its `mod` line.
  - The rig is critic 80's (`qa_c80.rs`) with the account's display name supplied ("Olga Teacher", as `snapshot_of`
    fills it) and 46 new round-9 modes. It draws the real `bank_links_ui::strip_with` (and so the real Send, Get latest
    and History pages) under a Banks-list card drawn by the real `BankLinks::badge` and `badge_line`; circle screens
    are the real `circles_ui::body_of`. Every state is reached through the real models with a scripted session: a send
    that times out after it left; the check answering version 3 (still), 4 by the teacher (went), 4 by "Bob Teacher"
    (a colleague first), 5 by Bob (two versions since), or failing (503); a later 409 naming the teacher or Bob; the
    page's Try again answered 201 (replayed) or 409; edits before Get latest (an added question corrected, a changed
    question changed again); an answer with no readable version then edits; the real merge review and Apply merge; a
    restart (a new edit model loaded from the same links) before the check; the invite refusals and the email form.
- **Frames.** On my own Xvfb `:82`: 51 modes at 1280, 760 and 420 px, light and dark, 306 frames. All exit 0; none
  blank (`c82tools/crop.py`). Frames in `scratchpad/c82shots/` (cropped in `c82crop/`); contact sheets
  `c82_s1.png` ... `c82_s18.png`, `c82_420a.png`, `c82_420b.png`, `c82_760.png`.
- **Suite.** `cargo test --lib share::` on the copy: 998 passed, 0 failed.
- **Rules kept.** The only repo file changed is this report. Nothing was committed. Port 8799 was not used. The rig on
  18791 (workerd pid 478730) was left running.

## Blockers

None. No path loses unsent work. Every "did not go through", "went through" and "may be the one you sent" line keeps
the changes on this computer and counted, with Get latest or Send beside it; every merge saves a restore point first.

## Important

### I1. After a failed check, the strip keeps "Couldn't check the circle just now" when the circle has since answered; on a conflict it says "Get the latest" over a lone Try again

Frames `r9-nocheck-retry-ok-*`, `r9-nocheck-retry-ok-page-*`, `r9-nocheck-retry-409-*`, `r9-nocheck-conflict-strip-*`.

This is the flaky-wifi path rounds 7 to 9 are about. The send times out. The check asked for at once fails too (same
wifi). The Send page, still open, says "Your send may have gone through ... Press Try again". The teacher presses Try
again a minute later, and the circle answers:

- **It had gone through** (201, replayed). The page says, correctly: "Sent. Version 4 is now the circle's newest. ...
  This send had already gone through; nothing was sent twice." [Done]. The badge is clean. Back on the bank, the strip
  says:
  > Couldn't check the circle just now. Your bank works as usual. **[Try again]** [...]
- **A colleague got in first** (409 naming Bob, version 4). The page is right ("Someone else sent changes first ...
  Bob Teacher sent version 4"). After Back, the badge says "newer version · 1 unsent" and the strip says:
  > Couldn't check the circle just now. Your bank works as usual.
  > Your changes were not sent: someone sent version 4 first. Get the latest, then send them. [Dismiss]
  > **[Try again]** [...]

  The note tells the teacher to get the latest. There is no Get latest button. The status line says the circle could
  not be reached, right after the circle answered.

The cause: `BankLinks.state` stays `Check::Offline` from the failed check until the 10-minute throttle runs out or
Try again is pressed. A send's answer pushes `check_asks` only when it is unclear, refused by role, or a 409 without
`head_version` (`circle_edit.rs` around 2556 and 2741), so a 201 or a 409 naming the version leaves the stale state.
Pressing Try again does fix it (the next check shows Get latest), so this is confusion, not loss. But the text and the
button disagree on the recovery path, and the strip contradicts both the badge and the page the teacher just read.

**Fix (cheap).**
- Any answer from the circle for the bank (a send's 201, any 409, a merge applied) clears `Check::Offline` for it: push
  the bank to `check_asks` there as well, or drop the state when `act_line` comes from an answer newer than the failed
  check.
- Or, in `strip_view`, while the state is Offline and the act line names Get latest (or Send), offer that button
  beside Try again.

**Test:**
- An unclear send, a check that fails, then the page's Try again answered 201: the strip after Done is not "Couldn't
  check the circle just now".
- The same with a 409 naming version 4: the strip offers Get latest.

## Brief checklist

| Item | Verdict |
|---|---|
| Strip decides by the version the send was based on | **Holds** in every case played: still at 3 (`r9-still-*`: "Your send did not go through: the circle still has version 3 ... press Send my changes" over a primary Send my changes); 4 by the teacher (`r9-mine-*`); 4 by Bob (`r9-other-*`); 5 by Bob (`r9-far-*`); and after a restart (`r9-restart-*`: "Your changes were sent. Get the latest ...", or "Version 4 is the latest. You have version 3 and 1 unsent change" for Bob's). Not reported (minor): with 5 by Bob, the note says "version 5. It may be the one you sent". A send based on 3 could only have made 4, and the heads named Bob for 5. "Bob Teacher sent version 5; your send may be version 4. Press Get latest to see." would be exact. |
| "Sent by you, your send went through" | **Holds, and agrees everywhere.** The strip line ("Your changes were sent. Get the latest so that this copy has the version they made.") and note, the badge ("newer version", no unsent count), the Send page ("Your earlier send went through, so these counts still include its changes ... Get latest" with Get latest on the page), the 409 ("Your earlier send went through, so the circle is ahead of your copy ... Olga Teacher sent version 4") and the merge ("Version 4 is your own send"). After Apply merge: "Your copy is up to date." Not reported: the strip says the point twice; a page left open through the check (`r9-mine-page`, `r9-other-page`) still says "may have gone through ... Press Try again". Try again gives the right result either way (a replay, or the colleague 409). |
| Colleague-sent-next naming | **Holds.** "Bob Teacher sent version 4 first, so your send did not go through. Your changes are still on this computer: get the latest, then send them." over a primary Get latest. Send then meets "Someone else sent changes first ..." beside "Bob Teacher sent version 4". Critic 80 I2 is gone: no "It may be your earlier send" beside a colleague's name, with or without the check (`r9-still-conflict*`, `r9-other-conflict`, `r9-nocheck-conflict`). It is left only when the display name is unknown (`r9-noname-conflict`). Not reported: `snapshot_of` gives a name whenever the gate is Ready, and sign-up requires one. Minor risk, not reported: "sent by you" is decided by display-name equality, so a same-named colleague's version is taken as the teacher's. The heads' committer id against the account's user id would be exact. |
| Merge: "Yours now" vs "the circle's newest", "Version N is your own send" | **Holds; critic 80 I1 is fixed.** An added question corrected before Get latest is "Changed by you: Solve 5x = 25. You sent this in version 4 and changed it since ..." It ends up once in the bank, and the Send page then says "0 added, 1 changed". A changed question changed again is no longer a conflict: it shows "Yours now: ... 15" and "In version 4 (the circle's newest): ... 13". The same holds after an unreadable answer (`r9-earlier-*`). Not reported (polish): the result box then says "0 kept as yours", just after "Your change stays here" (the next line and the badge do say it is unsent). "You and the circle added the same question" beside "Version 4 is your own send" reads as a coincidence. An added and then corrected question shows no Yours now and newest lines where a changed one does. After an unreadable answer the review says "its answer did not come back" where the strip says "the server's answer could not be read". |
| "N unchanged" once | **Holds** ("2 unchanged", "1 changed by you · 1 unchanged" in the counts only). |
| Invite sentence | **Holds** on the join page under "Found a code: C-...", on the Circles list and on Create a circle: "Your verSHion account needs an invite code from an administrator before it can use circles. The code it needs is not a circle's code. ..." [Open account...]. It fits all three. The email-invite lines are unchanged and still clear. |
| Retry keeps the note | **Holds** (`r9-still-send`: "Added a question" is back in the note; the confirmation box must be ticked again, as it should). After Get latest the next Send page starts empty (`r9-mine-edited-send`, `r9-other-merged-send`), so an old note does not ride on new changes. The decisions part is not shown in frames (the rig has no hidden-key bank). |
| Buttons match the text | **Hold**, except I1. |
| Layout | Every new line wraps cleanly at 760 and 420 px in both themes. Merge rows stack, badges keep their state, and Dismiss and the buttons stay on screen. |
| Remaining "pool" | **None on circle screens.** No string literal added in round 9 (`circle_edit.rs`, `circle_edit_ui.rs`, `circles_text.rs`, `merge.rs`, `circle_links.rs`, `bank_links*.rs`) says "pool", and no frame shows it. |

## What works

- The strip now tells the three outcomes apart and says each plainly: did not go through (Send), a colleague first
  (Get latest), went through (Get latest). It says them the same way after a restart.
- The teacher's own send is never presented as someone else's work. The merge names it, matches what it carried
  (even after a correction), and a later 409 names the colleague only when it is the colleague.
- The usual flaky-wifi case ends at "Your copy is up to date" or "1 unsent change" with Send, the question once.
