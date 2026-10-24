# Critic 43: circles phase A CLIENT (uncommitted, src/share/)

Scope: circles.rs, circles_api.rs, circles_ui.rs, circles_text.rs, circle_send.rs, circles_tests.rs, and the edits in browse*.rs, publish*.rs, shared*.rs, pool.rs, mod.rs, app.rs. Compared with `server/worker/src/pool/circles.ts`, `circlesend.ts`, `circlecheck.ts` and README rounds 32-35.

What I ran:
- `cargo test --lib share::` -> 410 pass. `cargo check --lib --target wasm32-unknown-unknown` is clean. `std::thread` is only in `cfg(not(wasm32))` arms.
- Throwaway tests, in a scratch COPY of the repo (nothing in the repo was touched; the copy is deleted). The test block is kept at `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c43_tests_block.rs` (appended to `circles_tests.rs` in the copy, run with `cargo test --lib c43_ -- --nocapture`).
- A worker answered on 18791 (`{"error":"not_found"}` on `/`), but every route needs a pool session, so I did not drive it. The server side was read from the TypeScript. Where I claim a server shape, I give the line. The round-35 test `test/pool/round35.test.ts:65` asserts the masked shape `[null, null, null, true]` for title, subject, author_name and hidden.

No injection or secret leak found (see "Held" at the end). Seven blockers below, all functional or trust, none a security break.

## Blockers (new class)

### B1. A masked (hidden) bank makes the whole circle page unreadable for the owner (high, contract mismatch with round 35)
- Server: `circles.ts:408-417` (round 35 B1). A REMOVED bank that is held, or whose author is not in good standing, is listed to the owner with `title, subject, course, grade, author_name = null` and `hidden: true`.
- Client: `circles_api.rs:187-206` (`BankEntry`: `title: String`, `subject: String`, `author_name: String`). `#[serde(default)]` covers a missing field, not an explicit `null`. `Pool::circle` (`circles_api.rs:498-501`) parses the whole `CircleDetail` with `parse`. There is also no `hidden` field.
- Ran (`c43_hidden_removed_bank_breaks_owner_page`): `GET /circles/:cid` as owner with one masked bank gives `detail loaded: false`, problem "The server sent a reply the app does not understand. Check the server address, or try again later." with a "Try again" that can never work.
- Consequence: in the very case round 35 was written for (erased author with `keep_on_delete: false`, suspended author, held removed bank) the owner cannot open the circle page. That means no invite, rename, close, leave, offer or purge. Wrong for 14 days at least.
- Fix: `Option<String>` for those five fields (or a custom null-to-empty deserializer), add `hidden: bool`, and show "a bank hidden while it is reviewed" instead of a blank card. See limit L1 for the follow-on.
- Check on the test suite: `circles_tests.rs` has no fixture with a null title, so 410 green says nothing here.

### B2. "Bring it back" fails silently, and is offered for banks that can never come back (medium)
- `circles.rs:1222-1230` routes a failed `RestoreBank` to `self.bank.act_problem`. `restore_bank` (`circles.rs:1899`) is only called from the circle page, where `self.bank` is `None`. The error is dropped; `act_problem` and `notice` both stay empty.
- Ran (`c43_restore_failure_is_silent`): 409 `under_moderation` on `.../undelete` gives `act_problem=None notice=None`. The click does nothing visible.
- `bank_can(Restore)` (`circles.rs:87`) is `removed && (Owner || is_author)` for any removal. The server (`circles.ts:1259-1275`) refuses with 409 `bank_removed` ("can no longer be restored") when the reason is not `author_removed` or `owner_removed` (erasure, `circle_closed`, admin). It also refuses with 403 when an author tries to undo the owner's removal (`owner_removed`). The client has `removed.reason` and `undo_until` (None for those reasons) and does not use them, so these are dead buttons.
- Fix: show only when `reason` is `author_removed` or `owner_removed` and `undo_until` is in the future; for the author, only `author_removed`; send errors from the circle page to `circle.act_problem`.

### B3. A sign-in that ends mid-action leaves Create, Copy and Report stuck "working" (medium-low)
- `problem_of` returns `None` for `SignedOut` and the callers just `return`: `on_act` (`circles.rs:1212`), `on_copy` (`:1152`). The in-flight flag set at launch is never cleared.
- Ran, with `s.fail(ApiError::SignedOut)`:
  - Create: `create.working=true` stays after the gate. The button is disabled; only "Cancel" gets out.
  - Copy: `CopyPanel::Working` stays; after re-login it is still `Working`. `cancel_copy` ignores `Working`, so the only way out is leaving the bank page. The user never learns the copy did not happen.
  - Report: `ReportState::Sending` stays, with the same disabled button.
- The existing `a_session_that_ends...` test covers the send window only.
- Fix: in the `SignedOut` arm reset `create.working`, `CopyPanel::Working` to `Choosing`, `ReportState::Sending` to `Editing`. This is the same class for each in-flight flag.

### B4. The UI tells members they may re-share publicly; phase A refuses all of it (medium, trust)
- Shown on every bank card (`circles_ui.rs:434`, `circles_text.rs reshare_line`): CC0 gives "Free to use: members may share it publicly."; every other licence including NC gives "Members may share it publicly if they credit the author." `LICENSE_SETTING_NOTE`, `RESHARE_NOTE` and the author's licence picker build on this. `README` round 32 and `circlecheck.ts:24` say the opposite: the public publish check refuses circle text "whatever the bank's licence (members' re-sharing of CC circle banks waits for phase C)".
- Result: an author who picks CC-BY to be generous, and a member who reads "may share it publicly", both get a different answer from the server (`422 circle_material`, only the "I wrote these myself" route). A teacher can build a public item from a copy and meet the refusal after the work.
- Fix: say "Later: members will be able to share it publicly if you choose a Creative Commons licence. Not yet: sharing a circle bank's questions publicly is not available in this version." Same for NC (the line also ignores NonCommercial).

### B5. A 404 for a missing TARGET is read as "you lost the circle" (medium-low)
- `on_act` (`circles.rs:1210,1231-1241`) treats every 404 as the caller losing access: it clears `circle`, `bank`, `panel`, goes to the list, and says "...you may have left it or been removed."
- The server uses `404 not_found` for the target too: `removeMember` (`circles.ts:841`, target already left), `setRole` (`:790`), `offerTransfer` (`:760`, target gone), `revokeInvite` (`:572`, unknown invite id).
- Ran (`c43_404_for_missing_target_kicks_owner_out`): remove, role and offer each give `screen=List circle_cleared=true`. A stale members list (a member left a minute ago) throws the owner out of their own circle with a wrong explanation.
- Fix: after a 404 on a member/invite action, reload the circle page instead (a real loss of access then shows from `GET /circles/:cid` as a 404, which `on_detail` already handles).

### B6. Failed actions on the circle page offer a dead "Try again" and never refresh (low-medium)
- `circles_ui.rs:399-406`: `problem_box(ui, &p, w, &mut r, &mut a)` for `act_problem` ignores `r`. An offline or 5xx failure sets `retry = true`, so a "Try again" button is drawn and does nothing.
- `on_act` never reloads the detail after a failure except `circle_closed`. After `403 circle_role_required` (role changed) or `409 no_pending_transfer`, the page keeps showing the old role and the Accept banner, and every click gives the same error. The messages say "Reload the page" and "Reload the circle" (`circles.rs:898,904`), but `reload_circle` (`:1484`) is dead code and no control calls it. The only way is to leave and re-enter.
- Fix: honour `r` (re-run the last act or reload) and reload the detail after any 403, 404 or 409 on an act.

### B7. Several actions have no busy guard; a double click drops the first result (low-medium)
- `launch` replaces any job of the same kind (`circles.rs:927`), so the first request still goes to the server and its result is thrown away. Buttons that use `busy_action` are safe. These are not guarded in the UI or the model: `withdraw_code`, `restore_bank`, `set_role`, `set_bank_license`, the member row buttons, and `send_report` when another act runs.
- Ran (`c43_double_click_withdraw`, two `withdraw_code` before a frame): two POSTs; the second answers `409 already_revoked`; the page shows "That code was already withdrawn." for a code the user just withdrew, and the list is not refreshed (still "Active"). `Report` can stay at `Sending` if a second act replaces it (same mechanism as B3).
- Fix: refuse a new act while `running(Act)` in the model (one place), not per button.

## Variants / limits to document (not blockers)

- L1. After B1 is fixed, a masked bank has no title, so cards need a "hidden" presentation. There is no purge route in the client at all (`POST .../purge`, round 33). An owner at the 100-bank cap with removed banks must wait 14 days, which is the case round 33 V3 added the route for.
- L2. Copy drops `origins`. `BankDoc.origins` is kept only for a one-line note (`circles_ui.rs:868`). The copy's description (`circles.rs:1920`) names the circle and author only, so upstream CC credits that the circle bank carries are lost in the local copy. The note says "their credits are kept with it", which is true of the circle bank, not of the copy.
- L3. `CircleMaterial::banks()` dedups by bank id and keeps the first hit's `may_claim_independence` (`pool.rs:628`, `publish.rs:1248`). The server computes it per position (`circlecheck.ts:174`). With a bank whose first hit is not claimable and a later one is, the button is enabled (`any`) but the list is empty and the user gets "There are no circle banks to name." Compute per bank with `any`.
- L4. `Refusal::message()` returns server `message` text unfiltered for `ModerationState`, `ItemRemoved`, `NotApproved`, `SchemaUpgradeRequired`, `LicenseIncompatible` (`pool.rs:442-451`). `circles.rs problem_of` passes `ModerationState` through `plain`; the other circle paths (`circle_send.rs problem`, `Problem` from `e.message()`) do not. Today every server message is a constant string, so there is no injection, but it breaks the rule "everything through `plain`" if a message ever includes a name.
- L5. `circle_closed` text always says "closed by its owner" (`pool.rs` `CircleClosed`, `circles.rs:880`, `circles_ui.rs:182,393`), but the server also closes for `admin` and `last_member_gone` (`closedError`, `circles.ts:239`). The Reopen button is offered to the owner on an admin-closed circle (`allowed(Reopen)` was true in `c43_circle_closed_reasons`); the server refuses with 409 and the answer says "closed by its owner". The server's own `message` field is right but unused.
- L6. `circles_limit` metric `circle_memberships_total` is worded "Leave one first" (`pool.rs`), but README F4 says leaving does not reduce it. The server `message` ("You can join another once some of them close") is correct and unused.
- L7. A lost `POST /circles` or `POST .../invites` response cannot be retried without making a second circle or code (no idempotency key on those routes). The "Try again" is offered for `PoolError::Api` (`problem_of`). Sends have a commit id; those do not. A lost `send` is replayed only while title, licence, note and payload are the same (`circle_send.rs:342`); if the user edits the title after a lost answer, a second bank is created.
- L8. `Outcome::ShareToCircle` sets `share_target` (`shared_ui.rs:300`) but nothing clears it when the chooser is closed or the tab changes (`SharedModel::goto` only clears `choosing`). A later "Share to pool..." from another tab opens the chooser with the circle heading and sends to that circle. The heading shows it and the destination radio on the options step lets the user switch, so this is a surprise, not a mis-send.
- L9. Invites and activity load failures show a warning with no retry (`circles_ui.rs:740,785`); `retry_circle` is only reachable from the main problem box. Daily limits (`circle_transfer_offers`, `item_reads`) show "You can try again in about N seconds" from `retry_after` (`circles_ui.rs:46`), which can be tens of thousands of seconds; the metric has no `metric_text` entry (generic "today's limit").
- L10. The licence picker offers four of six licences (`circles_ui.rs:975`). An NC or NC-SA bank is not highlighted as current and cannot be returned to after a change.
- L11. Secrets: `JoinState` derives `Debug` and prints `text` (the pasted code) in clear; nothing prints it today. `Work::Join(String)`, `Work::Preview(String)`, `JobMsg::Preview(_, String)` and the parsed `Value` in `create_invite` hold the code in plain `String`s that are not zeroized on drop. Nothing is written to disk or logs. Harmless but not what `Secret` promises.
- L12. `heads` (up to 200 ids) takes the first 200 of the sorted ids (`browse.rs circle_bank_ids`), so a person with more than 200 copies never gets badges for the rest.
- L13. Image copy: a failed blob fetch is counted as "missing" and the copy still completes (`circles.rs:552`). With a 429 from the `blob_miss` guard every picture would be missing, and the copy is still saved. The `Done` panel says how many are missing; there is no way to repeat just the pictures.
- L14. Test fixtures use invented statuses (404 for `circle_invite_invalid`, 410 for `bank_removed`; real 422 and 409). They pass because only the code is matched. They do not cover a null title (B1).

## Nitpicks

- `circles_ui.rs:558`: expiry of a new code uses `super::browse::now_ms()` instead of `m.now_ms()`.
- `activity_line`: an admin actor arrives as "an administrator", so lines start lower-case ("an administrator closed the circle.").
- `plain()` caps at 500 characters silently; a bank card with a title of only format characters is blank (no fallback "Untitled").
- `role_name`, `reload_circle` and `NOT_REACHED` are unused.
- `until()` says "now" for a past expiry on an *active* invite line; `invite_line` shows "works now".
- The member row's right-aligned buttons are placed in a `horizontal_wrapped`; the layout agent is on this, so not rated.

## Held (tried to break, did not)

- (b) Server strings: names, titles, descriptions, notes, circle names in the join preview, activity and offer banner all go through `api::plain`/`btext::author`/`shown`. egui labels carry no markup or links. Unknown `what` and unknown `status` degrade to neutral text. Raw codes: only the final fallback in `other_message` can show `(code)`, and `ident()` limits it to identifier characters; every circle code in the routes I read has a mapped sentence.
- (c) Secrets: `Secret` for the code in `InviteCreated`, the join preview and `Panel::Code` (custom `Debug`: "InviteCreated(id, <code>)"); `close()` wipes the panel and join text; `ImportedRecord` stores only `circle_id` and `circle_name`; no side file holds a code; `create_invite` errors do not echo it. The code is shown once and "Copy" is an explicit click.
- (a) Matching fields: list, detail, members, `pending_transfer`, `ownership_offered_until` (Option), invites (`note` null, cursor `created_at:invite_id`), activity (all 15 `what` values, `next_before`), join preview/`already`, heads (all four statuses, `until`), bank read head (`held`, `version_license`, `committed_by`), 202 offer, 429 `daily_limit` generic, `circle_material` null ids and names (`Option` and empty bank id handled in `publish_ui.rs:1115`). Every status and code I could compare matched except B1.
- (d) Commit id: kept while `cid|licence|message|meta|payload sha` is unchanged, cleared on done, payload refusal, licence refusal and `commit_id_conflict`; `attest` is always sent; blob_missing resends once. The server flags an independence claim only at the real publish and only for banks it was needed for (`publish.ts:2631`), so a stale claim is harmless; the client also clears it after a `circle_material` refusal.
- (e) Copy: new bank each time (`Dest::New`), correct bank/version/licence recorded, `item_id` = circle bank id, public update checks skip circle records (`check_updates` filter), the public `on_versions` cannot collide with circle ids, `apply_circle_heads` changes only circle records.
- (f) wasm: `cargo check --target wasm32-unknown-unknown` passes; `web_time` used for clocks; no `std::fs` or `SystemTime` in the circle files.
