// Round 38: circles phase B after critics 45a (product) and 45b (security),
// docs/critic-reports/2026-10-24-circles-phaseB-{product-critic45a,security-critic45b}.md.
// B1: confirmed student data is per VERSION (withheld from every member at every read until an admin releases that
// version); B2: the byte cap counts the versions the same commit prunes, and an owner can trim a bank's history;
// B3: a repeated published.qid is a copy (the first keeps its qid and origin). Variants: origins carried by the
// key-free match hash too; oldest_kept is the contiguous run (pinned versions listed); merge-initiate lists the first
// 50 after the base; a demoted author no longer changes the licence; the rollback reserves by value count as a send;
// circle privacy mails have their own budget.
import assert from "node:assert/strict";
import { test } from "node:test";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { bytesReserved } from "../../src/pool/shared.ts";
import { chunk, concat, framed, png, publish, publishId, rng, upload, World, type User } from "./harness.ts";
import { scanJson } from "../../src/pool/jsonscan.ts";
import { deflateSync } from "node:zlib";

const DAY = 86_400_000;
type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "local", name: "b", questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const stemsOf = (n: number, tag: string) => Array.from({ length: n }, (_, i) => `${tag} question ${i}: how does the delta's distributary channel ${i} shift over a decade?`);
const STUDENT = "Jenny Smith in period 3 scored 42 on the unit test: what is the class median?";
const CLEAN = "A clean question about deltas and fans?";

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
async function send(w: World, u: User, cid: string, items: Item[], o: { bid?: string; base?: number; license?: string; message?: string; confirm?: boolean } = {}) {
  const meta: Q = {
    commit_id: publishId(),
    license: o.license ?? "circle-only",
    attest: { own_work: true, terms_version: POOL_TERMS_VERSION },
    meta: { title: "Unit 2", subject: "Math", course: "", grade: "", tags: [], standards: [] },
    ...(o.message ? { message: o.message } : {}),
    ...(o.bid ? { base_version: o.base } : {}),
    ...(o.confirm ? { confirm_large_change: true } : {}),
  };
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, bankOf(items.map((x) => toQ(x, o.bid)))), contentType: PUB });
}
const keep = (qids: string[], stems: string[]) => stems.map((s, i) => (i < qids.length ? `qid:${qids[i]}:${s}` : s));
const history = async (w: World, u: User, cid: string, bid: string, before?: number) => w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions${before ? `?before=${before}` : ""}`, { user: u });
const rollback = (w: World, u: User, cid: string, bid: string, body: Q) => w.json<any>("POST", `/circles/${cid}/banks/${bid}/rollback`, { user: u, json: { commit_id: publishId(), ...body } });
const merge = (w: World, u: User, cid: string, bid: string, base: number) => w.json<any>("POST", `/circles/${cid}/banks/${bid}/merge`, { user: u, json: { base_version: base } });
const version = (w: World, u: User, cid: string, bid: string, n: number) => w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/${n}`, { user: u });
const lim = (w: World, metric: string, value: number) => w.sql.prepare(`INSERT OR REPLACE INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', ?, ?, 0, 'adm')`).run(metric, value);
/** A world where `STUDENT` is confirmed student data (an admin's privacy purge of a public item holding it). */
async function blocked() {
  const w = new World();
  const adm = w.user("admin");
  const P = w.user("teacher", "Pub");
  const it = (await publish(w, P, bankOf([q(STUDENT)]))).body as Q;
  assert.equal((await w.purge(adm, it.item_id, { questions: [0] })).status, 200);
  return { w, adm };
}
/** Every way a member could get version `n`'s text: the version read, a rollback to it, a merge from it, its history line. */
async function assertWithheld(w: World, cid: string, bid: string, n: number, users: User[], owner: User, label: string) {
  for (const u of users) {
    const r = await version(w, u, cid, bid, n);
    assert.deepEqual([r.status, r.body.error, r.body.payload], [404, "version_withheld", undefined], `${label}: ${u.name} reads v${n}`);
    const h = await history(w, u, cid, bid);
    const line = h.body.versions.find((x: Q) => x.version === n);
    assert.deepEqual([line.withheld, line.kept, line.message], [true, false, ""], `${label}: history line`);
  }
  const head = (await history(w, owner, cid, bid)).body.head_version;
  const rb = await rollback(w, owner, cid, bid, { to_version: n, base_version: head, confirm_large_change: true });
  assert.deepEqual([rb.status, rb.body.error], [404, "version_withheld"], `${label}: rollback target`);
  if (n < head) assert.equal((await merge(w, owner, cid, bid, n)).body.base_kept, false, `${label}: merge base`);
}

// ---- B1: student data is withheld per version ------------------------------------------------------------------

test("38-1 (B1): an editor's version holding confirmed student data stays withheld from EVERY member (the version read, its history line's message, a rollback to it, a merge from it, its images) after the author's clean version lifts the bank's hold, and after more versions; it is pruned in time like any version", async () => {
  const { w } = await blocked();
  const [A, E, V] = ["Ann", "Eli", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"], [V!, "viewer"]]);
  const s = await send(w, A!, cid, [CLEAN]);
  const bid = s.body.bank_id;
  // The image only the student-data version uses.
  const img = await upload(w, E!, png(8, 8, 99));
  assert.equal(img.status, 201, JSON.stringify(img.body));
  const dirty = await send(w, E!, cid, [`qid:${s.body.qids[0]}:${CLEAN}`, q(STUDENT, { image: { file: `media/${img.sha}.png`, width: 8, height: 8 } })], { bid, base: 1, message: "Eli's class list" });
  assert.deepEqual([dirty.status, dirty.body.held, dirty.body.withheld], [200, true, true], JSON.stringify(dirty.body));
  const clean = await send(w, A!, cid, [`qid:${s.body.qids[0]}:${CLEAN}`], { bid, base: 2 });
  assert.deepEqual([clean.status, clean.body.held, clean.body.withheld], [200, false, false]);
  assert.equal((await w.json("GET", `/circles/${cid}/banks/${bid}`, { user: V! })).status, 200, "the clean head is readable again");
  await assertWithheld(w, cid, bid, 2, [A!, E!, V!], A!, "after the clean send");
  assert.equal((await w.call("GET", `/circles/${cid}/banks/${bid}/blobs/${img.sha}`, { user: V! })).status, 404, "its image is withheld too");
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ? AND resolved_at IS NULL`, bid).length, 1, "the admin's flag stays open");
  // More clean versions never un-withhold it.
  let base = 3;
  for (let i = 0; i < 3; i++) base = (await send(w, E!, cid, [CLEAN, `More ${i}?`], { bid, base, confirm: true })).body.version;
  await assertWithheld(w, cid, bid, 2, [V!], A!, "three versions later");
  // ... and the history's oldest_kept/pinned do not count it as available.
  assert.equal((await version(w, V!, cid, bid, 1)).status, 200);
  // Pruned like any version once 20 newer ones exist (D16) and (round 46, critic 53b V7) its admin flag is resolved:
  // while the flag is open it is the admin's evidence and stays, withheld.
  for (let i = 0; i < 18; i++) base = (await send(w, A!, cid, [CLEAN, `Later ${i}?`], { bid, base, confirm: true })).body.version;
  assert.equal(base, 24);
  assert.deepEqual([(await version(w, V!, cid, bid, 2)).body.error, w.q<any>(`SELECT status FROM circle_versions WHERE bank_id = ? AND version = 2`, bid)[0].status], ["version_withheld", "previous"]);
  w.sql.prepare(`UPDATE admin_flags SET resolved_at = ? WHERE kind = 'privacy_text_match' AND item_id = ?`).run(w.clock, bid);
  base = (await send(w, A!, cid, [CLEAN, "After the decision?"], { bid, base, confirm: true })).body.version;
  assert.deepEqual([(await version(w, V!, cid, bid, 2)).body.error, w.q<any>(`SELECT status FROM circle_versions WHERE bank_id = ? AND version = 2`, bid)[0].status], ["version_not_retained", "pruned"]);
});

test("38-2 (B1): the author's own student-data version, then a rollback to the clean one: the rollback is readable, the dirty version is withheld; the dirty HEAD itself is withheld from everyone (its author fixes it by a rollback or a new version, never by reading it)", async () => {
  const { w } = await blocked();
  const [O, A, V] = ["Ola", "Ann", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [V!, "viewer"]]);
  const s = await send(w, A!, cid, [CLEAN, "Second clean question about levees?"]);
  const bid = s.body.bank_id;
  const dirty = await send(w, A!, cid, [...keep(s.body.qids, [CLEAN, "Second clean question about levees?"]), STUDENT], { bid, base: 1 });
  assert.deepEqual([dirty.body.held, dirty.body.withheld], [true, true]);
  // The head is withheld: its author gets version_withheld (not the text), members the held bank's 404.
  assert.deepEqual([(await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: A! })).body.error, (await version(w, A!, cid, bid, 2)).body.error], ["version_withheld", "version_withheld"]);
  assert.equal((await w.json("GET", `/circles/${cid}/banks/${bid}`, { user: V! })).status, 404);
  // The author rolls back to the clean version 1.
  const rb = await rollback(w, A!, cid, bid, { to_version: 1, base_version: 2 });
  assert.deepEqual([rb.status, rb.body.held, rb.body.withheld, rb.body.rollback_of], [200, false, false, 1], JSON.stringify(rb.body));
  assert.equal((await w.json("GET", `/circles/${cid}/banks/${bid}`, { user: V! })).status, 200);
  await assertWithheld(w, cid, bid, 2, [A!, V!, O!], O!, "after the rollback");
  // A rollback to a version holding it (or copying it) is never a way back to the text.
  assert.equal((await rollback(w, O!, cid, bid, { to_version: 2, base_version: 3 })).body.error, "version_withheld");
});

test("38-3 (B1): a near-rewording of confirmed student data is withheld the same way", async () => {
  const { w } = await blocked();
  const [A, E] = ["Ann", "Eli"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"]]);
  const s = await send(w, A!, cid, [CLEAN]);
  const bid = s.body.bank_id;
  const near = await send(w, E!, cid, [`qid:${s.body.qids[0]}:${CLEAN}`, STUDENT.replace("median", "middle value")], { bid, base: 1 });
  assert.deepEqual([near.body.held, near.body.withheld], [true, true], JSON.stringify(near.body));
  assert.deepEqual(w.q(`SELECT source FROM circle_holds WHERE bank_id = ?`, bid), [{ source: "privacy_near" }]);
  assert.equal((await send(w, A!, cid, [`qid:${s.body.qids[0]}:${CLEAN}`], { bid, base: 2 })).body.held, false);
  await assertWithheld(w, cid, bid, 2, [A!, E!], A!, "near");
});

test("38-4 (B1): only an admin releases a withheld version (that version: readable again, its flag resolved); a bank-wide release lifts holds but no version; an admin removal hides everything; the admin opens a withheld version on the privacy basis", async () => {
  const { w, adm } = await blocked();
  const [A, E, V] = ["Ann", "Eli", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"], [V!, "viewer"]]);
  const s = await send(w, A!, cid, [CLEAN]);
  const bid = s.body.bank_id;
  await send(w, E!, cid, [`qid:${s.body.qids[0]}:${CLEAN}`, STUDENT], { bid, base: 1 });
  await send(w, A!, cid, [`qid:${s.body.qids[0]}:${CLEAN}`], { bid, base: 2 });
  await send(w, E!, cid, [`qid:${s.body.qids[0]}:${CLEAN}`, STUDENT + " Again."], { bid, base: 3 });
  assert.equal((await w.admin(adm, "GET", "/admin/pool/circle-bank", null, `?id=${bid}&version=2&basis=privacy`)).status, 200, "the admin opens it (audited)");
  // A bank-wide release lifts the hold but releases no withheld version.
  const rel = await w.admin<any>(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, note: "Bank checked by hand." });
  assert.equal(rel.status, 200, rel.text);
  assert.equal(w.q(`SELECT 1 FROM circle_holds WHERE bank_id = ?`, bid).length, 0);
  for (const n of [2, 4]) assert.equal((await version(w, V!, cid, bid, n)).body.error, "version_withheld", `v${n}`);
  assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: V! })).body.error, "version_withheld", "the head (v4) too");
  // Releasing version 2: readable again, its flag resolved; version 4 stays withheld.
  const r2 = await w.admin<any>(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, version: 2, note: "A false positive, checked." });
  assert.deepEqual([r2.status, r2.body.released_version], [200, 2], r2.text);
  assert.equal((await version(w, V!, cid, bid, 2)).status, 200);
  assert.equal(w.q(`SELECT 1 FROM admin_flags f JOIN circle_versions v ON v.id = f.publish_id WHERE v.version = 2 AND v.bank_id = ? AND f.resolved_at IS NULL`, bid).length, 0);
  assert.equal((await version(w, V!, cid, bid, 4)).body.error, "version_withheld");
  assert.equal((await w.admin<any>(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, version: 3, note: "Not withheld at all." })).status, 409);
  // A removal hides every version from every member.
  assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-remove", { bank_id: bid, note: "Removed: student data." })).status, 200);
  for (const n of [1, 2, 3, 4]) assert.equal((await version(w, V!, cid, bid, n)).body.error, "bank_removed");
});

// ---- B2: the byte cap ----------------------------------------------------------------------------------------------

/** A random greyscale PNG of about `bytes` bytes (incompressible pixels). */
function bigPng(bytes: number, seed: number): Uint8Array {
  const r = rng(seed).next;
  const wd = 1400, h = Math.floor(bytes / (wd + 1));
  const ihdr = new Uint8Array(13);
  const dv = new DataView(ihdr.buffer);
  dv.setUint32(0, wd);
  dv.setUint32(4, h);
  ihdr.set([8, 0, 0, 0, 0], 8);
  const raw = new Uint8Array((wd + 1) * h);
  for (let i = 0; i < raw.length; i++) raw[i] = i % (wd + 1) === 0 ? 0 : Math.floor(r() * 256);
  return concat(new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw, { level: 0 })), chunk("IEND", new Uint8Array(0)));
}

test("38-5 (B2): an image-heavy bank (5 images of 2 MB, about 10 MB a version) reaches the 200 MB cap at its 20th version and keeps taking versions and rollbacks (the cap counts what the same commit prunes); a circle that is really full recovers by the owner trimming a bank's history; the 413 says what to do", async () => {
  const w = new World();
  const [O, E] = ["Ola", "Eli"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[E!, "editor"]]);
  const imgs: string[] = [];
  for (let i = 0; i < 5; i++) {
    const u = await upload(w, O!, bigPng(1_999_500, i + 1));
    assert.equal(u.status, 201, JSON.stringify(u.body));
    imgs.push(u.sha);
  }
  const withImgs = (tag: string, qids?: string[], bid?: string) =>
    imgs.map((sha, i) => q(`${tag}: what does figure ${i} show about the braided river?`, { image: { file: `media/${sha}.png`, width: 1400, height: 1427 }, ...(qids ? { published: { item_id: bid, qid: qids[i], publish_id: "p", version: 1 } } : {}) }));
  const s = await send(w, O!, cid, withImgs("v1"));
  assert.equal(s.status, 201, JSON.stringify(s.body));
  const bid = s.body.bank_id;
  const per = w.q<any>(`SELECT bytes FROM circle_versions WHERE bank_id = ?`, bid)[0].bytes;
  assert.ok(per > 9_900_000 && 20 * per <= 200 * 1024 * 1024 && 21 * per > 200 * 1024 * 1024, `${per} bytes a version`);
  let r = s;
  for (let v = 2; v <= 24; v++) {
    r = await send(w, v % 2 ? O! : E!, cid, withImgs(`v${v}`, r.body.qids, bid), { bid, base: v - 1 });
    assert.equal(r.status, 200, `v${v}: ${JSON.stringify(r.body)}`);
  }
  const used = () => w.q<any>(`SELECT SUM(bytes) AS n FROM circle_versions WHERE circle_id = ? AND status IN ('current', 'previous')`, cid)[0].n;
  assert.ok(used() <= 200 * 1024 * 1024 && used() >= 20 * per - 1000, `${used()}`);
  const rb = await rollback(w, O!, cid, bid, { to_version: 20, base_version: 24 });
  assert.equal(rb.status, 200, JSON.stringify(rb.body));
  // Really full: a second bank does not fit (and the answer says what helps).
  const b2 = await send(w, E!, cid, withImgs("second bank"));
  assert.deepEqual([b2.status, b2.body.error], [413, "quota_exceeded"]);
  assert.match(b2.body.message, /trim/i);
  assert.doesNotMatch(b2.body.message, /older version first/);
  // The owner (or the bank's author, D17) trims the bank to its 5 newest versions: their bodies stop counting at once.
  assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${bid}/trim`, { user: E!, json: { keep: 5 } })).status, 403);
  assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${bid}/trim`, { user: O!, json: { keep: 0 } })).status, 400);
  const t = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/trim`, { user: O!, json: { keep: 5 } });
  assert.deepEqual([t.status, t.body.pruned, t.body.oldest_kept], [200, 15, 21], JSON.stringify(t.body));
  assert.ok(used() <= 5 * per + 1000);
  assert.equal((await send(w, E!, cid, withImgs("second bank"))).status, 201);
  const h = await history(w, E!, cid, bid);
  assert.deepEqual([h.body.oldest_kept, h.body.versions.find((x: Q) => x.version === 20).kept], [21, false]);
  const act = (await w.json<any>("GET", `/circles/${cid}/activity`, { user: E! })).body.activity;
  assert.ok(act.some((a: Q) => a.what === "versions_trimmed" && a.by === "Ola"), JSON.stringify(act.slice(0, 3)));
  // The daily stage deletes the trimmed bodies.
  await w.pool.daily();
  assert.equal(w.r2.keys(`circles/${cid}/${bid}/`).length, 2 * 5);
});

// ---- B3 and variant 1: questions keep their origin ---------------------------------------------------------------

async function originWorld() {
  const w = new World();
  const [A, E, D, V] = ["Ann", "Eli", "Dee", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"], [V!, "viewer"]]);
  const T = "Dee's public question about glacial erratics?";
  const pub = (await publish(w, D!, bankOf([q(T)]), { license: "CC-BY-SA-4.0" })).body as Q;
  const withOrigin = { ...q(T), origin: { item_id: pub.item_id, qid: "x", publish_id: pub.publish_id, version: 1 } };
  const s1 = await send(w, A!, cid, [q("Ann's own question about moraines?"), withOrigin], { license: "CC-BY-SA-4.0" });
  assert.equal(s1.status, 201, JSON.stringify(s1.body));
  return { w, A, E, V, cid, T, pub, bid: s1.body.bank_id as string, qids: s1.body.qids as string[] };
}

test("38-6 (B3): a question sent twice with the same published.qid (the app's Duplicate keeps it) is a copy: the first keeps the qid and its origin, the copy gets a new qid and the same origin; nothing counts as removed; the licence stays bound", async () => {
  const { w, A, E, V, cid, T, pub, bid, qids } = await originWorld();
  // (round 39, critic 46b V1: an EXACT copy carries the credit; a changed one does not, round39.test.ts 39-4)
  const d = await send(w, E!, cid, [`qid:${qids[0]}:Ann's own question about moraines?`, `qid:${qids[1]}:${T}`, `qid:${qids[1]}:${T}`], { bid, base: 1, license: "CC-BY-SA-4.0" });
  assert.deepEqual([d.status, d.body.added, d.body.changed, d.body.removed], [200, 1, 0, 0], JSON.stringify(d.body));
  assert.deepEqual(d.body.qids.slice(0, 2), qids);
  assert.notEqual(d.body.qids[2], qids[1]);
  const r = await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: V! });
  assert.deepEqual(r.body.origins.map((o: Q) => [o.pos, o.item_id]), [[1, pub.item_id], [2, pub.item_id]]);
  assert.equal((await w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: A!, json: { license: "circle-only" } })).body.error, "license_incompatible");
  // A repeated qid among many does not trip the large-change guard either.
  const many = await send(w, E!, cid, [`qid:${qids[0]}:Ann's own question about moraines?`, ...Array.from({ length: 4 }, () => `qid:${qids[1]}:${T}`)], { bid, base: 2, license: "CC-BY-SA-4.0" });
  assert.deepEqual([many.status, many.body.removed], [200, 1], JSON.stringify(many.body));
});

test("38-7 (variant 1): an editor re-sending a public question WITHOUT its qid (same text) keeps its recorded origin (matched by the key-free hash), so the author cannot relicense it away", async () => {
  const { w, A, E, V, cid, T, pub, bid, qids } = await originWorld();
  const s2 = await send(w, E!, cid, [`qid:${qids[0]}:Ann's own question about moraines?`, T], { bid, base: 1, license: "CC-BY-SA-4.0" });
  assert.deepEqual([s2.status, s2.body.added, s2.body.removed], [200, 1, 1]);
  const r = await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: V! });
  assert.deepEqual(r.body.origins.map((o: Q) => [o.pos, o.item_id]), [[1, pub.item_id]]);
  assert.equal((await w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: A!, json: { license: "circle-only" } })).body.error, "license_incompatible");
});

// ---- variants: oldest_kept, merge list, demoted author, rollback reservation, mail budget -------------------------------

test("38-8 (variant): a version a report keeps past the 20 is listed as pinned; oldest_kept is where the contiguous kept run starts", async () => {
  const w = new World();
  const [A, R] = ["Ann", "Rae"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[R!, "viewer"]]);
  const s = await send(w, A!, cid, ["v1 about levees?"]);
  const bid = s.body.bank_id;
  assert.equal((await w.json("POST", `/circles/${cid}/banks/${bid}/report`, { user: R!, json: { reason: "other", note: "please check" } })).status, 202);
  let r = s;
  for (let v = 2; v <= 30; v++) r = await send(w, A!, cid, keep(r.body.qids, [`v${v} about levees?`]), { bid, base: v - 1 });
  const h = await history(w, R!, cid, bid);
  assert.deepEqual([h.body.oldest_kept, h.body.pinned_versions], [11, [1]]);
  const miss = await version(w, R!, cid, bid, 5);
  assert.deepEqual([miss.body.error, miss.body.oldest_kept, miss.body.pinned_versions], ["version_not_retained", 11, [1]]);
  assert.equal((await version(w, R!, cid, bid, 1)).status, 200, "the pinned version is kept");
  const mg = await merge(w, A!, cid, bid, 5);
  assert.deepEqual([mg.body.base_kept, mg.body.oldest_kept, mg.body.pinned_versions], [false, 11, [1]]);
});

test("38-9 (variant): merge-initiate lists the FIRST 50 versions after the base, oldest first (between_total says how many there are)", async () => {
  const w = new World();
  const [A, E] = ["Ann", "Eli"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"]]);
  let r = await send(w, A!, cid, ["v1 about dunes?"]);
  const bid = r.body.bank_id;
  for (let v = 2; v <= 60; v++) r = await send(w, A!, cid, keep(r.body.qids, [`v${v} about dunes?`]), { bid, base: v - 1 });
  const mg = await merge(w, E!, cid, bid, 3);
  assert.deepEqual([mg.body.between_total, mg.body.between.length, mg.body.between[0].version, mg.body.between[49].version], [57, 50, 4, 53]);
});

test("38-10 (variant): a demoted author (now a viewer) no longer changes the bank's licence (D19 while an editor)", async () => {
  const w = new World();
  const [O, A] = ["Ola", "Ann"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"]]);
  const s = await send(w, A!, cid, ["Ann's question about tides?"]);
  assert.equal((await w.json("POST", `/circles/${cid}/members/role`, { user: O!, json: { user_id: A!.id, role: "viewer" } })).status, 200);
  const p = await w.json<any>("PATCH", `/circles/${cid}/banks/${s.body.bank_id}`, { user: A!, json: { license: "CC-BY-4.0" } });
  assert.deepEqual([p.status, p.body.error, p.body.needed], [403, "circle_role_required", "editor"]);
});

test("38-11 (variant 4): a rollback reserves memory by the body's value count when that is more than its bytes, as a send does", async () => {
  const w = new World();
  const [A] = ["Ann"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, []);
  // Matching questions with many tiny pairs: many JSON values in few bytes.
  const tiny = (tag: string, qids?: string[], bid?: string) =>
    Array.from({ length: 150 }, (_, i) => ({
      ...q(`${tag} ${i}: match the pairs?`),
      kind: { Matching: { pairs: Array.from({ length: 40 }, (_, j) => ({ left: `${j}`, right: `${j}` })), distractors: [] } },
      ...(qids ? { published: { item_id: bid, qid: qids[i], publish_id: "p", version: 1 } } : {}),
    }));
  const s = await send(w, A!, cid, tiny("v1"));
  assert.equal(s.status, 201, JSON.stringify(s.body).slice(0, 300));
  const bid = s.body.bank_id;
  const r2 = await send(w, A!, cid, tiny("v2", s.body.qids, bid), { bid, base: 1 });
  assert.equal(r2.status, 200, JSON.stringify(r2.body).slice(0, 300));
  const body = new Uint8Array(await (await w.r2.get(w.q<any>(`SELECT full_key FROM circle_versions WHERE bank_id = ? AND version = 1`, bid)[0].full_key))!.arrayBuffer());
  const values = scanJson(body).values;
  assert.ok(values * 96 * 2 > body.length * 15, `${values} values in ${body.length} bytes`);
  let peak = 0;
  const db = w.db as any;
  const prep = db.prepare.bind(db);
  db.prepare = (sql: string) => ((peak = Math.max(peak, bytesReserved())), prep(sql));
  try {
    assert.equal((await rollback(w, A!, cid, bid, { to_version: 1, base_version: 2 })).status, 200);
  } finally {
    db.prepare = prep;
  }
  assert.ok(peak >= values * 96 * 2, `peak ${peak}, by value count ${values * 96 * 2}, by bytes ${body.length * 15}`);
  assert.equal(bytesReserved(), 0);
});

test("38-12 (variant 3): a circle's student-data mail has its own hourly budget (the public pool's publish matches cannot starve it, nor it them)", async () => {
  const { w } = await blocked();
  const [A] = ["Ann"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, []);
  // The public channel's budget for this hour is spent.
  w.sql.prepare(`INSERT OR REPLACE INTO pool_alert_budget (hour, channel, sent) VALUES (?, 'privacy_match', 99)`).run(Math.floor(w.clock / 3_600_000));
  w.notices.length = 0;
  const s = await send(w, A!, cid, [STUDENT]);
  assert.equal(s.body.held, true);
  assert.deepEqual(w.notices.filter((n) => n.kind === "privacy_text_match").map((n: any) => n.circleId), [cid]);
  assert.equal(w.q<any>(`SELECT sent FROM pool_alert_budget WHERE channel = 'privacy_match'`)[0].sent, 99);
});
