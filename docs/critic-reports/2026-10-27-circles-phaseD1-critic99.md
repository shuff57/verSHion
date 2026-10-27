# Circles phase D1, round 18, critic 99 (data safety)

Saved by the lead, condensed from the critic's reply. Scratch build HEAD 0d17a83 + rounds 15-18: `cargo test --lib` 1958 passed (1971 with 13 probes). Blockers 0, important 0, minor 9. Mutants: 60, 48 killed, 12 survive (2 real gaps, 4 smaller, 6 equivalent). Probes in scratchpad/c99/src/share/c99_probe_tests.rs.

## Write order now
First pass of a read and Undo pass: links file (`at_read`, `inherit` true) -> safety copy -> banks.json -> links file (done + news). Put mine back / Put all: banks.json -> links file. Settle/Dismiss/retry: links file only. Every crash window is safe; the problem is a failure path (m1).

## Minor
1. **m1:** the held-pass retry (61 s or Try again) sees `at_read` already set in memory, skips the links write, and writes banks.json anyway; strip then offers Send; after a restart her later Medium becomes High again and the Two Low->none news row is lost. Fix: if the links file is still unsaved save it first and hold on failure; extend s18_h11 past 61 s.
2. **m2:** links file unwritable until quit undoes Put mine back after restart (her value equals `at_read`, pass re-run takes the circle's again).
3. **m3:** re-run after restart does not move `sent_unread.modified` / `sent_record.modified`; strip says "Changes you made since are not sent yet" for nothing.
4. **m4:** restart forgets a held Undo pass (`inherit_missing` in memory only); Send carries her pre-read Low over a colleague's High, no news; stale `at_read` stays.
5. **m5:** safety copies multiply by number of held banks (3 held banks, 10 min editing another: 33 copies; round 17 would make 3). Fix: one "copy made for this state" for the whole model.
6. **m6 (known risk, confirmed):** setting the circle's own difficulty during a hold lists it as the app's change; Put mine back reverts her choice.
7. **m7:** a held Undo pass does not hold Get latest's review (`merge_waits_for_difficulty` requires `inherit`; unclear-send branch precedes Held); review says "You changed it, so it is kept" about her pre-read Low.
8. **m8:** `inherit` plain bool (non-bool drops the whole link); non-bool `changed` drops the whole news; an `at_read` entry with no `difficulty` read as "none" and taken as truth.
9. **m9:** a colleague's version that happens to match her old difficulty says "yours … was sent".

Mutation gaps: m12 (news not added on success), m24 (settling does not save links), smaller m21, m17, m61, m80; no test for the m1 retry.
