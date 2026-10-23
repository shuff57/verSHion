// Self-service account export (round 23, owner): GET /me/export. Everything the pool holds about the
// account that is the account's own: its profile, its items (the FULL form of each current publish:
// the author's own hidden keys included), its imports (ids, titles, the licence shown, dates; no one
// else's content or personal data), its reports, the notices it received, its attestations, its
// counter-notices and appeals, its sessions (no tokens). Never: another user's personal data
// (reporters, complainants, emails), admin notes, internal flags, hashes or sketches.
import {
  audit,
  json,
  PoolError,
  requireDaily,
  reserveBytes,
  VISIBLE_TO_OTHERS,
  type Ctx,
  type PoolPrincipal,
} from "./shared.ts";

/** A whole export above `whole` bytes of item bodies goes out as a manifest and parts of at most `part` (tests lower them). */
export const EXPORT_LIMITS = { whole: 8 * 1024 * 1024, part: 4 * 1024 * 1024 };
/** The session must have logged in this recently. */
export const EXPORT_FRESH_MS = 24 * 3600_000;

const HEADERS = (name: string) => ({
  "Content-Disposition": `attachment; filename="${name}"`,
  "Cache-Control": "no-store",
  "X-Content-Type-Options": "nosniff",
});

type ItemRow = {
  id: string;
  title: string;
  subject: string;
  license: string;
  visibility: string;
  removed_reason: string | null;
  created_at: number;
  updated_at: number;
  current_publish_id: string | null;
  full_key: string | null;
  full_bytes: number | null;
};

/** One retained version of one of the account's items (round 24: exported with its own full body). */
type PubRow = {
  item_id: string;
  publish_id: string;
  version: number;
  status: string;
  license: string;
  committed_at: number | null;
  full_key: string | null;
  full_bytes: number | null;
  hide_key: number;
};

export async function exportAccount(
  c: Ctx,
  req: Request,
  p: PoolPrincipal,
): Promise<Response> {
  const me = p.userId;
  const user = await c.db
    .prepare(
      `SELECT id, email, display_name, profile, trust, created_at, email_verified_at, deleted_at FROM users WHERE id = ?1`,
    )
    .bind(me)
    .first<{
      id: string;
      email: string | null;
      display_name: string | null;
      profile: string | null;
      trust: string;
      created_at: number;
      email_verified_at: number | null;
      deleted_at: number | null;
    }>();
  if (!user || user.deleted_at !== null) throw new PoolError(404, "not_found");
  const full = p.trust === "teacher" || p.trust === "admin";
  if (!full && p.trust !== "pending")
    throw new PoolError(403, "account_not_approved");
  // A fresh session: logged in within the last 24 hours (no re-authentication otherwise).
  const fresh = await c.db
    .prepare(
      `SELECT MAX(auth_at) AS a FROM sessions WHERE family_id = ?1 AND user_id = ?2 AND revoked_at IS NULL`,
    )
    .bind(p.sessionFamily ?? "", me)
    .first<{ a: number | null }>();
  if (!fresh?.a || fresh.a < c.now() - EXPORT_FRESH_MS)
    throw new PoolError(403, "reauth_required", {
      message:
        "Sign in again to export your account (an export needs a login in the last 24 hours).",
    });
  const url = new URL(req.url);
  const partArg = url.searchParams.get("part");
  const part =
    partArg === null
      ? null
      : /^[1-9][0-9]{0,4}$/.test(partArg)
        ? Number(partArg)
        : NaN;
  if (Number.isNaN(part))
    throw new PoolError(400, "invalid_request", { field: "part" });
  // Three exports a day (a part of one already counted does not count again).
  if (part === null) await requireDaily(c, me, "exports");
  let profile: unknown = null;
  try {
    profile = user.profile ? JSON.parse(user.profile) : null;
  } catch {
    profile = null;
  }
  const head = {
    format: "vershion-pool-export/v1",
    exported_at: c.now(),
    profile: {
      id: user.id,
      email: user.email,
      display_name: user.display_name,
      profile,
      approval: user.trust,
      created_at: user.created_at,
      email_verified_at: user.email_verified_at,
    },
  };
  if (!full) {
    if (part !== null) throw new PoolError(404, "not_found");
    await c.db.batch([
      audit(
        c.db,
        {
          actorId: me,
          actorKind: "user",
          action: "account.export",
          targetUserId: me,
          detail: { profile_only: true },
        },
        c.now(),
      ),
    ]);
    return json(
      200,
      { ...head, profile_only: true },
      HEADERS("vershion-export.json"),
    );
  }
  const items = (
    await c.db
      .prepare(
        `SELECT i.id, i.title, i.subject, i.license, i.visibility, i.removed_reason, i.created_at, i.updated_at, i.current_publish_id, p.full_key, p.full_bytes
           FROM items i LEFT JOIN item_publishes p ON p.publish_id = i.current_publish_id
          WHERE i.author_id = ?1 AND i.purged_at IS NULL ORDER BY i.created_at, i.id`,
      )
      .bind(me)
      .all<ItemRow>()
  ).results;
  // Round 24 (critic31a/31b): every RETAINED version, with its own full body, not only the current
  // one. `retain()` keeps current, the previous one and the one non-authors are served.
  const retained = (
    await c.db
      .prepare(
        `SELECT item_id, publish_id, version, status, license, committed_at, full_key, full_bytes, hide_key
           FROM item_publishes WHERE author_id = ?1 AND status IN ('current', 'previous') ORDER BY item_id, version`,
      )
      .bind(me)
      .all<PubRow>()
  ).results;
  const retainedOf = new Map<string, PubRow[]>();
  for (const v of retained) retainedOf.set(v.item_id, [...(retainedOf.get(v.item_id) ?? []), v]);
  const itemBytes = new Map<string, number>();
  for (const it of items) itemBytes.set(it.id, (retainedOf.get(it.id) ?? [{ full_bytes: it.full_bytes } as PubRow]).reduce((a, v) => a + (v.full_bytes ?? 0), 0));
  // Parts: the items' retained bodies in order, at most EXPORT_LIMITS.part of stored bytes each (one
  // oversized item is its own part).
  const parts: string[][] = [];
  let cur: string[] = [];
  let size = 0;
  for (const it of items) {
    const b = itemBytes.get(it.id) ?? 0;
    if (cur.length > 0 && size + b > EXPORT_LIMITS.part) {
      parts.push(cur);
      cur = [];
      size = 0;
    }
    cur.push(it.id);
    size += b;
  }
  if (cur.length > 0) parts.push(cur);
  const total = [...itemBytes.values()].reduce((a, x) => a + x, 0);
  /** The full bodies of `pairs` (id -> R2 key), held against the isolate's byte budget twice. */
  const readBodies = async (pairs: { id: string; key: string | null; bytes: number | null }[]) => {
    const out: Record<string, unknown> = {};
    const release = reserveBytes(2 * pairs.reduce((a, p) => a + (p.bytes ?? 0), 0));
    try {
      for (const p of pairs) {
        if (!p.key) continue;
        const o = await c.r2.get(p.key);
        if (!o) continue;
        try {
          out[p.id] = JSON.parse(await o.text()); // r2-object
        } catch {
          out[p.id] = null;
        }
      }
    } finally {
      release();
    }
    return out;
  };
  const currentPub = new Set(items.map((x) => x.current_publish_id).filter((x): x is string => x !== null));
  const currentPairs = (ids: Set<string>) =>
    items.filter((x) => ids.has(x.id)).map((it) => ({ id: it.id, key: it.full_key, bytes: it.full_bytes }));
  // (Round 25: the current publish's body is `bodies[item_id]`; it is not repeated in version_bodies,
  // so a part's wire size is what its byte accounting reserved.)
  const versionPairs = (ids: Set<string>) =>
    retained.filter((v) => ids.has(v.item_id) && !currentPub.has(v.publish_id)).map((v) => ({ id: v.publish_id, key: v.full_key, bytes: v.full_bytes }));
  if (part !== null) {
    const ids = parts[part - 1];
    if (!ids) throw new PoolError(404, "not_found");
    const set = new Set(ids);
    return json(
      200,
      {
        format: "vershion-pool-export/v1",
        part,
        of: parts.length,
        bodies: await readBodies(currentPairs(set)),
        version_bodies: await readBodies(versionPairs(set)),
      },
      HEADERS(`vershion-export-part-${part}.json`),
    );
  }
  const q = async <T>(sql: string) =>
    (await c.db.prepare(sql).bind(me).all<T>()).results;
  const data = {
    ...head,
    items: items.map((it) => ({
      id: it.id,
      title: it.title,
      subject: it.subject,
      license: it.license,
      visibility: it.visibility,
      state: it.removed_reason ?? "live",
      created_at: it.created_at,
      updated_at: it.updated_at,
      versions: (retainedOf.get(it.id) ?? []).map(({ item_id: _i, full_key: _k, full_bytes: _b, hide_key: _h, ...v }) => v),
    })),
    // What it imported or read: the item, its title, the licence it was shown, when. No content.
    // Round 24 (critic31a/31b): an item that is no longer visible to others (unlisted, held, removed)
    // has no title here: its author's title is private, and an importer is not its author.
    imports: await q(
      `SELECT im.item_id, (SELECT title FROM items i JOIN users u ON u.id = i.author_id WHERE i.id = im.item_id AND ${VISIBLE_TO_OTHERS}) AS title, im.kind, im.version, im.license, im.at FROM imports im WHERE im.user_id = ?1 ORDER BY im.at`,
    ),
    reports: await q(
      `SELECT item_id, reason, note, at, resolved_at, action FROM reports WHERE reporter_id = ?1 ORDER BY at`,
    ),
    // Notices it received: takedowns of its items (no complainant identity) and the mails sent to it.
    takedowns: await q(
      `SELECT id AS action_id, item_id, disabled_at, counter_notice_id IS NOT NULL AS countered, restored_at, purged_at, outcome FROM dmca_actions WHERE author_id = ?1 ORDER BY disabled_at`,
    ),
    notices: (
      await q<{ kind: string; at: number; payload: string }>(
        `SELECT kind, at, payload FROM pool_outbox WHERE json_extract(payload, '$.authorId') = ?1 ORDER BY at`,
      )
    ).map((n) => {
      const x = JSON.parse(n.payload) as Record<string, unknown>;
      return {
        kind: n.kind,
        at: n.at,
        item_id: x.itemId ?? null,
        action_id: x.actionId ?? null,
      };
    }),
    attestations: await q(
      `SELECT publish_id, item_id, at, terms_version FROM attestations WHERE user_id = ?1 ORDER BY at`,
    ),
    counter_notices: await q(
      `SELECT cn.action_id, cn.received_at, cn.name, cn.address, cn.phone, cn.signature, cn.material_desc FROM dmca_counter_notices cn JOIN dmca_actions a ON a.id = cn.action_id WHERE a.author_id = ?1 ORDER BY cn.received_at`,
    ),
    appeals: (
      await q<{
        item_id: string;
        at: number;
        detail: string | null;
        resolved_at: number | null;
      }>(
        `SELECT item_id, at, detail, resolved_at FROM admin_flags WHERE kind = 'appeal' AND user_id = ?1 ORDER BY at`,
      )
    ).map((a) => {
      let note: unknown = null;
      try {
        note = a.detail
          ? ((JSON.parse(a.detail) as { note?: unknown }).note ?? null)
          : null;
      } catch {
        note = null;
      }
      return { item_id: a.item_id, at: a.at, note, resolved_at: a.resolved_at };
    }),
    sessions: await q(
      `SELECT created_at, auth_at, expires_at, revoked_at, user_agent FROM sessions WHERE user_id = ?1 ORDER BY created_at`,
    ),
  };
  await c.db.batch([
    audit(
      c.db,
      {
        actorId: me,
        actorKind: "user",
        action: "account.export",
        targetUserId: me,
        detail: {
          items: items.length,
          parts: total > EXPORT_LIMITS.whole ? parts.length : 1,
        },
      },
      c.now(),
    ),
  ]);
  if (total > EXPORT_LIMITS.whole)
    return json(
      200,
      {
        ...data,
        export_too_large: true,
        manifest: {
          parts: parts.map((ids, i) => ({
            part: i + 1,
            items: ids,
            url: `/me/export?part=${i + 1}`,
          })),
        },
      },
      HEADERS("vershion-export.json"),
    );
  const all = new Set(items.map((x) => x.id));
  return json(
    200,
    {
      ...data,
      bodies: await readBodies(currentPairs(all)),
      // (round 24: each retained version's own full body, its hidden keys included: it is the author's)
      version_bodies: await readBodies(versionPairs(all)),
    },
    HEADERS("vershion-export.json"),
  );
}
