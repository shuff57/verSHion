// Workers AI relay for the desktop app. Contract: docs/ai-harness-spec.md §2.
//
// One endpoint, OpenAI-shaped in and out, so the app's existing non-Ollama path
// (`ai.rs::chat_turns` -> `choices/0/message/content`) and its base64 data-URI images work
// unchanged. The model is the worker's choice: a client's `model` field is ignored.
//
// The body is read by the pool's one guarded reader (`readJson`), never directly: the Worker's
// I4/I5 lint refuses any other direct body read in `src/`.
import { PoolError, readJson } from "../pool/shared.ts";
import { describeServices, isServiceId, SERVICES, utcWeekStart, WEEK_MS } from "./credits.ts";
import { readJob, startJob, type JobLimits } from "./jobs.ts";
import { AiStore, HOUR_MS, utcDayStart, DAY_MS } from "./store.ts";
import { dataUriBytes, err, json, type Message, type ModelOut, type Part } from "./util.ts";

/** The slice of the Workers AI binding this relay uses (the real type is a large generated union). */
export interface AiBinding {
  run(model: string, inputs: Record<string, unknown>): Promise<unknown>;
}

export interface AiDeps {
  /** `env.AI`; absent under `wrangler dev --local`, where the route answers 503. */
  ai?: AiBinding;
  /** The one model the relay calls. */
  model: string;
  /** Exact origins allowed to call the relay (the app's web build). */
  allowedOrigins: string[];
  /** A signed-in pool user. */
  authenticate(req: Request): Promise<{ userId: string } | null>;
  /**
   * The desktop app's relay key (`wrangler secret put AI_RELAY_KEY`). The app has no pool
   * session, so this admits it; it authorises the relay only and is never a Cloudflare token.
   * Unset: only signed-in users may call.
   */
  relayKey?: string;
  /** Signed-in accounts must spend credits through a service session (default on); false leaves only the hourly caps. */
  credits?: boolean;
  /**
   * Gemma 4's built-in thinking mode. Off by default: on a page transcription the hidden
   * reasoning was ~85-90% of the completion tokens (1.6-3.4k tokens spent on a ~200-token JSON
   * answer), which is where the latency and the empty/truncated replies came from. `AI_THINKING=1`
   * turns it back on for every request.
   */
  thinking?: boolean;
  /**
   * OpenRouter as the first upstream (`wrangler secret put OPENROUTER_API_KEY`). When set, a call goes
   * there first and, if that fails and the Workers AI binding exists, falls back to it. The key is
   * only ever sent in the Authorization header: never logged, never echoed.
   */
  openrouter?: {
    key: string;
    model: string;
    /** Only providers that do not store or train on the prompt (default true). */
    privacy?: boolean;
    timeoutMs?: number;
    /** Test seam. */
    fetch?: typeof fetch;
  };
  /** `env.DB`: usage rows and import jobs. Absent: no quota, and the job routes answer 503. */
  db?: D1Database;
  /** `env.POOL`: a job result above 256 KB goes here. */
  bucket?: R2Bucket;
  now?: () => number;
  /** Test seams and the deployment's quotas; production uses the constants and the env values. */
  limits?: Partial<Limits>;
}

export interface Limits extends JobLimits {
  maxOutTokens: number;
  /** Chat turns per principal per hour (callers without a credit session: the app's relay key, a keyless native client). */
  chatPerHour: number;
  /** Credits a signed-in user gets each UTC week (Monday 00:00). */
  weeklyCredits: number;
}

export const DEFAULT_LIMITS: Limits = {
  maxImageBytes: 4 * 1024 * 1024,
  maxOutTokens: 8192,
  chatPerHour: 120,
  weeklyCredits: 40,
  importPerDay: 20,
  // ~2 000 pages at the measured 23 neurons a page.
  dailyNeurons: 50_000,
  maxPages: 20,
};

export const AI_PATH = "/ai/chat/completions";
export const AI_HEALTH_PATH = "/ai/health";
export const AI_IMPORT_PATH = "/ai/import";
export const AI_SERVICE_PATH = "/ai/service";
export const AI_CREDITS_PATH = "/ai/credits";
const AI_IMPORT_ONE = /^\/ai\/import\/[^/]+$/;
export const AI_DEFAULT_MODEL = "@cf/google/gemma-4-26b-a4b-it";
export const OPENROUTER_DEFAULT_MODEL = "google/gemma-4-26b-a4b-it";
const OPENROUTER_URL = "https://openrouter.ai/api/v1/chat/completions";
/** Dollars per neuron (Workers AI bills $0.011 per 1,000). An OpenRouter cost is booked as the neurons it would equal, so one daily cap covers both upstreams. */
const USD_PER_NEURON = 0.000011;

const MAX_BODY_BYTES = 13 * 1024 * 1024;
const MAX_IMAGES = 4;
const MAX_MESSAGES = 40;

/** The bearer token of a request, if it has one. */
function bearer(req: Request): string | null {
  const m = /^Bearer\s+(\S+)$/i.exec(req.headers.get("Authorization") ?? "");
  return m ? m[1]! : null;
}

/** Constant-time comparison, so a wrong relay key cannot be found one byte at a time. */
function sameSecret(a: string, b: string): boolean {
  const x = new TextEncoder().encode(a);
  const y = new TextEncoder().encode(b);
  if (x.length !== y.length) return false;
  let diff = 0;
  for (let i = 0; i < x.length; i++) diff |= x[i]! ^ y[i]!;
  return diff === 0;
}

/** Validates `messages` and counts images across the whole conversation. */
function readMessages(
  v: unknown,
  maxImageBytes: number,
): { messages: Message[] } | { error: string } {
  if (!Array.isArray(v) || v.length === 0) return { error: "messages must be a non-empty array" };
  if (v.length > MAX_MESSAGES) return { error: `messages has more than ${MAX_MESSAGES} entries` };
  const messages: Message[] = [];
  let images = 0;
  for (const [i, m] of v.entries()) {
    if (typeof m !== "object" || m === null) return { error: `messages[${i}] is not an object` };
    const { role, content } = m as { role?: unknown; content?: unknown };
    if (typeof role !== "string" || !["system", "user", "assistant"].includes(role))
      return { error: `messages[${i}].role must be system, user or assistant` };
    if (typeof content === "string") {
      messages.push({ role, content });
      continue;
    }
    if (!Array.isArray(content) || content.length === 0)
      return { error: `messages[${i}].content must be a string or a non-empty array` };
    const parts: Part[] = [];
    for (const [j, part] of content.entries()) {
      const p = part as { type?: unknown; text?: unknown; image_url?: { url?: unknown } };
      if (p?.type === "text" && typeof p.text === "string") {
        parts.push({ type: "text", text: p.text });
      } else if (p?.type === "image_url" && typeof p.image_url?.url === "string") {
        const bytes = dataUriBytes(p.image_url.url);
        if (bytes === null)
          return { error: `messages[${i}].content[${j}] is not a png/jpeg/webp base64 data URI` };
        if (bytes > maxImageBytes) return { error: `messages[${i}].content[${j}] image is too large` };
        images += 1;
        if (images > MAX_IMAGES) return { error: `more than ${MAX_IMAGES} images in one request` };
        parts.push({ type: "image_url", image_url: { url: p.image_url.url } });
      } else {
        return { error: `messages[${i}].content[${j}] is not a text or image_url part` };
      }
    }
    messages.push({ role, content: parts });
  }
  return { messages };
}

/**
 * The app-facing reply: the upstream OpenAI body with `reasoning_content` and `usage.neurons`
 * dropped. Hidden reasoning can be larger than the answer (95 tokens for a ~10-token reply in the
 * probe), and it is not part of the contract. The neurons are kept aside for the quota.
 */
function shapeReply(raw: unknown): (ModelOut & { ok: true }) | null {
  const o = raw as
    | {
        choices?: { message?: { content?: unknown }; finish_reason?: unknown }[];
        usage?: { prompt_tokens?: unknown; completion_tokens?: unknown; total_tokens?: unknown; neurons?: unknown };
      }
    | null;
  const choice = o?.choices?.[0];
  const content = choice?.message?.content;
  // An empty completion is not an answer: it is retried once by the caller.
  if (typeof content !== "string" || content.trim() === "") return null;
  const n = (x: unknown): number => (typeof x === "number" && Number.isFinite(x) ? x : 0);
  const cost = n((o?.usage as { cost?: unknown } | undefined)?.cost);
  const inTokens = n(o?.usage?.prompt_tokens);
  const outTokens = n(o?.usage?.completion_tokens);
  return {
    ok: true,
    text: content,
    inTokens,
    outTokens,
    neurons: n(o?.usage?.neurons) || cost / USD_PER_NEURON,
    reply: {
      choices: [
        {
          index: 0,
          message: { role: "assistant", content },
          finish_reason: typeof choice?.finish_reason === "string" ? choice.finish_reason : "stop",
        },
      ],
      usage: { prompt_tokens: inTokens, completion_tokens: outTokens, total_tokens: n(o?.usage?.total_tokens) },
    },
  };
}

export function createAi(d: AiDeps) {
  const lim: Limits = { ...DEFAULT_LIMITS, ...d.limits };
  const now = d.now ?? Date.now;
  const store = d.db ? new AiStore(d.db) : undefined;

  /** One model turn: up to two tries, because Workers AI sometimes throws, or returns an empty
   *  completion, on a call that succeeds when repeated (about 1 in 8 in the live runs). Only the
   *  error CLASS is logged: the message can carry a provider key or the prompt (spec §5). */
  async function callModel(messages: Message[], max_tokens: number, temperature: number): Promise<ModelOut> {
    if (d.openrouter) {
      // One OpenRouter try, then Workers AI; with no binding to fall back to, a second OpenRouter try.
      let last: ModelOut = await callOpenRouter(messages, max_tokens, temperature);
      if (last.ok) return last;
      if (d.ai) return callWorkersAi(messages, max_tokens, temperature);
      last = await callOpenRouter(messages, max_tokens, temperature);
      return last;
    }
    return callWorkersAi(messages, max_tokens, temperature);
  }

  /** One OpenRouter turn. Only the status class or error class is logged. */
  async function callOpenRouter(messages: Message[], max_tokens: number, temperature: number): Promise<ModelOut> {
    const o = d.openrouter!;
    try {
      const res = await (o.fetch ?? fetch)(OPENROUTER_URL, {
        method: "POST",
        headers: { authorization: `Bearer ${o.key}`, "content-type": "application/json", "X-Title": "verSHion" },
        body: JSON.stringify({
          model: o.model,
          messages,
          response_format: { type: "json_object" },
          max_tokens,
          temperature,
          // No hidden reasoning (slow, and billed as output), and report the cost of the call.
          reasoning: { enabled: false },
          usage: { include: true },
          provider: { data_collection: o.privacy === false ? "allow" : "deny" },
        }),
        signal: AbortSignal.timeout(o.timeoutMs ?? 60_000),
      });
      if (!res.ok) {
        console.error(`ai: openrouter answered ${res.status}`);
        return { ok: false, code: "failed" };
      }
      const shaped = shapeReply(await res.json()); // r2-object: OpenRouter's reply to our call, not a request body
      if (shaped) return shaped;
      console.error("ai: openrouter returned no text");
      return { ok: false, code: "empty" };
    } catch (e) {
      console.error(`ai: openrouter call failed (${e instanceof Error ? e.name : "unknown"})`);
      return { ok: false, code: "failed" };
    }
  }

  async function callWorkersAi(messages: Message[], max_tokens: number, temperature: number): Promise<ModelOut> {
    if (!d.ai) return { ok: false, code: "failed" };
    const inputs = {
      messages,
      response_format: { type: "json_object" },
      max_tokens,
      temperature,
      // The deployment's choice only. Per-request thinking was tried for the figure fitter and
      // dropped: with the real ~45-parameter prompt it ran 280-400 s and often came back empty,
      // while a two-sentence visible "reasoning" field in the answer fixed the same mistakes in ~15 s.
      chat_template_kwargs: { enable_thinking: d.thinking === true },
    };
    let thrown = false;
    for (let attempt = 1; attempt <= 2; attempt++) {
      let raw: unknown;
      try {
        raw = await d.ai.run(d.model, inputs);
      } catch (e) {
        thrown = true;
        console.error(`ai: upstream call failed (${e instanceof Error ? e.name : "unknown"}), attempt ${attempt}`);
        continue;
      }
      thrown = false;
      const shaped = shapeReply(raw);
      if (shaped) return shaped;
      console.error(`ai: upstream returned no text, attempt ${attempt}`);
    }
    return { ok: false, code: thrown ? "failed" : "empty" };
  }

  /** The caller: a pool user, the relay key, or (keyless deployment, no Origin) the native app. */
  async function principal(req: Request): Promise<{ userId: string; credited: boolean } | Response> {
    // Same origin rule as the pool: a browser may only call from the app's own origins.
    const origin = req.headers.get("Origin");
    if (origin !== null && !d.allowedOrigins.includes(origin))
      return err(403, "origin_not_allowed", "This origin may not call the relay.");
    // With no relay key configured the native app (which sends no Origin) may use the relay: a
    // deployment without a key must stay unreachable (workers_dev = false, no route), which both
    // wrangler configs do. Browsers always send Origin, so a public page cannot use an unkeyed relay
    // even if it learns the URL.
    const token = bearer(req);
    const who = await d.authenticate(req);
    // Only a signed-in account is on credits: the relay key and a keyless native client have no identity to count against.
    if (who) return { userId: who.userId, credited: true };
    if (d.relayKey && token && sameSecret(token, d.relayKey)) return { userId: "relay", credited: false };
    if (d.relayKey || origin !== null) return err(401, "unauthenticated", "Sign in first, or use the relay key.");
    return { userId: "native", credited: false };
  }

  async function readBody(req: Request, userId: string): Promise<Record<string, unknown> | Response> {
    try {
      // One key per request: a PDF import converts its pages in parallel, and the pool's
      // per-account slot cap (4 bodies at once) is for its own routes, not for a batch of pages.
      // The isolate byte budget still bounds the memory all of them may hold together.
      return await readJson(req, `ai:${userId}:${crypto.randomUUID()}`, { max: MAX_BODY_BYTES });
    } catch (e) {
      if (e instanceof PoolError) {
        const code = e.status === 413 ? "payload_too_large" : e.status === 415 ? "unsupported_media_type" : "bad_request";
        return err(e.status, code, "The request body was refused.");
      }
      return err(400, "bad_request", "The body is not JSON.");
    }
  }

  /** The account's daily neuron cap and this caller's hourly turn cap; null when both have room.
   *  Counted from usage rows, so two requests racing at the line can both pass: the cap is a
   *  budget guard, not an exact meter. */
  async function overQuota(userId: string, onSession = false): Promise<Response | null> {
    if (!store) return null;
    try {
      return await checkQuota(store, userId, onSession);
    } catch {
      // Fail closed: the binding is billed to one account, so a quota that cannot be read (for
      // instance migrations 0116/0117 not applied) must not become an open relay.
      console.error("ai: quota unavailable");
      return err(503, "quota_unavailable", "The AI service is not ready. Try again later.");
    }
  }

  async function checkQuota(store: AiStore, userId: string, onSession: boolean): Promise<Response | null> {
    const t = now();
    const untilMidnight = { "Retry-After": String(Math.ceil((utcDayStart(t) + DAY_MS - t) / 1000)) };
    if ((await store.neuronsSince(utcDayStart(t))) >= lim.dailyNeurons)
      return err(429, "quota_exhausted", "The AI allowance for today is used up.", untilMidnight);
    // A credit session has its own call budget; the per-hour turn cap is for callers without one.
    if (!onSession && (await store.chatTurnsSince(userId, t - HOUR_MS)) >= lim.chatPerHour)
      return err(429, "rate_limited", "Too many requests this hour. Try again later.", { "Retry-After": "600" });
    return null;
  }

  /** `GET /ai/credits`: the weekly balance and the price list. */
  async function balance(userId: string, credited: boolean): Promise<Response> {
    if (!credited || !store) return json(200, { unlimited: true, services: describeServices() });
    const t = now();
    const weekStart = utcWeekStart(t);
    try {
      await store.refundExpired(userId, t);
      const used = await store.creditsSpent(userId, weekStart, t);
      const purchased = await store.purchasedBalance(userId);
      return json(200, { allowance: lim.weeklyCredits, used, left: Math.max(0, lim.weeklyCredits - used), purchased, resets_at: weekStart + WEEK_MS, services: describeServices() });
    } catch {
      console.error("ai: credits unavailable");
      return err(503, "quota_unavailable", "The AI service is not ready. Try again later.");
    }
  }

  /** `POST /ai/service`: prices a service, charges its credits and opens a session with a call budget. */
  async function startService(body: Record<string, unknown>, userId: string, credited: boolean): Promise<Response> {
    if (!isServiceId(body.service)) return err(400, "bad_request", "Unknown service.");
    const def = SERVICES[body.service];
    let pages = 0;
    if (def.takesPages) {
      pages = typeof body.pages === "number" ? Math.floor(body.pages) : 0;
      if (!Number.isFinite(pages) || pages < 1) return err(400, "bad_request", "pages must be a positive whole number.");
      if (pages > def.maxPages) return err(400, "too_many_pages", `At most ${def.maxPages} pages for this service.`);
    }
    // No identity to count credits against (the app's relay key, a keyless native client): no charge, no session.
    if (!credited || !store || d.credits === false) return json(200, { session: null, credits: 0, unlimited: true });
    const t = now();
    const weekStart = utcWeekStart(t);
    const credits = def.credits(pages);
    try {
      if ((await store.neuronsSince(utcDayStart(t))) >= lim.dailyNeurons)
        return err(429, "quota_exhausted", "The AI allowance for today is used up.", { "Retry-After": String(Math.ceil((utcDayStart(t) + DAY_MS - t) / 1000)) });
      await store.refundExpired(userId, t);
      // The weekly allowance pays first; the purchased balance covers the rest. A negative balance (a refund
      // took back credits already spent) covers nothing. One retry: a racing request can move either count.
      for (let attempt = 0; attempt < 2; attempt++) {
        const used = await store.creditsSpent(userId, weekStart, t);
        const purchased = Math.max(0, await store.purchasedBalance(userId));
        const freeLeft = Math.max(0, lim.weeklyCredits - used);
        const paid = Math.max(0, credits - freeLeft);
        if (paid > purchased) {
          return json(402, { error: { code: "insufficient_credits", message: `This needs ${credits} credits and you have ${freeLeft} left this week and ${purchased} purchased.` }, needed: credits, left: freeLeft, purchased, resets_at: weekStart + WEEK_MS });
        }
        const id = crypto.randomUUID().replace(/-/g, "");
        if (await store.startSession({ id, principal: userId, service: body.service, credits, paid, callsAllowed: def.calls(pages), pages, now: t, expiresAt: t + def.ttlMs, weekStart, allowance: lim.weeklyCredits })) {
          return json(200, { session: id, credits, paid, left: freeLeft - (credits - paid), purchased: purchased - paid, calls: def.calls(pages), expires_at: t + def.ttlMs, resets_at: weekStart + WEEK_MS });
        }
      }
      return err(429, "busy", "Another request is using your credits. Try again.");
    } catch {
      console.error("ai: could not start a service session");
      return err(503, "quota_unavailable", "The AI service is not ready. Try again later.");
    }
  }

  async function handle(req: Request, ctx?: { waitUntil(p: Promise<unknown>): void }): Promise<Response | null> {
    const url = new URL(req.url);
    // Unauthenticated and data-free: the Settings "Test connection" button checks the relay is up.
    if (url.pathname === AI_HEALTH_PATH) {
      if (req.method !== "GET") return err(405, "method_not_allowed", `Use GET ${AI_HEALTH_PATH}.`, { Allow: "GET" });
      return json(200, { ok: true, model: d.openrouter?.model ?? d.model, ai: d.ai !== undefined || d.openrouter !== undefined, upstream: d.openrouter ? "openrouter" : "workers" });
    }
    const job = url.pathname === AI_IMPORT_PATH ? "start" : AI_IMPORT_ONE.test(url.pathname) ? "read" : null;
    const credits = url.pathname === AI_CREDITS_PATH ? "balance" : url.pathname === AI_SERVICE_PATH ? "start" : null;
    if (url.pathname !== AI_PATH && job === null && credits === null) return null;
    if (req.method === "OPTIONS") return new Response(null, { status: 204 });
    const want = job === "read" || credits === "balance" ? "GET" : "POST";
    if (req.method !== want) return err(405, "method_not_allowed", `Use ${want} ${url.pathname.replace(/[0-9a-f]{32}$/, ":id")}.`, { Allow: `${want}, OPTIONS` });

    const who = await principal(req);
    if (who instanceof Response) return who;
    const { userId, credited } = who;
    if (credits === "balance") return balance(userId, credited);
    if (!d.ai && !d.openrouter) return err(503, "ai_not_configured", "This deployment has no Workers AI binding.");

    if (job !== null && !store) return err(503, "jobs_not_configured", "This deployment has no database for import jobs.");
    // The background import job is not priced in credits, so a signed-in account may not use it.
    if (job !== null && credited && d.credits !== false) return err(403, "use_service_session", "Start an AI service session instead of an import job.");
    const jobDeps = () => ({
      store: store!,
      bucket: d.bucket,
      now,
      limits: lim,
      model: (messages: Message[]) => callModel(messages, lim.maxOutTokens, 0),
    });
    if (job === "read") return readJob(jobDeps(), url.pathname.slice(AI_IMPORT_PATH.length + 1), userId);

    const body = await readBody(req, userId);
    if (body instanceof Response) return body;
    if (credits === "start") return startService(body, userId, credited);

    if (job === "start") {
      const over = await overQuota(userId);
      if (over) return over;
      // Without a context (tests) the job still runs; the caller awaits it through the returned promise.
      return startJob(jobDeps(), body, userId, (p) => (ctx ? ctx.waitUntil(p) : void p.catch(() => {})));
    }

    const read = readMessages(body.messages, lim.maxImageBytes);
    if ("error" in read) return err(400, "bad_request", read.error);
    // A signed-in account must be inside a service session it paid for; the session's call budget replaces the hourly cap.
    let session: string | null = null;
    if (credited && store && d.credits !== false) {
      const given = typeof body.session === "string" && /^[0-9a-f]{32}$/.test(body.session) ? body.session : null;
      if (!given) return err(402, "session_required", "Start the service from the app first (credits).");
      let granted = false;
      try {
        granted = await store.takeCall(given, userId, now());
      } catch {
        console.error("ai: session store unavailable");
        return err(503, "quota_unavailable", "The AI service is not ready. Try again later.");
      }
      if (!granted) return err(402, "session_exhausted", "This AI session has ended or used its calls. Start the service again.");
      session = given;
    }
    const over = await overQuota(userId, session !== null);
    if (over) return over;

    const requested = typeof body.max_tokens === "number" ? Math.floor(body.max_tokens) : lim.maxOutTokens;
    const max_tokens = Math.max(1, Math.min(requested, lim.maxOutTokens));
    const t = typeof body.temperature === "number" && Number.isFinite(body.temperature) ? body.temperature : 0.2;
    const temperature = Math.max(0, Math.min(t, 1));

    const out = await callModel(read.messages, max_tokens, temperature);
    if (!out.ok) return err(502, "model_failed", out.code === "failed" ? "The model call failed. Try again." : "The model returned no text.");
    if (store) {
      // A usage row that fails to write must not cost the caller an answer they already paid for.
      try {
        if (session) await store.callAnswered(session);
        await store.record(userId, { kind: "chat", inTokens: out.inTokens, outTokens: out.outTokens, neurons: out.neurons }, now());
      } catch {
        console.error("ai: could not record usage");
      }
    }
    return json(200, out.reply);
  }

  return { handle };
}
