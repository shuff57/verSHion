// Round 50: circles phase C after critic 60 (docs/critic-reports/2026-10-25-circles-phaseC-final-critic60.md).
// B2: `hide_off_releases` (and what turning hiding off releases) counts only a version turning hiding off can
// actually release: one whose open flags are all copyright matches an answer-key PATCH raised (`blocklist`,
// `via: hide_key`) and whose full form is clean now (hide-off's own check). A student-data match never counts, even
// one an answer-key PATCH raised: the loop "the field says true, hiding goes off, nothing is released, hiding goes on
// again, the field says true again" is gone.
// B1: turning hiding on answers `withheld` (and `held`) only when THIS PATCH withheld the head, never for a head
// that was withheld before it.
import assert from "node:assert/strict";
import { test } from "node:test";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { framed, publish, publishId, World, type User } from "./harness.ts";

type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "SAMPLEKEYzzq" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "local", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const STEM = "Copyrighted textbook problem 4.7: which organelle in the eukaryotic cell produces most of the ATP in respiration?";
const LAB = (extra: Q = {}) => mcq(STEM, [["A. Mitochondrion", true], ["B. Ribosome", false], ["C. Golgi body", false], ["D. Lysosome", false]], extra);
const UNL = () => mcq(STEM, [["Mitochondrion", true], ["Ribosome", false], ["Golgi body", false], ["Lysosome", false]]);
const CLEAN = "Clean question about eskers and drumlins and the glaciers that left them?";
const STUDENT = "Jenny Smith in period 3 scored 42 on the unit test: which of these is the class median?";
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][], name = "Algebra team"): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name } })).body.circle.id;
  for (const [u, role] of members) {
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } })).body.code;
    assert.equal((await w.json("POST", "/circles/join", { user: u, json: { code: k } })).status, 200);
    w.advance(1000);
  }
  return cid;
}
async function sendRaw(w: World, u: User, cid: string, payload: string, o: { bid?: string; base?: number; extra?: Q; confirm?: boolean } = {}) {
  const meta: Q = {
    commit_id: publishId(),
    license: "circle-only",
    attest: { own_work: true, terms_version: POOL_TERMS_VERSION },
    meta: { title: "Unit 2", subject: "Math", course: "", grade: "", tags: [], standards: [] },
    ...(o.bid ? { base_version: o.base } : {}),
    ...(o.confirm ? { confirm_large_change: true } : {}),
    ...(o.extra ?? {}),
  };
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, payload), contentType: PUB });
}
const patch = (w: World, u: User, cid: string, bid: string, body: Q) => w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: u, json: body });
const get = (w: World, u: User, path: string) => w.json<any>("GET", path, { user: u });
async function world() {
  const w = new World();
  const adm = w.user("admin");
  const [O, A, E, V, P] = ["Olga", "Ann", "Eli", "Vic", "Pat"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"], [V!, "viewer"]]);
  return { w, adm, O: O!, A: A!, E: E!, V: V!, P: P!, cid };
}
async function takedownOf(w: World, adm: User, P: User, qs: Q[]) {
  const item = (await publish(w, P, bankOf(qs))).body as Q;
  const td = await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: qs.map((_, i) => i) });
  assert.equal(td.status, 200, JSON.stringify(td.body));
}
/** Confirmed student data: a public item holding `stems` is purged by an administrator. */
async function studentDataOf(w: World, adm: User, P: User, stems: string[]) {
  const it = (await publish(w, P, bankOf(stems.map((s) => q(s))))).body as Q;
  const pu = await w.purge(adm, it.item_id, { questions: stems.map((_, i) => i) });
  assert.equal(pu.status, 200, JSON.stringify(pu.body));
}
/** The withheld answer for `u` at `path`: [status, error, hide_off_releases]. */
const withheld = async (w: World, u: User, path: string) => {
  const r = await get(w, u, path);
  return [r.status, r.body.error, r.body.hide_off_releases];
};
const openFlags = (w: World, bid: string) =>
  w.q<any>(`SELECT kind, detail ->> '$.via' AS via FROM admin_flags WHERE item_id = ? AND kind IN ('privacy_text_match', 'blocklist') AND resolved_at IS NULL ORDER BY kind`, bid);
const holds = (w: World, bid: string) => w.q<any>(`SELECT source FROM circle_holds WHERE bank_id = ? ORDER BY source`, bid).map((x) => x.source);

test("50-1 (B2, critic 60's c60-1): text confirmed as student data after the send; hiding on withholds the head with a student-data flag via hide_key: the field says false to the owner and the author, hiding off releases nothing and lifts no hold, and turning hiding on again still says false (no loop)", async () => {
  const { w, adm, O, A, P, cid } = await world();
  const s1 = await sendRaw(w, A, cid, bankOf([q(CLEAN), q(STUDENT)]));
  assert.equal(s1.status, 201, JSON.stringify(s1.body));
  const bid = s1.body.bank_id;
  const head = `/circles/${cid}/banks/${bid}`;
  await studentDataOf(w, adm, P, [STUDENT]);
  const on = await patch(w, A, cid, bid, { hide_key: true });
  assert.deepEqual([on.status, on.body.held, on.body.withheld], [200, true, true], JSON.stringify(on.body));
  assert.deepEqual(openFlags(w, bid), [{ kind: "privacy_text_match", via: "hide_key" }]);
  for (const u of [O, A]) {
    assert.deepEqual(await withheld(w, u, head), [404, "version_withheld", false], u.id);
    assert.deepEqual(await withheld(w, u, `${head}/versions/1`), [404, "version_withheld", false], u.id);
  }
  const hold0 = holds(w, bid);
  assert.ok(hold0.includes("privacy"), JSON.stringify(hold0));
  // Turned off anyway: nothing is released, the student-data flag stays open, the hold stays.
  const off = await patch(w, O, cid, bid, { hide_key: false });
  assert.deepEqual([off.status, off.body.hide_key, off.body.restored], [200, false, undefined], JSON.stringify(off.body));
  assert.deepEqual(await withheld(w, O, head), [404, "version_withheld", false]);
  assert.deepEqual(openFlags(w, bid), [{ kind: "privacy_text_match", via: "hide_key" }]);
  assert.deepEqual(holds(w, bid), hold0);
  // On again: the head is not scanned (nobody reads it), the answer says nothing was withheld by it, the field false.
  const again = await patch(w, A, cid, bid, { hide_key: true });
  assert.deepEqual([again.status, again.body.hide_key, again.body.held, again.body.withheld], [200, true, false, false], JSON.stringify(again.body));
  for (const u of [O, A]) assert.deepEqual(await withheld(w, u, head), [404, "version_withheld", false], u.id);
  const off2 = await patch(w, O, cid, bid, { hide_key: false });
  assert.deepEqual([off2.status, off2.body.restored], [200, undefined], JSON.stringify(off2.body));
  assert.deepEqual(openFlags(w, bid), [{ kind: "privacy_text_match", via: "hide_key" }]);
});

test("50-2 (B2): a head whose viewers' form matched both a copyright block and student data (both flags via hide_key) says false, and hiding off releases nothing", async () => {
  const { w, adm, O, A, P, cid } = await world();
  const bid = (await sendRaw(w, A, cid, bankOf([LAB(), q(STUDENT)]))).body.bank_id;
  await takedownOf(w, adm, P, [UNL()]);
  await studentDataOf(w, adm, P, [STUDENT]);
  const on = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "strip" }] });
  assert.deepEqual([on.status, on.body.withheld], [200, true], JSON.stringify(on.body));
  assert.deepEqual(openFlags(w, bid), [{ kind: "blocklist", via: "hide_key" }, { kind: "privacy_text_match", via: "hide_key" }]);
  const head = `/circles/${cid}/banks/${bid}`;
  for (const u of [O, A]) assert.deepEqual(await withheld(w, u, head), [404, "version_withheld", false], u.id);
  const off = await patch(w, O, cid, bid, { hide_key: false });
  assert.deepEqual([off.status, off.body.restored], [200, undefined], JSON.stringify(off.body));
  assert.equal(openFlags(w, bid).length, 2, "no flag resolved");
  assert.deepEqual(await withheld(w, O, head), [404, "version_withheld", false]);
});

test("50-3 (B2): a copyright match via hide_key whose FULL form matches too (critic 60's loop for copyright: an older version, and a head whose labelled text is taken down later) says false; once the full form is clean, true, and hiding off releases it", async () => {
  // An older version (no decisions recorded): the PATCH withholds it; its full text is the taken-down text itself.
  {
    const { w, adm, A, P, cid } = await world();
    const b2 = (await sendRaw(w, A, cid, bankOf([UNL()]))).body.bank_id;
    assert.equal((await sendRaw(w, A, cid, bankOf([q("Another clean question about moraines and kettle lakes?")]), { bid: b2, base: 1, confirm: true })).status, 200);
    await takedownOf(w, adm, P, [UNL()]);
    const r = await patch(w, A, cid, b2, { hide_key: true });
    assert.deepEqual([r.status, r.body.withheld, r.body.held], [200, false, false], JSON.stringify(r.body));
    const v1 = `/circles/${cid}/banks/${b2}/versions/1`;
    assert.deepEqual(await withheld(w, A, v1), [404, "version_withheld", false]);
    const off = await patch(w, A, cid, b2, { hide_key: false });
    assert.deepEqual([off.status, off.body.restored], [200, undefined], JSON.stringify(off.body));
    // On again (v1 is withheld, so it is not checked again): still false.
    assert.equal((await patch(w, A, cid, b2, { hide_key: true })).status, 200);
    assert.deepEqual(await withheld(w, A, v1), [404, "version_withheld", false]);
  }
  // The head (labels stripped for viewers) withheld by the PATCH; the labelled text is taken down afterwards.
  {
    const { w, adm, O, A, P, cid } = await world();
    const R = w.user("teacher", "Rob");
    const labelled = (await publish(w, R, bankOf([LAB()]))).body as Q;
    const bid = (await sendRaw(w, A, cid, bankOf([LAB()]))).body.bank_id;
    await takedownOf(w, adm, P, [UNL()]);
    assert.equal((await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "strip" }] })).body.withheld, true);
    const head = `/circles/${cid}/banks/${bid}`;
    assert.deepEqual(await withheld(w, O, head), [404, "version_withheld", true], "the full form is clean: hiding off would release it");
    const action = (await w.takedown<any>(adm, await notice(w, adm), labelled.item_id, { questions: [0] })).body.action_id;
    assert.ok(action);
    for (const u of [O, A]) assert.deepEqual(await withheld(w, u, head), [404, "version_withheld", false], u.id);
    // The full form's block is lifted: true again, and hiding off does release it.
    w.sql.prepare(`UPDATE blocked_hashes SET lifted_at = ? WHERE action_id = ?`).run(w.clock, action);
    assert.deepEqual(await withheld(w, O, head), [404, "version_withheld", true]);
    const off = await patch(w, O, cid, bid, { hide_key: false });
    assert.deepEqual([off.status, off.body.restored], [200, [1]], JSON.stringify(off.body));
    assert.equal((await w.call("GET", head, { user: O })).status, 200);
  }
});

test("50-4 (B1): turning hiding on over a head a send withheld answers held:false, withheld:false (this PATCH withheld nothing); one that withholds the head itself still says true", async () => {
  const { w, adm, O, A, P, cid } = await world();
  const b1 = (await sendRaw(w, A, cid, bankOf([q(CLEAN)]))).body.bank_id;
  await takedownOf(w, adm, P, [UNL()]);
  const s2 = await sendRaw(w, A, cid, bankOf([q(CLEAN), UNL()]), { bid: b1, base: 1, confirm: true });
  assert.deepEqual([s2.status, s2.body.held, s2.body.withheld], [200, true, true], JSON.stringify(s2.body));
  const on = await patch(w, O, cid, b1, { hide_key: true });
  assert.deepEqual([on.status, on.body.hide_key, on.body.held, on.body.withheld, on.body.unchanged], [200, true, false, false, undefined], JSON.stringify(on.body));
  assert.equal(w.q<any>(`SELECT hide_key FROM circle_banks WHERE id = ?`, b1)[0].hide_key, 1);
  assert.deepEqual(await withheld(w, O, `/circles/${cid}/banks/${b1}`), [404, "version_withheld", false], "a send's withhold");
  // This PATCH withholds the head itself: true, with the hold.
  const b2 = (await sendRaw(w, A, cid, bankOf([LAB()]))).body.bank_id;
  const on2 = await patch(w, A, cid, b2, { hide_key: true, labels: [{ question: 0, choice: "strip" }] });
  assert.deepEqual([on2.status, on2.body.held, on2.body.withheld], [200, true, true], JSON.stringify(on2.body));
});
