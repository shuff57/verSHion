# D2 client round 7 — builder brief (2026-10-29)

Round 6 is committed at `825dc14` (7 commits; share suite 1529/0/3). The fifth critic pair
(product vs Moodle, security vs ASVS) reviewed `80dd3d1..825dc14`; **both returned FINDINGS**.
Every finding below marked with a probe was re-run by the main session against the green tree and
**fails as described**.

## Important — fix first, one commit each, test first

### R1 (security) — a picture fetch skips the `Same`/`NotCountable` guard
**Location:** `src/share/question_history.rs:1070` (`if !deferred { … }`), `deferred = true` at `:1058`.
When `restore_step`'s second call passes `store_picture`, `deferred` is true and **both** the `Same`
check and the whole `NotCountable` block are skipped. A locally deleted question whose base recorded
a picture can be restored with an uncounted figure change; `changes_since` reports `changed: 0` and
the strip says "up to date" over a change it cannot count.
**Fix direction:** run the guard on the *resolved* picture identity (compute/compare before the
deferred return), or defer only the picture **storage** while still running the count check with the
already-known `t.sha`.
**Probe:** `/tmp/d2critic-security/src/share/question_history_tests.rs`
`zz_critic_a_picture_fetch_skips_the_not_countable_guard` — fails `Ok(6)` vs `Err(NotCountable)`.

### R2 (product) — the N1 guard is version-number-based; it refuses a restorable non-base version
**Location:** `src/share/question_history.rs:1085` (`if ask.body.version != link.base_version`).
Guarding on the version number instead of on state: a non-base version whose question is the base
version's exactly is refused as `NotCountable`, although restoring it makes the copy exactly the base
(strip drops 1 → 0). UI-reachable: it can be the only Restore row for a question last changed in an
older version.
**Fix direction:** decide by whether the restore undoes a counted change / would leave the copy in
step, not by `version == base_version`.
**Probe:** `/tmp/d2critic-product/src/share/question_history_tests.rs`
`zz_p5_a_restore_that_undoes_an_unsent_edit_is_not_refused_as_figure_only` — fails `Err(NotCountable)`
vs `Ok(6)`. May need that file's `linked_with(&v9q)` helper.

### R3 (product) — a picture a restore stored compares as "a different picture"
**Location:** `src/share/question_history.rs:1053` and `picture_sha_on_circle` at `:647-653`.
A restore stores the version's picture through the app store (PNG/JPEG → WebP, new file name) but
nothing records it in the link, so `base.local_image` is stale; `side_of_copy` falls back to the new
local name and Compare says "a different picture" against **the very version just restored**.
**Fix direction:** after a restore stores a picture, write the stored `ImageRef.file` into that
question's `BaseQuestion.local_image` (`snap.image` stays the circle's sha) and save the updated link;
or compare a picture by the blob it decodes to.
**Probe:** `/tmp/d2critic-product/src/share/question_history_tests.rs`
`zz_p5_a_picture_a_restore_stored_under_its_own_name_compares_as_the_same_picture` — fails with
Compare `["Picture"]`.

## Minor — fix where cheap; probe where behaviour is testable

- **M1 (security+product) restore-point dedupe compares lexicographically-last, not newest.**
  `src/share/browse.rs:216-220` (`have.sort(); have.last()`): `…-2.json` sorts before `….json`, so
  `last` is the *oldest* copy in the newest second and duplicates are still written. Parse the numeric
  suffix / sort by (stamp, index). Probe: security's
  `zz_critic_the_restore_point_dedupe_compares_against_the_oldest_copy_of_the_newest_second`.
- **M2 (security) history caps cover only `by`/`message`.** `src/share/circle_versions_api.rs:53,70,82`
  keep `version_id`, `license`, `review_copy` (and `MergeStart.head_by`/`head_version_id`) whole. Cap
  at parse. Probe: security's `zz_critic_a_history_lines_version_id_is_kept_whole`.
- **M3 (security+product) `finish_merge` does not clear `link.restored`.** `src/share/circle_edit.rs`
  (~4860-4872) vs the doc at `circle_links.rs:149-157`. Clear it, and either make a manual difficulty
  change remove the id or correct the doc.
- **M4 (security) N2 identity is exact-filename only.** `picture_sha_on_circle:644-656` breaks again
  after a second re-encode. Harden if cheap; otherwise document the limit.
- **M5 (product) per-question history truncates at one 50-version page silently.**
  `question_history_ui.rs:336-353, 883` read one `HISTORY_PAGE` and discard `VersionsPage.next_before`,
  while the bank History page has "Older versions". Page further, or say "older versions are not shown
  here".
- **M6 (product) `Show` disabled during a restore with no explanation.** `question_history_ui.rs:1290`
  draws a disabled button with no hover text. Add the reason ("a restore is reading its version").
- **M7 (product) the H3 keyboard hint is inside the menu it describes.**
  `question_history_ui.rs:1690` / `bankui.rs` `draw_card`. Put a short hint on the card or the menu
  button; the keyboard route itself is sound.

## Rules (unchanged)

- **Local commits only. Never push.**
- Do not touch `server/` or protected files: `src/screens.rs`, `src/theme.rs`, `src/figures*`,
  `src/fonts.rs`, `src/main.rs`, `examples/`.
- The tree is still dirty with three other workstreams. **Stage explicitly; never `git add -A` or
  `git commit -a`.** Stage only `src/share/**` (and `src/bankui.rs` if a fix needs it).
- **Commit after each finding** (WIP commits fine) so an API-limit kill loses at most one item.
- **Test first:** paste the probe into the real test file, watch it fail, then fix; keep it as a
  regression test.
- Verify `cargo test --lib share` ≥ 1529 + new tests, 0 failures; full `cargo test --lib` 0 failures
  (6 ignored, two from the figures workstream).
- Report: per finding what changed, test counts, anything skipped and why, commit hashes.
