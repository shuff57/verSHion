// Round 48: circles phase C after critics 55a (product) and 55b (security), docs/critic-reports/2026-10-25-circles-phaseC-*55*.
// S-B1: every answer-key PATCH that does work counts toward a day's attempts (refusals too), checked cheaply first.
// S-B2: the evidence bound keeps what an administrator's hold or an open report protects, ranks per sender (one
// account's flagged sends cannot displace another member's evidence), and the owner's remedy for a removed bank whose
// evidence an administrator has captured is a full purge (before that, purge-now frees the rest and says so: 200
// purged_partial). Limits: the keyless message fits short-answer questions; a hide_key change during a send or a
// rollback is 409 hide_key_changed; the PATCH's own lost race names the head; versions withheld past the day's flags are
// named on a bank-level cap flag and released by an unblock like flagged ones; pruned evidence is said as such; trim
// lists the evidence it keeps; an email invite to an address with an open invite rotates its code; neutral wording for
// a mail domain's hour and the inviter's own share; erasure clears the inviter's hourly domain rows; one role hint.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { test } from "node:test";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import type { PoolNotice } from "../../src/pool/shared.ts";
import { framed, publish, publishId, World, type User } from "./harness.ts";

const DAY = 86_400_000;
const HOUR = 3_600_000;
type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "SAMPLEKEYzzq" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "local", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const STEM = "Copyrighted textbook problem 4.7: which organelle in the eukaryotic cell produces most of the ATP in respiration?";
const LAB = (extra: Q = {}) => mcq(STEM, [["A. Mitochondrion", true], ["B. Ribosome", false], ["C. Golgi body", false], ["D. Lysosome", false]], extra);
const UNL = () => mcq(STEM, [["Mitochondrion", true], ["Ribosome", false], ["Golgi body", false], ["Lysosome", false]]);
const TEXTBOOK = "Copyrighted textbook problem 9.2: model the white-tailed deer population of the reserve with a yearly harvest of 12 percent.";
const CLEAN = "Clean question about eskers and drumlins and the glaciers that left them?";
/** share-core's keyless sentinel, read from its source (wire.rs KEYLESS_SENTINEL): a test never drifts from it. */
const SENTINEL = (() => {
  const src = readFileSync(new URL("../../../share-core/src/wire.rs", import.meta.url), "utf8");
  const m = /pub const KEYLESS_SENTINEL: &str =\s*"([^"]+)";/.exec(src);
  if (!m) throw new Error("KEYLESS_SENTINEL not found in share-core/src/wire.rs");
  return m[1]!;
})();
type Mail = Extract<PoolNotice, { kind: "circle_email_invite" }>;
const mails = (w: World) => w.notices.filter((n) => n.kind === "circle_email_invite") as Mail[];
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const limit = (w: World, metric: string, value: number, who = "*") =>
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, ?, ?, 0, 'test') ON CONFLICT (user_id, metric) DO UPDATE SET value = excluded.value`).run(who, metric, value);
async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][], name = "Algebra team"): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name } })).body.circle.id;
  for (const [u, role] of members) {
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } })).body.code;
    assert.equal((await w.json("POST", "/circles/join", { user: u, json: { code: k } })).status, 200);
    w.advance(1000);
  }
  return cid;
}
async function sendRaw(w: World, u: User, cid: string, payload: string, o: { bid?: string; base?: number; extra?: Q; confirm?: boolean } = {}) {
  const meta: Q = {
    commit_id: publishId(),
    license: "circle-only",
    attest: { own_work: true, terms_version: POOL_TERMS_VERSION },
    meta: { title: "Unit 2", subject: "Math", course: "", grade: "", tags: [], standards: [] },
    ...(o.bid ? { base_version: o.base } : {}),
    ...(o.confirm ? { confirm_large_change: true } : {}),
    ...(o.extra ?? {}),
  };
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, payload), contentType: PUB });
}
const patch = (w: World, u: User, cid: string, bid: string, body: Q) => w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: u, json: body });
const invite = (w: World, u: User, cid: string, body: Q) => w.json<any>("POST", `/circles/${cid}/invites/email`, { user: u, json: body });
const read = (w: World, u: User, path: string) => w.call("GET", path, { user: u });
const flags = (w: World, bid: string, kind = "blocklist") => w.q<any>(`SELECT * FROM admin_flags WHERE kind = ? AND item_id = ? ORDER BY at, id`, kind, bid);
const kept = (w: World, bid: string) => w.q<any>(`SELECT version, withheld_at IS NOT NULL AS w FROM circle_versions WHERE bank_id = ? AND status IN ('current', 'previous') AND full_key <> '' ORDER BY version`, bid);
async function world() {
  const w = new World();
  const adm = w.user("admin");
  const [O, A, E, V, P] = ["Olga", "Ann", "Eli", "Vic", "Pat"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"], [V!, "viewer"]]);
  return { w, adm, O: O!, A: A!, E: E!, V: V!, P: P!, cid };
}
async function takedownOf(w: World, adm: User, P: User, qs: Q[]) {
  const item = (await publish(w, P, bankOf(qs))).body as Q;
  const td = await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: qs.map((_, i) => i) });
  assert.equal(td.status, 200, JSON.stringify(td.body));
  return td.body.action_id as string;
}

const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};
const count = (w: World, u: User, metric: string) => w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = ? ORDER BY day DESC`, u.id, metric)[0]?.count ?? 0;
const status = (w: World, bid: string, v: number) => w.q<any>(`SELECT status, full_key <> '' AS body FROM circle_versions WHERE bank_id = ? AND version = ?`, bid, v)[0];

test("48-1 (S-B1): every answer-key PATCH that does work counts toward the day's 60 attempts, refusals included; the 61st is a 429 in plain words; a no-op and a malformed or out-of-range decision are refused before any work and cost no attempt", async () => {
  const { w, A, cid } = await world();
  const bid = (await sendRaw(w, A, cid, bankOf([mcq("Which gas do plants take in from the air?", [["A. Oxygen", false], ["B. Carbon dioxide", true], ["C. Helium", false]])]))).body.bank_id;
  // Cheap refusals first: the same setting again, a decision for a question the head does not have.
  assert.equal((await patch(w, A, cid, bid, { hide_key: false })).body.unchanged, true);
  const far = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 7, choice: "keep" }] });
  assert.deepEqual([far.status, far.body.error, far.body.field], [400, "invalid_request", "labels"], JSON.stringify(far.body));
  assert.equal(count(w, A, "circle_hide_attempts"), 0);
  for (let i = 0; i < 60; i++) assert.equal((await patch(w, A, cid, bid, { hide_key: true })).body.error, "labels_need_decision", `attempt ${i}`);
  assert.equal(count(w, A, "circle_hide_attempts"), 60);
  assert.equal(count(w, A, "circle_hide_changes"), 0);
  const over = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "keep" }] });
  assert.deepEqual([over.status, over.body.error, over.body.metric], [429, "daily_limit", "circle_hide_attempts"], JSON.stringify(over.body));
  assert.match(over.body.message, /tried/i);
  w.advance(DAY);
  let next = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "keep" }] });
  if (next.body.error === "order_ack_required") next = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "keep" }], order_acks: next.body.needs_order_ack });
  assert.equal(next.status, 200, "the next day");
});

test("48-2 (S-B2): the evidence bound never prunes a bank an administrator holds (nor a version an open report names): the 7th flagged send into a held bank keeps version 1, which the administrator still opens; the same sends into a bank not held prune it", async () => {
  const { w, adm, E, P, cid } = await world();
  await takedownOf(w, adm, P, [q(TEXTBOOK)]);
  limit(w, "circle_send_flags", 1000, E.id);
  const sendSeven = async (title: string) => {
    const s = await sendRaw(w, E, cid, bankOf([q(TEXTBOOK), q(`${title} padding 0?`)]));
    assert.equal(s.body.withheld, true);
    let base = 1;
    for (let i = 1; i < 7; i++) base = (await sendRaw(w, E, cid, bankOf([q(TEXTBOOK), q(`${title} padding ${i}?`)]), { bid: s.body.bank_id, base, confirm: true })).body.version;
    assert.equal(base, 7);
    return s.body.bank_id as string;
  };
  const free = await sendSeven("Free");
  assert.equal(status(w, free, 1).status, "pruned", "a bank not held: the bound prunes its oldest");
  // Held after its 5th version, before its 6th and 7th.
  const s = await sendRaw(w, E, cid, bankOf([q(TEXTBOOK), q("Held padding 0?")]));
  const held = s.body.bank_id;
  let base = 1;
  for (let i = 1; i < 5; i++) base = (await sendRaw(w, E, cid, bankOf([q(TEXTBOOK), q(`Held padding ${i}?`)]), { bid: held, base, confirm: true })).body.version;
  assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-hold", { bank_id: held, note: "Keeping this for the review." })).status, 200);
  for (let i = 5; i < 7; i++) base = (await sendRaw(w, E, cid, bankOf([q(TEXTBOOK), q(`Held padding ${i}?`)]), { bid: held, base, confirm: true })).body.version;
  assert.deepEqual(status(w, held, 1), { status: "previous", body: 1 });
  const open = await w.admin<any>(adm, "GET", "/admin/pool/circle-bank", null, `?id=${held}&version=1&basis=copyright`);
  assert.equal(open.status, 200, open.text.slice(0, 200));
});

test("48-3 (S-B2): the bound ranks per sender: one account's flagged versions across five banks cannot displace another member's older evidence; the circle still keeps at most 20", async () => {
  const { w, adm, A, E, P, cid } = await world();
  await takedownOf(w, adm, P, [q(TEXTBOOK)]);
  // The honest member's version is flagged, then she sends a clean fix: her flagged version is now evidence.
  const a1 = await sendRaw(w, A, cid, bankOf([q(TEXTBOOK), q("Ann's own question about cirques?")]));
  assert.equal(a1.body.withheld, true);
  assert.equal((await sendRaw(w, A, cid, bankOf([q(CLEAN)]), { bid: a1.body.bank_id, base: 1, confirm: true })).status, 200);
  w.advance(1000);
  limit(w, "circle_send_flags", 1000, E.id);
  limit(w, "circle_commits", 1000, E.id);
  for (let b = 0; b < 5; b++) {
    const s = await sendRaw(w, E, cid, bankOf([q(TEXTBOOK), q(`Bank ${b} padding 0?`)]));
    let base = 1;
    for (let i = 1; i < 9; i++) base = (await sendRaw(w, E, cid, bankOf([q(TEXTBOOK), q(`Bank ${b} padding ${i}?`)]), { bid: s.body.bank_id, base, confirm: true })).body.version;
    w.advance(1000);
  }
  assert.deepEqual(status(w, a1.body.bank_id, 1), { status: "previous", body: 1 }, "Ann's evidence stays");
  const evidence = w.q<any>(`SELECT COUNT(*) AS n FROM circle_versions WHERE circle_id = ? AND status = 'previous' AND withheld_at IS NOT NULL AND full_key <> ''`, cid)[0].n;
  assert.ok(evidence <= 20, `${evidence} kept`);
  assert.ok(evidence >= 15, `${evidence} kept (the bound is not over-eager)`);
});

test("48-4 (S-B2): a removed bank whose head is flagged evidence: purge-now frees everything else and says so (200 purged_partial, pending_review; the bank cannot be restored); once the administrator marks the evidence captured, purge-now purges it and the flag keeps its record", async () => {
  const { w, adm, O, E, P, cid } = await world();
  await takedownOf(w, adm, P, [q(TEXTBOOK)]);
  const s1 = await sendRaw(w, E, cid, bankOf([q(CLEAN)]));
  const bid = s1.body.bank_id;
  assert.equal((await sendRaw(w, E, cid, bankOf([q(CLEAN), q("Second clean one?")]), { bid, base: 1, confirm: true })).status, 200);
  assert.equal((await sendRaw(w, E, cid, bankOf([q(CLEAN), q(TEXTBOOK)]), { bid, base: 2, confirm: true })).body.withheld, true);
  assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: O })).status, 200);
  const pg = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/purge`, { user: O });
  assert.deepEqual([pg.status, pg.body.purged_partial, pg.body.pending_review, pg.body.evidence_kept], [200, true, true, 1], JSON.stringify(pg.body));
  assert.match(pg.body.message, /cannot be restored/);
  assert.deepEqual(kept(w, bid), [{ version: 3, w: 1 }], "only the evidence is kept");
  const un = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/undelete`, { user: O });
  assert.deepEqual([un.status, un.body.error], [409, "bank_removed"]);
  assert.equal((await w.admin(adm, "GET", "/admin/pool/circle-bank", null, `?id=${bid}&version=3&basis=copyright`)).status, 200, "the admin still opens it");
  // The administrator has what the review needs: the evidence is captured; the flag stays open with its record.
  const cap = await w.admin<any>(adm, "POST", "/admin/pool/circle-evidence-captured", { bank_id: bid, note: "Screenshots saved to the case file." });
  assert.deepEqual([cap.status, cap.body.flags], [200, 1], cap.text);
  const full = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/purge`, { user: O });
  assert.deepEqual([full.status, full.body.ok, full.body.purged_partial], [200, true, undefined], JSON.stringify(full.body));
  assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ?`, bid).length, 0);
  const f = flags(w, bid);
  assert.equal(f.length, 1);
  assert.equal(f[0].resolved_at, null, "the flag stays for the administrator's decision");
  const d = JSON.parse(f[0].detail);
  assert.deepEqual([d.evidence_captured, d.positions.count, d.hashes.length > 0], [true, 1, true]);
});

test("48-5 (limits): the keyless message fits the questions it names: a short-answer or essay question asks for the full bank or a sample answer, never 'set the correct answers'", async () => {
  const { w, O, A, V, cid } = await world();
  const essay = q("Explain in a paragraph why the seasons change.", { kind: { Short: { sample: "" } } });
  const s = await sendRaw(w, A, cid, bankOf([essay, mcq("Which planet is closest to the sun?", [["Mercury", true], ["Venus", false]])]));
  const bid = s.body.bank_id;
  assert.equal((await patch(w, A, cid, bid, { hide_key: true })).status, 200);
  const copy = JSON.parse(await (await read(w, V, `/circles/${cid}/banks/${bid}`)).text());
  assert.equal((await w.json("POST", `/circles/${cid}/members/role`, { user: O, json: { user_id: V.id, role: "editor" } })).status, 200);
  const qs = (copy.payload.bank.questions as Q[]).map(({ keyless: _k, ...x }) => ({ ...x, explanation: SENTINEL, published: { item_id: bid, qid: x.id, publish_id: "p", version: 1 } }));
  // The Mc re-keyed, the essay left as it is: only the essay is named, and the words fit it.
  qs[1].kind.Mc.answers = qs[1].kind.Mc.answers.map((a: Q) => ({ ...a, correct: a.text === "Mercury" }));
  qs[1].explanation = null;
  const r = await sendRaw(w, V, cid, JSON.stringify({ schema_version: 2, bank: { ...copy.payload.bank, questions: qs } }), { bid, base: 1 });
  assert.deepEqual([r.status, r.body.error, r.body.positions, r.body.kinds], [422, "keyless_question", [0], ["Short"]], JSON.stringify(r.body));
  assert.doesNotMatch(r.body.message, /correct answers/i);
  assert.match(r.body.message, /sample answer/i);
  assert.match(r.body.message, /full bank/i);
  // An Mc names its own fix.
  qs[0].explanation = null;
  qs[0].kind = { Short: { sample: "Because the axis is tilted." } };
  qs[1].explanation = SENTINEL;
  qs[1].kind.Mc.answers = qs[1].kind.Mc.answers.map((a: Q) => ({ ...a, correct: false }));
  const r2 = await sendRaw(w, V, cid, JSON.stringify({ schema_version: 2, bank: { ...copy.payload.bank, questions: qs } }), { bid, base: 1 });
  assert.deepEqual([r2.body.error, r2.body.positions, r2.body.kinds], ["keyless_question", [1], ["Mc"]], JSON.stringify(r2.body));
  assert.match(r2.body.message, /correct/i);
});

test("48-6 (limits): a send or a rollback that loses only because hide_key changed meanwhile is 409 hide_key_changed (send again), not a merge conflict; the PATCH's own lost race names the head", async () => {
  const { w, O, A, cid } = await world();
  const s1 = await sendRaw(w, A, cid, bankOf([q(CLEAN)]));
  const bid = s1.body.bank_id;
  w.r2.onPutPrefix = "circles/";
  w.r2.onPut = async () => void w.sql.prepare(`UPDATE circle_banks SET hide_key = 1 WHERE id = ?`).run(bid);
  const r = await sendRaw(w, A, cid, bankOf([q(CLEAN), q("Second?")]), { bid, base: 1, confirm: true });
  assert.deepEqual([r.status, r.body.error, r.body.hide_key], [409, "hide_key_changed", true], JSON.stringify(r.body));
  assert.match(r.body.message, /send/i);
  w.sql.prepare(`UPDATE circle_banks SET hide_key = 0 WHERE id = ?`).run(bid);
  assert.equal((await sendRaw(w, A, cid, bankOf([q(CLEAN), q("Second?")]), { bid, base: 1, confirm: true })).status, 200);
  w.r2.onPut = async () => void w.sql.prepare(`UPDATE circle_banks SET hide_key = 1 WHERE id = ?`).run(bid);
  const rb = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/rollback`, { user: O, json: { commit_id: publishId(), to_version: 1, base_version: 2, confirm_large_change: true } });
  assert.deepEqual([rb.status, rb.body.error], [409, "hide_key_changed"], JSON.stringify(rb.body));
  w.sql.prepare(`UPDATE circle_banks SET hide_key = 0 WHERE id = ?`).run(bid);
  // The PATCH strips the head (an R2 read); a version committed meanwhile (here written directly: a send would wait for
  // the heavy slot the PATCH holds) makes it lose: 409 conflict with the head.
  w.r2.onGet = async () => void w.sql.prepare(`UPDATE circle_banks SET head_version = 3, head_version_id = ? WHERE id = ?`).run("f".repeat(32), bid);
  const p = await patch(w, A, cid, bid, { hide_key: true });
  assert.deepEqual([p.status, p.body.error, p.body.head_version], [409, "conflict", 3], JSON.stringify(p.body));
});

test("48-7 (limits): versions withheld past the day's flags are named on one bank-level cap flag (bank, circle, versions, takedowns), and an unblock that lifts their takedown releases them like flagged ones (the head readable, the hold lifted)", async () => {
  const { w, adm, E, P, cid } = await world();
  const action = await takedownOf(w, adm, P, [q(TEXTBOOK)]);
  const s1 = await sendRaw(w, E, cid, bankOf([q(CLEAN)]));
  const bid = s1.body.bank_id;
  let base = 1;
  for (let i = 0; i < 8; i++) {
    const r = await sendRaw(w, E, cid, bankOf([q(CLEAN), q(TEXTBOOK), q(`Padding ${i}?`)]), { bid, base, confirm: true });
    assert.deepEqual([r.status, r.body.withheld], [200, true]);
    base = r.body.version;
  }
  assert.equal(flags(w, bid).length, 5);
  const cap = flags(w, bid, "cap");
  assert.equal(cap.length, 1);
  const d = JSON.parse(cap[0].detail);
  assert.deepEqual([d.metric, d.circle_id, d.bank_id, d.versions.map((x: Q) => x.version)], ["circle_send_flags", cid, bid, [7, 8, 9]]);
  assert.ok(d.versions.every((x: Q) => x.actions.includes(action)));
  assert.equal(cap[0].user_id, E.id);
  const un = await w.admin<any>(adm, "POST", "/admin/dmca/unblock", { action_id: action, note: "Withdrawn by the complainant.", release_circle_versions: true });
  assert.deepEqual([un.status, un.body.circle_versions_released], [200, 8], un.text);
  const h = await read(w, E, `/circles/${cid}/banks/${bid}`);
  assert.equal(h.status, 200, "the head is readable again");
  assert.equal(w.q(`SELECT 1 FROM circle_holds WHERE bank_id = ?`, bid).length, 0);
  assert.equal(JSON.parse(flags(w, bid, "cap")[0].detail).block_lifted, true);
});

test("48-8 (limits): pruned evidence is said as such in the history and on a read; trim lists the evidence it keeps in pinned_versions, as many as evidence_kept", async () => {
  const { w, adm, O, E, P, cid } = await world();
  await takedownOf(w, adm, P, [q(TEXTBOOK)]);
  limit(w, "circle_send_flags", 1000, E.id);
  const s = await sendRaw(w, E, cid, bankOf([q(TEXTBOOK), q("Padding 0?")]));
  const bid = s.body.bank_id;
  let base = 1;
  for (let i = 1; i < 8; i++) base = (await sendRaw(w, E, cid, bankOf([q(TEXTBOOK), q(`Padding ${i}?`)]), { bid, base, confirm: true })).body.version;
  base = (await sendRaw(w, O, cid, bankOf([q(CLEAN)]), { bid, base, confirm: true })).body.version;
  assert.equal(status(w, bid, 1).status, "pruned");
  const hist = (await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions`, { user: O })).body.versions as Q[];
  const v1 = hist.find((x) => x.version === 1)!;
  const v7 = hist.find((x) => x.version === 7)!;
  assert.deepEqual([v1.withheld, v1.kept, v1.review_copy], [true, false, "hashes_only"]);
  assert.deepEqual([v7.withheld, v7.review_copy], [true, "full"]);
  assert.equal(hist.find((x) => x.version === base)!.review_copy, undefined);
  const r = await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/1`, { user: E });
  assert.deepEqual([r.status, r.body.error, r.body.withheld, r.body.review_copy], [404, "version_not_retained", true, "hashes_only"], JSON.stringify(r.body));
  assert.doesNotMatch(r.body.message, /last 20 versions/);
  const tr = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/trim`, { user: O, json: { keep: 1 } });
  assert.equal(tr.status, 200);
  assert.ok(tr.body.evidence_kept > 0);
  assert.deepEqual(tr.body.pinned_versions, kept(w, bid).filter((x: Q) => x.w === 1).map((x: Q) => x.version));
  assert.equal(tr.body.pinned_versions.length, tr.body.evidence_kept);
});

test("48-9 (limits): an email invite to an address that has an open invite in the circle rotates that invite's code (one live code, the same invite) and says so; the old code no longer joins", async () => {
  const w = new World();
  const O = w.user("teacher", "Olga");
  const N = w.user("teacher", "Nia");
  const cid = await circleWith(w, O, []);
  const a = await invite(w, O, cid, { email: "nia@pvhs.example", role: "viewer" });
  assert.equal(a.status, 202);
  const b = await invite(w, O, cid, { email: "Nia+x@pvhs.example", role: "editor" });
  assert.deepEqual([b.status, b.body.invite_id, b.body.resent, b.body.role, b.body.address_label], [202, a.body.invite_id, true, "editor", a.body.address_label], JSON.stringify(b.body));
  const list = (await w.json<any>("GET", `/circles/${cid}/invites`, { user: O })).body.invites as Q[];
  assert.equal(list.filter((x) => x.state === "active").length, 1);
  const [m1, m2] = mails(w);
  assert.equal(mails(w).length, 2);
  assert.notEqual(m1!.code, m2!.code);
  assert.equal((await w.json<any>("POST", "/circles/join", { user: N, json: { code: m1!.code } })).status, 422, "the first code is dead");
  const j = await w.json<any>("POST", "/circles/join", { user: N, json: { code: m2!.code } });
  assert.equal(j.status, 200, JSON.stringify(j.body));
  // Used: the next invite to that address is a new one.
  const c = await invite(w, O, cid, { email: "nia@pvhs.example", role: "viewer" });
  assert.deepEqual([c.status, c.body.resent === true, c.body.invite_id === a.body.invite_id], [202, false, false]);
});

test("48-10 (limits): a mail domain's hour in neutral words, the inviter's own share named apart; erasure clears the inviter's hourly domain rows; one role hint for the answer-key PATCH, trim and rollback; migration 0114 can run again", async () => {
  const w = new World();
  const [O, O2] = ["Olga", "Omar"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, []);
  const c2 = await circleWith(w, O2!, []);
  const r: Q[] = [];
  for (let k = 0; k < 11; k++) r.push(await invite(w, O!, cid, { email: `t${k}@pvhs.example`, role: "viewer" }));
  assert.deepEqual([r[10]!.status, r[10]!.body.error, r[10]!.body.metric], [429, "hourly_limit", "circle_email_inviter_domain_hourly"]);
  assert.match(r[10]!.body.message, /you have sent/i);
  assert.match(r[10]!.body.message, /this mail domain/i);
  assert.doesNotMatch(r[10]!.body.message, /provider/i);
  limit(w, "circle_email_domain_hourly", 11);
  assert.equal((await invite(w, O2!, c2, { email: "u0@pvhs.example", role: "viewer" })).status, 202);
  const dom = await invite(w, O2!, c2, { email: "u1@pvhs.example", role: "viewer" });
  assert.deepEqual([dom.status, dom.body.metric], [429, "circle_email_domain_hourly"]);
  assert.match(dom.body.message, /this mail domain/i);
  assert.doesNotMatch(dom.body.message, /provider/i);
  assert.ok(w.q(`SELECT 1 FROM pool_alert_budget WHERE channel LIKE ?`, `%${O!.id}`).length > 0);
  await erase(w, O!);
  assert.equal(w.q(`SELECT 1 FROM pool_alert_budget WHERE channel LIKE ?`, `%${O!.id}%`).length, 0);
  // Role hints.
  const { w: w2, A, E, V, cid: c3 } = await world();
  const bid = (await sendRaw(w2, A, c3, bankOf([q(CLEAN)]))).body.bank_id;
  assert.equal((await sendRaw(w2, A, c3, bankOf([q(CLEAN), q("Second?")]), { bid, base: 1, confirm: true })).status, 200);
  const hints = [
    (await patch(w2, V, c3, bid, { hide_key: true })).body.needed,
    (await patch(w2, E, c3, bid, { hide_key: true })).body.needed,
    (await w2.json<any>("POST", `/circles/${c3}/banks/${bid}/trim`, { user: E, json: { keep: 1 } })).body.needed,
    (await w2.json<any>("POST", `/circles/${c3}/banks/${bid}/rollback`, { user: E, json: { commit_id: publishId(), to_version: 1, base_version: 2 } })).body.needed,
  ];
  assert.deepEqual(hints, ["owner_or_author", "owner_or_author", "owner_or_author", "owner_or_author"]);
  // Migration 0114 again on a database that has it.
  const dir = new URL("../../migrations/", import.meta.url).pathname;
  const files = readdirSync(dir).filter((f) => f.endsWith(".sql")).sort();
  const db = new DatabaseSync(":memory:");
  for (const f of files) db.exec(readFileSync(dir + f, "utf8"));
  db.exec(readFileSync(dir + files.find((f) => f.startsWith("0114_"))!, "utf8"));
});
