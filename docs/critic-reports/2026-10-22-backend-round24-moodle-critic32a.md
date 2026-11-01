###TASK 2026-10-22T16:10:54.359Z
I found 2 blockers, both reproduced in the in-process harness. Neither has a fix applied (I stayed read-only). Each one is a defect in a round-22/23 fix that no earlier critic covered, which is why I count them as blockers and not variants. If you read the convergence rule strictly, either could be filed as a B3 or V-a variant; I've stated my reasons below.

Everything I wrote is in `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/a8a6fadd-a053-441c-8fa2-cd39f6730aa8/scratchpad/critic32a/` (`lib.ts`, `s1.test.ts`, `s2.ts`–`s8.ts`, `f1.ts`, `diff.ts`, plus `*.out` logs). No repo files were edited.

## BLOCKER B-1 (ran): copier credited as "earlier publication" of the original author's own question

**Mechanism.** In `src/pool/provenance.ts:959-960`, the durable `author_stems` lookup runs only for questions her retained publishes did not already answer:
```ts
// (only where her retained publishes did not already answer: the durable record is for what they lost)
const rest = want.filter((p) =&gt; !out.has(p.pos) &amp;&amp; !cut?.has(p.pos) &amp;&amp; p.bodyToks);
```
So a retained version that is newer than the copier hides her older durable record (it should be the minimum of both). Then `earlyFirst` (`publish.ts:1594`) uses the newer date, and the copier wins the `earlier_match` lookup (`publish.ts:1600-1626`).

**Repro (`s2.ts`).**
- Ann publishes v0 (a typo, "Fether"), licence NC.
- Mal imports it and publishes v1 (typo fixed) → 201.
- Ann fixes the same typo (her first republish correctly credits nobody: chain `[]`).
- Then any one of these honest follow-ups credits Mal:
  - **Second bank** ("Final exam" containing the same question) → 201, chain `[{"author_name":"Mal",…,"earlier_match":true}]`, `license_check` `unverified_earlier_match` with the public text "Another author published these questions earlier; there is no record of this publisher being served them.", flag `license_soft`.
  - **Points-only republish of her new item** → 200, same chain crediting Mal.
  - **No purge needed:** typo fix plus two points edits, then a second bank → credits Mal.
- `s3.ts`: also reproduces with hidden keys (`hide=true`, 3 edits → credits Mal).
- `s1` OWN1: the drop-distractor and typo fixes show the same in preview (`pvchainMal=true`).

The key-fix and Short-answer-fix variants were clean.

**Why it counts.** It is a false public credit and accusation against an honest author, it binds downstream copies to Mal's link, and it falsifies README:171-174. Moodle has no comparable risk: import makes the importer the author (`public/question/format.php:444-474`, `createdby = $USER-&gt;id`, `ownerid = createdby`).

## BLOCKER B-2 (ran): a more permissive licence the author offers publicly is refused to an earlier reader after an unrelated edit

**Mechanism.** The licence-history key is `q:${hideKey ? strippedContent : fullContent}` (`publish.ts:2133`). `contentHashes` (`core.ts:453-462`) is an HMAC of `canonical(contentOf(q))`, which:
- keeps points, difficulty and topics, which README:699-702 says "the same question" leaves out;
- hashes a hidden publish's shuffled stripped order, so every hidden republish that re-strips gets a new key.

**Repro (`s8.ts`).** Ann publishes NC; Mal imports. Ann republishes CC0 publicly with only a points, difficulty or topic change. Mal previews a CC0 copy of what he was served → **422 license_incompatible**:
- 1/1 refused for shown items, 6/6 for hidden items, with or without Ann later switching back to NC.
- A control with no tweak: 0 refused.

**Edit-and-revert, hidden key (`s7.ts`).**
- SA → edit the stem under CC0 → revert to the original stem under CC0.
- Mal's CC0 copy of the exact content Ann now offers publicly as CC0 → `422`, `allowed ["CC-BY-SA-4.0"]`.
- Shown items pass. Whether a run is refused depends on the shuffle (seen in differential seeds 120 and 265).

**Why it counts.** An honest reader is refused although the author publicly granted the licence. README:360 ("a relicence to more permissive terms covers earlier readers") is broken for ordinary edits. Mal's workaround is to import again; nothing tells him that.

## Differential (b), ran: `diff.ts`, 320 seeds (100–419), 5,026 operations

**Generator.**
- Ann: public or unlisted at start; hidden key 40% of seeds; edits to stem and explanation; licence changes; switching between public and unlisted; republishes with a bad attestation (missing, `own_work:false`, old `terms_version`); delete and purge.
- Mal: reads and imports, then verbatim copies.
- Bea: copies without ever being served.
- Takedown endings: Mal's copy taken down, Ann named as the rightful item when she qualifies, then 31 days of cron, then Ann republishes.
- Erasure endings: Ann erased, then Mal copies again.
- Export and outbox leak sweeps.

**Model.** Grants = licences Mal was shown at any serve of that content, plus licences Ann publicly offered for exactly that content since that serve.

**Results.**
- **Clean:**
  - 0 Ann refusals; every bad attestation gave 422 and changed nothing.
  - 0 Bea refusals.
  - 0 uncredited copies.
  - 0 key leaks (Mal's and Bea's exports, notices, `pool_outbox`, serves).
  - 0 holds or strikes on Ann when she was named rightful.
- **COPY_WRONGLY_REFUSED** (seed 265) and **erasure_wrong_refusal** (seed 120): both B-2 (hidden edit and revert).
- **copy_allowed_by_current_unlisted_or_held_licence** (seed 359) and the seed 249 erasure case: variant V1 below.
- **COPY_LAUNDER** (seed 340): my model was too strict. Ann had publicly offered the same stem and choices CC0 with an explanation. Not a finding.
- **Ann held after a takedown while unlisted** (seeds 163, 216, 260, 316, 346): variant V2.
- The generator never had Mal make an edit before Ann, so it does not exercise B-1; `s2`/`s3` cover that.

## Authorship-by-order game (c), ran and read

- **Copier becoming "author":** Mal never gets exemption from his own earlier version for material Ann added. `addsNothing` blocks an added explanation (`provenance.ts:916`), and his own earliest date is after Ann's. The real gain is passive and comes from B-1: a copier who makes the obvious next edit first gets credited on her later publishes.
- **Author treated as copier:** yes, B-1.
- **Department banks and moving schools (`s4.ts`):** a teacher's new account copying her old one gets `unverified_earlier_match` crediting "Ann (Chico)" or "deleted teacher". If she imported from the old account first, relicensing to BY gives 422. This is the documented no-re-link limit, a non-blocker.

## Export (d)/(e), ran

- **Leak, non-blocker by the rule** (not a key, credit or launder). `export.ts:229-231` reads `(SELECT title FROM items WHERE id = im.item_id)` with no visibility check:
  - After Bob made his item unlisted and renamed it, Ann's GET → 404, but her export shows `"title":"PRIVATE Jane Doe IEP retake"`.
  - For an item privacy-removed by an admin (`removed_privacy`, inside the 14-day window), her export shows `"title":"Retake for student Jane Q. Doe (IEP)"`.
  - It should export the title as served, or `[removed]`.
- **Completeness:** retained previous publish bodies are not exported (`s6`: previous version present on the server, its body not in the export). Moodle exports every question version the user created or modified (`public/question/classes/privacy/provider.php:438-490`).
- No other-user emails, names, reporter or complainant identities, hashes, or Ann's hidden keys appeared in Mal's or Bea's exports across 320 seeds.

## pool_key_mismatch (f)

**Ran (`f1.ts`).** On a database populated before round 23 (no `pool_key_id`), the first write after the upgrade pins whatever key it runs under, without checking it against stored hashes (`shared.ts:981`):
- First write under a wrong key pins `6e12…`.
- After that, the correct key gets: Ann's publish **503**, DELETE of her own item **503**, `POST /items/versions` **503**.

This is a variant of the B1 key-rotation fix, a non-blocker. The fix would be to seed the pin in the migration or verify the key against an existing keyed hash before pinning a non-empty database. `/items/versions` failing under a mismatch also contradicts "plain reads continue".

**Read only:** first-use race (`INSERT OR IGNORE` then `SELECT`) and cron before the first request are safe.

## Variants and non-blockers

- **V1 (ran, `s1` LIC1).** The current licence is granted while the item is unlisted:
  - Ann unlists, relicenses CC0 and stays unlisted → Mal's CC0 copy → 200, with or without cron.
  - The source is `publish.ts:293`: "listed or not".
  - It also reaches ghosts through `now_license` frozen at purge (seed 249).
  - This contradicts README:175 ("unlisted grants nothing"). It is the author's own licence choice, so not a launder.
- **V2 (ran, `s5`).** Ann's own original item, unlisted when a takedown of Mal's copy is confirmed with no rightful item (she cannot be named, because rightful must be visible), is DMCA-held when she republishes or switches back to public. The public control is not held. README:566 and README:636-637 contradict each other. The admin's `exempt_authors` is the only remedy.
- Moving schools gets credited to the old account: the documented no-re-link limit.
- `/items/versions`, DELETE and report all fail under a key mismatch (see f).

**BLOCKERS: 2**</result>
<usage><subagent_tokens>219388</subagent_tokens><tool_uses>64</tool_uses><duration_ms>1740551</duration_ms></usage>
</task-notification>
###END

