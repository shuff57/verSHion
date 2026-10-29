# D2 client, critic round 13 — product / Moodle parity

**Build reviewed:** `83469e3` (r14). Read-only; frozen copy verified byte-identical to the commit.

**VERDICT: CLEAN — 0 blockers, 0 important, 3 minors.**

r14 fixes the r13 blocker and the recorded importants. At page level the substituted-picture round trip works: the
circle's own file is fetched, the payload names it, the version PUT goes, and this store's WebP is never sent. The
next send page and Compare read the question as unchanged.

## Minors

### 1 — the failed-fetch sentence names "a change you are not sending", which is false on the new fetch paths
**Where:** `src/share/pool.rs:133`, with `src/share/partial_send.rs:348` and `src/share/circle_edit.rs:3713-3718`.

A teacher changes only B's text; A is untouched but its picture is re-encoded here, so the send must fetch the
circle's original PNG to name it. If that GET fails, the page says "The picture for a change you are not sending
could not be fetched…". There is no such change. Suggested: "A picture this send needs could not be fetched from the
circle, so nothing was sent. Try again." Preserve the retry, the sign-in path and the held-back case's meaning.

### 2 — a base picture the circle will never serve is offered only as "Try again"
**Where:** `src/share/circle_edit.rs:784-800` (any non-sign-out error becomes `BaseMediaUnavailable`, retry true).

A picture purged or blocked after the version was committed makes every partial send retry forever with "Try again",
though nothing will change. Blocking the send may be the safe behaviour, but the state is presented as transient and
has no way forward. A fix should distinguish a 404 ("the circle no longer has this picture") from an unreachable
circle. Worker behaviour unanswered.

### 3 — every partial send re-downloads (and re-uploads) every untouched picture kept here re-encoded
**Where:** `src/share/circle_edit.rs:784-800,795`, `src/store.rs:226-233`, `src/share/partial_send.rs:348`.

The store keeps the circle's PNG as WebP, so the payload's `media/<circle sha>.png` is never on this computer; every
partial send fetches and re-uploads every untouched re-encoded picture. No loss; the send completes. Keeping the
fetched original under `media/<circle sha>.<ext>` (the unused `store::restore_image`) would avoid it; disk cost and a
cap are the trade-off.

## Verified correct

The blocker's page-level path (fetch once, hash-checked, PUT happens, WebP in no request); the mutation check
(`base_image = None` reproduces the r13 blocker); the recorded identity and Compare/next-summary reading unchanged;
only the image is substituted; the held path; `picture_now`, removed picture and bounded-keep tests; the fetch list
and `load_images`; the whole-bank path; 58 frames at 1280/420 with no new layout or wording state.

Residual noticed, not new: the strip still compares the picture by local file name (`merge::ours_delta`), so a future
encoder change storing one circle picture as a second local file could make the strip count a change the summary does
not. Recorded.

## Not checked

Worker blob/version routes beyond the served-blob rule; staging/network (the partial-send picture round-trip is still
unproven there); wasm32/clippy; protected files; dark theme; real screen/input/screen readers; performance beyond the
probes; the parked owner limits and recorded open minors.

**Suite counts:** share 1572/0/3; full lib 2263/0/6 (the commit message's 2267 counts four dirty-tree tests outside the
commit). Probes: 4, all pass; 2 mutations reverted.
