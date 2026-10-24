// Round 39: circles phase B after critics 46a (product) and 46b (security),
// docs/critic-reports/2026-10-24-circles-phaseB-{product-critic46a,security-critic46b}.md.
// M1: a student-data (privacy) hold no longer freezes pruning or trim (the withheld version is what protects the
// data), so a held bank at the byte cap can be repaired; N1: a bank's listing fields (title, subject, course, grade)
// are kept per version and never come from a withheld one; N2: a lost commit records its R2 keys as orphans before
// deleting them. Variants: a copy only carries a credit when it is an exact copy, and a non-author's origin on an
// existing question is ignored; trim keeps a withheld version while its flag is open; oldest_kept counts readable
// versions only; a stale send is answered before the heavy slot; the licence change is a compare-and-set on the head.
import assert from "node:assert/strict";
import { test } from "node:test";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { framed, publish, publishId, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "local", name: "b", questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
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
async function send(w: World, u: User, cid: string, items: Item[], o: { bid?: string; base?: number; license?: string; title?: string; subject?: string; confirm?: boolean; commitId?: string } = {}) {
  const meta: Q = {
    commit_id: o.commitId ?? publishId(),
    license: o.license ?? "circle-only",
    attest: { own_work: true, terms_version: POOL_TERMS_VERSION },
    meta: { title: o.title ?? "Unit 2", subject: o.subject ?? "Math", course: "", grade: "", tags: [], standards: [] },
    ...(o.bid ? { base_version: o.base } : {}),
    ...(o.confirm ? { confirm_large_change: true } : {}),
  };
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, bankOf(items.map((x) => toQ(x, o.bid)))), contentType: PUB });
}
const keep = (qids: string[], stems: string[]) => stems.map((s, i) => (i < qids.length ? `qid:${qids[i]}:${s}` : s));
const history = async (w: World, u: User, cid: string, bid: string) => w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions`, { user: u });
const rollback = (w: World, u: User, cid: string, bid: string, body: Q) => w.json<any>("POST", `/circles/${cid}/banks/${bid}/rollback`, { user: u, json: { commit_id: publishId(), ...body } });
const trim = (w: World, u: User, cid: string, bid: string, keepN: number) => w.json<any>("POST", `/circles/${cid}/banks/${bid}/trim`, { user: u, json: { keep: keepN } });
const lim = (w: World, metric: string, value: number) => w.sql.prepare(`INSERT OR REPLACE INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', ?, ?, 0, 'adm')`).run(metric, value);
const used = (w: World, cid: string) => w.q<any>(`SELECT SUM(bytes) AS n FROM circle_versions WHERE circle_id = ? AND status IN ('current', 'previous')`, cid)[0].n as number;
/** Same-length stems, so every version of the bank costs the same bytes. */
const stem = (v: number) => `Version ${String(v).padStart(3, "0")}: what does the esker at site ${String(v).padStart(3, "0")} tell us?`;

async function blockedWorld() {
  const w = new World();
  const adm = w.user("admin");
  const P = w.user("teacher", "Pub");
  const it = (await publish(w, P, bankOf([q(STUDENT)]))).body as Q;
  assert.equal((await w.purge(adm, it.item_id, { questions: [0] })).status, 200);
  return { w, adm };
}

// ---- M1: a held bank at the cap can be repaired ---------------------------------------------------------------------

/** Ann's bank with 22 versions (20 kept), Eli's student-data version 23 (the bank held, v23 withheld), and the circle's
 *  byte cap set to what is used now plus 100 bytes. */
async function heldAtCap() {
  const { w, adm } = await blockedWorld();
  const [O, A, E] = ["Ola", "Ann", "Eli"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"]]);
  let r = await send(w, A!, cid, [stem(1)]);
  const bid = r.body.bank_id;
  for (let v = 2; v <= 22; v++) r = await send(w, A!, cid, keep(r.body.qids, [stem(v)]), { bid, base: v - 1 });
  const d = await send(w, E!, cid, [...keep(r.body.qids, [stem(22)]), STUDENT], { bid, base: 22 });
  assert.deepEqual([d.status, d.body.held, d.body.withheld], [200, true, true]);
  const cap = used(w, cid) + 100;
  lim(w, "circle_bytes", cap);
  return { w, adm, O: O!, A: A!, E: E!, cid, bid, cap, qids: r.body.qids as string[] };
}

test("39-1 (46a M1): a bank held for student data at the byte cap: its author's clean version, a rollback and a trim all go through (a privacy hold no longer freezes pruning, and the cap counts the same prune); a report's hold still does", async () => {
  {
    const { w, A, cid, bid, qids, cap } = await heldAtCap();
    const fix = await send(w, A, cid, keep(qids, [stem(24)]), { bid, base: 23 });
    assert.deepEqual([fix.status, fix.body.held], [200, false], JSON.stringify(fix.body));
    assert.ok(used(w, cid) <= cap, "the cap holds after the prune");
  }
  {
    const { w, A, cid, bid } = await heldAtCap();
    const rb = await rollback(w, A, cid, bid, { to_version: 22, base_version: 23 });
    assert.deepEqual([rb.status, rb.body.held], [200, false], JSON.stringify(rb.body));
  }
  {
    const { w, A, cid, bid } = await heldAtCap();
    const t = await trim(w, A, cid, bid, 3);
    assert.deepEqual([t.status, t.body.pruned], [200, 17], JSON.stringify(t.body));
    assert.equal(w.q<any>(`SELECT status FROM circle_versions WHERE bank_id = ? AND version = 23`, bid)[0].status, "current", "the withheld head stays");
  }
  {
    // A report's hold (evidence for the admin) still freezes pruning and trim.
    const { w, A, E, cid, bid, qids } = await heldAtCap();
    lim(w, "circle_bytes", 200 * 1024 * 1024);
    assert.equal((await send(w, A, cid, keep(qids, [stem(24)]), { bid, base: 23 })).status, 200);
    w.sql.prepare(`INSERT INTO circle_holds (bank_id, reason, source, at) VALUES (?, 'reports', '', ?)`).run(bid, w.clock);
    assert.equal((await trim(w, A, cid, bid, 3)).body.error, "under_moderation");
    const before = w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ? AND status = 'pruned'`, bid).length;
    assert.equal((await send(w, A, cid, keep(qids, [stem(25)]), { bid, base: 24 })).status, 200);
    assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ? AND status = 'pruned'`, bid).length, before, "nothing pruned while a report holds it");
    void E;
  }
});

// ---- N1: listing fields per version --------------------------------------------------------------------------------

test("39-2 (46b N1): a withheld version's title and subject never reach a member: after the author's rollback, a clean version, or an admin's bank-wide release, the circle page, the head read, the update check and the activity list show the last clean version's", async () => {
  const { w, adm } = await blockedWorld();
  const [A, E, V] = ["Ann", "Eli", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"], [V!, "viewer"]]);
  const s = await send(w, A!, cid, ["A clean question about deltas and fans?"], { title: "Unit 2", subject: "Math" });
  const bid = s.body.bank_id;
  const dirty = await send(w, E!, cid, [...keep(s.body.qids, ["A clean question about deltas and fans?"]), STUDENT], { bid, base: 1, title: "Jenny Smith P3 scores", subject: "Jenny's class" });
  assert.equal(dirty.body.withheld, true);
  const seen = async () => {
    const out = [
      JSON.stringify((await w.json("GET", `/circles/${cid}`, { user: V! })).body),
      JSON.stringify((await w.json("GET", `/circles/${cid}/activity`, { user: V! })).body),
      JSON.stringify((await w.json("POST", "/circles/heads", { user: V!, json: { bank_ids: [bid] } })).body),
      JSON.stringify((await w.json("GET", `/circles`, { user: V! })).body),
    ];
    const head = await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: V! });
    out.push(JSON.stringify({ ...head.body, payload: undefined }));
    return { text: out.join("\n"), head };
  };
  // An admin's bank-wide release: the bank is listed again, with version 1's listing (version 2 stays withheld).
  assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, note: "Bank checked by hand." })).status, 200);
  let v = await seen();
  assert.ok(!/Jenny/.test(v.text), v.text.slice(0, 400));
  assert.match(v.text, /"title":"Unit 2"/);
  // The author's rollback to version 1.
  const rb = await rollback(w, A!, cid, bid, { to_version: 1, base_version: 2 });
  assert.equal(rb.status, 200);
  v = await seen();
  assert.ok(!/Jenny/.test(v.text), v.text.slice(0, 400));
  assert.deepEqual([v.head.status, v.head.body.bank.title, v.head.body.bank.subject], [200, "Unit 2", "Math"]);
  // A later clean version sets its own listing; another withheld one does not.
  assert.equal((await send(w, A!, cid, keep(rb.body.qids, ["A clean question about deltas and fans?"]), { bid, base: 3, title: "Unit 3", subject: "Math" })).status, 200);
  assert.equal((await send(w, E!, cid, [...keep(rb.body.qids, ["A clean question about deltas and fans?"]), STUDENT], { bid, base: 4, title: "Jenny again" })).body.withheld, true);
  assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, note: "Bank checked again." })).status, 200);
  v = await seen();
  assert.ok(!/Jenny/.test(v.text), v.text.slice(0, 400));
  assert.match(v.text, /"title":"Unit 3"/);
  // A bank whose only version is withheld lists no title at all once visible.
  const nb = await send(w, E!, cid, [STUDENT], { title: "Jenny Smith's bank" });
  assert.equal(nb.body.withheld, true);
  assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: nb.body.bank_id, note: "Bank checked by hand." })).status, 200);
  assert.ok(!/Jenny/.test((await seen()).text));
  // An admin's release of the version brings its own listing back (it is the newest readable one then).
  assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, version: 5, note: "A false positive, checked." })).status, 200);
  assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: V! })).body.bank.title, "Jenny again");
});

// ---- N2: lost commits record their R2 orphans -------------------------------------------------------------------------

test("39-3 (46b N2): a lost commit (or an abandoned attempt a retry discards) whose R2 delete fails records its keys in r2_orphans, and the hourly retry deletes them", async () => {
  const w = new World();
  const [A, E] = ["Ann", "Eli"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"]]);
  const s = await send(w, A!, cid, ["A question about kames?"]);
  const bid = s.body.bank_id;
  const r2 = w.r2 as any;
  const put = r2.put.bind(r2), del = r2.delete.bind(r2);
  // Eli's send loses its compare-and-set (the head moves while its objects are put) while R2 deletes fail.
  r2.put = async (k: string, ...rest: unknown[]) => {
    const out = await put(k, ...rest);
    if (k.endsWith(".idx.json")) w.sql.prepare(`UPDATE circle_banks SET head_version = head_version + 1 WHERE id = ?`).run(bid);
    return out;
  };
  r2.delete = async () => {
    throw new Error("R2 transient");
  };
  const e = await send(w, E!, cid, keep(s.body.qids, ["Eli's question about kames?"]), { bid, base: 1 });
  r2.put = put;
  assert.equal(e.status, 409);
  const kept = new Set(w.q<any>(`SELECT full_key, index_key FROM circle_versions`).flatMap((r) => [r.full_key, r.index_key]));
  const stray = w.r2.keys("circles/").filter((k) => !kept.has(k));
  assert.equal(stray.length, 2);
  assert.deepEqual(w.q<any>(`SELECT key FROM r2_orphans ORDER BY key`).map((r) => r.key), [...stray].sort());
  // An abandoned attempt (its index put failed), retried with the same commit_id after the in-flight window.
  w.sql.prepare(`UPDATE circle_banks SET head_version = 1 WHERE id = ?`).run(bid);
  const commit = publishId();
  r2.put = async (k: string, ...rest: unknown[]) => {
    if (k.endsWith(".idx.json")) throw new Error("R2 put failed");
    return put(k, ...rest);
  };
  assert.equal((await send(w, E!, cid, keep(s.body.qids, ["Eli's second question about kames?"]), { bid, base: 1, commitId: commit })).status, 500);
  r2.put = put;
  const staging = w.q<any>(`SELECT full_key FROM circle_versions WHERE status = 'staging'`);
  assert.equal(staging.length, 1);
  w.advance(10 * 60_000);
  const retry = await send(w, E!, cid, keep(s.body.qids, ["Eli's second question about kames?"]), { bid, base: 1, commitId: commit });
  assert.equal(retry.status, 200, JSON.stringify(retry.body));
  assert.ok(w.q<any>(`SELECT key FROM r2_orphans`).some((r) => r.key === staging[0].full_key), "the abandoned body is recorded");
  // The hourly retry deletes them once R2 answers again.
  r2.delete = del;
  w.advance(2 * 3_600_000);
  await w.pool.hourly();
  assert.equal(w.q(`SELECT 1 FROM r2_orphans`).length, 0);
  const live = new Set(w.q<any>(`SELECT full_key, index_key FROM circle_versions`).flatMap((r) => [r.full_key, r.index_key]));
  assert.deepEqual(w.r2.keys("circles/").filter((k) => !live.has(k)), []);
});

// ---- variant V1: a third party cannot graft or impose a credit ---------------------------------------------------------

test("39-4 (46b V1): an editor cannot put a public item's credit (and its licence bound) on text that is not that item's: a changed copy of a credited question gets no credit, and an origin an editor declares on the author's existing question is ignored; on a new question it is recorded", async () => {
  const w = new World();
  const [A, E, D, Z, V] = ["Ann", "Eli", "Dee", "Zed", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"], [V!, "viewer"]]);
  const T = "Dee's public question about glacial erratics?";
  const pub = (await publish(w, D!, bankOf([q(T)]), { license: "CC-BY-SA-4.0" })).body as Q;
  const s1 = await send(w, A!, cid, [q("Ann's own question about moraines?"), { ...q(T), origin: { item_id: pub.item_id, qid: "x", publish_id: pub.publish_id, version: 1 } }], { license: "CC-BY-SA-4.0" });
  const bid = s1.body.bank_id;
  const [own, credited] = s1.body.qids as string[];
  // (a) The credited question's qid twice: an exact copy keeps the credit, a changed one does not.
  const g = await send(w, E!, cid, [`qid:${own}:Ann's own question about moraines?`, `qid:${credited}:${T}`, `qid:${credited}:${T}`, `qid:${credited}:Eli's own question about kames and eskers?`], { bid, base: 1, license: "CC-BY-SA-4.0" });
  assert.equal(g.status, 200, JSON.stringify(g.body));
  const origins = async () => (await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: V! })).body.origins.map((o: Q) => o.pos);
  assert.deepEqual(await origins(), [1, 2]);
  // Ann drops every copy of the credited question: the bank is free of it, though Eli's changed copy stays.
  const s3 = await send(w, A!, cid, [`qid:${own}:Ann's own question about moraines?`, `qid:${g.body.qids[3]}:Eli's own question about kames and eskers?`], { bid, base: 2, license: "CC-BY-SA-4.0", confirm: true });
  assert.equal(s3.status, 200);
  assert.deepEqual(await origins(), []);
  // (b) Eli declares Zed's public item as the origin of Ann's existing question: ignored (no credit, no bound).
  const pz = (await publish(w, Z!, bankOf([q("Zed's unrelated public question?")]), { license: "CC-BY-SA-4.0" })).body as Q;
  const imp = await send(w, E!, cid, [{ ...q("Ann's own question about moraines?", { published: { item_id: bid, qid: own, publish_id: "p", version: 1 } }), origin: { item_id: pz.item_id, qid: "zz", publish_id: pz.publish_id, version: 1 } }, `qid:${g.body.qids[3]}:Eli's own question about kames and eskers?`], { bid, base: 3, license: "CC-BY-SA-4.0" });
  assert.equal(imp.status, 200);
  assert.deepEqual(await origins(), []);
  assert.equal((await w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: A!, json: { license: "circle-only" } })).status, 200, "Ann's licence is hers to change");
  // ... while an editor importing a NEW public question records its origin (credit and bound).
  const add = await send(w, E!, cid, [`qid:${own}:Ann's own question about moraines?`, `qid:${g.body.qids[3]}:Eli's own question about kames and eskers?`, { ...q(T), origin: { item_id: pub.item_id, qid: "x", publish_id: pub.publish_id, version: 1 } }], { bid, base: 4, license: "circle-only" });
  assert.deepEqual([add.status, add.body.error], [422, "license_incompatible"], "the bound applies to the new question");
});

// ---- trim keeps evidence; oldest_kept counts readable versions -------------------------------------------------------------

test("39-5 (46a variant 3): a trim keeps a withheld version while its student-data flag is open (the admin's evidence); once the flag is resolved the next trim prunes it", async () => {
  const { w, adm } = await blockedWorld();
  const [A, E] = ["Ann", "Eli"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"]]);
  let r = await send(w, A!, cid, [stem(1)]);
  const bid = r.body.bank_id;
  const d = await send(w, E!, cid, [...keep(r.body.qids, [stem(1)]), STUDENT], { bid, base: 1 });
  assert.equal(d.body.withheld, true);
  for (let v = 3; v <= 6; v++) r = await send(w, A!, cid, keep(r.body.qids, [stem(v)]), { bid, base: v - 1, confirm: true });
  const t = await trim(w, A!, cid, bid, 1);
  assert.equal(t.status, 200);
  assert.equal(w.q<any>(`SELECT status FROM circle_versions WHERE bank_id = ? AND version = 2`, bid)[0].status, "previous");
  assert.equal((await w.admin(adm, "GET", "/admin/pool/circle-bank", null, `?id=${bid}&version=2&basis=privacy`)).status, 200, "the admin can still open it");
  const flag = w.q<any>(`SELECT id FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ? AND resolved_at IS NULL`, bid)[0].id;
  assert.equal((await w.admin(adm, "POST", "/admin/pool/resolve-flag", { flag_id: flag })).status, 200);
  assert.equal((await trim(w, A!, cid, bid, 1)).body.pruned, 1);
  assert.equal(w.q<any>(`SELECT status FROM circle_versions WHERE bank_id = ? AND version = 2`, bid)[0].status, "pruned");
  // (resolving the flag never makes the version readable: only a version release does)
  assert.equal(w.q<any>(`SELECT withheld_at FROM circle_versions WHERE bank_id = ? AND version = 2`, bid)[0].withheld_at !== null, true);
});

test("39-6 (46a variant 8): oldest_kept and pinned_versions count readable versions only: a withheld version breaks the run", async () => {
  const { w } = await blockedWorld();
  const [A, E] = ["Ann", "Eli"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"]]);
  let r = await send(w, A!, cid, [stem(1)]);
  const bid = r.body.bank_id;
  for (let v = 2; v <= 3; v++) r = await send(w, A!, cid, keep(r.body.qids, [stem(v)]), { bid, base: v - 1 });
  await send(w, E!, cid, [...keep(r.body.qids, [stem(3)]), STUDENT], { bid, base: 3 });
  r = await send(w, A!, cid, keep(r.body.qids, [stem(5)]), { bid, base: 4 });
  const h = await history(w, A!, cid, bid);
  assert.deepEqual([h.body.oldest_kept, h.body.pinned_versions], [5, [1, 2, 3]]);
  const mg = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/merge`, { user: E!, json: { base_version: 4 } });
  assert.deepEqual([mg.body.base_kept, mg.body.oldest_kept], [false, 5]);
});

// ---- variant V6: a stale send is answered before the heavy slot ---------------------------------------------------------

test("39-7 (46b V6): a send based on an old head (or a replay) is answered without the heavy slot: with the slot taken it is a 409 (or the replay), not a wait and a 503", async () => {
  const w = new World({ heavySlots: { active: 1, queue: [] }, heavyQueueMs: 50 });
  const free = new World({ sql: w.sql, r2: w.r2 });
  free.tokens = w.tokens;
  const [A, E] = ["Ann", "Eli"].map((n) => w.user("teacher", n));
  const cid = await circleWith(free, A!, [[E!, "editor"]]);
  const s = await send(free, A!, cid, ["A question about drumlins?"]);
  const bid = s.body.bank_id;
  const commit = publishId();
  const s2 = await send(free, A!, cid, keep(s.body.qids, ["A question about drumlins, edited?"]), { bid, base: 1, commitId: commit });
  assert.equal(s2.status, 200);
  w.clock = free.clock;
  const stale = await send(w, E!, cid, keep(s.body.qids, ["Eli's question about drumlins?"]), { bid, base: 1 });
  assert.deepEqual([stale.status, stale.body.error, stale.body.head_version], [409, "conflict", 2]);
  const replay = await send(w, A!, cid, keep(s.body.qids, ["A question about drumlins, edited?"]), { bid, base: 1, commitId: commit });
  assert.deepEqual([replay.status, replay.body.replayed], [200, true]);
  assert.equal((await send(w, E!, cid, keep(s2.body.qids, ["Eli's question about drumlins?"]), { bid, base: 2 })).body.error, "busy", "a current-base send still needs the slot");
});

// ---- variant V7: the licence change is a compare-and-set on the head ---------------------------------------------------

test("39-8 (46b V7): a licence change races an editor's version adding a public question: it is refused (409) rather than leaving a licence the head's origins do not allow", async () => {
  const w = new World();
  const [A, E, D] = ["Ann", "Eli", "Dee"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"]]);
  const T = "Dee's public question about glacial erratics?";
  const pub = (await publish(w, D!, bankOf([q(T)]), { license: "CC-BY-SA-4.0" })).body as Q;
  const s = await send(w, A!, cid, ["Ann's question about tarns?"], { license: "CC-BY-SA-4.0" });
  const bid = s.body.bank_id;
  const db = w.db as any;
  const batch = db.batch.bind(db);
  let once = true;
  db.batch = async (ss: unknown[]) => {
    if (once && (ss as { sql: string }[]).some((x) => /UPDATE circle_banks SET license/.test(x.sql))) {
      once = false;
      db.batch = batch;
      const r = await send(w, E!, cid, [`qid:${s.body.qids[0]}:Ann's question about tarns?`, { ...q(T), origin: { item_id: pub.item_id, qid: "x", publish_id: pub.publish_id, version: 1 } }], { bid, base: 1, license: "CC-BY-SA-4.0" });
      assert.equal(r.status, 200, JSON.stringify(r.body));
    }
    return batch(ss);
  };
  const p = await w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: A!, json: { license: "circle-only" } });
  db.batch = batch;
  assert.deepEqual([p.status, p.body.error], [409, "conflict"]);
  assert.equal(w.q<any>(`SELECT license FROM circle_banks WHERE id = ?`, bid)[0].license, "CC-BY-SA-4.0");
});

void DAY;
