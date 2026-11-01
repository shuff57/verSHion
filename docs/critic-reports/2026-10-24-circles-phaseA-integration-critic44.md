# Circles phase A: client / worker integration critic 44

Scope: the Rust client (client commit 8dd4ddd) against the TypeScript worker (backend cacb512), run end to end over real HTTP against the local rig at `http://localhost:18791` (current code, migrations 0108/0109). Nothing else was contacted. `a@example.test` and `b@example.test` were not signed in as and their data was not touched; the seed circle "Maths department" is still there.

## Verdict

**One blocker (new class), found and reproduced.** A circle created with a blank description (the most common way to create one) cannot be opened by anyone, ever, from the client. Everything else I drove end to end agreed between the two sides; the rest are variants, limits and nitpicks listed below.

Baselines (run first, unchanged tree):

- `cargo test --lib share::`: 423 passed, 0 failed (700 filtered out).
- `cd server/worker && npm test`: 553 tests, 553 pass, 0 fail.

## Method

- A scratch crate (`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c44/`, path dependency on the repo, no repo file changed) drives the live worker through the client's own `Client`, `Pool`, `CircleSend`, `PublishRequest` and `publish_text` code, plus raw requests for shape checks.
- For every response: the raw JSON was parsed strictly with the client's own serde structs (`serde_json::from_value`, not the client's `unwrap_or_default`), the key set was diffed against the struct's fields, enum values (roles, invite states, head statuses, activity kinds) were checked against the client's tables, and every refusal's `Refusal::message()` was checked for plain English (no raw snake_case code, no "The pool refused the request (code)" fallback).
- Seven to eight throwaway accounts per run (owner, editor, viewer, second editor, outsider, erased-author, erased-owner, D13 owner, plus pending and teacher-only accounts), made through the real sign-up / mail-log confirm / `rig approve` flow, with passwords generated in the driver process and never printed, stored or passed in argv/env. About 740 checks passed in the full run; the failures are exactly the items below. Driver output: `run7.log` (scenarios 1 to 13), `run5.log` (14), `run6.log` (15) in the scratch directory.
- Local-only helpers used on `server/worker/.wrangler-ui` (the rig's own database): cleared `rate_limits` rows and the `circle_creates` counters of my `c44*` accounts (sign-up and 5-circles-a-day caps), and set `circle_transfers.expires_at`, `circles.undo_until` and `circles.close_reason='admin'` on circles I made, to reach expiry, past-undo and admin-close states. No seed row was touched.

## What was driven and agreed (client parser accepts it, codes and messages are plain)

- Create / list / detail / rename, with description and emoji at the client's exact limits (60-emoji name, 1,000-emoji description, 200-emoji code note: all accepted; the worker counts characters the same way).
- Invite codes: editor and viewer, 2-use code, used-up, revoked, revoke twice (`already_revoked`), non-owner (`circle_role_required needed owner`), bounds (400s), state enum `active/used_up/revoked/expired`, preview (uses nothing, `already_member`), join, join again (`already: true`, role present), outsider on a used code (`circle_invite_invalid`, same wording for every bad code), no-invite teacher (`invite_required`), pending account (`account_not_approved`). The client's grouped/ungrouped code forms are accepted by the worker's normaliser.
- Role changes (`changed` true/false), owner role refused, remove, removed member sees 404 and an empty list, re-join with a code made before the removal is refused and a code made after works, leave and re-join with the same code works, remove/role of a non-member 404, malformed id 400.
- Send a bank: attest, all six licences, `message`, replay with the same `commit_id` (`replayed: true`), same id with other content (`commit_id_conflict`), missing attest and old terms version (422), viewer and outsider (403 / 404), a picture bank (upload, send, served `image.file`, `circle_blob` hash-verified), every question kind with letter labels and "All of the above" accepted, keyless, duplicate choice, no correct, blank-count mismatch, empty stem, forbidden character and 1,001 questions all refused with wording.
- List / view / copy: `BankEntry` and `BankHead` strict parse, the served payload converts with `merge::convert_question` for every question, answer keys present as the screen says, `ETag` and 304, `heads` statuses `current/removed/under_review/unavailable` (outsider gets `unavailable`, never `current`), licence change by the author only, activity (50 per page, cursor `at:id` accepted by the client's cursor check, page 2 loads) and every activity kind in the client's table.
- Reports and holds: own bank (400 `own_item`), three distinct members hold a bank; author sees `held: true`, others see it gone from the list, `under_review` in heads and 404 on open.
- Ownership: offer (202), same offer again, replace, member declines, owner withdraws, accept (old owner becomes editor), non-owner offer 403, offer to self 400, expiry (accept gives `no_pending_transfer`, `ownership_offered_until` disappears), `circles_owned` cap at accept (`circles_limit` with the 14-day wording), `pending_transfer` visible to owner and offered member only.
- Owner leaves: later-joined editor becomes owner before an older viewer, then the viewer, then the last leaver closes it.
- Close / undo / purge: close (twice returns the same `undo_until`), reopen, past-undo reopen refused (`circle_closed`), closed circle: list shows `closed_at/close_reason/undo_until`, send/open/code/preview all refused with `circle_closed`; bank remove (author, owner), restore rules (author cannot undo the owner's removal), purge of a live bank (`bank_not_removed`) and of a removed one.
- Hidden / removed banks: erased author with `keep_on_delete: false` gives `hidden: true` with null `title/subject/course/grade/author_name` (read as empty by `null_text`), `removed.reason = account_deleted`, `undo_until` null; owner can purge; held-then-owner-removed bank is hidden with a licence and `undo_until`, purge refused with `under_moderation`.
- Public publish (D13): `circle_material` for public, unlisted and preview; hits with circle and bank names; independence statement lifts it for a CC bank; a statement does not lift a circle-only bank the publisher can read (`may_claim_independence: false` on every hit); the first sender may publish; a short client-side statement is refused locally; a held bank gives a generic hit with null ids and names (client reads them as empty and refuses to build a claim); a former member gets ids with null names, and a statement lifts it.
- Erasure: a member's bank is kept as "deleted teacher" with `author_name` and activity names consistent; an erased owner hands the circle to the editor; codes made by an erased owner list as `revoked` with creator "deleted teacher". Export: valid JSON with a `circles` section (memberships, owned, invites without codes, banks, versions, transfer offers).
- Version 2 (raw `PUT`, the client cannot send one yet): `SentBank` parses, `circle_bank` reads version 2, heads says `current` 2, activity has `version_sent` with `version: 2`.
- Declared-origin licence check (raw, the client sends no origin): `license_incompatible` with `allowed` licences, all parseable by `License::parse`, credits served in `origins`.
- CORS preflight for GET/POST/PUT/PATCH/DELETE on `/circles/...`: all allowed (web build).
- The client's message table, fed 58 status/code pairs the circle routes can produce: every one has a sentence, none leaks a code, except the two admin-only codes `not_removable` and `not_closable` (not reachable from the app).

## Blockers (new class)

### B1. A circle made without a description cannot be opened (null `description` against a non-null `String`)

Severity: **blocker** (every circle whose owner leaves the optional description blank is unusable from the client for every member, and there is no client way to repair it). Not caught by the seed (it always sends a description), by the 423 client tests (fixtures carry a description) or by the worker tests.

Request (exactly what `Pool::create_circle` sends when the description box is empty, `src/share/circles_api.rs:501-509`; the description is left out when blank):

```
POST /circles   {"name":"C44 no description"}
```

Response:

```
201 {"circle":{"banks":0,"closed_at":null,"created_at":1792828158622,"description":null,"id":"e92e11af72cb9f29ece8b44eee1ea9e9","members":1,"name":"C44 no description","role":"owner","updated_at":1792828158622}}
```

Then `GET /circles/e92e...` answers 200 with `"circle":{... "description":null ...}` as well.

Client side:

- `src/share/circles_api.rs:145` `pub description: String` in `CircleInfo` (`#[serde(default)]` only covers a missing key, not `null`; `BankEntry` already uses `null_text` for this reason, `CircleInfo` does not).
- `circles_api.rs:508`: `serde_json::from_value(...).unwrap_or_default()` swallows the parse error, so `create_circle` returns `CircleInfo { id: "" , .. }`. `circles.rs:1314-1318` then opens `CircleView { id: "" }`: the page the person lands on after "Create" is an error ("That item id is not valid.").
- `circles_api.rs:512-515`: `Pool::circle` is `parse::<CircleDetail>`; it returns `Err(Api(BadReply("invalid type: null, expected a string at line 1 column ...")))`, shown as "The server sent a reply the app does not understand. Check the server address, or try again later." with a retry button that can never succeed. This is also what the circle page shows when opened from the list, for the owner and for every member who joins by code.
- The Rename panel sends only `name` (`circles.rs:419`), so the client has no way to set a description afterwards. `PATCH {description:""}` also stores null (the same `optText`), so even `update_circle(Some(""))` re-breaks a repaired circle.

Server side: `server/worker/src/pool/circles.ts:87-92` `optText` turns `""`/absent into `null`; `:316` and `:325` store it; `:337` (create answer) and `:422` (detail) send it as is; `:203` types it `string | null`.

Reproduced three ways in the driver (`run7.log`): strict parse of the create answer fails, `client circle() opens a circle created without a description` fails, `create_circle(name, "")` returns an empty id, and `circle opens after the owner clears its description` fails.

Fix, either or both (the second is cheap and removes the class):

1. Server: send `description: description ?? ""` in the `POST /circles` answer (`circles.ts:337`) and the detail (`circles.ts:422`); add a test that creates a circle without a description and reads it back.
2. Client: `#[serde(deserialize_with = "null_text")]` on `CircleInfo.description` (and on any other plain-`String` field of a struct the server can null); make `create_circle` return `BadReply` when the circle has no id instead of `unwrap_or_default`; add a client test whose fixture has `"description": null`.

## Variants / limits

Numbered for reference; "known" means already in the handoff list L1-L14 and re-confirmed live.

1. **Silent parse failure is the class behind B1.** `circles()` (`circles_api.rs:497`) turns any one unparseable circle into an empty list ("You are not in any circle yet"); `create_circle` (`:508`) and `offer_ownership` (`:668`) return a default struct. Today only `CircleInfo.description` can trigger it (every other plain `String` I fed null-prone data to parsed: erased creators/authors come as "deleted teacher", masked banks use `null_text`), but any future null would hide the same way.
2. **Admin-closed circle (known, L5, confirmed live).** With `close_reason='admin'` and `undo_until` null: the list and the page say "The owner closed it"; the owner is offered "Reopen the circle" (`circles_ui.rs:471`) and the call answers `409 circle_closed` ("This circle can no longer be reopened."), which the client words as "This circle was closed by its owner." (`pool.rs:477`). The server's own text for `admin` and `last_member_gone` is not used.
3. **Circle send never declares an origin (known deferred model patch).** The worker's licence check on a send (`circlesend.ts:276-295`) runs only on a question's declared `origin`, and the client's `Question` has no such field, so a CC BY-NC bank imported from the pool can be sent into a circle as CC0 or circle-only without a refusal, and the circle shows no credits (`origins` is `[]`). If origins are added later, the refusal's `allowed` list for an NC source is `["CC-BY-NC-4.0","CC-BY-NC-SA-4.0"]`, which the four-licence picker (L10) cannot select, so the refusal would be unfixable from the screen until L10 is fixed.
4. **Keyless imported question in a circle send.** Refused with `keyless_needs_hide_key`, worded "hide the key, or set the answers yourself", but a circle send has no hide-key option (`hideKey: false` is fixed in `circlesend.ts:198,326`). Only "set the answers" can work.
5. **`bad_url` (and 7 other share-core codes) have no wording.** `publish_text.rs:199` falls back to "the pool refused it (bad_url)" for `bank.details.license_url` typed without `https://` (the field is user-editable, `bankui.rs:2692`, hint "https://"). Codes with no entry: `ambiguous_kind bad_figure_id bad_form bad_hash bad_id bad_license bad_url unknown_kind`; only `bad_url` and maybe `bad_license` are reachable by typing. Same path for public publish.
6. **D13 short statements pass by design.** A multiple-choice question of at most 12 words in all (stem plus choices), and short True/False and Short questions, are not matched against circle text (`isShort`), so those can be published publicly from circle material with no check or statement. Expected per the plan, noted because the screen text says circle material "can not be shared publicly".
7. **`circle_memberships_total` (known, L6)** still says "Leave one first" (`pool.rs:472`); leaving does not lower it.
8. **Daily metric `circle_transfer_offers`** has no entry in `metric_text` (`pool.rs:587-612`): "You have reached today's limit." with no "for ...".
9. **`403 account_not_approved` without a `message`** falls back to "The server does not allow that right now." (the worker does send one today).
10. **Held then removed bank**: the owner's list shows it hidden but with a "Delete for good now" button (`bank_can` Purge); pressing it gives `409 under_moderation` ("This bank is under review by an administrator."). Plain, but the button is offered for a state that always refuses.
11. **Hidden-bank wording for an erased author**: `HIDDEN_BANK_TITLE` says "Hidden while a review is open" for a bank hidden because its author's account was erased (`removed.reason = account_deleted`, no review).
12. **Documented mismatches, harmless to the client:** offering ownership to a non-member answers `404 not_found` (README says `409 member_unavailable`; the app only offers listed members); `restore` of a purged bank answers `404`, not `409 bank_removed`; `GET /circles/:cid` for a closed circle is `200` with `closed_at` (README implies `409`; the client handles the 200 and uses 409 only for banks, codes and sends); `POST /circles` returns `members`/`banks` and no `joined_at/close_reason/undo_until`, so it is neither `CircleInfo` nor `CircleSummary` (parses once B1 is fixed).
13. **No idempotency on create (known, L7)**, retried creates make a second circle; combined with `circle_creates` 5 a day and `circles_owned` 5 this can lock a teacher out of creating for the day after a flaky connection.

## Nitpicks

- `Joined.role` is a plain `String` and `Role::parse` maps an empty or unknown role to **viewer**; safe direction, but a missing role on a join would show "Viewer" for an editor.
- `activity_line` for a removed bank whose title is null reads "removed a bank."; for a removed-by-erasure author "deleted teacher left." (correct).
- `code_in` accepts any 24 letters/digits after `C-`; letters outside Crockford (U) are rejected by the worker with `400 invalid_request field code`, which the circle page words correctly ("That does not look like a circle code ...").
- `BankEntry.hidden` and `held`/`removed` keys are omitted when false; the client defaults handle it.
- `GET /circles/:cid` sends `pending_transfer: null` (not absent) when there is no offer; it parses.

## Cleanup

All throwaway accounts were erased through the API (the ones from each run in-process; 20 strays left by two aborted driver runs through forgot/reset then delete, with in-process passwords). `select count(*) from users where email like 'c44%' and deleted_at is null` is 0. Seed data left as it was (`b@example.test`, the "Maths department" circle and its two banks, the three public items). Build output of the scratch crate went to the repo's gitignored `target/` (`target/debug/c44`, `msgs`, `cleanup`); `git status` shows only the two untracked paths that were already there, plus this report.
