# Critic 45a: circles phase B (round 37), product / functional lens

Scope: uncommitted phase B backend on `shared-pool-phase1` (`circleversions.ts`, `circlesend.ts`, `privacynear.ts`,
`circles.ts`, `circleadmin.ts`, `publish.ts`, `shared.ts`, `index.ts`, `poolmail.ts`, README "Round 37"). Spec:
`docs/circles-plan.md` 4, 13.2, D1-D19.

Verified by running: `cd server/worker && npm test` = 572 pass, 0 fail (309 s). Own scenarios (workable files in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c45/`:
`s1.test.ts` S1-S4, `s2.test.ts` S5-S7, `s3.test.ts` S8, `s4.test.ts` S9; run with
`node --experimental-strip-types --no-warnings --test-reporter=spec --test <file>`).

What held up (tried to break, could not): compare-and-set with exactly one winner and the true head on the loser;
replay by `commit_id` (also after a newer version landed: 200 replayed; the same id with another body or another
rollback target: 409 `commit_id_conflict`); history paging over 120 versions (3 pages, 120 lines, no duplicates or
gaps, `next_before` chain correct, `before=0` 200, `before=1` empty); rollback to a pruned version (404
`version_not_retained`); rollback to a kept version (new version 121, `rollback_of`); editor who is not the bank's
author cannot roll back (403), the owner can; demoted author cannot roll back (403); the 428 guard and its refund;
licence `422 license_author_only`; unchanged resend on a later day reports 0/0/0 (no date churn).

## Blockers (new class)

### B1. A clean send lifts the student-data hold and thereby re-opens the dirty version to every member (HIGH, privacy)

`circlesend.ts:715` (`DELETE FROM circle_holds ... source = 'privacy' ... NOT PRIVACY_SQL`) and `:719`, plus
`circles.ts` `getBank` version branch (`status IN ('current','previous')`, gated only by `requireReadable`).

The hold is on the bank, not on the offending version. When the author sends a clean version (or rolls back to one),
the hold is deleted in the same batch, the bank becomes readable again, and `GET .../versions/:n` serves the dirty
version `n` (it is `previous`, kept for 20 versions) to every member, with the confirmed student text in the body.
The `privacy_text_match` admin flag is still open at that moment.

Repro (S1): admin confirms a student sentence as blocked; editor Eli sends it in v2 (`held: true`, hidden from
everyone but the author); Ann sends clean v3 (`held: false`). Eli (and any member) then `GET
/circles/:cid/banks/:bid/versions/2` = 200 and the body holds `"Jenny Smith in period 3 scored 42 ..."`. Printed
output: `E reads v2: 200 [ 'A clean question ...', 'Jenny Smith in period 3 ...' ]`, `flags open [ { resolved_at: null } ]`.

Why it matters: README says "a later version without it lifts that hold" as a feature, but the effect is that the
hold protects the data only until the first clean send, which is exactly what an honest author does right after the
notice. The plan's reason for the check (13.2: "private circles are where a class list would be pasted") is defeated.
Same for `privacy_near`, and for a rollback to a clean version.

Fix direction: remember which versions raised the hold (a per-version `held`/`student_data` mark); keep those
versions unreadable to non-authors (history line may stay, body not, `kept: false` for members) until an admin
resolves the flag, or let only an admin release; and never use them as rollback targets for non-admins.

### B2. Byte cap deadlock at steady state: a circle can never send or roll back again, and the error tells the user to do something that does not exist (HIGH, availability)

`circlesend.ts:683/699` (`BYTES_SQL + new <= cap`) is checked BEFORE the prune statement (`:742`) that would free the
oldest version; `whyLost` `:849` says "Remove a bank or an older version first". There is no route that removes an
older version.

Repro (S5): cap = 20.5 x one version's bytes. Versions 1-20 are accepted; v21 gets `413 quota_exceeded` ("storage is
full ... Remove ... an older version first"); a rollback to v18 also gets `413` (the repair path is blocked too);
`kept = 20`. Nothing the member can do frees it except removing a whole bank, and a removed bank's bytes count until
purge (14 days, or purge-now by owner/author).

Real-world reach: images are charged per version (README documents "images per version", `bytes` includes
`imageBytes`), so sustained editing is only possible while the sum of all banks' sizes is under cap/20 = 10 MB. One
bank with 5 images of 2 MB (the image max) is 10 MB per version: the 21st version is refused at the full 200 MB. The
final state (after pruning) would fit; only the order of operations refuses it.

Fix direction: count the cap net of the versions the same commit will prune (sum of 'previous' with
`version <= new-20`, not held/reported), or prune first inside the batch before the cap check; at least fix the
message. Document the 20-version multiplier either way.

### B3. Two questions sent with the same `published.qid` both lose their identity, and with it the recorded public origin and licence bound (MEDIUM-HIGH, silent loss; depends on the client sending the duplicate's qid)

`circlesend.ts:416` (`counts.get(s) === 1`) and `:483-492` (origins carry by circle qid).

A qid that appears twice in a send is given a new id in BOTH positions, so the original is counted "removed" and two
"added", and the origin carried for the original qid no longer matches anything.

Repro (S8): Ann sends v1 with her own question and a question with a declared origin (Dee's CC-BY-SA public item,
bank licence CC-BY-SA-4.0). Eli sends v2 with the same three questions plus a copy of the origin question that keeps
`published.qid` (what the app's Duplicate does: `src/bankui.rs:2567` `duplicate_in_bank` clones the whole question with
`..clone()`, `published` included). Result: `added 2, removed 1`, `origins after dup: []`, and Ann's `PATCH
license=circle-only` now returns 200 (before, `license_incompatible`). The credit to Dee and the licence bound are
gone with no signal to anyone.

Fix direction: keep the first occurrence's qid (and its origin), give new ids only to the later duplicates. This also
keeps the large-change guard honest (a duplicate-heavy edit no longer looks like removals).

## Variants and limits to document

1. `oldest_kept` is not "everything from here on is kept" when a reported (or held) old version is retained
   (circles.ts `oldestKept`, README promise on 404 `version_not_retained {oldest_kept}`). S2: a report on v1, then 29
   more sends: kept = {30..11, 1}, `oldest_kept: 1`; `GET .../versions/5` = 404 `version_not_retained` with
   `oldest_kept: 1`; merge-initiate for base 5 says `base_kept: false, oldest_kept: 1`. The per-line `kept` and
   `base_kept` are right; a client that compares `base >= oldest_kept` will be wrong. Offer a `kept_versions` list or
   a `first_contiguous_kept`, or document "use `kept`/`base_kept`".
2. The licence is frozen with the author forever: a removed or erased author leaves nobody who can change it
   (`PATCH` needs the author as an active member; editors send with the bank's licence). S9: after the author is
   removed from the circle the bank stays readable and editable but its licence can never change.
3. A demoted author (now a viewer) can still `PATCH` the licence of a bank the editors work on (S9: 200). Consistent
   with D19 but surprising; say so.
4. A student-data hold raised by an editor's send hides the AUTHOR's whole bank from every member, including all earlier
   clean versions, and only the admin is mailed (poolmail); the author gets no message, only `held: true` in the
   bank's own responses. The sender who pasted the text cannot read or fix the bank afterwards (404). Only the author
   (or an admin) can clear it. If the author left the circle, it is stuck until an admin acts.
5. While a bank is held, no version is pruned (documented) and bytes keep growing; combined with B2 a held bank can
   fill the circle's 200 MB.
6. Merge-initiate `between` is the NEWEST 50 versions after the base (`ORDER BY version DESC LIMIT 50`), so for a
   long gap the lines right after the base are the ones left out. `between_total` says so; the client must not treat
   `between` as the contiguous list.
7. A send whose head index is missing in R2 skips the large-change guard and also drops carried origins (the
   `prevIdx` is null, `circlesend.ts:403-411,485`); only the guard is in README.
8. Rollback restores the body and qids but not the listing fields (title, subject, course, grade), by design; say so
   in the history UI wording (a rollback to v1 after a rename leaves the new name).
9. A replay returns the first answer's `held`, which can be stale (the bank may since have been released or held).
10. The 20 a day rollback cap and the 100 a day send cap are per account across all circles; a team doing a merge
    session of many small sends shares them.
11. Dropping a public-origin question in a version frees the bank of that licence bound (37-14 says so on purpose);
    a later rollback brings it back. Fine, but the licence UI should warn before the rollback fails with
    `license_incompatible`.

## Nitpicks

- `replayOf` answers `409 commit_id_conflict` for a commit_id that exists for ANOTHER account (`circlesend.ts:266`);
  it is an existence oracle for a guessed id (ids are random, so low value).
- Rollback acquires the heavy slot after it has already compared the head (`rollbackOnce`); a rollback that waited is
  rejected only by the commit CAS (correct, just late work).
- Merge-initiate with `base == head` answers `400` with "Merge from an earlier version than the circle's current
  one"; an up-to-date client gets an error rather than "nothing to merge".
- `getVersion` passes `Number(n)` through; the route regex limits it to 10 digits so it is safe, but values above
  `head_version` count as an id miss (`item_miss`) for a merely-too-new version number.
- README "17 tests" is right only counting the four 37-11 scenarios as separate tests (13 `test(` blocks + 4).

## Verdict

Not clean for a commit as-is: B1 (privacy re-exposure through the history after an auto-lift) and B2 (cap
deadlock; the repair path is blocked too) are real, reproduced, and new to phase B. B3 is a silent data-lineage and
licence loss that needs only a duplicated question. The compare-and-set, idempotency, history paging, rollback rights
and refund accounting are sound.
