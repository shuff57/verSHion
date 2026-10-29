# D2 client, critic round 9 — product / Moodle parity

**Build reviewed:** `aac85dd` (r10). Read-only; probes, frames and runs in a frozen `git archive` copy.

**VERDICT: FINDINGS — 0 blockers, 2 important, 1 minor.**

The r9/r10 fixes themselves held: T1 (a partial send keeps a held-back restored picture's name), I1 (a merge
keeps an unpaired restore's name), I2's `figure_recorded` state for `base_of`/two-way records, the shared-file
minor, M1 (base record precedence), M2 (unknown figure unsafe both ways), M3 (lenient figure read), M4 (256-entry
cap, reader keeps the newest) and M5 (no menu hint on the new-question card).

## Important

### I1 — a Get latest writes the pool's figure into a base record and still marks it "not recorded"
**Where:** `src/share/merge.rs:1028-1045` (write at `:1038`), read by `src/share/question_history.rs:1122-1127`.

A link written before r10 has `figure_recorded: false` on every base entry. The teacher presses Get latest; every
row is `Unchanged`, and `apply_merge` replaces the snapshot and figure with the pool's newest version's but never
sets the flag. With no figure in the circle and one added by hand locally, the history page then refuses a restore
that lands exactly on the base (version 9, identical to 10 for that question) with the false sentence "This
version's figure is not the one your copy is based on...". The version's figure *is* the base's (both none).

**Fix:** a merge that writes the pool's figure records that it did (`figure_recorded = true`), including the
`Unchanged` path for old links. Preserve: `unknown_base`'s two-way records stay `false`; a row whose figure is still
the old base's must not claim to be a record of the pool's.

**Probes:** `zz_prod_r10_a_merge_that_writes_the_pools_figure_still_records_it_as_unknown`,
`zz_prod_r10_a_restore_that_lands_on_the_base_is_refused_after_a_get_latest_on_an_upgraded_link`.

### I2 — a send on a bank whose link keeps no base record leaves a stale picture name
**Where:** `src/share/circle_links.rs:759-761` (`after_send`), helper `:772-783`; same helper in
`after_partial_send` at `:839-842`.

`merge::bounded` returns an empty base above 500 questions or 1.5 MB (the plan's own examples include a 990-question
bank). A restore stored a WebP file `F` and recorded `F -> S_old`; there is no base record, so that name is the only
picture identity Compare has (correct before the send). After the whole-bank send the base is still empty, and
`picture_files_kept` keeps every name whose file some question still has — the stale `F -> S_old` — while the circle
now holds the uploaded file under its own hash. Compare "Your copy" against the version just sent says "a different
picture" for the picture that send uploaded.

**Fix:** after a whole-bank send, clear the map (every question was sent as itself); judge a partial send's keep
against the composed records, not the possibly-empty stored base.

**Probe:** `zz_prod_r10_a_send_that_drops_the_base_keeps_a_picture_name_the_sent_version_disagrees_with`.

## Minor

### M1 — a gone link's question menu offers a History page that closes unseen
**Where:** `src/share/question_history_ui.rs:1706-1708`; the page closes at `:934-935` (`link.gone()`).

After leaving/being removed/a closed circle, the card still offers and enables "History in the circle"; pressing it
returns true and the next frame closes the page before it is drawn. Nothing happens and nothing is said.

**Fix:** refuse the item for a `gone()` link, as for an unlinked bank; keep the draft-open disabled item, the
Shift+F10 route, and the page's own close when a link goes while open.

**Probe:** `zz_prod_r10_a_gone_links_question_history_menu_opens_a_page_that_closes_without_a_word`.

## Verified correct (summary)

Timeline, compare, restore, Undo, privacy and hidden-key handling, withheld/pruned versions, cache scoping and
account switches, read caps and 304s, the H3 hook, and the r9/r10 fixes listed above. 29 states rendered at 1280 and
420 plus 10 dark frames; no overlap, clipping, contrast or wording defect found. The 420 three-line rows and the
per-field stacked compare remain the parked owner decision.

## Not checked

Moodle's own history code (behaviour assumed from its UI); the `/changes` backend; a real screen, input devices and
screen readers; wasm32/clippy; huge-bank performance beyond a 501-question probe.

**Suite counts:** `cargo test --lib share::` 1546/0/3; full `cargo test --lib` 2244/0/6 (r10, with the probes filtered).
