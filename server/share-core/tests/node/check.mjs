// Runs the wasm builds under Node.
//   node tests/node/check.mjs [pkg] [test-pkg]
// * `pkg` (default ./pkg): the production build (`wasm-pack build --release --target web`). Checks
//   the exports (no `strip_with_seed`), invariants of `strip` with its internal seed, refusals, and
//   a wasm-memory CEILING: each hostile input and the legitimate 4 MB bank run in a fresh instance
//   (tests/node/mem.mjs) and must grow linear memory by less than 40 MB.
// * `test-pkg` (default ./target/pkg-test): the same crate built with `--features test-seed`
//   (`wasm-pack build --release --target web --out-dir target/pkg-test -- --features test-seed`).
//   Checks byte-for-byte equality with the native goldens for the fixed seed.
import { readFileSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const pkgDir = resolve(process.argv[2] || join(root, "pkg"));
const testDir = resolve(process.argv[3] || join(root, "target", "pkg-test"));
const enc = (s) => new TextEncoder().encode(s);
const CEILING = 40 * 1024 * 1024;

let failures = 0;
let checks = 0;
const check = (cond, msg) => {
  checks++;
  if (!cond) {
    failures++;
    console.error("FAIL:", msg);
  }
};
const canon = (v) => JSON.stringify(sortKeys(v));
function sortKeys(v) {
  if (Array.isArray(v)) return v.map(sortKeys);
  if (v && typeof v === "object") return Object.fromEntries(Object.keys(v).sort().map((k) => [k, sortKeys(v[k])]));
  return v;
}
const names = [
  "question_mc", "question_mc_multi", "question_tf", "question_matching", "question_blank",
  "question_short", "question_multipart", "question_figure_image", "question_matching_shared", "bank_all_kinds", "bank_no_details",
];
const payloadOf = (name) => {
  const inner = readFileSync(join(root, "fixtures", "model", `${name}.json`), "utf8");
  const key = name.startsWith("bank") ? "bank" : "question";
  return { key, inner, payload: `{"schema_version":1,"${key}":${inner}}` };
};
const hashes = JSON.parse(readFileSync(join(root, "fixtures", "hashes.json"), "utf8"));

// ---- production build -----------------------------------------------------------------------------
const m = await import(join(pkgDir, "share_core.js"));
m.initSync({ module: readFileSync(join(pkgDir, "share_core_bg.wasm")) });
check(m.strip_with_seed === undefined, "the production build does not export strip_with_seed");
check(m.strip.length === 2, "strip takes (payload bytes, options): no seed parameter");
// Decisions for every label finding: Strip where allowed (what the author would pick here).
const decide = (bytes) => {
  const f = JSON.parse(m.label_findings(bytes, ""));
  return JSON.stringify({ labels: f.label_findings.map((x) => ({ question: x.question, part: x.part ?? undefined, choice: x.strip_allowed ? "strip" : "keep" })) });
};
// strip as an author who acknowledges every preserved order (the ack is required).
const stripAck = (fn, bytes, opts, ...rest) => {
  let r = JSON.parse(fn(bytes, ...rest, opts));
  if (r.ok === false && r.errors[0].code === "order_ack_required") {
    const o = opts ? JSON.parse(opts) : {};
    o.order_acks = r.needs_order_ack;
    r = JSON.parse(fn(bytes, ...rest, JSON.stringify(o)));
  }
  return r;
};
// A JS string where bytes are expected: a clear refusal, never invalid_json.
for (const fn of ["validate", "validate_for_publish", "strip", "match_hash", "full_hash", "validate_meta", "label_findings"]) {
  const r = JSON.parse(m[fn]('{"schema_version":1}', ""));
  check(r.ok === false && r.errors[0].code === "expected_bytes", `${fn}(string) -> expected_bytes: ${JSON.stringify(r)}`);
}

for (const name of names) {
  const { key, inner, payload } = payloadOf(name);
  const v = JSON.parse(m.validate(enc(payload), ""));
  check(v.ok === true && canon(v.payload[key]) === canon(JSON.parse(inner)), `${name}: validate output equals the app JSON`);
  const a = stripAck(m.strip, enc(payload), decide(enc(payload)));
  const b = stripAck(m.strip, enc(payload), decide(enc(payload)));
  check(a.ok && b.ok, `${name}: strip`);
  check(!JSON.stringify(a.payload).includes('"correct":true'), `${name}: no correct flag`);
  check(canon(a.match_hashes) === canon(b.match_hashes), `${name}: match hashes independent of the internal seed`);
  check(canon(JSON.parse(m.match_hash(enc(payload), "")).hashes) === canon(hashes[name].match_hash), `${name}: match_hash`);
  // full_hash is keyed (round 6): no key, no hash; a key gives a stable HMAC that another key does not.
  check(JSON.parse(m.full_hash(enc(payload), "")).errors?.[0]?.code === "hash_key_required", `${name}: full_hash needs a key`);
  const k1 = JSON.stringify({ hash_key: "ab".repeat(32) }), k2 = JSON.stringify({ hash_key: "cd".repeat(32) });
  const h1 = JSON.parse(m.full_hash(enc(payload), k1)).hashes;
  check(h1.length === hashes[name].full_hash.length && h1.every((h) => /^[0-9a-f]{64}$/.test(h)) && canon(h1) === canon(JSON.parse(m.full_hash(enc(payload), k1)).hashes) && canon(h1) !== canon(JSON.parse(m.full_hash(enc(payload), k2)).hashes) && canon(h1) !== canon(hashes[name].full_hash), `${name}: full_hash keyed`);
}
// The internal seed differs per call: a 26-way shuffle comes out in more than one order.
const big = JSON.stringify({ schema_version: 1, question: { id: "q", stem: "Pick", kind: { Mc: { answers: Array.from({ length: 26 }, (_, i) => ({ text: `planet ${i}`, correct: i === 0 })) } }, points: null, difficulty: null, created: 0, modified: 0 } });
const orders = new Set(Array.from({ length: 5 }, () => JSON.stringify(stripAck(m.strip, enc(big), "").payload)));
check(orders.size > 1, "strip draws a fresh seed each call");

const bad = (payload, code) => {
  const r = JSON.parse(m.validate(typeof payload === "string" ? enc(payload) : payload, ""));
  check(r.ok === false && r.errors[0].code === code, `refuse ${code}: ${JSON.stringify(r).slice(0, 200)}`);
  return r;
};
const tf = (stem) => JSON.stringify({ schema_version: 1, question: { id: "q", stem, kind: { Tf: { correct: true } }, points: null, difficulty: null, created: 0, modified: 0 } });
const r1 = bad(tf("a‮b"), "forbidden_char");
check(r1.errors[0].cp === "U+202E" && r1.errors[0].offset === 1, "forbidden_char carries cp and offset");
bad(tf("a⁦b"), "forbidden_char");
bad(tf("ㅤ"), "forbidden_char");
bad(tf(" ​ "), "blank_text");
bad(tf("x".repeat(8193)), "too_long");
bad('{"schema_version":1,"question":{"id":"q","id":"r"}}', "duplicate_key");
bad("[".repeat(100000), "too_deep");
// JSON.stringify writes a lone surrogate as the escape \ud800, which the strict reader refuses;
// raw bytes that are not UTF-8 are refused before parsing.
bad(tf("a\ud800b"), "invalid_json");
bad(new Uint8Array([0x7b, 0xff, 0x7d]), "invalid_utf8");
check(JSON.parse(m.validate(enc(tf("Count \u{1F468}‍\u{1F469}‍\u{1F467} ❤️")), "")).ok, "emoji ZWJ sequence and VS16 accepted");
check(JSON.parse(m.validate(enc(tf("می‌خواهم")), "")).ok, "Persian ZWNJ accepted");
const oor = JSON.parse(m.strip(enc(tf("x")), JSON.stringify({ image_ok: [99] })));
check(oor.ok === false && oor.errors[0].code === "image_ok_out_of_range", "image_ok out of range");
check(JSON.parse(m.validate_meta(enc('{"title":"T","tags":["X"]}'), "")).meta.tags[0] === "x", "meta");
const lf = JSON.parse(m.strip(enc(JSON.stringify({ schema_version: 1, question: { id: "q", stem: "Pick", kind: { Mc: { answers: ["A) Venus", "B) Mars"].map((t, i) => ({ text: t, correct: i === 0 })) } }, points: null, difficulty: null, created: 0, modified: 0 } })), ""));
check(lf.ok === false && lf.errors[0].code === "labels_need_decision" && lf.label_findings[0].choices[0].after === "Venus", "labels need a decision, with the preview");
check(JSON.parse(m.schema_versions()).stripped === 2, "schema_versions");
check(m.argon2id(enc("pw"), new Uint8Array(16), 8, 1, 1).length === 32, "argon2id still exported");
// Round 6: the author-only data travels separately, once; formatting findings and the order preview.
{
  const p = enc(JSON.stringify({ schema_version: 1, question: { id: "q", stem: "Pick", kind: { Mc: { answers: ["Venus", "Mars", "Earth"].map((t, i) => ({ text: t, correct: i === 0 })) } }, points: null, difficulty: null, created: 0, modified: 0 } }));
  const s = JSON.parse(m.strip(p, ""));
  check(s.ok === true && s.author_only === undefined && s.served_orders === undefined, "strip carries no author data");
  const a = JSON.parse(m.strip_author_data());
  check(a.ok === true && a.served_orders[0].order.length === 3, "strip_author_data hands it out");
  check(JSON.parse(m.strip_author_data()).errors[0].code === "no_author_data", "only once");
  const d = enc(JSON.stringify({ schema_version: 1, question: { id: "q", stem: "Pick", kind: { Mc: { answers: ["A. *Venus", "B. Mars", "C. Earth"].map((t, i) => ({ text: t, correct: i === 0 })) } }, points: null, difficulty: null, created: 0, modified: 0 } }));
  const ff = JSON.parse(m.formatting_findings(d, JSON.stringify({ labels: [{ question: 0, choice: "strip" }] })));
  check(ff.ok === true && ff.formatting_findings.some((x) => x.reason === "star" && x.served === "*Venus") && ff.order_preview[0].movable.length === 3, "formatting_findings judges the served form");
  const r = JSON.parse(m.strip(d, JSON.stringify({ labels: [{ question: 0, choice: "strip" }] })));
  check(r.ok === false && r.errors[0].code === "key_in_formatting" && r.errors[0].author_only === true, "a key behind a label is refused, author-only");
  // Round 7: one call for the Worker; the wrapper clears author data on every strip entry.
  const pp = JSON.parse(m.publish_prepare(p, JSON.stringify({ hide_key: true, hash_key: "ab".repeat(32) })));
  check(pp.ok === true && pp.stripped.ok === true && pp.full_hashes.length === 1 && pp.match_hashes[0] === pp.stripped.match_hashes[0], "publish_prepare");
  check(JSON.parse(m.strip_author_data()).ok === true, "publish_prepare leaves the author data");
  JSON.parse(m.strip(p, "")); JSON.parse(m.strip("notbytes", ""));
  check(JSON.parse(m.strip_author_data()).errors[0].code === "no_author_data", "expected_bytes strip clears the author data");
  JSON.parse(m.strip(p, "")); JSON.parse(m.strip(p, 5));
  check(JSON.parse(m.strip_author_data()).errors[0].code === "no_author_data", "bad_options strip clears the author data");
  // Round 8: every export entry forgets the last strip's author data.
  for (const [name, call] of [["validate", () => m.validate(p, "")], ["label_findings", () => m.label_findings(p, "")], ["formatting_findings", () => m.formatting_findings(p, "")], ["match_hash", () => m.match_hash(p, "")], ["schema_versions", () => m.schema_versions()], ["memory_bytes", () => m.memory_bytes()]]) {
    JSON.parse(m.publish_prepare(p, JSON.stringify({ hide_key: true })));
    call();
    check(JSON.parse(m.strip_author_data()).errors[0].code === "no_author_data", `${name} clears the author data`);
  }
  // Round 8: a refusal carries the findings and the order preview (no second call).
  const rf = JSON.parse(m.publish_prepare(enc(JSON.stringify({ schema_version: 2, question: { id: "q", stem: "Pick", kind: { Mc: { answers: ["Venus \u2713", "Mars", "Earth"].map((t, i) => ({ text: t, correct: i === 0, locked: false })) } }, points: null, difficulty: null, created: 0, modified: 0 } })), JSON.stringify({ hide_key: true })));
  check(rf.ok === false && rf.formatting_findings.some((x) => x.served === "Venus \u2713") && rf.order_preview[0].movable.length === 3 && typeof rf.errors[0].hint === "string", "publish_prepare refusal shape");
  let threw = false;
  try { check(JSON.parse(m.full_hash(p, JSON.stringify({ hash_key: "\u20ac".repeat(22) }))).errors[0].code === "hash_key_required", "multibyte key refused"); } catch (e) { threw = true; }
  check(!threw, "a multibyte key does not trap");
  check(JSON.parse(m.full_hash(p, JSON.stringify({ hash_key: "+f".repeat(32) }))).errors[0].code === "hash_key_required", "signed hex refused");
  check(JSON.parse(m.full_hash(p, JSON.stringify({ hash_key: "00".repeat(32) }))).errors[0].code === "hash_key_required", "all-zero key refused");
}

// ---- round 17 (critic31a, critic31b) ---------------------------------------------------------------
{
  const q = (stem, answers) => JSON.stringify({ schema_version: 2, question: { id: "q00000001", stem, points: null, difficulty: null, topics: [], created: 0, modified: 0,
    kind: { Mc: { answers: answers.map(([text, correct]) => ({ text, correct, locked: false })) } } } });
  const pp = (p, o = { hide_key: true }) => JSON.parse(m.publish_prepare(enc(p), JSON.stringify(o)));
  const bank = (qs, desc = "", name = "B") => JSON.stringify({ schema_version: 2, bank: { id: "b00000001", name, created: 0, modified: 0, details: { description: desc, license_name: "CC BY 4.0", license_url: "https://example.com/l" }, questions: qs } });
  const reasons = (r) => (r.errors || []).map((e) => e.reason);
  // B1: the window limits where a key starts, not where it ends.
  const long = "Because the light from the Sun is scattered by the molecules of the atmosphere, and the shorter blue wavelengths are scattered much more strongly than the longer red ones, so blue reaches our eyes from every direction of the sky";
  check(reasons(pp(q("Why is the sky blue?\nAnswer: probably " + long, [[long, true], ["short", false]]))).includes("answer_line"), "round 17: a 228-character key after a lead");
  const filler = "word ".repeat(39);
  check(reasons(pp(q("Which volcano?\nAnswer: " + filler + "Olympus Mons", [["Olympus Mons", true], ["Mauna Kea", false]]))).includes("answer_line"), "round 17: a key behind 195 characters");
  check(pp(q("Which volcano?\nAnswer: " + "word ".repeat(41) + "Olympus Mons", [["Olympus Mons", true], ["Mauna Kea", false]])).ok === true, "round 17: a key starting past the window is the documented limit");
  // A2: every served field, including a choice after a lead with no separator, and a topic token.
  const qb = (stem, answers) => JSON.parse(q(stem, answers)).question;
  const q1 = qb("Q1?", [["Mars", true], ["x", false]]);
  q1.id = "q00000001";
  const q2 = qb("Q2?", [["Answer Mars", false], ["y", true]]);
  q2.id = "q00000002";
  check(reasons(pp(bank([q1, q2]))).includes("answer_line"), "round 17: a choice names another question's key");
  const q3 = qb("Describe.", [["y", true], ["z", false]]);
  q3.id = "q00000003";
  q3.topics = ["Unit 4: Quartz"];
  const q4 = qb("Q4?", [["Quartz", true], ["x", false]]);
  q4.id = "q00000004";
  check(reasons(pp(bank([q3, q4]))).includes("topic_is_key"), "round 17: another question's key as a topic token");
  // A3: a key list in the description of a bank whose order students see.
  const mk = (correct) => { const x = qb("Pick.", [["Jurassic", false], ["Triassic", false], ["Cretaceous", false], ["Permian", false]].map((a, i) => [a[0], i === correct])); x.keep_order = true; return x; };
  check(reasons(pp(bank([mk(2), mk(1), mk(3)], "1 C 2 B 3 D"))).includes("bank_key_list"), "round 17: a frozen-order key list in the description");
  // A6: a missing answer-lead language.
  check(reasons(pp(q("පිළිතුර: Kestrel", [["Kestrel", true], ["Merlin", false]]))).includes("answer_line"), "round 17: the Sinhala answer lead");
  // B2: a formatting finding names the key's choice, so it is author-only.
  const ff = JSON.parse(m.formatting_findings(enc(q("Which planet is red?", [["Mars ✓", true], ["Venus", false]])), ""));
  check(ff.ok === true && ff.formatting_findings.length > 0 && ff.formatting_findings.every((x) => x.author_only === true), "round 17: formatting findings are author-only");
}

// ---- memory ceiling, each case in a fresh instance --------------------------------------------------
for (const mode of ["many_choices", "params", "nfc", "objs", "objs2", "strs", "kept_objects", "tiny_answers", "labels", "long_strings", "legit"]) {
  for (const fn of mode === "many_choices" ? ["validate", "strip", "label_findings", "validate_for_publish"] : ["validate", "strip"]) {
    const out = JSON.parse(execFileSync(process.execPath, [join(root, "tests", "node", "mem.mjs"), pkgDir, mode, fn], { encoding: "utf8" }));
    console.log(`  ${mode.padEnd(13)} ${fn.padEnd(8)} ${String(out.bytes).padStart(8)} B in  +${(out.growth / 1048576).toFixed(1).padStart(5)} MB  ${String(out.ms).padStart(5)} ms  ${out.ok ? "ok" : out.code}`);
    check(out.growth < CEILING, `${mode}/${fn}: memory growth ${out.growth} >= ${CEILING}`);
    check(out.alive, `${mode}/${fn}: instance still works`);
    if (mode === "legit") check(out.ok, "legit bank accepted");
  }
}

// ---- per-function cost bound over field shapes, and the CPU budget (round 5) -----------------------
// Growth of linear memory per input byte, for every exported payload function: at most 7x the input
// plus 2 MB (the input copy, the tree, the cleaned text, the output string) and under the ceiling.
// Time: every call under 3 s; the round-4 decoration shape (36 s through the Worker) under 3 s too.
const FIELD = ["f_long_stem", "f_cr_refused", "f_bom_refused", "f_long_choice", "f_long_expl", "f_refused_first", "f_refused_middle", "f_refused_last", "f_crlf", "f_bom", "f_nfc_expand", "f_marks", "f_many_small", "f_deco", "f_bank_initial", "f_mp_keys", "f_lead_lengths"];
for (const mode of FIELD) {
  const row = [];
  for (const fn of ["validate", "validate_for_publish", "strip", "label_findings", "match_hash", "full_hash"]) {
    const out = JSON.parse(execFileSync(process.execPath, [join(root, "tests", "node", "mem.mjs"), pkgDir, mode, fn], { encoding: "utf8" }));
    row.push(`${fn} ${(out.growth / out.bytes).toFixed(2)}x ${out.ms}ms`);
    const bound = Math.min(CEILING, 7 * out.bytes + 2 * 1048576);
    check(out.growth <= bound, `${mode}/${fn}: memory growth ${out.growth} > ${bound}`);
    check(out.ms < 3000, `${mode}/${fn}: ${out.ms} ms >= 3000 ms`);
    check(out.alive, `${mode}/${fn}: instance still works`);
  }
  console.log(`  ${mode.padEnd(16)} ${row.join("  ")}`);
}

// ---- round 14 (critic26b): no export traps on case-changing text; a trapped instance is replaced ----
{
  const T = ["0", "1", "100", "%", "(", ")", "-", ":", ".", "Answer", "correct", "True", "A.", "a)", "\u0130", "\u1E9E", "\u01C5", "\u0149", "\u00DF", "\u017F",
    "\u212A", "\u212B", "\u0390", "\uFB03", "\u1C90", "\u0131", "\u03C2", "e\u0301\u0323", "\u0130stanbul est", "\u1E9E ist", " est ", " is ", " es ", " \u00E8 ", "\u200D", "\u0DCA", "\u7B54\u6848", " "];
  let seed = 0x2545f491;
  const rnd = (n) => { seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5; return (seed >>> 0) % n; };
  const text = (k) => Array.from({ length: 1 + rnd(k) }, () => T[rnd(T.length)] + (rnd(3) ? "" : " ")).join("");
  let threw = 0;
  for (let i = 0; i < 1500; i++) {
    const q = { id: "q00000001", stem: text(8), points: 1, difficulty: null, topics: [text(3)], created: 0, modified: 0,
      kind: rnd(2) ? { Mc: { answers: [0, 1, 2].map((k) => ({ text: text(6), correct: k === 0, locked: false })) } } : { Matching: { pairs: [0, 1].map(() => ({ left: text(5), right: text(4) })), distractors: [text(4)] } } };
    const p = enc(JSON.stringify({ schema_version: 2, question: q }));
    for (const f of [() => m.publish_prepare(p, JSON.stringify({ hide_key: true, hash_key: "ab".repeat(32) })), () => m.validate_for_publish(p, '{"hide_key":true}'), () => m.formatting_findings(p, ""), () => m.strip(p, "")]) {
      try { f(); } catch (e) { threw++; m.reinit(); }
    }
  }
  check(threw === 0, `wasm fuzz: ${threw} exports threw on case-changing text`);
  check(m.health() === 1, "health() is 1 after the fuzz");
  // reinit: a fresh instance that works, argon2id included.
  m.reinit();
  check(m.health() === 1, "health() is 1 after reinit()");
  check(JSON.parse(m.validate(enc('{"schema_version":2,"question":{"id":"q00000001","stem":"s","kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"created":0,"modified":0}}'), "")).ok === true, "validate works after reinit()");
  check(m.argon2id(enc("pw"), enc("saltsalt"), 64, 1, 1).length === 32, "argon2id works after reinit()");
}

// ---- round 17 (critic31a B6): the 4 MB Blank bank's wasm memory ------------------------------------
{
  const out = JSON.parse(execFileSync(process.execPath, [join(root, "tests", "node", "mem.mjs"), pkgDir, "blank170", "validate_for_publish"], { encoding: "utf8" }));
  console.log(`  blank170        validate_for_publish ${String(out.bytes).padStart(8)} B in  +${(out.growth / 1048576).toFixed(1)} MB  ${String(out.ms).padStart(5)} ms  ${out.ok ? "ok" : out.code}`);
  check(out.growth < CEILING, `blank170: memory growth ${out.growth} >= ${CEILING}`);
  check(out.ok && out.alive, "blank170: the 4 MB Blank bank publishes");
}

// ---- test-seed build: byte-for-byte against the native goldens -------------------------------------
if (existsSync(join(testDir, "share_core.js"))) {
  const t = await import(join(testDir, "share_core.js"));
  t.initSync({ module: readFileSync(join(testDir, "share_core_bg.wasm")) });
  for (const name of names) {
    const { payload } = payloadOf(name);
    const s = stripAck(t.strip_with_seed, enc(payload), decide(enc(payload)), new Uint8Array(32).fill(7));
    const golden = JSON.parse(readFileSync(join(root, "fixtures", "stripped", `${name}.json`), "utf8"));
    check(s.ok === true && canon(s.payload) === canon(golden), `${name}: wasm strip_with_seed equals the native golden`);
  }
} else {
  console.log("(test-seed build not found; golden equality skipped)");
  failures++;
}

console.log(`wasm memory after checks: ${m.memory_bytes()} bytes`);
console.log(`${checks - failures}/${checks} checks passed`);
process.exit(failures ? 1 : 0);
