// Round 54: circles phase D2, the per-question change lists (docs/circles-phaseD-plan.md section 7).
// GET /circles/:cid/banks/:bid/changes: kept, readable versions newest first, each compared with the next one
// below it, so the app can show a question's own history without downloading every version.
import assert from "node:assert/strict";
import { test } from "node:test";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { framed, publishId, World, type User } from "./harness.ts";

type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "local", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const used = (w: World, u: User, metric: string) => w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = ?`, u.id, metric)[0]?.count ?? 0;

/** A question as the circle holds it: `qid:<qid>:<stem>` re-sends the head's question with that qid. */
const toQ = (x: string, bid?: string): Q => {
  if (!x.startsWith("qid:")) return q(x);
  const [, qid, ...rest] = x.split(":");
  return q(rest.join(":"), { published: { item_id: bid, qid, publish_id: "p", version: 1 } });
};
async function send(w: World, u: User, cid: string, items: string[], o: { bid?: string; base?: number } = {}) {
  const meta: Q = {
    commit_id: publishId(),
    license: "circle-only",
    attest: { own_work: true, terms_version: POOL_TERMS_VERSION },
    meta: { title: "Unit 2", subject: "Math", course: "", grade: "", tags: [], standards: [] },
    ...(o.bid ? { base_version: o.base } : {}),
  };
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, bankOf(items.map((x) => toQ(x, o.bid)))), contentType: PUB });
}
async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][]): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name: "Algebra team" } })).body.circle.id;
  for (const [u, role] of members) {
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } })).body.code;
    assert.equal((await w.json("POST", "/circles/join", { user: u, json: { code: k } })).status, 200);
    w.advance(1000);
  }
  return cid;
}
const changes = (w: World, u: User | null, cid: string, bid: string, headers: Record<string, string> = {}) =>
  w.call("GET", `/circles/${cid}/banks/${bid}/changes`, { user: u, headers });
const versionBody = async (w: World, u: User, cid: string, bid: string, n: number): Promise<Q[]> => {
  const r = await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/${n}`, { user: u });
  assert.equal(r.status, 200, JSON.stringify(r.body));
  return r.body.payload.bank.questions as Q[];
};
/** The version's questions keyed by the qids its send answered with (the body itself carries no ids). */
function byQid(qs: Q[], qids: string[]) {
  assert.equal(qs.length, qids.length, "the body and its send's qids line up");
  return new Map(qs.map((x, i) => [qids[i]!, JSON.stringify(Object.fromEntries(Object.entries(x).filter(([k]) => k !== "published" && k !== "id").sort(([a], [b]) => a.localeCompare(b))))]));
}
/** The brute-force answer: what changed between two versions' bodies, by qid. */
function brute(older: Map<string, string>, newer: Map<string, string>) {
  const added = [...newer.keys()].filter((k) => !older.has(k));
  const changed = [...newer.keys()].filter((k) => older.has(k) && older.get(k) !== newer.get(k));
  const removed = [...older.keys()].filter((k) => !newer.has(k));
  return [added, changed, removed].map((l) => l.sort());
}
const listsOf = (entry: Q) => [entry.added, entry.changed, entry.removed].map((l: string[]) => [...l].sort());
const jsonBody = async (r: Response) => JSON.parse(await r.text()) as Q;

test("54-1: every list equals a brute-force diff of the two versions' bodies", async () => {
  const w = new World();
  const O = w.user("teacher", "Ola");
  const cid = await circleWith(w, O, []);
  const v1 = await send(w, O, cid, ["First question about cells and their parts?", "Second question about enzyme activity?"]);
  const bid = v1.body.bank_id;
  const q1 = v1.body.qids as string[];
  // v2 changes the second question and adds one; v3 removes the first and adds another.
  const v2 = await send(w, O, cid, [`qid:${q1[0]}:First question about cells and their parts?`, `qid:${q1[1]}:Second question about enzyme activity, reworded?`, "Third question about osmosis?"], { bid, base: 1 });
  assert.equal(v2.status, 200, JSON.stringify(v2.body));
  const q2 = v2.body.qids as string[];
  const v3 = await send(w, O, cid, [`qid:${q2[1]}:Second question about enzyme activity, reworded?`, `qid:${q2[2]}:Third question about osmosis?`, "Fourth question about diffusion?"], { bid, base: 2 });
  assert.equal(v3.status, 200, JSON.stringify(v3.body));
  const bodies = [byQid(await versionBody(w, O, cid, bid, 1), q1), byQid(await versionBody(w, O, cid, bid, 2), q2), byQid(await versionBody(w, O, cid, bid, 3), v3.body.qids)];
  const r = await changes(w, O, cid, bid);
  assert.equal(r.status, 200);
  const body = await jsonBody(r);
  assert.equal(body.per_question, true);
  // Newest first; the oldest in the window has no base inside it, so two entries for three versions.
  assert.deepEqual(body.versions.map((x: Q) => [x.version, x.compared_to]), [[3, 2], [2, 1]]);
  for (const e of body.versions) assert.deepEqual(listsOf(e), brute(bodies[e.compared_to - 1]!, bodies[e.version - 1]!), `version ${e.version}`);
  // The shapes the plan names: v2 changed one question and added one; v3 removed one and added one.
  assert.deepEqual([body.versions[1].changed.length, body.versions[1].added.length, body.versions[1].removed.length], [1, 1, 0]);
  assert.deepEqual([body.versions[0].changed.length, body.versions[0].added.length, body.versions[0].removed.length], [0, 1, 1]);
});

test("54-2: withheld and pruned versions have no list and are never a comparison base", async () => {
  const w = new World();
  const O = w.user("teacher", "Ola");
  const cid = await circleWith(w, O, []);
  const s = await send(w, O, cid, ["First question about igneous rock?", "Second question about sedimentary rock?"]);
  const bid = s.body.bank_id;
  const q1 = s.body.qids as string[];
  await send(w, O, cid, [`qid:${q1[0]}:First question about igneous rock?`, `qid:${q1[1]}:Second question about sedimentary rock, reworded?`], { bid, base: 1 });
  await send(w, O, cid, [`qid:${q1[0]}:First question about igneous rock, reworded?`, `qid:${q1[1]}:Second question about sedimentary rock, reworded?`], { bid, base: 2 });
  // v2 is withheld (a student-data flag): absent, and v3 compares with v1.
  w.sql.prepare(`UPDATE circle_versions SET withheld_at = ? WHERE bank_id = ? AND version = 2`).run(w.clock, bid);
  let body = await jsonBody(await changes(w, O, cid, bid));
  assert.deepEqual(body.versions.map((x: Q) => x.version), [3]);
  assert.equal(body.versions[0].compared_to, 1);
  // v3 is compared with v1: both questions were reworded since it (v2 is never a base).
  assert.deepEqual(listsOf(body.versions[0]), [[], [q1[0]!, q1[1]!].sort(), []]);
  // A pruned v1 is absent too: v3 then has no base inside the window and is left out.
  w.sql.prepare(`UPDATE circle_versions SET status = 'pruned' WHERE bank_id = ? AND version = 1`).run(bid);
  body = await jsonBody(await changes(w, O, cid, bid));
  assert.deepEqual(body.versions, []);
});

test("54-3: a viewer of a bank that hides its keys gets no lists; a non-member or former member gets 404", async () => {
  const w = new World();
  const O = w.user("teacher", "Ola"), E = w.user("teacher", "Eli"), V = w.user("teacher", "Vic"), X = w.user("teacher", "Xan");
  const cid = await circleWith(w, O, [[E, "editor"], [V, "viewer"]]);
  const s = await send(w, O, cid, ["First question about plate tectonics?", "Second question about seismic waves?"]);
  const bid = s.body.bank_id;
  // Hiding on: the viewer's answer is the plan's Q4 rule, and nothing is read or counted for it.
  const before = used(w, V, "circle_change_reads");
  assert.equal((await w.json("PATCH", `/circles/${cid}/banks/${bid}`, { user: O, json: { hide_key: true } })).status, 200);
  const r = await jsonBody(await changes(w, V, cid, bid));
  assert.deepEqual([r.per_question, r.reason, r.versions], [false, "keys_hidden", undefined]);
  assert.equal(used(w, V, "circle_change_reads"), before, "nothing counted for the refusal");
  // The editor still gets the lists, and an author demoted to viewer is treated as a viewer.
  assert.equal((await jsonBody(await changes(w, E, cid, bid))).per_question, true);
  w.sql.prepare(`UPDATE circle_members SET role = 'viewer' WHERE circle_id = ? AND user_id = ?`).run(cid, E.id);
  assert.equal((await jsonBody(await changes(w, E, cid, bid))).per_question, false);
  // A non-member and a signed-out reader get the counted 404 / 401, not a shape.
  assert.equal((await changes(w, X, cid, bid)).status, 404);
  assert.equal((await changes(w, null, cid, bid)).status, 401);
  w.sql.prepare(`UPDATE circle_members SET role = 'viewer', left_at = ?, left_reason = 'left' WHERE circle_id = ? AND user_id = ?`).run(w.clock, cid, V.id);
  assert.equal((await changes(w, V, cid, bid)).status, 404);
});

test("54-4: counted once, and a 304 from the ETag is not counted", async () => {
  const w = new World();
  const O = w.user("teacher", "Ola");
  const cid = await circleWith(w, O, []);
  const s = await send(w, O, cid, ["First question about glaciers?", "Second question about moraines?"]);
  const bid = s.body.bank_id;
  const r1 = await changes(w, O, cid, bid);
  assert.equal(r1.status, 200);
  assert.equal(used(w, O, "item_reads"), 1);
  assert.equal(used(w, O, "circle_change_reads"), 1);
  const etag = r1.headers.get("ETag");
  assert.ok(etag, "the response carries an ETag");
  const r2 = await changes(w, O, cid, bid, { "If-None-Match": etag! });
  assert.equal(r2.status, 304);
  assert.equal(used(w, O, "item_reads"), 1, "a 304 is not a read");
  assert.equal(used(w, O, "circle_change_reads"), 1);
  // A new version changes the ETag, so the next read is served and counted.
  const q1 = s.body.qids as string[];
  await send(w, O, cid, [`qid:${q1[0]}:First question about glaciers?`, `qid:${q1[1]}:Second question about moraines, reworded?`], { bid, base: 1 });
  const r3 = await changes(w, O, cid, bid, { "If-None-Match": etag! });
  assert.equal(r3.status, 200);
  assert.notEqual(r3.headers.get("ETag"), etag);
  assert.equal(used(w, O, "item_reads"), 2);
});

test("54-5: at most 40 versions are compared, newest first, and a missing index is a gap, never an error", async () => {
  const w = new World();
  const O = w.user("teacher", "Ola");
  const cid = await circleWith(w, O, []);
  const s = await send(w, O, cid, ["The bank whose history is long?"]);
  const bid = s.body.bank_id;
  const v1 = w.q<any>(`SELECT * FROM circle_versions WHERE bank_id = ?`, bid)[0];
  const bodyText = await (await w.r2.get(v1.full_key))!.text();
  const idxText = await (await w.r2.get(v1.index_key))!.text();
  const hex = (n: number) => (7).toString(16).padStart(4, "0") + n.toString(16).padStart(28, "0");
  const ins = w.sql.prepare(
    `INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, result_json, committer_id, base_version, message, license, status, full_key, index_key, bytes, question_count, added, changed, removed, seq, created_at, committed_at)
     VALUES (?, ?, ?, ?, ?, 'x', '{}', ?, ?, ?, 'circle-only', ?, ?, ?, 1000, 1, 0, 1, 0, 1, 1, ?)`,
  );
  const puts: [string, string][] = [];
  w.sql.exec("BEGIN");
  w.sql.prepare(`UPDATE circle_versions SET status = 'pruned', full_key = '', index_key = '' WHERE id = ?`).run(v1.id);
  let headId = "";
  for (let v = 2; v <= 60; v++) {
    const id = hex(v);
    const kept = v > 15; // 45 kept versions: more than the route reads, so the cap bites
    const fk = kept ? `circles/${cid}/${bid}/${id}.json` : "";
    const ik = kept ? `circles/${cid}/${bid}/${id}.idx.json` : "";
    ins.run(id, bid, cid, v, `commit-${id}`, O.id, v - 1, `message ${v}`, v === 60 ? "current" : kept ? "previous" : "pruned", fk, ik, v * 1000);
    if (kept) puts.push([fk, bodyText], [ik, idxText]);
    headId = id;
  }
  w.sql.prepare(`UPDATE circle_banks SET head_version = 60, head_version_id = ? WHERE id = ?`).run(headId, bid);
  w.sql.exec("COMMIT");
  for (const [k, v] of puts) await w.r2.put(k, v);
  const body = await jsonBody(await changes(w, O, cid, bid));
  // 45 readable kept versions, but the route reads 40 indexes: 39 comparisons, the oldest read left out.
  assert.equal(body.versions.length, 39);
  assert.deepEqual(body.versions.map((x: Q) => x.version), Array.from({ length: 39 }, (_, i) => 60 - i));
  assert.ok(body.versions.every((x: Q) => x.gap !== true && Array.isArray(x.added)), "every entry has its lists");
  // Remove one index from R2: that version, and the one that would compare with it, are gaps.
  const victim = w.q<any>(`SELECT index_key FROM circle_versions WHERE bank_id = ? AND version = 50`, bid)[0];
  await w.r2.delete(victim.index_key);
  const gap = await jsonBody(await changes(w, O, cid, bid, { "If-None-Match": "no-such-tag" }));
  const at = (v: number) => gap.versions.find((x: Q) => x.version === v);
  assert.equal(at(50).gap, true, "its own index is gone");
  assert.equal(at(51).gap, true, "its base is gone");
  assert.equal(at(52).gap, undefined, "further up the lists still work");
});
