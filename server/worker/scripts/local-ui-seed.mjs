// Seeds the LOCAL UI rig's pool (http://localhost:18791 only) with three sample shared items and
// one circle, made by a throwaway account b@example.test through the real API, the way the app
// does it: sign-up -> confirm with the link from the rig's mail log -> sign in -> approve ->
// POST /items (schema v2 bank, label decisions computed with share-core, attestation, licence),
// then POST /circles ("Maths department"), two banks sent into it (POST /circles/:id/banks, the
// same publish body with an own-work attestation and a licence) and two invite codes (one editor,
// one viewer), which are printed. Circle codes are not passwords: they are meant to be given out.
//
// Run it through `scripts/local-ui-rig.sh seed`. A first run (no b@example.test yet) does all of
// the above. Later runs (`--existing`, passed by the rig script) publish no public items again:
// they sign B in through the forgot-password link in the mail log (with a new random password),
// make the circle and its banks only if they are missing, withdraw the unused codes an earlier
// seed made and print two fresh ones. A first run refuses if the address already has a confirm
// link in the mail log.
//
// The password is generated here and held only in this process: never printed, written, logged or
// passed on. Tokens (confirm or reset link, access, refresh, device) likewise stay in memory.
// Nobody can sign in as B afterwards; the session is signed out at the end.
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
/** The token of the newest `kind` link ("confirm" or "reset") for EMAIL in the mail log, or null. Kept in memory only. */
function linkToken(kind) {
  const r = rig("links", EMAIL);
  const lines = r.out.split("\n");
  for (let i = 0; i < lines.length; i++) {
    if (new RegExp(`^${kind}\\s`).test(lines[i])) {
      const m = /#token=([A-Za-z0-9_\-.]+)/.exec(lines[i + 1] ?? "");
      if (m) return m[1];
    }
  }
  return null;
}
const confirmToken = () => linkToken("confirm");

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

// ---- the circle -----------------------------------------------------------------------------------
const CIRCLE = {
  name: "Maths department",
  description: "Local test circle made by 'rig seed': two banks from Sample Teacher B, to open, copy and add to.",
};
/** Written on the codes this script makes, so a later run can withdraw the ones nobody used. */
const CODE_NOTE = "rig seed";
const CIRCLE_BANKS = [
  {
    license: "CC-BY-4.0",
    message: "First version, for the start of the unit.",
    meta: { title: "Linear equations starter", subject: "Mathematics", grade: "8", tags: ["algebra", "equations"] },
    payload: bank("seedcircle1", "Linear equations starter", "Six warm-up questions on solving one- and two-step linear equations.", [
      q("Solve x + 7 = 12.", mc("x = 5", "x = 19", "x = -5", "x = 7")),
      q("Solve 3x = 21.", mc("x = 7", "x = 18", "x = 24", "x = 63")),
      q("Solve 2x - 4 = 10.", mc("x = 7", "x = 3", "x = 12", "x = 14")),
      q("Which value of y makes y / 4 = 3 true?", mc("12", "7", "3/4", "1")),
      q("x = 2 is a solution of 5x - 1 = 9.", { Tf: { correct: true } }),
      q("Explain the first step you would take to solve 4(x + 1) = 20, and why.", {
        Short: { sample: "Divide both sides by 4 (or expand the bracket), so that x + 1 = 5; then subtract 1 to get x = 4." },
      }, { points: 2 }),
    ]),
  },
  {
    license: "circle-only",
    meta: { title: "Angles exit ticket", subject: "Mathematics", grade: "7", tags: ["geometry", "angles"] },
    payload: bank("seedcircle2", "Angles exit ticket", "Four questions on angle facts, kept inside the circle (members may not share it publicly).", [
      q("Two angles on a straight line are 110 degrees and x. What is x?", mc("70 degrees", "110 degrees", "250 degrees", "90 degrees")),
      q("The angles of a triangle are 50, 60 and x degrees. What is x?", mc("70 degrees", "80 degrees", "110 degrees", "60 degrees")),
      q("Match each angle with its type.", {
        Matching: {
          pairs: [
            { left: "35 degrees", right: "Acute" },
            { left: "90 degrees", right: "Right" },
            { left: "140 degrees", right: "Obtuse" },
          ],
          distractors: ["Straight"],
        },
      }, { points: 3 }),
      q("Vertically opposite angles are always equal.", { Tf: { correct: true } }),
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

/** A circle bank's first version, as the app's "Share to a circle" sends it. */
async function sendToCircle(token, cid, it) {
  const payload = JSON.stringify(it.payload);
  const meta = {
    commit_id: randomBytes(16).toString("base64url"),
    license: it.license,
    attest: { own_work: true, terms_version: TERMS_VERSION },
    ...(it.message ? { message: it.message } : {}),
    meta: it.meta,
  };
  return call("POST", `/circles/${cid}/banks`, { token, body: Buffer.from(JSON.stringify(meta) + "\n" + payload), headers: { "Content-Type": PUBLISH_CONTENT_TYPE } });
}


/**
 * Brings "Linear equations starter" to three versions (so the history, a merge and "N new versions"
 * can be tried): version 2 rewords one question and adds one, version 3 rewords another. Each goes
 * over the head the way the app's "Send my changes" does (published.qid keeps each question's id).
 */
async function ensureThreeVersions(token, cid, bid) {
  for (let guard = 0; guard < 3; guard++) {
    const doc = await call("GET", `/circles/${cid}/banks/${bid}`, { token });
    if (doc.status !== 200 || !doc.data?.payload?.bank) die(`reading the circle bank ${bid} refused: ${why(doc)}`);
    const head = doc.data.bank ?? {};
    const version = head.version ?? 1;
    if (version >= 3) return version;
    const bank = doc.data.payload.bank;
    const vs = await call("GET", `/circles/${cid}/banks/${bid}/versions`, { token });
    const line = (vs.data?.versions ?? []).find((v) => v.version === version);
    if (!line?.version_id) die(`reading the history of ${bid} refused: ${why(vs)}`);
    const qs = bank.questions.map((x) => ({ ...x, published: { item_id: bid, qid: x.id, publish_id: line.version_id, version } }));
    let message;
    if (version === 1) {
      qs[2].stem = "Solve 2x - 4 = 10. Show each step.";
      const extra = JSON.parse(JSON.stringify(qs[1]));
      delete extra.published;
      extra.id = "seedcircle-extra";
      extra.stem = "Solve 5x = 35.";
      qs.push(extra);
      message = "Reworded question 3 and added one more.";
    } else {
      qs[0].stem = "Solve x + 7 = 12 for x.";
      message = "Clearer wording on question 1.";
    }
    const meta = {
      commit_id: randomBytes(16).toString("base64url"),
      license: head.license ?? "CC-BY-4.0",
      attest: { own_work: true, terms_version: TERMS_VERSION },
      base_version: version,
      message,
      meta: { title: head.title ?? "Linear equations starter", subject: head.subject ?? "Mathematics", grade: head.grade ?? "8", tags: ["algebra", "equations"] },
    };
    const payload = JSON.stringify({ schema_version: 2, bank: { ...bank, questions: qs } });
    const r = await call("PUT", `/circles/${cid}/banks/${bid}`, { token, body: Buffer.from(JSON.stringify(meta) + "\n" + payload), headers: { "Content-Type": PUBLISH_CONTENT_TYPE } });
    if (r.status !== 200 && r.status !== 201) die(`sending version ${version + 1} of ${bid} refused: ${why(r)} ${JSON.stringify(r.data?.errors ?? r.data?.message ?? "")}`);
    console.log(`Sent version ${r.data.version} of "${meta.meta.title}": ${message}`);
  }
  return 3;
}

/** The circle (made if missing), its two banks (sent if missing), and two fresh codes, printed. */
async function seedCircle(token) {
  const list = await call("GET", "/circles", { token });
  if (list.status !== 200) die(`GET /circles refused: ${why(list)}`);
  let circle = (list.data?.circles ?? []).find((c) => c.name === CIRCLE.name && c.role === "owner" && c.closed_at == null);
  if (circle) {
    const n = (k, one) => `${k} ${one}${k === 1 ? "" : "s"}`;
    console.log(`Circle "${CIRCLE.name}" already exists (${circle.id}, ${n(circle.members, "member")}, ${n(circle.banks, "bank")}).`);
  } else {
    const r = await call("POST", "/circles", { token, json: CIRCLE });
    if (r.status !== 201 || !r.data?.circle?.id) die(`creating the circle refused: ${why(r)} ${r.data?.message ?? ""}`);
    circle = r.data.circle;
    console.log(`Created circle "${CIRCLE.name}" (${circle.id}).`);
  }
  const cid = circle.id;

  const detail = await call("GET", `/circles/${cid}`, { token });
  if (detail.status !== 200) die(`GET /circles/${cid} refused: ${why(detail)}`);
  const have = new Set((detail.data?.banks ?? []).filter((b) => !b.removed && b.is_author).map((b) => b.title));
  for (const it of CIRCLE_BANKS) {
    if (have.has(it.meta.title)) {
      console.log(`Circle bank "${it.meta.title}" already there.`);
      continue;
    }
    const r = await sendToCircle(token, cid, it);
    if ((r.status !== 200 && r.status !== 201) || !r.data?.bank_id) die(`sending "${it.meta.title}" to the circle refused: ${why(r)} ${JSON.stringify(r.data?.errors ?? r.data?.message ?? "")}`);
    console.log(`Sent ${r.data.bank_id}  ${it.meta.title}  (version ${r.data.version}, ${it.license}, ${it.payload.bank.questions.length} questions)`);
  }

  const starter = ((await call("GET", `/circles/${cid}`, { token })).data?.banks ?? []).find((b) => !b.removed && b.is_author && b.title === CIRCLE_BANKS[0].meta.title);
  if (starter) await ensureThreeVersions(token, cid, starter.id);

  // Codes are shown only when made, so earlier unused seed codes are withdrawn and two new ones made.
  const inv = await call("GET", `/circles/${cid}/invites`, { token });
  if (inv.status !== 200) die(`GET /circles/${cid}/invites refused: ${why(inv)}`);
  let withdrawn = 0;
  for (const i of inv.data?.invites ?? []) {
    if (i.state !== "active" || i.note !== CODE_NOTE) continue;
    const r = await call("POST", `/circles/${cid}/invites/revoke`, { token, json: { invite_id: i.invite_id } });
    if (r.status !== 200) die(`withdrawing an old seed code refused: ${why(r)}`);
    withdrawn++;
  }
  if (withdrawn > 0) console.log(`Withdrew ${withdrawn} unused code${withdrawn === 1 ? "" : "s"} from an earlier seed.`);
  const codes = {};
  for (const role of ["editor", "viewer"]) {
    const r = await call("POST", `/circles/${cid}/invites`, { token, json: { role, note: CODE_NOTE } });
    if (r.status !== 201 || !r.data?.code) die(`making the ${role} code refused: ${why(r)} ${r.data?.message ?? ""}`);
    codes[role] = r.data;
  }
  const after = await call("GET", `/circles/${cid}`, { token });
  const banks = (after.data?.banks ?? []).filter((b) => !b.removed);
  console.log(`GET /circles/${cid}: ${after.status}, ${after.data?.members?.length ?? "?"} member(s), ${banks.length} bank(s): ${banks.map((b) => `"${b.title}" (${b.license})`).join(", ")}.`);
  const days = (ms) => Math.round((ms - Date.now()) / 86_400_000);
  console.log("");
  console.log(`Invite codes for the circle "${CIRCLE.name}" (each works once, for ${days(codes.editor.expires_at)} days):`);
  console.log(`  editor (can add banks):          ${codes.editor.code}`);
  console.log(`  viewer (can only look and copy): ${codes.viewer.code}`);
  console.log("Redeem one as a@example.test in the app: Shared, Circles tab, Enter a code..., paste it, Look up, Join.");
  console.log("These codes are not passwords. 'rig seed' again withdraws them (if unused) and prints new ones.");
}

// ---- the run -------------------------------------------------------------------------------------
const existing = process.argv[2] === "--existing";
const up = await fetch(BASE + "/").then(
  () => true,
  () => false,
);
if (!up) die(`nothing answers on ${BASE}: run 'scripts/local-ui-rig.sh start' first`);

const password = randomBytes(24).toString("base64url"); // in this process only
const profile = { display_name: "Sample Teacher B", school: "Example Middle School", adult_school_employee: true };

if (existing) {
  // B was seeded before and its password was never kept: take the account back the way its owner
  // would, with a reset link from the mail log, and a new random password held only here.
  const before = linkToken("reset");
  const fg = await call("POST", "/auth/forgot", { json: { email: EMAIL } });
  if (fg.status !== 202) die(`forgot-password refused: ${why(fg)}`);
  let rtoken = null;
  for (let i = 0; i < 40 && rtoken === null; i++) {
    const t = linkToken("reset");
    if (t !== null && t !== before) rtoken = t;
    else await sleep(250);
  }
  if (rtoken === null) die(`no new reset link for ${EMAIL} in the mail log. One goes out at most every 2 minutes (10 an hour): run 'rig seed' again in two minutes.`);
  const rs = await call("POST", "/auth/reset", { json: { token: rtoken, password } });
  rtoken = null;
  if (rs.status !== 200) die(`reset refused: ${why(rs)}`);
  const device = rs.data?.device_token ?? null;
  const li = await call("POST", "/auth/login", { json: { email: EMAIL, password }, headers: device ? { "X-Device-Token": device } : {} });
  if (li.status !== 200 || !li.data?.access_token) die(`sign-in refused: ${why(li)}`);
  if (li.data.user?.trust !== "teacher") die(`${EMAIL} is ${li.data.user?.trust}, not an approved teacher: run 'rig approve ${EMAIL}' (or 'rig reset')`);
  console.log(`Signed in as ${EMAIL} with a reset link from the mail log (the public items are not published again).`);
  await seedCircle(li.data.access_token);
  const out = await call("POST", "/auth/logout", { json: { refresh_token: li.data.refresh_token, forget_device: true }, token: li.data.access_token });
  console.log(`Signed out: ${out.status}.`);
  process.exit(0);
}

if (confirmToken() !== null) die(`${EMAIL} already has a confirm link in the mail log: it was seeded before (or signed up by hand). Nothing done.`);

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

await seedCircle(token);

// End the session and forget the password; B is not signed into again.
const out = await call("POST", "/auth/logout", { json: { refresh_token: session.refresh_token, forget_device: true }, token });
console.log(`Signed out: ${out.status}.`);
if (mineItems.length !== made.length) die(`expected ${made.length} items in /items/mine, saw ${mineItems.length}`);
