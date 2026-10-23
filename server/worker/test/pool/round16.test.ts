// Round 16: critics 21c (Moodle) and 21d (ASVS) inverted. B1 an infringer is never exempt from its
// own takedown; B2 a relay is found in every served form and by a near-rewording; B3 an original
// author is never bound by a copy she was served after her own first publish; B4 report holds follow
// the normalised content and outlive a delete; B5 student data in a Multipart part is blocked as a
// standalone question too; B6 a lifted explanation or model answer binds; B7 an image is a licence
// unit; and the hardening items (raw-bytes scan, tag shortlist, PNG decode caps, release bound to
// the reviewed publish, pending accounts rate-limited). B8 (migration): round16_migrate.test.ts.
import assert from "node:assert/strict";
import { test } from "node:test";
import { checkImage } from "../../src/pool/image.ts";
import { chunk, concat, CountingStore, png, publish, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ id: "q", stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const served = (r: any): Q[] => r.body.payload.bank.questions;
const out = (r: { status: number; body: Q }) => (r.status === 422 && r.body.error === "license_incompatible" ? `422${JSON.stringify(r.body.allowed)}` : String(r.status));
const holds = (w: World, id: unknown) => w.q<{ reason: string; source: string }>(`SELECT reason, source FROM item_holds WHERE item_id = ? ORDER BY reason, source`, id);
const flags = (w: World, id: unknown) => w.q<{ kind: string }>(`SELECT kind FROM admin_flags WHERE item_id = ? ORDER BY kind`, id).map((x) => x.kind);
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const NCSA = `422${JSON.stringify(["CC-BY-NC-SA-4.0"])}`;
const ANN_Q = () =>
  mcq(
    "In Ann's field study of the delta, which factor best explains the 2019 decline of juvenile Chinook salmon at the Freeport trap?",
    [["Elevated summer water temperature above 20 C", true], ["Predation by striped bass", false], ["Reduced turbidity", false], ["Lower dissolved oxygen", false]],
    { explanation: "Ann's own analysis of the temperature logger data from 2017-2019, with the regression slope she computed." },
  );

test("B1: the takedown's own infringer re-uploads verbatim (credited to the rightful author as an earlier publication) or reordered: held", async () => {
  for (const variant of ["verbatim", "reordered"]) {
    const w = new World();
    const adm = w.user("admin"), A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), R = w.user();
    const a = (await publish(w, A, bankOf([ANN_Q()]), { license: "CC-BY-NC-4.0" })).body;
    w.advance(DAY);
    const m = (await publish(w, M, bankOf([ANN_Q()]), { license: "CC0-1.0" })).body; // an offline copy, never served
    assert.deepEqual((await get(w, R, m.item_id as string)).body.questions[0].chain.map((l: Q) => [l.author_name, l.earlier_match]), [["Ann", true]]);
    assert.equal((await w.takedown(adm, await notice(w, adm), m.item_id, { rightful_item_id: a.item_id, questions: [0] })).status, 200);
    const body = variant === "verbatim" ? ANN_Q() : { ...ANN_Q(), kind: { Mc: { answers: [...ANN_Q().kind.Mc.answers].reverse() } } };
    const r = await publish(w, M, bankOf([body]), { license: "CC0-1.0" });
    assert.equal(r.status, 201);
    assert.equal((await get(w, R, r.body.item_id as string)).status, 404, variant);
    assert.ok(holds(w, r.body.item_id).some((h) => h.reason === "blocklist"), variant);
  }
  // Each defence alone: (a) an "earlier publication" credit never exempts a third account; (b) the
  // infringer is never exempt, even with a genuine import of the rightful item.
  {
    const w = new World();
    const adm = w.user("admin"), A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), P = w.user("teacher", "Pip"), R = w.user();
    const a = (await publish(w, A, bankOf([ANN_Q()]), { license: "CC-BY-NC-4.0" })).body;
    w.advance(DAY);
    const m = (await publish(w, M, bankOf([ANN_Q()]), { license: "CC-BY-NC-4.0" })).body;
    await w.takedown(adm, await notice(w, adm), m.item_id, { rightful_item_id: a.item_id, questions: [0] });
    const p = (await publish(w, P, bankOf([ANN_Q()]), { license: "CC-BY-NC-4.0" })).body;
    assert.equal((await get(w, R, p.item_id as string)).status, 404, "(a) earlier-match credit is no exemption");
  }
  {
    const w = new World();
    const adm = w.user("admin"), A = w.user("teacher", "Ann"), M = w.user("teacher", "Mal"), R = w.user();
    const a = (await publish(w, A, bankOf([ANN_Q()]), { license: "CC-BY-NC-4.0" })).body;
    w.advance(DAY);
    await imp(w, M, a.item_id as string);
    const m = (await publish(w, M, bankOf([ANN_Q()]), { license: "CC-BY-NC-4.0", visibility: "unlisted" })).body;
    await w.takedown(adm, await notice(w, adm), m.item_id, { rightful_item_id: a.item_id, questions: [0] });
    const r = (await publish(w, M, bankOf([ANN_Q()]), { license: "CC-BY-NC-4.0" })).body;
    assert.equal((await get(w, R, r.item_id as string)).status, 404, "(b) the infringer is never exempt");
  }
});

test("B2: a relay through an account never served the original is found whatever it changes: unverified_earlier_match, flagged, binding downstream", async () => {
  const variants: [string, (s: Q) => Q, boolean][] = [
    ["verbatim, hide_key", (s) => s, true],
    ["one added sentence", (s) => ({ ...s, stem: `${s.stem} Explain your choice.` }), false],
    ["explanation dropped", (s) => ({ ...s, explanation: null }), false],
    ["one choice word changed", (s) => ({ ...s, kind: { Mc: { answers: s.kind.Mc.answers.map((x: Q, i: number) => (i === 1 ? { ...x, text: "Predation by largemouth bass" } : x)) } } }), false],
    ["a distractor more", (s) => ({ ...s, kind: { Mc: { answers: [...s.kind.Mc.answers, { text: "Lower flow in spring", correct: false, locked: false }] } } }), false],
  ];
  for (const annHidden of [false, true])
    for (const [name, change, hide] of variants) {
      const w = new World();
      const A = w.user("teacher", "Ann"), M = w.user(), P = w.user("teacher", "Pip"), T = w.user(), R = w.user();
      const a = (await publish(w, A, bankOf([ANN_Q()]), { license: "CC-BY-NC-SA-4.0", hideKey: annHidden })).body;
      w.advance(DAY);
      await imp(w, M, a.item_id as string);
      w.advance(DAY);
      const p = (await publish(w, P, bankOf([change(ANN_Q())]), { license: "CC0-1.0", hideKey: hide })).body; // Mal hands it over offline
      const tag = `annHidden=${annHidden} ${name}`;
      assert.ok(p.item_id, tag);
      assert.equal((await get(w, R, p.item_id as string)).body.license_check.status, "unverified_earlier_match", tag);
      assert.ok(flags(w, p.item_id).includes("license_soft"), tag);
      if (!hide) assert.equal(out(await publish(w, T, bankOf(served(await imp(w, T, p.item_id as string))), { license: "CC0-1.0" })), NCSA, `${tag}: downstream`);
    }
  // A stem too short for near-rewording bands is still found by its served forms (key-free, without
  // its explanation), and flagged even when the relay keeps the licence.
  for (const [name, change, hide, lic] of [
    ["hide_key", (s: Q) => s, true, "CC0-1.0"],
    ["explanation dropped", (s: Q) => ({ ...s, explanation: null }), false, "CC0-1.0"],
    ["same licence", (s: Q) => s, true, "CC-BY-NC-SA-4.0"],
  ] as const) {
    const w = new World();
    const A = w.user("teacher", "Ann"), P = w.user(), R = w.user();
    const SHORT = () => mcq("Identify it.", [["Ann's pressed fern frond from the 2019 trip", true], ["A moss sample", false], ["A lichen sample", false]], { explanation: "Ann's own field notes on spore arrangement and frond shape." });
    await publish(w, A, bankOf([SHORT()]), { license: "CC-BY-NC-SA-4.0" });
    w.advance(DAY);
    const p = (await publish(w, P, bankOf([change(SHORT())]), { license: lic, hideKey: hide })).body;
    assert.equal((await get(w, R, p.item_id as string)).body.license_check.status, "unverified_earlier_match", name);
    assert.ok(flags(w, p.item_id).includes("license_soft"), name);
  }
});

test("B3: Ann's hidden-key original; Tess re-adds Ann's key and explanation; Ann opens Tess's copy and republishes her own question shown, any licence", async () => {
  const AE = "Worked reasoning: logger data, slope and the threshold the class computed together in the 2024 survey.";
  const Q0 = () => mcq("In the 2024 Freeport trap survey described in class, which of these factors best explains the decline of juvenile Chinook salmon in the lower reach?", [["Warm water above 21 C", true], ["Striped bass predation", false], ["Reduced turbidity", false]], { explanation: AE });
  for (const lic of ["CC-BY-4.0", "CC-BY-NC-4.0"]) {
    const w = new World();
    const A = w.user("teacher", "Ann"), T = w.user("teacher", "Tess"), R = w.user();
    const a = (await publish(w, A, bankOf([Q0()]), { license: "CC-BY-4.0", hideKey: true })).body;
    w.advance(DAY);
    const s = served(await imp(w, T, a.item_id as string))[0]!;
    const t = (await publish(w, T, bankOf([{ ...s, keyless: undefined, kind: { Mc: { answers: s.kind.Mc.answers.map((x: Q) => ({ ...x, correct: x.text.startsWith("Warm") })) } }, explanation: AE }]), { license: "CC-BY-4.0" })).body;
    w.advance(DAY);
    await get(w, A, t.item_id as string);
    const a2 = await publish(w, A, bankOf([Q0()]), { license: lic });
    assert.equal(a2.status, 201, lic);
    assert.deepEqual((await get(w, R, a2.body.item_id as string)).body.questions[0].chain, [], lic);
  }
});

test("B4: an item held by reports comes back neither as a case or punctuation variant nor after it is deleted and purged", async () => {
  const STEM = "Describe the photosynthesis experiment our class ran with pondweed and a lamp at three distances?";
  for (const variant of ["upper-case", "punctuation", "delete+purge"]) {
    const w = new World();
    const A = w.user(), R = w.user(), reporters = [w.user(), w.user(), w.user()];
    const a = (await publish(w, A, bankOf([q(STEM)]))).body;
    for (const u of reporters) {
      w.history(u);
      await w.json("POST", `/items/${a.item_id}/report`, { user: u, json: { reason: "other" } });
    }
    assert.equal(holds(w, a.item_id).length, 1);
    let stem = STEM;
    if (variant === "upper-case") stem = STEM.toUpperCase();
    if (variant === "punctuation") stem = `${STEM.slice(0, -1)} ?!`;
    if (variant === "delete+purge") {
      await w.json("DELETE", `/items/${a.item_id}`, { user: A });
      w.advance(15 * DAY);
      await w.pool.daily();
      await w.pool.hourly();
      assert.equal(w.q<{ p: number | null }>(`SELECT purged_at AS p FROM items WHERE id = ?`, a.item_id)[0]!.p !== null, true, "purged");
    }
    const r = await publish(w, A, bankOf([q(stem)]));
    assert.equal((await get(w, R, r.body.item_id as string)).status, 404, variant);
    assert.equal(holds(w, r.body.item_id)[0]?.source, `carried:${a.item_id}`, variant);
  }
});

test("B5: student data in a Multipart PART is held as a standalone question, inside another Multipart, and as a near-rewording; a part can be named", async () => {
  const PART = "Jordan Ellison (period 3, IEP for reading) scored 2/10 on Friday's quiz; explain what his answers show about his misconception.";
  const mp = (parts: Q[]) => q("Answer both parts about the reading quiz results.", { kind: { Multipart: { parts } } });
  const part = (stem: string) => ({ stem, kind: { Short: { sample: "his misconception" } } });
  const cases: [string, Q][] = [
    ["standalone", q(PART)],
    ["in another Multipart", q("Two questions on reading.", { kind: { Multipart: { parts: [part("Name one comprehension strategy you teach."), part(PART)] } } })],
    ["near-rewording", q(PART.replace("Friday's", "Thursday's"))],
  ];
  for (const [name, body] of cases) {
    const w = new World();
    const adm = w.user("admin"), A = w.user(), M = w.user(), R = w.user();
    const a = (await publish(w, A, bankOf([mp([part("Which strategy helps most?"), part(PART)])]))).body;
    const pv = await w.admin<any>(adm, "POST", "/admin/pool/purge-preview", { item_id: a.item_id, questions: [0] });
    assert.ok(pv.body.match[0].keys.length > 4, "the parts' keys are in the default scope");
    assert.equal((await w.purge(adm, a.item_id as string, { questions: [0] })).status, 200);
    const r = (await publish(w, M, bankOf([body]))).body;
    assert.equal((await get(w, R, r.item_id as string)).status, 404, name);
  }
  // An EXISTING standalone copy of the part is listed and held by the purge (not only later ones),
  // a lifted part inside another Multipart is held by the exact key (not only as a near-rewording),
  // and a near-rewording of the part inside another Multipart is held as one.
  {
    const w = new World();
    const adm = w.user("admin"), A = w.user(), M = w.user();
    const copy = (await publish(w, M, bankOf([q(PART)]))).body;
    const inMp = (await publish(w, M, bankOf([q("Earlier questions on reading.", { kind: { Multipart: { parts: [{ stem: "Which book?", kind: { Short: { sample: "Holes" } } }, { stem: PART, kind: { Short: { sample: "Mal's view" } } }] } } })]))).body;
    const a = (await publish(w, A, bankOf([mp([part("Which strategy helps most?"), part(PART)])]))).body;
    const pv = await w.admin<any>(adm, "POST", "/admin/pool/purge-preview", { item_id: a.item_id, questions: [0] });
    assert.deepEqual(pv.body.affected.map((x: Q) => x.item_id).sort(), [copy.item_id, inMp.item_id].sort());
    await w.purge(adm, a.item_id as string, { questions: [0] });
    assert.deepEqual(holds(w, copy.item_id), [{ reason: "student_data", source: "privacy" }]);
    assert.deepEqual(holds(w, inMp.item_id), [{ reason: "student_data", source: "privacy" }], "an existing Multipart with the part");
    const lifted = (await publish(w, M, bankOf([q("Two questions on reading.", { kind: { Multipart: { parts: [part("Name one comprehension strategy you teach."), part(PART)] } } })]))).body;
    assert.deepEqual(holds(w, lifted.item_id), [{ reason: "student_data", source: "privacy" }]);
    const nearIn = (await publish(w, M, bankOf([q("Two more questions on reading.", { kind: { Multipart: { parts: [{ stem: "Which book did the class read?", kind: { Short: { sample: "Holes" } } }, { stem: PART.replace("Friday's", "Thursday's"), kind: { Short: { sample: "a different answer of Mal's" } } }] } } })]))).body;
    assert.deepEqual(holds(w, nearIn.item_id), [{ reason: "student_data", source: "privacy_near" }]);
  }
  // A part can be named alone: then only that part's text is blocked.
  const w = new World();
  const adm = w.user("admin"), A = w.user(), M = w.user(), R = w.user();
  const a = (await publish(w, A, bankOf([mp([part("Which strategy helps most?"), part(PART)])]))).body;
  assert.equal((await w.purge(adm, a.item_id as string, { questions: [0], scopes: { 0: "part:1" } })).status, 200);
  assert.equal((await get(w, R, (await publish(w, M, bankOf([q(PART)]))).body.item_id as string)).status, 404, "part 1 blocked");
  assert.equal((await get(w, R, (await publish(w, M, bankOf([q("Which strategy helps most?")]))).body.item_id as string)).status, 200, "part 0 not");
  assert.equal((await w.admin(adm, "POST", "/admin/pool/purge-preview", { item_id: a.item_id, scopes: { 0: "part:x" } })).status, 400);
});

test("B6: a lifted explanation (25+ words) or one pasted as a model answer binds like a copy, read or imported, whatever the stem; a short one does not", async () => {
  const LONG = "Ann's explanation: the logger shows water above twenty degrees on forty days of the 2019 season, juvenile counts fell in exactly those weeks, and the regression slope she computed links the two clearly.";
  for (const how of ["import", "read"])
    for (const as of ["explanation", "model answer"]) {
      const w = new World();
      const A = w.user("teacher", "Ann"), M = w.user(), R = w.user();
      const a = (await publish(w, A, bankOf([mcq("Ann's question about the Freeport trap?", [["Temperature", true], ["Predation", false]], { explanation: LONG })]), { license: "CC-BY-NC-SA-4.0" })).body;
      w.advance(DAY);
      if (how === "import") await imp(w, M, a.item_id as string);
      else await get(w, M, a.item_id as string);
      const mine = as === "explanation" ? mcq("Mal's own question about salmon in rivers?", [["Warm water", true], ["Cold water", false]], { explanation: LONG }) : q("Mal's own question: why did the salmon decline?", { kind: { Short: { sample: LONG } } });
      const tag = `${how}/${as}`;
      assert.equal(out(await publish(w, M, bankOf([mine]), { license: "CC0-1.0" })), NCSA, tag);
      assert.equal(out(await publish(w, M, bankOf([mine]), { license: "CC0-1.0", independent: [a.item_id as string], statement: "I wrote this myself from my notes." })), NCSA, `${tag}: a claim never covers it`);
      const ok = await publish(w, M, bankOf([mine]), { license: "CC-BY-NC-SA-4.0" });
      assert.deepEqual((await get(w, R, ok.body.item_id as string)).body.questions[0].chain.map((l: Q) => l.author_name), ["Ann"], tag);
    }
  // Ann's long MODEL ANSWER pasted as Mal's explanation binds too.
  {
    const w = new World();
    const A = w.user(), M = w.user();
    const a = (await publish(w, A, bankOf([q("Ann's question: why did the salmon decline?", { kind: { Short: { sample: LONG } } })]), { license: "CC-BY-NC-SA-4.0" })).body;
    await imp(w, M, a.item_id as string);
    assert.equal(out(await publish(w, M, bankOf([mcq("Mal's own question about salmon in rivers?", [["Warm water", true], ["Cold water", false]], { explanation: LONG })]), { license: "CC0-1.0" })), NCSA, "answer -> explanation");
  }
  const w = new World();
  const A = w.user(), M = w.user();
  const a = (await publish(w, A, bankOf([mcq("Ann's question about the Freeport trap?", [["Temperature", true], ["Predation", false]], { explanation: "Warm water killed them." })]), { license: "CC-BY-NC-SA-4.0" })).body;
  await imp(w, M, a.item_id as string);
  assert.equal((await publish(w, M, bankOf([mcq("Mal's own question about salmon in rivers?", [["Warm water", true], ["Cold water", false]], { explanation: "Warm water killed them." })]), { license: "CC0-1.0" })).status, 201, "short: not a lift");
});

test("B7: an image is a licence unit: its exact bytes, read or imported, bind and credit their item; a claim never covers them; a stock image does not bind; a takedown finds copies by bytes", async () => {
  for (const how of ["read", "import"]) {
    const w = new World();
    const V = w.user("teacher", "Vic"), M = w.user(), R = w.user();
    const up = await upload(w, V, png(9, 9, 31));
    const v = (await publish(w, V, bankOf([q("Vic's diagram of a leaf cross-section: label the palisade layer.", { image: { file: `media/${up.sha}.png`, width: 9, height: 9 } })]), { license: "CC-BY-NC-SA-4.0" })).body;
    w.advance(DAY);
    if (how === "read") await get(w, M, v.item_id as string);
    else await imp(w, M, v.item_id as string);
    await upload(w, M, png(9, 9, 31));
    const mine = q("Mal's own question: what does this picture show?", { image: { file: `media/${up.sha}.png`, width: 9, height: 9 } });
    assert.equal(out(await publish(w, M, bankOf([mine]), { license: "CC0-1.0" })), NCSA, how);
    assert.equal(out(await publish(w, M, bankOf([mine]), { license: "CC0-1.0", independent: [v.item_id as string], statement: "I drew this myself from a textbook." })), NCSA, `${how}: claim`);
    const ok = await publish(w, M, bankOf([mine]), { license: "CC-BY-NC-SA-4.0" });
    // (Round 17 M6: an image link is typed: "image also used by", never an unmodified question copy.)
    assert.deepEqual((await get(w, R, ok.body.item_id as string)).body.questions[0].chain.map((l: Q) => [l.author_name, l.kind]), [["Vic", "image"]], how);
  }
  // A stock image three other authors' items already serve does not bind.
  {
    const w = new World();
    const M = w.user(), R = w.user();
    const img = png(9, 9, 41);
    let last = "";
    for (let k = 0; k < 3; k++) {
      const u = w.user();
      const up = await upload(w, u, img);
      last = (await publish(w, u, bankOf([q(`Stock photo question ${k} about a different topic each time, number ${k}.`, { image: { file: `media/${up.sha}.png`, width: 9, height: 9 } })]), { license: "CC-BY-NC-4.0" })).body.item_id as string;
    }
    await get(w, M, last);
    const up = await upload(w, M, img);
    assert.equal((await publish(w, M, bankOf([q("Mal's own question with the stock photo.", { image: { file: `media/${up.sha}.png`, width: 9, height: 9 } })]), { license: "CC0-1.0" })).status, 201);
    void R;
  }
  // A takedown of an image finds every item serving the bytes.
  {
    const w = new World();
    const adm = w.user("admin"), X = w.user(), P = w.user();
    const img = png(9, 9, 51);
    const ux = await upload(w, X, img);
    const x = (await publish(w, X, bankOf([q("The textbook figure 4.7 question.", { image: { file: `media/${ux.sha}.png`, width: 9, height: 9 } })]))).body;
    const up = await upload(w, P, img);
    const p = (await publish(w, P, bankOf([q("An unrelated question with the same figure.", { image: { file: `media/${up.sha}.png`, width: 9, height: 9 } })]), { visibility: "unlisted" })).body;
    const pv = await w.admin<any>(adm, "POST", "/admin/dmca/takedown-preview", { item_id: x.item_id, questions: [], images: [ux.sha] });
    assert.ok(pv.body.copies.some((cc: Q) => cc.item_id === p.item_id), JSON.stringify(pv.body.copies));
  }
});

test("hardening (21d 1): share-core's limits on the raw bytes first: deep nesting and duplicate keys are a 422, never a 500, and never parsed differently", async () => {
  const w = new World();
  const A = w.user();
  const deep = `{"schema_version":2,"bank":{"id":"x","name":"b","questions":[${"[".repeat(5000)}${"]".repeat(5000)}],"created":1,"modified":1}}`;
  const d = await publish(w, A, deep);
  assert.equal(d.status, 422, JSON.stringify(d.body).slice(0, 200));
  // A duplicate key: the Worker's own parse would keep the second; share-core's refusal is the answer.
  const one = JSON.parse(bankOf([q("A question about rivers?")]));
  const dup = JSON.stringify(one).replace(`"stem":"A question about rivers?"`, `"stem":"A question about rivers?","stem":"Another stem entirely?"`);
  const r = await publish(w, A, dup);
  assert.equal(r.status, 422);
  assert.ok((r.body.errors as Q[]).some((e) => e.code === "duplicate_key"), JSON.stringify(r.body.errors));
});

test("hardening (21d 2): the tag shortlist keeps only shown publishes, newest first", async () => {
  const w = new World({ searchCandidates: 2 });
  const P1 = w.user();
  const meta = (title: string) => ({ title, subject: "Biology", course: "", grade: "", tags: ["kelp"], standards: [] });
  // (The public item is the one the tag index lists LAST, so only a shortlist of shown publishes keeps it.)
  for (let k = 0; k < 3; k++) {
    w.advance(1000);
    await publish(w, w.user(), bankOf([q(`Unlisted kelp question ${k}?`)]), { visibility: "unlisted", meta: meta(`Unlisted ${k}`) });
  }
  w.advance(1000);
  const pub = (await publish(w, P1, bankOf([q("Public kelp question?")]), { meta: meta("Public") })).body;
  assert.deepEqual((await w.json<any>("GET", "/items?tag=kelp", { user: w.user() })).body.items.map((x: Q) => x.id), [pub.item_id]);
});

test("hardening (21d 6): a PNG whose pixels would decode past 16 Mpix or 64 MB is refused before inflating", async () => {
  const ihdr = (w: number, h: number, depth: number, color: number) => {
    const b = new Uint8Array(13);
    const dv = new DataView(b.buffer);
    dv.setUint32(0, w);
    dv.setUint32(4, h);
    b.set([depth, color, 0, 0, 0], 8);
    return b;
  };
  const head = (w: number, h: number, depth: number, color: number) => concat(new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr(w, h, depth, color)), chunk("IDAT", new Uint8Array([0x78, 0x9c])), chunk("IEND", new Uint8Array(0)));
  // 4096 x 4096 RGBA at 16 bits: 134 MB decoded, refused at the header (before any inflating).
  assert.deepEqual(await checkImage(head(4096, 4096, 16, 6)), { refused: "png_decoded_size" });
  // 2048 x 2048 at 16 bits (33.5 MB) passes the header check (refused later only for its bogus data).
  assert.notDeepEqual(await checkImage(head(2048, 2048, 16, 6)), { refused: "png_decoded_size" });
});

test("hardening (21d 7): a release names the publish the admin reviewed; a newer one is not released unseen", async () => {
  const w = new World();
  const adm = w.user("admin"), S = w.user(), I = w.user();
  const STEM = "Explain your reasoning for the previous answer in two sentences.";
  const inn = (await publish(w, I, bankOf([q(STEM, { kind: { Short: { sample: "Innocent." } } })]))).body;
  const s = (await publish(w, S, bankOf([q(STEM, { kind: { Short: { sample: "Jenny Smith scored 42." } } })]))).body;
  await w.purge(adm, s.item_id as string, { questions: [0] });
  const body = { item_id: inn.item_id, source: "privacy", note: "Generic stem, no student data here." };
  assert.equal((await w.admin(adm, "POST", "/admin/pool/release-hold", body)).status, 400);
  assert.equal((await w.admin(adm, "POST", "/admin/pool/release-hold", { ...body, publish_id: "AAAAAAAAAAAAAAAAAAAAAA" })).status, 409);
  assert.equal((await w.admin(adm, "POST", "/admin/pool/release-hold", { ...body, publish_id: inn.publish_id })).status, 200);
});

test("hardening (21d 8): a pending account's requests are counted against its rate limit before it is told it is not approved", async () => {
  const store = new CountingStore();
  const w = new World({ rateStore: store });
  const P = w.user("pending");
  const before = [...store.counts.values()].reduce((a, b) => a + b, 0);
  assert.equal((await w.json("GET", "/items", { user: P })).status, 403);
  assert.ok([...store.counts.values()].reduce((a, b) => a + b, 0) > before, "counted");
});

test("21c non-blockers: a near-identical stem is not credited beside an exact source (flagged); a relicensed source's refusal says what to do", async () => {
  // c4: imported X (exact) and Y (stem one word off): only X is credited, the near one flagged.
  const w = new World();
  const X = w.user("teacher", "X"), Y = w.user("teacher", "Y"), T = w.user(), R = w.user();
  const stem = "Which of these organisms in the pond survey from last spring produced the most oxygen per gram of biomass?";
  const x = (await publish(w, X, bankOf([q(stem)]))).body;
  const y = (await publish(w, Y, bankOf([q(stem.replace("spring", "summer"))]))).body;
  w.advance(DAY);
  await imp(w, T, x.item_id as string);
  await imp(w, T, y.item_id as string);
  const t = (await publish(w, T, bankOf([q(stem)]))).body;
  assert.deepEqual((await get(w, R, t.item_id as string)).body.questions[0].chain.map((l: Q) => l.author_name), ["X"]);
  assert.ok(flags(w, t.item_id).includes("origin_suspect"));
  // c14: Ann relicenses NC after Tess imported under CC-BY; Tess's sync is told why and what to do.
  const w2 = new World();
  const A = w2.user("teacher", "Ann"), T2 = w2.user("teacher", "Tess");
  const Q0 = mcq("Which city is the capitol of France?", [["Paris", true], ["Rome", false], ["Oslo", false]]);
  const a = (await publish(w2, A, bankOf([Q0]), { license: "CC-BY-4.0" })).body;
  await imp(w2, T2, a.item_id as string);
  await publish(w2, A, bankOf([{ ...Q0, stem: "Which city is the capital of France?", published: { item_id: a.item_id, qid: (a.qids as string[])[0], publish_id: a.publish_id, version: 1 } }]), { license: "CC-BY-NC-4.0", base: a.publish_id as string }, a.item_id as string);
  const s2 = served(await imp(w2, T2, a.item_id as string));
  const sync = await publish(w2, T2, bankOf(s2), { license: "CC-BY-4.0" });
  assert.equal(sync.body.error, "license_incompatible");
  assert.equal((sync.body.relicensed as Q[])[0]!.now, "CC-BY-NC-4.0");
  assert.match(String(sync.body.message), /relicensed/);
});
