// Round 22: critics 29a (Moodle) and 29b (ASVS), inverted. A takedown of an item purged before its
// notice follows the live state machine (pending strike, counter-notice window, upheld after it, voided
// by a put-back or withdrawal; B1). Sketches and bands of purged, erased or private content are HMAC
// under the 'sketch-v1' subkey of POOL_HASH_KEY, unordered sets, never FNV or unkeyed (B2). Every
// licence an item had at a content since a serve counts (V-a). A text unit has its own seq (29b B1);
// text first seen with one author stays hers however many copy it (29b B2). Privacy purges, put-backs,
// erasures and repeat-infringer checks that fail part way are finished (H2, H3, mediums).
import assert from "node:assert/strict";
import { test } from "node:test";
import { DatabaseSync } from "node:sqlite";
import { readFileSync, readdirSync } from "node:fs";
import { deleteContentStatements, flushTrapsLater } from "../../src/pool/index.ts";
import { guardCall, takeWasmTraps } from "../../src/pool/core.ts";
import { keyedBands, keyedBody, keyedSet, setFingerprintKey } from "../../src/pool/shared.ts";
import { stemBands, stemTokens } from "../../src/pool/provenance.ts";
import { LINEAGE_COPIES_MAX } from "../../src/pool/moderation.ts";
import { grantOptions } from "../../src/pool/license.ts";
import { framed, publish, publishId, TEST_HASH_KEY, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
const code = (r: { status: number; body: Q }) => (r.status >= 400 ? `${r.status} ${r.body.error}` : String(r.status));
const setLimit = (w: World, u: User, metric: string, value: number) =>
  w.sql.prepare(`INSERT OR REPLACE INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, ?, ?, 0, 'x')`).run(u.id, metric, value);
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const days = async (w: World, n: number) => {
  for (let d = 0; d < n; d++) {
    w.advance(DAY);
    await w.pool.hourly();
    await w.pool.daily();
  }
};
const quiet = async <T>(f: () => Promise<T>): Promise<T> => {
  const e = console.error;
  console.error = () => {};
  try {
    return await f();
  } finally {
    console.error = e;
  }
};
const LONG = "Ann's own worked explanation: the logger shows water above twenty degrees on forty days of the 2019 season, juvenile counts fell in exactly those weeks, and the regression slope links them clearly.";
const AQ = mcq("Ann's own question: in her 2019 delta survey, which landform formed first at the mouth of the river?", [["A delta lobe", true], ["A mesa", false]]);
const CN = (name: string) => ({ signature: `/s/ ${name}`, material_desc: "my own question", name, address: "1 Main St", phone: "555-0100", perjury_stmt: true, consent_jurisdiction: true, accept_service: true });
const chainOf = async (w: World, u: User, id: string) => ((await get(w, u, id)).body.questions?.[0]?.chain ?? []).map((l: any) => [l.author_name, l.license]);

/** The Nth D1 operation, or the first whose SQL matches, throws (once). */
function faults(w: World) {
  const db = w.db as any;
  const orig = { prepare: db.prepare, batch: db.batch };
  const st = { pred: null as null | ((sqls: string[]) => boolean), fired: 0 };
  const tick = (sqls: string[]) => {
    if (st.pred && st.pred(sqls)) {
      st.pred = null;
      st.fired++;
      throw new Error("D1_ERROR: injected failure");
    }
  };
  const wrap = (s: any): any => ({ sql: s.sql, args: s.args, bind: (...a: unknown[]) => wrap(s.bind(...a)), first: async () => (tick([s.sql]), s.first()), all: async () => (tick([s.sql]), s.all()), run: async () => (tick([s.sql]), s.run()) });
  db.prepare = (sql: string) => wrap(orig.prepare(sql));
  db.batch = async (ss: any[]) => (tick(ss.map((x) => x.sql)), orig.batch(ss));
  return st;
}

// ---- 29a B1: a takedown of a purged item, the live state machine ----------------------------------

/** Ann's item, copied by Bob (imported), deleted by Ann and purged; then the notice. */
async function ghostTakedown(o: { erase?: boolean } = {}) {
  const w = new World();
  const adm = w.user("admin"), Ann = w.user("teacher", "Ann"), Bob = w.user("teacher", "Bob"), Eve = w.user("teacher", "Eve");
  const a = (await publish(w, Ann, bankOf([AQ]), { license: "CC-BY-NC-4.0" })).body as any;
  await imp(w, Bob, a.item_id);
  const b = (await publish(w, Bob, bankOf([AQ]), { license: "CC-BY-NC-4.0" })).body as any;
  await w.json("DELETE", `/items/${a.item_id}`, { user: Ann });
  if (o.erase) {
    w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, Ann.id);
    await w.db.batch(deleteContentStatements(w.db, Ann.id, w.clock));
  }
  await days(w, 16);
  assert.equal(w.q<any>(`SELECT purged_at IS NOT NULL AS p FROM items WHERE id = ?`, a.item_id)[0].p, 1, "purged before the notice");
  w.notices.length = 0;
  const nid = await notice(w, adm);
  // (round 23 A2: a ghost's plan selects nothing by default: "all" is the explicit default)
  const t = await w.takedown<any>(adm, nid, a.item_id, { questions: "all" });
  assert.equal(t.status, 200);
  return { w, adm, Ann, Bob, Eve, a, b, nid, t };
}
const strikesOf = (w: World, u: User) => w.q<any>(`SELECT upheld_at IS NOT NULL AS u, voided_at IS NOT NULL AS v FROM strikes WHERE user_id = ?`, u.id).map((s) => [s.u, s.v]);

test("29a B1 (s2_td_purged): a takedown of a purged item is pending: the author is told, the copy keeps her credit, a counter-notice is taken, a withdrawal voids the strike", async () => {
  const { w, adm, Ann, Bob, Eve, a, b, nid, t } = await ghostTakedown();
  assert.deepEqual(strikesOf(w, Ann), [[0, 0]], "a pending strike");
  assert.deepEqual(w.notices.map((n: any) => n.kind), ["takedown_author"], "the author is told");
  assert.deepEqual(t.body.copies.map((x: any) => x.item_id), [b.item_id], "the copy is listed");
  assert.equal(t.body.copies_more, false);
  assert.deepEqual(await chainOf(w, Eve, b.item_id), [["Ann", "CC-BY-NC-4.0"]], "Ann's credit stays while the window is open");
  assert.equal(w.q(`SELECT 1 FROM void_items`).length, 0, "nothing voided before the uphold");
  assert.equal(code(await publish(w, Bob, bankOf(own(b, [AQ])), { license: "CC0-1.0", base: b.publish_id }, b.item_id)), "422 license_incompatible", "Bob cannot launder to CC0");
  const cn = await w.json<any>("POST", `/items/${a.item_id}/counter-notice`, { user: Ann, json: CN("Ann") });
  assert.equal(cn.status, 202, "the counter-notice route takes a purged item");
  const wd = await w.admin<any>(adm, "POST", "/admin/dmca/withdraw", { notice_id: nid, note: "complainant withdrew" });
  assert.deepEqual([wd.status, wd.body.restored], [200, 1]);
  assert.deepEqual(strikesOf(w, Ann), [[0, 1]], "the withdrawal voids the strike");
  assert.equal(w.q(`SELECT 1 FROM void_items`).length, 0);
  assert.equal(w.q<any>(`SELECT removed_reason FROM items WHERE id = ?`, a.item_id)[0].removed_reason, "author_deleted", "the tombstone is what it was");
  assert.equal(w.q(`SELECT 1 FROM blocked_hashes WHERE lifted_at IS NULL`).length, 0, "its blocks are lifted");
  await days(w, 40);
  assert.deepEqual(strikesOf(w, Ann), [[0, 1]], "never upheld later");
  assert.deepEqual(await chainOf(w, Eve, b.item_id), [["Ann", "CC-BY-NC-4.0"]]);
});

test("29a B1: with no counter-notice the ghost takedown is upheld after 30 days: evidence void, strike upheld, copies held, repeat-infringer check", async () => {
  const { w, Ann, Bob, Eve, a, b } = await ghostTakedown();
  await days(w, 29);
  assert.deepEqual(strikesOf(w, Ann), [[0, 0]], "still pending inside the window");
  assert.equal(w.q(`SELECT 1 FROM void_items WHERE item_id = ?`, a.item_id).length, 0);
  await days(w, 2);
  assert.deepEqual(strikesOf(w, Ann), [[1, 0]], "upheld");
  assert.equal(w.q(`SELECT 1 FROM void_items WHERE item_id = ?`, a.item_id).length, 1, "its evidence is void");
  assert.equal(w.q(`SELECT 1 FROM item_holds WHERE item_id = ? AND source LIKE 'dmca:%'`, b.item_id).length, 1, "the copy is held");
  assert.equal((await get(w, Eve, b.item_id)).status, 404);
  assert.equal(code(await w.json<any>("POST", `/items/${a.item_id}/counter-notice`, { user: Ann, json: CN("Ann") })), "409 window_closed");
  void Bob;
  // Three upheld ghost takedowns: the repeat-infringer flag.
  const w2 = new World();
  const adm = w2.user("admin"), I = w2.user("teacher", "Inf"), R = w2.user();
  for (let k = 0; k < 3; k++) {
    const x = (await publish(w2, I, bankOf([mcq(`Infringer's pasted question ${k} about plate boundaries over geologic time?`, [["Divergent", true], ["Static", false]])]))).body as any;
    await get(w2, R, x.item_id);
    await w2.json("DELETE", `/items/${x.item_id}`, { user: I });
    await days(w2, 16);
    assert.equal((await w2.takedown<any>(adm, await notice(w2, adm), x.item_id, { questions: "all" })).status, 200);
  }
  await days(w2, 31);
  assert.equal(w2.q(`SELECT 1 FROM strikes WHERE upheld_at IS NOT NULL`).length, 3);
  assert.equal(w2.q(`SELECT 1 FROM admin_flags WHERE kind = 'repeat_infringer' AND user_id = ?`, I.id).length, 1);
});

test("29a B1: a counter-notice on a ghost takedown puts it back after 10 business days: strike voided, blocks lifted, the tombstone as it was", async () => {
  const { w, Ann, Eve, a, b } = await ghostTakedown();
  w.advance(DAY);
  assert.equal((await w.json<any>("POST", `/items/${a.item_id}/counter-notice`, { user: Ann, json: CN("Ann") })).status, 202);
  await days(w, 45);
  assert.deepEqual(strikesOf(w, Ann), [[0, 1]]);
  assert.equal(w.q<any>(`SELECT outcome FROM dmca_actions`)[0].outcome, "put_back");
  assert.equal(w.q(`SELECT 1 FROM void_items`).length, 0);
  assert.equal(w.q(`SELECT 1 FROM blocked_hashes WHERE lifted_at IS NULL`).length, 0);
  assert.equal(w.q<any>(`SELECT removed_reason FROM items WHERE id = ?`, a.item_id)[0].removed_reason, "author_deleted");
  assert.deepEqual(await chainOf(w, Eve, b.item_id), [["Ann", "CC-BY-NC-4.0"]]);
});

test("29a B1: an erased author cannot counter: the takedown stands after 30 days, but with no rightful item named the ghost's licence still binds the copies", async () => {
  const { w, Ann, Bob, b, t } = await ghostTakedown({ erase: true });
  assert.deepEqual(w.notices.map((n: any) => n.kind), [], "an erased account is told nothing (the admin alone sees it)");
  assert.equal(t.body.copies.length, 1);
  await days(w, 31);
  assert.deepEqual(strikesOf(w, Ann), [[1, 0]], "the takedown stands");
  assert.equal(w.q(`SELECT 1 FROM void_items`).length, 0, "no evidence void: nobody launders it by the takedown");
  assert.equal(w.q(`SELECT 1 FROM item_holds WHERE item_id = ?`, b.item_id).length, 1, "the copy is held for the admin");
  w.sql.prepare(`DELETE FROM item_holds WHERE item_id = ?`).run(b.item_id);
  assert.equal(code(await publish(w, Bob, bankOf(own(b, [AQ])), { license: "CC0-1.0", base: b.publish_id }, b.item_id)), "422 license_incompatible");
});

test("29a B1: the same notice's takedown of a ghost again returns the same action; another notice is a 409", async () => {
  const { w, adm, a, nid, t } = await ghostTakedown();
  const again = await w.takedown<any>(adm, nid, a.item_id, { questions: "all" });
  assert.deepEqual([again.status, again.body.action_id], [200, t.body.action_id]);
  assert.equal(code(await w.takedown<any>(adm, await notice(w, adm), a.item_id, { questions: "all" })), "409 already_removed");
  assert.equal(w.q<any>(`SELECT status FROM dmca_notices WHERE id = ?`, nid)[0].status, "actioned");
});

// ---- 29a V-b, 29b: the takedown's copies, paging, a closed action --------------------------------

test("29a V-b (s3_td_cycle): the takedown response lists the copies hold-copies holds (first page, copies_more); /admin/dmca/copies pages each list on its own cursor; a closed action holds nothing more", async () => {
  const w = new World();
  const adm = w.user("admin"), Ann = w.user("teacher", "Ann");
  const BQ = mcq("Ann's second question: which gauge reading shows the flood peak two days after the storm?", [["Gauge 4", true], ["Gauge 1", false]]);
  const a = (await publish(w, Ann, bankOf([AQ, BQ]), { license: "CC-BY-NC-4.0" })).body as any;
  const N = LINEAGE_COPIES_MAX + 3;
  for (let k = 0; k < N; k++) {
    const u = w.user();
    await imp(w, u, a.item_id);
    await publish(w, u, bankOf([AQ, BQ]), { license: "CC-BY-NC-4.0" });
  }
  const t = await w.takedown<any>(adm, await notice(w, adm), a.item_id);
  assert.deepEqual([t.body.copies.length, t.body.copies_more], [LINEAGE_COPIES_MAX, true], "the real count, and that there are more");
  const aid = t.body.action_id as string;
  const h1 = await w.holdCopies<any>(adm, aid);
  assert.equal(h1.body.held.length, LINEAGE_COPIES_MAX);
  // One held page and the remaining candidates: each list pages on its own cursor.
  const g = await w.admin<any>(adm, "GET", "/admin/dmca/copies", null, `?action_id=${aid}`);
  assert.equal(g.body.held.length, LINEAGE_COPIES_MAX);
  assert.equal(g.body.candidates.length, 3);
  assert.equal(g.body.next_candidates, null);
  const seenHeld = new Set<string>(g.body.held.map((x: any) => x.item_id));
  for (let cur = g.body.next_held; cur; ) {
    const p = await w.admin<any>(adm, "GET", "/admin/dmca/copies", null, `?action_id=${aid}&held_after=${cur}`);
    for (const x of p.body.held) seenHeld.add(x.item_id);
    cur = p.body.next_held;
  }
  assert.equal(seenHeld.size, LINEAGE_COPIES_MAX);
  // Withdrawn: the copies query refuses, the candidates are not offered.
  await w.admin<any>(adm, "POST", "/admin/dmca/withdraw", { notice_id: w.q<any>(`SELECT notice_id FROM dmca_actions WHERE id = ?`, aid)[0].notice_id, note: "x" });
  assert.equal(code(await w.holdCopies<any>(adm, aid)), "409 action_closed");
  const g2 = await w.admin<any>(adm, "GET", "/admin/dmca/copies", null, `?action_id=${aid}`);
  assert.deepEqual([g2.body.restored, g2.body.candidates.length], [true, 0]);
});

// ---- 29a B2: keyed sketches and bands ------------------------------------------------------------

const NAME = "zelda", SUR = "quintanilla";
const STEM = `Student ${NAME} ${SUR} in period 3 scored 42 on the retake: what is the mean of 40 and 44?`;
const P = 0x01000193;
const PINV = (() => {
  let x = 1;
  for (let i = 0; i < 31; i++) (x = Math.imul(x, x) >>> 0), (x = Math.imul(x, P) >>> 0);
  return x;
})();
const fnvFrom = (h0: number, s: string) => {
  let h = h0 >>> 0;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, P) >>> 0;
  }
  return h >>> 0;
};
const back = (h: number, s: string) => {
  for (let i = s.length - 1; i >= 0; i--) {
    h = Math.imul(h, PINV) >>> 0;
    h = (h ^ s.charCodeAt(i)) >>> 0;
  }
  return h >>> 0;
};
const seeded = (s: string, seed: number) => fnvFrom((0x811c9dc5 ^ Math.imul(seed, 0x9e3779b1)) >>> 0, s);
/** Every TEXT value of every row of the tables that outlive content. */
const SKETCH_TABLES = ["ghost_questions", "ghost_units", "ghost_bands", "privacy_bands", "stem_bands", "item_questions", "item_part_keys"];
const cellsOf = (w: World) => SKETCH_TABLES.flatMap((t) => w.q<Record<string, unknown>>(`SELECT * FROM ${t}`).flatMap((r) => Object.entries(r).map(([k, v]) => ({ t, k, v: typeof v === "string" ? v : "" }))));

test("29a B2 (s1_sketch, t5_sketch): after a privacy purge no stored sketch or band can be inverted, tested against a dictionary of the actual words, or ordered", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user("teacher", "Ann"), R = w.user();
  const pub = (await publish(w, A, bankOf([mcq(STEM, [["42", true], ["40", false]])]))).body as any;
  // A live item gives any database reader known (token, sketch) pairs.
  await publish(w, A, bankOf([mcq("What is the boiling point of water at sea level in degrees?", [["100", true], ["50", false]])]));
  await get(w, R, pub.item_id);
  assert.equal((await w.purge<any>(adm, pub.item_id, { questions: [0] })).status, 200);
  await days(w, 20);
  const words = stemTokens(STEM);
  const dict = [...words, "the", "what", "smith", "maria"];
  const priv = w.q<{ band: string; tokens: string | null }>(`SELECT band, tokens FROM privacy_bands`);
  assert.ok(priv.length > 0 && priv.every((x) => x.tokens !== null), "the privacy near-check keeps its keyed rows");
  const sk = JSON.parse(priv[0]!.tokens!) as string[];
  // (a) 64-bit hex, deduplicated, sorted: no order, no positions.
  assert.ok(sk.every((x) => /^[0-9a-f]{16}$/.test(x)));
  assert.deepEqual(sk, [...new Set(sk)].sort());
  assert.equal(sk.length, new Set(words).size);
  // (b) FNV inversion from a live known pair, and from guessed stopwords: nothing decodes.
  const live = w.q<any>(`SELECT stem_tokens, tok_sk FROM item_questions WHERE tok_sk IS NOT NULL LIMIT 1`)[0];
  const lt = JSON.parse(live.stem_tokens) as string[], ls = JSON.parse(live.tok_sk) as string[];
  const vals = sk.map((x) => parseInt(x.slice(0, 8), 16)).concat(sk.map((x) => parseInt(x.slice(8), 16)));
  for (const t of lt)
    for (const v of ls) {
      const h0 = back(parseInt(v.slice(0, 8), 16), t);
      assert.deepEqual(dict.filter((d) => d !== t && vals.includes(fnvFrom(h0, d))), [], `from the pair (${t})`);
    }
  for (const g of ["what", "the", "of", "is", "student"])
    for (const v of vals) {
      const h0 = back(v, g);
      assert.ok(!dict.some((d) => d !== g && vals.includes(fnvFrom(h0, d))), `stopword ${g} decodes nothing`);
    }
  // (c) A dictionary of the actual words under every unkeyed scheme (FNV seeds, SHA-256, the old base36 sketch): no hit.
  const stored = new Set(cellsOf(w).flatMap((c) => (c.v.startsWith("[") ? (() => { try { return (JSON.parse(c.v) as unknown[]).map(String); } catch { return []; } })() : [c.v])));
  for (const d of dict) {
    for (let seed = 0; seed < 64; seed++) {
      assert.ok(!stored.has(seeded(d, seed).toString(36)) && !stored.has(seeded(d, seed).toString(16)), `fnv seed ${seed} of ${d}`);
    }
    const sha = Buffer.from(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(d))).toString("hex");
    assert.ok(!stored.has(sha.slice(0, 16)) && !stored.has(sha));
  }
  // (d) The unkeyed MinHash bands of the purged stem appear nowhere.
  for (const b of stemBands(words)) assert.ok(!stored.has(b), `unkeyed band ${b}`);
  for (const r of priv) assert.match(r.band, /^[0-9a-f]{16}$/);
  // (e) Positive control: they are exactly the keyed sets under the deployment's key.
  assert.deepEqual(sk, await keyedSet(words));
  assert.deepEqual(new Set(priv.map((r) => r.band)), new Set(await keyedBands(stemBands(words))));
  // (f) Reordered words give the same sketch (no order is kept).
  assert.deepEqual(await keyedSet([...words].reverse()), sk);
  assert.ok(!cellsOf(w).some((c) => c.v.toLowerCase().includes(NAME) || c.v.toLowerCase().includes(SUR)), "no plaintext");
});

test("29a B2 lint (s1b_erase): after a purge, an erasure and a delete-to-ghost, no table that outlives content keeps a word, an unkeyed band or an ordered hash of it", async () => {
  // The schema: ghost and privacy rows have no token-list columns.
  const w = new World();
  for (const t of ["ghost_questions", "ghost_units", "ghost_bands", "privacy_bands"]) {
    const cols = w.q<{ name: string }>(`PRAGMA table_info(${t})`).map((c) => c.name);
    assert.ok(!cols.some((c) => /^(stem_tokens|body_tokens|stem_bands|tokens_plain|lineage_root)$/.test(c)), `${t}: ${cols}`);
  }
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), R = w.user(), adm = w.user("admin");
  const S1 = `Mrs Delacroix asks: which river did our class visit on the Thursday field trip to Paradise?`;
  const S2 = `Coach Ferreira asks: which lake did the swim team cross on the Saturday trip to Oroville?`;
  const S3 = `Student ${NAME} ${SUR} in period 3 scored 42 on the retake: what is the mean of 40 and 44?`;
  const p1 = (await publish(w, A, bankOf([mcq(S1, [["The Feather River at Oroville", true], ["The Sacramento", false]])]))).body as any;
  const p2 = (await publish(w, B, bankOf([mcq(S2, [["Lake Oroville", true], ["Lake Tahoe", false]])]))).body as any;
  const p3 = (await publish(w, B, bankOf([q(`Part case: ${S3}`, { kind: { Multipart: { parts: [{ stem: S3, kind: { Short: { sample: "forty two" } } }] } } })]))).body as any;
  for (const p of [p1, p2, p3]) await get(w, R, p.item_id);
  await w.json("DELETE", `/items/${p1.item_id}`, { user: A });
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, A.id);
  await w.db.batch(deleteContentStatements(w.db, A.id, w.clock));
  await w.json("DELETE", `/items/${p2.item_id}`, { user: B });
  assert.equal((await w.purge<any>(adm, p3.item_id, { questions: [0] })).status, 200);
  await days(w, 30);
  const gone = [p1, p2, p3].map((p) => p.item_id);
  assert.equal(w.q(`SELECT 1 FROM stem_bands WHERE item_id IN (?, ?, ?)`, ...gone).length, 0, "no unkeyed band of a gone item");
  assert.equal(w.q(`SELECT 1 FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id WHERE p.item_id IN (?, ?, ?)`, ...gone).length, 0);
  assert.ok(w.q(`SELECT 1 FROM ghost_bands`).length > 0, "ghosts are found by keyed bands");
  const needles = [...new Set([S1, S2, S3].flatMap((s) => stemTokens(s)).filter((t) => t.length >= 5))];
  const bands = new Set([S1, S2, S3].flatMap((s) => stemBands(stemTokens(s))));
  for (const c of cellsOf(w)) {
    for (const n of needles) assert.ok(!c.v.toLowerCase().split(/[^a-z0-9]+/).includes(n), `${c.t}.${c.k} keeps "${n}"`);
    for (const b of bands) assert.ok(!c.v.includes(b), `${c.t}.${c.k} keeps an unkeyed band`);
  }
  for (const r of w.q<{ tok_sk: string | null; body_sk: string | null }>(`SELECT tok_sk, body_sk FROM ghost_questions`))
    for (const v of [r.tok_sk, r.body_sk]) if (v) {
      const xs = JSON.parse(v) as string[];
      assert.ok(xs.every((x) => /^[0-9a-f]{16}$/.test(x)));
      assert.deepEqual(xs.slice(1), [...new Set(xs.slice(1))].sort(), "a set, sorted");
    }
  // Source lint: the FNV sketch is gone from the code.
  for (const f of ["publish.ts", "provenance.ts", "moderation.ts", "cron.ts"]) assert.doesNotMatch(readFileSync(new URL(`../../src/pool/${f}`, import.meta.url), "utf8"), /sketchSeed|\bsketch\(/, f);
});

test("29a B2 differential: matching quality is unchanged: a near-rewording of a purged (ghost) source still binds, and the privacy near-check still holds", async () => {
  const STEMA = "In Ann's 2019 delta survey of the Feather River mouth, which landform formed first after the spring flood receded?";
  const STEMB = "In Ann's 2019 delta survey of the Feather River mouth, which landform formed first after the spring flood receded fully?";
  const ans: [string, boolean][] = [["A delta lobe of fresh sediment", true], ["A mesa of old rock", false], ["A butte", false]];
  for (const gone of [false, true]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    const a = (await publish(w, A, bankOf([mcq(STEMA, ans)]), { license: "CC-BY-NC-4.0" })).body as any;
    await imp(w, M, a.item_id);
    if (gone) {
      await w.json("DELETE", `/items/${a.item_id}`, { user: A });
      await days(w, 20);
      assert.equal(w.q(`SELECT 1 FROM item_questions`).length, 0, "only the ghost is left");
    }
    const r = await publish(w, M, bankOf([mcq(STEMB, ans)]), { license: "CC0-1.0" });
    assert.equal(code(r), "422 license_incompatible", `gone=${gone}: the near-rewording binds`);
  }
  // Privacy: a near-rewording of purged student data is held for review.
  const w = new World();
  const adm = w.user("admin"), A = w.user("teacher", "Ann"), Z = w.user("teacher", "Zed");
  const pub = (await publish(w, A, bankOf([mcq(STEM, [["42", true], ["40", false]])]))).body as any;
  assert.equal((await w.purge<any>(adm, pub.item_id, { questions: [0] })).status, 200);
  await days(w, 20);
  const near = await publish(w, Z, bankOf([mcq(STEM.replace("retake", "retake test"), [["42", true], ["40", false]])]), { visibility: "unlisted" });
  assert.equal(near.status, 201);
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ?`, near.body.item_id).length, 1, "the near-check flags it");
  const other = await publish(w, Z, bankOf([mcq("Which value is the mean of 10 and 14 on a short practice quiz?", [["12", true], ["10", false]])]), { visibility: "unlisted" });
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ?`, other.body.item_id).length, 0);
});


// ---- 29a V-a: licence history -------------------------------------------------------------------

test("29a V-a (s7_cc0_interval): a CC0 interval after the serve counts however many republishes there were, live or after delete and purge", async () => {
  for (const extra of [0, 1, 2])
    for (const tail of ["none", "delete+purge"]) {
      const w = new World();
      const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
      let a: any = (await publish(w, A, bankOf([AQ]), { license: "CC-BY-NC-4.0" })).body;
      await imp(w, M, a.item_id);
      const rp = async (L: string) => {
        w.advance(DAY);
        const r = await publish(w, A, bankOf(own(a, [AQ])), { license: L, base: a.publish_id }, a.item_id);
        assert.equal(r.status, 200);
        a = { ...r.body, item_id: a.item_id };
      };
      await rp("CC0-1.0");
      for (let k = 0; k < extra; k++) await rp("CC0-1.0");
      await rp("CC-BY-NC-4.0");
      if (tail !== "none") {
        await w.json("DELETE", `/items/${a.item_id}`, { user: A });
        await days(w, 20);
      }
      assert.equal(code(await publish(w, M, bankOf([AQ]), { license: "CC0-1.0" })), "201", `extra=${extra} tail=${tail}`);
    }
});

// ---- 29b B1: a text unit's own seq ----------------------------------------------------------------

const SQ = () => mcq("Sock's old question: which gauge reads the river level at the Freeport station?", [["Staff gauge", true], ["Rain gauge", false]]);
const ANN = (expl: string | null, pts: number | null = null) => mcq("In Ann's field study, which factor best explains the 2019 decline of juvenile Chinook salmon at the trap?", [["Warm water", true], ["Bass", false]], { explanation: expl, points: pts });

test("29b B1 (t1_seqcarry): a republish that ADDS a text gives it the new publish's seq, never the question's old one: no backdated laundering", async () => {
  for (const variant of ["control", "sock-backdate", "self-backdate"] as const) {
    const w = new World();
    const S = w.user("teacher", "Sock"), A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    const oldOwner = variant === "self-backdate" ? M : S;
    const oldLic = variant === "self-backdate" ? "CC-BY-NC-SA-4.0" : "CC0-1.0";
    const s = (await publish(w, oldOwner, bankOf([SQ()]), { license: oldLic })).body as any;
    w.advance(DAY);
    const a = (await publish(w, A, bankOf([ANN(LONG)]), { license: "CC-BY-NC-SA-4.0" })).body as any;
    w.advance(DAY);
    await get(w, M, a.item_id);
    if (variant !== "control") {
      assert.equal((await publish(w, oldOwner, bankOf(own(s, [{ ...SQ(), explanation: LONG }])), { license: oldLic, base: s.publish_id }, s.item_id)).status, 200);
      const row = w.q<any>(`SELECT iq.seq, iq.unit_seqs FROM item_questions iq JOIN items i ON i.current_publish_id = iq.publish_id WHERE i.id = ?`, s.item_id)[0];
      const annSeq = w.q<any>(`SELECT iq.seq FROM item_questions iq JOIN items i ON i.current_publish_id = iq.publish_id WHERE i.id = ?`, a.item_id)[0].seq;
      assert.ok(Object.values(JSON.parse(row.unit_seqs) as Record<string, number>).every((x) => x > annSeq), `${variant}: the text is dated by its own publish`);
      if (variant === "sock-backdate") await get(w, M, s.item_id);
    }
    w.advance(DAY);
    const mal = await publish(w, M, bankOf([mcq("Mal's quiz: what explains the 2019 decline of Chinook juveniles?", [["Temperature", true], ["Noise", false]], { explanation: LONG })]), { license: "CC0-1.0" });
    assert.equal(code(mal), "422 license_incompatible", variant);
    // (the text's own unit keys: those Ann's record and Mal's share)
    const ac = w.q<any>(`SELECT m.first_seq FROM author_contents m JOIN author_contents a ON a.hash = m.hash AND a.kind = 'unit' AND a.author_id = ? WHERE m.kind = 'unit' AND m.author_id = ?`, A.id, M.id).map((r) => r.first_seq);
    const annFirst = w.q<any>(`SELECT MIN(first_seq) AS s FROM author_contents WHERE kind = 'unit' AND author_id = ?`, A.id)[0].s;
    if (variant === "self-backdate") assert.ok(ac.length > 0 && ac.every((x) => x > annFirst), "Mal's own record of the text is not earlier than Ann's");
  }
});

test("29b B1 (t1b_victim): a sock that backdates Ann's text into an old item never makes Ann's own republish look like the copy", async () => {
  for (const [sockLic, annReads] of [["CC0-1.0", false], ["CC-BY-NC-SA-4.0", false], ["CC-BY-NC-SA-4.0", true]] as const) {
    const w = new World();
    const S = w.user("teacher", "Sock"), A = w.user("teacher", "Ann"), R = w.user();
    const s = (await publish(w, S, bankOf([SQ()]), { license: sockLic })).body as any;
    w.advance(DAY);
    const a = (await publish(w, A, bankOf([ANN(LONG)]), { license: "CC-BY-SA-4.0" })).body as any;
    w.advance(DAY);
    assert.equal((await publish(w, S, bankOf(own(s, [{ ...SQ(), explanation: LONG }])), { license: sockLic, base: s.publish_id }, s.item_id)).status, 200);
    if (annReads) await get(w, A, s.item_id);
    w.advance(DAY);
    const fix = await publish(w, A, bankOf(own(a, [ANN(LONG, 3)])), { license: "CC-BY-SA-4.0", base: a.publish_id }, a.item_id);
    assert.equal(fix.status, 200);
    assert.deepEqual(await chainOf(w, R, a.item_id), [], `${sockLic}/${annReads}: no link to the sock`);
    assert.deepEqual(w.q(`SELECT kind FROM admin_flags WHERE item_id = ?`, a.item_id), []);
    assert.equal((await publish(w, A, bankOf([mcq("Ann's second quiz: what explains the Chinook decline at the Freeport trap?", [["Heat", true], ["Light", false]], { explanation: LONG })]), { license: "CC-BY-SA-4.0" })).status, 201);
  }
});

// ---- 29b B2: stock needs authors earlier than the source -------------------------------------------

const MALQ = (extra: Q = {}) => mcq("Mal's own question about rivers: is the water warm in August?", [["Yes", true], ["No", false]], extra);
test("29b B2 (t1c_stock, t1d_flood): later copies (socks, backdated or not, 0/9/70 of them) never make a text stock; two earlier independent authors do", async () => {
  for (const mode of ["control", "two-later-socks", "two-backdated-socks", "two-earlier-authors"] as const) {
    const w = new World();
    const S1 = w.user("teacher", "Sock1"), S2 = w.user("teacher", "Sock2");
    const old: any[] = [];
    if (mode === "two-backdated-socks")
      for (const [k, S] of [S1, S2].entries()) old.push({ S, k, p: (await publish(w, S, bankOf([mcq(`Sock ${k} old filler: is ${k} even?`, [["yes", true], ["no", false]])]), { license: "CC0-1.0" })).body });
    if (mode === "two-earlier-authors")
      for (const [k, S] of [S1, S2].entries()) assert.equal((await publish(w, S, bankOf([mcq(`Earlier author ${k}: is ${k} even?`, [["yes", true], ["no", false]], { explanation: LONG })]), { license: "CC0-1.0" })).status, 201);
    w.advance(DAY);
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    const a = (await publish(w, A, bankOf([mcq("Ann's question: which factor explains the 2019 decline at the Freeport trap?", [["Temperature", true], ["Bass", false]], { explanation: LONG })]), { license: "CC-BY-NC-SA-4.0" })).body as any;
    await imp(w, M, a.item_id);
    w.advance(DAY);
    if (mode === "two-later-socks") for (const [k, S] of [S1, S2].entries()) await publish(w, S, bankOf([mcq(`Sock ${k} filler: is ${k} even?`, [["yes", true], ["no", false]], { explanation: LONG })]), { license: "CC0-1.0" });
    if (mode === "two-backdated-socks")
      for (const o of old) await publish(w, o.S, bankOf(own(o.p, [mcq(`Sock ${o.k} old filler: is ${o.k} even?`, [["yes", true], ["no", false]], { explanation: LONG })])), { license: "CC0-1.0", base: o.p.publish_id }, o.p.item_id);
    const r = await publish(w, M, bankOf([MALQ({ explanation: LONG })]), { license: "CC0-1.0" });
    assert.equal(code(r), mode === "two-earlier-authors" ? "201" : "422 license_incompatible", mode);
  }
  for (const n of [0, 9, 70]) {
    const w = new World();
    const S = w.user("teacher", "Sock"), A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    setLimit(w, S, "shared_questions", 100000);
    const a = (await publish(w, A, bankOf([mcq("Ann's question: which factor explains the 2019 decline at the Freeport trap?", [["Temperature", true], ["Bass", false]], { explanation: LONG })]), { license: "CC-BY-NC-SA-4.0" })).body as any;
    w.advance(DAY);
    await imp(w, M, a.item_id);
    for (let k = 0; k < n; k++) {
      const p = await publish(w, w.user("teacher", `Sock${k}`), bankOf([q(`Sock filler ${k}: what is ${k}+1?`, { explanation: LONG })]), { license: "CC0-1.0" });
      await get(w, M, p.body.item_id as string);
    }
    assert.equal(code(await publish(w, M, bankOf([MALQ({ explanation: LONG })]), { license: "CC0-1.0" })), "422 license_incompatible", `flood ${n}`);
  }
});

test("29b B2: copies by accounts served an earlier holder never count toward stock, even when they are earlier than the source the publisher was served", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), C = w.user("teacher", "Cal"), M = w.user("teacher", "Mal");
  const TXT = (who: string) => mcq(`${who}'s question: which factor explains the 2019 decline at the Freeport trap?`, [["Temperature", true], ["Bass", false]], { explanation: LONG });
  const a = (await publish(w, A, bankOf([TXT("Ann")]), { license: "CC-BY-NC-SA-4.0" })).body as any;
  w.advance(DAY);
  await imp(w, B, a.item_id);
  const b = (await publish(w, B, bankOf([TXT("Bob")]), { license: "CC-BY-NC-SA-4.0" })).body as any;
  w.advance(DAY);
  await imp(w, C, b.item_id);
  const c = (await publish(w, C, bankOf([TXT("Cal")]), { license: "CC-BY-NC-SA-4.0" })).body as any;
  w.advance(DAY);
  await imp(w, M, c.item_id);
  assert.equal(code(await publish(w, M, bankOf([MALQ({ explanation: LONG })]), { license: "CC0-1.0" })), "422 license_incompatible");
});

test("29b (t1d): planted copies alone never put the victim's publish on the served-driven path when the victim has read more than the plant", async () => {
  const w = new World();
  const S = w.user("teacher", "Sock"), A = w.user("teacher", "Ann"), X = w.user("teacher", "Xi");
  setLimit(w, S, "shared_questions", 100000);
  setLimit(w, X, "shared_questions", 100000);
  // Ann has read 120 items; the sock plants 70 copies of her text.
  setLimit(w, X, "publishes", 1000);
  setLimit(w, S, "publishes", 1000);
  for (let k = 0; k < 120; k++) {
    const x = await publish(w, X, bankOf([mcq(`Xi's filler ${k}: is ${k} prime?`, [["yes", true], ["no", false]])]));
    assert.equal(x.status, 201);
    assert.equal((await get(w, A, x.body.item_id as string)).status, 200);
  }
  const a = (await publish(w, A, bankOf([mcq("Ann's question: which factor explains the 2019 decline at the Freeport trap?", [["Temperature", true], ["Bass", false]], { explanation: LONG })]))).body as any;
  for (let k = 0; k < 70; k++) assert.equal((await publish(w, S, bankOf([q(`Sock filler ${k}: what is ${k}+1?`, { explanation: LONG })]), { license: "CC0-1.0", visibility: "unlisted" })).status, 201);
  let served = 0;
  w.stats.log = (sql) => void (/FROM served sh CROSS JOIN item_questions iq/.test(sql) && served++);
  const rp = await publish(w, A, bankOf([mcq("Ann's second quiz: what explains the Chinook decline at the Freeport trap?", [["Heat", true], ["Light", false]], { explanation: LONG })]));
  w.stats.log = undefined;
  assert.equal(rp.status, 201);
  void a;
  assert.equal(served, 0, "matched by key: 70 copies are fewer than her 120 serves");
});

// ---- 29b H2, H3, mediums: what fails part way is finished ------------------------------------------

test("29b H2 (t3b_privacy_fault): a privacy purge that fails while holding the copies past the list is finished by the cron (all held), and the admin's retry is a 200", async () => {
  for (const fail of [false, true]) {
    const w = new World({ affectedList: 1 });
    const adm = w.user("admin"), A = w.user("teacher", "Ann"), R = w.user();
    const Q0 = mcq("Student Zed Quorbly in period 3 scored 42 on the retake: what is the mean of 40 and 44?", [["42", true], ["40", false]]);
    const a = (await publish(w, A, bankOf([Q0]))).body as any;
    const copies: string[] = [];
    for (let k = 0; k < 4; k++) {
      const u = w.user();
      await imp(w, u, a.item_id);
      copies.push((await publish(w, u, bankOf([Q0]))).body.item_id as string);
    }
    const f = faults(w);
    if (fail) f.pred = (s) => s.some((x) => x.includes("INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source, detail)\n         SELECT i.id"));
    const p = await quiet(() => w.purge<any>(adm, a.item_id, { questions: [0], confirm_large: true }).catch(() => ({ status: "throw" })));
    if (fail) {
      assert.equal(p.status, "throw");
      assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'privacy_backlog' AND resolved_at IS NULL`).length, 1, "the backlog is recorded with the claim");
      const again = await w.purge<any>(adm, a.item_id, { questions: [0], confirm_large: true });
      assert.equal(again.status, 200, "the retry finishes it");
    }
    await days(w, 2);
    for (const c of copies) assert.equal(code(await get(w, R, c)), "404 not_found", `fault=${fail}`);
    assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'privacy_backlog' AND resolved_at IS NULL`).length, 0);
    assert.ok(w.r2.keys("").filter((k) => k.startsWith("items/")).every((k) => !k.includes(a.item_id)), "its bodies are under quarantine");
  }
});

test("29b H3 (t3_restore_fault): a put-back that fails after its claim is finished by the cron", async () => {
  for (const failOn of ["none", "UPDATE item_publishes SET full_key", "UPDATE strikes SET voided_at", "UPDATE items SET quarantined_at = NULL"] as const) {
    const w = new World();
    const adm = w.user("admin"), I = w.user("teacher", "Ina"), R = w.user();
    const x = (await publish(w, I, bankOf([mcq("Ina's own question about plate boundaries and their motion?", [["Divergent", true], ["Static", false]])]))).body as any;
    assert.equal((await w.takedown<any>(adm, await notice(w, adm), x.item_id)).status, 200);
    assert.equal((await w.json("POST", `/items/${x.item_id}/counter-notice`, { user: I, json: CN("Ina") })).status, 202);
    const f = faults(w);
    if (failOn !== "none") f.pred = (s) => s.some((qq) => qq.trimStart().startsWith(failOn));
    await quiet(() => days(w, 20));
    if (failOn !== "none") assert.equal(f.fired, 1, failOn);
    assert.deepEqual(w.q<any>(`SELECT removed_reason, quarantined_at FROM items WHERE id = ?`, x.item_id)[0], { removed_reason: null, quarantined_at: null }, failOn);
    assert.deepEqual(strikesOf(w, I), [[0, 1]], failOn);
    assert.equal((await get(w, R, x.item_id)).status, 200, failOn);
    assert.ok(w.q<any>(`SELECT full_key FROM item_publishes WHERE item_id = ?`, x.item_id).every((r) => r.full_key.startsWith("items/")));
    // (op 36: no body is left in R2 that no row, nor the orphan list, names)
    const refs = new Set<string>([...w.q<any>(`SELECT full_key AS k FROM item_publishes UNION SELECT stripped_key FROM item_publishes UNION SELECT key FROM r2_orphans`).map((r) => r.k)]);
    for (const k of w.r2.keys("")) if (k.startsWith("items/") || k.startsWith("quarantine/")) assert.ok(refs.has(k), `${failOn}: unreferenced ${k.split("/")[0]}`);
  }
});

test("29b: a repeat-infringer check that fails on the 3rd upheld strike is redone by the cron from the strikes", async () => {
  const w = new World();
  const adm = w.user("admin"), I = w.user("teacher", "Infringer");
  const f = faults(w);
  for (let k = 0; k < 3; k++) {
    const x = (await publish(w, I, bankOf([mcq(`Infringer's pasted textbook question ${k} about plate boundaries?`, [["Divergent", true], ["Static", false]])]))).body as any;
    assert.equal((await w.takedown<any>(adm, await notice(w, adm), x.item_id)).status, 200);
    w.advance(DAY);
  }
  let seen = 0;
  f.pred = (s) => s.some((x) => x.startsWith("SELECT COUNT(*) AS n FROM strikes WHERE user_id = ?1 AND upheld_at")) && ++seen === 3;
  await quiet(() => days(w, 40));
  assert.equal(f.fired, 1);
  assert.equal(w.q(`SELECT 1 FROM strikes WHERE upheld_at IS NOT NULL`).length, 3);
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'repeat_infringer'`).length, 1);
});

test("29b (t3c_races): a takedown marks its notice actioned", async () => {
  const w = new World();
  const adm = w.user("admin"), I = w.user("teacher", "Ina");
  const x = (await publish(w, I, bankOf([AQ]))).body as any;
  const nid = await notice(w, adm);
  assert.equal((await w.takedown<any>(adm, nid, x.item_id)).status, 200);
  assert.equal(w.q<any>(`SELECT status FROM dmca_notices WHERE id = ?`, nid)[0].status, "actioned");
});

test("29b (t4_erasure, t4b_appeal_erasure): an erasure whose batch failed is finished by the cron; its open appeals close so its removed items purge on time; audit notes about it go", async () => {
  for (const mode of ["ok", "fault"] as const) {
    const w = new World();
    const adm = w.user("admin"), V = w.user("teacher", "Vera");
    const U = w.user("teacher", "Ulrich Zandervoort");
    const v = (await publish(w, V, bankOf([mcq("Vera's own question about the water cycle and clouds?", [["Rain", true], ["Sand", false]])]))).body as any;
    await imp(w, U, v.item_id);
    const priv = (await publish(w, U, bankOf([q("Private stemoddlewump: describe osmosis?", { explanation: "Note snorkwhistle" })]), { visibility: "unlisted" })).body as any;
    await w.json("PUT", "/items/prefs", { user: U, json: { keep_on_delete: false } });
    await w.json("POST", `/items/${v.item_id}/report`, { user: U, json: { reason: "wrong_answer", note: "It is wrong grobblefink" } });
    await w.admin(adm, "POST", "/admin/pool/remove", { item_id: priv.item_id, note: "admin note about Ulrich Zandervoort" });
    await w.json("POST", `/items/${priv.item_id}/appeal`, { user: U, json: { note: "Please wibblequask" } });
    w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, U.id);
    if (mode === "fault") {
      const f = faults(w);
      f.pred = (s) => s.some((x) => x.startsWith("DELETE FROM served WHERE reader_id"));
      await assert.rejects(w.db.batch(deleteContentStatements(w.db, U.id, w.clock)));
      assert.equal(w.q(`SELECT 1 FROM erasure_done WHERE user_id = ?`, U.id).length, 0);
    } else await w.db.batch(deleteContentStatements(w.db, U.id, w.clock));
    await days(w, 16);
    assert.equal(w.q(`SELECT 1 FROM erasure_done WHERE user_id = ?`, U.id).length, 1, mode);
    assert.equal(w.q<any>(`SELECT purged_at IS NOT NULL AS p FROM items WHERE id = ?`, priv.item_id)[0].p, 1, `${mode}: purged at the normal time, not held by the appeal`);
    const hay = JSON.stringify(["reports", "admin_flags", "pool_audit", "served", "author_contents", "stem_bands", "blob_uploads", "item_questions", "dmca_counter_notices"].map((t) => w.q(`SELECT * FROM ${t}`))).toLowerCase();
    for (const n of ["grobblefink", "wibblequask", "zandervoort", "stemoddlewump", "snorkwhistle"]) assert.ok(!hay.includes(n), `${mode}: ${n}`);
    assert.equal(w.q(`SELECT 1 FROM served WHERE reader_id = ?`, U.id).length, 0);
  }
});

test("29b: an import that fails gives back its import allowance", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), R = w.user();
  const a = (await publish(w, A, bankOf([AQ]))).body as any;
  const f = faults(w);
  f.pred = (s) => s.some((x) => x.includes("INSERT") && x.includes("imports"));
  const r1 = await quiet(() => imp(w, R, a.item_id).catch(() => ({ status: 500 })));
  assert.ok(r1.status >= 500);
  assert.equal((await imp(w, R, a.item_id)).status, 200);
  assert.equal(w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'imports'`, R.id)[0].count, 1, "only the success counts");
  assert.equal(code(await w.json<any>("POST", `/items/${"f".repeat(32)}/import`, { user: R })), "404 not_found");
  assert.equal(w.q<any>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'imports'`, R.id)[0].count, 1);
});

test("29b (t2): a trap's count leaves with the request that trapped (flushTrapsLater), the login hasher's too", async () => {
  const w = new World();
  takeWasmTraps();
  const life = { reinit: () => {}, health: () => 1 };
  const g = guardCall(life as any, () => {
    throw new Error("unreachable");
  }, "argon2id");
  await quiet(async () => assert.throws(() => g()));
  const waits: Promise<unknown>[] = [];
  flushTrapsLater(w.db, (p) => waits.push(p));
  await Promise.all(waits);
  const row = w.q<any>(`SELECT value FROM pool_kv WHERE key LIKE 'wasm_traps:%'`);
  assert.deepEqual(row.map((r) => r.value), ["1"]);
  assert.equal(takeWasmTraps(), 0);
});

// ---- 29b t6: migration 0103's order backfill is linear ----------------------------------------------

test("29b (t6_migrate): 0103's seq backfill gives what the correlated form gave, and runs in well under D1's 30 s at 40k publishes", () => {
  const dir = new URL("../../migrations/", import.meta.url);
  const sql0103 = readFileSync(new URL("0103_pool_rounds_6_11.sql", dir), "utf8");
  const start = sql0103.indexOf("WITH pub_ord AS MATERIALIZED");
  const end = sql0103.indexOf("UPDATE pool_seq SET n = MAX(n,");
  assert.ok(start > 0 && end > start);
  const newSql = sql0103.slice(start, end);
  const oldSql = `WITH pub_ord AS (SELECT publish_id, item_id, ROW_NUMBER() OVER (ORDER BY committed_at, created_at, publish_id) AS o FROM item_publishes WHERE committed_at IS NOT NULL)
UPDATE item_questions SET seq = (SELECT MIN(po.o) FROM pub_ord po JOIN item_questions q2 ON q2.publish_id = po.publish_id
   WHERE po.item_id = (SELECT item_id FROM pub_ord WHERE publish_id = item_questions.publish_id) AND q2.qid = item_questions.qid AND q2.match_hash = item_questions.match_hash)
 WHERE seq = 0 AND publish_id IN (SELECT publish_id FROM pub_ord);
UPDATE item_questions SET lineage_seq = seq WHERE lineage_seq IS NULL AND seq > 0;
WITH pub_ord AS (SELECT publish_id, item_id, ROW_NUMBER() OVER (ORDER BY committed_at, created_at, publish_id) AS o FROM item_publishes WHERE committed_at IS NOT NULL)
UPDATE item_blobs SET seq = (SELECT MIN(po.o) FROM pub_ord po JOIN item_blobs b2 ON b2.publish_id = po.publish_id
   WHERE po.item_id = (SELECT item_id FROM pub_ord WHERE publish_id = item_blobs.publish_id) AND b2.sha256 = item_blobs.sha256)
 WHERE seq = 0 AND publish_id IN (SELECT publish_id FROM pub_ord);`;
  const mk = () => {
    const db = new DatabaseSync(":memory:");
    db.exec(`CREATE TABLE item_publishes (publish_id TEXT PRIMARY KEY, item_id TEXT, committed_at INTEGER, created_at INTEGER);
             CREATE TABLE item_questions (publish_id TEXT, qid TEXT, match_hash TEXT, seq INTEGER NOT NULL DEFAULT 0, lineage_seq INTEGER, PRIMARY KEY (publish_id, qid));
             CREATE TABLE item_blobs (publish_id TEXT, form TEXT, sha256 TEXT, seq INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (publish_id, form, sha256));`);
    return db;
  };
  const fill = (db: DatabaseSync, n: number) => {
    db.exec("BEGIN");
    const p = db.prepare(`INSERT INTO item_publishes VALUES (?, ?, ?, ?)`), iq = db.prepare(`INSERT INTO item_questions (publish_id, qid, match_hash) VALUES (?, ?, ?)`), ib = db.prepare(`INSERT INTO item_blobs (publish_id, form, sha256) VALUES (?, ?, ?)`);
    for (let k = 0; k < n; k++) {
      const item = `i${k % Math.max(1, Math.floor(n / 4))}`;
      p.run(`p${k}`, item, k % 13 === 0 ? null : 1000 + ((k * 7919) % n), k);
      for (let j = 0; j < 2; j++) iq.run(`p${k}`, `q${j}`, `m${(k + j) % 5}`);
      ib.run(`p${k}`, "full", `s${k % 3}`);
      if (k % 2) ib.run(`p${k}`, "stripped", `s${k % 3}`);
    }
    db.exec("COMMIT");
  };
  const dump = (db: DatabaseSync) => JSON.stringify([db.prepare(`SELECT * FROM item_questions ORDER BY publish_id, qid`).all(), db.prepare(`SELECT * FROM item_blobs ORDER BY publish_id, form, sha256`).all()]);
  const a = mk(), b = mk();
  fill(a, 400);
  fill(b, 400);
  a.exec(oldSql);
  b.exec(newSql);
  assert.equal(dump(b), dump(a), "the same seqs");
  const big = mk();
  fill(big, 40_000);
  const t0 = performance.now();
  big.exec(newSql);
  const ms = performance.now() - t0;
  // (Round 26: tolerant under load unless POOL_STRICT_TIMING=1; the old form took 165 s here.)
  assert.ok(ms < (process.env.POOL_STRICT_TIMING === "1" ? 3000 : 20000), `${ms.toFixed(0)} ms at 40k publishes`);
  void readdirSync;
});

// ---- differentials: backdating and stock, and the licence model ---------------------------------

const L5 = ["CC0-1.0", "CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"];
function sm(seed: number) {
  let s = Math.imul(seed ^ 0x5bd1e995, 0x27d4eb2d) >>> 0;
  return () => {
    s = (s + 0x9e3779b9) >>> 0;
    let z = s;
    z = Math.imul(z ^ (z >>> 16), 0x85ebca6b) >>> 0;
    z = Math.imul(z ^ (z >>> 13), 0xc2b2ae35) >>> 0;
    return ((z ^ (z >>> 16)) >>> 0) / 2 ** 32;
  };
}
const SEEDS = Number(process.env.POOL_PROPERTY_SEEDS ?? 12);
const START = Number(process.env.POOL_PROPERTY_START ?? 0);

test(`round 22 differential (${SEEDS} seeds): Mal's verbatim copy of Ann's question is allowed exactly under the licences Ann's item had at it since Mal's first serve; socks that backdate or flood Ann's text never free it`, async () => {
  for (let seed = START; seed < START + SEEDS; seed++) {
    const r = sm(seed);
    const pick = <T>(xs: T[]) => xs[Math.floor(r() * xs.length)]!;
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    const AT = mcq("Ann's question: which factor explains the 2019 decline at the Freeport trap?", [["Temperature", true], ["Bass", false]], { explanation: LONG });
    const socks: { u: User; p: any }[] = [];
    for (let k = 0; k < Math.floor(r() * 3); k++) {
      const u = w.user("teacher", `Sock${k}`);
      socks.push({ u, p: (await publish(w, u, bankOf([mcq(`Sock ${k} old filler: is ${k} even?`, [["yes", true], ["no", false]])]), { license: "CC0-1.0" })).body });
    }
    w.advance(DAY);
    let a: any = (await publish(w, A, bankOf([AT]), { license: pick(L5) })).body;
    const lic = () => w.q<any>(`SELECT license FROM items WHERE id = ?`, a.item_id)[0].license as string;
    let served = false;
    const granted = new Set<string>();
    const ops: string[] = [];
    for (let i = 0; i < 4 + Math.floor(r() * 6); i++) {
      w.advance(DAY / 2 + Math.floor(r() * DAY));
      const k = Math.floor(r() * 5);
      if (k === 0) {
        const L = pick(L5);
        const x = await publish(w, A, bankOf(own(a, [AT])), { license: L, base: a.publish_id }, a.item_id);
        if (x.status === 200) {
          a = { ...x.body, item_id: a.item_id };
          if (served) granted.add(L);
        }
        ops.push(`A>${L}`);
      } else if (k === 1) {
        await (r() < 0.5 ? get(w, M, a.item_id) : imp(w, M, a.item_id));
        served = true;
        granted.add(lic());
        ops.push(`M<A@${lic()}`);
      } else if (k === 2 && socks.length > 0) {
        const s = pick(socks);
        await publish(w, s.u, bankOf(own(s.p, [mcq("Sock filler again: is it even?", [["yes", true], ["no", false]], { explanation: LONG })])), { license: "CC0-1.0", base: s.p.publish_id }, s.p.item_id);
        if (r() < 0.5) await get(w, M, s.p.item_id);
        ops.push("sock-backdate");
      } else if (k === 3) {
        const u = w.user("teacher", `Late${i}`);
        const p = await publish(w, u, bankOf([q(`Late copy ${i}: what is ${i}+1?`, { explanation: LONG })]), { license: "CC0-1.0" });
        if (r() < 0.5 && p.status === 201) await get(w, M, p.body.item_id as string);
        ops.push("late-copy");
      } else ops.push("noop");
    }
    if (!served) continue;
    for (const L of L5) {
      const exp = [...granted].some((g) => grantOptions({ item: "", license: g, modified: false }).includes(L as any));
      const meta = { publish_id: publishId(), hide_key: false, license: L, attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: "2026-10-22" }, labels: [], order_acks: [], image_ok: [], visibility: "public", meta: { title: "p", subject: "Bio", course: "", grade: "", tags: [], standards: [] } };
      const got = await w.json<any>("POST", "/items/preview", { user: M, body: framed(meta, bankOf([AT])), contentType: "application/vnd.vershion.publish" });
      assert.equal(got.status === 200, exp, `seed ${seed} ${L}: got ${got.status} ${got.body.error ?? ""}, granted ${[...granted]} ops ${ops.join(" ")}`);
    }
  }
});

test("29a (diff.ts, tails): deleting, purging, erasing, taking down and restoring Ann's item never changes what Mal may do with her question", async () => {
  for (let seed = START; seed < START + Math.max(4, Math.floor(SEEDS / 2)); seed++) {
    const outs: string[][] = [];
    for (const tail of [false, true]) {
      const r = sm(seed);
      const pick = <T>(xs: T[]) => xs[Math.floor(r() * xs.length)]!;
      const w = new World();
      const adm = w.user("admin"), A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
      let a: any = (await publish(w, A, bankOf([AQ]), { license: pick(L5) })).body;
      for (let i = 0; i < 3 + Math.floor(r() * 4); i++) {
        w.advance(DAY);
        if (r() < 0.5) {
          const x = await publish(w, A, bankOf(own(a, [AQ])), { license: pick(L5), base: a.publish_id }, a.item_id);
          if (x.status === 200) a = { ...x.body, item_id: a.item_id };
        } else await (r() < 0.5 ? get(w, M, a.item_id) : imp(w, M, a.item_id));
      }
      const t = Math.floor(r() * 4);
      if (tail) {
        if (t === 0) {
          await w.json("DELETE", `/items/${a.item_id}`, { user: A });
          await days(w, 20);
        } else if (t === 1) {
          const nid = await notice(w, adm);
          await w.takedown<any>(adm, nid, a.item_id);
          w.advance(DAY);
          await w.admin<any>(adm, "POST", "/admin/dmca/withdraw", { notice_id: nid });
        } else if (t === 2) {
          const nid = await notice(w, adm);
          await w.takedown<any>(adm, nid, a.item_id);
          w.advance(DAY);
          await w.json<any>("POST", `/items/${a.item_id}/counter-notice`, { user: A, json: CN("Ann") });
          await days(w, 16);
        } else await days(w, 30);
      }
      const bat: string[] = [];
      for (const L of L5) {
        const meta = { publish_id: publishId(), hide_key: false, license: L, attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: "2026-10-22" }, labels: [], order_acks: [], image_ok: [], visibility: "public", meta: { title: "p", subject: "Bio", course: "", grade: "", tags: [], standards: [] } };
        const g = await w.json<any>("POST", "/items/preview", { user: M, body: framed(meta, bankOf([AQ])), contentType: "application/vnd.vershion.publish" });
        bat.push(`${L}=${g.status === 200 ? "ok" : g.body.error}`);
      }
      outs.push(bat);
    }
    assert.deepEqual(outs[1], outs[0], `seed ${seed}`);
  }
});

void keyedBody;
