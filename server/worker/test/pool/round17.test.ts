// Round 17: critics 22c (Moodle), 22e (ASVS) and the client additions, inverted. Material a question
// uses besides itself (texts, images, question units) binds only from the version the account was
// served (M1), never its own author (M2); a near stem with another body is another question (M3);
// questions wrapped into or taken out of a Multipart are the same question (M4); re-encoded images and
// relays of a text or an image are found (M5); image credits are typed (M6); the raw-bytes scan
// refuses a BOM and non-JSON tokens; a blocked image's publish is indistinguishable from a control's
// (A1); no publish or import while the backfill runs (A2, round16_migrate.test.ts); derived rows are
// pruned and capped (A3); a dry-run publish stores nothing; `key_leak` reports.
import assert from "node:assert/strict";
import { test } from "node:test";
import { deflateSync } from "node:zlib";
import { pngPixelHash } from "../../src/pool/image.ts";
import { scanJson } from "../../src/pool/jsonscan.ts";
import { sameServed } from "../../src/pool/publish.ts";
import { chunk, concat, decideLabels, framed, png, publish, publishId, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const img = (sha: string) => ({ file: `media/${sha}.png`, width: 9, height: 9 });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const out = (r: { status: number; body: Q }) => (r.status === 422 && r.body.error === "license_incompatible" ? `422${JSON.stringify(r.body.allowed)}` : String(r.status));
const flags = (w: World, id: unknown) => w.q<{ kind: string }>(`SELECT kind FROM admin_flags WHERE item_id = ? ORDER BY kind`, id).map((x) => x.kind);
const holds = (w: World, id: unknown) => w.q<{ reason: string; source: string }>(`SELECT reason, source FROM item_holds WHERE item_id = ? ORDER BY reason, source`, id);
const chain = async (w: World, u: User, id: unknown) => ((await get(w, u, id as string)).body.questions as Q[]).map((x) => (x.chain as Q[]).map((l) => [l.author_name, l.kind ?? null, l.earlier_match ?? false]));
const NCSA = `422${JSON.stringify(["CC-BY-NC-SA-4.0"])}`;
const OS = "Photosynthesis is the process by which plants, algae and some bacteria use light energy to convert carbon dioxide and water into glucose and oxygen, storing chemical energy in the bonds of sugar.";
const ANNQ = () => mcq("In Ann's field study of the delta, which factor best explains the 2019 decline of juvenile Chinook salmon at the Freeport trap?", [["Elevated summer water temperature above 20 C", true], ["Predation by striped bass", false], ["Reduced turbidity", false], ["Lower dissolved oxygen", false]]);
const own = (pub: any, qs: Q[]) => qs.map((x, i) => ({ ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } }));
/** The same pixels as png(w, h, seed), deflated at another level: other bytes, the same image. */
function pngLevel(w: number, h: number, seed: number, level: number) {
  const ihdr = new Uint8Array(13);
  const dv = new DataView(ihdr.buffer);
  dv.setUint32(0, w);
  dv.setUint32(4, h);
  ihdr.set([8, 0, 0, 0, 0], 8);
  const raw = new Uint8Array((w + 1) * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) raw[y * (w + 1) + 1 + x] = (x * 7 + y * 13 + seed) & 0xff;
  return concat(new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw, { level })), chunk("IEND", new Uint8Array(0)));
}

test("M1 (S1a-c): an explanation or image binds only from the version the account was served; a claim attributing a public source makes it a possible source", async () => {
  const Q0 = mcq("Which gas do plants release during the light reactions of photosynthesis in a leaf?", [["Oxygen", true], ["Carbon dioxide", false], ["Nitrogen", false]]);
  const mine = q("Tess: describe in your own words what photosynthesis is and where in the cell it happens.", { kind: { Short: { sample: "chloroplast" } }, explanation: OS });
  // S1b: Tess read v1 (no explanation); v2 adds it; Tess never saw v2.
  {
    const w = new World();
    const A = w.user("teacher", "Ann"), T = w.user("teacher", "Tess");
    const a = (await publish(w, A, bankOf([Q0]), { license: "CC-BY-NC-SA-4.0" })).body;
    await get(w, T, a.item_id as string);
    w.advance(DAY);
    assert.equal((await publish(w, A, bankOf(own(a, [{ ...Q0, explanation: OS }])), { license: "CC-BY-NC-SA-4.0", base: a.publish_id as string }, a.item_id as string)).status, 200);
    assert.equal((await publish(w, T, bankOf([mine]), { license: "CC-BY-4.0" })).status, 201, "S1b: a version never seen does not bind");
    // ... once she reads v2, it does.
    await get(w, T, a.item_id as string);
    assert.equal(out(await publish(w, T, bankOf([mine]), { license: "CC-BY-4.0" })), NCSA, "S1b: the version seen binds");
  }
  // S1c: the same with an image added in v2.
  {
    const w = new World();
    const A = w.user("teacher", "Ann"), T = w.user("teacher", "Tess");
    const Qi = q("Ann: label the parts of the flower shown in the diagram from the unit.");
    const a = (await publish(w, A, bankOf([Qi]), { license: "CC-BY-NC-SA-4.0" })).body;
    await get(w, T, a.item_id as string);
    const up = await upload(w, A, png(9, 9, 77));
    assert.equal((await publish(w, A, bankOf(own(a, [{ ...Qi, image: img(up.sha) }])), { license: "CC-BY-NC-SA-4.0", base: a.publish_id as string }, a.item_id as string)).status, 200);
    await upload(w, T, png(9, 9, 77));
    assert.equal((await publish(w, T, bankOf([q("Tess: which part of this flower produces pollen? Name it.", { image: img(up.sha) })]), { license: "CC-BY-4.0" })).status, 201, "S1c");
  }
  // S1a: both quoted OpenStax; Tess read Ann's item with it: bound, unless her claim attributes it.
  {
    const w = new World();
    const A = w.user("teacher", "Ann"), T = w.user("teacher", "Tess"), R = w.user();
    const a = (await publish(w, A, bankOf([{ ...Q0, explanation: OS }]), { license: "CC-BY-NC-SA-4.0" })).body;
    await get(w, T, a.item_id as string);
    assert.equal(out(await publish(w, T, bankOf([mine]), { license: "CC-BY-4.0" })), NCSA);
    assert.equal(out(await publish(w, T, bankOf([mine]), { license: "CC-BY-4.0", independent: [a.item_id as string], statement: "I wrote this myself." })), NCSA, "a claim without an attribution");
    const ok = await publish(w, T, bankOf([mine]), { license: "CC-BY-4.0", independent: [a.item_id as string], statement: "Quoted from OpenStax Biology 2e (CC-BY), not from this item." });
    assert.equal(ok.status, 201);
    assert.deepEqual(await chain(w, R, ok.body.item_id), [[["Ann", "text", false]]]);
    assert.equal((await get(w, R, ok.body.item_id as string)).body.license_check.status, "claimed_independent");
  }
});

test("M2 (S8): the original author is never bound by a copy of her own explanation or image, even after her item is purged", async () => {
  const TE = "Tess's own worked explanation: the logger shows water above twenty degrees on forty days of the 2019 season, juvenile counts fell in exactly those weeks, and the slope links them clearly.";
  for (const unit of ["explanation", "image"]) {
    const w = new World();
    const T = w.user("teacher", "Tess"), A = w.user("teacher", "Ann");
    const ut = await upload(w, T, png(9, 9, 91));
    const tq = unit === "explanation" ? mcq("Tess: which factor explains the salmon decline at the trap?", [["Temperature", true], ["Predation", false]], { explanation: TE }) : q("Tess's photo of the trap: what is the device in the picture used for?", { image: img(ut.sha) });
    const t1 = (await publish(w, T, bankOf([tq]), { license: "CC-BY-4.0" })).body;
    w.advance(DAY);
    await imp(w, A, t1.item_id as string);
    const ua = await upload(w, A, png(9, 9, 91));
    const aq = unit === "explanation" ? mcq("Ann: why did juvenile salmon numbers fall in 2019 on the lower river?", [["Warm water", true], ["Bass", false]], { explanation: TE }) : q("Ann: name the structure on the trap shown here and its purpose.", { image: img(ua.sha) });
    const a = await publish(w, A, bankOf([aq]), { license: "CC-BY-NC-SA-4.0" });
    assert.equal(a.status, 201, `${unit}: Ann's adaptation`);
    await w.json("DELETE", `/items/${t1.item_id}`, { user: T });
    w.advance(15 * DAY);
    await w.pool.daily();
    await w.pool.hourly();
    await get(w, T, a.body.item_id as string);
    await upload(w, T, png(9, 9, 91));
    assert.equal((await publish(w, T, bankOf([tq]), { license: "CC-BY-4.0" })).status, 201, unit);
  }
});

test("M3 (S3): a template stem with another body is another question: never credited, bound or labelled; the admin may be told", async () => {
  const MITO = () => mcq("Which of the following best describes the function of the mitochondria in a eukaryotic cell?", [["It produces ATP through cellular respiration", true], ["It stores genetic information", false], ["It synthesizes proteins", false], ["It packages lipids", false]]);
  const RIBO = () => mcq("Which of the following best describes the function of the ribosome in a eukaryotic cell?", [["It assembles amino acids into proteins", true], ["It digests worn-out organelles", false], ["It controls what enters the cell", false], ["It captures light energy", false]]);
  for (const how of ["none", "read", "import"]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), T = w.user(), R = w.user();
    const a = (await publish(w, A, bankOf([MITO()]), { license: "CC-BY-NC-SA-4.0" })).body;
    w.advance(DAY);
    if (how === "read") await get(w, T, a.item_id as string);
    if (how === "import") await imp(w, T, a.item_id as string);
    const r = await publish(w, T, bankOf([RIBO()]), { license: "CC-BY-4.0" });
    assert.equal(r.status, 201, how);
    assert.deepEqual(await chain(w, R, r.body.item_id), [[]], how);
    assert.equal((await get(w, R, r.body.item_id as string)).body.license_check.status, "ok", how);
    assert.ok(!flags(w, r.body.item_id).includes("license_soft"), how);
  }
  // The same body with a near stem still binds (a one-word edit of an import).
  const w = new World();
  const A = w.user(), T = w.user();
  const a = (await publish(w, A, bankOf([MITO()]), { license: "CC-BY-NC-SA-4.0" })).body;
  await imp(w, T, a.item_id as string);
  assert.equal(out(await publish(w, T, bankOf([{ ...MITO(), stem: MITO().stem.replace("eukaryotic", "animal") }]), { license: "CC0-1.0" })), NCSA);
});

test("M4 (S5, S5b): a question wrapped into a Multipart, or a part taken out, is the same question (read, import, downstream)", async () => {
  const wrap = () => q("Answer both parts.", { kind: { Multipart: { parts: [{ stem: ANNQ().stem, kind: { Mc: { answers: ANNQ().kind.Mc.answers } } }, { stem: "Name the river.", kind: { Short: { sample: "Sacramento" } } }] } } });
  for (const how of ["read", "import"]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user(), R = w.user();
    const a = (await publish(w, A, bankOf([ANNQ()]), { license: "CC-BY-NC-SA-4.0" })).body;
    w.advance(DAY);
    if (how === "read") await get(w, M, a.item_id as string);
    else await imp(w, M, a.item_id as string);
    assert.equal(out(await publish(w, M, bankOf([wrap()]), { license: "CC0-1.0" })), NCSA, `wrap ${how}`);
    const ok = await publish(w, M, bankOf([wrap()]), { license: "CC-BY-NC-SA-4.0" });
    assert.deepEqual(await chain(w, R, ok.body.item_id), [[["Ann", "part", false]]]);
  }
  // Unwrapping: a part of Ann's Multipart published alone.
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user();
  const a = (await publish(w, A, bankOf([wrap()]), { license: "CC-BY-NC-SA-4.0" })).body;
  await imp(w, M, a.item_id as string);
  assert.equal(out(await publish(w, M, bankOf([ANNQ()]), { license: "CC0-1.0" })), NCSA, "unwrap");
});

test("M5: a re-encoded image is the same image (decoded pixels); a text, image or part relayed through an account never served it is an earlier publication", async () => {
  // (a) the pixel hash: equal for re-deflated bytes, different for other pixels.
  const h1 = await pngPixelHash(png(9, 9, 31));
  assert.ok(h1 && h1 === (await pngPixelHash(pngLevel(9, 9, 31, 1))));
  assert.notEqual(h1, await pngPixelHash(png(9, 9, 32)));
  {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user();
    const ua = await upload(w, A, png(9, 9, 31));
    const a = (await publish(w, A, bankOf([{ ...ANNQ(), image: img(ua.sha) }]), { license: "CC-BY-NC-SA-4.0" })).body;
    await imp(w, M, a.item_id as string);
    const u = await upload(w, M, pngLevel(9, 9, 31, 1));
    assert.notEqual(u.sha, ua.sha);
    assert.equal(out(await publish(w, M, bankOf([q("Mal's own question: what does this picture show?", { image: img(u.sha) })]), { license: "CC0-1.0" })), NCSA, "re-encoded");
  }
  // (b) relays.
  for (const what of ["text", "image", "part"]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), P = w.user(), R = w.user();
    const ua = await upload(w, A, png(9, 9, 61));
    const LONG = "Ann's own explanation: the logger shows water above twenty degrees on forty days of the 2019 season, juvenile counts fell in exactly those weeks, and the regression slope links them clearly.";
    await publish(w, A, bankOf([{ ...ANNQ(), explanation: LONG, image: img(ua.sha) }]), { license: "CC-BY-NC-SA-4.0" });
    w.advance(DAY);
    const up = await upload(w, P, png(9, 9, 61));
    const body =
      what === "text"
        ? mcq("Pip's own question about rivers?", [["Yes", true], ["No", false]], { explanation: LONG })
        : what === "image"
          ? q("Pip's own question: what does this picture show?", { image: img(up.sha) })
          : q("Answer both parts.", { kind: { Multipart: { parts: [{ stem: ANNQ().stem, kind: { Mc: { answers: ANNQ().kind.Mc.answers } } }, { stem: "Name the river.", kind: { Short: { sample: "Sacramento" } } }] } } });
    const p = await publish(w, P, bankOf([body]), { license: "CC0-1.0" });
    assert.equal(p.status, 201, what);
    assert.equal((await get(w, R, p.body.item_id as string)).body.license_check.status, "unverified_earlier_match", what);
    assert.ok(flags(w, p.body.item_id).includes("license_soft"), what);
  }
});

test("M6 (S9): an image two peers serve is credited as an image (typed), and the same material from both is one: any grant of either applies", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bo"), T = w.user(), R = w.user();
  const ids: string[] = [];
  for (const [u, lic, s] of [[A, "CC-BY-4.0", "Ann: using the periodic table shown, which element has atomic number 26?"], [B, "CC-BY-NC-SA-4.0", "Bo: using the periodic table, name the group that contains the noble gases."]] as const) {
    const up = await upload(w, u, png(9, 9, 120));
    ids.push((await publish(w, u, bankOf([q(s, { image: img(up.sha) })]), { license: lic })).body.item_id as string);
  }
  for (const id of ids) await get(w, T, id);
  const up = await upload(w, T, png(9, 9, 120));
  const r = await publish(w, T, bankOf([q("Tess: using this periodic table, which period is sodium in?", { image: img(up.sha) })]), { license: "CC-BY-4.0" });
  assert.equal(r.status, 201);
  assert.deepEqual((await chain(w, R, r.body.item_id))[0]!.map((l) => l[1]), ["image", "image"]);
  // A list view alone is not "served".
  const w2 = new World();
  const A2 = w2.user(), T2 = w2.user();
  const up2 = await upload(w2, A2, png(9, 9, 121));
  await publish(w2, A2, bankOf([q("Ann's diagram question about cells.", { image: img(up2.sha) })]), { license: "CC-BY-NC-SA-4.0" });
  await w2.json("GET", "/items?limit=50", { user: T2 });
  await upload(w2, T2, png(9, 9, 121));
  const r2 = await publish(w2, T2, bankOf([q("Tess's own cell question with a picture.", { image: img(up2.sha) })]), { license: "CC0-1.0" });
  assert.equal(r2.status, 201, "browsing the list is no serve (an earlier publication at most)");
});

test("22e S8: an image the claimant uploaded before the other item was published is hers: her claim covers it", async () => {
  const w = new World();
  const ann = w.user("teacher", "Ann"), mal = w.user("teacher", "Mal");
  const photo = png(16, 16, 42);
  const au = await upload(w, ann, photo); // Ann had her photo first
  w.advance(DAY);
  const mu = await upload(w, mal, photo);
  const m = (await publish(w, mal, bankOf([q("Is this leaf deciduous? Explain how you can tell from the photo.", { image: { file: `media/${mu.sha}.png`, width: 16, height: 16 } })]), { license: "CC-BY-NC-4.0" })).body;
  await get(w, ann, m.item_id as string);
  const mine = q("Which tree did I photograph on our field trip, an oak or a maple?", { image: { file: `media/${au.sha}.png`, width: 16, height: 16 } });
  assert.equal((await publish(w, ann, bankOf([mine]), { license: "CC-BY-4.0" })).status, 422, "without a claim the serve binds");
  assert.equal((await publish(w, ann, bankOf([mine]), { license: "CC-BY-4.0", independent: [m.item_id as string], statement: "This is my own photograph, taken on our class field trip in 2024." })).status, 201);
  // A rival who uploaded after cannot do the same.
  const w2 = new World();
  const a2 = w2.user(), r2 = w2.user();
  const ua = await upload(w2, a2, photo);
  const ai = (await publish(w2, a2, bankOf([q("Ann's own photo question about leaves in autumn.", { image: { file: `media/${ua.sha}.png`, width: 16, height: 16 } })]), { license: "CC-BY-NC-4.0" })).body;
  w2.advance(DAY);
  await get(w2, r2, ai.item_id as string);
  const ur = await upload(w2, r2, photo);
  assert.equal((await publish(w2, r2, bankOf([q("Rival's question about the photo of a leaf.", { image: { file: `media/${ur.sha}.png`, width: 16, height: 16 } })]), { license: "CC-BY-4.0", independent: [ai.item_id as string], statement: "This is my own photograph." })).status, 422);
});

test("ASVS: the raw-bytes scan refuses a BOM, non-JSON tokens and a second top-level value", async () => {
  const e = (s: string) => new TextEncoder().encode(s);
  assert.equal(scanJson(new Uint8Array([0xef, 0xbb, 0xbf, ...e('{"a":1}')])).code, "bom");
  for (const s of ['{"a":tru}', '{"a":1x}', '{} {}', '{"a":NaN}', '{"a":\u00011}', '{"a":Infinity}']) assert.equal(scanJson(e(s)).ok, false, s);
  assert.equal(scanJson(e('{"a":[1,-2.5e3,true,false,null,"x"]}')).ok, true);
  const w = new World();
  const A = w.user();
  const r = await publish(w, A, `﻿${bankOf([q("A question about rivers?")])}`);
  assert.equal(r.status, 422);
});

test("22e A1: a privacy-blocked image's publish is indistinguishable from a control's, in the grace window and after it; other accounts keep their uploads", async () => {
  for (const after of [false, true]) {
    const w = new World();
    const admin = w.user("admin"), a = w.user(), m = w.user(), R = w.user();
    const X = png(8, 8, 11);
    const early = await upload(w, m, X); // M uploaded it before the block (S1b)
    const ctrlEarly = await upload(w, m, png(8, 8, 13));
    const up = await upload(w, a, X);
    const p = (await publish(w, a, bankOf([q("Student Jo Bloggs in period 2, see photo of the worksheet", { image: { file: `media/${up.sha}.png`, width: 8, height: 8 } })]))).body;
    assert.equal((await w.purge(admin, p.item_id as string)).status, 200);
    assert.equal(w.q(`SELECT * FROM pending_blobs WHERE uploader_id = ? AND sha256 = ?`, m.id, X.length ? early.sha : "").length, 1, "M's pending upload is kept");
    if (after) {
      w.advance(15 * DAY);
      await w.pool.daily();
      w.advance(2 * 3_600_000);
      await w.pool.hourly();
      await upload(w, m, X);
      await upload(w, m, png(8, 8, 13));
    }
    const pb = await publish(w, m, bankOf([q("My own photo question about autumn leaves colour", { image: { file: `media/${early.sha}.png`, width: 8, height: 8 } })]));
    const pc = await publish(w, m, bankOf([q("My own photo question about spring leaves colour", { image: { file: `media/${ctrlEarly.sha}.png`, width: 8, height: 8 } })]));
    assert.deepEqual([pb.status, Object.keys(pb.body).sort()], [pc.status, Object.keys(pc.body).sort()], `after=${after}`);
    assert.equal((await get(w, R, pb.body.item_id as string)).status, 404, "held");
  }
});

test("22e A3: derived rows are bounded: republishing a bank prunes its public bands, a second read writes nothing, part keys are capped", async () => {
  const w = new World();
  const A = w.user(), R = w.user();
  const W = ["delta", "estuary", "sediment", "tide", "marsh", "levee", "channel", "flood", "basin", "current", "salinity", "plume", "silt", "dune", "lagoon", "spit", "bar", "creek", "slough", "reef"];
  const stem = (k: number, v: number) => `Question ${k} revision ${v}: ${Array.from({ length: 10 }, (_, j) => `${W[(k * 7 + v * 13 + j * 3) % W.length]}${v}${j}`).join(" ")}?`;
  let pub: any = (await publish(w, A, bankOf(Array.from({ length: 40 }, (_, k) => q(stem(k, 0)))))).body;
  const bands = () => w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM stem_bands WHERE user_id = 'pub' OR user_id LIKE 'own:%'`)[0]!.n;
  const series: number[] = [bands()];
  for (let v = 1; v <= 5; v++) {
    const r = await publish(w, A, bankOf(own(pub, Array.from({ length: 40 }, (_, k) => q(stem(k, v))))), { base: pub.publish_id }, pub.item_id);
    assert.equal(r.status, 200);
    pub = { ...r.body, item_id: pub.item_id };
    series.push(bands());
  }
  assert.ok(series[5]! <= series[1]! * 1.2, JSON.stringify(series));
  // A second full read writes no evidence rows.
  await get(w, R, pub.item_id);
  const rows = () => w.q<{ n: number }>(`SELECT (SELECT COUNT(*) FROM import_questions WHERE user_id = ?1) + (SELECT COUNT(*) FROM stem_bands WHERE user_id = ?1) AS n`, R.id)[0]!.n;
  const n1 = rows();
  await get(w, R, pub.item_id);
  assert.equal(rows(), n1);
  // Part keys: a bank of 100 Multiparts of 50 parts writes at most PART_KEYS_MAX rows.
  const parts = Array.from({ length: 50 }, (_, k) => ({ stem: `Part ${k} asks something specific about the river delta sediment load?`, kind: { Tf: { correct: true } } }));
  const big = (await publish(w, A, bankOf(Array.from({ length: 100 }, (_, k) => q(`Multipart ${k} about deltas?`, { kind: { Multipart: { parts } } }))))).body;
  assert.ok(w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM item_part_keys WHERE publish_id = ?`, big.publish_id)[0]!.n <= 4000);
});

test("22e S9: images only deleted items keep count toward the removed-items cap (uploads refused past it)", async () => {
  const noisy = (n: number) => {
    const ihdr = new Uint8Array(13);
    const dv = new DataView(ihdr.buffer);
    dv.setUint32(0, 100);
    dv.setUint32(4, 100);
    ihdr.set([8, 0, 0, 0, 0], 8);
    const raw = new Uint8Array(101 * 100);
    let s = n * 2654435761;
    for (let i = 0; i < raw.length; i++) raw[i] = i % 101 === 0 ? 0 : (s = (s * 1103515245 + 12345) >>> 0) >>> 24;
    return concat(new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", new Uint8Array(0)));
  };
  const w = new World();
  const mal = w.user();
  const CAP = 60_000;
  for (const metric of ["storage_bytes", "removed_bytes"]) w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, ?, ?, 0, 'x')`).run(mal.id, metric, CAP);
  for (let r = 0; r < 12; r++) {
    const ups = [];
    for (let k = 0; k < 4; k++) ups.push(await upload(w, mal, noisy(r * 4 + k)));
    if (ups.some((u) => u.status !== 201)) break;
    const p = await publish(w, mal, bankOf(ups.map((u, i) => q(`Round ${r} picture ${i}: is this noise or a signal?`, { image: img(u.sha) }))));
    if (p.status !== 201) break;
    await w.json("DELETE", `/items/${p.body.item_id}`, { user: mal });
  }
  const r2Bytes = w.r2.keys("blobs/").reduce((a, k) => a + w.r2.objects.get(k)!.bytes.length, 0);
  assert.ok(r2Bytes <= 2 * CAP, `${r2Bytes}`);
});

test("dry-run publish (POST /items/preview): every refusal a publish gives, the student view, nothing stored, nothing about holds, parity with the publish", async () => {
  const w = new World();
  const A = w.user(), adm = w.user("admin"), S = w.user();
  const bank = [mcq("Which planet in our solar system is the largest by radius and by mass?", [["Jupiter", true], ["Mars", false], ["Venus", false], ["Mercury", false]]), q("True or false: the Sun is a star in the Milky Way galaxy.", { kind: { Tf: { correct: true } } })];
  const tables = ["items", "item_publishes", "item_questions", "item_part_keys", "stem_bands", "author_contents", "author_firsts", "admin_flags", "item_holds", "pool_audit", "question_ids"];
  const snap = () => tables.map((t) => w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM ${t}`)[0]!.n).join(",");
  const counters = () => JSON.stringify(w.q(`SELECT metric, count FROM usage_daily WHERE user_id = ? AND metric NOT IN ('previews', 'preview_units') ORDER BY metric`, A.id));
  const r2 = () => w.r2.keys("").length;
  const [s0, c0, k0] = [snap(), counters(), r2()];
  const pv = await w.json<any>("POST", "/items/preview", { user: A, body: framedOf(bank, true), contentType: "application/vnd.vershion.publish" });
  assert.equal(pv.status, 200, JSON.stringify(pv.body).slice(0, 300));
  assert.equal(pv.body.ok, true);
  assert.equal(pv.body.student_view.bank.questions.length, 2);
  assert.ok(!JSON.stringify(pv.body.student_view).includes('"correct":true'), "the student view is the stripped form");
  assert.equal("served_orders" in pv.body, false);
  assert.deepEqual([snap(), counters(), r2()], [s0, c0, k0], "nothing stored, no counter but previews");
  assert.equal(w.q<{ count: number }>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'previews'`, A.id)[0]!.count, 1);
  // A refusal has the publish's shape.
  const bad = await w.json<any>("POST", "/items/preview", { user: A, body: framedOf([mcq("What is the capital of France? ANSWER: B", [["Rome", false], ["Paris", true], ["Oslo", false]])], true), contentType: "application/vnd.vershion.publish" });
  assert.equal(bad.status, 422);
  assert.ok((bad.body.errors as Q[]).some((e) => e.code === "key_in_stem"));
  // Parity: the same body publishes, served the same (up to the shuffle).
  const p = await publish(w, A, bankOf(bank), { hideKey: true });
  assert.equal(p.status, 201);
  const g = await get(w, w.user(), p.body.item_id as string);
  assert.ok(sameServed(pv.body.student_view, g.body.payload, []), "same served form");
  // No hold oracle: a blocked text previews exactly like a fresh one.
  const PII = "Maria Lopez (period 3, IEP for ADHD) scored 41 on the unit test: what is her z-score?";
  const v = (await publish(w, S, bankOf([q(PII)]))).body;
  await w.purge(adm, v.item_id as string, { questions: [0] });
  const shape = (r: { status: number; body: Q }) => [r.status, Object.keys(r.body).sort().join(",")];
  const blocked = await w.json<any>("POST", "/items/preview", { user: A, body: framedOf([q(PII)], false), contentType: "application/vnd.vershion.publish" });
  const fresh = await w.json<any>("POST", "/items/preview", { user: A, body: framedOf([q("A fresh question with nobody's data in it at all?")], false), contentType: "application/vnd.vershion.publish" });
  assert.deepEqual(shape(blocked), shape(fresh));
  assert.ok(!JSON.stringify(blocked.body).match(/hold|held|block|privacy/i), JSON.stringify(blocked.body).slice(0, 200));
});

test("key_leak reports: one reporter flags the admin (with the served text) and tells the author; two hold the hidden-key item for review", async () => {
  const w = new World();
  const A = w.user(), B = w.user(), C = w.user();
  const item = (await publish(w, A, bankOf([mcq("Which planet is largest? (hint: it starts with J)", [["Jupiter", true], ["Mars", false], ["Venus", false]])]), { hideKey: true })).body;
  for (const u of [B, C]) {
    w.history(u);
    await get(w, u, item.item_id as string); // (round 18: a qualifying reporter was served it a day before)
  }
  w.advance(DAY);
  assert.equal((await w.json("POST", `/items/${item.item_id}/report`, { user: B, json: { reason: "key_leak", note: "The stem gives the answer away." } })).status, 202);
  const f = w.q<{ detail: string }>(`SELECT detail FROM admin_flags WHERE kind = 'key_leak' AND item_id = ?`, item.item_id);
  assert.equal(f.length, 1);
  assert.match(JSON.parse(f[0]!.detail).served, /Which planet is largest/);
  assert.deepEqual(holds(w, item.item_id), [], "one reporter never holds");
  assert.ok(w.notices.some((n) => n.kind === "key_leak_report"));
  await w.json("POST", `/items/${item.item_id}/report`, { user: C, json: { reason: "key_leak" } });
  assert.deepEqual(holds(w, item.item_id), [{ reason: "reports", source: "key_leak" }]);
});

function framedOf(qs: Q[], hide: boolean): Uint8Array {
  const payload = bankOf(qs);
  const meta = { publish_id: publishId(), hide_key: hide, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: "2026-10-22" }, labels: hide ? decideLabels(payload) : [], order_acks: [], image_ok: [], visibility: "public", meta: { title: "Preview", subject: "Biology", course: "", grade: "", tags: [], standards: [] } };
  return framed(meta, payload);
}
