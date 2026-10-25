// Round 45: circles PHASE C, the activity list (docs/circles-plan.md D8) after phase C: kinds for the new events
// (`email_invited`, `hide_key_changed`), stable pages (a cursor of the row's time and its permanent id: no row that
// existed at the first page is skipped or repeated, the cursor carries no service-wide counter, a bad cursor is a 400
// rather than page one again), no title of a withheld version (copyright or student data) on any line, and an erased
// actor shown as "deleted teacher" on every line.
import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { framed, publish, publishId, World, type User } from "./harness.ts";

type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "local", name: "b", questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const TEXTBOOK = "Copyrighted textbook problem 4.7: model the white-tailed deer population of the reserve with a yearly harvest of 12 percent.";
const CLEAN = "Clean question about eskers and drumlins and the glaciers that left them?";
const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};
async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][]): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name: "Algebra team" } })).body.circle.id;
  for (const [u, role] of members) {
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } })).body.code;
    assert.equal((await w.json("POST", "/circles/join", { user: u, json: { code: k } })).status, 200);
    w.advance(1000);
  }
  return cid;
}
async function send(w: World, u: User, cid: string, items: (string | Q)[], o: { bid?: string; base?: number; title?: string } = {}) {
  const meta: Q = {
    commit_id: publishId(),
    license: "circle-only",
    attest: { own_work: true, terms_version: POOL_TERMS_VERSION },
    meta: { title: o.title ?? "Unit 2", subject: "Math", course: "", grade: "", tags: [], standards: [] },
    ...(o.bid ? { base_version: o.base } : {}),
  };
  const qs = items.map((x) => (typeof x !== "string" ? x : x.startsWith("qid:") ? q(x.split(":").slice(2).join(":"), { published: { item_id: o.bid, qid: x.split(":")[1], publish_id: "p", version: 1 } }) : q(x)));
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, bankOf(qs)), contentType: PUB });
}
const activity = (w: World, u: User, cid: string, before?: string) => w.json<any>("GET", `/circles/${cid}/activity${before !== undefined ? `?before=${encodeURIComponent(before)}` : ""}`, { user: u });
async function allPages(w: World, u: User, cid: string, between?: () => Promise<void>): Promise<Q[]> {
  const out: Q[] = [];
  let before: string | undefined;
  for (let i = 0; i < 100; i++) {
    const r = await activity(w, u, cid, before);
    assert.equal(r.status, 200, JSON.stringify(r.body));
    out.push(...r.body.activity);
    if (!r.body.next_before) return out;
    before = r.body.next_before;
    await between?.();
  }
  throw new Error("too many pages");
}

test("45-1: the new events have their kinds: an email invite is `email_invited` (who, the role; never an address or an id), turning hidden keys on or off is `hide_key_changed` (the bank, its title, on or off); recording new strip decisions is not an event", async () => {
  const w = new World();
  const [O, A, V] = ["Olga", "Ann", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [V!, "viewer"]]);
  w.advance(1000);
  assert.equal((await w.json("POST", `/circles/${cid}/invites/email`, { user: O!, json: { email: "zed@mail.example", role: "viewer" } })).status, 202);
  w.advance(1000);
  const s = await send(w, A!, cid, [CLEAN]);
  const bid = s.body.bank_id;
  w.advance(1000);
  assert.equal((await w.json("PATCH", `/circles/${cid}/banks/${bid}`, { user: A!, json: { hide_key: true } })).status, 200);
  w.advance(1000);
  assert.equal((await w.json("PATCH", `/circles/${cid}/banks/${bid}`, { user: O!, json: { hide_key: true } })).status, 200, "again: new decisions, no event");
  w.advance(1000);
  assert.equal((await w.json("PATCH", `/circles/${cid}/banks/${bid}`, { user: A!, json: { hide_key: false } })).status, 200);
  const lines = (await activity(w, V!, cid)).body.activity as Q[];
  const mail = lines.filter((l) => l.what === "email_invited");
  assert.equal(mail.length, 1);
  assert.deepEqual(mail[0], { at: mail[0]!.at, what: "email_invited", by: "Olga", member: null, role: "viewer" });
  assert.ok(!JSON.stringify(lines).includes("zed"));
  const hk = lines.filter((l) => l.what === "hide_key_changed");
  assert.deepEqual(hk.map((l) => [l.by, l.bank_id, l.bank_title, l.hide_key]), [["Ann", bid, "Unit 2", false], ["Ann", bid, "Unit 2", true]]);
  assert.ok(!lines.some((l) => l.what === "other"), JSON.stringify(lines.map((l) => l.what)));
});

test("45-2: stable pages: every line that existed at the first page comes exactly once, in order, whatever is written meanwhile (also in the same millisecond); the cursor is the line's time and permanent id (no service-wide counter); a malformed cursor is a 400", async () => {
  const w = new World();
  const [O, A, V] = ["Olga", "Ann", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [V!, "viewer"]]);
  // 130 lines, most of them in the same millisecond (role changes back and forth).
  for (let i = 0; i < 130; i++) {
    assert.equal((await w.json("POST", `/circles/${cid}/members/role`, { user: O!, json: { user_id: A!.id, role: i % 2 === 0 ? "viewer" : "editor" } })).status, 200);
    if (i % 40 === 39) w.advance(1);
  }
  const ids = (rows: Q[]) => rows.map((l) => `${l.at}|${l.what}|${l.role ?? ""}`);
  const first = await allPages(w, V!, cid);
  const count = first.length;
  assert.ok(count >= 130);
  // The same walk while lines keep arriving between pages, in the very millisecond of the cursor.
  const seen = await allPages(w, V!, cid, async () => {
    for (let k = 0; k < 3; k++) await w.json("POST", `/circles/${cid}/members/role`, { user: O!, json: { user_id: A!.id, role: k % 2 === 0 ? "viewer" : "editor" } });
  });
  // Every line of the first walk is in the second one exactly as often (no line skipped or repeated); the newer ones
  // come on top of them.
  const tally = (rows: Q[]) => rows.reduce((m, l) => m.set(JSON.stringify(l), (m.get(JSON.stringify(l)) ?? 0) + 1), new Map<string, number>());
  const a = tally(first);
  const b = tally(seen);
  for (const [k, n] of a) assert.ok((b.get(k) ?? 0) >= n, `missing ${k}`);
  assert.ok(seen.length >= count && seen.length <= count + 9);
  // Newest first.
  for (let i = 1; i < first.length; i++) assert.ok(first[i - 1]!.at >= first[i]!.at);
  assert.equal(new Set(ids(first)).size < first.length, true, "many lines share a millisecond");
  const p1 = await activity(w, V!, cid);
  assert.match(p1.body.next_before, /^\d{1,16}:[0-9a-f-]{32,36}$/);
  const rowid = w.q<{ r: number }>(`SELECT MAX(rowid) AS r FROM pool_audit`)[0]!.r;
  assert.ok(!p1.body.next_before.endsWith(`:${rowid}`), "no service-wide row number in the cursor");
  for (const bad of ["x", "12", "12:", ":abc", "1:2:3", `${"9".repeat(17)}:${"a".repeat(32)}`, "1:ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"]) {
    const r = await activity(w, V!, cid, bad);
    assert.deepEqual([r.status, r.body.field], [400, "before"], bad);
  }
});

test("45-3: no line ever carries the title of a withheld version (copyright or student data), for any member, before or after a clean version or a rollback", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [O, A, E, V, P] = ["Olga", "Ann", "Eli", "Vic", "Pat"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"], [V!, "viewer"]]);
  const item = (await publish(w, P!, bankOf([q(TEXTBOOK)]))).body as Q;
  const nid = (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
  assert.equal((await w.takedown<any>(adm, nid, item.item_id, { questions: [0] })).status, 200);
  const s1 = await send(w, A!, cid, [CLEAN], { title: "Unit 1" });
  const bid = s1.body.bank_id;
  const s2 = await send(w, E!, cid, [`qid:${s1.body.qids[0]}:${CLEAN}`, TEXTBOOK], { bid, base: 1, title: "Jenny Smith P3 scores" });
  assert.equal(s2.body.withheld, true);
  const leak = "Jenny Smith";
  for (const u of [O!, A!, E!, V!]) assert.ok(!JSON.stringify((await activity(w, u, cid)).body).includes(leak), u.name);
  const own = (await activity(w, O!, cid)).body.activity.filter((l: Q) => l.bank_id === bid);
  assert.ok(own.length > 0 && own.every((l: Q) => l.bank_title === "Unit 1"), JSON.stringify(own));
  const vic = (await activity(w, V!, cid)).body.activity.filter((l: Q) => l.bank_id === bid);
  assert.ok(vic.every((l: Q) => l.bank_title === null), "hidden from a member who may not see the bank while it is held");
  // A rollback by the owner: the withheld title never comes back.
  assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${bid}/rollback`, { user: O!, json: { commit_id: publishId(), to_version: 1, base_version: 2 } })).status, 200);
  for (const u of [O!, A!, E!, V!]) {
    const t = JSON.stringify((await activity(w, u, cid)).body);
    assert.ok(!t.includes(leak), u.name);
    assert.ok(t.includes("Unit 1"));
  }
});

test("45-4: an erased actor is 'deleted teacher' on every line (an email invite, a key setting, a version, a join), and the account's name appears nowhere in the list", async () => {
  const w = new World();
  const [O, A, V] = ["Olga", "Annabel Quist", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [V!, "viewer"]]);
  const s = await send(w, A!, cid, [CLEAN]);
  assert.equal((await w.json("PATCH", `/circles/${cid}/banks/${s.body.bank_id}`, { user: A!, json: { hide_key: true } })).status, 200);
  // Annabel becomes the owner and invites by email, then is erased (Olga takes the circle back by succession).
  assert.equal((await w.json("POST", `/circles/${cid}/transfer`, { user: O!, json: { user_id: A!.id } })).status, 202);
  assert.equal((await w.json("POST", `/circles/${cid}/transfer/accept`, { user: A! })).status, 200);
  assert.equal((await w.json("POST", `/circles/${cid}/invites/email`, { user: A!, json: { email: "zed@mail.example", role: "viewer" } })).status, 202);
  await erase(w, A!);
  const lines = (await activity(w, V!, cid)).body.activity as Q[];
  const t = JSON.stringify(lines);
  assert.ok(!t.includes("Annabel") && !t.includes("Quist"));
  for (const what of ["email_invited", "hide_key_changed", "bank_shared", "joined"]) {
    const l = lines.find((x) => x.what === what && (x.by === "deleted teacher" || x.member === "deleted teacher"));
    assert.ok(l, `${what}: ${t}`);
  }
  assert.ok(lines.some((l) => l.what === "ownership_passed_on" && l.member === "Olga"));
});
