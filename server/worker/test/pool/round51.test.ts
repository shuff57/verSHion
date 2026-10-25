// Round 51: circles phase C after critic 61 (docs/critic-reports/2026-10-25-circles-phaseC-final-critic61.md).
// I2: the owner's (or the author's) read of a withheld version that turning hiding off may release no longer waits or
// queues for the isolate's one heavy slot: the full-form check runs only when the slot is free at once (a busy slot
// says false at once, and the read never takes a place in the queue, so other teachers' publishes are not turned away),
// and each check that runs is charged to `circle_hide_checks` (past it, false without a check).
// M1: of two hide-on PATCHes at once on one head, only the one whose batch withheld it answers `withheld: true`.
import assert from "node:assert/strict";
import { test } from "node:test";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { acquireHeavy } from "../../src/pool/publish.ts";
import { framed, publish, publishId, World, type User } from "./harness.ts";

type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "SAMPLEKEYzzq" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "local", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const STEM = "Copyrighted textbook problem 4.7: which organelle in the eukaryotic cell produces most of the ATP in respiration?";
const LAB = () => mcq(STEM, [["A. Mitochondrion", true], ["B. Ribosome", false], ["C. Golgi body", false], ["D. Lysosome", false]]);
const UNL = () => mcq(STEM, [["Mitochondrion", true], ["Ribosome", false], ["Golgi body", false], ["Lysosome", false]]);
const CLEAN = "Clean question about eskers and drumlins and the glaciers that left them?";
const STUDENT = "Jenny Smith in period 3 scored 42 on the unit test: which of these is the class median?";
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const limit = (w: World, metric: string, value: number, who = "*") =>
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, ?, ?, 0, 'test') ON CONFLICT (user_id, metric) DO UPDATE SET value = excluded.value`).run(who, metric, value);
const used = (w: World, u: User, metric: string) => w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = ?`, u.id, metric)[0]?.count ?? 0;
async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][], name = "Algebra team"): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name } })).body.circle.id;
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
type Slots = { active: number; queue: (() => void)[] };
async function world(o: { heavySlots?: Slots; heavyQueueMs?: number } = {}) {
  const w = new World(o as never);
  const adm = w.user("admin");
  const [O, A, V, P] = ["Olga", "Ann", "Vic", "Pat"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [V!, "viewer"]]);
  return { w, adm, O: O!, A: A!, V: V!, P: P!, cid };
}
async function takedownOf(w: World, adm: User, P: User, qs: Q[]) {
  const item = (await publish(w, P, bankOf(qs))).body as Q;
  const td = await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: qs.map((_, i) => i) });
  assert.equal(td.status, 200, JSON.stringify(td.body));
}
/** A bank whose head an answer-key PATCH withheld for a copyright match of its viewers' form only (a candidate). */
async function candidate(w: World, adm: User, A: User, P: User, cid: string): Promise<string> {
  const bid = (await sendRaw(w, A, cid, bankOf([LAB()]))).body.bank_id;
  await takedownOf(w, adm, P, [UNL()]);
  const on = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "strip" }] });
  assert.equal(on.body.withheld, true, JSON.stringify(on.body));
  return bid;
}

test("51-1 (critic 61 I2): with the heavy slot taken, owner and author reads answer at once, take no place in its queue, and another teacher's publish is not turned away", async () => {
  const slots: Slots = { active: 0, queue: [] };
  const { w, adm, O, A, P, cid } = await world({ heavySlots: slots, heavyQueueMs: 3000 });
  const bid = await candidate(w, adm, A, P, cid);
  const head = `/circles/${cid}/banks/${bid}`;
  // Idle: the check runs and says true.
  const idle = await get(w, O, head);
  assert.deepEqual([idle.status, idle.body.error, idle.body.hide_off_releases], [404, "version_withheld", true]);
  // Every place ever taken in the heavy queue is counted.
  let queued = 0;
  const push = slots.queue.push.bind(slots.queue);
  slots.queue.push = (...xs: (() => void)[]) => ((queued += xs.length), push(...xs));
  // Something else (a publish, a send) holds the slot.
  const rel = await acquireHeavy(3000, async () => {}, slots as never);
  try {
    const t = Date.now();
    const out = await Promise.all([get(w, O, head), get(w, O, head), get(w, A, head), get(w, A, head), get(w, O, `${head}/versions/1`)]);
    // (they answered while the slot was still taken: a read that waited for it would have taken 3000 ms, then false)
    assert.ok(Date.now() - t < 2500, `the reads answered at once (${Date.now() - t} ms)`);
    assert.equal(queued, 0, "no read took a place in the heavy queue");
    for (const r of out) assert.deepEqual([r.status, r.body.error, r.body.hide_off_releases], [404, "version_withheld", false], JSON.stringify(r.body));
    // Nothing was charged for a check that did not run.
    assert.equal(used(w, O, "circle_hide_checks"), 1);
    assert.equal(used(w, A, "circle_hide_checks"), 0);
    // Owner reads go on meanwhile; another teacher's publish waits its turn in the queue instead of a 503.
    const pub = publish(w, w.user("teacher", "Quinn"), bankOf([q("An unrelated clean question about tectonic plates and their speeds?")]));
    const rs = await Promise.all([get(w, O, head), get(w, A, head), get(w, O, head), get(w, A, head)]);
    for (const r of rs) assert.deepEqual([r.status, r.body.hide_off_releases], [404, false]);
    for (let i = 0; i < 200 && queued === 0; i++) await new Promise((r) => setTimeout(r, 10));
    assert.equal(queued, 1, "only the publish waited for the slot");
    rel();
    const p = await pub;
    assert.equal(p.status, 201, JSON.stringify(p.body));
  } finally {
    rel();
  }
  // Free again: the check runs again.
  const again = await get(w, A, head);
  assert.equal(again.body.hide_off_releases, true);
});

test("51-2 (critic 61 I2): each check a read runs is charged to circle_hide_checks; past it the read says false without a check; reads that check nothing are not charged", async () => {
  const { w, adm, O, A, V, P, cid } = await world();
  const bid = await candidate(w, adm, A, P, cid);
  const head = `/circles/${cid}/banks/${bid}`;
  limit(w, "circle_hide_checks", 2, O.id);
  const seen: unknown[] = [];
  for (let i = 0; i < 4; i++) seen.push((await get(w, O, head)).body.hide_off_releases);
  assert.deepEqual(seen, [true, true, false, false]);
  assert.equal(used(w, O, "circle_hide_checks"), 2);
  // The author has a budget of its own.
  assert.equal((await get(w, A, head)).body.hide_off_releases, true);
  assert.equal(used(w, A, "circle_hide_checks"), 1);
  // A viewer is told nothing and charged nothing; a readable bank's read checks nothing.
  const v = await get(w, V, head);
  assert.equal("hide_off_releases" in v.body, false);
  assert.equal(used(w, V, "circle_hide_checks"), 0);
  const b2 = (await sendRaw(w, A, cid, bankOf([q(CLEAN)]))).body.bank_id;
  assert.equal((await get(w, A, `/circles/${cid}/banks/${b2}`)).status, 200);
  assert.equal(used(w, A, "circle_hide_checks"), 1);
  // Turning hiding off still checks and releases (its own budget), whatever the reads' budget says.
  const off = await patch(w, O, cid, bid, { hide_key: false });
  assert.deepEqual(off.body.restored, [1], JSON.stringify(off.body));
  // A bank that no longer hides its keys checks nothing on a read.
  assert.equal((await get(w, O, head)).status, 200);
  assert.equal(used(w, O, "circle_hide_checks"), 2);
});

test("51-3 (critic 61 M1): two hide-on PATCHes at once on one head: only the one that withheld it answers withheld:true", async () => {
  const { w, adm, O, A, P, cid } = await world();
  const bid = (await sendRaw(w, A, cid, bankOf([LAB()]))).body.bank_id;
  await takedownOf(w, adm, P, [UNL()]);
  const [strip, keep] = await Promise.all([
    patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "strip" }] }),
    patch(w, O, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "keep" }], order_acks: [{ question: 0 }] }),
  ]);
  assert.equal(strip.status, 200, JSON.stringify(strip.body));
  assert.equal(keep.status, 200, JSON.stringify(keep.body));
  assert.deepEqual([strip.body.withheld, strip.body.held], [true, true], JSON.stringify(strip.body));
  // (round 53: the batch that ran second lost its compare-and-set on the setting; the head it read is withheld now, so
  // it is the same setting again: `unchanged`, with no withheld/held fields)
  assert.deepEqual([keep.body.withheld, keep.body.held, keep.body.unchanged], [undefined, undefined, true], JSON.stringify(keep.body));
  // The head is withheld, once.
  assert.equal(w.q<any>(`SELECT v.withheld_at IS NOT NULL AS w FROM circle_versions v JOIN circle_banks b ON b.head_version_id = v.id WHERE b.id = ?`, bid)[0].w, 1);
});

test("51-4 (critic 61 I1, decided: allowed): only the owner and the bank's author while an editor can tell which kind of match an answer-key change withheld; every other member's answers are the same for both", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [O, A, E, E2, V, R, P] = ["Olga", "Ann", "Eli", "Ezra", "Vic", "Rae", "Pat"].map((n) => w.user("teacher", n)) as User[];
  const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"], [E2!, "editor"], [V!, "viewer"], [R!, "editor"]]);
  // A creates the bank; E sends version 2 holding text later confirmed as student data; A sends version 3 (labelled MC).
  const bid = (await sendRaw(w, A!, cid, bankOf([q(CLEAN)]))).body.bank_id;
  assert.equal((await sendRaw(w, E!, cid, bankOf([q(CLEAN), q(STUDENT)]), { bid, base: 1 })).status, 200);
  assert.equal((await sendRaw(w, A!, cid, bankOf([LAB()]), { bid, base: 2 })).status, 200);
  const sd = (await publish(w, P!, bankOf([q(STUDENT)]))).body as Q;
  assert.equal((await w.purge(adm, sd.item_id, { questions: [0] })).status, 200);
  await takedownOf(w, adm, P!, [UNL()]);
  const on = await patch(w, O!, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "strip" }] });
  assert.equal(on.status, 200, JSON.stringify(on.body));
  const head = `/circles/${cid}/banks/${bid}`;
  const seen = async (u: User, path: string) => {
    const r = await get(w, u, path);
    return { status: r.status, error: r.body.error, field: "hide_off_releases" in r.body ? r.body.hide_off_releases : "absent", keys: Object.keys(r.body).sort().join(",") };
  };
  // The owner and the author (an editor) are told: version 2 (student data) false, version 3 (copyright) true.
  for (const u of [O!, A!]) {
    assert.equal((await seen(u, `${head}/versions/2`)).field, false);
    assert.equal((await seen(u, `${head}/versions/3`)).field, true);
    assert.equal((await seen(u, head)).field, true);
  }
  // Removed from the circle: R is not a member any more.
  assert.equal((await w.json("POST", `/circles/${cid}/members/remove`, { user: O!, json: { user_id: R!.id } })).status, 200);
  // Nobody else gets the field, and each gets the same answer for both versions.
  const others: [string, User][] = [["the editor who sent version 2", E!], ["another editor", E2!], ["a viewer", V!], ["a removed member", R!]];
  for (const [who, u] of others) {
    const [v2, v3, h] = [await seen(u, `${head}/versions/2`), await seen(u, `${head}/versions/3`), await seen(u, head)];
    for (const x of [v2, v3, h]) assert.equal(x.field, "absent", `${who}: ${JSON.stringify(x)}`);
    assert.deepEqual(v2, v3, `${who}: the two kinds look the same`);
  }
  // The author demoted to viewer is not told either.
  assert.equal((await w.json("POST", `/circles/${cid}/members/role`, { user: O!, json: { user_id: A!.id, role: "viewer" } })).status, 200);
  const [d2, d3] = [await seen(A!, `${head}/versions/2`), await seen(A!, `${head}/versions/3`)];
  assert.equal(d2.field, "absent");
  assert.equal(d3.field, "absent");
  assert.deepEqual(d2, d3);
});
