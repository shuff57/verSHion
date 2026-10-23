// Round 12: critics 16a (Moodle) / 16b (ASVS) inverted. M1/M2 "first" and "common" by content, M3
// the rightful item's serves, M4 field and normalised block keys with a preview and a per-item
// release, M5 independence never covers an exact copy, M6 normalised blocks and near flags, A1 the
// public publish is never retired, and the non-blocking items.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { poolNotifier } from "../../src/poolmail.ts";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { publish, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const LIC = ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"];
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const served = (r: any): Q[] => r.body.payload.bank.questions;
const withOrigin = (r: any) =>
  served(r).map((x: Q, i: number) => ({ ...x, origin: { item_id: r.body.item.id, qid: r.body.questions[i].qid, publish_id: r.body.item.publish_id, version: r.body.item.version, author_name: r.body.item.author_name, license: r.body.license, form: r.body.item.form } }));
const own = (pub: any, qs: Q[]) => qs.map((x, i) => (i < pub.qids.length ? { ...x, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } } : x));
const out = (r: { status: number; body: Q }) => (r.status === 422 && r.body.error === "license_incompatible" ? `422${JSON.stringify(r.body.allowed)}` : String(r.status));
const holds = (w: World, id: unknown) => w.q<{ reason: string; source: string }>(`SELECT reason, source FROM item_holds WHERE item_id = ? ORDER BY reason, source`, id);
const flags = (w: World, id: unknown) => w.q<{ kind: string }>(`SELECT kind FROM admin_flags WHERE item_id = ? ORDER BY kind`, id).map((x) => x.kind);
const curOf = (w: World, id: unknown) => w.q<{ current_publish_id: string }>(`SELECT current_publish_id FROM items WHERE id = ?`, id)[0]!.current_publish_id;
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const NCSA = `422${JSON.stringify(["CC-BY-NC-SA-4.0"])}`;

// ---- M1 / M2: "first" and "common" by content ------------------------------------------------------------

const annMc = () =>
  mcq(
    "What is photosynthesis?",
    [
      ["Ann's distinctive choice: chloroplast thylakoid light capture splitting water into oxygen", true],
      ["Ann's distractor: mitochondrial cristae oxidising pyruvate in the matrix", false],
      ["Ann's distractor: ribosomal translation of chlorophyll-binding proteins", false],
    ],
    // (At least 25 words: copying it alone is a lift, round 16 B6.)
    { explanation: "Ann's own long explanation: light reactions in the thylakoid membranes produce ATP and NADPH from water, and the Calvin cycle in the stroma then fixes CO2 into sugar using them." },
  );

test("M1 (b1, b3b): a planted stem makes nobody 'first' for a richer later question; copies of it, changed or hidden, still bind", async () => {
  for (const [plantKind, how] of [["Short", "import"], ["Short", "read"], ["Mc-same-stem", "import"]] as const) {
    const w = new World();
    const Mal = w.user("teacher", "Mal"), Ann = w.user("teacher", "Ann");
    const plant = plantKind === "Short" ? q("What is photosynthesis?", { kind: { Short: { sample: "a process" } } }) : mcq("What is photosynthesis?", [["Mal's own choice", true], ["Mal's other", false]]);
    assert.equal((await publish(w, Mal, bankOf([plant]), { license: "CC0-1.0", visibility: "unlisted" })).status, 201);
    w.advance(DAY);
    const a = (await publish(w, Ann, bankOf([annMc()]), { license: "CC-BY-NC-SA-4.0" })).body;
    w.advance(DAY);
    const s0 = served(how === "import" ? await imp(w, Mal, a.item_id as string) : await get(w, Mal, a.item_id as string))[0]!;
    const tag = `${plantKind}/${how}`;
    assert.equal(out(await publish(w, Mal, bankOf([s0]), { license: "CC0-1.0" })), NCSA, `${tag}: verbatim`);
    assert.equal(out(await publish(w, Mal, bankOf([{ ...s0, points: 2 }]), { license: "CC0-1.0" })), NCSA, `${tag}: points changed`);
    assert.equal(out(await publish(w, Mal, bankOf([s0]), { license: "CC0-1.0", hideKey: true })), NCSA, `${tag}: hidden key`);
    if (how === "import") assert.equal(out(await publish(w, Mal, bankOf([{ ...s0, explanation: `${s0.explanation} Mal adds one sentence.` }]), { license: "CC0-1.0" })), NCSA, `${tag}: an adaptation of an import`);
  }
  // b3b: the planter's own earlier Short with the same stem and ANY answer.
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
  const STEM = "Define osmosis in your own words.";
  const annQ = q(STEM, { kind: { Short: { sample: "Ann's model answer: net movement of water across a selectively permeable membrane down its water potential gradient." } }, explanation: "Ann's potato cylinder example." });
  await publish(w, M, bankOf([q(STEM, { kind: { Short: { sample: "x" } } })]), { license: "CC0-1.0", visibility: "unlisted" });
  w.advance(DAY);
  const a = (await publish(w, A, bankOf([annQ]), { license: "CC-BY-NC-SA-4.0" })).body;
  w.advance(DAY);
  const s = served(await imp(w, M, a.item_id as string));
  assert.equal(out(await publish(w, M, bankOf(s), { license: "CC0-1.0" })), NCSA);
  assert.equal(out(await publish(w, M, bankOf([{ ...s[0], points: 5 }]), { license: "CC0-1.0" })), NCSA);
  // The planter had Ann's stem AND choices first (a common Mc), Ann added her explanation: copying it,
  // even with the points changed, binds (the exception for additions, round 12: a licence-sense copy).
  {
    const w3 = new World();
    const M3 = w3.user("teacher", "Mal"), A3 = w3.user("teacher", "Ann");
    const { explanation: _e, ...bare } = annMc();
    await publish(w3, M3, bankOf([{ ...bare, explanation: null }]), { license: "CC0-1.0", visibility: "unlisted" });
    w3.advance(DAY);
    const a3 = (await publish(w3, A3, bankOf([annMc()]), { license: "CC-BY-NC-SA-4.0" })).body;
    w3.advance(DAY);
    const s3 = served(await get(w3, M3, a3.item_id as string))[0]!;
    assert.equal(out(await publish(w3, M3, bankOf([{ ...s3, points: 4 }]), { license: "CC0-1.0" })), NCSA, "Ann's explanation, points changed");
  }
  // A planted stem-only qid turned into Ann's rich question (reworded) starts a new lineage: Ann binds.
  {
    const w4 = new World();
    const M4 = w4.user("teacher", "Mal"), A4 = w4.user("teacher", "Ann");
    const STEM4 = "Explain in your own words how osmosis moves water across a membrane between cells";
    const plant = (await publish(w4, M4, bankOf([q(STEM4, { kind: { Short: { sample: "x" } } })]), { license: "CC0-1.0", visibility: "unlisted" })).body;
    w4.advance(DAY);
    const rich = q(STEM4, { kind: { Short: { sample: "Ann's long model answer about water potential and selectively permeable membranes." } }, explanation: "Ann's potato example." });
    const a4 = (await publish(w4, A4, bankOf([rich]), { license: "CC-BY-NC-SA-4.0" })).body;
    w4.advance(DAY);
    const s4 = served(await imp(w4, M4, a4.item_id as string))[0]!;
    const reworded = { ...s4, stem: "Explain in your own words how osmosis moves water across a membrane between two cells", published: { item_id: plant.item_id, qid: (plant.qids as string[])[0], publish_id: plant.publish_id, version: plant.version } };
    const r4 = await publish(w4, M4, bankOf([reworded]), { license: "CC0-1.0", base: plant.publish_id as string }, plant.item_id as string);
    assert.equal(out(r4), NCSA, "the plant's lineage does not cover Ann's added material");
  }
  // Inverted: the author's own SAME content published first stays theirs (the original author).
  const w2 = new World();
  const V = w2.user("teacher", "Vera"), X = w2.user("teacher", "Xan");
  const v = (await publish(w2, V, bankOf([annMc()]), { license: "CC-BY-4.0" })).body;
  w2.advance(DAY);
  const x = (await publish(w2, X, bankOf(served(await get(w2, X, v.item_id as string))), { license: "CC-BY-4.0" })).body;
  await get(w2, V, x.item_id as string);
  w2.advance(DAY);
  assert.equal((await publish(w2, V, bankOf([{ ...annMc(), points: 3 }]), { license: "CC-BY-NC-SA-4.0" })).status, 201, "Vera's own question, published first, never binds her");
});

test("M2 (b3): three older teachers with their OWN answers to the same stem never make Ann's model answer common", async () => {
  const STEM = "Define osmosis in your own words.";
  const annQ = q(STEM, { kind: { Short: { sample: "Ann's model answer: osmosis is the net movement of water across a selectively permeable membrane from low to high solute concentration." } }, explanation: "Ann's distinctive worked example with a potato cylinder." });
  const w = new World();
  const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
  for (let k = 0; k < 3; k++) {
    await publish(w, w.user("teacher", `Older${k}`), bankOf([q(STEM, { kind: { Short: { sample: `Older${k}'s own short answer ${k}` } } })]), { license: "CC0-1.0" });
    w.advance(DAY);
  }
  const a = (await publish(w, A, bankOf([annQ]), { license: "CC-BY-NC-SA-4.0" })).body;
  w.advance(DAY);
  assert.equal(out(await publish(w, M, bankOf(served(await get(w, M, a.item_id as string))), { license: "CC0-1.0" })), NCSA);
  // Inverted: three older teachers with Ann's SAME content (before her) make it common.
  const w2 = new World();
  const A2 = w2.user("teacher", "Ann"), M2 = w2.user("teacher", "Mal");
  for (let k = 0; k < 3; k++) {
    await publish(w2, w2.user("teacher", `Older${k}`), bankOf([annQ]), { license: "CC0-1.0" });
    w2.advance(DAY);
  }
  const a2 = (await publish(w2, A2, bankOf([annQ]), { license: "CC-BY-NC-SA-4.0" })).body;
  w2.advance(DAY);
  await get(w2, M2, a2.item_id as string);
  assert.equal((await publish(w2, M2, bankOf([annQ]), { license: "CC0-1.0" })).status, 201);
});

// ---- M3 ---------------------------------------------------------------------------------------------------

test("M3 (b5): the victim opening the infringing copy later never disqualifies her item as rightful; a serve before her first publish does", async () => {
  for (const looks of ["never", "get", "import"]) {
    const w = new World();
    const adm = w.user("admin"), V = w.user("teacher", "Victim"), X = w.user("teacher", "Infringer");
    const vq = q("Victim's original question about the photosynthetic electron transport chain in plants?", { explanation: "Victim's long original explanation." });
    const v = (await publish(w, V, bankOf([vq]), { license: "CC-BY-NC-4.0" })).body;
    w.advance(DAY);
    const x = (await publish(w, X, bankOf([vq]), { license: "CC0-1.0" })).body; // copied from outside the pool
    w.advance(DAY);
    if (looks === "get") await get(w, V, x.item_id as string);
    if (looks === "import") await imp(w, V, x.item_id as string);
    const td = await w.takedown<any>(adm, await notice(w, adm), x.item_id, { rightful_item_id: v.item_id, questions: [0] });
    assert.equal(td.status, 200, `${looks}: ${JSON.stringify(td.body)}`);
  }
  // A serve of X BEFORE R's first publish: R is derived from X (even though R had the content first).
  const w = new World();
  const adm = w.user("admin"), R = w.user("teacher", "Rae"), X = w.user("teacher", "Xi");
  const xq = q("Xi's unrelated first question about rocks?");
  const x = (await publish(w, X, bankOf([xq]))).body;
  w.advance(DAY);
  await get(w, R, x.item_id as string); // Rae saw Xi's item before publishing her own
  w.advance(DAY);
  const shared = q("The question Xi later adds about the nitrogen cycle in wetlands?");
  const r = (await publish(w, R, bankOf([shared]))).body;
  w.advance(DAY);
  assert.equal((await publish(w, X, bankOf(own(x, [xq, shared])), { base: x.publish_id as string }, x.item_id as string)).status, 200);
  const pv = await w.admin<any>(adm, "POST", "/admin/dmca/takedown-preview", { item_id: x.item_id, rightful_item_id: r.item_id, questions: [1] });
  assert.deepEqual(pv.body.rightful_check.reasons, ["rightful_derived_from_item"]);
});

// ---- M4 ---------------------------------------------------------------------------------------------------

test("M4 (b6): a privacy block of a generic-stem Short covers its content by default; a stem-wide block is shown with who it hits; an admin releases one item", async () => {
  const STEM = "Explain your reasoning for the previous answer.";
  const w = new World();
  const adm = w.user("admin"), A = w.user("teacher", "Ann"), I = w.user("teacher", "Innocent"), J = w.user("teacher", "Later"), R = w.user();
  const i0 = (await publish(w, I, bankOf([q(STEM, { kind: { Short: { sample: "Because the slope is constant." } }, explanation: "Innocent's own explanation." }), q("Innocent's other question about lines?")]))).body;
  w.advance(DAY);
  const a = (await publish(w, A, bankOf([q(STEM, { kind: { Short: { sample: "See rubric." } }, explanation: "Jenny Smith (ID 40211, period 3) wrote 42 here." }), q("Ann's other question about volcanoes?")]))).body;
  w.advance(DAY);
  // Round 13 (critic 19b): a privacy block's default covers the STEM (student data may be in it) and
  // the content; the preview says it is stem-wide and lists who it would hold.
  const pv = await w.admin<any>(adm, "POST", "/admin/pool/purge-preview", { item_id: a.item_id, questions: [0] });
  assert.equal(pv.body.match[0].scope, "default");
  assert.equal(pv.body.match[0].stem_wide, true);
  assert.deepEqual(pv.body.match[0].affects.map((x: Q) => x.item_id), [i0.item_id]);
  // The admin may choose the explanation alone (the data is only there), or the stem.
  const ex = await w.admin<any>(adm, "POST", "/admin/pool/purge-preview", { item_id: a.item_id, questions: [0], scopes: { 0: "explanation" } });
  assert.match(ex.body.match[0].keys[0], /^e:/);
  assert.equal(ex.body.match[0].stem_wide, false);
  assert.deepEqual(ex.body.affected, []);
  assert.equal((await w.purge(adm, a.item_id as string, { questions: [0], scopes: { 0: "explanation" } })).status, 200);
  assert.deepEqual(holds(w, i0.item_id), [], "narrowed: the innocent teacher is not held");
  assert.equal((await get(w, R, i0.item_id as string)).status, 200);
  const j = await publish(w, J, bankOf([q(STEM, { kind: { Short: { sample: "My own sample." } }, explanation: "Later teacher's own words." })]));
  assert.deepEqual(holds(w, j.body.item_id), [], "nor a later one with the same stem");
  // The same explanation elsewhere IS held.
  const k = await publish(w, J, bankOf([q("A different stem?", { kind: { Short: { sample: "See rubric." } }, explanation: "jenny smith (id 40211 period 3) wrote 42 here" })]));
  assert.deepEqual(holds(w, k.body.item_id), [{ reason: "student_data", source: "privacy" }]);
  // A generic stem (3+ other authors' visible items): the preview warns and suggests the narrower scope.
  const wg = new World();
  const admg = wg.user("admin"), Ag = wg.user();
  for (let n = 0; n < 3; n++) await publish(wg, wg.user(), bankOf([q(STEM, { kind: { Short: { sample: `Sample ${n}.` } } })]));
  const ag = (await publish(wg, Ag, bankOf([q(STEM, { kind: { Short: { sample: "See rubric." } }, explanation: "Jenny Smith wrote 42 here." })]))).body;
  const pg = await wg.admin<any>(admg, "POST", "/admin/pool/purge-preview", { item_id: ag.item_id, questions: [0] });
  assert.deepEqual([pg.body.match[0].generic_stem, pg.body.match[0].suggest_scope, pg.body.match[0].suggested], [true, "explanation", false]);
  // A stem-wide block hits the innocent item; the admin releases it, with a reason, for good.
  const w2 = new World();
  const adm2 = w2.user("admin"), A2 = w2.user(), I2 = w2.user();
  const inn = (await publish(w2, I2, bankOf([q(STEM, { kind: { Short: { sample: "Innocent sample." } } })]))).body;
  const a2 = (await publish(w2, A2, bankOf([q(STEM, { kind: { Short: { sample: "Jenny Smith scored 42." } } })]))).body;
  assert.equal((await w2.purge(adm2, a2.item_id as string, { questions: [0] })).status, 200);
  assert.deepEqual(holds(w2, inn.item_id), [{ reason: "student_data", source: "privacy" }]);
  assert.equal((await w2.admin(adm2, "POST", "/admin/pool/release-hold", { item_id: inn.item_id, source: "privacy" })).status, 400, "a reason is required");
  const rel = await w2.admin<any>(adm2, "POST", "/admin/pool/release-hold", { item_id: inn.item_id, publish_id: inn.publish_id, source: "privacy", note: "Generic stem, no student data in this item." });
  assert.equal(rel.status, 200);
  assert.deepEqual(holds(w2, inn.item_id), []);
  const rp = await publish(w2, I2, bankOf(own(inn, [q(STEM, { kind: { Short: { sample: "Innocent sample, edited." } } })])), { base: inn.publish_id as string }, inn.item_id as string);
  assert.equal(rp.status, 200);
  assert.deepEqual(holds(w2, inn.item_id), [], "its republish is not held again by the released keys");
  assert.equal(w2.q(`SELECT * FROM pool_audit WHERE action = 'item.release_hold'`).length, 1);
});

// ---- M5 ---------------------------------------------------------------------------------------------------

test("M5 (b7): independent_of never covers an exact copy of what the account was served, read or imported; a changed copy is flagged", async () => {
  for (const how of ["import", "read"]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal");
    // (More than 40 words served: distinctive, round 14 B5.)
    const Q0 = mcq("In Ann's field study of the Sacramento River delta, which factor best explains the 2019 decline of juvenile Chinook salmon?", [["Elevated summer water temperature", true], ["Predation near the pumps", false], ["Reduced turbidity", false]], { explanation: "Ann's own analysis of the temperature logger data from 2017 to 2019 at the Freeport trap." });
    const a = (await publish(w, A, bankOf([Q0]), { license: "CC-BY-NC-SA-4.0" })).body;
    w.advance(DAY);
    const r = how === "import" ? await imp(w, M, a.item_id as string) : await get(w, M, a.item_id as string);
    const p = await publish(w, M, bankOf(served(r)), { license: "CC0-1.0", independent: [a.item_id as string], statement: "I wrote these questions myself." });
    assert.equal(out(p), NCSA, `${how}: refused`);
    const changed = await publish(w, M, bankOf([{ ...served(r)[0], stem: "Which factor best explains the decline of juvenile salmon counts in the river delta in 2019, in Mal's view?", explanation: "Mal's own analysis." }]), { license: "CC0-1.0", independent: [a.item_id as string], statement: "I wrote these questions myself." });
    assert.equal(changed.status, 201, `${how}: a changed copy may be attested`);
    assert.ok(flags(w, changed.body.item_id).includes("independent_claim"));
  }
  // Round 14 (B5): a SHORT question merely READ can be attested independent: credited as a possible
  // source, badge claimed_independent; imported, it stays refused.
  for (const how of ["read", "import"]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), R = w.user();
    // (Not a "short statement", whose reads are no evidence at all, but 40 words or fewer.)
    const G = mcq("Which gas do green plants take in from the air for photosynthesis during the daylight hours?", [["Carbon dioxide", true], ["Oxygen", false], ["Nitrogen", false]]);
    const a = (await publish(w, A, bankOf([G]), { license: "CC-BY-NC-SA-4.0" })).body;
    w.advance(DAY);
    const r = how === "import" ? await imp(w, M, a.item_id as string) : await get(w, M, a.item_id as string);
    const p = await publish(w, M, bankOf(served(r)), { license: "CC0-1.0", independent: [a.item_id as string], statement: "I wrote this obvious question myself." });
    if (how === "import") {
      assert.equal(out(p), NCSA);
      continue;
    }
    assert.equal(p.status, 201);
    const g = await get(w, R, p.body.item_id as string);
    assert.equal(g.body.license_check.status, "claimed_independent");
    assert.deepEqual(g.body.questions[0].chain.map((l: Q) => [l.author_name, l.possible_source]), [["Ann", true]]);
    assert.ok(flags(w, p.body.item_id).includes("independent_claim"));
  }
});

// ---- M6 ---------------------------------------------------------------------------------------------------

test("M6 (b8): privacy and copyright blocks compare normalised text; a near-rewording of student data is held and flagged", async () => {
  const SD = "Jenny Smith (ID 40211) in period 3 scored 42 on the unit test; explain her error";
  for (const [name, edit] of [["identical", (s: string) => s], ["period", (s: string) => `${s}.`], ["case", (s: string) => s.toUpperCase()], ["comma", (s: string) => s.replace(";", ",")], ["quotes", (s: string) => s.replace("unit test", "“unit test”")]] as const) {
    const w = new World();
    const adm = w.user("admin"), A = w.user("teacher", "Ann"), C = w.user("teacher", "Cal"), F = w.user("teacher", "Fresh");
    const a = (await publish(w, A, bankOf([q(SD), q("Ann's other question about volcanoes?")]))).body;
    w.advance(DAY);
    const c = (await publish(w, C, bankOf([served(await imp(w, C, a.item_id as string))[0]!, q("Cal's own question about lakes?")]))).body;
    w.advance(DAY);
    assert.equal((await w.purge(adm, a.item_id as string, { questions: [0] })).status, 200);
    await publish(w, C, bankOf([q(edit(SD)), q("Cal's own question about lakes?")]), { base: curOf(w, c.item_id) }, c.item_id as string);
    assert.deepEqual(holds(w, c.item_id), [{ reason: "student_data", source: "privacy" }], `${name}: the held copy stays held`);
    const f = await publish(w, F, bankOf([q(edit(SD))]));
    assert.deepEqual(holds(w, f.body.item_id), [{ reason: "student_data", source: "privacy" }], `${name}: a fresh publish is held`);
    assert.ok(flags(w, f.body.item_id).includes("privacy_text_match"));
  }
  // A near-rewording (one word): HELD (round 13), and the admin is told.
  const w = new World();
  const adm = w.user("admin"), A = w.user(), F = w.user();
  const a = (await publish(w, A, bankOf([q(SD)]))).body;
  await w.purge(adm, a.item_id as string, { questions: [0] });
  const f = await publish(w, F, bankOf([q(SD.replace("42", "43"))]));
  assert.deepEqual(holds(w, f.body.item_id), [{ reason: "student_data", source: "privacy_near" }]);
  const fl = w.q<{ detail: string }>(`SELECT detail FROM admin_flags WHERE kind = 'privacy_text_match' AND item_id = ?`, f.body.item_id);
  assert.equal(fl.length, 1);
  assert.deepEqual(JSON.parse(fl[0]!.detail).near.first, [0]);
  // Copyright: a re-upload with the case changed is held too.
  const w3 = new World();
  const adm3 = w3.user("admin"), X = w3.user(), C3 = w3.user();
  const T = "Copyrighted textbook question 7.3 about the rate of change of a population?";
  const x = (await publish(w3, X, bankOf([q(T)]))).body;
  assert.equal((await w3.takedown(adm3, await notice(w3, adm3), x.item_id, { questions: [0] })).status, 200);
  assert.equal(holds(w3, (await publish(w3, C3, bankOf([q(T.toLowerCase())]))).body.item_id)[0]?.reason, "blocklist");
});

// ---- A1 ---------------------------------------------------------------------------------------------------

test("A1 (e1, e3): hidden-only republishes never retire the public publish: search, read and holds keep working, for every kind", async () => {
  const kinds: [string, Q, Q][] = [
    ["Mc", mcq("Which zebrafish organ regenerates after injury?", [["Heart", true], ["Liver", false], ["Brain", false]]), mcq("Which zebrafish organ regenerates after injury?", [["Heart", false], ["Liver", true], ["Brain", false]])],
    ["Tf", q("Zebrafish hearts regenerate after injury.", { kind: { Tf: { correct: true } } }), q("Zebrafish hearts regenerate after injury.", { kind: { Tf: { correct: false } } })],
    ["Short", q("Explain how zebrafish regenerate heart tissue.", { kind: { Short: { sample: "cardiomyocyte proliferation" } } }), q("Explain how zebrafish regenerate heart tissue.", { kind: { Short: { sample: "scar-free healing" } } })],
    ["Blank", q("Zebrafish regenerate the ___ after injury.", { kind: { Blank: { accept: [["heart"]] } } }), q("Zebrafish regenerate the ___ after injury.", { kind: { Blank: { accept: [["fin"]] } } })],
    [
      "Matching",
      q("Match each zebrafish organ to its regeneration time.", { kind: { Matching: { pairs: [{ left: "Heart", right: "Two months" }, { left: "Fin", right: "Two weeks" }], distractors: [] } } }),
      q("Match each zebrafish organ to its regeneration time.", { kind: { Matching: { pairs: [{ left: "Heart", right: "Two weeks" }, { left: "Fin", right: "Two months" }], distractors: [] } } }),
    ],
  ];
  for (const [name, k1, k2] of kinds) {
    const w = new World();
    const A = w.user(), B = w.user();
    let p: any = (await publish(w, A, bankOf([k1]), { hideKey: true })).body;
    const first = p.publish_id;
    for (const k of [k2, k1, k2]) {
      w.advance(60_000);
      const r = await publish(w, A, bankOf(own(p, [k])), { hideKey: true, base: p.publish_id }, p.item_id);
      assert.equal(r.status, 200, name);
      p = { ...p, ...r.body };
    }
    assert.equal(w.q<{ public_publish_id: string }>(`SELECT public_publish_id FROM items WHERE id = ?`, p.item_id)[0]!.public_publish_id, first, `${name}: nothing served changed`);
    assert.equal(w.q(`SELECT * FROM item_publishes WHERE publish_id = ?`, first).length, 1, `${name}: the public publish is kept`);
    const hits = (await w.json<any>("GET", "/items?q=zebrafish", { user: B })).body.items.map((x: Q) => x.id);
    assert.deepEqual(hits, [p.item_id], `${name}: still found`);
    assert.equal((await get(w, B, p.item_id)).status, 200);
  }
});

// ---- non-blocking ---------------------------------------------------------------------------------------

test("versions (e4): a held item says only 'held' to everyone, the author included (no privacy-vs-copyright oracle)", async () => {
  const w = new World();
  const adm = w.user("admin"), V = w.user(), A = w.user("teacher", "Att"), B = w.user();
  const S = "Maria Lopez in period 3 (IEP, ADHD) scored 41 on the unit test: what is her z-score?";
  const v = (await publish(w, V, bankOf([q(S)]))).body;
  await w.purge(adm, v.item_id as string, { questions: [0] });
  const T = "Copyrighted textbook question 7.3 about the rate of change of a population?";
  const t = (await publish(w, V, bankOf([q(T)]))).body;
  await w.takedown(adm, await notice(w, adm), t.item_id, { questions: [0] });
  const seen: string[] = [];
  for (const guess of [S, T]) {
    const p = (await publish(w, A, bankOf([q("An innocuous question about volcanoes?")]))).body;
    await get(w, B, p.item_id as string);
    await publish(w, A, bankOf(own(p, [q(guess)])), { base: p.publish_id as string }, p.item_id as string);
    for (const u of [A, B]) {
      const it = (await w.json<any>("POST", "/items/versions", { user: u, json: { ids: [p.item_id] } })).body.items[0];
      seen.push(`${it.status}/${it.reason}`);
    }
  }
  assert.deepEqual(seen, ["under_review/held", "under_review/held", "under_review/held", "under_review/held"]);
});

test("a deleted account's name is never stored again after the deletion (reads and publishes during the rewrite)", async () => {
  const w = new World({ rewriteBatch: 2, rewriteBatches: 1 });
  const D = w.user("teacher", "Dana Privatesurname");
  const d = (await publish(w, D, bankOf([q("Dana's question about basalt?")]))).body;
  const copies: string[] = [];
  for (let k = 0; k < 3; k++) {
    const u = w.user("teacher", `C${k}`);
    copies.push((await publish(w, u, bankOf(withOrigin(await imp(w, u, d.item_id as string))))).body.item_id as string);
  }
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, pw_params = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, D.id);
  await w.db.batch(deleteContentStatements(w.db, D.id, w.clock));
  // Before any cron run: a newcomer reads and publishes a copy.
  const N = w.user("teacher", "Newcomer");
  const np = await publish(w, N, bankOf(withOrigin(await imp(w, N, copies[0]!))));
  assert.equal(np.status, 201);
  const named = (t: string) => w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM ${t} WHERE instr(chain_json, 'Privatesurname') > 0`)[0]!.n;
  // (Round 19: evidence is read through the sources' own rows; nothing a read stores names her.)
  assert.equal(named("ghost_questions"), 0, "no stored evidence names her");
  assert.equal(w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM item_questions WHERE publish_id = ? AND instr(chain_json, 'Privatesurname') > 0`, np.body.publish_id)[0]!.n, 0, "nor its stored chain");
  for (let k = 0; k < 20; k++) await w.pool.hourly();
  assert.equal(named("item_questions") + named("ghost_questions"), 0);
});

test("alerts: one hourly budget per notice kind; the student-data match mail says whether the item is held", async () => {
  const w = new World();
  const adm = w.user("admin"), A = w.user(), F = w.user();
  const SD = "Jenny Smith (ID 40211) in period 3 scored 42 on the unit test; explain her error";
  const a = (await publish(w, A, bankOf([q(SD)]))).body;
  await w.purge(adm, a.item_id as string, { questions: [0] });
  await publish(w, F, bankOf([q(SD)]));
  await publish(w, F, bankOf([q(SD.replace("42", "43"))]));
  const pm = w.notices.filter((n) => n.kind === "privacy_text_match") as { held: boolean }[];
  assert.deepEqual(pm.map((n) => n.held), [true, false]);
  // The mail text (no item text in it).
  const sent: string[] = [];
  const env = { DB: { prepare: () => ({ bind: () => ({ first: async () => null, all: async () => ({ results: [] }) }) }) } } as never;
  const orig = globalThis.fetch;
  globalThis.fetch = (async (_u: unknown, init?: { body?: string }) => {
    sent.push(String(init?.body ?? ""));
    return new Response("{}", { status: 200 });
  }) as typeof fetch;
  try {
    const notify = poolNotifier({ ...(env as object), RESEND_API_KEY: "k", MAIL_FROM: "a@x.org", ADMIN_EMAIL: "admin@x.org", MAIL_MODE: "send" } as never);
    await notify({ kind: "privacy_text_match", itemId: "i1", held: true }).catch(() => {});
    await notify({ kind: "privacy_text_match", itemId: "i2", held: false }).catch(() => {});
  } finally {
    globalThis.fetch = orig;
  }
  const all = sent.join("\n");
  if (sent.length > 0) {
    assert.match(all, /held for review/);
    assert.match(all, /It is live/);
    assert.ok(!all.includes("Jenny"), "no item text in a mail");
  }
  // Separate budgets: five cron failures do not use up the cap alerts' budget.
  const w2 = new World();
  const { alertBudgeted } = await import("../../src/pool/shared.ts");
  const c = { db: w2.db, now: () => w2.clock, deps: { notify: async (n: unknown) => void w2.notices.push(n as never) } } as never;
  for (let k = 0; k < 6; k++) await alertBudgeted(c, { kind: "cron_failure", stage: `s${k}` });
  await alertBudgeted(c, { kind: "cap", userId: "u", metric: "publishes" });
  await new Promise((r) => setTimeout(r, 10));
  assert.equal(w2.notices.filter((n) => n.kind === "cron_failure").length, 5);
  assert.equal(w2.notices.filter((n) => n.kind === "cap").length, 1, "its own budget");
});

test("share-core round 6: ack_formatting reaches share-core; formatting refusals carry the author's preview, author-only codes", async () => {
  const w = new World();
  const A = w.user(), R = w.user();
  const mk = () => mcq("Which city is the capital of France?", [["PARIS", true], ["Berlin", false], ["Madrid", false]]);
  const r1 = await publish(w, A, bankOf([mk()]), { hideKey: true });
  assert.equal(r1.status, 422);
  const e = (r1.body.errors as Q[])[0]!;
  assert.deepEqual([e.code, e.reason, e.ack_allowed, e.author_only], ["key_in_formatting", "capitals", true, true]);
  assert.ok(Array.isArray(r1.body.formatting_findings) && Array.isArray(r1.body.order_preview), "the author's preview comes with the refusal");
  // Acknowledged: published.
  const meta = { title: "Unit test", subject: "Biology", course: "", grade: "", tags: [], standards: [] };
  const { framed, publishId, decideLabels } = await import("./harness.ts");
  const payload = bankOf([mk()]);
  const ok = await w.json<any>("POST", "/items", {
    user: A,
    body: framed({ publish_id: publishId(), hide_key: true, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: "2026-10-22" }, labels: decideLabels(payload), order_acks: [], image_ok: [], ack_formatting: [0], visibility: "public", meta }, payload),
    contentType: "application/vnd.vershion.publish",
  });
  let res = ok;
  if (res.status === 422 && res.body.error === "order_ack_required") {
    res = await w.json<any>("POST", "/items", {
      user: A,
      body: framed({ publish_id: publishId(), hide_key: true, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, attest: { own_work: true, terms_version: "2026-10-22" }, labels: decideLabels(payload), order_acks: res.body.needs_order_ack, image_ok: [], ack_formatting: [0], visibility: "public", meta }, payload),
      contentType: "application/vnd.vershion.publish",
    });
  }
  assert.equal(res.status, 201, JSON.stringify(res.body).slice(0, 300));
  // A client-sent origin is never served (drop_client_provenance).
  const withOrig = { ...q("A question with a forged origin?"), origin: { item_id: "f".repeat(32), qid: "q", publish_id: "p", version: 1, author_name: "Forged", license: "CC0-1.0", form: "full" } };
  const h = (await publish(w, A, bankOf([withOrig]), { hideKey: true })).body;
  assert.equal(served(await get(w, R, h.item_id as string))[0].origin, undefined);
});

test("migration: a database at the committed 0102 with rows migrates through 0103 and 0104, and the cron backfill completes it", async () => {
  // Build a pool with the full schema, then copy every row (the columns 0102 had) into a 0102 database.
  const src = new World();
  const A = src.user("teacher", "Ann"), B = src.user("teacher", "Bob");
  const a = (await publish(src, A, bankOf([q("Über die Photosynthese: what does the zebrafish heart do?", { explanation: "Ann's explanation." })]), { meta: { title: "Ökologie Unit", subject: "Biología", course: "", grade: "", tags: ["Größe"], standards: [] } })).body;
  src.advance(DAY);
  await imp(src, B, a.item_id as string);
  const dir = new URL("../../migrations/", import.meta.url).pathname;
  const files = readdirSync(dir).filter((f) => f.endsWith(".sql")).sort();
  const old = new DatabaseSync(":memory:");
  for (const f of files.filter((f) => f <= "0102_pool.sql")) old.exec(readFileSync(dir + f, "utf8"));
  old.exec("PRAGMA foreign_keys = OFF");
  for (const t of old.prepare(`SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> 'd1_migrations'`).all() as { name: string }[]) {
    const cols = (old.prepare(`PRAGMA table_info(${t.name})`).all() as { name: string }[]).map((c) => c.name);
    const have = new Set((src.sql.prepare(`PRAGMA table_info(${t.name})`).all() as { name: string }[]).map((c) => c.name));
    const common = cols.filter((c) => have.has(c));
    if (common.length !== cols.length) continue; // reshaped tables (evidence): 0103 rebuilds them
    for (const r of src.sql.prepare(`SELECT ${common.join(", ")} FROM ${t.name}`).all() as Record<string, unknown>[]) old.prepare(`INSERT OR IGNORE INTO ${t.name} (${common.join(", ")}) VALUES (${common.map(() => "?").join(", ")})`).run(...(common.map((c) => r[c]) as never[]));
  }
  for (const f of files.filter((f) => f > "0102_pool.sql")) old.exec(readFileSync(dir + f, "utf8"));
  assert.ok((old.prepare(`SELECT MIN(seq) AS s FROM item_questions`).get() as { s: number }).s > 0, "sequence numbers backfilled");
  assert.equal((old.prepare(`SELECT COUNT(*) AS n FROM author_firsts`).get() as { n: number }).n, 1);
  assert.equal((old.prepare(`SELECT COUNT(*) AS n FROM served_words`).get() as { n: number }).n, 0, "search words need the bodies: the cron");
  // The migrated database under a Worker with the same bodies: the cron re-derives what needs JS.
  const w = new World({ sql: old, r2: src.r2 });
  for (const [k, v] of src.tokens) w.tokens.set(k, v);
  await w.pool.hourly();
  const it = old.prepare(`SELECT title_fold, subject_fold, fold_rev FROM items WHERE id = ?`).get(a.item_id) as Q;
  assert.equal(it.fold_rev, 1);
  assert.match(it.title_fold, /oekologie/);
  assert.equal(it.subject_fold, "biologia");
  const iq = old.prepare(`SELECT bk_norm, bk_expl, body_hash FROM item_questions`).get() as Q;
  assert.match(iq.bk_norm, /^n:/);
  assert.match(iq.bk_expl, /^e:/);
  assert.ok(iq.body_hash);
  const hits = (await w.json<any>("GET", "/items?q=zebrafish", { user: B })).body.items.map((x: Q) => x.id);
  assert.deepEqual(hits, [a.item_id], "search works for the migrated item");
  assert.deepEqual((await w.json<any>("GET", "/items?q=gr", { user: B })).body.items.map((x: Q) => x.id), [a.item_id], "the folded tag (Größe → grosse) is a word");
});

test("normalisation (share-core round 7): invisible joiners and space variants never evade a block or a licence-sense copy", async () => {
  const { normText } = await import("../../src/pool/provenance.ts");
  const base = "Jenny Smith scored 42 on the unit test";
  for (const v of ["Jenny‍ Smith scored 42 on the unit test", "Jenny Smith‌ scored 42 on the unit test", "Jen͏ny Smith scored 42 on the unit test", "Jenny Smith scored 42 on the unit test", "Jen­ny Smith scored 42 on the unit⁠ test", "﻿Jenny Smith scored 42 on the unit test"])
    assert.equal(normText(v), normText(base), JSON.stringify(v));
  const w = new World();
  const adm = w.user("admin"), A = w.user(), F = w.user();
  const a = (await publish(w, A, bankOf([q(base)]))).body;
  await w.purge(adm, a.item_id as string, { questions: [0] });
  const f = await publish(w, F, bankOf([q("Jenny‍ Smith scored 42 on the unit test")]));
  assert.deepEqual(holds(w, f.body.item_id), [{ reason: "student_data", source: "privacy" }]);
});
