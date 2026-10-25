// Round 42: circles phase C, email invites (docs/circles-plan.md D5, 8.1). Round 46: after critics 53a/53b.
//
// POST /circles/:cid/invites/email {email, role} (the owner, like every circle code): makes an ordinary single-use
// circle code (circle_invites, kind 'email', only the code's hash stored) and has the server mail it from a FIXED
// text (poolmail.ts) that names no circle and no person (round 46: a circle's name is text its owner chose). The code
// then joins through POST /circles/join like any code, so the approval gate, the pool's invite-code gate, the circle's
// caps and the removal rules all apply unchanged.
//
// * No account-exists oracle: nobody's address is ever looked up (not the invitee's, not any account's). Whether a
//   mail goes depends only on the address string and on what was mailed to that mailbox today, never on accounts.
// * The inviter is told whether the mail went (round 46): `202 {emailed: true}`, or `422 invite_not_sent {emailed:
//   false, reason}` with nothing made: `not_deliverable` (an address this service's mailer does not send to) or
//   `address_limit` (that mailbox had circle_email_address mails today, from anyone).
// * One mailbox is one address for that count (round 46, critic 53b B1): NFKC, lower case, an IDN domain in its ASCII
//   form, trailing dots dropped, a `+tag` dropped, Gmail's dots and the googlemail alias folded (emailCapKey).
// * Caps: circle_email_invites (round 47: 40) a day per inviter, besides circle_invites a day, which an email invite
//   also counts toward; the circle's circle_open_email_invites (round 47: 40 open email invites, its 10 plain codes
//   apart); a full circle is refused up front. The hour's mail budget cannot be spent by one inviter or on one domain
//   (round 46): circle_email_invites_hourly (round 47: 30) per inviter (429 hourly_limit); circle_email_domain_hourly
//   (30) per domain, of which one inviter has at most circle_email_inviter_domain_hourly (10) and whose last fifth goes
//   only to inviters who mailed that domain fewer than EMAIL_SMALL_INVITER times this hour (round 47, critics 54a
//   variant 3 and 54b V1: 429 hourly_limit, "too many invitations to this mail provider"); and circle_email_hourly
//   (100) for the service, of which the last fifth goes only to inviters who sent fewer than EMAIL_SMALL_INVITER this
//   hour (503 busy).
// * Only an HMAC of the mailbox is kept (`circle_email_sends`, its own HKDF purpose `circle-invite-email`; the domain's
//   hour count is an HMAC too), deleted by the daily cron after two days; and (round 46) a MASKED label of the address
//   on the invite ("j***@gmail.com") so its owner can tell invites apart, nulled with the inviter's erasure and once
//   the invite is long dead. The address and the code live only in the in-memory notice handed to the mailer.
// * Round 48: a second invite to a mailbox that has an open email invite in the circle ROTATES it (new code mailed, old
//   one dead, same invite: `resent: true`), found by `email_key`, an HMAC of the circle and the mailbox kept with the
//   label and nulled with it.
// * Erasure (plan 8.1): the inviter's invites are revoked with its other codes and lose their labels; nothing can
//   match an invite to an address afterwards. Suspended or erased inviters send nothing: the session check refuses a
//   suspended account, and the write is guarded by ALIVE_SQL and the inviter's standing in the same statement.
import { ACTIVE_SQL, CIRCLE_CODE_DAYS, circleCodeHash, circleOrMiss, explain, IN_STANDING_SQL, MEMBERS_SQL, newCircleCode, normaliseCircleCode, OPEN_INVITES_SQL, OPEN_SQL, requireOpen, requireRole } from "./circles.ts";
import { ALIVE_SQL, audit, DAY_MS, dayOf, giveDaily, hmacOf, HOUR_MS, json, limitOf, newId, PoolError, readJson, requireDaily, send, throwIfErased, type Ctx, type PoolPrincipal } from "./shared.ts";

/**
 * The auth package's address rule (server/auth/src/http.ts normalizeEmail, which node's strip-only TypeScript cannot
 * load): a plain local part and a host name with a dot, at most 254 characters. Round 46: typed in NFKC (full-width
 * letters are their ASCII letters), an IDN domain in its ASCII (punycode) form, trailing dots dropped; the local part
 * must then be printable ASCII (no look-alike '@', no control, format or line-break character can reach a header).
 */
const EMAIL_RE = /^[^\s@"(),:;<>[\]\\]{1,64}@(?=.{1,253}$)[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$/;
export function normaliseInviteEmail(raw: unknown): string | null {
  if (typeof raw !== "string" || raw.length > 320) return null;
  const t = raw.normalize("NFKC").trim();
  const at = t.lastIndexOf("@");
  if (at <= 0) return null;
  const local = t.slice(0, at);
  let domain = t.slice(at + 1).replace(/\.+$/, "");
  if (!/^[\x21-\x7e]+$/.test(local) || domain === "") return null;
  if (!/^[\x21-\x7e]+$/.test(domain)) {
    // (only letters, digits, dots and hyphens of any script reach the IDN conversion: nothing a URL parser reads otherwise)
    if (!/^[\p{L}\p{M}\p{N}.-]+$/u.test(domain)) return null;
    try {
      domain = new URL(`http://${domain}/`).hostname;
    } catch {
      return null;
    }
  }
  const e = `${local}@${domain}`.toLowerCase();
  return e.length <= 254 && EMAIL_RE.test(e) ? e : null;
}
/** An address this service's mailer sends to (poolmail.ts checks the same before any mail): a letters-only top-level domain. */
export const DELIVERABLE_RE = /^[^\s@"(),:;<>[\]\\]{1,64}@[a-z0-9.-]{1,253}\.[a-z]{2,63}$/;
export const isDeliverable = (e: string): boolean => /^[\x21-\x7e]{3,254}$/.test(e) && DELIVERABLE_RE.test(e);
/**
 * The MAILBOX an address reaches, for the per-address cap (round 46, critic 53b B1): a `+tag` dropped (from the first
 * '+'), Gmail's dots ignored and googlemail.com read as gmail.com. Other providers' aliases are bounded by the
 * per-domain hourly cap instead.
 */
export function emailCapKey(e: string): string {
  const at = e.lastIndexOf("@");
  let local = e.slice(0, at);
  let domain = e.slice(at + 1);
  const plus = local.indexOf("+");
  if (plus >= 0) local = local.slice(0, plus);
  if (domain === "googlemail.com") domain = "gmail.com";
  if (domain === "gmail.com") local = local.replace(/\./g, "");
  return `${local}@${domain}`;
}
/** The masked label an owner's list shows: the address's first character and its domain. Round 47 (critic 54b V5): a
 *  one-character local part is not shown at all. (The invite's row adds " #2", " #3"... when the circle already has
 *  that label, so the owner can tell colleagues apart: critic 54a variant 4.) */
export const maskEmail = (e: string): string => {
  const at = e.lastIndexOf("@");
  return `${at > 1 ? e.slice(0, 1) : ""}***${e.slice(at)}`;
};
/** Mails an inviter may have sent this hour and still use the hour's last fifth (critic 53b B1: a share kept for others). */
export const EMAIL_SMALL_INVITER = 3;

const notSent = (reason: "address_limit" | "not_deliverable") =>
  new PoolError(422, "invite_not_sent", {
    emailed: false,
    reason,
    message:
      reason === "address_limit"
        ? "No invitation was sent: this address has had as many circle invitations today as it may get. Try again tomorrow, or give the person a code yourself."
        : "No invitation was sent: we cannot send mail to this address. Check it, or give the person a code yourself.",
  });

/**
 * Round 48: the rotation of an open email invite (see emailInvite): its code, role, issuer, time and expiry are new, its
 * id and label stay; the mailbox's count of the day is taken as for any mail. `202 {..., resent: true, message}`.
 */
async function rotate(
  c: Ctx,
  x: { me: string; cid: string; id: string; role: string; code: string; codeHash: string; expiresAt: number; now: number; addr: string; day: number; perAddress: number; membersCap: number; email: string; giveBack: () => Promise<void> },
): Promise<Response> {
  let res: D1Result[];
  try {
    res = await c.db.batch([
      c.db
        .prepare(`INSERT INTO circle_email_sends (addr, day, n) SELECT ?1, ?2, 1 WHERE ?3 >= 1 ON CONFLICT (addr, day) DO UPDATE SET n = n + 1 WHERE n < ?3`)
        .bind(x.addr, x.day, x.perAddress),
      c.db
        .prepare(
          `UPDATE circle_invites SET code_hash = ?3, role = ?4, created_by = ?5, created_at = ?6, expires_at = ?7
            WHERE id = ?1 AND circle_id = ?2 AND kind = 'email' AND revoked_at IS NULL AND uses < max_uses AND expires_at > ?6 AND changes() = 1
              AND ${OPEN_SQL("?2")} AND ${ACTIVE_SQL("?2", "?5", ["owner"])} AND ${ALIVE_SQL("?5")} AND ${IN_STANDING_SQL("?5")} AND ${MEMBERS_SQL("?2")} < ?8`,
        )
        .bind(x.id, x.cid, x.codeHash, x.role, x.me, x.now, x.expiresAt, x.membersCap),
      c.db.prepare(`UPDATE circle_email_sends SET n = MAX(0, n - 1) WHERE addr = ?1 AND day = ?2 AND NOT EXISTS (SELECT 1 FROM circle_invites WHERE id = ?3 AND code_hash = ?4)`).bind(x.addr, x.day, x.id, x.codeHash),
      audit(c.db, { actorId: x.me, actorKind: "user", action: "circle.email_invite", itemId: x.cid, targetUserId: null, detail: { invite_id: x.id, role: x.role, max_uses: 1, expires_at: x.expiresAt, resent: true } }, x.now, `EXISTS (SELECT 1 FROM circle_invites WHERE id = '${x.id}' AND code_hash = '${x.codeHash}')`),
      c.db.prepare(`SELECT email_label FROM circle_invites WHERE id = ?1 AND code_hash = ?2`).bind(x.id, x.codeHash),
    ]);
  } catch (e) {
    await x.giveBack();
    throw e;
  }
  const row = ((res[4]!.results as { email_label: string | null }[]) ?? [])[0];
  if (!row) {
    await x.giveBack();
    await throwIfErased(c, x.me);
    const left = (await c.db.prepare(`SELECT n FROM circle_email_sends WHERE addr = ?1 AND day = ?2`).bind(x.addr, x.day).first<{ n: number }>())?.n ?? 0;
    if (left >= x.perAddress) throw notSent("address_limit");
    const members = (await c.db.prepare(`SELECT ${MEMBERS_SQL("?1")} AS n`).bind(x.cid).first<{ n: number }>())?.n ?? 0;
    if (members >= x.membersCap) throw new PoolError(409, "circle_full", { limit: x.membersCap, message: `This circle has ${x.membersCap} members, the most allowed.` });
    await explain(c, x.cid, x.me, ["owner"]);
    // (the open invite was used, revoked or expired meanwhile: invite again for a new one)
    throw new PoolError(409, "conflict", { message: "That invitation changed meanwhile. Invite the address again." });
  }
  send(c, { kind: "circle_email_invite", to: x.email, code: x.code, role: x.role as "editor" | "viewer", expiresAt: x.expiresAt });
  return json(202, {
    ok: true,
    invite_id: x.id,
    kind: "email",
    role: x.role,
    max_uses: 1,
    expires_at: x.expiresAt,
    emailed: true,
    address_label: row.email_label ?? "",
    resent: true,
    message: "This address already had an open invitation from this circle: a new code was mailed and the earlier one no longer works.",
  });
}

/** POST /circles/:cid/invites/email {email, role}: 202 {ok, invite_id, kind: "email", role, max_uses, expires_at, emailed: true, address_label}. */
export async function emailInvite(c: Ctx, req: Request, p: PoolPrincipal, cid: string): Promise<Response> {
  const me = p.userId;
  const m = await circleOrMiss(c, cid, me);
  const b = await readJson(req, `u:${me}`, { deadlineMs: c.deps.bodyDeadlineMs });
  // Nothing but the address and the role: the inviter adds no text to the mail.
  for (const k of Object.keys(b)) if (k !== "email" && k !== "role") throw new PoolError(400, "invalid_request", { field: k, message: "Only an address and a role." });
  const email = normaliseInviteEmail(b.email);
  if (!email) throw new PoolError(400, "invalid_request", { field: "email", message: "This is not an email address we can send to." });
  const role = b.role;
  if (role !== "editor" && role !== "viewer") throw new PoolError(400, "invalid_request", { field: "role" });
  requireOpen(m);
  requireRole(m, "owner");
  const now = c.now();
  const day = dayOf(now);
  // Up front, before anything is charged or made (critic 53a variant 3): a full circle; an address our mailer does not
  // send to; a mailbox that had its mails for today. Each depends on the address string (and today's count for it),
  // never on whether anyone has an account.
  const membersCap = await limitOf(c, "*", "circle_members");
  const members = (await c.db.prepare(`SELECT ${MEMBERS_SQL("?1")} AS n`).bind(cid).first<{ n: number }>())?.n ?? 0;
  if (members >= membersCap) throw new PoolError(409, "circle_full", { limit: membersCap, message: `This circle has ${membersCap} members, the most allowed.` });
  if (!isDeliverable(email)) throw notSent("not_deliverable");
  const perAddress = await limitOf(c, "*", "circle_email_address");
  const capKey = emailCapKey(email);
  const addr = await hmacOf(c, "email", `circle-invite-email/v1:${capKey}`);
  const sentToday = (await c.db.prepare(`SELECT n FROM circle_email_sends WHERE addr = ?1 AND day = ?2`).bind(addr, day).first<{ n: number }>())?.n ?? 0;
  if (sentToday >= perAddress) throw notSent("address_limit");
  // Round 48 (critic 55a V6): an open email invite to this mailbox in this circle is ROTATED (a new code mailed, the old
  // one dead: one live code per address), not doubled. Found by an HMAC of the circle and the mailbox kept on the invite.
  const emailKey = await hmacOf(c, "email", `circle-invite-key/v1:${cid}:${capKey}`);
  const open = await c.db
    .prepare(
      `SELECT id FROM circle_invites INDEXED BY circle_invites_email_key WHERE circle_id = ?1 AND email_key = ?2 AND kind = 'email' AND revoked_at IS NULL AND uses < max_uses AND expires_at > ?3
        ORDER BY created_at DESC LIMIT 1`,
    )
    .bind(cid, emailKey, now)
    .first<{ id: string }>();
  await requireDaily(c, me, "circle_email_invites");
  const taken: ("circle_email_invites" | "circle_invites")[] = ["circle_email_invites"];
  const hour = Math.floor(now / HOUR_MS);
  const domainKey = `circle_email_domain:${await hmacOf(c, "email", `circle-invite-domain/v1:${capKey.slice(capKey.lastIndexOf("@") + 1)}`)}`;
  const inviterKey = `circle_email_inviter:${me}`;
  const slots: string[] = [];
  const giveBack = async () => {
    for (const x of taken) await giveDaily(c, me, x).catch(() => {});
    for (const ch of slots) await c.db.prepare(`UPDATE pool_alert_budget SET sent = MAX(0, sent - 1) WHERE hour = ?1 AND channel = ?2`).bind(hour, ch).run().catch(() => {});
  };
  /** One of the hour's counters (pool_alert_budget, deleted by the cron after 48 hours): its count after taking one, or null at its cap. */
  const takeSlot = async (channel: string, cap: number): Promise<number | null> => {
    const r = await c.db
      .prepare(
        `INSERT INTO pool_alert_budget (hour, channel, sent) SELECT ?1, ?2, 1 WHERE ?3 >= 1
         ON CONFLICT (hour, channel) DO UPDATE SET sent = sent + 1 WHERE sent < ?3 RETURNING sent`,
      )
      .bind(hour, channel, cap)
      .first<{ sent: number }>();
    if (r) slots.push(channel);
    return r?.sent ?? null;
  };
  const retryHour = { "Retry-After": String(Math.max(1, Math.ceil(((hour + 1) * HOUR_MS - now) / 1000))) };
  const busy = () => new PoolError(503, "busy", { message: "Too many invitations are being sent right now. Try again later." }, retryHour);
  // (round 47, critics 54a variant 3 / 54b V1: a busy mail domain is not an outage; said so, as a limit of the hour.
  // Round 48, critic 55a V6: neutral words, and the inviter's own share told apart from the domain's hour)
  const domainBusy = () =>
    new PoolError(429, "hourly_limit", { metric: "circle_email_domain_hourly", message: "There have been too many invitations to addresses at this mail domain in the last hour. Try again in an hour." }, retryHour);
  try {
    // (a rotated invite is no new code: it does not count toward the day's codes)
    if (!open) {
      await requireDaily(c, me, "circle_invites");
      taken.push("circle_invites");
    }
    // The hour (round 46, critic 53b B1): this inviter's share, this domain's share, then the service's budget, whose
    // last fifth is kept for inviters who sent little this hour.
    const mine = await takeSlot(inviterKey, await limitOf(c, me, "circle_email_invites_hourly"));
    if (mine === null)
      throw new PoolError(429, "hourly_limit", { metric: "circle_email_invites_hourly", message: "You have sent as many circle invitations as you may this hour. Try again later." }, retryHour);
    // The domain (round 47): this inviter's share of it first, then the domain's hour, whose last fifth is kept for
    // inviters who mailed it little this hour (so a few heavy inviters cannot close a big provider for everyone).
    const domainCap = await limitOf(c, "*", "circle_email_domain_hourly");
    const share = Math.min(domainCap, await limitOf(c, me, "circle_email_inviter_domain_hourly"));
    const here = await takeSlot(`${domainKey}:${me}`, share);
    if (here === null)
      throw new PoolError(
        429,
        "hourly_limit",
        { metric: "circle_email_inviter_domain_hourly", limit: share, message: `You have sent as many invitations to addresses at this mail domain as you may in an hour (${share}). Try again in an hour.` },
        retryHour,
      );
    if ((await takeSlot(domainKey, here <= EMAIL_SMALL_INVITER ? domainCap : domainCap - Math.ceil(domainCap / 5))) === null) throw domainBusy();
    const site = await limitOf(c, "*", "circle_email_hourly");
    const siteCap = mine <= EMAIL_SMALL_INVITER ? site : site - Math.ceil(site / 5);
    if ((await takeSlot("circle_email_invite", siteCap)) === null) throw busy();
  } catch (e) {
    await giveBack();
    throw e;
  }
  const cap = await limitOf(c, "*", "circle_open_email_invites");
  const code = newCircleCode();
  const label = maskEmail(email);
  const id = open?.id ?? newId();
  const expiresAt = now + CIRCLE_CODE_DAYS * DAY_MS;
  const codeHash = await circleCodeHash(normaliseCircleCode(code)!);
  if (open) return rotate(c, { me, cid, id, role, code, codeHash, expiresAt, now, addr, day, perAddress, membersCap, email, giveBack });
  let res: D1Result[];
  try {
    res = await c.db.batch([
      // (the mailbox's count of the day first, by its HMAC only, and only while it has room: no race past the cap)
      c.db
        .prepare(`INSERT INTO circle_email_sends (addr, day, n) SELECT ?1, ?2, 1 WHERE ?3 >= 1 ON CONFLICT (addr, day) DO UPDATE SET n = n + 1 WHERE n < ?3`)
        .bind(addr, day, perAddress),
      c.db
        .prepare(
          `INSERT INTO circle_invites (id, circle_id, code_hash, role, created_by, created_at, max_uses, uses, expires_at, note, kind, email_key, email_label)
           SELECT ?1, ?2, ?3, ?4, ?5, ?6, 1, 0, ?7, NULL, 'email', ?11,
                  -- (round 47: " #n" after a label the circle already has)
                  (SELECT CASE WHEN COUNT(*) = 0 THEN ?10 ELSE ?10 || ' #' || (COUNT(*) + 1) END FROM circle_invites li
                    WHERE li.circle_id = ?2 AND li.kind = 'email' AND (li.email_label = ?10 OR substr(li.email_label, 1, length(?10) + 2) = ?10 || ' #'))
            WHERE changes() = 1 AND ${OPEN_SQL("?2")} AND ${ACTIVE_SQL("?2", "?5", ["owner"])} AND ${ALIVE_SQL("?5")} AND ${IN_STANDING_SQL("?5")}
              AND ${OPEN_INVITES_SQL("?2", "?6", "email")} < ?8 AND ${MEMBERS_SQL("?2")} < ?9`,
        )
        .bind(id, cid, codeHash, role, me, now, expiresAt, cap, membersCap, label, emailKey),
      // (a count taken for an invite that was not made is given back in the same batch)
      c.db.prepare(`UPDATE circle_email_sends SET n = MAX(0, n - 1) WHERE addr = ?1 AND day = ?2 AND NOT EXISTS (SELECT 1 FROM circle_invites WHERE id = ?3)`).bind(addr, day, id),
      // (no target: the invitee is an address nobody keeps; members' activity shows "invited someone by email")
      audit(c.db, { actorId: me, actorKind: "user", action: "circle.email_invite", itemId: cid, targetUserId: null, detail: { invite_id: id, role, max_uses: 1, expires_at: expiresAt } }, now, `EXISTS (SELECT 1 FROM circle_invites WHERE id = '${id}')`),
      c.db.prepare(`SELECT 1 AS made, email_label FROM circle_invites WHERE id = ?1`).bind(id),
    ]);
  } catch (e) {
    // (round 46, critic 53b nitpick: a failed write gives the day's counts and the hour's slots back)
    await giveBack();
    throw e;
  }
  if (((res[4]!.results as unknown[]) ?? []).length !== 1) {
    await giveBack();
    await throwIfErased(c, me);
    const left = (await c.db.prepare(`SELECT n FROM circle_email_sends WHERE addr = ?1 AND day = ?2`).bind(addr, day).first<{ n: number }>())?.n ?? 0;
    if (left >= perAddress) throw notSent("address_limit");
    const n = await c.db.prepare(`SELECT ${OPEN_INVITES_SQL("?1", "?2", "email")} AS n, ${MEMBERS_SQL("?1")} AS members`).bind(cid, now).first<{ n: number; members: number }>();
    if ((n?.n ?? 0) >= cap)
      throw new PoolError(422, "circles_limit", {
        metric: "circle_open_email_invites",
        used: n?.n ?? 0,
        limit: cap,
        message: `This circle has ${cap} email invitations that have not been used yet. More can be sent as people join, as invitations expire after ${CIRCLE_CODE_DAYS} days, or when you withdraw one you no longer need.`,
      });
    if ((n?.members ?? 0) >= membersCap) throw new PoolError(409, "circle_full", { limit: membersCap, message: `This circle has ${membersCap} members, the most allowed.` });
    await explain(c, cid, me, ["owner"]);
  }
  send(c, { kind: "circle_email_invite", to: email, code, role, expiresAt });
  const stored = ((res[4]!.results as { email_label: string | null }[]) ?? [])[0]?.email_label ?? label;
  return json(202, { ok: true, invite_id: id, kind: "email", role, max_uses: 1, expires_at: expiresAt, emailed: true, address_label: stored });
}
