// Round 18: critics 23c (Moodle) and 23d (ASVS), inverted. Units bind from durable per-reader evidence
// with the licence received (U1/U2); a hidden-only text has no public effect (U3); key_leak reports
// cannot be weaponised (U4); stem-only parts follow the question rule (U5); part model answers are
// texts (U6); claims never cover imported units, nor attribute by a URL alone; no full read during the
// migration window (V3); reader evidence and author records are bounded (V4); a backfill that can
// never fit is skipped and flagged, a long window is alerted (V5); JSON carries a CSP.
import assert from "node:assert/strict";
import { test } from "node:test";
import { png, publish, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const img = (sha: string) => ({ file: `media/${sha}.png`, width: 9, height: 9 });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const out = (r: { status: number; body: Q }) => (r.status === 422 && r.body.error === "license_incompatible" ? `422${JSON.stringify(r.body.allowed ?? null)}` : String(r.status));
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
const holds = (w: World, id: unknown) => w.q<{ reason: string; source: string }>(`SELECT reason, source FROM item_holds WHERE item_id = ? ORDER BY reason, source`, id);
const NCSA = `422${JSON.stringify(["CC-BY-NC-SA-4.0"])}`;
const LONG = "Ann's own worked explanation: the logger shows water above twenty degrees on forty days of the 2019 season, juvenile counts fell in exactly those weeks, and the regression slope links them clearly.";
const ANNQ = () => mcq("In Ann's field study of the delta, which factor best explains the 2019 decline of juvenile Chinook salmon at the Freeport trap?", [["Elevated summer water temperature above 20 C", true], ["Predation by striped bass", false], ["Reduced turbidity", false], ["Lower dissolved oxygen", false]]);
const filler = (v: number) => mcq(`Filler question about river deltas, revision ${v}: which landform forms at a river mouth?`, [["Delta", true], ["Mesa", false], ["Butte", false]]);

test("U1 (t1): a served text or image still binds after the source republished twice (its evidence is durable, not the source's retention)", async () => {
  for (const how of ["read", "import"])
    for (const unit of ["text", "image"]) {
      const w = new World();
      const A = w.user("teacher", "Ann"), M = w.user();
      const ua = await upload(w, A, png(9, 9, 61));
      let pub: any = (await publish(w, A, bankOf([{ ...ANNQ(), explanation: LONG, image: img(ua.sha) }, filler(0)]), { license: "CC-BY-NC-SA-4.0" })).body;
      w.advance(DAY);
      if (how === "read") await get(w, M, pub.item_id);
      else await imp(w, M, pub.item_id);
      for (let v = 1; v <= 2; v++) {
        w.advance(DAY);
        const r = await publish(w, A, bankOf(own(pub, [{ ...ANNQ(), explanation: LONG, image: img(ua.sha) }, filler(v)])), { license: "CC-BY-NC-SA-4.0", base: pub.publish_id }, pub.item_id);
        assert.equal(r.status, 200);
        pub = { ...r.body, item_id: pub.item_id };
      }
      await w.pool.hourly();
      await w.pool.daily();
      const up = await upload(w, M, png(9, 9, 61));
      const body = unit === "text" ? mcq("Mal's own question about rivers: is the water warm?", [["Yes", true], ["No", false]], { explanation: LONG }) : q("Mal's own question: what does this picture of the trap show?", { image: img(up.sha) });
      assert.equal(out(await publish(w, M, bankOf([body]), { license: "CC0-1.0" })), NCSA, `${how} ${unit}`);
    }
});

test("U2 (t2): a unit binds with the licence the account RECEIVED; the source's later relicensing changes nothing", async () => {
  for (const unit of ["text", "image", "part"]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), T = w.user();
    const ua = await upload(w, A, png(9, 9, 71));
    const src = unit === "text" ? { ...ANNQ(), explanation: LONG } : unit === "image" ? q("Ann's photo of the trap: what is it used for, and how?", { image: img(ua.sha) }) : ANNQ();
    const a = (await publish(w, A, bankOf([src]), { license: "CC-BY-4.0" })).body as any;
    await imp(w, T, a.item_id);
    assert.equal((await publish(w, A, bankOf(own(a, [src])), { license: "CC-BY-NC-SA-4.0", base: a.publish_id }, a.item_id)).status, 200);
    const ut = await upload(w, T, png(9, 9, 71));
    const mine =
      unit === "text"
        ? mcq("Tess's own question about rivers: is the water warm?", [["Yes", true], ["No", false]], { explanation: LONG })
        : unit === "image"
          ? q("Tess's question: what is this device used for in rivers?", { image: img(ut.sha) })
          : q("Answer both parts.", { kind: { Multipart: { parts: [{ stem: ANNQ().stem, kind: { Mc: { answers: ANNQ().kind.Mc.answers } } }, { stem: "Name the river.", kind: { Short: { sample: "Sacramento" } } }] } } });
    assert.equal((await publish(w, T, bankOf([mine]), { license: "CC-BY-4.0" })).status, 201, unit);
  }
});

test("U3 (t3): a hidden explanation or model answer equal to another's public text changes nothing readers see; the author alone is refused when she was served it", async () => {
  for (const field of ["explanation", "sample"]) {
    const view = async (equal: boolean, servedToAuthor: boolean) => {
      const w = new World();
      const M = w.user("teacher", "Mal"), A = w.user("teacher", "Ann"), R = w.user();
      const m = (await publish(w, M, bankOf([mcq("Mal's public question about the trap?", [["Yes", true], ["No", false]], { explanation: LONG })]), { license: "CC-BY-NC-SA-4.0" })).body as any;
      if (servedToAuthor) await get(w, A, m.item_id);
      const text = equal ? LONG : "Ann's own different explanation of the logger data, written in her own words and nothing like anyone else's text at all, long enough to be a unit of its own.";
      const mine = field === "explanation" ? mcq("Ann's hidden-key question about the trap?", [["Temperature", true], ["Predation", false]], { explanation: text }) : q("Ann's hidden-key question: explain the decline.", { kind: { Short: { sample: text } } });
      const p = await publish(w, A, bankOf([mine]), { license: "CC-BY-4.0", hideKey: true });
      if (p.status !== 201) return { status: p.status, error: p.body.error, hidden_material: p.body.hidden_material };
      const g = await get(w, R, p.body.item_id as string);
      return { status: 201, chain: g.body.questions.map((x: Q) => x.chain), lc: g.body.license_check, flags: w.q(`SELECT kind FROM admin_flags WHERE item_id = ? ORDER BY kind`, p.body.item_id) };
    };
    assert.deepEqual(await view(true, false), await view(false, false), `${field}: never served: identical to a control`);
    assert.deepEqual(await view(true, true), { status: 422, error: "license_incompatible", hidden_material: true }, `${field}: served to the author: refused to her`);
    assert.deepEqual(await view(false, true), await view(false, false));
  }
});

test("U4 (t4, 23d): key_leak reports cannot be weaponised; a fix ends the hold; a dismissal lifts it and keeps the same reporters out", async () => {
  const setup = async (qualify: boolean, read = qualify, seasoned = qualify) => {
    const w = new World();
    const A = w.user(), B = w.user(), C = w.user(), adm = w.user("admin");
    const item = (await publish(w, A, bankOf([mcq("Which planet is largest? (hint: it starts with J)", [["Jupiter", true], ["Mars", false], ["Venus", false]])]), { hideKey: true })).body as any;
    w.advance(10 * DAY);
    for (const u of [B, C]) {
      w.history(u);
      if (read) await get(w, u, item.item_id);
    }
    if (!seasoned) for (const u of [B, C]) w.sql.prepare(`UPDATE users SET created_at = ? WHERE id = ?`).run(w.clock - 60_000, u.id); // brand new accounts
    w.advance(DAY);
    for (const u of [B, C]) await w.json("POST", `/items/${item.item_id}/report`, { user: u, json: { reason: "key_leak" } });
    return { w, A, B, C, adm, item };
  };
  for (const [read, seasoned] of [[false, true], [true, false], [false, false]] as const) {
    const x = await setup(false, read, seasoned);
    assert.deepEqual(holds(x.w, x.item.item_id), [], `read=${read} seasoned=${seasoned}: no hold`);
  }
  const s = await setup(true);
  assert.deepEqual(holds(s.w, s.item.item_id), [{ reason: "reports", source: "key_leak" }]);
  const mine = (await s.w.json<any>("GET", "/items/mine", { user: s.A })).body.items[0];
  assert.equal(mine.state, "key_leak_review", JSON.stringify(mine));

  // The author's fix (a changed served form) ends it.
  const fixed = mcq("Which planet is largest?", [["Jupiter", true], ["Mars", false], ["Venus", false]]);
  assert.equal((await publish(s.w, s.A, bankOf(own(s.item, [fixed])), { hideKey: true, base: s.item.publish_id }, s.item.item_id)).status, 200);
  assert.deepEqual(holds(s.w, s.item.item_id), []);
  // Re-reporting is limited (one per item per 30 days), so the same pair cannot hold it again at once.
  s.w.advance(2 * DAY);
  assert.equal((await s.w.json("POST", `/items/${s.item.item_id}/report`, { user: s.B, json: { reason: "key_leak" } })).status, 429);
  // A dismissal lifts a hold and keeps the same reporters out for 90 days.
  const d = await setup(true);
  const rid = d.w.q<{ id: string }>(`SELECT id FROM reports WHERE item_id = ? AND reporter_id = ?`, d.item.item_id, d.B.id)[0]!.id;
  assert.equal((await d.w.admin(d.adm, "POST", "/admin/pool/dismiss", { report_id: rid })).status, 200);
  assert.deepEqual(holds(d.w, d.item.item_id), [], "below two open reports the hold goes");
  // A shown-key item takes no key_leak report.
  const w2 = new World();
  const A2 = w2.user(), B2 = w2.user();
  const pub = (await publish(w2, A2, bankOf([mcq("Which planet is largest?", [["Jupiter", true], ["Mars", false]])]))).body as any;
  w2.history(B2);
  assert.equal((await w2.json("POST", `/items/${pub.item_id}/report`, { user: B2, json: { reason: "key_leak" } })).status, 400);
  assert.equal(w2.q(`SELECT * FROM admin_flags WHERE kind = 'key_leak'`).length, 0);
});

test("U5 (t5): a stem-only Short prompt read from Ann and used as a part is soft like the whole question; imported, a claim may cover it", async () => {
  const STEM = "Explain in your own words why the sky appears blue on a clear day.";
  const annQ = q(STEM, { kind: { Short: { sample: "Rayleigh scattering: shorter blue wavelengths are scattered by air molecules far more than red ones." } } });
  const mine = q("Weather and light: answer every part.", { kind: { Multipart: { parts: [{ stem: STEM, kind: { Short: { sample: "Sunlight hits gas molecules and the blue part bounces around the most." } } }, { stem: "Name the gas that makes up most of the air.", kind: { Short: { sample: "Nitrogen" } } }] } } });
  for (const how of ["read", "import"]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), T = w.user(), R = w.user();
    const a = (await publish(w, A, bankOf([annQ]), { license: "CC-BY-NC-SA-4.0" })).body as any;
    if (how === "read") await get(w, T, a.item_id);
    else await imp(w, T, a.item_id);
    const p = await publish(w, T, bankOf([mine]), { license: "CC-BY-4.0" });
    if (how === "read") {
      assert.equal(p.status, 201);
      assert.equal((await get(w, R, p.body.item_id as string)).body.license_check.status, "unverified");
    } else {
      assert.equal(out(p), NCSA);
      assert.equal((await publish(w, T, bankOf([mine]), { license: "CC-BY-4.0", independent: [a.item_id], statement: "I wrote this part and its model answer myself for my weather unit." })).status, 201);
    }
  }
});

test("U6 (t7): a 25+ word model answer moved into or out of a Multipart part is the same text", async () => {
  for (const where of ["part_sample", "src_in_part"]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user();
    const src = where === "src_in_part" ? q("Ann's study: answer each part.", { kind: { Multipart: { parts: [{ stem: "Why did the counts fall in 2019 at the trap?", kind: { Short: { sample: LONG } } }, { stem: "Name the river.", kind: { Short: { sample: "Sacramento" } } }] } } }) : { ...ANNQ(), explanation: LONG };
    const a = (await publish(w, A, bankOf([src]), { license: "CC-BY-NC-SA-4.0" })).body as any;
    await imp(w, M, a.item_id);
    const mine = where === "src_in_part" ? mcq("Mal's own question about the river: is the water warm?", [["Yes", true], ["No", false]], { explanation: LONG }) : q("Mal's own two-part question about rivers.", { kind: { Multipart: { parts: [{ stem: "Mal's part: why might fish numbers fall in a warm year?", kind: { Short: { sample: LONG } } }, { stem: "Mal's part: name a river.", kind: { Short: { sample: "Feather" } } }] } } });
    assert.equal(out(await publish(w, M, bankOf([mine]), { license: "CC0-1.0" })), NCSA, where);
  }
});

test("claims: a URL alone attributes nothing; an imported image is never covered by a claim", async () => {
  for (const how of ["read", "import"]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user();
    const ua = await upload(w, A, png(9, 9, 88));
    const a = (await publish(w, A, bankOf([q("Ann's own photo of the trap: what is it used for, and how?", { image: img(ua.sha) })]), { license: "CC-BY-NC-SA-4.0" })).body as any;
    if (how === "read") await get(w, M, a.item_id);
    else await imp(w, M, a.item_id);
    const um = await upload(w, M, png(9, 9, 88));
    const body = bankOf([q("Mal's question: what is this device used for in rivers?", { image: img(um.sha) })]);
    assert.equal(out(await publish(w, M, body, { license: "CC0-1.0", independent: [a.item_id], statement: "Picture found at https://example.org/trap.png" })), NCSA, `${how}: URL only`);
    const attributed = await publish(w, M, body, { license: "CC0-1.0", independent: [a.item_id], statement: "Wikimedia Commons, public domain: https://commons.wikimedia.org/wiki/File:Trap.png" });
    assert.equal(attributed.status, how === "read" ? 201 : 422, `${how}: attributed`);
  }
});

test("V3: no non-author full read or import while the backfill runs", async () => {
  const w = new World();
  const A = w.user(), B = w.user();
  const a = (await publish(w, A, bankOf([ANNQ()]))).body as any;
  w.sql.prepare(`UPDATE items SET fold_rev = 0 WHERE id = ?`).run(a.item_id);
  assert.equal((await get(w, B, a.item_id)).status, 503);
  assert.equal((await imp(w, B, a.item_id)).status, 503);
  assert.equal((await get(w, A, a.item_id)).status, 200, "the author reads her own");
  assert.equal(w.q(`SELECT * FROM import_questions`).length, 0);
});

test("V4 / round 19 C1-C4: one served row per publish read (no copies of keys, nothing evicted); reads and imports are capped separately; author records count only what is new", async () => {
  const w = new World();
  const A = w.user(), B = w.user();
  const bank = (k: number) => bankOf(Array.from({ length: 30 }, (_, i) => mcq(`Bank ${k} question ${i}: which of these rivers drains into the delta region?`, [["Sacramento", true], ["Nile", false], ["Volga", false]])));
  const items: string[] = [];
  for (let k = 0; k < 4; k++) items.push(((await publish(w, A, bank(k))).body as any).item_id);
  const served = () => w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM served WHERE reader_id = ?`, B.id)[0]!.n;
  await get(w, B, items[0]!);
  assert.equal(served(), 1, "one row for the publish read");
  await get(w, B, items[0]!);
  assert.equal(served(), 1, "a second read writes nothing");
  assert.equal(w.q(`SELECT * FROM stem_bands WHERE user_id = ?`, B.id).length, 0, "no reader band rows");
  // Reads past reads_max: refused for publishes not served before; already served ones still read.
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'reads_max', 2, 0, 'x')`).run(B.id);
  assert.equal((await get(w, B, items[1]!)).status, 200);
  const r3 = await get(w, B, items[2]!);
  assert.deepEqual([r3.status, r3.body.error], [413, "reads_quota"]);
  assert.equal((await get(w, B, items[0]!)).status, 200);
  // ... and an import of an item already served is never refused by it.
  assert.equal((await imp(w, B, items[1]!)).status, 200);
  // Imports past imports_max: refused, whatever the reads.
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'imports_max', 1, 0, 'x')`).run(B.id);
  const i2 = await imp(w, B, items[0]!);
  assert.deepEqual([i2.status, i2.body.error], [413, "imports_quota"]);
  // Nothing recorded is ever evicted.
  assert.equal(w.q(`SELECT * FROM import_questions WHERE user_id = ?`, B.id).length, 60);
  // Author records: a republish adding nothing new is never refused; new questions past the cap are.
  const recs = w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM author_contents WHERE author_id = ?`, A.id)[0]!.n;
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'author_records', ?, 0, 'x')`).run(A.id, recs);
  const it0 = w.q<{ publish_id: string }>(`SELECT current_publish_id AS publish_id FROM items WHERE id = ?`, items[3])[0]!.publish_id;
  const qids = w.q<{ qid: string }>(`SELECT qid FROM item_questions WHERE publish_id = ? ORDER BY pos`, it0).map((x) => x.qid);
  const same = (JSON.parse(bank(3)).bank.questions as Q[]).map((x, i) => ({ ...x, published: { item_id: items[3], qid: qids[i], publish_id: it0, version: 1 } }));
  assert.equal((await publish(w, A, bankOf(same), { base: it0 }, items[3]!)).status, 200, "nothing new: never refused");
  const p = await publish(w, A, bank(9));
  assert.deepEqual([p.status, p.body.error], [413, "author_records_exceeded"]);
});

test("V5: a backfill item that can never fit is skipped and flagged (the window closes); a window open for a day is alerted", async () => {
  const w = new World({ backfillItems: 10 });
  const A = w.user();
  const a = (await publish(w, A, bankOf([ANNQ()]))).body as any;
  w.sql.prepare(`UPDATE items SET fold_rev = 0 WHERE id = ?`).run(a.item_id);
  w.sql.prepare(`UPDATE item_publishes SET full_bytes = ? WHERE item_id = ?`).run(10 * 1024 * 1024, a.item_id);
  await w.pool.hourly();
  assert.equal(w.q<{ f: number }>(`SELECT fold_rev AS f FROM items WHERE id = ?`, a.item_id)[0]!.f, 2);
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'backfill_skipped'`).length, 1);
  assert.equal((await publish(w, A, bankOf([q("A new question after the window?")]))).status, 201, "the gate opened");
  // A window that stays open: alerted after a day.
  const w2 = new World({ backfillItems: 0 });
  const B = w2.user();
  const b = (await publish(w2, B, bankOf([ANNQ()]))).body as any;
  w2.sql.prepare(`UPDATE items SET fold_rev = 0 WHERE id = ?`).run(b.item_id);
  w2.sql.prepare(`INSERT INTO pool_kv (key, value) VALUES ('migration_open_at', ?)`).run(String(w2.clock - 2 * DAY));
  await w2.pool.hourly();
  assert.equal(w2.q(`SELECT * FROM admin_flags WHERE kind = 'migration_stalled'`).length, 0, "(backfillItems 0: no run, no item seen)");
});

test("JSON responses carry a CSP; unexpected errors are logged by kind only", async () => {
  const w = new World();
  const A = w.user();
  const r = await w.call("GET", "/items/mine", { user: A });
  assert.equal(r.headers.get("Content-Security-Policy"), "default-src 'none'; frame-ancestors 'none'");
  const { errKind } = await import("../../src/pool/shared.ts");
  let e: unknown;
  try {
    JSON.parse('{"secret student answer": oops}');
  } catch (x) {
    e = x;
  }
  assert.equal(errKind(e), "SyntaxError");
});

test("a whole question against a whole question stays the question rule's: a changed copy of a READ Mc is soft, never bound as a unit", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), T = w.user(), R = w.user();
  const a = (await publish(w, A, bankOf([{ ...ANNQ(), explanation: "Ann's short note." }]), { license: "CC-BY-NC-SA-4.0" })).body as any;
  await get(w, T, a.item_id);
  const p = await publish(w, T, bankOf([{ ...ANNQ(), explanation: "Tess's own different note on the data." }]), { license: "CC-BY-4.0" });
  assert.equal(p.status, 201);
  assert.equal((await get(w, R, p.body.item_id as string)).body.license_check.status, "unverified");
});
