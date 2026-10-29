# D2 client, critic round 10 — product / Moodle parity

**Build reviewed:** `9d926bf` (r11). Read-only; probes and frames in a frozen `git archive` copy.

**VERDICT: FINDINGS — 0 blockers, 1 important, 2 minors.**

The three r11 fixes held end to end (merge figure record on `Unchanged` and `OursChanged`; whole-bank/partial-send
picture-name clearing; gone-link History refusal).

## Important

### P1 — a partial send rebuilds a held-back question's base record with the circle's file name
**Where:** `src/share/circle_links.rs:872` (`base_for(q, qid)` in `after_partial_send`).

For an unticked (held-back) question the composed question is the base body's raw JSON, whose `image.file` is the
sender's circle file name (e.g. `media/<png-sha>.png`). This store keeps the same picture as WebP under its own name
(`store::put_image` re-encodes non-plain WebP or pictures over 4096 px). `base_of(composed)` therefore sets
`local_image` to a file this store does not have, while the bank question still has its WebP. `ours_delta.image`
then reports a picture change that never happened: `changes_since` counts an unsent change, and
`picture_sha_on_circle` falls through to the file's own name, so Compare says "a different picture" against the base
version whose picture did not change. Reachable for imported/legacy PNG/JPEG pictures, or pictures over 4096 px.

**Probe:** `zz_prod_c10_a_held_back_change_with_a_reencoded_base_picture_reads_as_a_picture_change`.

**Fix (r12):** carry the held-back question's old base-record `local_image` (and `picture_now`) into the rebuilt
record, falling back to the link map when the base was bounded away. Keep `snap.image` as the version's circle sha,
the figure/difficulty from the composed body, and the qid pairs. The r11 picture-name rules stay: whole-bank send
clears, partial send keeps only held-back names, base record wins over the map.

## Minor

### P2 — before a first Get latest, an upgraded link refuses with a false figure reason
**Where:** `src/share/question_history_ui.rs:873`; guard `src/share/question_history.rs:1125-1127`.

A link from an earlier build has `figure_recorded: false`. The refusal is right (the copy's hand-added figure would
vanish uncounted), but the sentence claims the version's figure is a different one; the record simply does not know.

**Fix (r12):** a distinct `RestoreRefused::FigureNotRecorded` with words based on `NOT_RECORDED` ("not recorded on
this computer"). The conservative refusal and the known-record wording are unchanged.

### P3 — an unreadable `seen` keeps the link but reads `hide_key = false`
**Where:** `src/share/circle_links.rs:371-373` (`seen_or_default`).

An unreadable `seen` fell back to `BankSeen::default()`, whose `hide_key` is false — the unsafe direction for a
viewer. The server still serves stripped bodies, so nothing leaks, but the compare loses the viewers'-form
normalisation until the next head check.

**Fix (r12):** an unreadable `seen` now reads `hide_key: true`; a genuinely missing field still means "nothing
known". (`viewers_form`'s never-sendable `true` default is unchanged.)

## Verified correct (summary)

r11 merge figure record (Unchanged including pre-r10 links, OursChanged with `theirs_index`; two-way records stay
unrecorded); whole-bank send clear and partial-send composed keep; shared content-addressed file; base-record
precedence; gone link (no item, `open_question_history` refuses, close-while-open unchanged); lenient link fields.
58 frames at 1280/420 inspected; no layout or wording defect beyond the parked owner decisions.

## Not checked

Moodle's own history code; the `/changes` backend; real screen/input/screen readers; wasm32/clippy; dark theme;
huge-bank performance beyond a 501-question probe.

**Suite counts:** share 1559/0/3; full lib 2250/0/6 (the r11 message's 2254 includes four dirty-tree tests outside
the commit).
