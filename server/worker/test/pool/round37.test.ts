// Round 37: circles PHASE B, shared editing (docs/circles-plan.md 4, 6.2, 13.2; D16, D17). The server keeps one
// official version of each circle bank and its history; editors send versions against a base (compare-and-set, 409
// with the true head), see the history, fetch a kept version (the merge base), start a merge (a signed token that
// records "merged from vX"), and the owner or the bank's author rolls back. No live co-editing, no automatic merge,
// no locks, no branches.
import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { bytesReserved } from "../../src/pool/shared.ts";
import { framed, publish, publishId, rng, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "a model answer kept with the key" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "local", name: "b", questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const stemsOf = (n: number, tag: string) => Array.from({ length: n }, (_, i) => `${tag} question ${i}: how does the delta's distributary channel ${i} shift over a decade?`);

async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][]): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name: "Algebra team" } })).body.circle.id;
  for (const [u, role] of members) {
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } })).body.code;
    assert.equal((await w.json("POST", "/circles/join", { user: u, json: { code: k } })).status, 200);
    w.advance(1000);
  }
  return cid;
}
interface SendOpts {
  bid?: string;
  base?: number;
  commitId?: string;
  license?: string;
  message?: string;
  mergeToken?: string;
  confirm?: boolean;
  title?: string;
}
/** A question: a stem (a new question), "qid:<qid>:<stem>" (the question the head holds as <qid>, as the app sends it back), or a whole question object. */
type Item = string | Q;
const toQ = (x: Item, bid?: string): Q => {
  if (typeof x !== "string") return x;
  if (!x.startsWith("qid:")) return q(x);
  const [, qid, ...rest] = x.split(":");
  return q(rest.join(":"), { published: { item_id: bid, qid, publish_id: "p", version: 1 } });
};
async function send(w: World, u: User, cid: string, items: Item[], o: SendOpts = {}) {
  const meta: Q = {
    commit_id: o.commitId ?? publishId(),
    license: o.license ?? "circle-only",
    attest: { own_work: true, terms_version: POOL_TERMS_VERSION },
    meta: { title: o.title ?? "Unit 2", subject: "Math", course: "", grade: "", tags: [], standards: [] },
    ...(o.message ? { message: o.message } : {}),
    ...(o.bid ? { base_version: o.base } : {}),
    ...(o.mergeToken ? { merge_token: o.mergeToken } : {}),
    ...(o.confirm ? { confirm_large_change: true } : {}),
  };
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, bankOf(items.map((x) => toQ(x, o.bid)))), contentType: PUB });
}
const keep = (qids: string[], stems: string[]) => stems.map((s, i) => (i < qids.length ? `qid:${qids[i]}:${s}` : s));
const readStems = async (w: World, u: User, path: string) => {
  const r = await w.json<any>("GET", path, { user: u });
  return { status: r.status, body: r.body, stems: r.status === 200 ? (r.body.payload.bank.questions as Q[]).map((x) => x.stem) : [] };
};
const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};
const history = async (w: World, u: User, cid: string, bid: string, before?: number) => w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions${before ? `?before=${before}` : ""}`, { user: u });
const rollback = (w: World, u: User, cid: string, bid: string, body: Q) => w.json<any>("POST", `/circles/${cid}/banks/${bid}/rollback`, { user: u, json: { commit_id: publishId(), ...body } });
const merge = (w: World, u: User, cid: string, bid: string, base: number) => w.json<any>("POST", `/circles/${cid}/banks/${bid}/merge`, { user: u, json: { base_version: base } });
/** A null anywhere in a response but the keys listed (round 36's rule for circle responses). */
function nulls(v: unknown, allowed: Set<string>, path = "$", out: string[] = []): string[] {
  if (Array.isArray(v)) v.forEach((x, i) => nulls(x, allowed, `${path}[${i}]`, out));
  else if (v && typeof v === "object")
    for (const [k, x] of Object.entries(v as Q)) {
      if (k === "payload") continue;
      if (x === null) {
        if (!allowed.has(k)) out.push(`${path}.${k}`);
      } else nulls(x, allowed, `${path}.${k}`, out);
    }
  return out;
}

// ---- 1. editors send versions ----------------------------------------------------------------------------

test("37-1: the owner and editors send versions of any bank they may read (viewers cannot); only the bank's author changes its licence; a held bank takes versions only from its author; the history names each sender", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [A, E, O2, V] = ["Ann", "Eli", "Ola", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"], [O2!, "editor"], [V!, "viewer"]]);
  const s1 = await send(w, E!, cid, stemsOf(3, "Eli"), { message: "first" });
  assert.equal(s1.status, 201);
  const bid = s1.body.bank_id, qids = s1.body.qids as string[];
  // Another editor sends version 2, the owner version 3.
  const s2 = await send(w, O2!, cid, keep(qids, [...stemsOf(3, "Eli"), "Ola adds a question about levees and floods?"]), { bid, base: 1, message: "Ola's addition" });
  assert.deepEqual([s2.status, s2.body.version, s2.body.added, s2.body.changed, s2.body.removed, s2.body.held], [200, 2, 1, 0, 0, false], JSON.stringify(s2.body));
  assert.deepEqual(s2.body.qids.slice(0, 3), qids, "qids stay");
  const s3 = await send(w, A!, cid, keep(s2.body.qids, [...stemsOf(3, "Eli"), "Ola adds a question about levees and floods?"]).map((x, i) => (i === 0 ? x.replace("decade", "century") : x)), { bid, base: 2 });
  assert.deepEqual([s3.status, s3.body.version, s3.body.changed], [200, 3, 1]);
  // A viewer cannot; an editor cannot change the licence (the author sets it, D19), the author can.
  assert.equal((await send(w, V!, cid, stemsOf(1, "Vic"), { bid, base: 3 })).body.error, "circle_role_required");
  const lic = await send(w, O2!, cid, keep(s3.body.qids, stemsOf(4, "x")), { bid, base: 3, license: "CC-BY-4.0" });
  assert.deepEqual([lic.status, lic.body.error, lic.body.license], [422, "license_author_only", "circle-only"]);
  assert.equal((await send(w, E!, cid, keep(s3.body.qids, stemsOf(4, "x")), { bid, base: 3, license: "CC-BY-4.0" })).status, 200);
  // ... and an editor then sends with the bank's new licence.
  const s5 = await send(w, O2!, cid, keep(s3.body.qids, stemsOf(4, "y")), { bid, base: 4, license: "CC-BY-4.0" });
  assert.equal(s5.status, 200, JSON.stringify(s5.body));
  // Held: hidden from members, so editors cannot send to it (404); its author can (and is told it is held).
  await w.admin(adm, "POST", "/admin/pool/circle-hold", { bank_id: bid, note: "Checking a report by hand." });
  assert.equal((await send(w, O2!, cid, stemsOf(1, "held"), { bid, base: 5, license: "CC-BY-4.0" })).status, 404);
  const s6 = await send(w, E!, cid, keep(s3.body.qids, stemsOf(4, "z")).concat(["fix"]), { bid, base: 5, license: "CC-BY-4.0" });
  assert.deepEqual([s6.status, s6.body.held], [200, true]);
  assert.equal((await history(w, V!, cid, bid)).status, 404, "held: not for a member who may not read it");
  await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, note: "Checked, nothing wrong." });
  const h2 = await history(w, V!, cid, bid);
  assert.deepEqual(
    h2.body.versions.map((x: Q) => [x.version, x.by, x.message, x.license]),
    [[6, "Eli", "", "CC-BY-4.0"], [5, "Ola", "", "CC-BY-4.0"], [4, "Eli", "", "CC-BY-4.0"], [3, "Ann", "", "circle-only"], [2, "Ola", "Ola's addition", "circle-only"], [1, "Eli", "first", "circle-only"]],
  );
  // The activity list says who sent what.
  const act = (await w.json<any>("GET", `/circles/${cid}/activity`, { user: V! })).body.activity.filter((a: Q) => a.what === "version_sent");
  assert.deepEqual(act.map((a: Q) => [a.by, a.version]).slice(0, 3), [["Eli", 6], ["Ola", 5], ["Eli", 4]]);
});

// ---- 2. compare-and-set: exactly one wins; idempotent retries ----------------------------------------------

test("37-2: two editors sending from the same base: exactly one wins, the other gets 409 with the true head (who, when); a retry with the same commit_id replays, another body under it is refused; with the second send in ANOTHER isolate at every statement boundary of the first, still exactly one", async () => {
  const w = new World();
  const [A, E1, E2] = ["Ann", "Eva", "Edd"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E1!, "editor"], [E2!, "editor"]]);
  const s1 = await send(w, A!, cid, stemsOf(2, "Ann"));
  const bid = s1.body.bank_id;
  const c1 = publishId();
  const a = await send(w, E1!, cid, keep(s1.body.qids, stemsOf(3, "Eva")), { bid, base: 1, commitId: c1 });
  assert.equal(a.status, 200);
  const at = w.q<any>(`SELECT committed_at FROM circle_versions WHERE id = ?`, a.body.version_id)[0].committed_at;
  w.advance(60_000);
  const b = await send(w, E2!, cid, keep(s1.body.qids, stemsOf(3, "Edd")), { bid, base: 1 });
  assert.deepEqual([b.status, b.body.error, b.body.head_version, b.body.head_by, b.body.head_at], [409, "conflict", 2, "Eva", at]);
  const replay = await send(w, E1!, cid, keep(s1.body.qids, stemsOf(3, "Eva")), { bid, base: 1, commitId: c1 });
  assert.deepEqual([replay.status, replay.body.replayed, replay.body.version_id], [200, true, a.body.version_id]);
  assert.equal((await send(w, E1!, cid, stemsOf(1, "other"), { bid, base: 2, commitId: c1 })).body.error, "commit_id_conflict");
  assert.equal((await send(w, E2!, cid, keep(s1.body.qids, stemsOf(3, "Eva")), { bid, base: 1, commitId: c1 })).body.error, "commit_id_conflict", "someone else's commit_id");
  assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ?`, bid).length, 2, "nothing stored for the loser");
  // Two isolates (each with its own heavy slot) over one database: Y's whole send runs at every statement
  // boundary of X's; exactly one wins every time, and nothing of the loser stays.
  const wins = { X: 0, Y: 0 };
  for (let k = 0; ; k++) {
    const w2 = new World();
    const iso = new World({ sql: w2.sql, r2: w2.r2, heavySlots: { active: 0, queue: [] } });
    iso.tokens = w2.tokens;
    const [A2, X, Y] = ["Ann", "X", "Y"].map((n) => w2.user("teacher", n));
    const c2 = await circleWith(w2, A2!, [[X!, "editor"], [Y!, "editor"]]);
    const t = await send(w2, A2!, c2, stemsOf(2, "Ann"));
    iso.clock = w2.clock;
    const db = w2.db as any;
    const prep = db.prepare.bind(db), batch = db.batch.bind(db);
    let n = 0, inner = false, innerStatus = 0;
    const hit = async () => {
      if (inner) return;
      if (n++ === k) {
        inner = true;
        innerStatus = (await send(iso, Y!, c2, keep(t.body.qids, stemsOf(3, "Y")), { bid: t.body.bank_id, base: 1 })).status;
        inner = false;
      }
    };
    const wrap = (s: any): any => {
      const o = Object.create(s);
      o.bind = (...x: unknown[]) => wrap(s.bind(...x));
      for (const m of ["first", "all", "run"]) o[m] = async () => (await hit(), s[m]());
      return o;
    };
    db.prepare = (sql: string) => wrap(prep(sql));
    db.batch = async (ss: unknown[]) => (await hit(), batch(ss));
    let outer = 0;
    try {
      outer = (await send(w2, X!, c2, keep(t.body.qids, stemsOf(3, "X")), { bid: t.body.bank_id, base: 1 })).status;
    } finally {
      db.prepare = prep;
      db.batch = batch;
    }
    if (k >= n) innerStatus = (await send(iso, Y!, c2, keep(t.body.qids, stemsOf(3, "Y")), { bid: t.body.bank_id, base: 1 })).status;
    const versions = w2.q<{ version: number; committer_id: string }>(`SELECT version, committer_id FROM circle_versions WHERE bank_id = ? AND status <> 'staging' ORDER BY version`, t.body.bank_id);
    assert.deepEqual([outer, innerStatus].sort(), [200, 409], `boundary ${k}/${n}: ${outer} / ${innerStatus}`);
    assert.deepEqual(versions.map((v) => v.version), [1, 2], `boundary ${k}`);
    assert.equal(versions[1]!.committer_id, outer === 200 ? X!.id : Y!.id, `boundary ${k}: the winner's version is the head`);
    assert.equal(w2.q(`SELECT 1 FROM circle_versions WHERE status = 'staging'`).length, 0, `boundary ${k}: no staging left`);
    assert.equal(w2.r2.keys(`circles/${c2}/`).length, 4, `boundary ${k}: the loser's R2 objects are gone`);
    if (outer === 200) wins.X++;
    else wins.Y++;
    if (k >= n) break;
  }
  assert.ok(wins.X > 0 && wins.Y > 0, JSON.stringify(wins));
});

// ---- 3. large changes ----------------------------------------------------------------------------------------

test("37-3: a version that drops more than half of the bank's questions needs confirm_large_change (428 confirm_large_required {removed, total}, the day's send given back); exactly half does not", async () => {
  const w = new World();
  const [A, E] = ["Ann", "Eli"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"]]);
  const s = await send(w, A!, cid, stemsOf(10, "Ann"));
  const bid = s.body.bank_id, qids = s.body.qids as string[];
  const half = await send(w, E!, cid, keep(qids.slice(0, 5), stemsOf(5, "Ann")), { bid, base: 1 });
  assert.deepEqual([half.status, half.body.removed], [200, 5]);
  const used = () => w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'circle_commits'`, E!.id)[0].count;
  const before = used();
  const big = await send(w, E!, cid, keep(qids.slice(0, 2), stemsOf(2, "Ann")), { bid, base: 2 });
  assert.deepEqual([big.status, big.body.error, big.body.removed, big.body.total], [428, "confirm_large_required", 3, 5]);
  assert.equal(used(), before, "given back");
  // Questions sent without their qids are new questions: the old ones count as removed.
  assert.equal((await send(w, E!, cid, stemsOf(5, "Ann"), { bid, base: 2 })).status, 428);
  const ok = await send(w, E!, cid, keep(qids.slice(0, 2), stemsOf(2, "Ann")), { bid, base: 2, confirm: true });
  assert.deepEqual([ok.status, ok.body.version], [200, 3]);
});

// ---- 4. history and kept versions ------------------------------------------------------------------------------

test("37-4: the history (newest first, 50 a page) and any kept version's body (the merge base, exactly what was sent); a pruned version is 404 version_not_retained {oldest_kept}; non-members 404; no nulls", async () => {
  const w = new World();
  const [A, E, V, X] = ["Ann", "Eli", "Vic", "Xia"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"], [V!, "viewer"]]);
  let r = await send(w, A!, cid, ["v1 question about oxbows?"]);
  const bid = r.body.bank_id;
  const sent: string[][] = [["v1 question about oxbows?"]];
  for (let v = 2; v <= 60; v++) {
    const stems = [`v${v} question about oxbows and their lakes?`];
    r = await send(w, v % 2 ? A! : E!, cid, keep(r.body.qids, stems), { bid, base: v - 1, message: `msg ${v}` });
    assert.equal(r.status, 200, JSON.stringify(r.body));
    sent.push(stems);
  }
  const p1 = await history(w, V!, cid, bid);
  assert.equal(p1.status, 200);
  assert.deepEqual([p1.body.head_version, p1.body.oldest_kept, p1.body.versions.length, p1.body.next_before], [60, 41, 50, 11]);
  const v60 = p1.body.versions[0];
  assert.deepEqual([v60.version, v60.by, v60.message, v60.kept, v60.changed, v60.question_count, "rollback_of" in v60, "merged_from" in v60], [60, "Eli", "msg 60", true, 1, 1, false, false]);
  assert.equal(p1.body.versions.find((x: Q) => x.version === 40).kept, false);
  const p2 = await history(w, V!, cid, bid, p1.body.next_before);
  assert.deepEqual([p2.body.versions.map((x: Q) => x.version), p2.body.next_before], [[10, 9, 8, 7, 6, 5, 4, 3, 2, 1], null]);
  assert.equal((await w.json("GET", `/circles/${cid}/banks/${bid}/versions?before=x`, { user: V! })).status, 400);
  // A kept version's body is exactly what was sent; a pruned one is not retained; others do not exist.
  for (const v of [41, 50, 60]) {
    const b = await readStems(w, V!, `/circles/${cid}/banks/${bid}/versions/${v}`);
    assert.deepEqual([b.status, b.body.bank.version, b.body.bank.head_version, b.body.bank.message, b.stems], [200, v, 60, `msg ${v}`, sent[v - 1]]);
    assert.deepEqual(nulls(b.body, new Set()), []);
  }
  const gone = await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/40`, { user: V! });
  assert.deepEqual([gone.status, gone.body.error, gone.body.oldest_kept], [404, "version_not_retained", 41]);
  assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/61`, { user: V! })).body.error, "not_found");
  assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/0`, { user: V! })).status, 404);
  // A version's ETag answers 304 (while the head stays where it is).
  const first = await w.call("GET", `/circles/${cid}/banks/${bid}/versions/50`, { user: V! });
  const tag = first.headers.get("ETag")!;
  assert.equal((await w.call("GET", `/circles/${cid}/banks/${bid}/versions/50`, { user: V!, headers: { "If-None-Match": tag } })).status, 304);
  // No nulls but the cursor (round 36).
  assert.deepEqual(nulls(p1.body, new Set(["next_before"])), []);
  // Non-members (and a former member): 404 everywhere.
  assert.equal((await w.json("POST", `/circles/${cid}/leave`, { user: E! })).status, 200);
  for (const u of [X!, E!]) {
    for (const path of [`/circles/${cid}/banks/${bid}/versions`, `/circles/${cid}/banks/${bid}/versions/50`]) assert.equal((await w.json("GET", path, { user: u })).status, 404);
    assert.equal((await merge(w, u, cid, bid, 50)).status, 404);
    assert.equal((await rollback(w, u, cid, bid, { to_version: 50, base_version: 60 })).status, 404);
  }
});

// ---- 5. merge-initiate --------------------------------------------------------------------------------------------

test("37-5: a merge starts with POST .../merge {base_version}: the head, whether the base is kept, the versions in between, and a signed 30-minute token; a send with it records merged_from; a token for someone else, another bank, another head, tampered or past 30 minutes is refused", async () => {
  const w = new World();
  const [A, E, F, V] = ["Ann", "Eli", "Fay", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"], [F!, "editor"], [V!, "viewer"]]);
  const s = await send(w, A!, cid, stemsOf(3, "Ann"));
  const bid = s.body.bank_id, qids = s.body.qids as string[];
  // E works from version 1 while F sends versions 2 and 3.
  let r = await send(w, F!, cid, keep(qids, stemsOf(3, "Ann")).concat(["Fay's question one?"]), { bid, base: 1, message: "Fay 1" });
  r = await send(w, F!, cid, keep(r.body.qids, [...stemsOf(3, "Ann"), "Fay's question one?", "Fay's question two?"]), { bid, base: 2, message: "Fay 2" });
  const conflict = await send(w, E!, cid, keep(qids, stemsOf(3, "Eli")), { bid, base: 1 });
  assert.equal(conflict.status, 409);
  assert.equal((await merge(w, V!, cid, bid, 1)).status, 403, "viewers do not send");
  const mg = await merge(w, E!, cid, bid, 1);
  assert.equal(mg.status, 200, JSON.stringify(mg.body));
  assert.deepEqual(
    [mg.body.base_version, mg.body.base_kept, mg.body.head_version, mg.body.head_by, mg.body.between.map((x: Q) => [x.version, x.by, x.message, x.added]), mg.body.between_total],
    // (round 38: the first versions after the base, oldest first)
    [1, true, 3, "Fay", [[2, "Fay", "Fay 1", 1], [3, "Fay", "Fay 2", 1]], 2],
  );
  assert.equal(mg.body.merge_token_expires_at, w.clock + 30 * 60_000);
  assert.deepEqual(nulls(mg.body, new Set()), []);
  // E merges on the device, then sends based on the head with the token: the history records the merge.
  const merged = await send(w, E!, cid, keep(r.body.qids, [...stemsOf(3, "Eli"), "Fay's question one?", "Fay's question two?"]), { bid, base: 3, mergeToken: mg.body.merge_token });
  assert.deepEqual([merged.status, merged.body.version, merged.body.merged_from], [200, 4, 1], JSON.stringify(merged.body));
  assert.equal((await history(w, V!, cid, bid)).body.versions[0].merged_from, 1);
  const act = (await w.json<any>("GET", `/circles/${cid}/activity`, { user: V! })).body.activity;
  assert.deepEqual([act[0].what, act[0].version, act[0].merged_from], ["version_sent", 4, 1]);
  // Refused tokens: another sender, tampered, a head that moved on, past 30 minutes, another bank; nothing is stored.
  const mg2 = await merge(w, E!, cid, bid, 2);
  const tail = (mergedQids: string[]) => keep(mergedQids, stemsOf(5, "x"));
  assert.equal((await send(w, F!, cid, tail(merged.body.qids), { bid, base: 4, mergeToken: mg2.body.merge_token })).body.error, "merge_token_invalid", "another sender");
  const tok: string = mg2.body.merge_token;
  const tamper = tok.slice(0, 5) + (tok[5] === "A" ? "B" : "A") + tok.slice(6);
  assert.equal((await send(w, E!, cid, tail(merged.body.qids), { bid, base: 4, mergeToken: tamper })).body.error, "merge_token_invalid", "tampered");
  assert.equal((await send(w, E!, cid, tail(merged.body.qids), { bid, base: 4, mergeToken: "nonsense" })).body.error, "merge_token_invalid");
  const other = await send(w, A!, cid, stemsOf(1, "Other bank"));
  const o2 = await send(w, A!, cid, keep(other.body.qids, stemsOf(2, "Other bank")), { bid: other.body.bank_id, base: 1 });
  assert.equal(o2.status, 200);
  const mg3 = await merge(w, E!, cid, other.body.bank_id, 1);
  assert.equal((await send(w, E!, cid, tail(merged.body.qids), { bid, base: 4, mergeToken: mg3.body.merge_token })).body.error, "merge_token_invalid", "another bank");
  w.advance(31 * 60_000);
  assert.equal((await send(w, E!, cid, tail(merged.body.qids), { bid, base: 4, mergeToken: mg2.body.merge_token })).body.error, "merge_token_invalid", "expired");
  assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ?`, bid).length, 4);
  // A base past the kept 20: base_kept false and the oldest kept, so the app falls back to a two-way review.
  for (let v = 5; v <= 25; v++) r = await send(w, A!, cid, [`v${v} stem about meanders?`], { bid, base: v - 1, confirm: true });
  const old = await merge(w, E!, cid, bid, 3);
  assert.deepEqual([old.status, old.body.base_kept, old.body.oldest_kept, old.body.between_total, old.body.between.length], [200, false, 6, 22, 22]);
  // A token whose head moved on (someone sent after the merge started).
  r = await send(w, A!, cid, [`v26 stem about meanders?`], { bid, base: 25, confirm: true });
  assert.equal((await send(w, E!, cid, [`E's merge`], { bid, base: 26, mergeToken: old.body.merge_token, confirm: true })).body.error, "merge_token_invalid", "another head");
  assert.equal((await merge(w, E!, cid, bid, 26)).status, 400, "nothing to merge from the head itself");
  assert.equal((await merge(w, E!, cid, bid, 27)).status, 400);
});

// ---- 6. rollback ----------------------------------------------------------------------------------------------------

test("37-6: rollback (owner or the bank's author, D17) makes a NEW version equal to a kept one, with the same compare-and-set; never a pruned one; 20 a day; idempotent by commit_id; a large drop needs the confirm", async () => {
  const w = new World();
  const [A, E, O] = ["Ann", "Eli", "Owner"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"]]);
  let r = await send(w, A!, cid, stemsOf(4, "v1"));
  const bid = r.body.bank_id;
  const v1qids = r.body.qids as string[];
  r = await send(w, E!, cid, keep(v1qids, stemsOf(4, "v2")), { bid, base: 1, message: "Eli's edit" });
  r = await send(w, E!, cid, keep(v1qids, stemsOf(4, "v3")), { bid, base: 2 });
  const c1 = publishId();
  const denied = await rollback(w, E!, cid, bid, { to_version: 1, base_version: 3 });
  assert.deepEqual([denied.status, denied.body.error, denied.body.needed], [403, "circle_role_required", "owner"], "an editor who is not the author cannot (D17)");
  const done = await rollback(w, A!, cid, bid, { to_version: 1, base_version: 3, commit_id: c1, message: "back to the original" });
  assert.deepEqual([done.status, done.body.version, done.body.rollback_of, done.body.changed, done.body.added, done.body.removed, done.body.held], [200, 4, 1, 4, 0, 0, false], JSON.stringify(done.body));
  assert.deepEqual(done.body.qids, v1qids, "the same questions, the same qids");
  const head = await readStems(w, E!, `/circles/${cid}/banks/${bid}`);
  assert.deepEqual([head.stems, head.body.bank.rollback_of, head.body.bank.version], [stemsOf(4, "v1"), 1, 4], "the head is version 1's body");
  const replay = await rollback(w, A!, cid, bid, { to_version: 1, base_version: 3, commit_id: c1, message: "back to the original" });
  assert.deepEqual([replay.status, replay.body.replayed, replay.body.version_id], [200, true, done.body.version_id]);
  assert.equal((await rollback(w, A!, cid, bid, { to_version: 2, base_version: 3, commit_id: c1 })).body.error, "commit_id_conflict");
  const stale = await rollback(w, O!, cid, bid, { to_version: 2, base_version: 3 });
  assert.deepEqual([stale.status, stale.body.error, stale.body.head_version, stale.body.head_by], [409, "conflict", 4, "Ann"], "a stale base is a conflict");
  assert.equal((await rollback(w, O!, cid, bid, { to_version: 4, base_version: 4 })).status, 400, "not to the head itself");
  assert.deepEqual((await rollback(w, O!, cid, bid, { to_version: 2, base_version: 4, message: "x".repeat(501) })).body.field, "message", "500 characters at most, as a send's");
  assert.equal((await rollback(w, O!, cid, bid, { to_version: 2, base_version: 4 })).status, 200);
  const hist = (await history(w, E!, cid, bid)).body.versions;
  assert.deepEqual([hist[1].rollback_of, hist[1].message, hist[1].by, hist[0].rollback_of, hist[0].by], [1, "back to the original", "Ann", 2, "Owner"]);
  const act = (await w.json<any>("GET", `/circles/${cid}/activity`, { user: E! })).body.activity;
  assert.deepEqual([act[1].what, act[1].version, act[1].rollback_of, act[1].by], ["rolled_back", 4, 1, "Ann"]);
  // A large drop needs the confirm: version 6 has 10 questions; rolling back to version 4 (4 questions) drops 6.
  r = await send(w, A!, cid, keep(v1qids, stemsOf(10, "v6")), { bid, base: 5 });
  assert.equal(r.status, 200, JSON.stringify(r.body));
  const big = await rollback(w, O!, cid, bid, { to_version: 4, base_version: 6 });
  assert.deepEqual([big.status, big.body.error, big.body.removed, big.body.total], [428, "confirm_large_required", 6, 10]);
  assert.equal((await rollback(w, O!, cid, bid, { to_version: 4, base_version: 6, confirm_large_change: true })).status, 200);
  // Never a pruned version.
  for (let v = 8; v <= 28; v++) r = await send(w, A!, cid, [`v${v} about sandbars?`], { bid, base: v - 1, confirm: true });
  const pr = await rollback(w, O!, cid, bid, { to_version: 2, base_version: 28 });
  assert.deepEqual([pr.status, pr.body.error, pr.body.oldest_kept], [404, "version_not_retained", 9]);
  // 20 a day (D10), counted per account; refusals are given back.
  w.advance(DAY);
  let hv = 28;
  for (let i = 0; i < 20; i++) {
    const x = await rollback(w, O!, cid, bid, { to_version: hv - 1, base_version: hv, confirm_large_change: true });
    assert.equal(x.status, 200, JSON.stringify(x.body));
    hv = x.body.version;
  }
  const cap = await rollback(w, O!, cid, bid, { to_version: hv - 1, base_version: hv, confirm_large_change: true });
  assert.deepEqual([cap.status, cap.body.error, cap.body.metric], [429, "daily_limit", "circle_rollbacks"]);
  assert.equal((await rollback(w, A!, cid, bid, { to_version: hv - 1, base_version: hv, confirm_large_change: true })).status, 200, "another account's own 20");
  // A rollback is not a send: the owner still sends today.
  assert.equal((await send(w, O!, cid, ["The owner's own version?"], { bid, base: hv + 1, confirm: true })).status, 200);
  // A demoted author can no longer roll back (D17: the author while an editor).
  assert.equal((await w.json("POST", `/circles/${cid}/members/role`, { user: O!, json: { user_id: A!.id, role: "viewer" } })).status, 200);
  assert.equal((await rollback(w, A!, cid, bid, { to_version: hv, base_version: hv + 2 })).body.needed, "owner");
});

// ---- 7. the confirmed-student-data check on sends ---------------------------------------------------------------------

test("37-7: a send or rollback whose text matches (or rewords) confirmed student data holds the bank at commit (never a refusal: it would confirm the block) and flags the admin (ids only); a later version without it lifts that hold; the admin may open it on the privacy basis", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [A, E, P] = ["Ann", "Eli", "Pub"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"]]);
  const STUDENT = "Jenny Smith in period 3 scored 42 on the unit test: what is the class median?";
  // The admin confirms the text as student data (a privacy purge of a public item holding it).
  const it = (await publish(w, P!, bankOf([q(STUDENT)]))).body as Q;
  assert.equal((await w.purge(adm, it.item_id, { questions: [0] })).status, 200);
  const s = await send(w, A!, cid, ["A clean question about deltas and fans?"]);
  const bid = s.body.bank_id;
  w.notices.length = 0;
  // An editor pastes it (with a changed case and a full stop: the normalised keys still match).
  const dirty = await send(w, E!, cid, keep(s.body.qids, ["A clean question about deltas and fans?", STUDENT.toUpperCase() + "."]), { bid, base: 1 });
  assert.deepEqual([dirty.status, dirty.body.held], [200, true], JSON.stringify(dirty.body));
  assert.deepEqual(w.q(`SELECT reason, source FROM circle_holds WHERE bank_id = ?`, bid), [{ reason: "student_data", source: "privacy" }]);
  assert.equal((await w.json("GET", `/circles/${cid}/banks/${bid}`, { user: E! })).status, 404, "hidden from members but its author");
  // (round 38: the version itself is withheld from everyone, its author too; round38.test.ts)
  assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: A! })).body.error, "version_withheld");
  const flags = w.q<any>(`SELECT detail, publish_id FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ?`, bid);
  assert.equal(flags.length, 1);
  assert.deepEqual(JSON.parse(flags[0].detail).positions, { count: 1, first: [1] });
  assert.ok(!flags[0].detail.toLowerCase().includes("jenny"), "ids and positions only");
  assert.deepEqual(w.notices.filter((n) => n.kind === "privacy_text_match").map((n: any) => [n.itemId, n.circleId]), [[bid, cid]]);
  // The admin may open it on the privacy basis (the send's own match is the basis).
  assert.equal((await w.admin(adm, "GET", "/admin/pool/circle-bank", null, `?id=${bid}&basis=privacy`)).status, 200);
  // The author removes it in a new version: the privacy hold goes (an admin's own hold would stay).
  const clean = await send(w, A!, cid, keep(dirty.body.qids.slice(0, 1), ["A clean question about deltas and fans?"]), { bid, base: 2 });
  assert.deepEqual([clean.status, clean.body.held], [200, false]);
  assert.equal(w.q(`SELECT 1 FROM circle_holds WHERE bank_id = ?`, bid).length, 0);
  // A rollback to the version holding it is refused (round 38: it stays withheld until an admin releases it).
  const rb = await rollback(w, A!, cid, bid, { to_version: 2, base_version: 3 });
  assert.deepEqual([rb.status, rb.body.error], [404, "version_withheld"]);
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ?`, bid).length, 1);
  // A near-rewording (one word changed) is held too, as at a public publish (round 13).
  const b2 = await send(w, A!, cid, [STUDENT.replace("median", "middle value")]);
  assert.deepEqual([b2.status, b2.body.held], [201, true], JSON.stringify(b2.body));
  assert.deepEqual(w.q(`SELECT source FROM circle_holds WHERE bank_id = ?`, b2.body.bank_id), [{ source: "privacy_near" }]);
  // The admin's release of the version resolves its flag (round 38: a bank-wide release leaves a withheld version's flag open).
  assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, note: "A false positive, checked." })).status, 200);
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ? AND resolved_at IS NULL`, bid).length, 1);
  assert.equal((await w.admin(adm, "POST", "/admin/pool/circle-release", { bank_id: bid, version: 2, note: "A false positive, checked." })).status, 200);
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ? AND resolved_at IS NULL`, bid).length, 0);
});

// ---- 8. tombstones ------------------------------------------------------------------------------------------------------

test("37-8: a purged bank's tombstone keeps no text (title, subject, course, grade blank: the columns are NOT NULL), no versions, no R2 objects; no audit or flag row carries its text", async () => {
  const w = new World();
  const [A] = ["Ann"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, []);
  const s = await send(w, A!, cid, stemsOf(2, "Ann"), { title: "Secret roster title" });
  const bid = s.body.bank_id;
  assert.equal((await send(w, A!, cid, keep(s.body.qids, stemsOf(2, "Bob")), { bid, base: 1, title: "Secret roster title 2", message: "Secret message" })).status, 200);
  w.sql.prepare(`UPDATE circle_banks SET subject = 'Secret subject', course = 'Secret course', grade = 'Secret grade' WHERE id = ?`).run(bid);
  assert.equal((await w.json("DELETE", `/circles/${cid}/banks/${bid}`, { user: A! })).status, 200);
  assert.equal((await w.json("POST", `/circles/${cid}/banks/${bid}/purge`, { user: A! })).status, 200);
  const t = w.q<any>(`SELECT title, subject, course, grade, purged_at, head_version_id FROM circle_banks WHERE id = ?`, bid)[0];
  assert.deepEqual([t.title, t.subject, t.course, t.grade, t.purged_at !== null, t.head_version_id], ["", "", "", "", true, null]);
  for (const { name } of w.q<{ name: string }>(`SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'`))
    for (const row of w.q(`SELECT * FROM "${name}"`)) assert.ok(!JSON.stringify(row).includes("Secret"), `${name}: ${JSON.stringify(row).slice(0, 160)}`);
  assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE bank_id = ?`, bid).length, 0);
  assert.deepEqual(w.r2.keys(`circles/${cid}/${bid}/`), []);
  // Gone for members and in the update check.
  assert.equal((await history(w, A!, cid, bid)).status, 404);
  assert.equal((await w.json<any>("POST", "/circles/heads", { user: A!, json: { bank_ids: [bid] } })).body.banks[0].status, "unavailable");
});

// ---- 9. the version graph against a model ---------------------------------------------------------------------------------

interface MV {
  by: string;
  stems: string[];
  qids: string[];
  message: string;
  rollback_of?: number;
  merged_from?: number;
}

test("37-9: oracle: random sends, stale sends, rollbacks, merges, prunes and an erasure against a model of the version graph: versions linear and gap-free, every accepted body exactly what was sent (or the version a rollback names), every refused send 409 with the true head, rights per D17, the last 20 kept (D16)", async () => {
  for (const seed of [37, 3701]) {
    const r = rng(seed).next;
    const w = new World();
    const names = ["Ola", "Ann", "Eli", "Fay", "Vic", "Zed"];
    const [O, A, E, F, V, Z] = names.map((n) => w.user("teacher", n)) as User[];
    const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"], [F!, "editor"], [V!, "viewer"], [Z!, "editor"]]);
    const role = new Map<User, string>([[O!, "owner"], [A!, "editor"], [E!, "editor"], [F!, "editor"], [V!, "viewer"], [Z!, "editor"]]);
    const nameOf = new Map<User, string>([[O!, "Ola"], [A!, "Ann"], [E!, "Eli"], [F!, "Fay"], [V!, "Vic"], [Z!, "Zed"]]);
    let erased = false;
    let stemN = 0;
    const fresh = () => `S${seed}-${stemN++}: what is the gradient of the line through point ${stemN}?`;
    const first = [fresh(), fresh(), fresh()];
    const s0 = await send(w, A!, cid, first, { message: "m1" });
    assert.equal(s0.status, 201);
    const bid = s0.body.bank_id;
    const model: MV[] = [{ by: "Ann", stems: first, qids: s0.body.qids, message: "m1" }];
    const head = () => model.length;
    const who = (u: User) => (erased && u === Z ? "deleted teacher" : nameOf.get(u)!);
    const pick = <T,>(xs: T[]) => xs[Math.floor(r() * xs.length)]!;
    const actors = [O!, A!, E!, F!, V!, Z!];
    const stemsIn = (items: string[]) => items.map((x) => (x.startsWith("qid:") ? x.split(":").slice(2).join(":") : x));
    const steps = 110;
    for (let step = 0; step < steps; step++) {
      const label = `seed ${seed} step ${step}`;
      const op = r();
      const u = pick(actors);
      if (step === 70 && !erased) {
        await erase(w, Z!);
        erased = true;
        model.forEach((v) => {
          if (v.by === "Zed") ((v.by = "deleted teacher"), (v.message = ""));
        });
      } else if (op < 0.5) {
        // A send: usually from the head, sometimes from an older base.
        const base = r() < 0.75 ? head() : Math.max(1, head() - 1 - Math.floor(r() * 3));
        const from = model[base - 1]!;
        const items: string[] = [];
        from.stems.forEach((st, i) => {
          const x = r();
          if (x < 0.7) items.push(`qid:${from.qids[i]}:${st}`);
          else if (x < 0.85) items.push(`qid:${from.qids[i]}:${fresh()}`);
        });
        for (let k = Math.floor(r() * 3); k > 0; k--) items.push(fresh());
        if (items.length === 0) items.push(fresh());
        const confirm = r() < 0.5;
        const msg = r() < 0.5 ? `msg ${step}` : "";
        const res = await send(w, u, cid, items, { bid, base, confirm, message: msg });
        const headV = model[head() - 1]!;
        const keptQ = new Set(items.filter((x) => x.startsWith("qid:")).map((x) => x.split(":")[1]));
        const removed = headV.qids.filter((x) => !keptQ.has(x)).length;
        if (erased && u === Z) assert.equal(res.status, 401, label);
        else if (role.get(u) === "viewer") assert.equal(res.body.error, "circle_role_required", label);
        else if (base !== head()) assert.deepEqual([res.status, res.body.error, res.body.head_version, res.body.head_by], [409, "conflict", head(), headV.by], label);
        else if (!confirm && removed * 2 > headV.qids.length) assert.deepEqual([res.status, res.body.removed, res.body.total], [428, removed, headV.qids.length], label);
        else {
          assert.deepEqual([res.status, res.body.version], [200, head() + 1], `${label}: ${JSON.stringify(res.body)}`);
          model.push({ by: who(u), stems: stemsIn(items), qids: res.body.qids, message: msg });
        }
      } else if (op < 0.75 && head() >= 2) {
        // A rollback.
        const base = r() < 0.8 ? head() : Math.max(2, head() - 1);
        const to = 1 + Math.floor(r() * (base - 1));
        const confirm = r() < 0.6;
        const res = await rollback(w, u, cid, bid, { to_version: to, base_version: base, confirm_large_change: confirm, message: `rb ${step}` });
        const may = role.get(u) === "owner" || (u === A && role.get(u) === "editor");
        const headV = model[head() - 1]!;
        const target = model[to - 1]!;
        const removed = headV.qids.filter((x) => !target.qids.includes(x)).length;
        if (erased && u === Z) assert.equal(res.status, 401, label);
        else if (!may) assert.deepEqual([res.status, res.body.needed], [403, "owner"], label);
        else if (base !== head()) assert.deepEqual([res.status, res.body.head_version], [409, head()], label);
        else if (to <= head() - 20) assert.deepEqual([res.status, res.body.error, res.body.oldest_kept], [404, "version_not_retained", head() - 19], label);
        else if (!confirm && removed * 2 > headV.qids.length) assert.deepEqual([res.status, res.body.removed, res.body.total], [428, removed, headV.qids.length], label);
        else {
          assert.deepEqual([res.status, res.body.version, res.body.rollback_of, res.body.qids], [200, head() + 1, to, target.qids], `${label}: ${JSON.stringify(res.body)}`);
          model.push({ by: who(u), stems: target.stems, qids: target.qids, message: `rb ${step}`, rollback_of: to });
        }
      } else if (op < 0.88 && head() >= 2) {
        // A merge: from an older base, then a send based on the head with the token.
        const base = Math.max(1, head() - 1 - Math.floor(r() * 25));
        const mg = await merge(w, u, cid, bid, base);
        if (erased && u === Z) assert.equal(mg.status, 401, label);
        else if (role.get(u) === "viewer") assert.equal(mg.status, 403, label);
        else {
          assert.deepEqual(
            [mg.status, mg.body.head_version, mg.body.base_kept, mg.body.between_total, mg.body.between.map((x: Q) => [x.version, x.by])],
            [200, head(), base > head() - 20, head() - base, model.slice(base).map((v, k) => [base + 1 + k, v.by]).slice(0, 50)],
            label,
          );
          const headV = model[head() - 1]!;
          const items = [...headV.stems.map((st, i) => `qid:${headV.qids[i]}:${st}`), fresh()];
          const res = await send(w, u, cid, items, { bid, base: head(), mergeToken: mg.body.merge_token, message: `merge ${step}` });
          assert.deepEqual([res.status, res.body.merged_from], [200, base], `${label}: ${JSON.stringify(res.body)}`);
          model.push({ by: who(u), stems: stemsIn(items), qids: res.body.qids, message: `merge ${step}`, merged_from: base });
        }
      } else {
        w.advance(DAY);
        await w.pool.hourly();
        await w.pool.daily();
        // After the daily prune, R2 holds exactly the kept versions' bodies and indexes.
        assert.equal(w.r2.keys(`circles/${cid}/${bid}/`).length, 2 * Math.min(20, head()), label);
      }
      // The history matches the model (every page); the head and a kept version are what was sent; a pruned one is not retained.
      if (step % 5 === 4 || step === steps - 1) {
        const lines: Q[] = [];
        let before: number | undefined;
        do {
          const h = await history(w, V!, cid, bid, before);
          assert.equal(h.status, 200, label);
          lines.push(...h.body.versions);
          before = h.body.next_before ?? undefined;
        } while (before);
        assert.deepEqual(
          lines.map((x) => [x.version, x.by, x.message, x.rollback_of ?? null, x.merged_from ?? null, x.kept]),
          model.map((v, k) => [k + 1, v.by, v.message, v.rollback_of ?? null, v.merged_from ?? null, k + 1 > head() - 20]).reverse(),
          label,
        );
        assert.deepEqual((await readStems(w, V!, `/circles/${cid}/banks/${bid}`)).stems, model[head() - 1]!.stems, label);
        const kv = Math.max(1, head() - Math.floor(r() * 20));
        const kb = await readStems(w, V!, `/circles/${cid}/banks/${bid}/versions/${kv}`);
        assert.deepEqual([kb.status, kb.stems, kb.body.payload.bank.questions.map((x: Q) => x.id)], [200, model[kv - 1]!.stems, model[kv - 1]!.qids], label);
        if (head() > 20) assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/${head() - 20}`, { user: V! })).body.oldest_kept, head() - 19, label);
      }
    }
    assert.ok(head() > 25, `seed ${seed}: ${head()} versions`);
    assert.ok(model.some((v) => v.rollback_of) && model.some((v) => v.merged_from), `seed ${seed}: rollbacks and merges happened`);
    assert.equal(w.q(`SELECT 1 FROM circle_versions WHERE status = 'staging'`).length, 0);
  }
});

// ---- 10. the sizes ---------------------------------------------------------------------------------------------------------

test("37-10: a 1,000-question bank of about 3.8 MB is sent by an editor, read, its earlier version read, merged and rolled back within the isolate's budget (nothing left reserved)", async () => {
  const w = new World({ heavyQueueMs: 600_000 });
  const [A, E] = ["Ann", "Eli"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"]]);
  const long = (tag: string, i: number) => `${tag} ${i}: ${"which statement about the braided river's sediment budget is best supported by the survey ".repeat(40)}?`;
  const big = (tag: string) => Array.from({ length: 1000 }, (_, i) => long(tag, i));
  const s = await send(w, A!, cid, big("v1"));
  assert.equal(s.status, 201, JSON.stringify(s.body).slice(0, 300));
  const bytes = w.q<any>(`SELECT bytes FROM circle_versions WHERE bank_id = ?`, s.body.bank_id)[0].bytes;
  assert.ok(bytes > 3_500_000 && bytes < 4_400_000, `${bytes} bytes`);
  const bid = s.body.bank_id;
  const s2 = await send(w, E!, cid, keep(s.body.qids, big("v2")), { bid, base: 1 });
  assert.equal(s2.status, 200, JSON.stringify(s2.body).slice(0, 300));
  assert.equal((await readStems(w, E!, `/circles/${cid}/banks/${bid}`)).stems.length, 1000);
  assert.equal((await readStems(w, E!, `/circles/${cid}/banks/${bid}/versions/1`)).stems[999], long("v1", 999));
  assert.equal((await merge(w, E!, cid, bid, 1)).status, 200);
  const rb = await rollback(w, A!, cid, bid, { to_version: 1, base_version: 2 });
  assert.deepEqual([rb.status, rb.body.changed], [200, 1000], JSON.stringify(rb.body).slice(0, 300));
  assert.equal((await readStems(w, E!, `/circles/${cid}/banks/${bid}`)).stems[999], long("v1", 999));
  assert.equal(bytesReserved(), 0, "every reservation released");
});

// ---- 11. erasure races ---------------------------------------------------------------------------------------------------

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
  // Before any cron: the request's own writes respect the erasure (the guard IN the write).
  const immediate = scanIds(w, [Z.id]);
  const staging = w.q(`SELECT 1 FROM circle_versions WHERE status = 'staging'`).length;
  const locks = w.q(`SELECT 1 FROM pool_locks`).length;
  for (const step of [3600_000, DAY, 15 * DAY, DAY]) {
    w.advance(step);
    await w.pool.hourly();
    await w.pool.daily();
  }
  return { w, Z, status, boundaries, check, immediate, staging, locks };
}
async function race(sc: RaceScenario) {
  const ref = await raceRun(sc, "ref");
  const ctl = await raceRun(sc, "control");
  assert.equal(ctl.status, 401, `${sc.name}: a request after the erasure is refused like an unauthenticated one`);
  const refRows = scanIds(ref.w, [ref.Z.id]);
  const ctlRows = scanIds(ctl.w, [ctl.Z.id]);
  for (const x of [ref, ctl]) {
    assert.deepEqual(scanIds(x.w, [ZNAME, "zq-private"]), {}, `${sc.name}: PII left`);
    x.check?.(x.w);
  }
  assert.ok(ref.boundaries >= 2, `${sc.name}: ${ref.boundaries} boundaries`);
  let committed = 0;
  for (let k = 0; k < ref.boundaries; k++) {
    const x = await raceRun(sc, k);
    if (x.status === 200) committed++;
    for (const [t, n] of Object.entries(x.immediate)) {
      const cap = Math.max(ref.immediate[t] ?? 0, ctl.immediate[t] ?? 0);
      assert.ok(n <= cap && cap > 0, `${sc.name}: erased before boundary ${k}/${ref.boundaries} (status ${x.status}): before the cron, ${t} holds ${n} rows of the erased account (a normal erasure leaves ${cap})`);
    }
    assert.equal(x.staging, 0, `${sc.name}: boundary ${k}: a staging row left before the cron`);
    assert.equal(x.locks, 0, `${sc.name}: boundary ${k}: an account lock left`);
    for (const [t, n] of Object.entries(scanIds(x.w, [x.Z.id]))) {
      const cap = Math.max(refRows[t] ?? 0, ctlRows[t] ?? 0);
      assert.ok(n <= cap && cap > 0, `${sc.name}: erased before boundary ${k}/${ref.boundaries} (status ${x.status}): residue in ${t} (${n} rows, a normal erasure leaves ${cap})`);
    }
    assert.deepEqual(scanIds(x.w, [ZNAME, "zq-private"]), {}, `${sc.name}: boundary ${k}: PII left`);
    assert.ok(x.w.r2.keys("circles/").length <= Math.max(ref.w.r2.keys("circles/").length, ctl.w.r2.keys("circles/").length), `${sc.name}: boundary ${k}: R2 objects left`);
    x.check?.(x.w);
  }
  assert.ok(committed >= 1 && committed < ref.boundaries, `${sc.name}: ${committed} of ${ref.boundaries} interleavings committed`);
}
async function raceWorld() {
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), Z = w.user("teacher", ZNAME);
  w.sql.prepare(`UPDATE users SET created_at = ? WHERE id IN (?, ?, ?)`).run(w.clock - 30 * DAY, A.id, B.id, Z.id);
  return { w, A, B, Z };
}
/** The bank's versions are linear and gap-free; the head row is the current version. */
const linear = (bid: string) => (w: World) => {
  const vs = w.q<any>(`SELECT version, status FROM circle_versions WHERE bank_id = ? AND status <> 'staging' ORDER BY version`, bid);
  assert.deepEqual(vs.map((v) => v.version), vs.map((_, i) => i + 1));
  const b = w.q<any>(`SELECT head_version FROM circle_banks WHERE id = ?`, bid)[0];
  assert.equal(b.head_version, vs.length);
};

const RACES: RaceScenario[] = [
  {
    name: "37 race PUT (Z, an editor, sends version 2 of Ann's bank)",
    async build() {
      const { w, A, Z } = await raceWorld();
      const cid = await circleWith(w, A, [[Z, "editor"]]);
      const s = await send(w, A, cid, ["Ann's question about sinkholes?"]);
      return { w, Z, req: () => send(w, Z, cid, keep(s.body.qids, ["zq-private v2 about sinkholes?"]), { bid: s.body.bank_id, base: 1, message: "zq-private v2" }), check: linear(s.body.bank_id) };
    },
  },
  {
    name: "37 race PUT with a merge token (Z merges and sends)",
    async build() {
      const { w, A, Z } = await raceWorld();
      const cid = await circleWith(w, A, [[Z, "editor"]]);
      const s = await send(w, A, cid, ["Ann's question about karst?"]);
      const s2 = await send(w, A, cid, keep(s.body.qids, ["Ann's question about karst, edited?"]), { bid: s.body.bank_id, base: 1 });
      const mg = await merge(w, Z, cid, s.body.bank_id, 1);
      return { w, Z, req: () => send(w, Z, cid, keep(s2.body.qids, ["zq-private merged karst?"]), { bid: s.body.bank_id, base: 2, mergeToken: mg.body.merge_token, message: "zq-private" }), check: linear(s.body.bank_id) };
    },
  },
  {
    name: "37 race rollback (Z, the author)",
    async build() {
      const { w, A, Z } = await raceWorld();
      const cid = await circleWith(w, A, [[Z, "editor"]]);
      const s = await send(w, Z, cid, ["Z's question about caves?"]);
      await send(w, A, cid, keep(s.body.qids, ["Ann's edit about caves?"]), { bid: s.body.bank_id, base: 1 });
      return { w, Z, req: () => rollback(w, Z, cid, s.body.bank_id, { to_version: 1, base_version: 2, message: "zq-private rollback" }), check: linear(s.body.bank_id) };
    },
  },
  {
    name: "37 race rollback (Z, the owner)",
    async build() {
      const { w, A, B, Z } = await raceWorld();
      const cid = await circleWith(w, Z, [[A, "editor"], [B, "viewer"]]);
      const s = await send(w, A, cid, ["Ann's question about dunes?"]);
      await send(w, A, cid, keep(s.body.qids, ["Ann's edit about dunes?"]), { bid: s.body.bank_id, base: 1 });
      return { w, Z, req: () => rollback(w, Z, cid, s.body.bank_id, { to_version: 1, base_version: 2, message: "zq-private" }), check: linear(s.body.bank_id) };
    },
  },
];
for (const sc of RACES)
  test(`37-11 ${sc.name}: erased before every statement boundary, nothing but a normal erasure's residue is left (no staging row, no lock, no PII), versions linear`, async () => {
    await race(sc);
  });

// ---- 12. busy shedding ------------------------------------------------------------------------------------------------------

test("37-12: concurrent sends from several accounts in one isolate: one heavy slot, four wait, the rest are shed with 503 busy (Retry-After; the day's send given back; nothing staged, nothing locked, nothing reserved); on one bank exactly one of them wins", async () => {
  const w = new World();
  const O = w.user("teacher", "Ola");
  const eds = Array.from({ length: 7 }, (_, i) => w.user("teacher", `Ed${i}`));
  const cid = await circleWith(w, O, eds.map((e) => [e, "editor"] as [User, "editor"]));
  const meta = (u: User) => ({ commit_id: publishId(), license: "circle-only", attest: { own_work: true, terms_version: POOL_TERMS_VERSION }, meta: { title: `By ${u.name}`, subject: "", course: "", grade: "", tags: [], standards: [] } });
  const res = await Promise.all(eds.map((e, i) => w.call("POST", `/circles/${cid}/banks`, { user: e, body: framed(meta(e), bankOf(stemsOf(40, `E${i}`).map((s) => q(s)))), contentType: PUB })));
  const st = res.map((x) => x.status);
  const shed = res.filter((x) => x.status === 503);
  assert.ok(shed.length >= 1 && st.filter((s) => s === 201).length >= 1 && shed.length + st.filter((s) => s === 201).length === 7, st.join(","));
  for (const x of shed) {
    assert.equal(((await x.json()) as Q).error, "busy");
    assert.ok(Number(x.headers.get("Retry-After")) > 0);
  }
  const shedUsers = eds.filter((_, i) => st[i] === 503);
  for (const u of shedUsers) assert.equal(w.q<any>(`SELECT COALESCE(SUM(count), 0) AS n FROM usage_daily WHERE user_id = ? AND metric = 'circle_commits'`, u.id)[0].n, 0, "given back");
  assert.equal(w.q(`SELECT 1 FROM circle_banks`).length, 7 - shed.length);
  assert.deepEqual([w.q(`SELECT 1 FROM circle_versions WHERE status = 'staging'`).length, w.q(`SELECT 1 FROM pool_locks`).length, bytesReserved()], [0, 0, 0]);
  // The slot is free again: a shed account's retry goes through.
  assert.equal((await send(w, shedUsers[0]!, cid, stemsOf(2, "retry"))).status, 201);
  // Five editors send version 2 of one bank at once: exactly one wins; the others are told the head (409) or shed (503).
  const b = await send(w, O, cid, stemsOf(3, "Ola"));
  const res2 = await Promise.all(eds.slice(0, 5).map((e, i) => send(w, e, cid, keep(b.body.qids, stemsOf(3, `Ed${i}`)), { bid: b.body.bank_id, base: 1 })));
  const st2 = res2.map((x) => x.status);
  assert.equal(st2.filter((s) => s === 200).length, 1, st2.join(","));
  assert.ok(st2.every((s) => s === 200 || s === 409 || s === 503), st2.join(","));
  for (const x of res2.filter((x) => x.status === 409)) assert.deepEqual([x.body.error, x.body.head_version], ["conflict", 2]);
  assert.deepEqual(w.q<any>(`SELECT version FROM circle_versions WHERE bank_id = ? ORDER BY version`, b.body.bank_id).map((x) => x.version), [1, 2]);
  assert.deepEqual([w.q(`SELECT 1 FROM circle_versions WHERE status = 'staging'`).length, w.q(`SELECT 1 FROM pool_locks`).length, bytesReserved()], [0, 0, 0]);
});

// ---- 13. D1 cost at the caps -------------------------------------------------------------------------------------------------

interface Cap {
  sql: string;
  args: unknown[];
}
function capture(w: World, is: (sql: string) => boolean): Cap[] {
  const out: Cap[] = [];
  w.stats.log = (sql, args) => {
    if (is(sql)) out.push({ sql, args: [...args] });
  };
  return out;
}
const planOf = (w: World, c: Cap) => (w.sql.prepare(`EXPLAIN QUERY PLAN ${c.sql}`).all(...(c.args as never[])) as { detail: string }[]).map((x) => x.detail);
const tableScans = (w: World, c: Cap) => planOf(w, c).filter((d) => /^SCAN /.test(d) && !/^SCAN (CONSTANT ROW|h|my|x|j|\(subquery-\d+\))(\s|$)/.test(d) && !/VIRTUAL TABLE/.test(d));
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

/** A circle at the caps built by SQL: 99 banks with 20 kept versions each, and one bank with `nv` versions (all but 20 pruned). */
async function bigHistory(nv: number) {
  const w = new World();
  const O = w.user("teacher", "Ola"), E = w.user("teacher", "Eli");
  const cid = await circleWith(w, O, [[E, "editor"]]);
  const s = await send(w, O, cid, ["The bank whose history is long?"]);
  const bid = s.body.bank_id;
  const v1 = w.q<any>(`SELECT * FROM circle_versions WHERE bank_id = ?`, bid)[0];
  const bodyText = await (await w.r2.get(v1.full_key))!.text(), idxText = await (await w.r2.get(v1.index_key))!.text();
  const hex = (n: number, tag: number) => (tag.toString(16).padStart(4, "0") + n.toString(16).padStart(28, "0")).slice(0, 32);
  const ins = w.sql.prepare(
    `INSERT INTO circle_versions (id, bank_id, circle_id, version, commit_id, request_sha256, result_json, committer_id, base_version, message, license, status, full_key, index_key, bytes, question_count, added, changed, removed, seq, created_at, committed_at)
     VALUES (?, ?, ?, ?, ?, 'x', '{}', ?, ?, ?, 'circle-only', ?, ?, ?, 1000, 1, 0, 1, 0, 1, 1, ?)`,
  );
  const puts: [string, string][] = [];
  w.sql.exec("BEGIN");
  w.sql.prepare(`UPDATE circle_versions SET status = 'pruned', full_key = '', index_key = '' WHERE id = ?`).run(v1.id);
  let headId = v1.id;
  for (let v = 2; v <= nv; v++) {
    const id = hex(v, 7);
    const kept = v > nv - 20;
    const fk = kept ? `circles/${cid}/${bid}/${id}.json` : "", ik = kept ? `circles/${cid}/${bid}/${id}.idx.json` : "";
    ins.run(id, bid, cid, v, `commit-${id}`, v % 2 ? O.id : E.id, v - 1, `message ${v}`, v === nv ? "current" : kept ? "previous" : "pruned", fk, ik, v * 1000);
    if (kept) puts.push([fk, bodyText], [ik, idxText]);
    headId = id;
  }
  w.sql.prepare(`UPDATE circle_banks SET head_version = ?, head_version_id = ? WHERE id = ?`).run(nv, headId, bid);
  // 99 more banks with 20 kept versions each (the circle's quota sum's worst case).
  const insB = w.sql.prepare(`INSERT INTO circle_banks (id, circle_id, author_id, title, license, head_version, head_version_id, question_count, created_at, updated_at) VALUES (?, ?, ?, 'b', 'circle-only', 20, ?, 10, 1, 1)`);
  for (let b = 0; b < 99; b++) {
    const ob = hex(b, 8);
    for (let v = 1; v <= 20; v++) ins.run(hex(b * 100 + v, 9), ob, cid, v, `c-${ob}-${v}`, O.id, v - 1, "m", v === 20 ? "current" : "previous", "k", "k", v * 1000);
    insB.run(ob, cid, O.id, hex(b * 100 + 20, 9));
  }
  w.sql.exec("COMMIT");
  for (const [k, v] of puts) await w.r2.put(k, v);
  return { w, O, E, cid, bid };
}

test("37-13: history, head, kept-version and merge-initiate reads at the caps (a bank with 1,000 versions in a circle of 100 banks x 20 kept versions): every statement by index (no table scan), each page bounded (50 lines), time flat in the history's length", async () => {
  const run = async (nv: number) => {
    const { w, E, cid, bid } = await bigHistory(nv);
    const caps = capture(w, (sql) => /circle_versions|circle_banks|circle_holds/.test(sql) && /^\s*(SELECT|WITH)/i.test(sql));
    const h1 = await history(w, E, cid, bid);
    assert.deepEqual([h1.status, h1.body.versions.length, h1.body.oldest_kept, h1.body.versions[0].version], [200, 50, nv - 19, nv]);
    const h2 = await history(w, E, cid, bid, Math.floor(nv / 2));
    assert.equal(h2.body.versions[0].version, Math.floor(nv / 2) - 1);
    assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: E })).status, 200);
    assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/${nv - 5}`, { user: E })).status, 200);
    assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/${nv - 25}`, { user: E })).body.error, "version_not_retained");
    const m1 = await merge(w, E, cid, bid, 1);
    assert.deepEqual([m1.status, m1.body.between.length, m1.body.between_total, m1.body.base_kept], [200, 50, nv - 1, false]);
    const m2 = await merge(w, E, cid, bid, nv - 3);
    assert.deepEqual([m2.body.between.length, m2.body.base_kept], [3, true]);
    w.stats.log = undefined;
    const pick = (re: RegExp) => caps.filter((c) => re.test(c.sql));
    return {
      w,
      caps,
      lines: pick(/v\.version < \?2 ORDER BY v\.version DESC/),
      between: pick(/v\.version > \?2 AND v\.version <= \?3/),
      oldest: pick(/SELECT version FROM circle_versions INDEXED BY circle_versions_bank/),
      kept: pick(/v\.bank_id = \?2 AND v\.version = \?3/),
    };
  };
  const small = await run(60), big = await run(1000);
  for (const c of big.caps) assert.deepEqual(tableScans(big.w, c), [], c.sql.replace(/\s+/g, " ").slice(0, 160));
  for (const [k, re] of [
    ["lines", /SEARCH v USING INDEX sqlite_autoindex_circle_versions_\d \(bank_id=\? AND version<\?\)/],
    ["between", /SEARCH v USING INDEX sqlite_autoindex_circle_versions_\d \(bank_id=\? AND version>\? AND version<\?\)/],
    ["oldest", /SEARCH circle_versions USING INDEX circle_versions_bank \(bank_id=\? AND status=\?\)/],
    ["kept", /SEARCH v USING INDEX sqlite_autoindex_circle_versions_\d \(bank_id=\? AND version=\?\)/],
  ] as const) {
    assert.ok(big[k].length >= 1, k);
    for (const c of big[k]) assert.ok(planOf(big.w, c).some((d) => re.test(d)), `${k}: ${planOf(big.w, c).join(" | ")}`);
  }
  // Flat in the history's length (60 -> 1,000 versions; a page reads 51 lines, the oldest-kept lookup the kept ones).
  const report: string[] = [];
  for (const k of ["lines", "between", "oldest", "kept"] as const)
    for (let i = 0; i < big[k].length; i++) {
      const [ts, tb] = [timeOf(small.w, small[k][i]!), timeOf(big.w, big[k][i]!)];
      report.push(`${k}[${i}]: ${ts.toFixed(3)} -> ${tb.toFixed(3)} ms`);
      assert.ok(tb < Math.max(LIM * ts, 5), report.join("; "));
    }
  if (process.env.POOL_ROUND37_REPORT === "1") console.log(report.join("\n"));
});

// ---- 14. public material across editors' versions ------------------------------------------------------------------------------

test("37-14: a public question's recorded origin stays with it through an editor's version (whose copy carries no origin fields) and a rollback, so the licence stays bound (D19, plan 7); its author's name is read at each read, never stored (an erased public author is 'deleted teacher')", async () => {
  const w = new World();
  const [A, E, D, V] = ["Ann", "Eli", "Dee", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, A!, [[E!, "editor"], [V!, "viewer"]]);
  const pub = (await publish(w, D!, bankOf([q("Dee's public question about glacial erratics?")]), { license: "CC-BY-SA-4.0" })).body as Q;
  const withOrigin = { ...q("Dee's public question about glacial erratics?"), origin: { item_id: pub.item_id, qid: "x", publish_id: pub.publish_id, version: 1 } };
  const s1 = await send(w, A!, cid, [q("Ann's own question about moraines?"), withOrigin], { license: "CC-BY-SA-4.0" });
  assert.equal(s1.status, 201, JSON.stringify(s1.body));
  const bid = s1.body.bank_id;
  // Eli's version (from his copy: no origin fields), with the public question moved to the front.
  const s2 = await send(w, E!, cid, [`qid:${s1.body.qids[1]}:Dee's public question about glacial erratics?`, `qid:${s1.body.qids[0]}:Ann's own question about moraines?`, "Eli's question about drumlins?"], { bid, base: 1, license: "CC-BY-SA-4.0" });
  assert.equal(s2.status, 200, JSON.stringify(s2.body));
  const r2 = await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: V! });
  assert.deepEqual(r2.body.origins.map((o: Q) => [o.pos, o.item_id, o.license, o.author_name]), [[0, pub.item_id, "CC-BY-SA-4.0", "Dee"]]);
  assert.equal((await w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: A!, json: { license: "circle-only" } })).body.error, "license_incompatible", "still bound");
  // A version that drops it is free of it; a rollback brings it (and the bound) back.
  const s3 = await send(w, E!, cid, [`qid:${s1.body.qids[0]}:Ann's own question about moraines?`, `qid:${s2.body.qids[2]}:Eli's question about drumlins?`], { bid, base: 2, license: "CC-BY-SA-4.0" });
  assert.equal(s3.status, 200);
  assert.equal((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: V! })).body.origins.length, 0);
  const rb = await rollback(w, A!, cid, bid, { to_version: 2, base_version: 3 });
  assert.equal(rb.status, 200, JSON.stringify(rb.body));
  assert.deepEqual((await w.json<any>("GET", `/circles/${cid}/banks/${bid}`, { user: V! })).body.origins.map((o: Q) => [o.pos, o.item_id]), [[0, pub.item_id]]);
  assert.equal((await w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: A!, json: { license: "circle-only" } })).body.error, "license_incompatible");
  // The public author is erased: her name goes from every circle read and was never stored with a version.
  await erase(w, D!);
  assert.deepEqual((await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions/2`, { user: V! })).body.origins.map((o: Q) => o.author_name), ["deleted teacher"]);
  assert.ok(w.q<any>(`SELECT origins FROM circle_versions WHERE origins IS NOT NULL`).every((x) => !x.origins.includes("Dee")));
});
