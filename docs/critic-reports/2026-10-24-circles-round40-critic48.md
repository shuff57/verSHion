# Critic 48: circles phase B, Round 40 (privacy / security lens)

Fresh reviewer. Scope: README "Round 40" only (owner path for a bank held only for student data, origin carrying,
admin mail with a version link, listing fallback for pre-0110 versions) and its interactions. No repo file was
changed except this report. `cd server/worker && npm test`: 598 pass, 0 fail. Attack scripts (all run with
`node --experimental-strip-types --no-warnings --test <file>`), in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c48/`:
`lib.ts` (helpers), `a1.test.ts` (leak sweep, oracle), `a2.test.ts` (admin and report holds, transfer, purge),
`a3.test.ts` (image route, origins), `a4.test.ts` (pre-0110 listing), `a5.test.ts` (dirty first version, equality oracle).

## Verdict

**Blockers (new class): none.** I could not get student-data text, a title, a message, an image or a withheld
version out through the new owner path. Two limits deserve a README line (V1 purge destroys the admin's evidence
at once; V2 the owner learns who sent the withheld version and that it was withheld). The origin rules open nothing
that a sender could not already do.

## 1. Owner path for a bank held only for student data

Setup used everywhere: owner Olga, author Ann (editor), dirty sender Eli (editor), Eve (editor), Vic (viewer). Ann sends
a clean v1, Eli sends v2 with a confirmed student-data question (`held: true, withheld: true`).

**Leak sweep (a1).** For Olga (owner), Ann (author), Eve and Vic I called: circle list, circle page, activity, head,
`versions`, `versions/1`, `versions/2`, `versions/3`, `merge {base_version:1}`, `/circles/heads`, `/me/export`, and
searched every body for the student text, name and score. Zero hits, in both variants: clean title and message, and a
dirty one (title "Jenny Smith period 3 scores", bank name "Jenny Smith period 3", message "Jenny Smith scored 42").
Results for the owner: list shows the bank with the clean title "Unit 2" and `held: true`; head `404 version_withheld`;
`versions/1` 200; `versions/2` and `/3` 404; history shows `[2, message "", withheld true, kept false]`; merge 200
(lines redacted by `lineOf`); heads `status: current`. Eve and Vic (non-owner) still get 404 on every bank route.

**Dirty first version (a5).** Eli creates the bank dirty from v1, with a student-name title/name/message. Owner list
shows `["", "", held true, question_count 1]` (blank listing), Vic's list is empty, the owner's `versions` and
history are clean of the text, `versions/1` 404, `rollback to_version 1` is 400 (nothing to go back to, as it should
be), and a clean replacement send works (428 only because it drops over half; with confirm it commits).

**Images (a3).** An image used only by the withheld version: `GET .../blobs/<sha>` is 404 for owner, author and the
sender, before and after the owner's rollback. The blob SQL needs a non-withheld version and a non-blocking hold.

**Admin hold, report hold (a2).** With an admin hold (`/admin/pool/circle-hold`) or a student-data report hold
(source `''`) beside the privacy hold: the owner's list is empty, head/versions/merge/rollback/trim are 404, `heads`
is `under_review`, activity has no title, and purge-now is `409 under_moderation` (also in the round 40 test).
`BLOCKING_HOLD_SQL` is consistently the NOT-IN-('privacy','privacy_near') test in every owner exception I found
(`circles.ts` 410, 417, 1000, 1055, 1110, 1274; `circlesend.ts` 791, 953; `circleversions.ts` 176, 187).

**Takedown.** Circle banks have no takedown state (only items); `admin` removal deletes the holds
(`circleadmin.ts:270`) so the owner sees the clean title of an admin-removed bank, which is the bank's ordinary state.
No hold-hidden title is shown to the owner through the removed-bank listing (`circles.ts:417` masks only when a
blocking hold exists).

**Demoted / removed owner; transfer to a hostile editor (a2).** Olga offers ownership to Eve (202), Eve accepts.
Olga is now an editor: every route on the held bank is 404, list empty, `heads` `under_review`. Eve, the new owner,
sees exactly what the old owner saw (clean title, `versions/1`, history with the withheld line, no text anywhere;
sweep clean). **Is it acceptable that a non-owner editor's dirty send is visible as metadata to a malicious new
owner? Yes.** What the new owner learns: the bank exists, the sender's display name and the time, the counts
(question_count, added/changed/removed), the licence, and that the version is withheld. The text, titles, messages,
images and the withheld version are never returned to anyone but the admin. What the new owner can do to it: roll back
to a clean version, replace, trim (a withheld version with an open flag survives a trim), remove and purge. The owner
could already remove and purge any unheld bank, so no new power over the author's work.

**Owner is the dirty sender.** Reads back clean versions and, as for anyone, never the withheld one. Fine.

**Purge on hold: is the text still there for the admin? No (V1).** Reproduced (a2): Eve removes the bank and
purges it at once. Before: admin `GET /admin/pool/circle-bank?id&version=2&basis=privacy` returns 200 with the
student text. After: `404 version_not_retained {oldest_kept: null}`. The flag stays open with positions and counts
only (`{"count":1,"first":[1]}`), the mail's instruction `circle-release {bank_id, version: 2}` now answers
`409 not_withheld` (the version row is gone), and only the bank-wide release (not in the mail) closes the flag. This is
the stated intent of Round 40 ("the purge deletes the withheld text"), and for exact matches it is privacy-favourable.

## 2. Origin carrying (a3)

Ran the editor cases on a bank whose author credited Dee's CC-BY-SA question:

* **Strip.** Eli drops the credited question and adds an adapted fresh one (`confirm_large_change`): origins `[]`,
  Ann's relicense to `circle-only` is 200. An editor can launder a credited question by drop-and-adapt. README says so
  ("an editor's adaptation of a credited question is credited only if the author re-declares it"). In-place edits keep
  the credit (the qid is kept), so an editor cannot strip without removing the original.
* **Impose / false credit.** Eli adds a new question of unrelated text declaring Dee's item as its origin: recorded
  (`author_name: "Dee"`, licence CC-BY-SA), Ann's relicense is now 422 until she removes it. On a copy of Ann's own
  uncredited question (reused qid, the copy is a new qid) the declaration is also recorded; on the kept qid it is
  ignored. The server never checks that the text relates to the item. This is the round 39 rule ("on a new one it is
  recorded"), unchanged by Round 40.
* **Duplicate.** Exact copies of a credited question carry the credit (match-hash), adapted copies by a non-author do
  not (round 39 rule, now order independent). Only the bank's author carries it onto adapted copies.
* Round 40's change widened nothing for an editor or the owner (`authorSends` is `bank.author_id === me`); a demoted
  or removed author cannot send at all.

No new impose/strip/duplicate path beyond the declaration-based credit system that was already there.

## 3. Admin mail

`poolmail.ts:31-40` contains ids only: bank id, circle id, version number, the audited admin URL and the release
body. No title, message, text, name or count. The ids are server-generated hex. The `version` is always set on the
send path (`circlesend.ts:886`). Nitpick: a queued legacy notice without `version` would render `version=` (the
admin route then answers 400) and `"version": 0`.

## 4. Listing fallback for pre-0110 versions (a4)

Ran v1 and v2 with NULL listing, v3 dirty with a student name in title and bank name (withheld), owner rolls back to
v1: the bank row becomes `Unit A / Math` (the v1 body's own name), or `Unit B / Math` when the v1 body name is blank
(the bank row's title at that moment, which `LISTING_REFRESH_SQL` had already taken from the newest readable
version). Recorded listing of v4 contains no withheld text. Owner and viewer pages clean. The fallback title comes
from a readable target body and the bank row's own columns, never from a withheld version. No leak.

## Blockers (new class)

None.

## Variants and limits

**V1. Purge-now by the owner destroys the admin's evidence immediately, including for near-rewording flags.**
`circles.ts` `OPEN_REPORT_BANK` (`BLOCKING_HOLD_SQL`) no longer counts `privacy`/`privacy_near`; repro in a2 above. A
colluding owner and sender, or a new owner by succession, can remove and purge within seconds of the mail. For
`privacy_near` (a rewording only flagged, explicitly "review it now" in the public pool) the admin can then no longer
judge a false positive; the version release says `not_withheld` and the flag stays open until a bank-wide release.
Severity: low to medium (availability of evidence, not exposure). Suggest: refuse purge-now (or delay it, as removal
already delays the daily purge by 14 days) while a `privacy_text_match` flag is open and younger than some days, or keep
the flagged version's text in an admin-only quarantine until the flag is resolved; and let the version release resolve
the flag of a purged bank.

**V2. The owner now learns who sent a withheld version, when, and that it matched student data.** `heads` (`by`, `at`),
`versions` (`withheld: true`, name), the list entry (`held: true`). Members already see `withheld` on a bank whose hold
lifted (round 38), so this is parity, but a bank that was hidden from the owner before is now labelled. No data.

**V3. Equality oracle against a withheld head (a5).** The owner can send a guess for a question the sender edited in
place and read `changed: 0` (equal) versus `changed: 1` in the answer, also `removed` counts. It needs the whole question
(stem, key, points, explanation) exactly, and it is one-shot per withheld head: the probe is a committed version, so the
next probe is compared with the guess, not the withheld text. The author had the same oracle before. Negligible.

**V4. Migration 0110 does not blank a withheld current version's bank row.** `UPDATE circle_versions SET listing = ...
WHERE status = 'current'` copies the bank row even when the version is withheld, and the bank row is only refreshed at
the next commit. On a database that already holds a withheld head (a staging that ran an earlier edit of 0110) the
owner now sees that bank's title in the list until the next send. A fresh database cannot have such a row; check
staging for `withheld_at IS NOT NULL` current versions and refresh their banks once.

**V5. False credit is unverified (carried, not new).** An editor can attach any visible public item's name and licence
to unrelated text on a new question (a3). Consider checking the declared origin against the item's match hashes or let
the author remove origins she did not declare.

## Nitpicks

* The mail points the admin at a version release that answers `409 not_withheld` once the bank is purged.
* `circlecheck.ts:168` treats any hold as hidden for naming (conservative, no leak).
* A dirty-only first version gives the owner a bank with blank title/subject and `question_count` of the withheld
  version (count only).
