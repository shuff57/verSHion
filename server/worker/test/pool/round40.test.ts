// Round 40: circles phase B after critic 47a (product; critic 47b found no blocker),
// docs/critic-reports/2026-10-24-circles-phaseB-product-critic47a.md.
// B1: a public question's credit no longer depends on where a copy sits: the bank's AUTHOR's copies (exact or
// adapted) all keep it; a non-author's copy keeps it only as an exact copy, whichever occurrence comes first.
// Small items: the owner can repair, trim, roll back or purge a bank held only for student data (its author gone or
// not); the admin mail says to release the VERSION; a rollback to a version from before 0110 lists that version's own
// bank name; the listing refresh reads the history once.
import assert from "node:assert/strict";
import { test } from "node:test";
import { poolNotifier } from "../../src/poolmail.ts";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { framed, publish, publishId, World, type User } from "./harness.ts";

type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "local", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const STUDENT = "Jenny Smith in period 3 scored 42 on the unit test: what is the class median?";

async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][]): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name: "Algebra team" } })).body.circle.id;
  for (const [u, role] of members) {
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } })).body.code;
    assert.equal((await w.json("POST", "/circles/join", { user: u, json: { code: k } })).status, 200);
    w.advance(1000);
  }
  return cid;
}
type Item = string | Q;
const toQ = (x: Item, bid?: string): Q => {
  if (typeof x !== "string") return x;
  if (!x.startsWith("qid:")) return q(x);
  const [, qid, ...rest] = x.split(":");
  return q(rest.join(":"), { published: { item_id: bid, qid, publish_id: "p", version: 1 } });
};
async function send(w: World, u: User, cid: string, items: Item[], o: { bid?: string; base?: number; license?: string; title?: string; name?: string; confirm?: boolean } = {}) {
  const meta: Q = {
    commit_id: publishId(),
    license: o.license ?? "circle-only",
    attest: { own_work: true, terms_version: POOL_TERMS_VERSION },
    meta: { title: o.title ?? "Unit 2", subject: "Math", course: "", grade: "", tags: [], standards: [] },
    ...(o.bid ? { base_version: o.base } : {}),
    ...(o.confirm ? { confirm_large_change: true } : {}),
  };
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, bankOf(items.map((x) => toQ(x, o.bid)), o.name)), contentType: PUB });
}
const keep = (qids: string[], stems: string[]) => stems.map((s, i) => (i < qids.length ? `qid:${qids[i]}:${s}` : s));
const rollback = (w: World, u: User, cid: string, bid: string, body: Q) => w.json<any>("POST", `/circles/${cid}/banks/${bid}/rollback`, { user: u, json: { commit_id: publishId(), ...body } });

// ---- B1: the credit does not depend on where a copy sits ---------------------------------------------------------------

const OWN = "Ann's own question about moraines and eskers?";
const OWN3 = "Ann's own third question about kames?";
const T = "Dee's public question about glacial erratics and the Laurentide ice sheet?";
const TB = "Dee's public question about glacial erratics and the Laurentide ice sheet, version B with other numbers?";
async function originWorld() {
  const w = new World();
  const [A, E, D, V] = ["Ann", "Eli", "Dee", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"], [V!, "viewer"]]);
  const pub = (await publish(w, D!, bankOf([q(T)]), { license: "CC-BY-SA-4.0" })).body as Q;
  const s1 = await send(w, A!, cid, [q(OWN), { ...q(T), origin: { item_id: pub.item_id, qid: "x", publish_id: pub.publish_id, version: 1 } }, q(OWN3)], { license: "CC-BY-SA-4.0" });
  assert.equal(s1.status, 201, JSON.stringify(s1.body));
  const bid = s1.body.bank_id as string;
  const [q0, q1, q2] = s1.body.qids as string[];
  const origins = async () => (await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: V! })).body.origins.map((o: Q) => o.pos);
  const relicense = async () => (await w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: A!, json: { license: "circle-only" } })).status;
  return { w, A: A!, E: E!, cid, bid, q0, q1, q2, origins, relicense };
}

test("40-1 (47a B1): the bank's author duplicates a credited question and adapts the copy: both keep the credit whichever comes first, and dropping the original keeps the adaptation's credit (the licence stays bound)", async () => {
  for (const order of ["copy below", "copy above"] as const) {
    const { w, A, cid, bid, q0, q1, q2, origins, relicense } = await originWorld();
    const items = order === "copy below" ? [`qid:${q1}:${T}`, `qid:${q1}:${TB}`] : [`qid:${q1}:${TB}`, `qid:${q1}:${T}`];
    const s2 = await send(w, A, cid, [`qid:${q0}:${OWN}`, ...items, `qid:${q2}:${OWN3}`], { bid, base: 1, license: "CC-BY-SA-4.0" });
    assert.deepEqual([s2.status, s2.body.removed], [200, 0], `${order}: ${JSON.stringify(s2.body)}`);
    assert.deepEqual(await origins(), [1, 2], `${order}: both credited`);
    // The exact original keeps the qid wherever it sits.
    assert.equal(s2.body.qids[order === "copy below" ? 1 : 2], q1, `${order}: the unchanged one keeps the qid`);
    const adapted = s2.body.qids[order === "copy below" ? 2 : 1];
    const s3 = await send(w, A, cid, [`qid:${q0}:${OWN}`, `qid:${adapted}:${TB}`, `qid:${q2}:${OWN3}`], { bid, base: 2, license: "CC-BY-SA-4.0" });
    assert.equal(s3.status, 200);
    assert.deepEqual(await origins(), [1], `${order}: the adaptation keeps its credit`);
    assert.equal(await relicense(), 422, `${order}: still bound`);
  }
  // Editing in place (one occurrence, the original dropped in the same send) keeps it too.
  const { w, A, cid, bid, q0, q1, q2, origins, relicense } = await originWorld();
  assert.equal((await send(w, A, cid, [`qid:${q0}:${OWN}`, `qid:${q1}:${TB}`, `qid:${q2}:${OWN3}`], { bid, base: 1, license: "CC-BY-SA-4.0" })).status, 200);
  assert.deepEqual([await origins(), await relicense()], [[1], 422]);
});

test("40-2 (47a B1, round 39 rule kept): an EDITOR's copies carry the credit only as exact copies, whichever comes first; an editor's adapted copy is a new, uncredited question in both orders", async () => {
  for (const order of ["copy below", "copy above"] as const) {
    const { w, A, E, cid, bid, q0, q1, q2, origins, relicense } = await originWorld();
    const items = order === "copy below" ? [`qid:${q1}:${T}`, `qid:${q1}:${TB}`] : [`qid:${q1}:${TB}`, `qid:${q1}:${T}`];
    const s2 = await send(w, E, cid, [`qid:${q0}:${OWN}`, ...items, `qid:${q2}:${OWN3}`], { bid, base: 1, license: "CC-BY-SA-4.0" });
    assert.deepEqual([s2.status, s2.body.added, s2.body.removed], [200, 1, 0], `${order}: ${JSON.stringify(s2.body)}`);
    const exactAt = order === "copy below" ? 1 : 2;
    assert.deepEqual(await origins(), [exactAt], `${order}: only the exact original`);
    assert.equal(s2.body.qids[exactAt], q1);
    // Ann drops the original: the editor's adaptation was never credited, so the bank is free of it.
    const adapted = s2.body.qids[order === "copy below" ? 2 : 1];
    assert.equal((await send(w, A, cid, [`qid:${q0}:${OWN}`, `qid:${adapted}:${TB}`, `qid:${q2}:${OWN3}`], { bid, base: 2, license: "CC-BY-SA-4.0" })).status, 200);
    assert.deepEqual([await origins(), await relicense()], [[], 200], order);
  }
});

// ---- (b) the owner's way out of a bank held for student data ------------------------------------------------------------

async function heldBank() {
  const w = new World();
  const adm = w.user("admin");
  const [O, A, E, V, P] = ["Olga", "Ann", "Eli", "Vic", "Pub"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"], [V!, "viewer"]]);
  const it = (await publish(w, P!, bankOf([q(STUDENT)]))).body as Q;
  assert.equal((await w.purge(adm, it.item_id, { questions: [0] })).status, 200);
  const s = await send(w, A!, cid, ["Clean question about eskers and drumlins?"], { title: "Unit 2" });
  const bid = s.body.bank_id;
  const d = await send(w, E!, cid, [...keep(s.body.qids, ["Clean question about eskers and drumlins?"]), STUDENT], { bid, base: 1 });
  assert.deepEqual([d.body.held, d.body.withheld], [true, true]);
  return { w, adm, O: O!, A: A!, E: E!, V: V!, cid, bid, qids: s.body.qids as string[] };
}

test("40-3 (47a V1): a bank held only for student data (its author gone or not): the owner finds it, rolls it back, sends a clean version or trims it (never reading the withheld version), and can remove and purge it (round 41: once the admin has judged its flag); a report's hold still stops all of that", async () => {
  {
    const { w, O, A, V, cid, bid } = await heldBank();
    assert.equal((await w.json("POST", `/circles/${cid}/leave`, { user: A })).status, 200);
    const page = (await w.json<any>("GET", `/circles/${cid}`, { user: O })).body.banks;
    assert.deepEqual(page.map((b: Q) => [b.id, b.title, b.held]), [[bid, "Unit 2", true]], "listed to the owner, with its clean title");
    assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: O })).body.error, "version_withheld");
    assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/1`, { user: O })).status, 200);
    assert.equal((await w.json("GET", `/circles/${cid}/banks/${bid}`, { user: V })).status, 404, "still hidden from other members");
    const rb = await rollback(w, O, cid, bid, { to_version: 1, base_version: 2 });
    assert.deepEqual([rb.status, rb.body.held], [200, false], JSON.stringify(rb.body));
    assert.equal((await w.json("GET", `/circles/${cid}/banks/${bid}`, { user: V })).status, 200);
  }
  {
    const { w, O, A, cid, bid, qids } = await heldBank();
    assert.equal((await w.json("POST", `/circles/${cid}/leave`, { user: A })).status, 200);
    const fix = await send(w, O, cid, keep(qids, ["Clean question about eskers and drumlins, fixed?"]), { bid, base: 2 });
    assert.deepEqual([fix.status, fix.body.held], [200, false], JSON.stringify(fix.body));
  }
  {
    const { w, O, cid, bid } = await heldBank();
    assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${bid}/trim`, { user: O, json: { keep: 1 } })).status, 200);
  }
  {
    // Its author suspended: the bank is under review for everyone, but the owner removes it and purges it as soon as the
    // admin has judged the student-data flag (round 41, critic 48 V1: 409 pending_review before that).
    const { w, adm, O, A, cid, bid } = await heldBank();
    w.sql.prepare(`UPDATE users SET suspended_at = ? WHERE id = ?`).run(w.clock, A.id);
    assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: O })).status, 200);
    // (round 48, critics 55a V5 / 55b V10: the purge-now that waits for the review is answered as the work it does: 200
    // purged_partial, pending_review)
    assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${bid}/purge`, { user: O })).body.pending_review, true);
    const flag = w.q<any>(`SELECT id FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ? AND resolved_at IS NULL`, bid)[0].id;
    assert.equal((await w.admin(adm, "POST", "/admin/pool/resolve-flag", { flag_id: flag })).status, 200);
    const pg = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/purge`, { user: O });
    assert.equal(pg.status, 200, JSON.stringify(pg.body));
    assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ?`, bid).length, 0, "the withheld text is gone");
  }
  {
    // A report's (or an admin's) hold keeps the bank hidden from the owner and keeps its evidence from a purge.
    const { w, O, A, cid, bid } = await heldBank();
    w.sql.prepare(`INSERT INTO circle_holds (bank_id, reason, source, at) VALUES (?, 'admin', 'manual', ?)`).run(bid, w.clock);
    assert.equal((await rollback(w, O, cid, bid, { to_version: 1, base_version: 2 })).status, 404);
    assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: O })).status, 200);
    assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${bid}/purge`, { user: O })).body.error, "under_moderation");
    void A;
  }
});

// ---- (a) the admin mail names the version ------------------------------------------------------------------------------

test("40-4 (47a V2): the admin's student-data mail for a circle names the version and says to release THAT version (a bank-wide release leaves it withheld)", async () => {
  const { w, cid, bid } = await heldBank();
  const n = w.notices.find((x) => x.kind === "privacy_text_match") as Q;
  assert.deepEqual([n.itemId, n.circleId, n.version], [bid, cid, 2]);
  const sent: string[] = [];
  const env = { DB: { prepare: () => ({ bind: () => ({ first: async () => null, all: async () => ({ results: [] }) }) }) } } as never;
  const orig = globalThis.fetch;
  globalThis.fetch = (async (_u: unknown, init?: { body?: string }) => {
    sent.push(String(init?.body ?? ""));
    return new Response("{}", { status: 200 });
  }) as typeof fetch;
  try {
    await poolNotifier({ ...(env as object), RESEND_API_KEY: "k", MAIL_FROM: "a@x.org", ADMIN_EMAIL: "admin@x.org", MAIL_MODE: "send" } as never)(n as never);
  } finally {
    globalThis.fetch = orig;
  }
  assert.equal(sent.length, 1);
  const text = JSON.parse(sent[0]!).text as string;
  assert.match(text, /version 2/i);
  assert.match(text, /"version": ?2/);
  assert.match(text, /bank-wide release/i);
  assert.ok(!/Jenny/.test(text));
});

// ---- (d) a rollback to a version from before 0110 ----------------------------------------------------------------------

test("40-5 (47a V3): a rollback to a version sent before migration 0110 (no listing recorded) lists that version's own bank name, not the head's title", async () => {
  const w = new World();
  const [A] = ["Ann"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, []);
  let r = await send(w, A!, cid, ["Q one about moraines?"], { title: "Unit A", name: "Unit A" });
  const bid = r.body.bank_id;
  r = await send(w, A!, cid, keep(r.body.qids, ["Q one about moraines, v2?"]), { bid, base: 1, title: "Unit B", name: "Unit B" });
  r = await send(w, A!, cid, keep(r.body.qids, ["Q one about moraines, v3?"]), { bid, base: 2, title: "Unit C", name: "Unit C" });
  // (as 0110 leaves a populated database: listing only on each bank's current version)
  w.sql.prepare(`UPDATE circle_versions SET listing = NULL WHERE bank_id = ? AND status = 'previous'`).run(bid);
  assert.equal((await rollback(w, A!, cid, bid, { to_version: 1, base_version: 3 })).status, 200);
  assert.equal(w.q<any>(`SELECT title FROM circle_banks WHERE id = ?`, bid)[0].title, "Unit A");
  assert.equal((await rollback(w, A!, cid, bid, { to_version: 2, base_version: 4 })).status, 200);
  assert.equal(w.q<any>(`SELECT title FROM circle_banks WHERE id = ?`, bid)[0].title, "Unit B");
});

// ---- (e) the listing refresh reads the history once --------------------------------------------------------------------

test("40-6 (47b V2): the bank's listing refresh finds its newest readable version with ONE search of the history (not one per field)", async () => {
  const { w, A, cid, bid, qids } = await heldBank();
  const caps: { sql: string; args: unknown[] }[] = [];
  w.stats.log = (sql, args) => {
    if (/lv\.listing/.test(sql)) caps.push({ sql, args: [...args] });
  };
  assert.equal((await send(w, A, cid, keep(qids, ["Clean question about eskers and drumlins, fixed?"]), { bid, base: 2 })).status, 200);
  w.stats.log = undefined;
  assert.equal(caps.length, 1);
  const plan = (w.sql.prepare(`EXPLAIN QUERY PLAN ${caps[0]!.sql}`).all(...(caps[0]!.args as never[])) as { detail: string }[]).map((x) => x.detail);
  assert.equal(plan.filter((d) => /^SEARCH lv /.test(d)).length, 1, plan.join(" | "));
  assert.ok(plan.some((d) => /SEARCH lv USING INDEX sqlite_autoindex_circle_versions_\d \(bank_id=\?( AND version>\?)?\)/.test(d)), plan.join(" | "));
  assert.equal(w.q<any>(`SELECT title FROM circle_banks WHERE id = ?`, bid)[0].title, "Unit 2");
});
