# Round 26 backend review, Moodle-parity critic 34c

**Scope.** `server/worker` (TypeScript Worker, D1, R2; `src/pool/*`, migrations 0102–0105, README Round 26 and earlier), compared with Moodle's question bank where relevant. Branch `shared-pool-phase1`.

**How it was run.** All runs were in the in-process harness (`test/pool/harness.ts`, D1 and R2 emulated in memory). The share-core `pkg` built at 13:05 was used as-is and not rebuilt. No repo file was edited apart from this report. Test runs used `node --experimental-strip-types --no-warnings`; node:test files used `--test`. Critic 33c's repro directory was copied unchanged into my own scratch before running (its originals were not mutated):

- Scratch: `/tmp/critic34c.7LX9xI/` (contains 33c's `l1*.ts`, `c1.ts`, `c2.ts`, `e1.ts`, `h1.ts`, `u*.ts`, `f*.ts`, `g33.ts`, `r32a/`, `radd/`, plus my probes below).
- My probes: `b2short.ts`, `b2ans.ts`, `b2verbatim.ts`, `b2diff.ts`, `d26.ts` (own differential), `b1diff.ts`, `b1blob.ts`, `b1legacy.ts`, `b1legacy2.ts`, `b3fold.ts`, `radd/mig2b.ts` (33c's `mig2.ts` with the round-26 `CREATE TABLE` line filtered for the re-run).
- 33c's repros were re-run with `run33c.sh` (logs `*.new.out`), and its 320-seed differential with `N=80 S0=<100|180|260|340> node --experimental-strip-types --no-warnings g33.ts` (`g33.new.out`).

**Baseline (run alone, apart from a second unrelated `npm test` from another session on the same machine).**

- `cd /home/shuff57/Documents/GitHub/verSHion/server/worker && npm run typecheck` → exit 0 (`/tmp/critic34c_tsc.out`).
- `npm test` → **464 tests, 464 pass, 0 fail**, exit 0 (`/tmp/critic34c_test.out`).
- A second, independent `npm test` from another session was running concurrently on the machine (load average ~30); my run still completed 464/464 with no timing-assertion failure.

## (a) Previous findings, re-run on the current tree

| Finding | Repro (from the scratch dir) | Result now | Verdict |
|---|---|---|---|
| 32a B-1: copier credited as an earlier publication | `node --experimental-strip-types --no-warnings r32a/s2.test.ts` and `... --test r32a/s1.test.ts r32a/s3.test.ts` | S2: 8/8 `chain []`, `lc ok`; OWN1: 8/8 `pvchainMal=false`; S3: 6/6 `credits Mal=false` | FIXED |
| 32a B-2: more permissive licence refused after an unrelated edit | `... radd/s8.ts`, `... radd/s7.ts` | s8: 0/16 refused (shown+hidden, points/difficulty/topics, with/without back-to-NC); s7: 12/12 → 200 | FIXED |
| 32a V1: unlisted item's licence grants | `... --test r32a/s1.test.ts` (LIC1) | stay_unlisted → 422 with and without the hourly cron; public_after → 200 | FIXED |
| 32a V2 / addendum B-1: author held after a takedown | `... radd/s5.ts`, `... radd/s5probe.ts` | s5 6/6: `holds []`, state live; s5probe: `holds []`, `upstreamOf=1` | FIXED |
| Export showed a private title | `... --test r32a/s1.test.ts` (EXP1/EXP1b) | unlisted and privacy-removed: `"title":null` in both | FIXED |
| pool_key pin | `... radd/f1.ts`, `... radd/new4.ts` | wrong key → 503, `pinned []`, nothing changed; right key afterwards → publish 201, DELETE 200, versions 200; new4 (one body missing) → 503, `pinned []` | FIXED |
| Addendum B-2: `author_stems.bk_expl_full` not backfilled | `... radd/new5.ts`, `... radd/new5b.ts` | both, `simulate_pre24=true` → `chain []`, `lc ok` | FIXED |
| Addendum B-3: re-key batch stall | `... radd/new7.ts` | every run: canonical CC-BY row present=true; the mappable row maps, old rows no longer hide it | FIXED |
| new6: hourly offer_sync | `... radd/new6.ts` | after an hourly-only run: 201 | FIXED |
| 33c B-1: taken-down material laundered through the upstream exemption | `... l1.ts`, `... l1b.ts`, `... b1diff.ts`, `... b1ghost.ts`, `... b1blob.ts` | all 4 variants: Carl's upstream republish with T is 200 and `holds=["blocklist"]`, Eve GET 404, `servesTextbookQ=false`; Zed control held; NEW item with T held; one-account variant still held. Extra controls: ghost takedown (`b1ghost.ts`): purged copy → snapshot 0 rows, the upstream's added T held, Eve 404; carried T → snapshot 2 rows, exempt, Eve 200. Blocked image (`b1blob.ts`): carried-only 200 / no holds; added blocked image 200 / `blocklist`, Eve 404. `b1diff.ts` 24 seeds × (add-now / carried-only): 0 failures. | FIXED |
| 33c B-2: copier credited as "earlier" after a typo fix + explanation (Mc) | `... c1.ts`, `... c2.ts` | c1: 12/12 `chain []`, `lc ok`; c2: 4/4 `chain []`, Zoe 201 | FIXED for Mc — **but STILL REPRODUCES for Short/Blank questions**, see (b) |
| 33c B-3: an erased account's records resurrected | `... e1.ts` | `author_stems` 8 → 0 after erasure, **0 after 1 hourly run and 0 after 16 days**, all four kept/hidden combinations | FIXED |
| 33c V1 (new item after a takedown of a copy) | `... h1.ts` | unchanged: `public_not_named` and `ann_unlisted` → 201 held (`blocklist`); `rightful_named` live | Unchanged documented variant (new2 family) |
| 33c V4: stem-backfill starvation | `test/pool/round26.test.ts` in `npm test` | the test passes (skip after `STEM_BACKFILL_FAILS`, admin flag, Bob's backfill completes) | FIXED |
| new2: delete+purge+re-add as a NEW item is held | `... radd/new2.ts` | unchanged | Documented limit |
| new1: Blank accept reorder still refused | `... radd/new1.ts` | unchanged (422) | Pre-existing documented limit |

Controls from earlier rounds also still hold: `f2.ts` / `f3.ts` (forged origin/chain into one's own item) produce `derivation []` and the accomplice's items are held; `radd/new3.ts` (Round-24 import/GET upstream forgery) holds both the viewed and unviewed copies.

## (b) BLOCKER: 33c's B-2 class is not fixed for Short/Blank questions

### B-2-residual (ran): 33c's B-2 is not fixed for Short/Blank questions — a copier or independent author is still credited as "earlier", and honest readers are refused

This is the same class as 33c B-2 (false first credit + honest reader refused), still reproducing through a hole in the round-26 fix. It is not a wording or scale variant: it fires on the ordinary edit the fix was written for (fix a typo, then write the explanation) whenever the question is a Short or Blank with a model/accepted answer.

**Mechanism.** The round-26 `lacksAdded` gate is an AND (`src/pool/provenance.ts:918-920`):

```ts
export const lacksAdded = (p, expl, ans) =>
  (p.expl == null || p.expl !== expl) && (p.ans == null || p.ans !== ans);
```

It is true only when the candidate source lacks **both** the explanation and the answer the publisher's version has. When the publisher adds one field while the other is unchanged and present in the candidate (a Short/Blank's model or accepted answer), the gate is false, so her own earlier version (`looseFirst`) is not used. The same gate is duplicated in the exact-content earlier-match SQL (`src/pool/publish.ts:1830-1832`, inside the `SELECT` at `:1817-1837`; the resulting chain link is the observed `modified:false, earlier_match:true` at `:1843`) and in `publicNearEarlier` (`src/pool/provenance.ts:1064-1067`). The `loose` map itself is populated correctly (`src/pool/provenance.ts:1001-1003`); only the gate that decides whether to use it is wrong. With the gate false, `first` is the later content's own first publication — here null, since Ann never published the typo-fixed content before — and the SQL admits the candidate (`COALESCE(iq.seq < CAST(NULL AS INTEGER), 1)`), crediting it. The README Round 26 claim ("a candidate source that itself lacks what she added (`lacksAdded`) is judged against it", README:163-170) is therefore false when the addition is one field and the other is an unchanged key.

**Exact inputs (`b2short.ts`; Bea identically in `b2diff.ts`/`b2ans.ts`).**

1. Ann publishes Short v0: stem "…at the mouth of the **Fether** river…" (long stem, so not a short statement), `Short.sample = "A delta lobe of fresh sediment deposited where the channel meets standing water"` (a non-null answer key), no explanation, `CC-BY-4.0`.
2. A day later Mal (served; `b2short.ts`) or Bea (never served; the same flow in `b2diff.ts`/`b2ans.ts`) publishes the obvious typo fix "…**Feather** river…" with the same sample and no explanation, `CC-BY-NC-4.0` → 201.
3. A day later Ann republishes "Feather" + her own explanation E (`CC0-1.0`).

**Exact outputs.**

- Ann's republish: 200. Eve's GET (public):

```
chain=[{"item_id":"db80a1a2…","version":1,"license":"CC-BY-NC-4.0","author_name":"Mal",
        "modified":false,"earlier_match":true}]
license_check={"status":"unverified_earlier_match", … "reason":"Another author published these
               questions earlier; there is no record of this publisher being served them."}
```

- Zoe imports Ann's public CC0 item and copies the same content verbatim under `CC0-1.0`:
  **422 `license_incompatible`**, `allowed ["CC-BY-NC-4.0","CC-BY-NC-SA-4.0"]`.
- An admin `license_soft` flag is raised against Ann's item.

**Control (same flow, keys absent).** Mc (`fieldKeys.ans = null`) reproduces none of this: `c1.ts` 12/12 `chain []` / `lc ok`, `c2.ts` 4/4 Zoe 201. Hidden-key variants are also clean for both kinds (the served form has neither key): `b2diff.ts` hide=true cells 0 failures. A verbatim copy with the same stem also stays clean (`b2verbatim.ts`), because it does not match the later version's bare content.

**Mirror (`b2ans.ts`, Short).** Ann v0 has explanation E and an empty model answer; the copier publishes the same; Ann adds the model answer → the copier is credited and Zoe is refused (same output; Mal and Bea both). `lacksAdded` fails symmetrically when the candidate has the field she added *other* material around (here, the explanation). A Blank mirror was not usable: a Blank with an empty accept list is refused `no_correct` at v0, so only the forward Blank case (add explanation while the accepted answer exists) is covered, by `b2diff.ts`.

**Differential (N runs).**

- `b2diff.ts` (72 runs = 3 kinds × 2 copier roles × 3 paths × hidden/shown × add-explanation/control): **10/10 valid Short/Blank shown-key add-explanation cases fail**; every Mc cell and every hidden cell passes; the add-explanation=false controls pass. (Two further Blank/second-bank cells are invalid payloads in my generator, not product behaviour.)
- `d26.ts` (own differential, 40 seeds / 451 operations: Ann, Mal, Bea, Carl, Zoe, admin; edits, second banks, licences, hidden keys, unlink/relink, delete+purge+re-add, takedowns, hold-copies, erasure, hourly/daily cron): 28 `FALSE_CREDIT` + 28 `UNVERIFIED_LC`, **all of this class**, and **0** of `ANN_REFUSED`, `ANN_HELD`, `READER_WRONG_REFUSAL`, `UPSTREAM_LAUNDER`, `ERASED_ROWS_BACK`.
- 33c's `g33.ts` (320 seeds, all Mc) does not exercise this cell, which is why 33c's fix verification did not catch it.

**Why it counts.** A false public "Another author published these questions earlier" credit against the author, a false first-publisher record that binds downstream copies, and an honest reader refused even though the author publicly offered CC0 — exactly 33c B-2's two harms, on an ordinary Short/Blank edit. The workable path for the reader exists (import again) but nothing tells them, as in 33c B-2. On the convergence rule: the *class* is 33c's B-2, but round 26 states it fixed that class ("It now also returns the earliest own version with the same body ignoring that addition; a candidate source that itself lacks what she added (`lacksAdded`) is judged against it", README:163-170), and it does not for the key-bearing kinds. I count it because it is squarely in the listed blocker classes (false credit / false "first publisher"; an honest teacher's readers wrongly refused), not a wording or scale variant, and it is the ordinary repair the fix was written for.

**Suggested fix.** Judge a candidate against the fields of her `loose` version, not against "both fields of the subject". Store the loose version's `bk_expl`/`bk_ans` (the retained `item_questions` row or the durable `author_stems` row already has them) with the loose seq, and use the loose first unless the candidate carries material that is in the later version but absent from the loose one. A minimum patch is to make the gate per-field (`(p.expl != null && cand.expl !== p.expl) || (p.ans != null && cand.ans !== p.ans)`) in all three places; the precise form above avoids under-crediting a source that carries her addition.

## (c) Non-blockers and variants (terse)

- **33c V1 / new2 (ran):** `h1.ts` and `radd/new2.ts` unchanged. An honest author's reuse of her own question in a NEW item has no derivation to the taken-down copy, so it is DMCA-held; the admin `exempt_authors` list is the path; documented at README:232.
- **V5 no-edit form (ran):** a sock's copy of an accomplice's infringing item still shields the accomplice from hold-copies; the admin needs a second notice. Documented round-26 limit.
- **Read-soft rule (ran):** `u4.ts`/`u5.ts` still show `GET` → own edit → verbatim relabel accepted (201) and a plain GET copy refused; documented V2.
- **Legacy `upstream_snap = 0` after retention (ran, `b1legacy.ts`, `b1legacy2.ts`):** an action made before round 26 keeps an upstream exempt while a publish it made before the action is still retained (first two ordinary republishes: no holds, Eve 200). Once that publish leaves retention (a changed public republish moves `public_publish_id`; the third republish and later), the fallback cannot see it and the upstream is held (`blocklist`). This is the documented "in a publish committed by the action's time" fallback (README:160-162); it cannot occur in a fresh deployment (round 25 was never deployed), and `exempt_authors` is the path. Non-blocker; if staging holds round-25 actions, seeding `dmca_upstream_keys` at migration time would keep them exempt.
- **Pre-0105 ghost with an unretained hidden reshuffle (ran, `radd/mig2b.ts`):** CC0 still 422 for a window whose exact content no longer exists to map. The known unmappable-window limit (README:215-217).
- **`radd/mig2.ts` as written** now stops early because round 26 adds `CREATE TABLE dmca_upstream_keys`, which the harness's fresh DB already applied; that is script incompatibility, not a product defect. The semantic check was re-run as `radd/mig2b.ts` (that one line filtered).
- **`r32a/s7b.ts`** fails on `no such column: content_hash` (a pre-round-12 column name); stale probe, not a product finding.
- **33c differential (`g33.ts`, ran, 320 seeds 100–419, 5,581 operations):** chunks 100–179 / 180–259 / 260–339 / 340–419 completed with exit 0. The three blocker classes are **gone**: `LAUNDER_VIA_UPSTREAM` 0 (was 80), `ERASED_AUTHOR_STEMS_RESURRECTED` 0 (was 144), `FALSE_CREDIT_TO_COPIER` 0 (was 1 at seed 354). What remains is exactly the known artefact/variant set 33c triaged: `ann_second_bank_held_after_td_unnamed(H1 variant)` 19, `copy_uncredited` 10 (the copy was held by the Carl takedown in the same flow), `COPY_LAUNDER` 2 (seeds 230 read-soft, 412 a more restrictive licence than the model's BY), `ERASURE_LAUNDER` 4 (seeds 230/242/296/412: a more restrictive licence, or a subset/near content). No new class. (The Mc-only generator cannot see the Short/Blank B-2 cell.)
- **s4 moving schools:** unchanged documented no-re-link limit.

## (d) What I did NOT run

- Real Cloudflare workerd/D1/R2 or staging; wrangler; no network, no port use.
- CPU/memory/timing measurements and `POOL_STRICT_TIMING=1`; the full suite was run once (pass), not repeatedly under controlled load.
- The blob hold-copies and `recheckBlocks` paths end-to-end: I exercised the blob **publish** hold (`b1blob.ts`: carried-only 200 / no holds; added blocked image 200 / `blocklist`; Eve 404). The other two call sites use the same `upstreamCarried` predicate.
- A real pre-round-25/pre-0105 database (legacy actions and old-style licence rows were simulated in SQL as 33c did).
- The browser/Rust client.

BLOCKERS: 1
