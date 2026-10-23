// PUT /blobs/:sha256 (docs/shared-pool.md "Blob rules", "Images"). Full bytes always; the hash is
// recomputed; the format is sniffed from the bytes; the blob is charged to the uploader as pending
// (quota checked in the same INSERT); the answer is the same whether or not the blob existed, so an
// upload is never an existence oracle.
import { MAX_IMAGE_BYTES, pngPixelHash } from "./image.ts";
import { ALIVE_SQL, throwIfErased, json, limitOf, PoolError, usage, readBody, requireDaily, REMOVED_SQL, SHA_RE, sha256Hex, USAGE_SQL, DAY_MS, type Ctx, type PoolPrincipal } from "./shared.ts";

/** A pending upload stays charged (and publishable) this long. */
export const PENDING_MS = DAY_MS;

export async function putBlob(c: Ctx, req: Request, p: PoolPrincipal, sha: string): Promise<Response> {
  const me = p.userId;
  if (!SHA_RE.test(sha)) throw new PoolError(400, "invalid_request", { field: "sha256" });
  // Charged BEFORE the body is read (round 7 H3): the day's uploads and the declared bytes; then the
  // body is read by the one body reader (length required and exact, budget, deadline, per-account cap).
  const declared = req.headers.get("Content-Length");
  if (declared === null) throw new PoolError(411, "length_required", { message: "Send the image with a Content-Length." });
  if (!/^\d{1,12}$/.test(declared) || Number(declared) > MAX_IMAGE_BYTES) throw new PoolError(413, "body_too_large", { limit: MAX_IMAGE_BYTES });
  await requireDaily(c, me, "blob_uploads");
  await requireDaily(c, me, "blob_bytes", Number(declared));
  const body = await readBody(req, { max: MAX_IMAGE_BYTES, key: `u:${me}`, deadlineMs: c.deps.bodyDeadlineMs });
  try {
    return await storeBlob(c, me, sha, body.bytes);
  } finally {
    body.release();
  }
}

async function storeBlob(c: Ctx, me: string, sha: string, bytes: Uint8Array): Promise<Response> {
  if ((await sha256Hex(bytes)) !== sha) throw new PoolError(422, "hash_mismatch");
  // ONE answer for every refused image (round 6): bytes confirmed as student data get exactly what a
  // malformed or unsupported file gets, after the same work, so an upload never confirms a privacy
  // block. The app explains structural problems itself (it runs the same checks before uploading).
  // Round 16 (critic21d 5): the uploader knows whether their own file is well formed, so refusing a
  // blocked image would confirm the block. A well-formed blocked image is ACCEPTED exactly like any
  // other (same answer, same charge, an R2 write of the same size), but its bytes are never stored:
  // a zero-filled stand-in goes to a quarantine key, and a publish that serves it is held by the
  // block (student_data), never refused.
  const info = await c.core.imageInfo(bytes);
  if ("refused" in info) throw new PoolError(422, "image_refused", { reason: "not_accepted" });
  const blocked = (await c.db.prepare(`SELECT 1 AS x FROM privacy_blocks WHERE kind = 'blob' AND hash = ?1`).bind(sha).first()) !== null;
  const now = c.now();
  const quota = await limitOf(c, me, "storage_bytes");
  // Charged once per (blob, uploader): a blob this author already pays for (pending or in one of
  // their publishes) costs nothing more. Another author's copy of the same image costs this one too.
  const pend = await c.db
    .prepare(
      `INSERT INTO pending_blobs (sha256, uploader_id, size, until)
       SELECT ?3, ?1, ?4, ?5
        WHERE ${ALIVE_SQL("?1")} AND (CASE WHEN EXISTS (SELECT 1 FROM pending_blobs WHERE sha256 = ?3 AND uploader_id = ?1 AND until >= ?2)
                      OR EXISTS (SELECT 1 FROM item_blobs ib JOIN item_publishes ip ON ip.publish_id = ib.publish_id WHERE ib.sha256 = ?3 AND ip.author_id = ?1)
                    THEN 0 ELSE ?4 END) + ${USAGE_SQL} <= ?6
         -- Round 17 (critic22e): and not past the removed-items cap (deleted items' bodies and images).
         AND ${REMOVED_SQL} <= ?7
       ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = excluded.until, size = excluded.size`,
    )
    .bind(me, now, sha, bytes.length, now + PENDING_MS, quota, await limitOf(c, me, "removed_bytes"))
    .run();
  if ((pend.meta.changes ?? 0) === 0) await throwIfErased(c, me); // (round 27, critic35d B-1)
  if ((pend.meta.changes ?? 0) === 0) throw new PoolError(413, "quota_exceeded", { used: await usage(c, me), needed: bytes.length, limit: quota });
  // Round 17 (critic22e S8): when this account first uploaded these bytes (ordering evidence for an
  // image independence claim: "my own photo, which I had before that item was published").
  await c.db.prepare(`INSERT OR IGNORE INTO blob_uploads (sha256, uploader_id, first_at) SELECT ?1, ?2, ?3 WHERE ${ALIVE_SQL("?2")}`).bind(sha, me, now).run();
  // Content-addressed: rewriting identical bytes is harmless, and it repairs a GC that raced us.
  if (blocked) {
    await c.r2.put(`quarantine/upload/${me}`, new Uint8Array(bytes.length), { httpMetadata: { contentType: info.contentType } });
    // The stand-in is removed within a day by the orphan sweep (round 17: no stand-ins kept).
    await c.db.prepare(`INSERT INTO r2_orphans (key, at, next_at) VALUES (?1, ?2, ?3) ON CONFLICT (key) DO UPDATE SET next_at = excluded.next_at`).bind(`quarantine/upload/${me}`, now, now + DAY_MS).run();
  }
  else await c.r2.put(`blobs/${sha}`, bytes, { httpMetadata: { contentType: info.contentType } });
  await c.db
    // A fresh upload brings back an image whose privacy grace window had ended (after an unblock);
    // never one still blocked (?7).
    // Round 17 (critic22c M5a): the decoded-pixel hash (PNG), so a re-encode is the same image.
    .prepare(`INSERT INTO blobs (sha256, size, content_type, width, height, first_uploaded_at, checked_at, pixel_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?8)
             ON CONFLICT (sha256) DO UPDATE SET missing_at = CASE WHEN ?7 = 1 THEN missing_at ELSE NULL END, privacy_purged_at = CASE WHEN ?7 = 1 THEN privacy_purged_at ELSE NULL END, checked_at = ?6,
                                                pixel_hash = COALESCE(blobs.pixel_hash, excluded.pixel_hash)`)
    .bind(sha, bytes.length, info.contentType, info.width, info.height, now, blocked ? 1 : 0, info.contentType === "image/png" ? await pngPixelHash(bytes) : null)
    .run();
  return json(201, { sha256: sha, size: bytes.length, width: info.width, height: info.height });
}
