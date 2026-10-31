// Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { AI_PATH, createAi, type AiDeps } from "../src/ai/index.ts";
import { viaRun, type Run } from "./ai-fake.ts";

const RELAY = "https://worker.example" + AI_PATH;

const OK_REPLY = {
  id: "r1",
  object: "chat.completion",
  choices: [{ index: 0, message: { role: "assistant", content: '{"ok":true}', reasoning: "hidden chain of thought" }, finish_reason: "stop" }],
  usage: { prompt_tokens: 10, completion_tokens: 20, total_tokens: 30, cost: 0.00011 },
};

function harness(overrides: Partial<AiDeps> = {}, run: Run = async () => OK_REPLY) {
  const captured: { model: string; inputs: Record<string, unknown> }[] = [];
  const deps: AiDeps = {
    openrouter: viaRun(run, captured),
    allowedOrigins: ["https://app.example.org"],
    authenticate: async () => ({ userId: "u1" }),
    ...overrides,
  };
  return { relay: createAi(deps), captured };
}

const post = (body: unknown, headers: Record<string, string> = {}): Request => {
  const text = typeof body === "string" ? body : JSON.stringify(body);
  return new Request(RELAY, {
    method: "POST",
    // readJson (the pool's guarded reader) requires an exact Content-Length, as every real client sends.
    headers: { "content-type": "application/json", "Content-Length": String(new TextEncoder().encode(text).length), ...headers },
    body: text,
  });
};

const say = (content = "hi") => ({ messages: [{ role: "user", content }] });

test("a valid request returns the upstream content with reasoning and cost stripped", async () => {
  const { relay, captured } = harness();
  const res = await relay.handle(post(say()));
  assert.equal(res!.status, 200);
  const body = (await res!.json()) as any;
  assert.equal(body.choices[0].message.content, '{"ok":true}');
  assert.equal(body.choices[0].message.reasoning, undefined);
  assert.equal(body.usage.cost, undefined);
  assert.deepEqual({ p: body.usage.prompt_tokens, c: body.usage.completion_tokens, t: body.usage.total_tokens }, { p: 10, c: 20, t: 30 });
  // The relay always asks for one JSON object: the figure loop depends on it (spec §2).
  assert.deepEqual(captured[0]!.inputs.response_format, { type: "json_object" });
});

test("the call goes to OpenRouter with the key, reasoning off, cost reporting and a no-retention provider filter", async () => {
  const calls: { url: string; headers: Record<string, string>; body: any }[] = [];
  const stub = (async (url: string, init: RequestInit) => {
    calls.push({ url, headers: init.headers as Record<string, string>, body: JSON.parse(init.body as string) });
    return new Response(JSON.stringify(OK_REPLY), { status: 200 });
  }) as unknown as typeof fetch;
  const { relay } = harness({ openrouter: { key: "or-secret-key", model: "google/test-model", fetch: stub } });
  assert.equal((await relay.handle(post(say())))!.status, 200);
  assert.equal(calls.length, 1);
  assert.equal(calls[0]!.url, "https://openrouter.ai/api/v1/chat/completions");
  assert.equal(calls[0]!.headers.authorization, "Bearer or-secret-key");
  assert.equal(calls[0]!.body.model, "google/test-model");
  assert.deepEqual(calls[0]!.body.reasoning, { enabled: false });
  assert.deepEqual(calls[0]!.body.usage, { include: true });
  assert.deepEqual(calls[0]!.body.provider, { data_collection: "deny" });
});

test("a client cannot turn reasoning on or pass provider options or model parameters", async () => {
  const { relay, captured } = harness();
  await relay.handle(post({ ...say(), thinking: true, reasoning: { enabled: true }, reasoning_effort: "high", provider: { order: ["x"] }, chat_template_kwargs: { enable_thinking: true } }));
  const sent = captured[0]!.inputs as any;
  assert.deepEqual(sent.reasoning, { enabled: false });
  assert.deepEqual(sent.provider, { data_collection: "deny" });
  assert.equal(sent.reasoning_effort, undefined);
  assert.equal(sent.thinking, undefined);
  assert.equal(sent.chat_template_kwargs, undefined);
});

test("the client cannot choose the model", async () => {
  const { relay, captured } = harness();
  await relay.handle(post({ ...say(), model: "evil/expensive-model" }));
  assert.equal(captured[0]!.model, "or/test-model");
});

test("privacy filtering is on by default and only the deployment can relax it", async () => {
  const strict = harness();
  await strict.relay.handle(post(say()));
  assert.deepEqual((strict.captured[0]!.inputs as any).provider, { data_collection: "deny" });
  const captured: { model: string; inputs: Record<string, unknown> }[] = [];
  const loose = harness({ openrouter: { ...viaRun(async () => OK_REPLY, captured), privacy: false } });
  await loose.relay.handle(post(say()));
  assert.deepEqual((captured[0]!.inputs as any).provider, { data_collection: "allow" });
});

test("max_tokens is clamped and temperature is bounded", async () => {
  const { relay, captured } = harness({ limits: { maxOutTokens: 64 } });
  await relay.handle(post({ ...say(), max_tokens: 999_999, temperature: 7 }));
  assert.equal(captured[0]!.inputs.max_tokens, 64);
  assert.equal(captured[0]!.inputs.temperature, 1);
});

test("images are accepted as png data URIs and passed through", async () => {
  const { relay, captured } = harness();
  const url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
  const res = await relay.handle(post({ messages: [{ role: "user", content: [{ type: "text", text: "read it" }, { type: "image_url", image_url: { url } }] }] }));
  assert.equal(res!.status, 200);
  const sent = captured[0]!.inputs.messages as any[];
  assert.equal(sent[0].content[1].image_url.url, url);
});

test("an oversized image is refused before the model is called", async () => {
  const { relay, captured } = harness({ limits: { maxImageBytes: 8 } });
  const res = await relay.handle(post({ messages: [{ role: "user", content: [{ type: "image_url", image_url: { url: "data:image/png;base64,iVBORw0KGgo=" } }] }] }));
  assert.equal(res!.status, 400);
  assert.equal(captured.length, 0);
});

test("a remote image URL is refused: only data URIs are relayed", async () => {
  const { relay } = harness();
  const res = await relay.handle(post({ messages: [{ role: "user", content: [{ type: "image_url", image_url: { url: "https://evil.example/x.png" } }] }] }));
  assert.equal(res!.status, 400);
  assert.match(((await res!.json()) as any).error.message, /data URI/);
});

test("no principal is 401 when the relay is keyed", async () => {
  const { relay } = harness({ authenticate: async () => null, relayKey: "k".repeat(32) });
  const res = await relay.handle(post(say()));
  assert.equal(res!.status, 401);
  assert.equal(((await res!.json()) as any).error.code, "unauthenticated");
});

test("no OpenRouter key is 503 ai_not_configured", async () => {
  const { relay } = harness({ openrouter: undefined });
  const res = await relay.handle(post(say()));
  assert.equal(res!.status, 503);
  assert.equal(((await res!.json()) as any).error.code, "ai_not_configured");
});

test("an empty or malformed body is 400", async () => {
  const { relay, captured } = harness();
  assert.equal((await relay.handle(post({ messages: [] })))!.status, 400);
  assert.equal((await relay.handle(post("not json")))!.status, 400);
  assert.equal((await relay.handle(post({ messages: [{ role: "tool", content: "x" }] })))!.status, 400);
  assert.equal(captured.length, 0);
});

test("an upstream failure is 502 and never echoes the upstream message or the key", async () => {
  const { relay } = harness({}, async () => {
    throw new Error("401 from provider: Bearer or-test-key and prompt text");
  });
  const res = await relay.handle(post(say()));
  assert.equal(res!.status, 502);
  const text = await res!.text();
  assert.doesNotMatch(text, /or-test-key|prompt text/);
  assert.equal(JSON.parse(text).error.code, "model_failed");
});

test("an upstream error status is tried twice and then a 502 that never shows the key or upstream text", async () => {
  let calls = 0;
  const stub = (async () => (calls++, new Response("upstream trouble with or-secret-key", { status: 500 }))) as unknown as typeof fetch;
  const orig = console.error;
  console.error = () => {};
  try {
    const { relay } = harness({ openrouter: { key: "or-secret-key", model: "m", fetch: stub } });
    const res = await relay.handle(post(say()));
    assert.equal(res!.status, 502);
    assert.equal(calls, 2);
    const text = await res!.text();
    assert.ok(!text.includes("or-secret-key") && !text.includes("trouble"));
  } finally {
    console.error = orig;
  }
});

test("one failed upstream call is retried once and then succeeds, logging only the error class", async () => {
  let calls = 0;
  const errs: string[] = [];
  const orig = console.error;
  console.error = (m: string) => void errs.push(String(m));
  try {
    const { relay } = harness({}, async () => {
      calls += 1;
      if (calls === 1) throw new Error("capacity: Bearer or-test-key prompt text");
      return { choices: [{ message: { content: '{"ok":true}' }, finish_reason: "stop" }] };
    });
    const res = await relay.handle(post(say()));
    assert.equal(res!.status, 200);
    assert.equal(calls, 2);
  } finally {
    console.error = orig;
  }
  assert.ok(errs.some((m) => /openrouter call failed \(Error\)/.test(m)), "the class is logged");
  assert.ok(errs.every((m) => !/or-test-key|prompt text/.test(m)), "the message is never logged");
});

test("an empty completion is retried once, and two of them are a 502", async () => {
  let calls = 0;
  const orig = console.error;
  console.error = () => {};
  try {
    const first = harness({}, async () => (calls++ === 0 ? { choices: [{ message: { content: "  " } }] } : OK_REPLY));
    assert.equal((await first.relay.handle(post(say())))!.status, 200);
    assert.equal(calls, 2);
    const none = harness({}, async () => ({ choices: [] }));
    assert.equal((await none.relay.handle(post(say())))!.status, 502);
    assert.equal(none.captured.length, 2);
  } finally {
    console.error = orig;
  }
});

test("a thrown or timed-out call is retried", async () => {
  let calls = 0;
  const boom = (async () => {
    if (calls++ === 0) throw new DOMException("timed out", "TimeoutError");
    return new Response(JSON.stringify(OK_REPLY), { status: 200 });
  }) as unknown as typeof fetch;
  const orig = console.error;
  console.error = () => {};
  try {
    const { relay } = harness({ openrouter: { key: "k", model: "m", fetch: boom } });
    assert.equal((await relay.handle(post(say())))!.status, 200);
    assert.equal(calls, 2);
  } finally {
    console.error = orig;
  }
});

test("a foreign origin is 403 and a GET is 405", async () => {
  const { relay } = harness();
  assert.equal((await relay.handle(post(say(), { Origin: "https://evil.example" })))!.status, 403);
  const get = new Request(RELAY, { method: "GET" });
  assert.equal((await relay.handle(get))!.status, 405);
});

test("other paths are not ours", async () => {
  const { relay } = harness();
  assert.equal(await relay.handle(new Request("https://worker.example/items")), null);
});

test("the relay key admits the desktop app; a wrong key does not", async () => {
  const key = "k".repeat(32);
  const { relay } = harness({ authenticate: async () => null, relayKey: key });
  const ok = await relay.handle(post(say(), { Authorization: `Bearer ${key}` }));
  assert.equal(ok!.status, 200);
  const short = await relay.handle(post(say(), { Authorization: `Bearer ${key.slice(1)}` }));
  assert.equal(short!.status, 401);
  const none = await relay.handle(post(say()));
  assert.equal(none!.status, 401);
});

test("with no relay key, a native client may call but a browser may not", async () => {
  const { relay } = harness({ authenticate: async () => null });
  // No Origin: the desktop app. Allowed while the deployment is unreachable.
  assert.equal((await relay.handle(post(say())))!.status, 200);
  // A browser always sends Origin; even an allowed one is refused without a key or a session.
  const browser = await relay.handle(post(say(), { Origin: "https://app.example.org" }));
  assert.equal(browser!.status, 401);
});

test("a configured relay key is required once set", async () => {
  const key = "k".repeat(32);
  const { relay } = harness({ authenticate: async () => null, relayKey: key });
  assert.equal((await relay.handle(post(say())))!.status, 401, "no token, no entry");
  assert.equal((await relay.handle(post(say(), { Authorization: `Bearer ${key}` })))!.status, 200);
});

test("health is unauthenticated and reports the OpenRouter model and whether a key is configured", async () => {
  const { relay } = harness({ authenticate: async () => null });
  const res = await relay.handle(new Request("https://worker.example/ai/health"));
  assert.equal(res!.status, 200);
  assert.deepEqual(await res!.json(), { ok: true, model: "or/test-model", ai: true, upstream: "openrouter" });
  const { relay: bare } = harness({ openrouter: undefined, authenticate: async () => null });
  const off = await bare.handle(new Request("https://worker.example/ai/health"));
  assert.deepEqual(await off!.json(), { ok: true, model: null, ai: false, upstream: "openrouter" });
});
