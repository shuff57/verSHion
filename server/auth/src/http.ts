// HTTP plumbing: JSON bodies, uniform responses, CORS, client mode, cookies.

const MAX_BODY_BYTES = 8 * 1024;

export class HttpError extends Error {
  constructor(
    public status: number,
    public code: string,
    public extra: Record<string, unknown> = {},
    public headers: Record<string, string> = {},
  ) {
    super(code);
  }
}

const BASE_HEADERS: Record<string, string> = {
  "Content-Type": "application/json; charset=utf-8",
  "Cache-Control": "no-store",
  Pragma: "no-cache",
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "no-referrer",
};

export function json(status: number, body: unknown, headers: Record<string, string> = {}): Response {
  const h = new Headers(BASE_HEADERS);
  for (const [k, v] of Object.entries(headers)) h.set(k, v);
  return new Response(body === null ? null : JSON.stringify(body), { status, headers: h });
}

export function empty(status = 204, headers: Record<string, string> = {}): Response {
  const h = new Headers(BASE_HEADERS);
  h.delete("Content-Type");
  for (const [k, v] of Object.entries(headers)) h.set(k, v);
  return new Response(null, { status, headers: h });
}

export function errorResponse(e: HttpError): Response {
  return json(e.status, { error: e.code, ...e.extra }, e.headers);
}

/**
 * Reads at most `max` bytes of a body, chunk by chunk, and stops (413) as soon as more arrive: a body
 * without a Content-Length (or with a false one) is never buffered beyond the limit.
 */
async function readLimited(req: Request, max: number): Promise<Uint8Array> {
  if (!req.body) return new Uint8Array(0);
  const reader = req.body.getReader();
  const parts: Uint8Array[] = [];
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    total += value.byteLength;
    if (total > max) {
      await reader.cancel().catch(() => {});
      throw new HttpError(413, "body_too_large");
    }
    parts.push(value);
  }
  const out = new Uint8Array(total);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.byteLength;
  }
  return out;
}

/**
 * Whether valid JSON `s` has an object with the same key twice (compared after unescaping). Linear;
 * memory: the keys of the open objects.
 */
export function hasDuplicateKey(s: string): boolean {
  const stack: (Set<string> | null)[] = [];
  // Whether the next string in the innermost object is a key.
  let wantKey = false;
  for (let i = 0; i < s.length; i++) {
    const ch = s[i]!;
    if (ch === "{") {
      stack.push(new Set());
      wantKey = true;
    } else if (ch === "[") {
      stack.push(null);
      wantKey = false;
    } else if (ch === "}" || ch === "]") {
      stack.pop();
      wantKey = false;
    } else if (ch === ",") {
      wantKey = stack.length > 0 && stack[stack.length - 1] !== null;
    } else if (ch === '"') {
      let j = i + 1;
      while (s[j] !== '"') j += s[j] === "\\" ? 2 : 1;
      if (wantKey) {
        const key = JSON.parse(s.slice(i, j + 1)) as string;
        const keys = stack[stack.length - 1]!;
        if (keys.has(key)) return true;
        keys.add(key);
        wantKey = false;
      }
      i = j;
    }
  }
  return false;
}

/** Reads a small JSON object body. Only `application/json` is accepted: HTML forms cannot send it cross-site without a preflight. */
export async function readJson(req: Request, max: number = MAX_BODY_BYTES): Promise<Record<string, unknown>> {
  const ct = (req.headers.get("Content-Type") ?? "").split(";")[0]!.trim().toLowerCase();
  if (ct !== "application/json") throw new HttpError(415, "unsupported_media_type");
  const len = Number(req.headers.get("Content-Length") ?? "0");
  if (len > max) throw new HttpError(413, "body_too_large");
  const buf = await readLimited(req, max);
  let v: unknown;
  let text: string;
  try {
    text = new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(buf);
    v = JSON.parse(text);
  } catch {
    throw new HttpError(400, "invalid_json");
  }
  // JSON.parse keeps the last of duplicate keys: a body with any is refused, never read two ways.
  if (hasDuplicateKey(text)) throw new HttpError(400, "invalid_json", { reason: "duplicate_key" });
  if (typeof v !== "object" || v === null || Array.isArray(v)) throw new HttpError(400, "invalid_json");
  return v as Record<string, unknown>;
}

export function str(body: Record<string, unknown>, key: string, max: number, required = true): string | null {
  const v = body[key];
  if (v === undefined || v === null) {
    if (required) throw new HttpError(400, "invalid_request", { field: key });
    return null;
  }
  if (typeof v !== "string" || v.length === 0 || v.length > max) throw new HttpError(400, "invalid_request", { field: key });
  return v;
}

const EMAIL_RE = /^[^\s@"(),:;<>[\]\\]{1,64}@(?=.{1,253}$)[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$/;

/** Trim, lower-case and check shape. The same address always maps to the same string. */
export function normalizeEmail(raw: string): string {
  // ASCII only (IDN domains in punycode): no lookalike '@' (U+FF20, U+FE6B), no control, format or
  // zero-width characters, nothing an admin could misread.
  if (!/^[\x21-\x7e]+$/.test(raw.trim())) throw new HttpError(400, "invalid_request", { field: "email" });
  const e = raw.trim().toLowerCase();
  if (e.length > 254 || !EMAIL_RE.test(e)) throw new HttpError(400, "invalid_request", { field: "email" });
  return e;
}

// ---- client mode and CORS -------------------------------------------------------------

export type ClientMode = "web" | "native";

export interface ClientInfo {
  mode: ClientMode;
  origin: string | null;
}

/**
 * Decides web vs native and enforces the origin rules.
 * - `X-Client: native` with an `Origin` header is refused, so a browser script can never
 *   receive a refresh token in a body.
 * - Any `Origin` not on the exact allow-list is refused.
 */
export function clientInfo(req: Request, allowed: ReadonlySet<string>): ClientInfo {
  const origin = req.headers.get("Origin");
  const xc = req.headers.get("X-Client");
  let mode: ClientMode = "web";
  if (xc !== null) {
    if (xc.trim().toLowerCase() !== "native") throw new HttpError(400, "invalid_client");
    mode = "native";
  }
  if (mode === "native" && origin !== null) throw new HttpError(400, "native_client_with_origin");
  if (origin !== null && !allowed.has(origin)) throw new HttpError(403, "origin_not_allowed");
  return { mode, origin };
}

export function corsHeaders(origin: string | null, allowed: ReadonlySet<string>): Record<string, string> {
  if (origin === null || !allowed.has(origin)) return { Vary: "Origin" };
  return {
    "Access-Control-Allow-Origin": origin,
    "Access-Control-Allow-Credentials": "true",
    "Access-Control-Expose-Headers": "Retry-After, ETag",
    Vary: "Origin",
  };
}

/** Answers a CORS preflight. Disallowed origins get a 403 with no CORS headers. */
export function preflight(req: Request, allowed: ReadonlySet<string>, methods: string): Response {
  const origin = req.headers.get("Origin");
  if (origin === null || !allowed.has(origin)) return empty(403, { Vary: "Origin" });
  return empty(204, {
    ...corsHeaders(origin, allowed),
    "Access-Control-Allow-Methods": methods,
    // X-Client is deliberately absent: a browser can never send `X-Client: native` cross-origin.
    "Access-Control-Allow-Headers": "Content-Type, Authorization, X-Sudo-Token, If-None-Match",
    "Access-Control-Max-Age": "600",
  });
}

export function withHeaders(res: Response, headers: Record<string, string>): Response {
  const out = new Response(res.body, res);
  for (const [k, v] of Object.entries(headers)) {
    if (k === "Vary") out.headers.append("Vary", v);
    else out.headers.set(k, v);
  }
  return out;
}

// ---- cookies --------------------------------------------------------------------------

export function refreshCookie(name: string, token: string, maxAgeSeconds: number): string {
  return `${name}=${token}; Path=/; Max-Age=${maxAgeSeconds}; HttpOnly; Secure; SameSite=Strict`;
}

export function clearCookie(name: string): string {
  return `${name}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Strict`;
}

export function readCookie(req: Request, name: string): string | null {
  const header = req.headers.get("Cookie");
  if (!header) return null;
  let found: string | null = null;
  for (const part of header.split(";")) {
    const i = part.indexOf("=");
    if (i < 0) continue;
    if (part.slice(0, i).trim() === name) {
      // Two cookies with our name means something planted one: refuse both.
      if (found !== null) return null;
      found = part.slice(i + 1).trim();
    }
  }
  return found;
}

export function bearer(req: Request): string | null {
  const h = req.headers.get("Authorization");
  if (!h) return null;
  const m = /^Bearer ([A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+)$/.exec(h.trim());
  return m ? m[1]! : null;
}
