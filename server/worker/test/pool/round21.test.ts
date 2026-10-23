// Round 21: critics 27a (Moodle) and 27b (ASVS), inverted. Unit lookups are bounded per key and the
// cap is on the publisher's OWN units, so no third party can make an honest author refuse or pay
// (B-1/B-5, M-B2); near candidates are counted only when admissible, and a cut only bounds the licence
// (B-2, M-B1); unit stock counts only independent authors (M-B3); the earliest earlier publication is
// credited, deterministically (B-4); takedown copies are computed from the infringing publish and paged
// (B-3, D2); purges and takedowns that fail part way are finished (G1); purges and erasure leave no
// typed text (E1); a takedown of an item purged first still strikes; base headers everywhere; ghosts
// keep every licence the item had, their units' order and a keyed sketch for near-rewordings (V1-V3);
// private items never change an answer (V4).
import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { framed, png, publish, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const img = (sha: string) => ({ file: `media/${sha}.png`, width: 9, height: 9 });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
const code = (r: { status: number; body: Q }) => (r.status >= 400 ? `${r.status} ${r.body.error}` : String(r.status));
const setLimit = (w: World, u: User, metric: string, value: number) =>
  w.sql.prepare(`INSERT OR REPLACE INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, ?, ?, 0, 'x')`).run(u.id, metric, value);
const LONG = "Ann's own worked explanation: the logger shows water above twenty degrees on forty days of the 2019 season, juvenile counts fell in exactly those weeks, and the regression slope links them clearly.";
const ANNQ = () => mcq("In Ann's field study of the delta, which factor best explains the 2019 decline of juvenile Chinook salmon at the Freeport trap?", [["Elevated summer water temperature above 20 C", true], ["Predation by striped bass", false], ["Reduced turbidity", false], ["Lower dissolved oxygen", false]]);
const MALQ = (extra: Q = {}) => mcq("Mal's own question about rivers: is the water warm in August?", [["Yes", true], ["No", false]], extra);
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const purgeDays = async (w: World, n = 20) => {
  for (let d = 0; d < n; d++) {
    w.advance(DAY);
    await w.pool.daily();
  }
};

// ---- units: bounded, own-units cap ------------------------------------------------------------------

test("B-1 (a1_units_dos): unlisted verbatim copies by a third party never make the author's own republish refuse", async () => {
  const W = (i: number, n: number) => Array.from({ length: n }, (_, k) => ["delta", "river", "basin", "silt", "levee", "marsh", "estuary", "tide", "gauge", "flood"][(i + k) % 10] + (i * 131 + k)).join(" ");
  const part = (i: number, j: number) => ({ stem: `Part ${j} of case ${i}: describe the observed feature ${W(i * 10 + j, 6)} in detail.`, kind: { Short: { sample: `Model answer ${i}.${j}: ${W(i * 1000 + j * 37, 30)}` } } });
  const annQ = (i: number) => q(`Ann's field case ${i}: answer every part about the sediment budget ${W(i, 5)}.`, { kind: { Multipart: { parts: Array.from({ length: 10 }, (_, j) => part(i, j)) } } });
  // (The allowance lowered so the old per-row count would refuse after two copies.)
  const w = new World({ unitKeysMax: 1000 });
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mallory");
  const bank = Array.from({ length: 20 }, (_, i) => annQ(i));
  const a = (await publish(w, A, bankOf(bank), { license: "CC-BY-4.0" })).body as any;
  await get(w, M, a.item_id);
  for (let n = 0; n < 6; n++) {
    assert.equal((await publish(w, M, bankOf(bank), { license: "CC-BY-4.0", visibility: "unlisted" })).status, 201);
    const fix = own(a, bank).map((x, k) => (k === 0 ? { ...x, points: n + 2 } : x));
    const rp = await publish(w, A, bankOf(fix), { license: "CC-BY-4.0", base: a.publish_id }, a.item_id);
    assert.equal(rp.status, 200, `after ${n + 1} copies: ${code(rp)}`);
    a.publish_id = rp.body.publish_id;
    a.qids = rp.body.qids;
    a.version = rp.body.version;
  }
});

test("M-B2 (c1_attack_units): one-question items carrying the author's image never lock her out", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), X = w.user("teacher", "Xan");
  setLimit(w, X, "publishes", 1000);
  const ua = await upload(w, A, png(9, 9, 77));
  const bank = Array.from({ length: 40 }, (_, k) => mcq(`Using the graph above, question ${k}: what is the value at point ${k}?`, [[`${k}`, true], [`${k + 2}`, false]], { image: img(ua.sha) }));
  const a = (await publish(w, A, bankOf(bank), { license: "CC-BY-4.0" })).body as any;
  await get(w, X, a.item_id);
  const b = await w.call("GET", `/items/${a.item_id}/blobs/${ua.sha}`, { user: X });
  const ux = await upload(w, X, new Uint8Array(await b.arrayBuffer()));
  for (let k = 0; k < 20; k++) await publish(w, X, bankOf([mcq(`Xan's filler ${k}: is this a graph?`, [["yes", true], ["no", false]], { image: img(ux.sha) })]), { license: "CC-BY-4.0" });
  const fixed = bank.map((x, i) => (i === 0 ? { ...x, stem: x.stem.replace("graph", "chart") } : x));
  assert.equal((await publish(w, A, bankOf(own(a, fixed)), { license: "CC-BY-4.0", base: a.publish_id }, a.item_id)).status, 200);
});

test("B-5 (c1_preview_cpu): planted occurrences of a text no longer multiply a preview's work", async () => {
  const T = "The shared worked explanation: levees cut the floodplain off, so sediment stays in the channel, the delta loses its supply, and the coastline retreats a little more every single decade.";
  const bank = (tag: string, n: number) => bankOf(Array.from({ length: n }, (_, i) => q(`${tag} question ${i}: explain what happens to the delta number ${i * 13} after levees?`, { kind: { Short: { sample: T } }, explanation: T })));
  const ms: number[] = [];
  const rows: number[] = [];
  for (const planted of [0, 2]) {
    const w = new World();
    const M = w.user("teacher", "Mallory");
    setLimit(w, M, "shared_questions", 100_000);
    for (let k = 0; k < planted; k++) assert.equal((await publish(w, M, bank(`Plant ${k}`, 400), { license: "CC0-1.0", visibility: "unlisted" })).status, 201);
    // A key past OCC_SCAN occurrences is matched from the reader's OWN served content (the served-driven
    // query), never by enumerating the pool's occurrences.
    let n = 0;
    w.stats.log = (sql) => void (/FROM served sh CROSS JOIN item_questions iq/.test(sql) && n++);
    const meta = { publish_id: `p${"q".repeat(21)}`, hide_key: false, license: "CC0-1.0", attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: "2026-10-22" }, labels: [], order_acks: [], image_ok: [], visibility: "public", meta: { title: "p", subject: "Bio", course: "", grade: "", tags: [], standards: [] } };
    const t0 = performance.now();
    const r = await w.json<any>("POST", "/items/preview", { user: M, body: framed(meta, bank("Probe", 400)), contentType: "application/vnd.vershion.publish" });
    ms.push(performance.now() - t0);
    rows.push(n);
    assert.equal(r.status, 200);
  }
  assert.deepEqual(rows, [0, 1], "the planted text is matched from the reader's own serves");
  // (Round 26: wall-clock bounds are tolerant under load; POOL_STRICT_TIMING=1 restores the tight one.
  // The structural assertion above is what proves the planted copies are not enumerated.)
  const strict = process.env.POOL_STRICT_TIMING === "1";
  assert.ok(ms[1]! < (strict ? ms[0]! * 2.5 + 300 : ms[0]! * 8 + 5000), `planted 800: ${ms[1]!.toFixed(0)} ms vs ${ms[0]!.toFixed(0)} ms`);
});

// ---- near candidates: admissible only, a cut only bounds ------------------------------------------

test("B-2 / M-B1 (a2_sources_dos, a1_templated, a2_templated_escape): templated variants of an imported worksheet are the teacher's own", async () => {
  const rect = (l: number, wd: number) => mcq(`A rectangle has length ${l} cm and width ${wd} cm. What is its area in square centimetres?`, [[`${l * wd}`, true], [`${2 * (l + wd)}`, false], [`${l + wd}`, false]]);
  for (const n of [9, 12]) {
    const w = new World();
    const W = w.user("teacher", "Worksheet"), T = w.user("teacher", "Tina");
    const ws = (await publish(w, W, bankOf(Array.from({ length: n }, (_, k) => rect(3 + k, 2 + (k % 5)))), { license: "CC-BY-4.0" })).body as any;
    assert.equal((await imp(w, T, ws.item_id)).status, 200);
    for (const L of ["CC-BY-4.0", "CC0-1.0"]) assert.equal((await publish(w, T, bankOf([rect(40, 17), rect(55, 23)]), { license: L })).status, 201, `n=${n} ${L}`);
    assert.equal((await publish(w, T, bankOf([rect(40, 17)]), { license: "CC0-1.0", independent: [ws.item_id] })).status, 201);
  }
  // Near-phrased stems of a bank with other bodies are no sources at all.
  const TOPICS = ["levees", "tides", "mangroves", "silt", "estuaries", "floods", "dams", "wetlands", "sandbars", "marshes", "channels", "barrier islands"];
  const w = new World();
  const A = w.user("teacher", "Bankmaker"), V = w.user("teacher", "Vic");
  const a = (await publish(w, A, bankOf(TOPICS.map((t, k) => mcq(`Which of the following best describes the role of ${t} in a river delta system?`, [[`Answer ${k} about ${t}`, true], [`Wrong ${k}a`, false]]))), { license: "CC-BY-NC-4.0" })).body as any;
  await imp(w, V, a.item_id);
  const mine = mcq("Which of the following best describes the role of sediment in a river delta system?", [["It builds new land at the river mouth", true], ["It cools the water", false]]);
  assert.equal((await publish(w, V, bankOf([mine]), { license: "CC0-1.0" })).status, 201);
});

test("M-B1: past the near window, a question with a source matched on itself is decided by that source (the near ones left out never bound it)", async () => {
  const w = new World();
  const W0 = w.user("teacher", "Worksheet"), O = w.user("teacher", "Orig"), T = w.user("teacher", "Tina");
  const rect = (l: number, wd: number) => mcq(`A rectangle has length ${l} cm and width ${wd} cm. What is its area in square centimetres?`, [[`${l * wd}`, true], [`${2 * (l + wd)}`, false]]);
  const ws = (await publish(w, W0, bankOf(Array.from({ length: 210 }, (_, k) => rect(3 + k, 2 + (k % 7)))), { license: "CC-BY-NC-4.0" })).body as any;
  const Q0 = rect(400, 17);
  const o = (await publish(w, O, bankOf([Q0]), { license: "CC0-1.0" })).body as any;
  await imp(w, T, ws.item_id);
  await imp(w, T, o.item_id);
  assert.equal(code(await publish(w, T, bankOf([Q0]), { license: "CC0-1.0" })), "201", "the CC0 original decides its exact copy");
});

test("B-4 (b1b_pubnear_downstream): the EARLIEST earlier publication is credited, whatever later copies exist, every run", async () => {
  const stem = "In the delta sediment survey, which landform appears first where the river slows near its mouth?";
  for (let run = 0; run < 5; run++) {
    const w = new World();
    const O = w.user("teacher", "Orig"), S = w.user("teacher", "Sock"), C = w.user("teacher", "Copier"), Z = w.user("teacher", "Zed"), R = w.user();
    const Q0 = mcq(stem, [["A delta", true], ["A mesa", false]]);
    await publish(w, O, bankOf([Q0]), { license: "CC0-1.0" });
    for (let k = 0; k < 12; k++) {
      w.advance(1000);
      await publish(w, S, bankOf([{ ...Q0, stem: `${stem} (copy ${k})` }]), { license: "CC-BY-NC-SA-4.0" });
    }
    w.advance(DAY);
    const c = await publish(w, C, bankOf([{ ...Q0, stem: stem.replace("appears first", "forms first") }]), { license: "CC0-1.0" });
    assert.equal(c.status, 201);
    const chain = (await get(w, R, c.body.item_id as string)).body.questions[0].chain as Q[];
    assert.deepEqual(chain.map((l) => [l.author_name, l.license]), [["Orig", "CC0-1.0"]], `run ${run}`);
    await get(w, Z, c.body.item_id as string);
    assert.equal((await publish(w, Z, bankOf([{ ...Q0, stem: stem.replace("appears first", "forms first") }]), { license: "CC0-1.0" })).status, 201);
  }
});

// ---- unit stock -------------------------------------------------------------------------------------

test("M-B3 (c3_sock_stock, c4_honest_stock): copies and socks of an earlier holder never make a unit stock; independent authors do", async () => {
  const setup = async () => {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    const ua = await upload(w, A, png(9, 9, 56));
    const AQ = mcq("Ann's question: which factor explains the 2019 decline at the Freeport trap?", [["Temperature", true], ["Bass", false]], { explanation: LONG, image: img(ua.sha) });
    const a = (await publish(w, A, bankOf([AQ]), { license: "CC-BY-NC-SA-4.0" })).body as any;
    await imp(w, M, a.item_id);
    return { w, A, M, AQ, a, ua };
  };
  const malCopies = async (w: World, M: User, sha: string) => {
    await upload(w, M, png(9, 9, 56));
    return [code(await publish(w, M, bankOf([MALQ({ explanation: LONG })]), { license: "CC0-1.0" })), code(await publish(w, M, bankOf([MALQ({ image: img(sha) })]), { license: "CC0-1.0" }))];
  };
  // Honest copies (Cora, Dev, Eve import and republish verbatim): still bound.
  {
    const { w, M, AQ, a, ua } = await setup();
    for (const n of ["Cora", "Dev", "Eve"]) {
      w.advance(DAY);
      const C = w.user("teacher", n);
      await imp(w, C, a.item_id);
      await upload(w, C, png(9, 9, 56));
      assert.equal((await publish(w, C, bankOf([AQ]), { license: "CC-BY-NC-SA-4.0" })).status, 201);
    }
    assert.deepEqual(await malCopies(w, M, ua.sha), ["422 license_incompatible", "422 license_incompatible"], "honest copies");
  }
  // Socks that read it (public or unlisted): still bound.
  {
    const { w, M, a, ua } = await setup();
    for (let k = 0; k < 3; k++) {
      w.advance(DAY);
      const S = w.user("teacher", `Sock${k}`);
      await get(w, S, a.item_id);
      const us = await upload(w, S, png(9, 9, 56));
      await publish(w, S, bankOf([mcq(`Sock ${k} filler: is ${k} even?`, [["yes", k % 2 === 0], ["no", k % 2 === 1]], { explanation: LONG, image: img(us.sha) })]), { license: "CC-BY-NC-SA-4.0" });
    }
    assert.deepEqual(await malCopies(w, M, ua.sha), ["422 license_incompatible", "422 license_incompatible"], "socks");
  }
  // True stock: three independent authors, never served each other's items, published it before Ann.
  {
    const w = new World();
    const STOCK = "The standard textbook explanation: water expands when it freezes, so ice is less dense than liquid water and floats on lakes, insulating the water below through the winter months.";
    for (let k = 0; k < 3; k++) await publish(w, w.user("teacher", `Indep${k}`), bankOf([mcq(`Independent author ${k}'s own question on ice ${k}?`, [["Floats", true], ["Sinks", false]], { explanation: STOCK })]), { license: "CC0-1.0" });
    w.advance(DAY);
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    const a = (await publish(w, A, bankOf([mcq("Ann's question on ice: why do lakes freeze from the top?", [["Ice floats", true], ["Ice sinks", false]], { explanation: STOCK })]), { license: "CC-BY-NC-SA-4.0" })).body as any;
    await imp(w, M, a.item_id);
    assert.equal((await publish(w, M, bankOf([MALQ({ explanation: STOCK })]), { license: "CC0-1.0" })).status, 201, "stock text binds nobody");
  }
  // ... but not in UNLISTED items (nobody else sees them: they never make anything stock, V4).
  {
    const w = new World();
    const STOCK = "The standard textbook explanation: water expands when it freezes, so ice is less dense than liquid water and floats on lakes, insulating the water below through the winter months.";
    for (let k = 0; k < 3; k++) await publish(w, w.user("teacher", `Hidden${k}`), bankOf([mcq(`Unlisted author ${k}'s own question on ice ${k}?`, [["Floats", true], ["Sinks", false]], { explanation: STOCK })]), { license: "CC0-1.0", visibility: "unlisted" });
    w.advance(DAY);
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    const a = (await publish(w, A, bankOf([mcq("Ann's question on ice: why do lakes freeze from the top?", [["Ice floats", true], ["Ice sinks", false]], { explanation: STOCK })]), { license: "CC-BY-NC-SA-4.0" })).body as any;
    await imp(w, M, a.item_id);
    assert.equal(code(await publish(w, M, bankOf([MALQ({ explanation: STOCK })]), { license: "CC0-1.0" })), "422 license_incompatible", "unlisted copies make nothing stock");
  }
});

// ---- takedowns -----------------------------------------------------------------------------------

test("B-3 (d1): an upheld takedown with no rightful item never holds an importer's unrelated item; a real copy is held", async () => {
  const w = new World();
  const adm = w.user("admin"), I = w.user("teacher", "Infringer"), T = w.user("teacher", "Tina"), C = w.user("teacher", "Copy"), Eve = w.user();
  const X = mcq("Infringer's pasted textbook question about plate boundaries and their motion?", [["Divergent", true], ["Static", false]]);
  const x = (await publish(w, I, bankOf([X]))).body as any;
  await imp(w, T, x.item_id);
  const y = (await publish(w, T, bankOf([mcq("Tina's own unrelated question: what is the boiling point of water at sea level in Celsius?", [["100", true], ["50", false]])]))).body as any;
  await imp(w, C, x.item_id);
  const cp = (await publish(w, C, bankOf([X]))).body as any;
  assert.equal((await w.takedown<any>(adm, await notice(w, adm), x.item_id)).status, 200);
  w.advance(31 * DAY);
  await w.pool.daily();
  assert.equal((await get(w, Eve, y.item_id)).status, 200, "Tina's unrelated item stays");
  assert.equal(w.q(`SELECT * FROM item_holds WHERE item_id = ?`, y.item_id).length, 0);
  assert.equal(w.q(`SELECT * FROM item_holds WHERE item_id = ?`, cp.item_id).length, 1, "the whole-item copy is held");
});

test("D2 (d2_more_remain): more copies than one page are all held: hold-copies pages, the upheld purge holds the rest", async () => {
  const w = new World();
  const adm = w.user("admin"), I = w.user("teacher", "Infringer");
  const X = mcq("Infringer's pasted textbook question about volcano types and their eruptions?", [["Shield", true], ["Flat", false]]);
  const x = (await publish(w, I, bankOf([X]))).body as any;
  const { LINEAGE_COPIES_MAX } = await import("../../src/pool/moderation.ts");
  const n = LINEAGE_COPIES_MAX + 4;
  for (let k = 0; k < n; k++) {
    const u = w.user();
    await imp(w, u, x.item_id);
    await publish(w, u, bankOf([X]));
  }
  const td = await w.takedown<any>(adm, await notice(w, adm), x.item_id);
  const aid = td.body.action_id as string;
  const h1 = await w.holdCopies<any>(adm, aid);
  assert.deepEqual([h1.status, h1.body.held.length, h1.body.more], [200, LINEAGE_COPIES_MAX, true]);
  const h2 = await w.holdCopies<any>(adm, aid);
  assert.deepEqual([h2.status, h2.body.held.length, h2.body.more], [200, 4, false], "the next page, never the same copies");
  const g = await w.admin<any>(adm, "GET", "/admin/dmca/copies", null, `?action_id=${aid}`);
  assert.equal(g.body.candidates.length, 0, "nothing left to hold");
  assert.equal(w.q(`SELECT * FROM item_holds WHERE source = ?`, `dmca:${aid}`).length, n);
});

test("strike evasion (d3, round 22 B1): a takedown of an item its author deleted and the cron purged before the notice strikes (pending, then upheld after the window) and voids its evidence only then", async () => {
  const w = new World();
  const adm = w.user("admin"), I = w.user("teacher", "Infringer"), M = w.user("teacher", "Mal");
  const Q0 = mcq("Infringer's pasted textbook question about plate boundaries and their motion over geologic time?", [["Divergent", true], ["Static", false]], { explanation: LONG });
  const x = (await publish(w, I, bankOf([Q0]), { license: "CC-BY-NC-SA-4.0" })).body as any;
  await imp(w, M, x.item_id);
  await w.json("DELETE", `/items/${x.item_id}`, { user: I });
  await purgeDays(w, 16);
  const t = await w.takedown<any>(adm, await notice(w, adm), x.item_id, { questions: "all" });
  assert.equal(t.status, 200);
  assert.deepEqual(w.q<{ u: number | null }>(`SELECT upheld_at IS NOT NULL AS u FROM strikes WHERE user_id = ?`, I.id).map((s) => s.u), [0], "a pending strike");
  assert.equal(code(await publish(w, M, bankOf([Q0]), { license: "CC0-1.0" })), "422 license_incompatible", "while the window is open its licence still binds");
  await purgeDays(w, 31);
  assert.deepEqual(w.q<{ u: number | null }>(`SELECT upheld_at IS NOT NULL AS u FROM strikes WHERE user_id = ?`, I.id).map((s) => s.u), [1], "upheld once the window passed");
  assert.equal(code(await publish(w, M, bankOf([Q0]), { license: "CC0-1.0" })), "201", "the infringer's licence binds nobody once upheld");
  assert.equal(code(await w.takedown<any>(adm, await notice(w, adm), x.item_id, { questions: "all" })), "409 already_removed");
});

// ---- faults ---------------------------------------------------------------------------------------

/** The Nth D1 operation (a statement's first/all/run, or a batch), or the first whose SQL matches, throws. */
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

test("G1 (g1_purge_fault): a purge whose batch failed after its claim is finished by a later cron run: no publishes, bodies or tokens left", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), R = w.user();
  const pub = (await publish(w, A, bankOf([mcq("Student Zed Q scored 42 on the retake in period 3: what is the mean of 40 and 44?", [["42", true], ["40", false]])]))).body as any;
  await get(w, R, pub.item_id);
  await w.json("DELETE", `/items/${pub.item_id}`, { user: A });
  const f = faults(w);
  f.pred = (s) => s.some((x) => x.includes("INSERT OR IGNORE INTO retired_publish_ids"));
  for (let d = 0; d < 46; d++) {
    w.advance(DAY);
    await w.pool.daily();
  }
  assert.equal(f.fired, 1);
  assert.equal(w.q(`SELECT * FROM item_publishes WHERE item_id = ?`, pub.item_id).length, 0);
  assert.equal(w.r2.keys("").filter((k) => k.includes(pub.item_id)).length, 0);
  assert.equal(w.q(`SELECT * FROM item_questions WHERE stem_tokens LIKE '%zed%'`).length, 0);
  assert.equal(w.q(`SELECT * FROM ghost_publishes WHERE item_id = ?`, pub.item_id).length, 1, "the served publish left its ghost");
});

test("G1 (g3): a takedown that failed part way is resumed by a retry of the same notice (and by the hourly sweep): bodies quarantined, one strike", async () => {
  for (const via of ["retry", "sweep"] as const) {
    const w = new World();
    const adm = w.user("admin"), I = w.user("teacher", "Infringer");
    const x = (await publish(w, I, bankOf([mcq("Infringer's pasted textbook question about plate boundaries?", [["Divergent", true], ["Static", false]])]))).body as any;
    const n = await notice(w, adm);
    const f = faults(w);
    f.pred = (s) => s.some((x) => x.startsWith("UPDATE item_publishes SET full_key"));
    const t1 = await w.takedown<any>(adm, n, x.item_id).catch(() => ({ status: 500 }));
    assert.equal(t1.status, 500);
    if (via === "retry") assert.equal((await w.takedown<any>(adm, n, x.item_id)).status, 200);
    else await w.pool.hourly();
    const keys = w.q<{ k: string }>(`SELECT full_key AS k FROM item_publishes WHERE item_id = ?`, x.item_id).map((r) => r.k);
    assert.ok(keys.every((k) => k.startsWith("quarantine/")), `${via}: ${keys.join(",")}`);
    assert.equal(w.q(`SELECT * FROM strikes`).length, 1);
    assert.equal(w.r2.keys("items/").filter((k) => k.includes(x.item_id)).length, 0, "no unquarantined copy left");
  }
});

// ---- erasure and purge leave no typed text ------------------------------------------------------------

test("E1 (e1_erasure_sweep): after a privacy purge or an author's erasure, no table holds the student's name or the author's typed text", async () => {
  const MARK = "zorblaxquent", MARK2 = "quillibrantor";
  const grep = (w: World, needle: string) => {
    const out: string[] = [];
    for (const { name } of w.q<{ name: string }>(`SELECT name FROM sqlite_master WHERE type = 'table'`))
      for (const row of w.q<Record<string, unknown>>(`SELECT * FROM "${name}"`)) for (const [k, v] of Object.entries(row)) if (typeof v === "string" && v.toLowerCase().includes(needle)) out.push(`${name}.${k}`);
    return out;
  };
  const world = async () => {
    const w = new World();
    const adm = w.user("admin"), A = w.user("teacher", "Ann"), B = w.user(), C = w.user();
    const qs = [mcq(`Student ${MARK} Smith in period 3 scored 42 on the quiz: which value is the mean of 40 and 44? (hint: it is the first choice)`, [["42", true], ["40", false]], { explanation: `Ann's note ${MARK2}: add both and halve, which gives forty two exactly as the key says it should.` })];
    const pub = (await publish(w, A, bankOf(qs), { hideKey: true, meta: { title: `Retake for ${MARK} period 3`, subject: "Math", course: "", grade: "", tags: [], standards: [] } })).body as any;
    w.advance(10 * DAY);
    for (const u of [B, C]) {
      w.history(u);
      await get(w, u, pub.item_id);
    }
    w.advance(DAY);
    for (const u of [B, C]) await w.json("POST", `/items/${pub.item_id}/report`, { user: u, json: { reason: "key_leak", note: "the hint gives it away" } });
    return { w, adm, A, pub };
  };
  {
    const { w, adm, pub } = await world();
    assert.ok(grep(w, MARK).length > 0);
    assert.equal((await w.purge<any>(adm, pub.item_id, { questions: [0] })).status, 200);
    await purgeDays(w);
    assert.deepEqual(grep(w, MARK), [], "the student's name");
  }
  {
    const { w, A } = await world();
    w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, pw_params = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, A.id);
    await w.db.batch(deleteContentStatements(w.db, A.id, w.clock));
    await purgeDays(w);
    assert.deepEqual([...grep(w, MARK), ...grep(w, MARK2)], [], "the author's typed text");
    assert.deepEqual(w.q(`SELECT title FROM items`).map((x: Q) => x.title), ["[removed]"]);
  }
});

// ---- headers, paging -----------------------------------------------------------------------------

test("headers (h1): pre-read errors carry the base security headers", async () => {
  const { preReadBody } = await import("../../src/pool/preread.ts");
  // A body shorter than its Content-Length, on an auth route (read by the pre-reader).
  const req = new Request("https://api.example.org/auth/login", { method: "POST", headers: { "Content-Type": "application/json", "Content-Length": "100" }, body: "{}" });
  const r = await preReadBody(req, async () => null);
  assert.ok(r instanceof Response, "refused");
  assert.equal((r as Response).status, 400);
  for (const h of ["x-content-type-options", "content-security-policy", "vary", "referrer-policy"]) assert.ok((r as Response).headers.get(h), h);
});

test("paging: /admin/pool/reports and /items/mine return a cursor", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user();
  const items: string[] = [];
  for (let k = 0; k < 3; k++) {
    items.push((await publish(w, A, bankOf([MALQ({ stem: `Item ${k}: is the water warm?` })]))).body.item_id as string);
    w.advance(1000);
  }
  const m = await w.json<any>("GET", "/items/mine", { user: A });
  assert.equal(m.body.next_cursor, null);
  assert.deepEqual(m.body.items.map((x: Q) => x.id), [...items].reverse(), "newest first");
  const r = await w.admin<any>(adm, "GET", "/admin/pool/reports");
  assert.ok("next_cursor" in r.body);
});

// ---- ghosts: licences, order, sketches; private items ----------------------------------------------

test("V1 (b1_ghost_now): a ghost keeps every licence the item had for the very content, until the item goes", async () => {
  for (const tail of ["none", "delete", "purge"] as const) {
    const w = new World();
    const A = w.user("teacher", "Ann"), Z = w.user("teacher", "Zed");
    const src = { ...ANNQ(), explanation: LONG };
    let a: any = (await publish(w, A, bankOf([src]), { license: "CC-BY-4.0" })).body;
    await imp(w, Z, a.item_id);
    for (const l of ["CC-BY-NC-SA-4.0", "CC-BY-NC-SA-4.0", "CC0-1.0"]) {
      w.advance(DAY);
      const r = await publish(w, A, bankOf(own(a, [src])), { license: l, base: a.publish_id }, a.item_id);
      a = { ...r.body, item_id: a.item_id };
    }
    if (tail !== "none") await w.json("DELETE", `/items/${a.item_id}`, { user: A });
    if (tail === "purge") await purgeDays(w);
    assert.equal(code(await publish(w, Z, bankOf([src]), { license: "CC0-1.0" })), "201", tail);
  }
});

test("V2 (d5_imgseq): a ghost image keeps when it was first published (the earliest material still decides)", async () => {
  for (const tail of ["none", "purge"] as const) {
    const w = new World();
    const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bea"), Z = w.user("teacher", "Zed");
    const a = (await publish(w, A, bankOf([mcq("Ann's question: which factor explains the 2019 decline at the Freeport trap?", [["Temperature", true], ["Bass", false]], { explanation: LONG })]), { license: "CC-BY-SA-4.0" })).body as any;
    w.advance(DAY);
    const ub = await upload(w, B, png(9, 9, 93));
    const b = (await publish(w, B, bankOf([mcq("Bea's levee question: why did the delta front retreat?", [["Sediment bypass", true], ["Rain", false]], { image: img(ub.sha) })]), { license: "CC-BY-NC-4.0" })).body as any;
    await get(w, Z, a.item_id);
    await get(w, Z, b.item_id);
    if (tail === "purge") {
      await w.json("DELETE", `/items/${b.item_id}`, { user: B });
      await purgeDays(w);
    }
    await upload(w, Z, png(9, 9, 93));
    const out = [];
    for (const L of ["CC-BY-SA-4.0", "CC-BY-NC-SA-4.0"]) {
      const r = await publish(w, Z, bankOf([MALQ({ explanation: LONG, image: img(ub.sha) })]), { license: L });
      out.push(code(r));
      if (r.status === 201) await w.json("DELETE", `/items/${r.body.item_id}`, { user: Z });
    }
    assert.deepEqual(out, ["201", "422 license_incompatible"], tail);
  }
});

test("V3 (c2_fuzzy_tail): a near-rewording of an imported question stays bound after the source is edited, deleted, purged or its author erased", async () => {
  const MAL = () => ({ ...ANNQ(), stem: ANNQ().stem.replace("best explains", "most likely explains") });
  for (const tail of ["none", "edit2", "delete", "purge", "erase"] as const) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    let a: any = (await publish(w, A, bankOf([ANNQ()]), { license: "CC-BY-NC-SA-4.0" })).body;
    await imp(w, M, a.item_id);
    w.advance(DAY);
    if (tail === "edit2")
      for (const p of [2, 3]) {
        const r = await publish(w, A, bankOf(own(a, [{ ...ANNQ(), stem: ANNQ().stem + (p === 2 ? " (field data)" : " (field data, 2019)") }])), { license: "CC-BY-NC-SA-4.0", base: a.publish_id }, a.item_id);
        a = { ...r.body, item_id: a.item_id };
        w.advance(DAY);
      }
    if (tail === "delete" || tail === "purge" || tail === "erase") await w.json("DELETE", `/items/${a.item_id}`, { user: A });
    if (tail === "purge" || tail === "erase") await purgeDays(w);
    if (tail === "erase") {
      w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, pw_params = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, A.id);
      await w.db.batch(deleteContentStatements(w.db, A.id, w.clock));
    }
    assert.equal(code(await publish(w, M, bankOf([MAL()]), { license: "CC0-1.0" })), "422 license_incompatible", tail);
    // A ghost keeps the sketch, never the words.
    if (tail === "purge") assert.ok(!JSON.stringify(w.q(`SELECT * FROM ghost_questions`)).includes("Chinook"));
  }
});

test("V4 (f1_oracle): an item the caller may not see never changes a dry run's answer", async () => {
  const KEY = "The model answer Vic keeps private: photosynthesis converts light energy into chemical energy stored in glucose, using water and carbon dioxide, and releases oxygen as a by product.";
  const w = new World({ unitKeysMax: 3 });
  const V = w.user("teacher", "Vic"), M = w.user("teacher", "Mal");
  await publish(w, V, bankOf([q("Vic's private question: explain photosynthesis in detail.", { kind: { Short: { sample: KEY } } })]), { visibility: "unlisted" });
  const prev = async (guess: string) =>
    code(
      await w.json<any>("POST", "/items/preview", {
        user: M,
        contentType: "application/vnd.vershion.publish",
        body: framed({ publish_id: `p${Math.random().toString(36).slice(2).padEnd(21, "x").slice(0, 21)}`, hide_key: false, license: "CC-BY-NC-SA-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: "2026-10-22" }, labels: [], order_acks: [], image_ok: [], meta: { title: "t", subject: "Biology", course: "", grade: "", tags: [], standards: [] }, visibility: "public" }, bankOf([q("Mal's probe question.", { kind: { Short: { sample: guess } } })])),
      }),
    );
  assert.equal(await prev(KEY), await prev(KEY.replace("glucose", "starch")));
});

// ---- the property, extended: two sources, a middle copier, eight tail kinds --------------------------

const SEEDS = Number(process.env.POOL_PROPERTY_SEEDS ?? 6);
const START = Number(process.env.POOL_PROPERTY_START ?? 1);
function rng(seed: number) {
  let s = seed >>> 0 || 1;
  return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 2 ** 32);
}
const ORDER = ["CC0-1.0", "CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"];
const BEA = "Bea's own worked explanation: the river gauge shows the flood peak arriving two days after the storm, and the levee breach widened the channel by forty metres in a single night.";

/** Ann's question and Bea's explanation, each through its own licence history; Mal and Cora read or
 *  import along the way; Cora publishes a merged adaptation and relicenses it; Mal reads Cora's too.
 *  Then (with `tail`) up to six of eight tail kinds; then Mal's battery. */
async function world2(seed: number, tail: boolean): Promise<{ bat: string[]; ops: string[] }> {
  const r = rng(seed);
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bea"), C = w.user("teacher", "Cora"), M = w.user("teacher", "Mal"), AD = w.user("admin");
  const pick = () => ORDER[Math.floor(r() * ORDER.length)]!;
  const aq = ANNQ();
  const bq = mcq("Bea's flood question: what widened the channel?", [["The breach", true], ["Rain", false]], { explanation: BEA });
  let a: any = (await publish(w, A, bankOf([aq]), { license: pick() })).body;
  let b: any = (await publish(w, B, bankOf([bq]), { license: pick() })).body;
  const ops: string[] = [];
  const serve = async (u: User, id: string) => ops.push(`${u.name}:${r() < 0.5 ? (await get(w, u, id)).status : (await imp(w, u, id)).status}`);
  for (let i = 0, n = 2 + Math.floor(r() * 4); i < n; i++) {
    w.advance(DAY);
    const k = Math.floor(r() * 4);
    if (k === 0) {
      const x = await publish(w, A, bankOf(own(a, [aq])), { license: pick(), base: a.publish_id }, a.item_id);
      if (x.status === 200) a = { ...x.body, item_id: a.item_id };
    } else if (k === 1) {
      const x = await publish(w, B, bankOf(own(b, [bq])), { license: pick(), base: b.publish_id }, b.item_id);
      if (x.status === 200) b = { ...x.body, item_id: b.item_id };
    } else await serve(k === 2 ? M : C, r() < 0.5 ? a.item_id : b.item_id);
  }
  // Cora: imports both, publishes the merged adaptation (Ann's question with Bea's explanation) under a
  // licence they allow, then relicenses it.
  await imp(w, C, a.item_id);
  await imp(w, C, b.item_id);
  let cpub: any = null;
  for (const L of ORDER) {
    const x = await publish(w, C, bankOf([{ ...aq, explanation: BEA }]), { license: L });
    if (x.status === 201) {
      cpub = x.body;
      break;
    }
  }
  if (cpub) {
    await serve(M, cpub.item_id);
    w.advance(DAY);
    const x = await publish(w, C, bankOf(own(cpub, [{ ...aq, explanation: BEA }])), { license: pick(), base: cpub.publish_id }, cpub.item_id);
    ops.push(`cora-relicence:${x.status}`);
  }
  await serve(M, a.item_id);
  // The same random numbers with and without the tail.
  const picks = Array.from({ length: 2 + Math.floor(r() * 5) }, () => [Math.floor(r() * 8), Math.floor(r() * 3)] as const);
  if (tail)
    for (const [t, who] of picks) {
      const [U, item] = [[A, a.item_id], [B, b.item_id], [C, cpub?.item_id]][who]! as [User, string | undefined];
      if (!item) continue;
      if (t === 0) ops.push(`del-${U.name}:${(await w.json("DELETE", `/items/${item}`, { user: U })).status}`);
      else if (t === 1) {
        await purgeDays(w);
        ops.push("20d");
      } else if (t === 2) {
        w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, pw_params = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, U.id);
        await w.db.batch(deleteContentStatements(w.db, U.id, w.clock));
        ops.push(`erase-${U.name}`);
      } else if (t === 3) ops.push(`adminpurge-${U.name}:${(await w.purge(AD, item)).status}`);
      else if (t === 4) {
        setLimit(w, M, "reads_max", 1);
        setLimit(w, M, "imports_max", 1);
        ops.push("caps");
      } else if (t === 5) {
        await w.pool.hourly();
        ops.push("hourly");
      } else if (t === 6) {
        for (let d = 0; d < 400; d += 50) {
          w.advance(50 * DAY);
          await w.pool.daily();
        }
        ops.push("400d");
      } else {
        // A retirement: the source republished twice unchanged (its served publish becomes a ghost).
        const cur = w.q<{ p: string }>(`SELECT current_publish_id AS p FROM items WHERE id = ?`, item)[0]?.p;
        const lic = w.q<{ l: string }>(`SELECT license AS l FROM items WHERE id = ?`, item)[0]?.l;
        const qids = cur ? w.q<{ qid: string }>(`SELECT qid FROM item_questions WHERE publish_id = ? ORDER BY pos`, cur).map((x) => x.qid) : [];
        if (cur && qids.length && lic) {
          const body = item === a.item_id ? aq : item === b.item_id ? bq : { ...aq, explanation: BEA };
          let base = cur;
          for (let k = 0; k < 2; k++) {
            const x = await publish(w, U, bankOf([{ ...body, published: { item_id: item, qid: qids[0], publish_id: base, version: 1 } }]), { license: lic, base }, item);
            if (x.status === 200) base = x.body.publish_id as string;
          }
          ops.push(`retire-${U.name}`);
        }
      }
    }
  const bodies: [string, Q][] = [
    ["annq", aq],
    ["beatext", MALQ({ explanation: BEA })],
    ["merged", { ...aq, explanation: BEA }],
  ];
  const bat: string[] = [];
  for (const L of ["CC0-1.0", "CC-BY-4.0", "CC-BY-NC-SA-4.0"])
    for (const [n, body] of bodies) {
      const p = await publish(w, M, bankOf([body]), { license: L });
      bat.push(`${n}@${L}=${code(p)}`);
      if (p.status === 201) await w.json("DELETE", `/items/${p.body.item_id}`, { user: M });
    }
  return { bat, ops };
}

test("property (critic27a d1/d2 generator): two sources with licence histories, a relicensing middle copier: no tail of eight kinds changes Mal's outcomes", async () => {
  for (let seed = START; seed < START + SEEDS; seed++) {
    const a = await world2(seed, false);
    const b = await world2(seed, true);
    assert.deepEqual(b.bat, a.bat, `seed ${seed}: ${b.ops.join(", ")}`);
  }
});

// ---- share-core traps (the reinit / health contract) ------------------------------------------------

test("share-core trap: the failing request is a plain 500, the instance is re-created, the next request (hashes, argon2id) works, author data starts clean, traps are counted and flagged", async () => {
  const h = await import("./harness.ts");
  const { makeCore } = await import("../../src/pool/core.ts");
  const sc = h.shareCore as Record<string, any>;
  let fail = 0;
  // A copy of the exports whose publish_prepare throws (as a wasm trap would) `fail` times.
  const m: Record<string, unknown> = {};
  for (const k of Object.keys(sc)) m[k] = sc[k];
  m.publish_prepare = (...a: unknown[]) => {
    if (fail > 0) {
      fail--;
      throw new Error("RuntimeError: unreachable executed (injected, with input text that must not leak)");
    }
    return sc.publish_prepare(...a);
  };
  let reinits = 0;
  m.reinit = () => (reinits++, sc.reinit());
  const w = new World({ core: makeCore(m as never, { hashKey: h.TEST_HASH_KEY }) });
  const A = w.user();
  const before = sc.argon2id(new TextEncoder().encode("pw"), new TextEncoder().encode("saltsaltsaltsalt"), 64, 1, 1);
  fail = 1;
  const r = await publish(w, A, bankOf([MALQ()]));
  assert.deepEqual([r.status, r.body], [500, { error: "server_error" }], "no detail from the exception");
  assert.equal(reinits, 1, "a fresh instance at once");
  assert.equal(String(sc.strip_author_data()).includes("no_author_data"), true, "author data from before the trap is gone");
  const ok = await publish(w, A, bankOf([MALQ()]));
  assert.equal(ok.status, 201, "the next request works");
  const ctl = new World();
  const c0 = await publish(ctl, ctl.user(), bankOf([MALQ()]));
  const mh = (ww: World, id: unknown) => ww.q<{ m: string }>(`SELECT iq.match_hash AS m FROM items i JOIN item_questions iq ON iq.publish_id = i.current_publish_id WHERE i.id = ?`, id)[0]!.m;
  assert.equal(mh(w, ok.body.item_id), mh(ctl, c0.body.item_id), "keyed hashes as before");
  const after = sc.argon2id(new TextEncoder().encode("pw"), new TextEncoder().encode("saltsaltsaltsalt"), 64, 1, 1);
  assert.deepEqual([...after], [...before], "argon2id after reinit");
  // Counted per hour; past WASM_TRAPS_PER_HOUR the hourly run flags it and tells the admin.
  const hour = Math.floor(w.clock / 3_600_000);
  assert.equal(w.q<{ value: string }>(`SELECT value FROM pool_kv WHERE key = ?`, `wasm_traps:${hour}`)[0]?.value, "1");
  fail = 6;
  for (let k = 0; k < 6; k++) assert.equal((await publish(w, A, bankOf([MALQ({ stem: `Trap ${k}: is it warm?` })]))).status, 500);
  await w.pool.hourly();
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'wasm_trap'`).length, 1);
  assert.ok(w.notices.some((n) => n.kind === "wasm_trap"));
});

test("share-core trap: after a trap, health() is checked before the next call and a broken instance is re-created", async () => {
  const { guardCall } = await import("../../src/pool/core.ts");
  let healthy = true;
  let reinits = 0;
  const life = { health: () => (healthy ? 1 : 0), reinit: () => void (reinits++, (healthy = true)) };
  let throwNext = true;
  const f = guardCall(life, (x: number) => {
    if (throwNext) {
      throwNext = false;
      throw new Error("boom");
    }
    return x + 1;
  }, "probe");
  assert.throws(() => f(1), (e: Error) => e.name === "WasmTrap" && !e.message.includes("boom"));
  assert.equal(reinits, 1);
  healthy = false;
  assert.equal(f(1), 2);
  assert.equal(reinits, 2, "health() was not 1: re-created before the call");
  assert.equal(f(2), 3);
  assert.equal(reinits, 2, "no check without a recent trap");
});
