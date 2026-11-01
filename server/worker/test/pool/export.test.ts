// Round 23 (owner): GET /me/export, the account's own data and nothing else's.
import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { EXPORT_LIMITS } from "../../src/pool/export.ts";
import { publish, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "Pat Complainant", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const CN = (name: string) => ({ signature: `/s/ ${name}`, material_desc: "my own question", name, address: "1 Main St", phone: "555-0100", perjury_stmt: true, consent_jurisdiction: true, accept_service: true });
/** A live session for `u`, logged in at `authAt`. */
const login = (w: World, u: User, authAt = w.clock) =>
  w.sql.prepare(`INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at, user_agent) VALUES (?, ?, ?, ?, ?, ?, ?, 'TestBrowser/1')`).run(`s-${u.id}-${authAt}`, `fam-${u.id}`, u.id, `h-${u.id}-${authAt}`, authAt, authAt, authAt + 30 * DAY);
const exp = async (w: World, u: User | null, part?: number) => {
  const r = await w.call("GET", `/me/export${part ? `?part=${part}` : ""}`, u ? { user: u } : {});
  const text = await r.text();
  return { status: r.status, headers: r.headers, text, body: (() => { try { return JSON.parse(text); } catch { return null; } })() as any };
};

async function world() {
  const w = new World();
  const adm = w.user("admin"), Ann = w.user("teacher", "Ann Exporter"), Bob = w.user("teacher", "Bob Other"), Eve = w.user("teacher", "Eve Reporter");
  const HID = "Ann's hidden worked explanation: the gauge shows the crest two days after the storm.";
  const a1 = (await publish(w, Ann, bankOf([mcq("Ann's first exported question about deltas and their lobes?", [["Lobe", true], ["Mesa", false]], { explanation: HID })]), { hideKey: true, license: "CC-BY-NC-4.0" })).body as any;
  w.advance(DAY);
  const a1b = (await publish(w, Ann, bankOf(own(a1, [mcq("Ann's first exported question about deltas and their lobes?", [["Lobe", true], ["Mesa", false]], { explanation: HID, points: 2 })])), { hideKey: true, license: "CC-BY-NC-4.0", base: a1.publish_id }, a1.item_id)).body as any;
  const a2 = (await publish(w, Ann, bankOf([q("Ann's second exported question about moraines?")]), { visibility: "unlisted" })).body as any;
  const b = (await publish(w, Bob, bankOf([q("Bob's own question about clouds forming above warm lakes, with his secret?", { kind: { Short: { sample: "BOBSECRETANSWER" } } })]), { hideKey: true })).body as any;
  await w.json("POST", `/items/${b.item_id}/import`, { user: Ann });
  await w.json("POST", `/items/${b.item_id}/report`, { user: Ann, json: { reason: "wrong_answer", note: "Ann's own report note" } });
  await w.json("POST", `/items/${a1.item_id}/report`, { user: Eve, json: { reason: "other", note: "Eve's note about Ann" } });
  await w.admin(adm, "POST", "/admin/pool/remove", { item_id: a2.item_id, note: "ADMINNOTE internal" });
  await w.json("POST", `/items/${a2.item_id}/appeal`, { user: Ann, json: { note: "Ann's appeal note" } });
  const t = await w.takedown<any>(adm, await notice(w, adm), a1.item_id);
  await w.json("POST", `/items/${a1.item_id}/counter-notice`, { user: Ann, json: CN("Ann Exporter") });
  login(w, Ann);
  return { w, adm, Ann, Bob, Eve, a1, a1b, a2, b, HID, t };
}

test("export: every section of the account's own data, the FULL form of its items (its hidden keys), attachment headers, an audit row without content", async () => {
  const { w, Ann, a1, a1b, a2, b, HID, t } = await world();
  const r = await exp(w, Ann);
  assert.equal(r.status, 200, r.text.slice(0, 300));
  assert.match(r.headers.get("content-disposition") ?? "", /^attachment; filename="vershion-export\.json"$/);
  assert.equal(r.headers.get("cache-control"), "no-store");
  assert.equal(r.headers.get("x-content-type-options"), "nosniff");
  const x = r.body;
  assert.deepEqual([x.profile.id, x.profile.email, x.profile.display_name, x.profile.approval], [Ann.id, w.q<any>(`SELECT email FROM users WHERE id = ?`, Ann.id)[0].email, "Ann Exporter", "teacher"]);
  assert.deepEqual(x.items.map((i: any) => i.id).sort(), [a1.item_id, a2.item_id].sort());
  const i1 = x.items.find((i: any) => i.id === a1.item_id);
  assert.deepEqual(i1.versions.map((v: any) => [v.publish_id, v.version]), w.q<any>(`SELECT publish_id, version FROM item_publishes WHERE item_id = ? AND status IN ('current','previous') ORDER BY version`, a1.item_id).map((v) => [v.publish_id, v.version]));
  assert.equal(i1.state, "takedown");
  assert.ok(JSON.stringify(x.bodies[a1.item_id]).includes(HID), "her hidden explanation: she is the author");
  assert.equal(x.imports.length, w.q(`SELECT 1 FROM imports WHERE user_id = ?`, Ann.id).length);
  assert.deepEqual(x.imports.map((i: any) => i.item_id), [b.item_id]);
  assert.deepEqual(x.reports.map((i: any) => [i.item_id, i.note]), [[b.item_id, "Ann's own report note"]]);
  assert.deepEqual(x.takedowns.map((i: any) => i.action_id), [t.body.action_id]);
  assert.deepEqual(x.notices.map((n: any) => [n.kind, n.action_id]), [["takedown_author", t.body.action_id]]);
  assert.deepEqual(x.attestations.map((a: any) => a.publish_id).sort(), [a1.publish_id, a1b.publish_id, a2.publish_id].sort());
  assert.deepEqual(x.counter_notices.map((c: any) => [c.name, c.address]), [["Ann Exporter", "1 Main St"]]);
  assert.deepEqual(x.appeals.map((a: any) => [a.item_id, a.note]), [[a2.item_id, "Ann's appeal note"]]);
  assert.deepEqual(x.sessions.map((s: any) => s.user_agent), ["TestBrowser/1"]);
  assert.ok(x.sessions.every((s: any) => !("refresh_hash" in s) && !("id" in s) && !("family_id" in s)), "no tokens or token hashes");
  const au = w.q<any>(`SELECT detail, note FROM pool_audit WHERE action = 'account.export' AND actor_id = ?`, Ann.id);
  assert.equal(au.length, 1);
  assert.ok(!(au[0].detail ?? "").includes(HID) && au[0].note === null, "no content in the audit");
});

test("export: nothing of anyone else (emails, reporter identities, complainants, others' hidden keys), no admin notes, flags, hashes or sketches", async () => {
  const { w, Ann, Bob, Eve } = await world();
  const r = await exp(w, Ann);
  const others = [Bob, Eve].flatMap((u) => [u.id, w.q<any>(`SELECT email FROM users WHERE id = ?`, u.id)[0].email]);
  for (const s of [...others, "Eve's note about Ann", "Eve Reporter", "Pat Complainant", "legal@pub.example", "ADMINNOTE", "BOBSECRETANSWER"]) assert.ok(!r.text.includes(s), `leaks ${s}`);
  const hashes = w.q<any>(`SELECT match_hash, full_content, keyfree_content, near_full, tok_sk, kbands FROM item_questions`).flatMap((x) => Object.values(x)).filter((v): v is string => typeof v === "string" && v.length >= 16);
  for (const h of hashes) assert.ok(!r.text.includes(h), "no hash or sketch");
  assert.ok(!/"refresh_hash"|"pw_hash"|"stage_token"|"admin_flags"|"dedup"/.test(r.text));
});

test("export: three a day; anonymous 401; a pending account gets its profile only; a session older than 24 hours must sign in again; an erased account cannot export", async () => {
  const { w, Ann } = await world();
  for (let k = 0; k < 3; k++) assert.equal((await exp(w, Ann)).status, 200);
  const r4 = await exp(w, Ann);
  assert.deepEqual([r4.status, r4.body.error], [429, "daily_limit"]);
  assert.equal((await exp(w, null)).status, 401);
  const P = w.user("pending", "Pen Ding");
  login(w, P);
  const rp = await exp(w, P);
  assert.equal(rp.status, 200);
  assert.deepEqual([rp.body.profile_only, rp.body.items, rp.body.profile.approval], [true, undefined, "pending"]);
  assert.equal((await exp(w, P, 1)).status, 404);
  const U = w.user("unverified", "Un Verified");
  login(w, U);
  assert.equal((await exp(w, U)).status, 403);
  const S = w.user("teacher", "Stale Session");
  login(w, S, w.clock - 25 * 3600_000);
  assert.deepEqual([(await exp(w, S)).status, (await exp(w, S)).body.error], [403, "reauth_required"]);
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, Ann.id);
  await w.db.batch(deleteContentStatements(w.db, Ann.id, w.clock));
  assert.ok([401, 404].includes((await exp(w, Ann)).status), "an erased account has no export");
});

test("export: past the size cap the export is a manifest of parts (each part's bodies within the part size), parts fetched by number", async () => {
  const save = { ...EXPORT_LIMITS };
  EXPORT_LIMITS.whole = 2000;
  EXPORT_LIMITS.part = 1500;
  try {
    const w = new World();
    const A = w.user("teacher", "Ann");
    const ids: string[] = [];
    for (let k = 0; k < 4; k++) ids.push((await publish(w, A, bankOf([q(`Ann's question ${k} about rivers and the long words ${"delta ".repeat(40)}?`)]))).body.item_id as string);
    login(w, A);
    const r = await exp(w, A);
    assert.equal(r.status, 200);
    assert.equal(r.body.export_too_large, true);
    assert.equal(r.body.bodies, undefined);
    const parts = r.body.manifest.parts as { part: number; items: string[] }[];
    assert.ok(parts.length >= 2);
    assert.deepEqual(parts.flatMap((p) => p.items).sort(), [...ids].sort());
    const got: string[] = [];
    for (const p of parts) {
      const x = await exp(w, A, p.part);
      assert.equal(x.status, 200);
      assert.deepEqual(Object.keys(x.body.bodies).sort(), [...p.items].sort());
      got.push(...Object.keys(x.body.bodies));
    }
    assert.deepEqual(got.sort(), [...ids].sort());
    assert.equal((await exp(w, A, parts.length + 1)).status, 404);
    assert.equal(w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'exports'`, A.id)[0].count, 1, "parts do not count again");
  } finally {
    Object.assign(EXPORT_LIMITS, save);
  }
});
