# HANDOFF 2026-10-27 (late evening): circles phase D1 — critics 101/102 ran, scoped fixes in, awaiting the re-check

Repo `verSHion`, branch `shared-pool-phase1`, last commit is the round 19–20 checkpoint. This file was rewritten after the round-19 takeover session (`vershion-fd`, `240c6322`) lost its builder to the weekly API limit. The limit was due to reset **2026-10-28 16:00 PT**; the owner lifted that gate early, so the critic pair ran on the evening of 2026-10-27 (see the scoped fix round below).

## The 5 known failures are fixed (2026-10-28)

The owner asked to fix the five `share_core::wasm::publish_prepare_json` failures. Cause: the viSHion path deps in
`Cargo.toml` turn on `serde_json`'s `arbitrary_precision` through feature unification, so `serde_json` hands a number
to `deserialize_any` as a one-entry map (`$serde_json::private::Number`); share-core's `Read::visit_map` skipped it at
a `Scalar` shape, so `points: 1.0` became `Skipped` and the validator said `expected_number`. Fix in
`server/share-core/src/json.rs`: a `Shape::Scalar` map that is that private-number form is read back into `Node::Num`
(the text charged to the budget); inert when the feature is off, so the worker's own build is unchanged. The owner
authorised this one edit outside `src/share/`.

`cargo test --lib`: **2099 passed, 0 failed, 4 ignored** — the five are fixed (later rounds: 2108/0/4). share-core's
own behaviour tests pass; its `critic24b_scans_are_linear` (an absolute <2.0 s bound) fails on this loaded box at ~3.0 s
both with and without this fix, so that one is environmental, not a regression. `cargo build --bins` and the wasm32
check green.

## Confirmed accounts may join circles (owner decision, 2026-10-28)

The owner decided: a **confirmed** (email-verified) account still awaiting its first approval decision may join a
circle with a circle code and act there **by its role** (viewer reads/copies; editor sends/merges; owner
invites/owns). The **public pool stays approval- and invite-gated**, and creating a circle stays behind approval.
Rejected applicants and revoked teachers are refused — their trust is also `pending`, so the gate checks the latest
approval decision (`awaitingFirstDecision`). A confirmed pending account may also upload the images a circle send
needs (`PUT /blobs/<sha256>`); `/items/*` stays gated.

Implementation: `server/worker/src/pool/index.ts` (`pendingCirclePath`, `pendingBlobPath`, `awaitingFirstDecision`,
the invite-gate exemption on circle paths for every account — a circle code, not a pool invite, admits to a circle);
`circles.ts` `IN_STANDING_SQL`/`AUTHOR_OK_SQL` count `pending`; the student-data instant hide still needs an approved
reporter. Client: a pending account sees only the Circles and Settings tabs, may start a circle send (never a public
publish), the linked-bank strip treats it as signed in, and background circle work runs for it; "Create a circle…" is
hidden. Legal pages (index, terms, privacy) updated to separate the public pool from circles — they remain **drafts for
review, not legal advice**.

Tests: worker `npm test` 694/694, `npm run typecheck` clean; Rust `cargo test --lib` 2108 passed, 0 failed, 5 ignored.
The `pending_circles.test.ts` file's TypeScript diagnostics are the IDE's (tests are outside `tsconfig.include`; the
suite runs and typecheck passes).

### Staging round-trip (2026-10-28)

The changed worker is deployed to `vershion-pool-staging` (version `05465e46`, `workers_dev = false`, no public URL).
Against a `wrangler dev --remote` session on the staging bindings (localhost:18793), the round-trip passed: a
throwaway account (confirmed, approved in the staging D1 as teacher with pool access) created a circle and a
3-question bank (v1), made two changes, and sent all-but-one-unticked -> the server answered version 2 with **added 0,
changed 1, removed 0**, and the unticked question is identical to version 1. The test is
`partial_send_staging_round_trip` (ignored; refuses any base but 18793). Cleanup: the D1 rows (account, circle, bank,
versions, qkeys, members) and the four R2 objects (`circles/<cid>/<bid>/<vid>{,.idx}.json`) were deleted and verified;
the remote dev session was stopped. The local rig on 18791 is still running for the demo. `cargo test --lib`: 2108
passed, 0 failed, 5 ignored.

Still unproven: production, real-network behaviour, and partial sends with pictures, additions or removals (the pure
tests cover those; this round-trip covered a changed raw-base question).

D1 rounds 19 and 20 are **committed locally** (the round 19–20 checkpoint, 45 files: 35 under `src/share/` plus this file and the nine critic reports 92-100). **Nothing is pushed.** The rest of the dirty tree belongs to the other sessions (see "Do not touch").

## Where D1 stands

Round 18 (committed in `0d17a83` as WIP) was failed by its critic pair; round 19 was scoped to the two important findings plus cheap minors, with the brief "this is the last scoped round". **All 7 round-19 items are now in the tree, with tests, and pass.** The previous version of this file said item 7 was only partly done; four more pin tests were added after it was written.

| # | Item | State |
|---|------|-------|
| 1 | c100 I2: a save failure that loses her edits warns, survives Dismiss, gives advice | done |
| 2 | c100 I1: Check now says "Checking…" then "Checked just now: still the same" | done |
| 3 | c99 m1/m2: held retry saves the links file first and holds on failure | done |
| 4 | c99 m4/m7/m5/m3: persist held Undo, hold Get latest, one safety copy per model, move sent times | done |
| 5 | c99 m6/m8/m9 | done |
| 6 | c100 minors 1, 2, 5, 7, 9, 10, 12 | done |
| 7 | Pin the mutation gaps | **done**: `s19_m12`, `s19_m17`, `s19_m21`, `s19_m24`, `s19_m61`, `s19_m80` in `bank_links_fix19_tests.rs` |

## Round 20 (2026-10-27): the leftover minors

The owner asked for the leftovers from critic 100 to be worked: **minors 3, 4, 6, 8, 11 and 12 are now done**.

| Minor | Change | Test |
|-------|--------|------|
| 3 | A row with no "Put mine back" says why and what she can do: `Kept` rows say the copy moved on and "set it by hand if you want yours"; `Changed` rows add the same tip | `c100_m3_…` |
| 4 | A put-back row names the circle's value ("…the circle's is Hard"); "Put all of mine back" **asks first** ("Put yours back on N questions?" / "Yes, put all back" / "Keep them") and changes nothing until the yes | `c100_m4_…` ×2 |
| 6 | The button column sits just past the widest line that offers one (was at the far edge, ~700 px away at 1280); a row with no button uses the whole width; at a narrow width the stem is cut at 28 chars, so a row is 3 lines, not 4, and "Easy -> Hard" stays whole | `c100_m6_…` ×3 |
| 8 | "Use my bank as it is" and "Get latest" name the same bank: the circle's title, never her bank's own name on one page and the circle's on the next (`merge_title`) | `c100_m8_…` |
| 11 | The news part of the badge has a short form ("difficulty"); "removed from the circle" is said as removed even when `gone()` is also true; `WAITING_NOTE` is "circle not checked yet", not "waiting for the circle check"; the "..." hint says "circle page", the menu's own words | `c100_m11_…` ×3 |
| 12 | The review-plus-not-kept strip is three sentences (`review_not_kept_line`), not five saying the same thing twice; the held line names verSHion once, not four times; the Account window drops "(UTC)". The rest of minor 12 (no "merge", no "REPLACES", no "copy that file over banks.json", no "redeemed") was already pinned by round 19's `c100_m12_leftover_words` | `c100_m12_…` ×3 |

New file: `src/share/bank_links_fix20_tests.rs` (13 tests). Five existing tests changed **deliberately**, each because round 20 reverses what it pinned: `fix18` `c98_m11` (as-is now names the circle's title), `c98_i2` (the news part is now shortened), `c98_m6` (Put all now asks), `fix15`/`fix17`/`fix18` waiting-note assertions (now the constant), `fix18` hint wording.

**Round 20's nine behavioural tests are mutation-checked, 9 of 9 killed** (scratch tree `/tmp/d1mut`, own target `/tmp/d1mut-target`, script `/tmp/d1mut-scripts/mut20.py`, logs `/tmp/d1mut20-*.log`):

| Mutant | What it breaks | Test that failed |
|--------|----------------|------------------|
| ma | "Put all" acts on the first press without asking | `c100_m4_put_all_…` |
| mb | a row with no button is held to the button column | `c100_m6_a_row_with_no_button_…` |
| mc | the stem is always cut at 60, whatever the width | `c100_m6_a_narrow_…` |
| md | `gone()` is asked before `seen.removed` (removed said as "left") | `c100_m11_removed_…` |
| me | a put-back row drops the circle's value | `c100_m4_a_put_back_…` |
| mf | a row whose button is gone gives no reason or tip | `c100_m3_…` |
| mr1 | the review line goes back to the five-sentence wall | `c100_m12_the_review_…` |
| mr2 | "verSHion tries again every minute" comes back (four names in the strip) | `c100_m12_the_held_…` |
| mr3 | the Account window says "(UTC)" again | `c100_m12_the_account_…` |

## Scoped fix round, critics 101/102 (2026-10-27 late evening)

The owner lifted the agent-quota gate early, so the pair ran on 2026-10-27 evening rather than after 2026-10-28 16:00 PT. Both were read-only on `a02c12d`, exactly per `docs/critic-reports/2026-10-28-round101-102-brief.md`. Both returned **important problem(s)**; per the brief the lead stopped and told the owner, and the owner then authorised one scoped fix round.

Reports: `docs/critic-reports/2026-10-27-circles-phaseD1-critic101.md` and `-critic102.md`. Five fixes, each with a test in `src/share/bank_links_fix21_tests.rs` (5 tests), each mutation-checked (5 of 5 killed):

| # | Finding | Fix |
|---|---------|-----|
| 1 | c101 I1: `finish_merge` wrote the links file before the merged bank, through an unchecked caller save. A failed banks write left the link advanced, the bank stale and her merge choices lost | the model writes all banks, **checked**, before `replace_link`; on failure the bank is restored and the page says nothing was changed, with Apply changes again. `banks_saved()` clears a stale "edits aren't saved" warning after the write |
| 2 | c102 I1: an under-review head returned before the `Learning::Held` arm, so a failed save was drawn as a calm "You can keep working here" with no Try again, while the badge said "couldn't save" | the `held` warning is applied after the state is chosen (as round 19 did for `unsaved`): where the line would mislead it is replaced, elsewhere the warning is added after it; warning colour, Try again first, a still-useful action (sign in) kept. The normal `Held` arm is untouched |
| 3 | c101 m1: "Put all of mine back" emptied `inherited_before`, so a restart re-ran the pass over her put-back | the round-19 guard now fires on `ran` (the pass's banks write is known to have happened). A held pass drops `pass_times`, so only both files unwritable at once could mask it |
| 4 | c102 m1: "Dismiss hides this list", but Dismiss deletes it for good | `NEWS_DISMISS` says it removes the list for good, with the offer to put hers back |
| 5 | c102 m2: the badge's "newer version" hover promised "Get latest … brings it in" where the page cannot offer it | the hover says the bank's page says what she can do |

One existing test changed **deliberately**: `bank_links_fix18_tests::c98_m6_the_news_list_layout` pinned the old Dismiss words; it now pins the new ones.

### Verification (2026-10-27 late evening, live tree)

- `cargo test --lib`: **2007 passed, 5 failed, 3 ignored** (2001 before + the 6 new; the 5 are the known share-core/vishion ones).
- `cargo build --bins`: EXIT 0. `cargo check --lib --target wasm32-unknown-unknown`: EXIT 0 (23 pre-existing warnings).
- Mutation: scratch `/tmp/d1mut21` (HEAD + live `src/share/`, own target `/tmp/d1mut21-target`, script `/tmp/mut21.py`). m0 pristine: 6/6 pass. m1–m6 each fail their pinning test. Logs `/tmp/mut21-m*.log`.

### Re-check of the fix diff (focused, same evening)

One critic re-checked only `git diff a02c12d -- src/share/`: **fixes hold**, no blocker or important, two minors — both fixed in the same round. Report: `2026-10-27-circles-phaseD1-recheck.md`.

- the held warning replaced every state's line, losing a removal's bring-back deadline and the sent-unread explanation and pushing History where there is no menu for it: it now replaces only where the line would mislead ("You can keep working here", "works as usual") and appends elsewhere;
- `pass_times` alone did not prove the pass's banks write: `ran` now also requires `bank.modified >= after`, so a crash or a failed safety copy cannot record a pass that never wrote the banks. New test `c101_m1_a_pass_times_write_…`; six mutants killed (m1–m6).

### Screenshots (2026-10-27 late evening, `/tmp/d1shots/`)

- `news21-{light,dark}-{1280,760,420}.png` — the news list with the new Dismiss words. `news21-dark-420.png` replaces the stale `news20-dark-420.png` (which still showed the 60-char stem from before round 20); the new dark frame shows the 28-char stem and three-line rows, matching light.
- `held21-{light,dark}-{1280,760,420}.png` — the Held save failure drawn over the signed-out strip: red panel, "your own edits aren't saved either", Try again first, the sign-in button kept. Fixtures rebuilt by `/tmp/fixture21.py` from the news fixture (difficulty pass pending, `banks.json.tmp` a directory). Recaptured after the re-check narrowing: the signed-out state keeps its own line and the warning follows it.
- **Not captured**: the under-review variant of that strip (needs a signed-in check, i.e. a live read) and the merge save-failure page (a merge needs a circle read). Both are pinned by `c102_i1_…` and `c101_i1_…`.

## Verification of rounds 19-20 (before the scoped fix round)

- `cargo test --lib`: **2001 passed, 5 failed, 3 ignored** (2009 total). Log: `/tmp/d1-m12-final.log`.
- `cargo build --bins`: EXIT 0.
- `cargo check --lib --target wasm32-unknown-unknown`: EXIT 0 (23 warnings, pre-existing).

The 5 failures are **not round 19's**: `bank_links_fix10_tests` ×3, `fix11` ×1, `fix12` ×1. They call `share_core::wasm::publish_prepare_json` and get `{"errors":[{"code":"expected_number","path":"bank.questions[0].points"}]}` on `"points": 1.0`. Cause: the **figures session** added `vishion-core` / `vishion-net` path deps to `Cargo.toml`; their `serde_json` features (`arbitrary_precision`, `preserve_order`) change how share-core's `Node::Num` sees `1.0`, so `as_f64()` returns `None`. Proven not D1's: in a HEAD-only scratch tree (`git archive HEAD` + live `src/share/`) those tests pass. `server/share-core` is untouched and its own suite passes. **The owner has ruled this belongs to the other session; do not edit share-core or the Cargo files as part of D1.**

## Mutation checks on the six new pin tests (2026-10-27 evening)

Method: `git archive HEAD | tar -x -C /tmp/d1mut`, `cp -a src/share/. /tmp/d1mut/src/share/`, `CARGO_TARGET_DIR=/tmp/d1mut-target` (own target, so the shared build lock is untouched), one mutant at a time with the pristine files restored after each.

| Mutant | Where | Result |
|--------|-------|--------|
| m12 | `circle_edit.rs` success path: drop `add_news(l, version, &already);` | **killed** |
| m17 | `(have.len() > known).then_some(have)` → `{ let _ = known; Some(have) }` | survives — **equivalent**, as documented in the test's doc comment (the caller only waits for questions the bank still has) |
| m21 | drop `!r.by.is_empty() &&` from the send-record guard | **killed** |
| m61 | `offered >= 2` → `offered >= 1` for "Put all of mine back" | **killed** |
| m80 | drop `&& !l.as_is` from the partial-count line | **killed** |
| m24 | `settle_all_news`: do not call `self.save()` | **killed** (re-confirmed) |

Scratch tree and logs: `/tmp/d1mut`, `/tmp/d1mut-<mutant>.log`, summary `/tmp/d1mut-summary.log`.

## Screenshots (2026-10-27 evening)

Twelve real-app frames, taken with the QA hooks: `VERSHION_STORE_DIR` (a fixture store), `VERSHION_SCREEN=bank`, `VERSHION_WINSIZE`, `VERSHION_SCRIPT` (`tap:x,y` in egui points), `VERSHION_SHOT`, `VERSHION_SHOT_FRAME=200`, under `xvfb-run`. Window scale is 1.7, so a 1280-point window shoots 2176 px wide; 420 points does work despite `min_inner_size([760,520])`.

In `/tmp/d1shots/` (11 MB, survives until the box reboots):

- `news-{light,dark}-{1280,760,420}.png` — the difficulty news list: three rows, per-row "Put mine back", "Put all of mine back", Dismiss, signed-out strip.
- `news20-{light,dark}-{1280,420}.png` — the same after round 20: the button column beside the rows, and the 420 view with the stem cut at 28 chars.
- `putall20-light-1280.png` — the "Put all of mine back" question (round 20): "Put yours back on 3 questions?…", "Yes, put all back" / "Keep them".
- `warn-{light,dark}-{1280,760,420}.png` — the round-19 I2 warning strip: red panel, "Your difficulty couldn't be put back, and your own edits aren't saved either…", Try again / Sign in… / "…", the news list kept beneath it.

The save failure was produced without code changes by making the banks write fail: `mkdir <store>/banks.json.tmp` (the backend writes `<name>.tmp` then renames, so this fails only `banks.json`). `share-circles.json.tmp` as a directory fails only the links file and gives the other path (m2), also captured once.

**Not captured: the Check-now success feedback** ("Checking…" then "Checked just now: still the same."). It needs a live signed-in circle read; the app is signed out in a fixture store and cannot reach that state offline. The wording and geometry are pinned by `c100_i1_check_now_*` in `bank_links_fix19_tests.rs`. If the owner wants a picture of it, that is a staging run and needs their yes.

Fixtures were made by a temporary `#[ignore]` test appended to `bank_links_fix19_tests.rs` (it ran `many_news(3)` from `bank_links_fix18_tests` and wrote `banks.json`, `share-circles.json`, `settings.json`). It has been removed; re-add it if the shots need regenerating.

## Phase D3 (partial send), piece 1 (2026-10-27 late evening)

The owner chose **D3 before D2**. D2's backend route stays with the db/sharing session; `server/` is untouched.

**Piece 1, committed: the pure composer.** `src/share/partial_send.rs`:
`compose_partial(link, bank, base_body, changed, ticked)` returns the send payload. `changed` is authoritative,
computed by the caller's summary against the base body over every field the server hashes (so a change to `figure`
counts); the composer never calls `merge::ours_changed`. An unticked changed question is sent as the base's raw
JSON plus a `published` block; an unticked addition is omitted; an unticked removal is restored after its base
predecessor, read from `link.qids` (the full pairs survive `bounded` emptying `link.base`). 18 tests in
`partial_send_tests.rs`; four mutants killed in `/tmp/d1mut21` (p1 removals from `link.base`, p2 ignore `changed`,
p3 anchor at the front, p4 send an unticked addition). `cargo test --lib`: 2026 passed, 5 known failed, 3 ignored.

Critic round: 2 blockers + 1 important, all fixed before the commit — a figure-only change sent unticked (the
snapshot cannot see `figure`; the server hashes it, `share-core/src/hash.rs:394`), removals missed on a bounded
link, and the same wrong predicate for an unknown difficulty. The composer now trusts the caller's change set, so
the body-based summary is the next piece and must include `figure` and the difficulty.

**Piece 2, committed: the body-based change list.** `changes_against_body(link, bank, base_body)` returns
`changed` / `added` / `removed`, comparing every field the server hashes: `merge::same_question` (stem, choices and
key, explanation, points, topics, difficulty, figure) plus the picture `{file,width,height}` and the keyless
placeholder. Removals come from `link.qids`, so a bounded link still lists them. `base_index` is shared with the
composer. 31 tests in the module; six mutants killed (figure ignored, difficulty ignored, picture ignored,
removals from `link.base`, the re-added strict check, the empty-bank refusal).

Piece-2 critic: **holds**, two minors, both fixed — a removal the base body lacks is now still listed (only an
unticked one makes the composer refuse), and an empty bank is refused with the composer's own words so the summary
cannot offer a send that can never compose.

**Piece 3, committed: the bookkeeping.** `CircleLink.held_back` (serde default, lenient reader; cleared by a
whole-bank send); `ComposedQuestion` / `compose_partial_parts` keep each composed question's local id (a restored
removal's raw carries the circle qid as its `id`); `after_partial_send` sets the base and pairs from the COMPOSED
questions (a ticked removal's pair dropped, an unticked one kept), stores `held_back`, marks the send-start bank
(so an edit during the send is not stale and shows as unsent), and clears the inherit state. A composed question's
snapshot is optional: a restored question this build cannot read still composes and keeps its pair, with no base
entry (the count then says not fully known). The server's returned qid wins where the answer's list aligns; a short
list is ignored whole, so no question borrows another's qid. 42 tests in the module; five mutants killed
(`held_back` not stored, `mark_bank` on the composed bank, `c.qid` over the answer, the length guard, the
unreadable-removal refusal).

Piece-3 critic: one important (an unreadable restored question refused the payload, so the caller's whole-bank
fallback would send the unticked removal) and three minors, all fixed.

**Piece 4, committed: the send summary model.** `PartialSend` (`Reading` / `Ready { base, body, print, changes, ticked }` /
`Unavailable` / `ReadFailed`), one base-version read per page (`Work::SendBase`), all changes ticked except
`link.held_back`, `toggle_change`/`tick_all`, the composed send through the existing path, the keyless check on the
composed questions (refusals mapped back to bank question numbers), and `after_partial_send` with the as-sent bank;
`held_back` = the unticked ids still differing. Critic blockers fixed in the same pass:
- the summary re-checks the bank's print at Send; a bank changed since the body arrived sends nothing, is re-listed
  against the same body, and keeps ticks by local id (new changes start ticked), sending over the body's own
  `base_version`;
- a 409 carries the unticked set across Get latest and the merge (`send_unticked`), so the rebuilt list starts them
  unticked;
- a transient base read is `ReadFailed` with Try again, never "the circle no longer keeps it" and never a silent
  whole-bank send; only 404/withheld does that;
- an unread answer (version 0) keeps the selection in `RetrySend` and reuses the commit id;
- send-on-top goes as the whole bank instead of composing against another base.
57 partial-send tests (15 page tests); full `cargo test --lib`: 2065 passed, 5 known failed, 3 ignored. The send-test
harness now answers an unscripted version read with 404 (keeps the ~130 whole-bank tests on today's path); two tests
that used the lost-job branch now script 503.

Deferred: the missing-picture fetch for an unticked question; narrowing the harness 404 fallback (the `JobKind::Base`
disconnected branch lost its test); decision/label refusal positions (keyless is mapped).

**Piece 5, committed: the visible summary.** The Ready page draws the plan 4.7 heading, the intro, one row per change
(kind, the bank's question number, the stem cut to one line at 1280 and a second line at 420), a tick box whose
accessible name carries the question and stem, `N of M ticked`, Tick all/Untick all, and `Send N change(s)` — disabled
with nothing ticked, with "Tick at least one change". Reading shows the spinner and waits; ReadFailed shows its stored
reason (session ended, daily limit, network) with Try again; Unavailable shows the state's line with **Get latest** and
Send everything. The notice is drawn above the rows; an in-step copy says there is nothing to send and has no Send;
after a partial send the result says "Sent version N with X of your Y changes. Z stay on this computer, not sent."
72 partial-send tests; full `cargo test --lib`: 2080 passed, 5 known failed, 3 ignored. No screenshots: the Ready state
needs a live circle read (the local rig on 18791 now makes it reachable; no GUI automation was run).

**Piece 6, committed: the picture fetch.** A partial send whose unticked, base-raw question names a picture not on this
computer fetches it from the circle with `circle_blob` (hash checked) and uses the fetched bytes under the base's own
file name (`FetchedMedia`, each picture moved out on its one read), so the question hashes as unchanged; the store
keeps a local copy best-effort. A failed fetch sends nothing, and the page's "Try again" re-composes and re-fetches.
`pool::media_sha` is shared with the uploader so the name rule cannot diverge. 82 partial-send tests; full
`cargo test --lib`: 2090 passed, 5 known failed, 3 ignored. A missing picture on the person's own question keeps
today's refusal and fetches nothing; whole-bank sends fetch nothing; the blob GET costs no item/change read.

**Minor round, committed.** The test harness's unscripted-version 404 is now opt-in (`Script::send_base_404`), so
Compare/History version reads are strict again; the lost `JobKind::Base` disconnected branch is pinned by a new test.
`Pictures`/`Decisions` carry a display-only map from payload positions to the bank's question numbers
(`display_question`), while the data and the request keep payload positions — covered for a restored removal before a
question, a picture row and a decision refusal, with the submitted `image_ok`/`labels` still in payload positions.
85 partial-send tests; full `cargo test --lib`: 2094 passed, 5 known failed, 3 ignored.

**Round-trip confirmed on the local rig.** `src/share/partial_send_rig_tests.rs` (ignored; refuses unless the base is
`http://localhost:18791`) signs up a throwaway account with an in-memory random password (never printed or stored),
approves it with the rig script, creates a circle and a 3-question bank (version 1), changes two questions, sends
all-but-one-unticked, and asserts the server answers `version 2, added 0, changed 1, removed 0` with the unticked
question byte-identical to version 1. Run it while the rig is up with
`cargo test --lib partial_send_rig_round_trip -- --ignored --nocapture`. It passed.

This confirms the plan's one unproven assumption locally: the raw base JSON round-trips as unchanged and the server
counts exactly the ticked change. Still unproven: staging/production, real-network behaviour, and partial sends with
pictures, additions or removals (the pure D3 tests cover those; this round-trip does not). Full
`cargo test --lib`: 2094 passed, 5 known failed, 4 ignored.

Next: staging (needs the owner's yes); otherwise D3 is done.

## What is left (in order)

1. **D1 is closed** (commits `fb1b7e9`, `cc49eed`; Tidy Undo `7956fe7`). **D3 piece 1 is committed**; the next piece is the tick-list model and send summary, which computes `changed` against the base body (including `figure` and the difficulty). Then `after_partial_send`, the send page, pictures, and the staging round-trip (owner's yes).
2. Local commits only; never push. See memory `circles-loop-authorisation`.

## Decisions taken by the owner (2026-10-27)

- **Keep "Check now" as the big button** when the circle's version can't be read. Round 19's visible feedback stands.
- **The `vishion-core`/`vishion-net` dep and the 5 share-core test failures belong to the other session** (the figures one). D1 does not touch them; a short bug note with the evidence is enough.
- Two other sessions have the tree dirty: one on figures (it added the vishion deps), one on the db/sharing work.

## Do not touch (other sessions' uncommitted work)

`Cargo.toml`, `Cargo.lock`, `src/ai.rs`, `src/app.rs`, `src/screens.rs`, `src/store.rs`, `src/lib.rs`, `src/model.rs`, `src/figures/`, `src/gradeui.rs`, `src/figuresui.rs`, `src/tidy.rs`, `src/bankui.rs`, `server/`. Final D1 edits go only to live `src/share/`. Naming rules from the brief: "difficulty", never "ratings" or "pool"; "safety copy" is one word.

## Pitfalls

- **Shared cargo target dir.** Other sessions hold the build lock; a `cargo test` can sit at "Blocking waiting for file lock" for minutes. This is not a hang. Use an own `CARGO_TARGET_DIR` for scratch work, as above.
- A full `cargo test --lib` takes about 84 s warm (150 s cold) once it has the lock.
- **Backgrounded `cargo ... | tail` prints nothing until exit.** Redirect to a file and read that.
- Task `.output` files under `/tmp/claude-1000/.../tasks/` vanish when a session restarts, and the per-session scratchpad dirs went with them. Use `/tmp/<name>.log` for long output.
- `xvfb-run` screenshots need the app built first (`target/debug/vershion`); the window scale is 1.7, so divide screenshot pixels by 1.7 to get the egui points a `VERSHION_SCRIPT` click expects.
- `docs/critic-reports/` has many untracked files from this loop (92 to 100, plus the `grade-p*` files from the other session). Do not delete them.

## Related

- `docs/handoff-shared-pool.md` is the standing project handoff (what is being built, standing rules).
- Memory: `circles-phaseD-decisions`, `circles-loop-authorisation`, `shared-pool-accounts`.
- Round 18 condensed reports: `docs/critic-reports/2026-10-27-circles-phaseD1-critic99.md` and `-critic100.md`.

## First commands

```
cd ~/Documents/GitHub/verSHion
git status --short src/share            # nothing: D1 is committed
cargo test --lib                        # expect 2001 pass, 5 fail (the share-core ones), 3 ignored
```
