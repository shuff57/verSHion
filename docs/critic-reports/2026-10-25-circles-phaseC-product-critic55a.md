# Circles phase C (rounds 42-47), product/functional critic 55a

Scope: uncommitted phase C backend on `shared-pool-phase1` after round 47. Lens: honest teachers, then the coming client.

`cd server/worker && npm test`: 652 tests, 652 pass, 0 fail. My scenarios (not in the repo) are in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c55/`
(`a1`, `l1`, `e1`, `u1`, `i1`, `i3`, `r1`, `s1`, `p1`, `.test.ts`). Run one with
`cd server/worker && node --experimental-strip-types --no-warnings <path>` (they print with `console.log`). The 54 scripts
`t2`, `t3`, `t5`, `t6`, `rel`, `pin` were rerun first: the 54a/54b blockers hold fixed in the form those scripts test them.
No repo file was changed except this report. Not run: staging, production, port 8799.

## Blockers (new class)

None found. Every item below has a way out for an honest user, or is wording/shape. The nearest to a blocker is V1.

## Round 54 fixes, checked

- 54a B1 (sentinel-only keyless copy): `t3` now `422 keyless_question {positions:[0,1]}` with hide on and off. Re-keying every
  question (sentinel gone, real keys) sends `200` (`a1` A3). A stripped-origin True/False `false` with a valid origin and no
  `rekeyed` is `422 keyless_question` (`a1` A5, positions [1]); with `rekeyed: true` it is `201` (A5b). My first A5 returned 201
  only because my origin ids were too short (a malformed origin is dropped and re-checked in the stored form, as the code
  says): not a bug, but a client that sends a bad `origin` on a stripped copy is not caught by the placeholder rule.
- 54a B2 (refusals cost): `t5` 12 banks x (422 + 200) all succeed; the usage row shows one charge per success. Invites: 14
  attempts, 10 mails, `circle_email_invites` 10 / `circle_invites` 10 (the 4 refusals gave back).
- 54a variant 1 (hide on withholds the head): `t2` PATCH on `200 held, withheld`; PATCH off `200 {restored:[1]}`; author,
  editor and owner read again; turning it on again withholds again (the block is still active); `/admin/dmca/unblock
  {release_circle_versions}` resolves both flags. Works as claimed.
- 54b N1 pin bound: `pin` 60 blocked sends: 5 open flags, the rest unflagged and pruned by the window; trim `pruned 19,
  evidence_kept 5`; delete then purge-now keeps only the evidence.
- 54b N2: `rel` unblock answers `circle_versions_released: 0` when the text is now confirmed student data; owner still gets 404.

## Variants and limits

### V1. A stripped copy of a question that never had a key (essay-style `Short`) can never be re-keyed, so a promoted viewer's own copy of such a bank is never sendable as it stands
`server/worker/src/pool/circlesend.ts:615-617` (the guard) with share-core `validate.rs:1102-1106` and the app's
`src/model.rs:124-141` (`rekeyed_with`: a `Short` is re-keyed only by a non-empty sample).

Repro `a1` A2/A3: Ann's bank has an essay (`Short` with sample `""`), a True/False, an Mc, a Short with a sample, no
explanations; hide on. A full-key send of it is fine (A4, 201, no false positive). Vic reads the stripped head (every question
`keyless: true`, the essay too), is promoted. The app holds the sentinel on every question; Vic re-keys all but the essay
(there is nothing to key): `422 keyless_question positions:[2]`, message "Set the correct answers first, or get a full copy
from an editor." Only way found to send her copy: type a made-up sample for the essay (A3 `200`; the owner's head then holds
`"see rubric"` as the essay's sample, a key the author never wrote). The honest path that works: as an editor she reads the
full head (`p1`: promoted viewer gets the Mc with `correct: true`, no `keys_hidden`) and edits that copy. The worker side is
right; the client must (a) re-import the full head when a viewer is promoted, (b) not let a keyless essay be the reason a
send is blocked. The message is wrong for an essay ("set the correct answers" has no meaning). Severity: low-medium, rare
(a promoted viewer who edits her stripped copy).

Related, not a bug: a re-keyed stripped copy sent over the head replaces the author's explanations and figures with nothing
(the stripped form has none); the diff says "changed", nothing says "explanations dropped". Client wording.

### V2. A send or rollback that loses its commit only because `hide_key` flipped answers a `409 conflict` whose `head_version` equals the sender's base
`server/worker/src/pool/circlesend.ts:1217-1222` (`conflict(c, bid)` is the fallback when the head did not move) and `:1000`
(`AND (?20 IS NULL OR hide_key = ?20)`). Repro `r1` (R2 hook flips `hide_key` during the put): `409 {"error":"conflict",
"head_version":1,"head_at":...,"head_by":"Ann","message":"The circle bank changed since you got it. Get the latest changes
and merge them, then send again."}` with base 1. The client (`src/share/pool.rs:387`) reads that as `CircleConflict`, "get
latest and merge": there is nothing newer, so the merge finds nothing; a plain resend works (R1 retry `200`, and if hiding
is now on the send first needs `labels`). Rare race, same code for rollback (47-10). Suggest a distinct message or
`error: "bank_settings_changed"`. The PATCH's own lost race answers `409 conflict` with only a message and no `head_version`
(`circles.ts:1694`), which the client maps to the public pool's `Conflict` (no `current_version`). Both are readable in the
client only if it treats a `conflict` without `head_version` as "try again".

### V3. After the 5-flags-a-day cap the newest (head) version is withheld with no flag; an unblock lifts everything else and leaves it, plus the bank hold
`server/worker/src/pool/circlesend.ts:935,1132`, `circleadmin.ts` unblock/release.
Repro `u1`: takedown; one editor sends 8 matching versions in a day. Flags: 5 `blocklist` + 1 `cap` (no bank or version in
its detail, only `metric: circle_send_flags`). Versions 7, 8, 9 are withheld without a flag. `/admin/dmca/unblock
{release_circle_versions:true}` answers `circle_flags_lifted 5, circle_versions_released 5`; afterwards the author still
gets `404 version_withheld` on her head (v9), the history shows 7-9 withheld, the bank's `blocklist` hold is still in
`circle_holds`, and the `cap` flag stays open. The author is not told why or that the takedown is lifted; her clean resend
works (`e1` "clean fix" `200 held:false`), so she is not stuck, but only if she tries. The administrator has to know to
run `circle-release {bank_id, version: 9}`; nothing in the `cap` flag names the bank. Suggest: unblock also releases a
flagless withheld version when its only reason was the lifted block, and the `cap` flag carries `circle_id`/`bank_id`.

### V4. What an author sees of evidence being kept or pruned is thin and in one place wrong
`e1` (8 matching sends, then a clean fix, trim, delete, purge-now):
- Each send answers `200 held:true withheld:true`; nothing says a flag was or was not raised, nor that after the 5th none
  will be. Fine for privacy; the author knows it is withheld and under review.
- The history lists the pruned evidence versions as `kept:false, withheld:true`, indistinguishable from "readable but
  trimmed". `GET versions/8` (pruned evidence) answers `404 version_not_retained ... "a bank keeps its last 20 versions in
  full"`, which is false for it (it is version 8 of 10; it was pruned by the evidence bound). The administrator's open of the
  same version carries `evidence_pruned: true`; the member's does not.
- Trim answers `pinned_versions: []` while `evidence_kept: 5`; the field the client reads (`circle_versions_api.rs:250`
  `Trimmed`) has no `evidence_kept`, so the owner is still not told in the app. `oldest_kept: null` is possible (`pin`
  trim, head withheld): the client handles `Option`.
Not blocking. The honest author always has a send of clean content that works.

### V5. Purge-now of a deleted bank with an open flag commits but answers `409 pending_review`
`server/worker/src/pool/circles.ts:1844`. Repro `e1`: delete `200`, purge-now `409 {"error":"pending_review","message":"...
Everything else in it has been deleted now; the bank is purged once that review is done.","evidence_kept":5}`, then
undelete `409 bank_removed "This bank can no longer be restored."`, versions pruned. The message is honest, the status
code says "refused". A client that treats 4xx as "nothing happened" keeps the "Undo" offer on screen and the owner
learns it is gone only on the next click. Others' data: it deletes the clean versions an editor sent after the false
positive too (as any purge-now does; the owner asked for a purge). Suggest a marker field (`purge_claimed: true`) and say
so in the README table for the client. The README documents "cannot be undone"; it does not tell the client to read a 409 as done.

### V6. Invites (`i1`, `i2`, `i3`)
- 15 colleagues at one school domain: 10 go (`202`, `emailed:true`, `address_label`), 11-15 are `429 hourly_limit
  {metric:"circle_email_domain_hourly", message:"There have been too many invitations to this mail provider in the last
  hour. Try again in an hour."}`; one hour later all five go (`202`). So it is two hours, as expected, and no daily or
  open-code cap interferes. A second inviter on the same domain gets her own 10 (the domain's 30 is not exhausted by one).
  Wording: for a school domain "this mail provider" is odd, and the message does not say it is the sender's own share
  (10 an hour per domain) that ran out; the same text is used for the inviter's share and the domain's. Refusals give
  their slots back (`i3`).
- Masked labels: `l***@pvhs.example`, `d***`, ... and for the three `j***` names `j***@pvhs.example`, `... #2`, `... #3`.
  The suffix keeps them apart but the owner still cannot tell jsmith from jjones from jlee; only the order of sending
  separates them. The client should show the creation time next to the label (the list has `created_at`).
- Resend: inviting the same address while the first invite is open makes a second code and a second mail (`i1`: label `#4`,
  both active, 2 of the 3 mails to that mailbox today). There is no "resend" and no "this address already has an open
  invite"; the owner must revoke the old code by label. Revoke `200` works and frees the open-email slot. Not a defect, a
  gap for the client.
- Mail text is understandable (no names, tells what an approved account and a redeemed pool code are, the expiry to the
  minute). `wrangler.toml:41` still has `APP_URL = "https://app.example.org"` and the client has no `/circles/join` route,
  so any mail sent before both exist carries a dead link (54a limit 6 stands, not mentioned in the README for 47).

### V7. Hide-key lifecycle (`l1`, `t6`)
- On: first PATCH without decisions `422 labels_need_decision` with `label_findings` (before/after text), not charged;
  second with `labels` `200`, charged once; a repeat of the same decisions `200 {unchanged:true}`, not charged.
- A rollback to a pre-flag version leaves viewers `409 keys_hidden_unavailable` (not charged, refunded) until the author or
  the owner PATCHes with decisions once (`t6`: `200`, the viewer reads); Eli (editor, not author) is `403`. The rollback
  itself is owner/author-only, so someone who can recover is always present. One step as claimed.
- Off: viewers read the full text at once (they were served stripped before); `restored` only lists versions the PATCH
  itself withheld.
- `403 circle_role_required` says `needed: "author"` to a viewer on the bank PATCH and `needed: "owner"` to an editor on
  the same route family (trim, rollback): the owner can do all of them; the field is not a stable contract for the client.

## API shape (new or changed, each seen in a live call, `s1`, `a1`, `i1`, `u1`, `e1`)

| Where | Field / error | Seen |
|---|---|---|
| `POST /circles/:id/banks` | request `hide_key: bool` (only on POST; on PUT it is `400 invalid_request {field:"hide_key"}`, no message); answer `keys_hidden: true` | yes |
| `POST/PUT` send, rollback answers | `held`, `withheld` (booleans) | yes |
| `422 keyless_question` | `{positions:[int], message}` (positions capped at 100) | yes |
| `PATCH /circles/:id/banks/:b` | `{hide_key, labels?, order_acks?, ack_formatting?, image_ok?}`; answers `{ok, hide_key, held, withheld, labels_removed, images_kept, images_removed, figures_removed, order_reasons, restored?}` or `{ok, hide_key, unchanged:true}`; `422 labels_need_decision {errors, label_findings}`; `422 order_ack_required {needs_order_ack}`; `429 daily_limit` with message; `409 conflict {message}` (no `head_version`) | yes |
| Bank head `bank` | `hide_key` (all roles), `keys_hidden: true` only for viewers; `409 keys_hidden_unavailable` | yes |
| Circle page bank row | `hide_key`; `held` (author) | yes |
| `404 version_withheld {message}`, `404 version_not_retained {oldest_kept, pinned_versions, message}` | member side; admin side adds `evidence_pruned`, flags' details | partly (the admin pruned open in `s1` answered 200 for a kept evidence version; the pruned-version 404 shape seen only on the member side) |
| `POST .../trim` | `{ok, pruned, evidence_kept, oldest_kept, pinned_versions}` | yes |
| `POST .../purge` | `409 pending_review {message, evidence_kept}` and it commits | yes |
| `POST /circles/:id/invites/email` | `202 {ok, invite_id, kind:"email", role, max_uses, expires_at, emailed, address_label}`; `422 invite_not_sent {emailed:false, reason, message}`; `429 hourly_limit {metric, message}` (`circle_email_domain_hourly`, `circle_email_invites_hourly`); `422 circles_limit {metric:"circle_open_email_invites"}` | yes (not `invite_not_sent`, which 54b ran) |
| `GET .../invites` | `kind`, `address_label`, `state`, `created_by_name` | yes |
| Flags (admin) | `hashes` (<=20), `evidence_pruned`, `evidence_pruned_at`, `via:"hide_key"`, `cap` flag `metric:"circle_send_flags"` | yes (`hashes`, `cap`) |

The Rust client today (`src/share/*`) has no hide-key, `keys_hidden`, `held`, `address_label`, `evidence_kept` or
`keyless_question` handling (grep): all of the above is for the next client work.

## README "Round 47" vs code

Checked and true: the keyless guard via `validate_for_publish` (including the retry on the stored form); refusals refund;
12 banks count twelve; evidence 5 a bank and 20 a circle; 5 flags a day then flagless withhold plus one `cap` flag; trim
answers `evidence_kept`; purge-now claims and prunes; limits 40 a day / 30 an hour / 40 open email invites; the domain
message; `#2`, `#3` labels; hide off restores `restored: [...]`; rollback compares `hide_key`. Not stated, found: V1 (essay
dead end), V3 (flagless head stays withheld after an unblock; the README says only "the matches still count toward the
held-publish budget"), V5 (409 commits; the README says "answered ... `409 pending_review`, and now also claims the purge"
and so is accurate but not a rule for the client), V6 (APP_URL).

## Migration 0114

`CREATE INDEX circle_versions_withheld ON circle_versions (circle_id, committed_at) WHERE withheld_at IS NOT NULL;`
One statement, no `IF NOT EXISTS` (like the other recent files: 0108, 0110-0113 also have none), so a second run fails,
which is normal here and atomic in D1. On populated data it only builds a small partial index; no row changes; the columns
exist from earlier migrations (the whole suite applies 0001-0114 in order). `src/` never names it (`INDEXED BY` is not used),
the planner chooses it. Numbering consecutive.

## Nitpicks

- `hourly_limit` for a domain and for the inviter's own 10 share the "mail provider" text (V6).
- `invalid_request {field}` has no message anywhere in this phase.
- `PATCH` hide_key answers `403 needed:"owner"` to a non-author editor; the author could do it (V7).
- History `kept:false` on a withheld head.

## Verdict

No new-class blocker. The 54a/54b blockers hold fixed. Real but bounded items for the client or a round 48: V1 (essay
dead end; message), V2 (409 with unchanged head), V3 (flagless withheld head after an unblock, cap flag without a bank), V5
(purge-now commits behind a 409), V4 (history/404 wording for pruned evidence).
