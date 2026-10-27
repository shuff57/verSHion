# Circles phase D1, product/UX critic 68 (as a non-technical teacher)

Scope: the uncommitted phase D1 on `shared-pool-phase1` (HEAD 142e6d6), after the fixes for critics 64 and 66. That
covers `src/share/bank_links*.rs`, `circles_ui.rs`, `circles.rs`, `circle_edit*.rs`, `browse*.rs`, `publish*.rs` and
`shared_ui.rs`, plus the hooks in `src/bankui.rs`. Spec: `docs/circles-phaseD-plan.md` sections 4, 11, 12 and 15.

## Evidence

- **Rig.** The QA rig (`scratchpad/qarig`, built 15:57) was newer than every file in `src/`, so it matched the current tree.
- **Frames.** On my own Xvfb `:68` I rendered 26 rig modes at 1280, 760 and 420 px in light and dark: 156 frames in
  `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c68shots/`.
  All exited 0.
- **Extra rig.** I built a scratch copy, `scratchpad/c68rig`, with send-page modes (`d-send`, `d-send-payload`,
  `d-send-account`) and a scripted click (`QA_CLICK`) to open the strip's "..." menu in the left, closed and removed states.
  That added 24 more frames plus the three `click-*-menu` frames.
- **Contact sheets.** They are in the same scratchpad: `c68sheets_strips1280.png`, `c68sheet_420strips.png`,
  `c68sheet_menus.png`, `c68sheet_send.png`, `c68sheet_join.png` and `c68sheet_pages.png`.
- **Imported tab.** It could not be drawn headless: it sits behind the signed-in Shared page, and `browse_ui::tab_body` is
  `pub(super)`. That tab is judged from the code only. Findings marked (code) are not in any frame.
- **Rules kept.** No repo file changed except this report. Nothing was committed. Ports 8799 and 18791 were not touched.

Not re-reported, as the brief asks: critic 66 minors 4, 6, 9, 12 and 14, and the list overflow at 420 px. The 760 px list
frame (`d-banks-list-760-1`) matches critic 66's: no worse.

## Blockers

None. I found no path that loses unsent work, shows a tracked unsent change as up to date, or puts full-form detail in
front of a viewer of a bank that hides its keys. Preview shows answer keys only when the bank was served in full
(`circles_ui.rs:1639`). Viewers of a hiding bank get the stripped form and the "Answer keys hidden" banner
(`d-preview-hidden-*`).

## Important

None. Every critic 66 Important now reads right in the frames:

- **I1.** The circle card's button says "Share one of my banks here...", with a hover that says where the bank goes.
  The paragraph explaining the two verbs comes next, then the rows.
- **I3.** Preview carries the row's own buttons, letters the choices A., B., C., and marks the correct one.
- **I5.** A bank that was added but not connected now has a ghost "Add to My Banks", with the reason and what to do in
  the row.
- **I6.** Left, removed and closed strips use the calm strip colour and say "You can keep using it as your own bank."
  Unlink is a small button. The "..." menus open (`c68sheet_menus.png`) with only items that work:
  - left: "Check now";
  - closed: "Circle page" and "Check now". `getCircle` serves closed circles, because `circleOrMiss` has no
    `requireOpen`;
  - removed: "Circle page" and "Check now".
- **I7.** The send page's main sentences no longer say "pool".
- **I2.** The Imported tab no longer drives a linked bank into "Update from pool..."; this was checked in the code.

## Minor

1. **"Pool" is still reachable on the send page and the share-into-circle page.** This finishes critic 66 I7.
   - The send page's problem rows use `publish_text::issue_text_circle`, which falls through to `issue_text`. Eight codes
     there still say "the pool": `forbidden_char`, `bad_image_file`, `bad_license`, `bad_id`, `bad_form`,
     `unknown_kind`/`ambiguous_kind`, and the catch-all "the pool refused it (...)" (`publish_text.rs:194-217`).
   - Refusals that `circle_edit::problem_of` leaves unmapped also show their pool text, for example `ImageRefused`:
     "The pool did not accept this picture".
   - "Share one of my banks here..." opens the share window. It still says "The pool checked these questions..."
     (`publish_ui.rs:992`) and "The pool already had this send" (`:946`).
   - Cheapest fix: run `circle_words` over `issue_text_circle`'s result and over the unmapped messages.
2. **The send page contradicts itself when the server lists no question** (`circle_edit_ui.rs:178`, `:194`, frame
   `d-send-payload-*`). First it says "verSHion checked these questions for the circle and needs you to change the items
   below". There are no items below. Then it says "verSHion did not say which question." To a teacher, that is the app
   arguing with itself. Suggested wording: when `issues` is empty, show one sentence: "The circle didn't accept this
   version, and verSHion's server didn't say which question. Check the long texts and the pictures, then send again."
3. **Failure rows on the Banks list can give an instruction that can't work** (`bank_links_ui.rs:47`, frame
   `d-banks-results-*`).
   - Every failure ends "Try again from the circle's page.", also when `p.retry` is false. With `NotFound`, the line reads
     "...not available to you any more... you may have left it... Try again from the circle's page." With
     `FULL_COPY_KEYLESS` it says "...Nothing was changed. Try again...".
   - The circles model's messages can also bring "pool" onto the Banks list: "The pool is being upgraded", "The pool does
     not allow that right now" (`circles.rs:1306`, `:1326`).
   - Fix: add the sentence only when `p.retry`, and give the row a "Go to the circle" link (`shared_ui::open_circle`).
   - The not-connected row has no "Open it" link and no remedy. The circle row has both ("Delete it in My Banks, then
     add it again").
4. **The results rows don't look like problems, and their button floats** (frame `d-banks-results-1280-0`). Failed and
   not-connected rows use the same quiet card and text colour as "Adding...", and "Dismiss" sits right after the text,
   in the middle of a full-width card. Right-align Dismiss. A small warning mark on the two result rows would do. A red
   wash is not wanted here.
5. **The share chooser undoes I1's split of verbs** (`shared_ui.rs:356`). After "Share one of my banks here...", the page
   heading is "Which bank do you want to **add** to the circle?". Make it "Which bank do you want to share with the
   circle?". The share result still says "Its members can open it and get a copy" (`publish_ui.rs:944`). Better: "Its
   members can add it to My Banks or make their own copy."
6. **Imported tab, linked row: the text and the button disagree** (code, `browse_ui.rs:486` with `:510`). The line says
   "open it in My Banks to send or get the latest", but the only button is "Open in the circle". That only affects
   working copies made before phase D, which includes the owner's own test data. Offer "Open in My Banks" (the Shared
   page already handles `Outcome::OpenBank`), or say "open it from the Banks page".
7. **The author's licence line points to a page that no longer exists** (`circle_edit.rs:214`). It says "...you can
   change it from the bank's page in the circle." The teacher is on the bank's page, in My Banks, when they read it.
   Change it to "...from Bank settings on the circle's page."
8. **The removed strip is long, and it is the only strip that doesn't name the circle** (`bank_links.rs:239`, frames
   `d-removed-*`). It runs four sentences: "...Your copy stays as it is. Ask the circle's owner... You can keep using it as
   your own bank." The second and fourth sentences say the same thing. Suggestion: "This bank was removed from Algebra
   team (it can be brought back until 3 Nov; ask the owner). You can keep using it as your own bank."
9. **The promoted viewers'-form strip makes the wrong step primary** (`bank_links.rs:276-282`, frame `d-promoted-*`). The
   line ends "add a full copy to get the answer keys and send changes", but the green button is "Get latest" and "Add a
   full copy..." is the ghost. The follow-copy case at `:288` already puts AddFullCopy first. Do the same here.
10. **The session ending in the middle of a send leaves no way to sign in on that page** (code, `circle_edit.rs:1139`,
    `:1832`). The send page goes back to the note and shows "Your session ended. Sign in again...". It has no Sign in
    button, and the strip that has one is hidden behind the page until "< Back". Set `ask_account()` there too, or draw a
    "Sign in..." button under that notice. Also, the Failed stage's button says "Open account..." while the strip says
    "Sign in..." (frame `d-send-account-*`). Use one label.
11. **Bank settings offers only "Add to My Banks"** (`circles_ui.rs:1597`). The row and Preview also offer "Make my own
    copy". Either add it here or leave it out on purpose. The button also touches the "Answer keys for viewers" card with
    no gap (`d-settings-760-0`).
12. **Preview, small things** (`circles_ui.rs:1653`, `:1743`). The points come before the stem ("1. (1 point) Solve
    3x - 4 = 11"). "Solve 3x - 4 = 11 (1 point)" reads more naturally. The correct choice is marked by a plain ✓ in the
    body colour. The bank page shows it bold and green, so it is easy to miss here.
13. **Process.** `src/bankui.rs` now has three call-sites plus two layout lines. Section 15 approved "badge, status strip,
    question-menu History item". `adding_rows` (`bankui.rs:462`) and the re-centred "..." with `advance_cursor_after_rect`
    (`:1417-1422`) cite "owner decision (b)" and critic 66 I8 in comments, but the plan doesn't record either approval.
    Write it into section 15 before the commit.

## Brief checklist

| Item | Verdict |
|---|---|
| Add vs copy verbs | Clear: one paragraph on the card, a hover on each button, viewer wording separate. The chooser heading drifts (minor 5). |
| "Share one of my banks here..." | Right label and hover; the chooser and result text behind it still say "add" and "pool" (minors 1, 5). |
| Preview with Add/Copy | Done; keys shown to full readers, hidden to hiding-bank viewers; small polish (minor 12). |
| Adding / failure / not-connected rows on Banks | Present and dismissible; wording and affordance gaps (minors 3, 4). |
| Left / removed / closed strips | Calm colour, Unlink small, menus hold only working items (checked by click). Removed line wordy (minor 8). |
| Send page wording, no "pool" | Main sentences done; issue rows and an empty-issues case remain (minors 1, 2). |
| "My Banks" naming | Consistent as "My Banks" in every new string; "Share one of my banks" is natural sentence case. Nav "Banks" / page "Question banks" unchanged (protected or pre-existing). |
| Imported tab text | Linked rows say so and lose "Update"; the button contradicts the line (minor 6). |
| Row badge / "..." layout in bankui.rs | Cards: badge under the title, "..." top-right, fine at all widths. List: "..." centred on taller linked rows, next row starts below; same as critic 66. Approval to record (minor 13). |

## What works

- The circle page reads as setup plus a list. Nothing on it edits. "Open in My Banks" is the primary on rows already in
  My Banks.
- The strip's 15 or so states each say one plain thing. The big button is always a real next step, never History or
  Unlink. The strip wraps cleanly at 420 px in both themes.
- Join: "Open the circle" is a ghost button, and unticked banks are listed as "not added" once the add is done.
- The Banks list shows adds that are still running, and their results, without a visit to Shared.
- Get latest, History and Unlink read "Get latest", "matches" and "Unlink" everywhere I looked. No "in step" and no
  "working copy" were left on these screens.
