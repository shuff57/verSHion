// Round 9: critics 12a (ASVS) / 12b (Moodle) inverted. The licence design resolution (J1-J3) has its
// own table in licence-cases.test.ts; here: J4 versions of held copies, J5 appeals stop the purge, J6
// credits independent of bank size, K1 the rightful exemption by content, K2 the unauthenticated
// pools, K3 per-kind flag caps, and the non-blocking items.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { addressCaps, preReadBody } from "../../src/pool/preread.ts";
import { bytesReserved, unauthReserved, UNAUTH_POOLS } from "../../src/pool/shared.ts";
import { API, CountingStore, publish, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const withOrigin = (r: any) =>
  r.body.payload.bank.questions.map((x: Q, i: number) => ({ ...x, origin: { item_id: r.body.item.id, qid: r.body.questions[i].qid, publish_id: r.body.item.publish_id, version: r.body.item.version, author_name: r.body.item.author_name, license: r.body.license, form: r.body.item.form } }));
const holds = (w: World, id: string) => w.q<{ reason: string; source: string }>(`SELECT reason, source FROM item_holds WHERE item_id = ? ORDER BY reason, source`, id);
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const versions = async (w: World, u: User, ids: string[]) => (await w.json<any>("POST", "/items/versions", { user: u, json: { ids } })).body.items.map((x: Q) => [x.status, x.reason ?? null]);
const filler = (n: number) => Array.from({ length: n }, (_, i) => q(`Filler statement number ${i} about igneous rocks and their formation.`, { kind: { Tf: { correct: i % 2 === 0 } } }));

// ---- J4 / J5 --------------------------------------------------------------------------------------------

test("J4 (p1): a HELD copy is under_review until it is purged, never removed while an unblock can still lift it", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user("teacher", "Alice"), C = w.user("teacher", "Carl"), I = w.user("teacher", "Ivy");
  const a = (await publish(w, A, bankOf([q("Jenny Smith in period 3 scored 42 on the test; explain her error."), q("Alice's photosynthesis question?")]))).body;
  w.advance(DAY);
  const ca = await imp(w, C, a.item_id as string);
  const c = (await publish(w, C, bankOf([withOrigin(ca)[0], q("Carl's own plate tectonics question?")]))).body;
  await imp(w, I, c.item_id as string);
  await imp(w, I, a.item_id as string);
  await w.purge(adm, a.item_id as string, { questions: [0] });
  w.advance(15 * DAY);
  await w.pool.daily();
  assert.deepEqual(await versions(w, I, [a.item_id as string, c.item_id as string]), [["removed", "privacy"], ["under_review", "held"]], "the purged original is gone; the held copy is not (round 12: a hold says only 'held')");
  for (const b of w.q<{ kind: string; hash: string }>(`SELECT kind, hash FROM privacy_blocks`)) await w.admin(adm, "POST", "/admin/pool/unblock", { kind: b.kind, hash: b.hash });
  assert.deepEqual((await versions(w, I, [c.item_id as string]))[0], ["current", null], "and an unblock lifts it");
});

test("J5 (p2): an open appeal stops the purge; the admin is reminded; unanswered for 60 days the item is restored; a confirmed purge goes ahead", async () => {
  const run = async () => {
    const w = new World();
    const adm = w.user("admin"), A = w.user("teacher", "Alice");
    const a = (await publish(w, A, bankOf([q("Fictional student Jenny Smith scored 42; explain the mistake."), q("Alice's volcano question?")]))).body;
    await w.purge(adm, a.item_id as string, { questions: [0] });
    w.advance(DAY);
    assert.equal((await w.json("POST", `/items/${a.item_id}/appeal`, { user: A, json: { note: "Jenny Smith is a fictional name from the textbook." } })).status, 202);
    return { w, adm, A, id: a.item_id as string };
  };
  const row = (w: World, id: string) => w.q<{ removed_reason: string | null; purged: number }>(`SELECT removed_reason, purged_at IS NOT NULL AS purged FROM items WHERE id = ?`, id)[0];
  {
    const { w, adm, A, id } = await run();
    w.advance(14 * DAY);
    await w.pool.daily();
    assert.deepEqual(row(w, id), { removed_reason: "privacy", purged: 0 }, "not purged while the appeal is open");
    assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'appeal_overdue' AND item_id = ?`, id).length, 1, "the admin is reminded");
    assert.equal((await w.admin(adm, "POST", "/admin/pool/privacy-restore", { item_id: id })).status, 200, "still restorable after the grace window");
    assert.equal((await get(w, A, id)).status, 200);
  }
  {
    const { w, id } = await run();
    w.advance(61 * DAY);
    await w.pool.daily();
    // Round 10 L1: a PRIVACY removal is never restored by default: escalated, still quarantined, not purged.
    assert.deepEqual(row(w, id), { removed_reason: "privacy", purged: 0 }, "60 days unanswered: escalated, never restored, never purged");
    assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'appeal_escalated' AND item_id = ?`, id).length, 1);
  }
  {
    const { w, adm, id } = await run();
    assert.equal((await w.admin(adm, "POST", "/admin/pool/appeal-decision", { item_id: id, decision: "purge", note: "real student data" })).status, 200);
    w.advance(14 * DAY);
    await w.pool.daily();
    assert.deepEqual(row(w, id), { removed_reason: "privacy", purged: 1 }, "an explicitly confirmed purge goes ahead");
  }
});

// ---- J6 -------------------------------------------------------------------------------------------------

test("J6 (b1): every contributor is credited in a 1-question and a 1000-question bank, and downstream copies keep them", async () => {
  for (const size of [1, 1000]) {
    const w = new World();
    const T = w.user("teacher", "Tess"), U = w.user("teacher", "Uma");
    const Q0 = q("State Newton's second law of motion in words and as an equation.");
    for (let k = 0; k < 6; k++) {
      const Ak = w.user("teacher", `Orig${k}`), Bk = w.user("teacher", `Copier${k}`);
      const a = (await publish(w, Ak, bankOf([Q0]), { license: "CC-BY-4.0" })).body;
      w.advance(1000);
      const b = (await publish(w, Bk, bankOf(withOrigin(await imp(w, Bk, a.item_id as string))), { license: "CC-BY-4.0" })).body;
      w.advance(1000);
      await imp(w, T, b.item_id as string);
    }
    w.advance(DAY);
    const r = await publish(w, T, bankOf([Q0, ...filler(size - 1)]), { license: "CC-BY-4.0" });
    assert.equal(r.status, 201);
    const names = (await get(w, w.user(), r.body.item_id as string)).body.questions[0].chain.map((l: Q) => l.author_name);
    assert.deepEqual(names, [0, 1, 2, 3, 4, 5].flatMap((k) => [`Orig${k}`, `Copier${k}`]), `bank of ${size}`);
    const iu = await imp(w, U, r.body.item_id as string);
    const u = (await publish(w, U, bankOf([withOrigin(iu)[0]]), { license: "CC-BY-4.0" })).body;
    assert.equal((await get(w, w.user(), u.item_id as string)).body.questions[0].chain.length, 13, "12 + Tess, downstream");
  }
});

test("J6: past CHAIN_MAX links a chain shows 'and N earlier contributors'; the stored credits (up to 64) stay complete", async () => {
  const w = new World();
  const T = w.user("teacher", "Tess");
  const Q0 = q("Name the longest river in Africa and the countries it crosses.");
  for (let k = 0; k < 40; k++) {
    const a = (await publish(w, w.user("teacher", `A${k}`), bankOf([Q0]), { license: "CC-BY-4.0" })).body;
    w.advance(1000);
    await imp(w, T, a.item_id as string);
  }
  const r = (await publish(w, T, bankOf([Q0]), { license: "CC-BY-4.0" })).body;
  const stored = JSON.parse(w.q<{ chain_json: string }>(`SELECT chain_json FROM item_questions WHERE publish_id = ?`, r.publish_id)[0]!.chain_json);
  assert.equal(stored.length, 32, "at most 32 sources are examined per question (EVIDENCE_PER_POSITION): all credited");
  const shown = (await get(w, w.user(), r.item_id as string)).body.questions[0].chain;
  assert.equal(shown.length, 32);
});

// ---- K1 -------------------------------------------------------------------------------------------------

test("K1 (e4): the rightful exemption follows CONTENT: a copier who imported Rita's item is still held for the textbook questions; a later import releases nothing", async () => {
  const w = new World();
  const adm = w.user("admin"), R = w.user("teacher", "Rita"), X = w.user("teacher", "Xavier"), C = w.user("teacher", "Copier"), S = w.user("teacher", "Stranger"), Z = w.user("teacher", "Zed");
  const ritaQ = q("Rita's original question about mitochondria and ATP synthase in detail?");
  const textbook = [q("Copyrighted textbook question one about the Calvin cycle regeneration step?"), q("Copyrighted textbook question two about photorespiration and RuBisCO?")];
  const r = (await publish(w, R, bankOf([ritaQ]), { license: "CC-BY-4.0" })).body;
  w.advance(DAY);
  const x = (await publish(w, X, bankOf([ritaQ, ...textbook]), { license: "CC-BY-4.0" })).body;
  // Round 11 P4: a rightful item must contain EVERY blocked hash, earlier: a selection with the
  // textbook questions (Rita never had them) is refused with the per-hash reasons.
  const bad = await w.takedown<any>(adm, await notice(w, adm), x.item_id, { rightful_item_id: r.item_id, exempt_authors: [R.id], questions: [0, 1, 2] });
  assert.equal(bad.status, 409);
  assert.deepEqual(bad.body.reasons, ["not_earlier_in_rightful"]);
  assert.equal(bad.body.hashes.length, 4, "two questions, each blocked by its exact and its normalised key (round 12 M6)");
  const td = await w.takedown<any>(adm, await notice(w, adm), x.item_id, { exempt_authors: [R.id], questions: [1, 2] });
  assert.equal(td.status, 200);
  const s1 = (await publish(w, S, bankOf(textbook))).body;
  assert.equal(holds(w, s1.item_id as string)[0]?.reason, "blocklist", "a stranger's re-upload is held");
  await imp(w, C, r.item_id as string);
  const c1 = (await publish(w, C, bankOf(textbook))).body;
  assert.equal(holds(w, c1.item_id as string)[0]?.reason, "blocklist", "importing Rita's item once does not exempt the textbook questions");
  const rz = await imp(w, Z, r.item_id as string);
  const z1 = (await publish(w, Z, bankOf(withOrigin(rz)))).body;
  assert.deepEqual(holds(w, z1.item_id as string), [], "Rita's own question (the rightful item contains it) is never held");
  await imp(w, S, r.item_id as string);
  w.advance(31 * DAY);
  await w.pool.daily();
  assert.equal(holds(w, s1.item_id as string)[0]?.reason, "blocklist", "upheld: still held (its blocked hashes are not Rita's)");
});

// ---- K2 -------------------------------------------------------------------------------------------------

const stalling = (n: number) => {
  let sent = false;
  return new ReadableStream<Uint8Array>({
    pull(c) {
      if (!sent) {
        sent = true;
        c.enqueue(new Uint8Array(n).fill(32));
        return;
      }
      return new Promise(() => {});
    },
  });
};
const pre = (path: string, ip: string, body: BodyInit, len: string, auth?: string, who: (r: Request) => Promise<{ userId: string; trust?: string } | null> = async () => null) =>
  preReadBody(new Request(`${API}${path}`, { method: "POST", headers: { "Content-Type": "application/json", "Content-Length": len, "CF-Connecting-IP": ip, ...(auth ? { Authorization: auth } : {}) }, body, duplex: "half" } as RequestInit), who);

test("K2 (e1): nested prefix caps; sign-in and sign-up have their own pools; a trusted bearer request is never refused for unauthenticated load; a 2 s deadline", async () => {
  // 256 /64s of one /48 hold at most 8 bodies (the /48 cap), not 512.
  const pend: Promise<unknown>[] = [];
  for (let net = 0; net < 256; net++) for (let k = 0; k < 2; k++) pend.push(pre("/auth/signup", `2001:db8:aa:${net.toString(16)}00::${k + 1}`, stalling(8000), "8192"));
  await new Promise((r) => setTimeout(r, 100));
  assert.ok(unauthReserved("signup") <= 8 * 8192, `held ${unauthReserved("signup")}`);
  // Fill the general pool to the brim from many networks; sign-in still passes from a fresh address.
  for (let a = 0; a < 40; a++) for (let k = 0; k < 4; k++) pend.push(pre("/auth/forgot", `198.${a}.0.${k}`, stalling(8000), "8192"));
  await new Promise((r) => setTimeout(r, 100));
  const room = UNAUTH_POOLS.general - unauthReserved("general");
  if (room > 0) pend.push(pre("/auth/forgot", "198.250.0.1", stalling(room), "8192"));
  await new Promise((r) => setTimeout(r, 20));
  assert.equal(unauthReserved("general"), UNAUTH_POOLS.general, "the general pool is full to the byte");
  const login = JSON.stringify({ email: "admin@school.example.org", password: "correct horse battery staple" });
  assert.ok(!((await pre("/auth/login", "198.51.100.20", login, String(login.length))) instanceof Response), "sign-in has its own pool");
  // A trusted account's bearer request is read under the account, whatever the unauthenticated load.
  const admin = await pre("/admin/pool/hold", "198.0.0.1", "{}", "2", "Bearer good", async () => ({ userId: "admin-1", trust: "admin" }));
  assert.ok(!(admin instanceof Response));
  // A pending account counts as unauthenticated (its address's caps: 4 on one IPv4 address).
  for (let k = 0; k < 4; k++) pend.push(pre("/auth/signup", "198.0.0.9", stalling(10), "8192"));
  await new Promise((r) => setTimeout(r, 20));
  const pending = await pre("/auth/signup", "198.0.0.9", stalling(10), "8192", "Bearer p", async () => ({ userId: "p-1", trust: "pending" }));
  assert.equal((pending as Response).status, 429, "198.0.0.9 already holds 4 unauthenticated bodies");
  // The deadline: 2 s for an unauthenticated body.
  const t = Date.now();
  const slow = await pre("/auth/login", "203.0.113.50", stalling(100), "2000");
  assert.equal((slow as Response).status, 408);
  assert.ok(Date.now() - t < 3000);
  assert.equal(bytesReserved(), 0, "none of it touched the isolate budget");
  // Every stalled body ends within the 2 s deadline, and everything it held is released.
  await Promise.all(pend);
  assert.equal(unauthReserved(), 0);
});

test("K2 (e2): a client that aborts mid-body gets a clean 400; authentication failing reads under the isolate, never the unauthenticated pools", async () => {
  let n = 0;
  const aborting = new ReadableStream<Uint8Array>({
    pull(c) {
      if (n++ === 0) return c.enqueue(new Uint8Array(4000));
      c.error(new Error("client went away"));
    },
  });
  const r = await pre("/auth/forgot", "192.0.2.1", aborting, "8192");
  assert.equal((r as Response).status, 400);
  assert.equal(((await (r as Response).json()) as Q).error, "body_aborted");
  const before = unauthReserved();
  const ok = await pre("/auth/forgot", "192.0.2.3", "{}", "2", "Bearer x", async () => {
    throw new Error("db down");
  });
  assert.ok(!(ok instanceof Response));
  assert.equal(unauthReserved(), before);
  assert.deepEqual(addressCaps("2001:db8::1").map((x) => x.cap), [2, 4, 8, 16]);
  assert.deepEqual(addressCaps("::ffff:192.0.2.1"), [{ key: "v4:192.0.2.1", cap: 4 }, { key: "v4:192.0.2/24", cap: 8 }]);
});

// ---- K3 -------------------------------------------------------------------------------------------------

test("K3 (e3): 25 junk origin_unverified publishes fold into ONE flag with a count; independence claims and soft-licence flags are never silenced", async () => {
  const w = new World();
  const bob = w.user("teacher", "Bob"), alice = w.user("teacher", "Alice");
  const b = (await publish(w, bob, bankOf([q("Describe the full Krebs cycle steps including every intermediate in order."), q("Explain the oxygen debt after exercise and lactate.")]), { license: "CC-BY-NC-4.0" })).body;
  await get(w, alice, b.item_id as string);
  w.advance(DAY);
  for (let i = 0; i < 25; i++)
    assert.equal(
      (await publish(w, alice, bankOf([q(`Filler question number ${i} about tectonic plate boundaries.`, { origin: { item_id: "ab".repeat(16), qid: "cd".repeat(16), publish_id: "x".repeat(22), version: 1, author_name: "x", license: "CC0-1.0", form: "full" } })]), { license: "CC0-1.0" })).status,
      201,
    );
  const unv = w.q<{ detail: string }>(`SELECT detail FROM admin_flags WHERE kind = 'origin_unverified' AND user_id = ?`, alice.id);
  assert.equal(unv.length, 1);
  assert.deepEqual([JSON.parse(unv[0]!.detail).publishes, JSON.parse(unv[0]!.detail).count], [25, 25]);
  // A changed copy of the NC read as CC0 (soft) and an independence claim, after the burst.
  const soft = await publish(w, alice, bankOf([q("Describe the full Krebs cycle steps including every intermediate in order.", { explanation: "Alice's own words" })]), { license: "CC0-1.0" });
  assert.equal(soft.status, 201);
  const ind = await publish(w, alice, bankOf([q("Explain the oxygen debt after exercise and lactate.")]), { license: "CC0-1.0", independent: [b.item_id as string], statement: "I wrote this one myself for my class." });
  assert.equal(ind.status, 201);
  const kinds = w.q<{ kind: string; agg: number | null }>(`SELECT kind, json_extract(detail, '$.aggregated') AS agg FROM admin_flags WHERE user_id = ? AND kind IN ('license_soft', 'independent_claim') ORDER BY kind`, alice.id);
  assert.deepEqual(kinds, [{ kind: "independent_claim", agg: null }, { kind: "license_soft", agg: null }], "each its own row, never folded");
});

// ---- non-blocking -----------------------------------------------------------------------------------------

test("display text is the author's (NFC); only the search column folds (x² stays x²; q=x2 finds it)", async () => {
  const w = new World();
  const A = w.user(), T = w.user();
  const r = await publish(w, A, bankOf([q("Solve x² + y² = r² for r.", { topics: ["x² topic"] })]), { meta: { title: "x² + y² = r² — Circles ½", subject: "Mathématiques", course: "Alg Ⅱ", grade: "", tags: ["X²"], standards: [] } });
  const g = (await get(w, T, r.body.item_id as string)).body.item;
  assert.deepEqual([g.title, g.subject, g.course, g.tags], ["x² + y² = r² — Circles ½", "Mathématiques", "Alg Ⅱ", ["x²", "x² topic"]]);
  assert.deepEqual((await w.json<any>("GET", "/items?q=x2", { user: T })).body.items.map((x: Q) => x.title), ["x² + y² = r² — Circles ½"]);
});

test("search: œ = oe, ü = ue as well as u, words inside hyphenated titles, and tags", async () => {
  const w = new World();
  const A = w.user(), T = w.user();
  for (const [title, tags] of [["Œuvres de Molière", []], ["Mathe für Schüler", []], ["co-operative learning", []], ["Plain title", ["photosynthesis"]]] as const)
    await publish(w, A, bankOf([q(`A question for ${title} with enough words?`)]), { meta: { title, subject: "Sci", course: "", grade: "", tags: [...tags], standards: [] } });
  const s = async (x: string) => (await w.json<any>("GET", `/items?q=${encodeURIComponent(x)}`, { user: T })).body.items.map((i: Q) => i.title);
  assert.deepEqual(await s("oeuvres"), ["Œuvres de Molière"]);
  assert.deepEqual(await s("schueler"), ["Mathe für Schüler"]);
  assert.deepEqual(await s("schuler"), ["Mathe für Schüler"]);
  assert.deepEqual(await s("cooperative"), ["co-operative learning"]);
  assert.deepEqual(await s("operative"), ["co-operative learning"]);
  assert.deepEqual(await s("photo"), ["Plain title"], "a tag prefix");
});

test("an empty JSON body (Content-Length 0) is no body on routes whose body is optional (import, undelete)", async () => {
  const w = new World();
  const A = w.user(), T = w.user();
  const a = (await publish(w, A, bankOf([q("Body test question about magnets and poles?")]))).body;
  const r = await w.call("POST", `/items/${a.item_id}/import`, { user: T, headers: { "Content-Length": "0", "Content-Type": "application/json" } });
  assert.equal(r.status, 200);
});

test("privacy blocks by default select only what no other author uses (any item); K is DMCA's", async () => {
  const w = new World();
  const adm = w.user("admin"), Att = w.user(), V1 = w.user(), V2 = w.user();
  const common = q("Name the powerhouse of the cell and describe its main function briefly.");
  const v1 = (await publish(w, V1, bankOf([common]), { visibility: "unlisted" })).body;
  w.advance(DAY);
  const a = (await publish(w, Att, bankOf([common, q("Student Jamie R. in period 3 has an IEP for reading.")]))).body;
  const v2 = (await publish(w, V2, bankOf([common]))).body;
  const p = await w.admin<any>(adm, "POST", "/admin/pool/purge-preview", { item_id: a.item_id });
  assert.deepEqual(p.body.match.map((m: Q) => [m.pos, m.selected, m.suggested]), [[0, false, true], [1, false, true]], "round 10: suggested, the admin ticks (round 14: student data is suggested whoever else pasted it)");
  // Round 14: suggested (not generic: fewer than 3 other authors' visible items); an admin who ticks
  // only the student-data question leaves the shared one alone.
  await w.purge(adm, a.item_id as string, { questions: [1] });
  assert.deepEqual([holds(w, v1.item_id as string), holds(w, v2.item_id as string)], [[], []]);
  // A generic stem (3+ other authors' visible items) is never suggested, and the preview says why.
  const w2 = new World();
  const adm2 = w2.user("admin"), Att2 = w2.user();
  for (let k = 0; k < 3; k++) await publish(w2, w2.user(), bankOf([common]));
  const a2 = (await publish(w2, Att2, bankOf([common, q("Student Jamie R. in period 3 has an IEP for reading.")]))).body;
  const p2 = await w2.admin<any>(adm2, "POST", "/admin/pool/purge-preview", { item_id: a2.item_id });
  assert.deepEqual(p2.body.match.map((m: Q) => [m.pos, m.suggested, m.generic_stem ?? false]), [[0, false, true], [1, true, false]]);
});

test("privacy: the copies a confirmed purge holds can be listed, then removed in bulk (quarantined, purged by the cron)", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user(), C = w.user(), V = w.user();
  const a = (await publish(w, A, bankOf([q("Student Jo Smith (ID 4411) failed the retake on Friday.")]))).body;
  const ca = await imp(w, C, a.item_id as string);
  const c = (await publish(w, C, bankOf([...withOrigin(ca), q("Carl's own clean question about rocks?")]))).body;
  await w.purge(adm, a.item_id as string);
  const list = await w.admin<any>(adm, "POST", "/admin/pool/purge-held-copies", { item_id: a.item_id });
  assert.deepEqual([list.body.total, list.body.items, list.body.removed], [1, [c.item_id], 0]);
  const done = await w.admin<any>(adm, "POST", "/admin/pool/purge-held-copies", { item_id: a.item_id, confirm: true });
  assert.equal(done.body.removed, 1);
  assert.equal((await get(w, V, c.item_id as string)).status, 404);
  w.advance(15 * DAY);
  await w.pool.daily();
  assert.equal(w.q<{ p: number }>(`SELECT purged_at IS NOT NULL AS p FROM items WHERE id = ?`, c.item_id)[0]!.p, 1);
});

test("account deletion: the rightful stand-in's name is resolved at read; counter-notice PII, stored chain names and prefs go", async () => {
  const w = new World();
  const adm = w.user("admin"), R = w.user("teacher", "Rita Realname"), X = w.user("teacher", "Xavier"), Z = w.user("teacher", "Zed"), O = w.user();
  const content = [q("Rita's distinctive question on the electron transport chain gradient?")];
  const r = (await publish(w, R, bankOf(content), { license: "CC-BY-4.0" })).body;
  const x = (await publish(w, X, bankOf(content), { license: "CC-BY-4.0" })).body;
  const z = (await publish(w, Z, bankOf(withOrigin(await imp(w, Z, x.item_id as string))))).body;
  await w.takedown(adm, await notice(w, adm), x.item_id, { rightful_item_id: r.item_id, exempt_authors: [R.id], questions: [] });
  w.advance(31 * DAY);
  await w.pool.daily();
  await w.json("PUT", "/items/prefs", { user: R, json: { keep_on_delete: true } });
  const { deleteContentStatements } = await import("../../src/pool/index.ts");
  await w.db.batch(deleteContentStatements(w.db, R.id, w.clock));
  w.sql.prepare(`UPDATE users SET deleted_at = ?, email = NULL, pw_hash = NULL, display_name = NULL WHERE id = ?`).run(w.clock, R.id);
  assert.deepEqual((await get(w, O, z.item_id as string)).body.questions[0].chain.map((l: Q) => l.author_name), ["deleted teacher"]);
  // (Round 11: stored chains are rewritten by the cron, in bounded batches.)
  for (let k = 0; k < 5; k++) await w.pool.hourly();
  assert.equal(w.q(`SELECT * FROM item_questions WHERE chain_json LIKE '%Rita Realname%'`).length, 0);
  assert.equal(w.q(`SELECT * FROM pool_prefs WHERE user_id = ?`, R.id).length, 0);
  // A counter-notice filed by an author who then deletes their account keeps its ids, not the person.
  const w2 = new World();
  const adm2 = w2.user("admin"), Y = w2.user();
  const y = (await publish(w2, Y, bankOf([q("Yan's question about the water cycle stages?")]))).body;
  await w2.takedown(adm2, await notice(w2, adm2), y.item_id);
  assert.equal((await w2.json("POST", `/items/${y.item_id}/counter-notice`, { user: Y, json: { signature: "Yan Q", material_desc: "m", name: "Yan Q", address: "1 Main St", phone: "555-0100", perjury_stmt: true, consent_jurisdiction: true, accept_service: true } })).status, 202);
  await w2.db.batch(deleteContentStatements(w2.db, Y.id, w2.clock));
  assert.deepEqual(w2.q(`SELECT name, address, phone, signature FROM dmca_counter_notices`), [{ name: "[deleted]", address: "[deleted]", phone: "[deleted]", signature: "[deleted]" }]);
});

test("/admin/pool/item streams the stored body under a read reservation", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user();
  const a = (await publish(w, A, bankOf([q("Admin view question?")]))).body;
  const r = await w.admin<any>(adm, "GET", "/admin/pool/item", null, `?id=${a.item_id}`);
  assert.equal(r.status, 200);
  assert.equal(r.body.payload_full.bank.questions[0].stem, "Admin view question?");
  assert.equal(bytesReserved(), 0, "released once read");
});

test("lint: no request body is read anywhere except the two readers (any identifier, any body method, destructured, piped, cloned)", () => {
  const offenders: string[] = [];
  const walk = (d: string) => {
    for (const f of readdirSync(d, { withFileTypes: true })) {
      const p = `${d}/${f.name}`;
      if (f.isDirectory()) walk(p);
      else if (/\.(ts|js|mjs)$/.test(f.name))
        for (const [k, line] of readFileSync(p, "utf8").split("\n").entries()) {
          const call = /\.(json|text|arrayBuffer|formData|blob)(<[^>]*>)?\(\s*\)|\.body\s*[!?]?\.?\s*getReader\(|\.(clone)\(\)\.(json|text|arrayBuffer)/.test(line);
          const raw = /\b(req|request|r|c\.req|ctx\.req)\.body\b/.test(line);
          // Round 11: destructuring the body off a request, handing a body to a Response, piping or
          // teeing a body, and cloning a request (the clone's body is the same stream).
          const indirect =
            /\{[^}]*\bbody\b[^}]*\}\s*=\s*(await\s+)?\w+/.test(line) ||
            /new Response\(\s*\w+(\.\w+)*\.body\b/.test(line) ||
            /\.body\s*[!?]?\.?\s*(pipeTo|pipeThrough|tee)\(/.test(line) ||
            /\b(req|request|r|c\.req|ctx\.req)\.clone\(/.test(line);
          if ((call || raw || indirect) && !/pool\/shared\.ts$|auth\/src\/http\.ts$/.test(p) && !/\/\/ r2-object/.test(line)) offenders.push(`${p}:${k + 1}: ${line.trim()}`);
        }
    }
  };
  for (const r0 of ["src", "../auth/src"]) walk(new URL(`../../${r0}`, import.meta.url).pathname);
  assert.deepEqual(offenders, []);
});

test("purges: oldest due first; one that fails waits for a backoff and is flagged, the rest go", async () => {
  const w = new World();
  const A = w.user();
  const ids: string[] = [];
  for (let i = 0; i < 3; i++) {
    const p = (await publish(w, A, bankOf([q(`Purge order question ${i}?`)]))).body;
    ids.push(p.item_id as string);
    await w.json("DELETE", `/items/${p.item_id}`, { user: A });
    w.advance(1000);
  }
  w.advance(15 * DAY);
  // The oldest item's purge fails in the database (a trigger stands in for the failure).
  w.sql.exec(`CREATE TRIGGER boom BEFORE UPDATE OF purged_at ON items WHEN NEW.id = '${ids[0]}' BEGIN SELECT RAISE(ABORT, 'boom'); END`);
  assert.equal((await w.pool.daily()).purged, 2);
  w.sql.exec(`DROP TRIGGER boom`);
  const failed = w.q<{ id: string; purge_failures: number }>(`SELECT id, purge_failures FROM items WHERE purged_at IS NULL`);
  assert.deepEqual(failed, [{ id: ids[0], purge_failures: 1 }]);
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'cron_failure' AND item_id = ?`, ids[0]).length, 1);
  assert.equal((await w.pool.daily()).purged, 0, "not before its backoff");
  w.advance(DAY + 1);
  assert.equal((await w.pool.daily()).purged, 1);
});

test("purges: an R2 delete that fails after the rows are gone is kept and retried hourly, never an orphan nobody can find", async () => {
  const w = new World();
  const A = w.user();
  const p = (await publish(w, A, bankOf([q("R2 orphan question?")]))).body;
  await w.json("DELETE", `/items/${p.item_id}`, { user: A });
  w.advance(15 * DAY);
  const del = w.r2.delete.bind(w.r2);
  w.r2.delete = async () => {
    throw new Error("r2 hiccup");
  };
  assert.equal((await w.pool.daily()).purged, 1);
  w.r2.delete = del;
  const kept = w.q<{ key: string }>(`SELECT key FROM r2_orphans`);
  assert.equal(kept.length, 1);
  assert.ok(w.r2.objects.has(kept[0]!.key));
  await w.pool.hourly();
  assert.equal(w.q(`SELECT * FROM r2_orphans`).length, 0);
  assert.ok(!w.r2.objects.has(kept[0]!.key));
});

test("a school NAT: 20 account-shares per address, and only requests that passed their account's limit count", async () => {
  const store = new CountingStore();
  const w = new World({ rateStore: store });
  const A = w.user();
  const id = (await publish(w, A, bankOf([q("NAT question about waves?")]))).body.item_id as string;
  const T = [w.user(), w.user(), w.user(), w.user()];
  for (const t of T) for (let i = 0; i < 300; i++) assert.notEqual((await w.call("GET", `/items/${id}`, { user: t, ip: "198.51.100.7" })).status, 429);
  // One account hammering past its own limit spends nothing of the address's budget.
  for (let i = 0; i < 400; i++) await w.call("GET", `/items/mine`, { user: T[0]!, ip: "198.51.100.7" });
  assert.equal((await w.call("GET", `/items/${id}`, { user: w.user(), ip: "198.51.100.7" })).status, 200, "a fifth teacher at the same school");
});

test("held publishes: at most 20 a day may end up held by a content block (any kind alike)", async () => {
  const w = new World();
  const adm = w.user("admin"), X = w.user(), M = w.user();
  const blocked = Array.from({ length: 21 }, (_, i) => q(`Blocked textbook question number ${i} about cells?`));
  const x = (await publish(w, X, bankOf(blocked))).body;
  await w.takedown(adm, await notice(w, adm), x.item_id);
  for (let i = 0; i < 20; i++) assert.equal((await publish(w, M, bankOf([blocked[i]!]))).status, 201);
  // Round 14 (B1): past the cap nothing is refused; a matching publish is held for review (probe_cap,
  // kept whatever happens to the block), the admin is alerted, an unrelated publish goes through.
  const r = await publish(w, M, bankOf([blocked[20]!]));
  assert.equal(r.status, 201);
  assert.ok(w.q(`SELECT * FROM item_holds WHERE item_id = ? AND source = 'probe_cap'`, r.body.item_id).length === 1);
  const u = await publish(w, M, bankOf([q("An unrelated question the same day?")]));
  assert.equal(u.status, 201);
  assert.deepEqual(w.q(`SELECT * FROM item_holds WHERE item_id = ?`, u.body.item_id), []);
  assert.ok(w.q(`SELECT * FROM admin_flags WHERE kind = 'cap' AND user_id = ?`, M.id).length >= 1);
});
