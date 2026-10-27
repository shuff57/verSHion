# Circles phase D1, product/UX critic 86 (as a non-technical teacher), round 11

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 11. Round 11 changed:

- a question's difficulty is merged, counted and shown (Get latest review, strip, badge, Send summary);
- "You sent this in version N" is said only of questions that send changed (critic 84 I2);
- the person's own version right after the send's base is the merge's base, also when colleagues sent more since (fewer
  false conflicts);
- the "may not have arrived yet" note is rewritten by the later check that settles it (critic 84 I1);
- "Yours now" vs "In version N (the circle's newest)" lines for a question that was added and then corrected.

Checked here: is the wording clear to a teacher; do the strip, the badge, the Send page, the Get latest page and the
conflict box contradict each other; do the buttons match the text; how difficulty is worded; is any "pool" left on a
circle screen. Only blockers and important findings are reported. A fresh critic who built none of this wrote this report.

## Evidence

- **Fresh copy and rig.** `scratchpad/c86/w`: the repo's `src/share/` and `src/bankui.rs` as they are now (checked byte for
  byte), on the compiling base the round 11 builder used (the other session's files at their earlier versions,
  `gradeui.rs` left out). The only changes are one added module, `src/share/qa_c86.rs` (cfg(test)), its `mod` line, and
  `winit` as a dev-dependency in the copy's `Cargo.toml` (to open an eframe window from the test thread). Earlier critics'
  rigs were gone, so this one was written new.
- **How states are reached.** Through the real models with the round tests' scripted sessions (`sent_with`,
  `latest_as`, `conflict_by`, `restore_answered`, ...). The window draws the real `BankLinks::badge` + `badge_line` on a
  Banks-list card, then the real `bank_links_ui::strip_with` (and so the real Send, Get latest and History pages).
  36 modes: difficulty changed by a colleague / by the teacher / by both / against a colleague's rewording / on a link an
  earlier build wrote (then Keep mine); the teacher's 4 then Bob's 5 (changed, added, and both edited); questions carried
  unchanged (check path and 409 path); changed again; added then corrected (review, after Apply, Send page); "may not have
  arrived yet" then a later check by the teacher or Bob (strip, review, Apply, Send page); a restore that landed later
  (strip, History); a later 409.
- **Frames.** My own Xvfb `:86` (stopped when done): 36 modes at 1280, 760 and 420 px, light and dark, 216 frames, all
  exit 0, none blank. Frames in `scratchpad/c86/shots/`; the text of every mode in `scratchpad/c86/text*.txt`.
- **Suite.** `cargo test --lib share::` on the copy: 1045 passed, 0 failed.
- **Rules kept.** The only repo file written is this report. Nothing committed. Port 8799 not used. The rig on 18791
  left alone.

## Blockers

None. No path loses unsent work or sends anything twice. Every merge still saves a restore point first.

## Important

### I1. A link written by an earlier build: a colleague's difficulty change shows as "CONFLICT: both changed" with "In version 3: (the earlier version is not kept)", while the strip says nothing is unsent. "Keep mine" then undoes the colleague's rating on the next send.

Frames `diff-legacy-strip-*`, `diff-legacy-merge-*`, `diff-legacy-keepmine-strip-*`.

Every link made before round 11 has no difficulty in its base (`Snap.difficulty` = not known). The teacher changed
nothing. Bob rates question 1 "Hard" in version 4. Then:

> Strip: Shared with Algebra team. Version 4 is the latest. You have version 3. **[Get latest]** (badge: "newer version", no unsent count)
>
> Get latest: 1 conflict (both changed) · **CONFLICT: both changed** · One · Difficulty
> In version 3: **(the earlier version is not kept)** · Yours now: none · In version 4 (the circle's newest): Hard
> ( ) Keep mine ( ) Use the circle's version ( ) Keep both ... · Choose one to continue.

Two statements on that page are false. The teacher did not change the question, and the strip and badge had just said
so. Version 3 is kept; only this computer has no record of its difficulty. A teacher told "both changed" tends to answer
"Keep mine". After that the strip reads "Version 4 is the latest. You have 1 unsent change." with Send my changes, and
that send puts the difficulty back to none, undoing Bob's rating (`diff-legacy-keepmine-strip`).

**Cause.** `merge.rs` around 690: with an unknown base difficulty and a difference, both sides are flagged, so the row
is a `Conflict`. The diff's base text is `NOT_KEPT` (merge.rs 466/510), a message written for a different case. Making
it a decision is right (a pre-round-11 copy may hold a difficulty change that was never counted). The wording is the
problem.

**Why it matters.** It hits every existing linked bank once per question a colleague re-rates, until the next send or
merge rewrites the base. The page contradicts the strip, and its wording pushes the teacher towards undoing a
colleague's work.

**Fix.**
- When the only reason for the decision is a base difficulty that is not known, do not label the row "both changed" or
  count it under "conflict (both changed)". Use its own label and note, for example: "This computer has no record of
  this question's difficulty in version 3, so verSHion can't tell who changed it. Choose which to keep."
- Base line: "In version 3: not recorded on this computer", never "(the earlier version is not kept)".
- Optional: make "Use the circle's version" the suggested choice when the teacher's copy still has the difficulty that
  every earlier record shows.

**Test.**
- A link with no base difficulty, the teacher unchanged, and the circle's version 4 changing only the difficulty: the
  review has no "both changed" and no "is not kept", names the missing record, and Apply stays blocked until a choice.
- The strip before it says no unsent change.

### I2. With the teacher's own version as the base, a real conflict shows only two of the three versions

Frames `own-then-bob-both-merge-*`.

This is the case round 11's own-version base makes common. The teacher's send of "One (fix)" lost its answer, and the
check found her version 4. She edited the question again ("One (fix 2)"). Bob then changed it in version 5 ("One (fix)
by Bob"). The conflict card says:

> **CONFLICT: both changed** · One (fix 2) · Question text
> Yours now: One (fix 2) · In version 5 (the circle's newest): One (fix) by Bob

There is no line for the version both started from (her version 4, "One (fix)"). Every other conflict card shows it ("In
version 3: ...", `diff-conflict-merge`). Without it, a teacher can't see what Bob changed or what she changed since. With
a real paragraph-long question she has to guess before choosing Keep mine (Bob's edit lost) or Use the circle's version
(her edit lost). The same rows that are "Changed in the circle" also drop the line. That is harmless there, because
"Yours now" equals the base.

**Cause.** `circle_edit_ui.rs` around 719-722: `two_way = r.fallback || r.resent.contains(&row.base_id)`. Critic 80's rule
(a question as the person's own send carried it is the circle's newest, so it is not shown under the old version's
number) still assumes the own version is the newest. Since round 11 it can be an older one.

**Fix.** When the own version is the base and is not the newest, show the base line as "In version 4 (your send):
<text>". Keep the two-line form only when the own version is the newest.

**Test.** Own version 4, the teacher edits the same question again, Bob's version 5 edits it. The conflict card has three
lines, including "In version 4 (your send): One (fix)". When the own version is the newest (critic 80's case), there is
still no old-version line.

## Brief checklist

| Item | Verdict |
|---|---|
| Difficulty in Get latest | **Holds** for links made by this build (`diff-theirs-*`): "Difficulty / In version 3: none / Yours now: none / In version 4 (the circle's newest): Hard". These are the editor's words (Easy, Medium, Hard), and "none" for unset matches Points. After Apply: "Your copy now matches the circle's newest version. There is nothing to send." A difficulty against a difficulty is a clear three-line conflict (`diff-conflict-merge`). **Not met** for links an earlier build wrote (I1). Not reported (documented limit 7): the teacher's difficulty against a colleague's rewording is a question-level conflict, so one of the two must be redone by hand (`diff-vs-stem-merge`). |
| Difficulty counted (strip, badge, Send) | **Holds:** badge "1 unsent change", strip "Version 3 is the latest. You have 1 unsent change." with a primary Send my changes, Send page "0 added, 1 changed, 0 removed", result "Version 4 is now the circle's newest. 0 added, 1 changed, 0 removed." (`diff-own-*`). The Send page does not name the question or the field (phase D3 scope). |
| "You sent this" only for what the send changed | **Holds; critic 84 I2 is fixed.** A question carried unchanged and edited later reads "Only you changed this. It stays as it is." with three lines, on both the check path and the 409 path (`carried-unchanged-*`). Changed again and added-then-corrected keep "You sent this in version 4 and changed it since" (`changed-again-merge`, `added-corrected-merge`). |
| Own version as base | **Holds** when only Bob changed it since (`own-then-bob-merge`, `own-then-bob-added-merge`): "Changed in the circle", Take the circle's version, no conflict, nothing added twice. **Not met** for a real conflict (I2). |
| "May not have arrived yet" rewritten | **Holds; critic 84 I1 is fixed.** A later check finding the teacher's 4: "The check found version 4 in the circle, sent by you: your send went through. Press Get latest to catch up." under "Your changes were sent. Get the latest ..." with a primary Get latest. Finding Bob's: "Bob Teacher sent version 4 first, so your send did not go through ..." under "You have version 3 and 1 unsent change". Restore: "The check found a newer version in the circle, version 4. It may be the restore of version 1: open History to see." with Get latest and History. A later 409 is not written over. No "still has version 3" or "press Send my changes" is left anywhere. |
| "Yours now" vs "the circle's newest" (added then corrected) | **Holds:** "Yours now: Solve 5x = 25 / In version 4 (the circle's newest): Solve 5x = 20". After Apply: "Version 4 is the latest. You have 1 unsent change." The Send page then says "0 added, 1 changed". |
| Strip vs badge vs Send vs Get latest vs conflict box | **Agree**, except I1 (strip "nothing unsent" against the review's "both changed"). Not reported (minor): after the person's own 409 the strip says "You have version 3 and 2 unsent changes" and the badge "2 unsent". The added question in that count already went in version 4, and the check path in the same state says "Your earlier changes were sent. Changes you made since are not sent yet." (`carried-conflict-strip` vs `sent-then-edit-strip`). Get latest is primary in both and puts it right. |
| Buttons match the text | **Hold** in every mode. |
| Layout | Every new line wraps at 760 and 420 px in both themes. Conflict cards stack, and Dismiss and the buttons stay on screen. |
| "pool" | **None** in any frame or in round 11's user-facing strings. |

Polish, not reported:
- The restore note says "It may be the restore of version 1" even when the check named another teacher as the sender of
  version 4. A send in the same case says "did not go through".
- The changed-in-circle row offers "Take the circle's version", while the conflict row offers "Use the circle's version".
- A note above an open Get latest page still says "Press Get latest to catch up".
