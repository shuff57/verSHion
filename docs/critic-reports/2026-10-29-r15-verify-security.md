# D2 client r15 — security verification

**Build reviewed:** `01b976e` (r15). Read-only; frozen copy verified byte-identical.

**VERDICT: FINDINGS — 0 blockers, 1 important, 1 minor.**

## Important

### I1 — `BaseMediaGone` is dead in production; a purged/blocked picture still offers Try again with retry on
**Where:** `src/share/circle_edit.rs:805`, with the real wire shape.

`PoolSession::send` returns `Ok(Response)` for every status except a sign-out; `Pool::call` converts non-2xx into
`PoolError::Refused`. The arm matched `PoolError::Api(ApiError::Http { status: 404, .. })`, which the real client never
produces; the 404 fell to the transient line. The r15 test injected the impossible shape, while an older test drove
the real one and asserted the old behaviour. Confirmed the same seam as the product report.

**Fix (r16):** match `Refused` with `detail.status == 404` inside the blob-fetch loop only (the licence `circle_heads`
call returns early), keep sign-out first, keep 429/5xx transient, name the validated media file and the question's
stem, and test through a real 404 `Response`.

## Minor

### M1 — keeping the fetched original verbatim persists peer metadata
`MediaSource::keep` → `store::restore_image` wrote the circle author's original PNG/JPEG/GIF/WebP bytes under
`media/<sha>.<ext>`, preserving EXIF/GPS, ICC, XMP, comments and trailing bytes. The store previously kept only a
stripped WebP; backups copy all of `media/`.

**Resolution (r16): the keep is DECLINED and reverted.** The per-send re-download residual is accepted and recorded.
The exact-name cache cannot be made metadata-free (re-encoding changes the hash it needs), so the safe choice is not
to keep the original at all. `restore_image_in`'s checks are untouched and unused by this path again.

## Verified

The write boundary (`restore_image_in` name/hash/extension/decodability checks; no traversal or foreign name); no
write on failure; the kept file could not be read as a different picture; the 404 message reveals nothing new; retry
semantics; sign-out path; the wording change; regression area (fetch list, identity record, caches, restore/Undo).

Observation, recorded: pre-existing (r14) `media.read(file)` runs before the name is validated in the fetch loop, so
a crafted `image.file` is an existence/timing oracle (the bytes cannot be uploaded). Worth a separate hardening.

## Not checked

wasm32/IndexedDB; staging/network; the Worker beyond the blob route; frames; clippy; protected files; dark theme;
screen readers; performance.

**Counts:** share 1573/0/3; full lib 2264/0/6; probes 2 pass, 1 fails (I1).
