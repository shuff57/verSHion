// Run: npm test. Buying credits: PaymentIntents for the Payment Element and the signed webhook, over real SQLite.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createBilling, stripeSignature, verifyStripe, DEFAULT_PACKS, type BillingDeps } from "../src/billing/index.ts";
import { AiStore } from "../src/ai/store.ts";
import { deleteContentStatements } from "../src/pool/index.ts";
import { d1, freshDb } from "./pool/harness.ts";

const NOW = Date.UTC(2026, 9, 30, 12, 0, 0);
const SECRET = "whsec_test_secret";
const lengthOf = (s: string) => String(new TextEncoder().encode(s).length);

/** A fixed-count stand-in for the auth package's D1RateLimitStore (its TypeScript does not run under strip-only mode). */
function counter() {
  const n = new Map<string, number>();
  return {
    hit: async (key: string, rule: { limit: number; windowSeconds: number }) => {
      const c = (n.get(key) ?? 0) + 1;
      n.set(key, c);
      return c <= rule.limit ? { allowed: true, retryAfter: 0 } : { allowed: false, retryAfter: rule.windowSeconds };
    },
  };
}

function world(o: Partial<BillingDeps> = {}) {
  const db = freshDb();
  const sent: { url: string; form: URLSearchParams; auth: string | null }[] = [];
  const stripeFetch = (async (url: string, init: any) => {
    sent.push({ url, form: new URLSearchParams(String(init.body)), auth: init.headers.authorization });
    return new Response(JSON.stringify({ id: "pi_new", client_secret: "pi_new_secret_x" }), { status: 200 });
  }) as unknown as typeof fetch;
  const billing = createBilling({
    enabled: true,
    stripeKey: "sk_test_x",
    webhookSecret: SECRET,
    publishableKey: "pk_test_x",
    db: d1(db),
    authenticate: async (r) => (r.headers.get("x-user") ? { userId: r.headers.get("x-user")! } : null),
    now: () => NOW,
    fetch: stripeFetch,
    ...o,
  });
  const store = new AiStore(d1(db));
  const send = async (path: string, body?: unknown, user: string | null = "u1", method = "POST") => {
    const text = body === undefined ? undefined : JSON.stringify(body);
    const res = await billing.handle(
      new Request("https://w.example" + path, {
        method,
        headers: { "content-type": "application/json", ...(text !== undefined ? { "content-length": lengthOf(text) } : {}), ...(user ? { "x-user": user } : {}) },
        body: text,
      }),
    );
    return { status: res!.status, body: (await res!.json()) as any, headers: res!.headers };
  };
  // A webhook as Stripe sends it: the raw body, signed with the endpoint secret.
  const hook = async (type: string, object: unknown, o: { secret?: string; t?: number; id?: string } = {}) => {
    const payload = JSON.stringify({ id: o.id ?? "evt_1", type, data: { object } });
    const t = o.t ?? Math.floor(NOW / 1000);
    const sig = `t=${t},v1=${await stripeSignature(o.secret ?? SECRET, t, payload)}`;
    const res = await billing.handle(new Request("https://w.example/billing/webhook", { method: "POST", headers: { "stripe-signature": sig, "content-length": lengthOf(payload) }, body: payload }));
    return { status: res!.status, body: (await res!.json()) as any };
  };
  return { send, hook, store, sent, db };
}

// The $15 pack: 400 credits.
const paid = (id = "pi_1", user = "u1", pack: string | undefined = "medium", cents = 1500, credits = 400) => ({
  id,
  amount_received: cents,
  metadata: { purchase_type: "ai_credits", user_id: user, credits: String(credits), ...(pack ? { pack } : {}) },
});

test("the default packs are 25 credits a dollar or better", () => {
  for (const p of DEFAULT_PACKS) assert.ok(p.credits >= p.usd * 25, p.id);
  assert.deepEqual(DEFAULT_PACKS.map((p) => [p.usd, p.credits]), [[5, 125], [15, 400], [40, 1100]]);
});

test("buying is off by default and the webhook still answers", async () => {
  const w = world({ enabled: false });
  assert.equal((await w.send("/billing/packs", undefined, "u1", "GET")).status, 404);
  assert.equal((await w.send("/billing/intent", { pack: "small" })).status, 404);
  assert.equal((await w.hook("payment_intent.succeeded", paid())).status, 200);
  assert.equal(await w.store.purchasedBalance("u1"), 400, "a sale made before the flag went off is still credited");
});

test("packs and intents need a signed-in account", async () => {
  const w = world();
  assert.equal((await w.send("/billing/packs", undefined, null, "GET")).status, 401);
  assert.equal((await w.send("/billing/intent", { pack: "small" }, null)).status, 401);
  const p = await w.send("/billing/packs", undefined, "u1", "GET");
  assert.equal(p.status, 200);
  assert.equal(p.body.publishable_key, "pk_test_x");
  assert.equal(p.body.packs.length, 3);
  assert.equal(p.body.custom.credits_per_usd, 25);
});

test("a pack intent is priced by the server and tagged with the account", async () => {
  const w = world();
  const r = await w.send("/billing/intent", { pack: "medium", amount_usd: 1, credits: 99999 });
  assert.equal(r.status, 200);
  assert.deepEqual([r.body.client_secret, r.body.credits, r.body.amount_usd], ["pi_new_secret_x", 400, 15]);
  const f = w.sent[0]!.form;
  assert.equal(w.sent[0]!.url, "https://api.stripe.com/v1/payment_intents");
  assert.equal(w.sent[0]!.auth, "Bearer sk_test_x");
  assert.deepEqual([f.get("amount"), f.get("currency"), f.get("metadata[user_id]"), f.get("metadata[credits]"), f.get("metadata[pack]")], ["1500", "usd", "u1", "400", "medium"]);
});

test("a custom amount is whole dollars inside the range, at the custom rate", async () => {
  const w = world();
  const r = await w.send("/billing/intent", { amount_usd: 20 });
  assert.deepEqual([r.status, r.body.credits], [200, 500]);
  for (const bad of [{ amount_usd: 4 }, { amount_usd: 201 }, { amount_usd: 7.5 }, { amount_usd: "9" }, { amount_usd: -5 }, { pack: "nope" }, {}]) {
    assert.equal((await w.send("/billing/intent", bad)).status, 400, JSON.stringify(bad));
  }
  assert.equal(w.sent.length, 1, "nothing invalid reached Stripe");
});

test("a body without a Content-Length or past the cap is refused, never read", async () => {
  const w = world();
  const none = await createBilling({ enabled: true, stripeKey: "sk", db: d1(freshDb()), authenticate: async () => ({ userId: "u1" }) }).handle(
    new Request("https://w.example/billing/intent", { method: "POST", headers: { "content-type": "application/json" }, body: new ReadableStream({ start: (c) => (c.enqueue(new TextEncoder().encode("{}")), c.close()) }), duplex: "half" } as RequestInit),
  );
  assert.equal(none!.status, 411);
  const big = "x".repeat(2000);
  const r = await w.send("/billing/intent", { pack: "small", pad: big });
  assert.equal(r.status, 413);
  assert.equal(w.sent.length, 0);
});

test("creating payments is rate limited per account, and invalid requests do not count", async () => {
  const w = world({ rateStore: counter(), intentsPerHour: 2 });
  for (let i = 0; i < 5; i++) assert.equal((await w.send("/billing/intent", { amount_usd: 1 })).status, 400);
  assert.equal((await w.send("/billing/intent", { pack: "small" })).status, 200);
  assert.equal((await w.send("/billing/intent", { pack: "small" })).status, 200);
  const r = await w.send("/billing/intent", { pack: "small" });
  assert.equal(r.status, 429);
  assert.equal(r.body.error.code, "rate_limited");
  assert.ok(Number(r.headers.get("retry-after")) > 0);
  assert.equal((await w.send("/billing/intent", { pack: "small" }, "u2")).status, 200, "another account is unaffected");
  assert.equal(w.sent.length, 3);
});

test("a Stripe failure is a clean 502", async () => {
  const w = world({ fetch: (async () => new Response("{}", { status: 500 })) as unknown as typeof fetch });
  const r = await w.send("/billing/intent", { pack: "small" });
  assert.equal(r.status, 502);
  assert.equal(r.body.error.code, "payment_unavailable");
});

test("signatures: wrong secret, stale time and a missing header are refused", async () => {
  const w = world();
  assert.equal((await w.hook("payment_intent.succeeded", paid(), { secret: "whsec_other" })).status, 400);
  assert.equal((await w.hook("payment_intent.succeeded", paid(), { t: Math.floor(NOW / 1000) - 3600 })).status, 400);
  const res = await createBilling({ enabled: true, webhookSecret: SECRET, authenticate: async () => null }).handle(
    new Request("https://w.example/billing/webhook", { method: "POST", headers: { "content-length": "2" }, body: "{}" }),
  );
  assert.equal(res!.status, 400);
  assert.equal(await w.store.purchasedBalance("u1"), 0);
  assert.equal(await verifyStripe(SECRET, "garbage", "{}", 0), false);
});

test("a webhook with no secret configured is a 503", async () => {
  const res = await createBilling({ enabled: true, authenticate: async () => null }).handle(new Request("https://w.example/billing/webhook", { method: "POST", body: "{}" }));
  assert.equal(res!.status, 503);
});

test("a payment credits once however often Stripe redelivers", async () => {
  const w = world();
  assert.equal((await w.hook("payment_intent.succeeded", paid())).status, 200);
  assert.equal((await w.hook("payment_intent.succeeded", paid(), { id: "evt_2" })).status, 200);
  assert.equal(await w.store.purchasedBalance("u1"), 400);
});

test("an amount that matches no price is not credited", async () => {
  const w = world();
  await w.hook("payment_intent.succeeded", paid("pi_a", "u1", "medium", 500, 400)); // a $5 payment claiming the $15 pack
  await w.hook("payment_intent.succeeded", paid("pi_b", "u1", undefined, 1500, 9999)); // metadata claims too many credits
  await w.hook("payment_intent.succeeded", paid("pi_c", "u1", undefined, 150, 37)); // $1.50 is below the minimum
  assert.equal(await w.store.purchasedBalance("u1"), 0);
});

test("payments that are not credit sales are ignored", async () => {
  const w = world();
  await w.hook("payment_intent.succeeded", { id: "pi_d", amount_received: 500, metadata: { purchase_type: "donation" } });
  await w.hook("payment_intent.succeeded", { id: "pi_e", amount_received: 500, metadata: {} });
  assert.equal(await w.store.purchasedBalance("u1"), 0);
});

test("a refund takes credits back pro rata, once per amount, and a full refund finishes the job", async () => {
  const w = world();
  await w.hook("payment_intent.succeeded", paid());
  await w.hook("charge.refunded", { id: "ch_1", payment_intent: "pi_1", amount_refunded: 500 }); // a third of $15
  assert.equal(await w.store.purchasedBalance("u1"), 400 - 134, "a third of 400, rounded up for the house");
  await w.hook("charge.refunded", { id: "ch_1", payment_intent: "pi_1", amount_refunded: 500 }, { id: "evt_r2" });
  assert.equal(await w.store.purchasedBalance("u1"), 400 - 134, "a redelivery changes nothing");
  await w.hook("charge.refunded", { id: "ch_1", payment_intent: "pi_1", amount_refunded: 1500 }, { id: "evt_r3" });
  assert.equal(await w.store.purchasedBalance("u1"), 0);
});

test("a dispute after a refund takes back only what is left", async () => {
  const w = world();
  await w.hook("payment_intent.succeeded", paid());
  await w.hook("charge.refunded", { id: "ch_1", payment_intent: "pi_1", amount_refunded: 750 });
  await w.hook("charge.dispute.created", { id: "dp_1", payment_intent: "pi_1", amount: 1500 }, { id: "evt_d" });
  assert.equal(await w.store.purchasedBalance("u1"), 0, "never below zero from the same payment");
});

test("credits already spent make the balance negative, which blocks purchased spending", async () => {
  const w = world();
  await w.hook("payment_intent.succeeded", paid());
  await w.store.ledger("u1", -150, "grant", "spent-by-hand", NOW); // stands in for sessions that used 150
  await w.hook("charge.dispute.created", { id: "dp_2", payment_intent: "pi_1", amount: 1500 });
  assert.equal(await w.store.purchasedBalance("u1"), -150);
});

test("a refund of a payment that is not ours is acknowledged and ignored", async () => {
  const w = world();
  assert.equal((await w.hook("charge.refunded", { id: "ch_x", payment_intent: "pi_unknown", amount_refunded: 500 })).status, 200);
  assert.equal((await w.hook("some.other.event", {})).status, 200);
});

test("a database failure is not acknowledged, so Stripe redelivers", async () => {
  const w = world({ db: { prepare: () => { throw new Error("down"); } } as any });
  assert.equal((await w.hook("payment_intent.succeeded", paid())).status, 500);
});

test("deleting an account erases its credits, and a payment that arrives afterwards credits nothing", async () => {
  const w = world();
  await w.hook("payment_intent.succeeded", paid());
  await w.store.ledger("u2", 125, "purchase", "pi_other", NOW, 500);
  w.db.prepare(`INSERT INTO ai_sessions (id, principal, service, credits, calls_allowed, created_at, expires_at) VALUES ('s1', 'u1', 'figure', 2, 40, ?, ?)`).run(NOW, NOW + 1000);
  assert.equal(await w.store.purchasedBalance("u1"), 400);
  await d1(w.db).batch(deleteContentStatements(d1(w.db), "u1", NOW));
  assert.equal(await w.store.purchasedBalance("u1"), 0);
  assert.deepEqual(w.db.prepare(`SELECT id FROM ai_sessions WHERE principal = 'u1'`).all(), []);
  assert.equal(await w.store.purchasedBalance("u2"), 125, "other accounts keep theirs");
  // The account is tombstoned; Stripe delivers a payment that was already in flight.
  w.db.prepare(`INSERT INTO users (id, trust, created_at, deleted_at) VALUES ('u1', 'teacher', 1, ?)`).run(NOW);
  assert.equal((await w.hook("payment_intent.succeeded", paid("pi_late"), { id: "evt_late" })).status, 200);
  assert.equal(await w.store.purchasedBalance("u1"), 0);
});
