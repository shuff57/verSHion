// Round 34: circles phase A after critics 41a and 41b (docs/critic-reports/2026-10-24-circles-phaseA-*-critic41*.md).
// Each test failed on the round-33 code.
// 1. An owner who closes a circle and then leaves (or is erased) hands it on: the successor keeps it closed and can
//    reopen it inside the 14 days (it was left ownerless and purged).
// 2. Standing: succession and transfers skip suspended (or no longer approved) accounts; a circle with no member in
//    good standing left closes; a suspended owner's codes admit nobody; a suspended author's banks are not served
//    (hidden as a held bank is) until the account is restored.
// 3. F1: the public publish check never names a bank the caller cannot see now: a held bank's hit is generic.
// 4. F6: the owner's delete of a bank its author removed makes it the owner's removal (the author cannot undo it).
// 5. F3: a transfer is an offer the new owner accepts within 7 days; the circles_owned cap is checked at the accept,
//    in its own statement (race-safe); a void offer (expired, offerer no longer owner, target gone) cannot be accepted.
// 6. The circles_owned refusal says a closed circle counts for 14 days.
import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { framed, png, publish, publishId, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const bankOf = (qs: Q[], name = "Circle bank") => JSON.stringify({ schema_version: 2, bank: { id: "local", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const withQs = (n: number, tag: string) => Array.from({ length: n }, (_, i) => q(`${tag} question ${i}: explain the role of the river delta in sediment transport ${i}?`));

async function circle(w: World, u: User, name = "Algebra team"): Promise<string> {
  const r = await w.json<any>("POST", "/circles", { user: u, json: { name } });
  assert.equal(r.status, 201, JSON.stringify(r.body));
  return r.body.circle.id;
}
async function code(w: World, owner: User, cid: string, role: "editor" | "viewer" = "editor", extra: Q = {}): Promise<string> {
  const r = await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role, ...extra } });
  assert.equal(r.status, 201, JSON.stringify(r.body));
  return r.body.code;
}
const join = (w: World, u: User, c: string) => w.json<any>("POST", "/circles/join", { user: u, json: { code: c } });
async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][], name?: string): Promise<string> {
  const cid = await circle(w, owner, name);
  for (const [u, role] of members) {
    assert.equal((await join(w, u, await code(w, owner, cid, role))).status, 200);
    w.advance(1000);
  }
  return cid;
}
async function send(w: World, u: User, cid: string, payload: string) {
  const meta: Q = { commit_id: publishId(), license: "circle-only", attest: { own_work: true, terms_version: POOL_TERMS_VERSION }, meta: { title: "Jane Doe IEP roster", subject: "Math", course: "", grade: "", tags: [], standards: [] } };
  return w.json<any>("POST", `/circles/${cid}/banks`, { user: u, body: framed(meta, payload), contentType: PUB });
}
const owners = (w: World, cid: string) => w.q<{ user_id: string }>(`SELECT user_id FROM circle_members WHERE circle_id = ? AND role = 'owner' AND left_at IS NULL`, cid).map((x) => x.user_id);
const suspend = (w: World, u: User, on = true) => w.sql.prepare(`UPDATE users SET suspended_at = ? WHERE id = ?`).run(on ? w.clock : null, u.id);
const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};
async function days(w: World, n: number) {
  for (let i = 0; i < n; i++) {
    w.advance(DAY);
    await w.pool.hourly();
    await w.pool.daily();
    await Promise.all(w.pending.splice(0));
  }
}
const offer = (w: World, owner: User, cid: string, to: User) => w.json<any>("POST", `/circles/${cid}/transfer`, { user: owner, json: { user_id: to.id } });
const accept = (w: World, u: User, cid: string) => w.json<any>("POST", `/circles/${cid}/transfer/accept`, { user: u });

test("34-1: an owner who closes the circle and then leaves, or is erased, hands it on: still closed, and the successor can reopen it inside the 14 days", async () => {
  for (const how of ["leave", "erase"] as const) {
    const w = new World();
    const [A, E, V] = ["Ann", "Eve", "Vic"].map((n) => w.user("teacher", n));
    const cid = await circleWith(w, A!, [[V!, "viewer"], [E!, "editor"]]);
    const bid = (await send(w, E!, cid, bankOf(withQs(1, "Eve")))).body.bank_id;
    assert.equal((await w.json("DELETE", `/circles/${cid}`, { user: A! })).status, 200);
    if (how === "leave") assert.equal((await w.json("POST", `/circles/${cid}/leave`, { user: A! })).status, 200);
    else await erase(w, A!);
    assert.deepEqual(owners(w, cid), [E!.id], `${how}: the editor takes over`);
    assert.equal(w.q<any>(`SELECT close_reason FROM circles WHERE id = ?`, cid)[0].close_reason, "owner_closed", "still closed");
    w.advance(3 * DAY);
    assert.equal((await w.json("POST", `/circles/${cid}/reopen`, { user: E! })).status, 200, `${how}: the successor reopens`);
    await days(w, 15);
    assert.equal(w.q<any>(`SELECT purged_at FROM circles WHERE id = ?`, cid)[0].purged_at, null, "nothing purged");
    assert.equal((await w.json("GET", `/circles/${cid}/banks/${bid}`, { user: V! })).status, 200);
  }
});

test("34-2: standing: succession and offers skip suspended accounts; a circle with nobody in good standing closes; a suspended owner's codes admit nobody; a suspended author's banks are not served until the account is restored", async () => {
  const w = new World();
  const [A, S1, E2, V, X] = ["Ann", "Sus", "Eli", "Vic", "Xia"].map((n) => w.user("teacher", n));
  // (a) The longest-standing editor is suspended: the next one takes over.
  const c1 = await circleWith(w, A!, [[S1!, "editor"], [E2!, "editor"], [V!, "viewer"]]);
  suspend(w, S1!);
  assert.equal((await w.json("POST", `/circles/${c1}/leave`, { user: A! })).status, 200);
  assert.deepEqual(owners(w, c1), [E2!.id]);
  // (b) An offer to a suspended member is refused.
  const o = await offer(w, E2!, c1, S1!);
  assert.deepEqual([o.status, o.body.error], [409, "member_unavailable"]);
  // (c) Only suspended members left: the owner leaving closes the circle.
  const T = w.user("teacher", "Tia");
  const c2 = await circleWith(w, A!, [[T, "editor"]], "Second");
  suspend(w, T);
  assert.equal((await w.json("POST", `/circles/${c2}/leave`, { user: A! })).status, 200);
  assert.deepEqual(owners(w, c2), []);
  assert.equal(w.q<any>(`SELECT close_reason FROM circles WHERE id = ?`, c2)[0].close_reason, "last_member_gone");
  // (d) A suspended owner's codes admit nobody; restored, they work again.
  const c3 = await circle(w, A!, "Third");
  const k = await code(w, A!, c3, "viewer", { max_uses: 5 });
  suspend(w, A!);
  assert.equal((await join(w, X!, k)).body.error, "circle_invite_invalid");
  suspend(w, A!, false);
  assert.equal((await join(w, X!, k)).status, 200);
  // (e) A suspended author's bank: hidden from every member (read, page, heads, image) as a held bank is.
  const up = await upload(w, E2!, png(7, 7, 3));
  const bid = (await send(w, E2!, c1, bankOf([q("What does the sketch of the terraces show?", { image: { file: `media/${up.sha}.png`, width: 7, height: 7 } })]))).body.bank_id;
  assert.equal((await w.json("GET", `/circles/${c1}/banks/${bid}`, { user: V! })).status, 200);
  suspend(w, E2!);
  assert.equal((await w.json("GET", `/circles/${c1}/banks/${bid}`, { user: V! })).status, 404);
  assert.equal((await w.call("GET", `/circles/${c1}/banks/${bid}/blobs/${up.sha}`, { user: V! })).status, 404);
  assert.deepEqual((await w.json<any>("GET", `/circles/${c1}`, { user: V! })).body.banks, []);
  assert.equal((await w.json<any>("POST", "/circles/heads", { user: V!, json: { bank_ids: [bid] } })).body.banks[0].status, "under_review");
  suspend(w, E2!, false);
  assert.equal((await w.json("GET", `/circles/${c1}/banks/${bid}`, { user: V! })).status, 200);
});

test("34-3 (F1): the publish check names only banks the caller can see now: a held bank's hit is generic (no ids, no titles); a former member's hit has ids but no names", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [O, M, X] = ["Owner", "Member", "Ex"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[M!, "editor"], [X!, "viewer"]]);
  const text = "Jane Doe's IEP goal: which accommodation does the plan list for the unit test?";
  const bid = (await send(w, O!, cid, bankOf([q(text)]))).body.bank_id;
  const visible = await publish(w, M!, bankOf([q(text)]));
  assert.deepEqual([visible.body.error, visible.body.hits[0].bank_title, visible.body.hits[0].circle_name], ["circle_material", "Jane Doe IEP roster", "Algebra team"]);
  await w.admin(adm, "POST", "/admin/pool/circle-hold", { bank_id: bid, note: "A student's IEP in a title." });
  const held = await publish(w, M!, bankOf([q(text)]));
  assert.equal(held.body.error, "circle_material");
  assert.deepEqual(held.body.positions, [0]);
  const h = held.body.hits[0];
  assert.deepEqual([h.bank_title, h.circle_name, h.bank_id, h.circle_id, h.may_claim_independence], [null, null, null, null, false], JSON.stringify(h));
  assert.ok(!JSON.stringify(held.body).includes("Jane Doe IEP roster"));
  // A removed member: still refused, ids (to name in a statement) but no names.
  await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, note: "Title fixed: no student name." });
  assert.equal((await w.json("POST", `/circles/${cid}/members/remove`, { user: O!, json: { user_id: X!.id } })).status, 200);
  const ex = await publish(w, X!, bankOf([q(text)]));
  assert.deepEqual([ex.body.error, ex.body.hits[0].bank_id, ex.body.hits[0].bank_title, ex.body.hits[0].circle_name], ["circle_material", bid, null, null]);
});

test("34-4 (F6): the owner's delete of a bank its author removed makes it the owner's removal: the author cannot undo it, the window is not extended", async () => {
  const w = new World();
  const [A, E] = ["Ann", "Eli"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"]]);
  const bid = (await send(w, E!, cid, bankOf(withQs(1, "Eli")))).body.bank_id;
  const d1 = await w.json<any>("DELETE", `/circles/${cid}/banks/${bid}`, { user: E! });
  w.advance(DAY);
  const d2 = await w.json<any>("DELETE", `/circles/${cid}/banks/${bid}`, { user: A! });
  assert.deepEqual([d2.status, d2.body.undo_until], [200, d1.body.undo_until]);
  assert.equal(w.q<any>(`SELECT removed_reason FROM circle_banks WHERE id = ?`, bid)[0].removed_reason, "owner_removed");
  assert.equal((await w.json("POST", `/circles/${cid}/banks/${bid}/undelete`, { user: E! })).status, 403);
  assert.equal((await w.json("POST", `/circles/${cid}/banks/${bid}/undelete`, { user: A! })).status, 200);
});

test("34-5 (F3): a transfer is an offer: nothing changes until the member accepts it within 7 days; the cap is checked at the accept (race-safe); a void offer cannot be accepted; either side can drop it", async () => {
  const w = new World();
  const [A, B, C, Y] = ["Ann", "Bob", "Cy", "Yan"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[B!, "editor"], [C!, "viewer"]]);
  const k = await code(w, A!, cid, "viewer", { max_uses: 5 });
  const o = await offer(w, A!, cid, B!);
  assert.deepEqual([o.status, o.body.pending.to_user_id, o.body.pending.to_name, o.body.pending.expires_at], [202, B!.id, "Bob", w.clock + 7 * DAY]);
  assert.deepEqual(owners(w, cid), [A!.id], "nothing changes before the accept");
  assert.equal((await w.json<any>("GET", `/circles/${cid}`, { user: B! })).body.pending_transfer.to_user_id, B!.id);
  assert.equal((await w.json<any>("GET", `/circles/${cid}`, { user: C! })).body.pending_transfer, null, "only the owner and the target see it");
  assert.equal((await w.json<any>("GET", "/circles", { user: B! })).body.circles[0].ownership_offered_until, w.clock + 7 * DAY);
  assert.equal((await accept(w, C!, cid)).body.error, "no_pending_transfer", "not offered to C");
  const ok = await accept(w, B!, cid);
  assert.deepEqual([ok.status, owners(w, cid)], [200, [B!.id]]);
  assert.equal((await join(w, Y!, k)).body.error, "circle_invite_invalid", "the old owner's codes went with the role");
  assert.equal((await accept(w, B!, cid)).body.error, "no_pending_transfer", "an offer is used once");
  // Expiry; decline; an offer made void by the offerer leaving.
  assert.equal((await offer(w, B!, cid, C!)).status, 202);
  w.advance(7 * DAY + 1);
  assert.equal((await accept(w, C!, cid)).body.error, "no_pending_transfer");
  assert.equal((await offer(w, B!, cid, C!)).status, 202);
  assert.equal((await w.json("POST", `/circles/${cid}/transfer/cancel`, { user: C! })).status, 200, "the target declines");
  assert.equal((await accept(w, C!, cid)).body.error, "no_pending_transfer");
  assert.equal((await offer(w, B!, cid, C!)).status, 202);
  assert.equal((await w.json("POST", `/circles/${cid}/leave`, { user: B! })).status, 200);
  assert.equal((await accept(w, C!, cid)).body.error, "no_pending_transfer", "the offerer is no longer the owner");
  // The cap at the accept: a member owning 5 cannot take a sixth (and the message says closed ones count 14 days).
  for (let i = 0; i < 5; i++) await circle(w, Y!, `Y${i}`);
  w.advance(DAY);
  const c2 = await circleWith(w, A!, [[Y!, "editor"]], "Fifth");
  assert.equal((await offer(w, A!, c2, Y!)).status, 202);
  const full = await accept(w, Y!, c2);
  assert.deepEqual([full.status, full.body.error, full.body.metric, full.body.limit], [422, "circles_limit", "circles_owned", 5]);
  assert.match(full.body.message, /14 days/);
  // Two accepts at once (Z owns 4, offered two circles): the second runs inside the first at every boundary.
  for (let b = 0; ; b++) {
    const w2 = new World();
    const [Z, P1, P2] = ["Z", "P1", "P2"].map((n) => w2.user("teacher", n));
    for (let i = 0; i < 4; i++) await circle(w2, Z!, `Z${i}`);
    const d1 = await circleWith(w2, P1!, [[Z!, "editor"]], "D1"), d2 = await circleWith(w2, P2!, [[Z!, "editor"]], "D2");
    await offer(w2, P1!, d1, Z!);
    await offer(w2, P2!, d2, Z!);
    const db = w2.db as any;
    const prep = db.prepare.bind(db), batch = db.batch.bind(db);
    let n = 0, inner = false;
    const hit = async () => {
      if (inner) return;
      if (n++ === b) {
        inner = true;
        await accept(w2, Z!, d2);
        inner = false;
      }
    };
    const wrap = (s: any): any => {
      const o2 = Object.create(s);
      o2.bind = (...a: unknown[]) => wrap(s.bind(...a));
      for (const m of ["first", "all", "run"]) o2[m] = async () => (await hit(), s[m]());
      return o2;
    };
    db.prepare = (sql: string) => wrap(prep(sql));
    db.batch = async (ss: unknown[]) => (await hit(), batch(ss));
    try {
      await accept(w2, Z!, d1);
    } finally {
      db.prepare = prep;
      db.batch = batch;
    }
    const owned = w2.q<{ n: number }>(`SELECT COUNT(*) AS n FROM circle_members WHERE user_id = ? AND role = 'owner' AND left_at IS NULL`, Z!.id)[0]!.n;
    assert.ok(owned <= 5, `boundary ${b}: ${owned}`);
    if (b >= n) break;
  }
});

test("34-6: the circles_owned refusal says a closed circle still counts for 14 days", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann");
  const cs: string[] = [];
  for (let i = 0; i < 5; i++) cs.push(await circle(w, A, `C${i}`));
  w.advance(DAY);
  assert.equal((await w.json("DELETE", `/circles/${cs[0]}`, { user: A })).status, 200);
  const r = await w.json<any>("POST", "/circles", { user: A, json: { name: "sixth" } });
  assert.equal(r.body.metric, "circles_owned");
  assert.match(r.body.message, /closed.*14 days|14 days.*closed/i);
});
