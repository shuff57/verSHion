// Critic 58 B1 (docs/critic-reports/2026-10-25-circles-phaseC-integration-critic58.md): D1 refuses a LIKE or GLOB
// pattern longer than 50 bytes, so the account-deletion batch (its `LIKE 'circle_email_domain:%:' || ?1`, 58 bytes)
// failed on D1 for every account, and the cron's erasure_finish failed the same way each day, while node:sqlite (limit
// 50000) passed it. The harness's d1() now enforces D1's limit (bound, literal and concatenated patterns), and a source
// scan holds every LIKE/GLOB pattern the worker sends to one SQL literal within the limit, which catches the class even
// when no test happens to evaluate the operator on a row.
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { d1, D1_MAX_LIKE_PATTERN_BYTES, World, type User } from "./harness.ts";

const DAY = 86_400_000;
const ID = "0123456789abcdef0123456789abcdef0123"; // a 36-character user id
const TOO_COMPLEX = /LIKE or GLOB pattern too complex/;

test("D1 shim: a LIKE or GLOB pattern over 50 bytes is refused (bound, literal, concatenated, NOT, ESCAPE) when evaluated; 50 bytes passes; results are SQLite's; raw test queries are unaffected", async () => {
  const sql = new DatabaseSync(":memory:");
  sql.exec(`CREATE TABLE t (c TEXT, n INTEGER)`);
  const db = d1(sql);
  const long = `circle_email_domain:%:${ID}`;
  assert.equal(Buffer.byteLength(long), 58);
  // Like D1, the operator is only evaluated on a row: an empty table answers.
  assert.equal((await db.prepare(`DELETE FROM t WHERE c LIKE 'circle_email_domain:%:' || ?1`).bind(ID).run()).meta.changes, 0);
  sql.exec(`INSERT INTO t VALUES ('Abc_def', 5), ('x', 12), ('é', 1)`);
  for (const [s, args] of [
    [`DELETE FROM t WHERE c LIKE 'circle_email_domain:%:' || ?1`, [ID]],
    [`SELECT c FROM t WHERE c LIKE ?1`, [long]],
    [`SELECT c FROM t WHERE c NOT LIKE ?1`, [long]],
    [`SELECT c FROM t WHERE c LIKE ?1 ESCAPE '!'`, [long]],
    [`SELECT c FROM t WHERE c GLOB ?1 || ?2`, ["*".repeat(15), ID]],
    [`SELECT c FROM t WHERE c LIKE '${"a".repeat(51)}'`, []],
    [`SELECT c FROM t WHERE c GLOB '${"é".repeat(26)}'`, []], // 52 bytes in 26 characters: bytes count
  ] as [string, unknown[]][])
    await assert.rejects(db.prepare(s).bind(...args).all(), TOO_COMPLEX, s);
  // A refused statement in a batch rolls the whole batch back, as on D1.
  await assert.rejects(db.batch([db.prepare(`DELETE FROM t WHERE c = 'x'`), db.prepare(`DELETE FROM t WHERE c LIKE ?1`).bind(long)]), TOO_COMPLEX);
  assert.equal((sql.prepare(`SELECT COUNT(*) AS n FROM t`).get() as { n: number }).n, 3);
  // At the limit, and SQLite's own answers (case-insensitive LIKE, case-sensitive GLOB, ESCAPE, numbers, NULL).
  assert.deepEqual(await db.prepare(`SELECT ?1 LIKE ?1 AS r`).bind("a".repeat(D1_MAX_LIKE_PATTERN_BYTES)).first(), { r: 1 });
  const col = async (s: string, ...a: unknown[]) => (await db.prepare(s).bind(...a).all<{ c: unknown }>()).results.map((x) => x.c);
  assert.deepEqual(await col(`SELECT c FROM t WHERE c LIKE 'abc%'`), ["Abc_def"]);
  assert.deepEqual(await col(`SELECT c FROM t WHERE c LIKE 'abc!_%' ESCAPE '!'`), ["Abc_def"]);
  assert.deepEqual(await col(`SELECT c FROM t WHERE c LIKE 'abc!x%' ESCAPE '!'`), []);
  assert.deepEqual(await col(`SELECT c FROM t WHERE c GLOB 'A*'`), ["Abc_def"]);
  assert.deepEqual(await col(`SELECT c FROM t WHERE c GLOB 'a*'`), []);
  assert.deepEqual(await col(`SELECT n AS c FROM t WHERE n LIKE '1%' ORDER BY n`), [1, 12]);
  assert.deepEqual(await col(`SELECT c FROM t WHERE c NOT LIKE ?1 ORDER BY c`, "x"), ["Abc_def", "é"]);
  assert.deepEqual(await db.prepare(`SELECT NULL LIKE 'a' AS r`).first(), { r: null });
  // The test's own queries on the database (not D1) keep SQLite's default limit.
  assert.equal((sql.prepare(`SELECT ?1 LIKE ?1 AS r`).get(long) as { r: number }).r, 1);
});

const invite = (w: World, u: User, cid: string, email: string) => w.json<any>("POST", `/circles/${cid}/invites/email`, { user: u, json: { email, role: "viewer" } });
const tombstone = (w: World, u: User) =>
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);

test("critic 58 B1: account deletion commits with the inviter's hourly domain rows present (memberships end, the circle is handed on, its email invites are revoked, only its own rows go); the cron finishes an erasure whose batch had failed", async () => {
  const w = new World();
  const [O, E, O2, P] = ["Olga", "Eli", "Omar", "Pia"].map((n) => w.user("teacher", n)) as User[];
  const circle = async (owner: User, editor?: User) => {
    const cid = (await w.json<any>("POST", "/circles", { user: owner, json: { name: "Algebra team" } })).body.circle.id as string;
    if (editor) {
      const k = (await w.json<any>("POST", `/circles/${cid}/invites`, { user: owner, json: { role: "editor" } })).body.code;
      assert.equal((await w.json("POST", "/circles/join", { user: editor, json: { code: k } })).status, 200);
    }
    return cid;
  };
  const cid = await circle(O!, E!);
  const c2 = await circle(O2!);
  const c3 = await circle(P!);
  for (const [u, c, email] of [[O, cid, "t1@pvhs.example"], [O2, c2, "t2@pvhs.example"], [P, c3, "t3@pvhs.example"]] as [User, string, string][])
    assert.equal((await invite(w, u, c, email)).status, 202);
  const rows = (u: User) => w.q(`SELECT channel FROM pool_alert_budget WHERE instr(channel, ?) > 0`, u.id).length;
  const domainTotal = () => w.q(`SELECT 1 FROM pool_alert_budget WHERE channel GLOB 'circle_email_domain:*' AND channel NOT GLOB 'circle_email_domain:*:*'`).length;
  assert.deepEqual([rows(O!), rows(O2!), rows(P!), domainTotal()], [2, 2, 2, 1], "each inviter's own count and domain share, one domain total");

  tombstone(w, O!);
  await w.db.batch(deleteContentStatements(w.db, O!.id, w.clock));
  assert.equal(w.q(`SELECT 1 FROM erasure_done WHERE user_id = ?`, O!.id).length, 1);
  assert.equal(w.q(`SELECT 1 FROM circle_members WHERE user_id = ?`, O!.id).length, 0, "its memberships end");
  assert.deepEqual(w.q(`SELECT user_id, role FROM circle_members WHERE circle_id = ? AND left_at IS NULL`, cid), [{ user_id: E!.id, role: "owner" }], "the circle is handed on");
  assert.deepEqual(
    w.q(`SELECT revoked_at IS NOT NULL AS r, email_label, email_key FROM circle_invites WHERE created_by = ? AND kind = 'email'`, O!.id),
    [{ r: 1, email_label: null, email_key: null }],
    "its email invite is revoked and forgets the address",
  );
  assert.deepEqual([rows(O!), rows(O2!), rows(P!), domainTotal()], [0, 2, 2, 1], "only the erased inviter's rows go");

  // An erasure whose batch never committed (what the long LIKE did on D1 every time) is finished by the daily cron.
  tombstone(w, P!);
  w.advance(DAY);
  await w.pool.hourly();
  await w.pool.daily();
  assert.equal(w.q(`SELECT 1 FROM erasure_done WHERE user_id = ?`, P!.id).length, 1, "erasure_finish commits");
  assert.equal(w.q(`SELECT 1 FROM circle_members WHERE user_id = ?`, P!.id).length, 0);
  assert.equal(rows(P!), 0);
});

// ---- every LIKE/GLOB the worker sends: one SQL literal of at most 50 bytes ------------------------------------------

const root = new URL("../..", import.meta.url).pathname;
const walk = (dir: string): string[] =>
  readdirSync(dir).flatMap((f) => (statSync(`${dir}/${f}`).isDirectory() ? walk(`${dir}/${f}`) : [`${dir}/${f}`]));

/** Each LIKE/GLOB operator in `text` (comment lines skipped): its line and its pattern literal, or why it is not one. */
function likePatterns(text: string): { line: number; pattern?: string; problem?: string }[] {
  const out: { line: number; pattern?: string; problem?: string }[] = [];
  const lines = text.split("\n");
  lines.forEach((l, i) => {
    const t = l.trim();
    if (t.startsWith("//") || t.startsWith("*") || t.startsWith("/*") || t.startsWith("--")) return;
    if (/\b(?:like|glob)\s*\(/.test(l)) out.push({ line: i + 1, problem: "like()/glob() called as a function" });
    for (const m of l.matchAll(/\b(LIKE|GLOB)\b\s*/g)) {
      const rest = l.slice(m.index! + m[0].length);
      const lit = /^'((?:[^']|'')*)'/.exec(rest);
      if (!lit) {
        out.push({ line: i + 1, problem: `${m[1]} pattern is not a string literal: ${rest.slice(0, 40)}` });
        continue;
      }
      const pattern = lit[1]!.replace(/''/g, "'");
      const after = rest.slice(lit[0].length).trimStart();
      if (pattern.includes("${")) out.push({ line: i + 1, problem: `${m[1]} pattern interpolates a value: '${pattern}'` });
      else if (after.startsWith("||")) out.push({ line: i + 1, problem: `${m[1]} pattern is concatenated: '${pattern}' ${after.slice(0, 20)}` });
      else out.push({ line: i + 1, pattern });
    }
  });
  return out;
}

test("every LIKE/GLOB pattern in src/ and migrations/ is one SQL string literal of at most 50 bytes (D1's limit; no bound, interpolated or concatenated patterns)", () => {
  // The scanner itself: the pattern of B1 and its kin are caught.
  assert.deepEqual(
    likePatterns("x LIKE 'circle_email_domain:%:' || ?1\nc LIKE ?1\nc GLOB 'a*'\n// LIKE ?1 in a comment\nWHERE k NOT LIKE 'it''s%'").map((x) => x.problem ?? x.pattern),
    ["LIKE pattern is concatenated: 'circle_email_domain:%:' || ?1", "LIKE pattern is not a string literal: ?1", "a*", "it's%"],
  );
  const files = [...walk(`${root}src`).filter((f) => f.endsWith(".ts")), ...walk(`${root}migrations`).filter((f) => f.endsWith(".sql"))];
  const bad: string[] = [];
  let seen = 0;
  for (const f of files)
    for (const p of likePatterns(readFileSync(f, "utf8"))) {
      seen++;
      const at = `${f.slice(root.length)}:${p.line}`;
      if (p.problem) bad.push(`${at}: ${p.problem}`);
      else if (Buffer.byteLength(p.pattern!) > D1_MAX_LIKE_PATTERN_BYTES) bad.push(`${at}: pattern of ${Buffer.byteLength(p.pattern!)} bytes`);
    }
  assert.ok(seen >= 20, `the scan found the worker's LIKE/GLOB uses (${seen})`);
  assert.deepEqual(bad, []);
});
