// Round 24: critics 31a (Moodle) and 31b (ASVS). A copier is never the "earlier publisher" of the
// original author's own question (B1); a publicly relaxed licence reaches an earlier reader after an
// unrelated edit (B2), keyed by a canonical form of the stem and the answers; an unlisted item's
// licence grants nothing (S1); an author's own original is not held by a takedown of a copy of it
// (S2); the export never shows a private item's title to an importer (S3), exports every retained
// version's body (S4), and a pool populated before round 23 never pins the first key it sees (S5).
import assert from "node:assert/strict";
import { test } from "node:test";
import { makeCore } from "../../src/pool/index.ts";
import { EXPORT_LIMITS } from "../../src/pool/export.ts";
import { poolKeyId } from "../../src/pool/shared.ts";
import { PUBLISH_CONTENT_TYPE } from "../../src/pool/shared.ts";
import { framed, publish, publishId, shareCore, TEST_HASH_KEY, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const code = (r: { status: number; body: Q }) => (r.status >= 400 ? `${r.status} ${r.body.error}` : String(r.status));
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const login = (w: World, u: User) =>
  w.sql.prepare(`INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at, user_agent) VALUES (?, ?, ?, ?, ?, ?, ?, 'TestBrowser/1')`).run(`s-${u.id}`, `fam-${u.id}`, u.id, `h-${u.id}`, w.clock, w.clock, w.clock + 30 * DAY);
const exp = async (w: World, u: User) => {
  const r = await w.call("GET", "/me/export", { user: u });
  const text = await r.text();
  return { status: r.status, body: (() => { try { return JSON.parse(text); } catch { return null; } })() as any, text };
};

// ---- 1. a copier is never credited as the earlier publisher of the author's own question ----------

const STEMS = [
  "Ann's own question: in her 2019 delta survey, which landform formed first at the mouth of the Feather river?",
  "Ann's own question: in her 2019 delta survey, which landform formed first at the mouth of the Feather river channel?",
  "Ann's own question: in her 2019 delta survey, which landform formed first near the mouth of the Feather river channel?",
  "Ann's own question: in her 2019 delta survey, which landform formed first near the mouth of the Feather river?",
];
const EXPL = "Ann's own measured explanation of the 2019 delta survey, with the logger readings, the juvenile counts and the regression that ties them together.";
const C = (sv: number, expl: string | null) => mcq(STEMS[sv]!, [["A delta lobe of fresh sediment", true], ["A mesa of old rock", false], ["A butte", false]], { explanation: expl });

test("24-1: a copier who edited before the author's next edits is never the 'earlier publisher' of her own question, and her copies never bind to him", async () => {
  // Repro before the fix: Bob copied Ann's v1 and edited its stem; Ann then edited twice (a different
  // body) and reverted to Bob's stem. Her older record (v1) was skipped because a newer retained
  // version matched, so Bob looked earlier and Ann's v4 credited him; Eve's copy of v4 chained to him.
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), E = w.user("teacher", "Eve");
  let a: any = (await publish(w, A, bankOf([C(0, null)]), { license: "CC-BY-SA-4.0" })).body;
  const id = a.item_id;
  w.advance(DAY);
  await imp(w, B, id);
  assert.equal((await publish(w, B, bankOf([C(3, null)]), { license: "CC-BY-SA-4.0" })).status, 201, "Bob's edited copy");
  for (const [sv, expl] of [[1, EXPL], [2, EXPL], [3, null]] as [number, string | null][]) {
    w.advance(DAY);
    const r = await publish(w, A, bankOf(own(a, [C(sv, expl)])), { license: "CC-BY-SA-4.0", base: a.publish_id }, id);
    assert.equal(r.status, 200, JSON.stringify(r.body));
    a = { ...r.body, item_id: id };
    assert.deepEqual((await get(w, E, id)).body.questions[0].chain, [], `after her edit ${sv}: no credit to the copier`);
  }
  // Her own downstream copy (Eve imports Ann's final version) binds to Ann, never to Bob.
  await imp(w, E, id);
  const e = (await publish(w, E, bankOf([C(3, null)]), { license: "CC-BY-SA-4.0" })).body as any;
  const chain = (await get(w, A, e.item_id)).body.questions[0].chain as any[];
  assert.ok(chain.length >= 1);
  assert.equal(chain[0].item_id, id);
  assert.ok(!chain.some((l) => l.item_id === (w.q<any>(`SELECT id FROM items WHERE author_id = ?`, B.id)[0]?.id)), "never Bob");
});

test("24-1b: the same, with hidden keys and a later SHOWN second bank (the durable record keeps the hidden full form's explanation key)", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), E = w.user("teacher", "Eve");
  let a: any = (await publish(w, A, bankOf([C(0, EXPL)]), { license: "CC-BY-SA-4.0", hideKey: true })).body;
  w.advance(DAY);
  await imp(w, B, a.item_id);
  assert.equal((await publish(w, B, bankOf([C(3, EXPL)]), { license: "CC-BY-SA-4.0", hideKey: true })).status, 201);
  // Ann edits points only (hidden), then publishes the same question SHOWN in a second bank.
  w.advance(DAY);
  a = { ...(await publish(w, A, bankOf(own(a, [{ ...C(0, EXPL), points: 4 }])), { license: "CC-BY-SA-4.0", hideKey: true, base: a.publish_id }, a.item_id)).body, item_id: a.item_id };
  w.advance(DAY);
  const n = await publish(w, A, bankOf([{ ...C(3, EXPL), points: 4 }], "Final exam"), { license: "CC-BY-SA-4.0" });
  assert.equal(n.status, 201, JSON.stringify(n.body));
  assert.deepEqual((await get(w, E, n.body.item_id as string)).body.questions[0].chain, [], "no credit to the copier");
});

// ---- 2. a publicly relaxed licence reaches an earlier reader after an unrelated edit ---------------

test("24-2: the author's public relicence covers an earlier reader after a points/difficulty/topics edit (canonical stem+answers key), shown and hidden", async () => {
  for (const hideKey of [false, true]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    const base = mcq(STEMS[0]!, [["A delta lobe of fresh sediment", true], ["A mesa of old rock", false], ["A butte", false]], { points: 1, difficulty: "Low", topics: ["deltas"] });
    let a: any = (await publish(w, A, bankOf([base]), { license: "CC-BY-NC-4.0", hideKey })).body;
    w.advance(DAY);
    assert.equal((await imp(w, M, a.item_id)).status, 200);
    // An unrelated edit (points, difficulty and topics; the stem and the answers are untouched), and
    // the licence publicly relaxed to CC0.
    w.advance(DAY);
    const r = await publish(w, A, bankOf(own(a, [{ ...base, points: 5, difficulty: "High", topics: ["rivers"] }])), { license: "CC0-1.0", hideKey, base: a.publish_id }, a.item_id);
    assert.equal(r.status, 200, JSON.stringify(r.body));
    a = { ...r.body, item_id: a.item_id };
    const m = await publish(w, M, bankOf([{ ...base, points: 8 }]), { license: "CC0-1.0", hideKey });
    assert.equal(m.status, 201, `hide_key=${hideKey}: the relaxed licence reaches the earlier reader (${JSON.stringify(m.body).slice(0, 300)})`);
  }
});

// ---- 3. an unlisted item's licence grants nothing --------------------------------------------------

test("24-3: an unlisted item's licence is not counted as granted (question and unit paths)", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
  const LONG = "Ann's own worked explanation: the logger shows water above twenty degrees on forty days of the 2019 season, juvenile counts fell in exactly those weeks, and the regression slope links them clearly.";
  let a: any = (await publish(w, A, bankOf([mcq(STEMS[0]!, [["A delta lobe", true], ["A mesa", false]], { explanation: LONG })]), { license: "CC-BY-NC-4.0" })).body;
  w.advance(DAY);
  await imp(w, M, a.item_id);
  // Unlisted, then publicly relax it to CC0: it offers nothing, so the reader gets nothing.
  await w.json("PATCH", `/items/${a.item_id}`, { user: A, json: { visibility: "unlisted" } });
  w.advance(DAY);
  const r = await publish(w, A, bankOf(own(a, [mcq(STEMS[0]!, [["A delta lobe", true], ["A mesa", false]], { explanation: LONG, points: 3 })])), { license: "CC0-1.0", base: a.publish_id }, a.item_id);
  assert.equal(r.status, 200, JSON.stringify(r.body));
  // The question itself: a verbatim copy under CC0 is refused.
  assert.equal(code(await publish(w, M, bankOf([mcq(STEMS[0]!, [["A delta lobe", true], ["A mesa", false]])]), { license: "CC0-1.0" })), "422 license_incompatible", "question");
  // A unit (the long explanation) taken into another question is refused too.
  const other = mcq("Mal's own question about the survey programme on the lower river reaches?", [["Yes", true], ["No", false]], { explanation: LONG });
  assert.equal(code(await publish(w, M, bankOf([other]), { license: "CC0-1.0" })), "422 license_incompatible", "unit");
  // ... and a ghost's frozen licence is offered-only too: the same item deleted and purged while
  // unlisted still grants its reader nothing.
  await w.json("DELETE", `/items/${a.item_id}`, { user: A });
  for (let d = 0; d < 16; d++) {
    w.advance(DAY);
    await w.pool.hourly();
    await w.pool.daily();
  }
  assert.equal(w.q(`SELECT 1 FROM items WHERE id = ? AND purged_at IS NOT NULL`, a.item_id).length, 1, "purged");
  assert.equal(code(await publish(w, M, bankOf([mcq(STEMS[0]!, [["A delta lobe", true], ["A mesa", false]])]), { license: "CC0-1.0" })), "422 license_incompatible", "ghost");
});

// ---- 4. an author's own original is not held after a takedown of a copy of it ----------------------

test("24-4: a takedown of a copy never holds the author's own (by then unlisted) original, while a copy of the copy is held", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), Cc = w.user("teacher", "Cara");
  const a = (await publish(w, A, bankOf([C(0, null)]), { license: "CC-BY-SA-4.0" })).body as any;
  w.advance(DAY);
  await imp(w, B, a.item_id);
  const b = (await publish(w, B, bankOf([C(0, null)]), { license: "CC-BY-SA-4.0" })).body as any;
  w.advance(DAY);
  await imp(w, Cc, b.item_id);
  const c = (await publish(w, Cc, bankOf([C(0, null)]), { license: "CC-BY-SA-4.0" })).body as any;
  // Ann's original is now unlisted (so it cannot be named as the "rightful item", which needs a
  // visible item): the takedown must still not hold it.
  await w.json("PATCH", `/items/${a.item_id}`, { user: A, json: { visibility: "unlisted" } });
  const nid = await notice(w, adm);
  const t = await w.takedown<any>(adm, nid, b.item_id, { questions: "all" });
  assert.equal(t.status, 200, JSON.stringify(t.body));
  assert.equal((await w.holdCopies<any>(adm, t.body.action_id)).status, 200);
  const held = w.q<any>(`SELECT item_id FROM item_holds WHERE source = ?`, `dmca:${t.body.action_id}`).map((x) => x.item_id);
  assert.ok(!held.includes(a.item_id), "the author's own original is not held");
  assert.ok(held.includes(c.item_id), "Cara's copy of the taken-down copy is held");
  // The re-upload block does not hold it either: her republish, and switching back to public, are
  // clean (the V2 repro: she cannot be named rightful while unlisted, so this is decided by lineage).
  w.advance(DAY);
  const rp = await publish(w, A, bankOf(own(a, [C(0, null)])), { license: "CC-BY-SA-4.0", base: a.publish_id }, a.item_id);
  assert.equal(rp.status, 200, JSON.stringify(rp.body));
  assert.equal(w.q(`SELECT 1 FROM item_holds WHERE item_id = ?`, a.item_id).length, 0, "no blocklist hold on the original");
  assert.equal((await w.json("PATCH", `/items/${a.item_id}`, { user: A, json: { visibility: "public" } })).status, 200);
  assert.equal(w.q(`SELECT 1 FROM item_holds WHERE item_id = ?`, a.item_id).length, 0, "nor after switching back to public");
});

// ---- 5. the export never shows a private item's title to an importer -------------------------------

test("24-5: an importer's export shows no title of an item that is now unlisted, held or removed", async () => {
  const w = new World();
  const adm = w.user("admin"), Bob = w.user("teacher", "Bob"), Ann = w.user("teacher", "Ann");
  const meta = (title: string) => ({ title, subject: "Biology", course: "", grade: "", tags: [], standards: [] });
  const b1 = (await publish(w, Bob, bankOf([q("Bob's unlisted-in-time question about cirrus clouds and ice crystals?")]), { meta: meta("Bob's cirrus clouds item") })).body as any;
  const b2 = (await publish(w, Bob, bankOf([q("Bob's removed-in-time question about stratus clouds and drizzle?")]), { meta: meta("Bob's stratus clouds item") })).body as any;
  await imp(w, Ann, b1.item_id);
  await imp(w, Ann, b2.item_id);
  await w.json("PATCH", `/items/${b1.item_id}`, { user: Bob, json: { visibility: "unlisted" } });
  await w.admin(adm, "POST", "/admin/pool/remove", { item_id: b2.item_id, note: "privacy" });
  login(w, Ann);
  const r = await exp(w, Ann);
  assert.equal(r.status, 200);
  const rows = r.body.imports as { item_id: string; title: string | null }[];
  assert.deepEqual(rows.map((x) => x.item_id).sort(), [b1.item_id, b2.item_id].sort());
  assert.ok(rows.every((x) => x.title === null), "no title of a private or removed item");
  assert.ok(!r.text.includes("cirrus clouds") && !r.text.includes("stratus clouds"));
  // The author still sees her own titles.
  login(w, Bob);
  const br = await exp(w, Bob);
  assert.ok(br.body.items.some((i: any) => i.title.includes("cirrus clouds")));
});

// ---- 6. the export carries every retained version's body -------------------------------------------

test("24-6: the export includes the full body of every retained earlier version, not only the current one", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann");
  const HID = "Ann's hidden note for this version: the gauge crests on the second day after the storm.";
  let a: any = (await publish(w, A, bankOf([q("Ann's exported versioning question about snowmelt and runoff?", { explanation: HID })]), { hideKey: true })).body;
  const id = a.item_id;
  const ids: string[] = [a.publish_id];
  for (const [points, expl] of [[2, HID], [3, `${HID} A later revision of the note.`]] as [number, string][]) {
    w.advance(DAY);
    const r = await publish(w, A, bankOf(own(a, [q("Ann's exported versioning question about snowmelt and runoff?", { explanation: expl, points })])), { hideKey: true, base: a.publish_id }, id);
    assert.equal(r.status, 200, JSON.stringify(r.body));
    a = { ...r.body, item_id: id };
    ids.push(a.publish_id);
  }
  const retained = w.q<any>(`SELECT publish_id, full_key FROM item_publishes WHERE item_id = ? AND status IN ('current','previous')`, id);
  assert.ok(retained.length >= 2, "a current and at least one retained earlier version");
  const current = w.q<any>(`SELECT current_publish_id AS p FROM items WHERE id = ?`, id)[0].p as string;
  const earlier = retained.filter((v) => v.publish_id !== current);
  login(w, A);
  const r = await exp(w, A);
  assert.equal(r.status, 200);
  // The current body is `bodies[item]`; every OTHER retained version's body is in version_bodies.
  assert.deepEqual((r.body.version_bodies ?? {}) && Object.keys(r.body.version_bodies).sort(), earlier.map((x) => x.publish_id).sort());
  for (const v of earlier) {
    assert.ok(r.body.version_bodies[v.publish_id], `body of ${v.publish_id}`);
    const stored = JSON.parse(await (await w.r2.get(v.full_key))!.text());
    assert.deepEqual(r.body.version_bodies[v.publish_id], stored);
  }
  assert.ok(r.body.bodies[id], "the current body");
  // ... and a part of a large export carries them too.
  const saved = { ...EXPORT_LIMITS };
  EXPORT_LIMITS.whole = 1;
  EXPORT_LIMITS.part = 1024 * 1024;
  try {
    const big = await exp(w, A);
    assert.equal(big.body.export_too_large, true);
    const p = await w.call("GET", `/me/export?part=1`, { user: A });
    const pb = JSON.parse(await p.text());
    assert.ok(pb.version_bodies && Object.keys(pb.version_bodies).length >= 1);
  } finally {
    Object.assign(EXPORT_LIMITS, saved);
  }
});

// ---- 7. a pre-round-23 database never pins the wrong key ------------------------------------------

test("24-7: a pool populated before round 23 verifies the key before pinning: a wrong first key is refused and does not lock out the right one", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), adm = w.user("admin");
  const a = (await publish(w, A, bankOf([mcq(STEMS[0]!, [["A", true], ["B", false]], { explanation: EXPL })]), { license: "CC-BY-NC-4.0" })).body as any;
  assert.equal((await imp(w, M, a.item_id)).status, 200);
  // The pool as it was before round 23: identity data, no recorded key id.
  const right = await poolKeyId();
  w.sql.prepare(`DELETE FROM pool_kv WHERE key = 'pool_key_id' OR key LIKE 'key_mismatch_alerted:%'`).run();
  // A first write under another key must be refused AND must not pin that key.
  const w2 = new World({ sql: w.sql, r2: w.r2, core: makeCore(shareCore as any, { hashKey: "7a".repeat(32) }) });
  w2.tokens = w.tokens;
  (w2 as any).n = 50;
  w2.clock = w.clock;
  assert.equal(code(await publish(w2, M, bankOf([mcq("Mal's question about deltas?", [["A", true], ["B", false]], { explanation: EXPL })]), { license: "CC0-1.0" })), "503 pool_key_mismatch");
  // A bulk READ (item metadata, no hash) still answers under the mismatch; writes do not.
  assert.equal((await w2.json("POST", "/items/versions", { user: M, json: { ids: [a.item_id] } })).status, 200, "reads continue");
  assert.equal((await w2.json("DELETE", `/items/${a.item_id}`, { user: A })).status, 503, "DELETE still fails closed");
  assert.equal((await w2.json("POST", `/items/${a.item_id}/report`, { user: M, json: { reason: "other", note: "x" } })).status, 503, "report still fails closed");
  assert.equal(w.q(`SELECT 1 FROM pool_kv WHERE key = 'pool_key_id'`).length, 0, "the wrong key was not pinned");
  // The real key still works, and is the one recorded now (a fresh isolate: the first one has its
  // in-memory check cached).
  const w3 = new World({ sql: w.sql, r2: w.r2, core: makeCore(shareCore as any, { hashKey: TEST_HASH_KEY }) });
  w3.tokens = w.tokens;
  (w3 as any).n = 50;
  w3.clock = w.clock;
  assert.equal((await imp(w3, M, a.item_id)).status, 200, "the right key is not locked out");
  assert.deepEqual(w.q<any>(`SELECT value FROM pool_kv WHERE key = 'pool_key_id'`), [{ value: right }]);
  assert.equal((await w3.admin<any>(adm, "GET", "/admin/pool/key", null)).body.match, true);
});
