// Round 44: circles PHASE C, copyright re-upload block checks on sends (docs/circles-plan.md 13.3). The public
// pool's re-upload blocks (blocked_hashes: what a DMCA takedown removed, by every block key and image) are checked
// on every circle send and rollback, by the public path's rules (an active, unexpired block of a takedown; never
// what the admin exempted the sender from; never the rightful item's own material for its author or an account it
// was served to). A match never refuses: in the commit (no gap) it withholds THAT version from every member and holds
// the bank while it is the head, exactly as a student-data match does, so nothing a member sees tells a copyright
// match from a student-data match (the public path tells its publisher only "held for review"). The admin gets a
// `blocklist` flag and a mail; the account's matches count toward the public path's held-publish budget.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { poolNotifier } from "../../src/poolmail.ts";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { framed, png, publish, publishId, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "local", name: "b", questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const TEXTBOOK = "Copyrighted textbook problem 4.7: model the white-tailed deer population of the reserve with a yearly harvest of 12 percent.";
const STUDENT = "Jenny Smith in period 3 scored 42 on the unit test: what is the class median?";
const CLEAN = "Clean question about eskers and drumlins and the glaciers that left them?";
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;

async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][]): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name: "Algebra team" } })).body.circle.id;
  for (const [u, role] of members) {
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } })).body.code;
    assert.equal((await w.json("POST", "/circles/join", { user: u, json: { code: k } })).status, 200);
    w.advance(1000);
  }
  return cid;
}
async function send(w: World, u: User, cid: string, items: (string | Q)[], o: { bid?: string; base?: number; confirm?: boolean } = {}) {
  const meta: Q = {
    commit_id: publishId(),
    license: "circle-only",
    attest: { own_work: true, terms_version: POOL_TERMS_VERSION },
    meta: { title: "Unit 2", subject: "Math", course: "", grade: "", tags: [], standards: [] },
    ...(o.bid ? { base_version: o.base } : {}),
    ...(o.confirm ? { confirm_large_change: true } : {}),
  };
  const qs = items.map((x) => (typeof x !== "string" ? x : x.startsWith("qid:") ? q(x.split(":").slice(2).join(":"), { published: { item_id: o.bid, qid: x.split(":")[1], publish_id: "p", version: 1 } }) : q(x)));
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, bankOf(qs)), contentType: PUB });
}
const rollback = (w: World, u: User, cid: string, bid: string, body: Q) => w.json<any>("POST", `/circles/${cid}/banks/${bid}/rollback`, { user: u, json: { commit_id: publishId(), ...body } });
const flags = (w: World, bid: string, kind = "blocklist") => w.q<any>(`SELECT * FROM admin_flags WHERE kind = ? AND item_id = ?`, kind, bid);
const strip = (b: Q, drop: string[]) => Object.fromEntries(Object.entries(b).filter(([k]) => !drop.includes(k)));

/** A circle (owner Olga; Ann the author, Eli an editor, Vic a viewer), Pat's public item taken down (its text blocked). */
async function world(o: { takedown?: boolean; exempt?: string[] } = {}) {
  const w = new World();
  const adm = w.user("admin");
  const [O, A, E, V, P] = ["Olga", "Ann", "Eli", "Vic", "Pat"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"], [V!, "viewer"]]);
  const item = (await publish(w, P!, bankOf([q(TEXTBOOK)]))).body as Q;
  let actionId: string | null = null;
  if (o.takedown !== false) {
    const td = await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: [0], ...(o.exempt ? { exempt_authors: o.exempt.map((n) => ({ Olga: O, Ann: A, Eli: E, Vic: V } as Q)[n].id) } : {}) });
    assert.equal(td.status, 200, JSON.stringify(td.body));
    actionId = td.body.action_id;
  }
  return { w, adm, O: O!, A: A!, E: E!, V: V!, P: P!, cid, item, actionId };
}

test("44-1: a send holding material a takedown blocked is accepted but withheld (held, withheld), the bank hidden from members but its author and owner while it is the head, a blocklist flag and an admin mail; a clean version lifts the hold, the matching version stays withheld until the admin releases it", async () => {
  const { w, adm, O, A, E, V, cid } = await world();
  const s1 = await send(w, A, cid, [CLEAN]);
  const bid = s1.body.bank_id;
  const s2 = await send(w, E, cid, [`qid:${s1.body.qids[0]}:${CLEAN}`, TEXTBOOK], { bid, base: 1 });
  assert.equal(s2.status, 200, JSON.stringify(s2.body));
  assert.deepEqual([s2.body.held, s2.body.withheld, s2.body.version], [true, true, 2]);
  // Withheld from every member, its author and the owner included; the bank hidden from the others.
  for (const u of [A, O]) assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: u })).body.error, "version_withheld");
  for (const u of [E, V]) assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: u })).status, 404);
  assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/2`, { user: A })).body.error, "version_withheld");
  assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/1`, { user: A })).status, 200);
  const line = (await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions`, { user: A })).body.versions[0];
  assert.deepEqual([line.version, line.withheld, line.kept, line.message], [2, true, false, ""]);
  assert.equal((await w.json<any>("GET", `/circles/${cid}`, { user: V })).body.banks.length, 0);
  assert.equal((await w.json<any>("POST", "/circles/heads", { user: V, json: { bank_ids: [bid] } })).body.banks[0].status, "under_review");
  // The admin's flag (ids, positions, counts) and mail.
  const f = flags(w, bid);
  assert.equal(f.length, 1);
  const vid2 = w.q<any>(`SELECT id FROM circle_versions WHERE bank_id = ? AND version = 2`, bid)[0].id;
  assert.equal(f[0].publish_id, vid2);
  // (round 47: and the matching block hashes, the evidence a flag keeps when its version's body is pruned)
  assert.deepEqual(strip(JSON.parse(f[0].detail), ["actions", "hashes"]), { circle_id: cid, version: 2, positions: { count: 1, first: [1] } });
  assert.ok(JSON.parse(f[0].detail).hashes.length > 0);
  assert.ok(!f[0].detail.includes("deer"), "no text in the flag");
  assert.deepEqual(w.notices.filter((n) => n.kind === "circle_copyright_match"), [{ kind: "circle_copyright_match", itemId: bid, circleId: cid, version: 2 }]);
  assert.equal(w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'held_publishes'`, E.id)[0]?.count, 1, "the public path's held-publish budget");
  // The author fixes it: a clean version lifts the hold; version 2 stays withheld.
  const s3 = await send(w, A, cid, [`qid:${s1.body.qids[0]}:${CLEAN}`], { bid, base: 2 });
  assert.deepEqual([s3.status, s3.body.held, s3.body.withheld], [200, false, false]);
  assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: V })).status, 200);
  assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/2`, { user: V })).body.error, "version_withheld");
  // The admin opens it on its basis (audited) and releases THAT version.
  assert.equal((await w.admin(adm, "GET", "/admin/pool/circle-bank", null, `?id=${bid}&version=2&basis=privacy`)).status, 403, "no student-data basis");
  const open = await w.admin<any>(adm, "GET", "/admin/pool/circle-bank", null, `?id=${bid}&version=2&basis=copyright`);
  assert.equal(open.status, 200, open.text.slice(0, 200));
  assert.ok(open.text.includes("white-tailed deer"));
  assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, version: 2, note: "The publisher licensed it to the school." })).status, 200);
  assert.equal(flags(w, bid)[0].resolved_at !== null, true);
  assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/2`, { user: V })).status, 200);
});

test("44-2: no oracle beyond what a student-data match already shows: a copyright match and a student-data match look the same to every member (the send's answer, the version's 404, the history line, the circle page, heads, the purge refusal), and no member-facing message names either", async () => {
  const { w, adm, O, A, E, V, P, cid } = await world();
  const it = (await publish(w, P, bankOf([q(STUDENT)]))).body as Q;
  assert.equal((await w.purge(adm, it.item_id, { questions: [0] })).status, 200);
  const out: Record<string, Q> = {};
  for (const [k, text] of [["copyright", TEXTBOOK], ["student", STUDENT]] as const) {
    const s1 = await send(w, A, cid, [CLEAN]);
    const bid = s1.body.bank_id;
    const s2 = await send(w, E, cid, [`qid:${s1.body.qids[0]}:${CLEAN}`, text], { bid, base: 1 });
    const page = (await w.json<any>("GET", `/circles/${cid}`, { user: O })).body.banks.find((b: Q) => b.id === bid);
    const line = (await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions`, { user: A })).body.versions[0];
    const head = (await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/2`, { user: O })).body;
    const heads = (await w.json<any>("POST", "/circles/heads", { user: V, json: { bank_ids: [bid] } })).body.banks[0].status;
    assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: O })).status, 200);
    out[k] = {
      send: strip(s2.body, ["bank_id", "version_id", "qids", "circle_id"]),
      head,
      line: strip(line, ["version_id", "at"]),
      page: strip(page, ["id", "created_at", "updated_at"]),
      heads,
      purge: (await w.json<any>("POST", `/circles/${cid}/banks/${bid}/purge`, { user: O })).body,
    };
  }
  assert.deepEqual(out.copyright, out.student);
  assert.equal(out.copyright!.send.withheld, true);
  for (const m of [out.copyright!.head.message, out.copyright!.purge.message]) assert.doesNotMatch(m, /student|copyright|takedown|privacy/i, m);
});

test("44-3: the public path's exemptions: an account the admin exempted from the takedown, and the rightful item's author (and an account it was served to) send that material unheld; a lifted or expired block holds nothing", async () => {
  {
    const { w, A, E, cid } = await world({ exempt: ["Eli"] });
    const e = await send(w, E, cid, [TEXTBOOK]);
    assert.deepEqual([e.status, e.body.held, e.body.withheld], [201, false, false], "exempted");
    const a = await send(w, A, cid, [TEXTBOOK]);
    assert.deepEqual([a.body.held, a.body.withheld], [true, true]);
  }
  {
    // Rose wrote X; Pat's copy Y is taken down naming X as the rightful item.
    const w = new World();
    const adm = w.user("admin");
    const [O, R, P, M, N] = ["Olga", "Rose", "Pat", "Mo", "Nia"].map((n) => w.user("teacher", n));
    const cid = await circleWith(w, O!, [[R!, "editor"], [M!, "editor"], [N!, "editor"]]);
    const x = (await publish(w, R!, bankOf([q(TEXTBOOK)]))).body as Q;
    w.advance(DAY);
    assert.equal((await w.json("POST", `/items/${x.item_id}/import`, { user: P! })).status, 200);
    const y = (await publish(w, P!, bankOf([q(TEXTBOOK)]))).body as Q;
    assert.equal((await w.json("POST", `/items/${x.item_id}/import`, { user: N! })).status, 200);
    const td = await w.takedown<any>(adm, await notice(w, adm), y.item_id, { questions: [0], rightful_item_id: x.item_id });
    assert.equal(td.status, 200, JSON.stringify(td.body));
    assert.ok(w.q(`SELECT 1 FROM blocked_hashes WHERE action_id = ? AND lifted_at IS NULL`, td.body.action_id).length > 0);
    assert.deepEqual((await send(w, R!, cid, [TEXTBOOK])).body.withheld, false, "the rightful author's own words");
    assert.deepEqual((await send(w, N!, cid, [TEXTBOOK])).body.withheld, false, "served the rightful item");
    assert.deepEqual((await send(w, M!, cid, [TEXTBOOK])).body.withheld, true, "anyone else");
  }
  {
    const { w, adm, A, cid, actionId } = await world();
    assert.equal((await w.admin(adm, "POST", "/admin/dmca/unblock", { action_id: actionId, note: "Withdrawn by the complainant." })).status, 200);
    assert.equal((await send(w, A, cid, [TEXTBOOK])).body.withheld, false, "a lifted block");
  }
  {
    const { w, A, cid } = await world();
    w.advance(61 * DAY);
    assert.equal((await send(w, A, cid, [TEXTBOOK])).body.withheld, false, "an expired block (60 days by default)");
  }
});

test("44-4: a blocked IMAGE in a send is held the same way; the image route never serves it from the withheld version", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [O, A, P, V] = ["Olga", "Ann", "Pat", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [V!, "viewer"]]);
  const bytes = png(6, 6, 9);
  const pi = await upload(w, P!, bytes);
  const img = (sha: string) => ({ file: `media/${sha}.png`, width: 6, height: 6 });
  const item = (await publish(w, P!, bankOf([q("Label the parts of the diagram from the atlas plate.", { image: img(pi.sha) })]))).body as Q;
  const td = await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: [], images: [pi.sha] });
  assert.equal(td.status, 200, JSON.stringify(td.body));
  const ai = await upload(w, A!, bytes);
  const s = await send(w, A!, cid, [q("My own words about this diagram, nothing copied?", { image: img(ai.sha) })]);
  assert.equal(s.status, 201, JSON.stringify(s.body));
  assert.deepEqual([s.body.held, s.body.withheld], [true, true]);
  assert.equal((await w.call("GET", `/circles/${cid}/banks/${s.body.bank_id}/blobs/${pi.sha}`, { user: A! })).status, 404);
});

test("44-5: not retroactive, like student data: a version sent before the takedown stays readable; a rollback to it after the takedown makes a withheld version; a rollback of a withheld version is refused", async () => {
  const { w, adm, O, A, V, cid, item } = await world({ takedown: false });
  const s1 = await send(w, A, cid, [TEXTBOOK]);
  const bid = s1.body.bank_id;
  assert.equal(s1.body.withheld, false);
  await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: [0] });
  assert.equal((await w.json("GET", `/circles/${cid}/banks/${bid}`, { user: V })).status, 200, "not retroactive");
  const s2 = await send(w, A, cid, [CLEAN], { bid, base: 1, confirm: true });
  assert.equal(s2.body.withheld, false);
  const rb = await rollback(w, O, cid, bid, { to_version: 1, base_version: 2, confirm_large_change: true });
  assert.equal(rb.status, 200, JSON.stringify(rb.body));
  assert.deepEqual([rb.body.held, rb.body.withheld], [true, true]);
  assert.equal(flags(w, bid).length, 1);
  assert.equal(JSON.parse(flags(w, bid)[0].detail).rollback_of, 1);
  assert.equal((await rollback(w, O, cid, bid, { to_version: 2, base_version: 3, confirm_large_change: true })).body.withheld, false, "back to the clean one");
  const rb3 = await rollback(w, O, cid, bid, { to_version: 3, base_version: 4 });
  assert.deepEqual([rb3.status, rb3.body.error], [404, "version_withheld"], "never back to a withheld version");
});

test("44-6: the check is repeated inside the commit: a block confirmed while the send is in flight still withholds it (no gap)", async () => {
  const { w, adm, A, E, cid, item } = await world({ takedown: false });
  const s1 = await send(w, A, cid, [CLEAN]);
  const bid = s1.body.bank_id;
  w.r2.onPutPrefix = "circles/";
  w.r2.onPut = async () => void (await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: [0] }));
  const s2 = await send(w, E, cid, [`qid:${s1.body.qids[0]}:${CLEAN}`, TEXTBOOK], { bid, base: 1 });
  assert.equal(s2.status, 200);
  assert.deepEqual([s2.body.held, s2.body.withheld], [true, true]);
  assert.equal(flags(w, bid).length, 1);
});

test("44-7: the admin's side: the purge and the daily purge wait while the flag is open, a trim keeps the withheld version, a bank-wide release lifts the hold but leaves the version withheld, the generic resolve never makes it readable, circle-remove and circle-close decide it", async () => {
  {
    const { w, adm, O, A, E, cid } = await world();
    const s1 = await send(w, A, cid, [CLEAN]);
    const bid = s1.body.bank_id;
    await send(w, E, cid, [`qid:${s1.body.qids[0]}:${CLEAN}`, TEXTBOOK], { bid, base: 1 });
    assert.equal((await rollback(w, O, cid, bid, { to_version: 1, base_version: 2 })).status, 200);
    assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${bid}/trim`, { user: O, json: { keep: 1 } })).status, 200);
    assert.equal(w.q<any>(`SELECT status FROM circle_versions WHERE bank_id = ? AND version = 2`, bid)[0].status, "previous", "the withheld version survives the trim while its flag is open");
    assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: O })).status, 200);
    // (round 48: 200 purged_partial, pending_review; see round 48)
    assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${bid}/purge`, { user: O })).body.pending_review, true);
    w.advance(15 * DAY);
    await w.pool.daily();
    assert.equal(w.q<any>(`SELECT purged_at FROM circle_banks WHERE id = ?`, bid)[0].purged_at, null);
    // The generic resolve decides the flag; the version is not made readable by it.
    const fid = flags(w, bid)[0].id;
    assert.equal((await w.admin(adm, "POST", "/admin/pool/resolve-flag", { flag_id: fid })).status, 200);
    assert.ok(w.q<any>(`SELECT withheld_at FROM circle_versions WHERE bank_id = ? AND version = 2`, bid)[0].withheld_at !== null);
    await w.pool.daily();
    assert.ok(w.q<any>(`SELECT purged_at FROM circle_banks WHERE id = ?`, bid)[0].purged_at !== null);
  }
  {
    const { w, adm, A, E, V, cid } = await world();
    const s1 = await send(w, A, cid, [CLEAN]);
    const bid = s1.body.bank_id;
    await send(w, E, cid, [`qid:${s1.body.qids[0]}:${CLEAN}`, TEXTBOOK], { bid, base: 1 });
    assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, note: "Bank-wide release, checked." })).status, 200);
    assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: V })).body.error, "version_withheld", "listed again, its head still withheld");
    assert.equal(flags(w, bid)[0].resolved_at, null, "the version's flag stays for a version release");
    assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-remove", { bank_id: bid, note: "Removed: infringing." })).status, 200);
    assert.ok(flags(w, bid)[0].resolved_at !== null);
  }
  {
    const { w, adm, O, A, E, cid } = await world();
    const s1 = await send(w, A, cid, [CLEAN]);
    const bid = s1.body.bank_id;
    await send(w, E, cid, [`qid:${s1.body.qids[0]}:${CLEAN}`, TEXTBOOK], { bid, base: 1 });
    assert.equal((await w.json("DELETE", `/circles/${cid}`, { user: O })).status, 200);
    w.advance(15 * DAY);
    await w.pool.daily();
    assert.equal(w.q<any>(`SELECT purged_at FROM circles WHERE id = ?`, cid)[0].purged_at, null, "the circle's purge waits too");
    assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-close", { circle_id: cid, note: "Closed after review." })).status, 200);
    assert.ok(flags(w, bid)[0].resolved_at !== null);
  }
});

test("44-8: one held-publish budget per account across public publishes and circle sends: matching questions are counted, and past the day's 20 the admin is flagged", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [O, E, P] = ["Olga", "Eli", "Pat"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[E!, "editor"]]);
  const texts = Array.from({ length: 21 }, (_, i) => `Copyrighted workbook exercise ${i}: describe stage ${i} of the nitrogen cycle in the estuary the plate shows.`);
  const item = (await publish(w, P!, bankOf(texts.map((t) => q(t))))).body as Q;
  const td = await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: texts.map((_, i) => i) });
  assert.equal(td.status, 200);
  const s = await send(w, E!, cid, texts);
  assert.equal(s.body.withheld, true);
  assert.equal(w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'held_publishes'`, E!.id)[0].count, 21);
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'cap' AND user_id = ? AND detail LIKE '%held_publishes%'`, E!.id).length, 1);
});

test("44-9: the admin mail names the version and the basis to open it (ids only); migration 0113 keeps every hold and accepts the copyright reason", async () => {
  const sent: string[] = [];
  const orig = globalThis.fetch;
  globalThis.fetch = (async (_u: unknown, init?: { body?: string }) => {
    sent.push(String(init?.body ?? ""));
    return new Response("{}", { status: 200 });
  }) as typeof fetch;
  try {
    await poolNotifier({ DB: {} as D1Database, RESEND_API_KEY: "k", MAIL_FROM: "a@x.org", ADMIN_EMAIL: "admin@x.org" } as never)({ kind: "circle_copyright_match", itemId: "b".repeat(32), circleId: "c".repeat(32), version: 3 });
  } finally {
    globalThis.fetch = orig;
  }
  const m = JSON.parse(sent[0]!) as Q;
  assert.deepEqual(m.to, ["admin@x.org"]);
  assert.match(m.text, /version=3&basis=copyright/);
  assert.match(m.text, /"version": 3/);
  // 0113 on a database at 0112 with holds.
  const dir = new URL("../../migrations/", import.meta.url).pathname;
  const files = readdirSync(dir).filter((f) => f.endsWith(".sql")).sort();
  const mig = files.find((f) => f.startsWith("0113_"))!;
  assert.ok(mig, "migration 0113 exists");
  const db = new DatabaseSync(":memory:");
  for (const f of files.filter((f) => f < "0113")) db.exec(readFileSync(dir + f, "utf8"));
  db.exec(`INSERT INTO circle_holds (bank_id, reason, source, at, by) VALUES ('b1', 'student_data', 'privacy', 1, NULL), ('b1', 'reports', '', 2, NULL), ('b2', 'admin', 'manual', 3, 'adm')`);
  db.exec(readFileSync(dir + mig, "utf8"));
  assert.deepEqual(db.prepare(`SELECT bank_id, reason, source, at, by FROM circle_holds ORDER BY bank_id, reason`).all().map((x) => ({ ...x })), [
    { bank_id: "b1", reason: "reports", source: "", at: 2, by: null },
    { bank_id: "b1", reason: "student_data", source: "privacy", at: 1, by: null },
    { bank_id: "b2", reason: "admin", source: "manual", at: 3, by: "adm" },
  ]);
  db.exec(`INSERT INTO circle_holds (bank_id, reason, source, at) VALUES ('b3', 'copyright', 'blocklist', 4)`);
  assert.throws(() => db.exec(`INSERT INTO circle_holds (bank_id, reason, source, at) VALUES ('b3', 'nonsense', '', 4)`));
  assert.ok((db.prepare(`SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'circle_holds_at'`).all() as unknown[]).length === 1);
});
