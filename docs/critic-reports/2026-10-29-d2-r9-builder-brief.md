# D2 client round 9 — builder brief (2026-10-29)

r8 is at `cf01ba4` (2 commits; share suite 1542/0/3). The seventh pair: **product FINDINGS**
(1 important, 3 minors), **security CLEAN** (3 minors; no blocker/important). I re-ran every probe
below against the green tree; all fail as described. The loop still needs both clean on the same build.

## Important

### T1 (product) — a partial send that holds a restored picture back loses its name
**Location:** `src/share/circle_links.rs:805` (`after_partial_send` clears `picture_files` for **every**
question) and `:783-790` (a held-back question's base is rebuilt from the base body, which cannot name
the locally stored WebP file).
**Scenario:** restore a version's picture (stored as WebP; link map records it), send with that
question unticked → the map is cleared, the held-back base cannot name the file → reopening History
and comparing with that version says "a different picture". The strip stays truthful; only Compare lies.
**Fix:** clear only the map entries for questions this send actually sent; keep entries for held-back
questions and additions left out of the payload. Update the `CircleLink::picture_files` doc — it claims
the record is cleared on "a send or a merge", but a merge does not clear it and clearing would break
keep-ours.
**Probe:** `/tmp/d2critic3-product/src/share/question_history_tests.rs`
`zz_prod_r8_a_partial_send_that_holds_back_a_restored_picture_loses_its_name` — Compare `["Picture"]`.

## Minor

### M1 (security) — the map must not override an accurate base record; `finish_merge` leaves it stale
**Location:** `question_history.rs:653-658` (link map consulted **before** the base record),
`circle_edit.rs:4772` (`finish_merge`), `circle_links.rs:160-165` (doc).
A merge rebuilds an accurate base (`local_image = F`, `snap.image = S_new`) but a stale `F → S_old`
entry wins, so Compare misreads a copy that is in step. **Fix:** for a question that has a base entry,
the base record is authoritative; consult the link map only when there is no base entry (the unpaired
case it exists for). Drop map entries when that question's base is rebuilt (merge/send).
**Probes:** security `zz_sec_b_a_merge_should_clear_the_picture_files`,
`zz_sec_b_a_stale_picture_files_entry_must_not_override_the_base_record`.

### M2 (product) — pre-r8 links: an unknown base figure is still unsafe in the "version has no figure" direction
**Location:** `question_history.rs:1116-1119`; the comment at `merge.rs:81-85` claims "not known is
unsafe", but the guard only refuses when the **version** has a figure.
**Fix:** when the base record is `None`, also refuse if the restore would change the copy's current
figure (compare `merge::figure_key(r.figure)` with `existing`'s figure), then correct the comment.
**Probe:** product `zz_prod_r8_an_old_links_unknown_figure_still_lets_one_vanish_with_the_strip_in_step`.

### M3 (security) — one unreadable figure drops the whole link
**Location:** `merge.rs:80-85`. Add a lenient `figure_or_none` reader like `picture_now_or_none`, so a
link from a newer or corrupted build keeps its qids and every base record instead of being skipped.
**Probe:** security `zz_sec_b_a_link_with_one_unreadable_figure_should_still_parse`.

### M4 (product) — picture-record cap: the reader keeps the first 32, the writer the newest 32
**Location:** `circle_links.rs:170-181` (read) vs `question_history.rs:1166-1171` (write, `remove(0)`).
**Fix:** the reader must keep the newest entries; raise or prune the bound where cheap and document the
residual (after N unpaired restores the oldest name is lost).
**Probe:** product `zz_prod_r8_the_picture_record_cap_makes_compare_lie`.

### M5 (product) — the new-question card carries a menu hint but has no menu
**Location:** `bankui.rs:2497-2510` (hint) vs `:2517-2530` (menu only inside `if let Some(q) = q`;
`draw_card` is called with `q = None` at `:1168`). Set no hint (or a "New question" hint) when
`q.is_none()`.

### M6 (security) — accept-and-document unless cheap
`picture_files` is keyed only by local file name and wins over base records; a tampered on-disk link
can make a different picture look the same (same privilege as editing the banks; defense-in-depth).
The M1 precedence fix shrinks the blast radius. Validate if cheap, otherwise document.
**Probe:** security `zz_sec_b_a_tampered_picture_files_entry_must_not_make_a_different_picture_the_same`.

## Rules (unchanged)

Local commits only, never push; no `server/`, no protected files (`src/screens.rs`, `theme.rs`,
`figures*`, `fonts.rs`, `main.rs`, `examples/`); the tree still has three dirty workstreams — stage
explicitly, never `git add -A`/`commit -a`; commit after each finding; test first (paste the probe,
watch it fail, then fix) and keep it as a regression test; verify `cargo test --lib share` ≥ 1542 + new
and full `cargo test --lib` 0 failures. Report per finding, test counts, skips, commit hashes.
