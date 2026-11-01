// Round 20 (critic25a E2, critic25b F3/F5): evidence voids made before 0104 survive it (an item void
// marks the item and every serve of it; a reader void every serve of the reader; serves whose old rows
// were all voided stay voided), on the committed 0102-era fixtures; anonymised serves of erased readers
// get no served rows; 0104 refuses to run twice (its pre-flight check).
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { MemR2, publish, World, type User } from "./harness.ts";

const here = new URL("../../", import.meta.url).pathname;
const fx = (f: string) => readFileSync(`${here}test/pool/fixtures/${f}`, "utf8");
const bankOf = (qs: unknown[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const META = JSON.parse(fx("m7_0102_meta.json"));
const audit = (db: DatabaseSync, id: string, action: string, item: string | null, user: string | null, at: number) =>
  db.prepare(`INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id) VALUES (?, ?, 'adm', 'admin', ?, ?, ?)`).run(id, at, action, item, user);

function migrated(o: { before0103?: (db: DatabaseSync) => void; before0104?: (db: DatabaseSync) => void } = {}): { w: World; tess: User; db: DatabaseSync } {
  const db = new DatabaseSync(":memory:");
  for (const f of readdirSync(`${here}migrations`).filter((x) => x.endsWith(".sql") && x < "0103").sort()) db.exec(readFileSync(`${here}migrations/${f}`, "utf8"));
  db.exec(fx("m7_0102_dump.sql"));
  o.before0103?.(db);
  db.exec(readFileSync(`${here}migrations/0103_pool_rounds_6_11.sql`, "utf8"));
  o.before0104?.(db);
  db.exec(readFileSync(`${here}migrations/0104_pool_round12.sql`, "utf8"));
  db.exec(readFileSync(`${here}migrations/0105_pool_round24.sql`, "utf8"));
  db.exec(readFileSync(`${here}migrations/0106_pool_round28.sql`, "utf8"));
  // (Round 32: and every later migration, as a deployment applies them: 0107, 0108 (circles).)
  db.exec(readFileSync(`${here}migrations/0107_pool_round30.sql`, "utf8"));
  db.exec(readFileSync(`${here}migrations/0108_pool_round32.sql`, "utf8"));
  db.exec(readFileSync(`${here}migrations/0109_pool_round34.sql`, "utf8"));
  const r2 = new MemR2();
  for (const [k, b64, ct] of JSON.parse(fx("m7_0102_r2.json")) as [string, string, string | null][]) r2.objects.set(k, { bytes: new Uint8Array(Buffer.from(b64, "base64")), ...(ct ? { contentType: ct } : {}) });
  const w = new World({ sql: db, r2, backfillItems: 50 });
  w.clock = META.clock + 86_400_000;
  (w as unknown as { n: number }).n = 100;
  w.tokens.set(`tok-${META.tess}`, META.tess);
  return { w, db, tess: { id: META.tess, token: `tok-${META.tess}`, name: "Tess" } };
}
async function tessCc0(w: World, tess: User) {
  await w.pool.hourly();
  const p = await publish(w, tess, bankOf(META.served), { license: "CC0-1.0" });
  return p.status === 201 ? "201" : `${p.status} ${p.body.error}`;
}

test("E2: an ITEM evidence void made before 0104 (rows and audit, as HEAD's takedown purge and admin void wrote them) carries over", async () => {
  const control = migrated();
  assert.equal(await tessCc0(control.w, control.tess), "422 license_incompatible", "control: bound");
  for (const when of ["0102", "0103"] as const) {
    const voidIt = (db: DatabaseSync) => {
      db.prepare(`UPDATE import_questions SET void_at = 5 WHERE item_id = ?`).run(META.item);
      audit(db, `a-${when}`, "evidence.void", META.item, null, 5);
    };
    const { w, tess } = migrated(when === "0102" ? { before0103: voidIt } : { before0104: voidIt });
    assert.equal(w.q(`SELECT * FROM void_items WHERE item_id = ?`, META.item).length, 1, when);
    assert.ok(w.q<{ v: number | null }>(`SELECT void_at AS v FROM served WHERE item_id = ?`, META.item).every((x) => x.v !== null), when);
    assert.equal(await tessCc0(w, tess), "201", `${when}: as on a fresh database`);
  }
  // A void restored later stays restored.
  const { w, tess } = migrated({
    before0103: (db) => {
      audit(db, "a1", "evidence.void", META.item, null, 5);
      audit(db, "a2", "evidence.restore", META.item, null, 6);
    },
  });
  assert.equal(w.q(`SELECT * FROM void_items`).length, 0);
  assert.equal(await tessCc0(w, tess), "422 license_incompatible");
});

test("E2: a READER evidence void made before 0104 carries over; rows all voided without an audit stay voided (reader's own serves only)", async () => {
  for (const audited of [true, false]) {
    const { w, tess } = migrated({
      before0104: (db) => {
        db.prepare(`UPDATE import_questions SET void_at = 5 WHERE user_id = ?`).run(META.tess);
        if (audited) audit(db, "u1", "evidence.void", null, META.tess, 5);
      },
    });
    assert.ok(w.q<{ v: number | null }>(`SELECT void_at AS v FROM served WHERE reader_id = ?`, META.tess).every((x) => x.v !== null), `audited=${audited}`);
    assert.equal(w.q(`SELECT * FROM void_items`).length, 0, "a reader void never voids the item for everyone");
    assert.equal(await tessCc0(w, tess), "201", `audited=${audited}`);
  }
});

test("F3 (N3): an erased reader's anonymised serves get no served row in 0104", () => {
  const { w } = migrated({ before0104: (db) => db.prepare(`UPDATE imports SET user_id = 'deleted:' || 'abc' WHERE user_id = ?`).run(META.tess) });
  assert.equal(w.q(`SELECT * FROM served WHERE reader_id LIKE 'deleted:%'`).length, 0);
  assert.ok(w.q(`SELECT * FROM imports WHERE user_id LIKE 'deleted:%'`).length > 0, "the counts stay");
});

test("F5: 0104 has a pre-flight check: a second run stops at once with a clear error, changing nothing", () => {
  const { db } = migrated();
  const before = (db.prepare(`SELECT COUNT(*) AS n FROM served`).get() as { n: number }).n;
  assert.throws(() => db.exec(readFileSync(`${here}migrations/0104_pool_round12.sql`, "utf8")), /pool_migration_0104_already_applied already exists/);
  assert.equal((db.prepare(`SELECT COUNT(*) AS n FROM served`).get() as { n: number }).n, before);
});

test("round 21 (critic27a M-B4): a void made BEFORE a serve never voids that serve (as HEAD's voids touched only rows that existed)", async () => {
  // The fixture's first serve, less a day: the void predates every serve.
  const db0 = new DatabaseSync(":memory:");
  for (const f of readdirSync(`${here}migrations`).filter((x) => x.endsWith(".sql") && x < "0103").sort()) db0.exec(readFileSync(`${here}migrations/${f}`, "utf8"));
  db0.exec(fx("m7_0102_dump.sql"));
  const T0 = (db0.prepare(`SELECT MIN(at) AS t FROM imports`).get() as { t: number }).t - 86_400_000;
  for (const [name, before] of [
    ["reader void", (db: DatabaseSync) => audit(db, "v1", "evidence.void", null, META.tess, T0)],
    ["void, restore, void", (db: DatabaseSync) => (audit(db, "v1", "evidence.void", null, META.tess, T0 - 2), audit(db, "v2", "evidence.restore", null, META.tess, T0 - 1), audit(db, "v3", "evidence.void", null, META.tess, T0))],
    // (an item void of ANOTHER item: Tess's serve of the copied item stays; an item void of the item
    // itself makes it void for everyone, as on a fresh database)
    ["another item's void", (db: DatabaseSync) => audit(db, "v1", "evidence.void", META.hidden, null, T0)],
  ] as const) {
    const { w, tess } = migrated({ before0104: before });
    assert.ok(w.q<{ v: number | null }>(`SELECT void_at AS v FROM served WHERE reader_id = ? AND item_id = ?`, META.tess, META.item).every((x) => x.v === null), name);
    assert.equal(await tessCc0(w, tess), "422 license_incompatible", name);
  }
});
