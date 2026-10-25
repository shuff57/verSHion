// Round 43: circles PHASE C, "hide keys from viewers" (docs/circles-plan.md D12). A per-bank `hide_key` flag that the
// bank's author (while an editor or the owner) or the circle's owner sets with PATCH. VIEWERS then read every version
// of that bank in share-core's stripped form (its strip and key-hint gate, never anything the Worker invents); the
// owner and editors still read the full text; the stored bank is unchanged. The author's strip decisions (labels,
// order acknowledgements, low-risk formatting acknowledgements, images kept) are recorded per version: by the PATCH
// for the head, by every send to a bank that hides its keys (a send the gate refuses stores nothing), and copied by a
// rollback. A version the gate cannot strip with its recorded decisions is refused to viewers (409), never served
// whole. A viewer's read is publish-sized work: the heavy slot, the send's reservation, a daily cap of its own.
import assert from "node:assert/strict";
import { test } from "node:test";
import { POOL_TERMS_VERSION } from "../../src/pool/license.ts";
import { bytesReserved, type HeavySlots } from "../../src/pool/shared.ts";
import { decideLabels, framed, genQuestion, png, publish, publishId, rng, upload, World, type GenQuestion, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const PUB = "application/vnd.vershion.publish";
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "SAMPLEKEYzzq" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const mcq = (stem: string, answers: [string, boolean][], extra: Q = {}): Q => q(stem, { kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } }, ...extra });
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "local", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });

async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][]): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name: "Algebra team" } })).body.circle.id;
  for (const [u, role] of members) {
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } })).body.code;
    assert.equal((await w.json("POST", "/circles/join", { user: u, json: { code: k } })).status, 200);
    w.advance(1000);
  }
  return cid;
}
interface SendOpts {
  bid?: string;
  base?: number;
  message?: string;
  extra?: Q;
  confirm?: boolean;
}
async function sendRaw(w: World, u: User, cid: string, payload: string, o: SendOpts = {}) {
  const meta: Q = {
    commit_id: publishId(),
    license: "circle-only",
    attest: { own_work: true, terms_version: POOL_TERMS_VERSION },
    meta: { title: "Unit 2", subject: "Math", course: "", grade: "", tags: [], standards: [] },
    ...(o.message ? { message: o.message } : {}),
    ...(o.bid ? { base_version: o.base } : {}),
    ...(o.confirm ? { confirm_large_change: true } : {}),
    ...(o.extra ?? {}),
  };
  return w.json<any>(o.bid ? "PUT" : "POST", o.bid ? `/circles/${cid}/banks/${o.bid}` : `/circles/${cid}/banks`, { user: u, body: framed(meta, payload), contentType: PUB });
}
const patch = (w: World, u: User, cid: string, bid: string, body: Q) => w.json<any>("PATCH", `/circles/${cid}/banks/${bid}`, { user: u, json: body });
/** PATCH hide_key on as the app would: the label decisions it computes, then the order acknowledgements the pool asks for. */
async function hideKeys(w: World, u: User, cid: string, bid: string, payload: string, extra: Q = {}) {
  const body: Q = { hide_key: true, labels: decideLabels(payload), ...extra };
  let r = await patch(w, u, cid, bid, body);
  if (r.status === 422 && r.body.error === "order_ack_required") r = await patch(w, u, cid, bid, { ...body, order_acks: r.body.needs_order_ack });
  return r;
}
const read = (w: World, u: User, path: string, headers: Record<string, string> = {}) => w.call("GET", path, { user: u, headers });
const body = async (r: Response) => JSON.parse(await r.text()) as Q;

/** A bank with key material in every key-bearing slot (the harness's generator), and the secrets to sweep for. */
function genBank(seed: number, n = 12): { payload: string; qs: GenQuestion[]; secrets: string[] } {
  const r = rng(seed);
  const kinds = ["Mc", "Tf", "Matching", "Blank", "Short", "Multipart"];
  const qs = Array.from({ length: n }, (_, i) => genQuestion(r, kinds[i % kinds.length]!, i));
  const payload = JSON.stringify({ schema_version: 1, bank: { id: "blocal", name: "Generated bank", questions: qs.map((x) => x.json), created: 1760000000000, modified: 1760000000000 } });
  return { payload, qs, secrets: qs.flatMap((x) => x.secrets) };
}
/** What would give a key away in a served bank: a correct flag, a True/False true, an accepted blank, a sample, a right column, an explanation, a figure. */
function keyMaterial(payload: Q): string[] {
  const out: string[] = [];
  const walk = (qq: Q, where: string) => {
    const k = qq.kind ?? {};
    if (k.Mc && k.Mc.answers.some((a: Q) => a.correct)) out.push(`${where} Mc correct`);
    if (k.Tf && k.Tf.correct) out.push(`${where} Tf true`);
    if (k.Blank && k.Blank.accept.some((a: unknown[]) => a.length > 0)) out.push(`${where} Blank accept`);
    if (k.Short && k.Short.sample) out.push(`${where} Short sample`);
    if (k.Matching && k.Matching.pairs.some((p: Q) => p.right)) out.push(`${where} Matching right`);
    if (k.Multipart) k.Multipart.parts.forEach((p: Q, j: number) => walk(p, `${where}.${j}`));
    if (qq.explanation) out.push(`${where} explanation`);
    if (qq.figure) out.push(`${where} figure`);
  };
  (payload.bank.questions as Q[]).forEach((x, i) => walk(x, `q${i}`));
  return out;
}

/** A bank without letter labels (it passes the gate with no decisions) and its key material. */
function plainKeys(): { payload: string; secrets: string[] } {
  const qs = [
    mcq("Which planet is closest to the sun in our solar system?", [["Mercury", true], ["Venus", false], ["Mars", false]], { explanation: "EXPLkeyqqv" }),
    q("Name the largest ocean on the planet.", { kind: { Short: { sample: "SAMPLEkeyqqv" } } }),
    q("At sea level water boils at ___ degrees Celsius.", { kind: { Blank: { accept: [["ACCEPTkeyqqv"]] } } }),
    q("The moon is a planet of its own.", { kind: { Tf: { correct: true } } }),
  ];
  return { payload: bankOf(qs), secrets: ["EXPLkeyqqv", "SAMPLEkeyqqv", "ACCEPTkeyqqv"] };
}

async function world() {
  const w = new World();
  const [O, A, E, V] = ["Olga", "Ann", "Eli", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [E!, "editor"], [V!, "viewer"]]);
  return { w, O: O!, A: A!, E: E!, V: V!, cid };
}

test("43-1: the author hides the keys: viewers read every version stripped by share-core (no correct flag, accepted answer, sample, right column, explanation, figure or secret), editors and the owner still read the full text, the stored bank is unchanged; viewers' version messages are blank", async () => {
  const { w, O, A, E, V, cid } = await world();
  const g = plainKeys();
  const s1 = await sendRaw(w, A, cid, g.payload, { message: "v1 by Ann, the key to Q3 is choice2" });
  assert.equal(s1.status, 201, JSON.stringify(s1.body));
  const bid = s1.body.bank_id;
  const g2 = genBank(44);
  const s2 = await sendRaw(w, A, cid, g2.payload, { bid, base: 1, confirm: true, message: "v2: answers fixed" });
  assert.equal(s2.status, 200, JSON.stringify(s2.body));
  const r2before = new Map([...w.r2.objects].map(([k, v]) => [k, Buffer.from(v.bytes).toString("base64")]));
  const before = await body(await read(w, V, `/circles/${cid}/banks/${bid}`));
  assert.ok(keyMaterial(before.payload).length > 0, "before: viewers see the keys (D12's default)");
  const h = await hideKeys(w, A, cid, bid, g2.payload);
  assert.equal(h.status, 200, JSON.stringify(h.body));
  assert.equal(h.body.hide_key, true);
  // The stored bodies are untouched.
  assert.deepEqual(new Map([...w.r2.objects].map(([k, v]) => [k, Buffer.from(v.bytes).toString("base64")])), r2before);
  // The viewer: the head and an older version (sent before the flag; it passes the gate as it is), both stripped.
  for (const path of [`/circles/${cid}/banks/${bid}`, `/circles/${cid}/banks/${bid}/versions/2`]) {
    const r = await read(w, V, path);
    const t = await r.text();
    assert.equal(r.status, 200, t.slice(0, 300));
    const b = JSON.parse(t) as Q;
    assert.equal(b.bank.keys_hidden, true);
    assert.equal(b.bank.hide_key, true);
    assert.deepEqual(keyMaterial(b.payload), [], path);
    for (const s of g2.secrets) assert.ok(!t.includes(s), `${path} leaks ${s}`);
    assert.ok((b.payload.bank.questions as Q[]).every((x) => x.keyless === true));
    assert.deepEqual((b.payload.bank.questions as Q[]).map((x) => x.id), s2.body.qids, "the qids survive (a copy stays linked)");
    assert.equal(b.bank.message, "", "an editor's message is not shown to a viewer of a hidden-key bank");
  }
  // v1 is a different bank of questions: stripped too.
  const v1 = await read(w, V, `/circles/${cid}/banks/${bid}/versions/1`);
  const t1 = await v1.text();
  assert.equal(v1.status, 200);
  for (const s of g.secrets) assert.ok(!t1.includes(s));
  assert.deepEqual(keyMaterial(JSON.parse(t1).payload), []);
  // Editors, the owner and the author read the full text.
  for (const u of [O, A, E]) {
    const b = await body(await read(w, u, `/circles/${cid}/banks/${bid}`));
    assert.equal(b.bank.keys_hidden, undefined);
    assert.ok(keyMaterial(b.payload).length > 0);
    assert.equal(b.bank.message, "v2: answers fixed");
  }
  // The history: messages blank to the viewer only.
  const hv = await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions`, { user: V });
  assert.deepEqual(hv.body.versions.map((x: Q) => x.message), ["", ""]);
  const he = await w.json<any>("GET", `/circles/${cid}/banks/${bid}/versions`, { user: E });
  assert.deepEqual(he.body.versions.map((x: Q) => x.message), ["v2: answers fixed", "v1 by Ann, the key to Q3 is choice2"]);
  // The circle page says so to every member.
  const page = await w.json<any>("GET", `/circles/${cid}`, { user: V });
  assert.equal(page.body.banks[0].hide_key, true);
  // Turned off: viewers read the full text again.
  assert.deepEqual((await patch(w, A, cid, bid, { hide_key: false })).body, { ok: true, hide_key: false });
  assert.ok(keyMaterial((await body(await read(w, V, `/circles/${cid}/banks/${bid}`))).payload).length > 0);
  assert.equal(bytesReserved(), 0);
});

test("43-2: the key-hint gate: a head that gives a key away cannot be hidden (422 with share-core's findings, nothing changes); labels need the author's decision; a send to a hidden-key bank passes the gate or stores nothing; viewers then read it with the decisions it was sent with", async () => {
  const { w, A, E, V, cid } = await world();
  const leak = bankOf([mcq("What is the capital of France? ANSWER: B", [["Rome", false], ["Paris", true], ["Oslo", false]])]);
  const s = await sendRaw(w, A, cid, leak);
  const bid = s.body.bank_id;
  const r = await patch(w, A, cid, bid, { hide_key: true });
  assert.equal(r.status, 422);
  assert.ok((r.body.errors as Q[]).some((e) => e.code === "key_in_stem"), JSON.stringify(r.body));
  assert.equal(w.q<any>(`SELECT hide_key FROM circle_banks WHERE id = ?`, bid)[0].hide_key, 0);
  // Labelled choices: a decision per finding.
  const labelled = bankOf([mcq("Which gas do plants take in?", [["A. Oxygen", false], ["B. Carbon dioxide", true], ["C. Helium", false]])]);
  const s2 = await sendRaw(w, A, cid, labelled);
  const bid2 = s2.body.bank_id;
  const nol = await patch(w, A, cid, bid2, { hide_key: true });
  assert.deepEqual([nol.status, nol.body.error], [422, "labels_need_decision"]);
  assert.ok(Array.isArray(nol.body.label_findings));
  const ok = await patch(w, A, cid, bid2, { hide_key: true, labels: [{ question: 0, choice: "strip" }] });
  assert.equal(ok.status, 200, JSON.stringify(ok.body));
  assert.deepEqual(ok.body.labels_removed, [{ question: 0 }]);
  const vv = await body(await read(w, V, `/circles/${cid}/banks/${bid2}`));
  assert.deepEqual((vv.payload.bank.questions[0].kind.Mc.answers as Q[]).map((a) => a.text).sort(), ["Carbon dioxide", "Helium", "Oxygen"], "labels stripped as decided");
  // An editor's send to it: without decisions the gate refuses and nothing is stored.
  const n0 = w.q(`SELECT 1 FROM circle_versions`).length;
  const labelled2 = bankOf([mcq("Which gas do plants take in?", [["A. Oxygen", false], ["B. Carbon dioxide", true], ["C. Helium", false], ["D. Argon", false]], { published: { item_id: bid2, qid: s2.body.qids[0], publish_id: "p", version: 1 } })]);
  const bad = await sendRaw(w, E, cid, labelled2, { bid: bid2, base: 1 });
  assert.deepEqual([bad.status, bad.body.error], [422, "labels_need_decision"], JSON.stringify(bad.body));
  const leaky = await sendRaw(w, E, cid, bankOf([mcq("Capital of Spain? Answer: Madrid", [["Madrid", true], ["Lisbon", false], ["Rome", false]])]), { bid: bid2, base: 1, extra: { labels: [] }, confirm: true });
  assert.equal(leaky.status, 422);
  assert.ok((leaky.body.errors as Q[]).some((e) => e.code === "key_in_stem"));
  assert.equal(w.q(`SELECT 1 FROM circle_versions`).length, n0, "a refused send stores nothing");
  // With the decisions: accepted, and the viewer reads it stripped as decided.
  const good = await sendRaw(w, E, cid, labelled2, { bid: bid2, base: 1, extra: { labels: [{ question: 0, choice: "strip" }] } });
  assert.equal(good.status, 200, JSON.stringify(good.body));
  assert.equal(good.body.keys_hidden, true);
  const v2 = await body(await read(w, V, `/circles/${cid}/banks/${bid2}`));
  assert.deepEqual((v2.payload.bank.questions[0].kind.Mc.answers as Q[]).map((a) => a.text).sort(), ["Argon", "Carbon dioxide", "Helium", "Oxygen"]);
  assert.deepEqual(keyMaterial(v2.payload), []);
  // A preserved order needs its acknowledgement (as a public publish).
  const kept = bankOf([mcq("Order of the planets?", [["Mercury", false], ["Venus", true], ["Earth", false]], { keep_order: true, published: { item_id: bid2, qid: s2.body.qids[0], publish_id: "p", version: 1 } })]);
  const na = await sendRaw(w, E, cid, kept, { bid: bid2, base: 2 });
  assert.deepEqual([na.status, na.body.error, na.body.needs_order_ack], [422, "order_ack_required", [{ question: 0 }]]);
  assert.equal((await sendRaw(w, E, cid, kept, { bid: bid2, base: 2, extra: { order_acks: [{ question: 0 }] } })).status, 200);
});

test("43-3: per version, fail closed: an older version that cannot be stripped with what it recorded is refused to viewers (409 keys_hidden_unavailable, no text, no findings), never served whole; a rollback copies its target's decisions; the author's PATCH records new ones for the head", async () => {
  const { w, O, A, E, V, cid } = await world();
  const labelled = bankOf([mcq("Which gas do plants take in?", [["A. Oxygen", false], ["B. Carbon dioxide", true], ["C. Helium", false]])]);
  const s1 = await sendRaw(w, A, cid, labelled);
  const bid = s1.body.bank_id;
  const plain = bankOf([mcq("Which gas do plants take in?", [["Oxygen", false], ["Carbon dioxide", true], ["Helium", false]], { published: { item_id: bid, qid: s1.body.qids[0], publish_id: "p", version: 1 } })]);
  assert.equal((await sendRaw(w, A, cid, plain, { bid, base: 1 })).status, 200);
  assert.equal((await patch(w, A, cid, bid, { hide_key: true })).status, 200, "v2 has no labels: nothing to decide");
  // v1 (labelled, sent before the flag, no decisions recorded): the viewer is refused, plainly.
  const r = await read(w, V, `/circles/${cid}/banks/${bid}/versions/1`);
  const t = await r.text();
  assert.equal(r.status, 409);
  const b = JSON.parse(t) as Q;
  assert.equal(b.error, "keys_hidden_unavailable");
  assert.deepEqual(Object.keys(b).sort(), ["error", "message"]);
  assert.ok(!t.includes("Carbon") && !t.includes("label"));
  assert.equal((await read(w, E, `/circles/${cid}/banks/${bid}/versions/1`)).status, 200, "an editor still reads it");
  // The owner rolls back to v1: the new head copies v1's (absent) decisions, so viewers are refused it too ...
  const rb = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/rollback`, { user: O, json: { commit_id: publishId(), to_version: 1, base_version: 2 } });
  assert.equal(rb.status, 200, JSON.stringify(rb.body));
  assert.equal((await read(w, V, `/circles/${cid}/banks/${bid}`)).status, 409);
  // ... until the owner (or the author) records decisions for the head.
  const again = await patch(w, O, cid, bid, { hide_key: true, labels: [{ question: 0, choice: "strip" }] });
  assert.equal(again.status, 200, JSON.stringify(again.body));
  const v = await read(w, V, `/circles/${cid}/banks/${bid}`);
  assert.equal(v.status, 200);
  assert.deepEqual(keyMaterial((await body(v)).payload), []);
  // A rollback to v2 copies v2's decisions: readable stripped at once.
  const rb2 = await w.json<any>("POST", `/circles/${cid}/banks/${bid}/rollback`, { user: O, json: { commit_id: publishId(), to_version: 2, base_version: 3 } });
  assert.equal(rb2.status, 200);
  assert.equal((await read(w, V, `/circles/${cid}/banks/${bid}`)).status, 200);
  assert.equal(bytesReserved(), 0);
});

test("43-4: images: the viewer's copy keeps only the images the decisions keep (image_ok), and the bank's image route serves a viewer only those; editors get every image", async () => {
  const { w, A, E, V, cid } = await world();
  const i1 = await upload(w, A, png(4, 4, 1));
  const i2 = await upload(w, A, png(4, 4, 2));
  const img = (sha: string) => ({ file: `media/${sha}.png`, width: 4, height: 4 });
  const payload = bankOf([q("Label the diagram's parts.", { image: img(i1.sha) }), q("Which stage does the second diagram show?", { image: img(i2.sha) })]);
  const s = await sendRaw(w, A, cid, payload, { extra: { hide_key: true, image_ok: [0] } });
  assert.equal(s.status, 201, JSON.stringify(s.body));
  const bid = s.body.bank_id;
  assert.equal(w.q<any>(`SELECT hide_key FROM circle_banks WHERE id = ?`, bid)[0].hide_key, 1, "a new bank can hide its keys from its first version");
  const v = await body(await read(w, V, `/circles/${cid}/banks/${bid}`));
  assert.ok(v.payload.bank.questions[0].image, "image 0 kept");
  assert.equal(v.payload.bank.questions[1].image ?? null, null, "image 1 removed");
  const blob = (u: User, sha: string) => w.call("GET", `/circles/${cid}/banks/${bid}/blobs/${sha}`, { user: u });
  assert.equal((await blob(V, i1.sha)).status, 200);
  assert.equal((await blob(V, i2.sha)).status, 404, "an image the viewer's copy does not keep is not served to a viewer");
  assert.equal((await blob(E, i2.sha)).status, 200);
  assert.equal((await blob(A, i2.sha)).status, 200);
});

test("43-5: who may set it: the bank's author while an editor or the owner, and the circle's owner; never a non-author editor or a viewer (403), a demoted author (403), a non-member (404); the licence stays the author's alone", async () => {
  const { w, O, A, E, V, cid } = await world();
  const X = w.user("teacher", "Xan");
  const s = await sendRaw(w, A, cid, bankOf([q("A plain question?")]));
  const bid = s.body.bank_id;
  assert.deepEqual([(await patch(w, E, cid, bid, { hide_key: true })).status, (await patch(w, E, cid, bid, { hide_key: true })).body.error], [403, "circle_role_required"]);
  assert.equal((await patch(w, V, cid, bid, { hide_key: true })).status, 403);
  assert.equal((await patch(w, X, cid, bid, { hide_key: true })).status, 404);
  assert.equal((await patch(w, O, cid, bid, { hide_key: true })).status, 200, "the owner");
  assert.equal((await patch(w, O, cid, bid, { license: "CC-BY-4.0" })).status, 403, "the licence is still the author's");
  assert.equal((await patch(w, A, cid, bid, { hide_key: false })).status, 200, "the author while an editor");
  assert.equal((await w.json("POST", `/circles/${cid}/members/role`, { user: O, json: { user_id: A.id, role: "viewer" } })).status, 200);
  assert.equal((await patch(w, A, cid, bid, { hide_key: true })).status, 403, "a demoted author");
  for (const bad of [{}, { hide_key: "yes" }, { hide_key: false, labels: [] }, { hide_key: true, labels: "x" }, { hide_key: true, image_ok: [-1] }, { hide_key: true, nonsense: 1 }]) {
    const r = await patch(w, O, cid, bid, bad);
    assert.equal(r.status, 400, JSON.stringify(bad));
  }
  // A send that tries to change the flag is refused (only PATCH changes it after the first version).
  const put = await sendRaw(w, O, cid, bankOf([q("A plain question?")]), { bid, base: 1, extra: { hide_key: false } });
  assert.deepEqual([put.status, put.body.field], [400, "hide_key"]);
  // The demoted author now reads it as a viewer: stripped.
  assert.equal((await patch(w, O, cid, bid, { hide_key: true })).status, 200);
  assert.equal((await body(await read(w, A, `/circles/${cid}/banks/${bid}`))).bank.keys_hidden, true);
});

test("43-6: cost: a viewer's stripped read takes the isolate's heavy slot (503 busy while it is taken, an editor's plain read is not held up), reserves and releases like a send of that body, counts toward its own daily cap (circle_hidden_reads), and a 304 strips nothing and is not counted", async () => {
  const slots: HeavySlots = { active: 0, queue: [] };
  const w = new World({ heavySlots: slots, heavyQueueMs: 30 });
  const [O, A, V] = ["Olga", "Ann", "Vic"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "editor"], [V!, "viewer"]]);
  const g = genBank(46, 30);
  const s = await sendRaw(w, A!, cid, g.payload);
  const bid = s.body.bank_id;
  assert.equal((await hideKeys(w, A!, cid, bid, g.payload)).status, 200);
  const path = `/circles/${cid}/banks/${bid}`;
  slots.active = 1;
  const busy = await read(w, V!, path);
  assert.equal(busy.status, 503);
  assert.ok(Number(busy.headers.get("Retry-After")) > 0);
  assert.equal((await read(w, A!, path)).status, 200, "a full read needs no heavy slot");
  slots.active = 0;
  const ok = await read(w, V!, path);
  assert.equal(ok.status, 200);
  const etag = ok.headers.get("ETag")!;
  assert.notEqual(etag, (await read(w, A!, path)).headers.get("ETag"), "the viewer's ETag is not the editor's");
  assert.equal(bytesReserved(), 0, "everything released");
  const count = () => w.q<{ count: number }>(`SELECT count FROM usage_daily WHERE user_id = ? AND metric = 'circle_hidden_reads'`, V!.id)[0]?.count ?? 0;
  const c0 = count();
  assert.equal((await read(w, V!, path, { "If-None-Match": etag })).status, 304);
  assert.equal(count(), c0, "a 304 strips nothing");
  const head = await w.call("HEAD", path, { user: V! });
  assert.equal(head.status, 200);
  assert.equal(head.headers.get("Content-Length"), null, "the stripped length is not known without stripping");
  // The daily cap.
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'circle_hidden_reads', ?, 0, 'test')`).run(V!.id, c0 + 1);
  assert.equal((await read(w, V!, path)).status, 200);
  const over = await read(w, V!, path);
  assert.equal(over.status, 429);
  assert.equal((await body(over)).metric, "circle_hidden_reads");
  assert.equal(bytesReserved(), 0);
});

test("43-7: what viewers read is in the circle's re-share records: a viewer who publishes the stripped copy (labels stripped) publicly is refused circle_material, as for the full text", async () => {
  const { w, A, V, cid } = await world();
  const labelled = bankOf([mcq("Which organelle in a eukaryotic cell produces most of the ATP that the cell uses during aerobic respiration?", [["A. Mitochondrion", true], ["B. Ribosome", false], ["C. Golgi body", false], ["D. Lysosome", false]])]);
  const s = await sendRaw(w, A, cid, labelled, { extra: { hide_key: true, labels: [{ question: 0, choice: "strip" }] } });
  assert.equal(s.status, 201, JSON.stringify(s.body));
  const v = await body(await read(w, V, `/circles/${cid}/banks/${s.body.bank_id}`));
  assert.equal(v.bank.keys_hidden, true);
  assert.ok((v.payload.bank.questions[0].kind.Mc.answers as Q[]).every((a) => !/^[A-D]\. /.test(a.text) && a.correct === false), "the viewer's copy is the stripped form, its labels gone");
  assert.equal(w.q(`SELECT 1 FROM circle_qkeys WHERE bank_id = ?`, s.body.bank_id).length, 2, "the full form's and the served form's key-free hashes");
  const copy = JSON.stringify({ schema_version: 2, bank: { ...v.payload.bank, id: "mine" } });
  const p = await publish(w, V, copy, { hideKey: true });
  assert.equal(p.status, 422, JSON.stringify(p.body).slice(0, 400));
  assert.equal(p.body.error, "circle_material");
  // The served-form record carries the FIRST sender of the full text: the author herself publishes her own words.
  const mine = await publish(w, A, copy, { hideKey: true });
  assert.equal(mine.status, 201, JSON.stringify(mine.body).slice(0, 400));
});

test("43-8: the leak sweep: every route a viewer of a hidden-key bank can call (circle page, head, HEAD, versions, history, merge, rollback, trim, report, images, heads, activity, export) returns none of the bank's key material", async () => {
  const { w, A, E, V, cid } = await world();
  const g = genBank(48, 18);
  const s = await sendRaw(w, A, cid, g.payload, { message: "first" });
  const bid = s.body.bank_id;
  assert.equal((await hideKeys(w, A, cid, bid, g.payload)).status, 200);
  const g2 = genBank(49, 18);
  const labels2 = decideLabels(g2.payload);
  let s2 = await sendRaw(w, E, cid, g2.payload, { bid, base: 1, confirm: true, message: `the keys are ${g2.secrets.slice(0, 3).join(", ")}`, extra: { labels: labels2 } });
  if (s2.status === 422 && s2.body.error === "order_ack_required") s2 = await sendRaw(w, E, cid, g2.payload, { bid, base: 1, confirm: true, message: `the keys are ${g2.secrets.slice(0, 3).join(", ")}`, extra: { labels: labels2, order_acks: s2.body.needs_order_ack } });
  assert.equal(s2.status, 200, JSON.stringify(s2.body).slice(0, 400));
  w.sql.prepare(`INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at, user_agent) VALUES (?, ?, ?, ?, ?, ?, ?, 'TestBrowser/1')`).run(`s-${V.id}`, `fam-${V.id}`, V.id, `h-${V.id}`, w.clock, w.clock, w.clock + 30 * DAY);
  const texts: string[] = [];
  const grab = async (method: string, path: string, json?: Q) => {
    const r = await w.call(method, path, { user: V, ...(json ? { json } : {}) });
    texts.push(`${method} ${path} ${r.status} ${await r.text()} ${[...r.headers].join(";")}`);
    return r.status;
  };
  assert.equal(await grab("GET", `/circles/${cid}`), 200);
  assert.equal(await grab("GET", `/circles/${cid}/banks/${bid}`), 200);
  assert.equal(await grab("HEAD", `/circles/${cid}/banks/${bid}`), 200);
  assert.equal(await grab("GET", `/circles/${cid}/banks/${bid}/versions`), 200);
  for (const n of [1, 2]) assert.equal(await grab("GET", `/circles/${cid}/banks/${bid}/versions/${n}`), 200);
  assert.equal(await grab("POST", `/circles/${cid}/banks/${bid}/merge`, { base_version: 1 }), 403);
  assert.equal(await grab("POST", `/circles/${cid}/banks/${bid}/rollback`, { commit_id: publishId(), to_version: 1, base_version: 2 }), 403);
  assert.equal(await grab("POST", `/circles/${cid}/banks/${bid}/trim`, { keep: 1 }), 403);
  assert.equal(await grab("GET", `/circles/${cid}/activity`), 200);
  assert.equal(await grab("POST", "/circles/heads", { bank_ids: [bid] }), 200);
  assert.equal(await grab("GET", "/me/export"), 200);
  assert.equal(await grab("POST", `/circles/${cid}/banks/${bid}/report`, { reason: "other", note: "just looking" }), 202);
  const all = texts.join("\n");
  for (const sct of [...g.secrets, ...g2.secrets]) assert.ok(!all.includes(sct), `leaked ${sct}`);
  assert.ok(!all.includes('"correct":true'));
  assert.equal(bytesReserved(), 0);
});
