// Round 10: critics 13a (ASVS) / 13b (Moodle) inverted. The licence cases live in
// licence-cases.test.ts; here: L1 privacy appeals never restore by default, L2 the rightful
// exemption, M-6 the reader-visible licence check, M-7 credits past 64, M-8 admin bodies, M-9 an
// unchanged hidden republish, S-1 and the non-blocking items.
import assert from "node:assert/strict";
import { test } from "node:test";
import { addressCaps, preReadBody } from "../../src/pool/preread.ts";
import { unauthReserved } from "../../src/pool/shared.ts";
import { API, publish, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const served = (r: any): Q[] => r.body.payload.bank.questions;
const withOrigin = (r: any) =>
  served(r).map((x: Q, i: number) => ({ ...x, origin: { item_id: r.body.item.id, qid: r.body.questions[i].qid, publish_id: r.body.item.publish_id, version: r.body.item.version, author_name: r.body.item.author_name, license: r.body.license, form: r.body.item.form } }));
const holds = (w: World, id: string) => w.q<{ reason: string; source: string }>(`SELECT reason, source FROM item_holds WHERE item_id = ? ORDER BY reason, source`, id);
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
/** The daily cron at its 04:00 slot, as production runs it. */
const cron = async (w: World) => {
  const t = w.clock;
  w.clock = Math.floor(t / DAY) * DAY + 4 * 3600_000 + (t % DAY > 4 * 3600_000 ? DAY : 0);
  await w.pool.daily();
};

// ---- L1 -------------------------------------------------------------------------------------------------

test("L1 (appeal.ts): confirmed student data never comes back: the admin decides purge, a re-appeal after UTC midnight is refused, 60 silent days restore nothing", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user("teacher", "Alice"), C = w.user("teacher", "Carl"), R = w.user("teacher", "Reader");
  const a = (await publish(w, A, bankOf([q("Jenny Smith (ID 40211) in period 3 scored 42 on the test; explain her error."), q("Alice's volcano question?")]))).body;
  w.advance(DAY);
  const copy = (await publish(w, C, bankOf([served(await imp(w, C, a.item_id as string))[0]!]))).body;
  assert.equal((await w.purge(adm, a.item_id as string, { questions: [0] })).status, 200);
  const appeal = async () => (await w.json("POST", `/items/${a.item_id}/appeal`, { user: A, json: { note: "It is a fictional name." } })).status;
  assert.equal(await appeal(), 202);
  w.advance(19 * DAY);
  await cron(w);
  assert.equal((await w.admin(adm, "POST", "/admin/pool/appeal-decision", { item_id: a.item_id, decision: "purge" })).status, 200);
  w.clock = Math.ceil(w.clock / DAY) * DAY + 1000; // just after the next UTC midnight
  assert.equal(await appeal(), 409, "decided: no second appeal of the same removal");
  for (let i = 0; i < 62; i++) {
    w.advance(DAY);
    await cron(w);
  }
  assert.equal((await get(w, R, a.item_id as string)).status, 404);
  assert.equal((await get(w, R, copy.item_id as string)).status, 404);
  assert.equal(w.q<{ p: number }>(`SELECT purged_at IS NOT NULL AS p FROM items WHERE id = ?`, a.item_id)[0]!.p, 1);
});

test("L1: an unanswered privacy appeal escalates weekly and at 60 days, the item stays quarantined, its image bytes held; a policy removal is restored by default (flagged)", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user("teacher", "Alice"), B = w.user("teacher", "Bob");
  const a = (await publish(w, A, bankOf([q("Jenny Smith (ID 40211) in period 3 scored 42 on the test today."), q("Alice's other question about volcanoes?")]))).body;
  await w.purge(adm, a.item_id as string, { questions: [0] });
  assert.equal((await w.json("POST", `/items/${a.item_id}/appeal`, { user: A, json: { note: "fictional" } })).status, 202);
  const b = (await publish(w, B, bankOf([q("Bob's question about the moon phases and tides?")]))).body;
  await w.admin(adm, "POST", "/admin/pool/remove", { item_id: b.item_id, note: "policy" });
  assert.equal((await w.json("POST", `/items/${b.item_id}/appeal`, { user: B, json: { note: "it is fine" } })).status, 202);
  for (let i = 0; i < 62; i++) {
    w.advance(DAY);
    await cron(w);
  }
  const st = (id: unknown) => w.q<{ removed_reason: string | null; purged: number }>(`SELECT removed_reason, purged_at IS NOT NULL AS purged FROM items WHERE id = ?`, id)[0];
  assert.deepEqual(st(a.item_id), { removed_reason: "privacy", purged: 0 });
  // Round 11: ONE overdue flag per open appeal, its week count kept up to date (not one row a week).
  const od = w.q<{ detail: string }>(`SELECT detail FROM admin_flags WHERE kind = 'appeal_overdue' AND item_id = ?`, a.item_id);
  assert.equal(od.length, 1, "one reminder row");
  assert.ok(JSON.parse(od[0]!.detail).weeks >= 8, "its week count is current");
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'appeal_escalated' AND item_id = ?`, a.item_id).length, 1);
  assert.deepEqual(st(b.item_id), { removed_reason: null, purged: 0 }, "a policy removal is restored after 60 days");
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'appeal_auto_restored' AND item_id = ?`, b.item_id).length, 1);
  // privacy-restore stays an explicit admin action.
  assert.equal((await w.admin(adm, "POST", "/admin/pool/privacy-restore", { item_id: a.item_id })).status, 200);
});

// ---- L2 -------------------------------------------------------------------------------------------------

test("L2 (rightful2/rightful): naming the rightful item never lets an arbitrary publisher re-upload the content; the rightful item must be another author's, published first", async () => {
  for (const vis of ["public", "unlisted"] as const) {
    const w = new World();
    if (vis === "unlisted") {
      // Round 11 P4: a rightful item must be visible to others (it stands in for the credits).
      const adm = w.user("admin"), E = w.user("teacher", "Eve"), O = w.user("teacher", "Owner");
      const mat = [q("Owner's original question one about tectonic plate boundaries?")];
      const r = (await publish(w, O, bankOf(mat), { visibility: vis })).body;
      w.advance(DAY);
      const x = (await publish(w, E, bankOf(mat), { license: "CC0-1.0" })).body;
      const td = await w.takedown<any>(adm, await notice(w, adm), x.item_id, { rightful_item_id: r.item_id });
      assert.equal(td.status, 409);
      assert.deepEqual(td.body.reasons, ["rightful_not_visible"]);
      continue;
    }
    const adm = w.user("admin"), E = w.user("teacher", "Eve"), O = w.user("teacher", "Owner"), Z = w.user("teacher", "Zed");
    const mat = [q("Owner's original question one about tectonic plate boundaries?"), q("Owner's original question two about subduction zones?")];
    const r = (await publish(w, O, bankOf(mat), { visibility: vis })).body;
    w.advance(DAY);
    const x = (await publish(w, E, bankOf(mat), { license: "CC0-1.0" })).body;
    assert.equal((await w.takedown(adm, await notice(w, adm), x.item_id, { rightful_item_id: r.item_id })).status, 200);
    w.advance(DAY);
    const reE = (await publish(w, E, bankOf(mat), { license: "CC0-1.0" })).body;
    assert.equal(holds(w, reE.item_id as string)[0]?.reason, "blocklist", `${vis}: the infringer's re-upload is held`);
    const ro = (await publish(w, O, bankOf(mat))).body;
    assert.deepEqual(holds(w, ro.item_id as string), [], `${vis}: the rightful author is never held`);
    if (vis === "public") {
      const rz = (await publish(w, Z, bankOf(withOrigin(await imp(w, Z, r.item_id as string))))).body;
      assert.deepEqual(holds(w, rz.item_id as string), [], "someone who imported the rightful item is not held");
    }
  }
  const w = new World();
  const adm = w.user("admin"), E = w.user(), O = w.user();
  const stolen = [q("Copyrighted textbook question one about tectonic plate boundaries?")];
  const x2 = (await publish(w, E, bankOf(stolen))).body; // the infringer's own EARLIER sibling copy
  const x = (await publish(w, E, bankOf(stolen))).body;
  w.advance(DAY);
  const later = (await publish(w, O, bankOf([q("Owner's later unrelated question about rivers?")]))).body;
  for (const rid of [x2.item_id, later.item_id]) assert.equal((await w.takedown(adm, await notice(w, adm), x.item_id, { rightful_item_id: rid })).status, 409);
});

// ---- M-6 ------------------------------------------------------------------------------------------------

test("M-6 (s9): every read carries license_check; a copy whose licence its credits do not allow says mismatch, and is not released at an upheld takedown", async () => {
  const w = new World();
  const adm = w.user("admin"), R = w.user("teacher", "Rita"), F = w.user("teacher", "Fred"), D = w.user("teacher", "Dana"), V = w.user("teacher", "Viewer");
  const Qr = q("Rita's distinctive question on the Krebs cycle producing three NADH per turn, explain.", { explanation: "Rita's explanation." });
  // Round 16 (B2): an earlier VISIBLE publication now credits (and binds) a later near copy, so
  // Rita's item is unlisted when Fred publishes and goes public only after.
  const r = (await publish(w, R, bankOf([Qr]), { license: "CC-BY-NC-4.0", visibility: "unlisted" })).body;
  w.advance(DAY);
  const f = (await publish(w, F, bankOf([{ ...Qr, explanation: "Fred's tweak of it." }]), { license: "CC0-1.0" })).body; // a soft adaptation of nothing read
  assert.equal((await w.json("PATCH", `/items/${r.item_id}`, { user: R, json: { visibility: "public" } })).status, 200);
  w.advance(DAY);
  await get(w, D, r.item_id as string); // Dana was served Rita's item too (so the rightful exemption reaches her copy)
  const d = (await publish(w, D, bankOf([withOrigin(await imp(w, D, f.item_id as string))[0]]), { license: "CC0-1.0" })).body;
  assert.deepEqual((await get(w, V, d.item_id as string)).body.license_check, { status: "ok", basis: "recorded_sources" });
  const td = await w.takedown<any>(adm, await notice(w, adm), f.item_id, { questions: [0], rightful_item_id: r.item_id });
  await w.holdCopies(adm, td.body.action_id);
  assert.equal(holds(w, d.item_id as string).length, 1);
  w.advance(31 * DAY);
  await w.pool.daily();
  // The upheld takedown makes Rita's (NC) item the credit of Dana's CC0 copy: mismatch, kept held, flagged.
  assert.equal(holds(w, d.item_id as string).length, 1, "not released with a CC0 badge its credits contradict");
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'license_mismatch' AND item_id = ?`, d.item_id).length, 1);
  // Round 11 P5: the marker agrees with the licence rule at publish ("consistent with recorded
  // sources"): a contradiction the rule resolved (the earliest source, round 10 M-1) reads ok.
  const w2 = new World();
  const S = w2.user("teacher", "Sam"), N = w2.user("teacher", "Nia"), T = w2.user("teacher", "Tess"), V2 = w2.user();
  const cap = (e: string) => q("What is the capital of France, and why did it become the capital?", { explanation: e });
  for (const [u, e, lic] of [[S, "Sam's", "CC-BY-SA-4.0"], [N, "Nia's", "CC-BY-NC-4.0"]] as const) {
    const it = (await publish(w2, u, bankOf([cap(e)]), { license: lic })).body;
    w2.advance(DAY);
    await imp(w2, T, it.item_id as string);
  }
  const t = (await publish(w2, T, bankOf([cap("Tess's own")]), { license: "CC-BY-SA-4.0" })).body;
  const lc = (await get(w2, V2, t.item_id as string)).body.license_check;
  assert.deepEqual(lc, { status: "ok", basis: "recorded_sources" }, "what the rule allowed at publish is what readers see");
});

// ---- M-7 ------------------------------------------------------------------------------------------------

test("M-7 (s17): past 64 contributors the originators and the most recent are kept, the middle is one placeholder, the direct source is always there", async () => {
  const w = new World();
  const N = 70;
  const users = Array.from({ length: N }, (_, i) => w.user("teacher", `T${i}`));
  let prev = (await publish(w, users[0]!, bankOf([q("Original question on the Krebs cycle producing three NADH per turn, explain.", { explanation: "v0" })]), { license: "CC-BY-4.0" })).body as any;
  for (let i = 1; i < N; i++) {
    w.advance(1000);
    const p = await publish(w, users[i]!, bankOf(withOrigin(await imp(w, users[i]!, prev.item_id)).map((x: Q) => ({ ...x, explanation: `v${i}` }))), { license: "CC-BY-4.0" });
    prev = p.body;
  }
  const stored = JSON.parse(w.q<{ chain_json: string }>(`SELECT chain_json FROM item_questions WHERE publish_id = ?`, prev.publish_id)[0]!.chain_json) as Q[];
  const names = stored.map((l) => l.author_name);
  assert.deepEqual(names.slice(0, 8), ["T0", "T1", "T2", "T3", "T4", "T5", "T6", "T7"]);
  assert.match(names[8]!, /^and \d+ contributors between$/);
  // Round 11 (s8): the placeholder counts every contributor it stands for, inherited placeholders included.
  assert.equal(names[8], `and ${69 - (stored.length - 1)} contributors between`);
  assert.equal(names[names.length - 1], "T68", "the direct source");
  assert.deepEqual(names.slice(-4, -1), ["T65", "T66", "T67"], "the most recent are kept, not the oldest");
  assert.ok(stored.length <= 66);
  const shown = (await get(w, w.user(), prev.item_id)).body.questions[0].chain.map((l: Q) => l.author_name);
  // Round 11: the reader sees the first 8, one placeholder with the right count, the most recent 24.
  assert.deepEqual(shown.slice(0, 8), ["T0", "T1", "T2", "T3", "T4", "T5", "T6", "T7"]);
  assert.equal(shown[8], `and ${69 - 8 - 24} contributors between`);
  assert.equal(shown.length, 33);
  assert.equal(shown[shown.length - 1], "T68");
});

// ---- M-8 ------------------------------------------------------------------------------------------------

test("M-8 (s14): an admin's bodies up to 256 KB pass the pre-read (1000 hashes, 2,000 CJK characters x 3); others stay at 8 KB", async () => {
  const body = JSON.stringify({ item_id: "a".repeat(32), images: Array.from({ length: 1000 }, (_, i) => i.toString(16).padStart(64, "0")), a: "著".repeat(2000), b: "作".repeat(2000), c: "物".repeat(2000) });
  const len = String(new TextEncoder().encode(body).length);
  const mk = (auth: string, path = "/admin/pool/purge") => new Request(`${API}${path}`, { method: "POST", headers: { "Content-Type": "application/json", "Content-Length": len, "CF-Connecting-IP": "10.0.0.1", Authorization: auth }, body });
  assert.ok((await preReadBody(mk("Bearer a"), async () => ({ userId: "admin-1", trust: "admin" }))) instanceof Request);
  assert.equal(((await preReadBody(mk("Bearer t"), async () => ({ userId: "t-1", trust: "teacher" }))) as Response).status, 413);
  assert.equal(((await preReadBody(mk("Bearer a", "/auth/profile"), async () => ({ userId: "admin-1", trust: "admin" }))) as Response).status, 413, "only admin routes");
  const w = new World();
  assert.ok(w.pool.adminRoutes.every((r) => (r as { maxBodyBytes?: number }).maxBodyBytes === 256 * 1024));
  const adm = w.user("admin");
  const cjk = await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "山田", complainant_contact: "x@y.jp", work_desc: "著".repeat(2000), material_desc: "作".repeat(2000), has_signature: true, has_good_faith: true, has_perjury_stmt: true });
  assert.equal(cjk.status, 201);
});

// ---- M-9 ------------------------------------------------------------------------------------------------

test("M-9 (s6): a hidden-key republish that changes nothing served (MC, Matching, Multipart; key or explanation only) keeps the stripped body and public version", async () => {
  const mc = (c: number, e = "E") => q("Which of these planets in Alice's own list is largest by radius overall?", { kind: { Mc: { answers: ["Mercury", "Jupiter", "Mars", "Venus"].map((t, i) => ({ text: t, correct: i === c, locked: false })) } }, explanation: e });
  const mt = () => q("Match each country to its capital city in Alice's list.", { kind: { Matching: { pairs: [["France", "Paris"], ["Peru", "Lima"], ["Chad", "N'Djamena"]].map(([left, right]) => ({ left, right })), distractors: [] } } });
  const mp = (c: number) => q("Answer both parts about Alice's planets.", { kind: { Multipart: { parts: [{ stem: "Largest?", kind: { Mc: { answers: ["Mars", "Jupiter", "Venus"].map((t, i) => ({ text: t, correct: i === c, locked: false })) } } }] } } });
  for (const [nm, v1, v2] of [["mc nothing", [mc(1)], [mc(1)]], ["mc key", [mc(1)], [mc(2)]], ["mc expl", [mc(1, "E1")], [mc(1, "E2")]], ["matching", [mt()], [mt()]], ["multipart key", [mp(1)], [mp(2)]]] as const) {
    const w = new World();
    const A = w.user(), M = w.user();
    const a = (await publish(w, A, bankOf(v1 as unknown as Q[]), { hideKey: true })).body as any;
    assert.ok(a.item_id, `${nm}: ${JSON.stringify(a).slice(0, 200)}`);
    const g1 = await get(w, M, a.item_id);
    const r2 = await publish(w, A, bankOf((v2 as unknown as Q[]).map((x, k) => ({ ...x, published: { item_id: a.item_id, qid: a.qids[k], publish_id: a.publish_id, version: 1 } }))), { hideKey: true, base: a.publish_id }, a.item_id);
    const g2 = await get(w, M, a.item_id);
    assert.deepEqual([r2.status, r2.body.public_version, JSON.stringify(served(g1)) === JSON.stringify(served(g2)), g1.body.item.publish_id === g2.body.item.publish_id], [200, 1, true, true], nm);
  }
});

// ---- S-1 and non-blocking ---------------------------------------------------------------------------------

test("S-1 (starve.ts): one IPv4 /24 holds at most 8 unauthenticated sign-in bodies; a legitimate login elsewhere passes", async () => {
  const stalled = (ip: string) => new Request(`${API}/auth/login`, { method: "POST", headers: { "Content-Length": "2048", "CF-Connecting-IP": ip, "Content-Type": "application/json" }, body: new ReadableStream<Uint8Array>({ start: (c) => c.enqueue(new Uint8Array(2047)) }), duplex: "half" } as RequestInit);
  const pend: Promise<unknown>[] = [];
  for (let i = 0; i < 64; i++) for (let k = 0; k < 4; k++) pend.push(preReadBody(stalled(`198.51.0.${i}`), async () => null));
  await new Promise((r) => setTimeout(r, 50));
  assert.ok(unauthReserved("signin") <= 8 * 2047, `held ${unauthReserved("signin")}`);
  const lb = JSON.stringify({ email: "a@b.cd", password: "xxxxxxxxxxxxxx" });
  assert.ok((await preReadBody(new Request(`${API}/auth/login`, { method: "POST", headers: { "Content-Length": String(lb.length), "CF-Connecting-IP": "203.0.113.9", "Content-Type": "application/json" }, body: lb }), async () => null)) instanceof Request);
  assert.deepEqual(addressCaps("198.51.0.7").map((x) => x.cap), [4, 8]);
  await Promise.all(pend);
});

test("S-1 (del.ts): a deleted account's serves stay counted without naming it; its report audits lose the link", async () => {
  const w = new World();
  const A = w.user(), D = w.user("teacher", "Dana Doe");
  const a = (await publish(w, A, bankOf([q("A question about rivers and deltas?")]))).body;
  await imp(w, D, a.item_id as string);
  await w.json("POST", `/items/${a.item_id}/report`, { user: D, json: { reason: "other", note: "Dana's note" } });
  const { deleteContentStatements } = await import("../../src/pool/index.ts");
  w.sql.prepare(`UPDATE users SET email = NULL, pw_hash = NULL, display_name = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, D.id);
  await w.db.batch(deleteContentStatements(w.db, D.id, w.clock));
  assert.equal(w.q(`SELECT * FROM imports WHERE user_id = ?`, D.id).length, 0);
  assert.equal(w.q(`SELECT * FROM imports WHERE item_id = ?`, a.item_id).length, 1, "still counted");
  assert.equal(w.q(`SELECT * FROM pool_audit WHERE action = 'item.report' AND (actor_id = ? OR detail LIKE '%report_id%')`, D.id).length, 0);
});

test("held copies: their authors are told what the admin's hold is about and which questions; appeal-decision works on a held copy", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user(), C = w.user();
  const a = (await publish(w, A, bankOf([q("Student Jo Smith (ID 4411) failed the retake on Friday."), q("A's clean question about plate tectonics?")]))).body;
  const c = (await publish(w, C, bankOf([...withOrigin(await imp(w, C, a.item_id as string)).slice(0, 1), q("C's own clean question about rocks?")]))).body;
  await w.purge(adm, a.item_id as string, { questions: [0] });
  const mine = (await w.json<any>("GET", "/items/mine", { user: C })).body.items.find((x: Q) => x.id === c.item_id);
  assert.deepEqual([mine.state, mine.hold_class, mine.positions], ["held_for_review", "student_data", [0]]);
  assert.equal((await w.json("POST", `/items/${c.item_id}/appeal`, { user: C, json: { note: "mine is different" } })).status, 202);
  assert.equal((await w.admin(adm, "POST", "/admin/pool/appeal-decision", { item_id: c.item_id, decision: "restore" })).status, 200);
  assert.deepEqual(holds(w, c.item_id as string), []);
  assert.equal((await w.json("POST", `/items/${c.item_id}/appeal`, { user: C, json: { note: "again" } })).status, 409, "decided");
});

test("admin DMCA listings (notices, actions, counter-notices; paged); a counter-notice flags the admin; the item view shows actions and hold sources", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user();
  const a = (await publish(w, A, bankOf([q("Copied textbook question about cells and organelles?")]))).body;
  const nid = await notice(w, adm);
  const td = await w.takedown<any>(adm, nid, a.item_id);
  assert.equal((await w.json("POST", `/items/${a.item_id}/counter-notice`, { user: A, json: { signature: "A", material_desc: "m", name: "A", address: "addr", phone: "1", perjury_stmt: true, consent_jurisdiction: true, accept_service: true } })).status, 202);
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'counter_notice'`).length, 1);
  for (const p of ["/admin/dmca/notices", "/admin/dmca/actions", "/admin/dmca/counter-notices"]) {
    const r = await w.admin<any>(adm, "GET", p, null, "?limit=1");
    assert.equal(r.body.items.length, 1, p);
  }
  for (let i = 0; i < 2; i++) await notice(w, adm);
  const p1 = await w.admin<any>(adm, "GET", "/admin/dmca/notices", null, "?limit=2");
  const p2 = await w.admin<any>(adm, "GET", "/admin/dmca/notices", null, `?limit=2&cursor=${p1.body.next_cursor}`);
  assert.equal(p1.body.items.length + p2.body.items.length, 3);
  const item = await w.admin<any>(adm, "GET", "/admin/pool/item", null, `?id=${a.item_id}`);
  assert.equal(item.body.dmca_actions[0].id, td.body.action_id);
});

test("search: subject in q, tag filter folds case and accents, decomposed input; an empty bare POST is no body", async () => {
  const w = new World();
  const A = w.user(), T = w.user();
  const a = (await publish(w, A, bankOf([q("A question for the search test with enough words?")]), { meta: { title: "Plain", subject: "Géographie", course: "", grade: "", tags: ["Élève"], standards: [] } })).body;
  const ids = async (qs: string) => (await w.json<any>("GET", `/items?${qs}`, { user: T })).body.items.map((x: Q) => x.id);
  assert.deepEqual(await ids("q=geographie"), [a.item_id]);
  assert.deepEqual(await ids("tag=eleve"), [a.item_id]);
  assert.deepEqual(await ids(`tag=${encodeURIComponent("Élève")}`), [a.item_id]);
  const r = await w.pool.handle(new Request(`${API}/items/${a.item_id}/import`, { method: "POST", headers: { Authorization: `Bearer ${T.token}`, "CF-Connecting-IP": "10.0.0.2" } }));
  assert.equal(r!.status, 200);
});

// ---- share-core round 5 adaptation ------------------------------------------------------------------------

test("share-core round 5: served orders reach the publishing author only: never a listing, another user's response, a log, an audit row or a flag", async () => {
  const logs: string[] = [];
  const orig = { log: console.log, warn: console.warn, error: console.error };
  for (const k of ["log", "warn", "error"] as const) console[k] = (...a: unknown[]) => void logs.push(a.map(String).join(" "));
  try {
    const w = new World();
    const adm = w.user("admin"), A = w.user("teacher", "Alice"), B = w.user("teacher", "Bob");
    const mc = q("Which planet in this list is the largest by radius overall?", { kind: { Mc: { answers: ["Mercury", "Jupiter", "Mars", "Venus", "Earth"].map((t, i) => ({ text: t, correct: i === 1, locked: false })) } } });
    const a = await publish(w, A, bankOf([mc, q("Alice's second question about comets and their tails?")]), { hideKey: true });
    assert.equal(a.status, 201);
    assert.ok(JSON.stringify(a.body).includes("served_orders"), "the author's own publish response has them");
    const id = a.body.item_id as string;
    const seen: string[] = [];
    const take = async (r: Response | { text: string }) => seen.push(r instanceof Response ? await r.text() : r.text);
    await take(await w.call("GET", `/items/${id}`, { user: B }));
    await take(await w.call("HEAD", `/items/${id}`, { user: B }));
    await take(await w.call("POST", `/items/${id}/import`, { user: B }));
    await take(await w.call("GET", "/items?limit=50", { user: B }));
    await take(await w.call("GET", "/items?q=planet", { user: B }));
    await take(await w.call("POST", "/items/versions", { user: B, json: { ids: [id] } }));
    await take(await w.call("GET", "/items/mine", { user: B }));
    await take(await w.call("POST", `/items/${id}/report`, { user: B, json: { reason: "other", note: "n" } }));
    for (const [m, p, qs] of [["GET", "/admin/pool/flags", ""], ["GET", "/admin/pool/reports", ""], ["GET", "/admin/pool/item", `?id=${id}`], ["GET", "/admin/dmca/actions", ""]] as const)
      await take(await w.admin(adm, m, p, null, qs));
    seen.push(JSON.stringify(w.q(`SELECT * FROM pool_audit`)), JSON.stringify(w.q(`SELECT * FROM admin_flags`)), JSON.stringify(w.q(`SELECT * FROM reports`)), ...logs);
    // The author's own views of the item (GET as author) are fine, but nothing above is the author's.
    const leaks = seen.filter((t) => t.includes("served_orders"));
    assert.deepEqual(leaks, []);
  } finally {
    Object.assign(console, orig);
  }
});

test("share-core round 5: new refusal codes carry author-facing messages", async () => {
  const { CORE_MESSAGES } = await import("../../src/pool/core.ts");
  for (const k of ["labels_keep_refused", "placeholder_key", "symbol_font_char", "object_replacement", "keyless_key_material", "symbol_font_mapped"]) assert.ok(CORE_MESSAGES[k], k);
  const w = new World();
  const A = w.user();
  const r = await publish(w, A, bankOf([q("A pasted object here ￼ in the stem of the question?")]));
  assert.equal(r.status, 422);
  const e = (r.body.errors as Q[]).find((x) => x.code === "object_replacement");
  assert.ok(e && typeof e.message === "string", JSON.stringify(r.body).slice(0, 300));
});
