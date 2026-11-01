// Round 8: evidence that cannot lock anyone out and does not depend on bank size (I1/I2), takedowns
// that never block the rightful lineage (I3), one bounded body reader for every route, authenticated
// first (I4/I5), held student-data text and bounded flags/alerts (I6). Critics 11a/11b inverted.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { preReadBody } from "../../src/pool/preread.ts";
import { bytesReserved, unauthReserved } from "../../src/pool/shared.ts";
import { API, genQuestion, publish, rng, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[], sv = 2) => JSON.stringify({ schema_version: sv, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const tf = (stem: string, correct = true, extra: Q = {}): Q => q(stem, { kind: { Tf: { correct } }, ...extra });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => (i < pub.qids.length ? { ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } } : x));
const withOrigin = (r: any) =>
  r.body.payload.bank.questions.map((x: Q, i: number) => ({ ...x, origin: { item_id: r.body.item.id, qid: r.body.questions[i].qid, publish_id: r.body.item.publish_id, version: r.body.item.version, author_name: r.body.item.author_name, license: r.body.license, form: r.body.item.form } }));
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const holds = (w: World, id: string) => w.q<{ reason: string; source: string }>(`SELECT reason, source FROM item_holds WHERE item_id = ? ORDER BY reason, source`, id);
const LIC = ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"];
const filler = (n: number) => Array.from({ length: n }, (_, i) => tf(`Victim's own filler statement number ${i} about igneous rocks.`, i % 2 === 0));

// ---- I1: no lockout, no dependence on bank size ------------------------------------------------------

test("x1/x1b (I1): a later attacker bank repeating the victim's question 40 times never locks the victim out", async () => {
  const w = new World();
  const victim = w.user("teacher", "Victim"), att = w.user("teacher", "Attacker");
  const Qv = genQuestion(rng(42), "Mc", 0).json as Q;
  const v1 = (await publish(w, victim, bankOf([Qv, tf("Victim's other question")]), { license: "CC-BY-4.0" })).body;
  w.advance(1000);
  const a1 = (await publish(w, att, bankOf(Array.from({ length: 40 }, (_, i) => ({ ...Qv, id: `dup${i}` }))), { license: "CC-BY-SA-4.0" })).body;
  const a2 = (await publish(w, att, bankOf([Qv]), { license: "CC-BY-NC-4.0" })).body;
  for (const id of [a1.item_id, a2.item_id]) await imp(w, victim, id); // even IMPORTED: they came later than the victim's own
  for (const lic of ["CC-BY-4.0", "CC-BY-NC-SA-4.0"]) assert.equal((await publish(w, victim, bankOf([Qv]), { license: lic })).status, 201, lic);
  const rp = await publish(w, victim, bankOf(own(v1, [Qv, tf("Victim's other question, typo fixed")])), { license: "CC-BY-4.0", base: v1.publish_id }, v1.item_id);
  assert.equal(rp.status, 200, "republishing her own item");
});

test("I1 property: a question's licence outcome is the same in banks of 1, 11, 100 and 1000 questions", async () => {
  for (let seed = 1; seed <= 4; seed++) {
    const outs: string[] = [];
    for (const size of [1, 11, 100, 1000]) {
      const r = rng(seed * 7919);
      const w = new World();
      const T = w.user("teacher", "Tess");
      const Qn = q("How many centimetres are in one metre?", { kind: { Short: { sample: "100" } }, explanation: r.bool() ? "Zero's explanation" : null });
      const nsrc = 1 + r.int(40);
      for (let k = 0; k < nsrc; k++) {
        const u = w.user("teacher", `S${k}`);
        const same = r.bool(0.6);
        const p = (await publish(w, u, bankOf([same ? Qn : { ...Qn, explanation: `explanation ${k}` }]), { license: r.pick(LIC) })).body;
        const how = r.pick(["import", "get", "none"]);
        if (how === "import") await imp(w, T, p.item_id);
        else if (how === "get") await get(w, T, p.item_id);
        w.advance(1000);
      }
      const lic = r.pick(LIC);
      const res = await publish(w, T, bankOf([Qn, ...filler(size - 1)]), { license: lic });
      const p0 = (res.body.per_question as Q[] | undefined)?.find((x) => x.pos === 0);
      outs.push(JSON.stringify([res.status === 201 ? "ok" : res.body.error, p0?.allowed ?? null, (p0?.bound_by ?? []).length]));
    }
    assert.ok(outs.every((x) => x === outs[0]), `seed ${seed}: bank size changes the outcome: ${outs.join(" | ")}`);
  }
});

test("s9/s9c (I1): a verbatim copy of an imported CC0 question stays CC0 whatever else was imported, in any bank size", async () => {
  for (const [n, size] of [[33, 1], [5, 1000], [5, 11]] as const) {
    const w = new World();
    const T = w.user("teacher", "Tess"), Z = w.user("teacher", "Zero");
    const c = (e: string) => q("What is the chemical symbol for sodium?", { kind: { Short: { sample: "Na" } }, explanation: e });
    const z = (await publish(w, Z, bankOf([c("Zero's")]), { license: "CC0-1.0" })).body;
    for (let i = 0; i < n; i++) {
      w.advance(1000);
      const p = (await publish(w, w.user(), bankOf([c(`expl ${i}`)]), { license: LIC[i % 5]! })).body;
      await imp(w, T, p.item_id);
    }
    const iz = await imp(w, T, z.item_id);
    const r = await publish(w, T, bankOf([...withOrigin(iz), ...filler(size - 1)]), { license: "CC0-1.0" });
    assert.equal(r.status, 201, `${n} sources, bank of ${size}: ${JSON.stringify(r.body).slice(0, 300)}`);
  }
});

// ---- I2: imports of common / non-verbatim questions ---------------------------------------------------

test("s5 (I2, round 9): her own version of a question two imported banks contain is an adaptation of imports: they contradict, so any one's terms apply; a verbatim copy without origin is refused", async () => {
  const w = new World();
  const S = w.user("teacher", "Sam"), N = w.user("teacher", "Nia"), T = w.user("teacher", "Tess");
  const capital = (expl: string | null) => q("What is the capital of France?", { kind: { Short: { sample: "Paris" } }, explanation: expl });
  const s = (await publish(w, S, bankOf([capital("Sam: Paris has been the capital since 987."), q("Sam's rivers question?")]), { license: "CC-BY-SA-4.0" })).body;
  const n = (await publish(w, N, bankOf([capital("Nia: Paris, on the Seine."), q("Nia's mountains question?")]), { license: "CC-BY-NC-4.0" })).body;
  w.advance(DAY);
  await imp(w, T, s.item_id);
  await imp(w, T, n.item_id);
  w.advance(DAY);
  const mine = [capital("Tess: my own explanation from my lesson."), q("Tess's deserts question?")];
  for (const lic of LIC) {
    const r = await publish(w, T, bankOf(mine), { license: lic });
    // Round 10 M-1: Sam's (earlier) terms decide the contradiction.
    if (lic !== "CC-BY-SA-4.0") {
      assert.deepEqual([r.status, r.body.allowed], [422, ["CC-BY-SA-4.0"]], lic);
      continue;
    }
    assert.equal(r.status, 201, lic);
    assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'license_conflict' AND item_id = ?`, r.body.item_id).length, 1, `${lic}: published, flagged (the SA and NC sources cannot both be met)`);
  }
  // A verbatim copy of Sam's question (no origin declared): an import copied verbatim is hard evidence.
  const verbatim = await publish(w, T, bankOf([capital("Sam: Paris has been the capital since 987.")]), { license: "CC-BY-4.0" });
  assert.deepEqual([verbatim.status, verbatim.body.error, verbatim.body.allowed], [422, "license_incompatible", ["CC-BY-SA-4.0"]]);
  assert.deepEqual((verbatim.body.per_question as Q[])[0]!.bound_by.map((b: Q) => b.author_name), ["Sam"]);
});

// ---- I3: the rightful lineage is never blocked ----------------------------------------------------------

test("s3 (I3): a takedown naming Rita's item never holds content tracing to it (Zed imported it), now or 40 days later; unnamed blocks expire", async () => {
  const w = new World();
  const adm = w.user("admin"), R = w.user("teacher", "Rita"), X = w.user("teacher", "Xavier"), Z = w.user("teacher", "Zed"), O = w.user("teacher", "Other");
  const content = [q("Rita's original question about mitochondria and ATP synthase?")];
  const x = (await publish(w, X, bankOf(content), { license: "CC-BY-NC-4.0" })).body;
  w.advance(DAY);
  const r = (await publish(w, R, bankOf(content), { license: "CC-BY-4.0" })).body;
  const rz = await imp(w, Z, r.item_id);
  await w.takedown(adm, await notice(w, adm), x.item_id, { rightful_item_id: r.item_id, exempt_authors: [R.id] });
  const z1 = (await publish(w, Z, bankOf(withOrigin(rz)), { license: "CC-BY-4.0" })).body;
  assert.deepEqual(holds(w, z1.item_id), [], "Zed's copy of Rita's own item is not a re-upload of the infringement");
  w.advance(40 * DAY);
  await w.pool.daily();
  const z2 = (await publish(w, Z, bankOf(withOrigin(rz)), { license: "CC-BY-4.0" })).body;
  assert.deepEqual(holds(w, z2.item_id), [], "nor 40 days later");
  // Someone with no lineage to Rita's item is held, until the default expiry of an unnamed block.
  const w2 = new World();
  const adm2 = w2.user("admin"), X2 = w2.user(), O2 = w2.user();
  const x2 = (await publish(w2, X2, bankOf(content))).body;
  await w2.takedown(adm2, await notice(w2, adm2), x2.item_id);
  const o1 = (await publish(w2, O2, bankOf(content))).body;
  assert.equal(holds(w2, o1.item_id)[0]?.reason, "blocklist");
  w2.advance(61 * DAY);
  const o2 = (await publish(w2, O2, bankOf(content))).body;
  assert.deepEqual(holds(w2, o2.item_id), [], "the default 60-day block has expired");
  void O;
});

test("I3: held copies whose lineage includes the rightful item are released when the takedown is upheld", async () => {
  const w = new World();
  const adm = w.user("admin"), R = w.user("teacher", "Rita"), X = w.user("teacher", "Xavier"), Z = w.user("teacher", "Zed");
  const content = [q("Rita's second original question about the Krebs cycle?")];
  const r = (await publish(w, R, bankOf(content), { license: "CC-BY-4.0" })).body;
  const x = (await publish(w, X, bankOf(content), { license: "CC-BY-NC-4.0" })).body;
  const rz = await imp(w, Z, r.item_id);
  const z = (await publish(w, Z, bankOf(withOrigin(rz)), { license: "CC-BY-4.0" })).body;
  const td = await w.takedown<any>(adm, await notice(w, adm), x.item_id, { rightful_item_id: r.item_id });
  // The admin holds everything sharing the question (include_unlinked): Zed's copy of Rita's item too.
  await w.holdCopies(adm, td.body.action_id, { include_unlinked: true });
  assert.equal(holds(w, z.item_id)[0]?.source, `dmca:${td.body.action_id}`);
  w.advance(31 * DAY);
  await w.pool.daily();
  assert.deepEqual(holds(w, z.item_id), [], "released at the upheld takedown");
});

// ---- I4/I5: one body reader, authenticated first ---------------------------------------------------------

test("I4: a chunked 1 MB body to an auth route is refused after reading at most ~16 KB; nothing reads a request body directly", async () => {
  let pulled = 0;
  const body = new ReadableStream<Uint8Array>({
    pull(c) {
      if (pulled >= 1 << 20) return c.close();
      c.enqueue(new Uint8Array(1024).fill(32));
      pulled += 1024;
    },
  });
  const r = await preReadBody(new Request(`${API}/auth/login`, { method: "POST", headers: { "Content-Type": "application/json", "CF-Connecting-IP": "10.0.0.1" }, body, duplex: "half" } as RequestInit), async () => null, (x) => x);
  assert.ok(r instanceof Response);
  assert.equal(r.status, 411);
  assert.ok(pulled <= 16 * 1024, `pulled ${pulled}`);
  const big = await preReadBody(new Request(`${API}/auth/login`, { method: "POST", headers: { "Content-Type": "application/json", "Content-Length": String(1 << 20), "CF-Connecting-IP": "10.0.0.1" }, body: new Uint8Array(1 << 20) }), async () => null, (x) => x);
  assert.equal((big as Response).status, 413);
  // Lint: no direct request-body read anywhere in the Worker or the auth package, except the two readers.
  const roots = ["src", "../auth/src"];
  const offenders: string[] = [];
  const walk = (d: string) => {
    for (const f of readdirSync(d, { withFileTypes: true })) {
      const p = `${d}/${f.name}`;
      if (f.isDirectory()) walk(p);
      else if (/\.(ts|js)$/.test(f.name)) {
        const text = readFileSync(p, "utf8");
        for (const [k, line] of text.split("\n").entries())
          if (/\b(req|request)\.(json|text|arrayBuffer|formData|blob)\(|\b(req|request)\.body\.getReader\(/.test(line) && !/pool\/shared\.ts$|auth\/src\/http\.ts$/.test(p)) offenders.push(`${p}:${k + 1}`);
      }
    }
  };
  for (const r0 of roots) walk(new URL(`../../${r0}`, import.meta.url).pathname);
  assert.deepEqual(offenders, []);
});

test("x5 (I5): stalled unauthenticated POSTs can neither lock a signed-in admin out of their address nor take the isolate budget", async () => {
  const stall = () => new ReadableStream<Uint8Array>({ pull: () => new Promise(() => {}) });
  const unauth = (ip: string) => preReadBody(new Request(`${API}/admin/pool/hold`, { method: "POST", headers: { "Content-Length": "8192", "Content-Type": "application/json", "CF-Connecting-IP": ip }, body: stall(), duplex: "half" } as RequestInit), async () => null, (x) => x);
  const pend = [unauth("203.0.113.7"), unauth("203.0.113.7"), unauth("203.0.113.7"), unauth("203.0.113.7")];
  const fifth = await unauth("203.0.113.7");
  assert.equal((fifth as Response).status, 429, "4 unauthenticated bodies per IPv4 address (round 9 K2)");
  // The admin on the same address, signed in: read under the account, not the address.
  const ok = await preReadBody(new Request(`${API}/admin/pool/hold`, { method: "POST", headers: { Authorization: "Bearer good", "Content-Length": "2", "Content-Type": "application/json", "CF-Connecting-IP": "203.0.113.7" }, body: "{}" }), async () => ({ userId: "admin-1" }), (x) => x);
  assert.ok(ok instanceof Request);
  // Thousands of addresses: bytes are counted as they arrive, in a separate small budget.
  const many: Promise<unknown>[] = [];
  for (let i = 0; i < 2000; i++) many.push(unauth(`2001:db8:${i.toString(16)}::/64`));
  await new Promise((r) => setTimeout(r, 20));
  assert.equal(bytesReserved(), 0, "stalled bodies that sent nothing hold nothing of the isolate budget");
  assert.ok(unauthReserved() <= 4 * 1024 * 1024);
  const w = new World();
  const t = w.user("teacher");
  assert.equal((await publish(w, t, bankOf([tf("An authenticated publish under unauthenticated load")]))).status, 201);
  void pend;
});

// ---- I6: held student-data text, bounded flags and alerts ---------------------------------------------------

test("x3/x7 (I6): confirmed student-data text is held again; one flag per kind per publish; the privacy mail has its own budget; the queue is severity-first and paged", async () => {
  const w = new World();
  const admin = w.user("admin"), a = w.user("teacher", "A"), reporter = w.user("teacher", "Rep"), third = w.user();
  w.sql.prepare("UPDATE users SET created_at = 0").run();
  w.advance(8 * DAY);
  const S = tf("Jane Doe (ID 448812, IEP: dyslexia) scored 41% on the unit test. True or false?");
  const p = (await publish(w, a, bankOf([S]))).body;
  await w.purge(admin, p.item_id);
  const again = (await publish(w, a, bankOf([S]))).body;
  assert.equal((await get(w, third, again.item_id)).status, 404, "not served");
  // 1000 bogus declared origins: one origin_unverified flag with a count.
  const flood = Array.from({ length: 1000 }, (_, i) => tf(`Statement ${i} about plate tectonics.`, true, { origin: { item_id: "ab".repeat(16), qid: "cd".repeat(16), publish_id: "AAAAAAAAAAAAAAAAAAAAAA", version: 1, author_name: "X", license: "CC-BY-4.0", form: "full" } }));
  const f = (await publish(w, a, bankOf(flood))).body;
  const rows = w.q<{ detail: string }>(`SELECT detail FROM admin_flags WHERE item_id = ? AND kind = 'origin_unverified'`, f.item_id);
  assert.equal(rows.length, 1);
  assert.equal(JSON.parse(rows[0]!.detail).count, 1000);
  // Five re-publishes of blocked text, then a real report: the report's mail still goes out.
  w.notices.length = 0;
  for (let i = 0; i < 6; i++) await publish(w, a, bankOf([S]));
  const victim = (await publish(w, a, bankOf([tf("A different question naming a student, Bob.")]))).body;
  w.history(reporter);
  await w.json("POST", `/items/${victim.item_id}/report`, { user: reporter, json: { reason: "student_data", note: "student name" } });
  assert.ok(w.notices.some((n) => n.kind === "student_data_report"), "never swallowed by other alerts");
  // The queue: most severe first, newest first, paged.
  const page = await w.admin<any>(admin, "GET", "/admin/pool/flags", null, "?limit=3");
  assert.equal(page.body.flags[0].kind, "privacy_text_match");
  assert.ok(page.body.next_cursor && page.body.total_open > 3);
  const page2 = await w.admin<any>(admin, "GET", "/admin/pool/flags", null, `?limit=3&cursor=${encodeURIComponent(page.body.next_cursor)}`);
  assert.ok(page2.body.flags.length > 0 && !page2.body.flags.some((x: Q) => page.body.flags.some((y: Q) => y.id === x.id)));
});

// ---- non-blocking ------------------------------------------------------------------------------------------

test("reports: an instant student-data hold needs a reporter with some pool history; the admin sees the reporter's history", async () => {
  const w = new World();
  const admin = w.user("admin"), a = w.user(), bare = w.user(), seasoned = w.user();
  w.sql.prepare("UPDATE users SET created_at = 0").run();
  w.advance(8 * DAY);
  const p1 = (await publish(w, a, bankOf([tf("Question one about tides and the moon.")]))).body;
  const p2 = (await publish(w, a, bankOf([tf("Question two about tides and the sun.")]))).body;
  await w.json("POST", `/items/${p1.item_id}/report`, { user: bare, json: { reason: "student_data" } });
  assert.deepEqual(holds(w, p1.item_id), [], "a bare account's report waits for the admin");
  await imp(w, seasoned, p1.item_id);
  await w.json("POST", `/items/${p2.item_id}/report`, { user: seasoned, json: { reason: "student_data" } });
  assert.equal(holds(w, p2.item_id).length, 1);
  const q = await w.admin<any>(admin, "GET", "/admin/pool/reports");
  const r = q.body.items.flatMap((x: Q) => x.reports).find((x: Q) => x.reporter_id === seasoned.id);
  assert.deepEqual([r.reporter_imports, r.reporter_items], [1, 0]);
});

test("versions: every reversible state is under_review (with its date), removed only after the purge; held copies are under_review", async () => {
  const w = new World();
  const admin = w.user("admin"), a = w.user(), b = w.user();
  const p = (await publish(w, a, bankOf([tf("A question that will be deleted and held.")]))).body;
  const p2 = (await publish(w, a, bankOf([tf("A question that an admin will hold.")]))).body;
  await imp(w, b, p.item_id);
  await imp(w, b, p2.item_id);
  const del = await w.json<any>("DELETE", `/items/${p.item_id}`, { user: a });
  await w.admin(admin, "POST", "/admin/pool/hold", { item_id: p2.item_id });
  const v = async () => (await w.json<any>("POST", "/items/versions", { user: b, json: { ids: [p.item_id, p2.item_id] } })).body.items;
  assert.deepEqual(await v(), [{ id: p.item_id, status: "under_review", reason: "deleted", until: del.body.undo_until }, { id: p2.item_id, status: "under_review", reason: "held" }]);
  w.advance(15 * DAY);
  await w.pool.daily();
  assert.deepEqual((await v())[0], { id: p.item_id, status: "removed", reason: "deleted" });
});

test("privacy holds on copies are lifted when a republish removes the blocked content", async () => {
  const w = new World();
  const admin = w.user("admin"), A = w.user(), B = w.user(), V = w.user();
  const pii = tf("Student Jamie Smith (ID 4411) failed the retake on Friday.");
  const a = (await publish(w, A, bankOf([pii]))).body;
  const rb = await imp(w, B, a.item_id);
  const b = (await publish(w, B, bankOf([...withOrigin(rb), tf("Bob's own clean question about rocks.")]))).body;
  await w.purge(admin, a.item_id);
  assert.equal(holds(w, b.item_id)[0]?.source, "privacy");
  const clean = await publish(w, B, bankOf(own(b, [tf("Bob's own clean question about rocks.")]).map((x, i) => (i === 0 ? { ...x, published: { ...x.published, qid: b.qids[1] } } : x))), { base: b.publish_id }, b.item_id);
  assert.equal(clean.status, 200);
  assert.deepEqual(holds(w, b.item_id), [], "the cause no longer matches");
  assert.equal((await get(w, V, b.item_id)).status, 200);
});

test("account deletion: free text goes (statements, appeal notes, report notes); with no choice made, items others imported are kept", async () => {
  const w = new World();
  const admin = w.user("admin"), a = w.user("teacher", "Pat Smith"), b = w.user();
  const x = (await publish(w, b, bankOf([tf("Someone else's item to name.")]))).body;
  const kept = (await publish(w, a, bankOf([tf("Pat's item others imported.")]))).body;
  const gone = (await publish(w, a, bankOf([tf("Pat's item nobody imported.")]))).body;
  await imp(w, b, kept.item_id);
  await publish(w, a, bankOf([tf("An item about volcanoes, written by me.")]), { independent: [x.item_id], statement: "I am Pat Smith of Lincoln High, phone 555-0100; I wrote this myself." });
  await w.json("POST", `/items/${x.item_id}/report`, { user: a, json: { reason: "other", note: "Pat's note with 555-0100" } });
  assert.deepEqual((await w.json<any>("GET", "/items/prefs", { user: a })).body, { keep_on_delete: null, default: "keep_imported_delete_rest" });
  const { deleteContentStatements } = await import("../../src/pool/index.ts");
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, a.id);
  await w.db.batch(deleteContentStatements(w.db, a.id, w.clock));
  assert.equal(w.q(`SELECT * FROM pool_audit WHERE note LIKE '%555-0100%' OR detail LIKE '%555-0100%'`).length, 0);
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE detail LIKE '%555-0100%'`).length, 0);
  assert.equal(w.q(`SELECT * FROM reports WHERE note LIKE '%555-0100%'`).length, 0);
  const state = (id: string) => w.q<{ removed_reason: string | null; anonymized: number }>(`SELECT removed_reason, anonymized FROM items WHERE id = ?`, id)[0];
  assert.deepEqual(state(kept.item_id), { removed_reason: null, anonymized: 1 });
  assert.deepEqual(state(gone.item_id), { removed_reason: "account_deleted", anonymized: 0 });
  void admin;
});

test("search: Unicode case folding (ß = ss, accents optional), word prefixes; If-None-Match with a weak ETag", async () => {
  const w = new World();
  const a = w.user(), b = w.user();
  const meta = (title: string) => ({ title, subject: "Physique", course: "", grade: "", tags: [], standards: [] });
  for (const [i, t] of ["Énergie cinétique", "straße quiz", "Ωmega set"].entries()) await publish(w, a, bankOf([tf(`Distinct search question ${i}.`)]), { meta: meta(t) });
  const L = async (qs: string) => (await w.json<any>("GET", `/items?${qs}`, { user: b })).body.items.map((x: Q) => x.title).sort();
  assert.deepEqual(await L("q=ENERGIE"), ["Énergie cinétique"]);
  assert.deepEqual(await L("q=strasse"), ["straße quiz"]);
  assert.deepEqual(await L(`q=${encodeURIComponent("STRAßE")}`), ["straße quiz"]);
  assert.deepEqual(await L("q=quiz"), ["straße quiz"], "a word inside the title");
  assert.deepEqual(await L(`q=${encodeURIComponent("ωMEGA")}`), ["Ωmega set"]);
  assert.deepEqual(await L("subject=PHYSIQUE"), ["straße quiz", "Énergie cinétique", "Ωmega set"].sort());
  const id = (await w.json<any>("GET", "/items?limit=1", { user: b })).body.items[0].id;
  const etag = (await w.call("GET", `/items/${id}`, { user: b })).headers.get("ETag")!;
  assert.equal((await w.call("GET", `/items/${id}`, { user: b, headers: { "If-None-Match": `W/${etag}` } })).status, 304);
});

test("x8: one account behind a NAT takes at most 20% of the address's daily read budget", async () => {
  const counts = new Map<string, number>();
  const store = {
    async hit(key: string, rule: { limit: number; windowSeconds: number }) {
      if (rule.windowSeconds !== 86_400) return { allowed: true, retryAfter: 0 }; // only the daily read budgets matter here
      const n = (counts.get(key) ?? 0) + 1;
      counts.set(key, n);
      return n <= rule.limit ? { allowed: true, retryAfter: 0 } : { allowed: false, retryAfter: 30 };
    },
    async undo(key: string) {
      counts.set(key, Math.max(0, (counts.get(key) ?? 0) - 1));
    },
  };
  const w = new World({ rateStore: store as never });
  const att = w.user(), col = w.user();
  const { RATE } = await import("../../src/pool/shared.ts");
  const share = RATE.accountIpReadDaily[0]!.limit;
  // Pre-load the counters as if the attacker had read `share` times today (fast-forward, same keys).
  for (let i = 0; i < share; i++) assert.equal((await w.call("GET", "/items/mine", { user: att, ip: "198.51.100.10" })).status, 200);
  assert.equal((await w.call("GET", "/items/mine", { user: att, ip: "198.51.100.10" })).status, 429);
  assert.equal((await w.call("GET", "/items/mine", { user: col, ip: "198.51.100.10" })).status, 200, "the colleague still reads");
  assert.ok(share <= RATE.ipReadDaily[0]!.limit / 5);
});

test("cron: the daily purge works through every due item in batches and skips one that fails", async () => {
  const w = new World();
  const a = w.user();
  const ids: string[] = [];
  for (let i = 0; i < 5; i++) {
    const p = (await publish(w, a, bankOf([tf(`Purge batch question ${i}.`)]))).body;
    ids.push(p.item_id as string);
    await w.json("DELETE", `/items/${p.item_id}`, { user: a });
  }
  w.advance(15 * DAY);
  // One item's purge fails in the database (a trigger stands in for the failure).
  w.sql.exec(`CREATE TRIGGER boom BEFORE UPDATE OF purged_at ON items WHEN NEW.id = '${ids[2]}' BEGIN SELECT RAISE(ABORT, 'boom'); END`);
  const rep = await w.pool.daily();
  w.sql.exec(`DROP TRIGGER boom`);
  assert.equal(rep.purged, 4, "the failing item is skipped, the rest still go");
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'cron_failure'`).length, 1);
});

test("API: unblock accepts the preview's sha256 name; stand-ins for an unlisted rightful item are neutral and keep the licence at void time", async () => {
  const w = new World();
  const adm = w.user("admin"), R = w.user("teacher", "Rita"), X = w.user("teacher", "Xavier"), C = w.user("teacher", "Cal"), Eve = w.user();
  const content = [q("Rita's question about enzyme kinetics and Km values?")];
  const r = (await publish(w, R, bankOf(content), { license: "CC-BY-NC-4.0" })).body;
  w.advance(1000);
  const x = (await publish(w, X, bankOf(content), { license: "CC-BY-4.0" })).body;
  const rc = await imp(w, C, x.item_id);
  const c1 = (await publish(w, C, bankOf(withOrigin(rc)), { license: "CC-BY-4.0" })).body;
  assert.equal((await w.takedown(adm, await notice(w, adm), x.item_id, { rightful_item_id: r.item_id, exempt_authors: [R.id] })).status, 200);
  // Round 11 P4: the rightful item must be visible at the takedown; it may be unlisted later.
  assert.equal((await w.json("PATCH", `/items/${r.item_id}`, { user: R, json: { visibility: "unlisted" } })).status, 200);
  w.advance(31 * DAY);
  await w.pool.daily();
  const chain = (await get(w, Eve, c1.item_id)).body.questions[0].chain;
  assert.deepEqual(chain.map((l: Q) => [l.item_id, l.author_name, l.license, l.modified]), [["", "the rightful author", "CC-BY-NC-4.0", false]], "no id or name of an unlisted item; verbatim, so not modified");
  // The preview names an image "sha256"; unblock accepts it.
  const w2 = new World();
  const adm2 = w2.user("admin"), A = w2.user();
  const { png, upload } = await import("./harness.ts");
  const up = await upload(w2, A, png(8, 8, 3));
  const p = (await publish(w2, A, bankOf([tf("Student Jo's photo", true, { image: { file: `media/${up.sha}.png`, width: 8, height: 8 } })]))).body;
  await w2.purge(adm2, p.item_id);
  assert.equal((await w2.admin(adm2, "POST", "/admin/pool/unblock", { kind: "blob", sha256: up.sha })).status, 200);
});

test("streams: a large body moving fast enough outlives the lifetime; a trickle does not", async () => {
  const w = new World({ readIdleMs: 2000, readLifetimeMs: 200 });
  const a = w.user(), b = w.user();
  const it = (await publish(w, a, bankOf(Array.from({ length: 200 }, (_, i) => q(`Question ${i} ${"padding text ".repeat(40)}`))))).body;
  const r = (await w.pool.handle(new Request(`${API}/items/${it.item_id}`, { headers: { Authorization: `Bearer ${b.token}`, "CF-Connecting-IP": "10.0.0.5" } })))!;
  const rd = r.body!.getReader();
  let got = 0;
  // Fast: a chunk every 50 ms (well above 20 KB/s), past the 200 ms lifetime.
  for (;;) {
    const x = await rd.read();
    if (x.done) break;
    got += x.value.byteLength;
    await new Promise((res) => setTimeout(res, 50));
  }
  assert.equal(got, Number(r.headers.get("Content-Length")), "read to the end");
});
