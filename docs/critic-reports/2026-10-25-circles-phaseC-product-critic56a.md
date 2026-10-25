# Circles phase C (rounds 42-48), product/functional critic 56a

Scope: uncommitted phase C backend on `shared-pool-phase1` after round 48. Lens: honest teachers, then the Rust client that will meet it.

`cd server/worker && npm test`: 662 tests, 662 pass, 0 fail (run twice). My scenarios are not in the repo. They are in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c56/`
(`lib.ts` is the shared set-up copied from round48.test.ts; `s1`-`s11.test.ts`, `mig.mjs`). Run one with
`node --experimental-strip-types --no-warnings <path>`; they print with `console.log`. No repo file was changed except this
report. Not run: port 8799, staging, production, any commit.

## Blockers (new class)

None found. Every item below has a way out for an honest user with the tools that exist today, or is wording or shape.
The nearest to a blocker is V1 (a circle whose space is held by flagged heads, with no administrator acting).

## Round 48 fixes, checked live

- Attempts counter (55b B1): twelve banks, each through the three steps the app needs (`labels_need_decision`, then
  `order_ack_required`, then `200`), is 36 calls; `circle_hide_attempts` 37 with my one preview, `circle_hide_changes` 12
  (`s1`). Same-setting repeats, a malformed decision (`labels[].choice: "maybe"` is `400 invalid_request {field:"labels"}`)
  and a decision for a question the head does not have (`400 {field:"labels", message:"This bank's current version has 1
  questions."}`) cost nothing (`s2`). The 61st is `429 daily_limit {metric:"circle_hide_attempts", message}` with
  `Retry-After` (`s2`, limit lowered to 2 for the run).
- Evidence (55b B2): a flagged head survives remove plus purge-now, a captured flag frees it, trim keeps head plus evidence
  (`s3`, `s3b`, `s8`). Pruned evidence says so in the history and on the read (`review_copy`) (`s6b`, `s8`). Admin open of
  a pruned version is `404 version_not_retained {oldest_kept, pinned_versions, evidence_pruned:true, message, flags:[...]}` (`s8`).
- Keyless message and `kinds` (55a V1): `422 keyless_question {positions:[0,1,2,3], kinds:["Short","Short","Tf","Mc"], message}`
  (`s4`). `hide_key_changed` and the PATCH's own lost race both seen (`s4b`).
- Invites rotate (55a V6): `202 {..., resent:true, message}`, same `invite_id`, new code, old code `422 circle_invite_invalid`,
  rotation counts as a day's email invite but not a day's code (`s5`, `s7`).
- Unblock releases the cap-listed flagless versions (55a V3): an author with 9 withheld versions (5 flagged, 4 on the cap
  flag) reads her head again, hold gone, history all `withheld:false` (`s6`).
- 15 colleagues at one school: 10 go, 5 are `429 hourly_limit {metric:"circle_email_inviter_domain_hourly", limit:10, message}`
  `Retry-After: 3600`; the next hour all five go (`s5`). Copy: "You have sent as many invitations to addresses at this mail
  domain as you may in an hour (10). Try again in an hour." Clear.

## Variants and limits

### V1. A circle at its byte cap because of flagged heads is not freed by the owner, and the full-circle message gives advice that does not help
`server/worker/src/pool/circlesend.ts:221` (`QUOTA_MESSAGE`) and `:1287`; `circles.ts:1874-1912` (`purgeWhenReviewed` keeps
`PINNED_EVIDENCE_SQL`); `circlesend.ts:94` (`BANKS_SQL` counts every bank with `purged_at IS NULL`).
Repro `s3c`: three flagged heads of 30 KB, cap lowered to 3.5 heads; owner removes each and purge-now (each `200
purged_partial`). Bytes used before and after: 90,966 and 90,966. A clean 30 KB bank is then `413 quota_exceeded {scope:"circle",
used:90966, needed:30511, limit:106127}` with the text "The owner (or a bank's author) can trim a bank's older versions, or
remove a bank and purge it." Neither helps: trim keeps the head, purge keeps the evidence. A smaller bank fits only by luck.
Only after an administrator marks the evidence captured did `used` fall (71,235 with one captured). The evidence bound
(5 a bank, 20 a circle, a quarter of the bytes) does not cover heads, and a purge-claimed bank still counts toward the 100 banks.
The owner has no figure for what is held for review and cannot tell which bank holds it (after remove the circle row has no
`held`). With the default 200 MB and ordinary banks this needs a hostile or very unlucky circle (flags are 5 a day per
account), so I rate it low-medium, but with no administrator acting it is permanent and the owner is told the wrong way out.
Suggest: when the pinned heads are what fills the circle, say so in the 413 ("some space is held for an administrator's
review") and add `held_bytes` to the 413 body. README "Documented, not changed" does name the head and the owner's way out; the
413 text still contradicts it.

### V2. A `cap` flag the administrator resolved keeps collecting versions, and an unblock no longer finds any of them
`circlesend.ts:212-217` (`CAP_FLAG_SQL` is `ON CONFLICT (dedup) DO UPDATE` whatever `resolved_at` says);
`circleadmin.ts:84` (`f.kind = 'cap' AND f.resolved_at IS NULL`).
Repro `s10`: takedown; one editor sends 6 matching versions (5 flags plus one `cap` flag naming v6); admin
`POST /admin/pool/resolve-flag` on the cap flag (`200`); the editor sends 3 more the same day. The (resolved) cap flag now lists
v6-v10 and nothing is open. `/admin/dmca/unblock {release_circle_versions:true}` answers `circle_versions_released: 5` (the
five real flags) and v6-v10 stay withheld; the author's head is still `404 version_withheld`. The administrator has no open
item naming the bank. Her way out is a clean resend. Severity low-medium (the admin must resolve a cap flag as an "ack" first).
Suggest: a cap flag dedup that only matches open flags (a resolved one makes a new flag), and unblock reading cap flags
whatever their state.

### V3. A rotated invite looks like nothing in the owner's list, the activity or the mail
Repro `s5`, `s11`. `GET /circles/:cid/invites` has no `resent` field. A rotation overwrites `created_at`, so the original
invite time is lost and the row jumps to the top. The activity shows a second `email_invited` (`role` of the new role), also to
viewers. The mail is the same fixed text as the first, so it does not say it replaces an earlier code; the colleague who tries
the first code sees the generic `circle_invite_invalid` ("may have expired, been used up or been withdrawn"). Masked labels
cannot tell `jsmith`, `jjones` and `jlee` apart beyond `j***@... #2`; `a@` is `***@...`; two people at `jo`/`jsmith` share
`j***`. The client should show `created_at` and the role next to the label. Not blocking.
Also: the rotation counts as one of the 10 an hour for the domain, so an owner who used the 10 cannot resend to a colleague who
lost the mail until the next hour (`s5`: `429` on carol); three mails to one mailbox in a day then `422 invite_not_sent
address_limit` and even a fresh invite after revoke is refused (`s5`), as designed. The "+tag" fold means a rotation mail goes to
the address as typed this time (`jsmith+x@`), not the first one.

### V4. The purge-now answer is unambiguous, but nothing on the next list says "pending review"
Repro `s3`. Purge-now of a removed bank behind a pending flag: `200 {ok:true, purged_partial:true, pending_review:true,
evidence_kept:1, message}`; a repeat gives the same `200`; undelete `409 bank_removed {reason, message}`; GET bank `409
bank_removed {reason, undo_until}`. A client keyed on `ok` treats it as success, which is right. But the circle page then
lists the bank as `removed:{reason:"owner_removed", undo_until:<the moment of the click>}` for as long as the administrator
takes: the same row as "the undo window passed, purge next day". No field says purge was claimed or why it waits. Today's
client (`src/share/circles_api.rs:857-860`) discards the body (`Result<(), _>`), so it shows a plain "deleted" and the owner
never learns space remains used (V1). Suggest `purge_pending: true` on the removed row, and the client reads `purged_partial`.

### V5. A decision for a question with no label finding is a refused, charged attempt with a misleading code
`s2`: PATCH `{hide_key:true, labels:[{question:0,choice:"keep"}]}` on a bank with no labels answers `422 labels_need_decision
{errors:[{code:"labels_unused"}], label_findings:[]}` and counts toward the 60. A client that sends a decision for every
question ("keep" by default) would be refused for the whole bank and burn attempts; it must send only the questions that
`label_findings` names. Also a real formatting finding is not `formatting_findings` but an entry in `errors`
(`code:"key_in_formatting"`, `reason`, `ack_allowed`, `hint`, `author_only`), in `invalid_payload` when it is alone and inside
the `labels_need_decision` answer when both apply (`s9`); when `ack_allowed:false` (a star mark) the text must be fixed and a
new version sent. So the honest worst case is four calls a bank (labels, formatting, order acks, done) plus the retries a text
fix costs: 12 banks is 48, 15 is 60. It fits, only just. The 429 has `metric` and `message` but no `limit`/`used`, and "Try again
tomorrow" is UTC midnight (4 or 5 pm in California): `Retry-After` is the true value.

### V6. A promoted viewer's essay: the message is right now, but the client must re-import
`s4`: stripped copy sent as is: `422 keyless_question {positions:[0,1,2,3], kinds:["Short","Short","Tf","Mc"], message}`; after
re-keying only the Mc, `positions:[0,1,2]`. The message ("get the full bank again ... or give each of them its answer: mark the
correct one, or type a sample answer") is a single text for mixed kinds; the essay still cannot be fixed in place except by typing a
made-up sample. After promotion the viewer's read is the full bank (`keys_hidden` absent, no `keyless`), so the full copy is the
way. `kinds` cannot tell an essay from a keyed short answer (both `Short`, README says so). Client: on promotion, re-import.

### V7. Withheld and pruned evidence is visible to every member's history, not only the author
`s8`: a viewer's `GET .../versions` lists the editor's withheld versions with `by`, `withheld:true`, `kept:false` and
`review_copy:"full"|"hashes_only"`; a viewer's read of a pruned one says "withheld for an administrator's review and its text
is no longer kept". It does not say copyright or student data, so no new leak, but every member learns who had a version
withheld and how long the text is kept. `kept:false` with `review_copy:"full"` reads as a contradiction (kept for review, not
for members); the client should key off `withheld` and not show "trimmed". For a version withheld past the day's flags the
message says "a record of what matched is kept" but only the cap flag's takedown list exists (no block hashes).

### V8. Unblock caps at 50 versions and trim cannot free pinned bytes: both documented, both reproduced
`s6b`: after 8 flagged sends and a clean fix, `trim keep 1` answers `{ok, pruned:3, evidence_kept:5, oldest_kept:9,
pinned_versions:[1,2,3,4,5]}`: five pinned versions the owner cannot remove, as documented.

## Nitpicks

- README says `0111` and `0113` got `IF NOT EXISTS` "on their `CREATE TABLE`/`CREATE INDEX`". `0113`'s `CREATE TABLE
  circle_holds_r44` has none (only the index). It still reruns fine because the table is renamed away (see Migrations).
- The rollback `hide_key_changed` message says "send it again" (`circlesend.ts:1281`); for a rollback it should say roll back
  again.
- `PATCH` with `{}` (or a body with only `hide_key: undefined`) answers `400 invalid_request {field:"license", allowed:[...]}`:
  a client bug reads as a licence error. `invalid_request {field}` has no `message` for `role`, `hide_key`, `labels`.
- Evidence-captured for a version no flag names answers `200 {ok:true, flags:0}` (silent no-op); the note must be 10+ characters
  (`400 {field:"note", message:"Say why (at least 10 characters)."}`), which a script of mine hit first.
- Cap flag `detail.versions[].version` came back as `6.0` under the harness (a REAL bind inside `json_object`); a reader should
  parse it as a number. Not seen in the other flags (`JSON.stringify`).
- The 413 `used` excludes versions the commit would prune, so it can read lower than what the circle holds (V1).
- Hide-key off, an attempt too, also counts toward the 60.
- README's API table is behind the round-48 text: the `invites/email` row lacks `resent`, `message`, the two `hourly_limit`
  metrics and `circles_limit {metric:"circle_open_email_invites"}`; the trim row lacks `evidence_kept`; the versions row lacks
  `review_copy` and the `version_not_retained` extras; `circle-evidence-captured` is not in the admin route list
  (`README.md:1154-1159`); the `PATCH` row lacks its answers and errors. The table below is the one to use.

## API table for the client builder (each row seen in a live call in `c56/s*.test.ts` unless marked "not run")

Status and error codes use the JSON `error` string. Every error body also has `message` unless noted.

| Route | Request | Answer / fields | Errors seen |
|---|---|---|---|
| `POST /circles/:id/invites/email` | `{email, role: "editor"\|"viewer"}` only | `202 {ok, invite_id, kind:"email", role, max_uses:1, expires_at, emailed:true, address_label}`; rotation adds `resent:true, message` and keeps `invite_id`, label; role and expiry are new | `400 invalid_request {field:"email"\|"role"\|<other key>}` (message for email and extra keys, none for role); `404 not_found` non-member; `422 invite_not_sent {emailed:false, reason:"not_deliverable"\|"address_limit"}`; `429 hourly_limit {metric:"circle_email_invites_hourly"\|"circle_email_inviter_domain_hourly" (+limit)\|"circle_email_domain_hourly"}` with `Retry-After`; `429 daily_limit {metric}` (not run); `422 circles_limit {metric:"circle_open_email_invites", used, limit}`; `409 circle_full {limit}` (not run), `409 conflict` on a rotation lost race (code), `503 busy` (code); editor `403` (not run) |
| `GET /circles/:id/invites?after=` | owner | `{invites:[{invite_id, kind:"code"\|"email", address_label ("" for codes; "j***@d" or "...#2"), role, created_at (reset by a rotation), max_uses, uses, expires_at, revoked_at, note, created_by, state:"active"\|"used_up"\|"expired"\|"revoked", created_by_name}], next_cursor}`; no `resent` field | |
| `POST /circles/:id/invites/revoke` | `{invite_id}` | `200 {ok, invite_id}` | `409 already_revoked {revoked_at}`; a rotated invite revokes like any |
| `POST /circles/join` | `{code}` | unchanged | an old (rotated) code: `422 circle_invite_invalid` |
| `PATCH /circles/:id/banks/:b` | `{hide_key: bool, labels?:[{question, part?, choice:"strip"\|"keep"}], order_acks?:[{question, part?}], ack_formatting?:[int], image_ok?:[int]}` (or `{license}`) | on: `200 {ok, hide_key:true, held, withheld, labels_removed, images_kept, images_removed, figures_removed, order_reasons, restored?}`; same setting: `200 {ok, hide_key, unchanged:true}`; off: `200 {ok, hide_key:false, restored?:[v]}` | `400 invalid_request {field:"hide_key"\|"labels"\|"nonsense"\|"license" (for {})}` (a question outside the head adds `message`); `403 circle_role_required {needed:"owner_or_author", role}`; `422 labels_need_decision {errors, label_findings:[{question, part?, order_revealing, strip_allowed, choices:[{before, after}]}]}` (`errors[].code` may be `labels_unused`, `key_in_formatting`); `422 order_ack_required {errors, needs_order_ack:[{question,part?}], order_reasons}`; `422 invalid_payload {errors:[{code:"key_in_formatting", path, reason, ack_allowed, author_only, hint}]}`; `409 conflict {head_version, message}`; `429 daily_limit {metric:"circle_hide_attempts"\|"circle_hide_changes"}` + `Retry-After`; `503 busy` (code) |
| `POST /circles/:id/banks` (new) | send meta plus `hide_key: bool`, strip decisions | `201 {circle_id, bank_id, version, version_id, qids, added, changed, removed, held, withheld, keys_hidden?:true}` | `422 keyless_question {positions (<=100), kinds, message}`; `413 quota_exceeded {scope:"circle", used, needed, limit}`; `422 circles_limit {metric:"circle_banks"}` (code) |
| `PUT /circles/:id/banks/:b` (version) | send meta with `base_version`, `commit_id` | `200 {..., held, withheld}` | `400 invalid_request {field:"hide_key"}` if `hide_key` is sent; `422 keyless_question` (as above); `409 hide_key_changed {hide_key, head_version, message}` (nothing stored: send again, with decisions when `hide_key` is now true); `409 conflict {head_version, head_at, head_by}` (a real head move); `404 not_found` for a bank held from you |
| `POST .../rollback` | `{to_version, base_version, commit_id, ...}` | `200 {..., rollback_of, held, withheld}` | `409 hide_key_changed` (message says "send") ; `403 circle_role_required {needed:"owner_or_author"}` |
| `GET .../banks/:b` | member | bank `hide_key` (all roles), `keys_hidden:true` (viewers), `held` on the circle page row for author and owner | `404 version_withheld`; `409 keys_hidden_unavailable` (code; not run); `429 daily_limit {metric:"circle_hidden_reads"}` + `Retry-After`; after a purge claim `409 bank_removed {reason, undo_until}` |
| `GET /circles/:id` | member | bank row: `hide_key`, `held` (author, owner), `removed:{reason, undo_until}` (no purge-pending marker) | |
| `GET .../versions` | member | `versions:[{version, version_id, by, at, message, added, changed, removed, question_count, license, kept, withheld, review_copy?:"full"\|"hashes_only"}]` (`review_copy` only when `withheld`) | |
| `GET .../versions/:n` | member | a kept version | `404 version_withheld`; `404 version_not_retained {oldest_kept, pinned_versions, withheld?:true, review_copy?:"hashes_only", message}`; `404 not_found` beyond the head |
| `POST .../trim` | `{keep: 1-19}` | `200 {ok, pruned, evidence_kept, oldest_kept (may be null), pinned_versions:[v]}` (`pinned_versions.length == evidence_kept`) | `400 invalid_request {field:"keep", min:1, max:19}`; `403 circle_role_required {needed:"owner_or_author", role}` |
| `POST .../purge` | owner | `200 {ok:true}`; with a flag pending `200 {ok:true, purged_partial:true, pending_review:true, evidence_kept, message}` (repeatable; the purge is claimed; the daily run finishes it) | `409 under_moderation` (report or admin hold); `404 not_found` once purged |
| `POST .../undelete` | owner | | after a purge claim `409 bank_removed {reason, message}` |
| `GET /circles/:id/activity` | member | new `what`: `email_invited` (`by`, `member:null`, `role`), `hide_key_changed` (`hide_key`, `bank_id`, `bank_title`); a rotation is another `email_invited`; purge-now and revoke are not events | |
| `POST /admin/pool/circle-evidence-captured` | admin (sudo) `{bank_id, version?, note>=10}` | `200 {ok, flags:n}` (0 when nothing matches, also when repeated) | `400 invalid_request {field:"note"\|"bank_id"\|"version", message?}`; `404 not_found` |
| `GET /admin/pool/circle-bank?id&version&basis` | admin | pruned: `404 version_not_retained {oldest_kept, pinned_versions, evidence_pruned:true, message, flags:[{flag_id, kind, at, resolved_at, detail}]}`; after a purge: the same without `evidence_pruned` and `flags` | |
| `POST /admin/dmca/unblock` | `{action_id, note, release_circle_versions?}` | `200 {ok, lifted, released, circle_flags_lifted, circle_versions_released}` (counts the cap-listed ones) | cap flags already resolved are not read (V2) |
| Flag detail (admin) | | `positions:{count, first}`, `actions`, `hashes (<=20)`, `evidence_captured(_at,_by)`, `evidence_pruned(_at)`, `via:"hide_key"`, `block_lifted`; `cap` flag: `{metric:"circle_send_flags", circle_id, bank_id, versions:[{version, version_id, actions, student_data}]}` | |

## README "Round 47/48" against the code

True as stated: the 60-attempt counter with refusals included and its cheap pre-checks; 12 banks fit (36, or 48 with a
formatting step); `kinds` and the essay not accepted; `hide_key_changed` 409 and the PATCH lost race with `head_version`;
the cap flag now names bank, circle and versions and unblock releases them; `review_copy`; trim `pinned_versions`;
`purged_partial`; capture frees trim, the daily purge and the owner's purge; rotation and `resent`; the two domain texts;
role hint `owner_or_author` (the licence change keeps `author`, not run).
Not in the README, found: V1 (413 text), V2 (resolved cap flag), V3 (no `resent` anywhere but the answer), V4 (no purge marker),
V5 (`labels_unused`, formatting findings inside `errors`), and the nitpicks above.

## Migrations 0111-0115 (`mig.mjs`)

- Populated 0110 data (5 `circle_invites`, 3 `circle_holds` with reasons `reports`/`student_data`/`admin` and their sources, foreign
  keys on while the rebuild ran): after 0111-0115 every invite has `kind:'code'`, `email_label` and `email_key` NULL; all three
  holds are intact; `PRAGMA foreign_key_check` 0 rows, `integrity_check` ok; a `copyright` hold inserts.
- Fresh (0001-0115) against migrated (0001-0110 plus data plus 0111-0115): `sqlite_master` identical, no difference, nothing extra.
- Idempotence: rerun on a migrated database: `0113` OK, `0114` OK, `0111` fails `duplicate column name: kind`, `0112` fails
  `duplicate column name: hide_key`, `0115` fails `duplicate column name: email_key`. That is what the README says ("an `ADD
  COLUMN` cannot have it"); D1 applies each file once. A database that already ran the earlier edition of `0114` would keep its old
  index under `IF NOT EXISTS` (README says nothing is applied anywhere).
- `APP_URL` in `wrangler.toml:41` is still `https://app.example.org` and the client has no join link route, so any mail before both
  exist has a dead link (README names it). The mail path text ("Shared, then Circles, then Join a circle with a code") matches the
  client heading (`src/share/circles_ui.rs:329`).

## Verdict

No new-class blocker. Round 48 fixes all hold in the form I tested. For the client builder: read `purged_partial`,
`review_copy`, `kinds`, `evidence_kept`, `resent`, and treat `hide_key_changed` as "send again", a `conflict` without
`head_version` as "try again". For a round 49: V1 (the 413 advice and a figure for space held for review), V2 (cap flag after a
resolve), V4 (a purge-pending marker on the removed row), V5 (a clearer code than `labels_need_decision` for `labels_unused`).
