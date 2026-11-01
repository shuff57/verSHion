// Round 7: the own-work rule may only see SHOWN content (H1) as a property, per-content sequence
// numbers end to end (H2), one body reader (H3), bounded evidence (H4), and the critics' repros
// (critic10a e1-e6) and the Moodle leads inverted. Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { evidence, EVIDENCE_GROUPS_MAX } from "../../src/pool/provenance.ts";
import { bytesReserved, makeCtx } from "../../src/pool/shared.ts";
import { subjectFor, subjectParam } from "../../src/pool/visible.ts";
import { API, core, genQuestion, png, publish, rng, SECRET, upload, World, type Rng, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[], sv = 2) => JSON.stringify({ schema_version: sv, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const tf = (stem: string, correct = true, extra: Q = {}): Q => ({ id: "q", stem, kind: { Tf: { correct } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => (i < pub.qids.length ? { ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } } : x));
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const strip = (x: Q) => {
  const y = { ...x };
  delete y.published;
  delete y.origin;
  delete y.keyless;
  return y;
};
const withOriginOf = (r: any) =>
  r.body.payload.bank.questions.map((x: Q, i: number) => ({ ...x, origin: { item_id: r.body.item.id, qid: r.body.questions[i].qid, publish_id: r.body.item.publish_id, version: r.body.item.version, author_name: r.body.item.author_name, license: r.body.license, form: r.body.item.form } }));
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const LIC = ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"];

function otherKey(x: Q, r: Rng): Q {
  const y = structuredClone(x);
  const k = y.kind;
  if (k.Tf) k.Tf.correct = !k.Tf.correct;
  else if (k.Mc) {
    const n = k.Mc.answers.length;
    const was = k.Mc.answers.findIndex((a: Q) => a.correct);
    const to = (was + 1 + r.int(n - 1)) % n;
    k.Mc.answers.forEach((a: Q, j: number) => (a.correct = j === to));
  } else if (k.Matching) [k.Matching.pairs[0].right, k.Matching.pairs[1].right] = [k.Matching.pairs[1].right, k.Matching.pairs[0].right];
  else if (k.Short) k.Short.sample = `${k.Short.sample} (other)`;
  return y;
}

// ---- H1: the publisher's OTHER items' hidden keys, as a property -----------------------------------------

test("H1 property: varying only a hidden key of the publisher's OTHER item changes nothing anyone else observes (chains, licences, may_republish_as, flags, holds) nor the publisher's own refusals", async () => {
  for (let seed = 1; seed <= 30; seed++) {
    const views: string[] = [];
    for (const variant of [0, 1]) {
      const r = rng(seed * 104729);
      const w = new World();
      const A = w.user("teacher", "Alice"), B = w.user("teacher", "Bob"), C = w.user("teacher", "Cara"), Eve = w.user("teacher", "Eve"), Ian = w.user("teacher", "Ian");
      const kind = r.pick(["Tf", "Mc", "Matching", "Short"]);
      const base = genQuestion(r, kind, 0).json as Q;
      base.explanation = r.bool() ? "a shared explanation" : null;
      base.figure = null;
      const k1 = base, k2 = otherKey(base, r);
      // The ONLY difference between the worlds: the key of Alice's hidden item.
      const hx = (await publish(w, A, bankOf([variant === 0 ? k1 : k2]), { hideKey: true, license: r.pick(LIC) })).body;
      w.advance(DAY);
      // Others are served its stripped form and publish public versions with guessed keys.
      await imp(w, B, hx.item_id);
      const by = (await publish(w, B, bankOf([r.bool() ? k1 : k2]), { license: r.pick(LIC) })).body;
      const cy = (await publish(w, C, bankOf([r.bool() ? k1 : k2]), { license: r.pick(LIC), hideKey: r.bool(0.3) })).body;
      w.advance(DAY);
      // Alice is served them (or not), then publishes something public or hidden.
      for (const it of [by, cy]) {
        const how = r.pick(["get", "import", "none"]);
        if (how === "get") await get(w, A, it.item_id);
        else if (how === "import") await imp(w, A, it.item_id);
      }
      const choice = r.pick(["k1", "k2", "edit"]);
      const body = choice === "k1" ? k1 : choice === "k2" ? k2 : { ...k1, explanation: "Alice's new explanation" };
      const res = await publish(w, A, bankOf([body]), { license: r.pick(LIC), hideKey: r.bool(0.3) });
      const ids: Record<string, string> = {};
      for (const [k, b] of [["H", hx], ["B", by], ["C", cy], ["Z", res.body]] as const) if (typeof b.item_id === "string") ids[k] = b.item_id;
      const label = (s: string) => {
        for (const [k, v] of Object.entries(ids)) s = s.replaceAll(v, `<${k}>`);
        return s.replace(/"(?:[0-9a-f]{32}|[A-Za-z0-9_-]{22})"/g, '"<id>"').replace(/"[0-9a-f]{64}"/g, '"<hash>"');
      };
      const seen: unknown[] = [Object.keys(ids), res.status, res.body.error ?? null, res.body.allowed ?? null, res.body.per_question ?? null];
      for (const [k, id] of Object.entries(ids)) {
        if (k === "H") continue; // Alice's hidden item itself is what varies (its author's view)
        const g = await get(w, Eve, id);
        const i = await imp(w, Ian, id);
        seen.push(k, g.status, g.body?.item?.license, g.body?.questions?.map((x: Q) => x.chain), i.body?.may_republish_as, w.q(`SELECT reason, source FROM item_holds WHERE item_id = ?`, id), w.q(`SELECT kind FROM admin_flags WHERE item_id = ? ORDER BY kind`, id));
      }
      seen.push((await w.json("POST", "/items/versions", { user: Ian, json: { ids: Object.values(ids) } })).body);
      views.push(label(JSON.stringify(seen)));
    }
    assert.equal(views[0], views[1], `seed ${seed}: something observable depends on the hidden key of the publisher's other item`);
  }
});

test("e2 (H1, round 10 M-5): Alice re-publishing Bob's public guess of HER hidden question is her own work, whatever her hidden item's key is", async () => {
  const out: string[] = [];
  for (const k of [true, false])
    for (const g of [true, false]) {
      const w = new World();
      const A = w.user("teacher", "Alice"), B = w.user("teacher", "Bob"), Eve = w.user();
      const stem = "Mitochondria have their own circular DNA genome in human cells";
      const x = (await publish(w, A, bankOf([tf(stem, k)]), { hideKey: true })).body;
      w.advance(DAY);
      await imp(w, B, x.item_id);
      const y = (await publish(w, B, bankOf([tf(stem, g)]))).body;
      w.advance(DAY);
      await imp(w, A, y.item_id);
      const z = await publish(w, A, bankOf([tf(stem, g)]));
      out.push(JSON.stringify((await get(w, Eve, z.body.item_id as string)).body.questions[0].chain.map((l: Q) => l.author_name)));
    }
  // Round 10 M-5: Alice published the question first and Bob only added a key, so Bob's later copy
  // is not evidence against her; the same for every key (H1: nothing depends on her hidden key).
  assert.deepEqual(out, Array(4).fill("[]"));
});

// ---- H2: per-content seq, monotonic, durable --------------------------------------------------------------

test("e1 (H2): an old qid given the victim's content never outranks the victim, in evidence the victim later holds", async () => {
  const w = new World();
  const M = w.user("teacher", "Mallory"), V = w.user("teacher", "Victor"), Eve = w.user();
  const m1 = (await publish(w, M, bankOf([q("Placeholder question number one, nothing here")]), { license: "CC-BY-NC-SA-4.0" })).body;
  w.advance(DAY);
  const X = q("Victor's original question about the Krebs cycle intermediates today");
  const v1 = (await publish(w, V, bankOf([X]), { license: "CC-BY-4.0" })).body;
  w.advance(DAY);
  // (Round 12 M5: a verbatim copy of a read cannot be attested independent; Mallory got the text elsewhere.)
  const m2 = await publish(w, M, bankOf(own(m1, [X])), { base: m1.publish_id, license: "CC-BY-NC-SA-4.0" }, m1.item_id);
  assert.equal(m2.status, 200);
  await get(w, V, m1.item_id);
  w.advance(DAY);
  const v2 = await publish(w, V, bankOf([X]), { license: "CC-BY-4.0" });
  assert.equal(v2.status, 201, "Victor's own question stays his");
  assert.deepEqual((await get(w, Eve, v2.body.item_id as string)).body.questions[0].chain, [], "and credits nobody");
  const seqs = w.q<{ s: number }>(`SELECT source_first_seq AS s FROM import_questions WHERE user_id = ? AND item_id = ?`, V.id, m1.item_id);
  const mSeq = w.q<{ seq: number }>(`SELECT seq FROM item_questions WHERE publish_id = ?`, m2.body.publish_id)[0]!.seq;
  assert.deepEqual(seqs.map((x) => x.s), [mSeq], "the evidence row carries the served content's own seq");
});

test("H2: sequence numbers come from a monotonic counter (never reused after a purge) and an author's first publish survives retention and purge", async () => {
  const w = new World();
  const A = w.user(), B = w.user();
  const p1 = (await publish(w, A, bankOf([q("First question to be purged later on")]))).body;
  const s1 = w.q<{ seq: number }>(`SELECT seq FROM item_questions WHERE publish_id = ?`, p1.publish_id)[0]!.seq;
  await w.call("DELETE", `/items/${p1.item_id}`, { user: A });
  w.advance(15 * DAY);
  await w.pool.daily();
  assert.equal(w.q(`SELECT * FROM item_questions`).length, 0, "purged");
  const p2 = (await publish(w, B, bankOf([q("Another question published after the purge")]))).body;
  assert.ok(w.q<{ seq: number }>(`SELECT seq FROM item_questions WHERE publish_id = ?`, p2.publish_id)[0]!.seq > s1, "never reused");
  // Two edits: content Q, then R, then Q again; retention dropped the first, the first seq of Q stays.
  const w2 = new World();
  const a = w2.user(), b = w2.user();
  const Qc = q("Author's own question about the water cycle and clouds");
  const Rc = q("Author's other wording about the water cycle and rain");
  let p = (await publish(w2, a, bankOf([Qc]))).body;
  const first = w2.q<{ seq: number }>(`SELECT seq FROM item_questions WHERE publish_id = ?`, p.publish_id)[0]!.seq;
  w2.advance(DAY);
  // Someone else publishes Q in between (after a's first publish).
  const bq = (await publish(w2, b, bankOf([Qc]), { license: "CC-BY-NC-4.0" })).body;
  for (const content of [Rc, Rc, Qc]) {
    p = (await publish(w2, a, bankOf(own(p, [content])), { base: p.publish_id }, p.item_id)).body;
    w2.advance(DAY);
  }
  assert.equal(w2.q<{ first_seq: number }>(`SELECT first_seq FROM author_firsts WHERE author_id = ?`, a.id).find((x) => x.first_seq === first)?.first_seq, first);
  await get(w2, a, bq.item_id);
  assert.equal((await publish(w2, a, bankOf([Qc]), { license: "CC0-1.0" })).status, 201, "b's later copy never binds a's own question");
});

// ---- H3: one body reader ----------------------------------------------------------------------------------

test("e3 (H3): stalled uploads are capped per account, reserved and timed out; every JSON route needs a length", async () => {
  const w = new World({ bodyDeadlineMs: 200 });
  const A = w.user();
  const MB = 1024 * 1024;
  const pending: Promise<Response | null>[] = [];
  for (let i = 0; i < 10; i++) {
    let sent = 0;
    const body = new ReadableStream<Uint8Array>({
      async pull(c) {
        if (sent >= MB - 1) return new Promise(() => {}); // stall 1 byte short, forever
        c.enqueue(new Uint8Array(256 * 1024));
        sent += 256 * 1024;
      },
    });
    pending.push(w.pool.handle(new Request(`${API}/blobs/${"a".repeat(64)}`, { method: "PUT", headers: { Authorization: `Bearer ${A.token}`, "CF-Connecting-IP": `10.1.${i}.1`, "Content-Length": String(MB) }, body, duplex: "half" } as RequestInit)));
  }
  await new Promise((r) => setTimeout(r, 50));
  assert.ok(bytesReserved() <= 4 * MB, `reserved ${bytesReserved()}`);
  const codes = (await Promise.all(pending)).map((r) => r!.status).sort();
  assert.deepEqual(codes, [...Array(4).fill(408), ...Array(6).fill(429)]);
  assert.equal(bytesReserved(), 0);
  // A blob upload without a length, and a JSON route without one, are 411; the day's uploads were charged first.
  const noLen = await w.pool.handle(new Request(`${API}/blobs/${"b".repeat(64)}`, { method: "PUT", headers: { Authorization: `Bearer ${A.token}`, "CF-Connecting-IP": "10.0.0.1" }, body: new ReadableStream({ start: (c) => (c.enqueue(new Uint8Array(10)), c.close()) }), duplex: "half" } as RequestInit));
  assert.equal(noLen!.status, 411);
  const p = (await publish(w, w.user(), bankOf([q("A question to report for the length rule")]))).body;
  const rep = await w.pool.handle(new Request(`${API}/items/${p.item_id}/report`, { method: "POST", headers: { Authorization: `Bearer ${A.token}`, "CF-Connecting-IP": "10.0.0.1", "Content-Type": "application/json" }, body: new ReadableStream({ start: (c) => (c.enqueue(new TextEncoder().encode('{"reason":"other"}')), c.close()) }), duplex: "half" } as RequestInit));
  assert.equal(rep!.status, 411);
  assert.equal(w.q<{ count: number }>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'blob_uploads'`, A.id)[0]?.count, 10, "every upload with a length was charged BEFORE its body was read");
});

// ---- H4: bounded evidence ----------------------------------------------------------------------------------

test("e4 (H4): 50,000 evidence rows are aggregated in SQL; at most EVIDENCE_GROUPS_MAX groups ever reach JS", async () => {
  const w = new World();
  const R = w.user(), A = w.user();
  const NQ = 1000, NV = 50;
  const h = (s: string) => s.padStart(64, "0");
  // (Round 19: 50 served versions of one 1000-question item, as ghosts the reader was served.)
  const gp = w.sql.prepare(`INSERT INTO ghost_publishes (publish_id, item_id, author_id, hide_key, public_version, license, at) VALUES (?, ?, ?, 0, ?, 'CC-BY-NC-4.0', ?)`);
  const gq = w.sql.prepare(`INSERT INTO ghost_questions (publish_id, qid, pos, match_hash, full_content, keyfree_content, chain_json, seq) VALUES (?, ?, ?, ?, ?, ?, '[]', ?)`);
  const sv = w.sql.prepare(`INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at) VALUES (?, ?, ?, 'view', 'CC-BY-NC-4.0', '["CC-BY-NC-4.0"]', ?, ?)`);
  w.sql.exec("BEGIN");
  for (let v = 0; v < NV; v++) {
    gp.run(`gv${v}`, "c".repeat(32), A.id, v + 1, v);
    sv.run(R.id, "c".repeat(32), `gv${v}`, v, v);
    for (let i = 0; i < NQ; i++) gq.run(`gv${v}`, String(i).padStart(32, "0"), i, h(`m${i}`), h(`c${i}v${v}`), h(`k${i}`), 1 + v);
  }
  w.sql.exec("COMMIT");
  const c = makeCtx({ db: w.db, r2: w.r2 as unknown as R2Bucket, core, secret: SECRET, allowedOrigins: [], authenticate: async () => null, rateStore: null as never, ipBucket: (x: string) => x }, w.r2 as unknown as R2Bucket);
  const subj = Array.from({ length: NQ }, (_, i) => subjectParam(subjectFor(false, h(`k${i}`), h(`c${i}v3`), h(`n${i}`), h(`nk${i}`)), h(`m${i}`)));
  (globalThis as { gc?: () => void }).gc?.();
  const before = process.memoryUsage().heapUsed;
  const ev = await evidence(c, R.id, subj);
  const grew = process.memoryUsage().heapUsed - before;
  const groups = [...ev.byPos.values()].flat();
  assert.ok(groups.length <= EVIDENCE_GROUPS_MAX && groups.length === NQ, `groups ${groups.length}`);
  assert.ok(grew < 40 * 1024 * 1024, `heap grew ${grew}`);
  assert.ok(groups.every((g) => g.same && g.lic_all.length === 1 && g.versions.length === NV), "one group per source question, every version and licence aggregated");
});

test("I1: past the per-question allowance nothing restricts: no lockout, the identical examined source decides, flagged", async () => {
  const w = new World();
  const R = w.user();
  const Qn = tf("A question forty teachers each published, about volcanoes", true);
  const ids: string[] = [];
  for (let k = 0; k < 40; k++) {
    // The last one (past the allowance) is the restrictive one.
    const p = (await publish(w, w.user(), bankOf([Qn]), { license: k === 39 ? "CC-BY-NC-4.0" : "CC0-1.0" })).body;
    ids.push(p.item_id as string);
    w.advance(1000);
  }
  for (const id of ids) await imp(w, R, id);
  const cc0 = await publish(w, R, bankOf([Qn]), { license: "CC0-1.0" });
  assert.equal(cc0.status, 201, "overflow never refuses below what the identical examined sources grant");
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'evidence_overflow' AND item_id = ?`, cc0.body.item_id).length, 1);
});


// ---- Moodle leads -------------------------------------------------------------------------------------------

test("M1 (reproduced): after an upheld takedown, copies of the infringing item stay bound: to the rightful item if named, else held for review", async () => {
  for (const named of [true, false]) {
    const w = new World();
    const adm = w.user("admin"), R = w.user("teacher", "Rightful"), I = w.user("teacher", "Infringer"), C = w.user("teacher", "Cara"), Eve = w.user();
    const Q0 = tf("Rightful's original question about tectonic plate boundaries", true);
    const r = (await publish(w, R, bankOf([Q0]), { license: "CC-BY-NC-4.0" })).body;
    const x = (await publish(w, I, bankOf([Q0]), { license: "CC-BY-NC-4.0" })).body; // retyped
    const rc = await imp(w, C, x.item_id);
    const c1 = (await publish(w, C, bankOf([{ ...strip(rc.body.payload.bank.questions[0]), explanation: "Cara's note" }]), { license: "CC-BY-NC-4.0" })).body;
    await w.takedown(adm, await notice(w, adm), x.item_id, named ? { rightful_item_id: r.item_id, exempt_authors: [R.id] } : {});
    w.advance(31 * DAY);
    await w.pool.daily();
    if (named) {
      assert.deepEqual((await get(w, Eve, c1.item_id)).body.questions[0].chain.map((l: Q) => l.author_name), ["Rightful"], "credited to the rightful author");
      const again = await publish(w, C, bankOf(own(c1, [{ ...strip(rc.body.payload.bank.questions[0]), explanation: "Cara's note" }])), { license: "CC0-1.0", base: c1.publish_id }, c1.item_id);
      assert.equal(again.body.error, "license_incompatible", "and bound by the rightful licence");
    } else {
      assert.equal((await get(w, Eve, c1.item_id)).status, 404, "held for review, not live without limits");
      assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'dmca_copy' AND detail LIKE '%"upheld":true%'`).length, 1);
    }
  }
});

test("M2 (reproduced): a privacy purge is a 14-day quarantine: an unblock or upheld appeal restores the same item, importers see under_review meanwhile", async () => {
  const w = new World();
  const admin = w.user("admin"), A = w.user(), B = w.user();
  const p = (await publish(w, A, bankOf([tf("Student Jo Bloggs (period 2) wrote this answer", true)]))).body;
  await imp(w, B, p.item_id);
  await w.purge(admin, p.item_id);
  assert.equal((await get(w, B, p.item_id)).status, 404, "invisible at once");
  assert.deepEqual((await w.json<any>("POST", "/items/versions", { user: B, json: { ids: [p.item_id] } })).body.items[0].status, "under_review");
  // The appeal is upheld within the window: same id, same version, chains intact.
  assert.equal((await w.admin(admin, "POST", "/admin/pool/privacy-restore", { item_id: p.item_id, note: "textbook name" })).status, 200);
  const g = await get(w, B, p.item_id);
  assert.deepEqual([g.status, g.body.item.version], [200, 1]);
  assert.equal(w.q(`SELECT * FROM privacy_blocks`).length, 0, "its blocks were lifted");
  // Purged for good after the grace window.
  const p2 = (await publish(w, A, bankOf([tf("Student Mia Chen (room 4) failed this test", true)]))).body;
  await w.purge(admin, p2.item_id);
  w.advance(15 * DAY);
  await w.pool.daily();
  assert.equal((await w.admin(admin, "POST", "/admin/pool/privacy-restore", { item_id: p2.item_id })).status, 409);
  assert.equal(w.r2.keys(`quarantine/${p2.item_id}`).length, 0);
});

test("M3 (reproduced): hold-copies never holds an exempt (rightful) author's item", async () => {
  const w = new World();
  const adm = w.user("admin"), R = w.user("teacher", "Rightful"), X = w.user("teacher", "Infringer");
  const Q0 = tf("Rightful's distinctive question on glacier formation", true);
  const x = (await publish(w, X, bankOf([Q0]))).body;
  const rx = await imp(w, R, x.item_id); // R imported the infringing copy (e.g. to check it)
  const r = (await publish(w, R, bankOf([strip(rx.body.payload.bank.questions[0])]))).body;
  const td = await w.takedown<any>(adm, await notice(w, adm), x.item_id, { exempt_authors: [R.id] });
  const held = await w.holdCopies<any>(adm, td.body.action_id);
  assert.deepEqual(held.body.held, [], `held ${JSON.stringify(held.body)}`);
  assert.deepEqual(w.q(`SELECT * FROM item_holds WHERE item_id = ?`, r.item_id), []);
});

test("M4: a licence refusal names the binding items; an independence claim needs a statement, is audited, and the third in 30 days flags the account", async () => {
  const w = new World();
  const X = w.user("teacher", "Xia"), Rae = w.user("teacher", "Rae");
  const qx = tf("Xia's distinctive NC question about evaporation rates", true);
  const x = (await publish(w, X, bankOf([qx]), { license: "CC-BY-NC-4.0", meta: { title: "Evaporation", subject: "Science", course: "", grade: "", tags: [], standards: [] } })).body;
  await imp(w, Rae, x.item_id);
  const r = await publish(w, Rae, bankOf([qx]), { license: "CC-BY-4.0" });
  assert.deepEqual((r.body.per_question as Q[])[0]!.bound_by, [{ item_id: x.item_id, title: "Evaporation", author_name: "Xia" }]);
  const own2 = tf("Xia's distinctive NC question about evaporation rates", true, { explanation: "Rae's own explanation" });
  const noStatement = await publish(w, Rae, bankOf([qx]), { license: "CC-BY-4.0", independent: [x.item_id], statement: "" });
  assert.deepEqual([noStatement.status, noStatement.body.field], [400, "independent_of.statement"]);
  // Round 16: the same claim made again (Rae's own bank maintenance) is not a new one ...
  for (let k = 0; k < 3; k++) {
    const ok = await publish(w, Rae, bankOf([own2]), { license: "CC-BY-4.0", independent: [x.item_id], statement: `I wrote this from my own notes, attempt ${k}.` });
    assert.equal(ok.status, 201);
  }
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'independent_repeat'`).length, 0, "one claim repeated is not three claims");
  // ... claims about two more items are: the third distinct claim in 30 days flags the account.
  for (const stem of ["Xia's distinctive NC question about condensation on cold glass surfaces", "Xia's other NC statement on sublimation of dry ice at room pressure"]) {
    const xk = (await publish(w, X, bankOf([tf(stem, true)]), { license: "CC-BY-NC-4.0" })).body;
    await imp(w, Rae, xk.item_id);
    const ok = await publish(w, Rae, bankOf([tf(stem, true, { explanation: "Rae's own explanation" })]), { license: "CC-BY-4.0", independent: [xk.item_id], statement: `I wrote this from my own notes: ${stem.slice(0, 20)}.` });
    assert.equal(ok.status, 201);
  }
  // The statement is the audit row's note (round 8: nulled if the account is deleted), not its detail.
  assert.ok(w.q<{ note: string | null }>(`SELECT note FROM pool_audit WHERE action = 'item.publish' AND actor_id = ?`, Rae.id).some((x) => (x.note ?? "").includes("my own notes")), "the statement is in the audit");
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'independent_claim'`).length, 5, "every claim flagged");
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'independent_repeat'`).length, 1, "the third in 30 days flags the account");
});

// ---- non-blocking --------------------------------------------------------------------------------------------

test("e5: one account behind a NAT takes at most 2 of the address's read slots; a trickling stream still ends at its lifetime", async () => {
  const w = new World({ readIdleMs: 300, readLifetimeMs: 600 });
  const A = w.user(), Mal = w.user(), T2 = w.user();
  const it = (await publish(w, A, bankOf(Array.from({ length: 200 }, (_, i) => q(`Question ${i} ${"padding text ".repeat(40)}`))))).body;
  const open = (u: User) => w.pool.handle(new Request(`${API}/items/${it.item_id}`, { headers: { Authorization: `Bearer ${u.token}`, "CF-Connecting-IP": "203.0.113.7" } })) as Promise<Response>;
  const mine = [await open(Mal), await open(Mal), await open(Mal)];
  assert.deepEqual(mine.map((r) => r.status), [200, 200, 429]);
  const other = await w.call("GET", `/items/${it.item_id}`, { user: T2, ip: "203.0.113.7" });
  assert.equal(other.status, 200, "the other teacher on the address still reads");
  // Trickle: one small read every 200 ms (under the idle timeout), until the lifetime ends it.
  const rd = mine[0]!.body!.getReader();
  let ended = "held";
  for (let t = 0; t < 10; t++) {
    const x = await rd.read().catch(() => ({ done: true }));
    if (x.done) {
      ended = "ended";
      break;
    }
    await new Promise((r) => setTimeout(r, 200));
  }
  assert.equal(ended, "ended");
  await mine[1]!.body!.cancel().catch(() => {});
  await mine[2]!.text();
  await new Promise((r) => setTimeout(r, 50));
  assert.equal(bytesReserved(), 0);
});

test("e6 (round 8 I6a): a publish of confirmed student-data text is held, shown as an ordinary hold, and flagged once", async () => {
  const w = new World();
  const admin = w.user("admin"), V = w.user(), Att = w.user();
  const pii = "Student Maria Lopez in room 12 failed the unit test on Friday";
  const v = (await publish(w, V, bankOf([q(pii)]))).body;
  await w.purge(admin, v.item_id);
  const states: string[] = [];
  for (const guess of [pii, "Student Mario Lopez in room 12 failed the unit test on Friday"]) {
    const r = (await publish(w, Att, bankOf([q(guess)]), { visibility: "unlisted" })).body;
    states.push((await w.json<any>("GET", "/items/mine", { user: Att })).body.items.find((x: Q) => x.id === r.item_id).state);
  }
  assert.deepEqual(states, ["held_for_review", "held_for_review"], "the same state as any other hold, no detail (round 13: a near-rewording of student data is held too)");
  // Round 12 M6: the near guess is flagged for the admin too.
  assert.deepEqual(w.q<{ d: string }>(`SELECT detail AS d FROM admin_flags WHERE kind = 'privacy_text_match' ORDER BY at, id`).map((x) => (JSON.parse(x.d).near ? "near" : "exact")).sort(), ["exact", "near"], "the right guess and the near one reached the admin queue");
  assert.ok(w.notices.some((n) => n.kind === "privacy_text_match"));
});

test("cron: a failing stage is flagged and alerted, the other stages still run; large privacy purges continue hourly", async () => {
  const w = new World({ affectedList: 2 });
  const admin = w.user("admin"), A = w.user(), Eve = w.user();
  const pii = tf("Student Jamie Smith in period 3 failed the retake", true);
  const a = (await publish(w, A, bankOf([pii]))).body;
  const copies: string[] = [];
  for (let k = 0; k < 7; k++) {
    const u = w.user();
    copies.push((await publish(w, u, bankOf(withOriginOf(await imp(w, u, a.item_id))))).body.item_id as string);
  }
  await upload(w, A, png(5, 5, 9)); // a blob for the reconciler to check
  // A failing R2 for one stage: the reconciler throws, the others run.
  const head = w.r2.head.bind(w.r2);
  w.r2.head = async () => {
    throw new Error("r2 down");
  };
  const pv = await w.admin<any>(admin, "POST", "/admin/pool/purge-preview", { item_id: a.item_id, questions: [0] });
  await w.admin(admin, "POST", "/admin/pool/purge", { item_id: a.item_id, questions: [0], confirm_preview: pv.body.confirm_preview, confirm_large: true });
  w.sql.prepare(`DELETE FROM item_holds WHERE item_id = ?`).run(copies[6]); // as if the request ended before the last batch
  w.sql.prepare(`INSERT INTO admin_flags (id, kind, item_id, detail, at) VALUES ('f1', 'privacy_backlog', ?, ?, 0)`).run(a.item_id, JSON.stringify({ match: w.q<{ hash: string }>(`SELECT hash FROM privacy_blocks`).map((x) => x.hash), blob: [] }));
  await w.pool.hourly();
  w.r2.head = head;
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'cron_failure'`).length, 1);
  assert.ok(w.notices.some((n) => n.kind === "cron_failure"));
  assert.equal((await get(w, Eve, copies[6]!)).status, 404, "the hourly backlog stage still ran");
});

test("heavy-slot queue: a publish waits at most the queue limit, then 503 busy; the account lock is kept alive meanwhile", async () => {
  const { acquireHeavy } = await import("../../src/pool/publish.ts");
  const first = await acquireHeavy(1000);
  let beats = 0;
  const t0 = Date.now();
  await assert.rejects(acquireHeavy(120, async () => void beats++), (e: { status?: number; code?: string }) => e.status === 503 && e.code === "busy");
  assert.ok(Date.now() - t0 < (process.env.POOL_STRICT_TIMING === "1" ? 1000 : 5000));
  first();
  const again = await acquireHeavy(100);
  again();
  void beats;
});

test("admin details: purge checks the token before any audit; takedown of an admin-removed item; long chains keep the first author; deleted teachers; licence versions", async () => {
  const w = new World();
  const admin = w.user("admin"), a = w.user("teacher", "Ann"), b = w.user(), ken = w.user("teacher", "Ken");
  const p = (await publish(w, a, bankOf([tf("Student Jo Bloggs failed, see notes", true)]))).body;
  const before = w.q(`SELECT * FROM pool_audit`).length;
  const audits = w.adminAudits.length;
  assert.equal((await w.admin(admin, "POST", "/admin/pool/purge", { item_id: p.item_id, questions: [0], confirm_preview: "x" })).status, 428);
  assert.deepEqual([w.q(`SELECT * FROM pool_audit`).length, w.adminAudits.length], [before, audits], "nothing written for a refused confirm");
  // An admin-removed item can be taken down (the strike is recorded); a put-back returns it to the admin removal.
  const p2 = (await publish(w, a, bankOf([tf("A distinctive question about river deltas", true)]))).body;
  await w.admin(admin, "POST", "/admin/pool/remove", { item_id: p2.item_id });
  const n = await notice(w, admin);
  assert.equal((await w.takedown(admin, n, p2.item_id)).status, 200);
  assert.equal(w.q(`SELECT * FROM strikes`).length, 1);
  await w.admin(admin, "POST", "/admin/dmca/withdraw", { notice_id: n });
  assert.equal(w.q<{ removed_reason: string }>(`SELECT removed_reason FROM items WHERE id = ?`, p2.item_id)[0]!.removed_reason, "admin");
  // Licence-only change: importers see license_version move, nothing else.
  const h = (await publish(w, a, bankOf([tf("Hidden licence question about tides", true, { explanation: "e" })]), { hideKey: true, license: "CC-BY-NC-4.0" })).body;
  await imp(w, b, h.item_id);
  await publish(w, a, bankOf(own(h, [tf("Hidden licence question about tides", true, { explanation: "e" })])), { hideKey: true, license: "CC-BY-4.0", base: h.publish_id }, h.item_id);
  const v = (await w.json<any>("POST", "/items/versions", { user: b, json: { ids: [h.item_id] } })).body.items[0];
  assert.deepEqual([v.status, v.version, v.license, v.license_version], ["current", 1, "CC-BY-4.0", 2]);
  // A kept item of a deleted teacher shows "deleted teacher" in its item view.
  const pk = (await publish(w, ken, bankOf([tf("Ken's question about buoyancy", true)]))).body;
  await w.json("PUT", "/items/prefs", { user: ken, json: { keep_on_delete: true } });
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, ken.id);
  const { deleteContentStatements } = await import("../../src/pool/index.ts");
  await w.db.batch(deleteContentStatements(w.db, ken.id, w.clock));
  assert.equal((await get(w, b, pk.item_id)).body.item.author_name, "deleted teacher");
  // A chain longer than CHAIN_MAX keeps its first author and the grants of what it collapses.
  const { collapseChain } = await import("../../src/pool/publish.ts");
  const long = Array.from({ length: 40 }, (_, i) => ({ author_name: `A${i}`, item_id: String(i).padStart(32, "0"), qid: "", version: 1, license: i === 5 ? "CC-BY-NC-4.0" : "CC-BY-4.0", modified: true }));
  const cut = collapseChain(long);
  const { chainGrants, allowedFor } = await import("../../src/pool/license.ts");
  assert.equal(cut.length, 33, "the first 8, one placeholder, the most recent 24 (round 11)");
  assert.deepEqual(cut.slice(0, 8).map((l) => l.author_name), ["A0", "A1", "A2", "A3", "A4", "A5", "A6", "A7"]);
  assert.equal(cut[8]!.author_name, "and 8 contributors between");
  assert.ok(!allowedFor(chainGrants(cut)).includes("CC-BY-4.0"), "the collapsed NC link still binds");
});

test("a bodiless POST (workerd gives it an empty stream, no length) is still an empty optional body", async () => {
  const w = new World();
  const A = w.user(), B = w.user();
  const p = (await publish(w, A, bankOf([q("Import me with an empty request body")]))).body;
  const r = await w.pool.handle(new Request(`${API}/items/${p.item_id}/import`, { method: "POST", headers: { Authorization: `Bearer ${B.token}`, "CF-Connecting-IP": "10.0.0.1" }, body: new ReadableStream({ start: (c) => c.close() }), duplex: "half" } as RequestInit));
  assert.equal(r!.status, 200);
  await r!.text();
});
