# Circles phase D1, product/UX critic 64 (as a non-technical teacher)

Scope: uncommitted phase D1 on `shared-pool-phase1` (HEAD 142e6d6): `src/share/bank_links.rs`, `bank_links_ui.rs`,
`circles_ui.rs`, `circles.rs`, `circle_edit*.rs`, the two hooks in `src/bankui.rs` (1395, 864; H3 is phase D2), wording
in `circles_text.rs`. Spec: `docs/circles-phaseD-plan.md` sections 4, 12, 15. Evidence: the builder's frames in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/d1shots/` (all
read, 1280/760/420, light and dark) and `mutd1_summary.txt`. No extra frames were rendered: every finding below is
read directly from the code paths named. No repo file changed except this report; no commit; port 8799 not touched.

The owner's complaint was "clunky, my banks should be in banks, the circle only sets up sharing, I want to copy a whole
shared bank". D1 mostly answers it: the badge and the strip make My Banks the place to work, and the circle page no longer
edits anything. What still feels clunky is (a) the two verbs are offered with no word of what they mean, (b) several
states make the wrong button the big green one, and (c) a few paths end in a second bank with the same name, or in a
copy that is stuck.

## Blockers (new class)

None. No path found that loses unsent work, shows a tracked unsent change as in step, or puts full-form detail in front
of a viewer of a hiding bank. I4 and I2 come nearest: the work is still on the computer, but it can no longer reach the
circle from that bank.

## Important

### I1. "Add to my banks" and "Make my own copy" sit side by side with no explanation
`circles_ui.rs:719-723` (Banks card), `:913-935` (`with_actions`: no hover text). Spec 4.4 gives each verb a sentence;
neither is drawn. For a viewer, "Add to my banks" means something else (follows, can't send) under the same label. A
teacher has to guess, and a wrong guess leaves a bank to delete.
**Change:** under "Add a bank..." (after `:722`), whenever any row offers Add, draw
`para(ui, if role.can_add_banks() { "Add to my banks: the bank stays connected to the circle. You get newer versions when you choose and send your changes back. Make my own copy: a separate bank that is yours alone; nothing goes back and forth." } else { "Add to my banks: the bank stays connected to the circle, so you can get newer versions (viewers can't send changes). Make my own copy: a separate bank that is yours alone." })`.
Also add `RowButton::hint()` with the same two sentences and `.on_hover_text(hint)` on both buttons in `with_actions`.

### I2. A viewer promoted to editor is told "(you can view)" and has no way to send from the copy
`bank_links.rs:221-238`, `circles_ui.rs:745`. A follow link (`mode: Follow`, not `viewers_form`) keeps `can_send() == false`
after promotion, so the strip says "Shared with Algebra team (you can view) ... viewers can't send" while the circle page
says "Editor". "Add a full copy" is offered only for `viewers_form`, so there is no next step. The surviving mutation
`strip_ignores_link` (mutd1_summary) shows no test pins this branch.
**Change:** treat a follow link whose `role_of(l).can_add_banks()` like the viewers'-form case: at `bank_links.rs:221`,
use `if l.viewers_form || (l.mode == LinkMode::Follow && role_of(l).can_add_banks())`, with the line "You can send
changes now, but this copy was added when you could only view. Add a full copy to send from it." At
`circles_ui.rs:745`, use the same condition (add `mode` to `RowLink`). Do not upgrade the link in place: a follow copy
may have merged a viewers' form after hiding was turned on. Add a test for each of the two strings.

### I3. Adding banks advances only while the Shared page is on screen
`circles.rs:1160`/`:1198` (`auto_adds`) and the copy's landing in `sync_copy` are reached only from `shared_ui.rs:243`
and `:256`. "Add the ticked banks" (up to 10 banks, one after another) or "Add to my banks", followed by the natural
next click ("Banks" in the sidebar, to see them), leaves the rest waiting with no sign on the Banks list. They appear only
after the teacher goes back to Shared. Nothing is lost, but it looks broken.
**Change (choose one):** (a) pump the circles model from the Banks list: in `bank_links_ui::badge`, once per frame
(guard with `ctx.cumulative_frame_nr()`), when `c.adds` has `Waiting`/`Working` jobs call `c.frame(now, &snap)` and
`c.sync(&mut store.banks, &mut browse.model)` and save when it returns true. These requests were asked for by the
teacher, but this bends the "badge never starts a request" rule, so the orchestrator should approve it. (b) At least
say it: under the spinner (`circles_ui.rs:799-800`, `:513`) add "Keep this page open until it's done."

### I4. Left, removed or closed: the only button, drawn as the primary, is "Unlink..."; the wording contradicts itself
`bank_links.rs:186`, `:193`; badge `:57`; confirm text `bank_links_ui.rs:225`. A red box with one big green
"Unlink..." invites the click. For "removed (it can be brought back until 3 Nov)", unlinking before the owner brings the
bank back means this bank can never send again. The badge meanwhile says "not linked any more" although it is linked
(Unlink is offered), and the confirm says "You can add the circle's bank to My Banks again later", which suggests this
bank reconnects. It doesn't: adding again makes a second bank.
**Change:** `warn(line, vec![Unlink])` becomes `warn(line, vec![])` with Unlink kept in "..." (or drawn as a ghost),
and in `draw_strip` (`bank_links_ui.rs:182`, `:197`) never draw `Unlink`/`History` as primary (see polish 1). For the
removed case append "Ask the circle's owner if it should come back." Badge notes: "removed from the circle" /
"you left the circle" / "circle closed" (`bank_links.rs:56-57`; pick by `l.seen.removed`, `l.seen.circle_closed`,
else left). Confirm text: "Unlink? The bank stays here as it is, but it is no longer connected to the circle. Adding
the circle's bank again later makes a new bank; this one stays unconnected."

### I5. Same-named duplicates, and a silent hand-over of the link
`circles.rs:1647` (`name: String::new()`) with `:3170`. "Make my own copy" names the new bank exactly like the linked
one, and the button stays offered, so each click adds another "Unit 2 quiz". "Add a full copy" (`circles_ui.rs:745`,
then `circle_edit.rs:946` `add_link` retains by circle bank) quietly moves the link to the new bank. The earlier keyless
"Unit 2 quiz" loses its badge and nothing says so. My Banks then holds two banks with the same name, and the teacher
can't tell which one is connected.
**Change:** own copy: `name: if a.own { format!("{} (my copy)", title) } else { String::new() }`. Full copy: when the
add finds an existing `viewers_form` link for the same circle bank, set the notice "Your earlier copy without answer keys
stays in My Banks but is no longer connected. You can delete it when you're ready." Also append " (no answer keys)" to
that old bank's name, or name the new one "(with answer keys)".

### I6. A picture failure during "Add to my banks" makes an unconnected bank, says it in retired words, then offers Add again
`circles.rs:3195-3197`, `circles_ui.rs:801-806`. The note reads "this copy is a plain copy and is not linked ... Make the
working copy again". The row shows nothing for `Done { linked: false }` when `own == false`, and offers "Add to my
banks" again, so a retry adds a second bank.
**Change:** note: "The bank was added, but N of M pictures couldn't be fetched, so it isn't connected to the circle (a
send from it would remove those pictures for everyone). Delete it and press Add to my banks again when the pictures can
be fetched." Row: add a match arm for `(AddState::Done { linked: false, .. }, false)` that says "Added as "X", not
connected (pictures missing)." with an "Open it" link (`Outcome::OpenBank(bank_id)`).

### I7. Retired words on the new path
Reached from the strip:
- "Treat as in step..." (`bank_links.rs:118`) leads to `circle_edit_ui.rs:533-562`, which still says "in step",
  "Forget this link...", "Yes, forget the link", "make a new working copy". The strip calls the same thing "Unlink...".
- Headings and buttons say "Get latest and merge" (`circle_edit_ui.rs:470`, `:116`, `:246`, `:259`; spec 4.2 renames it).
- "Your copy is now in step with version N" (`:498`); "last in step" (`:106-107`).
- `circles_ui.rs:1579` "Change the questions in a working copy..." (Bank settings).
- `circle_edit.rs:957`, `:965` "list of working copies".
- `bank_links.rs:207` "Couldn't reach the pool" (for a circle).

**Change:** "in step" becomes "up to date" or "matches"; "Treat as in step..." becomes "Use my bank as it is..."; "Forget
this link..."/"Yes, forget the link" become "Unlink..."/"Yes, unlink"; "make a new working copy from the circle" becomes
"add the circle's bank to My Banks again (that makes a new bank)"; "Get latest and merge" becomes "Get latest";
"working copy" becomes "your bank in My Banks"; the offline line becomes "Couldn't check the circle just now. Your bank
works as usual."

### I8. Bank settings says the questions are shown here, and none are
`circles_text.rs:17` and `:43`, used at `circles_ui.rs:1459`/`:1461`. Both start with "The questions are shown here
without their answers." The page has no question list now. The next line (`:1469`) sends the teacher back to the circle
page to "open the bank there", with no button on this page.
**Change:** drop that first sentence from both constants. Under `:1469` add `Open in My Banks` (when this computer has
the link: `Outcome::OpenBank(local)`) or `Add to my banks` (otherwise).

### I9. Every check collapses the strip and jumps the page
`bank_links.rs:195-197`. While a check runs (each page open after 10 minutes, and Check now), the strip becomes one line
with no buttons, then grows again. The whole question grid moves about 40 px, and Send disappears under the cursor.
**Change:** don't return early. Build the normal view from what the link last knew, then set `busy = true` (the
spinner beside the pill is already drawn at `bank_links_ui.rs:172-174`) and keep its buttons. Keep the "Checking for a
newer version..." wording only when the link has never been checked (`role_seen_at.is_none()`).

### I10. Lost compared with before: looking before adding (needs the owner)
`circles_ui.rs` `bank_page` (the questions list, removed) and `:826` (Bank settings only for owner or author). An editor
or viewer can no longer see a circle bank's questions, or its history, without adding it or copying it first. Choosing
which banks to take now means making banks and deleting the wrong ones. Spec 4.3 says "no question list" on the Shared
page, so this is the owner's call. **Proposal:** a read-only "Look at the questions" link in each row's "More:"
(the old first-100 list, no editing), and "History" in "More:" for every member who may read the bank.

### I11. Share ▾ > "Share..." on a linked bank lets the teacher send it into its own circle again
`publish_ui.rs:1052-1064` lists every circle, including the one this bank is linked to. On a page whose strip says "Send
my changes...", the toolbar's "Share" is the more familiar word. Choosing the same circle makes a second circle bank, and
afterwards the window only says the bank "stays linked to that one" (`publish_ui.rs` round-D1 lines at 945-958).
**Change:** in the choices, when the source bank has a link to circle C, draw C's radio disabled with "This bank is
already in it: use Send my changes on the bank's page." Put the same sentence above the list.

## Make it feel right (ranked)

1. Primary only for a real next step: in `draw_strip` (`bank_links_ui.rs:182`, `:197`) use
   `i == 0 && !matches!(b, StripButton::History | StripButton::Unlink)`. In step, "History" then stops being a big
   green button that asks to be pressed.
2. "Open account..." becomes "Sign in..." (`bank_links.rs:116`): the line above it says "Sign in".
3. The offline strip uses `view`, not `warn` (`bank_links.rs:207`). Red for "your bank works as usual" is alarming.
4. "You have it." becomes "Your copy is up to date." (`bank_links.rs:237`, `:248`).
5. "verSHion can't count your changes here; Send compares with the circle." becomes "Press Send to see what you changed."
   (`:241`). In the badge, an uncountable copy currently looks exactly like an up-to-date one (`:61`): show "changes not
   counted".
6. Badge when behind AND unsent: "newer version · 3 unsent" rather than hiding the unsent work (`bank_links.rs:58-62`).
7. Join: once every add is done, show per-bank "Open" links and make "Open the circle" a ghost button
   (`circles_ui.rs:421`, `:503-517`). The spinners at `:513` and `:799` are about 30 px beside 12.5 px text: use
   `ui.add(egui::Spinner::new().size(14.0))`.
8. Own copy done line (`circles_ui.rs:804`): add an "Open it" link.
9. Strip "Add a full copy" only opens the circle page, where the same button has to be pressed again
   (`bank_links_ui.rs:136`): do the add from the strip, or label it "Add a full copy...".
10. Names: the sidebar says "Banks" and the page "Question banks", but the new text says "My Banks". Use one name: either
    lowercase "my banks" everywhere (`circles_ui.rs:660`, `:774`, `:799`, `:804`), or "Banks".
11. Destructive actions look like links: give "Remove..." in the row (`circles_ui.rs:834`) the danger colour, as on Bank
    settings, and draw the badge's gone/removed note in warn colour (`bank_links_ui.rs:37`).
12. Role mismatch: a demoted editor's badge says "3 unsent changes" (`bank_links.rs:61` checks only `l.can_send()`), and
    the circle row says "Nothing to send." (`circle_edit_ui.rs:1089`), while the strip says viewers can't send. Use
    `l.can_send() && role_of(l).can_add_banks()` in both.
13. In-place History or Version pages stay open after leaving the bank. Reopening "Unit 2 quiz" from the list shows
    History, not the questions (`bank_links_ui.rs:98`). Close History/Version (not Send/Get latest) when the bank page
    was not drawn last frame.
14. The lineage text of a linked Add hard-codes "version 9" in the bank's description (`circles.rs:3182`), which
    contradicts the strip after the next Get latest. Leave the version out for linked adds.
15. List rows: with the badge above the name, the row's "..." sits on the name line, low in the card, and linked rows
    are taller than plain ones (`bankui.rs:1395`, `:1411-1413`). Centring the menu on the card needs a `bankui.rs`
    change beyond the approved one-liner, so ask the owner first. The 420 px header overlap of Import/New bank with the
    title, and rows running off the right edge, are older than D1 (`L-420x900.png`).
16. Noise on every circle row: drop "Not shared publicly from here yet." (`circles_ui.rs:770`). It suggests a public
    action this page does not have. Viewer wording "can not" becomes "can't" (`circles_ui.rs:490`).

## What works

The badge and strip read well at 1280 and 760, and wrap cleanly at 420 in both themes. The strip's sentences for the
13 states are short and mostly plain. Behind + unsent makes Get latest primary and still offers Send (Q10). Send, Get
latest and History open in place with "< Back to Unit 2 quiz". Light-theme badge text measured about 5.5:1 (muted) and
7:1 (state) on the card. The circle page no longer edits, and "Open in My Banks" lands on the bank. The join offer ticks
by count and explains the viewer case.
