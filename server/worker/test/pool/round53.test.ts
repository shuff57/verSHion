// Round 53: carried-over backend items (critic 56a nitpicks, critic 61 M1's note, the staging run's hide-on nit).
// 1. The join's use of a code is a compare-and-set on the code it read: an invite rotated (round 48: a new code, role and
//    expiry on the same invite row) or given another role between the join's lookup and its write is not redeemed.
// 2. Two hide-on PATCHes at once: the one whose batch runs second (the setting and the head's decisions changed since it
//    read them) is `200 {unchanged: true}` when the bank now hides its keys as it asked (no activity line, no change
//    counted, no flag, nothing written onto the head); with other decisions recorded meanwhile it is `409 conflict`.
// 3. `labels_unused` (a label decision for a question or part with no label finding) is documented with the PATCH.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import type { PoolNotice } from "../../src/pool/shared.ts";
import { framed, publish, publishId, World, type User } from "./harness.ts";

type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "SAMPLEKEYzzq" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "local", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const STEM = "Copyrighted textbook problem 4.7: which organelle in the eukaryotic cell produces most of the ATP in respiration?";
const LAB = () => mcq(STEM, [["A. Mitochondrion", true], ["B. Ribosome", false], ["C. Golgi body", false], ["D. Lysosome", false]]);
const UNL = () => mcq(STEM, [["Mitochondrion", true], ["Ribosome", false], ["Golgi body", false], ["Lysosome", false]]);
const CLEAN = "Clean question about eskers and drumlins and the glaciers that left them?";
type Mail = Extract<PoolNotice, { kind: "circle_email_invite" }>;
const mails = (w: World) => w.notices.filter((n) => n.kind === "circle_email_invite") as Mail[];
const used = (w: World, u: User, metric: string) => w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = ?`, u.id, metric)[0]?.count ?? 0;
const audits = (w: World, action: string) => w.q<any>(`SELECT id FROM pool_audit WHERE action = ?`, action).length;
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;

async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][]): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name: "Algebra team" } })).body.circle.id;
  for (const [u, role] of members) {
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } })).body.code;
    assert.equal((await w.json("POST", "/circles/join", { user: u, json: { code: k } })).status, 200);
    w.advance(1000);
  }
  return cid;
}
async function sendRaw(w: World, u: User, cid: string, payload: string) {
  const meta: Q = { commit_id: publishId(), license: "circle-only", attest: { own_work: true, terms_version: POOL_TERMS_VERSION }, meta: { title: "Unit 2", subject: "Math", course: "", grade: "", tags: [], standards: [] } };
  return w.json<any>("POST", `/circles/${cid}/banks`, { user: u, body: framed(meta, payload), contentType: PUB });
}
const invite = (w: World, u: User, cid: string, body: Q) => w.json<any>("POST", `/circles/${cid}/invites/email`, { user: u, json: body });
const join = (w: World, u: User, code: string) => w.json<any>("POST", "/circles/join", { user: u, json: { code } });
const patch = (w: World, u: User, cid: string, bid: string, body: Q) => w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: u, json: body });
const activity = async (w: World, u: User, cid: string) => (await w.json<any>("GET", `/circles/${cid}/activity`, { user: u })).body.activity as Q[];
const member = (w: World, cid: string, u: User) => w.q<any>(`SELECT role FROM circle_members WHERE circle_id = ? AND user_id = ? AND left_at IS NULL`, cid, u.id)[0]?.role ?? null;

/** Runs `between` (once) just before the next D1 batch holding a statement that matches `re`. */
function beforeBatch(w: World, re: RegExp, between: () => Promise<void>) {
  const db = w.db as any;
  const batch = db.batch.bind(db);
  let fired = false;
  db.batch = async (ss: { sql: string }[]) => {
    if (!fired && ss.some((s) => re.test(s.sql))) {
      fired = true;
      await between();
    }
    return batch(ss);
  };
}

test("53-1 (join race): an email invite rotated between the join's lookup and its write is not redeemed with the old code; the new code joins with the new role", async () => {
  const w = new World();
  const O = w.user("teacher", "Olga");
  const N = w.user("teacher", "Nia");
  const cid = await circleWith(w, O, []);
  assert.equal((await invite(w, O, cid, { email: "nia@pvhs.example", role: "editor" })).status, 202);
  const old = mails(w)[0]!.code;
  // The owner sends the invitation again (as a viewer) while Nia's join of the old code is between its read and its write.
  let rotated: Q = {};
  beforeBatch(w, /SET uses = uses \+ 1/, async () => void (rotated = await invite(w, O, cid, { email: "nia@pvhs.example", role: "viewer" })));
  const j = await join(w, N, old);
  assert.deepEqual([rotated.status, rotated.body.resent], [202, true], JSON.stringify(rotated.body));
  assert.deepEqual([j.status, j.body.error], [422, "circle_invite_invalid"], JSON.stringify(j.body));
  assert.equal(member(w, cid, N), null, "the old code admitted nobody");
  assert.equal(w.q<any>(`SELECT uses FROM circle_invites WHERE circle_id = ? AND kind = 'email'`, cid)[0].uses, 0);
  assert.equal(audits(w, "circle.join"), 0);
  // The code the second mail carries works, with the role it was sent with.
  const fresh = mails(w)[1]!.code;
  const j2 = await join(w, N, fresh);
  assert.deepEqual([j2.status, j2.body.role], [200, "viewer"], JSON.stringify(j2.body));
  assert.equal(member(w, cid, N), "viewer");
});

test("53-2 (join race): an invite whose role changed between the join's lookup and its write is not redeemed with the role read; a retry joins with the role it has now", async () => {
  const w = new World();
  const O = w.user("teacher", "Olga");
  const N = w.user("teacher", "Nia");
  const cid = await circleWith(w, O, []);
  const code = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: O, json: { role: "editor" } })).body.code as string;
  // (no route changes a code's role alone today: a rotation changes its code too; the write is pinned to both all the same)
  w.stats.log = (sql) => {
    if (!/SET uses = uses \+ 1/.test(sql)) return;
    w.stats.log = undefined;
    w.sql.prepare(`UPDATE circle_invites SET role = 'viewer' WHERE circle_id = ?`).run(cid);
  };
  const j = await join(w, N, code);
  w.stats.log = undefined;
  assert.notEqual(member(w, cid, N), "editor", "never the role the join read");
  assert.deepEqual([j.status, j.body.error], [422, "circle_invite_invalid"], JSON.stringify(j.body));
  assert.equal(w.q<any>(`SELECT uses FROM circle_invites WHERE circle_id = ?`, cid)[0].uses, 0);
  const j2 = await join(w, N, code);
  assert.deepEqual([j2.status, j2.body.role], [200, "viewer"], JSON.stringify(j2.body));
});

/** A second isolate (its own heavy slot) over the same database and bucket: a PATCH that runs while another is mid-flight. */
function isolate(w: World): World {
  const iso = new World({ sql: w.sql, r2: w.r2, heavySlots: { active: 0, queue: [] } });
  iso.tokens = w.tokens;
  iso.clock = w.clock;
  return iso;
}
async function world() {
  const w = new World();
  const adm = w.user("admin");
  const [O, A, V, P] = ["Olga", "Ann", "Vic", "Pat"].map((n) => w.user("teacher", n)) as User[];
  const cid = await circleWith(w, O!, [[A!, "editor"], [V!, "viewer"]]);
  return { w, adm, O: O!, A: A!, V: V!, P: P!, cid };
}
/** `outer` reads the bank and the head, then (at its strip's R2 read of the head) `inner` runs to the end on another isolate. */
async function raced(w: World, outer: () => Promise<{ status: number; body: Q }>, inner: (iso: World) => Promise<{ status: number; body: Q }>) {
  const iso = isolate(w);
  let first: { status: number; body: Q } | null = null;
  w.r2.onGet = async () => void (first = await inner(iso));
  const second = await outer();
  assert.ok(first, "the inner PATCH ran");
  return { first: first as { status: number; body: Q }, second };
}

test("53-3 (staging nit): two hide-on PATCHes at once with the same decisions: the second batch is `unchanged` (one activity line, one change counted)", async () => {
  const { w, A, V, cid } = await world();
  const bid = (await sendRaw(w, A, cid, bankOf([q(CLEAN)]))).body.bank_id as string;
  const { first, second } = await raced(w, () => patch(w, A, cid, bid, { hide_key: true }), (iso) => patch(iso, A, cid, bid, { hide_key: true }));
  assert.deepEqual([first.status, first.body.hide_key, first.body.unchanged], [200, true, undefined], JSON.stringify(first.body));
  assert.deepEqual([second.status, second.body.hide_key, second.body.unchanged], [200, true, true], JSON.stringify(second.body));
  assert.equal(audits(w, "circle.bank_hide_key"), 1, "one activity line");
  assert.equal(audits(w, "circle.bank_strip_decisions"), 0);
  assert.equal((await activity(w, V, cid)).filter((x) => x.what === "hide_key_changed").length, 1);
  assert.equal(used(w, A, "circle_hide_changes"), 1, "one change counted");
  assert.equal(used(w, A, "circle_hide_attempts"), 2, "both did the work");
  assert.equal(w.q<any>(`SELECT hide_key FROM circle_banks WHERE id = ?`, bid)[0].hide_key, 1);
});

test("53-4 (critic 61 M1's note): two hide-on PATCHes at once whose viewers' form matches a takedown: one flag, one withhold, the second adds no flag and writes nothing onto the head", async () => {
  const { w, adm, A, P, cid } = await world();
  const bid = (await sendRaw(w, A, cid, bankOf([LAB()]))).body.bank_id as string;
  const item = (await publish(w, P, bankOf([UNL()]))).body as Q;
  assert.equal((await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: [0] })).status, 200);
  const body = { hide_key: true, labels: [{ question: 0, choice: "strip" }] };
  const { first, second } = await raced(w, () => patch(w, A, cid, bid, body), (iso) => patch(iso, A, cid, bid, body));
  assert.deepEqual([first.status, first.body.withheld, first.body.held], [200, true, true], JSON.stringify(first.body));
  assert.deepEqual([second.status, second.body.unchanged, second.body.withheld], [200, true, undefined], JSON.stringify(second.body));
  assert.equal(w.q<any>(`SELECT id FROM admin_flags WHERE kind = 'blocklist' AND item_id = ?`, bid).length, 1, "no second (empty) flag");
  assert.equal(audits(w, "circle.bank_hide_key"), 1);
  assert.equal(used(w, A, "circle_hide_changes"), 1);
});

test("53-5: two hide-on PATCHes at once with different decisions: the second is `409 conflict` and nothing of it is written (the first's decisions stay on the head)", async () => {
  const { w, adm, O, A, P, cid } = await world();
  const bid = (await sendRaw(w, A, cid, bankOf([LAB()]))).body.bank_id as string;
  const item = (await publish(w, P, bankOf([UNL()]))).body as Q;
  assert.equal((await w.takedown<any>(adm, await notice(w, adm), item.item_id, { questions: [0] })).status, 200);
  // The owner keeps the labels (the viewers' form matches nothing); the author's strip of them, read before, comes second.
  const keep = { hide_key: true, labels: [{ question: 0, choice: "keep" }], order_acks: [{ question: 0 }] };
  const { first, second } = await raced(w, () => patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "strip" }] }), (iso) => patch(iso, O, cid, bid, keep));
  assert.deepEqual([first.status, first.body.withheld], [200, false], JSON.stringify(first.body));
  assert.deepEqual([second.status, second.body.error], [409, "conflict"], JSON.stringify(second.body));
  const head = w.q<any>(`SELECT v.withheld_at, v.strip_opts FROM circle_versions v JOIN circle_banks b ON b.head_version_id = v.id WHERE b.id = ?`, bid)[0];
  assert.equal(head.withheld_at, null, "the second's match did not withhold the head");
  assert.equal(JSON.parse(head.strip_opts).labels[0].choice, "keep", "the first's decisions stay");
  assert.equal(w.q<any>(`SELECT id FROM admin_flags WHERE item_id = ?`, bid).length, 0);
  assert.equal(audits(w, "circle.bank_hide_key"), 1);
  assert.deepEqual([used(w, O, "circle_hide_changes"), used(w, A, "circle_hide_changes")], [1, 0], "the loser's change is given back");
  // Done again, it is a change like any other.
  const again = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "strip" }] });
  assert.deepEqual([again.status, again.body.withheld], [200, true], JSON.stringify(again.body));
});

test("53-6 (critic 56a V5): a label decision for a question with no label finding is `422 labels_need_decision` with `errors[].code` `labels_unused`, as the README's PATCH row says", async () => {
  const { w, A, cid } = await world();
  const bid = (await sendRaw(w, A, cid, bankOf([q(CLEAN)]))).body.bank_id as string;
  const r = await patch(w, A, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "keep" }] });
  assert.deepEqual([r.status, r.body.error, r.body.errors?.[0]?.code], [422, "labels_need_decision", "labels_unused"], JSON.stringify(r.body));
  assert.deepEqual(r.body.label_findings ?? [], []);
  assert.deepEqual([used(w, A, "circle_hide_attempts"), used(w, A, "circle_hide_changes")], [1, 0], "an attempt, not a change");
  const readme = readFileSync(new URL("../../README.md", import.meta.url), "utf8");
  const row = readme.split("\n").find((l) => l.startsWith("  | `PATCH /circles/:cid/banks/:bid`"));
  assert.ok(row, "the circle route table has the PATCH row");
  assert.match(row!, /labels_unused/);
});
