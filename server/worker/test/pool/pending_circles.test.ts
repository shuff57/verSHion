// Round 54 (owner decision): a CONFIRMED account still waiting for approval (`pending`) may join a circle with a
// circle code and act by its role there. The public pool stays approval- and invite-gated, and creating a circle
// stays behind approval. A suspended, deleted or signed-out account is still refused (the auth package's
// authenticate, as the harness models it, answers null for those).
import assert from "node:assert/strict";
import { test } from "node:test";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { API, framed, png, publishId, upload, World, type User } from "./harness.ts";

const PUB = "application/vnd.vershion.publish";
const DAY = 86_400_000;
type Q = Record<string, any>;

const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const bankOf = (qs: Q[], name = "Circle bank") => JSON.stringify({ schema_version: 2, bank: { id: "local", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });

async function circle(w: World, u: User, name = "Algebra team"): Promise<string> {
  const r = await w.json<any>("POST", "/circles", { user: u, json: { name } });
  assert.equal(r.status, 201, JSON.stringify(r.body));
  return r.body.circle.id;
}
async function code(w: World, owner: User, cid: string, role: "editor" | "viewer" = "editor"): Promise<string> {
  const r = await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } });
  assert.equal(r.status, 201, JSON.stringify(r.body));
  return r.body.code;
}
/** The latest approval decision the auth package would read: rejected/revoked leave `trust` on `pending`. */
function decide(w: World, u: User, decision: "rejected" | "revoked", at = w.clock): void {
  w.sql
    .prepare(`INSERT INTO approval_requests (id, user_id, requested_at, decided_at, decided_by, decision) VALUES (?, ?, ?, ?, ?, ?)`)
    .run(crypto.randomUUID(), u.id, at, at, "admin", decision);
}
/** A re-application after a decision: a newer row with no decision (the only open one). */
function reapplyRow(w: World, u: User, at = w.clock): void {
  w.sql.prepare(`INSERT INTO approval_requests (id, user_id, requested_at, decided_at) VALUES (?, ?, ?, NULL)`).run(crypto.randomUUID(), u.id, at);
}

async function send(w: World, u: User, cid: string, payload: string, bid?: string): Promise<any> {
  const meta: Q = {
    commit_id: publishId(),
    license: "circle-only",
    attest: { own_work: true, terms_version: POOL_TERMS_VERSION },
    meta: { title: "Unit 2 quiz", subject: "Math", course: "", grade: "", tags: [], standards: [] },
    ...(bid ? { base_version: 1, confirm_large_change: true } : {}),
  };
  const r = await w.json<any>(bid ? "PUT" : "POST", bid ? `/circles/${cid}/banks/${bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, payload), contentType: PUB });
  assert.equal(r.status, bid ? 200 : 201, JSON.stringify(r.body));
  return r.body;
}

test("54-1: a confirmed pending account joins a circle with a code, then reads, sends, merges and invites by role", async () => {
  const w = new World();
  const owner = w.user("teacher", "Olive");
  // invited: false: a NEW account with no pool invite. A circle code alone must admit it.
  const P = w.user("pending", "Pat", { invited: false });
  const cid = await circle(w, owner);
  const invite = await code(w, owner, cid, "editor");

  // Join preview and join: both reach the handler (the pool-invite and approval gates do not stop a pending account).
  const preview = await w.json<any>("POST", "/circles/join/preview", { user: P, json: { code: invite } });
  assert.deepEqual([preview.status, preview.body.role, preview.body.circle.id], [200, "editor", cid]);
  const joined = await w.json<any>("POST", "/circles/join", { user: P, json: { code: invite } });
  assert.deepEqual([joined.status, joined.body.role], [200, "editor"]);

  // Its circle list and page.
  const list = await w.json<any>("GET", "/circles", { user: P });
  assert.equal(list.status, 200);
  assert.deepEqual(list.body.circles.map((c: Q) => [c.id, c.role]), [[cid, "editor"]]);
  assert.equal((await w.json<any>("GET", `/circles/${cid}`, { user: P })).status, 200);

  // The owner sends a bank; the pending editor reads it.
  const v1 = await send(w, owner, cid, bankOf([q("First question about cells and their parts?")]));
  const read = await w.json<any>("GET", `/circles/${cid}/banks/${v1.bank_id}`, { user: P });
  assert.equal(read.status, 200);
  assert.equal(read.body.bank.title, "Unit 2 quiz");

  // The pending editor sends a new version and initiates a merge from version 1.
  const v2 = await send(w, P, cid, bankOf([q("Second question about cells and their parts?")]), v1.bank_id);
  assert.equal(v2.version, 2);
  const merge = await w.json<any>("POST", `/circles/${cid}/banks/${v1.bank_id}/merge`, { user: P, json: { base_version: 1 } });
  assert.deepEqual([merge.status, merge.body.base_version, merge.body.head_version], [200, 1, 2]);

  // An editor cannot issue invites: the role check inside the handler is the real gate.
  const denied = await w.json<any>("POST", `/circles/${cid}/invites`, { user: P, json: { role: "viewer" } });
  assert.deepEqual([denied.status, denied.body.error], [403, "circle_role_required"]);

  // Ownership passes to the pending account (its standing counts for the offer), and then it may invite.
  const offered = await w.json<any>("POST", `/circles/${cid}/transfer`, { user: owner, json: { user_id: P.id } });
  assert.equal(offered.status, 202);
  assert.equal((await w.json<any>("POST", `/circles/${cid}/transfer/accept`, { user: P })).status, 200);
  const issued = await w.json<any>("POST", `/circles/${cid}/invites`, { user: P, json: { role: "viewer" } });
  assert.deepEqual([issued.status, issued.body.role], [201, "viewer"]);

  // A bank the pending account authored is served to the (approved) former owner: its standing counts for a circle.
  const own = await send(w, P, cid, bankOf([q("A bank the pending teacher wrote for the circle?")]));
  const byOwner = await w.json<any>("GET", `/circles/${cid}/banks/${own.bank_id}`, { user: owner });
  assert.equal(byOwner.status, 200, JSON.stringify(byOwner.body));
  assert.deepEqual([byOwner.body.bank.title, byOwner.body.bank.is_author], ["Unit 2 quiz", false]);
});

test("54-2: the public pool stays gated for a pending account, and creating a circle stays behind approval", async () => {
  const w = new World();
  const P = w.user("pending", "Pat", { invited: false });
  for (const [m, p, o] of [
    ["GET", "/items", {}],
    ["GET", "/items/mine", {}],
    ["POST", "/items", { body: "x\ny", contentType: PUB }],
    // A new circle confers ownership and invites: it waits for approval (owner decision).
    ["POST", "/circles", { json: { name: "Mine" } }],
  ] as const) {
    const x = await w.json<any>(m, p, { user: P, ...o });
    assert.deepEqual([x.status, x.body.error], [403, "account_not_approved"], `${m} ${p}`);
  }
  // Redeeming a pool invite does not open the public pool while the account is pending (round 28).
  const adm = w.user("admin");
  const inv = await w.admin<any>(adm, "POST", "/admin/pool/invites", {});
  assert.equal((await w.json<any>("POST", "/me/invite", { user: P, json: { code: inv.body.code } })).status, 200);
  assert.deepEqual([(await w.json<any>("GET", "/items", { user: P })).status], [403]);
});

test("54-3: unverified, suspended, deleted and signed-out accounts are still refused on circle routes", async () => {
  const w = new World();
  const U = w.user("unverified", "Una", { invited: false });
  assert.deepEqual([(await w.json<any>("GET", "/circles", { user: U })).status, (await w.json<any>("POST", "/circles/join", { user: U, json: { code: "C-ABCDEFGHJKMNPQRSTVWXYZ0" } })).status], [403, 403]);

  const S = w.user("pending", "Sam", { invited: false });
  w.sql.prepare(`UPDATE users SET suspended_at = ? WHERE id = ?`).run(w.clock, S.id);
  assert.equal((await w.json<any>("GET", "/circles", { user: S })).status, 401);
  assert.equal((await w.json<any>("POST", "/circles/join", { user: S, json: { code: "C-ABCDEFGHJKMNPQRSTVWXYZ0" } })).status, 401);

  const D = w.user("pending", "Dee", { invited: false });
  w.sql.prepare(`UPDATE users SET email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, D.id);
  assert.equal((await w.json<any>("GET", "/circles", { user: D })).status, 401);

  assert.equal((await w.json<any>("GET", "/circles", { user: null })).status, 401);
  assert.equal((await w.json<any>("POST", "/circles/join", { user: null, json: { code: "C-ABCDEFGHJKMNPQRSTVWXYZ0" } })).status, 401);
});

test("54-4: an approved account without the pool invite can still join a circle, but not the public pool", async () => {
  const w = new World();
  const owner = w.user("teacher", "Olive");
  const cid = await circle(w, owner);
  const invite = await code(w, owner, cid, "viewer");
  const N = w.user("teacher", "Nora", { invited: false });
  const pub = await w.json<any>("GET", "/items", { user: N });
  assert.deepEqual([pub.status, pub.body.error], [403, "invite_required"]);
  // A circle code admits her: the list and the join are open, and the circle page is hers once joined.
  assert.equal((await w.json<any>("GET", "/circles", { user: N })).status, 200);
  const join = await w.json<any>("POST", "/circles/join", { user: N, json: { code: invite } });
  assert.deepEqual([join.status, join.body.role], [200, "viewer"]);
  assert.equal((await w.json<any>("GET", `/circles/${cid}`, { user: N })).status, 200);
  // Creating a circle is still behind the pool invite.
  assert.equal((await w.json<any>("POST", "/circles", { user: N, json: { name: "x" } })).body.error, "invite_required");
  // Admins stay exempt.
  const adm = w.user("admin", "Site Admin", { invited: false });
  assert.equal((await w.json<any>("GET", "/items", { user: adm })).status, 200);
  assert.equal((await w.json<any>("GET", "/circles", { user: adm })).status, 200);
});

test("54-5: a rejected applicant and a revoked teacher are refused; a genuine first-decision applicant may join", async () => {
  const w = new World();
  const owner = w.user("teacher", "Olive");
  const cid = await circle(w, owner);
  const rejected = w.user("pending", "Rej", { invited: false });
  decide(w, rejected, "rejected");
  const revoked = w.user("pending", "Rev", { invited: false });
  decide(w, revoked, "revoked");
  const first = w.user("pending", "First", { invited: false });
  const reapply = w.user("pending", "Again", { invited: false });
  decide(w, reapply, "rejected", w.clock - DAY);
  reapplyRow(w, reapply, w.clock);

  for (const u of [rejected, revoked]) {
    const list = await w.json<any>("GET", "/circles", { user: u });
    assert.deepEqual([list.status, list.body.error], [403, "account_not_approved"], u.name);
    const join = await w.json<any>("POST", "/circles/join", { user: u, json: { code: await code(w, owner, cid, "viewer") } });
    assert.deepEqual([join.status, join.body.error], [403, "account_not_approved"], u.name);
  }
  // No decision yet, and a rejected applicant who re-applied after the cooldown (a newer open row): both wait again.
  for (const u of [first, reapply]) {
    assert.equal((await w.json<any>("GET", "/circles", { user: u })).status, 200, u.name);
    const join = await w.json<any>("POST", "/circles/join", { user: u, json: { code: await code(w, owner, cid, "viewer") } });
    assert.deepEqual([join.status, join.body.role], [200, "viewer"], u.name);
  }
});

test("54-6: a pending editor uploads an image and sends a circle bank with it", async () => {
  const w = new World();
  const owner = w.user("teacher", "Olive");
  const P = w.user("pending", "Pat", { invited: false });
  const cid = await circle(w, owner);
  assert.equal((await w.json<any>("POST", "/circles/join", { user: P, json: { code: await code(w, owner, cid, "editor") } })).status, 200);
  // The image upload a circle send makes is open to a confirmed pending member (content-addressed; the quota applies).
  const img = png(8, 8, 3);
  const up = await upload(w, P, img);
  assert.equal(up.status, 201, JSON.stringify(up.body));
  const sent = await send(w, P, cid, bankOf([q("What does the figure show about river meanders?", { image: { file: `media/${up.sha}.png`, width: 8, height: 8 } })]));
  assert.equal(sent.version, 1);
  const head = await w.json<any>("GET", `/circles/${cid}/banks/${sent.bank_id}`, { user: owner });
  assert.deepEqual([head.status, head.body.bank.has_images], [200, true]);
  assert.equal((await w.call("GET", `/circles/${cid}/banks/${sent.bank_id}/blobs/${up.sha}`, { user: owner })).status, 200);
  // The public pool stays gated: the upload exemption is not a publish route.
  assert.equal((await w.json<any>("GET", "/items", { user: P })).status, 403);
});

test("54-7: a pending non-member gets the counted 404, nothing normalises into a circle, and a code admits two accounts", async () => {
  const w = new World();
  const owner = w.user("teacher", "Olive");
  const cid = await circle(w, owner);
  const X = w.user("pending", "Xena", { invited: false });
  const ghost = "0".repeat(32);
  for (const id of [cid, ghost]) {
    const r = await w.json<any>("GET", `/circles/${id}`, { user: X });
    assert.deepEqual([r.status, r.body.error], [404, "not_found"], id);
  }
  assert.equal(w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'item_miss'`, X.id)[0].count, 2, "both misses counted");
  // URL normalisation cannot turn a circle path into the public pool: these become /items and stay refused.
  for (const path of ["/circles/../items", "/circles/%2e%2e/items"]) {
    const r = await w.json<any>("GET", path, { user: X });
    assert.deepEqual([r.status, r.body.error], [403, "account_not_approved"], path);
  }
  // A trailing slash or uppercase hex is not a circle route at all (the worker's outer router answers 404).
  const noRoute = async (path: string) => {
    const h = new Headers({ Authorization: `Bearer ${X.token}`, "CF-Connecting-IP": "10.0.0.1" });
    return await w.pool.handle(new Request(`${API}${path}`, { method: "GET", headers: h }));
  };
  assert.equal(await noRoute(`/circles/${cid}/`), null, "trailing slash");
  assert.equal(await noRoute(`/circles/${cid.toUpperCase()}`), null, "uppercase hex");
  assert.equal(await noRoute(`/circles/${cid.slice(0, 31)}`), null, "short id");
  assert.equal(await noRoute(`/circles/${cid}/../items`), null, "normalised to /circles/items, not a route");
  // Two confirmed pending accounts may use the same owner's code (its allowed uses).
  const shared = await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role: "viewer", max_uses: 2 } });
  assert.equal(shared.status, 201);
  for (const u of [w.user("pending", "One", { invited: false }), w.user("pending", "Two", { invited: false })]) {
    const join = await w.json<any>("POST", "/circles/join", { user: u, json: { code: shared.body.code } });
    assert.deepEqual([join.status, join.body.role], [200, "viewer"], u.name);
  }
});

test("54-8: a pending member's student-data report does not hide another member's bank", async () => {
  const w = new World();
  const owner = w.user("teacher", "Olive");
  const cid = await circle(w, owner);
  const P = w.user("pending", "Pat", { invited: false });
  const B = w.user("teacher", "Bea");
  assert.equal((await w.json<any>("POST", "/circles/join", { user: P, json: { code: await code(w, owner, cid, "viewer") } })).status, 200);
  assert.equal((await w.json<any>("POST", "/circles/join", { user: B, json: { code: await code(w, owner, cid, "viewer") } })).status, 200);
  const s = await send(w, owner, cid, bankOf([q("A question about the water cycle and its stages?")]));
  w.advance(2 * DAY);
  // A pending member's report is taken for the administrator, but never hides the bank at once (no approved standing).
  const rp = await w.json<any>("POST", `/circles/${cid}/banks/${s.bank_id}/report`, { user: P, json: { reason: "student_data" } });
  assert.equal(rp.status, 202);
  assert.equal(w.q<any>(`SELECT 1 FROM circle_reports WHERE bank_id = ? AND reporter_id = ? AND reason = 'student_data'`, s.bank_id, P.id).length, 1, "the report is recorded");
  assert.equal(w.q<any>(`SELECT 1 FROM circle_holds WHERE bank_id = ?`, s.bank_id).length, 0, "no hold from a pending member");
  // The same report from an approved, established member still hides it at once.
  assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${s.bank_id}/report`, { user: B, json: { reason: "student_data" } })).status, 202);
  assert.equal(w.q<any>(`SELECT 1 FROM circle_holds WHERE bank_id = ?`, s.bank_id).length, 1, "an approved member's report hides it");
});
