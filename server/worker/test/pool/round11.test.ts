// Round 11: critics 15a (ASVS) / 15b (Moodle) inverted, and the share-core round-6 adapter. The
// licence cases (P1, P5, P6, P7, the hidden-key adapter) live in licence-cases.test.ts; here: the
// adapter (keyed full_hash, strip_author_data), P2 the bounded fuzzy lookup, P3 hold snapshots and
// the held-publish cap, P4 the rightful item, P8 student-data holds, and the non-blocking items.
import assert from "node:assert/strict";
import { test } from "node:test";
import { deriveHashKey, makeCore, type ShareCoreExports } from "../../src/pool/index.ts";
import { FUZZY_PAIRS_MAX, PUB_BAND_MAX } from "../../src/pool/provenance.ts";
import { bytesReserved } from "../../src/pool/shared.ts";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { genQuestion, publish, rng, shareCore, TEST_HASH_KEY, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const served = (r: any): Q[] => r.body.payload.bank.questions;
const withOrigin = (r: any) =>
  served(r).map((x: Q, i: number) => ({ ...x, origin: { item_id: r.body.item.id, qid: r.body.questions[i].qid, publish_id: r.body.item.publish_id, version: r.body.item.version, author_name: r.body.item.author_name, license: r.body.license, form: r.body.item.form } }));
const holds = (w: World, id: unknown) => w.q<{ reason: string; source: string }>(`SELECT reason, source FROM item_holds WHERE item_id = ? ORDER BY reason, source`, id);
const flags = (w: World, id: unknown) => w.q<{ kind: string }>(`SELECT kind FROM admin_flags WHERE item_id = ? ORDER BY kind`, id).map((x) => x.kind);
const curOf = (w: World, id: unknown) => w.q<{ current_publish_id: string }>(`SELECT current_publish_id FROM items WHERE id = ?`, id)[0]!.current_publish_id;
const mine = async (w: World, u: User, id: unknown) => (await w.json<any>("GET", "/items/mine", { user: u })).body.items.find((i: any) => i.id === id);
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const cron = async (w: World) => {
  const t = w.clock;
  w.clock = Math.floor(t / DAY) * DAY + 4 * 3600_000 + (t % DAY > 4 * 3600_000 ? DAY : 0);
  await w.pool.daily();
};

// ---- the share-core round-6 adapter ------------------------------------------------------------------

test("adapter: publish_prepare (strip inside) and strip_author_data run back to back in one call; two interleaved publishes never swap the served orders", async () => {
  const calls: string[] = [];
  let n = 0;
  // share-core round 8: EVERY export clears the author data, so the adapter must take it with the
  // very next export call after publish_prepare: every export is recorded here.
  const all: string[] = [];
  const rec = Object.fromEntries(
    Object.entries(shareCore as unknown as Record<string, unknown>).map(([k, f]) => [k, typeof f === "function" ? (...a: unknown[]) => (all.push(k), (f as (...x: unknown[]) => unknown)(...a)) : f]),
  );
  const wrapped = {
    ...rec,
    // share-core round 7: the strip runs inside publish_prepare.
    publish_prepare: (b: unknown, o: unknown) => {
      calls.push(`strip:${++n}`);
      all.push("publish_prepare");
      return (shareCore as unknown as Required<ShareCoreExports>).publish_prepare(b as never, o as never);
    },
    strip_author_data: () => {
      calls.push(`data:${n}`);
      all.push("strip_author_data");
      return (shareCore as unknown as ShareCoreExports).strip_author_data!();
    },
  } as unknown as ShareCoreExports;
  const w = new World({ core: makeCore(wrapped, { hashKey: TEST_HASH_KEY }) });
  // Every D1 call yields, so the two publishes interleave at every await.
  const A = w.user(), B = w.user();
  const r = rng(11);
  const bankA = Array.from({ length: 1 }, (_, i) => genQuestion(r, "Mc", i).json);
  const bankB = Array.from({ length: 4 }, (_, i) => genQuestion(r, "Matching", i).json);
  const runs = await Promise.all([
    publish(w, A, JSON.stringify({ schema_version: 2, bank: { id: "a", name: "a", questions: bankA, created: 1, modified: 1 } }), { hideKey: true }),
    publish(w, B, JSON.stringify({ schema_version: 2, bank: { id: "b", name: "b", questions: bankB, created: 1, modified: 1 } }), { hideKey: true }),
  ]);
  for (const x of runs) assert.equal(x.status, 201, JSON.stringify(x.body).slice(0, 300));
  // strip, then its own data, never another strip in between.
  for (let k = 0; k < calls.length; k += 2) assert.deepEqual([calls[k]!.split(":")[0], calls[k + 1]!.split(":")[0], calls[k]!.split(":")[1]], ["strip", "data", calls[k + 1]!.split(":")[1]], calls.join(","));
  assert.ok(calls.length >= 4, "both publishes stripped");
  all.forEach((k, i) => {
    if (k === "publish_prepare") assert.equal(all[i + 1], "strip_author_data", all.join(","));
  });
  assert.ok(all.includes("validate_meta") && all.filter((k) => k === "publish_prepare").length >= 2, all.join(","));
  // Each author's orders are about their own bank (questions 0 / 0..3, Matching only in B's).
  const oa = runs[0]!.body.served_orders as { question: number; matching?: boolean }[];
  const ob = runs[1]!.body.served_orders as { question: number; matching?: boolean }[];
  assert.ok(oa.every((o) => o.question === 0 && !o.matching), JSON.stringify(oa));
  assert.ok(ob.some((o) => o.matching) && ob.every((o) => o.question < 4), JSON.stringify(ob));
  // And the stripped body served to others is the one its own orders describe (a Matching bank, 4 questions).
  assert.equal(served(await get(w, A, runs[1]!.body.item_id as string)).length, 4);
});

test("adapter: no hash key, no hashing; the key is derived per purpose (HKDF) and deterministic", async () => {
  const keyless = makeCore(shareCore, { hashKey: null });
  assert.throws(() => keyless.fullHashes(new TextEncoder().encode(bankOf([q("A question?")]))), /hash_key|not configured/i);
  assert.throws(() => keyless.publishPrepare(new TextEncoder().encode(bankOf([q("A question?")])), { hideKey: false, labels: [], orderAcks: [], imageOk: [] }), /not configured/i);
  await assert.rejects(keyless.contentHashes([]), /not configured/i);
  const k1 = await deriveHashKey("x".repeat(40), "full-hash");
  assert.equal(k1, await deriveHashKey("x".repeat(40), "full-hash"));
  assert.notEqual(k1, await deriveHashKey("x".repeat(40), "other"));
  assert.notEqual(k1, await deriveHashKey("y".repeat(40), "full-hash"));
  assert.match(k1, /^[0-9a-f]{64}$/);
});

// ---- P2 -----------------------------------------------------------------------------------------------

test("P2 (eF, eF2): the fuzzy lookup is bounded and aggregated; evidence rows do not grow for the same content; reservations released", async () => {
  const w = new World();
  const A = w.user("teacher", "Reader"), B = w.user("teacher", "Source");
  const N = 120;
  let seed = 7;
  const rnd = () => ((seed = (seed * 1103515245 + 12345) >>> 0) / 2 ** 32);
  const word = () => Array.from({ length: 6 }, () => "abcdefghijklmnopqrstuvwxyz"[Math.floor(rnd() * 26)]).join("");
  const stems = Array.from({ length: N }, () => Array.from({ length: 12 }, word).join(" ") + "?");
  const b = (await publish(w, B, bankOf(stems.map((s) => q(s))), { license: "CC-BY-NC-SA-4.0" })).body;
  w.advance(DAY);
  await imp(w, A, b.item_id as string);
  const rows0 = w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM import_questions WHERE user_id = ?`, A.id)[0]!.n;
  // Re-reading the same content never grows the evidence (one row per served content).
  for (let k = 0; k < 3; k++) await get(w, A, b.item_id as string);
  assert.equal(w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM import_questions WHERE user_id = ?`, A.id)[0]!.n, rows0);
  // 40 more contents per question (as eF inflates them): the lookup still returns one row per pair.
  // (Round 19: evidence is the reader's served publishes; 39 more served versions of the bank, as
  // ghosts with other contents.)
  for (let k = 1; k < 40; k++)
    w.sql.exec(
      `INSERT INTO ghost_publishes (publish_id, item_id, author_id, hide_key, public_version, license, at) SELECT 'g${k}' || publish_id, item_id, author_id, hide_key, public_version + ${k}, license, ${k} FROM item_publishes WHERE item_id = '${b.item_id}';
       INSERT INTO ghost_questions SELECT 'g${k}' || publish_id, qid, pos, match_hash, full_content || '${k}', stripped_content, keyfree_content, near_full || '${k}', near_stripped, near_kf, near_bare_full, near_bare_stripped, chain_json, seq, stem_hash, bk_expl, bk_ans, NULL, tok_sk, body_sk, qkind, bk_norm, bk_stem, bk_full
         FROM item_questions WHERE publish_id IN (SELECT publish_id FROM item_publishes WHERE item_id = '${b.item_id}');
       INSERT INTO served SELECT reader_id, item_id, 'g${k}' || publish_id, kind, license, licenses_json, first_at, at + ${k}, NULL FROM served WHERE reader_id = '${A.id}' AND item_id = '${b.item_id}' AND publish_id NOT LIKE 'g%';`,
    );
  let fuzzyRows = -1;
  w.stats.log = (sql) => void (/^\s*WITH b AS MATERIALIZED/.test(sql) && fuzzyRows++);
  const sizes: number[] = [];
  const orig = w.sql.prepare.bind(w.sql);
  (w.sql as any).prepare = (sql: string) => {
    const st = orig(sql);
    if (!/^\s*WITH b AS MATERIALIZED \(SELECT CAST\(j.value ->> '\$.pos' AS INTEGER\) AS pos, j.value ->> '\$.mh'/.test(sql)) return st;
    return { all: (...a: any[]) => { const r = st.all(...a) as unknown[]; sizes.push(r.length); return r; }, get: st.get.bind(st), run: st.run.bind(st) };
  };
  // A rewords every question by one word: every position is a fuzzy candidate.
  const p = await publish(w, A, bankOf(stems.map((s) => q(s.replace(/^\w+/, "changed")))), { license: "CC-BY-NC-SA-4.0" });
  (w.sql as any).prepare = orig;
  assert.equal(p.status, 201);
  assert.equal(sizes.length, 1, "one fuzzy query");
  assert.ok(sizes[0]! <= N + 1 && sizes[0]! <= FUZZY_PAIRS_MAX, `aggregated: ${sizes[0]} rows for ${N} questions x 40 contents`);
  assert.ok(flags(w, p.body.item_id).includes("origin_suspect"));
  assert.equal(bytesReserved(), 0, "every reservation released");
  // A stock phrase shared by more than FUZZY_BAND_MAX served stems is skipped and flagged, not enumerated.
  const w2 = new World();
  const A2 = w2.user(), B2 = w2.user();
  // (Round 18: the fuzzy lookup runs over the public band index, whose hot-band bound is PUB_BAND_MAX.)
  const stock = Array.from({ length: PUB_BAND_MAX + 30 }, (_, i) => `Which of the following best describes the process shown in the diagram number ${i}?`);
  const b2 = (await publish(w2, B2, bankOf(stock.map((s) => q(s))))).body;
  w2.advance(DAY);
  await imp(w2, A2, b2.item_id as string);
  const p2 = await publish(w2, A2, bankOf([q("Which of the following best describes the process shown in the diagram number 7 here?")]));
  // (Round 21, critic27a M-B1: past the allowance the candidates left out only bound the licence: a
  // licence they all permit (here CC-BY) is published, with a placeholder link and a flag; one they
  // forbid (CC0) is refused, naming the first.)
  assert.equal(p2.status, 201, JSON.stringify(p2.body).slice(0, 200));
  assert.ok(flags(w2, p2.body.item_id).includes("origin_suspect"));
  const p2c = await publish(w2, A2, bankOf([q("Which of the following best describes the process shown in the diagram number 7 there?")]), { license: "CC0-1.0" });
  assert.deepEqual([p2c.status, p2c.body.error], [422, "license_incompatible"]);
  // A stock phrase only READ is a soft signal: flagged, never refused.
  const w3 = new World();
  const A3 = w3.user(), B3 = w3.user();
  const b3 = (await publish(w3, B3, bankOf(stock.map((s) => q(s))))).body;
  w3.advance(DAY);
  await get(w3, A3, b3.item_id as string);
  const p3 = await publish(w3, A3, bankOf([q("Which of the following best describes the process shown in the diagram number 7 here?")]));
  assert.equal(p3.status, 201);
  assert.ok(flags(w3, p3.body.item_id).includes("evidence_overflow"), "conservative: the admin sees that the lookup was capped");
});

// ---- P3 -----------------------------------------------------------------------------------------------

test("P3 (eA): a held copy's notice lists only what the admin saw; a republish learns nothing; every held publish counts toward 20 a day", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user(), X = w.user(), O = w.user();
  w.history(X);
  const sd1 = "Jenny Smith (ID 40211) in period 3 scored 42 on the test; explain her error.";
  const sd2 = "Marco Diaz (ID 55120) in period 5 scored 61 on the quiz; explain his error.";
  const a = (await publish(w, A, bankOf([q(sd1), q("Alice's volcano question about lava?")]))).body;
  const o = (await publish(w, O, bankOf([q(sd2), q("Other teacher's tide question here?")]))).body;
  w.advance(DAY);
  const r = await imp(w, X, a.item_id as string);
  const x = (await publish(w, X, bankOf([served(r)[0]!, q("X's own filler question one?")]))).body;
  w.advance(DAY);
  assert.equal((await w.purge(adm, a.item_id as string, { questions: [0] })).status, 200);
  assert.equal((await w.purge(adm, o.item_id as string, { questions: [0] })).status, 200);
  const before = await mine(w, X, x.item_id);
  assert.deepEqual(before.positions, [0], "the admin's snapshot");
  assert.equal(before.hold_class, "student_data");
  assert.ok(w.notices.some((n) => n.kind === "copy_held" && n.itemId === x.item_id && n.cause === "student_data"), "the author is told");
  // Probing republish: X keeps the held question and guesses at other texts.
  const guesses = ["Marco Diaz (ID 55119) in period 5 scored 61 on the quiz; explain his error.", sd2, "Marco Diaz (ID 55121) in period 5 scored 61 on the quiz; explain his error."];
  const rp = await publish(w, X, bankOf([served(r)[0]!, ...guesses.map((g) => q(g))]), { base: curOf(w, x.item_id) }, x.item_id as string);
  assert.equal(rp.status, 200);
  const after = await mine(w, X, x.item_id);
  assert.equal(after.positions, undefined, "no per-position information once the content changed");
  assert.equal(after.hold_class, "student_data");
  // Round 12: charged per question that matches a block, whatever the outcome: the held question, the
  // exact guess (sd2), and the two near-rewordings of blocked text.
  assert.equal(w.q<{ count: number }>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'held_publishes'`, X.id)[0]?.count, 4, "the held republish counted per matching question");
  // Past 20 a day (round 14 B1): never a refusal; the matching publishes are held for review (probe_cap).
  let last = rp;
  for (let n = 0; n < 17; n++) last = await publish(w, X, bankOf([served(r)[0]!, q(`Guess ${n} about a student's quiz score?`)]), { base: curOf(w, x.item_id) }, x.item_id as string);
  assert.equal(last.status, 200);
  assert.ok(w.q<{ count: number }>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'held_publishes'`, X.id)[0]!.count >= 20);
  assert.equal(w.q(`SELECT * FROM item_holds WHERE item_id = ? AND source = 'probe_cap'`, x.item_id).length, 1);
});

// ---- P4 -----------------------------------------------------------------------------------------------

test("P4 (eB, s6, s6b): the rightful item must contain every blocked hash in a publish before the item's first; visible; not derived; same check for replaced_by", async () => {
  const w = new World();
  const adm = w.user("admin"), V = w.user("teacher", "Victim"), X = w.user("teacher", "Infringer"), C = w.user("teacher", "Copier"), R = w.user();
  const vq = "Victim's original question about photosynthesis and light energy?";
  const xOld = (await publish(w, X, bankOf([q("X's unrelated old question about rocks and minerals?")]), { license: "CC-BY-NC-SA-4.0" })).body;
  w.advance(DAY);
  const v = (await publish(w, V, bankOf([q(vq)]), { license: "CC-BY-4.0" })).body;
  w.advance(DAY);
  const c = (await publish(w, C, bankOf(served(await imp(w, C, v.item_id as string))), { license: "CC-BY-4.0" })).body;
  // eB: X puts the victim's question into its OLD item and names it rightful.
  const rx = await get(w, X, v.item_id as string);
  assert.equal((await publish(w, X, bankOf([served(rx)[0]!, q("X's unrelated old question about rocks and minerals?")]), { base: curOf(w, xOld.item_id), license: "CC-BY-NC-SA-4.0", independent: [v.item_id as string] }, xOld.item_id as string)).status, 200);
  const pv = await w.admin<any>(adm, "POST", "/admin/dmca/takedown-preview", { item_id: v.item_id, rightful_item_id: xOld.item_id, questions: [0] });
  assert.equal(pv.status, 200);
  assert.equal(pv.body.rightful_check.ok, false);
  // (Round 12 M3: X read V after its old item existed, so that read does not count as derivation; the
  // per-hash order alone refuses it.)
  assert.deepEqual(pv.body.rightful_check.reasons, ["not_earlier_in_rightful"]);
  const h0 = pv.body.rightful_check.hashes[0];
  assert.ok(h0.rightful_seq > h0.item_seq, "the preview shows, per blocked hash, the earliest publish in each");
  const td = await w.takedown<any>(adm, await notice(w, adm), v.item_id, { rightful_item_id: xOld.item_id, questions: [0] });
  assert.equal(td.status, 409);
  assert.equal(td.body.error, "rightful_item_invalid");
  // The victim's copy and the victim are untouched.
  assert.deepEqual((await get(w, R, c.item_id as string)).body.license_check, { status: "ok", basis: "recorded_sources" });
  // s6b: an unrelated unlisted earlier item is refused (not visible, has none of the material).
  const un = (await publish(w, w.user(), bankOf([q("Unrelated unlisted question about clouds?")]), { visibility: "unlisted" })).body;
  const t2 = await w.takedown<any>(adm, await notice(w, adm), xOld.item_id, { rightful_item_id: un.item_id, questions: [0] });
  assert.equal(t2.status, 409);
  assert.deepEqual(t2.body.reasons, ["rightful_not_visible", "not_earlier_in_rightful"]);
  // The valid case: the victim's item is rightful for X's copy.
  const t3 = await w.takedown<any>(adm, await notice(w, adm), xOld.item_id, { rightful_item_id: v.item_id, questions: [0] });
  assert.equal(t3.status, 200);
  // replaced_by in /admin/pool/evidence: the same validation.
  const w2 = new World();
  const adm2 = w2.user("admin"), V2 = w2.user(), X2 = w2.user();
  const v2 = (await publish(w2, V2, bankOf([q(vq)]))).body;
  w2.advance(DAY);
  const x2 = (await publish(w2, X2, bankOf(served(await get(w2, X2, v2.item_id as string))))).body;
  const bad = await w2.admin<any>(adm2, "POST", "/admin/pool/evidence", { item_id: v2.item_id, action: "void", replaced_by: x2.item_id });
  assert.equal(bad.status, 409, "the later copy is never the rightful stand-in of the original");
  assert.equal((await w2.admin<any>(adm2, "POST", "/admin/pool/evidence", { item_id: x2.item_id, action: "void", replaced_by: v2.item_id })).status, 200);
});

// ---- P8 -----------------------------------------------------------------------------------------------

test("P8 (s7): a student_data hold is never auto-restored, whatever placed it; one overdue flag per appeal", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), Rep = w.user("teacher", "Reporter"), R = w.user();
  w.history(Rep);
  const a = (await publish(w, A, bankOf([q("Jenny Smith (ID 40211) in period 3 scored 42 on the test today."), q("Ann's other question about volcanoes?")]))).body;
  assert.equal((await w.json("POST", `/items/${a.item_id}/report`, { user: Rep, json: { reason: "student_data", note: "a real student's name and id" } })).status, 202);
  assert.deepEqual(holds(w, a.item_id), [{ reason: "student_data", source: "" }]);
  assert.equal((await w.json("POST", `/items/${a.item_id}/appeal`, { user: A, json: { note: "fictional" } })).status, 202);
  for (let i = 0; i < 62; i++) {
    w.advance(DAY);
    await cron(w);
  }
  assert.deepEqual(holds(w, a.item_id), [{ reason: "student_data", source: "" }], "still held after 60 days");
  assert.equal((await get(w, R, a.item_id as string)).status, 404);
  assert.equal(flags(w, a.item_id).filter((k) => k === "appeal_overdue").length, 1, "one overdue flag, kept up to date");
  assert.ok(flags(w, a.item_id).includes("appeal_escalated"));
  assert.ok(!flags(w, a.item_id).includes("appeal_auto_restored"));
});

// ---- non-blocking ---------------------------------------------------------------------------------------

test("a copy held after a takedown is released when its author republishes without the blocked material (flagged); the author is told", async () => {
  const w = new World();
  const adm = w.user("admin"), X = w.user(), C = w.user("teacher", "Cal");
  const bad = q("Copyrighted textbook question about the Calvin cycle regeneration step?");
  const x = (await publish(w, X, bankOf([bad, q("X's own question about rivers?")]))).body;
  w.advance(DAY);
  const c = (await publish(w, C, bankOf([...withOrigin(await imp(w, C, x.item_id as string)).slice(0, 1), q("Cal's own question about lakes?")]))).body;
  const td = await w.takedown<any>(adm, await notice(w, adm), x.item_id, { questions: [0] });
  assert.equal(td.status, 200);
  assert.equal((await w.holdCopies(adm, td.body.action_id)).status, 200);
  assert.equal(holds(w, c.item_id).length, 1);
  assert.ok(w.notices.some((n) => n.kind === "copy_held" && n.itemId === c.item_id && n.cause === "copyright"));
  const m = await mine(w, C, c.item_id);
  assert.equal(m.hold_class, "copyright", "the notice says what the hold is about");
  assert.match(m.notice, /copyright takedown/);
  // Still containing it: still held.
  const still = await publish(w, C, bankOf([served(await get(w, C, c.item_id as string))[0]!, q("Cal's new question about seas?")]), { base: curOf(w, c.item_id) }, c.item_id as string);
  assert.equal(still.status, 200);
  assert.ok(holds(w, c.item_id).length >= 1, "still held");
  // Without it: released, and the admin sees it.
  const clean = await publish(w, C, bankOf([q("Cal's own question about lakes?")]), { base: curOf(w, c.item_id) }, c.item_id as string);
  assert.equal(clean.status, 200);
  assert.deepEqual(holds(w, c.item_id), []);
  assert.ok(flags(w, c.item_id).includes("hold_cleared"));
});

test("search: question text from the SERVED form only (never a hidden key); licence filter; browse carries license_check", async () => {
  const w = new World();
  const A = w.user(), B = w.user(), R = w.user();
  const a = (await publish(w, A, bankOf([q("Explain photosynthesis in green plants", { kind: { Short: { sample: "zebrafish chlorophyll answer" } } })]), { hideKey: true, license: "CC-BY-4.0" })).body;
  const b = (await publish(w, B, bankOf([q("Describe the zebra migration routes")]), { license: "CC0-1.0" })).body;
  const ids = async (qs: string) => ((await w.json<any>("GET", `/items?${qs}`, { user: R })).body.items as Q[]).map((x) => x.id);
  assert.deepEqual(await ids("q=photosyn"), [a.item_id], "a word start of the served stem");
  assert.deepEqual(await ids("q=green%20photo"), [a.item_id], "every word must start a word");
  assert.deepEqual(await ids("q=zebrafish"), [], "the hidden sample is never searchable");
  assert.deepEqual(await ids("q=chlorophyll"), []);
  assert.deepEqual(await ids("q=zebra"), [b.item_id]);
  assert.deepEqual(await ids("license=CC0-1.0"), [b.item_id]);
  assert.equal((await w.json<any>("GET", "/items?license=MIT", { user: R })).status, 400);
  const page = (await w.json<any>("GET", "/items", { user: R })).body.items as Q[];
  assert.ok(page.every((x) => x.license_check?.basis === "recorded_sources"));
  assert.equal(w.q(`SELECT * FROM served_words WHERE word LIKE 'zebrafish%' OR word LIKE 'chlorophyll%'`).length, 0, "the full form is never indexed");
});

test("account deletion: one anonymous key per account (import counts stay right); others' chains are rewritten by the cron in bounded batches", async () => {
  const w = new World({ rewriteBatch: 2, rewriteBatches: 1 });
  const A = w.user("teacher", "Ann"), D = w.user("teacher", "Dana");
  const a = (await publish(w, A, bankOf([q("Ann's question about granite?")]))).body;
  await imp(w, D, a.item_id as string);
  // Ann republishes and Dana imports again: two import rows of one account (eN).
  const a2 = await publish(w, A, bankOf([q("Ann's question about granite, edited?")]), { base: a.publish_id as string }, a.item_id as string);
  assert.equal(a2.status, 200);
  await imp(w, D, a.item_id as string);
  assert.equal(w.q(`SELECT * FROM imports WHERE user_id = ? AND kind = 'import'`, D.id).length, 2);
  const d = (await publish(w, D, bankOf([q("Dana's question about basalt?")]))).body;
  const copies: string[] = [];
  for (let k = 0; k < 5; k++) {
    const u = w.user("teacher", `C${k}`);
    copies.push((await publish(w, u, bankOf(withOrigin(await imp(w, u, d.item_id as string))))).body.item_id as string);
  }
  const ic0 = (await mine(w, A, a.item_id)).import_count;
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, pw_params = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, D.id);
  const stmts = deleteContentStatements(w.db, D.id, w.clock);
  assert.ok(stmts.length < 40, `a bounded number of statements (${stmts.length}), none per row`);
  await w.db.batch(stmts);
  assert.equal((await mine(w, A, a.item_id)).import_count, ic0, "the serves of one account count once");
  const named = () => w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM item_questions WHERE instr(chain_json, '"Dana"') > 0`)[0]!.n;
  assert.equal(named(), 5);
  let runs = 0;
  while (named() > 0 && runs < 20) {
    await w.pool.hourly();
    runs++;
  }
  assert.equal(named(), 0, "rewritten");
  assert.ok(runs > 1, "in more than one bounded run");
  assert.equal(w.q(`SELECT * FROM deletion_rewrites WHERE done_at IS NULL`).length === 0 || runs < 20, true);
});

test("a kept qid keeps its seq across content changes with the same match_hash: credit order is unaffected", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), C = w.user("teacher", "Cat"), R = w.user();
  const stem = "Which gas do plants take in for photosynthesis?";
  const a = (await publish(w, A, bankOf([q(stem, { explanation: "Ann's." })]))).body;
  w.advance(DAY);
  const b = (await publish(w, B, bankOf([q(stem, { explanation: "Bob's." })]))).body;
  w.advance(DAY);
  const order = async () => {
    const ia = await imp(w, C, a.item_id as string);
    await imp(w, C, b.item_id as string);
    const c = await publish(w, C, bankOf([{ ...served(ia)[0]!, explanation: "Cat's own." }]));
    return ((await get(w, R, c.body.item_id as string)).body.questions[0].chain as Q[]).map((l) => l.author_name);
  };
  const first = await order();
  assert.deepEqual(first, ["Ann", "Bob"]);
  // Ann edits the explanation (same match_hash): her qid keeps its first publish.
  const seq0 = w.q<{ seq: number }>(`SELECT seq FROM item_questions WHERE publish_id = ?`, a.publish_id)[0]!.seq;
  const kept = { ...served(await get(w, A, a.item_id as string))[0]!, explanation: "Ann's, edited.", published: { item_id: a.item_id, qid: (a.qids as string[])[0], publish_id: a.publish_id, version: a.version } };
  const ra = await publish(w, A, bankOf([kept]), { base: a.publish_id as string }, a.item_id as string);
  assert.equal(ra.status, 200);
  assert.equal(w.q<{ seq: number }>(`SELECT seq FROM item_questions WHERE publish_id = ?`, ra.body.publish_id)[0]!.seq, seq0);
  w.advance(DAY);
  assert.deepEqual(await order(), ["Ann", "Bob"], "still Ann first");
});

test("EXPLAIN QUERY PLAN: no full table scan on publish, republish, read, import, browse (search, licence) and /items/mine", async () => {
  const w = new World();
  const users = Array.from({ length: 6 }, (_, i) => w.user("teacher", `U${i}`));
  const items: string[] = [];
  for (const [i, u] of users.entries()) for (let k = 0; k < 3; k++) items.push((await publish(w, u, bankOf([q(`Question ${i} ${k} about rivers and lakes?`), q(`Other ${i} ${k} about mountains?`)]))).body.item_id as string);
  for (const u of users.slice(0, 3)) for (const it of items.slice(6, 12)) await imp(w, u, it);
  const tables = new Set(w.q<{ name: string }>(`SELECT name FROM sqlite_master WHERE type = 'table'`).map((x) => x.name));
  // (Round 19, critic24d D3: aliases declared inside the views count too, so a scan of served or
  // item_publishes behind import_questions / pub_questions is caught.)
  const views = w.q<{ name: string; sql: string }>(`SELECT name, sql FROM sqlite_master WHERE type = 'view'`);
  const scans: string[] = [];
  let phase = "";
  w.stats.log = (sql, args) => {
    if (!/^\s*(SELECT|WITH|INSERT|UPDATE|DELETE)/i.test(sql)) return;
    const plan = w.sql.prepare(`EXPLAIN QUERY PLAN ${sql}`).all(...(args as never[])) as { detail: string }[];
    for (const r of plan) {
      const m = /^SCAN (\w+)(?: USING (?:COVERING )?INDEX (\w+))?/.exec(r.detail);
      if (!m || /VIRTUAL TABLE/.test(r.detail)) continue;
      const alias = m[1]!;
      // A real table: scanned by its own name, or by an alias declared as "<table> <alias>".
      const text = sql + views.filter((v) => new RegExp(`\\b${v.name}\\b`).test(sql)).map((v) => v.sql).join(" ");
      const real = tables.has(alias) || [...tables].some((t) => new RegExp(`(FROM|JOIN)\\s+${t}\\s+(AS\\s+)?${alias}\\b`, "i").test(text));
      if (!real) continue;
      // Browse pages along its partial index (ORDER BY created_at, LIMIT): the one allowed index scan.
      if (m[2] === "items_browse" && phase.startsWith("browse")) continue;
      scans.push(`${phase}: ${r.detail} :: ${sql.replace(/\s+/g, " ").slice(0, 120)}`);
    }
  };
  phase = "publish";
  const p = await publish(w, users[0]!, bankOf([...withOrigin(await imp(w, users[0]!, items[7]!)), q("New one about deserts?")]));
  phase = "republish";
  await publish(w, users[0]!, bankOf([q("Changed one about deserts?")]), { base: p.body.publish_id as string }, p.body.item_id as string);
  phase = "read";
  await get(w, users[4]!, items[2]!);
  phase = "import";
  await imp(w, users[5]!, items[3]!);
  phase = "browse";
  await w.json("GET", "/items?q=rivers", { user: users[5]! });
  phase = "browse-license";
  await w.json("GET", "/items?license=CC-BY-4.0", { user: users[5]! });
  phase = "mine";
  await w.json("GET", "/items/mine", { user: users[0]! });
  w.stats.log = undefined;
  assert.deepEqual(scans, []);
});
