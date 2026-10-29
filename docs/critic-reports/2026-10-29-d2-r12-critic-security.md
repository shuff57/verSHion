# D2 client, critic round 11 — security / ASVS

**Build reviewed:** `6777002` (r12). Read-only; frozen copy verified byte-identical to the commit.

**VERDICT: FINDINGS — 0 blockers, 1 important, 1 minor (defense-in-depth).**

The r12 merge fix, the `seen` default and the restore reason held. The partial-send picture carry was right on a
small bank but reintroduced the r11 class on a bank whose composed base is over either cap. No full-form content
reached a viewing restriction, no withheld text was served, no cross-account or authorization defect was found, and
no work was lost.

## Important

### I1 — a partial send on a bank whose composed base is over the cap loses the only name of a held-back re-encoded picture
**Where:** `src/share/circle_links.rs:882-892` (the r12 carry) with `:900-901` (`picture_files_kept` against the full
composed base, then `merge::bounded`).

A bank over 500 questions or a composed base over 1.5 MB stores no base records. A held-back question's picture is
the circle's PNG kept here as WebP; the link map (`F -> S`) is its only name. The r12 carry sets the rebuilt
record's `local_image = F`; the keep (judged against the full composed base) then drops the map entry because the
record names the file; `bounded` discards the record. Both records gone, so `picture_sha_on_circle` falls back to
`sha_of_file(F)` and Compare calls the circle's own picture "a different picture". The same happens when a stored
base grows over the byte cap during the send.

**Probe:** `zz_sec_r12_a_a_partial_send_on_a_large_bank_loses_the_held_back_picture_name` (fails on r12); its
pre-r12-record control passes. The pinned r9 I2 and P1 tests pass on r12, so the suite did not catch it.

**Fix (r13):** judge the keep against what will actually be STORED. When `bounded` discards the composed base, only a
question the send carried as this bank's own (`base_image.is_none()`) may silence the map; a held-back question keeps
the link's name. Preserves: a whole-bank send clears the map; a partial send still drops a stale name for a file it
carried as this bank's own; a stored record still beats the map.

## Minor

### M1 — the carry trusts the old record's picture name without checking it is the base body's picture
`local_image`/`picture_now` were carried whenever the base body's question had a picture, without comparing
`ob.snap.image` with the base version's sha (the map fallback does check). No writer keeps a link's records out of
step with `base_version`, so no reachable divergence was found; a hand-edited link (same privilege as editing the
banks) can disagree.

**Fix (r13):** carry only when `ob.snap.image == b.snap.image`; otherwise use the map fallback.

## Verified correct (summary)

Merge keep against the stored base (r12 I1); map precedence; the partial-send carry cannot make a different picture
look the same; `seen` leniency and cache scoping; `FigureNotRecorded`; r9 I2 send rules; hiding-bank and withheld
behaviour; restore/Undo; caches across account/role/hide changes.

## Not checked

Worker routes and suite; staging/network; the 200/day cap number; `qedit.rs` `opened_at`; wasm32/clippy; protected
files; frames beyond 17-19 and 24; dark theme; real screen/input; performance beyond the probes.

**Suite counts:** share 1561/0/3; full lib 2252/0/6; 58 frames; probes 4 pass, 1 fail (I1); with the candidate fix,
share 1566/0/3.
