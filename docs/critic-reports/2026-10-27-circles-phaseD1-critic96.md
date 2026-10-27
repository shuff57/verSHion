# Circles phase D1, round 16, critic 96 (product/UX)

Saved by the lead, condensed from the critic's reply. Scratch build HEAD 0d17a83 + rounds 15-16: `cargo test --lib` 1885 passed. 216 rig frames (1280/760/420, light/dark) + 9 real-app frames. Blockers: 0.

## Important
- **I1:** the read replaces her own ratings on every older-build linked bank whichever page is open; the "Ratings were updated…" line is kept in memory on that bank's page only (lost if she quits first); it doesn't say which questions or before/after (also covers deleting a rating); points to a `.json` restore point (named "merge") that Settings > Restore from backup (zip only) can't use. Fix: keep the notice on the link until dismissed and show it in the badge; name each question before -> after; per-question "Put mine back" as a local change; say plainly her ratings were replaced.
- **I2:** "Use my bank as it is…" confirm page titled `Get latest`, shows old "saved copy put back" red boxes and a "THIS" in caps; real effect: her next send overwrites the circle's difficulty ratings incl. colleagues' ratings an earlier build dropped. Needs its own title and text; Send page afterwards gives no warning.

## Minor
1. "Not kept": strip promises a check that never happens ("That needs…" unclear; main button Check now); badge/circle row/leave/close/remove panels say "stay until then" with no way out; held-for-review + not kept has no buttons (dead end).
2. "so stay until then" shown on Close circle / Remove bank where she isn't leaving; Held state says "once the circle has been checked" but the problem is the disk.
3. Daily limit: "You have reached today's limit" blames her, gives no time though server gives retry_after; strip says can't be checked while badge says newer version; Get latest page says "items" (jargon) with a Try again that can't work; "…" menu still offers Check now.
4. Save failure (Held): raw OS error in nested brackets; small grey text on calm green strip; Try again with disk full changes nothing visible.
5. "ratings" vs "difficulty" inconsistent.
6. Signed out: strip says "1 unsent change", badge "changes not counted"; bank 2 badge says newer version, strip doesn't.
7. Bank settings remove confirmation: text runs inline after "Report this bank…".
8. Dates: circle page still "Joined 2025-10-04" vs "Oct 8, 2025".

Out of scope (older): Banks header buttons over title at 420; list rows/question cards overflow at 420; long bank name cut mid-word; Close the circle is a filled green button.
