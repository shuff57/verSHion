// Rounds 13-14: critics 19b (ASVS) and 19a (Moodle) inverted. The privacy default holds the stem;
// held cap never throttles honest work (B1); order-free identity (B2); earlier identical publications
// (B3); content copies for takedowns and PATCH re-checks (B4); generic reads may be attested (B5);
// lineage walks (B6); block keys of the served form only (B7); soft relabels are unverified (B8);
// and the hardening items (keyed fingerprints, search shortlist, backfill, release expiry).
import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { publish, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const tf = (stem: string) => q(stem, { kind: { Tf: { correct: true } } });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const served = (r: any): Q[] => r.body.payload.bank.questions;
const own = (pub: any, qs: Q[]) => qs.map((x, i) => (i < pub.qids.length ? { ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } } : x));
const out = (r: { status: number; body: Q }) => (r.status === 422 && r.body.error === "license_incompatible" ? `422${JSON.stringify(r.body.allowed)}` : String(r.status));
const holds = (w: World, id: unknown) => w.q<{ reason: string; source: string }>(`SELECT reason, source FROM item_holds WHERE item_id = ? ORDER BY reason, source`, id);
const flags = (w: World, id: unknown) => w.q<{ kind: string }>(`SELECT kind FROM admin_flags WHERE item_id = ? ORDER BY kind`, id).map((x) => x.kind);
const chainOf = async (w: World, u: User, id: unknown) => ((await get(w, u, id as string)).body.questions as Q[]).map((x) => (x.chain as Q[]).map((l) => l.author_name));
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const NCSA = `422${JSON.stringify(["CC-BY-NC-SA-4.0"])}`;
const PII = "Maria Lopez (period 3, IEP for ADHD, lives at 12 Elm St) scored 41 on the unit test: what is her z-score?";

test("19b E2/E2b: a student-data purge holds every question with that stem, existing copies and later ones, whatever the answer", async () => {
  const w = new World();
  const adm = w.user("admin"), V = w.user(), C = w.user(), M = w.user(), R = w.user();
  const v = (await publish(w, V, bankOf([q(PII, { kind: { Short: { sample: "about -1.2" } } })]))).body;
  // An existing copy with its answer edited (before the purge).
  const c = (await publish(w, C, bankOf([q(PII, { kind: { Short: { sample: "-1.23 (rounded)" } } })]))).body;
  const pv = await w.admin<any>(adm, "POST", "/admin/pool/purge-preview", { item_id: v.item_id, questions: [0] });
  assert.equal(pv.body.match[0].scope, "default");
  assert.ok(pv.body.match[0].keys.some((k: string) => k.startsWith("s:")), "the stem alone is a key");
  assert.deepEqual(pv.body.affected.map((a: Q) => a.item_id), [c.item_id], "the edited copy is listed");
  assert.equal((await w.purge(adm, v.item_id as string, { questions: [0] })).status, 200);
  assert.deepEqual(holds(w, c.item_id), [{ reason: "student_data", source: "privacy" }]);
  for (const variant of [q(PII, { kind: { Short: { sample: "-1.3" } } }), q(PII, { explanation: "z = (x - mean)/sd" }), tf(PII), mcq(PII, [["-1.2", true], ["0", false]])]) {
    const r = (await publish(w, M, bankOf([variant]))).body;
    assert.equal((await get(w, R, r.item_id as string)).status, 404, JSON.stringify(variant.kind));
  }
});

test("B1: after a takedown in her favour the rightful author and her honest importer are never charged; nothing unrelated is ever refused", async () => {
  const w = new World();
  const adm = w.user("admin"), R = w.user("teacher", "Rae"), X = w.user(), T = w.user();
  const qs = Array.from({ length: 25 }, (_, i) => q(`Rae's original question ${i} about the ecology of vernal pools in the Central Valley?`, { explanation: `Rae's own explanation ${i}.` }));
  const r = (await publish(w, R, bankOf(qs), { license: "CC-BY-NC-4.0" })).body;
  w.advance(DAY);
  const x = (await publish(w, X, bankOf(qs), { license: "CC0-1.0" })).body;
  w.advance(DAY);
  const tImp = served(await imp(w, T, r.item_id as string));
  assert.equal((await w.takedown(adm, await notice(w, adm), x.item_id, { rightful_item_id: r.item_id, questions: qs.map((_, i) => i) })).status, 200);
  const rp = await publish(w, R, bankOf(own(r, qs.map((x0, i) => (i === 0 ? { ...x0, stem: x0.stem.replace("Valley?", "Valley ?") } : x0)))), { license: "CC-BY-NC-4.0", base: r.publish_id as string }, r.item_id as string);
  assert.equal(rp.status, 200);
  assert.equal((await publish(w, T, bankOf(tImp), { license: "CC-BY-NC-4.0" })).status, 201);
  const used = (u: User) => w.q<{ count: number }>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'held_publishes'`, u.id)[0]?.count ?? 0;
  assert.deepEqual([used(R), used(T)], [0, 0]);
  assert.equal((await publish(w, R, bankOf([q("Rae's brand new question about granite weathering?")]))).status, 201);
});

test("B2 (b2b): reordering choices, pairs or parts never makes a copy something else", async () => {
  const cases: [string, Q, (s: Q) => Q][] = [
    ["mc", mcq("Which factor best explains the 2019 decline of juvenile Chinook salmon at the Freeport trap in Ann's survey?", [["Temperature above 20 C", true], ["Predation by striped bass", false], ["Reduced turbidity", false]], { explanation: "Ann's analysis." }), (s) => ({ ...s, kind: { Mc: { answers: [...s.kind.Mc.answers].reverse() } } })],
    ["matching", q("Match each of Ann's survey sites to its dominant species.", { kind: { Matching: { pairs: [{ left: "Freeport", right: "Chinook" }, { left: "Rio Vista", right: "Striped bass" }, { left: "Antioch", right: "Delta smelt" }], distractors: [] } } }), (s) => ({ ...s, kind: { Matching: { ...s.kind.Matching, pairs: [...s.kind.Matching.pairs].reverse() } } })],
    ["multipart", q("Answer both parts about Ann's Freeport survey.", { kind: { Multipart: { parts: [{ stem: "Which month peaked?", kind: { Tf: { correct: true } } }, { stem: "Was the trap moved in 2018?", kind: { Tf: { correct: false } } }] } } }), (s) => ({ ...s, kind: { Multipart: { parts: [...s.kind.Multipart.parts].reverse() } } })],
  ];
  // (Each over 40 words served, so B5's generic-read exception does not apply.)
  const LONG = " Ann's own long analysis of the 2017 to 2019 temperature logger data at the Freeport trap, with the regression slope she computed and the three survey seasons she compared in detail for this question.";
  for (const [name, Q00, reorder] of cases)
    for (const ind of [false, true]) {
      const Q0 = { ...Q00, explanation: `${Q00.explanation ?? ""}${LONG}` };
      const w = new World();
      const A = w.user(), M = w.user();
      const a = (await publish(w, A, bankOf([Q0]), { license: "CC-BY-NC-SA-4.0" })).body;
      w.advance(DAY);
      const s = served(await get(w, M, a.item_id as string))[0]!;
      const p = await publish(w, M, bankOf([reorder(s)]), { license: "CC0-1.0", ...(ind ? { independent: [a.item_id as string], statement: "I wrote these questions myself." } : {}) });
      assert.equal(out(p), NCSA, `${name} ind=${ind}`);
    }
});

test("B3: Ann's exact content relayed by an account that was never served it: credited as an earlier identical publication, unverified, binding downstream", async () => {
  const Q0 = mcq("In Ann's field study of the delta, which factor best explains the 2019 decline of juvenile Chinook salmon at the Freeport trap?", [["Elevated summer water temperature above 20 C", true], ["Predation by striped bass", false], ["Reduced turbidity", false]], { explanation: "Ann's own analysis of the temperature logger data from 2017-2019, with the regression slope she computed." });
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user(), P = w.user("teacher", "Pip"), T = w.user();
  const a = (await publish(w, A, bankOf([Q0]), { license: "CC-BY-NC-SA-4.0" })).body;
  w.advance(DAY);
  const s = served(await imp(w, M, a.item_id as string));
  w.advance(DAY);
  const p = (await publish(w, P, bankOf(s), { license: "CC0-1.0" })).body;
  const g = await get(w, T, p.item_id as string);
  assert.equal(g.body.license_check.status, "unverified_earlier_match");
  assert.deepEqual(g.body.questions[0].chain.map((l: Q) => [l.author_name, l.earlier_match]), [["Ann", true]]);
  assert.equal(out(await publish(w, T, bankOf(served(await imp(w, T, p.item_id as string))), { license: "CC0-1.0" })), NCSA, "the relay does not launder downstream");
  // Common questions (3 earlier independent publishers) are left alone.
  const w2 = new World();
  const common = q("Name the powerhouse of the cell and describe its main function briefly in two sentences.");
  for (let k = 0; k < 3; k++) await publish(w2, w2.user(), bankOf([common]));
  const late = (await publish(w2, w2.user(), bankOf([common]))).body;
  assert.equal((await get(w2, w2.user(), late.item_id as string)).body.license_check.status, "ok");
});

test("B4 (b12, b5b): takedown copies are found by content across all visibility; PATCH public re-runs block checks; carried material is exempt only for the rightful author", async () => {
  const w = new World();
  const adm = w.user("admin"), X = w.user(), Pu = w.user(), R = w.user();
  const T = q("Copyrighted textbook problem 4.7 about the white-tailed deer population model with harvest?", { explanation: "The textbook's worked solution." });
  const stash = (await publish(w, Pu, bankOf([T]), { visibility: "unlisted" })).body; // pre-staged, no lineage
  const x = (await publish(w, X, bankOf([T]))).body;
  const pv = await w.admin<any>(adm, "POST", "/admin/dmca/takedown-preview", { item_id: x.item_id, questions: [0] });
  assert.deepEqual(pv.body.copies.map((cc: Q) => [cc.item_id, cc.visible]), [[stash.item_id, false]]);
  const td = await w.takedown<any>(adm, await notice(w, adm), x.item_id, { questions: [0] });
  assert.deepEqual((await w.holdCopies<any>(adm, td.body.action_id)).body.held, [stash.item_id]);
  // Without hold-copies: PATCH public and a republish are held by the block.
  const stash2 = (await publish(w, Pu, bankOf([{ ...T, explanation: "Pasted again." }]), { visibility: "unlisted" })).body;
  assert.ok(holds(w, stash2.item_id).some((h) => h.reason === "blocklist"));
  const w2 = new World();
  const adm2 = w2.user("admin"), X2 = w2.user(), P2 = w2.user(), R2 = w2.user();
  const s2 = (await publish(w2, P2, bankOf([T]), { visibility: "unlisted" })).body;
  const x2 = (await publish(w2, X2, bankOf([T]))).body;
  await w2.takedown(adm2, await notice(w2, adm2), x2.item_id, { questions: [0] });
  assert.equal((await w2.json("PATCH", `/items/${s2.item_id}`, { user: P2, json: { visibility: "public" } })).status, 200);
  assert.equal((await get(w2, R2, s2.item_id as string)).status, 404, "PATCH public re-ran the block check");
  void R;
});

test("B5 (b9): a short, generic question merely READ may be attested independent (possible source, claimed_independent); imported it stays refused", async () => {
  const G = mcq("Which of the following is the chemical symbol for sodium?", [["Na", true], ["So", false], ["Sd", false], ["S", false]]);
  for (const how of ["read", "import"]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), T = w.user(), R = w.user();
    const a = (await publish(w, A, bankOf([G]), { license: "CC-BY-NC-SA-4.0" })).body;
    w.advance(DAY);
    if (how === "read") await get(w, T, a.item_id as string);
    else await imp(w, T, a.item_id as string);
    const plain = await publish(w, T, bankOf([G]), { license: "CC-BY-4.0" });
    assert.equal(plain.status, 422, `${how}: without a claim it binds`);
    const p = await publish(w, T, bankOf([G]), { license: "CC-BY-4.0", independent: [a.item_id as string], statement: "I wrote this standard question myself years ago." });
    if (how === "import") {
      assert.equal(p.status, 422);
      continue;
    }
    assert.equal(p.status, 201);
    const g = await get(w, R, p.body.item_id as string);
    assert.equal(g.body.license_check.status, "claimed_independent");
    assert.deepEqual(g.body.questions[0].chain.map((l: Q) => [l.author_name, l.possible_source]), [["Ann", true]]);
    assert.ok(flags(w, p.body.item_id).includes("independent_claim"));
  }
});

test("B6 (b10): a lineage stays within Jaccard 0.6 of its root: a planted stem walked one word at a time never reaches Ann's", async () => {
  const ANN = "In the 2019 Freeport trap survey juvenile Chinook counts fell mainly because lower reach summer water exceeded twenty degrees";
  const PLANT = "Rocks along desert basins erode slowly through wind abrasion over many thousands of seasons while sand grains strike exposed faces daily";
  const w = new World();
  const A = w.user(), M = w.user();
  const plant = (await publish(w, M, bankOf([tf(PLANT)]), { license: "CC0-1.0", visibility: "unlisted" })).body;
  w.advance(DAY);
  const a = (await publish(w, A, bankOf([tf(ANN)]), { license: "CC-BY-NC-SA-4.0" })).body;
  w.advance(DAY);
  await imp(w, M, a.item_id as string);
  const target = ANN.split(" ");
  target[target.length - 1] = "degree";
  const cur = PLANT.split(" ");
  while (cur.length < target.length) cur.push(`x${cur.length}`);
  let pub: any = plant;
  let refused = false;
  for (let i = 0; i < target.length; i++) {
    if (cur[i] === target[i]) continue;
    cur[i] = target[i]!;
    const r = await publish(w, M, bankOf(own(pub, [tf(cur.slice(0, target.length).join(" "))])), { license: "CC0-1.0", base: pub.publish_id }, plant.item_id as string);
    if (r.status !== 200) {
      refused = true;
      assert.equal(out(r), NCSA);
      break;
    }
    pub = { ...r.body, item_id: plant.item_id };
    w.advance(60_000);
  }
  assert.ok(refused, "the walk ends bound by Ann's licence");
});

test("B7 (n1): a hidden-key Blank under a privacy block: every answer guess is held alike (no oracle on the hidden answer)", async () => {
  const w = new World();
  const adm = w.user("admin"), V = w.user(), M = w.user();
  const stem = "Student Jo Smith (ID 4411) in period 2 named the capital of Burkina Faso as ___.";
  const v = (await publish(w, V, bankOf([q(stem, { kind: { Blank: { accept: [["Ouagadougou"]] } } })]), { hideKey: true })).body;
  assert.equal((await w.purge(adm, v.item_id as string, { questions: [0], scopes: { 0: "full" } })).status, 200);
  const held = [];
  for (const g of ["Bamako", "Niamey", "Ouagadougou"]) held.push(holds(w, (await publish(w, M, bankOf([q(stem, { kind: { Blank: { accept: [[g]] } } })]), { hideKey: true })).body.item_id).length > 0);
  assert.ok(held.every((h) => h === held[0]), JSON.stringify(held));
});

test("B8 (b11): a soft relabel reads 'unverified'; the honest downstream importer is told the true bound (Ann's licence)", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), T = w.user(), R = w.user();
  const Q0 = mcq("Which factor best explains the 2019 decline of juvenile Chinook salmon at Ann's Freeport trap site?", [["Temperature", true], ["Predation", false], ["Turbidity", false]], { explanation: "Ann's analysis of logger data." });
  const a = (await publish(w, A, bankOf([Q0]), { license: "CC-BY-NC-SA-4.0" })).body;
  w.advance(DAY);
  const s = served(await get(w, M, a.item_id as string))[0]!;
  const m = (await publish(w, M, bankOf([{ ...s, explanation: `${s.explanation} Mal adds a sentence of his own.` }]), { license: "CC0-1.0" })).body;
  assert.equal((await get(w, R, m.item_id as string)).body.license_check.status, "unverified");
  const t = await publish(w, T, bankOf(served(await imp(w, T, m.item_id as string))), { license: "CC0-1.0" });
  assert.equal(t.status, 422);
  const bound = (t.body.per_question as Q[])[0]!.bound_by as Q[];
  assert.deepEqual(bound.map((b) => [b.author_name, b.license]), [["Ann", "CC-BY-NC-SA-4.0"]]);
});

test("rightful per blocked hash: one question merged in from the infringing copy never disqualifies the rest", async () => {
  const w = new World();
  const adm = w.user("admin"), V = w.user(), X = w.user();
  const own0 = q("Victim's original question about photosystem II water splitting in chloroplasts?", { explanation: "The victim's explanation." });
  const v = (await publish(w, V, bankOf([own0]))).body;
  w.advance(DAY);
  const x = (await publish(w, X, bankOf([own0, q("The infringer's own extra question about rocks?")]))).body;
  w.advance(DAY);
  const ix = served(await imp(w, V, x.item_id as string));
  const vr = await publish(w, V, bankOf(own(v, [own0, { ...ix[1]!, origin: { item_id: x.item_id, qid: "", version: 1 } }])), { base: v.publish_id as string }, v.item_id as string);
  assert.equal(vr.status, 200);
  const pv = await w.admin<any>(adm, "POST", "/admin/dmca/takedown-preview", { item_id: x.item_id, rightful_item_id: v.item_id, questions: [0] });
  assert.deepEqual(pv.body.rightful_check.reasons, []);
});

test("hardening: fingerprints are keyed; the search shortlist never keeps unlisted or held publishes; a failing backfill item is flagged, not retried; a release expires", async () => {
  const w = new World();
  const A = w.user(), B = w.user();
  const p = (await publish(w, A, bankOf([q("Zebrafish fin regeneration question for the keyed fingerprint check?")]))).body;
  const row = w.q<{ near_full: string; bk_full: string }>(`SELECT near_full, bk_full FROM item_questions WHERE publish_id = ?`, p.publish_id)[0]!;
  const { createHash } = await import("node:crypto");
  assert.notEqual(row.near_full, createHash("sha256").update("anything").digest("hex"));
  assert.match(row.bk_full, /^f:[0-9a-f]{64}$/);
  // The shortlist: an unlisted item with the word never reaches non-authors.
  await publish(w, B, bankOf([q("Zebrafish unlisted hidden question?")]), { visibility: "unlisted" });
  const hits = (await w.json<any>("GET", "/items?q=zebrafish", { user: w.user() })).body.items.map((x: Q) => x.id);
  assert.deepEqual(hits, [p.item_id]);
  // ... and the shortlist is taken among shown publishes only, newest first: newer unlisted ones never
  // crowd a public one out of it (shortlist of 2 here).
  const ws = new World({ searchCandidates: 2 });
  const P1 = ws.user();
  const pub1 = (await publish(ws, P1, bankOf([q("Kelpforest public question about otters?")]))).body;
  for (let k = 0; k < 3; k++) {
    ws.advance(1000);
    await publish(ws, ws.user(), bankOf([q(`Kelpforest unlisted question ${k}?`)]), { visibility: "unlisted" });
  }
  assert.deepEqual((await ws.json<any>("GET", "/items?q=kelpforest", { user: ws.user() })).body.items.map((x: Q) => x.id), [pub1.item_id]);
  // Block keys come from the SERVED form: a hidden answer has no "answer" key to block (no oracle).
  const wh = new World();
  const admh = wh.user("admin"), Vh = wh.user();
  const vh = (await publish(wh, Vh, bankOf([q("Name the student in period 2 who failed, in the model answer?", { kind: { Short: { sample: "Jo Smith (ID 4411)" } } })]), { hideKey: true })).body;
  assert.equal((await wh.admin<any>(admh, "POST", "/admin/pool/purge-preview", { item_id: vh.item_id, questions: [0], scopes: { 0: "answer" } })).status, 400);
  // Backfill: an item whose body cannot be read is marked and flagged once.
  const w2 = new World({ backfillItems: 5 });
  const C = w2.user();
  const it = (await publish(w2, C, bankOf([q("Backfill target question?")]))).body;
  w2.sql.prepare(`UPDATE items SET fold_rev = 0 WHERE id = ?`).run(it.item_id);
  w2.sql.prepare(`UPDATE item_publishes SET full_bytes = -1 WHERE item_id = ?`).run(it.item_id); // forces a failure path below
  const orig = w2.r2.get.bind(w2.r2);
  (w2.r2 as any).get = async () => {
    throw new Error("r2 down");
  };
  await w2.pool.hourly();
  (w2.r2 as any).get = orig;
  assert.equal(w2.q<{ f: number }>(`SELECT fold_rev AS f FROM items WHERE id = ?`, it.item_id)[0]!.f, 2);
  assert.equal(w2.q(`SELECT * FROM admin_flags WHERE kind = 'cron_failure' AND item_id = ?`, it.item_id).length, 1);
  await w2.pool.hourly();
  assert.equal(w2.q(`SELECT * FROM admin_flags WHERE kind = 'cron_failure' AND item_id = ?`, it.item_id).length, 1, "not retried");
  // A busy isolate is not the item's fault: no mark, the next run takes it.
  const w4 = new World({ backfillItems: 5 });
  const it4 = (await publish(w4, w4.user(), bankOf([q("Backfill busy question?")]))).body;
  w4.sql.prepare(`UPDATE items SET fold_rev = 0 WHERE id = ?`).run(it4.item_id);
  const { reserveBytes, ISOLATE_BYTES_BUDGET } = await import("../../src/pool/shared.ts");
  const hog = reserveBytes(ISOLATE_BYTES_BUDGET);
  try {
    await w4.pool.hourly();
  } finally {
    hog();
  }
  assert.deepEqual([w4.q<{ f: number }>(`SELECT fold_rev AS f FROM items WHERE id = ?`, it4.item_id)[0]!.f, w4.q(`SELECT * FROM admin_flags WHERE kind = 'cron_failure'`).length], [0, 0]);
  await w4.pool.hourly();
  assert.equal(w4.q<{ f: number }>(`SELECT fold_rev AS f FROM items WHERE id = ?`, it4.item_id)[0]!.f, 1);
  // A release-hold covers 90 days only.
  const w3 = new World();
  const adm3 = w3.user("admin"), S3 = w3.user(), I3 = w3.user();
  const STEM = "Explain your reasoning for the previous answer in two sentences.";
  const inn = (await publish(w3, I3, bankOf([q(STEM, { kind: { Short: { sample: "Innocent." } } })]))).body;
  const s3 = (await publish(w3, S3, bankOf([q(STEM, { kind: { Short: { sample: "Jenny Smith scored 42." } } })]))).body;
  await w3.purge(adm3, s3.item_id as string, { questions: [0] });
  await w3.admin(adm3, "POST", "/admin/pool/release-hold", { item_id: inn.item_id, publish_id: inn.publish_id, source: "privacy", note: "Generic stem, no student data here." });
  assert.deepEqual(holds(w3, inn.item_id), []);
  w3.advance(91 * DAY);
  await publish(w3, I3, bankOf(own(inn, [q(STEM, { kind: { Short: { sample: "Innocent, edited." } } })])), { base: inn.publish_id as string }, inn.item_id as string);
  assert.deepEqual(holds(w3, inn.item_id), [{ reason: "student_data", source: "privacy" }], "after 90 days the release no longer covers it");
});

test("account deletion removes the account's authorship records (round 20 N3: unreachable under any id)", async () => {
  const w = new World();
  const D = w.user(), O = w.user();
  await publish(w, D, bankOf([q("Dana's question about basalt columns?")]));
  await publish(w, O, bankOf([q("Olga's question about granite domes?")]));
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, pw_params = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, D.id);
  const others = w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM author_contents WHERE author_id = ?`, O.id)[0]!.n;
  await w.db.batch(deleteContentStatements(w.db, D.id, w.clock));
  assert.equal(w.q(`SELECT * FROM author_contents WHERE author_id = ? OR author_id LIKE 'deleted:%'`, D.id).length, 0);
  assert.equal(w.q(`SELECT * FROM author_firsts WHERE author_id = ?`, D.id).length, 0);
  assert.equal(w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM author_contents WHERE author_id = ?`, O.id)[0]!.n, others, "nobody else's");
});

test("workerd: content-key subqueries use no compound SELECT (D1 limits compound terms per statement)", async () => {
  const { qKeysOf, inQKeysOf, qKeyIn, qKeyEq } = await import("../../src/pool/shared.ts");
  for (const sql of [qKeysOf("?1"), inQKeysOf("x", "?1"), qKeyIn("iq", "(1)"), qKeyEq("iq", "x")]) assert.doesNotMatch(sql, /\bUNION\b/i);
});
