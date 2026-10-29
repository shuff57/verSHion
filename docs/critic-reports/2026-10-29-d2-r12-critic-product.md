# D2 client, critic round 11 — product / Moodle parity

**Build reviewed:** `6777002` (r12). Read-only; probes and frames in a frozen `git archive` copy.

**VERDICT: FINDINGS — 0 blockers, 2 new important, 1 new minor.** Plus the round's security I1 confirmed.

The r12 fixes held on a kept base: the P1 carry keeps this store's re-encoded file (and `picture_now`), the strip
counts nothing and Compare says "the same picture"; a genuine held-back picture change still counts; P2's reason is
truthful; an unreadable `seen` reads `hide_key = true` and a hiding-bank viewer's compare still folds keys and
normalises Matching.

## New important

### A — the send summary lists an untouched, re-encoded picture as a change, and the default sends it
**Where:** `src/share/partial_send.rs:201` (`changes_against_body`), `:234` (`same_content`), `:240` (`same_image`).

After Add or Get latest the store keeps a fetched PNG as WebP under its own name; the link records the pair
(`local_image` = WebP, `snap.image` = PNG), so `changes_since` says the copy is in step. The send summary compares
the bank to the base body by **file name**, so the untouched picture is listed as "Changed"; all rows start ticked,
so the default send carries the WebP and the server stores it under a new name. The circle's picture is rewritten
with the same pixels and the sender is credited with a picture change they never made; the strip and the send page
disagree.

**Probe:** `zz_prod_c11_a_reencoded_fetched_picture_is_listed_as_a_send_change` (failed: `changed == {local id}`).

**Fix (r13):** compare the picture by its circle identity (`picture_sha_on_circle`), not the local file name, and
send a question whose picture is the base's under the base body's picture identity (so the server hashes it
unchanged). `after_partial_send` records the identity the send used, not the file's own hash. Preserves: an unticked
question still goes as the base's raw JSON; a genuine local picture change still lists and sends as the bank's file;
the whole-bank path is unchanged.

### B — on a bank over the base cap, a fetched, re-encoded picture has no name anywhere
**Where:** `src/share/circle_links.rs:491` (`from_head` -> `bounded`), `:518` (`picture_files` empty),
`:817` (`picture_files_kept` only filters); `src/share/circle_edit.rs:4877` (merge keep); `circles.rs:703`,
`merge.rs:1096` (the fetched PNG is stored as WebP).

A bank over 500 questions or 1.5 MB stores no base records, and a link starts with an empty map. A fetched picture is
stored as WebP; the record that carried the circle's name is discarded by `bounded`; nothing repopulates the map.
`picture_sha_on_circle` falls back to the WebP's own hash, so Compare says "a different picture" for a picture the
circle itself holds. Reachable immediately for a large bank added to My Banks, and after any Get latest taking a
re-encoded picture.

**Probe:** `zz_prod_c11_a_large_added_bank_cannot_name_the_circles_pictures` (failed).

**Fix (r13):** `reencoded_pairs` derives `(file, circle sha)` from the base records before bounding; `from_head` and
`finish_merge` keep them when the stored base is empty. Small-bank records still win; the map cap and the r12 merge
keep stay.

## New minor

### D — the `seen` safe default over-hides even when a fresh reply says lists are available
**Where:** `src/share/question_history.rs:280`, `src/share/question_history_ui.rs:1248`.

An unreadable `seen` reads `hide_key = true` (correct default), but `timeline` lets it override a fresh `/changes`
reply whose `per_question` is true, and the panel states as fact that the bank hides its keys. Safe direction, but
the person loses history and reads a statement the server contradicts. Rare; recorded open (a fresh `per_question:
true` winning, or hedged wording, is the suggested follow-up).

**Probe:** `zz_prod_c11_p3_an_unreadable_seen_makes_a_viewer_a_hiding_viewer` (documents the current behaviour).

## Confirmed, already filed by the security critic

### I1 — a partial send on a bank whose composed base is over the cap loses the only name of a held-back picture
Same scenario as the security report's I1; fixed in r13 (judge the keep against what is stored; when bounding
discards the base, only questions the send carried as this bank's own silence the map).

## Verified correct

The r12 P1 carry on a kept base (re-encoded, ordinary, shared-file, `picture_now`, genuine change); P2 wording and
refusal; P3 unreadable `seen`; r12 merge keep; whole-bank clear; partial-send composed keep; lenient link reads;
60 frames at 1280/420 inspected, including the new unrecorded-figure frame.

## Not checked

Moodle's own code; the `/changes` backend; real network/staging; real screen/input/screen readers; wasm32/clippy;
dark theme; performance beyond a 501-question probe; the recorded open leniency and parked owner limits.

**Suite counts:** share 1561/0/3; full lib 2252/0/6; probes 7 total, 4 pass, 3 fail (A, B, security I1).
