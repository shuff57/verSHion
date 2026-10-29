# D2 client r15 — product verification

**Build reviewed:** `01b976e` (r15). Read-only; frozen copy verified byte-identical.

**VERDICT: FINDINGS — 0 blockers, 1 important, 2 minors.**

## Important

### I1 — a real HTTP 404 never reaches `BaseMediaGone`; the page still offers Try again forever
**Where:** `src/share/circle_edit.rs:805`, against `Pool::call` (`pool.rs:1332-1345`) and
`PoolError::from_response` (`:192-195`).

`Pool::call` turns every non-2xx answer into `PoolError::Refused`; only a session that returns
`Err(ApiError::Http)` makes the new arm's shape, and the real client never does for a 404. The new test passed only
because it injected that impossible shape; the older real-shape test (a 404 `Response`) still asserted `retry = true`
and Try again, and both passed in one suite.

**Fix (r16):** match `PoolError::Refused(r) if r.detail.status == 404`, build the message with the question's stem
from the payload, and use a real 404 `Response` in the test. The transient test now uses a 500.

## Minors

### M1 — the 404 advice "get the latest" is unreachable and would not help
A blob 404 happens during a send, so the strip offers no Get latest, and Get latest would re-read the same version.
The message now drops that advice, names the question, and says to remove or replace the picture.

### M2 — the keep avoids the re-download but not the re-upload
Every payload picture is still PUT on every send. The r15 commit message's "downloading and re-uploading" was too
broad. (r16 declines the keep entirely, for the security reason below; both halves remain, recorded.)

## Verified

The wording (no "change you are not sending"); transient failures still retry; sign-out still goes to sign-in;
nothing is sent on any fetch failure; the keep with the real store works (but see the security report); a
hash-mismatched or oversized blob is refused; a genuinely changed picture is still sent as the bank's own.

## Not checked

Worker blob semantics; staging/network; wasm32/clippy; protected files; dark theme; real screen/input; performance.

**Counts:** share 1573/0/3; full lib 2264/0/6 on the clean tree; 5 of 6 probes pass, the failing one is I1.
