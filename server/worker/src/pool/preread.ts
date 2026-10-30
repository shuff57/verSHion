// The body of every request the auth package handles (its account routes and the admin routes) is
// read here, by the pool's one body reader, BEFORE the auth package sees it (round 8 I4/I5):
// Content-Length required and exact, at most 8 KB. A request with a valid bearer token of a TRUSTED
// account (teacher or admin) is authenticated FIRST and read under its account (the isolate budget,
// the per-account caps): it is never refused because of unauthenticated traffic. Anything else (no
// token, a bad one, a pending or unverified account) is read under the small unauthenticated
// budgets (round 9 K2): a 2 s deadline, nested per-address caps, and separate pools for sign-in and
// sign-up so other unauthenticated traffic cannot starve them. Pool routes (/items, /blobs, /circles) are
// skipped: they read their own bodies after their own authentication.
import { json, PoolError, readBody, type UnauthPool } from "./shared.ts";

/** The auth package reads (account and admin) bodies of at most 8 KB ... */
export const AUTH_BODY_MAX = 8 * 1024;
/** ... except an admin's, on an admin route: 256 KB (round 10 M-8: hash lists, long notices). */
export const ADMIN_BODY_MAX = 256 * 1024;
/** Trust levels whose bearer requests are read under their account. */
export const TRUSTED = new Set(["teacher", "admin"]);
/** Unauthenticated bodies at once per address prefix (round 9 K2). */
export const PREFIX_CAPS = { v6_64: 2, v6_56: 4, v6_48: 8, v6_32: 16, v4: 4, v4_24: 8 } as const;

/** The per-address cap keys of an address: an IPv4 address, or an IPv6 address's /64, /56, /48 and /32. */
export function addressCaps(ip: string): { key: string; cap: number }[] {
  const a = ip.trim().toLowerCase().split("%")[0]!;
  if (a === "") return [{ key: "none", cap: PREFIX_CAPS.v6_64 }];
  // IPv4: the address and its /24 (round 10 S-1: a /24 cannot fill the sign-in pool).
  const v4 = (x: string) => [{ key: `v4:${x}`, cap: PREFIX_CAPS.v4 }, { key: `v4:${x.split(".").slice(0, 3).join(".")}/24`, cap: PREFIX_CAPS.v4_24 }];
  if (!a.includes(":")) return v4(a);
  const mapped = /^::ffff:(\d+\.\d+\.\d+\.\d+)$/.exec(a);
  if (mapped) return v4(mapped[1]!);
  const [head, tail] = a.split("::");
  const h = head ? head.split(":") : [];
  const t = tail !== undefined && tail !== "" ? tail.split(":") : [];
  const fill = a.includes("::") ? new Array(Math.max(0, 8 - h.length - t.length)).fill("0") : [];
  const g = [...h, ...fill, ...t].map((x) => (parseInt(x, 16) || 0).toString(16).padStart(4, "0"));
  const hex = g.join("");
  return [
    { key: `v6:${hex.slice(0, 16)}/64`, cap: PREFIX_CAPS.v6_64 },
    { key: `v6:${hex.slice(0, 14)}/56`, cap: PREFIX_CAPS.v6_56 },
    { key: `v6:${hex.slice(0, 12)}/48`, cap: PREFIX_CAPS.v6_48 },
    { key: `v6:${hex.slice(0, 8)}/32`, cap: PREFIX_CAPS.v6_32 },
  ];
}

function poolOf(path: string): UnauthPool {
  if (/^\/auth\/(login|refresh|reauth)$/.test(path)) return "signin";
  if (path === "/auth/signup") return "signup";
  return "general";
}

export async function preReadBody(
  req: Request,
  authenticate: (r: Request) => Promise<{ userId: string; trust?: string } | null>,
  _ipBucket?: (ip: string) => string,
): Promise<Request | Response> {
  const path = new URL(req.url).pathname;
  if (req.method === "GET" || req.method === "HEAD" || req.method === "OPTIONS" || /^\/(items|blobs|circles|ai)(\/|$)/.test(path)) return req;
  try {
    const ip = (req.headers.get("CF-Connecting-IP") ?? "").trim();
    const bearer = /^Bearer \S+$/.test(req.headers.get("Authorization") ?? "");
    let who: { userId: string; trust?: string } | null = null;
    let authFailed = false;
    if (bearer)
      try {
        who = await authenticate(req);
      } catch {
        authFailed = true;
      }
    const trusted = who !== null && (who.trust === undefined || TRUSTED.has(who.trust));
    const b = trusted
      ? await readBody(req, { max: who!.trust === "admin" && path.startsWith("/admin/") ? ADMIN_BODY_MAX : AUTH_BODY_MAX, key: `u:${who!.userId}`, optional: true })
      : authFailed
        ? // Authentication itself failed (not a bad token): fail open for the bearer request, read
          // under the address in the isolate budget, never the unauthenticated pools.
          await readBody(req, { max: AUTH_BODY_MAX, key: `authfail:${addressCaps(ip)[0]!.key}`, optional: true })
        : await readBody(req, { max: AUTH_BODY_MAX, key: `ip:${addressCaps(ip)[0]!.key}`, optional: true, unauth: { pool: poolOf(path), caps: addressCaps(ip) } });
    b.release();
    return new Request(req, { body: b.bytes.length > 0 ? b.bytes : null });
  } catch (e) {
    // (Round 21, critic27b: the pool's base headers on these too: nosniff, CSP, Vary, Referrer-Policy.)
    if (e instanceof PoolError) return json(e.status, { error: e.code, ...e.extra }, e.headers);
    // Anything else (the runtime refusing the stream, say) is a clean 400, never an unhandled 500.
    return Response.json({ error: "body_aborted" }, { status: 400, headers: { "Cache-Control": "no-store" } });
  }
}
