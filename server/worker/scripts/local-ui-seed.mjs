// Seeds the LOCAL UI rig's pool (http://localhost:18791 only) with three sample shared items,
// published by a throwaway account b@example.test through the real API, the way the app does it:
// sign-up -> confirm with the link from the rig's mail log -> sign in -> approve ->
// POST /items (schema v2 bank, label decisions computed with share-core, attestation, licence).
//
// Run it through `scripts/local-ui-rig.sh seed`, which first checks that b@example.test does not
// exist yet (a second run says so and stops). This script also refuses if the address already has
// a confirm link in the mail log.
//
// The password is generated here and held only in this process: never printed, written, logged or
// passed on. Tokens (confirm link, access, refresh, device) likewise stay in memory. Nobody can sign
// in as B afterwards; the session is signed out at the end.
import { spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const BASE = "http://localhost:18791";
const EMAIL = "b@example.test";
const here = dirname(fileURLToPath(import.meta.url)); // server/worker/scripts
const RIG = join(here, "local-ui-rig.sh");
const PKG = join(here, "../../share-core/pkg");
const PUBLISH_CONTENT_TYPE = "application/vnd.vershion.publish";
const ATTEST_TEXT_REV = "2026-10-19";
const TERMS_VERSION = "2026-10-22";

const shareCore = await import(pathToFileURL(join(PKG, "share_core.js")).href);
shareCore.initSync({ module: readFileSync(join(PKG, "share_core_bg.wasm")) });

const die = (msg) => {
  console.error(`local-ui-seed: ${msg}`);
  process.exit(1);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---- requests (nothing secret is ever printed) ---------------------------------------------------
async function call(method, path, { json, body, headers = {}, token, native = true } = {}) {
  const h = { ...headers };
  if (native) h["X-Client"] = "native";
  if (token) h.Authorization = `Bearer ${token}`;
  if (json !== undefined) {
    body = JSON.stringify(json);
    h["Content-Type"] = "application/json";
  }
  const res = await fetch(BASE + path, { method, headers: h, body });
  const text = await res.text();
  let data = null;
  try {
    data = JSON.parse(text);
  } catch {}
  return { status: res.status, data };
}
const why = (r) => `${r.status} ${r.data?.error ?? ""}`.trim();

function rig(...args) {
  const p = spawnSync(RIG, args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  return { code: p.status, out: p.stdout ?? "", err: p.stderr ?? "" };
}
/** The token of the newest confirm link for EMAIL in the mail log, or null. Kept in memory only. */
function confirmToken() {
  const r = rig("links", EMAIL);
  const lines = r.out.split("\n");
  for (let i = 0; i < lines.length; i++) {
    if (/^confirm\s/.test(lines[i])) {
      const m = /#token=([A-Za-z0-9_\-.]+)/.exec(lines[i + 1] ?? "");
      if (m) return m[1];
    }
  }
  return null;
}

// ---- the sample banks ----------------------------------------------------------------------------
const T = 1792800000000;
let qn = 0;
const q = (stem, kind, extra = {}) => ({
  id: `seedq${++qn}`,
  stem,
  kind,
  points: 1,
  difficulty: null,
  topics: [],
  explanation: null,
  created: T + qn,
  modified: T + qn,
  ...extra,
});
const mc = (right, ...wrong) => ({
  Mc: { answers: [{ text: right, correct: true, locked: false }, ...wrong.map((text) => ({ text, correct: false, locked: false }))] },
});
const bank = (id, name, description, questions) => ({
  schema_version: 2,
  bank: { id, name, questions, created: T, modified: T, details: { description } },
});

const ITEMS = [
  {
    hide_key: true,
    license: "CC-BY-4.0",
    meta: { title: "Fractions and percentages quick check", subject: "Mathematics", grade: "6", tags: ["fractions", "percentages"] },
    payload: bank("seedbank1", "Fractions and percentages quick check", "Eight short multiple-choice questions for a starter or exit ticket on fractions, decimals and percentages.", [
      q("What is 3/4 written as a decimal?", mc("0.75", "0.34", "0.43", "7.5")),
      q("What is 25% of 80?", mc("20", "25", "40", "55")),
      q("Which fraction is equivalent to 2/3?", mc("8/12", "6/8", "4/9", "3/2")),
      q("What is 1/2 + 1/4?", mc("3/4", "2/6", "1/6", "2/4")),
      q("Write 0.4 as a percentage.", mc("40%", "4%", "0.4%", "400%")),
      q("Which of these is the largest?", mc("0.7", "2/3", "65%", "0.66")),
      q("What is 3/5 of 45?", mc("27", "15", "9", "30")),
      q("A jacket costs $60 and is reduced by 10%. What is the new price?", mc("$54", "$50", "$59", "$66")),
    ]),
  },
  {
    hide_key: true,
    license: "CC-BY-4.0",
    meta: { title: "Cells and energy review", subject: "Science", grade: "8", tags: ["cells", "energy"] },
    payload: bank("seedbank2", "Cells and energy review", "A short mixed review of plant and animal cells and energy transfer, with one matching and one short-answer question.", [
      q("Which part of a plant cell carries out photosynthesis?", mc("Chloroplast", "Nucleus", "Cell membrane", "Vacuole")),
      q("Which type of energy is stored in a stretched rubber band?", mc("Elastic potential energy", "Thermal energy", "Sound energy", "Light energy")),
      q("Match each cell part with its job.", {
        Matching: {
          pairs: [
            { left: "Nucleus", right: "Controls the cell's activities" },
            { left: "Cell membrane", right: "Controls what enters and leaves the cell" },
            { left: "Mitochondrion", right: "Releases energy in respiration" },
            { left: "Cell wall", right: "Supports and strengthens a plant cell" },
          ],
          distractors: ["Stores starch for winter"],
        },
      }, { points: 4 }),
      q("Explain why a metal spoon in hot soup feels hot to touch after a minute.", {
        Short: { sample: "Heat is conducted from the soup along the metal spoon, because metals are good thermal conductors." },
      }, { points: 2 }),
    ]),
  },
  {
    hide_key: false,
    license: "CC-BY-4.0",
    meta: { title: "World capitals warm-up", subject: "Geography", grade: "7", tags: ["capitals"] },
    payload: bank("seedbank3", "World capitals warm-up", "Five quick questions on capital cities, with the answer key included for self-marking.", [
      q("What is the capital city of Canada?", mc("Ottawa", "Toronto", "Vancouver", "Montreal")),
      q("What is the capital city of Australia?", mc("Canberra", "Sydney", "Melbourne", "Perth")),
      q("What is the capital city of Japan?", mc("Tokyo", "Osaka", "Kyoto", "Nagoya")),
      q("What is the capital city of Brazil?", mc("Brasilia", "Rio de Janeiro", "Sao Paulo", "Salvador")),
      q("Nairobi is the capital city of Kenya.", { Tf: { correct: true } }),
    ]),
  },
];

/** The label decisions the app would make (strip where allowed), from share-core. */
function decideLabels(payload) {
  const f = JSON.parse(shareCore.label_findings(new TextEncoder().encode(payload), ""));
  return (f.label_findings ?? []).map((x) =>
    x.part === undefined ? { question: x.question, choice: x.strip_allowed ? "strip" : "keep" } : { question: x.question, part: x.part, choice: x.strip_allowed ? "strip" : "keep" },
  );
}

async function publish(token, it) {
  const payload = JSON.stringify(it.payload);
  const meta = {
    publish_id: randomBytes(16).toString("base64url"),
    visibility: "public",
    hide_key: it.hide_key,
    license: it.license,
    attestation: { accepted: true, text_rev: ATTEST_TEXT_REV },
    attest: { own_work: true, terms_version: TERMS_VERSION },
    labels: it.hide_key ? decideLabels(payload) : [],
    order_acks: [],
    image_ok: [],
    ack_formatting: [],
    meta: { course: "", standards: [], ...it.meta },
  };
  const send = () =>
    call("POST", "/items", { token, body: Buffer.from(JSON.stringify(meta) + "\n" + payload), headers: { "Content-Type": PUBLISH_CONTENT_TYPE } });
  let r = await send();
  if (r.status === 422 && r.data?.error === "order_ack_required") {
    meta.order_acks = r.data.needs_order_ack;
    r = await send();
  }
  return r;
}

// ---- the run -------------------------------------------------------------------------------------
const up = await fetch(BASE + "/").then(
  () => true,
  () => false,
);
if (!up) die(`nothing answers on ${BASE}: run 'scripts/local-ui-rig.sh start' first`);
if (confirmToken() !== null) die(`${EMAIL} already has a confirm link in the mail log: it was seeded before (or signed up by hand). Nothing done.`);

const password = randomBytes(24).toString("base64url"); // in this process only
const profile = { display_name: "Sample Teacher B", school: "Example Middle School", adult_school_employee: true };

// Like the app's sign-up page: address, password and profile; the confirm step sets them again.
const su = await call("POST", "/auth/signup", { json: { email: EMAIL, password, ...profile } });
if (su.status !== 202) die(`sign-up refused: ${why(su)}`);
console.log(`Signed up ${EMAIL}.`);

let vtoken = null;
for (let i = 0; i < 40 && vtoken === null; i++) {
  vtoken = confirmToken();
  if (vtoken === null) await sleep(250);
}
if (vtoken === null) die("no confirm link appeared in the mail log");
const vr = await call("POST", "/auth/verify", { json: { token: vtoken, password, ...profile } });
vtoken = null;
if (vr.status !== 200 || vr.data?.password_set !== true) die(`confirm refused: ${why(vr)}`);
const device = vr.data.device_token ?? null;
console.log("Confirmed with the link from the mail log; password, name and school set.");

const login = async () => {
  const r = await call("POST", "/auth/login", { json: { email: EMAIL, password }, headers: device ? { "X-Device-Token": device } : {} });
  if (r.status !== 200 || !r.data?.access_token) die(`sign-in refused: ${why(r)}`);
  return r.data;
};
let session = await login();
console.log(`Signed in (trust ${session.user?.trust}).`);

const ap = rig("approve", EMAIL);
if (ap.code !== 0) die(`approve failed: ${ap.err.trim() || ap.out.trim()}`);
console.log(ap.out.trim().split("\n").pop());

// The access token carries the trust it was issued with: end that session and sign in again so
// the new one says teacher.
await call("POST", "/auth/logout", { json: { refresh_token: session.refresh_token }, token: session.access_token });
session = await login();
const token = session.access_token;
const mineBefore = await call("GET", "/items/mine", { token });
if (mineBefore.status !== 200) die(`pool not open to ${EMAIL}: ${why(mineBefore)}`);

const made = [];
for (const it of ITEMS) {
  const r = await publish(token, it);
  if ((r.status !== 200 && r.status !== 201) || !r.data?.item_id) die(`publish of "${it.meta.title}" refused: ${why(r)} ${JSON.stringify(r.data?.issues ?? r.data?.reason ?? "")}`);
  made.push({ id: r.data.item_id, title: it.meta.title });
  console.log(`Published ${r.data.item_id}  ${it.meta.title}  (${it.hide_key ? "key hidden" : "key shown"}, ${it.license}, ${r.data.qids?.length ?? "?"} questions)`);
}

const pub = await call("GET", "/items?limit=50", { token });
const mine = await call("GET", "/items/mine", { token });
const listed = (pub.data?.items ?? []).filter((x) => made.some((m) => m.id === x.id));
const mineItems = mine.data?.items ?? [];
console.log(`GET /items: ${pub.status}, ${listed.length} of the ${made.length} new items listed publicly.`);
console.log(`GET /items/mine: ${mine.status}, ${mineItems.length} items for ${EMAIL}:`);
for (const x of mineItems) console.log(`  ${x.id}  ${x.title}  visibility=${x.visibility} hide_key=${x.hide_key} license=${x.license} questions=${x.question_count}`);

// End the session and forget the password; B is not signed into again.
const out = await call("POST", "/auth/logout", { json: { refresh_token: session.refresh_token, forget_device: true }, token });
console.log(`Signed out: ${out.status}.`);
if (mineItems.length !== made.length) die(`expected ${made.length} items in /items/mine, saw ${mineItems.length}`);
