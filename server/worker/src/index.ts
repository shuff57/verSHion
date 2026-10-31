// verSHion pool API Worker. Phase 1: accounts and auth, via the generic package in ../auth.
// Phase 2: the shared pool (src/pool/), with share-core's wasm behind one adapter (src/pool/core.ts).
import wasm from "../../share-core/pkg/share_core_bg.wasm";
import { argon2id, initSync } from "../../share-core/pkg/share_core.js";
import * as shareCore from "../../share-core/pkg/share_core.js";
import { assertProductionShareCore } from "./share-core-guard.js";
import { cleanText, createAuth, D1RateLimitStore, InputError, ipBucket, type Argon2idHasher, type Auth, type Principal } from "../../auth/src/index.js";
import { staffDomainLabels, staffDomainRoutes } from "./admin.js";
import { onDeleteContent, onRestoreContent, onSuspendContent } from "./cleanup.js";
import { createPool, deriveHashKey, flushTrapsLater, makeCore, POOL_METHODS, type Pool, type ShareCoreExports } from "./pool/index.ts";
import { guardCall } from "./pool/core.ts";
import { preReadBody } from "./pool/preread.ts";
import { json as poolJson } from "./pool/shared.ts";
import { poolNotifier } from "./poolmail.ts";
import { adminAlert, makeMailer, queueReminder, type MailEnv } from "./mail.js";
import { flushNoticeBacklog, noticeNewApplication } from "./notices.js";
import { AI_DEFAULT_MODEL, OPENROUTER_DEFAULT_MODEL, createAi, type AiBinding, type Limits } from "./ai/index.ts";
import { pruneAi } from "./ai/jobs.ts";
import { createBilling } from "./billing/index.ts";

export interface Env extends MailEnv {
  DB: D1Database;
  /** 32+ random characters; `wrangler secret put AUTH_SECRET`. */
  AUTH_SECRET: string;
  /** Comma-separated exact origins of the web build, e.g. "https://app.example.org". */
  ALLOWED_ORIGINS: string;
  /** "1" sends the admin a daily approval digest even when nothing is overdue. */
  APPROVAL_DIGEST?: string;
  /** R2 bucket for publish bodies, quarantine and images (wrangler.toml [[r2_buckets]] binding "POOL"). */
  POOL?: R2Bucket;
  /**
   * 32+ random characters; `wrangler secret put POOL_HASH_KEY`. share-core's keyed full_hash uses a
   * key derived from it by HKDF (purpose "full-hash"). Without it the pool refuses every request
   * (503 pool_not_configured); auth keeps working. Never rotate it casually: stored hashes use it.
   */
  POOL_HASH_KEY?: string;
  /**
   * Workers AI binding (`[ai]` in wrangler.toml). Absent under `wrangler dev --local`, where
   * `/ai/chat/completions` answers 503 ai_not_configured. See docs/ai-harness-spec.md.
   */
  AI?: AiBinding;
  /** The one model the relay calls. The client's `model` field is ignored on purpose. */
  AI_MODEL?: string;
  /**
   * `wrangler secret put AI_RELAY_KEY`: the desktop app's key to the relay. Inference is billed to
   * this account; the key only authorises the app. Not a Cloudflare API token.
   */
  AI_RELAY_KEY?: string;
  /**
   * `wrangler secret put OPENROUTER_API_KEY`: when set, the relay calls OpenRouter first and falls
   * back to the Workers AI binding. OPENROUTER_MODEL picks the model; OPENROUTER_PRIVACY="0" allows
   * providers that may keep prompts (the default asks OpenRouter for ones that do not).
   */
  OPENROUTER_API_KEY?: string;
  OPENROUTER_MODEL?: string;
  OPENROUTER_PRIVACY?: string;
  /** "1" turns Gemma 4's thinking mode on for the relay. Off by default (see src/ai/index.ts). */
  AI_THINKING?: string;
  /** Quotas (docs/ai-harness-spec.md section 5). Each is a positive integer; unset or invalid keeps the default. */
  AI_CHAT_PER_HOUR?: string;
  AI_IMPORT_PER_DAY?: string;
  /** Credits per signed-in account per UTC week (default 40). */
  AI_WEEKLY_CREDITS?: string;
  AI_DAILY_NEURONS?: string;
  AI_MAX_PAGES?: string;
  AI_JOB_TTL_HOURS?: string;
  /** "1" turns buying credits on (/billing/packs and /billing/intent answer 404 otherwise). The webhook works whenever its secret is set. */
  BILLING_ENABLED?: string;
  /** `wrangler secret put STRIPE_SECRET_KEY` (sk_test_... until launch) and `STRIPE_WEBHOOK_SECRET` (whsec_... of this Worker's own endpoint). */
  STRIPE_SECRET_KEY?: string;
  STRIPE_WEBHOOK_SECRET?: string;
  STRIPE_PUBLISHABLE_KEY?: string;
}

/** The derived full-hash key, per secret value (one HKDF per isolate). */
let hashKeyCache: { secret: string; key: string } | null = null;
async function poolHashKey(env: Env): Promise<string | null> {
  const s = env.POOL_HASH_KEY;
  if (typeof s !== "string" || s.length < 32) return null;
  if (hashKeyCache?.secret !== s) hashKeyCache = { secret: s, key: await deriveHashKey(s, "full-hash") };
  return hashKeyCache.key;
}

/** The trust levels of docs/shared-pool.md, lowest first. */
export const TRUST = ["unverified", "pending", "teacher", "admin"] as const;
export type Trust = (typeof TRUST)[number];

// Refuse to start with a test build of share-core (it would export a caller-seeded strip).
assertProductionShareCore(shareCore);

let wasmReady = false;
function ensureWasm(): void {
  if (!wasmReady) {
    initSync({ module: wasm });
    wasmReady = true;
  }
}
// (Round 21: argon2id behind the same trap guard as the pool's share-core calls.)
const guardedArgon2id = guardCall(shareCore as unknown as ShareCoreExports, argon2id, "argon2id");
const shareCoreHasher: Argon2idHasher = {
  argon2id(password, salt, p) {
    ensureWasm();
    return guardedArgon2id(password, salt, p.m, p.t, p.p);
  },
};

/** Sign-up fields beyond email and password: display name, school, and the adult-employee statement (COPPA/SOPIPA). */
function profile(body: Record<string, unknown>) {
  // Plain text only: no control, format or bidi characters, whitespace collapsed (the package checks
  // again whatever this returns).
  const name = cleanText(body.display_name, 100);
  if (!name) throw new InputError("required_plain_text", "display_name");
  const school = cleanText(body.school, 200);
  if (!school) throw new InputError("required_plain_text", "school");
  if (body.adult_school_employee !== true) throw new InputError("must_confirm", "adult_school_employee");
  return { displayName: name, profile: { school, adult_school_employee_at: new Date().toISOString() } };
}

const origins = (env: Env) => env.ALLOWED_ORIGINS.split(",").map((s) => s.trim()).filter(Boolean);

/** The pool and the auth package, wired together: the pool authenticates through auth, and its admin routes sit behind auth's admin guard. */
function build(env: Env, ctx: ExecutionContext | undefined, hashKey: string | null): { a: Auth; pool: Pool } {
  ensureWasm();
  let a: Auth | null = null;
  const pool = createPool({
    db: env.DB,
    r2: env.POOL,
    core: makeCore(shareCore as unknown as ShareCoreExports, { hashKey }),
    secret: env.AUTH_SECRET,
    allowedOrigins: origins(env),
    authenticate: (req) => a!.authenticate(req),
    rateStore: new D1RateLimitStore(env.DB),
    ipBucket: (ip) => ipBucket(ip),
    notify: poolNotifier(env),
    background: (p) => ctx?.waitUntil(p),
  });
  a = auth(env, pool);
  return { a, pool };
}

function auth(env: Env, pool: Pool): Auth {
  const mailer = makeMailer(env);
  return createAuth({
    db: env.DB,
    hasher: shareCoreHasher,
    mailer,
    secret: env.AUTH_SECRET,
    allowedOrigins: origins(env),
    issuer: "vershion-pool",
    audience: "vershion-pool",
    argon2: { m: 19456, t: 2, p: 1 }, // docs/phase1-argon2-spike.md
    trust: { levels: TRUST, afterVerify: "pending", requestApproval: true },
    profile,
    // Every verified account lands in the admin queue and the admin is emailed (no automatic promotion).
    onVerified: async (userId) => void (await noticeNewApplication(env, userId)),
    // The author's pool items are removed at once and purged after 14 days (cleanup.ts).
    onDelete: (userId, now) => onDeleteContent(env.DB, userId, now),
    // Never-confirmed accounts are removed after a week, freeing a squatted address.
    purgeUnverifiedAfterSeconds: 7 * 86400,
    onRefreshReuse: (e) => console.warn(`auth alert: ${e.type} user=${e.userId} family=${e.sessionFamily}`),
    paths: { adminRevoke: "/revoke-teacher" },
    // docs/shared-pool.md "Trust levels": no automatic promotion; an admin approves only on the
    // verified address being on the school's staff directory, or the school office confirming
    // through a contact the admin initiated. A name match is never evidence.
    approval: {
      approvedLevel: "teacher",
      adminLevels: ["admin"],
      evidenceKinds: ["directory", "school_confirmed"],
      reapplyCooldownSeconds: 7 * 86400,
      escalateAfterSeconds: 48 * 3600,
      digest: env.APPROVAL_DIGEST === "1",
      domainLabels: staffDomainLabels(env.DB),
      onQueueReminder: (e) => queueReminder(env, e),
      onAdminAlert: (e) => adminAlert(env, e),
      twoPersonAdminSuspension: true,
      onSuspend: (userId, now) => onSuspendContent(env.DB, userId, now),
      onRestore: (userId, now) => onRestoreContent(env.DB, userId, now),
      routes: [...staffDomainRoutes(env.DB), ...pool.adminRoutes],
    },
  });
}

/** Route guard for later phases: every route other than the account routes needs `teacher`. */
export async function requireTrust(a: Auth, req: Request, min: Trust): Promise<Principal | Response> {
  const p = await a.authenticate(req); // always checks revocation and reads current trust
  if (!p) return Response.json({ error: "unauthorized" }, { status: 401 });
  if (TRUST.indexOf(p.trust as Trust) < TRUST.indexOf(min)) return Response.json({ error: "forbidden" }, { status: 403 });
  return p;
}

/** A positive integer from an env var, or undefined (so the default stands) when unset or not one. */
function posInt(v: string | undefined): number | undefined {
  const n = Number(v);
  return v !== undefined && Number.isInteger(n) && n > 0 ? n : undefined;
}

function aiLimits(env: Env): Partial<Limits> {
  const l: Partial<Limits> = {};
  const set = <K extends keyof Limits>(k: K, v: number | undefined) => void (v !== undefined && (l[k] = v as Limits[K]));
  set("chatPerHour", posInt(env.AI_CHAT_PER_HOUR));
  set("importPerDay", posInt(env.AI_IMPORT_PER_DAY));
  set("dailyNeurons", posInt(env.AI_DAILY_NEURONS));
  set("maxPages", posInt(env.AI_MAX_PAGES));
  set("weeklyCredits", posInt(env.AI_WEEKLY_CREDITS));
  return l;
}

async function handleFetch(req: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
  const hashKey = await poolHashKey(env);
  const { a, pool } = build(env, ctx, hashKey);
  // Billing reads its own small bodies: Stripe's signature needs the raw webhook body, untouched.
  if (new URL(req.url).pathname.startsWith("/billing/")) {
    const billed = await createBilling({
      enabled: env.BILLING_ENABLED === "1",
      stripeKey: env.STRIPE_SECRET_KEY?.trim() || undefined,
      webhookSecret: env.STRIPE_WEBHOOK_SECRET?.trim() || undefined,
      publishableKey: env.STRIPE_PUBLISHABLE_KEY?.trim() || undefined,
      db: env.DB,
      authenticate: (r) => a.authenticate(r),
    }).handle(req);
    if (billed) return a.withCors(req, billed);
  }
  // EVERY body the auth package reads goes through the pool's one body reader first (round 8 I4/I5).
  const body = await preReadBody(req, (r) => a.authenticate(r), ipBucket);
  if (body instanceof Response) return a.withCors(req, body);
  req = body;
  const handled = await a.handle(req, ctx);
  if (handled) return handled;
  const pre = a.preflight(req, POOL_METHODS);
  if (pre) return pre;
  // The AI relay (docs/ai-harness-spec.md §2): authenticated, no D1 and no share-core involved.
  const ai = createAi({
    ai: env.AI,
    model: env.AI_MODEL ?? AI_DEFAULT_MODEL,
    allowedOrigins: origins(env),
    authenticate: (r) => a.authenticate(r),
    relayKey: env.AI_RELAY_KEY,
    thinking: env.AI_THINKING === "1",
    openrouter: env.OPENROUTER_API_KEY?.trim()
      ? { key: env.OPENROUTER_API_KEY.trim(), model: env.OPENROUTER_MODEL?.trim() || OPENROUTER_DEFAULT_MODEL, privacy: env.OPENROUTER_PRIVACY !== "0" }
      : undefined,
    db: env.DB,
    bucket: env.POOL,
    limits: aiLimits(env),
  });
  const relayed = await ai.handle(req, ctx);
  if (relayed) return a.withCors(req, relayed);
  // No hash key (POOL_HASH_KEY missing or too short): the pool refuses, never hashes unkeyed.
  if (hashKey === null && /^\/(items|blobs|circles)(\/|$)/.test(new URL(req.url).pathname))
    return a.withCors(req, poolJson(503, { error: "pool_not_configured" }));
  const pooled = await pool.handle(req);
  if (pooled) return a.withCors(req, pooled);
  // (Round 21: the pool's base headers on the fallbacks too.)
  return a.withCors(req, poolJson(404, { error: "not_found" }));
}

export default {
  async fetch(req: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    try {
      return await handleFetch(req, env, ctx);
    } finally {
      // (Round 22: share-core traps, the login hasher's too, are counted from the request that trapped.)
      flushTrapsLater(env.DB, (p) => ctx.waitUntil(p));
    }
  },

  async scheduled(c: ScheduledController, env: Env, ctx: ExecutionContext): Promise<void> {
    const { a, pool } = build(env, ctx, await poolHashKey(env));
    // Hourly: the summary for sign-up notices over the budget, and the pool's GC.
    await flushNoticeBacklog(env, c.scheduledTime);
    await pool.hourly();
    // The AI relay's expired jobs and old usage rows. A failure here must not stop the daily stage below.
    try {
      await pruneAi(env.DB, env.POOL, c.scheduledTime, (posInt(env.AI_JOB_TTL_HOURS) ?? 72) * 3_600_000);
    } catch {
      console.error("ai: prune failed");
    }
    // Daily (04:xx UTC): purge, and escalate an overdue queue or send the digest.
    if (new Date(c.scheduledTime).getUTCHours() === 4) {
      await a.purgeExpired();
      await a.reviewReminders();
      await pool.daily();
    }
  },
} satisfies ExportedHandler<Env>;
