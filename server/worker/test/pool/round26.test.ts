// Round 26: Moodle critic 33c, inverted. The upstream of a taken-down copy is exempt only for what it
// CARRIED when the action was made (B-1); her earlier version predates a source that lacks what she
// added (B-2); nothing re-creates an erased account's records (B-3); unreadable bodies never starve
// the stem backfill (V4).
import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { STEM_BACKFILL_FAILS } from "../../src/pool/publish.ts";
import { framed, publish, publishId, World, type User } from "./harness.ts";
import { PUBLISH_CONTENT_TYPE } from "../../src/pool/shared.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "x", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
const code = (r: { status: number; body: Q }) => (r.status >= 400 ? `${r.status} ${r.body.error}` : String(r.status));
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const days = async (w: World, n: number) => {
  for (let d = 0; d < n; d++) {
    w.advance(DAY);
    await w.pool.hourly();
    await w.pool.daily();
  }
};
const holds = (w: World, id: string) => w.q<any>(`SELECT reason, source FROM item_holds WHERE item_id = ?`, id).map((h) => h.reason);
const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};
async function preview(w: World, user: User, payload: string, license: string) {
  const meta: Q = { publish_id: publishId(), hide_key: false, license, attestation: { accepted: true, text_rev: "2026-10-19" }, labels: [], order_acks: [], image_ok: [], visibility: "public", meta: { title: "t", subject: "Biology", course: "", grade: "", tags: [], standards: [] } };
  return w.json<any>("POST", "/items/preview", { user, body: framed(meta, payload), contentType: PUBLISH_CONTENT_TYPE });
}

// ---- B-1: the upstream exemption covers only what it carried ------------------------------------

const B = q("Carl's benign question: name one tributary of the Sacramento river near Chico?");
const T = mcq("PUBLISHER TEXTBOOK Q17: which of the following best describes the process of sediment deposition at a river mouth delta?", [["Velocity drops and load settles", true], ["Uplift", false], ["Glacial scour", false]]);
const T2 = mcq("PUBLISHER TEXTBOOK Q18: which feature forms where a meandering river cuts off a loop of its own channel?", [["Oxbow lake", true], ["Cirque", false], ["Tombolo", false]]);

async function carl(variant: string) {
  const w = new World();
  const adm = w.user("admin"), C = w.user("teacher", "Carl"), M = w.user("teacher", "Mal"), E = w.user("teacher", "Eve"), Z = w.user("teacher", "Zed");
  const c: any = (await publish(w, C, bankOf(variant === "carried" ? [B, T] : [B]), { license: "CC-BY-4.0" })).body;
  w.advance(DAY);
  await imp(w, M, c.item_id);
  const mBank = variant === "two" ? [B, T, T2] : [B, T];
  const m: any = (await publish(w, M, bankOf(mBank), { license: "CC-BY-4.0" })).body;
  w.advance(DAY);
  await get(w, E, m.item_id);
  const t = await w.takedown<any>(adm, await notice(w, adm), m.item_id, { questions: variant === "two" ? [1, 2] : [1] });
  assert.equal(t.status, 200);
  return { w, adm, C, M, E, Z, c, m, act: t.body.action_id as string };
}

test("26 B-1 (l1): the upstream that ADDS the taken-down material after the block is held like any re-upload (after the uphold too, and by hold-copies); its unchanged republish stays exempt", async () => {
  for (const variant of ["plain", "uphold", "hold_copies", "two"]) {
    const { w, adm, C, E, Z, c, act } = await carl(variant);
    // Its ordinary republish of what it carried (B only): exempt (round 25's point).
    w.advance(60_000);
    const r0 = await publish(w, C, bankOf(own(c, [{ ...B, points: 2 }])), { license: "CC-BY-4.0", base: c.publish_id }, c.item_id);
    assert.equal(r0.status, 200);
    assert.deepEqual(holds(w, c.item_id), [], `${variant}: unchanged material stays exempt`);
    if (variant === "uphold") await days(w, 31);
    w.advance(60_000);
    const z = await publish(w, Z, bankOf([T]), { license: "CC-BY-4.0" });
    assert.deepEqual(holds(w, z.body.item_id as string), ["blocklist"], "control: anyone's re-upload is held");
    const add = variant === "two" ? [B, T, T2] : [B, T];
    const r = await publish(w, C, bankOf(own(r0.body, add.slice(0, 1)).concat(add.slice(1))), { license: "CC-BY-4.0", base: r0.body.publish_id as string }, c.item_id);
    assert.equal(r.status, 200);
    assert.deepEqual(holds(w, c.item_id), ["blocklist"], `${variant}: the upstream's later addition is held`);
    assert.equal((await get(w, E, c.item_id)).status, 404);
    if (variant === "hold_copies") {
      w.sql.prepare(`DELETE FROM item_holds WHERE item_id = ?`).run(c.item_id);
      const h = await w.holdCopies<any>(adm, act);
      assert.equal(h.status, 200);
      assert.ok(w.q(`SELECT 1 FROM item_holds WHERE item_id = ? AND source = ?`, c.item_id, `dmca:${act}`).length === 1, "hold-copies holds it");
    }
  }
});

test("26 B-1: an upstream that already CARRIED the material when the action was made stays exempt (the snapshot); an action made before round 26 falls back to 'committed by the action's time'", async () => {
  const { w, C, c, act } = await carl("carried");
  assert.equal(w.q(`SELECT 1 FROM dmca_upstream_keys WHERE action_id = ? AND item_id = ?`, act, c.item_id).length > 0, true);
  w.advance(60_000);
  const r = await publish(w, C, bankOf(own(c, [{ ...B, points: 3 }, T])), { license: "CC-BY-4.0", base: c.publish_id }, c.item_id);
  assert.equal(r.status, 200);
  assert.deepEqual(holds(w, c.item_id), []);
  // Legacy: no snapshot (upstream_snap = 0).
  const l = await carl("plain");
  l.w.sql.prepare(`UPDATE dmca_actions SET upstream_snap = 0 WHERE id = ?`).run(l.act);
  l.w.sql.prepare(`DELETE FROM dmca_upstream_keys WHERE action_id = ?`).run(l.act);
  l.w.advance(60_000);
  const r2 = await publish(l.w, l.C, bankOf(own(l.c, [B, T])), { license: "CC-BY-4.0", base: l.c.publish_id }, l.c.item_id);
  assert.equal(r2.status, 200);
  assert.deepEqual(holds(l.w, l.c.item_id), ["blocklist"], "legacy: added after the action, held");
});

// ---- B-2: her earlier version predates a source that lacks what she added ----------------------

const STEMS = ["Ann's own question: in her 2019 delta survey, which landform formed first at the mouth of the Fether river?", "Ann's own question: in her 2019 delta survey, which landform formed first at the mouth of the Feather river?"];
const LONG = "Ann's own worked explanation: the logger shows water above twenty degrees on forty days of the 2019 season, juvenile counts fell in exactly those weeks, and the regression slope links them clearly.";
const C3: [string, boolean][] = [["A delta lobe of fresh sediment", true], ["A mesa of old rock", false], ["A butte", false]];
const Qv = (s: number, e: number) => mcq(STEMS[s]!, C3, { explanation: e ? LONG : null });

test("26 B-2 (c1, c2): a copier's (or an independent author's) typo fix published before hers is never credited as earlier when she fixes the typo and adds her explanation; her readers are not refused", async () => {
  for (const who of ["Mal", "Bea"]) for (const path of ["republish 101", "republish 100 then 101", "second bank 101"]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), O = w.user("teacher", who), E = w.user("teacher", "Eve");
    let a: any = (await publish(w, A, bankOf([Qv(0, 0)]), { license: "CC-BY-4.0" })).body;
    w.advance(DAY);
    if (who === "Mal") await imp(w, O, a.item_id);
    assert.equal((await publish(w, O, bankOf([Qv(1, 0)]), { license: "CC-BY-NC-4.0" })).status, 201);
    w.advance(DAY);
    let id = a.item_id, pos = 0;
    if (path === "second bank 101") {
      const x = await publish(w, A, bankOf([mcq("Unrelated: which gas do plants mostly take in during photosynthesis in daylight hours?", [["Carbon dioxide", true], ["Neon", false]]), Qv(1, 1)], "Final exam"), { license: "CC0-1.0" });
      assert.equal(x.status, 201);
      id = x.body.item_id as string;
      pos = 1;
    } else {
      if (path === "republish 100 then 101") {
        a = { ...(await publish(w, A, bankOf(own(a, [Qv(1, 0)])), { license: "CC-BY-4.0", base: a.publish_id }, a.item_id)).body, item_id: a.item_id };
        w.advance(DAY);
      }
      const x = await publish(w, A, bankOf(own(a, [Qv(1, 1)])), { license: "CC0-1.0", base: a.publish_id }, a.item_id);
      assert.equal(x.status, 200);
    }
    const g = await get(w, E, id);
    assert.deepEqual(g.body.questions[pos].chain, [], `${who} / ${path}`);
    assert.notEqual(g.body.license_check?.status, "unverified_earlier_match");
    if (who === "Mal") assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE item_id = ? AND kind = 'license_soft'`, id).length, 0, `${path}: no soft flag over a copy of her own question`);
    // An honest reader of her CC0 item copies it under CC0.
    const Z = w.user("teacher", "Zoe");
    await imp(w, Z, id);
    assert.equal(code(await preview(w, Z, bankOf([Qv(1, 1)]), "CC0-1.0")), "200", `${who} / ${path}: Zoe's CC0 copy of her CC0 item`);
  }
});

test("26 B-2 (critic 34c): the addition gate is per field — an unchanged Short sample or Blank answer does not keep a copier's typo fix as the earlier publication", async () => {
  const kinds = [
    ["Short", { Short: { sample: "A delta lobe of fresh sediment" } }, STEMS],
    ["Blank", { Blank: { accept: [["delta lobe"]] } }, [
      "In her 2019 river survey, the landform that formed first at the mouth of the Fether ___ was recorded.",
      "In her 2019 river survey, the landform that formed first at the mouth of the Feather ___ was recorded.",
    ]],
  ] as const;
  for (const [kindName, kind, stems] of kinds)
    for (const who of ["Mal", "Bea"]) {
      const w = new World();
      const A = w.user("teacher", "Ann"), O = w.user("teacher", who), E = w.user("teacher", "Eve");
      const Qk = (s: number, e: number) => q(stems[s]!, { kind, explanation: e ? LONG : null });
      const a: any = (await publish(w, A, bankOf([Qk(0, 0)]), { license: "CC-BY-4.0" })).body;
      w.advance(DAY);
      if (who === "Mal") await imp(w, O, a.item_id);
      assert.equal((await publish(w, O, bankOf([Qk(1, 0)]), { license: "CC-BY-NC-4.0" })).status, 201);
      w.advance(DAY);
      // She fixes the same typo AND adds her explanation; the sample/answer is unchanged from v0.
      const x = await publish(w, A, bankOf(own(a, [Qk(1, 1)])), { license: "CC0-1.0", base: a.publish_id }, a.item_id);
      assert.equal(x.status, 200, `${kindName} / ${who}`);
      const g = await get(w, E, a.item_id);
      assert.deepEqual(g.body.questions[0].chain, [], `${kindName} / ${who}: no earlier credit`);
      assert.notEqual(g.body.license_check?.status, "unverified_earlier_match", `${kindName} / ${who}`);
      // An honest reader of her CC0 item copies it under CC0.
      const Z = w.user("teacher", "Zoe");
      await imp(w, Z, a.item_id);
      assert.equal(code(await preview(w, Z, bankOf([Qk(1, 1)]), "CC0-1.0")), "200", `${kindName} / ${who}: Zoe's CC0 copy`);
    }
});

// ---- B-3: nothing re-creates an erased account's records ------------------------------------------

/** Every per-account record an erasure deletes, for `id`. */
const accountRows = (w: World, id: string) =>
  JSON.stringify({
    author_contents: w.q(`SELECT COUNT(*) n FROM author_contents WHERE author_id = ?`, id)[0],
    author_firsts: w.q(`SELECT COUNT(*) n FROM author_firsts WHERE author_id = ?`, id)[0],
    author_stems: w.q(`SELECT COUNT(*) n FROM author_stems WHERE author_id = ?`, id)[0],
    stem_bands: w.q(`SELECT COUNT(*) n FROM stem_bands WHERE user_id IN (?, 'own:' || ?)`, id, id)[0],
    carried_keys: w.q(`SELECT COUNT(*) n FROM carried_keys WHERE author_id = ?`, id)[0],
    blob_uploads: w.q(`SELECT COUNT(*) n FROM blob_uploads WHERE uploader_id = ?`, id)[0],
    served: w.q(`SELECT COUNT(*) n FROM served WHERE reader_id = ?`, id)[0],
    usage: w.q(`SELECT COUNT(*) n FROM usage_daily WHERE user_id = ?`, id)[0],
    prefs: w.q(`SELECT COUNT(*) n FROM pool_prefs WHERE user_id = ?`, id)[0],
  });

test("26 B-3 (e1): after an erasure, no cron job (hourly or daily, any number of runs) re-creates the account's records, kept or removed items, shown or hidden", async () => {
  for (const hide of [false, true]) for (const kept of [false, true]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), R = w.user();
    const a = (await publish(w, A, bankOf([Qv(0, 1), mcq("Ann's second question about moraines and valleys in the Sierra?", [["Yes", true], ["No", false]])]), { hideKey: hide })).body as any;
    if (kept) await imp(w, M, a.item_id);
    else await get(w, R, a.item_id);
    await get(w, A, (await publish(w, M, bankOf([q("Mal's question about clouds above lakes in summer?")]))).body.item_id as string);
    assert.ok(w.q<any>(`SELECT COUNT(*) n FROM author_stems WHERE author_id = ?`, A.id)[0].n > 0);
    await erase(w, A);
    const after = accountRows(w, A.id);
    for (let h = 0; h < 30; h++) {
      w.advance(3600_000);
      await w.pool.hourly();
      assert.equal(accountRows(w, A.id), after, `hide=${hide} kept=${kept}: hour ${h}`);
    }
    for (let d = 0; d < 16; d++) {
      w.advance(DAY);
      await w.pool.hourly();
      await w.pool.daily();
      // (others keep publishing meanwhile, which runs the per-author backfill for them)
      if (d % 5 === 0) await publish(w, M, bankOf([q(`Mal's day ${d} question about tides and the moon?`)]));
      assert.equal(accountRows(w, A.id), after, `hide=${hide} kept=${kept}: day ${d}`);
    }
  }
});

test("26 B-3 (security 34d): an erasure that commits during the backfill's R2 read is not followed by the INSERT", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann");
  const a = (await publish(w, A, bankOf([Qv(0, 1)]), { hideKey: true })).body as any;
  // Pre-round-24 state: the full-form explanation key is missing, so the backfill reads the hidden body.
  w.sql.exec(`UPDATE author_stems SET bk_expl_full = NULL`);
  assert.ok(w.q<any>(`SELECT COUNT(*) n FROM author_stems WHERE author_id = ?`, A.id)[0].n > 0);
  const fk = w.q<any>(`SELECT full_key FROM item_publishes WHERE publish_id = ?`, a.publish_id)[0].full_key as string;
  let fired = false;
  const hook = async (key: string) => {
    if (key !== fk) {
      w.r2.onGet = hook; // not this body: keep waiting for the one the backfill reads
      return;
    }
    fired = true;
    await erase(w, A); // the account deletion commits between the SELECT and the INSERT
  };
  w.r2.onGet = hook;
  w.advance(3600_000);
  await w.pool.hourly();
  assert.equal(fired, true, "the erasure committed inside the backfill window");
  assert.equal(w.q<any>(`SELECT COUNT(*) n FROM author_stems WHERE author_id = ?`, A.id)[0].n, 0, "no resurrection");
});

test("26 V4: a publish whose stored body is unreadable is skipped after a few tries (and the admin told), so other authors' stem backfill proceeds", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), B2 = w.user("teacher", "Bob");
  const a = (await publish(w, A, bankOf([Qv(0, 1)]), { hideKey: true })).body as any;
  const b = (await publish(w, B2, bankOf([mcq("Bob's hidden question about volcano types and their lava flows?", [["Shield", true], ["Cinder", false]], { explanation: "Bob's explanation about lava viscosity and the shapes of cones." })]), { hideKey: true })).body as any;
  // Simulate the pre-round-24 state: no full-form explanation key; Ann's body lost.
  w.sql.exec(`UPDATE author_stems SET bk_expl_full = NULL`);
  const fk = w.q<any>(`SELECT full_key FROM item_publishes WHERE publish_id = ?`, a.publish_id)[0].full_key;
  await w.r2.delete(fk);
  for (let h = 0; h < STEM_BACKFILL_FAILS + 2; h++) {
    w.advance(3600_000);
    await w.pool.hourly();
  }
  assert.equal(w.q(`SELECT 1 FROM pool_kv WHERE key = ?`, `stem_bf_skip:${a.publish_id}`).length, 1, "skipped");
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'cron_failure' AND dedup = ?`, `stem_bf_skip:${a.publish_id}`).length, 1);
  assert.ok(w.q<any>(`SELECT COUNT(*) n FROM author_stems WHERE author_id = ? AND bk_expl_full IS NOT NULL`, B2.id)[0].n > 0, "Bob's backfill done");
  void b;
});

// ---- the round-26 differential ---------------------------------------------------------------------

function sm(seed: number) {
  let s = Math.imul(seed ^ 0x5bd1e995, 0x27d4eb2d) >>> 0;
  return () => {
    s = (s + 0x9e3779b9) >>> 0;
    let z = s;
    z = Math.imul(z ^ (z >>> 16), 0x85ebca6b) >>> 0;
    z = Math.imul(z ^ (z >>> 13), 0xc2b2ae35) >>> 0;
    return ((z ^ (z >>> 16)) >>> 0) / 2 ** 32;
  };
}
const SEEDS = Number(process.env.POOL_PROPERTY_SEEDS ?? 8);
const START = Number(process.env.POOL_PROPERTY_START ?? 0);
const L5 = ["CC0-1.0", "CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"];

test(`round 26 differential (${SEEDS} seeds): Ann's edits (typo fixes, explanations, second banks) never credit a later copier or an independent later author; a Carl upstream never serves blocked material added after the block; an erased account's records never come back`, async () => {
  for (let seed = START; seed < START + SEEDS; seed++) {
    const r = sm(seed);
    const pick = <T0>(xs: T0[]) => xs[Math.floor(r() * xs.length)]!;
    const w = new World();
    const adm = w.user("admin"), A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), Be = w.user("teacher", "Bea"), Ca = w.user("teacher", "Carl"), E = w.user("teacher", "Eve");
    let sv = 0, e = Math.floor(r() * 2);
    let a: any = (await publish(w, A, bankOf([Qv(sv, e)]), { license: pick(L5), hideKey: r() < 0.3 })).body;
    const ops: string[] = [`A${sv}${e}`];
    let erased = false;
    let carlState: { c: any; act: string | null; added: boolean } | null = null;
    for (let i = 0; i < 5 + Math.floor(r() * 6); i++) {
      w.advance(DAY / 2 + Math.floor(r() * DAY));
      const k = Math.floor(r() * 8);
      if (k <= 1 && !erased) {
        // Ann: a typo fix, her explanation, both, or a second bank
        const what = pick(["typo", "expl", "both", "bank"]);
        if (what === "typo" || what === "both") sv = 1;
        if (what === "expl" || what === "both") e = 1;
        if (what === "bank") {
          const x = await publish(w, A, bankOf([Qv(sv, e)], `bank${i}`), { license: pick(L5) });
          ops.push(`Abank${sv}${e}:${x.status}`);
          if (x.status === 201) assert.deepEqual((await get(w, E, x.body.item_id as string)).body.questions[0].chain, [], `seed ${seed}: her second bank credits nobody: ${ops.join(" ")}`);
        } else {
          const x = await publish(w, A, bankOf(own(a, [Qv(sv, e)])), { license: pick(L5), base: a.publish_id }, a.item_id);
          ops.push(`A${sv}${e}:${x.status}`);
          if (x.status === 200) a = { ...x.body, item_id: a.item_id };
        }
      } else if (k === 2 && !erased) {
        // Mal (served) or Bea (never served) publishes the obvious next edit first
        const u = pick([M, Be]);
        if (u === M) await imp(w, M, a.item_id);
        const x = await publish(w, u, bankOf([Qv(1, 0)], `o${i}`), { license: pick(L5) });
        ops.push(`${u.name}100:${x.status}`);
      } else if (k === 3 && !carlState) {
        // a Carl upstream: Mal imports Carl's benign item, adds T, is taken down
        const c: any = (await publish(w, Ca, bankOf([B], `c${i}`), { license: "CC-BY-4.0" })).body;
        await imp(w, M, c.item_id);
        const m: any = (await publish(w, M, bankOf([B, T], `m${i}`), { license: "CC-BY-4.0" })).body;
        const t = await w.takedown<any>(adm, await notice(w, adm), m.item_id, { questions: [1] });
        carlState = { c, act: t.status === 200 ? (t.body.action_id as string) : null, added: false };
        ops.push(`carl:${t.status}`);
      } else if (k === 4 && carlState && !carlState.added && carlState.act) {
        const x = await publish(w, Ca, bankOf(own(carlState.c, [B]).concat([T])), { license: "CC-BY-4.0", base: carlState.c.publish_id }, carlState.c.item_id);
        carlState.added = true;
        ops.push(`carlAddsT:${x.status}`);
      } else if (k === 5 && !erased && r() < 0.3) {
        await erase(w, A);
        erased = true;
        ops.push("eraseAnn");
      } else if (k === 6) {
        await days(w, 1 + Math.floor(r() * 3));
        ops.push("days");
      } else ops.push("noop");
      // invariants after every step
      if (carlState?.added) {
        const servesT = JSON.stringify((await get(w, E, carlState.c.item_id)).body).includes("PUBLISHER TEXTBOOK Q17");
        assert.equal(servesT, false, `seed ${seed}: Carl's upstream serves the blocked material: ${ops.join(" ")}`);
      }
      if (!erased) {
        const g = await get(w, E, a.item_id);
        if (g.status === 200) {
          const ch = JSON.stringify(g.body.questions[0].chain);
          assert.ok(!/Mal|Bea/.test(ch), `seed ${seed}: her item credits a later author ${ch}: ${ops.join(" ")}`);
        }
      }
    }
    if (erased) {
      const before = accountRows(w, A.id);
      await days(w, 3);
      for (let h = 0; h < 3; h++) {
        w.advance(3600_000);
        await w.pool.hourly();
      }
      assert.equal(accountRows(w, A.id), before, `seed ${seed}: an erased account's records came back: ${ops.join(" ")}`);
    }
  }
});
