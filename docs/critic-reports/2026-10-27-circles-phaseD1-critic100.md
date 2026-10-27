# Circles phase D1, round 18, critic 100 (product/UX)

Saved by the lead, condensed from the critic's reply. Scratch build HEAD 0d17a83 + rounds 15-18: `cargo test --lib` 1958 passed. 120 rig frames (20 modes, 1280/760/420, light/dark) + 23 real-app frames. Blockers: 0.

## Holding
News list settles (sent / changed since), badge parts combine incl. signed out, Put all of mine back saves in the real app, daily-limit wording, invite date, Bank-settings remove heading fixed; no "pool"/"ratings"/"saved copy" in circle wording.

## Important
- **I1:** "Check now" (now the big button when the version is not kept or held for review) does nothing visible: same answer, strip text unchanged, no "checked, nothing newer yet", and the text never says what it does.
- **I2:** a save failure that loses her edits looks harmless: Put mine back save failure shows plain text in the calm green strip with its own Dismiss, while the strip above still says "Your copy is up to date" and the badge is calm; after Dismiss nothing says her work isn't being saved; no "keep verSHion open" advice; "Check that verSHion may write to its data folder" is not actionable. The waiting-to-save path is shown in red; this one is not.

## Minor
1. Not-kept sentence still contradicts itself ("version 3, the circle's newest version, but the circle no longer keeps it").
2. Colours disagree when sending is off (badge/row red, strip calm; held-for-review badge red "changes not counted" vs strip "Waiting is enough"; red "1 unsent change" has no hover hint; put-back-from-safety-copy badge has no words).
3. After the copy moves on, Put mine back disappears with no reason and no tip to set it by hand.
4. After put back, rows no longer show the circle's value; Put all of mine back (8 questions) has no confirmation/undo.
5. News never retires itself: all rows sent/changed, badge still "difficulty updated", list reads as fresh news.
6. News layout: buttonless rows reserve the button column; 420 px squeezes text into 3-4 lines and splits maths mid-expression; at 1280 buttons ~700 px from rows.
7. Two stacked Dismiss buttons (list and error).
8. Bank names differ: Use-as-is page shows her bank's name, Send/Get latest pages show the circle's title.
9. Send page says "you can restore one later" - false for editors (only owner/author can restore).
10. Remove confirmation in circle-page bank row has no heading; "Remove..." stays visible.
11. Badge short forms: "newer" no hint; "left" used when removed; at 420 circle name shrinks to "Biolo…" but "difficulty updated" never shortened; "waiting for the circle check" awkward; "..." hint says "the circle's page", menu says "Circle page".
12. Walls of text: review-plus-not-kept message (5 sentences); held message says "verSHion" 4 times; leftovers "merge" wording, "copy that file over banks.json", "REPLACES" in capitals, invite panel "(UTC)" / "redeemed its account invite code".

Out of scope: Banks list rows past the window at 420/760; long titles cut mid-word.
