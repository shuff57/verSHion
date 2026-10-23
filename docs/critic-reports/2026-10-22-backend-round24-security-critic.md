# ASVS critic — verSHion shared pool, ROUND 24

Read-only. Scratch probes (not repo edits) are in
`/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/critic-asvs-24/`
(`upstream.test.ts`, `shield.test.ts`, `shield-read2/3`, `shield-ctrl0`, `mig.test.ts`, test logs/typecheck logs).

## (a) Findings

### BLOCKER 1 — The author's own original (the item the taken-down copy came FROM) is DMCA-held on its SECOND ordinary republish
Severity: High (wrongly held honest teacher; round 24's V2 fix is incomplete).
Where: `/home/shuff57/Documents/GitHub/verSHion/server/worker/src/pool/publish.ts:2439-2444` (re-upload block insert), helper `/home/shuff57/Documents/GitHub/verSHion/server/worker/src/pool/served.ts:83-96` (`upstreamOf`).
Mechanism: the round-24 upstream exemption is ANDed with "the *immediately previous* publish (`baseOf`, `?5`) carried the same key and `bp.committed_at <= bh.at`". The first republish after the takedown predates the block and is exempt; after that republish, `baseOf.committed_at > bh.at`, so the next publish is treated as a re-upload and gets an `item_holds` row (`reason=blocklist`, `source=dmca:<action>`) — even though `upstreamOf()` is true. The item disappears from all non-author reads (`VISIBLE_TO_OTHERS`).
Minimal repro (`upstream.test.ts`): Ann publishes Q; Bob imports and republishes Q; admin takes Bob's copy down with `questions:[0]` (creates 2 block hashes). Ann republishes her own item → 200, no holds; Ann republishes again → 200 and `item_holds = [{source:"dmca:…", reason:"blocklist"}]`. The hold is not cleared by later republishes while the key is still used.
Why the suite misses it: `test/pool/round24.test.ts:156` takes the copy down with `questions:"all"`. For a *copied* question that selects nothing (`blocking.ts:278` uses `byDefault`, which requires `!copied`; `sel.all` only affects ghosts), so test 24-4's "no re-upload hold" assertions pass with **zero** blocked hashes and never exercise the exemption.
Fix direction: for an `upstreamOf` item (and `rightAuthor`), do not require the base publish to predate the block; the derivation, not the publish timestamp, is the evidence.

### BLOCKER 2 — Upstream by import/read is forgeable: the taken-down item's author can shield any copy from hold-copies with one GET
Severity: High (launder/hold bypass of the copyright remedy).
Where: `/home/shuff57/Documents/GitHub/verSHion/server/worker/src/pool/moderation.ts:790-798` (`lineageCopies` exclusion), `/home/shuff57/Documents/GitHub/verSHion/server/worker/src/pool/served.ts:89-91` (`upstreamOf` branch 2), used again at `publish.ts:2440-2442` and `moderation.ts:1589-1590`.
Mechanism: branch 2 says "the taken-down item's author has an `imports` row for the candidate item". There is no `kind` filter (`recordServed` writes plain GET reads into `imports` with `kind='view'`) and no precedence/derivation check. So the author of the infringing item T can simply **read** any copy V of T (or import it); T's takedown then excludes V from `lineageCopies`, so the admin's preview does not list it and hold-copies holds nothing. A third party's copy can be shielded without that copier's cooperation.
Minimal repro (`shield.test.ts` control + `shield-read2/3`):
- Control: Ann publishes T; Bob imports T and publishes V; admin takes T down (`questions:"all"` or `[0]`); hold-copies → `held:[V]`.
- Attack: same, plus Ann performs one `GET /items/V` (200). hold-copies → `{"ok":true,"held":[],"more":false}`; no hold rows. With `questions:[0]` (real block hashes) the result is identical.
Fix direction: branch 2 must require a real import (`kind='import'`) **and** evidence of precedence (import/`first_at`, or V's first publish, before T's first publish), or drop branch 2 and rely on the validated origin/chain branches. The same restriction must go into `lineageCopies` and `recheckBlocks`.

### BLOCKER 3 — Migration 0105 does not re-key licence history for content that is no longer retained, so a public relicence stops reaching earlier readers
Severity: Medium (data loss + wrongly refused honest copier; upgrade-time only).
Where: `/home/shuff57/Documents/GitHub/verSHion/server/worker/migrations/0105_pool_round24.sql:23-39` (content_licences) and `:41-58` (item_offer). The re-key joins only `item_publishes` status `current/previous` and `item_questions` (and requires `near_* IS NOT NULL`); it never joins `ghost_questions`. A row whose exact content survives only in a retired version/ghost, or on an item whose near hashes are still NULL (`fold_rev=0`, awaiting the cron backfill — the backfill recomputes near hashes but never re-keys these rows), is left at `q:<exact hash>` forever. The new views compare `'q:' || near_full/near_stripped`, so the window becomes unreachable.
Repro (`mig.test.ts`): Ann v1 NC (exact hash E); Mal imports v1. Ann republishes the *identical* content as v2 under CC0. Two further publishes retire v1/v2 so only v3/v4 are retained. Rewrite the two v1/v2 `content_licences`/`item_offer` rows to the pre-round-24 form `q:<E>` and execute the 0105 statements verbatim.
- Pre-upgrade (0104 exact-content view formula): Mal's grant = `["CC-BY-NC-4.0","CC0-1.0"]`.
- After 0105: the view grants `["CC-BY-NC-4.0"]` only; Mal's copy of the exact content he was served is refused `422 license_incompatible`.
The same 0105 re-key also `INSERT OR IGNORE`s multiple exact rows into one canonical key with an arbitrary survivor (window/`until_at` loss for a repeated same-licence window); the hourly `offerReconcile` only restores the *current* licence.
Fix direction: add ghost-question re-keying (`full_content`/`stripped_content` = `substr(ckey,3)`), re-run the re-key after the backfill computes near hashes, and handle/flag rows that cannot be re-keyed instead of silently orphaning them.

## (b) Clean results (what I ran)

- `cd server/worker && npm test` → **450/450 pass, 0 fail** (exit 0) — includes `test/pool/round24.test.ts`.
- `cd server/auth && npm test` → **224/224 pass, 0 fail**, 18 files (exit 0).
- `cd server/worker && npm run typecheck` → exit 0, 0 errors.
- `cd server/auth && npm run typecheck` → exit 0, 0 errors.

Manual checks that came back clean:
- Routing/authz: all id-taking pool routes bind `visibleTo`/`VISIBLE_TO_OTHERS`; mutations bind `author_id`; pending accounts only reach `/me/export`; admin pool routes are mounted through `src/index.ts:139` behind auth's `extra()` guard (`auth/src/admin.ts:760-773`: admin trust + known device + sudo for writes). No non-admin path to an admin route found.
- Export data minimisation, part by part: profile = own; items/versions/bodies = `author_id = me` only; imports show a title only when `VISIBLE_TO_OTHERS` (unlisted/held/removed/suspended → null); reports = own; takedowns = own actions; `notices` filters `pool_outbox` on `$.authorId`, and the only outbox payload carrying `authorId` is `takedown_author` (no complainant contact); no hashes, sketches, admin notes or other users' identities. `bk_expl_full` is not exported.
- Answer-key boundary: hidden-key publishes return only stripped-form hashes/bodies; `mayRepublishAs`/`questionsFor` use stripped columns for non-authors; blobs gated by served form; the new `author_stems.bk_expl_full` is a keyed HMAC queried only with `s.author_id = <publisher>` and appears in no read/admin/export path.
- Fail-closed: `assertPoolKey` before every non-GET/HEAD pool write and every admin write and cron; `POST /items/versions` is metadata-only by design. `reserveBytes`, `takeDaily`/`requireDaily`, and the export body limits refuse rather than skip. No new silently-skipped cap found.
- Injection/input: all new SQL is parameterised; the only new interpolation (`upstreamOf`, `NOW_UNIT`, `OFFERED`) takes internal identifiers/column refs. `cleanText` keeps CRLF/bidi out of mail/admin fields; mail carries ids only.

## Non-blockers / variants (not counted as blockers)

1. **Held-publish charge omits the new upstream exemption** (`publish.ts:2515-2529`): an upstream original's matching republish is not held by the block SQL but is still charged `held_publishes`; at the documented `HELD_PUBLISHES_PER_DAY = 20` cap the `probe_cap` admin hold (`publish.ts:2536`) can then hold the original. Code-read only (I did not drive 20 matching republishes); variant of the documented cap.
2. **Round-24 test 24-4 is vacuous** for the re-upload path (`round24.test.ts:156`, `blocking.ts:237,278`): `questions:"all"` selects no position on a copied live item, so no block hashes exist and the "not held" assertion is trivially true. This is why Blocker 1 is not caught; add a `questions:[0]` case and a second republish.
3. **Durable author_stems conflict key collapses two hidden explanations** (`publish.ts:2337-2344`, `provenance.ts:967-971`): `(author_id, band, qhash, bk_expl, bk_ans)` with `bk_expl=''` for hidden publishes plus `COALESCE(existing, excluded)` keeps only the first `bk_expl_full`; if an author published the same stem hidden with two different explanations and both are purged, a later shown version matching the second may not recognise her earlier publication (residual of the Blocker 1 class). Not reproduced; exotic.
4. **`OFFERED` ignores author standing** (`publish.ts:265`): a suspended (not deleted) author's item still counts for "the source's licence now". Since the grant was public and CC grants are irrevocable, this looks acceptable; noted for completeness.
5. **Export part/whole size accounting counts a retained body once but sends the current body twice** (`export.ts:169,188-210,221-222,335-337`): a "4 MB part"/"8 MB whole" can be roughly double the stored bytes on the wire/memory. Self-export only; `reserveBytes(2×)` still fails closed if truly large.
6. **Migration merge of same-licence windows** (`0105:23-30`): `INSERT OR IGNORE` keeps an arbitrary row when several exact contents map to one canonical key; window loss is possible for a licence no longer current (the hourly reconcile only reopens the current licence).

## (c) Checks I could not run

- workerd/staging or any deployed environment (no deploy allowed): all probes ran in the in-process node:sqlite + real wasm harness.
- A genuine pre-round-24 database snapshot: Blocker 3 was reproduced by rewriting the exact-keyed rows to their documented 0104-0105 representation and executing the 0105 statements; no legacy dump was available.
- The 20/day `held_publishes` cap path for non-blocker 1 (code-read only).
- The builder's mutation drivers and any performance measurements.

BLOCKERS: 3
