// A fake OpenRouter for the relay tests: `run(model, body)` answers like the upstream's JSON (or throws, as a
// failed network call does), and every call is captured. `cost` is what the reply reports in `usage.cost`.
import type { AiDeps } from "../src/ai/index.ts";

export type Run = (model: string, body: Record<string, unknown>) => Promise<unknown>;

export function viaRun(run: Run, captured: { model: string; inputs: Record<string, unknown> }[] = []): NonNullable<AiDeps["openrouter"]> {
  const fetchStub = (async (_url: string, init: RequestInit) => {
    const body = JSON.parse(init.body as string) as Record<string, unknown>;
    captured.push({ model: String(body.model), inputs: body });
    const raw = await run(String(body.model), body);
    return new Response(JSON.stringify(raw), { status: 200, headers: { "content-type": "application/json" } });
  }) as unknown as typeof fetch;
  return { key: "or-test-key", model: "or/test-model", fetch: fetchStub };
}

/** A plain OpenRouter reply with the given text and cost in dollars. */
export const reply = (content = "{}", cost = 0.0001, extra: Record<string, unknown> = {}) => ({
  choices: [{ message: { role: "assistant", content }, finish_reason: "stop" }],
  usage: { prompt_tokens: 100, completion_tokens: 20, total_tokens: 120, cost },
  ...extra,
});
