// Round 30 (staging D1 insights): publish-time statements whose rows read grew with the square of the
// bank size, so a max-size publish took 20-60 s and two at once failed with D1_ERROR.
// 1. The durable stem record's INSERT at publish (publish.ts) joined every question and band with the
//    WHOLE stem record (?4) and matched the position afterwards: now it reads its own entry by position.
// 2. The stem backfill's INSERT (stemBackfillAuthor) had the same shape over its explanation keys (?3).
// 3. The backfill's candidate SELECT probed author_stems by (author_id, qhash), which the primary key
//    (author_id, band, qhash, ...) cannot seek: index author_stems_qhash (migration 0107).
// 4. ownFuzzyFirst's durable lookup joined every position with every record of her bands; a bank of like
//    stems republished made that every position times every record. Now only the positions that can
//    reach its FUZZY_PAIRS_MAX cut are joined (provenance.ts ownStemsWindow).
// Behaviour is unchanged: (a) the old statements are kept here as the oracle and compared with the new
// ones on random banks and adversarial states; (b) query plans and rows visited on large banks.
import assert from "node:assert/strict";
import { test } from "node:test";
import { FUZZY_PAIRS_MAX, OWN_STEMS_SQL, ownStemsWindow } from "../../src/pool/provenance.ts";
import { genQuestion, KINDS, publish, rng, World, type Rng, type User } from "./harness.ts";
import { EARLIER_SQL_R29, withoutCandidates } from "./oracles.ts";

type Q = Record<string, any>;
type Row = Record<string, unknown>;

// ---- the statements as they were before round 30 (the oracle) -------------------------------------

const GUARD = `EXISTS (SELECT 1 FROM items WHERE id = ?1 AND current_publish_id = ?2)`;
const OLD_PUBLISH_STEMS = `INSERT INTO author_stems (author_id, band, qhash, tok_sk, body_sk, bk_expl, bk_ans, bk_expl_full, first_seq)
         SELECT ?3, kb.value, iq.match_hash, iq.tok_sk, iq.body_sk, COALESCE(iq.bk_expl, ''), COALESCE(iq.bk_ans, ''), COALESCE(j.value ->> '$.e', ''), MIN(iq.seq)
           FROM item_questions iq CROSS JOIN json_each(COALESCE(iq.kbands, '[]')) kb CROSS JOIN json_each(?4) j
          WHERE iq.publish_id = ?2 AND iq.tok_sk IS NOT NULL AND CAST(j.value ->> '$.p' AS INTEGER) = iq.pos AND ${GUARD}
          GROUP BY kb.value, iq.match_hash, iq.bk_expl, iq.bk_ans, j.value ->> '$.e'
         ON CONFLICT (author_id, band, qhash, bk_expl, bk_ans) DO UPDATE SET
           first_seq = MIN(first_seq, excluded.first_seq),
           bk_expl_full = COALESCE(author_stems.bk_expl_full, excluded.bk_expl_full)`;
const OLD_BACKFILL_STEMS = `INSERT INTO author_stems (author_id, band, qhash, tok_sk, body_sk, bk_expl, bk_ans, bk_expl_full, first_seq)
         SELECT ?1, kb.value, q.match_hash, q.tok_sk, q.body_sk, COALESCE(q.bk_expl, ''), COALESCE(q.bk_ans, ''), j.value, q.seq
           FROM item_questions q CROSS JOIN json_each(COALESCE(q.kbands, '[]')) kb CROSS JOIN json_each(?3) j
          WHERE q.publish_id = ?2 AND q.tok_sk IS NOT NULL AND CAST(j.key AS INTEGER) = q.pos AND j.value IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM users du WHERE du.id = ?1 AND du.deleted_at IS NOT NULL)
         ON CONFLICT (author_id, band, qhash, bk_expl, bk_ans) DO UPDATE SET
           first_seq = MIN(first_seq, excluded.first_seq),
           bk_expl_full = COALESCE(author_stems.bk_expl_full, excluded.bk_expl_full)`;
const IS_PUBLISH_STEMS = /^\s*INSERT INTO author_stems [^]*FROM item_questions iq /;
const IS_BACKFILL_STEMS = /^\s*INSERT INTO author_stems [^]*FROM item_questions q /;
const IS_CANDIDATES = /^\s*SELECT p\.publish_id, p\.hide_key, p\.full_key, p\.full_bytes FROM item_publishes p/;
const IS_CRON_AUTHORS = /^\s*SELECT DISTINCT p\.author_id AS a FROM item_publishes p/;
const IS_OWN_STEMS = (sql: string) => sql === OWN_STEMS_SQL;
const IS_WINDOW = /^\s*SELECT j\.value AS band, \(SELECT COUNT\(\*\) FROM \(SELECT 1 FROM author_stems/;

// ---- helpers ----------------------------------------------------------------------------------------

const STEM_COLS = ["author_id", "band", "qhash", "tok_sk", "body_sk", "bk_expl", "bk_ans", "bk_expl_full", "first_seq"];
const stems = (w: World) => w.q(`SELECT ${STEM_COLS.join(", ")} FROM author_stems ORDER BY author_id, band, qhash, bk_expl, bk_ans`);
/** Runs `f` and undoes everything it wrote. */
function scratch<T>(w: World, f: () => T): T {
  w.sql.exec("SAVEPOINT r30");
  try {
    return f();
  } finally {
    w.sql.exec("ROLLBACK TO r30");
    w.sql.exec("RELEASE r30");
  }
}
function setStems(w: World, rows: Row[]) {
  w.sql.exec("DELETE FROM author_stems");
  const ins = w.sql.prepare(`INSERT INTO author_stems (${STEM_COLS.join(", ")}) VALUES (${STEM_COLS.map(() => "?").join(", ")})`);
  for (const r of rows) ins.run(...(STEM_COLS.map((c) => r[c]) as never[]));
}
/** author_stems after `sql` runs on the stem rows `pre` (and whatever else the database holds now). */
const afterRun = (w: World, sql: string, args: unknown[], pre: Row[]) =>
  scratch(w, () => {
    setStems(w, pre);
    w.sql.prepare(sql).run(...(args as never[]));
    return JSON.stringify(stems(w));
  });

interface Cap {
  sql: string;
  args: unknown[];
  /** author_stems when the statement was about to run. */
  pre: Row[];
}
/** Records each statement matching `is` with its arguments and the stem rows just before it. */
function capture(w: World, is: (sql: string) => boolean): Cap[] {
  const out: Cap[] = [];
  const prev = w.stats.log;
  w.stats.log = (sql, args) => {
    prev?.(sql, args);
    if (is(sql)) out.push({ sql, args: [...args], pre: stems(w) });
  };
  return out;
}

const shuffle = <T>(r: Rng, xs: T[]): T[] => {
  const a = [...xs];
  for (let i = a.length - 1; i > 0; i--) {
    const j = r.int(i + 1);
    [a[i], a[j]] = [a[j]!, a[i]!];
  }
  return a;
};
/** A random bank: every kind, duplicates (the same content, or the same stem and choices with another explanation) at other positions. */
function randomBank(r: Rng, n: number, tag: string): Q[] {
  const qs: Q[] = [];
  for (let i = 0; i < n; i++) {
    if (qs.length > 0 && r.bool(0.2)) {
      const src = r.pick(qs);
      qs.push({ ...src, id: `q${i}local`, ...(r.bool() ? { explanation: `${tag} other explanation ${r.int(3)}` } : {}) });
    } else qs.push(genQuestion(r, r.pick(KINDS), i).json);
  }
  return qs;
}
const bankOf = (qs: Q[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });

// ---- (a) the old and new statements give the same author_stems ---------------------------------------

test("30 (a1): the publish's stem record INSERT and the backfill INSERT give the old statements' rows on random banks (publish, reordered republish, legacy backfill)", async () => {
  const seeds = Number(process.env.POOL_PROPERTY_SEEDS ?? 6);
  let compared = 0;
  for (let seed = 0; seed < seeds; seed++) {
    const r = rng(3000 + seed);
    const w = new World();
    const A = w.user("teacher", "Ann"), B = w.user("teacher", "Bob");
    const pubCaps = capture(w, (s) => IS_PUBLISH_STEMS.test(s));
    const qs = randomBank(r, 1 + r.int(40), "a");
    const a = await publish(w, A, bankOf(qs), { hideKey: r.bool() });
    assert.equal(a.status, 201, JSON.stringify(a.body).slice(0, 300));
    // Bob's own bank (another author's records beside Ann's).
    assert.equal((await publish(w, B, bankOf(randomBank(r, 1 + r.int(20), "b")), { hideKey: r.bool() })).status, 201);
    // Ann's records age: some lose the full explanation key, some get a later first_seq (the upsert's MIN and COALESCE matter).
    w.sql.prepare(`UPDATE author_stems SET bk_expl_full = CASE WHEN (unicode(substr(qhash, 1, 1)) + ${seed}) % 3 = 0 THEN NULL ELSE bk_expl_full END, first_seq = first_seq + (unicode(substr(qhash, 2, 1)) + ${seed}) % 3`).run();
    w.advance(60_000);
    // A reordered, partly edited republish: positions move, so every question's own entry is another one.
    const re = shuffle(r, qs).map((x, i) => (r.bool(0.3) ? { ...x, stem: `${x.stem} (edited ${i})` } : x));
    const b = await publish(w, A, bankOf(re), { hideKey: r.bool(), base: a.body.publish_id as string }, a.body.item_id as string);
    assert.equal(b.status, 200, JSON.stringify(b.body).slice(0, 300));
    w.stats.log = undefined;
    assert.ok(pubCaps.length >= 3, "each publish wrote its stem record");
    for (const c of pubCaps) {
      const got = afterRun(w, c.sql, c.args, c.pre);
      assert.equal(afterRun(w, OLD_PUBLISH_STEMS, c.args, c.pre), got, `seed ${seed}: publish stem record`);
      compared++;
    }
    // The legacy backfill (records without the full explanation key, or missing) at the hourly cron.
    w.sql.exec(`UPDATE author_stems SET bk_expl_full = NULL WHERE (unicode(substr(qhash, 3, 1)) + ${seed}) % 2 = 0 OR qhash = (SELECT min(qhash) FROM author_stems)`);
    w.sql.exec(`DELETE FROM author_stems WHERE (unicode(substr(qhash, 4, 1)) + ${seed}) % 4 = 0 AND qhash <> (SELECT min(qhash) FROM author_stems)`);
    const bfCaps = capture(w, (s) => IS_BACKFILL_STEMS.test(s));
    w.advance(3600_000);
    await w.pool.hourly();
    w.stats.log = undefined;
    assert.ok(bfCaps.length >= 1, `seed ${seed}: the backfill ran`);
    for (const c of bfCaps) {
      assert.equal(afterRun(w, OLD_BACKFILL_STEMS, c.args, c.pre), afterRun(w, c.sql, c.args, c.pre), `seed ${seed}: backfill`);
      compared++;
    }
  }
  assert.ok(compared >= seeds * 4);
});

test("30 (a2): adversarial item rows and arguments: duplicate contents and bands, NULL and '' keys, short or long arguments, the guards (item not current, author erased)", async () => {
  const w = new World();
  const A = w.user("teacher", "Ann");
  const pubCaps = capture(w, (s) => IS_PUBLISH_STEMS.test(s));
  const a = await publish(w, A, bankOf(randomBank(rng(77), 60, "x")), { hideKey: true });
  assert.equal(a.status, 201);
  w.sql.exec(`UPDATE author_stems SET bk_expl_full = NULL`);
  const bfCaps = capture(w, (s) => IS_BACKFILL_STEMS.test(s));
  w.advance(3600_000);
  await w.pool.hourly();
  w.stats.log = undefined;
  const pubSql = pubCaps[0]!.sql, bfSql = bfCaps[0]!.sql;
  const pid = a.body.publish_id as string;
  const n = w.q<{ n: number }>(`SELECT COUNT(*) AS n FROM item_questions WHERE publish_id = ?`, pid)[0]!.n;
  const r = rng(4242);
  const pool = <T>(xs: T[]) => () => r.pick(xs);
  const band = pool(["b1", "b2", "b3", "b4", "b5", "b6"]);
  const mh = pool(["m1", "m2", "m3", "m4"]);
  const key = pool<string | null>([null, "", "k1", "k2"]);
  const ex = pool(["", "e1", "e2", "e3"]);
  let cmp = 0;
  for (let round = 0; round < 60; round++)
    scratch(w, () => {
      for (const x of w.q<{ qid: string }>(`SELECT qid FROM item_questions WHERE publish_id = ?`, pid)) {
        const kb = r.bool(0.1) ? null : JSON.stringify(Array.from({ length: r.int(4) }, band));
        w.sql
          .prepare(`UPDATE item_questions SET match_hash = ?, kbands = ?, tok_sk = ?, body_sk = ?, bk_expl = ?, bk_ans = ?, seq = ? WHERE publish_id = ? AND qid = ?`)
          .run(mh(), kb, r.bool(0.1) ? null : `["t${r.int(3)}"]`, r.bool(0.3) ? null : `["s${r.int(3)}"]`, key(), key(), 1 + r.int(5), pid, x.qid);
      }
      const pre = Array.from({ length: r.int(30) }, () => ({ author_id: r.bool(0.8) ? A.id : "someone", band: band(), qhash: mh(), tok_sk: "[]", body_sk: null, bk_expl: key() ?? "", bk_ans: key() ?? "", bk_expl_full: r.bool() ? null : ex(), first_seq: 1 + r.int(6) }));
      const dedup = [...new Map(pre.map((x) => [`${x.author_id}|${x.band}|${x.qhash}|${x.bk_expl}|${x.bk_ans}`, x])).values()];
      const len = r.pick([n, n, n, Math.max(0, n - 3), n + 2]);
      const rec = JSON.stringify(Array.from({ length: len }, (_, i) => ({ p: i, e: ex() })));
      const expl = JSON.stringify(Array.from({ length: len }, () => (r.bool(0.2) ? null : ex())));
      if (r.bool(0.1)) w.sql.prepare(`UPDATE items SET current_publish_id = 'other' WHERE id = ?`).run(a.body.item_id);
      if (r.bool(0.1)) w.sql.prepare(`UPDATE users SET email = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL, deleted_at = 1 WHERE id = ?`).run(A.id);
      const pa = [a.body.item_id, pid, A.id, rec], ba = [A.id, pid, expl];
      assert.equal(afterRun(w, OLD_PUBLISH_STEMS, pa, dedup), afterRun(w, pubSql, pa, dedup), `round ${round}: publish`);
      assert.equal(afterRun(w, OLD_BACKFILL_STEMS, ba, dedup), afterRun(w, bfSql, ba, dedup), `round ${round}: backfill`);
      cmp += 2;
    });
  assert.equal(cmp, 120);
});

/** Random durable stem records for `me`: few bands (hot ones shared by many records) or many. */
function seedStems(w: World, r: Rng, me: string, o: { records: number; bands: number; perRecord: number }) {
  const ins = w.sql.prepare(`INSERT OR IGNORE INTO author_stems (${STEM_COLS.join(", ")}) VALUES (${STEM_COLS.map(() => "?").join(", ")})`);
  w.sql.exec("BEGIN");
  for (let k = 0; k < o.records; k++) {
    const qhash = `h${r.int(o.records)}`, bk_expl = r.pick(["", "e1", "e2"]), bk_ans = r.pick(["", "a1"]);
    // (the same content under several bands, each row with its OWN values: which row a group reports must not change)
    for (let j = 0; j < o.perRecord; j++)
      ins.run(me, `b${r.int(o.bands)}`, qhash, `["t${r.int(4)}"]`, r.bool(0.3) ? null : `["s${r.int(4)}"]`, bk_expl, bk_ans, r.bool(0.3) ? null : `f${r.int(4)}`, 1 + r.int(50));
  }
  w.sql.exec("COMMIT");
}

test("30 (a3): her durable stem lookup over the positions ownStemsWindow keeps gives the full join's rows, in order, whether or not the FUZZY_PAIRS_MAX cut is reached", async () => {
  const w = new World();
  const c = { db: w.db } as never;
  let cut = 0, uncut = 0, trimmed = 0;
  for (let seed = 0; seed < 30; seed++) {
    const r = rng(9000 + seed);
    const me = w.user().id, other = w.user().id;
    // Hot: a few bands shared by hundreds of records. Sparse: many bands, few records each. Tight: one
    // band a position, one per record, so a position has exactly as many rows as its band has records
    // (the window's count is then exact, and leaving out one more position would change the answer).
    const regime = (["hot", "sparse", "tight"] as const)[seed % 3]!;
    if (regime === "hot") seedStems(w, r, me, { records: 200 + r.int(400), bands: 4 + r.int(6), perRecord: 1 + r.int(3) });
    else if (regime === "sparse") seedStems(w, r, me, { records: 40 + r.int(200), bands: 400, perRecord: 1 + r.int(3) });
    else seedStems(w, r, me, { records: 300 + r.int(600), bands: 8, perRecord: 1 });
    seedStems(w, r, other, { records: 100, bands: 6, perRecord: 2 });
    const npos = 1 + r.int(regime === "sparse" ? 400 : 120);
    const nb = () => (regime === "tight" ? 1 : 1 + r.int(8));
    const bandPool = regime === "hot" ? 12 : regime === "sparse" ? 800 : 8;
    // (positions in any order, with gaps, and, but when tight, now and then one twice)
    let at = 0;
    const kb = shuffle(
      r,
      Array.from({ length: npos }, (_, i) => ({ pos: regime === "tight" ? (at += 1 + r.int(3)) : i * (1 + r.int(2)), bands: Array.from({ length: nb() }, () => `b${r.int(bandPool)}`) })),
    );
    const full = w.q(OWN_STEMS_SQL, me, JSON.stringify(kb), FUZZY_PAIRS_MAX);
    const keep = await ownStemsWindow(c, me, kb);
    assert.ok(keep.every((x, i) => i === 0 || kb.indexOf(x) > kb.indexOf(keep[i - 1]!)), "the kept positions in their own order");
    const win = w.q(OWN_STEMS_SQL, me, JSON.stringify(keep), FUZZY_PAIRS_MAX);
    assert.deepEqual(win, full, `seed ${seed}`);
    if (full.length >= FUZZY_PAIRS_MAX) {
      cut++;
      if (keep.length < kb.length) trimmed++;
    } else {
      uncut++;
      assert.equal(keep.length, kb.length, `seed ${seed}: below the cut every position is joined`);
    }
  }
  assert.ok(cut >= 5 && uncut >= 5 && trimmed >= 3, `both regimes (${cut} cut, ${trimmed} of them with positions left out; ${uncut} not)`);
});

const IS_EARLIER = /^\s*SELECT CAST\(j\.value ->> '\$\.i' AS INTEGER\) AS pos,/;
/** The earlier-publication lookup as it was (oracles.ts; round 31 adds each position's candidates `c`). */
const oldEarlier = (c: Cap) => [c.args[0], withoutCandidates(c.args[1] as string)];

test("30 (a4): the earlier-publication lookup (publish.ts, questions matched by near hash) answers as the publish-driven form did, ties included (a bank holding a question twice)", async () => {
  let compared = 0, found = 0;
  for (let seed = 0; seed < Number(process.env.POOL_PROPERTY_SEEDS ?? 6); seed++) {
    const r = rng(5100 + seed);
    const w = new World();
    const [A, B, C, D] = [w.user("teacher", "Ann"), w.user("teacher", "Bob"), w.user("teacher", "Cy"), w.user("teacher", "Dee")];
    // Ann's bank holds some questions twice (the same content at two positions: a tie on seq).
    const qs = randomBank(r, 10 + r.int(30), "e");
    const twice = qs.flatMap((x, i) => (r.bool(0.3) ? [x, { ...x, id: `dup${i}` }] : [x]));
    assert.equal((await publish(w, A, bankOf(twice), { hideKey: r.bool(0.3) })).status, 201);
    w.advance(60_000);
    // Bob and Cy publish the same questions later (more publishes holding each match), Cy with edits.
    await publish(w, B, bankOf(shuffle(r, twice)), { hideKey: r.bool(0.3) });
    w.advance(60_000);
    await publish(w, C, bankOf(qs.map((x, i) => (r.bool(0.3) ? { ...x, stem: `${x.stem} Explain.` } : { ...x, id: `c${i}` }))), { hideKey: r.bool(0.3) });
    w.advance(60_000);
    const caps = capture(w, (s) => IS_EARLIER.test(s));
    // Dee publishes them too, without having read any: the lookup names the earliest publication.
    await publish(w, D, bankOf(shuffle(r, twice)), { hideKey: r.bool(0.3) });
    w.stats.log = undefined;
    for (const c of caps) {
      const now = w.q(c.sql, ...c.args), before = w.q(EARLIER_SQL_R29, ...oldEarlier(c));
      assert.deepEqual(now, before, `seed ${seed}`);
      found += now.filter((x) => x.m !== null).length;
      compared++;
    }
  }
  assert.ok(compared >= 3 && found >= 10, `${compared} lookups compared, ${found} earlier publications found`);
});

// ---- (b) plans and rows visited on large banks ------------------------------------------------------

/** The staging repro's banks: Matching of 50 pairs (stems 4+ words, alike) and Mc of 26 choices. */
const base = (i: number, stem: string, kind: Q, extra: Q = {}): Q => ({ id: `q${i}`, stem, kind, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const matching = (n: number, tag: string) =>
  bankOf(
    Array.from({ length: n }, (_, i) =>
      base(i, `${tag} question ${i} asks you to match these words carefully`, {
        Matching: { pairs: Array.from({ length: 50 }, (_, j) => ({ left: `L${i}x${j} alpha ${"l".repeat(12)}`, right: `R${i}x${j} bravo ${"r".repeat(12)}` })), distractors: [] },
      }, { explanation: `Because ${tag} ${i} is matched the way the course matched it (see unit three).` }),
    ),
  );
const mc = (n: number, tag: string) =>
  bankOf(
    Array.from({ length: n }, (_, i) =>
      base(i, `${tag} item ${i} which of these options is the right one here`, {
        Mc: { answers: Array.from({ length: 26 }, (_, j) => ({ text: `${tag} option ${i}-${j} text`, correct: j === i % 26, locked: false })) },
      }, { explanation: `${tag} reason ${i}` }),
    ),
  );

/**
 * A loop that runs once per row of an enclosing loop (a later loop of the same SELECT, or inside a
 * correlated subquery; an IN list is built once) and walks a WHOLE bound JSON array (`json_each(?N)`),
 * or all of an author's stem records by author_id alone, or (in a statement that runs over the bank's
 * positions, a bound array) all the questions of a publish: the shapes of round 30.
 */
function perRowWalks(w: World, sql: string, args: unknown[]): string[] {
  const plan = w.sql.prepare(`EXPLAIN QUERY PLAN ${sql}`).all(...(args as never[])) as { id: number; parent: number; detail: string }[];
  const byId = new Map(plan.map((x) => [x.id, x]));
  const first = new Map<number, number>();
  for (const x of plan) if (/^(SCAN|SEARCH) /.test(x.detail) && !first.has(x.parent)) first.set(x.parent, x.id);
  const boundArray = (d: string) => {
    const v = /^SCAN (\w+) VIRTUAL TABLE/.exec(d);
    return !!v && new RegExp(`json_each\\(\\s*\\?\\d+\\s*\\)\\s+(AS\\s+)?${v[1]}\\b`).test(sql);
  };
  const overPositions = boundArray(byId.get(first.get(0) ?? -1)?.detail ?? "");
  const out: string[] = [];
  for (const x of plan) {
    const up: string[] = [];
    for (let p = byId.get(x.parent); p; p = byId.get(p.parent)) up.push(p.detail);
    if (up.some((d) => /^LIST SUBQUERY/.test(d))) continue;
    if (first.get(x.parent) === x.id && !up.some((d) => /^CORRELATED/.test(d))) continue;
    if (boundArray(x.detail)) out.push(x.detail);
    if (/^SEARCH \w+ USING PRIMARY KEY \(author_id=\?\)$/.test(x.detail) && /\bauthor_stems\b/.test(sql)) out.push(x.detail);
    if (overPositions && /^SEARCH \w+ USING (COVERING )?INDEX (item_questions_publish_lineage|sqlite_autoindex_item_questions_1) \(publish_id=\?\)$/.test(x.detail)) out.push(x.detail);
  }
  return out;
}

interface Run {
  w: World;
  A: User;
  walks: string[];
  caps: Record<"publish" | "backfill" | "candidates" | "authors" | "own" | "window" | "earlier", Cap[]>;
  ownKb: { pos: number; bands: string[] }[];
}
/** A big-bank session: publish, a republish (one stem edited), another teacher's copy (never read), a Mc bank, the hourly backfill of legacy records. */
async function bigRun(n: number): Promise<Run> {
  const w = new World({ heavyQueueMs: 600_000 });
  const A = w.user("teacher", "Ann"), C = w.user("teacher", "Cal"), D = w.user("teacher", "Dee");
  const walks: string[] = [];
  const caps: Run["caps"] = { publish: [], backfill: [], candidates: [], authors: [], own: [], window: [], earlier: [] };
  let phase = "";
  w.stats.log = (sql, args) => {
    if (/^\s*(SELECT|WITH|INSERT|UPDATE|DELETE)/i.test(sql)) for (const x of perRowWalks(w, sql, args)) walks.push(`${phase}: ${x} :: ${sql.replace(/\s+/g, " ").slice(0, 140)}`);
    const k = IS_PUBLISH_STEMS.test(sql) ? "publish" : IS_BACKFILL_STEMS.test(sql) ? "backfill" : IS_CANDIDATES.test(sql) ? "candidates" : IS_CRON_AUTHORS.test(sql) ? "authors" : IS_OWN_STEMS(sql) ? "own" : IS_WINDOW.test(sql) ? "window" : IS_EARLIER.test(sql) ? "earlier" : null;
    if (k && (k === "own" || k === "window" ? phase === "republish" : k === "earlier" ? phase === "copy" : true)) caps[k].push({ sql, args: [...args], pre: k === "publish" || k === "backfill" || k === "earlier" ? [] : stems(w) });
  };
  phase = "publish";
  const a = await publish(w, A, matching(n, "Alpha"), { hideKey: true });
  assert.equal(a.status, 201, JSON.stringify(a.body).slice(0, 200));
  w.advance(60_000);
  phase = "republish";
  const b = await publish(w, A, matching(n, "Alpha").replace("question 0 asks", "question 0 now asks"), { hideKey: true, base: a.body.publish_id as string }, a.body.item_id as string);
  assert.equal(b.status, 200, JSON.stringify(b.body).slice(0, 200));
  w.advance(60_000);
  phase = "copy";
  assert.equal((await publish(w, D, matching(n, "Alpha").replace("question 3 asks", "question 3 now asks"))).status, 201);
  phase = "mc";
  assert.equal((await publish(w, C, mc(n, "Charlie"))).status, 201);
  phase = "hourly";
  w.sql.exec(`UPDATE author_stems SET bk_expl_full = NULL WHERE author_id = '${A.id}'`);
  w.advance(3600_000);
  await w.pool.hourly();
  w.stats.log = undefined;
  // Her positions as ownFuzzyFirst saw them at the republish: every question's keyed stem bands.
  const ownKb = w
    .q<{ pos: number; kbands: string }>(`SELECT pos, kbands FROM item_questions WHERE publish_id = ? AND kbands IS NOT NULL ORDER BY pos`, b.body.publish_id)
    .map((x) => ({ pos: x.pos, bands: JSON.parse(x.kbands) as string[] }))
    .filter((x) => x.bands.length > 0);
  return { w, A, walks, caps, ownKb };
}

/** Best of five wall-clock runs of a statement (each undone), in ms. */
function timeIt(w: World, c: Cap, restore: boolean): number {
  let best = Infinity;
  for (let i = 0; i < 5; i++)
    scratch(w, () => {
      if (restore) setStems(w, c.pre);
      const st = w.sql.prepare(c.sql);
      const t0 = performance.now();
      st.all(...(c.args as never[]));
      best = Math.min(best, performance.now() - t0);
    });
  return best;
}
/** Rows her durable lookup visits: the window's capped counts plus the join's records, or the full join's. */
function ownRows(w: World, me: string, kb: { pos: number; bands: string[] }[], window: boolean): number {
  const join = (k: unknown[]) =>
    w.q<{ n: number }>(
      `WITH b AS MATERIALIZED (SELECT CAST(j.value ->> '$.pos' AS INTEGER) AS pos, bv.value AS band FROM json_each(?2) j, json_each(j.value -> '$.bands') bv)
       SELECT COUNT(*) AS n FROM b CROSS JOIN author_stems s ON s.author_id = ?1 AND s.band = b.band`,
      me,
      JSON.stringify(k),
    )[0]!.n;
  if (!window) return join(kb);
  const counts = w.q<{ band: string; n: number }>(
    `SELECT j.value AS band, (SELECT COUNT(*) FROM (SELECT 1 FROM author_stems s WHERE s.author_id = ?1 AND s.band = j.value LIMIT ?3)) AS n FROM json_each(?2) j`,
    me,
    JSON.stringify([...new Set(kb.flatMap((x) => x.bands))]),
    FUZZY_PAIRS_MAX,
  );
  const n = new Map(counts.map((x) => [x.band, x.n]));
  let total = 0, last = Infinity;
  for (const x of [...kb].sort((p, q) => p.pos - q.pos)) {
    total += Math.max(0, ...x.bands.map((b) => n.get(b) ?? 0));
    if (total >= FUZZY_PAIRS_MAX) {
      last = x.pos;
      break;
    }
  }
  return counts.reduce((s, x) => s + x.n, 0) + join(kb.filter((x) => x.pos <= last));
}

test("30 (b): a 990 x 50 Matching bank (published, republished, backfilled) and a 1000 x 26 Mc bank: no statement walks a whole bound array or her whole stem record per row; rows visited and time grow linearly from 250 to 1000", async () => {
  const small = await bigRun(250);
  const big = await bigRun(990);
  // Every publish-time statement (and the hourly backfill), by its query plan.
  assert.deepEqual(big.walks, [], "per-row walks at 990");
  assert.deepEqual(small.walks, []);
  const { w, A, caps } = big;
  for (const k of ["publish", "backfill", "candidates", "authors", "own", "window", "earlier"] as const) assert.ok(caps[k].length >= 1, `${k} ran`);
  // ... and the old statements had exactly that shape (the lint sees what staging measured).
  for (const [old, c] of [[OLD_PUBLISH_STEMS, caps.publish[0]!], [OLD_BACKFILL_STEMS, caps.backfill[0]!]] as const) {
    const walks = perRowWalks(w, old, c.args);
    assert.equal(walks.length, 1, walks.join("; "));
    assert.match(walks[0]!, /^SCAN j VIRTUAL TABLE/);
  }
  assert.deepEqual(perRowWalks(w, EARLIER_SQL_R29, oldEarlier(caps.earlier[0]!)), ["SEARCH iq USING INDEX item_questions_publish_lineage (publish_id=?)"]);
  // The copy's earlier-publication lookup found Ann's bank for (nearly) every question, as before.
  const named = (sql: string, c: Cap) => w.q<{ m: string | null }>(sql, ...c.args).filter((x) => x.m !== null).length;
  assert.ok(named(caps.earlier[0]!.sql, caps.earlier[0]!) > 900);
  const noIndex = <T>(f: () => T) => scratch(w, () => (w.sql.exec(`DROP INDEX author_stems_qhash`), f()));
  noIndex(() => {
    for (const k of ["candidates", "authors"] as const) assert.deepEqual(perRowWalks(w, caps[k].at(-1)!.sql, caps[k].at(-1)!.args), ["SEARCH s USING PRIMARY KEY (author_id=?)"], `${k} without the index`);
  });
  // The index changes plans, never answers: the candidate and author SELECTs with and without it, on
  // the stem records when they ran (legacy ones to backfill) and now (all complete).
  for (const k of ["candidates", "authors"] as const)
    for (const c of caps[k])
      for (const pre of [c.pre, null]) {
        const ans = () => scratch(w, () => (pre && setStems(w, pre), JSON.stringify(w.q(c.sql, ...c.args))));
        assert.equal(noIndex(ans), ans(), k);
      }
  // Rows her durable lookup visits at the republish (exact counts): linear now, quadratic before.
  const rows = (r: Run, window: boolean) => scratch(r.w, () => (setStems(r.w, r.caps.own[0]!.pre), ownRows(r.w, r.A.id, r.ownKb, window)));
  const [n250, n990, o250, o990] = [rows(small, true), rows(big, true), rows(small, false), rows(big, false)];
  assert.ok(n990 / n250 <= 5, `durable lookup rows visited: ${n250} at 250, ${n990} at 990`);
  assert.ok(o990 / o250 >= 12, `the full join's: ${o250} at 250, ${o990} at 990`);
  assert.ok(o990 > 20 * n990, `${o990} vs ${n990}`);
  // The positions counted are the ones the publish joined.
  for (const r of [small, big]) {
    r.w.sql.exec("SAVEPOINT win");
    try {
      setStems(r.w, r.caps.own[0]!.pre);
      const keep = await ownStemsWindow({ db: r.w.db } as never, r.A.id, r.ownKb);
      assert.deepEqual((JSON.parse(r.caps.own[0]!.args[1] as string) as { pos: number }[]).map((x) => x.pos), keep.map((x) => x.pos));
      assert.ok(keep.length < r.ownKb.length / 10, `${keep.length} of ${r.ownKb.length} positions joined`);
    } finally {
      r.w.sql.exec("ROLLBACK TO win");
      r.w.sql.exec("RELEASE win");
    }
  }
  // Time (best of 5, each undone): every rewritten statement grows about 4x for 4x the bank. (The stem
  // record and backfill INSERTs are re-run over the rows they wrote; the candidate SELECT on the complete
  // records, its slowest case: every question probed; the durable lookup on the records it saw; the
  // earlier-publication lookup of the copy on everything published since.)
  const lim = process.env.POOL_STRICT_TIMING === "1" ? 8 : 12;
  const timed: string[] = [];
  for (const k of ["publish", "backfill", "candidates", "own", "window", "earlier"] as const) {
    const restore = k === "own" || k === "window";
    const t250 = timeIt(small.w, small.caps[k].at(-1)!, restore);
    const t990 = timeIt(w, caps[k].at(-1)!, restore);
    timed.push(`${k}: ${t250.toFixed(1)} -> ${t990.toFixed(1)} ms`);
    assert.ok(t990 < Math.max(lim * t250, 25), `${k} grew ${(t990 / t250).toFixed(1)}x (${timed.join("; ")})`);
  }
  if (process.env.POOL_ROUND30_REPORT === "1") console.log(timed.join("\n"), { n250, n990, o250, o990 });
});
