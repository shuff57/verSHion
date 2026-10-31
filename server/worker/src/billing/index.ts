// Buying credits: Stripe PaymentIntents for the Payment Element on the web page, and the webhook that credits
// (and takes back) the ledger. Zero dependencies: Stripe's REST API over fetch and its HMAC signature over
// Web Crypto, like the rest of this Worker.
//
// The flag (`enabled`) gates BUYING only (/billing/packs and /billing/intent answer 404 when off). The webhook
// stays live whenever its secret is set, so a refund or dispute on an earlier sale is never missed.
import { AiStore } from "../ai/store.ts";
import { PoolError, readBody } from "../pool/shared.ts";

export const BILLING_PACKS_PATH = "/billing/packs";
export const BILLING_INTENT_PATH = "/billing/intent";
export const BILLING_WEBHOOK_PATH = "/billing/webhook";

const STRIPE_API = "https://api.stripe.com/v1";
const STRIPE_VERSION = "2025-12-15.clover";
const WEBHOOK_TOLERANCE_S = 300;
const WEBHOOK_BODY_MAX = 64 * 1024;
const INTENT_BODY_MAX = 1024;

export interface Pack {
  id: string;
  /** Price in whole US dollars. */
  usd: number;
  credits: number;
}

/**
 * 25 credits per dollar (a credit is 4 cents; handwriting is a credit a page), with a small bonus on the larger
 * packs. Still to be checked against the measured cost per action on real tests (see docs/billing.md).
 */
export const CREDITS_PER_USD = 25;
export const DEFAULT_PACKS: Pack[] = [
  { id: "small", usd: 5, credits: 125 },
  { id: "medium", usd: 15, credits: 400 },
  { id: "large", usd: 40, credits: 1100 },
];

export interface BillingDeps {
  /** Off by default: buying is unavailable (404). */
  enabled: boolean;
  /** `wrangler secret put STRIPE_SECRET_KEY` (an sk_test_ key until launch). */
  stripeKey?: string;
  /** `wrangler secret put STRIPE_WEBHOOK_SECRET` (whsec_...), for this Worker's own endpoint. */
  webhookSecret?: string;
  /** Shown to the page that mounts the Payment Element. */
  publishableKey?: string;
  db?: D1Database;
  authenticate(req: Request): Promise<{ userId: string } | null>;
  /** The auth package's D1RateLimitStore (or any store with this shape). Absent: no limit on creating payments. */
  rateStore?: { hit(key: string, rule: { limit: number; windowSeconds: number }, now: number): Promise<{ allowed: boolean; retryAfter: number }> };
  /** Payments one account may start per hour (default 10). */
  intentsPerHour?: number;
  packs?: Pack[];
  /** Credits per dollar for a custom amount (default 25), and its whole-dollar range. */
  creditsPerUsd?: number;
  customMinUsd?: number;
  customMaxUsd?: number;
  now?: () => number;
  fetch?: typeof fetch;
}

const json = (status: number, body: unknown, headers: Record<string, string> = {}): Response =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store", ...headers } });
const err = (status: number, code: string, message: string): Response => json(status, { error: { code, message } });

function hex(b: ArrayBuffer): string {
  return Array.from(new Uint8Array(b), (x) => x.toString(16).padStart(2, "0")).join("");
}

/** Constant-time string equality (both are short hex digests). */
function same(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let r = 0;
  for (let i = 0; i < a.length; i++) r |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return r === 0;
}

export async function stripeSignature(secret: string, t: number, payload: string | Uint8Array): Promise<string> {
  const key = await crypto.subtle.importKey("raw", new TextEncoder().encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const head = new TextEncoder().encode(`${t}.`);
  const body = typeof payload === "string" ? new TextEncoder().encode(payload) : payload;
  const signed = new Uint8Array(head.length + body.length);
  signed.set(head);
  signed.set(body, head.length);
  return hex(await crypto.subtle.sign("HMAC", key, signed));
}

/** Verifies a `Stripe-Signature` header (`t=...,v1=...`) against the raw body. */
export async function verifyStripe(secret: string, header: string, payload: string | Uint8Array, nowS: number): Promise<boolean> {
  let t = NaN;
  const sigs: string[] = [];
  for (const part of header.split(",")) {
    const [k, v] = part.trim().split("=");
    if (k === "t") t = Number(v);
    else if (k === "v1" && v) sigs.push(v);
  }
  if (!Number.isFinite(t) || Math.abs(nowS - t) > WEBHOOK_TOLERANCE_S || sigs.length === 0) return false;
  const want = await stripeSignature(secret, t, payload);
  return sigs.some((s) => same(s, want));
}

/** The body through the Worker's one body reader (Content-Length required, size and concurrency capped), or the refusal. */
async function boundedBytes(req: Request, max: number, key: string): Promise<Uint8Array | Response> {
  try {
    const b = await readBody(req, { max, key, optional: true });
    b.release();
    return b.bytes;
  } catch (e) {
    if (e instanceof PoolError) return json(e.status, { error: { code: e.code, message: "The request body was refused." } }, e.headers);
    return err(400, "bad_request", "Could not read the body.");
  }
}

export function createBilling(d: BillingDeps) {
  const packs = d.packs ?? DEFAULT_PACKS;
  const perUsd = d.creditsPerUsd ?? CREDITS_PER_USD;
  const minUsd = d.customMinUsd ?? 5;
  const maxUsd = d.customMaxUsd ?? 200;
  const now = d.now ?? Date.now;
  const call = d.fetch ?? fetch;
  const store = d.db ? new AiStore(d.db) : null;

  /** What a payment of `cents` is worth: a pack's credits when the amount is exactly a pack's price, else the custom rate. */
  function creditsFor(cents: number, pack?: string): number | null {
    if (pack !== undefined) {
      const p = packs.find((x) => x.id === pack);
      return p && p.usd * 100 === cents ? p.credits : null;
    }
    if (cents % 100 !== 0) return null;
    const usd = cents / 100;
    return usd >= minUsd && usd <= maxUsd ? usd * perUsd : null;
  }

  function packsView(): Response {
    return json(200, { packs, custom: { min_usd: minUsd, max_usd: maxUsd, credits_per_usd: perUsd }, publishable_key: d.publishableKey ?? null });
  }

  /** `POST /billing/intent {pack}` or `{amount_usd}`: creates the PaymentIntent for the Payment Element. */
  async function intent(req: Request, userId: string): Promise<Response> {
    const bytes = await boundedBytes(req, INTENT_BODY_MAX, `u:${userId}`);
    if (bytes instanceof Response) return bytes;
    let body: any;
    try {
      body = JSON.parse(new TextDecoder().decode(bytes));
    } catch {
      body = null;
    }
    if (!body || typeof body !== "object") return err(400, "bad_request", "Send a JSON body.");
    let cents: number;
    let pack: string | undefined;
    if (typeof body.pack === "string") {
      const p = packs.find((x) => x.id === body.pack);
      if (!p) return err(400, "bad_request", "Unknown pack.");
      cents = p.usd * 100;
      pack = p.id;
    } else if (typeof body.amount_usd === "number" && Number.isInteger(body.amount_usd)) {
      cents = body.amount_usd * 100;
    } else {
      return err(400, "bad_request", "Choose a pack or a whole-dollar amount.");
    }
    const credits = creditsFor(cents, pack);
    if (credits === null) return err(400, "bad_amount", `Choose between $${minUsd} and $${maxUsd}, in whole dollars.`);
    // Counted only for a valid request: each one creates an object at Stripe, and a card-testing script would spam them.
    if (d.rateStore) {
      const hit = await d.rateStore.hit(`billing:intent:${userId}`, { limit: d.intentsPerHour ?? 10, windowSeconds: 3600 }, now());
      if (!hit.allowed) return json(429, { error: { code: "rate_limited", message: "Too many payment attempts. Try again later." } }, { "retry-after": String(hit.retryAfter) });
    }
    const form = new URLSearchParams({
      amount: String(cents),
      currency: "usd",
      description: `${credits} AI credits`,
      "automatic_payment_methods[enabled]": "true",
      "metadata[purchase_type]": "ai_credits",
      "metadata[user_id]": userId,
      "metadata[credits]": String(credits),
    });
    if (pack) form.set("metadata[pack]", pack);
    try {
      const res = await call(`${STRIPE_API}/payment_intents`, {
        method: "POST",
        headers: { authorization: `Bearer ${d.stripeKey}`, "content-type": "application/x-www-form-urlencoded", "stripe-version": STRIPE_VERSION },
        body: form,
      });
      const pi = (await res.json()) as { client_secret?: string; id?: string }; // r2-object: Stripe's reply to our call, not a request body
      if (!res.ok || !pi.client_secret) throw new Error(`stripe ${res.status}`);
      return json(200, { client_secret: pi.client_secret, publishable_key: d.publishableKey ?? null, credits, amount_usd: cents / 100 });
    } catch {
      console.error("billing: could not create a payment");
      return err(502, "payment_unavailable", "Could not start the payment. Try again.");
    }
  }

  /** Credits a succeeded payment once; the amount is re-checked against what was priced, never trusted from metadata alone. */
  async function fulfil(pi: any): Promise<void> {
    if (!store) throw new Error("no db");
    const meta = pi?.metadata ?? {};
    if (meta.purchase_type !== "ai_credits" || typeof meta.user_id !== "string" || !meta.user_id) return;
    const credits = creditsFor(Number(pi.amount_received), typeof meta.pack === "string" ? meta.pack : undefined);
    if (credits === null || String(credits) !== meta.credits) {
      console.error("billing: payment does not match a price, not credited");
      return;
    }
    // A payment that lands after the account was deleted credits nothing (the row would outlive the erasure); the
    // charge is logged for a manual refund in Stripe.
    if (await store.accountDeleted(meta.user_id)) {
      console.error(`billing: payment ${String(pi.id)} is for a deleted account, refund it in Stripe`);
      return;
    }
    await store.ledger(meta.user_id, credits, "purchase", String(pi.id), now(), Number(pi.amount_received));
  }

  /**
   * Takes back credits for a refund or dispute: the share of the purchase that was returned, minus what earlier
   * events already took back. May leave the balance negative, which blocks purchased spending.
   */
  async function reverse(piId: string, returnedCents: number, tag: string): Promise<void> {
    if (!store || typeof piId !== "string") return;
    const bought = await store.purchase(piId);
    if (!bought || !(returnedCents > 0)) return; // not a payment of ours
    const target = Math.min(bought.credits, Math.ceil((bought.credits * Math.min(returnedCents, bought.cents)) / bought.cents));
    const owed = target - (await store.reversedFor(`${piId}#`));
    if (owed > 0) await store.ledger(bought.principal, -owed, "reversal", `${piId}#${tag}`, now());
  }

  async function webhook(req: Request): Promise<Response> {
    if (req.method !== "POST") return err(405, "method_not_allowed", "Use POST.");
    if (!d.webhookSecret) return err(503, "billing_not_configured", "Billing is not configured.");
    const sig = req.headers.get("stripe-signature");
    const payload = await boundedBytes(req, WEBHOOK_BODY_MAX, "stripe:webhook");
    if (payload instanceof Response) return payload;
    if (!sig || !(await verifyStripe(d.webhookSecret, sig, payload, Math.floor(now() / 1000)))) return err(400, "bad_signature", "Invalid signature.");
    let event: any;
    try {
      event = JSON.parse(new TextDecoder().decode(payload));
    } catch {
      return err(400, "bad_request", "Invalid body.");
    }
    try {
      const o = event?.data?.object;
      switch (event?.type) {
        case "payment_intent.succeeded":
          await fulfil(o);
          break;
        case "charge.refunded":
          await reverse(String(o?.payment_intent ?? ""), Number(o?.amount_refunded), `refund:${Number(o?.amount_refunded)}`);
          break;
        case "charge.dispute.created":
          await reverse(String(o?.payment_intent ?? ""), Number(o?.amount), `dispute:${String(o?.id)}`);
          break;
      }
    } catch {
      // Not acknowledged: Stripe redelivers, and every handler above is idempotent.
      console.error("billing: webhook handler failed");
      return err(500, "handler_failed", "Try again.");
    }
    return json(200, { received: true });
  }

  /** Null when the path is not a billing route. */
  async function handle(req: Request): Promise<Response | null> {
    const path = new URL(req.url).pathname;
    if (path === BILLING_WEBHOOK_PATH) return webhook(req);
    if (path !== BILLING_PACKS_PATH && path !== BILLING_INTENT_PATH) return null;
    if (req.method === "OPTIONS") return new Response(null, { status: 204 });
    if (!d.enabled || !d.stripeKey || !store) return err(404, "not_found", "Not found.");
    const want = path === BILLING_PACKS_PATH ? "GET" : "POST";
    if (req.method !== want) return err(405, "method_not_allowed", `Use ${want} ${path}.`);
    const who = await d.authenticate(req);
    if (!who) return err(401, "unauthenticated", "Sign in first.");
    return path === BILLING_PACKS_PATH ? packsView() : intent(req, who.userId);
  }

  return { handle };
}
