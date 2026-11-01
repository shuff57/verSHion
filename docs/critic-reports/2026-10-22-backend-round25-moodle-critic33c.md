# Round 25 backend review, Moodle-parity critic 33c

**Scope.** `server/worker` (`src/pool/*`, migrations 0102–0105, README rounds 19–25), compared with Moodle's question bank (sparse clone in `scratchpad/ref/moodle`).

**How it was run.** Everything ran in the in-process harness (`test/pool/harness.ts`, share-core `pkg` from 13:05, not rebuilt). No repo file was edited apart from this report. Scratch files are in `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/a8a6fadd-a053-441c-8fa2-cd39f6730aa8/scratchpad/critic33c/`:
- `r32a/` and `radd/`: the previous critics' repro scripts, re-run unchanged.
- `l1*.ts`, `e1.ts`, `c1.ts`, `c2.ts`, `h1.ts`, `f2.ts`, `f3.ts`, `u1`–`u5.ts`: targeted probes.
- `g33.ts`: my own differential.
- `*.out`: the logs.

## (a) Previous findings, re-run on the current tree

| Finding | Script | Result now | Verdict |
|---|---|---|---|
| 32a B-1: copier credited as an earlier publication | `s2`, `s3`, `s1` OWN1 | s2 4/4 `chain []`, lc ok; s3 6/6 `credits Mal=false`; OWN1 16/16 `pvchainMal=false` | FIXED |
| 32a B-2: licence refused after an unrelated edit | `s8`, `s7` | s8: 0/1 refusals shown and 0/6 hidden, every tweak, with and without back-to-NC; s7 12/12 → 200 | FIXED |
| 32a V1: unlisted item's licence grants | `s1` LIC1 | stay_unlisted → 422 (before and after hourly); public_after → 200 | FIXED |
| 32a V2 and addendum B-1: author held after a takedown | `s5`, `s5probe` | s5 6/6: Ann holds `[]`, state live. s5probe (purge, no serve): holds `[]`, `upstreamOf=1` | FIXED |
| Export showed a private title | `s1` EXP1 | unlisted and privacy-removed both give `"title":null` | FIXED |
| pool_key pin | `f1`, `new4` | Wrong key → 503, nothing pinned; right key afterwards → 201 / DELETE 200 / versions 200. `new4` (one body missing) → 503, `pinned []` | FIXED |
| Addendum B-2: `bk_expl_full` not backfilled | `new5`, `new5b` | `simulate_pre24=true` gives `chain=[]`, lc ok | FIXED |
| Addendum B-3: re-key queue stall | `new7` | `canonical CC-BY row present=true`; unmappable rows stay at rest | FIXED |
| new6: hourly offer_sync | `new6` | `after hourly-only 201` | FIXED |
| new2 (documented limit) | `new2` | Ann's NEW item after delete and purge is still held | Unchanged (documented) |

**Test suite.** `npm test`: 458 tests, 457 pass. The one failure was `round21 B-5 (c1_preview_cpu)`, a CPU-timing assertion (`planted 800: 19595 ms vs 6950 ms`). It ran while my four differential processes were also running. Re-run alone it passes (`ok 1`). Typecheck exits 0.

## (b) New-class BLOCKERS

### B-1 (ran): taken-down material laundered through the round-25 upstream exemption

**Mechanism.** The upstream exemption is item-wide and has no time bound:
- `upstreamOf` (`served.ts`) exempts the upstream item from every blocked hash of the action.
- It is applied in:
  - the publish re-upload hold (`publish.ts:2612-2619`);
  - the held-publish charge (`:2702`);
  - `recheckBlocks` (`moderation.ts:1618`, read only);
  - hold-copies.
- `derivationOf` (`moderation.ts:841`) records every chain or origin item of the taken-down publish, whichever positions the admin selected.
- Round 25 removed the timestamp test, so the upstream item may ADD blocked material after the block, and it is never held.

**Inputs (`l1.ts`).**
1. Carl publishes C = [benign question B] (CC-BY-4.0).
2. Mal imports C, then publishes M = [B, T], where T is the publisher's textbook question.
3. Admin takedown of M with `questions:[1]` → `blocked=[{"kind":"match","n":2}]`, `derivation=[CarlUpstream]`.
4. Carl republishes C = [B, T].

**Outputs.**

| Variant | Carl's republish | Carl's holds | Eve's GET of C | Serves T | Controls |
|---|---|---|---|---|---|
| accomplice_upstream | 200 | `[]` | 200 | `servesTextbookQ=true` | Zed re-uploading T → `blocklist` hold; Carl's NEW item with T → `blocklist` hold |
| after 31-day uphold | 200 | `[]` | 200 | true | |
| hold_copies_after | 200 | not held | 200 | true | hold-copies holds `["Zed","CarlNew"]` |
| T and T2 from the same notice | 200 | `[]` | 200 | true | |

**Ruled out.**
- With only one account it does not work (`l1b.ts`): own imports and declared origins record nothing, `derivation []`, held.
- Forging a link into one's own item fails too (`f2`, `f3`: `chain []`, `derivation []`, Carl held).

So the attack needs two accounts, the sock model README already assumes ("socks or not").

**Differential.** `LAUNDER_VIA_UPSTREAM` in 80/80 Carl runs.

**Why it counts.** It is a new launder channel. Blocked material re-enters the pool, publicly served and never held, through the item that round 25 made exempt.

**Fix.** Exempt the upstream only for hashes it CARRIED when the action was made. For example, store `{item_id, hashes}` in `derivation_json` from the upstream's retained or ghost publishes at action time, and test `bh.hash IN` that set. Material added later is then held like any re-upload, and the ordinary republish of unchanged material stays exempt, which was the point of round 25.

### B-2 (ran): copier credited as "earlier", and honest downstream readers refused, when the author fixes a typo and adds her explanation

**Mechanism.** In `provenance.ts:916-917`, `addsNothing` disqualifies her own earlier version whenever her later version adds an explanation or answer (`:951` retained, `:981` durable). The copier's earlier near version is never held to the same test. So a source that lacks the added material just as her earlier version does still becomes "earlier".

**Inputs (`c1.ts`).**
1. Ann publishes v0: the stem "…Fether river?", 3 choices, no explanation, CC-BY-4.0.
2. Mal (who imported it) or Bea (never served) publishes the typo fix "…Feather river?".
3. Ann fixes the typo AND adds her own explanation.

**Outputs, Mal's typo fix first.**
- republish 101 → 200, chain `[{"author":"Mal","earlier_match":true}]`, lc `"unverified_earlier_match"`
- republish 100, then 101 → same
- second bank with 101 → 201, same

**Outputs, Bea's typo fix first (never served).** The same three, crediting Bea.

**Control.** If the other author published a verbatim copy of v0 instead → `chain []`, ok (all 6 cases).

**Downstream harm (`c2.ts`, shown key).** Mal's typo fix is under NC; Ann republishes 101 under CC0 → 200, with:
- chain `["Mal(earlier_match)"]`;
- `unverified_earlier_match`;
- flag `license_soft`.

Then Zoe imports Ann's CC0 item and copies it verbatim under CC0 → `422 license_incompatible`, `"allowed":["CC-BY-NC-4.0","CC-BY-NC-SA-4.0"]`. The same happens under CC-BY. Hidden key: chain `[]` (the explanation is not served), but `license_soft` is still flagged on Ann.

**Differential.** `FALSE_CREDIT_TO_COPIER` at seed 354:

```
initpublic000:CC-BY-NC-4.0 B100:201 B100:201 A100…:200 A101…:200
→ chain [{author_name:"Bea",earlier_match:true}], lc unverified_earlier_match
```

**Why it counts.** This is a false 'first' credit and a public accusation against an honest author in the most ordinary edit there is (fix a typo, write the explanation). It also wrongly refuses honest readers of her item. The mechanism is the round-23 `addsNothing` gate, not the round-24 earliest-record fix. README:206-211 ("a copier who makes the obvious next edit … first is not credited") is false here.

**Moodle.** Moodle never compares one user's questions with another's for credit. Import sets `createdby = $USER->id` and `ownerid = createdby` (`public/question/format.php:446-474`), and an edit is a new `question_versions` row of the same entry.

**Fix.** Judge her earlier version against each candidate source. If the source lacks what her later version added, just as her earlier version does, her earlier version still predates it.

### B-3 (ran): an erased account's durable authorship records are resurrected by the round-25 `stem_backfill`

**Mechanism.**
- Erasure deletes `author_stems` (`index.ts:196`; README round 20 N3: "the account's durable authorship records go with it").
- `stemBackfill` (`publish.ts:445-458`) picks any `item_publishes.author_id` with a retained publish of an unpurged item. Erased accounts and their kept or removed items are not excluded.
- `stemBackfillAuthor` (`:378-439`) then re-INSERTs the keyed sketches and records.
- Purge does not delete `author_stems`, so the rows outlive the items.

**Inputs (`e1.ts`).** Ann publishes (shown or hidden); Mal imports (the item is kept) or does not (the item is removed); `erase(Ann)`; then hourly and daily cron.

**Outputs.** `author_stems rows for Ann before=8 right-after-erasure=0 after-1-hourly=8 after-16d=8` in all 4 combinations, including "kept=false", where the item has been purged (`"p":1`).

**Differential.** `ERASED_AUTHOR_STEMS_RESURRECTED` in 144/165 erasure tails. The 21 clean ones had no retained publish left.

**Moodle.** `delete_data_for_user` anonymises `createdby`/`modifiedby` (`public/question/classes/privacy/provider.php:526-542`), and nothing re-derives the user's data afterwards.

**Fix.** In both functions, skip authors with `users.deleted_at IS NOT NULL` (or in `erasure_done`). Optionally re-run the `DELETE` in the erasure batch's daily redo.

## (c) Differential (`g33.ts`, my own model), ran

**Size.** 320 seeds (100–419), 5,581 operations, 4 parallel chunks. Hidden key in 35% of seeds; 25% start unlisted.

**Operations generated.**
- **Ann:** edits stem, choices or explanation (her own-version chains). Changes licence. Puts the question in second banks using:
  - her current version;
  - an earlier version of hers;
  - a version a copier published first.
  - She also toggles unlisted, makes bad attestations (null, `own_work:false`, an old terms version, `own_work:"yes"`), and deletes and purges.
- **Mal:** serves (import or GET); verbatim copies; publishes the obvious next edit BEFORE Ann does.
- **Bea:** never served; publishes independently, including typo fixes first.
- **Takedown tails** on Mal's copies: rightful named or not, hold-copies, 31 days of cron, then Ann republishes.
- **Carl-upstream tails.**
- **0105 re-key simulation:** Ann's canonical `ckey` reverted to exact `q:<content>` through retained and ghost publishes in `content_licences` and `item_offer`; hourly; windows compared (212 runs).
- **Sweeps:** export, notices, outbox and `derivation_json` key sweeps.
- **Erasure tails** followed by a Mal copy.

**Model.** Grants = the licences at each serve of that canonical content, plus windows publicly offered after the serve.

**Results.**
- **Clean (0 failures):**
  - Ann refused (any publish, PATCH or attestation path);
  - Ann's original held, including after PATCH back to public and after takedown or hold-copies;
  - Ann struck;
  - Bea refused;
  - wrong licence refusals;
  - re-key lost windows (212 runs);
  - attestation state changes;
  - KEY_LEAK: Mal, Bea and Carl exports, all notices, `pool_outbox`, `derivation_json`, Mal's hidden serves.
- **B-1:** 80. **B-3:** 144. **B-2:** 1 (seed 354).
- **H1 variant:** 19 (below).
- **Model artefacts, triaged:**
  - `copy_uncredited` ×10: Mal's copy was held by the Carl takedown, so the GET returned non-200.
  - `COPY_LAUNDER` 412 and `ERASURE_LAUNDER` 296/242/412: the content is a subset of, or near to, content served under a looser licence, or the result is a more restrictive licence than BY.
  - `COPY_LAUNDER` 230: the read-soft variant V2 below.

## (d) Variants and non-blockers (terse)

- **V1 (ran, `h1.ts`; g33 ×19). Ann's own question reused in a NEW bank is held after a copy's takedown when she is not named rightful.**
  - public_not_named → 201, `holds=["blocklist"]`, `state=held_for_review`, Eve GET 404.
  - ann_unlisted → same.
  - rightful_named → live.
  - The exemption is per item, not per author's pre-existing material. Same family as 32a V2 and new2. Fixed by the same "hashes the upstream carried" record as B-1, if extended to the upstream's author.
- **V2 (ran, `u4`/`u5`). A read (GET) followed by an edit is soft by design** (`GET then edit … CC-BY: 201`).
  - Consequence: once that soft version exists, a later IMPORT of Ann's identical content does not re-bind.
  - "GET, edit BY-SA, Ann publishes the edit, Mal IMPORTS it, verbatim under CC-BY: 201" (control without his own edit: 422).
  - Covered by the documented read-soft rule.
- **V3 (ran, `c2`).** Hidden key: Ann's typo fix plus explanation after a copier's fix puts `license_soft` on her own item (admin noise; no public effect).
- **V4 (read).** `stemBackfill` selects `LIMIT 5` authors with no ORDER and no progress marker. An author whose hidden body is unreadable (`continue` at `publish.ts:409`) is reselected every run, which can starve other authors' backfill (the new7 pattern). The publish-time per-author run mitigates this. Not reproduced.
- **V5 (ran).** The B-1 no-edit form: a sock's copy of an accomplice's infringing item exempts the accomplice's item from hold-copies. The admin is not shown it as a copy. This is the round-24/25 design trade-off; the admin needs a second notice.
- **V6.** new2 (delete, purge, re-add as a new item → held) still reproduces. It is documented.

## (e) Not run

Real workerd, D1 or R2; staging; a `recheckBlocks` path for B-1 (read only, same `upstreamOf`); performance of `stem_backfill` on R2.

BLOCKERS: 3
