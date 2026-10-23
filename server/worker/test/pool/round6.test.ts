// Round 6: structural rule 2 (every hold/block/unblock/purge derivation sees only SERVED images) as a
// property over admin action sequences, and the critics' repros (critic9a X1-X7, critic9b s1-s11)
// inverted. Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { bytesReserved, PUBLISH_CONTENT_TYPE } from "../../src/pool/shared.ts";
import { framed, png, publish, publishId, rng, upload, World, type User } from "./harness.ts";
// (At least 25 words: Ann copying it back is a lift, round 16.)
const BOB_NC = "Bob's NC explanation: chlorophyll absorbs light, the light reactions make ATP and NADPH from water, and the Calvin cycle uses them to turn carbon dioxide into glucose that the plant stores.";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[], sv = 2) => JSON.stringify({ schema_version: sv, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const tf = (stem: string, correct = true, extra: Q = {}): Q => ({ id: "q", stem, kind: { Tf: { correct } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const img = (sha: string) => ({ file: `media/${sha}.png`, width: 9, height: 9 });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const own = (pub: any, qs: Q[]) => qs.map((x, i) => (i < pub.qids.length ? { ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } } : x));
const withOrigin = (r: any, edit: (x: Q) => Q = (x) => x) =>
  r.body.payload.bank.questions.map((x: Q, i: number) =>
    edit({ ...x, origin: { item_id: r.body.item.id, qid: r.body.questions[i].qid, publish_id: r.body.item.publish_id, version: r.body.item.version, author_name: r.body.item.author_name, license: r.body.license, form: r.body.item.form } }),
  );
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const holds = (w: World, id: string) => w.q<{ reason: string; source: string }>(`SELECT reason, source FROM item_holds WHERE item_id = ? ORDER BY reason, source`, id);

// ---- STRUCTURAL RULE 2, as a property ------------------------------------------------------------------

const ACTIONS = ["purgeA2", "purgeA1", "reportV", "releaseV", "forceReleaseV", "unblockX", "unblockS", "unblockPII", "removeV", "unremoveV", "cron1", "cron8", "cron15", "reupB", "probeServed", "probeHidden", "repubV", "takedownB2", "holdCopies", "withdraw", "releaseCopies", "dmcaUnblock"] as const;

/**
 * One world: a victim V's hidden item carries a FULL-ONLY image (unticked) that is the attacker's
 * image X, another image Y, or none; attackers A and B serve X and a question S V also uses. The
 * same admin/attacker actions run in each; everything a non-author can observe is recorded.
 */
async function sequenceWorld(variant: "X" | "Y" | "none", seed: number, fixed: string[] | null = null): Promise<string> {
  const r = rng(seed);
  const w = new World();
  const admin = w.user("admin", "Adm"), V = w.user("teacher", "Vic"), A = w.user("teacher", "Att"), B = w.user("teacher", "Bea"), Eve = w.user("teacher", "Eve"), Imp = w.user("teacher", "Ian");
  const X = png(9, 9, 42), Y = png(9, 9, 77);
  const S = tf("Which organelle performs photosynthesis in plant leaves today?");
  const PII = q("Photo of student Jo Bloggs' worksheet here");
  const ua = await upload(w, A, X);
  const a1 = (await publish(w, A, bankOf([S]))).body;
  const a2 = (await publish(w, A, bankOf([{ ...PII, image: img(ua.sha) }]))).body;
  const rb = await imp(w, B, a2.item_id);
  let b2 = (await publish(w, B, bankOf(withOrigin(rb)))).body;
  const vimg = variant === "none" ? null : await upload(w, V, variant === "X" ? X : Y);
  const vq = q("Read the pipette shown and give the volume", { image: vimg ? img(vimg.sha) : null });
  let pv = (await publish(w, V, bankOf([S, vq]), { hideKey: true, imageOk: [] })).body;
  await imp(w, Imp, pv.item_id);
  const ids: Record<string, string> = { V: pv.item_id, A1: a1.item_id, A2: a2.item_id, B2: b2.item_id };
  const extra: string[] = [];
  let actionId: string | null = null;
  let noticeId: string | null = null;
  const seen: unknown[] = [];
  const label = (s: string) => {
    for (const [k, v] of Object.entries(ids)) s = s.replaceAll(v, `<${k}>`);
    for (const [i, v] of extra.entries()) s = s.replaceAll(v, `<P${i}>`);
    return s.replace(/"(?:[0-9a-f]{32}|[A-Za-z0-9_-]{22})"/g, '"<id>"').replace(/"confirm_preview":"[^"]+"/g, '"confirm_preview":"<t>"').replace(/"etag":"[^"]+"/gi, '"etag":"<e>"').replace(/"served_sha256":"[0-9a-f]{64}"/g, '"served_sha256":"<qid-dependent>"');
  };
  const lab = (id: string) => Object.entries(ids).find(([, v]) => v === id)?.[0] ?? (extra.includes(id) ? `P${extra.indexOf(id)}` : "?");
  const sorted = (xs: unknown[][]) => xs.map(([id, ...rest]) => [lab(id as string), ...rest]).sort((a, b) => (String(a[0]) < String(b[0]) ? -1 : 1));
  // Lists of items come back in id order (random per world): compared as sets of labels.
  const norm = (v: unknown): unknown => {
    if (Array.isArray(v)) {
      const xs = v.map(norm);
      const key = (x: unknown) => (typeof x === "string" ? lab(x) + x : x && typeof x === "object" && "item_id" in x ? lab(String((x as Q).item_id)) : JSON.stringify(x));
      return xs.every((x) => typeof x === "string" || (x && typeof x === "object" && "item_id" in (x as object))) ? [...xs].sort((a, b) => (key(a) < key(b) ? -1 : 1)) : xs;
    }
    if (v && typeof v === "object") return Object.fromEntries(Object.entries(v as Q).map(([k, x]) => [k, norm(x)]));
    return v;
  };
  const observe = async (step: string, res: unknown) => {
    res = norm(res);
    const g = await w.json<any>("GET", `/items/${pv.item_id}`, { user: Eve });
    const items = [ids.A1!, ids.A2!, ids.B2!, ...extra];
    seen.push({
      step,
      res,
      v: [g.status, g.status === 200 ? g.body : null],
      vers: (await w.json<any>("POST", "/items/versions", { user: Imp, json: { ids: [pv.item_id, ...items] } })).body,
      others: await (async () => {
        const out: number[] = [];
        for (const id of items) out.push((await get(w, Eve, id)).status); // sequential: the open-read cap is per account
        return out;
      })(),
      xViaB2: (await w.call("GET", `/items/${b2.item_id}/blobs/${ua.sha}`, { user: Eve })).status,
      list: sorted((await w.json<any>("GET", "/items?limit=50", { user: Eve })).body.items.map((x: Q) => [x.id, x.version, x.license])),
      mineA: sorted((await w.json<any>("GET", "/items/mine", { user: A })).body.items.map((x: Q) => [x.id, x.state])),
      mineB: sorted((await w.json<any>("GET", "/items/mine", { user: B })).body.items.map((x: Q) => [x.id, x.state])),
      // Holds on everyone else's items (V's own are observed only through what Eve and Ian get).
      holds: items.map((id) => holds(w, id)),
    });
  };
  const mhOf = (itemId: string, pos: number) => w.q<{ match_hash: string }>(`SELECT iq.match_hash FROM item_questions iq JOIN items i ON i.current_publish_id = iq.publish_id OR i.prev_publish_id = iq.publish_id WHERE i.id = ? AND iq.pos = ?`, itemId, pos)[0]?.match_hash ?? "0".repeat(64);
  const sHash = mhOf(a1.item_id, 0);
  const piiHash = mhOf(a2.item_id, 0);
  await observe("start", null);
  // Weighted towards the paths that matter (a purge of the guessed image, the grace window ending,
  // unblocks), so random sequences reach them often; fixed scenarios cover the critics' orders.
  const weighted = [...ACTIONS, "purgeA2", "purgeA2", "purgeA1", "cron15", "cron15", "unblockX", "unblockX", "repubV", "probeHidden"];
  const n = fixed ? fixed.length : 6 + r.int(6);
  for (let k = 0; k < n; k++) {
    const act = fixed ? fixed[k]! : r.pick(weighted);
    let res: unknown = null;
    const adm = async (path: string, body: Record<string, unknown>) => {
      const x = await w.admin<any>(admin, "POST", path, body);
      return [x.status, x.body];
    };
    switch (act) {
      case "purgeA2":
        res = await w.purge(admin, a2.item_id).then((x) => [x.status, x.body]);
        break;
      case "purgeA1":
        res = await w.purge(admin, a1.item_id).then((x) => [x.status, x.body]);
        break;
      case "reportV":
        res = (await w.json("POST", `/items/${pv.item_id}/report`, { user: Eve, json: { reason: "student_data" } })).status;
        break;
      case "releaseV":
        res = await adm("/admin/pool/release", { item_id: pv.item_id, reason: "student_data" });
        break;
      case "forceReleaseV":
        res = await adm("/admin/pool/release", { item_id: pv.item_id, reason: "student_data", source: "privacy", force: true });
        break;
      case "unblockX":
        res = await adm("/admin/pool/unblock", { kind: "blob", hash: ua.sha });
        break;
      case "unblockS":
        res = await adm("/admin/pool/unblock", { kind: "match", hash: sHash });
        break;
      case "unblockPII":
        res = await adm("/admin/pool/unblock", { kind: "match", hash: piiHash });
        break;
      case "removeV":
        res = await adm("/admin/pool/remove", { item_id: pv.item_id });
        break;
      case "unremoveV":
        res = await adm("/admin/pool/unremove", { item_id: pv.item_id });
        break;
      case "cron1":
      case "cron8":
      case "cron15":
        w.advance(Number(act.slice(4)) * DAY);
        res = [await w.pool.daily(), await w.pool.hourly()];
        break;
      case "reupB": {
        const u = await upload(w, B, X);
        const rp = await publish(w, B, bankOf(own(b2, withOrigin(rb))), { base: b2.publish_id }, b2.item_id);
        if (rp.status === 200) b2 = { ...b2, ...rp.body };
        res = [u.status, u.body, rp.status, rp.body.error ?? null];
        break;
      }
      case "probeServed": {
        const u = await upload(w, B, X);
        const p = await publish(w, B, bankOf([q(`B's probe ${k} with the guessed image`, { image: img(ua.sha) })]));
        if (p.body.item_id) extra.push(p.body.item_id as string);
        res = [u.status, u.body, p.status, p.body.error ?? null];
        break;
      }
      case "probeHidden": {
        const p = await publish(w, B, bankOf([q(`B's hidden probe ${k} with the guessed image`, { image: img(ua.sha) })]), { hideKey: true, imageOk: [] });
        if (p.body.item_id) extra.push(p.body.item_id as string);
        res = [p.status, p.body.error ?? null, p.body.reason ?? null];
        break;
      }
      case "repubV": {
        // V's own (author) action: identical served content in every world.
        const p = await publish(w, V, bankOf(own(pv, [S, vq])), { hideKey: true, imageOk: [], base: pv.publish_id }, pv.item_id);
        if (p.status === 200) pv = { ...pv, ...p.body };
        res = [p.status, p.body.error ?? null, p.body.public_version ?? null];
        break;
      }
      case "takedownB2": {
        noticeId ??= await notice(w, admin);
        const t = await w.takedown<any>(admin, noticeId, b2.item_id);
        if (t.status === 200) actionId = t.body.action_id;
        res = [t.status, t.body];
        break;
      }
      case "holdCopies":
        res = actionId ? await w.holdCopies(admin, actionId).then((x) => [x.status, x.body]) : null;
        break;
      case "withdraw":
        res = noticeId ? await adm("/admin/dmca/withdraw", { notice_id: noticeId }) : null;
        break;
      case "releaseCopies":
        res = actionId ? await adm("/admin/dmca/release-copies", { action_id: actionId }) : null;
        break;
      case "dmcaUnblock":
        res = actionId ? await adm("/admin/dmca/unblock", { action_id: actionId }) : null;
        break;
    }
    await observe(act, res);
  }
  return label(JSON.stringify(seen));
}

/** The first step at which two worlds' observations differ, for the failure message. */
function firstDiff(a: string, b: string): string {
  const x = JSON.parse(a) as unknown[], y = JSON.parse(b) as unknown[];
  const i = x.findIndex((v, k) => JSON.stringify(v) !== JSON.stringify(y[k]));
  if (i < 0) return "(same steps, different length)";
  const fx = x[i] as Record<string, unknown>, fy = y[i] as Record<string, unknown>;
  const keys = Object.keys(fx).filter((k) => JSON.stringify(fx[k]) !== JSON.stringify(fy[k]));
  return `step ${i} (${String(fx.step)}): ${keys.map((k) => `${k}: ${JSON.stringify(fx[k]).slice(0, 700)} <> ${JSON.stringify(fy[k]).slice(0, 700)}`).join("; ")}`;
}

test("structural rule 2: for every admin/attacker action sequence, nothing a non-author observes depends on a hidden item's full-only image (X, Y or none)", async () => {
  const fixed = [
    ["purgeA2", "cron15", "unblockX", "repubV", "probeHidden", "probeServed", "reupB", "cron1"], // X1, and publish-time probes
    ["purgeA2", "purgeA1", "unblockS", "unblockPII", "unblockX"], // X2: orphan releases
    ["purgeA2", "removeV", "unremoveV", "cron15", "removeV", "unremoveV"], // X6: the unremove re-hold
    ["purgeA2", "cron15", "probeHidden", "repubV", "unblockX", "reupB", "probeServed", "repubV"], // bytes kept for V only
    ["takedownB2", "holdCopies", "repubV", "probeServed", "withdraw", "releaseCopies", "dmcaUnblock"], // DMCA blob blocks
    ["reportV", "purgeA1", "releaseV", "forceReleaseV", "unblockS", "cron15"], // release sources
  ];
  const runs: [string, string[] | null][] = [...fixed.map((f, i) => [`fixed ${i}`, f] as [string, string[]]), ...Array.from({ length: 24 }, (_, i) => [`seed ${i + 1}`, null] as [string, null])];
  for (const [name, f] of runs) {
    const seed = f ? 1000 : Number(name.slice(5));
    const [x, y, none] = [await sequenceWorld("X", seed, f), await sequenceWorld("Y", seed, f), await sequenceWorld("none", seed, f)];
    assert.ok(x === y, `${name}: the full-only image X vs Y is observable: ${x === y ? "" : firstDiff(x, y)}`);
    assert.ok(x === none, `${name}: a full-only image vs none is observable: ${x === none ? "" : firstDiff(x, none)}`);
  }
});

// ---- F1: the critics' oracles, one by one ----------------------------------------------------------------

test("X1/X2/X6: a hidden item's full-only image never decides a hold (late unblock, orphan release, unremove)", async () => {
  for (const scenario of ["late-unblock", "orphan", "unremove"]) {
    const seen: number[] = [];
    for (const same of [true, false]) {
      const w = new World();
      const admin = w.user("admin"), V = w.user(), A = w.user(), Eve = w.user();
      const X = png(9, 9, 42);
      const S = tf("Which organelle performs photosynthesis in plant leaves today?");
      const ua = await upload(w, A, X);
      const a1 = (await publish(w, A, bankOf([S]))).body;
      const a2 = (await publish(w, A, bankOf([q("Photo of student Jo Bloggs' worksheet here", { image: img(ua.sha) })]))).body;
      const uv = await upload(w, V, same ? X : png(9, 9, 77));
      const pv = (await publish(w, V, bankOf([...(scenario === "orphan" ? [S] : []), q("Read the pipette shown and give the volume", { image: img(uv.sha) })]), { hideKey: true, imageOk: [] })).body;
      assert.equal((await w.purge(admin, a2.item_id)).status, 200);
      if (scenario === "late-unblock") {
        w.advance(15 * DAY);
        await w.pool.daily();
        await w.admin(admin, "POST", "/admin/pool/unblock", { kind: "blob", hash: ua.sha });
      } else if (scenario === "orphan") {
        // Round 9: V has S on their own, so the admin selects it explicitly.
        assert.equal((await w.purge(admin, a1.item_id, { questions: [0] })).status, 200);
        assert.equal((await get(w, Eve, pv.item_id)).status, 404, "held through the shared question");
        for (const h of w.q<{ hash: string }>(`SELECT hash FROM privacy_blocks WHERE kind = 'match'`)) await w.admin(admin, "POST", "/admin/pool/unblock", { kind: "match", hash: h.hash });
      } else {
        await w.admin(admin, "POST", "/admin/pool/remove", { item_id: pv.item_id });
        await w.admin(admin, "POST", "/admin/pool/unremove", { item_id: pv.item_id });
      }
      seen.push((await get(w, Eve, pv.item_id)).status);
    }
    assert.deepEqual(seen, [200, 200], `${scenario}: a right guess and a wrong one look the same`);
  }
});

test("F1: the cron byte purge keeps an author's unserved copy of the bytes, and nothing served can tell", async () => {
  const w = new World();
  const admin = w.user("admin"), V = w.user(), A = w.user(), B = w.user(), Eve = w.user();
  const X = png(9, 9, 42);
  const ua = await upload(w, A, X);
  const a2 = (await publish(w, A, bankOf([q("Photo of student Jo Bloggs' worksheet here", { image: img(ua.sha) })]))).body;
  const rb = await imp(w, B, a2.item_id);
  const b2 = (await publish(w, B, bankOf(withOrigin(rb)))).body;
  await upload(w, V, X);
  const vq = q("Read the pipette shown and give the volume", { image: img(ua.sha) });
  const pv = (await publish(w, V, bankOf([vq]), { hideKey: true, imageOk: [] })).body;
  await w.purge(admin, a2.item_id);
  w.advance(15 * DAY);
  await w.pool.daily();
  assert.ok(w.r2.keys("blobs/").includes(`blobs/${ua.sha}`), "V's own unserved use keeps the bytes");
  // ... but every served use treats the image as gone, and only V's unserved use can read it.
  assert.equal((await w.call("GET", `/items/${pv.item_id}/blobs/${ua.sha}`, { user: V })).status, 404, "still blocked");
  await w.admin(admin, "POST", "/admin/pool/unblock", { kind: "blob", hash: ua.sha });
  for (const h of w.q<{ hash: string }>(`SELECT hash FROM privacy_blocks`)) await w.admin(admin, "POST", "/admin/pool/unblock", { kind: "match", hash: h.hash });
  assert.equal((await w.call("GET", `/items/${pv.item_id}/blobs/${ua.sha}`, { user: V })).status, 200, "V reads V's own unserved image");
  assert.deepEqual(holds(w, b2.item_id), [{ reason: "admin", source: "needs_reupload" }], "B's served copy needs a re-upload, whether or not bytes exist");
  const probe = await publish(w, B, bankOf([q("B serves the image again", { image: img(ua.sha) })]));
  assert.deepEqual([probe.status, probe.body.error, probe.body.reason], [422, "blob_missing", "reupload"]);
  assert.equal((await publish(w, V, bankOf(own(pv, [vq])), { hideKey: true, imageOk: [], base: pv.publish_id }, pv.item_id)).status, 200, "V republishes V's own item");
  // A fresh upload brings it back for everyone.
  assert.equal((await upload(w, B, X)).status, 201);
  assert.equal((await publish(w, B, bankOf(own(b2, withOrigin(rb))), { base: b2.publish_id }, b2.item_id)).status, 200);
  assert.equal((await w.call("GET", `/items/${b2.item_id}/blobs/${ua.sha}`, { user: Eve })).status, 200);
});

// ---- F2: qid reuse ---------------------------------------------------------------------------------------

test("X3/X3b/X7: an old qid or item given new content never fakes 'first publisher' (questions, images, licences); control case unchanged", async () => {
  // X3: a question.
  {
    const w = new World();
    const admin = w.user("admin"), Att = w.user(), V = w.user();
    const a1 = (await publish(w, Att, bankOf([q("Placeholder question number one, nothing here")]))).body;
    w.advance(DAY);
    const vq = q("Victim's own original question about the Calvin cycle steps");
    const pv = (await publish(w, V, bankOf([vq]))).body;
    w.advance(DAY);
    const a2 = await publish(w, Att, bankOf([...own(a1, [vq]), q("Student Maria Lopez in room 12 failed the test.")]), { base: a1.publish_id }, a1.item_id);
    assert.equal(a2.status, 200);
    const prev = await w.admin<any>(admin, "POST", "/admin/pool/purge-preview", { item_id: a1.item_id });
    assert.deepEqual(prev.body.match.map((m: Q) => [m.pos, m.introduced, m.suggested]), [[0, false, false], [1, true, true]], "round 16 (critic21d 3): suggested only where this item introduced it");
    assert.ok(!prev.body.affected.some((x: Q) => x.item_id === pv.item_id), "the victim is not held by default");
  }
  // X3b: an image.
  {
    const w = new World();
    const admin = w.user("admin"), Att = w.user(), V = w.user();
    const a1 = (await publish(w, Att, bankOf([q("Placeholder question number one, nothing here")]))).body;
    w.advance(DAY);
    const uv = await upload(w, V, png(9, 9, 5));
    const pv = (await publish(w, V, bankOf([q("Victim diagram question about leaves", { image: img(uv.sha) })]))).body;
    w.advance(DAY);
    await imp(w, Att, pv.item_id);
    const a2 = await publish(w, Att, bankOf(own(a1, [q("Photo of student Jo Bloggs' worksheet here", { image: img(uv.sha) })])), { base: a1.publish_id }, a1.item_id);
    assert.equal(a2.status, 200);
    const prev = await w.admin<any>(admin, "POST", "/admin/pool/purge-preview", { item_id: a1.item_id });
    assert.deepEqual(prev.body.blob.map((b: Q) => [b.introduced, b.suggested]), [[false, false]]);
    assert.ok(!prev.body.affected.some((x: Q) => x.item_id === pv.item_id));
  }
  // X7: a licence, with the control (a plain adaptation is bound) first.
  {
    const w = new World();
    const Att = w.user("teacher", "Mallory"), V = w.user("teacher", "Victor"), Eve = w.user();
    const p = (await publish(w, Att, bankOf([q("Placeholder question number one, nothing here")]))).body;
    w.advance(DAY);
    const pv = (await publish(w, V, bankOf([q("Explain the role of chlorophyll in photosynthesis today", { explanation: "Victor's careful explanation" })]), { license: "CC-BY-NC-SA-4.0" })).body;
    const vi = await imp(w, Att, pv.item_id);
    const vq = { ...vi.body.payload.bank.questions[0] };
    // Round 8: the control is a VERBATIM copy of the import (hard evidence); an undeclared edit is soft.
    assert.equal((await publish(w, Att, bankOf([vq]), { license: "CC0-1.0" })).body.error, "license_incompatible", "control");
    assert.equal((await publish(w, Att, bankOf(own(p, [vq])), { license: "CC-BY-NC-SA-4.0", base: p.publish_id }, p.item_id)).status, 200);
    const s2 = await publish(w, Att, bankOf([vq]), { license: "CC0-1.0" });
    assert.equal(s2.body.error, "license_incompatible", "the old qid did not make Victor's question Mallory's own");
    const ok = await publish(w, Att, bankOf([{ ...vq, explanation: "edited" }]), { license: "CC-BY-NC-SA-4.0" });
    assert.deepEqual((await get(w, Eve, ok.body.item_id as string)).body.questions[0].chain.map((l: Q) => l.author_name), ["Victor"]);
  }
  // A kept qid with the SAME content keeps its seq (an ordinary republish is still "first").
  {
    const w = new World();
    const admin = w.user("admin"), A = w.user(), B = w.user();
    const qa = q("Author's own question about tides and the moon");
    const pa = (await publish(w, A, bankOf([qa]))).body;
    w.advance(DAY);
    await publish(w, B, bankOf([qa]));
    const ra = await publish(w, A, bankOf(own(pa, [{ ...qa, explanation: "added later" }])), { base: pa.publish_id }, pa.item_id);
    assert.equal(ra.status, 200);
    const prev = await w.admin<any>(admin, "POST", "/admin/pool/purge-preview", { item_id: pa.item_id });
    assert.deepEqual(prev.body.match.map((m: Q) => m.introduced), [true]);
  }
});

// ---- F3: void evidence flows into chains and grants --------------------------------------------------------

test("s1/F3: after an upheld takedown (or an admin void) the rightful author's republish drops the infringer's chain and grants; exempt authors are never held", async () => {
  for (const route of ["upheld", "admin"]) {
    const w = new World();
    const admin = w.user("admin", "Admin"), X = w.user("teacher", "Infringer"), R = w.user("teacher", "Rightful"), Eve = w.user();
    const Qs = [tf("Rightful's original question about mitochondria and ATP synthase", true)];
    const x = (await publish(w, X, bankOf(Qs), { license: "CC-BY-NC-4.0" })).body;
    await imp(w, R, x.item_id); // R imported it to check it (round 8: a mere read is soft evidence)
    assert.equal((await publish(w, R, bankOf(Qs), { license: "CC-BY-4.0" })).body.error, "license_incompatible");
    const r2 = (await publish(w, R, bankOf(Qs), { license: "CC-BY-NC-4.0" })).body;
    assert.deepEqual((await get(w, Eve, r2.item_id)).body.questions[0].chain.map((l: Q) => l.author_name), ["Infringer"]);
    if (route === "upheld") {
      const n = await notice(w, admin);
      assert.equal((await w.takedown(admin, n, x.item_id, { exempt_authors: [R.id] })).status, 200);
      w.advance(31 * DAY);
      await w.pool.daily();
    } else assert.equal((await w.admin(admin, "POST", "/admin/pool/evidence", { item_id: x.item_id, action: "void" })).status, 200);
    assert.deepEqual((await get(w, Eve, r2.item_id)).body.questions[0].chain, [], "no longer shown as credited to the infringer");
    const r3 = await publish(w, R, bankOf(own(r2, Qs)), { license: "CC-BY-4.0", base: r2.publish_id }, r2.item_id);
    assert.equal(r3.status, 200, JSON.stringify(r3.body));
    assert.equal(w.q<{ chain_json: string }>(`SELECT chain_json FROM item_questions WHERE publish_id = ?`, r3.body.publish_id)[0]!.chain_json, "[]", "the stored chain is rewritten");
    assert.deepEqual(holds(w, r2.item_id), [], "her own item is not held by the block her notice caused");
    const r5 = await publish(w, R, bankOf(Qs), { license: "CC-BY-4.0" });
    assert.equal(r5.status, 201);
    assert.deepEqual(holds(w, r5.body.item_id as string), [], route === "upheld" ? "exempt: a new item is not held either" : "no block");
    // Anyone else's re-upload IS held (upheld route).
    if (route === "upheld") {
      const other = await publish(w, w.user(), bankOf(Qs));
      assert.equal(holds(w, other.body.item_id as string)[0]?.reason, "blocklist");
    }
  }
});

test("F3b: a third party who imported an adaptation of void material is bound only by the adapter's own additions", async () => {
  const w = new World();
  const adm = w.user("admin"), X = w.user("teacher", "Infringer"), Y = w.user("teacher", "Yan"), R = w.user("teacher", "Rightful"), Eve = w.user();
  const Qx = tf("Rightful's original question about mitochondria and ATP synthase", true);
  const x = (await publish(w, X, bankOf([Qx]), { license: "CC-BY-NC-4.0" })).body;
  const ry = await imp(w, Y, x.item_id);
  const y = (await publish(w, Y, bankOf(withOrigin(ry, (z) => ({ ...z, explanation: "Yan's note" }))), { license: "CC-BY-NC-4.0" })).body;
  await imp(w, R, y.item_id);
  // Before the takedown: R's own Q (without Yan's note) is not a verbatim copy of what she imported: soft (round 8).
  assert.equal((await publish(w, R, bankOf([{ ...Qx, explanation: "Yan's note" }]), { license: "CC-BY-4.0" })).body.error, "license_incompatible", "a verbatim copy of Yan's item is bound before the takedown");
  await w.takedown(adm, await notice(w, adm), x.item_id);
  w.advance(31 * DAY);
  await w.pool.daily();
  const r = await publish(w, R, bankOf([Qx]), { license: "CC-BY-4.0" });
  assert.equal(r.status, 201, "R never took Yan's note: nothing traceable to valid evidence binds her");
  // No rightful item was named: Yan's copy is held for review (round 7 M1) rather than left live with no licence.
  assert.equal((await get(w, Eve, y.item_id)).status, 404);
  const aid = w.q<{ id: string }>(`SELECT id FROM dmca_actions`)[0]!.id;
  await w.admin(adm, "POST", "/admin/dmca/release-copies", { action_id: aid });
  assert.deepEqual((await get(w, Eve, y.item_id)).body.questions[0].chain, [], "Yan's chain no longer credits the infringer");
  // ... but a verbatim copy of Yan's adaptation (her note included) is still Yan's NC.
  const copy = await publish(w, R, bankOf([{ ...Qx, explanation: "Yan's note" }]), { license: "CC-BY-4.0" });
  assert.equal(copy.body.error, "license_incompatible");
});

// ---- F4: source-aware release ----------------------------------------------------------------------------

test("s7/F4: releasing a reason lifts only human holds; privacy and DMCA holds need their own route or force", async () => {
  const w = new World();
  const admin = w.user("admin"), A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), Rep = w.user(), V = w.user();
  const pii = tf("Student Jamie Q. Smith (ID 448812) failed the retake on 3/4", true);
  const a = (await publish(w, A, bankOf([pii]))).body;
  const rb = await imp(w, B, a.item_id);
  const b = (await publish(w, B, bankOf([...withOrigin(rb), tf("Bob's other question about rocks", true)]))).body;
  w.history(Rep);
  await w.json("POST", `/items/${b.item_id}/report`, { user: Rep, json: { reason: "student_data" } });
  await w.purge(admin, a.item_id);
  const rel = await w.admin<any>(admin, "POST", "/admin/pool/release", { item_id: b.item_id, reason: "student_data" });
  assert.deepEqual(rel.body.remaining, [{ reason: "student_data", source: "privacy" }]);
  assert.equal((await get(w, V, b.item_id)).status, 404, "the student data stays hidden");
  assert.equal((await w.admin<any>(admin, "POST", "/admin/pool/release", { item_id: b.item_id, reason: "student_data", source: "privacy" })).status, 409, "needs force");
  const forced = await w.admin<any>(admin, "POST", "/admin/pool/release", { item_id: b.item_id, reason: "student_data", source: "privacy", force: true });
  assert.equal(forced.body.released, 1);
  assert.equal(w.q(`SELECT * FROM pool_audit WHERE action = 'item.release_forced'`).length, 1);
  // A manual 'admin' release never drops a DMCA copy hold.
  const c0 = (await publish(w, A, bankOf([tf("Ann's second distinctive question on enzymes", true)]))).body;
  const rc = await imp(w, B, c0.item_id);
  const bc = (await publish(w, B, bankOf(withOrigin(rc)))).body;
  const td = await w.takedown<any>(admin, await notice(w, admin), c0.item_id);
  assert.deepEqual((await w.holdCopies<any>(admin, td.body.action_id)).body.held, [bc.item_id]);
  await w.admin(admin, "POST", "/admin/pool/hold", { item_id: bc.item_id, note: "checking tone" });
  await w.admin(admin, "POST", "/admin/pool/release", { item_id: bc.item_id, reason: "admin", note: "tone fine" });
  assert.deepEqual(holds(w, bc.item_id), [{ reason: "admin", source: `dmca:${td.body.action_id}` }]);
  assert.equal((await get(w, V, bc.item_id)).status, 404);
});

// ---- F5: reads of common questions; independence ------------------------------------------------------------

test("s10/F5 (round 9): a verbatim read binds; independence covers reads, and imports of common or non-verbatim questions", async () => {
  const w = new World();
  const Xia = w.user("teacher", "Xia"), Rae = w.user("teacher", "Rae"), Eve = w.user();
  const common = tf("Water boils at 100 degrees Celsius at sea level.", true);
  const x = (await publish(w, Xia, bankOf([common, tf("Xia's own NC question about evaporation rates in the open ocean at night", true)]), { license: "CC-BY-NC-4.0" })).body;
  await get(w, Rae, x.item_id);
  const mine = [common, tf("Rae's independent question on condensation", true)];
  const hard = await publish(w, Rae, bankOf(mine), { license: "CC-BY-SA-4.0" });
  assert.deepEqual([hard.status, hard.body.per_question?.[0]?.bound_by?.map((b: Q) => b.author_name)], [422, ["Xia"]], "a verbatim copy of a read binds (round 9 J1)");
  // Round 12 M5: independence never covers an exact copy of what was served... except (round 14 B5)
  // a short or generic question only READ: then it is a possible source, the badge says so.
  const claimed = await publish(w, Rae, bankOf(mine), { license: "CC-BY-SA-4.0", independent: [x.item_id] });
  assert.equal(claimed.status, 201);
  assert.equal((await get(w, Eve, claimed.body.item_id as string)).body.license_check.status, "claimed_independent");
  // ... a changed copy of a read can be (here: Rae's own reworded version).
  const mine2 = [tf("Water boils at one hundred degrees Celsius at sea level, as Rae measured.", true), tf("Rae's independent question on condensation", true)];
  const ind = await publish(w, Rae, bankOf(mine2), { license: "CC-BY-SA-4.0", meta: { title: "Rae", subject: "Physics", course: "", grade: "", tags: [], standards: [] }, independent: [x.item_id] });
  assert.equal(ind.status, 201, JSON.stringify(ind.body));
  assert.deepEqual((await get(w, Eve, ind.body.item_id as string)).body.questions[0].chain, [], "and nobody is credited");
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'independent_claim' AND user_id = ?`, Rae.id).length, 2, "flagged for the admin (both claims)");
  assert.equal(w.q(`SELECT * FROM pool_audit WHERE action = 'item.publish' AND detail LIKE '%independent_of%'`).length, 2, "audited");
  // An import binds a verbatim copy of a distinctive question, and cannot be attested away.
  const Ian = w.user();
  await imp(w, Ian, x.item_id);
  const verb = [tf("Xia's own NC question about evaporation rates in the open ocean at night", true)];
  assert.equal((await publish(w, Ian, bankOf(verb), { license: "CC-BY-SA-4.0" })).body.error, "license_incompatible");
  assert.equal((await publish(w, Ian, bankOf(verb), { license: "CC-BY-SA-4.0", independent: [x.item_id] })).body.error, "license_incompatible");
  // ... but a COMMON question imported (K other authors use it) can be attested independent.
  const w2 = new World();
  const N = w2.user(), Rd = w2.user();
  for (let k = 0; k < 3; k++) await publish(w2, w2.user(), bankOf([common]), { license: "CC-BY-4.0" });
  const nx = (await publish(w2, N, bankOf([common]), { license: "CC-BY-NC-4.0" })).body;
  await imp(w2, Rd, nx.item_id);
  assert.equal((await publish(w2, Rd, bankOf([common]), { license: "CC0-1.0" })).body.error, "license_incompatible", "an imported verbatim copy binds");
  assert.equal((await publish(w2, Rd, bankOf([common]), { license: "CC0-1.0", independent: [nx.item_id] })).status, 201, "a common question can be attested independent");
});


// ---- F6: modified against what was served -------------------------------------------------------------------

test("s11/F6: adding a key and an explanation to a stripped serve is a modification; a hidden copy still compares key-free only", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), V = w.user();
  const a = (await publish(w, A, bankOf([tf("Hidden-key source question about cell membranes", true, { explanation: "Ann's hidden note" })]), { license: "CC-BY-4.0", hideKey: true })).body;
  const rb = await imp(w, B, a.item_id);
  const qs = withOrigin(rb).map((z: Q) => {
    // Bob's own key (share-core now refuses the stripped form's placeholder value as a key).
    const y = { ...z, kind: { Tf: { correct: true } }, explanation: "Bob wrote this whole explanation" };
    delete y.keyless;
    return y;
  });
  const b = (await publish(w, B, bankOf(qs), { license: "CC-BY-4.0" })).body;
  assert.deepEqual((await get(w, V, b.item_id)).body.questions[0].chain.map((l: Q) => [l.author_name, l.modified]), [["Ann", true]]);
  const bh = (await publish(w, B, bankOf(qs), { license: "CC-BY-4.0", hideKey: true })).body;
  assert.deepEqual((await get(w, V, bh.item_id)).body.questions[0].chain.map((l: Q) => [l.author_name, l.modified]), [["Ann", false]], "hidden: key-free facts only");
});

// ---- non-blocking --------------------------------------------------------------------------------------------

test("X4: unread streams are capped per account and per address and time out; the budget is not pinned", async () => {
  // Idle long enough that a slow (loaded) machine cannot time streams out while the caps are probed.
  const w = new World({ readIdleMs: 1500 });
  const a = w.user(), att = w.user(), victim = w.user();
  const p = (await publish(w, a, bankOf([q("A streamed question body for the budget test")]))).body;
  const raw = (u: User, ip: string) => w.pool.handle(new Request(`https://api.example.org/items/${p.item_id}`, { headers: { Authorization: `Bearer ${u.token}`, "CF-Connecting-IP": ip } })) as Promise<Response>;
  const held: Response[] = [];
  const codes: number[] = [];
  for (let i = 0; i < 12; i++) {
    const r = await raw(att, `10.0.0.${i}`);
    codes.push(r.status);
    if (r.status === 200) held.push(r);
    else await r.text();
  }
  assert.deepEqual(codes, [...Array(4).fill(200), ...Array(8).fill(429)]);
  // Per address (round 7): 2 per account on one address, 8 in all on it.
  const perIp: Response[] = [];
  for (let i = 0; i < 9; i++) perIp.push(await raw(w.user(), "10.9.9.9"));
  assert.deepEqual(perIp.map((r) => r.status), [...Array(8).fill(200), 429], "per address too");
  const one = w.user();
  const mine: Response[] = [];
  for (let i = 0; i < 3; i++) mine.push(await raw(one, "10.9.9.10"));
  assert.deepEqual(mine.map((r) => r.status), [200, 200, 429], "one account takes at most 2 of an address's slots");
  for (const r of [...perIp, ...mine]) await r.body?.cancel().catch(() => {});
  assert.equal((await publish(w, victim, bankOf([q("Victim's question that should publish fine")]))).status, 201);
  await new Promise((r) => setTimeout(r, 1800));
  assert.equal(bytesReserved(), 0, "idle streams were cancelled and released");
  const again = await raw(att, "10.0.1.1");
  assert.equal(again.status, 200, "slots came back");
  await again.text();
  for (const r of held) await r.body?.cancel().catch(() => {});
  assert.equal(bytesReserved(), 0);
});

test("X5: the body must be exactly its declared Content-Length", async () => {
  const w = new World();
  const u = w.user();
  const big = bankOf(Array.from({ length: 40 }, (_, i) => q(`Question ${i} padding ${"x".repeat(200)}`)));
  const body = framed({ publish_id: publishId(), hide_key: false, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: "2026-10-22" }, labels: [], order_acks: [], image_ok: [], visibility: "public", meta: { title: "t", subject: "s", course: "", grade: "", tags: [], standards: [] } }, big);
  for (const declared of [10, body.length + 5]) {
    const r = await w.pool.handle(new Request("https://api.example.org/items", { method: "POST", headers: { Authorization: `Bearer ${u.token}`, "CF-Connecting-IP": "10.0.0.1", "Content-Type": PUBLISH_CONTENT_TYPE, "Content-Length": String(declared) }, body: new ReadableStream({ start: (c) => (c.enqueue(body), c.close()) }), duplex: "half" } as RequestInit));
    assert.deepEqual([r!.status, ((await r!.json()) as Q).error], [400, "length_mismatch"]);
  }
  assert.equal(bytesReserved(), 0);
});

test("affected lists fail closed: more than the list needs confirm_large, and every copy is held (in batches, continued by the cron)", async () => {
  const w = new World({ affectedList: 3 });
  const admin = w.user("admin"), A = w.user(), Eve = w.user();
  const pii = tf("Student Jamie Smith in period 3 failed the retake", true);
  const a = (await publish(w, A, bankOf([pii]))).body;
  const copies: string[] = [];
  for (let k = 0; k < 7; k++) {
    const u = w.user();
    const r = await imp(w, u, a.item_id);
    copies.push((await publish(w, u, bankOf(withOrigin(r)))).body.item_id as string);
  }
  // Seven copies make the question "common" (K = 3): the admin selects it explicitly.
  const pv = await w.admin<any>(admin, "POST", "/admin/pool/purge-preview", { item_id: a.item_id, questions: [0] });
  assert.deepEqual([pv.body.affected.length, pv.body.affected_total, pv.body.large], [3, 7, true]);
  const no = await w.admin<any>(admin, "POST", "/admin/pool/purge", { item_id: a.item_id, questions: [0], confirm_preview: pv.body.confirm_preview });
  assert.deepEqual([no.status, no.body.error], [428, "confirm_large_required"]);
  const pv2 = await w.admin<any>(admin, "POST", "/admin/pool/purge-preview", { item_id: a.item_id, questions: [0] });
  const yes = await w.admin<any>(admin, "POST", "/admin/pool/purge", { item_id: a.item_id, questions: [0], confirm_preview: pv2.body.confirm_preview, confirm_large: true });
  assert.equal(yes.status, 200);
  for (const id of copies) assert.equal((await get(w, Eve, id)).status, 404, "every copy held, not only the listed ones");
});

test("preview tokens bind the admin, the notice and block_days, and are single-use; hold-copies drops common questions", async () => {
  const w = new World();
  const admin = w.user("admin"), other = w.user("admin"), A = w.user(), B = w.user();
  const p = (await publish(w, A, bankOf([tf("Distinctive question by A about photosynthesis rates.", true)]))).body;
  const n1 = await notice(w, admin), n2 = await notice(w, admin);
  const pv = await w.admin<any>(admin, "POST", "/admin/dmca/takedown-preview", { item_id: p.item_id, notice_id: n1, block_days: 30 });
  const tok = pv.body.confirm_preview;
  const try_ = (who: User, body: Q) => w.admin<any>(who, "POST", "/admin/dmca/takedown", { item_id: p.item_id, confirm_preview: tok, ...body });
  assert.equal((await try_(other, { notice_id: n1, block_days: 30 })).status, 428, "another admin");
  assert.equal((await try_(admin, { notice_id: n2, block_days: 30 })).status, 428, "another notice");
  assert.equal((await try_(admin, { notice_id: n1, block_days: 365 })).status, 428, "another block length");
  assert.equal((await try_(admin, { notice_id: n1, block_days: 30 })).status, 200);
  // Single use: a restored item, the same plan, the same token: refused the second time.
  await w.admin(admin, "POST", "/admin/dmca/withdraw", { notice_id: n1 });
  assert.equal((await try_(admin, { notice_id: n1, block_days: 30 })).status, 428, "a used token");
  // hold-copies: a designated question that K unrelated authors also use is dropped.
  const common = tf("The Earth orbits the Sun.", true);
  const xa = (await publish(w, A, bankOf([common, tf("A's distinctive question two about leaves", true)]))).body;
  const rb = await imp(w, B, xa.item_id);
  await publish(w, B, bankOf([withOrigin(rb)[0]]));
  for (let k = 0; k < 3; k++) await publish(w, w.user(), bankOf([common]));
  const td = await w.takedown<any>(admin, await notice(w, admin), xa.item_id);
  const mh = w.q<{ match_hash: string }>(`SELECT match_hash FROM item_questions WHERE publish_id = (SELECT current_publish_id FROM items WHERE id = ?) AND pos = 0`, xa.item_id)[0]!.match_hash;
  const hp = await w.admin<any>(admin, "POST", "/admin/dmca/hold-copies-preview", { action_id: td.body.action_id, questions: [mh] });
  assert.deepEqual([hp.body.dropped_common, hp.body.copies], [[mh], []]);
});

test("getItem: a hold landing mid-read serves no body; counter-notice works under a miss lockout; HEAD, ETag, 304 and Allow", async () => {
  const w = new World();
  const admin = w.user("admin"), A = w.user(), B = w.user();
  const p = (await publish(w, A, bankOf([q("A question that will be held mid-read")]))).body;
  w.r2.onGet = async () => void w.sql.prepare(`INSERT INTO item_holds (item_id, reason, at, source) VALUES (?, 'admin', 0, 'manual')`).run(p.item_id);
  assert.equal((await get(w, B, p.item_id)).status, 404);
  assert.equal(w.q(`SELECT * FROM imports WHERE user_id = ?`, B.id).length, 0, "nothing recorded");
  w.sql.prepare(`DELETE FROM item_holds`).run();
  // HEAD / ETag / 304 / Content-Length.
  const g = await w.call("GET", `/items/${p.item_id}`, { user: B });
  const etag = g.headers.get("ETag")!;
  const len = Number(g.headers.get("Content-Length"));
  assert.ok(etag && len === (await g.arrayBuffer()).byteLength);
  const h = await w.call("HEAD", `/items/${p.item_id}`, { user: B });
  assert.deepEqual([h.status, h.headers.get("ETag"), Number(h.headers.get("Content-Length")), (await h.arrayBuffer()).byteLength], [200, etag, len, 0]);
  const nm = await w.call("GET", `/items/${p.item_id}`, { user: B, headers: { "If-None-Match": etag } });
  assert.deepEqual([nm.status, (await nm.arrayBuffer()).byteLength], [304, 0]);
  // A hidden-only republish (same served form) keeps the ETag a non-author sees.
  const ph = (await publish(w, A, bankOf([q("Hidden ETag question", { explanation: "one" })]), { hideKey: true })).body;
  const e1 = (await w.call("GET", `/items/${ph.item_id}`, { user: B })).headers.get("ETag");
  await publish(w, A, bankOf(own(ph, [q("Hidden ETag question", { explanation: "two" })])), { hideKey: true, base: ph.publish_id }, ph.item_id);
  assert.equal((await w.call("GET", `/items/${ph.item_id}`, { user: B })).headers.get("ETag"), e1, "nothing reveals the hidden change");
  const m405 = await w.call("POST", `/items/${p.item_id}`, { user: B });
  assert.equal(m405.headers.get("Allow"), "GET, HEAD, PUT, PATCH, DELETE");
  // Counter-notice under a miss lockout.
  const n = await notice(w, admin);
  await w.takedown(admin, n, p.item_id);
  w.sql.prepare(`INSERT INTO usage_daily (user_id, day, metric, count) VALUES (?, ?, 'item_miss', 1000)`).run(A.id, Math.floor(w.clock / DAY));
  const cn = await w.json("POST", `/items/${p.item_id}/counter-notice`, { user: A, json: { signature: "A", material_desc: "m", name: "A", address: "addr", phone: "1", perjury_stmt: true, consent_jurisdiction: true, accept_service: true } });
  assert.equal(cn.status, 202);
  // Withdraw: an unknown notice is a 404, a real one is audited.
  assert.equal((await w.admin(admin, "POST", "/admin/dmca/withdraw", { notice_id: "0".repeat(32) })).status, 404);
  await w.admin(admin, "POST", "/admin/dmca/withdraw", { notice_id: n });
  assert.equal(w.q(`SELECT * FROM pool_audit WHERE action = 'dmca.withdraw'`).length, 1);
});

test("PUT /blobs: a privacy-blocked image gets exactly the answer any well-formed image gets (round 16: the uploader knows their file is well formed)", async () => {
  const w = new World();
  const admin = w.user("admin"), a = w.user(), u = w.user(), R = w.user();
  const X = png(8, 8, 11);
  const up = await upload(w, a, X);
  const p = (await publish(w, a, bankOf([tf("Student Jo Bloggs in period 2, see photo", true, { image: img(up.sha) })]))).body;
  await w.purge(admin, p.item_id);
  const blocked = await upload(w, u, X);
  const fresh = await upload(w, u, png(8, 8, 12));
  const shape = (r: { status: number; body: Record<string, unknown> }) => [r.status, Object.keys(r.body).sort(), r.body.width, r.body.height];
  assert.deepEqual(shape(blocked), shape(fresh));
  // Publishing it: accepted, held by the block, never served.
  const pb = await publish(w, u, bankOf([tf("My own photo question about leaves in autumn", true, { image: img(up.sha) })]));
  assert.equal(pb.status, 201);
  assert.equal((await w.json("GET", `/items/${pb.body.item_id}`, { user: R })).status, 404);
});

test("privacy purge UX: the author sees removed_privacy with a notice and can appeal for 14 days", async () => {
  const w = new World();
  const admin = w.user("admin"), a = w.user();
  const p = (await publish(w, a, bankOf([tf("Student Jo Bloggs in period 2 failed", true)]))).body;
  await w.purge(admin, p.item_id);
  const m = await w.json<any>("GET", "/items/mine", { user: a });
  assert.deepEqual([m.body.items[0].state, m.body.items[0].appeal, typeof m.body.items[0].notice], ["removed_privacy", "available", "string"]);
  assert.equal((await w.json("POST", `/items/${p.item_id}/appeal`, { user: a, json: { note: "It is a made-up name from the textbook." } })).status, 202);
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'appeal'`).length, 1);
  // Round 9 J5: the open appeal stops the purge until the admin decides.
  w.advance(15 * DAY);
  await w.pool.daily();
  assert.equal((await w.json<any>("GET", "/items/mine", { user: a })).body.items.length, 1, "kept while the appeal is open");
  assert.equal((await w.admin(admin, "POST", "/admin/pool/appeal-decision", { item_id: p.item_id, decision: "purge" })).status, 200);
  await w.pool.daily();
  assert.equal((await w.json<any>("GET", "/items/mine", { user: a })).body.items.length, 0);
  assert.equal((await w.json("POST", `/items/${p.item_id}/appeal`, { user: a, json: { note: "again" } })).status, 404);
});

test("diamond imports: one link per item, never your own item, modified = OR over paths", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob"), C = w.user("teacher", "Cat"), V = w.user();
  const Q0 = tf("Photosynthesis converts light energy into chemical energy stored in glucose", true);
  const a = (await publish(w, A, bankOf([Q0]))).body;
  const b = (await publish(w, B, bankOf(withOrigin(await imp(w, B, a.item_id), (z) => ({ ...z, explanation: BOB_NC }))), { license: "CC-BY-NC-4.0" })).body;
  const back = await imp(w, A, b.item_id);
  const a2 = await publish(w, A, bankOf(own(a, [{ ...Q0, explanation: BOB_NC }])), { license: "CC-BY-NC-4.0", base: a.publish_id }, a.item_id);
  assert.equal(a2.status, 200);
  void back;
  assert.deepEqual((await get(w, V, a.item_id)).body.questions[0].chain.map((l: Q) => l.author_name), ["Bob"], "Ann's own item is not in Ann's chain");
  await imp(w, C, a.item_id);
  const rc = await imp(w, C, b.item_id);
  const c = (await publish(w, C, bankOf(withOrigin(rc, (z) => ({ ...z, explanation: "Cat's merge" }))), { license: "CC-BY-NC-SA-4.0" })).body;
  const chain = (await get(w, V, c.item_id)).body.questions[0].chain as Q[];
  assert.deepEqual(chain.map((l) => l.author_name).sort(), ["Ann", "Bob"]);
  assert.equal(new Set(chain.map((l) => l.item_id)).size, chain.length, "one link per item");
  assert.ok(chain.every((l) => l.modified));
  assert.equal(chain.find((l) => l.author_name === "Ann")!.version, 1, "the earliest version");
});

test("browse filters fold case in every script", async () => {
  const w = new World();
  const a = w.user(), r = w.user();
  await publish(w, a, bankOf([tf("Ökologie Frage eins", true)]), { meta: { title: "Ärzte und Öko", subject: "BIOLOGÍA", course: "", grade: "", tags: [], standards: [] } });
  const bySubject = await w.json<any>("GET", `/items?subject=${encodeURIComponent("biología")}`, { user: r });
  const byTitle = await w.json<any>("GET", `/items?q=${encodeURIComponent("ärz")}`, { user: r });
  assert.deepEqual([bySubject.body.items.length, byTitle.body.items.length], [1, 1]);
});
