# Circles phase D1, round 17, critic 98 (product/UX)

Saved by the lead, condensed from the critic's reply. Scratch build HEAD 0d17a83 + rounds 15-17: `cargo test --lib` 1920 passed. 246 rig frames + 8 real-app frames. Blockers: 0.

## Important
- **I1:** the "difficulty updated" list (`difficulty_news`) is cleared only by Dismiss. After she sends the put-back questions rows still say "not sent yet" while the strip says "up to date"; after Get latest to a version where a colleague re-rated, the row still offers Put mine back and pressing it replaces the colleague's difficulty with her old one as an unsent change, no warning; same if she edits the difficulty herself afterwards. Fix: when the copy's version moves on, settle the list: rows whose question changed since say "changed since" with no button, sent rows say "sent".
- **I2:** the "difficulty updated" badge replaces "newer version" / "not counted" / other states (`bank_links.rs` checks news before behind). Signed out: strip says version 3 of 5, 1 unsent; badge only "difficulty updated · 1 unsent". Fix: add the news to the badge rather than replace.

## Minor
1. "The circle's daily limit" is wrong: the server counts per account across circles and the public pool.
2. Save-failure warning talks only about the circle's difficulty; when banks.json can't be written her own edits are not saved either (`save_banks` ignores errors); no step she can take.
3. Version can't be read: "Version 3 is the latest" then "no longer keeps version 3"; any non-network error is worded as "no longer keeps"; held-for-review case doesn't say waiting for the review helps.
4. Version can't be read: filled main button is the risky "Use my bank as it is..."; badge "1 unsent change" in normal colour; circle-page row green though sending is off.
5. After "Use my bank as it is": "a change to a difficulty can't be counted here" next to "difficulties included"; overwrite warning not repeated on the Send page.
6. News list layout: ragged buttons, "Easy ->" / "Hard" split at 420, 8 rows push Send down, no "Put all of mine back", Dismiss deletes record with no warning, right above strip buttons.
7. News wording: "An earlier verSHion" reads like a typo; "replaced by the circle's" when the result is none; after everything is put back text still says "press Put mine back".
8. "changes not counted" badge text is jargon, never explained.
9. Remove panel in Bank settings has no question heading, sits under "Report this bank...".
10. Invite-by-email panel still says "until 2026-10-26 (UTC)" (date format).
11. "Use my bank as it is" page carries the circle's bank name; "..." tooltip mentions "check now" when hidden; "safety copy"/"saved copy"/"restore point" used for the same thing.
