# Critic 46a: circles phase B (rounds 37-38), product / functional lens, fresh eyes

Scope: uncommitted phase B on `shared-pool-phase1` (migration 0110, `circleversions.ts`, `circlesend.ts`, `circles.ts`,
`circleadmin.ts`, README "Round 37/38"). Spec `docs/circles-plan.md` 4.1, 6.2, 13.2, D10, D16-D19.

Ran: `cd server/worker && npm test` = 584 pass, 0 fail. Own scenarios in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c46/{n,m}.test.ts`
(they import the c45 helpers; run with `node --experimental-strip-types --no-warnings --test-reporter=spec --test <file>`).
The c45 repros (s1 to s4) were re-run unchanged.

## Step 1: round 37 findings re-checked

| Finding | Result (ran) |
|---|---|
| 45a/45b B1 student data lifted by a clean send | Fixed. S1: dirty v2 (held, withheld), Ann's clean v3 lifts the bank hold, Eli `GET .../versions/2` = 404 `version_withheld`; flag still open. v2's history line has `withheld: true, kept: false, message: ""`. |
| B2 byte-cap deadlock | Fixed. S5: v21 and a rollback at the cap are accepted (200), `kept = 20`. Trim route works: keep 1-19 only (0, 20, 1.5, "3", null all 400), owner or the bank's author while editor (non-author editor 403), prunes `previous` older than the newest `keep`, bytes stop counting at once, history lines stay, rollback to a trimmed version 404 `version_not_retained {oldest_kept}`. Report-named versions are skipped; a held bank answers `409 under_moderation`. |
| B3 duplicate `published.qid` | Fixed. S8: both copies carry the origin, licence change to circle-only is 422 `license_incompatible`. M3: with a copy placed BEFORE the original, the copy keeps the qid and the original gets a new one (added 1, changed 1, removed 0; no loss of origin). |
| Origin by text (45b v1) | Passes (round38 test 38-7 in the suite; not re-derived by hand). |
| `oldest_kept` + `pinned_versions` | S2: head 30 with a report on v1: `oldest_kept: 11, pinned_versions: [1]`; v5 is 404 `version_not_retained`; merge base 5 `base_kept: false`. |
| Merge `between` | Oldest first, first 50 after the base: base 1, head 25 -> `[2,3,4,...]`, 24 lines, `between_total` 24. |
| Demoted author licence | S9: viewer-author `PATCH` = 403 `circle_role_required`; demoted author rollback 403; owner rollback 200. |
| Rollback reservation | `rollbackOnce` now reserves by value count (code read, `circlesend.ts:975-976`; test 38-11). |
| History paging, replay, 409 head, commit_id conflicts | Unchanged and still correct (S4: 3 pages, 120 lines, no dups, `before=0` 200). |

## Blockers (new class)

### B1. A held bank at the byte cap cannot be repaired: the author's clean fix and a rollback get 413, trim gets 409, and the 413 text tells them to trim (MEDIUM-HIGH, availability; interaction of the round 38 B2 fix with holds)

`circlesend.ts:98-101` (`PRUNABLE_SQL` counts nothing while any `circle_holds` row exists), `circleversions.ts:184`
(trim refuses a held bank), `circlesend.ts:102-103` (`QUOTA_MESSAGE` says "can trim a bank's older versions").

Repro (`c46/m.test.ts` M1): Ann's bank at 22 versions, kept 20; Eli sends a version with a confirmed student sentence
(`held: true`); the circle cap is set equal to current usage + 100 bytes (the steady state round 38 B2 made reachable).
Result: Ann's clean fix `PUT` = **413 quota_exceeded**, Ann's rollback to v22 = **413**, Ann's trim = **409
under_moderation**. The 413 message says "The owner (or a bank's author) can trim a bank's older versions, or remove a
bank and purge it". The only way out is an admin bank-wide release (hold gone) or removing the bank, i.e. losing it.

Why this is a new class: the hold is the one thing the author's clean send would itself lift (the DELETE is in the same
batch), but the cap check and the prune both run against the state BEFORE that lift, so "held" is treated as "nothing
can be freed". Since round 38 makes student data a property of the version (`withheld_at`), the hold no longer needs to
freeze pruning for privacy holds (a report hold still needs it for evidence). A held bank also keeps growing (documented),
so it reaches this state by itself.

Fix direction: for `source IN ('privacy','privacy_near')` holds, let the prune and `PRUNABLE_SQL` proceed (withheld
versions are already unreadable), or count prunable bytes as if a clean commit's lift had happened; let trim run on a
privacy-only hold; at least do not advertise trim in the 413 when the bank is held.

(No other blocker found in the version-control semantics: see "checked" below.)

## Variants and limits

1. **The editor who pasted the text is locked out, and the owner too.** After Eli's dirty send (Ann's bank): Eli, the
   circle owner Olga (not the author), and every member get 404 on the bank (list empty for them); Olga's rollback and
   trim are 404 (`requireReadable` hides a held bank from non-authors, `circles.ts:1094`). Only Ann (no notice, only `held:
   true` in her own list/responses) or an admin can act, and Eli's legitimate questions in the same version are
   withheld too. With the version-level withhold the whole-bank hide is now redundant for privacy; consider limiting it to
   report/admin holds, or let the owner act. (45a 4, restated; medium.)
2. **Bank-wide admin release leaves a withheld head.** N1: after `circle-release {bank_id}` the bank is listed again,
   history shows v2 `withheld`, and GET head is 404 `version_withheld` for Ann, Eli and Olga alike: bank visible, head
   unreadable, flag open. Editors can send over a head nobody can read (Eli's clean fix 200). By design per the README,
   but the admin needs the second call (`version`), and the UI needs to handle "visible bank, no readable head".
3. **A trim or the first clean send can remove the evidence before the admin looks.** N3: clean v3 lifted the hold, then
   the author's `trim keep=1` pruned the withheld v2; the admin's `circle-bank?version=2&basis=privacy` is now 404
   `version_not_retained` while the `privacy_text_match` flag stays open; `circle-release {version: 2}` on a pruned
   version still answers 200 `released_version: 2` (nothing to release). Flags carry positions and counts, not text.
4. **An admin release is not an allow-list.** N4: v2 released as a false positive and readable; a rollback to v2 (same
   text) re-holds and withholds the NEW version v4 again, and a re-send of the text does the same. Expected, but every
   reuse of a released text needs another release.
5. **A withheld head leaves editors unable to merge.** Eli's "get latest" is 404 `version_withheld`; a send needs
   `base_version` = head, which he learns from history/409. The plan's two-way fallback is about a pruned base, not a
   withheld head; the client needs wording for it.
6. **The send response confirms a block match** (`held: true, withheld: true` to any editor, 100 sends a day). The code
   comment says a refusal is avoided because it "would confirm the block", but the held flag confirms equally. Low
   (needs the text, capped by D10); pre-existing since round 37.
7. **`rollback_of` can point at a version the same commit pruned** (M2: rollback to the oldest kept version 3 at head
   22 -> v23 with `rollback_of: 3`, v3 pruned in that commit). Harmless; history shows a `rollback_of` to a line with
   `kept: false`.
8. **`oldest_kept` counts withheld versions as kept** (`keptInfo` reads status/full_key only). A client that compares
   `base >= oldest_kept` gets 404 `version_withheld`, not "not retained". The line's `kept`/`withheld` are right.
9. Carried over, still true: licence frozen with the author; held bank accumulates bytes (and, per B1, can fill the circle);
   replay returns the first answer's `held`; send/rollback caps are per account across circles; rollback does not restore
   title/subject/course/grade.

## Checked, no defect found

Lost update: two concurrent bases, exactly one winner, 409 carries the true head; trim vs send is safe (trim's cutoff
reads `head_version` in the same statement; the send's prune and trim both only touch `previous`); a send that waited
re-reads the head after the heavy slot; staging rows never get a version number, so no gaps or duplicates (120 versions,
3 pages). Withheld read paths: version GET, head GET/HEAD/304, history message, rollback target, merge base, blob route
(`withheld_at IS NULL` join) all refuse. Withheld flag set in the same batch as the holds (no gap with a block confirmed
mid-send). Rollback to a withheld/pruned/trimmed/released target: 404 `version_withheld`/`version_not_retained`/200 with
re-scan. Trim: audit row only when something was pruned, exact-rights SQL, closed circle 409, removed bank 409.
Rollback charged net of the prune it causes (M2, 38-5). Migration 0110 is one `ADD COLUMN`; the migrate tests list it.

## Nitpicks

- Trim answers 404 (not 403) to a non-author owner of a held bank, and 200 `pruned: 0` when nothing qualifies.
- The README says `kept_versions`-style detail via `pinned_versions` capped at 50; a bank with more pinned versions
  silently lists 50.
- A copy placed before its original (M3) takes the qid; the original becomes "new". No loss, but lineage in history is
  reversed.

## Verdict

One new-class blocker (B1: held bank x byte cap x prune ordering, with an error message that points at a refused
action). Everything round 37's critics reported is fixed and verified; compare-and-set, history, rollback rights, trim
rights and the withheld read paths held up. B1 is a short fix (let privacy holds not freeze pruning); variant 1 is the
larger product question.
