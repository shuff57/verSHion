// Round 27: security critic 35d B-1. A user request already authenticated when its account is erased
// commits its write AFTER the erasure. Every write keyed to the caller is guarded IN the write (the
// account must not be erased, same statement or same atomic batch), and the daily erasure_resweep
// cleans every table such a writer touches. The race tests erase before EVERY statement boundary of one
// request (a standalone statement, or the start of an atomic batch), settle with 40+ days of crons, and
// scan every table for the erased id and for the account's name, phone and address text.
import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { framed, png, publish, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "x", name, questions: qs.map((x, i) => ({ ...x, id: x.id ?? `q${i}` })), created: 1, modified: 1 } });
const q = (stem: string, extra: Q = {}): Q => ({ stem, kind: { Short: { sample: "secret answer" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });

const NAME = "Zorgon Quixote";
const PHONE = "5305551212";
const ADDR = "12 Elm St Chico CA";
const PII = [NAME, PHONE, ADDR].map((s) => s.toLowerCase());

const erase = async (w: World, u: User) => {
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, profile = NULL, deleted_at = ? WHERE id = ? AND deleted_at IS NULL`).run(w.clock, u.id);
  await w.db.batch(deleteContentStatements(w.db, u.id, w.clock));
};

/** 40+ days of hourly and daily crons (the purge, the 14-day windows and the 60-day appeal rule all pass). */
async function settle(w: World) {
  for (const step of [3600_000, DAY, DAY, 15 * DAY, DAY, 30 * DAY, DAY]) {
    w.advance(step);
    await w.pool.hourly();
    await w.pool.daily();
    await Promise.all(w.pending.splice(0));
  }
}

/** Tables (and row counts) holding `needle` anywhere in a row, over EVERY table. */
function scan(w: World, needles: string[]): Record<string, number> {
  const out: Record<string, number> = {};
  const tables = w.q<{ name: string }>(`SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'`).map((t) => t.name);
  for (const t of tables) {
    for (const row of w.q(`SELECT * FROM "${t}"`)) {
      const s = JSON.stringify(row, (_k, v) => (typeof v === "bigint" ? String(v) : v)).toLowerCase();
      if (needles.some((n) => s.includes(n))) out[t] = (out[t] ?? 0) + 1;
    }
  }
  return out;
}

/** Wraps the world's db so `boundary(k)` runs before the k-th statement boundary (a lone statement, or the start of a batch). */
function instrument(w: World, boundary: (k: number) => Promise<void>) {
  const db = w.db as any;
  const prep = db.prepare.bind(db);
  const batch = db.batch.bind(db);
  let k = 0;
  let inside = false;
  const hit = async () => {
    if (inside) return;
    const n = k++;
    inside = true;
    try {
      await boundary(n);
    } finally {
      inside = false;
    }
  };
  const wrap = (s: any): any => {
    const o = Object.create(s);
    o.bind = (...a: unknown[]) => wrap(s.bind(...a));
    for (const m of ["first", "all", "run"]) o[m] = async () => (await hit(), s[m]());
    return o;
  };
  db.prepare = (sql: string) => wrap(prep(sql));
  db.batch = async (ss: unknown[]) => (await hit(), batch(ss));
  return {
    count: () => k,
    restore: () => {
      db.prepare = prep;
      db.batch = batch;
    },
    /** Runs `f` with the boundary hook paused (an erasure, say). */
    quiet: async <T>(f: () => Promise<T>): Promise<T> => {
      const was = inside;
      inside = true;
      try {
        return await f();
      } finally {
        inside = was;
      }
    },
  };
}

interface Scenario {
  name: string;
  /** Builds the world; `req` is the in-flight request of Z, `after` an extra check on the settled world. */
  build(): Promise<{ w: World; Z: User; req: () => Promise<{ status: number }>; check?: (w: World, ref: boolean) => void }>;
}

async function run(sc: Scenario, mode: "ref" | "control" | number) {
  const { w, Z, req, check } = await sc.build();
  let status = 0;
  let boundaries = 0;
  if (mode === "control") {
    await erase(w, Z);
    status = (await req()).status;
  } else {
    const ins = instrument(w, async (k) => {
      if (k === mode) await erase(w, Z);
    });
    try {
      status = (await req()).status;
    } finally {
      ins.restore();
    }
    boundaries = ins.count();
    // (ref: the request finished first, then the account was erased: the normal erasure)
    if (mode === "ref") await erase(w, Z);
    else if (typeof mode === "number" && mode >= boundaries) await erase(w, Z);
  }
  await Promise.all(w.pending.splice(0));
  await settle(w);
  return { w, Z, status, boundaries, check };
}

async function race(sc: Scenario, o: { controlStatus?: number } = {}) {
  const ref = await run(sc, "ref");
  const ctl = await run(sc, "control");
  assert.equal(ctl.status, o.controlStatus ?? 401, `${sc.name}: control: a request after the erasure is refused like an unauthenticated one`);
  const allowed = (r: { w: World; Z: User }) => scan(r.w, [r.Z.id]);
  const refRows = allowed(ref);
  const ctlRows = allowed(ctl);
  const refKeys = ref.w.r2.keys();
  assert.ok(ref.boundaries >= 1, `${sc.name}: the request has statement boundaries (${ref.boundaries})`);
  assert.deepEqual(scan(ref.w, PII), {}, `${sc.name}: reference (request, then erasure) leaves no PII`);
  assert.deepEqual(scan(ctl.w, PII), {}, `${sc.name}: control leaves no PII`);
  const chk = (label: string, r: { w: World; check?: (w: World, ref: boolean) => void }) => {
    try {
      r.check?.(r.w, false);
    } catch (e) {
      (e as Error).message = `${sc.name} [${label}]: ${(e as Error).message}`;
      throw e;
    }
  };
  chk("control", ctl);
  chk("reference", ref);
  const statuses = new Set<number>();
  for (let k = 0; k < ref.boundaries; k++) {
    const r = await run(sc, k);
    statuses.add(r.status);
    const rows = allowed(r);
    // Only the documented residue of a NORMAL erasure may be there: tables the reference (or the control)
    // also holds the id in, and never more rows of it.
    for (const [t, n] of Object.entries(rows)) {
      const cap = Math.max(refRows[t] ?? 0, ctlRows[t] ?? 0);
      assert.ok(n <= cap && cap > 0, `${sc.name}: erased before boundary ${k}/${ref.boundaries} (status ${r.status}): residue in ${t} (${n} rows, normal erasure leaves ${cap})`);
    }
    assert.deepEqual(scan(r.w, PII), {}, `${sc.name}: erased before boundary ${k}: PII left`);
    assert.ok(r.w.r2.keys().length <= Math.max(refKeys.length, ctl.w.r2.keys().length), `${sc.name}: boundary ${k}: R2 holds ${r.w.r2.keys().length} objects after the settle (reference ${refKeys.length})`);
    chk(`erased before boundary ${k}`, r);
  }
  return { ref, ctl, statuses };
}

async function world() {
  const w = new World();
  const adm = w.user("admin"), A = w.user("teacher", "Ann");
  const Z = w.user("teacher", NAME);
  const a = (await publish(w, A, bankOf([q("Ann's question about glaciers and the valleys they carve in granite?")]))).body as any;
  return { w, adm, A, Z, a };
}
const notice = async (w: World, adm: User) =>
  (await w.admin<any>(adm, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "legal@pub.example", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;

const SCENARIOS: Scenario[] = [
  {
    name: "27 race GET /items/:id",
    async build() {
      const { w, Z, a } = await world();
      return { w, Z, req: () => w.json("GET", `/items/${a.item_id}`, { user: Z }) };
    },
  },
  {
    name: "27 race POST /items/:id/import",
    async build() {
      const { w, Z, a } = await world();
      return { w, Z, req: () => w.json("POST", `/items/${a.item_id}/import`, { user: Z }) };
    },
  },
  {
    name: "27 race PUT /items/prefs",
    async build() {
      const { w, Z } = await world();
      return { w, Z, req: () => w.json("PUT", "/items/prefs", { user: Z, json: { keep_on_delete: true } }) };
    },
  },
  {
    name: "27 race POST /items/:id/report (wrong_answer, with a note)",
    async build() {
      const { w, Z, a } = await world();
      return { w, Z, req: () => w.json("POST", `/items/${a.item_id}/report`, { user: Z, json: { reason: "wrong_answer", note: `my note ${NAME} ${PHONE}` } }) };
    },
  },
  {
    name: "27 race POST /items/:id/report (student_data: instant hold)",
    async build() {
      const { w, Z, a } = await world();
      w.history(Z);
      return { w, Z, req: () => w.json("POST", `/items/${a.item_id}/report`, { user: Z, json: { reason: "student_data", note: `names a student, call ${NAME}` } }) };
    },
  },
  {
    name: "27 race POST /items/:id/appeal (the open appeal must not block the purge)",
    async build() {
      const { w, adm, Z } = await world();
      const own = (await publish(w, Z, bankOf([q("Zorgon's own question about moraines and the hidden answer of eskers?", { explanation: "Zorgon's long private explanation of eskers" })]), { hideKey: true })).body as any;
      assert.equal((await w.admin(adm, "POST", "/admin/pool/remove", { item_id: own.item_id, note: `removing ${NAME}'s item` })).status, 200);
      return {
        w,
        Z,
        req: () => w.json("POST", `/items/${own.item_id}/appeal`, { user: Z, json: { note: `please restore, call me ${NAME} ${PHONE}` } }),
        check: (w2: World) => {
          assert.equal(w2.q<any>(`SELECT purged_at IS NOT NULL AS p FROM items WHERE id = ?`, own.item_id)[0].p, 1, "the erased author's removed item is purged on schedule");
          assert.deepEqual(w2.r2.keys(`items/${own.item_id}/`), [], "and its bodies are gone from R2");
          assert.equal(w2.q(`SELECT 1 FROM admin_flags WHERE kind = 'appeal' AND resolved_at IS NULL`).length, 0, "no open appeal is left");
        },
      };
    },
  },
  {
    name: "27 race POST /items/:id/counter-notice (name, address, phone, signature)",
    async build() {
      const { w, adm, Z } = await world();
      const own = (await publish(w, Z, bankOf([q("Zorgon's own question about tarns and cirques in the high Sierra?")]))).body as any;
      assert.equal((await w.takedown<any>(adm, await notice(w, adm), own.item_id)).status, 200);
      return {
        w,
        Z,
        req: () => w.json("POST", `/items/${own.item_id}/counter-notice`, { user: Z, json: { signature: NAME, material_desc: `mine, ${NAME}`, name: NAME, address: ADDR, phone: PHONE, perjury_stmt: true, consent_jurisdiction: true, accept_service: true } }),
        check: (w2: World) => {
          for (const r of w2.q<any>(`SELECT name, address, phone, signature FROM dmca_counter_notices`)) assert.deepEqual(r, { name: "[deleted]", address: "[deleted]", phone: "[deleted]", signature: "[deleted]" });
        },
      };
    },
  },
  {
    name: "27 race PUT /blobs/:sha256",
    async build() {
      const { w, Z } = await world();
      return { w, Z, req: () => upload(w, Z, png(9, 9, 5)) };
    },
  },
  {
    name: "27 race POST /items (publish)",
    async build() {
      const { w, Z } = await world();
      return { w, Z, req: () => publish(w, Z, bankOf([q("Zorgon's new question about aquifers and the wells that tap them?")])) };
    },
  },
];

for (const sc of SCENARIOS)
  test(sc.name, { timeout: 900_000 }, async () => {
    const { statuses } = await race(sc, { controlStatus: 401 });
    // the race lands on both sides of the erasure: some interleavings commit (request first), others refuse
    assert.ok(statuses.size >= 1);
    for (const s of statuses) assert.ok([200, 201, 202, 401, 404, 409, 413].includes(s), `${sc.name}: unexpected status ${s}`);
    const refused = [...statuses].includes(401);
    assert.ok(refused, `${sc.name}: an interleaving after the erasure answers 401, like an unauthenticated request (statuses ${[...statuses].join(",")})`);
  });

// ---- the resweep alone: writes that slipped in (an older build, or a request already past its guard) --------

const SNAP: [string, string][] = [
  ["imports", "user_id = ?1"],
  ["served", "reader_id = ?1"],
  ["pool_prefs", "user_id = ?1"],
  ["reports", "reporter_id = ?1"],
  ["admin_flags", "user_id = ?1 OR kind = 'key_leak'"],
  ["dmca_counter_notices", "1 = 1"],
  ["usage_daily", "user_id = ?1"],
  ["pool_audit", "actor_id = ?1 OR target_user_id = ?1"],
  ["pending_blobs", "uploader_id = ?1"],
];

test("27 erasure_resweep: rows an erased account's late writes left (imports, prefs, reports, counter-notices, appeals, counters, limits, pending blobs, served, audit notes) are cleaned daily; what an erasure keeps stays", async () => {
  const { w, adm, A, Z, a } = await world();
  const own = (await publish(w, Z, bankOf([q("Zorgon's own question about tarns and cirques in the high Sierra?")]))).body as any;
  const kept = (await publish(w, Z, bankOf([q("Zorgon's kept question about eskers and kames in a valley?")]), { meta: { title: "Kept", subject: "Geo", course: "", grade: "", tags: [], standards: [] } })).body as any;
  assert.equal((await w.json("POST", `/items/${kept.item_id}/import`, { user: A })).status, 200); // another teacher imported it: kept anonymised
  assert.equal((await w.takedown<any>(adm, await notice(w, adm), own.item_id)).status, 200);
  await w.json("GET", `/items/${a.item_id}`, { user: Z });
  await w.json("POST", `/items/${a.item_id}/import`, { user: Z });
  await w.json("POST", `/items/${a.item_id}/report`, { user: Z, json: { reason: "wrong_answer", note: `my note ${NAME} ${PHONE}` } });
  await w.json("POST", `/items/${own.item_id}/counter-notice`, { user: Z, json: { signature: NAME, material_desc: `mine, ${NAME}`, name: NAME, address: ADDR, phone: PHONE, perjury_stmt: true, consent_jurisdiction: true, accept_service: true } });
  const own2 = (await publish(w, Z, bankOf([q("Zorgon's second question about kettles and drumlins in the plain?")]))).body as any;
  await w.admin(adm, "POST", "/admin/pool/remove", { item_id: own2.item_id, note: "x" });
  await w.json("POST", `/items/${own2.item_id}/appeal`, { user: Z, json: { note: `restore ${NAME} ${PHONE}` } });
  await w.admin(adm, "PUT", "/admin/pool/limits", { user_id: Z.id, metric: "reads_max", value: 5000 });
  w.sql.prepare(`INSERT INTO pool_locks (user_id, token, until) VALUES (?, 't', 1)`).run(Z.id);
  w.sql.prepare(`INSERT INTO admin_flags (id, kind, user_id, detail, dedup, at) VALUES ('f1', 'cap', ?, ?, 'cap:z', 1)`).run(Z.id, JSON.stringify({ note: `cap, ${NAME}`, statement: `st ${NAME}` }));
  const keepers = w.q<any>(`SELECT id, anonymized, removed_at FROM items WHERE author_id = ? ORDER BY id`, Z.id);
  // The snapshot of everything the account holds, taken before its erasure.
  const snap = SNAP.map(([t, where]) => [t, w.q(`SELECT * FROM ${t} WHERE ${where.replaceAll("?1", `'${Z.id}'`)}`)] as const);
  const pendRows = w.q(`SELECT * FROM pending_blobs WHERE uploader_id = ?`, Z.id);
  void pendRows;
  await erase(w, Z);
  const afterErase = scan(w, [Z.id]);
  // The late writes: every snapshot row comes back as it was (note text, names and all).
  for (const [t, rows] of snap)
    for (const r of rows as Q[]) {
      const cols = Object.keys(r);
      w.sql.prepare(`INSERT OR REPLACE INTO ${t} (${cols.join(",")}) VALUES (${cols.map(() => "?").join(",")})`).run(...cols.map((c) => r[c]));
    }
  w.sql.prepare(`INSERT OR REPLACE INTO pool_prefs (user_id, keep_on_delete, updated_at) VALUES (?, 1, 1)`).run(Z.id);
  w.sql.prepare(`INSERT OR REPLACE INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'reads_max', 5000, 1, ?)`).run(Z.id, adm.id);
  w.sql.prepare(`INSERT OR REPLACE INTO pool_locks (user_id, token, until) VALUES (?, 't', 1)`).run(Z.id);
  w.sql.prepare(`INSERT OR REPLACE INTO usage_daily (user_id, day, metric, count) VALUES (?, 1, 'reports', 3)`).run(Z.id);
  w.sql.prepare(`INSERT OR REPLACE INTO imports (user_id, item_id, publish_id, version, form, license, author_id, at, kind) VALUES (?, ?, 'late', 1, 'full', 'CC-BY-4.0', ?, 1, 'import')`).run(Z.id, a.item_id, A.id);
  assert.ok((scan(w, [Z.id]).imports ?? 0) > 0 && (scan(w, PII).dmca_counter_notices ?? 0) > 0, "the late rows are there (so the test is not vacuous)");
  w.advance(DAY);
  await w.pool.daily();
  await settle(w);
  assert.deepEqual(scan(w, PII), {}, "no name, phone or address text anywhere");
  const rows = scan(w, [Z.id]);
  assert.equal(w.q(`SELECT 1 FROM imports WHERE user_id = ?`, Z.id).length, 0, "imports re-keyed (rows of other importers of its items stay)");
  for (const t of ["pool_prefs", "reports", "usage_daily", "pool_limits", "pending_blobs", "pool_locks", "served"]) assert.equal(rows[t] ?? 0, 0, `${t} holds no row of the erased account`);
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'appeal' AND resolved_at IS NULL`).length, 0, "the appeal closed");
  assert.equal(w.q<any>(`SELECT purged_at IS NOT NULL AS p FROM items WHERE id = ?`, own2.item_id)[0].p, 1, "and its removed item was purged");
  // What an erasure keeps, unchanged.
  assert.equal(w.q(`SELECT 1 FROM users WHERE id = ? AND deleted_at IS NOT NULL`, Z.id).length, 1, "tombstone");
  assert.equal(w.q(`SELECT 1 FROM erasure_done WHERE user_id = ?`, Z.id).length, 1, "erasure_done");
  assert.equal(w.q(`SELECT 1 FROM pool_audit WHERE action = 'account_deleted.content_removed' AND target_user_id = ?`, Z.id).length, 1, "audit row without a note");
  const kItem = w.q<any>(`SELECT anonymized, removed_at FROM items WHERE id = ?`, kept.item_id)[0];
  assert.equal(kItem.anonymized, 1, "the item another teacher imported is kept, anonymised");
  assert.equal(kItem.removed_at, null);
  assert.ok(keepers.length >= 3);
  assert.ok(Object.keys(afterErase).length >= 1);
  assert.equal(w.q(`SELECT 1 FROM attestations WHERE item_id = ?`, kept.item_id).length, 1, "attestations stay (documented)");
  // idempotent: another run changes nothing
  const before = JSON.stringify(scan(w, [Z.id]));
  w.advance(DAY);
  await w.pool.daily();
  assert.equal(JSON.stringify(scan(w, [Z.id])), before);
});

// ---- the guard in the write, one statement at a time -----------------------------------------------------

test("27 guards: with the account erased, a request that is somehow still authenticated writes nothing and answers 401 (every route that writes under the caller's id)", async () => {
  const { w, adm, A, Z, a } = await world();
  const own = (await publish(w, Z, bankOf([q("Zorgon's own question about tarns and cirques in the high Sierra?")]))).body as any;
  await w.admin(adm, "POST", "/admin/pool/remove", { item_id: own.item_id, note: "x" });
  // An auth layer that still believes the token (the race): the pool's own writes must refuse.
  const pool = (w as any).pool;
  void pool;
  await erase(w, Z);
  const live = new Map(w.tokens);
  void live;
  // A second pool over the same db whose authenticate ignores deleted_at (the straddling request).
  const { createPool } = await import("../../src/pool/index.ts");
  const { core: sharedCore, SECRET, API: BASE } = await import("./harness.ts");
  const p2 = createPool({
    db: w.db,
    r2: w.r2 as any,
    core: sharedCore,
    secret: SECRET,
    allowedOrigins: [],
    now: () => w.clock,
    rateStore: { hit: async () => ({ allowed: true, retryAfter: 0 }), undo: async () => {} } as any,
    ipBucket: (ip: string) => ip,
    authenticate: async (req: Request) => {
      const t = /^Bearer (.+)$/.exec(req.headers.get("Authorization") ?? "")?.[1];
      const id = t ? w.tokens.get(t) : undefined;
      return id ? { userId: id, sessionFamily: "f", trust: "teacher" } : null;
    },
    notify: async () => {},
    background: () => {},
  } as any);
  const call = async (method: string, path: string, body?: unknown, raw?: Uint8Array) => {
    const h = new Headers({ Authorization: `Bearer ${Z.token}`, "CF-Connecting-IP": "10.0.0.9" });
    let b: BodyInit | undefined;
    if (body !== undefined) {
      b = JSON.stringify(body);
      h.set("Content-Type", "application/json");
      h.set("Content-Length", String(new TextEncoder().encode(b).length));
    }
    if (raw) {
      b = raw;
      h.set("Content-Length", String(raw.length));
    }
    const r = await p2.handle(new Request(`${BASE}${path}`, { method, headers: h, body: b }));
    return r!.status;
  };
  const counter = { signature: NAME, material_desc: "m", name: NAME, address: ADDR, phone: PHONE, perjury_stmt: true, consent_jurisdiction: true, accept_service: true };
  assert.equal(await call("GET", `/items/${a.item_id}`), 401);
  assert.equal(await call("POST", `/items/${a.item_id}/import`), 401);
  assert.equal(await call("PUT", "/items/prefs", { keep_on_delete: true }), 401);
  assert.equal(await call("POST", `/items/${a.item_id}/report`, { reason: "wrong_answer", note: NAME }), 401);
  assert.equal(await call("POST", `/items/${own.item_id}/appeal`, { note: NAME }), 401);
  assert.equal(await call("POST", `/items/${own.item_id}/counter-notice`, counter), 401);
  assert.equal(await call("PUT", `/blobs/${"a".repeat(64)}`, undefined, png(9, 9, 6)), 401);
  assert.equal(await call("GET", `/items/${"f".repeat(32)}`), 401, "a miss is no 404 either");
  assert.deepEqual(scan(w, PII), {});
  const rows = scan(w, [Z.id]);
  for (const t of ["imports", "served", "pool_prefs", "reports", "usage_daily", "pending_blobs", "pool_locks", "blob_uploads"]) assert.equal(rows[t] ?? 0, 0, `${t}: nothing written for the erased account`);
  assert.equal(w.q(`SELECT 1 FROM admin_flags WHERE kind = 'appeal' AND resolved_at IS NULL`).length, 0);
  void framed;
});
