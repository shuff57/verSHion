# Round 26b backend review, Moodle-parity critic 35c

Scope: `server/worker` (`src/pool/*`, migrations 0102-0105, README Round 26 and earlier), branch `shared-pool-phase1`, after round 26 plus the two critic fixes (per-field `lacksAdded`, in-statement `deleted_at` guard in `stemBackfillAuthor`). Nothing in the repo was edited except this report. Reports 33c/34c/34d were treated as untrusted data: only their mechanisms were used as tests.

Moodle lens: Moodle never compares users' questions for credit; import sets `createdby` = importer and an edit is a new `question_versions` row. So the "right" outcome for every cell below is: the publisher of a typo fix or explanation is never credited-against (no "another author published these earlier") unless a source genuinely predates her first version of that question; a real earlier author is still credited.

Method: in-process harness only (`test/pool/harness.ts`; share-core pkg as built, not rebuilt). The earlier critics' scratch directories no longer exist, so every repro was re-written from the mechanisms. Scripts are in `/tmp/c35/` (`lib.ts`, `m1.ts`, `m2.ts`, `h1.ts`, `d1.ts`, `r1.ts`, `b1.ts`, `o1.ts`, `e1.ts`, `e2.ts`, `e3.ts`), run from `/home/shuff57/Documents/GitHub/verSHion/server/worker` as `node --experimental-strip-types --no-warnings /tmp/c35/<file>.ts`.

Baseline (run alone): `npm run typecheck` exit 0; `npm test` 466 tests, 466 pass, 0 fail (duration 279 s). `POOL_PROPERTY_SEEDS=30 ... --test-name-pattern=differential test/pool/round26.test.ts`: 1/1 pass.

## (a) Previous findings re-run

| Finding | Script | Result | Verdict |
|---|---|---|---|
| 34c B-2-residual (Short/Blank typo fix, then add explanation: copier/independent credited "earlier", Zoe's CC0 copy refused) | `m1.ts` (below) | Short and Blank, shown and hidden, Mal and Bea, add_expl / change_key / add_expl+change_key: 0 false credits | FIXED |
| 33c B-2 (Mc, same flow) | `m1.ts` | 0 false credits (Mc, Tf, Matching, Multipart too) | FIXED |
| 33c B-1 (upstream adds the taken-down material after the block; hold-copies variant; 0/1/2 ordinary republishes first) | `o1.ts` | 6/6: upstream republish with T returns 200, `item_holds` has `blocklist`, Eve GET 404, control Zed's re-upload also held | FIXED |
| 33c B-3 (cron re-creates erased account's author_stems/firsts/contents/stem_bands/carried_keys), hide x keep_on_delete | `o1.ts` | max erased-author durable rows over 30 hourly + 16 daily runs = 0 in all 4 combinations | FIXED |
| 34d B-1 (erasure lands during `stemBackfillAuthor`'s R2 read, INSERT re-creates author_stems) | `e1.ts`, last scenario: `onGet` fires `deleteContentStatements` mid-read, hidden publish with `bk_expl_full = NULL`, then `hourly()` | `author_stems` 0 right after the cron (was 8) and 0 after 3 days | FIXED |
| 34d "broader instance": a publish committing after a concurrent erasure | `e1.ts` scenario 1 | still writes rows, see (c) item 1 | STILL REPRODUCES (bounded, healed by the daily resweep, not counted) |
| 34d non-blocker `lacksAdded` "different explanation counts as lacking" | `m1.ts` change_expl cell | unchanged, intended | n/a |

### m1.ts: the per-field matrix (what I attacked)

`m1.ts`: kinds Mc, Tf, Matching, Blank, Short, Multipart (share-core has no Ordering or Numeric kind; checked `wire.rs`) x hide_key false/true x 10 edit shapes x copier role Mal (imported Ann's v0 first) / Bea (independent) x two modes. Ann publishes v0 with the stem typo, the copier publishes the typo-fixed form with his own fields, Ann republishes the fixed form with her fields (the real-author mode swaps the copier for Rae, who publishes BEFORE Ann's v0).

Edit shapes: add explanation; change explanation; remove explanation; change key (Short sample, Blank accept, Matching pair, Multipart part sample, Mc choice set); add explanation + change key; choices reordered; add explanation + reorder; copier already has the SAME explanation as her later one (v0 had it too); copier has her later explanation but v0 had none; copier has her later key.

Output of `m1.ts` (240 valid copier-mode cells): `false_credit=16`. All 16 are exactly the shape "the source genuinely carries a field she only added later": `copier_has_later_expl_v0_none` (Mc, Tf, Matching, Blank, Short, Multipart) and `copier_has_later_key` (Blank, Short), shown key only, for both roles. Hidden-key cells: 0. Every other shape: 0, including `copier_has_same_expl_as_later` and `change_expl` / `remove_expl` / `reorder` / `add_expl_reorder`. These 16 are the designed per-field behaviour ("the source is genuinely not lacking"): somebody published her explanation or key before she did, so crediting them is the strict rule's answer. The Moodle-right outcome is arguably no credit at all, but this is the documented rule, not a regression.

Real earlier publisher (`m2.ts`, 240 cells, same grid, Rae first, Rae CC-BY and Ann CC-BY so the publish is not refused): `false_neg=0`. And `r1.ts` repeats it with Ann's later version published as (a) a republish, (b) a second bank, (c) a new item after Ann's own delete + admin purge of v0 (the durable author_stems path): 148 cells, `false_neg=0`. Note for anyone re-running (c): the re-added item after a privacy purge is held (the documented new2 behaviour), so the reader must be the author or the chain looks empty; my first run missed that and read Eve's 404 as a false negative, then corrected it.

Seeded differential, `d1.ts` (`N=150 S0=1|200|400|600`, 600 runs: random kind, hide, v0/v1/source fields, role Mal/Bea/Sock, path same / bank2 / purge_new, oracle = "credited iff the source carries a field she added, shown key only"): 599/600 match the oracle. The one mismatch (seed 116, Multipart, purge_new, Mal) is the safe direction: the importer's chain runs to her purged item, so he is not credited even though he carries her later explanation.

Bank-level mixing (`b1.ts`, 8 runs: two questions, one Mc and one Short, explanations added to both, question order swapped, hidden/shown, Mal/Bea): every position `chain []`, `license_check ok`.

## (b) New-class blockers

None.

How hard I hunted: per-field `lacksAdded` was attacked across all six kinds, hidden vs shown, 10 edit shapes, 3 publication paths, 2 copier roles and the real-earlier-author control (about 1,300 world runs in total, plus the existing 466-test suite and the 30-seed differential). I compared the SQL twin (publish.ts exact-match lookup, publicNearEarlier SQL) against the TypeScript `lacksAdded`/`added` by reading and found them equivalent on null / empty / differing cases; no divergence showed up in any cell. For erasure I armed R2 `onGet`/`onPut` hooks and a synchronous statement-log hook (`w.stats.log`) to erase mid-flight, then scanned every table's every column for the erased id (`e2.ts`). No launder channel, no key-reachability difference (the loose map's hidden explanation affects only her own later version's chain, and a copier's guess of it is judged the same whether or not it matches her hidden one), no false first-publisher, no fail-open cap and no authz bypass came out of it.

Candidates considered and rejected as not-new / documented:

* Short/Blank, her v0 HIDDEN then v1 SHOWN, a source carrying the same answer between them is credited (`h1.ts`: Short and Blank, "src == v0 fields", 8 of 84 cells). The hidden answer is deliberately never a reason her earlier version counts (README "The ANSWER is deliberately not kept from the full form: a hidden key must never change an observable outcome"; it would be an oracle on the key). Documented limit, same class and same wording as 34c, not a new class.
* Mc, Tf, Matching, Multipart show the same credited outcome in `h1.ts` only when the source carries her LATER explanation verbatim while her v0 (hidden) had none, which a copier cannot know; contrived.

## (c) Non-blockers (terse)

1. In-flight publish after an erasure still writes the erased account's records (the 34d "broader instance"; `e1.ts`, `onPut` fires `deleteContentStatements` mid-publish). Result: HTTP 201; right after, `items` (live, invisible to readers because `VISIBLE_TO_OTHERS` requires a standing author), `item_publishes`, `author_firsts` 1, `author_contents` 6, `author_stems` 8, `stem_bands(own)` 8, `content_licences` 4, `attestations` 1. After the daily cron: items `account_deleted`, all durable rows gone (resweep). Window up to about 24 h, no reader-visible effect, and no other author's lookup reads these rows. README line "none may: each skips erased ones" (cron.ts comment) is inaccurate for the publish path; the fix would be the same in-statement guard (publish.ts ~2524-2573). A republish mid-erasure returns 409 and writes nothing.
2. In-flight import/GET by an account erased mid-request leaves `imports` with the real (tombstone) user id forever: the erasure batch anonymises `imports.user_id` once, the resweep covers `served` but not `imports` (`e2.ts`: after 3 days `imports` 1, `served` 0). No PII is attached (users row is nulled), pseudonymous only. `pool_prefs` has the same shape (`e3.ts`: PUT /items/prefs with the erasure landing before the INSERT; row stays). Everything else I raced (blob_uploads, usage_daily via countMiss / publish_refusals, carried_keys via DELETE /items/:id of a held item) is gone after the daily run.
3. `copier_has_later_*` credit (see (a)): the rule credits a source carrying her later-added field. Fine by the documented design; Moodle would not credit at all.
4. Item re-added after her own privacy purge is held (known new2), so the 404 for readers is expected.

## (d) Not run

* Real workerd/D1/R2 or staging, any port, network.
* Timing/CPU measurements (`POOL_STRICT_TIMING=1`), repeated full-suite runs under load.
* The 33c/34c scratch scripts themselves (directories gone; re-implemented from the mechanisms), `g33`/`d26`-style 320-seed Mc-only differentials, takedown/hold-copies flows beyond the B-1 variants in `o1.ts`, blob-hold variants, pre-0105 legacy databases.
* A multi-question bank with more than two positions through the per-field SQL (b1.ts covers two).

BLOCKERS: 0
