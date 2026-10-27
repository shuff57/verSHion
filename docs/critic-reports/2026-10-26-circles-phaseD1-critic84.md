# Circles phase D1, product/UX critic 84 (as a non-technical teacher), round 10

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 10. Round 10 changed:

- the strip note after a check that found the circle still at the version a send or restore was based on: "Your send may
  not have arrived yet: the circle still has version N. ... press Send my changes to send them." and "The restore of
  version N may not have arrived yet: the circle still has version M. Open History to try again." (the check no
  longer settles the send as "did not go through");
- any answer from the circle for a bank (a send's 201, any 409, Get latest, History, a licence read) ends a stale
  "Couldn't check the circle just now" and checks again at once (critic 82 I1);
- the merge treats what an unclear send carried as the teacher's own only while the version right after its base is
  not a colleague's, so a colleague's identical added question shows as "New in the circle";
- text compared as the server stores it (Symbol-font characters, topics), unrelated to wording.

Checked here: is the wording clear to a teacher; do the strip, the badge, the Send page, the Get latest page and the
conflict box contradict each other; do the buttons match the text; is any "pool" left on a circle screen. Known
exceptions, not re-reported: the Shared page's account guide and the Account window. Only blockers and important
findings are reported. A fresh critic who built none of this wrote this report.

## Evidence

- **Fresh copy and rig.**
  - `scratchpad/c84`: the repo's `src/share/` and `src/bankui.rs` as they are now (checked byte for byte), on the
    compiling base of the other files that critics 80 and 82 used (the other session's `ai.rs`, `app.rs`, `lib.rs`,
    `model.rs`, `screens.rs`, `store.rs`, `figures/mod.rs` are the earlier versions; `gradeui.rs` left out). The only
    change in `src/share/` is one added module, `qa_c84.rs`, and its `mod` line.
  - The rig is critic 82's with 29 new round-10 modes. It draws the real `bank_links_ui::strip_with` (and so the real
    Send, Get latest and History pages) under a Banks-list card drawn by the real `BankLinks::badge` and `badge_line`;
    circle screens are the real `circles_ui::body_of`. Every state is reached through the real models with a scripted
    session: a send or restore that times out; the check answering version 3 (still) and then, on a later check
    ("Check now"), version 4 by the teacher or by "Bob Teacher"; Send pressed again (201 replayed, or a 409 naming the
    teacher after an edit); a failed check followed by the page's Try again (201 or 409); a colleague's version 4 that
    adds the same question the teacher's send carried (as sent, and after the teacher corrected it); the teacher's 4
    then Bob's 5; an edit to a question the send did not touch, then Get latest.
- **Frames.** On my own Xvfb `:84` (stopped when done): 43 modes at 1280, 760 and 420 px, light and dark (258 frames),
  plus 2 follow-up modes (12 frames). All exit 0; none blank (`c82tools/crop.py`). Frames in `scratchpad/c84shots/`
  (cropped in `c84crop/`); contact sheets `c84_s1.png` ... `c84_s14.png`.
- **Suite.** `cargo test --lib share::` on the copy: 1022 passed, 0 failed.
- **Rules kept.** The only repo file changed is this report. Nothing was committed. Port 8799 was not used. The rig on
  18791 was left alone.

## Blockers

None. No path loses unsent work or sends anything twice: a Send pressed after "may not have arrived yet" goes as the
same request (`r10-still-resend-ok`: "This send had already gone through; nothing was sent twice."), and an edited
resend meets "Your earlier send went through ..." with Get latest (`r10-still-edited-conflict`). Every merge saves a
restore point first.

## Important

### I1. "May not have arrived yet: the circle still has version 3 ... press Send my changes" stays on the strip and on the Get latest page after a later check finds version 4

Frames `r10-still-landed-strip-*`, `r10-still-landed-merge-*`, `r10-still-other-strip-*`, `r10-restore-landed-strip-*`.

This is the case round 10's wording exists for: the send was slow, the first check still saw version 3, and the send
lands a little later. The next check ("Check now", or the page opened again after 10 minutes) finds version 4, sent by
the teacher. The strip line and badge move on, but the note under them does not:

> Shared with Algebra team. Your changes were sent. Get the latest so that this copy has the version they made.
> *Your send may not have arrived yet: the circle still has version 3. Your changes are still on this computer; press
> Send my changes to send them.* [Dismiss]
> **[Get latest]** [History] [...]          (badge: "Algebra team · newer version")

Two sentences next to each other say opposite things ("were sent", "may not have arrived"; version 4 vs "still has
version 3"), and the note names a Send my changes button the strip no longer shows. Pressing Get latest carries the
note onto the review, right above "Version 4 is your own send from this computer: its answer did not come back."

When a colleague's version 4 is found instead (`r10-still-other-strip`), the line says "Version 4 is the latest. You
have version 3 and 1 unsent change. Get the latest first, then send." and the note still says "the circle still has
version 3 ... press Send my changes". Nothing says the send did not go through, which an immediate check would have
said ("Bob Teacher sent version 4 first, so your send did not go through.").

A restore has the same problem (`r10-restore-landed-strip`): "Version 4 is the latest. You have version 3." above "The
restore of version 2 may not have arrived yet: the circle still has version 3. Open History to try again."

**Cause.** `checked()` writes the after-check note once. The send note comes from `unclear_noted`, which is removed
on the first check, and the restore note from `unclear_restores`, whose `found` is set once. Round 10 made the first
answer "not settled", but nothing rewrites the note when a later check settles it. Before round 10 the first check
settled it as "did not go through", so the one-time write was enough.

**Why it matters.** Rounds 7 to 10 are about telling the teacher what happened to a send on flaky wifi. On the exact
path round 10 now allows (a slow send that lands after the check), the strip again tells two stories and names a
missing button. The primary button (Get latest) is the right one, so this is confusion, not loss.

**Fix.**
- While a send's check found "still at the base", keep it as unsettled: leave the key in `unclear_noted`, or keep a
  separate "said Still" mark. The next check that finds Mine, OtherFirst or Maybe then rewrites the note
  (`results.replace_text`) with `unclear_mine` / `unclear_other_first` / `unclear_moved`.
- Or drop the Send result once the link turns `sent_unread` or `not_through`.
- For a restore, update `found` while the earlier text was the "still" text and `head > base` now.

**Test:**
- An unclear send; a check at the base ("may not have arrived yet"); a second check at base+1 by the teacher. No result
  or strip text says "still has version" or "press Send my changes", and the note says the send went through.
- The same with a colleague at base+1: the note names the colleague and says the send did not go through.
- The same for a restore.

### I2. The merge says "You sent this in version 4 and changed it since" about a question that send never changed

Frames `r10-mine-other-edited-merge-*`, `r10-still-edited-merge-*`.

The teacher adds "Solve 5x = 20" and sends; the answer is lost, and the check finds version 4 by the teacher ("your
send went through. Press Get latest to catch up"). The teacher goes on working and fixes a different, older question:
"Solve 2x + 3 = 11" becomes "Solve 2x + 3 = 13". Get latest then shows:

> What was sent since: Version 4, by Olga Teacher, 1 added, **0 changed**, 0 removed: "Added a question"
> Version 4 is your own send from this computer ...
> **Changed by you** Solve 2x + 3 = 13
> **You sent this in version 4 and changed it since.** Your change stays here and goes with your next send.
> Yours now: Solve 2x + 3 = 13 · In version 4 (the circle's newest): Solve 2x + 3 = 11

The page says version 4 changed nothing, then that the teacher sent this question's change in version 4. Its own
compare lines show version 4 without the change. The same happens on round 10's own path: still at 3, an edit, Send,
then a 409 naming the teacher, then Get latest.

**Cause.** `SentRecord` holds every question the send carried, unchanged ones too. The `resent` loop in the review
build (`circle_edit.rs` around 3079) marks each carried question that the circle has as carried, so every
`OursChanged` row among them gets `sent_then_changed`. In a 30-question bank, every question edited after a lost answer
gets this note.

**Why it matters.** The note makes a false claim about what reached colleagues, on the page where the teacher decides
what to keep. The outcome is right (the edit is kept and goes with the next send), so this is not a loss. But it is
the "Get latest page contradicts the story" class that critics 80 and 82 counted as important.

**Fix.** Add a question to `resent`, and give it the note, only when the send changed it. That means its carried
content differs from the link's base entry (`merge::ours_changed(b, &sq.q)` before the base is replaced). Unchanged
carried questions can still move the base (no effect), but get no note. Or keep only changed and added questions in
`SentRecord`.

**Test:**
- An unclear send that adds Q5 and carries Q1 unchanged; check Mine (and, separately, a 409 by the teacher after a
  "still" check); Q1 edited; Get latest. Q1 is "changed by you" without "You sent this in version 4".
- critic 82's case keeps its note: a question changed by the send and changed again keeps it.

## Brief checklist

| Item | Verdict |
|---|---|
| "May not have arrived yet" (send) | **Holds at the moment it is said** (`r10-still-strip`): "Version 3 is the latest. You have 1 unsent change." with the note and a primary Send my changes; badge "1 unsent change"; the Send page keeps the note and "Added a question" (`r10-still-send`); a page left open still says "Press Try again: if it went through, nothing is sent twice". Resending is safe and says so on the result (`r10-still-resend-ok`, then "Your copy is up to date."). **Not met once a later check moves on** (I1). Polish, not reported: the strip note itself does not say that pressing Send cannot send twice. The page's Try again says it, and it is true here too. |
| "May not have arrived yet" (restore) | **Holds at first** (`r10-restore-still-strip`, `r10-restore-still-history`: History is on the strip and the restore can be chosen again; same commit id). **Stale after a later check** (I1). Polish: "Version 3 is the latest. Your copy is up to date." beside "may not have arrived yet" reads more settled than the note. |
| Strip re-checks after the circle answers (critic 82 I1) | **Fixed.** Try again answered 201 → "Version 4 is the latest. Your copy is up to date." (`r10-nocheck-retry-ok`); answered 409 by Bob → "Bob Teacher sent version 4 first, so your send did not go through ..." over a primary Get latest, Send, History (`r10-nocheck-retry-409`). No "Couldn't check" after an answer. |
| 409 offers Get latest | **Holds** on the page (own: "Your earlier send went through ..." / colleague: "Someone else sent changes first ...", each with primary Get latest) and on the strip after Back. |
| Colleague's same question shown as "New in the circle" | **Holds.** Uncorrected, it is linked ("You and the circle added the same question ..."); corrected to 5x = 25, the review names "Version 4, by Bob Teacher", offers "New in the circle: Solve 5x = 20" and leaves the teacher's 5x = 25 as "yours alone". The strip had said "Bob Teacher sent version 4 first, so your send did not go through". The story agrees end to end. |
| Own-send rules | **Hold** for 4 by the teacher (`r9-mine-*`), 4 by Bob (`r9-other-*`, `r10-coll-same-*`), the teacher's 4 then Bob's 5 (`r10-far-merge`: "Version 4 is your own send", Bob's typo fix offered as "Changed in the circle"), and after a restart (`r9-restart-*`). Exception: the note in I2. |
| Strip vs badge vs Send page vs Get latest page vs conflict box | **Agree** in every mode except I1 (note vs line) and I2 (merge note vs its own "0 changed"). |
| Buttons match the text | **Hold**, except I1 (the note names Send my changes when the strip offers only Get latest and History). |
| Layout | Every new line wraps cleanly at 760 and 420 px in both themes. Dismiss and the buttons stay on screen, and merge rows stack. |
| Remaining "pool" | **None on circle screens.** The round 10 string literals (`circle_edit.rs`, `bank_links*.rs`, `merge.rs`, `canon.rs`, `circle_links.rs`) add none. The only hits are the public-pool credit line and the `circle_words` / `merge_words` replacement tables, as before. No frame shows "pool". |

## What works

- After a failed check, any answer from the circle puts the strip right at once. The flaky-wifi case critic 82 found
  now ends at "up to date", or at Get latest with the colleague named.
- Pressing Send after "may not have arrived yet" is safe either way: replayed when unchanged, or the own-send conflict
  with Get latest when edited. The review then recognises the teacher's own version.
- A colleague's identical question is no longer taken for the teacher's own send. The review names the colleague, and
  the teacher's correction stays theirs.
