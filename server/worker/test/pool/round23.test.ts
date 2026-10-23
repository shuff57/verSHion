// Round 23: critics 30a (Moodle) and 30b (ASVS), inverted. A ghost takedown is planned and blocked by
// the live rules (A2) and upheld by one resumable claim-and-finish (A1, F1); counter-notices are taken
// only while the action is open (A3); the author's notice cannot be lost (F2/F3) and she sees the
// takedown (Z1). POOL_HASH_KEY is permanent: another key fails closed (30a B1). An erased author's
// takedown never launders on any path (30a B2). The earliest author of a question is never bound or
// credited because of a later copier (30a B3). Licences count only while publicly offered (30a B4), in
// bounded rows (W1). Erasure honours keep_on_delete (F4), withdrawals release holds (G1), admin notes
// stay as evidence (T1).
import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteContentStatements, makeCore } from "../../src/pool/index.ts";
import { PUBLISH_CONTENT_TYPE } from "../../src/pool/shared.ts";
import { grantOptions } from "../../src/pool/license.ts";
import { framed, publish, publishId, shareCore, TEST_HASH_KEY, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
const code = (r: { status: number; body: Q }) => (r.status >= 400 ? `${r.status} ${r.body.error}` : String(r.status));
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const days = async (w: World, n: number) => {
  for (let d = 0; d < n; d++) {
    w.advance(DAY);
    await w.pool.hourly();
    await w.pool.daily();
  }
};
const quiet = async <T>(f: () => Promise<T>): Promise<T> => {
  const e = console.error;
  console.error = () => {};
  try {
    return await f();
  } finally {
    console.error = e;
  }
};
const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};
const CN = (name: string) => ({ signature: `/s/ ${name}`, material_desc: "my own question", name, address: "1 Main St", phone: "555-0100", perjury_stmt: true, consent_jurisdiction: true, accept_service: true });
const strikesOf = (w: World, u: User) => w.q<any>(`SELECT upheld_at IS NOT NULL AS u, voided_at IS NOT NULL AS v FROM strikes WHERE user_id = ?`, u.id).map((s) => [s.u, s.v]);
async function preview(w: World, user: User, payload: string, license: string, extra: Q = {}, itemId: string | null = null) {
  const meta: Q = { publish_id: publishId(), hide_key: false, license, attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: "2026-10-22" }, labels: [], order_acks: [], image_ok: [], meta: { title: "t", subject: "Biology", course: "", grade: "", tags: [], standards: [] }, ...extra };
  if (itemId === null) meta.visibility = "public";
  return w.json<any>("POST", itemId ? `/items/${itemId}/preview` : "/items/preview", { user, body: framed(meta, payload), contentType: PUBLISH_CONTENT_TYPE });
}
/** Fails (once) the k-th D1 operation whose SQL matches `re`. */
function inject(w: World, re: RegExp, k = 1) {
  const db = w.db as any, orig = { prepare: db.prepare, batch: db.batch };
  const st = { n: 0, fired: false };
  const tick = (sqls: string[]) => {
    if (!st.fired && sqls.some((s) => re.test(s)) && ++st.n === k) {
      st.fired = true;
      throw new Error("D1_ERROR: injected");
    }
  };
  const wrap = (s: any): any => ({ sql: s.sql, args: s.args, bind: (...a: unknown[]) => wrap(s.bind(...a)), first: async () => (tick([s.sql]), s.first()), all: async () => (tick([s.sql]), s.all()), run: async () => (tick([s.sql]), s.run()) });
  db.prepare = (sql: string) => wrap(orig.prepare.call(db, sql));
  db.batch = async (ss: any[]) => (tick(ss.map((x) => x.sql)), orig.batch.call(db, ss.map((x: any) => ({ sql: x.sql, args: x.args }))));
  return { st, restore: () => Object.assign(db, orig) };
}

const AQ = mcq("Ann's own question: in her 2019 delta survey, which landform formed first at the mouth of the river?", [["A delta lobe", true], ["A mesa", false]]);
const COMMON = mcq("What is the chemical symbol for sodium?", [["Na", true], ["So", false]]);
const EXTRA = (k: string) => mcq(`An unrelated extra question number ${k} about the water cycle and evaporation rates?`, [["Yes", true], ["No", false]]);
const LONG = "Ann's own worked explanation: the logger shows water above twenty degrees on forty days of the 2019 season, juvenile counts fell in exactly those weeks, and the regression slope links them clearly.";

/** Ann's item, copied by Bob (imported), deleted by Ann and purged. */
async function ghost() {
  const w = new World();
  const adm = w.user("admin"), Ann = w.user("teacher", "Ann"), Bob = w.user("teacher", "Bob"), Eve = w.user("teacher", "Eve");
  const a = (await publish(w, Ann, bankOf([AQ]), { license: "CC-BY-NC-4.0" })).body as any;
  await imp(w, Bob, a.item_id);
  const b = (await publish(w, Bob, bankOf([AQ]), { license: "CC-BY-NC-4.0" })).body as any;
  await w.json("DELETE", `/items/${a.item_id}`, { user: Ann });
  await days(w, 16);
  return { w, adm, Ann, Bob, Eve, a, b };
}

// ---- 30b A2: a ghost takedown's plan --------------------------------------------------------------

test("30b A2 (a_ghost, a2_control): a ghost takedown applies exactly the admin's selection and exempt authors, like the live item; with no selection nothing is blocked", async () => {
  const run = async (gone: boolean) => {
    const w = new World();
    const adm = w.user("admin"), Ann = w.user("teacher", "Ann"), Carol = w.user("teacher", "Carol");
    const others = [w.user("teacher", "Dan"), w.user("teacher", "Erin"), w.user("teacher", "Finn")];
    const c = (await publish(w, Carol, bankOf([COMMON, EXTRA("c")]))).body as any;
    const os: any[] = [];
    for (const [i, u] of others.entries()) os.push((await publish(w, u, bankOf([COMMON, EXTRA(String(i))]))).body);
    const R = w.user();
    const a = (await publish(w, Ann, bankOf([AQ, COMMON]))).body as any;
    await get(w, R, a.item_id);
    if (gone) {
      await w.json("DELETE", `/items/${a.item_id}`, { user: Ann });
      await days(w, 16);
      const nid0 = await notice(w, adm);
      const none = await w.takedown<any>(adm, nid0, a.item_id);
      assert.equal(code(none), "409 selection_required", "no selection: nothing blocked");
      assert.equal(w.q(`SELECT 1 FROM blocked_hashes`).length + w.q(`SELECT 1 FROM dmca_actions`).length, 0);
      const pv0 = await w.admin<any>(adm, "POST", "/admin/dmca/takedown-preview", { notice_id: nid0, item_id: a.item_id });
      assert.deepEqual([pv0.body.ghost, pv0.body.selection_required, pv0.body.match.length], [true, true, 2], "the preview lists the ghost's positions");
    }
    const nid = await notice(w, adm);
    const pv = await w.admin<any>(adm, "POST", "/admin/dmca/takedown-preview", { notice_id: nid, item_id: a.item_id, exempt_authors: [Carol.id], questions: [0] });
    assert.deepEqual(pv.body.match.map((m: any) => [m.pos, m.selected]), [[0, true], [1, false]]);
    assert.equal((await w.takedown<any>(adm, nid, a.item_id, { exempt_authors: [Carol.id], questions: [0] })).status, 200);
    const blocks = w.q<any>(`SELECT hash FROM blocked_hashes WHERE lifted_at IS NULL ORDER BY hash`).map((r) => r.hash);
    const exempt = w.q(`SELECT 1 FROM dmca_exempt`).length;
    const holds: string[] = [];
    for (const [u, p, tag] of [[Carol, c, "c"], ...others.map((u, i) => [u, os[i], String(i)])] as any[]) {
      assert.equal((await publish(w, u, bankOf([...own(p, [COMMON, EXTRA(tag)]), EXTRA(`${tag}-new`)]), { base: p.publish_id }, p.item_id)).status, 200);
      holds.push(...w.q<any>(`SELECT source FROM item_holds WHERE item_id = ?`, p.item_id).map((r) => r.source));
    }
    return { blocks, exempt, holds };
  };
  const live = await run(false);
  const gh = await run(true);
  assert.deepEqual(gh, live, "the same blocks, exempt rows and (no) holds, live or ghost");
  assert.equal(live.exempt, 1);
  assert.deepEqual(live.holds, []);
});

// ---- 30b A1, F1, A3: upholds ----------------------------------------------------------------------

test("30b A1 (a_ghost): a ghost takedown with a counter-notice and a court action is upheld by resolve 'remove': strike, evidence void, copy held; a repeat is refused, nothing doubled", async () => {
  const { w, adm, Ann, a, b } = await ghost();
  const t = await w.takedown<any>(adm, await notice(w, adm), a.item_id, { questions: "all" });
  const aid = t.body.action_id;
  assert.equal((await w.json<any>("POST", `/items/${a.item_id}/counter-notice`, { user: Ann, json: CN("Ann") })).status, 202);
  assert.equal((await w.admin<any>(adm, "POST", "/admin/dmca/court-action", { action_id: aid })).status, 200);
  const r = await w.admin<any>(adm, "POST", "/admin/dmca/resolve", { action_id: aid, outcome: "remove", note: "court sided with complainant" });
  assert.equal(r.status, 200);
  assert.deepEqual(w.q<any>(`SELECT outcome, uphold_pending FROM dmca_actions WHERE id = ?`, aid)[0], { outcome: "court_upheld", uphold_pending: 0 });
  assert.deepEqual(strikesOf(w, Ann), [[1, 0]]);
  assert.equal(w.q(`SELECT 1 FROM void_items WHERE item_id = ?`, a.item_id).length, 1);
  assert.equal(w.q(`SELECT 1 FROM item_holds WHERE item_id = ?`, b.item_id).length, 1);
  assert.ok((await w.admin<any>(adm, "POST", "/admin/dmca/resolve", { action_id: aid, outcome: "remove" })).status >= 400);
  await days(w, 40);
  assert.deepEqual(strikesOf(w, Ann), [[1, 0]]);
});

test("30b F1 (f_faults): a ghost uphold that fails after its claim is finished by the cron", async () => {
  const { w, adm, Ann, a, b } = await ghost();
  const t = await w.takedown<any>(adm, await notice(w, adm), a.item_id, { questions: "all" });
  await days(w, 29);
  const inj = inject(w, /UPDATE strikes SET upheld_at = \?2 WHERE action_id = \?1/);
  await quiet(() => days(w, 2));
  inj.restore();
  assert.ok(inj.st.fired);
  await days(w, 1);
  assert.deepEqual(w.q<any>(`SELECT outcome, uphold_pending FROM dmca_actions WHERE id = ?`, t.body.action_id)[0], { outcome: "upheld", uphold_pending: 0 });
  assert.deepEqual(strikesOf(w, Ann), [[1, 0]]);
  assert.equal(w.q(`SELECT 1 FROM void_items`).length, 1);
  assert.equal(w.q(`SELECT 1 FROM item_holds WHERE item_id = ?`, b.item_id).length, 1);
});

test("30b A3 (a_ghost): a counter-notice that loses the race with the day-30 uphold is a 409 window_closed (no 202, no row); the admin can reverse the uphold", async () => {
  const w = new World();
  const adm = w.user("admin"), Ann = w.user("teacher", "Ann"), Bob = w.user("teacher", "Bob");
  const a = (await publish(w, Ann, bankOf([AQ]), { license: "CC-BY-NC-4.0" })).body as any;
  await imp(w, Bob, a.item_id);
  const b = (await publish(w, Bob, bankOf([AQ]), { license: "CC-BY-NC-4.0" })).body as any;
  await w.json("DELETE", `/items/${a.item_id}`, { user: Ann });
  await days(w, 16);
  const t = await w.takedown<any>(adm, await notice(w, adm), a.item_id, { questions: "all" });
  const aid = t.body.action_id;
  const notified = w.q<any>(`SELECT author_notified_at AS n FROM dmca_actions WHERE id = ?`, aid)[0].n;
  w.clock = notified + 30 * DAY - 1;
  const db = w.db as any, orig = db.batch;
  let fired = false;
  db.batch = async (ss: any[]) => {
    if (!fired && ss.some((s: any) => /SET counter_notice_id = \?2/.test(s.sql))) {
      fired = true;
      w.clock += 2;
      await w.pool.daily();
    }
    return orig.call(db, ss);
  };
  const cn = await w.json<any>("POST", `/items/${a.item_id}/counter-notice`, { user: Ann, json: CN("Ann") });
  db.batch = orig;
  assert.ok(fired);
  assert.equal(code(cn), "409 window_closed");
  assert.equal(w.q(`SELECT 1 FROM dmca_counter_notices`).length, 0);
  assert.deepEqual(strikesOf(w, Ann), [[1, 0]]);
  // The admin reverses it (e.g. the counter-notice was sent in time by mail).
  const rv = await w.admin<any>(adm, "POST", "/admin/dmca/resolve", { action_id: aid, outcome: "put_back" });
  assert.equal(rv.status, 200);
  assert.deepEqual(strikesOf(w, Ann), [[1, 1]]);
  assert.equal(w.q(`SELECT 1 FROM void_items`).length, 0, "evidence restored");
  assert.equal(w.q(`SELECT 1 FROM item_holds WHERE item_id = ?`, b.item_id).length, 0);
  assert.equal(w.q(`SELECT 1 FROM blocked_hashes WHERE lifted_at IS NULL`).length, 0);
});

// ---- 30b F2, F3, Z1: the author's notice and list ---------------------------------------------------

test("30b F2/F3 (f_faults): the author's takedown notice is written with the action: a failure after it never loses the mail (ghost and live), and it is sent once", async () => {
  {
    const { w, adm, a } = await ghost();
    const nid = await notice(w, adm);
    w.notices.length = 0;
    const inj = inject(w, /FROM pool_outbox WHERE sent_at IS NULL AND id IN/);
    const t1 = await quiet(() => w.takedown<any>(adm, nid, a.item_id, { questions: "all" }).catch(() => ({ status: 500 })));
    inj.restore();
    assert.ok(inj.st.fired && t1.status >= 500);
    const t2 = await w.takedown<any>(adm, nid, a.item_id, { questions: "all" });
    assert.equal(t2.status, 200);
    await days(w, 1);
    assert.deepEqual(w.notices.map((n: any) => n.kind).filter((k: string) => k === "takedown_author"), ["takedown_author"]);
  }
  {
    const w = new World();
    const adm = w.user("admin"), Ann = w.user("teacher", "Ann");
    const a = (await publish(w, Ann, bankOf([AQ]))).body as any;
    const nid = await notice(w, adm);
    w.notices.length = 0;
    const inj = inject(w, /INSERT INTO r2_orphans \(key, at, next_at\)/);
    await quiet(() => w.takedown<any>(adm, nid, a.item_id).catch(() => null));
    inj.restore();
    assert.ok(inj.st.fired);
    await days(w, 1);
    assert.deepEqual(w.notices.map((n: any) => n.kind).filter((k: string) => k === "takedown_author"), ["takedown_author"], "sent by the hourly outbox although the request failed");
    assert.equal((await w.takedown<any>(adm, nid, a.item_id)).status, 200);
    assert.equal(w.notices.filter((n: any) => n.kind === "takedown_author").length, 1, "once");
  }
});

test("30b Z1 (z_authz): /items/mine lists a ghost takedown awaiting the author's counter-notice; only she can counter it", async () => {
  const { w, adm, Ann, Bob, a } = await ghost();
  const t = await w.takedown<any>(adm, await notice(w, adm), a.item_id, { questions: "all" });
  const mine = await w.json<any>("GET", "/items/mine", { user: Ann });
  const row = mine.body.items.find((x: any) => x.id === a.item_id);
  assert.deepEqual([row?.state, row?.action_id, row?.counter_notice_sent], ["takedown_pending", t.body.action_id, false]);
  assert.ok(row.counter_notice_by > w.clock);
  assert.equal(Object.keys(row).filter((k) => /title|stem|question/.test(k)).length, 0, "no content");
  assert.equal(code(await w.json<any>("POST", `/items/${a.item_id}/counter-notice`, { user: Bob, json: CN("Bob") })), "404 not_found");
  assert.equal((await w.json<any>("POST", `/items/${a.item_id}/counter-notice`, { user: Ann, json: CN("Ann") })).status, 202);
  assert.equal((await w.json<any>("GET", "/items/mine", { user: Ann })).body.items.find((x: any) => x.id === a.item_id).counter_notice_sent, true);
});

// ---- 30b F4, G1, T1 ---------------------------------------------------------------------------------

test("30b F4 (f_faults): keep_on_delete holds when the erasure batch fails and so does that day's retry", async () => {
  const w = new World();
  const U = w.user("teacher", "Ursula");
  const x = (await publish(w, U, bankOf([q("Ursula's question about erosion on hillsides after rain?")]))).body as any;
  await w.json("PUT", `/items/prefs`, { user: U, json: { keep_on_delete: true } });
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, U.id);
  const i1 = inject(w, /INSERT OR IGNORE INTO erasure_done/);
  await quiet(() => w.db.batch(deleteContentStatements(w.db, U.id, w.clock)).catch(() => null));
  i1.restore();
  const i2 = inject(w, /INSERT OR IGNORE INTO erasure_done/);
  w.advance(DAY);
  await quiet(() => w.pool.daily());
  i2.restore();
  assert.ok(i1.st.fired && i2.st.fired);
  await days(w, 20);
  assert.deepEqual(w.q(`SELECT anonymized, removed_reason FROM items WHERE id = ?`, x.item_id), [{ anonymized: 1, removed_reason: null }]);
});

test("30b G1 (g_putback_fault): a ghost withdrawal whose hold release fails is finished by the cron (and by a retry)", async () => {
  const w = new World();
  const adm = w.user("admin"), Ann = w.user("teacher", "Ann"), Dan = w.user("teacher", "Dan"), R = w.user();
  const EX = mcq("Dan's extra question about evaporation over warm lakes in summer?", [["Yes", true], ["No", false]]);
  const d = (await publish(w, Dan, bankOf([AQ]))).body as any;
  const a = (await publish(w, Ann, bankOf([AQ, COMMON]))).body as any;
  await get(w, R, a.item_id);
  await w.json("DELETE", `/items/${a.item_id}`, { user: Ann });
  await days(w, 16);
  const nid = await notice(w, adm);
  assert.equal((await w.takedown<any>(adm, nid, a.item_id, { questions: [0] })).status, 200);
  await publish(w, Dan, bankOf([...own(d, [AQ]), EX]), { base: d.publish_id }, d.item_id);
  assert.equal(w.q(`SELECT 1 FROM item_holds WHERE item_id = ?`, d.item_id).length, 1, "held by the re-upload block");
  const inj = inject(w, /^\s*DELETE FROM item_holds WHERE source = \?1 AND reason = \?2 AND NOT/);
  await quiet(() => w.admin<any>(adm, "POST", "/admin/dmca/withdraw", { notice_id: nid, note: "withdrawn" }).catch(() => null));
  inj.restore();
  assert.ok(inj.st.fired);
  await days(w, 1);
  assert.equal(w.q(`SELECT 1 FROM item_holds WHERE item_id = ?`, d.item_id).length, 0);
  assert.equal((await get(w, R, d.item_id)).status, 200);
});

test("30b T1 (t_audit): an erasure nulls the notes the account wrote; an admin's note about it stays, its name scrubbed when written", async () => {
  const w = new World();
  const adm = w.user("admin"), Inf = w.user("teacher", "Ingrid Fallowmere");
  const x = (await publish(w, Inf, bankOf([q("Infringer's pasted question about plate boundaries over geologic time?")]))).body as any;
  await w.admin(adm, "POST", "/admin/pool/remove", { item_id: x.item_id, note: "EVIDENCE: Ingrid Fallowmere pasted publisher X page 41" });
  assert.equal((await w.json("POST", `/items/${x.item_id}/appeal`, { user: Inf, json: { note: "Please, it's mine" } })).status, 202);
  assert.equal(w.q(`SELECT 1 FROM pool_audit WHERE actor_id = ? AND note IS NOT NULL`, Inf.id).length, 1, "her appeal note");
  const adminNote = () => w.q<any>(`SELECT note FROM pool_audit WHERE note LIKE 'EVIDENCE%'`)[0]?.note;
  assert.equal(adminNote(), `EVIDENCE: [user ${Inf.id}] pasted publisher X page 41`);
  await erase(w, Inf);
  assert.equal(adminNote(), `EVIDENCE: [user ${Inf.id}] pasted publisher X page 41`, "kept as evidence");
  assert.equal(w.q(`SELECT 1 FROM pool_audit WHERE actor_id = ? AND note IS NOT NULL`, Inf.id).length, 0, "her own notes go");
});

// ---- 30b W1, 30a B4: the licence history --------------------------------------------------------

test("30b W1 (w_amp): flipping a bank's licence keeps one row per content and licence (re-opened, never duplicated); closed rows no serve reaches are collected", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann");
  const N = 60;
  const qs = Array.from({ length: N }, (_, i) => q(`Generated question ${i} about rivers deltas lakes and the number ${i * 7919}?`, { explanation: `Explanation ${i}: the delta forms where the river ${i} slows and drops sediment into the basin over many seasons of floods.`, kind: { Short: { sample: `answer ${i} sample text` } } }));
  let cur = (await publish(w, A, bankOf(qs), { license: "CC-BY-4.0" })).body as any;
  const rows0 = w.q<any>(`SELECT COUNT(*) AS n FROM content_licences`)[0].n;
  for (let k = 0; k < 6; k++) {
    w.advance(60_000);
    const r = await publish(w, A, bankOf(own(cur, qs)), { license: ["CC-BY-NC-4.0", "CC-BY-4.0"][k % 2]!, base: cur.publish_id }, cur.item_id);
    assert.equal(r.status, 200);
    cur = { ...cur, ...r.body };
  }
  const rows = w.q<any>(`SELECT COUNT(*) AS n FROM content_licences`)[0].n;
  assert.equal(rows, rows0 * 2, "contents x 2 licences, whatever the number of flips");
  assert.equal(w.q(`SELECT 1 FROM content_licences GROUP BY item_id, ckey, license HAVING COUNT(*) > 1`).length, 0);
  // Nobody was served it: the closed rows are collected.
  await days(w, 1);
  assert.equal(w.q<any>(`SELECT COUNT(*) AS n FROM content_licences WHERE until_at IS NOT NULL`)[0].n, 0);
  assert.equal(w.q<any>(`SELECT COUNT(*) AS n FROM content_licences`)[0].n, rows0, "the open rows stay");
});

test("30a B4 (s9_private_toggle): a licence counts only while the item offers it publicly: a CC0 window while unlisted grants nothing", async () => {
  for (const v of ["private CC0 window", "public CC0 window", "private window, reader first served after"] as const) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    let a: any = (await publish(w, A, bankOf([AQ]), { license: "CC-BY-NC-4.0" })).body;
    const id = a.item_id;
    const rp = async (L: string) => {
      w.advance(DAY);
      const r = await publish(w, A, bankOf(own(a, [AQ])), { license: L, base: a.publish_id }, id);
      assert.equal(r.status, 200);
      a = { ...r.body, item_id: id };
    };
    if (v !== "private window, reader first served after") await imp(w, M, id);
    if (v !== "public CC0 window") await w.json("PATCH", `/items/${id}`, { user: A, json: { visibility: "unlisted" } });
    await rp("CC0-1.0");
    await rp("CC-BY-NC-4.0");
    if (v !== "public CC0 window") await w.json("PATCH", `/items/${id}`, { user: A, json: { visibility: "public" } });
    if (v === "private window, reader first served after") {
      w.advance(DAY);
      await imp(w, M, id);
    }
    assert.equal(code(await preview(w, M, bankOf([AQ]), "CC0-1.0")), v === "public CC0 window" ? "200" : "422 license_incompatible", v);
  }
});

// ---- 30a B1: the key is permanent ---------------------------------------------------------------

test("30a B1 (s1_rotation, s3_rotation_refuse): under another POOL_HASH_KEY every write and cron fails closed (503 pool_key_mismatch, one alert), nothing changes; reads continue", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), adm = w.user("admin");
  const a = (await publish(w, A, bankOf([mcq("Some question about rivers and deltas in the valley floor?", [["A", true], ["B", false]], { explanation: LONG })]), { license: "CC-BY-NC-4.0" })).body as any;
  assert.equal((await imp(w, M, a.item_id)).status, 200);
  const snapshot = () => JSON.stringify(["items", "item_questions", "served", "imports", "content_licences", "author_contents", "blocked_hashes"].map((t) => w.q(`SELECT * FROM ${t}`)));
  const before = snapshot();
  const w2 = new World({ sql: w.sql, r2: w.r2, core: makeCore(shareCore as any, { hashKey: "7a".repeat(32) }) });
  w2.tokens = w.tokens;
  (w2 as any).n = 50;
  w2.clock = w.clock;
  try {
    for (const r of [
      await publish(w2, M, bankOf([mcq("Mal's question about deltas?", [["A", true], ["B", false]], { explanation: LONG })]), { license: "CC0-1.0" }),
      await w2.json<any>("POST", `/items/${a.item_id}/import`, { user: M }),
      await w2.json<any>("POST", `/items/${a.item_id}/report`, { user: M, json: { reason: "other", note: "x" } }),
      await w2.admin<any>(adm, "POST", "/admin/pool/hold", { item_id: a.item_id, note: "x" }),
    ])
      assert.equal(code(r), "503 pool_key_mismatch");
    assert.equal((await w2.json<any>("GET", `/items/${a.item_id}`, { user: M })).status, 200, "reads continue");
    assert.equal(await w2.pool.daily(), null, "the cron does nothing");
    assert.equal(snapshot(), before, "nothing changed");
    assert.equal(w2.notices.filter((n: any) => n.kind === "key_mismatch").length, 1, "one alert");
    assert.equal((await w2.admin<any>(adm, "GET", "/admin/pool/key", null)).body.match, false);
  } finally {
    makeCore(shareCore as any, { hashKey: TEST_HASH_KEY });
  }
  assert.equal((await imp(w, M, a.item_id)).status, 200, "the original key works again");
});

// ---- 30a B2: erased authors on every path ---------------------------------------------------------

test("30a B2 (s4_erased_live_td): an upheld takedown of an erased author's item never voids its evidence without a rightful item: anonymised, kept, within the 14 days or a ghost", async () => {
  for (const mode of ["ghost", "anonymised-imported", "anonymised-keep", "removed-within-14d"] as const) {
    const w = new World();
    const adm = w.user("admin"), A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    const a = (await publish(w, A, bankOf([AQ]), { license: "CC-BY-NC-4.0" })).body as any;
    if (mode === "ghost" || mode === "anonymised-imported") await imp(w, M, a.item_id);
    else await get(w, M, a.item_id);
    const m = (await publish(w, M, bankOf([AQ]), { license: "CC-BY-NC-4.0" })).body as any;
    if (mode === "anonymised-keep") await w.json("PUT", "/items/prefs", { user: A, json: { keep_on_delete: true } });
    if (mode === "ghost") await w.json("DELETE", `/items/${a.item_id}`, { user: A });
    await erase(w, A);
    if (mode === "ghost") await days(w, 16);
    assert.equal((await w.takedown<any>(adm, await notice(w, adm), a.item_id, { questions: "all" })).status, 200, mode);
    await days(w, 31);
    assert.equal(w.q(`SELECT 1 FROM void_items`).length, 0, mode);
    w.sql.prepare(`DELETE FROM item_holds WHERE item_id = ?`).run(m.item_id);
    assert.equal(code(await publish(w, M, bankOf(own(m, [AQ])), { license: "CC0-1.0", base: m.publish_id }, m.item_id)), "422 license_incompatible", mode);
    assert.equal(code(await preview(w, M, bankOf([AQ]), "CC0-1.0")), "422 license_incompatible", mode);
  }
});

// ---- 30a B3: the earliest author is never bound by a later copier -------------------------------

const STEMS = ["Ann's own question: in her 2019 delta survey, which landform formed first at the mouth of the Feather river?", "Ann's own question: in her 2019 delta survey, which landform formed first at the mouth of the Feather river channel?"];
const C = (sv: number, e: number) => mcq(STEMS[sv]!, [["A delta lobe of fresh sediment", true], ["A mesa of old rock", false], ["A butte", false]], { explanation: e ? LONG : null });

test("30a B3 (s6_min, s6b): the original author's own edits (stem reworded, explanation removed, back again) never credit or bind her to the copier she never read", async () => {
  for (const [steps, cp] of [
    [["e1", "imp", "e0", "e1", "copy", "s1", "e0"], [0, 1]],
    [["e1", "imp", "copy", "s1", "e0"], [0, 1]],
    [["e1", "imp", "copy", "s1"], [0, 1]],
    [["imp", "copy", "s1"], [0, 0]],
    [["e1", "imp", "copy", "e0"], [0, 1]],
    [["e1", "imp", "copy", "s1", "e0", "s0"], [0, 1]],
  ] as [string[], [number, number]][]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), E = w.user("teacher", "Eve");
    let sv = 0, e = 0;
    let a: any = (await publish(w, A, bankOf([C(sv, e)]), { license: "CC-BY-NC-SA-4.0" })).body;
    const id = a.item_id;
    for (const s of steps) {
      w.advance(DAY);
      if (s === "imp") await imp(w, M, id);
      else if (s === "copy") assert.equal((await publish(w, M, bankOf([C(...cp)]), { license: "CC-BY-NC-SA-4.0" })).status, 201);
      else {
        if (s === "e1") e = 1;
        if (s === "e0") e = 0;
        if (s === "s1") sv = 1;
        if (s === "s0") sv = 0;
        const r = await publish(w, A, bankOf(own(a, [C(sv, e)])), { license: "CC-BY-NC-SA-4.0", base: a.publish_id }, id);
        assert.equal(r.status, 200);
        a = { ...r.body, item_id: id };
        assert.deepEqual((await get(w, E, id)).body.questions[0].chain, [], `${steps.join(" ")}: no credit to the copier after ${s}`);
      }
    }
    w.advance(DAY);
    for (const L of ["CC0-1.0", "CC-BY-SA-4.0"]) assert.equal(code(await preview(w, A, bankOf(own(a, [C(sv, e)])), L, { base_publish_id: a.publish_id }, id)), "200", `${steps.join(" ")}: relicense ${L}`);
  }
  // ... and after her item is deleted and purged, her new item with the same material is still hers.
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), E = w.user("teacher", "Eve");
  let a: any = (await publish(w, A, bankOf([C(0, 1)]), { license: "CC-BY-NC-SA-4.0" })).body;
  w.advance(DAY);
  await imp(w, M, a.item_id);
  assert.equal((await publish(w, M, bankOf([C(0, 1)]), { license: "CC-BY-NC-SA-4.0" })).status, 201);
  w.advance(DAY);
  a = { ...(await publish(w, A, bankOf(own(a, [C(1, 1)])), { license: "CC-BY-NC-SA-4.0", base: a.publish_id }, a.item_id)).body, item_id: a.item_id };
  await w.json("DELETE", `/items/${a.item_id}`, { user: A });
  await days(w, 16);
  const n = await publish(w, A, bankOf([C(1, 0)]), { license: "CC-BY-SA-4.0" });
  assert.equal(n.status, 201);
  assert.deepEqual((await get(w, E, n.body.item_id as string)).body.questions[0].chain, [], "no earlier_match credit to the copier");
  assert.deepEqual(w.q(`SELECT kind FROM admin_flags WHERE item_id = ?`, n.body.item_id), []);
});

// ---- variants ------------------------------------------------------------------------------------

test("30a variants: a ghost put-back flags admin-held copies for release as a live one does; an erased account's item can be taken down inside its 14 days", async () => {
  const { w, adm, Ann, a, b } = await ghost();
  const nid = await notice(w, adm);
  const t = await w.takedown<any>(adm, nid, a.item_id, { questions: "all" });
  assert.equal((await w.holdCopies<any>(adm, t.body.action_id)).status, 200);
  assert.equal(w.q(`SELECT 1 FROM item_holds WHERE item_id = ?`, b.item_id).length, 1);
  await w.admin<any>(adm, "POST", "/admin/dmca/withdraw", { notice_id: nid, note: "x" });
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'dmca_copy' AND dedup = ?`, `putback:${t.body.action_id}`).length, 1);
  void Ann;
  const w2 = new World();
  const adm2 = w2.user("admin"), U = w2.user("teacher", "Ulla"), R = w2.user();
  const x = (await publish(w2, U, bankOf([AQ]))).body as any;
  await get(w2, R, x.item_id);
  await erase(w2, U);
  assert.equal(w2.q<any>(`SELECT removed_reason FROM items WHERE id = ?`, x.item_id)[0].removed_reason, "account_deleted");
  assert.equal((await w2.takedown<any>(adm2, await notice(w2, adm2), x.item_id)).status, 200);
});

// ---- differential: the honest author is never bound by a later publisher --------------------------

const L5 = ["CC0-1.0", "CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"];
function sm(seed: number) {
  let s = Math.imul(seed ^ 0x5bd1e995, 0x27d4eb2d) >>> 0;
  return () => {
    s = (s + 0x9e3779b9) >>> 0;
    let z = s;
    z = Math.imul(z ^ (z >>> 16), 0x85ebca6b) >>> 0;
    z = Math.imul(z ^ (z >>> 13), 0xc2b2ae35) >>> 0;
    return ((z ^ (z >>> 16)) >>> 0) / 2 ** 32;
  };
}
const SEEDS = Number(process.env.POOL_PROPERTY_SEEDS ?? 10);
const START = Number(process.env.POOL_PROPERTY_START ?? 0);

test(`round 23 differential (${SEEDS} seeds): for any sequence of her edits, copies by others, deletes and purges, the earliest author's own versions are never refused, credited or bound because of a later publisher`, async () => {
  void grantOptions;
  for (let seed = START; seed < START + SEEDS; seed++) {
    const r = sm(seed);
    const pick = <T>(xs: T[]) => xs[Math.floor(r() * xs.length)]!;
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), B = w.user("teacher", "Bea"), E = w.user("teacher", "Eve");
    let sv = 0, e = Math.floor(r() * 2);
    let a: any = (await publish(w, A, bankOf([C(sv, e)]), { license: pick(L5) })).body;
    let gone = false;
    const versions = new Set<string>([`${sv}${e}`]);
    const ops: string[] = [];
    for (let i = 0; i < 4 + Math.floor(r() * 6); i++) {
      w.advance(DAY / 2 + Math.floor(r() * DAY));
      const k = Math.floor(r() * 6);
      if (k <= 1 && !gone) {
        if (r() < 0.5) sv = 1 - sv;
        else e = 1 - e;
        const x = await publish(w, A, bankOf(own(a, [C(sv, e)])), { license: pick(L5), base: a.publish_id }, a.item_id);
        ops.push(`A${sv}${e}:${x.status}`);
        assert.equal(x.status, 200, `seed ${seed}: ${ops.join(" ")}`);
        a = { ...x.body, item_id: a.item_id };
        versions.add(`${sv}${e}`);
      } else if (k === 2 && !gone) {
        const u = pick([M, B]);
        await (r() < 0.5 ? imp(w, u, a.item_id) : get(w, u, a.item_id));
        const v = pick([...versions]);
        const x = await publish(w, u, bankOf([C(Number(v[0]), Number(v[1]))]), { license: pick(L5) });
        ops.push(`${u.name}copy${v}:${x.status}`);
      } else if (k === 3 && !gone && r() < 0.4) {
        await w.json("DELETE", `/items/${a.item_id}`, { user: A });
        await days(w, 16);
        gone = true;
        ops.push("purged");
      } else ops.push("noop");
    }
    // Every version Ann ever published, re-published by her (new item), under every licence she ever
    // could: never refused or credited because of Mal or Bea.
    for (const v of versions) {
      const p = await preview(w, A, bankOf([C(Number(v[0]), Number(v[1]))]), "CC0-1.0");
      assert.equal(p.status, 200, `seed ${seed} ${v}: ${code(p)} ${JSON.stringify(p.body.per_question ?? "")} ops ${ops.join(" ")}`);
      const chainNames = JSON.stringify(p.body.chains ?? []);
      assert.ok(!chainNames.includes(M.id) && !chainNames.includes(B.id) && !/Mal|Bea/.test(chainNames), `seed ${seed} ${v}: credited ${chainNames} ops ${ops.join(" ")}`);
    }
    if (!gone) assert.deepEqual((await get(w, E, a.item_id)).body.questions[0].chain, [], `seed ${seed}: her item credits nobody`);
  }
});

// ---- 30b p_publish_faults, e_erasure ------------------------------------------------------------

test("30b (p_publish_faults): a republish (relicence + new text) failing at any D1 operation leaves no partial state, and its retry lands", async () => {
  const bad: string[] = [];
  let ops = 0;
  for (let k = 1; k < 400; k++) {
    const w = new World();
    const A = w.user("teacher", "Ann");
    const Q0 = q("Ann's question about deltas and lobes at the river mouth?", { explanation: "Ann's explanation one about sediment." });
    const Q1 = q("Ann's second question about moraines and valleys?");
    const p = (await publish(w, A, bankOf([Q0, Q1]), { license: "CC-BY-4.0" })).body as any;
    w.advance(60_000);
    const next = [{ ...own(p, [Q0])[0], explanation: "Ann's NEW explanation about sediment and slowing water." }, own(p, [Q0, Q1])[1]];
    let n = 0;
    const db = w.db as any, orig = { prepare: db.prepare, batch: db.batch };
    const tick = () => {
      if (++n === k) throw new Error("D1_ERROR: injected");
    };
    const wrap = (s: any): any => ({ sql: s.sql, args: s.args, bind: (...a: unknown[]) => wrap(s.bind(...a)), first: async () => (tick(), s.first()), all: async () => (tick(), s.all()), run: async () => (tick(), s.run()) });
    db.prepare = (sql: string) => wrap(orig.prepare.call(db, sql));
    db.batch = async (ss: any[]) => (tick(), orig.batch.call(db, ss.map((x: any) => ({ sql: x.sql, args: x.args }))));
    await quiet(() => publish(w, A, bankOf(next), { license: "CC-BY-NC-4.0", base: p.publish_id }, p.item_id).catch(() => null));
    Object.assign(db, orig);
    if (n < k) {
      ops = k - 1;
      break;
    }
    const it = w.q<any>(`SELECT license, current_publish_id AS cp FROM items WHERE id = ?`, p.item_id)[0];
    const open = w.q<any>(`SELECT license FROM content_licences WHERE item_id = ? AND until_at IS NULL GROUP BY license`, p.item_id);
    if (it.license !== (it.cp === p.publish_id ? "CC-BY-4.0" : "CC-BY-NC-4.0") || open.length !== 1 || open[0].license !== it.license) bad.push(`op ${k}: ${JSON.stringify({ it, open })}`);
    const r2 = await publish(w, A, bankOf(next), { license: "CC-BY-NC-4.0", base: it.cp }, p.item_id);
    const open2 = w.q<any>(`SELECT license FROM content_licences WHERE item_id = ? AND until_at IS NULL GROUP BY license`, p.item_id);
    if (!(r2.status === 200 || (r2.status === 409 && it.cp !== p.publish_id)) || open2.length !== 1 || open2[0].license !== "CC-BY-NC-4.0") bad.push(`op ${k} retry ${code(r2)} ${JSON.stringify(open2)}`);
  }
  assert.ok(ops > 20, `swept ${ops} operations`);
  assert.deepEqual(bad, []);
});

test("30b (e_erasure): after an erasure and 40 days of crons no table keeps the account's typed text or name, beyond ids and an admin's (scrubbed) evidence note", async () => {
  const w = new World();
  const adm = w.user("admin"), U = w.user("teacher", "Ursulaname"), B = w.user("teacher", "Bob"), R = w.user();
  const MARK = "ursulamark";
  const x2 = (await publish(w, U, bankOf([q(`Ursula ${MARK}2 question about glaciers and moraines in valleys?`)]))).body as any;
  await w.admin(adm, "POST", "/admin/pool/remove", { item_id: x2.item_id, note: `admin note about Ursulaname` });
  await w.json("POST", `/items/${x2.item_id}/appeal`, { user: U, json: { note: `appeal ${MARK}appealnote` } });
  const x3 = (await publish(w, U, bankOf([mcq(`Ursula ${MARK}3 question about delta lobes and mouths of rivers?`, [["A lobe", true], ["A mesa", false]])]))).body as any;
  await get(w, R, x3.item_id);
  await w.json("DELETE", `/items/${x3.item_id}`, { user: U });
  const bi = (await publish(w, B, bankOf([q("Bob's own question about clouds forming above warm lakes?")]))).body as any;
  await imp(w, U, bi.item_id);
  await w.json("POST", `/items/${bi.item_id}/report`, { user: U, json: { reason: "other", note: `report ${MARK}reportnote` } });
  await w.json("PUT", `/items/prefs`, { user: U, json: { keep_on_delete: false } });
  await days(w, 16);
  assert.equal((await w.takedown<any>(adm, await notice(w, adm), x3.item_id, { questions: "all" })).status, 200);
  assert.equal((await w.json("POST", `/items/${x3.item_id}/counter-notice`, { user: U, json: { ...CN(`CN${MARK}`), address: `addr${MARK}` } })).status, 202);
  await erase(w, U);
  await days(w, 40);
  const hits: string[] = [];
  for (const { name } of w.q<{ name: string }>(`SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'`))
    for (const row of w.q<Record<string, unknown>>(`SELECT * FROM "${name}"`))
      for (const [k, v] of Object.entries(row)) if (typeof v === "string" && (v.includes(MARK) || v.includes("Ursulaname") || v.includes("1 Main St") || v.includes("555-0100"))) hits.push(`${name}.${k}`);
  assert.deepEqual(hits, []);
  for (const k of w.r2.keys("")) assert.ok(!new TextDecoder().decode(w.r2.objects.get(k)!.bytes).includes(MARK), k);
  assert.equal(w.q<any>(`SELECT note FROM pool_audit WHERE action = 'item.remove' OR note LIKE 'admin note%'`).map((r) => r.note).filter(Boolean).join(), `admin note about [user ${U.id}]`);
});

// ---- owner: the publishing attestation --------------------------------------------------------------

test("owner: publish and republish need the teacher's attestation of the current terms (preview does not); it is recorded per publish and shown to admins only", async () => {
  const { POOL_TERMS_VERSION, ATTEST_STATEMENT } = await import("../../src/pool/license.ts");
  const w = new World();
  const A = w.user("teacher", "Ann"), R = w.user(), adm = w.user("admin");
  const Q0 = mcq("Ann's own attested question about deltas and river mouths?", [["A lobe", true], ["A mesa", false]]);
  for (const bad of [null, { own_work: false, terms_version: POOL_TERMS_VERSION }, { own_work: true, terms_version: "2020-01-01" }, { own_work: "yes", terms_version: POOL_TERMS_VERSION }]) {
    const r = await publish(w, A, bankOf([Q0]), { attest: bad });
    assert.equal(code(r), "422 attestation_required", JSON.stringify(bad));
    assert.deepEqual([r.body.terms_version, r.body.statement], [POOL_TERMS_VERSION, ATTEST_STATEMENT]);
  }
  assert.equal(w.q(`SELECT 1 FROM items`).length, 0, "nothing published");
  // A preview does not need it.
  const meta = { publish_id: publishId(), hide_key: false, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, labels: [], order_acks: [], image_ok: [], visibility: "public", meta: { title: "t", subject: "Biology", course: "", grade: "", tags: [], standards: [] } };
  assert.equal((await w.json<any>("POST", "/items/preview", { user: A, body: framed(meta, bankOf([Q0])), contentType: PUBLISH_CONTENT_TYPE })).status, 200);
  w.clock += 1000;
  const a = (await publish(w, A, bankOf([Q0]))).body as any;
  const at0 = w.clock;
  w.advance(DAY);
  assert.equal(code(await publish(w, A, bankOf(own(a, [{ ...Q0, points: 2 }])), { base: a.publish_id, attest: null }, a.item_id)), "422 attestation_required", "a republish too");
  const rp = (await publish(w, A, bankOf(own(a, [{ ...Q0, points: 2 }])), { base: a.publish_id }, a.item_id)).body as any;
  assert.deepEqual(w.q(`SELECT publish_id, user_id, at, terms_version FROM attestations ORDER BY at`), [
    { publish_id: a.publish_id, user_id: A.id, at: at0, terms_version: POOL_TERMS_VERSION },
    { publish_id: rp.publish_id, user_id: A.id, at: w.clock, terms_version: POOL_TERMS_VERSION },
  ]);
  // Never in a non-admin response; in the admin's item view and DMCA actions.
  for (const r of [await get(w, R, a.item_id), await get(w, A, a.item_id), await w.json<any>("GET", "/items/mine", { user: A })]) assert.ok(!/terms_version|attested_by|attestations/.test(JSON.stringify(r.body)));
  const av = await w.admin<any>(adm, "GET", "/admin/pool/item", null, `?id=${a.item_id}`);
  assert.ok(JSON.stringify(av.body).includes(`"terms_version":"${POOL_TERMS_VERSION}"`), JSON.stringify(av.body).slice(0, 300));
  await w.takedown<any>(adm, await notice(w, adm), a.item_id);
  const acts = await w.admin<any>(adm, "GET", "/admin/dmca/actions", null);
  assert.ok(JSON.stringify(acts.body).includes(POOL_TERMS_VERSION));
  // Kept with the ghost after a purge; the user id stays (pseudonymous) after erasure.
  await erase(w, A);
  await days(w, 40);
  assert.ok(w.q(`SELECT 1 FROM attestations WHERE user_id = ?`, A.id).length >= 1);
});

test("30a B3 (differential seed 92): a later copy of the author's own question is never credited as an earlier publication of her new version", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), Bea = w.user("teacher", "Bea");
  let a: any = (await publish(w, A, bankOf([C(0, 0)]), { license: "CC-BY-SA-4.0" })).body;
  w.advance(DAY);
  await get(w, Bea, a.item_id);
  assert.equal((await publish(w, Bea, bankOf([C(0, 0)]), { license: "CC-BY-SA-4.0" })).status, 201);
  for (const [sv, e] of [[1, 0], [1, 1]]) {
    w.advance(DAY);
    a = { ...(await publish(w, A, bankOf(own(a, [C(sv!, e!)])), { license: "CC-BY-SA-4.0", base: a.publish_id }, a.item_id)).body, item_id: a.item_id };
  }
  const p = await preview(w, A, bankOf([C(1, 1)]), "CC0-1.0");
  assert.equal(p.status, 200);
  assert.ok(!JSON.stringify(p.body.chains).includes("Bea"), JSON.stringify(p.body.chains));
});

test("30a B3: an independent near-identical question published AFTER her original but before her edit is never credited as earlier than her (her own earlier version counts, with or without its explanation, and after a purge)", async () => {
  const Z = (_sv: number) => mcq(STEMS[0]!.replace("river?", "river exactly?"), [["A delta lobe of fresh sediment", true], ["A mesa of old rock", false], ["A butte", false]]);
  for (const purge of [false, true]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), Zed = w.user("teacher", "Zed"), E = w.user("teacher", "Eve");
    let a: any = (await publish(w, A, bankOf([C(0, 1)]), { license: "CC-BY-NC-SA-4.0" })).body;
    w.advance(DAY);
    assert.equal((await publish(w, Zed, bankOf([Z(0)]), { license: "CC-BY-NC-SA-4.0" })).status, 201);
    w.advance(DAY);
    if (!purge) {
      a = { ...(await publish(w, A, bankOf(own(a, [C(1, 0)])), { license: "CC-BY-NC-SA-4.0", base: a.publish_id }, a.item_id)).body, item_id: a.item_id };
      assert.deepEqual((await get(w, E, a.item_id)).body.questions[0].chain, [], "no credit to Zed");
      assert.equal(code(await preview(w, A, bankOf(own(a, [C(1, 0)])), "CC0-1.0", { base_publish_id: a.publish_id }, a.item_id)), "200");
    } else {
      await get(w, E, a.item_id);
      await w.json("DELETE", `/items/${a.item_id}`, { user: A });
      await days(w, 16);
      const n = await publish(w, A, bankOf([C(1, 0)]), { license: "CC-BY-NC-SA-4.0" });
      assert.equal(n.status, 201);
      assert.deepEqual((await get(w, E, n.body.item_id as string)).body.questions[0].chain, [], "no credit to Zed after her purge");
    }
  }
});
