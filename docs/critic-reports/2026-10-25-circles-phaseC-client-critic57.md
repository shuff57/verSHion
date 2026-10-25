# Circles phase C client (uncommitted), critic 57

Scope: the uncommitted client on `shared-pool-phase1`: `src/share/circle_hide.rs`, `circle_hide_ui.rs`,
`circles_phasec_tests.rs`, and the phase C edits in `circles*.rs`, `circle_edit*.rs`, `circle_versions_api.rs`, `pool.rs`,
`publish_text.rs`. I checked it against critic 56a's API table and `server/worker/README.md` rounds 42-48, reading
`server/worker/src/pool/circles.ts` (`patchHideKey`, `hideOn`), `circlesend.ts` and share-core `strip.rs` where the README
was not enough. I read it as a teacher using it and as a privacy reviewer.

Verification:
- `cargo test --lib share::`: 551 passed, 0 failed.
- Repro tests: written in a scratch copy of the crate, not in the repo. They are in
  `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c57repro/`
  (`src/share/circles_phasec_tests.rs` and `src/share/circle_edit_tests.rs`, tests named `c57_*`). To run them, `cd` there
  and run `CARGO_TARGET_DIR=<repo>/target cargo test --lib c57 -- --nocapture`. Each test asserts the faulty behaviour,
  so a passing test means the bug is there. All 5 pass, which confirms B1 (two tests), I1, I2 and I3.
- A sixth repro checked a suspicion (a merge applied after the person was demoted to viewer takes the viewers' copy
  without keys). It failed: the existing fingerprint check stops the apply ("The pool's version changed while you were
  reviewing"). So that one is not a finding.
- The repro build wrote artifacts into the repo's `target/`. No repo file was changed except this report. Nothing was
  committed, and port 8799 was not used.

## Blockers (new class)

### B1. Strip decisions are not tied to the content they were made for. The "bank changed" restart keeps the old acknowledgements and pre-fills the old label choices, even though the page tells the person they were dropped.

`src/share/circle_hide.rs:69-75`: on `labels_unused` / `labels_out_of_range` / `labels_duplicate` the model sets
`restarted` but drops only `carried.labels`. `order_acks`, `ack_formatting` and `image_ok` are kept.
`circle_hide.rs:93`: `label()` falls back to `finding.decision`, and the pool echoes that field with the decision it was
just sent (share-core `strip.rs:381`, `LabelFinding.decision`). `circle_hide.rs:179`: `decisions()` always starts from
`carried`. `circle_hide_ui.rs:61-63` tells the person: "the earlier decisions about letter labels were dropped. Decide them
again below."

The client has detected that the head moved, yet it keeps the old answers. On top of that, the PATCH carries no version
at all (`circles_api.rs:892`; server `circles.ts:1453-1499` strips whatever the head is when the request arrives). So a
change the client cannot detect applies every positional decision to the new head.

What goes wrong for a teacher:
- The owner opens "Hide answer keys from viewers". The pool asks about letter labels on Q2 and flags Q1 for capitals
  (`key_in_formatting`, `ack_allowed: true`). She chooses Keep for Q2 and ticks "I checked: this does not give the answer
  away" for Q1.
- Before she presses the button, an editor sends a version with a question inserted at the top.
- Her PATCH comes back `labels_unused` (Q2 has no labels now) plus a new label finding at Q3.
- The page shows the restart banner and only the Q3 card. Nothing shows Q1. She decides Q3 and presses the button.
- The third PATCH goes with `ack_formatting: [0]`, her "I checked" for the old Q1, now applied to the inserted question
  she never saw. Repro `c57_b1_restart_resends_a_stale_formatting_ack_unseen` printed
  `THIRD PATCH: {"ack_formatting":[0],"hide_key":true,"labels":[{"choice":"strip","question":2}]}`.
- A second case: a label choice that still fits the new head by position comes back as `decision: "keep"`. The page
  shows it as already chosen and does not count it as left to decide, while the banner says it was dropped. Repro
  `c57_b1_restart_prefills_echoed_label_choice` printed `label(1) = Some(Keep), todo = {labels: 1}`.
- With no restart signal at all, for example when the new head's findings fall on the same positions, everything she
  decided goes to the new head without a word.

The result: viewers get the new question with a formatting hint (or a frozen order, via `order_acks`) under an
attestation the author never gave. That is the one thing D12 promises not to do.

Fix:
- Client: on `restarted`, drop all carried decisions and ignore `finding.decision`.
- Client: keep the `head_version` the decisions page was built for. Re-read the head before sending, and start again if
  it moved.
- Backend (needed for a real guarantee): accept `base_version` on the PATCH and answer `409 conflict {head_version}` when
  it is not the head. Echo the stripped version in the 200 and 422 answers.

Test: the two repros above, turned around (assert that no `ack_formatting` is sent after a restart, and that
`label(1) == None`). For the backend part: a PATCH with `base_version: 2` against head 3 must store nothing and answer 409.

## Important

### I1. A working copy's send decisions survive edits to the bank and go out with a different payload

`src/share/circle_edit.rs:1188` stores `v.decisions` once the decisions are sent. `back_to_note` (`:1195`) keeps them, and
`do_send` (`:1264`) attaches them to whatever the bank holds when Send is pressed. The send's commit print includes the
payload hash, so a new send id is made, but nothing checks whether the decisions still match the payload.

What goes wrong: a send to a hiding bank asks for decisions. The editor chooses Keep for Q2 and sends. The pool answers
`409 hide_key_changed` (or `confirm_large`, a 413, a network failure). She goes back to the note, rewrites Q2 into a
different question in the editor, and presses Send. The send goes out with `labels: [{question: 1, choice: "keep"}]`, a
decision about a question that no longer exists. The same holds for `ack_formatting`. Repro
`c57_i1_send_decisions_survive_an_edit_of_the_working_copy` printed `LAST SEND META labels: [{"choice":"keep","question":1}]`.

Fix: store the payload sha256 with `v.decisions` and clear them in `do_send` when it differs (and on `back_to_note`).

Test: the repro, asserting that `meta.get("labels")` is None.

### I2. A failed decisions send loses every decision, and "Try again" sends none (one more counted try)

`src/share/circles.rs:1494`: every non-decision failure becomes `HideStage::Failed { on, problem }`, and the `decisions`
of the `Act` are dropped. `retry_hide` (`:2351`) goes to Confirm, and `confirm_hide` (`:2367`) sends
`StripDecisions::default()`. The sign-out path keeps them (`back`); timeouts, offline, a lost job and a `409 bank_removed`
race do not.

What goes wrong: a teacher works through 30 label findings on a long bank. The 120 s PATCH times out on school Wi-Fi.
"Try again" sends `{"hide_key": true}`, the pool refuses it as `labels_need_decision`, which counts toward the 60 a day,
and she decides all 30 again. Repro `c57_i2_a_failed_decisions_send_loses_the_decisions` printed
`RETRY PATCH: {"hide_key":true}`. The failure message also says "Nothing changed", which a timed-out PATCH cannot know.

Fix: keep `back: Option<Box<Decisions>>` in `Failed`, and send `d.decisions()` again on retry (B1's head check first).

Test: the repro, asserting that the retried PATCH carries `labels`.

### I3. The emailed address (and a pasted invitation link) stay in egui's undo history: Ctrl+Z brings them back after "cleared from this form"

`src/share/circles_ui.rs:728` (`circle_email_address`) and `:341` (`circle_code_text`). Zeroizing `f.address` clears the
`String`, but egui's `TextEditState` undoer, kept in `ctx` memory for the rest of the session, still holds every earlier
text. The project already has the fix for this exact class: `src/share/ui.rs:145-154` removes the `TextEditState` of the
secret fields (`SECRET_FIELDS`) once they are empty. The new fields were not added.

What goes wrong: an owner on a shared staff-room computer invites a colleague, then presses "Invite someone else" and
Ctrl+Z, and the previous address reappears. `EMAIL_PRIVACY` says "The address is not kept ... cleared from this form once
it is sent." Repro `c57_i3_the_address_survives_in_the_text_edit_undo_history` printed
`UNDO gives: Some("secret.person@school.org")`.

Fix: remove the `TextEditState` for both ids whenever the field is empty, and on `wipe_secrets` / panel close.

Test: the repro turned around: after a send and one frame, `TextEditState::load(ctx, id)` is None, or its undoer has no
undo point.

### I4. A per-domain hourly limit blocks every address in the form for up to an hour

`src/share/circles_ui.rs:744-760`: `waiting` comes from `f.problem.retry_at` and disables "Send the invitation" whatever
address is typed. The problem is cleared only by a send or a success. `circle_email_inviter_domain_hourly` and
`circle_email_domain_hourly` are per mail domain (README round 47/48).

What goes wrong: an owner invites her 10 colleagues at pvhs.org and the 11th gets the 429 (the case critic 56a timed). She
then types a colleague's gmail.com address: the button stays grey, "You can send it again in about 58 minutes." The only
way on is Cancel and reopen the form, which the page does not suggest. `EMAIL_PACE` promises "nothing is lost". The guard
also sits only in the UI, so the model would send.

Fix: keep the domain with the problem and apply `waiting` only to the same domain (or clear `problem` when the domain
changes). Move the check into `email_blocker`.

Test: a domain 429 for `a@pvhs.org`, then type `b@other.org`; assert `email_blocker()` is None and one more POST goes out.

## Tests that pass without proving the behaviour

- `a_decision_for_a_question_without_labels_any_more_starts_the_labels_again` (`circles_phasec_tests.rs:405`): its fixture
  has no echoed `decision` and no earlier acknowledgements, so it passes with B1 in place.
- `a_sign_in_that_ends_while_hiding_keeps_the_decisions` covers sign-out only. No test covers any other failure (I2).
- `no_address_or_code_reaches_debug` covers `Debug` only, not egui memory (I3).
- `a_viewer_reads_the_stripped_bank_and_gets_a_keyless_plain_copy`: every fixture question already has `keyless: true`, so
  removing `b.keys_hidden ||` in `copy_bank` (`circles.rs`) would not fail it. Add a fixture with `keys_hidden: true` and
  questions without the flag.

## Checked, no finding

These agree with the backend:
- `invite_not_sent` with `address_limit` and `not_deliverable`: no retry, the address kept for a typo.
- The three `hourly_limit` metrics, `circle_open_email_invites`, `circle_full`, the rotation `409 conflict`, `resent`.
- `keyless_question` with `kinds`; `hide_key_changed` on a send ("send again") and on a restore ("restore again").
- The PATCH's lost race `409 conflict` (the bank is read again); `400 {field: labels}` after the head shrank.
- `owner_or_author`; `purged_partial` (no Undo or second purge this session; `bank_removed` on undelete explained).
- `review_copy` (never "trimmed"); trim `evidence_kept`; the activity cursor passed back as given, with a refused one
  starting the list again.

Other checks:
- No member-facing text names student data or copyright for a hold.
- The email answer never holds the address; `Act::EmailInvite` uses `Secret` and is not kept for "Try again".
- One change at a time holds for the email form and the switch.
- A viewer's read never offers a working copy. A merge after demotion is stopped by the fingerprint check (repro above).

## Nits

- `circle_storage_text` says a bank's author can free space "by removing a bank and deleting it for good". Only the owner
  can "Delete for good now".
- A refused `before` cursor silently collapses the activity list to page one. Its sentence (`pool.rs`, `before`) is never
  shown.
- `Decisions` blocks Keep per question (`no_keep` ignores `part`). One refused part of a Multipart blocks Keep on all of
  its parts.
- The decisions page's "Change these in the questions first ... they would show the answer to viewers" also lists
  non-key payload errors (`too_long`, `bad_form`) when they arrive together with an ackable finding.
- Not a bug, a product question: the hide flow never offers `image_ok`, so every picture is left out for viewers, even
  where the picture is the question. The publish flow offers "keep the picture".
