// Round 42: circles PHASE C, email invites (docs/circles-plan.md D5, 8.1). The owner invites by email: the server
// sends a FIXED text (only the circle name, re-cleaned, comes from a person) holding a normal single-use circle code,
// so a join still goes through POST /circles/join and every gate in front of it. The answer is the same whether or
// not the address has an account (no look-up of anyone's address), 20 a day per inviter, 3 mails a day per address
// across all circles and inviters (silently: the answer and the list do not change), a site-wide hourly budget, and
// only an HMAC of the address is kept (its own HKDF purpose), for at most two days.
import assert from "node:assert/strict";
import { test } from "node:test";
import { poolNotifier } from "../../src/poolmail.ts";
import { deleteContentStatements } from "../../src/pool/index.ts";
import type { PoolNotice } from "../../src/pool/shared.ts";
import { World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
type Mail = Extract<PoolNotice, { kind: "circle_email_invite" }>;
const mails = (w: World) => w.notices.filter((n) => n.kind === "circle_email_invite") as Mail[];
const invite = (w: World, u: User, cid: string, body: Q) => w.json<any>("POST", `/circles/${cid}/invites/email`, { user: u, json: body });
const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};
async function circleWith(w: World, owner: User, members: [User, "editor" | "viewer"][], name = "Algebra team"): Promise<string> {
  const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name } })).body.circle.id;
  for (const [u, role] of members) {
    const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role } })).body.code;
    assert.equal((await w.json("POST", "/circles/join", { user: u, json: { code: k } })).status, 200);
    w.advance(1000);
  }
  return cid;
}
/** Every text column of every table, as one string (a sweep for what must never be stored). */
function dump(w: World): string {
  const tables = w.q<{ name: string }>(`SELECT name FROM sqlite_master WHERE type = 'table'`).map((t) => t.name);
  return tables.map((t) => JSON.stringify(w.q(`SELECT * FROM "${t}"`))).join("\n");
}

test("42-1: the owner invites by email: 202 without the code, one fixed mail holding a normal single-use code, the code joins through the usual join (preview, role, used up), the list says kind email; neither the address nor the code is stored anywhere", async () => {
  const w = new World();
  const [O, Z] = ["Olga", "Zed"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, []);
  const r = await invite(w, O!, cid, { email: "  Zed.Teacher+circles@Zed-School.Example.ORG ", role: "editor" });
  assert.equal(r.status, 202, JSON.stringify(r.body));
  // (round 46: `emailed` says the mail went; `address_label` is the masked address the owner's list shows)
  assert.deepEqual(Object.keys(r.body).sort(), ["address_label", "emailed", "expires_at", "invite_id", "kind", "max_uses", "ok", "role"]);
  assert.deepEqual([r.body.emailed, r.body.address_label], [true, "z***@zed-school.example.org"]);
  assert.equal(r.body.kind, "email");
  assert.equal(r.body.max_uses, 1);
  assert.equal(r.body.expires_at, w.clock + 7 * DAY);
  const m = mails(w);
  assert.equal(m.length, 1);
  assert.equal(m[0]!.to, "zed.teacher+circles@zed-school.example.org", "trimmed and lower-cased, sent to the address as typed otherwise");
  assert.equal("circleName" in m[0]!, false, "round 46: the mail carries no circle name");
  assert.equal(m[0]!.role, "editor");
  assert.match(m[0]!.code, /^C(-[0-9A-Z]{4}){6}$/);
  const code = m[0]!.code;
  // The code is an ordinary circle code: preview, then join with the invite's role; it works once.
  const pv = await w.json<any>("POST", "/circles/join/preview", { user: Z!, json: { code } });
  assert.deepEqual([pv.status, pv.body.role, pv.body.circle.name], [200, "editor", "Algebra team"]);
  assert.deepEqual((await w.json<any>("POST", "/circles/join", { user: Z!, json: { code } })).body, { ok: true, circle_id: cid, role: "editor" });
  const P = w.user("teacher", "Pat");
  assert.equal((await w.json<any>("POST", "/circles/join", { user: P, json: { code } })).body.error, "circle_invite_invalid");
  // The owner's list: kind email, used up; never the code or the address.
  const list = await w.json<any>("GET", `/circles/${cid}/invites`, { user: O! });
  const row = list.body.invites.find((x: Q) => x.invite_id === r.body.invite_id);
  assert.deepEqual([row.kind, row.state, row.role, row.max_uses, row.note], ["email", "used_up", "editor", 1, ""]);
  const codeRow = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: O!, json: { role: "viewer" } })).body.invite_id;
  assert.equal((await w.json<any>("GET", `/circles/${cid}/invites`, { user: O! })).body.invites.find((x: Q) => x.invite_id === codeRow).kind, "code");
  const all = dump(w).toLowerCase();
  for (const s of ["zed.teacher", code.toLowerCase(), code.replaceAll("-", "").toLowerCase()]) assert.ok(!all.includes(s), `stored: ${s}`);
  // (round 46: the address's domain is stored only inside the invite's masked label)
  assert.equal(all.split("zed-school").length - 1, w.q(`SELECT 1 FROM circle_invites WHERE email_label LIKE '%zed-school%'`).length);
  // The HMAC kept for the per-address cap is not a plain hash of the address.
  const addr = w.q<{ addr: string; n: number }>(`SELECT addr, n FROM circle_email_sends`);
  assert.equal(addr.length, 1);
  const plain = Buffer.from(await crypto.subtle.digest("SHA-256", new TextEncoder().encode("zed.teacher+circles@zed-school.example.org"))).toString("hex");
  assert.notEqual(addr[0]!.addr, plain);
  // The export lists the invite (its kind), never the address.
  w.sql.prepare(`INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at, user_agent) VALUES (?, ?, ?, ?, ?, ?, ?, 'TestBrowser/1')`).run(`s-${O!.id}`, `fam-${O!.id}`, O!.id, `h-${O!.id}`, w.clock, w.clock, w.clock + 30 * DAY);
  const ex = await w.json<any>("GET", "/me/export", { user: O! });
  assert.equal(ex.status, 200, JSON.stringify(ex.body));
  assert.ok(ex.body.circles.invites_created.some((x: Q) => x.invite_id === r.body.invite_id && x.kind === "email"));
  assert.ok(!JSON.stringify(ex.body).toLowerCase().includes("zed.teacher"));
});

test("42-2: no account-exists oracle: the answer, the mail and the work are the same for an address with an account, without one, and for a member's own address; nobody's address is ever looked up", async () => {
  const w = new World();
  const [O, A] = ["Olga", "Ann"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[A!, "viewer"]]);
  const seen: string[] = [];
  w.stats.log = (sql) => void seen.push(sql);
  const shape = (b: Q) => Object.fromEntries(Object.entries(b).map(([k, v]) => [k, k === "invite_id" || k === "address_label" ? typeof v : v]));
  const answers: Q[] = [];
  const statements: number[] = [];
  for (const email of ["u2@school.example.org" /* Ann's */, "nobody-here@elsewhere.example", "u1@school.example.org" /* Olga's own */]) {
    const before = w.stats.statements;
    const r = await invite(w, O!, cid, { email, role: "viewer" });
    assert.equal(r.status, 202);
    answers.push(shape(r.body));
    statements.push(w.stats.statements - before);
  }
  assert.deepEqual(answers[0], answers[1]);
  assert.deepEqual(answers[1], answers[2]);
  assert.equal(new Set(statements).size, 1, `the same statements each time: ${statements}`);
  assert.equal(mails(w).length, 3, "every address is mailed the same way");
  assert.ok(!seen.some((s) => /\bemail\s*(=|IN\b|LIKE\b)/i.test(s)), "no statement looks anything up by an address");
});

test("42-3: caps: 20 email invites a day per inviter (429, nothing sent); 3 mails a day per address across circles and inviters, the rest silently not mailed (same answer, same list); the circle's open codes; a site-wide hourly budget (503, nothing kept); the next day it works again", async () => {
  const w = new World();
  const [O, O2] = ["Olga", "Omar"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, []);
  const cid2 = await circleWith(w, O2!, [], "Geometry team");
  // Per address: Olga twice, Omar once (case and a +tag do not make another address), then Omar again: round 46 (critic
  // 53a variant 1), the inviter is told plainly that no mail went (emailed: false, address_limit) and nothing is made.
  const target = ["Kim@Mail.Example", "kim@mail.example", "kim+x@mail.example", "KIM@mail.example"];
  const r: Q[] = [];
  for (const [k, e] of target.entries()) r.push((await invite(w, k < 2 ? O! : O2!, k < 2 ? cid : cid2, { email: e, role: "viewer" })).body);
  assert.ok(r.slice(0, 3).every((x) => x.ok === true && x.kind === "email" && x.emailed === true), JSON.stringify(r));
  assert.deepEqual([r[3]!.error, r[3]!.emailed, r[3]!.reason], ["invite_not_sent", false, "address_limit"]);
  assert.equal(mails(w).length, 3, "the fourth mail to one address in a day is not sent");
  const l2 = (await w.json<any>("GET", `/circles/${cid2}/invites`, { user: O2! })).body.invites;
  assert.deepEqual(l2.map((x: Q) => [x.kind, x.state]), [["email", "active"]], "nothing made for the mail that did not go");
  assert.equal(w.q(`SELECT 1 FROM circle_email_sends`).length, 1, "one per-address row (an HMAC of the cap key)");
  // The next day the address can be mailed again, and the counter rows of two days ago go with the daily cron.
  w.advance(DAY);
  assert.equal((await invite(w, O2!, cid2, { email: "kim@mail.example", role: "viewer" })).status, 202);
  assert.equal(mails(w).length, 4);
  w.advance(2 * DAY);
  await w.pool.daily();
  assert.equal(w.q(`SELECT 1 FROM circle_email_sends`).length, 0, "kept at most two days");
  // Per inviter: 20 a day (the circle's 10 open codes and, round 46, the inviter's 10 an hour are raised for this part).
  // (Round 47: the default is 40 a day; this account's own limit is set to 20, the mechanism is the same. The circle's
  // open email invitations and this inviter's share of one domain's hour are raised for this part too.)
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'circle_open_email_invites', 100, 0, 'test')`).run();
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'circle_email_invites_hourly', 100, 0, 'test')`).run(O!.id);
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'circle_email_invites', 20, 0, 'test')`).run(O!.id);
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'circle_email_inviter_domain_hourly', 100, 0, 'test')`).run(O!.id);
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'circle_email_domain_hourly', 100, 0, 'test')`).run();
  const before = mails(w).length;
  for (let i = 0; i < 20; i++) assert.equal((await invite(w, O!, cid, { email: `t${i}@mail.example`, role: "viewer" })).status, 202, `invite ${i}`);
  const over = await invite(w, O!, cid, { email: "t99@mail.example", role: "viewer" });
  assert.deepEqual([over.status, over.body.error, over.body.metric], [429, "daily_limit", "circle_email_invites"]);
  assert.equal(mails(w).length, before + 20);
  // (round 48, critic 55a V6: Olga's second invite to kim's mailbox above rotated the first one's code: one invite)
  assert.equal(w.q(`SELECT 1 FROM circle_invites WHERE circle_id = ? AND kind = 'email'`, cid).length, 21);
  // The circle's open codes: an email invite is a code. (Round 47: of its own kind, circle_open_email_invites, 40 by
  // default; the circle's 10 plain codes are apart.)
  w.advance(DAY);
  w.sql.prepare(`UPDATE pool_limits SET value = 21 WHERE user_id = '*' AND metric = 'circle_open_email_invites'`).run();
  const full = await invite(w, O!, cid, { email: "t100@mail.example", role: "viewer" });
  assert.deepEqual([full.status, full.body.error, full.body.metric], [422, "circles_limit", "circle_open_email_invites"]);
  // The site-wide hourly budget: past it 503 busy, nothing kept, nothing charged.
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'circle_email_hourly', 0, 0, 'test')`).run();
  w.sql.prepare(`UPDATE pool_limits SET value = 100 WHERE user_id = '*' AND metric = 'circle_open_email_invites'`).run();
  const n0 = w.q(`SELECT 1 FROM circle_invites`).length;
  const busy = await w.call("POST", `/circles/${cid}/invites/email`, { user: O!, json: { email: "t101@mail.example", role: "viewer" } });
  assert.equal(busy.status, 503);
  assert.equal(((await busy.json()) as Q).error, "busy");
  assert.ok(Number(busy.headers.get("Retry-After")) > 0);
  assert.equal(w.q(`SELECT 1 FROM circle_invites`).length, n0);
  const used = w.q<{ metric: string; count: number }>(`SELECT metric, count FROM usage_daily WHERE user_id = ? AND day = ?`, O!.id, Math.floor(w.clock / DAY));
  assert.equal(used.find((x) => x.metric === "circle_email_invites")?.count ?? 0, 0, `refusals give the day's count back: ${JSON.stringify(used)}`);
});

test("42-4: only the owner invites; a non-member is a 404; a closed circle 409; the address is checked strictly (header injection, look-alike or non-ASCII letters, spaces, length) and nothing but email and role is accepted, so the inviter adds no text to the mail", async () => {
  const w = new World();
  const [O, E, V, X] = ["Olga", "Eli", "Vic", "Xan"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, [[E!, "editor"], [V!, "viewer"]]);
  const ok = { email: "kim@mail.example", role: "viewer" };
  assert.deepEqual([(await invite(w, E!, cid, ok)).status, (await invite(w, E!, cid, ok)).body.error], [403, "circle_role_required"]);
  assert.equal((await invite(w, V!, cid, ok)).status, 403);
  assert.equal((await invite(w, X!, cid, ok)).status, 404);
  assert.equal((await invite(w, X!, "a".repeat(32), ok)).status, 404);
  for (const email of [
    "kim@mail.example\r\nBcc: victim@else.example",
    "kim@mail.example\nSubject: free",
    "kim@mail.example, victim@else.example",
    "kím@mail.example",
    "kim mail@mail.example",
    "kim@mail",
    "@mail.example",
    `${"k".repeat(65)}@mail.example`,
    `kim@${"m".repeat(250)}.example`,
    "",
    7,
    null,
  ]) {
    const r = await invite(w, O!, cid, { email, role: "viewer" });
    assert.deepEqual([r.status, r.body.field], [400, "email"], JSON.stringify(email));
  }
  for (const extra of [{ note: "Hi! Click here" }, { message: "join us" }, { name: "x" }, { circle_name: "Free prizes" }, { max_uses: 5 }]) {
    const r = await invite(w, O!, cid, { ...ok, ...extra });
    assert.equal(r.status, 400, JSON.stringify(extra));
  }
  assert.deepEqual([(await invite(w, O!, cid, { email: "kim@mail.example", role: "owner" })).body.field], ["role"]);
  assert.equal(mails(w).length, 0);
  // (round 46: a domain in another script is an IDN: it is mailed, and listed, in its ASCII (punycode) form, so a
  // look-alike shows as what it is)
  const idn = await invite(w, O!, cid, { email: "kim@mаil.example" /* Cyrillic a */, role: "viewer" });
  assert.deepEqual([idn.status, idn.body.address_label], [202, "k***@xn--mil-6cd.example"]);
  assert.equal(mails(w)[0]!.to, "kim@xn--mil-6cd.example");
  assert.equal((await w.json("DELETE", `/circles/${cid}`, { user: O! })).status, 200);
  assert.equal((await invite(w, O!, cid, ok)).body.error, "circle_closed");
  assert.equal(mails(w).length, 1, "only the IDN invite's");
});

test("42-5: no mail for an erased or suspended inviter: a request in flight when the account is erased writes nothing (401) and mails nothing; the erasure revokes the account's open email invites; a revoked email invite admits nobody", async () => {
  const w = new World();
  const [O, Z] = ["Olga", "Zed"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, []);
  const r = await invite(w, O!, cid, { email: "zed@mail.example", role: "viewer" });
  const code = mails(w)[0]!.code;
  assert.equal((await w.json<any>("POST", `/circles/${cid}/invites/revoke`, { user: O!, json: { invite_id: r.body.invite_id } })).status, 200);
  assert.equal((await w.json<any>("POST", "/circles/join", { user: Z!, json: { code } })).body.error, "circle_invite_invalid");
  // In flight: the account is erased just before the invite is written.
  let armed = true;
  w.stats.log = (sql) => {
    if (armed && /INSERT INTO circle_invites/.test(sql)) {
      armed = false;
      w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, O!.id);
    }
  };
  const n0 = mails(w).length;
  const inflight = await invite(w, O!, cid, { email: "pat@mail.example", role: "viewer" });
  assert.equal(inflight.status, 401);
  assert.equal(mails(w).length, n0);
  assert.equal(w.q(`SELECT 1 FROM circle_invites WHERE circle_id = ? AND kind = 'email' AND revoked_at IS NULL`, cid).length, 0);
  w.stats.log = undefined;
  // An erasure revokes what the account issued.
  const w2 = new World();
  const [O2] = ["Olga"].map((n) => w2.user("teacher", n));
  const cid2 = await circleWith(w2, O2!, []);
  await invite(w2, O2!, cid2, { email: "zed@mail.example", role: "viewer" });
  await erase(w2, O2!);
  assert.equal(w2.q(`SELECT 1 FROM circle_invites WHERE kind = 'email' AND revoked_at IS NULL`).length, 0);
  // A suspended account is not let in at all.
  const w3 = new World();
  const [O3] = ["Olga"].map((n) => w3.user("teacher", n));
  const cid3 = await circleWith(w3, O3!, []);
  w3.sql.prepare(`UPDATE users SET suspended_at = ? WHERE id = ?`).run(w3.clock, O3!.id);
  assert.equal((await invite(w3, O3!, cid3, { email: "zed@mail.example", role: "viewer" })).status, 401);
  assert.equal(mails(w3).length, 0);
});

test("42-6: the mail is a fixed text (MAIL_MODE log): a fixed subject, (round 46) no circle name and no inviter name at all, the code and a fragment link, the expiry; a hostile address or code is never put in a mail", async () => {
  const lines: string[] = [];
  const orig = console.log;
  console.log = (...a: unknown[]) => void lines.push(a.map(String).join(" "));
  const env = { DB: {} as D1Database, MAIL_MODE: "log", APP_URL: "http://localhost:5173" };
  const send = poolNotifier(env as never);
  const code = "C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01";
  try {
    await send({ kind: "circle_email_invite", to: "zed@mail.example", circleName: "Algebra   team", code, role: "editor", expiresAt: Date.UTC(2026, 9, 31, 12) });
    await send({ kind: "circle_email_invite", to: "zed@mail.example", circleName: "Evil\r\nBcc: victim@else.example", code, role: "viewer", expiresAt: Date.UTC(2026, 9, 31, 12) });
    await send({ kind: "circle_email_invite", to: "zed@mail.example\r\nBcc: victim@else.example", circleName: "Algebra team", code, role: "viewer", expiresAt: 0 });
    await send({ kind: "circle_email_invite", to: "zed@mail.example", circleName: "x".repeat(81), code, role: "viewer", expiresAt: Date.UTC(2026, 9, 31) });
    await send({ kind: "circle_email_invite", to: "zed@mail.example", circleName: "Algebra team", code: "not a code\nBcc: x", role: "viewer", expiresAt: 0 });
  } finally {
    console.log = orig;
  }
  const out = lines.join("\n");
  const mailsLogged = out.split("[dev mail]").slice(1);
  assert.equal(mailsLogged.length, 3, "the bad address and the bad code are never mailed");
  const [first, second, third] = mailsLogged as [string, string, string];
  assert.match(first, /^ to=zed@mail\.example subject=verSHion: you are invited to a circle\n/);
  assert.doesNotMatch(first, /Algebra/, "round 46: no circle name, whatever the notice carries");
  assert.match(first, /as an editor/);
  assert.match(first, new RegExp(`\\n${code}\\n`));
  assert.match(first, new RegExp(`http://localhost:5173/circles/join#code=${code}`));
  assert.match(first, /2026-10-31/);
  assert.doesNotMatch(first, /Olga|Teacher/);
  assert.ok(first.length < 1200, "short");
  assert.doesNotMatch(second, /Evil|Bcc|victim/, "no name");
  assert.match(second, /a circle on verSHion/);
  assert.doesNotMatch(third, /x{81}/);
  assert.ok(!out.includes("victim@else.example\n") && !/to=[^\n]*victim/.test(out));
  // Production mode: the provider gets one recipient and the fixed subject.
  const sent: Q[] = [];
  const origFetch = globalThis.fetch;
  globalThis.fetch = (async (_u: unknown, init?: { body?: string }) => {
    sent.push(JSON.parse(String(init?.body ?? "{}")));
    return new Response("{}", { status: 200 });
  }) as typeof fetch;
  try {
    await poolNotifier({ DB: {} as D1Database, RESEND_API_KEY: "k", MAIL_FROM: "pool@x.org", APP_URL: "https://app.example.org" } as never)({ kind: "circle_email_invite", to: "zed@mail.example", circleName: "Algebra team", code, role: "viewer", expiresAt: 0 });
  } finally {
    globalThis.fetch = origFetch;
  }
  assert.deepEqual(sent.map((x) => [x.to, x.subject]), [[["zed@mail.example"], "verSHion: you are invited to a circle"]]);
  assert.match(sent[0]!.text, /https:\/\/app\.example\.org\/circles\/join#code=/);
});

test("42-7: the HMAC is domain-separated and keyed: the per-address row is no plain hash of the address in any of its forms; the same address in another form (case, a +tag) maps to the same row", async () => {
  const w = new World();
  const [O] = ["Olga"].map((n) => w.user("teacher", n));
  const cid = await circleWith(w, O!, []);
  await invite(w, O!, cid, { email: "Zed@Mail.Example", role: "viewer" });
  await invite(w, O!, cid, { email: "zed+anything@mail.example", role: "viewer" });
  const rows = w.q<{ addr: string; n: number }>(`SELECT addr, n FROM circle_email_sends`);
  assert.deepEqual(rows.map((x) => x.n), [2]);
  assert.match(rows[0]!.addr, /^[A-Za-z0-9_-]{43}$/);
  for (const s of ["zed@mail.example", "user:zed@mail.example", "circle-invite-email/v1:zed@mail.example"]) {
    const plain = Buffer.from(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(s))).toString("base64url");
    assert.notEqual(rows[0]!.addr, plain);
  }
});
