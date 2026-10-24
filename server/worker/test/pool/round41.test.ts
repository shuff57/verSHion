// Round 41: circles phase B after critic 48 (no blocker), docs/critic-reports/2026-10-24-circles-round40-critic48.md.
// V1: a bank with an OPEN student-data flag (the admin has not judged it yet) is not purged, neither by the owner's
// purge-now (409 pending_review) nor by the daily purge, so the withheld text the admin needs stays until the admin
// releases, resolves or removes; trim and rollback still work. Nitpick: a notice without a version renders no broken
// link. V4: migration 0110 blanks the listing of a bank whose current version is withheld.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { poolNotifier } from "../../src/poolmail.ts";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { framed, publish, publishId, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "local", name: "b", questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const STUDENT = "Jenny Smith in period 3 scored 42 on the unit test: what is the class median?";
const CLEAN = "Clean question about eskers and drumlins?";

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
const purgeNow = (w: World, u: User, cid: string, bid: string) => w.json<any>("POST", `/circles/${cid}/banks/${bid}/purge`, { user: u });
const openFlags = (w: World, bid: string) => w.q(`SELECT 1 FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ? AND resolved_at IS NULL`, bid).length;

async function flaggedBank() {
  const w = new World();
  const adm = w.user("admin");
  const [O, A, E, P] = ["Olga", "Ann", "Eli", "Pub"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"]]);
  const it = (await publish(w, P!, bankOf([q(STUDENT)]))).body as Q;
  assert.equal((await w.purge(adm, it.item_id, { questions: [0] })).status, 200);
  const s = await send(w, A!, cid, [CLEAN]);
  const bid = s.body.bank_id;
  const d = await send(w, E!, cid, [`qid:${s.body.qids[0]}:${CLEAN}`, STUDENT], { bid, base: 1 });
  assert.equal(d.body.withheld, true);
  return { w, adm, O: O!, A: A!, E: E!, cid, bid, qids: s.body.qids as string[] };
}

test("41-1 (48 V1): while a student-data flag on the bank is open, the owner's purge-now is a plain 409 pending_review (the withheld text stays for the admin); trim and rollback still work; once the admin releases the version the purge goes through", async () => {
  const { w, adm, O, cid, bid } = await flaggedBank();
  assert.equal((await rollback(w, O, cid, bid, { to_version: 1, base_version: 2 })).status, 200, "the owner still repairs it");
  assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${bid}/trim`, { user: O, json: { keep: 1 } })).status, 200);
  assert.equal(w.q<any>(`SELECT status FROM circle_versions WHERE bank_id = ? AND version = 2`, bid)[0].status, "previous", "the flagged version survives the trim");
  assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: O })).status, 200);
  const pg = await purgeNow(w, O, cid, bid);
  assert.deepEqual([pg.status, pg.body.error], [409, "pending_review"], JSON.stringify(pg.body));
  assert.match(pg.body.message, /administrator/);
  assert.equal((await w.admin(adm, "GET", "/admin/pool/circle-bank", null, `?id=${bid}&version=2&basis=privacy`)).status, 200, "the admin can still judge it");
  // The admin releases the version the mail names: the flag is resolved, the purge goes through.
  assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, version: 2, note: "A false positive, checked." })).status, 200);
  assert.equal(openFlags(w, bid), 0);
  assert.equal((await purgeNow(w, O, cid, bid)).status, 200);
  assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ?`, bid).length, 0);
});

test("41-2 (48 V1): the daily purge of a removed bank, and of a closed circle, waits for an open student-data flag like a hold or a report; the admin's resolve (or removal, or close) lets it go", async () => {
  {
    const { w, adm, O, cid, bid } = await flaggedBank();
    assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: O })).status, 200);
    w.advance(15 * DAY);
    await w.pool.daily();
    assert.equal(w.q<any>(`SELECT purged_at FROM circle_banks WHERE id = ?`, bid)[0].purged_at, null, "waits for the admin");
    const flag = w.q<any>(`SELECT id FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ? AND resolved_at IS NULL`, bid)[0].id;
    assert.equal((await w.admin(adm, "POST", "/admin/pool/resolve-flag", { flag_id: flag })).status, 200);
    await w.pool.daily();
    assert.ok(w.q<any>(`SELECT purged_at FROM circle_banks WHERE id = ?`, bid)[0].purged_at !== null);
  }
  {
    const { w, adm, cid, bid } = await flaggedBank();
    assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-remove", { bank_id: bid, note: "Removed: student data." })).status, 200);
    assert.equal(openFlags(w, bid), 0, "the admin's removal decides it");
    w.advance(15 * DAY);
    await w.pool.daily();
    assert.ok(w.q<any>(`SELECT purged_at FROM circle_banks WHERE id = ?`, bid)[0].purged_at !== null);
    void cid;
  }
  {
    // The owner closes the circle: its purge waits for the flag; an admin close decides it.
    const { w, adm, O, cid, bid } = await flaggedBank();
    assert.equal((await w.json("DELETE", `/circles/${cid}`, { user: O })).status, 200);
    w.advance(15 * DAY);
    await w.pool.daily();
    assert.equal(w.q<any>(`SELECT purged_at FROM circles WHERE id = ?`, cid)[0].purged_at, null);
    assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-close", { circle_id: cid, note: "Closed after review." })).status, 200);
    assert.equal(openFlags(w, bid), 0);
    w.advance(15 * DAY);
    await w.pool.daily();
    assert.ok(w.q<any>(`SELECT purged_at FROM circles WHERE id = ?`, cid)[0].purged_at !== null);
  }
});

test("41-3 (48 nitpick): a student-data notice without a version (queued before round 40) renders no version link and points the admin at the flag's version", async () => {
  const sent: string[] = [];
  const env = { DB: { prepare: () => ({ bind: () => ({ first: async () => null, all: async () => ({ results: [] }) }) }) } } as never;
  const orig = globalThis.fetch;
  globalThis.fetch = (async (_u: unknown, init?: { body?: string }) => {
    sent.push(String(init?.body ?? ""));
    return new Response("{}", { status: 200 });
  }) as typeof fetch;
  try {
    await poolNotifier({ ...(env as object), RESEND_API_KEY: "k", MAIL_FROM: "a@x.org", ADMIN_EMAIL: "admin@x.org", MAIL_MODE: "send" } as never)({ kind: "privacy_text_match", itemId: "b".repeat(32), circleId: "c".repeat(32), held: true });
  } finally {
    globalThis.fetch = orig;
  }
  const text = JSON.parse(sent[0]!).text as string;
  assert.doesNotMatch(text, /version=&|version=\s|"version": 0|Version \?/);
  assert.match(text, /flag/);
});

test("41-4 (48 V4): migration 0110 on a populated database records each current version's listing, and blanks the listing of a bank whose current version is withheld", async () => {
  const src = new World();
  const [A] = ["Ann"].map((n) => src.user("teacher", n));
  const cid = await circleWith(src, A!, []);
  const b1 = (await send(src, A!, cid, ["A clean bank's question?"])).body.bank_id;
  const b2 = (await send(src, A!, cid, ["A bank whose head will be withheld?"])).body.bank_id;
  const dir = new URL("../../migrations/", import.meta.url).pathname;
  const files = readdirSync(dir).filter((f) => f.endsWith(".sql")).sort();
  assert.ok(files.includes("0110_pool_round38.sql"));
  const sql = readFileSync(dir + "0110_pool_round38.sql", "utf8");
  // A database at 0109 with those rows.
  const old = new DatabaseSync(":memory:");
  for (const f of files.filter((f) => f < "0110")) old.exec(readFileSync(dir + f, "utf8"));
  old.exec("PRAGMA foreign_keys = OFF");
  const cols = (t: string) => (old.prepare(`PRAGMA table_info("${t}")`).all() as { name: string }[]).map((x) => x.name);
  for (const t of ["users", "circles", "circle_members", "circle_banks", "circle_versions"]) {
    const keep = cols(t);
    for (const row of src.q(`SELECT * FROM "${t}"`)) {
      const ks = Object.keys(row).filter((k) => keep.includes(k));
      old.prepare(`INSERT OR REPLACE INTO "${t}" (${ks.map((k) => `"${k}"`).join(", ")}) VALUES (${ks.map(() => "?").join(", ")})`).run(...(ks.map((k) => (row as Q)[k]) as never[]));
    }
  }
  old.exec(sql);
  const l = old.prepare(`SELECT bank_id, listing FROM circle_versions WHERE status = 'current' ORDER BY bank_id`).all() as { bank_id: string; listing: string }[];
  assert.ok(l.every((x) => JSON.parse(x.listing).title === "Unit 2"), JSON.stringify(l));
  // A database that ran an earlier edit of 0110 (the column already there, a withheld head, no listings): the same
  // statements but the column additions blank that bank's listing and record none for its withheld head.
  const again = new DatabaseSync(":memory:");
  for (const f of files.filter((f) => f < "0110")) again.exec(readFileSync(dir + f, "utf8"));
  again.exec("PRAGMA foreign_keys = OFF");
  for (const t of ["users", "circles", "circle_members", "circle_banks", "circle_versions"]) {
    const keep = (again.prepare(`PRAGMA table_info("${t}")`).all() as { name: string }[]).map((x) => x.name);
    for (const row of src.q(`SELECT * FROM "${t}"`)) {
      const ks = Object.keys(row).filter((k) => keep.includes(k));
      again.prepare(`INSERT OR REPLACE INTO "${t}" (${ks.map((k) => `"${k}"`).join(", ")}) VALUES (${ks.map(() => "?").join(", ")})`).run(...(ks.map((k) => (row as Q)[k]) as never[]));
    }
  }
  again.exec(`ALTER TABLE circle_versions ADD COLUMN withheld_at INTEGER`);
  again.prepare(`UPDATE circle_versions SET withheld_at = 1 WHERE bank_id = ? AND status = 'current'`).run(b2);
  again.exec(`ALTER TABLE circle_versions ADD COLUMN listing TEXT`);
  for (const stmt of sql.replace(/--[^\n]*\n/g, "\n").split(";").map((x) => x.trim()).filter((x) => x && !/^ALTER TABLE/i.test(x))) again.exec(stmt);
  const banks = (again.prepare(`SELECT id, title, subject FROM circle_banks ORDER BY id`).all() as { id: string; title: string; subject: string }[]).map((x) => ({ ...x }));
  assert.deepEqual(banks.find((b) => b.id === b2), { id: b2, title: "", subject: "" });
  assert.deepEqual(banks.find((b) => b.id === b1), { id: b1, title: "Unit 2", subject: "Math" });
  assert.equal((again.prepare(`SELECT listing FROM circle_versions WHERE bank_id = ? AND status = 'current'`).get(b2) as { listing: string | null }).listing, null);
});
