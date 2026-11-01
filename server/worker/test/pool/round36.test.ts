// Round 36: circles after integration critic 44 (docs/critic-reports/2026-10-24-circles-phaseA-integration-critic44.md).
// Each test failed on round 35's code.
// B1. A circle created (or edited) without a description answered `"description": null`, which the client's plain
//     String rejects: nobody could open the circle. Every circle response now answers text as "" when there is none
//     (description, notes, messages, names); the only nulls left are documented ones (timestamps and states, a closed
//     circle's reason, the pending offer, an activity line's missing actor/member/title, and a hidden bank's text).
// Variant 2. GET /circles and GET /circles/:cid say who closed a circle: `closed_by` "owner" | "admin" | "last_member"
//     (null while open), so the app offers Reopen only for the owner's own close.
import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { framed, publishId, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 });
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "local", name: "b", questions: qs.map((x, i) => ({ ...x, id: `q${i}` })), created: 1, modified: 1 } });

/** Keys whose null is documented, anywhere. */
const NULLABLE = new Set([
  // timestamps and states
  "closed_at", "close_reason", "closed_by", "undo_until", "revoked_at", "removed_at", "left_at", "left_reason", "resolved_at", "action", "until", "ownership_offered_until",
  // the pending offer (an object or null) and paging cursors (null: no more pages)
  "pending_transfer", "next_cursor", "next_before",
]);
/** Keys an activity line (an object with `what`) may have null: a system action has no actor, a circle action no member, a hidden bank no title. */
const ACTIVITY_NULLABLE = new Set(["by", "member", "bank_title"]);
/** A hidden bank's text (an object with `hidden: true`). */
const HIDDEN_NULLABLE = new Set(["title", "subject", "course", "grade", "author_name"]);
function nulls(v: unknown, path = "$", out: string[] = []): string[] {
  if (Array.isArray(v)) v.forEach((x, i) => nulls(x, `${path}[${i}]`, out));
  else if (v && typeof v === "object") {
    const o = v as Q;
    for (const [k, x] of Object.entries(o)) {
      if (k === "payload") continue; // (a bank's own content: its question fields are the app's)
      if (x === null) {
        if (NULLABLE.has(k) || ("what" in o && ACTIVITY_NULLABLE.has(k)) || (o.hidden === true && HIDDEN_NULLABLE.has(k))) continue;
        out.push(`${path}.${k}`);
      } else nulls(x, `${path}.${k}`, out);
    }
  }
  return out;
}
const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};

test("36-1 (B1): a circle created without a description answers description \"\" (create, detail, list, edit), and no circle response carries an undocumented null", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [O, E, V, X] = ["Owner", "Eve", "Vic", "Xia"].map((n) => w.user("teacher", n));
  const seen: [string, unknown][] = [];
  const call = async (label: string, method: string, path: string, o: Q = {}) => {
    const r = await w.json<any>(method, path, o);
    seen.push([label, r.body]);
    return r;
  };
  const cr = await call("create", "POST", "/circles", { user: O, json: { name: "C44 no description" } });
  assert.equal(cr.status, 201);
  assert.equal(cr.body.circle.description, "", "create answer");
  const cid = cr.body.circle.id;
  assert.equal((await call("detail", "GET", `/circles/${cid}`, { user: O })).body.circle.description, "", "detail");
  assert.equal((await call("patch", "PATCH", `/circles/${cid}`, { user: O, json: { description: "" } })).body.description, "", "edit to blank");
  assert.equal((await call("patch null", "PATCH", `/circles/${cid}`, { user: O, json: { name: "Renamed", description: null } })).body.description, "");
  // Members, codes (no note), join, preview, offer, a bank (no message), its read, heads, activity with a system line.
  for (const [u, role] of [[E!, "editor"], [V!, "viewer"]] as const) {
    const k = (await call("invite", "POST", `/circles/${cid}/invites`, { user: O, json: { role } })).body.code;
    await call("preview", "POST", "/circles/join/preview", { user: u, json: { code: k } });
    await call("join", "POST", "/circles/join", { user: u, json: { code: k } });
    w.advance(1000);
  }
  await call("invites", "GET", `/circles/${cid}/invites`, { user: O });
  const meta: Q = { commit_id: publishId(), license: "circle-only", attest: { own_work: true, terms_version: POOL_TERMS_VERSION }, meta: { title: "Unit 2", subject: "", course: "", grade: "", tags: [], standards: [] } };
  const s = await call("send", "POST", `/circles/${cid}/banks`, { user: E!, body: framed(meta, bankOf([q("What carries sediment past the delta front?")])), contentType: PUB });
  const bid = s.body.bank_id;
  await call("read", "GET", `/circles/${cid}/banks/${bid}`, { user: V! });
  await call("heads", "POST", "/circles/heads", { user: V!, json: { bank_ids: [bid, "0".repeat(32)] } });
  await call("report", "POST", `/circles/${cid}/banks/${bid}/report`, { user: V!, json: { reason: "other" } });
  await call("offer", "POST", `/circles/${cid}/transfer`, { user: O, json: { user_id: E!.id } });
  await call("detail+offer", "GET", `/circles/${cid}`, { user: E! });
  await call("list", "GET", "/circles", { user: E! });
  // A hidden bank (an erased author who chose "do not keep"): its text is the documented null.
  const Z = w.user("teacher", "Zed");
  const kz = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: O, json: { role: "editor" } })).body.code;
  await w.json("POST", "/circles/join", { user: Z, json: { code: kz } });
  const zb = (await w.json<any>("POST", `/circles/${cid}/banks`, { user: Z, body: framed({ ...meta, commit_id: publishId() }, bankOf([q("Zed's question about levees and their breaches?")])), contentType: PUB })).body.bank_id;
  await w.json("PUT", "/items/prefs", { user: Z, json: { keep_on_delete: false } });
  await erase(w, Z);
  await call("detail owner, hidden bank", "GET", `/circles/${cid}`, { user: O });
  await call("activity", "GET", `/circles/${cid}/activity`, { user: V! });
  assert.ok(zb);
  // The export's circles section (a code without a note, a report without a note, a version without a message).
  w.sql.prepare(`INSERT INTO sessions (id, user_id, family_id, created_at, auth_at, expires_at, refresh_hash) VALUES ('s1', ?, ?, ?, ?, ?, 'h1')`).run(O.id, `fam-${O.id}`, w.clock, w.clock, w.clock + DAY);
  w.sql.prepare(`INSERT INTO sessions (id, user_id, family_id, created_at, auth_at, expires_at, refresh_hash) VALUES ('s2', ?, ?, ?, ?, ?, 'h2')`).run(V!.id, `fam-${V!.id}`, w.clock, w.clock, w.clock + DAY);
  w.sql.prepare(`INSERT INTO sessions (id, user_id, family_id, created_at, auth_at, expires_at, refresh_hash) VALUES ('s3', ?, ?, ?, ?, ?, 'h3')`).run(E!.id, `fam-${E!.id}`, w.clock, w.clock, w.clock + DAY);
  for (const u of [O, V!, E!]) seen.push([`export ${u.name}`, (await w.json<any>("GET", "/me/export", { user: u })).body.circles]);
  // A closed circle in the list and the detail.
  await w.admin(adm, "POST", "/admin/pool/circle-close", { circle_id: cid, note: "Closed after a policy review." });
  await call("list closed", "GET", "/circles", { user: V! });
  await call("detail closed", "GET", `/circles/${cid}`, { user: V! });
  const bad = seen.flatMap(([label, body]) => nulls(body).map((p) => `${label}: ${p}`));
  assert.deepEqual(bad, [], bad.join("\n"));
  assert.ok(seen.length >= 25);
  void X;
});

test("36-2 (variant 2): GET /circles and GET /circles/:cid say who closed a circle: closed_by owner | admin (null while open), so the app offers Reopen only for the owner's own close", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [O, M] = ["Owner", "Member"].map((n) => w.user("teacher", n));
  const mk = async (name: string) => {
    const cid = (await w.json<any>("POST", "/circles", { user: O, json: { name } })).body.circle.id;
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: O, json: { role: "viewer" } })).body.code;
    await w.json("POST", "/circles/join", { user: M, json: { code: k } });
    return cid;
  };
  const [a, b, c] = [await mk("Open"), await mk("By owner"), await mk("By admin")];
  assert.equal((await w.json("DELETE", `/circles/${b}`, { user: O })).status, 200);
  await w.admin(adm, "POST", "/admin/pool/circle-close", { circle_id: c, note: "Closed after a policy review." });
  for (const u of [O, M]) {
    const list = new Map(((await w.json<any>("GET", "/circles", { user: u })).body.circles as Q[]).map((x) => [x.id, x.closed_by]));
    assert.deepEqual([list.get(a), list.get(b), list.get(c)], [null, "owner", "admin"]);
    const det = await Promise.all([a, b, c].map(async (x) => (await w.json<any>("GET", `/circles/${x}`, { user: u })).body.circle.closed_by));
    assert.deepEqual(det, [null, "owner", "admin"]);
  }
  assert.equal((await w.json<any>("POST", `/circles/${c}/reopen`, { user: O })).status, 409, "an admin's close is not the owner's to undo");
});
