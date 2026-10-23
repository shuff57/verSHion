# Whole-phase integration critic 39 (client x Worker x share-core x auth)

Branch shared-pool-phase1. Static reading of src/share/* against server/worker/src/pool/*, plus replays of client-exact bytes against the Worker test harness. No repo file was edited except this report.

## (a) Method and results

Run:
- `cargo test --lib share::` : 341 passed, 0 failed.
- `cargo check --lib --target wasm32-unknown-unknown` : OK (dead-code warnings only). Wasm parity holds.
- `cargo test --release --test share_core_fixtures` : 8 passed, 0 failed.
- Replay scripts (scratchpad, outside repo; Node + `server/worker/test/pool/harness.ts`) with the client's exact meta (mirror of `PublishRequest::meta_json`, pool.rs) and a bank from the share-core model fixture:
  - preview, then 422 `labels_need_decision`, then client-style decisions, then preview 200, then publish 201.
  - Same publish_id again returns 200 `replayed:true` (idempotent).
  - Blob PUT with a real app-encoded WebP returns 201.
  - `student_view` from preview passes the client's stripped-cleanliness checks.
  - PublishResult, PreviewResult, /items/mine (states), /items/versions, import head, counter-notice (202), appeal, report, patch and delete shapes all parse as the client expects.
  - Formatting/stem refusals (key_in_formatting, key_in_stem) arrive with `ack_allowed` flags the client uses correctly.
- Static: RefusalKind::classify, retry logic, cursors (match Worker `^\d{1,16}:[0-9a-f]{32}$`), CORS, secrets handling, local-data flows, offline gates.

Not run: `cd server/worker && npm test` (477 expected, ~4 min). Not run in this pass; the harness replays above exercise the same code.

## (b) BLOCKERS

### B1. Key-leak report is filed as `other`, so the Worker's key-leak machinery never fires

- Client: `ReportChoice::KeyLeak` leads to `report_key_leak` (src/share/pool.rs ~2108-2188; called from src/share/browse.rs:514; UI text src/share/browse_text.rs:105-127). It sends `POST /items/:id/report {"reason":"other","note":"Answer key visible: ..."}`. The `ReportReason` enum has no key-leak variant, and its comment says "the Worker has no such reason". That is false.
- Worker: `REPORT_REASONS` includes `"key_leak"` (server/worker/src/pool/moderation.ts:60); the dedicated key-leak machinery is at moderation.ts ~258-325.
- Replay: two qualifying importers each report `other` with the prefix. Both get 202, but the result is `holds after client-style x2: []  flags: []`. The same two reports as `key_leak` give `holds after key_leak x2: [{"reason":"reports","source":"key_leak"}]  flags: [{"kind":"key_leak"}]`, and the author's /items/mine state becomes `key_leak_review`.
- Effect: an importer who reports a leaked answer key through the app's own flow gets a success message, but the item is never held, flagged or reviewed. The leaked-key protection for hidden-key items is dead for the only client.
- Fix: add `ReportReason::KeyLeak => "key_leak"` and offer it only for `hide_key` items. Also handle the Worker's replies: 400 `invalid_request` field=reason (key_leak on a non-hidden-key item) and 429 `daily_limit` metric `key_leak_reports` (one per item per 30 days, 5/day) as plain messages. Correct the stale comment.

## (c) Non-blockers (terse)

1. Republish loses identity: the client never writes `published.qid` (model has no field), so every republish gets all-new qids. Stripped Mc/Matching/Multipart choice order is also reshuffled per publish, so the served kind JSON differs even when nothing changed. Replay: v1 vs v2 qids all differ. Result: an importer's merge cannot match by qid or fingerprint (FNV of stem + kind JSON); updated questions show as "removed in pool" plus "new in pool", and a default merge duplicates them. `/items/versions` reports "newer" for an unchanged republish. No data loss (merge runs on a copy with a restore point).
2. Re-sharing an imported keyless copy: Mc/Tf/Blank/Short/Multipart publish 201 with hide_key. Matching is refused 422 `invalid_payload` `blank_text` at `bank.questions[i].kind.Matching.pairs[j].right`, even with hide_key=false, because the right sides are empty and the sentinel copy carries no `keyless:true`. The teacher must re-key first (deferred model patch).
3. Legal links: the client builds `https://<pool api host>/legal/<page>.html` (src/share/mine_text.rs ~155-160), but the legal pages are deployed on the Pages site (server/web/legal/README.md). The Worker answers 404 JSON for `/legal`, so the links are dead, including the DMCA-related page. Fix before launch.
4. Web CORS (server/auth/src/http.ts corsHeaders): Allow-Headers lacks `If-None-Match`, and Expose-Headers lists only `Retry-After`, so ETag is not exposed. The web client's ETag cache is inert (never breaks anything).
5. Worker codes shown only generically ("The server refused the request (code)"): pool_key_mismatch, too_many_units, author_records_exceeded, length_required, unsupported_media_type, invalid_json, body_timeout.
6. Stale comment in pool.rs ("There is no dry-run route"), although `/items/preview` exists. `ImageRefused` message is generic.
7. Call retry: DELETE/undelete retried on 500 or timeout can show failure after success; import is retried on 5xx (harmless, read-like).
8. `derive(Debug)` on `PublishRequest` (payload with keys) and `pool::CounterNotice` (PII). Nothing formats them, and no secret was found in logs; consider a manual Debug anyway.
9. Honest-key notes: `share-imported.json` base snapshots hold only pool-served (keyless for hidden-key items) content; teacher-typed keys are never copied into the base. `share-published.json` holds ids and title only. Tokens live only in the OS keychain. Both JSON files are plain files with default permissions, same as banks.json.
10. The client uses share-core only as a dev-dependency (tests), so there is no runtime hash-key or pkg version coupling on the client side. Preview is server-side.

Checked and OK: routes, methods and content types; `framed` body and exact Content-Length; schema_version rules (new=1, republish=2); publish_id format (32 hex); attest/attestation fields; visibility and base_publish_id placement; labels/order_acks/image_ok/ack_formatting shapes; Retry-After; pagination cursors; /items/mine states; counter-notice body; export parts; wasm parity; no startup I/O; pool windows closed until clicked; signed-out and unconfigured gates sit only inside pool windows; the unsaved-exam share is disabled; import appends or creates a new bank; merge works on a copy with a restore point saved first.

## (d) Not run / limits

- UI never viewed on screen (no on-screen check of any pool window, wording or layout).
- No real server, staging or network; Worker exercised only through the in-process test harness.
- Web build (wasm32 target) only type-checked, not run in a browser; the CORS findings are from reading.
- `npm test` for server/worker not run (see (a)).
- Unverified: whether a bank open in an editor can be overwritten by a concurrent import.
- BlobMissing resend path and the exact UI gating of the key-leak option for non-hidden-key items were not exercised.

BLOCKERS: 1
