// Round 47: circles phase C after critics 54a (product) and 54b (security), docs/critic-reports/2026-10-25-circles-phaseC-*54*.
// P-B1: the keyless guard asks share-core (its own rule: the `keyless` flag OR the sentinel explanation the app stores,
// and a stripped copy's placeholder True/False), so the app's real copy (sentinel, no flag) is refused too. P-B2: only
// a hide_key PATCH that changes something is charged to the day's changes; refusals and the decision steps are not.
// S-N1: withheld versions kept as the administrator's evidence are bounded (5 a bank, 20 a circle, newest first; older
// ones pruned, their flags marked evidence_pruned), a sender's flags in a circle are capped a day, and the owner can
// always free the rest (trim, purge-now). S-N2: an unblock releases a version only when the lifted takedown was its
// only reason. Variants: bulk invites (40 a day, 30 an hour, 30 open email invites, a fair share of a domain's hour,
// plain words when a provider is busy), distinct masked labels, refunded hidden reads on 409, turning hidden keys off
// undoes the PATCH's own withhold when the full text is clean, a rollback compares hide_key at its commit.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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

test("47-1 (P-B1): the app's keyless copy (the sentinel explanation, no keyless flag) is refused like a flagged one, by share-core's own rule: a promoted viewer's copy sent back (hide_key on and off), a new bank, and a stripped copy's placeholder True/False", async () => {
  assert.match(SENTINEL, /No answer key/);
  const { w, O, A, V, cid } = await world();
  const s = await sendRaw(w, A, cid, bankOf([mcq("Which planet is closest to the sun?", [["Mercury", true], ["Venus", false], ["Mars", false]]), q("Name the largest ocean on the planet.")]));
  const bid = s.body.bank_id;
  assert.equal((await patch(w, A, cid, bid, { hide_key: true })).status, 200);
  const copy = JSON.parse(await (await read(w, V, `/circles/${cid}/banks/${bid}`)).text());
  assert.equal((await w.json("POST", `/circles/${cid}/members/role`, { user: O, json: { user_id: V.id, role: "editor" } })).status, 200);
  // As the app holds it: no `keyless` field, the sentinel explanation (src/model.rs is_keyless).
  const appQs = (copy.payload.bank.questions as Q[]).map(({ keyless: _k, ...x }) => ({ ...x, explanation: SENTINEL, stem: `${x.stem} (edited)`, published: { item_id: bid, qid: x.id, publish_id: "p", version: 1 } }));
  const back = JSON.stringify({ schema_version: 2, bank: { ...copy.payload.bank, questions: appQs } });
  const n0 = w.q(`SELECT 1 FROM circle_versions`).length;
  const r = await sendRaw(w, V, cid, back, { bid, base: 1 });
  assert.deepEqual([r.status, r.body.error, r.body.positions], [422, "keyless_question", [0, 1]], JSON.stringify(r.body));
  assert.equal(w.q(`SELECT 1 FROM circle_versions`).length, n0);
  const head = JSON.parse(await (await read(w, O, `/circles/${cid}/banks/${bid}`)).text());
  assert.equal(head.payload.bank.questions[0].kind.Mc.answers[0].correct, true, "the keys stay");
  assert.equal((await patch(w, A, cid, bid, { hide_key: false })).status, 200);
  assert.equal((await sendRaw(w, V, cid, back, { bid, base: 1 })).body.error, "keyless_question", "hide_key off too");
  assert.equal((await sendRaw(w, V, cid, JSON.stringify({ schema_version: 2, bank: { ...copy.payload.bank, id: "new", questions: appQs.map(({ published: _p, ...x }) => x) } }))).body.error, "keyless_question", "a new bank");
  // A stripped copy whose flag and sentinel were lost: its True/False false is a placeholder (share-core's placeholder_key).
  const origin = { item_id: "a".repeat(32), qid: "b".repeat(32), publish_id: "c".repeat(32), version: 1, author_name: "Pat", license: "CC-BY-4.0", form: "stripped" };
  const tf = q("The moon is a planet of its own.", { kind: { Tf: { correct: false } }, origin });
  const pr = await sendRaw(w, A, cid, bankOf([q("A keyed question about tides?"), tf]));
  assert.deepEqual([pr.status, pr.body.error, pr.body.positions], [422, "keyless_question", [1]], JSON.stringify(pr.body));
  // A copy re-keyed by its owner (share-core's `rekeyed`) is not a placeholder.
  assert.notEqual((await sendRaw(w, A, cid, bankOf([{ ...tf, rekeyed: true }]))).body.error, "keyless_question");
  // A field the send drops (a malformed origin) does not hide the sentinel from the check.
  const hidden = await sendRaw(w, A, cid, bankOf([q("Another question about tides?", { explanation: SENTINEL, origin: { form: "stripped" } })]));
  assert.deepEqual([hidden.status, hidden.body.error, hidden.body.positions], [422, "keyless_question", [0]], JSON.stringify(hidden.body));
  // An ordinary explanation is not a keyless marker.
  assert.equal((await sendRaw(w, A, cid, bankOf([q("A normal question?", { explanation: "Because the tide follows the moon." })]))).status, 201);
});

test("47-2 (P-B2): the day's answer-key changes count only PATCHes that change something: twelve banks through the app's three steps (refused for labels, then for an order acknowledgement, then done) all succeed and count twelve; the cap refuses the change past it, never a refusal", async () => {
  const { w, A, cid } = await world();
  const banks: string[] = [];
  for (let i = 0; i < 12; i++) banks.push((await sendRaw(w, A, cid, bankOf([mcq(`Which gas do plants take in, case ${i}?`, [["A. Oxygen", false], ["B. Carbon dioxide", true], ["C. Helium", false]])]))).body.bank_id);
  const changes = () => w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'circle_hide_changes'`, A.id)[0]?.count ?? 0;
  for (const bid of banks) {
    const r1 = await patch(w, A, cid, bid, { hide_key: true });
    assert.equal(r1.body.error, "labels_need_decision");
    const r2 = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "keep" }] });
    assert.equal(r2.body.error, "order_ack_required");
    const r3 = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "keep" }], order_acks: r2.body.needs_order_ack });
    assert.equal(r3.status, 200, JSON.stringify(r3.body));
  }
  assert.equal(changes(), 12);
  limit(w, "circle_hide_changes", 12, A.id);
  const over = await patch(w, A, cid, banks[0]!, { hide_key: false });
  assert.deepEqual([over.status, over.body.metric], [429, "circle_hide_changes"]);
  // A refusal past the cap is a refusal, not a charge; a no-op stays free.
  assert.equal((await patch(w, A, cid, banks[1]!, { hide_key: true, labels: [{ question: 0, choice: "keep" }], order_acks: [{ question: 0 }] })).body.unchanged, true);
  assert.equal(changes(), 12);
});

test("47-3 (S-N1): sixty-one blocked sends by one editor: its flags in the circle are capped a day (the admin is told), the kept evidence is bounded (5 a bank beyond the 20 kept), a clean fix still fits, trim and purge-now free everything else, and the admin can still open a flagged version within the bound", async () => {
  const { w, adm, O, E, P, cid } = await world();
  await takedownOf(w, adm, P, [q(TEXTBOOK)]);
  // The editor's own bank (its author may send to it while it is held by its own match).
  const s1 = await sendRaw(w, E, cid, bankOf([q(TEXTBOOK), q("Padding first about moraines?")]));
  assert.deepEqual([s1.status, s1.body.withheld], [201, true]);
  const bid = s1.body.bank_id;
  const one = w.q<any>(`SELECT bytes FROM circle_versions WHERE bank_id = ?`, bid)[0].bytes as number;
  limit(w, "circle_bytes", one * 30);
  let base = 1;
  for (let i = 0; i < 60; i++) {
    const r = await sendRaw(w, E, cid, bankOf([q(TEXTBOOK), q(`Padding ${i} about moraines?`)]), { bid, base, confirm: true });
    assert.equal(r.status, 200, `send ${i}: ${JSON.stringify(r.body)}`);
    assert.deepEqual([r.body.held, r.body.withheld], [true, true]);
    base = r.body.version;
  }
  // At most 5 flags a day from one sender in one circle; the rest are withheld all the same, and the admin is told once.
  assert.equal(flags(w, bid).length, 5);
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'cap' AND user_id = ? AND detail LIKE '%circle_send_flags%'`, E.id).length, 1);
  const k = kept(w, bid);
  assert.ok(k.length <= 25, `bounded: ${k.length} versions kept`);
  // A clean fix fits (the owner may send to a bank held by a send's own match).
  const fix = await sendRaw(w, O, cid, bankOf([q(CLEAN), q("A second clean question about kames?")]), { bid, base, confirm: true });
  assert.equal(fix.status, 200, JSON.stringify(fix.body));
  assert.equal(fix.body.held, false);
  // The admin opens a flagged version within the bound.
  const pinned = flags(w, bid).map((f) => JSON.parse(f.detail).version as number);
  const open = await w.admin<any>(adm, "GET", "/admin/pool/circle-bank", null, `?id=${bid}&version=${pinned[pinned.length - 1]}&basis=copyright`);
  assert.equal(open.status, 200, open.text.slice(0, 200));
  // The owner's trim frees everything but the head and the bounded evidence, and says how many it keeps for the admin.
  const tr = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/trim`, { user: O, json: { keep: 1 } });
  assert.equal(tr.status, 200);
  const after = kept(w, bid);
  assert.ok(after.length <= 1 + 5, JSON.stringify(after));
  assert.equal(tr.body.evidence_kept, after.filter((x: Q) => x.w === 1).length);
  assert.ok(tr.body.evidence_kept > 0);
  // Removed, then purge-now: still pending the admin's review, but everything that is not evidence is freed.
  assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: O })).status, 200);
  const pg = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/purge`, { user: O });
  // (round 48: answered as the work it does, 200 purged_partial, pending_review)
  assert.deepEqual([pg.status, pg.body.purged_partial, pg.body.pending_review], [200, true, true]);
  assert.ok(kept(w, bid).length > 0 && kept(w, bid).every((x: Q) => x.w === 1), "only withheld evidence stays");
});

test("47-4 (S-N1): the evidence bound across days and banks: at most 5 flagged versions a bank and 20 a circle are kept beyond the window, newest first; the flags of pruned evidence are marked evidence_pruned (their positions stay) and the admin's open of one says so", async () => {
  const { w, adm, O, A, E, P, cid } = await world();
  await takedownOf(w, adm, P, [q(TEXTBOOK)]);
  limit(w, "circle_send_flags", 1000, E.id);
  limit(w, "circle_commits", 1000, A.id);
  const banks: string[] = [];
  for (let b = 0; b < 5; b++) {
    const s = await sendRaw(w, A, cid, bankOf([q(`${CLEAN} (bank ${b})`)]));
    assert.equal(s.status, 201, JSON.stringify(s.body));
    const bid = s.body.bank_id;
    banks.push(bid);
    let base = 1;
    for (let i = 0; i < 6; i++) {
      base = (await sendRaw(w, E, cid, bankOf([q(TEXTBOOK), q(`Bank ${b} padding ${i}?`)]), { bid, base, confirm: true })).body.version;
      base = (await sendRaw(w, A, cid, bankOf([q(`${CLEAN} (bank ${b})`)]), { bid, base, confirm: true })).body.version;
    }
    for (let i = 0; i < 21; i++) base = (await sendRaw(w, A, cid, bankOf([q(`${CLEAN} (bank ${b})`), q(`Later ${i}?`)]), { bid, base, confirm: true })).body.version;
    w.advance(1000);
  }
  let total = 0;
  for (const bid of banks) {
    const ev = kept(w, bid).filter((x: Q) => x.w === 1);
    assert.ok(ev.length <= 5, `bank bound: ${ev.length}`);
    total += ev.length;
    const fl = flags(w, bid);
    assert.equal(fl.length, 6);
    assert.equal(fl.filter((f) => JSON.parse(f.detail).evidence_pruned === true).length, 6 - ev.length);
    assert.ok(fl.every((f) => JSON.parse(f.detail).positions.count === 1), "the positions stay");
  }
  assert.ok(total <= 20, `circle bound: ${total}`);
  // The admin's open of pruned evidence says it was pruned.
  const pruned = flags(w, banks[0]!).find((f) => JSON.parse(f.detail).evidence_pruned === true)!;
  const v = JSON.parse(pruned.detail).version;
  const op = await w.admin<any>(adm, "GET", "/admin/pool/circle-bank", null, `?id=${banks[0]}&version=${v}&basis=copyright`);
  assert.deepEqual([op.status, op.body.error, op.body.evidence_pruned], [404, "version_not_retained", true]);
  void O;
});

test("47-5 (S-N2): an unblock releases a circle version only when the lifted takedown was its only reason: not one whose text is now confirmed student data, not one in a bank an administrator or reports hold", async () => {
  {
    const { w, adm, A, E, P, cid } = await world();
    const R = w.user("teacher", "Rob");
    const it1 = (await publish(w, P, bankOf([q(TEXTBOOK)]))).body as Q;
    const it2 = (await publish(w, R, bankOf([q(TEXTBOOK)]))).body as Q;
    const td = await w.takedown<any>(adm, await notice(w, adm), it1.item_id, { questions: [0] });
    const s1 = await sendRaw(w, A, cid, bankOf([q(CLEAN)]));
    const bid = s1.body.bank_id;
    assert.equal((await sendRaw(w, E, cid, bankOf([q(CLEAN), q(TEXTBOOK)]), { bid, base: 1, confirm: true })).body.withheld, true);
    // Later the same text is confirmed as student data (an administrator's purge of another public item holding it).
    assert.equal((await w.purge(adm, it2.item_id, { questions: [0] })).status, 200);
    const un = await w.admin<any>(adm, "POST", "/admin/dmca/unblock", { action_id: td.body.action_id, note: "Withdrawn by the complainant.", release_circle_versions: true });
    assert.deepEqual([un.status, un.body.circle_flags_lifted, un.body.circle_versions_released], [200, 1, 0], un.text);
    assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/2`, { user: A })).body.error, "version_withheld");
  }
  {
    const { w, adm, A, E, P, cid } = await world();
    const action = await takedownOf(w, adm, P, [q(TEXTBOOK)]);
    const s1 = await sendRaw(w, A, cid, bankOf([q(CLEAN)]));
    const bid = s1.body.bank_id;
    assert.equal((await sendRaw(w, E, cid, bankOf([q(CLEAN), q(TEXTBOOK)]), { bid, base: 1, confirm: true })).body.withheld, true);
    assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-hold", { bank_id: bid, note: "Holding while I look at it." })).status, 200);
    const un = await w.admin<any>(adm, "POST", "/admin/dmca/unblock", { action_id: action, note: "Withdrawn by the complainant.", release_circle_versions: true });
    assert.deepEqual([un.status, un.body.circle_versions_released], [200, 0], un.text);
  }
  {
    // The case it is for still works: copyright alone, released.
    const { w, adm, A, E, P, cid } = await world();
    const action = await takedownOf(w, adm, P, [q(TEXTBOOK)]);
    const s1 = await sendRaw(w, A, cid, bankOf([q(CLEAN)]));
    const bid = s1.body.bank_id;
    assert.equal((await sendRaw(w, E, cid, bankOf([q(CLEAN), q(TEXTBOOK)]), { bid, base: 1, confirm: true })).body.withheld, true);
    const un = await w.admin<any>(adm, "POST", "/admin/dmca/unblock", { action_id: action, note: "Withdrawn by the complainant.", release_circle_versions: true });
    assert.deepEqual([un.status, un.body.circle_versions_released], [200, 1], un.text);
  }
});

test("47-6 (variants): bulk invites: 30 an hour and 40 a day per inviter, 40 open email invitations a circle (its 10 codes apart); one inviter has at most 10 of a domain's 30 an hour, the domain's last fifth is kept for light inviters, and a busy mail provider is said in plain words, not as an outage", async () => {
  // One owner, colleagues at one school: 10 this hour (that domain's share of one inviter), the rest the next hour.
  const w = new World();
  const [O] = ["Olga"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, []);
  const r: Q[] = [];
  for (let k = 0; k < 11; k++) r.push(await invite(w, O!, cid, { email: `teacher${k}@pvhs.example`, role: "viewer" }));
  assert.deepEqual(r.slice(0, 10).map((x) => x.status), Array(10).fill(202));
  // (round 48, critic 55a V6: the inviter's own share of the domain is its own metric, in neutral words)
  assert.deepEqual([r[10]!.status, r[10]!.body.error, r[10]!.body.metric], [429, "hourly_limit", "circle_email_inviter_domain_hourly"]);
  assert.match(r[10]!.body.message, /mail domain/i);
  w.advance(HOUR);
  for (let k = 10; k < 15; k++) assert.equal((await invite(w, O!, cid, { email: `teacher${k}@pvhs.example`, role: "viewer" })).status, 202);
  assert.equal(mails(w).length, 15);
  // Three heavy inviters on one big domain cannot shut it for a light one.
  const w2 = new World();
  const heavies = ["H1", "H2", "H3"].map((n) => w2.user("teacher", n));
  const light = w2.user("teacher", "Light");
  const hc: string[] = [];
  for (const h of heavies) hc.push(await circleWith(w2, h, []));
  const lc = await circleWith(w2, light, []);
  const out: Q[] = [];
  for (const [i, h] of heavies.entries()) for (let k = 0; k < 12; k++) out.push(await invite(w2, h, hc[i]!, { email: `p${i}x${k}@gmail.com`, role: "viewer" }));
  const refused = out.filter((x) => x.status !== 202);
  assert.equal(out.length - refused.length, 24, "the heavy inviters together reach the domain's four fifths");
  for (const x of refused) {
    assert.deepEqual([x.status, x.body.error], [429, "hourly_limit"], JSON.stringify(x.body));
    assert.ok(["circle_email_domain_hourly", "circle_email_inviter_domain_hourly"].includes(x.body.metric));
    assert.match(x.body.message, /mail domain/i);
  }
  const ok = await invite(w2, light, lc, { email: "friend@gmail.com", role: "viewer" });
  assert.deepEqual([ok.status, ok.body.emailed], [202, true], JSON.stringify(ok.body));
  // 30 an hour and 40 a day per inviter (every address at its own domain here).
  const w3 = new World();
  const [B] = ["Bea"].map((n) => w3.user("teacher", n));
  const c3 = await circleWith(w3, B!, []);
  for (let k = 0; k < 30; k++) assert.equal((await invite(w3, B!, c3, { email: `t${k}@d${k}.example`, role: "viewer" })).status, 202, `invite ${k}`);
  const hour = await invite(w3, B!, c3, { email: "t30@d30.example", role: "viewer" });
  assert.deepEqual([hour.status, hour.body.metric], [429, "circle_email_invites_hourly"]);
  w3.advance(HOUR);
  const c4 = await circleWith(w3, B!, [], "Second circle");
  for (let k = 0; k < 10; k++) assert.equal((await invite(w3, B!, c4, { email: `u${k}@e${k}.example`, role: "viewer" })).status, 202);
  const day = await invite(w3, B!, c4, { email: "u10@e10.example", role: "viewer" });
  assert.deepEqual([day.status, day.body.metric], [429, "circle_email_invites"]);
  // The circle's open email invitations (40; 30 here), apart from its 10 plain codes.
  w3.advance(DAY);
  limit(w3, "circle_open_email_invites", 30);
  const full = await invite(w3, B!, c3, { email: "v1@f1.example", role: "viewer" });
  assert.deepEqual([full.status, full.body.error, full.body.metric], [422, "circles_limit", "circle_open_email_invites"], JSON.stringify(full.body));
  assert.equal((await w3.json<any>("POST", `/circles/${c3}/invites`, { user: B!, json: { role: "viewer" } })).status, 201, "codes have their own 10");
});

test("47-7 (variants): masked labels tell colleagues apart (a number after a repeated label) and hide a one-character local part", async () => {
  const w = new World();
  const [O] = ["Olga"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, []);
  const labels: string[] = [];
  for (const e of ["jsmith@pvhs.example", "jjones@pvhs.example", "jlee@pvhs.example", "j@pvhs.example", "jo@pvhs.example"]) labels.push((await invite(w, O!, cid, { email: e, role: "viewer" })).body.address_label);
  assert.deepEqual(labels, ["j***@pvhs.example", "j***@pvhs.example #2", "j***@pvhs.example #3", "***@pvhs.example", "j***@pvhs.example #4"]);
  const list = (await w.json<any>("GET", `/circles/${cid}/invites`, { user: O! })).body.invites.map((x: Q) => x.address_label).sort();
  assert.deepEqual(list, [...labels].sort());
});

test("47-8 (variants): a viewer's read refused with 409 does not spend the day's hidden reads", async () => {
  const { w, A, V, cid } = await world();
  const s = await sendRaw(w, A, cid, bankOf([q(CLEAN)]), { extra: { hide_key: true } });
  const bid = s.body.bank_id;
  // Decisions the gate no longer accepts (a label decision for a question that has no labels).
  w.sql.prepare(`UPDATE circle_versions SET strip_opts = ? WHERE bank_id = ?`).run(JSON.stringify({ labels: [{ question: 0, choice: "strip" }], order_acks: [], ack_formatting: [], image_ok: [], image_shas: [] }), bid);
  const count = () => w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'circle_hidden_reads'`, V.id)[0]?.count ?? 0;
  for (let i = 0; i < 3; i++) assert.equal((await read(w, V, `/circles/${cid}/banks/${bid}`)).status, 409);
  assert.equal(count(), 0);
});

test("47-9 (variants): turning hidden keys off undoes the withhold the turning-on made (the full text is clean): the head is readable again, its flag resolved, the bank's hold lifted; not when the full text matches a block too", async () => {
  {
    const { w, adm, O, A, V, P, cid } = await world();
    const s = await sendRaw(w, A, cid, bankOf([LAB()]));
    const bid = s.body.bank_id;
    await takedownOf(w, adm, P, [UNL()]);
    const on = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "strip" }] });
    assert.deepEqual([on.status, on.body.withheld], [200, true], JSON.stringify(on.body));
    const off = await patch(w, A, cid, bid, { hide_key: false });
    assert.deepEqual([off.status, off.body.hide_key, off.body.restored], [200, false, [1]], JSON.stringify(off.body));
    const h = JSON.parse(await (await read(w, O, `/circles/${cid}/banks/${bid}`)).text());
    assert.equal(h.bank.version, 1, "readable again, its keys with it");
    assert.equal((await read(w, V, `/circles/${cid}/banks/${bid}`)).status, 200, "the hold is lifted");
    assert.ok(flags(w, bid).length > 0 && flags(w, bid).every((f) => f.resolved_at !== null));
    assert.equal(w.q(`SELECT 1 FROM circle_holds WHERE bank_id = ?`, bid).length, 0);
    // Turned on again with the same decisions: its viewers' form is checked afresh and withheld again.
    const again = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "strip" }] });
    assert.deepEqual([again.status, again.body.withheld], [200, true], JSON.stringify(again.body));
  }
  {
    const { w, adm, A, P, cid } = await world();
    const R = w.user("teacher", "Rob");
    const labelled = (await publish(w, R, bankOf([LAB()]))).body as Q;
    const s = await sendRaw(w, A, cid, bankOf([LAB()]));
    const bid = s.body.bank_id;
    await takedownOf(w, adm, P, [UNL()]);
    assert.equal((await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "strip" }] })).body.withheld, true);
    // Meanwhile the labelled full text itself is taken down too: turning hiding off restores nothing.
    assert.equal((await w.takedown<any>(adm, await notice(w, adm), labelled.item_id, { questions: [0] })).status, 200);
    const off = await patch(w, A, cid, bid, { hide_key: false });
    assert.deepEqual([off.status, off.body.restored], [200, undefined], JSON.stringify(off.body));
    assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: A })).body.error, "version_withheld");
  }
});

test("47-10 (variants): a rollback compares the bank's hide_key at its commit: turned on meanwhile, it is a 409 conflict and stores nothing", async () => {
  const { w, O, A, cid } = await world();
  const s1 = await sendRaw(w, A, cid, bankOf([q(CLEAN)]));
  const bid = s1.body.bank_id;
  assert.equal((await sendRaw(w, A, cid, bankOf([q(CLEAN), q("Second?")]), { bid, base: 1, confirm: true })).status, 200);
  w.r2.onPutPrefix = "circles/";
  w.r2.onPut = async () => void w.sql.prepare(`UPDATE circle_banks SET hide_key = 1 WHERE id = ?`).run(bid);
  const n0 = w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ?`, bid).length;
  const rb = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/rollback`, { user: O, json: { commit_id: publishId(), to_version: 1, base_version: 2, confirm_large_change: true } });
  // (round 48, critic 55a V2: 409 hide_key_changed, a resend, not a merge)
  assert.deepEqual([rb.status, rb.body.error], [409, "hide_key_changed"], JSON.stringify(rb.body));
  assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ?`, bid).length, n0);
});
