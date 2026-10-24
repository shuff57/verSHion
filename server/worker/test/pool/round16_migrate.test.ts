// Round 16 (critic21d B8): a database populated by the committed 0102-era worker (1d311ff; fixtures
// written by fixtures/m7_populate_0102.ts.txt) migrates through 0103 / 0104 with its evidence kept:
// the importer who republishes the imported question under CC0 is refused, as on a fresh database,
// before and after the cron backfill fills in the hashes rounds 6-16 added.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { MemR2, publish, World, type User } from "./harness.ts";

const here = new URL("../../", import.meta.url).pathname;
const fx = (f: string) => readFileSync(`${here}test/pool/fixtures/${f}`, "utf8");
const bankOf = (qs: unknown[]) => JSON.stringify({ schema_version: 2, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });

function migrated(before?: (db: DatabaseSync) => void): { w: World; ann: User; tess: User; meta: any } {
  const db = new DatabaseSync(":memory:");
  // Every migration up to the committed 0102 (git 1d311ff: 0001-0102), then its data.
  for (const f of readdirSync(`${here}migrations`).filter((x) => x.endsWith(".sql") && x < "0103").sort()) db.exec(readFileSync(`${here}migrations/${f}`, "utf8"));
  db.exec(fx("m7_0102_dump.sql"));
  before?.(db);
  // (Round 32: and every later migration, 0107 and 0108 (circles), as a deployment applies them.)
  for (const f of ["0103_pool_rounds_6_11.sql", "0104_pool_round12.sql", "0105_pool_round24.sql", "0106_pool_round28.sql", "0107_pool_round30.sql", "0108_pool_round32.sql", "0109_pool_round34.sql", "0110_pool_round38.sql"]) db.exec(readFileSync(`${here}migrations/${f}`, "utf8"));
  const r2 = new MemR2();
  for (const [k, b64, ct] of JSON.parse(fx("m7_0102_r2.json")) as [string, string, string | null][]) r2.objects.set(k, { bytes: new Uint8Array(Buffer.from(b64, "base64")), ...(ct ? { contentType: ct } : {}) });
  const meta = JSON.parse(fx("m7_0102_meta.json"));
  const w = new World({ sql: db, r2, backfillItems: 50 });
  w.clock = meta.clock + 86_400_000;
  (w as unknown as { n: number }).n = 100;
  const as = (id: string, name: string): User => {
    const token = `tok-${id}`;
    w.tokens.set(token, id);
    return { id, token, name };
  };
  return { w, ann: as(meta.ann, "Ann"), tess: as(meta.tess, "Tess"), meta };
}

test("B8: pre-migration evidence survives 0103/0104: the importer's CC0 republish is refused, before and after the backfill", async () => {
  const { w, tess, meta } = migrated();
  // (Round 19: one served row per (reader, served publish), from the 0102 imports.)
  assert.equal(w.q(`SELECT * FROM served`).length, 2, "both serves kept");
  assert.equal(w.q(`SELECT * FROM import_questions`).length, 2, "and read through the served publishes' rows");
  const copy = meta.served as unknown[];
  // Round 17 (critic22e A2): no publish or import while the backfill has items to do.
  const before = await publish(w, tess, bankOf(copy), { license: "CC0-1.0" });
  assert.deepEqual([before.status, before.body.error], [503, "pool_migrating"]);
  assert.equal((await w.json("POST", `/items/${meta.item}/import`, { user: tess })).status, 503);
  // The backfill fills in the hashes; the evidence rows follow.
  await w.pool.hourly();
  assert.equal(w.q(`SELECT * FROM import_questions WHERE near_hash IS NULL OR near_kf IS NULL`).length, 0);
  assert.equal(w.q(`SELECT * FROM item_questions WHERE near_full IS NULL OR near_kf IS NULL OR stem_bands IS NULL`).length, 0);
  // ... and the original author's durable records are written by the backfill too.
  assert.ok(w.q(`SELECT * FROM author_contents WHERE author_id = ? AND kind = 'nshown'`, meta.ann).length > 0);
  assert.ok(w.q(`SELECT * FROM author_firsts WHERE author_id = ?`, meta.ann).length > 0);
  const after = await publish(w, tess, bankOf(copy), { license: "CC0-1.0" });
  assert.equal(after.body.error, "license_incompatible");
  // ... the same as on a fresh database: a compatible licence goes through, crediting Ann.
  const ok = await publish(w, tess, bankOf(copy), { license: "CC-BY-NC-SA-4.0" });
  assert.equal(ok.status, 201, JSON.stringify(ok.body).slice(0, 300));
  const g = await w.json<any>("GET", `/items/${ok.body.item_id}`, { user: w.user() });
  assert.deepEqual(g.body.questions[0].chain.map((l: any) => l.author_name), ["Ann"]);
});

test("0103 aggregates the committed schema's evidence per served content: an import wins over a read, every licence shown is kept (critic22e S6)", async () => {
  const { w, meta } = migrated((db) => {
    // Tess also READ an earlier publish of the same content under NC-SA (another 0102 row, same content).
    const row = db.prepare(`SELECT * FROM import_questions WHERE user_id = ? AND item_id = ?`).get(JSON.parse(fx("m7_0102_meta.json")).tess, JSON.parse(fx("m7_0102_meta.json")).item) as Record<string, unknown>;
    db.prepare(`INSERT INTO imports (user_id, item_id, publish_id, version, form, license, author_id, at, kind) VALUES (?, ?, 'EarlierPublishOfAnnsItem', 1, 'full', 'CC-BY-4.0', ?, 1, 'view')`).run(row.user_id as string, row.item_id as string, JSON.parse(fx("m7_0102_meta.json")).ann);
    db.prepare(`INSERT INTO import_questions (user_id, publish_id, qid, item_id, match_hash, content_hash, keyfree_hash, chain_json, author_name, source_first_seq, stem_hash) VALUES (?, 'EarlierPublishOfAnnsItem', ?, ?, ?, ?, ?, '[]', 'Ann', 0, NULL)`)
      .run(row.user_id as string, row.qid as string, row.item_id as string, row.match_hash as string, row.content_hash as string, row.keyfree_hash as string);
  });
  // (Round 19: one served row per served publish; the earlier publish, gone, is a ghost.)
  const sv = w.q<{ kind: string; licenses_json: string }>(`SELECT kind, licenses_json FROM served WHERE reader_id = ? AND item_id = ? ORDER BY kind`, meta.tess, meta.item);
  // (0103 had united the licences of the one content: the served row of the retained publish keeps both)
  assert.deepEqual(sv.map((x) => [x.kind, (JSON.parse(x.licenses_json) as string[]).sort()]), [["import", ["CC-BY-4.0", "CC-BY-NC-SA-4.0"]], ["view", ["CC-BY-4.0"]]]);
  assert.equal(w.q(`SELECT * FROM ghost_publishes WHERE publish_id = 'EarlierPublishOfAnnsItem'`).length, 1);
  // ... and every licence shown is kept: the same content was shown CC-BY too, so a CC-BY copy goes through.
  await w.pool.hourly();
  const { publish: pub } = await import("./harness.ts");
  const ok = await pub(w, { id: meta.tess, token: `tok-${meta.tess}`, name: "Tess" }, bankOf(meta.served as unknown[]), { license: "CC-BY-4.0" });
  assert.equal(ok.status, 201, JSON.stringify(ok.body).slice(0, 300));
});
