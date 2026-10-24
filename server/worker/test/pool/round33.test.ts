// Round 33: fixes after the circles phase A critics (critic40a, critic40b; docs/critic-reports/2026-10-24-*).
// Each test here failed on the round-32 code.
// 1. A removed member cannot rejoin with a code issued before the removal; a member who left (or was
//    demoted, then left) rejoins with an older code at most in the role held when leaving.
// 2. Codes never outlive their issuer's owner role (leave, transfer, removal); the list names the issuer.
// 3. circles_owned holds through a transfer (in the same statement: race-safe).
// 4. A hold delays a bank's and a circle's purge as an open report does.
// 5. Only the owner undoes the owner's removal of a bank.
// 6. The public publish check is bounded: one row per hash with a hit, at most `circle_memberships_total`
//    circles considered (only circles holding records), a memory reservation; and memberships, former ones
//    included, are capped per account.
// 7. A circle send reserves memory by the publish formula (dense JSON included).
// V1. The activity list never shows the title of a bank the viewer may not see.
// V2. No version under an open report, and none while the bank is held, is pruned.
// V3. The owner purges a removed bank at once (it then stops counting toward the circle's caps).
import assert from "node:assert/strict";
import { test } from "node:test";
import { CIRCLE_HITS_SQL, checkCircleMaterial } from "../../src/pool/circlecheck.ts";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { bytesReserved } from "../../src/pool/shared.ts";
import { core, framed, publishId, World, type User } from "./harness.ts";

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
async function send(w: World, u: User, cid: string, payload: string, o: { bid?: string; base?: number } = {}) {
  const meta: Q = { commit_id: publishId(), license: "circle-only", attest: { own_work: true, terms_version: POOL_TERMS_VERSION }, meta: { title: "Unit 2 quiz", subject: "Math", course: "", grade: "", tags: [], standards: [] }, ...(o.bid ? { base_version: o.base, confirm_large_change: true } : {}) }; // (round 37: see round32's send)
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, payload), contentType: PUB });
}
const owners = (w: World, cid: string) => w.q<{ user_id: string }>(`SELECT user_id FROM circle_members WHERE circle_id = ? AND role = 'owner' AND left_at IS NULL`, cid).map((x) => x.user_id);
const roleOf = (w: World, cid: string, u: User) => w.q<{ role: string; left_at: number | null }>(`SELECT role, left_at FROM circle_members WHERE circle_id = ? AND user_id = ?`, cid, u.id)[0];
async function days(w: World, n: number) {
  for (let i = 0; i < n; i++) {
    w.advance(DAY);
    await w.pool.hourly();
    await w.pool.daily();
    await Promise.all(w.pending.splice(0));
  }
}

test("33-1: a removed member cannot rejoin with a code issued before the removal (a new code works); a member who left rejoins with an older code at most in the role held when leaving", async () => {
  const w = new World();
  const [A, X, Y, Z] = ["Ann", "Xavier", "Yara", "Zed"].map((n) => w.user("teacher", n));
  const cid = await circle(w, A!);
  const team = await code(w, A!, cid, "editor", { max_uses: 10 });
  for (const u of [X!, Y!, Z!]) assert.equal((await join(w, u, team)).status, 200);
  w.advance(1000);
  // Removal: the same multi-use code no longer lets X back in (preview too); the same 422 as any bad code.
  assert.equal((await w.json("POST", `/circles/${cid}/members/remove`, { user: A!, json: { user_id: X!.id } })).status, 200);
  const again = await join(w, X!, team);
  assert.deepEqual([again.status, again.body.error], [422, "circle_invite_invalid"], JSON.stringify(again.body));
  assert.equal((await w.json<any>("POST", "/circles/join/preview", { user: X!, json: { code: team } })).status, 422);
  assert.equal(roleOf(w, cid, X!)!.left_at !== null, true, "still out");
  // A code issued AFTER the removal works (the owner chose to let them back).
  w.advance(1000);
  const fresh = await code(w, A!, cid, "viewer");
  const back = await join(w, X!, fresh);
  assert.deepEqual([back.status, back.body.role], [200, "viewer"]);
  // Demotion, then leave, then the old editor code: back only as a viewer.
  assert.equal((await w.json("POST", `/circles/${cid}/members/role`, { user: A!, json: { user_id: Y!.id, role: "viewer" } })).status, 200);
  w.advance(1000);
  assert.equal((await w.json("POST", `/circles/${cid}/leave`, { user: Y! })).status, 200);
  const y = await join(w, Y!, team);
  assert.deepEqual([y.status, y.body.role], [200, "viewer"], JSON.stringify(y.body));
  assert.equal(roleOf(w, cid, Y!)!.role, "viewer");
  // An editor who left rejoins as an editor with the old code; with a newer code, the code's role.
  assert.equal((await w.json("POST", `/circles/${cid}/leave`, { user: Z! })).status, 200);
  assert.equal((await join(w, Z!, team)).body.role, "editor");
  assert.equal((await w.json("POST", `/circles/${cid}/leave`, { user: Y! })).status, 200);
  w.advance(1000);
  const newer = await code(w, A!, cid, "editor");
  assert.equal((await join(w, Y!, newer)).body.role, "editor", "a code issued after leaving gives its own role");
});

test("33-2: codes never outlive their issuer's owner role: a transfer, a leave and a removal revoke the issuer's open codes (and a code whose issuer is no longer the owner is refused anyway); the list names the issuer", async () => {
  const w = new World();
  const [A, B, S, T] = ["Ann", "Bob", "Stranger", "Tess"].map((n) => w.user("teacher", n));
  const cid = await circle(w, A!);
  assert.equal((await join(w, B!, await code(w, A!, cid, "editor"))).status, 200);
  const big = await code(w, A!, cid, "editor", { max_uses: 50, expires_in_days: 30 });
  const lst = await w.json<any>("GET", `/circles/${cid}/invites`, { user: A! });
  assert.ok(lst.body.invites.every((i: Q) => i.created_by === A!.id && i.created_by_name === "Ann"), JSON.stringify(lst.body.invites[0]));
  // Transfer: A's open codes are revoked in the same batch.
  assert.equal((await w.json("POST", `/circles/${cid}/transfer`, { user: A!, json: { user_id: B!.id } })).status, 202);
  assert.equal((await w.json("POST", `/circles/${cid}/transfer/accept`, { user: B! })).status, 200, "(round 34: accepted)");
  assert.deepEqual((await join(w, S!, big)).body.error, "circle_invite_invalid", "the former owner's code no longer works");
  assert.ok(w.q<any>(`SELECT revoked_at FROM circle_invites WHERE created_by = ?`, A!.id).every((x) => x.revoked_at !== null));
  // Leave: the leaving owner's codes are revoked (succession hands the circle on).
  const c2 = await circle(w, B!, "Second");
  assert.equal((await join(w, A!, await code(w, B!, c2, "editor"))).status, 200);
  const k2 = await code(w, B!, c2, "viewer", { max_uses: 5 });
  assert.equal((await w.json("POST", `/circles/${c2}/leave`, { user: B! })).status, 200);
  assert.deepEqual(owners(w, c2), [A!.id]);
  assert.equal((await join(w, S!, k2)).body.error, "circle_invite_invalid");
  // Defence in depth: an open code whose issuer is no longer the active owner is refused at the join.
  const c3 = await circle(w, B!, "Third");
  assert.equal((await join(w, A!, await code(w, B!, c3, "editor"))).status, 200);
  const k3 = await code(w, B!, c3, "viewer", { max_uses: 5 });
  w.sql.prepare(`UPDATE circle_members SET role = 'editor' WHERE circle_id = ? AND user_id = ?`).run(c3, B!.id);
  w.sql.prepare(`UPDATE circle_members SET role = 'owner' WHERE circle_id = ? AND user_id = ?`).run(c3, A!.id);
  assert.equal((await join(w, T!, k3)).body.error, "circle_invite_invalid");
});

test("33-3: circles_owned (5) holds through a transfer, checked in the transfer's own statement: two transfers to one member at once never take them past it", async () => {
  const w = new World();
  const [Y, X1, X2] = ["Yolanda", "Xan", "Xia"].map((n) => w.user("teacher", n));
  for (let i = 0; i < 4; i++) await circle(w, Y!, `Y${i}`);
  const c1 = await circle(w, X1!, "X1");
  const c2 = await circle(w, X2!, "X2");
  for (const c of [c1, c2]) assert.equal((await join(w, Y!, await code(w, c === c1 ? X1! : X2!, c, "editor"))).status, 200);
  // Y owns 4: the first transfer is fine, the second would make 6.
  w.advance(DAY);
  // (round 34: offers, then the member's accepts; the cap is checked at the accept)
  assert.equal((await w.json("POST", `/circles/${c1}/transfer`, { user: X1!, json: { user_id: Y!.id } })).status, 202);
  assert.equal((await w.json("POST", `/circles/${c2}/transfer`, { user: X2!, json: { user_id: Y!.id } })).status, 202);
  assert.equal((await w.json("POST", `/circles/${c1}/transfer/accept`, { user: Y! })).status, 200);
  const r = await w.json<any>("POST", `/circles/${c2}/transfer/accept`, { user: Y! });
  assert.deepEqual([r.status, r.body.error, r.body.metric, r.body.used, r.body.limit], [422, "circles_limit", "circles_owned", 5, 5]);
  assert.deepEqual(owners(w, c2), [X2!.id], "nothing changed");
  // Interleaved: the second transfer runs inside the first one's request, at each of its statement boundaries.
  for (let k = 0; ; k++) {
    const w2 = new World();
    const [Y2, A1, A2] = ["Y", "A1", "A2"].map((n) => w2.user("teacher", n));
    for (let i = 0; i < 4; i++) await circle(w2, Y2!, `Y${i}`);
    const d1 = await circle(w2, A1!, "A1"), d2 = await circle(w2, A2!, "A2");
    for (const [c, o] of [[d1, A1!], [d2, A2!]] as const) await join(w2, Y2!, await code(w2, o, c, "editor"));
    for (const [c, o] of [[d1, A1!], [d2, A2!]] as const) await w2.json("POST", `/circles/${c}/transfer`, { user: o, json: { user_id: Y2!.id } });
    const db = w2.db as any;
    const prep = db.prepare.bind(db), batch = db.batch.bind(db);
    let n = 0, inner = false;
    const hit = async () => {
      if (inner) return;
      if (n++ === k) {
        inner = true;
        await w2.json("POST", `/circles/${d2}/transfer/accept`, { user: Y2! });
        inner = false;
      }
    };
    const wrap = (s: any): any => {
      const o = Object.create(s);
      o.bind = (...a: unknown[]) => wrap(s.bind(...a));
      for (const m of ["first", "all", "run"]) o[m] = async () => (await hit(), s[m]());
      return o;
    };
    db.prepare = (sql: string) => wrap(prep(sql));
    db.batch = async (ss: unknown[]) => (await hit(), batch(ss));
    try {
      await w2.json("POST", `/circles/${d1}/transfer/accept`, { user: Y2! });
    } finally {
      db.prepare = prep;
      db.batch = batch;
    }
    const owned = w2.q<{ n: number }>(`SELECT COUNT(*) AS n FROM circle_members WHERE user_id = ? AND role = 'owner' AND left_at IS NULL`, Y2!.id)[0]!.n;
    assert.ok(owned <= 5, `boundary ${k}: ${owned} owned`);
    if (k >= n) break;
  }
});

test("33-4: a hold delays the purge of a removed bank and of a closed circle holding a held bank, as an open report does; the admin's release or removal lets it go", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [A, B] = ["Ann", "Bob"].map((n) => w.user("teacher", n));
  const cid = await circle(w, A!);
  assert.equal((await join(w, B!, await code(w, A!, cid, "editor"))).status, 200);
  const s = await send(w, B!, cid, bankOf(withQs(2, "Bob")));
  const bid = s.body.bank_id;
  assert.equal((await w.admin<any>(adm, "POST", "/admin/pool/circle-hold", { bank_id: bid, note: "Looks like a copied publisher test." })).status, 200);
  assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: B! })).status, 200);
  await days(w, 15);
  assert.equal(w.q<any>(`SELECT purged_at FROM circle_banks WHERE id = ?`, bid)[0].purged_at, null, "held: kept for the admin");
  assert.equal(w.q(`SELECT 1 FROM circle_holds WHERE bank_id = ?`, bid).length, 1, "and the hold stays");
  assert.equal((await w.admin<any>(adm, "GET", "/admin/pool/circle-bank", null, `?id=${bid}&basis=report`)).status, 403, "(no report: the hold alone is no basis to open)");
  await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, note: "Checked: the teacher's own material." });
  await days(w, 1);
  assert.ok(w.q<any>(`SELECT purged_at FROM circle_banks WHERE id = ?`, bid)[0].purged_at !== null, "released: purged");
  // A closed circle with a held bank stays until the hold is resolved; an admin removal resolves it.
  const s2 = await send(w, B!, cid, bankOf(withQs(1, "Bob2")));
  await w.admin(adm, "POST", "/admin/pool/circle-hold", { bank_id: s2.body.bank_id, note: "Second look needed here." });
  assert.equal((await w.json("DELETE", `/circles/${cid}`, { user: A! })).status, 200);
  await days(w, 15);
  assert.equal(w.q<any>(`SELECT purged_at FROM circles WHERE id = ?`, cid)[0].purged_at, null);
  assert.equal((await w.admin<any>(adm, "POST", "/admin/pool/circle-remove", { bank_id: s2.body.bank_id, note: "Removed after review." })).status, 200);
  await days(w, 15);
  assert.ok(w.q<any>(`SELECT purged_at FROM circles WHERE id = ?`, cid)[0].purged_at !== null);
});

test("33-5: only the owner undoes the owner's removal of a bank; the author's own removal is undone by the author or the owner", async () => {
  const w = new World();
  const [A, E] = ["Ann", "Eli"].map((n) => w.user("teacher", n));
  const cid = await circle(w, A!);
  assert.equal((await join(w, E!, await code(w, A!, cid, "editor"))).status, 200);
  const bid = (await send(w, E!, cid, bankOf(withQs(1, "Eli")))).body.bank_id;
  assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: A! })).status, 200);
  const u = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/undelete`, { user: E! });
  assert.deepEqual([u.status, u.body.error, u.body.needed], [403, "circle_role_required", "owner"]);
  assert.ok(w.q<any>(`SELECT removed_at FROM circle_banks WHERE id = ?`, bid)[0].removed_at !== null);
  assert.equal((await w.json("POST", `/circles/${cid}/banks/${bid}/undelete`, { user: A! })).status, 200);
  assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: E! })).status, 200);
  assert.equal((await w.json("POST", `/circles/${cid}/banks/${bid}/undelete`, { user: E! })).status, 200, "the author's own removal");
});

test("33-6: the public publish check is bounded: one result row per hash that has a hit, at most `circle_memberships_total` circles considered (circles holding records only), whatever the rows an account holds; joins stop at that many memberships, former ones included", async () => {
  // (a) The statement: 1,000 distinct hashes against 500 and 2,000 membership rows of circles holding records.
  const run = (n: number) => {
    const w = new World();
    const A = w.user("teacher", "A"), O = w.user("teacher", "O");
    const hs = Array.from({ length: 1000 }, (_, i) => `${i}`.padStart(64, "a"));
    w.sql.exec("BEGIN");
    const ci = w.sql.prepare(`INSERT INTO circles (id, name, created_by, created_at, updated_at, qkeys) VALUES (?, 'c', ?, 1, 1, 1)`);
    const m = w.sql.prepare(`INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq, left_at, left_reason) VALUES (?, ?, 'viewer', 1, ?, ?, 'left')`);
    const k = w.sql.prepare(`INSERT INTO circle_qkeys (circle_id, match_hash, bank_id, first_version, first_seq, first_by) VALUES (?, ?, ?, 1, ?, ?)`);
    for (let i = 0; i < n; i++) {
      const cid = i.toString(16).padStart(32, "0");
      ci.run(cid, O.id);
      m.run(cid, A.id, i + 10, 1000 + i);
      // every circle holds 10 of the hashes, in 3 banks each
      for (let j = 0; j < 10; j++) for (let b = 0; b < 3; b++) k.run(cid, hs[(i * 10 + j) % 1000]!, `${b}`.padStart(32, "b"), 100 + b + i, O.id);
    }
    w.sql.exec("COMMIT");
    const t = performance.now();
    const rows = w.q(CIRCLE_HITS_SQL, A.id, JSON.stringify(hs), 100);
    const ms = performance.now() - t;
    return { w, rows, ms, A, hs };
  };
  const r500 = run(500), r2000 = run(2000);
  for (const r of [r500, r2000]) {
    assert.ok(r.rows.length <= 1000, `one row per hash at most: ${r.rows.length}`);
    // the 100 most recent memberships (circles 400-499 / 1900-1999) each hold 10 hashes: 1,000 hashes hit
    assert.equal(r.rows.length, 1000);
  }
  assert.ok(r2000.ms < Math.max(3 * r500.ms, 50), `bounded by the cap, not the rows: ${r500.ms.toFixed(1)} -> ${r2000.ms.toFixed(1)} ms`);
  const plan = (r2000.w.sql.prepare(`EXPLAIN QUERY PLAN ${CIRCLE_HITS_SQL}`).all(r2000.A.id, "[]", 100) as { detail: string }[]).map((x) => x.detail);
  // one probe per (hash, circle): the pairs are materialised once (a scalar subquery referenced four times is
  // otherwise run four times), and the memberships come by the account's index, at most the cap
  assert.equal(plan.filter((d) => /SEARCH k USING COVERING INDEX circle_qkeys_first \(circle_id=\? AND match_hash=\?\)/.test(d)).length, 1, plan.join(" | "));
  assert.ok(plan.includes("MATERIALIZE x") && plan.some((d) => /SEARCH m USING INDEX circle_members_user \(user_id=\?\)/.test(d)), plan.join(" | "));
  // (b) The check reserves its results' memory (a busy isolate refuses rather than overcommits).
  let peak = 0;
  const w = r500.w;
  const orig = w.db.prepare.bind(w.db);
  (w.db as any).prepare = (sql: string) => {
    const s = orig(sql) as any;
    if (sql === CIRCLE_HITS_SQL) {
      const b = s.bind.bind(s);
      s.bind = (...a: unknown[]) => {
        const x = b(...a);
        const all = x.all.bind(x);
        x.all = async () => ((peak = Math.max(peak, bytesReserved())), all());
        return x;
      };
    }
    return s;
  };
  await assert.rejects(checkCircleMaterial({ db: w.db } as never, r500.A.id, r500.hs.map((h) => [h]), r500.hs.map(() => false), null), (e: any) => e.code === "circle_material");
  (w.db as any).prepare = orig;
  assert.ok(peak >= 1000 * 256, `reserved ${peak} bytes for 1,000 hashes`);
  // (c) Joins and creates stop at circle_memberships_total (100) rows, former memberships included.
  const w3 = new World();
  const P = w3.user("teacher", "P"), Own = w3.user("teacher", "Own");
  w3.sql.exec("BEGIN");
  for (let i = 0; i < 100; i++) {
    const cid = (i + 5000).toString(16).padStart(32, "0");
    w3.sql.prepare(`INSERT INTO circles (id, name, created_by, created_at, updated_at) VALUES (?, 'c', ?, 1, 1)`).run(cid, Own.id);
    w3.sql.prepare(`INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq, left_at, left_reason) VALUES (?, ?, 'viewer', 1, ?, 2, 'left')`).run(cid, P.id, i + 1);
  }
  w3.sql.exec("COMMIT");
  const c = await circle(w3, Own, "fresh");
  const j = await join(w3, P, await code(w3, Own, c, "viewer"));
  assert.deepEqual([j.status, j.body.error, j.body.metric, j.body.limit], [422, "circles_limit", "circle_memberships_total", 100], JSON.stringify(j.body));
  assert.equal((await w3.json<any>("POST", "/circles", { user: P, json: { name: "mine" } })).body.metric, "circle_memberships_total");
  // A former member re-joining reuses its row: never refused by this cap.
  const old = (5000).toString(16).padStart(32, "0");
  w3.sql.prepare(`INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?, ?, 'owner', 1, 999)`).run(old, Own.id);
  w3.advance(1000);
  assert.equal((await join(w3, P, await code(w3, Own, old, "viewer"))).status, 200);
});

test("33-7: a circle send reserves the publish's heavy-phase memory, dense JSON included (many tiny values cost more than their bytes)", async () => {
  const w = new World();
  const O = w.user("teacher", "Owner");
  const cid = await circle(w, O);
  const dense = JSON.stringify({ schema_version: 2, bank: { id: "local", name: "B", questions: [{ id: "q0", stem: "x", kind: { Short: { sample: "a" } }, points: null, difficulty: null, topics: Array.from({ length: 190_000 }, () => "a"), image: null, explanation: null, figure: null, created: 1, modified: 1 }], created: 1, modified: 1 } });
  const orig = core.publishPrepare.bind(core);
  let seen = -1;
  (core as any).publishPrepare = (...a: any[]) => ((seen = Math.max(seen, bytesReserved())), (orig as any)(...a));
  try {
    await send(w, O, cid, dense);
  } finally {
    (core as any).publishPrepare = orig;
  }
  assert.ok(seen >= 190_000 * 96 * 2, `reserved ${seen} bytes; the publish formula asks ${190_000 * 96 * 2}`);
});

test("33-V1: the activity list shows a bank's title only to who may see the bank (held: the author; removed: the owner and the author)", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [A, E, V] = ["Ann", "Ed", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circle(w, A!);
  for (const [u, r] of [[E!, "editor"], [V!, "viewer"]] as const) assert.equal((await join(w, u, await code(w, A!, cid, r))).status, 200);
  const bid = (await send(w, E!, cid, bankOf(withQs(1, "Ed")))).body.bank_id;
  w.sql.prepare(`UPDATE circle_banks SET title = 'Secret title Jane Doe roster' WHERE id = ?`).run(bid);
  await w.admin(adm, "POST", "/admin/pool/circle-hold", { bank_id: bid, note: "Student names in the title." });
  const titles = async (u: User) => (await w.json<any>("GET", `/circles/${cid}/activity`, { user: u })).body.activity.filter((x: Q) => x.bank_id === bid).map((x: Q) => x.bank_title);
  assert.deepEqual(await titles(V!), [null], "held: hidden from members");
  assert.deepEqual(await titles(A!), [null], "... the owner included");
  assert.deepEqual(await titles(E!), ["Secret title Jane Doe roster"], "the author sees it");
  await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, note: "Title fixed by the author later." });
  assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: E! })).status, 200);
  assert.ok((await titles(V!)).every((t: unknown) => t === null), "removed: hidden from members");
  assert.ok((await titles(A!)).every((t: unknown) => t === "Secret title Jane Doe roster"), "the owner sees a removed bank");
});

test("33-V2: no version under an open report is pruned, and none while the bank is held; the admin can open the reported version after the author's 21 more sends", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [A, R] = ["Ann", "Rae"].map((n) => w.user("teacher", n));
  w.sql.prepare(`UPDATE users SET created_at = ? WHERE id = ?`).run(w.clock - 30 * DAY, R!.id);
  const cid = await circle(w, A!);
  assert.equal((await join(w, R!, await code(w, A!, cid, "viewer"))).status, 200);
  const bid = (await send(w, A!, cid, bankOf([q("Version 1 lists Jane Doe's grade: what is the median?")]))).body.bank_id;
  w.advance(2 * DAY);
  assert.equal((await w.json("POST", `/circles/${cid}/banks/${bid}/report`, { user: R!, json: { reason: "student_data" } })).status, 202);
  assert.equal(w.q(`SELECT 1 FROM circle_holds WHERE bank_id = ?`, bid).length, 1);
  let base = 1;
  for (let v = 2; v <= 22; v++) base = (await send(w, A!, cid, bankOf([q(`Version ${v} without names: what is the median of 4 and 6?`)]), { bid, base })).body.version;
  assert.equal(base, 22);
  assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ? AND status = 'pruned'`, bid).length, 0, "held: nothing pruned");
  await days(w, 1);
  const v1 = await w.admin<any>(adm, "GET", "/admin/pool/circle-bank", null, `?id=${bid}&version=1&basis=report`);
  assert.equal(v1.status, 200, v1.text.slice(0, 200));
  assert.match(JSON.stringify(v1.body.payload), /Jane Doe/);
  // Released: the next send prunes past the 20 kept, except a version an open report still names.
  w.sql.prepare(`DELETE FROM circle_holds WHERE bank_id = ?`).run(bid);
  assert.equal((await send(w, A!, cid, bankOf([q("Version 23 about medians?")]), { bid, base })).status, 200);
  const pruned = w.q<{ version: number }>(`SELECT version FROM circle_versions WHERE bank_id = ? AND status = 'pruned' ORDER BY version`, bid).map((x) => x.version);
  assert.deepEqual(pruned, [2, 3], "v1 is named by an open report and stays");
});

test("33-V3: the owner purges a removed bank at once (no hold or open report): it stops counting toward the circle's caps and records", async () => {
  const w = new World();
  const [A, E] = ["Ann", "Ed"].map((n) => w.user("teacher", n));
  const cid = await circle(w, A!);
  assert.equal((await join(w, E!, await code(w, A!, cid, "editor"))).status, 200);
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'circle_banks', 3, 0, 'adm')`).run();
  const junk: string[] = [];
  for (let i = 0; i < 3; i++) junk.push((await send(w, E!, cid, bankOf(withQs(2, `junk${i}`)))).body.bank_id);
  assert.equal((await send(w, A!, cid, bankOf(withQs(1, "Ann")))).body.metric, "circle_banks");
  assert.equal((await w.json("POST", `/circles/${cid}/members/remove`, { user: A!, json: { user_id: E!.id } })).status, 200);
  for (const b of junk) assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${b}`, { user: A! })).status, 200);
  assert.equal((await send(w, A!, cid, bankOf(withQs(1, "Ann")))).body.metric, "circle_banks", "removed banks still count");
  // Only the owner, only a removed bank.
  const p0 = await w.json<any>("POST", `/circles/${cid}/banks/${junk[0]}/purge`, { user: E! });
  assert.equal(p0.status, 404);
  for (const b of junk) assert.equal((await w.json("POST", `/circles/${cid}/banks/${b}/purge`, { user: A! })).status, 200);
  assert.equal(w.q<any>(`SELECT qkeys FROM circles WHERE id = ?`, cid)[0].qkeys, 0, "records released");
  assert.deepEqual(w.r2.keys(`circles/${cid}/`), []);
  assert.equal((await send(w, A!, cid, bankOf(withQs(1, "Ann")))).status, 201);
  const live = (await send(w, A!, cid, bankOf(withQs(1, "Ann2")))).body.bank_id;
  assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${live}/purge`, { user: A! })).body.error, "bank_not_removed");
});
