# Critic 47a: circles phase B (rounds 37-39), product / functional lens, fresh eyes

Scope: uncommitted phase B on `shared-pool-phase1` (migration 0110 as edited in place, `circleversions.ts`, `circlesend.ts`,
`circles.ts`, `circleadmin.ts`, `privacynear.ts`, `poolmail.ts`, README "Round 37" to "Round 39"). Spec `docs/circles-plan.md`
4.1, 6.2, 13.2, D1-D19. Never touched port 8799, staging or production; no repo file changed except this report.

Ran:
* `cd server/worker && npm test`: 592 tests, 592 pass, 0 fail.
* The c45 and c46 scripts, unchanged, in `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/{c45,c46}/`.
* New scripts in `.../scratchpad/c47/` (run with `node --experimental-strip-types --no-warnings --test-reporter=spec --test <file>`):
  `fz.test.ts` (model-based fuzz, below), `cap.test.ts` (byte-cap fuzz), `or.test.ts` (OR1-OR4, credit and licence),
  `mig.test.ts` (0110 on populated data), `left.test.ts` (author gone, bank held), `plan.test.ts` (query plan).

## Step 1: earlier blockers stay fixed

| Earlier finding | Re-run result |
|---|---|
| 45 a1 / s1: clean v3 lifts the bank hold, member reads dirty v2 | c45/a1: Eli `versions/2` = 404 both while held and after the lift; flag still open; history `kept:false`. Fixed. |
| 45 s5: byte-cap deadlock at 20 versions | c45/s2 S5: rollback at the cap 200, `kept = 20`. Fixed. |
| 45 s8 / a3: duplicate qid and origin laundering | c45/s3: both copies keep the credit, licence change to circle-only 422 `license_incompatible`. Fixed (but see B1 below for what round 39 did to it). |
| 46a M1: held bank at the cap | c46/m M1: rollback 200, trim 200 (was 413 and 409). Ann's *clean fix* is still 413 in that script only because the fix is larger than the one version the commit prunes; README says so, and the trim the message names now works. Fixed as claimed. |
| 46b N1: withheld version's title in the listing | c46/w2: after the rollback, listing, head read and activity all show "Unit 2". Fixed. |
| 46b N2: orphan keys after a lost commit | c46/r1: both keys now in `r2_orphans`. Fixed. |
| 46b V1 graft / imposed origin | c46/o1: editor's changed copy gets no credit; editor-declared origin on Ann's kept question is ignored. Fixed. |
| c45 s2 S6 fails | Script calls a `PATCH .../members/:id` route that does not exist (known from 46b). Not a regression. |

## Step 2: how I attacked it

1. **Model-based fuzz (`fz.test.ts`).** A tiny model of the history (stems, qids, title, withheld, kept, open flag per version)
   drives 26 seeds x 70-90 random operations by three accounts (author, editor, owner): sends with title changes, add, edit, delete,
   reorder, stale bases (409), dirty sends of confirmed student text, merge-initiate and merge-token sends, rollbacks to any
   version, trims with random `keep`, per-version admin releases. After EVERY operation it checks: the HTTP status class of the
   operation against the model (including "non-author onto a held bank = 404", "rollback to pruned = `version_not_retained`",
   "to withheld = `version_withheld`"), qids kept or fresh, `withheld` in the answer, every history line's `kept` / `withheld`,
   `base_kept`, dense version numbers 1..head, exactly one `current` row, DB `pruned` equals the model's prune (window of 20,
   trim, open-flag exception), `oldest_kept` against the readable-run rule, the head body and random old version bodies stem by
   stem, and the bank's `title` against "listing of the newest version that is not withheld". Result: 26 seeds (heads 22 to 65),
   zero divergences, after fixing my own model twice (it must treat a re-sent released text as dirty again, which README
   documents). So send, 409, merge, rollback, trim, release, pruning at the 20-version horizon, listing fields and
   `oldest_kept` agree with their documentation under long mixed histories.
2. **Byte-cap fuzz (`cap.test.ts`).** 8 seeds x 60 operations with the circle cap set to 4 to 10 versions, dirty sends and rollbacks
   mixed in. The cap is never exceeded; every 413 (up to 26 per seed) is followed by an author trim (keep 1) and a send that then
   succeeds: 0 stuck states.
3. **Migration 0110 on populated data (`mig.test.ts`).** Legacy banks (listing NULL, backfill only on `current`) work, except the
   known soft spot in V4.
4. **Credit and licence (`or.test.ts`).** See B1.
5. Code reading of every commit/prune/trim/listing/release statement, the export and purge code, and the README's round-39 claims
   against it.

## Blockers (new class)

### B1. Whether an honest adaptation of a public-origin question keeps its credit (and the licence bound) depends on whether the copy sits above or below the original (MEDIUM, licence and credit integrity; round 39 made it so)

Where: `circlesend.ts:473-479` (first occurrence of a repeated qid keeps the qid; later ones are new), `:555-563` (`carry` by qid
only for the kept occurrence), `:565-569` and `:610-616` (a later occurrence is credited only when its key-free match hash equals the
head's, i.e. an EXACT copy). README round 39 "46b V1" states the rule.

Honest scenario (the app's Duplicate keeps `published`, then the teacher edits the copy to make "version B" of a credited question).
Repro `c47/or.test.ts` OR1 and OR4, same edit, only the position differs:

* Ann's bank holds Dee's CC BY-SA question Q (declared origin), bank licence CC BY-SA.
* Eli sends Q, then the adapted copy QB (same qid, new text) right after it: `origins = [[1,"Dee"]]`; QB has **no credit**.
  Ann then removes the original and keeps QB: `origins = []`, and `PATCH license: circle-only` = **200**. An adaptation of a CC BY-SA
  item is now an all-rights-reserved circle question with no attribution and no bound.
* Same send with QB placed ABOVE Q: QB takes the qid and the credit, Q gets the credit by exact hash: `origins = [[1,"Dee"],[2,"Dee"]]`;
  after Ann drops Q, `PATCH circle-only` = **422 license_incompatible** (correct).

Why it matters: the plan (D13, section 7) and the task statement want an honest adaptation to keep its credit; round 38 got there by
crediting every copy, round 39 closed the editor "graft" hole by crediting only exact copies, which removes the credit from every
legitimate adaptation, and the outcome flips with list order. The client phase B cannot rescue it: the served body carries no
`origin` (it is deleted at write, `circlesend.ts:578-585`, and the app's `origins` note is not fed back), so an editor's duplicate has
nothing to re-declare. (Dropping and re-adding a credited question is a documented limit; this is the in-place adaptation.)

Fix direction: the graft threat is a NON-author putting a credit on text that is not the credited item's. Carry the recorded origin to
every occurrence of a repeated qid when the sender is the bank's author (she can only hurt herself), and for a non-author carry it too
but mark it `carried`, so the author can see it and drop it; or choose the "first occurrence" by match-hash equality with the head's
text instead of list position, and still credit the changed one. Either removes the order dependence.

(No other new-class blocker found in the version-control semantics; see "checked".)

## Variants and limits

V1. **Author gone, bank privacy-held: nobody in the circle can repair it, and the owner cannot even purge it (MEDIUM-LOW; availability,
extends 45a 4 / 46a v1).** `c47/left.test.ts`: Eli's dirty send held Ann's bank; Ann then leaves the circle. The owner's clean send =
404, rollback = 404, trim = 404 (`requireReadable`, `circles.ts:1094`: held and not the author). Owner `DELETE` = 200 (removed, 14
days) but `POST .../purge` = 409 `under_moderation` (`circles.ts` `OPEN_REPORT_BANK` counts any hold), so the bank keeps counting
against the circle's 100 banks and 200 MB until an administrator acts. The same dead end exists when the author is suspended or
erased with the bank kept. Round 38 made the bank-wide hide redundant for student data (the version is withheld); restricting the
hide to report and admin holds, or letting the owner act on a privacy-held bank, removes it.

V2. **The admin mail tells the admin to release the wrong way (`poolmail.ts:38`).** It says "release it with /admin/pool/circle-release
if it is a false positive". A bank-wide release (as the mail describes, no `version`) lifts the hold but leaves the version withheld
and the flag open (README round 38 B1); the bank is then listed with an unreadable head for everybody (46a v2). The mail must say
`{bank_id, version}` and name the version (the flag has it).

V3. **Pre-0110 history rolls back without its title (`mig.test.ts`).** Legacy `previous` versions have `listing = NULL`; a rollback to
them records the bank's CURRENT title (`circlesend.ts:1092`), so "rollback to v1 lists v1's title" is true only for versions sent after
migration. Test: v1 "Unit A", v2 "Unit B", v3 "Unit C": rollback to v1 and to v2 both leave "Unit C". Cosmetic; say so in the README.

V4. **A title is last-writer-wins on every send and nobody is told.** `circlesend.ts:642,782`: each version carries the sender's meta title
(or his local bank name) and overwrites the bank's. An editor merging from a copy made before Ann's rename silently renames the bank
back. Recoverable by rollback (which now restores titles), but there is no three-way rule for the listing fields.

V5. **Any bank author (while editor) can trim away every co-editor's merge base and rollback target with one call** (`keep: 1`,
`circleversions.ts` trim). D17 gave rollback and delete to the owner and the author, trim is the round-38 addition; only an activity
line (`versions_trimmed`) records it. Editors then get `base_kept: false` and the two-way fallback. Worth saying in the app copy.

V6. **Retroactivity and rollback.** A block confirmed after a version was sent leaves that version readable, but a rollback to it makes a
NEW withheld version and a bank hold (documented as 46b V3). An author who "goes back to the good one" after such a block makes
things worse; every older version carrying the text does the same.

V7. **Documented, re-verified true:** a released text is not an allow-list (a rollback to a released version is withheld again; my fuzz
needed this); editors cannot merge with a withheld head except by reading the history (they send with `base_version` = head, and the
fuzz does exactly that, it works); `held`/`withheld` in the sender's answer confirm a match to that sender.

V8. **Export (plan 6.5 as written) lists versions sent without bodies, so the content a teacher wrote in circle banks is not in
`GET /me/export`.** It matches the plan; flag it for counsel (D9) because phase B makes the circle the only place that content lives.

## Checked, no defect found (all by running, not only reading)

* Compare-and-set and 409: stale base 409 with the true head, refunded; a non-author onto a held bank 404 before 409 (consistent with
  `requireReadable`); `commit_id` replay, conflict and cross-user reuse unchanged from 46.
* Merge-initiate: `base_kept` equals readability in every fuzz state; token binds account, bank, base and head; a send with the token
  records `merged_from`.
* 20-version horizon: pruning by the commit exactly equals the model (including skip of versions a flag pins, trim exception);
  `oldest_kept` and `pinned_versions` agree with the readable-run rule in 26 histories.
* Listing fields (title, subject, course, grade) equal the newest non-withheld version's after every send, rollback, trim and per-version
  release in the fuzz; `LISTING_REFRESH_SQL` uses the (bank_id, version) index (`plan.test.ts`, four index searches, no scan).
* Quotas: no state in 8 cap-fuzz seeds is stuck; the cap is never exceeded; trim frees at once.
* Role and ownership changes, closed circle, purge, erasure: all guarded in the compare-and-set (`ACTIVE_SQL`, `OPEN_SQL`,
  `ALIVE_SQL`); purge deletes versions, blob rows, qkeys and holds and records orphans; export has no bodies (as planned).
* Rollback rights (owner or author-while-editor), trim rights, bad `keep` values: 46's results hold.

## README versus code

* README line 323 ("the bank keeps its listing fields") and line 264 ("A rollback restores ... not the bank's title") are superseded by
  round 39 (a rollback restores its target's listing). Historic sections, but stale.
* Round 39's claim "an origin a NON-author declares on a question the head already holds is ignored" matches `circlesend.ts:549`; the
  claim "an adaptation keeps its attribution" holds only for in-place edits and for the copy that sits first (B1).
* Everything else I tested in the round 38/39 text (trim vs report/flag, `pinned_versions`, PRUNABLE parity, V4/V6/V7/V9 fixes) matches
  the code.

## Nitpicks

* `circle-release {version}` on a pruned version answers 200 `released_version` (nothing readable to release); unchanged from 46a.
* The 413 text still says trim can help even when the only freeable bank is one the caller may not trim (an editor on someone else's
  bank); the owner is named, which is enough.
* A retry of a pre-purge `commit_id` would re-create the bank (version rows are deleted at purge); needs a retry after 14 days.

## Verdict

One new-class blocker (B1, medium): the credit and licence bound of an adapted public question depend on list order, and the
common "duplicate, edit, delete the original" flow loses them. Version-control mechanics themselves (compare-and-set, merge bases,
rollback, trim, pruning, listing, caps, withheld versions) held up under a model-checked 26-seed fuzz and an 8-seed cap fuzz with no
divergence. V1 (author gone, bank held) is the largest remaining limit and is an availability, not a data, problem.
