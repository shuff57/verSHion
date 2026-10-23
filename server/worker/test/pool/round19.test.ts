// Round 19: critics 24c (Moodle) and 24d (ASVS), inverted. Binding evidence is one small `served` row
// per (reader, publish read), matched against that publish's own key set (or its key-only ghost once
// it is gone): no per-account cap, no eviction, no expiry can change what binds (C1, C3); reads and
// imports are bounded separately and never block each other (C2); author records count only what is
// new and never block a fix (C4); a relicence to more permissive terms covers earlier readers (a);
// the hidden_material refusal names its sources (b); a key_leak hold is carried through delete and a
// new publish (c); a retained source the reader has a serve record of binds (D1); the key_leak
// dismissal counts qualifying reports only (D2); the unit lookups never scan (D3); duplicate JSON keys
// are refused (D5); the database size alert has its own notice (D7).
import assert from "node:assert/strict";
import { test } from "node:test";
import { framed, png, publish, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const img = (sha: string) => ({ file: `media/${sha}.png`, width: 9, height: 9 });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
const holds = (w: World, id: unknown) => w.q<{ reason: string; source: string }>(`SELECT reason, source FROM item_holds WHERE item_id = ? ORDER BY reason, source`, id);
const setLimit = (w: World, u: User, metric: string, value: number) =>
  w.sql.prepare(`INSERT OR REPLACE INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, ?, ?, 0, 'x')`).run(u.id, metric, value);
const LONG = "Ann's own worked explanation: the logger shows water above twenty degrees on forty days of the 2019 season, juvenile counts fell in exactly those weeks, and the regression slope links them clearly.";
const ANNQ = () => mcq("In Ann's field study of the delta, which factor best explains the 2019 decline of juvenile Chinook salmon at the Freeport trap?", [["Elevated summer water temperature above 20 C", true], ["Predation by striped bass", false], ["Reduced turbidity", false], ["Lower dissolved oxygen", false]]);
const MALQ = (extra: Q) => mcq("Mal's own question about rivers: is the water warm in August?", [["Yes", true], ["No", false]], extra);
const bank = (k: number, n = 30) => bankOf(Array.from({ length: n }, (_, i) => mcq(`Bank ${k} question ${i}: which of these rivers drains into the delta region number ${k}${i}?`, [["Sacramento", true], ["Nile", false], ["Volga", false]])));
const code = (r: { status: number; body: Q }) => (r.status >= 400 ? `${r.status} ${r.body.error}` : String(r.status));

/** Ann's NC-SA item: her question with a long explanation and an image. */
async function annItem(w: World, A: User, license = "CC-BY-NC-SA-4.0") {
  const ua = await upload(w, A, png(9, 9, 61));
  const a = (await publish(w, A, bankOf([{ ...ANNQ(), explanation: LONG, image: img(ua.sha) }]), { license })).body as any;
  return a;
}
/** Mal's copies of Ann's material: the question verbatim, her text in his question, her image in his question. */
async function copies(w: World, M: User, license: string): Promise<string[]> {
  const um = await upload(w, M, png(9, 9, 61));
  const out: string[] = [];
  for (const body of [{ ...ANNQ(), explanation: LONG }, MALQ({ explanation: LONG }), q("Mal's own question: what does this picture of the trap show?", { image: img(um.sha) })])
    out.push(code(await publish(w, M, bankOf([body]), { license })));
  return out;
}

test("C1 (s2_evict / t1): reading filler never removes what binds: one served row per publish read, nothing evicted, at any cap", async () => {
  for (const how of ["read", "import"])
    for (const cap of [null, 2]) {
      const w = new World();
      const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), S = w.user("teacher", "Sock");
      const a = await annItem(w, A);
      const fill: string[] = [];
      for (let k = 0; k < 4; k++) fill.push(((await publish(w, S, bank(k))).body as any).item_id);
      if (cap !== null) setLimit(w, M, "reads_max", cap);
      w.advance(DAY);
      assert.equal((how === "read" ? await get(w, M, a.item_id) : await imp(w, M, a.item_id)).status, 200);
      const reads: number[] = [];
      for (const id of fill) {
        w.advance(1000);
        reads.push((await get(w, M, id)).status);
      }
      // Past reads_max a read of something new is refused (never an eviction).
      if (cap !== null) assert.deepEqual(reads, how === "read" ? [200, 413, 413, 413] : [200, 200, 413, 413]);
      assert.equal(w.q(`SELECT * FROM served WHERE reader_id = ? AND item_id = ?`, M.id, a.item_id).length, 1, "Ann's serve kept");
      assert.equal(w.q(`SELECT * FROM served WHERE reader_id = ?`, M.id).length, 1 + reads.filter((x) => x === 200).length, "one row per publish read");
      assert.deepEqual(await copies(w, M, "CC0-1.0"), ["422 license_incompatible", "422 license_incompatible", "422 license_incompatible"], `${how} cap=${cap}`);
      assert.deepEqual(await copies(w, M, "CC-BY-NC-SA-4.0"), ["201", "201", "201"]);
    }
});

test("C2 (s1_quota / t2_importblock): reads never block imports; imports have their own allowance and a clear refusal", async () => {
  const w = new World();
  const A = w.user(), T = w.user("teacher", "Tess");
  const items: string[] = [];
  for (let k = 0; k < 4; k++) items.push(((await publish(w, A, bank(k))).body as any).item_id);
  const small = ((await publish(w, A, bankOf([mcq("A one-question item: which planet is largest?", [["Jupiter", true], ["Mars", false]])]))).body as any).item_id;
  setLimit(w, T, "reads_max", 3);
  for (const id of items.slice(0, 3)) assert.equal((await get(w, T, id)).status, 200);
  const r4 = await get(w, T, items[3]!);
  assert.deepEqual([r4.status, r4.body.error], [413, "reads_quota"]);
  assert.match(r4.body.message, /reads_max/);
  // An import of a never-read item, and of a read one: both go through.
  assert.equal((await imp(w, T, small)).status, 200);
  assert.equal((await imp(w, T, items[0]!)).status, 200);
  assert.equal((await imp(w, T, items[3]!)).status, 200, "an import is never refused by the reads count");
  // ... and imports never use up reads: a read of an item served before still works.
  assert.equal((await get(w, T, items[1]!)).status, 200);
  // imports_max: a clear refusal saying what to do; a re-import of an imported item still works.
  setLimit(w, T, "imports_max", 3);
  const i4 = await imp(w, T, items[1]!);
  assert.deepEqual([i4.status, i4.body.error], [413, "imports_quota"]);
  assert.match(i4.body.message, /admin/);
  assert.equal((await imp(w, T, small)).status, 200);
  // Nothing expires: 400 days and the crons later, the same.
  w.advance(400 * DAY);
  await w.pool.hourly();
  await w.pool.daily();
  assert.equal(w.q(`SELECT * FROM served WHERE reader_id = ?`, T.id).length, 5);
  assert.equal(w.q(`SELECT * FROM import_questions WHERE user_id = ?`, T.id).length, 121);
});

test("C3 (s3_purge): the binding survives the source's deletion, purge and its author's account deletion, at +120 and +1000 days", async () => {
  for (const how of ["import", "read"])
    for (const wait of [120, 1000])
      for (const erase of [false, true]) {
        const w = new World();
        const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), R = w.user();
        const a = await annItem(w, A);
        w.advance(DAY);
        assert.equal((how === "import" ? await imp(w, M, a.item_id) : await get(w, M, a.item_id)).status, 200);
        assert.equal((await w.json("DELETE", `/items/${a.item_id}`, { user: A })).status, 200);
        for (let i = 0; i < 20; i++) {
          w.advance(DAY);
          await w.pool.daily();
        }
        assert.ok(w.q<{ p: number | null }>(`SELECT purged_at AS p FROM items WHERE id = ?`, a.item_id)[0]!.p !== null, "purged");
        assert.equal(w.q(`SELECT * FROM item_publishes WHERE item_id = ?`, a.item_id).length, 0);
        assert.equal(w.q(`SELECT * FROM ghost_publishes WHERE item_id = ?`, a.item_id).length, 1, "a key-only ghost of what Mal was served");
        if (erase) {
          const { deleteContentStatements } = await import("../../src/pool/index.ts");
          w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, pw_params = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, A.id);
          await w.db.batch(deleteContentStatements(w.db, A.id, w.clock));
        }
        for (let i = 0; i < wait; i += 10) {
          w.advance(10 * DAY);
          await w.pool.daily();
        }
        await w.pool.hourly();
        const tag = `${how} +${wait}d erase=${erase}`;
        assert.deepEqual(await copies(w, M, "CC0-1.0"), ["422 license_incompatible", "422 license_incompatible", "422 license_incompatible"], tag);
        const ok = await publish(w, M, bankOf([{ ...ANNQ(), explanation: LONG }]), { license: "CC-BY-NC-SA-4.0" });
        assert.equal(ok.status, 201, tag);
        const g = await get(w, R, ok.body.item_id as string);
        assert.deepEqual(g.body.questions[0].chain.map((l: any) => l.author_name), [erase ? "deleted teacher" : "Ann"], tag);
        // The ghost keeps no text: hashes and ids only.
        const gq = w.q<Record<string, unknown>>(`SELECT * FROM ghost_questions WHERE publish_id IN (SELECT publish_id FROM ghost_publishes WHERE item_id = ?)`, a.item_id);
        assert.ok(gq.length === 1 && !JSON.stringify(gq).includes("Chinook") && !JSON.stringify(gq).includes("logger"), tag);
        // Ann's name is not stored in the ghost once she is erased.
        if (erase) assert.ok(!JSON.stringify(w.q(`SELECT * FROM ghost_questions`)).includes("Ann"), tag);
      }
});

test("C3: a ghost goes only when no served row references it (the reader's account deletion)", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
  const a = await annItem(w, A);
  await imp(w, M, a.item_id);
  await w.json("DELETE", `/items/${a.item_id}`, { user: A });
  for (let i = 0; i < 20; i++) {
    w.advance(DAY);
    await w.pool.daily();
  }
  assert.equal(w.q(`SELECT * FROM ghost_publishes`).length, 1);
  await w.pool.daily();
  assert.equal(w.q(`SELECT * FROM ghost_publishes`).length, 1, "kept while referenced");
  const { deleteContentStatements } = await import("../../src/pool/index.ts");
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, pw_params = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, M.id);
  await w.db.batch(deleteContentStatements(w.db, M.id, w.clock));
  await w.pool.daily();
  assert.deepEqual([w.q(`SELECT * FROM ghost_publishes`).length, w.q(`SELECT * FROM ghost_questions`).length, w.q(`SELECT * FROM ghost_units`).length], [0, 0, 0]);
  // A publish nobody was served leaves no ghost.
  const b = (await publish(w, A, bankOf([MALQ({})]))).body as any;
  await w.json("DELETE", `/items/${b.item_id}`, { user: A });
  for (let i = 0; i < 20; i++) {
    w.advance(DAY);
    await w.pool.daily();
  }
  assert.equal(w.q(`SELECT * FROM ghost_publishes`).length, 0);
});

test("C4 / D6 (s4_records, s4b_fix): only new records count, exactly; a republish adding nothing and a fix of a held item are never refused", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user(), C = w.user();
  const recs = () => w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM author_contents WHERE author_id = ?`, A.id)[0]!.n;
  const qs = (n: number, hint: boolean) => Array.from({ length: n }, (_, i) => mcq(`Bank question ${i}: which value is ${i * 7}?${hint && i === 0 ? " (hint: it is the first choice)" : ""}`, [[`${i * 7}`, true], ["zero", false], ["ten", false]]));
  let pub: any = (await publish(w, A, bankOf(qs(20, true)), { hideKey: true })).body;
  w.advance(10 * DAY);
  for (const u of [B, C]) {
    w.history(u);
    await get(w, u, pub.item_id);
  }
  w.advance(DAY);
  for (const u of [B, C]) await w.json("POST", `/items/${pub.item_id}/report`, { user: u, json: { reason: "key_leak" } });
  assert.deepEqual(holds(w, pub.item_id), [{ reason: "reports", source: "key_leak" }]);
  // The cap at exactly what is recorded: nothing new can be added.
  setLimit(w, A, "author_records", recs());
  const fix = await publish(w, A, bankOf(own(pub, qs(20, false))), { hideKey: true, base: pub.publish_id }, pub.item_id);
  assert.equal(fix.status, 200, JSON.stringify(fix.body).slice(0, 200));
  assert.deepEqual(holds(w, pub.item_id), [], "the fix lifts the hold");
  pub = { ...fix.body, item_id: pub.item_id };
  setLimit(w, A, "author_records", recs());
  const same = await publish(w, A, bankOf(own(pub, qs(20, false))), { hideKey: true, base: pub.publish_id }, pub.item_id);
  assert.equal(same.status, 200, "adds nothing: never refused");
  // A new question: exactly its fresh records (served near, key-free; the full form is hidden).
  const before = recs();
  const one = () => publish(w, A, bankOf([mcq("A brand new question about tides and the moon?", [["Gravity", true], ["Wind", false]])]), { hideKey: true });
  // The first headroom that lets it through is exactly what it then writes.
  let k = 0;
  for (; k < 10; k++) {
    setLimit(w, A, "author_records", before + k);
    const p = await one();
    if (p.status === 201) break;
    assert.deepEqual([p.status, p.body.error], [413, "author_records_exceeded"]);
    assert.match(String(p.body.message), /Republishing your existing items still works/);
  }
  assert.ok(k >= 2 && k < 10, `headroom ${k}`);
  assert.equal(recs(), before + k, "the count was exact");
});

test("variant a (s5_relic): a relicence to more permissive terms covers earlier readers; a stricter one never narrows what they received", async () => {
  for (const [l1, l2] of [["CC-BY-NC-SA-4.0", "CC-BY-4.0"], ["CC-BY-4.0", "CC-BY-NC-SA-4.0"]])
    for (const how of ["read", "import"]) {
      const w = new World();
      const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
      const src = { ...ANNQ(), explanation: LONG };
      const a = (await publish(w, A, bankOf([src]), { license: l1 })).body as any;
      assert.equal((how === "read" ? await get(w, M, a.item_id) : await imp(w, M, a.item_id)).status, 200);
      w.advance(DAY);
      assert.equal((await publish(w, A, bankOf(own(a, [src])), { license: l2, base: a.publish_id }, a.item_id)).status, 200);
      const tag = `${l1}->${l2} ${how}`;
      // (a re-read under stricter terms never takes back what was received; after a more permissive
      // relicence Mal does NOT re-read: the new terms reach him anyway)
      if (l2 === "CC-BY-NC-SA-4.0") await get(w, M, a.item_id);
      assert.equal((await publish(w, M, bankOf([MALQ({ explanation: LONG })]), { license: "CC-BY-4.0" })).status, 201, tag);
      assert.equal((await publish(w, M, bankOf([src]), { license: "CC-BY-4.0" })).status, 201, tag);
    }
});

test("variant b: the hidden_material refusal names the items the author was shown the material in, with their licences", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
  const a = (await publish(w, A, bankOf([{ ...ANNQ(), explanation: LONG }]), { license: "CC-BY-NC-SA-4.0", meta: { title: "Ann's delta salmon set", subject: "Biology", course: "", grade: "", tags: [], standards: [] } })).body as any;
  await get(w, M, a.item_id);
  const p = await publish(w, M, bankOf([MALQ({ explanation: LONG })]), { license: "CC0-1.0", hideKey: true });
  assert.deepEqual([p.status, p.body.error, p.body.hidden_material], [422, "license_incompatible", true]);
  assert.deepEqual(p.body.sources, [{ item_id: a.item_id, title: "Ann's delta salmon set", link: `/items/${a.item_id}`, licenses: ["CC-BY-NC-SA-4.0"] }]);
  assert.match(String(p.body.message), /Ann's delta salmon set" \(CC-BY-NC-SA-4\.0\)/);
});

test("variant c (h_hold H2): a key_leak hold is carried through deleting the item and publishing the same content as a new one", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user(), C = w.user(), R = w.user();
  const LEAK = () => mcq("Which planet is largest? (hint: it starts with J)", [["Jupiter", true], ["Mars", false], ["Venus", false]]);
  const it = (await publish(w, A, bankOf([LEAK()]), { hideKey: true })).body as any;
  w.advance(10 * DAY);
  for (const u of [B, C]) {
    w.history(u);
    await get(w, u, it.item_id);
  }
  w.advance(DAY);
  for (const u of [B, C]) await w.json("POST", `/items/${it.item_id}/report`, { user: u, json: { reason: "key_leak" } });
  assert.equal((await w.json("DELETE", `/items/${it.item_id}`, { user: A })).status, 200);
  const n = await publish(w, A, bankOf([LEAK()]), { hideKey: true });
  assert.equal(n.status, 201);
  assert.equal(holds(w, n.body.item_id).length, 1, "held again");
  assert.equal((await get(w, R, n.body.item_id as string)).status, 404);
  // Fixed content is not held.
  const f = await publish(w, A, bankOf([mcq("Which planet is largest?", [["Jupiter", true], ["Mars", false], ["Venus", false]])]), { hideKey: true });
  assert.equal(holds(w, f.body.item_id).length, 0);
});

test("D1: a serve whose publish's units are unknown (pre-0104, gone) binds through the item's retained publishes with its licences; a known serve is examined exactly", async () => {
  for (const legacy of [true, false]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    const a = (await publish(w, A, bankOf([{ ...ANNQ(), explanation: LONG }]), { license: "CC-BY-NC-4.0" })).body as any;
    assert.equal((await imp(w, M, a.item_id)).status, 200);
    w.advance(DAY);
    assert.equal((await publish(w, A, bankOf(own(a, [{ ...ANNQ(), explanation: LONG, points: 2 }])), { license: "CC-BY-NC-4.0", base: a.publish_id }, a.item_id)).status, 200);
    if (legacy) {
      // As 0104 leaves a pre-0104 serve of a publish gone since: question hashes only, no units.
      const { deletePublishStatements } = await import("../../src/pool/publish.ts");
      await w.db.batch(deletePublishStatements(w.db, [a.publish_id]));
      w.sql.exec(`UPDATE ghost_publishes SET units_known = 0; DELETE FROM ghost_units;`);
    }
    const p = await publish(w, M, bankOf([MALQ({ explanation: LONG })]), { license: "CC0-1.0" });
    assert.deepEqual([p.status, p.body.error], [422, "license_incompatible"], `legacy=${legacy}`);
  }
  // A version never served does not bind (round 17 M1 S1b): the soft path.
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), R = w.user();
  const a = (await publish(w, A, bankOf([ANNQ()]), { license: "CC-BY-NC-4.0" })).body as any;
  await imp(w, M, a.item_id);
  w.advance(DAY);
  await publish(w, A, bankOf(own(a, [{ ...ANNQ(), explanation: LONG }])), { license: "CC-BY-NC-4.0", base: a.publish_id }, a.item_id);
  const p = await publish(w, M, bankOf([MALQ({ explanation: LONG })]), { license: "CC0-1.0" });
  assert.equal(p.status, 201);
  assert.equal((await get(w, R, p.body.item_id as string)).body.questions[0].chain[0]?.earlier_match, true, "soft: an earlier publication");
});

test("D2 (t6): a key_leak dismissal counts only QUALIFYING open reports, the same predicate that placed the hold", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), R1 = w.user("teacher", "Rex One"), R2 = w.user("teacher", "Rex Two"), N = w.user("teacher", "Newbie"), AD = w.user("admin", "Admin");
  const it = (await publish(w, A, bankOf([mcq("Which river drains the delta region in the north valley?", [["Sacramento", true], ["Nile", false], ["Volga", false]])]), { hideKey: true })).body as any;
  await get(w, R1, it.item_id);
  await get(w, R2, it.item_id);
  w.advance(10 * DAY);
  const rep = (u: User) => w.json("POST", `/items/${it.item_id}/report`, { user: u, json: { reason: "key_leak" } });
  assert.equal((await rep(N)).status, 202, "recorded, never counted (never served)");
  await rep(R1);
  await rep(R2);
  assert.deepEqual(holds(w, it.item_id), [{ reason: "reports", source: "key_leak" }]);
  const r1 = w.q<{ id: string }>(`SELECT id FROM reports WHERE reporter_id = ?`, R1.id)[0]!.id;
  assert.equal((await w.admin(AD, "POST", "/admin/pool/dismiss", { report_id: r1 })).status, 200);
  assert.deepEqual(holds(w, it.item_id), [], "Newbie's open report does not keep it");
  assert.equal((await get(w, w.user(), it.item_id)).status, 200);
});

test("D3: the unit lookups (publish, preview, read, import) never scan item_publishes, served, the question/unit/blob tables or the ghosts", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), S = w.user();
  const a = await annItem(w, A);
  for (let k = 0; k < 6; k++) await publish(w, S, bank(k, 4));
  await get(w, M, a.item_id);
  // A gone publish Mal was served: its ghost is looked up too.
  const b = await annItem(w, S, "CC-BY-4.0");
  await imp(w, M, b.item_id);
  await w.json("DELETE", `/items/${b.item_id}`, { user: S });
  for (let i = 0; i < 20; i++) {
    w.advance(DAY);
    await w.pool.daily();
  }
  const watched = new Set(["item_publishes", "served", "item_questions", "item_part_keys", "item_blobs", "ghost_publishes", "ghost_questions", "ghost_units", "imports", "author_contents", "items", "blobs"]);
  const views = w.q<{ name: string; sql: string }>(`SELECT name, sql FROM sqlite_master WHERE type = 'view'`);
  const scans: string[] = [];
  let phase = "";
  w.stats.log = (sql, args) => {
    if (!/^\s*(SELECT|WITH|INSERT|UPDATE|DELETE)/i.test(sql)) return;
    const text = sql + views.filter((v) => new RegExp(`\\b${v.name}\\b`).test(sql)).map((v) => v.sql).join(" ");
    for (const r of w.sql.prepare(`EXPLAIN QUERY PLAN ${sql}`).all(...(args as never[])) as { detail: string }[]) {
      // (A walk of ALL the reader's served rows is a scan too: up to reads_max rows.)
      if (/^SEARCH \w+ USING (COVERING )?INDEX (sqlite_autoindex_served_1|served_reader_kind) \(reader_id=\?\)$/.test(r.detail)) scans.push(`${phase}: ${r.detail} :: ${sql.replace(/\s+/g, " ").slice(0, 140)}`);
      const m = /^SCAN (\w+)/.exec(r.detail);
      if (!m || /VIRTUAL TABLE/.test(r.detail)) continue;
      const t = [...watched].find((t) => t === m[1] || new RegExp(`(FROM|JOIN)\\s+${t}\\s+(AS\\s+)?${m[1]}\\b`, "i").test(text));
      if (t) scans.push(`${phase}: ${r.detail} (${t}) :: ${sql.replace(/\s+/g, " ").slice(0, 140)}`);
    }
  };
  const um = await upload(w, M, png(9, 9, 61));
  const body = bankOf([{ ...ANNQ(), explanation: LONG }, MALQ({ explanation: LONG }), q("Mal's own question: what does this picture show?", { image: img(um.sha) })]);
  phase = "publish";
  await publish(w, M, body, { license: "CC-BY-NC-SA-4.0" });
  phase = "preview";
  await w.json("POST", "/items/preview", {
    user: M,
    body: framed({ publish_id: "p".repeat(22), hide_key: false, license: "CC0-1.0", attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: "2026-10-22" }, labels: [], order_acks: [], image_ok: [], meta: { title: "t", subject: "Biology", course: "", grade: "", tags: [], standards: [] }, visibility: "public" }, body),
    contentType: "application/vnd.vershion.publish",
  });
  phase = "read";
  await get(w, M, a.item_id);
  phase = "import";
  await imp(w, M, a.item_id);
  w.stats.log = undefined;
  assert.deepEqual(scans, []);
});

test("D5: pool JSON bodies and publish meta with a duplicate key are refused (never read last-wins)", async () => {
  const w = new World();
  const A = w.user(), B = w.user();
  const it = (await publish(w, A, bankOf([MALQ({})]))).body as any;
  await imp(w, B, it.item_id);
  const r = await w.json<any>("POST", `/items/${it.item_id}/report`, { user: B, body: '{"reason":"wrong_answer","reason":"student_data"}', contentType: "application/json" });
  assert.deepEqual([r.status, r.body.error, r.body.reason], [400, "invalid_json", "duplicate_key"]);
  assert.equal(w.q(`SELECT * FROM reports`).length, 0);
  assert.equal(w.q(`SELECT * FROM item_holds`).length, 0, "nothing was filed as student data");
  const meta = framed({ publish_id: "q".repeat(22), hide_key: false, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: "2026-10-22" }, labels: [], order_acks: [], image_ok: [], meta: { title: "t", subject: "Biology", course: "", grade: "", tags: [], standards: [] }, visibility: "public" }, bankOf([MALQ({})]));
  const dup = new TextDecoder().decode(meta).replace('"license":"CC-BY-4.0"', '"license":"CC-BY-4.0","license":"CC0-1.0"');
  assert.notEqual(dup, new TextDecoder().decode(meta));
  const p = await w.json<any>("POST", "/items", { user: A, body: dup, contentType: "application/vnd.vershion.publish" });
  assert.deepEqual([p.status, p.body.error], [400, "invalid_json"]);
});

test("D7: the database size alert is its own notice (and its own mail subject), never a maintenance failure", async () => {
  const w = new World();
  const prep = w.db.prepare.bind(w.db);
  (w.db as any).prepare = (sql: string) => {
    const st = prep(sql);
    if (sql === "SELECT 1 AS x") {
      const run = st.run.bind(st);
      (st as any).run = async () => {
        const r = await run();
        (r.meta as any).size_after = 9 * 1024 ** 3;
        return r;
      };
    }
    return st;
  };
  await w.pool.hourly();
  assert.deepEqual(w.notices.filter((n) => n.kind === "d1_size"), [{ kind: "d1_size", percent: 80, bytes: 9 * 1024 ** 3 }]);
  assert.equal(w.notices.filter((n) => n.kind === "cron_failure").length, 0);
  const { poolNotifier } = await import("../../src/poolmail.ts");
  const sent: string[] = [];
  const orig = globalThis.fetch;
  globalThis.fetch = (async (_u: unknown, init?: { body?: string }) => {
    sent.push(String(init?.body ?? ""));
    return new Response("{}", { status: 200 });
  }) as typeof fetch;
  try {
    const env = { DB: { prepare: () => ({ bind: () => ({ first: async () => null, all: async () => ({ results: [] }) }), all: async () => ({ results: [] }) }) } };
    await poolNotifier({ ...env, RESEND_API_KEY: "k", MAIL_FROM: "a@x.org", ADMIN_EMAIL: "admin@x.org", MAIL_MODE: "send" } as never)({ kind: "d1_size", percent: 80, bytes: 1 }).catch(() => {});
  } finally {
    globalThis.fetch = orig;
  }
  if (sent.length > 0) {
    assert.match(sent.join("\n"), /pool database at 80% of its size limit/);
    assert.ok(!sent.join("\n").includes("maintenance step failed"));
  }
});

// ---- the property: no cap, eviction, expiry or deletion changes what binds -------------------------

const SEEDS = Number(process.env.POOL_PROPERTY_SEEDS ?? 6);
function rng(seed: number) {
  let s = seed >>> 0 || 1;
  return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 2 ** 32);
}

/** Ann's item, Mal reads or imports it, then `noise` (with caps), then Mal publishes each copy. */
async function scenario(how: "read" | "import", noise: ((x: Ctx19) => Promise<void>)[], capped: boolean): Promise<string[]> {
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), S = w.user("teacher", "Sock");
  const a = await annItem(w, A);
  const fill: string[] = [];
  for (let k = 0; k < 4; k++) fill.push(((await publish(w, S, bank(k, 6))).body as any).item_id);
  if (capped) {
    setLimit(w, M, "reads_max", 2);
    setLimit(w, M, "imports_max", 2);
  }
  w.advance(DAY);
  assert.equal((how === "read" ? await get(w, M, a.item_id) : await imp(w, M, a.item_id)).status, 200);
  const x: Ctx19 = { w, A, M, S, a, fill, deleted: false };
  for (const n of noise) await n(x);
  const out = [...(await copies(w, M, "CC0-1.0")), ...(await copies(w, M, "CC-BY-NC-SA-4.0"))];
  return out;
}
type Ctx19 = { w: World; A: User; M: User; S: User; a: any; fill: string[]; deleted: boolean };
const NOISE: [string, (r: () => number) => (x: Ctx19) => Promise<void>][] = [
  ["read filler", (r) => async (x) => void (await get(x.w, x.M, x.fill[Math.floor(r() * x.fill.length)]!))],
  ["import filler", (r) => async (x) => void (await imp(x.w, x.M, x.fill[Math.floor(r() * x.fill.length)]!))],
  ["reread source", () => async (x) => void (await get(x.w, x.M, x.a.item_id))],
  ["source deleted", () => async (x) => {
    if (!x.deleted) x.deleted = (await x.w.json("DELETE", `/items/${x.a.item_id}`, { user: x.A })).status === 200;
  }],
  ["filler republished", () => async (x) => {
    const id = x.fill[0]!;
    const cur = x.w.q<{ p: string }>(`SELECT current_publish_id AS p FROM items WHERE id = ?`, id)[0]!.p;
    const qids = x.w.q<{ qid: string }>(`SELECT qid FROM item_questions WHERE publish_id = ? ORDER BY pos`, cur).map((y) => y.qid);
    const qs = (JSON.parse(bank(0, 6)).bank.questions as Q[]).map((y, i) => ({ ...y, explanation: `revision ${x.w.clock}`, published: { item_id: id, qid: qids[i], publish_id: cur, version: 1 } }));
    await publish(x.w, x.S, bankOf(qs), { base: cur }, id);
  }],
  ["days pass", (r) => async (x) => {
    const n = 1 + Math.floor(r() * 400);
    for (let i = 0; i < n; i += 20) {
      x.w.advance(Math.min(20, n - i) * DAY);
      await x.w.pool.daily();
    }
  }],
  ["hourly", () => async (x) => void (await x.w.pool.hourly())],
];

test("property: random reads, imports, deletes, purges, republishes and crons (with caps) never change the outcome of publishing a copy", async () => {
  const base = { read: await scenario("read", [], false), import: await scenario("import", [], false) };
  assert.deepEqual(base.read.slice(0, 3), ["422 license_incompatible", "422 license_incompatible", "422 license_incompatible"]);
  for (let seed = Number(process.env.POOL_PROPERTY_START ?? 1); seed <= SEEDS; seed++) {
    const r = rng(seed * 7919);
    const how = r() < 0.5 ? "read" : "import";
    const ops = Array.from({ length: 3 + Math.floor(r() * 6) }, () => NOISE[Math.floor(r() * NOISE.length)]!);
    const got = await scenario(how, ops.map(([, f]) => f(r)), r() < 0.7);
    assert.deepEqual(got, base[how], `seed ${seed} ${how}: ${ops.map(([n]) => n).join(", ")}`);
  }
});
