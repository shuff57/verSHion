Round 24 Moodle-parity review — verSHion worker (read-only; scratch in /tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/critic-moodle-24/).

BASELINE IMPORTANT: while I was reviewing, the tree was edited to Round 25 (files 12:02–12:05; README Round 25 at line 150). I verified Round 24 on the pre-edit tree, then re-ran every repro on the current tree. Round-25-only findings are labelled.

(a) PREVIOUS FINDINGS
- 31a B-1 (copier credited as earlier): FIXED. s2 4/4 flows no Mal link (chain [], license_check ok); s3 6/6 credits Mal=false; OWN1 16/16; round24 24-1/24-1b pass. Earliest-of-retained-vs-durable and the hidden full-form explanation key both work.
- 31a B-2 (relicence refused after unrelated edit): FIXED at runtime. s8 16/16 zero refusals (shown+hidden; points/difficulty/topics; with/without back-to-NC); s7 12/12 200. Canonical key is points/difficulty/topics/order-free; a Blank accept reorder is still refused (new1, 422).
- 31a V1 (unlisted grants nothing): FIXED for live (LIC1 stay_unlisted 422; 24-3). Residual: pre-24 ghosts keep frozen now_licenses captured without OFFERED (non-blocker).
- 31a V2 (author's own original held): FIXED in Round 24 for the tested paths (s5 6/6; 24-4), but the Round-25 edit regresses the no-ghost uphold path (B-1 below).
- Export title nulling: FIXED (EXP1: null for unlisted and privacy-removed; 24-5).
- Retained-version bodies: FIXED (s6; 24-6; Round 25 excludes the current body from version_bodies and updated the test).
- f pool_key_mismatch: FIXED (f1: wrong key 503, nothing pinned; right key 201/DELETE 200/versions 200; 24-7). Residual one-row sampling (non-blocker).
- Differential: clean on the Round-24 tree — 320 seeds (100–419), 5,026 ops, 0 failures (old 120/265/249/359 cases gone).

(b) NEW-CLASS BLOCKERS (current tree)
B-1 (Round-25-introduced; wrongly held honest author). The upstream-original exemption lapses once the taken-down copy is purged with no serve, because no ghost is left.
Files: src/pool/served.ts:89-96 (upstreamOf = current-publish chain or ghost chain only; Round 25 removed the import branch), src/pool/publish.ts:383 (ghost only WHERE EXISTS served), src/pool/publish.ts:2497-2505 (re-upload hold), src/pool/moderation.ts:1590 and 787-798.
Repro s5probe.ts: Ann publishes NC; Mal imports+publishes; nobody serves Mal's copy; Ann unlists; admin takedown of Mal's copy with questions:[0]; 31 days of cron (upheld → copy purged; no serve ⇒ no ghost); Ann republishes her own item. Observed: hold {reason:"blocklist", source:"dmca:<action>"}; /items/mine "held_for_review"; non-author GET 404; upstreamOf=0; ghost_publishes=0. Round 24 passed this (s5). With a forced block the public original is held too; an unlisted one cannot be named rightful. Fix: keep the taken-down publish's origin/chain on the action (or ghost it without serves) so the derivation survives the uphold.

B-2 (Round-24 migration coverage; false credit). author_stems.bk_expl_full is added but never backfilled, so the B-1 false credit returns for hidden versions first published before the upgrade.
Files: migrations/0105_pool_round24.sql:19 (ALTER only), src/pool/publish.ts:2393-2400 (only a new publish writes it), src/pool/provenance.ts:980 (expl = bk_expl_full || bk_expl).
Repro new5.ts / new5b.ts: Ann hidden v0 with explanation E; simulate pre-round-24 by NULLing bk_expl_full (new5) or deleting the author_stems row (new5b, a pre-round-23 DB); Mal imports and publishes the fixed-stem copy; Ann publishes the same question SHOWN with E in a second bank without republishing item 1. Observed: chain [{author_name:"Mal", earlier_match:true}], license_check "unverified_earlier_match" ("Another author published these questions earlier..."). Control (record intact): chain [], ok. Fix: a bounded backfill computing the explanation key from the retained full R2 bodies.

B-3 (current tree; round-25 re-key batch cannot resume; full suite fails). licenceRekey takes the first 1,000 old-style rows by rowid and deletes only those it maps; unmappable rows are never removed, so once 1,000 sit at the front the stage never reaches a mappable row behind them and never "stops when no old-style row remains".
Files: src/pool/publish.ts:316-345 (batch; LIMITs at 323, 332, 336, 341), src/pool/cron.ts:62 (hourly), :140 (daily).
Repro new7.ts: 1,000 unmappable q: rows, then one mappable exact row; 3 hourly runs → old-style rows stay 1002, the mappable row stays exact, the canonical row never appears.
Independent evidence: the owner's own F1 lint fails on exactly these LIMITs — current `npm test` = 455 tests, 454 pass, 1 fail (not ok 229, test/pool/round20.test.ts:294; publish.ts:323/332/336/341). Round-24 tree was 450/450.

(c) NON-BLOCKERS / RESIDUALS
- mig3.ts: even with a resuming batch, a window whose exact content was a hidden reshuffle publish that was never served and is no longer retained has no content to map (no ghost) → the earlier reader still gets 422 after 0105. Unrecoverable for pre-0105 rows; README:165 "loses nothing" is overstated (document the limit).
- new6.ts: a licence re-opened after an admin hold release appears only on the daily offer_sync (cron.ts:145), not hourly; README:261 says "reconciled hourly". Observed: hourly-only 422, daily 201 (≤24 h wrong refusal).
- new2.ts: an author who deleted+purged her original and re-adds the question as a NEW item is held (no derivation to the taken-down copy) — same V2 family; admin exemption is the path.
- new4.ts: poolKeyVerdict samples one retained publish (shared.ts:993-1012); if that body is missing it answers "unknown" and the isolate accepts any key and never pins, even when other readable bodies would prove a mismatch (wrong-key publish 201). Contradicts README "all unreadable"; operator-error precondition.
- ownFuzzyFirst's durable query has LIMIT FUZZY_PAIRS_MAX (2000) with no cut/overflow marker (provenance.ts:967-974); scale edge, not reproduced.
- README:180 says 0105 adds author_stems.bk_expl_full / bk_ans_full; only bk_expl_full exists (the answer is deliberately not kept).
- Pre-24 ghosts keep now_licenses captured without OFFERED (V1 residual on migrated data; author's own licence choice).
- new3: the Round-24 import/GET upstream forgery is gone (Round 25 fixed it; both the viewed and unviewed accomplice copies are held).

(d) CLEAN RESULTS
- Round-24 tree: npm test 450/450 exit 0 (this run overlapped the 12:02 edit); typecheck exit 0; round24 8/8; s1 3/3; s2/s3/s5/s6/s7/s8/f1 as above; differential 320 seeds/5,026 ops/0 fails.
- Current (Round 25) tree: typecheck exit 0; auth 224/224 exit 0; re-ran s2/s3/s7/s8/f1/mig2/s4 (as above); new1/new3/new6/new7; npm 455 tests/454 pass/1 fail (F1 lint = B-3).
- No repo files edited; all scratch files under the path above.

(e) NOT RUN
- Real Cloudflare workerd/D1/R2 or staging e2e (node harness only).
- The full 320-seed differential on the current tree (Round-24 run + targeted probes only).
- Staging CPU/memory measurement.

BLOCKERS: 3
