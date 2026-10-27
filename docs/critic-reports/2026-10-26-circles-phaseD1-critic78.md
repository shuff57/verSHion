# Circles phase D1, product/UX critic 78 (as a non-technical teacher), round 7

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 7. Round 7 changed:

- a send with no clear answer (none came, a timeout, an unreadable answer, a 500): "Your send may have gone through,
  but no clear answer came: ..." on the Send page and on the strip, the bank marked "when last checked" and checked at
  once;
- a send whose answer could not be read, then more edits: "Your earlier changes were sent. Changes you made since are
  not sent yet. Get the latest first, then send them." with Send offered, and the badge "newer version · unsent changes";
- a send refused because the person is now a viewer: "...can let you send changes by making you an editor", and the
  bank checked at once;
- what the Send page said, kept on the strip after Back or Done;
- the owner's wording on circle screens: "an invite code for the circle", "The circle is not available on this server
  yet".

Checked here: do the strip, the badge, the Send page and the buttons contradict each other; is the wording clear to a
teacher; is any "pool" left on a circle screen. Known exceptions, not re-reported: the Shared page's account guide and
the Account window (they name the shared pool on purpose). Only blockers and important findings are reported. A fresh
critic who built none of this wrote this report.

## Evidence

- **Fresh copy and rig.**
  - `scratchpad/c78src`: the repo's `src/share/` and `src/bankui.rs` as they are now, on critic 76's compiling copy
    for every other file (the other session's `gradeui.rs` and `model.rs` are mid-edit there and left out). QA hooks
    are appended to `circle_edit.rs` only (`qa_sent` calls the real `on_sent`, `qa_leave` the real `close`,
    `qa_check_asks` counts the checks asked for). Every line in the frames is the code's own text.
  - `scratchpad/c78rig`: critic 76's rig plus 30 round-7 modes (`w-*`, `x-banks-w-*`, `x-circle-join*`,
    `x-circle-email`). They cover: unclear answers with the Send page open (timeout, 500, unreadable) and closed;
    the strip after the check that follows (the send went through: version 8 by the sender; or it did not); Send opened
    from that strip; a 409 naming the sender's own version; the viewer refusal (page open, closed, after the check);
    Back or Done on conflict, large, daily limit, role, unclear, held, unread and plain sends; the unread send with later
    edits (strip, Banks cards and list, Send page); the Get latest review after a send that went through without a
    readable answer; the join refusals and the email form with the owner's wording.
  - The rig has no session, so the "check at once" was played by the real `checked()` with the link moved to version 8
    (as `apply_check_as` writes it), or left at 7. `rig: check asks: 1` confirms the unclear send asked for the check.
- **Frames.** On my own Xvfb `:78` (now stopped): 30 modes at 1280, 760 and 420 px, light and dark, 180 frames. All
  exit 0, none blank. They are in `scratchpad/c78shots/`. Contact sheets in the scratchpad: `c78_unclear_page.png`,
  `c78_went.png`, `c78_since.png`, `c78_since420.png`, `c78_conf.png`, `c78_back1.png`, `c78_back2.png`,
  `c78_notconf.png`.
- **Suite.** `cargo test --lib share::` on the copy: 924 passed, 0 failed.
- **Rules kept.** The only repo file changed is this report. Nothing was committed. Port 8799 was not used. The rig on
  18791 (workerd pid 274528) was left running.

## Blockers

None. No path loses unsent work: every "not sent" and "may have gone through" line keeps the changes on this computer
and counted, with Send or Get latest beside it.

## Important

### I1. A send that went through without a clear answer: the check calls the teacher's own version someone else's, and Get latest adds their new questions a second time

Frames `w-unclear-went-*`, `x-banks-w-went-*`, `w-merge-went-*`. The send timed out after it left (or got a 500 or an
unreadable answer), and it did go through as version 8. The strip checks at once, as round 7 intends, and then reads:

> Shared with Algebra team. Version 8 is the latest. You have version 7 and 3 unsent changes. Get the latest first,
> then send.
>
> Your send may have gone through, but no clear answer came: ... Your changes are still on this computer; the circle is
> checked again to see where it stands. [Dismiss]
>
> **[Get latest]** [Send my changes...] [History]

The Banks card says "newer version · 3 unsent". What a teacher reads:

- The note promises a check, and the check is done, but the note still says it is coming. Nothing ever answers "did
  my send go through?".
- The 3 changes that are now in version 8 are called unsent, and version 8 looks like a colleague's.

Following the main button opens the Get latest review (`w-merge-went`):

> Your copy matches version 7. The circle has version 8. ...
> 1 new in the circle · 3 unchanged
> 1 question in this bank is yours alone (not in the circle's bank yet), so the merge leaves it as it is.
> New in the circle: **Solve 5x = 20** (•) Add it to my bank ( ) Do not add it

"Solve 5x = 20" is the question the teacher added and just sent. The circle's copy has a new circle id. The teacher's
own copy is not linked to it. `build_plan` never matches a question new in the circle to an identical one this bank
has but that is not linked, and the default is Add. The result:

- the bank now has the question twice;
- the old copy counts as "1 unsent change";
- the next Send puts a duplicate into the circle's bank, where colleagues see it.

The same review follows round 6's unread send ("Press Get latest to catch up"), so that path doubles new questions too.
Questions that were changed or removed come through correctly ("You and the circle made the same change").

**Why it matters.** Round 7 makes this path the one the strip recommends, on flaky school wifi. Nothing is lost, but
the bank and then the circle get duplicates. The teacher has no way to know that the "new" question is their own.

**Fix.**
- (cheap, main) In `build_plan`, or before it in the circle merge, take a "new in the circle" question that has the
  same content (`same_result`) as an unlinked question of this bank as a match, not an add. Link it to that question
  and say "You and the circle added the same question".
- After the check, replace the note. If the newest version did not move: "Your send did not go through. Press Send my
  changes to send it." If it moved: "The circle has a newer version, version 8. It may be the one you sent. Get latest
  to see."
- Better still: keep the commit id and the note of the unclear send (and of a version-0 send, where `finish_send` now
  drops `pending_send`), and have "Send my changes..." re-send exactly that request. The server replays the first
  answer if it went through (`replayOf` runs before the head check), so the outcome is known and the link moves on with
  its qids, with no merge. Today the new Send page starts with an empty note. If the first send had a note, the retry
  is a new request and meets the 409 in I2.

**Test:** an unclear send that made version 8 with one question added, then a check, then Get latest. The bank has the
question once, the strip shows 0 unsent, and the note no longer says "is checked again".

### I2. "Your earlier changes were sent" on the strip; the Send page counts them again, and the conflict page blames "someone else"

Frames `w-since-*`, `x-banks-w-since*`, `w-since-send-*`, `w-since-conflict-*`, `w-unclear-went-conflict-*`. The
strip and the badge agree: "Your earlier changes were sent. Changes you made since are not sent yet. Get the latest
first, then send them." and "newer version · unsent changes". The second button, Send my changes..., opens a page that
says:

> Since your copy last matched the circle: 2 added, 3 changed, 0 removed.

That is the earlier changes (1 added, 2 changed) plus the later ones. When pressed, Send meets the 409 for the
teacher's own version (a new commit id, because `pending_send` was dropped):

> **Someone else sent changes first, so nothing was sent and nothing was overwritten.**
> Sample Teacher sent version 8, 1 minute ago.
> Get their changes and merge them with yours, then send. You decide what happens to every question you both changed.

This sits right under the page's own note: "The version was sent, but the server's answer could not be read." After
Back, the strip says "Your changes were not sent: someone sent version 8 first." The teacher is Sample Teacher. Three
screens tell three stories about one send. "Get their changes" then leads into I1's duplicates.

**Fix (cheap).**
- On the Send page, while the bank's last send is `sent_unread`, say it above the counts: "Your earlier send went
  through; these counts still include its changes until you get the latest. Get latest first." Put Get latest on the
  page.
- In `conflict_line` and the conflict box, when the version's sender is the person signed in (compare `head_by` with
  the account's display name), write: "Your earlier send made version 8, so this one was not sent. Get latest to catch
  up, then send what is left." Do the same in the strip's "not sent" line.

**Test:** an unread send, an edit, then Send meeting a 409 with `head_by` = the sender. Neither the page nor the strip
says "someone" or "Someone else".

### I3 (for the owner). The owner's invite wording now reads as if the circle's own code were wrong

Frames `x-circle-joininvite-*`, `x-circle-email-*`. Round 7 maps "the shared pool" to "the circle" on circle screens,
as the owner decided. In the two places it lands, the result reads as false, or as a loop:

- **Joining** (a teacher whose account has no pool invite), right under "Found a code: C-7KQ2-...":
  > The circle needs an invite code. Open the Account window, find "Invite code" and enter the code an admin gave you.

  The teacher has just entered the circle's code. They will think it is wrong and ask the circle's owner for a new one.
  Or they will paste the C- code into the Account window, which calls this "the shared pool" and refuses it.
- **Invite by email** (the owner's form):
  > verSHion mails the teacher an invitation with a code ... To join, they need a verSHion account that an
  > administrator has approved and that has redeemed an invite code for the circle. Until then the code can not be used.

  The code being mailed is the circle's invite code, so the sentence reads as a loop.

**Suggestion for the owner.** These keep "pool" off the screen and say which code is meant:
- "Your verSHion account needs an invite code from an administrator before it can join circles. This is not the
  circle's code. Open the Account window, find "Invite code" and enter it."
- "...an account that an administrator has approved and that has redeemed its account invite code. Until then the
  circle's code in the mail can not be used."

On a lighter note, "The circle is not available on this server yet" (`x-circle-joinnotconf`) reads as "this circle".
"Circles are not available on this server yet" says what is meant.

## Brief checklist

| Item | Verdict |
|---|---|
| "Your send may have gone through, but no clear answer came" | **Holds on the Send page** (`w-unclear-page-*`: timeout, 500, unreadable; Try again is the main button, the counts stay, at every width and in both themes). **Holds on the strip before the check** ("Version 7 when last checked. You have 3 unsent changes." with Send). **Not after the check**: I1. |
| Unsure, then the check | **Not met**: I1. The note never updates. When the send went through, the strip and badge count the sent changes as unsent and recommend Get latest, which doubles added questions. When it did not (`w-unclear-notwent`), "Version 7 is the latest. You have 3 unsent changes." with Send is right, but the note still says "may have gone through". |
| "Not sent yet... Get the latest first, then send" with Send | **Strip and badge agree** (`w-since`, `x-banks-w-since*`: "newer version · unsent changes", the name cut to "Algebra t..." at 420 px with the state kept). **The Send page and the conflict page contradict them**: I2. |
| Viewer-refused send | **Holds.** The page (`w-role-page`) says "You can only look at this circle's banks now. The owner can let you send changes by making you an editor." The strip, once checked (`w-role-checked`), turns into "(you can view)... You changed 3 questions here. They stay on this computer; viewers can't send." with Get latest, and the note under it matches. Not reported: the page still offers "Change the note", which can not help. |
| Result notes left after Back or Done | **Hold.** Conflict ("someone sent version 9 first. Get the latest, then send them." over Get latest), too large, daily limit, role, unclear, held ("Version 8 was sent. This version is held back...") and unread all reach the strip with Dismiss. A plain send that went through adds nothing (`w-ok-done`). Not reported, as before: the daily-limit note says "Try again tomorrow" over a Send button. The unread strip says its point twice, and calls it "the server's answer" where the page says "the circle's answer". |
| Owner-decided wording | **I3** (for the owner). |
| Remaining "pool" | **None new on circle screens.** The round 7 sentences say "verSHion's server" and "the circle's answer". The Account window that "Open account..." opens from the join refusal still says "The shared pool is invite-only". That is legitimate there, but it names the same code differently from the circle screen (I3). The offline reason shows the server's address as the transport wrote it, as critic 76 noted. |

## What works

- A send with no clear answer is no longer called "not sent". The Send page offers the safe retry, and its wording
  says plainly that nothing is sent twice.
- Edits made after an unread send are never called sent, on the strip or on the Banks cards and list.
- A viewer whose send was refused sees the viewer strip at once, and the refusal says the right thing.
- Every answer the teacher did not act on, on the Send page, survives Back and Done as a dismissible note on the bank's
  own strip.
