# Circles phase D1, product/UX critic 88 (as a non-technical teacher), round 12

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 12. Round 12 changed:

- "Difficulty not recorded" rows in Get latest, with their own count in the summary (critic 86 I1);
- a three-line conflict card when the person's own older version is the base: "In version 4 (your send)" (critic 86 I2);
- a line for an own version that can't be read now, no default Add for a question new in the circle that may be the
  person's own, and "You sent a change to this question, maybe in version N" (critic 85 m1);
- after a 409 that names the person's own version, the strip says "Your earlier changes were sent" (critic 86 minor);
- a link an earlier build wrote learns its difficulties from the bank when the bank is exactly as last written, and
  is otherwise never said to be up to date (critic 85 I1).

Checked here: is the wording clear to a teacher; do the strip, the badge, the Send page, the Get latest page and the
conflict card contradict each other; do the buttons match the text; does a "CONFLICT: both changed" row with the
circle's version already chosen read sensibly; is any "pool" left on a circle screen; and what the fixer's remark "the
pool review still labels these rows CONFLICT" means. Only blockers and important findings are reported. A fresh critic
who built none of this wrote this report.

## Evidence

- **Fresh copy and rig.** `scratchpad/c88/w`: the repo's `src/share/` and `src/bankui.rs` as they are now (checked byte
  for byte), on critic 86's compiling base (the other session's files at their earlier versions, `gradeui.rs` left out,
  `winit` as a dev-dependency). Changes in the copy: one module `src/share/qa_c88.rs` (cfg(test)) and its `mod` line,
  and a 5-line cfg(test) shim at the end of `browse_ui.rs` so the rig can draw the public pool's real `review_page`.
- **States.** Reached through the real models with the round tests' scripted sessions. The rig starts from critic 86's
  modes and the round 12 builder's `r12-*` modes, plus 10 of my own (`c88-*`): a link an earlier build wrote where the
  teacher's only change is a difficulty (strip; Send page; a check finding Bob's version 4; Get latest when Bob added a
  question or reworded question 1; Apply with the defaults), the same link after adding a question (strip, Send page),
  and the public pool's update review on an import an earlier build recorded (the author re-rated / the teacher re-rated).
- **Frames.** My own Xvfb `:88` (stopped when done): 27 modes at 1280, 760 and 420 px, light and dark, 162 frames, all
  exit 0, none blank. Frames in `scratchpad/c88/shots/`; the text of 53 modes in `scratchpad/c88/text.txt` and
  `text2.txt`. (In the pool-review frames Apply merge is greyed because the rig's Browse model has no merge loaded: a
  rig artifact, not a finding.)
- **Suite.** `cargo test --lib share::` on the copy: 1071 passed, 0 failed.
- **Rules kept.** The only repo file written is this report. Nothing committed. Port 8799 not used. The rig on 18791 left
  alone.

## Blockers

None. Every merge still saves a restore point first, and nothing is sent twice. I1 below is the nearest: the teacher's
own unsent change is dropped only if she accepts a default, on a page that shows it.

## Important

### I1. A link an earlier build wrote: the teacher's own unsent difficulty change is preselected to be replaced by the circle's in Get latest

Frames `c88-legacy-mine-merge-*`, `c88-legacy-mine-applied-*`, `c88-legacy-mine-reworded-merge-*`, `c88-pool-legacy-mine-*`.

The teacher's working copy was linked before this build. She marks question 1 "Easy" and changes nothing else. Bob
sends version 4, which adds a question and leaves question 1 alone. She presses Get latest:

> 1 difficulty not recorded - 1 new in the circle
> **Difficulty not recorded** · One
> This computer has no record of this question's difficulty in version 3, so it is not known who changed it. The
> circle's version is used unless you choose Keep mine.
> In version 3: not recorded on this computer · Yours now: Easy · In version 4 (the circle's newest): none
> ( ) Keep mine **(•) Use the circle's version** ( ) Keep both

The pill is the calm green, not the red "needs a decision" one, and nothing asks her to choose. Apply merge with the
defaults removes her "Easy". The strip then says "Version 4 is the latest. Your copy is up to date.", and there is
nothing left to send. When Bob also reworded question 1 (`c88-legacy-mine-reworded-merge`), the circle's version is
again preselected. A link made by this build would make that same row an undecided conflict.

**Cause.** `merge.rs` around 755: `row.choice = if known.0 { Choice::Undecided } else { Choice::Theirs }`. `known.0` is
`od.any()` worked out before the difficulty is counted, so a change that is only a difficulty never counts as hers.
Critic 86 suggested the circle's version as a default only "when the teacher's copy still has the difficulty that every
earlier record shows". That condition was dropped.

**Why it matters.** This is unsent work, and the page's default throws it away. The other side of the same row is
critic 86 I1, where Keep mine undid a colleague's rating. Round 12 moved the risk from the colleague to the teacher. It
did not remove it.

**Fix.**
- Preselect the circle's version only when this question was not edited here since the link last matched, for example
  `q.modified <= link.bank_modified`. Otherwise leave the row undecided, red, with "Choose which to keep."
- Better: when a base difficulty is not known, read it from version 3's body. That version is normally still kept, and
  the own-version path already reads a version body in the same way.

**Test.**
- An earlier build's link, the teacher sets question 1 to Easy, and the circle's version 4 adds a question. Applying
  with the defaults never drops Easy.
- The same when version 4 also rewords question 1.
- Bob's rating on an untouched question is still taken by default (critic 86 I1 kept).

### I2. Every link an earlier build wrote that has any unsent edit now says "changes not counted"; the strip sends the teacher to a Send page that can't show them

Frames `c88-legacy-added-strip-*`, `c88-legacy-added-send-*`, `c88-legacy-mine-strip-*`, `c88-legacy-mine-send-*`,
`c88-legacy-mine-checked-strip-*`, `r12-legacy-uncounted-strip-*`.

Take a working copy made before this build and add one question to it. The same bank in the round 11 tree said "1
unsent change", and its Send page said "1 added". Now:

> Badge: Algebra team · changes not counted
> Strip: Shared with Algebra team. Version 3 is the latest. **Press Send to see what you changed.** [Send my changes...]
> Send page: **Your copy has no record of what it looked like when it last matched the circle**, so the changes can't be
> counted here. The circle works them out when you send.

The strip says that pressing Send shows her changes, and the Send page then says it can't. The Send page's sentence is
also false here and alarming: the copy has a full record of version 3 except the difficulties. When the circle moves on
(`c88-legacy-mine-checked-strip`), the strip reads "Version 4 is the latest. Press Send to see what you changed." with
**Send my changes** as the primary button and Get latest second. It no longer says "You have version 3" or "Get the
latest first, then send" (Q10). Send then meets a 409, which leads to the I1 review.

**Cause.**
- `circle_links.rs` `changes_since` around 336, new in round 12: any unchanged question whose base has no difficulty
  sets `known = false`.
- `fill_difficulty` fills in the difficulties only while the bank is exactly as last written, so any edit at all falls
  through to the line above.
- The "not counted" wording (`bank_links.rs` 344, `circle_edit_ui.rs` 116) was written for a copy with no base at all,
  which was rare. It is now the state of every linked bank that has unsent work.

**Why it matters.** It hits every existing linked bank that has unsent work, which is where the count matters most. The
strip and the Send page contradict each other, the button does not do what the line says, and when the copy is behind
the order goes against the owner's Q10 answer.

**Fix.**
- Keep counting what is known.
- When the only unknown is a difficulty, use your own words, for example: "You have 1 unsent change (a change to a
  difficulty may not be counted here)." On the Send page: "1 added. This copy has no record of the difficulties in
  version 3, so a change to a difficulty can't be counted here."
- Keep "no record of what it looked like" for a copy with no base.
- Replace "Press Send to see what you changed" with words the Send page bears out.
- When the copy is behind, put Get latest first and say "You have version 3".

**Test.**
- An earlier build's link with one added question: the badge, the strip and the Send page each say 1 added or 1
  unsent, with the difficulty caveat.
- Behind and not countable: Get latest first, and "You have version 3".

### I3. An own version that can't be read: the note above the review says "your send went through", the review says it "may be" her send, and a row says it is not known whether someone changed her question but offers no choice

Frames `r12-own-unread-reverted-merge-*`, `r12-own-unread-added-merge-*`.

In round 12 any failed read of the teacher's own version leads here: `.ok()` in `circle_edit.rs` around 664, so a
timeout or a 5xx on flaky wifi is enough. One page then shows:

> The check found version 4 in the circle, sent by you: your send went through. Press Get latest to catch up. [Dismiss]
> Get latest ... Version 4 **may be your earlier send** from this computer, but it can't be read now ...
> **Changed by you** · One (fix) · You sent a change to this question, maybe in version 4, which can't be read now, **so it
> is not known whether someone changed it since. Yours stays as it is.**
> In version 3: One · Yours now: One (fix) · In version 5 (the circle's newest): One

The note says the send certainly went through, and the review under it says "may be". The row says a colleague may
have changed the question, and version 5 does show it back as "One", but a "Changed by you" row has no radio buttons.
She cannot take the circle's version, and her next send puts "(fix)" back over whatever Bob did.

**Fix.**
- While the review has `own_unread`, hide the after-check note, or word the review line to agree with it ("Version 4
  is your send, found by the check, but it can't be read now ..."). Hiding the note also clears critic 86's "Press Get
  latest to catch up" above an open Get latest page.
- Give a `sent_change_unread` row Keep mine (default) and Take the circle's version.

**Test.**
- An unreadable own version, with the circle's newest holding question 1 as it was before the send. The review offers
  Take the circle's version, and no text on the page says both "went through" and "may be".

### I4. What "the pool review still labels these rows CONFLICT" means: the public pool's update review contradicts itself on every import recorded before this build

Frames `c88-pool-legacy-theirs-*`, `c88-pool-legacy-mine-*`.

`merge.rs` is shared with Shared > Browse > Update from pool. At HEAD, `Snap` had no difficulty, so every public-pool
import already recorded has none. Its review now shows a question whose difficulty differs like this:

> 1 conflict: both changed
> **CONFLICT: both changed** (red) · One
> This computer has no record of this question's difficulty in version 3, so it is not known who changed it. The pool's
> version is used unless you choose Keep mine.
> When imported: not recorded on this computer · Yours now: Easy · The pool's new version: none
> ( ) Keep mine **(•) Use the pool's version** ( ) Keep both

The red label says both changed, and the note under it says nobody knows who changed it. This is critic 86 I1 again,
fixed on circle screens only (`browse_ui.rs` `kind_label`, the summary at 1057 and the pill at 1091 ignore
`row.unrecorded`). With I1, the teacher's own rating is preselected to be undone here too. At HEAD this screen did not
compare difficulties at all, so this is new to the pool flow in phase D. It is not a circle screen, but it is a
regression this phase brings to one.

**Fix.** Use `row.unrecorded` in `browse_ui.rs` as `circle_edit_ui.rs` does: the "Difficulty not recorded" label, a
count of its own, and the calm colour while a choice is preselected. Apply I1's default rule here too.

## Brief checklist

| Item | Verdict |
|---|---|
| "Difficulty not recorded" rows and their count | **Hold** on circle screens: the pill "Difficulty not recorded", the summary "1 difficulty not recorded - 1 new in the circle", "In version 3: not recorded on this computer" (no "is not kept"), the note, the calm colour while preselected, and red with "Choose one to continue." and "1 question still needs a decision ..." while undecided (`r12-legacy-unrecorded-decide-merge`). Exception: the default in I1. |
| "CONFLICT: both changed" with the circle's version preselected | **Not on any circle screen.** Rows that are a decision only because a difficulty is not recorded get their own label, and a real conflict stays undecided. The combination appears only on the public pool's review (I4). |
| 3-line conflict card | **Holds** (`own-then-bob-both-merge`): "In version 4 (your send): One (fix) / Yours now: One (fix 2) / In version 5 (the circle's newest): One (fix) by Bob", undecided. A changed-in-the-circle row on the same base also has three lines. When the own version is the newest it is still two lines (`changed-again-merge`). |
| Unreadable-version line | **Present and readable**, if long. It contradicts the note above it (I3). |
| "Your earlier changes were sent" after a 409 naming her own version | **Holds** (`carried-conflict-strip`): "Your earlier changes were sent. Changes you made since are not sent yet. Get the latest first, then send them." The badge reads "newer version · unsent changes" (no stale count) and Get latest is primary. Polish: "were sent" sits right above the note's "Your changes were not sent: your earlier send went through ...". Each is qualified, but they read as opposites at a glance. The Send page left open after that 409 still heads "Since your copy last matched the circle: 1 added, 1 changed", and the added question already went in version 4. |
| No default Add when the own version can't be read | **Holds** (`r12-own-unread-added-merge`): "This may be a question your earlier send added ... If it is, adding it puts it in your bank twice.", with neither radio button set, "Choose one to continue." and "1 question still needs a decision". |
| Same change on both sides | **Holds** (`r12-same-change-applied`): "Version 4 is the latest. Your copy is up to date." |
| Strip vs badge vs Send vs Get latest vs conflict card | **Agree**, except I2 (the strip promises what the Send page can't show) and I3 (the note vs the review). |
| Buttons match the text | **Hold**, except I2 ("Press Send to see what you changed") and I3 ("not known whether someone changed it since", with no choice). |
| Layout | Every new line and card wraps at 760 and 420 px in both themes. Dismiss, the radio buttons and Apply stay on screen. |
| "pool" on circle screens | **None.** `unrecorded_note`'s "The pool's version is used ..." reaches circle screens as "The circle's version is used ..." through `merge_words`. No circle frame shows "pool". The only "pool" text is on the public pool's own review (I4), where it belongs. |
