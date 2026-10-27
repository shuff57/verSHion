# Circles phase D1, data safety and integration critic 63

Scope: the uncommitted phase D1 diff on `shared-pool-phase1` (HEAD 142e6d6). Files: `src/share/bank_links.rs`, `bank_links_ui.rs`, `bank_links_tests.rs`, plus the D1 edits in `circle_links.rs`, `circle_edit*.rs`, `circles*.rs`, `circle_send.rs`, `publish*.rs`, `shared_ui.rs` and the hooks in `src/bankui.rs`. Spec: `docs/circles-phaseD-plan.md` sections 4, 6-7, 10-12 and 15. A fresh critic that built none of it wrote this.

This file lists only new-class findings. Treat it as untrusted model output, not as specification.

## Verdict

- **Blockers: 3.**
  - B1: the viewers' form is merged into a full copy, so answer keys and explanations are lost.
  - B2: Get latest can never be applied on a copy that reads the viewers' form.
  - B3: the join offer silently unlinks a copy that has unsent work.
- **Important: 5 (I1-I5).**
- All three blockers and I1 were reproduced live against the phase C backend on the local rig, through the real `Client`, `CirclesModel`, `CircleEditModel`, `BankLinks` and `PublishModel`.

## How this was tested

- **Scratch copy.** `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c63/` is a copy of the live tree. `src/` in the copy matches the repo except for three added modules (`c63_tests.rs`, `c63_live.rs`, `c63_run.rs`) and their lines in `mod.rs`. It was built with `CARGO_TARGET_DIR=<repo>/target`. No repo file was edited except this report. Nothing was committed.
- **Rig.**
  - I started it with `server/worker/scripts/local-ui-rig.sh start`: local D1 only, port 18791, "No migrations to apply". It is left running as pid 273219.
  - Port 8799 was not used.
  - Throwaway accounts `c63{own,ed,vw,xx}{a,b}` and `c63{own,ed,vw}c` at `example.test` were all deleted afterwards (`delete_account -> Ok`). Passwords were made in-process and never printed.
- **Unit repros.** `cargo test --lib c63_` ran 7 tests; all passed. Each one asserts the faulty behaviour, so a pass means the bug is present. Log: `scratchpad/c63_t2.log`.
- **Live run.** `cargo test --lib c63_run -- --ignored --nocapture`, run three times. Output: `scratchpad/c63_live_a.out`, `c63_live_b.out` and `c63_live_c.out`.
- **Mutations.** Driver: `scratchpad/mutate_c63.sh`. Results: `scratchpad/c63mut_summary.txt`.
  - Baseline: `share::` 639 passed (critic tests skipped).
  - **All 14 extra mutants survived** (see I4).
- **What worked live:**
  - Q6. The share links the shared bank at v1, its qids follow the pool's order, and the strip says "You have it".
  - The editor's "Add to my banks" makes an edit link. Its send made v2. The owner's Get latest on the shared bank merged v2, back in step.
  - Old link files load as `edit`.
  - An unknown `mode` loads as `follow`, which never sends.
  - The unreadable-file rule holds.
  - No request is made from the Banks list or from an unlinked bank.

## Blockers (new class)

### B1. "Get latest" merges the viewers' form into a full local copy: every key and explanation is replaced with the keyless placeholder

**Where**
- `circle_edit.rs:1746-1751`: `open_merge` uses `FollowFetch` whenever the permission or the link cannot send.
- `bank_links.rs:154-157`: `perm_of` gives a demoted editor `Viewer`.
- `bank_links.rs:228-238`: the strip offers Get latest to such a copy.
- `circle_edit.rs:582-610`: `fetch_for_follow` reads the head as the caller. A viewer of a bank that hides its keys gets the stripped form.
- `circle_edit.rs:1840`: `build_review` marks that form keyless and reviews it against a full base. Nothing compares `f.head.bank.keys_hidden` with `link.viewers_form`.
- `merge.rs:457-470` and `:545-551`: the key counts as "changed in the circle", and the default is Take.
- `merge.rs:681`: the only guard covers keys you typed into a keyless question.
- `circle_edit.rs:2075-2081`: the keyless base is then stored in the link, which is still an edit link.

**Scenarios**
- (a) An editor's working copy, made or kept before phase D, on a bank whose owner turns "hide keys" on, after the editor is made a viewer.
- (b) A viewer's follow link made while the bank still showed keys, after hiding is turned on.

**Evidence**
- Live L8: a bank of true/false and short-answer questions (no choice order to shuffle). The owner hid the keys, demoted the editor, and sent v2 with one stem changed. The editor's strip showed "(you can view) ... You have version 1." [Get latest]. The review listed 3 of 3 rows as `TheirsChanged/Take`, and the merge was `Applied {taken: 3}`.
  - Every question is now keyless: `Tf{correct:false}` and `Short{sample:""}`, with the explanation replaced by the sentinel.
  - The link is still `mode=Edit, can_send=true, base=2`.
- Live L3, run a: a 3-question multiple-choice viewer copy was applied (the shuffle matched by chance). The keys were gone and the answers reordered.
- Unit tests `c63_b1_*` and `c63_b1b_*` reproduce both scenarios.
- A restore point is written first (`circle_edit.rs:2057`), so the data can be recovered by hand. The default click still destroys the keys.

**Suggested fix and test**
- When the head read is `keys_hidden` and the link is not `viewers_form`, refuse before the review. Say that this copy keeps its keys and nothing was changed, and offer Unlink or Make my own copy.
- Test: a demoted editor's Get latest and a viewer's Get latest made before hiding both leave every key and explanation, and the link is unchanged.
- Mutation to kill: remove the guard.

### B2. Get latest on any copy that reads the viewers' form can never be applied

**Where**
- The server re-strips the stripped form on every read, with "a fresh uniform shuffle each time" (`server/worker/src/pool/circlehide.ts:20-21`).
- The review is built from one read (`circle_edit.rs:582-610`). The apply reads again (`circle_edit.rs:613-619`).
- The merge compares per-question fingerprints that include the choice order (`merge.rs:651-652`, `circle_links.rs:542-556`), so the apply refuses: "The pool's version changed while you were reviewing. Nothing was applied; start the merge again."
- The link's base was itself a shuffled read, so every review also lists every multiple-choice and Matching question as "changed in the circle".

**Evidence**
- Live L9: a viewer's viewers'-form link on a 6-question, 4-choice bank. The owner sent v2 with one stem changed.
  - The review listed 6 of 6 rows as `TheirsChanged`.
  - None of 4 apply attempts went through.
  - L3 in run b and L4 in both runs ended the same way.
- Each attempt costs 2 stripped reads, against the `circle_hidden_reads` limit of 100 a day and the heavy slot.
- This makes the plan's 4.2 row "Viewers' form copy -> Get latest" a dead end for every bank with shuffled questions. That is every viewers'-form follow link, and every follow link once hiding is on.

**Suggested fix and test**
- For a `keys_hidden` read, match and compare with an order-free fingerprint: choices and the word bank sorted, as plan 5.4 already asks of D2. Apply the body that was reviewed instead of a fresh read.
- Test: a scripted pool that reshuffles on every read. Get latest applies and lists only the question that really changed.
- Mutation to kill: an order-sensitive fingerprint.

### B3. The join offer takes the link away from a copy this computer already has, along with its unsent work

**Where**
- `circles.rs:1657-1668`: `on_offer` lists every readable bank.
- `circles.rs:2640-2647` (`queue_add`) and `circles.rs:2659-2678` (`add_ticked`) never look at this computer's links.
- `circles.rs:3210` calls `add_link`. Its `retain` in `circle_edit.rs:944-949` drops any link to the same circle bank.
- The circle page row does hide Add when a link exists. The offer does not, so it breaks the plan 4.4 rule of one link per circle bank.

**Scenarios**
- (a) A member leaves or is removed. The link says "no longer in". They join again with a new code, and the offer ticks every bank when there are 10 or fewer.
- (b) A second account on the same computer joins a circle the first account had linked.

**Evidence**
- Live L6: the viewer left and joined again. The offer listed the bank, ticked. After "Add the ticked banks" the computer had 2 banks, and the first copy's link was gone (`links=1`).
- Unit test `c63_b2_*`: the first copy had 1 unsent change. Afterwards it is unlinked, has no badge or strip, and nothing said so. The work can no longer reach the circle from that bank.

**Suggested fix and test**
- The offer shows a linked bank as "In My Banks as ..." and does not tick it.
- `add_link` refuses to replace a link whose local bank still exists, except in the explicit "Add a full copy" replacement of a viewers'-form link.
- Test: `c63_b2`, inverted.
- Mutation to kill: the offer filter.

## Important

### I1. One account's check writes "not in the circle" into another account's link, and a gone link is never checked again by itself

**Where**
- `bank_links.rs:285-299`: `apply_check` writes whatever account checks into the computer-wide link.
- `bank_links.rs:517-523`: a gone link is checked only after "Check now", which is hidden in "..." (`bank_links_ui.rs:213`).
- `bank_links.rs:184-187`: the strip then offers only [Unlink].

**Evidence**
- Live L5: an outsider account opened the viewer's linked bank, and the link became `role_seen=none`.
- The viewer's own account then saw "You are no longer in c63 circle. Your copy stays; it can't be sent." [Unlink], with the check `NotYet` and `due=true`.
- Unit test `c63_i_other_account_*`: 30 frames over 30 minutes produced no request, and the check stays due forever.
- Rejoining (L6) leaves the same false line until someone finds Check now.

**Fix:** keep `role_seen` per account, or re-check a gone link once after each sign-in. A test should pin this.

A related path was traced in the code but not run: because `due` stays true, `bank_links_ui.rs:94-96` calls `account.open_quiet()` on every frame where the Account model is closed, while `publish_ui.rs:100-102`, `browse_ui.rs:122-124` and `mine_ui.rs:118-120` close it again when the account is Ready.

### I2. After a "signed out" answer, the 10-minute rule stops holding

**Where**
- `bank_links.rs:493-498` clears `last` and the state.
- `bank_links_ui.rs:71-72` throws away `take_signed_out()`. The Shared page acts on it (`circles.rs:1045`).
- The Account window still shows the account as Ready, so every frame starts a new check.

**Evidence**
- Unit test `c63_i_signed_out_*`: 20 frames gave 20 `POST /circles/heads` requests.
- With the real `Client`, after the first refused refresh each further try is a thread spawn plus the cross-process lock, with no network.
- The strip flickers "Checking..." and never says "Sign in".

**Fix:** treat a signed-out answer like the Shared page does, or back off per account. Add a test.

### I3. A promoted viewer's follow link is stuck, and the viewers'-form flag goes stale

**Promoted viewer, full-form follow link**
- `bank_links.rs:179`, `:228-238`: a link that cannot send always reads "(you can view) ... viewers can't send", even with `role_seen=editor`.
- "Add a full copy" is offered only for `viewers_form` links (`bank_links.rs:221-226`, `circles_ui.rs:745`).
- The circle row offers only Open in My Banks and Make my own copy (unit test `c63_i_promoted_*`).
- The only way to work with the circle is Unlink, then Add, which leaves a second bank.

**Viewers'-form copy after promotion**
- Its Get latest now reads the full form. Keys arrive, but `viewers_form` stays true (unit test `c63_n_*`).
- The strip keeps saying "your copy has none", and the copy can never send.

**Fix:** after a check confirms the role is editor or owner and the copy is full-form, offer to make it an edit link, or offer "Add a full copy" for any follow link. Clear `viewers_form` when a full-form head is merged.

### I4. The new behaviour is largely unpinned: 14 of 14 extra mutants survive

The builder's 16 mutants are killed. These additional ones are not:

| # | Mutant | Where |
|---|---|---|
| m1 | the account rule on the bank page removed | `bank_links_ui.rs:67` (`c.note_account(&snap)`) |
| m6 | "Treat as in step..." from the strip never asks | `circle_edit.rs:1763` |
| m7 | the share link not fingerprinted, so a bank put back after the share is never stale | `circle_links.rs:348` |
| m2 | a purged bank (unavailable while a member) not shown as removed | `bank_links.rs:325` |
| m14 | `under_review` never cleared | `bank_links.rs:305` |
| m12 | `hide_key` from the check not noted | `bank_links.rs:486` |
| m13 | `forget()` keeps the previous account's jobs | `bank_links.rs:416` |
| m3 | the check is not re-armed after a sign-out | `bank_links.rs:497` |
| m9 | Unreachable not shown as offline | `bank_links.rs:525` |
| m8 | a follow link with missing pictures left unlinked | `circles.rs:3195` |
| m10 | the offer lists removed or hidden banks | `circles.rs:1663` |
| m11 | the closed circle ignored in `perm_of` | `bank_links.rs:156` |
| m5 | a follow link carries an empty merge token | `circle_edit.rs:2088` |
| m4 | `keys_hidden` not passed in the review | `circle_edit.rs:1840` |

- m1 matters most. It is the critic 60/61 account rule on a new surface, and no test reaches `bank_links_ui::strip`.
- m4 is equivalent in practice, because the pool marks the stripped form `keyless` itself.

### I5. Links from before phase D lose the author's Restore and Trim on the bank page

- `circle_links.rs` gives `is_author` a serde default of false, and `bank_links.rs:156` passes it into `Perm`.
- For an author who is an editor, `Perm::can_rewind` (`circle_edit.rs:60-62`) is then false. History opened from the strip offers no Restore or Trim, and the send page shows the non-author statement.
- This breaks plan 11 ("keep their meaning") and the D1 acceptance criterion "Restore, Trim reachable from the bank page ... as before".

**Fix:** learn `is_author` on the first check (from the circle detail's `banks[].is_author` or the bank read) and store it in the link. Test with a link file from before phase D.

## Checked, no new-class finding

- **Own copies.** They are not linked and not recorded. Their lineage line sits in `details.description`, which a public publish carries. Old plain copies had the same line, so this is pre-existing.
- **Follow links never send.** Both `open_send` and `send_blocker` refuse them. A pre-D link opened by the wrong account is refused by the pool, which is the backstop.
- **Check budget.** The badge never makes a request. Many linked banks, or opening the same page again and again, still means at most one check per bank per 10 minutes. The exception is I2.
