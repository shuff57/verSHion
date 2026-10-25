// Round 49: circles phase C after critic 59 (docs/critic-reports/2026-10-25-circles-phaseC-client-critic59.md).
// B1: a withheld version's `404 version_withheld` tells the members who may change the answer-key setting (the
// circle's owner, the bank's author while an editor) whether turning hiding off would release it: `hide_off_releases`
// is true only when the bank hides its keys and every open flag on that version was raised by an answer-key PATCH
// (`via: hide_key`, hide-off's own rule); a send's withhold says false. Nobody else is told. Turning hiding back on
// stays possible on a withheld head, and a viewer never reads the keys the author asked to hide.
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
/** The withheld answer for `u` at `path`: [status, error, hide_off_releases]. */
const withheld = async (w: World, u: User, path: string) => {
  const r = await get(w, u, path);
  return [r.status, r.body.error, r.body.hide_off_releases];
};

test("49-1 (B1): a head an answer-key PATCH withheld says hide_off_releases: true to the owner and the author (the head and its version read); a send's flag on it too, or hiding off, says false; nobody else is told", async () => {
  const { w, adm, O, A, E, V, P, cid } = await world();
  const bid = (await sendRaw(w, A, cid, bankOf([LAB()]))).body.bank_id;
  await takedownOf(w, adm, P, [UNL()]);
  const on = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "strip" }] });
  assert.deepEqual([on.status, on.body.withheld], [200, true], JSON.stringify(on.body));
  const head = `/circles/${cid}/banks/${bid}`;
  for (const u of [O, A]) {
    assert.deepEqual(await withheld(w, u, head), [404, "version_withheld", true], u.id);
    assert.deepEqual(await withheld(w, u, `${head}/versions/1`), [404, "version_withheld", true], u.id);
  }
  // another editor and a viewer do not read a held bank at all; neither answer names the field
  for (const u of [E, V]) {
    const r = await get(w, u, head);
    assert.deepEqual([r.status, r.body.error, "hide_off_releases" in r.body], [404, "not_found", false], JSON.stringify(r.body));
  }
  // An open send flag on that version too (no `via`): turning hiding off would not release it.
  const vid = w.q<any>(`SELECT id FROM circle_versions WHERE bank_id = ? AND version = 1`, bid)[0].id;
  w.sql.prepare(`INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at) VALUES ('f-send-49', 'blocklist', ?, ?, ?, '{}', ?)`).run(bid, vid, A.id, w.clock);
  assert.deepEqual(await withheld(w, O, head), [404, "version_withheld", false]);
  w.sql.prepare(`DELETE FROM admin_flags WHERE id = 'f-send-49'`).run();
  assert.deepEqual(await withheld(w, O, head), [404, "version_withheld", true]);
  // The author demoted to a viewer still reads the withheld answer (the author of a held bank) but is not told.
  assert.equal((await w.json("POST", `/circles/${cid}/members/role`, { user: O, json: { user_id: A.id, role: "viewer" } })).status, 200);
  const demoted = await get(w, A, head);
  assert.deepEqual([demoted.status, demoted.body.error, "hide_off_releases" in demoted.body], [404, "version_withheld", false], JSON.stringify(demoted.body));
  // What it promised: turning hiding off releases the head.
  const off = await patch(w, O, cid, bid, { hide_key: false });
  assert.deepEqual([off.status, off.body.hide_key, off.body.restored], [200, false, [1]], JSON.stringify(off.body));
  assert.equal((await w.call("GET", head, { user: O })).status, 200);
});

test("49-2 (B1, critic 59's section S): a send to a hiding bank withheld by a copyright match says hide_off_releases: false; turning hiding off anyway releases nothing, hiding can be turned on again on the withheld head, and once released a viewer reads the head without its keys", async () => {
  const { w, adm, O, A, V, P, cid } = await world();
  await takedownOf(w, adm, P, [UNL()]);
  const s1 = await sendRaw(w, A, cid, bankOf([q(CLEAN)]), { extra: { hide_key: true } });
  assert.deepEqual([s1.status, s1.body.withheld], [201, false], JSON.stringify(s1.body));
  const bid = s1.body.bank_id;
  const s2 = await sendRaw(w, A, cid, bankOf([q(CLEAN), LAB()]), { bid, base: 1, confirm: true, extra: { labels: [{ question: 1, choice: "strip" }] } });
  assert.deepEqual([s2.status, s2.body.held, s2.body.withheld], [200, true, true], JSON.stringify(s2.body));
  const f = w.q<any>(`SELECT detail ->> '$.via' AS via FROM admin_flags WHERE item_id = ? AND kind = 'blocklist' AND resolved_at IS NULL`, bid);
  assert.deepEqual(f, [{ via: null }], "the send's own flag");
  const head = `/circles/${cid}/banks/${bid}`;
  for (const u of [O, A]) {
    assert.deepEqual(await withheld(w, u, head), [404, "version_withheld", false], u.id);
    assert.deepEqual(await withheld(w, u, `${head}/versions/2`), [404, "version_withheld", false], u.id);
  }
  // Turned off anyway (the switch): nothing is released and the head stays withheld (hiding off: still false).
  const off = await patch(w, A, cid, bid, { hide_key: false });
  assert.deepEqual([off.status, off.body.hide_key, off.body.restored], [200, false, undefined], JSON.stringify(off.body));
  assert.deepEqual(await withheld(w, O, head), [404, "version_withheld", false]);
  // Hiding can be turned on again while the head is withheld (no strip of a head nobody reads).
  const again = await patch(w, A, cid, bid, { hide_key: true });
  assert.deepEqual([again.status, again.body.hide_key, again.body.unchanged], [200, true, undefined], JSON.stringify(again.body));
  assert.equal(w.q<any>(`SELECT hide_key FROM circle_banks WHERE id = ?`, bid)[0].hide_key, 1);
  // An administrator's release (simulated): the viewer reads the head's viewers' form, never the keys.
  w.sql.prepare(`UPDATE circle_versions SET withheld_at = NULL WHERE bank_id = ?`).run(bid);
  w.sql.prepare(`UPDATE admin_flags SET resolved_at = ? WHERE item_id = ?`).run(w.clock, bid);
  w.sql.prepare(`DELETE FROM circle_holds WHERE bank_id = ?`).run(bid);
  const r = await w.call("GET", head, { user: V });
  const t = await r.text();
  assert.equal(r.status, 200, t);
  const doc = JSON.parse(t);
  assert.equal(doc.bank.keys_hidden, true);
  const answers = doc.payload.bank.questions.flatMap((x: Q) => x.kind?.Mc?.answers ?? []);
  assert.ok(answers.length > 0 && answers.every((a: Q) => a.correct !== true), t);
  assert.ok(!/SAMPLEKEYzzq/.test(t), "no short-answer key either");
});

test("49-3 (B1): a bank that does not hide its keys says false for a withheld head, and an answer-key PATCH's flag on an older version is answered on that version's read (round 50: false here, its full text is the taken-down text, so hiding off would not release it)", async () => {
  const { w, adm, O, A, P, cid } = await world();
  const b1 = (await sendRaw(w, A, cid, bankOf([q(CLEAN)]))).body.bank_id;
  const b2 = (await sendRaw(w, A, cid, bankOf([UNL()]))).body.bank_id;
  assert.equal((await sendRaw(w, A, cid, bankOf([q("Another clean question about moraines and kettle lakes?")]), { bid: b2, base: 1, confirm: true })).status, 200);
  await takedownOf(w, adm, P, [UNL()]);
  // A send that matches a block, hiding never on: false.
  const s2 = await sendRaw(w, A, cid, bankOf([q(CLEAN), UNL()]), { bid: b1, base: 1, confirm: true });
  assert.deepEqual([s2.status, s2.body.withheld], [200, true], JSON.stringify(s2.body));
  assert.deepEqual(await withheld(w, O, `/circles/${cid}/banks/${b1}`), [404, "version_withheld", false]);
  // An older version (no decisions recorded) the PATCH withholds: its own read answers the field (round 50, critic 60
  // B2: false, the full text matches the block too and hiding off releases nothing; see 50-3); the head reads.
  const r = await patch(w, A, cid, b2, { hide_key: true });
  assert.deepEqual([r.status, r.body.withheld], [200, false], JSON.stringify(r.body));
  assert.deepEqual(await withheld(w, A, `/circles/${cid}/banks/${b2}/versions/1`), [404, "version_withheld", false]);
  assert.equal((await w.call("GET", `/circles/${cid}/banks/${b2}`, { user: A })).status, 200);
});
