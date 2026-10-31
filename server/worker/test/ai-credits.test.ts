// Run: npm test. Weekly credits and service sessions on the AI relay, over real SQLite.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createAi, type AiBinding, type AiDeps } from "../src/ai/index.ts";
import { SERVICES, utcWeekStart } from "../src/ai/credits.ts";
import { AiStore, DAY_MS } from "../src/ai/store.ts";
import { d1, freshDb } from "./pool/harness.ts";

const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
const MON = Date.UTC(2026, 9, 26, 9, 0, 0); // Monday 2026-10-26

function world(o: Partial<AiDeps> & { fail?: boolean } = {}) {
  const db = freshDb();
  const clock = { t: MON };
  let modelCalls = 0;
  const ai: AiBinding = {
    run: async () => {
      modelCalls++;
      if (o.fail) throw new Error("down");
      return { choices: [{ message: { content: "{}" }, finish_reason: "stop" }], usage: { prompt_tokens: 10, completion_tokens: 2, total_tokens: 12, neurons: 1 } };
    },
  };
  const relay = createAi({
    ai,
    model: "@cf/test/model",
    allowedOrigins: [],
    authenticate: async (r) => (r.headers.get("x-user") ? { userId: r.headers.get("x-user")! } : null),
    db: d1(db),
    now: () => clock.t,
    relayKey: "k".repeat(32),
    limits: { weeklyCredits: 10 },
    ...o,
  });
  const send = async (path: string, body?: unknown, user = "u1", method = "POST", extra: Record<string, string> = {}) => {
    const text = body === undefined ? undefined : JSON.stringify(body);
    const res = await relay.handle(
      new Request("https://w.example" + path, {
        method,
        headers: { "content-type": "application/json", ...(text !== undefined ? { "Content-Length": String(new TextEncoder().encode(text).length) } : {}), ...(user ? { "x-user": user } : {}), ...extra },
        body: text,
      }),
    );
    return { status: res.status, body: (await res.json()) as any };
  };
  const chat = (session?: string, user = "u1") => send("/ai/chat/completions", { messages: [{ role: "user", content: "hi" }], ...(session ? { session } : {}) }, user);
  return { clock, send, chat, calls: () => modelCalls, store: new AiStore(d1(db)) };
}

test("a week starts on Monday 00:00 UTC", () => {
  assert.equal(utcWeekStart(MON), Date.UTC(2026, 9, 26));
  assert.equal(utcWeekStart(MON + 6 * DAY_MS + 10 * 3_600_000), Date.UTC(2026, 9, 26)); // Sunday evening
  assert.equal(utcWeekStart(MON + 7 * DAY_MS), Date.UTC(2026, 10, 2));
});

test("a signed-in account cannot chat without a service session", async () => {
  const w = world();
  const r = await w.chat();
  assert.equal(r.status, 402);
  assert.equal(r.body.error.code, "session_required");
  assert.equal(w.calls(), 0);
  assert.equal((await w.chat("0".repeat(32))).body.error.code, "session_exhausted");
});

test("starting a service charges the server's price and the balance shows it", async () => {
  const w = world();
  const s = await w.send("/ai/service", { service: "import", pages: 4, credits: 0 });
  assert.equal(s.status, 200);
  assert.equal(s.body.credits, 2, "the client cannot set the price");
  assert.equal(s.body.left, 8);
  const b = await w.send("/ai/credits", undefined, "u1", "GET");
  assert.deepEqual([b.body.allowance, b.body.used, b.body.left], [10, 2, 8]);
  assert.equal(b.body.resets_at, Date.UTC(2026, 10, 2));
  assert.equal(b.body.services.import.credits, 2);
  assert.equal((await w.chat(s.body.session)).status, 200);
});

test("handwriting is a credit per page and a page cap applies", async () => {
  const w = world();
  assert.equal((await w.send("/ai/service", { service: "handwriting", pages: 3 })).body.credits, 3);
  assert.equal((await w.send("/ai/service", { service: "import", pages: 21 })).body.error.code, "too_many_pages");
  assert.equal((await w.send("/ai/service", { service: "import" })).status, 400);
  assert.equal((await w.send("/ai/service", { service: "nope", pages: 1 })).status, 400);
  assert.equal((await w.send("/ai/service", { service: "constructor", pages: 1 })).status, 400);
});

test("credits run out, name what is needed, and come back next Monday", async () => {
  const w = world();
  for (let i = 0; i < 5; i++) assert.equal((await w.send("/ai/service", { service: "figure" })).status, 200); // 5 x 2 = 10
  const r = await w.send("/ai/service", { service: "figure" });
  assert.equal(r.status, 402);
  assert.equal(r.body.error.code, "insufficient_credits");
  assert.deepEqual([r.body.needed, r.body.left], [2, 0]);
  w.clock.t = MON + 7 * DAY_MS;
  assert.equal((await w.send("/ai/service", { service: "figure" })).status, 200);
});

test("credits are per account", async () => {
  const w = world();
  for (let i = 0; i < 5; i++) await w.send("/ai/service", { service: "figure" }, "u1");
  assert.equal((await w.send("/ai/service", { service: "figure" }, "u1")).status, 402);
  assert.equal((await w.send("/ai/service", { service: "figure" }, "u2")).status, 200);
});

test("a session belongs to its account", async () => {
  const w = world();
  const s = await w.send("/ai/service", { service: "figure" }, "u1");
  assert.equal((await w.chat(s.body.session, "u2")).status, 402);
  assert.equal((await w.chat(s.body.session, "u1")).status, 200);
});

test("a session's call budget and lifetime are enforced", async () => {
  const w = world();
  const s = await w.send("/ai/service", { service: "handwriting", pages: 1 }); // 4 calls
  for (let i = 0; i < 4; i++) assert.equal((await w.chat(s.body.session)).status, 200);
  assert.equal((await w.chat(s.body.session)).body.error.code, "session_exhausted");
  const e = await w.send("/ai/service", { service: "figure" });
  w.clock.t += SERVICES.figure.ttlMs + 1;
  assert.equal((await w.chat(e.body.session)).body.error.code, "session_exhausted");
});

test("a session that expires without one answered call is refunded", async () => {
  const w = world({ fail: true });
  const s = await w.send("/ai/service", { service: "import", pages: 2 });
  assert.equal((await w.chat(s.body.session)).status, 502);
  assert.equal((await w.send("/ai/credits", undefined, "u1", "GET")).body.left, 8, "still charged while open");
  w.clock.t += SERVICES.import.ttlMs + 1;
  assert.equal((await w.send("/ai/credits", undefined, "u1", "GET")).body.left, 10, "refunded after it lapsed unused");
});

test("the relay key and an anonymous native client are not on credits", async () => {
  const w = world();
  const key = { authorization: "Bearer " + "k".repeat(32) };
  const r = await w.send("/ai/chat/completions", { messages: [{ role: "user", content: "hi" }] }, "", "POST", key);
  assert.equal(r.status, 200);
  const s = await w.send("/ai/service", { service: "import", pages: 2 }, "", "POST", key);
  assert.equal(s.body.unlimited, true);
  assert.equal((await w.send("/ai/credits", undefined, "", "GET", key)).body.unlimited, true);
});

test("signed-in accounts cannot use the unpriced import job", async () => {
  const w = world();
  const r = await w.send("/ai/import", { pages: [{ mime: "image/png", b64: PNG }] });
  assert.equal(r.status, 403);
  assert.equal(r.body.error.code, "use_service_session");
});

// Purchased credits: the weekly allowance (10 here) pays first, the ledger covers the rest.
const burnWeek = async (w: ReturnType<typeof world>, user = "u1") => {
  for (let i = 0; i < 5; i++) assert.equal((await w.send("/ai/service", { service: "figure" }, user)).status, 200); // 5 x 2 = 10
};

test("purchased credits cover a service once the weekly allowance is used", async () => {
  const w = world();
  await burnWeek(w);
  assert.equal((await w.send("/ai/service", { service: "figure" })).status, 402);
  assert.equal(await w.store.ledger("u1", 5, "purchase", "cs_1", MON), true);
  const s = await w.send("/ai/service", { service: "figure" });
  assert.equal(s.status, 200);
  assert.deepEqual([s.body.credits, s.body.paid, s.body.left, s.body.purchased], [2, 2, 0, 3]);
  const b = await w.send("/ai/credits", undefined, "u1", "GET");
  assert.deepEqual([b.body.left, b.body.purchased], [0, 3]);
});

test("the allowance pays first and the balance only the shortfall", async () => {
  const w = world();
  for (let i = 0; i < 4; i++) await w.send("/ai/service", { service: "figure" }); // 8 of 10 used
  await w.store.ledger("u1", 5, "purchase", "cs_1", MON);
  const s = await w.send("/ai/service", { service: "handwriting", pages: 3 }); // 3 credits: 2 free + 1 paid
  assert.deepEqual([s.body.credits, s.body.paid, s.body.left, s.body.purchased], [3, 1, 0, 4]);
});

test("a replayed purchase credits once, and the balance is per account", async () => {
  const w = world();
  assert.equal(await w.store.ledger("u1", 5, "purchase", "cs_1", MON), true);
  assert.equal(await w.store.ledger("u1", 5, "purchase", "cs_1", MON), false);
  assert.equal(await w.store.purchasedBalance("u1"), 5);
  assert.equal(await w.store.purchasedBalance("u2"), 0);
});

test("an insufficient balance names both buckets and charges nothing", async () => {
  const w = world();
  await burnWeek(w);
  await w.store.ledger("u1", 1, "purchase", "cs_1", MON);
  const r = await w.send("/ai/service", { service: "figure" });
  assert.equal(r.status, 402);
  assert.deepEqual([r.body.needed, r.body.left, r.body.purchased], [2, 0, 1]);
  assert.equal(await w.store.purchasedBalance("u1"), 1);
});

test("racing sessions cannot overspend the balance", async () => {
  const w = world();
  await burnWeek(w);
  await w.store.ledger("u1", 4, "purchase", "cs_1", MON);
  const rs = await Promise.all(Array.from({ length: 6 }, () => w.send("/ai/service", { service: "figure" })));
  assert.equal(rs.filter((r) => r.status === 200).length, 2, "4 credits buy exactly two 2-credit sessions");
  assert.equal(await w.store.purchasedBalance("u1"), 0);
});

test("a refund or dispute that overdraws the balance blocks purchased spending, not the free week", async () => {
  const w = world();
  await burnWeek(w);
  await w.store.ledger("u1", 4, "purchase", "cs_1", MON);
  assert.equal((await w.send("/ai/service", { service: "figure" })).status, 200); // 2 spent, 2 left
  assert.equal(await w.store.ledger("u1", -4, "reversal", "re_1", MON), true);
  assert.equal(await w.store.purchasedBalance("u1"), -2);
  assert.equal((await w.send("/ai/service", { service: "figure" })).status, 402);
  assert.equal((await w.send("/ai/credits", undefined, "u1", "GET")).body.purchased, -2);
  w.clock.t = MON + 7 * DAY_MS;
  assert.equal((await w.send("/ai/service", { service: "figure" })).status, 200, "the new week's free credits still work");
});

test("an unused session gives the purchased part back, once", async () => {
  const w = world({ fail: true });
  await burnWeek(w);
  await w.store.ledger("u1", 2, "purchase", "cs_1", MON);
  const s = await w.send("/ai/service", { service: "figure" });
  assert.equal((await w.chat(s.body.session)).status, 502);
  assert.equal(await w.store.purchasedBalance("u1"), 0, "spent while open");
  w.clock.t += SERVICES.figure.ttlMs + 1;
  assert.equal((await w.send("/ai/credits", undefined, "u1", "GET")).body.purchased, 2);
  assert.equal((await w.send("/ai/credits", undefined, "u1", "GET")).body.purchased, 2, "a second look does not refund twice");
});
