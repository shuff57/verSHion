# Circles phase D1, round 15, critic 94 (product/UX)

Saved by the lead, condensed from the critic's reply. Scratch build of HEAD 0d17a83 plus round-15 `src/share/`: `cargo test --lib` 1854 passed. 168 rig frames (1280/760/420, light and dark) plus 14 real-app frames. Blockers: 0.

## Important
- **I1:** real app at 420 px, a linked bank with a long name (70 chars): the breadcrumb and name row do not wrap, the page gets wider than the window, and `draw_strip` (`ui.set_min_width(ui.available_width())`) follows it, so the strip's sentence is clipped with no sideways scroll. The rig draws a simplified page and missed it. Fix: limit the strip width to the visible area or let breadcrumb/name shrink (layout-only `bankui.rs` change is approved). Test: a real-app frame at 420 px.

## Owner's rule consequence (O1)
Reading the circle's version silently replaces the teacher's own unsent ratings (including where the circle has none); `inherit_pending` writes without a restore point; strip says "up to date". Suggest: restore point before the write plus a one-time line "Ratings were updated from the circle's version N".

## Minor
- m1 Dead end when the base version can't be read and she is not behind ("Check now" fails the same way every time; say what she can do).
- m2 "Can't check the circle right now" shown while the page just loaded; after 15 s "It can't be reached just now" while the read is only slow; offline wording differs.
- m3 Screens disagree while the base version can't be read (strip, badge, circle page row, leave panel "Send it first").
- m4 Newest version held for review while waiting for read: no buttons and no reason.
- m5 Banks list view at 420 px: badge doesn't shorten circle name, runs off the row.
- m6 Close circle / remove bank / remove member panels still say "copies" and don't mention unsent changes.
- m7 Get latest while waiting: "…nothing in your bank has changed." followed by "Nothing in your bank changes yet."
- m8 Notes say "If you use the circle's…", "Keep yours, or keep both"; buttons say "Take…", "Keep mine" (`merge.rs` ~780).
- m9 Signed-out strip: badge says "1 unsent" but strip doesn't; date formats differ (2025-10-08 vs Oct 9, 2025).
