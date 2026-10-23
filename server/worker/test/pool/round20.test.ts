// Round 20: critics 25a (Moodle) and 25b (ASVS), inverted. A link records the licence that allowed the
// use, so downstream copies are bound by exactly that (E1); caps only ever refuse (F1); a serve is
// recorded only against a publish that still exists (F2); erasure takes the account's records and
// report notes (F3); the ghost GC and preview CPU are bounded (F4); a re-read of a corrected version
// is never a new read, a held fix is exempt only up to HELD_FIX_RECORDS; the 'relicensed' hint reads
// the served rows; and no tail of deletes, purges, erasures, crons or caps changes an outcome.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { reliedLicence } from "../../src/pool/license.ts";
import { HELD_FIX_RECORDS } from "../../src/pool/publish.ts";
import { GHOST_GC_ROWS } from "../../src/pool/cron.ts";
import { png, publish, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const img = (sha: string) => ({ file: `media/${sha}.png`, width: 9, height: 9 });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
const code = (r: { status: number; body: Q }) => (r.status >= 400 ? `${r.status} ${r.body.error}` : String(r.status));
const setLimit = (w: World, u: User, metric: string, value: number) =>
  w.sql.prepare(`INSERT OR REPLACE INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, ?, ?, 0, 'x')`).run(u.id, metric, value);
const LONG = "Ann's own worked explanation: the logger shows water above twenty degrees on forty days of the 2019 season, juvenile counts fell in exactly those weeks, and the regression slope links them clearly.";
const ANNQ = () => mcq("In Ann's field study of the delta, which factor best explains the 2019 decline of juvenile Chinook salmon at the Freeport trap?", [["Elevated summer water temperature above 20 C", true], ["Predation by striped bass", false], ["Reduced turbidity", false], ["Lower dissolved oxygen", false]]);
const MALQ = (extra: Q) => mcq("Mal's own question about rivers: is the water warm in August?", [["Yes", true], ["No", false]], extra);
const TARGETS = ["CC0-1.0", "CC-BY-4.0", "CC-BY-NC-SA-4.0"];
/** The CC table (docs/shared-pool.md): what an adaptation of each licence may carry. */
const MAY: Record<string, string[]> = { "CC0-1.0": ["CC0-1.0", "CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"], "CC-BY-4.0": ["CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"], "CC-BY-SA-4.0": ["CC-BY-SA-4.0"], "CC-BY-NC-4.0": ["CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"], "CC-BY-NC-SA-4.0": ["CC-BY-NC-SA-4.0"] };

// ---- E1: the licence a link records ----------------------------------------------------------------

/** Ann (history `lics`, same content), Mal reads or imports at the first, publishes `body` under `t`; Zed reads Mal's item only and copies. */
async function chainWorld(lics: string[], how: "read" | "import", kind: "verbatim" | "unit", t: string) {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), Z = w.user("teacher", "Zed"), R = w.user();
  const src = { ...ANNQ(), explanation: LONG };
  let a: any = (await publish(w, A, bankOf([src]), { license: lics[0] })).body;
  assert.equal((how === "read" ? await get(w, M, a.item_id) : await imp(w, M, a.item_id)).status, 200);
  for (const l of lics.slice(1)) {
    w.advance(DAY);
    const r = await publish(w, A, bankOf(own(a, [src])), { license: l, base: a.publish_id }, a.item_id);
    assert.equal(r.status, 200);
    a = { ...r.body, item_id: a.item_id };
  }
  const body = kind === "verbatim" ? src : MALQ({ explanation: LONG });
  const m = await publish(w, M, bankOf([body]), { license: t });
  if (m.status !== 201) return { mal: code(m), link: null, zed: [] as string[] };
  const g = await get(w, R, m.body.item_id as string);
  const link = (g.body.questions[0].chain as Q[]).find((l) => l.author_name === "Ann")?.license ?? null;
  await get(w, Z, m.body.item_id as string);
  const zed: string[] = [];
  for (const L of TARGETS) zed.push(`${L}:${code(await publish(w, Z, bankOf([body]), { license: L }))}`);
  return { mal: "201", link, zed };
}

test("E1 (t1b, t1c, t4): a link records the licence that allowed the use; downstream equals reading the source at that licence (every combination)", async () => {
  const L3 = ["CC0-1.0", "CC-BY-4.0", "CC-BY-NC-SA-4.0"];
  for (const l1 of L3)
    for (const l2 of L3) {
      if (l1 === l2) continue;
      for (const how of ["read", "import"] as const)
        for (const kind of ["verbatim", "unit"] as const)
          for (const t of TARGETS) {
            const got = await chainWorld([l1, l2], how, kind, t);
            if (got.mal !== "201") continue;
            // What Mal relied on: the most permissive of what was available (received l1, l2 now for the
            // same content) that permits t (computed here from the CC table, not by the code under test).
            const permits = (l: string) => (l === "CC0-1.0" ? true : kind === "unit" ? (MAY[l] ?? []).includes(t) : l === t);
            const relied = [l1, l2].filter(permits).sort((x, y) => (MAY[y] ?? []).length - (MAY[x] ?? []).length)[0] ?? null;
            assert.equal(reliedLicence([[l1, l2]], t, kind === "unit"), relied, "reliedLicence agrees");
            assert.ok(relied, `${l1}->${l2} ${how} ${kind} @${t}`);
            const tag = `${l1}->${l2} ${how} ${kind} @${t}`;
            assert.equal(got.link, relied, `${tag}: the link`);
            // The same downstream as if Ann had always published under the relied licence.
            const ref = await chainWorld([relied], how, kind, t);
            assert.deepEqual(got.zed, ref.zed, `${tag}: Zed`);
          }
    }
});

test("E1: the critic's flash cases: a CC0 copy credits CC0 (never a stricter label), and its downstream copies are free", async () => {
  for (const how of ["read", "import"] as const) {
    const a = await chainWorld(["CC-BY-NC-SA-4.0", "CC0-1.0"], how, "verbatim", "CC0-1.0");
    assert.deepEqual([a.mal, a.link], ["201", "CC0-1.0"]);
    assert.ok(a.zed.every((z) => z.endsWith(":201")), JSON.stringify(a.zed));
    const b = await chainWorld(["CC0-1.0", "CC-BY-NC-SA-4.0"], how, "unit", "CC0-1.0");
    assert.deepEqual([b.mal, b.link], ["201", "CC0-1.0"], "received CC0 is irrevocable");
    assert.ok(b.zed.every((z) => z.endsWith(":201")), JSON.stringify(b.zed));
  }
});

test("E1: a unit's received licences are those of the serves that HELD it (never another version's)", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
  const a = (await publish(w, A, bankOf([ANNQ()]), { license: "CC-BY-4.0" })).body as any;
  await get(w, M, a.item_id);
  w.advance(DAY);
  // v2 adds the text and is NC; Mal reads it; then Ann relicenses again to CC-BY-NC-SA (the text stays).
  const v2 = (await publish(w, A, bankOf(own(a, [{ ...ANNQ(), explanation: LONG }])), { license: "CC-BY-NC-4.0", base: a.publish_id }, a.item_id)).body as any;
  await get(w, M, a.item_id);
  await publish(w, A, bankOf(own({ ...v2, item_id: a.item_id }, [{ ...ANNQ(), explanation: LONG }])), { license: "CC-BY-NC-SA-4.0", base: v2.publish_id }, a.item_id);
  const p = await publish(w, M, bankOf([MALQ({ explanation: LONG })]), { license: "CC-BY-4.0" });
  assert.deepEqual([p.status, p.body.error], [422, "license_incompatible"], "v1's CC-BY never covers the text v1 did not have");
});

test("E1 / seed 29: the source's licence now outlives the source (deleted, purged, its author erased): frozen in the ghost", async () => {
  // Question: received CC-BY, the same content now NC-SA: a verbatim NC-SA copy is allowed (by the
  // licence now). Unit: received BY-SA, the text now CC-BY: Mal's own question carrying it may be CC-BY.
  for (const [kind, l1, l2, t] of [["verbatim", "CC-BY-4.0", "CC-BY-NC-SA-4.0", "CC-BY-NC-SA-4.0"], ["unit", "CC-BY-SA-4.0", "CC-BY-4.0", "CC-BY-4.0"]] as const)
    for (const tail of ["none", "delete", "purge", "erase"] as const) {
      const w = new World();
      const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
      const src = { ...ANNQ(), explanation: LONG };
      const a = (await publish(w, A, bankOf([src]), { license: l1 })).body as any;
      await imp(w, M, a.item_id);
      w.advance(DAY);
      assert.equal((await publish(w, A, bankOf(own(a, [src])), { license: l2, base: a.publish_id }, a.item_id)).status, 200);
      if (tail !== "none") await w.json("DELETE", `/items/${a.item_id}`, { user: A });
      if (tail === "purge" || tail === "erase")
        for (let d = 0; d < 20; d++) {
          w.advance(DAY);
          await w.pool.daily();
        }
      if (tail === "erase") {
        const { deleteContentStatements } = await import("../../src/pool/index.ts");
        w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, pw_params = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, A.id);
        await w.db.batch(deleteContentStatements(w.db, A.id, w.clock));
      }
      if (tail === "purge") assert.equal(w.q(`SELECT * FROM item_publishes WHERE item_id = ?`, a.item_id).length, 0, "gone: only the ghost is left");
      const body = kind === "verbatim" ? src : MALQ({ explanation: LONG });
      assert.equal(code(await publish(w, M, bankOf([body]), { license: t })), "201", `${kind} ${tail}`);
    }
});

// ---- variants -------------------------------------------------------------------------------------

test("variant 1: the 'relicensed' hint reads what the account was served", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
  const a = (await publish(w, A, bankOf([{ ...ANNQ(), explanation: LONG }]), { license: "CC-BY-4.0" })).body as any;
  await get(w, M, a.item_id);
  w.advance(DAY);
  const changed = { ...ANNQ(), explanation: LONG + " Version two adds a remark." };
  await publish(w, A, bankOf(own(a, [changed])), { license: "CC-BY-NC-4.0", base: a.publish_id }, a.item_id);
  await get(w, M, a.item_id);
  // The imports rows say nothing any more (as for an account whose import records were anonymised).
  w.sql.prepare(`DELETE FROM imports WHERE user_id = ?`).run(M.id);
  const p = await publish(w, M, bankOf([changed]), { license: "CC-BY-4.0" });
  assert.equal(p.body.error, "license_incompatible");
  assert.deepEqual(p.body.relicensed?.map((x: Q) => x.item_id), [a.item_id]);
});

test("variant 2: a re-read of a corrected version of an item already read is never refused by reads_max", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
  const a = (await publish(w, A, bankOf([ANNQ()]))).body as any;
  const b = (await publish(w, A, bankOf([MALQ({})]))).body as any;
  setLimit(w, M, "reads_max", 1);
  assert.equal((await get(w, M, a.item_id)).status, 200);
  await publish(w, A, bankOf(own(a, [{ ...ANNQ(), points: 2 }])), { base: a.publish_id }, a.item_id);
  assert.equal((await get(w, M, a.item_id)).status, 200, "the corrected version");
  assert.equal((await get(w, M, b.item_id)).status, 413, "a new item still counts");
});

test("variant 3: a fix of a held item is exempt from author_records only up to HELD_FIX_RECORDS new records", async () => {
  for (const big of [false, true]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), B = w.user(), C = w.user();
    setLimit(w, A, "shared_questions", 100_000);
    const qs = (n: number, v: string) => Array.from({ length: n }, (_, i) => mcq(`Held bank question ${i}: which value is ${i * 7}?${i === 0 ? ` (hint: it is the first choice) ${v}` : ""}`, [[`${i * 7}`, true], ["zero", false]]));
    const n = big ? Math.ceil(HELD_FIX_RECORDS / 2) + 50 : 20;
    const pub = (await publish(w, A, bankOf(qs(n, "")), { hideKey: true })).body as any;
    w.advance(10 * DAY);
    for (const u of [B, C]) {
      w.history(u);
      await get(w, u, pub.item_id);
    }
    w.advance(DAY);
    for (const u of [B, C]) await w.json("POST", `/items/${pub.item_id}/report`, { user: u, json: { reason: "key_leak" } });
    setLimit(w, A, "author_records", w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM author_contents WHERE author_id = ?`, A.id)[0]!.n);
    // A fix that rewrites every question: about 2 new records per question (hidden: served near and key-free).
    const fix = await publish(w, A, bankOf(own(pub, qs(n, "").map((x, i) => ({ ...x, stem: `${x.stem} Rewritten ${i}.`.replace(" (hint: it is the first choice) ", "") })))), { hideKey: true, base: pub.publish_id }, pub.item_id);
    assert.equal(fix.status === 200, !big, `big=${big}: ${code(fix)}`);
    if (big) assert.equal(fix.body.error, "author_records_exceeded");
  }
});

// ---- F1: caps only refuse --------------------------------------------------------------------------

test("F1 / round 21: the unit allowance is on the publisher's OWN distinct units (too_many_units past it); padding with someone else's units changes nothing", async () => {
  const text = (k: number) => `Explanation ${k} in full: levees cut the floodplain off, so sediment ${k} stays in the channel and the delta number ${k} loses its supply every single decade from now on.`;
  for (const n of [3, 12]) {
    const w = new World({ unitKeysMax: 10 });
    const M = w.user("teacher", "Mal");
    const r = await publish(w, M, bankOf(Array.from({ length: n }, (_, k) => MALQ({ stem: `Mal's question ${k} about rivers: is the water warm?`, explanation: text(k) }))), { license: "CC0-1.0" });
    assert.equal(code(r), n === 3 ? "201" : "422 too_many_units", `n=${n}`);
  }
});

test("F1 / round 21: past the per-question source allowance with no copy among them, a licence a source left out forbids is refused (naming it); one they all permit is published, flagged", async () => {
  const w = new World();
  const R = w.user("teacher", "Reader");
  const base = "Explain how the delta's sediment budget changed after the levees were built along the river";
  const items: string[] = [];
  for (let k = 0; k < 40; k++) {
    // 34 imported adaptations of one stem, each with its own long explanation (no copy among them).
    const expl = `Author ${k}'s own explanation number ${k}: levees cut the floodplain off, so sediment ${k} stays in the channel and the delta ${k} shrinks each decade.`;
    items.push((await publish(w, w.user(), bankOf([q(`${base}?`, { explanation: expl })]), { license: k === 39 ? "CC-BY-NC-4.0" : "CC0-1.0" })).body.item_id as string);
    w.advance(1000);
  }
  for (const id of items) await imp(w, R, id);
  const p = await publish(w, R, bankOf([q(`${base}?`, { explanation: "The reader's own explanation: levees changed where sediment goes, and the delta lost its supply." })]), { license: "CC0-1.0" });
  assert.deepEqual([p.status, p.body.error, p.body.positions], [422, "license_incompatible", [0]]);
  const named = (p.body.per_question[0].bound_by as Q[]).map((b) => b.item_id);
  assert.ok(named.includes(items[39]), "the forbidding source left out is named");
  // (the 32 earliest are kept and named as binders; of those left out, only the forbidding one is named)
  assert.ok(named.every((x) => !items.slice(32, 39).includes(x)), "never a left-out source whose licence allows it");
  const nc = await publish(w, R, bankOf([q(`${base}?`, { explanation: "The reader's own explanation: levees changed where sediment goes, and the delta lost its supply." })]), { license: "CC-BY-NC-4.0" });
  assert.equal(nc.status, 201, JSON.stringify(nc.body).slice(0, 200));
  const chain = (await get(w, w.user(), nc.body.item_id as string)).body.questions[0].chain as Q[];
  assert.ok(chain.some((l) => l.item_id === "" && (l.collapsed_licenses as string[]).includes("CC-BY-NC-4.0")), "a placeholder link carries the licences left out");
  // A question its author did not touch keeps its earlier outcome (round 9 J2): published attesting
  // independence of them, then republished unchanged without the attestation.
  const own0 = q(`${base}?`, { explanation: "The reader's own explanation, published first under an attestation of independence of every source." });
  const v1 = (await publish(w, R, bankOf([own0]), { license: "CC0-1.0", independent: items })).body as any;
  assert.ok(v1.item_id, JSON.stringify(v1).slice(0, 200));
  const rp = await publish(w, R, bankOf(own(v1, [own0])), { license: "CC0-1.0", base: v1.publish_id }, v1.item_id);
  assert.equal(rp.status, 200, JSON.stringify(rp.body).slice(0, 200));
});

test("F1: past the source allowance the copies kept are the EARLIEST, so the original still decides (33 later relabels)", async () => {
  const w = new World();
  const T0 = w.user("teacher", "Tess");
  const Q0 = mcq("A question many teachers relabelled: which landform forms where a river meets the sea?", [["Delta", true], ["Mesa", false], ["Butte", false]]);
  const ids = [(await publish(w, w.user("teacher", "Orig"), bankOf([Q0]), { license: "CC0-1.0" })).body.item_id as string];
  for (let k = 0; k < 33; k++) {
    w.advance(1000);
    ids.push((await publish(w, w.user(), bankOf([Q0]), { license: "CC-BY-NC-SA-4.0" })).body.item_id as string);
  }
  // (the original is also the first one served: an order by serve time, latest first, would drop it)
  for (const id of ids) {
    await imp(w, T0, id);
    w.advance(1000);
  }
  assert.equal(code(await publish(w, T0, bankOf([Q0]), { license: "CC0-1.0" })), "201", "the CC0 original decides; later relabels never narrow it");
});

test("F1 lint (rounds 20-21): every LIMIT, slice or take on a decision path is an existence check, a bounded batch that resumes, a page with a cursor, a display list, or a cap that refuses, flags or carries what it leaves out", () => {
  const files = ["publish.ts", "provenance.ts", "blocking.ts", "moderation.ts", "licensecheck.ts", "read.ts", "admin.ts", "cron.ts", "shared.ts", "served.ts", "visible.ts", "license.ts"];
  // Each allowed form, with why it is safe.
  const allow: [RegExp, string][] = [
    [/LIMIT 1\b(?!\d)/, "existence / the earliest one"],
    [/LIMIT \$\{\w+ \+ 1\}|LIMIT \$\{\(c\.deps\.\w+ \?\? \w+\) \+ 1\}/, "one more than the cap: the caller refuses, flags every question, or pages past it"],
    [/LIMIT 1001`/, "rightful check: one more than it covers, failed past it"],
    [/LIMIT \?2\)\)/, "occurrence counts up to OCC_SCAN + 1: a choice of plan, never an answer"],
    [/ORDER BY x\.rowid LIMIT \?4\)/, "per-key windows of the earliest occurrences: stock and relays (soft), the earliest decide"],
    [/LIMIT \?[3-7]\b|LIMIT \?5 \+ 1|LIMIT \?7 \+ 1/, "fuzzy and earlier-publication caps: counted before any cut and refused, carried or flagged"],
    [/LIMIT \$\{size\} RETURNING/, "privacy hold batch: returns whether more remain (resumed by the cron)"],
    [/LIMIT \?2`\)\.bind\(itemId, COPIES_PER_CALL\)/, "purge of held copies: paged, with the total and the remainder"],
    [/LIMIT \$\{listMax\}`/, "block plan list: the total is counted separately"],
    [/ORDER BY i\.id LIMIT 20`/, "block plan: the items a block alone holds, a preview list"],
    [/LIMIT 500\)|r2_orphans .*LIMIT 500|LIMIT 200`|LIMIT \$\{size\}`|LIMIT \$\{c\.deps\.backfillItems|LIMIT \$\{GHOST_GC_ROWS\}|LIMIT 50`,$|LIMIT 20`\)|LIMIT \?1`\)|LIMIT 100`\)/, "machinery batches that resume next run"],
    [/AND i\.license NOT IN \(SELECT value FROM json_each\(\?3\)\) LIMIT 50`/, "soft check: the offenders listed (filtered before the cut)"],
    [/LIMIT \$\{SEARCH_EXAMINED\}|searchCandidates|LIMIT \?6`|ORDER BY r\.at, r\.id LIMIT 500`|LIMIT \$\{MINE_PAGE\}`|LIMIT \$\{LINEAGE_COPIES_MAX\}`|LIMIT \?4`|LIMIT \?3`/, "browse and listings: cursor pages, no decision"],
    [/LIMIT 500`,$/, "notices to authors of held copies (mail only)"],
    [/LIMIT \$\{OFFER_SYNC_ITEMS\}`|LIMIT \$\{CL_GC_ROWS\}\)`|a\.restored_at IS NOT NULL LIMIT 50`\)/, "round 23 machinery batches that resume next run (licence reconcile and GC, restored holds)"],
    [/DISTINCT user_id FROM imports WHERE user_id IN \(\$\{done\}\) LIMIT 50`/, "round 27 erasure resweep: a bounded batch of erased accounts whose imports are re-keyed; the rest resume next day, nothing is decided by it"],
    [/LIMIT \$\{LICENCE_REKEY_ROWS\}/, "licence re-key batch: only MAPPABLE rows enter it, so nothing can block the queue; it resumes next run"],
    [/LIMIT \$\{STEM_BACKFILL_PUBLISHES\}|LIMIT \$\{STEM_BACKFILL_AUTHORS\}/, "stem backfill: a bounded batch of legacy records; the next run continues, nothing is decided by it"],
    [/ORDER BY p\.publish_id LIMIT 5`/, "key verification: up to five readable bodies (an existence probe), never a decision cut"],
    [/terms_version\)\.slice\(0, 40\)/, "the attested terms version, truncated for storage (compared exactly first)"],
    [/ORDER BY a\.disabled_at DESC LIMIT 100`,$/, "the author's pending ghost takedowns listed (display)"],
    [/\.slice\(17\)|\.slice\(0, -1\)|\.slice\(0, 32\)|\.slice\(5\)|\.slice\(i, i \+ n\)|\.slice\(off, off \+ chunk\)|\.slice\(0, 4096\)/, "string and chunk slicing, no cut"],
    [/\.slice\(0, (20|50|100|200)\)|first: xs\.slice\(0, FLAG_DETAIL_MAX\)|relicensed\.slice\(0, 20\)/, "a response or flag lists the first few; the decision covers all"],
    [/\)\)\.slice\(0, (16|8)\);$|body\.slice\(1\), memo/, "a keyed hash truncated (64 / 32 bits), a body's words after its kind tag"],
    [/\.slice\(0, 64\)/, "token lists capped the same way on both sides of a comparison (a sketch)"],
    [/rows\.results\.slice\(0, (limit|lim)\)|cand\.slice\(0, LINEAGE_COPIES_MAX\)|r\.copies\.slice\(0, LINEAGE_COPIES_MAX\), copies_more|held\.results\.slice\(0, LINEAGE_COPIES_MAX\)|page\.slice\(0, LINEAGE_COPIES_MAX\)/, "a page of one more than shown: the cursor says there is more"],
    [/over\.slice\(0, Math\.floor\(HOT_BUDGET \/ B\)\)/, "the keys counted again (a plan choice); the rest stay on the served-driven path, never skipped"],
    [/hs\.slice\(0, FUZZY_KEEP\)|hs\.slice\(FUZZY_KEEP\)/, "the admissible near sources kept; the rest carried as licences (FuzzyCut)"],
    [/occs\.slice\(0, k\)|chain\.slice\(0, -1\)|chain\.slice\(0, keepHead\)|chain\.slice\(chain\.length - keepTail\)|chain\.slice\(keepHead, chain\.length - keepTail\)|links\.slice\(0, CREDITS_HEAD\)/, "list logic: nothing left out (collapsed links keep their licences)"],
    [/\.slice\(0, SEARCH_WORDS_MAX \+ 500\)|\.slice\(0, 6\)/, "search words (browse only)"],
    [/match\.filter\(\(x\) => x\.selected\)\.slice\(0, 200\)/, "block plan preview: who each block alone holds, shown for the first 200"],
    [/shared\.results\.slice\(0, 1000\)/, "rightful check: past 1,000 it fails (too_many_shared_questions)"],
    [/hashes: list\.slice\(0, 200\)|bodyTokens\(\{ kind/, "a response lists the first few"],
  ];
  const bad: string[] = [];
  for (const f of files) {
    const lines = readFileSync(new URL(`../../src/pool/${f}`, import.meta.url), "utf8").split("\n");
    lines.forEach((l, i) => {
      if (!/LIMIT [0-9?$]|\.slice\(|\.take\(/.test(l) || /^\s*(\/\/|--|\*)/.test(l)) return;
      if (!allow.some(([re]) => re.test(l))) bad.push(`${f}:${i + 1}: ${l.trim().slice(0, 140)}`);
    });
  }
  assert.deepEqual(bad, []);
});

// ---- F2, F3, F4 ------------------------------------------------------------------------------------

test("F2 (r7_race): a read whose publish was pruned before its serve was recorded is a 404 with nothing recorded; with the race absent it binds through the ghost", async () => {
  const LONG2 = "A different worked explanation for version two: temperatures were logged hourly, and the juvenile trap counts were compared week by week across the whole of the season.";
  const LONG3 = "A third worked explanation for version three: the logger, the trap and the weekly counts together show that warm weeks matched the weeks with the fewest juvenile salmon.";
  for (const race of [false, true]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    const v1 = (await publish(w, A, bankOf([{ ...ANNQ(), explanation: LONG }]), { license: "CC-BY-NC-SA-4.0" })).body as any;
    const repub = async () => {
      for (const ex of [LONG2, LONG3]) {
        const cur = w.q<{ c: string }>(`SELECT current_publish_id AS c FROM items WHERE id = ?`, v1.item_id)[0]!.c;
        const r = await publish(w, A, bankOf(own({ ...v1, publish_id: cur }, [{ ...ANNQ(), explanation: ex }])), { license: "CC-BY-NC-SA-4.0", base: cur }, v1.item_id);
        assert.equal(r.status, 200);
      }
    };
    if (race) {
      w.beforeServe = repub;
      assert.equal((await get(w, M, v1.item_id)).status, 404, "retry: nothing was served");
      assert.equal(w.q(`SELECT * FROM served`).length, 0);
      assert.equal(w.q(`SELECT * FROM imports WHERE user_id = ?`, M.id).length, 0);
      // The retry reads (and binds to) what is current now.
      assert.equal((await get(w, M, v1.item_id)).status, 200);
      assert.equal(code(await publish(w, M, bankOf([MALQ({ explanation: LONG3 })]), { license: "CC0-1.0" })), "422 license_incompatible");
    } else {
      assert.equal((await get(w, M, v1.item_id)).status, 200);
      await repub();
      assert.equal(w.q(`SELECT * FROM ghost_publishes WHERE publish_id = ?`, v1.publish_id).length, 1);
      assert.equal(code(await publish(w, M, bankOf([MALQ({ explanation: LONG })]), { license: "CC0-1.0" })), "422 license_incompatible");
    }
  }
});

test("F3 (N2): a key_leak report's note is never copied into an admin flag (nothing to survive erasure)", async () => {
  const w = new World();
  const A = w.user(), B = w.user("teacher", "Rex");
  const it = (await publish(w, A, bankOf([MALQ({})]), { hideKey: true })).body as any;
  await get(w, B, it.item_id);
  w.advance(10 * DAY);
  await w.json("POST", `/items/${it.item_id}/report`, { user: B, json: { reason: "key_leak", note: "Rex Privatename here: choice A is longest" } });
  const f = w.q<{ detail: string }>(`SELECT detail FROM admin_flags WHERE kind = 'key_leak'`);
  assert.equal(f.length, 1);
  assert.ok(!f[0]!.detail.includes("Privatename") && JSON.parse(f[0]!.detail).report_id, f[0]!.detail);
});

test("F4 (g1_gc): the ghost GC is bounded by rows per run and finishes over runs", async () => {
  const w = new World();
  w.sql.exec("BEGIN");
  const gp = w.sql.prepare(`INSERT INTO ghost_publishes (publish_id, item_id, author_id, hide_key, public_version, license, at) VALUES (?, 'i', 'a', 0, 1, 'CC0-1.0', 0)`);
  const gu = w.sql.prepare(`INSERT INTO ghost_units (publish_id, pos, part, key, kind) VALUES (?, ?, -1, ?, 'text')`);
  for (let p = 0; p < 6; p++) {
    gp.run(`g${p}`);
    for (let k = 0; k < GHOST_GC_ROWS / 4; k++) gu.run(`g${p}`, k, `e:${p}:${k}`);
  }
  w.sql.exec("COMMIT");
  const total = () => w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM ghost_units`)[0]!.n;
  const before = total();
  await w.pool.daily();
  assert.equal(before - total(), GHOST_GC_ROWS, "one run: at most GHOST_GC_ROWS rows");
  await w.pool.daily();
  assert.equal(total(), 0);
  await w.pool.daily();
  assert.equal(w.q(`SELECT * FROM ghost_publishes`).length, 0);
});

test("F4: dry runs count their unit lookups against preview_units", async () => {
  const w = new World();
  const A = w.user();
  setLimit(w, A, "preview_units", 3);
  const { framed } = await import("./harness.ts");
  const prev = async (qs: Q[]) =>
    w.json<any>("POST", "/items/preview", {
      user: A,
      body: framed({ publish_id: `p${"x".repeat(21)}${Math.random().toString(36).slice(2, 6)}`, hide_key: false, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: "2026-10-22" }, labels: [], order_acks: [], image_ok: [], meta: { title: "t", subject: "Biology", course: "", grade: "", tags: [], standards: [] }, visibility: "public" }, bankOf(qs)),
      contentType: "application/vnd.vershion.publish",
    });
  assert.equal((await prev([MALQ({})])).status, 200);
  const r = await prev([MALQ({ explanation: LONG }), { ...ANNQ(), explanation: LONG }]);
  assert.deepEqual([r.status, r.body.metric], [429, "preview_units"]);
});

// ---- the property: no tail changes an outcome -------------------------------------------------------

const SEEDS = Number(process.env.POOL_PROPERTY_SEEDS ?? 8);
function rng(seed: number) {
  let s = seed >>> 0 || 1;
  return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 2 ** 32);
}
const ORDER = ["CC0-1.0", "CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"];
const LONG2 = "Ann's second worked explanation: compare the 2018 and 2019 temperature loggers, then note the juvenile counts at the Freeport trap fell only in the warm weeks.";

/** Ann's item through a random history (edits, relicences, hide toggles), Mal reading and importing along the way; then (with `tail`) deletes, purges, erasure, crons, caps; then Mal's battery of copies. */
async function history(seed: number, tail: boolean): Promise<{ bat: string[]; ops: string[] }> {
  const r = rng(seed);
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), S = w.user("teacher", "Sock"), AD = w.user("admin", "Admin");
  const ua = await upload(w, A, png(9, 9, 61));
  let lic = ORDER[1 + Math.floor(r() * 2)]!;
  let hide = false;
  let body: Q = { ...ANNQ(), explanation: LONG, image: img(ua.sha) };
  const versions: Q[] = [body];
  let pub: any = (await publish(w, A, bankOf([body]), { license: lic })).body;
  const fill: string[] = [];
  for (let k = 0; k < 3; k++) fill.push(((await publish(w, S, bankOf([mcq(`Sock filler ${k}: which river number ${k} drains into the delta near Freeport?`, [["Sacramento", true], ["Nile", false]])]))).body as any).item_id);
  w.advance(DAY);
  const ops: string[] = [];
  for (let i = 0, n = 3 + Math.floor(r() * 7); i < n; i++) {
    const op = Math.floor(r() * 8);
    w.advance(Math.floor(r() * 3) * DAY + 1000);
    if (op === 0) ops.push(`read:${(await get(w, M, pub.item_id)).status}`);
    else if (op === 1) ops.push(`import:${(await imp(w, M, pub.item_id)).status}`);
    else if (op <= 5) {
      let nb = { ...body }, nl = lic, nh = hide;
      if (op === 2) nb = { ...nb, points: (nb.points ?? 1) + 1 };
      if (op === 3) nb = { ...nb, explanation: nb.explanation === LONG ? LONG2 : LONG };
      if (op === 4) nl = ORDER[Math.min(ORDER.length - 1, ORDER.indexOf(lic) + 1 + Math.floor(r() * 2))]!;
      if (op === 5) nh = !hide;
      const x = await publish(w, A, bankOf(own(pub, [nb])), { license: nl, hideKey: nh, base: pub.publish_id }, pub.item_id);
      ops.push(`ann:${["pts", "expl", "lic", "hide"][op - 2]}:${nl}:${nh}:${x.status}`);
      if (x.status === 200) {
        pub = { ...x.body, item_id: pub.item_id };
        body = nb;
        lic = nl;
        hide = nh;
        versions.push(nb);
      }
    } else if (op === 6) await get(w, M, fill[Math.floor(r() * fill.length)]!);
    else await imp(w, M, fill[Math.floor(r() * fill.length)]!);
  }
  if (!ops.some((x) => /^(read|import):200/.test(x))) await get(w, M, pub.item_id);
  // The same random numbers are drawn with and without the tail (so the batteries line up).
  const nt = 2 + Math.floor(r() * 6);
  const picks = Array.from({ length: nt }, () => Math.floor(r() * 8));
  if (tail)
    for (const t of picks) {
      if (t === 0) ops.push(`del:${(await w.json("DELETE", `/items/${pub.item_id}`, { user: A })).status}`);
      else if (t === 1) {
        for (let d = 0; d < 20; d++) {
          w.advance(DAY);
          await w.pool.daily();
        }
        ops.push("20d");
      } else if (t === 2) {
        const { deleteContentStatements } = await import("../../src/pool/index.ts");
        w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, pw_params = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, A.id);
        await w.db.batch(deleteContentStatements(w.db, A.id, w.clock));
        ops.push("eraseAnn");
      } else if (t === 3) ops.push(`adminpurge:${(await w.purge(AD, pub.item_id)).status}`);
      else if (t === 4) {
        setLimit(w, M, "reads_max", 1);
        setLimit(w, M, "imports_max", 1);
        ops.push("caps");
      } else if (t === 5) {
        await w.pool.hourly();
        ops.push("hourly");
      } else if (t === 6) {
        for (let d = 0; d < 400; d += 50) {
          w.advance(50 * DAY);
          await w.pool.daily();
        }
        ops.push("400d");
      } else {
        for (const f of fill) await w.json("DELETE", `/items/${f}`, { user: S });
        ops.push("delfill");
      }
    }
  const um = await upload(w, M, png(9, 9, 61));
  const bodies: [string, Q][] = [
    ...versions.map((v, i) => [`v${i}`, { ...v, image: null }] as [string, Q]),
    ["expl1", MALQ({ explanation: LONG })],
    ["expl2", MALQ({ explanation: LONG2 })],
    ["img", q("Mal's own question: what does this picture of the trap show?", { image: img(um.sha) })],
    ["part", q("Mal's multipart on the delta", { kind: { Multipart: { parts: [{ stem: ANNQ().stem, kind: ANNQ().kind }, { stem: "Mal's part b: name one river", kind: { Short: { sample: "Feather" } } }] } } })],
  ];
  const bat: string[] = [];
  for (const L of ["CC0-1.0", "CC-BY-4.0", "CC-BY-NC-SA-4.0"])
    for (const [n, b] of bodies) {
      const p = await publish(w, M, bankOf([b]), { license: L });
      bat.push(`${n}@${L}=${code(p)}`);
      if (p.status === 201) await w.json("DELETE", `/items/${p.body.item_id}`, { user: M });
    }
  return { bat, ops };
}

test("property (t2_diff): a tail of deletes, purges, erasures, crons, caps never changes Mal's outcomes after any licence history", async () => {
  for (let seed = Number(process.env.POOL_PROPERTY_START ?? 1); seed <= SEEDS; seed++) {
    const s = seed === 1 ? 29 : seed; // (seed 29 is the critic's relicence-then-erasure case)
    const a = await history(s, false);
    const b = await history(s, true);
    assert.deepEqual(b.bat, a.bat, `seed ${s}: ${b.ops.join(", ")}`);
  }
});

test("F1: past the student-data band allowance every question sharing a band is flagged and held (never compared on a part)", async () => {
  const SD = "Jenny Smith (student id 4471) scored 42 on the unit test; her mother asked for a retake on Friday";
  const v = SD.replace("scored 42 on", "earned 40 in").replace("Friday", "Monday").replace("asked for", "requested");
  for (const cap of [undefined, 0]) {
    const w = new World({ privacyBandRowsMax: cap });
    const adm = w.user("admin"), A = w.user(), F = w.user();
    const a = (await publish(w, A, bankOf([q(SD)]))).body as any;
    await w.purge(adm, a.item_id, { questions: [0] });
    const f = (await publish(w, F, bankOf([q(v)]))).body as any;
    const flagged = w.q(`SELECT * FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ?`, f.item_id).length;
    assert.equal(flagged, cap === 0 ? 1 : 0, `cap=${cap}: below the near threshold, flagged only when the comparison was cut`);
  }
});
