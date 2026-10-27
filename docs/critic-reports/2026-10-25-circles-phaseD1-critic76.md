# Circles phase D1, product/UX critic 76 (as a non-technical teacher), round 6

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 6. Round 6 changed:

- the send results said on the strip when the Send page was closed under a send ("Your changes were not sent: ...");
- the strip after a send whose answer could not be read ("Your changes were sent. Get the latest so that this copy has
  the version they made.");
- "Version N when last checked" after a restore or trim that got no answer;
- the sign-in-needed lines;
- version 0 in the "Sent" box;
- the share window's title at the sign-in step ("Share to a circle").

Checked here: do the result lines on the strips make sense; do the buttons match what the text says; do the strip, the
badge and the Send page contradict each other; is any "pool" left. Known exceptions, not re-reported:
`EMAIL_NEEDS_ACCOUNT`, the "shared pool" invite-code errors, and the Shared page's account guide. Only blockers and
important findings are reported. A fresh critic who built none of this wrote this report.

## Evidence

- **Fresh copy and rig.**
  - `scratchpad/c76src` is a copy of the current tree. Its viSHion path dependencies are made absolute.
  - `diff -rq` against the repo's `src/share` and `src/bankui.rs` shows one difference only: `circle_edit.rs` has QA
    hooks added at its end. `qa_sent` hands a send's answer to the real `on_sent` (with the Send page open or closed),
    `qa_timeout` runs the real `stop_waiting` after a minute, and `qa_rolled` / `qa_trimmed` run the real restore and
    trim handlers. Every line in the frames is the code's own text.
  - The rig is `scratchpad/c76rig`: critic 74's rig plus `z-*` modes. They cover these answers to a send with the page
    closed: unread (version 0), conflict with and without the newer version, 500, offline, `circle_role_required`,
    signed out, `confirm_large`, merge token, daily limit, held, and done. They also cover the unread "Sent" box (open,
    then Done), unread followed by more edits (strip and Banks list), unread while signed out, a restore or trim with
    no answer (in step, with unsent changes, behind), a restore answered with version 0, the share window at the sign-in
    step (signed out, and waiting for approval), and the signed-out strip.
- **Frames.** Taken on my own Xvfb `:76` (now stopped): 28 modes at 1280, 760 and 420 px, light and dark, 166 frames.
  All exit 0, and all are readable and not blank. They are in `scratchpad/c76shots/`. Contact sheets are in the
  scratchpad: `c76_strips1280a.png`, `c76_strips1280b.png`, `c76_strips1280c.png`, `c76_ok.png`, `c76_pages.png`,
  `c76_420a.png`, `c76_420b.png`, `c76_unread_edits.png`, `c76_banks_unread_edits.png` and `c76_restore_v0.png`.
- **Suite.** `cargo test --lib share::` on the copy: 868 passed, 0 failed.
- **Rules kept.** The only repo file changed is this report. Nothing was committed. Port 8799 was not used. The rig on
  18791 (workerd pid 274528) was left running.

## Blockers

None. No path loses unsent work. Each "not sent" line keeps the changes counted, and Send or Get latest is there to
recover.

## Important

### I1. After a send whose answer could not be read, later edits are said to be sent

Frames `z-unread-edits-*` and `x-banks-unread-edits-*`. The send went through but its answer was unreadable (round 6's
`unread_sends`). The teacher then edits one question and adds one. The strip still reads:

> Shared with Algebra team. Your changes were sent. Get the latest so that this copy has the version they made.
> [Get latest] [History]

On the Banks list, the card says only "newer version". `strip_view_known` returns this line before it looks at the
changes. `badge_of_sent` sets the unsent count to 0 while the flag is set. The flag stays until a merge, a send or a
restart, so for that whole time the new edits are called sent and Send is not offered.

**Why it matters.** "Your changes were sent" is the one thing a teacher acts on. The ones who stop here believe
colleagues have the new edits. Nothing is lost: Get latest keeps the edits as "yours", and a restart counts them again.

**Fix (cheap).**
- Keep the bank's `modified` with the flag: the value from `Inflight.bank` when the send started.
- When the bank changed after that, use the behind-and-unsent wording: "Your earlier changes were sent. Changes you made
  since are not sent yet. Get the latest first, then send them." Offer [Get latest] [Send my changes...] [History].

**Test:** an unread send, then an edit. The strip offers Send after Get latest and never says the later edit was sent.

### I2. A restore answered without a readable version says "Version 0" next to "up to date"

Frame `z-restore-v0-*`. This uses the real `on_rolled` with `Ok(version 0)`. Round 6 fixed version 0 for sends only.
The strip reads:

> Shared with Algebra team. Version 7 is the latest. Your copy is up to date.
>
> Version 0 was restored from version 6. [Dismiss]

By code, the History page says "Done. Version 0 is now the circle's newest, with the content of version 6."
(`restore_lines`).

**Why it is wrong.** The circle now has a newer version, but `note_head(0)` moves nothing and the bank is not marked
unsure. The strip therefore makes the false "up to date" claim that critics 69, 72 and 74 held the line on, until the
10-minute check. It is the same rare path as critic 74 I1, on the restore side.

**Fix.**
- On `Ok` with version 0 in `on_rolled`: add the bank to `unsure` (so the strip says "when last checked") and push it
  to `check_asks`.
- In `restore_lines` and `restore_note`, drop the number: "Version 6 was restored, but the circle's answer could not
  be read. Open History to see the new version."

**Test:** a restore answered with version 0. Neither screen says "Version 0" or "up to date".

### I3 (rare, cheap). A send refused because the person is now a viewer: the strip still offers Send

Frame `z-role-left-*` (403 `circle_role_required`, with the page closed). The strip reads:

> Shared with Algebra team. Version 7 is the latest. You have 3 unsent changes.
>
> Your changes were not sent: You can only look at this circle's banks. The owner can let you add banks by changing
> your role to editor. They are still on this computer.
>
> [Send my changes...] [History]

The strip's own line and its main button still treat the teacher as an editor. Pressing Send gets the same refusal
again. The line also talks about adding banks when the teacher was sending changes. The strip corrects itself only at
the next 10-minute check.

**Fix.** On `circle_role_required` (any 403 refusal of a send), push the bank to `check_asks`, as the conflict without
a version already does. The strip then turns into the viewer line ("You changed 3 here. They stay on this computer;
viewers can't send.") at once. For a send, word it "...can let you send changes by making you an editor."

## Brief checklist

| Item | Verdict |
|---|---|
| Result lines on strips make sense | **Hold**, except I1 and I3. Each of these sits under the strip's line, above the right buttons, and keeps the changes counted at every width and in both themes: the conflict ("someone sent version 9 first. Get the latest, then send them." with Get latest as the main button), 500, too-large ("Press Send my changes to send them anyway"), daily limit, signed out, held and no-answer lines. A send that went through with the page closed says nothing extra, and the strip moves to "Version 8 is the latest. Your copy is up to date." Not reported: the offline line repeats itself ("your work on this device is unaffected. They are still on this computer.") and shows the server's address. |
| Buttons match the text | **Hold**, except I3. The signed-out result "Sign in, then send your changes again" sits over a lone "Sign in...". The no-answer line "open History to see whether it did" has History next to it. The unread strip offers Get latest. Not reported (one click from the right button): the unread "Sent" box says "Press Get latest to catch up" but offers only Done, and Done lands on the strip's Get latest. The rare merge-token line says "Get the latest changes and merge again" with no Get latest on the strip, but Send works there. |
| Strip, badge and Send page agree | **Not met in I1 and I2.** Otherwise they agree: the unread badge says "newer version", matching the strip's Get latest. A held version, from the sender's side, is "Version 8 is the latest. Your copy is up to date." plus the held note, which is consistent. Polish only: the unread strip says the same thing twice (its line and its result), and the Send page says "the circle's answer" where the strip says "the server's answer". |
| "Version N when last checked" | **Holds.** "Shared with Algebra team. Version 7 when last checked." sits with the no-answer note and History, or with "You have 3 unsent changes" and Send, or with "You have version 7" and Get latest. It is never "up to date" while a restore may still land. |
| Sign-in-needed lines | **Hold.** "Version 7 when last checked, 2 days ago. Sign in to send or get newer versions." [Sign in...]. The send's session line is dropped once signed in (by code). Pre-existing and rare, not reported: an account that is signed in but waiting, suspended or withdrawn also gets "Sign in to send" (`z-share-pending`: the share window says "Your account is waiting for approval." over that strip). |
| Version 0 | **Fixed for sends** (the "Sent" box and the strip). **Not for restores** (I2). |
| "Share to a circle" title | **Holds** at the sign-in step (`z-share-gate`, `z-share-pending`) and on Options. |
| Remaining "pool" | **None new.** A grep of every circle screen file finds only the known exceptions and the true "items from the public pool" credit line. No round 6 sentence says "pool". |

## What works

- With the Send page closed under a send, the outcome is now said on the bank's own strip, in plain words, with the
  changes kept and counted. A conflict moves the strip to "Get the latest first, then send" at once.
- A restore or trim with no answer no longer claims the latest version or "up to date".
- The signed-out strip and the send's session line agree, and both point at the one button they name.
