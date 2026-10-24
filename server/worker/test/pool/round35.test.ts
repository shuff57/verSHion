// Round 35: circles after critic 42 (docs/critic-reports/2026-10-24-circles-round34-critic42.md). Each test failed on
// round 34's code.
// B1. The owner still finds REMOVED banks whatever their author's standing (an erased author's removed banks, a
//     suspended author's removed banks), so they can be purged at once; a removed bank that is held, or whose author is
//     not in good standing, is listed without its text (title, subject, course, grade, author name), as F1's generic hit.
// B2. Ownership offers are capped a day (`circle_transfer_offers`, 429 daily_limit); an identical repeated offer writes
//     nothing; offer and cancel audit rows are kept out of the circle's activity scan (not keyed by the circle id).
// Nitpicks: the export lists only OPEN offers; no SQL comment inside a fragment other statements embed.
import assert from "node:assert/strict";
import { test } from "node:test";
import { CODE_USABLE_SQL, IN_STANDING_SQL, AUTHOR_OK_SQL } from "../../src/pool/circles.ts";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { framed, publishId, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 });
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "local", name: "b", questions: qs.map((x, i) => ({ ...x, id: `q${i}` })), created: 1, modified: 1 } });

async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][]): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name: "Algebra team" } })).body.circle.id;
  for (const [u, role] of members) {
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } })).body.code;
    assert.equal((await w.json("POST", "/circles/join", { user: u, json: { code: k } })).status, 200);
    w.advance(1000);
  }
  return cid;
}
async function share(w: World, u: User, cid: string, title: string): Promise<string> {
  const meta: Q = { commit_id: publishId(), license: "circle-only", attest: { own_work: true, terms_version: POOL_TERMS_VERSION }, meta: { title, subject: "Math", course: "", grade: "", tags: [], standards: [] } };
  const r = await w.json<any>("POST", `/circles/${cid}/banks`, { user: u, body: framed(meta, bankOf([q(`${title}: what carries sediment past the delta front?`)])), contentType: PUB });
  assert.equal(r.status, 201, JSON.stringify(r.body));
  return r.body.bank_id;
}
const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};
const suspend = (w: World, u: User, on = true) => w.sql.prepare(`UPDATE users SET suspended_at = ? WHERE id = ?`).run(on ? w.clock : null, u.id);
const page = async (w: World, u: User, cid: string) => (await w.json<any>("GET", `/circles/${cid}`, { user: u })).body.banks as Q[];

test("35-1 (B1): the owner finds a removed bank whatever its author's standing (erased with keep_on_delete false; suspended) and can purge it at once; a removed bank that is held or whose author is not in good standing is listed without its text", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [O, E, S, V] = ["Owner", "Erin", "Sam", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[E!, "editor"], [S!, "editor"], [V!, "viewer"]]);
  const eb = await share(w, E!, cid, "Erin roster title");
  const sb = await share(w, S!, cid, "Sam roster title");
  const hb = await share(w, S!, cid, "Held roster title");
  // Erased author who chose "do not keep": the bank is removed (account_deleted), still counting toward the caps.
  assert.equal((await w.json("PUT", "/items/prefs", { user: E!, json: { keep_on_delete: false } })).status, 200);
  await erase(w, E!);
  // A held bank its author then removed; a removed bank of an author then suspended.
  await w.admin(adm, "POST", "/admin/pool/circle-hold", { bank_id: hb, note: "Student names in the title." });
  for (const b of [sb, hb]) assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${b}`, { user: S! })).status, 200);
  suspend(w, S!);
  const mine = await page(w, O!, cid);
  const by = new Map(mine.map((b) => [b.id, b]));
  assert.deepEqual([...by.keys()].sort(), [eb, hb, sb].sort(), "the owner sees all three removed banks");
  for (const b of [eb, sb, hb]) {
    const x = by.get(b)!;
    assert.ok(x.removed, JSON.stringify(x));
    assert.deepEqual([x.title, x.subject, x.author_name, x.hidden], [null, null, null, true], `listed without its text: ${JSON.stringify(x)}`);
  }
  assert.ok(!JSON.stringify(mine).includes("roster title"));
  assert.deepEqual(await page(w, V!, cid), [], "members see none of them");
  // The owner purges the erased author's bank at once (the suspended author's too); held: the admin decides.
  assert.equal((await w.json("POST", `/circles/${cid}/banks/${eb}/purge`, { user: O! })).status, 200);
  assert.equal((await w.json("POST", `/circles/${cid}/banks/${sb}/purge`, { user: O! })).status, 200);
  assert.equal((await w.json<any>("POST", `/circles/${cid}/banks/${hb}/purge`, { user: O! })).body.error, "under_moderation");
  // A removed bank of an author in good standing is still listed in full to the owner.
  w.sql.prepare(`UPDATE circle_members SET role = 'editor' WHERE user_id = ?`).run(V!.id);
  const ob2 = await share(w, V!, cid, "Plain title");
  assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${ob2}`, { user: V! })).status, 200);
  assert.equal((await page(w, O!, cid)).find((b) => b.id === ob2)!.title, "Plain title");
});

test("35-2 (B2): ownership offers are capped a day (429 daily_limit circle_transfer_offers); an identical repeated offer writes nothing and is not charged; offer and cancel rows stay out of the circle's activity scan", async () => {
  const w = new World();
  const [O, B, C] = ["Owner", "Bob", "Cy"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[B!, "editor"], [C!, "viewer"]]);
  const scanRange = () => w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM pool_audit WHERE item_id = ?`, cid)[0]!.n;
  const audits = () => w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM pool_audit`)[0]!.n;
  const range0 = scanRange();
  const first = await w.json<any>("POST", `/circles/${cid}/transfer`, { user: O!, json: { user_id: B!.id } });
  assert.equal(first.status, 202);
  const a1 = audits();
  w.advance(1000);
  const again = await w.json<any>("POST", `/circles/${cid}/transfer`, { user: O!, json: { user_id: B!.id } });
  assert.deepEqual([again.status, again.body.pending.expires_at], [202, first.body.pending.expires_at], "the same open offer");
  assert.equal(audits(), a1, "an identical repeated offer writes nothing");
  // 10 offers a day (the repeat above was not charged): 9 more alternate C and B, the 11th is refused.
  for (let i = 0; i < 9; i++) {
    assert.equal((await w.json("POST", `/circles/${cid}/transfer`, { user: O!, json: { user_id: i % 2 ? B!.id : C!.id } })).status, 202, `offer ${i + 2}`);
    assert.equal((await w.json<any>("POST", `/circles/${cid}/transfer/cancel`, { user: i % 2 ? B! : O! })).body.cancelled, true);
  }
  const over = await w.json<any>("POST", `/circles/${cid}/transfer`, { user: O!, json: { user_id: C!.id } });
  assert.deepEqual([over.status, over.body.error, over.body.metric], [429, "daily_limit", "circle_transfer_offers"]);
  // A cancel with nothing to cancel writes nothing.
  const a2 = audits();
  assert.equal((await w.json<any>("POST", `/circles/${cid}/transfer/cancel`, { user: O! })).body.cancelled, false);
  assert.equal(audits(), a2);
  // The circle's activity scan (pool_audit by item_id) saw none of the offers and cancels.
  assert.equal(scanRange(), range0, "offers and cancels are not in the activity scan's range");
  assert.ok(w.q(`SELECT 1 FROM pool_audit WHERE action IN ('circle.transfer_offer', 'circle.transfer_cancel')`).length >= 19, "they are audited (by account, with the circle in the detail)");
  // An accepted offer is activity.
  w.advance(DAY);
  assert.equal((await w.json("POST", `/circles/${cid}/transfer`, { user: O!, json: { user_id: B!.id } })).status, 202);
  assert.equal((await w.json("POST", `/circles/${cid}/transfer/accept`, { user: B! })).status, 200);
  assert.equal(scanRange(), range0 + 1);
  assert.equal((await w.json<any>("GET", `/circles/${cid}/activity`, { user: C! })).body.activity[0].what, "ownership_transferred");
});

test("35-3 (nitpicks): the export lists only open offers; the SQL fragments other statements embed carry no SQL comment (a flattened statement would lose the rest of its line)", async () => {
  for (const f of [CODE_USABLE_SQL("i", "?1"), IN_STANDING_SQL("?1"), AUTHOR_OK_SQL("b")]) assert.ok(!f.includes("--"), f);
  const w = new World();
  const [O, B] = ["Owner", "Bob"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[B!, "editor"]]);
  assert.equal((await w.json("POST", `/circles/${cid}/transfer`, { user: O!, json: { user_id: B!.id } })).status, 202);
  w.advance(8 * DAY);
  w.sql.prepare(`INSERT INTO sessions (id, user_id, family_id, created_at, auth_at, expires_at, refresh_hash) VALUES ('s1', ?, ?, ?, ?, ?, 'h')`).run(O!.id, `fam-${O!.id}`, w.clock, w.clock, w.clock + DAY);
  const ex = await w.json<any>("GET", "/me/export", { user: O! });
  assert.equal(ex.status, 200);
  assert.deepEqual(ex.body.circles.transfer_offers, [], "an expired offer is not open");
});
