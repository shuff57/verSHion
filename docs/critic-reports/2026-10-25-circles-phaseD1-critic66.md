# Circles phase D1, product/UX critic 66 (as a non-technical teacher)

Scope: uncommitted phase D1 on `shared-pool-phase1` (HEAD 142e6d6), after the critic 64 fixes: `src/share/bank_links*.rs`,
`circles_ui.rs`, `circles.rs`, `circle_edit*.rs`, `browse*.rs`, `publish*.rs`, `shared_ui.rs`, and the hooks in
`src/bankui.rs`. Spec: `docs/circles-phaseD-plan.md` sections 4, 11, 12, 15.

Evidence: the QA rig (`scratchpad/qarig`) was rebuilt against the current tree (15:10). I rendered 34 rig modes at 1280, 760
and 420 px in light and dark on my own Xvfb `:78`, 203 frames in all, into
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c66shots/`. One
frame timed out (`d-offline-420-1`). Contact sheets are in `.../scratchpad/c66sheets/`. Findings marked (code) come from
reading the code and are not in a frame. The only repo file changed is this report. Nothing was committed, and ports 8799
and 18791 were not touched.

Not re-reported, as the brief says: the "adding..." rows need a `banks_body` hook that is not approved yet, and list rows
overflow at 420 px.

## Blockers

None. I found no path that loses unsent work, shows a tracked unsent change as up to date, or shows full-form detail to
a viewer of a hiding bank.

## Important

### I1. "Add a bank..." and "Add to my banks" sit together and go in opposite directions
`circles_ui.rs:757-765`, frames `d-circle-editor-*`, `d-circle-owner-*`. The Banks card opens with a green "Add a
bank...", which shares one of *my* banks into the circle. Right under it, a paragraph explains "Add to my banks", and every
row has a green "Add to my banks", which takes the *circle's* bank into mine. Both are green "Add" buttons and they go
opposite ways. A teacher who wants to "add a bank to the circle" can easily press the wrong one, and the reverse.
**Change:** label the card button "Share one of my banks here..." with the hover "Puts a copy of one of your banks into this
circle". Keep the verbs paragraph next to the rows, below the button.

### I2. A linked bank is still recorded as an import, which contradicts the strip (code)
`circles.rs:3445` records every non-own add with `record_circle_copy`. Two places then speak against the link:
- The Shared > Imported tab (`browse_ui.rs:480-483`) says of a bank added with "Add to my banks": "A copy from the
  circle "Algebra team". Nothing you change here goes back to the circle." For an edit link that is false.
- Once the Shared page has checked heads (`shared_ui.rs:251-253` calls `apply_circle_heads`, `browse.rs:1913`), the
  linked bank's toolbar turns into "Share (update) ▾" with "Update from pool..." (`bankui.rs:629-633`, `:699`). That
  sits right above a strip that says "Version 9 is the latest... Get latest". Pressing "Update from pool..." goes through
  `pending_update` to `open_item_for_update` (`browse.rs:997-998`, `:1244-1245`). That function opens the circle bank's
  id as a **public pool item**, so the teacher gets a not-found page in the pool browser.

The same happened to working copies before phase D, but D1 makes this the main path for everyone who adds a bank.
**Change:** (a) in `land`, do not call `record_circle_copy` when a link was made (`out.link.is_some()`), and drop the
record for a bank when it gets linked. Or (b), at minimum, have `update_for_bank`, `update_count` and the Imported row
skip records whose bank has a circle link, and have `open_item_for_update` refuse `r.circle.is_some()`. For a still-linked
record, the Imported row should say "Linked to the circle "X": open it in My Banks to send or get the latest."

### I3. Preview ends in a dead end
`circles_ui.rs:1575-1628`, frames `d-preview-*`. The teacher opens Preview to decide, but the page has no "Add to my banks"
and no "Make my own copy". They have to go back, find the row and press it there. The preview also hides the answers
from editors and the owner, who get them anyway once they add the bank, and the answer key is exactly what a teacher
checks before adopting a quiz.
**Change:** under the title line, draw the row's own buttons (Open in My Banks, or Add to my banks + Make my own copy, by
the same rules as `banks_section`). For a reader who will get the full bank (`!head.keys_hidden`), show the correct
choice the way the bank page does ("A. 4 ✓") and change the line to "Read only: nothing here changes the bank."

### I4. Results of a background add show only on the Shared page (code)
Adds now finish in the background (`shared_ui.rs:68-70`, `browse_ui.rs:84`). That part works. What goes wrong is what the
teacher is told afterwards:
- A failed add (`AddState::Failed`) shows only in the circle row and the join list (`circles_ui.rs:524`, `:868`).
  `adding_rows_of` filters out failures too (`bank_links_ui.rs:35`), so even after the pending hook a failure just
  disappears.
- "The bank was added, but N of M pictures couldn't be fetched, so it is not linked..." and "...stays in My Banks but is
  no longer connected to the circle" (`circles.rs:3282`, `:3296-3299`) go to the circles model's `notice`. The teacher
  only sees them on returning to Shared. Meanwhile the new bank appears in My Banks with no badge and no strip, and the
  teacher assumes it is connected.

**Change:** keep a small "last add results" list in the circles model (failed, or added-but-not-linked) and give the
same planned Banks-list hook a dismissible line per result, for example ""Unit 3 test" couldn't be added: <reason>. Try
again from the circle's page." When the owner approves that hook, it should draw these results, not only Waiting and
Working.

### I5. After "added, not linked (pictures missing)", the row still offers a green "Add to my banks"
`circles_ui.rs:793-796` with `:846-848`, frame `d-circle-editor-1280-0` (Linear equations starter). The row says "Added as
"Linear equations starter", not linked (pictures missing). Open it" and still shows the primary "Add to my banks".
Pressing it makes a second bank with the same name. The longer advice ("Delete it and press Add to my banks again when
the pictures can be fetched") is only in the notice, not in the row.
**Change:** when `job` is `Done { linked: false }` and `!own`, draw "Add to my banks" as a ghost button. Put the advice
in the row: "Added as "X", not connected: N pictures couldn't be fetched. Delete it in My Banks, then add it again." Name
a retry "X (2)", or ask before adding a second bank with the same name.

### I6. Left, removed and closed: a red box with only "..." in it, and most of its menu fails
`bank_links.rs:222-233`, `bank_links_ui.rs:289-304`, frames `d-gone-*`, `d-closed-*`, `d-removed-*`. The I4 fix from critic
64 took out the big Unlink but left nothing visible to do. The menu still offers "History of the bank" and "Circle page".
For someone who left, both lead to an error (a former member gets 404). The red wash also says "something broke",
although the bank works as a normal bank.
**Change:** draw these three states in the neutral strip colour. Add the sentence "You can keep using it as your own
bank." Show "Unlink..." as a ghost button (never primary). For `l.gone()`, hide "History of the bank" and "Circle page"
in the menu (keep "Check now", in case they rejoin).

### I7. "Pool" wording and a dead-end hint on the send page that the strip now opens
- `circle_edit_ui.rs:178` "The pool checked these questions...", `:194` "The pool did not say which question", `:220`
  and `circles_text.rs:64` "the pool makes a viewers' copy", and `circle_edit.rs:1106-1107`, `:1117`, `:1128` ("as the
  pool allows", "The pool does not allow that"). A teacher sending to a private circle reads "pool" as "public".
  `circle_words` (`circle_edit_ui.rs:450`) exists but is applied only to merge notes. Change these to "the circle" or
  "verSHion's server".
- `circle_edit_ui.rs:344`: when sign-in fails mid-send, the notice says "Open my sharing settings from the top of this
  page". On the bank page there is nothing like that. Change it to "Your sign-in ended. Press Sign in... above, then send
  again." and make the strip show its Sign in button (`StripButton::OpenAccount`) while this notice is up.

### I8. `src/bankui.rs` goes beyond the approved one-line hooks
`bankui.rs:1416-1421` adds two more lines: the "..." menu re-centred on the row, and `ui.advance_cursor_after_rect`. They
look right in `d-banks-list-*`, but section 15 approved "three one-line hooks". Critic 64 polish 15 said to ask the owner
first. Get the owner's yes before the commit, or take those two lines out.

## Minor

1. Names: "Add to my banks" (lower case) beside "Open in My Banks" and "In My Banks as", while the sidebar says "Banks" and
   the page says "Question banks". Choose one form (for example "Add to My Banks").
2. Preview lists the choices as "1. 4", "2. -4", "3. 8". In a maths bank these read as 1.4 and 2.-4. Use A., B., C. as the
   bank page does (`circles_ui.rs:1611` `body_lines`).
3. The viewers'-form strip (`bank_links.rs:268`) and the held strip (`:255`) leave out "Shared with <circle>" and the
   version, and have no History. The held strip shows a green "Send my changes..." even with 0 changes. That opens
   "send on top", which replaces what the circle has. Only make it primary when `changes.total() > 0`. When an editor sees
   the promoted viewers' form (`d-promoted`), the strip doesn't say why "Add a full copy..." is offered. Add: "You can
   edit now: add a full copy to get the answer keys and send changes."
4. The offline strip (`bank_links.rs:244`) throws away what the link knew. It should keep "Version 7 is the latest. You
   have 3 unsent changes." and add "Couldn't check the circle just now."
5. "Uncounted" (`:288`) offers Get latest when the copy is not behind. Clicking it only answers "There is nothing to
   get."
6. The bank page has two "..." buttons about 200 px apart (the toolbar's and the strip's). While History, Send or Get latest
   is open in place of the questions, the toolbar's green "Add question" stays live above it. At 420 px, "Add question"
   and "Send my changes..." are two primaries stacked on top of each other. Spec 4.8 ("'...' holds History" at 420) is not
   done: the "..." wraps onto a line of its own (`d-unsent-420-0`).
7. Join: "Open the circle" is primary while "Add the ticked banks" is also primary (`d-join-*`). Make it a ghost button
   until the offer is started or dismissed. After Add, unticked banks drop off the list without a word
   (`circles_ui.rs:512-514`). Show them as "not added".
8. The circle page row reads the link's remembered role, not the role the page has just loaded. In `d-circle-viewer-*` a
   viewer sees "Nothing to send." (`circle_edit_ui.rs:1086`). Pass `d.circle.role()` into `standing`.
9. A viewer's follow copy has a bare badge ("Circle · Algebra team", the Fractions card). Consider "· view only".
10. Bank settings says the keys line twice: "Everyone in this circle can see the answer keys..." (`circles_ui.rs:1531`)
    and, in the switch card, "Everyone in this circle sees the answer keys, viewers too."
11. Kept-keys wording (`bank_links.rs:261`): "To keep them, Get latest is not offered." would read better as "Get latest is
    turned off here so that you keep them."
12. `circles_text.rs:10` tells editors to share to a circle with "Share to pool...". The label is correct but the wrong
    word for a private circle (the label itself predates D1).
13. The author's row "Your original bank is not linked. Add the circle's bank..." (`circles_ui.rs:824`) doesn't say that
    this makes a second bank with the same name. Add "(this adds a second bank; you can delete your original
    afterwards)".
14. The list overflow known at 420 px also happens at 760 px with a long name. In `d-banks-list-760-1` the Geometry row
    runs off the edge and its "..." can't be seen. This is the same old cause, so it is noted here only.

## What works

- **The verbs.** "Add to my banks" and "Make my own copy" now each come with a plain sentence: on the card (worded
  separately for editors and viewers) and on hover.
- **The badge.** It reads cleanly in cards and list rows at 1280, 760 and 420 in both themes: "newer version · 1 unsent",
  "no longer in the circle" in the warning colour, and the circle name shortened before the state.
- **The strip sentences.** Short and plain: "Your copy is up to date.", "Press Send to see what you changed.", "Sign in...",
  "Couldn't check the circle just now. Your bank works as usual."
- **Primary buttons.** History and Unlink are never primary.
- **No jump.** A check no longer collapses the strip.
- **Unlink.** The confirm text is honest about making a new bank.
- **Get latest and History** open in place with "< Back to Unit 2 quiz".
- **Bank settings** no longer claims to show questions, and has Open in My Banks or Add to my banks.
- **Share window.** It greys out the circle the bank is already in.
- **Copy names.** Own copies and full copies get distinct names ("(my copy)", "(with answer keys)").
- **Join list.** It lists already-added banks as "already in My Banks as ...".
