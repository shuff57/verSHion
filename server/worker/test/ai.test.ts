// Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { AI_PATH, createAi, type AiBinding, type AiDeps } from "../src/ai/index.ts";

const RELAY = "https://worker.example" + AI_PATH;

function harness(overrides: Partial<AiDeps> = {}) {
  const captured: { model: string; inputs: Record<string, unknown> }[] = [];
  const ai: AiBinding = {
    run: async (model, inputs) => {
      captured.push({ model, inputs });
      return {
        id: "r1",
        object: "chat.completion",
        model: `${model}-external`,
        choices: [
          {
            index: 0,
            message: { role: "assistant", content: '{"ok":true}', reasoning_content: "hidden chain of thought" },
            finish_reason: "stop",
          },
        ],
        usage: { prompt_tokens: 10, completion_tokens: 20, total_tokens: 30, neurons: 1.5 },
      };
    },
  };
  const deps: AiDeps = {
    ai,
    model: "@cf/test/model",
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

test("a valid request returns the upstream content with reasoning and neurons stripped", async () => {
  const { relay, captured } = harness();
  const res = await relay.handle(post(say()));
  assert.equal(res!.status, 200);
  const body = (await res!.json()) as any;
  assert.equal(body.choices[0].message.content, '{"ok":true}');
  assert.equal(body.choices[0].message.reasoning_content, undefined);
  assert.equal(body.usage.neurons, undefined);
  assert.deepEqual(
    { p: body.usage.prompt_tokens, c: body.usage.completion_tokens, t: body.usage.total_tokens },
    { p: 10, c: 20, t: 30 },
  );
  // The relay always asks for one JSON object: the figure loop depends on it (spec §2).
  assert.deepEqual(captured[0]!.inputs.response_format, { type: "json_object" });
});

test("thinking mode is off by default and on only when the deployment asks", async () => {
  const off = harness();
  await off.relay.handle(post(say()));
  assert.deepEqual(off.captured[0]!.inputs.chat_template_kwargs, { enable_thinking: false });
  const on = harness({ thinking: true });
  await on.relay.handle(post(say()));
  assert.deepEqual(on.captured[0]!.inputs.chat_template_kwargs, { enable_thinking: true });
});

test("a client cannot turn thinking on or pass template arguments or model parameters", async () => {
  const { relay, captured } = harness();
  await relay.handle(post({ ...say(), thinking: true, chat_template_kwargs: { enable_thinking: true, other: 1 }, reasoning_effort: "high" }));
  assert.deepEqual(captured[0]!.inputs.chat_template_kwargs, { enable_thinking: false });
  assert.equal(captured[0]!.inputs.reasoning_effort, undefined);
});

test("the client cannot choose the model", async () => {
  const { relay, captured } = harness();
  await relay.handle(post({ ...say(), model: "@cf/an/expensive/model" }));
  assert.equal(captured[0]!.model, "@cf/test/model");
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
  const res = await relay.handle(
    post({ messages: [{ role: "user", content: [{ type: "text", text: "read it" }, { type: "image_url", image_url: { url } }] }] }),
  );
  assert.equal(res!.status, 200);
  const sent = captured[0]!.inputs.messages as any[];
  assert.equal(sent[0].content[1].image_url.url, url);
});

test("an oversized image is refused before the model is called", async () => {
  const { relay, captured } = harness({ limits: { maxImageBytes: 8 } });
  const res = await relay.handle(
    post({
      messages: [
        { role: "user", content: [{ type: "image_url", image_url: { url: "data:image/png;base64,iVBORw0KGgo=" } }] },
      ],
    }),
  );
  assert.equal(res!.status, 400);
  assert.equal(captured.length, 0);
});

test("a remote image URL is refused: only data URIs are relayed", async () => {
  const { relay } = harness();
  const res = await relay.handle(
    post({ messages: [{ role: "user", content: [{ type: "image_url", image_url: { url: "https://evil.example/x.png" } }] }] }),
  );
  assert.equal(res!.status, 400);
  assert.match(((await res!.json()) as any).error.message, /data URI/);
});

test("no principal is 401 when the relay is keyed", async () => {
  const { relay } = harness({ authenticate: async () => null, relayKey: "k".repeat(32) });
  const res = await relay.handle(post(say()));
  assert.equal(res!.status, 401);
  assert.equal(((await res!.json()) as any).error.code, "unauthenticated");
});

test("no AI binding is 503 ai_not_configured", async () => {
  const { relay } = harness({ ai: undefined });
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

test("an upstream failure is 502 and never echoes the upstream message", async () => {
  const { relay } = harness({
    ai: {
      run: async () => {
        throw new Error("401 from provider: Bearer sk-live-SECRET and prompt text");
      },
    },
  });
  const res = await relay.handle(post(say()));
  assert.equal(res!.status, 502);
  const text = await res!.text();
  assert.doesNotMatch(text, /sk-live-SECRET|prompt text/);
  assert.equal(JSON.parse(text).error.code, "model_failed");
});

test("one failed upstream call is retried once and then succeeds", async () => {
  let calls = 0;
  const errs: string[] = [];
  const orig = console.error;
  console.error = (m: string) => void errs.push(String(m));
  try {
    const { relay } = harness({
      ai: {
        run: async () => {
          calls += 1;
          if (calls === 1) throw new Error("capacity: Bearer sk-live-SECRET prompt text");
          return { choices: [{ message: { content: '{"ok":true}' }, finish_reason: "stop" }] };
        },
      },
    });
    const res = await relay.handle(post(say()));
    assert.equal(res!.status, 200);
    assert.equal(calls, 2);
  } finally {
    console.error = orig;
  }
  assert.ok(errs.some((m) => /upstream call failed \(Error\), attempt 1/.test(m)), "the class is logged");
  assert.ok(errs.every((m) => !/SECRET|prompt text/.test(m)), "the message is never logged");
});

test("two failed upstream calls are a 502 and call the model exactly twice", async () => {
  let calls = 0;
  const orig = console.error;
  console.error = () => {};
  try {
    const { relay } = harness({ ai: { run: async () => (calls++, Promise.reject(new Error("down"))) } });
    assert.equal((await relay.handle(post(say())))!.status, 502);
    assert.equal(calls, 2);
  } finally {
    console.error = orig;
  }
});

test("a reply without text is 502", async () => {
  const { relay } = harness({ ai: { run: async () => ({ choices: [] }) } });
  assert.equal((await relay.handle(post(say())))!.status, 502);
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

test("health is unauthenticated and reports the model and whether the binding exists", async () => {
  const { relay } = harness({ authenticate: async () => null });
  const res = await relay.handle(new Request("https://worker.example/ai/health"));
  assert.equal(res!.status, 200);
  assert.deepEqual(await res!.json(), { ok: true, model: "@cf/test/model", ai: true, upstream: "workers" });
  const { relay: bare } = harness({ ai: undefined });
  const off = await bare.handle(new Request("https://worker.example/ai/health"));
  assert.equal(((await off!.json()) as any).ai, false);
});

// ---- OpenRouter as the first upstream ----

function orHarness(answers: (() => Response)[], overrides: Partial<AiDeps> = {}) {
  const calls: { url: string; headers: Record<string, string>; body: any }[] = [];
  let i = 0;
  const fetchStub = (async (url: string, init: RequestInit) => {
    calls.push({ url, headers: init.headers as Record<string, string>, body: JSON.parse(init.body as string) });
    const make = answers[Math.min(i++, answers.length - 1)]!;
    return make();
  }) as unknown as typeof fetch;
  const h = harness({ openrouter: { key: "or-secret-key", model: "google/test-model", fetch: fetchStub }, ...overrides });
  return { ...h, calls };
}
const orOk = (content = '{"ok":true}') => () =>
  new Response(JSON.stringify({ choices: [{ message: { role: "assistant", content, reasoning: "hidden" }, finish_reason: "stop" }], usage: { prompt_tokens: 7, completion_tokens: 9, total_tokens: 16, cost: 0.00011 } }), { status: 200, headers: { "content-type": "application/json" } });
const orFail = (status = 502) => () => new Response("upstream trouble with or-secret-key", { status });

test("with OpenRouter configured the call goes there first, with reasoning off and a no-retention provider filter", async () => {
  const { relay, captured, calls } = orHarness([orOk()]);
  const res = await relay.handle(post(say()));
  assert.equal(res!.status, 200);
  assert.equal(captured.length, 0, "Workers AI is not called when OpenRouter answers");
  assert.equal(calls.length, 1);
  assert.equal(calls[0]!.url, "https://openrouter.ai/api/v1/chat/completions");
  assert.equal(calls[0]!.headers.authorization, "Bearer or-secret-key");
  assert.equal(calls[0]!.body.model, "google/test-model");
  assert.deepEqual(calls[0]!.body.reasoning, { enabled: false });
  assert.deepEqual(calls[0]!.body.usage, { include: true });
  assert.deepEqual(calls[0]!.body.response_format, { type: "json_object" });
  assert.deepEqual(calls[0]!.body.provider, { data_collection: "deny" });
  const body = (await res!.json()) as any;
  assert.equal(body.choices[0].message.content, '{"ok":true}');
  assert.equal(body.choices[0].message.reasoning, undefined);
  assert.equal(body.usage.cost, undefined);
});

test("the client still cannot choose the OpenRouter model or pass provider options", async () => {
  const { relay, calls } = orHarness([orOk()]);
  await relay.handle(post({ ...say(), model: "evil/model", provider: { order: ["x"] }, reasoning: { enabled: true } }));
  assert.equal(calls[0]!.body.model, "google/test-model");
  assert.deepEqual(calls[0]!.body.provider, { data_collection: "deny" });
  assert.deepEqual(calls[0]!.body.reasoning, { enabled: false });
});

test("privacy filtering is on by default and only the deployment can relax it", async () => {
  const stub = (calls: any[]) => (async (_u: string, init: RequestInit) => {
    calls.push(JSON.parse(init.body as string));
    return orOk()();
  }) as unknown as typeof fetch;
  const strict: any[] = [];
  await harness({ openrouter: { key: "k", model: "m", fetch: stub(strict) } }).relay.handle(post(say()));
  assert.deepEqual(strict[0].provider, { data_collection: "deny" });
  const loose: any[] = [];
  await harness({ openrouter: { key: "k", model: "m", privacy: false, fetch: stub(loose) } }).relay.handle(post(say({}.toString())));
  assert.deepEqual(loose[0].provider, { data_collection: "allow" });
});

test("when OpenRouter fails the Workers AI binding answers", async () => {
  const { relay, captured, calls } = orHarness([orFail(503)]);
  const res = await relay.handle(post(say()));
  assert.equal(res!.status, 200);
  assert.equal(calls.length, 1);
  assert.equal(captured.length, 1);
  assert.equal(((await res!.json()) as any).choices[0].message.content, '{"ok":true}');
});

test("an OpenRouter failure with no binding is tried twice and then a 502 that never shows the key or upstream text", async () => {
  const { relay, calls } = orHarness([orFail(500)], { ai: undefined });
  const res = await relay.handle(post(say()));
  assert.equal(res!.status, 502);
  assert.equal(calls.length, 2);
  const text = await res!.text();
  assert.ok(!text.includes("or-secret-key") && !text.includes("trouble"));
});

test("an empty OpenRouter completion falls back instead of returning nothing", async () => {
  const { relay, captured } = orHarness([orOk("  ")]);
  const res = await relay.handle(post(say()));
  assert.equal(res!.status, 200);
  assert.equal(captured.length, 1);
});

test("a thrown or timed-out OpenRouter call falls back", async () => {
  const boom = (async () => {
    throw new DOMException("timed out", "TimeoutError");
  }) as unknown as typeof fetch;
  const { relay, captured } = harness({ openrouter: { key: "k", model: "m", fetch: boom } });
  const res = await relay.handle(post(say()));
  assert.equal(res!.status, 200);
  assert.equal(captured.length, 1);
});

test("health names the OpenRouter model and upstream; an OpenRouter-only deployment counts as configured", async () => {
  const { relay } = orHarness([orOk()], { ai: undefined, authenticate: async () => null });
  const res = await relay.handle(new Request("https://worker.example/ai/health"));
  assert.deepEqual(await res!.json(), { ok: true, model: "google/test-model", ai: true, upstream: "openrouter" });
});
