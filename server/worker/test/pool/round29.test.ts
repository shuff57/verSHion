// Round 29: a hidden-key Matching bank of 3.81 MB (990 questions x 50 pairs, stems of 4+ words) was
// refused 503 busy even on an idle isolate: the publish reserved 16x its body for the heavy phase and
// its fuzzy lookup reserved its worst case (2,000 rows x 8 KB) ON TOP, past the 72 MB budget, while the
// payload trees were still alive. Now the trees (and the sketch memo) go before the evidence and fuzzy
// lookups, and the lookups borrow the part of the heavy reservation the publish provably no longer
// uses (shared.ts lender, publish.ts postHeavyBytes); the rest is reserved from the isolate as before.
import assert from "node:assert/strict";
import { test } from "node:test";
import { FUZZY_PAIRS_MAX, FUZZY_ROW_BYTES, stemBands, stemTokens } from "../../src/pool/provenance.ts";
import { MAX_TAGS, postHeavyBytes } from "../../src/pool/publish.ts";
import { bytesReserved, ISOLATE_BYTES_BUDGET, lender, MB, PoolError, PUBLISH_COST_PER_BYTE, reserveBytes } from "../../src/pool/shared.ts";
import { core, enc, publish, World } from "./harness.ts";

type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const base = (i: number, stem: string, kind: Q, extra: Q = {}): Q => ({ id: `q${i}`, stem, kind, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
/** The staging repro's shape: 990 Matching questions of 50 pairs, every stem 4+ words (so it has MinHash bands), 3.81 MB. */
const matchingBank = (tag: string) =>
  bankOf(
    Array.from({ length: 990 }, (_, i) =>
      base(i, `${tag} question ${i} asks you to match these words carefully`, {
        Matching: { pairs: Array.from({ length: 50 }, (_, j) => ({ left: `L${i}x${j} alpha ${"l".repeat(12)}`, right: `R${i}x${j} bravo ${"r".repeat(12)}` })), distractors: [] },
      }, { explanation: `Because ${tag} ${i} is matched the way the course matched it (see unit three).` }),
    ),
  );
const isBusy = (r: { status: number; body: Q }) => r.status === 503 && r.body.error === "busy";
const FUZZY_WORST = FUZZY_PAIRS_MAX * FUZZY_ROW_BYTES;

/** Watches every D1 statement: the most reserved at once, and the reservations seen at the fuzzy lookups. */
function watch(w: World, at?: (sql: string) => void) {
  const seen = { max: 0, fuzzy: 0, evidence: 0 };
  w.stats.log = (sql) => {
    seen.max = Math.max(seen.max, bytesReserved());
    if (/^\s*WITH b AS MATERIALIZED/.test(sql)) seen.fuzzy++;
    if (/^\s*WITH subj AS MATERIALIZED/.test(sql)) seen.evidence++;
    at?.(sql);
  };
  return seen;
}

test("the 3.81 MB hidden-key Matching bank (990 x 50 pairs, 4+-word stems) is accepted on an idle isolate; its fuzzy lookups run inside what the publish holds; the budget returns to zero", async () => {
  const bank = matchingBank("Idle");
  const bytes = enc(bank).length;
  assert.ok(bytes > 3.8 * MB && bytes < 3.85 * MB, `the repro's size: ${(bytes / MB).toFixed(2)} MB`);
  const qs = JSON.parse(bank).bank.questions as Q[];
  assert.equal(qs.length, 990);
  assert.ok(qs.every((q) => q.kind.Matching.pairs.length === 50 && stemBands(stemTokens(q.stem)).length > 0), "every stem has MinHash bands (4+ words): the fuzzy lookups run");
  // The old accounting: 16x the body for the whole publish, and the fuzzy worst case on top.
  assert.ok(PUBLISH_COST_PER_BYTE * bytes + FUZZY_WORST > ISOLATE_BYTES_BUDGET, "this bank was over the budget before round 29");
  const w = new World();
  const A = w.user();
  const seen = watch(w);
  const r = await publish(w, A, bank, { hideKey: true });
  w.stats.log = undefined;
  assert.equal(r.status, 201, JSON.stringify(r.body).slice(0, 300));
  assert.ok(seen.fuzzy >= 2 && seen.evidence >= 1, `the evidence and fuzzy lookups ran (${seen.evidence}, ${seen.fuzzy})`);
  assert.ok(seen.max <= ISOLATE_BYTES_BUDGET, `never more than the budget: ${seen.max}`);
  // The lookups borrowed: nothing was reserved beyond the request body and the heavy phase (16x the
  // framed body; its meta is a few KB), where the fuzzy worst case alone would have added 16 MB.
  assert.ok(seen.max <= PUBLISH_COST_PER_BYTE * (bytes + 256 * 1024), `peak ${(seen.max / MB).toFixed(1)} MB`);
  assert.equal(bytesReserved(), 0, "every reservation released after a success");
  assert.ok(w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM stem_bands WHERE item_id = ?`, r.body.item_id)[0]!.n > 0, "the item's stem bands are indexed");
});

test("two concurrent 3.81 MB Matching publishes: one is accepted, the other is shed with 503 busy by the byte budget, never overcommitted; zero after", async () => {
  // A long heavy-slot wait, so the second is refused by the byte budget (not by the queue's timeout).
  const w = new World({ heavyQueueMs: 300_000 });
  const A = w.user(), B = w.user();
  const seen = watch(w);
  const runs = await Promise.all([publish(w, A, matchingBank("ConcA"), { hideKey: true }), publish(w, B, matchingBank("ConcB"), { hideKey: true })]);
  w.stats.log = undefined;
  const st = runs.map((x) => x.status).sort();
  assert.deepEqual(st, [201, 503], JSON.stringify(runs.map((x) => [x.status, x.body.error, x.body.message])));
  const shed = runs.find((x) => x.status === 503)!;
  assert.ok(isBusy(shed));
  assert.equal(shed.body.message, "The server is busy; try again shortly.", "the byte budget refused it (not the heavy queue)");
  assert.ok(seen.max <= ISOLATE_BYTES_BUDGET, `never more than the budget: ${seen.max}`);
  assert.equal(bytesReserved(), 0, "every reservation released, the success's and the refusal's");
});

test("lent bytes never return to the isolate: with the rest of the budget taken mid-publish, the 3.81 MB bank still completes on its own reservation", async () => {
  const w = new World();
  const A = w.user();
  let hog: (() => void) | null = null;
  let hogBytes = 0;
  const seen = watch(w, (sql) => {
    // At the first evidence query (the payload trees are gone): another request takes everything left.
    if (!hog && /^\s*WITH subj AS MATERIALIZED/.test(sql)) {
      hogBytes = ISOLATE_BYTES_BUDGET - bytesReserved();
      hog = reserveBytes(hogBytes);
      assert.throws(() => reserveBytes(1), (e: unknown) => e instanceof PoolError && e.status === 503);
    }
  });
  const r = await publish(w, A, matchingBank("Pressed"), { hideKey: true });
  w.stats.log = undefined;
  assert.ok(hog && hogBytes > 0, "the budget was filled during the lookups");
  assert.equal(r.status, 201, JSON.stringify(r.body).slice(0, 300));
  assert.equal(bytesReserved(), hogBytes, "the publish released all it held; the other request keeps its own");
  assert.ok(seen.fuzzy >= 2, "the fuzzy lookups ran with the isolate full");
  assert.ok(seen.max <= ISOLATE_BYTES_BUDGET);
  hog!();
  assert.equal(bytesReserved(), 0);
});

test("a publish with no spare (its holdings estimate is above its heavy reservation) still reserves its lookups from the isolate: refused 503 busy when full, everything released", async () => {
  // 1,000 small Short questions, every stem 4+ distinct words: 15x a 0.3 MB body is far below what
  // 1,000 questions hold after the heavy phase, so nothing is lent.
  const qs = Array.from({ length: 1000 }, (_, i) => base(i, `Spare ${i} w${i}a w${i}b w${i}c w${i}d?`, { Short: { sample: `s${i}` } }));
  const bank = bankOf(qs);
  const bytes = enc(bank).length;
  assert.ok(postHeavyBytes({ bodies: 2 * bytes, metaBytes: 0, questions: 1000, parts: 0, words: 0, wordChars: 0 }) > (PUBLISH_COST_PER_BYTE - 1) * bytes);
  const w = new World();
  const A = w.user();
  let hog: (() => void) | null = null;
  watch(w, (sql) => {
    if (!hog && /^\s*WITH subj AS MATERIALIZED/.test(sql)) hog = reserveBytes(ISOLATE_BYTES_BUDGET - bytesReserved());
  });
  // The first evidence query's own reservation was taken before the hook; the next one finds the isolate full.
  const r = await publish(w, A, bank, { hideKey: true });
  w.stats.log = undefined;
  assert.ok(isBusy(r), JSON.stringify(r.body).slice(0, 200));
  assert.ok(hog);
  hog!();
  assert.equal(bytesReserved(), 0, "every reservation released after the refusal");
  // ... and the same publish on an idle isolate goes through.
  assert.equal((await publish(w, A, bank, { hideKey: true })).status, 201);
  assert.equal(bytesReserved(), 0);
});

test("a publish refused AFTER its lookups borrowed (too many tags, at staging) releases everything", async () => {
  const qs = Array.from({ length: MAX_TAGS + 10 }, (_, i) => base(i, `Tagged question ${i} about the cell membrane`, { Tf: { correct: true } }, { topics: [`topic ${i}`] }));
  const w = new World();
  const A = w.user();
  const seen = watch(w);
  const r = await publish(w, A, bankOf(qs), { hideKey: true });
  w.stats.log = undefined;
  assert.equal(r.status, 422);
  assert.equal(r.body.error, "too_many_tags");
  assert.ok(seen.fuzzy >= 1 && seen.evidence >= 1, "refused after the lookups");
  assert.equal(bytesReserved(), 0);
});

test("lender: borrows the spare first and reserves only the rest; the spare is never freed to others; release returns it (idempotent); a refused reservation borrows nothing", () => {
  assert.equal(bytesReserved(), 0);
  const held = reserveBytes(40 * MB); // the holder's own reservation (a publish's heavy phase)
  const lend = lender(10 * MB);
  const r1 = lend(6 * MB);
  assert.equal(bytesReserved(), 40 * MB, "fully borrowed: nothing more reserved");
  const r2 = lend(6 * MB);
  assert.equal(bytesReserved(), 42 * MB, "4 MB borrowed, 2 MB reserved");
  // The borrowed bytes are still reserved: no one else can take them.
  assert.throws(() => reserveBytes(ISOLATE_BYTES_BUDGET - 42 * MB + 1), (e: unknown) => e instanceof PoolError && e.status === 503 && e.code === "busy");
  r2();
  r2();
  assert.equal(bytesReserved(), 40 * MB, "release is idempotent");
  // The isolate is full: a borrow within the spare still works; beyond it, 503 busy and nothing borrowed.
  const hog = reserveBytes(ISOLATE_BYTES_BUDGET - 40 * MB);
  const r3 = lend(4 * MB);
  assert.throws(() => lend(1 * MB), (e: unknown) => e instanceof PoolError && e.status === 503);
  r3();
  r1();
  const r4 = lend(10 * MB);
  assert.equal(bytesReserved(), ISOLATE_BYTES_BUDGET, "the whole spare is lendable again");
  r4();
  hog();
  held();
  assert.equal(bytesReserved(), 0);
  // A negative spare lends nothing.
  const none = lender(-5);
  const r5 = none(1024);
  assert.equal(bytesReserved(), 1024);
  r5();
  assert.equal(bytesReserved(), 0);
});

test("share-core adapter: the stripped form keeps only its bytes (its JSON text is decoded on demand, never kept as a copy)", () => {
  const p = core.publishPrepare(enc(bankOf([base(0, "Is this statement about cells true?", { Tf: { correct: true } })])), { hideKey: true, labels: [], orderAcks: [], imageOk: [] });
  assert.ok(!("errors" in p), JSON.stringify(p).slice(0, 300));
  const st = (p as { stripped: Record<string, unknown> }).stripped;
  const d = Object.getOwnPropertyDescriptor(st, "json")!;
  assert.equal(typeof d.get, "function", "an accessor, not a decoded copy");
  assert.equal(st.json, new TextDecoder().decode(st.bytes as Uint8Array));
});
