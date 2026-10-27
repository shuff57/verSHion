# Circles phase D1, product/UX critic 92 (as a non-technical teacher), round 14

Scope: HEAD 0d17a83 on `shared-pool-phase1` (the local commit that holds all of phase D1). Round 14 changed:

- "Changed in the circle" rows where a difficulty is not recorded: nothing chosen at the start, a red pill until a choice
  is made, and a note saying that Keep mine drops the circle's changes (critic 90 I2);
- a copy linked by an earlier build takes the circle's difficulty where it has none and the teacher has not edited
  the question since (the owner's decision, critic 90 I1);
- counts for such links after the one-time read of the base version (critic 89);
- the public pool's Update review uses the same rows.

Checked here, as a non-technical teacher: whether the wording is clear; whether the strip, the badge, the Send page, the
Get latest page and the circle page contradict each other; whether the buttons do what the text says; whether a teacher
can finish Get latest; whether "pool" appears on any circle screen. I also took a fresh look at the whole D1 flow: add a
circle bank to My Banks, edit, send, get latest, preview, make my own copy, join, leave. Only blockers and important
findings are reported. A fresh critic who built none of this wrote this report.

## Evidence

- **Fresh copy.** `scratchpad/c92/w` is `git archive HEAD` (0d17a83). The other session's uncommitted files are not in
  it. Changes in the copy: `winit` as a dev-dependency, one module `src/share/qa_c92.rs` (cfg(test), adapted from
  critic 90's rig) with its `mod` line, and a 5-line cfg(test) shim at the end of `browse_ui.rs` so that the rig can
  draw the public pool's real `review_page`.
- **States.** All are reached through the real models with the round tests' scripted sessions. The rig keeps critic 90's
  modes and the builder's `r14-*` modes. I added 40 of my own (`c92-*`):
  - a link the phase C build wrote, with no record of a read (`difficulty_read: None`, as in the round's own
    `phase_c`): the copy takes the circle's rating, she cleared the rating later, she has her own rating, and she fixed
    a typo in the question that lost Bob's rating. Each of these is shown on the Banks list, the strip, the Send page,
    Get latest and after Apply;
  - four questions with version 3 no longer kept: two reworded by Bob (one also re-rated) and one only re-rated, shown
    undecided, half decided and applied;
  - Keep mine or Take on a "Changed in the circle" row, then the strip and the Send page;
  - the pool review with Take chosen, or with the teacher's own rating;
  - the D1 flow: the circle page as editor, viewer and owner; Preview; after Add (the circle page, the strip, the strip
    with an edit, Leave asked); after Make my own copy (the circle page and the Banks list with its lineage line); a
    viewer's strip; the banks being added on the Banks list; the join offer with three banks; and the strip after
    leaving, with and without an unsent change.

  Circle names differ between screens ("Biology team" against "Algebra team"). That comes from the fixtures.
- **Frames.** On my own Xvfb `:92` (stopped when done): 29 modes at 1280, 760 and 420 px, light and dark. That is 174
  frames, all exit 0 and none blank. They are in `scratchpad/c92/shots/`. The text of 62 modes is in
  `scratchpad/c92/text1.txt` to `text4.txt`.
- **Suite.** `cargo test --lib share::` on the copy: 1130 passed, 0 failed.
- **Rules kept.** The only repo file written is this report. Nothing was committed. Port 8799 was not used. The rig on
  18791 was left alone.

## Blockers

None. Every merge still saves a restore point first. Nothing is sent twice. On every page I reached, no default throws
away an unsent change of the teacher's.

## Important

### I1. Bob's rating that the phase C merge dropped, on a question the teacher edited since: "Only you changed this", no choice, and her next send removes his rating

Frames `c92-legacy-typo-strip-*`, `c92-legacy-typo-merge-*`, `c92-legacy-typo-applied-*`.

This is critic 90 I1's case with one more step. Bob rated question 1 "Hard" in version 3. The phase C Get latest left
it out. Afterwards the teacher fixed a typo in question 1. Round 14 does not give this copy Bob's rating, because the
question was edited after the copy last matched. Then:

> Strip: Version 3 is the latest. You have 1 unsent change. (true: the typo)
>
> Get latest (Bob adds a question in version 4): **Changed by you** · One (typo fixed) · **Only you changed this. It
> stays as it is.**
> Question text · In version 3: One · Yours now: One (typo fixed) · In version 4: One
> Difficulty · In version 3: Hard · **Yours now: none** · In version 4 (the circle's newest): Hard
> (no buttons on the row) [Apply merge]

The page says she removed a rating that she never had. It gives her no way to take the circle's rating. After Apply the
strip says "1 unsent change", and her next send (to share the typo fix) sets question 1 back to no difficulty in the
circle. To prevent that, she has to notice the "Yours now: none" line and set "Hard" by hand.

**Cause.** `circle_links.rs` `inherit_difficulty` (around 415-430) skips any question whose `modified` is later than
`bank_modified`. It treats an edit of any field as a deliberate change of the rating. `merge.rs` then makes the
difference an `OursChanged` row with the fixed note.

**Why it matters.** Teachers often fix questions in their copy. Every such fix on a question that lost a colleague's
rating to the phase C merge gives critic 90 I1's harm, with a page that states something false.

**Fix (the owner chooses).**
- (a) Take the circle's rating whenever the copy has none, whatever the question's time. The only work this loses is an
  unsent removal of a rating, made before a phase C Get latest, which is very rare.
- (b) Keep the owner's rule, but where it was the only reason not to take the rating, do not call the difference hers.
  Give the difficulty its own note, for example: "Version 3 rated this Hard. An earlier verSHion did not bring ratings
  across, so it may never have reached you." Also give a way to take the circle's rating without losing the typo fix.

**Test.**
- The phase C link, with version 3 rating question 1 Hard. She edits question 1's wording. After the read, no page says
  she removed the rating. After Get latest and a send, the circle still has "Hard" unless she chose otherwise.

### I2. "Leave this circle?" speaks of "copies" and says nothing of banks in My Banks or their unsent changes

Frames `c92-flow-added-leave-ask-*` and `c92-flow-left-unsent-strip-*`.

The Banks card on the same page says `In My Banks as "Cells"`, and the strip says "You have 1 unsent change". The leave
panel says only:

> You will lose access to this circle's banks. Copies you already made stay on your computer. You can come back with a
> new code. [Leave] [Stay]

D1 never calls them copies: the verb is "Add to My Banks", and the owner asked for one name, "My Banks". A teacher can
read "lose access to this circle's banks" as "Cells leaves My Banks". The panel does not warn her that a bank with
unsent changes can't be sent afterwards. Once she has left, the strip says "Your copy stays; it can't be sent", and the
change stays unsent until the owner gives her a new code.

**Cause.** `circles_text.rs` `LEAVE_MEMBER` (120) is the phase C text. `circles_ui.rs` around 1221 draws it without
looking at the links. `CLOSE_CIRCLE`, `REMOVE_BANK` and the member-removal line at `circles_ui.rs` 1255 use the same
"copies" wording.

**Fix.**
- Say it in My Banks words, for example: "Banks you added to My Banks stay there as your own. They stop getting newer
  versions and can't be sent."
- Add one line for each linked bank of this circle that has unsent changes, for example: "Cells has 1 unsent change.
  Send it first if the circle should have it."

**Test.**
- An edit link of this circle with one unsent change: the leave panel names the bank and its count.
- With nothing unsent, the panel has no such line.

### I3. One Get latest page now has two kinds of undecided cards, with their options in opposite order and two different verbs

Frames `c92-many-merge-*`, `c92-many-half-merge-*`.

Round 14 makes "Changed in the circle" cards wait for a click. They now sit next to "Difficulty not recorded" cards that
also wait for one:

> **Difficulty not recorded** · Which of these is prime? · ( ) Keep mine ( ) Use the circle's version
> **Changed in the circle** · Solve 2x + 3 = 7 · ( ) Take the circle's version ( ) Keep mine

When a colleague has re-rated and reworded many questions (critic 90's 6-row and 30-row cases), a teacher who clicks
down the column, or who decides "the circle's version everywhere", keeps her own on some cards and takes the circle's on
others. Keep mine on a "Difficulty not recorded" card keeps "none", and her next send removes the colleague's rating.
Each card's buttons match its own note, but the cards do not match each other.

**Fix.** Use one order and one label on every card: always "Use the circle's version" and "Keep mine", in the same
order. The "may be your earlier send" card and the pool review are included.

**Test.** On a review with Conflict, undecided TheirsChanged and unrecorded rows, every card lists the circle's option
and Keep mine in the same order, with the same label.

## Brief checklist

| Item | Verdict |
|---|---|
| "Changed in the circle" with an unrecorded difficulty | **Holds.** Nothing is chosen at the start. The pill is red until a choice is made, then calm (1280 dark, 420 dark). Each card says "Choose one to continue." The line above Apply counts what is left. The note reads: "Take the circle's version to get its changes and its rating, or keep yours: the circle's changes to this question are then not kept." The summary counts it as "changed in the circle", never "difficulty not recorded" (`c90-legacy-bob-both-merge`, `r14-bob-both-taken-merge`, `r13-legacy-reworded-merge`). Keep mine leaves "1 unsent change" and Take leaves "Your copy is up to date" (`c92-bob-both-*`), as the note says. Order and labels: I3. |
| The copy takes the circle's rating (older links) | **Holds where the question was not edited.** On critic 90's phantom case, the strip says "Your copy is up to date" with "One · Hard", the Send page says "Nothing in your copy differs ...", the next Get latest is plain ("1 new in the circle - 1 unchanged"), and version 3 is read once. Her own rating is kept and counted (`c92-legacy-hers-*`). A rating she cleared later is hers (`c92-legacy-cleared-*`). Exception: I1. Not reported (owner's decision): the rating arrives silently, with no line on the strip and no restore point. |
| Counts after the one-time read | **Hold.** With version 3 readable: "You have 1 unsent change", with no caveat, on the badge, the strip and the Send page (`c88-legacy-*`, `r13-legacy-read-*`). With it not kept: the count plus the difficulty caveat, and Get latest first when behind (`r13-legacy-*`, `c90-legacy-*`). Unchanged minor (critic 90): the Banks-list badge says "changes not counted" until the bank's page is first opened. |
| Pool Update review, same rows | **Holds** (`r14-pool-legacy-reworded`, `c92-pool-reworded-*`): "1 changed in the pool", a red pill until a choice is made, nothing chosen, the same note in pool words, Apply greyed with the count. This is the public pool's own screen, so "pool" belongs there. Polish: its note says "since version 3" while its lines say "When imported". |
| Can a teacher finish Get latest | **Yes.** Four questions: 2 decisions, then Apply. The result is "Version 4 is the latest. Your copy is up to date." with Bob's rewordings and ratings in the bank (`c92-many-applied`). |
| Strip vs badge vs Send vs Get latest vs circle page | **Agree**, except I1, where the review calls a dropped rating hers. Not reported (minor, earlier rounds): a Send page with nothing to send says "the version it was made from" where the others say "since your copy last matched the circle"; after Keep mine on a circle change, the Send page says "the history shows that you merged in the circle's newer changes". |
| Buttons match the text | **Hold** card by card. Across cards: I3. |
| "pool" on circle screens | **None.** I searched the text of every circle mode. The new note reaches circle screens as "The circle changed this question ..." through `merge_words`. |
| D1 flow, fresh eyes | **Holds**, except I2. Circle page: "Add to My Banks" is the primary button, then "Make my own copy". The two verbs are explained once. More holds Preview, Report, Bank settings and Remove. After Add the row says `In My Banks as "Cells"` and "Nothing to send.", with Open in My Banks. Preview is read only, with Add and Make my own copy. Own copy: the row says `Your own copy "Cells (my copy)" is in My Banks. It is not linked to the circle.` with Open it, and the bank's details say where it came from. Join offer: "Add its banks to My Banks?", with ticks, "Add the ticked banks" and "Not now". On the Banks list, while banks are added: `Adding "Cells" from Algebra team...`. Strip: in step, unsent, viewer ("you can view"), and left ("You are no longer in Algebra team. Your copy stays; it can't be sent."). Minor leftover: the keyless-send message in `pool.rs` 719 still says "make a new working copy from the circle". |
| Layout | Every new card and note wraps at 760 and 420 px in both themes. Radio buttons, Apply and the counting line stay on screen. |
