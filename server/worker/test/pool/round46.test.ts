// Round 46: circles phase C after critics 53a (product) and 53b (security), docs/critic-reports/2026-10-25-circles-phaseC-*.
// P-B1: a keyless question (an answer-key-free copy, such as a viewer's copy of a bank that hides its keys) is refused on
// every circle send, whatever the bank's setting: the stored full text is the only copy of the keys. S-B1: the per-address
// mail cap counts one mailbox once (case, +tags, Gmail dots and googlemail, NFKC, IDN, trailing dots), and the hourly mail
// budget cannot be spent by one inviter or on one domain (a per-inviter hourly cap, a per-domain hourly cap, a share kept
// for inviters who sent little). S-B2: what VIEWERS of a bank that hides its keys are served (the stripped form) is checked
// against confirmed student data and copyright re-upload blocks too, on sends, rollbacks and the PATCH that hides the keys,
// and withheld exactly like a send's match. Variants: the mail names no circle and no person and says what joining needs;
// the inviter is told whether a mail went (no account-exists oracle); the owner's list shows a masked address; a full
// circle is refused up front; the PATCH has a daily cap and a no-op; ETags follow re-recorded decisions; viewers read only
// versions whose served form was checked; withheld versions with an open flag are not pruned; unblocking a takedown marks
// (and can release) the circle versions it held; the activity cursor belongs to its circle.
import assert from "node:assert/strict";
import { test } from "node:test";
import { poolNotifier } from "../../src/poolmail.ts";
import { deleteContentStatements } from "../../src/pool/index.ts";
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
const STUDENT = "Jenny Smith in period 3 scored 42 on the unit test: which of these is the class median?";
const CLEAN = "Clean question about eskers and drumlins and the glaciers that left them?";
type Mail = Extract<PoolNotice, { kind: "circle_email_invite" }>;
const mails = (w: World) => w.notices.filter((n) => n.kind === "circle_email_invite") as Mail[];
const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};
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
const read = (w: World, u: User, path: string, headers: Record<string, string> = {}) => w.call("GET", path, { user: u, headers });
const flags = (w: World, bid: string, kind: string) => w.q<any>(`SELECT * FROM admin_flags WHERE kind = ? AND item_id = ?`, kind, bid);

async function world() {
  const w = new World();
  const adm = w.user("admin");
  const [O, A, E, V, P] = ["Olga", "Ann", "Eli", "Vic", "Pat"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"], [V!, "viewer"]]);
  return { w, adm, O: O!, A: A!, E: E!, V: V!, P: P!, cid };
}

test("46-1 (P-B1): a keyless question is refused on every circle send (a viewer promoted to editor sending her copy back, hide_key on or off, a new bank, a demoted and re-promoted editor): 422 keyless_question with the positions and plain words; nothing stored, the keys stay", async () => {
  const { w, O, A, V, cid } = await world();
  const s = await sendRaw(w, A, cid, bankOf([mcq("Which planet is closest to the sun?", [["Mercury", true], ["Venus", false], ["Mars", false]]), q("Name the largest ocean on the planet.")]));
  const bid = s.body.bank_id;
  assert.equal((await patch(w, A, cid, bid, { hide_key: true })).status, 200);
  const copy = JSON.parse(await (await read(w, V, `/circles/${cid}/banks/${bid}`)).text());
  assert.ok((copy.payload.bank.questions as Q[]).every((x) => x.keyless === true));
  assert.equal((await w.json("POST", `/circles/${cid}/members/role`, { user: O, json: { user_id: V.id, role: "editor" } })).status, 200);
  const back = JSON.stringify({ schema_version: 2, bank: { ...copy.payload.bank, questions: (copy.payload.bank.questions as Q[]).map((x) => ({ ...x, stem: `${x.stem} (edited)`, published: { item_id: bid, qid: x.id, publish_id: "p", version: 1 } })) } });
  const n0 = w.q(`SELECT 1 FROM circle_versions`).length;
  const r = await sendRaw(w, V, cid, back, { bid, base: 1 });
  assert.deepEqual([r.status, r.body.error, r.body.positions], [422, "keyless_question", [0, 1]], JSON.stringify(r.body));
  assert.match(r.body.message, /answer key/i);
  assert.equal(w.q(`SELECT 1 FROM circle_versions`).length, n0, "nothing stored");
  const head = JSON.parse(await (await read(w, A, `/circles/${cid}/banks/${bid}`)).text());
  assert.equal(head.bank.version, 1);
  assert.equal(head.payload.bank.questions[0].kind.Mc.answers[0].correct, true, "the keys stay");
  // With the setting off, and as a new bank: the same refusal.
  assert.equal((await patch(w, A, cid, bid, { hide_key: false })).status, 200);
  assert.equal((await sendRaw(w, V, cid, back, { bid, base: 1 })).body.error, "keyless_question");
  assert.equal((await sendRaw(w, V, cid, JSON.stringify({ schema_version: 2, bank: { ...copy.payload.bank, id: "new" } }))).body.error, "keyless_question");
  // Demoted, then promoted again.
  for (const role of ["viewer", "editor"]) assert.equal((await w.json("POST", `/circles/${cid}/members/role`, { user: O, json: { user_id: V.id, role } })).status, 200);
  assert.equal((await sendRaw(w, V, cid, back, { bid, base: 1 })).body.error, "keyless_question");
  // One keyless question among keyed ones is named by its position.
  const mixed = bankOf([q("A keyed question about tides?"), { ...copy.payload.bank.questions[1], id: "k" }]);
  const m = await sendRaw(w, A, cid, mixed, { bid, base: 1, confirm: true });
  assert.deepEqual([m.status, m.body.positions], [422, [1]]);
});

test("46-2 (S-B1): one mailbox is one address for the per-address cap (case, a +tag, Gmail dots, googlemail, a trailing dot, full-width letters, an IDN domain), and the inviter is told plainly when no mail went (emailed:false, reason address_limit or not_deliverable; nothing made); the mail goes to the address in its ASCII form", async () => {
  const w = new World();
  const [O] = ["Olga"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, []);
  limit(w, "circle_email_invites_hourly", 100, O!.id);
  const forms = ["victim@gmail.com", "Vic.Tim@GMAIL.com", "v.i.c.t.i.m@googlemail.com", "victim+x@gmail.com", "victim@gmail.com.", " victim@gmail.com ", "ｖｉｃｔｉｍ@gmail.com", "VICTIM@googlemail.com."];
  const out: Q[] = [];
  for (const e of forms) out.push(await invite(w, O!, cid, { email: e, role: "viewer" }));
  assert.deepEqual(out.map((r) => [r.status, r.body.emailed]), [[202, true], [202, true], [202, true], ...Array(5).fill([422, false])], JSON.stringify(out.map((r) => r.body)));
  for (const r of out.slice(3)) assert.deepEqual([r.body.error, r.body.reason], ["invite_not_sent", "address_limit"]);
  assert.match(out[3]!.body.message, /3|today|tomorrow/i);
  assert.equal(mails(w).length, 3);
  // (round 48, critic 55a V6: the three mails to one mailbox in one circle are one invite whose code was rotated twice;
  // nothing is made or rotated for a mail that does not go)
  assert.equal(w.q(`SELECT 1 FROM circle_invites WHERE kind = 'email'`).length, 1, "nothing made for a mail that does not go");
  assert.equal(w.q(`SELECT 1 FROM circle_email_sends`).length, 1, "one mailbox, one row");
  assert.equal(mails(w)[1]!.to, "vic.tim@gmail.com", "sent to the address as typed (lower-cased)");
  // An IDN domain is one address in either form; the mail goes to its punycode form.
  const idn = await invite(w, O!, cid, { email: "kim@bücher.example", role: "viewer" });
  assert.deepEqual([idn.status, idn.body.emailed], [202, true], JSON.stringify(idn.body));
  assert.equal(mails(w).at(-1)!.to, "kim@xn--bcher-kva.example");
  assert.equal((await invite(w, O!, cid, { email: "kim@xn--bcher-kva.example", role: "viewer" })).status, 202);
  assert.equal(w.q<any>(`SELECT MAX(n) AS n FROM circle_email_sends`)[0].n >= 2, true);
  // An address our mailer will not send to is said so, and nothing is made.
  const nd = await invite(w, O!, cid, { email: "kim@host.123", role: "viewer" });
  assert.deepEqual([nd.status, nd.body.error, nd.body.reason, nd.body.emailed], [422, "invite_not_sent", "not_deliverable", false]);
  // The day's counts are given back for a mail that did not go.
  const used = w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'circle_email_invites'`, O!.id)[0].count;
  assert.equal(used, 5);
});

test("46-3 (S-B1): five accounts sending 20 invites each to dot-variants of one Gmail mailbox mail it at most 3 times a day, and a legitimate teacher's invite in the same hour still goes; one inviter cannot spend more than its hourly share, one domain cannot take the hour, and a share of the hour is kept for inviters who sent little", async () => {
  const w = new World();
  const attackers = ["M1", "M2", "M3", "M4", "M5"].map((n) => w.user("teacher", n));
  const cids: string[] = [];
  for (const a of attackers) cids.push(await circleWith(w, a, [], `Circle of ${a.name}`));
  const dotted = (k: number) => {
    const name = "victimname";
    let s = "";
    for (let i = 0; i < name.length; i++) s += name[i] + ((k >> i) & 1 ? "." : "");
    return `${s.replace(/\.$/, "")}@gmail.com`;
  };
  const statuses: number[] = [];
  for (const [i, a] of attackers.entries()) for (let k = 0; k < 20; k++) statuses.push((await invite(w, a, cids[i]!, { email: dotted(i * 20 + k), role: "viewer" })).status);
  assert.ok(mails(w).length <= 3, `${mails(w).length} mails to one mailbox`);
  assert.ok(statuses.every((s) => s === 202 || s === 422 || s === 429), JSON.stringify(statuses));
  const T = w.user("teacher", "Teresa");
  const tc = await circleWith(w, T, []);
  const legit = await invite(w, T, tc, { email: "colleague@school.example", role: "editor" });
  assert.deepEqual([legit.status, legit.body.emailed], [202, true], JSON.stringify(legit.body));
  // One inviter: at most circle_email_invites_hourly (10) mails an hour. (Round 47: 30 by default; this account's own
  // limit is set to 10 here, the mechanism is the same.)
  const w2 = new World();
  const [I] = ["Ida"].map((n) => w2.user("teacher", n));
  const c2 = await circleWith(w2, I!, []);
  limit(w2, "circle_open_invites", 100);
  limit(w2, "circle_email_invites_hourly", 10, I!.id);
  const r2: Q[] = [];
  for (let k = 0; k < 11; k++) r2.push(await invite(w2, I!, c2, { email: `person${k}@school${k}.example`, role: "viewer" }));
  assert.deepEqual(r2.slice(0, 10).map((r) => r.status), Array(10).fill(202));
  assert.deepEqual([r2[10]!.status, r2[10]!.body.error, r2[10]!.body.metric], [429, "hourly_limit", "circle_email_invites_hourly"]);
  w2.advance(HOUR);
  assert.equal((await invite(w2, I!, c2, { email: "late@school.example", role: "viewer" })).status, 202, "the next hour");
  // One domain: at most circle_email_domain_hourly mails an hour to it, from everyone; other domains still go.
  const w3 = new World();
  const owners = ["D1", "D2", "D3"].map((n) => w3.user("teacher", n));
  const c3: string[] = [];
  for (const o of owners) c3.push(await circleWith(w3, o, []));
  limit(w3, "circle_email_domain_hourly", 2);
  assert.equal((await invite(w3, owners[0]!, c3[0]!, { email: "a1@gmail.com", role: "viewer" })).status, 202);
  assert.equal((await invite(w3, owners[1]!, c3[1]!, { email: "a2@googlemail.com", role: "viewer" })).status, 202);
  const dom = await invite(w3, owners[2]!, c3[2]!, { email: "a3@gmail.com", role: "viewer" });
  // (round 47, critics 54a variant 3 / 54b V1: a busy mail provider is a limit of the hour in plain words, not 503 busy)
  assert.deepEqual([dom.status, dom.body.error, dom.body.metric], [429, "hourly_limit", "circle_email_domain_hourly"]);
  assert.equal((await invite(w3, owners[2]!, c3[2]!, { email: "a3@school.example", role: "viewer" })).status, 202);
  // The kept share: past 80% of the hour's budget only inviters who sent fewer than 3 this hour are mailed.
  const w4 = new World();
  const heavy = w4.user("teacher", "Heavy");
  const light = w4.user("teacher", "Light");
  const ch = await circleWith(w4, heavy, []);
  const cl = await circleWith(w4, light, []);
  limit(w4, "circle_email_hourly", 5);
  const h4: number[] = [];
  for (let k = 0; k < 5; k++) h4.push((await invite(w4, heavy, ch, { email: `h${k}@d${k}.example`, role: "viewer" })).status);
  assert.deepEqual(h4, [202, 202, 202, 202, 503], "the last fifth is kept for others");
  assert.equal((await invite(w4, light, cl, { email: "friend@other.example", role: "viewer" })).status, 202);
  assert.equal((await invite(w4, light, cl, { email: "friend2@other2.example", role: "viewer" })).status, 503, "the hour is spent");
});

test("46-4 (S-B2): a send to a bank that hides its keys is checked in the form viewers are served too: text a takedown blocked in its served form (labels stripped) is withheld like any match", async () => {
  const { w, adm, A, V, P, cid } = await world();
  const item = (await publish(w, P, bankOf([UNL()]))).body as Q;
  assert.equal((await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: [0] })).status, 200);
  const plain = await sendRaw(w, A, cid, bankOf([LAB()]));
  assert.deepEqual([plain.body.held, plain.body.withheld], [false, false], "the labelled full text is not the blocked text (as on the public path)");
  const hid = await sendRaw(w, A, cid, bankOf([LAB()]), { extra: { hide_key: true, labels: [{ question: 0, choice: "strip" }] } });
  assert.equal(hid.status, 201, JSON.stringify(hid.body));
  assert.deepEqual([hid.body.held, hid.body.withheld], [true, true], "what viewers would be served is the blocked text");
  assert.equal(flags(w, hid.body.bank_id, "blocklist").length, 1);
  assert.equal((await read(w, V, `/circles/${cid}/banks/${hid.body.bank_id}`)).status, 404);
  // (Student data needs no such case: its check also compares the stem, and near-rewordings of it, which stripping
  // labels does not change; the viewers' form is checked against confirmed student data all the same.)
});

test("46-5 (S-B2): takedown, then PATCH hide_key: the head the viewers would be served matches the block, so the head is withheld and the bank held exactly as a send's match (the answer says held and withheld, a blocklist flag, the admin mail); older versions without recorded decisions are checked too; viewers never read the blocked text", async () => {
  const { w, adm, O, A, V, P, cid } = await world();
  const s1 = await sendRaw(w, A, cid, bankOf([q(CLEAN), LAB()]));
  const bid = s1.body.bank_id;
  const s2 = await sendRaw(w, A, cid, bankOf([q(CLEAN, { published: { item_id: bid, qid: s1.body.qids[0], publish_id: "p", version: 1 } }), LAB({ published: { item_id: bid, qid: s1.body.qids[1], publish_id: "p", version: 1 } }), q("A third clean question about cirques?")]), { bid, base: 1 });
  assert.equal(s2.status, 200);
  const item = (await publish(w, P, bankOf([UNL()]))).body as Q;
  assert.equal((await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: [0] })).status, 200);
  const r = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 1, choice: "strip" }] });
  assert.equal(r.status, 200, JSON.stringify(r.body));
  assert.deepEqual([r.body.hide_key, r.body.held, r.body.withheld], [true, true, true]);
  for (const u of [A, O]) assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: u })).body.error, "version_withheld");
  assert.equal((await read(w, V, `/circles/${cid}/banks/${bid}`)).status, 404);
  const f = flags(w, bid, "blocklist");
  assert.ok(f.length >= 1);
  assert.ok(w.notices.some((n) => n.kind === "circle_copyright_match"));
  // The author repairs it: a version without that question (decisions for its viewers' form) is clean again.
  const s3 = await sendRaw(w, A, cid, bankOf([q(CLEAN, { published: { item_id: bid, qid: s1.body.qids[0], publish_id: "p", version: 1 } })]), { bid, base: 2, confirm: true });
  assert.deepEqual([s3.status, s3.body.held, s3.body.withheld], [200, false, false], JSON.stringify(s3.body));
  assert.equal((await read(w, V, `/circles/${cid}/banks/${bid}`)).status, 200);
  // Version 1 (labelled, no decisions recorded: its viewers' form cannot be made without them) stays refused to viewers;
  // version 2 stays withheld; nothing a viewer reads is the blocked text.
  const v1 = await read(w, V, `/circles/${cid}/banks/${bid}/versions/1`);
  const t1 = await v1.text();
  assert.equal(v1.status, 409, t1);
  assert.equal(((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/2`, { user: V })).body).error, "version_withheld");
  assert.ok(!/Mitochondrion/.test(t1));
  // An older version with no decisions recorded whose viewers' form IS the blocked text: the PATCH withholds it and
  // flags it (no bank hold: it is not the head); the head records its decisions and stays readable.
  const w2 = new World();
  const adm2 = w2.user("admin");
  const [O2, A2, V2, P2] = ["Olga", "Ann", "Vic", "Pat"].map((n) => w2.user("teacher", n));
  const c2 = await circleWith(w2, O2!, [[A2!, "editor"], [V2!, "viewer"]]);
  const b1 = await sendRaw(w2, A2!, c2, bankOf([UNL()]));
  const b2 = b1.body.bank_id;
  assert.equal((await sendRaw(w2, A2!, c2, bankOf([q(CLEAN)]), { bid: b2, base: 1, confirm: true })).status, 200);
  const it2 = (await publish(w2, P2!, bankOf([UNL()]))).body as Q;
  assert.equal((await w2.takedown<any>(adm2, await notice(w2, adm2), it2.item_id, { questions: [0] })).status, 200);
  const r2 = await patch(w2, A2!, c2, b2, { hide_key: true });
  assert.deepEqual([r2.status, r2.body.held, r2.body.withheld], [200, false, false], JSON.stringify(r2.body));
  assert.equal(((await w2.json<any>("GET", `/circles/${c2}/banks/${b2}/versions/1`, { user: A2! })).body).error, "version_withheld");
  assert.equal(flags(w2, b2, "blocklist").length, 1);
  assert.equal((await read(w2, V2!, `/circles/${c2}/banks/${b2}`)).status, 200);
});

test("46-6 (S-B2): a rollback of a bank that hides its keys checks the target's viewers' form with its recorded decisions; a viewer reads only versions whose served form was checked (none recorded: 409, a HEAD and a 304 too)", async () => {
  const { w, adm, O, A, V, P, cid } = await world();
  const s1 = await sendRaw(w, A, cid, bankOf([LAB()]), { extra: { hide_key: true, labels: [{ question: 0, choice: "strip" }] } });
  const bid = s1.body.bank_id;
  assert.equal(s1.body.withheld, false);
  const s2 = await sendRaw(w, A, cid, bankOf([q(CLEAN)]), { bid, base: 1, confirm: true });
  assert.equal(s2.status, 200, JSON.stringify(s2.body));
  const item = (await publish(w, P, bankOf([UNL()]))).body as Q;
  assert.equal((await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: [0] })).status, 200);
  const rb = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/rollback`, { user: O, json: { commit_id: publishId(), to_version: 1, base_version: 2, confirm_large_change: true } });
  assert.deepEqual([rb.status, rb.body.held, rb.body.withheld], [200, true, true], JSON.stringify(rb.body));
  // A version with no decisions recorded (the setting turned on after it; beyond what the PATCH checks) is not served to viewers.
  const w2 = new World();
  const [O2, A2, V2] = ["Olga", "Ann", "Vic"].map((n) => w2.user("teacher", n));
  const c2 = await circleWith(w2, O2!, [[A2!, "editor"], [V2!, "viewer"]]);
  const b2 = (await sendRaw(w2, A2!, c2, bankOf([q(CLEAN)]))).body.bank_id;
  w2.sql.prepare(`UPDATE circle_banks SET hide_key = 1 WHERE id = ?`).run(b2);
  const path = `/circles/${c2}/banks/${b2}`;
  const g = await read(w2, V2!, path);
  assert.deepEqual([g.status, ((await g.json()) as Q).error], [409, "keys_hidden_unavailable"]);
  assert.equal((await w2.call("HEAD", path, { user: V2! })).status, 409);
  assert.equal((await read(w2, V2!, path, { "If-None-Match": "*" })).status, 409);
  assert.equal(w2.q(`SELECT 1 FROM usage_daily WHERE metric = 'circle_hidden_reads'`).length, 0, "no strip was run");
  // The PATCH records (and checks) the head.
  assert.equal((await patch(w2, A2!, c2, b2, { hide_key: true })).status, 200);
  assert.equal((await read(w2, V2!, path)).status, 200);
});

test("46-7 (variants): the invitation mail names no circle and no person, says what joining needs (an approved account with the pool's invite code), gives the expiry's time; the owner's list shows a masked address, erased with the inviter; a full circle is refused up front; counts are given back when the write fails", async () => {
  const lines: string[] = [];
  const orig = console.log;
  console.log = (...a: unknown[]) => void lines.push(a.map(String).join(" "));
  try {
    await poolNotifier({ DB: {} as D1Database, MAIL_MODE: "log", APP_URL: "http://localhost:5173" } as never)({ kind: "circle_email_invite", to: "zed@mail.example", circleName: "Your account is locked: verify at https://evil.example/login now", code: "C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01", role: "editor", expiresAt: Date.UTC(2026, 9, 31, 12) } as Mail);
  } finally {
    console.log = orig;
  }
  const text = lines.join("\n");
  assert.doesNotMatch(text, /evil|locked|verify at|https:\/\/evil/i, "no text a person chose");
  assert.match(text, /invite code/i);
  assert.match(text, /approved/i);
  assert.match(text, /2026-10-31 12:00 UTC/);
  assert.match(text, /Join a circle with a code/);
  // The owner's list: a masked address per email invite.
  const w = new World();
  const [O, A, B] = ["Olga", "Ann", "Bea"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, []);
  const r = await invite(w, O!, cid, { email: "Jane.Doe@Gmail.com", role: "viewer" });
  assert.equal(r.body.address_label, "j***@gmail.com");
  const list = (await w.json<any>("GET", `/circles/${cid}/invites`, { user: O! })).body.invites;
  assert.equal(list.find((x: Q) => x.invite_id === r.body.invite_id).address_label, "j***@gmail.com");
  assert.equal(list.length, 1);
  await erase(w, O!);
  assert.deepEqual(w.q<any>(`SELECT email_label FROM circle_invites`).map((x) => x.email_label), [null], "erased with its inviter");
  // A full circle is refused before anything is made or mailed.
  const w2 = new World();
  const [O2, M2] = ["Olga", "Max"].map((n) => w2.user("teacher", n));
  const c2 = await circleWith(w2, O2!, [[M2!, "viewer"]]);
  limit(w2, "circle_members", 2);
  const full = await invite(w2, O2!, c2, { email: "new@school.example", role: "viewer" });
  assert.deepEqual([full.status, full.body.error], [409, "circle_full"]);
  assert.equal(mails(w2).length, 0);
  assert.equal(w2.q(`SELECT 1 FROM circle_invites WHERE kind = 'email'`).length, 0);
  assert.equal(w2.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'circle_email_invites'`, O2!.id)[0]?.count ?? 0, 0);
  // A write that fails gives the day's counts and the hour's slot back.
  const w3 = new World();
  const [O3] = ["Olga"].map((n) => w3.user("teacher", n));
  const c3 = await circleWith(w3, O3!, []);
  let armed = true;
  w3.stats.log = (sql) => {
    if (armed && /INSERT INTO circle_invites/.test(sql)) {
      armed = false;
      throw new Error("D1 hiccup");
    }
  };
  assert.equal((await invite(w3, O3!, c3, { email: "x@school.example", role: "viewer" })).status, 500);
  w3.stats.log = undefined;
  assert.equal(w3.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'circle_email_invites'`, O3!.id)[0]?.count ?? 0, 0);
  assert.equal(w3.q<any>(`SELECT COALESCE(SUM(sent), 0) AS n FROM pool_alert_budget WHERE channel LIKE 'circle_email%'`)[0].n, 0);
  void A;
  void B;
});

test("46-8 (variants): the hide_key PATCH: only those who may change it get past the role check before the body is read; the same value again is a 200 without an audit line or a charge; a daily cap with a plain 429", async () => {
  const { w, A, E, V, cid } = await world();
  const bid = (await sendRaw(w, A, cid, bankOf([q("A plain question?")]))).body.bank_id;
  // A non-author editor sending a body that does not even parse is refused for the role, not for the body.
  for (const u of [E, V]) {
    const r = await w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: u, body: "{not json", contentType: "application/json" });
    assert.deepEqual([r.status, r.body.error], [403, "circle_role_required"]);
  }
  const audits = () => w.q(`SELECT 1 FROM pool_audit WHERE action IN ('circle.bank_hide_key', 'circle.bank_strip_decisions')`).length;
  assert.equal((await patch(w, A, cid, bid, { hide_key: true })).status, 200);
  const a1 = audits();
  const charged = () => w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'circle_hide_changes'`, A.id)[0]?.count ?? 0;
  const c1 = charged();
  assert.equal(c1, 1);
  const same = await patch(w, A, cid, bid, { hide_key: true });
  assert.deepEqual([same.status, same.body.unchanged], [200, true]);
  assert.equal(audits(), a1);
  assert.equal(charged(), c1);
  assert.equal((await patch(w, A, cid, bid, { hide_key: false })).status, 200);
  const offAgain = await patch(w, A, cid, bid, { hide_key: false });
  assert.deepEqual([offAgain.status, offAgain.body.unchanged], [200, true]);
  limit(w, "circle_hide_changes", charged(), A.id);
  const over = await patch(w, A, cid, bid, { hide_key: true });
  assert.deepEqual([over.status, over.body.error, over.body.metric], [429, "daily_limit", "circle_hide_changes"]);
  assert.match(over.body.message, /today|tomorrow/i);
});

test("46-9 (variants): the viewers' ETag follows re-recorded decisions; the hidden-read daily cap says so in words", async () => {
  const { w, A, V, cid } = await world();
  const s = await sendRaw(w, A, cid, bankOf([mcq("Which gas do plants take in?", [["A. Oxygen", false], ["B. Carbon dioxide", true], ["C. Helium", false]])]), { extra: { hide_key: true, labels: [{ question: 0, choice: "strip" }] } });
  const bid = s.body.bank_id;
  const path = `/circles/${cid}/banks/${bid}`;
  const e1 = (await read(w, V, path)).headers.get("ETag")!;
  assert.equal((await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "keep" }], order_acks: [{ question: 0 }] })).status, 200);
  const r2 = await read(w, V, path, { "If-None-Match": e1 });
  assert.equal(r2.status, 200, "decisions changed: the old ETag no longer matches");
  assert.notEqual(r2.headers.get("ETag"), e1);
  limit(w, "circle_hidden_reads", 0, V.id);
  const over = await w.json<any>("GET", path, { user: V });
  assert.deepEqual([over.status, over.body.metric], [429, "circle_hidden_reads"]);
  assert.match(over.body.message, /today|tomorrow/i);
});

test("46-10 (variants): a withheld version whose admin flag is open is not pruned by later versions (copyright and student data alike), nor counted as room the commit frees; once the flag is decided the next version prunes it", async () => {
  const { w, adm, A, E, P, cid } = await world();
  const item = (await publish(w, P, bankOf([UNL()]))).body as Q;
  assert.equal((await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: [0] })).status, 200);
  const s1 = await sendRaw(w, A, cid, bankOf([q(CLEAN)]));
  const bid = s1.body.bank_id;
  const keepQ = q(CLEAN, { published: { item_id: bid, qid: s1.body.qids[0], publish_id: "p", version: 1 } });
  const dirty = await sendRaw(w, E, cid, bankOf([keepQ, UNL()]), { bid, base: 1 });
  assert.equal(dirty.body.withheld, true);
  let base = 2;
  for (let i = 0; i < 21; i++) base = (await sendRaw(w, A, cid, bankOf([keepQ, q(`Later ${i}?`)]), { bid, base, confirm: true })).body.version;
  assert.equal(base, 23);
  assert.equal(w.q<any>(`SELECT status FROM circle_versions WHERE bank_id = ? AND version = 2`, bid)[0].status, "previous", "pinned while its flag is open");
  assert.equal(w.q<any>(`SELECT status FROM circle_versions WHERE bank_id = ? AND version = 1`, bid)[0].status, "pruned");
  const fid = flags(w, bid, "blocklist")[0].id;
  assert.equal((await w.admin(adm, "POST", "/admin/pool/resolve-flag", { flag_id: fid })).status, 200);
  await sendRaw(w, A, cid, bankOf([keepQ, q("After the decision?")]), { bid, base, confirm: true });
  assert.equal(w.q<any>(`SELECT status FROM circle_versions WHERE bank_id = ? AND version = 2`, bid)[0].status, "pruned");
});

test("46-11 (variants): the admin's copyright basis follows the send's rule (an expired block is no basis); unblocking a takedown marks the circle flags it caused 'block lifted' and can release their versions in the same call (never one also withheld for student data)", async () => {
  {
    const { w, adm, A, P, cid } = await world();
    const item = (await publish(w, P, bankOf([UNL()]))).body as Q;
    await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: [0] });
    const s = await sendRaw(w, A, cid, bankOf([q(CLEAN)]));
    // A bank whose records match only an EXPIRED block has no basis.
    w.sql.prepare(`INSERT OR IGNORE INTO circle_qkeys (circle_id, match_hash, bank_id, first_version, first_seq, first_by) SELECT ?, hash, ?, 1, 1, ? FROM blocked_hashes WHERE kind = 'match'`).run(cid, s.body.bank_id, A.id);
    w.sql.prepare(`UPDATE blocked_hashes SET expires_at = ?`).run(w.clock - 1);
    assert.equal((await w.admin(adm, "GET", "/admin/pool/circle-bank", null, `?id=${s.body.bank_id}&basis=copyright`)).status, 403);
  }
  {
    const { w, adm, A, E, V, P, cid } = await world();
    const item = (await publish(w, P, bankOf([UNL()]))).body as Q;
    const td = await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: [0] });
    const it2 = (await publish(w, P, bankOf([q(STUDENT)]))).body as Q;
    assert.equal((await w.purge(adm, it2.item_id, { questions: [0] })).status, 200);
    const s1 = await sendRaw(w, A, cid, bankOf([q(CLEAN)]));
    const bid = s1.body.bank_id;
    const keepQ = q(CLEAN, { published: { item_id: bid, qid: s1.body.qids[0], publish_id: "p", version: 1 } });
    assert.equal((await sendRaw(w, E, cid, bankOf([keepQ, UNL()]), { bid, base: 1 })).body.withheld, true);
    // (the bank is held while its head is withheld: its author sends the next version)
    assert.equal((await sendRaw(w, A, cid, bankOf([keepQ, UNL(), q(STUDENT)]), { bid, base: 2 })).body.withheld, true);
    const un = await w.admin<any>(adm, "POST", "/admin/dmca/unblock", { action_id: td.body.action_id, note: "Withdrawn by the complainant.", release_circle_versions: true });
    assert.equal(un.status, 200, un.text);
    assert.deepEqual([un.body.circle_flags_lifted, un.body.circle_versions_released], [2, 1]);
    const fl = flags(w, bid, "blocklist").map((f) => [JSON.parse(f.detail).version, JSON.parse(f.detail).block_lifted, f.resolved_at !== null]);
    assert.deepEqual(fl.sort(), [[2, true, true], [3, true, false]]);
    assert.equal((await read(w, A, `/circles/${cid}/banks/${bid}/versions/2`)).status, 200, "released");
    void V;
    assert.equal(((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/3`, { user: A })).body).error, "version_withheld", "still withheld for student data");
  }
});

test("46-12 (nitpick): the activity cursor must be a line of THIS circle", async () => {
  const w = new World();
  const [O, X] = ["Olga", "Xan"].map((n) => w.user("teacher", n));
  const c1 = await circleWith(w, O!, []);
  const c2 = await circleWith(w, X!, [], "Other");
  const foreign = w.q<any>(`SELECT id, at FROM pool_audit WHERE item_id = ? LIMIT 1`, c2)[0];
  const r = await w.json<any>("GET", `/circles/${c1}/activity?before=${foreign.at}:${foreign.id}`, { user: O! });
  assert.deepEqual([r.status, r.body.field], [400, "before"]);
  const own = w.q<any>(`SELECT id, at FROM pool_audit WHERE item_id = ? LIMIT 1`, c1)[0];
  assert.equal((await w.json<any>("GET", `/circles/${c1}/activity?before=${own.at}:${own.id}`, { user: O! })).status, 200);
});
