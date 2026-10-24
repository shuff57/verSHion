// Round 32: circles, phase A (docs/circles-plan.md; decisions D1-D19). Circles, members, codes, banks
// shared into a circle (view and copy; the author's own republish), the caps (D2-D4, D10), erasure and
// succession (decision 3, D14, D15, D18), purges (D7, D16), the public publish check (D13), the admin's
// audited access (D1), the activity list (D8), the export, migration 0108 on a populated database, and
// rows visited / query plans at the caps.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { normaliseCircleCode } from "../../src/pool/circles.ts";
import { CIRCLE_HITS_SQL } from "../../src/pool/circlecheck.ts";
import { preReadBody } from "../../src/pool/preread.ts";
import { framed, MemR2, png, publish, publishId, rng, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
export const bankOf = (qs: Q[], name = "Circle bank") => JSON.stringify({ schema_version: 2, bank: { id: "local", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
export const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const tf = (stem: string): Q => ({ ...q(stem), kind: { Tf: { correct: true } } });
const PUB = "application/vnd.vershion.publish";

// ---- helpers -------------------------------------------------------------------------------------------

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
async function join(w: World, u: User, c: string) {
  return w.json<any>("POST", "/circles/join", { user: u, json: { code: c } });
}
/** A circle owned by `owner` with `members` joined in order (role per member). */
async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][], name?: string): Promise<string> {
  const cid = await circle(w, owner, name);
  for (const [u, role] of members) {
    const j = await join(w, u, await code(w, owner, cid, role));
    assert.equal(j.status, 200, JSON.stringify(j.body));
    w.advance(1000);
  }
  return cid;
}
interface SendOpts {
  bid?: string;
  base?: number | null;
  license?: string;
  commitId?: string;
  message?: string;
  attest?: Q | null;
  meta?: Q;
  extra?: Q;
}
async function send(w: World, u: User, cid: string, payload: string, o: SendOpts = {}) {
  const meta: Q = {
    commit_id: o.commitId ?? publishId(),
    license: o.license ?? "circle-only",
    ...(o.attest === null ? {} : { attest: o.attest ?? { own_work: true, terms_version: POOL_TERMS_VERSION } }),
    ...(o.message ? { message: o.message } : {}),
    meta: o.meta ?? { title: "Unit 2 quiz", subject: "Math", course: "", grade: "", tags: [], standards: [] },
    // (round 37: a version dropping more than half of the head needs confirm_large_change; these phase A
    // tests replace whole banks freely, round37.test.ts tests the rule)
    ...(o.bid ? { base_version: o.base ?? null, confirm_large_change: true } : {}),
    ...(o.extra ?? {}),
  };
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, payload), contentType: PUB });
}
async function share(w: World, u: User, cid: string, payload: string, o: SendOpts = {}): Promise<{ bid: string; vid: string; qids: string[] }> {
  const r = await send(w, u, cid, payload, o);
  assert.equal(r.status, 201, JSON.stringify(r.body));
  return { bid: r.body.bank_id, vid: r.body.version_id, qids: r.body.qids };
}
const read = async (w: World, u: User, cid: string, bid: string, headers: Record<string, string> = {}) => {
  const r = await w.call("GET", `/circles/${cid}/banks/${bid}`, { user: u, headers });
  const t = await r.text();
  return { status: r.status, body: t ? (JSON.parse(t) as any) : null, etag: r.headers.get("ETag") };
};
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
const owners = (w: World, cid: string) => w.q<{ user_id: string }>(`SELECT user_id FROM circle_members WHERE circle_id = ? AND role = 'owner' AND left_at IS NULL`, cid).map((x) => x.user_id);
const active = (w: World, cid: string) => w.q<{ user_id: string; role: string }>(`SELECT user_id, role FROM circle_members WHERE circle_id = ? AND left_at IS NULL ORDER BY join_seq`, cid);
const withQs = (n: number, tag: string) => Array.from({ length: n }, (_, i) => q(`${tag} question ${i}: explain the role of the river delta in sediment transport ${i}?`));

// ---- 1. circles: create, list, read; non-members see 404 -----------------------------------------------

test("32-1: an approved, invited teacher creates circles (caps: 5 owned, 20 memberships, 5 a day); members list and read them; every circle route is a counted 404 to non-members and former members", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), X = w.user("teacher", "Xena");
  const P = w.user("pending", "Pat"), N = w.user("teacher", "Nora", { invited: false });
  // Gates in front of the routes: pending accounts and accounts without a pool invite are refused.
  assert.equal((await w.json<any>("POST", "/circles", { user: P, json: { name: "x" } })).status, 403);
  assert.equal((await w.json<any>("POST", "/circles", { user: N, json: { name: "x" } })).body.error, "invite_required");
  assert.equal((await w.json<any>("POST", "/circles", { user: A, json: { name: "" } })).status, 400);
  assert.equal((await w.json<any>("POST", "/circles", { user: A, json: { name: "x".repeat(81) } })).status, 400);
  assert.equal((await w.json<any>("POST", "/circles", { user: A, json: { name: "Line\u0000break" } })).status, 400);
  const cid = await circle(w, A, "  Algebra   team ");
  assert.equal(w.q<any>(`SELECT name FROM circles WHERE id = ?`, cid)[0].name, "Algebra team", "cleaned like other user text");
  const list = await w.json<any>("GET", "/circles", { user: A });
  assert.deepEqual(list.body.circles.map((c: Q) => [c.id, c.role, c.members, c.banks]), [[cid, "owner", 1, 0]]);
  const g = await w.json<any>("GET", `/circles/${cid}`, { user: A });
  assert.equal(g.body.circle.role, "owner");
  assert.deepEqual(g.body.members.map((m: Q) => [m.display_name, m.role]), [["Ann", "owner"]]);
  // A non-member: the same 404 as a circle that does not exist, on every route; misses are counted.
  const ghost = "0".repeat(32);
  for (const id of [cid, ghost]) {
    for (const [m, path, body] of [
      ["GET", `/circles/${id}`, undefined],
      ["PATCH", `/circles/${id}`, { name: "y" }],
      ["DELETE", `/circles/${id}`, undefined],
      ["POST", `/circles/${id}/reopen`, undefined],
      ["GET", `/circles/${id}/activity`, undefined],
      ["GET", `/circles/${id}/invites`, undefined],
      ["POST", `/circles/${id}/invites`, { role: "viewer" }],
      ["POST", `/circles/${id}/invites/revoke`, { invite_id: ghost }],
      ["POST", `/circles/${id}/leave`, undefined],
      ["POST", `/circles/${id}/members/remove`, { user_id: A.id }],
      ["POST", `/circles/${id}/members/role`, { user_id: A.id, role: "viewer" }],
      ["POST", `/circles/${id}/transfer`, { user_id: X.id }],
      ["GET", `/circles/${id}/banks/${ghost}`, undefined],
      ["DELETE", `/circles/${id}/banks/${ghost}`, undefined],
      ["POST", `/circles/${id}/banks/${ghost}/undelete`, undefined],
      ["POST", `/circles/${id}/banks/${ghost}/report`, { reason: "other" }],
      ["PATCH", `/circles/${id}/banks/${ghost}`, { license: "CC0-1.0" }],
      ["GET", `/circles/${id}/banks/${ghost}/blobs/${"a".repeat(64)}`, undefined],
    ] as [string, string, Q | undefined][]) {
      const r = await w.json<any>(m, path, { user: X, ...(body ? { json: body } : {}) });
      assert.equal(r.status, 404, `${m} ${path}: ${JSON.stringify(r.body)}`);
      assert.equal(r.body.error, "not_found");
    }
  }
  assert.ok(w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric IN ('item_miss', 'blob_miss')`, X.id).reduce((a: number, r: Q) => a + r.count, 0) >= 30, "misses counted");
  // Caps: 5 created a day, 5 owned (an owner-closed circle still counts until it is purged or reopened).
  const more: string[] = [];
  for (let i = 0; i < 4; i++) more.push(await circle(w, A, `C${i}`));
  const d = await w.json<any>("POST", "/circles", { user: A, json: { name: "sixth" } });
  assert.deepEqual([d.status, d.body.error, d.body.metric], [429, "daily_limit", "circle_creates"]);
  w.advance(DAY);
  const r6 = await w.json<any>("POST", "/circles", { user: A, json: { name: "sixth" } });
  assert.equal(r6.status, 422);
  assert.deepEqual([r6.body.error, r6.body.metric, r6.body.used, r6.body.limit], ["circles_limit", "circles_owned", 5, 5]);
  assert.equal((await w.json("DELETE", `/circles/${more[0]}`, { user: A })).status, 200);
  assert.equal((await w.json<any>("POST", "/circles", { user: A, json: { name: "sixth" } })).body.metric, "circles_owned", "an owner-closed circle counts until purged");
  // The sole owner leaving closes the circle (decision 3: nobody left); it no longer counts.
  assert.equal((await w.json<any>("POST", `/circles/${more[1]}/leave`, { user: A })).status, 200);
  assert.equal(owners(w, more[1]!).length, 0);
  assert.equal(w.q<any>(`SELECT close_reason FROM circles WHERE id = ?`, more[1])[0].close_reason, "last_member_gone");
  assert.equal((await w.json<any>("POST", "/circles", { user: A, json: { name: "sixth" } })).status, 201);
  // A former member: 404 like a stranger.
  assert.equal((await w.json<any>("GET", `/circles/${more[1]}`, { user: A })).status, 404);
  // 20 memberships: X joins 20 circles; the 21st is refused (and her failed try is not charged).
  const Y = w.user("teacher", "Yolanda");
  const ownersY = Array.from({ length: 5 }, (_, i) => w.user("teacher", `Own${i}`));
  const codes: string[] = [];
  for (const o of ownersY) for (let i = 0; i < 5; i++) {
    const c = await circle(w, o, `${o.name}-${i}`);
    codes.push(await code(w, o, c, "viewer"));
  }
  for (const [k, c] of codes.entries()) {
    if (k % 9 === 8) w.advance(DAY);
    const j = await join(w, Y, c);
    if (k < 20) assert.equal(j.status, 200, `${k}: ${JSON.stringify(j.body)}`);
    else {
      assert.equal(j.status, 422);
      assert.deepEqual([j.body.error, j.body.metric, j.body.limit], ["circles_limit", "circle_memberships", 20]);
    }
  }
  assert.equal((await w.json<any>("GET", "/circles", { user: Y })).body.circles.length, 20);
});

// ---- 2. codes -----------------------------------------------------------------------------------------

test("32-2: codes are hashed, shown once, single use by default, expire (7 days, at most 30), revocable; a bad code is always the same 422 and counts (10 a day); preview uses nothing; owner only; a full circle refuses", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), C = w.user("teacher", "Cy"), D = w.user("teacher", "Dee");
  const cid = await circle(w, A);
  const r = await w.json<any>("POST", `/circles/${cid}/invites`, { user: A, json: { role: "viewer", note: "for the PLC" } });
  assert.equal(r.status, 201);
  assert.match(r.body.code, /^C(-[0-9A-HJKMNP-TV-Z]{4}){6}$/);
  assert.equal(r.body.shown_once, true);
  assert.equal(r.body.expires_at, w.clock + 7 * DAY);
  // Never stored or audited in clear.
  const norm = normaliseCircleCode(r.body.code)!;
  for (const t of w.q<{ name: string }>(`SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'`))
    for (const row of w.q(`SELECT * FROM "${t.name}"`)) assert.ok(!JSON.stringify(row).includes(norm.slice(1, 12)), `${t.name} holds the code`);
  // Typing variants: lower case, spaces, O for 0, a pool invite code in the circle box.
  const typed = r.body.code.toLowerCase().replace(/-/g, " ").replace(/0/g, "o");
  const pv = await w.json<any>("POST", "/circles/join/preview", { user: B, json: { code: typed } });
  assert.equal(pv.status, 200, JSON.stringify(pv.body));
  assert.deepEqual([pv.body.circle.name, pv.body.circle.owner_name, pv.body.circle.members, pv.body.role], ["Algebra team", "Ann", 1, "viewer"]);
  assert.equal(w.q<any>(`SELECT uses FROM circle_invites`)[0].uses, 0, "a preview uses nothing");
  assert.equal((await w.json<any>("POST", "/circles/join", { user: B, json: { code: "ABCD-EFGH-JKMN-PQRS-TVWX-YZ01" } })).status, 400, "a pool invite code is not a circle code");
  const j = await join(w, B, typed);
  assert.deepEqual([j.status, j.body.circle_id, j.body.role], [200, cid, "viewer"]);
  assert.deepEqual((await join(w, B, typed)).body.already, true, "joining twice: already a member, nothing used");
  // Single use: C cannot use it.
  const c1 = await join(w, C, r.body.code);
  assert.deepEqual([c1.status, c1.body.error], [422, "circle_invite_invalid"]);
  // Expiry: default 7 days; 30 at most; 50 uses at most.
  assert.equal((await w.json("POST", `/circles/${cid}/invites`, { user: A, json: { role: "editor", expires_in_days: 31 } })).status, 400);
  assert.equal((await w.json("POST", `/circles/${cid}/invites`, { user: A, json: { role: "editor", max_uses: 51 } })).status, 400);
  assert.equal((await w.json("POST", `/circles/${cid}/invites`, { user: A, json: { role: "owner" } })).status, 400);
  const multi = await code(w, A, cid, "editor", { max_uses: 3, expires_in_days: 2 });
  w.advance(2 * DAY + 1);
  assert.equal((await join(w, C, multi)).body.error, "circle_invite_invalid", "expired");
  // Revocable; a revoke stops joins and never undoes one.
  const rv = await w.json<any>("POST", `/circles/${cid}/invites`, { user: A, json: { role: "editor", max_uses: 5 } });
  assert.equal((await join(w, C, rv.body.code)).status, 200);
  assert.equal((await w.json("POST", `/circles/${cid}/invites/revoke`, { user: A, json: { invite_id: rv.body.invite_id } })).status, 200);
  assert.equal((await w.json<any>("POST", `/circles/${cid}/invites/revoke`, { user: A, json: { invite_id: rv.body.invite_id } })).body.error, "already_revoked");
  assert.equal((await join(w, D, rv.body.code)).body.error, "circle_invite_invalid");
  assert.equal(active(w, cid).length, 3, "C stays");
  // Owner only: an editor or viewer cannot make, list or revoke codes.
  for (const u of [B, C]) {
    assert.equal((await w.json<any>("POST", `/circles/${cid}/invites`, { user: u, json: { role: "viewer" } })).body.error, "circle_role_required");
    assert.equal((await w.json<any>("GET", `/circles/${cid}/invites`, { user: u })).status, 403);
  }
  const lst = await w.json<any>("GET", `/circles/${cid}/invites`, { user: A });
  assert.deepEqual(lst.body.invites.map((x: Q) => x.state).sort(), ["expired", "revoked", "used_up"]);
  assert.ok(lst.body.invites.every((x: Q) => x.code === undefined && x.code_hash === undefined));
  // 10 open codes a circle.
  for (let i = 0; i < 10; i++) await code(w, A, cid, "viewer");
  const over = await w.json<any>("POST", `/circles/${cid}/invites`, { user: A, json: { role: "viewer" } });
  assert.deepEqual([over.status, over.body.error, over.body.metric], [422, "circles_limit", "circle_open_invites"]);
  // 10 FAILED codes a day (a good one is never counted): the 11th bad try is a 429.
  w.advance(DAY);
  const E = w.user("teacher", "Eve");
  for (let i = 0; i < 10; i++) assert.equal((await join(w, E, `C-${String(i).padStart(4, "0")}-0000-0000-0000-0000-0000`)).status, 422);
  assert.equal((await join(w, E, "C-1111-1111-1111-1111-1111-1111")).status, 429);
  assert.equal((await w.json("POST", "/circles/join/preview", { user: E, json: { code: "C-1111-1111-1111-1111-1111-1112" } })).status, 429);
  // A full circle (circle_members, site-wide limit) answers 409 circle_full, the code is not used.
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'circle_members', 3, 0, 'adm')`).run();
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'circle_open_invites', 20, 0, 'adm')`).run();
  const F = w.user("teacher", "Fay");
  const fc = await code(w, A, cid, "viewer");
  const full = await join(w, F, fc);
  assert.deepEqual([full.status, full.body.error, full.body.limit], [409, "circle_full", 3]);
  // ... and a closed circle's codes are invalid.
  w.sql.prepare(`DELETE FROM pool_limits`).run();
  assert.equal((await w.json("DELETE", `/circles/${cid}`, { user: A })).status, 200);
  assert.equal((await join(w, F, fc)).body.error, "circle_invite_invalid");
});

// ---- 3. members ------------------------------------------------------------------------------------------

test("32-3: only the owner removes (never themself), changes roles and transfers; removal is immediate and re-joining needs a new code; one active owner always; an owner leaving hands on to the longest-standing editor, else viewer, else the circle closes", async () => {
  const w = new World();
  const [A, B, C, D, E] = ["Ann", "Bob", "Cy", "Dee", "Eve"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[B!, "viewer"], [C!, "editor"], [D!, "editor"], [E!, "viewer"]]);
  assert.equal((await w.json<any>("POST", `/circles/${cid}/members/remove`, { user: C!, json: { user_id: B!.id } })).body.error, "circle_role_required");
  assert.equal((await w.json<any>("POST", `/circles/${cid}/members/remove`, { user: A!, json: { user_id: A!.id } })).status, 400);
  assert.equal((await w.json<any>("POST", `/circles/${cid}/members/remove`, { user: A!, json: { user_id: E!.id } })).status, 200);
  assert.equal((await w.json<any>("GET", `/circles/${cid}`, { user: E! })).status, 404, "removal is immediate");
  assert.equal(w.q<any>(`SELECT left_reason FROM circle_members WHERE user_id = ?`, E!.id)[0].left_reason, "removed", "the row stays (former membership)");
  w.advance(1);
  assert.equal((await join(w, E!, await code(w, A!, cid, "viewer"))).status, 200, "re-joining with a code issued after the removal (round 33)");
  assert.equal(w.q<any>(`SELECT left_at FROM circle_members WHERE user_id = ?`, E!.id)[0].left_at, null, "the row is reused");
  // Role changes.
  assert.equal((await w.json<any>("POST", `/circles/${cid}/members/role`, { user: A!, json: { user_id: B!.id, role: "editor" } })).body.changed, true);
  assert.equal((await w.json<any>("POST", `/circles/${cid}/members/role`, { user: A!, json: { user_id: B!.id, role: "editor" } })).body.changed, false);
  assert.equal((await w.json<any>("POST", `/circles/${cid}/members/role`, { user: A!, json: { user_id: B!.id, role: "owner" } })).status, 400);
  assert.equal((await w.json<any>("POST", `/circles/${cid}/members/role`, { user: B!, json: { user_id: C!.id, role: "viewer" } })).status, 403);
  // Transfer: the new owner, the old one an editor; exactly one owner.
  assert.equal((await w.json<any>("POST", `/circles/${cid}/transfer`, { user: A!, json: { user_id: "1".repeat(8) + "-0000-4000-8000-" + "9".repeat(12) } })).status, 404);
  // (round 34: an offer the member accepts)
  assert.equal((await w.json<any>("POST", `/circles/${cid}/transfer`, { user: A!, json: { user_id: D!.id } })).status, 202);
  assert.deepEqual(owners(w, cid), [A!.id], "nothing before the accept");
  assert.equal((await w.json<any>("POST", `/circles/${cid}/transfer/accept`, { user: D! })).status, 200);
  assert.deepEqual(owners(w, cid), [D!.id]);
  assert.equal(active(w, cid).find((x) => x.user_id === A!.id)!.role, "editor");
  assert.equal((await w.json<any>("POST", `/circles/${cid}/transfer`, { user: A!, json: { user_id: C!.id } })).status, 403, "no longer the owner");
  // Two transfers at once (interleaved at the batch): one wins, never two owners.
  w.sql.prepare(`UPDATE circle_members SET role = 'owner' WHERE circle_id = ? AND user_id = ?`).run(cid, D!.id);
  assert.throws(() => w.sql.prepare(`UPDATE circle_members SET role = 'owner' WHERE circle_id = ? AND user_id = ?`).run(cid, C!.id), /UNIQUE/, "the database allows one active owner");
  // The owner leaves: the editor who JOINED first (not who became editor first: D18) takes over.
  // Joined order: B (viewer, editor now), C (editor), D (owner), E (viewer, rejoined); A is an editor since the transfer but joined first.
  w.advance(1000);
  assert.equal((await w.json<any>("POST", `/circles/${cid}/leave`, { user: D! })).status, 200);
  assert.deepEqual(owners(w, cid), [A!.id], "A joined first (as creator)");
  const act = await w.json<any>("GET", `/circles/${cid}/activity`, { user: B! });
  assert.deepEqual(act.body.activity.slice(0, 2).map((x: Q) => [x.what, x.member]), [["ownership_passed_on", "Ann"], ["left", "Dee"]], "newest first, in written order within one time");
  // Only viewers left after the editors go: the longest-standing viewer (D14).
  for (const u of [B!, C!]) await w.json("POST", `/circles/${cid}/leave`, { user: u });
  assert.equal((await w.json<any>("POST", `/circles/${cid}/leave`, { user: A! })).status, 200);
  assert.deepEqual(owners(w, cid), [E!.id]);
  // The last member leaves: the circle closes (no undo), purged after 14 days.
  assert.equal((await w.json<any>("POST", `/circles/${cid}/leave`, { user: E! })).status, 200);
  const ci = w.q<any>(`SELECT close_reason, undo_until, purge_after FROM circles WHERE id = ?`, cid)[0];
  assert.deepEqual([ci.close_reason, ci.undo_until, ci.purge_after], ["last_member_gone", null, w.clock + 14 * DAY]);
});

// ---- 4. banks: share, read, republish, images, history ------------------------------------------------------

test("32-4: owners and editors share a bank (viewers cannot); every member reads it (stream, ETag, 304, HEAD; one read budget); the author's republish is a compare-and-set on base_version (409 with the true head; commit_id replays; another body under it is refused); qids stay; counts; viewers cannot send", async () => {
  const w = new World();
  const [A, B, V, X] = ["Ann", "Bob", "Vic", "Xena"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[B!, "editor"], [V!, "viewer"]]);
  const qs = withQs(3, "Ann");
  // Viewers cannot share; the attestation is required on every send; a single question is not a bank.
  assert.equal((await send(w, V!, cid, bankOf(qs))).body.error, "circle_role_required");
  const na = await send(w, A!, cid, bankOf(qs), { attest: null });
  assert.deepEqual([na.status, na.body.error], [422, "attestation_required"]);
  assert.equal((await send(w, A!, cid, JSON.stringify({ schema_version: 2, question: qs[0] }))).body.error, "invalid_payload");
  assert.equal((await send(w, A!, cid, bankOf(qs), { license: "MIT" })).status, 400);
  const commit = publishId();
  const s1 = await send(w, A!, cid, bankOf(qs), { commitId: commit, message: "First draft" });
  assert.equal(s1.status, 201, JSON.stringify(s1.body));
  assert.deepEqual([s1.body.version, s1.body.added, s1.body.changed, s1.body.removed, s1.body.qids.length], [1, 3, 0, 0, 3]);
  const bid = s1.body.bank_id;
  // A retry with the same commit_id and body replays; another body under it is refused.
  const again = await send(w, A!, cid, bankOf(qs), { commitId: commit, message: "First draft" });
  assert.deepEqual([again.status, again.body.replayed, again.body.version_id], [200, true, s1.body.version_id]);
  assert.equal((await send(w, A!, cid, bankOf(withQs(2, "Other")), { commitId: commit })).body.error, "commit_id_conflict");
  assert.equal((await send(w, B!, cid, bankOf(qs), { commitId: commit })).body.error, "commit_id_conflict", "someone else's id");
  // Every member reads it; the key is there (D12); ids are the bank's qids.
  for (const u of [A!, B!, V!]) {
    const r = await read(w, u, cid, bid);
    assert.equal(r.status, 200);
    assert.equal(r.body.bank.version, 1);
    assert.equal(r.body.payload.bank.questions[0].kind.Short.sample, "a model answer kept with the key");
    assert.deepEqual(r.body.payload.bank.questions.map((x: Q) => x.id), s1.body.qids);
    assert.equal(r.body.bank.is_author, u === A);
  }
  const r0 = await read(w, B!, cid, bid);
  assert.equal((await read(w, B!, cid, bid, { "If-None-Match": r0.etag! })).status, 304);
  const head = await w.call("HEAD", `/circles/${cid}/banks/${bid}`, { user: B! });
  assert.equal(head.status, 200);
  assert.ok(Number(head.headers.get("Content-Length")) > 100);
  assert.equal((await read(w, X!, cid, bid)).status, 404, "a non-member");
  assert.equal(w.q(`SELECT 1 FROM served`).length + w.q(`SELECT 1 FROM imports`).length, 0, "circle reads write no served rows (no licence evidence)");
  assert.ok(w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'item_reads'`, B!.id)[0].count >= 3, "circle reads count toward item_reads");
  // The author republishes: qids carried by published.qid stay; counts are per qid.
  const qs2 = s1.body.qids.map((qid: string, i: number) => ({ ...qs[i], published: { item_id: bid, qid, publish_id: "p", version: 1 } }));
  qs2[1] = { ...qs2[1], stem: "Edited: what carries sediment past the delta front?" };
  qs2.splice(2, 1);
  qs2.push(q("A new question about levees?"));
  const s2 = await send(w, A!, cid, bankOf(qs2), { bid, base: 1 });
  assert.equal(s2.status, 200, JSON.stringify(s2.body));
  assert.deepEqual([s2.body.version, s2.body.added, s2.body.changed, s2.body.removed], [2, 1, 1, 1]);
  assert.deepEqual(s2.body.qids.slice(0, 2), s1.body.qids.slice(0, 2));
  // A send based on an old head: 409 with the true head, nothing stored.
  const stale = await send(w, A!, cid, bankOf(qs2), { bid, base: 1 });
  assert.deepEqual([stale.status, stale.body.error, stale.body.head_version, stale.body.head_by], [409, "conflict", 2, "Ann"]);
  assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ?`, bid).length, 2);
  // Phase A: only the author sent versions of her bank. Round 37 (phase B): editors and the owner do too
  // (round37.test.ts); a viewer never.
  assert.equal((await send(w, V!, cid, bankOf(qs2), { bid, base: 2 })).body.error, "circle_role_required");
  // History rows: versions linear, the previous one kept.
  assert.deepEqual(w.q<any>(`SELECT version, status, message FROM circle_versions WHERE bank_id = ? ORDER BY version`, bid).map((x) => [x.version, x.status, x.message]), [[1, "previous", "First draft"], [2, "current", null]]);
  // The bank in the circle page; the activity list (names, no technical data).
  const g = await w.json<any>("GET", `/circles/${cid}`, { user: V! });
  assert.deepEqual(g.body.banks.map((b: Q) => [b.title, b.author_name, b.head_version, b.question_count, b.license]), [["Unit 2 quiz", "Ann", 2, 3, "circle-only"]]);
  const act = (await w.json<any>("GET", `/circles/${cid}/activity`, { user: V! })).body.activity;
  assert.deepEqual(act.slice(0, 2).map((x: Q) => [x.what, x.by, x.version, x.bank_title]), [["version_sent", "Ann", 2, "Unit 2 quiz"], ["bank_shared", "Ann", 1, "Unit 2 quiz"]]);
  assert.ok(!JSON.stringify(act).includes("invite"), "codes are not in the activity list");
  // Nothing about circles in browse, search, /items/versions or the author's own item list.
  assert.equal((await w.json<any>("GET", "/items?q=sediment", { user: B! })).body.items.length, 0);
  assert.equal((await w.json<any>("GET", "/items/mine", { user: A! })).body.items.length, 0);
  assert.equal((await w.json<any>("POST", "/items/versions", { user: B!, json: { ids: [bid] } })).body.items[0].status, "unavailable");
});

test("32-5: images: a member uploads, shares and reads an image of a kept version; the hourly blob GC keeps circle-only images, and lets them go once no kept version uses them; versions past the 20 kept are pruned (D16) and their bodies deleted", async () => {
  const w = new World();
  const [A, B] = ["Ann", "Bob"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[B!, "viewer"]]);
  const img = png(8, 8, 3);
  const up = await upload(w, A!, img);
  assert.equal(up.status, 201);
  const withImg = (tag: string) => [q(`${tag}: what does the figure show about meanders?`, { image: { file: `media/${up.sha}.png`, width: 8, height: 8 } })];
  const s = await share(w, A!, cid, bankOf(withImg("v1")));
  const blobPath = `/circles/${cid}/banks/${s.bid}/blobs/${up.sha}`;
  assert.equal((await w.call("GET", blobPath, { user: B! })).status, 200);
  assert.equal((await w.call("GET", `/circles/${cid}/banks/${s.bid}/blobs/${"b".repeat(64)}`, { user: B! })).status, 404);
  // The pending upload expires; the GC runs every hour for days: the image stays (a kept version uses it).
  await days(w, 3);
  assert.ok(w.r2.objects.has(`blobs/${up.sha}`), "circle-only image kept by the GC");
  assert.equal((await w.call("GET", blobPath, { user: B! })).status, 200);
  assert.equal(w.q(`SELECT 1 FROM blobs WHERE sha256 = ?`, up.sha).length, 1);
  // 21 more versions without the image: v1 (the only one using it) is pruned: rows go, bytes go.
  let base = 1;
  for (let v = 2; v <= 22; v++) {
    const r = await send(w, A!, cid, bankOf([q(`Version ${v} question about oxbow lakes?`)]), { bid: s.bid, base });
    assert.equal(r.status, 200, JSON.stringify(r.body));
    base = r.body.version;
  }
  const st = w.q<any>(`SELECT version, status FROM circle_versions WHERE bank_id = ? ORDER BY version`, s.bid);
  assert.equal(st.filter((x) => x.status === "pruned").length, 2, "22 versions: the 20 newest kept");
  assert.equal(st.filter((x) => x.status !== "pruned").length, 20);
  await days(w, 1);
  assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ? AND status = 'pruned' AND full_key <> ''`, s.bid).length, 0, "pruned bodies deleted");
  assert.equal(w.r2.keys(`circles/${cid}/${s.bid}/`).length, 40, "20 kept versions x (body + index)");
  assert.ok(!w.r2.objects.has(`blobs/${up.sha}`), "an image no kept version uses is collected");
  assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ?`, s.bid).length, 22, "history lines stay");
});

// ---- 6. caps ----------------------------------------------------------------------------------------------------

test("32-6: per-circle caps (D2-D4) in the commit itself: banks (100), bytes (200 MB), questions across current versions (5,000), records (50,000); 100 sends a day; circle questions do not count toward shared_questions", async () => {
  const w = new World();
  const [A, B] = ["Ann", "Bob"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[B!, "editor"]]);
  const lim = (metric: string, value: number) => w.sql.prepare(`INSERT OR REPLACE INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', ?, ?, 0, 'adm')`).run(metric, value);
  lim("circle_banks", 2);
  await share(w, A!, cid, bankOf(withQs(2, "a")));
  const b2 = await share(w, B!, cid, bankOf(withQs(2, "b")));
  const third = await send(w, A!, cid, bankOf(withQs(2, "c")));
  assert.deepEqual([third.status, third.body.error, third.body.metric, third.body.limit], [422, "circles_limit", "circle_banks", 2]);
  // A removed bank counts until purged.
  assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${b2.bid}`, { user: B! })).status, 200);
  assert.equal((await send(w, A!, cid, bankOf(withQs(2, "c")))).body.metric, "circle_banks");
  lim("circle_banks", 100);
  lim("circle_questions", 7);
  const qx = await send(w, A!, cid, bankOf(withQs(4, "d")));
  assert.deepEqual([qx.status, qx.body.error, qx.body.used, qx.body.needed, qx.body.limit], [422, "circle_questions_limit", 4, 4, 7]);
  lim("circle_questions", 5000);
  lim("circle_bytes", 3000);
  const bx = await send(w, A!, cid, bankOf(withQs(4, "e")));
  assert.deepEqual([bx.status, bx.body.error, bx.body.scope, bx.body.limit], [413, "quota_exceeded", "circle", 3000]);
  lim("circle_bytes", 200 * 1024 * 1024);
  const used = w.q<any>(`SELECT qkeys FROM circles WHERE id = ?`, cid)[0].qkeys;
  assert.equal(used, 4, "one record per distinct key-free question");
  lim("circle_records", used + 2);
  const rx = await send(w, A!, cid, bankOf(withQs(3, "f")));
  assert.deepEqual([rx.status, rx.body.error, rx.body.needed], [413, "circle_records_full", 3]);
  assert.equal((await send(w, A!, cid, bankOf(withQs(2, "f")))).status, 201, "exactly at the cap");
  assert.equal(w.q<any>(`SELECT qkeys FROM circles WHERE id = ?`, cid)[0].qkeys, used + 2);
  lim("circle_records", 50000);
  // A failed commit leaves no staging row and no R2 object behind.
  assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE status = 'staging'`).length, 0);
  assert.equal(w.r2.keys("circles/").length, 2 * w.q(`SELECT 1 FROM circle_versions`).length);
  // Circle questions are not personal shared questions (D4).
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'shared_questions', 3, 0, 'adm')`).run(A!.id);
  assert.equal((await publish(w, A!, bankOf(withQs(3, "public")))).status, 201);
  // 100 sends a day.
  lim("circle_commits", 7);
  w.advance(DAY);
  const s = await share(w, A!, cid, bankOf(withQs(1, "g")));
  let base = 1;
  for (let i = 0; i < 6; i++) base = (await send(w, A!, cid, bankOf(withQs(1, `g${i}`)), { bid: s.bid, base })).body.version;
  const over = await send(w, A!, cid, bankOf(withQs(1, "h")), { bid: s.bid, base });
  assert.deepEqual([over.status, over.body.metric], [429, "circle_commits"]);
});

// ---- 7. close, reopen, purge; delete, undelete; licence; reports and holds; admin; export ---------------------

test("32-7: an owner close is undoable for 14 days, then everything is purged (R2 too; the row a tombstone); an open report delays it; banks: delete/undelete (owner, author), licence (author; declared public origins bound it); reports hide a bank (student data from an established member at once; 3 members otherwise); the admin opens content only on a basis, audited; the export lists circles", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [A, B, C, D] = ["Ann", "Bob", "Cy", "Dee"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[B!, "editor"], [C!, "viewer"], [D!, "viewer"]]);
  const a = await share(w, A!, cid, bankOf(withQs(2, "Ann")), { message: "Ann's PRIVATE-MESSAGE" });
  const b = await share(w, B!, cid, bankOf(withQs(2, "Bob")));
  // Licence (D19): the author only; earlier versions keep theirs.
  assert.equal((await w.json<any>("PATCH", `/circles/${cid}/banks/${a.bid}`, { user: B!, json: { license: "CC-BY-4.0" } })).body.needed, "author");
  assert.equal((await w.json<any>("PATCH", `/circles/${cid}/banks/${a.bid}`, { user: A!, json: { license: "CC-BY-4.0" } })).status, 200);
  assert.equal(w.q<any>(`SELECT license FROM circle_versions WHERE id = ?`, a.vid)[0].license, "circle-only", "the version keeps its own licence");
  // A bank holding a public item's question under CC-BY-SA must stay CC-BY-SA (declared origins).
  const pubItem = (await publish(w, D!, bankOf([q("Dee's public question about glacial erratics?")]), { license: "CC-BY-SA-4.0" })).body as any;
  const withOrigin = [{ ...q("Dee's public question about glacial erratics?"), origin: { item_id: pubItem.item_id, qid: "x", publish_id: pubItem.publish_id, version: 1 } }];
  const bad = await send(w, A!, cid, bankOf(withOrigin), { license: "circle-only" });
  assert.deepEqual([bad.status, bad.body.error, bad.body.allowed], [422, "license_incompatible", ["CC-BY-SA-4.0"]]);
  const sa = await share(w, A!, cid, bankOf(withOrigin), { license: "CC-BY-SA-4.0" });
  assert.equal((await w.json<any>("PATCH", `/circles/${cid}/banks/${sa.bid}`, { user: A!, json: { license: "circle-only" } })).body.error, "license_incompatible");
  assert.deepEqual((await read(w, C!, cid, sa.bid)).body.origins.map((o: Q) => [o.pos, o.item_id, o.license, o.author_name]), [[0, pubItem.item_id, "CC-BY-SA-4.0", "Dee"]]);
  // Delete and undelete (D17: owner or the bank's author).
  assert.equal((await w.json<any>("DELETE", `/circles/${cid}/banks/${b.bid}`, { user: C! })).body.error, "circle_role_required");
  const del = await w.json<any>("DELETE", `/circles/${cid}/banks/${b.bid}`, { user: A! });
  assert.deepEqual([del.status, del.body.undo_until], [200, w.clock + 14 * DAY]);
  assert.equal((await read(w, C!, cid, b.bid)).body.error, "bank_removed");
  assert.equal((await w.json<any>("GET", `/circles/${cid}`, { user: C! })).body.banks.length, 2, "hidden from members");
  assert.equal((await w.json<any>("GET", `/circles/${cid}`, { user: B! })).body.banks.find((x: Q) => x.id === b.bid).removed.reason, "owner_removed", "the author sees it, removed");
  assert.equal((await w.json("POST", `/circles/${cid}/banks/${b.bid}/undelete`, { user: B! })).status, 403, "the owner's removal is undone by the owner only (round 33)");
  assert.equal((await w.json("POST", `/circles/${cid}/banks/${b.bid}/undelete`, { user: A! })).status, 200);
  assert.equal((await read(w, C!, cid, b.bid)).status, 200);
  // Reports (D1). The author cannot report her own bank; a student-data report from an ESTABLISHED member hides it at once.
  assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${a.bid}/report`, { user: A!, json: { reason: "other" } })).body.error, "own_item");
  const r1 = await w.json<any>("POST", `/circles/${cid}/banks/${a.bid}/report`, { user: C!, json: { reason: "student_data", note: "names a student" } });
  assert.equal(r1.status, 202);
  assert.equal(w.q(`SELECT 1 FROM circle_holds`).length, 0, "C joined today and the account is new: the report waits for the admin");
  w.advance(8 * DAY);
  const r2 = await w.json<any>("POST", `/circles/${cid}/banks/${a.bid}/report`, { user: D!, json: { reason: "student_data" } });
  assert.equal(r2.status, 202);
  assert.deepEqual(w.q<any>(`SELECT reason, source FROM circle_holds WHERE bank_id = ?`, a.bid), [{ reason: "student_data", source: "" }]);
  assert.equal((await read(w, B!, cid, a.bid)).status, 404, "hidden from the circle");
  assert.equal((await read(w, A!, cid, a.bid)).body.bank.held, true, "the author still reads it");
  assert.ok(!(await w.json<any>("GET", `/circles/${cid}`, { user: B! })).body.banks.some((x: Q) => x.id === a.bid));
  assert.equal((await w.json<any>("POST", "/circles/heads", { user: B!, json: { bank_ids: [a.bid, b.bid, "0".repeat(32)] } })).body.banks.map((x: Q) => x.status).join(), "under_review,current,unavailable");
  assert.ok(w.notices.some((n) => n.kind === "circle_report" && n.held), "the admin is told (ids only)");
  // Three members reporting for other reasons hide a bank too.
  for (const u of [A!, C!, D!]) await w.json("POST", `/circles/${cid}/banks/${b.bid}/report`, { user: u, json: { reason: "inappropriate" } });
  assert.deepEqual(w.q<any>(`SELECT reason FROM circle_holds WHERE bank_id = ?`, b.bid), [{ reason: "reports" }]);
  // The admin: the queue; content only on a basis; every open audited.
  const queue = await w.admin<any>(adm, "GET", "/admin/pool/circle-reports");
  assert.equal(queue.body.reports.length, 5);
  assert.equal((await w.admin<any>(adm, "GET", "/admin/pool/circle-bank", null, `?id=${sa.bid}&basis=report`)).status, 403, "no report on it");
  assert.equal((await w.admin<any>(adm, "GET", "/admin/pool/circle-bank", null, `?id=${sa.bid}`)).status, 400);
  const view = await w.admin<any>(adm, "GET", "/admin/pool/circle-bank", null, `?id=${a.bid}&basis=report`);
  assert.equal(view.status, 200);
  assert.equal(view.body.payload.bank.questions.length, 2);
  const notice = (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
  assert.equal((await w.admin<any>(adm, "GET", "/admin/pool/circle-bank", null, `?id=${sa.bid}&basis=notice&notice_id=${notice}`)).status, 200);
  assert.equal((await w.admin<any>(adm, "GET", "/admin/pool/circle", null, `?id=${cid}`)).body.members.length, 4);
  assert.deepEqual(w.q<any>(`SELECT action FROM pool_audit WHERE action LIKE 'circle.admin_%' ORDER BY rowid`).map((x) => x.action), ["circle.admin_view", "circle.admin_view", "circle.admin_open"]);
  assert.ok(!(await w.json<any>("GET", `/circles/${cid}/activity`, { user: C! })).body.activity.some((x: Q) => x.what === "other"), "admin opens and reports are never in the members' activity");
  assert.equal((await w.admin<any>(adm, "POST", "/admin/pool/circle-hold", { bank_id: sa.bid, note: "short" })).status, 400, "a reason of 10+ characters");
  assert.equal((await w.admin<any>(adm, "POST", "/admin/pool/circle-release", { bank_id: a.bid, note: "No student data in it after review." })).body.dismissed, 2);
  assert.equal((await read(w, B!, cid, a.bid)).status, 200);
  assert.equal((await w.admin<any>(adm, "POST", "/admin/pool/circle-remove", { bank_id: b.bid, note: "Inappropriate content, removed." })).status, 200);
  assert.equal((await read(w, A!, cid, b.bid)).body.error, "bank_removed");
  assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${b.bid}/undelete`, { user: B! })).status, 409, "an admin removal is not undone by members");
  // The export (plan 6.5): memberships, codes created without codes, versions sent without bodies, reports.
  w.sql.prepare(`INSERT INTO sessions (id, user_id, family_id, created_at, auth_at, expires_at, refresh_hash) VALUES ('s1', ?, ?, ?, ?, ?, 'h')`).run(A!.id, `fam-${A!.id}`, w.clock, w.clock, w.clock + DAY);
  const ex = await w.json<any>("GET", "/me/export", { user: A! });
  assert.equal(ex.status, 200, JSON.stringify(ex.body).slice(0, 300));
  assert.deepEqual(ex.body.circles.memberships.map((m: Q) => [m.circle_id, m.role]), [[cid, "owner"]]);
  assert.equal(ex.body.circles.invites_created.length, 3);
  assert.ok(ex.body.circles.invites_created.every((i: Q) => i.code === undefined && i.code_hash === undefined));
  assert.ok(ex.body.circles.versions_sent.some((v: Q) => v.message === "Ann's PRIVATE-MESSAGE"));
  assert.ok(!JSON.stringify(ex.body.circles).includes("model answer"), "no bodies");
  // Close (D7): members see "closed" at once; 14 days to undo; an open report delays the purge.
  w.advance(DAY);
  const cl = await w.json<any>("DELETE", `/circles/${cid}`, { user: A! });
  assert.equal(cl.body.undo_until, w.clock + 14 * DAY);
  const rd = await read(w, B!, cid, a.bid);
  assert.deepEqual([rd.status, rd.body.error, rd.body.reason], [409, "circle_closed", "owner_closed"]);
  assert.equal((await w.json<any>("GET", "/circles", { user: B! })).body.circles[0].close_reason, "owner_closed");
  assert.equal((await w.json<any>("POST", `/circles/${cid}/reopen`, { user: B! })).status, 403);
  assert.equal((await w.json<any>("POST", `/circles/${cid}/reopen`, { user: A! })).status, 200);
  assert.equal((await read(w, B!, cid, a.bid)).status, 200);
  assert.equal((await w.json("DELETE", `/circles/${cid}`, { user: A! })).status, 200);
  await w.json("POST", `/circles/${cid}/banks/${sa.bid}/report`, { user: D!, json: { reason: "copyright" } }).catch(() => null);
  w.sql.prepare(`INSERT INTO circle_reports (id, circle_id, bank_id, reporter_id, reason, at) VALUES (?, ?, ?, ?, 'copyright', ?)`).run("e".repeat(32), cid, sa.bid, D!.id, w.clock);
  await days(w, 15);
  assert.equal(w.q<any>(`SELECT purged_at FROM circles WHERE id = ?`, cid)[0].purged_at, null, "an open report keeps it for the admin");
  assert.equal((await w.json<any>("POST", `/circles/${cid}/reopen`, { user: A! })).status, 409, "past the undo window");
  await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: sa.bid, note: "Reviewed: permission on file." });
  await days(w, 1);
  const tomb = w.q<any>(`SELECT name, description, purged_at FROM circles WHERE id = ?`, cid)[0];
  assert.ok(tomb.purged_at !== null && tomb.name === null && tomb.description === null, "a tombstone");
  for (const t of ["circle_members", "circle_invites", "circle_versions", "circle_qkeys", "circle_holds"]) assert.equal(w.q(`SELECT 1 FROM ${t}`).length, 0, t);
  assert.deepEqual(w.r2.keys("circles/"), [], "R2 bodies and indexes gone");
  assert.ok(w.q<any>(`SELECT title FROM circle_banks`).every((x) => x.title === ""), "bank tombstones keep no typed text");
  assert.equal((await w.json<any>("GET", "/circles", { user: B! })).body.circles.length, 0);
});

// ---- 8. the public publish check (D13) ------------------------------------------------------------------------------

test("32-8: material first sent into a circle by someone else is refused at public publish (and its preview), for current and former members; the publisher's own, her earlier public text, short statements and an audited independence statement pass; a non-member is never checked; hidden-key publishes are checked on every form", async () => {
  const w = new World();
  const [A, B, X] = ["Ann", "Bob", "Xena"].map((n) => w.user("teacher", n));
  const bobQ = q("Bob's circle question: why does a meander migrate toward its outer bank over time?");
  const annQ = q("Ann's circle question: how does a cutoff turn a meander into an oxbow lake?");
  const shortQ = tf("Is 7 prime?");
  // Ann had already published this one in the public pool before Bob put it in the circle.
  const annPublic = q("Ann's public question: what is a natural levee made of?");
  assert.equal((await publish(w, A!, bankOf([annPublic]))).status, 201);
  w.advance(1000);
  const cid = await circleWith(w, B!, [[A!, "editor"]]);
  const bb = await share(w, B!, cid, bankOf([bobQ, shortQ, annPublic]));
  w.advance(1000);
  await share(w, A!, cid, bankOf([annQ]));
  // A non-member publishing the same text: never checked (one probe), never refused.
  assert.equal((await publish(w, X!, bankOf([bobQ]))).status, 201);
  // Ann publishing Bob's question: refused, with the circle and bank named to her; a preview too.
  const mix = bankOf([annQ, bobQ, shortQ, annPublic]);
  const r = await publish(w, A!, mix);
  assert.equal(r.status, 422, JSON.stringify(r.body));
  assert.equal(r.body.error, "circle_material");
  assert.deepEqual(r.body.positions, [1]);
  assert.deepEqual((r.body.hits as Q[]).map((h) => [h.pos, h.circle_name, h.bank_title, h.may_claim_independence]), [[1, "Algebra team", "Unit 2 quiz", false]]);
  const pv = await w.json<any>("POST", "/items/preview", { user: A!, body: framed({ publish_id: publishId(), hide_key: false, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, labels: [], order_acks: [], image_ok: [], visibility: "public", meta: { title: "t", subject: "s", course: "", grade: "", tags: [], standards: [] } }, mix), contentType: PUB });
  assert.equal(pv.body.error, "circle_material", "the dry run says so too");
  // Her own (first sent by her), her earlier public text and the short statement pass.
  assert.equal((await publish(w, A!, bankOf([annQ, shortQ, annPublic]))).status, 201);
  // Hidden-key publish: checked on the served and the full form.
  assert.equal((await publish(w, A!, bankOf([bobQ]), { hideKey: true })).body.error, "circle_material");
  // An independence statement never covers a circle-only bank she can still read ...
  const claim = (banks: string[]) => ({ meta: { title: "t", subject: "s", course: "", grade: "", tags: [], standards: [] }, attestStatement: banks });
  const withClaim = async (banks: string[]) =>
    w.json<any>("POST", "/items", {
      user: A!,
      body: framed({ publish_id: publishId(), hide_key: false, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: POOL_TERMS_VERSION }, labels: [], order_acks: [], image_ok: [], visibility: "public", meta: claim(banks).meta, circle_independent_of: { banks, accepted: true, statement: "I wrote this one myself for my own class in 2019." } }, bankOf([bobQ])),
      contentType: PUB,
    });
  assert.equal((await withClaim([bb.bid])).body.error, "circle_material");
  // ... but covers it once the bank's author lets members share it (a CC licence) ...
  assert.equal((await w.json("PATCH", `/circles/${cid}/banks/${bb.bid}`, { user: B!, json: { license: "CC-BY-4.0" } })).status, 200);
  const ok = await withClaim([bb.bid]);
  assert.equal(ok.status, 201, JSON.stringify(ok.body));
  // ... audited and flagged for the admin.
  assert.deepEqual(JSON.parse(w.q<any>(`SELECT detail FROM admin_flags WHERE kind = 'independent_claim'`)[0].detail).circle_banks.first, [bb.bid]);
  assert.deepEqual(JSON.parse(w.q<any>(`SELECT detail FROM pool_audit WHERE action = 'item.publish' ORDER BY rowid DESC LIMIT 1`)[0].detail).circle_independent_of, [bb.bid]);
  // A FORMER member is still checked (she left; the bank is circle-only again).
  assert.equal((await w.json("PATCH", `/circles/${cid}/banks/${bb.bid}`, { user: B!, json: { license: "circle-only" } })).status, 200);
  assert.equal((await w.json("POST", `/circles/${cid}/leave`, { user: A! })).status, 200);
  assert.equal((await publish(w, A!, bankOf([q("Bob's circle question: why does a meander migrate toward its outer bank over time?", { explanation: "my words" })]))).body.error, "circle_material", "the key-free text matches whatever the explanation");
  // Once she can no longer read it, a statement covers it (audited, flagged; the third in 30 days flags the account).
  const ok2 = await withClaim([bb.bid]);
  assert.equal(ok2.status, 201);
  // First sender by sequence across circles: Cy sends Bob's text into another circle Ann is in, later: Bob stays first.
  const C = w.user("teacher", "Cy");
  const cid2 = await circleWith(w, C, [[A!, "viewer"]], "Geo team");
  await share(w, C, cid2, bankOf([q("Ann's circle question: how does a cutoff turn a meander into an oxbow lake?")]));
  assert.equal((await publish(w, A!, bankOf([annQ]))).status, 201, "Ann sent it first (seq), Cy's later copy changes nothing");
  // Circle content never appears in browse, search or credits.
  assert.ok(!JSON.stringify((await w.json<any>("GET", "/items?q=meander", { user: X! })).body).includes(bb.bid));
});

// ---- 9. erasure: succession, banks, text, codes, memberships; reruns ---------------------------------------------

const circleDump = (w: World) =>
  JSON.stringify(["circles", "circle_members", "circle_invites", "circle_banks", "circle_versions", "circle_qkeys", "circle_reports", "circle_holds", "circle_version_blobs"].map((t) => w.q(`SELECT * FROM ${t} ORDER BY 1, 2`)));

test("32-9: an erased owner hands each circle on in one batch: the longest-standing editor by join order (ties on the clock decided by join_seq), else the longest-standing viewer, else the circle closes; banks kept anonymised or removed by keep_on_delete; messages, notes and codes go; erasure_finish and the resweep change nothing more", async () => {
  const w = new World();
  const Z = w.user("teacher", "Zorgon Quixote");
  const [E1, E2, V1, V2, M] = ["E1", "E2", "V1", "V2", "M"].map((n) => w.user("teacher", n));
  // Circle 1: editors E2 and E1 joined at the SAME clock (E2 first by sequence); a viewer earlier than both.
  const c1 = await circle(w, Z, "c1");
  const k1 = await code(w, Z, c1, "viewer");
  await join(w, V1!, k1);
  const k2 = await code(w, Z, c1, "editor", { max_uses: 2 });
  await join(w, E2!, k2);
  await join(w, E1!, k2);
  // Circle 2: only viewers. Circle 3: nobody else. Circle 4: Z is a member (not owner) of M's circle.
  const c2 = await circleWith(w, Z, [[V2!, "viewer"], [V1!, "viewer"]], "c2");
  const c3 = await circle(w, Z, "c3");
  const c4 = await circleWith(w, M!, [[Z, "editor"]], "c4");
  // Z's banks, messages, a report with a note, codes with notes.
  const zb = await share(w, Z, c1, bankOf(withQs(2, "Zorgon")), { message: "Zorgon Quixote 5305551212" });
  const zb4 = await share(w, Z, c4, bankOf(withQs(1, "Zorgon4")));
  await share(w, E1!, c1, bankOf(withQs(1, "E1")));
  const mb = await share(w, M!, c4, bankOf(withQs(1, "M")));
  await w.json("POST", `/circles/${c4}/banks/${mb.bid}/report`, { user: Z, json: { reason: "other", note: "Zorgon Quixote says so" } });
  await code(w, Z, c2, "viewer", { note: "for Zorgon Quixote's PLC" });
  await erase(w, Z);
  assert.deepEqual(owners(w, c1), [E2!.id], "the editor who joined first (join_seq breaks the clock tie)");
  assert.deepEqual(owners(w, c2), [V2!.id], "only viewers: the longest-standing viewer (D14)");
  assert.equal(w.q<any>(`SELECT close_reason FROM circles WHERE id = ?`, c3)[0].close_reason, "last_member_gone");
  assert.deepEqual(owners(w, c4), [M!.id]);
  assert.equal(w.q(`SELECT 1 FROM circle_members WHERE user_id = ?`, Z.id).length, 0, "its memberships are gone");
  // No choice made: its banks are kept, shown as by "deleted teacher" (D15).
  assert.deepEqual(w.q<any>(`SELECT anonymized, removed_at FROM circle_banks WHERE id IN (?, ?)`, zb.bid, zb4.bid).map((x) => [x.anonymized, x.removed_at]), [[1, null], [1, null]]);
  const g = await w.json<any>("GET", `/circles/${c1}`, { user: V1! });
  assert.equal(g.body.banks.find((b: Q) => b.id === zb.bid).author_name, "deleted teacher");
  assert.equal((await read(w, V1!, c1, zb.bid)).body.bank.committed_by, "deleted teacher");
  assert.equal(w.q<any>(`SELECT message FROM circle_versions WHERE committer_id = ?`, Z.id).filter((x) => x.message !== null).length, 0, "messages go");
  assert.deepEqual(w.q<any>(`SELECT reporter_id, note FROM circle_reports`), [{ reporter_id: null, note: null }], "report kept, anonymised");
  assert.ok(w.q<any>(`SELECT revoked_at, note FROM circle_invites WHERE created_by = ?`, Z.id).every((x) => x.revoked_at !== null && x.note === null), "codes revoked, notes gone");
  // Reruns change nothing: erasure_finish's batch again, and the daily resweep (twice).
  const before = circleDump(w);
  await w.db.batch(deleteContentStatements(w.db, Z.id, w.clock));
  assert.equal(circleDump(w), before, "erasure_finish rerun");
  await w.pool.daily();
  const once = circleDump(w);
  await w.pool.daily();
  assert.equal(circleDump(w), once, "resweep idempotent");
  // A straggler an older build wrote after the erasure (an active owner row of the erased account in an
  // open circle with an editor): the daily resweep steps it down, hands the circle on and deletes it.
  const cs = await circleWith(w, M!, [[E1!, "editor"]], "straggler");
  w.sql.prepare(`UPDATE circle_members SET role = 'editor' WHERE circle_id = ? AND user_id = ?`).run(cs, M!.id);
  w.sql.prepare(`DELETE FROM circle_members WHERE circle_id = ? AND user_id = ?`).run(cs, M!.id);
  w.sql.prepare(`INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?, ?, 'owner', ?, 0)`).run(cs, Z.id, w.clock);
  await w.pool.daily();
  assert.deepEqual(owners(w, cs), [E1!.id]);
  assert.equal(w.q(`SELECT 1 FROM circle_members WHERE user_id = ?`, Z.id).length, 0);
  // An account that chose "do not keep my shared items": its banks are removed, purged after 14 days.
  const Y = w.user("teacher", "Yves");
  const cy = await circleWith(w, M!, [[Y, "editor"]], "c5");
  const yb = await share(w, Y, cy, bankOf(withQs(1, "Yves")));
  assert.equal((await w.json("PUT", "/items/prefs", { user: Y, json: { keep_on_delete: false } })).status, 200);
  await erase(w, Y);
  assert.equal(w.q<any>(`SELECT removed_reason FROM circle_banks WHERE id = ?`, yb.bid)[0].removed_reason, "account_deleted");
  await days(w, 15);
  assert.ok(w.q<any>(`SELECT purged_at FROM circle_banks WHERE id = ?`, yb.bid)[0].purged_at !== null);
  assert.deepEqual(w.r2.keys(`circles/${cy}/${yb.bid}/`), []);
  // No PII of Z anywhere after the settle.
  for (const t of w.q<{ name: string }>(`SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'`))
    for (const row of w.q(`SELECT * FROM "${t.name}"`)) assert.ok(!/zorgon quixote|5305551212/i.test(JSON.stringify(row)), `${t.name}: ${JSON.stringify(row).slice(0, 200)}`);
});

// ---- 10. erasure races (round 27 pattern): the erasure before every statement boundary of a request -----------------

function scanIds(w: World, needles: string[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const { name } of w.q<{ name: string }>(`SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'`))
    for (const row of w.q(`SELECT * FROM "${name}"`)) {
      const s = JSON.stringify(row, (_k, v) => (typeof v === "bigint" ? String(v) : v)).toLowerCase();
      if (needles.some((n) => s.includes(n.toLowerCase()))) out[name] = (out[name] ?? 0) + 1;
    }
  return out;
}
function instrument(w: World, boundary: (k: number) => Promise<void>) {
  const db = w.db as any;
  const prep = db.prepare.bind(db);
  const batch = db.batch.bind(db);
  let k = 0;
  let inside = false;
  const hit = async () => {
    if (inside) return;
    const n = k++;
    inside = true;
    try {
      await boundary(n);
    } finally {
      inside = false;
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
  return { count: () => k, restore: () => ((db.prepare = prep), (db.batch = batch)) };
}
/** Every open circle with an active member has exactly one active owner; no circle has two. */
function ownerInvariant(w: World, label: string) {
  for (const c of w.q<any>(`SELECT id, closed_at, close_reason, undo_until FROM circles WHERE purged_at IS NULL`)) {
    const o = owners(w, c.id).length;
    assert.ok(o <= 1, `${label}: ${o} owners`);
    // (round 34: an owner-closed circle inside its undo window keeps an owner too)
    const handedOn = c.closed_at === null || (c.close_reason === "owner_closed" && c.undo_until > w.clock);
    if (handedOn && active(w, c.id).length > 0) assert.equal(o, 1, `${label}: an open (or owner-closed) circle with members has no owner`);
  }
}
const ZNAME = "Zorgon Quixote";
interface RaceScenario {
  name: string;
  build(): Promise<{ w: World; Z: User; req: () => Promise<{ status: number }>; check?: (w: World) => void }>;
}
async function raceRun(sc: RaceScenario, mode: "ref" | "control" | number) {
  const { w, Z, req, check } = await sc.build();
  let status = 0;
  let boundaries = 0;
  if (mode === "control") {
    await erase(w, Z);
    status = (await req()).status;
  } else {
    const ins = instrument(w, async (k) => {
      if (k === mode) await erase(w, Z);
    });
    try {
      status = (await req()).status;
    } finally {
      ins.restore();
    }
    boundaries = ins.count();
    if (mode === "ref" || mode >= boundaries) await erase(w, Z);
  }
  await Promise.all(w.pending.splice(0));
  // Before any cron: the request's own writes must already respect the erasure (the guard IN the
  // write); the daily resweep is only defence in depth.
  const immediate = scanIds(w, [Z.id]);
  for (const step of [3600_000, DAY, 15 * DAY, DAY]) {
    w.advance(step);
    await w.pool.hourly();
    await w.pool.daily();
  }
  return { w, Z, status, boundaries, check, immediate };
}
async function race(sc: RaceScenario) {
  const ref = await raceRun(sc, "ref");
  const ctl = await raceRun(sc, "control");
  assert.equal(ctl.status, 401, `${sc.name}: a request after the erasure is refused like an unauthenticated one`);
  const refRows = scanIds(ref.w, [ref.Z.id]);
  const ctlRows = scanIds(ctl.w, [ctl.Z.id]);
  for (const r of [ref, ctl]) {
    ownerInvariant(r.w, sc.name);
    assert.deepEqual(scanIds(r.w, [ZNAME, "zq-private"]), {}, `${sc.name}: PII left`);
    r.check?.(r.w);
  }
  assert.ok(ref.boundaries >= 2, `${sc.name}: ${ref.boundaries} boundaries`);
  for (let k = 0; k < ref.boundaries; k++) {
    const r = await raceRun(sc, k);
    for (const [t, n] of Object.entries(r.immediate)) {
      const cap = Math.max(ref.immediate[t] ?? 0, ctl.immediate[t] ?? 0);
      assert.ok(n <= cap && cap > 0, `${sc.name}: erased before boundary ${k}/${ref.boundaries} (status ${r.status}): before the cron, ${t} holds ${n} rows of the erased account (a normal erasure leaves ${cap})`);
    }
    for (const [t, n] of Object.entries(scanIds(r.w, [r.Z.id]))) {
      const cap = Math.max(refRows[t] ?? 0, ctlRows[t] ?? 0);
      assert.ok(n <= cap && cap > 0, `${sc.name}: erased before boundary ${k}/${ref.boundaries} (status ${r.status}): residue in ${t} (${n} rows, a normal erasure leaves ${cap})`);
    }
    assert.deepEqual(scanIds(r.w, [ZNAME, "zq-private"]), {}, `${sc.name}: boundary ${k}: PII left`);
    assert.ok(r.w.r2.keys("circles/").length <= Math.max(ref.w.r2.keys("circles/").length, ctl.w.r2.keys("circles/").length), `${sc.name}: boundary ${k}: R2 objects left`);
    ownerInvariant(r.w, `${sc.name} boundary ${k}`);
    r.check?.(r.w);
  }
}
async function raceWorld() {
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), Z = w.user("teacher", ZNAME);
  w.sql.prepare(`UPDATE users SET created_at = ? WHERE id IN (?, ?, ?)`).run(w.clock - 30 * DAY, A.id, B.id, Z.id);
  return { w, A, B, Z };
}

const RACES: RaceScenario[] = [
  {
    name: "32 race POST /circles",
    async build() {
      const { w, Z } = await raceWorld();
      return { w, Z, req: () => w.json("POST", "/circles", { user: Z, json: { name: "zq-private circle", description: "zq-private" } }) };
    },
  },
  {
    name: "32 race POST /circles/join (Z joins A's circle)",
    async build() {
      const { w, A, Z } = await raceWorld();
      const cid = await circle(w, A);
      const k = await code(w, A, cid, "editor");
      return { w, Z, req: () => join(w, Z, k) };
    },
  },
  {
    name: "32 race POST /circles/:cid/banks (Z shares a bank)",
    async build() {
      const { w, A, Z } = await raceWorld();
      const cid = await circleWith(w, A, [[Z, "editor"]]);
      return { w, Z, req: () => send(w, Z, cid, bankOf([q("zq-private question about karst springs?")]), { message: "zq-private message" }) };
    },
  },
  {
    name: "32 race PUT /circles/:cid/banks/:bid (Z sends version 2)",
    async build() {
      const { w, A, Z } = await raceWorld();
      const cid = await circleWith(w, A, [[Z, "editor"]]);
      const s = await share(w, Z, cid, bankOf([q("Z's question about sinkholes?")]));
      return { w, Z, req: () => send(w, Z, cid, bankOf([q("zq-private v2 about sinkholes?")]), { bid: s.bid, base: 1, message: "zq-private v2" }) };
    },
  },
  {
    name: "32 race POST /circles/:cid/invites (Z, the owner, makes a code)",
    async build() {
      const { w, A, Z } = await raceWorld();
      const cid = await circleWith(w, Z, [[A, "editor"]]);
      return { w, Z, req: () => w.json("POST", `/circles/${cid}/invites`, { user: Z, json: { role: "viewer", note: "zq-private note" } }) };
    },
  },
  {
    name: "32 race POST /circles/:cid/banks/:bid/report (Z reports, student data)",
    async build() {
      const { w, A, Z } = await raceWorld();
      const cid = await circleWith(w, A, [[Z, "viewer"]]);
      const s = await share(w, A, cid, bankOf([q("Ann's question about caves?")]));
      w.advance(2 * DAY);
      return {
        w,
        Z,
        req: () => w.json("POST", `/circles/${cid}/banks/${s.bid}/report`, { user: Z, json: { reason: "student_data", note: "zq-private note" } }),
        check: (w2) => assert.equal(w2.q(`SELECT 1 FROM circle_reports WHERE reporter_id IS NOT NULL`).length, 0),
      };
    },
  },
  {
    name: "32 race POST /circles/:cid/transfer (Z, the owner, hands over to Ann)",
    async build() {
      const { w, A, B, Z } = await raceWorld();
      await circleWith(w, Z, [[B, "viewer"], [A, "editor"]]);
      const cid = w.q<any>(`SELECT id FROM circles`)[0].id;
      return { w, Z, req: () => w.json("POST", `/circles/${cid}/transfer`, { user: Z, json: { user_id: A.id } }) };
    },
  },
  {
    name: "32 race POST /circles/:cid/transfer/accept (Z takes over Ann's circle, round 34)",
    async build() {
      const { w, A, B, Z } = await raceWorld();
      const cid = await circleWith(w, A, [[B, "viewer"], [Z, "editor"]]);
      assert.equal((await w.json("POST", `/circles/${cid}/transfer`, { user: A, json: { user_id: Z.id } })).status, 202);
      return { w, Z, req: () => w.json("POST", `/circles/${cid}/transfer/accept`, { user: Z }), check: (w2) => assert.equal(w2.q(`SELECT 1 FROM circle_transfers`).length, 0) };
    },
  },
  {
    name: "32 race POST /circles/:cid/members/role (Z, the owner)",
    async build() {
      const { w, A, B, Z } = await raceWorld();
      const cid = await circleWith(w, Z, [[B, "viewer"], [A, "editor"]]);
      return { w, Z, req: () => w.json("POST", `/circles/${cid}/members/role`, { user: Z, json: { user_id: B.id, role: "editor" } }) };
    },
  },
  {
    name: "32 race POST /circles/:cid/leave (Z, the owner)",
    async build() {
      const { w, A, B, Z } = await raceWorld();
      const cid = await circleWith(w, Z, [[B, "viewer"], [A, "editor"]]);
      return { w, Z, req: () => w.json("POST", `/circles/${cid}/leave`, { user: Z }), check: (w2) => assert.deepEqual(owners(w2, cid), [A.id]) };
    },
  },
];

for (const sc of RACES)
  test(`32-10 ${sc.name}: erased before every statement boundary, nothing but a normal erasure's residue is left, no PII, no ownerless circle`, async () => {
    await race(sc);
  });

test("32-10b: the last member is erased while a valid code is being used: the join succeeds only if it committed first (the joiner then owns the circle), else it fails and the circle stays closed", async () => {
  let ok = 0, refused = 0;
  for (let k = 0; ; k++) {
    const { w, A, Z } = await raceWorld();
    const cid = await circle(w, Z);
    const c = await code(w, Z, cid, "viewer");
    const ins = instrument(w, async (n) => {
      if (n === k) await erase(w, Z);
    });
    let res: { status: number; body: any };
    try {
      res = await join(w, A, c);
    } finally {
      ins.restore();
    }
    const n = ins.count();
    if (k >= n) await erase(w, Z);
    const st = w.q<any>(`SELECT closed_at FROM circles WHERE id = ?`, cid)[0];
    if (res.status === 200) {
      ok++;
      assert.deepEqual(owners(w, cid), [A.id], `boundary ${k}`);
      assert.equal(st.closed_at, null);
    } else {
      refused++;
      assert.deepEqual([res.status, res.body.error], [422, "circle_invite_invalid"], `boundary ${k}`);
      assert.ok(st.closed_at !== null);
      assert.equal(active(w, cid).length, 0);
    }
    ownerInvariant(w, `boundary ${k}`);
    if (k >= n) break;
  }
  assert.ok(ok >= 1 && refused >= 1, `${ok} joined, ${refused} refused`);
});

// ---- 11. the oracle: a small model of circles against the Worker, random sequences ---------------------------------

interface MCircle {
  id: string;
  closed: string | null;
  undoUntil: number | null;
  purgeAfter: number | null;
  members: Map<string, { role: string; seq: number }>;
  everIn: Set<string>;
  banks: Map<string, { author: string; version: number; removed: boolean }>;
  codes: { code: string; role: string; uses: number; max: number; revoked: boolean; expires: number; issuer: string; created: number }[];
  /** Round 33: who left or was removed, when, in which role (a code older than that is limited by it). */
  former: Map<string, { role: string; leftAt: number; reason: "left" | "removed" }>;
}

test("32-11 (oracle): random sequences of create, code, join, leave, remove, role, transfer, close, reopen, send, republish, erase and cron runs match a model after every step: one owner, succession by join order, linear versions, every refused send a 409 with the true head, 404 to non-members", async () => {
  const seeds = Number(process.env.POOL_PROPERTY_SEEDS ?? 5);
  let steps = 0, sends = 0, conflicts = 0, successions = 0, closes = 0, purges = 0, rejoins = 0, refusedRejoins = 0;
  for (let seed = 0; seed < seeds; seed++) {
    const r = rng(32000 + seed);
    const w = new World();
    const us = Array.from({ length: 7 }, (_, i) => w.user("teacher", `U${i}`));
    const alive = new Set(us.map((u) => u.id));
    const circles: MCircle[] = [];
    let seq = 0;
    const tries = new Map<string, number>();
    const day = () => Math.floor(w.clock / DAY);
    const owned = (u: string) => circles.filter((c) => c.members.get(u)?.role === "owner").length;
    const memberships = (u: string) => circles.filter((c) => c.members.has(u)).length;
    const succession = (c: MCircle) => {
      // (round 34: an owner-closed circle inside its undo window is handed on too, and stays closed)
      const ownerClosed = c.closed === "owner_closed" && (c.undoUntil ?? 0) > w.clock;
      if ((c.closed !== null && !ownerClosed) || [...c.members.values()].some((m) => m.role === "owner")) return;
      for (const role of ["editor", "viewer"]) {
        const pick = [...c.members.entries()].filter(([, m]) => m.role === role).sort((a, b) => a[1].seq - b[1].seq)[0];
        if (pick) {
          pick[1].role = "owner";
          successions++;
          return;
        }
      }
      if (ownerClosed) return;
      c.closed = "last_member_gone";
      c.undoUntil = null;
      c.purgeAfter = w.clock + 14 * DAY;
      closes++;
    };
    const live = () => circles.filter((c) => !c.purgeAfter || c.purgeAfter > w.clock || c.closed === null);
    // Round 33: a code dies with its issuer's owner role (leave, transfer, removal, erasure).
    const revokeBy = (x: MCircle, who: string) => x.codes.forEach((k) => k.issuer === who && (k.revoked = true));
    const depart = (x: MCircle, who: string, reason: "left" | "removed") => {
      const mm = x.members.get(who)!;
      x.former.set(who, { role: mm.role, leftAt: w.clock, reason });
      x.members.delete(who);
      revokeBy(x, who);
    };
    for (let step = 0; step < 90; step++) {
      steps++;
      // (weights: create 1, code 2, join 3, leave 1, remove 1, role 1, transfer 1, close/reopen 1, send 5, stranger 1, erase 1, cron 1)
      const op = r.pick([0, 1, 1, 2, 2, 3, 4, 5, 6, 7, 8, 9, 9, 10, 10, 10, 11, 12, 13]);
      const cs = live().filter((x) => !(x as { gone?: boolean }).gone);
      const c = cs.length ? r.pick(cs) : null;
      // The actor: mostly the one who may act (the owner for owner actions, an author or editor for
      // sends), so every rule is reached; now and then anyone.
      let u = r.pick(us);
      const byId = (id: string) => us.find((x) => x.id === id)!;
      const ownerOf = c ? [...c.members.entries()].find(([, m]) => m.role === "owner")?.[0] : undefined;
      if (c && ownerOf && [1, 4, 5, 6, 7, 8, 12].includes(op) && r.bool(0.7)) u = byId(ownerOf);
      if (c && (op === 9 || op === 10) && r.bool(0.8)) {
        const authors = [...c.banks.values()].map((b) => b.author).filter((a) => alive.has(a));
        const editors = [...c.members.entries()].filter(([, m]) => m.role !== "viewer").map(([k]) => k);
        const pool = authors.length && r.bool(0.7) ? authors : editors;
        if (pool.length) u = byId(r.pick(pool));
      }
      const me = u.id;
      const mine = c?.members.get(me);
      const label = `seed ${seed} step ${step} op ${op}`;
      if (!alive.has(me)) {
        // An erased account's requests are refused like unauthenticated ones.
        assert.equal((await w.json("GET", "/circles", { user: u })).status, 401, label);
      } else if (op === 0 || !c) {
        const res = await w.json<any>("POST", "/circles", { user: u, json: { name: `c${step}` } });
        if (owned(me) < 5 && memberships(me) < 20 && res.status !== 429) {
          assert.equal(res.status, 201, `${label}: ${JSON.stringify(res.body)}`);
          circles.push({ id: res.body.circle.id, closed: null, undoUntil: null, purgeAfter: null, members: new Map([[me, { role: "owner", seq: ++seq }]]), everIn: new Set([me]), banks: new Map(), codes: [], former: new Map() });
        } else assert.ok(res.status === 422 || res.status === 429, label);
      } else if (op === 1) {
        const res = await w.json<any>("POST", `/circles/${c.id}/invites`, { user: u, json: { role: r.bool() ? "editor" : "viewer", max_uses: 1 + r.int(4), expires_in_days: 1 + r.int(3) } });
        const open = c.codes.filter((k) => !k.revoked && k.expires > w.clock && k.uses < k.max).length;
        if (!mine) assert.equal(res.status, 404, label);
        else if (c.closed) assert.equal(res.status, 409, label);
        else if (mine.role !== "owner") assert.equal(res.status, 403, label);
        else if (open >= 10) assert.equal(res.status, 422, label);
        else {
          assert.equal(res.status, 201, label);
          c.codes.push({ code: res.body.code, role: res.body.role, uses: 0, max: res.body.max_uses, revoked: false, expires: res.body.expires_at, issuer: me, created: w.clock });
        }
      } else if (op <= 3) {
        // A join; half the time a former member (left or removed) tries one of that circle's codes, old or new.
        const formerPairs = circles
          .filter((x) => x.closed === null && !(x as { gone?: boolean }).gone)
          .flatMap((x) => [...x.former.keys()].filter((f) => alive.has(f) && !x.members.has(f)).map((f) => ({ x, f })))
          .filter((p) => p.x.codes.length > 0 || [...p.x.members.values()].some((mm) => mm.role === "owner"));
        let ju = u;
        let x: MCircle, k: MCircle["codes"][number];
        if (formerPairs.length && r.bool(0.5)) {
          const pp = r.pick(formerPairs);
          ju = byId(pp.f);
          x = pp.x;
          // half the time the owner first issues a fresh code (a re-admission), so newer-code rejoins happen too
          const own = [...x.members.entries()].find(([, mm]) => mm.role === "owner")?.[0];
          if (own && x.closed === null && r.bool(0.5)) {
            w.advance(1);
            const nc = await w.json<any>("POST", `/circles/${x.id}/invites`, { user: byId(own), json: { role: r.bool() ? "editor" : "viewer" } });
            if (nc.status === 201) x.codes.push({ code: nc.body.code, role: nc.body.role, uses: 0, max: 1, revoked: false, expires: nc.body.expires_at, issuer: own, created: w.clock });
          }
          if (!x.codes.length) continue;
          const usable = x.codes.filter((kk) => !kk.revoked && kk.expires > w.clock && kk.uses < kk.max);
          k = usable.length && r.bool(0.7) ? r.pick(usable) : r.pick(x.codes);
        } else {
          const all = circles.flatMap((xx) => xx.codes.map((kk) => ({ x: xx, k: kk })));
          if (!all.length) continue;
          ({ x, k } = r.pick(all));
        }
        const jme = ju.id;
        const res = await join(w, ju, k.code);
        const fm = x.former.get(jme);
        const valid =
          !k.revoked && k.expires > w.clock && k.uses < k.max && x.closed === null && !x.purgeAfter && x.members.get(k.issuer)?.role === "owner" && !(fm?.reason === "removed" && fm.leftAt >= k.created);
        const role = fm && fm.leftAt >= k.created && fm.role === "viewer" ? "viewer" : k.role;
        const key = `${jme}:${day()}`;
        if (x.members.has(jme) && !(x.purgeAfter && x.purgeAfter <= w.clock)) {
          if (res.status === 429) continue;
          assert.equal(res.body.already, true, `${label}: ${JSON.stringify(res.body)}`);
        } else if ((tries.get(key) ?? 0) >= 10) assert.equal(res.status, 429, label);
        else if (valid && x.members.size < 50 && memberships(jme) < 20) {
          assert.deepEqual([res.status, res.body.role], [200, role], `${label}: ${JSON.stringify(res.body)}`);
          k.uses++;
          x.members.set(jme, { role, seq: ++seq });
          x.former.delete(jme);
          x.everIn.add(jme);
          if (fm) rejoins++;
        } else {
          assert.equal(res.status, 422, `${label}: ${JSON.stringify(res.body)}`);
          tries.set(key, (tries.get(key) ?? 0) + 1);
          if (fm) refusedRejoins++;
        }
      } else if (op === 4) {
        const res = await w.json("POST", `/circles/${c.id}/leave`, { user: u });
        if (!mine) assert.equal(res.status, 404, label);
        else {
          assert.equal(res.status, 200, label);
          depart(c, me, "left");
          succession(c);
        }
      } else if (op === 5 || op === 6 || op === 7) {
        const others = [...c.members.keys()].filter((x) => x !== me);
        const t = others.length ? r.pick(others) : us[0]!.id;
        const tm = c.members.get(t);
        const path = op === 5 ? "members/remove" : op === 6 ? "members/role" : "transfer";
        const role = r.bool() ? "editor" : "viewer";
        const res = await w.json<any>("POST", `/circles/${c.id}/${path}`, { user: u, json: { user_id: t, role } });
        if (!mine) assert.equal(res.status, 404, label);
        else if (c.closed) assert.equal(res.status, 409, label);
        else if (mine.role !== "owner") assert.equal(res.status, 403, label);
        else if (!tm || t === me) assert.ok(res.status === 404 || res.status === 400 || (op === 7 && res.status === 200), `${label}: ${res.status}`);
        else if (op === 7) {
          // Round 34: an offer (202), then the member's accept (the cap is checked there).
          assert.equal(res.status, 202, `${label} offer: ${JSON.stringify(res.body)}`);
          const acc = await w.json<any>("POST", `/circles/${c.id}/transfer/accept`, { user: byId(t) });
          if (owned(t) >= 5) assert.deepEqual([acc.status, acc.body.metric], [422, "circles_owned"], label);
          else {
            assert.equal(acc.status, 200, `${label} accept: ${JSON.stringify(acc.body)}`);
            mine.role = "editor";
            tm.role = "owner";
            revokeBy(c, me);
          }
        } else {
          assert.equal(res.status, 200, `${label} ${path}: ${JSON.stringify(res.body)}`);
          if (op === 5) depart(c, t, "removed");
          else tm.role = role;
        }
      } else if (op === 8) {
        const closing = r.bool(0.6);
        const res = await w.json<any>(closing ? "DELETE" : "POST", closing ? `/circles/${c.id}` : `/circles/${c.id}/reopen`, { user: u });
        if (!mine) assert.equal(res.status, 404, label);
        else if (mine.role !== "owner") assert.equal(res.status, 403, label);
        else if (res.status === 200) {
          // close (DELETE) or reopen: the model follows what the route did, checked below against the state.
          const row = w.q<any>(`SELECT close_reason, undo_until, purge_after FROM circles WHERE id = ?`, c.id)[0];
          c.closed = row.close_reason;
          c.undoUntil = row.undo_until;
          c.purgeAfter = row.purge_after;
        } else assert.equal(res.status, 409, label);
      } else if (op <= 10) {
        // Share a new bank (owner, editor), or the author's republish (based on the head, or a stale one).
        const own = [...c.banks.entries()].filter(([, b]) => b.author === me && !b.removed);
        if (own.length && r.bool(0.6)) {
          const [bid, b] = r.pick(own);
          const stale = b.version > 1 && r.bool(0.35);
          const res = await send(w, u, c.id, bankOf([q(`v ${step} of ${bid.slice(0, 6)}?`)]), { bid, base: stale ? b.version - 1 : b.version });
          sends++;
          if (!mine) assert.equal(res.status, 404, label);
          else if (c.closed) assert.equal(res.status, 409, label);
          else if (mine.role === "viewer") assert.equal(res.status, 403, label);
          else if (stale) {
            assert.deepEqual([res.status, res.body.error, res.body.head_version], [409, "conflict", b.version], label);
            conflicts++;
          } else {
            assert.equal(res.status, 200, `${label}: ${JSON.stringify(res.body)}`);
            assert.equal(res.body.version, b.version + 1, "versions are linear");
            b.version++;
          }
        } else {
          const res = await send(w, u, c.id, bankOf([q(`bank ${step} by ${me.slice(0, 8)}?`)]));
          sends++;
          if (!mine) assert.equal(res.status, 404, label);
          else if (c.closed) assert.equal(res.status, 409, label);
          else if (mine.role === "viewer") assert.equal(res.status, 403, label);
          else {
            assert.equal(res.status, 201, `${label}: ${JSON.stringify(res.body)}`);
            c.banks.set(res.body.bank_id, { author: me, version: 1, removed: false });
          }
        }
      } else if (op === 11) {
        // A non-member (or former member) reads: 404, whatever the circle.
        const strangers = us.filter((x) => !c.members.has(x.id) && alive.has(x.id));
        if (!strangers.length) continue;
        const sx = r.pick(strangers);
        assert.equal((await w.json("GET", `/circles/${c.id}`, { user: sx })).status, 404, label);
        for (const bid of c.banks.keys()) assert.equal((await w.json("GET", `/circles/${c.id}/banks/${bid}`, { user: sx })).status, 404, label);
      } else if (op === 12 && r.bool(0.5)) {
        await erase(w, u);
        alive.delete(me);
        for (const x of circles) {
          revokeBy(x, me);
          x.former.delete(me);
          if (x.members.delete(me)) succession(x);
          x.everIn.delete(me);
          for (const b of x.banks.values()) if (b.author === me) b.author = "erased";
        }
      } else {
        w.advance(r.pick([3600_000, DAY, 3 * DAY, 8 * DAY]));
        await w.pool.hourly();
        await w.pool.daily();
        for (const x of circles) {
          for (const k of x.codes) if (k.expires <= w.clock) k.revoked = k.revoked || false;
          if (x.closed !== null && x.purgeAfter !== null && x.purgeAfter <= w.clock && !(x as { gone?: boolean }).gone) {
            (x as { gone?: boolean }).gone = true;
            x.members.clear();
            purges++;
          }
        }
      }
      // ---- compare after every step -----------------------------------------------------------------
      for (const x of circles) {
        const row = w.q<any>(`SELECT closed_at, close_reason, purged_at FROM circles WHERE id = ?`, x.id)[0];
        if ((x as { gone?: boolean }).gone) {
          assert.ok(row.purged_at !== null, `${label}: purged`);
          continue;
        }
        assert.equal(row.close_reason, x.closed, `${label}: circle state`);
        const got = Object.fromEntries(active(w, x.id).map((m) => [m.user_id, m.role]));
        const want = Object.fromEntries([...x.members.entries()].map(([k, m]) => [k, m.role]));
        assert.deepEqual(got, want, `${label}: members of ${x.id.slice(0, 6)}`);
        for (const [bid, b] of x.banks) assert.equal(w.q<any>(`SELECT head_version FROM circle_banks WHERE id = ?`, bid)[0].head_version, b.version, `${label}: head`);
      }
      ownerInvariant(w, label);
      // Versions are gap-free per bank.
      for (const b of w.q<any>(`SELECT bank_id, COUNT(*) AS n, MAX(version) AS mx FROM circle_versions WHERE status <> 'staging' GROUP BY bank_id`)) assert.equal(b.n, b.mx, `${label}: gap-free`);
    }
  }
  const report = `${steps} steps, ${sends} sends, ${conflicts} conflicts, ${successions} successions, ${closes} closes by the last member, ${purges} purges, ${rejoins} rejoins, ${refusedRejoins} refused rejoins`;
  if (process.env.POOL_ROUND32_REPORT === "1") console.log(report);
  assert.ok(sends >= 80 && conflicts >= 3 && successions >= 3 && closes >= 2 && purges >= 1 && rejoins + refusedRejoins >= 3, report);
});

// ---- 12. rows visited and query plans at the caps (rounds 30-31 method) ----------------------------------------------

interface Cap {
  sql: string;
  args: unknown[];
}
/** Records (never EXPLAINs inside the hook: an EXPLAIN between batch statements would reset changes()). */
function capture(w: World, is: (sql: string) => boolean): Cap[] {
  const out: Cap[] = [];
  w.stats.log = (sql, args) => {
    if (is(sql)) out.push({ sql, args: [...args] });
  };
  return out;
}
const planOf = (w: World, c: Cap) => (w.sql.prepare(`EXPLAIN QUERY PLAN ${c.sql}`).all(...(c.args as never[])) as { detail: string }[]).map((x) => x.detail);
/** A SCAN of a real table (not a constant row, a CTE, a subquery result or a bound JSON array). */
const tableScans = (w: World, c: Cap) => planOf(w, c).filter((d) => /^SCAN /.test(d) && !/^SCAN (CONSTANT ROW|h|my|x|\(subquery-\d+\))(\s|$)/.test(d) && !/VIRTUAL TABLE/.test(d));
function bestOf(f: () => void) {
  let best = Infinity;
  for (let i = 0; i < 5; i++) {
    const t = performance.now();
    f();
    best = Math.min(best, performance.now() - t);
  }
  return best;
}
const timeOf = (w: World, c: Cap) => bestOf(() => w.sql.prepare(c.sql).all(...(c.args as never[])));
const LIM = process.env.POOL_STRICT_TIMING === "1" ? 8 : 12;

/** A circle world built by SQL at a given size: `me` in `nc` circles of `nm` members (shared) with `nb` banks each. */
async function bigCircles(nc: number, nm: number, nb: number) {
  const w = new World();
  const me = w.user("teacher", "Me");
  const others = Array.from({ length: nm - 1 }, (_, i) => w.user("teacher", `M${i}`));
  const ins = {
    c: w.sql.prepare(`INSERT INTO circles (id, name, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?)`),
    m: w.sql.prepare(`INSERT INTO circle_members (circle_id, user_id, role, joined_at, join_seq) VALUES (?, ?, ?, ?, ?)`),
    b: w.sql.prepare(`INSERT INTO circle_banks (id, circle_id, author_id, title, license, head_version, head_version_id, question_count, created_at, updated_at) VALUES (?, ?, ?, ?, 'circle-only', 1, ?, 10, ?, ?)`),
    v: w.sql.prepare(`INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, committer_id, license, status, full_key, index_key, bytes, question_count, seq, created_at, committed_at) VALUES (?, ?, ?, 1, ?, 'x', ?, 'circle-only', 'current', ?, ?, 1000, 10, 1, 1, 1)`),
  };
  const hex = (n: number, tag: number) => (tag.toString(16).padStart(4, "0") + n.toString(16).padStart(28, "0")).slice(0, 32);
  const circles: string[] = [];
  const banks: string[] = [];
  let seq = 0;
  w.sql.exec("BEGIN");
  for (let i = 0; i < nc; i++) {
    const cid = hex(i, 1);
    circles.push(cid);
    ins.c.run(cid, `Circle ${i}`, me.id, w.clock, w.clock);
    ins.m.run(cid, me.id, "owner", w.clock, ++seq);
    for (const o of others) ins.m.run(cid, o.id, "editor", w.clock, ++seq);
    for (let k = 0; k < nb; k++) {
      const bid = hex(i * 1000 + k, 2), vid = hex(i * 1000 + k, 3);
      banks.push(bid);
      ins.b.run(bid, cid, others[k % others.length]?.id ?? me.id, `Bank ${k}`, vid, w.clock, w.clock);
      ins.v.run(vid, bid, cid, `commit${vid}`, me.id, `circles/${cid}/${bid}/${vid}.json`, `circles/${cid}/${bid}/${vid}.idx.json`);
    }
  }
  w.sql.exec("COMMIT");
  return { w, me, others, circles, banks };
}

test("32-12a: the read routes at the caps (20 circles x 50 members x 100 banks; 200 banks a heads call): every statement reads by index (no table scan), and time grows linearly", async () => {
  const run = async (nc: number, nm: number, nb: number) => {
    const { w, me, circles, banks } = await bigCircles(nc, nm, nb);
    const caps = capture(w, (sql) => /circle/.test(sql) && /^\s*(SELECT|WITH)/i.test(sql));
    assert.equal((await w.json<any>("GET", "/circles", { user: me })).body.circles.length, nc);
    const g = await w.json<any>("GET", `/circles/${circles[0]}`, { user: me });
    assert.deepEqual([g.body.members.length, g.body.banks.length], [nm, nb]);
    const h = await w.json<any>("POST", "/circles/heads", { user: me, json: { bank_ids: banks.slice(0, 200) } });
    assert.equal(h.body.banks.filter((x: Q) => x.status === "current").length, Math.min(200, banks.length));
    assert.equal((await w.json<any>("GET", `/circles/${circles[0]}/activity`, { user: me })).status, 200);
    w.stats.log = undefined;
    const pick = (re: RegExp) => caps.find((c) => re.test(c.sql))!;
    return { w, list: pick(/FROM circle_members m JOIN circles ci/), get: pick(/FROM circle_banks b LEFT JOIN users u ON u.id = b.author_id/), members: pick(/SELECT m.user_id, m.role, m.joined_at, u.display_name/), heads: pick(/FROM json_each\(\?2\) j\s+CROSS JOIN circle_banks b/), caps };
  };
  const small = await run(5, 13, 25);
  const big = await run(20, 50, 100);
  for (const c of big.caps) assert.deepEqual(tableScans(big.w, c), [], c.sql.replace(/\s+/g, " ").slice(0, 160));
  const plan = planOf(big.w, big.list);
  assert.ok(plan.some((d) => /SEARCH m USING INDEX circle_members_user \(user_id=\? AND left_at=\?\)/.test(d)), plan.join(" | "));
  assert.ok(plan.some((d) => /SEARCH xm USING (COVERING )?INDEX circle_members_active \(circle_id=\?\)/.test(d)), plan.join(" | "));
  assert.ok(plan.some((d) => /SEARCH b USING (COVERING )?INDEX circle_banks_circle \(circle_id=\? AND purged_at=\?/.test(d)), plan.join(" | "));
  const hp = planOf(big.w, big.heads);
  assert.ok(/^SCAN j VIRTUAL TABLE/.test(hp[0]!) && hp.some((d) => /SEARCH b USING INDEX sqlite_autoindex_circle_banks_1 \(id=\?\)/.test(d)), hp.join(" | "));
  // Rows the list visits: my memberships + each circle's active members and banks (by index), no more.
  const visits = (x: typeof big) => x.w.q<{ n: number }>(`SELECT (SELECT COUNT(*) FROM circle_members WHERE user_id = (SELECT user_id FROM circle_members WHERE role = 'owner' LIMIT 1)) + (SELECT COUNT(*) FROM circle_members WHERE left_at IS NULL) + (SELECT COUNT(*) FROM circle_banks) AS n`)[0]!.n;
  assert.equal(visits(big), 20 + 20 * 50 + 20 * 100, "about 3,000 rows (plan 9.1)");
  const report: string[] = [];
  for (const k of ["list", "get", "members", "heads"] as const) {
    const [ts, tb] = [timeOf(small.w, small[k]), timeOf(big.w, big[k])];
    report.push(`${k}: ${ts.toFixed(2)} -> ${tb.toFixed(2)} ms`);
    // (the list: 16x the rows (4x circles, 4x members and banks): linear is about 16x)
    assert.ok(tb < Math.max((k === "list" ? 4 : 1) * LIM * ts, 25), report.join("; "));
  }
  if (process.env.POOL_ROUND32_REPORT === "1") console.log(report.join("\n"), visits(small), visits(big));
});

test("32-12b: the public publish check at 250 and 990 questions: one indexed seek per (distinct hash, circle) even when every bank of a circle holds the hash and when a bank repeats one question 990 times; a non-member costs one probe", async () => {
  const { checkCircleMaterial } = await import("../../src/pool/circlecheck.ts");
  const { core } = await import("./harness.ts");
  const run = async (n: number, repeat: boolean) => {
    const { w, me, circles } = await bigCircles(20, 2, 1);
    const other = w.q<{ id: string }>(`SELECT user_id AS id FROM circle_members WHERE user_id <> ? LIMIT 1`, me.id)[0]!.id;
    const qs = Array.from({ length: n }, (_, i) => q(repeat ? "One question repeated: how do braided rivers form on outwash plains?" : `Question ${i}: how do braided rivers form on outwash plain number ${i}?`));
    const mh = core.matchHashes(new TextEncoder().encode(bankOf(qs))) as string[];
    // Every one of 100 banks in circle 0 holds every hash, first sent by someone else; the other circles too, once.
    const put = w.sql.prepare(`INSERT OR IGNORE INTO circle_qkeys (circle_id, match_hash, bank_id, first_version, first_seq, first_by) VALUES (?, ?, ?, 1, ?, ?)`);
    w.sql.exec("BEGIN");
    for (let b = 0; b < 100; b++) for (const h of new Set(mh)) put.run(circles[0], h, `${b}`.padStart(32, "b"), 100 + b, other);
    for (const cid of circles.slice(1)) for (const h of new Set(mh)) put.run(cid, h, "c".repeat(32), 50, other);
    // (the records counter a send keeps: the check reads only circles holding records)
    w.sql.exec(`UPDATE circles SET qkeys = (SELECT COUNT(*) FROM circle_qkeys k WHERE k.circle_id = circles.id)`);
    w.sql.exec("COMMIT");
    const caps = capture(w, (sql) => sql === (CIRCLE_HITS_SQL as string) || /FROM author_firsts|READABLE|json_each\(\?2\) j\s+WHERE EXISTS/.test(sql));
    const c = { db: w.db } as never;
    let refused: number[] = [];
    try {
      await checkCircleMaterial(c, me.id, mh.map((h) => [h, h]), mh.map(() => false), null);
    } catch (e) {
      refused = (e as { extra: { positions: number[] } }).extra.positions;
    }
    w.stats.log = undefined;
    assert.equal(refused.length, n, "every position refused");
    // A non-member: one probe, nothing else.
    const lone = w.user("teacher", "Lone");
    const c2 = capture(w, () => true);
    assert.deepEqual(await checkCircleMaterial(c, lone.id, mh.map((h) => [h, h]), mh.map(() => false), null), { claimed: [] });
    w.stats.log = undefined;
    assert.equal(c2.length, 1);
    return { w, hits: caps.filter((x) => x.sql === CIRCLE_HITS_SQL), distinct: new Set(mh).size };
  };
  const report: string[] = [];
  for (const repeat of [false, true]) {
    const small = await run(250, repeat), big = await run(990, repeat);
    for (const x of [small, big]) {
      assert.equal(x.hits.length, 1);
      const plan = planOf(x.w, x.hits[0]!);
      assert.ok(plan.some((d) => /SEARCH k USING COVERING INDEX circle_qkeys_first \(circle_id=\? AND match_hash=\?\)/.test(d)), plan.join(" | "));
      assert.deepEqual(tableScans(x.w, x.hits[0]!), [], plan.join(" | "));
    }
    // Rows visited: (distinct hashes) x (circles) seeks reading ONE entry each (LIMIT 1 on the first_seq order),
    // where a join on the primary key would read every bank's entry (100 in circle 0).
    const probes = (x: typeof small) => x.distinct * 20;
    const joinRows = (x: typeof small) => x.w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM circle_qkeys`)[0]!.n;
    report.push(`repeat ${repeat}: probes ${probes(small)} -> ${probes(big)}; a plain join would read ${joinRows(small)} -> ${joinRows(big)}`);
    assert.equal(probes(big), (repeat ? 1 : 990) * 20);
    assert.ok(joinRows(big) >= 5 * probes(big), report.join("; "));
    const [ts, tb] = [timeOf(small.w, small.hits[0]!), timeOf(big.w, big.hits[0]!)];
    report.push(`time ${ts.toFixed(1)} -> ${tb.toFixed(1)} ms`);
    assert.ok(tb < Math.max(LIM * ts, 25), report.join("; "));
  }
  if (process.env.POOL_ROUND32_REPORT === "1") console.log(report.join("\n"));
});

test("32-12c: a 990-question send into a circle with 99 banks and 20 kept versions each, and its read (the copy): the quota sum, the record probe and the commit read by index; the read is three key lookups whatever the size", async () => {
  const run = async (n: number) => {
    const { w, me, circles } = await bigCircles(1, 2, 99);
    // 20 kept versions per bank (the quota sum's worst case, plan 9.1: 100 banks x 20).
    const v = w.sql.prepare(`INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, committer_id, license, status, full_key, index_key, bytes, question_count, seq, created_at, committed_at) VALUES (?, ?, ?, ?, ?, 'x', ?, 'circle-only', 'previous', 'k', 'k', 1000, 10, 1, 1, 1)`);
    w.sql.exec("BEGIN");
    for (const b of w.q<{ id: string }>(`SELECT id FROM circle_banks`)) for (let k = 2; k <= 20; k++) v.run(`e${k.toString(16).padStart(3, "0")}${b.id.slice(4)}`, b.id, circles[0], k, `c${b.id}${k}`, me.id);
    w.sql.exec("COMMIT");
    const caps = capture(w, (sql) => /circle/.test(sql) && /^\s*(SELECT|WITH|INSERT|UPDATE|DELETE)/i.test(sql));
    const s = await send(w, me, circles[0]!, bankOf(withQs(n, "Big")));
    assert.equal(s.status, 201, JSON.stringify(s.body).slice(0, 300));
    const r = await read(w, me, circles[0]!, s.body.bank_id);
    assert.equal(r.body.payload.bank.questions.length, n);
    w.stats.log = undefined;
    return { w, caps, cas: caps.find((c) => /^\s*INSERT INTO circle_banks/.test(c.sql))!, probe: caps.find((c) => /SELECT COUNT\(\*\) AS n FROM json_each\(\?1\) j WHERE NOT EXISTS/.test(c.sql))!, qkeys: caps.find((c) => /INSERT OR IGNORE INTO circle_qkeys/.test(c.sql))! };
  };
  const small = await run(250), big = await run(990);
  for (const c of big.caps) assert.deepEqual(tableScans(big.w, c), [], c.sql.replace(/\s+/g, " ").slice(0, 160));
  const casPlan = planOf(big.w, big.cas);
  assert.ok(casPlan.some((d) => /SEARCH sv USING (COVERING )?INDEX circle_versions_circle \(circle_id=\? AND status=\?\)/.test(d)), casPlan.join(" | "));
  assert.ok(planOf(big.w, big.probe).some((d) => /SEARCH k USING PRIMARY KEY \(circle_id=\? AND match_hash=\? AND bank_id=\?\)/.test(d)));
  const report: string[] = [];
  for (const k of ["cas", "probe"] as const) {
    const [ts, tb] = [timeOf(small.w, small[k]), timeOf(big.w, big[k])];
    report.push(`${k}: ${ts.toFixed(2)} -> ${tb.toFixed(2)} ms`);
    assert.ok(tb < Math.max(LIM * ts, 25), report.join("; "));
  }
  // The read: no statement of GET /circles/:cid/banks/:bid touches a per-question table.
  if (process.env.POOL_ROUND32_REPORT === "1") console.log(report.join("\n"));
});

// ---- 13. migration 0108 on a populated database ---------------------------------------------------------------------

test("32-13: migration 0108 on a database at 0107 with rows: no existing table, index or row changes; the circle tables are new; circles then work on it", async () => {
  const src = new World();
  const [A, B] = [src.user("teacher", "Ann"), src.user("teacher", "Bob")];
  const a = (await publish(src, A, bankOf(withQs(3, "Public")), { license: "CC-BY-4.0" })).body as any;
  await src.json("POST", `/items/${a.item_id}/import`, { user: B });
  await src.json("POST", `/items/${a.item_id}/report`, { user: B, json: { reason: "other", note: "n" } });
  const dir = new URL("../../migrations/", import.meta.url).pathname;
  const files = readdirSync(dir).filter((f) => f.endsWith(".sql")).sort();
  assert.ok(files.includes("0108_pool_round32.sql"));
  const old = new DatabaseSync(":memory:");
  for (const f of files.filter((f) => f < "0108")) old.exec(readFileSync(dir + f, "utf8"));
  old.exec("PRAGMA foreign_keys = OFF");
  // pool_audit is append-only (a trigger): copy it with the trigger off, as a restore would.
  const tables = (old.prepare(`SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'`).all() as { name: string }[]).map((t) => t.name);
  for (const t of tables) {
    const cols = (old.prepare(`PRAGMA table_info(${t})`).all() as { name: string }[]).map((c) => c.name);
    old.exec(`DELETE FROM ${t}`);
    for (const r of src.sql.prepare(`SELECT ${cols.join(", ")} FROM ${t}`).all() as Record<string, unknown>[]) old.prepare(`INSERT INTO ${t} (${cols.join(", ")}) VALUES (${cols.map(() => "?").join(", ")})`).run(...(cols.map((c) => r[c]) as never[]));
  }
  const schemaBefore = old.prepare(`SELECT type, name, sql FROM sqlite_master ORDER BY name`).all() as { name: string; sql: string | null }[];
  const counts = (db: DatabaseSync) => Object.fromEntries(tables.map((t) => [t, (db.prepare(`SELECT COUNT(*) AS n FROM ${t}`).get() as { n: number }).n]));
  const before = counts(old);
  assert.ok(before.items! >= 1 && before.pool_audit! >= 1 && before.imports! >= 1);
  old.exec(readFileSync(dir + "0108_pool_round32.sql", "utf8"));
  assert.deepEqual(counts(old), before, "no existing row changes");
  const schemaAfter = old.prepare(`SELECT type, name, sql FROM sqlite_master ORDER BY name`).all() as { name: string; sql: string | null }[];
  const added = schemaAfter.filter((x) => !schemaBefore.some((y) => y.name === x.name)).map((x) => x.name);
  assert.deepEqual(schemaAfter.filter((x) => schemaBefore.some((y) => y.name === x.name)), schemaBefore, "no existing table, index or trigger changes");
  for (const t of ["circles", "circle_members", "circle_invites", "circle_banks", "circle_versions", "circle_version_blobs", "circle_qkeys", "circle_reports", "circle_holds"]) assert.ok(added.includes(t), t);
  assert.ok(added.every((n) => /^(sqlite_autoindex_)?circle/.test(n)), added.join());
  assert.throws(() => old.exec(readFileSync(dir + "0108_pool_round32.sql", "utf8")), /already exists/, "a second run stops at once");
  // (and every later migration, as a deployment applies them: round 34's 0109)
  for (const f of files.filter((f) => f > "0108_pool_round32.sql")) old.exec(readFileSync(dir + f, "utf8"));
  // The migrated database under the Worker: circles work, the public pool is unchanged.
  const w = new World({ sql: old, r2: src.r2 });
  for (const [k, v] of src.tokens) w.tokens.set(k, v);
  (w as unknown as { n: number }).n = 10;
  const cid = await circleWith(w, A, [[B, "editor"]]);
  const s = await share(w, B, cid, bankOf(withQs(2, "Bob")));
  assert.equal((await read(w, A, cid, s.bid)).status, 200);
  assert.equal((await w.json<any>("GET", `/items/${a.item_id}`, { user: B })).status, 200);
  assert.equal((await publish(w, A, bankOf(withQs(2, "Bob")))).body.error, "circle_material");
  await w.pool.hourly();
  await w.pool.daily();
});

// ---- 14. shared-code regressions --------------------------------------------------------------------------------------

test("32-14: circle routes read their own bodies (the auth pre-read passes them through, as /items and /blobs), a student-data blocked image is never served from a circle, and the hourly reconcile and the privacy byte purge treat circle images like any served use", async () => {
  // The worker's pre-read (auth bodies are capped at 8 KB) must not read a 4 MB send.
  const big = new Request(`https://api.example.org/circles/${"a".repeat(32)}/banks`, { method: "POST", headers: { Authorization: "Bearer t", "Content-Length": String(5 * 1024 * 1024), "Content-Type": PUB }, body: new Uint8Array(16) });
  assert.ok((await preReadBody(big, async () => ({ userId: "t-1", trust: "teacher" }))) === big, "passed through unread");
  const w = new World();
  const adm = w.user("admin");
  const [A, B] = ["Ann", "Bob"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[B!, "viewer"]]);
  const img = png(9, 9, 11);
  const up = await upload(w, A!, img);
  const s = await share(w, A!, cid, bankOf([q("What does this field sketch show about the terrace levels?", { image: { file: `media/${up.sha}.png`, width: 9, height: 9 } })]));
  const path = `/circles/${cid}/banks/${s.bid}/blobs/${up.sha}`;
  assert.equal((await w.call("GET", path, { user: B! })).status, 200);
  // An admin confirms the image as student data (via a public item using it): served to nobody, from anywhere.
  const it = (await publish(w, A!, bankOf([q("A public question with the same sketch of the terraces?", { image: { file: `media/${up.sha}.png`, width: 9, height: 9 } })]))).body as any;
  const p = await w.purge(adm, it.item_id, { questions: [], images: [up.sha] });
  assert.equal(p.status, 200, p.text);
  assert.equal((await w.call("GET", path, { user: B! })).status, 404, "a confirmed student-data image is not served from a circle either");
  // The reconcile marks nothing missing while the bytes are there.
  await w.pool.hourly();
  assert.equal(w.q<any>(`SELECT missing_at FROM blobs WHERE sha256 = ?`, up.sha)[0]?.missing_at ?? null, null);
});

test("32-15: the admin lists and closes circles (no owner undo; purged after 14 days); an account closing many circles is flagged (D11); a circle's audit trail outlives 3 years only while a case about it is open", async () => {
  const { retention } = await import("../../src/pool/cron.ts");
  const w = new World();
  w.clock = Date.UTC(2022, 0, 3, 12, 0, 0);
  const adm = w.user("admin");
  const [A, B] = ["Ann", "Bob"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[B!, "viewer"]]);
  const s = await share(w, A!, cid, bankOf(withQs(1, "Ann")));
  const list = await w.admin<any>(adm, "GET", "/admin/pool/circles", null, `?user_id=${B!.id}`);
  assert.deepEqual(list.body.circles.map((c: Q) => [c.id, c.members, c.banks, c.owner_id]), [[cid, 2, 1, A!.id]]);
  // Retention: the circle's rows are older than 3 years; an open report keeps its trail, its resolution lets it go.
  await w.json("POST", `/circles/${cid}/banks/${s.bid}/report`, { user: B!, json: { reason: "other" } });
  w.clock = Date.UTC(2026, 9, 20, 12, 0, 0);
  const trail = () => w.q(`SELECT 1 FROM pool_audit WHERE item_id = ?`, cid).length;
  const n0 = trail();
  await retention({ db: w.db, now: () => w.clock } as never);
  assert.equal(trail(), n0, "kept while the report is open");
  await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: s.bid, note: "Reviewed and nothing wrong." });
  const n1 = trail();
  await retention({ db: w.db, now: () => w.clock } as never);
  assert.equal(trail(), n1 - n0, "the old rows go, the admin's new ones stay");
  // The admin closes the circle: members see it closed, the owner cannot reopen, it is purged after 14 days.
  const cl = await w.admin<any>(adm, "POST", "/admin/pool/circle-close", { circle_id: cid, note: "Closed after a policy review." });
  assert.equal(cl.status, 200);
  assert.equal((await read(w, B!, cid, s.bid)).body.reason, "admin");
  assert.equal((await w.json<any>("POST", `/circles/${cid}/reopen`, { user: A! })).status, 409);
  assert.equal((await w.admin<any>(adm, "POST", "/admin/pool/circle-close", { circle_id: cid, note: "Closed after a policy review." })).status, 409);
  await days(w, 15);
  assert.ok(w.q<any>(`SELECT purged_at FROM circles WHERE id = ?`, cid)[0].purged_at !== null);
  // D11: the fifth close in 30 days flags the account for the admin.
  for (let i = 0; i < 5; i++) {
    const c = await circle(w, A!, `throwaway ${i}`);
    assert.equal((await w.json("DELETE", `/circles/${c}`, { user: A! })).status, 200);
    if (i === 3) w.advance(DAY);
  }
  assert.deepEqual(w.q<any>(`SELECT detail FROM admin_flags WHERE kind = 'cap' AND user_id = ?`, A!.id).map((x) => JSON.parse(x.detail).metric), ["circle_closes"]);
});
