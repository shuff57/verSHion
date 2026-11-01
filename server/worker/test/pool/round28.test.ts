// Round 28 (owner decisions). 1. Retention: audit and takedown records older than 3 years go (daily,
// bounded), except what an open dispute, an appeal, a hold, a re-upload block in force or its upstream
// exemption snapshot still needs; pool_audit stays append-only for its first 3 years by the database's
// own clock. 2. Invite codes: pool use needs a redeemed admin-issued code besides the admin's approval;
// codes are stored hashed, single use or capped, may expire, can be revoked; existing approved accounts
// keep working. 3. The counter-notice is offered in /items/mine only inside its 30-day window, by the
// same rule the counter-notice route refuses with after it.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { createPool, deleteContentStatements } from "../../src/pool/index.ts";
import { RETENTION_ACTIONS, RETENTION_MS, RETENTION_ROWS } from "../../src/pool/cron.ts";
import { INVITE_ALPHABET, normaliseInviteCode } from "../../src/pool/invites.ts";
import { QUARANTINE_MS } from "../../src/pool/moderation.ts";
import { API, core, publish, SECRET, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "x", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
const code = (r: { status: number; body: Q }) => (r.status >= 400 ? `${r.status} ${r.body.error}` : String(r.status));
const notice = async (w: World, adm: User, complete = true) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: complete })).body.notice_id as string;
const CN = (name: string) => ({ signature: `/s/ ${name}`, material_desc: "my own question", name, address: "1 Main St", phone: "555-0100", perjury_stmt: true, consent_jurisdiction: true, accept_service: true });
const days = async (w: World, n: number) => {
  for (let d = 0; d < n; d++) {
    w.advance(DAY);
    await w.pool.hourly();
    await w.pool.daily();
  }
};
const holds = (w: World, id: string) => w.q<any>(`SELECT reason FROM item_holds WHERE item_id = ?`, id).map((h) => h.reason);
const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};
const count = (w: World, sql: string, ...a: unknown[]) => (w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM ${sql}`, ...a)[0]!.n);
const cronFailures = (w: World) => w.q<any>(`SELECT detail FROM admin_flags WHERE kind = 'cron_failure'`).map((f) => JSON.parse(f.detail).stage);
/** Rows of every table holding `needle` (case-insensitive). */
function scan(w: World, needle: string): Record<string, number> {
  const out: Record<string, number> = {};
  for (const { name } of w.q<{ name: string }>(`SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'`))
    for (const row of w.q(`SELECT * FROM "${name}"`))
      if (JSON.stringify(row, (_k, v) => (typeof v === "bigint" ? String(v) : v)).toLowerCase().includes(needle.toLowerCase())) out[name] = (out[name] ?? 0) + 1;
  return out;
}
const OLD = Date.UTC(2022, 0, 3, 12, 0, 0);
const NOW = Date.UTC(2026, 9, 20, 12, 0, 0);

// ---- 1. retention ------------------------------------------------------------------------------------

test("28-1: closed takedown records and audit rows older than 3 years go; open, active or disputed ones and everything younger stay", async () => {
  const w = new World();
  w.clock = OLD;
  const adm = w.user("admin");
  const [A, B, C, D, E, H] = ["Ann", "Bob", "Cat", "Dan", "Eve", "Hal"].map((n) => w.user("teacher", n));
  const pub = async (u: User, stem: string) => (await publish(w, u, bankOf([q(stem)]))).body as any;
  const a1 = await pub(A!, "Ann's question about the Feather river and the dam built above Oroville?");
  const a2 = await pub(B!, "Bob's question about the Yuba river gold rush towns and their tailings?");
  const a3 = await pub(C!, "Cat's question about Lassen Peak's 1915 eruption and its mudflows?");
  const a4 = await pub(D!, "Dan's question about the Sutter Buttes as an eroded volcanic dome?");
  const a5 = await pub(E!, "Eve's question about Butte Creek spring-run salmon and their pools?");
  const h = await pub(H!, "Hal's question about valley oak savanna and acorn woodpeckers?");
  const act = async (item: any, extra: Q = {}) => {
    const nid = await notice(w, adm);
    const t = await w.takedown<any>(adm, nid, item.item_id, { questions: [0], ...extra });
    assert.equal(t.status, 200, JSON.stringify(t.body));
    return { nid, id: t.body.action_id as string };
  };
  // T1 withdrawn (put back); T2 upheld (its 60-day blocks expire); T3 upheld with blocks for 10 years
  // (still in force); T4 counter-noticed, put back, its flag resolved; T5 the same, its flag still open.
  const t1 = await act(a1);
  assert.equal((await w.admin<any>(adm, "POST", "/admin/dmca/withdraw", { notice_id: t1.nid, note: "complainant withdrew" })).status, 200);
  const t2 = await act(a2);
  const t3 = await act(a3, { block_days: 3650 });
  const t4 = await act(a4);
  const t5 = await act(a5);
  assert.equal((await w.json<any>("POST", `/items/${a4.item_id}/counter-notice`, { user: D, json: CN("Dan") })).status, 202);
  assert.equal((await w.json<any>("POST", `/items/${a5.item_id}/counter-notice`, { user: E, json: CN("Eve") })).status, 202);
  await w.admin(adm, "POST", "/admin/pool/hold", { item_id: h.item_id, note: "under review" });
  const openNotice = await notice(w, adm);
  const deficient = await notice(w, adm, false);
  await days(w, 32);
  const state = (id: string) => w.q<any>(`SELECT restored_at IS NOT NULL AS r, purged_at IS NOT NULL AS p FROM dmca_actions WHERE id = ?`, id).map((x) => [x.r, x.p])[0];
  assert.deepEqual([t1, t2, t3, t4, t5].map((t) => state(t.id)), [[1, 0], [0, 1], [0, 1], [1, 0], [1, 0]], "closed: withdrawn, upheld, upheld, put back, put back");
  const flag4 = w.q<any>(`SELECT id FROM admin_flags WHERE kind = 'counter_notice' AND item_id = ?`, a4.item_id)[0].id;
  await w.admin(adm, "POST", "/admin/pool/resolve-flag", { flag_id: flag4 });
  const oldAudit = count(w, `pool_audit`);
  const snapshot = () => ({
    actions: w.q<any>(`SELECT id FROM dmca_actions ORDER BY id`).map((x) => x.id),
    notices: w.q<any>(`SELECT id FROM dmca_notices ORDER BY id`).map((x) => x.id),
    counter: count(w, `dmca_counter_notices`),
    strikes: count(w, `strikes`),
    blocks: count(w, `blocked_hashes`),
    audit: count(w, `pool_audit`),
  });
  // Two years and nine months later: nothing is old enough yet (and a little new activity).
  w.clock = OLD + 1000 * DAY;
  const before = snapshot();
  await w.pool.daily();
  const mid = snapshot();
  assert.deepEqual(mid, before, "younger than 3 years: nothing goes");
  await pub(A!, "Ann's newer question about the Bidwell Mansion and its 1868 builders?");
  await w.admin(adm, "POST", "/admin/pool/hold", { item_id: a1.item_id, note: "a later look" });
  await w.admin(adm, "POST", "/admin/pool/release", { item_id: a1.item_id, reason: "admin", note: "looked again: nothing wrong with it" });
  assert.equal(holds(w, a1.item_id).length, 0);
  // Past three years.
  w.clock = NOW;
  const cut = NOW - RETENTION_MS;
  const young = w.q<any>(`SELECT id FROM pool_audit WHERE at >= ?`, cut).map((x) => x.id);
  assert.ok(young.length > 0);
  await w.pool.hourly();
  await w.pool.daily();
  assert.deepEqual(cronFailures(w), [], "no stage failed");
  const kept = new Set(w.q<any>(`SELECT id FROM dmca_actions`).map((x) => x.id));
  assert.deepEqual([t1, t2, t3, t4, t5].map((t) => kept.has(t.id)), [false, false, true, false, true], "T3 (block in force) and T5 (open counter-notice flag) stay");
  for (const t of [t1, t2, t4]) {
    for (const tbl of ["dmca_counter_notices", "strikes", "dmca_exempt", "blocked_hashes", "dmca_upstream_keys"]) assert.equal(count(w, `${tbl} WHERE action_id = ?`, t.id), 0, `${tbl} of a removed action`);
    assert.equal(count(w, `dmca_notices WHERE id = ?`, t.nid), 0, "its notice goes once no action names it");
  }
  for (const t of [t3, t5]) {
    assert.equal(count(w, `strikes WHERE action_id = ?`, t.id), 1);
    assert.equal(count(w, `dmca_notices WHERE id = ?`, t.nid), 1);
  }
  assert.equal(count(w, `blocked_hashes WHERE action_id = ? AND lifted_at IS NULL`, t3.id) > 0, true, "T3's block in force stays");
  assert.equal(count(w, `dmca_counter_notices WHERE action_id = ?`, t5.id), 1, "the open dispute's counter-notice stays");
  assert.equal(count(w, `dmca_counter_notices WHERE action_id = ?`, t4.id), 0, "the closed one's (name, address, phone) is gone");
  assert.equal(count(w, `dmca_notices WHERE id = ?`, openNotice), 1, "an open notice can still be acted on: kept");
  assert.equal(count(w, `dmca_notices WHERE id = ?`, deficient), 0);
  assert.equal(count(w, `void_items WHERE item_id = ?`, a2.item_id), 1, "the upheld takedown's evidence void is licensing state, not a record: kept");
  // Audit: the old trail of a1/a2/a4 is gone; a3's and a5's (records kept) and the held item's stay; nothing young went.
  const oldOf = (item: string) => count(w, `pool_audit WHERE item_id = ? AND at < ?`, item, cut);
  assert.deepEqual([a1, a2, a4].map((x) => oldOf(x.item_id)), [0, 0, 0]);
  for (const x of [a3, a5, h]) assert.ok(oldOf(x.item_id) > 0, `kept: the trail of ${x.item_id}`);
  assert.equal(count(w, `pool_audit WHERE action = 'dmca.notice' AND json_extract(detail, '$.notice_id') = ?`, openNotice), 1, "the open notice's row stays");
  const keptItems = new Set([a3.item_id, a5.item_id, h.item_id]);
  const keptRefs = new Set([t3.nid, t5.nid, openNotice, t3.id, t5.id]);
  for (const r of w.q<any>(`SELECT action, item_id, detail FROM pool_audit WHERE at < ?`, cut)) {
    const d = r.detail ? JSON.parse(r.detail) : {};
    assert.ok(keptItems.has(r.item_id) || keptRefs.has(d.notice_id) || keptRefs.has(d.action_id), `an old row with no reason to stay: ${JSON.stringify(r)}`);
  }
  assert.ok(count(w, `pool_audit WHERE at < ?`, cut) < oldAudit, "old rows went");
  const left = new Set(w.q<any>(`SELECT id FROM pool_audit`).map((x) => x.id));
  assert.deepEqual(young.filter((id) => !left.has(id)), [], "no row younger than 3 years went");
  // The kept records still work: the admin lists them, the author's export still has her takedown.
  const listed = (await w.admin<any>(adm, "GET", "/admin/dmca/actions")).body;
  assert.ok(JSON.stringify(listed).includes(t3.id) && !JSON.stringify(listed).includes(t1.id));
  // Idempotent: another run removes nothing more.
  const after = snapshot();
  await w.pool.daily();
  const again = snapshot();
  assert.deepEqual({ ...again, audit: 0 }, { ...after, audit: 0 });
});

test("28-1: a block still in force keeps its takedown's upstream exemption snapshot (the original's republish is not held); once it expires both go", async () => {
  const B0 = q("Carl's benign question: name one tributary of the Sacramento river near Chico?");
  const T = mcq("PUBLISHER TEXTBOOK Q17: which of the following best describes the process of sediment deposition at a river mouth delta?", [["Velocity drops and load settles", true], ["Uplift", false], ["Glacial scour", false]]);
  for (const blockDays of [3650, 60]) {
    const w = new World();
    w.clock = OLD;
    const adm = w.user("admin"), C = w.user("teacher", "Carl"), M = w.user("teacher", "Mal"), Z = w.user("teacher", "Zed");
    const c: any = (await publish(w, C, bankOf([B0, T]), { license: "CC-BY-4.0" })).body;
    w.advance(DAY);
    await imp(w, M, c.item_id);
    const m: any = (await publish(w, M, bankOf([B0, T]), { license: "CC-BY-4.0" })).body;
    w.advance(DAY);
    const t = await w.takedown<any>(adm, await notice(w, adm), m.item_id, { questions: [1], block_days: blockDays });
    assert.equal(t.status, 200);
    const act = t.body.action_id as string;
    assert.ok(count(w, `dmca_upstream_keys WHERE action_id = ? AND item_id = ?`, act, c.item_id) > 0, "the snapshot");
    await days(w, 32); // upheld: closed
    assert.equal(w.q<any>(`SELECT purged_at IS NOT NULL AS p FROM dmca_actions WHERE id = ?`, act)[0].p, 1);
    w.clock = NOW;
    await w.pool.daily();
    const inForce = blockDays === 3650;
    assert.equal(count(w, `dmca_actions WHERE id = ?`, act), inForce ? 1 : 0, `block_days ${blockDays}`);
    assert.equal(count(w, `dmca_upstream_keys WHERE action_id = ?`, act) > 0, inForce, "the snapshot goes with its action, only then");
    w.advance(60_000);
    const r = await publish(w, C, bankOf(own(c, [{ ...B0, points: 3 }, T])), { license: "CC-BY-4.0", base: c.publish_id }, c.item_id);
    assert.equal(r.status, 200);
    assert.deepEqual(holds(w, c.item_id), [], "the original's republish of what it carried is never held");
    const z = await publish(w, Z, bankOf([T]), { license: "CC-BY-4.0" });
    assert.deepEqual(holds(w, z.body.item_id as string), inForce ? ["blocklist"] : [], "control: a stranger's re-upload is held exactly while the block is in force");
    assert.deepEqual(cronFailures(w), []);
  }
});

test("28-1: bounded: at most RETENTION_ACTIONS actions and RETENTION_ROWS audit rows a run, oldest first; the next run continues", async () => {
  const w = new World();
  const T0 = Date.UTC(2020, 5, 1);
  const ins = w.sql.prepare(`INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at, restored_at, outcome) VALUES (?, 'n', ?, 'u', ?, ?, ?, 'put_back')`);
  const extra = 5;
  for (let i = 0; i < RETENTION_ACTIONS + extra; i++) {
    const id = i.toString(16).padStart(32, "0");
    ins.run(id, `f${id.slice(1)}`, T0 + i, T0 + i, T0 + i + 1);
    w.sql.prepare(`INSERT INTO strikes (id, user_id, action_id, at, voided_at) VALUES (?, 'u', ?, ?, ?)`).run(`s${id.slice(1)}`, id, T0 + i, T0 + i + 1);
  }
  const a = w.sql.prepare(`INSERT INTO pool_audit (id, at, actor_kind, action) VALUES (?, ?, 'system', 'limits.set')`);
  for (let i = 0; i < RETENTION_ROWS + 3; i++) a.run(`a${i}`, T0 + i);
  a.run("young", w.clock);
  await w.pool.daily();
  assert.equal(count(w, `dmca_actions`), extra);
  assert.equal(count(w, `strikes`), extra);
  assert.deepEqual(w.q<any>(`SELECT MIN(disabled_at) AS m FROM dmca_actions`)[0].m, T0 + RETENTION_ACTIONS, "the oldest went first");
  assert.equal(count(w, `pool_audit WHERE at < ?`, T0 + RETENTION_ROWS + 3), 3);
  assert.equal(count(w, `pool_audit WHERE id = 'a0'`), 0);
  await w.pool.daily();
  assert.equal(count(w, `dmca_actions`), 0);
  assert.equal(count(w, `pool_audit WHERE at < ?`, T0 + RETENTION_ROWS + 3), 0);
  assert.equal(count(w, `pool_audit WHERE id = 'young'`), 1);
  assert.deepEqual(cronFailures(w), []);
});

test("28-1: pool_audit is append-only for 3 years by the DATABASE's clock: a young row cannot be deleted, a skewed application clock deletes nothing young and fails nothing", async () => {
  const w = new World();
  const now = Date.now();
  const a = w.sql.prepare(`INSERT INTO pool_audit (id, at, actor_kind, action) VALUES (?, ?, 'system', 'limits.set')`);
  a.run("young", now - 1000 * DAY);
  a.run("edge", now - RETENTION_MS + 60_000);
  a.run("old", now - RETENTION_MS - DAY);
  assert.throws(() => w.sql.prepare(`DELETE FROM pool_audit WHERE id = 'young'`).run(), /append-only/);
  assert.throws(() => w.sql.prepare(`DELETE FROM pool_audit WHERE id = 'edge'`).run(), /append-only/);
  assert.throws(() => w.sql.prepare(`UPDATE pool_audit SET action = 'x' WHERE id = 'old'`).run(), /append-only/, "updates stay refused");
  // An application clock ten years ahead: its cutoff would take everything; the database's floor holds.
  w.clock = now + 3650 * DAY;
  await w.pool.daily();
  assert.deepEqual(w.q<any>(`SELECT id FROM pool_audit WHERE id IN ('young', 'edge', 'old') ORDER BY id`).map((x) => x.id), ["edge", "young"]);
  assert.ok(!cronFailures(w).includes("retention"));
});

test("28-1: retention and the erasure resweep together: an erased author's old takedown loses its counter-notice PII either way, and no stage fails", async () => {
  const w = new World();
  w.clock = OLD;
  const adm = w.user("admin"), Z = w.user("teacher", "Zorgon Quill");
  const z = (await publish(w, Z, bankOf([q("Zorgon's question about tarns and cirques in the high Sierra?")]))).body as any;
  const t = await w.takedown<any>(adm, await notice(w, adm), z.item_id, { questions: [0] });
  assert.equal((await w.json<any>("POST", `/items/${z.item_id}/counter-notice`, { user: Z, json: CN("Zorgon Quill") })).status, 202);
  await days(w, 16); // put back after 10 business days
  await erase(w, Z);
  // A straggler written after the erasure (the round-27 race), and an invite redemption likewise.
  w.sql.prepare(`UPDATE dmca_counter_notices SET name = 'Zorgon Quill' WHERE action_id = ?`).run(t.body.action_id);
  w.sql.prepare(`INSERT OR IGNORE INTO pool_invite_redemptions (user_id, invite_id, at) VALUES (?, NULL, 0)`).run(Z.id);
  w.advance(DAY);
  await w.pool.daily();
  assert.deepEqual(scan(w, "Zorgon Quill"), {}, "the resweep scrubbed it");
  assert.equal(count(w, `pool_invite_redemptions WHERE user_id = ?`, Z.id), 0, "the resweep removes a redemption written after the erasure");
  // Three years on, the counter-notice flag is still open (the admin never resolved it): the record is
  // kept, scrubbed; once resolved, retention removes it.
  w.clock = NOW;
  await w.pool.daily();
  assert.equal(count(w, `dmca_counter_notices WHERE action_id = ? AND name = '[deleted]'`, t.body.action_id), 1);
  w.sql.prepare(`UPDATE dmca_counter_notices SET name = 'Zorgon Quill' WHERE action_id = ?`).run(t.body.action_id);
  const flag = w.q<any>(`SELECT id FROM admin_flags WHERE kind = 'counter_notice' AND item_id = ?`, z.item_id)[0].id;
  await w.admin(adm, "POST", "/admin/pool/resolve-flag", { flag_id: flag });
  w.advance(DAY);
  await w.pool.daily();
  assert.equal(count(w, `dmca_counter_notices`), 0);
  assert.equal(count(w, `dmca_actions WHERE id = ?`, t.body.action_id), 0);
  assert.deepEqual(scan(w, "Zorgon Quill"), {});
  assert.deepEqual(cronFailures(w), []);
});

// ---- 2. invite codes ---------------------------------------------------------------------------------

const issue = async (w: World, adm: User, body: Q = {}) => {
  const r = await w.admin<any>(adm, "POST", "/admin/pool/invites", body);
  assert.equal(r.status, 201, r.text);
  return r.body as { invite_id: string; code: string; max_uses: number; expires_at: number | null };
};
const redeem = (w: World, u: User, c: unknown) => w.json<any>("POST", "/me/invite", { user: u, json: { code: c } });
const browse = (w: World, u: User) => w.json<any>("GET", "/items?scope=public", { user: u });

test("28-2: a new teacher cannot use the pool until it redeems an admin's code; the code is shown once and stored only hashed; admins and existing accounts are unaffected", async () => {
  const w = new World();
  const adm = w.user("admin", "Ada", { invited: false }), old = w.user("teacher", "Olga"), N = w.user("teacher", "Nina", { invited: false });
  assert.equal((await browse(w, adm)).status, 200, "admins are exempt");
  assert.equal((await browse(w, old)).status, 200, "an account approved before the gate keeps working");
  const refused = await browse(w, N);
  assert.equal(code(refused), "403 invite_required");
  assert.match(refused.body.message, /invite/);
  for (const path of ["/items/mine", "/items/prefs"]) assert.equal(code(await w.json<any>("GET", path, { user: N })), "403 invite_required", path);
  w.sql.prepare(`INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at, user_agent) VALUES (?, ?, ?, ?, ?, ?, ?, 'T/1')`).run(`s-${N.id}`, `fam-${N.id}`, N.id, `h-${N.id}`, w.clock, w.clock, w.clock + 30 * DAY);
  assert.equal((await w.json<any>("GET", "/me/export", { user: N })).status, 200, "its own data export stays open");
  assert.equal(code(await redeem(w, N, "not a code")), "400 invalid_request");
  const inv = await issue(w, adm, { note: "pilot: Chico High science" });
  assert.match(inv.code, /^([0-9A-HJKMNP-TV-Z]{4}-){5}[0-9A-HJKMNP-TV-Z]{4}$/);
  assert.equal(inv.max_uses, 1);
  assert.equal(inv.expires_at, null);
  const norm = normaliseInviteCode(inv.code)!;
  assert.deepEqual(scan(w, inv.code), {}, "the code is stored nowhere");
  assert.deepEqual(scan(w, norm), {}, "nor its normalised form");
  assert.equal(count(w, `pool_invites WHERE id = ? AND length(code_hash) = 64`, inv.invite_id), 1);
  // Typed loosely: lower case, spaces, O for 0 and I/L for 1 are read as the code.
  const loose = inv.code.toLowerCase().replaceAll("-", " ").replaceAll("0", "o").replaceAll("1", "l");
  const r = await redeem(w, N, loose);
  assert.deepEqual([r.status, r.body], [200, { ok: true, redeemed: true }]);
  assert.equal((await browse(w, N)).status, 200);
  assert.deepEqual((await redeem(w, N, inv.code)).body, { ok: true, already: true }, "a second redemption uses nothing");
  assert.equal(w.q<any>(`SELECT uses FROM pool_invites WHERE id = ?`, inv.invite_id)[0].uses, 1);
  // Audited (both logs for the admin's write; never the code).
  assert.deepEqual(w.q<any>(`SELECT action, actor_id, target_user_id FROM pool_audit WHERE action LIKE 'invite.%' ORDER BY at, action`).map((x) => [x.action, x.actor_id, x.target_user_id]), [
    ["invite.issue", adm.id, null],
    ["invite.redeem", N.id, N.id],
  ]);
  assert.ok((w.adminAudits as any[]).some((e) => e.action === "pool.invite_issue" && e.targetId === inv.invite_id));
  assert.ok(!JSON.stringify(w.adminAudits).includes(norm.slice(0, 8)));
  // The admin sees who redeemed it, and an account's standing.
  const one = await w.admin<any>(adm, "GET", "/admin/pool/invites", null, `?id=${inv.invite_id}`);
  assert.deepEqual([one.body.invite.state, one.body.invite.uses, one.body.invite.redeemed_by.map((x: any) => x.user_id)], ["used_up", 1, [N.id]]);
  assert.equal((await w.admin<any>(adm, "GET", "/admin/pool/invites", null, `?user_id=${N.id}`)).body.redemption.invite_id, inv.invite_id);
  assert.equal((await w.admin<any>(adm, "GET", "/admin/pool/invites", null, `?user_id=${old.id}`)).body.redemption.grandfathered, true);
  assert.equal((await w.admin<any>(adm, "GET", "/admin/pool/invites", null, `?user_id=${adm.id}`)).body.redemption, null);
  assert.equal((await w.admin<any>(adm, "GET", "/admin/pool/invites")).body.invites.length, 1);
});

test("28-2: single use or a use cap, an optional expiry, revocation; every refusal is the same invite_invalid; a revoked code never undoes a redemption", async () => {
  const w = new World();
  const adm = w.user("admin");
  const us = Array.from({ length: 7 }, (_, i) => w.user("teacher", `New ${i}`, { invited: false }));
  const once = await issue(w, adm);
  assert.equal((await redeem(w, us[0]!, once.code)).status, 200);
  assert.equal(code(await redeem(w, us[1]!, once.code)), "422 invite_invalid", "single use");
  const two = await issue(w, adm, { max_uses: 2, expires_in_days: 3 });
  assert.equal(two.expires_at, w.clock + 3 * DAY);
  assert.equal((await redeem(w, us[1]!, two.code)).status, 200);
  assert.equal((await redeem(w, us[2]!, two.code)).status, 200);
  assert.equal(code(await redeem(w, us[3]!, two.code)), "422 invite_invalid", "the cap");
  const exp = await issue(w, adm, { max_uses: 5, expires_in_days: 1 });
  w.advance(DAY);
  assert.equal(code(await redeem(w, us[3]!, exp.code)), "422 invite_invalid", "expired (at the instant it ends)");
  const rev = await issue(w, adm, { max_uses: 5 });
  assert.equal((await redeem(w, us[4]!, rev.code)).status, 200);
  assert.equal((await w.admin<any>(adm, "POST", "/admin/pool/invites/revoke", { invite_id: rev.invite_id, note: "posted in a staff chat" })).status, 200);
  assert.equal(code(await redeem(w, us[5]!, rev.code)), "422 invite_invalid", "revoked");
  assert.equal((await browse(w, us[4]!)).status, 200, "the earlier redeemer keeps access (revoke-teacher or suspend is the way to cut an account off)");
  assert.equal((await w.admin<any>(adm, "POST", "/admin/pool/invites/revoke", { invite_id: rev.invite_id })).status, 409);
  assert.equal((await w.admin<any>(adm, "POST", "/admin/pool/invites/revoke", { invite_id: "f".repeat(32) })).status, 404);
  assert.equal(count(w, `pool_audit WHERE action = 'invite.revoke'`), 1);
  const unknown = Array.from({ length: 24 }, () => INVITE_ALPHABET[0]).join("");
  assert.equal(code(await redeem(w, us[5]!, unknown)), "422 invite_invalid", "an unknown code: the same answer");
  // Validation of the admin's input.
  for (const body of [{ max_uses: 0 }, { max_uses: 1001 }, { max_uses: 1.5 }, { expires_in_days: 0 }, { expires_in_days: 366 }, { max_uses: "3" }])
    assert.equal((await w.admin<any>(adm, "POST", "/admin/pool/invites", body)).status, 400, JSON.stringify(body));
  const states = (await w.admin<any>(adm, "GET", "/admin/pool/invites")).body.invites.map((x: any) => x.state).sort();
  assert.deepEqual(states, ["expired", "revoked", "used_up", "used_up"]);
  // The last use, raced by two accounts: exactly one wins.
  const last = await issue(w, adm);
  const [x, y] = await Promise.all([redeem(w, us[5]!, last.code), redeem(w, us[6]!, last.code)]);
  assert.deepEqual([x.status, y.status].sort(), [200, 422]);
  assert.equal(w.q<any>(`SELECT uses FROM pool_invites WHERE id = ?`, last.invite_id)[0].uses, 1);
});

test("28-2: redemption attempts are a daily allowance; a pending applicant can redeem before approval and needs both", async () => {
  const w = new World();
  const adm = w.user("admin"), P = w.user("pending", "Pat", { invited: false }), G = w.user("teacher", "Gus", { invited: false });
  const wrong = (i: number) => INVITE_ALPHABET.slice(0, 20) + String(i).padStart(4, "0");
  for (let i = 0; i < 10; i++) assert.equal(code(await redeem(w, G, wrong(i))), "422 invite_invalid");
  const inv = await issue(w, adm);
  assert.equal(code(await redeem(w, G, inv.code)), "429 daily_limit", "the 11th try of the day");
  w.advance(DAY);
  assert.equal((await redeem(w, G, inv.code)).status, 200, "the next day");
  // Pending: the redemption works, the pool does not (approval is still required), then both.
  assert.equal(code(await browse(w, P)), "403 account_not_approved");
  const inv2 = await issue(w, adm);
  assert.equal((await redeem(w, P, inv2.code)).status, 200);
  assert.equal(code(await browse(w, P)), "403 account_not_approved");
  w.setTrust(P, "teacher");
  assert.equal((await browse(w, P)).status, 200);
  // Approved with no code: refused until one is redeemed.
  const Q2 = w.user("pending", "Quin", { invited: false });
  w.setTrust(Q2, "teacher");
  assert.equal(code(await browse(w, Q2)), "403 invite_required");
});

test("28-2: migration 0106 grandfathers exactly the accounts approved when it runs (teacher or admin, not deleted); a later approval needs a code", async () => {
  const here = new URL("../..", import.meta.url).pathname;
  const db = new DatabaseSync(":memory:");
  const files = readdirSync(`${here}migrations`).filter((x) => x.endsWith(".sql")).sort();
  for (const f of files.filter((x) => x < "0106")) db.exec(readFileSync(`${here}migrations/${f}`, "utf8"));
  const id = (n: number) => `${String(n).padStart(8, "0")}-0000-4000-8000-9${String(n).padStart(11, "0")}`;
  const add = db.prepare(`INSERT INTO users (id, email, email_verified_at, pw_hash, display_name, trust, created_at) VALUES (?, ?, 1, 'x', ?, ?, 1)`);
  add.run(id(1), "t@school.example.org", "Tia", "teacher");
  add.run(id(2), "a@school.example.org", "Abe", "admin");
  add.run(id(3), "p@school.example.org", "Pia", "pending");
  db.prepare(`INSERT INTO users (id, email_verified_at, trust, created_at, deleted_at) VALUES (?, 1, 'teacher', 1, 5)`).run(id(4));
  for (const f of files.filter((x) => x >= "0106")) db.exec(readFileSync(`${here}migrations/${f}`, "utf8"));
  assert.deepEqual((db.prepare(`SELECT user_id, invite_id FROM pool_invite_redemptions ORDER BY user_id`).all() as any[]).map((x) => [x.user_id, x.invite_id]), [[id(1), null], [id(2), null]]);
  const w = new World({ sql: db });
  const tok = (n: number) => (w.tokens.set(`t${n}`, id(n)), { id: id(n), token: `t${n}`, name: "" });
  assert.equal((await browse(w, tok(1))).status, 200);
  db.prepare(`UPDATE users SET trust = 'teacher' WHERE id = ?`).run(id(3));
  assert.equal(code(await browse(w, tok(3))), "403 invite_required");
});

test("28-2: erasure removes the account's redemption; a request still authenticated after it is a 401, never a 403 or a write", async () => {
  const w = new World();
  const adm = w.user("admin"), N = w.user("teacher", "Nora", { invited: false });
  const inv = await issue(w, adm, { max_uses: 3 });
  assert.equal((await redeem(w, N, inv.code)).status, 200);
  await erase(w, N);
  assert.equal(count(w, `pool_invite_redemptions WHERE user_id = ?`, N.id), 0);
  assert.equal(w.q<any>(`SELECT uses FROM pool_invites WHERE id = ?`, inv.invite_id)[0].uses, 1, "the use count stays");
  // An authentication layer that still believes the token (the round-27 race).
  const p2 = createPool({
    db: w.db,
    r2: w.r2 as any,
    core,
    secret: SECRET,
    allowedOrigins: [],
    now: () => w.clock,
    rateStore: { hit: async () => ({ allowed: true, retryAfter: 0 }), undo: async () => {} } as any,
    ipBucket: (ip: string) => ip,
    authenticate: async () => ({ userId: N.id, sessionFamily: "f", trust: "teacher" }),
    notify: async () => {},
    background: () => {},
  } as any);
  const call = async (method: string, path: string, body?: unknown) => {
    const h = new Headers({ Authorization: "Bearer x", "CF-Connecting-IP": "10.0.0.9" });
    let b: string | undefined;
    if (body !== undefined) {
      b = JSON.stringify(body);
      h.set("Content-Type", "application/json");
      h.set("Content-Length", String(new TextEncoder().encode(b).length));
    }
    return (await p2.handle(new Request(`${API}/${path.replace(/^\//, "")}`, { method, headers: h, body: b })))!.status;
  };
  assert.equal(await call("GET", "/items?scope=public"), 401);
  assert.equal(await call("POST", "/me/invite", { code: inv.code }), 401);
  assert.equal(count(w, `pool_invite_redemptions WHERE user_id = ?`, N.id), 0, "nothing written");
  assert.equal(w.q<any>(`SELECT uses FROM pool_invites WHERE id = ?`, inv.invite_id)[0].uses, 1, "no use taken");
});

// ---- 3. the counter-notice window in /items/mine ------------------------------------------------------

const mineRow = async (w: World, u: User, id: string) => (await w.json<any>("GET", "/items/mine", { user: u })).body.items.find((x: any) => x.id === id);

test("28-3: a live takedown offers the counter-notice (with its end) only inside the 30-day window; at its end mine says closed and the route refuses, with no cron run in between", async () => {
  for (const at of ["inside", "end"] as const) {
    const w = new World();
    const adm = w.user("admin"), A = w.user("teacher", "Ann");
    const a = (await publish(w, A, bankOf([q("Ann's question about the Chico Creek nature preserve and its alders?")]))).body as any;
    const t0 = w.clock;
    assert.equal((await w.takedown<any>(adm, await notice(w, adm), a.item_id, { questions: [0] })).status, 200);
    let row = await mineRow(w, A, a.item_id);
    assert.deepEqual([row.state, row.counter_notice, row.counter_notice_by], ["taken_down", "available", t0 + QUARANTINE_MS]);
    w.clock = t0 + QUARANTINE_MS - (at === "inside" ? 1 : 0);
    row = await mineRow(w, A, a.item_id);
    if (at === "inside") {
      assert.deepEqual([row.counter_notice, row.counter_notice_by], ["available", t0 + QUARANTINE_MS]);
      assert.equal((await w.json<any>("POST", `/items/${a.item_id}/counter-notice`, { user: A, json: CN("Ann") })).status, 202);
      row = await mineRow(w, A, a.item_id);
      assert.deepEqual([row.counter_notice, row.counter_notice_by], ["received", undefined]);
    } else {
      assert.deepEqual([row.state, row.counter_notice, row.counter_notice_by], ["taken_down", "closed", undefined], "closed at the instant the route starts refusing");
      assert.equal(code(await w.json<any>("POST", `/items/${a.item_id}/counter-notice`, { user: A, json: CN("Ann") })), "409 window_closed");
      assert.equal(count(w, `dmca_counter_notices`), 0);
    }
  }
});

test("28-3: a ghost takedown (item purged before the notice) says whether the counter-notice is available, by the same rule", async () => {
  for (const at of ["inside", "end"] as const) {
    const w = new World();
    const adm = w.user("admin"), A = w.user("teacher", "Ann");
    const a = (await publish(w, A, bankOf([q("Ann's own question about the vernal pools of the Chico plains?")]))).body as any;
    await w.json("DELETE", `/items/${a.item_id}`, { user: A });
    await days(w, 16);
    const t0 = w.clock;
    assert.equal((await w.takedown<any>(adm, await notice(w, adm), a.item_id, { questions: "all" })).status, 200);
    let row = await mineRow(w, A, a.item_id);
    assert.deepEqual([row.state, row.counter_notice, row.counter_notice_by, row.counter_notice_sent], ["takedown_pending", "available", t0 + QUARANTINE_MS, false]);
    w.clock = t0 + QUARANTINE_MS - (at === "inside" ? 1 : 0);
    row = await mineRow(w, A, a.item_id);
    const post = await w.json<any>("POST", `/items/${a.item_id}/counter-notice`, { user: A, json: CN("Ann") });
    if (at === "inside") {
      assert.equal(row.counter_notice, "available");
      assert.equal(post.status, 202);
      assert.equal((await mineRow(w, A, a.item_id)).counter_notice, "received");
    } else {
      assert.equal(row.counter_notice, "closed");
      assert.equal(code(post), "409 window_closed");
    }
  }
});
