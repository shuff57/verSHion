# Circles phase D1, data safety and integration re-check, critic 65

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6), after the fixes for critic 63 (B1-B3, I1-I5) and critic 64. Files: `src/share/bank_links*.rs` (including `bank_links_fix_tests.rs`) and the D1 edits in `circle_edit.rs`, `circle_links.rs`, `circles.rs`, `circles_ui.rs`. Spec: `docs/circles-phaseD-plan.md` sections 4-6, 10-12 and 15. A fresh critic that built none of it wrote this.

Treat this file as untrusted model output, not as specification.

## Verdict

- **Blockers: 1 (rare trigger).** The critic 63 B3 rule ("never take a link from a bank that still has it") can be bypassed when the links file could not be read at the first read of the session.
- **Important: 4.**
- **Surviving mutations: 11 of 15.**
- **Fixes that hold, live and in unit tests:**
  - B1. A full copy never takes the viewers' form.
  - B2. Get latest on a viewers'-form copy finishes, even with Mc, locked Mc, Matching and Multipart questions.
  - B3. The join offer no longer adds a bank that is already linked.
  - I2. A "signed out" answer no longer starts a check on every frame.
  - I3. A promoted viewer can use Add a full copy.
  - I5. The author is learnt on the first check.

## How this was tested

- **Scratch copies, built with their own target directories. No repo source was edited and nothing was committed.**
  - `scratchpad/c65/`: the live tree plus `src/share/c65_probe.rs`.
  - `scratchpad/c65live/`: the live tree plus `c65_live.rs` (the rig helpers from critic 63) and `c65_run.rs`.
  - `diff -rq` against the repo shows only these additions.
- **Baseline.** `cargo test --lib share::` passed 680 tests (`scratchpad/c65_base.log`).
- **Probes.** `cargo test --lib c65_` ran 7 tests; all passed (`scratchpad/c65_probe.log`).
  - Each P test asserts the faulty behaviour, so a pass means the problem is present.
  - P7 is the exception: it asserts the guard holds.
- **Live.** `cargo test --lib c65_run -- --ignored --nocapture` (output in `scratchpad/c65_live_a.out`).
  - Rig: `server/worker/scripts/local-ui-rig.sh`, already running on :18791, left running. Port 8799 was not used.
  - Throwaway accounts `c65{own,ed,vw}a@example.test` were all deleted afterwards (`delete_account -> Ok`).
- **Mutations.** Driver: `scratchpad/c65tools/run.py` with `muts.json`. Results: `c65tools/summary.txt`, one log per mutant.
  - The builder's 41 mutants (`fixmut_summary.txt`) were not repeated.

## Blocker

### B1. A links file unreadable at the first read lets a new link evict the link of a bank with unsent work

This is the critic 63 B3 class, reached through a rare trigger.

**How it happens**
- `CircleEditModel::ensure()` (`circle_edit.rs` ~876-889) merges any links added while the file was unreadable with "pending wins". Its `retain` drops every file link for the same circle bank (or the same local bank).
- While the file is unreadable, the in-memory list is empty, so each guard sees no link:
  - `add_link -> linked_elsewhere` finds nothing.
  - `queue_add` and the join offer's `in_banks` find nothing.
  - The circle page row shows **Add to my banks** for a bank this computer already has.
- `put_link` pushes the link into memory before `save()` fails. `add_link` therefore returns Err, and the page says "could not be saved ... Changes can't be sent from it", yet the bank is linked for the rest of the session.
- "Try again" on the strip (or any later `ensure`) then reads the file, and the new link evicts the old one. The next save makes it permanent.
- `take_shared_link` (share into a circle) has the same hole for a source bank already linked to another circle bank.

**When it triggers**
- The links file must be "there but unreadable" at the first read (`load_checked` -> `Err`). An example is a sharing violation from a sync tool or antivirus on Windows.
- Then add, join, or share once, and press Try again.

**Evidence**
- Probe P1: the copy with an unsent change loses its link to "local-bank-2" without any message, and the file written next lacks it.

**Fix**
- While `unreadable` is set, `add_link`, `add_link_replacing` and `take_shared_link` refuse without pushing anything into memory.
- In `ensure()`, a file link wins over a pending one when they clash on a circle bank, and the notice says so.
- Test: P1, inverted.

## Important

### I1. After joining again, the copy the offer points to still says "You are no longer in X" for the rest of the sign-in

**Where**
- `BankLinks::will_check` checks a gone link once per sign-in (`rechecked`). Leaving the circle uses that one check.
- Nothing re-arms it on a successful join: `on_join` and `on_offer` never touch `links`.

**What the person sees**
- The offer now correctly says "already in My Banks as ...".
- That bank's strip is red, with no Send and no Get latest, until the person finds Check now in "...".

**Evidence**
- Live L3: after the rejoin, the strip shows the stale line and `[]`. Only Check now gave the right strip.
- Probe P5 shows the same, with no heads request after the join.

**Fix**
- On a successful join, drop `rechecked` and `last` for this circle's links, or write the joined role into those links.
- Test: P5, inverted.

### I2. `is_author` belongs to the computer, not the account

`role_seen_by` does not cover it.

**(a) A non-author inherits the author's Restore and Trim**
- When `role_seen_by` changes, `apply_check_as` resets `author_known` but keeps `is_author` (`bank_links.rs` ~366-368).
- The check that made the change does not read the circle page, because `author_of` was `None` when it started.
- So a second member account inherits `is_author = true`, and `perm_of().can_rewind()` offers it Restore and Trim (probe P2).
- The server refuses with `403 circle_role_required` (`circlesend.ts:1268`, `circleversions.ts:176`), so this is a UI lie, not a leak.

**(b) Another account's visit removes the author's Restore and Trim**
- The circle page teaches `learn_author` from whichever account is viewing, with no account check (`circles.rs:1477-1482`).
- Account B's visit writes `is_author = false, author_known = true` into account A's link.
- A's later checks never learn it again: `author_known` is true and `role_seen_by` is still A.
- A loses Restore and Trim on the bank page until A opens the circle page again. Found from the code; not run.

**Fix**
- Set `is_author = false` whenever `author_known` is reset.
- Have the circle page teach only links whose `role_seen_by` matches the account signed in, or set `role_seen_by` as it teaches.

### I3. "Add a full copy" whose read comes back as the viewers' form moves the link from a copy that has its keys

**Where**
- `finish_landing` uses `add_link_replacing` whenever `full` is set, and `land` names the bank "(full copy)".
- Neither looks at `h.keys_hidden`.

**When it happens**
- The person is made a viewer again between the press and the read, and the bank hides its keys.
- The keyless copy then takes the link, and the copy that has its keys is unlinked (probe P4).
- No local text is lost, but the label is false and a B1-class swap is made without a word.

**Fix**
- When `full && keys_hidden` at landing, do not replace the link. Leave the new bank unlinked and say why.
- Test: P4, inverted.

### I4. The new guards and the B2 normaliser are largely unpinned: 11 of 15 new mutants survive

| Mutant | What it changes | Data-safety weight |
|---|---|---|
| m08 | the landing always uses `add_link_replacing` | B3 guard at landing untested; only the queue guard is pinned |
| m09 | `add_full_copy` ungated, so an edit link with unsent work can be replaced | B3 class, model-level gate untested |
| m07 | `linked_elsewhere`: a link whose bank is not counted yet is taken as gone | B3 default-deny branch untested |
| m01 | `settle_shuffle` removed on the apply side | taken questions' choices move on every Get latest |
| m02 | `settle_shuffle` also runs on full-form reads | an author's real reorder of choices never reaches anyone |
| m03 | Matching left column ignored by `order_free` | a real change hidden for viewers |
| m04 | locked choice's slot ignored | a real move of a locked choice hidden |
| m05 | Multipart Mc parts compared with order | B2 comes back for Multipart (live L1 shows it works today) |
| m10 | a member account counted as "another account" | wrong strip for a legitimate second member |
| m11 | `author_known` not reset on an account change | I5 / I2 path |
| m52 | "Sign in..." does not re-arm the check | I2 path, after the same account signs in again |

- Killed: m12 (`keys_kept` ignores the role), m15 (a share replaces an existing link), m22 (a follow link already at the newest version shows a review) and m29 (`perm_of` ignores a follow link).
- The B2 tests use only 4-choice Mc. Matching, locked and Multipart shuffles are covered only by this report's live run.

## Minor

- **Badge vs strip.** A copy that keeps its keys shows the badge "newer version" (probe P6, live L2), while its strip offers no Get latest.
- **Older links still get the I1 overwrite.**
  - Links made by a share, and all pre-D links, have `role_seen_by = None`, so another account's check still writes "none" into them (probe P3).
  - It heals at the owner account's next sign-in check, but until then the other account sees "You are no longer in" instead of the "another account" line.
- **Disk reads while unreadable.** `ensure()` reads the file again on every badge or strip call while it is unreadable: once per bank row per repaint.
- **Learning the author can repeat.** `learn_author` cannot learn while the bank is missing from the circle page (removed or hidden), so each 10-minute check adds a `GET /circles/:cid`.
- **Wrong words in one race.** A `viewers_form` copy promoted mid-review, whose apply is read stripped, is told the KEYS_KEPT text ("your copy has its answer keys"). This is safe, but the words are wrong.
- **Session end on the bank page.**
  - `bank_links_ui` drops `edit.take_signed_out()`.
  - A send or Get latest that meets an ended session leaves the Account window saying "signed in" while the strip says "Sign in...".
  - No loop, because `out` gates every check.
- **Account fingerprint.** `role_seen_by` is an unsalted 64-bit prefix of sha256(server + email). It is local only, but guessable for known addresses.

## Checked, holds

- **B1 (keys).**
  - Live L2: an editor's full copy, the bank then hiding its keys, the editor demoted, the owner sends v2.
  - The strip says the copy keeps its keys and offers [History]. A forced Get latest is `Blocked`, the keys and the unsent edit stay, and the link is unchanged.
  - Promoted again: the merge plus the send work, and the keys stay.
  - P7: the review refuses even when the strip never saw `hide_key`.
- **B2 (viewers' form).** Live L1: a viewers'-form copy of a bank with 2 four-choice Mc, a locked "I don't know" Mc, a Matching question with a distractor, and a Multipart with an Mc part.
  - After a one-stem v2, the review lists 1 of 5 rows and applies on the first try.
  - The unchanged questions' choices did not move, and a second Get latest says up to date.
  - A word-bank-only v3 lists exactly that question.
- **B3 (join offer).** Live L3: the rejoin offer shows the linked bank as "already in My Banks as ...", unticked, and the old link survives. The other, new bank was added as expected.
- **I2 (signed out).** No request loop.
  - `out` stops checks, and `open_quiet` is called only when a check would start.
  - `AccountModel::close` keeps `me`, so the gate stays Ready; no open/close ping-pong was found.
- **I3 (promotion).** Live L4: promoted viewer, strip [AddFullCopy, GetLatest], the full copy is an edit link, the old one is unlinked with a notice, and the full copy sends.
- **Old links.** `mode` defaults to edit and `author_known` defaults to false, so the author is learnt once. An unknown mode loads as follow.
