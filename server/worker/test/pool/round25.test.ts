// Round 25 (critic-asvs-24, security). Three blockers in the round-24 work:
// 1. the upstream-original exemption lapsed after one ordinary republish (base publish newer than the
//    block), so the author's own original was DMCA-held;
// 2. the upstream exemption's "the taken-down item's author imported it" branch was forgeable with a
//    read/import of a copy (one GET shielded an accomplice's copy from hold-copies);
// 3. migration 0105 did not re-key licence windows of content no longer retained (no ghost mapping),
//    losing a public relicence for an earlier reader at upgrade time.
// Plus the held-publish cap, which charged an exempt upstream original.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { publish, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "x", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: [["A delta lobe of fresh sediment", true], ["A mesa of old rock", false], ["A butte", false]].map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const code = (r: { status: number; body: Q }) => (r.status >= 400 ? `${r.status} ${r.body.error}` : String(r.status));
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;

const STEM = "Ann's own question: in her 2019 delta survey, which landform formed first at the mouth of the Feather river?";
const OTHER = "Ann's other question about the flood plain of the lower river and its terraces?";
const THIRD = "Ann's third question about the sediment load carried by the river in spring?";

// ---- 1. the upstream exemption survives ordinary republishes --------------------------------------

test("25-1: the author's own original is never held by a takedown of a copy, through any number of ordinary republishes", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), E = w.user("teacher", "Eve");
  let a: any = (await publish(w, A, bankOf([mcq(STEM)]), { license: "CC-BY-SA-4.0" })).body;
  const id = a.item_id;
  w.advance(DAY);
  assert.equal((await imp(w, B, id)).status, 200);
  const b = (await publish(w, B, bankOf([mcq(STEM)]), { license: "CC-BY-SA-4.0" })).body as any;
  // The admin ticks the copied question (the default "all" selects nothing on a copied item).
  const t = await w.takedown<any>(adm, await notice(w, adm), b.item_id, { questions: [0] });
  assert.equal(t.status, 200, JSON.stringify(t.body));
  assert.ok(w.q(`SELECT 1 FROM blocked_hashes WHERE action_id = ?`, t.body.action_id).length > 0, "the takedown blocks material");
  assert.deepEqual(w.q(`SELECT 1 FROM item_holds WHERE item_id = ?`, id), [], "not held right after the takedown");
  for (let k = 0; k < 3; k++) {
    w.advance(60_000);
    const r = await publish(w, A, bankOf(own(a, [mcq(STEM)]), "final"), { license: "CC-BY-SA-4.0", base: a.publish_id }, id);
    assert.equal(r.status, 200, `republish ${k + 1}: ${JSON.stringify(r.body)}`);
    a = { ...r.body, item_id: id };
    assert.deepEqual(w.q(`SELECT source, reason FROM item_holds WHERE item_id = ?`, id), [], `not held after republish ${k + 1}`);
    assert.equal((await get(w, E, id)).status, 200, `visible after republish ${k + 1}`);
  }
  // ... while a genuine re-upload of blocked material by someone else is still held.
  const f = (await publish(w, E, bankOf([mcq(STEM)]), { license: "CC-BY-SA-4.0" })).body as any;
  assert.ok(w.q(`SELECT 1 FROM item_holds WHERE item_id = ?`, f.item_id).length > 0, "a fresh copy by another author is held");
});

// ---- 2. upstream-by-import/read is not forgeable ---------------------------------------------------

test("25-2: one import or GET of an accomplice's copy does not shield it from hold-copies", async () => {
  for (const how of ["import", "get"] as const) {
    const w = new World();
    const adm = w.user("admin"), A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob");
    const t = (await publish(w, A, bankOf([q(STEM)]), { license: "CC-BY-SA-4.0" })).body as any;
    w.advance(DAY);
    assert.equal((await imp(w, B, t.item_id)).status, 200);
    const v = (await publish(w, B, bankOf([q(STEM)]), { license: "CC-BY-SA-4.0" })).body as any;
    w.advance(DAY);
    // The infringer reads/imports the accomplice's copy: this used to be accepted as "upstream".
    const seen = how === "import" ? await imp(w, A, v.item_id) : await get(w, A, v.item_id);
    assert.equal(seen.status, 200, how);
    const td = await w.takedown<any>(adm, await notice(w, adm), t.item_id, { questions: "all" });
    assert.equal(td.status, 200, JSON.stringify(td.body));
    assert.equal((await w.holdCopies<any>(adm, td.body.action_id)).status, 200);
    const held = w.q<any>(`SELECT item_id FROM item_holds WHERE source = ?`, `dmca:${td.body.action_id}`).map((x) => x.item_id);
    assert.ok(held.includes(v.item_id), `${how}: the accomplice's copy is held`);
  }
  // The genuine upstream original (Bob copies Ann's item) is still exempt (s5/24-4 behaviour).
  const w = new World();
  const adm = w.user("admin"), A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob");
  const a = (await publish(w, A, bankOf([q(STEM)]), { license: "CC-BY-SA-4.0" })).body as any;
  w.advance(DAY);
  await imp(w, B, a.item_id);
  const b = (await publish(w, B, bankOf([q(STEM)]), { license: "CC-BY-SA-4.0" })).body as any;
  const td = await w.takedown<any>(adm, await notice(w, adm), b.item_id, { questions: [0] });
  assert.equal((await w.holdCopies<any>(adm, td.body.action_id)).status, 200);
  const held = w.q<any>(`SELECT item_id FROM item_holds WHERE source = ?`, `dmca:${td.body.action_id}`).map((x) => x.item_id);
  assert.ok(!held.includes(a.item_id), "the true upstream original is not held");
});

// ---- 3. no licence window is lost when its content is no longer retained ---------------------------

const MIGRATION = new URL("../../migrations/0105_pool_round24.sql", import.meta.url).pathname;
/** The data statements of 0105's re-key (comments/ALTER/views stripped), as an upgrade would run them. */
function rekeyStatements(): string[] {
  const sql = readFileSync(MIGRATION, "utf8")
    .split("\n")
    .filter((l) => !l.trimStart().startsWith("--"))
    .join("\n");
  return sql
    .split(";")
    .map((s) => s.trim())
    .filter((s) => /^INSERT|^DELETE/.test(s));
}
/** Ann's v1 (NC), Mal imports; v2 is the same exact content publicly relicensed CC0; v3/v4 retire v1/v2. */
async function retiredWindow(w: World, A: User, M: User) {
  const v1 = (await publish(w, A, bankOf([mcq(STEM)]), { license: "CC-BY-NC-4.0" })).body as any;
  const id = v1.item_id;
  const exact = w.q<any>(`SELECT full_content FROM item_questions WHERE publish_id = ?`, v1.publish_id)[0].full_content as string;
  w.advance(DAY);
  await imp(w, M, id);
  w.advance(DAY);
  const v2 = (await publish(w, A, bankOf(own(v1, [mcq(STEM)])), { license: "CC0-1.0", base: v1.publish_id }, id)).body as any;
  const canon = w.q<any>(`SELECT ckey FROM content_licences WHERE item_id = ? AND license = 'CC0-1.0' AND ckey LIKE 'q:%'`, id)[0].ckey as string;
  assert.notEqual(canon, `q:${exact}`);
  w.advance(DAY);
  const v3 = (await publish(w, A, bankOf(own(v2, [q(OTHER)])), { license: "CC0-1.0", base: v2.publish_id }, id)).body as any;
  w.advance(DAY);
  await publish(w, A, bankOf(own(v3, [q(THIRD)])), { license: "CC0-1.0", base: v3.publish_id }, id);
  const retained = w.q<any>(`SELECT publish_id FROM item_publishes WHERE item_id = ? AND status IN ('current','previous')`, id).map((x) => x.publish_id);
  assert.ok(!retained.includes(v1.publish_id) && !retained.includes(v2.publish_id), "v1/v2 are not retained");
  assert.ok(w.q(`SELECT 1 FROM ghost_publishes WHERE item_id = ?`, id).length > 0, "their ghost keeps the content");
  // Simulate pre-0105 rows: the licence history was keyed by the EXACT content hash.
  const n = w.sql.prepare(`UPDATE content_licences SET ckey = 'q:' || ? WHERE item_id = ? AND ckey = ?`).run(exact, id, canon);
  assert.ok(n.changes >= 2, "both exact-keyed rows");
  w.sql.prepare(`UPDATE item_offer SET ckey = 'q:' || ? WHERE item_id = ? AND ckey = ?`).run(exact, id, canon);
  return { id, exact, canon };
}

test("25-3: migration 0105 re-keys a licence window through the ghost of a version no longer retained", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
  await retiredWindow(w, A, M);
  for (const s of rekeyStatements()) w.sql.exec(s + ";");
  const rows = w.q<any>(`SELECT licenses_json FROM import_questions WHERE user_id = ? AND content_hash IS NOT NULL`, M.id);
  assert.ok(rows.flatMap((r) => JSON.parse(r.licenses_json ?? "[]")).includes("CC0-1.0"), "the view grants the relicence");
  assert.equal(code(await publish(w, M, bankOf([mcq(STEM)]), { license: "CC0-1.0" })), "201", "the earlier reader is not refused");
  // Idempotent: a second run changes nothing.
  for (const s of rekeyStatements()) w.sql.exec(s + ";");
});

test("25-4: the hourly licence_rekey stage finishes the re-key after the backfill (migration-time NULL near hashes)", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
  const { exact } = await retiredWindow(w, A, M);
  await w.pool.hourly();
  const rows = w.q<any>(`SELECT licenses_json FROM import_questions WHERE user_id = ? AND content_hash IS NOT NULL`, M.id);
  assert.ok(rows.flatMap((r) => JSON.parse(r.licenses_json ?? "[]")).includes("CC0-1.0"), "the cron re-keyed the window");
  assert.equal(code(await publish(w, M, bankOf([mcq(STEM)]), { license: "CC0-1.0" })), "201");
  assert.equal(w.q(`SELECT 1 FROM content_licences WHERE ckey = ?`, `q:${exact}`).length, 0, "no old exact-keyed row survives");
});

// ---- addendum (moodle-24): the derivation survives a purge with no serve --------------------------

test("25-6: a copy purged after its uphold with NO serve still identifies the original on her next republish", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob");
  let a: any = (await publish(w, A, bankOf([mcq(STEM)]), { license: "CC-BY-NC-4.0" })).body;
  const id = a.item_id;
  w.advance(DAY);
  assert.equal((await imp(w, B, id)).status, 200);
  const b = (await publish(w, B, bankOf([mcq(STEM)]), { license: "CC-BY-NC-4.0" })).body as any;
  w.advance(DAY);
  // Bob's copy is never read or imported: it will leave no ghost when it is purged.
  await w.json("PATCH", `/items/${id}`, { user: A, json: { visibility: "unlisted" } });
  const t = await w.takedown<any>(adm, await notice(w, adm), b.item_id, { questions: [0] });
  assert.equal(t.status, 200, JSON.stringify(t.body));
  for (let d = 0; d < 31; d++) {
    w.advance(DAY);
    await w.pool.hourly();
    await w.pool.daily();
  }
  assert.equal(w.q(`SELECT 1 FROM ghost_publishes WHERE item_id = ?`, b.item_id).length, 0, "no serve, no ghost");
  assert.equal(w.q(`SELECT 1 FROM item_publishes WHERE item_id = ?`, b.item_id).length, 0, "the copy is gone");
  assert.ok(w.q(`SELECT 1 FROM dmca_actions WHERE id = ? AND derivation_json IS NOT NULL`, t.body.action_id).length === 1, "the derivation is on the action");
  w.advance(60_000);
  const r = await publish(w, A, bankOf(own(a, [{ ...mcq(STEM), points: 3 }])), { license: "CC-BY-NC-4.0", base: a.publish_id }, id);
  assert.equal(r.status, 200, JSON.stringify(r.body));
  assert.deepEqual(w.q(`SELECT reason, source FROM item_holds WHERE item_id = ?`, id), [], "the original is not held");
  const mine = (await w.json<any>("GET", "/items/mine", { user: A })).body.items.find((x: any) => x.id === id);
  assert.equal(mine.state, "live");
});

// ---- addendum (moodle-24): legacy stem records are backfilled -------------------------------------

test("25-7: a pre-round-24 stem record (NULL full explanation key, or missing entirely) never credits the copier", async () => {
  const S1 = "Ann's own question: in her 2019 delta survey, which landform formed first at the mouth of the Fether river?";
  const Q = (stem: string, x: Q = {}) => mcq(stem, { explanation: "Ann's own measured explanation of the 2019 delta survey and its regressions over the season.", ...x });
  for (const sim of ["null", "delete"] as const) {
    const w = new World();
    const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), E = w.user("teacher", "Eve");
    assert.equal((await publish(w, A, bankOf([Q(S1)]), { license: "CC-BY-4.0", hideKey: true })).status, 201);
    if (sim === "null") w.sql.prepare(`UPDATE author_stems SET bk_expl_full = NULL`).run();
    else w.sql.prepare(`DELETE FROM author_stems`).run();
    w.advance(DAY);
    const hidden = w.q<any>(`SELECT item_id FROM item_publishes WHERE hide_key = 1 AND status = 'current'`)[0].item_id as string;
    assert.equal((await imp(w, B, hidden)).status, 200);
    assert.equal((await publish(w, B, bankOf([Q(STEM)]), { license: "CC-BY-4.0", hideKey: true })).status, 201, "the copier edits first");
    w.advance(DAY);
    // Her later SHOWN version of the same question (with the same explanation) in a second bank.
    const n = await publish(w, A, bankOf([Q(STEM)], "Final exam"), { license: "CC-BY-4.0" });
    assert.equal(n.status, 201, JSON.stringify(n.body));
    assert.deepEqual((await get(w, E, n.body.item_id as string)).body.questions[0].chain, [], `${sim}: no credit to the copier`);
  }
});

// ---- addendum (moodle-24): unmappable rows cannot block the re-key queue ---------------------------

test("25-8: 1,000 unmappable old-style rows do not stop the re-key from reaching a mappable row", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann");
  const a = (await publish(w, A, bankOf([mcq(STEM)]), { license: "CC-BY-NC-4.0" })).body as any;
  const exact = w.q<any>(`SELECT full_content FROM item_questions WHERE publish_id = ?`, a.publish_id)[0].full_content as string;
  const canon = w.q<any>(`SELECT ckey FROM content_licences WHERE item_id = ? AND license = 'CC-BY-NC-4.0' AND ckey LIKE 'q:%'`, a.item_id)[0].ckey as string;
  for (let i = 0; i < 1000; i++) w.sql.prepare(`INSERT INTO content_licences (item_id, ckey, license, from_at) VALUES (?, ?, 'CC0-1.0', ?)`).run("f".repeat(32), `q:${i.toString(16).padStart(64, "0")}`, 1000 + i);
  // One mappable old-style row, behind the 1,000 in rowid order.
  w.sql.prepare(`INSERT INTO content_licences (item_id, ckey, license, from_at) VALUES (?, ?, 'CC-BY-4.0', ?)`).run(a.item_id, `q:${exact}`, 2000);
  w.sql.prepare(`DELETE FROM content_licences WHERE item_id = ? AND ckey = ?`, a.item_id, canon).run();
  await w.pool.hourly();
  assert.equal(w.q(`SELECT 1 FROM content_licences WHERE item_id = ? AND ckey = ?`, a.item_id, `q:${exact}`).length, 0, "the mappable row is re-keyed");
  assert.ok(w.q(`SELECT 1 FROM content_licences WHERE item_id = ? AND ckey = ? AND license = 'CC-BY-4.0'`, a.item_id, canon).length === 1, "its canonical window exists");
  assert.equal(w.q(`SELECT 1 FROM content_licences WHERE item_id = ? AND ckey = ?`, "f".repeat(32), `q:${(0).toString(16).padStart(64, "0")}`).length, 1, "unmappable rows stay at rest");
});


// ---- non-blocker: the held-publish cap never charges an exempt upstream original --------------------

test("25-5: 22 ordinary republishes past the held-publish cap never hold the author's own original", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), C = w.user("teacher", "Cara");
  let a: any = (await publish(w, A, bankOf([mcq(STEM)]), { license: "CC-BY-SA-4.0" })).body;
  const id = a.item_id;
  w.advance(DAY);
  await imp(w, B, id);
  const b = (await publish(w, B, bankOf([mcq(STEM)]), { license: "CC-BY-SA-4.0" })).body as any;
  w.advance(DAY);
  await imp(w, C, b.item_id);
  await publish(w, C, bankOf([mcq(STEM)]), { license: "CC-BY-SA-4.0" });
  const t = await w.takedown<any>(adm, await notice(w, adm), b.item_id, { questions: [0] });
  assert.equal(t.status, 200, JSON.stringify(t.body));
  assert.equal((await w.holdCopies<any>(adm, t.body.action_id)).status, 200);
  for (let k = 0; k < 22; k++) {
    w.advance(60_000);
    const r = await publish(w, A, bankOf(own(a, [mcq(STEM)]), "final"), { license: "CC-BY-SA-4.0", base: a.publish_id }, id);
    assert.equal(r.status, 200, `republish ${k + 1}: ${JSON.stringify(r.body)}`);
    a = { ...r.body, item_id: id };
    assert.deepEqual(w.q(`SELECT source, reason FROM item_holds WHERE item_id = ?`, id), [], `held after republish ${k + 1}`);
  }
  assert.equal(w.q(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'held_publishes'`, A.id).length, 0, "the exempt original is never charged");
});
