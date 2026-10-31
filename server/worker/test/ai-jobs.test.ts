// Run: npm test. The relay's quota and import jobs, over real SQLite (the pool harness's D1 shim).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { AI_IMPORT_PATH, AI_PATH, createAi, type AiDeps } from "../src/ai/index.ts";
import { reply, viaRun, type Run } from "./ai-fake.ts";
import { DITTO_PROMPT } from "../src/ai/prompt.ts";
import { pagePrompt, pruneAi, STALE_MS } from "../src/ai/jobs.ts";
import { DAY_MS, HOUR_MS, utcDayStart } from "../src/ai/store.ts";
import { d1, freshDb, MemR2 } from "./pool/harness.ts";

const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
const T0 = Date.UTC(2026, 9, 30, 12, 0, 0);

function world(o: Partial<AiDeps> & { cost?: number; run?: Run } = {}) {
  const db = freshDb();
  const bucket = new MemR2();
  const clock = { t: T0 };
  const calls: { inputs: Record<string, unknown> }[] = [];
  const openrouter = viaRun(
    o.run ??
      (async (_m, inputs) => {
        calls.push({ inputs });
        return reply('{"format":"queshion/ditto"}', o.cost ?? 0.002);
      }),
  );
  const pending: Promise<unknown>[] = [];
  const relay = createAi({
    model: "@cf/test/model",
    openrouter,
    allowedOrigins: [],
    authenticate: async (r) => {
      const u = r.headers.get("x-user");
      return u ? { userId: u } : null;
    },
    db: d1(db),
    bucket: bucket as unknown as R2Bucket,
    now: () => clock.t,
    credits: false,
    ...o,
  });
  const ctx = { waitUntil: (p: Promise<unknown>) => void pending.push(p) };
  const send = (path: string, init: { method?: string; user?: string; body?: unknown } = {}) => {
    const text = init.body === undefined ? undefined : JSON.stringify(init.body);
    return relay.handle(
      new Request("https://w.example" + path, {
        method: init.method ?? "POST",
        headers: {
          "content-type": "application/json",
          ...(text !== undefined ? { "Content-Length": String(new TextEncoder().encode(text).length) } : {}),
          ...(init.user ? { "x-user": init.user } : {}),
        },
        body: text,
      }),
      ctx,
    );
  };
  const settle = async () => void (await Promise.all(pending.splice(0)));
  return { db, bucket, clock, calls, relay, send, settle };
}

const chat = { messages: [{ role: "user", content: "hi" }] };
const page = { mime: "image/png", b64: PNG };

// ---- quota -----------------------------------------------------------------------------------

test("each answered chat turn is recorded with its tokens and what OpenRouter billed, and nothing else", async () => {
  const w = world();
  assert.equal((await w.send(AI_PATH, { user: "u1", body: chat }))!.status, 200);
  const rows = w.db.prepare("SELECT * FROM ai_usage").all() as Record<string, unknown>[];
  assert.equal(rows.length, 1);
  assert.deepEqual(
    { p: rows[0]!.principal, k: rows[0]!.kind, i: rows[0]!.in_tokens, o: rows[0]!.out_tokens, usd: rows[0]!.cost_usd, n: rows[0]!.neurons, s: rows[0]!.session, at: rows[0]!.at },
    { p: "u1", k: "chat", i: 100, o: 20, usd: 0.002, n: 0, s: null, at: T0 },
  );
  assert.deepEqual(Object.keys(rows[0]!).sort(), ["at", "cost_usd", "id", "in_tokens", "kind", "neurons", "out_tokens", "principal", "session"]);
});

test("a turn answered by the Workers AI fallback is booked in dollars at $0.000011 a neuron", async () => {
  const boom = (async () => new Response("x", { status: 500 })) as unknown as typeof fetch;
  const orig = console.error;
  console.error = () => {};
  try {
    const w = world({
      openrouter: { key: "k", model: "m", fetch: boom },
      ai: { run: async () => ({ choices: [{ message: { content: "{}" } }], usage: { prompt_tokens: 3, completion_tokens: 4, neurons: 1000 } }) },
    });
    assert.equal((await w.send(AI_PATH, { user: "u1", body: chat }))!.status, 200);
    const row = w.db.prepare("SELECT cost_usd, neurons FROM ai_usage").get() as any;
    assert.deepEqual({ usd: Number(row.cost_usd.toFixed(6)), n: row.neurons }, { usd: 0.011, n: 0 });
  } finally {
    console.error = orig;
  }
});

test("a principal over the hourly turn cap gets 429 rate_limited, without a model call, and the hour rolls", async () => {
  const w = world({ limits: { chatPerHour: 2 } });
  for (let i = 0; i < 2; i++) assert.equal((await w.send(AI_PATH, { user: "u1", body: chat }))!.status, 200);
  const before = w.calls.length;
  const res = (await w.send(AI_PATH, { user: "u1", body: chat }))!;
  assert.equal(res.status, 429);
  assert.equal(((await res.json()) as any).error.code, "rate_limited");
  assert.ok(res.headers.get("Retry-After"));
  assert.equal(w.calls.length, before, "no model call is spent on a refused turn");
  assert.equal((await w.send(AI_PATH, { user: "u2", body: chat }))!.status, 200, "another principal is not affected");
  w.clock.t += HOUR_MS + 1;
  assert.equal((await w.send(AI_PATH, { user: "u1", body: chat }))!.status, 200, "an hour later the window has moved");
});

test("the account's daily dollar cap is 429 quota_exhausted for everyone until UTC midnight", async () => {
  const w = world({ cost: 0.4, limits: { dailyUsd: 0.5 } });
  assert.equal((await w.send(AI_PATH, { user: "u1", body: chat }))!.status, 200); // $0.40
  assert.equal((await w.send(AI_PATH, { user: "u2", body: chat }))!.status, 200); // $0.80, over the cap now
  const res = (await w.send(AI_PATH, { user: "u3", body: chat }))!;
  assert.equal(res.status, 429);
  assert.equal(((await res.json()) as any).error.code, "quota_exhausted");
  const wait = Number(res.headers.get("Retry-After"));
  assert.equal(wait, Math.ceil((utcDayStart(T0) + DAY_MS - T0) / 1000));
  w.clock.t = utcDayStart(T0) + DAY_MS + 1;
  assert.equal((await w.send(AI_PATH, { user: "u3", body: chat }))!.status, 200, "the next UTC day starts clean");
});

test("a bad request is refused before it is counted, and a failed model call records nothing", async () => {
  const w = world({ run: async () => { throw new Error("down"); } });
  const orig = console.error;
  console.error = () => {};
  try {
    assert.equal((await w.send(AI_PATH, { user: "u1", body: { messages: [] } }))!.status, 400);
    assert.equal((await w.send(AI_PATH, { user: "u1", body: chat }))!.status, 502);
  } finally {
    console.error = orig;
  }
  assert.equal((w.db.prepare("SELECT COUNT(*) AS n FROM ai_usage").get() as any).n, 0);
});

// ---- jobs ------------------------------------------------------------------------------------

async function start(w: ReturnType<typeof world>, body: unknown, user = "u1") {
  const res = (await w.send(AI_IMPORT_PATH, { user, body }))!;
  return { res, id: res.status === 202 ? (((await res.json()) as any).job_id as string) : "" };
}
const read = async (w: ReturnType<typeof world>, id: string, user = "u1") => (await w.send(`${AI_IMPORT_PATH}/${id}`, { method: "GET", user }))!;

test("a job runs its pages in order and returns each page's text", async () => {
  const w = world();
  const { res, id } = await start(w, { hint: "Quiz 3", pages: [page, page, page] });
  assert.equal(res.status, 202);
  assert.match(id, /^[0-9a-f]{32}$/);
  await w.settle();
  const got = (await (await read(w, id)).json()) as any;
  assert.equal(got.status, "done");
  assert.deepEqual({ done: got.pages_done, total: got.pages_total, error: got.error }, { done: 3, total: 3, error: null });
  assert.deepEqual(got.result.pages.map((p: any) => p.page), [1, 2, 3]);
  assert.ok(got.result.pages.every((p: any) => p.text === '{"format":"queshion/ditto"}'));
  assert.equal(w.calls.length, 3);
  // Each page's prompt is the app's, with the user's note, and the model is not asked to think.
  const content = (w.calls[0]!.inputs.messages as any[])[0].content;
  assert.equal(content[0].text, pagePrompt("Quiz 3"));
  assert.equal(content[1].image_url.url, `data:image/png;base64,${PNG}`);
  assert.deepEqual(w.calls[0]!.inputs.reasoning, { enabled: false });
  assert.equal((w.db.prepare("SELECT COUNT(*) AS n FROM ai_usage WHERE kind = 'import_page'").get() as any).n, 3);
});

test("the prompt the worker sends is the file the app embeds", () => {
  assert.equal(DITTO_PROMPT, readFileSync(new URL("../../../prompts/ditto-convert.md", import.meta.url), "utf8"));
  assert.equal(pagePrompt("  "), DITTO_PROMPT);
});

test("a failing page is reported on that page and the job still finishes with the others", async () => {
  let n = 0;
  const w = world({
    run: async () => {
      n += 1;
      if (n === 2 || n === 3) throw new Error("secret provider text"); // page 2 fails both tries
      return reply("{}", 0.0001);
    },
  });
  const orig = console.error;
  console.error = () => {};
  try {
    const { id } = await start(w, { pages: [page, page, page] });
    await w.settle();
    const got = (await (await read(w, id)).json()) as any;
    assert.equal(got.status, "done");
    assert.deepEqual(got.result.pages.map((p: any) => p.text !== undefined), [true, false, true]);
    assert.equal(got.result.pages[1].error, "model_failed");
    assert.doesNotMatch(JSON.stringify(got), /secret provider text/);
  } finally {
    console.error = orig;
  }
});

test("a job where no page can be read is failed, with a fixed sentence", async () => {
  const w = world({ run: async () => ({ choices: [] }) });
  const orig = console.error;
  console.error = () => {};
  try {
    const { id } = await start(w, { pages: [page] });
    await w.settle();
    const got = (await (await read(w, id)).json()) as any;
    assert.deepEqual({ s: got.status, e: got.error }, { s: "failed", e: "No page could be read." });
  } finally {
    console.error = orig;
  }
});

test("a job is private to the principal that made it", async () => {
  const w = world();
  const { id } = await start(w, { pages: [page] });
  await w.settle();
  assert.equal((await read(w, id, "u1")).status, 200);
  assert.equal((await read(w, id, "u2")).status, 404, "another user sees the same 404 as a missing job");
  assert.equal((await read(w, "0".repeat(32), "u1")).status, 404);
  assert.equal((await read(w, "not-an-id", "u1")).status, 404);
});

test("job input is validated before anything is stored", async () => {
  const w = world({ limits: { maxPages: 2, maxImageBytes: 100 } });
  const bad: [string, unknown][] = [
    ["no pages", { pages: [] }],
    ["too many pages", { pages: [page, page, page] }],
    ["not an image type", { pages: [{ mime: "application/pdf", b64: PNG }] }],
    ["not base64", { pages: [{ mime: "image/png", b64: "not base64 !!" }] }],
    ["a page that is not an object", { pages: [null] }],
    ["an oversized image", { pages: [{ mime: "image/png", b64: "A".repeat(400) }] }],
    ["a long hint", { hint: "x".repeat(501), pages: [page] }],
  ];
  for (const [name, body] of bad) assert.equal((await w.send(AI_IMPORT_PATH, { user: "u1", body }))!.status, 400, name);
  assert.equal((w.db.prepare("SELECT COUNT(*) AS n FROM ai_jobs").get() as any).n, 0);
  assert.equal(w.calls.length, 0);
});

test("a principal over the daily import cap gets 429 and no job is made", async () => {
  const w = world({ limits: { importPerDay: 2 } });
  for (let i = 0; i < 2; i++) assert.equal((await start(w, { pages: [page] })).res.status, 202);
  const over = await start(w, { pages: [page] });
  assert.equal(over.res.status, 429);
  assert.equal(((await over.res.json()) as any).error.code, "rate_limited");
  assert.equal((w.db.prepare("SELECT COUNT(*) AS n FROM ai_jobs").get() as any).n, 2);
  assert.equal((await start(w, { pages: [page] }, "u2")).res.status, 202);
});

test("when the daily dollar cap is reached mid-job, the remaining pages say quota_exhausted", async () => {
  const w = world({ cost: 0.3, limits: { dailyUsd: 0.5 } });
  const { id } = await start(w, { pages: [page, page, page] });
  await w.settle();
  const got = (await (await read(w, id)).json()) as any;
  assert.deepEqual(got.result.pages.map((p: any) => p.error ?? "ok"), ["ok", "ok", "quota_exhausted"]);
});

test("a job that stops moving is reported failed once it is stale", async () => {
  const w = world();
  const { id } = await start(w, { pages: [page] });
  await w.settle();
  w.db.prepare("UPDATE ai_jobs SET status = 'running', updated_at = ?1 WHERE id = ?2").run(T0, id);
  assert.equal(((await (await read(w, id)).json()) as any).status, "running");
  w.clock.t += STALE_MS + 1;
  const got = (await (await read(w, id)).json()) as any;
  assert.deepEqual({ s: got.status, e: got.error }, { s: "failed", e: "The job was interrupted. Start it again." });
});

test("a result above 256 KB is kept in R2 and still returned whole", async () => {
  const big = "x".repeat(300 * 1024);
  const w = world({ run: async () => reply(big, 0.0001) });
  const { id } = await start(w, { pages: [page] });
  await w.settle();
  assert.deepEqual(w.bucket.keys("ai/jobs/"), [`ai/jobs/${id}/result.json`]);
  assert.equal((w.db.prepare("SELECT result FROM ai_jobs WHERE id = ?1").get(id) as any).result, null);
  const got = (await (await read(w, id)).json()) as any;
  assert.equal(got.status, "done");
  assert.equal(got.result.pages[0].text.length, big.length);
});

test("the hourly prune drops expired jobs, their R2 results, and usage past 90 days", async () => {
  const w = world({ run: async () => reply("x".repeat(300 * 1024), 0.0001) });
  const { id } = await start(w, { pages: [page] });
  await w.settle();
  const db = d1(w.db);
  assert.equal(await pruneAi(db, w.bucket as unknown as R2Bucket, T0 + 71 * HOUR_MS, 72 * HOUR_MS), 0, "inside the ttl nothing goes");
  assert.equal(await pruneAi(db, w.bucket as unknown as R2Bucket, T0 + 73 * HOUR_MS, 72 * HOUR_MS), 1);
  assert.equal((w.db.prepare("SELECT COUNT(*) AS n FROM ai_jobs").get() as any).n, 0);
  assert.deepEqual(w.bucket.keys("ai/jobs/"), []);
  assert.ok(id);
  assert.equal((w.db.prepare("SELECT COUNT(*) AS n FROM ai_usage").get() as any).n, 1, "usage is kept for 90 days");
  await pruneAi(db, w.bucket as unknown as R2Bucket, T0 + 91 * DAY_MS, 72 * HOUR_MS);
  assert.equal((w.db.prepare("SELECT COUNT(*) AS n FROM ai_usage").get() as any).n, 0);
});

test("job routes need a database, a caller and the right method", async () => {
  const w = world({ db: undefined });
  assert.equal((await w.send(AI_IMPORT_PATH, { user: "u1", body: { pages: [page] } }))!.status, 503, "no database");
  const k = world({ authenticate: async () => null, relayKey: "k".repeat(32) });
  assert.equal((await k.send(AI_IMPORT_PATH, { body: { pages: [page] } }))!.status, 401);
  assert.equal((await k.send(`${AI_IMPORT_PATH}/${"a".repeat(32)}`, { method: "GET" }))!.status, 401);
  const m = world();
  assert.equal((await m.send(AI_IMPORT_PATH, { method: "GET", user: "u1" }))!.status, 405);
  assert.equal((await m.send(`${AI_IMPORT_PATH}/${"a".repeat(32)}`, { method: "POST", user: "u1", body: {} }))!.status, 405);
});

test("a quota that cannot be read fails closed with 503 and spends no model call", async () => {
  const w = world();
  w.db.exec("DROP TABLE ai_usage");
  const orig = console.error;
  console.error = () => {};
  try {
    const res = (await w.send(AI_PATH, { user: "u1", body: chat }))!;
    assert.equal(res.status, 503);
    assert.equal(((await res.json()) as any).error.code, "quota_unavailable");
    assert.equal(w.calls.length, 0);
    assert.equal((await start(w, { pages: [page] })).res.status, 503);
  } finally {
    console.error = orig;
  }
});
