# D2 client, critic round 10 — security / ASVS

**Build reviewed:** `9d926bf` (r11). Read-only; frozen copy verified identical to `git archive 9d926bf`.

**VERDICT: FINDINGS — 0 blockers, 1 important, 2 minors.**

The r11 fixes held except one merge-order regression. No full-form content reached a viewer of a hiding bank, no
withheld text was served or compared, no cross-account or authorization defect was found, and no work was lost.

## Important

### I1 — a merge judges the picture-name keep from the merged base and then discards that base
**Where:** `src/share/circle_edit.rs:4859-4860`.

`picture_files_kept(&link.picture_files, &new_base, …)` was judged against the full merged base, then
`link.base = merge::bounded(new_base)` stored a base that is empty above 500 questions or 1.5 MB. A map entry is the
only record of a re-encoded picture (circle's PNG stored as WebP under the WebP's own hash) when the base is bounded
away. Judging against the record that bounding discards dropped that entry; Compare then fell back to the file's
own hash and called a picture the circle itself holds "a different picture". Reachable on a large bank after a
restore plus a colleague's edit and Get latest.

**Probe:** `zz_sec_r11_a_merge_judged_from_a_dropped_base_loses_the_only_picture_name` (fails on r11); the twin
`zz_sec_r11_a2_the_pre_r11_order_kept_the_name` runs the pre-r11 order and passes.

**Fix (r12):** judge the keep against the base that is actually stored (`link.base`, after `bounded`). A record
bounding discards cannot silence the map; with a bounded-away base every entry survives. The r9 I2 send fixes stay
(a whole-bank send clears; a partial send judges from the composed records; a stored record still beats the map).

## Minor

### M1 — "one unreadable link field no longer drops the link" holds only for the fields r9 named
Still dropping the whole entry on a wrong type: `base_version`, `linked_at`, `is_author`, `author_known`,
`difficulty_read`, `title`, the whole `base` array, `base[].local_id`, `base[].snap`, `base[].snap.points`. The
consequence is the r9 M1 one (a corrupted file silently unlinks the bank and the next save drops the entry). Same
privilege as editing the banks; minor.

**Probe:** `zz_sec_r11_b_wrong_typed_fields_that_still_drop_the_link` (still fails; recorded open).
The accepted consequence is that a non-array `qids` empties the identity map and the next send shows affected
questions as "Added", not silently.

### M2 — a `seen` that is not an object falls back to `hide_key = false`
`seen: {"hide_key": null}` read `true`, but `seen: null` or any non-object read `BankSeen::default()` with
`hide_key = false`. The server's `per_question` and `keys_hidden` are the backstop; reachable impact today is a
strip offering Get latest that the merge then refuses.

**Fix (r12):** an unreadable `seen` reads the safe `hide_key: true`; a missing `seen` stays the default.

## Verified correct (summary)

Merge figure record and its two-way exception; picture names (whole-bank clear, composed partial keep, unpaired
restore keep, base-record precedence, one-pass equivalence over generated cases, 256 cap agreement); lenient reads
for the r9-named fields, `viewers_form` unsafe default, unknown `mode` = Follow, viewers'-form links unsendable even
after role change; gone-link refusal; hiding-bank frames show bank lines only and no key text.

## Not checked

The worker routes and suite; real network/staging; the 200/day cap number; `qedit.rs` `opened_at`; wasm32/clippy;
protected files; frames other than the three hiding/follow-viewer states.

**Suite counts:** share 1559/0/3; full lib 2250/0/6; 58 frames; probes 8 pass, 2 fail (I1, M1).
