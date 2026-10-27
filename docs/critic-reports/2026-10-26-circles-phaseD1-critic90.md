# Circles phase D1, product/UX critic 90 (as a non-technical teacher), round 13

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 13. Round 13 changed:

- "Difficulty not recorded" rows start with nothing chosen (critic 88 I1, critic 87 I1);
- a link an earlier build wrote learns its difficulties from the circle's own copy of its base version, read once, never
  from the bank (critic 87 B1);
- the strip, the badge, the Send page and the circle page's bank line say what is counted plus a difficulty caveat,
  keep "You have version N", and put Get latest first when the copy is behind (critic 88 I2);
- Keep mine / Take the circle's version on a "may be your earlier send" row, and no after-check note above that review
  (critic 88 I3);
- the public pool's Update review uses the "Difficulty not recorded" pill and count (critic 88 I4);
- a server error while reading the person's own version says "try again in a minute" with no Try again button (critic 87 m2).

Checked here: is the wording clear to a teacher; do the strip, the badge, the Send page, the Get latest page, the conflict
card and the circle page contradict each other; do the buttons match the text; can a teacher finish Get latest when rows
are undecided (and is Apply's greyed state explained); is any "pool" left on a circle screen. Only blockers and important
findings are reported. A fresh critic who built none of this wrote this report.

## Evidence

- **Fresh copy and rig.** `scratchpad/c90/w`: the repo's `src/share/` and `src/bankui.rs` as they are now (checked byte
  for byte with `diff -rq`), on critic 88's compiling base (the other session's files at their earlier versions,
  `gradeui.rs` left out, `winit` as a dev-dependency). Changes in the copy: one module `src/share/qa_c90.rs`
  (cfg(test)) and its `mod` line, and a 5-line cfg(test) shim at the end of `browse_ui.rs` so the rig can draw the public
  pool's real `review_page`.
- **States.** Reached through the real models with the round tests' scripted sessions. The rig starts from critic 86 and
  88's modes and the round 13 builder's `r13-*` modes. Since round 13 reads version 3 for an earlier build's link, the rig
  now serves a readable version 3 by default (a mode that wants it gone answers 404 `version_not_retained`). 22 modes are
  my own (`c90-*`): a colleague re-rating 6 of 8 questions on an earlier build's link whose version 3 is no longer kept
  (strip, review, half decided, all decided and applied, circle page); the same link with nothing changed, one question
  added, or behind (strip, Send page, circle page); a server error on Get latest and on the own-version read; Take the
  circle's version on a "may be your earlier send" row, then Apply; an earlier build's link whose version 3 holds a
  colleague's rating the phase C merge left out (Banks list before the page is opened, strip, Send page, Get latest, after Apply); and
  Bob rewording and rating a question the teacher never touched, version 3 not kept. The circle page is the real
  `circles_ui::body_of` (its header shows the fixture's circle name, "Biology team": a rig artifact).
- **Frames.** My own Xvfb `:90` (stopped when done): 32 modes at 1280, 760 and 420 px, light and dark, 192 frames, all
  exit 0, none blank. Frames in `scratchpad/c90/shots/`; the text of 86 modes in `scratchpad/c90/text.txt`,
  `text2.txt` and `text3.txt`.
- **Suite.** `cargo test --lib share::` on the copy: 1096 passed, 0 failed.
- **Rules kept.** The only repo file written is this report. Nothing committed. Port 8799 not used. The rig on 18791 left
  alone.

## Blockers

None. Every merge still saves a restore point first, nothing is sent twice, and no unsent change of the teacher's is
dropped by a default on any page I reached.

## Important

### I1. A colleague's rating that the phase C build's Get latest left out is called the teacher's own unsent change; the review says "Only you changed this", offers no choice, and her next send removes the rating from the circle

Frames `c90-phantom-list-*`, `c90-phantom-strip-*`, `c90-phantom-send-*`, `c90-phantom-merge-*`, `c90-phantom-applied-*`.

The phase C build ignored difficulty when it merged. Bob rated question 1 "Hard" in a version that also changed
something else; the teacher pressed Get latest with that build, which took his other change and left her question 1 at
no difficulty, and recorded her copy as matching version 3. She has not touched the bank since. With round 13:

> Banks list (page not opened yet): Algebra team · changes not counted
>
> Strip: Shared with Algebra team. Version 3 is the latest. **You have 1 unsent change.** [Send my changes...]
>
> Send page: Since your copy last matched the circle: 0 added, **1 changed**, 0 removed.
>
> Get latest (Bob then adds a question in version 4): 1 changed by you - 1 new in the circle
> **Changed by you** · One · **Only you changed this. It stays as it is.**
> Difficulty · In version 3: Hard · Yours now: none · In version 4 (the circle's newest): Hard
> (no buttons on that row) ... [Apply merge]

Every page tells her she made a change she did not make, and none says which question it is except the review. The
review states it as fact and offers no way to take the circle's rating. After Apply the strip says "Version 4 is the
latest. You have 1 unsent change." (`c90-phantom-applied`), and the next send she makes for any reason puts question 1 back to no difficulty in the circle, undoing Bob's
rating. To avoid that she would have to notice the "In version 3: Hard" line and set the difficulty by hand.

**Cause.** `circle_links.rs` `learn_difficulty` (around 374) fills the base from the circle's version 3, and
`changes_since` then counts any difference as hers. Its own comment says the bank "can still hold a rating the circle
does not have (the teacher's unsent one), or lack one it has (a colleague's that merge dropped)". The two cases can't be
told apart on this computer, but only the first is handled: the second is counted, worded and defaulted as the
teacher's. `merge.rs` (around 738) then makes it an `OursChanged` row with the fixed note and no options.

**Why it matters.** It hits every link the phase C build wrote where a colleague's rating arrived with other changes.
The page states something false as certain, the default undoes a colleague's work in the circle, and the strip and badge
keep inviting a send. This is critic 86 I1's harm in the opposite direction, and it survives round 13's "never decided
for her" rule because this row is not marked as unknown.

**Fix.**
- When a difficulty was learnt from the circle's version (not recorded by this computer) and the bank differs from it on
  a question the bank has not edited since the link last matched (`q.modified <= link.bank_modified`, or the bank print
  still matching), treat that difficulty as not known, as the 404 path already does: the strip and badge give the
  caveat instead of counting it, and Get latest shows a "Difficulty not recorded" row with nothing chosen.
- With nothing newer to get, say it on the strip in words the Send page bears out, for example: "Question 1's difficulty
  differs from the circle's. An earlier verSHion did not compare difficulties, so it may be a colleague's rating you
  never got. Check it before you send."

**Test.**
- An earlier build's link, the bank as last written with question 1 at none, version 3 serving "Hard": the strip and
  badge never say "1 unsent change" for it, and Get latest shows question 1 undecided with "Use the circle's version".
- critic 87's P1x (the bank holding "Hard", the circle none) is still never "up to date" and never undone by a default.

### I2. A colleague's rewording is hidden behind a "Difficulty not recorded" label and count; "Keep mine" there throws the rewording away

Frames `c90-legacy-bob-both-merge-*`, `r13-legacy-reworded-merge-*`.

An earlier build's link whose version 3 is no longer kept. The teacher changed nothing. Bob rewords question 1 and rates
it "Hard" in version 4. Get latest:

> 1 difficulty not recorded
> **Difficulty not recorded** (red) · One
> This computer has no record of this question's difficulty in version 3, so it is not known who changed it. Choose
> which to keep.
> Question text · In version 3: One · Yours now: One · In version 4 (the circle's newest): One, reworded by Bob
> Difficulty · In version 3: not recorded on this computer · Yours now: none · In version 4: Hard
> ( ) Keep mine ( ) Use the circle's version ( ) Keep both

The summary and the pill both say the row is about a difficulty. The circle's change to the wording is known (version 3
and her copy agree) but is counted nowhere and named nowhere except in the three lines under "Question text". "It is not
known who changed it" reads as if it were about the whole question. The same label appears when the teacher's only change
is her own rating against Bob's rewording (`r13-legacy-reworded-merge`). A teacher who reads the label and wants her
difficulty kept presses Keep mine. That keeps "One", and her next send undoes Bob's rewording. When version 3 can be read, the
same change by Bob is a plain "Changed in the circle" row with his version taken by default.

**Cause.** `merge.rs` around 767: `unrecorded && !(known.0 && known.1)` marks the row `unrecorded` whenever the teacher's
side has no other known change, even when the circle's side does (`known.1`). The label in `circle_edit_ui.rs` around
724 and the summary around 691 then follow `row.unrecorded` alone.

**Fix.**
- Use the "Difficulty not recorded" label and count only when neither side has any other known change.
- When the circle changed something else, label and count the row "Changed in the circle". Keep it undecided, because
  the difficulty is unknown, and give it its own note, for example: "Bob changed this question in version 4. Its
  difficulty also differs from yours, and this computer has no record of version 3's, so it is not known whose rating is
  newer. Use the circle's version to take his wording and his rating, or keep yours (his wording is then not kept)."

**Test.**
- An earlier build's link, version 3 not kept, the teacher unchanged, version 4 rewording and rating question 1: the
  summary counts 1 changed in the circle, the pill is not "Difficulty not recorded", the note names what Keep mine
  loses, and nothing is chosen.
- A difficulty-only difference keeps today's "Difficulty not recorded" row.

## Brief checklist

| Item | Verdict |
|---|---|
| "Difficulty not recorded" rows, nothing preselected | **Hold.** Every such row starts with no radio set, a red pill, "Choose one to continue." and, above Apply merge, "N questions still need a decision before the merge can be applied." (`c90-rated6-merge`, `r13-legacy-unreadable-merge`). A teacher's own "Easy" is no longer taken away by a default (critic 88 I1 fixed). With version 3 readable, those rows do not appear at all: the review reads the version and shows a plain "Changed by you" or "Changed in the circle" (`r13-legacy-read-merge`), except in I1. Label: I2. |
| Can a teacher finish Get latest with undecided rows; is the greyed Apply explained | **Yes.** Six undecided rows: each card says "Choose one to continue.", the line above the button counts what is left (6, then 3 after three choices, `c90-rated6-half-merge`), each decided card turns to the calm colour, and Apply merge becomes active when the last one is chosen. Applying gives "Version 4 is the latest. Your copy is up to date." with the six ratings in the bank (`c90-rated6-applied`). Not reported (minor): there is no "Use the circle's version for all", so a colleague who re-rated 30 questions means 30 clicks. |
| One story about uncounted changes (strip, badge, Send page, circle page) | **Holds** for what round 13 aimed at. One added question: badge "1 unsent change", strip "You have 1 unsent change. This copy has no record of the difficulties in version 3, so a change to a difficulty can't be counted here.", Send page "1 added, 0 changed, 0 removed" plus the same sentence, circle page "Not sent yet: 1 added, 0 changed, 0 removed. A change to a difficulty can't be counted for this copy." (`r13-legacy-added-*`, `c90-legacy-added-circle`). "Press Send to see what you changed" is gone. Behind: "You have version 3 and 1 unsent change ... Get the latest first, then send." with Get latest first, badge "newer version · 1 unsent" (`r13-legacy-behind-strip`, `c90-legacy-behind-circle`). No record at all keeps "no record of what it looked like" with Get latest first (`r13-norecord-behind-strip`). Exception: I1, where a count is given that is not the teacher's. |
| "You have version N" kept, Get latest first when behind | **Holds** in every behind mode, also with nothing changed (`c90-legacy-behind-none-strip`, `c90-rated6-strip`). |
| Keep mine / Take the circle's version on "may be your earlier send" rows | **Holds** (`r13-own-unread-reverted-merge`): "You sent a change to this question, maybe in version 4, which can't be read now, so it is not known whether someone changed it since. Yours stays as it is unless you take the circle's version." with Keep mine chosen and Take the circle's version offered. No after-check note above that review, so nothing on the page says both "went through" and "may be" (critic 88 I3 fixed). Taking the circle's version and applying gives "Version 5 is the latest. Your copy is up to date." (`c90-own-unread-take-applied`). |
| Pool Update review: "Difficulty not recorded" pill and count | **Hold** (`c88-pool-legacy-*`): "1 difficulty not recorded", the pill, "When imported: not recorded on this computer", nothing chosen, "1 question still needs a decision". No "CONFLICT: both changed" over an unknown (critic 88 I4 fixed). "pool" there is the public pool's own screen. |
| "Try again in a minute" on server errors, no button | **Holds, and the text matches the buttons.** A 503 on Get latest or on the own-version read: "verSHion's server had a problem of its own. Nothing was changed; try again in a minute." with only Back (`c90-server-*`). A timeout keeps "Can't reach ...: the connection timed out." with Try again (`r13-own-read-failed-merge`). Polish, not reported: on the own-version path the after-check note "your send went through. Press Get latest to catch up." still sits above the failed page. |
| Strip vs badge vs Send vs Get latest vs conflict card vs circle page | **Agree**, except I1 (a change counted on every page that the teacher did not make) and I2 (the label and count against the card's own "Question text" lines). Not reported (minor): before a linked bank's page is first opened after the update, its Banks-list badge says "changes not counted" (the base version has not been read yet, `c90-phantom-list`), and it changes once the page is opened. With nothing changed and version 3 unreadable while not behind (a read that failed for a moment), the strip's only button besides History is a primary "Send my changes...", and the Send page says "0 added, 0 changed, 0 removed" with an active Send once confirmed (`c90-legacy-none-*`). |
| Buttons match the text | **Hold**, except I1 (no way on the review to take a rating the page shows the circle has). |
| Layout | Every new line, card and note wraps at 760 and 420 px in both themes. The six-row review stacks cleanly at 420, and Apply merge and its counting line stay together under the last card. |
| "pool" on circle screens | **None.** Every circle frame says "circle" or "verSHion's server". The only "pool" text is on the public pool's own review, where it belongs. |
