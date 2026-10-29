# D2 client, critic round 12 — security / ASVS

**Build reviewed:** `494db7c` (r13). Read-only; frozen copy verified byte-identical to the commit.

**VERDICT: FINDINGS — 0 blockers, 1 important, 4 minors.**

r13's substitution was sound where the old base record names the file, and the r11 held-back path held. But the
"identity the send used" was recorded only when the old base record's `local_image` named the file; the other two
ways a picture can be named in the link fell back to the file's own hash, so Compare and the next summary called the
circle's own picture different — the class r13 set out to fix. No full-form content reached a viewing restriction, no
withheld text was served, no cross-account or authorization defect, no work lost.

## Important

### I1 — `after_partial_send` records the file's own hash, not the circle identity the composer sent, whenever the old base record does not name the file
**Where:** `src/share/circle_links.rs:911-917` (the only path recording the sent identity), `:930-941` (`sent_as_own`
counts every carried question), with the substitution at `src/share/partial_send.rs:309-322`.

Two reachable states:
(a) A fetched re-encoded picture named only by the link map, on a bank whose composed base is over the cap (r13's
`reencoded_pairs` fills exactly such a map). The partial send substitutes the base identity; the keep counts the
question as "sent as this bank's own", drops the only map entry, and `bounded` discards the record.
(b) A picture named by `picture_now` (a restore) or by the map on any bank: `base_for` rebuilds
`snap.image = sha_of_file(F)`, and the old-record carry cannot fire when `local_image` is `None`.

Consequence: `picture_sha_on_circle` returns the WebP hash; Compare says "a different picture"; the next summary
lists the untouched picture as a change, all rows ticked, so the default send re-uploads the WebP and rewrites the
circle's picture for everyone. Local bank intact; no leak; no loss.

**Probes:** `zz_sec12_a_large_banks_partial_send_drops_the_circle_name_of_a_sent_picture`,
`zz_sec12_a2_the_next_summary_after_that_send_lists_the_untouched_picture_as_a_change`,
`zz_sec12_a_restored_base_picture_sent_under_the_circles_identity_loses_that_identity`.

**Fix (r14):** the composer carries the identity it used (`ComposedQuestion::picture_on_circle`); `after_partial_send`
records that as the rebuilt `snap.image` and uses it to decide which carried questions may silence the map (only a
question sent under its own file hash). Preserves: a stored record stays authoritative; a held-back name survives a
bounded keep; a genuinely new picture still loses a stale map name; a genuine picture change still lists; the
whole-bank path is unchanged.

## Minor

### M1 — the lost-answer matcher reads a re-encoded picture by the file's own hash
**Where:** `src/share/circle_edit.rs:5826-5832` (`same_picture`), used by `merge::rebase_own_send`.

With r13's substitution the sent version carries `S0` while the bank holds `H`; the matcher says different, so after
a lost/unreadable answer Get latest reads the person's own send as a colleague's change, and a later edit becomes a
conflict with themselves (the critic 80 I1 regression).

**Fix (r14):** `same_picture_on_circle(link, q, t)` — the plain matcher plus the link's picture identity — used at the
rebase call. An added question still matches by its own hash; a genuinely different picture still does not.

### M2 — `from_head` writes more picture names than the link's cap
`reencoded_pairs` was uncapped while the reader keeps the last 256, so a large bank with >256 re-encoded pictures lost
the first names on reload.

**Fix (r14):** cap to `PICTURE_FILES_MAX`, keeping the newest, as `finish_merge` already did.

### M3 — the map is keyed by file, so one local picture with two circle names makes the second question read as changed
Two circle questions whose different files (PNG vs JPEG, or a resize) become the same stored WebP make the map hold
two names for one file; `find` returns the first, so the second question is listed as a picture change. Reachable
only when the circle holds the same pixels under two files (another client's upload). Not fixed: the real fix is a
per-question name on a bounded base (an additive `local_id -> sha` field), which is a link-format change.
**Recorded open.**

### M4 — the carry matched `None == None`, recording a removed picture as the base's
The r13 `else if` matched when the bank question and the old record both name no file; a picture the person removed
was recorded with the old base version's `snap.image`.

**Fix (r14):** the carry is driven by `picture_on_circle`, which is `None` when the send removed the picture.

## Verified correct (summary)

Only the image is substituted (id, `published`, created/modified and every other field intact); per-question
identity lookups; the r11 M1 carry; r11 I1 held-back name on a bounded bank; `reencoded_pairs` pairing and
precedence; substituted uploads are hash-checked (`load_images`, `send_checked`); the whole-bank path; authorization,
withheld versions, caches, restore/Undo; frame 19 shows no full-form detail. The tampered-map residual is unchanged
in kind (r13 makes the composer use a tampered name too: a genuine local change can be dropped from the send, which
is documented).

## Not checked

Worker routes/backend; staging/network; wasm32/clippy; protected files; dark theme; real screen/input; performance.

**Suite counts:** share 1567/0/3; full lib 2258/0/6; 58 frames; probes 7, all failing on the intended assertion.
