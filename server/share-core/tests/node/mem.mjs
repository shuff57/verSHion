// One payload through a fresh wasm instance; prints {mode, bytes, growth, ms, code} as JSON.
// Usage: node tests/node/mem.mjs <pkg dir> <mode> [validate|strip]
// The modes are the critics' hostile inputs (scratchpad critic11b/node/mem.mjs) plus the legit bank.
import { readFileSync } from "node:fs";
const [pkg, mode, fn = "strip"] = process.argv.slice(2);
const m = await import(pkg + "/share_core.js");
m.initSync({ module: readFileSync(pkg + "/share_core_bg.wasm") });
const mem0 = m.memory_bytes();
let p;
const bank = (qs) => `{"schema_version":1,"bank":{"id":"b","name":"n","created":1,"modified":2,"questions":[${qs.join(",")}]}}`;
if (mode === "many_choices") {
  const L = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
  const part = (k) => `{"stem":"p","kind":{"Mc":{"answers":[${Array.from({ length: 26 }, (_, j) => `{"text":"${L[j]}. w${k}${j}${"x".repeat(25)}"${j === 0 ? ',"correct":true' : ""}}`).join(",")}]}}}`;
  p = bank(Array.from({ length: 41 }, (_, i) => `{"id":"q${i}","stem":"s","kind":{"Multipart":{"parts":[${Array.from({ length: 50 }, (_, k) => part(k)).join(",")}]}},"points":null,"difficulty":null,"created":1,"modified":1}`));
} else if (mode === "params") {
  const ps = Array.from({ length: 180 }, (_, k) => `"p${k}":"xxxxxx"`).join(",");
  p = bank(Array.from({ length: 1000 }, (_, i) => `{"id":"q${i}","stem":"s","kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"created":1,"modified":1,"figure":{"id":"f","params":{${ps}}}}`));
} else if (mode === "nfc") {
  const N = String.fromCodePoint(0x1d160);
  const qs = [];
  let len = 200;
  for (let i = 0; i < 1000; i++) {
    const ans = Array.from({ length: 26 }, (_, j) => `{"text":"${N.repeat(169)}${String.fromCharCode(97 + j)}","correct":${j === 0}}`).join(",");
    const s = `{"id":"q${i}","stem":"${N.repeat(682)}","explanation":"${N.repeat(682)}","kind":{"Mc":{"answers":[${ans}]}},"points":null,"difficulty":null,"created":1,"modified":1}`;
    const n = new TextEncoder().encode(s).length;
    if (len + n > 4_190_000) break;
    len += n + 1;
    qs.push(s);
  }
  p = bank(qs);
} else if (mode === "objs") {
  p = `{"schema_version":1,"question":{"x":[${'{"":0},'.repeat(248_999)}{"":0}]}}`;
} else if (mode === "objs2") {
  const unit = '{"":{"":{"":{"":{"":{"":{"":{"":{"":{"":{"":{"":0}}}}}}}}}}}},';
  p = `{"schema_version":1,"question":{"x":[${unit.repeat(Math.floor(499_000 / 13))}0]}}`;
} else if (mode === "strs") {
  p = `{"schema_version":1,"question":{"x":[${'"",'.repeat(498_999)}""]}}`;
} else if (mode === "kept_objects") {
  const parts = Array(50).fill('{"stem":"p","kind":{"Tf":{"correct":true}}}').join(",");
  p = bank(Array.from({ length: 1000 }, (_, i) => `{"id":"q${i}","stem":"s","kind":{"Multipart":{"parts":[${parts}]}},"points":null,"difficulty":null,"created":1,"modified":1}`));
} else if (mode === "tiny_answers") {
  const ans = Array(26).fill('{"text":"a"}').join(",");
  p = bank(Array.from({ length: 1000 }, (_, i) => `{"id":"q${i}","stem":"s","kind":{"Mc":{"answers":[${ans}]}},"points":null,"difficulty":null,"created":1,"modified":1}`));
} else if (mode === "labels") {
  const lab = "A. ".repeat(640), lab2 = "B. ".repeat(640);
  p = bank(Array.from({ length: 1000 }, (_, i) => `{"id":"q${i}","stem":"s","kind":{"Mc":{"answers":[{"text":"${lab}x","correct":true},{"text":"${lab2}y"}]}},"points":null,"difficulty":null,"created":1,"modified":1}`));
} else if (mode === "long_strings") {
  p = bank(Array.from({ length: 1000 }, (_, i) => `{"id":"q${i}","stem":"${"s".repeat(4000)}","kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"created":1,"modified":1}`));
} else if (mode.startsWith("f_")) {
  // Field shapes (the same as `field_shapes` in tests/memory.rs) and the formatting-check CPU shape.
  const one = (stem, kind, extra = "") => `{"schema_version":1,"question":{"id":"q","stem":${stem},"kind":${kind},"points":null,"difficulty":null,"created":1,"modified":1${extra}}}`;
  const tf = '{"Tf":{"correct":true}}';
  const big = 4 * 1048576 - 400;
  const js = JSON.stringify;
  const fill = (stem) => {
    const qs = [];
    let len = 200;
    for (let i = 0; i < 1000; i++) {
      const q = `{"id":"q${i}","stem":${stem(i)},"kind":${tf},"points":null,"difficulty":null,"created":1,"modified":1}`;
      const n = new TextEncoder().encode(q).length;
      if (len + n + 1 > 4 * 1048576 - 100) break;
      len += n + 1;
      qs.push(q);
    }
    return bank(qs);
  };
  const atLimit = (pos) => () => {
    const a = "a".repeat(8 * 1024 - 3);
    const k = [0, Math.floor(a.length / 2), a.length][pos];
    return js(a.slice(0, k) + "\u202e" + a.slice(k));
  };
  const ans = (t) => `{"Mc":{"answers":[{"text":${js(t)},"correct":true},{"text":"b"}]}}`;
  const shapes = {
    f_long_stem: () => one(js("a".repeat(big)), tf),
    f_cr_refused: () => one(js("\r" + "a".repeat(big - 8) + "\u202e"), tf),
    f_bom_refused: () => one(js("\ufeff" + "a".repeat(big - 8) + "\u202e"), tf),
    f_long_choice: () => one('"s"', ans("a".repeat(big))),
    f_long_expl: () => one('"s"', tf, `,"explanation":${js("a".repeat(big))}`),
    f_refused_first: () => fill(atLimit(0)),
    f_refused_middle: () => fill(atLimit(1)),
    f_refused_last: () => fill(atLimit(2)),
    f_crlf: () => fill(() => js("\r\n".repeat(4 * 1024 - 2))),
    f_bom: () => fill(() => js("\ufeff".repeat(2700) + "x")),
    f_nfc_expand: () => fill(() => js(String.fromCodePoint(0x1d160).repeat(2000))),
    f_marks: () => fill(() => js(("a" + "\u0301".repeat(16)).repeat(240))),
    // Round 16 (critic28b B1): 99 short samples "a.a....b" at every odd length 3..199, then
    // True/False stems of 64 answer leads over "a." runs: the lead scan walks a trie, budgeted.
    f_lead_lengths: () => {
      const qs = [];
      for (let l = 3; l < 200; l += 2) qs.push(`{"id":"s${l}","stem":"Explain.","kind":{"Short":{"sample":${js("a.".repeat((l - 1) / 2) + "b")}}},"points":null,"difficulty":null,"created":1,"modified":1}`);
      const stem = js("Statement. " + Array.from({ length: 64 }, () => "answer:" + "a.".repeat(27)).join("") + "a.".repeat(100));
      for (let i = 0; i < 880; i++) qs.push(`{"id":"t${i}","stem":${stem},"kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"created":1,"modified":1}`);
      return bank(qs);
    },
    // Round 13 (critic24b B1): 4,096 accepted answers starting "q" in 64 Blank questions, and
    // keyless stems of 33 answer lines each: the bank-level scan looks keys up, never scans them.
    f_bank_initial: () => {
      const tag = (i) => { let s = ""; for (let k = 0; k < 3; k++) { s += String.fromCharCode(97 + (i % 26)); i = Math.floor(i / 26); } return s; };
      const qs = [];
      let ki = 0;
      for (let b = 0; b < 64; b++) {
        const accept = Array.from({ length: 4 }, () => Array.from({ length: 16 }, () => "q-q-q-q-q-q-q-q-q-" + tag(ki++)));
        qs.push(`{"id":"k${b}","stem":"Fill: ___ and ___ and ___ and ___.","kind":{"Blank":{"accept":${js(accept)}}},"points":null,"difficulty":null,"created":1,"modified":1}`);
      }
      const stem = js("Explain.\n" + ("answer: " + "q-".repeat(30) + "\n").repeat(56));
      for (let i = 0; i < 1000 - 64; i++) qs.push(`{"id":"s${i}","stem":${stem},"keyless":true,"kind":{"Short":{"sample":""}},"points":null,"difficulty":null,"created":1,"modified":1}`);
      return bank(qs).replace('"schema_version":1', '"schema_version":2');
    },
    // Round 13 (critic24b B2): one Multipart of 16 Mc parts per question, each part stem holding
    // the other parts' 240 key texts, padded to 8 KB.
    f_mp_keys: () => {
      const tag = (i) => { let s = ""; for (let k = 0; k < 3; k++) { s += String.fromCharCode(97 + (i % 26)); i = Math.floor(i / 26); } return s; };
      const keys = Array.from({ length: 16 }, (_, j) => Array.from({ length: 26 }, (_, k) => "w" + tag(j * 26 + k)));
      const parts = keys.map((ks, j) => {
        let stem = "Given " + keys.flatMap((o, jj) => jj === j ? [] : o.slice(0, 16)).join(" ") + " ";
        stem += ".".repeat(Math.max(0, 8000 - stem.length));
        return `{"stem":${js(stem)},"kind":{"Mc":{"answers":[${ks.map((t, k) => `{"text":"${t}","correct":${k < 16}}`).join(",")}]}}}`;
      }).join(",");
      const q = (i) => `{"id":"m${i}","stem":"A problem.","kind":{"Multipart":{"parts":[${parts}]}},"points":null,"difficulty":null,"created":1,"modified":1}`;
      const qs = [];
      let len = 200;
      for (let i = 0; i < 1000; i++) { const s = q(i); if (len + s.length > 4_000_000) break; len += s.length + 1; qs.push(s); }
      return bank(qs);
    },
    f_many_small: () => {
      const a = Array.from({ length: 26 }, (_, j) => `{"text":"choice ${j} ${"x".repeat(80)}","correct":${j === 0}}`).join(",");
      return bank(Array.from({ length: 1000 }, (_, i) => `{"id":"q${i}","stem":"s","kind":{"Mc":{"answers":[${a}]}},"points":null,"difficulty":null,"created":1,"modified":1}`));
    },
    // The decoration shape that took 36 s in round 4: 77 questions of 26 choices, each choice a
    // 1,021-character run of "*_" around a word.
    f_deco: () => bank(Array.from({ length: 77 }, (_, i) => `{"id":"q${i}","stem":"s","kind":{"Mc":{"answers":[${Array.from({ length: 26 }, (_, j) => `{"text":${js("*_".repeat(255) + "w" + j + "*_".repeat(255) + "*")},"correct":${j === 0}}`).join(",")}]}},"points":null,"difficulty":null,"created":1,"modified":1}`)),
  };
  p = shapes[mode]();
} else if (mode === "blank170") {
  // (Round 17, critic31a B6: 170 Blank x 50 blanks x 20 distinct variants, about 4 MB.)
  const acc = Array.from({ length: 50 }, (_, b) => `[${Array.from({ length: 20 }, (_, k) => `"ka${b}${k} lele ka lele"`).join(",")}]`).join(",");
  p = bank(Array.from({ length: 170 }, (_, i) => `{"id":"q${i}","stem":"${"___ ".repeat(50)}","kind":{"Blank":{"accept":[${acc}]}},"points":null,"difficulty":null,"created":1,"modified":1}`));
} else if (mode === "legit") {
  p = JSON.stringify({
    schema_version: 1,
    bank: {
      id: "b", name: "Big", created: 0, modified: 0,
      questions: Array.from({ length: 1000 }, (_, i) => ({
        id: `q${i}`, stem: "S".repeat(600), points: 1, difficulty: null, created: 0, modified: 0, explanation: "E".repeat(400),
        kind: { Mc: { answers: Array.from({ length: 26 }, (_, j) => ({ text: `planet ${j} ${"x".repeat(60)}`, correct: j === 0, locked: false })) } },
      })),
    },
  });
}
const bytes = new TextEncoder().encode(p);
const t = performance.now();
// The author's decisions (Keep for every label finding) come from the preview call, inside the
// measurement: the ceiling covers the whole session.
let opts = "";
if (fn === "strip") {
  const f = JSON.parse(m.label_findings(bytes, ""));
  if (f.ok) opts = JSON.stringify({ labels: f.label_findings.map((x) => ({ question: x.question, part: x.part ?? undefined, choice: "keep" })) });
}
const call = {
  strip: () => m.strip(bytes, opts),
  validate: () => m.validate(bytes, ""),
  label_findings: () => m.label_findings(bytes, ""),
  validate_for_publish: () => m.validate_for_publish(bytes, '{"hide_key":true}'),
  match_hash: () => m.match_hash(bytes, ""),
  full_hash: () => m.full_hash(bytes, JSON.stringify({ hash_key: "ab".repeat(32) })),
};
const r = JSON.parse(call[fn]());
const ms = performance.now() - t;
// The instance must still work afterwards.
const again = JSON.parse(m.validate(new TextEncoder().encode('{"schema_version":1}'), ""));
console.log(JSON.stringify({ mode, fn, bytes: bytes.length, growth: m.memory_bytes() - mem0, ms: Math.round(ms), ok: r.ok, code: r.ok ? null : r.errors[0].code, alive: again.ok === false }));
