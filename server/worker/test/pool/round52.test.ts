// Round 52: circles phase C, the app's picture step after critic 62 (docs/critic-reports/2026-10-25-circles-phaseC-pictures-critic62.md).
// B1: PATCH .../banks/:bid {hide_key, ..., base_version}: the decisions (labels, order acknowledgements, formatting
// acknowledgements, the pictures kept) are by position, so the app pins the change to the version they were made on.
// A head that is not `base_version` is `409 conflict {head_version}` before anything is read, decided or charged; the
// batch's compare-and-set on the head it checked closes the rest (critic 57's suggestion too).
// I2: hiding on again with other pictures kept (the app's "Change picture choices") re-strips the head, as any change
// of decisions does, and counts toward the day's answer-key changes.
// I5: POST /circles/heads says `hide_key` for each bank it reports as current (and only those), so the app knows
// before a send whether the bank hides its keys.
import assert from "node:assert/strict";
import { test } from "node:test";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { framed, png, publishId, upload, World, type User } from "./harness.ts";

type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "SAMPLEKEYzzq" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "local", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const limit = (w: World, metric: string, value: number, who = "*") =>
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, ?, ?, 0, 'test') ON CONFLICT (user_id, metric) DO UPDATE SET value = excluded.value`).run(who, metric, value);
const used = (w: World, u: User, metric: string) => w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = ?`, u.id, metric)[0]?.count ?? 0;
const audits = (w: World, action: string) => w.q<any>(`SELECT id FROM pool_audit WHERE action = ?`, action).length;

async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][]): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name: "Algebra team" } })).body.circle.id;
  for (const [u, role] of members) {
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } })).body.code;
    assert.equal((await w.json("POST", "/circles/join", { user: u, json: { code: k } })).status, 200);
    w.advance(1000);
  }
  return cid;
}
async function sendRaw(w: World, u: User, cid: string, payload: string, o: { bid?: string; base?: number } = {}) {
  const meta: Q = {
    commit_id: publishId(),
    license: "circle-only",
    attest: { own_work: true, terms_version: POOL_TERMS_VERSION },
    meta: { title: "Unit 2", subject: "Math", course: "", grade: "", tags: [], standards: [] },
    ...(o.bid ? { base_version: o.base, confirm_large_change: true } : {}),
  };
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, payload), contentType: PUB });
}
const patch = (w: World, u: User, cid: string, bid: string, body: Q) => w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: u, json: body });
const get = (w: World, u: User, path: string) => w.json<any>("GET", path, { user: u });
const heads = async (w: World, u: User, ids: string[]) => (await w.json<any>("POST", "/circles/heads", { user: u, json: { bank_ids: ids } })).body.banks as Q[];
const img = (sha: string) => ({ file: `media/${sha}.png`, width: 4, height: 4 });

async function world() {
  const w = new World();
  const [O, A, E, V] = ["Olga", "Ann", "Eli", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"], [V!, "viewer"]]);
  return { w, O: O!, A: A!, E: E!, V: V!, cid };
}

/** Critic 62's c62-1: the author looks at v1 (a harmless picture on question 1); an editor then sends v2 with a new
 *  first question whose picture shows the answer. */
async function racedBank(w: World, A: User, E: User, cid: string) {
  const benign = await upload(w, A, png(4, 4, 1));
  const answer = await upload(w, E, png(4, 4, 2));
  const s1 = await sendRaw(w, A, cid, bankOf([q("Label the parts of the cell.", { id: "qa", image: img(benign.sha) }), q("Second question?", { id: "qb" })]));
  assert.equal(s1.status, 201, JSON.stringify(s1.body));
  const bid = s1.body.bank_id as string;
  const s2 = await sendRaw(w, E, cid, bankOf([q("What does the picture show?", { id: "qn", image: img(answer.sha) }), q("Label the parts of the cell.", { id: "qa", image: img(benign.sha) }), q("Second question?", { id: "qb" })]), { bid, base: 1 });
  assert.equal(s2.status, 200, JSON.stringify(s2.body));
  return { bid, benign: benign.sha, answer: answer.sha };
}

test("52-1 (critic 62 B1): a hide-key PATCH pinned to the version its decisions were made on is refused with 409 conflict when a newer version arrived, before any decision is read, applied or charged", async () => {
  const { w, A, E, V, cid } = await world();
  const { bid, benign, answer } = await racedBank(w, A, E, cid);
  // The author's PATCH made from v1's list: refused, the head it lost to named.
  const pinned = await patch(w, A, cid, bid, { hide_key: true, image_ok: [0], base_version: 1 });
  assert.deepEqual([pinned.status, pinned.body.error, pinned.body.head_version], [409, "conflict", 2], JSON.stringify(pinned.body));
  // Before any decision: one naming a question past the head's count is still a conflict, not a refusal for it.
  const far = await patch(w, A, cid, bid, { hide_key: true, image_ok: [7], labels: [{ question: 9, choice: "strip" }], base_version: 1 });
  assert.deepEqual([far.status, far.body.error], [409, "conflict"], JSON.stringify(far.body));
  // Nothing changed and nothing was charged.
  assert.equal(w.q<any>(`SELECT hide_key FROM circle_banks WHERE id = ?`, bid)[0].hide_key, 0);
  assert.equal(audits(w, "circle.bank_hide_key"), 0);
  assert.deepEqual([used(w, A, "circle_hide_changes"), used(w, A, "circle_hide_attempts")], [0, 0]);
  const v = await get(w, V, `/circles/${cid}/banks/${bid}`);
  assert.equal(v.body.bank.hide_key, false);
  // Pinned to the version the author now looked at: the picture she kept (now question 2) is kept, the new one is not.
  const ok = await patch(w, A, cid, bid, { hide_key: true, image_ok: [1], base_version: 2 });
  assert.equal(ok.status, 200, JSON.stringify(ok.body));
  assert.deepEqual([ok.body.images_kept, ok.body.images_removed], [[1], [0]]);
  const seen = await get(w, V, `/circles/${cid}/banks/${bid}`);
  assert.equal(seen.body.payload.bank.questions[0].image, null, "the picture the author never saw is left out");
  assert.equal(seen.body.payload.bank.questions[1].image.file, `media/${benign}.png`);
  assert.equal((await w.call("GET", `/circles/${cid}/banks/${bid}/blobs/${answer}`, { user: V })).status, 404);
  assert.equal((await w.call("GET", `/circles/${cid}/banks/${bid}/blobs/${benign}`, { user: V })).status, 200);
});

test("52-2 (critic 62 B1): base_version is a version number and goes only with hide_key; hiding off pinned to an older version is a conflict too", async () => {
  const { w, A, E, cid } = await world();
  const { bid } = await racedBank(w, A, E, cid);
  for (const bad of ["2", 0, -1, 1.5, null, true, 2 ** 40]) {
    const r = await patch(w, A, cid, bid, { hide_key: true, base_version: bad });
    assert.deepEqual([r.status, r.body.field], [400, "base_version"], `base_version ${JSON.stringify(bad)}`);
  }
  // with the licence, or alone: refused (the licence PATCH is pinned by its own compare-and-set)
  for (const body of [{ license: "circle-only", base_version: 2 }, { base_version: 2 }]) {
    const r = await patch(w, A, cid, bid, body);
    assert.deepEqual([r.status, r.body.field], [400, "base_version"], JSON.stringify(body));
  }
  assert.equal(used(w, A, "circle_hide_attempts"), 0, "a malformed request is not a try");
  // on, at the head: done; off at an older version: a conflict, the keys stay hidden; off at the head: done
  assert.equal((await patch(w, A, cid, bid, { hide_key: true, base_version: 2 })).status, 200);
  const stale = await patch(w, A, cid, bid, { hide_key: false, base_version: 1 });
  assert.deepEqual([stale.status, stale.body.error, stale.body.head_version], [409, "conflict", 2]);
  assert.equal(w.q<any>(`SELECT hide_key FROM circle_banks WHERE id = ?`, bid)[0].hide_key, 1);
  const off = await patch(w, A, cid, bid, { hide_key: false, base_version: 2 });
  assert.deepEqual([off.status, off.body.hide_key], [200, false]);
  // without base_version, as before
  assert.equal((await patch(w, A, cid, bid, { hide_key: true })).status, 200);
});

test("52-3 (critic 62 I2): with the keys hidden, hiding on again pinned to the head with other pictures kept changes which pictures viewers get, and counts toward the day's answer-key changes", async () => {
  const { w, A, V, cid } = await world();
  const pic = await upload(w, A, png(4, 4, 3));
  const s1 = await sendRaw(w, A, cid, bankOf([q("Label the parts of the cell.", { image: img(pic.sha) }), q("Second question?")]));
  const bid = s1.body.bank_id as string;
  const blob = () => w.call("GET", `/circles/${cid}/banks/${bid}/blobs/${pic.sha}`, { user: V }).then((r) => r.status);
  const kept = await patch(w, A, cid, bid, { hide_key: true, image_ok: [0], base_version: 1 });
  assert.deepEqual([kept.status, kept.body.images_kept], [200, [0]], JSON.stringify(kept.body));
  assert.equal((await get(w, V, `/circles/${cid}/banks/${bid}`)).body.payload.bank.questions[0].image.file, `media/${pic.sha}.png`);
  assert.equal(await blob(), 200);
  // "Change picture choices": the same setting, the picture now dropped (the app sends no image_ok)
  const dropped = await patch(w, A, cid, bid, { hide_key: true, base_version: 1 });
  assert.deepEqual([dropped.status, dropped.body.hide_key, dropped.body.images_kept, dropped.body.images_removed], [200, true, [], [0]], JSON.stringify(dropped.body));
  assert.equal((await get(w, V, `/circles/${cid}/banks/${bid}`)).body.payload.bank.questions[0].image, null);
  assert.equal(await blob(), 404);
  assert.deepEqual([audits(w, "circle.bank_hide_key"), audits(w, "circle.bank_strip_decisions")], [1, 1], "an activity line for turning it on; a record for the new decisions");
  assert.equal(used(w, A, "circle_hide_changes"), 2);
  // The same choices again change nothing and are free; a stale pin is a conflict, also free.
  assert.equal((await patch(w, A, cid, bid, { hide_key: true, base_version: 1 })).body.unchanged, true);
  assert.equal((await patch(w, A, cid, bid, { hide_key: true, image_ok: [0], base_version: 3 })).status, 409);
  assert.equal(used(w, A, "circle_hide_changes"), 2);
  // Past the day's changes: refused, nothing changes for viewers.
  limit(w, "circle_hide_changes", 2, A.id);
  const over = await patch(w, A, cid, bid, { hide_key: true, image_ok: [0], base_version: 1 });
  assert.deepEqual([over.status, over.body.error, over.body.metric], [429, "daily_limit", "circle_hide_changes"], JSON.stringify(over.body));
  assert.equal(await blob(), 404);
});

test("52-4 (critic 62 I5): POST /circles/heads says hide_key for a bank it reports as current, to every member who reads it, and for nothing else", async () => {
  const { w, O, A, E, V, cid } = await world();
  const hiding = (await sendRaw(w, A, cid, bankOf([q("One?")]))).body.bank_id as string;
  const showing = (await sendRaw(w, E, cid, bankOf([q("Two?")]))).body.bank_id as string;
  const gone = (await sendRaw(w, A, cid, bankOf([q("Three?")]))).body.bank_id as string;
  assert.equal((await patch(w, A, cid, hiding, { hide_key: true, base_version: 1 })).status, 200);
  assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${gone}`, { user: A })).status, 200);
  const nobody = "0".repeat(32);
  for (const u of [O, A, E, V]) {
    const hs = await heads(w, u, [hiding, showing, gone, nobody]);
    assert.deepEqual(hs.map((h) => [h.status, h.hide_key]), [["current", true], ["current", false], ["removed", undefined], ["unavailable", undefined]], u.name);
    assert.deepEqual(Object.keys(hs[0]!).sort(), ["at", "bank_id", "by", "hide_key", "license", "status", "version", "version_id"]);
  }
  // A member who left reads nothing of it.
  assert.equal((await w.json("POST", `/circles/${cid}/leave`, { user: V })).status, 200);
  assert.deepEqual(Object.keys((await heads(w, V, [hiding]))[0]!).sort(), ["bank_id", "status"]);
});
