# Circles phase D1, round 8, critic 79 (data safety)

Saved by the lead: the critic could not write files; this is its report, condensed. Scratch copy `scratchpad/c79/`, rig 18791. Build in scratch copy: `cargo test --lib` 1653 passed; wasm32 and bins ok. Blockers: 0. Mutants: 50 of 58 killed.

## Important
- **I1 (web, already in HEAD, needs a server change in server/auth):** "Sign out everywhere" in a tab acts on the other tab's account. The web client sends no refresh token, the cookie decides, and after another tab signed in as Y the cookie is Y's: Y is signed out on every device, X stays signed in elsewhere, and the app says "Signed out on every device." Fix: server refuses (409 session_mismatch) when bearer user differs from cookie session user; client then says the session here changed and looks up who is signed in. Held: out of scope for D1 (server untouched).
- **I2:** "Did not go through" said of a send or restore that made the newest version. Other reads (History `note_head`, Shared page `apply_heads`, merge start on a withheld head) raise `head_seen` without settling `unclear_noted`/`unclear_restores`; the later check sees after == before. Fix: keep the version the send/restore was based on and decide "moved" by head > base.
- **I3:** matching misses text the server rewrites to composed (NFC) form (`same_result`, merge.rs ~476); a decomposed accent comes back as "New in the circle" with Add ticked: the duplicate again. Fix: compare NFC-normalised text.

## Minor
1. After a restart the person's own version is blamed on "Someone else" (`maybe_sent`, `retry_note`, `pending_send` are memory-only); compare `head_by` with own name.
2. `maybe_sent` outlives a "did not go through" note.
3. Server checks the daily limit before the replay (`circlesend.ts` ~366-417, 602-607): retry of the day's last send is answered 429. Server change; held.
4. The retry is not the same request after a merge, for hide-keys banks, or over a held version (`close()` clears `carry`; `open_send` resets decisions/picked/base_override).
5. Matching ignores difficulty and figure.
6. Trim wording "deletes nothing more" not exact.

## Survivors
a02 (restore refresh ok but /me fails), c10, c12 (race guards), e26 (next Send page keeps old note after Get latest), g02 (no test with duplicate identical new questions in the circle). Equivalent: c08, g01, g06.
