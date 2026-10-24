// Round 31 (staging D1 insights): the earlier-publication lookup (publish.ts) averaged 26.5 s and 21
// million rows when two teachers published the same 990-question bank: each position walked every
// question holding any of its near hashes, so a hash many questions share (a bank holding a question many
// times, or one stem and choices with different explanations) made it positions x questions. Now each
// hash is followed once per chunk (EARLIER_FIRSTS_SQL) and each position's own test runs on its few
// candidates (EARLIER_SQL). The hourly licence re-key tested every `q:` row against every question of its
// item; it now finds its rows from the questions' side.
// (a) The old statements (oracles.ts) are the oracle: random banks with many duplicates, ties, erased
//     authors, holds, her own earlier versions (her "firsts") and additions, hidden and shown.
// (b) Rows visited and plans at 250 and 990 questions with a near-identical earlier bank.
import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { EARLIER_FIRSTS_SQL, EARLIER_SQL, earlierCandidates, LICENCE_REKEY_ROWS, licenceRekey } from "../../src/pool/publish.ts";
import { publish, rng, World, type Rng, type User } from "./harness.ts";
import { EARLIER_SQL_R29, EARLIER_SQL_R30, licenceRekeyR30, withoutCandidates } from "./oracles.ts";

type Q = Record<string, any>;
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs.map((x, i) => ({ ...x, id: `q${i}` })), created: 1, modified: 1 } });
const IS_EARLIER = /^\s*SELECT CAST\(j\.value ->> '\$\.i' AS INTEGER\) AS pos,/;
const IS_FIRSTS = (sql: string) => sql === EARLIER_FIRSTS_SQL;

const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};

// ---- (a) the earlier-publication lookup against the old statements --------------------------------

/** A question from a small pool of templates, each with stem, choice, answer and explanation variants. */
function variant(r: Rng, t: number): Q {
  const stem = `Template ${t} asks which of the listed field observations ${r.bool(0.2) ? "best supports" : "supports"} the river delta hypothesis here?`;
  const correct = r.int(2);
  const answers = ["Sediment fans out at the mouth", "Water runs uphill in spring", "Salt marsh retreats inland", `Choice ${r.int(2)} about tides`].map((text, j) => ({ text, correct: j === correct, locked: false }));
  const ex = r.int(4);
  return { stem, kind: { Mc: { answers } }, points: null, difficulty: null, topics: [], image: null, explanation: ex === 0 ? null : `Explanation ${ex} for template ${t}: sediment load and slope.`, figure: null, created: 1, modified: 1 };
}

test("31 (a1): the earlier-publication lookup over each position's candidates answers as the round-30 and round-29 forms did, on random pools (duplicates, ties, erased authors, holds, her earlier versions)", async () => {
  let compared = 0, named = 0, firsts = 0, added = 0;
  for (let seed = 0; seed < Number(process.env.POOL_PROPERTY_SEEDS ?? 8); seed++) {
    const r = rng(31000 + seed);
    const w = new World();
    const us = Array.from({ length: 5 }, (_, i) => w.user("teacher", `T${i}`));
    const items: { id: string; pub: string; by: User }[] = [];
    // Every lookup is compared when it runs, on the same database as the production statement.
    w.stats.log = (sql, args) => {
      if (!IS_EARLIER.test(sql)) return;
      const now = w.q(sql, ...args);
      const old = [args[0], withoutCandidates(args[1] as string)];
      assert.deepEqual(now, w.q(EARLIER_SQL_R30, ...old), `seed ${seed}: round-30 form`);
      assert.deepEqual(now, w.q(EARLIER_SQL_R29, ...old), `seed ${seed}: round-29 form`);
      const ps = JSON.parse(old[1] as string) as { first: number | null; loose: number | null; expl: string | null }[];
      firsts += ps.filter((p) => p.first !== null).length;
      added += ps.filter((p) => p.loose !== null && p.loose !== p.first).length;
      named += now.filter((x) => x.m !== null).length;
      compared++;
    };
    for (let step = 0; step < 14; step++) {
      const u = r.pick(us);
      const pick = r.int(10);
      if (pick === 0 && items.length) {
        // An earlier publisher is erased; an item someone imported stays, anonymised ("deleted teacher").
        const it = r.pick(items);
        if (it.by !== u) await w.json("POST", `/items/${it.id}/import`, { user: u });
        await erase(w, it.by);
      } else if (pick === 1 && items.length) {
        w.sql.prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) VALUES (?, 'admin', ?, NULL, 'manual')`).run(r.pick(items).id, w.clock);
      } else {
        const own = items.filter((x) => x.by === u);
        const n = 3 + r.int(10);
        // (many duplicates: few templates, and now and then the same question twice in a bank)
        const qs = Array.from({ length: n }, () => variant(r, r.int(6)));
        if (r.bool(0.4)) qs.push({ ...qs[0]! });
        const re = own.length > 0 && r.bool(0.4) ? r.pick(own) : null;
        const res = re ? await publish(w, u, bankOf(qs), { hideKey: r.bool(0.4), base: re.pub }, re.id) : await publish(w, u, bankOf(qs), { hideKey: r.bool(0.4), license: "CC-BY-4.0" });
        if (res.status === 200 || res.status === 201) {
          if (re) re.pub = res.body.publish_id as string;
          else items.push({ id: res.body.item_id as string, pub: res.body.publish_id as string, by: u });
        }
      }
      w.advance(3_600_000);
    }
    w.stats.log = undefined;
  }
  const counts = `${compared} lookups, ${named} earlier publications named, ${firsts} positions with her own first, ${added} with a looser first`;
  if (process.env.POOL_ROUND31_REPORT === "1") console.log(counts);
  assert.ok(compared >= 40 && named >= 40 && firsts >= 40 && added >= 5, counts);
});

test("31 (a2): a position's answer can be a source behind the first one (it shares her added explanation): the candidates include it", async () => {
  // Ann publishes the question; Bob publishes it later with a typo fixed; Cy, later still, with the fix,
  // Ann's explanation and another correct choice (the same key-free content only); Ann then fixes the
  // typo and adds that explanation herself. Her first of this content is unknown, her looser first is
  // her first version. Bob's, the first source, lacks what she added and is not before it: Cy's is the
  // one named, as before, though it is the first question of none of her hashes.
  const w = new World();
  const [A, B, C, D] = [w.user("teacher", "Ann"), w.user("teacher", "Bob"), w.user("teacher", "Cy"), w.user("teacher", "Dee")];
  const base = (expl: string | null, correct = 0, fixed = true): Q => ({ stem: `Which of the listed field observations best supports the delta hypothesis in the ${fixed ? "Feather" : "Fether"} river survey?`, kind: { Mc: { answers: ["Sediment fans out", "Water runs uphill", "Marsh retreats"].map((text, j) => ({ text, correct: j === correct, locked: false })) } }, points: null, difficulty: null, topics: [], image: null, explanation: expl, figure: null, created: 1, modified: 1 });
  const E = "Ann's explanation: the sediment load drops where the slope flattens, so the fan grows seaward.";
  const seen: { sql: string; args: unknown[] }[] = [];
  w.stats.log = (sql, args) => {
    if (IS_EARLIER.test(sql)) {
      const old = [args[0], withoutCandidates(args[1] as string)];
      assert.deepEqual(w.q(sql, ...args), w.q(EARLIER_SQL_R30, ...old));
      seen.push({ sql, args: [...args] });
    }
  };
  const a = (await publish(w, A, bankOf([base(null, 0, false)]))).body as Q;
  w.advance(3_600_000);
  await publish(w, B, bankOf([base(null)]));
  w.advance(3_600_000);
  await publish(w, C, bankOf([base(E, 1)]));
  w.advance(3_600_000);
  const a2 = await publish(w, A, bankOf([base(E)]), { base: a.publish_id }, a.item_id);
  assert.equal(a2.status, 200);
  w.advance(3_600_000);
  await publish(w, D, bankOf([base(E)]));
  w.stats.log = undefined;
  assert.ok(seen.length >= 4);
  const ann = seen[3]!;
  assert.match(String(w.q<{ m: string }>(ann.sql, ...ann.args)[0]!.m), /"author_name":"Cy"/, "Ann's lookup names Cy's");
  // Somewhere a lookup's answer was not the first question of its hashes: without the explanation
  // candidates it would differ (the reason they are there).
  let behind = 0;
  for (const s of seen) {
    const ps = JSON.parse(s.args[1] as string) as Q[];
    const firstOnly = await earlierCandidates({ db: w.db } as never, s.args[0] as string, ps.map((p) => ({ n: p.n as string[], expl: null, ans: null })));
    const narrow = JSON.stringify(ps.map((p, i) => ({ ...p, c: firstOnly[i]!.c })));
    if (JSON.stringify(w.q(s.sql, s.args[0], narrow)) !== JSON.stringify(w.q(EARLIER_SQL_R30, s.args[0], withoutCandidates(s.args[1] as string)))) behind++;
  }
  assert.ok(behind >= 1, "a source behind the first one was named");
});

test("31 (a4): adversarial rows and positions: near hashes shared across columns, hidden and shown, own rows, held and erased authors' items, NULL keys, ties on seq and lineage; every first / looser-first / added-key combination", async () => {
  const w = new World();
  const us = Array.from({ length: 4 }, (_, i) => w.user("teacher", `U${i}`));
  const ids: string[] = [];
  const r0 = rng(31400);
  for (const [k, u] of [...us, ...us].entries()) {
    const p = await publish(w, u, bankOf(Array.from({ length: 12 }, () => variant(r0, r0.int(6)))), { hideKey: k % 3 === 1 });
    if (p.status === 201) ids.push(p.body.item_id as string);
    w.advance(3_600_000);
  }
  await w.json("POST", `/items/${ids[0]}/import`, { user: us[1]! });
  await erase(w, us[0]!); // her imported item stays, anonymised; the others go
  w.sql.prepare(`INSERT INTO item_holds (item_id, reason, at, by, source) VALUES (?, 'admin', 1, NULL, 'manual')`).run(ids[2]);
  const c = { db: w.db } as never;
  const H = ["h1", "h2", "h3", "h4", "h5"], K = ["e:1", "e:2", "e:3"], A = ["a:1", "a:2"];
  const qids = w.q<{ rowid: number }>(`SELECT rowid FROM item_questions`).map((x) => x.rowid);
  let compared = 0, named = 0;
  for (let round = 0; round < 80; round++) {
    const r = rng(31500 + round);
    const maybe = <T>(xs: T[], p = 0.3) => (r.bool(p) ? null : r.pick(xs));
    w.sql.exec("SAVEPOINT a4");
    try {
      const up = w.sql.prepare(`UPDATE item_questions SET near_full = ?, near_stripped = ?, near_kf = ?, near_bare_full = ?, near_bare_stripped = ?, bk_expl = ?, bk_ans = ?, seq = ?, lineage_seq = ? WHERE rowid = ?`);
      for (const id of qids) up.run(maybe(H, 0.5), maybe(H, 0.6), maybe(H, 0.5), maybe(H, 0.6), maybe(H, 0.6), maybe(K, 0.4), maybe(A, 0.5), 1 + r.int(5), r.bool(0.2) ? null : 1 + r.int(3), id);
      const ps = Array.from({ length: 12 }, (_, i) => {
        const first = r.bool(0.4) ? null : 1 + r.int(6);
        const ownLoose = r.bool(0.5) ? null : 1 + r.int(6);
        const expl = maybe(K), ans = maybe(A);
        return {
          i,
          n: [...new Set(Array.from({ length: 1 + r.int(3) }, () => r.pick(H)))],
          first,
          loose: first === null ? ownLoose : ownLoose === null ? first : Math.min(first, ownLoose),
          expl: r.bool(0.1) ? "" : expl,
          ans,
          lexpl: r.bool(0.3) ? expl : maybe(K, 0.6),
          lans: r.bool(0.3) ? ans : maybe(A, 0.6),
        };
      });
      const me = r.pick(us).id;
      const withC = await earlierCandidates(c, me, ps);
      const now = w.q(EARLIER_SQL, me, JSON.stringify(withC));
      assert.deepEqual(now, w.q(EARLIER_SQL_R30, me, JSON.stringify(ps)), `round ${round}: round-30 form`);
      assert.deepEqual(now, w.q(EARLIER_SQL_R29, me, JSON.stringify(ps)), `round ${round}: round-29 form`);
      named += now.filter((x) => x.m !== null).length;
      compared++;
    } finally {
      w.sql.exec("ROLLBACK TO a4");
      w.sql.exec("RELEASE a4");
    }
  }
  assert.ok(compared === 80 && named >= 200, `${named} named in ${compared} rounds`);
});

// ---- (a3) the licence re-key against the old statements --------------------------------------------

const dump = (w: World) =>
  JSON.stringify([
    w.q(`SELECT item_id, ckey, license, from_at, until_at FROM content_licences ORDER BY item_id, ckey, license`),
    w.q(`SELECT item_id, ckey FROM item_offer ORDER BY item_id, ckey`),
  ]);
function scratch<T>(w: World, f: () => T): T {
  w.sql.exec("SAVEPOINT r31");
  try {
    return f();
  } finally {
    w.sql.exec("ROLLBACK TO r31");
    w.sql.exec("RELEASE r31");
  }
}

test("31 (a3): the hourly licence re-key takes the same batches and writes the same rows as before (live and ghost content, hidden and shown, 'Q:' keys, unmappable and canonical rows, past the 1,000-row batch)", async () => {
  for (let seed = 0; seed < 3; seed++) {
    const r = rng(31300 + seed);
    const w = new World();
    const us = Array.from({ length: 3 }, () => w.user());
    const ids: string[] = [];
    for (let k = 0; k < 4; k++) {
      const qs = Array.from({ length: seed === 0 && k === 0 ? 420 : 6 + r.int(30) }, (_, i) => variant(r, i));
      const p = await publish(w, us[k % 3]!, bankOf(qs), { hideKey: k % 2 === 1 });
      ids.push(p.body.item_id as string);
      // a republish keeps the previous publish (two versions of each content)
      if (r.bool()) await publish(w, us[k % 3]!, bankOf(qs.map((x) => ({ ...x, explanation: "Edited." }))), { hideKey: r.bool(), base: p.body.publish_id as string }, p.body.item_id as string);
      w.advance(3_600_000);
    }
    // Ghosts: a copy of some live publishes' questions under a gone publish of the same item, some
    // without near hashes (not mappable through the ghost), and an item known only by its ghost.
    const live = w.q<Q>(`SELECT q.*, p.item_id, p.hide_key FROM item_questions q JOIN item_publishes p ON p.publish_id = q.publish_id`);
    const gp = w.sql.prepare(`INSERT OR IGNORE INTO ghost_publishes (publish_id, item_id, author_id, hide_key, public_version, license, at) VALUES (?, ?, ?, ?, 1, 'CC-BY-4.0', 1)`);
    const gq = w.sql.prepare(`INSERT INTO ghost_questions (publish_id, qid, pos, match_hash, full_content, stripped_content, near_full, near_stripped, chain_json, seq) VALUES (?, ?, ?, ?, ?, ?, ?, ?, '[]', 1)`);
    for (const [k, q] of live.filter(() => r.bool(0.3)).entries()) {
      const pid = `ghost-${q.item_id}-${k % 2}`;
      gp.run(pid, q.item_id, us[0]!.id, r.int(2));
      gq.run(pid, `g${k}`, k, q.match_hash, r.bool(0.9) ? `ghostfull${k}` : q.full_content, `ghoststripped${k}`, r.bool(0.8) ? `gnear${k}` : null, r.bool(0.8) ? `gnears${k}` : null);
    }
    gp.run("ghost-only", "0".repeat(32), us[1]!.id, 0);
    for (let k = 0; k < 5; k++) gq.run("ghost-only", `o${k}`, k, `m${k}`, `onlyfull${k}`, null, `onear${k}`, null);
    const ghosts = w.q<Q>(`SELECT q.*, p.item_id, p.hide_key FROM ghost_questions q JOIN ghost_publishes p ON p.publish_id = q.publish_id`);
    // Old-style rows: `q:<content>` (now and then `Q:`), in several licences, some already canonical too, some unmappable.
    const cl = w.sql.prepare(`INSERT OR IGNORE INTO content_licences (item_id, ckey, license, from_at, until_at) VALUES (?, ?, ?, ?, ?)`);
    const off = w.sql.prepare(`INSERT OR IGNORE INTO item_offer (item_id, ckey) VALUES (?, ?)`);
    for (const q of [...live, ...ghosts]) {
      const content = r.bool(0.15) ? q.full_content : q.hide_key === 1 ? q.stripped_content : q.full_content;
      if (content === null || (seed > 0 && r.bool(0.2))) continue;
      const key = `${r.bool(0.1) ? "Q" : "q"}:${content}`;
      for (const lic of ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0"].filter(() => seed === 0 || r.bool(0.7))) cl.run(q.item_id, key, lic, 1000 + r.int(1000), r.bool(0.5) ? null : 3000 + r.int(1000));
      if (r.bool(0.7)) off.run(q.item_id, key);
    }
    for (let k = 0; k < 50; k++) {
      cl.run(r.pick(ids), `q:${"ab".repeat(31)}${k.toString(16).padStart(2, "0")}`, "CC0-1.0", 5, null);
      off.run(r.pick(ids), `q:${"cd".repeat(31)}${k.toString(16).padStart(2, "0")}`);
    }
    const c = { db: w.db } as never;
    let changed = 0;
    for (let run = 0; run < 4; run++) {
      const before = dump(w);
      const old = scratch(w, () => {
        for (const s of licenceRekeyR30(LICENCE_REKEY_ROWS)) w.sql.prepare(s).run();
        return dump(w);
      });
      await licenceRekey(c);
      assert.equal(dump(w), old, `seed ${seed} run ${run}`);
      if (dump(w) !== before) changed++;
    }
    assert.ok(changed >= (seed === 0 ? 2 : 1), `seed ${seed}: ${changed} runs changed rows (the 1,000-row batch resumes)`);
  }
});

// ---- (b) rows visited and plans at 250 and 990 questions ---------------------------------------------

/** Two teachers' near-identical banks: one stem and set of pairs, `dup` questions per explanation (worst case: every explanation different, so positions differ but share their key-free and bare hashes). */
const sameBank = (n: number, tag: string, dup: number) =>
  bankOf(
    Array.from({ length: n }, (_, i) => ({
      stem: `${tag} question asks you to match these words carefully`,
      kind: { Matching: { pairs: Array.from({ length: 5 }, (_, j) => ({ left: `L${j} alpha`, right: `R${j} bravo` })), distractors: [] } },
      points: null,
      difficulty: null,
      topics: [],
      image: null,
      explanation: `Because ${tag} ${Math.floor(i / dup)} is matched the way the course matched it.`,
      figure: null,
      created: 1,
      modified: 1,
    })),
  );

interface Cap {
  sql: string;
  args: unknown[];
}
async function pairRun(n: number, dup: number) {
  const w = new World({ heavyQueueMs: 600_000 });
  const [A, B] = [w.user("teacher", "Ann"), w.user("teacher", "Bob")];
  assert.equal((await publish(w, A, sameBank(n, "Alpha", dup))).status, 201);
  w.advance(3_600_000);
  const caps: { firsts: Cap[]; earlier: Cap[] } = { firsts: [], earlier: [] };
  w.stats.log = (sql, args) => {
    if (IS_FIRSTS(sql)) caps.firsts.push({ sql, args: [...args] });
    if (IS_EARLIER.test(sql)) caps.earlier.push({ sql, args: [...args] });
  };
  // Bob publishes the same bank (one explanation edited), never having read Ann's.
  const b = await publish(w, B, sameBank(n, "Alpha", dup).replace("Because Alpha 0 is", "Because Alpha 0 really is"));
  w.stats.log = undefined;
  assert.equal(b.status, 201);
  assert.equal(caps.firsts.length, 1);
  assert.equal(caps.earlier.length, 1);
  return { w, A, B, caps };
}
/** Rows the lookup visits: per position its hashes' questions (round 30), or each hash's questions once per column and each position's candidates (round 31). */
function visits(w: World, firsts: Cap, earlier: Cap) {
  const ps = JSON.parse(earlier.args[1] as string) as { n: string[]; c: number[] }[];
  const cols = ["near_full", "near_bare_full", "near_stripped", "near_bare_stripped", "near_kf"];
  const any = new Map<string, number>(), each = new Map<string, number>();
  for (const v of JSON.parse(firsts.args[1] as string) as string[]) {
    any.set(v, w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM item_questions WHERE ${cols.map((x) => `${x} = ?1`).join(" OR ")}`, v)[0]!.n);
    each.set(v, cols.reduce((s, x) => s + w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM item_questions WHERE ${x} = ?`, v)[0]!.n, 0));
  }
  return {
    old: ps.reduce((s, p) => s + p.n.reduce((t, v) => t + any.get(v)!, 0), 0),
    now: [...each.values()].reduce((s, x) => s + x, 0) + ps.reduce((s, p) => s + p.c.length, 0),
    named: w.q<{ m: string | null }>(earlier.sql, ...earlier.args).filter((x) => x.m !== null).length,
  };
}
function bestOf(f: () => void) {
  let best = Infinity;
  for (let i = 0; i < 5; i++) {
    const t = performance.now();
    f();
    best = Math.min(best, performance.now() - t);
  }
  return best;
}

test("31 (b): two teachers' near-identical banks (250 and 990 questions; one stem, explanations shared by 1 or 990 questions): the lookup's rows visited grow linearly, every hash is followed by its index once, the answers are the old ones", async () => {
  const report: string[] = [];
  for (const dup of [1, 990]) {
    const small = await pairRun(250, dup);
    const big = await pairRun(990, dup);
    const v250 = visits(small.w, small.caps.firsts[0]!, small.caps.earlier[0]!);
    const v990 = visits(big.w, big.caps.firsts[0]!, big.caps.earlier[0]!);
    report.push(`dup ${dup}: round 30 ${v250.old} -> ${v990.old}; round 31 ${v250.now} -> ${v990.now}; named ${v250.named}, ${v990.named}`);
    assert.ok(v990.named >= 980 && v250.named >= 240, report.join("\n"));
    assert.ok(v990.now / v250.now <= 5, `round 31 grows linearly: ${report.join("\n")}`);
    assert.ok(v990.old / v250.old >= 12, `round 30 grew with the square: ${report.join("\n")}`);
    assert.ok(v990.old > 50 * v990.now, report.join("\n"));
    // The answers at 250 are the old statement's (at 990 the old one takes seconds: (a) covers it).
    const e = small.caps.earlier[0]!;
    assert.deepEqual(small.w.q(e.sql, ...e.args), small.w.q(EARLIER_SQL_R30, e.args[0], withoutCandidates(e.args[1] as string)));
    // Plans: each hash's questions by that column's index, each candidate by its rowid.
    const plan = (c: Cap) => (big.w.sql.prepare(`EXPLAIN QUERY PLAN ${c.sql}`).all(...(c.args as never[])) as { detail: string }[]).map((x) => x.detail);
    const p1 = plan(big.caps.firsts[0]!);
    for (const col of ["near_full", "near_bare_full", "near_stripped", "near_bare_stripped", "near_kf"]) assert.ok(p1.includes(`SEARCH iq USING INDEX item_questions_${col} (${col}=?)`), `${col}: ${p1.join(" | ")}`);
    assert.ok(!p1.some((d) => /^SCAN (iq|p|i|u)\b/.test(d)), p1.join(" | "));
    const p2 = plan(big.caps.earlier[0]!);
    assert.ok(p2.includes("SEARCH iq USING INTEGER PRIMARY KEY (rowid=?)") && !p2.some((d) => /^SEARCH \w+ USING INDEX item_questions_near_/.test(d)), p2.join(" | "));
    // Time of both statements, best of 5: about 4x for 4x the bank.
    const t = (x: typeof small) => bestOf(() => {
      x.w.q(x.caps.firsts[0]!.sql, ...x.caps.firsts[0]!.args);
      x.w.q(x.caps.earlier[0]!.sql, ...x.caps.earlier[0]!.args);
    });
    const [t250, t990] = [t(small), t(big)];
    report.push(`dup ${dup}: ${t250.toFixed(1)} -> ${t990.toFixed(1)} ms`);
    assert.ok(t990 < Math.max((process.env.POOL_STRICT_TIMING === "1" ? 8 : 12) * t250, 25), report.join("\n"));
  }
  if (process.env.POOL_ROUND31_REPORT === "1") console.log(report.join("\n"));
});

test("31 (b2): the hourly licence re-key's batch is found from the questions' side: no `q:` row is tested against every question of its item", async () => {
  const { w } = await pairRun(250, 1);
  const c = { db: w.db } as never;
  const plans: string[] = [];
  w.stats.log = (sql, args) => {
    if (!/content_licences|item_offer/.test(sql) || !/LIMIT 1000/.test(sql)) return;
    for (const x of w.sql.prepare(`EXPLAIN QUERY PLAN ${sql}`).all(...(args as never[])) as { detail: string }[]) plans.push(x.detail);
  };
  await licenceRekey(c);
  w.stats.log = undefined;
  assert.ok(plans.length > 0);
  // Before: every `q:` row (SCAN cx / ox) with a correlated walk of its item's questions.
  assert.ok(!plans.some((d) => /^SCAN (cx|ox)\b/.test(d)), plans.join(" | "));
  assert.ok(plans.some((d) => /^SEARCH x USING (INDEX|PRIMARY KEY|COVERING INDEX) .*\(item_id=\? AND ckey=\?\)/.test(d)), plans.join(" | "));
  const old = licenceRekeyR30(LICENCE_REKEY_ROWS).flatMap((s) => (w.sql.prepare(`EXPLAIN QUERY PLAN ${s}`).all() as { detail: string }[]).map((x) => x.detail));
  assert.ok(old.some((d) => /^SCAN (cx|ox)\b/.test(d)), "the old statements scanned every row");
});
