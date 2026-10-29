# D2 client, critic round 13 — security / ASVS

**Build reviewed:** `83469e3` (r14). Read-only; frozen copy verified byte-identical to the commit.

**VERDICT: CLEAN — 0 blockers, 0 importants, 0 new minors.**

r14 closes the round-12 I1 and M1–M4 without opening a new one in the picture-identity, fetch-list, map-keep, cap or
lost-answer-matcher paths. Seven probes pin the invariants; all pass on this build. No full-form detail reaches a
hiding-bank viewer, no withheld text is served from the new code, no cross-account or cross-role cache leak, no lost
work, no unbounded reply, no send that cannot complete for a reason this round introduced.

## Verified (summary)

- **Substitution identity and payload file agree.** The identity is the base body's sha only where the composer
  substitutes, the file's own hash otherwise, and none when the picture is removed. Over a six-question mixed send,
  `{base_image} == {payload picture files} - {this bank's own files}`, every entry a valid media name: the fetch list
  cannot name a file the payload does not, and the payload cannot name a base file missing locally without it being
  on the fetch list.
- **Fetch is safe.** `send_checked` fetches only valid media names by sha, through `circle_blob` (sha format, response
  cap, byte-hash check), and `load_images` reads the uploaded bytes from the fetched map, hash-checked. The bank's WebP
  is not read for a substituted slot. The blob route is bank-scoped, membership- and hold/hide-checked, and refuses
  withheld versions (cross-check).
- **`after_partial_send`** records the identity the send used (held-back, misaligned-answer probes); only a question
  sent under its own file hash silences the map, and only when the rebuilt base is stored; a bounded base keeps
  held-back and substituted names; a stored record beats a stale map name; a removed picture records none; the
  `from_head` cap keeps the newest 256 and is reload-stable.
- **`same_picture_on_circle`** cannot match across questions or files (`rebase_own_send` requires the same circle id
  and Sig first); a genuinely different local picture does not match; a link with no name falls back to by-content.
- **Tampering residual** pinned at its exact boundary: with an honest link a genuine local change is listed and sent;
  only a link edited on disk to map the file to the circle's sha makes the summary omit it. Same privilege as editing
  the banks; r14 does not widen it.
- **Authorization, withheld versions, caches, restore/Undo** unchanged and re-read.

## Observations (not findings)

1. `BaseMediaUnavailable`'s sentence ("a change you are not sending") can be false for a ticked substituted question;
   no security impact, retry advice honest. Recorded as a product minor.
2. A history page left open across a withhold keeps already-served lines/text until reopened; nothing withheld is
   fetched. Pre-existing and bounded.
3. Recorded open minors not re-raised: file-keyed map collision, unreadable `seen` over-hides, unreadable non-scalar
   link fields.

## Not checked

Worker routes/backend as a review surface (only the blob route read as a cross-check); staging/network; wasm32/clippy;
protected files; dark theme; real screen/input/screen readers; performance beyond 501-question probes; Moodle.

**Suite counts:** share 1572/0/3; full lib 2263/0/6 (the commit message's 2267 counts four dirty-tree tests outside the
commit); 58 frames; probes 7/7 pass.
