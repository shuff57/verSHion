// Workers AI relay for the desktop app. Contract: docs/ai-harness-spec.md §2.
//
// One endpoint, OpenAI-shaped in and out, so the app's existing non-Ollama path
// (`ai.rs::chat_turns` -> `choices/0/message/content`) and its base64 data-URI images work
// unchanged. The model is the worker's choice: a client's `model` field is ignored.
//
// The body is read by the pool's one guarded reader (`readJson`), never directly: the Worker's
// I4/I5 lint refuses any other direct body read in `src/`.
import { PoolError, readJson } from "../pool/shared.ts";

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
  /**
   * Gemma 4's built-in thinking mode. Off by default: on a page transcription the hidden
   * reasoning was ~85-90% of the completion tokens (1.6-3.4k tokens spent on a ~200-token JSON
   * answer), which is where the latency and the empty/truncated replies came from. `AI_THINKING=1`
   * turns it back on for every request.
   */
  thinking?: boolean;
  /** Test seams; production uses the constants. */
  limits?: { maxImageBytes?: number; maxOutTokens?: number };
}

export const AI_PATH = "/ai/chat/completions";
export const AI_HEALTH_PATH = "/ai/health";
export const AI_DEFAULT_MODEL = "@cf/google/gemma-4-26b-a4b-it";

const MAX_BODY_BYTES = 13 * 1024 * 1024;
const MAX_IMAGE_BYTES = 4 * 1024 * 1024;
const MAX_IMAGES = 4;
const MAX_OUT_TOKENS = 8192;
const MAX_MESSAGES = 40;

type Part = { type: "text"; text: string } | { type: "image_url"; image_url: { url: string } };
type Message = { role: string; content: string | Part[] };

const json = (status: number, body: unknown, headers: Record<string, string> = {}): Response =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json", ...headers } });

const err = (status: number, code: string, message: string, headers?: Record<string, string>): Response =>
  json(status, { error: { code, message } }, headers);

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

/** Decoded byte count of a base64 image data URI, or null when the URI is not one we accept. */
function dataUriBytes(url: string): number | null {
  const m = /^data:image\/(?:png|jpeg|webp);base64,([A-Za-z0-9+/=]+)$/.exec(url);
  return m ? Math.floor((m[1]!.length * 3) / 4) : null;
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
 * probe), and it is not part of the contract.
 */
function shapeReply(raw: unknown): Record<string, unknown> | null {
  const o = raw as
    | {
        choices?: { message?: { content?: unknown }; finish_reason?: unknown }[];
        usage?: { prompt_tokens?: unknown; completion_tokens?: unknown; total_tokens?: unknown };
      }
    | null;
  const choice = o?.choices?.[0];
  const content = choice?.message?.content;
  // An empty completion is not an answer: it is retried once by the caller.
  if (typeof content !== "string" || content.trim() === "") return null;
  const n = (x: unknown): number => (typeof x === "number" && Number.isFinite(x) ? x : 0);
  return {
    choices: [
      {
        index: 0,
        message: { role: "assistant", content },
        finish_reason: typeof choice?.finish_reason === "string" ? choice.finish_reason : "stop",
      },
    ],
    usage: {
      prompt_tokens: n(o?.usage?.prompt_tokens),
      completion_tokens: n(o?.usage?.completion_tokens),
      total_tokens: n(o?.usage?.total_tokens),
    },
  };
}

export function createAi(d: AiDeps) {
  const maxImageBytes = d.limits?.maxImageBytes ?? MAX_IMAGE_BYTES;
  const maxOutTokens = d.limits?.maxOutTokens ?? MAX_OUT_TOKENS;

  async function handle(req: Request): Promise<Response | null> {
    const url = new URL(req.url);
    // Unauthenticated and data-free: the Settings "Test connection" button checks the relay is up.
    if (url.pathname === AI_HEALTH_PATH) {
      if (req.method !== "GET") return err(405, "method_not_allowed", `Use GET ${AI_HEALTH_PATH}.`, { Allow: "GET" });
      return json(200, { ok: true, model: d.model, ai: d.ai !== undefined });
    }
    if (url.pathname !== AI_PATH) return null;
    if (req.method === "OPTIONS") return new Response(null, { status: 204 });
    if (req.method !== "POST")
      return err(405, "method_not_allowed", `Use POST ${AI_PATH}.`, { Allow: "POST, OPTIONS" });

    // Same origin rule as the pool: a browser may only call from the app's own origins.
    const origin = req.headers.get("Origin");
    if (origin !== null && !d.allowedOrigins.includes(origin))
      return err(403, "origin_not_allowed", "This origin may not call the relay.");

    // A signed-in pool user, or the desktop app's relay key. With no relay key configured the
    // native app (which sends no Origin) may use the relay: a deployment without a key must stay
    // unreachable (workers_dev = false, no route), which both wrangler configs do. Browsers always
    // send Origin, so a public page cannot use an unkeyed relay even if it learns the URL.
    const token = bearer(req);
    const who = await d.authenticate(req);
    let userId = who?.userId ?? (d.relayKey && token && sameSecret(token, d.relayKey) ? "relay" : null);
    if (!userId) {
      if (d.relayKey || origin !== null) return err(401, "unauthenticated", "Sign in first, or use the relay key.");
      userId = "native";
    }
    if (!d.ai) return err(503, "ai_not_configured", "This deployment has no Workers AI binding.");

    let body: Record<string, unknown>;
    try {
      // One key per request: a PDF import converts its pages in parallel, and the pool's
      // per-account slot cap (4 bodies at once) is for its own routes, not for a batch of pages.
      // The isolate byte budget still bounds the memory all of them may hold together.
      body = await readJson(req, `ai:${userId}:${crypto.randomUUID()}`, { max: MAX_BODY_BYTES });
    } catch (e) {
      if (e instanceof PoolError) {
        const code = e.status === 413 ? "payload_too_large" : e.status === 415 ? "unsupported_media_type" : "bad_request";
        return err(e.status, code, "The request body was refused.");
      }
      return err(400, "bad_request", "The body is not JSON.");
    }

    const read = readMessages(body.messages, maxImageBytes);
    if ("error" in read) return err(400, "bad_request", read.error);

    const requested = typeof body.max_tokens === "number" ? Math.floor(body.max_tokens) : maxOutTokens;
    const max_tokens = Math.max(1, Math.min(requested, maxOutTokens));
    const t = typeof body.temperature === "number" && Number.isFinite(body.temperature) ? body.temperature : 0.2;
    const temperature = Math.max(0, Math.min(t, 1));

    const inputs = {
      messages: read.messages,
      response_format: { type: "json_object" },
      max_tokens,
      temperature,
      // The deployment's choice only. Per-request thinking was tried for the figure fitter and
      // dropped: with the real ~45-parameter prompt it ran 280-400 s and often came back empty,
      // while a two-sentence visible "reasoning" field in the answer fixed the same mistakes in ~15 s.
      chat_template_kwargs: { enable_thinking: d.thinking === true },
    };
    // Up to two tries: Workers AI sometimes throws, or returns an empty completion, on a call that
    // succeeds when repeated (about 1 call in 8 in the live runs). A third failure is the caller's.
    // Only the error CLASS is logged: the message can carry a provider key or the prompt (spec §5).
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
      if (shaped) return json(200, shaped);
      console.error(`ai: upstream returned no text, attempt ${attempt}`);
    }
    return err(502, "model_failed", thrown ? "The model call failed. Try again." : "The model returned no text.");
  }

  return { handle };
}
