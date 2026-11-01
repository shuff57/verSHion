// share-core round 8 in the Worker: refusals carry their previews inline (no second call), key_in_stem
// reaches the author only, Matching left columns are served shuffled (`left_order`), the author's
// served orders always describe the body actually served (also a reused one), and `dropped` is real.
import assert from "node:assert/strict";
import { test } from "node:test";
import * as shareCore from "../../../share-core/pkg/share_core.js";
import { makeCore, type ShareCoreExports } from "../../src/pool/core.ts";
import { publish, TEST_HASH_KEY, World } from "./harness.ts";

type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mc = (stem: string, texts: string[], c: number, extra: Q = {}) => q(stem, { kind: { Mc: { answers: texts.map((text, i) => ({ text, correct: i === c, locked: false })) } }, ...extra });
const mt = (pairs: [string, string][], extra: Q = {}) => q("Match each country to its capital city in Alice's list.", { kind: { Matching: { pairs: pairs.map(([left, right]) => ({ left, right })), distractors: ["Bern"] } }, ...extra });
const get = (w: World, u: { id: string }, id: string) => w.json<any>("GET", `/items/${id}`, { user: u as never });
const served = (r: any): Q[] => r.body.payload.bank.questions;
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
const PAIRS: [string, string][] = [["France", "Paris"], ["Peru", "Lima"], ["Chad", "N'Djamena"], ["Fiji", "Suva"], ["Laos", "Vientiane"]];

test("round 8: a refusal carries formatting_findings / order_preview inline, without a second share-core call; key_in_stem is author-only and never ackable", async () => {
  const calls: string[] = [];
  const rec = Object.fromEntries(
    Object.entries(shareCore as unknown as Record<string, unknown>).map(([k, f]) => [k, typeof f === "function" ? (...a: unknown[]) => (calls.push(k), (f as (...x: unknown[]) => unknown)(...a)) : f]),
  ) as unknown as ShareCoreExports;
  const w = new World({ core: makeCore(rec, { hashKey: TEST_HASH_KEY }) });
  const A = w.user();
  const cases: [string, Q][] = [
    ["answer_line", mc("What is the capital of France? ANSWER: B", ["Rome", "Paris", "Oslo"], 1)],
    ["gift", q("The sky is {=blue ~red} on a clear day.", { kind: { Short: { sample: "blue" } } })],
    ["tf_value", q("Water boils at 100 C at sea level (True)", { kind: { Tf: { correct: true } } })],
  ];
  for (const [reason, x] of cases)
    for (const hideKey of [true]) {
      calls.length = 0;
      const r = await publish(w, A, bankOf([x]), { hideKey });
      assert.equal(r.status, 422, `${reason} hide=${hideKey}`);
      const e = (r.body.errors as Q[]).find((y) => y.code === "key_in_stem")!;
      assert.deepEqual([e.reason, e.ack_allowed, e.author_only, typeof e.hint, typeof e.message], [reason, false, true, "string", "string"], reason);
      assert.ok(Array.isArray(r.body.formatting_findings) && typeof r.body.formatting_findings_total === "number" && Array.isArray(r.body.order_preview), reason);
      assert.deepEqual(calls.filter((k) => k === "publish_prepare").length, 1, calls.join(","));
      assert.ok(!calls.includes("formatting_findings") && !calls.includes("label_findings"), calls.join(","));
      // An acknowledgement does not lift it.
      const ack = await publish(w, A, bankOf([x]), { hideKey, ackFormatting: [0] });
      assert.equal(ack.status, 422, `ack ${reason} hide=${hideKey}`);
    }
  // A Matching left item naming its own right item.
  const lr = await publish(w, A, bankOf([mt([["France (Paris)", "Paris"], ["Peru", "Lima"], ["Chad", "N'Djamena"]])]), { hideKey: true });
  assert.equal(lr.status, 422);
  assert.ok((lr.body.errors as Q[]).some((y) => y.code === "key_in_stem" && y.reason === "left_names_right"), JSON.stringify(lr.body.errors));
  // Nothing of a refusal is stored (no item, no audit naming the reason).
  assert.equal(w.q(`SELECT * FROM items`).length, 0);
  assert.equal(w.q(`SELECT * FROM pool_audit WHERE detail LIKE '%key_in_stem%' OR detail LIKE '%answer_line%'`).length, 0);
});

test("round 8: Matching left items are served shuffled; the author's served_orders give left_order (served left i = pairs[left_order[i]]) and the bank order", async () => {
  const w = new World();
  const A = w.user(), R = w.user();
  let shuffledSeen = false;
  for (let k = 0; k < 6; k++) {
    const a = await publish(w, A, bankOf([mt(PAIRS)]), { hideKey: true });
    assert.equal(a.status, 201, JSON.stringify(a.body).slice(0, 300));
    const so = (a.body.served_orders as Q[]).find((o) => o.matching)!;
    assert.ok(Array.isArray(so.left_order) && so.left_order.length === PAIRS.length, JSON.stringify(so));
    const s = served(await get(w, R, a.body.item_id as string))[0]!.kind.Matching;
    assert.deepEqual(s.pairs.map((p: Q) => p.left), so.left_order.map((i: number) => PAIRS[i]![0]));
    // The bank: order[k] indexes the author's right column (rights, then distractors).
    const right = [...PAIRS.map((p) => p[1]), "Bern"];
    assert.deepEqual(s.distractors, so.order.map((i: number) => right[i]));
    assert.ok(s.pairs.every((p: Q) => p.right === ""), "no pairing served");
    if (so.left_order.some((v: number, i: number) => v !== i)) shuffledSeen = true;
  }
  assert.ok(shuffledSeen, "the left column is shuffled");
  // keep_order: the left column stays as authored, and no left_order.
  const kept = await publish(w, A, bankOf([mt(PAIRS, { keep_order: true })]), { hideKey: true });
  assert.equal(kept.status, 201);
  const ks = (kept.body.served_orders as Q[]).find((o) => o.matching);
  assert.ok(!ks || ks.left_order === undefined, JSON.stringify(ks));
  assert.deepEqual(served(await get(w, R, kept.body.item_id as string))[0]!.kind.Matching.pairs.map((p: Q) => p.left), PAIRS.map((p) => p[0]));
});

test("round 8: a reused stripped body (hidden key change only) comes with served_orders that describe IT, for Mc and Matching", async () => {
  for (let trial = 0; trial < 4; trial++) {
    const w = new World();
    const A = w.user(), R = w.user();
    const T = ["Mercury", "Jupiter", "Mars", "Venus", "Saturn"];
    const v1 = [mc("Which of these planets in Alice's own list is largest by radius overall?", T, 1), mt(PAIRS)];
    const a = (await publish(w, A, bankOf(v1), { hideKey: true })).body as any;
    assert.ok(a.item_id, JSON.stringify(a).slice(0, 300));
    // The key changes (Mc correct choice; Matching pairing), nothing served does.
    const swapped: [string, string][] = PAIRS.map(([l], i) => [l, PAIRS[(i + 1) % PAIRS.length]![1]]);
    const v2 = [mc("Which of these planets in Alice's own list is largest by radius overall?", T, 4), mt(swapped)];
    const r2 = await publish(w, A, bankOf(own(a, v2)), { hideKey: true, base: a.publish_id }, a.item_id);
    assert.equal(r2.status, 200);
    assert.equal(r2.body.public_version, 1, "reused");
    const s = served(await get(w, R, a.item_id));
    const so = r2.body.served_orders as Q[];
    const m = so.find((o) => !o.matching && o.question === 0)!;
    assert.deepEqual(s[0]!.kind.Mc.answers.map((x: Q) => x.text), m.order.map((i: number) => T[i]));
    const mm = so.find((o) => o.matching)!;
    assert.deepEqual(s[1]!.kind.Matching.pairs.map((p: Q) => p.left), mm.left_order.map((i: number) => swapped[i]![0]));
    const right = [...swapped.map((p) => p[1]), "Bern"];
    assert.deepEqual(s[1]!.kind.Matching.distractors, mm.order.map((i: number) => right[i]));
  }
});

test("round 8: with keep_order a reordered Matching left column IS a served change (no reuse); without it, it is not", async () => {
  for (const keep of [true, false]) {
    const w = new World();
    const A = w.user();
    const a = (await publish(w, A, bankOf([mt(PAIRS, keep ? { keep_order: true } : {})]), { hideKey: true })).body as any;
    const r2 = await publish(w, A, bankOf(own(a, [mt([...PAIRS].reverse(), keep ? { keep_order: true } : {})])), { hideKey: true, base: a.publish_id }, a.item_id);
    assert.equal(r2.status, 200);
    assert.equal(r2.body.public_version, keep ? 2 : 1, `keep=${keep}`);
  }
});

test("round 8: `dropped` (share-core's notes) reaches the author on a hidden-key publish too", async () => {
  const w = new World();
  const A = w.user();
  // A Word Symbol-font character (U+F0E0) is mapped and reported (symbol_font_mapped).
  for (const hideKey of [false, true]) {
    const r = await publish(w, A, bankOf([q("Which arrow  points right in Alice's diagram?")]), { hideKey });
    assert.equal(r.status, 201, JSON.stringify(r.body).slice(0, 300));
    assert.ok((r.body.dropped as Q[]).some((d) => d.reason === "symbol_font_mapped" && typeof d.message === "string"), JSON.stringify(r.body.dropped));
  }
});
