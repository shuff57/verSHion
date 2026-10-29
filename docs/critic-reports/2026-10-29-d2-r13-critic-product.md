# D2 client, critic round 12 — product / Moodle parity

**Build reviewed:** `494db7c` (r13). Read-only; frozen copy verified byte-identical before the probes.

**VERDICT: FINDINGS — 1 blocker, 1 important (latent behind the blocker's fix), 1 minor.**

The r13 comparison change was right — an untouched, re-encoded picture is no longer listed as a change, and the
composer sends it under the base body's identity — but the composed payload names a file this computer does not
have, and the send page never fetches it.

## BLOCKER

### 1 — a partial send of a bank with a fetched, re-encoded picture fails as "a picture used by this question is missing from this device"
**Where:** `src/share/partial_send.rs:313-324` (the payload's image is replaced; `base_image` left `None`),
`src/share/circle_edit.rs:3712-3718` (the fetch list is built from `base_image`), `send_checked`/`load_images`.

A circle PNG/JPEG kept here as WebP means `media/<circle sha>.png` is not on this computer. The composer correctly
sends the unchanged picture under that name but does not record it as `base_image`, so the send page does not fetch
it; `load_images` then refuses with `MissingMedia` ("A picture used by this question … is missing from this
device"), which is not retryable and whose advice is wrong. Reachable on almost every partial send that includes a
re-encoded picture on a question the send did not hold back.

**Probe:** `zz_c12_a_send_that_names_the_bases_picture_never_fetches_it_and_fails_as_a_missing_picture` (page level).

**Fix (r14):** a substituted question records the base body's file in `base_image`, which is exactly the fetch list
the page uses. The payload identity is unchanged; the held path behaves as before; a genuine local picture change is
still sent as the bank's own file.

## IMPORTANT

### 2 — on a bank whose composed base is over either cap, a partial send drops the only name of a picture it did not change
Even after the blocker is fixed, the keep treated a substituted question as "sent as this bank's own", silencing the
link-map entry while `bounded` discarded the record: Compare and the next summary then called the untouched picture
changed, and the default send re-uploaded the WebP.

**Probes:** `zz_c12_a_partial_send_on_a_bounded_bank_keeps_the_unchanged_pictures_name`,
`zz_c12_a_partial_send_that_grows_the_base_over_the_cap_keeps_the_pictures_name`.

**Fix (r14):** the composer carries `picture_on_circle`; only a question sent under its own file hash may silence the
map. A bounded base keeps held-back and substituted names.

## MINOR

### 3 — `from_head` wrote the picture-name map with no cap
>256 fetched, re-encoded pictures were written; the reader keeps the newest 256, so the old names vanished on
reload. **Fix (r14):** cap to `PICTURE_FILES_MAX`, newest kept.

## Verified correct

The A comparison (untouched re-encoded picture not a change; text change still listed; genuine picture change
listed and sent as ours; dimensions and one-side-only cases); the substituted value is exactly the base body's
`image`; B's `from_head` naming; I1/M1 unit-level behaviour; `picture_sha_on_circle` precedence; send summary vs
strip agreement; history/compare/restore unaffected. 58 frames; no new UI defect.

## Not checked

Moodle's code; the `/changes` worker route; staging/network (the partial-send picture round-trip is still unproven);
real screen/input/screen readers; wasm32/clippy; dark theme; performance beyond the 501-question probes; the parked
owner limits and recorded open minors.

**Suite counts:** share 1567/0/3; full lib 2258/0/6; probes 9, of which 3 fail (findings 1–3) and 6 pass (controls and
the blocker's own failure being asserted).
